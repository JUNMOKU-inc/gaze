use crate::{
    CaptureEngine, CaptureError, CaptureResult, DisplayInfo, FrameData, StreamConfig, WindowInfo,
};

use screencapturekit::cm::CMTime;
use screencapturekit::cv::CVPixelBufferLockFlags;
use screencapturekit::prelude::*;
use screencapturekit::screenshot_manager::SCScreenshotManager;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGMainDisplayID() -> u32;
}

/// Ensure a CoreGraphics session (CGS) exists.
///
/// CLI/headless processes have no `NSApplication`, so CGS is not
/// automatically initialized. Without this, ScreenCaptureKit's
/// window-capture API crashes with `CGS_REQUIRE_INIT`.
pub(crate) fn ensure_cgs_session() {
    // SAFETY: CGMainDisplayID() is a pure read-only CoreGraphics function
    // with no preconditions. It returns the main display ID and triggers
    // CGS session initialization as a side effect.
    unsafe {
        CGMainDisplayID();
    }
}

/// macOS screen capture implementation using ScreenCaptureKit
pub struct NativeCaptureEngine;

impl NativeCaptureEngine {
    pub fn new() -> Self {
        Self
    }
}

/// Check if screen recording permission is granted.
///
/// Attempts to retrieve shareable content. If this succeeds, the user has
/// granted screen recording permission.
pub fn has_permission() -> bool {
    SCShareableContent::get().is_ok()
}

/// Request screen recording permission.
///
/// On macOS, calling `SCShareableContent::get()` triggers the system
/// permission dialog if permission has not been granted yet. Returns
/// `true` if permission is available after the call.
pub fn request_permission() -> bool {
    // Accessing SCShareableContent triggers the macOS permission dialog
    // if not already granted.
    SCShareableContent::get().is_ok()
}

/// Helper: get shareable content, mapping errors to CaptureError
fn get_shareable_content() -> Result<SCShareableContent, CaptureError> {
    SCShareableContent::get().map_err(|e| {
        let msg = e.to_string();
        if msg.contains("permission") || msg.contains("denied") || msg.contains("not authorized") {
            CaptureError::PermissionDenied
        } else {
            CaptureError::Internal(anyhow::anyhow!("Failed to get shareable content: {e}"))
        }
    })
}

/// Helper: find display by ID from shareable content
fn find_display(content: &SCShareableContent, display_id: u32) -> Result<SCDisplay, CaptureError> {
    content
        .displays()
        .into_iter()
        .find(|d| d.display_id() == display_id)
        .ok_or(CaptureError::DisplayNotFound(display_id))
}

/// Helper: capture a display via SCScreenshotManager and return RGBA data
fn capture_display_screenshot(display: &SCDisplay) -> Result<CaptureResult, CaptureError> {
    let filter = SCContentFilter::create()
        .with_display(display)
        .with_excluding_windows(&[])
        .build();

    let width = display.width();
    let height = display.height();

    let config = SCStreamConfiguration::new()
        .with_width(width)
        .with_height(height)
        .with_pixel_format(PixelFormat::BGRA)
        .with_shows_cursor(false);

    let cg_image = SCScreenshotManager::capture_image(&filter, &config)
        .map_err(|e| CaptureError::Internal(anyhow::anyhow!("Screenshot capture failed: {e}")))?;

    let rgba_data = cg_image
        .rgba_data()
        .map_err(|e| CaptureError::Internal(anyhow::anyhow!("Failed to extract RGBA data: {e}")))?;

    #[allow(clippy::cast_possible_truncation)]
    Ok(CaptureResult {
        width: cg_image.width() as u32,
        height: cg_image.height() as u32,
        data: rgba_data,
    })
}

impl CaptureEngine for NativeCaptureEngine {
    fn capture_fullscreen(&self, display_id: u32) -> Result<CaptureResult, CaptureError> {
        tracing::info!(display_id, "Capturing fullscreen via ScreenCaptureKit");

        let content = get_shareable_content()?;
        let display = find_display(&content, display_id)?;
        capture_display_screenshot(&display)
    }

    fn capture_region(
        &self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
    ) -> Result<CaptureResult, CaptureError> {
        tracing::info!(x, y, w, h, "Capturing region via ScreenCaptureKit");

        let content = get_shareable_content()?;
        let displays = content.displays();
        let display = displays.first().ok_or(CaptureError::NoDisplays)?;

        // Capture the full display, then crop the requested region
        let full = capture_display_screenshot(display)?;

        // Use the image crate to crop
        let img_buf: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
            image::ImageBuffer::from_raw(full.width, full.height, full.data).ok_or_else(|| {
                CaptureError::Internal(anyhow::anyhow!(
                    "Failed to create image buffer from capture data"
                ))
            })?;

        let dyn_image = image::DynamicImage::ImageRgba8(img_buf);

        // Clamp crop bounds to image dimensions
        #[allow(clippy::cast_sign_loss)]
        let crop_x = x.max(0) as u32;
        #[allow(clippy::cast_sign_loss)]
        let crop_y = y.max(0) as u32;
        let crop_w = w.min(full.width.saturating_sub(crop_x));
        let crop_h = h.min(full.height.saturating_sub(crop_y));

        if crop_w == 0 || crop_h == 0 {
            return Err(CaptureError::Internal(anyhow::anyhow!(
                "Region is out of display bounds"
            )));
        }

        let cropped = dyn_image.crop_imm(crop_x, crop_y, crop_w, crop_h);
        let cropped_rgba = cropped.to_rgba8();

        Ok(CaptureResult {
            width: cropped_rgba.width(),
            height: cropped_rgba.height(),
            data: cropped_rgba.into_raw(),
        })
    }

    fn capture_window(&self, window_id: u32) -> Result<CaptureResult, CaptureError> {
        tracing::info!(window_id, "Capturing window via ScreenCaptureKit");

        let content = get_shareable_content()?;

        let window = content
            .windows()
            .into_iter()
            .find(|w| w.window_id() == window_id)
            .ok_or_else(|| {
                CaptureError::Internal(anyhow::anyhow!("Window {window_id} not found"))
            })?;

        let filter = SCContentFilter::create().with_window(&window).build();

        let frame = window.frame();
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let width = (frame.width as u32).max(1);
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let height = (frame.height as u32).max(1);

        let config = SCStreamConfiguration::new()
            .with_width(width)
            .with_height(height)
            .with_pixel_format(PixelFormat::BGRA)
            .with_shows_cursor(false);

        let cg_image = SCScreenshotManager::capture_image(&filter, &config).map_err(|e| {
            CaptureError::Internal(anyhow::anyhow!("Window screenshot capture failed: {e}"))
        })?;

        let rgba_data = cg_image.rgba_data().map_err(|e| {
            CaptureError::Internal(anyhow::anyhow!("Failed to extract RGBA data: {e}"))
        })?;

        #[allow(clippy::cast_possible_truncation)]
        Ok(CaptureResult {
            width: cg_image.width() as u32,
            height: cg_image.height() as u32,
            data: rgba_data,
        })
    }

    fn list_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
        tracing::info!("Listing displays via ScreenCaptureKit");

        let content = get_shareable_content()?;
        let displays = content.displays();

        Ok(displays
            .iter()
            .map(|d| {
                let frame = d.frame();
                // Estimate scale factor from pixel width vs frame width
                let scale = if frame.width > 0.0 {
                    f64::from(d.width()) / frame.width
                } else {
                    1.0
                };

                DisplayInfo {
                    id: d.display_id(),
                    name: format!("Display {}", d.display_id()),
                    width: d.width(),
                    height: d.height(),
                    scale_factor: scale,
                }
            })
            .collect())
    }

    fn list_windows(&self) -> Result<Vec<WindowInfo>, CaptureError> {
        tracing::info!("Listing windows via ScreenCaptureKit");

        let content = get_shareable_content()?;
        let windows = content.windows();

        Ok(windows
            .iter()
            .filter(|w| w.window_layer() == 0) // Normal windows only
            .map(|w| {
                let app_name = w
                    .owning_application()
                    .map(|app| app.application_name())
                    .unwrap_or_default();

                WindowInfo {
                    id: w.window_id(),
                    title: w.title().unwrap_or_default(),
                    app_name,
                    is_on_screen: w.is_on_screen(),
                }
            })
            .collect())
    }
}

/// Start a recording stream using SCStream (ScreenCaptureKit continuous capture).
#[allow(clippy::type_complexity)]
pub fn start_recording_stream(
    config: StreamConfig,
) -> Result<(mpsc::Receiver<FrameData>, Box<dyn FnOnce() + Send>), CaptureError> {
    let content = get_shareable_content()?;
    let display = find_display(&content, config.display_id)?;

    let filter = SCContentFilter::create()
        .with_display(&display)
        .with_excluding_windows(&[])
        .build();

    // Frame interval: 1/fps seconds
    let frame_interval = CMTime::new(1, config.fps as i32);

    let stream_config = SCStreamConfiguration::new()
        .with_width(config.width)
        .with_height(config.height)
        .with_pixel_format(PixelFormat::BGRA)
        .with_shows_cursor(config.show_cursor)
        .with_minimum_frame_interval(&frame_interval);

    let (tx, rx) = mpsc::channel::<FrameData>();

    let mut stream = SCStream::new(&filter, &stream_config);

    let start_time = std::time::Instant::now();

    stream.add_output_handler(
        move |sample: screencapturekit::cm::CMSampleBuffer,
              of_type: screencapturekit::stream::output_type::SCStreamOutputType| {
            if of_type != SCStreamOutputType::Screen {
                return;
            }

            let Some(pixel_buffer) = sample.image_buffer() else {
                return;
            };

            let Ok(guard) = pixel_buffer.lock(CVPixelBufferLockFlags::READ_ONLY) else {
                return;
            };

            let width = guard.width();
            let height = guard.height();
            let bytes_per_row = guard.bytes_per_row();
            let bgra_slice = guard.as_slice();

            if bgra_slice.is_empty() {
                return;
            }

            // Convert BGRA to RGBA, handling stride (bytes_per_row may differ from width*4)
            let expected_stride = width * 4;
            let mut rgba = Vec::with_capacity(width * height * 4);

            for row in 0..height {
                let row_start = row * bytes_per_row;
                let row_end = row_start + expected_stride;
                if row_end > bgra_slice.len() {
                    break;
                }
                let row_data = &bgra_slice[row_start..row_end];
                for pixel in row_data.chunks_exact(4) {
                    // BGRA -> RGBA
                    rgba.push(pixel[2]); // R
                    rgba.push(pixel[1]); // G
                    rgba.push(pixel[0]); // B
                    rgba.push(pixel[3]); // A
                }
            }

            let timestamp_secs = start_time.elapsed().as_secs_f64();

            let frame = FrameData {
                rgba,
                width: width as u32,
                height: height as u32,
                timestamp_secs,
            };

            // If the receiver is dropped, silently stop sending
            let _ = tx.send(frame);
        },
        SCStreamOutputType::Screen,
    );

    stream
        .start_capture()
        .map_err(|e| CaptureError::Internal(anyhow::anyhow!("Failed to start stream: {e}")))?;

    // Wrap stream in Arc<Mutex> so the stop closure can take ownership
    let stream = Arc::new(Mutex::new(Some(stream)));

    let stop_fn = Box::new(move || {
        if let Ok(mut guard) = stream.lock() {
            if let Some(s) = guard.take() {
                let _ = s.stop_capture();
                // Stream is dropped here, releasing ScreenCaptureKit resources
            }
        }
    });

    Ok((rx, stop_fn))
}
