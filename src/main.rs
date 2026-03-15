//! Asset Manager — interactive tool for managing asset metadata.
//!
//! Run with:
//!   cargo run
//!   cargo run -- D:/my-data-dir

mod archive;
mod data;
mod detail_panel;
mod export;
mod grid;
mod image_loader;
mod resources;
mod shortcuts;
mod status_bar;
mod tree_panel;
mod ui_persist;
mod viewport;

use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use resources::*;
use std::path::PathBuf;

fn main() {
    let data_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    // Avoid canonicalize on Windows — it adds \\?\ UNC prefix that causes issues
    let data_dir = if data_dir.is_absolute() {
        data_dir
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(&data_dir))
            .unwrap_or(data_dir)
    };

    eprintln!("Data directory: {}", data_dir.display());

    let manager_data = data::load_data(&data_dir);
    let ui_state_persisted = ui_persist::load_ui_state(&data_dir);

    // Restore resources from persisted UI state
    let tree_state = TreeState::from_persisted(&ui_state_persisted);

    let selection = match &ui_state_persisted.selected_path {
        Some(s) => TreeSelection {
            selected_path: Some(data::FileRef::from_string(s)),
        },
        None => TreeSelection::default(),
    };

    let active_tab = match ui_state_persisted.active_tab.as_str() {
        "Bundles" => Tab::Bundles,
        _ => Tab::Browse,
    };

    let camera_state = CameraState {
        zoom: ui_state_persisted.zoom.clamp(MIN_ZOOM, MAX_ZOOM),
        snap_zoom: ui_state_persisted.snap_zoom,
        fit_requested: true,
        ..Default::default()
    };

    let tile_state = TileState {
        enabled: ui_state_persisted.tile_preview,
    };

    App::new()
        .add_plugins((
            DefaultPlugins.set(ImagePlugin::default_nearest()),
            EguiPlugin::default(),
        ))
        .insert_resource(DataDir { path: data_dir })
        .insert_resource(ManagerState {
            data: manager_data,
            dirty: false,
        })
        .insert_resource(selection)
        .insert_resource(camera_state)
        .init_resource::<GridState>()
        .insert_resource(tile_state)
        .insert_resource(tree_state)
        .init_resource::<CurrentImage>()
        .init_resource::<CellSelection>()
        .init_resource::<AnimationPreview>()
        .insert_resource(UiState {
            active_tab,
            ..Default::default()
        })
        .add_systems(Startup, (viewport::setup, restore_selected_image))
        .add_systems(
            Update,
            (
                viewport::update_preview_sprite,
                viewport::clear_cell_on_file_change,
                viewport::pan_zoom,
                viewport::cell_click,
                viewport::cell_hover,
                viewport::animation_tick,
                viewport::grid_keyboard,
                tree_panel::file_navigation,
                viewport::auto_fit_zoom,
                viewport::apply_camera,
                viewport::draw_grid,
                viewport::update_tile_preview,
                viewport::update_window_title,
                save_ui_on_change,
            )
                .chain(),
        )
        .init_resource::<EguiPointerState>()
        .add_systems(
            EguiPrimaryContextPass,
            (
                tree_panel::tree_panel_ui,
                detail_panel::detail_panel_ui,
                status_bar::status_bar_ui,
                shortcuts::shortcuts_overlay,
                update_egui_pointer_state,
            )
                .chain(),
        )
        .run();
}

/// Track whether egui wants the pointer or keyboard input.
fn update_egui_pointer_state(
    mut contexts: bevy_egui::EguiContexts,
    mut pointer_state: ResMut<EguiPointerState>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    pointer_state.over_ui = ctx.is_pointer_over_area();
    pointer_state.wants_keyboard = ctx.wants_keyboard_input();
}

/// On startup, if a selected path was restored, load its image.
fn restore_selected_image(
    selection: Res<TreeSelection>,
    mut current: ResMut<CurrentImage>,
    mut camera: ResMut<CameraState>,
    mut grid_state: ResMut<GridState>,
    manager: Res<ManagerState>,
) {
    let Some(ref file_ref) = selection.selected_path else {
        return;
    };

    let name = file_ref.display_name();
    if !image_loader::is_image_file(&name) {
        return;
    }

    match image_loader::load_image(file_ref) {
        Ok(loaded) => {
            current.width = loaded.rgba.width();
            current.height = loaded.rgba.height();
            current.rgba = Some(loaded.rgba);
            current.info = Some(loaded.info);
            current.file_ref = Some(file_ref.clone());
            camera.fit_requested = true;

            // Restore saved grid
            let key = file_ref.to_string_repr();
            if let Some(grid) = manager.data.grids.get(&key) {
                grid_state.cell_w = grid.cell_w;
                grid_state.cell_h = grid.cell_h;
                grid_state.visible = true;
            }
        }
        Err(e) => {
            eprintln!("Failed to restore image: {e}");
        }
    }
}

/// Save UI state on discrete user actions (expand/collapse, select, tab switch)
/// and after scroll settles (debounced).
fn save_ui_on_change(
    time: Res<Time>,
    data_dir: Res<DataDir>,
    mut tree_state: ResMut<TreeState>,
    selection: Res<TreeSelection>,
    camera: Res<CameraState>,
    tile_state: Res<TileState>,
    ui_state: Res<UiState>,
) {
    // Tick down scroll settle timer
    let mut scroll_settled = false;
    if tree_state.scroll_pending_save {
        tree_state.scroll_settle_timer -= time.delta_secs();
        if tree_state.scroll_settle_timer <= 0.0 {
            tree_state.scroll_pending_save = false;
            scroll_settled = true;
        }
    }

    // Save on: discrete tree actions, scroll settled, selection/tab/viewport changes
    let discrete = tree_state.save_requested
        || scroll_settled
        || selection.is_changed()
        || ui_state.is_changed();
    if !discrete {
        return;
    }
    tree_state.save_requested = false;

    let tab_str = match ui_state.active_tab {
        Tab::Browse => "Browse",
        Tab::Bundles => "Bundles",
    };

    let persisted = ui_persist::PersistedUiState {
        expanded_nodes: tree_state.expanded.clone(),
        tree_scroll_y: tree_state.scroll_y,
        selected_path: selection
            .selected_path
            .as_ref()
            .map(|f| f.to_string_repr()),
        active_tab: tab_str.to_string(),
        zoom: camera.zoom,
        snap_zoom: camera.snap_zoom,
        tile_preview: tile_state.enabled,
    };

    ui_persist::save_ui_state(&data_dir.path, &persisted);
}
