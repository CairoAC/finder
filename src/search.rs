use ignore::WalkBuilder;
use nucleo::{Config, Nucleo, Utf32String};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct SearchEntry {
    pub file: String,
    pub line_num: usize,
    pub content: String,
}

pub struct Searcher {
    entries: Vec<SearchEntry>,
    nucleo: Nucleo<u32>,
}

impl Searcher {
    pub fn new(dir: &Path) -> Self {
        let entries = Self::load_entries(dir);
        let config = Config::DEFAULT.match_paths();
        let nucleo: Nucleo<u32> = Nucleo::new(config, Arc::new(|| {}), None, 1);

        let injector = nucleo.injector();
        for (idx, entry) in entries.iter().enumerate() {
            let haystack = format!("{} {}", entry.file, entry.content);
            injector.push(idx as u32, |_, cols| {
                cols[0] = Utf32String::from(haystack.as_str());
            });
        }

        Self { entries, nucleo }
    }

    fn load_entries(dir: &Path) -> Vec<SearchEntry> {
        let mut entries = Vec::new();

        let walker = WalkBuilder::new(dir)
            .hidden(false)
            .git_ignore(true)
            .build();

        for result in walker {
            let Ok(entry) = result else { continue };
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let Some(ext) = path.extension() else { continue };
            if ext != "md" {
                continue;
            }

            let Ok(content) = std::fs::read_to_string(path) else { continue };

            let file_name = path
                .strip_prefix(dir)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            for (line_idx, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                entries.push(SearchEntry {
                    file: file_name.clone(),
                    line_num: line_idx + 1,
                    content: trimmed.to_string(),
                });
            }
        }

        entries
    }

    pub fn search(&mut self, query: &str) -> Vec<SearchEntry> {
        self.nucleo.pattern.reparse(
            0,
            query,
            nucleo::pattern::CaseMatching::Ignore,
            nucleo::pattern::Normalization::Smart,
            false,
        );

        self.nucleo.tick(100);

        let snapshot = self.nucleo.snapshot();
        let mut results = Vec::new();

        for item in snapshot.matched_items(..snapshot.matched_item_count().min(100)) {
            let idx = *item.data as usize;
            if idx < self.entries.len() {
                results.push(self.entries[idx].clone());
            }
        }

        results
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}
