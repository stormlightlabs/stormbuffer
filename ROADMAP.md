# Stormbuffer roadmap

Stormbuffer is a local-first memory store for people and AI agents. It
stores sourced project knowledge as readable files under human control.

This roadmap describes product direction and release priorities. The
[concept documentation](docs/src/content/docs/concepts/) explains how the
system works, while [TODO.md](TODO.md) tracks implementation and dependencies.

## Direction

Stormbuffer should support this memory loop:

1. After a qualifying event, propose one sourced fact, decision, procedure, or
   project checkpoint as a candidate.
2. Review agent-created candidates before they become active.
3. Recall the right memory in a later session with enough context to use and
   cite it.
4. Correct stale knowledge without losing its history.

The product focuses on durable memory that changes future work. The agent host
continues to own recent conversation and fleeting task state. Stormbuffer owns
the project-scoped knowledge and resumable checkpoints worth carrying into
another session.

### Outcome

Judge the product by later reuse: an agent finds and cites a relevant record,
then changes its work because of that evidence. Routine sessions produce no new
record. The reader/user-facing [memory loop](docs/src/content/docs/concepts/memory-workflow.md)
describes how the host and the two stores divide responsibility.

## Agent capture and recall

The next product work should reduce setup time and improve what agents capture.

### Five-minute first memory

A new user should be able to install Stormbuffer, initialize a project store,
connect an agent through the documented skill or MCP server, approve one sourced
memory, and recall it with a citation within five minutes. Setup examples should
be copyable. `doctor` should identify failures in that path.

### Installable global skill

Using global memory should not require a hand-maintained copy of the agent skill
or a particular dotfiles layout. `sbuf` should install the maintained
global-scope skill into a caller-selected agent skill directory. The installed
artifact should travel with the CLI, work without a source checkout or network
access, select the global store visibly, and update without silently replacing
user changes.

### A smaller agent API

The MCP surface now exposes recall, get, remember, update, and forget intents.
Common calls stay short while results include provenance, conflicts, approval
requirements, and supersession links. Forgetting through MCP archives a memory.
Only a person using the CLI can permanently delete one.

### Better capture

The maintained agent skill expresses memory use as a small decision tree. It
recalls when prior durable context may affect the work, considers capture
only after a high-signal event, and otherwise takes no memory action. A capture
event permits evaluation; it does not guarantee a proposal.

That host decision is observable and testable. At a capture boundary, the host
classifies the event, chooses whether to abstain, propose, update, or
checkpoint, and gives a stable reason. Tests exercise realistic scenarios
against the installed policy, including corrections,
accepted decisions, tentative discussion, routine completion, and knowledge
already preserved in the repository. No-proposal is a valid outcome.

The host owns this semantic judgment. Stormbuffer core continues to enforce
scope, validation, lifecycle, and approval policy. The assessment is host-side
and disposable; it does not add a core worthiness classifier or store raw
conversation content.

### Project-scoped continuity

Project-scoped dogfooding confirmed that checkpoints and recall can resume work
across sessions. A useful checkpoint records completed work, the exact
unresolved state, settled decisions, the next meaningful action, and relevant
references. It omits chronology and routine commands, and it is unnecessary
when repository artifacts already preserve the state. It also omits dead ends
and transient details. Future failed handoffs will distinguish capture,
retrieval, and presentation failures. The scenarios did not expose a repeatable
gap that requires a separate brief or working-memory primitive.

### Measure usefulness

Feedback tied to retrieval receipts distinguishes knowledge that was never
captured, memory that retrieval missed, retrieved memory the agent ignored, and
retrieved memory that was stale or incorrect. It also shows whether a memory
was used, cited, corrected, or followed by a reviewed proposal. Reports store
no raw prompts or transcripts.

The evaluation reports time to later reuse, proposal approval, edit, rejection,
and duplicate rates, retrieved-and-used rate, stale-memory corrections, and
context cost per used memory. Repeatable failures decide whether Stormbuffer
next needs better capture, retrieval, or maintenance.

## Pre-web retrieval and maintenance

Use observed failures to choose the next retrieval or maintenance work. Likely
work includes proposing merges for duplicates, superseding stale memories,
archiving unused material, or improving ranking where the evaluation corpus and
real use show a repeatable miss. Stormbuffer should suggest these changes for
review rather than silently rewriting canonical memory.

Separate project context from repository isolation. A project view should
combine the nearest project store with applicable global memory. A strict local
view should read only the nearest `.sbuf/`, with no global fallback. The CLI
should expose the strict repository boundary as `--local` rather than making
users infer it from the current overloaded `--project` flag.

Give each project store a stable identity in canonical `store.toml` metadata.
Store a machine-stable project ID separately from its editable display name,
and stop deriving record scope from the repository directory name. SQLite may
project that identity for filtering, but it must remain rebuildable from the
canonical store and records. Do not duplicate a project name across every
record unless independent record export requires it.

The lifecycle and recovery commands expose the selected view's lifecycle
counts, canonical and disposable disk usage, index and model versions, and last
successful synchronization. `doctor --repair` fixes only disposable state;
malformed or ambiguous canonical records still require a person. Add preview
and verification paths for import and export before building the web editor.

Give people one candidate inbox and a read-only `audit` command for possible
duplicates, refinements, stale checkpoints, unresolved candidates, and broken
record links. Audit findings explain their evidence and point to existing
lifecycle commands. They never change canonical records. A later whole-store
destruction command must identify the exact store, require strong confirmation,
and offer an export before deletion. Avoid generic `reset` and `clear` commands
whose targets are unclear.

Exact normalized-body equality remains a deterministic duplicate check. A
different body under the same normalized title, kind, and scope only proves
possible overlap; it does not prove contradiction. Build a reviewed relation
corpus that distinguishes equivalence, one-way refinement, contradiction,
compatible additions, temporal change, related records, and unrelated records.

Once the corpus and usefulness measurements can expose errors, add a
replaceable local relation analyzer. Hybrid retrieval should select a small set
of candidate pairs, then pairwise inference may label their relationship or
abstain. Run the analyzer in shadow mode before showing advisory results. It
must not approve, reject, merge, supersede, archive, or rewrite canonical
records. Store inferred relationships and model fingerprints only in disposable
projections.

Milestone 6 covers packages, model and cache diagnostics, generated completions
and man pages, and installation smoke tests.

## Later: local web editor

Milestone 7 covers the local web app. It will provide browsing, search,
editing, review, and lifecycle controls for people who prefer not to use the
CLI. A graph may show stored relationships such as supersession, scope, source,
and shared tags. It will not infer an opaque knowledge graph.

The server will bind to loopback only. Remote access requires authentication
and a threat model. Core policy applies to both CLI and web workflows.

## Outside the roadmap

Stormbuffer is not pursuing a hosted user-profile service, a full agent runtime,
raw conversation storage, broad connector catalogs, or autonomous edits to
canonical memory. New ranking stages and inferred relationships require a
measured failure they can solve.

## Decisions to validate

- How much evidence `remember` and `update` need in the common MCP call.
- Whether receipt feedback provides enough signal when it stores no user
  content.
