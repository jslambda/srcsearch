
# Beyond grep

## Structure-aware search for Rust code with `srcsearch`

Hamid Alavi Toussi

Github: [jslambda](https://github.com/jslambda)

---

<!-- .slide: class="lead" -->

# 1. Introduction / opening

## Where is multiline searching implemented?

You'd like to work on multiline search in `ripgrep` project.

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
| exact text / regular expression | analyzed (processed) query |
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
...
```

13 matching lines across 6 files in the pinned repository.

<!--note: the tests results are generic and does not point to the specific implementation of multisearch-->

---

# Ask for entities instead of lines

```console
$ srcsearch search \
    --index-dir .srcsearch \
    --query 'multiline AND search'

crates/core/flags/defs.rs:4503:1: Flag for Multiline
crates/searcher/src/searcher/glue.rs:149:1: MultiLine < 's , M , S >
README.md:211:1: Feature comparison
crates/searcher/src/searcher/mod.rs:627:1: Searcher
crates/core/flags/defs.rs:4591:1: Flag for MultilineDotall
crates/regex/src/literal.rs:11:1: InnerLiterals
crates/core/flags/defs.rs:6949:1: Flag for StopOnNonmatch
crates/core/flags/defs.rs:4251:1: Flag for MaxCount
crates/core/flags/defs.rs:1326:1: Flag for Count
crates/core/flags/hiargs.rs:26:1: HiArgs
```

The first result is `Flag for Multiline`; other high-ranking entities include
`MultiLine<'s, M, S>`, `Searcher`, and `MultilineDotall`.

The result gives us useful identifiers and locations for the next search.

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

---

# The record collects evidence from different fields.

`multiline` and `search` belong to one entity, but need not occur on one line.

## Why the first result ranks

```text
crates/core/flags/defs.rs:4503:1
Flag for Multiline                         score: 18.597

term        field          score      contribution
search      code            1.734          9.3%
multiline   signature      16.862         90.7%
```


<!--No single matching line has to contain the whole clue.-->

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

```

Every record is converted to a Tantivy document:

```text
Tantivy documents ───► Tantivy index
```

<!--Parsing supplies structure that plain text does not contain.-->

---

# Rust source → entity record

`rust2json` extracts an entity and its source location.

```text
SearchRecord::RustIndexEntry

kind            impl
name            Flag for Multiline
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
    ├── searchable:     title · name · qualified_name · signature
    │                   body_text · doc · code
    └── identification: record_type · file_path · kind
                        line_start · line_end  
                        (used to identify and present a result)
    │
    ▼
Tantivy document ──analyze fields──► inverted index
```

<!-- Implementation TODO: Make fields used only to identify or present results
stored-only instead of indexed. Keep file_path indexed because incremental updates
delete documents by path; searchable fields must also remain indexed. -->

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

 - Analysis (text processing)
 - Inverted index 
 - TF-IDF (BM25)

---

# Analysis: what counts as a term?

| Fields | Analysis | Example |
| --- | --- | --- |
| `title`, `body_text`, `doc` | tokenize, lowercase, English stem | `Searching searched` → `search`, `search` |
| `signature`, `code` | tokenize, lowercase | `MultiLine` → `multiline` |
<!--| `name`, `qualified_name` | exact whole value | `MultiLine` stays `MultiLine` 
it seems we do have tokenization for name.

srcsearch search -i .srcsearch --query '"Flag for Multiline"' --limit 1 --explain | grep -e context -C 5 --color
                  "description": "avgdl, average length of field"
                }
              ]
            }
          ],
          "context": [
            "Term=Term(field=name, type=Str, \"Flag for Multiline\")"
          ]
        }
      ]
    },

srcsearch search -i .srcsearch --query 'Flag  Multiline' --limit 1 --explain | grep -e context -C 5 --color
                  "description": "avgdl, average length of field"
                }
              ]
            }
          ],
          "context": [
            "Term=Term(field=name, type=Str, \"Multiline\")"
          ]
        }
      ]
    },

TODO impl: fine tune name analysis
--
|-->

The same analyzer is applied to the query for each field (at search time).

Stemming connects word forms. It does not add synonyms: `lookup` does not become `search`.

---

# The inverted index

Without an index, every query would have to read and analyze every record.
An inverted index does that work once, when the project is indexed, and maps each
**field + term** to a list of matching records:

```text
(signature, multiline) ──► A:1, B:1
(doc,       search)    ──► A:3, C:2
(code,      search)    ──► B:4
```

At search time, Tantivy looks up only the query terms and combines their short
postings lists. 
<!--It can find candidate records without scanning unrelated content;
the stored frequencies and positions also support ranking and phrase queries.-->

---

# Back to our search example!

Let $C(q)$ be the set of eligible records for query $q$.

Let $P(t)$ be the set of records containing term $t$ in any searched field.

For our query:

$$
C(\texttt{multiline AND search})
= P(\texttt{multiline}) \cap P(\texttt{search})
= \{A,B\}
$$

The eligible records are ranked by using BM25 algorithm.

<!--```python
def rank(record):
    score = 0.0

    for (field, term), frequencies in postings.items():
        if term not in query_terms or record not in frequencies:
            continue

        score += field_boosts[field] * bm25(
            tf=frequencies[record],
            document_frequency=len(frequencies),
        )

    return score


ranking = sorted(
    ((record, rank(record)) for record in candidates),
    key=lambda item: item[1],
    reverse=True,
)

```-->

---

# Retrieval first, ranking second

```text
analyzed query
      │
      ▼
Boolean operators
      │
      ▼
eligible records
      │
      ▼
BM25 ranking
      │
      ▼
ranked results
```

---

# TF-IDF: Ranking the results

In these two slides, a document $d$ means one searched field within a record,
such as its signature, documentation, or code.

$$
\operatorname{IDF}(t)
= \ln\\left(\frac{N}{\operatorname{df}(t)}\right)
$$

- $N$: total number of documents
- $\operatorname{df}(t)$: number of documents containing term $t$

$$
\operatorname{score}(d,Q)
= \sum_{t \in Q}
\operatorname{tf}(t,d) \cdot \operatorname{IDF}(t)
$$

1. More mentions add evidence.
2. Rare terms carry more information.

---

# IDF: Example

For 100 documents:

| Documents containing term | IDF |
| ---: | ---: |
| 5 | $\ln(100/5) \approx 3.00$ |
| 50 | $\ln(100/50) \approx 0.69$ |

---

# BM25: A more normalized TF-IDF

<!--For each field, Tantivy uses a smoothed IDF (boost rare terms even more):-->

$$
\operatorname{IDF}_{\mathrm{BM25}}(t)
= \ln\\left(1+\frac{N-\operatorname{df}(t)+0.5}
{\operatorname{df}(t)+0.5}\right)
$$

$$
\operatorname{BM25}(d,Q)
= \sum_{t \in Q}\operatorname{IDF}_{\mathrm{BM25}}(t)
\frac{\operatorname{tf}(t,d)(k_1+1)}
{\operatorname{tf}(t,d)+k_1\left(1-b+b\frac{L_d}{\overline L}\right)}
$$

- $L_d$: length of $d$ 
- $\overline L$: average length of the documents 

BM25 adds two practical ideas:

- **term-frequency saturation:** the tenth mention adds less than the first 
       <!--(tf in denominator)-->
- **length normalization:** a match in a short document is stronger evidence than a match in a very long document

`srcsearch` uses Tantivy's $k_1=1.2$ and $b=0.75$.

---

<!--# One worked comparison

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
-->


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

<!--Each field has its own term and length statistics.-->

---

# Back to the motivating example

Why does `Flag for Multiline` rank first?

- the complete entity satisfies both Boolean clauses
- `multiline` appears in its signature
- `search` appears in its code
- the rare, focused signature match dominates the score

In the example query `multiline AND search`, the term `multiline` tells us more than the common term `search`.

<!--BM25 ranks **lexical evidence**. It does not measure whether the result truly answers the question.-->

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

## Three repository exploration tasks

Local searches on a pinned `ripgrep` checkout, using fixed first queries.
`srcsearch` returns at most 10 results per task.

This measures retrieval; agent answer quality and tool-call counts were not tested.

---

# Commands for the three tasks

Run from the `ripgrep` root, with `srcsearch` on PATH and `$bench_index`
pointing to its prebuilt index (absolute path).

```sh
# 1. Multiline strategy selection and execution
rg -n -i 'multiline.*search' .
srcsearch search -i "$bench_index" -q 'multiline AND search' -l 10

# 2. Guide to searching only Rust files
rg -n -i 'search.*file' -g '*.md' .
srcsearch search -i "$bench_index" -s doc -q 'search for file' -l 10

# 3. Error construction and suggestion handling
rg -n -F 'unrecognized flag --' .
srcsearch search -i "$bench_index" -q 'unrecognized flag' -l 10
```

---

# Tasks and observed results

| Task | `rg` | `srcsearch` |
| --- | --- | --- |
| Find multiline strategy selection and execution | 13 lines; neither implementation found | `MultiLine` rank 2; `Searcher` rank 4 |
| Find the guide to searching files (search by using type option is the main target) | 51 results; some are relevant but they are hard to find | Relevant guide section rank 4 |
| Find `unrecognized flag --` construction and suggestion handling | 1 line; error construction found | Construction absent from top 10; `suggest()` rank 1 |

Results count when they point into the relevant entity or documentation section.

---

# What this suggests for agents

- Use `srcsearch` to discover likely implementation units and documentation.
- Use `rg` for known strings and exact occurrences.
- Inspect the returned source before drawing conclusions.

[Benchmark: queries, locations, and measurements](../benchmarks/ripgrep-codex.md)

---

<!-- .slide: class="lead" -->

## Questions?

[`github.com/jslambda/srcsearch`](https://github.com/jslambda/srcsearch)

---

# Future work
 - Python (already implemented), JavaScript, Java
 - Semantic search (specially useful for searching documentation and comments)
 
# References

- [`srcsearch` repository and README](https://github.com/jslambda/srcsearch)
- [Tantivy 0.25.0 BM25 implementation](https://docs.rs/tantivy/0.25.0/src/tantivy/query/bm25.rs.html)
- [Tantivy query parser](https://docs.rs/tantivy/0.25.0/tantivy/query/struct.QueryParser.html)
- [`rust-indexer`](https://github.com/jslambda/rust-indexer)
- [`markdown-indexer`](https://github.com/jslambda/markdown-indexer)
