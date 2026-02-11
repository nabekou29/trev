//! Application action definitions.

/// Placeholder for the top-level action enum.
///
/// Will be expanded as features are implemented.
#[derive(Debug, Clone)]
pub enum Action {
    /// Tree-related actions.
    Tree(TreeAction),
}

/// Actions that modify the tree state.
#[derive(Debug, Clone)]
pub enum TreeAction {
    /// Placeholder variant to satisfy non-empty enum requirement.
    Noop,
}
