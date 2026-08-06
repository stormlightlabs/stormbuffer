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
CLI = os.environ.get("STORMBUFFER_BIN", str(REPOSITORY / "target/debug/stormbuffer"))
MCP = os.environ.get(
    "STORMBUFFER_MCP_BIN", str(REPOSITORY / "target/debug/stormbuffer-mcp")
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


def main() -> int:
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
            [CLI, "--project", "invoke", "propose"],
            cwd=project,
            env=env,
            stdin=(
                '{"version":1,"title":"Offline release","kind":"fact",'
                '"access":"agent","body":"The release must work offline.",'
                '"sources":[{"kind":"document","reference":"RELEASE.md#offline",'
                '"actor":"human"}]}\n'
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
        assert context_response["result"]["receipt"]["contract_version"] == "stormbuffer-context-v1"
        assert context_response["result"]["blocks"][0]["record_id"] == record_id
        assert context_response["result"]["receipt"]["query"] == "offline release"

        search = run(
            [CLI, "--project", "invoke", "search"],
            cwd=project,
            env=env,
            stdin='{"version":1,"query":"offline release","limit":5}\n',
        )
        search_response = json.loads(search.stdout)

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
                    "name": "stormbuffer_search",
                    "arguments": {"query": "offline release", "limit": 5},
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
        assert envelope == search_response
        assert json.loads(responses[1]["result"]["content"][0]["text"]) == envelope
        assert any(item["record_id"] == record_id for item in envelope["result"])

    print("stormbuffer-memory verify: passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
