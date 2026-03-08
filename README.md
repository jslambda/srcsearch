# srcsearch

`srcsearch` indexes Rust source and Markdown documentation, then lets you query the result with Tantivy-based full-text search.

It can be used in two ways:

1. **CLI** (`srcsearch`) for local workflows and scripting.
2. **Library** (`srcsearch`) for embedding indexing/search in your own Rust tooling.

## What gets indexed

- `*.rs` files (symbols, signatures, docs, and optional code snippets)
- `*.md` files (section titles and body text)
- Directory traversal skips common generated folders: `target`, `.git`, and `node_modules`.

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

#### Search scopes

- `all` (default): query title/body text + Rust symbol/signature/doc/code fields
- `doc`: query title/body text + Rust doc fields only (ignores signatures/code)

---

## Library usage

Add `srcsearch` to your project (path dependency for local checkout shown):

```toml
[dependencies]
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
