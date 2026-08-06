#!/usr/bin/env python3
"""Exercise an unpacked Stormbuffer release without touching user data."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

CLI_NAMES = ("stormbuffer", "stormbuf", "sbuf")


def run(
    binary: Path,
    arguments: list[str],
    *,
    directory: Path,
    environment: dict[str, str],
    input_text: str | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [binary, *arguments],
        cwd=directory,
        env=environment,
        input=input_text,
        capture_output=True,
        check=True,
        text=True,
        timeout=30,
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("archive", type=Path)
    parser.add_argument("--version", required=True)
    args = parser.parse_args()
    archive = args.archive.resolve()

    checksum = archive.with_name(f"{archive.name}.sha256")
    checksum_parts = checksum.read_text(encoding="utf-8").strip().split()
    if len(checksum_parts) != 2 or checksum_parts[1] != archive.name:
        raise RuntimeError("checksum file must name the release archive")
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    if checksum_parts[0] != digest:
        raise RuntimeError("release archive checksum does not match")

    with tempfile.TemporaryDirectory(prefix="stormbuffer-smoke-") as temporary:
        temporary_root = Path(temporary)
        install_root = temporary_root / "install"
        shutil.unpack_archive(archive, install_root)
        package_roots = [path for path in install_root.iterdir() if path.is_dir()]
        if len(package_roots) != 1:
            raise RuntimeError("release archive must contain one top-level directory")
        package_root = package_roots[0]
        suffix = ".exe" if os.name == "nt" else ""

        required_files = (
            "LICENSE",
            "README.md",
            "RELEASE.md",
            "share/man/man1/stormbuffer.1",
            "share/man/man1/stormbuffer-mcp.1",
            "share/completions/stormbuffer.bash",
            "share/completions/stormbuffer.zsh",
            "share/completions/stormbuffer.fish",
            "share/completions/stormbuffer.ps1",
        )
        missing_files = [
            name for name in required_files if not (package_root / name).is_file()
        ]
        if missing_files:
            raise RuntimeError(
                f"release archive is missing required files: {', '.join(missing_files)}"
            )

        isolated_home = temporary_root / "home"
        project = temporary_root / "project"
        project.mkdir()
        environment = os.environ.copy()
        environment.update(
            {
                "HOME": str(isolated_home),
                "USERPROFILE": str(isolated_home),
                "LOCALAPPDATA": str(temporary_root / "data"),
                "APPDATA": str(temporary_root / "data"),
                "XDG_DATA_HOME": str(temporary_root / "data"),
                "XDG_CACHE_HOME": str(temporary_root / "cache"),
            }
        )

        binaries = {
            name: package_root / "bin" / f"{name}{suffix}"
            for name in (*CLI_NAMES, "stormbuffer-mcp")
        }
        for name in CLI_NAMES:
            version = run(
                binaries[name],
                ["--version"],
                directory=project,
                environment=environment,
            )
            if args.version not in version.stdout:
                raise RuntimeError(f"{name} did not report version {args.version}")
            help_output = run(
                binaries[name], ["--help"], directory=project, environment=environment
            )
            if f"Usage: {name}" not in help_output.stdout:
                raise RuntimeError(f"{name} help did not use its invoked name")

        mcp_version = run(
            binaries["stormbuffer-mcp"],
            ["--version"],
            directory=project,
            environment=environment,
        )
        if args.version not in mcp_version.stdout:
            raise RuntimeError("stormbuffer-mcp reported the wrong version")

        run(
            binaries["stormbuffer"],
            ["--project", "init"],
            directory=project,
            environment=environment,
        )
        mcp_input = "\n".join(
            (
                json.dumps(
                    {
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "initialize",
                        "params": {
                            "protocolVersion": "2025-06-18",
                            "capabilities": {},
                            "clientInfo": {
                                "name": "stormbuffer-release-check",
                                "version": args.version,
                            },
                        },
                    }
                ),
                json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}),
                json.dumps(
                    {
                        "jsonrpc": "2.0",
                        "id": 2,
                        "method": "tools/list",
                        "params": {},
                    }
                ),
            )
        )
        mcp_output = run(
            binaries["stormbuffer-mcp"],
            ["--stdio", "--project"],
            directory=project,
            environment=environment,
            input_text=f"{mcp_input}\n",
        )
        mcp_responses = [json.loads(line) for line in mcp_output.stdout.splitlines()]
        if len(mcp_responses) != 2:
            raise RuntimeError("packaged MCP server returned unexpected responses")
        if (
            mcp_responses[0].get("result", {}).get("serverInfo", {}).get("name")
            != "stormbuffer-mcp"
        ):
            raise RuntimeError("packaged MCP server failed initialization")
        if len(mcp_responses[1].get("result", {}).get("tools", [])) != 6:
            raise RuntimeError("packaged MCP server exposed an unexpected tool surface")
        proposal = {
            "version": 1,
            "title": "Release smoke memory",
            "kind": "fact",
            "access": "agent",
            "body": "Canonical Markdown remains outside the installation directory.",
            "sources": [
                {
                    "kind": "document",
                    "reference": "RELEASE.md",
                    "actor": "human",
                }
            ],
        }
        response = run(
            binaries["stormbuffer"],
            ["--project", "invoke", "propose"],
            directory=project,
            environment=environment,
            input_text=json.dumps(proposal),
        )
        if not json.loads(response.stdout).get("ok"):
            raise RuntimeError("release binary could not write a canonical record")

        records = list((project / ".sbuf" / "records").glob("*.md"))
        if len(records) != 1:
            raise RuntimeError("release smoke test did not create one canonical record")
        canonical = records[0].read_bytes()
        run(
            binaries["stormbuf"],
            ["--project", "sync"],
            directory=project,
            environment=environment,
        )
        if records[0].read_bytes() != canonical:
            raise RuntimeError("sync changed canonical Markdown")

        shutil.rmtree(package_root)
        if records[0].read_bytes() != canonical:
            raise RuntimeError("removing the installation changed canonical Markdown")

    print(f"release check passed for {archive.name}")


if __name__ == "__main__":
    main()
