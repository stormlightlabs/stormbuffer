---
title: Quick start
description: >
  Initialize a private or shared store, inspect it, and retrieve project memory
  within a token budget.
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

Initialization creates the configured store structure and leaves stored metadata unchanged.

## Inspect the store

`root` and `status` are read-only. Use them to print the resolved location and inspect the store:

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

Use `show` to print the canonical Markdown and `edit` to validate and replace it atomically.
Use `supersede`, `archive`, or `restore` to retain lifecycle history. Active records appear in `list`.
Use `list --all` to include inactive records.

## Search your memory

Search active project memory and any initialized global memory with:

```sh
sbuf --project search release
```

Stormbuffer synchronizes the disposable search index before each search, so edits made directly
to the Markdown files appear in the results. Use `context` when another program needs a
machine-readable selection capped by a token budget:

```sh
sbuf --project context release --budget 400
```

Semantic retrieval is local. Global `sbuf init` acquires the pinned fastembed model into
the platform cache, and project searches reuse that cache. Project initialization creates only the
project store. A failed model download does not affect stored Markdown. The command reports how
to repair the model cache.

## Choose what to share

The quick start uses `--shared` to create a store that can travel with the repository. It writes
an allowlist that excludes indexes, models, locks, temporary files, and other machine-local
artifacts from version control. Commit only `.sbuf/store.toml`, `.sbuf/.gitignore`, and the
canonical Markdown files under `.sbuf/records/`.

For personal project memory, omit `--shared` and arrange for `.sbuf/` to be ignored before
initialization. This does not opt a checkout out of a shared store because its canonical files are
already tracked. Put personal memory in the global store or a separate private checkout.

If a team shares records, review their source references and repository access policy first.
Do not store secrets, raw transcripts, or generic project documentation.

## Try this repository's shared example

Stormbuffer itself includes a shared project store with canonical Markdown and no required
generated index. From a fresh checkout, rebuild the disposable projection and retrieve a known
decision:

```sh
sbuf --project sync
sbuf --project search "canonical records projection failures"
sbuf --project context "What survives an index failure?" --budget 256
```

The search and context output should cite record
`019fd5d7-6e0c-7d93-b9fe-54b02f7f11e9`. Its source-backed answer is that canonical Markdown
survives projection failure and `sync` repairs the disposable index. If that record is absent,
the shared-store record was not checked in.
