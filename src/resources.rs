//! Bevy resources for the asset manager.

use bevy::prelude::*;
use std::path::PathBuf;

use crate::data::FileRef;
use crate::ui_persist::PersistedUiState;
use std::collections::BTreeSet;

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
// Tree selection + expansion state
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
pub struct TreeSelection {
    pub selected_path: Option<FileRef>,
}

#[derive(Resource)]
pub struct TreeState {
    /// Set of path strings for expanded nodes
    pub expanded: BTreeSet<String>,
    /// Scroll offset to restore
    pub scroll_y: f32,
    /// Whether scroll needs restoring (first frame only)
    pub restore_scroll: bool,
    /// Debounce timer for scroll saves — counts down after scroll stops
    pub scroll_settle_timer: f32,
    /// Whether scroll changed and is waiting to settle
    pub scroll_pending_save: bool,
    /// Set to true when a discrete action (expand/collapse) needs saving
    pub save_requested: bool,
}

impl Default for TreeState {
    fn default() -> Self {
        Self {
            expanded: BTreeSet::new(),
            scroll_y: 0.0,
            restore_scroll: false,
            scroll_settle_timer: 0.0,
            scroll_pending_save: false,
            save_requested: false,
        }
    }
}

impl TreeState {
    pub fn from_persisted(persisted: &PersistedUiState) -> Self {
        Self {
            expanded: persisted.expanded_nodes.clone(),
            scroll_y: persisted.tree_scroll_y,
            restore_scroll: true,
            scroll_settle_timer: 0.0,
            scroll_pending_save: false,
            save_requested: false,
        }
    }
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
