use std::path::Path;

use srcsearch::{SearchRecord, SearchScope, index_project, index_target, search_tantivy_index};
use tantivy::schema::Value;

#[test]
fn indexes_markdown_and_rust() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let project_root = Path::new("tests/fixtures/simple_project");
    let records = index_project(project_root)?;

    let mut saw_markdown = false;
    let mut saw_rust = false;

    for record in &records {
        match record {
            SearchRecord::MarkdownSection {
                file_path,
                section: _,
            } => {
                if file_path == "docs/guide.md" {
                    saw_markdown = true;
                }
            }
            SearchRecord::RustIndexEntry(entry) => {
                if entry.file == "src/lib.rs" {
                    saw_rust = true;
                }
            }
        }
    }

    assert!(saw_markdown, "expected markdown section records");
    assert!(saw_rust, "expected rust index records");

    let json = serde_json::to_string(&records)?;
    let parsed: serde_json::Value = serde_json::from_str(&json)?;
    assert!(parsed.is_array());

    Ok(())
}

#[test]
fn indexes_rust_code_between_line_bounds() -> std::result::Result<(), Box<dyn std::error::Error>> {
    use srcsearch::write_tantivy_index;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tantivy::Index;

    let project_root = Path::new("tests/fixtures/simple_project");
    let records = index_project(project_root)?;

    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let output_dir = std::env::temp_dir().join(format!("rustearch-index-tests-{unique}"));

    write_tantivy_index(&records, &output_dir, Some(project_root))?;

    let index = Index::open_in_dir(&output_dir)?;
    let schema = index.schema();
    let code_field = schema
        .get_field("code")
        .expect("code field should exist in schema");
    let record_type_field = schema
        .get_field("record_type")
        .expect("record_type field should exist");

    let reader = index.reader()?;
    let searcher = reader.searcher();
    let docs = searcher.search(
        &tantivy::query::AllQuery,
        &tantivy::collector::TopDocs::with_limit(20),
    )?;

    let mut saw_snippet = false;
    for (_score, doc_address) in docs {
        let doc: tantivy::TantivyDocument = searcher.doc(doc_address)?;
        let is_rust = doc
            .get_first(record_type_field)
            .and_then(|value| value.as_str())
            .is_some_and(|value| value == "rust");

        if !is_rust {
            continue;
        }

        let code = doc
            .get_first(code_field)
            .and_then(|value| value.as_str())
            .unwrap_or_default();

        if code.contains("pub fn add_one(value: i32) -> i32 {") {
            saw_snippet = true;
            assert!(code.contains("value + 1"));
            assert!(!code.contains("pub struct Widget"));
        }
    }

    drop(searcher);
    drop(reader);
    drop(index);

    fs::remove_dir_all(&output_dir)?;
    assert!(
        saw_snippet,
        "expected rust document with indexed code snippet"
    );

    Ok(())
}

#[test]
fn doc_scope_search_ignores_code_and_signature_matches()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    use srcsearch::write_tantivy_index;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let project_root = Path::new("tests/fixtures/simple_project");
    let records = index_project(project_root)?;

    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let output_dir = std::env::temp_dir().join(format!("rustearch-doc-scope-tests-{unique}"));

    write_tantivy_index(&records, &output_dir, Some(project_root))?;

    let all_hits = search_tantivy_index(&output_dir, "i32", 10, SearchScope::All)?;
    assert!(
        !all_hits.is_empty(),
        "expected a code/signature hit in all scope"
    );

    let doc_hits = search_tantivy_index(&output_dir, "i32", 10, SearchScope::Doc)?;
    assert!(
        doc_hits.is_empty(),
        "did not expect code/signature-only query to match in doc scope"
    );

    fs::remove_dir_all(&output_dir)?;
    Ok(())
}

#[test]
fn index_target_supports_single_file() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let markdown_file = Path::new("tests/fixtures/simple_project/docs/guide.md");

    let records = index_target(markdown_file, Path::new("tests/fixtures/simple_project"))?;

    assert!(
        records
            .iter()
            .all(|record| matches!(record, SearchRecord::MarkdownSection { .. })),
        "only markdown records should be returned when indexing markdown file"
    );
    assert!(records.iter().any(|record| matches!(
        record,
        SearchRecord::MarkdownSection {
            file_path,
            section: _
        } if file_path.ends_with("guide.md")
    )));

    Ok(())
}

#[test]
fn index_target_directory_equals_sum_of_supported_files()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let fixture_dir = Path::new("tests/fixtures/simple_project");
    let dir_records = index_target(fixture_dir, fixture_dir)?;

    let mut composed_records = Vec::new();
    composed_records.extend(index_target(
        Path::new("tests/fixtures/simple_project/src/lib.rs"),
        fixture_dir,
    )?);
    composed_records.extend(index_target(
        Path::new("tests/fixtures/simple_project/docs/guide.md"),
        fixture_dir,
    )?);

    let mut dir_serialized: Vec<String> = dir_records
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<_, _>>()?;
    dir_serialized.sort();

    let mut composed_serialized: Vec<String> = composed_records
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<_, _>>()?;
    composed_serialized.sort();

    assert_eq!(dir_serialized, composed_serialized);

    Ok(())
}

#[test]
fn index_target_uses_explicit_project_root_for_relative_paths()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let file = Path::new("tests/fixtures/simple_project/src/lib.rs");
    let root = Path::new("tests/fixtures");

    let records = index_target(file, root)?;

    assert!(records.iter().any(|record| matches!(
        record,
        SearchRecord::RustIndexEntry(entry) if entry.file == "simple_project/src/lib.rs"
    )));

    Ok(())
}

#[test]
fn index_target_directory_skips_ignored_directories()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!("rustearch-ignored-dirs-{unique}"));

    fs::create_dir_all(root.join("src"))?;
    fs::create_dir_all(root.join("target"))?;
    fs::create_dir_all(root.join(".git"))?;
    fs::create_dir_all(root.join("node_modules"))?;

    fs::write(root.join("src/keep.rs"), "pub fn keep() {}")?;
    fs::write(root.join("target/skip.rs"), "pub fn skip_target() {}")?;
    fs::write(root.join(".git/skip.md"), "# skip git")?;
    fs::write(root.join("node_modules/skip.md"), "# skip node_modules")?;

    let records = index_target(&root, &root)?;

    assert!(records.iter().any(|record| matches!(
        record,
        SearchRecord::RustIndexEntry(entry) if entry.file == "src/keep.rs"
    )));
    assert!(!records.iter().any(|record| matches!(
        record,
        SearchRecord::RustIndexEntry(entry) if entry.file.contains("target/")
    )));
    assert!(!records.iter().any(|record| matches!(
        record,
        SearchRecord::MarkdownSection { file_path, .. } if file_path.contains(".git/")
    )));
    assert!(!records.iter().any(|record| matches!(
        record,
        SearchRecord::MarkdownSection { file_path, .. } if file_path.contains("node_modules/")
    )));

    fs::remove_dir_all(&root)?;
    Ok(())
}
