//! Text input buffer with Emacs-style keybindings.

use std::path::PathBuf;

use crossterm::event::{
    KeyCode,
    KeyEvent,
    KeyModifiers,
};

/// Reusable text buffer with cursor and Emacs-style editing.
///
/// Used by [`InputState`] (create/rename) and search input.
#[derive(Debug, Clone)]
pub struct TextBuffer {
    /// Current text value.
    pub value: String,
    /// Cursor position within the value (byte offset).
    pub cursor_pos: usize,
}

impl TextBuffer {
    /// Create a new, empty text buffer.
    #[must_use]
    pub const fn new() -> Self {
        Self { value: String::new(), cursor_pos: 0 }
    }

    /// Create a text buffer with initial value and cursor position.
    #[must_use]
    pub const fn with_value(value: String, cursor_pos: usize) -> Self {
        Self { value, cursor_pos }
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

    /// Replace the buffer value and move the cursor to the end.
    pub fn set_value(&mut self, value: &str) {
        self.value.clone_from(&value.to_string());
        self.cursor_pos = self.value.len();
    }

    /// Push spans for the buffer value with an inverted-block cursor.
    ///
    /// Splits the text at the cursor position and renders the character
    /// under the cursor (or a trailing space) with `bg:White fg:Black`.
    pub fn push_cursor_spans(&self, spans: &mut Vec<ratatui::text::Span<'_>>) {
        use ratatui::style::{
            Color,
            Style,
        };
        use ratatui::text::Span;

        let (before, after) = self.value.split_at(self.cursor_pos);
        spans.push(Span::raw(before.to_string()));

        let cursor_style = Style::default().bg(Color::White).fg(Color::Black);
        let mut after_chars = after.chars();
        if let Some(cursor_char) = after_chars.next() {
            spans.push(Span::styled(cursor_char.to_string(), cursor_style));
            let rest: String = after_chars.collect();
            if !rest.is_empty() {
                spans.push(Span::raw(rest));
            }
        } else {
            spans.push(Span::styled(" ", cursor_style));
        }
    }

    /// Handle a key event for text editing only.
    ///
    /// Returns `true` if the key was consumed as an editing action,
    /// `false` if it was not handled (caller should process it).
    pub fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Backspace, _) | (KeyCode::Char('h'), KeyModifiers::CONTROL) => {
                self.delete_char_backward();
                true
            }
            (KeyCode::Delete, _) | (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                self.delete_char_forward();
                true
            }
            (KeyCode::Left, _) | (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                self.move_cursor_left();
                true
            }
            (KeyCode::Right, _) | (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
                self.move_cursor_right();
                true
            }
            (KeyCode::Home, _) | (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
                self.move_cursor_home();
                true
            }
            (KeyCode::End, _) | (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                self.move_cursor_end();
                true
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                self.clear_to_start();
                true
            }
            (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                self.clear_to_end();
                true
            }
            (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                self.delete_word_backward();
                true
            }
            (KeyCode::Char('b'), KeyModifiers::ALT) => {
                self.move_word_backward();
                true
            }
            (KeyCode::Char('f'), KeyModifiers::ALT) => {
                self.move_word_forward();
                true
            }
            (KeyCode::Char('d'), KeyModifiers::ALT) => {
                self.delete_word_forward();
                true
            }
            (KeyCode::Char(ch), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                self.insert_char(ch);
                true
            }
            _ => false,
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
        chars.peek().map_or(0, |&(i, ch)| i + ch.len_utf8())
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
        chars.peek().map_or(self.value.len(), |&(i, _)| self.cursor_pos + i)
    }
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Action to perform when inline input is confirmed.
#[derive(Debug, Clone)]
pub enum InputAction {
    /// Create a new file or directory at the given parent.
    Create {
        /// Parent directory for the new entry.
        parent_dir: PathBuf,
    },
    /// Create a new directory at the given parent.
    CreateDirectory {
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
    pub prompt: String,
    /// Text buffer with cursor and editing support.
    pub buffer: TextBuffer,
    /// Action to perform on confirmation.
    pub on_confirm: InputAction,
}

impl InputState {
    /// Create a new input state for file creation.
    #[must_use]
    pub fn for_create(parent_dir: PathBuf) -> Self {
        Self {
            prompt: "Create: ".to_string(),
            buffer: TextBuffer::new(),
            on_confirm: InputAction::Create { parent_dir },
        }
    }

    /// Create a new input state for directory creation.
    #[must_use]
    pub fn for_create_directory(parent_dir: PathBuf) -> Self {
        Self {
            prompt: "Create directory: ".to_string(),
            buffer: TextBuffer::new(),
            on_confirm: InputAction::CreateDirectory { parent_dir },
        }
    }

    /// Create a new input state for renaming.
    ///
    /// Pre-fills the value with the existing name and places cursor before the extension.
    #[must_use]
    pub fn for_rename(target: PathBuf) -> Self {
        let name = target.file_name().unwrap_or_default().to_string_lossy().to_string();

        // Place cursor before the extension (e.g. "file|.txt").
        let cursor_pos = name.rfind('.').filter(|&pos| pos > 0).unwrap_or(name.len());

        Self {
            prompt: "Rename: ".to_string(),
            buffer: TextBuffer::with_value(name, cursor_pos),
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
            InputAction::CreateDirectory { .. } => "Create directory".to_string(),
            InputAction::Rename { target } => {
                let name = target.file_name().unwrap_or_default().to_string_lossy();
                format!("Rename: {name}")
            }
        }
    }

    /// Handle a key event, dispatching to the appropriate editing method.
    ///
    /// Returns `Some(true)` on Enter (confirm), `Some(false)` on Esc (cancel),
    /// `None` for regular editing keys.
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<bool> {
        match key.code {
            KeyCode::Enter => Some(true),
            KeyCode::Esc => Some(false),
            _ => {
                self.buffer.handle_key_event(key);
                None
            }
        }
    }
}

/// Action to perform when confirmation dialog is accepted.
#[derive(Debug, Clone, Copy)]
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

/// A single item in a selection menu.
#[derive(Debug, Clone)]
pub struct MenuItem {
    /// Shortcut key to select this item.
    pub key: char,
    /// Display label for the item.
    pub label: String,
    /// Value associated with the item (e.g. text to copy).
    pub value: String,
}

/// Action to perform when a menu item is selected.
#[derive(Debug, Clone, Copy)]
pub enum MenuAction {
    /// Copy the selected item's value to clipboard.
    CopyToClipboard,
    /// Apply the selected item's value as the new sort order.
    SelectSortOrder,
    /// Execute the resolved action from a user-defined menu.
    Custom,
}

/// State for a selection menu overlay.
#[derive(Debug, Clone)]
pub struct MenuState {
    /// Menu title displayed in the border.
    pub title: String,
    /// Available menu items.
    pub items: Vec<MenuItem>,
    /// Currently highlighted item index.
    pub cursor: usize,
    /// Action to perform on item selection.
    pub on_select: MenuAction,
    /// Resolved actions for custom menu items (indexed by item position).
    ///
    /// Only populated when `on_select` is `MenuAction::Custom`.
    pub item_actions: Vec<crate::action::Action>,
}

/// Phase of the search interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchPhase {
    /// User is actively typing the search query.
    Typing,
    /// Search is confirmed and acting as a filter.
    Filtered,
}

/// What the fuzzy search matches against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchMode {
    /// Match against file/directory name only.
    #[default]
    Name,
    /// Match against the relative path from the root.
    Path,
}

impl SearchMode {
    /// Toggle between `Name` and `Path`.
    #[must_use]
    pub const fn toggle(self) -> Self {
        match self {
            Self::Name => Self::Path,
            Self::Path => Self::Name,
        }
    }
}

/// State for the search overlay.
#[derive(Debug, Clone)]
pub struct SearchState {
    /// Text buffer for the search query.
    pub buffer: TextBuffer,
    /// Current search phase.
    pub phase: SearchPhase,
    /// What to match against (file name or path).
    pub mode: SearchMode,
    /// Index into search history during Up/Down navigation (`None` = new input).
    pub history_index: Option<usize>,
    /// Query text saved before entering history navigation.
    pub original_query: String,
}

impl SearchState {
    /// Create a new search state for a fresh search.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buffer: TextBuffer::new(),
            phase: SearchPhase::Typing,
            mode: SearchMode::Name,
            history_index: None,
            original_query: String::new(),
        }
    }
}

/// A single keybinding entry for the help overlay.
#[derive(Debug, Clone)]
pub struct HelpBinding {
    /// Human-readable key display (e.g. "j", "C-d", "zz"). Empty for unbound actions.
    pub key_display: String,
    /// Raw action name used for grouping (e.g. `tree.move_down`).
    pub action_name: String,
    /// Human-readable description (e.g. "Move down").
    pub description: String,
    /// The resolved action for execution on Enter.
    pub action: crate::action::Action,
    /// Whether a keybinding is assigned to this action.
    pub has_keybinding: bool,
}

/// State for the keybinding help overlay.
#[derive(Debug, Clone)]
pub struct HelpState {
    /// Scroll offset within the binding list.
    pub scroll_offset: usize,
    /// Cursor index into the filtered binding list.
    pub cursor: usize,
    /// Pre-collected keybinding entries (all actions, bound and unbound).
    pub bindings: Vec<HelpBinding>,
    /// Text buffer for filtering bindings by key or description.
    pub filter: TextBuffer,
    /// Whether the filter input is currently active.
    pub filtering: bool,
}

impl HelpState {
    /// Return references to bindings that match the current filter.
    ///
    /// When the filter is empty, all bindings are returned.
    /// Matching is case-insensitive against key display, description, and action name.
    pub fn filtered_bindings(&self) -> Vec<&HelpBinding> {
        if self.filter.value.is_empty() {
            return self.bindings.iter().collect();
        }
        let query = self.filter.value.to_ascii_lowercase();
        self.bindings
            .iter()
            .filter(|b| {
                b.key_display.to_ascii_lowercase().contains(&query)
                    || b.description.to_ascii_lowercase().contains(&query)
                    || b.action_name.to_ascii_lowercase().contains(&query)
            })
            .collect()
    }
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
    /// Selection menu overlay.
    Menu(MenuState),
    /// Search mode (typing or filtered).
    Search(SearchState),
    /// Keybinding help overlay.
    Help(HelpState),
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    /// Create a `TextBuffer` with the given value and cursor position for testing.
    fn buf(value: &str, cursor_pos: usize) -> TextBuffer {
        TextBuffer::with_value(value.to_string(), cursor_pos)
    }

    /// Create an `InputState` with the given value and cursor position for testing.
    fn input_with(value: &str, cursor_pos: usize) -> InputState {
        InputState {
            prompt: String::new(),
            buffer: buf(value, cursor_pos),
            on_confirm: InputAction::Create { parent_dir: PathBuf::from("/tmp") },
        }
    }

    #[rstest]
    fn input_state_for_create_has_empty_value() {
        let state = InputState::for_create(PathBuf::from("/tmp"));
        assert_that!(state.buffer.value.as_str(), eq(""));
        assert_that!(state.buffer.cursor_pos, eq(0));
        assert_that!(state.prompt.as_str(), eq("Create: "));
    }

    #[rstest]
    fn input_state_for_rename_prefills_name() {
        let state = InputState::for_rename(PathBuf::from("/tmp/hello.txt"));
        assert_that!(state.buffer.value.as_str(), eq("hello.txt"));
        assert_that!(state.prompt.as_str(), eq("Rename: "));
    }

    #[rstest]
    fn input_state_for_rename_cursor_before_extension() {
        let state = InputState::for_rename(PathBuf::from("/tmp/document.pdf"));
        // "document" is 8 chars, cursor should be at position 8 (before ".pdf").
        assert_that!(state.buffer.cursor_pos, eq(8));
    }

    #[rstest]
    fn input_state_for_rename_no_extension() {
        let state = InputState::for_rename(PathBuf::from("/tmp/Makefile"));
        // No extension, cursor at end.
        assert_that!(state.buffer.cursor_pos, eq(8)); // "Makefile" length
    }

    #[rstest]
    fn input_state_for_rename_dotfile() {
        let state = InputState::for_rename(PathBuf::from("/tmp/.gitignore"));
        // ".gitignore" — stem is ".gitignore", no extension. cursor at end.
        assert_that!(state.buffer.cursor_pos, eq(10)); // ".gitignore" length
    }

    #[rstest]
    fn app_mode_default_is_normal() {
        let mode = AppMode::default();
        assert!(matches!(mode, AppMode::Normal));
    }

    // --- TextBuffer editing tests ---

    #[rstest]
    fn insert_char_at_beginning() {
        let mut b = buf("", 0);
        b.insert_char('h');
        b.insert_char('i');
        assert_that!(b.value.as_str(), eq("hi"));
        assert_that!(b.cursor_pos, eq(2));
    }

    #[rstest]
    fn insert_char_in_middle() {
        let mut b = buf("ab.txt", 2); // ab|.txt
        b.insert_char('c');
        assert_that!(b.value.as_str(), eq("abc.txt"));
        assert_that!(b.cursor_pos, eq(3));
    }

    #[rstest]
    fn delete_char_backward_removes_before_cursor() {
        let mut b = buf("hello", 3); // hel|lo
        b.delete_char_backward();
        assert_that!(b.value.as_str(), eq("helo"));
        assert_that!(b.cursor_pos, eq(2));
    }

    #[rstest]
    fn delete_char_backward_at_beginning_is_noop() {
        let mut b = buf("hello", 0);
        b.delete_char_backward();
        assert_that!(b.value.as_str(), eq("hello"));
        assert_that!(b.cursor_pos, eq(0));
    }

    #[rstest]
    fn delete_char_forward_removes_at_cursor() {
        let mut b = buf("hello", 2); // he|llo
        b.delete_char_forward();
        assert_that!(b.value.as_str(), eq("helo"));
        assert_that!(b.cursor_pos, eq(2));
    }

    #[rstest]
    fn delete_char_forward_at_end_is_noop() {
        let mut b = buf("hello", 5);
        b.delete_char_forward();
        assert_that!(b.value.as_str(), eq("hello"));
    }

    #[rstest]
    fn move_cursor_left_and_right() {
        let mut b = buf("abc", 2);
        b.move_cursor_left();
        assert_that!(b.cursor_pos, eq(1));
        b.move_cursor_right();
        assert_that!(b.cursor_pos, eq(2));
    }

    #[rstest]
    fn move_cursor_home_and_end() {
        let mut b = buf("hello", 3);
        b.move_cursor_home();
        assert_that!(b.cursor_pos, eq(0));
        b.move_cursor_end();
        assert_that!(b.cursor_pos, eq(5));
    }

    #[rstest]
    fn clear_to_start_removes_before_cursor() {
        let mut b = buf("hello", 3); // hel|lo
        b.clear_to_start();
        assert_that!(b.value.as_str(), eq("lo"));
        assert_that!(b.cursor_pos, eq(0));
    }

    // --- InputState handle_key_event tests ---

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
        assert_that!(state.buffer.value.as_str(), eq("x"));
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
        let mut b = buf("", 0);
        b.insert_char('日');
        b.insert_char('本');
        assert_that!(b.value.as_str(), eq("日本"));
        assert_that!(b.cursor_pos, eq(6)); // 3 bytes per char
        b.move_cursor_left();
        assert_that!(b.cursor_pos, eq(3));
        b.delete_char_backward();
        assert_that!(b.value.as_str(), eq("本"));
        assert_that!(b.cursor_pos, eq(0));
    }

    #[rstest]
    fn clear_to_end_removes_after_cursor() {
        let mut b = buf("hello world", 5); // hello| world
        b.clear_to_end();
        assert_that!(b.value.as_str(), eq("hello"));
        assert_that!(b.cursor_pos, eq(5));
    }

    #[rstest]
    fn clear_to_end_at_end_is_noop() {
        let mut b = buf("hello", 5);
        b.clear_to_end();
        assert_that!(b.value.as_str(), eq("hello"));
    }

    #[rstest]
    fn delete_word_backward_removes_word() {
        let mut b = buf("hello world", 11); // hello world|
        b.delete_word_backward();
        assert_that!(b.value.as_str(), eq("hello "));
        assert_that!(b.cursor_pos, eq(6));
    }

    #[rstest]
    fn delete_word_backward_at_beginning_is_noop() {
        let mut b = buf("hello", 0);
        b.delete_word_backward();
        assert_that!(b.value.as_str(), eq("hello"));
        assert_that!(b.cursor_pos, eq(0));
    }

    #[rstest]
    fn delete_word_backward_with_spaces() {
        let mut b = buf("foo  bar", 8); // foo  bar|
        b.delete_word_backward();
        assert_that!(b.value.as_str(), eq("foo  "));
        assert_that!(b.cursor_pos, eq(5));
    }

    #[rstest]
    fn delete_word_backward_skips_trailing_spaces() {
        let mut b = buf("hello   ", 8); // hello   |
        b.delete_word_backward();
        assert_that!(b.value.as_str(), eq(""));
        assert_that!(b.cursor_pos, eq(0));
    }

    #[rstest]
    fn delete_word_forward_removes_word() {
        let mut b = buf("hello world", 0); // |hello world
        b.delete_word_forward();
        assert_that!(b.value.as_str(), eq("world"));
        assert_that!(b.cursor_pos, eq(0));
    }

    #[rstest]
    fn delete_word_forward_at_end_is_noop() {
        let mut b = buf("hello", 5);
        b.delete_word_forward();
        assert_that!(b.value.as_str(), eq("hello"));
    }

    #[rstest]
    fn move_word_backward_moves_to_word_start() {
        let mut b = buf("hello world", 11); // hello world|
        b.move_word_backward();
        assert_that!(b.cursor_pos, eq(6)); // hello |world
    }

    #[rstest]
    fn move_word_backward_at_beginning_stays() {
        let mut b = buf("hello", 0);
        b.move_word_backward();
        assert_that!(b.cursor_pos, eq(0));
    }

    #[rstest]
    fn move_word_forward_moves_to_next_word() {
        let mut b = buf("hello world", 0); // |hello world
        b.move_word_forward();
        assert_that!(b.cursor_pos, eq(6)); // hello |world
    }

    #[rstest]
    fn move_word_forward_at_end_stays() {
        let mut b = buf("hello", 5);
        b.move_word_forward();
        assert_that!(b.cursor_pos, eq(5));
    }

    // --- TextBuffer handle_key_event tests ---

    #[rstest]
    fn handle_key_ctrl_w_deletes_word_backward() {
        let mut b = buf("hello world", 11);
        let key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_that!(b.handle_key_event(key), eq(true));
        assert_that!(b.value.as_str(), eq("hello "));
    }

    #[rstest]
    fn handle_key_ctrl_k_clears_to_end() {
        let mut b = buf("hello world", 5);
        let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL);
        assert_that!(b.handle_key_event(key), eq(true));
        assert_that!(b.value.as_str(), eq("hello"));
    }

    #[rstest]
    fn handle_key_alt_b_moves_word_backward() {
        let mut b = buf("hello world", 11);
        let key = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT);
        assert_that!(b.handle_key_event(key), eq(true));
        assert_that!(b.cursor_pos, eq(6));
    }

    #[rstest]
    fn handle_key_alt_f_moves_word_forward() {
        let mut b = buf("hello world", 0);
        let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT);
        assert_that!(b.handle_key_event(key), eq(true));
        assert_that!(b.cursor_pos, eq(6));
    }

    #[rstest]
    fn handle_key_ctrl_b_moves_left() {
        let mut b = buf("abc", 2);
        let key = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL);
        assert_that!(b.handle_key_event(key), eq(true));
        assert_that!(b.cursor_pos, eq(1));
    }

    #[rstest]
    fn handle_key_ctrl_f_moves_right() {
        let mut b = buf("abc", 1);
        let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL);
        assert_that!(b.handle_key_event(key), eq(true));
        assert_that!(b.cursor_pos, eq(2));
    }

    #[rstest]
    fn handle_key_unhandled_returns_false() {
        let mut b = buf("abc", 0);
        let key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        assert_that!(b.handle_key_event(key), eq(false));
    }

    #[rstest]
    fn move_word_with_punctuation() {
        let mut b = buf("foo.bar baz", 0);
        b.move_word_forward();
        assert_that!(b.cursor_pos, eq(4)); // foo.|bar baz
    }
}
