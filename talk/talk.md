<!--The outline follows proposal.md-->

# Beyond grep: structure-aware search for Rust code with `srcsearch`


## Central idea

The important difference between `ripgrep` and `srcsearch` is not simply how they
match text. It is the unit they search. `ripgrep` returns matching lines, while
`srcsearch` ranks complete Rust entities and Markdown sections. The tools are
complementary: use structural search to discover likely implementation units,
then use line-oriented search for precise and exhaustive inspection.

<!--## Audience takeaways

By the end of the talk, the audience should understand:

- why the choice of searchable unit changes the results a search tool can return;
- the roles of indexing, fields, BM25, and relevance ranking;
- how `srcsearch` turns Rust entities and Markdown sections into searchable records;
- when to choose `srcsearch`, `ripgrep`, or a combination of both;
- why lexical ranking is useful for coding agents, but is not semantic understanding.-->

## Outline

### 1. Motivating example: a search where the exact words are unknown 

- Put the audience in a concrete situation: you have just joined the `ripgrep`
  project and need to change how multiline searching works. Before editing anything,
  you must answer: 

  “Where is multiline searching implemented?”

- Point out what is missing: you do not know whether the relevant identifier is
  `Multiline`, `MultiLine`, or something else, which crate owns it, or what exact
  phrase appears in the source.
- Start with the natural line-oriented search:

  ```bash
  % rg --vimgrep 'multiline.*search'
  ###
  tests/multiline.rs:20:25:// Tests that even in a multiline search, a '.' does not match a newline.
  tests/multiline.rs:91:15:// Tests that multiline search works when reading from stdin. This is an
  tests/multiline.rs:92:27:// important test because multiline search must read the entire contents of
  tests/multiline.rs:103:14:// Test that multiline search and contextual matches work.
  CHANGELOG.md:524:58:  Fix bug where `\A` could produce unanchored matches in multiline search.
  tests/tests.rs:22:24:// Tests for ripgrep's multiline search support.
  README.md:220:1:multiline search and opt-in fancy regex support via PCRE2.
  FAQ.md:519:24:slower when performing multiline searches? Well, that's because there are
  FAQ.md:654:40:for line-by-line searching by enabling multiline search. After all, our
  FAQ.md:682:58:valid UTF-8 to PCRE2. Unfortunately, one key downside of multiline search is
  crates/core/flags/defs.rs:4541:38:default. This flag only applies when multiline search is enabled.
  crates/core/flags/defs.rs:4610:29:match line terminators when multiline searching is enabled. This flag has no
  crates/core/flags/defs.rs:4611:11:effect if multiline searching isn't enabled with the \flag{multiline} flag.
  ```
  <!--the results under tests are testing multiline search but the tests are generic, and does
  not point to the specific implementation we are after.-->
  It returns 13 matching lines across 6 files in the pinned repository, including
  prose containing phrases such as `multiline search` and `multiline searches`.
  Those matches are accurate, but the audience must still decide which complete
  code unit is the useful starting point.
- Then reveal the same question as a structural query:

  ```bash
  srcsearch search \
    --index-dir .srcsearch \
    --query 'multiline AND search'
  ```

  The ranked results begin with the `Multiline` flag entity, followed by entities
  such as `MultiLine`, `Searcher`, and `MultilineDotall`. In the first result,
  `multiline` comes from the declaration while `search` appears in its methods and
  documentation; **no single matching line has to contain the whole clue.**
- Introduce the talk's question: what should a source-search tool return when we know
  the concept but not the text?
- State the thesis: the right result may be a complete code entity, not a matching
  line.

```rust
impl Flag for Multiline {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'U')
    }
    fn name_long(&self) -> &'static str {
        "multiline"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-multiline")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Enable searching across multiple lines."
    }
    fn doc_long(&self) -> &'static str {
        r#"
This flag enables searching across multiple lines.
.sp
When multiline mode is enabled, ripgrep will lift the restriction that a

```

```
hamid@fast ripgrep % srcsearch search -i .srcsearch -q "search AND multiline" --explain --json | summarize.py
crates/core/flags/defs.rs:4503:1: Flag for Multiline (score: 18.597)
term         field          score percent  boost      base   freq      idf      dl     avgdl       n
search       code           1.734    9.3%    1.0     1.734    2.0    2.045   472.0   147.008   182.0
multiline    signature     16.862   90.7%    1.0    16.862    1.0    5.112     4.0     3.379     8.0
-----------------
crates/searcher/src/searcher/glue.rs:149:1: MultiLine < 's , M , S > (score: 10.925)
term         field          score percent  boost      base   freq      idf      dl     avgdl       n
search       code           1.967   18.0%    1.0     1.967    4.0    2.045   792.0   147.008   182.0
multiline    signature      8.958   82.0%    1.0     8.958    1.0    5.112    10.0     3.379     8.0
-----------------
```
<!--https://docs.rs/tantivy/latest/tantivy/query/struct.QueryParser.html
(title:multiline OR body:multiline OR docs:multiline)
AND
(title:search OR body:search OR docs:search)

term search penalized because of its high frequency in the corpus
-->
### 2. `ripgrep` solves a different problem well (2:00–5:00)

- Establish `ripgrep` as the baseline rather than the opponent.
- Show where line-oriented search excels:
  - exact strings and regular expressions;
  - exhaustive results;
  - immediate searching without building an index;
  - searching arbitrary file types.
- Show the friction during exploration:
  - a narrow expression can return nothing;
  - a broad expression can return many lines that the user must group and prioritize;
  - relevant words can appear in different parts of the same code entity without
    appearing together on one line.
- Transition: this is a retrieval and ranking problem, not a failure of grep.

### 3. Change the unit being searched (5:00–8:00)

- Contrast the searchable units:
  - `ripgrep`: a matching line and its surrounding context;
  - `srcsearch`: a Rust function, struct, enum, trait, implementation, or other entity;
  - `srcsearch`: a Markdown section beneath a heading.
- Explain that one Rust record keeps related information together in fields such as:
  - name;
  - signature;
  - documentation;
  - source code.
- Use a simple example: `multiline` can occur in a declaration while `search` occurs
  in documentation or methods associated with that entity. The complete entity can
  satisfy and rank for the query even when no single line does.
- Emphasize that parsing supplies the structure that plain text does not contain.

### 4. Search concepts in just enough depth (8:00–12:00)

Use four presentation beats: analysis, candidate retrieval, BM25, and field scoring.
The equations and numerical examples below also serve as speaker notes; show the
BM25 formula and one worked comparison during the four-minute slot.

#### Analysis: what counts as a term?

An index moves work ahead of query time: parse files into records, analyze each
searchable field, and store lookup structures. Here a **document** means one indexed
record (a Rust entity or Markdown section), not necessarily a whole file.

Analysis defines which text can match. `srcsearch` uses these field configurations:

| Fields | Analysis | Example |
| --- | --- | --- |
| `title`, `body_text`, `doc` | Split into alphanumeric tokens, lowercase, English stemming | `Searching searched` → `search`, `search` |
| `signature`, `code` | Default text analyzer: split into alphanumeric tokens, remove overlong tokens, lowercase; no stemming | `MultiLine` → `multiline`; `searching` stays `searching` |
| `name`, `qualified_name` | Raw whole-value indexing; no lowercasing or stemming | `MultiLine` stays `MultiLine` |

Queries are analyzed with the corresponding field's analyzer too. Stemming maps
inflected forms to a common stem; it is not synonym expansion (`lookup` does not
automatically become `search`). Lowercasing `MultiLine` makes `multiline`, but does
not split it into `multi` and `line`. Exact name fields therefore behave differently
from the tokenized declaration and documentation fields.

#### Inverted index: retrieve candidates before ranking them

An **inverted index** maps a field and term to a postings list of record IDs.
Tokenized text postings also store occurrence counts and positions, supporting
scoring and phrase queries. A hypothetical example:

```text
(signature, multiline) -> A: 1 occurrence, B: 1 occurrence
(doc, search)          -> A: 3 occurrences, C: 2 occurrences
(code, search)         -> B: 4 occurrences
```

Let \(P_f(t)\) be the set of records containing analyzed term \(t\) in field \(f\).
For these two words, an unqualified term can match any default search field:

$$
P(t) = \bigcup_{f \in F} P_f(t)
$$

Boolean operators determine eligibility:

$$
\begin{aligned}
C(\texttt{multiline AND search}) &= P(\texttt{multiline}) \cap P(\texttt{search})
                                  = \{A,B\} \\
C(\texttt{multiline OR search})  &= P(\texttt{multiline}) \cup P(\texttt{search})
                                  = \{A,B,C\}
\end{aligned}
$$

The two required terms can occur in different fields of the same record. `AND`
does not require the same line, field, or adjacent positions. `srcsearch` leaves
Tantivy's default operator as `OR`, so `multiline search` is not equivalent to
`multiline AND search`. Ranking orders the eligible records; a high score cannot
compensate for a missing required term.

#### From TF-IDF intuition to the actual BM25 formula

First consider one tokenized field. Define:

| Symbol | Meaning |
| --- | --- |
| \(d\), \(Q\) | A record and a set of distinct analyzed query terms |
| \(N\) | Number of indexed records |
| \(\operatorname{tf}(t,d)\) | Occurrences of term \(t\) in this field of record \(d\) |
| \(\operatorname{df}(t)\) | Number of records containing \(t\) in this field |
| \(L_d\), \(\overline L\) | Field length in tokens and average field length |

One simple TF-IDF scoring variant uses raw counts and an unnormalized sum:

$$
\operatorname{score}_{\mathrm{TF\text{-}IDF}}(d,Q)
= \sum_{t \in Q} \operatorname{tf}(t,d)
  \ln\!\left(\frac{N}{\operatorname{df}(t)}\right)
$$

This illustrates two signals: repeated mentions add evidence, and rare terms carry
more weight. For \(N=100\), a term in 5 records has IDF \(\ln(20)\approx3.00\),
while a term in 50 records has IDF \(\ln(2)\approx0.69\). Terms absent from the
index have no matching postings and contribute nothing. There are other TF-IDF
variants, including logarithmic TF and cosine normalization; this is an
introductory scoring model, not the formula `srcsearch` executes.

`srcsearch` uses Tantivy's BM25 term scoring. For a matching term:

$$
\operatorname{IDF}(t)
= \ln\!\left(1 + \frac{N-\operatorname{df}(t)+0.5}
                         {\operatorname{df}(t)+0.5}\right)
$$

$$
\operatorname{BM25}(d,Q)
= \sum_{t \in Q} \operatorname{IDF}(t)\,
  \frac{\operatorname{tf}(t,d)(k_1+1)}
       {\operatorname{tf}(t,d)+k_1\!\left(1-b+b\frac{L_d}{\overline L}\right)}
$$

The pinned Tantivy 0.25.0 uses \(k_1=1.2\) and \(b=0.75\). The logarithm is
natural; this smoothed IDF stays positive even for terms present in every record.

- **Saturation:** at average field length, the TF factor becomes
  \(2.2\operatorname{tf}/(\operatorname{tf}+1.2)\). For TF values 1, 3, and 10,
  it is approximately 1.00, 1.57, and 1.96; its limit is 2.2. Ten repetitions do
  not provide ten times the evidence. Larger \(k_1\) delays saturation.
- **Length normalization:** holding TF fixed, a longer field gets a smaller
  contribution. \(b=0\) disables length normalization; \(b=1\) applies the full
  field-length ratio inside the denominator. Length means tokens, not source lines.

For a concrete comparison, suppose `multiline` occurs in 5 of 100 records in one
field, and that field averages 100 tokens:

$$
\operatorname{IDF}(\texttt{multiline})
= \ln\!\left(1+\frac{95.5}{5.5}\right) \approx 2.91
$$

| Record | TF | Field length | TF/length factor | Term score, before boosts |
| --- | ---: | ---: | ---: | ---: |
| A: compact entity | 1 | 50 | \(2.2/(1+0.75)=1.257\) | 3.66 |
| B: long entity | 3 | 200 | \(6.6/(3+2.1)=1.294\) | 3.77 |

Three mentions in the longer entity only slightly outweigh one in the compact
entity. These are illustrative corpus statistics, not the measured demo scores.

#### Fields: combine several views of the same entity

For the simple Boolean term query above, matching term clauses contribute additive
scores with field boosts:

$$
\operatorname{score}(d,Q)
= \sum_{f\in F} w_f
  \sum_{t\in Q_f} \operatorname{IDF}_f(t)\,
  \frac{\operatorname{tf}_f(t,d)(k_1+1)}
       {\operatorname{tf}_f(t,d)+k_1\!\left(1-b+b\frac{L_{d,f}}{\overline L_f}\right)}
$$

Here \(Q_f\) contains the terms produced by that field's query analyzer; missing
matches contribute zero. Each field has its own document frequencies and lengths.
Tantivy computes \(\overline L_f\) as the field's total token count divided by
\(N\), including records without that field in the denominator. Thus Markdown
sections and Rust entities influence the corpus statistics together.

The current boosts, configured in `src/lib.rs`, are:

| Field | Boost \(w_f\) |
| --- | ---: |
| `title`, `name`, `qualified_name` | 4 |
| `signature`, `doc`, `body_text` | 2 |
| `code` | 1 |

A signature contribution of 3.66 becomes 7.32 after its boost. A `search` match in
the same record's code contributes an additional score calculated with the code
field's statistics. A term matching several fields can contribute several times.
This is a sum of boosted field scores, not BM25F's combination of field evidence
before saturation. Boosts express a preference, not a guarantee that every name
match outranks every code match.

For exact score reproduction, account for Tantivy's compressed field lengths and
floating-point arithmetic. The raw `name` and `qualified_name` fields do not store
term frequencies; their term scorers use frequency 1. Use `--explain` to inspect
the actual scoring tree; phrases and other query types need their own explanation
rather than assuming the simple term-sum formula applies unchanged.

```text
field-specific query analysis -> postings + Boolean eligibility
                             -> boosted BM25 contributions -> ranked records
```

Close with the boundary: these scores measure lexical evidence, not the probability
that a result answers the question. Stemming connects word forms, and structure
brings related text together, but neither supplies synonym matching or semantic
understanding. Scores depend on the query and corpus, so they are not universal
confidence values or directly comparable across unrelated searches.

Speaker references: the local `build_tantivy_schema`, `register_doc_text_analyzer`,
and `search_tantivy_index_with_explain` implementation in [src/lib.rs](../src/lib.rs),
plus Tantivy 0.25.0's
[BM25 implementation](https://docs.rs/tantivy/0.25.0/src/tantivy/query/bm25.rs.html)
and [query parser documentation](https://docs.rs/tantivy/0.25.0/tantivy/query/struct.QueryParser.html).

### 5. How `srcsearch` builds and searches the index (12:00–16:00)

- Walk through the pipeline:
  1. traverse the project;
  2. parse supported Rust and Markdown files;
  3. extract Rust entities and Markdown sections;
  4. store their content and metadata as fielded records in Tantivy;
  5. analyze the query and rank matching records with BM25;
  6. return a small set of results with file and source locations.
- Show one representative Rust record and one Markdown record.
- Explain why source locations remain important: ranked retrieval should lead back to
  the real code or documentation immediately.
- Briefly mention document-scoped search as a way to exclude code and signatures when
  the user is looking for explanations rather than implementations.

### 6. Demo: exploring the `ripgrep` repository (16:00–22:00)

#### Demo A: where is multiline searching implemented? (16:00–19:00)

- Run a line-oriented query:

  ```bash
  rg -n -i 'multiline.*search'
  ```

- Note what the output provides: exact matching lines, but no grouping or relevance
  ranking by Rust entity.
- Run the structural query:

  ```bash
  srcsearch search \
    --index-dir .srcsearch \
    --query 'multiline AND search'
  ```

- Inspect results such as `Multiline`, `MultiLine`, and `Searcher`.
- Explain why the `Multiline` flag can rank: the query signals are distributed across
  the declaration, methods, and documentation stored in one record.
- Follow up with `rg` using a newly discovered identifier to demonstrate the combined
  discovery-and-inspection workflow.

#### Demo B: find documentation about searching files (19:00–22:00)

- Start with the natural but narrow phrase:

  ```bash
  rg -n -i 'search for file'
  ```

- Broaden it and point out the cost of manually inspecting many results:

  ```bash
  rg -n -i 'search.*file'
  ```

- Ask for ranked documentation results instead:

  ```bash
  srcsearch search \
    --index-dir .srcsearch \
    --scope doc \
    --query 'search for file'
  ```

- Inspect likely starting points such as “Recursive search” and “Manual filtering:
  file types.”
- Reinforce the distinction between ranking useful starting points and exhaustively
  listing textual occurrences.

### 7. Retrieval for coding agents (22:00–25:00)

- Relate the opening problem to an agent entering an unfamiliar repository.
- An agent often receives a task-level description rather than an identifier or exact
  string.
- A small ranked set of complete functions, types, implementations, and documentation
  sections gives the agent useful context and vocabulary for its next search.
- Present a two-stage workflow:
  1. use ranked structural retrieval to discover likely entities and identifiers;
  2. use precise tools such as `ripgrep` to verify references and inspect every
     occurrence before modifying code.
- Note practical advantages: the index and queries run locally, and structured results
  can be consumed by developer tools.

### 8. What worked, what did not, and what comes next (25:00–28:00)

- What worked:
  - parsing creates useful retrieval boundaries;
  - fielded records preserve signals distributed across an entity;
  - BM25 provides a strong, lightweight ranking baseline;
  - Markdown sections make project documentation part of the same discovery workflow.
- Limitations:
  - relevance depends on the query and corpus;
  - the highest-ranked result is not necessarily the correct answer;
  - lexical search can match individually relevant words while missing the intended
    meaning;
  - building and updating an index adds a step that `ripgrep` does not require;
  - only supported languages and document structures receive structural treatment.
- Possible next step: semantic or hybrid search that combines lexical precision with
  embeddings, while retaining entity boundaries, metadata, and source locations.

### 9. Conclusion (28:00–30:00)

- Return to the opening question and summarize how entity-oriented retrieval provides
  a useful starting point without knowing the exact identifier.
- Give the decision rule:
  - use `ripgrep` when you know the text or need every occurrence;
  - use `srcsearch` when you need ranked Rust entities or documentation sections;
  - use both when exploring and then verifying an unfamiliar codebase.
- Close with the main message: better code search can come not only from a different
  matching algorithm, but from choosing a more meaningful unit to search.
- Invite questions.

## Demo preparation and fallback

- Index the pinned `ripgrep` revision before the talk and verify every command against
  that index.
- Keep terminal font size, result limit, and window layout optimized for projection.
- Save the expected output for each command as slides or screenshots in case the live
  demo fails.
- Avoid claiming that the `rg` and `srcsearch` queries are semantically identical;
  they illustrate different retrieval tasks.
- Leave surprising or imperfect rankings visible when useful—they provide a natural
  transition into the limitations of BM25.

## Q&A prompts

If the audience needs a starting point, invite questions about:

- index freshness and incremental updates;
- support for additional programming languages;
- field weighting and score explanations;
- index size and query performance;
- integrating local retrieval into coding-agent workflows;
- combining BM25 with semantic or vector search.
