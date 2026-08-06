---
title: Data model
description: >
  Store small, sourced memories as portable Markdown with a typed TOML frontmatter contract.
section: Concepts
group: Core concepts
order: 5
---

Markdown is authoritative with TOML frontmatter to carry the fields needed for validation,
policy, and retrieval; the body stays readable and user-authored.

## Four memory kinds

| Kind         | Use it for                                   |
| ------------ | -------------------------------------------- |
| `fact`       | Durable facts, constraints, and preferences. |
| `decision`   | A choice and the rationale behind it.        |
| `procedure`  | Reusable instructions or workflows.          |
| `checkpoint` | Current state of an ongoing project.         |

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

Project memory stays private unless the team deliberately chooses to share it.
```

- `format_version` is required and must be `1`.
- Unknown frontmatter fields are rejected instead of being silently discarded.
- IDs are non-nil UUIDs.
- Scopes are `global` or `project:<name>`, access is `human` or `agent`, and source kinds
  are `conversation`, `document`, `issue`, or `url`.
- Timestamps use RFC 3339 and `updated_at` cannot precede `created_at`.

The core validates lifecycle transitions as `candidate → active`, `active → superseded|archived`,
and `archived → active` for restore; superseded records are terminal.

The body remains readable Markdown and is preserved exactly through parse/render round
trips, while frontmatter gives Stormbuffer the fields it needs for policy and retrieval.

## Lifecycle and boundaries

Agent-created records normally begin as candidates.

Human-authored records can become active immediately.

The normal lifecycle is:

```text
candidate → active → superseded
                    ↘ archived
```

Supersession retains history and does not silently rewrite the old claim. Normal
retrieval excludes superseded and archived records. Permanent deletion is a separate,
deliberate `forget --destroy` operation. Noninteractive use must also pass `--yes`.

Useful memory is independently understandable, specific to its user or project, backed by
a source, and small enough to retrieve as a unit. Raw transcripts, generic knowledge,
fleeting task state, unsupported inference, duplicate documentation, and secrets stay
out of the store.
