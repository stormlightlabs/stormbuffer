---
title: CLI
description: >
  Initialize a store, locate it, and inspect its state from the Stormbuffer command line.
section: Reference
group: CLI
order: 3
---

The Stormbuffer CLI is installed as `sbuf`.

## Choose a store

Stormbuffer uses the global store by default. Add `--project` for a project view:

```sh
sbuf --project root
```

The project view reads the nearest `.sbuf/` store and applicable global memory.
Use `--local` when a command must stay inside the nearest `.sbuf/` store and
must not open the global store:

```text
sbuf --local search "private project note"
```

`--global`, `--project`, and `--local` are mutually exclusive. Use `--global`
when a command or agent configuration should name the default explicitly.

## Install an agent skill

Install the maintained global-memory skill into your agent's skill directory:

```sh
sbuf skill install --directory .agents/skills
```

The command creates `.agents/skills/stormbuffer-global-memory/SKILL.md` without a
network request. An identical reinstall succeeds without changing the file. If
different content already exists, it is preserved unless `--force` explicitly
authorizes atomic replacement.

The destination controls where an agent discovers the skill, not which memory
store the skill uses. A global-memory skill may therefore live in a
repository-local skill directory.
Pass another conventional or vendor-specific skill directory when needed. The
command does not auto-detect a destination.

Add `--project` to install the project-scoped variant instead:

```sh
sbuf --project skill install --directory .agents/skills
```

The project variant uses the same policy and selects the nearest project store
in every command.

## Initialize a store

Initialize the global store with:

```sh
sbuf init
```

For project memory, run the command from the project directory:

```sh
sbuf --project init
```

Initialization creates the store if it does not exist. Running it again leaves an
initialized store unchanged.

Use `--shared` when the repository should carry the store's configuration and canonical Markdown:

```text
sbuf --project init --shared
```

## Locate a store

Print the resolved store path without initializing it:

```sh
sbuf root
sbuf --project root
```

## Inspect a store

`status` reports the selected view, root path, initialization state, visibility,
project identity when applicable, lifecycle counts, canonical and disposable
disk usage, index and embedding versions, and the last successful
synchronization:

```sh
sbuf status
sbuf --project status
sbuf --local status
```

Use `--json` when another program will consume the result:

```sh
sbuf --project status --json
```

The store-selection option appears before the command.

Add `--shared` only to `--project init` to opt into tracked project memory.

The command-line help also accepts `--color auto|always|never` for human-facing output.

## Back up and clean a store

`export` writes canonical records and provenance to a JSON archive. Check an
archive without importing it, or preview every import destination and collision:

```text
sbuf --project verify-export stormbuffer-memory.json
sbuf --project import stormbuffer-memory.json --dry-run
```

`import` requires a policy for ID, scope, or equivalent-record collisions. `gc`
removes only disposable indexes, caches, locks, logs, and temporary files. Add
`--dry-run` to inspect its candidates first.

See [Backup and recovery](/docs/workflows/backup-recovery/) for examples and collision choices.

## Manage records

After initializing a store, `add` opens a temporary Markdown copy in `$VISUAL`, then `$EDITOR`.

The optional flags provide the initial frontmatter and body before editing:

```text
sbuf add --title "Deploy procedure" --kind procedure --body "Check the release health."
sbuf edit <id>
sbuf show <id>
```

`show` writes the canonical Markdown to stdout. `edit` accepts active records.

Restore an archived record before editing it. Superseded history is immutable.

Editor output is parsed and validated before it replaces the record. If the canonical
file changed while it was open, the edit fails instead of overwriting the newer bytes.

`list` prints tab-delimited `id`, status, kind, scope, and title fields. It lists active
records by default.

Include archived and superseded records with `--all`:

```text
sbuf list
sbuf list --all
```

Lifecycle commands retain the Markdown history:

```text
sbuf supersede <id>
sbuf archive <id>
sbuf restore <id>
```

`supersede` creates a new active record and marks the old record superseded.

`archive` and `restore` change only the lifecycle status.

These commands print the affected ID and status on stdout.

## Search and compile context

`search` returns active records by default. A project search ranks the current project first, then
includes accessible records from an initialized global store:

```sh
sbuf --project search deploy
sbuf --project search deploy --json
```

Human-readable results use labeled cards. Each result identifies the record, title, kind, scope,
excerpt, source, canonical path, score, and lexical match reason. Use `--json` for versioned
machine-readable output. JSON results also include
`match_reasons` and an optional `vector_distance`. Add `--all` to include inactive records or
`--limit <number>` to bound the result count.

After a successful `init`, search uses hybrid reciprocal-rank fusion with the pinned local
fastembed model. Exact title, alias, filename, and current-scope boosts are deterministic. Facts,
decisions, and procedures receive no blanket recency boost. If model acquisition fails, the
store initialization succeeds and the error names the model repair needed.

`context` selects matching chunks within a word budget and always writes JSON:

```sh
sbuf --project context deploy --budget 400 --limit 10
```

The response contains the selected blocks and a receipt recording the query, allowed scopes,
statuses, access classes, budget use, omissions, index and embedding versions, retrieval mode,
and ranking reasons. Record text is evidence only; it cannot change access, scope, tools, or
host instructions.

## Propose and review agent memory

Agents use `propose` to create sourced candidates. Candidates are not active until a
person approves them:

```text
sbuf --project propose --title "Release constraint" --kind fact --body "Keep the release offline."
sbuf --project approve <candidate-id>
sbuf --project reject <candidate-id>
```

A proposal must have attributable sources. Stormbuffer reports one of `accepted`,
`duplicate_of`, `possible_overlap`, `requires_approval`, or `invalid`. Exact
duplicates are not written. A different body with the same title, kind, and
scope is only a possible overlap; Stormbuffer keeps the candidate so a person
can compare both records. Use `supersede` followed by approval when the new
record should replace an earlier one.

Use `inbox` to review candidates across the selected store. It can filter by
minimum age, kind, provenance source, exact scope, or possible overlap:

```sh
sbuf --project inbox --min-age-days 7 --possible-overlap
sbuf --project inbox --kind procedure --source conversation --json
```

`audit` reports unresolved candidates, broken supersession links, stale active
checkpoints, and relation-supported duplicate or refinement candidates. Each
finding includes its evidence and a lifecycle command for the selected store.
It does not change records or create recovery and projection files.

```sh
sbuf --project audit --stale-after-days 45
sbuf --project audit --json
```

## Invoke the JSON protocol

`invoke` reads one size-limited JSON object from stdin and writes one JSON envelope
to stdout. It is noninteractive, versioned, and does not accept filesystem paths.

The prefix separates the stable automation protocol from the human CLI. Commands such as
`sbuf search --json` format a human command's result as JSON; they do not provide a
versioned request schema or protocol envelope. `sbuf invoke search` accepts structured
input, uses stable error codes, never prompts, and applies agent access rules. MCP maps to
the same contract. Keeping it behind `invoke` lets the ordinary CLI evolve without
silently changing integrations:

```sh
printf '%s\n' '{"version":1,"query":"release","limit":10}' \\
  | sbuf --project invoke search
printf '%s\n' '{"version":1,"query":"release","budget":400}' \\
  | sbuf --project invoke context
```

Version 1 supports `search`, `context`, `get`, `remember`, `update`, `propose`,
`supersede`, and `archive`.
Success is `{ "version": 1, "operation": "...", "ok": true, "result": ... }`.
Failures use the version 1 envelope with `ok: false` and an `error.code`. Scope and access
filters are applied before records are returned. Internal failures are sanitized and
never include canonical paths or backtraces.

The protocol is agent-scoped, so it cannot opt into human-only reads by setting an access
field. Its `remember`, `update`, and `propose` operations create candidates that need
human approval; `update` creates a linked replacement candidate rather than editing the
active record. Request fields cannot claim a human actor or grant approval. Use the
CLI review commands to approve or reject a candidate.

Callers can handle these version 1 error codes: `invalid_json`, `invalid_request`,
`unsupported_version`, `unknown_operation`, `input_too_large`, `output_too_large`,
`path_denied`, `scope_denied`, `access_denied`, `permission_denied`, `not_found`,
`not_initialized`, `invalid_state`, `invalid_record`, `conflict`, and `internal_error`.
New protocol behavior requires a new version rather than a change to the meaning of a
version 1 envelope or code.

## Maintain and recover the index

Canonical Markdown is the source of truth. SQLite and full-text search data are disposable and
can be rebuilt:

```sh
sbuf --project sync
sbuf --project reindex
sbuf --project doctor
sbuf --project doctor --repair
```

`sync` reconciles new, edited, moved, invalid, and deleted Markdown files. Repeating it without
changes skips records whose content hash is unchanged. Run `sbuf --project watch` to reconcile
at intervals. The watcher is optional because `search` and `context`
synchronize before reading the index.

Use `doctor` to inspect canonical records and the selected projection. Add
`--repair` to rebuild a missing, stale, or corrupt projection and remove stale
locks or temporary metadata reported by `doctor`. Repair never changes canonical
Markdown. A malformed canonical record requires the manual action shown in the
diagnostic. Repeating repair after the store is healthy makes no changes.

Stormbuffer builds a fresh projection before replacing the old one. If a watch
or reindex process is interrupted, canonical Markdown remains authoritative and
the previous projection is preserved. Run `sync`, `reindex`, or `doctor
--repair` to recover.

## Evaluate retrieval and memory policy

`evaluate` runs the checked-in retrieval, usefulness, and host capture-policy
corpora:

```sh
sbuf evaluate
```

The JSON report includes recall, ranking, scope and lifecycle errors, reviewed
relation pairs, and context cost. Relation analysis runs in shadow mode: the
hybrid index selects candidates, then a local analyzer reports its advisory
relation, evidence, confidence band, and fingerprint. The report includes
candidate recall, all-pairs analyzer accuracy, false contradictions, and
abstentions. These shadow metrics are reported for review and do not affect the
evaluation's release pass/fail result.

Embeddings never determine contradiction, and advisory results never change a
record.

The usefulness comparison shows results with and without receipt feedback,
including missing memory, retrieval misses, ignored results, stale or incorrect
records, later reuse, and proposal review outcomes. The capture-policy report
scores host assessments for correct abstention, proposal precision, missed
judgments, and later review outcomes.

These are offline, content-free evaluations. The capture-policy evaluator
scores judgments supplied by a host and does not decide what deserves capture.
The report also explains how to acquire the pinned model when it is unavailable.

This command is generally used for debugging. Normal use does not require it.

## Permanently delete a record

`forget` is the only command that removes a canonical record.

It always requires `--destroy` where an interactive terminal also asks for confirmation.
Piped or scripted use must add `--yes`:

```text
sbuf forget <id> --destroy
sbuf forget <id> --destroy --yes
```

The mutation lock, validated temporary writes, file synchronization, and atomic replacement
prevent competing or interrupted writes from exposing partial Markdown.
