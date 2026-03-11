//! Viewport systems: pan/zoom, camera, grid overlay, tile preview.

use bevy::camera::visibility::NoFrustumCulling;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::grid;
use crate::image_loader;
use crate::resources::*;

const GRID_COLOR: Color = Color::srgba(1.0, 1.0, 0.0, 0.4);
const ZOOM_FACTOR: f32 = 1.15;

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct PreviewSprite;

#[derive(Component)]
pub struct TileSprite;

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

pub fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

// ---------------------------------------------------------------------------
// Image display
// ---------------------------------------------------------------------------

pub fn update_preview_sprite(
    mut commands: Commands,
    current: Res<CurrentImage>,
    mut images: ResMut<Assets<Image>>,
    existing: Query<Entity, With<PreviewSprite>>,
    tiles: Query<Entity, With<TileSprite>>,
) {
    if !current.is_changed() {
        return;
    }

    for entity in &existing {
        commands.entity(entity).despawn();
    }
    for entity in &tiles {
        commands.entity(entity).despawn();
    }

    if let Some(ref rgba) = current.rgba {
        let handle = image_loader::rgba_to_bevy_handle(rgba, &mut images);
        commands.spawn((PreviewSprite, Sprite::from_image(handle), NoFrustumCulling));
    }
}

// ---------------------------------------------------------------------------
// Pan / Zoom
// ---------------------------------------------------------------------------

pub fn pan_zoom(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut scroll_events: MessageReader<MouseWheel>,
    mut camera: ResMut<CameraState>,
    pointer: Res<EguiPointerState>,
) {
    // Keyboard shortcuts always work regardless of pointer location
    if keyboard.just_pressed(KeyCode::Home) || keyboard.just_pressed(KeyCode::KeyR) {
        camera.fit_requested = true;
        camera.pan = Vec2::ZERO;
    }

    // Mouse interactions only when pointer is over the preview area (not egui panels)
    if !pointer.over_ui {
        for ev in scroll_events.read() {
            if ev.y == 0.0 {
                continue;
            }
            if ev.y > 0.0 {
                camera.zoom *= ZOOM_FACTOR;
            } else {
                camera.zoom /= ZOOM_FACTOR;
            }
            camera.zoom = camera.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
            if camera.snap_zoom {
                camera.zoom = camera.zoom.round().max(1.0);
            }
        }

        if mouse_buttons.just_pressed(MouseButton::Left) {
            camera.dragging = true;
            camera.last_cursor = windows.single().ok().and_then(|w| w.cursor_position());
        }
    }

    if mouse_buttons.just_released(MouseButton::Left) {
        camera.dragging = false;
        camera.last_cursor = None;
    }

    if camera.dragging {
        let cursor = windows.single().ok().and_then(|w| w.cursor_position());
        if let (Some(current), Some(last)) = (cursor, camera.last_cursor) {
            let delta = current - last;
            let zoom = camera.zoom;
            camera.pan += Vec2::new(delta.x, -delta.y) / zoom;
        }
        camera.last_cursor = cursor;
    }
}

// ---------------------------------------------------------------------------
// Auto-fit zoom
// ---------------------------------------------------------------------------

pub fn auto_fit_zoom(
    mut camera_state: ResMut<CameraState>,
    current: Res<CurrentImage>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if !camera_state.fit_requested {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };

    if current.width == 0 || current.height == 0 {
        camera_state.fit_requested = false;
        return;
    }

    let viewport_w = (window.width() - LEFT_PANEL_WIDTH - RIGHT_PANEL_WIDTH - FIT_MARGIN).max(1.0);
    let viewport_h = (window.height() - STATUS_BAR_HEIGHT - FIT_MARGIN).max(1.0);
    let img_w = current.width as f32;
    let img_h = current.height as f32;

    let zoom = (viewport_w / img_w).min(viewport_h / img_h);
    camera_state.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    camera_state.pan = Vec2::ZERO;
    camera_state.fit_requested = false;
}

// ---------------------------------------------------------------------------
// Apply camera
// ---------------------------------------------------------------------------

pub fn apply_camera(
    camera: Res<CameraState>,
    mut camera_q: Query<&mut Transform, With<Camera2d>>,
) {
    for mut tf in &mut camera_q {
        tf.translation.x = -camera.pan.x;
        tf.translation.y = -camera.pan.y;
        let safe_zoom = camera.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        tf.scale = Vec3::splat(1.0 / safe_zoom);
    }
}

// ---------------------------------------------------------------------------
// Keyboard shortcuts for grid
// ---------------------------------------------------------------------------

pub fn grid_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    current: Res<CurrentImage>,
    mut grid_state: ResMut<GridState>,
    mut ui_state: ResMut<UiState>,
) {
    if keyboard.just_pressed(KeyCode::F1) {
        ui_state.show_shortcuts = !ui_state.show_shortcuts;
    }

    if keyboard.just_pressed(KeyCode::KeyG) {
        grid_state.visible = !grid_state.visible;
    }

    if current.width == 0 || current.height == 0 {
        return;
    }

    if grid_state.cell_w == 0 {
        grid_state.cell_w = current.width;
    }
    if grid_state.cell_h == 0 {
        grid_state.cell_h = current.height;
    }

    let valid_w = grid::valid_cell_sizes(current.width);
    let valid_h = grid::valid_cell_sizes(current.height);

    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let adjust_w = ctrl || !shift;
    let adjust_h = shift || !ctrl;

    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        if adjust_w {
            grid_state.cell_w = grid::prev_valid_size(&valid_w, grid_state.cell_w, current.width);
        }
        if adjust_h {
            grid_state.cell_h = grid::prev_valid_size(&valid_h, grid_state.cell_h, current.height);
        }
    }

    if keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd) {
        if adjust_w {
            grid_state.cell_w = grid::next_valid_size(&valid_w, grid_state.cell_w, current.width);
        }
        if adjust_h {
            grid_state.cell_h = grid::next_valid_size(&valid_h, grid_state.cell_h, current.height);
        }
    }
}

// ---------------------------------------------------------------------------
// Grid overlay
// ---------------------------------------------------------------------------

pub fn draw_grid(
    grid_state: Res<GridState>,
    current: Res<CurrentImage>,
    mut gizmos: Gizmos,
) {
    if !grid_state.visible {
        return;
    }

    let cw = grid_state.cell_w as f32;
    let ch = grid_state.cell_h as f32;
    if cw == 0.0 || ch == 0.0 {
        return;
    }

    let w = current.width as f32;
    let h = current.height as f32;
    if w == 0.0 || h == 0.0 {
        return;
    }

    let cols = (w / cw).round() as i32;
    let rows = (h / ch).round() as i32;
    let left = -w / 2.0;
    let top = h / 2.0;

    for c in 0..=cols {
        let x = left + c as f32 * cw;
        gizmos.line_2d(Vec2::new(x, top), Vec2::new(x, top - h), GRID_COLOR);
    }
    for r in 0..=rows {
        let y = top - r as f32 * ch;
        gizmos.line_2d(Vec2::new(left, y), Vec2::new(left + w, y), GRID_COLOR);
    }
}

// ---------------------------------------------------------------------------
// Tile preview
// ---------------------------------------------------------------------------

pub fn update_tile_preview(
    mut commands: Commands,
    camera_state: Res<CameraState>,
    tile_state: Res<TileState>,
    current: Res<CurrentImage>,
    mut images: ResMut<Assets<Image>>,
    existing_tiles: Query<Entity, With<TileSprite>>,
    preview_sprite: Query<&Sprite, With<PreviewSprite>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if !camera_state.is_changed() && !tile_state.is_changed() && !current.is_changed() {
        return;
    }

    for entity in &existing_tiles {
        commands.entity(entity).despawn();
    }

    if !tile_state.enabled {
        return;
    }

    let Some(ref rgba) = current.rgba else {
        return;
    };

    let Ok(_main_sprite) = preview_sprite.single() else {
        return;
    };

    let Ok(window) = windows.single() else {
        return;
    };

    let w = current.width as f32;
    let h = current.height as f32;
    if w == 0.0 || h == 0.0 {
        return;
    }

    let handle = image_loader::rgba_to_bevy_handle(rgba, &mut images);

    // Use visible viewport area (window minus panels) for tile count
    let safe_zoom = camera_state.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    let view_w = (window.width() - LEFT_PANEL_WIDTH - RIGHT_PANEL_WIDTH).max(1.0);
    let view_h = (window.height() - STATUS_BAR_HEIGHT).max(1.0);
    let world_w = view_w / safe_zoom;
    let world_h = view_h / safe_zoom;
    let cols = ((world_w / w).ceil() as i32 + 2).max(3);
    let rows = ((world_h / h).ceil() as i32 + 2).max(3);

    let rect = bevy::math::Rect::new(TILE_INSET, TILE_INSET, w - TILE_INSET, h - TILE_INSET);

    for row in 0..rows {
        for col in 0..cols {
            if row == rows / 2 && col == cols / 2 {
                continue;
            }
            let offset_x = (col - cols / 2) as f32 * w;
            let offset_y = -(row - rows / 2) as f32 * h;
            commands.spawn((
                TileSprite,
                Sprite {
                    image: handle.clone(),
                    rect: Some(rect),
                    custom_size: Some(Vec2::new(w, h)),
                    ..default()
                },
                Transform::from_xyz(offset_x, offset_y, -0.1),
                NoFrustumCulling,
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Window title
// ---------------------------------------------------------------------------

pub fn update_window_title(
    camera: Res<CameraState>,
    current: Res<CurrentImage>,
    mut windows: Query<&mut Window>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };

    let zoom_pct = (camera.zoom * 100.0).round() as i32;

    let title = if let Some(ref file_ref) = current.file_ref {
        let name = file_ref.display_name();
        let size = if current.width > 0 {
            format!("{}x{}", current.width, current.height)
        } else {
            "?".into()
        };
        format!("Asset Manager — {name} — {size} — {zoom_pct}%")
    } else {
        "Asset Manager".into()
    };

    window.title = title;
}
