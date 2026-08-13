# To-Dos

## Milestone 7: Version 0.1.0 hardening

### SB-701 — Finish the hybrid MCP recall boundary

Tested hybrid `memory_recall` through the MCP server with a
deterministic injected embedder, and report why semantic retrieval fell back to
lexical retrieval.

### SB-702 — Build a realistic software-agent memory benchmark

Expanded the deterministic evaluation corpus to roughly 100–300 canonical memories
and at least 100 retrieval questions across several simulated projects and historical revisions.

### SB-703 — Characterize scale and retrieval latency

Added a reproducible harness that generates realistic stores at about
100, 1,000, and 10,000 records and measures cold and warm behavior.

### SB-704 — Reject secrets in agent-originated core writes

**What to build:** Add a conservative detector in `stormbuffer-core` and apply
it to agent-originated `remember` and `update` operations before canonical
writes.

**Acceptance criteria:**

- [x] Detection covers PEM private keys, obvious bearer or authorization
      headers, strong common API-token prefixes, and reliably identifiable
      credential-bearing URLs.
- [x] Rejection returns an actionable validation error that does not echo the
      secret, log the candidate body, or write a canonical record.
- [x] Tests cover every supported pattern and ordinary code, hashes, UUIDs, and
      example placeholders that must remain valid.
- [x] Direct human editing remains outside this automated-write safeguard.

**Verification:** Run focused core and adapter tests for accepted and rejected
`remember` and `update` requests, including an assertion that errors contain no
secret material.

### SB-705 — Enforce embedding token limits during chunking

**What to build:** Use tokenizer or model metadata to split or validate
structural Markdown chunks before embedding so every input fits the model's
declared token limit.

**Blocked by:** SB-702

**Acceptance criteria:**

- [x] Embedding inputs are bounded by model tokens rather than whitespace word
      counts while preserving structural Markdown boundaries where possible.
- [x] Tests cover code, long identifiers, paths, JSON, shell commands, and dense
      punctuation that exceed 256 model tokens despite a low word count.
- [x] Chunking stays deterministic and uses no LLM-based splitter.

**Verification:** Run focused chunking and indexing tests with a tokenizer-aware
test model and assert that no emitted embedding input exceeds its manifest.

### SB-706 — Preserve optional source freshness metadata

**What to build:** Extend source provenance with optional `observed_at`,
`revision`, and `content_hash` fields so a later audit can compare a memory's
source revision without rewriting the memory.

**Acceptance criteria:**

- [x] Each field is optional, round-trips through canonical Markdown, and is
      preserved by every adapter.
- [x] Source types are not required to provide metadata they cannot support.
- [x] Local file or Git-backed sources use stable hashes or revisions where
      those values are already available.
- [x] This ticket does not add source crawling, connectors, automatic
      invalidation, or canonical-memory rewriting.

**Verification:** Run core codec and protocol round-trip tests with all, some,
and none of the optional source fields.

### SB-707 — Evaluate additional chunks from one record

**What to build:** Add benchmark cases where an answer needs two independent
sections from one record and cases where redundant chunks should not consume the
context budget. Change retrieval only if the evaluation shows a material loss.

**Blocked by:** SB-702

**Acceptance criteria:**

- [ ] The evaluation compares answer and context quality under the current
      record-diversity rule and a bounded multi-chunk policy.
- [ ] If the change is justified, retrieval selects the best chunk per record
      first, permits at most one additional non-redundant chunk from a selected
      record, and charges it to the normal context budget.
- [ ] If the change is not justified, the results are recorded and record-level
      deduplication remains unchanged.

**Verification:** Run the focused evaluation and retain a retrieval change only
when the reported quality improves without redundant chunks crowding out other
records.

### SB-708 — Add repository CI before release

**What to build:** Add GitHub Actions for supported platforms and a lightweight
protocol smoke test, using deterministic embedders instead of remote model
artifacts.

**Acceptance criteria:**

- [ ] CI runs Rust formatting, clippy with warnings denied, and all workspace
      tests with all features.
- [ ] CI runs the documentation site's check, lint, test, and build commands.
- [ ] JSON and MCP boundaries receive a smoke test when workspace tests do not
      already exercise them.
- [ ] CI does not download remote embedding artifacts, and release automation
      is deferred until ordinary CI is stable.

**Verification:** Run the workflow commands locally and confirm the configured
platform matrix completes in GitHub Actions.

**Milestone exit:** Retrieval behavior is measured at realistic quality and
scale, automated writes reject high-confidence secrets, embedding inputs obey
model limits, hybrid fallback is observable, and supported platforms pass CI
without remote model downloads.

## Milestone 8: Agent host lifecycle integrations

### SB-801 — Add lifecycle recall and capture consideration to Codex

**What to build:** Package the Stormbuffer skill, read-only-by-default MCP
configuration, and `UserPromptSubmit` and `Stop` hooks as a Codex plugin. Add a
small CLI adapter that translates prompt hook events to the versioned
`sbuf invoke context` protocol and returns
`hookSpecificOutput.additionalContext`. Use `Stop` to ask the current model
once to evaluate the completed turn for a capture event.

**Blocked by:** SB-703, SB-704

**Acceptance criteria:**

- [ ] The adapter passes only the current user prompt and the selected global,
      project, or local scope to the shared context protocol.
- [ ] Matching context is available before the first model call and retrieval
      runs once per user message.
- [ ] Injected context is bounded, marked as untrusted evidence, and preserves
      its receipt and record IDs.
- [ ] Empty results, malformed events, and an unavailable Stormbuffer do not
      block the host or add invalid context.
- [ ] The `Stop` hook emits one capture-evaluation continuation, uses
      `stop_hook_active` plus an integration marker to prevent recursion, and
      does not cause a second recall for its internal prompt.
- [ ] A qualifying capture event lets the model submit a candidate through the
      versioned CLI or explicitly write-enabled MCP `remember` or `update`
      flow; routine completion submits nothing.
- [ ] Lifecycle code does not parse transcripts, use `SessionEnd`, edit records,
      approve candidates, or activate memory.
- [ ] Warm subprocess retrieval latency is measured before the hook is enabled
      by default.

**Verification:** Run focused CLI process tests with fixture hook events for
each scope, capture outcome, loop guard, and failure mode. Install the plugin
and confirm one retrieval before the first model call, one candidate for a
qualifying correction or decision, and no candidate for routine completion.

### SB-802 — Add lifecycle recall and capture consideration to Pi

**Blocked by:** SB-801

**What to build:** Package a Pi extension and the Stormbuffer skill. Retrieve
through `before_agent_start` and add successful recall as hidden, persistent
custom context for the turn. At `agent_settled`, send one tagged custom
follow-up with `triggerTurn: true` that asks the model to evaluate capture.

**Acceptance criteria:**

- [ ] The extension reuses the prompt, scope, output, and failure behavior
      established by the Codex adapter.
- [ ] Retrieval runs once per user message and does not run again for repeated
      model calls in the same turn.
- [ ] The existing Pi MCP adapter remains the interface for explicit memory
      operations, with no duplicated tool schemas or policy in the extension.
- [ ] The extension uses `agent_settled`, not `agent_end`, so capture evaluation
      starts only after automatic retries, compaction retries, and queued
      follow-ups are exhausted.
- [ ] The tagged capture turn cannot schedule itself or trigger duplicate
      recall. The extension never writes or approves records directly.
- [ ] A qualifying event can produce a candidate through the existing skill or
      write-enabled MCP interface, while routine completion produces none.
- [ ] The extension does not retrieve through the `context` event.
- [ ] Unit fixtures cover empty, malformed, unavailable, and oversized results
      for every supported scope.

**Verification:** Run the extension unit tests, install the package in Pi, and
confirm recall before the first model call plus guarded candidate creation and
no-op outcomes at `agent_settled`.

### SB-803 — Add lifecycle recall and capture consideration to OpenCode

**Blocked by:** SB-802

**What to build:** Pin a supported OpenCode plugin version and verify its
pre-model context API before implementing the adapter. Use the V2 direct context
hook if it is stable; otherwise use the stable typed `chat.message` and
`experimental.chat.system.transform` hooks with a session-and-message cache.
Verify `session.idle` and the session prompt API separately, then use them for
one tagged capture-evaluation turn when their types and behavior are sufficient.

**Acceptance criteria:**

- [ ] Implementation stops at the API gate if the pinned version has no stable
      way to add context before the first model call.
- [ ] The adapter retrieves once per user message even when OpenCode repeats a
      transform hook or model call.
- [ ] Prompt, scope, output, and failure behavior match the Codex and Pi
      integrations without duplicating core retrieval policy.
- [ ] End-of-turn capture stops at its own API gate unless the pinned version
      can identify the idle session, start a prompt, and prevent recursion.
- [ ] A supported `session.idle` path asks the model to evaluate capture once,
      suppresses recall and capture rescheduling for that internal turn, and
      leaves candidate creation to the existing skill or write-enabled MCP
      interface.
- [ ] Qualifying events produce reviewable candidates; routine completion
      produces nothing; lifecycle code cannot approve or activate memory.
- [ ] Compatibility tests cover repeated transforms, host errors, scope
      isolation, malformed results, an unavailable Stormbuffer, and idle-event
      recursion.

**Verification:** Run the plugin's compatibility tests against the pinned
OpenCode version, then install it and confirm one pre-model retrieval per user
message and one guarded capture evaluation after the session becomes idle.

**Milestone exit:** Codex, Pi, and OpenCode retrieve bounded, sourced context
before the first model call in that order. Each host preserves scope, fails
softly when recall is unavailable, and asks the model once at a stable end point
whether the turn warrants a candidate. Candidate creation uses the existing
skill or MCP policy and remains subject to human approval.

## Milestone 9: Local web editor and graph

### SB-901 — Define the local server API and security boundary

**What to build:** Expose core browse/search/lifecycle operations through an
HTTP API that binds to loopback, protects concurrent writes, handles signals,
and shuts down under a service manager.

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

### SB-902 — Build the human web editor

**What to build:** Add an accessible responsive app for listing, searching,
viewing, creating, editing, approving, superseding, archiving, and restoring
memories without the CLI.

**Blocked by:** SB-901

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

### SB-903 — Add the stored-relation graph

**What to build:** Visualize selected memories and their stored supersession,
scope, source, and shared-tag relationships in an Obsidian-style graph linked to
the editor.

**Blocked by:** SB-902

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

### SB-904 — Package and document daemonized operation

**What to build:** Document and test foreground server operation under common
service managers without adding a custom daemon supervisor.

**Blocked by:** SB-901, SB-902, SB-903

**Acceptance criteria:**

- [ ] Example user-service configurations name the store and bind address and
      preserve in-progress writes during restart and shutdown.
- [ ] Logs work with the service manager and contain no record bodies by default.
- [ ] Documentation covers installation, upgrade, backup, and recovery.
- [ ] An end-to-end test shows that the web app and CLI return matching records
      and enforce core lifecycle policy.

**Verification:** Run the documented foreground smoke test and at least one
service-manager integration check on a supported platform.

**Milestone exit:** A person can run Stormbuffer as a loopback-only service,
edit memory without the CLI, and inspect stored relationships in an accessible
graph.
