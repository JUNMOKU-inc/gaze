use std::path::PathBuf;
use tauri::{AppHandle, Manager as _};

pub use snapforge_core::Settings;

fn settings_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("settings.json")
}

pub fn load_settings(app: &AppHandle) -> Settings {
    snapforge_core::load_settings_or_default(&settings_path(app))
}

fn save_settings(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = settings_path(app);
    snapforge_core::save_settings(&path, settings).map_err(|e| e.to_string())?;
    tracing::info!(?path, "Settings saved");
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_launch_at_login(enabled: bool) -> Result<(), String> {
    use std::process::Command;

    if enabled {
        let script = r#"tell application "System Events" to make login item at end with properties {path:"/Applications/Gaze.app", hidden:false}"#;
        let output = Command::new("osascript")
            .args(["-e", script])
            .output()
            .map_err(|e| format!("Failed to add login item: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("osascript failed to add login item: {stderr}"));
        }
    } else {
        let script = r#"tell application "System Events" to delete login item "Gaze""#;
        let output = Command::new("osascript")
            .args(["-e", script])
            .output()
            .map_err(|e| format!("Failed to remove login item: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Not an error if the item doesn't exist
            if !stderr.contains("Can't get login item") {
                return Err(format!("osascript failed to remove login item: {stderr}"));
            }
        }
    }
    Ok(())
}

/// Apply launch_at_login setting to the OS. Surfaces failure to the caller.
#[cfg(target_os = "macos")]
pub fn apply_launch_at_login(enabled: bool) -> Result<(), String> {
    set_launch_at_login(enabled)
}

/// Apply launch_at_login setting to the OS.
#[cfg(not(target_os = "macos"))]
pub fn apply_launch_at_login(_enabled: bool) -> Result<(), String> {
    // Not implemented for non-macOS yet
    Ok(())
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    load_settings(&app)
}

#[tauri::command]
pub fn update_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    // Serialize the entire read-apply-write sequence so concurrent invocations
    // (rapid UI toggles, multiple settings windows) can't race on the disk file
    // or leave AppState / OS state diverged.
    let state = app.state::<crate::state::AppState>();
    let _guard = state.settings_mu.lock().unwrap_or_else(|e| e.into_inner());

    let old_settings = load_settings(&app);

    // Persist first: if the disk write fails, OS state stays consistent with
    // what the user sees on next restart.
    save_settings(&app, &settings)?;

    // Apply launch-at-login change last, and surface failure to the caller so
    // the frontend can alert the user instead of silently diverging.
    if settings.launch_at_login != old_settings.launch_at_login {
        apply_launch_at_login(settings.launch_at_login)
            .map_err(|e| format!("Failed to update login item: {e}"))?;
    }

    Ok(())
}
