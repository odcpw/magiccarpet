#!/usr/bin/env python3
from __future__ import annotations

import copy
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
import validate_classic_hd_spec  # noqa: E402


class ClassicHdSpecTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.spec = validate_classic_hd_spec.load_spec(ROOT / "classic-hd-spec.json")

    def test_pinned_spec_is_complete(self) -> None:
        self.assertEqual(validate_classic_hd_spec.validate_spec(self.spec), [])

    def test_dirty_source_and_oracle_claims_are_rejected(self) -> None:
        dirty = copy.deepcopy(self.spec)
        dirty["source"]["commit"] = "dirty-worktree"
        self.assertTrue(validate_classic_hd_spec.validate_spec(dirty))
        oracle = copy.deepcopy(self.spec)
        oracle["provenance"]["original_binary_oracle"] = True
        self.assertTrue(validate_classic_hd_spec.validate_spec(oracle))

    def test_non_four_three_profile_is_rejected(self) -> None:
        invalid = copy.deepcopy(self.spec)
        invalid["milestone_1_policy"]["supported_profiles"][0]["game_resolution"] = [1920, 1080]
        self.assertTrue(validate_classic_hd_spec.validate_spec(invalid))

    def test_every_stage_has_a_discriminating_test_requirement(self) -> None:
        for stage in self.spec["pipeline_contract"]:
            self.assertTrue(stage["test"], stage["id"])


if __name__ == "__main__":
    unittest.main()
