# Stormbuffer roadmap

Stormbuffer is a local-first memory store for people and AI agents. It stores
sourced project knowledge as readable files under human control.

## Version 0.1.0 hardening

Core agent writes now reject high-confidence secrets before canonical Markdown
is written. Embedding inputs honor the selected model's token limit, and source
receipts can preserve when and at which revision content was observed. The
release still needs CI across supported platforms. Multiple chunks from one
record will be considered only if the benchmark shows that the current diversity
rule loses necessary evidence.

The scale baseline now covers 100, 1,000, and 10,000 records with deterministic
local embeddings. At 10,000 records, warm MCP recall had a 1.65-second median,
while warm reconciliation alone had a 1.24-second median. Freshness work is the
first bottleneck to investigate. Evaluate a persisted generation or freshness
marker before considering a daemon or broader cache redesign.

## Agent host lifecycle integrations

Codex and Pi now provide prompt-time recall through host-native lifecycle
plugins. OpenCode remains the next integration. Each completed adapter passes
only the current user prompt to the versioned `sbuf invoke context` protocol and
injects a successful, bounded result before the first model call. Each one
retrieves once per host turn, preserves receipts and record IDs, marks recalled
text as untrusted evidence, and continues normally when Stormbuffer is
unavailable or has no matching memory.

Scope must remain explicit and follow the CLI: global by default, project for
combined project and applicable global memory, and local for strict nearest-
store retrieval.

The Codex and Pi integrations use each host's stable end-of-turn boundary to
ask the current model once whether the completed turn contains a capture event.
The model applies the skill's admission rules and, only when they pass, uses the
versioned CLI or explicitly write-enabled MCP `remember` or `update` interface
to create a reviewable candidate. Routine completion produces no candidate.
Lifecycle code never edits canonical Markdown directly, approves a candidate,
or activates memory. Both integrations use guarded continuations rather than
parsing transcripts.

### Codex

The Codex plugin bundles the Stormbuffer skill, read-only-by-default MCP
configuration, and `UserPromptSubmit` and `Stop` hooks. Its adapter reads the
prompt hook event, calls the shared context protocol, and emits
`hookSpecificOutput.additionalContext`. At `Stop`, it returns one guarded
continuation prompt that asks Codex to evaluate capture and use the skill's
candidate workflow when warranted. `stop_hook_active` and an integration marker
prevent loops and suppress prompt-time recall for that internal continuation.
The adapter does not use `SessionEnd` or parse transcripts. Warm subprocess
retrieval latency is covered by a repeatable package benchmark.

### Pi

The Pi package contains an extension and the Stormbuffer skill. It retrieves
through `before_agent_start` and adds successful recall as hidden, persistent
custom context for the turn. It keeps explicit memory operations in the
existing skill or MCP adapter rather than duplicating tool schemas or policy.
At
[`agent_settled`](https://pi.dev/docs/latest/extensions#agent-start--agent-end--agent-settled),
the extension sends one tagged custom follow-up with `triggerTurn: true` after
retries, compaction retries, and queued follow-ups are exhausted. The resulting
run cannot schedule itself. The extension neither writes records nor retrieves
through the repeated `context` event.

### OpenCode

Implement OpenCode last against a fixed, supported plugin version. First verify
that the target version exposes a stable pre-model context hook. Prefer the V2
direct context hook if it is stable by then; otherwise use the stable typed
`chat.message` and `experimental.chat.system.transform` hooks with a bounded
session-and-message cache so retrieval still occurs once. Stop at the API gate
if neither route provides a stable lifecycle boundary. Separately verify the
[`session.idle`](https://opencode.ai/docs/plugins/#events) event and session
prompt API, then use them to request one tagged capture-evaluation turn. Guard
that turn from retriggering capture or prompt-time recall. If the pinned API
cannot identify the session, start a prompt reliably, and prevent recursion,
defer end-of-turn capture rather than inferring behavior from unstable types.

Each integration is complete only when tests show that matching memory is
available before the first model call, retrieval happens once per user message,
scope boundaries are preserved, output remains bounded, and missing or
malformed data cannot block the host. Installation smoke tests must also prove
that a qualifying capture event can create a candidate, an ordinary completed
turn creates nothing, internal continuation cannot loop, and lifecycle code
cannot approve or activate memory.

## Local web editor and stored-relation graph

After the first release, a local web app will support browsing, search, editing,
review, and lifecycle controls for people who prefer not to use the CLI. A graph
may show relationships stored in record fields, including supersession, scope,
sources, and shared tags. It will not infer an opaque knowledge graph.

The server now runs through `sbuf serve` and exposes a documented OpenAPI
contract for browsing, lexical search, conditional record edits, and lifecycle
controls. It binds only to loopback, refuses remote and wildcard addresses,
uses canonical-record ETags for edits, logs to stderr, and shuts down on Ctrl-C
or `SIGTERM`. The CLI and web app will use the same core policy.

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
