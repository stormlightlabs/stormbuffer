---
title: How storage works
description: Understand what Stormbuffer keeps, what it can rebuild, and what you need to back up.
section: Concepts
group: Core concepts
order: 6
---

Stormbuffer keeps your memory local and readable by making Markdown-backed records the source
of truth. Search indexes, vector tables, model files, and caches are derived from them.

## Your records stay readable

Each memory is a Markdown file with TOML frontmatter. You can read, copy, inspect, and repair
those files with ordinary tools you usually use so your data is not trapped in a database or
hosted service.

Stormbuffer validates changes before replacing a record.

A failed indexing or cache update does not invalidate a saved record.

## Indexes are disposable

Search data, sqlite-vec tables, and model caches are projections of your records.

Stormbuffer can rebuild them from Markdown. They do not need to be part of your backup.
Semantic retrieval uses a verified local ONNX model; the core never sends record text to a
remote model.

Use `sbuf status` to see which store is selected. Back up that store's Markdown records.

## Global and project stores stay separate

The global store holds memory that follows you across projects.

A project store belongs to one repository and lives under `.sbuf/`.

Commands use the global store by default; add `--project` to select project memory explicitly.

Use `--project init --shared` when the repository should carry the store. Otherwise, add `.sbuf/`
to the repository's ignore rules before creating project memory.

## Retrieval projections

Lexical indexing runs during `sync`. Global `init` acquires the pinned fastembed
`AllMiniLML6V2` artifacts into the platform cache at `stormbuffer/models`; project stores reuse
that same cache.

Downloads are verified with pinned BLAKE3 checksums before fastembed loads them.
Search and context rebuild the versioned vector projection before use. A missing,
corrupt, or mismatched model fails with a repair instruction instead of falling
back silently.
