//! Parse a settings-driven shortcut string (e.g. `"Alt+Shift+3"`) into a
//! Tauri `Shortcut`. The string syntax is checked by
//! `snapforge_core::validate_shortcut_str` at the settings boundary; this
//! module only handles the conversion to the runtime type.

use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

pub fn parse_shortcut(s: &str) -> Result<Shortcut, String> {
    snapforge_core::validate_shortcut_str(s)?;

    let parts: Vec<&str> = s.split('+').map(str::trim).collect();
    let key = parts[parts.len() - 1];
    let modifier_strs = &parts[..parts.len() - 1];

    let mut modifiers = Modifiers::empty();
    for raw in modifier_strs {
        modifiers |= modifier_from_str(raw)
            .ok_or_else(|| format!("Unknown modifier '{raw}' in shortcut '{s}'"))?;
    }

    let code =
        code_from_str(key).ok_or_else(|| format!("Unsupported key '{key}' in shortcut '{s}'"))?;

    Ok(Shortcut::new(Some(modifiers), code))
}

fn modifier_from_str(raw: &str) -> Option<Modifiers> {
    match raw {
        "Alt" | "Option" => Some(Modifiers::ALT),
        "Shift" => Some(Modifiers::SHIFT),
        "Ctrl" | "Control" => Some(Modifiers::CONTROL),
        "Cmd" | "Command" | "Super" => Some(Modifiers::SUPER),
        _ => None,
    }
}

fn code_from_str(raw: &str) -> Option<Code> {
    if raw.len() == 1 {
        let c = raw.as_bytes()[0].to_ascii_uppercase();
        return match c {
            b'A' => Some(Code::KeyA),
            b'B' => Some(Code::KeyB),
            b'C' => Some(Code::KeyC),
            b'D' => Some(Code::KeyD),
            b'E' => Some(Code::KeyE),
            b'F' => Some(Code::KeyF),
            b'G' => Some(Code::KeyG),
            b'H' => Some(Code::KeyH),
            b'I' => Some(Code::KeyI),
            b'J' => Some(Code::KeyJ),
            b'K' => Some(Code::KeyK),
            b'L' => Some(Code::KeyL),
            b'M' => Some(Code::KeyM),
            b'N' => Some(Code::KeyN),
            b'O' => Some(Code::KeyO),
            b'P' => Some(Code::KeyP),
            b'Q' => Some(Code::KeyQ),
            b'R' => Some(Code::KeyR),
            b'S' => Some(Code::KeyS),
            b'T' => Some(Code::KeyT),
            b'U' => Some(Code::KeyU),
            b'V' => Some(Code::KeyV),
            b'W' => Some(Code::KeyW),
            b'X' => Some(Code::KeyX),
            b'Y' => Some(Code::KeyY),
            b'Z' => Some(Code::KeyZ),
            b'0' => Some(Code::Digit0),
            b'1' => Some(Code::Digit1),
            b'2' => Some(Code::Digit2),
            b'3' => Some(Code::Digit3),
            b'4' => Some(Code::Digit4),
            b'5' => Some(Code::Digit5),
            b'6' => Some(Code::Digit6),
            b'7' => Some(Code::Digit7),
            b'8' => Some(Code::Digit8),
            b'9' => Some(Code::Digit9),
            _ => None,
        };
    }

    if let Some(num_str) = raw.strip_prefix('F') {
        if let Ok(n) = num_str.parse::<u8>() {
            return match n {
                1 => Some(Code::F1),
                2 => Some(Code::F2),
                3 => Some(Code::F3),
                4 => Some(Code::F4),
                5 => Some(Code::F5),
                6 => Some(Code::F6),
                7 => Some(Code::F7),
                8 => Some(Code::F8),
                9 => Some(Code::F9),
                10 => Some(Code::F10),
                11 => Some(Code::F11),
                12 => Some(Code::F12),
                _ => None,
            };
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alt_shift_digit() {
        let s = parse_shortcut("Alt+Shift+3").unwrap();
        assert_eq!(
            s,
            Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::Digit3)
        );
    }

    #[test]
    fn option_alias_maps_to_alt() {
        let s = parse_shortcut("Option+A").unwrap();
        assert_eq!(s, Shortcut::new(Some(Modifiers::ALT), Code::KeyA));
    }

    #[test]
    fn command_alias_maps_to_super() {
        let s = parse_shortcut("Command+A").unwrap();
        assert_eq!(s, Shortcut::new(Some(Modifiers::SUPER), Code::KeyA));
    }

    #[test]
    fn lowercase_letter_normalized() {
        let s = parse_shortcut("Alt+a").unwrap();
        assert_eq!(s, Shortcut::new(Some(Modifiers::ALT), Code::KeyA));
    }

    #[test]
    fn function_key() {
        let s = parse_shortcut("Ctrl+F12").unwrap();
        assert_eq!(s, Shortcut::new(Some(Modifiers::CONTROL), Code::F12));
    }

    #[test]
    fn macos_reserved_rejected() {
        // Delegates to validate_shortcut_str.
        assert!(parse_shortcut("Cmd+Shift+3").is_err());
    }

    #[test]
    fn empty_string_rejected() {
        assert!(parse_shortcut("").is_err());
    }
}
