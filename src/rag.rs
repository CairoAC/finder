use crate::search::LoadedFile;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Schema, Field, TEXT, STORED, STRING, Value};
use tantivy::{doc, Index, IndexWriter, IndexSettings, ReloadPolicy, directory::MmapDirectory};

#[derive(Debug, Clone)]
pub struct RagChunk {
    pub file: String,
    pub line: usize,
    pub content: String,
    #[allow(dead_code)]
    pub score: f32,
}

pub struct RagIndex {
    index: Index,
    file_field: Field,
    line_field: Field,
    content_field: Field,
}

fn get_cache_dir(cwd: &std::path::Path) -> PathBuf {
    let hash = format!("{:x}", md5::compute(cwd.to_string_lossy().as_bytes()));
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("finder")
        .join(&hash[..16])
}

fn get_file_mtimes(files: &[LoadedFile], cwd: &std::path::Path) -> HashMap<String, u64> {
    files.iter().filter_map(|f| {
        let path = cwd.join(&f.name);
        fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| (f.name.clone(), d.as_secs()))
    }).collect()
}

fn load_cached_mtimes(cache_dir: &PathBuf) -> Option<HashMap<String, u64>> {
    let path = cache_dir.join("mtimes.json");
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_mtimes(cache_dir: &PathBuf, mtimes: &HashMap<String, u64>) {
    let path = cache_dir.join("mtimes.json");
    if let Ok(json) = serde_json::to_string(mtimes) {
        let _ = fs::write(path, json);
    }
}

fn extract_paragraphs(content: &str) -> Vec<(usize, String)> {
    let mut paragraphs = Vec::new();
    let mut current_para = String::new();
    let mut start_line = 0;

    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !current_para.is_empty() {
                paragraphs.push((start_line + 1, current_para.clone()));
                current_para.clear();
            }
        } else {
            if current_para.is_empty() {
                start_line = i;
            } else {
                current_para.push(' ');
            }
            current_para.push_str(trimmed);
        }
    }
    if !current_para.is_empty() {
        paragraphs.push((start_line + 1, current_para));
    }
    paragraphs
}

fn build_schema() -> (Schema, Field, Field, Field) {
    let mut schema_builder = Schema::builder();
    let file_field = schema_builder.add_text_field("file", STRING | STORED);
    let line_field = schema_builder.add_text_field("line", STRING | STORED);
    let content_field = schema_builder.add_text_field("content", TEXT | STORED);
    (schema_builder.build(), file_field, line_field, content_field)
}

impl RagIndex {
    pub fn new(files: &[LoadedFile], cwd: &std::path::Path) -> Self {
        let cache_dir = get_cache_dir(cwd);
        let current_mtimes = get_file_mtimes(files, cwd);
        let cached_mtimes = load_cached_mtimes(&cache_dir);

        let needs_rebuild = cached_mtimes.as_ref() != Some(&current_mtimes)
            || !cache_dir.join("meta.json").exists();

        let (schema, file_field, line_field, content_field) = build_schema();

        let index = if needs_rebuild {
            let _ = fs::remove_dir_all(&cache_dir);
            fs::create_dir_all(&cache_dir).unwrap();

            let dir = MmapDirectory::open(&cache_dir).unwrap();
            let index = Index::create(dir, schema, IndexSettings::default()).unwrap();
            let mut index_writer: IndexWriter = index.writer(15_000_000).unwrap();

            for file in files {
                for (line_num, para) in extract_paragraphs(&file.content) {
                    index_writer.add_document(doc!(
                        file_field => file.name.clone(),
                        line_field => line_num.to_string(),
                        content_field => para
                    )).unwrap();
                }
            }
            index_writer.commit().unwrap();
            save_mtimes(&cache_dir, &current_mtimes);
            index
        } else {
            let dir = MmapDirectory::open(&cache_dir).unwrap();
            Index::open(dir).unwrap()
        };

        Self { index, file_field, line_field, content_field }
    }

    pub fn search_chunks(&self, query: &str, limit: usize) -> Vec<RagChunk> {
        let reader = self.index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .unwrap();
        let searcher = reader.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![self.content_field]);

        let parsed_query = match query_parser.parse_query(query) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };

        let top_docs = match searcher.search(&parsed_query, &TopDocs::with_limit(limit)) {
            Ok(docs) => docs,
            Err(_) => return Vec::new(),
        };

        let mut chunks = Vec::new();
        for (score, doc_address) in top_docs {
            if let Ok(doc) = searcher.doc::<tantivy::TantivyDocument>(doc_address) {
                let file = doc.get_first(self.file_field).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let line = doc.get_first(self.line_field).and_then(|v| v.as_str()).unwrap_or("0").parse().unwrap_or(0);
                let content = doc.get_first(self.content_field).and_then(|v| v.as_str()).unwrap_or("").to_string();
                chunks.push(RagChunk { file, line, content, score });
            }
        }
        chunks
    }

}
