use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::{self, IsTerminal, Stdout};

pub type Screen = Terminal<CrosstermBackend<Stdout>>;

pub fn restore() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
}

/// Restores the terminal on errors/unwind; a hook also restores before panic output.
pub struct TerminalGuard {
    screen: Screen,
}

impl TerminalGuard {
    pub fn enter() -> io::Result<Self> {
        if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
            return Err(io::Error::other("watch requires an interactive terminal"));
        }
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            previous(info);
        }));
        enable_raw_mode()?;
        if let Err(error) = execute!(io::stdout(), EnterAlternateScreen) {
            restore();
            return Err(error);
        }
        match Terminal::new(CrosstermBackend::new(io::stdout())) {
            Ok(screen) => Ok(Self { screen }),
            Err(error) => {
                restore();
                Err(error)
            }
        }
    }

    pub fn screen(&mut self) -> &mut Screen {
        &mut self.screen
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore();
    }
}
