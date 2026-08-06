---
title: CLI reference
description: Initialize a store, locate it, and inspect its state from the Stormbuffer command line.
section: Reference
group: CLI
order: 3
---

The Stormbuffer CLI is available as `stormbuffer`, `stormbuf`, or `sbuf`. Each name accepts the same commands and options.

## Choose a store

Stormbuffer uses a global store by default. Add `--project` to use the nearest `.stormbuffer/` directory instead:

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

Initialization creates the store if it does not exist. Running it again leaves an existing store unchanged.

## Locate a store

Print the resolved store path without initializing it:

```sh
stormbuffer root
stormbuffer --project root
```

## Inspect a store

`status` reports the selected scope, root path, initialization state, and record count:

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

The global `--project` option can appear before the command. The command-line help also accepts `--color auto|always|never` for human-facing output.
