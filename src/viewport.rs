//! Viewport systems: pan/zoom, camera, grid overlay, tile preview.

use bevy::camera::visibility::NoFrustumCulling;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::image_loader;
use crate::resources::*;

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 50.0;
const GRID_COLOR: Color = Color::srgba(1.0, 1.0, 0.0, 0.4);

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

    // Despawn old preview
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    for entity in &tiles {
        commands.entity(entity).despawn();
    }

    // Spawn new preview if we have an image
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
    mut browser: ResMut<BrowserState>,
) {
    // Reset
    if keyboard.just_pressed(KeyCode::Home) || keyboard.just_pressed(KeyCode::KeyR) {
        browser.fit_requested = true;
        browser.pan = Vec2::ZERO;
    }

    // Zoom via scroll wheel — proportional scaling for smooth feel
    for ev in scroll_events.read() {
        if ev.y == 0.0 {
            continue;
        }
        // Multiply/divide by a fixed factor for consistent feel at all zoom levels
        let factor = 1.15_f32;
        if ev.y > 0.0 {
            browser.zoom *= factor;
        } else {
            browser.zoom /= factor;
        }
        browser.zoom = browser.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        if browser.snap_zoom {
            browser.zoom = browser.zoom.round().max(1.0);
        }
    }

    // Pan via left-click drag
    let cursor = windows.single().ok().and_then(|w| w.cursor_position());

    if mouse_buttons.just_pressed(MouseButton::Left) {
        browser.dragging = true;
        browser.last_cursor = cursor;
    }
    if mouse_buttons.just_released(MouseButton::Left) {
        browser.dragging = false;
        browser.last_cursor = None;
    }

    if browser.dragging {
        if let (Some(current), Some(last)) = (cursor, browser.last_cursor) {
            let delta = current - last;
            let zoom = browser.zoom;
            browser.pan += Vec2::new(delta.x, -delta.y) / zoom;
        }
        browser.last_cursor = cursor;
    }
}

// ---------------------------------------------------------------------------
// Auto-fit zoom
// ---------------------------------------------------------------------------

const LEFT_PANEL_WIDTH: f32 = 280.0;
const RIGHT_PANEL_WIDTH: f32 = 320.0;
const FIT_MARGIN: f32 = 32.0;

pub fn auto_fit_zoom(
    mut browser: ResMut<BrowserState>,
    current: Res<CurrentImage>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if !browser.fit_requested {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };

    if current.width == 0 || current.height == 0 {
        browser.fit_requested = false;
        return;
    }

    let viewport_w = (window.width() - LEFT_PANEL_WIDTH - RIGHT_PANEL_WIDTH - FIT_MARGIN).max(1.0);
    let viewport_h = (window.height() - FIT_MARGIN).max(1.0);
    let img_w = current.width as f32;
    let img_h = current.height as f32;

    let zoom = (viewport_w / img_w).min(viewport_h / img_h);
    browser.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    browser.pan = Vec2::ZERO;
    browser.fit_requested = false;
}

// ---------------------------------------------------------------------------
// Apply camera
// ---------------------------------------------------------------------------

pub fn apply_camera(
    browser: Res<BrowserState>,
    mut camera_q: Query<&mut Transform, With<Camera2d>>,
) {
    for mut tf in &mut camera_q {
        tf.translation.x = -browser.pan.x;
        tf.translation.y = -browser.pan.y;
        let safe_zoom = browser.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        tf.scale = Vec3::splat(1.0 / safe_zoom);
    }
}

// ---------------------------------------------------------------------------
// Keyboard shortcuts for grid
// ---------------------------------------------------------------------------

/// All divisors of `dim` that are >= 8.
fn valid_cell_sizes(dim: u32) -> Vec<u32> {
    (8..=dim).filter(|&s| dim % s == 0).collect()
}

fn prev_valid_size(valid: &[u32], current: u32, dim: u32) -> u32 {
    if valid.is_empty() {
        return (8..current).rev().find(|&d| dim % d == 0).unwrap_or(current);
    }
    valid.iter().copied().rev().find(|&s| s < current).unwrap_or(current)
}

fn next_valid_size(valid: &[u32], current: u32, dim: u32) -> u32 {
    if valid.is_empty() {
        return ((current + 1)..=dim).find(|&d| d >= 8 && dim % d == 0).unwrap_or(current);
    }
    valid.iter().copied().find(|&s| s > current).unwrap_or(current)
}

pub fn grid_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    current: Res<CurrentImage>,
    mut browser: ResMut<BrowserState>,
) {
    // G — toggle grid
    if keyboard.just_pressed(KeyCode::KeyG) {
        browser.grid_visible = !browser.grid_visible;
    }

    if current.width == 0 || current.height == 0 {
        return;
    }

    // Initialize cell size if needed
    if browser.cell_w == 0 {
        browser.cell_w = current.width;
    }
    if browser.cell_h == 0 {
        browser.cell_h = current.height;
    }

    let valid_w = valid_cell_sizes(current.width);
    let valid_h = valid_cell_sizes(current.height);

    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let adjust_w = ctrl || !shift;
    let adjust_h = shift || !ctrl;

    // Minus — smaller cells (more divisions)
    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        if adjust_w {
            browser.cell_w = prev_valid_size(&valid_w, browser.cell_w, current.width);
        }
        if adjust_h {
            browser.cell_h = prev_valid_size(&valid_h, browser.cell_h, current.height);
        }
    }

    // Plus — larger cells (fewer divisions)
    if keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd) {
        if adjust_w {
            browser.cell_w = next_valid_size(&valid_w, browser.cell_w, current.width);
        }
        if adjust_h {
            browser.cell_h = next_valid_size(&valid_h, browser.cell_h, current.height);
        }
    }
}

// ---------------------------------------------------------------------------
// Grid overlay
// ---------------------------------------------------------------------------

pub fn draw_grid(
    browser: Res<BrowserState>,
    current: Res<CurrentImage>,
    mut gizmos: Gizmos,
) {
    if !browser.grid_visible {
        return;
    }

    let cw = browser.cell_w as f32;
    let ch = browser.cell_h as f32;
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
    browser: Res<BrowserState>,
    current: Res<CurrentImage>,
    mut images: ResMut<Assets<Image>>,
    existing_tiles: Query<Entity, With<TileSprite>>,
    preview_sprite: Query<&Sprite, With<PreviewSprite>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    // Only rebuild when state changes
    if !browser.is_changed() && !current.is_changed() {
        return;
    }

    // Despawn old tiles
    for entity in &existing_tiles {
        commands.entity(entity).despawn();
    }

    if !browser.tile_preview {
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

    // Auto-calculate tile count from visible viewport area
    let safe_zoom = browser.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    let world_w = window.width() / safe_zoom;
    let world_h = window.height() / safe_zoom;
    // Extra +2 so tiles cover edges during panning
    let cols = ((world_w / w).ceil() as i32 + 2).max(3);
    let rows = ((world_h / h).ceil() as i32 + 2).max(3);

    // Inset 0.1px on the texture rect to prevent sub-pixel gridline bleeding
    let inset = 0.1;
    let rect = bevy::math::Rect::new(inset, inset, w - inset, h - inset);

    // Center tile is the main sprite at (0,0); spawn surrounding tiles
    for row in 0..rows {
        for col in 0..cols {
            if row == rows / 2 && col == cols / 2 {
                continue; // skip center — that's the main preview sprite
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
    browser: Res<BrowserState>,
    current: Res<CurrentImage>,
    mut windows: Query<&mut Window>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };

    let zoom_pct = (browser.zoom * 100.0).round() as i32;

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
