use std::{io, time::Duration};

use anyhow::Result;
use crossterm::{
    event::{DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use restui::{app::App, storage::Storage, ui};

#[tokio::main]
async fn main() -> Result<()> {
    let app = App::new(Storage::discover()?)?;
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableBracketedPaste
    )?;
    terminal.show_cursor()?;
    result
}

async fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, mut app: App) -> Result<()> {
    while !app.should_quit {
        app.poll_responses();
        terminal.draw(|frame| ui::draw(frame, &app))?;

        if crossterm::event::poll(Duration::from_millis(50))? {
            match crossterm::event::read()? {
                Event::Key(key)
                    if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                {
                    app.handle_key(key);
                }
                Event::Paste(text) => app.handle_paste(&text),
                _ => {}
            }
        }
    }
    Ok(())
}
