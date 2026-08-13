# To-Dos

## Milestone 0: Usable shell and living docs

### SB-001 — Establish the Rust workspace and shared conventions

Built the crates as one workspace with shared error, tracing, and test
conventions.

### SB-002 — Ship the CLI command shell

Defined the public command tree in Clap and implemented `--help`, `--version`,
`init`, `root`, and `status`. Unfinished commands report that they are not
implemented and make no changes.

### SB-003 — Apply the CLI output and color contract

Added consistent stdout/stderr behavior, documented exit statuses, and
`owo-colors` styling that respects terminals, `NO_COLOR`, and
`--color auto|always|never`. Made `sbuf` the single public entry point.

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

### SB-503 — Add the agent memory decision tree

Added a five-outcome recall and capture tree to the canonical agent skill.
Capture evaluation now starts only after a named event, admits at most one
sourced candidate, and leaves lifecycle and approval policy to core. Contract
scenarios in the Rust integration tests cover capture, rejection, and ordinary
completion with no proposal.

### SB-504 — Validate project-scoped continuity

Validated project-scoped continuity through the public agent protocol. The
dogfood test captures a sourced checkpoint with completed work, unresolved
state, settled decisions, the next action, and references, then retrieves and
cites it from a separate process before resuming. A companion scenario confirms
that repository-preserved state needs no checkpoint. Neither handoff failed, so
no capture, retrieval, or presentation failure needed recording. The scenarios
exposed no repeatable gap that warrants a separate brief primitive.

### SB-505 — Record disposable receipt feedback

Added opaque IDs and timestamps to retrieval receipts. Checked-in JSON
judgments classify retrieved evidence as included, cited, ignored, or corrected
and resulting proposals as approved, edited, rejected, or superseding. The
rebuildable SQLite projection retains only receipt and record IDs, timestamps,
and outcomes; judgment parsing rejects raw prompts, answers, and transcripts.

### SB-506 — Measure memory usefulness

**What to build:** Join aggregate receipt feedback to the offline corpus so
evaluations distinguish absent memory, retrieval misses, ignored results, and
stale or incorrect memory. Report whether retrieved memory affected later work
and whether proposed memory survived human review.

**Blocked by:** SB-505

**Acceptance criteria:**

- [ ] Reports distinguish knowledge that was never captured, memory that
      retrieval missed, retrieved memory the agent ignored, and retrieved
      memory that was stale or incorrect.
- [ ] Reports include retrieved-and-used rate, stale corrections, context cost
      per used memory, and time to later reuse.
- [ ] Reports include proposal approval, edit, rejection, and duplicate rates.
- [ ] Evaluation output contains no raw prompts or transcripts.

**Verification:** Run the retrieval evaluation corpus with and without receipt
feedback and compare the reported metrics.

### SB-507 — Install the global agent skill from the CLI

Added `sbuf skill install` for offline installation into a caller-selected agent
skill directory. Global and project variants are generated from one canonical
policy according to the CLI's selected scope. Identical reinstalls are no-ops,
conflicting files are preserved by default, and `--force` replaces them
atomically. Process tests cover both scopes and every installation outcome.

### SB-508 — Evaluate the host capture policy

**What to build:** Define a structured host assessment for named capture
events. It records the event, an abstain/propose/update/checkpoint disposition,
and a stable reason, with at most one atomic candidate. Exercise the installed
skill against realistic conversation scenarios instead of testing a separate
hand-written decision tree.

**Blocked by:** SB-505

**Acceptance criteria:**

- [ ] Scenarios cover durable corrections, accepted decisions, tentative
      discussion, routine completion, repository-authoritative knowledge,
      confirmed root causes, and necessary handoffs.
- [ ] Expected results include correct abstention reasons as well as proposals,
      updates, and checkpoints.
- [ ] Evaluation reports proposal precision, missed-memory judgments, and
      approval, edit, rejection, and duplicate outcomes.
- [ ] Assessments and feedback store no raw prompts or transcripts.
- [ ] Capture worthiness remains a host decision; core receives only an
      admitted candidate and continues to own validation and lifecycle policy.

**Verification:** Run the checked-in capture scenarios against the packaged
skill and inspect the assessment and receipt-feedback output.

### SB-509 — Correct overlap semantics and build a relation corpus

**What to build:** Preserve exact normalized-body equality as the deterministic
duplicate fast path. Treat a different body with the same normalized title,
kind, and scope as possible overlap rather than proven conflict. Build a
reviewed pair corpus for semantic relation evaluation.

**Acceptance criteria:**

- [ ] Exact matches still return `duplicate_of` deterministically.
- [ ] Same-title records with different bodies do not claim a contradiction
      without relation evidence.
- [ ] The corpus covers paraphrase, one-way refinement, contradiction,
      compatible additions, temporal change, conditional differences, related
      records, and unrelated records.
- [ ] Process and protocol tests describe the revised result and its human
      review path.

**Verification:** Run repository and protocol tests, then compare the old
title/body heuristic with the reviewed relation corpus.

### SB-510 — Add advisory semantic relation analysis

**What to build:** Retrieve a small set of related records with the existing
hybrid index, then classify each candidate pair with a replaceable local
relation analyzer. The analyzer may report equivalence, one-way entailment,
contradiction, related, unrelated, or unknown.

**Blocked by:** SB-506, SB-509

**Acceptance criteria:**

- [ ] Embeddings select candidate pairs but never determine contradiction by
      similarity alone.
- [ ] Pairwise analysis can abstain and reports evidence, a confidence band,
      and an analyzer fingerprint.
- [ ] The analyzer first runs in shadow mode and is compared with reviewed
      corpus judgments.
- [ ] Inferred relations are advisory and cannot approve, reject, merge,
      supersede, archive, or edit a canonical record.
- [ ] Model output and inferred relations live in a disposable, rebuildable
      projection; Markdown remains canonical.
- [ ] No record content is sent to a remote model.

**Verification:** Evaluate deterministic, retrieval-only, and pairwise variants
against the relation corpus. Inspect false contradictions and abstentions before
enabling review warnings.

### SB-511 — Separate project context from repository isolation

**What to build:** Give project stores a stable canonical identity and separate
the composed project view from strict nearest-repository retrieval. A project
view may include applicable global records; strict local retrieval must use
only the nearest `.sbuf/`.

**Acceptance criteria:**

- [ ] `store.toml` stores a stable project ID and an editable project name, with
      validation and initialization tests.
- [ ] Renaming a repository directory does not change its project identity or
      make its existing records fall outside the current project scope.
- [ ] Two repositories with the same directory name do not share a semantic
      project identity.
- [ ] `--project` selects the composed project view and `--local` selects only
      the nearest repository store. Their behavior is unambiguous for search,
      context, status, mutations, and machine-readable invocation.
- [ ] Strict local retrieval never opens or returns records from the global
      store; project retrieval retains applicable global context.
- [ ] Project identity is canonical metadata. Any SQLite fields are disposable
      projections and can be rebuilt without losing identity.
- [ ] Scope derivation is owned by `stormbuffer-core` rather than duplicated in
      CLI and protocol adapters.

**Verification:** Initialize two same-named repositories, rename one, and test
project and strict-local retrieval through core, CLI, and JSON protocol
boundaries. Rebuild each index and confirm that project identity and filtering
are unchanged.

### SB-512 — Expand store status and safe repair

**What to build:** Make `status` the single operational summary for a selected
store, and let `doctor` repair disposable state when the repair has one
unambiguous outcome.

**Blocked by:** SB-511

**Acceptance criteria:**

- [ ] `status` reports lifecycle counts, canonical and disposable disk usage,
      index and embedding versions, and the last successful synchronization.
- [ ] Human and JSON output identify whether status describes the global,
      project, or strict local view.
- [ ] `doctor --repair` can rebuild or remove disposable projections, stale
      locks, and temporary metadata reported by `doctor`.
- [ ] `doctor --repair` never edits, archives, replaces, or deletes canonical
      records and gives a concrete manual action for canonical failures.
- [ ] Repeated repair is a no-op after the store becomes healthy.

**Verification:** Corrupt each disposable component in an isolated store, run
`doctor --repair`, and confirm that canonical Markdown is byte-for-byte
unchanged. Exercise human and JSON status output at each store boundary.

### SB-513 — Add backup previews and guarded store destruction

**What to build:** Let users inspect restore consequences before import, verify
an export independently, and deliberately remove an entire selected store when
starting over is the intended operation.

**Blocked by:** SB-511

**Acceptance criteria:**

- [ ] `import --dry-run` reports ID, scope, destination, and equivalent-record
      collisions without writing records or projections.
- [ ] Export verification checks the archive schema, record integrity, and
      provenance without importing it.
- [ ] Whole-store destruction prints the resolved store identity and affected
      canonical and disposable data before confirmation.
- [ ] Noninteractive destruction requires both `--yes` and the expected stable
      store ID, and offers an export before deleting canonical records.
- [ ] The CLI does not add ambiguous `reset` or `clear` commands.

**Verification:** Preview colliding imports and verify valid and corrupted
exports. Exercise cancelled, wrong-ID, backed-up, and confirmed destruction in
isolated global and project stores.

### SB-514 — Add a candidate inbox and read-only memory audit

**What to build:** Provide one review queue for candidate records and an `audit`
command that reports possible maintenance work with evidence and an explicit
follow-up command.

**Blocked by:** SB-505, SB-510

**Acceptance criteria:**

- [ ] The candidate inbox filters by age, kind, source, scope, and possible
      overlap, with human and machine-readable output.
- [ ] `audit` reports unresolved candidates, broken record links, stale
      checkpoints, and relation-supported duplicate or refinement candidates.
- [ ] Each audit finding names its evidence, confidence or deterministic rule,
      and the existing lifecycle command a person may choose to run.
- [ ] Running `audit` never edits, approves, rejects, supersedes, archives, or
      deletes a canonical record.
- [ ] Any usage-based finding distinguishes missing receipt history from known
      non-use and remains unavailable until SB-505 supplies that evidence.

**Verification:** Run the candidate inbox and audit against checked-in lifecycle
and relation fixtures. Snapshot human and JSON findings, then confirm that the
canonical record tree is unchanged.

**Milestone exit:** Agents can propose candidates after high-signal capture
events without sweeping routine work, explain why they abstained, resume project
work from sourced checkpoints, install global-memory behavior without
maintaining a separate skill copy, distinguish capture, retrieval, and use
failures, surface possible record relationships without autonomous changes, and
help people inspect, repair, review, back up, or deliberately remove a store
through commands with specific targets.

## Milestone 6: MCP and releases

### SB-601 — Implement the MCP adapter

Mapped the approved resources and tools to core operations over
stdio without duplicating storage, ranking, or policy.

### SB-602 — Add the behavioral agent skill

Documented when agents should search, compile context, propose
durable memory, report conflicts, and avoid storing unsuitable material.

### SB-603 — Harden packaging and releases

**What to build:** Produce cross-platform releases with `sbuf`,
`stormbuffer-mcp`, verified model setup, generated `sbuf` man pages and
completions, docs, and an installation smoke test on a machine without
Stormbuffer.

**Blocked by:** SB-003, SB-004, SB-006, SB-304, SB-402, SB-405, SB-406, SB-601

**Acceptance criteria:**

- [x] Supported platform artifacts prove `sbuf` and `stormbuffer-mcp` start and
      report the packaged version.
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
