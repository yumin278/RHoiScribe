//------------------------------------------------------------------------------------
// rnmdb_store.rs -- Part of RHoiScribe
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
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use rchadow::rnmdb::{RnmdbPageStore, database_error};
use rnmdb_common::ids::PageId;
use rnmdb_storage::{
    Page, PageCryptoKey, PageSize, SingleFileBackend, SingleFileOptions, StorageBackend,
};

pub(crate) const DEFAULT_RNMDB_PAGE_SIZE_BYTES: usize = 16 * 1024;

pub(crate) struct RnmdbSingleFilePageStore {
    backend: SingleFileBackend,
    page_size_bytes: usize,
}

impl RnmdbSingleFilePageStore {
    pub(crate) fn open_or_create(path: &Path, page_size_bytes: usize) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }

        let page_key = rhoiscribe_page_key()?;
        let backend = if path.exists() {
            SingleFileBackend::open_with_key(path, page_key).map_err(|error| error.to_string())?
        } else {
            SingleFileBackend::create(
                path,
                SingleFileOptions::new(PageSize::new(page_size_bytes)).with_page_key(page_key),
            )
            .map_err(|error| error.to_string())?
        };
        let page_size_bytes = backend.page_size().bytes();

        Ok(Self {
            backend,
            page_size_bytes,
        })
    }

    pub(crate) fn read_payload_page(&self, page_id: u64) -> Result<Option<Vec<u8>>, String> {
        self.backend
            .read_page(PageId::new(page_id))
            .map(|page| page.map(|page| page.payload().to_vec()))
            .map_err(|error| error.to_string())
    }

    pub(crate) fn write_payload_page(&self, page_id: u64, payload: Vec<u8>) -> Result<(), String> {
        if payload.len() != self.page_size_bytes {
            return Err(format!(
                "RNMDB page payload must be {} bytes, got {}",
                self.page_size_bytes,
                payload.len()
            ));
        }

        let page = Page::new(PageId::new(page_id), payload).map_err(|error| error.to_string())?;
        self.backend
            .write_page(page)
            .and_then(|_| self.backend.sync().map(|_| ()))
            .map_err(|error| error.to_string())
    }

    pub(crate) fn page_size_bytes(&self) -> usize {
        self.page_size_bytes
    }
}

impl RnmdbPageStore for RnmdbSingleFilePageStore {
    fn page_size_bytes(&self) -> usize {
        self.page_size_bytes
    }

    fn read_page(&self, page_id: u64) -> rchadow::Result<Option<Vec<u8>>> {
        self.read_payload_page(page_id).map_err(database_error)
    }

    fn write_page(&mut self, page_id: u64, payload: Vec<u8>) -> rchadow::Result<()> {
        self.write_payload_page(page_id, payload)
            .map_err(database_error)
    }

    fn sync(&mut self) -> rchadow::Result<()> {
        self.backend
            .sync()
            .map(|_| ())
            .map_err(|error| database_error(error.to_string()))
    }
}

pub(crate) fn default_rhoiscribe_dir() -> PathBuf {
    user_home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".rhoiscribe")
}

pub(crate) fn clean_display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn user_home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

fn rhoiscribe_page_key() -> Result<PageCryptoKey, String> {
    let path = default_rhoiscribe_dir().join("rnmdb-page.key");
    match read_page_key(&path) {
        Ok(Some(bytes)) => Ok(PageCryptoKey::from_bytes(bytes)),
        Ok(None) => create_page_key(&path).map(PageCryptoKey::from_bytes),
        Err(error) => Err(error),
    }
}

fn read_page_key(path: &Path) -> Result<Option<[u8; 32]>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    decode_hex_key(content.trim()).map(Some)
}

fn create_page_key(path: &Path) -> Result<[u8; 32], String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|error| error.to_string())?;
    match write_new_page_key(path, &bytes) {
        Ok(()) => Ok(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => read_page_key(path)?
            .ok_or_else(|| "RNMDB page key was created concurrently but is empty".to_string()),
        Err(error) => Err(error.to_string()),
    }
}

fn write_new_page_key(path: &Path, bytes: &[u8; 32]) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(encode_hex_key(bytes).as_bytes())?;
    file.write_all(b"\n")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.sync_all()
}

fn encode_hex_key(bytes: &[u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(nibble_to_hex(byte >> 4));
        output.push(nibble_to_hex(byte & 0x0f));
    }
    output
}

fn decode_hex_key(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 {
        return Err("RNMDB page key must be 64 hexadecimal characters".to_string());
    }

    let mut bytes = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_to_nibble(chunk[0])?;
        let low = hex_to_nibble(chunk[1])?;
        bytes[index] = (high << 4) | low;
    }
    Ok(bytes)
}

fn nibble_to_hex(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0' + value),
        10..=15 => char::from(b'a' + value - 10),
        _ => unreachable!("nibble values are always <= 15"),
    }
}

fn hex_to_nibble(value: u8) -> Result<u8, String> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err("RNMDB page key contains a non-hexadecimal character".to_string()),
    }
}
