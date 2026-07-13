//------------------------------------------------------------------------------------
// state/path.rs -- Part of RHoiScribe
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
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime},
};

use rnmdb_cli::page_key_from_hex;
use rnmdb_storage::PageCryptoKey;

pub(crate) const STATE_DATABASE_FILE_NAME: &str = "state.rnmdb";
pub(crate) const LEGACY_STATE_DATABASE_FILE_NAME: &str = "rhoiscribe-state.rnmdb";
pub(crate) const PAGE_KEY_FILE_NAME: &str = "rnmdb-page.key";

const LOCK_RETRY_COUNT: usize = 250;
const LOCK_RETRY_DELAY: Duration = Duration::from_millis(20);
const STALE_LOCK_AFTER: Duration = Duration::from_secs(30 * 60);

pub(crate) fn default_rhoiscribe_dir() -> PathBuf {
    user_home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".rhoiscribe")
}

pub(crate) fn state_store_path(store_path: Option<&str>) -> PathBuf {
    let path = store_path
        .map(clean_input_path)
        .unwrap_or_else(|| default_rhoiscribe_dir().join(STATE_DATABASE_FILE_NAME));
    canonical_state_database_path(path)
}

fn clean_input_path(path: &str) -> PathBuf {
    PathBuf::from(path.trim().trim_matches('"'))
}

pub(crate) fn canonical_state_database_path(path: PathBuf) -> PathBuf {
    if file_name_matches(&path, LEGACY_STATE_DATABASE_FILE_NAME) {
        return path.with_file_name(STATE_DATABASE_FILE_NAME);
    }
    path
}

pub(crate) fn legacy_state_database_path(path: &Path) -> PathBuf {
    if file_name_matches(path, STATE_DATABASE_FILE_NAME) {
        return path.with_file_name(LEGACY_STATE_DATABASE_FILE_NAME);
    }
    path.to_path_buf()
}

fn file_name_matches(path: &Path, expected: &str) -> bool {
    path.file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(expected))
}

pub(crate) fn clean_display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn user_home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

pub(crate) fn ensure_parent(path: &Path) -> Result<(), String> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(fs::create_dir_all)
        .transpose()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub(crate) fn sync_parent_directory(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    sync_directory(parent).map_err(|error| error.to_string())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(windows)]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?
        .sync_all()
}

#[cfg(not(any(unix, windows)))]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}

pub(crate) fn page_crypto_key() -> Result<PageCryptoKey, String> {
    let path = default_rhoiscribe_dir().join(PAGE_KEY_FILE_NAME);
    match read_page_key(&path)? {
        Some(key) => Ok(key),
        None => create_page_key(&path),
    }
}

pub(crate) fn existing_page_crypto_key() -> Result<PageCryptoKey, String> {
    let path = default_rhoiscribe_dir().join(PAGE_KEY_FILE_NAME);
    read_page_key(&path)?.ok_or_else(|| {
        format!(
            "existing RNMDB page key is missing at {}",
            clean_display_path(&path)
        )
    })
}

fn read_page_key(path: &Path) -> Result<Option<PageCryptoKey>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read RNMDB page key at {}: {error}",
            clean_display_path(path)
        )
    })?;
    page_key_from_hex(content.trim())
        .map(Some)
        .map_err(|error| {
            format!(
                "RNMDB page key at {} is invalid: {error}",
                clean_display_path(path)
            )
        })
}

fn create_page_key(path: &Path) -> Result<PageCryptoKey, String> {
    ensure_parent(path)?;
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|error| error.to_string())?;
    match write_new_page_key(path, &bytes) {
        Ok(()) => Ok(PageCryptoKey::from_bytes(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            concurrent_page_key(path)
        }
        Err(error) => Err(error.to_string()),
    }
}

fn concurrent_page_key(path: &Path) -> Result<PageCryptoKey, String> {
    read_page_key(path)?
        .ok_or_else(|| "RNMDB page key was created concurrently but could not be read".to_string())
}

fn write_new_page_key(path: &Path, bytes: &[u8; 32]) -> std::io::Result<()> {
    let mut file = new_key_file(path)?;
    file.write_all(encode_hex_key(bytes).as_bytes())?;
    file.write_all(b"\n")?;
    secure_key_permissions(&file)?;
    file.sync_all()
}

fn new_key_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

#[cfg(unix)]
fn secure_key_permissions(file: &File) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn secure_key_permissions(_file: &File) -> std::io::Result<()> {
    Ok(())
}

fn encode_hex_key(bytes: &[u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(nibble_to_hex(byte >> 4));
        output.push(nibble_to_hex(byte & 0x0f));
    }
    output
}

fn nibble_to_hex(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0' + value),
        10..=15 => char::from(b'a' + value - 10),
        _ => '?',
    }
}

pub(crate) struct StateMutationLock {
    path: PathBuf,
    _file: File,
}

impl StateMutationLock {
    pub(crate) fn acquire(store_path: &Path) -> Result<Self, String> {
        let path = mutation_lock_path(store_path);
        ensure_parent(&path)?;
        acquire_lock_file(&path).map(|file| Self { path, _file: file })
    }
}

impl Drop for StateMutationLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn mutation_lock_path(store_path: &Path) -> PathBuf {
    let mut file_name = store_path
        .file_name()
        .unwrap_or_else(|| OsStr::new(STATE_DATABASE_FILE_NAME))
        .to_os_string();
    file_name.push(".lock");
    store_path.with_file_name(file_name)
}

fn acquire_lock_file(path: &Path) -> Result<File, String> {
    for _ in 0..LOCK_RETRY_COUNT {
        remove_stale_lock(path)?;
        match try_create_lock(path)? {
            Some(file) => return Ok(file),
            None => thread::sleep(LOCK_RETRY_DELAY),
        }
    }
    Err(format!(
        "timed out waiting for state store lock at {}",
        clean_display_path(path)
    ))
}

fn try_create_lock(path: &Path) -> Result<Option<File>, String> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn remove_stale_lock(path: &Path) -> Result<(), String> {
    if !lock_is_stale(path) {
        return Ok(());
    }
    fs::remove_file(path).map_err(|error| error.to_string())
}

fn lock_is_stale(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    SystemTime::now()
        .duration_since(modified)
        .is_ok_and(|age| age > STALE_LOCK_AFTER)
}
