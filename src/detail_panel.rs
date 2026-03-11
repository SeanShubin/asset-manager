//! Right panel: tabbed detail view (Browse / Bundles).

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::data::{self, DirRole, FileRef, GridDef};
use crate::grid;
use crate::resources::*;

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

pub fn detail_panel_ui(
    mut contexts: EguiContexts,
    selection: Res<TreeSelection>,
    current: Res<CurrentImage>,
    mut camera: ResMut<CameraState>,
    mut grid_state: ResMut<GridState>,
    mut tile_state: ResMut<TileState>,
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
        .default_width(RIGHT_PANEL_WIDTH)
        .show(ctx, |ui| {
            // Tab bar
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ui_state.active_tab, Tab::Browse, "Browse");
                ui.selectable_value(&mut ui_state.active_tab, Tab::Bundles, "Bundles");
            });
            ui.separator();

            // Status message
            if let Some((ref msg, _)) = ui_state.status_message {
                ui.colored_label(egui::Color32::YELLOW, msg.as_str());
                ui.separator();
            }

            match ui_state.active_tab {
                Tab::Browse => show_browse_tab(
                    ui, &selection, &current, &mut camera, &mut grid_state,
                    &mut tile_state, &mut manager, &data_dir, &mut ui_state,
                ),
                Tab::Bundles => show_bundles_tab(ui),
            }
        });
}

// ---------------------------------------------------------------------------
// Browse tab (combined browse + grid)
// ---------------------------------------------------------------------------

fn show_browse_tab(
    ui: &mut egui::Ui,
    selection: &TreeSelection,
    current: &CurrentImage,
    camera: &mut CameraState,
    grid_state: &mut GridState,
    tile_state: &mut TileState,
    manager: &mut ManagerState,
    data_dir: &DataDir,
    ui_state: &mut UiState,
) {
    let Some(ref file_ref) = selection.selected_path else {
        ui.label("Nothing selected. Browse the file tree on the left.");
        return;
    };

    // -- File info --
    let path_str = file_ref.to_string_repr();
    ui.label(egui::RichText::new(&path_str).strong().size(12.0));

    if current.width > 0 {
        if let Some(ref info) = current.info {
            ui.label(format!(
                "{}x{} px, {} colors, {}",
                current.width,
                current.height,
                info.unique_colors,
                if info.has_alpha { "has alpha" } else { "opaque" },
            ));
            ui.label(format!(
                "{}, {}",
                info.color_type,
                format_file_size(info.file_size),
            ));
        } else {
            ui.label(format!("{}x{} px", current.width, current.height));
        }
    }

    ui.separator();

    // -- Hierarchy designation (disk directories only) --
    if let FileRef::Disk(path) = file_ref {
        if path.is_dir() {
            show_hierarchy(ui, path, manager, data_dir, ui_state);
            ui.separator();
        }
    }

    // -- Grid editor (when an image is loaded) --
    if current.width > 0 {
        show_grid_section(ui, current, camera, grid_state, tile_state, manager, data_dir, ui_state);
        ui.separator();
    }

    // -- Tags --
    show_tags_section(ui, file_ref, manager, data_dir, ui_state);
}

// ---------------------------------------------------------------------------
// Hierarchy section
// ---------------------------------------------------------------------------

fn show_hierarchy(
    ui: &mut egui::Ui,
    path: &std::path::Path,
    manager: &mut ManagerState,
    data_dir: &DataDir,
    ui_state: &mut UiState,
) {
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

    if let Some(ref ar) = manager.data.is_inside_asset_root(&normalized) {
        ui.label(format!("Inside asset root: {ar}"));
    }
    if let Some(ref cr) = manager.data.is_inside_creator_root(&normalized) {
        ui.label(format!("Inside creator root: {cr}"));
    }
}

// ---------------------------------------------------------------------------
// Grid section
// ---------------------------------------------------------------------------

fn show_grid_section(
    ui: &mut egui::Ui,
    current: &CurrentImage,
    camera: &mut CameraState,
    grid_state: &mut GridState,
    tile_state: &mut TileState,
    manager: &mut ManagerState,
    data_dir: &DataDir,
    ui_state: &mut UiState,
) {
    ui.heading("Grid");

    if grid_state.cell_w == 0 {
        grid_state.cell_w = current.width;
    }
    if grid_state.cell_h == 0 {
        grid_state.cell_h = current.height;
    }

    let valid_w = grid::valid_cell_sizes(current.width);
    let valid_h = grid::valid_cell_sizes(current.height);

    // Cell width controls
    ui.horizontal(|ui| {
        ui.label("Width:");
        if ui.small_button("-").clicked() {
            grid_state.cell_w = grid::prev_valid_size(&valid_w, grid_state.cell_w, current.width);
        }
        ui.monospace(format!("{}", grid_state.cell_w));
        if ui.small_button("+").clicked() {
            grid_state.cell_w = grid::next_valid_size(&valid_w, grid_state.cell_w, current.width);
        }
        ui.label("px");
    });

    // Cell height controls
    ui.horizontal(|ui| {
        ui.label("Height:");
        if ui.small_button("-").clicked() {
            grid_state.cell_h = grid::prev_valid_size(&valid_h, grid_state.cell_h, current.height);
        }
        ui.monospace(format!("{}", grid_state.cell_h));
        if ui.small_button("+").clicked() {
            grid_state.cell_h = grid::next_valid_size(&valid_h, grid_state.cell_h, current.height);
        }
        ui.label("px");
    });

    // Grid info
    if grid_state.cell_w > 0 && grid_state.cell_h > 0
        && current.width % grid_state.cell_w == 0
        && current.height % grid_state.cell_h == 0
    {
        let cols = current.width / grid_state.cell_w;
        let rows = current.height / grid_state.cell_h;
        ui.label(format!("{cols} cols x {rows} rows"));
    }

    ui.checkbox(&mut grid_state.visible, "Show grid (G)");

    // Apply / Clear grid buttons
    ui.horizontal(|ui| {
        if ui.button("Apply Grid").clicked() {
            if let Some(ref file_ref) = current.file_ref {
                let key = file_ref.to_string_repr();
                manager.data.grids.insert(key, GridDef {
                    cell_w: grid_state.cell_w,
                    cell_h: grid_state.cell_h,
                });
                manager.dirty = true;
                match data::save_data(&data_dir.path, &manager.data) {
                    Ok(()) => {
                        manager.dirty = false;
                        ui_state.status_message = Some((
                            format!("Grid applied: {}x{}", grid_state.cell_w, grid_state.cell_h),
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
        if let Some(saved_grid) = manager.data.grids.get(&key) {
            ui.colored_label(
                egui::Color32::from_rgb(100, 200, 100),
                format!("Saved: {}x{}", saved_grid.cell_w, saved_grid.cell_h),
            );
        }
    }

    ui.checkbox(&mut camera.snap_zoom, "Snap zoom to integers");
    ui.checkbox(&mut tile_state.enabled, "Tile preview");
}

// ---------------------------------------------------------------------------
// Tags section
// ---------------------------------------------------------------------------

fn show_tags_section(
    ui: &mut egui::Ui,
    file_ref: &FileRef,
    manager: &mut ManagerState,
    data_dir: &DataDir,
    ui_state: &mut UiState,
) {
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
// Bundles tab (placeholder)
// ---------------------------------------------------------------------------

fn show_bundles_tab(ui: &mut egui::Ui) {
    ui.heading("Bundles");
    ui.label("Bundle management coming soon.");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

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
