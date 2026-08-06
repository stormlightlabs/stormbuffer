---
title: Quick start
description: >
  Initialize a private store, inspect its location, and keep project memory out of source control.
section: Get started
group: Get started
order: 2
---

Stormbuffer supports a global store for personal memory and a project store for work that
belongs to one repository.

## Initialize a store

For a user-wide store, run:

```sh
stormbuffer init
```

For a project-local store, run the command from the project directory:

```sh
stormbuffer --project init
```

Initialization creates the configured store structure without changing existing metadata.

## Inspect the store

Use `root` to print the resolved location and `status` to inspect the store without changing it:

```sh
stormbuffer root
stormbuffer status
```

A status report identifies the selected scope, root path, initialization state, and record count.
Add `--json` for machine-readable output.

## Create and manage a memory

`add` opens a temporary Markdown record in `$VISUAL` or `$EDITOR`:

```text
stormbuffer --project add --title "Release constraint" --kind fact
stormbuffer --project list
```

Use `show` to print the canonical Markdown, `edit` to change it safely, and `supersede`,
`archive`, or `restore` to retain lifecycle history. Active records appear in `list`.
Use `list --all` to include inactive records.

## Search your memory

Search active project memory and any initialized global memory with:

```sh
stormbuffer --project search release
```

Stormbuffer synchronizes the disposable search index before each search, so edits made directly
to the Markdown files appear in the results. Use `context` when another program needs a bounded,
machine-readable selection:

```sh
stormbuffer --project context release --budget 400
```

Semantic retrieval is local. `init` acquires the pinned fastembed model into the platform
cache, and project searches reuse that cache. If acquisition fails, the store remains valid and
the command reports how to repair the model.

## Keep project data private

Project memory lives under `.sbuf/` and is private by default. Add it to the project’s ignore
rules before creating records:

```sh
printf '%s\n' '.sbuf/' >> .gitignore
```

If a team shares records, review their source references and repository access policy first.
Keep secrets, raw transcripts, and generic project documentation out of the memory store.

To create a shared store:

```sh
stormbuffer --project init --shared
```

Commit only `.sbuf/store.toml`, `.sbuf/.gitignore`, and the canonical Markdown files under
`.sbuf/records/`.

The generated allowlist keeps indexes, models, locks, temporary files, and
other machine-local artifacts out of version control.
