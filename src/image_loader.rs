//! Image loading from disk files and zip entries.

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::HashSet;
use std::io::Read;
use std::path::Path;

use crate::data::FileRef;
use crate::resources::ImageInfo;

/// Recognized image extensions.
pub fn is_image_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".bmp")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
}

/// Result of loading an image: RGBA pixels plus metadata.
pub struct LoadedImage {
    pub rgba: image::RgbaImage,
    pub info: ImageInfo,
}

/// Load image + metadata from a FileRef.
pub fn load_image(file_ref: &FileRef) -> Result<LoadedImage, String> {
    let bytes = load_raw_bytes(file_ref)?;
    let file_size = bytes.len() as u64;
    let img = image::load_from_memory(&bytes).map_err(|e| format!("Decode error: {e}"))?;
    let color_type = format!("{:?}", img.color());
    let rgba = img.to_rgba8();
    let (has_alpha, unique_colors) = analyze_rgba(&rgba);

    Ok(LoadedImage {
        rgba,
        info: ImageInfo {
            file_size,
            color_type,
            has_alpha,
            unique_colors,
        },
    })
}

fn load_raw_bytes(file_ref: &FileRef) -> Result<Vec<u8>, String> {
    match file_ref {
        FileRef::Disk(path) => {
            std::fs::read(path).map_err(|e| format!("Read error: {e}"))
        }
        FileRef::ZipEntry { zip_path, entry } => {
            read_zip_entry_bytes(zip_path, entry)
        }
        FileRef::NestedZipEntry {
            outer_zip,
            inner_entry,
            entry,
        } => {
            let inner_bytes = read_zip_entry_bytes(outer_zip, inner_entry)?;
            let cursor = std::io::Cursor::new(inner_bytes);
            let mut inner_archive =
                zip::ZipArchive::new(cursor).map_err(|e| format!("Invalid inner zip: {e}"))?;
            let mut ze = inner_archive
                .by_name(entry)
                .map_err(|e| format!("Inner entry not found: {e}"))?;

            let mut buf = Vec::new();
            ze.read_to_end(&mut buf)
                .map_err(|e| format!("Read inner entry error: {e}"))?;
            Ok(buf)
        }
    }
}

fn analyze_rgba(rgba: &image::RgbaImage) -> (bool, usize) {
    let mut has_alpha = false;
    let mut colors: HashSet<[u8; 4]> = HashSet::new();

    for pixel in rgba.pixels() {
        if pixel.0[3] < 255 {
            has_alpha = true;
        }
        colors.insert(pixel.0);
    }

    (has_alpha, colors.len())
}

/// Extract raw bytes of a zip entry from a disk zip file.
pub fn read_zip_entry_bytes(zip_path: &Path, entry_name: &str) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(zip_path)
        .map_err(|e| format!("Cannot open zip {}: {e}", zip_path.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Invalid zip: {e}"))?;
    let mut entry = archive
        .by_name(entry_name)
        .map_err(|e| format!("Entry not found: {e}"))?;

    let mut buf = Vec::new();
    entry
        .read_to_end(&mut buf)
        .map_err(|e| format!("Read entry error: {e}"))?;
    Ok(buf)
}

/// Convert an RgbaImage to a Bevy Image handle.
pub fn rgba_to_bevy_handle(rgba: &image::RgbaImage, images: &mut Assets<Image>) -> Handle<Image> {
    let bevy_image = Image::new(
        Extent3d {
            width: rgba.width(),
            height: rgba.height(),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba.as_raw().clone(),
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD | bevy::asset::RenderAssetUsages::MAIN_WORLD,
    );
    images.add(bevy_image)
}
