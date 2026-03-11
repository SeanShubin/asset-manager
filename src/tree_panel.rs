//! Left panel: file system tree browser with zip support and hierarchy indicators.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use std::path::{Path, PathBuf};

use crate::data::{DirRole, FileRef, ManagerData};
use crate::image_loader;
use crate::resources::*;

const MAX_DEPTH: usize = 15;

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

pub fn tree_panel_ui(
    mut contexts: EguiContexts,
    mut selection: ResMut<TreeSelection>,
    mut current: ResMut<CurrentImage>,
    mut browser: ResMut<BrowserState>,
    mut images: ResMut<Assets<Image>>,
    manager: Res<ManagerState>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::SidePanel::left("tree_panel")
        .default_width(280.0)
        .show(ctx, |ui| {
            ui.heading("File Browser");
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                // On Windows, show drive roots
                #[cfg(target_os = "windows")]
                show_drives(ui, &mut selection, &mut current, &mut browser, &mut images, &manager.data);

                #[cfg(not(target_os = "windows"))]
                show_dir(
                    ui, Path::new("/"), 0,
                    &mut selection, &mut current, &mut browser, &mut images,
                    &manager.data,
                );
            });
        });
}

// ---------------------------------------------------------------------------
// Drive listing (Windows)
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn show_drives(
    ui: &mut egui::Ui,
    selection: &mut TreeSelection,
    current: &mut CurrentImage,
    browser: &mut BrowserState,
    images: &mut Assets<Image>,
    data: &ManagerData,
) {
    for letter in b'A'..=b'Z' {
        let drive = format!("{}:/", letter as char);
        let drive_path = PathBuf::from(&drive);
        if drive_path.exists() {
            let role = data.classify_dir(&drive);
            let label = format_dir_label(&drive, &role);

            egui::CollapsingHeader::new(label)
                .id_salt(&drive)
                .show(ui, |ui| {
                    show_dir(ui, &drive_path, 1, selection, current, browser, images, data);
                });
        }
    }
}

// ---------------------------------------------------------------------------
// Recursive directory tree
// ---------------------------------------------------------------------------

fn show_dir(
    ui: &mut egui::Ui,
    path: &Path,
    depth: usize,
    selection: &mut TreeSelection,
    current: &mut CurrentImage,
    browser: &mut BrowserState,
    images: &mut Assets<Image>,
    data: &ManagerData,
) {
    if depth > MAX_DEPTH {
        ui.label("(max depth)");
        return;
    }

    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) => {
            ui.colored_label(egui::Color32::RED, format!("Error: {e}"));
            return;
        }
    };

    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();

    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            dirs.push(p);
        } else {
            files.push(p);
        }
    }

    dirs.sort();
    files.sort();

    // Directories
    for dir in &dirs {
        let dir_str = dir.to_string_lossy().replace('\\', "/");
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?");
        let role = data.classify_dir(&dir_str);
        let label = format_dir_label(name, &role);

        let is_selected = selection.selected_path.as_ref()
            == Some(&FileRef::Disk(dir.clone()));

        let header = egui::CollapsingHeader::new(label)
            .id_salt(dir);

        let response = header.show(ui, |ui| {
            show_dir(ui, dir, depth + 1, selection, current, browser, images, data);
        });

        // Click on header text to select the directory
        if response.header_response.clicked() {
            selection.selected_path = Some(FileRef::Disk(dir.clone()));
        }
    }

    // Files
    for file in &files {
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?");

        let is_zip = name.to_ascii_lowercase().ends_with(".zip");
        let is_image = image_loader::is_image_file(name);

        let file_ref = FileRef::Disk(file.clone());
        let is_selected = selection.selected_path.as_ref() == Some(&file_ref);

        if is_zip {
            // Zip files are expandable
            let zip_label = egui::RichText::new(format!("  \u{1F4E6} {name}"))
                .color(egui::Color32::from_rgb(200, 180, 100));

            let header = egui::CollapsingHeader::new(zip_label)
                .id_salt(file);

            header.show(ui, |ui| {
                show_zip_contents(ui, file, selection, current, browser, images);
            });
        } else {
            let icon = if is_image { "\u{1F5BC}" } else { "\u{1F4C4}" };
            let label_text = format!("  {icon} {name}");

            let label = if is_selected {
                egui::RichText::new(label_text).strong().color(egui::Color32::WHITE)
            } else if is_image {
                egui::RichText::new(label_text).color(egui::Color32::LIGHT_BLUE)
            } else {
                egui::RichText::new(label_text)
            };

            let response = ui.selectable_label(is_selected, label);
            if response.clicked() {
                select_file(file_ref, selection, current, browser, images);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Zip contents tree
// ---------------------------------------------------------------------------

fn show_zip_contents(
    ui: &mut egui::Ui,
    zip_path: &Path,
    selection: &mut TreeSelection,
    current: &mut CurrentImage,
    browser: &mut BrowserState,
    images: &mut Assets<Image>,
) {
    let file = match std::fs::File::open(zip_path) {
        Ok(f) => f,
        Err(e) => {
            ui.colored_label(egui::Color32::RED, format!("Cannot open: {e}"));
            return;
        }
    };

    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            ui.colored_label(egui::Color32::RED, format!("Invalid zip: {e}"));
            return;
        }
    };

    // Build a tree structure from flat entry paths
    let mut entries: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            let name = entry.name().to_string();
            if !name.ends_with('/') {
                entries.push(name);
            }
        }
    }
    entries.sort();

    // Render flat list with indentation based on path depth
    // For simplicity, group by top-level directory
    show_zip_entries(ui, zip_path, &entries, "", selection, current, browser, images);
}

fn show_zip_entries(
    ui: &mut egui::Ui,
    zip_path: &Path,
    entries: &[String],
    prefix: &str,
    selection: &mut TreeSelection,
    current: &mut CurrentImage,
    browser: &mut BrowserState,
    images: &mut Assets<Image>,
) {
    // Collect immediate children and subdirectories under prefix
    let mut subdirs: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();

    for entry in entries {
        let suffix = if prefix.is_empty() {
            entry.as_str()
        } else if let Some(s) = entry.strip_prefix(prefix) {
            s
        } else {
            continue;
        };

        if suffix.is_empty() {
            continue;
        }

        if let Some(slash_pos) = suffix.find('/') {
            let dir_name = &suffix[..slash_pos];
            let full_dir = if prefix.is_empty() {
                format!("{dir_name}/")
            } else {
                format!("{prefix}{dir_name}/")
            };
            if !subdirs.contains(&full_dir) {
                subdirs.push(full_dir);
            }
        } else {
            files.push(entry.clone());
        }
    }

    subdirs.sort();

    for subdir in &subdirs {
        let display_name = subdir.trim_end_matches('/');
        let display_name = display_name.rsplit('/').next().unwrap_or(display_name);

        egui::CollapsingHeader::new(format!("\u{1F4C1} {display_name}"))
            .id_salt(format!("{}{}", zip_path.display(), subdir))
            .show(ui, |ui| {
                show_zip_entries(
                    ui, zip_path, entries, subdir,
                    selection, current, browser, images,
                );
            });
    }

    for file_entry in &files {
        let file_name = file_entry.rsplit('/').next().unwrap_or(file_entry);
        let is_image = image_loader::is_image_file(file_name);
        let file_ref = FileRef::ZipEntry {
            zip_path: zip_path.to_path_buf(),
            entry: file_entry.clone(),
        };

        let is_selected = selection.selected_path.as_ref() == Some(&file_ref);

        let icon = if is_image { "\u{1F5BC}" } else { "\u{1F4C4}" };
        let label_text = format!("  {icon} {file_name}");

        let label = if is_selected {
            egui::RichText::new(label_text).strong().color(egui::Color32::WHITE)
        } else if is_image {
            egui::RichText::new(label_text).color(egui::Color32::LIGHT_BLUE)
        } else {
            egui::RichText::new(label_text)
        };

        let response = ui.selectable_label(is_selected, label);
        if response.clicked() {
            select_file(file_ref, selection, current, browser, images);
        }
    }
}

// ---------------------------------------------------------------------------
// Selection handler
// ---------------------------------------------------------------------------

fn select_file(
    file_ref: FileRef,
    selection: &mut TreeSelection,
    current: &mut CurrentImage,
    browser: &mut BrowserState,
    _images: &mut Assets<Image>,
) {
    let name = file_ref.display_name();
    selection.selected_path = Some(file_ref.clone());

    if image_loader::is_image_file(&name) {
        match image_loader::load_rgba(&file_ref) {
            Ok(rgba) => {
                current.width = rgba.width();
                current.height = rgba.height();
                current.rgba = Some(rgba);
                current.file_ref = Some(file_ref);
                browser.fit_requested = true;
                browser.cell_w = 0;
                browser.cell_h = 0;
            }
            Err(e) => {
                eprintln!("Failed to load image: {e}");
                current.rgba = None;
                current.width = 0;
                current.height = 0;
                current.file_ref = None;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn format_dir_label(name: &str, role: &DirRole) -> egui::RichText {
    match role {
        DirRole::AssetRoot => {
            egui::RichText::new(format!("\u{1F4C1} {name} [AR]"))
                .color(egui::Color32::from_rgb(100, 220, 100))
        }
        DirRole::CreatorRoot => {
            egui::RichText::new(format!("\u{1F4C1} {name} [CR]"))
                .color(egui::Color32::from_rgb(100, 160, 255))
        }
        DirRole::AssetPackRoot => {
            egui::RichText::new(format!("\u{1F4C1} {name} [PR]"))
                .color(egui::Color32::from_rgb(200, 130, 255))
        }
        DirRole::None => {
            egui::RichText::new(format!("\u{1F4C1} {name}"))
        }
    }
}
