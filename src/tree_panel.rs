//! Left panel: file system tree browser with zip support and hierarchy indicators.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use std::path::{Path, PathBuf};

use crate::data::{DirRole, FileRef, ManagerData};
use crate::image_loader;
use crate::resources::*;

const MAX_DEPTH: usize = 15;

/// Mutable state threaded through the tree rendering call chain.
struct TreeContext<'a> {
    selection: &'a mut TreeSelection,
    current: &'a mut CurrentImage,
    camera: &'a mut CameraState,
    grid_state: &'a mut GridState,
    tree_state: &'a mut TreeState,
}

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

pub fn tree_panel_ui(
    mut contexts: EguiContexts,
    mut selection: ResMut<TreeSelection>,
    mut current: ResMut<CurrentImage>,
    mut camera: ResMut<CameraState>,
    mut grid_state: ResMut<GridState>,
    manager: Res<ManagerState>,
    mut tree_state: ResMut<TreeState>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::SidePanel::left("tree_panel")
        .default_width(LEFT_PANEL_WIDTH)
        .show(ctx, |ui| {
            ui.heading("File Browser");

            // Bookmarks — quick-jump to designated roots
            let has_bookmarks = !manager.data.asset_roots.is_empty()
                || !manager.data.creator_roots.is_empty()
                || !manager.data.asset_pack_roots.is_empty();

            if has_bookmarks {
                ui.separator();
                egui::CollapsingHeader::new("Bookmarks")
                .default_open(true)
                .show(ui, |ui| {
                    for path in &manager.data.asset_roots {
                        let short = short_name(path);
                        let label = egui::RichText::new(format!("[AR] {short}"))
                            .color(egui::Color32::from_rgb(100, 220, 100));
                        if ui.add(egui::Button::new(label).frame(false)).clicked() {
                            expand_path_ancestors(path, &mut tree_state);
                        }
                    }
                    for path in manager.data.creator_roots.keys() {
                        let short = short_name(path);
                        let label = egui::RichText::new(format!("[CR] {short}"))
                            .color(egui::Color32::from_rgb(100, 160, 255));
                        if ui.add(egui::Button::new(label).frame(false)).clicked() {
                            expand_path_ancestors(path, &mut tree_state);
                        }
                    }
                    for path in manager.data.asset_pack_roots.keys() {
                        let short = short_name(path);
                        let label = egui::RichText::new(format!("[PR] {short}"))
                            .color(egui::Color32::from_rgb(200, 130, 255));
                        if ui.add(egui::Button::new(label).frame(false)).clicked() {
                            expand_path_ancestors(path, &mut tree_state);
                        }
                    }
                });
            }

            ui.separator();

            let scroll_id = egui::Id::new("tree_scroll");

            let mut scroll_area = egui::ScrollArea::vertical().id_salt(scroll_id);
            if tree_state.restore_scroll {
                scroll_area = scroll_area.vertical_scroll_offset(tree_state.scroll_y);
                tree_state.restore_scroll = false;
            }

            let mut ctx = TreeContext {
                selection: &mut selection,
                current: &mut current,
                camera: &mut camera,
                grid_state: &mut grid_state,
                tree_state: &mut tree_state,
            };

            let output = scroll_area.show(ui, |ui| {
                #[cfg(target_os = "windows")]
                show_drives(ui, &mut ctx, &manager.data);

                #[cfg(not(target_os = "windows"))]
                show_dir(ui, Path::new("/"), 0, &mut ctx, &manager.data);
            });

            ctx.tree_state.force_open.clear();

            // Track scroll position — debounce saves until scroll settles
            let new_scroll_y = output.state.offset.y;
            if (new_scroll_y - ctx.tree_state.scroll_y).abs() > 0.5 {
                ctx.tree_state.scroll_y = new_scroll_y;
                ctx.tree_state.scroll_settle_timer = SCROLL_SETTLE_SECS;
                ctx.tree_state.scroll_pending_save = true;
            }
        });
}

// ---------------------------------------------------------------------------
// Drive listing (Windows)
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn show_drives(
    ui: &mut egui::Ui,
    ctx: &mut TreeContext,
    data: &ManagerData,
) {
    for letter in b'A'..=b'Z' {
        let drive = format!("{}:/", letter as char);
        let drive_path = PathBuf::from(&drive);
        if drive_path.exists() {
            let role = data.classify_dir(&drive);
            let label = format_dir_label(&drive, &role);
            let node_key = drive.clone();

            let header = egui::CollapsingHeader::new(label).id_salt(&drive);
            let header = apply_open_state(header, &node_key, ctx.tree_state);
            let response = header.show(ui, |ui| {
                show_dir(ui, &drive_path, 1, ctx, data);
            });

            track_expansion(response.openness, &node_key, ctx.tree_state);
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
    ctx: &mut TreeContext,
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
        let node_key = dir_str.clone();

        let header = egui::CollapsingHeader::new(label).id_salt(dir);
        let header = apply_open_state(header, &node_key, ctx.tree_state);
        let response = header.show(ui, |ui| {
            show_dir(ui, dir, depth + 1, ctx, data);
        });

        track_expansion(response.openness, &node_key, ctx.tree_state);

        if response.header_response.clicked() {
            ctx.selection.selected_path = Some(FileRef::Disk(dir.clone()));
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
        let is_selected = ctx.selection.selected_path.as_ref() == Some(&file_ref);

        if is_zip {
            let zip_label = egui::RichText::new(format!("  \u{1F4E6} {name}"))
                .color(egui::Color32::from_rgb(200, 180, 100));

            let node_key = file.to_string_lossy().replace('\\', "/");

            let header = egui::CollapsingHeader::new(zip_label).id_salt(file);
            let header = apply_open_state(header, &node_key, ctx.tree_state);
            let response = header.show(ui, |ui| {
                show_zip_contents(ui, file, ctx, data);
            });

            track_expansion(response.openness, &node_key, ctx.tree_state);
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
                apply_selection(file_ref, ctx, data);
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
    ctx: &mut TreeContext,
    data: &ManagerData,
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

    show_zip_entries(ui, zip_path, &entries, "", ctx, data);
}

fn show_zip_entries(
    ui: &mut egui::Ui,
    zip_path: &Path,
    entries: &[String],
    prefix: &str,
    ctx: &mut TreeContext,
    data: &ManagerData,
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

        let header = egui::CollapsingHeader::new(format!("\u{1F4C1} {display_name}"))
            .id_salt(format!("{}{}", zip_path.display(), subdir));
        let header = apply_open_state(header, &node_key, ctx.tree_state);
        let response = header.show(ui, |ui| {
            show_zip_entries(ui, zip_path, entries, subdir, ctx, data);
        });

        track_expansion(response.openness, &node_key, ctx.tree_state);
    }

    for file_entry in &files {
        let file_name = file_entry.rsplit('/').next().unwrap_or(file_entry);
        let is_image = image_loader::is_image_file(file_name);
        let file_ref = FileRef::ZipEntry {
            zip_path: zip_path.to_path_buf(),
            entry: file_entry.clone(),
        };

        let is_selected = ctx.selection.selected_path.as_ref() == Some(&file_ref);

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
            apply_selection(file_ref, ctx, data);
        }
    }
}

// ---------------------------------------------------------------------------
// Selection handler
// ---------------------------------------------------------------------------

fn apply_selection(
    file_ref: FileRef,
    ctx: &mut TreeContext,
    data: &ManagerData,
) {
    let name = file_ref.display_name();
    ctx.selection.selected_path = Some(file_ref.clone());

    if image_loader::is_image_file(&name) {
        match image_loader::load_rgba(&file_ref) {
            Ok(rgba) => {
                ctx.current.width = rgba.width();
                ctx.current.height = rgba.height();
                ctx.current.rgba = Some(rgba);
                ctx.current.file_ref = Some(file_ref.clone());
                ctx.camera.fit_requested = true;

                // Restore saved grid or reset
                let key = file_ref.to_string_repr();
                if let Some(grid) = data.grids.get(&key) {
                    ctx.grid_state.cell_w = grid.cell_w;
                    ctx.grid_state.cell_h = grid.cell_h;
                    ctx.grid_state.visible = true;
                } else {
                    ctx.grid_state.cell_w = 0;
                    ctx.grid_state.cell_h = 0;
                }
            }
            Err(e) => {
                eprintln!("Failed to load image: {e}");
                ctx.current.rgba = None;
                ctx.current.width = 0;
                ctx.current.height = 0;
                ctx.current.file_ref = None;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Keyboard file navigation (Left/Right arrow)
// ---------------------------------------------------------------------------

pub fn file_navigation(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<TreeSelection>,
    mut current: ResMut<CurrentImage>,
    mut camera: ResMut<CameraState>,
    mut grid_state: ResMut<GridState>,
    manager: Res<ManagerState>,
) {
    let left = keyboard.just_pressed(KeyCode::ArrowLeft);
    let right = keyboard.just_pressed(KeyCode::ArrowRight);
    if !left && !right {
        return;
    }

    let Some(ref file_ref) = selection.selected_path else {
        return;
    };

    let siblings = sibling_files(file_ref);
    if siblings.is_empty() {
        return;
    }

    let current_idx = siblings.iter().position(|f| f == file_ref);
    let new_idx = match current_idx {
        Some(idx) => {
            if left && idx > 0 {
                idx - 1
            } else if right && idx + 1 < siblings.len() {
                idx + 1
            } else {
                return;
            }
        }
        None => 0,
    };

    let new_ref = siblings[new_idx].clone();
    let name = new_ref.display_name();
    selection.selected_path = Some(new_ref.clone());

    if image_loader::is_image_file(&name) {
        match image_loader::load_rgba(&new_ref) {
            Ok(rgba) => {
                current.width = rgba.width();
                current.height = rgba.height();
                current.rgba = Some(rgba);
                current.file_ref = Some(new_ref.clone());
                camera.fit_requested = true;

                let key = new_ref.to_string_repr();
                if let Some(grid) = manager.data.grids.get(&key) {
                    grid_state.cell_w = grid.cell_w;
                    grid_state.cell_h = grid.cell_h;
                    grid_state.visible = true;
                } else {
                    grid_state.cell_w = 0;
                    grid_state.cell_h = 0;
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

/// List sibling files (sorted) for the given FileRef.
fn sibling_files(file_ref: &FileRef) -> Vec<FileRef> {
    match file_ref {
        FileRef::Disk(path) => {
            let Some(parent) = path.parent() else {
                return Vec::new();
            };
            let Ok(entries) = std::fs::read_dir(parent) else {
                return Vec::new();
            };
            let mut files: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect();
            files.sort();
            files.into_iter().map(FileRef::Disk).collect()
        }
        FileRef::ZipEntry { zip_path, entry } => {
            let prefix = match entry.rfind('/') {
                Some(idx) => &entry[..=idx],
                None => "",
            };

            let Ok(file) = std::fs::File::open(zip_path) else {
                return Vec::new();
            };
            let Ok(mut archive) = zip::ZipArchive::new(file) else {
                return Vec::new();
            };

            let mut files: Vec<String> = Vec::new();
            for i in 0..archive.len() {
                if let Ok(ze) = archive.by_index(i) {
                    let name = ze.name().to_string();
                    if name.ends_with('/') {
                        continue;
                    }
                    let suffix = if prefix.is_empty() {
                        name.as_str()
                    } else if let Some(s) = name.strip_prefix(prefix) {
                        s
                    } else {
                        continue;
                    };
                    if !suffix.contains('/') {
                        files.push(name);
                    }
                }
            }
            files.sort();
            files
                .into_iter()
                .map(|e| FileRef::ZipEntry {
                    zip_path: zip_path.clone(),
                    entry: e,
                })
                .collect()
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn apply_open_state(
    header: egui::CollapsingHeader,
    node_key: &str,
    tree_state: &TreeState,
) -> egui::CollapsingHeader {
    if tree_state.force_open.contains(node_key) {
        header.open(Some(true))
    } else {
        header.default_open(tree_state.expanded.contains(node_key))
    }
}

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

fn short_name(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    trimmed.rsplit('/').next().unwrap_or(trimmed)
}

fn expand_path_ancestors(path: &str, tree_state: &mut TreeState) {
    let normalized = path.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').collect();
    let mut ancestor = String::new();
    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            ancestor = format!("{part}/");
        } else if part.is_empty() {
            continue;
        } else {
            ancestor = format!("{}{part}", ancestor);
        }
        tree_state.expanded.insert(ancestor.clone());
        tree_state.force_open.insert(ancestor.clone());
        if i > 0 {
            ancestor.push('/');
        }
    }
    tree_state.expanded.insert(normalized.clone());
    tree_state.force_open.insert(normalized);
    tree_state.save_requested = true;
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
