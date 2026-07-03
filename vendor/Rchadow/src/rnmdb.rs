//! Optional RNovModularDB-compatible storage adapters.
//!
//! Rchadow intentionally does not pin RNMDB crate internals. Host projects wrap
//! RNMDB sessions or page stores and implement the small traits in this module.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::playsets::{ModIndex, Playset, PlaysetDatabase};
use crate::{Error, Result};

/// Backend name used in database errors from this module.
pub const RNMDB_BACKEND_NAME: &str = "RNMDB";

const PLAYSETS_TABLE: &str = "rchadow_playsets";
const METADATA_TABLE: &str = "rchadow_metadata";
const MOD_INDEX_KEY: &str = "mod_index";
const DISK_MAGIC: &[u8] = b"RCHADOWRNOV";
const DISK_SCHEMA_VERSION: u32 = 1;
const DISK_VERSION_OFFSET: usize = DISK_MAGIC.len();
const DISK_DOCUMENT_LEN_OFFSET: usize = DISK_VERSION_OFFSET + 4;
const DISK_PAGE_COUNT_OFFSET: usize = DISK_DOCUMENT_LEN_OFFSET + 8;
const DISK_HEADER_LEN: usize = DISK_PAGE_COUNT_OFFSET + 4;

/// RNMDB SQL value shape consumed by Rchadow adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RnmdbSqlValue {
    /// SQL TEXT value.
    Text(String),
    /// SQL NULL value.
    Null,
    /// SQL BOOL value.
    Bool(bool),
    /// SQL INT64 value.
    Int64(i64),
    /// SQL UINT64 value.
    UInt64(u64),
    /// SQL BYTES value.
    Bytes(Vec<u8>),
}

/// RNMDB SQL command output shape consumed by Rchadow adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RnmdbSqlOutput {
    /// Query result rows. Each row is represented as an ordered value vector.
    Rows(Vec<Vec<RnmdbSqlValue>>),
    /// Mutating statement result.
    RowsAffected(u64),
    /// DDL statement result.
    SchemaChanged,
    /// Text output, such as an explain plan.
    Text(String),
}

/// Minimal SQL session contract for RNMDB-backed in-memory storage.
pub trait RnmdbSqlSession {
    /// Executes one SQL statement and maps the RNMDB output into Rchadow values.
    fn execute(&mut self, sql: &str) -> Result<RnmdbSqlOutput>;
}

/// Minimal fixed-page store contract for RNMDB-backed disk storage.
pub trait RnmdbPageStore {
    /// Returns the fixed payload size, in bytes, for each page.
    fn page_size_bytes(&self) -> usize;

    /// Reads one page payload by page id.
    fn read_page(&self, page_id: u64) -> Result<Option<Vec<u8>>>;

    /// Writes one complete page payload by page id.
    fn write_page(&mut self, page_id: u64, payload: Vec<u8>) -> Result<()>;

    /// Flushes pending page writes.
    fn sync(&mut self) -> Result<()>;
}

/// RNMDB embedded in-memory SQL storage for Rchadow data.
pub struct RnmdbMemoryPlaysetDatabase<S> {
    session: S,
}

impl<S> RnmdbMemoryPlaysetDatabase<S>
where
    S: RnmdbSqlSession,
{
    /// Wraps an RNMDB SQL session and initializes the Rchadow schema.
    pub fn new(session: S) -> Result<Self> {
        Self::from_session(session)
    }

    /// Wraps an existing RNMDB SQL session and initializes the Rchadow schema.
    pub fn from_session(session: S) -> Result<Self> {
        let mut database = Self { session };
        database.initialize_schema()?;
        Ok(database)
    }

    /// Returns the underlying session.
    pub fn session(&self) -> &S {
        &self.session
    }

    /// Returns the underlying mutable session.
    pub fn session_mut(&mut self) -> &mut S {
        &mut self.session
    }

    /// Consumes the adapter and returns the underlying session.
    pub fn into_session(self) -> S {
        self.session
    }

    fn initialize_schema(&mut self) -> Result<()> {
        execute_schema(
            &mut self.session,
            "CREATE TABLE IF NOT EXISTS rchadow_playsets (id TEXT NOT NULL, document TEXT NOT NULL);",
        )?;
        execute_schema(
            &mut self.session,
            "CREATE TABLE IF NOT EXISTS rchadow_metadata (key TEXT NOT NULL, document TEXT NOT NULL);",
        )
    }
}

impl<S> PlaysetDatabase for RnmdbMemoryPlaysetDatabase<S>
where
    S: RnmdbSqlSession,
{
    fn load_playsets(&mut self) -> Result<Vec<Playset>> {
        let rows = select_text_rows(
            &mut self.session,
            "SELECT document FROM rchadow_playsets ORDER BY id;",
        )?;
        let mut playsets = rows
            .into_iter()
            .map(|document| deserialize_json::<Playset>(&document))
            .collect::<Result<Vec<_>>>()?;
        normalize_playsets(&mut playsets);
        Ok(playsets)
    }

    fn save_playset(&mut self, playset: &mut Playset) -> Result<()> {
        playset.normalize();
        let id = sql_string(&playset.id);
        let document = sql_string(&serialize_json(playset)?);
        execute_write(
            &mut self.session,
            &format!("DELETE FROM {PLAYSETS_TABLE} WHERE id = {id};"),
        )?;
        execute_write(
            &mut self.session,
            &format!("INSERT INTO {PLAYSETS_TABLE} (id, document) VALUES ({id}, {document});"),
        )
    }

    fn delete_playset(&mut self, playset: &Playset) -> Result<()> {
        execute_write(
            &mut self.session,
            &format!(
                "DELETE FROM {PLAYSETS_TABLE} WHERE id = {};",
                sql_string(&playset.id)
            ),
        )
    }

    fn load_mod_index(&mut self) -> Result<Option<ModIndex>> {
        let rows = select_text_rows(
            &mut self.session,
            &format!(
                "SELECT document FROM {METADATA_TABLE} WHERE key = {} LIMIT 1;",
                sql_string(MOD_INDEX_KEY)
            ),
        )?;
        rows.into_iter()
            .next()
            .map(|document| deserialize_json::<ModIndex>(&document))
            .transpose()
    }

    fn save_mod_index(&mut self, index: &ModIndex) -> Result<()> {
        let key = sql_string(MOD_INDEX_KEY);
        let document = sql_string(&serialize_json(index)?);
        execute_write(
            &mut self.session,
            &format!("DELETE FROM {METADATA_TABLE} WHERE key = {key};"),
        )?;
        execute_write(
            &mut self.session,
            &format!("INSERT INTO {METADATA_TABLE} (key, document) VALUES ({key}, {document});"),
        )
    }
}

/// RNMDB fixed-page disk storage for Rchadow data.
pub struct RnmdbDiskPlaysetDatabase<S> {
    store: S,
}

impl<S> RnmdbDiskPlaysetDatabase<S>
where
    S: RnmdbPageStore,
{
    /// Wraps an RNMDB page store.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Returns the underlying page store.
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Returns the underlying mutable page store.
    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    /// Consumes the adapter and returns the underlying page store.
    pub fn into_store(self) -> S {
        self.store
    }

    fn load_snapshot(&self) -> Result<RnmdbDiskSnapshot> {
        let Some(first_payload) = self.store.read_page(1)? else {
            return Ok(RnmdbDiskSnapshot::default());
        };

        decode_snapshot(&self.store, &first_payload)
    }

    fn save_snapshot(&mut self, snapshot: &RnmdbDiskSnapshot) -> Result<()> {
        let page_size = self.store.page_size_bytes();
        let pages = encode_snapshot_pages(snapshot, page_size)?;
        for (index, payload) in pages.into_iter().enumerate() {
            self.store.write_page(index as u64 + 1, payload)?;
        }
        self.store.sync()
    }
}

impl<S> PlaysetDatabase for RnmdbDiskPlaysetDatabase<S>
where
    S: RnmdbPageStore,
{
    fn load_playsets(&mut self) -> Result<Vec<Playset>> {
        let mut playsets = self.load_snapshot()?.playsets;
        normalize_playsets(&mut playsets);
        Ok(playsets)
    }

    fn save_playset(&mut self, playset: &mut Playset) -> Result<()> {
        playset.normalize();
        let mut snapshot = self.load_snapshot()?;
        snapshot
            .playsets
            .retain(|stored| !stored.id.eq_ignore_ascii_case(&playset.id));
        snapshot.playsets.push(playset.clone());
        snapshot
            .playsets
            .sort_by_key(|stored| stored.name.to_lowercase());
        self.save_snapshot(&snapshot)
    }

    fn delete_playset(&mut self, playset: &Playset) -> Result<()> {
        let mut snapshot = self.load_snapshot()?;
        snapshot
            .playsets
            .retain(|stored| !stored.id.eq_ignore_ascii_case(&playset.id));
        self.save_snapshot(&snapshot)
    }

    fn load_mod_index(&mut self) -> Result<Option<ModIndex>> {
        Ok(self.load_snapshot()?.mod_index)
    }

    fn save_mod_index(&mut self, index: &ModIndex) -> Result<()> {
        let mut snapshot = self.load_snapshot()?;
        snapshot.mod_index = Some(index.clone());
        self.save_snapshot(&snapshot)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RnmdbDiskSnapshot {
    schema_version: u32,
    playsets: Vec<Playset>,
    mod_index: Option<ModIndex>,
}

/// Builds a database error for host RNMDB wrapper implementations.
pub fn database_error(message: impl Into<String>) -> Error {
    Error::Database {
        backend: RNMDB_BACKEND_NAME,
        message: message.into(),
    }
}

fn execute_schema<S>(session: &mut S, sql: &str) -> Result<()>
where
    S: RnmdbSqlSession,
{
    match session.execute(sql)? {
        RnmdbSqlOutput::SchemaChanged | RnmdbSqlOutput::RowsAffected(_) => Ok(()),
        other => Err(database_error(format!(
            "unexpected schema output: {other:?}"
        ))),
    }
}

fn execute_write<S>(session: &mut S, sql: &str) -> Result<()>
where
    S: RnmdbSqlSession,
{
    match session.execute(sql)? {
        RnmdbSqlOutput::RowsAffected(_) | RnmdbSqlOutput::SchemaChanged => Ok(()),
        other => Err(database_error(format!(
            "unexpected write output: {other:?}"
        ))),
    }
}

fn select_text_rows<S>(session: &mut S, sql: &str) -> Result<Vec<String>>
where
    S: RnmdbSqlSession,
{
    match session.execute(sql)? {
        RnmdbSqlOutput::Rows(rows) => rows
            .into_iter()
            .map(|row| match row.first() {
                Some(RnmdbSqlValue::Text(value)) => Ok(value.clone()),
                Some(value) => Err(database_error(format!(
                    "expected TEXT value, got {value:?}"
                ))),
                None => Err(database_error("expected one selected column")),
            })
            .collect(),
        other => Err(database_error(format!(
            "unexpected query output: {other:?}"
        ))),
    }
}

fn encode_snapshot_pages(snapshot: &RnmdbDiskSnapshot, page_size: usize) -> Result<Vec<Vec<u8>>> {
    if page_size <= DISK_HEADER_LEN {
        return Err(database_error(
            "RNMDB page size is too small for Rchadow disk envelope",
        ));
    }

    let mut snapshot = snapshot.clone();
    snapshot.schema_version = DISK_SCHEMA_VERSION;
    let document = serialize_json(&snapshot)?.into_bytes();
    let first_capacity = page_size - DISK_HEADER_LEN;
    let remaining_len = document.len().saturating_sub(first_capacity);
    let page_count = 1 + remaining_len.div_ceil(page_size);
    let page_count_u32 = u32::try_from(page_count)
        .map_err(|_| database_error("Rchadow disk snapshot requires too many pages"))?;

    let mut pages = Vec::with_capacity(page_count);
    let mut offset = 0;
    for page_index in 0..page_count {
        let mut payload = vec![0_u8; page_size];
        let capacity = if page_index == 0 {
            payload[..DISK_MAGIC.len()].copy_from_slice(DISK_MAGIC);
            payload[DISK_VERSION_OFFSET..DISK_DOCUMENT_LEN_OFFSET]
                .copy_from_slice(&DISK_SCHEMA_VERSION.to_be_bytes());
            payload[DISK_DOCUMENT_LEN_OFFSET..DISK_PAGE_COUNT_OFFSET]
                .copy_from_slice(&(document.len() as u64).to_be_bytes());
            payload[DISK_PAGE_COUNT_OFFSET..DISK_HEADER_LEN]
                .copy_from_slice(&page_count_u32.to_be_bytes());
            first_capacity
        } else {
            page_size
        };
        let start = if page_index == 0 { DISK_HEADER_LEN } else { 0 };
        let copy_len = capacity.min(document.len().saturating_sub(offset));
        payload[start..start + copy_len].copy_from_slice(&document[offset..offset + copy_len]);
        offset += copy_len;
        pages.push(payload);
    }

    Ok(pages)
}

fn decode_snapshot<S>(store: &S, first_payload: &[u8]) -> Result<RnmdbDiskSnapshot>
where
    S: RnmdbPageStore,
{
    if first_payload.len() < DISK_HEADER_LEN || &first_payload[..DISK_MAGIC.len()] != DISK_MAGIC {
        return Err(database_error("invalid Rchadow RNMDB disk envelope"));
    }

    let schema_version = u32::from_be_bytes(
        first_payload[DISK_VERSION_OFFSET..DISK_DOCUMENT_LEN_OFFSET]
            .try_into()
            .expect("u32"),
    );
    if schema_version != DISK_SCHEMA_VERSION {
        return Err(database_error(format!(
            "unsupported Rchadow RNMDB disk schema version {schema_version}"
        )));
    }

    let document_len = u64::from_be_bytes(
        first_payload[DISK_DOCUMENT_LEN_OFFSET..DISK_PAGE_COUNT_OFFSET]
            .try_into()
            .expect("u64"),
    ) as usize;
    let page_count = u32::from_be_bytes(
        first_payload[DISK_PAGE_COUNT_OFFSET..DISK_HEADER_LEN]
            .try_into()
            .expect("u32"),
    ) as u64;
    let mut document = Vec::with_capacity(document_len);
    document.extend_from_slice(&first_payload[DISK_HEADER_LEN..]);

    for page_id in 2..=page_count {
        let Some(payload) = store.read_page(page_id)? else {
            return Err(database_error(format!(
                "missing Rchadow RNMDB disk page {page_id}"
            )));
        };
        document.extend_from_slice(&payload);
    }

    document.truncate(document_len);
    let text = String::from_utf8(document).map_err(|source| {
        database_error(format!("stored Rchadow snapshot is not UTF-8: {source}"))
    })?;
    deserialize_json(&text)
}

fn serialize_json<T>(value: &T) -> Result<String>
where
    T: Serialize,
{
    serde_json::to_string(value).map_err(|source| Error::Json {
        path: PathBuf::from("<rnmdb>"),
        source,
    })
}

fn deserialize_json<T>(document: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(document).map_err(|source| Error::Json {
        path: PathBuf::from("<rnmdb>"),
        source,
    })
}

fn normalize_playsets(playsets: &mut [Playset]) {
    for playset in playsets.iter_mut() {
        playset.normalize();
    }
    playsets.sort_by_key(|playset| playset.name.to_lowercase());
}

fn sql_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
