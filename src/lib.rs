use markdown2json::{CodeBlock, Section, index_markdown};
use rust2json::{IndexEntry, build_file_index};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tantivy::schema::{Field, INDEXED, STORED, STRING, Schema, TEXT, Value};
use tantivy::{Index, Score, TantivyDocument, Term, doc};
use tantivy::{collector::TopDocs, query::QueryParser};
use walkdir::{DirEntry, WalkDir};

pub type AppResult<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Debug)]
pub enum SearchRecord {
    MarkdownSection { file_path: String, section: Section },
    RustIndexEntry(IndexEntry),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchHit {
    pub score: Score,
    pub record_type: String,
    pub file_path: String,
    pub title: Option<String>,
    pub name: Option<String>,
    pub kind: Option<String>,
    pub signature: Option<String>,
    pub line_start: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchScope {
    All,
    Doc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SupportedFileKind {
    Rust,
    Markdown,
}

impl Clone for SearchRecord {
    fn clone(&self) -> Self {
        match self {
            SearchRecord::MarkdownSection { file_path, section } => SearchRecord::MarkdownSection {
                file_path: file_path.clone(),
                section: section.clone(),
            },
            SearchRecord::RustIndexEntry(entry) => {
                SearchRecord::RustIndexEntry(clone_index_entry(entry))
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum SearchRecordDef {
    MarkdownSection {
        file_path: String,
        section: SectionDef,
    },
    RustIndexEntry(IndexEntryDef),
}

impl Serialize for SearchRecord {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        SearchRecordDef::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SearchRecord {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let def = SearchRecordDef::deserialize(deserializer)?;
        Ok(SearchRecord::from(def))
    }
}

impl From<&SearchRecord> for SearchRecordDef {
    fn from(record: &SearchRecord) -> Self {
        match record {
            SearchRecord::MarkdownSection { file_path, section } => {
                SearchRecordDef::MarkdownSection {
                    file_path: file_path.clone(),
                    section: SectionDef::from(section.clone()),
                }
            }
            SearchRecord::RustIndexEntry(entry) => {
                SearchRecordDef::RustIndexEntry(IndexEntryDef::from(entry))
            }
        }
    }
}

impl From<SearchRecordDef> for SearchRecord {
    fn from(def: SearchRecordDef) -> Self {
        match def {
            SearchRecordDef::MarkdownSection { file_path, section } => {
                SearchRecord::MarkdownSection {
                    file_path,
                    section: Section::from(section),
                }
            }
            SearchRecordDef::RustIndexEntry(entry) => {
                SearchRecord::RustIndexEntry(IndexEntry::from(entry))
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CodeBlockDef {
    lang: Option<String>,
    meta: Option<String>,
    value: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SectionDef {
    title: String,
    level: u8,
    body_text: Vec<String>,
    code_blocks: Vec<CodeBlockDef>,
}

impl From<CodeBlock> for CodeBlockDef {
    fn from(block: CodeBlock) -> Self {
        Self {
            lang: block.lang,
            meta: block.meta,
            value: block.value,
        }
    }
}

impl From<CodeBlockDef> for CodeBlock {
    fn from(block: CodeBlockDef) -> Self {
        Self {
            lang: block.lang,
            meta: block.meta,
            value: block.value,
        }
    }
}

impl From<Section> for SectionDef {
    fn from(section: Section) -> Self {
        Self {
            title: section.title,
            level: section.level,
            body_text: section.body_text,
            code_blocks: section
                .code_blocks
                .into_iter()
                .map(CodeBlockDef::from)
                .collect(),
        }
    }
}

impl From<SectionDef> for Section {
    fn from(section: SectionDef) -> Self {
        Self {
            title: section.title,
            level: section.level,
            body_text: section.body_text,
            code_blocks: section
                .code_blocks
                .into_iter()
                .map(CodeBlock::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct IndexEntryDef {
    kind: String,
    name: String,
    file: String,
    line_start: u32,
    line_end: u32,
    signature: String,
    doc_summary: Option<String>,
    doc: Option<String>,
}

impl From<&IndexEntry> for IndexEntryDef {
    fn from(entry: &IndexEntry) -> Self {
        Self {
            kind: entry.kind.clone(),
            name: entry.name.clone(),
            file: entry.file.clone(),
            line_start: entry.line_start,
            line_end: entry.line_end,
            signature: entry.signature.clone(),
            doc_summary: entry.doc_summary.clone(),
            doc: entry.doc.clone(),
        }
    }
}

impl From<IndexEntryDef> for IndexEntry {
    fn from(entry: IndexEntryDef) -> Self {
        Self {
            kind: entry.kind,
            name: entry.name,
            file: entry.file,
            line_start: entry.line_start,
            line_end: entry.line_end,
            signature: entry.signature,
            doc_summary: entry.doc_summary,
            doc: entry.doc,
        }
    }
}

fn clone_index_entry(entry: &IndexEntry) -> IndexEntry {
    IndexEntry {
        kind: entry.kind.clone(),
        name: entry.name.clone(),
        file: entry.file.clone(),
        line_start: entry.line_start,
        line_end: entry.line_end,
        signature: entry.signature.clone(),
        doc_summary: entry.doc_summary.clone(),
        doc: entry.doc.clone(),
    }
}

pub fn collect_files(project_root: &Path) -> AppResult<(Vec<PathBuf>, Vec<PathBuf>)> {
    collect_supported_files(project_root)
}

fn collect_supported_files(target_dir: &Path) -> AppResult<(Vec<PathBuf>, Vec<PathBuf>)> {
    let mut rust_files = Vec::new();
    let mut markdown_files = Vec::new();

    for entry in WalkDir::new(target_dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| should_walk(entry))
    {
        let entry = entry.map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        if !entry.file_type().is_file() {
            continue;
        }

        match classify_supported_file(entry.path()) {
            Some(SupportedFileKind::Rust) => rust_files.push(entry.path().to_path_buf()),
            Some(SupportedFileKind::Markdown) => markdown_files.push(entry.path().to_path_buf()),
            None => {}
        }
    }

    Ok((rust_files, markdown_files))
}

fn classify_supported_file(path: &Path) -> Option<SupportedFileKind> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("rs") => Some(SupportedFileKind::Rust),
        Some("md") => Some(SupportedFileKind::Markdown),
        _ => None,
    }
}

fn should_walk(entry: &DirEntry) -> bool {
    let file_name = entry.file_name().to_string_lossy();
    if entry.file_type().is_dir() {
        !matches!(file_name.as_ref(), "target" | ".git" | "node_modules")
    } else {
        true
    }
}

pub fn index_project(project_root: &Path) -> AppResult<Vec<SearchRecord>> {
    index_target(project_root, project_root)
}

pub fn index_target(target: &Path, project_root: &Path) -> AppResult<Vec<SearchRecord>> {
    let (rust_files, markdown_files) = if target.is_dir() {
        collect_supported_files(target)?
    } else if target.is_file() {
        let mut rust_files = Vec::new();
        let mut markdown_files = Vec::new();
        match classify_supported_file(target) {
            Some(SupportedFileKind::Rust) => rust_files.push(target.to_path_buf()),
            Some(SupportedFileKind::Markdown) => markdown_files.push(target.to_path_buf()),
            None => {}
        }
        (rust_files, markdown_files)
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("index target does not exist: {}", target.display()),
        )
        .into());
    };

    let mut records = Vec::new();

    for path in markdown_files {
        let src = fs::read_to_string(&path).map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("failed to read {}: {err}", path.display()),
            )
        })?;
        let sections = index_markdown(&src).map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("failed to parse markdown {}: {err}", path.display()),
            )
        })?;
        let relative = path
            .strip_prefix(project_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();

        for section in sections {
            records.push(SearchRecord::MarkdownSection {
                file_path: relative.clone(),
                section,
            });
        }
    }

    for path in rust_files {
        let relative = path.strip_prefix(project_root).unwrap_or(&path);
        let relative_str = relative.to_string_lossy().into_owned();
        let mut entries = build_file_index(&path).map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("failed to index rust file {}: {err}", path.display()),
            )
        })?;
        for entry in &mut entries {
            entry.file = relative_str.clone();
        }
        records.extend(entries.into_iter().map(SearchRecord::RustIndexEntry));
    }

    Ok(records)
}

pub fn write_json(records: &[SearchRecord], output_path: &Path) -> AppResult<()> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "failed to create output directory {}: {err}",
                    parent.display()
                ),
            )
        })?;
    }

    let tmp_path = output_path.with_extension("tmp");
    let mut tmp_file = fs::File::create(&tmp_path).map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("failed to create temp file {}: {err}", tmp_path.display()),
        )
    })?;
    serde_json::to_writer_pretty(&mut tmp_file, records).map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("failed to serialize {}: {err}", output_path.display()),
        )
    })?;
    fs::rename(&tmp_path, output_path).map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("failed to rename {}: {err}", tmp_path.display()),
        )
    })?;
    Ok(())
}

pub fn write_tantivy_index(
    records: &[SearchRecord],
    output_dir: &Path,
    project_root: Option<&Path>,
) -> AppResult<()> {
    if output_dir.exists() {
        if !output_dir.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("output path is not a directory: {}", output_dir.display()),
            )
            .into());
        }

        let mut entries = fs::read_dir(output_dir).map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "failed to read output directory {}: {err}",
                    output_dir.display()
                ),
            )
        })?;
        if entries
            .next()
            .transpose()
            .map_err(|err| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "failed to inspect output directory {}: {err}",
                        output_dir.display()
                    ),
                )
            })?
            .is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("output directory must be empty: {}", output_dir.display()),
            )
            .into());
        }
    } else {
        fs::create_dir_all(output_dir).map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "failed to create output directory {}: {err}",
                    output_dir.display()
                ),
            )
        })?;
    }

    let index = Index::create_in_dir(output_dir, build_tantivy_schema()).map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "failed to create tantivy index in {}: {err}",
                output_dir.display()
            ),
        )
    })?;
    let mut writer = index.writer(50_000_000).map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("failed to initialize tantivy index writer: {err}"),
        )
    })?;

    let schema = index.schema();
    let schema_fields = TantivySchemaFields::from_schema(&schema)?;

    for record in records {
        let document = build_tantivy_document(record, &schema_fields, project_root)?;
        writer.add_document(document).map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("failed to add document to tantivy index: {err}"),
            )
        })?;
    }

    writer.commit().map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("failed to commit tantivy index: {err}"),
        )
    })?;
    writer.wait_merging_threads().map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("failed to finalize tantivy index writer: {err}"),
        )
    })?;

    Ok(())
}

pub fn update_tantivy_index(
    records: &[SearchRecord],
    index_dir: &Path,
    project_root: Option<&Path>,
    changed_files: &[String],
) -> AppResult<()> {
    let index = if index_dir.exists() {
        if !index_dir.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("index path is not a directory: {}", index_dir.display()),
            )
            .into());
        }

        Index::open_in_dir(index_dir).map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "failed to open tantivy index in {}: {err}",
                    index_dir.display()
                ),
            )
        })?
    } else {
        fs::create_dir_all(index_dir).map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "failed to create output directory {}: {err}",
                    index_dir.display()
                ),
            )
        })?;
        Index::create_in_dir(index_dir, build_tantivy_schema()).map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "failed to create tantivy index in {}: {err}",
                    index_dir.display()
                ),
            )
        })?
    };

    let schema = index.schema();
    let schema_fields = TantivySchemaFields::from_schema(&schema)?;
    let mut writer = index.writer(50_000_000).map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("failed to initialize tantivy index writer: {err}"),
        )
    })?;

    for changed_file in changed_files {
        writer.delete_term(Term::from_field_text(schema_fields.file_path, changed_file));
    }

    for record in records {
        let document = build_tantivy_document(record, &schema_fields, project_root)?;
        writer.add_document(document).map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("failed to add document to tantivy index: {err}"),
            )
        })?;
    }

    writer.commit().map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("failed to commit tantivy index: {err}"),
        )
    })?;
    writer.wait_merging_threads().map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("failed to finalize tantivy index writer: {err}"),
        )
    })?;

    Ok(())
}

fn build_tantivy_schema() -> Schema {
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field("record_type", STRING | STORED);
    schema_builder.add_text_field("file_path", STRING | STORED);
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("name", STRING | STORED);
    schema_builder.add_text_field("kind", STRING | STORED);
    schema_builder.add_text_field("signature", TEXT | STORED);
    schema_builder.add_text_field("body_text", TEXT | STORED);
    schema_builder.add_text_field("doc", TEXT | STORED);
    schema_builder.add_text_field("code", TEXT | STORED);
    schema_builder.add_u64_field("line_start", INDEXED | STORED);
    schema_builder.add_u64_field("line_end", INDEXED | STORED);
    schema_builder.build()
}

struct TantivySchemaFields {
    record_type: Field,
    file_path: Field,
    title: Field,
    name: Field,
    kind: Field,
    signature: Field,
    body_text: Field,
    doc_field: Field,
    code_field: Field,
    line_start: Field,
    line_end: Field,
}

impl TantivySchemaFields {
    fn from_schema(schema: &Schema) -> AppResult<Self> {
        Ok(Self {
            record_type: get_tantivy_doc_field(schema, "record_type")?,
            file_path: get_tantivy_doc_field(schema, "file_path")?,
            title: get_tantivy_doc_field(schema, "title")?,
            name: get_tantivy_doc_field(schema, "name")?,
            kind: get_tantivy_doc_field(schema, "kind")?,
            signature: get_tantivy_doc_field(schema, "signature")?,
            body_text: get_tantivy_doc_field(schema, "body_text")?,
            doc_field: get_tantivy_doc_field(schema, "doc")?,
            code_field: get_tantivy_doc_field(schema, "code")?,
            line_start: get_tantivy_doc_field(schema, "line_start")?,
            line_end: get_tantivy_doc_field(schema, "line_end")?,
        })
    }
}

fn build_tantivy_document(
    record: &SearchRecord,
    schema_fields: &TantivySchemaFields,
    project_root: Option<&Path>,
) -> AppResult<TantivyDocument> {
    let document = match record {
        SearchRecord::MarkdownSection {
            file_path: source_file,
            section,
        } => doc!(
            schema_fields.record_type => "markdown",
            schema_fields.file_path => source_file.clone(),
            schema_fields.title => section.title.clone(),
            schema_fields.body_text => section.body_text.join("\n")
        ),
        SearchRecord::RustIndexEntry(entry) => {
            let mut document = doc!(
                schema_fields.record_type => "rust",
                schema_fields.file_path => entry.file.clone(),
                schema_fields.name => entry.name.clone(),
                schema_fields.kind => entry.kind.clone(),
                schema_fields.signature => entry.signature.clone(),
                schema_fields.line_start => u64::from(entry.line_start),
                schema_fields.line_end => u64::from(entry.line_end),
            );
            if let Some(doc_text) = &entry.doc {
                document.add_text(schema_fields.doc_field, doc_text);
            }
            if let Some(root) = project_root {
                if let Some(code_snippet) = extract_code_snippet(root, entry)? {
                    document.add_text(schema_fields.code_field, code_snippet);
                }
            }
            document
        }
    };

    Ok(document)
}

pub fn get_tantivy_doc_field(schema: &Schema, field_name: &str) -> AppResult<Field> {
    schema.get_field(field_name).map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("failed to read field '{field_name}' from schema: {err}"),
        )
        .into()
    })
}

pub fn search_tantivy_index(
    index_dir: &Path,
    query: &str,
    limit: i64,
    scope: SearchScope,
) -> AppResult<Vec<SearchHit>> {
    if limit <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "tantivy search limit must be at least 1",
        )
        .into());
    }

    if !index_dir.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "tantivy index directory does not exist: {}",
                index_dir.display()
            ),
        )
        .into());
    }
    if !index_dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "tantivy index path is not a directory: {}",
                index_dir.display()
            ),
        )
        .into());
    }

    let index = Index::open_in_dir(index_dir).map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "failed to open tantivy index in {}: {err}",
                index_dir.display()
            ),
        )
    })?;
    let schema = index.schema();
    let record_type = get_tantivy_doc_field(&schema, "record_type")?;
    let file_path = get_tantivy_doc_field(&schema, "file_path")?;
    let title = get_tantivy_doc_field(&schema, "title")?;
    let name = get_tantivy_doc_field(&schema, "name")?;
    let kind = get_tantivy_doc_field(&schema, "kind")?;
    let signature = get_tantivy_doc_field(&schema, "signature")?;
    let line_start = schema.get_field("line_start").ok();
    let body_text = get_tantivy_doc_field(&schema, "body_text")?;
    let doc_field = get_tantivy_doc_field(&schema, "doc")?;
    let code_field = get_tantivy_doc_field(&schema, "code")?;

    let reader = index.reader().map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("failed to create tantivy reader: {err}"),
        )
    })?;
    let searcher = reader.searcher();
    let search_fields = match scope {
        SearchScope::All => vec![title, body_text, name, signature, doc_field, code_field],
        SearchScope::Doc => vec![title, body_text, doc_field],
    };
    let query_parser = QueryParser::for_index(&index, search_fields);
    let parsed_query = query_parser.parse_query(query).map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("failed to parse query '{query}': {err}"),
        )
    })?;

    let limit = usize::try_from(limit).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "tantivy search limit is too large for this platform",
        )
    })?;

    let top_docs = searcher
        .search(&parsed_query, &TopDocs::with_limit(limit))
        .map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("failed to execute tantivy query '{query}': {err}"),
            )
        })?;

    let mut hits = Vec::with_capacity(top_docs.len());
    for (score, doc_address) in top_docs {
        let retrieved: TantivyDocument = searcher.doc(doc_address).map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("failed to read tantivy document: {err}"),
            )
        })?;

        let get_text = |field| {
            retrieved
                .get_first(field)
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        };

        hits.push(SearchHit {
            score,
            record_type: get_text(record_type).unwrap_or_default(),
            file_path: get_text(file_path).unwrap_or_default(),
            title: get_text(title),
            name: get_text(name),
            kind: get_text(kind),
            signature: get_text(signature),
            line_start: line_start
                .and_then(|field| retrieved.get_first(field).and_then(|value| value.as_u64())),
        });
    }

    Ok(hits)
}

// TODO: we call this function for every Rust symbol. Better to go through the files, and
// index the code snippets
fn extract_code_snippet(project_root: &Path, entry: &IndexEntry) -> AppResult<Option<String>> {
    let source = project_root.join(&entry.file);
    if !source.exists() {
        return Ok(None);
    }

    let src = fs::read_to_string(&source).map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("failed to read source file {}: {err}", source.display()),
        )
    })?;

    let start = entry.line_start.saturating_sub(1) as usize;
    let end = entry.line_end as usize;
    let lines: Vec<&str> = src.lines().collect();
    if start >= lines.len() || start >= end {
        return Ok(None);
    }

    let end = end.min(lines.len());
    Ok(Some(lines[start..end].join("\n")))
}

#[cfg(test)]
mod tests {
    use super::{
        SearchRecord, SearchScope, extract_code_snippet, get_tantivy_doc_field,
        search_tantivy_index, update_tantivy_index, write_tantivy_index,
    };
    use markdown2json::Section;
    use rust2json::IndexEntry;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tantivy::schema::{STORED, STRING, Schema, TEXT};
    use tantivy::{Index, doc};

    fn temp_path(test_name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("rustearch-{test_name}-{unique}"))
    }

    #[test]
    fn writes_tantivy_index_for_mixed_records() {
        let output_dir = temp_path("tantivy-index");
        let records = vec![
            SearchRecord::MarkdownSection {
                file_path: "README.md".to_string(),
                section: Section {
                    title: "Overview".to_string(),
                    level: 1,
                    body_text: vec!["search docs".to_string()],
                    code_blocks: vec![],
                },
            },
            SearchRecord::RustIndexEntry(IndexEntry {
                kind: "fn".to_string(),
                name: "write_tantivy_index".to_string(),
                file: "src/lib.rs".to_string(),
                line_start: 10,
                line_end: 20,
                signature: "pub fn write_tantivy_index(...)".to_string(),
                doc_summary: None,
                doc: Some("Indexes records".to_string()),
            }),
        ];

        write_tantivy_index(&records, &output_dir, None).expect("index write should succeed");

        let index = Index::open_in_dir(&output_dir).expect("index should be readable");
        let reader = index.reader().expect("reader should be created");
        let searcher = reader.searcher();
        assert_eq!(searcher.num_docs(), 2);

        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn update_tantivy_index_replaces_documents_for_changed_file() {
        let output_dir = temp_path("update-index-replace");
        let initial_records = vec![SearchRecord::MarkdownSection {
            file_path: "README.md".to_string(),
            section: Section {
                title: "Old title".to_string(),
                level: 1,
                body_text: vec!["old body".to_string()],
                code_blocks: vec![],
            },
        }];
        write_tantivy_index(&initial_records, &output_dir, None)
            .expect("index write should succeed");

        let updated_records = vec![SearchRecord::MarkdownSection {
            file_path: "README.md".to_string(),
            section: Section {
                title: "New title".to_string(),
                level: 1,
                body_text: vec!["new body".to_string()],
                code_blocks: vec![],
            },
        }];

        update_tantivy_index(
            &updated_records,
            &output_dir,
            None,
            &["README.md".to_string()],
        )
        .expect("index update should succeed");

        let old_hits = search_tantivy_index(&output_dir, "old", 10, SearchScope::All)
            .expect("search should succeed");
        assert!(old_hits.is_empty());

        let new_hits = search_tantivy_index(&output_dir, "new", 10, SearchScope::All)
            .expect("search should succeed");
        assert_eq!(new_hits.len(), 1);
        assert_eq!(new_hits[0].title.as_deref(), Some("New title"));

        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn update_tantivy_index_deletes_stale_documents_when_changed_file_has_no_records() {
        let output_dir = temp_path("update-index-delete-only");
        let initial_records = vec![SearchRecord::MarkdownSection {
            file_path: "README.md".to_string(),
            section: Section {
                title: "Old title".to_string(),
                level: 1,
                body_text: vec!["old body".to_string()],
                code_blocks: vec![],
            },
        }];
        write_tantivy_index(&initial_records, &output_dir, None)
            .expect("index write should succeed");

        update_tantivy_index(&[], &output_dir, None, &["README.md".to_string()])
            .expect("index update should succeed");

        let hits = search_tantivy_index(&output_dir, "old", 10, SearchScope::All)
            .expect("search should succeed");
        assert!(hits.is_empty());

        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn rejects_non_empty_output_directory() {
        let output_dir = temp_path("non-empty");
        fs::create_dir_all(&output_dir).expect("output dir should be created");
        fs::write(output_dir.join("existing.txt"), "do not overwrite")
            .expect("fixture file should be written");

        let result = write_tantivy_index(&[], &output_dir, None);

        assert!(result.is_err());
        let message = format!("{}", result.expect_err("should fail"));
        assert!(message.contains("must be empty"));

        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn extracts_code_snippet_using_line_bounds() {
        let fixture_root = Path::new("tests/fixtures/simple_project");
        let entry = IndexEntry {
            kind: "fn".to_string(),
            name: "add_one".to_string(),
            file: "src/lib.rs".to_string(),
            line_start: 4,
            line_end: 6,
            signature: "pub fn add_one(value: i32) -> i32".to_string(),
            doc_summary: None,
            doc: None,
        };

        let snippet = extract_code_snippet(fixture_root, &entry)
            .expect("snippet extraction should succeed")
            .expect("snippet should exist");

        assert!(snippet.contains("pub fn add_one"));
        assert!(!snippet.contains("#[cfg(test)]"));
    }

    #[test]
    fn searches_tantivy_index_and_returns_hit_metadata() {
        let output_dir = temp_path("search-index");
        let records = vec![
            SearchRecord::MarkdownSection {
                file_path: "README.md".to_string(),
                section: Section {
                    title: "Getting started".to_string(),
                    level: 1,
                    body_text: vec!["tantivy quickstart guide".to_string()],
                    code_blocks: vec![],
                },
            },
            SearchRecord::RustIndexEntry(IndexEntry {
                kind: "fn".to_string(),
                name: "search_tantivy_index".to_string(),
                file: "src/lib.rs".to_string(),
                line_start: 100,
                line_end: 110,
                signature: "pub fn search_tantivy_index(...)".to_string(),
                doc_summary: None,
                doc: Some("Run tantivy query and return hits".to_string()),
            }),
        ];
        write_tantivy_index(&records, &output_dir, None).expect("index write should succeed");

        let markdown_hits = search_tantivy_index(&output_dir, "quickstart", 10, SearchScope::All)
            .expect("search should succeed");

        assert_eq!(markdown_hits.len(), 1);
        assert_eq!(markdown_hits[0].record_type, "markdown");
        assert_eq!(markdown_hits[0].file_path, "README.md");
        assert_eq!(markdown_hits[0].title.as_deref(), Some("Getting started"));
        assert_eq!(markdown_hits[0].line_start, None);

        let rust_hits =
            search_tantivy_index(&output_dir, "search_tantivy_index", 10, SearchScope::All)
                .expect("search should succeed");
        assert_eq!(rust_hits.len(), 1);
        assert_eq!(rust_hits[0].record_type, "rust");
        assert_eq!(rust_hits[0].line_start, Some(100));

        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn searches_tantivy_index_with_doc_scope_excludes_code_and_signature() {
        let output_dir = temp_path("search-index-doc-scope");
        let records = vec![
            SearchRecord::MarkdownSection {
                file_path: "README.md".to_string(),
                section: Section {
                    title: "Getting started".to_string(),
                    level: 1,
                    body_text: vec!["tantivy quickstart guide".to_string()],
                    code_blocks: vec![],
                },
            },
            SearchRecord::RustIndexEntry(IndexEntry {
                kind: "fn".to_string(),
                name: "search_tantivy_index".to_string(),
                file: "src/lib.rs".to_string(),
                line_start: 100,
                line_end: 110,
                signature: "pub fn search_tantivy_index(...)".to_string(),
                doc_summary: None,
                doc: Some("Run tantivy query and return hits".to_string()),
            }),
        ];
        write_tantivy_index(&records, &output_dir, None).expect("index write should succeed");

        let code_hits =
            search_tantivy_index(&output_dir, "search_tantivy_index", 10, SearchScope::Doc)
                .expect("search should succeed");
        assert!(code_hits.is_empty());

        let docs_hits = search_tantivy_index(&output_dir, "quickstart", 10, SearchScope::Doc)
            .expect("search should succeed");
        assert_eq!(docs_hits.len(), 1);
        assert_eq!(docs_hits[0].record_type, "markdown");

        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn gets_tantivy_doc_field_from_schema() {
        let mut schema_builder = Schema::builder();
        let title = schema_builder.add_text_field("title", TEXT | STORED);
        let schema = schema_builder.build();

        let field = get_tantivy_doc_field(&schema, "title").expect("field should exist");

        assert_eq!(field, title);
    }

    #[test]
    fn get_tantivy_doc_field_returns_error_for_missing_field() {
        let schema = Schema::builder().build();

        let result = get_tantivy_doc_field(&schema, "missing");

        assert!(result.is_err());
        let message = format!("{}", result.expect_err("should fail"));
        assert!(message.contains("failed to read field 'missing'"));
    }

    #[test]
    fn search_tantivy_index_does_not_require_line_fields() {
        let output_dir = temp_path("search-index-no-line-fields");

        let mut schema_builder = Schema::builder();
        let record_type = schema_builder.add_text_field("record_type", STRING | STORED);
        let file_path = schema_builder.add_text_field("file_path", STRING | STORED);
        let title = schema_builder.add_text_field("title", TEXT | STORED);
        let name = schema_builder.add_text_field("name", STRING | STORED);
        let kind = schema_builder.add_text_field("kind", STRING | STORED);
        let signature = schema_builder.add_text_field("signature", TEXT | STORED);
        let body_text = schema_builder.add_text_field("body_text", TEXT | STORED);
        let doc_field = schema_builder.add_text_field("doc", TEXT | STORED);
        let code_field = schema_builder.add_text_field("code", TEXT | STORED);
        let schema = schema_builder.build();

        fs::create_dir_all(&output_dir).expect("output dir should be created");
        let index = Index::create_in_dir(&output_dir, schema).expect("index should be created");
        let mut writer = index.writer(15_000_000).expect("writer should be created");
        writer
            .add_document(doc!(
                record_type => "markdown",
                file_path => "README.md",
                title => "Quickstart",
                body_text => "search content",
                name => "",
                kind => "",
                signature => "",
                doc_field => "",
                code_field => "",
            ))
            .expect("document should be added");
        writer.commit().expect("index should commit");
        writer
            .wait_merging_threads()
            .expect("merge threads should finish");

        let hits = search_tantivy_index(&output_dir, "quickstart", 5, SearchScope::All)
            .expect("search should succeed");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record_type, "markdown");
        assert_eq!(hits[0].file_path, "README.md");

        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn search_tantivy_index_fails_for_missing_directory() {
        let missing_dir = temp_path("missing-search-index");

        let result = search_tantivy_index(&missing_dir, "anything", 10, SearchScope::All);

        assert!(result.is_err());
        let message = format!("{}", result.expect_err("should fail"));
        assert!(message.contains("does not exist"));
    }

    #[test]
    fn search_tantivy_index_rejects_negative_limit() {
        let output_dir = temp_path("search-index-negative-limit");
        let records = vec![SearchRecord::MarkdownSection {
            file_path: "README.md".to_string(),
            section: Section {
                title: "Getting started".to_string(),
                level: 1,
                body_text: vec!["tantivy quickstart guide".to_string()],
                code_blocks: vec![],
            },
        }];
        write_tantivy_index(&records, &output_dir, None).expect("index write should succeed");

        let result = search_tantivy_index(&output_dir, "quickstart", -1, SearchScope::All);

        assert!(result.is_err());
        let message = format!("{}", result.expect_err("negative limit should fail"));
        assert!(message.contains("at least 1"));

        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn search_tantivy_index_rejects_zero_limit() {
        let output_dir = temp_path("search-index-zero-limit");
        let records = vec![SearchRecord::MarkdownSection {
            file_path: "README.md".to_string(),
            section: Section {
                title: "Getting started".to_string(),
                level: 1,
                body_text: vec!["tantivy quickstart guide".to_string()],
                code_blocks: vec![],
            },
        }];
        write_tantivy_index(&records, &output_dir, None).expect("index write should succeed");

        let result = search_tantivy_index(&output_dir, "quickstart", 0, SearchScope::All);

        assert!(result.is_err());
        let message = format!("{}", result.expect_err("zero limit should fail"));
        assert!(message.contains("at least 1"));

        let _ = fs::remove_dir_all(&output_dir);
    }
}
