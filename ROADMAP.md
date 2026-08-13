# Stormbuffer roadmap

Stormbuffer is a local-first memory store for people and AI agents. It stores
sourced project knowledge as readable files under human control.

## Version 0.1.0 hardening

The release also needs core rejection of high-confidence secrets in agent
writes, tokenizer-aware embedding chunks, optional source revision metadata,
and CI across supported platforms. Multiple chunks from one record will be
considered only if the benchmark shows that the current diversity rule loses
necessary evidence.

## Agent host lifecycle integrations

Add prompt-time recall integrations in this order: Codex, Pi, then OpenCode.
Each host adapter will pass only the current user prompt to the versioned
`sbuf invoke context` protocol and inject a successful, bounded result before
the first model call. It will retrieve once per user message, preserve receipts
and record IDs, mark recalled text as untrusted evidence, and continue normally
when Stormbuffer is unavailable or has no matching memory.

Scope must remain explicit and follow the CLI: global by default, project for
combined project and applicable global memory, and local for strict nearest-
store retrieval.

Each integration will also use the host's stable end-of-turn boundary to ask
the current model once whether the completed turn contains a capture event. The
model will apply the skill's admission rules and, only when they pass, use the
versioned CLI or explicitly write-enabled MCP `remember` or `update` interface
to create a reviewable candidate. Routine completion produces no candidate.
Lifecycle code will never edit canonical Markdown directly, approve a
candidate, or activate memory. Prefer a guarded continuation of the current
model context over transcript inspection.

### Codex

Implement Codex first as a plugin bundle containing the Stormbuffer skill,
read-only-by-default MCP configuration, and `UserPromptSubmit` and `Stop` hooks.
Add a small CLI adapter that reads the prompt hook event, calls the shared
context protocol, and emits `hookSpecificOutput.additionalContext`. At `Stop`,
return one guarded continuation prompt that asks Codex to evaluate capture and
use the skill's candidate workflow when warranted. Use `stop_hook_active` and
an integration marker to prevent loops and suppress prompt-time recall for that
internal continuation. Do not use `SessionEnd`: its output cannot steer Codex,
while the documented
[`Stop` hook](https://learn.chatgpt.com/docs/hooks#stop) can continue the turn
with the model's existing context. Do not parse transcripts. Measure warm
subprocess retrieval latency before enabling prompt-time recall by default.

### Pi

Package a Pi extension with the Stormbuffer skill after the Codex behavior is
stable. Retrieve through `before_agent_start` and add the result as hidden,
persistent custom context for that turn. Keep the existing Pi MCP adapter for
memory operations instead of duplicating its tools in the extension. At
[`agent_settled`](https://pi.dev/docs/latest/extensions#agent-start--agent-end--agent-settled),
send one tagged custom follow-up with `triggerTurn: true` so the model evaluates
capture after retries, compaction retries, and queued follow-ups are exhausted.
Guard the resulting run from scheduling itself. The extension does not write
records; the model may create a candidate through the existing skill or MCP
interface when writes are enabled. Do not retrieve on every `context` event.

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
