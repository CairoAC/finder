use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, HeadingLevel, CodeBlockKind};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};

const DIM: Color = Color::Rgb(140, 140, 140);
const YELLOW: Color = Color::Rgb(255, 200, 100);
const CODE_BG: Color = Color::Rgb(30, 30, 35);
const CODE_FG: Color = Color::Rgb(180, 180, 180);

pub fn render(input: &str) -> Text<'static> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(input, opts);
    let mut renderer = MarkdownRenderer::new();
    renderer.run(parser);
    renderer.into_text()
}

struct MarkdownRenderer {
    lines: Vec<Line<'static>>,
    current_spans: Vec<Span<'static>>,
    style_stack: Vec<Style>,
    list_stack: Vec<Option<u64>>,
    in_code_block: bool,
    code_block_lang: String,
    needs_newline: bool,
    blockquote_depth: usize,
}

impl MarkdownRenderer {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            current_spans: Vec::new(),
            style_stack: vec![Style::default().fg(Color::White)],
            list_stack: Vec::new(),
            in_code_block: false,
            code_block_lang: String::new(),
            needs_newline: false,
            blockquote_depth: 0,
        }
    }

    fn run<'a>(&mut self, parser: Parser<'a>) {
        for event in parser {
            self.handle_event(event);
        }
        self.flush_line();
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.text(&text),
            Event::Code(code) => self.inline_code(&code),
            Event::SoftBreak => self.soft_break(),
            Event::HardBreak => self.hard_break(),
            Event::Rule => self.rule(),
            Event::TaskListMarker(checked) => self.task_marker(checked),
            _ => {}
        }
    }

    fn start_tag(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => self.start_paragraph(),
            Tag::Heading { level, .. } => self.start_heading(level),
            Tag::BlockQuote(_) => self.start_blockquote(),
            Tag::CodeBlock(kind) => self.start_code_block(kind),
            Tag::List(start) => self.start_list(start),
            Tag::Item => self.start_item(),
            Tag::Emphasis => self.push_style(Style::default().add_modifier(Modifier::ITALIC)),
            Tag::Strong => self.push_style(Style::default().add_modifier(Modifier::BOLD)),
            Tag::Strikethrough => self.push_style(Style::default().add_modifier(Modifier::CROSSED_OUT)),
            Tag::Link { .. } => self.push_style(Style::default().add_modifier(Modifier::UNDERLINED)),
            _ => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.end_paragraph(),
            TagEnd::Heading(_) => self.end_heading(),
            TagEnd::BlockQuote(_) => self.end_blockquote(),
            TagEnd::CodeBlock => self.end_code_block(),
            TagEnd::List(_) => self.end_list(),
            TagEnd::Item => {}
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link => {
                self.pop_style();
            }
            _ => {}
        }
    }

    fn start_paragraph(&mut self) {
        if self.needs_newline {
            self.push_line(Line::default());
        }
        self.needs_newline = false;
    }

    fn end_paragraph(&mut self) {
        self.flush_line();
        self.needs_newline = true;
    }

    fn start_heading(&mut self, level: HeadingLevel) {
        self.flush_line();
        if !self.lines.is_empty() {
            self.push_line(Line::default());
        }

        let style = match level {
            HeadingLevel::H1 => Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            HeadingLevel::H2 => Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H3 => Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD | Modifier::ITALIC),
            _ => Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::ITALIC),
        };

        self.push_style(style);
        self.needs_newline = false;
    }

    fn end_heading(&mut self) {
        self.pop_style();
        self.flush_line();
        self.push_line(Line::default());
        self.needs_newline = false;
    }

    fn start_blockquote(&mut self) {
        if self.needs_newline && self.blockquote_depth == 0 {
            self.push_line(Line::default());
        }
        self.blockquote_depth += 1;
        self.push_style(Style::default().fg(DIM).add_modifier(Modifier::ITALIC));
        self.needs_newline = false;
    }

    fn end_blockquote(&mut self) {
        self.blockquote_depth = self.blockquote_depth.saturating_sub(1);
        self.pop_style();
        self.needs_newline = true;
    }

    fn start_code_block(&mut self, kind: CodeBlockKind) {
        if !self.lines.is_empty() {
            self.push_line(Line::default());
        }

        self.code_block_lang = match kind {
            CodeBlockKind::Fenced(lang) => lang.to_string(),
            CodeBlockKind::Indented => String::new(),
        };

        let header = format!("```{}", self.code_block_lang);
        self.push_line(Line::styled(header, Style::default().fg(CODE_FG).bg(CODE_BG)));
        self.in_code_block = true;
        self.needs_newline = false;
    }

    fn end_code_block(&mut self) {
        self.push_line(Line::styled("```", Style::default().fg(CODE_FG).bg(CODE_BG)));
        self.in_code_block = false;
        self.needs_newline = true;
    }

    fn start_list(&mut self, start: Option<u64>) {
        if self.list_stack.is_empty() && self.needs_newline {
            self.push_line(Line::default());
        }
        self.list_stack.push(start);
        self.needs_newline = false;
    }

    fn end_list(&mut self) {
        self.list_stack.pop();
        self.needs_newline = true;
    }

    fn start_item(&mut self) {
        self.flush_line();

        let indent = "  ".repeat(self.list_stack.len().saturating_sub(1));

        if let Some(last) = self.list_stack.last_mut() {
            match last {
                None => {
                    let bullet = Span::styled(
                        format!("{}• ", indent),
                        Style::default().fg(Color::White),
                    );
                    self.current_spans.push(bullet);
                }
                Some(num) => {
                    let number = Span::styled(
                        format!("{}{}. ", indent, num),
                        Style::default().fg(Color::White),
                    );
                    *num += 1;
                    self.current_spans.push(number);
                }
            }
        }
        self.needs_newline = false;
    }

    fn task_marker(&mut self, checked: bool) {
        let marker = if checked { "[x] " } else { "[ ] " };
        self.current_spans.push(Span::styled(
            marker.to_string(),
            Style::default().fg(if checked { Color::White } else { DIM }),
        ));
    }

    fn text(&mut self, text: &str) {
        if self.in_code_block {
            for line in text.lines() {
                self.push_line(Line::styled(
                    format!("  {}", line),
                    Style::default().fg(CODE_FG).bg(CODE_BG),
                ));
            }
            return;
        }

        let style = self.current_style();

        if self.blockquote_depth > 0 {
            let prefix = "│ ".repeat(self.blockquote_depth);
            for (i, line) in text.lines().enumerate() {
                if i > 0 {
                    self.flush_line();
                }
                if self.current_spans.is_empty() || i > 0 {
                    self.current_spans.push(Span::styled(
                        prefix.clone(),
                        Style::default().fg(DIM),
                    ));
                }
                self.current_spans.push(Span::styled(line.to_string(), style));
            }
        } else {
            self.current_spans.push(Span::styled(text.to_string(), style));
        }
    }

    fn inline_code(&mut self, code: &str) {
        self.current_spans.push(Span::styled(
            format!(" {} ", code),
            Style::default().fg(CODE_FG).bg(CODE_BG),
        ));
    }

    fn soft_break(&mut self) {
        self.current_spans.push(Span::raw(" "));
    }

    fn hard_break(&mut self) {
        self.flush_line();
    }

    fn rule(&mut self) {
        self.flush_line();
        if self.needs_newline {
            self.push_line(Line::default());
        }
        self.push_line(Line::styled(
            "─".repeat(40),
            Style::default().fg(Color::DarkGray),
        ));
        self.needs_newline = true;
    }

    fn push_style(&mut self, style: Style) {
        let current = self.current_style();
        let new_style = current.patch(style);
        self.style_stack.push(new_style);
    }

    fn pop_style(&mut self) {
        if self.style_stack.len() > 1 {
            self.style_stack.pop();
        }
    }

    fn current_style(&self) -> Style {
        *self.style_stack.last().unwrap_or(&Style::default())
    }

    fn flush_line(&mut self) {
        if !self.current_spans.is_empty() {
            let spans = std::mem::take(&mut self.current_spans);
            self.lines.push(Line::from(spans));
        }
    }

    fn push_line(&mut self, line: Line<'static>) {
        self.flush_line();
        self.lines.push(line);
    }

    fn into_text(mut self) -> Text<'static> {
        self.flush_line();
        Text::from(self.lines)
    }
}

pub fn highlight_citations(text: Text<'static>) -> Text<'static> {
    let citation_re = regex::Regex::new(r"\[([^\]]+:\d+(?:[-,]\s*\d+)*)\]").unwrap();

    let new_lines: Vec<Line<'static>> = text
        .lines
        .into_iter()
        .map(|line| {
            let full_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

            if !citation_re.is_match(&full_text) {
                return line;
            }

            let mut char_styles: Vec<Style> = Vec::new();
            for span in &line.spans {
                for _ in span.content.chars() {
                    char_styles.push(span.style);
                }
            }

            let mut new_spans: Vec<Span<'static>> = Vec::new();
            let mut last_end = 0;

            for cap in citation_re.captures_iter(&full_text) {
                let m = cap.get(0).unwrap();

                if m.start() > last_end {
                    let segment = &full_text[last_end..m.start()];
                    let style = char_styles.get(last_end).copied().unwrap_or_default();
                    new_spans.push(Span::styled(segment.to_string(), style));
                }

                new_spans.push(Span::styled(
                    m.as_str().to_string(),
                    Style::default().fg(YELLOW).add_modifier(Modifier::DIM),
                ));

                last_end = m.end();
            }

            if last_end < full_text.len() {
                let segment = &full_text[last_end..];
                let style = char_styles.get(last_end).copied().unwrap_or_default();
                new_spans.push(Span::styled(segment.to_string(), style));
            }

            Line::from(new_spans)
        })
        .collect();

    Text::from(new_lines)
}
