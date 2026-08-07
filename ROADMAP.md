# Stormbuffer roadmap

Status: draft product and architecture contract

Stormbuffer is a local-first memory store for people and software agents. It
keeps durable facts, decisions, procedures, and project checkpoints in readable
Markdown. A rebuildable SQLite index makes that material searchable without
turning the database into a second source of truth.

The product ships as the equivalent CLI entry points `stormbuffer`, `stormbuf`,
and `sbuf`, plus an MCP server. The CLI must be useful from the first milestone,
even while later commands are honest stubs. The final milestone adds an optional
local web server and editor with an Obsidian-style graph.

`TODO.md` contains the implementation tickets and dependencies for this roadmap.

## Product contract

Stormbuffer should let a person or agent:

- keep a small, sourced memory that remains understandable without Stormbuffer;
- find exact names and commands as reliably as semantically similar ideas;
- supply bounded, attributable evidence for retrieval-augmented generation;
- distinguish proposed memories from approved ones;
- correct history through explicit supersession rather than silent rewriting;
- inspect, repair, export, archive, and deliberately destroy their own data;
- use the same behavior through the human CLI, JSON invocation protocol, MCP,
  and, eventually, the web app.

The four memory kinds are:

| Kind         | Use                                         |
| ------------ | ------------------------------------------- |
| `fact`       | Durable facts, constraints, and preferences |
| `decision`   | A choice and its rationale                  |
| `procedure`  | Reusable instructions or workflow           |
| `checkpoint` | Current state of an ongoing project         |

A memory is independently understandable, useful later, specific to its user or
project, backed by a source, and small enough to retrieve as a unit. Raw
transcripts, generic knowledge, fleeting task state, unsupported inference,
duplicate documentation, and secrets do not belong in the store.

## Architecture

```text
Human or agent
      |
      +-- stormbuffer / stormbuf / sbuf
      +-- strict JSON invocation
      +-- MCP adapter
      +-- local web app (final milestone)
      |
      v
stormbuffer-core
  validation, policy, retrieval, mutation, RAG context compilation
      |
      +-- Markdown records       canonical and portable
      +-- SQLite projection      disposable and rebuildable
            +-- metadata
            +-- FTS5
            +-- sqlite-vec
```

All interfaces call `stormbuffer-core`. The MCP and web crates must not edit
Markdown or SQLite directly or reimplement policy. Markdown is authoritative;
SQLite may be deleted and rebuilt without losing user data.

Keep the current four-crate shape unless implementation proves it inadequate:

- `stormbuffer-core`: records, configuration, repositories, indexing,
  retrieval, mutation policy, and context compilation
- `stormbuffer`: CLI and user-facing process behavior
- `stormbuffer-mcp`: thin MCP resources and tools
- `stormbuffer-server`: local HTTP server and web-app integration

The `docs/` SvelteKit app is the documentation site. The web editor may share
small presentation components with it later, but it remains a different product
surface with different deployment and security requirements.

## Canonical records

Records are Markdown with TOML frontmatter. The initial schema stays small:

```toml
+++
format_version = 1
id = "01989af2-4305-7b19-88b1-e8ae4ea9a02b"
title = "The Stormbuffer core owns all writes"
kind = "decision"
scope = "project:stormbuffer"
status = "active"
access = "agent"
created_at = "2026-08-05T20:09:00-05:00"
updated_at = "2026-08-05T20:09:00-05:00"
tags = ["architecture", "storage"]
aliases = ["single write boundary"]
supersedes = []

[[sources]]
kind = "conversation"
reference = "stormbuffer://session/2026-08-05"
actor = "user"
+++

The Stormbuffer core is the only component allowed to validate and mutate
durable memory.
```

Required fields map to a typed Rust model: format version, ID, title, kind,
scope, status, access, timestamps, tags, aliases, superseded IDs, sources, and
body. The current record format version is `1`; unknown frontmatter fields are
invalid. Do not add confidence or importance scores without an evaluation showing
that they solve a retrieval failure.

The lifecycle is `candidate -> active -> superseded|archived`. Normal retrieval
excludes superseded and archived records. Permanent deletion requires the
explicit `forget --destroy` path.

Use platform data and cache directories rather than hard-coded Unix paths.
Project stores live under a `.sbuf/` directory. Project memory is private and
ignored by Git by default; a user must deliberately opt into a shared store.
`stormbuffer --project init --shared` creates that explicit opt-in. A shared
store commits only its configuration, ignore rules, and canonical Markdown
records. SQLite databases, FTS and vector projections, embeddings, downloaded
models, locks, temporary files, and logs remain ignored and rebuildable.

## Index and retrieval

SQLite is a materialized projection of validated records. It contains relational
metadata, chunks, provenance, FTS5 data, vectors, content hashes, and index/model
metadata. Changes for one record are transactional. CLI writes use a store lock,
temporary file, `fsync`, and atomic rename before updating the index. If indexing
fails, the committed Markdown wins and `stormbuffer sync` repairs the projection.

Indexing follows this pipeline:

```text
scan -> parse -> validate -> hash -> chunk -> FTS -> embed -> vector index
```

Unchanged files are skipped and stale rows are removed. Most facts and decisions
form one chunk. Longer records split at headings and conservative token
boundaries without splitting code blocks or lists.

Retrieval runs FTS5 and vector search in parallel, combines their ranks with
reciprocal-rank fusion, collapses chunks by memory, applies small deterministic
scope/title/status adjustments, and returns bounded results. Exact title and
alias matches and the current project scope may receive a boost. Facts,
decisions, and procedures receive no blanket recency boost.

The embedding implementation is isolated behind an `Embedder` interface and the
vector implementation behind a `VectorIndex` interface. Start with local ONNX
inference using `all-MiniLM-L6-v2` (384 dimensions), pin pre-1.0 vector
dependencies, and store model and tokenizer hashes. Model migrations build and
validate a new versioned vector table before switching the active table.

An evaluation corpus compares FTS-only, vector-only, and hybrid retrieval. It
tracks recall at 5, mean reciprocal rank, wrong-scope and superseded-memory
retrieval, duplicate/conflicting proposals, and context tokens per useful
memory. Ranking complexity needs evidence from this suite.

## Retrieval-augmented generation

Stormbuffer owns retrieval and context assembly. The calling agent or application
owns generation. The initial RAG design does not add a hosted-model SDK, model
runner, or provider-specific prompt format to the core. This keeps retrieval
usable offline and prevents the core from sending memory to a remote service.

`context` returns ordered evidence blocks rather than a prose prompt. Each block
includes stable record and chunk identifiers, title, scope, lifecycle and access
metadata, source references, and the text selected within the caller's budget.
The receipt records the query, filters, index and embedding versions, ranking
reasons, omitted results, and truncation. CLI, JSON, and MCP expose the same core
shape, with presentation differences only where the interface requires them.

The host places its instructions, the user's question, and retrieved evidence in
distinct message or data boundaries. Record bodies are untrusted quoted evidence;
instructions found inside them cannot grant tools, change access, widen scope, or
override the user's request. Access and lifecycle filters run before context
assembly. When the evidence does not support an answer, the host says so instead
of filling the gap from model memory. Factual claims cite the record IDs that
support them, and conflicting active evidence remains visible.

RAG evaluation separates failures in retrieval, context assembly, and generation.
The checked-in suite measures retrieval recall and rank, context precision and
recall, claim support, citation precision and recall, answer relevance, correct
abstention, scope leakage, and resistance to instructions embedded in records.
It includes answerable, unanswerable, conflicting, long-context, and adversarial
questions. Model-assisted scores supplement inspectable expected record IDs and
claim-level judgments; they do not silently rewrite expected results.

This design follows the retriever/generator split introduced in the
[original RAG paper](https://arxiv.org/abs/2005.11401), keeps context bounded in
light of measured [long-context position sensitivity](https://arxiv.org/abs/2307.03172),
and evaluates retrieval and generation separately as proposed by
[RAGAS](https://arxiv.org/abs/2309.15217). The threat model treats retrieved text
as a source of indirect prompt injection, consistent with
[OWASP's prompt-injection guidance](https://genai.owasp.org/llmrisk/llm01-prompt-injection/).
Hybrid lexical and semantic retrieval remains the baseline; contextual indexing
or reranking is added only if the corpus shows a failure it fixes, informed by
[Anthropic's contextual retrieval experiments](https://www.anthropic.com/engineering/contextual-retrieval).

## CLI contract

The public command tree is visible in the first milestone:

```text
stormbuffer [--project] init [--shared]
stormbuffer root
stormbuffer status                 stormbuffer add
stormbuffer propose                stormbuffer approve <candidate>
stormbuffer reject <candidate>     stormbuffer edit <id>
stormbuffer show <id>              stormbuffer list
stormbuffer search <query>         stormbuffer context <query>
stormbuffer supersede <id>         stormbuffer archive <id>
stormbuffer restore <id>           stormbuffer forget <id> --destroy
stormbuffer sync                   stormbuffer watch
stormbuffer reindex                stormbuffer gc
stormbuffer doctor                 stormbuffer export
stormbuffer import                 stormbuffer invoke <operation>
stormbuffer mcp --stdio
```

`stormbuf` and `sbuf` invoke the same command tree and behavior. Packaging may
use aliases, links, or small launcher binaries, but tests must exercise every
installed name. Help output should use the name the person invoked when
practical.

The CLI follows the [Command Line Interface Guidelines](https://clig.dev/):

- useful `--help` at every level, concise examples, and an accurate `--version`;
- primary results on stdout and diagnostics on stderr;
- quiet success, actionable errors, stable non-zero exit statuses, and no
  panic/backtrace in ordinary user errors;
- prompts only on an interactive terminal, with `--yes` or an equivalent
  explicit non-interactive path where confirmation is appropriate;
- respect pipes, redirection, terminal width, cancellation, and established
  configuration precedence;
- `--color auto|always|never`, with `auto` limited to terminals and `NO_COLOR`
  disabling color unless a person explicitly requests `--color=always`;
- `owo-colors` for human-facing style, never ANSI escapes in JSON or redirected
  output;
- destructive actions named and confirmed; `forget --destroy` remains the only
  permanent deletion path.

The first milestone may return clear “not implemented yet” errors for unfinished
commands. Stubs still parse arguments, show help, avoid touching data, and exit
with a documented non-zero code. Implemented commands and stubs must be clearly
distinguishable in help and documentation.

Agent automation uses `stormbuffer invoke <operation>` with JSON on stdin and
JSON on stdout. Logs go only to stderr. The protocol is non-interactive, bounded,
uncolored, scoped, and versioned, with stable machine-readable error codes. It
does not accept arbitrary filesystem paths.

Generate shell completions with `clap_complete` and man pages with
`clap_mangen` from the same Clap command definition used at runtime. Release
checks fail when committed generated artifacts do not match the command tree.

## Agent writes and MCP

Human writes may become active immediately. Agents normally create candidates.
Before accepting a proposal, the core validates provenance and checks for
duplicates and conflicts. Results use a stable outcome vocabulary:
`accepted`, `duplicate_of`, `conflicts_with`, `requires_approval`, or `invalid`.
Conflicting information creates a superseding record; it does not silently edit
the old claim.

The MCP server is a compatibility adapter over the core. Initial resources are
`stormbuffer://record/{id}`, `stormbuffer://scope/{scope}/records`, and
`stormbuffer://candidate/{id}`. Initial tools are `stormbuffer_search`,
`stormbuffer_context`, `stormbuffer_get`, `stormbuffer_propose`,
`stormbuffer_supersede`, and `stormbuffer_archive`.

MCP does not expose raw SQL, arbitrary file editing, reindexing, or destructive
deletion. Write tools are disabled by default or require host-granted approval.
Stdio is the default transport.

## Documentation contract

Documentation changes ship with the behavior they describe. A command, option,
configuration key, record field, protocol response, or user workflow is not done
until its reference and examples are current. The CLI command definition remains
the source of truth for help, man pages, and completions; prose still needs human
review.

The static docs site uses SvelteKit, `adapter-static`, mdsvex, typed frontmatter,
and Pagefind. Its information architecture should feel familiar to Docusaurus
users: persistent top navigation, hierarchical sidebar, breadcrumbs, previous
and next links, a right-hand table of contents on wide screens, version-visible
pages, strong code blocks, and fast local search. It should not copy Docusaurus
branding or components.

Typography uses Fontsource variable packages where available:

- IBM Plex Serif for page and section headings;
- IBM Plex Sans for navigation, controls, body text, and general UI;
- JetBrains Mono for code and terminal examples.

The site must work without client-side JavaScript for reading and navigation,
meet WCAG 2.2 AA for its core flows, support narrow screens and keyboard use,
and emit a fully static build that Pagefind indexes after generation.

## Milestones

### 0. Usable shell and living docs

Establish the Cargo workspace, shared error/config conventions, the complete
Clap command tree, all three executable names, color/output policy, man pages,
completions, and the first useful `init`, `root`, and `status` flows. Unfinished
commands are safe, documented stubs. Replace the starter Svelte page with the
static mdsvex documentation shell and publish CLI/reference pages from day one.

Exit: a new user can install a development build, invoke any public command,
initialize a store, understand what is implemented, generate completions/man
pages, and browse searchable static documentation.

### 1. Canonical Markdown store

Implement the typed record schema, parsing and rendering, platform/project
locations, validation, atomic writes, locking, and human CRUD/lifecycle flows.
SQLite is not required for correctness in this milestone; list/show may scan the
canonical files.

Exit: records survive round trips without losing user-authored Markdown and all
lifecycle operations are covered through public core/CLI behavior.

### 2. Rebuildable lexical index

Add SQLite migrations, hashes, transactional projection, chunking, FTS5 search,
sync/reindex/watch, recovery behavior, and doctor diagnostics.

Exit: deleting the cache and reindexing produces equivalent search behavior;
manual Markdown edits and interrupted index updates recover safely.

### 3. Semantic and hybrid retrieval

Add verified model acquisition, ONNX embeddings, versioned sqlite-vec indexes,
hybrid rank fusion, bounded context compilation, receipts, and the retrieval
evaluation harness.

Exit: hybrid retrieval meets the checked-in evaluation thresholds and context
output stays within its requested budget.

### 4. Grounded RAG and agent workflow

Define the provider-neutral evidence contract, grounded-answer and injection
evaluations, candidate proposal/review, provenance rules, duplicate/conflict
detection, permissions, and versioned `invoke` operations. Dogfood the completed
flow with this repository's deliberately shared `.sbuf/` store. Commit its
configuration and Markdown records while keeping every projection and runtime
artifact ignored. Add import/export and garbage collection where they support
recovery and portability.

Exit: an unattended agent can retrieve bounded evidence, produce cited answers,
abstain when evidence is insufficient, and propose memory using public contracts.
This repository's committed memory supplies a reproducible end-to-end example,
while a person retains control over activation and destructive actions.

### 5. MCP compatibility and release hardening

Expose the approved resource/tool surface through the shared core. Finish the
behavioral agent skill, cross-platform packaging, model/cache diagnostics,
release artifact checks, and complete user/operator documentation.

Exit: MCP and CLI contract tests return equivalent results, packaged aliases,
man pages, and completions work on supported platforms, and a clean install can
pass the documented smoke test.

### 6. Local web editor and graph

Build the optional daemonizable `stormbuffer-server` and a small human-facing
web app. It supports browsing, searching, creating, editing, approving,
superseding, archiving, and restoring records through core APIs. The graph uses
explicit relationships such as supersession, scope, source, and shared tags; it
does not infer an opaque knowledge graph.

Default binding is loopback-only. Remote binding requires an explicit security
design and must never be enabled accidentally. Editing handles concurrent file
changes without silently overwriting them. The server runs in the foreground,
logs to stderr, shuts down cleanly, and works under normal service managers
without requiring its own daemon manager.

Exit: a person can operate the supported memory lifecycle and inspect an
accessible, useful graph without the CLI, and both interfaces observe identical
validation and policy.

## v0.2 product priorities

Version 0.2 should make the existing memory loop easy to adopt before it adds
new retrieval machinery. Stormbuffer's useful boundary is durable project
memory that remains readable, sourced, repairable, and under human control. It
is not a hosted user-profile service or an autonomous knowledge graph.

Work in this order:

1. Deliver a five-minute first success: install Stormbuffer, initialize a
   project store, connect one supported agent, propose a sourced memory, approve
   it, and retrieve it with a citation. `doctor` must diagnose failures in that
   path and point to a concrete remedy.
2. Make agent setup copyable and verifiable. Ship one-command skill installation
   and concise MCP examples for supported clients, with an end-to-end smoke test
   that proves the agent is using the intended project store.
3. Reduce capture effort without weakening review. Let agents turn session or
   repository evidence into candidate memories, but keep activation and
   destructive lifecycle changes explicitly human-controlled.
4. Explain retrieval decisions. Show the selected memory's scope, lifecycle,
   sources, and receipt so a person can understand why it appeared and correct
   the underlying record or policy.
5. Measure adoption before expanding the architecture. Test the first-success
   flow with people unfamiliar with the project and track time to first cited
   retrieval, setup failures, abandoned steps, and rejected proposals alongside
   the retrieval evaluation corpus.

Do not prioritize inferred knowledge graphs, broad connector catalogs, hosted
profiles, or additional ranking stages unless first-use observations or checked-
in retrieval evaluations demonstrate that they solve a measured failure.

## Cross-cutting completion rules

Every milestone must keep the following checks green once their configuration
exists:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
pnpm --dir docs check
pnpm --dir docs lint
pnpm --dir docs test
pnpm --dir docs build
```

Use focused tests during implementation and the full applicable set at milestone
exit. Public behavior needs tests at the highest stable boundary: core
integration tests for storage/retrieval, process-level tests for CLI and JSON,
protocol tests for MCP, and browser tests for critical web flows. Subjective
documentation and graph usability receive a human review.

## Risks and decisions to validate

- `sqlite-vec` and Rust ONNX bindings may change before 1.0. Keep both behind
  narrow interfaces, pin versions, and test upgrades against the corpus.
- Shipping a model affects binary/install size and offline behavior. Choose the
  acquisition and verification policy before semantic search ships.
- Executable aliases differ across package managers and Windows. Packaging
  tests must prove `stormbuffer`, `stormbuf`, and `sbuf`, not assume symlinks.
- Shared project stores introduce merge and privacy concerns. Shared mode must
  remain explicit and tracks only canonical configuration and Markdown; this
  repository's `.sbuf/` store is the reference example.
- Retrieved Markdown may contain instructions intended to steer the generator.
  Treat it as untrusted evidence, enforce policy outside the model, and include
  indirect prompt injection in the evaluation corpus.
- Generator behavior varies by model and version. Keep the context contract
  provider-neutral, record evaluation configuration, and diagnose retrieval,
  context, and generation failures separately.
- Web editing introduces concurrent writes and network exposure. Keep the server
  last, loopback-only by default, and reuse core locking and validation.
- Typed frontmatter and versioned docs need a settled content schema before the
  documentation corpus grows.
