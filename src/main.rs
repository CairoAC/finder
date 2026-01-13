mod app;
mod compass;
mod search;
mod ui;

use app::App;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io::{self, stdout};
use std::process::Command;

fn main() -> io::Result<()> {
    let cwd = std::env::current_dir()?;
    let mut app = App::new(cwd);

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let result = run(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;

    if let Some(entry) = app.selected_entry {
        let file_path = app.cwd.join(&entry.file);
        Command::new("nvim")
            .arg(format!("+{}", entry.line_num))
            .arg(&file_path)
            .status()?;
    }

    result
}

fn run<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        let visible_count = terminal.get_frame().area().height as usize / 2;

        terminal.draw(|frame| {
            ui::draw(frame, app);
        })?;

        if app.should_quit {
            return Ok(());
        }

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Esc => app.on_escape(),
                    KeyCode::Enter => app.on_enter(),
                    KeyCode::Backspace => app.on_backspace(),
                    KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() || key.code == KeyCode::Up => {
                        app.on_up();
                    }
                    KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() || key.code == KeyCode::Down => {
                        app.on_down(visible_count);
                    }
                    KeyCode::Char(c) => {
                        if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) && c == 'c' {
                            app.on_escape();
                        } else {
                            app.on_char(c);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
