#!/usr/bin/env python3
"""Prove one published Goat policy reaches Rust, TypeScript, and Unity."""

from __future__ import annotations

import argparse
import copy
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

from run_engine_session_editmode import run as run_unity
from verify_neutral_game_session import MC_STATUS_OK, NeutralClient
from verify_semantic_flight import SemanticFlightError, require, run_cli, sha256_file


HERE = Path(__file__).resolve().parent
PRIVATE = HERE.parents[1]
WEB = PRIVATE / "remc2-sim-rs/web"
UNITY_PROJECT = PRIVATE / "remc2-unity-native"
ENGAGEMENT = "ai.engagement_boundary"


def publish_variant(sim_cli: Path, root: Path) -> tuple[Path, dict]:
    workspace = root / "workspace"
    run_cli(
        sim_cli,
        [
            "workspace-create",
            "--workspace", str(workspace),
            "--workspace-id", "f8-goat-policy-proof",
            "--owner-export", str(PRIVATE / "generated/level1-export-m"),
            "--owner-closure", str(PRIVATE / "generated/level1-asset-closure.json"),
            "--owner-cache", str(PRIVATE / "generated/level1-unity-asset-cache-plan29-final"),
        ],
    )
    changes = root / "changes.json"
    changes.write_text(
        json.dumps(
            [{
                "id": "ai.enemy.default.policy",
                "value": {"type": "enum", "value": "vigilant_goat_v1"},
                "expected_value": {"type": "enum", "value": "classic_goat_v1"},
            }],
            separators=(",", ":"),
        ),
        encoding="utf-8",
    )
    staged = run_cli(
        sim_cli,
        [
            "set",
            "--workspace", str(workspace),
            "--idempotency-key", "f8-vigilant-goat-policy",
            "--timing", "session_reset",
            "--changes", str(changes),
            "--intent", "one-factor ordinary-Goat lifecycle proof",
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
    require(validation["accepted"] is True, "Goat policy change did not validate")
    require(
        preview["data"]["diff"]
        == [{
            "id": "ai.enemy.default.policy",
            "before": {"type": "enum", "value": "classic_goat_v1"},
            "after": {"type": "enum", "value": "vigilant_goat_v1"},
        }],
        "published input is not a one-factor Goat-policy diff",
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
        ["get", "--workspace", str(workspace), "--id", "ai.enemy.default.policy"],
    )
    require(
        read_back["data"]["value"] == {"type": "enum", "value": "vigilant_goat_v1"},
        "published Goat policy did not read back",
    )
    return workspace, receipt


def drive(client: NeutralClient, generation: int) -> tuple[dict, list[dict]]:
    require(client.step(generation, 0, {}) == MC_STATUS_OK, "native Goat-policy step failed")
    snapshot = client.snapshot()
    events = client.events(generation, 1, 0)["value"]["events"]
    return snapshot, events


def verify_native(plugin: Path, workspace: Path) -> dict:
    baseline = NeutralClient(plugin)
    authored = NeutralClient(plugin, workspace=workspace)
    try:
        baseline_snapshot, baseline_events = drive(baseline, 1)
        authored_snapshot, authored_events = drive(authored, 1)
        baseline_value = baseline_snapshot["value"]
        authored_value = authored_snapshot["value"]
        for key in (
            "tick", "player_pose", "player_motion", "caster_mana",
            "objective_count", "action_sequence",
        ):
            require(baseline_value[key] == authored_value[key], f"countermetric changed: {key}")
        baseline_engagements = [row for row in baseline_events if row["event_id"] == ENGAGEMENT]
        authored_engagements = [row for row in authored_events if row["event_id"] == ENGAGEMENT]
        require(not baseline_engagements, "classic policy unexpectedly engaged on the first pass")
        require(authored_engagements, "vigilant policy produced no ordinary engagement")
        require(
            all(row["actor"] is not None and row["participant"] is not None
                for row in authored_engagements),
            "engagement event omitted opaque actor or participant identity",
        )
        require(
            [row["event_id"] for row in authored_events if row["event_id"] != ENGAGEMENT]
            == [row["event_id"] for row in baseline_events],
            "an undeclared non-engagement event changed",
        )

        first = copy.deepcopy(authored_value)
        first_event_ids = [row["event_id"] for row in authored_events]
        require(authored.reset(1, 1) == MC_STATUS_OK, "authored reset failed")
        replay, replay_events = drive(authored, 2)
        require(replay["value"] == first, "authored reset replay is not exact")
        require(
            [row["event_id"] for row in replay_events] == first_event_ids,
            "authored reset event order changed",
        )
        return {
            "baseline_definition": baseline_snapshot["identity"]["definition_hash"],
            "variant_definition": authored_snapshot["identity"]["definition_hash"],
            "baseline_engagement_count": 0,
            "variant_engagement_count": len(authored_engagements),
            "tick": authored_value["tick"],
            "event_count": len(authored_events),
        }
    finally:
        baseline.close()
        authored.close()


def run_typescript(plugin: Path, workspace: Path) -> dict:
    environment = os.environ.copy()
    environment["MC_GAME_SESSION_PLUGIN"] = str(plugin)
    environment["MC_SEMANTIC_VARIANT_WORKSPACE"] = str(workspace)
    completed = subprocess.run(
        ["node", "--test", "tests/semantic-goat-policy.contract.test.ts"],
        cwd=WEB,
        env=environment,
        check=False,
        capture_output=True,
        text=True,
        timeout=180,
    )
    require(completed.returncode == 0, f"TypeScript Goat-policy proof failed: {completed.stderr}")
    require(
        "pass 1" in completed.stdout and "fail 0" in completed.stdout,
        f"TypeScript Goat-policy proof counters disagree: {completed.stdout}",
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
        with tempfile.TemporaryDirectory(prefix="magiccarpet-f8-") as temporary:
            root = Path(temporary)
            workspace, receipt = publish_variant(sim_cli, root)
            report = {
                "ok": True,
                "status": "PASS",
                "contract": "carpet-flight-semantic-goat-policy-proof/v1",
                "plugin_sha256": sha256_file(plugin),
                "sim_cli_sha256": sha256_file(sim_cli),
                "package_diff": ["ai.enemy.default.policy:classic_goat_v1->vigilant_goat_v1"],
                "receipt_hash": receipt["receipt_hash"],
                "native": verify_native(plugin, workspace),
                "typescript": run_typescript(plugin, workspace),
                "unity": run_unity(
                    args.unity_project.resolve(),
                    plugin,
                    root / "unity-results.xml",
                    root / "unity.log",
                    Path("/home/oliver/Unity/Hub/Editor/6000.3.20f1/Editor/Unity"),
                    "MagicCarpet.Native.Tests.SemanticGoatPolicyEditModeTests",
                    workspace,
                ),
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
