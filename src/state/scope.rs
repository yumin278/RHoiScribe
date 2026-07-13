//------------------------------------------------------------------------------------
// state/scope.rs -- Part of RHoiScribe
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
    fs,
    path::{Path, PathBuf},
};

use super::{GLOBAL_SCOPE_KEY, GLOBAL_SCOPE_KIND, MOD_SCOPE_KIND};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StateScope {
    Global,
    Mod { root: PathBuf, key: String },
}

impl StateScope {
    pub(crate) fn from_mod_root(mod_root: Option<&str>) -> Result<Self, String> {
        match mod_root {
            Some(input) => resolve_mod_scope(input),
            None => Ok(Self::Global),
        }
    }

    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Global => GLOBAL_SCOPE_KIND,
            Self::Mod { .. } => MOD_SCOPE_KIND,
        }
    }

    pub(crate) fn key(&self) -> &str {
        match self {
            Self::Global => GLOBAL_SCOPE_KEY,
            Self::Mod { key, .. } => key,
        }
    }

    pub(crate) fn mod_root_text(&self) -> Option<String> {
        match self {
            Self::Global => None,
            Self::Mod { root, .. } => Some(root.to_string_lossy().into_owned()),
        }
    }
}

pub(crate) fn validate_stored_scope(
    scope_kind: &str,
    scope_key: &str,
    mod_root: Option<&str>,
) -> Result<(), String> {
    match scope_kind {
        GLOBAL_SCOPE_KIND => validate_stored_global(scope_key, mod_root),
        MOD_SCOPE_KIND => validate_stored_mod(scope_key, mod_root),
        _ => Err(format!("unknown preference scope kind `{scope_kind}`")),
    }
}

fn resolve_mod_scope(input: &str) -> Result<StateScope, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(format!("mod_root `{input}` must not be empty"));
    }
    let canonical = fs::canonicalize(trimmed)
        .map_err(|error| format!("mod_root `{input}` could not be canonicalized: {error}"))?;
    if !canonical.is_dir() {
        return Err(format!(
            "mod_root `{input}` must resolve to an existing directory"
        ));
    }
    let normalized = normalized_root(&canonical, input)?;
    Ok(StateScope::Mod {
        root: PathBuf::from(&normalized),
        key: scope_identity(&normalized),
    })
}

fn validate_stored_global(scope_key: &str, mod_root: Option<&str>) -> Result<(), String> {
    if scope_key != GLOBAL_SCOPE_KEY {
        return Err("global preference scope key is invalid".to_string());
    }
    if mod_root.is_some() {
        return Err("global preference must not contain mod_root".to_string());
    }
    Ok(())
}

fn validate_stored_mod(scope_key: &str, mod_root: Option<&str>) -> Result<(), String> {
    let root = mod_root
        .filter(|root| !root.is_empty())
        .ok_or_else(|| "mod preference must contain mod_root".to_string())?;
    if scope_key != scope_identity(root) {
        return Err("mod preference scope key does not match mod_root".to_string());
    }
    Ok(())
}

fn normalized_root(path: &Path, input: &str) -> Result<String, String> {
    let value = path
        .to_str()
        .ok_or_else(|| format!("mod_root `{input}` resolves to a non-Unicode path"))?;
    Ok(normalized_platform_root(value))
}

#[cfg(windows)]
fn normalized_platform_root(value: &str) -> String {
    let readable = if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = value
        .strip_prefix(r"\\?\")
        .filter(|rest| has_drive_prefix(rest))
    {
        rest.to_string()
    } else {
        value.to_string()
    };
    readable.replace('\\', "/")
}

#[cfg(windows)]
fn has_drive_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[cfg(not(windows))]
fn normalized_platform_root(value: &str) -> String {
    value.to_string()
}

#[cfg(windows)]
fn scope_identity(root: &str) -> String {
    root.to_lowercase()
}

#[cfg(not(windows))]
fn scope_identity(root: &str) -> String {
    root.to_string()
}
