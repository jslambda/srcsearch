use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use srcsearch::{
    SearchHit, SearchScope, index_project, index_target, search_tantivy_index,
    update_tantivy_index, write_json, write_tantivy_index,
};

#[derive(Clone, Debug, ValueEnum, PartialEq, Eq)]
enum SearchScopeArg {
    All,
    Doc,
}

impl From<SearchScopeArg> for SearchScope {
    fn from(value: SearchScopeArg) -> Self {
        match value {
            SearchScopeArg::All => SearchScope::All,
            SearchScopeArg::Doc => SearchScope::Doc,
        }
    }
}

const CLI_USAGE_HELP: &str = concat!(
    "Usage:\n",
    "  srcsearch json --project-root . --output index.json\n",
    "  srcsearch json --project-dir . -o index.json\n",
    "  srcsearch index --project-root . --output-dir index\n",
    "  srcsearch index -p . -o index\n",
    "  srcsearch update --project-root . --index-dir index --changed-file src/lib.rs\n",
    "  srcsearch search --index-dir index --query quickstart --scope doc\n",
    "  srcsearch search -i index -q quickstart -s doc",
);

#[derive(Debug, Parser)]
#[command(
    name = "srcsearch",
    version,
    about = "Index Rust and Markdown documentation",
    after_help = CLI_USAGE_HELP
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Json {
        #[arg(
            long,
            short = 'p',
            visible_alias = "project-dir",
            default_value = ".",
            value_name = "PATH"
        )]
        project_root: PathBuf,
        #[arg(long, short = 'o', value_name = "FILE")]
        output: PathBuf,
    },
    Index {
        #[arg(
            long,
            short = 'p',
            visible_alias = "project-dir",
            default_value = ".",
            value_name = "PATH"
        )]
        project_root: PathBuf,
        #[arg(long, short = 'o', value_name = "DIR")]
        output_dir: PathBuf,
    },
    Update {
        #[arg(
            long,
            short = 'p',
            visible_alias = "project-dir",
            default_value = ".",
            value_name = "PATH"
        )]
        project_root: PathBuf,
        #[arg(long, short = 'i', value_name = "DIR")]
        index_dir: PathBuf,
        #[arg(long = "changed-file", required = true, num_args = 1.., value_name = "PATH")]
        changed_files: Vec<String>,
    },
    #[command(
        about = "Search indexed content (stemming matches inflected forms in title/body/doc fields)"
    )]
    Search {
        #[arg(long, short = 'i', value_name = "DIR")]
        index_dir: PathBuf,
        #[arg(long, short = 'q', value_name = "QUERY")]
        query: String,
        #[arg(long, short = 'l', default_value_t = 10, value_name = "N")]
        limit: i64,
        #[arg(long, short = 's', value_enum, default_value_t = SearchScopeArg::All)]
        scope: SearchScopeArg,
        #[arg(long)]
        json: bool,
    },
}

/// Returns `Some` only when the input string exists and is not whitespace-only.
fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.trim().is_empty())
}

/// Builds a stable display label for a hit by preferring fields that best identify the record.
fn format_hit_label(hit: &SearchHit) -> String {
    // Field precedence rules (keep stable for deterministic output):
    // 1) markdown prefers section title, then name/signature fallbacks.
    // 2) rust prefers symbol name, then signature, then title fallback.
    // 3) unknown/missing values resolve to "(untitled)".
    match hit.record_type.as_str() {
        "markdown" => non_empty(hit.title.as_deref())
            .or(non_empty(hit.name.as_deref()))
            .or(non_empty(hit.signature.as_deref()))
            .unwrap_or("(untitled)")
            .to_string(),
        "rust" => non_empty(hit.name.as_deref())
            .or(non_empty(hit.signature.as_deref()))
            .or(non_empty(hit.title.as_deref()))
            .unwrap_or("(untitled)")
            .to_string(),
        _ => non_empty(hit.title.as_deref())
            .or(non_empty(hit.name.as_deref()))
            .or(non_empty(hit.signature.as_deref()))
            .unwrap_or("(untitled)")
            .to_string(),
    }
}

/// Formats search hits into `path:line:col: label` lines that editors can parse as jump targets.
fn format_search_hits(hits: &[SearchHit]) -> String {
    if hits.is_empty() {
        return "No results found.\n".to_string();
    }

    let mut output = String::new();
    for hit in hits {
        let line_start = hit.line_start.unwrap_or(1);
        output.push_str(&format!(
            "{}:{}:1: {}\n",
            hit.file_path,
            line_start,
            format_hit_label(hit)
        ));
    }

    output
}

/// Parses CLI arguments and dispatches to indexing, update, or search workflows.
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    match args.command {
        Commands::Json {
            project_root,
            output,
        } => {
            let records = index_project(&project_root)?;
            write_json(&records, &output)?;
        }
        Commands::Index {
            project_root,
            output_dir,
        } => {
            let records = index_project(&project_root)?;
            write_tantivy_index(&records, &output_dir, Some(&project_root))?;
        }
        Commands::Update {
            project_root,
            index_dir,
            changed_files,
        } => {
            let mut changed_records = Vec::new();
            for changed_file in &changed_files {
                let path = project_root.join(changed_file);
                if !path.exists() {
                    continue;
                }
                let mut file_records = index_target(&path, &project_root)?;
                changed_records.append(&mut file_records);
            }

            update_tantivy_index(
                &changed_records,
                &index_dir,
                Some(&project_root),
                &changed_files,
            )?;
        }
        Commands::Search {
            index_dir,
            query,
            limit,
            scope,
            json,
        } => {
            let hits = search_tantivy_index(&index_dir, &query, limit, scope.into())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&hits)?);
            } else {
                print!("{}", format_search_hits(&hits));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Cli, Commands, SearchScopeArg, format_search_hits};
    use clap::{CommandFactory, Parser, error::ErrorKind};
    use srcsearch::SearchHit;
    use std::path::PathBuf;

    #[test]
    fn parses_json_subcommand() {
        let cli = Cli::parse_from([
            "srcsearch",
            "json",
            "--project-root",
            ".",
            "--output",
            "index.json",
        ]);

        match cli.command {
            Commands::Json {
                project_root,
                output,
            } => {
                assert_eq!(project_root, PathBuf::from("."));
                assert_eq!(output, PathBuf::from("index.json"));
            }
            Commands::Index { .. } | Commands::Update { .. } | Commands::Search { .. } => {
                panic!("expected json subcommand")
            }
        }
    }

    #[test]
    fn parses_index_subcommand() {
        let cli = Cli::parse_from([
            "srcsearch",
            "index",
            "--project-root",
            ".",
            "--output-dir",
            "index",
        ]);

        match cli.command {
            Commands::Index {
                project_root,
                output_dir,
            } => {
                assert_eq!(project_root, PathBuf::from("."));
                assert_eq!(output_dir, PathBuf::from("index"));
            }
            Commands::Json { .. } | Commands::Update { .. } | Commands::Search { .. } => {
                panic!("expected index subcommand")
            }
        }
    }

    #[test]
    fn parses_short_aliases() {
        let cli = Cli::parse_from(["srcsearch", "json", "-p", ".", "-o", "index.json"]);

        match cli.command {
            Commands::Json {
                project_root,
                output,
            } => {
                assert_eq!(project_root, PathBuf::from("."));
                assert_eq!(output, PathBuf::from("index.json"));
            }
            Commands::Index { .. } | Commands::Update { .. } | Commands::Search { .. } => {
                panic!("expected json subcommand")
            }
        }
    }

    #[test]
    fn defaults_project_dir_to_current_directory() {
        let cli = Cli::parse_from(["srcsearch", "index", "-o", "index"]);

        match cli.command {
            Commands::Index {
                project_root,
                output_dir,
            } => {
                assert_eq!(project_root, PathBuf::from("."));
                assert_eq!(output_dir, PathBuf::from("index"));
            }
            Commands::Json { .. } | Commands::Update { .. } | Commands::Search { .. } => {
                panic!("expected index subcommand")
            }
        }
    }

    #[test]
    fn parses_project_dir_alias() {
        let cli = Cli::parse_from([
            "srcsearch",
            "index",
            "--project-dir",
            ".",
            "--output-dir",
            "index",
        ]);

        match cli.command {
            Commands::Index {
                project_root,
                output_dir,
            } => {
                assert_eq!(project_root, PathBuf::from("."));
                assert_eq!(output_dir, PathBuf::from("index"));
            }
            Commands::Json { .. } | Commands::Update { .. } | Commands::Search { .. } => {
                panic!("expected index subcommand")
            }
        }
    }

    #[test]
    fn parses_update_subcommand() {
        let cli = Cli::parse_from([
            "srcsearch",
            "update",
            "--project-root",
            ".",
            "--index-dir",
            "index",
            "--changed-file",
            "src/lib.rs",
            "--changed-file",
            "README.md",
        ]);

        match cli.command {
            Commands::Update {
                project_root,
                index_dir,
                changed_files,
            } => {
                assert_eq!(project_root, PathBuf::from("."));
                assert_eq!(index_dir, PathBuf::from("index"));
                assert_eq!(
                    changed_files,
                    vec!["src/lib.rs".to_string(), "README.md".to_string()]
                );
            }
            Commands::Json { .. } | Commands::Index { .. } | Commands::Search { .. } => {
                panic!("expected update subcommand")
            }
        }
    }

    #[test]
    fn parses_search_subcommand_with_defaults() {
        let cli = Cli::parse_from([
            "srcsearch",
            "search",
            "--index-dir",
            "index",
            "--query",
            "quickstart",
        ]);

        match cli.command {
            Commands::Search {
                index_dir,
                query,
                limit,
                scope,
                json,
            } => {
                assert_eq!(index_dir, PathBuf::from("index"));
                assert_eq!(query, "quickstart");
                assert_eq!(limit, 10);
                assert_eq!(scope, SearchScopeArg::All);
                assert!(!json);
            }
            Commands::Json { .. } | Commands::Index { .. } | Commands::Update { .. } => {
                panic!("expected search subcommand")
            }
        }
    }

    #[test]
    fn parses_search_subcommand_short_flags() {
        let cli = Cli::parse_from([
            "srcsearch",
            "search",
            "-i",
            "index",
            "-q",
            "tantivy",
            "-l",
            "5",
        ]);

        match cli.command {
            Commands::Search {
                index_dir,
                query,
                limit,
                scope,
                json,
            } => {
                assert_eq!(index_dir, PathBuf::from("index"));
                assert_eq!(query, "tantivy");
                assert_eq!(limit, 5);
                assert_eq!(scope, SearchScopeArg::All);
                assert!(!json);
            }
            Commands::Json { .. } | Commands::Index { .. } | Commands::Update { .. } => {
                panic!("expected search subcommand")
            }
        }
    }

    #[test]
    fn parses_search_subcommand_large_limit() {
        let cli = Cli::parse_from([
            "srcsearch",
            "search",
            "--index-dir",
            "index",
            "--query",
            "tantivy",
            "--limit",
            "9223372036854775807",
        ]);

        match cli.command {
            Commands::Search { limit, json, .. } => {
                assert_eq!(limit, i64::MAX);
                assert!(!json);
            }
            Commands::Json { .. } | Commands::Index { .. } | Commands::Update { .. } => {
                panic!("expected search subcommand")
            }
        }
    }

    #[test]
    fn parses_search_scope_doc() {
        let cli = Cli::parse_from([
            "srcsearch",
            "search",
            "--index-dir",
            "index",
            "--query",
            "quickstart",
            "--scope",
            "doc",
        ]);

        match cli.command {
            Commands::Search { scope, .. } => assert_eq!(scope, SearchScopeArg::Doc),
            Commands::Json { .. } | Commands::Index { .. } | Commands::Update { .. } => {
                panic!("expected search subcommand")
            }
        }
    }

    #[test]
    fn parses_search_scope_short_alias() {
        let cli = Cli::parse_from([
            "srcsearch",
            "search",
            "-i",
            "index",
            "-q",
            "quickstart",
            "-s",
            "doc",
        ]);

        match cli.command {
            Commands::Search { scope, .. } => assert_eq!(scope, SearchScopeArg::Doc),
            Commands::Json { .. } | Commands::Index { .. } | Commands::Update { .. } => {
                panic!("expected search subcommand")
            }
        }
    }

    #[test]
    fn rejects_invalid_search_scope() {
        let err = Cli::try_parse_from([
            "srcsearch",
            "search",
            "--index-dir",
            "index",
            "--query",
            "quickstart",
            "--scope",
            "invalid",
        ])
        .expect_err("invalid scope should fail");

        assert_eq!(err.kind(), ErrorKind::InvalidValue);
    }

    #[test]
    fn parses_search_json_flag() {
        let cli = Cli::parse_from([
            "srcsearch",
            "search",
            "--index-dir",
            "index",
            "--query",
            "quickstart",
            "--json",
        ]);

        match cli.command {
            Commands::Search { json, .. } => assert!(json),
            Commands::Json { .. } | Commands::Index { .. } | Commands::Update { .. } => {
                panic!("expected search subcommand")
            }
        }
    }

    #[test]
    fn formats_empty_search_results() {
        assert_eq!(format_search_hits(&[]), "No results found.\n");
    }

    #[test]
    fn formats_search_result_lines_for_editor_links() {
        let hits = vec![SearchHit {
            score: 1.2345,
            record_type: "rust".to_string(),
            file_path: "src/lib.rs".to_string(),
            title: None,
            name: Some("search_tantivy_index".to_string()),
            kind: Some("fn".to_string()),
            signature: Some("pub fn search_tantivy_index(...)".to_string()),
            line_start: Some(42),
            line_end: Some(47),
            heading_line: None,
        }];

        let output = format_search_hits(&hits);

        assert_eq!(output, "src/lib.rs:42:1: search_tantivy_index\n");
    }

    #[test]
    fn formats_search_result_lines_with_default_line_number() {
        let hits = vec![SearchHit {
            score: 2.0,
            record_type: "markdown".to_string(),
            file_path: "README.md".to_string(),
            title: Some("semantic search over code and docs".to_string()),
            name: None,
            kind: None,
            signature: None,
            line_start: None,
            line_end: None,
            heading_line: None,
        }];

        let output = format_search_hits(&hits);

        assert_eq!(
            output,
            "README.md:1:1: semantic search over code and docs\n"
        );
    }

    #[test]
    fn search_help_mentions_stemming_behavior() {
        let mut command = Cli::command();
        let help = command
            .find_subcommand_mut("search")
            .expect("search subcommand should exist")
            .render_long_help()
            .to_string();

        assert!(help.contains("stemming matches inflected forms in title/body/doc fields"));
    }

    #[test]
    fn help_usage_mentions_search_and_aliases() {
        let mut command = Cli::command();
        let help = command.render_help().to_string();

        assert!(help.contains("srcsearch json --project-root . --output index.json"));
        assert!(help.contains("srcsearch json --project-dir . -o index.json"));
        assert!(help.contains("srcsearch index --project-root . --output-dir index"));
        assert!(help.contains("srcsearch index -p . -o index"));
        assert!(help.contains("srcsearch search --index-dir index --query quickstart --scope doc"));
        assert!(help.contains("srcsearch search -i index -q quickstart -s doc"));
    }
}
