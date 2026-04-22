use base64::Engine as _;
use serde::Serialize;
use snapforge_core::{build_capture_filename, detect_image_format, CaptureMetadata};
use tauri::{AppHandle, Manager as _, State};

use crate::clipboard;
use crate::error::CommandError;
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfoResponse {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfoResponse {
    pub id: u32,
    pub title: String,
    pub app_name: String,
    pub is_on_screen: bool,
}

#[tauri::command]
pub fn list_displays(state: State<'_, AppState>) -> Result<Vec<DisplayInfoResponse>, CommandError> {
    tracing::debug!("Listing displays");
    let displays = state.capture_engine.list_displays()?;
    Ok(displays
        .into_iter()
        .map(|d| DisplayInfoResponse {
            id: d.id,
            name: d.name,
            width: d.width,
            height: d.height,
            scale_factor: d.scale_factor,
        })
        .collect())
}

#[tauri::command]
pub fn list_windows(state: State<'_, AppState>) -> Result<Vec<WindowInfoResponse>, CommandError> {
    tracing::debug!("Listing windows");
    let windows = state.capture_engine.list_windows()?;
    Ok(windows
        .into_iter()
        .map(|w| WindowInfoResponse {
            id: w.id,
            title: w.title,
            app_name: w.app_name,
            is_on_screen: w.is_on_screen,
        })
        .collect())
}

/// Fetch the latest capture metadata (used by preview window on mount)
#[tauri::command]
pub fn get_latest_capture(state: State<'_, AppState>) -> Option<CaptureMetadata> {
    state
        .latest_capture
        .read()
        .ok()
        .and_then(|guard| guard.clone())
}

#[tauri::command]
pub fn check_permission() -> bool {
    tracing::info!("Checking screen recording permission");
    snapforge_capture::has_permission()
}

#[tauri::command]
pub fn request_permission() -> bool {
    tracing::info!("Requesting screen recording permission");
    snapforge_capture::request_permission()
}

/// Re-copy an image to the clipboard from its base64-encoded PNG data.
#[tauri::command]
pub fn copy_image_from_base64(image_base64: String) -> Result<(), CommandError> {
    let png_bytes = base64::engine::general_purpose::STANDARD
        .decode(&image_base64)
        .map_err(|e| CommandError {
            message: format!("Failed to decode base64: {e}"),
            code: "decode_error".to_string(),
        })?;

    clipboard::copy_encoded_to_clipboard(&png_bytes).map_err(|e| CommandError {
        message: e,
        code: "clipboard_error".to_string(),
    })?;

    tracing::info!("Image re-copied to clipboard from base64");
    Ok(())
}

/// Resolve a save location path, expanding `~` to HOME.
/// Falls back to `~/Desktop` if the resolved path doesn't exist.
fn resolve_save_dir(save_location: &str) -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let expanded = if save_location.starts_with("~/") {
        save_location.replacen("~", &home, 1)
    } else if save_location == "~" {
        home.clone()
    } else {
        save_location.to_string()
    };
    let path = std::path::PathBuf::from(expanded);
    if path.is_dir() {
        path
    } else {
        std::path::PathBuf::from(home).join("Desktop")
    }
}

/// Save a captured image with the correct file extension.
/// The optimized image may be WebP (Claude/Gemini) or PNG (GPT-4o),
/// so we detect the format from the magic bytes.
/// Save location is read from settings (defaults to ~/Desktop).
#[tauri::command]
pub fn save_capture_to_desktop(
    app: AppHandle,
    image_base64: String,
) -> Result<String, CommandError> {
    let image_bytes = base64::engine::general_purpose::STANDARD
        .decode(&image_base64)
        .map_err(|e| CommandError {
            message: format!("Failed to decode base64: {e}"),
            code: "decode_error".to_string(),
        })?;

    // Detect actual format from magic bytes
    let ext = detect_image_format(&image_bytes);

    let settings = crate::settings::load_settings(&app);
    let save_dir = resolve_save_dir(&settings.save_location);

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = build_capture_filename(ext, &timestamp.to_string());
    let path = save_dir.join(&filename);

    std::fs::write(&path, &image_bytes).map_err(|e| CommandError {
        message: format!("Failed to write file: {e}"),
        code: "io_error".to_string(),
    })?;

    tracing::info!(path = %path.display(), "Capture saved");
    Ok(filename)
}

/// Save a GIF recording.
/// Save location is read from settings (defaults to ~/Desktop).
#[tauri::command]
pub fn save_gif_to_desktop(app: AppHandle, gif_base64: String) -> Result<String, CommandError> {
    let gif_bytes = base64::engine::general_purpose::STANDARD
        .decode(&gif_base64)
        .map_err(|e| CommandError {
            message: format!("Failed to decode base64: {e}"),
            code: "decode_error".to_string(),
        })?;

    let settings = crate::settings::load_settings(&app);
    let save_dir = resolve_save_dir(&settings.save_location);

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("Gaze_{timestamp}.gif");
    let path = save_dir.join(&filename);

    std::fs::write(&path, &gif_bytes).map_err(|e| CommandError {
        message: format!("Failed to write file: {e}"),
        code: "io_error".to_string(),
    })?;

    tracing::info!(path = %path.display(), "GIF saved");
    Ok(filename)
}

/// Close a specific preview window by its label (called from inline JS).
#[tauri::command]
pub fn close_preview_by_label(app: AppHandle, label: String) {
    crate::preview_window::close_preview_by_label(&app, &label);
}

/// Resize and reposition a preview window (used for expand/collapse).
#[tauri::command]
pub fn resize_preview_window(
    app: AppHandle,
    label: String,
    width: f64,
    height: f64,
    x: f64,
    y: f64,
) -> Result<(), CommandError> {
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(width, height)));
        let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)));
        Ok(())
    } else {
        Err(CommandError {
            message: format!("Window not found: {label}"),
            code: "window_not_found".to_string(),
        })
    }
}

/// Get the primary monitor's logical size (used by JS to calculate expand dimensions).
#[tauri::command]
pub fn get_screen_size(app: AppHandle, label: String) -> Result<(f64, f64), CommandError> {
    if let Some(window) = app.get_webview_window(&label) {
        if let Ok(Some(monitor)) = window.primary_monitor() {
            let size = monitor.size();
            let scale = monitor.scale_factor();
            return Ok((size.width as f64 / scale, size.height as f64 / scale));
        }
    }
    Err(CommandError {
        message: "Could not determine screen size".to_string(),
        code: "screen_error".to_string(),
    })
}

/// Reposition a single preview window back to its correct stack slot.
#[tauri::command]
pub fn reposition_preview(app: AppHandle, label: String) {
    crate::preview_window::reposition_single(&app, &label);
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- detect_image_format tests ---

    #[test]
    fn detect_png_format() {
        let png_header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(detect_image_format(&png_header), "png");
    }

    #[test]
    fn detect_webp_format() {
        let mut webp_header = Vec::new();
        webp_header.extend_from_slice(b"RIFF");
        webp_header.extend_from_slice(&[0x00; 4]); // size placeholder
        webp_header.extend_from_slice(b"WEBP");
        assert_eq!(detect_image_format(&webp_header), "webp");
    }

    #[test]
    fn detect_jpeg_format() {
        let jpeg_header = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(detect_image_format(&jpeg_header), "jpeg");
    }

    #[test]
    fn detect_riff_but_not_webp() {
        // RIFF header but not WEBP (e.g., WAV file)
        let mut wav_header = Vec::new();
        wav_header.extend_from_slice(b"RIFF");
        wav_header.extend_from_slice(&[0x00; 4]);
        wav_header.extend_from_slice(b"WAVE");
        assert_eq!(detect_image_format(&wav_header), "png"); // fallback
    }

    #[test]
    fn detect_unknown_format_fallback() {
        assert_eq!(detect_image_format(&[0x00, 0x01, 0x02]), "png");
    }

    #[test]
    fn detect_empty_bytes_fallback() {
        assert_eq!(detect_image_format(&[]), "png");
    }

    #[test]
    fn detect_short_riff_fallback() {
        // RIFF but too short to check WEBP
        assert_eq!(detect_image_format(b"RIFF1234"), "png");
    }

    // --- build_capture_filename tests ---

    #[test]
    fn build_filename_png() {
        let name = build_capture_filename("png", "20260327_120000");
        assert_eq!(name, "Gaze_20260327_120000.png");
    }

    #[test]
    fn build_filename_webp() {
        let name = build_capture_filename("webp", "20260327_120000");
        assert_eq!(name, "Gaze_20260327_120000.webp");
    }

    // --- DisplayInfoResponse / WindowInfoResponse tests ---

    #[test]
    fn display_info_response_serializes() {
        let resp = DisplayInfoResponse {
            id: 1,
            name: "Main".into(),
            width: 2560,
            height: 1440,
            scale_factor: 2.0,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"name\":\"Main\""));
        assert!(json.contains("\"width\":2560"));
    }

    // --- resolve_save_dir tests ---

    #[test]
    fn resolve_save_dir_tilde_desktop() {
        let home = std::env::var("HOME").unwrap();
        let result = resolve_save_dir("~/Desktop");
        let expected = std::path::PathBuf::from(format!("{home}/Desktop"));
        assert_eq!(result, expected);
    }

    #[test]
    fn resolve_save_dir_absolute_tmp() {
        let result = resolve_save_dir("/tmp");
        assert_eq!(result, std::path::PathBuf::from("/tmp"));
    }

    #[test]
    fn resolve_save_dir_nonexistent_falls_back() {
        let home = std::env::var("HOME").unwrap();
        let result = resolve_save_dir("~/nonexistent_dir_xyz_12345");
        let expected = std::path::PathBuf::from(format!("{home}/Desktop"));
        assert_eq!(result, expected);
    }

    #[test]
    fn resolve_save_dir_tilde_only() {
        let home = std::env::var("HOME").unwrap();
        let result = resolve_save_dir("~");
        assert_eq!(result, std::path::PathBuf::from(&home));
    }

    #[test]
    fn resolve_save_dir_absolute_nonexistent_falls_back() {
        let home = std::env::var("HOME").unwrap();
        let result = resolve_save_dir("/nonexistent_path_abc_999");
        let expected = std::path::PathBuf::from(format!("{home}/Desktop"));
        assert_eq!(result, expected);
    }

    #[test]
    fn resolve_save_dir_path_with_spaces() {
        // A nonexistent path with spaces should fall back
        let home = std::env::var("HOME").unwrap();
        let result = resolve_save_dir("~/My Screenshots/subfolder");
        let expected = std::path::PathBuf::from(format!("{home}/Desktop"));
        assert_eq!(result, expected);
    }

    #[test]
    fn window_info_response_serializes() {
        let resp = WindowInfoResponse {
            id: 42,
            title: "Test Window".into(),
            app_name: "Safari".into(),
            is_on_screen: true,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"id\":42"));
        assert!(json.contains("\"isOnScreen\":true"));
    }
}
