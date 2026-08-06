#!/usr/bin/env python3
"""Build a Stormbuffer release archive from compiled binaries."""

from __future__ import annotations

import argparse
import hashlib
import shutil
import tarfile
import tempfile
import zipfile
from pathlib import Path

CLI_NAMES = ("stormbuffer", "stormbuf", "sbuf")


def copy_tree(source: Path, destination: Path) -> None:
    if not source.is_dir():
        raise FileNotFoundError(f"required release directory is missing: {source}")
    shutil.copytree(source, destination)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--output", type=Path, default=Path("dist"))
    args = parser.parse_args()

    repository = Path(__file__).resolve().parents[2]
    executable_suffix = ".exe" if "windows" in args.target else ""
    build_directory = repository / "target" / args.target / "release"
    archive_name = f"stormbuffer-{args.version}-{args.target}"
    args.output.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="stormbuffer-release-") as temporary:
        root = Path(temporary) / archive_name
        binary_directory = root / "bin"
        binary_directory.mkdir(parents=True)
        for name in (*CLI_NAMES, "stormbuffer-mcp"):
            source = build_directory / f"{name}{executable_suffix}"
            if not source.is_file():
                raise FileNotFoundError(f"required release binary is missing: {source}")
            shutil.copy2(source, binary_directory / source.name)

        for name in ("LICENSE", "README.md", "RELEASE.md"):
            shutil.copy2(repository / name, root / name)
        copy_tree(repository / "assets" / "man", root / "share" / "man" / "man1")
        copy_tree(
            repository / "assets" / "completions",
            root / "share" / "completions",
        )

        if executable_suffix:
            archive = args.output / f"{archive_name}.zip"
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
                for path in sorted(root.rglob("*")):
                    if path.is_file():
                        output.write(path, path.relative_to(root.parent))
        else:
            archive = args.output / f"{archive_name}.tar.gz"
            with tarfile.open(archive, "w:gz", format=tarfile.PAX_FORMAT) as output:
                output.add(root, arcname=root.name)

    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    checksum = archive.with_name(f"{archive.name}.sha256")
    checksum.write_text(f"{digest}  {archive.name}\n", encoding="utf-8")
    print(archive)


if __name__ == "__main__":
    main()
