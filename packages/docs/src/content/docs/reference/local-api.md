---
title: Local API
description: Run and integrate with Stormbuffer's loopback-only HTTP API.
section: Reference
group: Local API
order: 5
---

`sbuf serve` runs the local API for the selected store. It is intended for the
local web editor and other software on the same machine.

```sh
sbuf serve
sbuf --project serve --port 7343
sbuf --local serve --bind ::1
```

The default address is `http://127.0.0.1:7342`. `--bind` accepts only loopback
addresses. Stormbuffer refuses wildcard and remote addresses; remote access is
not supported until authentication and a threat model exist.

The foreground process logs startup, requests, errors, and shutdown to stderr.
It handles Ctrl-C and `SIGTERM`, stops accepting new connections, and lets
in-flight core operations finish. Use a service manager to restart it if the
machine restarts.

The selected scope works like the CLI. `--project` searches the nearest project
store plus applicable global records, while record browsing and mutation remain
inside the selected canonical store. The server never edits Markdown or SQLite
directly; every operation calls `stormbuffer-core`.

## API contract

The generated OpenAPI 3 document is available at:

```text
GET /openapi.json
```

All application endpoints are versioned under `/v1`:

| Method | Endpoint                                  | Operation                                 |
| ------ | ----------------------------------------- | ----------------------------------------- |
| `GET`  | `/v1/records?all=false`                   | List canonical records.                   |
| `POST` | `/v1/records`                             | Add a human-authored record.              |
| `GET`  | `/v1/records/{id}`                        | Read one record and its ETag.             |
| `PUT`  | `/v1/records/{id}`                        | Replace one active record conditionally.  |
| `POST` | `/v1/records/{id}/approve`                | Approve a candidate.                      |
| `POST` | `/v1/records/{id}/reject`                 | Reject a candidate by archiving it.       |
| `POST` | `/v1/records/{id}/archive`                | Archive an active record.                 |
| `POST` | `/v1/records/{id}/restore`                | Restore an archived record.               |
| `GET`  | `/v1/search?query=...&limit=20&all=false` | Search the reconciled lexical projection. |

Search results omit canonical host paths. The HTTP search endpoint uses lexical
retrieval and synchronizes the disposable projection before querying it. The
CLI and agent protocols retain their configured hybrid retrieval behavior.

## Conditional edits

`GET /v1/records/{id}` returns an `ETag` header. Send that exact value in
`If-Match` when replacing the record:

```sh
etag=$(curl -sD - http://127.0.0.1:7342/v1/records/<id> -o record.json \
  | awk 'tolower($1) == "etag:" { print $2 }' | tr -d '\r')

curl -X PUT http://127.0.0.1:7342/v1/records/<id> \
  -H "Content-Type: application/json" \
  -H "If-Match: $etag" \
  --data-binary @replacement.json
```

A missing precondition returns `428 precondition_required`. A stale ETag or a
change detected while the update is committed returns `412 revision_conflict`.
The response includes the current ETag when it was available, so the client can
reload rather than overwrite an external Markdown edit.

The replacement JSON supplies `title`, `kind`, `access`, `tags`, `aliases`,
`supersedes`, `sources`, and `body`. It cannot change the record ID, scope,
creation time, or lifecycle state. Use lifecycle endpoints for state changes.

`POST /v1/records` returns `201` for a written record, `409` for an exact
duplicate, and `422` for an invalid record. Its proposal response reports the
core outcome without exposing canonical paths.

## Errors and limits

JSON bodies are limited to 256 KiB and reject unknown fields. Errors have this
shape:

```json
{
	"error": {
		"code": "validation_error",
		"message": "field `kind` is invalid: must be one of fact, decision, procedure, or checkpoint"
	}
}
```

Errors do not expose canonical paths, record bodies, or backtraces. Core locking
serializes writes across the API, CLI, and other local Stormbuffer processes.
