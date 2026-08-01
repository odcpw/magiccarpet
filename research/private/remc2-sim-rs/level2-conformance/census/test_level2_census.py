#!/usr/bin/env python3
"""Focused production-path and negative tests for the Level 2 census."""

from __future__ import annotations

import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import build_level2_census as census


ROOT = Path(__file__).resolve().parents[5]
FIRST = ROOT / "research/private/generated/l2-debug3.fJ3PaM/level2"
SECOND = ROOT / "research/private/generated/mc2-l2-native2.Ibpz9b/out"
SEALED = Path(__file__).with_name("level2-authored-reset-census.json")


class Level2CensusTests(unittest.TestCase):
    def test_checked_in_census_is_complete(self) -> None:
        value = json.loads(SEALED.read_text())
        census.validate(value)
        for name in census.CENSUS_FILES:
            self.assertEqual(
                value["qualified_exports"][0]["file_sha256"][name],
                value["qualified_exports"][1]["file_sha256"][name],
            )

    def test_cli_validation_path_is_production_bound(self) -> None:
        subprocess.run(
            [
                "python3",
                str(Path(__file__).with_name("build_level2_census.py")),
                "--validate",
                str(SEALED),
            ],
            cwd=ROOT,
            check=True,
        )

    def test_production_builder_rebuilds_the_sealed_denominator(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mc2-l2-census-") as directory:
            output = Path(directory) / "rebuilt.json"
            subprocess.run(
                [
                    "python3",
                    str(Path(__file__).with_name("build_level2_census.py")),
                    "research/private/generated/l2-debug3.fJ3PaM/level2",
                    "research/private/generated/mc2-l2-native2.Ibpz9b/out",
                    "--output",
                    str(output),
                ],
                cwd=ROOT,
                check=True,
            )
            rebuilt = json.loads(output.read_text())
            sealed = json.loads(SEALED.read_text())
            self.assertEqual(rebuilt, sealed)

    def test_duplicate_authored_partition_fails(self) -> None:
        value = copy.deepcopy(json.loads(SEALED.read_text()))
        value["authored"]["partitions"]["null"].append(0)
        with self.assertRaises(ValueError):
            census.validate(value)

    def test_dropped_reset_slot_fails(self) -> None:
        value = copy.deepcopy(json.loads(SEALED.read_text()))
        value["reset"]["slots"].pop()
        with self.assertRaises(ValueError):
            census.validate(value)

    def test_discriminating_role_mutation_fails(self) -> None:
        value = copy.deepcopy(json.loads(SEALED.read_text()))
        building = next(row for row in value["reset"]["slots"] if row["role"] == "building")
        building["role"] = "dolmen"
        with self.assertRaises(ValueError):
            census.validate(value)

    def test_duplicate_reset_partition_membership_fails(self) -> None:
        value = copy.deepcopy(json.loads(SEALED.read_text()))
        value["reset"]["partitions"]["inactive"].append(1)
        with self.assertRaises(ValueError):
            census.validate(value)

    def test_reset_relation_mutation_fails(self) -> None:
        value = copy.deepcopy(json.loads(SEALED.read_text()))
        value["reset"]["slots"][1]["relations"]["next"] ^= 1
        with self.assertRaises(ValueError):
            census.validate(value)

    def test_record_hash_mutation_fails(self) -> None:
        value = copy.deepcopy(json.loads(SEALED.read_text()))
        value["authored"]["rows"][155]["sha256"] = "0" * 64
        with self.assertRaises(ValueError):
            census.validate(value)

    def test_source_provenance_mutation_fails(self) -> None:
        value = copy.deepcopy(json.loads(SEALED.read_text()))
        value["qualified_exports"][1]["source"]["compiled_source_sha256"] = "0" * 64
        with self.assertRaises(ValueError):
            census.validate(value)

    def test_player_relation_mutation_fails(self) -> None:
        value = copy.deepcopy(json.loads(SEALED.read_text()))
        value["wizard_player"]["player_records"][0]["entity_slot"] = 999
        with self.assertRaises(ValueError):
            census.validate(value)


if __name__ == "__main__":
    unittest.main()
