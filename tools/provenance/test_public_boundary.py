#!/usr/bin/env python3
"""Adversarial self-test for audit_public_tree.py."""

from __future__ import annotations

import hashlib
import json
import subprocess
import tempfile
from pathlib import Path


HERE = Path(__file__).resolve().parent
SCANNER = HERE / "audit_public_tree.py"
PROJECT_ROOT = HERE.parent.parent
PUBLIC_POLICY = PROJECT_ROOT / "provenance/public-boundary-policy.json"
PRIVATE_POLICY = PROJECT_ROOT / "docs/provenance/public-boundary-policy.json"
POLICY = PUBLIC_POLICY if PUBLIC_POLICY.is_file() else PRIVATE_POLICY


def run(root: Path, manifest: Path | None = None) -> subprocess.CompletedProcess[str]:
    command = [str(SCANNER), str(root), "--policy", str(POLICY)]
    if manifest is not None:
        command += ["--asset-manifest", str(manifest)]
    return subprocess.run(command, text=True, capture_output=True, check=False)


def expect_fail(root: Path, needle: str, manifest: Path | None = None) -> None:
    result = run(root, manifest)
    output = result.stdout + result.stderr
    assert result.returncode == 1, output
    assert needle in output, output


def write_manifest(path: Path, asset: Path, digest: str) -> None:
    path.write_text(json.dumps({
        "schema_version": 1,
        "assets": [{
            "path": asset.as_posix(),
            "sha256": digest,
            "class": "ORIGINAL_PROJECT",
            "author": "boundary-self-test",
            "source": "synthetic-test",
            "license": "test-only",
            "modification": "none",
            "attribution": "none",
            "redistribution": True,
        }],
    }), encoding="utf-8")


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="mc-public-boundary-") as raw:
        base = Path(raw)

        clean = base / "clean"
        clean.mkdir()
        (clean / "README.md").write_text("# Synthetic public tree\n", encoding="utf-8")
        assert run(clean).returncode == 0

        private_marker = (
            b"/home/oliver/Projects/odcpw/"
            b"magiccarpet/research/"
            b"private/x"
        )
        cases = {
            "pe": ("payload.txt", b"MZ" + b"\0" * 64, "FORBIDDEN_MAGIC"),
            "zip": ("payload.txt", b"PK\x03\x04" + b"\0" * 64, "FORBIDDEN_MAGIC"),
            "empty_zip": (
                "payload.txt",
                b"PK\x05\x06" + b"\0" * 18,
                "FORBIDDEN_MAGIC",
            ),
            "bzip_renamed": ("payload.txt", b"BZh91AY&SY", "FORBIDDEN_MAGIC"),
            "woff2_renamed": (
                "payload.txt",
                b"wOF2" + b"\0" * 64,
                "UNMANIFESTED_ASSET",
            ),
            "nul_binary": (
                "payload.txt",
                b"synthetic\0binary",
                "UNMANIFESTED_BINARY",
            ),
            "ghidra": ("analysis.gpr", b"synthetic", "FORBIDDEN_SUFFIX"),
            "archive": ("bundle.7z", b"synthetic", "FORBIDDEN_SUFFIX"),
            "tar_suffix": ("bundle.tar", b"synthetic", "FORBIDDEN_SUFFIX"),
            "private_path": (
                "notes.md",
                private_marker,
                "FORBIDDEN_TEXT",
            ),
            "unmanifested_media": (
                "image.png",
                b"\x89PNG\r\n\x1a\nsynthetic",
                "UNMANIFESTED_ASSET",
            ),
        }
        for label, (name, content, needle) in cases.items():
            root = base / label
            root.mkdir()
            (root / name).write_bytes(content)
            expect_fail(root, needle)

        symlink = base / "symlink"
        symlink.mkdir()
        (symlink / "outside").symlink_to("/etc/passwd")
        expect_fail(symlink, "SYMLINK")

        ghidra_directory = base / "ghidra_directory"
        ghidra_directory.mkdir()
        (ghidra_directory / "project.rep").mkdir()
        expect_fail(ghidra_directory, "FORBIDDEN_DIRECTORY_SUFFIX")

        nested_git = base / "nested_git"
        (nested_git / "nested/.git").mkdir(parents=True)
        (nested_git / "nested/.git/payload.txt").write_bytes(b"MZsynthetic")
        expect_fail(nested_git, "NESTED_GIT_DIRECTORY")

        large_private_path = base / "large_private_path"
        large_private_path.mkdir()
        (large_private_path / "large.txt").write_bytes(
            b"x" * (2 * 1024 * 1024 + 1) + private_marker
        )
        expect_fail(large_private_path, "FORBIDDEN_TEXT")

        nul_private_path = base / "nul_private_path"
        nul_private_path.mkdir()
        (nul_private_path / "nul.txt").write_bytes(b"\0" + private_marker)
        expect_fail(nul_private_path, "FORBIDDEN_TEXT")

        gitignore_private_path = base / "gitignore_private_path"
        gitignore_private_path.mkdir()
        (gitignore_private_path / ".gitignore").write_bytes(private_marker + b"\n")
        expect_fail(gitignore_private_path, "FORBIDDEN_TEXT")

        manifest_ok = base / "manifest_ok"
        manifest_ok.mkdir()
        asset = Path("original.png")
        content = b"\x89PNG\r\n\x1a\nsynthetic-original-project-image"
        (manifest_ok / asset).write_bytes(content)
        digest = hashlib.sha256(content).hexdigest()
        manifest = base / "assets.json"
        write_manifest(manifest, asset, digest)
        assert run(manifest_ok, manifest).returncode == 0

        empty_metadata = base / "empty_metadata"
        empty_metadata.mkdir()
        (empty_metadata / asset).write_bytes(content)
        bad_manifest = base / "bad-assets.json"
        bad_manifest.write_text(json.dumps({
            "schema_version": 1,
            "assets": [{
                "path": asset.as_posix(),
                "sha256": digest,
                "class": "",
                "author": None,
                "source": "",
                "license": "",
                "modification": "",
                "attribution": "",
                "redistribution": True,
            }],
        }), encoding="utf-8")
        expect_fail(empty_metadata, "empty/non-string", bad_manifest)

        private_metadata = base / "private_metadata"
        private_metadata.mkdir()
        (private_metadata / asset).write_bytes(content)
        private_manifest = base / "private-assets.json"
        private_manifest.write_text(json.dumps({
            "schema_version": 1,
            "assets": [{
                "path": asset.as_posix(),
                "sha256": digest,
                "class": "ORIGINAL_PROJECT",
                "author": "boundary-self-test",
                "source": private_marker.decode("ascii"),
                "license": "test-only",
                "modification": "none",
                "attribution": "none",
                "redistribution": True,
            }],
        }), encoding="utf-8")
        expect_fail(private_metadata, "forbidden private marker", private_manifest)

        mismatch = base / "manifest_mismatch"
        mismatch.mkdir()
        (mismatch / asset).write_bytes(content + b"changed")
        expect_fail(mismatch, "ASSET_HASH_MISMATCH", manifest)

        lfs = base / "lfs"
        lfs.mkdir()
        forbidden_hash = json.loads(POLICY.read_text(encoding="utf-8"))[
            "forbidden_sha256"
        ][0]["sha256"]
        pointer = (
            "version https://git-lfs.github.com/spec/v1\n"
            "ext-0-synthetic boundary-test\n"
            f"oid sha256:{forbidden_hash}\n"
            "size 70046593\n"
        )
        (lfs / "innocent.txt").write_text(pointer, encoding="ascii")
        expect_fail(lfs, "FORBIDDEN_HASH")

    print("PUBLIC_BOUNDARY_SELF_TEST_PASS cases=22")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
