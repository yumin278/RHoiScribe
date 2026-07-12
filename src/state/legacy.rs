//------------------------------------------------------------------------------------
// state/legacy.rs -- Part of RHoiScribe
//
// Copyright (C) 2026 CzXieDdan. All rights reserved.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// https://github.com/czxieddan/RHoiScribe
//------------------------------------------------------------------------------------

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rnmdb_common::ids::PageId;
use rnmdb_storage::{
    PageCryptoKey, SingleFileBackend, SingleFileFormatCompatibilityStatus, SingleFileOptions,
    StorageBackend, check_single_file_format_compatibility, upgrade_single_file_with_key,
    verify_single_file_with_key,
};
use serde::Deserialize;
use serde_json::Value;

use super::{
    GLOBAL_SCOPE_KEY, GLOBAL_SCOPE_KIND, StateMigrationReport, StoredPreferenceRecord,
    StoredToolLogRecord, global_record_key,
    path::{legacy_state_database_path, page_crypto_key, sync_parent_directory},
    state_database_error,
    store::RnmdbStateStore,
};

const PREFERENCES_PAGE_ID: u64 = 1;
const TOOL_LOG_INDEX_PAGE_ID: u64 = 2;
const TOOL_LOG_DATA_START_PAGE_ID: u64 = 3;
const SQL_FRAME_MAGIC: &[u8; 8] = b"RNOVSI01";
const LEGACY_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
struct LegacyPreferences {
    schema_version: u32,
    preferences: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct LegacyToolLogIndex {
    schema_version: u32,
    byte_len: usize,
    page_count: u64,
}

impl LegacyToolLogIndex {
    fn is_empty(&self) -> bool {
        self.byte_len == 0 && self.page_count == 0
    }
}

#[derive(Debug, Deserialize)]
struct LegacyToolLogEntry {
    sequence: u64,
    timestamp_unix_seconds: u64,
    tool_name: String,
    arguments: Value,
    success: bool,
    result: Option<Value>,
    error: Option<String>,
}

struct LegacySnapshot {
    preferences: Vec<StoredPreferenceRecord>,
    logs: Vec<StoredToolLogRecord>,
}

struct ReadableSource {
    original_path: PathBuf,
    readable_path: PathBuf,
    temporary_upgrade: Option<PathBuf>,
}

enum ExistingLayout {
    Sql,
    Legacy(LegacySnapshot),
}

pub(super) fn prepare_state_database(
    canonical_path: &Path,
) -> Result<Option<StateMigrationReport>, String> {
    let Some(source_path) = existing_source_path(canonical_path) else {
        return Ok(None);
    };
    let key =
        page_crypto_key().map_err(|error| state_database_error(canonical_path, "open", error))?;
    let readable = prepare_readable_source(&source_path, canonical_path, key)?;
    let layout = match inspect_existing_layout(&readable.readable_path, canonical_path, key) {
        Ok(layout) => layout,
        Err(error) => return Err(clean_readable_source(&readable, canonical_path, error)),
    };
    finish_existing_layout(readable, canonical_path, key, layout)
}

fn existing_source_path(canonical_path: &Path) -> Option<PathBuf> {
    if canonical_path.is_file() {
        return Some(canonical_path.to_path_buf());
    }
    let legacy_path = legacy_state_database_path(canonical_path);
    legacy_path.is_file().then_some(legacy_path)
}

fn prepare_readable_source(
    source_path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
) -> Result<ReadableSource, String> {
    let compatibility = check_single_file_format_compatibility(source_path)
        .map_err(|error| state_database_error(canonical_path, "open", error.to_string()))?;
    match compatibility.status() {
        SingleFileFormatCompatibilityStatus::Supported => Ok(ReadableSource {
            original_path: source_path.to_path_buf(),
            readable_path: source_path.to_path_buf(),
            temporary_upgrade: None,
        }),
        SingleFileFormatCompatibilityStatus::UnsupportedOlder => {
            upgrade_legacy_format(source_path, canonical_path, key)
        }
        SingleFileFormatCompatibilityStatus::UnsupportedNewer => Err(state_database_error(
            canonical_path,
            "open",
            "state database requires a newer RNMDB engine",
        )),
    }
}

fn upgrade_legacy_format(
    source_path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
) -> Result<ReadableSource, String> {
    let target = unique_temporary_path(canonical_path, "legacy-format-upgrade")?;
    reject_existing_target(canonical_path, &target, "migrate")?;
    if let Err(error) = upgrade_single_file_with_key(source_path, &target, key) {
        let error = state_database_error(canonical_path, "migrate", error.to_string());
        return Err(clean_created_migration(&target, canonical_path, error));
    }
    if let Err(error) = verify_authenticated(&target, canonical_path, key) {
        return Err(clean_created_migration(&target, canonical_path, error));
    }
    if let Err(error) = sync_verified_temporary(&target, canonical_path) {
        return Err(clean_created_migration(&target, canonical_path, error));
    }
    Ok(ReadableSource {
        original_path: source_path.to_path_buf(),
        readable_path: target.clone(),
        temporary_upgrade: Some(target),
    })
}

fn inspect_existing_layout(
    readable_path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
) -> Result<ExistingLayout, String> {
    let backend = SingleFileBackend::open_with_key(readable_path, key)
        .map_err(|error| state_database_error(canonical_path, "open", error.to_string()))?;
    if backend.catalog_root().is_some() {
        return Ok(ExistingLayout::Sql);
    }
    if legacy_root_is_sql_frame(&backend, canonical_path)? {
        return Ok(ExistingLayout::Sql);
    }
    read_legacy_snapshot(&backend, canonical_path)
        .map(ExistingLayout::Legacy)
        .map_err(|error| state_database_error(canonical_path, "migrate", error))
}

fn legacy_root_is_sql_frame(
    backend: &SingleFileBackend,
    canonical_path: &Path,
) -> Result<bool, String> {
    let page = read_page(backend, canonical_path, PREFERENCES_PAGE_ID)?;
    Ok(page.is_some_and(|payload| payload.starts_with(SQL_FRAME_MAGIC)))
}

fn finish_existing_layout(
    readable: ReadableSource,
    canonical_path: &Path,
    key: PageCryptoKey,
    layout: ExistingLayout,
) -> Result<Option<StateMigrationReport>, String> {
    match layout {
        ExistingLayout::Sql => finish_existing_sql(readable, canonical_path, key),
        ExistingLayout::Legacy(snapshot) => {
            remove_temporary_upgrade(&readable, canonical_path)?;
            migrate_snapshot(&readable.original_path, canonical_path, key, snapshot)
        }
    }
}

fn finish_existing_sql(
    readable: ReadableSource,
    canonical_path: &Path,
    key: PageCryptoKey,
) -> Result<Option<StateMigrationReport>, String> {
    if let Err(error) = verify_authenticated(&readable.readable_path, canonical_path, key) {
        return Err(clean_readable_source(&readable, canonical_path, error));
    }
    if readable.readable_path != readable.original_path {
        let backup = match unique_backup_path(canonical_path) {
            Ok(path) => path,
            Err(error) => return Err(clean_readable_source(&readable, canonical_path, error)),
        };
        return swap_database(
            &readable.original_path,
            &readable.readable_path,
            &backup,
            canonical_path,
        )
        .map(Some);
    }
    if readable.original_path != canonical_path {
        promote_legacy_name(&readable.original_path, canonical_path)?;
    }
    Ok(None)
}

fn promote_legacy_name(source: &Path, canonical_path: &Path) -> Result<(), String> {
    reject_existing_target(canonical_path, canonical_path, "swap")?;
    fs::rename(source, canonical_path)
        .map_err(|error| state_database_error(canonical_path, "swap", error.to_string()))?;
    if let Err(error) = sync_parent_directory(canonical_path) {
        return restore_legacy_name(source, canonical_path, error);
    }
    Ok(())
}

fn restore_legacy_name(
    source: &Path,
    canonical_path: &Path,
    failure: String,
) -> Result<(), String> {
    let restore = match fs::rename(canonical_path, source) {
        Ok(()) => directory_sync_status(source),
        Err(error) => format!("restore failed: {error}"),
    };
    Err(state_database_error(
        canonical_path,
        "swap",
        format!("legacy-name directory sync failed: {failure}; {restore}"),
    ))
}

fn read_legacy_snapshot(
    backend: &SingleFileBackend,
    canonical_path: &Path,
) -> Result<LegacySnapshot, String> {
    let preferences = read_legacy_preferences(backend, canonical_path)?;
    let logs = read_legacy_logs(backend, canonical_path)?;
    Ok(LegacySnapshot { preferences, logs })
}

fn read_legacy_preferences(
    backend: &SingleFileBackend,
    canonical_path: &Path,
) -> Result<Vec<StoredPreferenceRecord>, String> {
    let Some(payload) = read_page(backend, canonical_path, PREFERENCES_PAGE_ID)? else {
        return Ok(Vec::new());
    };
    let Some(preferences) = decode_length_prefixed::<LegacyPreferences>(&payload, "preferences")?
    else {
        return Ok(Vec::new());
    };
    validate_legacy_schema(preferences.schema_version, "preferences")?;
    let updated_at = unix_timestamp_now();
    preferences
        .preferences
        .into_iter()
        .map(|(key, value)| legacy_preference_record(key, value, updated_at))
        .collect()
}

fn legacy_preference_record(
    preference_key: String,
    value: Value,
    updated_at_unix_seconds: u64,
) -> Result<StoredPreferenceRecord, String> {
    let value_json = serde_json::to_string(&value).map_err(|error| error.to_string())?;
    Ok(StoredPreferenceRecord {
        record_key: global_record_key(&preference_key),
        scope_kind: GLOBAL_SCOPE_KIND.to_string(),
        scope_key: GLOBAL_SCOPE_KEY.to_string(),
        mod_root: None,
        preference_key,
        value_json,
        updated_at_unix_seconds,
    })
}

fn read_legacy_logs(
    backend: &SingleFileBackend,
    canonical_path: &Path,
) -> Result<Vec<StoredToolLogRecord>, String> {
    let Some(payload) = read_page(backend, canonical_path, TOOL_LOG_INDEX_PAGE_ID)? else {
        return Ok(Vec::new());
    };
    let Some(index) = decode_length_prefixed::<LegacyToolLogIndex>(&payload, "tool log index")?
    else {
        return Ok(Vec::new());
    };
    validate_legacy_schema(index.schema_version, "tool log index")?;
    if index.is_empty() {
        return Ok(Vec::new());
    }
    validate_log_index(&index, backend)?;
    let bytes = read_legacy_log_bytes(backend, canonical_path, &index)?;
    let entries = serde_json::from_slice::<Vec<LegacyToolLogEntry>>(&bytes)
        .map_err(|error| format!("failed to decode legacy tool logs: {error}"))?;
    entries.into_iter().map(legacy_log_record).collect()
}

fn validate_log_index(
    index: &LegacyToolLogIndex,
    backend: &SingleFileBackend,
) -> Result<(), String> {
    let page_size = backend.page_size().bytes();
    let expected = index.byte_len.div_ceil(page_size);
    let actual = usize::try_from(index.page_count)
        .map_err(|_| "legacy tool log page count does not fit this platform".to_string())?;
    if actual != expected {
        return Err(format!(
            "legacy tool log index page count {actual} does not match byte length {}",
            index.byte_len
        ));
    }
    let file_len = fs::metadata(backend.path())
        .map_err(|error| format!("failed to inspect legacy state database size: {error}"))?
        .len();
    let byte_len = u64::try_from(index.byte_len)
        .map_err(|_| "legacy tool log byte length does not fit RNMDB limits".to_string())?;
    if byte_len > file_len {
        return Err("legacy tool log byte length exceeds the database file size".to_string());
    }
    Ok(())
}

fn read_legacy_log_bytes(
    backend: &SingleFileBackend,
    canonical_path: &Path,
    index: &LegacyToolLogIndex,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(index.byte_len)
        .map_err(|error| format!("legacy tool log allocation failed: {error}"))?;
    for offset in 0..index.page_count {
        let page_id = TOOL_LOG_DATA_START_PAGE_ID.saturating_add(offset);
        let payload = read_page(backend, canonical_path, page_id)?
            .ok_or_else(|| format!("legacy tool log page {page_id} is missing"))?;
        bytes.extend_from_slice(&payload);
    }
    bytes.truncate(index.byte_len);
    Ok(bytes)
}

fn legacy_log_record(entry: LegacyToolLogEntry) -> Result<StoredToolLogRecord, String> {
    let arguments_json =
        serde_json::to_string(&entry.arguments).map_err(|error| error.to_string())?;
    let result_json = entry
        .result
        .map(|value| serde_json::to_string(&value))
        .transpose()
        .map_err(|error| error.to_string())?;
    Ok(StoredToolLogRecord {
        sequence: entry.sequence,
        timestamp_unix_seconds: entry.timestamp_unix_seconds,
        scope_kind: GLOBAL_SCOPE_KIND.to_string(),
        scope_key: GLOBAL_SCOPE_KEY.to_string(),
        mod_root: None,
        tool_name: entry.tool_name,
        arguments_json,
        success: entry.success,
        result_json,
        error_text: entry.error,
    })
}

fn read_page(
    backend: &SingleFileBackend,
    canonical_path: &Path,
    page_id: u64,
) -> Result<Option<Vec<u8>>, String> {
    backend
        .read_page(PageId::new(page_id))
        .map(|page| page.map(|page| page.payload().to_vec()))
        .map_err(|error| {
            state_database_error(
                canonical_path,
                "migrate",
                format!("failed to read legacy page {page_id}: {error}"),
            )
        })
}

fn decode_length_prefixed<T>(payload: &[u8], label: &str) -> Result<Option<T>, String>
where
    T: for<'de> Deserialize<'de>,
{
    if payload.len() < 4 {
        return Ok(None);
    }
    let length = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
    if length == 0 {
        return Ok(None);
    }
    if length > payload.len().saturating_sub(4) {
        return Err(format!("legacy {label} page has an invalid payload length"));
    }
    serde_json::from_slice(&payload[4..4 + length])
        .map(Some)
        .map_err(|error| format!("failed to decode legacy {label}: {error}"))
}

fn validate_legacy_schema(version: u32, label: &str) -> Result<(), String> {
    if version <= LEGACY_SCHEMA_VERSION {
        return Ok(());
    }
    Err(format!(
        "legacy {label} schema version {version} is newer than supported version {LEGACY_SCHEMA_VERSION}"
    ))
}

fn migrate_snapshot(
    source_path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
    snapshot: LegacySnapshot,
) -> Result<Option<StateMigrationReport>, String> {
    let backup_path = unique_backup_path(canonical_path)?;
    let migration_path = unique_temporary_path(canonical_path, "migrating-sql-v2")?;
    reject_existing_target(canonical_path, &migration_path, "migrate")?;
    reserve_migration_database(&migration_path, canonical_path, key)?;
    let build_result = build_migration_database(
        &migration_path,
        canonical_path,
        &backup_path,
        source_path,
        key,
        &snapshot,
    );
    if let Err(error) = build_result {
        return Err(clean_created_migration(
            &migration_path,
            canonical_path,
            error,
        ));
    }
    swap_database(source_path, &migration_path, &backup_path, canonical_path).map(Some)
}

fn reserve_migration_database(
    migration_path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
) -> Result<(), String> {
    SingleFileBackend::create(
        migration_path,
        SingleFileOptions::default().with_page_key(key),
    )
    .map(drop)
    .map_err(|error| state_database_error(canonical_path, "migrate", error.to_string()))
}

fn build_migration_database(
    migration_path: &Path,
    canonical_path: &Path,
    backup_path: &Path,
    source_path: &Path,
    key: PageCryptoKey,
    snapshot: &LegacySnapshot,
) -> Result<(), String> {
    let mut store = RnmdbStateStore::create_migration(migration_path, canonical_path)?;
    store.import_legacy(
        &snapshot.preferences,
        &snapshot.logs,
        source_path,
        backup_path,
    )?;
    store.verify_import(
        snapshot.preferences.len(),
        snapshot.logs.len(),
        source_path,
        backup_path,
    )?;
    drop(store);
    verify_authenticated(migration_path, canonical_path, key)?;
    sync_verified_temporary(migration_path, canonical_path)
}

fn clean_created_migration(path: &Path, canonical_path: &Path, error: String) -> String {
    match cleanup_temporary(path) {
        Ok(()) => error,
        Err(cleanup) => state_database_error(
            canonical_path,
            "migrate",
            format!("{error}; failed to remove incomplete migration database: {cleanup}"),
        ),
    }
}

fn verify_authenticated(
    path: &Path,
    canonical_path: &Path,
    key: PageCryptoKey,
) -> Result<(), String> {
    let report = verify_single_file_with_key(path, key)
        .map_err(|error| state_database_error(canonical_path, "verify", error.to_string()))?;
    if report.encryption_authenticated() && report.is_valid() {
        return Ok(());
    }
    Err(state_database_error(
        canonical_path,
        "verify",
        "RNMDB authenticated verification did not validate every stored page",
    ))
}

fn sync_verified_temporary(path: &Path, canonical_path: &Path) -> Result<(), String> {
    sync_parent_directory(path)
        .map_err(|error| state_database_error(canonical_path, "verify", error))
}

fn swap_database(
    source: &Path,
    migration: &Path,
    backup: &Path,
    canonical_path: &Path,
) -> Result<StateMigrationReport, String> {
    if let Err(error) = reject_existing_target(canonical_path, backup, "swap") {
        return Err(clean_created_migration(migration, canonical_path, error));
    }
    if let Err(error) = fs::rename(source, backup) {
        let error = state_database_error(canonical_path, "swap", error.to_string());
        return Err(clean_created_migration(migration, canonical_path, error));
    }
    if let Err(error) = sync_parent_directory(backup) {
        return recover_uninstalled(source, migration, backup, canonical_path, error);
    }
    install_migration(source, migration, backup, canonical_path)
}

fn install_migration(
    source: &Path,
    migration: &Path,
    backup: &Path,
    canonical_path: &Path,
) -> Result<StateMigrationReport, String> {
    if let Err(error) = reject_existing_target(canonical_path, canonical_path, "swap") {
        return recover_uninstalled(source, migration, backup, canonical_path, error);
    }
    if let Err(error) = fs::rename(migration, canonical_path) {
        return recover_uninstalled(source, migration, backup, canonical_path, error.to_string());
    }
    if let Err(error) = sync_parent_directory(canonical_path) {
        return recover_installed(source, backup, canonical_path, error);
    }
    Ok(StateMigrationReport {
        retained_backup_path: backup.to_path_buf(),
    })
}

fn recover_uninstalled(
    source: &Path,
    migration: &Path,
    backup: &Path,
    canonical_path: &Path,
    failure: String,
) -> Result<StateMigrationReport, String> {
    let restore = restore_original(backup, source);
    let cleanup = cleanup_status(migration);
    Err(recovery_error(canonical_path, &failure, &restore, &cleanup))
}

fn recover_installed(
    source: &Path,
    backup: &Path,
    canonical_path: &Path,
    failure: String,
) -> Result<StateMigrationReport, String> {
    let cleanup = cleanup_status(canonical_path);
    let restore = match path_entry_exists(canonical_path) {
        Ok(true) => "original restore not attempted because replacement cleanup failed".to_string(),
        Ok(false) => restore_original(backup, source),
        Err(error) => format!(
            "original restore not attempted because replacement path could not be inspected: {error}"
        ),
    };
    Err(recovery_error(canonical_path, &failure, &restore, &cleanup))
}

fn restore_original(backup: &Path, source: &Path) -> String {
    match path_entry_exists(source) {
        Ok(true) => {
            return format!(
                "original restore refused because source path already exists; original remains at {}",
                backup.to_string_lossy()
            );
        }
        Err(error) => {
            return format!(
                "original restore refused because source path could not be inspected ({error}); original remains at {}",
                backup.to_string_lossy()
            );
        }
        Ok(false) => {}
    }
    match fs::rename(backup, source) {
        Ok(()) => format!("original restored; {}", directory_sync_status(source)),
        Err(error) => format!(
            "original restore failed: {error}; original remains at {}",
            backup.to_string_lossy()
        ),
    }
}

fn recovery_error(canonical_path: &Path, failure: &str, restore: &str, cleanup: &str) -> String {
    state_database_error(
        canonical_path,
        "swap",
        format!("install failed: {failure}; restore status: {restore}; cleanup status: {cleanup}"),
    )
}

fn cleanup_status(path: &Path) -> String {
    match cleanup_temporary(path) {
        Ok(()) => "temporary database removed and directory synced".to_string(),
        Err(error) => format!("temporary database cleanup failed: {error}"),
    }
}

fn cleanup_temporary(path: &Path) -> Result<(), String> {
    let Some(metadata) = path_entry_metadata(path)? else {
        return Ok(());
    };
    if !metadata.file_type().is_file() {
        return Err(format!(
            "refusing to remove non-file temporary database entry {}",
            path.to_string_lossy()
        ));
    }
    fs::remove_file(path).map_err(|error| error.to_string())?;
    sync_parent_directory(path)
}

fn directory_sync_status(path: &Path) -> String {
    match sync_parent_directory(path) {
        Ok(()) => "parent directory synced".to_string(),
        Err(error) => format!("parent directory sync failed: {error}"),
    }
}

fn unique_backup_path(canonical_path: &Path) -> Result<PathBuf, String> {
    unique_sibling_path(canonical_path, "pre-sql-v2", "swap")
}

fn unique_temporary_path(canonical_path: &Path, label: &str) -> Result<PathBuf, String> {
    unique_sibling_path(canonical_path, label, "migrate")
}

fn unique_sibling_path(canonical_path: &Path, label: &str, stage: &str) -> Result<PathBuf, String> {
    for suffix in 0..10_000_u32 {
        let label = if suffix == 0 {
            label.to_string()
        } else {
            format!("{label}.{suffix}")
        };
        let candidate = sibling_path(canonical_path, &label, stage)?;
        let occupied = path_entry_exists(&candidate)
            .map_err(|error| state_database_error(canonical_path, stage, error))?;
        if !occupied {
            return Ok(candidate);
        }
    }
    Err(state_database_error(
        canonical_path,
        stage,
        format!("could not allocate a unique {label} sibling path"),
    ))
}

fn sibling_path(canonical_path: &Path, label: &str, stage: &str) -> Result<PathBuf, String> {
    let stem = canonical_path
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| state_database_error(canonical_path, stage, "invalid database name"))?;
    let file_name = format!("{}.{}.rnmdb", stem.to_string_lossy(), label);
    Ok(canonical_path.with_file_name(file_name))
}

fn reject_existing_target(canonical_path: &Path, target: &Path, stage: &str) -> Result<(), String> {
    let occupied = path_entry_exists(target)
        .map_err(|error| state_database_error(canonical_path, stage, error))?;
    if !occupied {
        return Ok(());
    }
    Err(state_database_error(
        canonical_path,
        stage,
        format!(
            "refusing to overwrite migration target {}",
            target.to_string_lossy()
        ),
    ))
}

fn path_entry_exists(path: &Path) -> Result<bool, String> {
    path_entry_metadata(path).map(|metadata| metadata.is_some())
}

fn path_entry_metadata(path: &Path) -> Result<Option<fs::Metadata>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "failed to inspect path entry {}: {error}",
            path.to_string_lossy()
        )),
    }
}

fn remove_temporary_upgrade(
    readable: &ReadableSource,
    canonical_path: &Path,
) -> Result<(), String> {
    let Some(path) = &readable.temporary_upgrade else {
        return Ok(());
    };
    cleanup_temporary(path).map_err(|error| state_database_error(canonical_path, "migrate", error))
}

fn clean_readable_source(
    readable: &ReadableSource,
    canonical_path: &Path,
    error: String,
) -> String {
    let Some(path) = &readable.temporary_upgrade else {
        return error;
    };
    clean_created_migration(path, canonical_path, error)
}

fn unix_timestamp_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
