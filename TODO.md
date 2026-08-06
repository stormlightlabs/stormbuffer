# To-Dos

## Milestone 0: Usable shell and living docs

### SB-001 — Establish the Rust workspace and shared conventions

Made the existing crates build as one workspace with a small,
clear dependency surface and common error, tracing, and test conventions.

### SB-002 — Ship the complete CLI command shell

Defined the full public command tree in Clap so users can
discover Stormbuffer immediately. Implement `--help`, `--version`, `init`,
`root`, and `status`; make every unfinished command a safe and explicit stub.

### SB-003 — Apply the CLI output, color, and alias contract

Added consistent stdout/stderr behavior, stable exit statuses, and
`owo-colors` styling that respects terminals, `NO_COLOR`, and
`--color auto|always|never`. Make `stormbuffer`, `stormbuf`, and `sbuf`
equivalent entry points.

### SB-004 — Generate man pages and shell completions

Generated man pages with `clap_mangen` and supported shell completions
with `clap_complete` from the runtime command definition.

### SB-005 — Build the static documentation foundation

Replaced the Svelte starter with a static, mdsvex-based docs
site using typed frontmatter, Docusaurus-like navigation, Pagefind, and the
specified typography.

### SB-006 — Keep documentation current

Made docs part of the definition of done, exercise documented
CLI examples, and verify that build.rs writes root-level operator artifacts from
the shared Clap command tree.

## Milestone 1: Canonical Markdown store

### SB-101 — Implement the record model and TOML frontmatter codec

Added typed IDs, enums, source records, timestamps, and a loss-conscious
Markdown/TOML parser and renderer for the canonical schema.

### SB-102 — Resolve global and project stores safely

Used platform directories for global data/cache locations and discover
`.sbuf/` project configuration with private defaults.

### SB-103 — Add atomic repository operations and lifecycle commands

Implemented locked, atomic creation and updates plus add, edit, show,
list, supersede, archive, restore, and guarded permanent deletion.

## Milestone 2: Rebuildable lexical index

### SB-201 — Build the SQLite projection and migrations

Added versioned migrations for scopes, records, chunks, sources, index metadata,
and contentless-delete FTS5, with SQLite treated only as cache.

### SB-202 — Implement deterministic chunking and incremental sync

Scanned, validate, hash, chunk, and project changed records in per-file
transactions while removing stale rows.

### SB-203 — Deliver FTS search and lexical context output

Implemented scoped FTS5 search with phrase, prefix, and exact title/alias
behavior, then expose it through `search` and bounded `context`.

### SB-204 — Add watch, reindex, and doctor recovery flows

Implemented convenience watching, explicit full rebuild, and diagnostics
for canonical/index/model/config inconsistencies.

## Milestone 3: Semantic and hybrid retrieval

### SB-301 — Add verified local embedding models

Implement the `Embedder` boundary, model manifest and checksum verification,
tokenizer/pooling/normalization behavior, and explicit model setup for local CPU inference.

### SB-302 — Add versioned vector indexes

Implemented the narrow `VectorIndex` boundary over pinned `sqlite-vec`,
including filtered search and non-destructive model migrations.

### SB-303 — Fuse lexical and semantic retrieval

Retrieved lexical and vector candidates, combine them with reciprocal-rank
fusion, collapse chunks, apply documented deterministic boosts, and compile bounded context.

### SB-304 — Establish retrieval evaluations and release thresholds

Checked in a representative corpus and query set and compare FTS-only,
vector-only, and hybrid behavior with reported metrics.

## Milestone 4: Grounded RAG and agent workflow

### SB-401 — Define the provider-neutral RAG context contract

Made `context` return bounded, ordered evidence blocks and a
receipt that any host model can consume without giving Stormbuffer responsibility
for generation or remote model access.

### SB-402 — Evaluate grounded answers and citations

Extended the retrieval corpus into a RAG question suite with
inspectable supporting records, expected claims or abstention, and a repeatable
adapter for evaluating a configured host model.

### SB-403 — Implement candidate review and provenance policy

Added propose, approve, and reject flows with source validation,
duplicate/conflict checks, explicit supersession, and narrowly scoped direct
activation permissions.

### SB-404 — Publish the versioned JSON invocation protocol

Exposed search, context, get, propose, supersede, and archive
through `invoke` with stable envelopes and errors.

### SB-405 — Complete portable import, export, and garbage collection

Added lossless export/import with collision handling and safe cleanup of
disposable cache/model artifacts.

### SB-406 — Dogfood Stormbuffer with a shared project store

Initialized this repository as the reference shared-store
example and curate a small memory set that helps agents work on Stormbuffer.

## Milestone 5: MCP and releases

### SB-501 — Implement the thin MCP adapter

Mapped the approved resources and tools to core operations over
stdio without duplicating storage, ranking, or policy.

### SB-502 — Add the behavioral agent skill

Documented when agents should search, compile context, propose
durable memory, report conflicts, and avoid storing unsuitable material.

### SB-503 — Harden packaging and releases

**What to build:** Produce cross-platform releases with all executable names,
verified model setup, man pages, completions, docs, and a clean-install smoke
test.

**Blocked by:** SB-003, SB-004, SB-006, SB-304, SB-402, SB-405, SB-406, SB-501

**Acceptance criteria:**

- [x] Supported platform artifacts prove all three CLI names work.
- [x] Package uninstall does not delete canonical user data.
- [x] Release checks cover generated artifacts, licenses, checksums, and docs.
- [x] A clean offline/online install matrix documents model behavior.
- [x] Upgrade and rollback paths preserve canonical records.

**Verification:** Run the release smoke test in clean supported environments.

**Milestone exit:** Stormbuffer is installable, documented, and interoperable
through CLI, JSON, and MCP on supported platforms.

## Milestone 6: Local web editor and graph

### SB-601 — Define the local server API and security boundary

**What to build:** Expose core browse/search/lifecycle operations through a small
HTTP API with loopback-only defaults, concurrency protection, and clean process
behavior suitable for service managers.

**Blocked by:** SB-403, SB-405, SB-503

**Acceptance criteria:**

- [ ] The server calls core APIs and never edits Markdown/SQLite directly.
- [ ] Default binding is loopback-only; remote binding is unavailable until an
      explicit authentication and threat-model decision is implemented.
- [ ] Edits use revision/ETag-style preconditions and report external changes.
- [ ] The foreground process logs to stderr, handles signals, and shuts down
      without corrupting writes.
- [ ] API and operator documentation match the implementation.

**Verification:** Run API integration tests for binding, concurrency conflicts,
signals, validation, and lifecycle parity.

### SB-602 — Build the human web editor

**What to build:** Add an accessible responsive app for listing, searching,
viewing, creating, editing, approving, superseding, archiving, and restoring
memories without the CLI.

**Blocked by:** SB-601

**Acceptance criteria:**

- [ ] Every supported lifecycle action uses the server/core validation and shows
      actionable errors.
- [ ] Candidate, active, superseded, and archived states are visually and
      textually distinct.
- [ ] Unsaved and concurrent changes cannot be discarded silently.
- [ ] Core flows work with keyboard and screen reader semantics at narrow and
      wide layouts.
- [ ] Browser tests cover search, edit, approval, conflict, archive, and restore.

**Verification:** Run web checks and browser tests, then perform a keyboard and
screen-reader smoke review.

### SB-603 — Add the explicit-relation graph

**What to build:** Visualize selected memories and their explicit supersession,
scope, source, and shared-tag relationships in an Obsidian-style graph linked to
the editor.

**Blocked by:** SB-602

**Acceptance criteria:**

- [ ] Graph nodes and edges can always explain which stored field created them.
- [ ] Filters constrain scope, kind, status, relationship, and result size.
- [ ] Selecting a node opens its record and editor navigation can focus it in
      the graph.
- [ ] Large stores use bounded queries and remain responsive at a documented
      target size.
- [ ] A non-canvas/list representation exposes the same relationships for
      keyboard and assistive-technology users.
- [ ] The app makes no inferred entity or importance claims.

**Verification:** Run graph data contract and browser interaction tests, then
review readability on representative small and target-size stores.

### SB-604 — Package and document daemonized operation

**What to build:** Document and test foreground server operation under common
service managers without adding a custom daemon supervisor.

**Blocked by:** SB-601, SB-602, SB-603

**Acceptance criteria:**

- [ ] Example user-service configurations use explicit store/bind settings and
      safe restart/shutdown behavior.
- [ ] Logs work with the service manager and contain no record bodies by default.
- [ ] Install, upgrade, backup, and recovery instructions are complete.
- [ ] The web app and CLI observe the same changes and policies in an end-to-end
      parity test.

**Verification:** Run the documented foreground smoke test and at least one
service-manager integration check on a supported platform.

**Milestone exit:** A person can safely run Stormbuffer as a local service, edit
memory without the CLI, and inspect explicit relationships in an accessible graph.
