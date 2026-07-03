//! Shared path helpers.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::{Error, Result};

/// Reads a UTF-8 text file with path-aware error reporting.
pub fn read_to_string(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Writes a UTF-8 text file after creating its parent directory.
pub fn write_string(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }

    fs::write(path, contents).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Writes JSON using a same-directory temporary file before replacement.
pub fn write_json_pretty<T>(path: &Path, value: &T) -> Result<()>
where
    T: Serialize,
{
    let contents = serde_json::to_string_pretty(value).map_err(|source| Error::Json {
        path: path.to_path_buf(),
        source,
    })?;
    write_string_atomic(path, &(contents + "\n"))
}

/// Writes a UTF-8 text file through a same-directory temporary path.
pub fn write_string_atomic(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }

    let temporary_path = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
    fs::write(&temporary_path, contents).map_err(|source| Error::Io {
        path: temporary_path.clone(),
        source,
    })?;

    if path.exists() {
        fs::remove_file(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }

    fs::rename(&temporary_path, path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Creates a directory tree with path-aware error reporting.
pub fn create_dir_all(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Converts a path to a launcher-friendly forward-slash string.
pub fn to_forward_slash_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            Component::CurDir => None,
            other => Some(other.as_os_str().to_string_lossy().into_owned()),
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Returns true when `path` is inside `directory`.
pub fn is_inside_directory(path: &Path, directory: &Path) -> bool {
    let full_path = absolute_path(path);
    let full_directory = absolute_path(directory);
    full_path.starts_with(full_directory)
}

/// Builds a deterministic bounded filename from untrusted input.
pub fn safe_file_stem(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if is_forbidden_file_name_character(character) {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    let trimmed = sanitized.trim();

    let stem = if trimmed.is_empty() || matches!(trimmed, "." | "..") {
        hash_hex(value)
    } else {
        trimmed.to_owned()
    };

    if stem.chars().count() <= 120 {
        return stem;
    }

    let prefix = stem.chars().take(80).collect::<String>();
    format!("{}_{}", prefix, &hash_hex(value)[..12])
}

/// Resolves a possibly relative descriptor content path.
pub fn resolve_content_path(
    descriptor_path: &Path,
    content_path: &str,
    game_user_directory: Option<&Path>,
) -> PathBuf {
    let trimmed = content_path.trim();
    if trimmed.is_empty() {
        return PathBuf::new();
    }

    let content = PathBuf::from(trimmed);
    if content.is_absolute() {
        return content;
    }

    if let Some(user_directory) = game_user_directory {
        let candidate = user_directory.join(&content);
        if candidate.exists() {
            return candidate;
        }
    }

    descriptor_path
        .parent()
        .map(|parent| parent.join(&content))
        .unwrap_or(content)
}

/// Returns a lowercase hex SHA-256 digest.
pub fn hash_hex(value: &str) -> String {
    use sha2::{Digest, Sha256};

    hex::encode(Sha256::digest(value.as_bytes()))
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

fn is_forbidden_file_name_character(character: char) -> bool {
    matches!(
        character,
        '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0'
    ) || character.is_control()
}
