mod app;
mod chat;
mod compass;
mod markdown;
mod rag;
mod search;
mod ui;
mod update;

use app::{App, Mode};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io::{self, stdout, Write};
use std::process::Command;
use tokio::sync::mpsc;
fn copy_to_clipboard(text: &str) {
    use std::process::{Command, Stdio};

    let clean_text: String = text
        .chars()
        .filter(|c| !matches!(*c, '│' | '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' | '─' | '║' | '═'))
        .collect();

    let is_wsl = std::path::Path::new("/mnt/c/WINDOWS/system32/clip.exe").exists();

    let (cmd, args): (&str, &[&str]) = if is_wsl {
        ("clip.exe", &[])
    } else {
        ("xclip", &["-selection", "clipboard"])
    };

    if let Ok(mut child) = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(clean_text.as_bytes());
        }
        let _ = child.wait();
    }
}

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
    let (quick_tx, mut quick_rx) = mpsc::unbounded_channel::<String>();

    let mut selection_start: Option<(u16, u16)> = None;
    let mut selection_end: Option<(u16, u16)> = None;
    let mut selecting = false;
    let mut screen_buffer: Vec<String> = Vec::new();

    loop {
        while let Ok(chunk) = rx.try_recv() {
            app.append_response(&chunk);
        }

        while let Ok(chunk) = quick_rx.try_recv() {
            app.append_quick_response(&chunk);
        }

        let completed = terminal.draw(|frame| {
            ui::draw(frame, app, selection_start, selection_end);
        })?;

        screen_buffer.clear();
        for y in 0..completed.area.height {
            let mut line = String::new();
            for x in 0..completed.area.width {
                let cell = &completed.buffer[(x, y)];
                line.push_str(cell.symbol());
            }
            screen_buffer.push(line);
        }

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
                            KeyCode::Down => app.on_down(),
                            KeyCode::Char(c) => {
                                if key
                                    .modifiers
                                    .contains(crossterm::event::KeyModifiers::CONTROL)
                                {
                                    match c {
                                        'c' => app.on_escape(),
                                        'o' => app.enter_directory_picker(),
                                        _ => {}
                                    }
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
                            KeyCode::Char(c)
                                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                            {
                                match c {
                                    'c' => {
                                        if app.chat_streaming {
                                            app.cancel_streaming();
                                        } else {
                                            app.on_escape();
                                        }
                                    }
                                    'o' if !app.chat_streaming => app.enter_directory_picker(),
                                    _ => {}
                                }
                            }
                            KeyCode::Backspace if !app.chat_streaming => app.on_backspace(),
                            KeyCode::Up => app.on_up(),
                            KeyCode::Down => app.on_down(),
                            KeyCode::Char('c')
                                if key.modifiers.contains(crossterm::event::KeyModifiers::ALT)
                                    && !app.citations.is_empty() =>
                            {
                                app.enter_citations_mode();
                            }
                            KeyCode::Char(c) if !app.chat_streaming => {
                                app.on_char(c);
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
                            KeyCode::Down => app.on_down(),
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
                        Mode::DirectoryPicker => match key.code {
                            KeyCode::Esc => app.on_escape(),
                            KeyCode::Enter => app.select_directory(),
                            KeyCode::Backspace => app.on_backspace(),
                            KeyCode::Up => app.on_up(),
                            KeyCode::Down => app.on_down(),
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
                        Mode::QuickAnswer => match key.code {
                            KeyCode::Esc if !app.quick_streaming => app.on_escape(),
                            KeyCode::Tab => app.toggle_quick_sources(),
                            KeyCode::Up if app.quick_sources_expanded => app.quick_sources_up(),
                            KeyCode::Down if app.quick_sources_expanded => app.quick_sources_down(),
                            KeyCode::Enter => {
                                if app.quick_sources_expanded && !app.quick_sources.is_empty() {
                                    app.open_quick_source();
                                } else if !app.quick_streaming
                                    && !app.quick_query.is_empty()
                                    && app.api_key.is_some()
                                {
                                    app.prepare_quick_search();
                                    let messages = app.build_quick_messages();
                                    let api_key = app.api_key.clone().unwrap();
                                    let new_tx = quick_tx.clone();

                                    app.start_quick_answer();

                                    tokio::spawn(async move {
                                        let _ =
                                            chat::stream_chat(&api_key, messages, new_tx).await;
                                    });
                                }
                            }
                            KeyCode::Char(c)
                                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                            {
                                match c {
                                    'c' => {
                                        if app.quick_streaming {
                                            app.cancel_quick();
                                        } else {
                                            app.on_escape();
                                        }
                                    }
                                    'r' if !app.quick_streaming => {
                                        app.rebuild_rag_index();
                                    }
                                    'n' if !app.quick_streaming => {
                                        app.new_quick_conversation();
                                    }
                                    _ => {}
                                }
                            }
                            KeyCode::Backspace if !app.quick_streaming => app.on_backspace(),
                            KeyCode::Char(c) if !app.quick_streaming => app.on_char(c),
                            _ => {}
                        },
                    }
                }
                Event::Mouse(mouse) => {
                    match mouse.kind {
                        MouseEventKind::Down(MouseButton::Left) => {
                            selection_start = Some((mouse.column, mouse.row));
                            selection_end = Some((mouse.column, mouse.row));
                            selecting = true;
                        }
                        MouseEventKind::Drag(MouseButton::Left) => {
                            if selecting {
                                selection_end = Some((mouse.column, mouse.row));
                            }
                        }
                        MouseEventKind::Up(MouseButton::Left) => {
                            if selecting {
                                selection_end = Some((mouse.column, mouse.row));
                                selecting = false;

                                if let (Some(start), Some(end)) = (selection_start, selection_end) {
                                    let text = extract_text(&screen_buffer, start, end);
                                    if !text.is_empty() {
                                        copy_to_clipboard(&text);
                                    }
                                }

                                selection_start = None;
                                selection_end = None;
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

fn extract_text(buffer: &[String], start: (u16, u16), end: (u16, u16)) -> String {
    let (start, end) = if start.1 < end.1 || (start.1 == end.1 && start.0 <= end.0) {
        (start, end)
    } else {
        (end, start)
    };

    let (start_col, start_row) = (start.0 as usize, start.1 as usize);
    let (end_col, end_row) = (end.0 as usize, end.1 as usize);

    let mut result = String::new();

    for row in start_row..=end_row {
        if row >= buffer.len() {
            continue;
        }

        let line = &buffer[row];
        let chars: Vec<char> = line.chars().collect();

        let col_start = if row == start_row { start_col } else { 0 };
        let col_end = if row == end_row { (end_col + 1).min(chars.len()) } else { chars.len() };

        if col_start < chars.len() {
            let selected: String = chars[col_start..col_end.min(chars.len())].iter().collect();
            result.push_str(selected.trim_end());
        }

        if row != end_row {
            result.push('\n');
        }
    }

    result.trim().to_string()
}
