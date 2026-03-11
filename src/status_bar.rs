//! Bottom status bar panel.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::resources::*;

pub fn status_bar_ui(
    mut contexts: EguiContexts,
    mut browser: ResMut<BrowserState>,
    current: Res<CurrentImage>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Zoom slider (logarithmic for smooth feel across range)
            ui.label("Zoom:");
            let mut zoom_pct = browser.zoom * 100.0;
            let slider = egui::Slider::new(&mut zoom_pct, 10.0..=5000.0)
                .logarithmic(true)
                .suffix("%")
                .max_decimals(0);
            if ui.add(slider).changed() {
                browser.zoom = zoom_pct / 100.0;
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
            if browser.grid_visible && browser.cell_w > 0 && browser.cell_h > 0 {
                ui.label(format!("Grid: {}x{}", browser.cell_w, browser.cell_h));
            }

            if browser.tile_preview {
                ui.label("| Tiling");
            }
        });
    });
}
