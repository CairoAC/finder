use crate::chat::ChatMessage;
use crate::rag::{RagChunk, RagIndex};
use crate::search::{build_context, load_md_files, LoadedFile, SearchEntry, Searcher};
use ignore::WalkBuilder;
use nucleo_matcher::{pattern::{CaseMatching, Normalization, Pattern}, Matcher, Utf32Str};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Search,
    Chat,
    Citations,
    DirectoryPicker,
    QuickAnswer,
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
    loaded_files: Vec<LoadedFile>,
    rag_index: RagIndex,
    pub dir_entries: Vec<PathBuf>,
    pub dir_filtered: Vec<PathBuf>,
    pub dir_query: String,
    pub dir_selected: usize,
    pub dir_scroll: usize,
    pub original_cwd: PathBuf,
    pub quick_query: String,
    pub quick_response: String,
    pub quick_streaming: bool,
    pub quick_sources: Vec<RagChunk>,
    pub quick_sources_expanded: bool,
    pub quick_sources_selected: usize,
}

impl App {
    pub fn new(cwd: PathBuf) -> Self {
        let loaded_files = load_md_files(&cwd);
        let searcher = Searcher::from_files(&loaded_files);
        let entry_count = searcher.entry_count();
        let md_context = build_context(&loaded_files);
        let rag_index = RagIndex::new(&loaded_files, &cwd);
        let api_key = crate::chat::find_api_key();
        let original_cwd = cwd.clone();

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
            loaded_files,
            rag_index,
            dir_entries: Vec::new(),
            dir_filtered: Vec::new(),
            dir_query: String::new(),
            dir_selected: 0,
            dir_scroll: 0,
            original_cwd,
            quick_query: String::new(),
            quick_response: String::new(),
            quick_streaming: false,
            quick_sources: Vec::new(),
            quick_sources_expanded: false,
            quick_sources_selected: 0,
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
                } else if c == '@' && self.query.is_empty() {
                    self.mode = Mode::QuickAnswer;
                    self.quick_query.clear();
                    self.quick_response.clear();
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
            Mode::DirectoryPicker => {
                self.dir_query.push(c);
                self.filter_directories();
            }
            Mode::QuickAnswer => {
                if !self.quick_streaming {
                    self.quick_query.push(c);
                }
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
            Mode::DirectoryPicker => {
                self.dir_query.pop();
                self.filter_directories();
            }
            Mode::QuickAnswer => {
                if !self.quick_streaming {
                    if self.quick_query.is_empty() {
                        self.mode = Mode::Search;
                    } else {
                        self.quick_query.pop();
                    }
                }
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
            Mode::DirectoryPicker => {
                if self.dir_selected > 0 {
                    self.dir_selected -= 1;
                    if self.dir_selected < self.dir_scroll {
                        self.dir_scroll = self.dir_selected;
                    }
                }
            }
            Mode::QuickAnswer => {}
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
            Mode::DirectoryPicker => {
                let count = self.dir_list().len();
                if self.dir_selected + 1 < count {
                    self.dir_selected += 1;
                    if self.dir_selected >= self.dir_scroll + visible_count {
                        self.dir_scroll = self.dir_selected - visible_count + 1;
                    }
                }
            }
            Mode::QuickAnswer => {}
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
            Mode::DirectoryPicker => {
                self.mode = Mode::Search;
                self.dir_query.clear();
                self.dir_filtered.clear();
                self.dir_selected = 0;
                self.dir_scroll = 0;
            }
            Mode::QuickAnswer => {
                if self.quick_streaming {
                    return;
                }
                self.mode = Mode::Search;
                self.quick_query.clear();
                self.quick_response.clear();
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

FORMATTING RULES:
1. Use markdown formatting for better readability:
   - Use **bold** for important terms and emphasis
   - Use *italic* for technical terms or names
   - Use ## headers to organize sections (only H2 and H3)
   - Use bullet lists (- item) for multiple items
   - Use numbered lists (1. 2. 3.) for sequential steps
   - Use `code` for inline code, commands, or file names
   - Use code blocks with ``` for multi-line code
2. Keep responses concise and well-structured
3. When referencing the documents, include citations using [file:line] format
4. Place citations inline: "The installation requires cargo [README.md:20]"

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

    pub fn enter_directory_picker(&mut self) {
        self.dir_entries = self.scan_directories();
        self.dir_filtered.clear();
        self.dir_query.clear();
        self.dir_selected = 0;
        self.dir_scroll = 0;
        self.mode = Mode::DirectoryPicker;
    }

    fn scan_directories(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        // Add parent directories (up to 3 levels) with their actual names
        // e.g., "../www", "../../jow", "../../../Users"
        let mut ancestor = self.original_cwd.clone();
        for i in 1..=3 {
            if let Some(parent) = ancestor.parent() {
                if let Some(name) = parent.file_name() {
                    let prefix = "../".repeat(i);
                    dirs.push(PathBuf::from(format!("{}{}", prefix, name.to_string_lossy())));
                }
                ancestor = parent.to_path_buf();
            } else {
                break;
            }
        }

        // Add subdirectories (5 levels deep)
        let walker = WalkBuilder::new(&self.original_cwd)
            .hidden(true)
            .git_ignore(true)
            .max_depth(Some(5))
            .build();

        for result in walker {
            let Ok(entry) = result else { continue };
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            if let Ok(rel) = path.strip_prefix(&self.original_cwd) {
                if !rel.as_os_str().is_empty() {
                    dirs.push(rel.to_path_buf());
                }
            }
        }

        dirs.sort();
        dirs
    }

    pub fn filter_directories(&mut self) {
        if self.dir_query.is_empty() {
            self.dir_filtered.clear();
            self.dir_selected = 0;
            self.dir_scroll = 0;
            return;
        }

        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let pattern = Pattern::parse(&self.dir_query, CaseMatching::Ignore, Normalization::Smart);

        let mut scored: Vec<(i64, PathBuf)> = self
            .dir_entries
            .iter()
            .filter_map(|p| {
                let s = p.to_string_lossy();
                let mut buf = Vec::new();
                let haystack = Utf32Str::new(&s, &mut buf);
                pattern.score(haystack, &mut matcher).map(|score| (score as i64, p.clone()))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        self.dir_filtered = scored.into_iter().map(|(_, p)| p).collect();
        self.dir_selected = 0;
        self.dir_scroll = 0;
    }

    pub fn dir_list(&self) -> &[PathBuf] {
        if self.dir_query.is_empty() {
            &self.dir_entries
        } else {
            &self.dir_filtered
        }
    }

    pub fn select_directory(&mut self) {
        let list = self.dir_list();
        if let Some(selected) = list.get(self.dir_selected) {
            let new_cwd = self.original_cwd.join(selected);
            if let Ok(canonical) = new_cwd.canonicalize() {
                self.cwd = canonical.clone();
                self.original_cwd = canonical;
                self.loaded_files = load_md_files(&self.cwd);
                self.searcher = Searcher::from_files(&self.loaded_files);
                self.entry_count = self.searcher.entry_count();
                self.md_context = build_context(&self.loaded_files);
                self.rag_index = RagIndex::new(&self.loaded_files, &self.cwd);
                self.query.clear();
                self.results.clear();
                self.selected = 0;
                self.scroll_offset = 0;
            }
        }
        self.mode = Mode::Search;
    }

    pub fn start_quick_answer(&mut self) {
        if self.quick_query.is_empty() || self.quick_streaming || self.api_key.is_none() {
            return;
        }
        self.quick_response.clear();
        self.quick_streaming = true;
    }

    pub fn append_quick_response(&mut self, text: &str) {
        if text == "\n[DONE]" {
            self.quick_streaming = false;
        } else {
            self.quick_response.push_str(text);
        }
    }

    pub fn cancel_quick(&mut self) {
        if self.quick_streaming {
            self.quick_streaming = false;
            self.quick_response.push_str("\n\n[cancelled]");
        }
    }

    pub fn rebuild_rag_index(&mut self) {
        if let Some(cache_dir) = dirs::cache_dir() {
            let _ = std::fs::remove_dir_all(cache_dir.join("finder"));
        }
        self.loaded_files = load_md_files(&self.cwd);
        self.rag_index = RagIndex::new(&self.loaded_files, &self.cwd);
        self.quick_sources.clear();
    }

    pub fn prepare_quick_search(&mut self) {
        self.quick_sources = self.rag_index.search_chunks(&self.quick_query, 20);
        self.quick_sources_selected = 0;
    }

    pub fn build_quick_messages(&self) -> Vec<ChatMessage> {
        let relevant_context: String = self.quick_sources.iter()
            .map(|c| format!("[{}:{}] {}\n\n", c.file, c.line, c.content))
            .collect();
        vec![
            ChatMessage {
                role: "system".to_string(),
                content: format!(
                    r#"You are a technical assistant. Give a complete but speakable answer.

Rules:
- 4-6 sentences covering the key points
- Use simple language that can be read aloud in a meeting
- Include specific details (names, values, differences) from the context
- No greetings, no markdown formatting, no bullet points
- Write in a natural speaking flow

RELEVANT CONTEXT:
{}"#,
                    relevant_context
                ),
            },
            ChatMessage {
                role: "user".to_string(),
                content: self.quick_query.clone(),
            },
        ]
    }

    pub fn toggle_quick_sources(&mut self) {
        self.quick_sources_expanded = !self.quick_sources_expanded;
    }

    pub fn quick_sources_up(&mut self) {
        if self.quick_sources_selected > 0 {
            self.quick_sources_selected -= 1;
        }
    }

    pub fn quick_sources_down(&mut self) {
        if self.quick_sources_selected + 1 < self.quick_sources.len() {
            self.quick_sources_selected += 1;
        }
    }

    pub fn open_quick_source(&mut self) {
        if let Some(chunk) = self.quick_sources.get(self.quick_sources_selected) {
            let file_path = self.cwd.join(&chunk.file);
            let _ = std::process::Command::new("nvim")
                .arg(format!("+{}", chunk.line))
                .arg(&file_path)
                .status();
        }
    }
}
