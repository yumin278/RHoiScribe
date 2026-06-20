//------------------------------------------------------------------------------------
// project_effective_files.rs -- Part of RHoiScribe
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

use std::{collections::HashMap, fs};

use super::paradox_lexer::{TokenKind, tokenize};
use super::project_files::ProjectFile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EffectiveProjectFiles {
    pub(crate) files: Vec<ProjectFile>,
    pub(crate) hidden_by_replace_path: usize,
    pub(crate) shadowed_by_logical_path: usize,
}

pub(crate) fn effective_project_files(files: Vec<ProjectFile>) -> EffectiveProjectFiles {
    let root_order = root_order(&files);
    let replace_paths = collect_replace_paths(&files);
    let mut descriptors = Vec::new();
    let mut candidates = HashMap::<String, ProjectFile>::new();
    let mut hidden_by_replace_path = 0usize;
    let mut shadowed_by_logical_path = 0usize;

    for file in files {
        if is_mod_descriptor_path(&file.relative_path) {
            descriptors.push(file);
            continue;
        }

        if hidden_by_replace_path_rule(&file, &replace_paths) {
            hidden_by_replace_path += 1;
            continue;
        }

        let logical_path = logical_path_key(&file.relative_path);
        if insert_effective_file(&root_order, &mut candidates, logical_path, file) {
            shadowed_by_logical_path += 1;
        }
    }

    let mut files = descriptors;
    files.extend(candidates.into_values());
    files.sort_by(|left, right| {
        (&left.root, &left.relative_path).cmp(&(&right.root, &right.relative_path))
    });

    EffectiveProjectFiles {
        files,
        hidden_by_replace_path,
        shadowed_by_logical_path,
    }
}

fn root_order(files: &[ProjectFile]) -> HashMap<String, usize> {
    let mut order = HashMap::new();
    for file in files {
        let next = order.len();
        order.entry(file.root.clone()).or_insert(next);
    }
    order
}

fn collect_replace_paths(files: &[ProjectFile]) -> Vec<String> {
    let mut replace_paths = Vec::new();

    for file in files {
        if !is_mod_like_root(file.root_role.as_deref())
            || !is_mod_descriptor_path(&file.relative_path)
        {
            continue;
        }

        let Ok(content) = fs::read_to_string(&file.absolute_path) else {
            continue;
        };
        replace_paths.extend(replace_paths_from_descriptor(&content));
    }

    replace_paths.sort();
    replace_paths.dedup();
    replace_paths
}

fn replace_paths_from_descriptor(content: &str) -> Vec<String> {
    let tokens = tokenize(content);
    let mut paths = Vec::new();
    let mut index = 0usize;

    while index + 2 < tokens.len() {
        if tokens[index].text == "replace_path"
            && tokens[index + 1].kind == TokenKind::Equals
            && matches!(tokens[index + 2].kind, TokenKind::Word | TokenKind::String)
        {
            let path = normalize_relative_path(&tokens[index + 2].text);
            if !path.is_empty() {
                paths.push(path.to_ascii_lowercase());
            }
            index += 3;
            continue;
        }
        index += 1;
    }

    paths
}

fn hidden_by_replace_path_rule(file: &ProjectFile, replace_paths: &[String]) -> bool {
    if is_mod_like_root(file.root_role.as_deref()) {
        return false;
    }

    let logical_path = logical_path_key(&file.relative_path);
    replace_paths
        .iter()
        .any(|replace_path| path_starts_with(&logical_path, replace_path))
}

fn insert_effective_file(
    root_order: &HashMap<String, usize>,
    candidates: &mut HashMap<String, ProjectFile>,
    logical_path: String,
    file: ProjectFile,
) -> bool {
    let Some(existing) = candidates.get(&logical_path) else {
        candidates.insert(logical_path, file);
        return false;
    };

    let shadowed =
        effective_precedence(&file, root_order) > effective_precedence(existing, root_order);
    if shadowed {
        candidates.insert(logical_path, file);
    }
    true
}

fn effective_precedence(file: &ProjectFile, root_order: &HashMap<String, usize>) -> (u8, usize) {
    (
        root_role_rank(file.root_role.as_deref()),
        root_order.get(&file.root).copied().unwrap_or(usize::MAX),
    )
}

fn root_role_rank(role: Option<&str>) -> u8 {
    match role.map(str::to_ascii_lowercase).as_deref() {
        Some("game") => 0,
        Some("dlc") => 10,
        _ => 20,
    }
}

fn is_mod_like_root(role: Option<&str>) -> bool {
    !matches!(
        role.map(str::to_ascii_lowercase).as_deref(),
        Some("game" | "dlc")
    )
}

fn is_mod_descriptor_path(path: &str) -> bool {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    file_name == "descriptor.mod" || file_name.ends_with(".mod")
}

fn logical_path_key(path: &str) -> String {
    normalize_relative_path(path).to_ascii_lowercase()
}

fn normalize_relative_path(path: &str) -> String {
    path.replace('\\', "/").trim().trim_matches('/').to_string()
}

fn path_starts_with(path: &str, prefix: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}
