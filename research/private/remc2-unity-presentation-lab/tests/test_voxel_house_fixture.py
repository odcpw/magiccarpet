import json
import os
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_CODE = ROOT / "Assets" / "Editor" / "VoxelMode" / "VoxelHouseRenderFixture.cs"
FIXTURE_MENU = ROOT / "Assets" / "Editor" / "VoxelMode" / "VoxelHouseRenderFixtureMenu.cs"
README = ROOT / "Assets" / "Editor" / "VoxelMode" / "README.md"


def fixture_report() -> Path | None:
    configured = os.environ.get("MC2_VOXEL_FIXTURE_REPORT")
    candidates = [
        Path(configured) if configured else None,
        Path("/home/oliver/voxel-import-output/voxel-house-build29.fixture-report.json"),
    ]
    for candidate in candidates:
        if candidate and candidate.is_file():
            return candidate
    return None


class VoxelHouseFixtureTests(unittest.TestCase):
    def test_fixture_is_pure_and_stateful(self) -> None:
        source = FIXTURE_CODE.read_text(encoding="utf-8")
        menu = FIXTURE_MENU.read_text(encoding="utf-8")
        readme = README.read_text(encoding="utf-8")
        for state in ("Intact", "Opposite", "NightWindow", "Cutaway", "Breach"):
            self.assertIn(state, source)
        self.assertIn("Replay", source)
        self.assertIn("Reset", source)
        self.assertIn("cells_are_data_only", source)
        self.assertIn('PersistentGameObjectCount', source)
        self.assertIn('["persistent_game_objects"] = VoxelJsonValue.Number("0")', source)
        self.assertNotIn("new GameObject", source)
        self.assertNotIn("using UnityEngine;", source)
        self.assertIn("Build deterministic BUILD-29 state fixture report", menu)
        self.assertIn("reset/replay trace must hash identically twice", readme)

    @unittest.skipUnless(fixture_report(), "fixture report is not mounted")
    def test_golden_fixture_report_has_all_states_and_replay_evidence(self) -> None:
        report_path = fixture_report()
        assert report_path is not None
        report = json.loads(report_path.read_text(encoding="utf-8"))
        self.assertEqual(report["fixture_version"], 1)
        self.assertTrue(report["cells_are_data_only"])
        self.assertEqual(report["cell_count"], 427)
        self.assertEqual(report["room_count"], 6)
        self.assertEqual(report["furniture_count"], 6)
        self.assertEqual(report["lit_window_metadata_count"], 2)
        frames = {frame["state"]: frame for frame in report["frames"]}
        self.assertEqual(set(frames), {"Intact", "Opposite", "NightWindow", "Cutaway", "Breach"})
        self.assertEqual(frames["Intact"]["visible_cells"], 427)
        self.assertEqual(frames["Opposite"]["visible_cells"], 427)
        self.assertEqual(frames["NightWindow"]["lit_windows"], 2)
        self.assertLess(frames["Cutaway"]["visible_cells"], 427)
        self.assertLess(frames["Breach"]["visible_cells"], 427)
        for frame in frames.values():
            self.assertLessEqual(frame["visible_cells"], 4096)
            self.assertLessEqual(frame["batches"], 24)
            self.assertEqual(frame["persistent_game_objects"], 0)
            self.assertEqual(frame["active_cosmetic_debris"], 0)
            self.assertIsNotNone(re.fullmatch(r"[0-9a-f]{64}", frame["sha256"]))
        replay = report["replay"]
        self.assertTrue(replay["reset_replay_matches"])
        self.assertEqual(replay["first_sha256"], replay["reset_sha256"])
        self.assertEqual(replay["sequence"], "Intact,Opposite,NightWindow,Cutaway,Breach")


if __name__ == "__main__":
    unittest.main()
