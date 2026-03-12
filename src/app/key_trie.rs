//! Trie-based key sequence resolver.
//!
//! Supports both single-key and multi-key sequence bindings with context-aware
//! resolution. The trie allows prefix matching to detect pending sequences and
//! fallback actions for timeout resolution.

use std::collections::{
    BTreeSet,
    HashMap,
};

use crate::action::Action;
use crate::app::key_parse::KeyBinding;
use crate::app::keymap::KeyContext;

/// Context set required for a binding to match.
type WhenSet = BTreeSet<KeyContext>;

/// Result of looking up a key sequence in the trie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrieLookup<'a> {
    /// Complete match — execute this action.
    Resolved(&'a Action),
    /// Prefix match only — wait for more keys.
    Pending,
    /// Prefix match, but the current position also has a binding.
    /// On timeout, execute the fallback action.
    PendingWithFallback(&'a Action),
    /// No match at all.
    NoMatch,
}

/// A node in the key binding trie.
#[derive(Debug, Default)]
struct TrieNode {
    /// Actions at this node, keyed by their required context set.
    actions: Vec<(WhenSet, Action)>,
    /// Child nodes keyed by the next key in the sequence.
    children: HashMap<KeyBinding, Self>,
}

/// Trie for key sequence resolution.
#[derive(Debug)]
pub struct KeyTrie {
    /// Root node of the trie.
    root: TrieNode,
}

impl KeyTrie {
    /// Create an empty trie.
    pub fn new() -> Self {
        Self { root: TrieNode::default() }
    }

    /// Insert a key sequence binding with the given context set and action.
    pub fn insert(&mut self, sequence: &[KeyBinding], when: WhenSet, action: Action) {
        let mut node = &mut self.root;
        for key in sequence {
            node = node.children.entry(*key).or_default();
        }
        node.actions.push((when, action));
    }

    /// Look up a key sequence against active contexts.
    ///
    /// Resolution rules:
    /// - If the sequence leads to a node with matching actions **and** has children
    ///   with deeper bindings → `PendingWithFallback`
    /// - If the sequence leads to a node with matching actions **and** no children → `Resolved`
    /// - If the sequence is a valid prefix but no action at this depth → `Pending`
    /// - Otherwise → `NoMatch`
    pub fn lookup<'a>(
        &'a self,
        sequence: &[KeyBinding],
        active_contexts: &BTreeSet<KeyContext>,
    ) -> TrieLookup<'a> {
        let mut node = &self.root;

        for key in sequence {
            match node.children.get(key) {
                Some(child) => node = child,
                None => return TrieLookup::NoMatch,
            }
        }

        let resolved_action = resolve_best_action(&node.actions, active_contexts);
        let has_children = !node.children.is_empty();

        match (resolved_action, has_children) {
            (Some(action), true) => TrieLookup::PendingWithFallback(action),
            (Some(action), false) => TrieLookup::Resolved(action),
            (None, true) => TrieLookup::Pending,
            (None, false) => TrieLookup::NoMatch,
        }
    }

    /// Collect all registered bindings from the trie.
    ///
    /// Returns a list of `(key_sequence, context_set, action)` tuples by
    /// performing a depth-first traversal.
    pub fn collect_bindings(&self) -> Vec<(Vec<KeyBinding>, WhenSet, Action)> {
        let mut result = Vec::new();
        Self::collect_node(&self.root, &mut Vec::new(), &mut result);
        result
    }

    /// Recursively collect bindings from a trie node.
    fn collect_node(
        node: &TrieNode,
        path: &mut Vec<KeyBinding>,
        result: &mut Vec<(Vec<KeyBinding>, WhenSet, Action)>,
    ) {
        for (when, action) in &node.actions {
            if !matches!(action, Action::Noop) {
                result.push((path.clone(), when.clone(), action.clone()));
            }
        }
        for (key, child) in &node.children {
            path.push(*key);
            Self::collect_node(child, path, result);
            path.pop();
        }
    }
}

/// Find the best matching action from a set of context-guarded actions.
///
/// Among all actions whose `when` set is a subset of `active_contexts`,
/// the one with the largest (most specific) `when` set wins.
/// `Noop` actions are filtered out.
fn resolve_best_action<'a>(
    actions: &'a [(WhenSet, Action)],
    active_contexts: &BTreeSet<KeyContext>,
) -> Option<&'a Action> {
    actions
        .iter()
        .filter(|(when, _)| when.is_subset(active_contexts))
        .max_by_key(|(when, _)| when.len())
        .map(|(_, action)| action)
        .filter(|a| !matches!(a, Action::Noop))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use crossterm::event::{
        KeyCode,
        KeyModifiers,
    };
    use rstest::*;

    use super::*;
    use crate::action::TreeAction;

    fn kb(c: char) -> KeyBinding {
        (KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn file_ctx() -> BTreeSet<KeyContext> {
        BTreeSet::from([KeyContext::File])
    }

    fn dir_ctx() -> BTreeSet<KeyContext> {
        BTreeSet::from([KeyContext::Directory])
    }

    // --- Single key bindings ---

    #[rstest]
    fn single_key_resolved() {
        let mut trie = KeyTrie::new();
        trie.insert(&[kb('j')], BTreeSet::new(), Action::Tree(TreeAction::MoveDown));

        let result = trie.lookup(&[kb('j')], &file_ctx());
        assert_eq!(result, TrieLookup::Resolved(&Action::Tree(TreeAction::MoveDown)));
    }

    #[rstest]
    fn single_key_no_match() {
        let trie = KeyTrie::new();
        let result = trie.lookup(&[kb('z')], &file_ctx());
        assert_eq!(result, TrieLookup::NoMatch);
    }

    // --- Multi-key sequence bindings ---

    #[rstest]
    fn two_key_sequence_resolved() {
        let mut trie = KeyTrie::new();
        trie.insert(&[kb('z'), kb('z')], BTreeSet::new(), Action::Tree(TreeAction::CenterCursor));

        let result = trie.lookup(&[kb('z'), kb('z')], &file_ctx());
        assert_eq!(result, TrieLookup::Resolved(&Action::Tree(TreeAction::CenterCursor)));
    }

    #[rstest]
    fn prefix_of_sequence_is_pending() {
        let mut trie = KeyTrie::new();
        trie.insert(&[kb('z'), kb('z')], BTreeSet::new(), Action::Tree(TreeAction::CenterCursor));

        let result = trie.lookup(&[kb('z')], &file_ctx());
        assert_eq!(result, TrieLookup::Pending);
    }

    // --- PendingWithFallback ---

    #[rstest]
    fn single_key_with_deeper_binding_is_pending_with_fallback() {
        let mut trie = KeyTrie::new();
        // g → JumpFirst (single key)
        trie.insert(&[kb('g')], BTreeSet::new(), Action::Tree(TreeAction::JumpFirst));
        // g g → JumpLast (sequence)
        trie.insert(&[kb('g'), kb('g')], BTreeSet::new(), Action::Tree(TreeAction::JumpLast));

        let result = trie.lookup(&[kb('g')], &file_ctx());
        assert_eq!(result, TrieLookup::PendingWithFallback(&Action::Tree(TreeAction::JumpFirst)));
    }

    // --- Context resolution ---

    #[rstest]
    fn context_specific_binding_wins_over_universal() {
        let mut trie = KeyTrie::new();
        trie.insert(&[kb('e')], BTreeSet::new(), Action::Tree(TreeAction::Expand));
        trie.insert(
            &[kb('e')],
            BTreeSet::from([KeyContext::Directory]),
            Action::Tree(TreeAction::ToggleExpand),
        );

        let result = trie.lookup(&[kb('e')], &dir_ctx());
        assert_eq!(result, TrieLookup::Resolved(&Action::Tree(TreeAction::ToggleExpand)));
    }

    #[rstest]
    fn context_mismatch_falls_back_to_universal() {
        let mut trie = KeyTrie::new();
        trie.insert(&[kb('e')], BTreeSet::new(), Action::Tree(TreeAction::Expand));
        trie.insert(
            &[kb('e')],
            BTreeSet::from([KeyContext::Directory]),
            Action::Tree(TreeAction::ToggleExpand),
        );

        let result = trie.lookup(&[kb('e')], &file_ctx());
        assert_eq!(result, TrieLookup::Resolved(&Action::Tree(TreeAction::Expand)));
    }

    #[rstest]
    fn noop_action_filtered_out() {
        let mut trie = KeyTrie::new();
        trie.insert(&[kb('n')], BTreeSet::new(), Action::Noop);

        let result = trie.lookup(&[kb('n')], &file_ctx());
        assert_eq!(result, TrieLookup::NoMatch);
    }

    #[rstest]
    fn no_match_for_unregistered_context() {
        let mut trie = KeyTrie::new();
        trie.insert(&[kb('x')], BTreeSet::from([KeyContext::Daemon]), Action::Quit);

        // File context doesn't include Daemon.
        let result = trie.lookup(&[kb('x')], &file_ctx());
        assert_eq!(result, TrieLookup::NoMatch);
    }

    // --- Empty sequence ---

    #[rstest]
    fn empty_sequence_with_children_is_pending() {
        let mut trie = KeyTrie::new();
        trie.insert(&[kb('j')], BTreeSet::new(), Action::Tree(TreeAction::MoveDown));

        // Root has children but no action → Pending.
        let result = trie.lookup(&[], &file_ctx());
        assert_eq!(result, TrieLookup::Pending);
    }

    #[rstest]
    fn empty_sequence_empty_trie_is_no_match() {
        let trie = KeyTrie::new();
        let result = trie.lookup(&[], &file_ctx());
        assert_eq!(result, TrieLookup::NoMatch);
    }

    // --- Partial sequence with no match ---

    #[rstest]
    fn wrong_second_key_is_no_match() {
        let mut trie = KeyTrie::new();
        trie.insert(&[kb('z'), kb('z')], BTreeSet::new(), Action::Tree(TreeAction::CenterCursor));

        let result = trie.lookup(&[kb('z'), kb('x')], &file_ctx());
        assert_eq!(result, TrieLookup::NoMatch);
    }
}
