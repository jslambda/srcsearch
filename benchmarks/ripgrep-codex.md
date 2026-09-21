# Tiny Codex search benchmark: ripgrep

This is a six-run pilot for a specific question: does giving Codex access to
`srcsearch` help it locate and explain answers in the ripgrep repository? It
compares normal `rg`-based exploration with the same exploration plus a ready
`srcsearch` index. The tasks are based on the two ripgrep examples in the main
README: finding a Rust entity from clues spread across its code, and finding a
documentation section without knowing its wording. The third task is an exact
string control where `rg` should work well.

## Setup

1. Use ripgrep commit `3fce3b5bb0236da2df6d99672afb8a719642eca7`
   in `.project/ripgrep`. Build `srcsearch` from this repository and build one
   index before the timed runs. Keep the index unchanged for every run.

   ```sh
   cargo build
   bench_index=$(mktemp -d)
   target/debug/srcsearch index -p .project/ripgrep -o "$bench_index"
   ```

2. Start a fresh Codex session for each task and condition, with the working
   directory set to `.project/ripgrep`. Give it only the shared instructions,
   the condition instructions, and one task prompt below. Do not give it this
   file, the srcsearch README, previous transcripts, or the answer key. Keep
   model, effort, time limit, and available non-search tools the same.
3. Run each task once per condition (six runs). Alternate condition order:
   A then B for task 1, B then A for task 2, A then B for task 3. Repeating
   with the reverse order later will help distinguish a real effect from run
   variation.

**Shared instructions:** “Work only in this ripgrep checkout. Answer the
question with repository-relative file paths and line numbers, and briefly
explain what the cited code or documentation says. You may inspect files with
normal shell commands. Do not edit files or use the web. Stop after 5 minutes
or 12 search commands, whichever comes first.”

**Condition A, `rg` baseline:** “Use `rg` (including `rg --files`) for
repository search. Do not use `srcsearch` or its index.”

**Condition B, `srcsearch` available:** “You may use both `rg` and
`srcsearch` for repository search. The index is already built. Search it with
`<absolute path to the srcsearch binary> search -i <absolute path to
bench_index> -q 'your query'`.” For example, the binary path may be
`/absolute/path/to/srcsearch/target/debug/srcsearch`. Tell Codex the actual
binary and index paths in the run prompt. Do not require it to use
`srcsearch`; whether it chooses to is part of the result.

## Tasks (give only the quoted prompt to Codex)

1. **Implementation discovery:** “When a pattern can match across line
   boundaries, where does the searcher select that strategy, and which unit
   performs the search? Cite both locations and explain their roles.”
2. **Documentation discovery:** “A user wants to search only Rust files
   without writing a glob. Find the guide section that explains how, give
   the command it recommends, and cite the section.”
3. **Exact string control:** “Find the code that constructs the error message
   `unrecognized flag --` for a long option. What happens before that error
   is returned if a suggestion is available? Cite the code.”

## Answer key and scoring

Score from the answer and its cited source, not from whether a particular
search command was used. Give each task 0, 1, or 2 points. A correct answer
without a valid citation gets at most 1 point. Accept nearby line numbers
that identify the same function or section.

| Task | Two-point answer | One-point answer |
| --- | --- | --- |
| 1 | `crates/searcher/src/searcher/mod.rs` around lines 769–792 selects `MultiLine::new(...).run()` when `multi_line_with_matcher` is true; `crates/searcher/src/searcher/glue.rs` around lines 142–210 defines and runs `MultiLine`. Both roles are explained and cited. | Finds only one of those locations, or cites both without distinguishing their roles. |
| 2 | `GUIDE.md` around line 324, “Manual filtering: file types,” explains `rg 'fn run' --type rust` (or `-trust`) and cites it. | Gives the right flag or the right section, but not both with support. |
| 3 | `crates/core/flags/parse.rs` around lines 265–269 constructs the message, calls `suggest(&name)`, and appends the suggestion after a blank line before returning the error. | Finds the message construction but misses or misstates the suggestion behavior. |

Record one row per run:

| Task | Condition | Score (0–2) | Elapsed seconds | Search commands | All tool calls | Input + output tokens, if available | Used srcsearch? | Notes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- | --- |

Report the paired score difference for each task and the total points out of
6 for each condition. Also report median elapsed time and search-command
count, plus any timeout. Include the exact Codex model and `srcsearch` version.
Six runs can show whether this setup is promising; they cannot establish a
reliable performance advantage. In particular, the exact string task checks
that adding `srcsearch` does not distract Codex from a simple `rg` search.
