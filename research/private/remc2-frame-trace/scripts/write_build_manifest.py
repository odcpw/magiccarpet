#!/usr/bin/env python3
"""Write deterministic provenance for the isolated Debug trace target."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git(root: Path, *arguments: str) -> bytes:
    return subprocess.check_output(["git", "-C", str(root), *arguments])


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", type=Path, required=True)
    parser.add_argument("--upstream-root", type=Path, required=True)
    parser.add_argument("--trace-root", type=Path, required=True)
    parser.add_argument("--build-root", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    args = parser.parse_args()

    upstream = args.upstream_root.resolve(strict=True)
    build = args.build_root.resolve(strict=True)
    binary = args.binary.resolve(strict=True)
    source_paths = [
        Path(line)
        for line in (build / "frame-trace-source-list.txt")
        .read_text(encoding="utf-8")
        .splitlines()
        if line
    ]
    sources = []
    identity = bytearray()
    for path in sorted(source_paths, key=lambda item: str(item)):
        resolved = path.resolve(strict=True)
        logical = resolved.relative_to(upstream).as_posix()
        digest = sha256(resolved)
        sources.append({"path": logical, "sha256": digest})
        identity.extend(f"{logical}\0{digest}\n".encode())
    upstream_diff = git(upstream, "diff", "--binary", "--no-ext-diff", "HEAD")
    manifest = {
        "schema": "mc2-frame-trace-build/v1",
        "build_type": "Debug",
        "target": "remc2-frame-trace",
        "binary_sha256": sha256(binary),
        "compiled_source_sha256": hashlib.sha256(identity).hexdigest(),
        "upstream_commit": git(upstream, "rev-parse", "HEAD").decode().strip(),
        "upstream_diff_sha256": hashlib.sha256(upstream_diff).hexdigest(),
        "sources": sources,
    }
    output = build / "frame-trace-build.json"
    output.write_text(
        json.dumps(manifest, sort_keys=True, indent=2) + "\n", encoding="utf-8"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
