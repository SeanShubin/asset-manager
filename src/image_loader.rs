//! Image loading from disk files and zip entries.

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::io::Read;
use std::path::Path;

use crate::data::FileRef;

/// Recognized image extensions.
pub fn is_image_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".bmp")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
}

/// Load raw RGBA bytes from a FileRef.
pub fn load_rgba(file_ref: &FileRef) -> Result<image::RgbaImage, String> {
    match file_ref {
        FileRef::Disk(path) => load_rgba_from_disk(path),
        FileRef::ZipEntry { zip_path, entry } => load_rgba_from_zip(zip_path, entry),
        FileRef::NestedZipEntry {
            outer_zip,
            inner_entry,
            entry,
        } => load_rgba_from_nested_zip(outer_zip, inner_entry, entry),
    }
}

fn load_rgba_from_disk(path: &Path) -> Result<image::RgbaImage, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Read error: {e}"))?;
    let img = image::load_from_memory(&bytes).map_err(|e| format!("Decode error: {e}"))?;
    Ok(img.to_rgba8())
}

fn load_rgba_from_zip(zip_path: &Path, entry_name: &str) -> Result<image::RgbaImage, String> {
    let buf = read_zip_entry_bytes(zip_path, entry_name)?;
    let img = image::load_from_memory(&buf).map_err(|e| format!("Decode error: {e}"))?;
    Ok(img.to_rgba8())
}

fn load_rgba_from_nested_zip(
    outer_zip: &Path,
    inner_entry: &str,
    entry_name: &str,
) -> Result<image::RgbaImage, String> {
    let inner_bytes = read_zip_entry_bytes(outer_zip, inner_entry)?;
    let cursor = std::io::Cursor::new(inner_bytes);
    let mut inner_archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("Invalid inner zip: {e}"))?;
    let mut entry = inner_archive
        .by_name(entry_name)
        .map_err(|e| format!("Inner entry not found: {e}"))?;

    let mut buf = Vec::new();
    entry
        .read_to_end(&mut buf)
        .map_err(|e| format!("Read inner entry error: {e}"))?;

    let img = image::load_from_memory(&buf).map_err(|e| format!("Decode error: {e}"))?;
    Ok(img.to_rgba8())
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
