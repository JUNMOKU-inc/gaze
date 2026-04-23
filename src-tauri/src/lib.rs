mod capture_flow;
mod clipboard;
mod commands;
mod error;
mod logging;
pub(crate) mod preview_window;
mod recording;
mod settings;
mod shortcut;
mod state;
mod tray;

use state::AppState;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

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

/// Register the four capture shortcuts from the current settings.
///
/// Each shortcut is registered independently so a malformed or rejected entry
/// (e.g. a string that no longer parses, or an OS that refuses the binding)
/// only disables that one — the rest still work.
pub fn register_global_shortcuts(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let settings = crate::settings::load_settings(app);

    register_one(
        app,
        "fullscreen",
        &settings.shortcut_fullscreen,
        tray::trigger_fullscreen_capture,
    );
    register_one(
        app,
        "area",
        &settings.shortcut_area,
        tray::trigger_area_capture,
    );
    register_one(
        app,
        "window",
        &settings.shortcut_window,
        tray::trigger_window_capture,
    );
    register_one(
        app,
        "gif",
        &settings.shortcut_gif,
        tray::toggle_gif_recording,
    );

    Ok(())
}

fn register_one(
    app: &tauri::AppHandle,
    name: &'static str,
    spec: &str,
    trigger: fn(&tauri::AppHandle),
) {
    let parsed = match crate::shortcut::parse_shortcut(spec) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(name, spec, error = %e, "Invalid shortcut; skipping registration");
            return;
        }
    };

    let app_handle = app.clone();
    let result = app
        .global_shortcut()
        .on_shortcut(parsed, move |_app, sc, event| {
            if event.state == ShortcutState::Pressed {
                tracing::info!(name, ?sc, "Global shortcut fired");
                trigger(&app_handle);
            }
        });

    match result {
        Ok(()) => tracing::info!(name, spec, "Global shortcut registered"),
        Err(e) => tracing::warn!(name, spec, error = %e, "Failed to register shortcut"),
    }
}
