//! Unified archive abstraction over zip and rar formats.
//!
//! This is the **only** module that knows about specific archive formats.
//! The rest of the codebase calls these functions and never touches
//! `zip` or `unrar` directly.

use std::io::Read;
use std::path::Path;

// ---------------------------------------------------------------------------
// Format detection
// ---------------------------------------------------------------------------

enum Format {
    Zip,
    Rar,
}

fn detect_format(name: &str) -> Option<Format> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".zip") {
        Some(Format::Zip)
    } else if lower.ends_with(".rar") {
        Some(Format::Rar)
    } else {
        None
    }
}

/// Returns `true` if the filename has a recognised archive extension.
pub fn is_archive(name: &str) -> bool {
    detect_format(name).is_some()
}

// ---------------------------------------------------------------------------
// List entries
// ---------------------------------------------------------------------------

/// List all file entries (non-directory) in an archive on disk.
pub fn list_entries(path: &Path) -> Result<Vec<String>, String> {
    let name = path.to_string_lossy();
    match detect_format(&name) {
        Some(Format::Zip) => list_entries_zip_file(path),
        Some(Format::Rar) => list_entries_rar_file(path),
        None => Err(format!("Unrecognised archive format: {name}")),
    }
}

/// List all file entries from archive bytes already in memory.
///
/// `hint_name` is the archive filename, used to detect the format.
pub fn list_entries_from_bytes(bytes: &[u8], hint_name: &str) -> Result<Vec<String>, String> {
    match detect_format(hint_name) {
        Some(Format::Zip) => list_entries_zip_bytes(bytes),
        Some(Format::Rar) => list_entries_rar_bytes(bytes),
        None => Err(format!("Unrecognised archive format: {hint_name}")),
    }
}

// ---------------------------------------------------------------------------
// Read a single entry
// ---------------------------------------------------------------------------

/// Read a single entry's bytes from an archive on disk.
pub fn read_entry(path: &Path, entry_name: &str) -> Result<Vec<u8>, String> {
    let name = path.to_string_lossy();
    match detect_format(&name) {
        Some(Format::Zip) => read_entry_zip_file(path, entry_name),
        Some(Format::Rar) => read_entry_rar_file(path, entry_name),
        None => Err(format!("Unrecognised archive format: {name}")),
    }
}

/// Read a single entry's bytes from archive bytes already in memory.
///
/// `hint_name` is the archive filename, used to detect the format.
pub fn read_entry_from_bytes(
    bytes: &[u8],
    hint_name: &str,
    entry_name: &str,
) -> Result<Vec<u8>, String> {
    match detect_format(hint_name) {
        Some(Format::Zip) => read_entry_zip_bytes(bytes, entry_name),
        Some(Format::Rar) => read_entry_rar_bytes(bytes, entry_name),
        None => Err(format!("Unrecognised archive format: {hint_name}")),
    }
}

// ===========================================================================
// Zip implementation
// ===========================================================================

fn list_entries_zip_file(path: &Path) -> Result<Vec<String>, String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    list_entries_zip(file)
}

fn list_entries_zip_bytes(bytes: &[u8]) -> Result<Vec<String>, String> {
    list_entries_zip(std::io::Cursor::new(bytes))
}

fn list_entries_zip<R: Read + std::io::Seek>(reader: R) -> Result<Vec<String>, String> {
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| format!("Invalid zip: {e}"))?;
    let mut entries = Vec::new();
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            let name = entry.name().to_string();
            if !name.ends_with('/') {
                entries.push(name);
            }
        }
    }
    entries.sort();
    Ok(entries)
}

fn read_entry_zip_file(path: &Path, entry_name: &str) -> Result<Vec<u8>, String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    read_entry_zip(file, entry_name)
}

fn read_entry_zip_bytes(bytes: &[u8], entry_name: &str) -> Result<Vec<u8>, String> {
    read_entry_zip(std::io::Cursor::new(bytes), entry_name)
}

fn read_entry_zip<R: Read + std::io::Seek>(reader: R, entry_name: &str) -> Result<Vec<u8>, String> {
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| format!("Invalid zip: {e}"))?;
    let mut entry = archive
        .by_name(entry_name)
        .map_err(|e| format!("Entry not found: {e}"))?;
    let mut buf = Vec::new();
    entry
        .read_to_end(&mut buf)
        .map_err(|e| format!("Read entry error: {e}"))?;
    Ok(buf)
}

// ===========================================================================
// RAR implementation
// ===========================================================================

fn list_entries_rar_file(path: &Path) -> Result<Vec<String>, String> {
    let archive = unrar::Archive::new(path)
        .open_for_listing()
        .map_err(|e| format!("Cannot open rar {}: {e}", path.display()))?;

    let mut entries = Vec::new();
    for result in archive {
        let header = result.map_err(|e| format!("RAR list error: {e}"))?;
        if !header.is_directory() {
            entries.push(normalize_rar_path(&header.filename));
        }
    }
    entries.sort();
    Ok(entries)
}

fn read_entry_rar_file(path: &Path, entry_name: &str) -> Result<Vec<u8>, String> {
    let mut archive = unrar::Archive::new(path)
        .open_for_processing()
        .map_err(|e| format!("Cannot open rar {}: {e}", path.display()))?;

    while let Some(header) = archive
        .read_header()
        .map_err(|e| format!("RAR header error: {e}"))?
    {
        let name = normalize_rar_path(&header.entry().filename);
        if name == entry_name {
            let (data, _) = header.read().map_err(|e| format!("RAR read error: {e}"))?;
            return Ok(data);
        }
        archive = header.skip().map_err(|e| format!("RAR skip error: {e}"))?;
    }

    Err(format!("Entry not found in rar: {entry_name}"))
}

/// RAR from bytes: write to a temp file, then use the file-based API.
/// The `unrar` crate requires a file path (it wraps the C unrar library).
fn with_temp_rar<T>(bytes: &[u8], f: impl FnOnce(&Path) -> Result<T, String>) -> Result<T, String> {
    let tmp = std::env::temp_dir().join(format!("asset_mgr_{}.rar", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| format!("Cannot write temp rar: {e}"))?;
    let result = f(&tmp);
    let _ = std::fs::remove_file(&tmp);
    result
}

fn list_entries_rar_bytes(bytes: &[u8]) -> Result<Vec<String>, String> {
    with_temp_rar(bytes, list_entries_rar_file)
}

fn read_entry_rar_bytes(bytes: &[u8], entry_name: &str) -> Result<Vec<u8>, String> {
    with_temp_rar(bytes, |path| read_entry_rar_file(path, entry_name))
}

/// Normalize a RAR entry path: backslashes → forward slashes.
fn normalize_rar_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
