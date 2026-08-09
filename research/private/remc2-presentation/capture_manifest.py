#!/usr/bin/env python3
"""Create honest, mode-scoped presentation capture manifests.

This is intentionally a manifest/dry-run tool. It never copies owner assets or
claims that a frame was captured; renderer backends attach evidence later.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


PRESENTATION_ROOT = Path(__file__).resolve().parent
CORPORA_ROOT = PRESENTATION_ROOT / "corpora"
SCHEMA_PATH = PRESENTATION_ROOT / "corpus-schema.json"
QUALITY_TARGET_PATH = PRESENTATION_ROOT / "quality" / "level1-night-quality-target.json"
TOOL_VERSION = "mc-presentation-capture-manifest/1"


class ManifestError(ValueError):
    pass


def load_schema() -> dict[str, Any]:
    return json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))


def _inside(path: Path, root: Path) -> bool:
    try:
        path.resolve().relative_to(root.resolve())
        return True
    except ValueError:
        return False


def _mode_directory(schema: dict[str, Any], mode: str) -> str:
    try:
        return str(schema["modes"][mode]["corpus_directory"])
    except KeyError as exc:
        raise ManifestError("unsupported presentation mode: " + mode) from exc


def _selected_routes(schema: dict[str, Any], route_id: str) -> list[dict[str, Any]]:
    routes = list(schema["capture_plan"])
    if route_id == "all":
        return routes
    selected = [route for route in routes if route["id"] == route_id]
    if not selected:
        raise ManifestError("unknown Level 1 Night route: " + route_id)
    return selected


def _display_metadata(schema: dict[str, Any], profile: str) -> dict[str, Any]:
    try:
        metadata = schema["display_profiles"][profile]
    except KeyError as exc:
        raise ManifestError("unknown display profile: " + profile) from exc
    return {"profile_id": profile, **metadata}


def build_manifest(
    mode: str,
    profile: str,
    route_id: str = "all",
    source_commit: str = "unrecorded",
    build_id: str = "dry-run-unrecorded",
    unity_editor: str = "unrecorded",
    gpu: str = "unrecorded",
    driver: str = "unrecorded",
) -> dict[str, Any]:
    schema = load_schema()
    mode_directory = _mode_directory(schema, mode)
    mode_config = schema["modes"][mode]
    expected_profile = str(mode_config["profile"])
    if profile != expected_profile:
        raise ManifestError(f"profile {profile!r} does not belong to mode {mode!r}; expected {expected_profile!r}")

    routes = _selected_routes(schema, route_id)
    quality_target = json.loads(QUALITY_TARGET_PATH.read_text(encoding="utf-8"))
    hash_policy = str(mode_config["hash_policy"])
    manifest: dict[str, Any] = {
        "schema_id": schema["schema_id"],
        "schema_version": schema["schema_version"],
        "manifest_id": f"{mode_directory}-{profile}-dry-run",
        "mode": mode,
        "profile": profile,
        "corpus_directory": mode_directory,
        "capture_status": "placeholder-not-captured",
        "renderer": mode_config["renderer"],
        "build_metadata": {
            "source_commit": source_commit,
            "build_id": build_id,
            "unity_editor": unity_editor,
            "gpu": gpu,
            "driver": driver,
            "capture_tool_version": TOOL_VERSION,
            "metadata_complete": all(value != "unrecorded" for value in (source_commit, build_id, unity_editor, gpu, driver)),
        },
        "display_metadata": _display_metadata(schema, profile),
        "determinism": {
            "policy": hash_policy,
            "frame_hash": None,
            "state_hash": None,
            "hash_status": "not-captured",
            "input_route_is_authoritative": True,
            "gpu_lit_requires_region_metrics": mode == "unity-enhanced",
        },
        "capture_plan": routes,
        "visual_review": {
            "quality_target_id": quality_target["quality_target_id"],
            "status": "not-reviewed",
            "failure_ids": schema["visual_review_failure_ids"],
            "human_review_required": True,
        },
        "provenance": {
            "track": "P",
            "owner_payload_embedded": False,
            "oracle_comparison": "not-used",
            "project": mode_config["project"],
            "reference_policy": mode_config["reference_policy"],
        },
    }
    if mode == "exact":
        manifest["provenance"].update(
            {
                "protected_reference_path": "research/private/remc2-unity-native/qualification/level1-reference-corpus.json",
                "protected_reference_copied": False,
            }
        )
    else:
        manifest["provenance"].update(
            {
                "gameplay_authority": "Rust simulation core",
                "owner_inputs": "external paths only; not embedded in this corpus",
            }
        )
    return manifest


def write_dry_run_manifest(output: Path, manifest: dict[str, Any], mode: str) -> Path:
    schema = load_schema()
    expected_directory = _mode_directory(schema, mode)
    expected_root = CORPORA_ROOT / expected_directory
    output = output.expanduser()
    if not output.is_absolute():
        output = (Path.cwd() / output).resolve()
    else:
        output = output.resolve()

    if output.exists() and output.is_symlink():
        raise ManifestError("refusing symlink output: " + str(output))
    if not _inside(output, expected_root):
        raise ManifestError(
            f"corpus crossover refused: {output} is outside the {mode} corpus root {expected_root}"
        )
    if _inside(output, PRESENTATION_ROOT / "../remc2-unity-native"):
        raise ManifestError("refusing to write inside the protected Exact project")

    expected_root.mkdir(parents=True, exist_ok=True)
    if output.exists():
        existing = json.loads(output.read_text(encoding="utf-8"))
        if existing.get("mode") != mode or existing.get("corpus_directory") != expected_directory:
            raise ManifestError("existing manifest mode/corpus directory does not match the selected output root")
        if existing.get("capture_status") not in (None, "placeholder-not-captured"):
            raise ManifestError("refusing to overwrite a non-placeholder capture manifest")

    output.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return output


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", required=True, choices=("exact", "classic-hd", "unity-enhanced"))
    parser.add_argument("--profile", required=True)
    parser.add_argument("--route", default="all", help="route id from corpus-schema.json, or all")
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--source-commit", default="unrecorded")
    parser.add_argument("--build-id", default="dry-run-unrecorded")
    parser.add_argument("--unity-editor", default="unrecorded")
    parser.add_argument("--gpu", default="unrecorded")
    parser.add_argument("--driver", default="unrecorded")
    parser.add_argument("--dry-run", action="store_true", help="write a placeholder manifest without frame evidence")
    args = parser.parse_args()
    if not args.dry_run:
        parser.error("only --dry-run manifest generation is implemented; refusing to claim a real capture")

    try:
        manifest = build_manifest(
            mode=args.mode,
            profile=args.profile,
            route_id=args.route,
            source_commit=args.source_commit,
            build_id=args.build_id,
            unity_editor=args.unity_editor,
            gpu=args.gpu,
            driver=args.driver,
        )
        output = write_dry_run_manifest(args.output, manifest, args.mode)
    except (ManifestError, OSError, json.JSONDecodeError) as exc:
        print("FAIL capture manifest: " + str(exc))
        return 1
    print(f"PASS placeholder manifest: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
