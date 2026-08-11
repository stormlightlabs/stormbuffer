---
title: Data model
description: Store sourced memories as Markdown with typed TOML frontmatter.
section: Concepts
group: Core concepts
order: 5
---

Markdown is authoritative. TOML frontmatter carries the fields needed for validation, policy,
and retrieval. The body contains the user-authored content.

## Four memory kinds

| Kind         | Use it for                                   |
| ------------ | -------------------------------------------- |
| `fact`       | Durable facts, constraints, and preferences. |
| `decision`   | A choice and the rationale behind it.        |
| `procedure`  | Reusable instructions or workflows.          |
| `checkpoint` | Current state of an ongoing project.         |

## What belongs in memory

A memory should make sense on its own, apply to its user or project, cite a
source, and cover one claim or procedure. Record it when it can change a future
decision or action and can be corrected when the source changes.

Good candidates often come from a user correction, an accepted decision and its
rationale, a confirmed surprising root cause, an undocumented constraint, or
the discovery that a stored memory is stale. Raw transcripts, generic
knowledge, routine task progress, unsupported inference, duplicate
documentation, and secrets do not belong in the store.

The agent host owns recent conversation and fleeting task state. A checkpoint
is appropriate when another session needs a sourced account of the current
state to resume the work.

## Canonical record shape

A record begins with TOML frontmatter and then a Markdown body:

```toml
+++
format_version = 1
id = "01989af2-4305-7b19-88b1-e8ae4ea9a02b"
title = "Keep project memory out of source control"
kind = "decision"
scope = "project:stormbuffer"
status = "active"
access = "agent"
created_at = "2026-08-05T20:09:00-05:00"
updated_at = "2026-08-05T20:09:00-05:00"
tags = ["privacy", "source-control"]
aliases = ["ignore project memory"]
supersedes = []

[[sources]]
kind = "conversation"
reference = "stormbuffer://session/2026-08-05"
actor = "user"
+++

Project memory is private unless the team chooses to share it.
```

- `format_version` is required and must be `1`.
- Unknown frontmatter fields are rejected instead of being silently discarded.
- IDs are non-nil UUIDs.
- Scopes are `global` or `project:<name>`, access is `human` or `agent`, and source kinds
  are `conversation`, `document`, `issue`, or `url`.
- Timestamps use RFC 3339 and `updated_at` cannot precede `created_at`.

The core validates lifecycle transitions as `candidate → active`, `active → superseded|archived`,
and `archived → active` for restore. Superseded records are terminal.

The body is readable Markdown and is preserved exactly through parse/render round
trips, while frontmatter gives Stormbuffer the fields it needs for policy and retrieval.

## Lifecycle and boundaries

Agent-created records normally begin as candidates.

Human-authored records can become active immediately.

The normal lifecycle is:

```text
candidate → active → superseded
                    ↘ archived
```

Supersession retains history and does not rewrite the old claim. Normal retrieval excludes
superseded and archived records. Permanent deletion requires `forget --destroy`.
Noninteractive use must also pass `--yes`.
