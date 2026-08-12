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

The maintained agent skill should express memory use as a small decision tree.
It recalls when prior durable context may affect the work, considers capture
only after a high-signal event, and otherwise takes no memory action. A capture
event permits evaluation; it does not guarantee a proposal.

The skill owns this semantic judgment. Stormbuffer core continues to enforce
scope, validation, lifecycle, and approval policy. This boundary keeps the
workflow inspectable without introducing a second worthiness classifier.

### Project-scoped continuity

Validate that project scope, checkpoints, and recall are enough to resume work
across sessions. Create a checkpoint only when normal project artifacts do not
preserve enough state for another session. It should record completed work, the
exact unresolved state, settled decisions, the next meaningful action, and
relevant references while omitting chronology and routine commands. Add a
separate brief or working-memory primitive only if real handoffs expose a
repeatable gap in discovery or presentation.

### Measure usefulness

Add feedback tied to retrieval receipts. Store no raw prompts or transcripts.
The feedback should distinguish knowledge that was never captured, memory that
retrieval missed, retrieved memory the agent ignored, and retrieved memory that
was stale or incorrect. It should also show whether a memory was used, cited,
corrected, or followed by a reviewed proposal.

Measure time to first cited recall, proposal approval, edit, rejection, and
duplicate rates, retrieved-and-used rate, stale-memory corrections, context
cost per used memory, and time to later reuse. Use repeatable failures to decide
whether Stormbuffer next needs better capture, retrieval, or maintenance.

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
- Whether receipt feedback provides enough signal when it stores no user
  content.
- Whether scoped checkpoints cover cross-session handoffs or require a brief
  primitive.
