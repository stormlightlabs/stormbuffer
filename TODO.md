# To-Dos

## Milestone 0: Usable shell and living docs

### SB-001 — Establish the Rust workspace and shared conventions

Built the crates as one workspace with shared error, tracing, and test
conventions.

### SB-002 — Ship the CLI command shell

Defined the public command tree in Clap and implemented `--help`, `--version`,
`init`, `root`, and `status`. Unfinished commands report that they are not
implemented and make no changes.

### SB-003 — Apply the CLI output, color, and alias contract

Added consistent stdout/stderr behavior, documented exit statuses, and
`owo-colors` styling that respects terminals, `NO_COLOR`, and
`--color auto|always|never`. Made `stormbuffer`, `stormbuf`, and `sbuf`
equivalent entry points.

### SB-004 — Generate man pages and shell completions

Generated man pages with `clap_mangen` and supported shell completions
with `clap_complete` from the runtime command definition.

### SB-005 — Build the static documentation foundation

Replaced the Svelte starter with a static, mdsvex-based docs
site using typed frontmatter, Docusaurus-like navigation, Pagefind, and the
specified typography.

### SB-006 — Integrate documentation into public changes

Made docs part of the definition of done, exercised documented CLI examples,
and verified that build.rs writes root-level operator artifacts from the Clap
command tree.

## Milestone 1: Canonical Markdown store

### SB-101 — Implement the record model and TOML frontmatter codec

Added typed IDs, enums, source records, timestamps, and a loss-conscious
Markdown/TOML parser and renderer for the canonical schema.

### SB-102 — Resolve global and project stores

Resolved global data and cache locations through platform directories and
project stores through the nearest `.sbuf/` configuration.

### SB-103 — Add atomic repository operations and lifecycle commands

Implemented locked, atomic creation and updates plus add, edit, show,
list, supersede, archive, restore, and guarded permanent deletion.

## Milestone 2: Rebuildable lexical index

### SB-201 — Build the SQLite projection and migrations

Added versioned migrations for scopes, records, chunks, sources, index metadata,
and contentless-delete FTS5, with SQLite treated only as cache.

### SB-202 — Implement deterministic chunking and incremental sync

Scanned, validated, hashed, chunked, and projected changed records in per-file
transactions while removing stale rows.

### SB-203 — Deliver FTS search and lexical context output

Implemented scoped FTS5 search with phrase, prefix, and exact title/alias
behavior, then exposed it through `search` and budgeted `context`.

### SB-204 — Add watch, reindex, and doctor recovery flows

Implemented convenience watching, full rebuild, and diagnostics
for canonical/index/model/config inconsistencies.

## Milestone 3: Semantic and hybrid retrieval

### SB-301 — Add verified local embedding models

Implemented the `Embedder` boundary, model manifest and checksum verification,
tokenizer/pooling/normalization behavior, and model setup for local CPU inference.

### SB-302 — Add versioned vector indexes

Implemented the narrow `VectorIndex` boundary over pinned `sqlite-vec`,
including filtered search and non-destructive model migrations.

### SB-303 — Fuse lexical and semantic retrieval

Retrieved lexical and vector candidates, combined them with reciprocal-rank
fusion, collapsed chunks, applied documented deterministic boosts, and compiled
context to a caller-provided budget.

### SB-304 — Establish retrieval evaluations and release thresholds

Checked in a representative corpus and query set and compared FTS-only,
vector-only, and hybrid behavior with reported metrics.

## Milestone 4: Grounded RAG and agent workflow

### SB-401 — Define the provider-neutral RAG context contract

Made `context` return ordered evidence blocks and a receipt within a caller-provided
budget. Host models consume this contract. Stormbuffer does not handle generation
or remote model access.

### SB-402 — Evaluate grounded answers and citations

Extended the retrieval corpus into a RAG question suite with
inspectable supporting records, expected claims or abstention, and a repeatable
adapter for evaluating a configured host model.

### SB-403 — Implement candidate review and provenance policy

Added propose, approve, and reject flows with source validation,
duplicate/conflict checks, supersession links, and direct activation only for
permitted callers.

### SB-404 — Publish the versioned JSON invocation protocol

Exposed search, context, get, propose, supersede, and archive
through `invoke` with versioned envelopes and error codes.

### SB-405 — Add import, export, and garbage collection

Added lossless export/import with collision handling and cleanup limited to
disposable cache and model artifacts.

### SB-406 — Dogfood Stormbuffer with a shared project store

Initialized this repository as the reference shared-store
example and curated memories that help agents work on Stormbuffer.

## Milestone 5: Agent capture and recall

### SB-501 — Add intent-level memory mutations

Added compact remember and update requests to the versioned JSON protocol.
Remember records one source and reports validation, duplicate, conflict, and
approval outcomes. Update preserves the active record while creating a linked
replacement candidate; approval atomically activates it and supersedes the old
record.

### SB-502 — Replace the MCP surface with intent-level memory tools

Replaced the MCP tool list with `memory_recall`, `memory_get`,
`memory_remember`, `memory_update`, and `memory_forget`. Recall uses budgeted
core context, writes retain core review and scope policy, and forget only
archives. Protocol tests cover all five mappings and the documented stdio host
configuration.

### SB-503 — Add capture checks to the agent skill

**What to build:** Teach the agent skill to check for durable memory after user
corrections, accepted decisions, surprising confirmed root causes, undocumented
constraints, resumable project state, and discoveries of stale memory.

**Blocked by:** SB-501, SB-602

**Acceptance criteria:**

- [ ] Each high-signal event produces at most one proposal with attributable
      evidence, a future trigger, a future behavior change, and a correction
      path.
- [ ] The skill rejects routine success, generic knowledge, and material owned
      by authoritative project documentation.
- [ ] Fleeting working state is rejected. A sourced project checkpoint is
      allowed when another session needs it to resume work.

**Verification:** Run the skill examples against accepted and rejected capture
cases.

### SB-504 — Validate project-scoped continuity

**What to build:** Dogfood project-scoped checkpoints across sessions and record
any discovery or presentation gap before designing a separate brief primitive.

**Blocked by:** SB-406, SB-503

**Acceptance criteria:**

- [ ] A later session can find and cite the checkpoint needed to resume work.
- [ ] Checkpoints omit transient detail that does not affect later work.
- [ ] Any failed handoff records whether capture, retrieval, or presentation
      caused the failure.
- [ ] A separate brief primitive is proposed only when the observed gap cannot
      be solved through checkpoints and recall.

**Verification:** Complete a cross-session dogfood scenario and inspect the
captured sources, retrieval result, and resumed work.

### SB-505 — Record disposable receipt feedback

**What to build:** Record whether retrieved evidence was included, cited,
ignored, or corrected and whether the answer led to an approved, edited,
rejected, or superseding proposal.

**Blocked by:** SB-402

**Acceptance criteria:**

- [ ] Feedback joins to retrieval receipts without storing raw prompts or
      transcripts.
- [ ] Feedback is stored in a disposable projection.
- [ ] Checked-in evaluation judgments remain readable and reviewable.
- [ ] Tests cover each evidence and proposal outcome.

**Verification:** Run the receipt feedback tests and inspect one rebuilt
projection from the checked-in judgments.

### SB-506 — Measure memory usefulness

**What to build:** Join aggregate receipt feedback to the offline corpus so
evaluations distinguish absent memory from failed retrieval and report whether
retrieved memory affected later work.

**Blocked by:** SB-304, SB-505

**Acceptance criteria:**

- [ ] Reports distinguish missing memory from retrieval failure.
- [ ] Reports include retrieved-and-used rate, stale corrections, context cost
      per used memory, and time to later reuse.
- [ ] Evaluation output contains no raw prompts or transcripts.

**Verification:** Run the retrieval evaluation corpus with and without receipt
feedback and compare the reported metrics.

**Milestone exit:** Agents can capture high-signal memory through the intent API,
resume project work from sourced checkpoints, and measure whether recalled
memory was used.

## Milestone 6: MCP and releases

### SB-601 — Implement the MCP adapter

Mapped the approved resources and tools to core operations over
stdio without duplicating storage, ranking, or policy.

### SB-602 — Add the behavioral agent skill

Documented when agents should search, compile context, propose
durable memory, report conflicts, and avoid storing unsuitable material.

### SB-603 — Harden packaging and releases

**What to build:** Produce cross-platform releases with all executable names,
verified model setup, man pages, completions, docs, and an installation smoke
test on a machine without Stormbuffer.

**Blocked by:** SB-003, SB-004, SB-006, SB-304, SB-402, SB-405, SB-406, SB-601

**Acceptance criteria:**

- [x] Supported platform artifacts prove all three CLI names work.
- [x] Package uninstall does not delete canonical user data.
- [x] Release checks cover generated artifacts, licenses, checksums, and docs.
- [x] An offline/online install matrix documents model behavior.
- [x] Upgrade and rollback paths preserve canonical records.

**Verification:** Run the release smoke test in each supported environment.

**Milestone exit:** Stormbuffer is installable, documented, and interoperable
through CLI, JSON, and MCP on supported platforms.

## Milestone 7: Local web editor and graph

### SB-701 — Define the local server API and security boundary

**What to build:** Expose core browse/search/lifecycle operations through an
HTTP API that binds to loopback, protects concurrent writes, handles signals,
and shuts down under a service manager.

**Blocked by:** SB-403, SB-405, SB-603

**Acceptance criteria:**

- [ ] The server calls core APIs and never edits Markdown/SQLite directly.
- [ ] Default binding is loopback-only. Remote binding is unavailable until an
      authentication and a threat model are implemented.
- [ ] Edits use revision/ETag-style preconditions and report external changes.
- [ ] The foreground process logs to stderr, handles signals, and shuts down
      without corrupting writes.
- [ ] API and operator documentation match the implementation.

**Verification:** Run API integration tests for binding, concurrency conflicts,
signals, validation, and lifecycle parity.

### SB-702 — Build the human web editor

**What to build:** Add an accessible responsive app for listing, searching,
viewing, creating, editing, approving, superseding, archiving, and restoring
memories without the CLI.

**Blocked by:** SB-701

**Acceptance criteria:**

- [ ] Every supported lifecycle action uses core validation and reports
      validation or conflict errors with recovery instructions.
- [ ] Candidate, active, superseded, and archived states are visually and
      textually distinct.
- [ ] Unsaved and concurrent changes cannot be discarded silently.
- [ ] Core flows work with keyboard and screen reader semantics at narrow and
      wide layouts.
- [ ] Browser tests cover search, edit, approval, conflict, archive, and restore.

**Verification:** Run web checks and browser tests, then perform a keyboard and
screen-reader smoke review.

### SB-703 — Add the stored-relation graph

**What to build:** Visualize selected memories and their stored supersession,
scope, source, and shared-tag relationships in an Obsidian-style graph linked to
the editor.

**Blocked by:** SB-702

**Acceptance criteria:**

- [ ] Graph nodes and edges can always explain which stored field created them.
- [ ] Filters constrain scope, kind, status, relationship, and result size.
- [ ] Selecting a node opens its record and editor navigation can focus it in
      the graph.
- [ ] Queries enforce a documented result limit and remain responsive at the
      target store size.
- [ ] A non-canvas/list representation exposes the stored relationships for
      keyboard and assistive-technology users.
- [ ] The app makes no inferred entity or importance claims.

**Verification:** Run graph data contract and browser interaction tests, then
review readability with fixtures at the documented target size.

### SB-704 — Package and document daemonized operation

**What to build:** Document and test foreground server operation under common
service managers without adding a custom daemon supervisor.

**Blocked by:** SB-701, SB-702, SB-703

**Acceptance criteria:**

- [ ] Example user-service configurations name the store and bind address and
      preserve in-progress writes during restart and shutdown.
- [ ] Logs work with the service manager and contain no record bodies by default.
- [ ] Documentation covers installation, upgrade, backup, and recovery.
- [ ] An end-to-end test shows that the web app and CLI return matching records
      and enforce core lifecycle policy.

**Verification:** Run the documented foreground smoke test and at least one
service-manager integration check on a supported platform.

**Milestone exit:** A person can run Stormbuffer as a loopback-only service, edit
memory without the CLI, and inspect stored relationships in an accessible graph.
