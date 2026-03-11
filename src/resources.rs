//! Bevy resources for the asset manager.

use bevy::prelude::*;
use std::path::PathBuf;

use crate::data::FileRef;

// ---------------------------------------------------------------------------
// Data directory
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct DataDir {
    pub path: PathBuf,
}

// ---------------------------------------------------------------------------
// Manager state (wraps persisted data)
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct ManagerState {
    pub data: crate::data::ManagerData,
    pub dirty: bool,
}

// ---------------------------------------------------------------------------
// Tree selection
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
pub struct TreeSelection {
    pub selected_path: Option<FileRef>,
}

// ---------------------------------------------------------------------------
// Browser / viewport state
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct BrowserState {
    pub zoom: f32,
    pub pan: Vec2,
    pub dragging: bool,
    pub last_cursor: Option<Vec2>,
    pub snap_zoom: bool,
    pub grid_visible: bool,
    pub cell_w: u32,
    pub cell_h: u32,
    pub tile_preview: bool,
    pub tile_cols: u32,
    pub tile_rows: u32,
    pub fit_requested: bool,
}

impl Default for BrowserState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            dragging: false,
            last_cursor: None,
            snap_zoom: false,
            grid_visible: false,
            cell_w: 0,
            cell_h: 0,
            tile_preview: false,
            tile_cols: 3,
            tile_rows: 3,
            fit_requested: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Current image
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
pub struct CurrentImage {
    pub file_ref: Option<FileRef>,
    pub rgba: Option<image::RgbaImage>,
    pub width: u32,
    pub height: u32,
}

// ---------------------------------------------------------------------------
// UI mode / state
// ---------------------------------------------------------------------------

#[derive(Default, PartialEq, Clone, Copy)]
pub enum Tab {
    #[default]
    Browse,
    Grid,
    Bundles,
}

#[derive(Resource)]
pub struct UiState {
    pub active_tab: Tab,
    pub bundle_edit: Option<String>,
    pub new_bundle_name: String,
    pub new_dest_path: String,
    pub status_message: Option<(String, f64)>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            active_tab: Tab::Browse,
            bundle_edit: None,
            new_bundle_name: String::new(),
            new_dest_path: String::new(),
            status_message: None,
        }
    }
}
