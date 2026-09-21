# Sandbox-local ripgrep search benchmark

This small benchmark checks whether `srcsearch` returns useful starting points
for three questions a coding agent might ask about ripgrep. It runs only local
search commands; no separate Codex process, network access, or sandbox
exception is needed. It measures **retrieval**, not Codex answer quality or
the number of tool calls Codex would choose to make.

The first two questions come from the ripgrep examples in the main README:
finding a Rust implementation whose clues are spread across an entity, and
finding documentation when the exact wording is unknown. The third is an exact
string control. The paired queries are fixed in advance so the run is
repeatable. They are examples of reasonable first queries, not optimized
search strategies for either tool.

## Corpus and setup

- ripgrep: `.project/ripgrep`, commit
  `3fce3b5bb0236da2df6d99672afb8a719642eca7`
- srcsearch: `0.2.0`, built from this checkout
- `rg`: local `ripgrep 15.2.0`
- Index build is outside the query timings. Build it once, then run all
  searches from the ripgrep directory against that same index.

```sh
cargo build
bench_index=$(mktemp -d)
target/debug/srcsearch index -p .project/ripgrep -o "$bench_index"
cd .project/ripgrep
srcsearch_bin=$(realpath ../../target/debug/srcsearch)
```

Use the *absolute* path to `bench_index` from the setup shell in every
`srcsearch` command. The index command accepts an empty directory.

## Tasks and fixed first queries

| Task | Question | `rg` query | `srcsearch` query | Answer location |
| --- | --- | --- | --- | --- |
| 1. Cross-line search | Where are the strategy chosen and the multiline search performed? | `rg -n -i 'multiline.*search' .` | `"$srcsearch_bin" search -i "$bench_index" -q 'multiline AND search' -l 10` | `crates/searcher/src/searcher/mod.rs` around 769–792 and `crates/searcher/src/searcher/glue.rs` around 142–210 |
| 2. File types | Where does the guide explain searching only Rust files without a glob? | `rg -n -i 'search.*file' -g '*.md' .` | `"$srcsearch_bin" search -i "$bench_index" -s doc -q 'search for file' -l 10` | `GUIDE.md` section “Manual filtering: file types,” starting at 324; `--type rust` is shown around 337–345 |
| 3. Exact error | Where is `unrecognized flag --` constructed, and how is a suggestion added? | `rg -n -F 'unrecognized flag --' .` | `"$srcsearch_bin" search -i "$bench_index" -q 'unrecognized flag' -l 10` | `crates/core/flags/parse.rs` around 265–269 |

For each output, record the rank of the first result pointing into the answer
location. A `srcsearch` result counts when its entity or Markdown section
contains that location; an `rg` result counts when its line is in that entity
or section. For task 1, record both implementation locations. Also record
the number of returned lines. For repeatable latency measurements, run each
command seven times, discard no runs, and report the median wall time. Query
latency excludes index build time and says little about an agent's total time.

## Local run, 2026-09-21

The index was freshly built from the pinned checkout. Results below use the
fixed commands above. Times are medians of seven local subprocess runs
on this machine; they are context, not a general speed comparison.

| Task | Tool | Relevant result rank | Returned lines | Median query time |
| --- | --- | --- | ---: | ---: |
| 1 | `rg` | No line in either implementation unit | 13 | 7.64 ms |
| 1 | `srcsearch` | `glue.rs` 2; `mod.rs` 4 | 10 | 14.78 ms |
| 2 | `rg` | Guide section lines at 8 and 9 | 54 | 6.47 ms |
| 2 | `srcsearch` | Guide section 4 | 10 | 12.97 ms |
| 3 | `rg` | Error construction line 1 | 1 | 7.41 ms |
| 3 | `srcsearch` | No entity containing line 266 in top 10; rank 1 is another entity in `parse.rs` | 10 | 14.15 ms |

This suggests `srcsearch` is useful as an **additional discovery tool** for
the cross-line implementation question, and possibly for documentation
discovery. The exact-string control favors `rg`. The fixed queries differ in
expressiveness and a coding agent can reformulate them, inspect results, and
combine tools. Therefore these numbers do not establish that Codex answers
better or faster with `srcsearch`; that requires isolated Codex runs with and
without access to the index.
