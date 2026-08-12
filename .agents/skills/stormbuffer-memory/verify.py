#!/usr/bin/env python3
"""Run the public CLI and MCP examples from the Stormbuffer memory skill."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

REPOSITORY = Path(__file__).resolve().parents[3]
CLI = os.environ.get("STORMBUFFER_BIN", str(REPOSITORY / "target/debug/sbuf"))
MCP = os.environ.get(
    "STORMBUFFER_MCP_BIN", str(REPOSITORY / "target/debug/stormbuffer-mcp")
)
FIXTURES = Path(__file__).with_name("fixtures.json")
SKILL = Path(__file__).with_name("SKILL.md")
PACKAGED_SKILL = REPOSITORY / "crates/cli/assets/stormbuffer-memory.md"

CAPTURE_EVENTS = {
    "user_correction",
    "explicit_remember",
    "accepted_decision",
    "confirmed_root_cause",
    "undocumented_constraint",
    "necessary_handoff",
    "stale_memory",
}
REJECTIONS = {
    "routine_success",
    "current_progress",
    "transient_failure",
    "temporary_workaround",
    "tentative_choice",
    "brainstorming",
    "generic_knowledge",
    "duplicated_documentation",
    "raw_transcript",
    "unsupported_user_inference",
    "source_dump",
    "secret",
    "fleeting_state",
}
OUTCOMES = {
    "continue",
    "recall_and_cite",
    "propose_candidate",
    "update_stale",
    "create_checkpoint",
}
EXPECTED_FIXTURES = {
    "no_capture_event": ("continue", 0),
    "recall_prior_decision": ("recall_and_cite", 0),
    "user_correction": ("propose_candidate", 1),
    "explicit_remember": ("propose_candidate", 1),
    "accepted_decision": ("propose_candidate", 1),
    "confirmed_root_cause": ("propose_candidate", 1),
    "undocumented_constraint": ("propose_candidate", 1),
    "necessary_handoff": ("create_checkpoint", 1),
    "stale_memory": ("update_stale", 1),
    "documented_correction": ("continue", 0),
    "routine_success": ("continue", 0),
    "current_progress": ("continue", 0),
    "transient_failure": ("continue", 0),
    "temporary_workaround": ("continue", 0),
    "tentative_choice": ("continue", 0),
    "brainstorming": ("continue", 0),
    "generic_knowledge": ("continue", 0),
    "duplicated_documentation": ("continue", 0),
    "raw_transcript": ("continue", 0),
    "unsupported_user_inference": ("continue", 0),
    "source_dump": ("continue", 0),
    "secret": ("continue", 0),
    "fleeting_state": ("continue", 0),
}
REQUIRED_POLICY_TEXT = (
    "The five visible outcomes are: continue with no memory action, recall and cite, propose one candidate, update or supersede stale memory, and create a necessary checkpoint.",
    "No: stop. Do not evaluate or propose memory merely because work completed.",
    "Yes: the event permits evaluation; it does not require storage.",
    "Never create more than one candidate.",
    "An explicit request to remember something is a capture event, not an exemption.",
    "Project retrieval can also return global records. Ignore them unless the task asks for global context or a record directly constrains this project.",
    "routine success or current task progress",
    "transient failures, temporary workarounds, or fleeting state",
    "tentative choices, brainstorming, or speculation",
    "generic knowledge or duplicated authoritative documentation",
    "raw chat transcripts, tool transcripts, or source dumps",
    "unsupported inferences about a user",
    "passwords, API keys, tokens, credentials, personal data, or other secrets",
)


def run(
    command: list[str],
    *,
    cwd: Path,
    env: dict[str, str],
    stdin: str = "",
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=cwd,
        env=env,
        input=stdin,
        text=True,
        capture_output=True,
        check=True,
    )


def verify_fixtures() -> None:
    fixtures = json.loads(FIXTURES.read_text())
    identifiers = [fixture["id"] for fixture in fixtures]
    assert len(identifiers) == len(set(identifiers)), "fixture IDs must be unique"
    actual = {
        fixture["id"]: (fixture["expected"], fixture["candidate_count"])
        for fixture in fixtures
    }
    assert actual == EXPECTED_FIXTURES, "fixtures must match the independent outcome contract"
    assert {fixture["expected"] for fixture in fixtures} == OUTCOMES
    assert {
        fixture.get("capture_event")
        for fixture in fixtures
        if fixture.get("capture_event")
    } == CAPTURE_EVENTS
    assert {fixture.get("rejection") for fixture in fixtures if fixture.get("rejection")} == REJECTIONS
    assert all(fixture["candidate_count"] <= 1 for fixture in fixtures)
    assert all(
        fixture.get("evidence") is True
        for fixture in fixtures
        if fixture["candidate_count"] == 1
    )
    no_event = next(fixture for fixture in fixtures if fixture["id"] == "no_capture_event")
    assert no_event["expected"] == "continue" and no_event["candidate_count"] == 0

    policy = " ".join(SKILL.read_text().split()).replace("**", "")
    for requirement in REQUIRED_POLICY_TEXT:
        assert requirement in policy, f"skill policy is missing: {requirement}"
    assert PACKAGED_SKILL.read_text() == SKILL.read_text(), (
        "packaged skill asset must exactly mirror the canonical project skill"
    )


def main() -> int:
    verify_fixtures()
    with tempfile.TemporaryDirectory(prefix="stormbuffer-skill-") as temporary:
        root = Path(temporary)
        project = root / "project"
        project.mkdir()
        env = os.environ.copy()
        env.update(
            {
                "STORMBUFFER_TEST_MODE": "1",
                "HOME": str(root / "home"),
                "XDG_DATA_HOME": str(root / "data"),
                "XDG_CACHE_HOME": str(root / "cache"),
            }
        )

        run([CLI, "--project", "init"], cwd=project, env=env)
        proposal = run(
            [CLI, "--project", "invoke", "remember"],
            cwd=project,
            env=env,
            stdin=(
                '{"version":1,"title":"Offline release","kind":"fact",'
                '"body":"The release must work offline.",'
                '"source":{"kind":"document","reference":"RELEASE.md#offline",'
                '"actor":"human"}}\n'
            ),
        )
        proposal_response = json.loads(proposal.stdout)
        assert proposal_response["result"]["outcome"] == "requires_approval"
        record_id = proposal_response["result"]["record_id"]
        run([CLI, "--project", "approve", record_id], cwd=project, env=env)

        context = run(
            [CLI, "--project", "invoke", "context"],
            cwd=project,
            env=env,
            stdin='{"version":1,"query":"offline release","budget":128}\n',
        )
        context_response = json.loads(context.stdout)
        assert context_response["ok"]
        assert (
            context_response["result"]["receipt"]["contract_version"]
            == "stormbuffer-context-v1"
        )
        assert context_response["result"]["blocks"][0]["record_id"] == record_id
        assert context_response["result"]["receipt"]["query"] == "offline release"

        messages = [
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "stormbuffer-memory-verify",
                        "version": "0.1.0",
                    },
                },
            },
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "memory_recall",
                    "arguments": {"query": "offline release", "budget": 128},
                },
            },
        ]
        mcp = run(
            [MCP, "--stdio", "--project"],
            cwd=project,
            env=env,
            stdin="\n".join(json.dumps(message) for message in messages) + "\n",
        )
        assert not mcp.stderr, mcp.stderr
        responses = [json.loads(line) for line in mcp.stdout.splitlines()]
        assert responses[0]["result"]["serverInfo"]["name"] == "stormbuffer-mcp"
        envelope = responses[1]["result"]["structuredContent"]
        assert envelope == context_response
        assert json.loads(responses[1]["result"]["content"][0]["text"]) == envelope
        assert envelope["result"]["blocks"][0]["record_id"] == record_id

    print("stormbuffer-memory verify: passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
