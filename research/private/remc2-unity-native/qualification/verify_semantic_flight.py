#!/usr/bin/env python3
"""Prove one published flight value reaches Rust, TypeScript, and Unity.

The baseline and variant use the same plug-in and client executables. The only
input difference is an F3 workspace whose active immutable definition changes
``flight.player.max_forward_speed`` from 80 to 16.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

from run_engine_session_editmode import run as run_unity
from verify_neutral_game_session import MC_STATUS_OK, NeutralClient


HERE = Path(__file__).resolve().parent
PRIVATE = HERE.parents[1]
WEB = PRIVATE / "remc2-sim-rs/web"
UNITY_PROJECT = PRIVATE / "remc2-unity-native"


class SemanticFlightError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SemanticFlightError(message)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def run_cli(executable: Path, arguments: list[str]) -> dict:
    completed = subprocess.run(
        [str(executable), "game", *arguments],
        check=False,
        capture_output=True,
        text=True,
    )
    require(
        completed.returncode == 0 and completed.stderr == "",
        f"sim-cli {' '.join(arguments)} failed: {completed.stderr.strip()}",
    )
    value = json.loads(completed.stdout)
    require(value.get("ok") is True, "sim-cli returned a non-success envelope")
    return value


def publish_variant(sim_cli: Path, root: Path) -> tuple[Path, dict]:
    workspace = root / "workspace"
    export = PRIVATE / "generated/level1-export-m"
    closure = PRIVATE / "generated/level1-asset-closure.json"
    cache = PRIVATE / "generated/level1-unity-asset-cache-plan29-final"
    run_cli(
        sim_cli,
        [
            "workspace-create",
            "--workspace", str(workspace),
            "--workspace-id", "f6-flight-proof",
            "--owner-export", str(export),
            "--owner-closure", str(closure),
            "--owner-cache", str(cache),
        ],
    )
    changes = root / "changes.json"
    changes.write_text(
        json.dumps(
            [
                {
                    "id": "flight.player.max_forward_speed",
                    "value": {"type": "integer", "value": 16},
                    "expected_value": {"type": "integer", "value": 80},
                }
            ],
            separators=(",", ":"),
        ),
        encoding="utf-8",
    )
    staged = run_cli(
        sim_cli,
        [
            "set",
            "--workspace", str(workspace),
            "--idempotency-key", "f6-flight-limit-16",
            "--timing", "session_reset",
            "--changes", str(changes),
            "--intent", "one-factor ordinary-flight proof",
        ],
    )
    change_set = root / "change-set.json"
    change_set.write_text(
        json.dumps(staged["data"]["change_set"], separators=(",", ":")),
        encoding="utf-8",
    )
    preview = run_cli(
        sim_cli,
        ["preview", "--workspace", str(workspace), "--change-set", str(change_set)],
    )
    validation = preview["data"]["validation"]
    require(validation["accepted"] is True, "flight change did not validate")
    require(
        preview["data"]["diff"]
        == [
            {
                "id": "flight.player.max_forward_speed",
                "before": {"type": "integer", "value": 80},
                "after": {"type": "integer", "value": 16},
            }
        ],
        "published input is not a one-factor flight diff",
    )
    applied = run_cli(
        sim_cli,
        [
            "apply",
            "--workspace", str(workspace),
            "--change-set", str(change_set),
            "--validation-receipt", validation["receipt_hash"],
        ],
    )
    receipt = applied["data"]["receipt"]
    require(receipt["resulting_definition_revision"] == 2, "unexpected definition revision")
    read_back = run_cli(
        sim_cli,
        [
            "get",
            "--workspace", str(workspace),
            "--id", "flight.player.max_forward_speed",
        ],
    )
    require(read_back["data"]["value"]["value"] == 16, "published value did not read back")
    return workspace, receipt


def drive(client: NeutralClient) -> tuple[dict, list[str]]:
    command = {"longitudinal": "accelerate"}
    for tick in range(2):
        require(client.step(1, tick, command) == MC_STATUS_OK, "native flight step failed")
    snapshot = client.snapshot()
    events = client.events(1, 2, 0)
    return snapshot, [row["event_id"] for row in events["value"]["events"]]


def verify_native(plugin: Path, workspace: Path) -> dict:
    baseline = NeutralClient(plugin)
    authored = NeutralClient(plugin, workspace=workspace)
    try:
        baseline_snapshot, baseline_events = drive(baseline)
        authored_snapshot, authored_events = drive(authored)
        baseline_value = baseline_snapshot["value"]
        authored_value = authored_snapshot["value"]
        require(
            baseline_value["player_motion"]["target_speed"] == 32
            and authored_value["player_motion"]["target_speed"] == 16,
            "published limit did not change the ordinary target-speed clamp",
        )
        require(
            baseline_value["player_motion"]["actual_speed"]
            != authored_value["player_motion"]["actual_speed"],
            "published limit did not reach ordinary velocity integration",
        )
        require(
            baseline_value["player_pose"] != authored_value["player_pose"],
            "published limit did not reach ordinary displacement",
        )
        for key in ("tick", "caster_mana", "objective_count", "action_sequence"):
            require(baseline_value[key] == authored_value[key], f"countermetric changed: {key}")
        require(baseline_events == authored_events, "unrelated semantic event order changed")

        first = json.loads(json.dumps(authored_value))
        require(authored.reset(1, 2) == MC_STATUS_OK, "authored reset failed")
        replay, replay_events = drive_generation_two(authored)
        require(replay["value"] == first, "authored reset replay is not exact")
        require(replay_events == authored_events, "authored reset event order changed")
        return {
            "baseline_definition": baseline_snapshot["identity"]["definition_hash"],
            "variant_definition": authored_snapshot["identity"]["definition_hash"],
            "baseline_target_speed": 32,
            "variant_target_speed": 16,
            "tick": authored_value["tick"],
            "caster_mana": authored_value["caster_mana"],
            "objective_count": authored_value["objective_count"],
            "action_sequence": authored_value["action_sequence"],
            "semantic_event_count": len(authored_events),
        }
    finally:
        baseline.close()
        authored.close()


def drive_generation_two(client: NeutralClient) -> tuple[dict, list[str]]:
    command = {"longitudinal": "accelerate"}
    for tick in range(2):
        require(client.step(2, tick, command) == MC_STATUS_OK, "native replay step failed")
    snapshot = client.snapshot()
    events = client.events(2, 2, 0)
    return snapshot, [row["event_id"] for row in events["value"]["events"]]


def run_typescript(plugin: Path, workspace: Path) -> dict:
    environment = os.environ.copy()
    environment["MC_GAME_SESSION_PLUGIN"] = str(plugin)
    environment["MC_SEMANTIC_VARIANT_WORKSPACE"] = str(workspace)
    completed = subprocess.run(
        ["node", "--test", "tests/semantic-flight.contract.test.ts"],
        cwd=WEB,
        env=environment,
        check=False,
        capture_output=True,
        text=True,
        timeout=180,
    )
    require(completed.returncode == 0, f"TypeScript flight proof failed: {completed.stderr}")
    require(
        "pass 1" in completed.stdout and "fail 0" in completed.stdout,
        f"TypeScript flight proof counters disagree: {completed.stdout}",
    )
    return {"status": "PASS", "tests": 1}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--plugin", required=True, type=Path)
    parser.add_argument("--sim-cli", required=True, type=Path)
    parser.add_argument("--unity-project", type=Path, default=UNITY_PROJECT)
    args = parser.parse_args()
    plugin = args.plugin.resolve()
    sim_cli = args.sim_cli.resolve()
    try:
        require(plugin.is_file() and not plugin.is_symlink(), f"missing candidate plugin: {plugin}")
        require(sim_cli.is_file() and not sim_cli.is_symlink(), f"missing sim-cli: {sim_cli}")
        with tempfile.TemporaryDirectory(prefix="magiccarpet-f6-") as temporary:
            root = Path(temporary)
            workspace, receipt = publish_variant(sim_cli, root)
            native = verify_native(plugin, workspace)
            typescript = run_typescript(plugin, workspace)
            unity = run_unity(
                args.unity_project.resolve(),
                plugin,
                root / "unity-results.xml",
                root / "unity.log",
                Path("/home/oliver/Unity/Hub/Editor/6000.3.20f1/Editor/Unity"),
                "MagicCarpet.Native.Tests.SemanticFlightEditModeTests",
                workspace,
            )
            report = {
                "ok": True,
                "status": "PASS",
                "contract": "carpet-flight-semantic-flight-proof/v1",
                "plugin_sha256": sha256_file(plugin),
                "sim_cli_sha256": sha256_file(sim_cli),
                "package_diff": ["flight.player.max_forward_speed:80->16"],
                "receipt_hash": receipt["receipt_hash"],
                "native": native,
                "typescript": typescript,
                "unity": unity,
            }
        print(json.dumps(report, sort_keys=True))
        return 0
    except (
        SemanticFlightError,
        OSError,
        ValueError,
        KeyError,
        subprocess.TimeoutExpired,
    ) as error:
        print(json.dumps({"ok": False, "status": "FAIL", "error": str(error)}), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
