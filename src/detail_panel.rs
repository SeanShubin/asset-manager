//! Right panel: tabbed detail view (Browse / Grid / Bundles).

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::data::{self, DirRole, FileRef, GridDef};
use crate::resources::*;

// ---------------------------------------------------------------------------
// Grid helpers
// ---------------------------------------------------------------------------

/// All divisors of `dim` that are >= 8.
fn valid_cell_sizes(dim: u32) -> Vec<u32> {
    (8..=dim).filter(|&s| dim % s == 0).collect()
}

fn prev_valid_size(valid: &[u32], current: u32, dim: u32) -> u32 {
    if valid.is_empty() {
        return (8..current).rev().find(|&d| dim % d == 0).unwrap_or(current);
    }
    valid.iter().copied().rev().find(|&s| s < current).unwrap_or(current)
}

fn next_valid_size(valid: &[u32], current: u32, dim: u32) -> u32 {
    if valid.is_empty() {
        return ((current + 1)..=dim).find(|&d| d >= 8 && dim % d == 0).unwrap_or(current);
    }
    valid.iter().copied().find(|&s| s > current).unwrap_or(current)
}

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

pub fn detail_panel_ui(
    mut contexts: EguiContexts,
    selection: Res<TreeSelection>,
    current: Res<CurrentImage>,
    mut browser: ResMut<BrowserState>,
    mut manager: ResMut<ManagerState>,
    mut ui_state: ResMut<UiState>,
    data_dir: Res<DataDir>,
    time: Res<Time>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // Decay status message
    if let Some((_, ref mut ttl)) = ui_state.status_message {
        *ttl -= time.delta_secs_f64();
        if *ttl <= 0.0 {
            ui_state.status_message = None;
        }
    }

    egui::SidePanel::right("detail_panel")
        .default_width(320.0)
        .show(ctx, |ui| {
            // Tab bar
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ui_state.active_tab, Tab::Browse, "Browse");
                ui.selectable_value(&mut ui_state.active_tab, Tab::Grid, "Grid");
                ui.selectable_value(&mut ui_state.active_tab, Tab::Bundles, "Bundles");
            });
            ui.separator();

            // Status message
            if let Some((ref msg, _)) = ui_state.status_message {
                ui.colored_label(egui::Color32::YELLOW, msg.as_str());
                ui.separator();
            }

            match ui_state.active_tab {
                Tab::Browse => show_browse_tab(ui, &selection, &current, &mut manager, &data_dir, &mut ui_state),
                Tab::Grid => show_grid_tab(ui, &current, &mut browser, &mut manager, &data_dir, &mut ui_state),
                Tab::Bundles => show_bundles_tab(ui),
            }
        });
}

// ---------------------------------------------------------------------------
// Browse tab
// ---------------------------------------------------------------------------

fn show_browse_tab(
    ui: &mut egui::Ui,
    selection: &TreeSelection,
    current: &CurrentImage,
    manager: &mut ManagerState,
    data_dir: &DataDir,
    ui_state: &mut UiState,
) {
    ui.heading("Selected");

    let Some(ref file_ref) = selection.selected_path else {
        ui.label("Nothing selected. Browse the file tree on the left.");
        return;
    };

    // Show path
    let path_str = file_ref.to_string_repr();
    ui.label(egui::RichText::new(&path_str).strong().size(12.0));

    // Image info
    if current.width > 0 {
        ui.label(format!("{}x{} px", current.width, current.height));
    }

    ui.separator();

    // Hierarchy designation (only for disk directories)
    if let FileRef::Disk(path) = file_ref {
        if path.is_dir() {
            let normalized = path.to_string_lossy().replace('\\', "/");
            let current_role = manager.data.classify_dir(&normalized);

            ui.heading("Directory Role");

            match current_role {
                DirRole::AssetRoot => {
                    ui.colored_label(
                        egui::Color32::from_rgb(100, 220, 100),
                        "This is an Asset Root",
                    );
                    if ui.button("Remove Asset Root").clicked() {
                        manager.data.asset_roots.remove(&normalized);
                        manager.dirty = true;
                        save_and_status(manager, data_dir, ui_state);
                    }
                }
                DirRole::CreatorRoot => {
                    ui.colored_label(
                        egui::Color32::from_rgb(100, 160, 255),
                        "This is a Creator Root",
                    );
                    if ui.button("Remove Creator Root").clicked() {
                        manager.data.creator_roots.remove(&normalized);
                        manager.dirty = true;
                        save_and_status(manager, data_dir, ui_state);
                    }
                }
                DirRole::AssetPackRoot => {
                    ui.colored_label(
                        egui::Color32::from_rgb(200, 130, 255),
                        "This is an Asset Pack Root",
                    );
                    if ui.button("Remove Asset Pack Root").clicked() {
                        manager.data.asset_pack_roots.remove(&normalized);
                        manager.dirty = true;
                        save_and_status(manager, data_dir, ui_state);
                    }
                }
                DirRole::None => {
                    // Show available designation buttons based on hierarchy
                    if ui.button("Mark as Asset Root").clicked() {
                        manager.data.asset_roots.insert(normalized.clone());
                        manager.dirty = true;
                        save_and_status(manager, data_dir, ui_state);
                    }

                    if let Some(asset_root) = manager.data.is_inside_asset_root(&normalized) {
                        if ui.button("Mark as Creator Root").clicked() {
                            manager.data.creator_roots.insert(
                                normalized.clone(),
                                data::CreatorRootEntry {
                                    asset_root: asset_root.clone(),
                                },
                            );
                            manager.dirty = true;
                            save_and_status(manager, data_dir, ui_state);
                        }
                    }

                    if let Some(creator_root) = manager.data.is_inside_creator_root(&normalized) {
                        if ui.button("Mark as Asset Pack Root").clicked() {
                            manager.data.asset_pack_roots.insert(
                                normalized.clone(),
                                data::AssetPackRootEntry {
                                    creator_root: creator_root.clone(),
                                },
                            );
                            manager.dirty = true;
                            save_and_status(manager, data_dir, ui_state);
                        }
                    }
                }
            }

            // Show hierarchy context
            ui.separator();
            if let Some(ref ar) = manager.data.is_inside_asset_root(&normalized) {
                ui.label(format!("Inside asset root: {ar}"));
            }
            if let Some(ref cr) = manager.data.is_inside_creator_root(&normalized) {
                ui.label(format!("Inside creator root: {cr}"));
            }
        }
    }

    // Grid info for selected file
    let grid_key = file_ref.to_string_repr();
    if let Some(grid) = manager.data.grids.get(&grid_key) {
        ui.separator();
        ui.label(format!("Grid: {}x{} cells", grid.cell_w, grid.cell_h));
    }

    // Tags
    ui.separator();
    ui.heading("Tags");

    let file_key = file_ref.to_string_repr();

    // Collect all known tags: seeds + any used across all files
    let seed_tags = &["4dir-walk", "8dir-walk"];
    let mut all_tags: Vec<String> = seed_tags.iter().map(|s| s.to_string()).collect();
    for tags_set in manager.data.tags.values() {
        for tag in tags_set {
            if !all_tags.contains(tag) {
                all_tags.push(tag.clone());
            }
        }
    }
    all_tags.sort();

    let active_tags = manager.data.tags.get(&file_key).cloned().unwrap_or_default();

    ui.horizontal_wrapped(|ui| {
        for tag in &all_tags {
            let is_active = active_tags.contains(tag);
            if ui.selectable_label(is_active, tag).clicked() {
                let entry = manager.data.tags.entry(file_key.clone()).or_default();
                if is_active {
                    entry.remove(tag);
                    if entry.is_empty() {
                        manager.data.tags.remove(&file_key);
                    }
                } else {
                    entry.insert(tag.clone());
                }
                manager.dirty = true;
                save_and_status(manager, data_dir, ui_state);
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Grid tab
// ---------------------------------------------------------------------------

fn show_grid_tab(
    ui: &mut egui::Ui,
    current: &CurrentImage,
    browser: &mut BrowserState,
    manager: &mut ManagerState,
    data_dir: &DataDir,
    ui_state: &mut UiState,
) {
    ui.heading("Grid Editor");

    if current.width == 0 {
        ui.label("Select an image to configure its grid.");
        return;
    }

    // File name
    if let Some(ref file_ref) = current.file_ref {
        ui.label(egui::RichText::new(file_ref.display_name()).strong());
    }
    ui.label(format!("{}x{} px", current.width, current.height));

    ui.separator();

    // Initialize cell size if not set
    if browser.cell_w == 0 {
        browser.cell_w = current.width;
    }
    if browser.cell_h == 0 {
        browser.cell_h = current.height;
    }

    let valid_w = valid_cell_sizes(current.width);
    let valid_h = valid_cell_sizes(current.height);

    // Cell width controls
    ui.horizontal(|ui| {
        ui.label("Width:");
        if ui.small_button("-").clicked() {
            browser.cell_w = prev_valid_size(&valid_w, browser.cell_w, current.width);
        }
        ui.monospace(format!("{}", browser.cell_w));
        if ui.small_button("+").clicked() {
            browser.cell_w = next_valid_size(&valid_w, browser.cell_w, current.width);
        }
        ui.label("px");
    });

    // Cell height controls
    ui.horizontal(|ui| {
        ui.label("Height:");
        if ui.small_button("-").clicked() {
            browser.cell_h = prev_valid_size(&valid_h, browser.cell_h, current.height);
        }
        ui.monospace(format!("{}", browser.cell_h));
        if ui.small_button("+").clicked() {
            browser.cell_h = next_valid_size(&valid_h, browser.cell_h, current.height);
        }
        ui.label("px");
    });

    // Grid info
    if browser.cell_w > 0 && browser.cell_h > 0
        && current.width % browser.cell_w == 0
        && current.height % browser.cell_h == 0
    {
        let cols = current.width / browser.cell_w;
        let rows = current.height / browser.cell_h;
        ui.label(format!("{cols} cols x {rows} rows"));
    }

    ui.separator();

    // Grid visibility toggle
    ui.checkbox(&mut browser.grid_visible, "Show grid (G)");

    // Apply / Clear grid buttons
    ui.horizontal(|ui| {
        if ui.button("Apply Grid").clicked() {
            if let Some(ref file_ref) = current.file_ref {
                let key = file_ref.to_string_repr();
                eprintln!("Apply grid: key={key} cell={}x{}", browser.cell_w, browser.cell_h);
                manager.data.grids.insert(key, GridDef {
                    cell_w: browser.cell_w,
                    cell_h: browser.cell_h,
                });
                manager.dirty = true;
                match data::save_data(&data_dir.path, &manager.data) {
                    Ok(()) => {
                        manager.dirty = false;
                        ui_state.status_message = Some((
                            format!("Grid applied: {}x{}", browser.cell_w, browser.cell_h),
                            3.0,
                        ));
                    }
                    Err(e) => {
                        eprintln!("Save failed: {e}");
                        ui_state.status_message = Some((format!("Save failed: {e}"), 5.0));
                    }
                }
            } else {
                ui_state.status_message = Some(("No image selected.".into(), 3.0));
            }
        }
        if ui.button("Clear Grid").clicked() {
            if let Some(ref file_ref) = current.file_ref {
                let key = file_ref.to_string_repr();
                if manager.data.grids.remove(&key).is_some() {
                    manager.dirty = true;
                    match data::save_data(&data_dir.path, &manager.data) {
                        Ok(()) => {
                            manager.dirty = false;
                            ui_state.status_message = Some(("Grid cleared.".into(), 3.0));
                        }
                        Err(e) => {
                            ui_state.status_message = Some((format!("Save failed: {e}"), 5.0));
                        }
                    }
                }
            }
        }
    });

    // Saved grid indicator
    if let Some(ref file_ref) = current.file_ref {
        let key = file_ref.to_string_repr();
        if let Some(grid) = manager.data.grids.get(&key) {
            ui.colored_label(
                egui::Color32::from_rgb(100, 200, 100),
                format!("Saved: {}x{}", grid.cell_w, grid.cell_h),
            );
        }
    }

    ui.separator();

    // Snap zoom
    ui.checkbox(&mut browser.snap_zoom, "Snap zoom to integers");

    ui.separator();

    // Tile preview
    ui.checkbox(&mut browser.tile_preview, "Tile preview");

    if browser.tile_preview {
        ui.horizontal(|ui| {
            ui.label("Cols:");
            let mut cols = browser.tile_cols as i32;
            if ui.add(egui::DragValue::new(&mut cols).range(1..=20)).changed() {
                browser.tile_cols = cols.max(1) as u32;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Rows:");
            let mut rows = browser.tile_rows as i32;
            if ui.add(egui::DragValue::new(&mut rows).range(1..=20)).changed() {
                browser.tile_rows = rows.max(1) as u32;
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Bundles tab (placeholder)
// ---------------------------------------------------------------------------

fn show_bundles_tab(ui: &mut egui::Ui) {
    ui.heading("Bundles");
    ui.label("Bundle management coming soon.");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn save_and_status(manager: &mut ManagerState, data_dir: &DataDir, ui_state: &mut UiState) {
    match data::save_data(&data_dir.path, &manager.data) {
        Ok(()) => {
            manager.dirty = false;
            ui_state.status_message = Some(("Saved.".into(), 3.0));
        }
        Err(e) => {
            ui_state.status_message = Some((format!("Save failed: {e}"), 5.0));
        }
    }
}
