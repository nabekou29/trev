//! Key notation parser for configuration keybindings.
//!
//! Parses Vim-style key notations like `"<C-a>"`, `"<S-CR>"`, `"G"`
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
/// - Angle-bracket notation: `"<C-a>"`, `"<A-j>"`, `"<S-CR>"`
/// - Combined modifiers: `"<C-S-a>"`
/// - Special keys: `"<CR>"`, `"<Space>"`, `"<Tab>"`, `"<Esc>"`, `"<BS>"`
/// - Arrow keys: `"<Up>"`, `"<Down>"`, `"<Left>"`, `"<Right>"`
/// - Function keys: `"<F1>"` through `"<F12>"`
///
/// Modifier prefixes (inside `<...>`):
/// - `C-` — Control
/// - `A-` or `M-` — Alt (Meta)
/// - `S-` — Shift
pub fn parse_key(s: &str) -> Result<KeyBinding, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty key notation".to_string());
    }

    // <...> notation.
    if let Some(inner) = s.strip_prefix('<').and_then(|r| r.strip_suffix('>')) {
        if inner.is_empty() {
            return Err("empty angle-bracket notation".to_string());
        }
        return parse_angle_bracket(inner);
    }

    // Single character.
    if s.len() == 1 {
        let c = s.chars().next().ok_or("empty key")?;
        let mut modifiers = KeyModifiers::NONE;
        if c.is_ascii_uppercase() {
            modifiers |= KeyModifiers::SHIFT;
        }
        return Ok((KeyCode::Char(c), modifiers));
    }

    Err(format!("unknown key notation: {s}"))
}

/// Parse the content inside angle brackets `<...>`.
fn parse_angle_bracket(inner: &str) -> Result<KeyBinding, String> {
    let mut modifiers = KeyModifiers::NONE;
    let mut remaining = inner;

    // Parse modifier prefixes: C-, A-/M-, S-.
    loop {
        if let Some(rest) = remaining.strip_prefix("C-") {
            modifiers |= KeyModifiers::CONTROL;
            remaining = rest;
        } else if let Some(rest) =
            remaining.strip_prefix("A-").or_else(|| remaining.strip_prefix("M-"))
        {
            modifiers |= KeyModifiers::ALT;
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("S-") {
            modifiers |= KeyModifiers::SHIFT;
            remaining = rest;
        } else {
            break;
        }
    }

    // Single character after modifiers.
    if remaining.len() == 1 {
        let c = remaining.chars().next().ok_or("empty key")?;
        if c.is_ascii_uppercase() && !modifiers.contains(KeyModifiers::SHIFT) {
            modifiers |= KeyModifiers::SHIFT;
        }
        return Ok((KeyCode::Char(c), modifiers));
    }

    let code = parse_key_name(remaining)?;

    // S-Tab → BackTab (terminals send BackTab for Shift+Tab).
    if code == KeyCode::Tab && modifiers.contains(KeyModifiers::SHIFT) {
        return Ok((KeyCode::BackTab, modifiers));
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

/// Tokenize a key sequence string into individual key notation tokens.
///
/// Tokens are either angle-bracket groups (`<C-a>`, `<CR>`) or single characters.
/// Multi-character sequences without angle brackets are split per character:
/// `"zz"` → `["z", "z"]`, `"g<CR>"` → `["g", "<CR>"]`.
fn tokenize_key_sequence(s: &str) -> Result<Vec<String>, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty key sequence".to_string());
    }

    let mut tokens = Vec::new();
    let mut chars = s.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c == '<' {
            // Consume until matching '>'.
            let mut token = String::new();
            for ch in chars.by_ref() {
                token.push(ch);
                if ch == '>' {
                    break;
                }
            }
            if !token.ends_with('>') {
                return Err(format!("unclosed angle bracket in: {s}"));
            }
            tokens.push(token);
        } else if c.is_whitespace() {
            return Err(format!(
                "unexpected space in key sequence: {s:?} (use concatenated form, e.g. \"zz\" not \"z z\")"
            ));
        } else {
            tokens.push(c.to_string());
            chars.next();
        }
    }

    Ok(tokens)
}

/// Parse a key sequence string into a `Vec<KeyBinding>`.
///
/// For example, `"zz"` produces two bindings for `z` and `z`.
/// Single keys like `"j"` produce a one-element vector.
/// Angle-bracket tokens are supported: `"g<CR>"` → `g`, `Enter`.
#[cfg(test)]
pub fn parse_key_sequence(s: &str) -> Result<Vec<KeyBinding>, String> {
    let tokens = tokenize_key_sequence(s)?;
    let mut sequence = Vec::with_capacity(tokens.len());
    for token in &tokens {
        sequence.push(parse_key(token)?);
    }
    Ok(sequence)
}

/// Parse a key sequence with uppercase SHIFT expansion, producing all variant combinations.
///
/// Each key in the sequence is expanded via [`parse_key_expanded`], then a Cartesian product
/// is computed so that every terminal-compatible combination is covered.
///
/// For example, `"Gg"` expands to `[(G,SHIFT), g], [(G,NONE), g]`.
pub fn parse_key_sequence_expanded(s: &str) -> Result<Vec<Vec<KeyBinding>>, String> {
    let tokens = tokenize_key_sequence(s)?;
    let mut expanded_parts: Vec<Vec<KeyBinding>> = Vec::with_capacity(tokens.len());
    for token in &tokens {
        expanded_parts.push(parse_key_expanded(token)?);
    }

    // Compute Cartesian product.
    let mut result: Vec<Vec<KeyBinding>> = vec![Vec::new()];
    for variants in &expanded_parts {
        let mut new_result = Vec::with_capacity(result.len() * variants.len());
        for prefix in &result {
            for variant in variants {
                let mut seq = prefix.clone();
                seq.push(*variant);
                new_result.push(seq);
            }
        }
        result = new_result;
    }

    Ok(result)
}

/// Parse a special key name (case-insensitive).
fn parse_key_name(s: &str) -> Result<KeyCode, String> {
    match s.to_lowercase().as_str() {
        "cr" | "enter" | "return" => Ok(KeyCode::Enter),
        "space" => Ok(KeyCode::Char(' ')),
        "tab" => Ok(KeyCode::Tab),
        "esc" | "escape" => Ok(KeyCode::Esc),
        "bs" | "backspace" => Ok(KeyCode::Backspace),
        "del" | "delete" => Ok(KeyCode::Delete),
        "ins" | "insert" => Ok(KeyCode::Insert),
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

    // --- parse_key: angle-bracket modifier + key ---

    #[rstest]
    #[case("<C-a>", KeyCode::Char('a'), KeyModifiers::CONTROL)]
    #[case("<A-j>", KeyCode::Char('j'), KeyModifiers::ALT)]
    #[case("<M-j>", KeyCode::Char('j'), KeyModifiers::ALT)]
    #[case("<C-S-a>", KeyCode::Char('a'), KeyModifiers::CONTROL | KeyModifiers::SHIFT)]
    fn parse_modifier_key(
        #[case] input: &str,
        #[case] expected_code: KeyCode,
        #[case] expected_mods: KeyModifiers,
    ) {
        let (code, mods) = parse_key(input).unwrap();
        assert_eq!(code, expected_code);
        assert_eq!(mods, expected_mods);
    }

    // --- parse_key: uppercase in angle brackets implies SHIFT ---

    #[rstest]
    #[case("<C-G>", KeyCode::Char('G'), KeyModifiers::CONTROL | KeyModifiers::SHIFT)]
    fn parse_angle_uppercase_implies_shift(
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
    #[case("<CR>", KeyCode::Enter, KeyModifiers::NONE)]
    #[case("<Space>", KeyCode::Char(' '), KeyModifiers::NONE)]
    #[case("<Tab>", KeyCode::Tab, KeyModifiers::NONE)]
    #[case("<Esc>", KeyCode::Esc, KeyModifiers::NONE)]
    #[case("<BS>", KeyCode::Backspace, KeyModifiers::NONE)]
    #[case("<Up>", KeyCode::Up, KeyModifiers::NONE)]
    #[case("<Down>", KeyCode::Down, KeyModifiers::NONE)]
    #[case("<Left>", KeyCode::Left, KeyModifiers::NONE)]
    #[case("<Right>", KeyCode::Right, KeyModifiers::NONE)]
    #[case("<F1>", KeyCode::F(1), KeyModifiers::NONE)]
    #[case("<F12>", KeyCode::F(12), KeyModifiers::NONE)]
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
        let (code, mods) = parse_key("<C-CR>").unwrap();
        assert_eq!(code, KeyCode::Enter);
        assert_eq!(mods, KeyModifiers::CONTROL);
    }

    // --- parse_key: errors ---

    #[rstest]
    #[case("")]
    #[case("<>")]
    #[case("<C->")]
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
        let bindings = parse_key_expanded("<CR>").unwrap();
        assert_that!(bindings.len(), eq(1));
    }

    // --- parse_key_sequence: single key ---

    #[rstest]
    fn sequence_single_key() {
        let seq = parse_key_sequence("j").unwrap();
        assert_that!(seq.len(), eq(1));
        assert_eq!(seq[0], (KeyCode::Char('j'), KeyModifiers::NONE));
    }

    // --- parse_key_sequence: multi-key (space-free notation) ---

    #[rstest]
    fn sequence_two_keys() {
        let seq = parse_key_sequence("zz").unwrap();
        assert_that!(seq.len(), eq(2));
        assert_eq!(seq[0], (KeyCode::Char('z'), KeyModifiers::NONE));
        assert_eq!(seq[1], (KeyCode::Char('z'), KeyModifiers::NONE));
    }

    #[rstest]
    fn sequence_mixed_char_and_bracket() {
        let seq = parse_key_sequence("g<CR>").unwrap();
        assert_that!(seq.len(), eq(2));
        assert_eq!(seq[0], (KeyCode::Char('g'), KeyModifiers::NONE));
        assert_eq!(seq[1], (KeyCode::Enter, KeyModifiers::NONE));
    }

    #[rstest]
    fn sequence_three_chars() {
        let seq = parse_key_sequence("abc").unwrap();
        assert_that!(seq.len(), eq(3));
        assert_eq!(seq[0], (KeyCode::Char('a'), KeyModifiers::NONE));
        assert_eq!(seq[1], (KeyCode::Char('b'), KeyModifiers::NONE));
        assert_eq!(seq[2], (KeyCode::Char('c'), KeyModifiers::NONE));
    }

    // --- parse_key_sequence: errors ---

    #[rstest]
    fn sequence_empty_is_error() {
        assert!(parse_key_sequence("").is_err());
        assert!(parse_key_sequence("   ").is_err());
    }

    #[rstest]
    fn sequence_space_is_error() {
        assert!(parse_key_sequence("z z").is_err());
    }

    #[rstest]
    fn sequence_unclosed_bracket_is_error() {
        assert!(parse_key_sequence("z<CR").is_err());
    }

    // --- parse_key_sequence_expanded: lowercase produces single sequence ---

    #[rstest]
    fn sequence_expanded_lowercase_single() {
        let seqs = parse_key_sequence_expanded("zz").unwrap();
        assert_that!(seqs.len(), eq(1));
        assert_that!(seqs[0].len(), eq(2));
    }

    // --- parse_key_sequence_expanded: uppercase produces cartesian product ---

    #[rstest]
    fn sequence_expanded_uppercase_produces_product() {
        let seqs = parse_key_sequence_expanded("Gg").unwrap();
        // G expands to 2 variants, g to 1 → 2 * 1 = 2 sequences.
        assert_that!(seqs.len(), eq(2));
        assert_eq!(seqs[0][0], (KeyCode::Char('G'), KeyModifiers::SHIFT));
        assert_eq!(seqs[0][1], (KeyCode::Char('g'), KeyModifiers::NONE));
        assert_eq!(seqs[1][0], (KeyCode::Char('G'), KeyModifiers::NONE));
        assert_eq!(seqs[1][1], (KeyCode::Char('g'), KeyModifiers::NONE));
    }

    #[rstest]
    fn sequence_expanded_two_uppercase_produces_four() {
        let seqs = parse_key_sequence_expanded("GG").unwrap();
        // 2 * 2 = 4 sequences.
        assert_that!(seqs.len(), eq(4));
    }

    // --- parse_key_sequence_expanded: bracket in sequence ---

    #[rstest]
    fn sequence_expanded_with_bracket_key() {
        let seqs = parse_key_sequence_expanded("z<C-a>").unwrap();
        assert_that!(seqs.len(), eq(1));
        assert_that!(seqs[0].len(), eq(2));
        assert_eq!(seqs[0][0], (KeyCode::Char('z'), KeyModifiers::NONE));
        assert_eq!(seqs[0][1], (KeyCode::Char('a'), KeyModifiers::CONTROL));
    }
}
