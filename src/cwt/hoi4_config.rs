//------------------------------------------------------------------------------------
// hoi4_config.rs -- Part of RHoiScribe
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Hoi4CwtConfigSource {
    pub(crate) github_owner: &'static str,
    pub(crate) github_repository: &'static str,
    pub(crate) revision: &'static str,
    pub(crate) license: &'static str,
    pub(crate) source_directory: &'static str,
    pub(crate) source_format: &'static str,
    pub(crate) runtime_storage: &'static str,
}

pub(crate) const HOI4_CWT_CONFIG: Hoi4CwtConfigSource = Hoi4CwtConfigSource {
    github_owner: "NS9927",
    github_repository: "cwtools-hoi4-config",
    revision: "584e57ad975bb9b2408851cf440d75d2e58b2860",
    license: "MIT",
    source_directory: "config",
    source_format: "github_git_archive",
    runtime_storage: "compiled GitHub archive bytes; decompressed into process memory only",
};

impl Hoi4CwtConfigSource {
    pub(crate) fn source_slug(&self) -> String {
        format!("{}/{}", self.github_owner, self.github_repository)
    }

    pub(crate) fn upstream_url(&self) -> String {
        format!("https://github.com/{}", self.source_slug())
    }

    pub(crate) fn git_url(&self) -> String {
        format!("{}.git", self.upstream_url())
    }

    pub(crate) fn archive_url(&self) -> String {
        format!("{}/archive/{}.zip", self.upstream_url(), self.revision)
    }

    pub(crate) fn embedded_source_id(&self) -> String {
        format!("embedded-github:{}@{}", self.source_slug(), self.revision)
    }

    pub(crate) fn virtual_source_prefix(&self) -> String {
        format!("github://{}/{}/", self.source_slug(), self.source_directory)
    }

    pub(crate) fn virtual_path(&self, relative_path: &str) -> String {
        format!(
            "{}{}",
            self.virtual_source_prefix(),
            relative_path.trim_start_matches('/')
        )
    }

    pub(crate) fn archive_source_relative_path<'a>(&self, path: &'a str) -> Option<&'a str> {
        let source_directory = self.source_directory.trim_matches('/');
        let direct_prefix = format!("{source_directory}/");
        if let Some(relative_path) = path.strip_prefix(&direct_prefix) {
            return Some(relative_path);
        }

        let nested_marker = format!("/{source_directory}/");
        path.split_once(&nested_marker)
            .map(|(_, relative_path)| relative_path)
    }
}
