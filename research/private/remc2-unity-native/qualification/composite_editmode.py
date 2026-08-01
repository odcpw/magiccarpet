#!/usr/bin/env python3
"""Build and validate a fail-closed composite Unity EditMode receipt."""

from __future__ import annotations

import argparse
import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any


SCHEMA = "mc2-plan29-composite-editmode/v1"
EDIT_PREFIX = "MagicCarpet.Native.Tests.NativeSessionEditModeTests."
EXPECTED_EDITMODE_NAMES = {
    EDIT_PREFIX + name
    for name in (
        "AbiAndManagedLayoutsArePinned",
        "BulkTerrainCheckpointReusesBuffersWithinAnAllocationBudget",
        "CombatManagedFallbackIsSuppressedForEveryNativeFamily",
        "EconomyManagedFallbackIsSuppressedOnlyByItsNativeCapability",
        "ExitPresentationResetStateIsGenerationAndTickBound",
        "ExitPresentationUsesHistoryFovAndIntegerRgb6PaletteMath",
        "FrozenActorManagedFallbackIsSuppressedOnlyForTheQualifiedThirtyOne",
        "GoatBulkRowsAreExactReusableResettableAndSessionLocal",
        "KnownInputAndCameraRouteMatchesTheQualifiedGolden",
        "LevelOneAutopilotCompletesThroughOrdinaryPlayerInputs",
        "LevelOneAutopilotFreshSessionsHaveIdenticalDigests",
        "LevelOneAutopilotNegativeControlsStopAtTheirObjectives",
        "LevelOneAutopilotObservationExcludesHiddenEnemyState",
        "LevelOneMissionActorsAndFireballAreLiveNativeState",
        "LevelOneNaturalFireflyRetaliationUsesOnlyOrdinaryPlayerInputs",
        "LevelOnePostCompletionAutopilotFightsGroups33And34BeforeExit",
        "LevelOnePostCompletionPilotKeepsAVisibleMultipartTargetLock",
        "LiveSimulationUsesTheRemc2DefaultThirtyHertzCadence",
        "LiveTerrainAndCastleLifecycleAreTypedHashedResettableAndSessionLocal",
        "MissingOrTamperedOwnerInputsFailClosed",
        "MissionActorManagedFallbackIsSuppressedOnlyByItsNativeCapability",
        "NativePaletteIsAtomicAndMatchesQualifiedPalnAtReset",
        "ObjectiveTextboxBlendsAllPaletteInputsThenOverwritesWithForeground",
        "ObjectiveTextboxUsesPixelCentreNearestSamplingAndClearsAtExpiry",
        "OpenResetDisposeAndTwoSessionsRemainIndependent",
        "OwnerCacheUsesItsSemanticClosureIdentity",
        "PresentationFrameOwnsOneIndexedPlaneAndZeroManagedDuplicates",
        "ResolvedAudioSidecarsAreDiscardedWithoutOwnerConsumer",
        "SelectorFramesAdvanceOnlyFromNativeCpuAdmissionOrQualificationFeedback",
        "SpellAssignmentOpcodesDriveTheSourceHandAndTierSnapshot",
        "WorldFrameCompositesSupportedFamiliesWithoutChangingTerrainOnlyContract",
    )
}


class CompositeError(RuntimeError):
    pass


def canonical_json(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def require_file(path: Path, label: str) -> None:
    if not path.is_file() or path.is_symlink():
        raise CompositeError(f"missing or symlinked {label}: {path}")


def parse_xml(path: Path, label: str) -> dict[str, Any]:
    require_file(path, f"{label} XML")
    root = ET.parse(path).getroot()
    if root.tag != "test-run":
        raise CompositeError(f"unexpected {label} XML root")
    properties = {
        item.attrib.get("name"): item.attrib.get("value")
        for item in root.findall(".//property")
    }
    if properties.get("platform") != "EditMode":
        raise CompositeError(f"{label} platform is not EditMode")
    cases = root.findall(".//test-case")
    names = [item.attrib.get("fullname", "") for item in cases]
    if not names or any(not name for name in names):
        raise CompositeError(f"{label} has an unnamed or empty test registry")
    if len(names) != len(set(names)):
        raise CompositeError(f"{label} has duplicate test names")
    results = {name: item.attrib.get("result", "") for name, item in zip(names, cases)}
    if any(result not in {"Passed", "Failed"} for result in results.values()):
        raise CompositeError(f"{label} has a nonterminal test result")
    total = int(root.attrib.get("total", "-1"))
    passed = int(root.attrib.get("passed", "-1"))
    failed = int(root.attrib.get("failed", "-1"))
    if total != len(results) or passed != sum(value == "Passed" for value in results.values()):
        raise CompositeError(f"{label} result counters disagree with test cases")
    if failed != sum(value == "Failed" for value in results.values()):
        raise CompositeError(f"{label} failure counter disagrees with test cases")
    for field in ("inconclusive", "skipped"):
        if int(root.attrib.get(field, "-1")) != 0:
            raise CompositeError(f"{label} contains {field} tests")
    return {
        "path": str(path.resolve()),
        "sha256": sha256_file(path),
        "result": root.attrib.get("result", ""),
        "total": total,
        "passed": passed,
        "failed": failed,
        "results": results,
    }


def build_composite(
    base_xml: Path,
    focused_xml: Path,
    sources: dict[str, Path],
    expected_names: set[str] = EXPECTED_EDITMODE_NAMES,
) -> dict[str, Any]:
    base = parse_xml(base_xml, "base")
    focused = parse_xml(focused_xml, "focused")
    base_names = set(base["results"])
    failed_names = {
        name for name, result in base["results"].items() if result == "Failed"
    }
    focused_names = set(focused["results"])
    if base_names != expected_names:
        raise CompositeError("base XML is not the exact EditMode registry")
    if not failed_names:
        raise CompositeError("base XML has no failed cases to replace")
    if not base["result"].startswith("Failed"):
        raise CompositeError("base XML is not an honest failed full run")
    if focused["result"] != "Passed" or focused["failed"] != 0:
        raise CompositeError("focused XML did not pass")
    if focused_names != failed_names:
        raise CompositeError("focused XML is missing, duplicating, or adding unrelated cases")
    if any(result != "Passed" for result in focused["results"].values()):
        raise CompositeError("focused XML contains a non-passing case")

    source_receipts: dict[str, dict[str, str]] = {}
    if not sources:
        raise CompositeError("composite receipt requires source hashes")
    for name, path in sorted(sources.items()):
        if not name or name in source_receipts:
            raise CompositeError("invalid or duplicate source label")
        require_file(path, f"source {name}")
        source_receipts[name] = {
            "path": str(path.resolve()),
            "sha256": sha256_file(path),
        }

    receipt: dict[str, Any] = {
        "schema": SCHEMA,
        "status": "PASS",
        "expected_count": len(expected_names),
        "base": {key: base[key] for key in ("path", "sha256", "result", "total", "passed", "failed")},
        "focused": {
            key: focused[key]
            for key in ("path", "sha256", "result", "total", "passed", "failed")
        },
        "base_passed_names": sorted(base_names - failed_names),
        "replaced_failed_names": sorted(failed_names),
        "focused_passed_names": sorted(focused_names),
        "union_passed_names": sorted(expected_names),
        "sources": source_receipts,
    }
    receipt["receipt_sha256"] = sha256_bytes(canonical_json(receipt))
    validate_composite(receipt, expected_names)
    return receipt


def validate_composite(
    receipt: dict[str, Any],
    expected_names: set[str] = EXPECTED_EDITMODE_NAMES,
) -> list[str]:
    if receipt.get("schema") != SCHEMA or receipt.get("status") != "PASS":
        raise CompositeError("wrong composite schema or status")
    claimed_hash = receipt.get("receipt_sha256")
    unsigned = {key: value for key, value in receipt.items() if key != "receipt_sha256"}
    if claimed_hash != sha256_bytes(canonical_json(unsigned)):
        raise CompositeError("composite receipt self-hash mismatch")
    if receipt.get("expected_count") != len(expected_names):
        raise CompositeError("composite expected-count drift")
    base_passed = receipt.get("base_passed_names")
    replaced = receipt.get("replaced_failed_names")
    focused = receipt.get("focused_passed_names")
    union = receipt.get("union_passed_names")
    if not all(isinstance(value, list) for value in (base_passed, replaced, focused, union)):
        raise CompositeError("composite name lists are malformed")
    if len(union) != len(set(union)) or set(union) != expected_names:
        raise CompositeError("composite union is missing, duplicated, or unrelated")
    if set(replaced) != set(focused) or set(base_passed) & set(focused):
        raise CompositeError("focused replacement set is not exact")
    if set(base_passed) | set(focused) != expected_names:
        raise CompositeError("composite union does not prove every expected case")
    if not replaced:
        raise CompositeError("composite has no honest replacement cases")

    base = receipt.get("base", {})
    focused_receipt = receipt.get("focused", {})
    if (
        base.get("total") != len(expected_names)
        or base.get("passed") != len(base_passed)
        or base.get("failed") != len(replaced)
        or not str(base.get("result", "")).startswith("Failed")
    ):
        raise CompositeError("base receipt counters are inconsistent")
    if (
        focused_receipt.get("total") != len(focused)
        or focused_receipt.get("passed") != len(focused)
        or focused_receipt.get("failed") != 0
        or focused_receipt.get("result") != "Passed"
    ):
        raise CompositeError("focused receipt counters are inconsistent")
    for label, source in (("base", base), ("focused", focused_receipt)):
        path = Path(str(source.get("path", "")))
        require_file(path, f"{label} receipt input")
        if source.get("sha256") != sha256_file(path):
            raise CompositeError(f"{label} XML hash mismatch")
    sources = receipt.get("sources")
    if not isinstance(sources, dict) or not sources:
        raise CompositeError("composite source hashes are absent")
    for name, source in sources.items():
        if not isinstance(name, str) or not isinstance(source, dict):
            raise CompositeError("malformed composite source entry")
        path = Path(str(source.get("path", "")))
        require_file(path, f"source {name}")
        if source.get("sha256") != sha256_file(path):
            raise CompositeError(f"source hash mismatch: {name}")
    return sorted(union)


def source_argument(value: str) -> tuple[str, Path]:
    if "=" not in value:
        raise argparse.ArgumentTypeError("source must be LABEL=PATH")
    label, raw_path = value.split("=", 1)
    if not label or not raw_path:
        raise argparse.ArgumentTypeError("source must be LABEL=PATH")
    return label, Path(raw_path)


def main() -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    compose = subparsers.add_parser("compose")
    compose.add_argument("--base-results", type=Path, required=True)
    compose.add_argument("--focused-results", type=Path, required=True)
    compose.add_argument("--source", type=source_argument, action="append", required=True)
    compose.add_argument("--output", type=Path, required=True)
    validate = subparsers.add_parser("validate")
    validate.add_argument("--receipt", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "compose":
        sources = dict(args.source)
        if len(sources) != len(args.source):
            raise CompositeError("duplicate source labels")
        receipt = build_composite(args.base_results, args.focused_results, sources)
        args.output.write_bytes(canonical_json(receipt))
    else:
        require_file(args.receipt, "composite receipt")
        receipt = json.loads(args.receipt.read_text())
        validate_composite(receipt)
    print(json.dumps({"ok": True, "status": "PASS", "tests": 31}, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (CompositeError, OSError, ValueError, ET.ParseError) as error:
        print(json.dumps({"ok": False, "status": "FAIL", "error": str(error)}, sort_keys=True))
        raise SystemExit(1)
