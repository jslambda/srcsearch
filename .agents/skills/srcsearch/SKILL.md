---
name: srcsearch
description: Use srcsearch to discover Rust or Python implementation entities and Markdown documentation when the exact wording is unknown or clues span multiple lines.
---

# srcsearch

Use `srcsearch` for ranked discovery. Use `rg` for exact strings, regexes,
and exhaustive textual matches.

Run from the target project root with `srcsearch` on PATH. Reuse its existing
index, or create one in an empty or nonexistent directory:

```sh
srcsearch index --project-root . --output-dir .srcsearch
```

Search with a few meaningful terms; use `AND` when both clues must match:

```sh
srcsearch search --index-dir .srcsearch --query 'multiline AND search' --limit 10
srcsearch search --index-dir .srcsearch --scope doc --query 'search for file' --limit 10
```

`--scope doc` searches documentation fields, including source doc comments.
Results point to entities or sections: open the surrounding content, then use
`rg` to locate exact lines. Add `--json` for entity boundaries and metadata.

Keep the index current after edits:

```sh
srcsearch update --project-root . --index-dir .srcsearch --changed-file src/lib.rs
```

A top result is a starting point, and a missing top-ten hit does not prove
absence. Reformulate when results miss the target. Switch to `rg` when you
have an exact error message or identifier, need a regex, or need every textual
occurrence. For example, use `rg --line-number --fixed-strings 'unrecognized flag --' .`
to locate an exact error string.
