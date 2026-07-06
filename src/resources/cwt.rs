//------------------------------------------------------------------------------------
// cwt.rs -- Part of RHoiScribe
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

use std::fmt::Write as _;

use super::cwt_bundle;

pub const CWT_CATALOG_URI: &str = "rhoiscribe://hoi4/cwt/catalog";
pub const CWT_METADATA_URI: &str = "rhoiscribe://hoi4/cwt/metadata";
pub const CWT_SOURCE_URI_PREFIX: &str = "rhoiscribe://hoi4/cwt/source/";

const VIRTUAL_SOURCE_PREFIX: &str = "bundled://hoi4-cwt/config/";
const CWT_RESOURCE_SOURCE_FORMAT: &str = "cwtools-hoi4-config";
const CWT_RUNTIME_STORAGE: &str = "compiled Rust static strings; no runtime CWT disk state";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CwtResourceCatalog {
    catalog_index: String,
    metadata: String,
    source_count: usize,
    rule_source_count: usize,
    total_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hoi4CwtSource {
    pub path: &'static str,
    pub content: &'static str,
    pub mime_type: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CwtResourceEntry {
    pub(crate) uri: String,
    pub(crate) name: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) mime_type: &'static str,
    pub(crate) size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CwtResourceText {
    pub(crate) text: String,
    pub(crate) mime_type: &'static str,
}

impl CwtResourceCatalog {
    pub fn load_embedded() -> Self {
        let source_count = cwt_bundle::EMBEDDED_CWT_CONFIG_SOURCES.len();
        let rule_source_count = embedded_hoi4_cwt_sources()
            .filter(Hoi4CwtSource::is_rule_source)
            .count();
        let total_bytes = embedded_hoi4_cwt_sources()
            .map(|source| source.content.len())
            .sum();

        Self {
            catalog_index: catalog_index_toml(source_count, rule_source_count, total_bytes),
            metadata: metadata_markdown(source_count, rule_source_count, total_bytes),
            source_count,
            rule_source_count,
            total_bytes,
        }
    }

    pub fn source_count(&self) -> usize {
        self.source_count
    }

    pub fn rule_source_count(&self) -> usize {
        self.rule_source_count
    }

    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    pub(crate) fn resource_entries(&self) -> Vec<CwtResourceEntry> {
        let mut entries = Vec::with_capacity(self.source_count + 2);
        entries.push(CwtResourceEntry {
            uri: CWT_CATALOG_URI.to_string(),
            name: "hoi4_cwt_catalog".to_string(),
            title: "HOI4 CWT resource catalog".to_string(),
            description: "Structured index of bundled virtual HOI4 CWT config sources.".to_string(),
            mime_type: "application/toml",
            size: self.catalog_index.len(),
        });
        entries.push(CwtResourceEntry {
            uri: CWT_METADATA_URI.to_string(),
            name: "hoi4_cwt_metadata".to_string(),
            title: "HOI4 CWT snapshot metadata".to_string(),
            description: "Traceability and runtime storage policy for bundled HOI4 CWT config."
                .to_string(),
            mime_type: "text/markdown",
            size: self.metadata.len(),
        });

        entries.extend(embedded_hoi4_cwt_sources().map(|source| CwtResourceEntry {
            uri: source.resource_uri(),
            name: source.resource_name(),
            title: format!("HOI4 CWT {}", source.path),
            description: "Bundled virtual source from NS9927/cwtools-hoi4-config.".to_string(),
            mime_type: source.mime_type,
            size: source.content.len(),
        }));

        entries
    }

    pub(crate) fn read_text(&self, uri: &str) -> Option<CwtResourceText> {
        match uri {
            CWT_CATALOG_URI => Some(CwtResourceText {
                text: self.catalog_index.clone(),
                mime_type: "application/toml",
            }),
            CWT_METADATA_URI => Some(CwtResourceText {
                text: self.metadata.clone(),
                mime_type: "text/markdown",
            }),
            _ => {
                let source_path = uri.strip_prefix(CWT_SOURCE_URI_PREFIX)?;
                embedded_hoi4_cwt_sources()
                    .find(|source| source.path == source_path)
                    .map(|source| CwtResourceText {
                        text: source.content.to_string(),
                        mime_type: source.mime_type,
                    })
            }
        }
    }
}

impl Hoi4CwtSource {
    pub fn virtual_path(&self) -> String {
        format!("{}{}", VIRTUAL_SOURCE_PREFIX, self.path)
    }

    pub fn resource_uri(&self) -> String {
        format!("{}{}", CWT_SOURCE_URI_PREFIX, self.path)
    }

    pub fn is_rule_source(&self) -> bool {
        self.path.ends_with(".cwt")
    }

    fn resource_name(&self) -> String {
        let mut name = String::from("hoi4_cwt_");
        for character in self.path.chars() {
            if character.is_ascii_alphanumeric() {
                name.push(character.to_ascii_lowercase());
            } else {
                name.push('_');
            }
        }
        name
    }
}

pub fn embedded_hoi4_cwt_sources() -> impl Iterator<Item = Hoi4CwtSource> {
    cwt_bundle::EMBEDDED_CWT_CONFIG_SOURCES
        .iter()
        .map(|source| Hoi4CwtSource {
            path: source.path,
            content: source.content,
            mime_type: source.mime_type,
        })
}

pub(crate) fn is_cwt_resource_uri(uri: &str) -> bool {
    uri == CWT_CATALOG_URI || uri == CWT_METADATA_URI || uri.starts_with(CWT_SOURCE_URI_PREFIX)
}

fn catalog_index_toml(source_count: usize, rule_source_count: usize, total_bytes: usize) -> String {
    let mut output = String::new();

    writeln!(
        &mut output,
        "source_format = {}",
        toml_string(CWT_RESOURCE_SOURCE_FORMAT)
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "runtime_storage = {}",
        toml_string(CWT_RUNTIME_STORAGE)
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "upstream_url = {}",
        toml_string(cwt_bundle::HOI4_CWT_CONFIG_UPSTREAM_URL)
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "revision = {}",
        toml_string(cwt_bundle::HOI4_CWT_CONFIG_REVISION)
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "license = {}",
        toml_string(cwt_bundle::HOI4_CWT_CONFIG_LICENSE)
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "snapshot_date = {}",
        toml_string(cwt_bundle::HOI4_CWT_CONFIG_SNAPSHOT_DATE)
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "content_sha256 = {}",
        toml_string(cwt_bundle::HOI4_CWT_CONFIG_CONTENT_SHA256)
    )
    .expect("writing to String cannot fail");
    writeln!(&mut output, "source_count = {source_count}").expect("writing to String cannot fail");
    writeln!(&mut output, "rule_source_count = {rule_source_count}")
        .expect("writing to String cannot fail");
    writeln!(&mut output, "total_bytes = {total_bytes}\n").expect("writing to String cannot fail");

    for source in embedded_hoi4_cwt_sources() {
        writeln!(&mut output, "[[sources]]").expect("writing to String cannot fail");
        writeln!(&mut output, "path = {}", toml_string(source.path))
            .expect("writing to String cannot fail");
        writeln!(
            &mut output,
            "virtual_path = {}",
            toml_string(&source.virtual_path())
        )
        .expect("writing to String cannot fail");
        writeln!(
            &mut output,
            "resource_uri = {}",
            toml_string(&source.resource_uri())
        )
        .expect("writing to String cannot fail");
        writeln!(&mut output, "mime_type = {}", toml_string(source.mime_type))
            .expect("writing to String cannot fail");
        writeln!(&mut output, "byte_len = {}", source.content.len())
            .expect("writing to String cannot fail");
        writeln!(&mut output, "rule_source = {}\n", source.is_rule_source())
            .expect("writing to String cannot fail");
    }

    output
}

fn metadata_markdown(source_count: usize, rule_source_count: usize, total_bytes: usize) -> String {
    format!(
        "# HOI4 CWT config snapshot\n\n\
         - Upstream: {}\n\
         - Revision: `{}`\n\
         - License: {}\n\
         - Snapshot date: {}\n\
         - Content SHA-256: `{}`\n\
         - Sources: {}\n\
         - Rule sources: {}\n\
         - Embedded bytes: {}\n\
         - Runtime storage: compiled static strings in process memory.\n\n\
         Virtual paths use `{}`. RHoiScribe does not extract, copy, cache, lock, \
         or rewrite these CWT sources on disk at runtime.\n",
        cwt_bundle::HOI4_CWT_CONFIG_UPSTREAM_URL,
        cwt_bundle::HOI4_CWT_CONFIG_REVISION,
        cwt_bundle::HOI4_CWT_CONFIG_LICENSE,
        cwt_bundle::HOI4_CWT_CONFIG_SNAPSHOT_DATE,
        cwt_bundle::HOI4_CWT_CONFIG_CONTENT_SHA256,
        source_count,
        rule_source_count,
        total_bytes,
        VIRTUAL_SOURCE_PREFIX,
    )
}

fn toml_string(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                write!(&mut output, "\\u{{{:x}}}", character as u32)
                    .expect("writing to String cannot fail");
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}
