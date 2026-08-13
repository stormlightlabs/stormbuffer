# Changelog

All notable changes to Stormbuffer are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - Unreleased

### Added

- Added the `sbuf` command-line interface for creating, editing, listing,
  searching, superseding, archiving, restoring, and permanently deleting
  memories. Human-readable output respects terminal color settings and
  `NO_COLOR`; automation can use bounded JSON output and stable exit codes.
- Added canonical Markdown records with TOML frontmatter, typed identifiers,
  provenance, lifecycle states, validation, locking, and atomic writes.
- Added global stores and repository-local project stores. Stable project IDs
  survive directory renames, `--project` combines project and applicable global
  memory, and `--local` restricts access to the nearest project store.
- Added rebuildable SQLite, FTS5, and sqlite-vec projections with incremental
  synchronization, deterministic chunking, verified local embedding models,
  hybrid retrieval, and budgeted context assembly with citations and receipts.
- Added `watch`, `reindex`, `status`, and `doctor --repair` for inspecting and
  rebuilding disposable state without changing canonical records.
- Added candidate review and provenance checks. Agents can propose, remember,
  update, and archive memories, while people retain approval and permanent
  deletion authority. Candidate inbox and read-only audit commands help review
  unresolved or stale records and broken links.
- Added a versioned, non-interactive JSON invocation protocol for search,
  context, record lookup, and memory lifecycle operations.
- Added an MCP adapter with intent-level recall, lookup, remember, update, and
  forget tools that use the same core storage, retrieval, scope, and review
  rules as the CLI.
- Added offline-installable global and project agent skills with a tested
  decision tree for recall, capture, correction, and necessary handoffs.
- Added lossless import and export, import previews, export verification,
  disposable garbage collection, and guarded whole-store destruction with an
  optional recovery export.
- Added retrieval, grounded-answer, capture-policy, usefulness, and reviewed
  relation evaluations. Advisory local relation analysis runs in shadow mode
  and stores its results only in disposable projections.
- Added a static documentation site, generated man pages and shell completions,
  cross-platform release archives, checksums, license checks, and installation
  smoke tests.

### Changed

- Treat records with the same normalized title, kind, and scope but different
  bodies as possible overlaps for human review. Only normalized body equality
  is considered a deterministic duplicate.

[0.1.0]: https://github.com/stormlightlabs/stormbuffer/releases/tag/v0.1.0
