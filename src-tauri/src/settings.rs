use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager as _};

pub use snapforge_core::Settings;

/// How long the watcher coalesces rapid filesystem events. macOS FSEvents and
/// editors like VS Code often emit multiple events for a single save.
const WATCH_DEBOUNCE: Duration = Duration::from_millis(500);

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

/// Watch settings.json for external edits (e.g. `gaze settings set` from the
/// CLI) and reapply OS-level side effects whose state lives outside the file.
///
/// The fields read on every capture (preview position, save location, GIF
/// settings, etc.) reload themselves the next time they're consulted, so the
/// watcher only needs to handle:
/// - `launch_at_login`: backed by an OS login item, must be re-synced on
///   change
///
/// (`shortcut_*` will be added once registration becomes settings-driven.)
///
/// We watch the parent directory rather than the file itself: many editors
/// rename-on-save, and on Linux inotify watches the inode — a rename would
/// silently break a file-level watch.
pub fn spawn_settings_watcher(app: AppHandle) {
    let path = settings_path(&app);
    let parent = match path.parent() {
        Some(p) => p.to_path_buf(),
        None => {
            tracing::warn!(?path, "Settings path has no parent; watcher disabled");
            return;
        }
    };

    if let Err(e) = std::fs::create_dir_all(&parent) {
        tracing::warn!(?parent, error = %e, "Could not create settings dir; watcher disabled");
        return;
    }

    std::thread::Builder::new()
        .name("settings-watcher".into())
        .spawn(move || run_watcher_loop(app, path, parent))
        .map(|_| ())
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to spawn settings watcher thread");
        });
}

fn run_watcher_loop(app: AppHandle, settings_file: PathBuf, parent: PathBuf) {
    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

    let mut watcher: RecommendedWatcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(e) => {
            tracing::warn!(error = %e, "Could not initialize file watcher; CLI changes won't auto-apply");
            return;
        }
    };

    if let Err(e) = watcher.watch(&parent, RecursiveMode::NonRecursive) {
        tracing::warn!(?parent, error = %e, "Could not watch settings dir; CLI changes won't auto-apply");
        return;
    }

    tracing::info!(?settings_file, "Settings watcher started");

    let mut last_applied = load_settings(&app);
    let mut pending_since: Option<Instant> = None;

    loop {
        let timeout = match pending_since {
            Some(start) => WATCH_DEBOUNCE.saturating_sub(start.elapsed()),
            None => Duration::from_secs(60),
        };

        match rx.recv_timeout(timeout) {
            Ok(Ok(event)) if event_touches(&event, &settings_file) => {
                pending_since.get_or_insert_with(Instant::now);
            }
            Ok(Ok(_)) => {
                // Event in the dir but not on our file (e.g. sibling rename).
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "Settings watcher event error");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if pending_since
                    .take_if(|start| start.elapsed() >= WATCH_DEBOUNCE)
                    .is_some()
                {
                    apply_external_settings_change(&app, &mut last_applied);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                tracing::warn!("Settings watcher channel disconnected; stopping");
                return;
            }
        }
    }
}

fn event_touches(event: &Event, target: &std::path::Path) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) && event.paths.iter().any(|p| p == target)
}

fn apply_external_settings_change(app: &AppHandle, last_applied: &mut Settings) {
    // Take the same lock as `update_settings` so a concurrent UI write and an
    // external CLI write don't fight over OS state.
    let state = app.state::<crate::state::AppState>();
    let _guard = state.settings_mu.lock().unwrap_or_else(|e| e.into_inner());

    let new_settings = load_settings(app);

    if new_settings.launch_at_login != last_applied.launch_at_login {
        match apply_launch_at_login(new_settings.launch_at_login) {
            Ok(()) => tracing::info!(
                enabled = new_settings.launch_at_login,
                "launch_at_login synced from external settings change"
            ),
            Err(e) => tracing::warn!(error = %e, "Failed to sync launch_at_login"),
        }
    }

    *last_applied = new_settings;
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind, RemoveKind};
    use std::path::PathBuf;

    fn make_event(kind: EventKind, paths: Vec<PathBuf>) -> Event {
        Event {
            kind,
            paths,
            attrs: notify::event::EventAttributes::new(),
        }
    }

    #[test]
    fn event_touches_matches_target_modify() {
        let target = PathBuf::from("/tmp/settings.json");
        let event = make_event(
            EventKind::Modify(ModifyKind::Any),
            vec![target.clone(), PathBuf::from("/tmp/other.json")],
        );
        assert!(event_touches(&event, &target));
    }

    #[test]
    fn event_touches_matches_target_create() {
        let target = PathBuf::from("/tmp/settings.json");
        let event = make_event(EventKind::Create(CreateKind::File), vec![target.clone()]);
        assert!(event_touches(&event, &target));
    }

    #[test]
    fn event_touches_matches_target_remove() {
        let target = PathBuf::from("/tmp/settings.json");
        let event = make_event(EventKind::Remove(RemoveKind::File), vec![target.clone()]);
        assert!(event_touches(&event, &target));
    }

    #[test]
    fn event_touches_ignores_unrelated_path() {
        let target = PathBuf::from("/tmp/settings.json");
        let event = make_event(
            EventKind::Modify(ModifyKind::Any),
            vec![PathBuf::from("/tmp/other.json")],
        );
        assert!(!event_touches(&event, &target));
    }

    #[test]
    fn event_touches_ignores_access_kind() {
        // Access events (e.g. atime) are noisy and should not trigger reapply.
        let target = PathBuf::from("/tmp/settings.json");
        let event = make_event(
            EventKind::Access(notify::event::AccessKind::Read),
            vec![target.clone()],
        );
        assert!(!event_touches(&event, &target));
    }
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
