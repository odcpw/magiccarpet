#!/usr/bin/env python3
"""Positive and mutation-negative tests for validate_level1_matrix.py."""

from __future__ import annotations

import collections
import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any, Callable

TOOLS_DIR = Path(__file__).resolve().parent
WORKSPACE_ROOT = TOOLS_DIR.parent
CANONICAL_MATRIX = (
    WORKSPACE_ROOT / "level1-conformance" / "level1-conformance-matrix.json"
)
sys.path.insert(0, str(TOOLS_DIR))

import validate_level1_matrix as validator  # noqa: E402


class Level1MatrixValidatorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.inventory = validator.capture_cargo_test_inventory(WORKSPACE_ROOT)
        cls.canonical = validator.load_matrix(CANONICAL_MATRIX)

    def validate_temporary(self, document: dict[str, Any]) -> dict[str, Any]:
        with tempfile.TemporaryDirectory(prefix="level1-matrix-test-") as directory:
            path = Path(directory) / "matrix.json"
            path.write_text(
                json.dumps(document, indent=2) + "\n",
                encoding="utf-8",
            )
            return validator.validate_file(path, self.inventory)

    def assert_mutation_rejected(
        self,
        mutate: Callable[[dict[str, Any]], None],
        message_fragment: str,
    ) -> None:
        document = copy.deepcopy(self.canonical)
        mutate(document)
        with self.assertRaisesRegex(
            validator.ValidationError, message_fragment
        ):
            self.validate_temporary(document)

    def test_canonical_matrix_passes(self) -> None:
        summary = self.validate_temporary(copy.deepcopy(self.canonical))
        self.assertEqual(
            summary,
            {
                "ok": True,
                "rows": 182,
                "exact_rows": 98,
                "non_exact_rows": 84,
                "exclusions": 18,
                "plan_items": 22,
                "authored_indices": 308,
                "dispatch_slots": 845,
                "cargo_tests": 429,
            },
        )

    def test_unknown_cargo_test_binding_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            document["rows"][0]["evidence_bindings"][0]["test"] = (
                "unknown::test::binding"
            )

        self.assert_mutation_rejected(mutate, "does not resolve exactly once")

    def test_dangling_plan_reference_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            row = next(
                row for row in document["rows"] if row["classification"] != "Exact"
            )
            row["plan_item"] = "PLAN-DOES-NOT-EXIST"

        self.assert_mutation_rejected(mutate, "dangling plan_item reference")

    def test_duplicate_authored_census_index_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            indices = document["authored_census"]["nonzero_indices"]
            indices.append(indices[0])

        self.assert_mutation_rejected(mutate, "duplicate values")

    def test_missing_authored_census_index_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            document["authored_census"]["nonzero_indices"].pop()

        self.assert_mutation_rejected(mutate, "expected exactly 308 indices")

    def test_duplicate_dispatch_slot_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            slots = document["dispatch_census"]["slots"]
            slots[-1] = slots[0]

        self.assert_mutation_rejected(mutate, "duplicate values")

    def test_missing_dispatch_slot_mapping_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            mapping = document["dispatch_census"]["slot_to_exclusion"]
            mapping.pop(next(iter(mapping)))

        self.assert_mutation_rejected(mutate, "do not exactly partition slots")

    def test_bad_dispatch_exclusion_mapping_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            mapping = document["dispatch_census"]["slot_to_exclusion"]
            mapping[next(iter(mapping))] = "EXC-DOES-NOT-EXIST"

        self.assert_mutation_rejected(mutate, "unknown exclusion")

    def test_plan_dependency_cycle_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            plans = {plan["id"]: plan for plan in document["plan_items"]}
            plans["PLAN-01"]["depends_on"] = ["PLAN-02"]
            plans["PLAN-02"]["depends_on"] = ["PLAN-01"]

        self.assert_mutation_rejected(mutate, "plan dependency cycle")

    def test_exact_row_without_executable_evidence_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            row = next(
                row for row in document["rows"] if row["classification"] == "Exact"
            )
            row["evidence_bindings"] = []

        self.assert_mutation_rejected(mutate, "no resolving executable rust-test")

    def test_non_exact_row_without_owned_rows_entry_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            row = next(
                row for row in document["rows"] if row["classification"] != "Exact"
            )
            owner = next(
                plan
                for plan in document["plan_items"]
                if plan["id"] == row["plan_item"]
            )
            owner["owned_rows"].remove(row["id"])

        self.assert_mutation_rejected(mutate, "not a bijection")

    def test_non_exact_row_with_duplicate_ownership_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            row = next(
                row for row in document["rows"] if row["classification"] != "Exact"
            )
            other = next(
                plan
                for plan in document["plan_items"]
                if plan["id"] != row["plan_item"]
            )
            other["owned_rows"].append(row["id"])

        self.assert_mutation_rejected(mutate, "duplicate ownership")

    def test_exact_row_ownership_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            row = next(
                row for row in document["rows"] if row["classification"] == "Exact"
            )
            document["plan_items"][0]["owned_rows"].append(row["id"])

        self.assert_mutation_rejected(mutate, "Exact row .* must not be owned")

    def test_inconsistent_meta_total_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            document["meta"]["totals"]["rows"] += 1

        self.assert_mutation_rejected(mutate, "expected recomputed value 182")

    def test_unknown_field_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            document["rows"][0]["quiet_default"] = True

        self.assert_mutation_rejected(mutate, "unknown fields")

    def test_missing_field_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            del document["rows"][0]["confidence"]

        self.assert_mutation_rejected(mutate, "missing fields")

    def test_weakly_typed_integer_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            document["dispatch_census"]["slot_count"] = "845"

        self.assert_mutation_rejected(mutate, "expected integer")

    def test_duplicate_json_field_is_rejected(self) -> None:
        canonical_text = CANONICAL_MATRIX.read_text(encoding="utf-8")
        duplicate_text = canonical_text.replace(
            '"schema": {',
            '"schema": null,\n  "schema": {',
            1,
        )
        with tempfile.TemporaryDirectory(prefix="level1-matrix-test-") as directory:
            path = Path(directory) / "matrix.json"
            path.write_text(duplicate_text, encoding="utf-8")
            with self.assertRaisesRegex(
                validator.ValidationError, "duplicate field 'schema'"
            ):
                validator.validate_file(path, self.inventory)

    def test_ambiguous_live_test_inventory_is_rejected(self) -> None:
        inventory = collections.Counter(self.inventory)
        bound_name = self.canonical["rows"][0]["evidence_bindings"][0]["test"]
        inventory[bound_name] = 2
        with self.assertRaisesRegex(
            validator.ValidationError, "does not resolve exactly once"
        ):
            validator.validate_document(copy.deepcopy(self.canonical), inventory)


if __name__ == "__main__":
    unittest.main(verbosity=2)
