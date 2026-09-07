
# Talk Proposal

- Title of talk: Beyond grep: structure-aware search for Rust code with `srcsearch`
- Name of speaker: Hamid Alavi Toussi
- Duration of talk excluding Q&A: 30m
- Would you like to answer questions after the talk?: Yes
- Link to speaker profile, like GitHub, LinkedIn, etc.:
  https://www.linkedin.com/in/hamid-alavi-toussi-553751196/
  https://github.com/jslambda
  https://github.com/jslambda/srcsearch
  https://crates.io/crates/srcsearch


## Description of talk

`ripgrep` is excellent when you know the text or regular expression you are looking for. But sometimes the question is less precise: *Where is multiline searching implemented?* or *Where does this project document how files are searched?*

[`srcsearch`](https://github.com/jslambda/srcsearch) is a lightweight search engine for source code and project documentation that I wrote in Rust. It parses Rust source files and Markdown documentation and indexes them using Tantivy, with BM25-based relevance ranking.

The key difference from grep-like tools is the **unit being searched**.

A line-oriented search tool matches individual lines. `srcsearch` instead indexes meaningful structures: Rust entities such as functions, structs, and implementations, as well as Markdown sections. Information belonging to an entity is kept together in fields such as its name, signature, documentation, and source code.

This means a query can match information distributed across an entire Rust entity. For example, one query term might occur in a type's declaration while another occurs in its documentation or methods. `srcsearch` can combine those signals and rank the entity as one result, even when no individual source line contains the complete query.

A more detailed explanation and some examples are available in the [`srcsearch` README](https://github.com/jslambda/srcsearch/blob/main/README.md#why-use-srcsearch-alongside-ripgrepgrep).

This is also useful for **coding agents**. An agent exploring an unfamiliar repository often does not know the exact identifier or string to search for. Instead, it may start with a higher-level task such as *find the code responsible for filtering files* or *locate the documentation for the search configuration*. Returning a small ranked set of relevant functions, types, implementations, and documentation sections can give the agent better starting points than a large list of matching lines. Because `srcsearch` runs locally and exposes structured results, it can also be used as a retrieval component inside agentic developer tools.

In the talk, I'll give an overview of search concepts such as indexing, BM25 ranking, and semantic search. I'll then use the `ripgrep` codebase itself as an example to compare line-oriented search with ranked, entity-oriented search. We'll see cases where `rg` is the right tool for exhaustive textual matching, and cases where `srcsearch` provides a useful way to explore an unfamiliar codebase.

I'll also cover:

* search concepts: indexing, BM25, ranking, and relevance
* how Rust source and Markdown are turned into searchable records
* how Tantivy and BM25 rank those records
* how this style of retrieval can be useful for coding agents
* semantic search as a possible next step

The goal isn't to replace `ripgrep`. A useful workflow is to use `srcsearch` to discover likely implementation units or documentation sections, and then use `ripgrep` for precise and exhaustive inspection. The same pattern can work for coding agents: use ranked structural search for discovery, then more precise tools for inspection and modification.

Along the way, I'll discuss what worked, what did not, and the limitations of BM25-based lexical search compared with semantic/vector search.

## Speaker bio

Hamid Alavi Toussi is a software engineer at Elsevier. He is interested in search and information retrieval, data management, and developer tooling, among other things.
