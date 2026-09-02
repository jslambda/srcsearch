# srcsearch

`srcsearch` is a lightweight search engine for source code and project documentation. It indexes Rust and Python source files plus Markdown content, then lets developers query it using Tantivy-powered full-text search with BM25 ranking.

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

Explain top scores:

```bash
cargo run -- search --index-dir index --query quickstart --json --explain
```

Search results include an `explanation_short` list for each of the top four hits. Each
entry identifies a query `term` and the indexed `field` in which it matched. This compact
explanation is included even without `--explain`. Hits after the top four have an empty
list.

`--explain` additionally asks Tantivy for its complete score explanation for those top
four hits. The flag is most useful with `--json`, where each result contains a `hit`
payload, the detailed `explanation` string (or `null` after the fourth hit), and
`explanation_short`.

Example JSON result payload:

```json
[
  {
    "hit": {
      "score": 4.23791,
      "record_type": "markdown",
      "file_path": "docs/guide.md",
      "title": "Quickstart",
      "name": null,
      "qualified_name": null,
      "kind": null,
      "signature": null,
      "line_start": 1,
      "line_end": 18,
      "heading_line": 1
    },
    "explanation": "Explanation({ ... })",
    "explanation_short": [
      {
        "term": "quickstart",
        "field": "title"
      }
    ]
  }
]
```

#### Search scopes

- `all` (default): query title/body text + source symbol/qualified-name/signature/doc/code fields
- `doc`: query title/body text + Rust doc fields only (ignores signatures/code)

Notes:

- Queries run against `title`, `body_text`, and Rust `doc` fields use stemming, so inflected forms (for example `running` vs `run`) may match.
- Source search hits include `qualified_name`; Python methods use their class-qualified form (for example `Client.fetch`) so identically named methods can be distinguished in text and JSON output.

### Convenience scripts

The repository includes a few helper scripts under `scripts/` for common local workflows. They all assume the index directory is `.srcsearch` at the project root.

- `scripts/srcindex` — create a fresh `.srcsearch` index from the current project.
- `scripts/srcreindex` — remove `.srcsearch` and rebuild it from scratch.
- `scripts/srcquery "<query>"` — run a regular search (`--scope all`) against `.srcsearch` (optionally add `--json`).
- `scripts/srcdoc "<query>"` — run a docs-focused search (`--scope doc`) against `.srcsearch` (optionally add `--json`).
- `scripts/summarize.py` — read `srcsearch search --json --explain` output from stdin and print a compact per-hit score breakdown.

Typical usage:

```bash
scripts/srcindex
scripts/srcquery "how does indexing work"
scripts/srcdoc "search scope" --json
srcsearch search -i .srcsearch -q index --json --explain | python3 scripts/summarize.py
```

`scripts/summarize.py` is intended for ranking diagnostics. It extracts each explained scoring clause and prints the query term, matched field, contributed score, percentage of the hit score, boost, unboosted base score, term frequency (`freq`), inverse document frequency (`idf`), document length (`dl`), average document length (`avgdl`), and matching document count (`n`). Rows are separated by `-----------------` between hits. A typical summary looks like:

```text
src/lib.rs:331:1: index_project (score: 8.009)
term         field          score percent  boost      base   freq      idf      dl     avgdl       n
index        signature      3.015   37.6%    2.0     1.507    1.0    1.881     9.0     5.605    12.0
index        doc            2.717   33.9%    2.0     1.358    1.0    2.267     8.0     3.037     8.0
index        code           2.277   28.4%    1.0     2.277    2.0    1.293    24.0   108.667    22.0
-----------------
src/lib.rs:542:1: update_tantivy_index (score: 7.644)
term         field          score percent  boost      base   freq      idf      dl     avgdl       n
index        signature      3.291   43.1%    2.0     1.645    2.0    1.881    17.0     5.605    12.0
index        doc            1.736   22.7%    2.0     0.868    1.0    2.267    15.0     3.037     8.0
index        code           2.617   34.2%    1.0     2.617   30.0    1.293   280.0   108.667    22.0
-----------------
src/lib.rs:657:1: register_doc_text_analyzer (score: 7.196)
term         field          score percent  boost      base   freq      idf      dl     avgdl       n
index        signature      4.834   67.2%    2.0     2.417    2.0    1.881     7.0     5.605    12.0
index        code           2.361   32.8%    1.0     2.361    3.0    1.293    38.0   108.667    22.0
-----------------
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

## Why use srcsearch alongside ripgrep/grep?

[`ripgrep`](https://github.com/BurntSushi/ripgrep) is a line-oriented search tool:
it finds lines matching a regular expression. That is the appropriate tool when
you know the text or pattern to look for and want exhaustive matches. `srcsearch`
addresses a different retrieval task: ranking likely code and documentation
entities when the query terms may occur in different parts of an entity.

`srcsearch` parses and indexes Rust entities and Markdown sections. A Rust result
can rank because a query matches information distributed across its name,
signature, documentation, and code. Markdown titles and section bodies are also
stored as distinct fields.

### Example1: Find information distributed across a Rust entity

Suppose you are exploring the ripgrep repository and ask where multiline searching
is implemented. After indexing the repository (you can run `srcsearch index -p . -o .srcsearch` inside ripgrep folder), run the following inside ripgrep folder:

```bash
srcsearch search \
  --index-dir .srcsearch \
  --query 'multiline AND search'
```

The top results include entities such as:

```text
crates/core/flags/defs.rs:4503:1: Flag for Multiline
crates/searcher/src/searcher/glue.rs:149:1: MultiLine < 's , M , S >
README.md:211:1: Feature comparison
crates/searcher/src/searcher/mod.rs:627:1: Searcher
crates/core/flags/defs.rs:4591:1: Flag for MultilineDotall
```

The first hit is one indexed `Multiline` flag entity. Its declaration supplies
`multiline`, while methods and documentation within the entity supply `search`.
This is structure-aware retrieval: the searchable unit is the complete entity,
not an individual source line.

A comparable line-oriented search is still useful:

```bash
rg -n -i 'multiline.*search'
```

It finds 13 matching lines in 6 files in this version of ripgrep, including prose
such as `multiline search` and `multiline searches`. What it does not do is group
those lines into Rust entities or rank the entities. Use `srcsearch` to discover
the likely implementation units, then `rg` to inspect every textual occurrence.

### Example2: Search for a concept without knowing its wording

If you want documentation about searching for files, an exact search is narrow:

```bash
rg -n -i 'search for file'
```

It finds no lines in this version of ripgrep. Broadening the expression improves
recall, but produces 183 matching lines in 22 files for you to inspect and
prioritize:

```bash
rg -n -i 'search.*file'
```

A documentation-scoped `srcsearch` query instead ranks Markdown sections and Rust
documentation while excluding Rust signatures and code:

```bash
srcsearch search \
  --index-dir .srcsearch \
  --scope doc \
  --query 'search for file'
```

Its top results include:

```text
GUIDE.md:117:1: Recursive search
crates/core/main.rs:109:1: search
GUIDE.md:627:1: File encoding
GUIDE.md:324:1: Manual filtering: file types
GUIDE.md:949:1: Reducing preprocessor overhead
```

The query finds conceptual sections such as `Recursive search` and `Manual
filtering: file types` even though the exact phrase does not occur. If you add `--limit 1000` 
to the searchc command, you can see that it matches 318
documentation entities in total; the CLI shows the 10 highest-ranked results by
default. Ranking matters here because the goal is to find a useful starting point,
not to print hundreds of unranked occurrences.

<!--Relevance remains query- and corpus-dependent. For example, `File encoding` ranks
above `Manual filtering: file types`, so BM25 order is not a ground-truth judgment.
Documentation fields also use English stemming, allowing `run` to match inflected
forms such as `running`.-->

<!--A srcsearch query for `regex matcher` ranks the matcher
tests, matcher implementations, and the `grep-regex` crate documentation near the
top. An exhaustive `rg -n -i 'regex|matcher'` search returns 2,027 matching lines
in 88 files. These queries are not semantically identical; the comparison shows
the difference between ranked entity retrieval and exhaustive line retrieval, not
that one tool has universally better recall.-->

The figures above were reproduced with `srcsearch 0.2.0` and
`ripgrep 15.2.0`, using ripgrep repository commit
`3fce3b5bb0236da2df6d99672afb8a719642eca7`. Full entity counts were obtained
with `--limit 100000`, which exceeded the number of matches for each query.

### Know the limitation: ranked lexical search is not semantic understanding

Natural-language-like input does not make `srcsearch` a semantic or vector search
engine. It remains lexical, BM25-style search. A broad query such as `how regex
matching is performed` can rank individually related terms—for example, discussions
of globs compiled to regular expressions—without answering the intended question.
More specific concept terms usually produce better results.

<!--Use score explanations to understand a surprising result:

```bash
srcsearch search \
  --index-dir .srcsearch \
  --query "regex matcher" \
  --json \
  --explain
```

This reports the matched fields and, for the top results, Tantivy's full score
explanation. By comparison, `rg --json` provides structured match events rather
than relevance scores.-->

### Choose the tool based on the task

| Use case | Prefer |
| --- | --- |
| Find an exact string, regex, or every occurrence | `ripgrep` |
| Search any file type immediately, without an index | `ripgrep` |
| Discover likely Rust entities or Markdown sections by concept | `srcsearch` |
| Match query terms across the fields of one Rust entity | `srcsearch` |
| Search Markdown and Rust documentation/comments while excluding code | `srcsearch --scope doc` |

A productive workflow is to use `srcsearch` for discovery and then `ripgrep` for
exhaustive inspection. For example, `srcsearch` can identify names such as
`MultiLine`, `MultilineDotall`, and `Searcher`; once you know those names, `rg` can
find every exact occurrence and support regex-based follow-up searches.

---

## Development

```bash
cargo test
cargo fmt
```
