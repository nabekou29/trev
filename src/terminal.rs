//! Terminal initialization and restoration helpers.

use ratatui::DefaultTerminal;

/// Initialize the terminal for TUI rendering.
///
/// Sets up raw mode, alternate screen, and installs a panic hook
/// that restores the terminal before printing the panic message.
pub fn init() -> DefaultTerminal {
    ratatui::init()
}

/// Restore the terminal to its original state.
///
/// Disables raw mode and leaves the alternate screen.
pub fn restore() {
    ratatui::restore();
}

