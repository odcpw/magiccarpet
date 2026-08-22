#!/usr/bin/env python3
"""Prove one typed semantic asset reaches native, TypeScript, and Unity caches."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

from run_engine_session_editmode import run as run_unity
from verify_neutral_game_session import MC_STATUS_OK, NeutralClient, NeutralNativeError
from verify_semantic_flight import SemanticFlightError, require, run_cli, sha256_file


HERE = Path(__file__).resolve().parent
PRIVATE = HERE.parents[1]
SIM = PRIVATE / "remc2-sim-rs"
WEB = SIM / "web"
UNITY_PROJECT = PRIVATE / "remc2-unity-native"
FIXTURE = SIM / "crates/sim-core/tests/fixtures/custom-synthetic-v1"
TRANSFORM = SIM / "tools/build_indexed_sprite.py"
SLOT = "presentation.spell.fireball.projectile_asset"
BASELINE = "sha256:11200749e51e49d4a9f4be95ffa8f6c0636be87e36781481c89e6a0825232508"
VARIANT = "sha256:c924ac200c84b9983374d1abcacff27414c3f565ac82792394353e0358ee0b34"


def create_workspace(sim_cli: Path, path: Path, workspace_id: str) -> dict:
    return run_cli(
        sim_cli,
        [
            "workspace-create",
            "--workspace", str(path),
            "--workspace-id", workspace_id,
            "--package", str(FIXTURE),
        ],
    )


def publish_variant(sim_cli: Path, root: Path) -> tuple[Path, Path, dict]:
    baseline_workspace = root / "baseline-workspace"
    variant_workspace = root / "variant-workspace"
    create_workspace(sim_cli, baseline_workspace, "f9-baseline")
    create_workspace(sim_cli, variant_workspace, "f9-variant")
    changes = root / "asset-changes.json"
    changes.write_text(
        json.dumps(
            [{
                "id": SLOT,
                "value": {"type": "reference", "value": VARIANT},
                "expected_value": {"type": "reference", "value": BASELINE},
            }],
            separators=(",", ":"),
        ),
        encoding="utf-8",
    )
    staged = run_cli(
        sim_cli,
        [
            "set",
            "--workspace", str(variant_workspace),
            "--idempotency-key", "f9-publish-variant",
            "--timing", "package_reload",
            "--changes", str(changes),
            "--intent", "one-factor typed presentation asset replacement",
        ],
    )
    change_set = root / "asset-change-set.json"
    change_set.write_text(
        json.dumps(staged["data"]["change_set"], separators=(",", ":")),
        encoding="utf-8",
    )
    preview = run_cli(
        sim_cli,
        ["preview", "--workspace", str(variant_workspace), "--change-set", str(change_set)],
    )
    validation = preview["data"]["validation"]
    require(bool(validation["accepted"]), "typed asset change did not validate")
    require(
        preview["data"]["diff"] == [{
            "id": SLOT,
            "before": {"type": "reference", "value": BASELINE},
            "after": {"type": "reference", "value": VARIANT},
        }],
        "typed asset publication is not a one-factor semantic diff",
    )
    applied = run_cli(
        sim_cli,
        [
            "apply",
            "--workspace", str(variant_workspace),
            "--change-set", str(change_set),
            "--validation-receipt", validation["receipt_hash"],
        ],
    )["data"]["receipt"]
    require(applied["resulting_definition_revision"] == 2, "asset revision is not 2")
    require(
        applied["prior_artifact_graph_hash"] == applied["resulting_artifact_graph_hash"],
        "semantic binding substituted the retained artifact graph",
    )
    return baseline_workspace, variant_workspace, applied


def verify_transform() -> dict:
    commands = [
        (
            "fireball-baseline.source.json",
            "source.fireball.baseline",
            "projectile.indices.u8",
        ),
        (
            "fireball-variant.source.json",
            "source.fireball.variant",
            "projectile-variant.indices.u8",
        ),
    ]
    for source, source_id, output in commands:
        completed = subprocess.run(
            [
                sys.executable,
                str(TRANSFORM),
                "--source", str(FIXTURE / source),
                "--recipe", str(FIXTURE / "fireball.recipe.json"),
                "--source-id", source_id,
                "--recipe-id", "recipe.fireball.indexed",
                "--output", str(FIXTURE / output),
                "--check",
            ],
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
        require(completed.returncode == 0, f"deterministic asset transform failed: {completed.stderr}")
    return {
        "source_baseline_sha256": sha256_file(FIXTURE / "fireball-baseline.source.json"),
        "source_variant_sha256": sha256_file(FIXTURE / "fireball-variant.source.json"),
        "recipe_sha256": sha256_file(FIXTURE / "fireball.recipe.json"),
        "runtime_baseline_sha256": sha256_file(FIXTURE / "projectile.indices.u8"),
        "runtime_variant_sha256": sha256_file(FIXTURE / "projectile-variant.indices.u8"),
        "rebuild": "exact",
    }


def verify_native(plugin: Path, baseline_workspace: Path, variant_workspace: Path) -> dict:
    baseline = NeutralClient(plugin, workspace=baseline_workspace)
    variant = NeutralClient(plugin, workspace=variant_workspace)
    try:
        baseline_before = baseline.snapshot()
        variant_before = variant.snapshot()
        for key in ("state_hash", "conformance_hash", "event_head", "player_pose", "player_motion"):
            require(
                baseline_before["value"][key] == variant_before["value"][key],
                f"asset-only countermetric changed: {key}",
            )
        baseline_asset = baseline.presentation_asset(1, 0, SLOT)
        for _ in range(7):
            require(
                baseline.presentation_asset(1, 0, SLOT) == baseline_asset,
                "asset polling changed its observation",
            )
        variant_asset = variant.presentation_asset(1, 0, SLOT)
        require(baseline_asset["value"]["content_address"] == BASELINE, "wrong baseline asset")
        require(variant_asset["value"]["content_address"] == VARIANT, "wrong variant asset")
        require(baseline_asset["value"]["indices"] != variant_asset["value"]["indices"],
                "runtime payload did not change")
        require(baseline.snapshot() == baseline_before, "baseline asset reads mutated the session")
        require(variant.snapshot() == variant_before, "variant asset reads mutated the session")
        try:
            variant.presentation_asset(1, 0, "presentation.spell.fireball.missing_asset")
            raise SemanticFlightError("unknown asset slot was accepted")
        except NeutralNativeError:
            pass
        command: dict[str, object] = {}
        require(baseline.step(1, 0, command) == MC_STATUS_OK, "baseline step failed")
        require(variant.step(1, 0, command) == MC_STATUS_OK, "variant step failed")
        baseline_after = baseline.snapshot()
        variant_after = variant.snapshot()
        for key in ("state_hash", "conformance_hash", "event_head", "player_pose", "player_motion"):
            require(
                baseline_after["value"][key] == variant_after["value"][key],
                f"asset-only step countermetric changed: {key}",
            )
        require(
            baseline.events(1, 1, 0)["value"] == variant.events(1, 1, 0)["value"],
            "asset-only gameplay events changed",
        )
        return {
            "baseline_definition": baseline_before["identity"]["definition_hash"],
            "variant_definition": variant_before["identity"]["definition_hash"],
            "baseline_content_address": BASELINE,
            "variant_content_address": VARIANT,
            "polling_reads": 8,
            "state_hash": baseline_after["value"]["state_hash"],
            "conformance_hash": baseline_after["value"]["conformance_hash"],
            "gameplay_events": baseline_after["value"]["event_head"],
        }
    finally:
        baseline.close()
        variant.close()


def run_typescript(plugin: Path, baseline_workspace: Path, variant_workspace: Path) -> dict:
    environment = os.environ.copy()
    environment["MC_GAME_SESSION_PLUGIN"] = str(plugin)
    environment["MC_SEMANTIC_BASELINE_WORKSPACE"] = str(baseline_workspace)
    environment["MC_SEMANTIC_VARIANT_WORKSPACE"] = str(variant_workspace)
    completed = subprocess.run(
        ["node", "--test", "tests/semantic-presentation-asset.contract.test.ts"],
        cwd=WEB,
        env=environment,
        check=False,
        capture_output=True,
        text=True,
        timeout=180,
    )
    require(completed.returncode == 0, f"TypeScript asset proof failed: {completed.stderr}")
    require("pass 1" in completed.stdout and "fail 0" in completed.stdout,
            f"TypeScript asset proof counters disagree: {completed.stdout}")
    return {"status": "PASS", "tests": 1, "cache_invalidation": "observed"}


def verify_rollback(sim_cli: Path, workspace: Path, applied: dict, plugin: Path) -> dict:
    rolled = run_cli(
        sim_cli,
        [
            "rollback",
            "--workspace", str(workspace),
            "--expected-definition-hash", applied["resulting_definition_hash"],
            "--expected-definition-revision", str(applied["resulting_definition_revision"]),
            "--source-transaction", applied["transaction_id"],
            "--idempotency-key", "f9-forward-rollback",
        ],
    )["data"]["receipt"]
    require(rolled["resulting_definition_revision"] == 3, "rollback was not forward revision 3")
    require(rolled["resulting_artifact_graph_hash"] == applied["prior_artifact_graph_hash"],
            "rollback did not retain the baseline artifact graph")
    client = NeutralClient(plugin, workspace=workspace)
    try:
        asset = client.presentation_asset(1, 0, SLOT)
        require(asset["value"]["content_address"] == BASELINE,
                "rollback did not restore baseline binding")
    finally:
        client.close()
    return {"receipt_hash": rolled["receipt_hash"], "revision": 3, "binding": BASELINE}


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
        transform = verify_transform()
        with tempfile.TemporaryDirectory(prefix="magiccarpet-f9-") as temporary:
            root = Path(temporary)
            baseline_workspace, variant_workspace, applied = publish_variant(sim_cli, root)
            native = verify_native(plugin, baseline_workspace, variant_workspace)
            typescript = run_typescript(plugin, baseline_workspace, variant_workspace)
            unity = run_unity(
                args.unity_project.resolve(),
                plugin,
                root / "unity-results.xml",
                root / "unity.log",
                Path("/home/oliver/Unity/Hub/Editor/6000.3.20f1/Editor/Unity"),
                "MagicCarpet.Native.Tests.SemanticPresentationAssetEditModeTests",
                variant_workspace,
                baseline_workspace,
            )
            rollback = verify_rollback(sim_cli, variant_workspace, applied, plugin)
            report = {
                "ok": True,
                "status": "PASS",
                "contract": "carpet-flight-semantic-presentation-asset-proof/v1",
                "plugin_sha256": sha256_file(plugin),
                "unity_plugin_sha256": sha256_file(
                    args.unity_project.resolve() / "Assets/Plugins/x86_64/libsim_capi.so"
                ),
                "sim_cli_sha256": sha256_file(sim_cli),
                "package_graph_hash": applied["resulting_package_graph_hash"],
                "artifact_graph_hash": applied["resulting_artifact_graph_hash"],
                "definition_diff": [f"{SLOT}:{BASELINE}->{VARIANT}"],
                "receipt_hash": applied["receipt_hash"],
                "transform": transform,
                "native": native,
                "typescript": typescript,
                "unity": unity,
                "rollback": rollback,
                "same_executables": True,
            }
        require(report["plugin_sha256"] == report["unity_plugin_sha256"],
                "Unity plug-in differs from the tested candidate")
        print(json.dumps(report, sort_keys=True))
        return 0
    except (
        SemanticFlightError,
        NeutralNativeError,
        OSError,
        ValueError,
        KeyError,
        subprocess.TimeoutExpired,
    ) as error:
        print(json.dumps({"ok": False, "status": "FAIL", "error": str(error)}), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
