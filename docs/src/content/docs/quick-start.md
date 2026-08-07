---
title: Quick start
description: >
  Initialize a private or shared store, inspect it, and retrieve bounded project memory.
section: Get started
group: Get started
order: 2
---

Stormbuffer supports a global store for personal memory and a project store for work that
belongs to one repository.

## Initialize a store

For a user-wide store, run:

```sh
sbuf init
```

For a project-local store, run the command from the project directory:

```sh
sbuf --project init --shared
```

Initialization creates the configured store structure without changing existing metadata.

## Inspect the store

Use `root` to print the resolved location and `status` to inspect the store without changing it:

```sh
sbuf root
sbuf status
```

A status report identifies the selected scope, root path, initialization state, and record count.
Add `--json` for machine-readable output.

## Create and manage a memory

`add` opens a temporary Markdown record in `$VISUAL` or `$EDITOR`:

```text
sbuf --project add --title "Release constraint" --kind fact
sbuf --project list
```

Use `show` to print the canonical Markdown, `edit` to change it safely, and `supersede`,
`archive`, or `restore` to retain lifecycle history. Active records appear in `list`.
Use `list --all` to include inactive records.

## Search your memory

Search active project memory and any initialized global memory with:

```sh
sbuf --project search release
```

Stormbuffer synchronizes the disposable search index before each search, so edits made directly
to the Markdown files appear in the results. Use `context` when another program needs a bounded,
machine-readable selection:

```sh
sbuf --project context release --budget 400
```

Semantic retrieval is local. Global `sbuf init` acquires the pinned fastembed model into
the platform cache, and project searches reuse that cache. Project initialization creates only the
project store. If model acquisition fails, the store remains valid and the command reports how to
repair it.

## Choose what to share

The quick start uses `--shared` to create a store that can travel with the repository. It writes
an allowlist that keeps indexes, models, locks, temporary files, and other machine-local artifacts
out of version control. Commit only `.sbuf/store.toml`, `.sbuf/.gitignore`, and the canonical
Markdown files under `.sbuf/records/`.

For personal project memory, omit `--shared` and arrange for `.sbuf/` to be ignored before
initialization. Do not use that approach to opt out of an existing shared store: its canonical
files are already tracked. Keep personal memory in the global store or a separate private checkout.

If a team shares records, review their source references and repository access policy first.
Keep secrets, raw transcripts, and generic project documentation out of the memory store.

## Try this repository's shared example

Stormbuffer itself includes a shared project store with canonical Markdown and no required
generated index. From a clean checkout, rebuild the disposable projection and retrieve a known
decision:

```sh
sbuf --project sync
sbuf --project search "canonical records projection failures"
sbuf --project context "What survives an index failure?" --budget 256
```

The search and context output should cite record
`019fd5d7-6e0c-7d93-b9fe-54b02f7f11e9`. Its source-backed answer is that canonical Markdown
survives projection failure and `sync` repairs the disposable index. If that record is absent,
the checkout does not contain the complete shared-store example.
