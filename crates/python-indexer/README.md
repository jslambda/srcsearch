# python-indexer

`python-indexer` is a Rust library for building a lightweight index of Python
source code. It parses Python files and returns `IndexEntry` values for
top-level functions, top-level classes, and methods defined directly in those
classes.

Each entry includes its kind, local and qualified names, file path, source
lines, declaration signature, and (when present) the complete Python docstring
plus its first non-empty line.

## `IndexEntry`

`IndexEntry` is the main output unit of the library. Each indexed declaration
produces one value with this shape:

```rust
pub struct IndexEntry {
    pub kind: String,                // "fn", "async_fn", or "class"
    pub name: String,                // Declaration name
    pub qualified_name: String,      // Class-qualified method name
    pub file: String,                // Indexed file path
    pub line_start: u32,             // One-based starting line
    pub line_end: u32,               // One-based ending line
    pub signature: String,           // Declaration header without decorators
    pub doc_summary: Option<String>, // First non-empty docstring line
    pub doc: Option<String>,         // Complete Python docstring
}
```

`doc_summary` and `doc` are `None` when the declaration has no docstring.

## Use as a library

Add the crate to a Rust project. While developing alongside this repository,
use a path dependency:

```toml
[dependencies]
python-indexer = { path = "../python-indexer" }
```

Index a single Python file with `build_file_index`:

```rust
use std::error::Error;
use std::path::Path;

use python_indexer::build_file_index;

fn main() -> Result<(), Box<dyn Error>> {
    let entries = build_file_index(Path::new("src/service.py"))?;

    for entry in entries {
        println!("{} {} at {}:{}", entry.kind, entry.qualified_name, entry.file, entry.line_start);
        println!("  {}", entry.signature);
        if let Some(summary) = entry.doc_summary {
            println!("  {summary}");
        }
    }

    Ok(())
}
```

To recursively index every `.py` file below a project directory, use
`build_index`:

```rust
use std::error::Error;
use std::path::Path;

use python_indexer::build_index;

fn main() -> Result<(), Box<dyn Error>> {
    let entries = build_index(Path::new("./my-python-project"))?;
    println!("Indexed {} declarations", entries.len());
    Ok(())
}
```

`build_index` reports file paths relative to the supplied project directory;
`build_file_index` reports the path passed to it. Both return an error when a
file cannot be read or parsed.

## Indexed declarations

Given this Python file:

```python
class Ac:
    """An example client."""

    def myfoo(self, value: int) -> str:
        """Format a value."""
        return str(value)
```

the library produces separate entries for `Ac` and `myfoo`. The method entry
has qualified name `"Ac.myfoo"`, kind `"fn"`, signature
`"def myfoo(self, value: int) -> str"`, and the docstring summary
`"Format a value."`. Asynchronous functions and methods use kind `"async_fn"`;
classes use `"class"`.

The index intentionally covers top-level declarations and direct methods of
top-level classes. It does not currently index nested functions, nested
classes, or methods inherited from another class.
