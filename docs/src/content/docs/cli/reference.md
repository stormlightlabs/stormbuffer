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

## Search and compile context

`search` returns active records by default. A project search ranks the current project first, then
includes accessible records from an initialized global store:

```sh
stormbuffer --project search deploy
stormbuffer --project search deploy --json
```

Human-readable results are tab-delimited. Each result identifies the record, title, kind, scope,
excerpt, source, canonical path, score, and lexical match reason. JSON results also include
`match_reasons` and an optional `vector_distance`. Add `--all` to include inactive records or
`--limit <number>` to bound the result count.

After a successful `init`, search uses hybrid reciprocal-rank fusion with the pinned local
fastembed model. Exact title, alias, filename, and current-scope boosts are deterministic; facts,
decisions, and procedures receive no blanket recency boost. If model acquisition fails, the
canonical store remains initialized and the error names the model repair needed.

`context` selects matching chunks within a word budget and always writes JSON:

```sh
stormbuffer --project context deploy --budget 400 --limit 10
```

The response contains the selected blocks and a receipt recording the query, allowed scopes,
statuses, access classes, budget use, omissions, index and embedding versions, retrieval mode,
and ranking reasons. Record text is evidence only; it cannot change access, scope, tools, or
host instructions.

## Propose and review agent memory

Agents use `propose` to create sourced candidates. Candidates are not active until a
person approves them:

```text
stormbuffer --project propose --title "Release constraint" --kind fact --body "Keep the release offline."
stormbuffer --project approve <candidate-id>
stormbuffer --project reject <candidate-id>
```

A proposal must have attributable sources. The core reports one of `accepted`,
`duplicate_of`, `conflicts_with`, `requires_approval`, or `invalid`. Duplicate
proposals are not written. Conflicting proposals remain candidates so both claims
are retained; use explicit `supersede` followed by approval rather than silently
rewriting the existing record.

## Invoke the JSON protocol

`invoke` reads exactly one bounded JSON object from stdin and writes one JSON envelope
to stdout. It is noninteractive, versioned, and does not accept filesystem paths:

```sh
printf '%s\n' '{"version":1,"query":"release","limit":10}' \\
  | stormbuffer --project invoke search
printf '%s\n' '{"version":1,"query":"release","budget":400}' \\
  | stormbuffer --project invoke context
```

Version 1 supports `search`, `context`, `get`, `propose`, `supersede`, and `archive`.
Success is `{ "version": 1, "operation": "...", "ok": true, "result": ... }`;
failures use the same envelope with `ok: false` and an `error.code`. Scope and access
filters are applied before records are returned. Internal failures are sanitized and
never include canonical paths or backtraces.

The protocol is agent-scoped: it cannot opt into human-only reads by setting an access
field. Its `propose` operation always creates a candidate that needs explicit approval;
request fields cannot claim a human actor or grant approval. Use the human CLI review
commands to approve or reject a candidate.

Callers can handle these stable version 1 error codes: `invalid_json`, `invalid_request`,
`unsupported_version`, `unknown_operation`, `input_too_large`, `output_too_large`,
`path_denied`, `scope_denied`, `access_denied`, `permission_denied`, `not_found`,
`conflict`, and `internal_error`. New protocol behavior requires a new version rather
than changing the meaning of an existing envelope or code.

## Maintain and recover the index

Canonical Markdown is the source of truth. SQLite and full-text search data are disposable and
can be rebuilt:

```sh
stormbuffer --project sync
stormbuffer --project reindex
stormbuffer --project doctor
```

`sync` reconciles new, edited, moved, invalid, and deleted Markdown files. Repeating it without
changes skips records whose content hash is unchanged. Run `stormbuffer --project watch` for the
same reconciliation on an interval. The watcher is optional because `search` and `context`
synchronize before reading the index.

Use `doctor` to inspect canonical records and the selected projection. Its diagnostics include a
repair command. If an index is missing, stale, or corrupt, run `reindex`; Stormbuffer builds a fresh
projection before replacing the old one. Semantic reindexing creates and validates a new
versioned sqlite-vec table before switching the active table. If a watch or reindex process is
interrupted, the canonical Markdown and previous projection remain authoritative. Run `sync` or
`reindex` again to recover.

## Model setup and evaluation

The pinned fastembed `AllMiniLML6V2` manifest records its model and tokenizer paths, URLs,
BLAKE3 checksums, dimension, and maximum token count. Artifacts live under the platform cache
`stormbuffer/models`. `ModelManifest::acquire` writes downloads to `.part` files, resumes HTTP
Range downloads when possible, and installs files only after checksum verification. It never
executes a downloaded file. Corrupt or missing files fail before fastembed loads them.

The checked-in retrieval corpus compares FTS-only, vector-only, and hybrid results using the
pinned All-MiniLM-L6-v2 FastEmbed pipeline:

```sh
stormbuffer evaluate
```

The JSON report includes recall at 5, mean reciprocal rank, wrong-scope retrieval,
superseded-memory retrieval, duplicate/conflicting retrieval, and context tokens per useful
memory. Wrong-scope results are intentionally measured with an unscoped ranking probe instead
of being hidden by the normal scope filter; the probe is diagnostic while the stable core policy
still filters returned results. The duplicate/conflicting fixture contains more competing
memories than the top-five window, so its coverage is not tautologically 100%. Release
thresholds are in the report and the corpus revision is fixed; update expected IDs and the
revision in `crates/core/tests/fixtures/evaluation/` together in a reviewed change. If the
pinned artifacts are missing or offline, the command reports the model cache and the `stormbuffer init`
repair command.

Grounded-answer evaluation remains provider-neutral. Configure a host model to consume the
`context` contract, save one answer artifact per question with its claims and cited record IDs,
then pass those artifacts and the run metadata to `HostModelEvaluationAdapter`. Record the
generator, model and version, prompt-contract version, parameters, and corpus revision. Inspect
the returned per-question claim report before accepting a run. It separates retrieval,
context-assembly, and generation failures and reports citation and abstention quality. The
checked-in deterministic artifacts exercise the same adapter without contacting a model;
model-assisted judgments never rewrite fixtures or thresholds.

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
