use base64::Engine as _;
use snapforge_core::CaptureMetadata;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Manager as _, WebviewWindowBuilder,
};

use crate::capture_flow;
use crate::preview_window;
use crate::state::AppState;

/// Build and register the system tray with context menu.
pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let area_capture = MenuItemBuilder::with_id("area_capture", "Area Capture")
        .accelerator("Alt+Shift+2")
        .build(app)?;

    let window_capture = MenuItemBuilder::with_id("window_capture", "Window Capture")
        .accelerator("Alt+Shift+1")
        .build(app)?;

    let fullscreen_capture = MenuItemBuilder::with_id("fullscreen_capture", "Fullscreen Capture")
        .accelerator("Alt+Shift+3")
        .build(app)?;

    let ocr_capture = MenuItemBuilder::with_id("ocr_capture", "OCR Capture")
        .accelerator("Alt+Shift+4")
        .build(app)?;

    let gif_record = MenuItemBuilder::with_id("gif_record", "Record GIF")
        .accelerator("Alt+Shift+5")
        .build(app)?;

    let settings = MenuItemBuilder::with_id("settings", "Settings...")
        .accelerator("CmdOrCtrl+,")
        .build(app)?;

    let quit = MenuItemBuilder::with_id("quit", "Quit")
        .accelerator("CmdOrCtrl+Q")
        .build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&area_capture)
        .item(&window_capture)
        .item(&fullscreen_capture)
        .item(&ocr_capture)
        .item(&gif_record)
        .separator()
        .item(&settings)
        .separator()
        .item(&quit)
        .build()?;

    TrayIconBuilder::new()
        .icon(tauri::image::Image::from_bytes(include_bytes!(
            "../icons/tray-template-32.png"
        ))?)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| {
            handle_menu_event(app, event.id().as_ref());
        })
        .build(app)?;

    tracing::info!("System tray initialized");
    Ok(())
}

fn handle_menu_event(app: &AppHandle, menu_id: &str) {
    match menu_id {
        "fullscreen_capture" => {
            tracing::info!("Tray: fullscreen capture requested");
            trigger_fullscreen_capture(app);
        }
        "area_capture" => {
            tracing::info!("Tray: area capture requested");
            trigger_area_capture(app);
        }
        "window_capture" => {
            tracing::info!("Tray: window capture requested");
            trigger_window_capture(app);
        }
        "ocr_capture" => {
            tracing::info!("Tray: OCR capture requested (not yet implemented)");
        }
        "gif_record" => {
            tracing::info!("Tray: GIF record toggle requested");
            toggle_gif_recording(app);
        }
        "settings" => {
            tracing::info!("Tray: settings requested");
            open_settings_window(app);
        }
        "quit" => {
            tracing::info!("Tray: quit requested");
            app.exit(0);
        }
        other => {
            tracing::warn!(menu_id = other, "Unknown tray menu event");
        }
    }
}

fn open_settings_window(app: &AppHandle) {
    // If the settings window already exists, just focus it.
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.set_focus();
        return;
    }

    match WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App("settings.html".into()),
    )
    .title("Gaze Settings")
    .inner_size(560.0, 520.0)
    .decorations(true)
    .resizable(false)
    .center()
    .focused(true)
    .build()
    {
        Ok(_) => tracing::info!("Settings window opened"),
        Err(e) => tracing::error!("Failed to open settings window: {e}"),
    }
}

/// Show preview on the main thread (required for WebviewWindow creation in Tauri v2).
fn show_preview_on_main_thread(app: &AppHandle, metadata: CaptureMetadata) {
    let app_handle = app.clone();
    if let Err(e) = app.run_on_main_thread(move || {
        preview_window::show_preview(&app_handle, &metadata);
    }) {
        tracing::error!("Failed to dispatch show_preview to main thread: {e}");
    }
}

/// Trigger area capture. Runs in a background thread since screencapture blocks.
pub fn trigger_area_capture(app: &AppHandle) {
    if !preview_window::debounce_check() {
        return;
    }

    let auto_copy = crate::settings::load_settings(app).auto_copy;
    let app_handle = app.clone();
    std::thread::spawn(
        move || match capture_flow::execute_area_capture(auto_copy) {
            Ok(Some(metadata)) => {
                show_preview_on_main_thread(&app_handle, metadata);
            }
            Ok(None) => {
                tracing::info!("Area capture cancelled by user");
            }
            Err(e) => {
                tracing::error!("Area capture failed: {e}");
            }
        },
    );
}

/// Trigger window capture. Runs in a background thread since screencapture blocks.
pub fn trigger_window_capture(app: &AppHandle) {
    if !preview_window::debounce_check() {
        return;
    }

    let auto_copy = crate::settings::load_settings(app).auto_copy;
    let app_handle = app.clone();
    std::thread::spawn(
        move || match capture_flow::execute_window_capture(auto_copy) {
            Ok(Some(metadata)) => {
                show_preview_on_main_thread(&app_handle, metadata);
            }
            Ok(None) => {
                tracing::info!("Window capture cancelled by user");
            }
            Err(e) => {
                tracing::error!("Window capture failed: {e}");
            }
        },
    );
}

/// Trigger fullscreen capture. Runs in a background thread for consistency.
pub fn trigger_fullscreen_capture(app: &AppHandle) {
    if !preview_window::debounce_check() {
        return;
    }

    let auto_copy = crate::settings::load_settings(app).auto_copy;
    let app_handle = app.clone();
    std::thread::spawn(
        move || match capture_flow::execute_fullscreen_capture(auto_copy) {
            Ok(Some(metadata)) => {
                show_preview_on_main_thread(&app_handle, metadata);
            }
            Ok(None) => {
                tracing::error!("Fullscreen capture produced no output");
            }
            Err(e) => {
                tracing::error!("Fullscreen capture failed: {e}");
            }
        },
    );
}

/// Play a macOS system sound for audio feedback.
#[cfg(target_os = "macos")]
fn play_system_sound(name: &str) {
    // Fire and forget — don't block on sound playback
    let name = name.to_string();
    std::thread::spawn(move || {
        let _ = std::process::Command::new("afplay")
            .arg(format!("/System/Library/Sounds/{name}.aiff"))
            .output();
    });
}

/// Toggle GIF recording. If not recording, start. If recording, stop and show preview.
pub fn toggle_gif_recording(app: &AppHandle) {
    // Debounce: global shortcuts can fire multiple times per keypress
    static LAST_TOGGLE: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
    {
        let mut last = LAST_TOGGLE.lock().unwrap();
        let now = std::time::Instant::now();
        if let Some(prev) = *last {
            if now.duration_since(prev) < std::time::Duration::from_millis(500) {
                tracing::debug!("GIF toggle debounced");
                return;
            }
        }
        *last = Some(now);
    }

    let state = app.state::<AppState>();
    let mut recording_guard = state.recording.lock().unwrap_or_else(|e| {
        tracing::warn!("Recording lock poisoned: {e}");
        e.into_inner()
    });

    if recording_guard.is_some() {
        // Stop recording
        let handle = recording_guard.take().unwrap();
        play_system_sound("Hero");
        // Hide indicator on main thread
        let app_for_hide = app.clone();
        let _ = app.run_on_main_thread(move || {
            crate::preview_window::hide_recording_indicator(&app_for_hide);
        });
        let auto_copy = crate::settings::load_settings(app).auto_copy;
        let app_handle = app.clone();
        std::thread::spawn(move || {
            match crate::recording::stop_recording(handle) {
                Ok(result) => {
                    tracing::info!(
                        path = %result.gif_path.display(),
                        frames = result.frame_count,
                        size = result.file_size,
                        "GIF recording saved"
                    );

                    // Read GIF and prepare preview data
                    if let Ok(gif_bytes) = std::fs::read(&result.gif_path) {
                        // Copy to clipboard
                        if auto_copy {
                            if let Err(e) = crate::clipboard::copy_encoded_to_clipboard(&gif_bytes)
                            {
                                tracing::warn!("Failed to copy GIF to clipboard: {e}");
                            }
                        }

                        // Show GIF preview
                        let gif_base64 =
                            base64::engine::general_purpose::STANDARD.encode(&gif_bytes);
                        let preview_data = crate::preview_window::GifPreviewData {
                            media_type: "gif".to_string(),
                            original_width: result.width,
                            original_height: result.height,
                            optimized_width: result.width,
                            optimized_height: result.height,
                            file_size: result.file_size as usize,
                            timestamp: chrono::Local::now().to_rfc3339(),
                            image_base64: String::new(),
                            gif_base64,
                            frame_count: result.frame_count,
                            duration_secs: result.duration_secs,
                        };
                        let app_for_preview = app_handle.clone();
                        let _ = app_handle.run_on_main_thread(move || {
                            crate::preview_window::show_gif_preview(
                                &app_for_preview,
                                &preview_data,
                            );
                        });
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to stop recording: {e}");
                }
            }
        });
    } else {
        // Start recording
        let settings = crate::settings::load_settings(app);

        // Get main display ID
        let display_id = state
            .capture_engine
            .list_displays()
            .ok()
            .and_then(|d| d.first().map(|d| d.id))
            .unwrap_or(1);

        let config = crate::recording::RecordingConfig {
            display_id,
            fps: settings.gif_fps,
            max_width: 640,
            max_duration_secs: settings.max_recording_sec,
            quality: settings.gif_quality,
            show_cursor: true,
        };

        match crate::recording::start_recording(config) {
            Ok(handle) => {
                *recording_guard = Some(handle);
                tracing::info!("GIF recording started");
                play_system_sound("Submarine");
                // Show recording indicator on main thread
                let app_for_indicator = app.clone();
                let _ = app.run_on_main_thread(move || {
                    crate::preview_window::show_recording_indicator(&app_for_indicator);
                });
            }
            Err(e) => {
                tracing::error!("Failed to start recording: {e}");
            }
        }
    }
}
