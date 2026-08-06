---
title: Quick start
description: Initialize a private store, inspect its location, and keep project memory out of source control.
section: Get started
group: Get started
order: 2
---

Stormbuffer supports a global store for personal memory and a project store for work that belongs to one repository.

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

A status report identifies the selected scope, root path, initialization state, and record count. Add `--json` for machine-readable output.

## Keep project data private

Project memory lives under `.stormbuffer/` and is private by default. Add it to the project’s ignore rules before creating records:

```sh
printf '%s\n' '.stormbuffer/' >> .gitignore
```

If a team shares records, review their source references and repository access policy first. Keep secrets, raw transcripts, and generic project documentation out of the memory store.
