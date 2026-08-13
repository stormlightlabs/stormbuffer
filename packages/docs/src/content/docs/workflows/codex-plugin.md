---
title: Codex plugin
description: Install the Stormbuffer Codex plugin from a source checkout.
section: Integrations
group: Agent plugins
order: 4
---

The Codex plugin recalls relevant Stormbuffer records before the first model
call and asks Codex to consider durable capture after the turn. It includes the
Stormbuffer skill and a read-only MCP server configuration.

Stormbuffer is not published yet. Codex must install this plugin from a local
source checkout.

## Before you install

Complete the [source build](/docs/workflows/source-build/), initialize the store
you intend to use, and confirm that `sbuf` is on `PATH`:

```sh
sbuf --version
sbuf status
```

## Install in Codex

From the Stormbuffer repository root, add its local marketplace and install the
plugin:

```sh
codex plugin marketplace add "$PWD"
codex plugin add stormbuffer@stormbuffer-source
```

The first command registers this checkout as a marketplace. The second installs
the plugin from `packages/codex-plugin-stormbuffer`. Restart Codex after the
installation so it loads the hooks, skill, and MCP configuration.

Confirm that Codex knows about the marketplace and plugin:

```sh
codex plugin marketplace list
codex plugin list
```

Codex plugins execute local code with your user permissions. Review the package
before installing it and keep the checkout in a location you control.

## Select a memory scope

The plugin uses the global store unless `STORMBUFFER_SCOPE` selects another
scope. Set the variable in the shell that starts Codex:

```sh
STORMBUFFER_SCOPE=project codex
```

The supported values match the CLI:

- `global` uses only the global store.
- `project` combines the nearest project store with applicable global memory.
- `local` uses only the nearest project store.

Start Codex inside the project whose store you want when using `project` or
`local`. Recall fails softly if the selected store is missing or unavailable.

## Candidate writes

The bundled MCP server is read-only by default. Recall never approves or
activates a record. When the capture check finds a durable correction,
decision, constraint, preference, procedure, or checkpoint, the skill can use
the versioned `sbuf invoke remember` or `update` flow to create a candidate for
your review.

## Update or remove the plugin

After pulling newer source, run `pnpm install --frozen-lockfile` again. Remove
and reinstall the plugin to refresh Codex's installed snapshot:

```sh
codex plugin remove stormbuffer@stormbuffer-source
codex plugin add stormbuffer@stormbuffer-source
```

To stop using the plugin, run the remove command. Remove the marketplace as
well if you no longer want Codex to track this checkout:

```sh
codex plugin marketplace remove stormbuffer-source
```
