pub use snapforge_core::CaptureMetadata;

use snapforge_core::{process_image_bytes, temp_capture_path};
use std::path::Path;
use std::process::Command;

use crate::clipboard;

/// Execute area capture using macOS native `screencapture -i -s -x` (interactive selection).
///
/// Returns `Ok(None)` if the user cancelled (Escape). When `auto_copy` is true
/// the optimized image is also placed on the clipboard.
#[cfg(target_os = "macos")]
pub fn execute_area_capture(auto_copy: bool) -> Result<Option<CaptureMetadata>, String> {
    run_screencapture(&["-i", "-s", "-x"], auto_copy)
}

/// Execute window capture using macOS native `screencapture -i -w -x` (interactive selection).
///
/// Returns `Ok(None)` if the user cancelled (Escape).
#[cfg(target_os = "macos")]
pub fn execute_window_capture(auto_copy: bool) -> Result<Option<CaptureMetadata>, String> {
    run_screencapture(&["-i", "-w", "-x"], auto_copy)
}

/// Execute fullscreen capture using macOS native `screencapture -x` (non-interactive).
#[cfg(target_os = "macos")]
pub fn execute_fullscreen_capture(auto_copy: bool) -> Result<Option<CaptureMetadata>, String> {
    run_screencapture(&["-x"], auto_copy)
}

#[cfg(target_os = "macos")]
fn run_screencapture(args: &[&str], auto_copy: bool) -> Result<Option<CaptureMetadata>, String> {
    let tmp_path = temp_capture_path();
    let mut cmd_args: Vec<&str> = args.to_vec();
    let tmp_path_str = tmp_path.to_string_lossy();
    cmd_args.push(&tmp_path_str);

    tracing::info!(args = ?cmd_args, "Running screencapture");

    let status = Command::new("screencapture")
        .args(&cmd_args)
        .status()
        .map_err(|e| format!("Failed to launch screencapture: {e}"))?;

    if !status.success() || !tmp_path.exists() {
        tracing::info!("Capture cancelled or produced no output");
        return Ok(None);
    }

    let processed = process_captured_file(&tmp_path, auto_copy);
    let _ = std::fs::remove_file(&tmp_path);
    processed.map(Some)
}

fn process_captured_file(file_path: &Path, auto_copy: bool) -> Result<CaptureMetadata, String> {
    let raw_bytes =
        std::fs::read(file_path).map_err(|e| format!("Failed to read capture file: {e}"))?;
    let processed = process_image_bytes(&raw_bytes).map_err(|e| e.to_string())?;

    if auto_copy {
        clipboard::copy_rgba_to_clipboard(
            &processed.rgba,
            processed.metadata.optimized_width,
            processed.metadata.optimized_height,
        )?;
    }

    Ok(processed.metadata)
}
