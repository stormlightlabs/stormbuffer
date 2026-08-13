---
title: Codex
description: Install the Stormbuffer Codex plugin from source.
section: Integrations
group: Agent plugins
order: 4
---

The Codex plugin recalls relevant Stormbuffer records before the first model
call and asks Codex to consider durable capture after the turn. It includes the
Stormbuffer skill and an MCP server configured for candidate writes.

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

The bundled MCP server can recall memories and use `memory_remember` or
`memory_update` to propose a candidate for your review. A candidate is not
active until you approve it. The plugin cannot approve, activate, archive, or
perform destructive lifecycle operations through MCP.

After a successful candidate write through the bundled MCP server, a narrowly
matched `PostToolUse` hook records that capture already happened for the
current turn. The Stop hook consumes that signal and lets Codex finish instead
of asking it to review the same turn again. If no MCP candidate was written,
Stop remains a fallback capture boundary. Direct CLI writes are not tracked,
because doing so would require running a hook after every shell command.

That boundary fits the normal agent workflow: the agent proposes durable
corrections, decisions, constraints, preferences, procedures, and checkpoints;
you decide whether they become active memory.

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
