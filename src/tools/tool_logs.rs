//------------------------------------------------------------------------------------
// tool_logs.rs -- Part of RHoiScribe
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
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::state::{
    GLOBAL_SCOPE_KEY, GLOBAL_SCOPE_KIND, RnmdbStateStore, StateMutationLock, StoredToolLogRecord,
    clean_display_path, state_database_error,
};

use super::preferences::preference_store_path;

pub(crate) const MAX_TOOL_LOG_ENTRIES: usize = 32767;

const DEFAULT_TOOL_LOG_LIMIT: usize = 100;
const MAX_LOGGED_JSON_CHARS: usize = 8192;
const BACKEND_NAME: &str = "RNMDB single-file page store";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolLogEntry {
    pub sequence: u64,
    pub timestamp_unix_seconds: u64,
    pub tool_name: String,
    pub arguments: Value,
    pub success: bool,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ToolLogQueryRequest {
    pub store_path: Option<String>,
    pub pattern: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolLogExportRequest {
    pub store_path: Option<String>,
    pub output_path: String,
    pub pattern: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolLogQueryResult {
    pub store_path: String,
    pub backend: String,
    pub total_entries: usize,
    pub matched_entries: usize,
    pub entries: Vec<ToolLogEntry>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolLogExportResult {
    pub store_path: String,
    pub output_path: String,
    pub exported_entries: usize,
    pub messages: Vec<String>,
}

pub(crate) struct ToolLogAppend {
    pub(crate) tool_name: String,
    pub(crate) arguments: Value,
    pub(crate) success: bool,
    pub(crate) result: Option<Value>,
    pub(crate) error: Option<String>,
}

pub(crate) fn tool_log_store_path_from_arguments(arguments: &Map<String, Value>) -> Option<String> {
    arguments
        .get("store_path")
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub(crate) fn record_tool_call(
    store_path: Option<&str>,
    append: ToolLogAppend,
) -> Result<Option<String>, String> {
    let store_path = preference_store_path(store_path);
    let _lock = acquire_store_lock(&store_path)?;
    let record = stored_log_record(append, &store_path)?;
    let mut store = RnmdbStateStore::open(&store_path)?;
    store.append_log(record, MAX_TOOL_LOG_ENTRIES)?;
    Ok(store
        .take_migration_report()
        .map(|report| report.retained_backup_message()))
}

pub fn query_tool_logs(request: ToolLogQueryRequest) -> Result<ToolLogQueryResult, String> {
    let store_path = preference_store_path(request.store_path.as_deref());
    let (entries, migration_message) = read_tool_log_entries(&store_path)?;
    let matcher = compile_log_regex(request.pattern.as_deref())?;
    let limit = query_limit(request.limit, DEFAULT_TOOL_LOG_LIMIT);
    let total_entries = entries.len();
    let (matched_entries, entries) = filter_log_entries(&entries, matcher.as_ref(), limit)?;
    let mut messages = vec![
        "tool logs share the same RNMDB database as agent preferences".to_string(),
        format!("only the latest {MAX_TOOL_LOG_ENTRIES} entries are retained"),
    ];
    append_migration_message(&mut messages, migration_message);
    Ok(ToolLogQueryResult {
        store_path: clean_display_path(&store_path),
        backend: BACKEND_NAME.to_string(),
        total_entries,
        matched_entries,
        entries,
        messages,
    })
}

pub fn export_tool_logs(request: ToolLogExportRequest) -> Result<ToolLogExportResult, String> {
    let store_path = preference_store_path(request.store_path.as_deref());
    let limit = query_limit(request.limit, MAX_TOOL_LOG_ENTRIES);
    let (entries, migration_message) =
        filtered_tool_log_entries(&store_path, request.pattern.as_deref(), limit)?;
    let output_path = Path::new(&request.output_path);
    write_tool_log_export(output_path, tool_log_export_payload(&store_path, &entries))?;
    let mut messages = vec!["matching tool logs exported as JSON".to_string()];
    append_migration_message(&mut messages, migration_message);
    Ok(ToolLogExportResult {
        store_path: clean_display_path(&store_path),
        output_path: clean_display_path(output_path),
        exported_entries: entries.len(),
        messages,
    })
}

fn filtered_tool_log_entries(
    store_path: &Path,
    pattern: Option<&str>,
    limit: usize,
) -> Result<(Vec<ToolLogEntry>, Option<String>), String> {
    let (entries, migration_message) = read_tool_log_entries(store_path)?;
    let matcher = compile_log_regex(pattern)?;
    let (_, entries) = filter_log_entries(&entries, matcher.as_ref(), limit)?;
    Ok((entries, migration_message))
}

fn read_tool_log_entries(store_path: &Path) -> Result<(Vec<ToolLogEntry>, Option<String>), String> {
    let _lock = acquire_store_lock(store_path)?;
    let mut store = RnmdbStateStore::open(store_path)?;
    let migration_message = store
        .take_migration_report()
        .map(|report| report.retained_backup_message());
    let entries = store
        .list_global_logs()?
        .into_iter()
        .map(|record| tool_log_entry(record, store_path))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((entries, migration_message))
}

fn append_migration_message(messages: &mut Vec<String>, migration: Option<String>) {
    if let Some(message) = migration {
        messages.push(message);
    }
}

fn acquire_store_lock(store_path: &Path) -> Result<StateMutationLock, String> {
    StateMutationLock::acquire(store_path)
        .map_err(|error| state_database_error(store_path, "open", error))
}

fn stored_log_record(
    append: ToolLogAppend,
    store_path: &Path,
) -> Result<StoredToolLogRecord, String> {
    let arguments = compact_json_value(append.arguments);
    let result = append.result.map(compact_json_value);
    Ok(StoredToolLogRecord {
        sequence: 0,
        timestamp_unix_seconds: unix_timestamp_now(),
        scope_kind: GLOBAL_SCOPE_KIND.to_string(),
        scope_key: GLOBAL_SCOPE_KEY.to_string(),
        mod_root: None,
        tool_name: append.tool_name,
        arguments_json: encode_json(&arguments, store_path)?,
        success: append.success,
        result_json: result
            .as_ref()
            .map(|value| encode_json(value, store_path))
            .transpose()?,
        error_text: append.error,
    })
}

fn tool_log_entry(record: StoredToolLogRecord, store_path: &Path) -> Result<ToolLogEntry, String> {
    Ok(ToolLogEntry {
        sequence: record.sequence,
        timestamp_unix_seconds: record.timestamp_unix_seconds,
        tool_name: record.tool_name,
        arguments: decode_json(&record.arguments_json, store_path)?,
        success: record.success,
        result: record
            .result_json
            .as_deref()
            .map(|value| decode_json(value, store_path))
            .transpose()?,
        error: record.error_text,
    })
}

fn encode_json(value: &Value, store_path: &Path) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|error| state_database_error(store_path, "query", error.to_string()))
}

fn decode_json(value: &str, store_path: &Path) -> Result<Value, String> {
    serde_json::from_str(value)
        .map_err(|error| state_database_error(store_path, "query", error.to_string()))
}

fn write_tool_log_export(output_path: &Path, payload: Value) -> Result<(), String> {
    create_output_parent(output_path)?;
    fs::write(
        output_path,
        serde_json::to_vec_pretty(&payload).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn create_output_parent(output_path: &Path) -> Result<(), String> {
    output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(fs::create_dir_all)
        .transpose()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn tool_log_export_payload(store_path: &Path, entries: &[ToolLogEntry]) -> Value {
    json!({
        "store_path": clean_display_path(store_path),
        "backend": BACKEND_NAME,
        "exported_entries": entries.len(),
        "entries": entries,
    })
}

fn compile_log_regex(pattern: Option<&str>) -> Result<Option<Regex>, String> {
    pattern
        .filter(|pattern| !pattern.trim().is_empty())
        .map(Regex::new)
        .transpose()
        .map_err(|error| error.to_string())
}

fn filter_log_entries(
    entries: &[ToolLogEntry],
    matcher: Option<&Regex>,
    limit: usize,
) -> Result<(usize, Vec<ToolLogEntry>), String> {
    let mut matched_entries = 0;
    let mut filtered = Vec::new();
    for entry in entries.iter().rev() {
        if !matches_log_entry(entry, matcher)? {
            continue;
        }
        matched_entries += 1;
        if filtered.len() < limit {
            filtered.push(entry.clone());
        }
    }
    Ok((matched_entries, filtered))
}

fn matches_log_entry(entry: &ToolLogEntry, matcher: Option<&Regex>) -> Result<bool, String> {
    let Some(matcher) = matcher else {
        return Ok(true);
    };
    let text = serde_json::to_string(entry).map_err(|error| error.to_string())?;
    Ok(matcher.is_match(&text))
}

fn query_limit(limit: Option<usize>, default_limit: usize) -> usize {
    limit
        .unwrap_or(default_limit)
        .clamp(1, MAX_TOOL_LOG_ENTRIES)
}

fn unix_timestamp_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn compact_json_value(value: Value) -> Value {
    let Ok(encoded) = serde_json::to_string(&value) else {
        return value;
    };
    if encoded.chars().count() <= MAX_LOGGED_JSON_CHARS {
        return value;
    }
    let preview = encoded
        .chars()
        .take(MAX_LOGGED_JSON_CHARS)
        .collect::<String>();
    json!({
        "truncated": true,
        "original_json_chars": encoded.chars().count(),
        "preview": preview,
    })
}
