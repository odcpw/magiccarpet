#!/usr/bin/env python3
"""Run the Unity neutral-engine EditMode class or a batch of named tests."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path


ASSEMBLY = "MagicCarpet.Native.EditModeTests"
TEST_FILTER = "MagicCarpet.Native.Tests.EngineSessionEditModeTests"
UNITY = Path("/home/oliver/Unity/Hub/Editor/6000.3.20f1/Editor/Unity")
TIMEOUT_SECONDS = 600


class EngineSessionEditModeError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise EngineSessionEditModeError(message)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def parse_report(
    path: Path, test_filter: str, *, test_names: tuple[str, ...] = ()
) -> dict[str, object]:
    require(path.is_file() and not path.is_symlink(), f"missing fresh Unity XML: {path}")
    root = ET.parse(path).getroot()
    require(root.tag == "test-run", "unexpected Unity XML root")
    properties = {
        item.attrib.get("name"): item.attrib.get("value")
        for item in root.findall(".//property")
    }
    require(properties.get("platform") == "EditMode", "Unity report is not EditMode")
    cases = root.findall(".//test-case")
    require(cases, "Unity engine-session selection is empty")
    names = [case.attrib.get("fullname", "") for case in cases]
    require(len(names) == len(set(names)), "Unity report contains duplicate test names")
    if test_names:
        missing = sorted(set(test_names) - set(names))
        unexpected = sorted(set(names) - set(test_names))
        require(not missing and not unexpected,
                f"Unity selection mismatch: missing={missing}, unexpected={unexpected}")
    else:
        require(
            all(name.startswith(test_filter + ".") for name in names),
            "Unity report contains unrelated tests",
        )
    require(
        all(case.attrib.get("result") == "Passed" for case in cases),
        "Unity report contains a non-passing test",
    )
    require(root.attrib.get("result") == "Passed", "Unity test run did not pass")
    total = int(root.attrib.get("total", "-1"))
    passed = int(root.attrib.get("passed", "-1"))
    failed = int(root.attrib.get("failed", "-1"))
    require(total == len(cases) and passed == len(cases), "Unity counters disagree")
    require(failed == 0, "Unity report contains failures")
    require(
        int(root.attrib.get("skipped", "-1")) == 0
        and int(root.attrib.get("inconclusive", "-1")) == 0,
        "Unity report contains skipped or inconclusive tests",
    )
    return {"selected": len(names), "names": sorted(names)}


def run(
    project: Path,
    plugin: Path,
    results: Path,
    log: Path,
    unity: Path,
    test_filter: str = TEST_FILTER,
    semantic_workspace: Path | None = None,
    semantic_baseline_workspace: Path | None = None,
    *,
    test_names: tuple[str, ...] = (),
    graphics: bool = False,
) -> dict[str, object]:
    if test_names:
        require(len(set(test_names)) == len(test_names), "duplicate requested Unity test names")
        require(
            all(re.fullmatch(r"(?:[A-Za-z_]\w*\.)+[A-Za-z_]\w*", name) for name in test_names),
            "expected fully qualified, non-parameterized Unity test names",
        )
        unity_filter = ";".join(f"^{re.escape(name)}$" for name in test_names)
    else:
        unity_filter = test_filter
    require(project.is_dir(), f"missing Unity project: {project}")
    require(plugin.is_file() and not plugin.is_symlink(), f"missing candidate plugin: {plugin}")
    installed = project / "Assets/Plugins/x86_64/libsim_capi.so"
    require(installed.is_file() and not installed.is_symlink(), "project plugin is missing")
    plugin_hash = sha256_file(plugin)
    require(
        sha256_file(installed) == plugin_hash,
        "Unity project plugin does not equal the explicitly supplied candidate",
    )
    for path, label in ((results, "results"), (log, "log")):
        require(not path.exists() and not path.is_symlink(), f"pre-existing {label}: {path}")
        path.parent.mkdir(parents=True, exist_ok=True)
    command = [str(unity), "-batchmode"]
    if not graphics:
        command.append("-nographics")
    command += [
        "-projectPath", str(project),
        "-runTests", "-testPlatform", "EditMode",
        "-assemblyNames", ASSEMBLY,
        "-testFilter", unity_filter,
        "-testResults", str(results),
        "-logFile", str(log),
    ]
    environment = os.environ.copy()
    if semantic_workspace is not None:
        require(
            semantic_workspace.is_dir() and not semantic_workspace.is_symlink(),
            f"missing semantic workspace: {semantic_workspace}",
        )
        environment["MC_SEMANTIC_VARIANT_WORKSPACE"] = str(semantic_workspace)
    if semantic_baseline_workspace is not None:
        require(
            semantic_baseline_workspace.is_dir()
            and not semantic_baseline_workspace.is_symlink(),
            f"missing semantic baseline workspace: {semantic_baseline_workspace}",
        )
        environment["MC_SEMANTIC_BASELINE_WORKSPACE"] = str(
            semantic_baseline_workspace
        )
    completed = subprocess.run(
        command,
        cwd=project,
        env=environment,
        check=False,
        timeout=TIMEOUT_SECONDS,
    )
    require(completed.returncode == 0, f"Unity command failed ({completed.returncode})")
    report = parse_report(results, test_filter, test_names=test_names)
    require(log.is_file() and log.stat().st_size > 0, "Unity log is empty")
    return {
        "ok": True,
        "status": "PASS",
        "unity": "6000.3.20f1",
        "assembly": ASSEMBLY,
        "filter": unity_filter,
        "plugin_sha256": plugin_hash,
        **report,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--plugin", type=Path, required=True)
    parser.add_argument("--results", type=Path, required=True)
    parser.add_argument("--log", type=Path, required=True)
    parser.add_argument("--unity", type=Path, default=UNITY)
    selection = parser.add_mutually_exclusive_group()
    selection.add_argument("--test-filter", default=TEST_FILTER, help="select a whole test class")
    selection.add_argument("--test-name", action="append", default=[],
                           help="exact, fully qualified test method (repeat to batch)")
    parser.add_argument("--graphics", action="store_true", help="enable Unity's graphics device")
    parser.add_argument("--semantic-workspace", type=Path)
    parser.add_argument("--semantic-baseline-workspace", type=Path)
    args = parser.parse_args()
    try:
        report = run(
            args.project.resolve(),
            args.plugin.resolve(),
            args.results.resolve(),
            args.log.resolve(),
            args.unity.resolve(),
            args.test_filter,
            args.semantic_workspace.resolve()
            if args.semantic_workspace is not None
            else None,
            args.semantic_baseline_workspace.resolve()
            if args.semantic_baseline_workspace is not None
            else None,
            test_names=tuple(args.test_name),
            graphics=args.graphics,
        )
    except (
        EngineSessionEditModeError,
        OSError,
        ValueError,
        ET.ParseError,
        subprocess.TimeoutExpired,
    ) as error:
        print(json.dumps({"ok": False, "status": "FAIL", "error": str(error)}), file=sys.stderr)
        return 1
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
