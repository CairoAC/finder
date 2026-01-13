use crate::app::App;
use crate::compass::COMPASS;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

const BLUE: Color = Color::Rgb(100, 149, 237);
const DIM: Color = Color::Rgb(128, 128, 128);

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(3),
        Constraint::Min(1),
    ])
    .split(area);

    draw_header(frame, chunks[0], app);
    draw_input(frame, chunks[1], app);
    draw_results(frame, chunks[2], app);
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
        .replace(dirs::home_dir().map(|h| h.to_string_lossy().to_string()).unwrap_or_default().as_str(), "~");

    let lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled(COMPASS[0], compass_style),
            Span::styled("  Finder ", text_style.add_modifier(Modifier::BOLD)),
            Span::styled("v0.1.0", dim_style),
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

fn draw_input(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let input_line = Line::from(vec![
        Span::styled("> ", Style::default().fg(BLUE)),
        Span::styled(&app.query, Style::default().fg(Color::White)),
        Span::styled("_", Style::default().fg(BLUE).add_modifier(Modifier::SLOW_BLINK)),
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
            "Type to search..."
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
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
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

pub fn visible_result_count(frame: &Frame) -> usize {
    let area = frame.area();
    let header_and_input = 8;
    let available = area.height.saturating_sub(header_and_input) as usize;
    available / 2
}
