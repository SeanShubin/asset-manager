//! Bevy resources for the asset manager.

use bevy::prelude::*;
use std::path::PathBuf;
use std::sync::{mpsc, Mutex};

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
    /// Regex filter text (raw input string)
    pub filter_text: String,
    /// Compiled regex from filter_text (None if empty or invalid)
    pub filter_regex: Option<regex::Regex>,
    /// Search results from recursive scan
    pub search_results: Vec<crate::data::FileRef>,
    /// Root path used for the last search (display only)
    pub search_root: String,
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
            filter_text: String::new(),
            filter_regex: None,
            search_results: Vec::new(),
            search_root: String::new(),
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
            filter_text: String::new(),
            filter_regex: None,
            search_results: Vec::new(),
            search_root: String::new(),
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
    /// Accumulated pixel distance during current drag (to distinguish click vs drag)
    pub drag_distance: f32,
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
            drag_distance: 0.0,
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
// Cell selection
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
pub struct CellSelection {
    /// Currently selected cell (col, row) — None when no cell selected
    pub selected: Option<(u32, u32)>,
    /// Tracks which file the cell belongs to, so we clear on actual file change
    pub file_key: String,
}

// ---------------------------------------------------------------------------
// Animation preview
// ---------------------------------------------------------------------------

/// Walk cycle sequence: idle(1) → walk1(0) → idle(1) → walk2(2)
pub const WALK_CYCLE: [usize; 4] = [1, 0, 1, 2];

/// RPG Maker default: stride 12.5 / move_speed 125 = 100ms per frame
pub const WALK_FRAME_DURATION: f32 = 0.1;

/// Column offsets within a 3-col block: [walk1, idle, walk2]
pub const WALK_FRAME_COL: [u32; 3] = [0, 1, 2];

#[derive(Resource, Default)]
pub struct AnimationPreview {
    pub playing: bool,
    /// Position in WALK_CYCLE (0..4)
    pub cycle_pos: usize,
    pub timer: f32,
}

impl AnimationPreview {
    /// Check if a grid is a valid 4dir-walk grid (cols multiple of 3, rows multiple of 4).
    pub fn is_valid_grid(grid_cols: u32, grid_rows: u32) -> bool {
        grid_cols >= 3 && grid_rows >= 4 && grid_cols % 3 == 0 && grid_rows % 4 == 0
    }
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
    pub info: Option<ImageInfo>,
}

/// Metadata about the loaded image, computed at load time.
#[derive(Clone, Default)]
pub struct ImageInfo {
    pub file_size: u64,
    pub color_type: String,
    pub has_alpha: bool,
    pub unique_colors: usize,
}

// ---------------------------------------------------------------------------
// UI mode / state
// ---------------------------------------------------------------------------

#[derive(Default, PartialEq, Clone, Copy)]
pub enum Tab {
    #[default]
    Browse,
    Bundles,
}

/// Progress update from a background export thread.
pub enum ExportProgress {
    /// (files_written_so_far, total_files)
    Progress(usize, usize),
    /// Export finished successfully — total files written.
    Done(usize),
    /// Export failed with an error message.
    Failed(String),
}

/// An in-flight bundle export running on a background thread.
pub struct ExportTask {
    pub receiver: Mutex<mpsc::Receiver<ExportProgress>>,
    pub total: usize,
    pub written: usize,
}

#[derive(Resource)]
pub struct UiState {
    pub active_tab: Tab,
    pub status_message: Option<(String, f64)>,
    pub show_shortcuts: bool,
    pub new_bundle_name: String,
    pub new_tag_name: String,
    pub export_task: Option<ExportTask>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            active_tab: Tab::Browse,
            status_message: None,
            show_shortcuts: false,
            new_bundle_name: String::new(),
            new_tag_name: String::new(),
            export_task: None,
        }
    }
}
