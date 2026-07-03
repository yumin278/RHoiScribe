use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::Result;
use crate::paths::{
    hash_hex, is_inside_directory, read_to_string, resolve_content_path, safe_file_stem,
    to_forward_slash_path,
};

use super::ModEntry;

/// Parses simple Clausewitz key-value scalar lines from descriptor files.
pub fn read_clausewitz_key_values(path: &Path) -> Result<HashMap<String, String>> {
    let contents = read_to_string(path)?;
    let mut values = HashMap::new();

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        values.insert(key.trim().to_lowercase(), unquote(value.trim()));
    }

    Ok(values)
}

/// Parses a HOI4 mod descriptor into a library model.
pub fn parse_mod_descriptor(path: &Path, game_user_dir: &Path) -> Result<ModEntry> {
    let values = read_clausewitz_key_values(path)?;
    let title = values
        .get("name")
        .cloned()
        .unwrap_or_else(|| fallback_title(path));
    let remote_file_id = values.get("remote_file_id").cloned().unwrap_or_default();
    let version = values
        .get("version")
        .or_else(|| values.get("supported_version"))
        .cloned()
        .unwrap_or_default();
    let raw_content_path = values
        .get("path")
        .or_else(|| values.get("archive"))
        .cloned()
        .unwrap_or_default();
    let content_path = if raw_content_path.trim().is_empty() {
        path.parent().map(PathBuf::from).unwrap_or_default()
    } else {
        resolve_content_path(path, &raw_content_path, Some(game_user_dir))
    };

    let id = if remote_file_id.trim().is_empty() {
        path.to_string_lossy().to_uppercase()
    } else {
        remote_file_id.clone()
    };
    let launcher_path = launcher_mod_path(path, game_user_dir, &remote_file_id);

    Ok(ModEntry {
        id,
        title,
        descriptor_path: path.to_path_buf(),
        raw_content_path,
        remote_file_id,
        launcher_path,
        content_path,
        version,
    })
}

/// Returns the launcher descriptor path for a descriptor.
pub fn launcher_mod_path(
    descriptor_path: &Path,
    game_user_dir: &Path,
    remote_file_id: &str,
) -> String {
    if !remote_file_id.trim().is_empty() {
        return format!("mod/ugc_{remote_file_id}.mod");
    }

    let local_mod_dir = game_user_dir.join("mod");
    if is_inside_directory(descriptor_path, &local_mod_dir) {
        if let Some(name) = descriptor_path.file_name() {
            return format!("mod/{}", name.to_string_lossy());
        }
    }

    format!(
        "mod/{}",
        generated_descriptor_file_name(descriptor_path, remote_file_id)
    )
}

/// Returns the generated descriptor filename used inside the game user mod directory.
pub fn generated_descriptor_file_name(descriptor_path: &Path, remote_file_id: &str) -> String {
    if !remote_file_id.trim().is_empty() {
        return format!("ugc_{remote_file_id}.mod");
    }

    let mut name = descriptor_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "rchadow_mod".to_owned());

    if name.eq_ignore_ascii_case("descriptor") {
        name = descriptor_path
            .parent()
            .and_then(Path::file_name)
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or(name);
    }

    let stem = safe_file_stem(if name.trim().is_empty() {
        "rchadow_mod"
    } else {
        &name
    });
    let hash = hash_hex(&descriptor_path.to_string_lossy());
    format!("{}_{hash}.mod", stem, hash = &hash[..8])
}

/// Escapes a Clausewitz string value.
pub fn escape_clausewitz_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Returns a path relative to `base` using forward slashes.
pub fn relative_launcher_path(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .map(to_forward_slash_path)
        .unwrap_or_else(|_| to_forward_slash_path(path))
}

fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed[1..trimmed.len() - 1]
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    } else {
        trimmed.to_owned()
    }
}

fn fallback_title(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Unknown Mod".to_owned())
}
