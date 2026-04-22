use arboard::{Clipboard, ImageData};
use gaze_cli::{run_cli, CaptureBackend, CapturedImage, DisplayRecord, WindowRecord};
use std::borrow::Cow;
use std::process::Command;

struct RealCaptureBackend {
    engine: Box<dyn snapforge_capture::CaptureEngine>,
}

impl RealCaptureBackend {
    fn new() -> Self {
        Self {
            engine: snapforge_capture::create_engine(),
        }
    }
}

impl CaptureBackend for RealCaptureBackend {
    fn has_permission(&self) -> bool {
        snapforge_capture::has_permission()
    }

    fn list_displays(&self) -> Result<Vec<DisplayRecord>, String> {
        self.engine
            .list_displays()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|display| {
                Ok(DisplayRecord {
                    id: display.id,
                    name: display.name,
                    width: display.width,
                    height: display.height,
                    scale_factor: display.scale_factor,
                })
            })
            .collect()
    }

    fn list_windows(&self) -> Result<Vec<WindowRecord>, String> {
        self.engine
            .list_windows()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|window| {
                Ok(WindowRecord {
                    id: window.id,
                    title: window.title,
                    app_name: window.app_name,
                    is_on_screen: window.is_on_screen,
                })
            })
            .collect()
    }

    fn capture_fullscreen(&self, display_id: Option<u32>) -> Result<CapturedImage, String> {
        let display_id = match display_id {
            Some(display_id) => display_id,
            None => {
                self.engine
                    .list_displays()
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .next()
                    .ok_or_else(|| "No displays available".to_string())?
                    .id
            }
        };

        let capture = self
            .engine
            .capture_fullscreen(display_id)
            .map_err(|e| e.to_string())?;
        Ok(CapturedImage {
            width: capture.width,
            height: capture.height,
            rgba: capture.data,
        })
    }

    fn capture_window(&self, window_id: u32) -> Result<CapturedImage, String> {
        let capture = self
            .engine
            .capture_window(window_id)
            .map_err(|e| e.to_string())?;
        Ok(CapturedImage {
            width: capture.width,
            height: capture.height,
            rgba: capture.data,
        })
    }

    fn capture_area_interactive(&self) -> Result<Option<Vec<u8>>, String> {
        run_screencapture(&["-i", "-s", "-x"])
    }

    fn capture_window_interactive(&self) -> Result<Option<Vec<u8>>, String> {
        run_screencapture(&["-i", "-w", "-x"])
    }

    fn copy_rgba_to_clipboard(
        &self,
        rgba_data: &[u8],
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let image_data = ImageData {
            width: width as usize,
            height: height as usize,
            bytes: Cow::Borrowed(rgba_data),
        };

        let mut clipboard =
            Clipboard::new().map_err(|e| format!("Failed to open clipboard: {e}"))?;
        clipboard
            .set_image(image_data)
            .map_err(|e| format!("Failed to copy image to clipboard: {e}"))
    }
}

fn run_screencapture(args: &[&str]) -> Result<Option<Vec<u8>>, String> {
    let tmp_path = snapforge_core::temp_capture_path();
    let mut command_args: Vec<&str> = args.to_vec();
    let tmp_path_str = tmp_path.to_string_lossy();
    command_args.push(&tmp_path_str);

    let status = Command::new("screencapture")
        .args(&command_args)
        .status()
        .map_err(|e| format!("Failed to launch screencapture: {e}"))?;

    if !status.success() {
        return Ok(None);
    }

    let bytes = std::fs::read(&tmp_path);
    let _ = std::fs::remove_file(&tmp_path);
    match bytes {
        Ok(data) => Ok(Some(data)),
        Err(_) => Ok(None), // File absent = user cancelled
    }
}

fn main() {
    let backend = RealCaptureBackend::new();
    let code = run_cli(
        std::env::args_os(),
        &backend,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    );
    std::process::exit(i32::from(code));
}
