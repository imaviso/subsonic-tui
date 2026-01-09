//! Terminal setup and teardown utilities.

use std::io::{stdout, Stdout};

use color_eyre::Result;
use crossterm::{
    cursor,
    event::{DisableMouseCapture, EnableMouseCapture},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;

/// A type alias for the terminal type used in this application.
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initialize the terminal for TUI rendering.
pub fn init() -> Result<Tui> {
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(EnableMouseCapture)?;
    stdout().execute(cursor::Hide)?;
    enable_raw_mode()?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    Ok(terminal)
}

/// Restore the terminal to its original state.
pub fn restore() -> Result<()> {
    stdout().execute(cursor::Show)?;
    stdout().execute(DisableMouseCapture)?;
    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;

    Ok(())
}

/// Install panic and error hooks that restore the terminal before printing errors.
pub fn install_hooks() -> Result<()> {
    let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::default()
        .panic_section(format!(
            "This is a bug. Please report it at: {}",
            env!("CARGO_PKG_REPOSITORY")
        ))
        .into_hooks();

    // Set the panic hook
    let panic_hook = panic_hook.into_panic_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore();
        panic_hook(panic_info);
    }));

    // Set the error hook
    let eyre_hook = eyre_hook.into_eyre_hook();
    color_eyre::eyre::set_hook(Box::new(move |error| {
        let _ = restore();
        eyre_hook(error)
    }))?;

    Ok(())
}
