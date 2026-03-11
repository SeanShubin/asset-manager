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
    mut tree_state: ResMut<TreeState>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::SidePanel::left("tree_panel")
        .default_width(280.0)
        .show(ctx, |ui| {
            ui.heading("File Browser");
            ui.separator();

            let scroll_id = egui::Id::new("tree_scroll");

            // Build scroll area — restore offset on first frame if needed
            let mut scroll_area = egui::ScrollArea::vertical().id_salt(scroll_id);
            if tree_state.restore_scroll {
                scroll_area = scroll_area.vertical_scroll_offset(tree_state.scroll_y);
                tree_state.restore_scroll = false;
            }

            let output = scroll_area.show(ui, |ui| {
                #[cfg(target_os = "windows")]
                show_drives(ui, &mut selection, &mut current, &mut browser, &mut images, &manager.data, &mut tree_state);

                #[cfg(not(target_os = "windows"))]
                show_dir(
                    ui, Path::new("/"), 0,
                    &mut selection, &mut current, &mut browser, &mut images,
                    &manager.data, &mut tree_state,
                );
            });

            // Track scroll position — debounce saves until scroll settles
            let new_scroll_y = output.state.offset.y;
            if (new_scroll_y - tree_state.scroll_y).abs() > 0.5 {
                tree_state.scroll_y = new_scroll_y;
                tree_state.scroll_settle_timer = 0.5; // wait 0.5s after last change
                tree_state.scroll_pending_save = true;
            }
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
    tree_state: &mut TreeState,
) {
    for letter in b'A'..=b'Z' {
        let drive = format!("{}:/", letter as char);
        let drive_path = PathBuf::from(&drive);
        if drive_path.exists() {
            let role = data.classify_dir(&drive);
            let label = format_dir_label(&drive, &role);
            let node_key = drive.clone();
            let is_open = tree_state.expanded.contains(&node_key);

            let response = egui::CollapsingHeader::new(label)
                .id_salt(&drive)
                .default_open(is_open)
                .show(ui, |ui| {
                    show_dir(ui, &drive_path, 1, selection, current, browser, images, data, tree_state);
                });

            track_expansion(response.openness, &node_key, tree_state);
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
    tree_state: &mut TreeState,
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
        let node_key = dir_str.clone();
        let is_open = tree_state.expanded.contains(&node_key);

        let header = egui::CollapsingHeader::new(label)
            .id_salt(dir)
            .default_open(is_open);

        let response = header.show(ui, |ui| {
            show_dir(ui, dir, depth + 1, selection, current, browser, images, data, tree_state);
        });

        track_expansion(response.openness, &node_key, tree_state);

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
            let zip_label = egui::RichText::new(format!("  \u{1F4E6} {name}"))
                .color(egui::Color32::from_rgb(200, 180, 100));

            let node_key = file.to_string_lossy().replace('\\', "/");
            let is_open = tree_state.expanded.contains(&node_key);

            let response = egui::CollapsingHeader::new(zip_label)
                .id_salt(file)
                .default_open(is_open)
                .show(ui, |ui| {
                    show_zip_contents(ui, file, selection, current, browser, images, data, tree_state);
                });

            track_expansion(response.openness, &node_key, tree_state);
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
                select_file(file_ref, selection, current, browser, images, data);
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
    data: &ManagerData,
    tree_state: &mut TreeState,
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

    show_zip_entries(ui, zip_path, &entries, "", selection, current, browser, images, data, tree_state);
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
    data: &ManagerData,
    tree_state: &mut TreeState,
) {
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

        let node_key = format!("{}//{}", zip_path.to_string_lossy().replace('\\', "/"), subdir);
        let is_open = tree_state.expanded.contains(&node_key);

        let response = egui::CollapsingHeader::new(format!("\u{1F4C1} {display_name}"))
            .id_salt(format!("{}{}", zip_path.display(), subdir))
            .default_open(is_open)
            .show(ui, |ui| {
                show_zip_entries(
                    ui, zip_path, entries, subdir,
                    selection, current, browser, images, data, tree_state,
                );
            });

        track_expansion(response.openness, &node_key, tree_state);
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
            select_file(file_ref, selection, current, browser, images, data);
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
    data: &ManagerData,
) {
    let name = file_ref.display_name();
    selection.selected_path = Some(file_ref.clone());

    if image_loader::is_image_file(&name) {
        match image_loader::load_rgba(&file_ref) {
            Ok(rgba) => {
                current.width = rgba.width();
                current.height = rgba.height();
                current.rgba = Some(rgba);
                current.file_ref = Some(file_ref.clone());
                browser.fit_requested = true;

                // Restore saved grid or reset
                let key = file_ref.to_string_repr();
                if let Some(grid) = data.grids.get(&key) {
                    browser.cell_w = grid.cell_w;
                    browser.cell_h = grid.cell_h;
                    browser.grid_visible = true;
                } else {
                    browser.cell_w = 0;
                    browser.cell_h = 0;
                }
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

/// Track whether a collapsing header is open or closed.
/// `openness` is 0.0 (collapsed) to 1.0 (open), animated.
fn track_expansion(openness: f32, node_key: &str, tree_state: &mut TreeState) {
    let was_open = tree_state.expanded.contains(node_key);
    let is_open = openness > 0.5;

    if is_open != was_open {
        if is_open {
            tree_state.expanded.insert(node_key.to_string());
        } else {
            tree_state.expanded.remove(node_key);
        }
        tree_state.save_requested = true;
    }
}

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
