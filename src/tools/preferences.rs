//------------------------------------------------------------------------------------
// preferences.rs -- Part of RHoiScribe
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
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::state::{
    GLOBAL_SCOPE_KEY, GLOBAL_SCOPE_KIND, RnmdbStateStore, StateMutationLock,
    StoredPreferenceRecord, clean_display_path, global_record_key, state_database_error,
    state_store_path,
};

const BACKEND_NAME: &str = "RNMDB single-file page store";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ListAgentPreferencesRequest {
    pub store_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetAgentPreferenceRequest {
    pub key: String,
    pub value: Value,
    pub store_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeleteAgentPreferenceRequest {
    pub key: String,
    pub store_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentPreferenceItem {
    pub key: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentPreferencesResult {
    pub store_path: String,
    pub backend: String,
    pub preferences: Vec<AgentPreferenceItem>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentPreferenceMutationResult {
    pub store_path: String,
    pub backend: String,
    pub key: String,
    pub removed: bool,
    pub value: Option<Value>,
    pub preferences: Vec<AgentPreferenceItem>,
    pub messages: Vec<String>,
}

pub fn list_agent_preferences(
    request: ListAgentPreferencesRequest,
) -> Result<AgentPreferencesResult, String> {
    let store_path = preference_store_path(request.store_path.as_deref());
    let _lock = acquire_store_lock(&store_path)?;
    let mut store = RnmdbStateStore::open(&store_path)?;
    let migration_message = take_migration_message(&mut store);
    let records = store.list_global_preferences()?;
    let preferences = preference_items(records, &store_path)?;
    Ok(AgentPreferencesResult {
        store_path: clean_display_path(&store_path),
        backend: BACKEND_NAME.to_string(),
        preferences,
        messages: result_messages(
            "preferences are shared across MCP clients and IDEs through .rhoiscribe",
            migration_message,
        ),
    })
}

pub fn set_agent_preference(
    request: SetAgentPreferenceRequest,
) -> Result<AgentPreferenceMutationResult, String> {
    let key = normalized_preference_key(&request.key)?;
    let store_path = preference_store_path(request.store_path.as_deref());
    let _lock = acquire_store_lock(&store_path)?;
    let mut store = RnmdbStateStore::open(&store_path)?;
    let migration_message = take_migration_message(&mut store);
    let record = stored_preference(&key, &request.value, &store_path)?;
    store.upsert_preference(&record)?;
    let preferences = preference_items(store.list_global_preferences()?, &store_path)?;
    Ok(AgentPreferenceMutationResult {
        store_path: clean_display_path(&store_path),
        backend: BACKEND_NAME.to_string(),
        key,
        removed: false,
        value: Some(request.value),
        preferences,
        messages: result_messages(
            "preference stored in RNMDB-backed .rhoiscribe state",
            migration_message,
        ),
    })
}

pub fn delete_agent_preference(
    request: DeleteAgentPreferenceRequest,
) -> Result<AgentPreferenceMutationResult, String> {
    let key = normalized_preference_key(&request.key)?;
    let store_path = preference_store_path(request.store_path.as_deref());
    let _lock = acquire_store_lock(&store_path)?;
    let mut store = RnmdbStateStore::open(&store_path)?;
    let migration_message = take_migration_message(&mut store);
    let records = store.list_global_preferences()?;
    let value = preference_value(&records, &key, &store_path)?;
    let removed = store.delete_preference(&global_record_key(&key))?;
    let preferences = preference_items(store.list_global_preferences()?, &store_path)?;
    Ok(AgentPreferenceMutationResult {
        store_path: clean_display_path(&store_path),
        backend: BACKEND_NAME.to_string(),
        key,
        removed,
        value,
        preferences,
        messages: result_messages("preference state updated in RNMDB", migration_message),
    })
}

pub(crate) fn preference_store_path(store_path: Option<&str>) -> PathBuf {
    state_store_path(store_path)
}

pub(crate) fn is_state_database_error(error: &str) -> bool {
    crate::state::is_state_database_error(error)
}

fn acquire_store_lock(store_path: &std::path::Path) -> Result<StateMutationLock, String> {
    StateMutationLock::acquire(store_path)
        .map_err(|error| state_database_error(store_path, "open", error))
}

fn take_migration_message(store: &mut RnmdbStateStore) -> Option<String> {
    store
        .take_migration_report()
        .map(|report| report.retained_backup_message())
}

fn result_messages(primary: &str, migration: Option<String>) -> Vec<String> {
    let mut messages = vec![primary.to_string()];
    if let Some(message) = migration {
        messages.push(message);
    }
    messages
}

fn stored_preference(
    key: &str,
    value: &Value,
    store_path: &std::path::Path,
) -> Result<StoredPreferenceRecord, String> {
    let value_json = serde_json::to_string(value)
        .map_err(|error| state_database_error(store_path, "query", error.to_string()))?;
    Ok(StoredPreferenceRecord {
        record_key: global_record_key(key),
        scope_kind: GLOBAL_SCOPE_KIND.to_string(),
        scope_key: GLOBAL_SCOPE_KEY.to_string(),
        mod_root: None,
        preference_key: key.to_string(),
        value_json,
        updated_at_unix_seconds: unix_timestamp_now(),
    })
}

fn preference_items(
    records: Vec<StoredPreferenceRecord>,
    store_path: &std::path::Path,
) -> Result<Vec<AgentPreferenceItem>, String> {
    records
        .into_iter()
        .map(|record| preference_item(record, store_path))
        .collect()
}

fn preference_item(
    record: StoredPreferenceRecord,
    store_path: &std::path::Path,
) -> Result<AgentPreferenceItem, String> {
    let value = decode_preference_value(&record.value_json, store_path)?;
    Ok(AgentPreferenceItem {
        key: record.preference_key,
        value,
    })
}

fn preference_value(
    records: &[StoredPreferenceRecord],
    key: &str,
    store_path: &std::path::Path,
) -> Result<Option<Value>, String> {
    records
        .iter()
        .find(|record| record.preference_key == key)
        .map(|record| decode_preference_value(&record.value_json, store_path))
        .transpose()
}

fn decode_preference_value(value: &str, store_path: &std::path::Path) -> Result<Value, String> {
    serde_json::from_str(value)
        .map_err(|error| state_database_error(store_path, "query", error.to_string()))
}

fn normalized_preference_key(key: &str) -> Result<String, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("preference key is required".to_string());
    }
    if !key
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.'))
    {
        return Err(
            "preference key may contain only ASCII letters, digits, underscore, dash, or dot"
                .to_string(),
        );
    }
    Ok(key.to_ascii_lowercase())
}

fn unix_timestamp_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
