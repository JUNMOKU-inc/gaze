use arboard::{Clipboard, ImageData};
use image::ImageReader;
use std::borrow::Cow;
use std::io::Cursor;

/// Copy encoded image bytes (PNG/WebP) to the system clipboard.
///
/// Decodes the image to extract RGBA data that `arboard` requires.
/// Prefer `copy_rgba_to_clipboard` when raw RGBA is already available.
pub fn copy_encoded_to_clipboard(encoded_data: &[u8]) -> Result<(), String> {
    let img = ImageReader::new(Cursor::new(encoded_data))
        .with_guessed_format()
        .map_err(|e| format!("Failed to guess image format: {e}"))?
        .decode()
        .map_err(|e| format!("Failed to decode image for clipboard: {e}"))?;

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    copy_rgba_to_clipboard(&rgba.into_raw(), width, height)
}

/// Copy raw RGBA pixel data to the system clipboard.
pub fn copy_rgba_to_clipboard(rgba_data: &[u8], width: u32, height: u32) -> Result<(), String> {
    let image_data = ImageData {
        width: width as usize,
        height: height as usize,
        bytes: Cow::Borrowed(rgba_data),
    };

    let mut clipboard = Clipboard::new().map_err(|e| format!("Failed to open clipboard: {e}"))?;

    clipboard
        .set_image(image_data)
        .map_err(|e| format!("Failed to copy image to clipboard: {e}"))?;

    tracing::info!(width, height, "Image copied to clipboard");
    Ok(())
}
