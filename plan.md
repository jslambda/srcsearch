# Python indexer integration plan

The Python indexer now has the conventional Cargo library layout. The following
work remains before it can be used by `srcsearch` to index Python projects.

## 1. Connect Python files to the `srcsearch` pipeline

- Add `python-indexer` as a path dependency of the root crate.
- Recognize `.py` files alongside Rust and Markdown when collecting and indexing
  targets.
- Add a Python source-record variant, or replace the Rust-specific source-record
  type with a language-neutral one.
- Preserve the language in persisted JSON and Tantivy documents, using a
  distinct record type such as `"python"`.
- Update search-result conversion and code-snippet extraction to handle Python
  entries as well as Rust entries.

## 2. Make symbols unambiguous

Methods are currently indexed only by their local name. Add a qualified name
or a parent/container field so `Client.fetch` and `OtherClient.fetch` can be
distinguished in search results and serialized output.

## 3. Decide and document declaration scope

The current index covers top-level functions and classes plus methods directly
inside top-level classes. Decide whether that intentional scope is sufficient.
If not, recursively traverse suites to index nested functions, nested classes,
and methods of nested classes. In either case, document the chosen behavior in
the public API documentation.

## 4. Apply project traversal rules suitable for Python

Avoid indexing virtual environments, caches, source-control metadata, build
outputs, and generated/vendor directories. In particular, skip directories
such as `.venv`, `venv`, `__pycache__`, `.git`, `build`, and `dist`, ideally by
sharing the root project's existing traversal and ignore policy rather than
maintaining a separate walker.

## 5. Establish failure and encoding behavior

- Decide whether a single unreadable or unparsable Python file should fail the
  entire index or be recorded as a per-file diagnostic while the remaining
  files are indexed.
- Support Python source encoding declarations if non-UTF-8 Python files are in
  scope; `read_to_string` currently accepts only UTF-8.
- Consider a concrete public error type so callers can distinguish traversal,
  file-read, decoding, and parse failures instead of receiving only
  `Box<dyn Error>`.

## 6. Validate the intended behavior

When implementing the above, add focused tests for Python target collection,
project-relative paths, language-specific searchable records, qualified method
names, ignored Python environment directories, multiline/decorated signatures,
and the selected parse-error and encoding policies.

## 7. Update public documentation

Once Python support is exposed through `srcsearch`, update the root README and
crate-level API documentation to describe supported extensions, record types,
scope limitations, and Python indexing examples.
