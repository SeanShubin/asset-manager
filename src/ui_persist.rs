//! Persistent UI state — saved to `ui_state.toml` in the data directory.
//!
//! Tracks session state that should survive restarts:
//! - Tree expansion (which directories are open)
//! - Tree scroll position
//! - Selected tree node
//! - Active tab
//! - Viewport settings (zoom, snap, tile preview)

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

const UI_STATE_FILENAME: &str = "ui_state.toml";

// ---------------------------------------------------------------------------
// Persisted UI state
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PersistedUiState {
    /// Which tree nodes are expanded (path strings, including zip paths with //)
    #[serde(default)]
    pub expanded_nodes: BTreeSet<String>,

    /// Vertical scroll offset of the tree panel
    #[serde(default)]
    pub tree_scroll_y: f32,

    /// Currently selected path in the tree (None if nothing selected)
    #[serde(default)]
    pub selected_path: Option<String>,

    /// Active detail panel tab
    #[serde(default)]
    pub active_tab: String,

    /// Viewport zoom level
    #[serde(default = "default_zoom")]
    pub zoom: f32,

    /// Snap zoom to integers
    #[serde(default)]
    pub snap_zoom: bool,

    /// Tile preview enabled
    #[serde(default)]
    pub tile_preview: bool,
}

fn default_zoom() -> f32 {
    1.0
}

// ---------------------------------------------------------------------------
// Load / Save
// ---------------------------------------------------------------------------

pub fn load_ui_state(data_dir: &Path) -> PersistedUiState {
    let path = data_dir.join(UI_STATE_FILENAME);
    match std::fs::read_to_string(&path) {
        Ok(text) => match toml::from_str::<PersistedUiState>(&text) {
            Ok(state) => {
                eprintln!("Loaded UI state from {}", path.display());
                state
            }
            Err(e) => {
                eprintln!("Failed to parse UI state {}: {e}", path.display());
                PersistedUiState::default()
            }
        },
        Err(_) => PersistedUiState::default(),
    }
}

pub fn save_ui_state(data_dir: &Path, state: &PersistedUiState) {
    let path = data_dir.join(UI_STATE_FILENAME);
    match toml::to_string_pretty(state) {
        Ok(text) => {
            if let Err(e) = std::fs::write(&path, &text) {
                eprintln!("Failed to write UI state: {e}");
            }
        }
        Err(e) => {
            eprintln!("Failed to serialize UI state: {e}");
        }
    }
}
