# Stormbuffer roadmap

Stormbuffer is a local-first memory store for people and AI agents. It stores
sourced project knowledge as readable files under human control.

This roadmap covers work planned beyond version 0.1.0. See
[CHANGELOG.md](CHANGELOG.md) for completed work, the
[documentation site](docs/src/content/docs/) for current behavior, and
[TODO.md](TODO.md) for implementation details and dependencies.

## Local web editor

The next milestone adds a local web app for people who prefer not to use the
CLI. It will support browsing, search, editing, review, and lifecycle controls.
A graph may show relationships stored in record fields, including supersession,
scope, sources, and shared tags. It will not infer an opaque knowledge graph.

The server will bind to loopback only. Remote access requires authentication
and a threat model. The CLI and web app will use the same core policy.

## Later work

Use measured retrieval and maintenance failures to choose later work. Possible
changes include better ranking or reviewed suggestions for merging duplicates,
superseding stale memories, and archiving unused material. Stormbuffer should
present these changes for review instead of rewriting canonical memory.

## Outside the roadmap

Stormbuffer is not pursuing a hosted user-profile service, a full agent runtime,
raw conversation storage, broad connector catalogs, or autonomous edits to
canonical memory. New ranking stages and inferred relationships require a
measured failure they can solve.

## Decisions to validate

- How much evidence `remember` and `update` need in the common MCP call.
- Whether receipt feedback provides enough signal when it stores no user
  content.
