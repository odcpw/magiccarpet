#!/usr/bin/env python3
from __future__ import annotations

import copy
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
import validate_enhanced_coverage  # noqa: E402


class EnhancedCoverageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.matrix = validate_enhanced_coverage.load_matrix(ROOT / "enhanced-coverage-matrix.json")

    def test_level1_matrix_is_complete_and_fail_closed(self) -> None:
        self.assertEqual(validate_enhanced_coverage.validate_matrix(self.matrix), [])

    def test_wildcard_and_missing_family_are_rejected(self) -> None:
        invalid = copy.deepcopy(self.matrix)
        invalid["coverage"] = [row for row in invalid["coverage"] if row.get("family_key") != "Goat"]
        invalid["coverage"].append(
            {
                "id": "occupant.wildcard",
                "domain": "occupant",
                "family_key": "*",
                "source_owner": "test",
                "current_rust_surface": "test",
                "unity_consumer": "test",
                "route": "new-gfx-q",
                "state_fields": ["must fail"],
            }
        )
        errors = validate_enhanced_coverage.validate_matrix(invalid)
        self.assertTrue(any("wildcard" in error for error in errors))
        self.assertTrue(any("missing Level 1 family: Goat" in error for error in errors))

    def test_exclusions_require_reasons_and_supported_rows_cannot_be_wildcards(self) -> None:
        invalid = copy.deepcopy(self.matrix)
        reserved = next(row for row in invalid["coverage"] if row.get("family_key") == "Reserved")
        reserved.pop("exclusion_reason")
        errors = validate_enhanced_coverage.validate_matrix(invalid)
        self.assertTrue(any("explicit exclusion needs a reason" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
