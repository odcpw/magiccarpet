#!/usr/bin/env python3
"""Run and validate the current Level 2 Unity EditMode development lane.

This command selects the Level 2 managed production-path test class from the
current EditMode assembly.  It intentionally validates the selected names and
their results without pinning the assembly's evolving global test count.
"""

from __future__ import annotations

import argparse
import hashlib
import hmac
import json
import os
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any


EDITMODE_ASSEMBLY = "MagicCarpet.Native.EditModeTests"
LEVEL2_TEST_FILTER = "MagicCarpet.Native.Tests.NativeCampaignSessionEditModeTests"
CF13_GENERIC_CONSUMER_TEST = (
    LEVEL2_TEST_FILTER
    + ".Cf13GenericCampaignConsumerOpensStepsDrainsFramesAndResets"
)
UNITY_EDITOR = Path("/home/oliver/Unity/Hub/Editor/6000.3.20f1/Editor/Unity")
UNITY_TIMEOUT_SECONDS = 600


class Level2EditModeError(RuntimeError):
    """Raised when the current Level 2 report is absent or invalid."""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise Level2EditModeError(message)


def _counter(root: ET.Element, name: str) -> int:
    raw = root.attrib.get(name)
    try:
        return int(raw) if raw is not None else -1
    except ValueError as error:
        raise Level2EditModeError(f"Level 2 report has a non-integer {name} counter") from error


def parse_level2_report(path: Path) -> dict[str, Any]:
    """Validate a fresh Unity EditMode XML report for the Level 2 class.

    The class prefix is the stable selection contract.  Individual test names
    and the total number of tests may grow as Level 2 development continues.
    """

    _require(path.is_file() and not path.is_symlink(), f"missing Level 2 XML: {path}")
    root = ET.parse(path).getroot()
    _require(root.tag == "test-run", f"unexpected Level 2 XML root: {path}")
    properties = {
        item.attrib.get("name"): item.attrib.get("value")
        for item in root.findall(".//property")
    }
    _require(properties.get("platform") == "EditMode", "Level 2 XML platform is not EditMode")

    cases = root.findall(".//test-case")
    _require(cases, "Level 2 test selection is empty")
    names = [case.attrib.get("fullname", "") for case in cases]
    _require(all(names), "Level 2 XML contains an unnamed test case")
    _require(len(names) == len(set(names)), "Level 2 XML contains duplicate test names")
    prefix = LEVEL2_TEST_FILTER + "."
    unexpected = sorted(name for name in names if not name.startswith(prefix))
    _require(not unexpected, f"Level 2 selection contains unrelated tests: {unexpected}")
    _require(
        CF13_GENERIC_CONSUMER_TEST in names,
        "Level 2 selection is missing the CF13 generic campaign consumer test",
    )

    results = [case.attrib.get("result", "") for case in cases]
    _require(
        all(result == "Passed" for result in results),
        "Level 2 XML contains a non-passing test case",
    )
    total = _counter(root, "total")
    passed = _counter(root, "passed")
    failed = _counter(root, "failed")
    _require(total == len(cases), "Level 2 XML total disagrees with selected cases")
    _require(passed == len(cases), "Level 2 XML passed counter disagrees with selected cases")
    _require(failed == 0, "Level 2 XML reports failed tests")
    for field in ("inconclusive", "skipped"):
        _require(_counter(root, field) == 0, f"Level 2 XML contains {field} tests")
    _require(root.attrib.get("result") == "Passed", "Level 2 tests did not pass")

    return {
        "path": str(path.resolve()),
        "platform": "EditMode",
        "selected": len(names),
        "total": total,
        "passed": passed,
        "failed": failed,
        "names": sorted(names),
    }


def verify_release_plugin_identity(project: Path) -> str:
    """Require Unity to load the exact current release artifact."""

    release = project.parent / "remc2-sim-rs" / "target" / "release" / "libsim_capi.so"
    installed = project / "Assets" / "Plugins" / "x86_64" / "libsim_capi.so"
    for path, label in ((release, "release plugin"), (installed, "Unity plugin")):
        _require(path.is_file() and not path.is_symlink(), f"missing {label}: {path}")
    release_sha256 = hashlib.sha256(release.read_bytes()).hexdigest()
    installed_sha256 = hashlib.sha256(installed.read_bytes()).hexdigest()
    _require(
        hmac.compare_digest(installed_sha256, release_sha256),
        "Unity plugin does not match the current release libsim_capi.so",
    )
    return release_sha256


def build_unity_command(
    project: Path,
    results: Path,
    log: Path,
    *,
    unity: Path = UNITY_EDITOR,
) -> list[str]:
    """Build the real Unity CLI command for the current Level 2 class."""

    return [
        str(unity),
        "-batchmode",
        "-nographics",
        "-projectPath",
        str(project),
        "-runTests",
        "-testPlatform",
        "EditMode",
        "-assemblyNames",
        EDITMODE_ASSEMBLY,
        "-testFilter",
        LEVEL2_TEST_FILTER,
        "-testResults",
        str(results),
        "-logFile",
        str(log),
    ]


def run_level2_editmode(
    project: Path,
    results: Path,
    log: Path,
    *,
    unity: Path = UNITY_EDITOR,
    environment: dict[str, str] | None = None,
) -> dict[str, Any]:
    """Execute Unity and read back only the fresh selected report."""

    _require(project.is_dir(), f"Unity project is missing: {project}")
    plugin_sha256 = verify_release_plugin_identity(project)
    for path, label in ((results, "results"), (log, "log")):
        _require(not path.exists() and not path.is_symlink(), f"pre-existing {label}: {path}")
        path.parent.mkdir(parents=True, exist_ok=True)

    command = build_unity_command(project, results, log, unity=unity)
    completed = subprocess.run(
        command,
        cwd=project,
        env=environment or os.environ.copy(),
        check=False,
        timeout=UNITY_TIMEOUT_SECONDS,
    )
    _require(completed.returncode == 0, f"Level 2 Unity command failed ({completed.returncode})")
    return {**parse_level2_report(results), "plugin_sha256": plugin_sha256}


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run current Level 2 EditMode tests without a global test-count lock."
    )
    parser.add_argument("--project", type=Path, default=Path.cwd())
    parser.add_argument("--results", type=Path, required=True)
    parser.add_argument("--log", type=Path, required=True)
    parser.add_argument("--unity", type=Path, default=UNITY_EDITOR)
    args = parser.parse_args()

    try:
        report = run_level2_editmode(
            args.project.resolve(),
            args.results.resolve(),
            args.log.resolve(),
            unity=args.unity.resolve(),
        )
    except (Level2EditModeError, OSError, ET.ParseError, subprocess.TimeoutExpired) as error:
        print(json.dumps({"ok": False, "status": "FAIL", "error": str(error)}), file=sys.stderr)
        return 1

    print(json.dumps({
        "ok": True,
        "status": "PASS",
        "assembly": EDITMODE_ASSEMBLY,
        "filter": LEVEL2_TEST_FILTER,
        **report,
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
