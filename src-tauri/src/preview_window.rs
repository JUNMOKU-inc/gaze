//! Preview popup window — pure Rust-driven, no React.
//!
//! Each capture creates a new webview with data baked into HTML.
//! Multiple previews stack vertically from the bottom-right corner.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use snapforge_core::CaptureMetadata;
use tauri::{AppHandle, Manager as _, WebviewWindowBuilder};

/// Metadata for GIF recording preview (passed to preview.html)
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GifPreviewData {
    pub media_type: String,
    pub original_width: u32,
    pub original_height: u32,
    pub optimized_width: u32,
    pub optimized_height: u32,
    pub file_size: usize,
    pub timestamp: String,
    pub image_base64: String,
    pub gif_base64: String,
    pub frame_count: u32,
    pub duration_secs: f64,
}

/// Debounce guard: ignore duplicate shortcut fires within 500ms.
static LAST_CAPTURE: Mutex<Option<Instant>> = Mutex::new(None);

/// Monotonically increasing counter for unique window labels.
static WINDOW_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Stack of open preview window labels (oldest first).
static PREVIEW_STACK: Mutex<Vec<String>> = Mutex::new(Vec::new());

const WINDOW_WIDTH: f64 = 260.0;
const WINDOW_HEIGHT: f64 = 190.0;
const MARGIN: f64 = 24.0;
const DOCK_HEIGHT: f64 = 72.0;
const STACK_GAP: f64 = 8.0;

pub fn debounce_check() -> bool {
    let mut last = LAST_CAPTURE.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    if let Some(prev) = *last {
        if now.duration_since(prev).as_millis() < 500 {
            tracing::debug!("Capture debounced (duplicate shortcut fire)");
            return false;
        }
    }
    *last = Some(now);
    true
}

/// Create a new preview popup, stacking above any existing ones.
pub fn show_preview(app: &AppHandle, metadata: &CaptureMetadata) {
    let seq = WINDOW_COUNTER.fetch_add(1, Ordering::Relaxed);
    let label = format!("preview-{seq}");

    // Read settings (max_previews, preview_position)
    let settings = crate::settings::load_settings(app);
    let max_previews = (settings.max_previews as usize).clamp(1, 10);
    let position = settings.preview_position;

    // Reserve our slot atomically: evict AND push under the same lock so a
    // concurrent show_preview / show_gif_preview can't race past the
    // len-check and push a second window, exceeding max_previews.
    {
        let mut stack = PREVIEW_STACK.lock().unwrap_or_else(|e| e.into_inner());
        while stack.len() >= max_previews {
            let oldest = stack.remove(0);
            if let Some(w) = app.get_webview_window(&oldest) {
                let _ = w.close();
            }
        }
        stack.push(label.clone());
    }

    // Shift existing windows up to make room for the new one at the bottom
    reposition_stack(app, 1);

    let metadata_json = serde_json::to_string(metadata).unwrap_or_else(|_| "{}".to_string());

    tracing::info!(label = %label, "Creating preview window...");

    // preview.html is a self-contained page that reads window.__GAZE_DATA__
    // and window.__GAZE_LABEL__ on load. No document.write/innerHTML needed —
    // the page renders itself, preserving the Tauri IPC bridge.
    match WebviewWindowBuilder::new(app, &label, tauri::WebviewUrl::App("preview.html".into()))
        .title("Gaze Preview")
        .inner_size(WINDOW_WIDTH, WINDOW_HEIGHT)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(true)
        .focused(false)
        .accept_first_mouse(true)
        .initialization_script(format!(
            "window.__GAZE_DATA__ = {metadata_json}; window.__GAZE_LABEL__ = \"{label}\";",
        ))
        .build()
    {
        Ok(window) => {
            // Position at configured corner (slot 0 = newest)
            position_window(&window, 0, &position);

            // macOS: make window follow spaces + transparent background
            #[cfg(target_os = "macos")]
            if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                configure_macos_window(&window);
            })) {
                tracing::warn!("macOS window configuration failed: {e:?}");
            }

            tracing::info!(label = %label, "Preview window created and shown");
        }
        Err(e) => {
            tracing::error!(label = %label, "Failed to create preview window: {e}");
            // Un-reserve the slot we claimed above so the stack doesn't grow
            // unbounded on repeated build() failures.
            let mut stack = PREVIEW_STACK.lock().unwrap_or_else(|err| err.into_inner());
            stack.retain(|l| l != &label);
            drop(stack);
            reposition_stack(app, 0);
        }
    }
}

/// Show a small recording indicator at the top-center of the screen.
pub fn show_recording_indicator(app: &AppHandle) {
    // Close any existing indicator first
    if let Some(w) = app.get_webview_window("recording-indicator") {
        let _ = w.close();
    }

    let start_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    match WebviewWindowBuilder::new(
        app,
        "recording-indicator",
        tauri::WebviewUrl::App("recording-indicator.html".into()),
    )
    .title("")
    .inner_size(120.0, 32.0)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .visible(true)
    .focused(false)
    .initialization_script(format!("window.__GAZE_RECORDING_START__ = {start_ms};",))
    .build()
    {
        Ok(window) => {
            // Center horizontally at top of screen
            if let Ok(Some(monitor)) = window.primary_monitor() {
                let size = monitor.size();
                let scale = monitor.scale_factor();
                let screen_w = size.width as f64 / scale;
                let x = (screen_w - 120.0) / 2.0;
                let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(
                    x, 12.0,
                )));
            }

            #[cfg(target_os = "macos")]
            if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                configure_macos_window(&window);
            })) {
                tracing::warn!("macOS indicator window config failed: {e:?}");
            }

            tracing::info!("Recording indicator shown");
        }
        Err(e) => {
            tracing::error!("Failed to create recording indicator: {e}");
        }
    }
}

/// Hide the recording indicator.
pub fn hide_recording_indicator(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("recording-indicator") {
        let _ = w.close();
    }
}

/// Create a new preview popup for a GIF recording.
pub fn show_gif_preview(app: &AppHandle, gif_data: &GifPreviewData) {
    let seq = WINDOW_COUNTER.fetch_add(1, Ordering::Relaxed);
    let label = format!("preview-{seq}");

    let settings = crate::settings::load_settings(app);
    let max_previews = (settings.max_previews as usize).clamp(1, 10);
    let position = settings.preview_position;

    {
        let mut stack = PREVIEW_STACK.lock().unwrap_or_else(|e| e.into_inner());
        while stack.len() >= max_previews {
            let oldest = stack.remove(0);
            if let Some(w) = app.get_webview_window(&oldest) {
                let _ = w.close();
            }
        }
        stack.push(label.clone());
    }

    reposition_stack(app, 1);

    let data_json = serde_json::to_string(gif_data).unwrap_or_else(|_| "{}".to_string());

    tracing::info!(label = %label, "Creating GIF preview window...");

    match WebviewWindowBuilder::new(app, &label, tauri::WebviewUrl::App("preview.html".into()))
        .title("Gaze Preview")
        .inner_size(WINDOW_WIDTH, WINDOW_HEIGHT)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(true)
        .focused(false)
        .accept_first_mouse(true)
        .initialization_script(format!(
            "window.__GAZE_DATA__ = {data_json}; window.__GAZE_LABEL__ = \"{label}\";",
        ))
        .build()
    {
        Ok(window) => {
            position_window(&window, 0, &position);

            #[cfg(target_os = "macos")]
            if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                configure_macos_window(&window);
            })) {
                tracing::warn!("macOS window configuration failed: {e:?}");
            }

            tracing::info!(label = %label, "GIF preview window created");
        }
        Err(e) => {
            tracing::error!(label = %label, "Failed to create GIF preview window: {e}");
            // Un-reserve the slot on build failure to prevent unbounded stack growth.
            let mut stack = PREVIEW_STACK.lock().unwrap_or_else(|err| err.into_inner());
            stack.retain(|l| l != &label);
            drop(stack);
            reposition_stack(app, 0);
        }
    }
}

/// Close a specific preview window by label and reposition the stack.
pub fn close_preview_by_label(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.close();
    }

    if let Ok(mut stack) = PREVIEW_STACK.lock() {
        stack.retain(|l| l != label);
    }

    // Reposition remaining windows to fill the gap
    reposition_stack(app, 0);
}

/// Reposition a single preview window back to its correct stack slot.
pub fn reposition_single(app: &AppHandle, label: &str) {
    let position = crate::settings::load_settings(app).preview_position;
    let stack = match PREVIEW_STACK.lock() {
        Ok(s) => s.clone(),
        Err(e) => e.into_inner().clone(),
    };
    // Stack is ordered oldest-first. Newest (last) = slot 0.
    for (i, l) in stack.iter().rev().enumerate() {
        if l == label {
            if let Some(window) = app.get_webview_window(label) {
                let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
                    WINDOW_WIDTH,
                    WINDOW_HEIGHT,
                )));
                position_window(&window, i, &position);
            }
            break;
        }
    }
}

/// Close all preview windows.
#[allow(dead_code)]
pub fn close_all_previews(app: &AppHandle) {
    if let Ok(mut stack) = PREVIEW_STACK.lock() {
        for label in stack.drain(..) {
            if let Some(w) = app.get_webview_window(&label) {
                let _ = w.close();
            }
        }
    }
}

/// Reposition all stacked preview windows.
/// `extra_offset`: additional slots to shift up (used when making room for a new window).
fn reposition_stack(app: &AppHandle, extra_offset: usize) {
    let position = crate::settings::load_settings(app).preview_position;
    let stack = match PREVIEW_STACK.lock() {
        Ok(s) => s.clone(),
        Err(_) => return,
    };

    // Stack is ordered oldest-first. Newest (last) is at slot 0 (bottom).
    // When extra_offset=1, we're about to add a new window at slot 0,
    // so existing windows need to shift up by 1.
    for (i, label) in stack.iter().rev().enumerate() {
        let slot = i + extra_offset;
        if let Some(window) = app.get_webview_window(label) {
            position_window(&window, slot, &position);
        }
    }
}

/// Compute logical (x, y) for a preview window at the given stack slot and corner.
///
/// Pure function so tests exercise the same code path that runs in production.
/// `position` values: "bottom_right" (default), "bottom_left", "top_right", "top_left"
/// (also accepts kebab-case variants like "top-left").
pub(crate) fn compute_corner_position(
    logical_w: f64,
    logical_h: f64,
    slot: usize,
    position: &str,
) -> (f64, f64) {
    match position {
        "top_left" | "top-left" => (MARGIN, MARGIN + (slot as f64 * (WINDOW_HEIGHT + STACK_GAP))),
        "top_right" | "top-right" => (
            logical_w - WINDOW_WIDTH - MARGIN,
            MARGIN + (slot as f64 * (WINDOW_HEIGHT + STACK_GAP)),
        ),
        "bottom_left" | "bottom-left" => (
            MARGIN,
            logical_h
                - WINDOW_HEIGHT
                - MARGIN
                - DOCK_HEIGHT
                - (slot as f64 * (WINDOW_HEIGHT + STACK_GAP)),
        ),
        _ => (
            // bottom_right (default)
            logical_w - WINDOW_WIDTH - MARGIN,
            logical_h
                - WINDOW_HEIGHT
                - MARGIN
                - DOCK_HEIGHT
                - (slot as f64 * (WINDOW_HEIGHT + STACK_GAP)),
        ),
    }
}

/// Position a window at the given stack slot in the specified screen corner.
fn position_window(window: &tauri::WebviewWindow, slot: usize, position: &str) {
    if let Ok(Some(monitor)) = window.primary_monitor() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let logical_w = size.width as f64 / scale;
        let logical_h = size.height as f64 / scale;

        let (x, y) = compute_corner_position(logical_w, logical_h, slot, position);
        let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)));
    }
}

// ─── macOS-specific configuration ──────────────────────────────────

#[cfg(target_os = "macos")]
fn configure_macos_window(window: &tauri::WebviewWindow) {
    use objc2_app_kit::NSWindow;

    // Configure NSWindow
    let ns_window = match window.ns_window() {
        Ok(ptr) => unsafe { &*(ptr.cast::<NSWindow>()) },
        Err(e) => {
            tracing::warn!("Failed to get NSWindow: {e}");
            return;
        }
    };

    unsafe {
        // Follow across all spaces (Mission Control / 3-finger swipe)
        ns_window.setCollectionBehavior(
            objc2_app_kit::NSWindowCollectionBehavior::CanJoinAllSpaces
                | objc2_app_kit::NSWindowCollectionBehavior::Stationary,
        );

        // Transparent window background
        let clear = objc2_app_kit::NSColor::clearColor();
        ns_window.setBackgroundColor(Some(&clear));
        ns_window.setOpaque(false);
    }

    // Make WKWebView background transparent
    let _ = window.with_webview(|webview| {
        // webview.inner() returns the WKWebView pointer
        // We need to call [wkWebView setValue:@NO forKey:@"drawsBackground"]
        let wk_ptr = webview.inner();
        if !wk_ptr.is_null() {
            unsafe {
                // Use raw objc_msgSend to avoid objc2 version API differences
                let key = objc2_foundation::ns_string!("drawsBackground");
                let no = objc2_foundation::NSNumber::numberWithBool(false);
                let _: () = objc2::msg_send![
                    &*wk_ptr.cast::<objc2::runtime::NSObject>(),
                    setValue: &*no,
                    forKey: key
                ];
            }
        }
    });
}

// HTML generation functions removed — preview.html is now self-contained.
// It reads window.__GAZE_DATA__ and window.__GAZE_LABEL__ set by initialization_script.

// ─── Pure helper functions (testable without Tauri runtime) ─────────

#[cfg(test)]
/// Check if a capture should be debounced based on the last capture time.
/// Pure function for testing — accepts parameters instead of reading global state.
fn should_debounce(last_capture: Option<Instant>, now: Instant, threshold_ms: u64) -> bool {
    if let Some(prev) = last_capture {
        now.duration_since(prev).as_millis() < threshold_ms as u128
    } else {
        false
    }
}

#[cfg(test)]
/// Determine which labels to evict when the stack is at/over capacity.
/// Returns labels to close (oldest first).
fn compute_eviction_targets(stack: &[String], max_previews: usize) -> Vec<String> {
    if stack.len() >= max_previews {
        let to_remove = stack.len() - max_previews + 1;
        stack[..to_remove].to_vec()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
/// Test helper: apply retina scaling then call the production `compute_corner_position`.
/// Tests hit the real function body this way — no duplicated logic.
fn calculate_window_position(
    monitor_width: f64,
    monitor_height: f64,
    scale_factor: f64,
    slot: usize,
) -> (f64, f64) {
    calculate_window_position_for_corner(
        monitor_width,
        monitor_height,
        scale_factor,
        slot,
        "bottom_right",
    )
}

#[cfg(test)]
fn calculate_window_position_for_corner(
    monitor_width: f64,
    monitor_height: f64,
    scale_factor: f64,
    slot: usize,
    position: &str,
) -> (f64, f64) {
    compute_corner_position(
        monitor_width / scale_factor,
        monitor_height / scale_factor,
        slot,
        position,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // --- Original tests ---

    #[test]
    fn window_counter_increments() {
        let a = WINDOW_COUNTER.load(Ordering::Relaxed);
        let b = WINDOW_COUNTER.fetch_add(1, Ordering::Relaxed);
        assert_eq!(a, b);
        assert_eq!(WINDOW_COUNTER.load(Ordering::Relaxed), b + 1);
    }

    #[test]
    fn window_label_format() {
        let seq = 42u32;
        let label = format!("preview-{seq}");
        assert_eq!(label, "preview-42");
        assert!(label.starts_with("preview-"));
    }

    #[test]
    fn position_calculation_slot_0() {
        let screen_w: f64 = 1920.0;
        let screen_h: f64 = 1080.0;
        let x = screen_w - WINDOW_WIDTH - MARGIN;
        let y =
            screen_h - WINDOW_HEIGHT - MARGIN - DOCK_HEIGHT - (0.0 * (WINDOW_HEIGHT + STACK_GAP));
        assert_eq!(x, 1920.0 - 260.0 - 24.0);
        assert_eq!(y, 1080.0 - 190.0 - 24.0 - 72.0);
    }

    #[test]
    fn position_calculation_stacking() {
        let screen_h: f64 = 1080.0;
        let base_y = screen_h - WINDOW_HEIGHT - MARGIN - DOCK_HEIGHT;

        let y_slot_0 = base_y - (0.0 * (WINDOW_HEIGHT + STACK_GAP));
        let y_slot_1 = base_y - (1.0 * (WINDOW_HEIGHT + STACK_GAP));
        let y_slot_2 = base_y - (2.0 * (WINDOW_HEIGHT + STACK_GAP));

        let step = y_slot_0 - y_slot_1;
        assert!((step - (WINDOW_HEIGHT + STACK_GAP)).abs() < f64::EPSILON);
        assert!((y_slot_1 - y_slot_2 - step).abs() < f64::EPSILON);
    }

    #[test]
    fn layout_constants_are_sensible() {
        // Use runtime indirection to avoid clippy::assertions_on_constants
        let vals: Vec<f64> = vec![WINDOW_WIDTH, WINDOW_HEIGHT, MARGIN];
        for v in &vals {
            assert!(*v > 0.0, "Layout constant must be positive: {v}");
        }
        let nonneg: Vec<f64> = vec![DOCK_HEIGHT, STACK_GAP];
        for v in &nonneg {
            assert!(v.is_finite(), "Layout constant must be finite: {v}");
        }
    }

    // --- should_debounce tests ---

    #[test]
    fn debounce_first_capture_passes() {
        let now = Instant::now();
        assert!(!should_debounce(None, now, 500));
    }

    #[test]
    fn debounce_within_threshold_blocks() {
        let first = Instant::now();
        let second = first + Duration::from_millis(100);
        assert!(should_debounce(Some(first), second, 500));
    }

    #[test]
    fn debounce_at_threshold_boundary_blocks() {
        let first = Instant::now();
        let second = first + Duration::from_millis(499);
        assert!(should_debounce(Some(first), second, 500));
    }

    #[test]
    fn debounce_after_threshold_passes() {
        let first = Instant::now();
        let second = first + Duration::from_millis(501);
        assert!(!should_debounce(Some(first), second, 500));
    }

    #[test]
    fn debounce_exactly_at_threshold_passes() {
        let first = Instant::now();
        let second = first + Duration::from_millis(500);
        assert!(!should_debounce(Some(first), second, 500));
    }

    #[test]
    fn debounce_zero_threshold_never_blocks() {
        let first = Instant::now();
        let second = first + Duration::from_millis(0);
        assert!(!should_debounce(Some(first), second, 0));
    }

    // --- compute_eviction_targets tests ---

    #[test]
    fn eviction_empty_stack_no_targets() {
        let targets = compute_eviction_targets(&[], 5);
        assert!(targets.is_empty());
    }

    #[test]
    fn eviction_under_limit_no_targets() {
        let stack = vec!["a".into(), "b".into()];
        let targets = compute_eviction_targets(&stack, 5);
        assert!(targets.is_empty());
    }

    #[test]
    fn eviction_at_limit_removes_oldest() {
        let stack: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let targets = compute_eviction_targets(&stack, 3);
        assert_eq!(targets, vec!["a"]);
    }

    #[test]
    fn eviction_over_limit_removes_multiple() {
        let stack: Vec<String> = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        let targets = compute_eviction_targets(&stack, 2);
        assert_eq!(targets, vec!["a", "b", "c"]);
    }

    #[test]
    fn eviction_max_one_keeps_only_newest() {
        let stack: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let targets = compute_eviction_targets(&stack, 1);
        assert_eq!(targets, vec!["a", "b", "c"]);
    }

    #[test]
    fn eviction_single_item_at_limit() {
        let stack: Vec<String> = vec!["a".into()];
        let targets = compute_eviction_targets(&stack, 1);
        assert_eq!(targets, vec!["a"]);
    }

    // --- calculate_window_position tests ---

    #[test]
    fn position_slot_zero_bottom_right() {
        let (x, y) = calculate_window_position(2560.0, 1440.0, 1.0, 0);
        assert!((x - (2560.0 - WINDOW_WIDTH - MARGIN)).abs() < 0.1);
        let expected_y = 1440.0 - WINDOW_HEIGHT - MARGIN - DOCK_HEIGHT;
        assert!((y - expected_y).abs() < 0.1);
    }

    #[test]
    fn position_slot_one_above_slot_zero() {
        let (_, y0) = calculate_window_position(2560.0, 1440.0, 1.0, 0);
        let (_, y1) = calculate_window_position(2560.0, 1440.0, 1.0, 1);
        assert!(y1 < y0);
        let gap = y0 - y1;
        assert!((gap - (WINDOW_HEIGHT + STACK_GAP)).abs() < 0.1);
    }

    #[test]
    fn position_with_retina_scaling() {
        let (x1, y1) = calculate_window_position(5120.0, 2880.0, 2.0, 0);
        let (x2, y2) = calculate_window_position(2560.0, 1440.0, 1.0, 0);
        assert!((x1 - x2).abs() < 0.1);
        assert!((y1 - y2).abs() < 0.1);
    }

    #[test]
    fn position_x_is_same_across_slots() {
        let (x0, _) = calculate_window_position(1920.0, 1080.0, 1.0, 0);
        let (x1, _) = calculate_window_position(1920.0, 1080.0, 1.0, 1);
        let (x2, _) = calculate_window_position(1920.0, 1080.0, 1.0, 2);
        assert!((x0 - x1).abs() < 0.001);
        assert!((x1 - x2).abs() < 0.001);
    }

    #[test]
    fn position_stacks_evenly() {
        let (_, y0) = calculate_window_position(1920.0, 1080.0, 1.0, 0);
        let (_, y1) = calculate_window_position(1920.0, 1080.0, 1.0, 1);
        let (_, y2) = calculate_window_position(1920.0, 1080.0, 1.0, 2);
        let gap01 = y0 - y1;
        let gap12 = y1 - y2;
        assert!((gap01 - gap12).abs() < 0.001);
    }

    // --- calculate_window_position_for_corner tests ---

    #[test]
    fn corner_bottom_right_matches_default() {
        let (x1, y1) = calculate_window_position(2560.0, 1440.0, 1.0, 0);
        let (x2, y2) = calculate_window_position_for_corner(2560.0, 1440.0, 1.0, 0, "bottom_right");
        assert!((x1 - x2).abs() < 0.001);
        assert!((y1 - y2).abs() < 0.001);
    }

    #[test]
    fn corner_top_left_slot_zero() {
        let (x, y) = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 0, "top_left");
        assert!((x - MARGIN).abs() < 0.001, "x should be at left margin");
        assert!((y - MARGIN).abs() < 0.001, "y should be at top margin");
    }

    #[test]
    fn corner_top_right_slot_zero() {
        let (x, y) = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 0, "top_right");
        let expected_x = 1920.0 - WINDOW_WIDTH - MARGIN;
        assert!((x - expected_x).abs() < 0.001, "x should be at right edge");
        assert!((y - MARGIN).abs() < 0.001, "y should be at top margin");
    }

    #[test]
    fn corner_bottom_left_slot_zero() {
        let (x, y) = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 0, "bottom_left");
        assert!((x - MARGIN).abs() < 0.001, "x should be at left margin");
        let expected_y = 1080.0 - WINDOW_HEIGHT - MARGIN - DOCK_HEIGHT;
        assert!((y - expected_y).abs() < 0.001, "y should be at bottom edge");
    }

    #[test]
    fn corner_top_left_stacks_downward() {
        let (_, y0) = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 0, "top_left");
        let (_, y1) = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 1, "top_left");
        assert!(
            y1 > y0,
            "top_left slot 1 should be below slot 0 (stacks downward)"
        );
        let gap = y1 - y0;
        assert!((gap - (WINDOW_HEIGHT + STACK_GAP)).abs() < 0.001);
    }

    #[test]
    fn corner_top_right_stacks_downward() {
        let (_, y0) = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 0, "top_right");
        let (_, y1) = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 1, "top_right");
        assert!(y1 > y0, "top_right stacks downward");
    }

    #[test]
    fn corner_bottom_left_stacks_upward() {
        let (_, y0) = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 0, "bottom_left");
        let (_, y1) = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 1, "bottom_left");
        assert!(y1 < y0, "bottom_left stacks upward");
    }

    #[test]
    fn corner_bottom_right_stacks_upward() {
        let (_, y0) = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 0, "bottom_right");
        let (_, y1) = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 1, "bottom_right");
        assert!(y1 < y0, "bottom_right stacks upward");
    }

    #[test]
    fn corner_kebab_case_matches_snake_case() {
        let (x1, y1) = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 0, "top_left");
        let (x2, y2) = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 0, "top-left");
        assert!((x1 - x2).abs() < 0.001);
        assert!((y1 - y2).abs() < 0.001);

        let (x3, y3) = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 0, "bottom_left");
        let (x4, y4) = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 0, "bottom-left");
        assert!((x3 - x4).abs() < 0.001);
        assert!((y3 - y4).abs() < 0.001);
    }

    #[test]
    fn corner_unknown_value_falls_back_to_bottom_right() {
        let (x1, y1) = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 0, "bottom_right");
        let (x2, y2) =
            calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 0, "invalid_value");
        assert!((x1 - x2).abs() < 0.001);
        assert!((y1 - y2).abs() < 0.001);
    }

    #[test]
    fn corner_with_retina_scaling() {
        // 2x retina should produce same logical positions
        let (x1, y1) = calculate_window_position_for_corner(5120.0, 2880.0, 2.0, 0, "top_left");
        let (x2, y2) = calculate_window_position_for_corner(2560.0, 1440.0, 1.0, 0, "top_left");
        assert!((x1 - x2).abs() < 0.001);
        assert!((y1 - y2).abs() < 0.001);
    }

    #[test]
    fn corner_all_four_have_distinct_positions() {
        let br = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 0, "bottom_right");
        let bl = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 0, "bottom_left");
        let tr = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 0, "top_right");
        let tl = calculate_window_position_for_corner(1920.0, 1080.0, 1.0, 0, "top_left");

        // All four positions should be distinct
        let positions = [br, bl, tr, tl];
        for i in 0..positions.len() {
            for j in (i + 1)..positions.len() {
                assert!(
                    (positions[i].0 - positions[j].0).abs() > 1.0
                        || (positions[i].1 - positions[j].1).abs() > 1.0,
                    "Positions {i} and {j} should differ"
                );
            }
        }
    }
}
