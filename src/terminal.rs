//! Terminal initialization and restoration helpers.

use std::io;

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

/// Install a custom panic hook that restores the terminal before panicking.
///
/// This ensures the terminal is always restored even if the application panics.
#[expect(dead_code, reason = "Will be called from main() once error handling is finalized")]
pub fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Best-effort terminal restoration — ignore errors.
        let _ = crossterm::execute!(io::stderr(), crossterm::terminal::LeaveAlternateScreen);
        let _ = crossterm::terminal::disable_raw_mode();
        original_hook(panic_info);
    }));
}
