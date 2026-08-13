---
title: The Memory Loop
description: Decide what the agent host, project store, and global store should remember.
section: Concepts
group: Core concepts
order: 4
---

Stormbuffer holds reviewed long-term memory for agents. The agent host retains
the current conversation and temporary task state. Stormbuffer keeps the
smaller set of sourced knowledge that should affect work in a later session.

## Choose where knowledge belongs

| Layer         | What it owns                                              | Examples                                                                        |
| ------------- | --------------------------------------------------------- | ------------------------------------------------------------------------------- |
| Agent host    | The current conversation and temporary working state      | Recent messages, tool output, a short-lived plan                                |
| Project store | Durable knowledge tied to one repository                  | Architecture decisions, local commands, constraints, resumable checkpoints      |
| Global store  | Durable knowledge that follows one person across projects | Stable preferences, cross-project procedures, recurring environment constraints |

Repository documentation remains the authority for facts it already explains.
A memory can point an agent to that material or explain how to apply it. Copying
the documentation into Stormbuffer creates another version to maintain.

## Use memory during work

1. Search once for the exact topic when prior context may affect the work.
2. Use applicable active records and cite the IDs that affect the answer or
   implementation.
3. Continue the task without recording the conversation or sweeping it for
   memories.
4. A correction, accepted decision, confirmed surprising cause, undocumented
   constraint, stale record, or necessary handoff can trigger one memory
   evaluation.
5. Propose one sourced candidate if the knowledge will change future work. A
   person approves, edits, or rejects it.
6. Supersede a stale record with its replacement so both the correction and its
   history remain visible.

A trigger permits evaluation; it does not require a new record. Routine success,
temporary progress, generic knowledge, duplicated documentation, speculation,
and secrets do not belong in the store. Most sessions should produce no
candidate.

## Write checkpoints for real handoffs

A project checkpoint is useful when another session cannot recover the current
state from repository files. Record what is complete, the exact unresolved
state, settled decisions, the next meaningful action, and links to the relevant
sources. Leave out chronology, routine commands, dead ends, and temporary
details.

Do not create a checkpoint when the repository already preserves everything
needed to continue. A checkpoint should reduce uncertainty for the next session,
not repeat a status file or task list.

## Review the store

Each active record should:

- make sense without the original conversation
- cover one fact, decision, procedure, or checkpoint
- name an attributable source
- live in the narrowest useful scope
- change a likely future decision or action
- be easy to correct, supersede, archive, or delete

Judge the store by later retrieval. An agent should find the relevant evidence
within the context budget, cite it, and use it. Record count says little if
agents cannot find or apply the contents.

See [How storage works](/docs/concepts/architecture/) for the canonical storage
boundary and [Data model](/docs/concepts/data-model/) for record kinds and
lifecycle.
