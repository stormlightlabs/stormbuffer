---
title: How storage works
description: Understand what Stormbuffer keeps, what it can rebuild, and what you need to back up.
section: Concepts
group: Core concepts
order: 6
---

Stormbuffer keeps your memory local and readable by making Markdown-backed records the source
of truth. Search indexes and caches are derived from them.

## Your records stay readable

Each memory is a Markdown file with TOML frontmatter. You can read, copy, inspect, and repair
those files with ordinary tools you usually use so your data is not trapped in a database or
hosted service.

Stormbuffer validates changes before replacing a record.

A failed indexing or cache update does not invalidate a saved record.

## Indexes are disposable

Search data and model caches are projections of your records.

Stormbuffer can rebuild them from Markdown, so they do not need to be part of your backup.

Use `stormbuffer status` to see which store is selected. Back up that store's Markdown records.

## Global and project stores stay separate

The global store holds memory that follows you across projects.

A project store belongs to one repository and lives under `.sbuf/`.

Commands use the global store by default; add `--project` to select project memory explicitly.

Project stores are private unless initialized with `--project init --shared`.
