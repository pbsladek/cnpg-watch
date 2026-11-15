use cnpg_watch::terminal::TerminalGuard;
use std::io::Read;

fn main() -> anyhow::Result<()> {
    let mut terminal = TerminalGuard::enter()?;
    terminal
        .screen()
        .draw(|f| f.render_widget("READY", f.area()))?;
    // Isolate ownership/restoration from the application's input event parser.
    let mut key = [0];
    std::io::stdin().read_exact(&mut key)?;
    match key[0] {
        b'p' => panic!("terminal lifecycle test"),
        b'e' => anyhow::bail!("terminal lifecycle test"),
        _ => Ok(()),
    }
}
