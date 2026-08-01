import importlib.util
import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TOOL = ROOT / "validate_asset_tiers.py"
SPEC = importlib.util.spec_from_file_location("asset_tiers", TOOL)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class AssetTierTests(unittest.TestCase):
    def setUp(self) -> None:
        self.schema = json.loads((ROOT / "asset-tier-schema.json").read_text())
        self.manifest = json.loads((ROOT / "asset-tier-manifest.json").read_text())

    def test_placeholder_manifest_is_honest_and_complete(self) -> None:
        self.assertEqual(MODULE.validate(self.schema, self.manifest), [])
        self.assertFalse(self.manifest["owner_payload_embedded"])

    def test_validated_entry_requires_a_hash(self) -> None:
        manifest = json.loads(json.dumps(self.manifest))
        manifest["entries"][0]["status"] = "validated"
        self.assertTrue(any("validated without sha256" in error for error in MODULE.validate(self.schema, manifest)))

    def test_family_mixing_and_missing_original_are_rejected(self) -> None:
        manifest = json.loads(json.dumps(self.manifest))
        manifest["entries"][0]["tier"] = "optional-high-resolution"
        manifest["entries"][0]["status"] = "fallback-original"
        self.assertTrue(any("lacks mandatory original" in error for error in MODULE.validate(self.schema, manifest)))


if __name__ == "__main__":
    unittest.main()
