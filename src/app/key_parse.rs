//! Key notation parser for configuration keybindings.
//!
//! Parses human-readable key notations like `"ctrl+a"`, `"shift+enter"`, `"G"`
//! into crossterm `KeyCode` / `KeyModifiers` pairs.

use crossterm::event::{
    KeyCode,
    KeyModifiers,
};

/// A parsed key binding: `(KeyCode, KeyModifiers)`.
pub type KeyBinding = (KeyCode, KeyModifiers);

/// Parse a key notation string into `(KeyCode, KeyModifiers)`.
///
/// Supports formats:
/// - Single character: `"a"`, `"G"`, `"1"`, `"/"`
/// - Modifier + key: `"ctrl+a"`, `"alt+j"`, `"shift+enter"`
/// - Special keys: `"enter"`, `"space"`, `"tab"`, `"esc"`, `"backspace"`
/// - Arrow keys: `"up"`, `"down"`, `"left"`, `"right"`
/// - Function keys: `"f1"` through `"f12"`
pub fn parse_key(s: &str) -> Result<KeyBinding, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty key notation".to_string());
    }

    let mut modifiers = KeyModifiers::NONE;
    let mut remaining = s;

    // Parse modifier prefixes (ctrl+, alt+, shift+).
    loop {
        if let Some(rest) = remaining.strip_prefix("ctrl+") {
            modifiers |= KeyModifiers::CONTROL;
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("alt+") {
            modifiers |= KeyModifiers::ALT;
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("shift+") {
            modifiers |= KeyModifiers::SHIFT;
            remaining = rest;
        } else {
            break;
        }
    }

    let code = parse_key_code(remaining)?;

    // Uppercase single character implies SHIFT.
    if let KeyCode::Char(c) = code
        && c.is_ascii_uppercase()
    {
        modifiers |= KeyModifiers::SHIFT;
    }

    Ok((code, modifiers))
}

/// Parse expanded key bindings for uppercase letters.
///
/// Returns two bindings for uppercase letters: `(Char('G'), SHIFT)` and `(Char('G'), NONE)`,
/// because terminals inconsistently report SHIFT for uppercase letters.
/// For other keys, returns a single binding.
pub fn parse_key_expanded(s: &str) -> Result<Vec<KeyBinding>, String> {
    let (code, modifiers) = parse_key(s)?;

    if let KeyCode::Char(c) = code
        && c.is_ascii_uppercase()
        && modifiers == KeyModifiers::SHIFT
    {
        // Both SHIFT and NONE variants for terminal compatibility.
        return Ok(vec![(code, KeyModifiers::SHIFT), (code, KeyModifiers::NONE)]);
    }

    Ok(vec![(code, modifiers)])
}

/// Format a key binding as a human-readable string (for error messages).
#[allow(dead_code)]
pub fn format_key(code: KeyCode, modifiers: KeyModifiers) -> String {
    let mut parts = Vec::new();

    if modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("ctrl".to_string());
    }
    if modifiers.contains(KeyModifiers::ALT) {
        parts.push("alt".to_string());
    }
    if modifiers.contains(KeyModifiers::SHIFT) {
        parts.push("shift".to_string());
    }

    let key_str = match code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Esc => "esc".to_string(),
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::Delete => "delete".to_string(),
        KeyCode::Insert => "insert".to_string(),
        KeyCode::Home => "home".to_string(),
        KeyCode::End => "end".to_string(),
        KeyCode::PageUp => "pageup".to_string(),
        KeyCode::PageDown => "pagedown".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::F(n) => format!("f{n}"),
        _ => format!("{code:?}"),
    };

    parts.push(key_str);
    parts.join("+")
}

/// Parse a bare key code name (no modifiers).
fn parse_key_code(s: &str) -> Result<KeyCode, String> {
    // Single character.
    if s.len() == 1 {
        let c = s.chars().next().ok_or("empty key")?;
        return Ok(KeyCode::Char(c));
    }

    // Special keys (case-insensitive).
    match s.to_lowercase().as_str() {
        "enter" | "return" | "cr" => Ok(KeyCode::Enter),
        "space" => Ok(KeyCode::Char(' ')),
        "tab" => Ok(KeyCode::Tab),
        "esc" | "escape" => Ok(KeyCode::Esc),
        "backspace" | "bs" => Ok(KeyCode::Backspace),
        "delete" | "del" => Ok(KeyCode::Delete),
        "insert" | "ins" => Ok(KeyCode::Insert),
        "home" => Ok(KeyCode::Home),
        "end" => Ok(KeyCode::End),
        "pageup" | "pgup" => Ok(KeyCode::PageUp),
        "pagedown" | "pgdn" => Ok(KeyCode::PageDown),
        "up" => Ok(KeyCode::Up),
        "down" => Ok(KeyCode::Down),
        "left" => Ok(KeyCode::Left),
        "right" => Ok(KeyCode::Right),
        "f1" => Ok(KeyCode::F(1)),
        "f2" => Ok(KeyCode::F(2)),
        "f3" => Ok(KeyCode::F(3)),
        "f4" => Ok(KeyCode::F(4)),
        "f5" => Ok(KeyCode::F(5)),
        "f6" => Ok(KeyCode::F(6)),
        "f7" => Ok(KeyCode::F(7)),
        "f8" => Ok(KeyCode::F(8)),
        "f9" => Ok(KeyCode::F(9)),
        "f10" => Ok(KeyCode::F(10)),
        "f11" => Ok(KeyCode::F(11)),
        "f12" => Ok(KeyCode::F(12)),
        _ => Err(format!("unknown key: {s}")),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    // --- parse_key: single characters ---

    #[rstest]
    #[case("j", KeyCode::Char('j'), KeyModifiers::NONE)]
    #[case("k", KeyCode::Char('k'), KeyModifiers::NONE)]
    #[case("/", KeyCode::Char('/'), KeyModifiers::NONE)]
    #[case("1", KeyCode::Char('1'), KeyModifiers::NONE)]
    fn parse_single_char(
        #[case] input: &str,
        #[case] expected_code: KeyCode,
        #[case] expected_mods: KeyModifiers,
    ) {
        let (code, mods) = parse_key(input).unwrap();
        assert_eq!(code, expected_code);
        assert_eq!(mods, expected_mods);
    }

    // --- parse_key: uppercase implies SHIFT ---

    #[rstest]
    #[case("G", KeyCode::Char('G'), KeyModifiers::SHIFT)]
    #[case("J", KeyCode::Char('J'), KeyModifiers::SHIFT)]
    fn parse_uppercase_implies_shift(
        #[case] input: &str,
        #[case] expected_code: KeyCode,
        #[case] expected_mods: KeyModifiers,
    ) {
        let (code, mods) = parse_key(input).unwrap();
        assert_eq!(code, expected_code);
        assert_eq!(mods, expected_mods);
    }

    // --- parse_key: modifier + key ---

    #[rstest]
    #[case("ctrl+a", KeyCode::Char('a'), KeyModifiers::CONTROL)]
    #[case("alt+j", KeyCode::Char('j'), KeyModifiers::ALT)]
    #[case("ctrl+shift+a", KeyCode::Char('a'), KeyModifiers::CONTROL | KeyModifiers::SHIFT)]
    fn parse_modifier_key(
        #[case] input: &str,
        #[case] expected_code: KeyCode,
        #[case] expected_mods: KeyModifiers,
    ) {
        let (code, mods) = parse_key(input).unwrap();
        assert_eq!(code, expected_code);
        assert_eq!(mods, expected_mods);
    }

    // --- parse_key: special keys ---

    #[rstest]
    #[case("enter", KeyCode::Enter, KeyModifiers::NONE)]
    #[case("space", KeyCode::Char(' '), KeyModifiers::NONE)]
    #[case("tab", KeyCode::Tab, KeyModifiers::NONE)]
    #[case("esc", KeyCode::Esc, KeyModifiers::NONE)]
    #[case("backspace", KeyCode::Backspace, KeyModifiers::NONE)]
    #[case("up", KeyCode::Up, KeyModifiers::NONE)]
    #[case("down", KeyCode::Down, KeyModifiers::NONE)]
    #[case("left", KeyCode::Left, KeyModifiers::NONE)]
    #[case("right", KeyCode::Right, KeyModifiers::NONE)]
    #[case("f1", KeyCode::F(1), KeyModifiers::NONE)]
    #[case("f12", KeyCode::F(12), KeyModifiers::NONE)]
    fn parse_special_keys(
        #[case] input: &str,
        #[case] expected_code: KeyCode,
        #[case] expected_mods: KeyModifiers,
    ) {
        let (code, mods) = parse_key(input).unwrap();
        assert_eq!(code, expected_code);
        assert_eq!(mods, expected_mods);
    }

    // --- parse_key: modifier + special key ---

    #[rstest]
    fn parse_ctrl_enter() {
        let (code, mods) = parse_key("ctrl+enter").unwrap();
        assert_eq!(code, KeyCode::Enter);
        assert_eq!(mods, KeyModifiers::CONTROL);
    }

    // --- parse_key: errors ---

    #[rstest]
    #[case("")]
    #[case("ctrl+")]
    #[case("nonexistent")]
    fn parse_key_error(#[case] input: &str) {
        assert!(parse_key(input).is_err());
    }

    // --- parse_key_expanded: uppercase produces two bindings ---

    #[rstest]
    fn expanded_uppercase_produces_two_bindings() {
        let bindings = parse_key_expanded("G").unwrap();
        assert_that!(bindings.len(), eq(2));
        assert_eq!(bindings[0], (KeyCode::Char('G'), KeyModifiers::SHIFT));
        assert_eq!(bindings[1], (KeyCode::Char('G'), KeyModifiers::NONE));
    }

    #[rstest]
    fn expanded_lowercase_produces_one_binding() {
        let bindings = parse_key_expanded("j").unwrap();
        assert_that!(bindings.len(), eq(1));
        assert_eq!(bindings[0], (KeyCode::Char('j'), KeyModifiers::NONE));
    }

    #[rstest]
    fn expanded_special_key_produces_one_binding() {
        let bindings = parse_key_expanded("enter").unwrap();
        assert_that!(bindings.len(), eq(1));
    }

    // --- format_key ---

    #[rstest]
    #[case(KeyCode::Char('a'), KeyModifiers::NONE, "a")]
    #[case(KeyCode::Char('a'), KeyModifiers::CONTROL, "ctrl+a")]
    #[case(KeyCode::Enter, KeyModifiers::NONE, "enter")]
    #[case(KeyCode::F(1), KeyModifiers::ALT, "alt+f1")]
    fn format_key_display(
        #[case] code: KeyCode,
        #[case] mods: KeyModifiers,
        #[case] expected: &str,
    ) {
        assert_that!(format_key(code, mods).as_str(), eq(expected));
    }
}
