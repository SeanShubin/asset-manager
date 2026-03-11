# Asset Manager

Interactive desktop tool for organizing, previewing, and exporting game assets. Built with Bevy and egui.

## Features

- **File browser** — Browse drives, directories, and files in a tree view, including files nested inside zip archives
- **Image preview** — View image files with pan/zoom in a dedicated viewport
- **Grid overlay** — Adjust grid cell size to define sprite sheet layouts, approve and persist grid definitions
- **Tiling preview** — See what an image looks like tiled/repeated in a grid
- **Directory hierarchy** — Designate directories as asset roots, creator roots, and asset pack roots to organize your asset library
- **Bundles** — Create named bundles, assign files to bundles at specific relative paths, and export bundles to a target directory
- **Metadata persistence** — All metadata stored in a single `asset_manager.toml` file in your chosen data directory

## Usage

```
cargo run
cargo run -- D:/my-assets
```

The optional argument specifies the data directory where `asset_manager.toml` is stored. Defaults to the current directory.

## Controls

- **Scroll wheel** — Zoom in/out
- **Left-click drag** — Pan
- **Home / R** — Reset pan/zoom to fit image
