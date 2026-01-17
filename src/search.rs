use ignore::WalkBuilder;
use nucleo::{Config, Nucleo, Utf32String};
use nucleo_matcher::{Matcher, pattern::Pattern, pattern::CaseMatching, pattern::Normalization};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct LoadedFile {
    pub name: String,
    pub content: String,
}

#[derive(Clone)]
pub struct SearchEntry {
    pub file: String,
    pub line_num: usize,
    pub content: String,
    pub match_indices: Vec<u32>,
}

pub fn load_md_files(dir: &Path) -> Vec<LoadedFile> {
    let mut files = Vec::new();

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

        let name = path
            .strip_prefix(dir)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        files.push(LoadedFile { name, content });
    }

    files
}

pub fn build_context(files: &[LoadedFile]) -> String {
    let mut context = String::new();
    for file in files {
        context.push_str(&format!("\n--- {} ---\n", file.name));
        for (i, line) in file.content.lines().enumerate() {
            context.push_str(&format!("[{}:{}] {}\n", file.name, i + 1, line));
        }
    }
    context
}

pub struct Searcher {
    entries: Vec<SearchEntry>,
    nucleo: Nucleo<u32>,
}

impl Searcher {
    pub fn from_files(files: &[LoadedFile]) -> Self {
        let entries = Self::build_entries(files);
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

    fn build_entries(files: &[LoadedFile]) -> Vec<SearchEntry> {
        let mut entries = Vec::new();

        for file in files {
            for (line_idx, line) in file.content.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                entries.push(SearchEntry {
                    file: file.name.clone(),
                    line_num: line_idx + 1,
                    content: trimmed.to_string(),
                    match_indices: Vec::new(),
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
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

        for item in snapshot.matched_items(..snapshot.matched_item_count().min(100)) {
            let idx = *item.data as usize;
            if idx < self.entries.len() {
                let mut entry = self.entries[idx].clone();
                let mut indices = Vec::new();
                let mut buf = Vec::new();
                let haystack = nucleo_matcher::Utf32Str::new(&entry.content, &mut buf);
                pattern.indices(haystack, &mut matcher, &mut indices);
                entry.match_indices = indices;
                results.push(entry);
            }
        }

        results
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}
