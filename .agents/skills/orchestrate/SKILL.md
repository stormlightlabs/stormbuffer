---
name: orchestrate
description: Orchestrate bounded coding work with one or two Pi instances running Luna xhigh through Pi's ChatGPT provider in a neighboring Herdr tab. Use when the user explicitly asks to orchestrate, delegate, or parallelize work with Herdr and Pi in this repository. Do not trigger for ordinary background work or generic subagent delegation.
---

# Orchestrate

Verify that the current session is managed by Herdr before issuing control commands:

```sh
test "${HERDR_ENV:-}" = 1
```

If the check fails, explain that Herdr orchestration is unavailable and stop.
Use no more than two Pi instances. Keep the user's focus unchanged and do not
close tabs, panes, or agents that this workflow did not create.

## Create the neighboring tab

If one does not exist, create a background tab in the current workspace, preserving
the current working directory:

```sh
herdr tab create --workspace "$HERDR_WORKSPACE_ID" --cwd "$PWD" --label PI --no-focus
```

Read the root pane ID from `.result.root_pane.pane_id`. Create a second pane
only when the work benefits from a second instance:

```sh
herdr pane split <root-pane-id> --direction right --cwd "$PWD" --no-focus
```

Read the new pane ID from `.result.pane.pane_id`. Treat all returned IDs as
opaque values; never infer them from layout or examples.

## Start Pi

Start each instance with Luna xhigh through Pi's ChatGPT provider:

```sh
herdr agent start <first-name> --kind pi --pane <root-pane-id> -- --provider openai-codex --model gpt-5.6-luna --thinking xhigh
herdr agent start <second-name> --kind pi --pane <split-pane-id> -- --provider openai-codex --model gpt-5.6-luna --thinking xhigh
```

Use short unique names matching `[a-z][a-z0-9_-]{0,31}`. Omit the second
command when one instance is enough.

## Delegate and integrate

Give each Pi instance a bounded task with an explicit deliverable and
verification target:

```sh
herdr agent prompt <name> "<task>" --wait --timeout 120000
```

Assign only one writer to an overlapping file set. Use the other instance for
independent files, read-only investigation, or review. Tell every instance to
preserve unrelated work and follow the repository's `AGENTS.md` instructions.

After each turn, inspect lifecycle state and the recent transcript:

```sh
herdr agent get <name>
herdr agent read <name> --source recent-unwrapped --lines 120
```

If an instance is blocked or a wait fails, inspect it before sending a focused
follow-up. Review all shared-tree changes yourself, resolve integration issues,
and run the smallest relevant verification. Do not treat an agent's success
claim as verification.

## Paired review mode

When the user asks for a review, or a completed delegated change warrants an
independent review, reuse the two Pi instances (with fresh context) as complementary
read-only, reviewers.

Prompt both before waiting so they work concurrently:

- The standard reviewer checks correctness, error handling, security,
  concurrency, resource use, API boundaries, tests, and maintainability.
- The adversarial reviewer tries to break the change with hostile inputs,
  partial failures, violated invariants, misleading tests, and unverified
  assumptions. Prefer a small number of consequential findings over nits.

Give both reviewers the same target, intent, changed-file list, and relevant
constraints. Require findings in this form:

```text
severity · path:line — problem → impact → fix direction
```

Wait until both reviewers finish before acting on either report. Merge rather
than concatenate:

- deduplicate the same root cause
- keep the higher justified severity
- identify findings as `standard`, `adversarial`, or `both`

Ground every finding in inspected code and label downstream effects that were
not verified.

For changes owned by the current task, fix confirmed findings and run one fresh
paired pass when the fix materially changes behavior.

For external review, remain read-only. Never publish review feedback or mutate a
pull request without the user's explicit request.
