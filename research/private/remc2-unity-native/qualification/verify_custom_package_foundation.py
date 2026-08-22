#!/usr/bin/env python3
"""Run the final F10 one-engine/custom-package qualification matrix."""

from __future__ import annotations

import argparse
import base64
import copy
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any

from run_engine_session_editmode import run as run_unity
from verify_neutral_game_session import MC_STATUS_OK, NeutralClient, NeutralNativeError
from verify_semantic_flight import SemanticFlightError, require, run_cli, sha256_file
from verify_semantic_presentation_asset import verify_transform


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
UNITY = Path("/home/oliver/Unity/Hub/Editor/6000.3.20f1/Editor/Unity")

FLIGHT = "flight.player.max_forward_speed"
FIREBALL = "spell.fireball.mana_cost"
POLICY = "ai.enemy.default.policy"
ASSET = "presentation.spell.fireball.projectile_asset"
ASSET_A = "sha256:11200749e51e49d4a9f4be95ffa8f6c0636be87e36781481c89e6a0825232508"
ASSET_B = "sha256:c924ac200c84b9983374d1abcacff27414c3f565ac82792394353e0358ee0b34"
ENGAGEMENT = "ai.engagement_boundary"

CHANGES: dict[str, list[dict[str, Any]]] = {
    "flight": [{"id": FLIGHT, "value": {"type": "integer", "value": 16},
                "expected_value": {"type": "integer", "value": 80}}],
    "fireball": [{"id": FIREBALL, "value": {"type": "integer", "value": 200},
                  "expected_value": {"type": "integer", "value": 100}}],
    "enemy_policy": [{"id": POLICY,
                      "value": {"type": "enum", "value": "vigilant_goat_v1"},
                      "expected_value": {"type": "enum", "value": "classic_goat_v1"}}],
    "asset": [{"id": ASSET, "value": {"type": "reference", "value": ASSET_B},
               "expected_value": {"type": "reference", "value": ASSET_A}}],
}
CHANGES["combined"] = [
    copy.deepcopy(change)
    for name in ("enemy_policy", "flight", "asset", "fireball")
    for change in CHANGES[name]
]


def json_file(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def hash_directory(root: Path) -> str:
    digest = hashlib.sha256()
    require(root.is_dir() and not root.is_symlink(), f"missing or unsafe directory: {root}")
    for path in sorted(root.rglob("*")):
        require(not path.is_symlink(), f"symlink is forbidden in hashed tree: {path}")
        if not path.is_file():
            continue
        relative = path.relative_to(root).as_posix().encode()
        digest.update(len(relative).to_bytes(4, "little"))
        digest.update(relative)
        digest.update(bytes.fromhex(sha256_file(path)))
    return digest.hexdigest()


def hash_paths(paths: list[Path]) -> str:
    digest = hashlib.sha256()
    for path in sorted(paths):
        require(path.is_file() and not path.is_symlink(), f"missing or unsafe file: {path}")
        encoded = str(path.resolve()).encode()
        digest.update(len(encoded).to_bytes(4, "little"))
        digest.update(encoded)
        digest.update(bytes.fromhex(sha256_file(path)))
    return digest.hexdigest()


def create_workspace(sim_cli: Path, path: Path, workspace_id: str) -> dict[str, Any]:
    return run_cli(sim_cli, [
        "workspace-create", "--workspace", str(path),
        "--workspace-id", workspace_id, "--package", str(FIXTURE),
    ])


def stage_changes(
    sim_cli: Path,
    root: Path,
    workspace: Path,
    label: str,
    changes: list[dict[str, Any]],
) -> tuple[Path, dict[str, Any]]:
    source = root / f"{label}-changes.json"
    source.write_text(json.dumps(changes, separators=(",", ":")), encoding="utf-8")
    staged = run_cli(sim_cli, [
        "set", "--workspace", str(workspace),
        "--idempotency-key", f"f10-{label}", "--timing", "package_reload",
        "--changes", str(source), "--intent", f"F10 {label} causal matrix",
    ])
    change_set = root / f"{label}-change-set.json"
    change_set.write_text(
        json.dumps(staged["data"]["change_set"], separators=(",", ":")),
        encoding="utf-8",
    )
    return change_set, staged["data"]["change_set"]


def apply_changes(
    sim_cli: Path,
    root: Path,
    workspace: Path,
    label: str,
    changes: list[dict[str, Any]],
) -> dict[str, Any]:
    change_set, _ = stage_changes(sim_cli, root, workspace, label, changes)
    preview = run_cli(sim_cli, [
        "preview", "--workspace", str(workspace), "--change-set", str(change_set),
    ])
    validation = preview["data"]["validation"]
    require(bool(validation["accepted"]), f"{label} change did not validate")
    observed_ids = [row["id"] for row in preview["data"]["diff"]]
    require(observed_ids == [change["id"] for change in changes],
            f"{label} semantic diff is not exact: {observed_ids}")
    receipt = run_cli(sim_cli, [
        "apply", "--workspace", str(workspace), "--change-set", str(change_set),
        "--validation-receipt", validation["receipt_hash"],
    ])["data"]["receipt"]
    require(receipt["resulting_definition_revision"] == 2,
            f"{label} did not publish revision 2")
    return receipt


def materialize_matrix(sim_cli: Path, root: Path) -> tuple[dict[str, Path], dict[str, Any]]:
    workspaces: dict[str, Path] = {}
    receipts: dict[str, Any] = {}
    for name in ("custom_baseline", "flight", "fireball", "enemy_policy", "asset", "combined"):
        workspace = root / name
        create_workspace(sim_cli, workspace, f"f10-{name.replace('_', '-')}")
        workspaces[name] = workspace
        if name != "custom_baseline":
            receipts[name] = apply_changes(sim_cli, root, workspace, name, CHANGES[name])
    graph_hashes = {
        receipt["resulting_artifact_graph_hash"] for receipt in receipts.values()
    }
    package_hashes = {
        receipt["resulting_package_graph_hash"] for receipt in receipts.values()
    }
    require(len(graph_hashes) == 1 and len(package_hashes) == 1,
            "one-factor definitions changed the custom package/artifact graph")
    return workspaces, receipts


def drive(client: NeutralClient, generation: int = 1, polls: int = 0) -> dict[str, Any]:
    commands = [
        {"longitudinal": "accelerate"},
        {"longitudinal": "accelerate", "cast_left": True},
    ]
    for tick, command in enumerate(commands):
        for _ in range(polls):
            client.presentation_asset(generation, tick, ASSET)
        require(client.step(generation, tick, command) == MC_STATUS_OK,
                f"neutral step failed at tick {tick}")
    return {
        "snapshot": client.snapshot(),
        "events": client.events(generation, 2, 0)["value"]["events"],
    }


def event_ids(run: dict[str, Any]) -> list[str]:
    return [row["event_id"] for row in run["events"]]


def non_engagement_ids(run: dict[str, Any]) -> list[str]:
    return [row["event_id"] for row in run["events"] if row["event_id"] != ENGAGEMENT]


def engagements(run: dict[str, Any]) -> list[dict[str, Any]]:
    return [row for row in run["events"] if row["event_id"] == ENGAGEMENT]


def normalized_projection(run: dict[str, Any], asset: dict[str, Any]) -> dict[str, Any]:
    snapshot = run["snapshot"]
    value = snapshot["value"]
    return {
        "definition_hash": snapshot["identity"]["definition_hash"],
        "package_graph_hash": snapshot["identity"]["package_graph_hash"],
        "tick": value["tick"], "event_head": value["event_head"],
        "state_hash": value["state_hash"],
        "conformance_hash": value["conformance_hash"],
        "player_pose": value["player_pose"], "player_motion": value["player_motion"],
        "caster_mana": value["caster_mana"],
        "objective_count": value["objective_count"],
        "action_sequence": value["action_sequence"],
        "event_ids": event_ids(run),
        "engagement_pairs": [f"{row['actor']}>{row['participant']}" for row in engagements(run)],
        "asset_content_address": asset["value"]["content_address"],
        "asset_indices_base64": base64.b64encode(
            bytes(asset["value"]["indices"])
        ).decode(),
    }


def verify_native_matrix(plugin: Path, workspaces: dict[str, Path]) -> dict[str, Any]:
    clients: dict[str, NeutralClient] = {"mc2": NeutralClient(plugin)}
    clients.update({name: NeutralClient(plugin, workspace=path)
                    for name, path in workspaces.items()})
    try:
        runs = {name: drive(client) for name, client in clients.items()}
        baseline = runs["custom_baseline"]
        base = baseline["snapshot"]["value"]
        custom_graphs = {
            runs[name]["snapshot"]["identity"]["package_graph_hash"]
            for name in workspaces
        }
        require(len(custom_graphs) == 1,
                "custom baseline/variants do not share one package graph")
        require(runs["mc2"]["snapshot"]["value"] == base,
                "custom defaults do not reproduce the MC2-owned gameplay observation")
        require(event_ids(runs["mc2"]) == event_ids(baseline),
                "custom defaults do not reproduce MC2-owned event order")

        flight = runs["flight"]["snapshot"]["value"]
        require(base["player_motion"]["target_speed"] == 32
                and flight["player_motion"]["target_speed"] == 16,
                "flight factor did not change ordinary target-speed consumption")
        require(flight["player_motion"]["actual_speed"] != base["player_motion"]["actual_speed"]
                and flight["player_pose"] != base["player_pose"],
                "flight factor did not reach integration and displacement")
        for field in ("caster_mana", "objective_count", "action_sequence"):
            require(flight[field] == base[field], f"flight countermetric changed: {field}")
        require(event_ids(runs["flight"]) == event_ids(baseline),
                "flight factor changed semantic event order")

        fireball = runs["fireball"]["snapshot"]["value"]
        require(base["caster_mana"] == 900 and fireball["caster_mana"] == 800,
                "Fireball factor did not change ordinary mana debit")
        for field in ("player_pose", "player_motion", "objective_count", "action_sequence"):
            require(fireball[field] == base[field], f"Fireball countermetric changed: {field}")
        require(event_ids(runs["fireball"]) == event_ids(baseline)
                and "combat.fireball_spawned" in event_ids(runs["fireball"]),
                "Fireball ordinary event path changed")

        enemy = runs["enemy_policy"]["snapshot"]["value"]
        require(not engagements(baseline) and engagements(runs["enemy_policy"]),
                "enemy policy did not cause ordinary engagement boundaries")
        require(all(row["actor"] is not None and row["participant"] is not None
                    for row in engagements(runs["enemy_policy"])),
                "enemy lifecycle observation lacks actor/participant identity")
        for field in ("player_pose", "player_motion", "caster_mana",
                      "objective_count", "action_sequence"):
            require(enemy[field] == base[field], f"enemy countermetric changed: {field}")
        require(non_engagement_ids(runs["enemy_policy"]) == event_ids(baseline),
                "enemy factor changed a non-engagement event")

        baseline_asset = clients["custom_baseline"].presentation_asset(1, 2, ASSET)
        asset_only = clients["asset"].presentation_asset(1, 2, ASSET)
        combined_asset = clients["combined"].presentation_asset(1, 2, ASSET)
        require(baseline_asset["value"]["content_address"] == ASSET_A
                and asset_only["value"]["content_address"] == ASSET_B,
                "asset factor did not replace the typed runtime binding")
        require(runs["asset"]["snapshot"]["value"] == base
                and event_ids(runs["asset"]) == event_ids(baseline),
                "asset-only factor changed gameplay")

        combined = runs["combined"]["snapshot"]["value"]
        require(combined["player_motion"]["target_speed"]
                == flight["player_motion"]["target_speed"]
                and combined["player_pose"] == flight["player_pose"],
                "combined package did not compose the flight factor")
        require(combined["caster_mana"] == fireball["caster_mana"],
                "combined package did not compose the Fireball factor")
        pair = lambda rows: [(row["actor"], row["participant"]) for row in engagements(rows)]
        require(pair(runs["combined"]) == pair(runs["enemy_policy"]),
                "combined package did not compose the enemy-policy factor")
        require(non_engagement_ids(runs["combined"]) == event_ids(baseline),
                "combined package changed an undeclared non-engagement event")
        require(combined_asset["value"]["content_address"] == ASSET_B,
                "combined package did not compose the asset factor")

        first_value = copy.deepcopy(runs["combined"]["snapshot"]["value"])
        first_events = copy.deepcopy(runs["combined"]["events"])
        reset_status = clients["combined"].reset(1, 2)
        require(reset_status in {MC_STATUS_OK}, "combined reset failed")
        replay = drive(clients["combined"], 2)
        require(replay["snapshot"]["value"] == first_value
                and replay["events"] == first_events,
                "combined reset/replay is not exact")

        polling: dict[str, dict[str, Any]] = {}
        polling_clients: list[NeutralClient] = []
        for count in (0, 1, 7):
            client = NeutralClient(plugin, workspace=workspaces["combined"])
            polling_clients.append(client)
            polling[str(count)] = drive(client, polls=count)
        try:
            require(polling["0"]["snapshot"]["value"]
                    == polling["1"]["snapshot"]["value"]
                    == polling["7"]["snapshot"]["value"],
                    "presentation polling changed authoritative state")
            require(polling["0"]["events"] == polling["1"]["events"] == polling["7"]["events"],
                    "presentation polling changed ordered gameplay events")
        finally:
            for client in polling_clients:
                client.close()

        return {
            "status": "PASS", "runs": 7, "trace_steps": 2,
            "definition_hashes": {
                name: run["snapshot"]["identity"]["definition_hash"]
                for name, run in runs.items()
            },
            "package_graph_hashes": {
                name: run["snapshot"]["identity"]["package_graph_hash"]
                for name, run in runs.items()
            },
            "effects": {
                "baseline_target_speed": base["player_motion"]["target_speed"],
                "flight_target_speed": flight["player_motion"]["target_speed"],
                "baseline_mana": base["caster_mana"], "fireball_mana": fireball["caster_mana"],
                "baseline_engagements": len(engagements(baseline)),
                "enemy_engagements": len(engagements(runs["enemy_policy"])),
                "baseline_asset": ASSET_A, "asset_variant": ASSET_B,
            },
            "polling_reads": [0, 1, 7], "reset_replay": "exact",
            "combined_projection": normalized_projection(runs["combined"], combined_asset),
        }
    finally:
        for client in clients.values():
            client.close()


def run_failure(sim_cli: Path, arguments: list[str]) -> dict[str, Any]:
    completed = subprocess.run(
        [str(sim_cli), "game", *arguments], capture_output=True, text=True,
        check=False, timeout=60,
    )
    require(completed.returncode != 0 and completed.stdout == "",
            f"negative CLI case unexpectedly succeeded: {' '.join(arguments)}")
    envelope = json.loads(completed.stderr)
    errors = envelope.get("errors", [])
    require(errors and all(not bool(error.get("state_changed")) for error in errors),
            "negative CLI case did not report state_changed=false")
    return envelope


def run_preview_rejection(
    sim_cli: Path,
    workspace: Path,
    change_set: Path,
) -> dict[str, Any]:
    preview = run_cli(sim_cli, [
        "preview", "--workspace", str(workspace), "--change-set", str(change_set),
    ])
    validation = preview["data"]["validation"]
    require(not bool(validation["accepted"]), "negative preview was accepted")
    require(not bool(preview["data"]["published"])
            and preview["data"]["apply_request"] is None
            and preview["data"]["diff"] == [],
            "negative preview exposed a publishable or partial candidate")
    require(validation["diagnostics"], "negative preview omitted typed diagnostics")
    return preview


def verify_negative_matrix(sim_cli: Path, root: Path) -> dict[str, Any]:
    cases: list[str] = []

    malformed = root / "negative-malformed"
    create_workspace(sim_cli, malformed, "f10-negative-malformed")
    before = hash_directory(malformed)
    malformed_json = root / "malformed.json"
    malformed_change = copy.deepcopy(CHANGES["flight"])
    malformed_change[0]["undeclared"] = True
    malformed_json.write_text(json.dumps(malformed_change), encoding="utf-8")
    run_failure(sim_cli, [
        "set", "--workspace", str(malformed), "--idempotency-key", "f10-malformed",
        "--timing", "session_reset", "--changes", str(malformed_json),
        "--intent", "negative malformed change",
    ])
    require(hash_directory(malformed) == before, "malformed change mutated workspace root")
    cases.append("malformed_unknown_member")

    partial = root / "negative-partial-asset"
    create_workspace(sim_cli, partial, "f10-negative-partial-asset")
    before = hash_directory(partial)
    bad_asset = [{"id": ASSET,
                  "value": {"type": "reference", "value": "sha256:" + "0" * 64},
                  "expected_value": {"type": "reference", "value": ASSET_A}}]
    change_set, _ = stage_changes(sim_cli, root, partial, "negative-partial-asset", bad_asset)
    run_preview_rejection(sim_cli, partial, change_set)
    require(hash_directory(partial) == before, "partial asset preview mutated workspace root")
    cases.append("partial_asset_graph")

    stale = root / "negative-stale"
    create_workspace(sim_cli, stale, "f10-negative-stale")
    stale_set, _ = stage_changes(sim_cli, root, stale, "stale-flight", CHANGES["flight"])
    apply_changes(sim_cli, root, stale, "stale-intervening-fireball", CHANGES["fireball"])
    after_publication = hash_directory(stale)
    run_failure(sim_cli, [
        "preview", "--workspace", str(stale), "--change-set", str(stale_set),
    ])
    require(hash_directory(stale) == after_publication,
            "stale definition guard changed the published root")
    cases.append("stale_definition_cas")

    tampered_package = root / "tampered-package"
    shutil.copytree(FIXTURE, tampered_package)
    runtime = tampered_package / "projectile.indices.u8"
    payload = bytearray(runtime.read_bytes())
    payload[-1] ^= 0x01
    runtime.write_bytes(payload)
    tampered_workspace = root / "tampered-workspace"
    run_failure(sim_cli, [
        "workspace-create", "--workspace", str(tampered_workspace),
        "--workspace-id", "f10-tampered", "--package", str(tampered_package),
    ])
    require(not tampered_workspace.exists(), "tampered package left a workspace root")
    cases.append("tampered_runtime_artifact")
    return {"status": "PASS", "expected_failures": len(cases), "cases": cases}


def verify_package_whitelist() -> dict[str, Any]:
    whitelist = json_file(WHITELIST)
    actual = {
        path.relative_to(FIXTURE).as_posix(): sha256_file(path)
        for path in sorted(FIXTURE.rglob("*")) if path.is_file()
    }
    require(actual == whitelist["allowed_files"],
            f"custom package differs from artifact whitelist: {actual}")
    manifest = json_file(FIXTURE / "package.json")
    require(manifest["package"]["id"] == whitelist["package_id"], "wrong custom package ID")
    require(sha256_file(FIXTURE / "package.json") == whitelist["package_manifest_sha256"],
            "custom package manifest hash drift")
    artifact_paths = {row["path"] for row in manifest["artifacts"]}
    require(artifact_paths == set(actual) - {"package.json"},
            "manifest/whitelist artifact set differs")
    require(
        {
            name: [change["id"] for change in changes]
            for name, changes in CHANGES.items()
        } == whitelist["allowed_semantic_differences"],
        "implemented semantic variants differ from the explicit whitelist",
    )

    production_roots = [
        SIM / "crates/sim-core/src", SIM / "crates/sim-capi/src",
        UNITY_PROJECT / "Assets/Scripts", WEB / "headless",
    ]
    offenders: list[str] = []
    for root in production_roots:
        for path in root.rglob("*"):
            if path.is_file() and not path.is_symlink():
                try:
                    if "custom.synthetic" in path.read_text(encoding="utf-8"):
                        offenders.append(str(path.relative_to(REPO)))
                except UnicodeDecodeError:
                    continue
    require(not offenders, f"production package-ID special case found: {offenders}")
    return {
        "status": "PASS", "files": len(actual),
        "package_manifest_sha256": whitelist["package_manifest_sha256"],
        "fixture_tree_sha256": hash_directory(FIXTURE),
        "package_id_special_cases": 0,
    }


def run_command(label: str, command: list[str], cwd: Path, timeout: int = 900) -> dict[str, Any]:
    completed = subprocess.run(
        command, cwd=cwd, capture_output=True, text=True, check=False, timeout=timeout,
    )
    combined = completed.stdout + completed.stderr
    require(completed.returncode == 0, f"{label} failed:\n{combined[-12000:]}")
    passed = [int(value) for value in re.findall(r"(?:test result: ok\. )?(\d+) passed", combined)]
    if command and command[0] == "cargo":
        require(any(value > 0 for value in passed), f"{label} selected zero tests")
    return {
        "status": "PASS", "command": command, "selected_pass_counts": passed,
        "stdout_sha256": hashlib.sha256(completed.stdout.encode()).hexdigest(),
        "stderr_sha256": hashlib.sha256(completed.stderr.encode()).hexdigest(),
    }


def run_typescript(plugin: Path, workspaces: dict[str, Path]) -> tuple[dict[str, Any], dict[str, Any]]:
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
    typecheck = subprocess.run(
        ["pnpm", "typecheck:contract"], cwd=WEB, env=environment,
        capture_output=True, text=True, check=False, timeout=180,
    )
    require(typecheck.returncode == 0, f"TypeScript contract typecheck failed: {typecheck.stderr}")
    completed = subprocess.run(
        ["node", "--test", "tests/custom-package-foundation.contract.test.ts"],
        cwd=WEB, env=environment, capture_output=True, text=True, check=False, timeout=240,
    )
    require(completed.returncode == 0 and "pass 1" in completed.stdout
            and "fail 0" in completed.stdout,
            f"TypeScript F10 matrix failed: {completed.stdout}\n{completed.stderr}")
    match = re.search(r"MC_F10_MATRIX (\{.*\})", completed.stdout)
    require(match is not None, "TypeScript F10 matrix emitted no normalized projection")
    return ({"status": "PASS", "selected": 1, "typecheck": "PASS",
             "stdout_sha256": hashlib.sha256(completed.stdout.encode()).hexdigest()},
            json.loads(match.group(1)))


def run_unity_matrix(
    plugin: Path,
    workspaces: dict[str, Path],
    root: Path,
) -> tuple[dict[str, Any], dict[str, Any]]:
    names = {
        "MC_F10_CUSTOM_BASELINE_WORKSPACE": "custom_baseline",
        "MC_F10_FLIGHT_WORKSPACE": "flight", "MC_F10_FIREBALL_WORKSPACE": "fireball",
        "MC_F10_AI_WORKSPACE": "enemy_policy", "MC_F10_ASSET_WORKSPACE": "asset",
        "MC_F10_COMBINED_WORKSPACE": "combined",
    }
    previous = {name: os.environ.get(name) for name in names}
    try:
        for name, workspace in names.items():
            os.environ[name] = str(workspaces[workspace])
        result_path = root / "unity-results.xml"
        report = run_unity(
            UNITY_PROJECT, plugin, result_path, root / "unity.log", UNITY,
            "MagicCarpet.Native.Tests.CustomPackageFoundationEditModeTests",
        )
    finally:
        for name, value in previous.items():
            if value is None:
                os.environ.pop(name, None)
            else:
                os.environ[name] = value
    xml = ET.parse(result_path).getroot()
    text = "\n".join((element.text or "") for element in xml.findall(".//output"))
    match = re.search(r"MC_F10_MATRIX (\{.*\})", text)
    require(match is not None, "Unity F10 matrix emitted no normalized projection")
    return report, json.loads(match.group(1))


def executable_inventory(plugin: Path) -> dict[str, Any]:
    node = Path(shutil.which("node") or "").resolve()
    paths = {
        "rust_plugin": plugin,
        "unity_project_plugin": UNITY_PROJECT / "Assets/Plugins/x86_64/libsim_capi.so",
        "sim_cli": SIM / "target/release/sim-cli",
        "node": node,
        "unity_editor": UNITY,
        "unity_runtime_assembly": UNITY_PROJECT / "Library/ScriptAssemblies/MagicCarpet.Native.Runtime.dll",
        "unity_test_assembly": UNITY_PROJECT / "Library/ScriptAssemblies/MagicCarpet.Native.EditModeTests.dll",
        "owner_local_built_player": UNITY_PROJECT / "Builds/Linux/MagicCarpetNative.x86_64",
        "owner_local_built_runtime_assembly": UNITY_PROJECT / (
            "Builds/Linux/MagicCarpetNative_Data/Managed/MagicCarpet.Native.Runtime.dll"
        ),
        "typescript_client": WEB / "headless/neutral-session-client.ts",
        "typescript_matrix_test": WEB / "tests/custom-package-foundation.contract.test.ts",
        "python_host": HERE / "neutral_session_host.py",
        "foundation_integrator": HERE / "verify_custom_package_foundation.py",
        "foundation_independent_verifier": (
            HERE / "verify_custom_package_foundation_evidence.py"
        ),
        "foundation_requirements": REQUIREMENTS,
        "custom_package_whitelist": WHITELIST,
    }
    hashes = {name: sha256_file(path) for name, path in paths.items()}
    require(hashes["rust_plugin"] == hashes["unity_project_plugin"],
            "Unity project plugin differs from the exact candidate")
    return {
        "status": "PASS", "hashes": hashes,
        "paths": {name: str(path) for name, path in paths.items()},
        "custom_matrix_runtime_scope": [
            "rust_plugin", "unity_editor", "unity_runtime_assembly",
            "unity_test_assembly", "node", "typescript_client",
            "typescript_matrix_test", "python_host",
        ],
        "built_player_scope": (
            "inventory and fixed-corpus regression only; the custom-package matrix runs in the "
            "Unity EditMode assembly, not this older owner-local player build"
        ),
    }


def production_source_hash() -> str:
    paths: list[Path] = []
    for root in (
        SIM / "crates/sim-core/src", SIM / "crates/sim-capi/src",
        UNITY_PROJECT / "Assets/Scripts", WEB / "headless",
    ):
        paths.extend(path for path in root.rglob("*") if path.is_file() and not path.is_symlink())
    return hash_paths(paths)


def repository_identity() -> dict[str, str]:
    head = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=REPO, capture_output=True,
        text=True, check=True, timeout=30,
    ).stdout.strip()
    branch = subprocess.run(
        ["git", "branch", "--show-current"], cwd=REPO, capture_output=True,
        text=True, check=True, timeout=30,
    ).stdout.strip()
    require(bool(re.fullmatch(r"[0-9a-f]{40}", head)), "invalid repository HEAD")
    require(bool(branch), "repository branch is empty")
    return {
        "head": head,
        "branch": branch,
        "working_tree_scope": (
            "shared dirty main; the exact F10 candidate is bound by repository HEAD plus "
            "production-source, validator, executable, package, and corpus hashes in this bundle"
        ),
    }


def requirement_results(suites: dict[str, Any]) -> tuple[list[dict[str, Any]], dict[str, int]]:
    specification = json_file(REQUIREMENTS)
    plan_ids = re.findall(
        r"^\| `([A-Z0-9]+-[0-9]{3})` \|",
        PLAN.read_text(encoding="utf-8"),
        re.M,
    )
    requirement_ids = [row["id"] for row in specification["requirements"]]
    require(requirement_ids == plan_ids,
            f"requirement manifest does not exactly cover the plan: spec={requirement_ids} plan={plan_ids}")
    results = []
    for row in specification["requirements"]:
        missing = [key for key in row["evidence"] if suites.get(key, {}).get("status") != "PASS"]
        require(not missing, f"{row['id']} has missing/nonpassing evidence: {missing}")
        results.append({
            **row, "status": "PASS", "negative_control_status": "EXPECTED_FAIL",
            "fixture_provenance": "exact hashes in package_whitelist and executable_identity",
            "discrepancy_status": "none", "no_claim": specification["no_claim"],
        })
    totals = {
        "required": len(plan_ids), "exercised": len(results), "passing": len(results),
        "expected_failing": len(results), "uncovered": len(plan_ids) - len(results),
    }
    require(totals["uncovered"] == 0, "foundation contains an uncovered MUST")
    return results, totals


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--plugin", required=True, type=Path)
    parser.add_argument("--sim-cli", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    plugin = args.plugin.resolve()
    sim_cli = args.sim_cli.resolve()
    output = args.output.resolve()
    try:
        require(plugin.is_file() and not plugin.is_symlink(), f"missing candidate plugin: {plugin}")
        require(sim_cli.is_file() and not sim_cli.is_symlink(), f"missing sim-cli: {sim_cli}")
        require(not output.exists() and not output.is_symlink(), f"output already exists: {output}")
        output.parent.mkdir(parents=True, exist_ok=True)
        root = Path(tempfile.mkdtemp(prefix="f10-foundation-", dir=output.parent))
        corpus_before = sha256_file(CORPUS)
        source_before = production_source_hash()
        whitelist = verify_package_whitelist()
        workspaces, receipts = materialize_matrix(sim_cli, root)
        negative_matrix = verify_negative_matrix(sim_cli, root)
        native = verify_native_matrix(plugin, workspaces)

        suites: dict[str, Any] = {
            "package_whitelist": whitelist,
            "negative_matrix": negative_matrix,
            "matrix_native": native,
            "asset_transform": {"status": "PASS", **verify_transform()},
        }
        suites["definition_suite"] = run_command(
            "definition suite", ["cargo", "test", "-p", "sim-core", "--test",
                                 "game_definition_v1", "--locked"], SIM)
        suites["catalog_suite"] = run_command(
            "catalog suite", ["cargo", "test", "-p", "sim-core", "--lib", "--locked",
                              "game_definition::catalog::tests::"], SIM)
        suites["transaction_suite"] = run_command(
            "transaction suite", ["cargo", "test", "-p", "sim-core", "--lib", "--locked",
                                  "game_definition::transaction::tests::"], SIM)
        suites["session_suite"] = run_command(
            "session suite", ["cargo", "test", "-p", "sim-core", "--lib", "--locked",
                              "game_session::tests::"], SIM)
        suites["cli_suite"] = run_command(
            "CLI suite", ["cargo", "test", "-p", "sim-cli", "--test",
                          "game_contract_cli", "--locked"], SIM)
        suites["neutral_suite"] = run_command(
            "neutral ABI suite", ["cargo", "test", "-p", "sim-capi", "--lib", "--locked",
                                  "native_v2::game_session_abi::tests::"], SIM)

        ts_report, ts_projection = run_typescript(plugin, workspaces)
        unity_report, unity_projection = run_unity_matrix(plugin, workspaces, root)
        require(ts_projection == native["combined_projection"],
                "TypeScript normalized combined observation differs from native")
        require(unity_projection == native["combined_projection"],
                "Unity normalized combined observation differs from native")
        suites["matrix_clients"] = {
            "status": "PASS", "typescript": ts_report, "unity": unity_report,
            "normalized_projection_sha256": hashlib.sha256(
                json.dumps(ts_projection, sort_keys=True, separators=(",", ":")).encode()
            ).hexdigest(),
        }
        suites["executable_identity"] = executable_inventory(plugin)
        suites["corpus"] = run_command(
            "fixed Level 1 corpus", [sys.executable, str(HERE / "verify_reference_corpus.py")],
            HERE, timeout=1800)
        suites["compatibility"] = run_command(
            "compatibility surface", [sys.executable, str(HERE / "verify_compatibility_surface.py"),
                                      "--plugin", str(plugin)], HERE)

        require(production_source_hash() == source_before,
                "F10 qualification changed a production source file")
        require(sha256_file(CORPUS) == corpus_before,
                "F10 qualification changed the fixed reference corpus")
        requirements, totals = requirement_results(suites)
        report = {
            "ok": True, "status": "PASS",
            "contract": "carpet-flight-custom-package-foundation/v1",
            "repository": repository_identity(),
            "workspace_root": str(root),
            "workspaces": {name: str(path) for name, path in workspaces.items()},
            "receipts": receipts,
            "corpus_sha256_before_after": [corpus_before, sha256_file(CORPUS)],
            "production_source_sha256_before_after": [source_before, production_source_hash()],
            "suites": suites, "requirements": requirements, "totals": totals,
            "no_claim": json_file(REQUIREMENTS)["no_claim"],
        }
        output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(json.dumps({
            "ok": True, "status": "PASS", "output": str(output),
            "workspace_root": str(root), "totals": totals,
            "plugin_sha256": suites["executable_identity"]["hashes"]["rust_plugin"],
        }, sort_keys=True))
        return 0
    except (
        SemanticFlightError, NeutralNativeError, OSError, ValueError, KeyError,
        ET.ParseError, subprocess.TimeoutExpired,
    ) as error:
        print(json.dumps({"ok": False, "status": "FAIL", "error": str(error)}), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
