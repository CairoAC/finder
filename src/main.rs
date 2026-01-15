mod app;
mod chat;
mod compass;
mod search;
mod ui;
mod update;

use app::{App, Mode};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io::{self, stdout};
use std::process::Command;
use tokio::sync::mpsc;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.contains(&"--version".to_string()) || args.contains(&"-v".to_string()) {
        println!("finder {}", update::current_version());
        return Ok(());
    }

    if args.contains(&"--update".to_string()) {
        update::run_update();
        return Ok(());
    }

    let rt = tokio::runtime::Runtime::new().unwrap();

    let update_msg = rt.block_on(async {
        update::check_for_update().await
    });

    if let Some(new_version) = &update_msg {
        eprintln!(
            "Update available: {} -> {} (run `f --update` to upgrade)\n",
            update::current_version(),
            new_version
        );
    }

    let cwd = std::env::current_dir()?;
    let mut app = App::new(cwd);

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let result = rt.block_on(run(&mut terminal, &mut app));

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

    if let Some(entry) = app.selected_entry {
        let file_path = app.cwd.join(&entry.file);
        Command::new("nvim")
            .arg(format!("+{}", entry.line_num))
            .arg(&file_path)
            .status()?;
    }

    result
}

async fn run<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    loop {
        let visible_count = terminal.get_frame().area().height as usize / 3;

        while let Ok(chunk) = rx.try_recv() {
            app.append_response(&chunk);
        }

        terminal.draw(|frame| {
            ui::draw(frame, app);
        })?;

        if app.should_quit {
            return Ok(());
        }

        if event::poll(std::time::Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    match app.mode {
                        Mode::Search => match key.code {
                            KeyCode::Esc => app.on_escape(),
                            KeyCode::Enter => app.on_enter(),
                            KeyCode::Backspace => app.on_backspace(),
                            KeyCode::Up => app.on_up(),
                            KeyCode::Down => app.on_down(visible_count),
                            KeyCode::Char(c) => {
                                if key
                                    .modifiers
                                    .contains(crossterm::event::KeyModifiers::CONTROL)
                                    && c == 'c'
                                {
                                    app.on_escape();
                                } else {
                                    app.on_char(c);
                                }
                            }
                            _ => {}
                        },
                        Mode::Chat => match key.code {
                            KeyCode::Esc if !app.chat_streaming => app.on_escape(),
                            KeyCode::Enter => {
                                if !app.chat_streaming
                                    && !app.chat_input.is_empty()
                                    && app.api_key.is_some()
                                {
                                    let messages = app.build_messages();
                                    let api_key = app.api_key.clone().unwrap();
                                    let new_tx = tx.clone();

                                    app.start_chat();

                                    tokio::spawn(async move {
                                        let _ =
                                            chat::stream_chat(&api_key, messages, new_tx).await;
                                    });
                                }
                            }
                            KeyCode::Char('c')
                                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                            {
                                if app.chat_streaming {
                                    app.cancel_streaming();
                                } else {
                                    app.on_escape();
                                }
                            }
                            KeyCode::Backspace if !app.chat_streaming => app.on_backspace(),
                            KeyCode::Up => app.on_up(),
                            KeyCode::Down => app.on_down(visible_count),
                            KeyCode::Char(c) if !app.chat_streaming => {
                                if c == 'c'
                                    && app.chat_input.is_empty()
                                    && !app.citations.is_empty()
                                {
                                    app.enter_citations_mode();
                                } else {
                                    app.on_char(c);
                                }
                            }
                            _ => {}
                        },
                        Mode::Citations => match key.code {
                            KeyCode::Esc => app.on_escape(),
                            KeyCode::Enter => {
                                app.jump_to_citation(app.citations_selected);
                            }
                            KeyCode::Backspace => app.on_backspace(),
                            KeyCode::Up => app.on_up(),
                            KeyCode::Down => app.on_down(visible_count),
                            KeyCode::Char(c) => {
                                if key
                                    .modifiers
                                    .contains(crossterm::event::KeyModifiers::CONTROL)
                                    && c == 'c'
                                {
                                    app.on_escape();
                                } else {
                                    app.on_char(c);
                                }
                            }
                            _ => {}
                        },
                    }
                }
                Event::Mouse(mouse) => {
                    if app.mode == Mode::Search {
                        let results_start_row = 9;
                        match mouse.kind {
                            MouseEventKind::ScrollUp => app.on_up(),
                            MouseEventKind::ScrollDown => app.on_down(visible_count),
                            MouseEventKind::Down(MouseButton::Left) => {
                                if mouse.row >= results_start_row {
                                    let relative_row = (mouse.row - results_start_row) as usize;
                                    let clicked_idx = relative_row / 2 + app.scroll_offset;
                                    app.on_click(clicked_idx, visible_count);
                                }
                            }
                            _ => {}
                        }
                    } else {
                        match mouse.kind {
                            MouseEventKind::ScrollUp => app.on_up(),
                            MouseEventKind::ScrollDown => app.on_down(visible_count),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
