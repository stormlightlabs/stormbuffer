# Stormbuffer implementation tickets

These tickets implement `ROADMAP.md`. They are listed in dependency order. Take
one ticket per focused implementation session when practical, update the docs in
the same change, and check the acceptance criteria before moving on.

## Milestone 0: Usable shell and living docs

### SB-001 — Establish the Rust workspace and shared conventions

**What to build:** Make the existing crates build as one workspace with a small,
clear dependency surface and common error, tracing, and test conventions.

**Blocked by:** None - can start immediately

**Acceptance criteria:**

- [x] The root workspace includes `core`, `cli`, `mcp`, and `server` crates.
- [x] Package and binary names match the Stormbuffer naming contract.
- [x] Workspace formatting, linting, and test commands are documented and pass.
- [x] Placeholder arithmetic and hello-world code is removed.

**Verification:** Run the Rust checks in `ROADMAP.md`.

### SB-002 — Ship the complete CLI command shell

**What to build:** Define the full public command tree in Clap so users can
discover Stormbuffer immediately. Implement `--help`, `--version`, `init`,
`root`, and `status`; make every unfinished command a safe and explicit stub.

**Blocked by:** SB-001

**Acceptance criteria:**

- [x] Every command in the roadmap parses and has useful command-level help.
- [x] `init`, `root`, and `status` work for global and project stores.
- [x] Stubs identify themselves, make no data changes, and return a documented
      non-zero status.
- [x] Expected user errors are concise and do not expose Rust panics.
- [x] Process-level tests cover success, invalid input, stubs, and piped output.

**Verification:** Run focused CLI integration tests and inspect top-level and
nested help in a narrow and a non-interactive terminal.

### SB-003 — Apply the CLI output, color, and alias contract

**What to build:** Add consistent stdout/stderr behavior, stable exit statuses,
and `owo-colors` styling that respects terminals, `NO_COLOR`, and
`--color auto|always|never`. Make `stormbuffer`, `stormbuf`, and `sbuf`
equivalent entry points.

**Blocked by:** SB-002

**Acceptance criteria:**

- [x] Primary output uses stdout; diagnostics and logs use stderr.
- [x] Redirected and JSON output never contains ANSI escapes.
- [x] `NO_COLOR` and each `--color` mode have process-level tests.
- [x] All three executable names expose the same commands and exit behavior.
- [x] Help uses the invoked name where the packaging approach permits it.

**Verification:** Run CLI process tests with TTY/non-TTY output and each binary
name.

### SB-004 — Generate man pages and shell completions

**What to build:** Generate man pages with `clap_mangen` and supported shell
completions with `clap_complete` from the runtime command definition.

**Blocked by:** SB-002

**Acceptance criteria:**

- [x] A workspace build regenerates all root-level artifacts deterministically.
- [x] Common shells receive completion files and every public command appears.
- [x] Man pages cover the top-level CLI and useful subcommand detail.
- [x] A test exercises the shared artifact renderer and verifies the build outputs.

**Verification:** Build twice with no asset diff, render a man page, and load at
least one completion file in its target shell.

### SB-005 — Build the static documentation foundation

**What to build:** Replace the Svelte starter with a static, mdsvex-based docs
site using typed frontmatter, Docusaurus-like navigation, Pagefind, and the
specified typography.

**Blocked by:** None - can start immediately

**Acceptance criteria:**

- [x] SvelteKit uses `adapter-static`; Markdown pages use mdsvex and validated
      frontmatter.
- [x] The site has top navigation, sidebar, breadcrumbs, table of contents,
      previous/next links, version display, and responsive navigation.
- [x] IBM Plex Serif styles headings, IBM Plex Sans styles body/UI, and JetBrains
      Mono styles code using Fontsource variable packages where available.
- [x] Pagefind indexes the production build and search works without a hosted
      service.
- [x] Reading and navigation work without client-side JavaScript.
- [x] The first pages cover installation, quick start, CLI status, data model,
      and architecture without presenting stubs as finished features.

**Verification:** Run the docs checks in `ROADMAP.md`, then keyboard-check the
built site at desktop and narrow widths with JavaScript disabled.

### SB-006 — Keep documentation current

**What to build:** Make docs part of the definition of done, exercise documented
CLI examples, and verify that build.rs writes root-level operator artifacts from
the shared Clap command tree.

**Blocked by:** SB-002, SB-004, SB-005

**Acceptance criteria:**

- [x] Contributor guidance names the public changes that require docs updates.
- [x] A repeatable docs check exercises documented CLI examples and tests the
      shared artifact renderer against `assets/man/` and `assets/completions/`.
- [x] Examples used as smoke tests fail when documented commands drift.
- [x] Release guidance includes a human review for prose and navigation.

**Verification:** Build the workspace, run the focused artifact and documented
example tests, then run the docs checks. Build outputs remain root-level and
gitignored; `crates/cli/build.rs` is the only generator.

**Milestone exit:** A development install offers useful initialization and
inspection, an honest full command shell, all three names, generated operator
artifacts, and searchable static docs.

## Milestone 1: Canonical Markdown store

### SB-101 — Implement the record model and TOML frontmatter codec

**What to build:** Add typed IDs, enums, source records, timestamps, and a
loss-conscious Markdown/TOML parser and renderer for the canonical schema.

**Blocked by:** SB-001

**Acceptance criteria:**

- [x] Valid records round-trip without changing the body or losing metadata.
- [x] Missing, unknown, malformed, and incompatible-version fields produce
      actionable errors with file context.
- [x] IDs, scopes, statuses, access values, and lifecycle transitions validate.
- [x] Fixtures cover all four kinds, multiple sources, Unicode, code blocks,
      and malformed records.
- [x] The schema reference and examples match the implemented format.

**Verification:** Run focused core codec and fixture tests.

### SB-102 — Resolve global and project stores safely

**What to build:** Use platform directories for global data/cache locations and
discover `.sbuf/` project configuration with private defaults.

**Blocked by:** SB-101

**Acceptance criteria:**

- [x] Resolution is deterministic and testable without using a real home
      directory.
- [x] Project discovery handles nested working directories and a missing store.
- [x] New project stores default to local/private memory and explain shared-mode
      implications before opt-in.
- [x] `stormbuffer --project init --shared` is the explicit shared-store opt-in;
      `--shared` is rejected for a global store.
- [x] Shared mode tracks only store configuration, narrow ignore rules, and
      canonical Markdown; SQLite, FTS/vector projections, embeddings, models,
      locks, temporary files, and logs are ignored.
- [x] `init`, `root`, and `status` use the core resolver instead of CLI-local
      logic.

**Verification:** Run resolver integration tests in temporary directory trees.

### SB-103 — Add atomic repository operations and lifecycle commands

**What to build:** Implement locked, atomic creation and updates plus add, edit,
show, list, supersede, archive, restore, and guarded permanent deletion.

**Blocked by:** SB-101, SB-102

**Acceptance criteria:**

- [x] Writes validate before commit and use a same-filesystem temporary file,
      `fsync`, atomic rename, and a store mutation lock.
- [x] `$EDITOR` flows preserve user Markdown and report concurrent changes.
- [x] Supersession retains history and default listing excludes inactive data.
- [x] `forget --destroy` requires deliberate confirmation or an explicit
      non-interactive acknowledgement; no other command permanently deletes.
- [x] Interrupted/competing write tests never produce a partially written
      canonical record.
- [x] User and command reference docs describe the implemented behavior.

**Verification:** Run core repository tests and process-level lifecycle tests.

**Milestone exit:** A person can manage the complete record lifecycle using only
canonical Markdown and public CLI commands.

## Milestone 2: Rebuildable lexical index

### SB-201 — Build the SQLite projection and migrations

**What to build:** Add versioned migrations for scopes, records, chunks, sources,
index metadata, and contentless-delete FTS5, with SQLite treated only as cache.

**Blocked by:** SB-101, SB-102

**Acceptance criteria:**

- [ ] A new database enables foreign keys and appropriate WAL behavior.
- [ ] Schema upgrades are transactional and tested from every supported version.
- [ ] No projected field is the sole copy of user information.
- [ ] Dropping and rebuilding the database preserves canonical data.

**Verification:** Run migration and clean-rebuild integration tests.

### SB-202 — Implement deterministic chunking and incremental sync

**What to build:** Scan, validate, hash, chunk, and project changed records in
per-file transactions while removing stale rows.

**Blocked by:** SB-201

**Acceptance criteria:**

- [ ] BLAKE3 hashes skip unchanged files.
- [ ] Heading-aware chunks do not split code blocks or lists and include the
      record title/heading in retrieval input.
- [ ] A failed projection leaves canonical Markdown intact and recoverable.
- [ ] `sync` reconciles manual edits, moves, invalid files, and deleted files.
- [ ] Repeated sync with no changes performs no record reindex work.

**Verification:** Run sync tests over checked-in filesystem fixtures.

### SB-203 — Deliver FTS search and lexical context output

**What to build:** Implement scoped FTS5 search with phrase, prefix, and exact
title/alias behavior, then expose it through `search` and bounded `context`.

**Blocked by:** SB-202

**Acceptance criteria:**

- [ ] Results default to active accessible records in current-project then
      allowed-global scope.
- [ ] Results include ID, title, kind, scope, excerpt, source, path, score, and
      lexical match reason.
- [ ] Context output respects its budget and includes a machine-readable receipt.
- [ ] Exact filenames, commands, aliases, and Unicode text have regression tests.
- [ ] CLI and docs examples use only the implemented result shape.

**Verification:** Run search/context integration tests and documented examples.

### SB-204 — Add watch, reindex, and doctor recovery flows

**What to build:** Implement convenience watching, explicit full rebuild, and
diagnostics for canonical/index/model/config inconsistencies.

**Blocked by:** SB-202, SB-203

**Acceptance criteria:**

- [ ] Correctness never depends on the watcher running.
- [ ] `reindex` builds a fresh projection and switches safely.
- [ ] `doctor` distinguishes warnings from failures and suggests repair commands.
- [ ] Interrupting watch or reindex leaves the existing store usable.
- [ ] Recovery procedures are documented and tested.

**Verification:** Run recovery tests with invalid files and interrupted rebuilds.

**Milestone exit:** Exact search is useful, manual edits reconcile, and the entire
index can be discarded and rebuilt safely.

## Milestone 3: Semantic and hybrid retrieval

### SB-301 — Add verified local embedding models

**What to build:** Implement the `Embedder` boundary, model manifest and checksum
verification, tokenizer/pooling/normalization behavior, and explicit model setup
for local CPU inference.

**Blocked by:** SB-202

**Acceptance criteria:**

- [ ] Model acquisition is documented, resumable or safely retryable, and never
      executes unverified content.
- [ ] Documents and queries use the intended tokenization, attention-mask mean
      pooling, and L2 normalization.
- [ ] Offline behavior and a missing/corrupt model produce actionable errors.
- [ ] Deterministic fixtures verify dimensions and numerical tolerances.

**Verification:** Run embedder contract tests with a pinned fixture model.

### SB-302 — Add versioned vector indexes

**What to build:** Implement the narrow `VectorIndex` boundary over pinned
`sqlite-vec`, including filtered search and non-destructive model migrations.

**Blocked by:** SB-201, SB-301

**Acceptance criteria:**

- [ ] Embeddings are keyed to chunks and filter by scope, kind, and active state.
- [ ] Model/table/checksum/dimension metadata is recorded.
- [ ] Backfill and validation complete before the active vector table switches.
- [ ] A failed migration leaves the previous index available.

**Verification:** Run vector contract, filter, backfill, and rollback tests.

### SB-303 — Fuse lexical and semantic retrieval

**What to build:** Retrieve lexical and vector candidates, combine them with
reciprocal-rank fusion, collapse chunks, apply documented deterministic boosts,
and compile bounded context.

**Blocked by:** SB-203, SB-302

**Acceptance criteria:**

- [ ] Search receipts explain lexical, vector, and deterministic match reasons.
- [ ] Superseded, archived, inaccessible, and wrong-scope records are excluded.
- [ ] No blanket recency boost applies to facts, decisions, or procedures.
- [ ] Context selection is deterministic for a fixed store/model and stays
      within budget.

**Verification:** Run hybrid retrieval and context-budget integration tests.

### SB-304 — Establish retrieval evaluations and release thresholds

**What to build:** Check in a representative corpus and query set and compare
FTS-only, vector-only, and hybrid behavior with reported metrics.

**Blocked by:** SB-303

**Acceptance criteria:**

- [ ] The harness reports every metric named in the roadmap.
- [ ] Queries include exact terms, paraphrases, scope collisions, superseded
      records, duplicates, and conflicts.
- [ ] Initial release thresholds and an intentional update process are documented.
- [ ] Ranking changes cannot update expected results silently.

**Verification:** Run the evaluation command and inspect the checked-in summary.

**Milestone exit:** Hybrid search clears the agreed corpus thresholds and emits
bounded, attributable context.

## Milestone 4: Grounded RAG and agent workflow

### SB-401 — Define the provider-neutral RAG context contract

**What to build:** Make `context` return bounded, ordered evidence blocks and a
receipt that any host model can consume without giving Stormbuffer responsibility
for generation or remote model access.

**Blocked by:** SB-303

**Acceptance criteria:**

- [ ] Every evidence block carries stable record/chunk IDs, title, scope, status,
      access, source references, selected text, and ranking reasons.
- [ ] The receipt records the query, filters, index/model versions, budget,
      truncation, and omitted-result count without leaking inaccessible records.
- [ ] Access, scope, and lifecycle policy runs before context assembly; fixed
      inputs produce deterministic ordering and truncation.
- [ ] The contract distinguishes host instructions, user input, and untrusted
      record text and states that record content cannot grant tools or authority.
- [ ] The core neither calls a generator nor transmits records to a remote model.
- [ ] Human, JSON, and MCP presentations derive from the same core result.

**Verification:** Run core context-contract tests for budgets, filtering,
determinism, hostile record text, and empty or insufficient retrieval.

### SB-402 — Evaluate grounded answers and citations

**What to build:** Extend the retrieval corpus into a RAG question suite with
inspectable supporting records, expected claims or abstention, and a repeatable
adapter for evaluating a configured host model.

**Blocked by:** SB-304, SB-401

**Acceptance criteria:**

- [ ] Fixtures cover answerable, unanswerable, conflicting, wrong-scope,
      superseded, long-context, and indirect prompt-injection cases.
- [ ] Reports separate retrieval, context-assembly, and generation failures and
      include context precision/recall, claim support, citation precision/recall,
      answer relevance, correct abstention, and scope leakage.
- [ ] Every factual expected claim names its supporting or contradicting record
      IDs; generated citations are checked against those records.
- [ ] Evaluation records the generator, model/version, prompt-contract version,
      parameters, and corpus revision needed to reproduce a run.
- [ ] Model-assisted judgments remain reviewable and cannot silently replace
      checked-in expectations or release thresholds.

**Verification:** Run deterministic corpus checks, then one documented configured
generator evaluation and inspect its claim-level report.

### SB-403 — Implement candidate review and provenance policy

**What to build:** Add propose, approve, and reject flows with source validation,
duplicate/conflict checks, explicit supersession, and narrowly scoped direct
activation permissions.

**Blocked by:** SB-103, SB-303

**Acceptance criteria:**

- [ ] Agents create candidates by default; human writes can become active.
- [ ] Proposal outcomes use the stable vocabulary in the roadmap.
- [ ] Unsupported inference and missing provenance are rejected clearly.
- [ ] Conflicts retain both claims and require explicit supersession/approval.
- [ ] Policy behavior has adversarial tests and user-facing documentation.

**Verification:** Run proposal/lifecycle integration tests over duplicate,
conflict, invalid, and approval cases.

### SB-404 — Publish the versioned JSON invocation protocol

**What to build:** Expose search, context, get, propose, supersede, and archive
through `invoke` with stable envelopes and errors.

**Blocked by:** SB-401, SB-403

**Acceptance criteria:**

- [ ] stdin/stdout contain only protocol JSON and logs use stderr.
- [ ] Operations are non-interactive, output-bounded, scope-aware, and reject
      arbitrary filesystem paths.
- [ ] Protocol versioning and stable error codes are documented.
- [ ] Golden/contract tests cover success, malformed input, denial, and internal
      failure without leaking sensitive paths or backtraces.

**Verification:** Run protocol contract tests and pipe every documented example
through a JSON parser.

### SB-405 — Complete portable import, export, and garbage collection

**What to build:** Add lossless export/import with collision handling and safe
cleanup of disposable cache/model artifacts.

**Blocked by:** SB-103, SB-204

**Acceptance criteria:**

- [ ] Export/import round-trips canonical records and provenance.
- [ ] ID, scope, and existing-record collisions require an explicit policy.
- [ ] `gc` never removes canonical records and reports reclaimed disposable data.
- [ ] Backup, move, and recovery workflows are documented.

**Verification:** Run round-trip, collision, and dry-run cleanup tests.

### SB-406 — Dogfood Stormbuffer with a shared project store

**What to build:** Initialize this repository as the reference shared-store
example and curate a small memory set that helps agents work on Stormbuffer.

**Blocked by:** SB-103, SB-402, SB-405

**Acceptance criteria:**

- [ ] The repository commits `.sbuf/store.toml`, `.sbuf/.gitignore`, and canonical
      Markdown under `.sbuf/records/`.
- [ ] `.sbuf/` ignores SQLite databases and sidecars, FTS/vector projections,
      embeddings, downloaded models, locks, temporary files, logs, and other
      rebuildable runtime artifacts.
- [ ] Records capture sourced project facts, decisions, procedures, and useful
      checkpoints without copying whole sections of `ROADMAP.md`, `TODO.md`, or
      `AGENTS.md`.
- [ ] A clean clone can rebuild all projections from the committed files and run
      documented retrieval and grounded-answer examples against the project.
- [ ] Tests fail if a generated or machine-local artifact under `.sbuf/` becomes
      trackable or if an example cites a missing record.
- [ ] Contributor docs explain the privacy and merge implications of shared
      project memory and how to opt out locally without deleting canonical files.

**Verification:** Clone or copy only tracked files into a temporary root, rebuild
the store, run the checked-in example questions, and audit `.sbuf/` ignore rules.

**Milestone exit:** An unattended agent can retrieve bounded evidence, produce
cited or correctly abstaining answers, and propose through a stable JSON boundary
without gaining uncontrolled write or deletion access. This repository provides
a reproducible shared-store example containing Markdown but no derived index.

## Milestone 5: MCP and releases

### SB-501 — Implement the thin MCP adapter

**What to build:** Map the approved resources and tools to core operations over
stdio without duplicating storage, ranking, or policy.

**Blocked by:** SB-404

**Acceptance criteria:**

- [ ] Resource URIs and tool schemas match the roadmap and docs.
- [ ] CLI JSON and MCP return equivalent core results for shared operations.
- [ ] Write tools are disabled by default or gated by an explicit host grant.
- [ ] Raw SQL, arbitrary files, reindex, and destructive deletion are absent.
- [ ] Protocol tests cover cancellation, malformed requests, and clean shutdown.

**Verification:** Run MCP contract tests against a temporary store.

### SB-502 — Add the behavioral agent skill

**What to build:** Document when agents should search, compile context, propose
durable memory, report conflicts, and avoid storing unsuitable material.

**Blocked by:** SB-404, SB-501

**Acceptance criteria:**

- [ ] The skill delegates all parsing, ranking, and mutation to public interfaces.
- [ ] Examples cite returned record IDs/receipts and prefer project scope.
- [ ] The skill forbids secrets, speculation, generic knowledge, raw transcripts,
      and duplicate authoritative documentation.
- [ ] Example sessions pass against the current CLI/MCP contracts.

**Verification:** Run skill examples as smoke tests and review the guidance for
least privilege.

### SB-503 — Harden packaging and releases

**What to build:** Produce cross-platform releases with all executable names,
verified model setup, man pages, completions, docs, and a clean-install smoke
test.

**Blocked by:** SB-003, SB-004, SB-006, SB-304, SB-402, SB-405, SB-406, SB-501

**Acceptance criteria:**

- [ ] Supported platform artifacts prove all three CLI names work.
- [ ] Package uninstall does not delete canonical user data.
- [ ] Release checks cover generated artifacts, licenses, checksums, and docs.
- [ ] A clean offline/online install matrix documents model behavior.
- [ ] Upgrade and rollback paths preserve canonical records.

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
memory without the CLI, and inspect explicit relationships in an accessible
graph.
