from __future__ import annotations

import hashlib
import importlib.util
import json
import struct
import tempfile
import unittest
from pathlib import Path
from unittest import mock


MODULE_PATH = Path(__file__).parents[1] / "build_owner_asset_catalog.py"
SPEC = importlib.util.spec_from_file_location("owner_catalog", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
catalog = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(catalog)


def fake_rnc(unpacked_size: int, payload: bytes = b"abc") -> bytes:
    return (
        b"RNC\x01"
        + struct.pack(">IIHHBB", unpacked_size, len(payload), 0x1234, 0x5678, 0, 1)
        + payload
    )


class HeaderAndIdentityTests(unittest.TestCase):
    def test_rnc_header_is_big_endian_and_bounded(self) -> None:
        archive = fake_rnc(0x6604)
        header = catalog.rnc_header(archive)
        self.assertIsNotNone(header)
        assert header is not None
        self.assertEqual(header["method"], 1)
        self.assertEqual(header["unpacked_size"], 0x6604)
        self.assertEqual(header["archive_size"], len(archive))
        self.assertIsNone(catalog.rnc_header(archive[:10]))

    def test_sprite_hashes_keep_indices_and_transparency_separate(self) -> None:
        # Row: transparent x0, visible palette indices 7 and 8, end.
        encoded = bytes((0xFF, 2, 7, 8, 0))
        decoded = catalog.decode_indexed_rle(encoded, 0, 3, 1)
        self.assertEqual(decoded["status"], "decoded_indexed_rle")
        self.assertEqual(
            decoded["indexed_pixels_sha256"],
            hashlib.sha256(bytes((0, 7, 8))).hexdigest(),
        )
        self.assertEqual(
            decoded["transparency_mask_sha256"],
            hashlib.sha256(bytes((0, 1, 1))).hexdigest(),
        )


class ContainerIndexTests(unittest.TestCase):
    def test_level_offsets_become_content_addressed_records(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "LEVELS").mkdir()
            first = fake_rnc(0x6604)
            second = fake_rnc(0x6604, b"xyz")
            dat = first + second
            offsets = [0, len(first), len(dat)] + [len(dat)] * 997
            (root / "LEVELS/LEVELS.DAT").write_bytes(dat)
            (root / "LEVELS/LEVELS.TAB").write_bytes(
                struct.pack("<1000I", *offsets)
            )
            records, unresolved = catalog.parse_level_records(
                root / "LEVELS/LEVELS.DAT",
                root / "LEVELS/LEVELS.TAB",
                root,
            )
        self.assertEqual(len(records), 2)
        self.assertEqual(unresolved, [])
        self.assertEqual(records[0]["rnc"]["unpacked_size"], 0x6604)
        self.assertEqual(
            records[0]["content_address"],
            f"sha256:{hashlib.sha256(first).hexdigest()}",
        )

    def test_tmap_schema_keeps_metadata_group_and_null_dimensions(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "DATA").mkdir()
            archive = fake_rnc(64 * 64)
            (root / "DATA/TMAPS1-0.DAT").write_bytes(archive)
            (root / "DATA/TMAPS1-0.TAB").write_bytes(
                struct.pack("<iiHiiH", 17, 0, 9, 0, len(archive), 9)
            )
            records, unresolved = catalog.parse_tmap_records(
                root / "DATA/TMAPS1-0.DAT",
                root / "DATA/TMAPS1-0.TAB",
                root,
                1,
            )
        self.assertEqual(unresolved, [])
        self.assertEqual(len(records), 1)
        self.assertEqual(records[0]["metadata_dword_0"], 17)
        self.assertEqual(records[0]["animation_or_allocation_group"], 9)
        self.assertEqual(records[0]["palette_path"], "DATA/PALN-0.DAT")
        self.assertEqual(
            records[0]["dimensions"], {"width": None, "height": None}
        )

    def test_tmap_rejects_archive_that_crosses_index_span(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "DATA").mkdir()
            archive = fake_rnc(4096, b"payload")
            (root / "DATA/TMAPS1-0.DAT").write_bytes(archive)
            (root / "DATA/TMAPS1-0.TAB").write_bytes(
                struct.pack("<iihiih", 0, 0, -1, 0, len(archive) - 2, -1)
            )
            records, unresolved = catalog.parse_tmap_records(
                root / "DATA/TMAPS1-0.DAT",
                root / "DATA/TMAPS1-0.TAB",
                root,
                1,
            )
        self.assertEqual(records[0]["animation_or_allocation_group"], -1)
        self.assertEqual(records[0]["decoder_status"], "unresolved_container")
        self.assertEqual(len(unresolved), 1)


class BoundaryTests(unittest.TestCase):
    def test_missing_required_sources_fail_before_catalogue(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(
                catalog.CatalogInputError, "missing .* required sources"
            ):
                catalog.build_catalog(Path(directory), None)

    def test_symlink_inside_source_root_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.write_bytes(b"x")
            (root / "link").symlink_to(target)
            with self.assertRaisesRegex(catalog.CatalogInputError, "symlink"):
                catalog.source_files(root.resolve())

    def test_public_output_path_is_rejected(self) -> None:
        with self.assertRaisesRegex(
            catalog.CatalogInputError, "must remain under"
        ):
            catalog.validate_private_output(Path("/tmp/public-catalog.json"))

    def test_outputs_are_atomic_and_confined_to_private_root(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            private_root = Path(directory)
            output = private_root / "generated/owner-asset-catalog.json"
            fixture = {
                "schema_version": 1,
                "scope": "test",
                "catalog_sha256": "abc",
                "counts": {},
                "provenance": [],
                "realm_contract": {},
                "coverage_matrix": {},
                "coverage_contract": {},
                "unresolved": [],
            }
            with mock.patch.object(catalog, "PRIVATE_ROOT", private_root):
                catalog.write_outputs(fixture, output)
            self.assertTrue(output.is_file())
            self.assertTrue(
                output.with_name("owner-asset-catalog-summary.json").is_file()
            )
            self.assertTrue(
                output.with_name("owner-asset-catalog-unresolved.csv").is_file()
            )
            self.assertEqual(list(output.parent.glob("*.tmp")), [])


class OwnerDataIntegrationTests(unittest.TestCase):
    @unittest.skipUnless(
        catalog.DEFAULT_OWNER_ROOT.is_dir(), "private owner-data root unavailable"
    )
    def test_real_owner_catalog_is_deterministic_and_complete_at_core_surfaces(
        self,
    ) -> None:
        first = catalog.build_catalog(catalog.DEFAULT_OWNER_ROOT, None)
        second = catalog.build_catalog(catalog.DEFAULT_OWNER_ROOT, None)
        self.assertEqual(first["catalog_sha256"], second["catalog_sha256"])
        self.assertEqual(
            json.dumps(first, sort_keys=True), json.dumps(second, sort_keys=True)
        )
        self.assertEqual(first["realm_contract"]["1"], "night")
        self.assertEqual(first["realm_contract"]["level_1_regression_map_type"], 1)
        self.assertEqual(first["counts"]["entries_by_category"]["level_record"], 183)
        self.assertEqual(
            first["counts"]["entries_by_category"]["terrain_texture_map"], 1512
        )
        self.assertEqual(
            first["counts"]["entries_by_category"]["world_sprites"], 1572
        )
        for row in first["coverage_matrix"].values():
            self.assertEqual(
                row["expected_files"],
                row["catalogued_files"] + row["missing_files"],
            )
            self.assertEqual(
                row["entry_count"],
                row["structurally_valid_entries"]
                + row["unresolved_container_or_decode_entries"],
            )
        self.assertEqual(first["coverage_matrix"]["levels"]["unresolved_files"], 2)
        self.assertEqual(
            first["coverage_matrix"]["buildings"]["unresolved_files"], 3
        )
        self.assertFalse(
            any(str(catalog.DEFAULT_OWNER_ROOT) in json.dumps(row) for row in first["files"])
        )


if __name__ == "__main__":
    unittest.main()
