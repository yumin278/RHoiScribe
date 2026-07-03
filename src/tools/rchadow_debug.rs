//------------------------------------------------------------------------------------
// rchadow_debug.rs -- Part of RHoiScribe
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
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use rchadow::{
    games::hoi4::{ApplyPlaysetOptions, Hoi4Paths, Hoi4PlaysetManager, ModEntry},
    playsets::{ModIndex, ModIndexEntry, Playset, PlaysetDatabase},
    rnmdb::RnmdbDiskPlaysetDatabase,
};
use serde::{Deserialize, Serialize};

use super::environment::{Hoi4DebugRunRequest, Hoi4QualityCheck, validate_hoi4_debug_run};
use super::rnmdb_store::{
    DEFAULT_RNMDB_PAGE_SIZE_BYTES, RnmdbSingleFilePageStore, clean_display_path,
    default_rhoiscribe_dir,
};

const DEBUG_ARGUMENTS: &str = "-gdpr-compliant -debug_mode";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RchadowDebugLaunchRequest {
    pub game_path: String,
    pub document_path: String,
    pub workspace_mod_path: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub launch: Option<bool>,
    pub apply_playset: Option<bool>,
    pub mode: Option<String>,
    pub store_path: Option<String>,
    pub playset_name: Option<String>,
    pub workshop_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RchadowDebugLaunchResult {
    pub ready: bool,
    pub launched: bool,
    pub applied_playset: bool,
    pub mode: String,
    pub store_path: Option<String>,
    pub exe_args: Vec<String>,
    pub enabled_mods: Vec<String>,
    pub missing_mods: Vec<String>,
    pub dlc_load_path: Option<String>,
    pub checks: Vec<Hoi4QualityCheck>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DebugPersistenceMode {
    Memory,
    Disk,
}

struct DebugLaunchInputs {
    game_path: PathBuf,
    document_path: PathBuf,
    workspace_mod_path: PathBuf,
    mode: DebugPersistenceMode,
    launch: bool,
    apply_playset: bool,
    store_path: Option<PathBuf>,
}

struct DebugExecution {
    launched: bool,
    applied_playset: bool,
    enabled_mods: Vec<String>,
    dlc_load_path: Option<String>,
}

pub fn launch_hoi4_debug_with_rchadow(
    request: RchadowDebugLaunchRequest,
) -> Result<RchadowDebugLaunchResult, String> {
    let inputs = DebugLaunchInputs::from_request(&request);
    let mut messages = vec![format!(
        "Rchadow debug mode selected: {}",
        inputs.mode.as_str()
    )];
    let preflight = run_preflight(&request, &inputs);

    if !base_preflight_ready(&preflight.checks) {
        messages
            .push("Rchadow launch skipped because required preflight checks are red".to_string());
        return Ok(result_without_rchadow(
            inputs.mode,
            inputs.store_path,
            preflight.checks,
            messages,
        ));
    }

    let manager = playset_manager(&request, &inputs);
    let mods = manager
        .discover_mods()
        .map_err(|error| format!("Rchadow failed to discover HOI4 mods: {}", error))?;
    let expected_mod_names = expected_mod_names(&inputs.workspace_mod_path, &request.dependencies);
    let selected_mods = selected_mods(&mods, &inputs.workspace_mod_path, &expected_mod_names);
    let missing_mods = missing_mod_names(&expected_mod_names, &selected_mods);
    let ready = missing_mods.is_empty();
    let execution = execute_debug_workflow(
        &request,
        &inputs,
        &manager,
        &mods,
        &selected_mods,
        &missing_mods,
        &mut messages,
    )?;

    Ok(debug_launch_result(
        inputs,
        preflight.checks,
        execution,
        ready,
        missing_mods,
        messages,
    ))
}

fn debug_launch_result(
    inputs: DebugLaunchInputs,
    checks: Vec<Hoi4QualityCheck>,
    execution: DebugExecution,
    ready: bool,
    missing_mods: Vec<String>,
    messages: Vec<String>,
) -> RchadowDebugLaunchResult {
    RchadowDebugLaunchResult {
        ready,
        launched: execution.launched,
        applied_playset: execution.applied_playset,
        mode: inputs.mode.as_str().to_string(),
        store_path: inputs.store_path.map(|path| clean_display_path(&path)),
        exe_args: DEBUG_ARGUMENTS
            .split_whitespace()
            .map(ToString::to_string)
            .collect(),
        enabled_mods: execution.enabled_mods,
        missing_mods,
        dlc_load_path: execution.dlc_load_path,
        checks,
        messages,
    }
}

impl DebugLaunchInputs {
    fn from_request(request: &RchadowDebugLaunchRequest) -> Self {
        let mode = choose_mode(request);
        Self {
            game_path: PathBuf::from(clean_input_path(&request.game_path)),
            document_path: PathBuf::from(clean_input_path(&request.document_path)),
            workspace_mod_path: PathBuf::from(clean_input_path(&request.workspace_mod_path)),
            mode,
            launch: request.launch.unwrap_or(false),
            apply_playset: request.apply_playset.unwrap_or(true),
            store_path: store_path_for_mode(mode, request.store_path.as_deref()),
        }
    }
}

fn run_preflight(
    request: &RchadowDebugLaunchRequest,
    inputs: &DebugLaunchInputs,
) -> super::environment::Hoi4DebugRunResult {
    validate_hoi4_debug_run(Hoi4DebugRunRequest {
        game_path: clean_display_path(&inputs.game_path),
        document_path: clean_display_path(&inputs.document_path),
        workspace_mod_path: clean_display_path(&inputs.workspace_mod_path),
        dependencies: request.dependencies.clone(),
        launch: Some(false),
    })
}

fn playset_manager(
    request: &RchadowDebugLaunchRequest,
    inputs: &DebugLaunchInputs,
) -> Hoi4PlaysetManager {
    Hoi4PlaysetManager::new(Hoi4Paths {
        game_user_dir: inputs.document_path.clone(),
        workshop_dir: request
            .workshop_path
            .as_deref()
            .map(clean_input_path)
            .map(PathBuf::from),
        game_executable: Some(inputs.game_path.join("hoi4.exe")),
    })
}

fn execute_debug_workflow(
    request: &RchadowDebugLaunchRequest,
    inputs: &DebugLaunchInputs,
    manager: &Hoi4PlaysetManager,
    mods: &[ModEntry],
    selected_mods: &[&ModEntry],
    missing_mods: &[String],
    messages: &mut Vec<String>,
) -> Result<DebugExecution, String> {
    let enabled_mods = selected_mods
        .iter()
        .map(|mod_entry| mod_entry.launcher_path.clone())
        .collect::<Vec<_>>();
    if !missing_mods.is_empty() {
        messages.push(format!(
            "Rchadow could not find required mod descriptor(s): {}",
            missing_mods.join(", ")
        ));
        return Ok(DebugExecution::from_enabled_mods(enabled_mods));
    }

    let mut playset = debug_playset(request, selected_mods);
    persist_playset_if_needed(
        inputs.mode,
        inputs.store_path.as_deref(),
        &mut playset,
        mods,
        messages,
    )?;
    apply_and_launch_debug(manager, inputs, &playset, mods, enabled_mods, messages)
}

fn apply_and_launch_debug(
    manager: &Hoi4PlaysetManager,
    inputs: &DebugLaunchInputs,
    playset: &Playset,
    mods: &[ModEntry],
    fallback_enabled_mods: Vec<String>,
    messages: &mut Vec<String>,
) -> Result<DebugExecution, String> {
    let mut execution = DebugExecution::from_enabled_mods(fallback_enabled_mods);
    if inputs.apply_playset {
        let applied = manager
            .apply_playset(playset, mods, ApplyPlaysetOptions::default())
            .map_err(|error| format!("Rchadow failed to apply HOI4 playset: {}", error))?;
        execution.enabled_mods = applied.enabled_mods;
        execution.dlc_load_path = Some(clean_display_path(&applied.dlc_load_path));
        execution.applied_playset = true;
    }

    if inputs.launch {
        manager
            .launch(DEBUG_ARGUMENTS)
            .map_err(|error| format!("Rchadow failed to launch HOI4: {}", error))?;
        execution.launched = true;
        messages.push("hoi4.exe launched through Rchadow".to_string());
    }
    Ok(execution)
}

impl DebugExecution {
    fn from_enabled_mods(enabled_mods: Vec<String>) -> Self {
        Self {
            launched: false,
            applied_playset: false,
            enabled_mods,
            dlc_load_path: None,
        }
    }
}

fn result_without_rchadow(
    mode: DebugPersistenceMode,
    store_path: Option<PathBuf>,
    checks: Vec<Hoi4QualityCheck>,
    messages: Vec<String>,
) -> RchadowDebugLaunchResult {
    RchadowDebugLaunchResult {
        ready: false,
        launched: false,
        applied_playset: false,
        mode: mode.as_str().to_string(),
        store_path: store_path.map(|path| clean_display_path(&path)),
        exe_args: DEBUG_ARGUMENTS
            .split_whitespace()
            .map(ToString::to_string)
            .collect(),
        enabled_mods: Vec::new(),
        missing_mods: Vec::new(),
        dlc_load_path: None,
        checks,
        messages,
    }
}

fn choose_mode(request: &RchadowDebugLaunchRequest) -> DebugPersistenceMode {
    match request
        .mode
        .as_deref()
        .map(|mode| mode.to_ascii_lowercase())
    {
        Some(mode) if matches!(mode.as_str(), "disk" | "persistent" | "project") => {
            DebugPersistenceMode::Disk
        }
        Some(mode) if matches!(mode.as_str(), "memory" | "temporary" | "temp") => {
            DebugPersistenceMode::Memory
        }
        _ if request.store_path.is_some() || request.launch != Some(true) => {
            DebugPersistenceMode::Disk
        }
        _ => DebugPersistenceMode::Memory,
    }
}

fn store_path_for_mode(mode: DebugPersistenceMode, requested: Option<&str>) -> Option<PathBuf> {
    if mode == DebugPersistenceMode::Memory {
        return None;
    }
    Some(
        requested
            .map(|path| PathBuf::from(clean_input_path(path)))
            .unwrap_or_else(|| default_rhoiscribe_dir().join("rchadow-debug-playsets.rnmdb")),
    )
}

fn base_preflight_ready(checks: &[Hoi4QualityCheck]) -> bool {
    checks.iter().all(|check| {
        check.status == "green" || matches!(check.name.as_str(), "playset_enabled_mods")
    })
}

fn expected_mod_names(workspace_mod_path: &Path, dependencies: &[String]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let descriptor_path = workspace_mod_path.join("descriptor.mod");
    if let Ok(content) = fs::read_to_string(descriptor_path) {
        if let Some(name) = descriptor_value(&content, "name") {
            names.insert(name);
        }
        names.extend(descriptor_dependencies(&content));
    }
    names.extend(
        dependencies
            .iter()
            .map(|dependency| dependency.trim())
            .filter(|dependency| !dependency.is_empty())
            .map(ToString::to_string),
    );
    names
}

fn selected_mods<'a>(
    mods: &'a [ModEntry],
    workspace_mod_path: &Path,
    expected_mod_names: &BTreeSet<String>,
) -> Vec<&'a ModEntry> {
    mods.iter()
        .filter(|mod_entry| {
            expected_mod_names
                .iter()
                .any(|expected| mod_entry.title.eq_ignore_ascii_case(expected))
                || paths_point_to_same_location(&mod_entry.content_path, workspace_mod_path)
        })
        .collect()
}

fn missing_mod_names(
    expected_mod_names: &BTreeSet<String>,
    selected_mods: &[&ModEntry],
) -> Vec<String> {
    expected_mod_names
        .iter()
        .filter(|name| {
            !selected_mods
                .iter()
                .any(|mod_entry| mod_entry.title.eq_ignore_ascii_case(name))
        })
        .cloned()
        .collect()
}

fn debug_playset(request: &RchadowDebugLaunchRequest, selected_mods: &[&ModEntry]) -> Playset {
    let mut playset = Playset::named(
        request
            .playset_name
            .as_deref()
            .unwrap_or("RHoiScribe Debug"),
    );
    playset.id = "rhoiscribe_debug".to_string();
    playset.source = "RHoiScribe".to_string();
    playset.enabled_mod_ids = selected_mods
        .iter()
        .map(|mod_entry| mod_entry.id.clone())
        .collect();
    playset.mod_ids = playset.enabled_mod_ids.clone();
    playset
}

fn persist_playset_if_needed(
    mode: DebugPersistenceMode,
    store_path: Option<&Path>,
    playset: &mut Playset,
    mods: &[ModEntry],
    messages: &mut Vec<String>,
) -> Result<(), String> {
    if mode == DebugPersistenceMode::Memory {
        messages.push("debug playset kept in memory for this run".to_string());
        return Ok(());
    }

    let path = store_path.ok_or_else(|| "disk mode requires an RNMDB store path".to_string())?;
    let store = RnmdbSingleFilePageStore::open_or_create(path, DEFAULT_RNMDB_PAGE_SIZE_BYTES)?;
    let mut database = RnmdbDiskPlaysetDatabase::new(store);
    database
        .save_mod_index(&ModIndex::new(mods.iter().map(mod_index_entry).collect()))
        .map_err(|error| error.to_string())?;
    database
        .save_playset(playset)
        .map_err(|error| error.to_string())?;
    messages.push(format!(
        "debug playset persisted through Rchadow RNMDB disk backend at {}",
        clean_display_path(path)
    ));
    Ok(())
}

fn mod_index_entry(mod_entry: &ModEntry) -> ModIndexEntry {
    ModIndexEntry {
        id: mod_entry.id.clone(),
        name: mod_entry.title.clone(),
        source: if mod_entry.is_steam_workshop_mod() {
            "steam_workshop".to_string()
        } else {
            "local".to_string()
        },
        remote_file_id: mod_entry.remote_file_id.clone(),
        descriptor_path: mod_entry.descriptor_path.clone(),
        launcher_path: mod_entry.launcher_path.clone(),
        content_path: mod_entry.content_path.clone(),
        version: mod_entry.version.clone(),
    }
}

impl DebugPersistenceMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Disk => "disk",
        }
    }
}

fn descriptor_value(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        let before_comment = line.split_once('#').map_or(line, |(before, _)| before);
        let Some((left, right)) = before_comment.split_once('=') else {
            continue;
        };
        if left.trim() != key {
            continue;
        }
        let value = right.trim().trim_matches('"').trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn descriptor_dependencies(content: &str) -> Vec<String> {
    let Some(start) = content.find("dependencies") else {
        return Vec::new();
    };
    let Some(open) = content[start..].find('{').map(|index| start + index) else {
        return Vec::new();
    };
    let Some(close) = content[open..].find('}').map(|index| open + index) else {
        return Vec::new();
    };
    quoted_strings(&content[open..=close])
}

fn quoted_strings(content: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut chars = content.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '"' {
            values.push(read_quoted_value(&mut chars));
        }
    }
    values
}

fn read_quoted_value<I>(chars: &mut I) -> String
where
    I: Iterator<Item = char>,
{
    let mut value = String::new();
    let mut escaped = false;
    for character in chars.by_ref() {
        if escaped {
            value.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            break;
        } else {
            value.push(character);
        }
    }
    value
}

fn paths_point_to_same_location(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => clean_display_path(left).eq_ignore_ascii_case(&clean_display_path(right)),
    }
}

fn clean_input_path(path: &str) -> String {
    path.trim().trim_matches('"').to_string()
}
