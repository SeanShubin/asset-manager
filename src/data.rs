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
    NestedZipEntry {
        outer_zip: PathBuf,
        inner_entry: String,
        entry: String,
    },
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
            FileRef::NestedZipEntry {
                outer_zip,
                inner_entry,
                entry,
            } => {
                format!(
                    "{}{}{}{}{}",
                    outer_zip.to_string_lossy().replace('\\', "/"),
                    ZIP_SEPARATOR,
                    inner_entry,
                    ZIP_SEPARATOR,
                    entry
                )
            }
        }
    }

    pub fn from_string(s: &str) -> Self {
        // Split on "//" — 1 segment = disk, 2 = zip entry, 3+ = nested zip
        let segments: Vec<&str> = s.split(ZIP_SEPARATOR).collect();
        match segments.len() {
            1 => FileRef::Disk(PathBuf::from(segments[0])),
            2 => FileRef::ZipEntry {
                zip_path: PathBuf::from(segments[0]),
                entry: segments[1].to_string(),
            },
            _ => FileRef::NestedZipEntry {
                outer_zip: PathBuf::from(segments[0]),
                inner_entry: segments[1].to_string(),
                // Join remaining segments back with "//" for deeper nesting
                entry: segments[2..].join(ZIP_SEPARATOR),
            },
        }
    }

    pub fn display_name(&self) -> String {
        let name_str = match self {
            FileRef::Disk(p) => {
                return p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
            }
            FileRef::ZipEntry { entry, .. } => entry.as_str(),
            FileRef::NestedZipEntry { entry, .. } => entry.as_str(),
        };
        Path::new(name_str)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string()
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
    #[serde(default)]
    pub export_roots: BTreeSet<String>,
    /// Registered tag names
    #[serde(default)]
    pub tag_names: BTreeSet<String>,
    /// Tags per file — key is FileRef string repr, value is set of tag strings
    #[serde(default)]
    pub tags: BTreeMap<String, BTreeSet<String>>,
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
    /// Tag filter: true = tag must be present, false = tag must be absent.
    /// Tags not in the map are ignored (don't care).
    #[serde(default)]
    pub tag_filter: BTreeMap<String, bool>,
}

// ---------------------------------------------------------------------------
// Directory role classification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirRole {
    AssetRoot,
    CreatorRoot,
    AssetPackRoot,
    ExportRoot,
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
        } else if self.export_roots.contains(&normalized) {
            DirRole::ExportRoot
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

    /// All known tags: registered names + any actually applied to files, sorted.
    pub fn all_known_tags(&self) -> Vec<String> {
        let mut all = self.tag_names.clone();
        for tags_set in self.tags.values() {
            for tag in tags_set {
                all.insert(tag.clone());
            }
        }
        all.into_iter().collect()
    }

    /// Count how many files have the given tag.
    pub fn tag_count(&self, tag: &str) -> usize {
        self.tags.values().filter(|ts| ts.contains(tag)).count()
    }

    /// Delete a tag everywhere: tag_names, all file tags, all bundle filters.
    pub fn delete_tag(&mut self, tag: &str) {
        self.tag_names.remove(tag);
        for file_tags in self.tags.values_mut() {
            file_tags.remove(tag);
        }
        // Clean up empty tag sets
        self.tags.retain(|_, ts| !ts.is_empty());
        for bundle in self.bundles.values_mut() {
            bundle.tag_filter.remove(tag);
        }
    }

    /// Returns file keys matching a bundle's tag filter.
    /// true = tag must be present, false = tag must be absent.
    /// Bundles with no true entries match nothing.
    pub fn query_bundle_files(&self, bundle: &BundleDef) -> Vec<String> {
        if !bundle.tag_filter.values().any(|&v| v) {
            return Vec::new();
        }

        let mut results: Vec<String> = self
            .tags
            .iter()
            .filter(|(_, file_tags)| {
                for (tag, &required) in &bundle.tag_filter {
                    if required && !file_tags.contains(tag) {
                        return false;
                    }
                    if !required && file_tags.contains(tag) {
                        return false;
                    }
                }
                true
            })
            .map(|(key, _)| key.clone())
            .collect();
        results.sort();
        results
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

/// Save data and update UI status message.
pub fn save_and_status(
    manager: &mut crate::resources::ManagerState,
    data_dir: &crate::resources::DataDir,
    ui_state: &mut crate::resources::UiState,
) {
    match save_data(&data_dir.path, &manager.data) {
        Ok(()) => {
            manager.dirty = false;
            ui_state.status_message = Some(("Saved.".into(), 3.0));
        }
        Err(e) => {
            ui_state.status_message = Some((format!("Save failed: {e}"), 5.0));
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
