use crate::chat::ChatMessage;
use crate::search::{SearchEntry, Searcher};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Search,
    Chat,
    Citations,
}

#[derive(Debug, Clone)]
pub struct Citation {
    pub file: String,
    pub line: usize,
}

pub struct App {
    pub query: String,
    pub results: Vec<SearchEntry>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub should_quit: bool,
    pub selected_entry: Option<SearchEntry>,
    pub cwd: PathBuf,
    pub entry_count: usize,
    pub mode: Mode,
    pub chat_input: String,
    pub chat_messages: Vec<ChatMessage>,
    pub chat_response: String,
    pub chat_streaming: bool,
    pub chat_scroll: usize,
    pub md_context: String,
    pub api_key: Option<String>,
    pub citations: Vec<Citation>,
    pub citations_query: String,
    pub citations_filtered: Vec<Citation>,
    pub citations_selected: usize,
    searcher: Searcher,
}

impl App {
    pub fn new(cwd: PathBuf) -> Self {
        let searcher = Searcher::new(&cwd);
        let entry_count = searcher.entry_count();
        let md_context = crate::chat::load_context(&cwd);
        let api_key = crate::chat::find_api_key();

        Self {
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            should_quit: false,
            selected_entry: None,
            cwd,
            entry_count,
            mode: Mode::Search,
            chat_input: String::new(),
            chat_messages: Vec::new(),
            chat_response: String::new(),
            chat_streaming: false,
            chat_scroll: 0,
            md_context,
            api_key,
            citations: Vec::new(),
            citations_query: String::new(),
            citations_filtered: Vec::new(),
            citations_selected: 0,
            searcher,
        }
    }

    pub fn parse_citations(&mut self) {
        self.citations.clear();
        let re = regex::Regex::new(r"\[([^\]]+):(\d+)\]").unwrap();
        for cap in re.captures_iter(&self.chat_response) {
            let file = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            let line = cap.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
            if !self.citations.iter().any(|c| c.file == file && c.line == line) {
                self.citations.push(Citation { file, line });
            }
        }
    }

    pub fn jump_to_citation(&mut self, idx: usize) {
        let citations = if self.citations_query.is_empty() {
            &self.citations
        } else {
            &self.citations_filtered
        };
        if let Some(citation) = citations.get(idx) {
            self.selected_entry = Some(SearchEntry {
                file: citation.file.clone(),
                line_num: citation.line,
                content: String::new(),
                match_indices: Vec::new(),
            });
            self.should_quit = true;
        }
    }

    pub fn enter_citations_mode(&mut self) {
        if !self.citations.is_empty() {
            self.mode = Mode::Citations;
            self.citations_query.clear();
            self.citations_filtered.clear();
            self.citations_selected = 0;
        }
    }

    pub fn filter_citations(&mut self) {
        if self.citations_query.is_empty() {
            self.citations_filtered.clear();
            self.citations_selected = 0;
            return;
        }

        let query = self.citations_query.to_lowercase();
        self.citations_filtered = self
            .citations
            .iter()
            .filter(|c| c.file.to_lowercase().contains(&query))
            .cloned()
            .collect();
        self.citations_selected = 0;
    }

    pub fn citations_count(&self) -> usize {
        if self.citations_query.is_empty() {
            self.citations.len()
        } else {
            self.citations_filtered.len()
        }
    }

    pub fn on_char(&mut self, c: char) {
        match self.mode {
            Mode::Search => {
                if c == '?' {
                    self.mode = Mode::Chat;
                } else {
                    self.query.push(c);
                    self.update_search();
                }
            }
            Mode::Chat => {
                if !self.chat_streaming {
                    self.chat_input.push(c);
                }
            }
            Mode::Citations => {
                self.citations_query.push(c);
                self.filter_citations();
            }
        }
    }

    pub fn on_backspace(&mut self) {
        match self.mode {
            Mode::Search => {
                self.query.pop();
                self.update_search();
            }
            Mode::Chat => {
                if !self.chat_streaming {
                    self.chat_input.pop();
                }
            }
            Mode::Citations => {
                self.citations_query.pop();
                self.filter_citations();
            }
        }
    }

    pub fn on_up(&mut self) {
        match self.mode {
            Mode::Search => {
                if self.selected > 0 {
                    self.selected -= 1;
                    if self.selected < self.scroll_offset {
                        self.scroll_offset = self.selected;
                    }
                }
            }
            Mode::Chat => {
                if self.chat_scroll > 0 {
                    self.chat_scroll -= 1;
                }
            }
            Mode::Citations => {
                if self.citations_selected > 0 {
                    self.citations_selected -= 1;
                }
            }
        }
    }

    pub fn on_down(&mut self, visible_count: usize) {
        match self.mode {
            Mode::Search => {
                if self.selected + 1 < self.results.len() {
                    self.selected += 1;
                    if self.selected >= self.scroll_offset + visible_count {
                        self.scroll_offset = self.selected - visible_count + 1;
                    }
                }
            }
            Mode::Chat => {
                self.chat_scroll += 1;
            }
            Mode::Citations => {
                let count = self.citations_count();
                if self.citations_selected + 1 < count {
                    self.citations_selected += 1;
                }
            }
        }
    }

    pub fn on_enter(&mut self) {
        if let Some(entry) = self.results.get(self.selected) {
            self.selected_entry = Some(entry.clone());
            self.should_quit = true;
        }
    }

    pub fn on_escape(&mut self) {
        match self.mode {
            Mode::Search => self.should_quit = true,
            Mode::Chat => {
                if self.chat_streaming {
                    return;
                }
                self.mode = Mode::Search;
                self.chat_input.clear();
            }
            Mode::Citations => {
                self.mode = Mode::Chat;
                self.citations_query.clear();
                self.citations_filtered.clear();
                self.citations_selected = 0;
            }
        }
    }

    pub fn on_click(&mut self, idx: usize, visible_count: usize) {
        if self.mode != Mode::Search {
            return;
        }

        if idx >= self.results.len() {
            return;
        }

        if idx == self.selected {
            self.on_enter();
        } else {
            self.selected = idx;
            if self.selected < self.scroll_offset {
                self.scroll_offset = self.selected;
            } else if self.selected >= self.scroll_offset + visible_count {
                self.scroll_offset = self.selected - visible_count + 1;
            }
        }
    }

    pub fn start_chat(&mut self) {
        if self.chat_input.is_empty() || self.chat_streaming || self.api_key.is_none() {
            return;
        }

        self.chat_messages.push(ChatMessage {
            role: "user".to_string(),
            content: self.chat_input.clone(),
        });

        self.chat_input.clear();
        self.chat_response.clear();
        self.chat_streaming = true;
        self.chat_scroll = 0;
    }

    pub fn append_response(&mut self, text: &str) {
        if text == "\n[DONE]" {
            self.chat_streaming = false;
            self.parse_citations();
            self.chat_messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: self.chat_response.clone(),
            });
        } else {
            self.chat_response.push_str(text);
        }
    }

    pub fn cancel_streaming(&mut self) {
        if self.chat_streaming {
            self.chat_streaming = false;
            self.chat_response.push_str("\n\n[cancelled]");
        }
    }

    pub fn build_messages(&self) -> Vec<ChatMessage> {
        let mut messages = vec![ChatMessage {
            role: "system".to_string(),
            content: format!(
                r#"You are a helpful assistant. Answer questions based on the following markdown documents.

RULES:
1. Respond in plain text only, no markdown formatting (no **, no ##, no bullets)
2. When you reference information from the documents, include citations using the format [file:line]
3. Place citations inline where you use the information, like: "The installation requires cargo [README.md:20]"
4. Each line in the documents is prefixed with [file:line] for reference

DOCUMENTS:
{}"#,
                self.md_context
            ),
        }];
        messages.extend(self.chat_messages.clone());
        if !self.chat_input.is_empty() {
            messages.push(ChatMessage {
                role: "user".to_string(),
                content: self.chat_input.clone(),
            });
        }
        messages
    }

    fn update_search(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;

        if self.query.is_empty() {
            self.results.clear();
        } else {
            self.results = self.searcher.search(&self.query);
        }
    }
}
