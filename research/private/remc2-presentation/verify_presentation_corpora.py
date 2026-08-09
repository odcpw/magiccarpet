#!/usr/bin/env python3
"""Verify the mode-separated Level 1 Night presentation corpus manifests."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent
CORPORA = ROOT / "corpora"
SCHEMA_PATH = ROOT / "corpus-schema.json"
QUALITY_PATH = ROOT / "quality" / "level1-night-quality-target.json"
EXACT_REFERENCE = ROOT.parent / "remc2-unity-native" / "qualification" / "level1-reference-corpus.json"


def _check(checks: list[dict[str, Any]], name: str, passed: bool, detail: str) -> None:
    checks.append({"name": name, "passed": passed, "detail": detail})


def verify() -> dict[str, Any]:
    schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
    quality = json.loads(QUALITY_PATH.read_text(encoding="utf-8"))
    checks: list[dict[str, Any]] = []

    _check(checks, "schema identity", schema.get("schema_id") == "mc-presentation-corpus-v1" and schema.get("schema_version") == 1, str(SCHEMA_PATH))
    _check(checks, "quality target identity", quality.get("quality_target_id") == "level1-night-presentation-v1", str(QUALITY_PATH))
    _check(checks, "protected Exact reference exists", EXACT_REFERENCE.is_file(), str(EXACT_REFERENCE))

    manifests: dict[str, dict[str, Any]] = {}
    required = set(schema["required_manifest_fields"])
    required_build = set(schema["build_metadata_fields"])
    for mode, config in schema["modes"].items():
        directory = str(config["corpus_directory"])
        root = CORPORA / directory
        manifest_path = root / "manifest.json"
        manifest: dict[str, Any] = {}
        if manifest_path.is_file():
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifests[mode] = manifest
        _check(checks, f"{mode} corpus root exists", root.is_dir(), str(root))
        _check(checks, f"{mode} manifest exists", manifest_path.is_file(), str(manifest_path))
        _check(checks, f"{mode} manifest fields", required.issubset(manifest), ", ".join(sorted(required - set(manifest))))
        _check(checks, f"{mode} mode/root match", manifest.get("mode") == mode and manifest.get("corpus_directory") == directory, str(manifest_path))
        build_metadata = manifest.get("build_metadata", {})
        _check(checks, f"{mode} build metadata", required_build.issubset(build_metadata), ", ".join(sorted(required_build - set(build_metadata))))
        _check(checks, f"{mode} profile matches schema", manifest.get("profile") == config.get("profile"), str(manifest_path))
        _check(checks, f"{mode} renderer matches schema", manifest.get("renderer") == config.get("renderer"), str(manifest_path))
        _check(checks, f"{mode} placeholder is honest", manifest.get("capture_status") == "placeholder-not-captured" and manifest.get("determinism", {}).get("hash_status") == "not-captured", str(manifest_path))

        route_names = {route.get("capture_name") for route in manifest.get("capture_plan", [])}
        required_route_names = {route.get("capture_name") for route in schema["capture_plan"]}
        _check(checks, f"{mode} required capture plan", required_route_names.issubset(route_names), ", ".join(sorted(required_route_names - route_names)))

        files = [path for path in root.rglob("*") if path.is_file()]
        non_manifest_files = [path for path in files if path.name != "manifest.json"]
        _check(checks, f"{mode} dry run has no unregistered payload", not non_manifest_files, ", ".join(str(path) for path in non_manifest_files) or "none")

        provenance = manifest.get("provenance", {})
        _check(checks, f"{mode} no embedded owner payload", provenance.get("owner_payload_embedded") is False, str(manifest_path))
        if mode == "exact":
            _check(checks, "Exact reference was not copied", provenance.get("protected_reference_copied") is False, str(manifest_path))
            _check(checks, "Exact manifest does not claim cross-mode proof", manifest.get("determinism", {}).get("policy") == "indexed-byte-hash", str(manifest_path))
        else:
            _check(checks, f"{mode} gameplay authority is Rust", provenance.get("gameplay_authority") == "Rust simulation core", str(manifest_path))

    for mode, manifest in manifests.items():
        for other_mode, other_config in schema["modes"].items():
            if other_mode == mode:
                continue
            _check(checks, f"{mode} does not name {other_mode} root", other_config["corpus_directory"] not in json.dumps(manifest), str(CORPORA / schema["modes"][mode]["corpus_directory"] / "manifest.json"))

    for failure_id in schema["visual_review_failure_ids"]:
        _check(checks, f"quality target covers {failure_id}", failure_id in quality.get("failure_thresholds", {}), str(QUALITY_PATH))

    status = "PASS" if all(check["passed"] for check in checks) else "FAIL"
    return {"status": status, "checks": checks, "failure_count": sum(not check["passed"] for check in checks)}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    summary = verify()
    if args.json:
        print(json.dumps(summary, indent=2, sort_keys=True))
    else:
        print(summary["status"] + " presentation corpus verification")
        for check in summary["checks"]:
            print(("PASS" if check["passed"] else "FAIL") + " " + check["name"] + ": " + check["detail"])
    return 0 if summary["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
