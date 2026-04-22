mod error;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

// Re-export platform implementation
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "windows")]
use windows as platform;

pub use error::CaptureError;

/// Information about a display/monitor
#[derive(Debug, Clone)]
pub struct DisplayInfo {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

/// Information about a window
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: u32,
    pub title: String,
    pub app_name: String,
    pub is_on_screen: bool,
}

/// Result of a screen capture operation
#[derive(Debug, Clone)]
pub struct CaptureResult {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // Raw RGBA pixels
}

/// Configuration for screen recording stream
#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub display_id: u32,
    pub fps: u32,
    pub width: u32,
    pub height: u32,
    pub show_cursor: bool,
}

/// A single captured frame from a recording stream
#[derive(Debug, Clone)]
pub struct FrameData {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub timestamp_secs: f64,
}

/// Trait for screen capture engines (enables mocking in tests)
pub trait CaptureEngine: Send + Sync {
    fn capture_fullscreen(&self, display_id: u32) -> Result<CaptureResult, CaptureError>;
    fn capture_region(&self, x: i32, y: i32, w: u32, h: u32)
        -> Result<CaptureResult, CaptureError>;
    fn capture_window(&self, window_id: u32) -> Result<CaptureResult, CaptureError>;
    fn list_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError>;
    fn list_windows(&self) -> Result<Vec<WindowInfo>, CaptureError>;
}

/// Check if screen recording permission is granted
pub fn has_permission() -> bool {
    platform::has_permission()
}

/// Request screen recording permission (opens system dialog)
pub fn request_permission() -> bool {
    platform::request_permission()
}

/// Create the default capture engine for the current platform.
///
/// On macOS, this also initializes the CoreGraphics session (CGS)
/// so that ScreenCaptureKit works correctly in CLI/headless processes.
pub fn create_engine() -> Box<dyn CaptureEngine> {
    #[cfg(target_os = "macos")]
    platform::ensure_cgs_session();

    Box::new(platform::NativeCaptureEngine::new())
}

/// Start a screen recording stream. Returns a receiver for frames and a stop handle.
///
/// Call `stop_fn()` to stop the stream.
#[cfg(target_os = "macos")]
#[allow(clippy::type_complexity)]
pub fn start_recording_stream(
    config: StreamConfig,
) -> Result<
    (
        std::sync::mpsc::Receiver<FrameData>,
        Box<dyn FnOnce() + Send>,
    ),
    CaptureError,
> {
    platform::start_recording_stream(config)
}

#[cfg(test)]
pub mod mock {
    use super::*;

    /// Mock capture engine for testing
    pub struct MockCaptureEngine {
        pub displays: Vec<DisplayInfo>,
        pub windows: Vec<WindowInfo>,
        pub capture_result: CaptureResult,
    }

    impl Default for MockCaptureEngine {
        fn default() -> Self {
            Self::new()
        }
    }

    impl MockCaptureEngine {
        pub fn new() -> Self {
            Self {
                displays: vec![DisplayInfo {
                    id: 1,
                    name: "Mock Display".into(),
                    width: 1920,
                    height: 1080,
                    scale_factor: 2.0,
                }],
                windows: vec![WindowInfo {
                    id: 100,
                    title: "Mock Window".into(),
                    app_name: "MockApp".into(),
                    is_on_screen: true,
                }],
                capture_result: CaptureResult {
                    width: 100,
                    height: 100,
                    data: vec![0u8; 100 * 100 * 4], // 100x100 RGBA black
                },
            }
        }

        pub fn with_capture_result(mut self, result: CaptureResult) -> Self {
            self.capture_result = result;
            self
        }
    }

    impl CaptureEngine for MockCaptureEngine {
        fn capture_fullscreen(&self, _display_id: u32) -> Result<CaptureResult, CaptureError> {
            Ok(self.capture_result.clone())
        }

        fn capture_region(
            &self,
            _x: i32,
            _y: i32,
            _w: u32,
            _h: u32,
        ) -> Result<CaptureResult, CaptureError> {
            Ok(self.capture_result.clone())
        }

        fn capture_window(&self, _window_id: u32) -> Result<CaptureResult, CaptureError> {
            Ok(self.capture_result.clone())
        }

        fn list_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
            Ok(self.displays.clone())
        }

        fn list_windows(&self) -> Result<Vec<WindowInfo>, CaptureError> {
            Ok(self.windows.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mock::MockCaptureEngine;

    #[test]
    fn display_info_clone_and_debug() {
        let info = DisplayInfo {
            id: 1,
            name: "Test".into(),
            width: 1920,
            height: 1080,
            scale_factor: 2.0,
        };
        let cloned = info.clone();
        assert_eq!(cloned.id, 1);
        assert_eq!(cloned.name, "Test");
        assert_eq!(cloned.width, 1920);
        assert_eq!(cloned.height, 1080);
        assert!((cloned.scale_factor - 2.0).abs() < f64::EPSILON);
        let debug = format!("{:?}", info);
        assert!(debug.contains("DisplayInfo"));
    }

    #[test]
    fn window_info_clone_and_debug() {
        let info = WindowInfo {
            id: 42,
            title: "My Window".into(),
            app_name: "App".into(),
            is_on_screen: true,
        };
        let cloned = info.clone();
        assert_eq!(cloned.id, 42);
        assert_eq!(cloned.title, "My Window");
        assert!(cloned.is_on_screen);
        let debug = format!("{:?}", info);
        assert!(debug.contains("WindowInfo"));
    }

    #[test]
    fn capture_result_clone_and_debug() {
        let result = CaptureResult {
            width: 100,
            height: 200,
            data: vec![0u8; 100 * 200 * 4],
        };
        let cloned = result.clone();
        assert_eq!(cloned.width, 100);
        assert_eq!(cloned.height, 200);
        assert_eq!(cloned.data.len(), 100 * 200 * 4);
        let debug = format!("{:?}", result);
        assert!(debug.contains("CaptureResult"));
    }

    #[test]
    fn mock_engine_defaults() {
        let engine = MockCaptureEngine::new();
        assert_eq!(engine.displays.len(), 1);
        assert_eq!(engine.displays[0].name, "Mock Display");
        assert_eq!(engine.windows.len(), 1);
        assert_eq!(engine.windows[0].title, "Mock Window");
        assert_eq!(engine.capture_result.width, 100);
        assert_eq!(engine.capture_result.height, 100);
    }

    #[test]
    fn mock_engine_with_custom_capture_result() {
        let custom_result = CaptureResult {
            width: 50,
            height: 50,
            data: vec![255u8; 50 * 50 * 4],
        };
        let engine = MockCaptureEngine::new().with_capture_result(custom_result);
        assert_eq!(engine.capture_result.width, 50);
        assert_eq!(engine.capture_result.height, 50);
    }

    #[test]
    fn mock_engine_capture_fullscreen() {
        let engine = MockCaptureEngine::new();
        let result = engine.capture_fullscreen(1).unwrap();
        assert_eq!(result.width, 100);
        assert_eq!(result.height, 100);
    }

    #[test]
    fn mock_engine_capture_region() {
        let engine = MockCaptureEngine::new();
        let result = engine.capture_region(0, 0, 50, 50).unwrap();
        assert_eq!(result.width, 100); // Mock returns its fixed result
    }

    #[test]
    fn mock_engine_capture_window() {
        let engine = MockCaptureEngine::new();
        let result = engine.capture_window(100).unwrap();
        assert_eq!(result.width, 100);
    }

    #[test]
    fn mock_engine_list_displays() {
        let engine = MockCaptureEngine::new();
        let displays = engine.list_displays().unwrap();
        assert_eq!(displays.len(), 1);
        assert_eq!(displays[0].width, 1920);
    }

    #[test]
    fn mock_engine_list_windows() {
        let engine = MockCaptureEngine::new();
        let windows = engine.list_windows().unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].app_name, "MockApp");
    }

    #[test]
    fn mock_engine_implements_capture_engine_trait() {
        // Verify the mock can be used as a trait object
        let engine: Box<dyn CaptureEngine> = Box::new(MockCaptureEngine::new());
        let displays = engine.list_displays().unwrap();
        assert_eq!(displays.len(), 1);
    }
}
