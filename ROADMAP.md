# Stormbuffer roadmap

Stormbuffer is a local-first memory store for people and software agents. It
stores sourced project knowledge as readable files under human control.

This roadmap describes product direction and release priorities. The
[concept documentation](docs/src/content/docs/concepts/) explains how the
system works, while [TODO.md](TODO.md) tracks implementation and dependencies.

## Product direction

Stormbuffer should support this memory loop:

1. Capture a fact, decision, procedure, or project checkpoint from attributable
   evidence.
2. Review agent-created memories before they become active.
3. Recall the right memory in a later session with enough context to use and
   cite it.
4. Correct stale knowledge without losing its history.

The product focuses on durable memory that changes future work. The agent host
continues to own recent conversation and fleeting task state. Stormbuffer owns
the project-scoped knowledge and resumable checkpoints worth carrying into
another session.

## Agent capture and recall

The next product work should reduce setup time and improve what agents capture.

### Five-minute first memory

A new user should be able to install Stormbuffer, initialize a project store,
connect an agent through the documented skill or MCP server, approve one sourced
memory, and recall it with a citation within five minutes. Setup examples should
be copyable. `doctor` should identify failures in that path.

### A smaller agent API

The MCP surface now exposes recall, get, remember, update, and forget intents.
Common calls stay short while results include provenance, conflicts, approval
requirements, and supersession links. Forgetting through MCP archives a memory.
Only a person using the CLI can permanently delete one.

### Better capture

Agents should propose memory after a user correction, an accepted
decision and rationale, a confirmed surprising root cause, an undocumented
constraint, resumable project state, or the discovery of stale memory. Routine
success and information already maintained in project documentation should not
produce another memory.

Each proposal should contain one claim and its sources. It needs a future
trigger, a change to future behavior, and a correction or supersession path.

### Project-scoped continuity

Validate that project scope, checkpoints, and recall are enough to resume work
across sessions. Add a separate brief or working-memory primitive only if real
handoffs expose a gap in discovery or presentation.

### Measure usefulness

Add feedback tied to retrieval receipts. Store no raw prompts or transcripts.
The feedback should distinguish missing memory from failed retrieval and show
whether a memory was used, ignored, corrected, or followed by a reviewed
proposal.

Measure time to first cited recall, proposal approval and edit
rates, retrieved-and-used rate, stale-memory corrections, context cost per
used memory, and time to later reuse. These results should decide whether
Stormbuffer next needs better capture, retrieval, or maintenance.

## Later retrieval and maintenance

Use observed failures to choose the next retrieval or maintenance work. Likely
work includes proposing merges for duplicates, superseding stale memories,
archiving unused material, or improving ranking where the evaluation corpus and
real use show a repeatable miss. Stormbuffer should suggest these changes for
review rather than silently rewriting canonical memory.

Milestone 6 covers packages, model and cache diagnostics, generated completions
and man pages, and installation smoke tests.

## Later: local web editor

Milestone 7 covers the optional local web app. It will provide browsing, search,
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
- How an MCP server should select project and global scope when the caller omits
  it.
- Whether receipt feedback provides enough signal when it stores no user
  content.
- Whether scoped checkpoints cover cross-session handoffs or require a brief
  primitive.
- Which model acquisition and packaging approach supports semantic retrieval
  online and offline.
