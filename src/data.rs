//! Persistence types for the asset manager.
//!
//! All metadata is stored in a single `asset_manager.toml` in the data directory.
//! Zip-internal paths use `//` as separator: `D:/foo/pack.zip//sprites/hero.png`.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// File reference (disk path or zip entry)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FileRef {
    Disk(PathBuf),
    ZipEntry { zip_path: PathBuf, entry: String },
}

const ZIP_SEPARATOR: &str = "//";

impl FileRef {
    pub fn to_string_repr(&self) -> String {
        match self {
            FileRef::Disk(p) => p.to_string_lossy().replace('\\', "/"),
            FileRef::ZipEntry { zip_path, entry } => {
                format!(
                    "{}{}{}",
                    zip_path.to_string_lossy().replace('\\', "/"),
                    ZIP_SEPARATOR,
                    entry
                )
            }
        }
    }

    pub fn from_string(s: &str) -> Self {
        if let Some(idx) = s.find(ZIP_SEPARATOR) {
            FileRef::ZipEntry {
                zip_path: PathBuf::from(&s[..idx]),
                entry: s[idx + ZIP_SEPARATOR.len()..].to_string(),
            }
        } else {
            FileRef::Disk(PathBuf::from(s))
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            FileRef::Disk(p) => p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string(),
            FileRef::ZipEntry { entry, .. } => {
                Path::new(entry)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Persistence types (TOML)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ManagerData {
    #[serde(default)]
    pub asset_roots: BTreeSet<String>,
    #[serde(default)]
    pub creator_roots: BTreeMap<String, CreatorRootEntry>,
    #[serde(default)]
    pub asset_pack_roots: BTreeMap<String, AssetPackRootEntry>,
    #[serde(default)]
    pub grids: BTreeMap<String, GridDef>,
    #[serde(default)]
    pub bundles: BTreeMap<String, BundleDef>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatorRootEntry {
    pub asset_root: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetPackRootEntry {
    pub creator_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridDef {
    pub cell_w: u32,
    pub cell_h: u32,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BundleDef {
    #[serde(default)]
    pub export_path: String,
    #[serde(default)]
    pub files: Vec<BundleFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleFile {
    pub source: String,
    pub dest: String,
}

// ---------------------------------------------------------------------------
// Directory role classification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirRole {
    AssetRoot,
    CreatorRoot,
    AssetPackRoot,
    None,
}

impl ManagerData {
    pub fn classify_dir(&self, path: &str) -> DirRole {
        let normalized = path.replace('\\', "/");
        if self.asset_roots.contains(&normalized) {
            DirRole::AssetRoot
        } else if self.creator_roots.contains_key(&normalized) {
            DirRole::CreatorRoot
        } else if self.asset_pack_roots.contains_key(&normalized) {
            DirRole::AssetPackRoot
        } else {
            DirRole::None
        }
    }

    pub fn is_inside_asset_root(&self, path: &str) -> Option<String> {
        let normalized = path.replace('\\', "/");
        self.asset_roots
            .iter()
            .find(|root| normalized.starts_with(root.as_str()) && normalized.len() > root.len())
            .cloned()
    }

    pub fn is_inside_creator_root(&self, path: &str) -> Option<String> {
        let normalized = path.replace('\\', "/");
        self.creator_roots
            .keys()
            .find(|root| normalized.starts_with(root.as_str()) && normalized.len() > root.len())
            .cloned()
    }
}

// ---------------------------------------------------------------------------
// Load / Save
// ---------------------------------------------------------------------------

const DATA_FILENAME: &str = "asset_manager.toml";

pub fn data_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join(DATA_FILENAME)
}

pub fn load_data(data_dir: &Path) -> ManagerData {
    let path = data_file_path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(text) => match toml::from_str::<ManagerData>(&text) {
            Ok(data) => {
                eprintln!("Loaded metadata from {}", path.display());
                data
            }
            Err(e) => {
                eprintln!("Failed to parse {}: {e} — starting fresh", path.display());
                ManagerData::default()
            }
        },
        Err(_) => {
            eprintln!(
                "No metadata at {} — starting fresh",
                path.display()
            );
            ManagerData::default()
        }
    }
}

pub fn save_data(data_dir: &Path, data: &ManagerData) -> Result<(), String> {
    let path = data_file_path(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create dir: {e}"))?;
    }
    let text = toml::to_string_pretty(data).map_err(|e| format!("Serialize error: {e}"))?;
    std::fs::write(&path, &text).map_err(|e| format!("Write error: {e}"))?;
    Ok(())
}
