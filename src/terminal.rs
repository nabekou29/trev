//! Terminal initialization and restoration helpers.

use std::io::stdout;

use crossterm::event::{
    KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use ratatui::DefaultTerminal;

/// Initialize the terminal for TUI rendering.
///
/// Sets up raw mode, alternate screen, and installs a panic hook
/// that restores the terminal before printing the panic message.
/// Also enables keyboard enhancement (Kitty protocol) if the terminal supports it,
/// so that modified keys like Shift+Enter can be distinguished.
pub fn init() -> DefaultTerminal {
    let terminal = ratatui::init();

    // Enable keyboard enhancement for Shift+Enter etc.
    // Silently ignored if the terminal does not support it.
    let _ = crossterm::execute!(
        stdout(),
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    );

    terminal
}

/// Restore the terminal to its original state.
///
/// Disables raw mode, leaves the alternate screen, and pops keyboard enhancement.
pub fn restore() {
    let _ = crossterm::execute!(stdout(), PopKeyboardEnhancementFlags);
    ratatui::restore();
}
