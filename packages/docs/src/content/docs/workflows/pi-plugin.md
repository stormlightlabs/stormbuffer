---
title: Pi plugin
description: Install the Stormbuffer Pi plugin from a source checkout.
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

The extension does not write or approve records. When its capture check finds a
durable correction, decision, constraint, preference, procedure, or checkpoint,
the installed skill can use the versioned `sbuf invoke remember` or `update`
flow to create a candidate for your review.

## Update or remove the package

After pulling newer source, run `pnpm install --frozen-lockfile` again. Restart
Pi or enter `/reload` so it reloads the local extension and skill.

Remove the package with the same local path recorded at installation:

```sh
pi remove /absolute/path/to/stormbuffer/packages/pi-plugin-stormbuffer
```
