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

/// Parse a `default_provider` string into `LlmProvider`, or return a descriptive error.
fn parse_provider(s: &str) -> Result<snapforge_pipeline::LlmProvider, String> {
    serde_json::from_value::<snapforge_pipeline::LlmProvider>(serde_json::Value::String(
        s.to_string(),
    ))
    .map_err(|e| format!("invalid default_provider '{s}': {e}"))
}

/// Write the provider into AppState. Recovers from a poisoned lock rather than panicking.
pub fn sync_provider_to_state(app: &AppHandle, provider_str: &str) {
    let Ok(provider) = parse_provider(provider_str) else {
        tracing::warn!(
            value = provider_str,
            "default_provider in settings.json is unrecognised; keeping current AppState value"
        );
        return;
    };
    let state = app.state::<crate::state::AppState>();
    let mut guard = state
        .default_provider
        .write()
        .unwrap_or_else(|e| e.into_inner());
    *guard = provider;
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

    // Validate at the boundary so invalid values never reach disk or AppState.
    parse_provider(&settings.default_provider)?;

    let old_settings = load_settings(&app);

    // Persist first: if the disk write fails, AppState and the OS state stay
    // consistent with what the user sees on next restart.
    save_settings(&app, &settings)?;

    // Commit to in-memory state only after disk succeeded.
    sync_provider_to_state(&app, &settings.default_provider);

    // Apply launch-at-login change last, and surface failure to the caller so
    // the frontend can alert the user instead of silently diverging.
    if settings.launch_at_login != old_settings.launch_at_login {
        apply_launch_at_login(settings.launch_at_login)
            .map_err(|e| format!("Failed to update login item: {e}"))?;
    }

    Ok(())
}
