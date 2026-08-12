---
title: Quick start
description: Initialize a store, add a memory, and retrieve it later.
section: Get started
group: Get started
order: 2
---

This guide uses the global store, which is selected by default. It is the
shortest path from installation to a searchable memory.

## Initialize a store

```sh
sbuf init
```

## Inspect the store

Use `root` and `status` to confirm which store is selected:

```sh
sbuf root
sbuf status
```

Add `--json` to `status` for machine-readable output.

## Create and manage a memory

`add` opens a Markdown record in `$VISUAL` or `$EDITOR`:

```text
sbuf add --title "Release constraint" --kind fact
sbuf list
```

Save one durable claim and include its source. `list` shows active records. The
[CLI reference](/docs/cli/reference/) covers editing and lifecycle commands.

## Search your memory

Search active memory with:

```sh
sbuf search release
```

Use `context` when another program needs a machine-readable selection within a
token budget:

```sh
sbuf context release --budget 400
```

Search results cite the matching record. Context output preserves those
citations while selecting only what fits the requested budget.

## Use a project store

When knowledge belongs to one repository, run the same commands with
`--project` from that repository:

```sh
sbuf --project init
sbuf --project status
```

See [The memory loop](/docs/concepts/memory-workflow/) for what belongs in each
store. [Backup and recovery](/docs/workflows/backup-recovery/) explains how to
keep project memory private or share a curated store with a repository.
