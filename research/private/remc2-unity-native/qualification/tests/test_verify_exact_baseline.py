#!/usr/bin/env python3
"""Focused tests for the Exact baseline guard and ABI additive policy."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


QUALIFICATION = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(QUALIFICATION))
import verify_exact_baseline  # noqa: E402


class ExactBaselineGuardTests(unittest.TestCase):
    def _repo(self) -> tuple[tempfile.TemporaryDirectory[str], Path, dict]:
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        files = {
            "project/settings.txt": "settings-v1\n",
            "frame/frame.txt": "frame 320x200\n",
            "readiness/readiness.txt": "ready\n",
            "corpus/corpus.json": "{\"version\": 1}\n",
            "scripts/run.sh": "#!/bin/sh\n-mc-qualified\n",
            "abi/mc_sim.h": "#define MC_NATIVE_ABI_VERSION 2u\nint mc_native_open(void);\n",
            "contracts/native.cs": "internal const uint AbiVersion = 2;\n",
        }
        for relative, content in files.items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
        manifest = {
            "schema_version": 1,
            "baseline": {"source_commit": "HEAD", "unity_version": "test", "native_abi_version": 2},
            "protected_files": {
                "project": ["project/settings.txt"],
                "frame": ["frame/frame.txt"],
                "readiness": ["readiness/readiness.txt"],
                "corpus": ["corpus/corpus.json"],
                "scripts": ["scripts/run.sh"],
                "contracts": ["contracts/native.cs"],
            },
            "abi_v2_header": "abi/mc_sim.h",
            "contract_assertions": [
                {"id": "abi", "path": "contracts/native.cs", "regex": "AbiVersion = 2"}
            ],
        }
        manifest_path = root / "manifest.json"
        manifest_path.write_text(json.dumps(manifest) + "\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(root), "init", "-q"], check=True)
        subprocess.run(["git", "-C", str(root), "config", "user.email", "test@example.invalid"], check=True)
        subprocess.run(["git", "-C", str(root), "config", "user.name", "Exact guard test"], check=True)
        subprocess.run(["git", "-C", str(root), "add", "."], check=True)
        subprocess.run(["git", "-C", str(root), "commit", "-qm", "baseline"], check=True)
        return temporary, root, manifest

    def test_baseline_passes(self) -> None:
        temporary, root, _ = self._repo()
        self.addCleanup(temporary.cleanup)
        result = verify_exact_baseline.verify(root, root / "manifest.json")
        self.assertTrue(result["pass"], result)

    def test_each_protected_category_rejects_synthetic_drift(self) -> None:
        for category, relative in (
            ("project", "project/settings.txt"),
            ("frame", "frame/frame.txt"),
            ("readiness", "readiness/readiness.txt"),
            ("corpus", "corpus/corpus.json"),
            ("scripts", "scripts/run.sh"),
            ("contracts", "contracts/native.cs"),
        ):
            with self.subTest(category=category):
                temporary, root, _ = self._repo()
                path = root / relative
                path.write_text(path.read_text(encoding="utf-8") + "synthetic drift\n", encoding="utf-8")
                result = verify_exact_baseline.verify(root, root / "manifest.json")
                self.assertFalse(result["pass"], result)
                failures = [failure for failure in result["failures"] if failure["category"] == category]
                self.assertTrue(failures, result)
                temporary.cleanup()

    def test_additive_header_symbol_is_reported_but_allowed(self) -> None:
        temporary, root, _ = self._repo()
        self.addCleanup(temporary.cleanup)
        header = root / "abi/mc_sim.h"
        header.write_text(header.read_text(encoding="utf-8") + "int mc_native_new_surface(void);\n", encoding="utf-8")
        result = verify_exact_baseline.verify(root, root / "manifest.json")
        self.assertTrue(result["pass"], result)
        abi_checks = [check for check in result["checks"] if check["category"] == "abi_v2"]
        self.assertEqual(abi_checks[0]["added_symbols"], ["mc_native_new_surface"])

    def test_existing_header_signature_change_is_forbidden(self) -> None:
        temporary, root, _ = self._repo()
        self.addCleanup(temporary.cleanup)
        header = root / "abi/mc_sim.h"
        header.write_text("#define MC_NATIVE_ABI_VERSION 2u\nint mc_native_open(int changed);\n", encoding="utf-8")
        result = verify_exact_baseline.verify(root, root / "manifest.json")
        self.assertFalse(result["pass"], result)
        self.assertTrue(any(failure["category"] == "abi_v2" for failure in result["failures"]))


if __name__ == "__main__":
    unittest.main()
