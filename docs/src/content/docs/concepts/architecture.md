---
title: How storage works
description: Understand canonical records, disposable projections, and backups.
section: Concepts
group: Core concepts
order: 6
---

Stormbuffer stores memory as Markdown-backed records. Search indexes, vector tables, model
files, and caches are derived from those records.

## Records are Markdown

Each memory is a Markdown file with TOML frontmatter. You can read, copy, inspect, and repair
those files with ordinary text tools. The records do not depend on a database or hosted service.

Stormbuffer validates changes before replacing a record.

A failed indexing or cache update does not invalidate a saved record.

## Indexes are disposable

Search data, sqlite-vec tables, and model caches are projections of your records.

Stormbuffer can rebuild them from Markdown, so backups only need the records and store
configuration. Semantic retrieval uses a verified local ONNX model. The core never sends record
text to a remote model.

Use `sbuf status` to see which store is selected. Back up that store's Markdown records.

## Global and project stores

The global store holds memory that follows you across projects.

A project store belongs to one repository and lives under `.sbuf/`.

Commands use the global store by default. Add `--project` to select project memory.

Use `--project init --shared` when the repository should carry the store. Otherwise, add `.sbuf/`
to the repository's ignore rules before creating project memory.

## Retrieval projections

Lexical indexing runs during `sync`. Global `init` acquires the pinned fastembed
`AllMiniLML6V2` artifacts into the platform cache at `stormbuffer/models`. Project stores reuse
the platform cache.

Downloads are verified with pinned BLAKE3 checksums before fastembed loads them.
Search and context rebuild the versioned vector projection before use. A missing,
corrupt, or mismatched model fails with a repair instruction instead of falling
back silently.

## Retrieval and context

Stormbuffer combines lexical and semantic search, then compiles selected records
into evidence blocks within a caller-provided budget. Each block retains its record and chunk IDs,
scope, lifecycle state, source references, and selected text. A receipt records
the query, filters, ranking details, omissions, and truncation so a caller can
explain what retrieval returned.

Search exposes ranked records for inspection. Context assembly prepares
attributable evidence within a caller-provided budget. Agent-facing recall uses
this retrieval and context path rather than implementing another ranking
system.

## Stormbuffer and the agent host

Stormbuffer owns storage, retrieval, lifecycle policy, and context assembly. The
host owns recent conversation, instructions, and generation. Retrieved record
bodies are untrusted evidence. Text inside them cannot grant tools, widen scope,
or override the user's request.
