#!/usr/bin/env python3
"""Check that the Stormbuffer binaries needed by this skill are healthy."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def binary(name: str) -> str:
    path = shutil.which(name)
    if path is None:
        raise RuntimeError(f"{name} was not found on PATH")
    return path


def run(command: list[str], *, cwd: Path, env: dict[str, str], stdin: str = "") -> str:
    result = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        input=stdin,
        text=True,
        capture_output=True,
        timeout=20,
        check=False,
    )
    if result.returncode != 0:
        detail = (
            result.stderr.strip() or result.stdout.strip() or "no diagnostic output"
        )
        raise RuntimeError(f"{' '.join(command)} failed: {detail}")
    return result.stdout


def verify() -> None:
    sbuf = binary("sbuf")
    mcp = binary("stormbuffer-mcp")

    with tempfile.TemporaryDirectory(prefix="stormbuffer-health-") as temporary:
        root = Path(temporary)
        project = root / "project"
        project.mkdir()
        env = os.environ.copy()
        env.update(
            {
                "HOME": str(root / "home"),
                "USERPROFILE": str(root / "home"),
                "XDG_DATA_HOME": str(root / "data"),
                "XDG_CACHE_HOME": str(root / "cache"),
            }
        )

        run([sbuf, "--version"], cwd=project, env=env)
        run([sbuf, "--project", "init"], cwd=project, env=env)
        search = run(
            [sbuf, "--project", "invoke", "search"],
            cwd=project,
            env=env,
            stdin='{"version":1,"query":"health check"}\n',
        )
        envelope = json.loads(search)
        if envelope.get("version") != 1 or envelope.get("ok") is not True:
            raise RuntimeError("sbuf returned an invalid JSON protocol envelope")

        messages = [
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {"name": "stormbuffer-health", "version": "1"},
                },
            },
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}},
        ]
        output = run(
            [mcp, "--stdio", "--project"],
            cwd=project,
            env=env,
            stdin="\n".join(json.dumps(message) for message in messages) + "\n",
        )
        responses = [json.loads(line) for line in output.splitlines()]
        if (
            len(responses) != 2
            or "result" not in responses[0]
            or "result" not in responses[1]
        ):
            raise RuntimeError("stormbuffer-mcp returned an invalid protocol response")
        available = {
            tool.get("name") for tool in responses[1]["result"].get("tools", [])
        }
        required = {
            "memory_recall",
            "memory_get",
            "memory_remember",
            "memory_update",
            "memory_forget",
        }
        if missing := required - available:
            raise RuntimeError(
                f"stormbuffer-mcp is missing tools: {', '.join(sorted(missing))}"
            )


def main() -> int:
    try:
        verify()
    except (
        json.JSONDecodeError,
        OSError,
        RuntimeError,
        subprocess.TimeoutExpired,
    ) as error:
        print(f"Stormbuffer health check failed: {error}", file=sys.stderr)
        return 1
    print("Stormbuffer health check passed: sbuf and stormbuffer-mcp are ready")
    return 0


if __name__ == "__main__":
    sys.exit(main())
