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

Use [The memory loop](/docs/concepts/memory-workflow/) to decide whether
knowledge belongs in Stormbuffer and which store should own it. This page covers
the record shape and lifecycle after that decision.

## Canonical record shape

A record begins with TOML frontmatter and then a Markdown body:

```toml
+++
format_version = 1
id = "01989af2-4305-7b19-88b1-e8ae4ea9a02b"
title = "Keep project memory out of source control"
kind = "decision"
scope = "project:01989af2-4305-7b19-88b1-e8ae4ea9a03b"
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
observed_at = "2026-08-05T20:09:00-05:00"
revision = "session-42"
content_hash = "blake3:8db6c6f72c33..."
+++

Project memory is private unless the team chooses to share it.
```

- `format_version` is required and must be `1`.
- Unknown frontmatter fields are rejected instead of being silently discarded.
- IDs are non-nil UUIDs.
- Scopes are `global` or `project:<project-id>`, access is `human` or `agent`, and source kinds
  are `conversation`, `document`, `issue`, or `url`.
- Timestamps use RFC 3339 and `updated_at` cannot precede `created_at`.

Each source may also include `observed_at`, `revision`, and `content_hash`.
These fields are optional: include only the freshness information the source
already provides, such as a Git revision or a stable file hash. Stormbuffer
preserves the values for later audits but does not crawl the source, detect
changes, or rewrite the memory automatically.

The core validates lifecycle transitions as `candidate → active`, `active → superseded|archived`,
and `archived → active` for restore. Superseded records are terminal.

The body is readable Markdown and is preserved exactly through parse/render round
trips, while frontmatter gives Stormbuffer the fields it needs for policy and retrieval.

For a project store, `.sbuf/store.toml` holds the stable project ID used in
record scopes and a separate editable project name. Renaming the repository or
changing that display name does not change its identity. Back up `store.toml`
with the Markdown records; the SQLite index can be rebuilt from them.

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
