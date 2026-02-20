//! Key-to-action mapping with context-aware resolution.
//!
//! Provides the [`KeyMap`] struct that resolves key events into application actions.
//! Supports context-based binding sections: universal, file, directory, and daemon variants.
//! More specific context sets take priority over less specific ones.

use std::collections::{
    BTreeSet,
    HashMap,
};

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
use crate::app::key_parse;
use crate::config::{
    ContextBindings,
    KeyBindingEntry,
    KeybindingConfig,
};

/// Key binding type alias: `(KeyCode, KeyModifiers)`.
type KeyBinding = (KeyCode, KeyModifiers);

/// A set of required contexts for a keybinding to match.
/// Empty set means "matches always" (universal fallback).
type WhenSet = BTreeSet<KeyContext>;

/// Composite key: `(KeyBinding, WhenSet)`.
type BindingKey = (KeyBinding, WhenSet);

/// Context in which a key event occurs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KeyContext {
    /// Running in daemon mode (IPC server active).
    Daemon,
    /// Cursor is on a directory.
    Directory,
    /// Cursor is on a file.
    File,
}

/// Maps key events to application actions with context-aware resolution.
///
/// Resolution: among all registered bindings for a key whose `when` set is
/// a subset of the active contexts, the one with the largest (most specific)
/// `when` set wins. `Noop` actions are filtered out.
#[derive(Debug)]
pub struct KeyMap {
    /// All bindings indexed by `(key, when-set)`.
    bindings: HashMap<BindingKey, Action>,
}

impl KeyMap {
    /// Resolve a key event to an action using context-aware lookup.
    ///
    /// `active_contexts` describes the current runtime state
    /// (e.g. `{Daemon, File}` or `{Directory}`).
    pub fn resolve(
        &self,
        key: KeyEvent,
        active_contexts: &BTreeSet<KeyContext>,
    ) -> Option<&Action> {
        let kb: KeyBinding = (key.code, key.modifiers);
        self.bindings
            .iter()
            .filter(|((k, when), _)| *k == kb && when.is_subset(active_contexts))
            .max_by_key(|((_, when), _)| when.len())
            .map(|(_, action)| action)
            .filter(|a| !matches!(a, Action::Noop))
    }

    /// Build a `KeyMap` from configuration.
    ///
    /// Loads defaults first (unless disabled), then applies user bindings on top.
    /// Each section's bindings are registered with the corresponding context set.
    pub fn from_config(config: &KeybindingConfig) -> Self {
        let mut km = Self::empty();

        // Load defaults (universal + context-specific defaults like daemon.file).
        if !config.disable_default && !config.universal.disable_default {
            km.load_universal_defaults();
        }

        // Apply user bindings from each section.
        let sections: &[(&ContextBindings, WhenSet)] = &[
            (&config.universal, BTreeSet::new()),
            (&config.file, BTreeSet::from([KeyContext::File])),
            (&config.directory, BTreeSet::from([KeyContext::Directory])),
            (&config.daemon.universal, BTreeSet::from([KeyContext::Daemon])),
            (
                &config.daemon.file,
                BTreeSet::from([KeyContext::Daemon, KeyContext::File]),
            ),
            (
                &config.daemon.directory,
                BTreeSet::from([KeyContext::Daemon, KeyContext::Directory]),
            ),
        ];

        for (section, when_set) in sections {
            for entry in &section.bindings {
                if let Err(e) = km.apply_entry(entry, when_set) {
                    tracing::warn!("skipping keybinding: {e}");
                }
            }
        }

        km
    }

    /// Create an empty `KeyMap` with no bindings.
    fn empty() -> Self {
        Self { bindings: HashMap::new() }
    }

    /// Apply a single keybinding entry with the given context set.
    fn apply_entry(&mut self, entry: &KeyBindingEntry, when_set: &WhenSet) -> Result<(), String> {
        let action = resolve_entry_action(entry)?;
        let expanded = key_parse::parse_key_expanded(&entry.key)?;

        for binding in expanded {
            self.bindings
                .insert((binding, when_set.clone()), action.clone());
        }

        Ok(())
    }

    /// Register a single key binding in the universal (no-context) map.
    fn bind(&mut self, code: KeyCode, modifiers: KeyModifiers, action: Action) {
        self.bindings
            .insert(((code, modifiers), BTreeSet::new()), action);
    }

    /// Register a key binding with a specific context set.
    fn bind_when(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        when: WhenSet,
        action: Action,
    ) {
        self.bindings.insert(((code, modifiers), when), action);
    }

    /// Load default vim-style universal keybindings.
    fn load_universal_defaults(&mut self) {
        self.load_default_navigation();
        self.load_default_preview();
        self.load_default_display();
        self.load_default_file_ops();
        self.load_default_daemon();
    }

    /// Default navigation and quit bindings.
    fn load_default_navigation(&mut self) {
        self.bind(KeyCode::Char('q'), KeyModifiers::NONE, Action::Quit);
        self.bind(KeyCode::Char('j'), KeyModifiers::NONE, Action::Tree(TreeAction::MoveDown));
        self.bind(KeyCode::Down, KeyModifiers::NONE, Action::Tree(TreeAction::MoveDown));
        self.bind(KeyCode::Char('k'), KeyModifiers::NONE, Action::Tree(TreeAction::MoveUp));
        self.bind(KeyCode::Up, KeyModifiers::NONE, Action::Tree(TreeAction::MoveUp));
        self.bind(KeyCode::Char('l'), KeyModifiers::NONE, Action::Tree(TreeAction::Expand));
        self.bind(KeyCode::Right, KeyModifiers::NONE, Action::Tree(TreeAction::Expand));
        self.bind(KeyCode::Char('h'), KeyModifiers::NONE, Action::Tree(TreeAction::Collapse));
        self.bind(KeyCode::Left, KeyModifiers::NONE, Action::Tree(TreeAction::Collapse));
        // Enter on directories: toggle expand/collapse.
        // Enter on files in daemon mode is handled by load_default_daemon().
        self.bind_when(
            KeyCode::Enter,
            KeyModifiers::NONE,
            BTreeSet::from([KeyContext::Directory]),
            Action::Tree(TreeAction::ToggleExpand),
        );
        self.bind(KeyCode::Char('g'), KeyModifiers::NONE, Action::Tree(TreeAction::JumpFirst));
        self.bind(KeyCode::Char('G'), KeyModifiers::SHIFT, Action::Tree(TreeAction::JumpLast));
        self.bind(KeyCode::Char('G'), KeyModifiers::NONE, Action::Tree(TreeAction::JumpLast));
        self.bind(KeyCode::Char('d'), KeyModifiers::CONTROL, Action::Tree(TreeAction::HalfPageDown));
        self.bind(KeyCode::Char('u'), KeyModifiers::CONTROL, Action::Tree(TreeAction::HalfPageUp));
    }

    /// Default preview scroll and toggle bindings.
    fn load_default_preview(&mut self) {
        self.bind(KeyCode::Char('J'), KeyModifiers::SHIFT, Action::Preview(PreviewAction::ScrollDown));
        self.bind(KeyCode::Char('J'), KeyModifiers::NONE, Action::Preview(PreviewAction::ScrollDown));
        self.bind(KeyCode::Char('K'), KeyModifiers::SHIFT, Action::Preview(PreviewAction::ScrollUp));
        self.bind(KeyCode::Char('K'), KeyModifiers::NONE, Action::Preview(PreviewAction::ScrollUp));
        self.bind(KeyCode::Char('L'), KeyModifiers::SHIFT, Action::Preview(PreviewAction::ScrollRight));
        self.bind(KeyCode::Char('L'), KeyModifiers::NONE, Action::Preview(PreviewAction::ScrollRight));
        self.bind(KeyCode::Char('H'), KeyModifiers::SHIFT, Action::Preview(PreviewAction::ScrollLeft));
        self.bind(KeyCode::Char('H'), KeyModifiers::NONE, Action::Preview(PreviewAction::ScrollLeft));
        self.bind(KeyCode::Char('U'), KeyModifiers::SHIFT, Action::Preview(PreviewAction::HalfPageUp));
        self.bind(KeyCode::Char('U'), KeyModifiers::NONE, Action::Preview(PreviewAction::HalfPageUp));
        self.bind(KeyCode::Tab, KeyModifiers::NONE, Action::Preview(PreviewAction::CycleNextProvider));
        self.bind(KeyCode::BackTab, KeyModifiers::SHIFT, Action::Preview(PreviewAction::CyclePrevProvider));
        self.bind(KeyCode::Char('P'), KeyModifiers::SHIFT, Action::Preview(PreviewAction::TogglePreview));
        self.bind(KeyCode::Char('P'), KeyModifiers::NONE, Action::Preview(PreviewAction::TogglePreview));
    }

    /// Default display toggle bindings.
    fn load_default_display(&mut self) {
        self.bind(KeyCode::Char('E'), KeyModifiers::SHIFT, Action::Tree(TreeAction::ExpandAll));
        self.bind(KeyCode::Char('E'), KeyModifiers::NONE, Action::Tree(TreeAction::ExpandAll));
        self.bind(KeyCode::Char('W'), KeyModifiers::SHIFT, Action::Tree(TreeAction::CollapseAll));
        self.bind(KeyCode::Char('W'), KeyModifiers::NONE, Action::Tree(TreeAction::CollapseAll));
        self.bind(KeyCode::Char('.'), KeyModifiers::NONE, Action::Tree(TreeAction::ToggleHidden));
        self.bind(KeyCode::Char('I'), KeyModifiers::SHIFT, Action::Tree(TreeAction::ToggleIgnored));
        self.bind(KeyCode::Char('I'), KeyModifiers::NONE, Action::Tree(TreeAction::ToggleIgnored));
        self.bind(KeyCode::Char('R'), KeyModifiers::SHIFT, Action::Tree(TreeAction::Refresh));
        self.bind(KeyCode::Char('R'), KeyModifiers::NONE, Action::Tree(TreeAction::Refresh));
    }

    /// Default file operation bindings.
    fn load_default_file_ops(&mut self) {
        self.bind(KeyCode::Char(' '), KeyModifiers::NONE, Action::FileOp(FileOpAction::ToggleMark));
        self.bind(KeyCode::Char('a'), KeyModifiers::NONE, Action::FileOp(FileOpAction::CreateFile));
        self.bind(KeyCode::Char('r'), KeyModifiers::NONE, Action::FileOp(FileOpAction::Rename));
        self.bind(KeyCode::Char('y'), KeyModifiers::NONE, Action::FileOp(FileOpAction::Yank));
        self.bind(KeyCode::Char('x'), KeyModifiers::NONE, Action::FileOp(FileOpAction::Cut));
        self.bind(KeyCode::Char('p'), KeyModifiers::NONE, Action::FileOp(FileOpAction::Paste));
        self.bind(KeyCode::Char('d'), KeyModifiers::NONE, Action::FileOp(FileOpAction::Delete));
        self.bind(KeyCode::Char('D'), KeyModifiers::SHIFT, Action::FileOp(FileOpAction::SystemTrash));
        self.bind(KeyCode::Char('D'), KeyModifiers::NONE, Action::FileOp(FileOpAction::SystemTrash));
        self.bind(KeyCode::Char('u'), KeyModifiers::NONE, Action::FileOp(FileOpAction::Undo));
        self.bind(KeyCode::Char('r'), KeyModifiers::CONTROL, Action::FileOp(FileOpAction::Redo));
        self.bind(KeyCode::Esc, KeyModifiers::NONE, Action::FileOp(FileOpAction::ClearSelections));
        self.bind(KeyCode::Char('c'), KeyModifiers::NONE, Action::FileOp(FileOpAction::CopyMenu));
    }

    /// Default daemon-mode keybindings.
    fn load_default_daemon(&mut self) {
        let daemon = BTreeSet::from([KeyContext::Daemon]);
        let daemon_file = BTreeSet::from([KeyContext::Daemon, KeyContext::File]);

        // q in daemon mode: notify the editor to close the panel/float
        // (overrides universal quit so the plugin can handle cleanup properly).
        self.bind_when(KeyCode::Char('q'), KeyModifiers::NONE, daemon, Action::Notify("close".to_string()));

        // Enter on a file in daemon mode: send open_file notification to the editor.
        self.bind_when(KeyCode::Enter, KeyModifiers::NONE, daemon_file, Action::Notify("open_file".to_string()));
    }
}

/// Resolve the action from a keybinding entry.
///
/// Exactly one of `action`, `run`, or `notify` must be set.
fn resolve_entry_action(entry: &KeyBindingEntry) -> Result<Action, String> {
    let set_count = u8::from(entry.action.is_some())
        + u8::from(entry.run.is_some())
        + u8::from(entry.notify.is_some());

    if set_count == 0 {
        return Err(format!("keybinding '{}' has no action, run, or notify", entry.key));
    }
    if set_count > 1 {
        return Err(format!(
            "keybinding '{}' has multiple of action/run/notify (specify exactly one)",
            entry.key
        ));
    }

    if let Some(ref action_str) = entry.action {
        return action_str
            .parse::<Action>()
            .map_err(|e| format!("keybinding '{}': {e}", entry.key));
    }

    if let Some(ref cmd) = entry.run {
        return Ok(Action::Shell(cmd.clone()));
    }

    if let Some(ref method) = entry.notify {
        return Ok(Action::Notify(method.clone()));
    }

    // Unreachable due to set_count check above.
    Err(format!("keybinding '{}': invalid state", entry.key))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::literal_string_with_formatting_args
)]
mod tests {
    use rstest::*;

    use super::*;
    use crate::config::DaemonBindings;

    fn default_keymap() -> KeyMap {
        KeyMap::from_config(&KeybindingConfig::default())
    }

    /// Build a context set from a slice of contexts.
    fn ctx(contexts: &[KeyContext]) -> BTreeSet<KeyContext> {
        contexts.iter().copied().collect()
    }

    fn file_ctx() -> BTreeSet<KeyContext> {
        ctx(&[KeyContext::File])
    }

    fn dir_ctx() -> BTreeSet<KeyContext> {
        ctx(&[KeyContext::Directory])
    }

    fn daemon_file_ctx() -> BTreeSet<KeyContext> {
        ctx(&[KeyContext::Daemon, KeyContext::File])
    }

    fn daemon_dir_ctx() -> BTreeSet<KeyContext> {
        ctx(&[KeyContext::Daemon, KeyContext::Directory])
    }

    fn entry(key: &str, action: &str) -> KeyBindingEntry {
        KeyBindingEntry {
            key: key.to_string(),
            action: Some(action.to_string()),
            run: None,
            notify: None,
        }
    }

    // --- Default bindings ---

    #[rstest]
    fn resolve_q_to_quit() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(km.resolve(key, &file_ctx()), Some(&Action::Quit));
    }

    #[rstest]
    fn resolve_j_to_move_down() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Tree(TreeAction::MoveDown))
        );
    }

    #[rstest]
    fn resolve_k_to_move_up() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Tree(TreeAction::MoveUp))
        );
    }

    #[rstest]
    fn resolve_l_to_expand() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Tree(TreeAction::Expand))
        );
    }

    #[rstest]
    fn resolve_h_to_collapse() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Tree(TreeAction::Collapse))
        );
    }

    #[rstest]
    fn resolve_enter_on_directory_to_toggle_expand() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(
            km.resolve(key, &dir_ctx()),
            Some(&Action::Tree(TreeAction::ToggleExpand))
        );
    }

    #[rstest]
    fn resolve_enter_on_file_is_none() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(km.resolve(key, &file_ctx()), None);
    }

    #[rstest]
    fn resolve_enter_on_daemon_file_to_notify_open_file() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(
            km.resolve(key, &daemon_file_ctx()),
            Some(&Action::Notify("open_file".to_string()))
        );
    }

    #[rstest]
    fn resolve_g_to_jump_first() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Tree(TreeAction::JumpFirst))
        );
    }

    #[rstest]
    fn resolve_shift_g_to_jump_last() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Tree(TreeAction::JumpLast))
        );
    }

    #[rstest]
    fn resolve_ctrl_d_to_half_page_down() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Tree(TreeAction::HalfPageDown))
        );
    }

    #[rstest]
    fn resolve_ctrl_u_to_half_page_up() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Tree(TreeAction::HalfPageUp))
        );
    }

    #[rstest]
    fn resolve_space_to_toggle_mark() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::FileOp(FileOpAction::ToggleMark))
        );
    }

    #[rstest]
    fn resolve_shift_e_to_expand_all() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Tree(TreeAction::ExpandAll))
        );
    }

    #[rstest]
    fn resolve_shift_w_to_collapse_all() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('W'), KeyModifiers::SHIFT);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Tree(TreeAction::CollapseAll))
        );
    }

    #[rstest]
    fn resolve_shift_p_to_toggle_preview() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('P'), KeyModifiers::SHIFT);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Preview(PreviewAction::TogglePreview))
        );
    }

    #[rstest]
    fn resolve_c_to_copy_menu() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::FileOp(FileOpAction::CopyMenu))
        );
    }

    #[rstest]
    fn resolve_dot_to_toggle_hidden() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Tree(TreeAction::ToggleHidden))
        );
    }

    #[rstest]
    fn resolve_shift_i_to_toggle_ignored() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('I'), KeyModifiers::SHIFT);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Tree(TreeAction::ToggleIgnored))
        );
    }

    #[rstest]
    fn resolve_unknown_key_to_none() {
        let km = default_keymap();
        let key = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE);
        assert_eq!(km.resolve(key, &file_ctx()), None);
    }

    // --- from_config tests ---

    #[rstest]
    fn from_config_defaults_included() {
        let config = KeybindingConfig::default();
        let km = KeyMap::from_config(&config);
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Tree(TreeAction::MoveDown))
        );
    }

    #[rstest]
    fn from_config_disable_default_starts_empty() {
        let config = KeybindingConfig { disable_default: true, ..KeybindingConfig::default() };
        let km = KeyMap::from_config(&config);
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(km.resolve(key, &file_ctx()), None);
    }

    #[rstest]
    fn from_config_universal_disable_default() {
        let config = KeybindingConfig {
            universal: ContextBindings { disable_default: true, ..Default::default() },
            ..KeybindingConfig::default()
        };
        let km = KeyMap::from_config(&config);
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(km.resolve(key, &file_ctx()), None);
    }

    #[rstest]
    fn from_config_universal_binding_overrides_default() {
        let config = KeybindingConfig {
            universal: ContextBindings {
                bindings: vec![entry("j", "tree.move_up")],
                ..Default::default()
            },
            ..KeybindingConfig::default()
        };
        let km = KeyMap::from_config(&config);
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Tree(TreeAction::MoveUp))
        );
    }

    #[rstest]
    fn from_config_noop_unbinds_default() {
        let config = KeybindingConfig {
            universal: ContextBindings {
                bindings: vec![entry("q", "noop")],
                ..Default::default()
            },
            ..KeybindingConfig::default()
        };
        let km = KeyMap::from_config(&config);
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(km.resolve(key, &file_ctx()), None);
    }

    #[rstest]
    fn from_config_directory_section_binding() {
        let config = KeybindingConfig {
            directory: ContextBindings {
                bindings: vec![entry("<CR>", "tree.toggle_expand")],
                ..Default::default()
            },
            file: ContextBindings {
                bindings: vec![entry("<CR>", "quit")],
                ..Default::default()
            },
            ..KeybindingConfig::default()
        };
        let km = KeyMap::from_config(&config);
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);

        // Directory context → directory section's toggle_expand (more specific than universal default).
        assert_eq!(
            km.resolve(key, &dir_ctx()),
            Some(&Action::Tree(TreeAction::ToggleExpand))
        );
        // File context → file section's quit (more specific than universal default).
        assert_eq!(km.resolve(key, &file_ctx()), Some(&Action::Quit));
    }

    #[rstest]
    fn from_config_universal_fallback_when_no_context_binding() {
        let config = KeybindingConfig {
            universal: ContextBindings {
                bindings: vec![entry("j", "tree.move_down")],
                ..Default::default()
            },
            directory: ContextBindings {
                bindings: vec![entry("j", "tree.expand")],
                ..Default::default()
            },
            ..KeybindingConfig::default()
        };
        let km = KeyMap::from_config(&config);
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);

        // Directory → directory-specific binding (more specific).
        assert_eq!(
            km.resolve(key, &dir_ctx()),
            Some(&Action::Tree(TreeAction::Expand))
        );
        // File → falls back to universal binding.
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Tree(TreeAction::MoveDown))
        );
    }

    #[rstest]
    fn from_config_shell_command() {
        let config = KeybindingConfig {
            universal: ContextBindings {
                bindings: vec![KeyBindingEntry {
                    key: "o".to_string(),
                    action: None,
                    run: Some("open {path}".to_string()),
                    notify: None,
                }],
                ..Default::default()
            },
            ..KeybindingConfig::default()
        };
        let km = KeyMap::from_config(&config);
        let key = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Shell("open {path}".to_string()))
        );
    }

    #[rstest]
    fn from_config_notify() {
        let config = KeybindingConfig {
            universal: ContextBindings {
                bindings: vec![KeyBindingEntry {
                    key: "<C-o>".to_string(),
                    action: None,
                    run: None,
                    notify: Some("open_file".to_string()),
                }],
                ..Default::default()
            },
            ..KeybindingConfig::default()
        };
        let km = KeyMap::from_config(&config);
        let key = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL);
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Notify("open_file".to_string()))
        );
    }

    #[rstest]
    fn from_config_uppercase_expands_to_both_shift_variants() {
        let config = KeybindingConfig {
            disable_default: true,
            universal: ContextBindings {
                bindings: vec![entry("G", "tree.jump_last")],
                ..Default::default()
            },
            ..KeybindingConfig::default()
        };
        let km = KeyMap::from_config(&config);

        let key_shift = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);
        assert_eq!(
            km.resolve(key_shift, &file_ctx()),
            Some(&Action::Tree(TreeAction::JumpLast))
        );

        let key_none = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE);
        assert_eq!(
            km.resolve(key_none, &file_ctx()),
            Some(&Action::Tree(TreeAction::JumpLast))
        );
    }

    // --- Daemon context tests ---

    #[rstest]
    fn daemon_universal_binding_matches_in_daemon_mode() {
        let config = KeybindingConfig {
            disable_default: true,
            daemon: DaemonBindings {
                universal: ContextBindings {
                    bindings: vec![entry("q", "quit")],
                    ..Default::default()
                },
                ..Default::default()
            },
            ..KeybindingConfig::default()
        };
        let km = KeyMap::from_config(&config);
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);

        // Matches in daemon+file context.
        assert_eq!(km.resolve(key, &daemon_file_ctx()), Some(&Action::Quit));
        // Does NOT match without daemon context.
        assert_eq!(km.resolve(key, &file_ctx()), None);
    }

    #[rstest]
    fn daemon_file_binding_more_specific_than_file() {
        let config = KeybindingConfig {
            disable_default: true,
            file: ContextBindings {
                bindings: vec![entry("<CR>", "tree.expand")],
                ..Default::default()
            },
            daemon: DaemonBindings {
                file: ContextBindings {
                    bindings: vec![entry("<CR>", "quit")],
                    ..Default::default()
                },
                ..Default::default()
            },
            ..KeybindingConfig::default()
        };
        let km = KeyMap::from_config(&config);
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);

        // daemon+file → daemon.file (2 contexts) beats file (1 context).
        assert_eq!(km.resolve(key, &daemon_file_ctx()), Some(&Action::Quit));
        // file-only → file section.
        assert_eq!(
            km.resolve(key, &file_ctx()),
            Some(&Action::Tree(TreeAction::Expand))
        );
    }

    #[rstest]
    fn daemon_directory_binding_not_matched_for_file() {
        let config = KeybindingConfig {
            disable_default: true,
            daemon: DaemonBindings {
                directory: ContextBindings {
                    bindings: vec![entry("<CR>", "tree.toggle_expand")],
                    ..Default::default()
                },
                ..Default::default()
            },
            ..KeybindingConfig::default()
        };
        let km = KeyMap::from_config(&config);
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);

        // daemon+directory → matches.
        assert_eq!(
            km.resolve(key, &daemon_dir_ctx()),
            Some(&Action::Tree(TreeAction::ToggleExpand))
        );
        // daemon+file → does NOT match (Directory not in active set).
        assert_eq!(km.resolve(key, &daemon_file_ctx()), None);
    }
}
