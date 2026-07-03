//! Hearts of Iron IV integration.

mod descriptors;
mod dlc_load;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::launch::{LaunchCommand, LaunchRunner, ProcessLaunchRunner, split_arguments};
use crate::paths::{create_dir_all, write_json_pretty, write_string};
use crate::playsets::Playset;
use crate::{Error, Result};

use descriptors::{
    escape_clausewitz_string, generated_descriptor_file_name, parse_mod_descriptor,
    read_clausewitz_key_values, relative_launcher_path,
};
pub use dlc_load::DlcLoadDocument;

/// Paths required for HOI4 playset operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hoi4Paths {
    /// HOI4 user directory containing `mod` and `dlc_load.json`.
    pub game_user_dir: PathBuf,

    /// Optional Steam workshop content directory.
    pub workshop_dir: Option<PathBuf>,

    /// Optional executable path used for launch.
    pub game_executable: Option<PathBuf>,
}

/// HOI4 mod metadata discovered from a descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModEntry {
    /// Stable identifier. Steam mods use `remote_file_id`; local mods use descriptor path.
    pub id: String,

    /// Display title.
    pub title: String,

    /// Descriptor file path.
    pub descriptor_path: PathBuf,

    /// Raw path or archive value from the descriptor.
    pub raw_content_path: String,

    /// Steam workshop id when present.
    pub remote_file_id: String,

    /// Launcher path used in `dlc_load.json`.
    pub launcher_path: String,

    /// Resolved content directory or archive path.
    pub content_path: PathBuf,

    /// Version label.
    pub version: String,
}

impl ModEntry {
    /// Returns true when this mod came from Steam Workshop metadata.
    pub fn is_steam_workshop_mod(&self) -> bool {
        !self.remote_file_id.trim().is_empty()
    }
}

/// HOI4 DLC metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DlcEntry {
    /// Launcher path used by HOI4.
    pub id: String,

    /// Display title.
    pub title: String,

    /// Descriptor file path.
    pub descriptor_path: PathBuf,

    /// Launcher-relative descriptor path.
    pub launcher_path: String,
}

/// Options used when applying a playset.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ApplyPlaysetOptions {
    /// Launcher DLC paths that should be disabled.
    pub disabled_dlc_launcher_paths: Vec<String>,
}

/// Result of applying a playset.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AppliedPlayset {
    /// Enabled launcher mod paths written to `dlc_load.json`.
    pub enabled_mods: Vec<String>,

    /// Disabled DLC launcher paths written to `dlc_load.json`.
    pub disabled_dlcs: Vec<String>,

    /// Path to the written load document.
    pub dlc_load_path: PathBuf,
}

/// High-level HOI4 playset manager.
#[derive(Clone, Debug)]
pub struct Hoi4PlaysetManager {
    paths: Hoi4Paths,
}

impl Hoi4PlaysetManager {
    /// Creates a manager from explicit paths.
    pub fn new(paths: Hoi4Paths) -> Self {
        Self { paths }
    }

    /// Returns configured paths.
    pub fn paths(&self) -> &Hoi4Paths {
        &self.paths
    }

    /// Returns the default HOI4 user directory for the current platform.
    pub fn default_game_user_dir() -> Option<PathBuf> {
        directories::UserDirs::new().map(|dirs| {
            dirs.document_dir()
                .unwrap_or_else(|| dirs.home_dir())
                .join("Paradox Interactive")
                .join("Hearts of Iron IV")
        })
    }

    /// Discovers mods from the configured user and workshop directories.
    pub fn discover_mods(&self) -> Result<Vec<ModEntry>> {
        let mut descriptors = Vec::new();
        let local_mod_dir = self.paths.game_user_dir.join("mod");
        descriptors.extend(find_named_files(&local_mod_dir, "*.mod")?);

        if let Some(workshop_dir) = &self.paths.workshop_dir {
            descriptors.extend(find_named_files(workshop_dir, "*.mod")?);
            descriptors.extend(find_named_files(workshop_dir, "descriptor.mod")?);
        }

        self.discover_mods_from_paths(dedup_paths(descriptors))
    }

    /// Discovers mods from explicit descriptor paths.
    pub fn discover_mods_from_paths<I, P>(&self, paths: I) -> Result<Vec<ModEntry>>
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        let mut mods = paths
            .into_iter()
            .map(Into::into)
            .map(|path| parse_mod_descriptor(&path, &self.paths.game_user_dir))
            .collect::<Result<Vec<_>>>()?;

        mods.sort_by_key(|entry| entry.title.to_lowercase());
        Ok(mods)
    }

    /// Discovers DLC descriptors from the configured executable directory.
    pub fn discover_dlcs(&self) -> Result<Vec<DlcEntry>> {
        let Some(executable) = &self.paths.game_executable else {
            return Ok(Vec::new());
        };
        let Some(game_dir) = executable.parent() else {
            return Ok(Vec::new());
        };
        let dlc_dir = game_dir.join("dlc");
        if !dlc_dir.exists() {
            return Ok(Vec::new());
        }

        let mut dlcs = Vec::new();
        for descriptor_path in find_named_files(&dlc_dir, "*.dlc")? {
            let values = read_clausewitz_key_values(&descriptor_path)?;
            let title = values.get("name").cloned().unwrap_or_else(|| {
                descriptor_path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Unknown DLC".to_owned())
            });
            let launcher_path = relative_launcher_path(game_dir, &descriptor_path);
            dlcs.push(DlcEntry {
                id: launcher_path.clone(),
                title,
                descriptor_path,
                launcher_path,
            });
        }

        dlcs.sort_by_key(|entry| entry.title.to_lowercase());
        Ok(dlcs)
    }

    /// Applies a playset by writing generated descriptors and `dlc_load.json`.
    pub fn apply_playset(
        &self,
        playset: &Playset,
        mods: &[ModEntry],
        options: ApplyPlaysetOptions,
    ) -> Result<AppliedPlayset> {
        let enabled_ids = playset
            .enabled_mod_ids
            .iter()
            .map(|id| id.to_lowercase())
            .collect::<HashSet<_>>();
        let mut enabled_mods = Vec::new();

        for mod_id in playset.ordered_mod_ids() {
            if !enabled_ids.contains(&mod_id.to_lowercase()) {
                continue;
            }

            let Some(mod_entry) = mods
                .iter()
                .find(|entry| entry.id.eq_ignore_ascii_case(mod_id))
            else {
                continue;
            };
            let launcher_path = self.ensure_launcher_mod_descriptor(mod_entry)?;
            if !enabled_mods
                .iter()
                .any(|existing: &String| existing.eq_ignore_ascii_case(&launcher_path))
            {
                enabled_mods.push(launcher_path);
            }
        }

        let disabled_dlcs = dedup_strings(options.disabled_dlc_launcher_paths);
        let document = DlcLoadDocument {
            enabled_mods: enabled_mods.clone(),
            disabled_dlcs: disabled_dlcs.clone(),
        };
        let dlc_load_path = self.paths.game_user_dir.join("dlc_load.json");
        create_dir_all(&self.paths.game_user_dir)?;
        write_json_pretty(&dlc_load_path, &document)?;

        Ok(AppliedPlayset {
            enabled_mods,
            disabled_dlcs,
            dlc_load_path,
        })
    }

    /// Builds and launches HOI4 using the default process runner.
    pub fn launch(&self, launch_arguments: &str) -> Result<()> {
        self.launch_with_runner(launch_arguments, &ProcessLaunchRunner)
    }

    /// Builds and launches HOI4 using an injected runner.
    pub fn launch_with_runner(
        &self,
        launch_arguments: &str,
        runner: &dyn LaunchRunner,
    ) -> Result<()> {
        let executable = self
            .paths
            .game_executable
            .as_ref()
            .ok_or(Error::MissingPath("game_executable"))?;
        if !executable.exists() {
            return Err(Error::PathNotFound {
                path: executable.clone(),
            });
        }

        let mut command = LaunchCommand::new(executable);
        command.args = split_arguments(launch_arguments);
        runner.run(&command)
    }

    fn ensure_launcher_mod_descriptor(&self, mod_entry: &ModEntry) -> Result<String> {
        let local_mod_dir = self.paths.game_user_dir.join("mod");
        if crate::paths::is_inside_directory(&mod_entry.descriptor_path, &local_mod_dir) {
            return Ok(mod_entry.launcher_path.clone());
        }

        create_dir_all(&local_mod_dir)?;
        let descriptor_file_name =
            generated_descriptor_file_name(&mod_entry.descriptor_path, &mod_entry.remote_file_id);
        let descriptor_path = local_mod_dir.join(&descriptor_file_name);
        let content_path = if mod_entry.content_path.as_os_str().is_empty() {
            mod_entry
                .descriptor_path
                .parent()
                .map(PathBuf::from)
                .unwrap_or_default()
        } else {
            mod_entry.content_path.clone()
        };
        let content_key = if content_path.is_file() {
            "archive"
        } else {
            "path"
        };
        let mut lines = vec![
            format!("name=\"{}\"", escape_clausewitz_string(&mod_entry.title)),
            format!(
                "{}=\"{}\"",
                content_key,
                escape_clausewitz_string(&content_path.to_string_lossy())
            ),
        ];

        if !mod_entry.remote_file_id.trim().is_empty() {
            lines.push(format!(
                "remote_file_id=\"{}\"",
                escape_clausewitz_string(&mod_entry.remote_file_id)
            ));
        }
        lines.push(String::new());

        write_string(&descriptor_path, &lines.join("\n"))?;
        Ok(format!("mod/{descriptor_file_name}"))
    }
}

fn find_named_files(root: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|source| Error::Io {
            path: root.to_path_buf(),
            source: std::io::Error::other(source),
        })?;
        if !entry.file_type().is_file() {
            continue;
        }

        let file_name = entry.file_name().to_string_lossy();
        let matches = if let Some(extension) = pattern.strip_prefix("*.") {
            entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|actual| actual.eq_ignore_ascii_case(extension))
        } else {
            file_name.eq_ignore_ascii_case(pattern)
        };

        if matches {
            paths.push(entry.path().to_path_buf());
        }
    }

    Ok(paths)
}

fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(path.to_string_lossy().to_lowercase()))
        .collect()
}

fn dedup_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.to_lowercase()))
        .collect()
}
