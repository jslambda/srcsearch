# srcsearch

`srcsearch` is a lightweight search engine for source code and project documentation. It indexes Rust source files and Markdown content, then lets developers query it using Tantivy-powered full-text search with BM25 ranking.

It can be used in two ways:

1. **CLI** (`srcsearch`) for local workflows and scripting.
2. **Library** (`srcsearch`) for embedding indexing/search in your own Rust tooling.

---

## CLI usage

The crate provides a binary named `srcsearch` with these subcommands:

- `json` — build a JSON output
- `index` — build a Tantivy index directory
- `update` — incrementally update an existing Tantivy index for changed files
- `search` — query a Tantivy index

### Build and run

```bash
cargo run -- --help
```

### 1) Generate a JSON output

```bash
cargo run -- json --project-root . --output index.json
```

Short form:

```bash
cargo run -- json -p . -o index.json
```

### 2) Build a Tantivy index directory

```bash
cargo run -- index --project-root . --output-dir index
```

Short form:

```bash
cargo run -- index -p . -o index
```

> `--output-dir` must be empty (or not exist yet) when creating a fresh index.

### 3) Update an existing index after file changes

```bash
cargo run -- update \
  --project-root . \
  --index-dir index \
  --changed-file src/lib.rs \
  --changed-file docs/guide.md
```

Short form:

```bash
cargo run -- update -p . -i index --changed-file src/lib.rs
```

### 4) Search the index

Search all fields (default scope):

```bash
cargo run -- search --index-dir index --query quickstart
```

Restrict search to documentation-focused fields only:

```bash
cargo run -- search --index-dir index --query quickstart --scope doc
```

JSON output:

```bash
cargo run -- search --index-dir index --query quickstart --json
```

Example JSON result payload:

```json
[
  {
    "score": 4.23791,
    "record_type": "markdown",
    "file_path": "docs/guide.md",
    "title": "Quickstart",
    "name": null,
    "kind": null,
    "signature": null,
    "line_start": 1,
    "line_end": 18,
    "heading_line": 1
  },
  {
    "score": 3.91244,
    "record_type": "rust",
    "file_path": "src/lib.rs",
    "title": null,
    "name": "add_one",
    "kind": "fn",
    "signature": "pub fn add_one(value: i32) -> i32",
    "line_start": 3,
    "line_end": 5,
    "heading_line": null
  },
  {
    "score": 3.10582,
    "record_type": "rust",
    "file_path": "src/lib.rs",
    "title": null,
    "name": "Widget",
    "kind": "struct",
    "signature": "pub struct Widget",
    "line_start": 8,
    "line_end": 12,
    "heading_line": null
  },
  {
    "score": 2.84467,
    "record_type": "rust",
    "file_path": "src/lib.rs",
    "title": null,
    "name": "Widget::new",
    "kind": "impl",
    "signature": "pub fn new() -> Self",
    "line_start": 14,
    "line_end": 16,
    "heading_line": null
  }
]
```

#### Search scopes

- `all` (default): query title/body text + Rust symbol/signature/doc/code fields
- `doc`: query title/body text + Rust doc fields only (ignores signatures/code)

Notes:

- Queries run against `title`, `body_text`, and Rust `doc` fields use stemming, so inflected forms (for example `running` vs `run`) may match.

### Convenience scripts

The repository includes a few helper scripts under `scripts/` for common local workflows. They all assume the index directory is `.srcsearch` at the project root.

- `scripts/srcindex` — create a fresh `.srcsearch` index from the current project.
- `scripts/srcreindex` — remove `.srcsearch` and rebuild it from scratch.
- `scripts/srcquery "<query>"` — run a regular search (`--scope all`) against `.srcsearch`.
- `scripts/srcdoc "<query>"` — run a docs-focused search (`--scope doc`) against `.srcsearch`.

Typical usage:

```bash
scripts/srcindex
scripts/srcquery "how does indexing work"
scripts/srcdoc "search scope"
```

---

## Library usage

Add `srcsearch` from crates.io:

```toml
[dependencies]
srcsearch = "0.1"
```

If you are working from a local checkout instead, you can use a path dependency:

```toml
srcsearch = { path = "../srcsearch" }
```

### Build records from a project (or a single target)

```rust
use std::path::Path;
use srcsearch::{index_project, index_target};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let records = index_project(Path::new("."))?;
    println!("indexed {} records", records.len());

    // You can also index just one file or one directory:
    let changed = index_target(Path::new("src/lib.rs"), Path::new("."))?;
    println!("indexed {} changed-records", changed.len());
    Ok(())
}
```

### Write JSON or Tantivy index

```rust
use std::path::Path;
use srcsearch::{index_project, write_json, write_tantivy_index};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(".");
    let records = index_project(root)?;

    write_json(&records, Path::new("index.json"))?;
    write_tantivy_index(&records, Path::new("index"), Some(root))?;
    Ok(())
}
```

### Incremental update

```rust
use std::path::Path;
use srcsearch::{index_target, update_tantivy_index};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(".");
    let changed_files = vec!["src/lib.rs".to_string()];

    let mut changed_records = Vec::new();
    for file in &changed_files {
        let path = root.join(file);
        let mut file_records = index_target(&path, root)?;
        changed_records.append(&mut file_records);
    }

    update_tantivy_index(&changed_records, Path::new("index"), Some(root), &changed_files)?;
    Ok(())
}
```

### Search from code

```rust
use std::path::Path;
use srcsearch::{search_tantivy_index, SearchScope};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hits = search_tantivy_index(Path::new("index"), "quickstart", 10, SearchScope::Doc)?;

    for hit in hits {
        println!("{} {} {:?}", hit.record_type, hit.file_path, hit.line_start);
    }

    Ok(())
}
```

---

## Development

```bash
cargo test
cargo fmt
```
