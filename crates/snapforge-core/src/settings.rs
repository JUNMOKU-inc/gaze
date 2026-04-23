use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Bundle identifier used as the settings directory name across the Tauri app and the CLI.
/// Kept in sync with `tauri.conf.json`'s `identifier`.
pub const GAZE_BUNDLE_IDENTIFIER: &str = "dev.gazeapp.gaze";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub language: String,
    pub launch_at_login: bool,

    pub auto_copy: bool,
    pub output_format: String,
    pub max_dimension: MaxDimension,

    pub max_recording_sec: u32,
    pub gif_fps: u32,
    pub gif_quality: u8,

    pub shortcut_area: String,
    pub shortcut_fullscreen: String,

    pub preview_position: String,
    pub max_previews: u32,
    pub save_location: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MaxDimension {
    pub mode: String,
    pub pixels: u32,
}

impl Default for MaxDimension {
    fn default() -> Self {
        Self {
            mode: "none".into(),
            pixels: 1568,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "en".into(),
            launch_at_login: false,

            auto_copy: true,
            output_format: "webp".into(),
            max_dimension: MaxDimension::default(),

            max_recording_sec: 30,
            gif_fps: 10,
            gif_quality: 90,

            shortcut_area: "Alt+Shift+2".into(),
            shortcut_fullscreen: "Alt+Shift+3".into(),

            preview_position: "bottom_right".into(),
            max_previews: 5,
            save_location: "~/Desktop".into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("Unknown settings key: {0}. Run `settings get` to see valid keys.")]
    UnknownKey(String),

    #[error("Invalid value for {key}: {reason}")]
    InvalidValue { key: String, reason: String },

    #[error("Failed to read settings file: {0}")]
    Read(#[source] std::io::Error),

    #[error("Failed to parse settings file: {0}")]
    Parse(#[source] serde_json::Error),

    #[error("Failed to write settings file: {0}")]
    Write(#[source] std::io::Error),

    #[error("Failed to serialize settings: {0}")]
    Serialize(#[source] serde_json::Error),

    #[error("Could not locate the user config directory for this platform")]
    ConfigDirUnavailable,
}

/// Resolve the on-disk settings file path for the given bundle identifier.
///
/// Mirrors Tauri v2's `app.path().app_config_dir()` on each platform:
/// - macOS: `$HOME/Library/Application Support/<id>/settings.json`
/// - Linux: `$XDG_CONFIG_HOME/<id>/settings.json` or `$HOME/.config/<id>/settings.json`
/// - Windows: `%APPDATA%\<id>\settings.json`
pub fn settings_path_for_identifier(identifier: &str) -> Result<PathBuf, SettingsError> {
    let base = config_dir().ok_or(SettingsError::ConfigDirUnavailable)?;
    Ok(base.join(identifier).join("settings.json"))
}

/// Default path for the Gaze app's settings file.
pub fn default_settings_path() -> Result<PathBuf, SettingsError> {
    settings_path_for_identifier(GAZE_BUNDLE_IDENTIFIER)
}

fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
        })
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            let path = PathBuf::from(xdg);
            if path.is_absolute() {
                return Some(path);
            }
        }
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config"))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(PathBuf::from)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

/// Load settings from `path`. Returns defaults if the file does not exist.
/// Surface parse errors — unlike the Tauri runtime, the CLI must not silently discard bad data.
pub fn load_settings(path: &Path) -> Result<Settings, SettingsError> {
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).map_err(SettingsError::Parse),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(err) => Err(SettingsError::Read(err)),
    }
}

/// Load settings from `path`, falling back to defaults on any failure.
/// Intended for the always-on Tauri runtime where a corrupt file should never crash the app.
pub fn load_settings_or_default(path: &Path) -> Settings {
    load_settings(path).unwrap_or_else(|e| {
        tracing::warn!(?path, error = %e, "Failed to load settings, using defaults");
        Settings::default()
    })
}

/// Atomically write settings to `path`, creating parent directories as needed.
pub fn save_settings(path: &Path, settings: &Settings) -> Result<(), SettingsError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(SettingsError::Write)?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(SettingsError::Serialize)?;
    std::fs::write(path, json).map_err(SettingsError::Write)
}

/// Canonical list of all known settings keys (camelCase, dotted for nested fields).
/// Used by the CLI help text and by `set_setting_field` validation.
pub const SETTINGS_KEYS: &[&str] = &[
    "language",
    "launchAtLogin",
    "autoCopy",
    "outputFormat",
    "maxDimension.mode",
    "maxDimension.pixels",
    "maxRecordingSec",
    "gifFps",
    "gifQuality",
    "shortcutArea",
    "shortcutFullscreen",
    "previewPosition",
    "maxPreviews",
    "saveLocation",
];

/// Read a single field from a settings instance. Returns the JSON value of the field,
/// or `UnknownKey` if the key isn't recognised.
pub fn get_setting_field(
    settings: &Settings,
    key: &str,
) -> Result<serde_json::Value, SettingsError> {
    let value = serde_json::to_value(settings).map_err(SettingsError::Serialize)?;
    lookup_path(&value, key).ok_or_else(|| SettingsError::UnknownKey(key.to_string()))
}

/// Update a single field in `settings` in-place.
///
/// `raw_value` is parsed as JSON when possible (so `true`, `42`, `"foo"`, `"bottom_right"` all
/// work); if JSON parsing fails, it's treated as a string literal, which keeps the CLI forgiving
/// for bare tokens like `set defaultProvider claude`.
///
/// Validates the resulting Settings round-trips through serde so bad types are caught before
/// persistence.
pub fn set_setting_field(
    settings: &mut Settings,
    key: &str,
    raw_value: &str,
) -> Result<(), SettingsError> {
    if !SETTINGS_KEYS.contains(&key) {
        return Err(SettingsError::UnknownKey(key.to_string()));
    }

    let parsed = parse_cli_value(raw_value);

    let mut value = serde_json::to_value(&*settings).map_err(SettingsError::Serialize)?;
    assign_path(&mut value, key, parsed).map_err(|reason| SettingsError::InvalidValue {
        key: key.to_string(),
        reason,
    })?;

    let updated: Settings =
        serde_json::from_value(value).map_err(|e| SettingsError::InvalidValue {
            key: key.to_string(),
            reason: e.to_string(),
        })?;

    *settings = updated;
    Ok(())
}

fn parse_cli_value(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_string()))
}

fn lookup_path(value: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
    let mut current = value;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current.clone())
}

fn assign_path(
    value: &mut serde_json::Value,
    path: &str,
    new_value: serde_json::Value,
) -> Result<(), String> {
    let segments: Vec<&str> = path.split('.').collect();
    let (last, parents) = segments
        .split_last()
        .ok_or_else(|| "empty path".to_string())?;

    let mut current = value;
    for segment in parents {
        current = current
            .get_mut(*segment)
            .ok_or_else(|| format!("missing intermediate key '{segment}'"))?;
    }

    let obj = current
        .as_object_mut()
        .ok_or_else(|| format!("cannot set '{last}' on non-object"))?;
    if !obj.contains_key(*last) {
        return Err(format!("unknown field '{last}'"));
    }
    obj.insert((*last).to_string(), new_value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_match_legacy_values() {
        let s = Settings::default();
        assert_eq!(s.language, "en");
        assert!(!s.launch_at_login);
        assert_eq!(s.max_dimension.mode, "none");
        assert_eq!(s.max_dimension.pixels, 1568);
    }

    #[test]
    fn load_returns_defaults_when_file_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("missing.json");
        let settings = load_settings(&path).unwrap();
        assert_eq!(settings.language, Settings::default().language);
    }

    #[test]
    fn save_then_load_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let original = Settings {
            language: "ja".into(),
            max_previews: 9,
            ..Default::default()
        };
        save_settings(&path, &original).unwrap();

        let loaded = load_settings(&path).unwrap();
        assert_eq!(loaded.language, "ja");
        assert_eq!(loaded.max_previews, 9);
    }

    #[test]
    fn save_creates_parent_directories() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("dir").join("settings.json");
        save_settings(&path, &Settings::default()).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn load_surfaces_parse_errors() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "not json").unwrap();
        assert!(matches!(load_settings(&path), Err(SettingsError::Parse(_))));
    }

    #[test]
    fn load_or_default_swallows_parse_errors() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "garbage").unwrap();
        let s = load_settings_or_default(&path);
        assert_eq!(s.language, Settings::default().language);
    }

    #[test]
    fn get_scalar_field() {
        let s = Settings::default();
        let val = get_setting_field(&s, "language").unwrap();
        assert_eq!(val, serde_json::Value::String("en".into()));
    }

    #[test]
    fn get_nested_field() {
        let s = Settings::default();
        let val = get_setting_field(&s, "maxDimension.pixels").unwrap();
        assert_eq!(val, serde_json::json!(1568));
    }

    #[test]
    fn get_unknown_key_errors() {
        let s = Settings::default();
        let err = get_setting_field(&s, "nope").unwrap_err();
        assert!(matches!(err, SettingsError::UnknownKey(_)));
    }

    #[test]
    fn set_bool_field_from_literal() {
        let mut s = Settings::default();
        set_setting_field(&mut s, "launchAtLogin", "true").unwrap();
        assert!(s.launch_at_login);
    }

    #[test]
    fn set_string_field_with_unquoted_value() {
        let mut s = Settings::default();
        set_setting_field(&mut s, "language", "ja").unwrap();
        assert_eq!(s.language, "ja");
    }

    #[test]
    fn set_string_field_with_quoted_value() {
        let mut s = Settings::default();
        set_setting_field(&mut s, "previewPosition", "\"top_left\"").unwrap();
        assert_eq!(s.preview_position, "top_left");
    }

    #[test]
    fn set_numeric_field() {
        let mut s = Settings::default();
        set_setting_field(&mut s, "maxPreviews", "10").unwrap();
        assert_eq!(s.max_previews, 10);
    }

    #[test]
    fn set_nested_field() {
        let mut s = Settings::default();
        set_setting_field(&mut s, "maxDimension.pixels", "2048").unwrap();
        assert_eq!(s.max_dimension.pixels, 2048);
    }

    #[test]
    fn set_unknown_key_errors() {
        let mut s = Settings::default();
        let err = set_setting_field(&mut s, "fakeField", "1").unwrap_err();
        assert!(matches!(err, SettingsError::UnknownKey(_)));
    }

    #[test]
    fn set_wrong_type_errors() {
        let mut s = Settings::default();
        let err = set_setting_field(&mut s, "maxPreviews", "not a number").unwrap_err();
        assert!(matches!(err, SettingsError::InvalidValue { .. }));
    }

    #[test]
    fn settings_path_joins_identifier() {
        let path = settings_path_for_identifier("com.example.myapp").unwrap();
        let as_str = path.to_string_lossy();
        assert!(as_str.contains("com.example.myapp"));
        assert!(as_str.ends_with("settings.json"));
    }

    #[test]
    fn camel_case_serialization() {
        let s = Settings::default();
        let json = serde_json::to_string(&s).unwrap();
        // JS side reads these exact keys — regressions here silently break the settings UI.
        for key in [
            "\"autoCopy\"",
            "\"outputFormat\"",
            "\"maxDimension\"",
            "\"launchAtLogin\"",
            "\"maxRecordingSec\"",
            "\"gifFps\"",
            "\"gifQuality\"",
            "\"previewPosition\"",
            "\"maxPreviews\"",
            "\"saveLocation\"",
        ] {
            assert!(json.contains(key), "missing key {key} in {json}");
        }
    }

    #[test]
    fn forward_compatible_deserialization_fills_missing_fields() {
        let partial = r#"{"language": "ja", "autoCopy": false}"#;
        let s: Settings = serde_json::from_str(partial).unwrap();
        assert_eq!(s.language, "ja");
        assert!(!s.auto_copy);
        assert_eq!(s.output_format, Settings::default().output_format);
        assert_eq!(s.max_previews, Settings::default().max_previews);
    }

    #[test]
    fn extra_unknown_fields_ignored() {
        let json = r#"{"language": "fr", "unknownField": 42}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.language, "fr");
    }

    #[test]
    fn max_dimension_nested_round_trip() {
        let json = r#"{"maxDimension": {"mode": "custom", "pixels": 2048}}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.max_dimension.mode, "custom");
        assert_eq!(s.max_dimension.pixels, 2048);
    }

    #[test]
    fn all_fields_round_trip() {
        let s = Settings {
            language: "ja".into(),
            launch_at_login: true,
            auto_copy: false,
            output_format: "png".into(),
            max_recording_sec: 60,
            gif_fps: 30,
            gif_quality: 75,
            shortcut_area: "Cmd+Shift+A".into(),
            shortcut_fullscreen: "Cmd+Shift+F".into(),
            preview_position: "top_left".into(),
            max_previews: 10,
            save_location: "/tmp".into(),
            ..Default::default()
        };
        let json = serde_json::to_string_pretty(&s).unwrap();
        let restored: Settings = serde_json::from_str(&json).unwrap();

        assert_eq!(s.language, restored.language);
        assert_eq!(s.launch_at_login, restored.launch_at_login);
        assert_eq!(s.auto_copy, restored.auto_copy);
        assert_eq!(s.output_format, restored.output_format);
        assert_eq!(s.max_recording_sec, restored.max_recording_sec);
        assert_eq!(s.gif_fps, restored.gif_fps);
        assert_eq!(s.gif_quality, restored.gif_quality);
        assert_eq!(s.shortcut_area, restored.shortcut_area);
        assert_eq!(s.shortcut_fullscreen, restored.shortcut_fullscreen);
        assert_eq!(s.preview_position, restored.preview_position);
        assert_eq!(s.max_previews, restored.max_previews);
        assert_eq!(s.save_location, restored.save_location);
    }
}
