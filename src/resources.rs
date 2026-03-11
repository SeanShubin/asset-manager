//! Bevy resources for the asset manager.

use bevy::prelude::*;
use std::path::PathBuf;

use crate::data::FileRef;
use crate::ui_persist::PersistedUiState;
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// Shared constants
// ---------------------------------------------------------------------------

pub const MIN_ZOOM: f32 = 0.1;
pub const MAX_ZOOM: f32 = 50.0;
pub const LEFT_PANEL_WIDTH: f32 = 280.0;
pub const RIGHT_PANEL_WIDTH: f32 = 320.0;
pub const FIT_MARGIN: f32 = 32.0;
pub const STATUS_BAR_HEIGHT: f32 = 28.0;
pub const SCROLL_SETTLE_SECS: f32 = 0.5;
pub const TILE_INSET: f32 = 0.1;

// ---------------------------------------------------------------------------
// Egui pointer ownership
// ---------------------------------------------------------------------------

/// Set each frame by the egui pass — true when the mouse is over a UI panel.
#[derive(Resource, Default)]
pub struct EguiPointerState {
    pub over_ui: bool,
}

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
    /// One-shot: force these nodes open (overrides egui internal state)
    pub force_open: BTreeSet<String>,
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
            force_open: BTreeSet::new(),
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
            force_open: BTreeSet::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Camera state (zoom, pan, drag)
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct CameraState {
    pub zoom: f32,
    pub pan: Vec2,
    pub dragging: bool,
    pub last_cursor: Option<Vec2>,
    pub snap_zoom: bool,
    pub fit_requested: bool,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            dragging: false,
            last_cursor: None,
            snap_zoom: false,
            fit_requested: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Grid overlay state
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
pub struct GridState {
    pub visible: bool,
    pub cell_w: u32,
    pub cell_h: u32,
}

// ---------------------------------------------------------------------------
// Tile preview state
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
pub struct TileState {
    pub enabled: bool,
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
    pub status_message: Option<(String, f64)>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            active_tab: Tab::Browse,
            status_message: None,
        }
    }
}
