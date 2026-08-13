# To-Dos

This file tracks unfinished implementation work. See
[CHANGELOG.md](CHANGELOG.md) for completed work and the
[documentation site](docs/src/content/docs/) for current behavior.

## Milestone 7: Local web editor and graph

### SB-701 — Define the local server API and security boundary

**What to build:** Expose core browse/search/lifecycle operations through an
HTTP API that binds to loopback, protects concurrent writes, handles signals,
and shuts down under a service manager.

**Acceptance criteria:**

- [ ] The server calls core APIs and never edits Markdown/SQLite directly.
- [ ] Default binding is loopback-only. Remote binding is unavailable until an
      authentication and a threat model are implemented.
- [ ] Edits use revision/ETag-style preconditions and report external changes.
- [ ] The foreground process logs to stderr, handles signals, and shuts down
      without corrupting writes.
- [ ] API and operator documentation match the implementation.

**Verification:** Run API integration tests for binding, concurrency conflicts,
signals, validation, and lifecycle parity.

### SB-702 — Build the human web editor

**What to build:** Add an accessible responsive app for listing, searching,
viewing, creating, editing, approving, superseding, archiving, and restoring
memories without the CLI.

**Blocked by:** SB-701

**Acceptance criteria:**

- [ ] Every supported lifecycle action uses core validation and reports
      validation or conflict errors with recovery instructions.
- [ ] Candidate, active, superseded, and archived states are visually and
      textually distinct.
- [ ] Unsaved and concurrent changes cannot be discarded silently.
- [ ] Core flows work with keyboard and screen reader semantics at narrow and
      wide layouts.
- [ ] Browser tests cover search, edit, approval, conflict, archive, and restore.

**Verification:** Run web checks and browser tests, then perform a keyboard and
screen-reader smoke review.

### SB-703 — Add the stored-relation graph

**What to build:** Visualize selected memories and their stored supersession,
scope, source, and shared-tag relationships in an Obsidian-style graph linked to
the editor.

**Blocked by:** SB-702

**Acceptance criteria:**

- [ ] Graph nodes and edges can always explain which stored field created them.
- [ ] Filters constrain scope, kind, status, relationship, and result size.
- [ ] Selecting a node opens its record and editor navigation can focus it in
      the graph.
- [ ] Queries enforce a documented result limit and remain responsive at the
      target store size.
- [ ] A non-canvas/list representation exposes the stored relationships for
      keyboard and assistive-technology users.
- [ ] The app makes no inferred entity or importance claims.

**Verification:** Run graph data contract and browser interaction tests, then
review readability with fixtures at the documented target size.

### SB-704 — Package and document daemonized operation

**What to build:** Document and test foreground server operation under common
service managers without adding a custom daemon supervisor.

**Blocked by:** SB-701, SB-702, SB-703

**Acceptance criteria:**

- [ ] Example user-service configurations name the store and bind address and
      preserve in-progress writes during restart and shutdown.
- [ ] Logs work with the service manager and contain no record bodies by default.
- [ ] Documentation covers installation, upgrade, backup, and recovery.
- [ ] An end-to-end test shows that the web app and CLI return matching records
      and enforce core lifecycle policy.

**Verification:** Run the documented foreground smoke test and at least one
service-manager integration check on a supported platform.

**Milestone exit:** A person can run Stormbuffer as a loopback-only service,
edit memory without the CLI, and inspect stored relationships in an accessible
graph.
