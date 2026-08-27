use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use ruff_python_ast::{Expr, Stmt};
use ruff_python_parser::parse_module;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    pub kind: String,
    pub name: String,
    pub file: String,
    pub line_start: u32,
    pub line_end: u32,
    pub signature: String,
    pub doc_summary: Option<String>,
    pub doc: Option<String>,
}

/// Build an index of the top-level classes and functions in every Python file
/// below `project_root`.
pub fn build_index(project_root: &Path) -> Result<Vec<IndexEntry>, Box<dyn Error>> {
    if !project_root.is_dir() {
        return Err(format!("{} is not a directory", project_root.display()).into());
    }
    let mut files = Vec::new();
    collect_python_files(project_root, &mut files)?;
    files.sort();

    let mut entries = Vec::new();
    for path in files {
        let relative = path.strip_prefix(project_root).unwrap_or(&path);
        entries.extend(index_path(&path, relative)?);
    }
    Ok(entries)
}

/// Build an index for a single Python source file.
pub fn build_file_index(path: &Path) -> Result<Vec<IndexEntry>, Box<dyn Error>> {
    index_path(path, path)
}

/// Recursively adds Python source files below `directory` to `files`.
fn collect_python_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_python_files(&path, files)?;
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "py") {
            files.push(path);
        }
    }
    Ok(())
}

/// Parses and indexes one Python source file using `displayed_path` in its entries.
fn index_path(path: &Path, displayed_path: &Path) -> Result<Vec<IndexEntry>, Box<dyn Error>> {
    let source = fs::read_to_string(path)?;
    let parsed = parse_module(&source)?;
    Ok(index_suite(displayed_path, &source, parsed.suite()))
}

/// Creates entries for top-level declarations and methods of top-level classes.
fn index_suite(path: &Path, source: &str, suite: &[Stmt]) -> Vec<IndexEntry> {
    suite
        .iter()
        .flat_map(|statement| match statement {
            Stmt::FunctionDef(function) => vec![build_entry(
                if function.is_async { "async_fn" } else { "fn" },
                function.name.as_str(),
                path,
                source,
                function.range.start().into(),
                function.range.end().into(),
                function.body.as_slice(),
            )],
            Stmt::ClassDef(class) => {
                let mut entries = vec![build_entry(
                    "class",
                    class.name.as_str(),
                    path,
                    source,
                    class.range.start().into(),
                    class.range.end().into(),
                    class.body.as_slice(),
                )];
                entries.extend(class.body.iter().filter_map(|statement| {
                    let Stmt::FunctionDef(method) = statement else {
                        return None;
                    };
                    Some(build_entry(
                        if method.is_async { "async_fn" } else { "fn" },
                        method.name.as_str(),
                        path,
                        source,
                        method.range.start().into(),
                        method.range.end().into(),
                        method.body.as_slice(),
                    ))
                }));
                entries
            }
            _ => Vec::new(),
        })
        .collect()
}

/// Builds an index entry from a parsed declaration and its source range.
fn build_entry(
    kind: &str,
    name: &str,
    file: &Path,
    source: &str,
    start: u32,
    end: u32,
    body: &[Stmt],
) -> IndexEntry {
    let (doc_summary, doc) = extract_docs(body);
    IndexEntry {
        kind: kind.to_string(),
        name: name.to_string(),
        file: file.to_string_lossy().into_owned(),
        line_start: line_number(source, start as usize),
        line_end: line_number(source, end as usize),
        signature: signature(source, start as usize, body),
        doc_summary,
        doc,
    }
}

/// Returns the one-based line number containing `offset`.
fn line_number(source: &str, offset: usize) -> u32 {
    source.as_bytes()[..offset.min(source.len())]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count() as u32
        + 1
}

/// Extracts a declaration's signature, excluding decorators and its trailing colon.
fn signature(source: &str, start: usize, body: &[Stmt]) -> String {
    let body_start = body
        .first()
        .map(statement_start)
        .unwrap_or(source.len())
        .min(source.len());
    let header = source[start.min(body_start)..body_start].trim_end();
    let header = declaration_header(header);
    header
        .strip_suffix(':')
        .unwrap_or(header)
        .trim_end()
        .to_string()
}

/// Removes leading decorators from a declaration header.
fn declaration_header(header: &str) -> &str {
    let mut offset = 0;
    for line in header.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("def ")
            || trimmed.starts_with("async def ")
            || trimmed.starts_with("class ")
        {
            return &header[offset + (line.len() - trimmed.len())..];
        }
        offset += line.len();
    }
    header
}

/// Returns the byte offset at which a statement begins.
fn statement_start(statement: &Stmt) -> usize {
    let start: u32 = match statement {
        Stmt::FunctionDef(node) => node.range.start().into(),
        Stmt::ClassDef(node) => node.range.start().into(),
        Stmt::Return(node) => node.range.start().into(),
        Stmt::Delete(node) => node.range.start().into(),
        Stmt::TypeAlias(node) => node.range.start().into(),
        Stmt::Assign(node) => node.range.start().into(),
        Stmt::AugAssign(node) => node.range.start().into(),
        Stmt::AnnAssign(node) => node.range.start().into(),
        Stmt::For(node) => node.range.start().into(),
        Stmt::While(node) => node.range.start().into(),
        Stmt::If(node) => node.range.start().into(),
        Stmt::With(node) => node.range.start().into(),
        Stmt::Match(node) => node.range.start().into(),
        Stmt::Raise(node) => node.range.start().into(),
        Stmt::Try(node) => node.range.start().into(),
        Stmt::Assert(node) => node.range.start().into(),
        Stmt::Import(node) => node.range.start().into(),
        Stmt::ImportFrom(node) => node.range.start().into(),
        Stmt::Global(node) => node.range.start().into(),
        Stmt::Nonlocal(node) => node.range.start().into(),
        Stmt::Expr(node) => node.range.start().into(),
        Stmt::Pass(node) => node.range.start().into(),
        Stmt::Break(node) => node.range.start().into(),
        Stmt::Continue(node) => node.range.start().into(),
        Stmt::IpyEscapeCommand(node) => node.range.start().into(),
    };
    start as usize
}

/// Extracts a Python docstring and its first non-empty line from a body.
fn extract_docs(body: &[Stmt]) -> (Option<String>, Option<String>) {
    let Some(Stmt::Expr(expression)) = body.first() else {
        return (None, None);
    };
    let Expr::StringLiteral(literal) = expression.value.as_ref() else {
        return (None, None);
    };
    let doc = literal.value.to_str().to_string();
    let summary = doc
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string);
    (summary, Some(doc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    /// Verifies indexing declarations and their docstrings.
    fn indexes_python_declarations() -> Result<(), Box<dyn Error>> {
        let source = r#"def greet(name: str = "world") -> str:
    """Return a greeting.

    More details.
    """
    return f"Hello, {name}"

async def fetch(url):
    pass

class Client(Base):
    """An HTTP client."""

    async def fetch(self, url: str):
        """Fetch a resource."""
        pass

value = 42
"#;
        let path = temporary_path("declarations");
        fs::write(&path, source)?;
        let entries = build_file_index(&path)?;
        fs::remove_file(&path)?;

        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].kind, "fn");
        assert_eq!(entries[0].name, "greet");
        assert_eq!(
            entries[0].signature,
            "def greet(name: str = \"world\") -> str"
        );
        assert_eq!(entries[0].line_start, 1);
        assert_eq!(entries[0].line_end, 6);
        assert_eq!(
            entries[0].doc_summary.as_deref(),
            Some("Return a greeting.")
        );
        assert!(entries[0].doc.as_deref().unwrap().contains("More details."));
        assert_eq!(entries[1].kind, "async_fn");
        assert_eq!(entries[2].kind, "class");
        assert_eq!(entries[3].kind, "async_fn");
        assert_eq!(entries[3].name, "fetch");
        assert_eq!(entries[3].signature, "async def fetch(self, url: str)");
        assert_eq!(entries[3].doc_summary.as_deref(), Some("Fetch a resource."));
        Ok(())
    }

    #[test]
    /// Verifies decorators are omitted from rendered signatures.
    fn excludes_decorators_from_signatures() -> Result<(), Box<dyn Error>> {
        let path = temporary_path("decorators");
        fs::write(
            &path,
            "@route('/items')\nasync def items(limit: int = 10):\n    pass\n",
        )?;
        let entries = build_file_index(&path)?;
        fs::remove_file(&path)?;

        assert_eq!(entries[0].line_start, 1);
        assert_eq!(entries[0].signature, "async def items(limit: int = 10)");
        Ok(())
    }

    #[test]
    /// Verifies decorator strings cannot be mistaken for a declaration header.
    fn ignores_declaration_like_lines_inside_decorator_strings() -> Result<(), Box<dyn Error>> {
        let path = temporary_path("decorator_string");
        fs::write(
            &path,
            "@decorate(\n    \"\"\"\ndef not_a_declaration():\n    \"\"\"\n)\ndef greet() -> str:\n    return \"hello\"\n",
        )?;
        let entries = build_file_index(&path)?;
        fs::remove_file(&path)?;

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].signature, "def greet() -> str");
        Ok(())
    }

    #[test]
    /// Verifies recursive indexing uses project-relative paths.
    fn recursively_indexes_python_files_with_relative_paths() -> Result<(), Box<dyn Error>> {
        let root = temporary_path("project");
        let package = root.join("package");
        fs::create_dir_all(&package)?;
        fs::write(root.join("top.py"), "def top():\n    pass\n")?;
        fs::write(package.join("nested.py"), "class Nested:\n    pass\n")?;
        fs::write(package.join("ignored.txt"), "def ignored(): pass\n")?;
        let entries = build_index(&root)?;
        fs::remove_dir_all(&root)?;

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].file, "package/nested.py");
        assert_eq!(entries[1].file, "top.py");
        Ok(())
    }

    /// Creates a unique temporary Python-file path for a test fixture.
    fn temporary_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("python_indexer_{label}_{unique}.py"))
    }
}
