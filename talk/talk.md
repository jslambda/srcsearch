<!--The outline follows proposal.md-->

# Beyond grep: structure-aware search for Rust code with `srcsearch`

## Talk details

- Speaker: Hamid Alavi Toussi
- Talk length: 30 minutes, excluding Q&A
- Format: explanation supported by a live demo of `ripgrep` and `srcsearch`
- Example codebase: `ripgrep`

## Central idea

The important difference between `ripgrep` and `srcsearch` is not simply how they
match text. It is the unit they search. `ripgrep` returns matching lines, while
`srcsearch` ranks complete Rust entities and Markdown sections. The tools are
complementary: use structural search to discover likely implementation units,
then use line-oriented search for precise and exhaustive inspection.

## Audience takeaways

By the end of the talk, the audience should understand:

- why the choice of searchable unit changes the results a search tool can return;
- the roles of indexing, fields, BM25, and relevance ranking;
- how `srcsearch` turns Rust entities and Markdown sections into searchable records;
- when to choose `srcsearch`, `ripgrep`, or a combination of both;
- why lexical ranking is useful for coding agents, but is not semantic understanding.

## Outline

### 1. Opening: a search where the exact words are unknown (0:00–2:00)

- Put the audience in a concrete situation: you have just joined the `ripgrep`
  project and need to change how multiline searching works. Before editing anything,
  you must answer: “Where is multiline searching implemented?”
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
  documentation; no single matching line has to contain the whole clue.
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

- Begin with the job of an index: spend time before the query parsing records and
  analyzing their text so that a search does not have to scan every byte in the
  repository. Analysis breaks text into terms, normalizes them, and—for selected
  documentation fields—stems related forms such as `search`, `searching`, and
  `searched` toward the same searchable term.
- Introduce an **inverted index** by reversing the usual view of the source. Instead
  of asking “which words are in this record?”, store “which records contain this
  word?” For a tiny corpus, the postings might look like:

  ```text
  multiline -> Multiline flag, MultiLine implementation, Feature comparison
  search    -> Multiline flag, MultiLine implementation, Searcher, ...
  ```

  Each posting also retains information such as the field and frequency of the term.
  Looking up the postings for `multiline` and `search` quickly produces candidate
  records; ranking decides which candidate to show first.
- Use **TF-IDF** to build the ranking intuition one piece at a time:
  - **Term frequency (TF):** a term appearing several times in a record is usually
    stronger evidence than a single incidental mention. A `Multiline` entity whose
    documentation repeatedly discusses searching is therefore more promising than
    a record that mentions it once.
  - **Inverse document frequency (IDF):** a term found in only a few records carries
    more information than a term found almost everywhere. In this corpus,
    `multiline` is likely more discriminating than the common term `search`.
  - The simplified mental model is:

    ```text
    score(record, query) = sum(TF(term, record) * IDF(term))
    IDF(term)             ≈ log(total records / records containing term)
    ```

    Do not calculate the score on stage; use the formula to tell the story: repeated
    matches help, rare terms help more, and evidence from all query terms is added
    into one relevance score.
- Connect that intuition to **BM25**, which `srcsearch` actually uses. BM25 keeps the
  useful TF-IDF idea that frequent and rare terms contribute differently, but makes
  two important refinements:
  - term-frequency saturation: the tenth repetition of `search` adds much less
    evidence than the first few repetitions;
  - length normalization: a match in a compact function or type is not automatically
    overwhelmed by a very long source or documentation record containing more words.
- Explain **fields** as several searchable views of the same entity. A Rust record
  has fields such as name, signature, documentation, and code. Their matches can be
  combined into one score, while field boosts can make a match in a descriptive name
  or signature more influential than the same term buried in a long body. This is
  why `multiline` in a declaration and `search` in its documentation can jointly
  retrieve the complete `Multiline` entity.
- Summarize the retrieval path:

  ```text
  query terms -> postings -> candidate records -> BM25 scores -> ranked results
  ```

- Draw the boundary clearly: TF-IDF and BM25 rank lexical evidence. Text analysis can
  connect word forms and an entity can combine terms from several fields, but neither
  method understands intent or meaning like a semantic or vector search system.

- Stemming

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
