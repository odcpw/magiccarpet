#!/usr/bin/env python3
"""Focused regression tests for reusable/open conformance matrix profiles."""

from __future__ import annotations

import collections
import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
WORKSPACE = TOOLS.parent
CANONICAL = WORKSPACE / "level1-conformance" / "level1-conformance-matrix.json"
sys.path.insert(0, str(TOOLS))

import render_level1_matrix as renderer  # noqa: E402
import validate_level1_matrix as validator  # noqa: E402


class MatrixProfileTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.inventory = validator.capture_cargo_test_inventory(WORKSPACE)
        cls.canonical = validator.load_matrix(CANONICAL)
        cls.profile = validator.open_matrix_validation_profile(
            schema_id="mc2-level2-conformance-matrix",
            schema_version=1,
            expected_rows=182,
            expected_authored_indices=308,
            expected_dispatch_slots=845,
        )

    def validate_open(self, document: dict) -> dict:
        with tempfile.TemporaryDirectory(prefix="matrix-profile-") as directory:
            path = Path(directory) / "matrix.json"
            path.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
            return validator.validate_file(path, self.inventory, self.profile)

    def open_document(self) -> dict:
        document = copy.deepcopy(self.canonical)
        document["schema"]["id"] = self.profile.schema_id
        document["schema"]["version"] = self.profile.schema_version
        row = document["rows"][0]
        row["classification"] = "Partial"
        row["mismatch_detail"] = ["Level 2 implementation is not closed yet"]
        row["plan_item"] = document["plan_items"][0]["id"]
        document["plan_items"][0]["owned_rows"] = [row["id"]]
        totals = document["meta"]["totals"]
        totals["by_classification"]["Exact"] -= 1
        totals["by_classification"]["Partial"] += 1
        subsystem = totals["by_subsystem"][row["subsystem"]]
        subsystem["Exact"] -= 1
        subsystem["Partial"] = subsystem.get("Partial", 0) + 1
        return document

    def assert_rejected(self, mutate, text: str) -> None:
        document = self.open_document()
        mutate(document)
        with self.assertRaisesRegex(validator.ValidationError, text):
            self.validate_open(document)

    def test_honest_non_closed_level2_matrix_validates(self) -> None:
        summary = self.validate_open(self.open_document())
        self.assertEqual(summary["rows"], 182)
        self.assertEqual(summary["exact_rows"], 181)
        self.assertEqual(summary["non_exact_rows"], 1)
        self.assertFalse(summary.get("closed", False))

    def test_open_profile_rejects_duplicate_ids(self) -> None:
        self.assert_rejected(
            lambda document: document["rows"][1].__setitem__("id", document["rows"][0]["id"]),
            "duplicate ID",
        )

    def test_open_profile_rejects_missing_census_partition(self) -> None:
        def remove_partition(document: dict) -> None:
            row_id = next(iter(document["authored_census"]["row_coverage"]))
            document["authored_census"]["row_coverage"][row_id] = []

        self.assert_rejected(remove_partition, "authored_indices does not exactly")

    def test_open_profile_rejects_unresolved_test(self) -> None:
        def unresolved(document: dict) -> None:
            document["rows"][0]["evidence_bindings"][0]["test"] = "missing::test"

        self.assert_rejected(unresolved, "does not resolve exactly once")

    def test_open_profile_rejects_non_exact_without_owner(self) -> None:
        self.assert_rejected(
            lambda document: document["rows"][0].__setitem__("plan_item", None),
            "expected non-empty string",
        )

    def test_open_profile_rejects_stale_pin(self) -> None:
        self.assert_rejected(
            lambda document: document["rows"][0]["source"].__setitem__("commit", "0" * 40),
            "pinned remc2 commit",
        )

    def test_open_profile_rejects_invalid_exclusion(self) -> None:
        self.assert_rejected(
            lambda document: document["exclusions"][0]["source_anchors"].clear(),
            "expected at least one source anchor",
        )

    def test_open_profile_rejects_plan_cycle(self) -> None:
        self.assert_rejected(
            lambda document: document["plan_items"][0]["depends_on"].append(document["plan_items"][0]["id"]),
            "plan dependency cycle",
        )

    def test_generic_renderer_is_open_and_default_renderer_is_unchanged(self) -> None:
        document = self.open_document()
        source_bytes = (json.dumps(document, sort_keys=True) + "\n").encode()
        summary = validator.validate_document(document, self.inventory, self.profile)
        rendered = renderer.render_document(document, source_bytes, summary, profile=self.profile)
        self.assertIn("mc2-level2-conformance-matrix", rendered)
        self.assertIn("non-closed matrix makes no completeness claim", rendered)
        self.assertNotIn("Bounded private Level 1 conformance is closed", rendered)
        default_source = CANONICAL.read_bytes()
        level1_summary = validator.require_closed(CANONICAL, self.inventory)
        self.assertEqual(
            renderer.render_document(
                validator.load_matrix(CANONICAL), default_source, level1_summary
            ),
            renderer.render(
                validator.load_matrix(CANONICAL), default_source, level1_summary
            ),
        )

    def test_generic_cli_accepts_explicit_matrix_profile_and_output(self) -> None:
        document = self.open_document()
        profile_document = {
            "matrix_validation": {
                "schema_id": self.profile.schema_id,
                "schema_version": self.profile.schema_version,
                "pinned_commit": self.profile.pinned_commit,
                "expected_rows": self.profile.expected_rows,
                "expected_authored_indices": self.profile.expected_authored_indices,
                "expected_dispatch_slots": self.profile.expected_dispatch_slots,
            }
        }
        with tempfile.TemporaryDirectory(prefix="matrix-profile-cli-") as directory:
            root = Path(directory)
            matrix = root / "level2.json"
            profile = root / "profile.json"
            output = root / "level2.md"
            matrix.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
            profile.write_text(json.dumps(profile_document, indent=2) + "\n", encoding="utf-8")
            validate = subprocess.run(
                [
                    sys.executable,
                    str(TOOLS / "validate_level1_matrix.py"),
                    str(matrix),
                    "--profile",
                    str(profile),
                    "--json",
                ],
                cwd=WORKSPACE,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(validate.returncode, 0, validate.stdout + validate.stderr)
            self.assertEqual(json.loads(validate.stdout)["non_exact_rows"], 1)
            render = subprocess.run(
                [
                    sys.executable,
                    str(TOOLS / "render_level1_matrix.py"),
                    "--input",
                    str(matrix),
                    "--profile",
                    str(profile),
                    "--output",
                    str(output),
                ],
                cwd=WORKSPACE,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(render.returncode, 0, render.stdout + render.stderr)
            self.assertIn("non-closed matrix makes no completeness claim", output.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
