//! Bundle export logic — copies matched files into a directory tree
//! using a user-defined path template.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex};

use crate::data::FileRef;
use crate::image_loader;
use crate::resources::{ExportProgress, ExportTask};

// ---------------------------------------------------------------------------
// Template resolution
// ---------------------------------------------------------------------------

/// Extract template variables from a `FileRef` key string.
///
/// Available placeholders:
///   {zip}      — zip filename without extension
///   {dir[N]}   — Nth directory segment of the inner (zip-entry) path (0-indexed)
///   {dir[-N]}  — negative indexing from the end of directory segments
///   {stem}     — filename without extension
///   {ext}      — file extension (no dot)
///   {name}     — full filename (stem.ext)
pub struct TemplateVars {
    pub zip: String,
    /// Directory segments of the inner path (excludes the filename).
    pub dirs: Vec<String>,
    pub stem: String,
    pub ext: String,
    pub name: String,
}

impl TemplateVars {
    pub fn from_key(key: &str) -> Self {
        let file_ref = FileRef::from_string(key);

        let (zip_path, inner) = match &file_ref {
            FileRef::ZipEntry { zip_path, entry } => {
                (Some(zip_path.clone()), entry.clone())
            }
            FileRef::NestedZipEntry { outer_zip, inner_entry, entry } => {
                // Use inner_entry zip as the "zip" and the final entry as the path
                let _ = outer_zip;
                (
                    Some(PathBuf::from(inner_entry)),
                    entry.clone(),
                )
            }
            FileRef::Disk(p) => (None, p.to_string_lossy().into_owned()),
        };

        let zip = zip_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let inner_path = Path::new(&inner);
        let name = inner_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let stem = inner_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let ext = inner_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        // Directory segments = all components except the final filename
        let dirs: Vec<String> = inner_path
            .parent()
            .map(|p| {
                p.components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();

        Self { zip, dirs, stem, ext, name }
    }
}

/// Expand a template string using the given variables.
/// Returns an error if an unrecognised or out-of-range placeholder is found.
fn expand_template(template: &str, vars: &TemplateVars) -> Result<String, String> {
    let mut result = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '{' {
            result.push(ch);
            continue;
        }
        // Collect placeholder name up to '}'
        let mut placeholder = String::new();
        let mut closed = false;
        for c in chars.by_ref() {
            if c == '}' {
                closed = true;
                break;
            }
            placeholder.push(c);
        }
        if !closed {
            return Err(format!("Unclosed placeholder '{{{placeholder}'"));
        }

        let value = resolve_placeholder(&placeholder, vars)?;
        result.push_str(&value);
    }

    Ok(result)
}

fn resolve_placeholder(placeholder: &str, vars: &TemplateVars) -> Result<String, String> {
    match placeholder {
        "zip" => Ok(vars.zip.clone()),
        "stem" => Ok(vars.stem.clone()),
        "ext" => Ok(vars.ext.clone()),
        "name" => Ok(vars.name.clone()),
        _ if placeholder.starts_with("dir[") && placeholder.ends_with(']') => {
            let idx_str = &placeholder[4..placeholder.len() - 1];
            let idx: isize = idx_str
                .parse()
                .map_err(|_| format!("Invalid index in {{{placeholder}}}"))?;
            let len = vars.dirs.len() as isize;
            let resolved = if idx >= 0 { idx } else { len + idx };
            if resolved < 0 || resolved >= len {
                return Err(format!(
                    "{{{placeholder}}} out of range (have {} dir segment{})",
                    vars.dirs.len(),
                    if vars.dirs.len() == 1 { "" } else { "s" }
                ));
            }
            Ok(vars.dirs[resolved as usize].clone())
        }
        _ => Err(format!("Unknown placeholder {{{placeholder}}}")),
    }
}

// ---------------------------------------------------------------------------
// Resolve all output paths up-front and detect conflicts
// ---------------------------------------------------------------------------

/// For each input key, compute the relative output path.
/// Returns an error if any two keys resolve to the same path.
pub fn resolve_output_paths(
    template: &str,
    file_keys: &[String],
) -> Result<Vec<(String, String)>, String> {
    if template.trim().is_empty() {
        return Err("Export template is empty.".into());
    }

    let mut seen: HashMap<String, String> = HashMap::new(); // normalised -> first source key
    let mut out = Vec::with_capacity(file_keys.len());

    for key in file_keys {
        let vars = TemplateVars::from_key(key);
        let rel = expand_template(template, &vars)?;
        let normalised = rel.replace('\\', "/").to_lowercase();

        if let Some(prev_key) = seen.get(&normalised) {
            return Err(format!(
                "Conflict: two files map to \"{rel}\":\n  {prev_key}\n  {key}"
            ));
        }
        seen.insert(normalised, key.clone());
        out.push((key.clone(), rel));
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// Async export
// ---------------------------------------------------------------------------

/// Spawn a background thread that exports files using the template.
///
/// Resolves all output paths first (detecting conflicts synchronously),
/// then copies files on the background thread with progress reporting.
pub fn export_bundle_async(
    export_root: &str,
    template: &str,
    file_keys: &[String],
) -> Result<ExportTask, String> {
    // Resolve paths synchronously so conflicts are caught before any I/O.
    let plan = resolve_output_paths(template, file_keys)?;

    let root = PathBuf::from(export_root);
    let total = plan.len();
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let mut written = 0usize;

        for (key, rel_path) in &plan {
            let file_ref = FileRef::from_string(key);
            let bytes = match image_loader::load_raw_bytes(&file_ref) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Skipping {key}: {e}");
                    continue;
                }
            };

            let dest = root.join(rel_path);
            if let Some(parent) = dest.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    let _ = tx.send(ExportProgress::Failed(
                        format!("Cannot create dir {}: {e}", parent.display()),
                    ));
                    return;
                }
            }

            if let Err(e) = std::fs::write(&dest, &bytes) {
                let _ = tx.send(ExportProgress::Failed(
                    format!("Write failed for {rel_path}: {e}"),
                ));
                return;
            }
            written += 1;
            let _ = tx.send(ExportProgress::Progress(written, total));
        }

        let _ = tx.send(ExportProgress::Done(written));
    });

    Ok(ExportTask {
        receiver: Mutex::new(rx),
        total,
        written: 0,
    })
}

// ---------------------------------------------------------------------------
// Preview helper — returns resolved paths without writing anything
// ---------------------------------------------------------------------------

/// Resolve output paths for preview display. Returns the list or an error string.
pub fn preview_output_paths(
    template: &str,
    file_keys: &[String],
) -> Result<Vec<String>, String> {
    let plan = resolve_output_paths(template, file_keys)?;
    Ok(plan.into_iter().map(|(_, rel)| rel).collect())
}
