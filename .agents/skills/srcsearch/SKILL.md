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
srcsearch index -p . -o .srcsearch
```

Search with a few meaningful terms; use `AND` when both clues must match:

```sh
srcsearch search -i .srcsearch -q 'multiline AND search' -l 10
srcsearch search -i .srcsearch -s doc -q 'search for file' -l 10
```

`-s doc` searches documentation fields, including source doc comments.
Results point to entities or sections: open the surrounding content, then use
`rg` to locate exact lines. Add `--json` for entity boundaries and metadata.

Keep the index current after edits:

```sh
srcsearch update -p . -i .srcsearch --changed-file src/lib.rs
```

A top result is a starting point, and a missing top-ten hit does not prove
absence. Reformulate or switch to `rg` when results miss the target. In the
ripgrep benchmark, cross-line implementation and guide searches found useful
entities; exact error construction was found directly with `rg -n -F`.
