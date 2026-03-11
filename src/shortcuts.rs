//! Keyboard shortcuts help overlay.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::resources::*;

pub fn shortcuts_overlay(
    mut contexts: EguiContexts,
    mut ui_state: ResMut<UiState>,
) {
    if !ui_state.show_shortcuts {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let mut open = true;
    egui::Window::new("Keyboard Shortcuts")
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .open(&mut open)
        .show(ctx, |ui| {
            egui::Grid::new("shortcuts_grid")
                .num_columns(2)
                .spacing([20.0, 4.0])
                .show(ui, |ui| {
                    shortcut(ui, "F1", "Toggle this help");
                    shortcut(ui, "G", "Toggle grid overlay");
                    shortcut(ui, "+  /  -", "Increase / decrease cell size");
                    shortcut(ui, "Ctrl + (+/-)", "Adjust width only");
                    shortcut(ui, "Shift + (+/-)", "Adjust height only");
                    shortcut(ui, "Home  /  R", "Reset zoom (fit to window)");
                    shortcut(ui, "Left  /  Right", "Previous / next sibling file");
                    shortcut(ui, "Scroll wheel", "Zoom in / out (over preview)");
                    shortcut(ui, "Left-click drag", "Pan (over preview)");
                });
        });

    if !open {
        ui_state.show_shortcuts = false;
    }
}

fn shortcut(ui: &mut egui::Ui, key: &str, desc: &str) {
    ui.label(egui::RichText::new(key).strong().monospace());
    ui.label(desc);
    ui.end_row();
}
