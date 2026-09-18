
# Beyond grep

## Structure-aware search for Rust code with `srcsearch`

Hamid Alavi Toussi

---

<!-- .slide: class="lead" -->

# 1. Introduction / opening

## Where is multiline searching implemented?

You have just joined the `ripgrep` project.

You do not know:

- the identifier
- the crate
- the words used in the source

---

# Two different search questions

> **Where does this text occur?**

`ripgrep` returns matching lines.

> **Which implementation units are probably about this concept?**

`srcsearch` returns ranked code entities and documentation sections.

---

# The central idea

The biggest difference is the **unit being searched**.

| `ripgrep` | `srcsearch` |
| --- | --- |
| line | Rust entity or Markdown section |
| exact text / regular expression | analyzed query |
| exhaustive matches | ranked starting points |
| no index | prebuilt index |

The tools are complementary: **discover, then inspect**.

---

<!-- .slide: class="lead" -->

# 2. A motivating example

## “multiline search” in `ripgrep`

---

# Start with the natural search

```console
$ rg --vimgrep 'multiline.*search'
README.md:220:1:multiline search and opt-in fancy regex support via PCRE2.
CHANGELOG.md:524:58:  Fix bug where `\A` could produce unanchored matches in multiline search.
FAQ.md:519:24:slower when performing multiline searches? Well, that's because there are
FAQ.md:654:40:for line-by-line searching by enabling multiline search. After all, our
FAQ.md:682:58:valid UTF-8 to PCRE2. Unfortunately, one key downside of multiline search is
tests/multiline.rs:20:25:// Tests that even in a multiline search, a '.' does not match a newline.
tests/multiline.rs:91:15:// Tests that multiline search works when reading from stdin. This is an
tests/multiline.rs:92:27:// important test because multiline search must read the entire contents of
tests/multiline.rs:103:14:// Test that multiline search and contextual matches work.
tests/tests.rs:22:24:// Tests for ripgrep's multiline search support.
crates/core/flags/defs.rs:4541:38:default. This flag only applies when multiline search is enabled.
crates/core/flags/defs.rs:4610:29:match line terminators when multiline searching is enabled. This flag has no
crates/core/flags/defs.rs:4611:11:effect if multiline searching isn't enabled with the \flag{multiline} flag.
```

13 matching lines across 6 files in the pinned repository.

Every match is correct. Which one is the implementation?
<!--note: the tests results are generic and does not point to the specific implementation of multisearch-->

---

# The clue may span many lines

```rust
impl Flag for Multiline {
    fn name_long(&self) -> &'static str {
        "multiline"
    }

    fn doc_category(&self) -> Category {
        Category::Search
    }

    fn doc_short(&self) -> &'static str {
        r"Enable searching across multiple lines."
    }
}
```

`multiline` and `search` belong to one entity, but need not occur on one line.

---

# Ask for entities instead of lines

```console
$ srcsearch search \
    --index-dir .srcsearch \
    --query 'multiline AND search'
```

The first result is `Flag for Multiline`; other high-ranking entities include
`MultiLine<'s, M, S>`, `Searcher`, and `MultilineDotall`.

The result gives us useful identifiers and locations for the next search.

---

# Why the first result ranks

```text
crates/core/flags/defs.rs:4503:1
Flag for Multiline                         score: 18.597

term        field          score      contribution
search      code            1.734          9.3%
multiline   signature      16.862         90.7%
```

The record collects evidence from different fields.

No single matching line has to contain the whole clue.

---

<!-- .slide: class="lead" -->

# 3. How `srcsearch` works

## From files to `SearchRecord`s

---

# Choose meaningful boundaries

One file becomes many searchable records.

```text
Rust source ──parse items──────► functions, structs, enums,
                                 traits, modules, impl blocks

Markdown ────parse headings────► one section per heading

                    records ───► Tantivy index
```

Parsing supplies structure that plain text does not contain.

---

# Rust source → entity record

`rust2json` extracts an entity and its source location.

```text
SearchRecord::RustIndexEntry

kind            impl
name            Multiline
signature       impl Flag for Multiline
documentation   ...
file + lines    crates/core/flags/defs.rs:4503–...
```

The index keeps related declaration, documentation, and code together.

---

# Markdown → section record

`markdown2json` groups content beneath each heading.

```text
SearchRecord::MarkdownSection

title           Recursive search
body_text       paragraphs in this section
file + lines    README.md:...
```

<!--`srcsearch` indexes the heading as `title` and paragraphs as `body_text`.-->

---

# Records → fielded documents

```text
SearchRecord
    │
    ▼
Tantivy document
    │
    ├── content:   title · name · signature
    │              body_text · doc · code
    |               (used in search)
    ├── metadata:  record_type · file_path · kind
    └── locations: line_start · line_end (used for presenting a result) 
    │
    ▼
Tantivy document ──analyze fields──► inverted index
```

---

# Index once, search many times

```console
$ srcsearch index \
    --project-root . \
    --output-dir .srcsearch

$ srcsearch search \
    --index-dir .srcsearch \
    --query 'multiline AND search'
```

At query time: analyze the query, retrieve candidates, rank them, return the top N.

---

<!-- .slide: class="lead" -->

# 4. Search and indexing concepts

## Inverted index → TF-IDF intuition → BM25

---

# Analysis: what counts as a term?

| Fields | Analysis | Example |
| --- | --- | --- |
| `title`, `body_text`, `doc` | tokenize, lowercase, English stem | `Searching searched` → `search`, `search` |
| `signature`, `code` | tokenize, lowercase | `MultiLine` → `multiline` |
| `name`, `qualified_name` | exact whole value | `MultiLine` stays `MultiLine` |

The same analyzer is applied to the query for each field (at search time).

Stemming connects word forms. It does not add synonyms: `lookup` does not become `search`.

---

# The inverted index

Instead of scanning every record, map each **field + term** to its records:

```text
(signature, multiline) ──► A:1, B:1
(doc,       search)    ──► A:3, C:2
(code,      search)    ──► B:4
```

For our query:

$$
C(\texttt{multiline AND search})
= P(\texttt{multiline}) \cap P(\texttt{search})
= \{A,B\}
$$

The terms may occur in different fields of the same record.

---

# Retrieval first, ranking second

```text
analyzed query
      │
      ▼
postings lists + Boolean operators
      │
      ▼
eligible records
      │
      ▼
boosted BM25 contributions
      │
      ▼
ranked results
```

`AND` decides eligibility. A high score cannot replace a missing required term.

The default operator is `OR`, so write `AND` when both clues are required.

---

# TF-IDF: two useful intuitions

$$
\operatorname{score}(d,Q)
= \sum_{t \in Q}
\underbrace{\operatorname{tf}(t,d)}\_{\text{mentions}}
\underbrace{\ln\\!\left(\frac{N}{\operatorname{df}(t)}\right)}\_{\text{rarity}}
$$

1. More mentions add evidence.
2. Rare terms carry more information.

For 100 records:

| Records containing term | IDF |
| ---: | ---: |
| 5 | $\ln(20) \approx 3.00$ |
| 50 | $\ln(2) \approx 0.69$ |

`multiline` tells us more than the common word `search`.

---

# BM25: TF-IDF with restraint

$$
\operatorname{BM25}(d,Q)
= \sum_{t \in Q}\operatorname{IDF}(t)
\frac{\operatorname{tf}(t,d)(k_1+1)}
{\operatorname{tf}(t,d)+k_1\left(1-b+b\frac{L_d}{\overline L}\right)}
$$

BM25 adds two practical ideas:

- **term-frequency saturation:** the tenth mention adds less than the first
- **length normalization:** one hit in a focused record is stronger evidence than one hit in a very long record

`srcsearch` uses Tantivy's $k_1=1.2$ and $b=0.75$.

---

# One worked comparison

Suppose `multiline` appears in 5 of 100 records and the average field is 100 tokens.

$$
\operatorname{IDF}(\texttt{multiline})
= \ln\!\left(1+\frac{95.5}{5.5}\right) \approx 2.91
$$

| Record | TF | Length | BM25 term score |
| --- | ---: | ---: | ---: |
| A: compact entity | 1 | 50 | 3.66 |
| B: long entity | 3 | 200 | 3.77 |

Three mentions in a long entity only slightly outweigh one in a compact entity.

---

# Fields express what matters

| Field | Boost |
| --- | ---: |
| `title`, `name`, `qualified_name` | 4× |
| `signature`, `doc`, `body_text` | 2× |
| `code` | 1× |

For `multiline AND search`:

```text
multiline in signature ──► rare + focused + 2× boost
search in code          ──► common + long field + 1× boost
                           ─────────────────────────────
                           combined score for the entity
```

Each field has its own term and length statistics.

---

# Back to the motivating example

Why does `Flag for Multiline` rank first?

- `multiline` appears in its signature
- `search` appears in its code and documentation
- the rare, focused signature match dominates the score
- the complete entity satisfies both Boolean clauses

BM25 ranks **lexical evidence**. It does not measure whether the result truly answers the question.

---

# Search the right scope

| Scope | Default fields |
| --- | --- |
| `all` | `title`, `body_text`, `name`, `qualified_name`, `signature`, `doc`, `code` |
| `doc` | `title`, `body_text`, `doc` |

```console
$ srcsearch search \
    --index-dir .srcsearch \
    --scope doc \
    --query 'search for file'
```

Use document scope when the question asks for an explanation rather than an implementation.

---

<!-- .slide: class="lead" -->

# 5. `srcsearch` for coding agents

## Retrieval as the first step in repository exploration

---

# Agents begin with tasks, not identifiers

Typical requests sound like:

- “Change how multiline searching works.”
- “Find the code responsible for filtering files.”
- “Locate the documentation for search configuration.”

An agent entering an unfamiliar repository lacks the project's vocabulary.

A few complete, ranked entities provide both **context** and **new search terms**.

---

# A two-stage workflow

```text
task-level description
        │
        ▼
srcsearch: ranked structural retrieval
        │
        ├── likely functions and types
        ├── documentation sections
        └── identifiers + source locations
        │
        ▼
ripgrep / compiler / tests: precise verification
        │
        ▼
informed code change
```

Discover broadly; verify exhaustively before editing.

---

# Why it fits agent tooling

- Runs locally over the checked-out repository
- Returns bounded, structured results
- Preserves complete implementation units
- Includes source locations for direct inspection
- Supports JSON and score explanations
- Searches code and project documentation together

The index becomes a lightweight retrieval layer for developer tools.

---

# Boundaries to remember

- Ranking depends on the query and the indexed corpus.
- The top result can still be wrong.
- Lexical search does not understand synonyms or intent.
- The index must be built and kept fresh.
- Only supported languages and structures get meaningful boundaries.

A possible next step is hybrid search: lexical ranking plus embeddings, while preserving entity boundaries and source locations.

---

# Which tool should I use?

| Need | Start with |
| --- | --- |
| Known text or regular expression | `ripgrep` |
| Every occurrence | `ripgrep` |
| Likely implementation units | `srcsearch` |
| Ranked documentation sections | `srcsearch --scope doc` |
| Explore, then verify | both |

Better source search can begin with a more meaningful unit to search.

---

<!-- .slide: class="lead" -->

# A line tells you where words meet.

# An entity tells you where ideas belong.

## Questions?

[`github.com/jslambda/srcsearch`](https://github.com/jslambda/srcsearch)

---

# References

- [`srcsearch` repository and README](https://github.com/jslambda/srcsearch)
- [Tantivy 0.25.0 BM25 implementation](https://docs.rs/tantivy/0.25.0/src/tantivy/query/bm25.rs.html)
- [Tantivy query parser](https://docs.rs/tantivy/0.25.0/tantivy/query/struct.QueryParser.html)
- [`rust-indexer`](https://github.com/jslambda/rust-indexer)
- [`markdown-indexer`](https://github.com/jslambda/markdown-indexer)

The numerical examples are illustrative. The opening scores are measured from the pinned `ripgrep` example described in `talk.md`.
