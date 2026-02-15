//! Text input buffer with Emacs-style keybindings.

use std::path::PathBuf;

use crossterm::event::{
    KeyCode,
    KeyEvent,
    KeyModifiers,
};

/// Action to perform when inline input is confirmed.
#[derive(Debug, Clone)]
pub enum InputAction {
    /// Create a new file or directory at the given parent.
    Create {
        /// Parent directory for the new entry.
        parent_dir: PathBuf,
    },
    /// Rename the target file/directory.
    Rename {
        /// Path of the file/directory to rename.
        target: PathBuf,
    },
}

/// State for the inline text input field.
#[derive(Debug, Clone)]
pub struct InputState {
    /// Display prompt (e.g. "Create: " or "Rename: ").
    #[allow(dead_code, reason = "Displayed in UI input widget (pending)")]
    pub prompt: String,
    /// Current input text value.
    pub value: String,
    /// Cursor position within the value (byte offset).
    pub cursor_pos: usize,
    /// Action to perform on confirmation.
    pub on_confirm: InputAction,
}

impl InputState {
    /// Create a new input state for file creation.
    #[must_use]
    pub fn for_create(parent_dir: PathBuf) -> Self {
        Self {
            prompt: "Create: ".to_string(),
            value: String::new(),
            cursor_pos: 0,
            on_confirm: InputAction::Create { parent_dir },
        }
    }

    /// Create a new input state for renaming.
    ///
    /// Pre-fills the value with the existing name and places cursor before the extension.
    #[must_use]
    pub fn for_rename(target: PathBuf) -> Self {
        let name = target
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Place cursor before the extension (e.g. "file|.txt").
        let cursor_pos = name
            .rfind('.')
            .filter(|&pos| pos > 0)
            .unwrap_or(name.len());

        Self {
            prompt: "Rename: ".to_string(),
            value: name,
            cursor_pos,
            on_confirm: InputAction::Rename { target },
        }
    }

    /// Get the block title for display in the input box border.
    ///
    /// - Create: `"Create"`
    /// - Rename: `"Rename: original_name.ext"`
    #[must_use]
    pub fn title(&self) -> String {
        match &self.on_confirm {
            InputAction::Create { .. } => "Create".to_string(),
            InputAction::Rename { target } => {
                let name = target
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                format!("Rename: {name}")
            }
        }
    }

    /// Insert a character at the current cursor position.
    pub fn insert_char(&mut self, ch: char) {
        self.value.insert(self.cursor_pos, ch);
        self.cursor_pos += ch.len_utf8();
    }

    /// Delete the character before the cursor (Backspace).
    pub fn delete_char_backward(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let prev = self.prev_char_boundary();
        self.value.drain(prev..self.cursor_pos);
        self.cursor_pos = prev;
    }

    /// Delete the character at the cursor (Delete key).
    pub fn delete_char_forward(&mut self) {
        if self.cursor_pos >= self.value.len() {
            return;
        }
        let next = self.next_char_boundary();
        self.value.drain(self.cursor_pos..next);
    }

    /// Move cursor one character to the left.
    pub fn move_cursor_left(&mut self) {
        self.cursor_pos = self.prev_char_boundary();
    }

    /// Move cursor one character to the right.
    pub fn move_cursor_right(&mut self) {
        self.cursor_pos = self.next_char_boundary();
    }

    /// Move cursor to the beginning (Ctrl+A / Home).
    pub const fn move_cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move cursor to the end (Ctrl+E / End).
    pub const fn move_cursor_end(&mut self) {
        self.cursor_pos = self.value.len();
    }

    /// Clear text from cursor to the beginning (Ctrl+U).
    pub fn clear_to_start(&mut self) {
        self.value.drain(..self.cursor_pos);
        self.cursor_pos = 0;
    }

    /// Clear text from cursor to the end (Ctrl+K).
    pub fn clear_to_end(&mut self) {
        self.value.truncate(self.cursor_pos);
    }

    /// Move cursor one word backward (Alt+B).
    pub fn move_word_backward(&mut self) {
        self.cursor_pos = self.prev_word_boundary();
    }

    /// Move cursor one word forward (Alt+F).
    pub fn move_word_forward(&mut self) {
        self.cursor_pos = self.next_word_boundary();
    }

    /// Delete the word before the cursor (Ctrl+W).
    pub fn delete_word_backward(&mut self) {
        let boundary = self.prev_word_boundary();
        self.value.drain(boundary..self.cursor_pos);
        self.cursor_pos = boundary;
    }

    /// Delete the word after the cursor (Alt+D).
    pub fn delete_word_forward(&mut self) {
        let boundary = self.next_word_boundary();
        self.value.drain(self.cursor_pos..boundary);
    }

    /// Handle a key event, dispatching to the appropriate editing method.
    ///
    /// Returns `Some(true)` on Enter (confirm), `Some(false)` on Esc (cancel),
    /// `None` for regular editing keys.
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<bool> {
        match (key.code, key.modifiers) {
            (KeyCode::Enter, _) => Some(true),
            (KeyCode::Esc, _) => Some(false),
            (KeyCode::Backspace, _) | (KeyCode::Char('h'), KeyModifiers::CONTROL) => {
                self.delete_char_backward();
                None
            }
            (KeyCode::Delete, _) | (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                self.delete_char_forward();
                None
            }
            (KeyCode::Left, _) | (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                self.move_cursor_left();
                None
            }
            (KeyCode::Right, _) | (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
                self.move_cursor_right();
                None
            }
            (KeyCode::Home, _) | (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
                self.move_cursor_home();
                None
            }
            (KeyCode::End, _) | (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                self.move_cursor_end();
                None
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                self.clear_to_start();
                None
            }
            (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                self.clear_to_end();
                None
            }
            (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                self.delete_word_backward();
                None
            }
            (KeyCode::Char('b'), KeyModifiers::ALT) => {
                self.move_word_backward();
                None
            }
            (KeyCode::Char('f'), KeyModifiers::ALT) => {
                self.move_word_forward();
                None
            }
            (KeyCode::Char('d'), KeyModifiers::ALT) => {
                self.delete_word_forward();
                None
            }
            (KeyCode::Char(ch), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                self.insert_char(ch);
                None
            }
            _ => None,
        }
    }

    /// Find the byte position of the previous character boundary.
    fn prev_char_boundary(&self) -> usize {
        self.value.floor_char_boundary(self.cursor_pos.saturating_sub(1))
    }

    /// Find the byte position of the next character boundary.
    fn next_char_boundary(&self) -> usize {
        self.value.ceil_char_boundary(self.cursor_pos + 1)
    }

    /// Find the byte position of the previous word boundary.
    ///
    /// Walks backward: skips non-alphanumeric chars, then skips alphanumeric chars.
    fn prev_word_boundary(&self) -> usize {
        let before = &self.value[..self.cursor_pos];
        let mut chars = before.char_indices().rev().peekable();

        // Skip non-alphanumeric characters (whitespace, punctuation).
        while chars.peek().is_some_and(|(_, ch)| !ch.is_alphanumeric()) {
            chars.next();
        }

        // Skip alphanumeric characters (the word body).
        while chars.peek().is_some_and(|(_, ch)| ch.is_alphanumeric()) {
            chars.next();
        }

        // Return the byte position after the last consumed character,
        // or 0 if we consumed everything.
        chars
            .peek()
            .map_or(0, |&(i, ch)| i + ch.len_utf8())
    }

    /// Find the byte position of the next word boundary.
    ///
    /// Walks forward: skips alphanumeric chars, then skips non-alphanumeric chars.
    fn next_word_boundary(&self) -> usize {
        let after = &self.value[self.cursor_pos..];
        let mut chars = after.char_indices().peekable();

        // Skip alphanumeric characters (current word).
        while chars.peek().is_some_and(|(_, ch)| ch.is_alphanumeric()) {
            chars.next();
        }

        // Skip non-alphanumeric characters (whitespace, punctuation).
        while chars.peek().is_some_and(|(_, ch)| !ch.is_alphanumeric()) {
            chars.next();
        }

        // Return the byte position of the next word start,
        // or end of string if no more words.
        chars
            .peek()
            .map_or(self.value.len(), |&(i, _)| self.cursor_pos + i)
    }
}

/// Action to perform when confirmation dialog is accepted.
#[derive(Debug, Clone)]
pub enum ConfirmAction {
    /// Permanently delete files.
    PermanentDelete,
    /// Move files to custom trash (undo-able).
    CustomTrash,
    /// Send files to OS system trash.
    SystemTrash,
}

/// State for the confirmation dialog.
#[derive(Debug, Clone)]
pub struct ConfirmState {
    /// Dialog message.
    pub message: String,
    /// Paths of files to operate on.
    pub paths: Vec<PathBuf>,
    /// Action to perform on confirmation.
    pub on_confirm: ConfirmAction,
}

/// Application mode state machine.
#[derive(Debug, Clone, Default)]
pub enum AppMode {
    /// Normal tree navigation mode.
    #[default]
    Normal,
    /// Inline text input mode (create/rename).
    Input(InputState),
    /// Confirmation dialog mode (delete).
    Confirm(ConfirmState),
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    /// Create an `InputState` with the given value and cursor position for testing.
    fn input_with(value: &str, cursor_pos: usize) -> InputState {
        InputState {
            prompt: String::new(),
            value: value.to_string(),
            cursor_pos,
            on_confirm: InputAction::Create {
                parent_dir: PathBuf::from("/tmp"),
            },
        }
    }

    #[rstest]
    fn input_state_for_create_has_empty_value() {
        let state = InputState::for_create(PathBuf::from("/tmp"));
        assert_that!(state.value.as_str(), eq(""));
        assert_that!(state.cursor_pos, eq(0));
        assert_that!(state.prompt.as_str(), eq("Create: "));
    }

    #[rstest]
    fn input_state_for_rename_prefills_name() {
        let state = InputState::for_rename(PathBuf::from("/tmp/hello.txt"));
        assert_that!(state.value.as_str(), eq("hello.txt"));
        assert_that!(state.prompt.as_str(), eq("Rename: "));
    }

    #[rstest]
    fn input_state_for_rename_cursor_before_extension() {
        let state = InputState::for_rename(PathBuf::from("/tmp/document.pdf"));
        // "document" is 8 chars, cursor should be at position 8 (before ".pdf").
        assert_that!(state.cursor_pos, eq(8));
    }

    #[rstest]
    fn input_state_for_rename_no_extension() {
        let state = InputState::for_rename(PathBuf::from("/tmp/Makefile"));
        // No extension, cursor at end.
        assert_that!(state.cursor_pos, eq(8)); // "Makefile" length
    }

    #[rstest]
    fn input_state_for_rename_dotfile() {
        let state = InputState::for_rename(PathBuf::from("/tmp/.gitignore"));
        // ".gitignore" — stem is ".gitignore", no extension. cursor at end.
        assert_that!(state.cursor_pos, eq(10)); // ".gitignore" length
    }

    #[rstest]
    fn app_mode_default_is_normal() {
        let mode = AppMode::default();
        assert!(matches!(mode, AppMode::Normal));
    }

    #[rstest]
    fn insert_char_at_beginning() {
        let mut state = input_with("", 0);
        state.insert_char('h');
        state.insert_char('i');
        assert_that!(state.value.as_str(), eq("hi"));
        assert_that!(state.cursor_pos, eq(2));
    }

    #[rstest]
    fn insert_char_in_middle() {
        let mut state = InputState::for_rename(PathBuf::from("/tmp/ab.txt"));
        // cursor at 2 (before ".txt"), value is "ab.txt" → "ab|.txt"
        assert_that!(state.cursor_pos, eq(2));
        state.insert_char('c');
        assert_that!(state.value.as_str(), eq("abc.txt"));
        assert_that!(state.cursor_pos, eq(3));
    }

    #[rstest]
    fn delete_char_backward_removes_before_cursor() {
        let mut state = input_with("hello", 3); // hel|lo
        state.delete_char_backward();
        assert_that!(state.value.as_str(), eq("helo"));
        assert_that!(state.cursor_pos, eq(2));
    }

    #[rstest]
    fn delete_char_backward_at_beginning_is_noop() {
        let mut state = input_with("hello", 0);
        state.delete_char_backward();
        assert_that!(state.value.as_str(), eq("hello"));
        assert_that!(state.cursor_pos, eq(0));
    }

    #[rstest]
    fn delete_char_forward_removes_at_cursor() {
        let mut state = input_with("hello", 2); // he|llo
        state.delete_char_forward();
        assert_that!(state.value.as_str(), eq("helo"));
        assert_that!(state.cursor_pos, eq(2));
    }

    #[rstest]
    fn delete_char_forward_at_end_is_noop() {
        let mut state = input_with("hello", 5);
        state.delete_char_forward();
        assert_that!(state.value.as_str(), eq("hello"));
    }

    #[rstest]
    fn move_cursor_left_and_right() {
        let mut state = input_with("abc", 2);
        state.move_cursor_left();
        assert_that!(state.cursor_pos, eq(1));
        state.move_cursor_right();
        assert_that!(state.cursor_pos, eq(2));
    }

    #[rstest]
    fn move_cursor_home_and_end() {
        let mut state = input_with("hello", 3);
        state.move_cursor_home();
        assert_that!(state.cursor_pos, eq(0));
        state.move_cursor_end();
        assert_that!(state.cursor_pos, eq(5));
    }

    #[rstest]
    fn clear_to_start_removes_before_cursor() {
        let mut state = input_with("hello", 3); // hel|lo
        state.clear_to_start();
        assert_that!(state.value.as_str(), eq("lo"));
        assert_that!(state.cursor_pos, eq(0));
    }

    #[rstest]
    fn handle_key_enter_returns_confirm() {
        let mut state = input_with("", 0);
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(state.handle_key_event(key), Some(true));
    }

    #[rstest]
    fn handle_key_esc_returns_cancel() {
        let mut state = input_with("", 0);
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(state.handle_key_event(key), Some(false));
    }

    #[rstest]
    fn handle_key_char_inserts() {
        let mut state = input_with("", 0);
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        assert_eq!(state.handle_key_event(key), None);
        assert_that!(state.value.as_str(), eq("x"));
    }

    #[rstest]
    fn title_for_create_shows_create() {
        let state = InputState::for_create(PathBuf::from("/tmp"));
        assert_that!(state.title().as_str(), eq("Create"));
    }

    #[rstest]
    fn title_for_rename_shows_original_name() {
        let state = InputState::for_rename(PathBuf::from("/tmp/hello.txt"));
        assert_that!(state.title().as_str(), eq("Rename: hello.txt"));
    }

    #[rstest]
    fn unicode_multibyte_editing() {
        let mut state = input_with("", 0);
        state.insert_char('日');
        state.insert_char('本');
        assert_that!(state.value.as_str(), eq("日本"));
        assert_that!(state.cursor_pos, eq(6)); // 3 bytes per char
        state.move_cursor_left();
        assert_that!(state.cursor_pos, eq(3));
        state.delete_char_backward();
        assert_that!(state.value.as_str(), eq("本"));
        assert_that!(state.cursor_pos, eq(0));
    }

    #[rstest]
    fn clear_to_end_removes_after_cursor() {
        let mut state = input_with("hello world", 5); // hello| world
        state.clear_to_end();
        assert_that!(state.value.as_str(), eq("hello"));
        assert_that!(state.cursor_pos, eq(5));
    }

    #[rstest]
    fn clear_to_end_at_end_is_noop() {
        let mut state = input_with("hello", 5);
        state.clear_to_end();
        assert_that!(state.value.as_str(), eq("hello"));
    }

    #[rstest]
    fn delete_word_backward_removes_word() {
        let mut state = input_with("hello world", 11); // hello world|
        state.delete_word_backward();
        assert_that!(state.value.as_str(), eq("hello "));
        assert_that!(state.cursor_pos, eq(6));
    }

    #[rstest]
    fn delete_word_backward_at_beginning_is_noop() {
        let mut state = input_with("hello", 0);
        state.delete_word_backward();
        assert_that!(state.value.as_str(), eq("hello"));
        assert_that!(state.cursor_pos, eq(0));
    }

    #[rstest]
    fn delete_word_backward_with_spaces() {
        let mut state = input_with("foo  bar", 8); // foo  bar|
        state.delete_word_backward();
        assert_that!(state.value.as_str(), eq("foo  "));
        assert_that!(state.cursor_pos, eq(5));
    }

    #[rstest]
    fn delete_word_backward_skips_trailing_spaces() {
        let mut state = input_with("hello   ", 8); // hello   |
        state.delete_word_backward();
        assert_that!(state.value.as_str(), eq(""));
        assert_that!(state.cursor_pos, eq(0));
    }

    #[rstest]
    fn delete_word_forward_removes_word() {
        let mut state = input_with("hello world", 0); // |hello world
        state.delete_word_forward();
        assert_that!(state.value.as_str(), eq("world"));
        assert_that!(state.cursor_pos, eq(0));
    }

    #[rstest]
    fn delete_word_forward_at_end_is_noop() {
        let mut state = input_with("hello", 5);
        state.delete_word_forward();
        assert_that!(state.value.as_str(), eq("hello"));
    }

    #[rstest]
    fn move_word_backward_moves_to_word_start() {
        let mut state = input_with("hello world", 11); // hello world|
        state.move_word_backward();
        assert_that!(state.cursor_pos, eq(6)); // hello |world
    }

    #[rstest]
    fn move_word_backward_at_beginning_stays() {
        let mut state = input_with("hello", 0);
        state.move_word_backward();
        assert_that!(state.cursor_pos, eq(0));
    }

    #[rstest]
    fn move_word_forward_moves_to_next_word() {
        let mut state = input_with("hello world", 0); // |hello world
        state.move_word_forward();
        assert_that!(state.cursor_pos, eq(6)); // hello |world
    }

    #[rstest]
    fn move_word_forward_at_end_stays() {
        let mut state = input_with("hello", 5);
        state.move_word_forward();
        assert_that!(state.cursor_pos, eq(5));
    }

    #[rstest]
    fn handle_key_ctrl_w_deletes_word_backward() {
        let mut state = input_with("hello world", 11);
        let key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!(state.handle_key_event(key), None);
        assert_that!(state.value.as_str(), eq("hello "));
    }

    #[rstest]
    fn handle_key_ctrl_k_clears_to_end() {
        let mut state = input_with("hello world", 5);
        let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL);
        assert_eq!(state.handle_key_event(key), None);
        assert_that!(state.value.as_str(), eq("hello"));
    }

    #[rstest]
    fn handle_key_alt_b_moves_word_backward() {
        let mut state = input_with("hello world", 11);
        let key = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT);
        assert_eq!(state.handle_key_event(key), None);
        assert_that!(state.cursor_pos, eq(6));
    }

    #[rstest]
    fn handle_key_alt_f_moves_word_forward() {
        let mut state = input_with("hello world", 0);
        let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT);
        assert_eq!(state.handle_key_event(key), None);
        assert_that!(state.cursor_pos, eq(6));
    }

    #[rstest]
    fn handle_key_ctrl_b_moves_left() {
        let mut state = input_with("abc", 2);
        let key = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL);
        assert_eq!(state.handle_key_event(key), None);
        assert_that!(state.cursor_pos, eq(1));
    }

    #[rstest]
    fn handle_key_ctrl_f_moves_right() {
        let mut state = input_with("abc", 1);
        let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL);
        assert_eq!(state.handle_key_event(key), None);
        assert_that!(state.cursor_pos, eq(2));
    }

    #[rstest]
    fn move_word_with_punctuation() {
        let mut state = input_with("foo.bar baz", 0);
        state.move_word_forward();
        assert_that!(state.cursor_pos, eq(4)); // foo.|bar baz
    }
}
