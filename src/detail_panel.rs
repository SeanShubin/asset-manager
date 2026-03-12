//! Right panel: tabbed detail view (Browse / Bundles).

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::data::{self, DirRole, FileRef, GridDef};
use crate::export;
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
    cell_selection: Res<CellSelection>,
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
                    &cell_selection,
                ),
                Tab::Bundles => show_bundles_tab(ui, &mut manager, &data_dir, &mut ui_state),
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
    cell_selection: &CellSelection,
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

    // -- Hierarchy designation --
    show_hierarchy(ui, file_ref, manager, data_dir, ui_state);
    ui.separator();

    // -- Grid editor (when an image is loaded) --
    if current.width > 0 {
        show_grid_section(ui, current, camera, grid_state, tile_state, manager, data_dir, ui_state);
        ui.separator();
    }

    // -- Cell info + cell tags (when a cell is selected) --
    if let Some((col, row)) = cell_selection.selected {
        if grid_state.visible && grid_state.cell_w > 0 && grid_state.cell_h > 0 {
            show_cell_section(ui, file_ref, col, row, grid_state, manager, data_dir, ui_state);
            ui.separator();
        }
    }

    // -- Tags (file-level) --
    show_tags_section(ui, file_ref, manager, data_dir, ui_state);
}

// ---------------------------------------------------------------------------
// Hierarchy section
// ---------------------------------------------------------------------------

fn show_hierarchy(
    ui: &mut egui::Ui,
    file_ref: &FileRef,
    manager: &mut ManagerState,
    data_dir: &DataDir,
    ui_state: &mut UiState,
) {
    let normalized = file_ref.to_string_repr();
    let current_role = manager.data.classify_dir(&normalized);

    // Only disk directories can be asset/creator/export roots
    let is_disk_dir = matches!(file_ref, FileRef::Disk(p) if p.is_dir());

    ui.heading("Role");

    match current_role {
        DirRole::AssetRoot => {
            ui.colored_label(
                egui::Color32::from_rgb(100, 220, 100),
                "Asset Root",
            );
            if ui.button("Remove Asset Root").clicked() {
                manager.data.asset_roots.remove(&normalized);
                manager.dirty = true;
                data::save_and_status(manager, data_dir, ui_state);
            }
        }
        DirRole::CreatorRoot => {
            ui.colored_label(
                egui::Color32::from_rgb(100, 160, 255),
                "Creator Root",
            );
            if ui.button("Remove Creator Root").clicked() {
                manager.data.creator_roots.remove(&normalized);
                manager.dirty = true;
                data::save_and_status(manager, data_dir, ui_state);
            }
        }
        DirRole::AssetPackRoot => {
            ui.colored_label(
                egui::Color32::from_rgb(200, 130, 255),
                "Asset Pack Root",
            );
            if ui.button("Remove Asset Pack Root").clicked() {
                manager.data.asset_pack_roots.remove(&normalized);
                manager.dirty = true;
                data::save_and_status(manager, data_dir, ui_state);
            }
        }
        DirRole::ExportRoot => {
            ui.colored_label(
                egui::Color32::from_rgb(255, 200, 100),
                "Export Root",
            );
            if ui.button("Remove Export Root").clicked() {
                manager.data.export_roots.remove(&normalized);
                manager.dirty = true;
                data::save_and_status(manager, data_dir, ui_state);
            }
        }
        DirRole::None => {
            if is_disk_dir {
                if ui.button("Mark as Asset Root").clicked() {
                    manager.data.asset_roots.insert(normalized.clone());
                    manager.dirty = true;
                    data::save_and_status(manager, data_dir, ui_state);
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
                        data::save_and_status(manager, data_dir, ui_state);
                    }
                }

                if ui.button("Mark as Export Root").clicked() {
                    manager.data.export_roots.insert(normalized.clone());
                    manager.dirty = true;
                    data::save_and_status(manager, data_dir, ui_state);
                }
            }

            // Asset pack root: anything inside a creator root (dirs, zips, zip subdirs)
            if let Some(creator_root) = manager.data.is_inside_creator_root(&normalized) {
                if ui.button("Mark as Asset Pack Root").clicked() {
                    manager.data.asset_pack_roots.insert(
                        normalized.clone(),
                        data::AssetPackRootEntry {
                            creator_root: creator_root.clone(),
                        },
                    );
                    manager.dirty = true;
                    data::save_and_status(manager, data_dir, ui_state);
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
    let all_tags = manager.data.all_known_tags();

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
                data::save_and_status(manager, data_dir, ui_state);
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Cell section
// ---------------------------------------------------------------------------

fn show_cell_section(
    ui: &mut egui::Ui,
    file_ref: &FileRef,
    col: u32,
    row: u32,
    grid_state: &GridState,
    manager: &mut ManagerState,
    data_dir: &DataDir,
    ui_state: &mut UiState,
) {
    let file_key = file_ref.to_string_repr();
    let cell_key = format!("{file_key}@{col},{row}");

    ui.heading("Cell");
    ui.label(format!(
        "Col {col}, Row {row} ({}x{} px)",
        grid_state.cell_w, grid_state.cell_h
    ));

    // Cell tags
    let all_tags = manager.data.all_known_tags();
    let active_tags = manager.data.tags.get(&cell_key).cloned().unwrap_or_default();

    ui.horizontal_wrapped(|ui| {
        for tag in &all_tags {
            let is_active = active_tags.contains(tag);
            if ui.selectable_label(is_active, tag).clicked() {
                let entry = manager.data.tags.entry(cell_key.clone()).or_default();
                if is_active {
                    entry.remove(tag);
                    if entry.is_empty() {
                        manager.data.tags.remove(&cell_key);
                    }
                } else {
                    entry.insert(tag.clone());
                }
                manager.dirty = true;
                data::save_and_status(manager, data_dir, ui_state);
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Bundles tab
// ---------------------------------------------------------------------------

fn show_bundles_tab(
    ui: &mut egui::Ui,
    manager: &mut ManagerState,
    data_dir: &DataDir,
    ui_state: &mut UiState,
) {
    // -- Tag management --
    ui.heading("Tags");

    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut ui_state.new_tag_name);
        if ui.button("Add").clicked() && !ui_state.new_tag_name.trim().is_empty() {
            let name = ui_state.new_tag_name.trim().to_string();
            if manager.data.tag_names.insert(name) {
                manager.dirty = true;
                data::save_and_status(manager, data_dir, ui_state);
            }
            ui_state.new_tag_name.clear();
        }
    });

    let all_tags = manager.data.all_known_tags();
    let mut tag_to_delete: Option<String> = None;

    for tag in &all_tags {
        let count = manager.data.tag_count(tag);
        ui.horizontal(|ui| {
            ui.label(format!("{tag} ({count})"));
            if ui
                .small_button(egui::RichText::new("x").color(egui::Color32::from_rgb(255, 100, 100)))
                .clicked()
            {
                tag_to_delete = Some(tag.clone());
            }
        });
    }

    if let Some(tag) = tag_to_delete {
        manager.data.delete_tag(&tag);
        manager.dirty = true;
        data::save_and_status(manager, data_dir, ui_state);
    }

    ui.separator();

    // -- Bundle management --
    ui.heading("Bundles");

    // Create new bundle
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut ui_state.new_bundle_name);
        if ui.button("Create").clicked() && !ui_state.new_bundle_name.trim().is_empty() {
            let name = ui_state.new_bundle_name.trim().to_string();
            if !manager.data.bundles.contains_key(&name) {
                manager.data.bundles.insert(name, data::BundleDef::default());
                manager.dirty = true;
                data::save_and_status(manager, data_dir, ui_state);
            }
            ui_state.new_bundle_name.clear();
        }
    });

    ui.separator();

    let all_tags = manager.data.all_known_tags();

    // Collect bundle names to iterate without borrow conflict
    let bundle_names: Vec<String> = manager.data.bundles.keys().cloned().collect();
    let mut to_delete: Option<String> = None;

    let mut save_needed = false;

    for bundle_name in &bundle_names {
        let id = ui.make_persistent_id(format!("bundle_{bundle_name}"));

        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
            .show_header(ui, |ui| {
                ui.label(egui::RichText::new(bundle_name).strong());
            })
            .body(|ui| {
                let bundle = manager.data.bundles.get_mut(bundle_name).unwrap();

                // Export root selection
                ui.horizontal(|ui| {
                    ui.label("Export to:");
                    let export_roots: Vec<String> =
                        manager.data.export_roots.iter().cloned().collect();
                    if export_roots.is_empty() {
                        ui.colored_label(egui::Color32::GRAY, "(no export roots defined)");
                    } else {
                        egui::ComboBox::from_id_salt(format!("export_root_{bundle_name}"))
                            .selected_text(if bundle.export_path.is_empty() {
                                "(none)"
                            } else {
                                &bundle.export_path
                            })
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_value(
                                        &mut bundle.export_path,
                                        String::new(),
                                        "(none)",
                                    )
                                    .clicked()
                                {
                                    save_needed = true;
                                }
                                for root in &export_roots {
                                    if ui
                                        .selectable_value(
                                            &mut bundle.export_path,
                                            root.clone(),
                                            root,
                                        )
                                        .clicked()
                                    {
                                        save_needed = true;
                                    }
                                }
                            });
                    }
                });

                // Tag filter: absent (--) -> true (+) -> false (-) -> absent
                ui.label("Tag filter:");
                ui.horizontal_wrapped(|ui| {
                    for tag in &all_tags {
                        let state = bundle.tag_filter.get(tag).copied();
                        let (prefix, color) = match state {
                            None => ("--", egui::Color32::GRAY),
                            Some(true) => ("+", egui::Color32::from_rgb(100, 220, 100)),
                            Some(false) => ("-", egui::Color32::from_rgb(255, 100, 100)),
                        };
                        let label = format!("{prefix} {tag}");
                        if ui
                            .add(egui::Button::new(
                                egui::RichText::new(&label).color(color),
                            ))
                            .clicked()
                        {
                            match state {
                                None => { bundle.tag_filter.insert(tag.clone(), true); }
                                Some(true) => { bundle.tag_filter.insert(tag.clone(), false); }
                                Some(false) => { bundle.tag_filter.remove(tag); }
                            }
                            save_needed = true;
                        }
                    }
                });

                // Snapshot for query (avoids borrow conflict with manager)
                let bundle_snapshot = data::BundleDef {
                    export_path: bundle.export_path.clone(),
                    tag_filter: bundle.tag_filter.clone(),
                };
                let matched = manager.data.query_bundle_files(&bundle_snapshot);

                // Matched files preview
                ui.label(format!("{} matched files", matched.len()));
                if !matched.is_empty() {
                    egui::ScrollArea::vertical()
                        .id_salt(format!("bundle_files_{bundle_name}"))
                        .max_height(120.0)
                        .show(ui, |ui| {
                            for key in &matched {
                                let file_ref = data::FileRef::from_string(key);
                                ui.label(file_ref.display_name());
                            }
                        });
                }

                // Action buttons
                ui.horizontal(|ui| {
                    if ui.button("Export").clicked() {
                        if bundle_snapshot.export_path.trim().is_empty() {
                            ui_state.status_message =
                                Some(("Set an export path first.".into(), 3.0));
                        } else if matched.is_empty() {
                            ui_state.status_message =
                                Some(("No files match this bundle.".into(), 3.0));
                        } else {
                            match export::export_bundle(&bundle_snapshot.export_path, &matched) {
                                Ok(count) => {
                                    ui_state.status_message = Some((
                                        format!("Exported {count} files."),
                                        3.0,
                                    ));
                                }
                                Err(e) => {
                                    ui_state.status_message =
                                        Some((format!("Export failed: {e}"), 5.0));
                                }
                            }
                        }
                    }
                    if ui
                        .button(egui::RichText::new("Delete").color(egui::Color32::from_rgb(255, 100, 100)))
                        .clicked()
                    {
                        to_delete = Some(bundle_name.clone());
                    }
                });
            });
    }

    if save_needed {
        manager.dirty = true;
        data::save_and_status(manager, data_dir, ui_state);
    }

    // Handle deletion outside the loop
    if let Some(name) = to_delete {
        manager.data.bundles.remove(&name);
        manager.dirty = true;
        data::save_and_status(manager, data_dir, ui_state);
    }
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

