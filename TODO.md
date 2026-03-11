# Asset Manager — Implementation Plan

## Architecture

Standalone Bevy 0.18 + bevy_egui app for managing asset metadata.

**UI Layout**: Three egui panels + Bevy viewport
- Left panel: File tree browser (drives, dirs, zips) with hierarchy indicators
- Center: Bevy viewport — image preview, grid overlay, tiling preview
- Right panel: Tabbed detail panel (Browse / Grid / Bundles)
- Bottom: Status bar

**Metadata**: Single `asset_manager.toml` in the data directory.
Zip-internal paths use `//` separator: `D:/foo/pack.zip//sprites/hero.png`

**Hierarchy**: asset root > creator root > asset pack root

## Module Structure

- `main.rs` — App setup, CLI parsing
- `data.rs` — `ManagerData`, `FileRef`, TOML persistence types
- `resources.rs` — Bevy Resources
- `image_loader.rs` — Load images from disk or zip entries
- `tree_panel.rs` — Left panel: file tree with zip browsing, hierarchy indicators
- `detail_panel.rs` — Right panel: Browse/Grid/Bundles tabs
- `status_bar.rs` — Bottom panel
- `viewport.rs` — Pan/zoom, camera, grid gizmos, tile preview systems
- `export.rs` — Bundle export logic

## Phases

### Phase 1: Skeleton + File Tree + Image Preview
- [x] Create repo, Cargo.toml, .gitignore
- [x] `data.rs` — FileRef, ManagerData, persistence types, load/save
- [x] `resources.rs` — DataDir, ManagerState, TreeSelection, BrowserState, CurrentImage, UiState
- [x] `image_loader.rs` — load image from disk/zip, convert to Bevy Image
- [x] `viewport.rs` — pan/zoom, apply camera, auto-fit zoom, draw grid, tile preview sprites
- [x] `tree_panel.rs` — left panel with filesystem tree (CollapsingHeader), click to select, zip browsing
- [x] `detail_panel.rs` — right panel skeleton with tab bar, Browse tab with file info + hierarchy buttons
- [x] `status_bar.rs` — bottom panel with status message, zoom %, file dimensions
- [x] `main.rs` — CLI arg parsing, App setup, register all systems
- [x] Verify it compiles

### Phase 2: Grid Controls
- [ ] Grid tab in detail panel: interactive cell size +/- buttons (not just keyboard)
- [ ] `valid_cell_sizes` helper (divisors >= 8)
- [ ] Apply Grid / Clear Grid buttons with persistence to `grids` table
- [x] Snap zoom toggle (wired in BrowserState + pan_zoom, needs UI toggle)
- [ ] Snap zoom checkbox in Grid tab UI
- [x] Tile preview mode: spawn/manage tile sprites (viewport.rs, needs UI controls)
- [ ] Tile preview checkbox + cols/rows sliders in Grid tab UI
- [x] Auto-fit zoom on image load (viewport.rs)
- [ ] Keyboard shortcuts for grid: G toggle, +/- cell size, Ctrl/Shift modifiers

### Phase 3: Hierarchy Designations
- [x] Browse tab: "Mark as Asset Root" button (persists to `asset_roots`)
- [x] Browse tab: "Mark as Creator Root" button (validates parent is asset root)
- [x] Browse tab: "Mark as Asset Pack Root" button (validates parent is creator root)
- [x] Role indicators in tree: [AR] green, [CR] blue, [PR] purple
- [x] Unmark buttons for each role
- [ ] Hierarchy validation feedback in status bar

### Phase 4: Zip Support
- [x] Detect `.zip` files in tree, render as expandable directories
- [x] Build virtual directory tree from zip entries
- [x] Load image from zip entry for preview (image_loader.rs)
- [ ] Nested zip support (zips inside zips)

### Phase 5: Bundles + Export
- [ ] Bundles tab: create/delete bundles with name field
- [ ] Export path field per bundle
- [ ] "Add selected file to bundle" with destination path input
- [ ] File list display per bundle with remove button
- [ ] Export: copy disk files, extract zip entries to export path
- [ ] Export metadata: write manifest TOML alongside exported files

### Phase 6: Polish
- [x] Drive listing on Windows (tree_panel.rs)
- [x] Support BMP/JPG/GIF in image preview (image_loader.rs uses `image` crate)
- [ ] Keyboard shortcuts (Ctrl+S save, G toggle grid, +/- cell size)
- [x] Status message decay timer (detail_panel.rs)
- [x] Window title with current file, zoom %, grid info (viewport.rs)
