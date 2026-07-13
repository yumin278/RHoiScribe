//------------------------------------------------------------------------------------
// state_maintenance.rs -- Part of RHoiScribe
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

use serde::{Deserialize, Serialize};

use crate::state::{
    clean_display_path,
    maintenance::{self, StateBackupReport, StateInspectionReport},
};

pub(crate) const INSPECT_STATE_TOOL: &str = "inspect_rhoiscribe_state";
pub(crate) const BACKUP_STATE_TOOL: &str = "backup_rhoiscribe_state";

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InspectRhoiscribeStateRequest {
    pub store_path: Option<String>,
    #[serde(default)]
    pub deep_verify: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InspectRhoiscribeStateResult {
    pub database_path: String,
    pub read_only: bool,
    pub schema_version: u32,
    pub rnmdb_revision: String,
    pub format_version: u16,
    pub page_size_bytes: usize,
    pub file_len_bytes: u64,
    pub present_page_records: u64,
    pub superblock_generation: u64,
    pub catalog_root: u64,
    pub deep_verification_performed: bool,
    pub verification_valid: bool,
    pub encryption_authenticated: bool,
    pub authenticated_page_records: u64,
    pub last_migration_source_path: Option<String>,
    pub last_migration_backup_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BackupRhoiscribeStateRequest {
    pub store_path: Option<String>,
    pub destination: String,
    #[serde(default)]
    pub apply: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupRhoiscribeStateResult {
    pub applied: bool,
    pub dry_run: bool,
    pub source_path: String,
    pub destination_path: String,
    pub planned_bytes: u64,
    pub bytes_copied: Option<u64>,
    pub page_size_bytes: usize,
    pub present_page_records: u64,
    pub superblock_generation: u64,
    pub verification_valid: bool,
    pub encryption_authenticated: bool,
    pub overwrite_allowed: bool,
}

pub(crate) fn inspect_rhoiscribe_state(
    request: InspectRhoiscribeStateRequest,
) -> Result<InspectRhoiscribeStateResult, String> {
    maintenance::inspect_state(request.store_path.as_deref(), request.deep_verify)
        .map(inspection_result)
}

pub(crate) fn backup_rhoiscribe_state(
    request: BackupRhoiscribeStateRequest,
) -> Result<BackupRhoiscribeStateResult, String> {
    maintenance::backup_state(
        request.store_path.as_deref(),
        &request.destination,
        request.apply,
    )
    .map(backup_result)
}

pub(crate) fn is_state_maintenance_tool(name: &str) -> bool {
    matches!(name, INSPECT_STATE_TOOL | BACKUP_STATE_TOOL)
}

fn inspection_result(report: StateInspectionReport) -> InspectRhoiscribeStateResult {
    InspectRhoiscribeStateResult {
        database_path: clean_display_path(&report.database_path),
        read_only: true,
        schema_version: report.schema_version,
        rnmdb_revision: report.rnmdb_revision,
        format_version: report.format_version,
        page_size_bytes: report.page_size_bytes,
        file_len_bytes: report.file_len_bytes,
        present_page_records: report.present_page_records,
        superblock_generation: report.superblock_generation,
        catalog_root: report.catalog_root,
        deep_verification_performed: report.deep_verification_performed,
        verification_valid: report.verification_valid,
        encryption_authenticated: report.encryption_authenticated,
        authenticated_page_records: report.authenticated_page_records,
        last_migration_source_path: report.last_migration_source_path,
        last_migration_backup_path: report.last_migration_backup_path,
    }
}

fn backup_result(report: StateBackupReport) -> BackupRhoiscribeStateResult {
    BackupRhoiscribeStateResult {
        applied: report.applied,
        dry_run: !report.applied,
        source_path: clean_display_path(&report.source_path),
        destination_path: clean_display_path(&report.destination_path),
        planned_bytes: report.planned_bytes,
        bytes_copied: report.bytes_copied,
        page_size_bytes: report.page_size_bytes,
        present_page_records: report.present_page_records,
        superblock_generation: report.superblock_generation,
        verification_valid: report.verification_valid,
        encryption_authenticated: report.encryption_authenticated,
        overwrite_allowed: false,
    }
}
