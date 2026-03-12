//! Viewport systems: pan/zoom, camera, grid overlay, tile preview.

use bevy::camera::visibility::NoFrustumCulling;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::grid;
use crate::image_loader;
use crate::resources::*;

const GRID_COLOR: Color = Color::srgba(1.0, 1.0, 0.0, 0.4);
const CELL_HIGHLIGHT_COLOR: Color = Color::srgba(0.0, 1.0, 1.0, 0.6);
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
    grid_state: Res<GridState>,
    anim: Res<AnimationPreview>,
    mut images: ResMut<Assets<Image>>,
    existing: Query<Entity, With<PreviewSprite>>,
    tiles: Query<Entity, With<TileSprite>>,
) {
    if !current.is_changed() && !anim.is_changed() {
        return;
    }

    for entity in &existing {
        commands.entity(entity).despawn();
    }
    for entity in &tiles {
        commands.entity(entity).despawn();
    }

    let Some(ref rgba) = current.rgba else {
        return;
    };

    if anim.playing {
        let cw = grid_state.cell_w;
        if cw > 0 {
            let cols = current.width / cw;
            if cols >= 3 && cols % 3 == 0 {
                let frame_index = WALK_CYCLE[anim.cycle_pos];
                let frame_col = WALK_FRAME_COL[frame_index];
                let expanded = build_expanded_image(rgba, cw, frame_col);
                let handle = image_loader::rgba_to_bevy_handle(&expanded, &mut images);
                commands.spawn((PreviewSprite, Sprite::from_image(handle), NoFrustumCulling));
                return;
            }
        }
    }

    let handle = image_loader::rgba_to_bevy_handle(rgba, &mut images);
    commands.spawn((PreviewSprite, Sprite::from_image(handle), NoFrustumCulling));
}

/// Build an expanded image: each 3-col block gets a 4th column showing the
/// current animation frame. Result is 4/3 the original width.
fn build_expanded_image(
    rgba: &image::RgbaImage,
    cell_w: u32,
    frame_col_offset: u32,
) -> image::RgbaImage {
    let orig_w = rgba.width();
    let orig_h = rgba.height();
    let cols = orig_w / cell_w;
    let num_blocks = cols / 3;
    let new_w = num_blocks * 4 * cell_w;

    let mut out = image::RgbaImage::new(new_w, orig_h);

    for block in 0..num_blocks {
        for local_col in 0..3u32 {
            let src_x = (block * 3 + local_col) * cell_w;
            let dst_x = (block * 4 + local_col) * cell_w;
            for y in 0..orig_h {
                for x in 0..cell_w {
                    out.put_pixel(dst_x + x, y, *rgba.get_pixel(src_x + x, y));
                }
            }
        }
        // 4th column: animated frame
        let src_x = (block * 3 + frame_col_offset) * cell_w;
        let dst_x = (block * 4 + 3) * cell_w;
        for y in 0..orig_h {
            for x in 0..cell_w {
                out.put_pixel(dst_x + x, y, *rgba.get_pixel(src_x + x, y));
            }
        }
    }

    out
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
            camera.drag_distance = 0.0;
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
            camera.drag_distance += delta.length();
            let zoom = camera.zoom;
            camera.pan += Vec2::new(delta.x, -delta.y) / zoom;
        }
        camera.last_cursor = cursor;
    }
}

// ---------------------------------------------------------------------------
// Cell click detection
// ---------------------------------------------------------------------------

const CLICK_THRESHOLD: f32 = 3.0;

pub fn cell_click(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Res<CameraState>,
    grid_state: Res<GridState>,
    current: Res<CurrentImage>,
    pointer: Res<EguiPointerState>,
    anim: Res<AnimationPreview>,
    mut cell_selection: ResMut<CellSelection>,
) {
    if !mouse_buttons.just_released(MouseButton::Left) {
        return;
    }

    // Only select cells when grid is visible and click wasn't a drag
    if !grid_state.visible || camera.drag_distance > CLICK_THRESHOLD || pointer.over_ui {
        return;
    }

    // Don't select cells during animation (expanded image has different layout)
    if anim.playing {
        return;
    }

    let cw = grid_state.cell_w;
    let ch = grid_state.cell_h;
    if cw == 0 || ch == 0 || current.width == 0 || current.height == 0 {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    // Convert screen position to world position
    // Bevy renders to the full window; camera center is at window center
    let screen_center_x = window.width() / 2.0;
    let screen_center_y = window.height() / 2.0;
    let safe_zoom = camera.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    let world_x = (cursor_pos.x - screen_center_x) / safe_zoom - camera.pan.x;
    let world_y = -((cursor_pos.y - screen_center_y) / safe_zoom + camera.pan.y);

    // Image coords: origin at top-left of image
    let img_w = current.width as f32;
    let img_h = current.height as f32;
    let img_x = world_x + img_w / 2.0;
    let img_y = img_h / 2.0 - world_y;

    if img_x < 0.0 || img_y < 0.0 || img_x >= img_w || img_y >= img_h {
        // Clicked outside the image — deselect cell
        cell_selection.selected = None;
        return;
    }

    let col = (img_x / cw as f32) as u32;
    let row = (img_y / ch as f32) as u32;

    cell_selection.selected = Some((col, row));
}

// ---------------------------------------------------------------------------
// Auto-fit zoom
// ---------------------------------------------------------------------------

pub fn auto_fit_zoom(
    mut camera_state: ResMut<CameraState>,
    current: Res<CurrentImage>,
    anim: Res<AnimationPreview>,
    grid_state: Res<GridState>,
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

    // When animation is playing the displayed image is 4/3 wider (expanded)
    let img_w = if anim.playing && grid_state.cell_w > 0 {
        let cols = current.width / grid_state.cell_w;
        if cols >= 3 && cols % 3 == 0 {
            current.width as f32 * 4.0 / 3.0
        } else {
            current.width as f32
        }
    } else {
        current.width as f32
    };
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
// Animation preview tick
// ---------------------------------------------------------------------------

pub fn animation_tick(
    time: Res<Time>,
    mut anim: ResMut<AnimationPreview>,
) {
    if !anim.playing {
        return;
    }

    let new_timer = anim.timer + time.delta_secs();
    if new_timer >= WALK_FRAME_DURATION {
        // Cycle advanced — normal mutation triggers is_changed()
        anim.timer = new_timer - WALK_FRAME_DURATION;
        anim.cycle_pos = (anim.cycle_pos + 1) % WALK_CYCLE.len();
    } else {
        // Just ticking — bypass change detection to avoid rebuilding sprite every frame
        anim.bypass_change_detection().timer = new_timer;
    }
}

// ---------------------------------------------------------------------------
// Grid overlay
// ---------------------------------------------------------------------------

pub fn draw_grid(
    grid_state: Res<GridState>,
    current: Res<CurrentImage>,
    cell_selection: Res<CellSelection>,
    anim: Res<AnimationPreview>,
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

    let orig_w = current.width as f32;
    let h = current.height as f32;
    if orig_w == 0.0 || h == 0.0 {
        return;
    }

    // When animating, the displayed image is 4/3 wider (expanded)
    let w = if anim.playing {
        let cols = current.width / grid_state.cell_w;
        if cols >= 3 && cols % 3 == 0 {
            orig_w * 4.0 / 3.0
        } else {
            orig_w
        }
    } else {
        orig_w
    };

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

    // Highlight selected cell (no highlight during animation — all blocks animate)
    let highlight_cell = if anim.playing {
        None
    } else {
        cell_selection.selected
    };

    if let Some((col, row)) = highlight_cell {
        let x0 = left + col as f32 * cw;
        let y0 = top - row as f32 * ch;
        let x1 = x0 + cw;
        let y1 = y0 - ch;
        gizmos.line_2d(Vec2::new(x0, y0), Vec2::new(x1, y0), CELL_HIGHLIGHT_COLOR);
        gizmos.line_2d(Vec2::new(x1, y0), Vec2::new(x1, y1), CELL_HIGHLIGHT_COLOR);
        gizmos.line_2d(Vec2::new(x1, y1), Vec2::new(x0, y1), CELL_HIGHLIGHT_COLOR);
        gizmos.line_2d(Vec2::new(x0, y1), Vec2::new(x0, y0), CELL_HIGHLIGHT_COLOR);
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
    grid_state: Res<GridState>,
    cell_selection: Res<CellSelection>,
    anim: Res<AnimationPreview>,
    mut images: ResMut<Assets<Image>>,
    existing_tiles: Query<Entity, With<TileSprite>>,
    preview_sprite: Query<&Sprite, With<PreviewSprite>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if !camera_state.is_changed()
        && !tile_state.is_changed()
        && !current.is_changed()
        && !cell_selection.is_changed()
        && !anim.is_changed()
    {
        return;
    }

    for entity in &existing_tiles {
        commands.entity(entity).despawn();
    }

    if !tile_state.enabled || anim.playing {
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

    // Determine tile source rect and size
    let cw = grid_state.cell_w as f32;
    let ch = grid_state.cell_h as f32;
    let has_grid = cw > 0.0 && ch > 0.0;

    let (tile_rect, tile_w, tile_h) = if let Some((col, row)) = cell_selection.selected {
        if has_grid {
            let x = col as f32 * cw;
            let y = row as f32 * ch;
            (
                bevy::math::Rect::new(x + TILE_INSET, y + TILE_INSET, x + cw - TILE_INSET, y + ch - TILE_INSET),
                cw,
                ch,
            )
        } else {
            (bevy::math::Rect::new(TILE_INSET, TILE_INSET, w - TILE_INSET, h - TILE_INSET), w, h)
        }
    } else {
        (bevy::math::Rect::new(TILE_INSET, TILE_INSET, w - TILE_INSET, h - TILE_INSET), w, h)
    };

    // Use visible viewport area (window minus panels) for tile count
    let safe_zoom = camera_state.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    let view_w = (window.width() - LEFT_PANEL_WIDTH - RIGHT_PANEL_WIDTH).max(1.0);
    let view_h = (window.height() - STATUS_BAR_HEIGHT).max(1.0);
    let world_w = view_w / safe_zoom;
    let world_h = view_h / safe_zoom;
    let cols = ((world_w / tile_w).ceil() as i32 + 2).max(3);
    let rows = ((world_h / tile_h).ceil() as i32 + 2).max(3);

    for row in 0..rows {
        for col in 0..cols {
            if row == rows / 2 && col == cols / 2 {
                continue;
            }
            let offset_x = (col - cols / 2) as f32 * tile_w;
            let offset_y = -(row - rows / 2) as f32 * tile_h;
            commands.spawn((
                TileSprite,
                Sprite {
                    image: handle.clone(),
                    rect: Some(tile_rect),
                    custom_size: Some(Vec2::new(tile_w, tile_h)),
                    ..default()
                },
                Transform::from_xyz(offset_x, offset_y, -0.1),
                NoFrustumCulling,
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Clear cell selection on file change
// ---------------------------------------------------------------------------

pub fn clear_cell_on_file_change(
    selection: Res<TreeSelection>,
    mut cell_selection: ResMut<CellSelection>,
    mut anim: ResMut<AnimationPreview>,
) {
    let current_key = selection
        .selected_path
        .as_ref()
        .map(|f| f.to_string_repr())
        .unwrap_or_default();
    if current_key != cell_selection.file_key {
        cell_selection.file_key = current_key;
        cell_selection.selected = None;
        anim.playing = false;
        anim.cycle_pos = 0;
        anim.timer = 0.0;
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
