//! Left panel: file system tree browser with zip support and hierarchy indicators.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use std::path::{Path, PathBuf};

use crate::data::{self, DirRole, FileRef, ManagerData};
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
    mut manager: ResMut<ManagerState>,
    mut tree_state: ResMut<TreeState>,
    data_dir: Res<DataDir>,
    mut ui_state: ResMut<UiState>,
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
                || !manager.data.asset_pack_roots.is_empty()
                || !manager.data.export_roots.is_empty();

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
                    for path in &manager.data.export_roots {
                        let short = short_name(path);
                        let label = egui::RichText::new(format!("[EX] {short}"))
                            .color(egui::Color32::from_rgb(255, 200, 100));
                        if ui.add(egui::Button::new(label).frame(false)).clicked() {
                            expand_path_ancestors(path, &mut tree_state);
                        }
                    }
                });
            }

            ui.separator();

            // Regex filter + search
            ui.horizontal(|ui| {
                ui.label("Regex:");
                let response = ui.text_edit_singleline(&mut tree_state.filter_text);
                if response.changed() {
                    tree_state.filter_regex = if tree_state.filter_text.is_empty() {
                        None
                    } else {
                        regex::RegexBuilder::new(&tree_state.filter_text)
                            .case_insensitive(true)
                            .build()
                            .ok()
                    };
                }
                if tree_state.filter_regex.is_none() && !tree_state.filter_text.is_empty() {
                    ui.colored_label(egui::Color32::RED, "!");
                }
            });

            // Search button — scans selected directory recursively
            ui.horizontal(|ui| {
                let has_regex = tree_state.filter_regex.is_some();
                if ui.add_enabled(has_regex, egui::Button::new("Search")).clicked() {
                    let search_root = pick_search_root(&selection, &manager.data);
                    if let Some(root) = search_root {
                        let re = tree_state.filter_regex.clone().unwrap();
                        tree_state.search_root = root.clone();
                        tree_state.search_results = recursive_search(&root, &re);
                    }
                }
                if !tree_state.search_results.is_empty() {
                    if ui.small_button("Clear").clicked() {
                        tree_state.search_results.clear();
                        tree_state.search_root.clear();
                    }
                }
            });

            // Search results
            if !tree_state.search_results.is_empty() {
                ui.separator();
                let count = tree_state.search_results.len();
                let root_short = short_name(&tree_state.search_root);
                ui.label(format!("{count} matches under {root_short}"));

                // Mass tagging — show tag buttons for all results
                let all_tags = manager.data.all_known_tags();
                let result_keys: Vec<String> = tree_state
                    .search_results
                    .iter()
                    .map(|f| f.to_string_repr())
                    .collect();

                ui.horizontal_wrapped(|ui| {
                    for tag in &all_tags {
                        // Count how many results have this tag
                        let have_count = result_keys
                            .iter()
                            .filter(|k| {
                                manager
                                    .data
                                    .tags
                                    .get(*k)
                                    .map_or(false, |t| t.contains(tag))
                            })
                            .count();
                        let total = result_keys.len();

                        let (indicator, color) = if have_count == total {
                            ("[+]", egui::Color32::from_rgb(100, 220, 100))
                        } else if have_count > 0 {
                            ("[~]", egui::Color32::YELLOW)
                        } else {
                            ("[ ]", egui::Color32::GRAY)
                        };

                        let label = format!("{indicator} {tag}");
                        if ui
                            .add(egui::Button::new(
                                egui::RichText::new(&label).color(color).size(11.0),
                            ))
                            .clicked()
                        {
                            // If all have it, remove from all; otherwise add to all
                            let add = have_count < total;
                            for key in &result_keys {
                                let entry =
                                    manager.data.tags.entry(key.clone()).or_default();
                                if add {
                                    entry.insert(tag.clone());
                                } else {
                                    entry.remove(tag);
                                    if entry.is_empty() {
                                        manager.data.tags.remove(key);
                                    }
                                }
                            }
                            manager.dirty = true;
                            data::save_and_status(&mut manager, &data_dir, &mut ui_state);
                        }
                    }
                });

                // Clone results to avoid borrow conflict with tree_state
                let results: Vec<FileRef> = tree_state.search_results.clone();

                egui::ScrollArea::vertical()
                    .id_salt("search_results")
                    .max_height(200.0)
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        for file_ref in &results {
                            let name = file_ref.display_name();
                            let is_image = image_loader::is_image_file(&name);
                            let is_selected = selection.selected_path.as_ref() == Some(file_ref);

                            let icon = if is_image { "\u{1F5BC}" } else { "\u{1F4C4}" };
                            let label = if is_selected {
                                egui::RichText::new(format!("{icon} {name}"))
                                    .strong().color(egui::Color32::WHITE)
                            } else if is_image {
                                egui::RichText::new(format!("{icon} {name}"))
                                    .color(egui::Color32::LIGHT_BLUE)
                            } else {
                                egui::RichText::new(format!("{icon} {name}"))
                            };

                            let response = ui.selectable_label(is_selected, label)
                                .on_hover_text(file_ref.to_string_repr());

                            if is_selected {
                                response.scroll_to_me(Some(egui::Align::Center));
                            }
                            if response.clicked() {
                                // Expand ancestors so the file is visible in the tree
                                let path_str = file_ref.to_string_repr();
                                expand_path_ancestors(&path_str, &mut tree_state);

                                selection.selected_path = Some(file_ref.clone());
                                if image_loader::is_image_file(&name) {
                                    match image_loader::load_image(file_ref) {
                                        Ok(loaded) => {
                                            current.width = loaded.rgba.width();
                                            current.height = loaded.rgba.height();
                                            current.rgba = Some(loaded.rgba);
                                            current.info = Some(loaded.info);
                                            current.file_ref = Some(file_ref.clone());
                                            camera.fit_requested = true;

                                            let key = file_ref.to_string_repr();
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
                                        }
                                    }
                                }
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

        // Apply regex filter to non-zip leaf files
        if !is_zip {
            if let Some(ref re) = ctx.tree_state.filter_regex {
                if !re.is_match(name) {
                    continue;
                }
            }
        }

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

            if response.header_response.clicked() {
                ctx.selection.selected_path = Some(FileRef::Disk(file.clone()));
            }
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

        if response.header_response.clicked() {
            ctx.selection.selected_path = Some(FileRef::ZipEntry {
                zip_path: zip_path.to_path_buf(),
                entry: subdir.clone(),
            });
        }
    }

    for file_entry in &files {
        let file_name = file_entry.rsplit('/').next().unwrap_or(file_entry);
        let is_zip = file_name.to_ascii_lowercase().ends_with(".zip");
        let is_image = image_loader::is_image_file(file_name);

        // Apply regex filter to non-zip leaf files
        if !is_zip {
            if let Some(ref re) = ctx.tree_state.filter_regex {
                if !re.is_match(file_name) {
                    continue;
                }
            }
        }

        if is_zip {
            // Nested zip — render as expandable
            let zip_label = egui::RichText::new(format!("  \u{1F4E6} {file_name}"))
                .color(egui::Color32::from_rgb(200, 180, 100));

            let node_key = format!(
                "{}//{}",
                zip_path.to_string_lossy().replace('\\', "/"),
                file_entry
            );

            let header = egui::CollapsingHeader::new(zip_label)
                .id_salt(format!("nested_{}{}", zip_path.display(), file_entry));
            let header = apply_open_state(header, &node_key, ctx.tree_state);
            let response = header.show(ui, |ui| {
                show_nested_zip_contents(ui, zip_path, file_entry, ctx, data);
            });

            track_expansion(response.openness, &node_key, ctx.tree_state);
        } else {
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
}

// ---------------------------------------------------------------------------
// Nested zip contents tree
// ---------------------------------------------------------------------------

fn show_nested_zip_contents(
    ui: &mut egui::Ui,
    outer_zip: &Path,
    inner_entry: &str,
    ctx: &mut TreeContext,
    data: &ManagerData,
) {
    let inner_bytes = match image_loader::read_zip_entry_bytes(outer_zip, inner_entry) {
        Ok(bytes) => bytes,
        Err(e) => {
            ui.colored_label(egui::Color32::RED, format!("Cannot read inner zip: {e}"));
            return;
        }
    };

    let cursor = std::io::Cursor::new(inner_bytes);
    let mut archive = match zip::ZipArchive::new(cursor) {
        Ok(a) => a,
        Err(e) => {
            ui.colored_label(egui::Color32::RED, format!("Invalid inner zip: {e}"));
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

    show_nested_zip_entries(ui, outer_zip, inner_entry, &entries, "", ctx, data);
}

fn show_nested_zip_entries(
    ui: &mut egui::Ui,
    outer_zip: &Path,
    inner_entry: &str,
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

    let outer_str = outer_zip.to_string_lossy().replace('\\', "/");

    for subdir in &subdirs {
        let display_name = subdir.trim_end_matches('/');
        let display_name = display_name.rsplit('/').next().unwrap_or(display_name);

        let node_key = format!("{outer_str}//{inner_entry}//{subdir}");

        let header = egui::CollapsingHeader::new(format!("\u{1F4C1} {display_name}"))
            .id_salt(format!("nested_{}_{}_{}",outer_str, inner_entry, subdir));
        let header = apply_open_state(header, &node_key, ctx.tree_state);
        let response = header.show(ui, |ui| {
            show_nested_zip_entries(ui, outer_zip, inner_entry, entries, subdir, ctx, data);
        });

        track_expansion(response.openness, &node_key, ctx.tree_state);

        if response.header_response.clicked() {
            ctx.selection.selected_path = Some(FileRef::NestedZipEntry {
                outer_zip: outer_zip.to_path_buf(),
                inner_entry: inner_entry.to_string(),
                entry: subdir.clone(),
            });
        }
    }

    for file_entry in &files {
        let file_name = file_entry.rsplit('/').next().unwrap_or(file_entry);
        let is_image = image_loader::is_image_file(file_name);

        // Apply regex filter
        if let Some(ref re) = ctx.tree_state.filter_regex {
            if !re.is_match(file_name) {
                continue;
            }
        }

        let file_ref = FileRef::NestedZipEntry {
            outer_zip: outer_zip.to_path_buf(),
            inner_entry: inner_entry.to_string(),
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
        match image_loader::load_image(&file_ref) {
            Ok(loaded) => {
                ctx.current.width = loaded.rgba.width();
                ctx.current.height = loaded.rgba.height();
                ctx.current.rgba = Some(loaded.rgba);
                ctx.current.info = Some(loaded.info);
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
                ctx.current.info = None;
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
    tree_state: Res<TreeState>,
) {
    let left = keyboard.just_pressed(KeyCode::ArrowLeft);
    let right = keyboard.just_pressed(KeyCode::ArrowRight);
    let up = keyboard.just_pressed(KeyCode::ArrowUp);
    let down = keyboard.just_pressed(KeyCode::ArrowDown);

    // Up/Down navigate search results when results are present
    if (up || down) && !tree_state.search_results.is_empty() {
        let results = &tree_state.search_results;
        let current_idx = selection
            .selected_path
            .as_ref()
            .and_then(|sel| results.iter().position(|f| f == sel));

        let new_idx = match current_idx {
            Some(idx) => {
                if up && idx > 0 {
                    idx - 1
                } else if down && idx + 1 < results.len() {
                    idx + 1
                } else {
                    return;
                }
            }
            None => 0,
        };

        let new_ref = results[new_idx].clone();
        select_file(new_ref, &mut selection, &mut current, &mut camera, &mut grid_state, &manager);
        return;
    }

    // Left/Right navigate siblings
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
    select_file(new_ref, &mut selection, &mut current, &mut camera, &mut grid_state, &manager);
}

/// Load and select a file, updating all relevant state.
fn select_file(
    file_ref: FileRef,
    selection: &mut TreeSelection,
    current: &mut CurrentImage,
    camera: &mut CameraState,
    grid_state: &mut GridState,
    manager: &ManagerState,
) {
    let name = file_ref.display_name();
    selection.selected_path = Some(file_ref.clone());

    if image_loader::is_image_file(&name) {
        match image_loader::load_image(&file_ref) {
            Ok(loaded) => {
                current.width = loaded.rgba.width();
                current.height = loaded.rgba.height();
                current.rgba = Some(loaded.rgba);
                current.info = Some(loaded.info);
                current.file_ref = Some(file_ref.clone());
                camera.fit_requested = true;

                let key = file_ref.to_string_repr();
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
                current.info = None;
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
        FileRef::NestedZipEntry {
            outer_zip,
            inner_entry,
            entry,
        } => {
            let prefix = match entry.rfind('/') {
                Some(idx) => &entry[..=idx],
                None => "",
            };

            let Ok(inner_bytes) = image_loader::read_zip_entry_bytes(outer_zip, inner_entry)
            else {
                return Vec::new();
            };
            let cursor = std::io::Cursor::new(inner_bytes);
            let Ok(mut archive) = zip::ZipArchive::new(cursor) else {
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
                .map(|e| FileRef::NestedZipEntry {
                    outer_zip: outer_zip.clone(),
                    inner_entry: inner_entry.clone(),
                    entry: e,
                })
                .collect()
        }
    }
}

// ---------------------------------------------------------------------------
// Recursive search
// ---------------------------------------------------------------------------

/// Pick the best root directory to search from.
/// Uses the selected directory, or its parent if a file is selected,
/// or falls back to the first asset root.
fn pick_search_root(selection: &TreeSelection, data: &ManagerData) -> Option<String> {
    if let Some(ref file_ref) = selection.selected_path {
        match file_ref {
            FileRef::Disk(path) => {
                if path.is_dir() {
                    return Some(path.to_string_lossy().replace('\\', "/"));
                }
                if let Some(parent) = path.parent() {
                    return Some(parent.to_string_lossy().replace('\\', "/"));
                }
            }
            FileRef::ZipEntry { zip_path, .. } => {
                if let Some(parent) = zip_path.parent() {
                    return Some(parent.to_string_lossy().replace('\\', "/"));
                }
            }
            FileRef::NestedZipEntry { outer_zip, .. } => {
                if let Some(parent) = outer_zip.parent() {
                    return Some(parent.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    // Fall back to first asset root
    data.asset_roots.iter().next().cloned()
}

const MAX_SEARCH_RESULTS: usize = 200;

/// Recursively search a directory for files matching the regex.
/// Scans disk files and inside zip archives (including nested zips).
fn recursive_search(root: &str, re: &regex::Regex) -> Vec<FileRef> {
    let mut results = Vec::new();
    search_dir(Path::new(root), re, &mut results, 0);
    results
}

fn search_dir(dir: &Path, re: &regex::Regex, results: &mut Vec<FileRef>, depth: usize) {
    if depth > MAX_DEPTH || results.len() >= MAX_SEARCH_RESULTS {
        return;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    let mut items: Vec<_> = entries.flatten().collect();
    items.sort_by_key(|e| e.path());

    for entry in items {
        if results.len() >= MAX_SEARCH_RESULTS {
            return;
        }
        let path = entry.path();
        if path.is_dir() {
            search_dir(&path, re, results, depth + 1);
        } else {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.to_ascii_lowercase().ends_with(".zip") {
                search_zip(&path, re, results);
            } else if re.is_match(name) {
                results.push(FileRef::Disk(path));
            }
        }
    }
}

fn search_zip(zip_path: &Path, re: &regex::Regex, results: &mut Vec<FileRef>) {
    let Ok(file) = std::fs::File::open(zip_path) else {
        return;
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return;
    };

    for i in 0..archive.len() {
        if results.len() >= MAX_SEARCH_RESULTS {
            return;
        }
        let Ok(entry) = archive.by_index(i) else {
            continue;
        };
        let name = entry.name().to_string();
        if name.ends_with('/') {
            continue;
        }

        let file_name = name.rsplit('/').next().unwrap_or(&name);

        if file_name.to_ascii_lowercase().ends_with(".zip") {
            // Nested zip — search inside it
            drop(entry);
            search_nested_zip(zip_path, &name, re, results);
        } else if re.is_match(file_name) {
            results.push(FileRef::ZipEntry {
                zip_path: zip_path.to_path_buf(),
                entry: name,
            });
        }
    }
}

fn search_nested_zip(
    outer_zip: &Path,
    inner_entry: &str,
    re: &regex::Regex,
    results: &mut Vec<FileRef>,
) {
    let Ok(inner_bytes) = image_loader::read_zip_entry_bytes(outer_zip, inner_entry) else {
        return;
    };
    let cursor = std::io::Cursor::new(inner_bytes);
    let Ok(mut archive) = zip::ZipArchive::new(cursor) else {
        return;
    };

    for i in 0..archive.len() {
        if results.len() >= MAX_SEARCH_RESULTS {
            return;
        }
        let Ok(entry) = archive.by_index(i) else {
            continue;
        };
        let name = entry.name().to_string();
        if name.ends_with('/') {
            continue;
        }
        let file_name = name.rsplit('/').next().unwrap_or(&name);
        if re.is_match(file_name) {
            results.push(FileRef::NestedZipEntry {
                outer_zip: outer_zip.to_path_buf(),
                inner_entry: inner_entry.to_string(),
                entry: name,
            });
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
        DirRole::ExportRoot => {
            egui::RichText::new(format!("\u{1F4C1} {name} [EX]"))
                .color(egui::Color32::from_rgb(255, 200, 100))
        }
        DirRole::None => {
            egui::RichText::new(format!("\u{1F4C1} {name}"))
        }
    }
}
