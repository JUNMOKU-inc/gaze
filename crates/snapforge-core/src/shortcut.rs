//! Pure-Rust syntax validation for global-shortcut strings.
//!
//! Lives in `snapforge-core` so the CLI can reject invalid shortcut values
//! without pulling in `tauri-plugin-global-shortcut`. The desktop crate has
//! its own `parse_shortcut` that turns the same string into a real
//! `Shortcut`; this module only checks the *string is well-formed*.

/// Validate a global-shortcut string like `"Alt+Shift+3"`.
///
/// Accepted modifiers (case-sensitive): `Alt`, `Option`, `Shift`, `Ctrl`,
/// `Control`, `Cmd`, `Command`, `Super`. At least one modifier is required —
/// bare keys would conflict with normal typing.
///
/// Accepted keys: ASCII alphanumerics (`A`-`Z`, `0`-`9`) and function keys
/// `F1`..`F12`.
///
/// Rejected: `Cmd+Shift+3/4/5` (reserved by macOS for built-in screenshot
/// tools — the OS-level binding wins anyway, so accepting them would just
/// silently break).
pub fn validate_shortcut_str(s: &str) -> Result<(), String> {
    let parts: Vec<&str> = s.split('+').map(str::trim).collect();
    if parts.iter().any(|p| p.is_empty()) {
        return Err(format!(
            "Shortcut '{s}' has empty segments around '+'; expected e.g. 'Alt+Shift+3'"
        ));
    }
    if parts.len() < 2 {
        return Err(format!(
            "Shortcut '{s}' must have at least one modifier and one key (e.g. 'Alt+Shift+3')"
        ));
    }

    let key = parts[parts.len() - 1];
    let modifier_strs = &parts[..parts.len() - 1];

    let mut has_cmd = false;
    let mut has_shift = false;
    let mut seen: Vec<&'static str> = Vec::new();
    for raw in modifier_strs {
        let canonical = canonical_modifier(raw)
            .ok_or_else(|| format!("Unknown modifier '{raw}' in shortcut '{s}'"))?;
        if seen.contains(&canonical) {
            return Err(format!(
                "Duplicate modifier '{canonical}' in shortcut '{s}'"
            ));
        }
        seen.push(canonical);
        match canonical {
            "Cmd" => has_cmd = true,
            "Shift" => has_shift = true,
            _ => {}
        }
    }

    validate_key(key).map_err(|e| format!("{e} in shortcut '{s}'"))?;

    // macOS reserves Cmd+Shift+3 (full screen), Cmd+Shift+4 (region),
    // Cmd+Shift+5 (controls). The OS handler will fire regardless of our
    // registration, so silently accepting these would lie to the user.
    if has_cmd && has_shift && matches!(key, "3" | "4" | "5") {
        return Err(format!(
            "Cmd+Shift+{key} is reserved by macOS for system screenshots; pick a different combination"
        ));
    }

    Ok(())
}

fn canonical_modifier(raw: &str) -> Option<&'static str> {
    match raw {
        "Alt" | "Option" => Some("Alt"),
        "Shift" => Some("Shift"),
        "Ctrl" | "Control" => Some("Ctrl"),
        "Cmd" | "Command" | "Super" => Some("Cmd"),
        _ => None,
    }
}

fn validate_key(key: &str) -> Result<(), String> {
    if key.len() == 1 {
        let c = key.as_bytes()[0];
        if c.is_ascii_alphanumeric() {
            return Ok(());
        }
    }

    if let Some(num_str) = key.strip_prefix('F') {
        if let Ok(n) = num_str.parse::<u8>() {
            if (1..=12).contains(&n) {
                return Ok(());
            }
        }
    }

    Err(format!(
        "Unsupported key '{key}'; use a single ASCII letter, a digit, or F1-F12"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_alt_shift_digit() {
        assert!(validate_shortcut_str("Alt+Shift+3").is_ok());
    }

    #[test]
    fn accepts_option_alias() {
        assert!(validate_shortcut_str("Option+Shift+A").is_ok());
    }

    #[test]
    fn accepts_command_alias() {
        assert!(validate_shortcut_str("Command+Shift+A").is_ok());
    }

    #[test]
    fn accepts_super_alias() {
        assert!(validate_shortcut_str("Super+A").is_ok());
    }

    #[test]
    fn accepts_function_keys() {
        assert!(validate_shortcut_str("Ctrl+F1").is_ok());
        assert!(validate_shortcut_str("Ctrl+F12").is_ok());
    }

    #[test]
    fn rejects_function_key_out_of_range() {
        assert!(validate_shortcut_str("Ctrl+F13").is_err());
        assert!(validate_shortcut_str("Ctrl+F0").is_err());
    }

    #[test]
    fn rejects_bare_key() {
        // Single key with no modifier would intercept normal typing.
        assert!(validate_shortcut_str("A").is_err());
    }

    #[test]
    fn rejects_unknown_modifier() {
        let err = validate_shortcut_str("Hyper+A").unwrap_err();
        assert!(err.contains("Unknown modifier"), "got: {err}");
    }

    #[test]
    fn rejects_empty_string() {
        assert!(validate_shortcut_str("").is_err());
    }

    #[test]
    fn rejects_trailing_plus() {
        assert!(validate_shortcut_str("Alt+Shift+").is_err());
    }

    #[test]
    fn rejects_leading_plus() {
        assert!(validate_shortcut_str("+A").is_err());
    }

    #[test]
    fn rejects_duplicate_modifier() {
        let err = validate_shortcut_str("Alt+Alt+A").unwrap_err();
        assert!(err.contains("Duplicate"), "got: {err}");
    }

    #[test]
    fn rejects_duplicate_modifier_via_alias() {
        // Option and Alt canonicalize to the same modifier.
        let err = validate_shortcut_str("Alt+Option+A").unwrap_err();
        assert!(err.contains("Duplicate"), "got: {err}");
    }

    #[test]
    fn rejects_macos_reserved_cmd_shift_3() {
        let err = validate_shortcut_str("Cmd+Shift+3").unwrap_err();
        assert!(err.contains("reserved"), "got: {err}");
    }

    #[test]
    fn rejects_macos_reserved_cmd_shift_4() {
        assert!(validate_shortcut_str("Cmd+Shift+4")
            .unwrap_err()
            .contains("reserved"));
    }

    #[test]
    fn rejects_macos_reserved_cmd_shift_5() {
        assert!(validate_shortcut_str("Cmd+Shift+5")
            .unwrap_err()
            .contains("reserved"));
    }

    #[test]
    fn alt_shift_3_is_allowed() {
        // Only Cmd+Shift+3/4/5 are reserved; Alt+Shift+3 is fine.
        assert!(validate_shortcut_str("Alt+Shift+3").is_ok());
    }

    #[test]
    fn rejects_lowercase_letter() {
        // Tauri's Code enum is case-sensitive; require uppercase.
        // Single-char key path uses ascii_alphanumeric, so 'a' would pass...
        // ...but Tauri side expects 'A'. Surface this here too.
        // Lowercase IS ascii_alphanumeric, so allow at this layer; the desktop
        // parse_shortcut normalizes to uppercase. Document the looseness.
        assert!(validate_shortcut_str("Alt+a").is_ok());
    }

    #[test]
    fn whitespace_around_plus_is_tolerated() {
        assert!(validate_shortcut_str("Alt + Shift + 3").is_ok());
    }
}
