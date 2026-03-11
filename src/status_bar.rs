//! Bottom status bar panel.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::resources::*;

pub fn status_bar_ui(
    mut contexts: EguiContexts,
    mut camera: ResMut<CameraState>,
    grid_state: Res<GridState>,
    tile_state: Res<TileState>,
    current: Res<CurrentImage>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Zoom slider (logarithmic for smooth feel across range)
            ui.label("Zoom:");
            let mut zoom_pct = camera.zoom * 100.0;
            let min_pct = MIN_ZOOM * 100.0;
            let max_pct = MAX_ZOOM * 100.0;
            let slider = egui::Slider::new(&mut zoom_pct, min_pct..=max_pct)
                .logarithmic(true)
                .suffix("%")
                .max_decimals(0);
            if ui.add(slider).changed() {
                camera.zoom = zoom_pct / 100.0;
            }

            ui.separator();

            // Current file
            if let Some(ref file_ref) = current.file_ref {
                ui.label(file_ref.display_name());
                if current.width > 0 {
                    ui.label(format!("({}x{})", current.width, current.height));
                }
            } else {
                ui.label("No image selected");
            }

            ui.separator();

            // Grid info
            if grid_state.visible && grid_state.cell_w > 0 && grid_state.cell_h > 0 {
                ui.label(format!("Grid: {}x{}", grid_state.cell_w, grid_state.cell_h));
            }

            if tile_state.enabled {
                ui.label("| Tiling");
            }
        });
    });
}
