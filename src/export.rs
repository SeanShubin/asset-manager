//! Bundle export logic — copies matched files to a flat export directory.

use std::collections::HashMap;
use std::path::Path;

use crate::data::FileRef;
use crate::image_loader;

/// Export files to a flat directory. Returns the number of files written.
///
/// Handles filename collisions by appending `_1`, `_2`, etc.
pub fn export_bundle(export_path: &str, file_keys: &[String]) -> Result<usize, String> {
    let dir = Path::new(export_path);
    std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create export dir: {e}"))?;

    // Track used filenames to handle collisions
    let mut name_counts: HashMap<String, usize> = HashMap::new();
    let mut written = 0;

    for key in file_keys {
        let file_ref = FileRef::from_string(key);
        let bytes = match image_loader::load_raw_bytes(&file_ref) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Skipping {key}: {e}");
                continue;
            }
        };

        let original_name = file_ref.display_name();
        let dest_name = unique_name(&original_name, &mut name_counts);
        let dest_path = dir.join(&dest_name);

        std::fs::write(&dest_path, &bytes)
            .map_err(|e| format!("Write failed for {dest_name}: {e}"))?;
        written += 1;
    }

    Ok(written)
}

/// Generate a unique filename, appending `_1`, `_2`, etc. on collision.
fn unique_name(name: &str, counts: &mut HashMap<String, usize>) -> String {
    let count = counts.entry(name.to_string()).or_insert(0);
    *count += 1;

    if *count == 1 {
        return name.to_string();
    }

    let path = Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = path.extension().and_then(|e| e.to_str());

    match ext {
        Some(ext) => format!("{}_{}.{}", stem, *count - 1, ext),
        None => format!("{}_{}", stem, *count - 1),
    }
}
