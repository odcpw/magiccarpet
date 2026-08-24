from __future__ import annotations

import importlib.util
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path


TOOL = Path(__file__).resolve().parents[1] / "run_level2_editmode.py"
SPEC = importlib.util.spec_from_file_location("level2_editmode", TOOL)
assert SPEC and SPEC.loader
level2 = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(level2)


def write_xml(path: Path, names: list[str], result: str = "Passed") -> None:
    root = ET.Element(
        "test-run",
        result=result,
        total=str(len(names)),
        passed=str(len(names) if result == "Passed" else 0),
        failed=str(0 if result == "Passed" else len(names)),
        inconclusive="0",
        skipped="0",
    )
    properties = ET.SubElement(root, "properties")
    ET.SubElement(properties, "property", name="platform", value="EditMode")
    for name in names:
        ET.SubElement(root, "test-case", fullname=name, result=result)
    ET.ElementTree(root).write(path, encoding="utf-8", xml_declaration=True)


class Level2EditModeQualificationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_added_legitimate_level2_test_is_accepted_without_global_count_lock(self) -> None:
        prefix = level2.LEVEL2_TEST_FILTER
        names = [
            level2.CF13_GENERIC_CONSUMER_TEST,
            f"{prefix}.Level2ProductionStartupOpensCampaignSessionAndValidatesDayProfile",
            f"{prefix}.Level2CampaignCabiIsReachableFromManagedSession",
            f"{prefix}.NewLegitimateLevel2ProductionCheck",
        ]
        report = self.root / "current-level2.xml"
        write_xml(report, names)

        parsed = level2.parse_level2_report(report)

        self.assertEqual(parsed["selected"], 4)
        self.assertEqual(parsed["total"], 4)
        self.assertEqual(parsed["names"], sorted(names))

    def test_zero_selected_level2_tests_are_rejected(self) -> None:
        report = self.root / "empty-level2.xml"
        write_xml(report, [])

        with self.assertRaisesRegex(level2.Level2EditModeError, "selection is empty"):
            level2.parse_level2_report(report)

    def test_missing_cf13_generic_consumer_test_is_rejected(self) -> None:
        report = self.root / "missing-cf13.xml"
        write_xml(report, [
            f"{level2.LEVEL2_TEST_FILTER}.Level2ProductionStartupOpensCampaignSessionAndValidatesDayProfile",
        ])

        with self.assertRaisesRegex(level2.Level2EditModeError, "missing the CF13"):
            level2.parse_level2_report(report)

    def test_selection_rejects_unrelated_level1_test(self) -> None:
        report = self.root / "mixed.xml"
        write_xml(report, [
            level2.CF13_GENERIC_CONSUMER_TEST,
            f"{level2.LEVEL2_TEST_FILTER}.Level2ProductionStartupOpensCampaignSessionAndValidatesDayProfile",
            "MagicCarpet.Native.Tests.NativeSessionEditModeTests.AbiAndManagedLayoutsArePinned",
        ])

        with self.assertRaisesRegex(level2.Level2EditModeError, "unrelated tests"):
            level2.parse_level2_report(report)

    def test_unity_command_selects_the_level2_class(self) -> None:
        command = level2.build_unity_command(
            Path("/tmp/project"),
            Path("/tmp/results.xml"),
            Path("/tmp/results.log"),
            unity=Path("/tmp/Unity"),
        )

        self.assertEqual(command[command.index("-assemblyNames") + 1], level2.EDITMODE_ASSEMBLY)
        self.assertEqual(command[command.index("-testFilter") + 1], level2.LEVEL2_TEST_FILTER)

    def test_release_plugin_identity_requires_exact_installed_bytes(self) -> None:
        project = self.root / "remc2-unity-native"
        release = self.root / "remc2-sim-rs" / "target" / "release" / "libsim_capi.so"
        installed = project / "Assets" / "Plugins" / "x86_64" / "libsim_capi.so"
        release.parent.mkdir(parents=True)
        installed.parent.mkdir(parents=True)
        release.write_bytes(b"exact current release")
        installed.write_bytes(b"exact current release")

        digest = level2.verify_release_plugin_identity(project)

        self.assertEqual(len(digest), 64)
        installed.write_bytes(b"stale installed plugin")
        with self.assertRaisesRegex(
            level2.Level2EditModeError,
            "does not match the current release",
        ):
            level2.verify_release_plugin_identity(project)


if __name__ == "__main__":
    unittest.main(verbosity=2)
