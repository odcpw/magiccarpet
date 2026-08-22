#!/usr/bin/env python3
"""Independently rerun and inspect an F10 foundation evidence bundle."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import re
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any

from verify_neutral_game_session import MC_STATUS_OK, NeutralClient


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
PRIVATE = HERE.parents[1]
SIM = PRIVATE / "remc2-sim-rs"
WEB = SIM / "web"
UNITY_PROJECT = PRIVATE / "remc2-unity-native"
FIXTURE = SIM / "crates/sim-core/tests/fixtures/custom-synthetic-v1"
REQUIREMENTS = HERE / "foundation-requirements-v1.json"
WHITELIST = HERE / "custom-package-artifact-whitelist-v1.json"
PLAN = REPO / "docs/carpet-flight-engine-plan.md"
CORPUS = HERE / "level1-reference-corpus.json"
ASSET = "presentation.spell.fireball.projectile_asset"
ENGAGEMENT = "ai.engagement_boundary"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def run(client: NeutralClient) -> dict[str, Any]:
    commands = [
        {"longitudinal": "accelerate"},
        {"longitudinal": "accelerate", "cast_left": True},
    ]
    for tick, command in enumerate(commands):
        require(client.step(1, tick, command) == MC_STATUS_OK,
                f"independent native step failed at {tick}")
    return {
        "snapshot": client.snapshot(),
        "events": client.events(1, 2, 0)["value"]["events"],
    }


def event_ids(value: dict[str, Any]) -> list[str]:
    return [row["event_id"] for row in value["events"]]


def engagement_rows(value: dict[str, Any]) -> list[dict[str, Any]]:
    return [row for row in value["events"] if row["event_id"] == ENGAGEMENT]


def projection(value: dict[str, Any], asset: dict[str, Any]) -> dict[str, Any]:
    snapshot = value["snapshot"]
    state = snapshot["value"]
    return {
        "definition_hash": snapshot["identity"]["definition_hash"],
        "package_graph_hash": snapshot["identity"]["package_graph_hash"],
        "tick": state["tick"], "event_head": state["event_head"],
        "state_hash": state["state_hash"], "conformance_hash": state["conformance_hash"],
        "player_pose": state["player_pose"], "player_motion": state["player_motion"],
        "caster_mana": state["caster_mana"], "objective_count": state["objective_count"],
        "action_sequence": state["action_sequence"], "event_ids": event_ids(value),
        "engagement_pairs": [
            f"{row['actor']}>{row['participant']}" for row in engagement_rows(value)
        ],
        "asset_content_address": asset["value"]["content_address"],
        "asset_indices_base64": base64.b64encode(bytes(asset["value"]["indices"])).decode(),
    }


def rerun_native(plugin: Path, workspaces: dict[str, Path]) -> dict[str, Any]:
    clients = {"mc2": NeutralClient(plugin)}
    clients.update({name: NeutralClient(plugin, workspace=path)
                    for name, path in workspaces.items()})
    try:
        values = {name: run(client) for name, client in clients.items()}
        baseline = values["custom_baseline"]
        base = baseline["snapshot"]["value"]
        require(values["mc2"]["snapshot"]["value"] == base,
                "independent MC2/custom baseline mismatch")
        require(event_ids(values["mc2"]) == event_ids(baseline),
                "independent MC2/custom event mismatch")
        require(base["player_motion"]["target_speed"] == 32
                and values["flight"]["snapshot"]["value"]["player_motion"]["target_speed"] == 16,
                "independent flight effect mismatch")
        require(base["caster_mana"] == 900
                and values["fireball"]["snapshot"]["value"]["caster_mana"] == 800,
                "independent Fireball effect mismatch")
        require(not engagement_rows(baseline) and len(engagement_rows(values["enemy_policy"])) == 33,
                "independent enemy-policy effect mismatch")
        asset_a = clients["custom_baseline"].presentation_asset(1, 2, ASSET)
        asset_b = clients["asset"].presentation_asset(1, 2, ASSET)
        combined_asset = clients["combined"].presentation_asset(1, 2, ASSET)
        require(asset_a["value"]["content_address"] != asset_b["value"]["content_address"],
                "independent asset effect mismatch")
        require(values["asset"]["snapshot"]["value"] == base,
                "independent asset countermetric mismatch")
        combined = values["combined"]["snapshot"]["value"]
        require(combined["player_motion"]["target_speed"] == 16
                and combined["caster_mana"] == 800
                and len(engagement_rows(values["combined"])) == 33
                and combined_asset["value"]["content_address"]
                == asset_b["value"]["content_address"],
                "independent combined composition mismatch")
        return projection(values["combined"], combined_asset)
    finally:
        for client in clients.values():
            client.close()


def parse_marker(text: str, label: str) -> dict[str, Any]:
    match = re.search(r"MC_F10_MATRIX (\{.*\})", text)
    require(match is not None, f"{label} emitted no MC_F10_MATRIX marker")
    return json.loads(match.group(1))


def rerun_typescript(plugin: Path, workspaces: dict[str, Path]) -> dict[str, Any]:
    environment = os.environ.copy()
    environment["MC_GAME_SESSION_PLUGIN"] = str(plugin)
    environment.update({
        "MC_F10_CUSTOM_BASELINE_WORKSPACE": str(workspaces["custom_baseline"]),
        "MC_F10_FLIGHT_WORKSPACE": str(workspaces["flight"]),
        "MC_F10_FIREBALL_WORKSPACE": str(workspaces["fireball"]),
        "MC_F10_AI_WORKSPACE": str(workspaces["enemy_policy"]),
        "MC_F10_ASSET_WORKSPACE": str(workspaces["asset"]),
        "MC_F10_COMBINED_WORKSPACE": str(workspaces["combined"]),
    })
    completed = subprocess.run(
        ["node", "--test", "tests/custom-package-foundation.contract.test.ts"],
        cwd=WEB, env=environment, capture_output=True, text=True, check=False, timeout=240,
    )
    require(completed.returncode == 0 and "pass 1" in completed.stdout
            and "fail 0" in completed.stdout,
            f"independent TypeScript matrix failed: {completed.stdout}\n{completed.stderr}")
    return parse_marker(completed.stdout, "TypeScript")


def rerun_unity(plugin: Path, workspaces: dict[str, Path], root: Path) -> dict[str, Any]:
    environment = os.environ.copy()
    environment.update({
        "MC_F10_CUSTOM_BASELINE_WORKSPACE": str(workspaces["custom_baseline"]),
        "MC_F10_FLIGHT_WORKSPACE": str(workspaces["flight"]),
        "MC_F10_FIREBALL_WORKSPACE": str(workspaces["fireball"]),
        "MC_F10_AI_WORKSPACE": str(workspaces["enemy_policy"]),
        "MC_F10_ASSET_WORKSPACE": str(workspaces["asset"]),
        "MC_F10_COMBINED_WORKSPACE": str(workspaces["combined"]),
    })
    result = root / "independent-unity.xml"
    log = root / "independent-unity.log"
    completed = subprocess.run([
        sys.executable, str(HERE / "run_engine_session_editmode.py"),
        "--project", str(UNITY_PROJECT), "--plugin", str(plugin),
        "--results", str(result), "--log", str(log),
        "--test-filter", "MagicCarpet.Native.Tests.CustomPackageFoundationEditModeTests",
    ], cwd=HERE, env=environment, capture_output=True, text=True, check=False, timeout=900)
    require(completed.returncode == 0,
            f"independent Unity matrix failed: {completed.stdout}\n{completed.stderr}")
    xml = ET.parse(result).getroot()
    require(xml.attrib.get("passed") == "1" and xml.attrib.get("failed") == "0",
            "independent Unity selection/counters disagree")
    output = "\n".join((element.text or "") for element in xml.findall(".//output"))
    return parse_marker(output, "Unity")


def rerun_command(command: list[str], cwd: Path, timeout: int = 1800) -> dict[str, Any]:
    completed = subprocess.run(
        command, cwd=cwd, capture_output=True, text=True, check=False, timeout=timeout,
    )
    require(completed.returncode == 0,
            f"independent command failed: {' '.join(command)}\n{completed.stdout}{completed.stderr}")
    if command[0] == "cargo":
        selected = [int(value) for value in re.findall(
            r"(?:test result: ok\. )?(\d+) passed", completed.stdout + completed.stderr
        )]
        require(any(value > 0 for value in selected),
                f"independent command selected zero tests: {' '.join(command)}")
    return {
        "command": command,
        "stdout_sha256": hashlib.sha256(completed.stdout.encode()).hexdigest(),
        "stderr_sha256": hashlib.sha256(completed.stderr.encode()).hexdigest(),
    }


def verify_inventory(report: dict[str, Any]) -> dict[str, str]:
    inventory = report["suites"]["executable_identity"]
    observed = {
        name: sha256_file(Path(path)) for name, path in inventory["paths"].items()
    }
    require(observed == inventory["hashes"], "executable/client hash differs from integrator bundle")
    require(observed["rust_plugin"] == observed["unity_project_plugin"],
            "candidate and installed Unity plugin differ")
    whitelist = json.loads(WHITELIST.read_text(encoding="utf-8"))
    actual = {
        path.relative_to(FIXTURE).as_posix(): sha256_file(path)
        for path in sorted(FIXTURE.rglob("*")) if path.is_file()
    }
    require(actual == whitelist["allowed_files"], "independent package whitelist mismatch")
    return observed


def verify_requirements(report: dict[str, Any]) -> None:
    specification = json.loads(REQUIREMENTS.read_text(encoding="utf-8"))
    plan_ids = re.findall(
        r"^\| `([A-Z0-9]+-[0-9]{3})` \|", PLAN.read_text(encoding="utf-8"), re.M
    )
    spec_ids = [row["id"] for row in specification["requirements"]]
    report_ids = [row["id"] for row in report["requirements"]]
    require(plan_ids == spec_ids == report_ids, "atomic requirement coverage drift")
    require(all(row["status"] == "PASS"
                and row["negative_control_status"] == "EXPECTED_FAIL"
                and row["discrepancy_status"] == "none" for row in report["requirements"]),
            "requirement bundle contains a nonpassing or hidden discrepancy")
    require(report["totals"] == {
        "required": len(plan_ids), "exercised": len(plan_ids), "passing": len(plan_ids),
        "expected_failing": len(plan_ids), "uncovered": 0,
    }, "requirement totals hide an uncovered clause")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        report_path = args.report.resolve()
        output = args.output.resolve()
        require(report_path.is_file() and not report_path.is_symlink(), "unsafe report path")
        require(not output.exists() and not output.is_symlink(), "verifier output already exists")
        report = json.loads(report_path.read_text(encoding="utf-8"))
        require(report.get("status") == "PASS"
                and report.get("contract") == "carpet-flight-custom-package-foundation/v1",
                "input is not a passing F10 foundation bundle")
        verify_requirements(report)
        current_head = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=REPO, capture_output=True,
            text=True, check=True, timeout=30,
        ).stdout.strip()
        require(report["repository"]["head"] == current_head,
                "repository HEAD differs from the integrator candidate")
        hashes = verify_inventory(report)
        require(report["corpus_sha256_before_after"][0]
                == report["corpus_sha256_before_after"][1] == sha256_file(CORPUS),
                "fixed corpus changed during or after integrator run")
        workspaces = {name: Path(path) for name, path in report["workspaces"].items()}
        require(all(path.is_dir() and not path.is_symlink() for path in workspaces.values()),
                "integrator workspace is absent or unsafe")
        plugin = Path(report["suites"]["executable_identity"]["paths"]["rust_plugin"])
        native = rerun_native(plugin, workspaces)
        typescript = rerun_typescript(plugin, workspaces)
        unity = rerun_unity(plugin, workspaces, Path(report["workspace_root"]))
        expected = report["suites"]["matrix_native"]["combined_projection"]
        require(native == typescript == unity == expected,
                "independent native/TypeScript/Unity projection mismatch")

        commands = []
        for name in (
            "definition_suite", "catalog_suite", "transaction_suite",
            "session_suite", "cli_suite", "neutral_suite",
        ):
            commands.append(rerun_command(report["suites"][name]["command"], SIM))
        commands.append(rerun_command(
            [sys.executable, str(HERE / "verify_reference_corpus.py")], HERE
        ))
        commands.append(rerun_command([
            sys.executable, str(HERE / "verify_compatibility_surface.py"),
            "--plugin", str(plugin),
        ], HERE))
        require(sha256_file(CORPUS) == report["corpus_sha256_before_after"][0],
                "independent rerun changed the fixed corpus")
        result = {
            "ok": True, "status": "PASS",
            "contract": "carpet-flight-custom-package-foundation-independent-verifier/v1",
            "integrator_report": str(report_path),
            "integrator_report_sha256": sha256_file(report_path),
            "executable_hashes": hashes,
            "normalized_projection_sha256": hashlib.sha256(
                json.dumps(native, sort_keys=True, separators=(",", ":")).encode()
            ).hexdigest(),
            "rerun_commands": commands, "totals": report["totals"],
            "no_claim": report["no_claim"],
        }
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(json.dumps({
            "ok": True, "status": "PASS", "output": str(output),
            "totals": report["totals"],
        }, sort_keys=True))
        return 0
    except (
        RuntimeError, OSError, ValueError, KeyError, ET.ParseError, subprocess.TimeoutExpired,
    ) as error:
        print(json.dumps({"ok": False, "status": "FAIL", "error": str(error)}), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
