//! Generic playset models and storage.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::paths::{create_dir_all, read_to_string, safe_file_stem, write_json_pretty};
use crate::{Error, Result};

/// A game-agnostic ordered playset.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playset {
    /// Stable playset identifier.
    #[serde(default)]
    pub id: String,

    /// Human-readable playset name.
    #[serde(default)]
    pub name: String,

    /// Enabled mod identifiers.
    #[serde(default)]
    pub enabled_mod_ids: Vec<String>,

    /// Ordered mod identifiers contained in the playset.
    #[serde(default)]
    pub mod_ids: Vec<String>,

    /// Disabled DLC identifiers or launcher paths.
    #[serde(default)]
    pub disabled_dlc_ids: Vec<String>,

    /// Source label for display or synchronization.
    #[serde(default)]
    pub source: String,

    /// True when the playset was imported from an external launcher.
    #[serde(default)]
    pub is_external: bool,

    /// True when consumers may edit this playset.
    #[serde(default = "default_can_edit", rename = "can_edit")]
    pub can_edit: bool,
}

impl Playset {
    /// Creates an editable playset with a generated identifier.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().simple().to_string(),
            name: name.into(),
            enabled_mod_ids: Vec::new(),
            mod_ids: Vec::new(),
            disabled_dlc_ids: Vec::new(),
            source: "Rchadow".to_owned(),
            is_external: false,
            can_edit: true,
        }
    }

    /// Creates a stable default playset.
    pub fn default_playset() -> Self {
        Self {
            id: "default".to_owned(),
            name: "Default".to_owned(),
            ..Self::named("Default")
        }
    }

    /// Returns ordered mod ids when available, otherwise enabled ids.
    pub fn ordered_mod_ids(&self) -> &[String] {
        if self.mod_ids.is_empty() {
            &self.enabled_mod_ids
        } else {
            &self.mod_ids
        }
    }

    /// Normalizes required fields in place.
    pub fn normalize(&mut self) {
        normalize_playset(self, None);
    }
}

impl Default for Playset {
    fn default() -> Self {
        Self::default_playset()
    }
}

/// A stored mod index entry that can be consumed by external tools.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModIndexEntry {
    /// Stable mod identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Source label such as local or steam.
    pub source: String,
    /// Optional upstream remote file id.
    pub remote_file_id: String,
    /// Descriptor path on disk.
    pub descriptor_path: PathBuf,
    /// Launcher path used in a game's load file.
    pub launcher_path: String,
    /// Resolved content path on disk.
    pub content_path: PathBuf,
    /// Mod version label.
    pub version: String,
}

/// A persisted index of discoverable mods.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModIndex {
    /// Schema version for the index document.
    pub schema_version: String,
    /// UTC update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Indexed mods.
    pub mods: Vec<ModIndexEntry>,
}

impl ModIndex {
    /// Creates a mod index using schema version 1.0.
    pub fn new(mods: Vec<ModIndexEntry>) -> Self {
        Self {
            schema_version: "1.0".to_owned(),
            updated_at: Utc::now(),
            mods,
        }
    }
}

/// Storage contract for playsets and related Rchadow metadata.
pub trait PlaysetDatabase {
    /// Loads all stored playsets.
    fn load_playsets(&mut self) -> Result<Vec<Playset>>;

    /// Saves one playset after normalizing required fields.
    fn save_playset(&mut self, playset: &mut Playset) -> Result<()>;

    /// Deletes one playset if it exists.
    fn delete_playset(&mut self, playset: &Playset) -> Result<()>;

    /// Loads the optional stored mod index.
    fn load_mod_index(&mut self) -> Result<Option<ModIndex>>;

    /// Saves the mod index.
    fn save_mod_index(&mut self, index: &ModIndex) -> Result<()>;
}

/// Filesystem-backed playset store.
#[derive(Clone, Debug)]
pub struct PlaysetStore {
    root_dir: PathBuf,
    playsets_dir: PathBuf,
    mods_dir: PathBuf,
    mod_index_path: PathBuf,
}

impl PlaysetStore {
    /// Creates a store rooted at `root_dir`.
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        let root_dir = root_dir.into();
        let playsets_dir = root_dir.join("playsets");
        let mods_dir = root_dir.join("mods");
        let mod_index_path = mods_dir.join("index.json");
        Self {
            root_dir,
            playsets_dir,
            mods_dir,
            mod_index_path,
        }
    }

    /// Returns the root directory.
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    /// Returns the playsets directory.
    pub fn playsets_dir(&self) -> &Path {
        &self.playsets_dir
    }

    /// Returns the mod index directory.
    pub fn mods_dir(&self) -> &Path {
        &self.mods_dir
    }

    /// Returns the mod index path.
    pub fn mod_index_path(&self) -> &Path {
        &self.mod_index_path
    }

    /// Loads every valid playset JSON document in the store.
    pub fn load_all(&self) -> Result<Vec<Playset>> {
        create_dir_all(&self.playsets_dir)?;
        let mut playsets = Vec::new();

        for entry in fs::read_dir(&self.playsets_dir).map_err(|source| Error::Io {
            path: self.playsets_dir.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| Error::Io {
                path: self.playsets_dir.clone(),
                source,
            })?;
            let path = entry.path();
            if !path.is_file() || !has_json_extension(&path) {
                continue;
            }

            let Some(mut playset) = try_load_playset(&path) else {
                continue;
            };
            let fallback_id = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned());
            normalize_playset(&mut playset, fallback_id.as_deref());
            playsets.push(playset);
        }

        playsets.sort_by_key(|playset| playset.name.to_lowercase());
        Ok(playsets)
    }

    /// Saves a playset after normalizing required fields.
    pub fn save(&self, playset: &mut Playset) -> Result<PathBuf> {
        create_dir_all(&self.playsets_dir)?;
        normalize_playset(playset, None);
        let path = self.path_for(playset);
        write_json_pretty(&path, playset)?;
        Ok(path)
    }

    /// Deletes the playset file if it exists.
    pub fn delete(&self, playset: &Playset) -> Result<()> {
        let path = self.path_for(playset);
        if path.exists() {
            fs::remove_file(&path).map_err(|source| Error::Io {
                path: path.clone(),
                source,
            })?;
        }
        Ok(())
    }

    /// Saves a mod index document.
    pub fn save_mod_index(&self, index: &ModIndex) -> Result<()> {
        create_dir_all(&self.mods_dir)?;
        write_json_pretty(&self.mod_index_path, index)
    }

    /// Builds the storage path for a playset id.
    pub fn path_for(&self, playset: &Playset) -> PathBuf {
        self.playsets_dir
            .join(format!("{}.json", safe_file_stem(&playset.id)))
    }
}

impl PlaysetDatabase for PlaysetStore {
    fn load_playsets(&mut self) -> Result<Vec<Playset>> {
        self.load_all()
    }

    fn save_playset(&mut self, playset: &mut Playset) -> Result<()> {
        self.save(playset).map(|_| ())
    }

    fn delete_playset(&mut self, playset: &Playset) -> Result<()> {
        self.delete(playset)
    }

    fn load_mod_index(&mut self) -> Result<Option<ModIndex>> {
        if !self.mod_index_path.exists() {
            return Ok(None);
        }

        let contents = read_to_string(&self.mod_index_path)?;
        serde_json::from_str(&contents)
            .map(Some)
            .map_err(|source| Error::Json {
                path: self.mod_index_path.clone(),
                source,
            })
    }

    fn save_mod_index(&mut self, index: &ModIndex) -> Result<()> {
        PlaysetStore::save_mod_index(self, index)
    }
}

fn try_load_playset(path: &Path) -> Option<Playset> {
    let contents = fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

fn normalize_playset(playset: &mut Playset, fallback_id: Option<&str>) {
    if playset.id.trim().is_empty() {
        playset.id = fallback_id
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
    }

    if playset.name.trim().is_empty() {
        playset.name = playset.id.clone();
    }

    if playset.mod_ids.is_empty() && !playset.enabled_mod_ids.is_empty() {
        playset.mod_ids = playset.enabled_mod_ids.clone();
    }

    if playset.source.trim().is_empty() {
        playset.source = "Rchadow".to_owned();
    }

    if playset.is_external {
        playset.can_edit = false;
    }
}

fn has_json_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
}

fn default_can_edit() -> bool {
    true
}
