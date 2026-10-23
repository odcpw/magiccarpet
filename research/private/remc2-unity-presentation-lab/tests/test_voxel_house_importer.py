import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
VOXEL_EDITOR = ROOT / "Assets" / "Editor" / "VoxelMode"
IMPORTER = VOXEL_EDITOR / "VoxelHouseImporter.cs"
JSON_CODE = VOXEL_EDITOR / "VoxelJson.cs"
MENU = VOXEL_EDITOR / "VoxelHouseImporterMenu.cs"


def owner_root() -> Path | None:
    configured = os.environ.get("MC2_VOXEL_OWNER_ROOT")
    candidates = [
        Path(configured) if configured else None,
        Path("/home/oliver/Projects/odcpw/magiccarpet/research/private/generated/voxel-house-lookdev"),
    ]
    for candidate in candidates:
        if candidate and (candidate / "exports/build29/voxel-house-manifest-v1.json").is_file():
            return candidate
    return None


def canonical(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


class VoxelHouseImporterStaticTests(unittest.TestCase):
    def test_editor_only_mapping_contract_is_explicit(self) -> None:
        importer = IMPORTER.read_text(encoding="utf-8")
        json_code = JSON_CODE.read_text(encoding="utf-8")
        menu = MENU.read_text(encoding="utf-8")
        self.assertNotIn("using UnityEngine;", importer)
        self.assertNotIn("GameObject", importer.replace("GameObjects", ""))
        self.assertIn("cells_are_data_only", importer)
        self.assertIn("SourceToUnity at runtime", importer)
        self.assertIn("source_aabb_center_bottom", importer)
        self.assertIn("batched render groups; no per-cell GameObjects", importer)
        self.assertIn("ReadPackageBytes", importer)
        self.assertIn("MaxManifestBytes", importer)
        self.assertIn("ValidateOnly", menu)
        self.assertIn("Assets", importer)
        self.assertIn("Resources", importer)
        self.assertNotIn("Assets/Scripts", importer)
        self.assertIn("duplicate object key", json_code)

    def test_generated_output_is_ignored(self) -> None:
        ignore = (VOXEL_EDITOR / "Generated" / ".gitignore").read_text(encoding="utf-8")
        self.assertIn("*", ignore)
        self.assertIn("!.gitignore", ignore)

    @unittest.skipUnless(owner_root(), "qualified owner package is not mounted")
    def test_golden_package_hashes_caps_and_authority(self) -> None:
        root = owner_root()
        assert root is not None
        manifest_path = root / "exports/build29/voxel-house-manifest-v1.json"
        schema_path = root / "contract/voxel-house-manifest-v1.schema.json"
        recipe_path = root / "exports/build29/surface-recipe-v1.json"
        receipt_path = root / "exports/build29/provenance-receipt.json"
        qualification_path = root / "exports/build29/qualification-report.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        recipe = json.loads(recipe_path.read_text(encoding="utf-8"))
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        qualification = json.loads(qualification_path.read_text(encoding="utf-8"))

        self.assertEqual(manifest["manifest_id"], "private.build29.house.v1")
        self.assertEqual(manifest["coordinate_system"]["anchor"], {"cell_coord": [0, 0, 0], "kind": "source_aabb_center_bottom", "world_m": [0.0, 0.0, 0.0]})
        self.assertEqual(manifest["authority"], {
            "collision": "source_aabb",
            "detail": "voxel_structure_mirror",
            "feedback_to_gameplay": False,
            "gameplay": "source_house_state",
            "standalone_town": False,
            "targeting": "source_authoritative",
        })
        self.assertEqual(hashlib.sha256(schema_path.read_bytes()).hexdigest(), manifest["hashes"]["schema_sha256"])
        self.assertEqual(hashlib.sha256(manifest_path.read_bytes()).hexdigest(), receipt["manifest_sha256"])
        recipe_payload = copy.deepcopy(recipe)
        recipe_payload.pop("recipe_sha256", None)
        self.assertEqual(hashlib.sha256(canonical(recipe_payload)).hexdigest(), manifest["hashes"]["recipe_sha256"])
        semantic_payload = copy.deepcopy(manifest)
        semantic_payload["hashes"].pop("semantic_sha256")
        self.assertEqual(hashlib.sha256(canonical(semantic_payload)).hexdigest(), manifest["hashes"]["semantic_sha256"])
        self.assertEqual(qualification["status"], "pass")
        self.assertFalse(qualification["level1_boundary"]["unity_or_gameplay_placement"])
        for key, cap in {"cells": 4096, "materials": 16}.items():
            self.assertLessEqual(len(manifest[key]), cap)
        for key, cap in {"parts": 64, "supports": 64, "rooms": 64, "furniture": 256, "openings": 128, "render_groups": 64}.items():
            self.assertLessEqual(len(manifest[key]), cap)

    @unittest.skipUnless(owner_root(), "qualified owner package is not mounted")
    def test_contract_rejects_wrong_hash_provenance_anchor_duplicate_and_cap(self) -> None:
        root = owner_root()
        assert root is not None
        validator = root / "contract/validate_voxel_house_manifest.py"
        manifest_path = root / "exports/build29/voxel-house-manifest-v1.json"
        schema_path = root / "contract/voxel-house-manifest-v1.schema.json"
        baseline = json.loads(manifest_path.read_text(encoding="utf-8"))

        mutations = {
            "wrong-hash": lambda payload: payload["hashes"].__setitem__("source_sha256", "0" * 64),
            "wrong-provenance": lambda payload: payload["provenance"].__setitem__("class", "public"),
            "wrong-anchor": lambda payload: payload["coordinate_system"]["anchor"].__setitem__("kind", "visual_center"),
            "duplicate-id": lambda payload: payload["parts"][1].__setitem__("id", payload["parts"][0]["id"]),
            "cap": lambda payload: payload["budgets"]["observed"].__setitem__("cells", 4097),
        }
        with tempfile.TemporaryDirectory(prefix="voxel-importer-contract-") as temporary:
            temporary_root = Path(temporary)
            temporary_manifest = temporary_root / "manifest.json"
            for name, mutate in mutations.items():
                payload = copy.deepcopy(baseline)
                mutate(payload)
                temporary_manifest.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
                result = subprocess.run(
                    [sys.executable, str(validator), str(temporary_manifest), "--schema", str(schema_path)],
                    text=True,
                    capture_output=True,
                    check=False,
                )
                self.assertNotEqual(result.returncode, 0, name)


if __name__ == "__main__":
    unittest.main()
