//------------------------------------------------------------------------------------
// service.rs -- Part of RHoiScribe
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
    collections::HashMap,
    error::Error,
    fmt,
    sync::{Arc, RwLock},
};

use super::workspace::{
    CwtWorkspaceConfig, CwtWorkspaceError, CwtWorkspaceHandle, CwtWorkspaceStatus,
    workspace_handle_id,
};

#[derive(Default)]
pub struct CwtLanguageService {
    workspaces: RwLock<HashMap<String, Arc<CwtWorkspaceHandle>>>,
}

#[derive(Debug)]
pub enum CwtLanguageServiceError {
    RegistryLockPoisoned,
    Workspace(CwtWorkspaceError),
}

impl CwtLanguageService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_workspace(
        &self,
        config: CwtWorkspaceConfig,
    ) -> Result<Arc<CwtWorkspaceHandle>, CwtLanguageServiceError> {
        let id = workspace_handle_id(&config);
        let handle = {
            let mut workspaces = self
                .workspaces
                .write()
                .map_err(|_| CwtLanguageServiceError::RegistryLockPoisoned)?;
            workspaces
                .entry(id.clone())
                .or_insert_with(|| Arc::new(CwtWorkspaceHandle::new(id, config)))
                .clone()
        };
        handle
            .refresh()
            .map_err(CwtLanguageServiceError::Workspace)?;
        Ok(handle)
    }

    pub fn get_workspace(
        &self,
        handle_id: &str,
    ) -> Result<Option<Arc<CwtWorkspaceHandle>>, CwtLanguageServiceError> {
        let workspaces = self
            .workspaces
            .read()
            .map_err(|_| CwtLanguageServiceError::RegistryLockPoisoned)?;
        Ok(workspaces.get(handle_id).cloned())
    }

    pub fn list_workspace_statuses(
        &self,
    ) -> Result<Vec<CwtWorkspaceStatus>, CwtLanguageServiceError> {
        let workspaces = self
            .workspaces
            .read()
            .map_err(|_| CwtLanguageServiceError::RegistryLockPoisoned)?;
        workspaces
            .values()
            .map(|handle| handle.status().map_err(CwtLanguageServiceError::Workspace))
            .collect()
    }

    pub fn clear(&self) -> Result<usize, CwtLanguageServiceError> {
        let mut workspaces = self
            .workspaces
            .write()
            .map_err(|_| CwtLanguageServiceError::RegistryLockPoisoned)?;
        let cleared = workspaces.len();
        workspaces.clear();
        Ok(cleared)
    }
}

impl fmt::Debug for CwtLanguageService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CwtLanguageService")
            .field("workspaces", &"RwLock<HashMap<String, CwtWorkspaceHandle>>")
            .finish()
    }
}

impl fmt::Display for CwtLanguageServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CwtLanguageServiceError::RegistryLockPoisoned => {
                write!(
                    formatter,
                    "CWT language workspace registry lock is poisoned"
                )
            }
            CwtLanguageServiceError::Workspace(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for CwtLanguageServiceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CwtLanguageServiceError::Workspace(error) => Some(error),
            _ => None,
        }
    }
}
