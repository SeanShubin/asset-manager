//! Image loading from disk files and archive entries.

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::HashSet;

use crate::archive;
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

pub fn load_raw_bytes(file_ref: &FileRef) -> Result<Vec<u8>, String> {
    match file_ref {
        FileRef::Disk(path) => std::fs::read(path).map_err(|e| format!("Read error: {e}")),
        FileRef::ZipEntry { zip_path, entry } => archive::read_entry(zip_path, entry),
        FileRef::NestedZipEntry {
            outer_zip,
            inner_entry,
            entry,
        } => {
            let inner_bytes = archive::read_entry(outer_zip, inner_entry)?;
            archive::read_entry_from_bytes(&inner_bytes, inner_entry, entry)
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
