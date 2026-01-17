use crate::search::LoadedFile;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Schema, TEXT, STORED, STRING, Value};
use tantivy::{doc, Index, IndexWriter, ReloadPolicy};

pub struct RagIndex {
    index: Index,
    file_field: tantivy::schema::Field,
    line_field: tantivy::schema::Field,
    content_field: tantivy::schema::Field,
}

impl RagIndex {
    pub fn new(files: &[LoadedFile]) -> Self {
        let mut schema_builder = Schema::builder();
        let file_field = schema_builder.add_text_field("file", STRING | STORED);
        let line_field = schema_builder.add_text_field("line", STRING | STORED);
        let content_field = schema_builder.add_text_field("content", TEXT | STORED);
        let schema = schema_builder.build();

        let index = Index::create_in_ram(schema.clone());
        let mut index_writer: IndexWriter = index.writer(15_000_000).unwrap();

        for file in files {
            for (i, line) in file.content.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                index_writer.add_document(doc!(
                    file_field => file.name.clone(),
                    line_field => (i + 1).to_string(),
                    content_field => trimmed.to_string()
                )).unwrap();
            }
        }
        index_writer.commit().unwrap();

        Self {
            index,
            file_field,
            line_field,
            content_field,
        }
    }

    pub fn search(&self, query: &str, limit: usize) -> String {
        let reader = self.index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .unwrap();
        let searcher = reader.searcher();

        let query_parser = QueryParser::for_index(&self.index, vec![self.content_field]);

        let parsed_query = match query_parser.parse_query(query) {
            Ok(q) => q,
            Err(_) => return String::new(),
        };

        let top_docs = match searcher.search(&parsed_query, &TopDocs::with_limit(limit)) {
            Ok(docs) => docs,
            Err(_) => return String::new(),
        };

        let mut context = String::new();
        for (_score, doc_address) in top_docs {
            if let Ok(doc) = searcher.doc::<tantivy::TantivyDocument>(doc_address) {
                let file = doc.get_first(self.file_field)
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let line = doc.get_first(self.line_field)
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let content = doc.get_first(self.content_field)
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                context.push_str(&format!("[{}:{}] {}\n", file, line, content));
            }
        }

        context
    }
}
