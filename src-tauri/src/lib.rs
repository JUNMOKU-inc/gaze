mod capture_flow;
mod clipboard;
mod commands;
mod error;
mod logging;
pub(crate) mod preview_window;
mod recording;
mod settings;
mod state;
mod tray;

use state::AppState;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logging::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::capture::list_displays,
            commands::capture::list_windows,
            commands::capture::get_latest_capture,
            commands::capture::check_permission,
            commands::capture::request_permission,
            commands::capture::copy_image_from_base64,
            commands::capture::copy_text_to_clipboard,
            commands::capture::prepare_annotated_capture,
            commands::capture::copy_annotation_bundle,
            commands::capture::save_capture_to_desktop,
            commands::capture::save_gif_to_desktop,
            commands::capture::close_preview_by_label,
            commands::capture::resize_preview_window,
            commands::capture::get_screen_size,
            commands::capture::reposition_preview,
            commands::pipeline::optimize_for_llm,
            settings::get_settings,
            settings::update_settings,
        ])
        .setup(|app| {
            // Set up system tray
            tray::setup_tray(app.handle())?;

            // Hide app from dock (menu-bar only)
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            // Register global hotkeys
            register_global_shortcuts(app.handle())?;

            // Watch settings.json for external edits (CLI-driven changes etc.)
            crate::settings::spawn_settings_watcher(app.handle().clone());

            tracing::info!("Gaze started (menu-bar only mode)");
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            // Prevent app from exiting when all windows are closed.
            // Gaze is a menu-bar app — it stays alive via the system tray.
            // But allow explicit quit (tray "Quit" / app.exit()) to go through.
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    // Window-close triggered (no exit code) — keep app alive
                    api.prevent_exit();
                }
                // Explicit exit (code = Some) — let it proceed
            }
        });
}

fn register_global_shortcuts(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Use Option+Shift to avoid conflicting with macOS native Cmd+Shift+3/4 screenshots
    let fullscreen_shortcut = Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::Digit3);
    let area_shortcut = Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::Digit2);

    let app_handle = app.clone();
    app.global_shortcut()
        .on_shortcut(fullscreen_shortcut, move |_app, shortcut, event| {
            if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                tracing::info!(?shortcut, "Global shortcut: fullscreen capture");
                tray::trigger_fullscreen_capture(&app_handle);
            }
        })?;

    let app_handle = app.clone();
    app.global_shortcut()
        .on_shortcut(area_shortcut, move |_app, shortcut, event| {
            if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                tracing::info!(?shortcut, "Global shortcut: area capture");
                tray::trigger_area_capture(&app_handle);
            }
        })?;

    let window_shortcut = Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::Digit1);
    let app_handle = app.clone();
    app.global_shortcut()
        .on_shortcut(window_shortcut, move |_app, shortcut, event| {
            if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                tracing::info!(?shortcut, "Global shortcut: window capture");
                tray::trigger_window_capture(&app_handle);
            }
        })?;

    let gif_shortcut = Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::Digit5);
    let app_handle = app.clone();
    app.global_shortcut()
        .on_shortcut(gif_shortcut, move |_app, shortcut, event| {
            if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                tracing::info!(?shortcut, "Global shortcut: toggle GIF recording");
                tray::toggle_gif_recording(&app_handle);
            }
        })?;

    tracing::info!(
        "Global shortcuts registered: Option+Shift+1 (window), Option+Shift+2 (area), Option+Shift+3 (fullscreen), Option+Shift+5 (GIF record)"
    );
    Ok(())
}
