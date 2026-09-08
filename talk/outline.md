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

- Open with a question from an unfamiliar repository: “Where is multiline searching
  implemented?”
- Point out that the intent is clear, but the identifier, file, and exact wording are
  unknown.
- Introduce the talk's question: what should a source-search tool return when we know
  the concept but not the text?
- State the thesis: the right result may be a complete code entity, not a matching
  line.

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

- **Indexing:** do parsing and text analysis ahead of time so queries can retrieve
  records efficiently.
- **Inverted index:** map analyzed terms to the records and fields that contain them.
- **Relevance ranking:** order plausible results instead of treating all matches as
  equally useful.
- **BM25 intuition:** reward informative term matches while accounting for term
  frequency, corpus rarity, and record length.
- **Fields:** allow matches in a name, signature, documentation, and code to contribute
  to the result as parts of one entity.
- Draw the boundary clearly: BM25 is lexical ranking. It can rank related word forms
  and distributed term matches, but it does not understand meaning like a semantic or
  vector search system.

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
