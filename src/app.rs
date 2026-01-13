use crate::search::{SearchEntry, Searcher};
use std::path::PathBuf;

pub struct App {
    pub query: String,
    pub results: Vec<SearchEntry>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub should_quit: bool,
    pub selected_entry: Option<SearchEntry>,
    pub cwd: PathBuf,
    pub entry_count: usize,
    searcher: Searcher,
}

impl App {
    pub fn new(cwd: PathBuf) -> Self {
        let searcher = Searcher::new(&cwd);
        let entry_count = searcher.entry_count();

        Self {
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            should_quit: false,
            selected_entry: None,
            cwd,
            entry_count,
            searcher,
        }
    }

    pub fn on_char(&mut self, c: char) {
        self.query.push(c);
        self.update_search();
    }

    pub fn on_backspace(&mut self) {
        self.query.pop();
        self.update_search();
    }

    pub fn on_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            if self.selected < self.scroll_offset {
                self.scroll_offset = self.selected;
            }
        }
    }

    pub fn on_down(&mut self, visible_count: usize) {
        if self.selected + 1 < self.results.len() {
            self.selected += 1;
            if self.selected >= self.scroll_offset + visible_count {
                self.scroll_offset = self.selected - visible_count + 1;
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
        self.should_quit = true;
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
