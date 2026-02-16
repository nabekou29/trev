//! Key-to-action mapping.
//!
//! Provides the [`KeyMap`] struct that resolves key events into application actions.
//! Currently uses a hardcoded default mapping; future versions will support
//! user-configurable overrides via TOML config.

use std::collections::HashMap;

use crossterm::event::{
    KeyCode,
    KeyEvent,
    KeyModifiers,
};

use crate::action::{
    Action,
    FileOpAction,
    PreviewAction,
    TreeAction,
};

/// Key binding type alias: `(KeyCode, KeyModifiers)`.
type KeyBinding = (KeyCode, KeyModifiers);

/// Maps key events to application actions.
///
/// Wraps a `HashMap` of `(KeyCode, KeyModifiers) -> Action` bindings.
/// Constructed with default vim-style bindings via [`KeyMap::default()`].
#[derive(Debug)]
pub struct KeyMap {
    /// Key binding lookup table.
    bindings: HashMap<KeyBinding, Action>,
}

impl KeyMap {
    /// Resolve a key event to an action, if a binding exists.
    pub fn resolve(&self, key: KeyEvent) -> Option<&Action> {
        // Try exact modifiers first.
        self.bindings.get(&(key.code, key.modifiers))
    }

    /// Register a single key binding.
    fn bind(&mut self, code: KeyCode, modifiers: KeyModifiers, action: Action) {
        self.bindings.insert((code, modifiers), action);
    }
}

impl Default for KeyMap {
    /// Create the default vim-style key map.
    fn default() -> Self {
        let mut km = Self { bindings: HashMap::new() };

        // Quit.
        km.bind(KeyCode::Char('q'), KeyModifiers::NONE, Action::Quit);

        // Tree navigation (vim-style).
        km.bind(KeyCode::Char('j'), KeyModifiers::NONE, Action::Tree(TreeAction::MoveDown));
        km.bind(KeyCode::Down, KeyModifiers::NONE, Action::Tree(TreeAction::MoveDown));
        km.bind(KeyCode::Char('k'), KeyModifiers::NONE, Action::Tree(TreeAction::MoveUp));
        km.bind(KeyCode::Up, KeyModifiers::NONE, Action::Tree(TreeAction::MoveUp));
        km.bind(KeyCode::Char('l'), KeyModifiers::NONE, Action::Tree(TreeAction::Expand));
        km.bind(KeyCode::Right, KeyModifiers::NONE, Action::Tree(TreeAction::Expand));
        km.bind(KeyCode::Char('h'), KeyModifiers::NONE, Action::Tree(TreeAction::Collapse));
        km.bind(KeyCode::Left, KeyModifiers::NONE, Action::Tree(TreeAction::Collapse));
        km.bind(KeyCode::Enter, KeyModifiers::NONE, Action::Tree(TreeAction::ToggleExpand));
        km.bind(KeyCode::Char('g'), KeyModifiers::NONE, Action::Tree(TreeAction::JumpFirst));
        km.bind(KeyCode::Char('G'), KeyModifiers::SHIFT, Action::Tree(TreeAction::JumpLast));
        km.bind(KeyCode::Char('G'), KeyModifiers::NONE, Action::Tree(TreeAction::JumpLast));
        km.bind(KeyCode::Char('d'), KeyModifiers::CONTROL, Action::Tree(TreeAction::HalfPageDown));
        km.bind(KeyCode::Char('u'), KeyModifiers::CONTROL, Action::Tree(TreeAction::HalfPageUp));

        // Preview scroll (Shift + vim-style).
        km.bind(
            KeyCode::Char('J'),
            KeyModifiers::SHIFT,
            Action::Preview(PreviewAction::ScrollDown),
        );
        km.bind(KeyCode::Char('J'), KeyModifiers::NONE, Action::Preview(PreviewAction::ScrollDown));
        km.bind(KeyCode::Char('K'), KeyModifiers::SHIFT, Action::Preview(PreviewAction::ScrollUp));
        km.bind(KeyCode::Char('K'), KeyModifiers::NONE, Action::Preview(PreviewAction::ScrollUp));
        km.bind(
            KeyCode::Char('L'),
            KeyModifiers::SHIFT,
            Action::Preview(PreviewAction::ScrollRight),
        );
        km.bind(
            KeyCode::Char('L'),
            KeyModifiers::NONE,
            Action::Preview(PreviewAction::ScrollRight),
        );
        km.bind(
            KeyCode::Char('H'),
            KeyModifiers::SHIFT,
            Action::Preview(PreviewAction::ScrollLeft),
        );
        km.bind(KeyCode::Char('H'), KeyModifiers::NONE, Action::Preview(PreviewAction::ScrollLeft));
        km.bind(
            KeyCode::Char('U'),
            KeyModifiers::SHIFT,
            Action::Preview(PreviewAction::HalfPageUp),
        );
        km.bind(KeyCode::Char('U'), KeyModifiers::NONE, Action::Preview(PreviewAction::HalfPageUp));

        // Expand/Collapse all.
        km.bind(KeyCode::Char('E'), KeyModifiers::SHIFT, Action::Tree(TreeAction::ExpandAll));
        km.bind(KeyCode::Char('E'), KeyModifiers::NONE, Action::Tree(TreeAction::ExpandAll));
        km.bind(KeyCode::Char('W'), KeyModifiers::SHIFT, Action::Tree(TreeAction::CollapseAll));
        km.bind(KeyCode::Char('W'), KeyModifiers::NONE, Action::Tree(TreeAction::CollapseAll));

        // Provider cycling.
        km.bind(KeyCode::Tab, KeyModifiers::NONE, Action::Preview(PreviewAction::CycleProvider));

        // Preview toggle.
        km.bind(
            KeyCode::Char('P'),
            KeyModifiers::SHIFT,
            Action::Preview(PreviewAction::TogglePreview),
        );
        km.bind(
            KeyCode::Char('P'),
            KeyModifiers::NONE,
            Action::Preview(PreviewAction::TogglePreview),
        );

        // File operations.
        km.bind(KeyCode::Char(' '), KeyModifiers::NONE, Action::FileOp(FileOpAction::ToggleMark));
        km.bind(KeyCode::Char('a'), KeyModifiers::NONE, Action::FileOp(FileOpAction::CreateFile));
        km.bind(KeyCode::Char('r'), KeyModifiers::NONE, Action::FileOp(FileOpAction::Rename));
        km.bind(KeyCode::Char('y'), KeyModifiers::NONE, Action::FileOp(FileOpAction::Yank));
        km.bind(KeyCode::Char('x'), KeyModifiers::NONE, Action::FileOp(FileOpAction::Cut));
        km.bind(KeyCode::Char('p'), KeyModifiers::NONE, Action::FileOp(FileOpAction::Paste));
        km.bind(KeyCode::Char('d'), KeyModifiers::NONE, Action::FileOp(FileOpAction::Delete));
        km.bind(KeyCode::Char('D'), KeyModifiers::SHIFT, Action::FileOp(FileOpAction::SystemTrash));
        km.bind(KeyCode::Char('D'), KeyModifiers::NONE, Action::FileOp(FileOpAction::SystemTrash));
        km.bind(KeyCode::Char('u'), KeyModifiers::NONE, Action::FileOp(FileOpAction::Undo));
        km.bind(KeyCode::Char('r'), KeyModifiers::CONTROL, Action::FileOp(FileOpAction::Redo));
        km.bind(KeyCode::Esc, KeyModifiers::NONE, Action::FileOp(FileOpAction::ClearSelections));
        km.bind(KeyCode::Char('c'), KeyModifiers::NONE, Action::FileOp(FileOpAction::CopyMenu));

        km
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use rstest::*;

    use super::*;

    fn default_keymap() -> KeyMap {
        KeyMap::default()
    }

    #[rstest]
    fn resolve_q_to_quit() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(km.resolve(key), Some(&Action::Quit));
    }

    #[rstest]
    fn resolve_j_to_move_down() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(km.resolve(key), Some(&Action::Tree(TreeAction::MoveDown)));
    }

    #[rstest]
    fn resolve_k_to_move_up() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        assert_eq!(km.resolve(key), Some(&Action::Tree(TreeAction::MoveUp)));
    }

    #[rstest]
    fn resolve_l_to_expand() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
        assert_eq!(km.resolve(key), Some(&Action::Tree(TreeAction::Expand)));
    }

    #[rstest]
    fn resolve_h_to_collapse() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        assert_eq!(km.resolve(key), Some(&Action::Tree(TreeAction::Collapse)));
    }

    #[rstest]
    fn resolve_enter_to_toggle_expand() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(km.resolve(key), Some(&Action::Tree(TreeAction::ToggleExpand)));
    }

    #[rstest]
    fn resolve_g_to_jump_first() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        assert_eq!(km.resolve(key), Some(&Action::Tree(TreeAction::JumpFirst)));
    }

    #[rstest]
    fn resolve_shift_g_to_jump_last() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);
        assert_eq!(km.resolve(key), Some(&Action::Tree(TreeAction::JumpLast)));
    }

    #[rstest]
    fn resolve_ctrl_d_to_half_page_down() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert_eq!(km.resolve(key), Some(&Action::Tree(TreeAction::HalfPageDown)));
    }

    #[rstest]
    fn resolve_ctrl_u_to_half_page_up() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(km.resolve(key), Some(&Action::Tree(TreeAction::HalfPageUp)));
    }

    #[rstest]
    fn resolve_space_to_toggle_mark() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(km.resolve(key), Some(&Action::FileOp(FileOpAction::ToggleMark)));
    }

    #[rstest]
    fn resolve_shift_e_to_expand_all() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT);
        assert_eq!(km.resolve(key), Some(&Action::Tree(TreeAction::ExpandAll)));
    }

    #[rstest]
    fn resolve_shift_w_to_collapse_all() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('W'), KeyModifiers::SHIFT);
        assert_eq!(km.resolve(key), Some(&Action::Tree(TreeAction::CollapseAll)));
    }

    #[rstest]
    fn resolve_shift_p_to_toggle_preview() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('P'), KeyModifiers::SHIFT);
        assert_eq!(km.resolve(key), Some(&Action::Preview(PreviewAction::TogglePreview)));
    }

    #[rstest]
    fn resolve_c_to_copy_menu() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);
        assert_eq!(km.resolve(key), Some(&Action::FileOp(FileOpAction::CopyMenu)));
    }

    #[rstest]
    fn resolve_unknown_key_to_none() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE);
        assert_eq!(km.resolve(key), None);
    }
}
