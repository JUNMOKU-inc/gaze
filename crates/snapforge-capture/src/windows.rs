use crate::{CaptureEngine, CaptureError, CaptureResult, DisplayInfo, WindowInfo};

/// Windows screen capture implementation using Windows Graphics Capture API
pub struct NativeCaptureEngine;

impl NativeCaptureEngine {
    pub fn new() -> Self {
        Self
    }
}

/// Check if screen recording permission is granted (always true on Windows)
pub fn has_permission() -> bool {
    true
}

/// Request screen recording permission (no-op on Windows, always returns true)
pub fn request_permission() -> bool {
    true
}

impl CaptureEngine for NativeCaptureEngine {
    fn capture_fullscreen(&self, _display_id: u32) -> Result<CaptureResult, CaptureError> {
        // TODO: Implement using Windows Graphics Capture API
        tracing::warn!("capture_fullscreen not yet implemented");
        Err(CaptureError::Internal(anyhow::anyhow!("Not implemented")))
    }

    fn capture_region(
        &self,
        _x: i32,
        _y: i32,
        _w: u32,
        _h: u32,
    ) -> Result<CaptureResult, CaptureError> {
        // TODO: Implement using Windows Graphics Capture API
        tracing::warn!("capture_region not yet implemented");
        Err(CaptureError::Internal(anyhow::anyhow!("Not implemented")))
    }

    fn capture_window(&self, _window_id: u32) -> Result<CaptureResult, CaptureError> {
        // TODO: Implement using Windows Graphics Capture API
        tracing::warn!("capture_window not yet implemented");
        Err(CaptureError::Internal(anyhow::anyhow!("Not implemented")))
    }

    fn list_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
        // TODO: Implement using Windows Graphics Capture API
        tracing::warn!("list_displays not yet implemented");
        Ok(vec![])
    }

    fn list_windows(&self) -> Result<Vec<WindowInfo>, CaptureError> {
        // TODO: Implement using Windows Graphics Capture API
        tracing::warn!("list_windows not yet implemented");
        Ok(vec![])
    }
}
