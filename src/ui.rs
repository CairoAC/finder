use crate::app::{App, Mode};
use crate::compass::COMPASS;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph, Wrap},
    Frame,
};

const BLUE: Color = Color::Rgb(100, 149, 237);
const DIM: Color = Color::Rgb(128, 128, 128);
const HIGHLIGHT: Color = Color::Rgb(255, 200, 100);

pub fn draw(frame: &mut Frame, app: &App) {
    match app.mode {
        Mode::Search => draw_search(frame, app),
        Mode::Chat => draw_chat(frame, app),
    }
}

fn calc_input_height(text_len: usize, width: u16) -> u16 {
    let inner_width = width.saturating_sub(4) as usize;
    if inner_width == 0 {
        return 3;
    }
    let lines = (text_len + 3).div_ceil(inner_width);
    (lines as u16 + 2).max(3)
}

fn draw_search(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let input_height = calc_input_height(app.query.len(), area.width);

    let chunks = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(input_height),
        Constraint::Min(1),
    ])
    .split(area);

    draw_header(frame, chunks[0], app);
    draw_search_input(frame, chunks[1], app);
    draw_results(frame, chunks[2], app);
}

fn draw_chat(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let input_height = calc_input_height(app.chat_input.len(), area.width);

    let chunks = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(input_height),
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
        .border_style(Style::default().fg(DIM))
        .padding(Padding::horizontal(1));

    let text = format!("> {}_", app.query);
    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false })
        .block(block);
    frame.render_widget(paragraph, area);
}

fn draw_chat_input(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM))
        .padding(Padding::horizontal(1));

    let text = format!("? {}_", app.chat_input);
    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false })
        .block(block);
    frame.render_widget(paragraph, area);
}

fn draw_results(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::horizontal([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .split(area);

    draw_results_list(frame, chunks[0], app);
    draw_preview(frame, chunks[1], app);
}

fn draw_results_list(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM))
        .padding(Padding::horizontal(1));

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

            let max_content_width = area.width.saturating_sub(8) as usize;
            let truncated_content: String = entry.content.chars().take(max_content_width).collect();
            let truncated_len = truncated_content.chars().count();
            let suffix = if entry.content.chars().count() > max_content_width { "..." } else { "" };

            let truncated_indices: Vec<u32> = entry
                .match_indices
                .iter()
                .filter(|&&i| (i as usize) < truncated_len)
                .copied()
                .collect();

            let mut content_spans = vec![Span::raw("  \"")];
            content_spans.extend(highlight_text(&truncated_content, &truncated_indices, content_style));
            content_spans.push(Span::styled(format!("{}\"", suffix), content_style));

            let lines = vec![
                Line::from(vec![
                    Span::styled(marker, marker_style),
                    Span::styled(format!(" {}:{}", entry.file, entry.line_num), file_style),
                ]),
                Line::from(content_spans),
            ];

            ListItem::new(lines)
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner);
}

fn draw_preview(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(entry) = app.results.get(app.selected) else {
        let paragraph = Paragraph::new(Span::styled("No preview", Style::default().fg(DIM)));
        frame.render_widget(paragraph, inner);
        return;
    };

    let file_path = app.cwd.join(&entry.file);
    let Ok(content) = std::fs::read_to_string(&file_path) else {
        let paragraph = Paragraph::new(Span::styled("Cannot read file", Style::default().fg(DIM)));
        frame.render_widget(paragraph, inner);
        return;
    };

    let lines: Vec<&str> = content.lines().collect();
    let target_line = entry.line_num.saturating_sub(1);
    let visible_lines = inner.height as usize;
    let half_visible = visible_lines / 2;

    let start_line = target_line.saturating_sub(half_visible);
    let end_line = (start_line + visible_lines).min(lines.len());

    let preview_lines: Vec<Line> = lines[start_line..end_line]
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let actual_line_num = start_line + i + 1;
            let is_target = actual_line_num == entry.line_num;

            let line_num_style = if is_target {
                Style::default().fg(HIGHLIGHT)
            } else {
                Style::default().fg(DIM)
            };

            let content_style = if is_target {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(DIM)
            };

            let max_width = inner.width.saturating_sub(6) as usize;
            let truncated: String = line.chars().take(max_width).collect();

            Line::from(vec![
                Span::styled(format!("{:>4} ", actual_line_num), line_num_style),
                Span::styled(truncated, content_style),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(preview_lines);
    frame.render_widget(paragraph, inner);
}

fn draw_chat_response(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM))
        .padding(Padding::horizontal(1));

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

fn highlight_text(text: &str, indices: &[u32], base_style: Style) -> Vec<Span<'static>> {
    let highlight_style = base_style.fg(HIGHLIGHT);
    let chars: Vec<char> = text.chars().collect();
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut is_highlight = false;

    for (i, &c) in chars.iter().enumerate() {
        let should_highlight = indices.contains(&(i as u32));

        if should_highlight != is_highlight {
            if !current.is_empty() {
                let style = if is_highlight { highlight_style } else { base_style };
                spans.push(Span::styled(current.clone(), style));
                current.clear();
            }
            is_highlight = should_highlight;
        }
        current.push(c);
    }

    if !current.is_empty() {
        let style = if is_highlight { highlight_style } else { base_style };
        spans.push(Span::styled(current, style));
    }

    spans
}

fn draw_chat_footer(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut hints: Vec<Span> = if app.chat_streaming {
        vec![
            Span::styled("streaming... ", Style::default().fg(BLUE)),
            Span::styled("[Ctrl+C]", Style::default().fg(DIM)),
            Span::styled(" cancel", Style::default().fg(DIM)),
        ]
    } else {
        vec![
            Span::styled("[Esc]", Style::default().fg(BLUE)),
            Span::styled(" back", Style::default().fg(DIM)),
        ]
    };

    if !app.chat_streaming && !app.citations.is_empty() && app.chat_input.is_empty() {
        hints.push(Span::styled("  |  ", Style::default().fg(DIM)));
        for (i, citation) in app.citations.iter().take(9).enumerate() {
            hints.push(Span::styled(format!("[{}]", i + 1), Style::default().fg(HIGHLIGHT)));
            hints.push(Span::styled(format!(" {}:{} ", citation.file, citation.line), Style::default().fg(DIM)));
        }
    }

    let paragraph = Paragraph::new(Line::from(hints));
    frame.render_widget(paragraph, inner);
}
