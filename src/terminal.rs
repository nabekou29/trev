//! Terminal initialization and restoration helpers.

use std::io::stdout;

use crossterm::event::{
    DisableBracketedPaste,
    DisableMouseCapture,
    EnableBracketedPaste,
    EnableMouseCapture,
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
/// When `mouse` is true, enables mouse capture for click and scroll wheel events.
pub fn init(mouse: bool) -> DefaultTerminal {
    let terminal = ratatui::init();

    if mouse {
        // Enable mouse capture for click and scroll wheel events.
        let _ = crossterm::execute!(stdout(), EnableMouseCapture);
    }

    // Enable keyboard enhancement for Shift+Enter etc.
    // Silently ignored if the terminal does not support it.
    let _ = crossterm::execute!(
        stdout(),
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    );

    // Enable bracketed paste so Cmd+V triggers Event::Paste.
    let _ = crossterm::execute!(stdout(), EnableBracketedPaste);

    terminal
}

/// Restore the terminal to its original state.
///
/// Disables raw mode, leaves the alternate screen, pops keyboard enhancement,
/// and disables mouse capture (safe to call even if mouse was never enabled).
pub fn restore() {
    let _ = crossterm::execute!(stdout(), DisableBracketedPaste);
    let _ = crossterm::execute!(stdout(), PopKeyboardEnhancementFlags);
    let _ = crossterm::execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
}
