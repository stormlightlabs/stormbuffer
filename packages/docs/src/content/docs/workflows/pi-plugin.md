---
title: Pi
description: Install the Stormbuffer Pi plugin from source.
section: Integrations
group: Agent plugins
order: 5
---

The Pi package recalls relevant Stormbuffer records in `before_agent_start` and
adds them as hidden context for the turn. After Pi reports the agent settled,
the extension asks the model once whether the completed work warrants durable
capture. The package also installs the Stormbuffer skill.

Stormbuffer is not published yet. Pi must load this package from a local source
checkout.

## Before you install

Complete the [source build](/docs/workflows/source-build/), initialize the store
you intend to use, and confirm that `sbuf` is on `PATH`:

```sh
sbuf --version
sbuf status
```

## Install for your user

From the Stormbuffer repository root, install the local package directory:

```sh
pi install "$PWD/packages/pi-plugin-stormbuffer"
```

Pi records the local path without copying the package. Keep the checkout in the
same location and retain its pnpm workspace dependencies. Restart Pi after the
installation, then confirm the package appears:

```sh
pi list
```

Pi packages execute local code with your user permissions. Review the package
before installing it and keep the checkout in a location you control.

For a project-local installation, run the command from that project and add
`-l`. Use an absolute path to the Stormbuffer package:

```sh
pi install -l /absolute/path/to/stormbuffer/packages/pi-plugin-stormbuffer
```

Pi asks you to trust a project before it loads project-local extensions.

## Select a memory scope

The package uses the global store unless `STORMBUFFER_SCOPE` selects another
scope. Set the variable in the shell that starts Pi:

```sh
STORMBUFFER_SCOPE=project pi
```

The supported values match the CLI:

- `global` uses only the global store.
- `project` combines the nearest project store with applicable global memory.
- `local` uses only the nearest project store.

Start Pi inside the project whose store you want when using `project` or
`local`. Recall fails softly if the selected store is missing or unavailable.

## Candidate writes

The extension does not start or configure an MCP server, and installing the
package does not grant write access. The installed skill can create candidates
through the versioned `sbuf invoke remember` and `update` commands when Pi has
shell access.

When Pi connects through `pi-mcp-adapter`, configure the Stormbuffer server for
candidate writes and restart Pi. The [MCP reference](/docs/reference/mcp/)
provides the global and project configuration, access limits, and verification
steps.

## Avoid duplicate skills

The package includes the `stormbuffer-memory` skill. If Pi already loads a skill
with that name from `~/.agents/skills`, `.agents/skills`, or another package, it
warns about the collision and keeps the first copy it discovered. Keep one copy,
or use `pi config` to disable the package's skill while leaving its extension
enabled.

`stormbuffer-global-memory` has a different name and can coexist with the
package skill. Installing that global skill in both a user and project skill
directory still creates a collision; keep it in only one discovered location.

## Update or remove the package

After pulling newer source, run `pnpm install --frozen-lockfile` again. Restart
Pi or enter `/reload` so it reloads the local extension and skill.

Remove the package with the same local path recorded at installation:

```sh
pi remove /absolute/path/to/stormbuffer/packages/pi-plugin-stormbuffer
```
