//! Asset Manager — interactive tool for managing asset metadata.
//!
//! Run with:
//!   cargo run
//!   cargo run -- D:/my-data-dir

mod data;
mod detail_panel;
mod export;
mod image_loader;
mod resources;
mod status_bar;
mod tree_panel;
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

    let data_dir = std::fs::canonicalize(&data_dir).unwrap_or(data_dir);

    eprintln!("Data directory: {}", data_dir.display());

    let manager_data = data::load_data(&data_dir);

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
        .init_resource::<TreeSelection>()
        .init_resource::<BrowserState>()
        .init_resource::<CurrentImage>()
        .init_resource::<UiState>()
        .add_systems(Startup, viewport::setup)
        .add_systems(
            Update,
            (
                viewport::update_preview_sprite,
                viewport::pan_zoom,
                viewport::auto_fit_zoom,
                viewport::apply_camera,
                viewport::draw_grid,
                viewport::update_tile_preview,
                viewport::update_window_title,
            )
                .chain(),
        )
        .add_systems(
            EguiPrimaryContextPass,
            (
                tree_panel::tree_panel_ui,
                detail_panel::detail_panel_ui,
                status_bar::status_bar_ui,
            ),
        )
        .run();
}
