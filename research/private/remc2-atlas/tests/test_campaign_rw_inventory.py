from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[4]
TOOL = ROOT / "research/private/remc2-atlas/build_campaign_rw_inventory.py"
ALLOWED_CLASSES = {
    "immutable_fact",
    "session_state",
    "derived_index",
    "presentation_only",
}
EXPECTED_POSITIONS = {
    "prepass",
    "slot_visit",
    "same_pass_epilogue",
    "following_pass_cleanup",
    "postpass",
}


class CampaignRwInventoryTest(unittest.TestCase):
    def run_builder(self, output_dir: Path, summary: Path) -> None:
        result = subprocess.run(
            [
                sys.executable,
                str(TOOL),
                "--output-dir",
                str(output_dir),
                "--summary",
                str(summary),
            ],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
            timeout=60,
        )
        self.assertIn("transitions", result.stdout)

    def test_two_runs_are_byte_identical_and_complete(self) -> None:
        with tempfile.TemporaryDirectory(prefix="campaign-rw-inventory-") as name:
            root = Path(name)
            first_dir = root / "first"
            second_dir = root / "second"
            first_summary = root / "first.md"
            second_summary = root / "second.md"
            self.run_builder(first_dir, first_summary)
            self.run_builder(second_dir, second_summary)

            first_artifact = (first_dir / "campaign-rw-inventory.json").read_bytes()
            second_artifact = (second_dir / "campaign-rw-inventory.json").read_bytes()
            self.assertEqual(first_artifact, second_artifact)
            self.assertEqual(
                (first_dir / "campaign-rw-inventory.sha256").read_bytes(),
                (second_dir / "campaign-rw-inventory.sha256").read_bytes(),
            )
            self.assertEqual(first_summary.read_bytes(), second_summary.read_bytes())
            digest = hashlib.sha256(first_artifact).hexdigest()
            self.assertEqual(
                (first_dir / "campaign-rw-inventory.sha256")
                .read_text(encoding="utf-8")
                .split()[0],
                digest,
            )

            try:
                artifact = json.loads(first_artifact)
            except json.JSONDecodeError as exc:
                self.fail(f"generated artifact is not valid JSON: {exc}")
            self.assertEqual(artifact["schema_version"], "campaign-rw-inventory-v1")
            self.assertEqual(
                set(artifact["route"]["schedule_positions"]),
                EXPECTED_POSITIONS,
            )
            self.assertEqual(
                {row["kind"] for row in artifact["route"]["route_kinds"]},
                set(
                    row["slot_kind"]
                    for row in artifact["transitions"]
                    if row["slot_kind"] is not None
                ),
            )
            transitions = artifact["transitions"]
            self.assertGreaterEqual(len(transitions), 40)
            self.assertEqual(
                [row["transition_ordinal"] for row in transitions],
                list(range(len(transitions))),
            )
            route_kinds = {
                row["kind"] for row in artifact["route"]["route_kinds"]
            }
            self.assertTrue(set(range(13)).issubset(route_kinds))
            self.assertEqual(
                set(row["slot_kind"] for row in transitions if row["slot_kind"] is not None),
                route_kinds,
            )
            for transition in transitions:
                self.assertIn(transition["schedule_position"], EXPECTED_POSITIONS)
                self.assertEqual(
                    [row["ordinal"] for row in transition["rng_draws"]],
                    list(range(len(transition["rng_draws"]))),
                )
                self.assertEqual(
                    [row["ordinal"] for row in transition["emissions"]],
                    list(range(len(transition["emissions"]))),
                )
                self.assertTrue(transition["rust_handlers"])
                for cell in transition["cells"]:
                    self.assertTrue(cell["evidence"])
                    classification = cell["classification"]
                    self.assertTrue(
                        classification in ALLOWED_CLASSES or classification is None
                    )
                    if classification is None:
                        self.assertTrue(cell["open_questions"])
                    for item in cell["evidence"]:
                        self.assertTrue(item["file"])
                        self.assertIsInstance(item["line"], int)
                        self.assertGreater(item["line"], 0)

            self.assertGreater(artifact["coverage"]["open_question_count"], 0)
            self.assertTrue(artifact["coverage"]["known_unreached_callbacks"])
            line_counts: dict[str, int] = {}
            for transition in transitions:
                evidence_rows = list(transition["rust_handlers"]) + list(
                    transition["source_handlers"]
                )
                evidence_rows += [
                    item["evidence"]
                    for item in transition["rng_draws"]
                    + transition["emissions"]
                ]
                for item in evidence_rows:
                    evidence_path = ROOT / item["file"]
                    self.assertTrue(evidence_path.is_file(), item["file"])
                    if item["file"] not in line_counts:
                        line_counts[item["file"]] = len(
                            evidence_path.read_text(encoding="utf-8").splitlines()
                        )
                    self.assertGreaterEqual(item["line"], 1)
                    self.assertLessEqual(item["line"], line_counts[item["file"]])
            for callback in artifact["coverage"]["known_unreached_callbacks"]:
                for item in callback["evidence"]:
                    evidence_path = ROOT / item["file"]
                    self.assertTrue(evidence_path.is_file(), item["file"])
                    if item["file"] not in line_counts:
                        line_counts[item["file"]] = len(
                            evidence_path.read_text(encoding="utf-8").splitlines()
                        )
                    self.assertGreaterEqual(item["line"], 1)
                    self.assertLessEqual(item["line"], line_counts[item["file"]])


if __name__ == "__main__":
    unittest.main()
