//! Bundle export logic — copies matched files to a flat export directory.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{mpsc, Mutex};

use crate::data::FileRef;
use crate::image_loader;
use crate::resources::{ExportProgress, ExportTask};

/// Spawn a background thread that exports files and reports progress.
///
/// Returns an `ExportTask` that the UI can poll each frame.
pub fn export_bundle_async(export_path: &str, file_keys: &[String]) -> Result<ExportTask, String> {
    let dir = Path::new(export_path);
    std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create export dir: {e}"))?;

    let total = file_keys.len();
    let (tx, rx) = mpsc::channel();

    let keys: Vec<String> = file_keys.to_vec();
    let dir_owned = dir.to_path_buf();

    std::thread::spawn(move || {
        let mut name_counts: HashMap<String, usize> = HashMap::new();
        let mut written = 0usize;

        for key in &keys {
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
            let dest_path = dir_owned.join(&dest_name);

            if let Err(e) = std::fs::write(&dest_path, &bytes) {
                let _ = tx.send(ExportProgress::Failed(
                    format!("Write failed for {dest_name}: {e}"),
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
