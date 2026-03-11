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
- `grid.rs` — Shared grid size calculation helpers
- `export.rs` — Bundle export logic

## Phases

### Phase 1: Skeleton + File Tree + Image Preview
- [x] Create repo, Cargo.toml, .gitignore
- [x] `data.rs` — FileRef, ManagerData, persistence types, load/save
- [x] `resources.rs` — DataDir, ManagerState, TreeSelection, CameraState, GridState, TileState, CurrentImage, UiState
- [x] `image_loader.rs` — load image from disk/zip, convert to Bevy Image
- [x] `viewport.rs` — pan/zoom, apply camera, auto-fit zoom, draw grid, tile preview sprites
- [x] `tree_panel.rs` — left panel with filesystem tree, click to select, zip browsing
- [x] `detail_panel.rs` — right panel with tab bar, Browse tab with file info + hierarchy buttons
- [x] `status_bar.rs` — bottom panel with status message, zoom %, file dimensions
- [x] `main.rs` — CLI arg parsing, App setup, register all systems

### Phase 2: Grid Controls
- [x] Grid tab: interactive cell size +/- buttons
- [x] `valid_cell_sizes` helper (divisors >= 8)
- [x] Apply Grid / Clear Grid buttons with persistence
- [x] Snap zoom toggle
- [x] Tile preview with auto-calculated tile count
- [x] Auto-fit zoom on image load
- [x] Keyboard shortcuts: G toggle grid, +/- cell size, Ctrl/Shift modifiers
- [x] Saved grid indicator in Grid tab

### Phase 3: Hierarchy + Navigation
- [x] Browse tab: Mark/Unmark Asset Root, Creator Root, Asset Pack Root
- [x] Role indicators in tree: [AR] green, [CR] blue, [PR] purple
- [x] Bookmarks for quick-jump to designated roots
- [x] Left/right arrow file navigation
- [x] Tags (toggleable: 4dir-walk, 8dir-walk, etc.)
- [x] Mouse wheel ownership (egui panels vs viewport)
- [ ] Hierarchy validation feedback in status bar

### Phase 4: Zip Support
- [x] Detect `.zip` files in tree, render as expandable directories
- [x] Build virtual directory tree from zip entries
- [x] Load image from zip entry for preview
- [x] Nested zip support (zips inside zips)

### Phase 5: Bundles + Export
- [ ] Bundles tab: create/delete bundles with name field
- [ ] Export path field per bundle
- [ ] "Add selected file to bundle" with destination path input
- [ ] File list display per bundle with remove button
- [ ] Export: copy disk files, extract zip entries to export path
- [ ] Export metadata: write manifest TOML alongside exported files

### Phase 6: Polish
- [x] Tree regex filter (case-insensitive, filters leaf files by name)
- [ ] Animated sprite preview (RPG Maker default frame timings)
- [x] Image info panel (color type, has-alpha, unique color count, file size)
- [x] Keyboard shortcuts help overlay (F1)
