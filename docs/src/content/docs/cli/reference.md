---
title: CLI reference
description: >
  Initialize a store, locate it, and inspect its state from the Stormbuffer command line.
section: Reference
group: CLI
order: 3
---

The Stormbuffer CLI is available as `stormbuffer`, `stormbuf`, or `sbuf`.
Each name accepts the same commands and options.

## Choose a store

Stormbuffer uses a global store by default. Add `--project` to use the nearest `.sbuf/`
directory instead:

```sh
stormbuffer --project root
```

## Initialize a store

Initialize the global store with:

```sh
stormbuffer init
```

For project memory, run the command from the project directory:

```sh
stormbuffer --project init
```

Initialization creates the store if it does not exist.

Running it again leaves an existing store unchanged.

Project stores are private by default but you can opt into tracked configuration and canonical
Markdown explicitly:

```text
stormbuffer --project init --shared
```

## Locate a store

Print the resolved store path without initializing it:

```sh
stormbuffer root
stormbuffer --project root
```

## Inspect a store

`status` reports the selected scope, root path, initialization state, visibility, and record count:

```sh
stormbuffer status
stormbuffer --project status
```

Use `--json` when another program will consume the result:

```sh
stormbuffer --project status --json
stormbuf --project status
sbuf --project root
```

The global `--project` option can appear before the command.

Add `--shared` only to `--project init` to opt into tracked project memory.

The command-line help also accepts `--color auto|always|never` for human-facing output.

## Manage records

After initializing a store, `add` opens a temporary Markdown copy in `$VISUAL`, then `$EDITOR`.

The optional flags provide the initial frontmatter and body before editing:

```text
stormbuffer add --title "Deploy procedure" --kind procedure --body "Check the release health."
stormbuffer edit <id>
stormbuffer show <id>
```

`show` writes the canonical Markdown to stdout. `edit` accepts active records.

Restore an archived record before editing it, while superseded history remains immutable.

Editor output is parsed and validated before it replaces the record. If the canonical
file changed while it was open, the edit fails instead of overwriting the newer bytes.

`list` prints tab-delimited `id`, status, kind, scope, and title fields. It lists active
records by default.

Include archived and superseded records with `--all`:

```text
stormbuffer list
stormbuffer list --all
```

Lifecycle commands retain the Markdown history:

```text
stormbuffer supersede <id>
stormbuffer archive <id>
stormbuffer restore <id>
```

`supersede` creates a new active record and marks the old record superseded.

`archive` and `restore` change only the lifecycle status.

These commands print the affected ID and status on stdout.

## Permanently delete a record

`forget` is the only command that removes a canonical record.

It always requires `--destroy` where an interactive terminal also asks for confirmation.
Piped or scripted use must add `--yes`:

```text
stormbuffer forget <id> --destroy
stormbuffer forget <id> --destroy --yes
```

The mutation lock, validated temporary writes, file synchronization, and atomic replacement
keep competing or interrupted writes from exposing partial Markdown.
