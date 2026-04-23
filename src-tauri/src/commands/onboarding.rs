use tauri::{AppHandle, Manager as _};

use crate::error::CommandError;

/// Mark onboarding as completed, persist the change, and close the
/// onboarding window. Called from `onboarding.html` after the user reaches
/// the final step.
///
/// The window close happens on the Rust side because `window.close()` from
/// the WebView JS does not propagate to Tauri v2's window manager — the
/// window stays open even though the JS ran. Closing via the AppHandle
/// goes through the proper event loop.
#[tauri::command]
pub fn mark_onboarding_complete(app: AppHandle) -> Result<(), CommandError> {
    {
        let state = app.state::<crate::state::AppState>();
        let _guard = state.settings_mu.lock().unwrap_or_else(|e| e.into_inner());

        let mut settings = crate::settings::load_settings(&app);
        if !settings.onboarding_completed {
            settings.onboarding_completed = true;

            let path = app
                .path()
                .app_config_dir()
                .map_err(|e| CommandError {
                    message: format!("Could not resolve config dir: {e}"),
                    code: "config_dir_error".into(),
                })?
                .join("settings.json");

            snapforge_core::save_settings(&path, &settings).map_err(|e| CommandError {
                message: e.to_string(),
                code: "save_settings_error".into(),
            })?;
            tracing::info!("Onboarding marked complete");
        }
    }

    if let Some(window) = app.get_webview_window("onboarding") {
        if let Err(e) = window.close() {
            tracing::warn!(error = %e, "Failed to close onboarding window");
        }
    }

    Ok(())
}

/// Open macOS System Settings directly to the Screen Recording privacy pane.
/// The URL scheme works on macOS Ventura+ and silently no-ops on other OSes.
#[tauri::command]
pub fn open_screen_recording_settings() -> Result<(), CommandError> {
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
            .status()
            .map_err(|e| CommandError {
                message: format!("Failed to open System Settings: {e}"),
                code: "open_failed".into(),
            })?;
        if !status.success() {
            return Err(CommandError {
                message: format!("`open` exited with status {status}"),
                code: "open_failed".into(),
            });
        }
    }
    Ok(())
}
