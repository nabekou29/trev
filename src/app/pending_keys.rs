//! Pending key state for multi-key sequence bindings.
//!
//! Tracks accumulated key presses while waiting for a sequence to resolve,
//! and provides timeout detection for falling back to shorter bindings.

use std::time::{
    Duration,
    Instant,
};

use crossterm::event::{
    KeyCode,
    KeyModifiers,
};

use super::key_parse::KeyBinding;

/// Tracks pending key presses for multi-key sequence resolution.
#[derive(Debug)]
pub struct PendingKeys {
    /// Accumulated key presses.
    keys: Vec<KeyBinding>,
    /// When the first pending key was received.
    started_at: Option<Instant>,
    /// Maximum time to wait for the next key in a sequence.
    timeout: Duration,
}

impl PendingKeys {
    /// Create a new `PendingKeys` with the given timeout.
    pub const fn new(timeout: Duration) -> Self {
        Self { keys: Vec::new(), started_at: None, timeout }
    }

    /// Whether there are pending keys waiting for resolution.
    pub const fn is_pending(&self) -> bool {
        !self.keys.is_empty()
    }

    /// Whether the pending sequence has timed out.
    pub fn is_timed_out(&self) -> bool {
        self.started_at.is_some_and(|t| t.elapsed() >= self.timeout)
    }

    /// Remaining time until timeout, or zero if already timed out.
    pub fn remaining(&self) -> Duration {
        self.started_at.map_or(self.timeout, |t| self.timeout.saturating_sub(t.elapsed()))
    }

    /// Add a key to the pending sequence.
    pub fn push(&mut self, key: KeyBinding) {
        if self.keys.is_empty() {
            self.started_at = Some(Instant::now());
        }
        self.keys.push(key);
    }

    /// Get the current pending key sequence.
    pub fn keys(&self) -> &[KeyBinding] {
        &self.keys
    }

    /// Clear all pending keys and reset the timer.
    pub fn clear(&mut self) {
        self.keys.clear();
        self.started_at = None;
    }

    /// Format the pending keys for status bar display.
    ///
    /// Returns a string like `"z-"` for a single pending `z` key.
    pub fn display_string(&self) -> String {
        let mut s = String::new();
        for (code, mods) in &self.keys {
            format_key_binding(&mut s, *code, *mods);
        }
        s.push('-');
        s
    }
}

/// Format a key binding as a display string.
///
/// Convenience wrapper around [`format_key_binding`] that returns a new `String`.
pub fn key_display(code: KeyCode, mods: KeyModifiers) -> String {
    let mut s = String::new();
    format_key_binding(&mut s, code, mods);
    s
}

/// Format a single key binding, appending to an existing string.
///
/// Single plain characters are shown as-is (e.g. `j`).
/// Special keys and modifier combinations are wrapped in angle brackets
/// (e.g. `<Enter>`, `<C-a>`, `<S-Tab>`).
pub fn format_key_binding(s: &mut String, code: KeyCode, mods: KeyModifiers) {
    use std::fmt::Write;

    let has_mods = mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT);
    let is_plain_char = matches!(code, KeyCode::Char(_)) && !has_mods;

    if !is_plain_char {
        s.push('<');
    }
    if mods.contains(KeyModifiers::CONTROL) {
        s.push_str("C-");
    }
    if mods.contains(KeyModifiers::ALT) {
        s.push_str("A-");
    }
    if mods.contains(KeyModifiers::SHIFT) && !matches!(code, KeyCode::BackTab) {
        s.push_str("S-");
    }
    match code {
        KeyCode::Char(c) => s.push(c),
        KeyCode::Enter => s.push_str("Enter"),
        KeyCode::Esc => s.push_str("Esc"),
        KeyCode::Tab => s.push_str("Tab"),
        KeyCode::BackTab => s.push_str("S-Tab"),
        KeyCode::Backspace => s.push_str("BS"),
        KeyCode::Up => s.push_str("Up"),
        KeyCode::Down => s.push_str("Down"),
        KeyCode::Left => s.push_str("Left"),
        KeyCode::Right => s.push_str("Right"),
        KeyCode::F(n) => {
            s.push('F');
            let _ = write!(s, "{n}");
        }
        _ => s.push('?'),
    }
    if !is_plain_char {
        s.push('>');
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[rstest]
    fn new_pending_keys_is_not_pending() {
        let pk = PendingKeys::new(Duration::from_millis(500));
        assert!(!pk.is_pending());
        assert!(!pk.is_timed_out());
    }

    #[rstest]
    fn push_makes_pending() {
        let mut pk = PendingKeys::new(Duration::from_millis(500));
        pk.push((KeyCode::Char('z'), KeyModifiers::NONE));
        assert!(pk.is_pending());
        assert_that!(pk.keys().len(), eq(1));
    }

    #[rstest]
    fn clear_resets_pending() {
        let mut pk = PendingKeys::new(Duration::from_millis(500));
        pk.push((KeyCode::Char('z'), KeyModifiers::NONE));
        pk.clear();
        assert!(!pk.is_pending());
        assert!(pk.keys().is_empty());
    }

    #[rstest]
    fn display_string_single_key() {
        let mut pk = PendingKeys::new(Duration::from_millis(500));
        pk.push((KeyCode::Char('z'), KeyModifiers::NONE));
        assert_that!(pk.display_string().as_str(), eq("z-"));
    }

    #[rstest]
    fn display_string_two_keys() {
        let mut pk = PendingKeys::new(Duration::from_millis(500));
        pk.push((KeyCode::Char('g'), KeyModifiers::NONE));
        pk.push((KeyCode::Char('t'), KeyModifiers::NONE));
        assert_that!(pk.display_string().as_str(), eq("gt-"));
    }

    #[rstest]
    fn display_string_ctrl_key() {
        let mut pk = PendingKeys::new(Duration::from_millis(500));
        pk.push((KeyCode::Char('a'), KeyModifiers::CONTROL));
        assert_that!(pk.display_string().as_str(), eq("<C-a>-"));
    }

    #[rstest]
    fn timeout_with_zero_duration() {
        let mut pk = PendingKeys::new(Duration::ZERO);
        pk.push((KeyCode::Char('z'), KeyModifiers::NONE));
        assert!(pk.is_timed_out());
    }

    #[rstest]
    fn remaining_returns_full_timeout_when_not_started() {
        let pk = PendingKeys::new(Duration::from_millis(500));
        assert_that!(pk.remaining().as_millis(), eq(500));
    }
}
