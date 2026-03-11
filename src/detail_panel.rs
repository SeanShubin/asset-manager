//! Right panel: tabbed detail view (Browse / Grid / Bundles).

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::data::{self, DirRole, FileRef};
use crate::resources::*;

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

pub fn detail_panel_ui(
    mut contexts: EguiContexts,
    selection: Res<TreeSelection>,
    current: Res<CurrentImage>,
    browser: Res<BrowserState>,
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
                Tab::Grid => show_grid_tab(ui, &current, &browser),
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
}

// ---------------------------------------------------------------------------
// Grid tab
// ---------------------------------------------------------------------------

fn show_grid_tab(
    ui: &mut egui::Ui,
    current: &CurrentImage,
    browser: &BrowserState,
) {
    ui.heading("Grid Editor");

    if current.width == 0 {
        ui.label("Select an image to configure its grid.");
        return;
    }

    ui.label(format!("Image: {}x{} px", current.width, current.height));

    ui.separator();

    // Current grid info
    if browser.cell_w > 0 && browser.cell_h > 0 {
        let cols = current.width / browser.cell_w;
        let rows = current.height / browser.cell_h;
        ui.label(format!(
            "Cell: {}x{} px  ({} cols x {} rows)",
            browser.cell_w, browser.cell_h, cols, rows
        ));
    } else {
        ui.label("No grid set. Use +/- keys or the Grid tab to define one.");
    }

    ui.separator();

    ui.label("Grid controls (+/- keys, Ctrl/Shift modifiers)");
    ui.label("G key to toggle grid visibility");

    // Tile preview info
    if browser.tile_preview {
        ui.separator();
        ui.label(format!(
            "Tile preview: {}x{}",
            browser.tile_cols, browser.tile_rows
        ));
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
