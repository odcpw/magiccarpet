import importlib.util
import xml.etree.ElementTree as ET
from pathlib import Path

import pytest


TOOL = Path(__file__).resolve().parents[1] / "run_engine_session_editmode.py"
SPEC = importlib.util.spec_from_file_location("engine_session_editmode", TOOL)
assert SPEC and SPEC.loader
editmode = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(editmode)

SELECTED = ("Example.FirstFixture.TestA", "Example.SecondFixture.TestB")


def write_report(path, names):
    root = ET.Element("test-run", result="Passed", total=str(len(names)),
                      passed=str(len(names)), failed="0", skipped="0", inconclusive="0")
    ET.SubElement(ET.SubElement(root, "properties"), "property", name="platform", value="EditMode")
    for name in names:
        ET.SubElement(root, "test-case", fullname=name, result="Passed")
    ET.ElementTree(root).write(path)


def test_exact_batch_requires_the_requested_set_across_fixtures(tmp_path):
    report = tmp_path / "results.xml"
    write_report(report, SELECTED[::-1])

    assert editmode.parse_report(report, "", test_names=SELECTED)["names"] == sorted(SELECTED)


@pytest.mark.parametrize("names, error", [
    (SELECTED[:1], "missing="),
    ((SELECTED[0], "Example.SecondFixture.TestBExtra"), "unexpected="),
    ((SELECTED[0], SELECTED[0]), "duplicate test names"),
])
def test_passing_unity_report_cannot_hide_a_bad_selection(tmp_path, names, error):
    report = tmp_path / "results.xml"
    write_report(report, names)

    with pytest.raises(editmode.EngineSessionEditModeError, match=error):
        editmode.parse_report(report, "", test_names=SELECTED)
