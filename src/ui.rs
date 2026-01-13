use crate::app::{App, Mode};
use crate::compass::COMPASS;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

const BLUE: Color = Color::Rgb(100, 149, 237);
const DIM: Color = Color::Rgb(128, 128, 128);

pub fn draw(frame: &mut Frame, app: &App) {
    match app.mode {
        Mode::Search => draw_search(frame, app),
        Mode::Chat => draw_chat(frame, app),
    }
}

fn draw_search(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(3),
        Constraint::Min(1),
    ])
    .split(area);

    draw_header(frame, chunks[0], app);
    draw_search_input(frame, chunks[1], app);
    draw_results(frame, chunks[2], app);
}

fn draw_chat(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .split(area);

    draw_header(frame, chunks[0], app);
    draw_chat_input(frame, chunks[1], app);
    draw_chat_response(frame, chunks[2], app);
    draw_chat_footer(frame, chunks[3], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let compass_style = Style::default().fg(BLUE);
    let text_style = Style::default().fg(Color::White);
    let dim_style = Style::default().fg(DIM);

    let cwd_display = app
        .cwd
        .to_string_lossy()
        .replace(
            dirs::home_dir()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_default()
                .as_str(),
            "~",
        );

    let mode_indicator = match app.mode {
        Mode::Search => "",
        Mode::Chat => " [CHAT]",
    };

    let lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled(COMPASS[0], compass_style),
            Span::styled("  Finder ", text_style.add_modifier(Modifier::BOLD)),
            Span::styled("v0.1.0", dim_style),
            Span::styled(mode_indicator, Style::default().fg(BLUE)),
        ]),
        Line::from(vec![
            Span::styled(COMPASS[1], compass_style),
            Span::styled(format!("  {}", cwd_display), dim_style),
        ]),
        Line::from(vec![
            Span::styled(COMPASS[2], compass_style),
            Span::styled(format!("  {} lines indexed", app.entry_count), dim_style),
        ]),
    ];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

fn draw_search_input(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let input_line = Line::from(vec![
        Span::styled("> ", Style::default().fg(BLUE)),
        Span::styled(&app.query, Style::default().fg(Color::White)),
        Span::styled(
            "_",
            Style::default().fg(BLUE).add_modifier(Modifier::SLOW_BLINK),
        ),
    ]);

    let paragraph = Paragraph::new(input_line);
    frame.render_widget(paragraph, inner);
}

fn draw_chat_input(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let input_line = Line::from(vec![
        Span::styled("? ", Style::default().fg(BLUE)),
        Span::styled(&app.chat_input, Style::default().fg(Color::White)),
        Span::styled(
            "_",
            Style::default().fg(BLUE).add_modifier(Modifier::SLOW_BLINK),
        ),
    ]);

    let paragraph = Paragraph::new(input_line);
    frame.render_widget(paragraph, inner);
}

fn draw_results(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.results.is_empty() {
        let msg = if app.query.is_empty() {
            "Type to search... (press ? for chat)"
        } else {
            "No results"
        };
        let paragraph = Paragraph::new(Span::styled(msg, Style::default().fg(DIM)));
        frame.render_widget(paragraph, inner);
        return;
    }

    let visible_height = inner.height as usize / 2;
    let items: Vec<ListItem> = app
        .results
        .iter()
        .enumerate()
        .skip(app.scroll_offset)
        .take(visible_height)
        .map(|(idx, entry)| {
            let is_selected = idx == app.selected;
            let marker = if is_selected { ">" } else { " " };
            let marker_style = Style::default().fg(BLUE);

            let file_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let content_style = Style::default().fg(DIM);

            let truncated_content: String = entry.content.chars().take(60).collect();
            let content_display = if entry.content.len() > 60 {
                format!("{}...", truncated_content)
            } else {
                truncated_content
            };

            let lines = vec![
                Line::from(vec![
                    Span::styled(marker, marker_style),
                    Span::styled(format!(" {}:{}", entry.file, entry.line_num), file_style),
                ]),
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("\"{}\"", content_display), content_style),
                ]),
            ];

            ListItem::new(lines)
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner);
}

fn draw_chat_response(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.api_key.is_none() {
        let paragraph = Paragraph::new(Span::styled(
            "OPENROUTER_API_KEY not found. Set it in ~/.env or environment.",
            Style::default().fg(Color::Red),
        ));
        frame.render_widget(paragraph, inner);
        return;
    }

    let content = if app.chat_response.is_empty() && !app.chat_streaming {
        if app.chat_messages.is_empty() {
            "Type your question and press Enter...".to_string()
        } else {
            let mut history = String::new();
            for msg in &app.chat_messages {
                let prefix = if msg.role == "user" { "You: " } else { "AI: " };
                history.push_str(prefix);
                history.push_str(&msg.content);
                history.push_str("\n\n");
            }
            history
        }
    } else if app.chat_streaming {
        format!("{}|", app.chat_response)
    } else {
        app.chat_response.clone()
    };

    let style = if app.chat_response.is_empty() && app.chat_messages.is_empty() {
        Style::default().fg(DIM)
    } else {
        Style::default().fg(Color::White)
    };

    let paragraph = Paragraph::new(content)
        .style(style)
        .wrap(Wrap { trim: false })
        .scroll((app.chat_scroll as u16, 0));

    frame.render_widget(paragraph, inner);
}

fn draw_chat_footer(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let hints = if app.chat_streaming {
        vec![Span::styled("streaming...", Style::default().fg(BLUE))]
    } else {
        vec![
            Span::styled("[f]", Style::default().fg(BLUE)),
            Span::styled(" follow-up  ", Style::default().fg(DIM)),
            Span::styled("[Esc]", Style::default().fg(BLUE)),
            Span::styled(" back to search  ", Style::default().fg(DIM)),
            Span::styled("[q]", Style::default().fg(BLUE)),
            Span::styled(" quit", Style::default().fg(DIM)),
        ]
    };

    let paragraph = Paragraph::new(Line::from(hints));
    frame.render_widget(paragraph, inner);
}
