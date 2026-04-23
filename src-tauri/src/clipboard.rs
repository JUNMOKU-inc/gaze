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

pub fn copy_text_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| format!("Failed to open clipboard: {e}"))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|e| format!("Failed to copy text to clipboard: {e}"))?;
    Ok(())
}

pub fn copy_png_and_text_to_clipboard(png_data: &[u8], text: &str) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        copy_png_and_text_to_clipboard_macos(png_data, text).map(|_| true)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = png_data;
        let _ = text;
        Ok(false)
    }
}

#[cfg(target_os = "macos")]
fn copy_png_and_text_to_clipboard_macos(png_data: &[u8], text: &str) -> Result<(), String> {
    use objc2::runtime::ProtocolObject;
    use objc2_app_kit::{
        NSPasteboard, NSPasteboardItem, NSPasteboardTypePNG, NSPasteboardTypeString,
        NSPasteboardWriting,
    };
    use objc2_foundation::{NSArray, NSData, NSString};

    let pasteboard = unsafe { NSPasteboard::generalPasteboard() };
    let _ = unsafe { pasteboard.clearContents() };

    let image_item = unsafe { NSPasteboardItem::new() };
    let image_data = NSData::with_bytes(png_data);
    let image_ok = unsafe { image_item.setData_forType(&image_data, NSPasteboardTypePNG) };
    if !image_ok {
        return Err("Failed to set PNG data on pasteboard item".to_string());
    }

    let text_item = unsafe { NSPasteboardItem::new() };
    let text_ns = NSString::from_str(text);
    let text_ok = unsafe { text_item.setString_forType(&text_ns, NSPasteboardTypeString) };
    if !text_ok {
        return Err("Failed to set text data on pasteboard item".to_string());
    }

    let objects = NSArray::from_vec(vec![
        ProtocolObject::<dyn NSPasteboardWriting>::from_retained(image_item),
        ProtocolObject::<dyn NSPasteboardWriting>::from_retained(text_item),
    ]);
    let wrote = unsafe { pasteboard.writeObjects(&objects) };
    if !wrote {
        return Err("Failed to write image and text objects to the pasteboard".to_string());
    }

    tracing::info!("Image + prompt copied to pasteboard");
    Ok(())
}
