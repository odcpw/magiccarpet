#!/usr/bin/env python3
"""Prove that the public worker mount namespace cannot see private home state."""

from __future__ import annotations

import subprocess
import shutil
import tempfile
from pathlib import Path


HERE = Path(__file__).resolve().parent
SCANNER = HERE / "audit_public_tree.py"
BOUNDARY_TEST = HERE / "test_public_boundary.py"
POLICY = HERE / "public-boundary-policy.json"


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="mc-boundary-probe-") as raw:
        public = Path(raw) / "public"
        (public / "tools/provenance").mkdir(parents=True)
        (public / "provenance").mkdir()
        (public / "sentinel.txt").write_text("PUBLIC_OK\n", encoding="ascii")
        shutil.copy2(SCANNER, public / "tools/provenance/audit_public_tree.py")
        shutil.copy2(BOUNDARY_TEST, public / "tools/provenance/test_public_boundary.py")
        shutil.copy2(POLICY, public / "provenance/public-boundary-policy.json")
        command = [
            "bwrap",
            "--unshare-all",
            "--die-with-parent",
            "--new-session",
            "--clearenv",
            "--ro-bind", "/usr", "/usr",
            "--symlink", "usr/bin", "/bin",
            "--symlink", "usr/lib", "/lib",
            "--symlink", "usr/lib64", "/lib64",
            "--symlink", "usr/sbin", "/sbin",
            "--dir", "/etc",
            "--ro-bind", "/etc/ld.so.cache", "/etc/ld.so.cache",
            "--ro-bind", "/etc/localtime", "/etc/localtime",
            "--tmpfs", "/tmp",
            "--dir", "/workspace",
            "--bind", str(public), "/workspace",
            "--proc", "/proc",
            "--dev", "/dev",
            "--chdir", "/workspace",
            "--setenv", "HOME", "/tmp",
            "--setenv", "PATH", "/usr/bin:/bin",
            "/bin/sh", "-c",
            (
                "/usr/bin/python3 tools/provenance/audit_public_tree.py /workspace "
                "--policy /workspace/provenance/public-boundary-policy.json && "
                "/usr/bin/python3 tools/provenance/test_public_boundary.py && "
                'test "$(cat sentinel.txt)" = PUBLIC_OK && '
                "test ! -e /home && "
                "test ! -e /run && "
                "test ! -e /root && "
                "test ! -e /var && "
                "/usr/bin/python3 -c 'import socket; s=socket.socket(); "
                "s.settimeout(0.1); assert s.connect_ex((\"1.1.1.1\", 53)) != 0'"
            ),
        ]
        result = subprocess.run(command, text=True, capture_output=True, check=False)
        if result.returncode != 0:
            print(result.stdout, end="")
            print(result.stderr, end="")
            return result.returncode
    print("PUBLIC_SANDBOX_SELF_TEST_PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
