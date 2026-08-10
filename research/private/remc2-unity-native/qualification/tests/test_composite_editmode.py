from __future__ import annotations

import copy
import importlib.util
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path


TOOL = Path(__file__).resolve().parents[1] / "composite_editmode.py"
SPEC = importlib.util.spec_from_file_location("composite_editmode", TOOL)
assert SPEC and SPEC.loader
composite = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(composite)


def write_xml(path: Path, results: list[tuple[str, str]], result: str) -> None:
    passed = sum(value == "Passed" for _, value in results)
    failed = sum(value == "Failed" for _, value in results)
    root = ET.Element(
        "test-run",
        result=result,
        total=str(len(results)),
        passed=str(passed),
        failed=str(failed),
        inconclusive="0",
        skipped="0",
    )
    properties = ET.SubElement(root, "properties")
    ET.SubElement(properties, "property", name="platform", value="EditMode")
    for name, value in results:
        ET.SubElement(root, "test-case", fullname=name, result=value)
    ET.ElementTree(root).write(path, encoding="utf-8", xml_declaration=True)


class CompositeEditModeTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.base = self.root / "base.xml"
        self.focused = self.root / "focused.xml"
        self.source = self.root / "source.cs"
        self.source.write_text("sealed source\n")
        self.names = sorted(composite.PLAN29_ARCHIVAL_EDITMODE_NAMES)
        self.failed = self.names[:3]
        write_xml(
            self.base,
            [(name, "Failed" if name in self.failed else "Passed") for name in self.names],
            "Failed(Child)",
        )
        write_xml(self.focused, [(name, "Passed") for name in self.failed], "Passed")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def receipt(self) -> dict:
        return composite.build_composite(
            self.base, self.focused, {"test_source": self.source}
        )

    def test_exact_failed_to_focused_union_passes(self) -> None:
        receipt = self.receipt()
        self.assertEqual(
            composite.validate_composite(receipt),
            self.names,
        )

    def test_duplicate_base_name_fails(self) -> None:
        results = [(name, "Failed" if name in self.failed else "Passed") for name in self.names]
        results[-1] = results[0]
        write_xml(self.base, results, "Failed(Child)")
        with self.assertRaises(composite.CompositeError):
            self.receipt()

    def test_missing_or_unrelated_focused_case_fails(self) -> None:
        for names in (self.failed[:-1], [*self.failed, "unrelated"]):
            with self.subTest(names=names):
                write_xml(self.focused, [(name, "Passed") for name in names], "Passed")
                with self.assertRaises(composite.CompositeError):
                    self.receipt()

    def test_nonpassing_focused_case_fails(self) -> None:
        write_xml(
            self.focused,
            [(name, "Failed" if index == 0 else "Passed") for index, name in enumerate(self.failed)],
            "Failed(Child)",
        )
        with self.assertRaises(composite.CompositeError):
            self.receipt()

    def test_count_list_and_source_hash_mutations_fail(self) -> None:
        for mutation in ("count", "duplicate", "source"):
            receipt = self.receipt()
            with self.subTest(mutation=mutation):
                if mutation == "count":
                    receipt["expected_count"] -= 1
                elif mutation == "duplicate":
                    receipt["union_passed_names"][1] = receipt["union_passed_names"][0]
                else:
                    self.source.write_text("changed source\n")
                if mutation != "source":
                    unsigned = {
                        key: value for key, value in receipt.items() if key != "receipt_sha256"
                    }
                    receipt["receipt_sha256"] = composite.sha256_bytes(
                        composite.canonical_json(unsigned)
                    )
                with self.assertRaises(composite.CompositeError):
                    composite.validate_composite(receipt)
                self.source.write_text("sealed source\n")


if __name__ == "__main__":
    unittest.main(verbosity=2)
