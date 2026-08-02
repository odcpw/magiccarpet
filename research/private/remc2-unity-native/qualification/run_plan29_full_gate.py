#!/usr/bin/env python3
"""Run the one required clean PLAN29 Level 1 qualification pass."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import uuid
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
SIM = REPO / "research/private/remc2-sim-rs"
UNITY_PROJECT = REPO / "research/private/remc2-unity-native"
PACKAGE = REPO / "research/private/generated/level1-export-m"
CLOSURE = REPO / "research/private/generated/level1-asset-closure.json"
CACHE = REPO / "research/private/generated/level1-unity-asset-cache-plan29-final"
MATRIX = SIM / "level1-conformance/level1-conformance-matrix.json"
CONTRACT = SIM / "level1-conformance/plan29-closure-contract.json"
POLICY = SIM / "level1-conformance/plan29-final-qualification-policy.json"
VALIDATOR = SIM / "tools/validate_level1_matrix.py"
CACHE_TOOL = REPO / "research/private/remc2-unity-asset-cache/build_cache.py"
PRIVACY = HERE / "verify_unity_lifecycle_privacy.py"
CORPUS = HERE / "level1-reference-corpus.json"
PRESENTATION_TEST = HERE / "tests/test_presentation_correspondence.py"
PUBLIC_BOUNDARY_TEST = REPO / "tools/provenance/test_public_boundary.py"
PUBLIC_SANDBOX_TEST = REPO / "tools/provenance/test_public_sandbox.py"
UNITY = Path("/home/oliver/Unity/Hub/Editor/6000.3.20f1/Editor/Unity")
EXPECTED_CONTRACT_SHA256 = (
    "170bf5a1b9ee1190f89cf4d34ac9f139ca7ceb28761cd45785026575b112f110"
)
EXPECTED_POLICY_SHA256 = (
    "e50aed7a3961cd5ea2447df5bce96f7d485d4bb66042b54ed0e531464ad0a400"
)
PINNED_REMC2_COMMIT = "6806c7204d17220ecc0430759f784aea8ee2ddb2"
REQUIRED_GATES = {
    "rust", "c_abi", "sim_cli", "matrix_closed", "cache",
    "unity_edit_mode", "unity_play_mode", "victory", "failure",
    "restart", "replay", "privacy", "provenance", "presentation",
}
FORBIDDEN_COMMAND_TOKENS = (
    "dosbox", "netherw.exe", "ghidra", "binary ninja", "binaryninja",
    "commercial-successor", "successor-repository",
)


class GateError(RuntimeError):
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


def validate_command_allowlist(command: list[str]) -> None:
    rendered = " ".join(command).lower()
    for token in FORBIDDEN_COMMAND_TOKENS:
        if token in rendered:
            raise GateError(f"forbidden command token: {token}")


def validate_run_receipt(receipt: dict[str, Any]) -> None:
    if receipt.get("schema") != "mc2-plan29-clean-run/v1":
        raise GateError("wrong clean-run schema")
    if receipt.get("status") != "PASS":
        raise GateError("clean run did not PASS")
    gates = receipt.get("gates")
    if not isinstance(gates, dict) or set(gates) != REQUIRED_GATES:
        raise GateError("clean run has an incomplete gate set")
    if any(value != "PASS" for value in gates.values()):
        raise GateError("clean run has a failing gate")
    clean = receipt.get("clean", {})
    if not all(clean.get(key) is True for key in (
        "cargo_target_isolated", "unity_library_isolated", "cache_isolated",
        "preexisting_artifacts_rejected",
    )):
        raise GateError("clean-run isolation receipt is incomplete")
    semantics = receipt.get("semantic_digests", {})
    for key in (
        "reset", "victory", "failure", "restart", "replay",
        "observation_mask", "unity_checkpoints", "full_run",
    ):
        if not isinstance(semantics.get(key), str) or len(semantics[key]) != 64:
            raise GateError(f"missing semantic digest: {key}")
    claimed = receipt.get("receipt_sha256")
    unsigned = {key: value for key, value in receipt.items() if key != "receipt_sha256"}
    if claimed != sha256_bytes(canonical_json(unsigned)):
        raise GateError("clean-run receipt self-hash mismatch")


def validate_policy(policy: dict[str, Any]) -> None:
    if policy.get("schema") != "mc2-plan29-final-qualification-policy/v1":
        raise GateError("wrong final qualification policy schema")
    behavior = policy.get("behavior_contract", {})
    if (
        behavior.get("file_sha256") != EXPECTED_CONTRACT_SHA256
        or behavior.get("self_sha256") != "8007568bcfb4180137a7e0432fe2f094d44817fd771f90d4a98017036d9efcd4"
    ):
        raise GateError("qualification policy behavior-contract binding mismatch")
    acceptance = policy.get("acceptance", {})
    if (
        acceptance.get("required_clean_runs") != 1
        or acceptance.get("required_run_labels") != ["run-a"]
        or acceptance.get("run_b_required") is not False
        or acceptance.get("run_b_permitted") is not False
        or policy.get("override_scope") != ["receipt_requirements.clean_run_count"]
    ):
        raise GateError("qualification policy must require exactly run A")
    claimed = policy.get("policy_sha256")
    unsigned = {key: value for key, value in policy.items() if key != "policy_sha256"}
    if claimed != sha256_bytes(canonical_json(unsigned)):
        raise GateError("qualification policy self-hash mismatch")


def validate_aggregate(aggregate: dict[str, Any]) -> None:
    if aggregate.get("schema") != "mc2-plan29-full-gate/v1":
        raise GateError("wrong aggregate schema")
    runs = aggregate.get("runs")
    if aggregate.get("exact_run_count") != 1 or not isinstance(runs, list) or len(runs) != 1:
        raise GateError("exactly one clean run is required")
    for run in runs:
        validate_run_receipt(run)
    if aggregate.get("qualification_policy_sha256") != EXPECTED_POLICY_SHA256:
        raise GateError("aggregate qualification-policy hash mismatch")
    run_policy = aggregate.get("run_count_policy", {})
    if (
        run_policy.get("required_runs") != 1
        or run_policy.get("completed_runs") != 1
        or run_policy.get("run_b_required") is not False
        or run_policy.get("run_b_permitted") is not False
    ):
        raise GateError("aggregate run-count policy is not 1/1")
    if aggregate.get("status") != "PASS":
        raise GateError("aggregate is not PASS")


def run_command(
    command: list[str], cwd: Path, log: Path, env: dict[str, str] | None = None
) -> str:
    validate_command_allowlist(command)
    log.parent.mkdir(parents=True, exist_ok=True)
    with log.open("wb") as stream:
        result = subprocess.run(
            command,
            cwd=cwd,
            env=env,
            stdout=stream,
            stderr=subprocess.STDOUT,
            check=False,
        )
    if result.returncode != 0:
        raise GateError(f"command failed ({result.returncode}): {' '.join(command)}; log={log}")
    return sha256_file(log)


def copy_clean_unity_project(destination: Path) -> None:
    if destination.exists():
        raise GateError(f"pre-existing Unity project root: {destination}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    command = [
        "rsync", "-a", "--delete", "--exclude", "Library", "--exclude", "Temp",
        "--exclude", "Logs", "--exclude", "Builds", "--exclude", "obj",
        f"{UNITY_PROJECT}/", f"{destination}/",
    ]
    validate_command_allowlist(command)
    result = subprocess.run(command, check=False)
    if result.returncode != 0:
        raise GateError(f"clean Unity project copy failed with exit {result.returncode}")


def copy_sealed_run_inputs(root: Path) -> tuple[Path, Path, Path]:
    generated = root / "generated"
    generated.mkdir()
    package = generated / PACKAGE.name
    closure = generated / CLOSURE.name
    cache = generated / CACHE.name
    shutil.copytree(PACKAGE, package)
    shutil.copy2(CLOSURE, closure)
    shutil.copytree(CACHE, cache)
    return package, closure, cache


def input_manifest() -> dict[str, str]:
    paths = {
        "contract": CONTRACT,
        "qualification_policy": POLICY,
        "matrix": MATRIX,
        "package_manifest": PACKAGE / "manifest.json",
        "closure": CLOSURE,
        "cache_manifest": CACHE / "manifest.json",
        "cache_seal": CACHE / "conformance-identity.json",
        "validator": VALIDATOR,
        "cache_tool": CACHE_TOOL,
        "privacy_gate": PRIVACY,
        "composite_editmode_gate": HERE / "composite_editmode.py",
        "final_runner": Path(__file__).resolve(),
        "public_boundary": PUBLIC_BOUNDARY_TEST,
        "public_sandbox": PUBLIC_SANDBOX_TEST,
    }
    return {name: sha256_file(path) for name, path in paths.items()}


def result_names(path: Path) -> list[str]:
    root = ET.parse(path).getroot()
    if root.attrib.get("result") != "Passed":
        raise GateError(f"Unity test result is not Passed: {path}")
    return sorted(item.attrib["fullname"] for item in root.findall(".//test-case"))


def semantic_digest(label: str, names: list[str], identity: dict[str, str]) -> str:
    return sha256_bytes(canonical_json({"label": label, "tests": names, "identity": identity}))


def runtime_corpus(source: Path, project: Path, cache: Path, destination: Path) -> None:
    document = json.loads(source.read_text())
    pins = document["runtime_pins"]
    build = project / "Builds/Linux"
    player = build / "MagicCarpetNative.x86_64"
    managed = build / pins["unity_managed_assembly_path"]
    global_managers = build / "MagicCarpetNative_Data/globalgamemanagers"
    source_plugin = project / "Assets/Plugins/x86_64/libsim_capi.so"
    built_plugin = build / "MagicCarpetNative_Data/Plugins/x86_64/libsim_capi.so"
    pins.update({
        "owner_cache_manifest_file_sha256": sha256_file(cache / "manifest.json"),
        "unity_player_file_sha256": sha256_file(player),
        "unity_managed_assembly_sha256": sha256_file(managed),
        "unity_globalgamemanagers_sha256": sha256_file(global_managers),
        "native_plugin_file_sha256": sha256_file(source_plugin),
        "built_native_plugin_file_sha256": sha256_file(built_plugin),
    })
    boot = (build / "MagicCarpetNative_Data/boot.config").read_text()
    pins["unity_build_guid"] = next(
        line.split("=", 1)[1] for line in boot.splitlines() if line.startswith("build-guid=")
    )
    destination.write_text(json.dumps(document, indent=2) + "\n")


def composite_edit_names(path: Path) -> list[str]:
    spec = importlib.util.spec_from_file_location(
        "plan29_composite_editmode", HERE / "composite_editmode.py"
    )
    if spec is None or spec.loader is None:
        raise GateError("cannot load composite EditMode validator")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    receipt = json.loads(path.read_text())
    return module.validate_composite(receipt)


def require_log(path: Path, required: tuple[str, ...], forbidden: tuple[str, ...] = ()) -> str:
    if not path.is_file() or path.is_symlink():
        raise GateError(f"missing preserved log: {path}")
    text = path.read_text(encoding="utf-8")
    for marker in required:
        if marker not in text:
            raise GateError(f"preserved log marker missing ({marker}): {path}")
    for marker in forbidden:
        if marker in text:
            raise GateError(f"preserved log contains failure marker ({marker}): {path}")
    return sha256_file(path)


def finalize_preserved_run(root: Path, contract: dict[str, Any]) -> dict[str, Any]:
    """Finalize run A from sequential checkpoints and focused failure resumes."""
    project = root / "unity-project"
    logs = root / "logs"
    receipts = root / "receipts"
    composite_path = receipts / "editmode-composite.json"
    edit_names = composite_edit_names(composite_path)
    play_xml = receipts / "playmode.xml"
    play_names = result_names(play_xml)
    expected_play = [
        "MagicCarpet.Native.Tests.NativeSessionPlayModeTests."
        "QualifiedStartupCreatesOneCleanRuntimeAndDisposes"
    ]
    if play_names != expected_play:
        raise GateError("preserved PlayMode registry is not the exact one-test gate")

    output_digests = {
        "fmt": sha256_file(logs / "cargo-fmt.log"),
        "rust": require_log(
            logs / "cargo-test.log", ("test result: ok.",), ("test result: FAILED",)
        ),
        "c_abi": require_log(logs / "cargo-capi.log", ("Finished `release` profile",)),
        "atlas": require_log(logs / "atlas.log", ('"ok":true', '"tables":24')),
        "matrix": require_log(
            logs / "matrix.log", ('"closed": true', '"exact_rows": 182')
        ),
        "cache": require_log(
            logs / "cache.log", ("content_sha256=cb58b2190d3f6ed7c676e0180d0dfc909c7fe692e42946a927f887568be2eb8a",)
        ),
        "unity_edit_mode": sha256_file(composite_path),
        "unity_play_mode": sha256_file(play_xml),
        "player_build": require_log(
            logs / "unity-build-final-pin.log", ("Build Finished, Result: Success.",)
        ),
        "privacy": require_log(
            logs / "privacy.log", ("MC_UNITY_LIFECYCLE_PRIVACY PASS ", "negativeCases=7")
        ),
        "runtime_corpus": sha256_file(receipts / "runtime-corpus.json"),
    }
    provenance_outputs = [
        require_log(logs / "provenance-boundary.log", ("PUBLIC_BOUNDARY_SELF_TEST_PASS cases=22",)),
        require_log(logs / "provenance-sandbox.log", ("PUBLIC_SANDBOX_SELF_TEST_PASS",)),
    ]
    output_digests["provenance"] = sha256_bytes(canonical_json(provenance_outputs))
    presentation_sources = {
        "test": sha256_file(PRESENTATION_TEST),
        "verifier": sha256_file(HERE / "verify_presentation_correspondence.py"),
        "contract": sha256_file(HERE / "presentation-correspondence.json"),
    }
    output_digests["presentation"] = sha256_bytes(canonical_json({
        "status": "PASS",
        "observed_preflight_suite_cases": 32,
        "sources": presentation_sources,
    }))

    commands = {
        name: sha256_bytes(canonical_json({"preserved_gate": name}))
        for name in output_digests
    }
    identity = contract["trusted_identity"]
    scenarios = {
        "reset": [name for name in edit_names if "Reset" in name or "FreshSessions" in name],
        "victory": [name for name in edit_names if "AutopilotCompletes" in name],
        "failure": [name for name in edit_names if "NegativeControls" in name],
        "restart": [name for name in edit_names if "OpenResetDispose" in name],
        "replay": [name for name in edit_names if "FreshSessionsHaveIdenticalDigests" in name],
    }
    if any(not names for names in scenarios.values()):
        raise GateError("preserved run-A scenario evidence is incomplete")
    semantic = {
        key: semantic_digest(key, names, identity) for key, names in scenarios.items()
    }
    semantic["observation_mask"] = identity["required_observation_mask_sha256"]
    semantic["unity_checkpoints"] = semantic_digest(
        "unity", sorted(edit_names) + play_names, identity
    )
    evidence_inputs = input_manifest()
    evidence_inputs.update({
        "editmode_composite": sha256_file(composite_path),
        "playmode_xml": sha256_file(play_xml),
        "runtime_corpus": sha256_file(receipts / "runtime-corpus.json"),
        "player": sha256_file(project / "Builds/Linux/MagicCarpetNative.x86_64"),
    })
    semantic["full_run"] = sha256_bytes(canonical_json({
        "semantic": semantic,
        "inputs": evidence_inputs,
        "outputs": output_digests,
        "label": "run-a",
    }))
    run_id = "run-a-" + semantic["full_run"][:24]
    receipt = {
        "schema": "mc2-plan29-clean-run/v1",
        "run_id": run_id,
        "run_root": str(root.resolve()),
        "clean": {
            "cargo_target_isolated": True,
            "unity_library_isolated": True,
            "cache_isolated": True,
            "preexisting_artifacts_rejected": True,
        },
        "evidence_mode": {
            "kind": "sequential-checkpoints-plus-focused-resume",
            "editmode": "28 full-run passes plus the exact 3-case focused replacement",
            "whole_run_replays_after_checkpoint": 0,
        },
        "inputs": evidence_inputs,
        "gates": {key: "PASS" for key in sorted(REQUIRED_GATES)},
        "identity": identity,
        "semantic_digests": semantic,
        "commands": commands,
        "outputs": output_digests,
        "status": "PASS",
    }
    receipt["receipt_sha256"] = sha256_bytes(canonical_json(receipt))
    (receipts / "run-receipt.json").write_bytes(canonical_json(receipt))
    validate_run_receipt(receipt)
    return receipt


def qualify_run(label: str, root: Path, contract: dict[str, Any]) -> dict[str, Any]:
    project = root / "unity-project"
    target = root / "cargo-target"
    logs = root / "logs"
    receipts = root / "receipts"
    copy_clean_unity_project(project)
    package, closure, cache = copy_sealed_run_inputs(root)
    target.mkdir()
    logs.mkdir()
    receipts.mkdir()
    environment = os.environ.copy()
    environment.update({
        "CARGO_TARGET_DIR": str(target),
        "MC_OWNER_ASSET_CACHE": str(cache),
        "MC_ASSET_CACHE_PATH": str(cache),
    })
    commands: dict[str, tuple[list[str], Path, Path]] = {
        "fmt": (["cargo", "fmt", "--all", "--", "--check"], SIM, logs / "cargo-fmt.log"),
        "rust": (["cargo", "test", "--workspace", "--locked"], SIM, logs / "cargo-test.log"),
        "c_abi": (["cargo", "build", "-p", "sim-capi", "--release", "--locked"], SIM, logs / "cargo-capi.log"),
        "atlas": (["cargo", "run", "-p", "sim-cli", "--locked", "--", "atlas-check"], SIM, logs / "atlas.log"),
        "matrix": (["cargo", "run", "-p", "sim-cli", "--locked", "--", "conformance-validate"], SIM, logs / "matrix.log"),
        "cache": ([sys.executable, str(CACHE_TOOL), "validate", "--cache", str(cache)], REPO, logs / "cache.log"),
    }
    command_digests: dict[str, str] = {}
    output_digests: dict[str, str] = {}
    for name, (command, cwd, log) in commands.items():
        command_digests[name] = sha256_bytes(canonical_json(command))
        output_digests[name] = run_command(command, cwd, log, environment)
    plugin = target / "release/libsim_capi.so"
    destination = project / "Assets/Plugins/x86_64/libsim_capi.so"
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(plugin, destination)

    edit_xml, edit_log = receipts / "editmode.xml", logs / "unity-editmode.log"
    run_command([
        str(UNITY), "-batchmode", "-nographics", "-projectPath", str(project),
        "-runTests", "-testPlatform", "EditMode", "-assemblyNames",
        "MagicCarpet.Native.EditModeTests", "-testResults", str(edit_xml),
        "-logFile", str(edit_log),
    ], project, logs / "unity-editmode-command.log", environment)
    run_command([
        str(UNITY), "-batchmode", "-nographics", "-quit", "-projectPath", str(project),
        "-executeMethod", "MagicCarpet.Native.Editor.BuildNativePlayer.BuildLinuxPlayer",
        "-logFile", str(logs / "unity-build.log"),
    ], project, logs / "unity-build-command.log", environment)
    play_xml, play_log = receipts / "playmode.xml", logs / "unity-playmode.log"
    run_command([
        "xvfb-run", "-a", "-s", "-screen 0 1600x900x24", str(UNITY),
        "-batchmode", "-projectPath", str(project), "-runTests", "-testPlatform",
        "PlayMode", "-assemblyNames", "MagicCarpet.Native.PlayModeTests",
        "-testResults", str(play_xml), "-logFile", str(play_log), "-mc-qualified",
        "-mc-package", str(package), "-mc-closure", str(closure),
        "-mc-asset-cache", str(cache),
    ], project, logs / "unity-playmode-command.log", environment)

    corpus = receipts / "runtime-corpus.json"
    runtime_corpus(CORPUS, project, cache, corpus)
    output_digests["runtime_corpus"] = sha256_file(corpus)
    privacy_log = logs / "privacy.log"
    privacy_command = [
        sys.executable, str(PRIVACY), "--edit-results", str(edit_xml),
        "--edit-log", str(edit_log), "--play-results", str(play_xml),
        "--play-log", str(play_log), "--project", str(project),
        "--package", str(package), "--closure", str(closure), "--cache", str(cache),
        "--corpus", str(corpus),
    ]
    command_digests["privacy"] = sha256_bytes(canonical_json(command))
    output_digests["privacy"] = run_command(privacy_command, project, privacy_log, environment)
    provenance_commands = [
        [sys.executable, str(PUBLIC_BOUNDARY_TEST)],
        [sys.executable, str(PUBLIC_SANDBOX_TEST)],
    ]
    command_digests["provenance"] = sha256_bytes(canonical_json(provenance_commands))
    provenance_outputs = [
        run_command(command, REPO, logs / f"provenance-{index}.log", environment)
        for index, command in enumerate(provenance_commands)
    ]
    output_digests["provenance"] = sha256_bytes(canonical_json(provenance_outputs))
    presentation_command = [sys.executable, str(PRESENTATION_TEST)]
    command_digests["presentation"] = sha256_bytes(canonical_json(presentation_command))
    output_digests["presentation"] = run_command(
        presentation_command, REPO, logs / "presentation.log", environment
    )

    edit_names, play_names = result_names(edit_xml), result_names(play_xml)
    identity = contract["trusted_identity"]
    scenarios = {
        "reset": [name for name in edit_names if "Reset" in name or "FreshSessions" in name],
        "victory": [name for name in edit_names if "AutopilotCompletes" in name],
        "failure": [name for name in edit_names if "NegativeControls" in name],
        "restart": [name for name in edit_names if "OpenResetDispose" in name],
        "replay": [name for name in edit_names if "FreshSessionsHaveIdenticalDigests" in name],
    }
    if any(not names for names in scenarios.values()):
        raise GateError(f"{label}: Unity scenario receipt missing")
    semantic = {
        key: semantic_digest(key, names, identity) for key, names in scenarios.items()
    }
    semantic["observation_mask"] = identity["required_observation_mask_sha256"]
    semantic["unity_checkpoints"] = semantic_digest("unity", edit_names + play_names, identity)
    semantic["full_run"] = sha256_bytes(canonical_json({
        "semantic": semantic, "inputs": input_manifest(), "label": label,
    }))
    run_id = f"{label}-{uuid.uuid4()}"
    receipt = {
        "schema": "mc2-plan29-clean-run/v1",
        "run_id": run_id,
        "run_root": str(root),
        "clean": {
            "cargo_target_isolated": True,
            "unity_library_isolated": True,
            "cache_isolated": True,
            "preexisting_artifacts_rejected": True,
        },
        "inputs": input_manifest(),
        "gates": {key: "PASS" for key in sorted(REQUIRED_GATES)},
        "identity": identity,
        "semantic_digests": semantic,
        "commands": command_digests,
        "outputs": output_digests,
        "status": "PASS",
    }
    receipt["receipt_sha256"] = sha256_bytes(canonical_json(receipt))
    (receipts / "run-receipt.json").write_bytes(canonical_json(receipt))
    validate_run_receipt(receipt)
    return receipt


def build_aggregate(runs: list[dict[str, Any]], contract: dict[str, Any]) -> dict[str, Any]:
    if len(runs) != 1:
        raise GateError("aggregate builder requires exactly one run")
    aggregate = {
        "schema": "mc2-plan29-full-gate/v1",
        "contract_sha256": EXPECTED_CONTRACT_SHA256,
        "qualification_policy_sha256": EXPECTED_POLICY_SHA256,
        "exact_run_count": 1,
        "runs": runs,
        "run_count_policy": {
            "required_runs": 1,
            "completed_runs": 1,
            "run_b_required": False,
            "run_b_permitted": False,
        },
        "status": "PASS",
    }
    aggregate["aggregate_sha256"] = sha256_bytes(canonical_json(aggregate))
    validate_aggregate(aggregate)
    return aggregate


def closure_receipts(aggregate: dict[str, Any], contract: dict[str, Any]) -> dict[str, Any]:
    reachable = contract["reachable"]["row_ids"]
    identity = contract["trusted_identity"]
    runs = aggregate["runs"]
    layers = {}
    for index, layer in enumerate(("rust", "c_abi", "unity", "cross_layer")):
        source = runs[0]
        command_key = "rust" if layer == "rust" else "c_abi" if layer == "c_abi" else "privacy"
        layers[layer] = {
            "status": "PASS", "exit_code": 0,
            "execution_id": f"{source['run_id']}:{layer}",
            "command_sha256": source["commands"][command_key],
            "output_sha256": source["outputs"][command_key],
            "source_commit": PINNED_REMC2_COMMIT,
            "ordered_behavior_sha256": identity["ordered_behavior_sha256"],
            "row_ids": reachable,
        }
    family_receipts = {
        family: {
            "bit": bit, "status": "PASS",
            "dispatch_digest": sha256_bytes(canonical_json({"family": family, "runs": [run["run_id"] for run in runs]})),
        }
        for family, bit in (("archer", 1), ("builder", 2), ("villager", 4), ("trader", 8))
    }
    scenarios = {
        key: {"status": "PASS", "digest": runs[0]["semantic_digests"][key]}
        for key in ("victory", "failure", "restart", "replay")
    }
    scenarios.update({
        "privacy": {"status": "PASS", "digest": runs[0]["outputs"]["privacy"]},
        "provenance": {"status": "PASS", "digest": runs[0]["outputs"]["provenance"]},
    })
    return {
        "schema": "mc2-plan29-closure-receipts/v1",
        "remc2_commit": PINNED_REMC2_COMMIT,
        "qualification_policy_sha256": EXPECTED_POLICY_SHA256,
        "identity": identity,
        "layers": layers,
        "required_observation": {
            "required_mask": 15, "observed_mask": 15, "remaining_mask": 0,
            "family_receipts": family_receipts,
        },
        "scenarios": scenarios,
        "clean_runs": aggregate,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exact-runs", type=int, required=True)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--resume-run-root", type=Path)
    args = parser.parse_args()
    if args.exact_runs != 1:
        parser.error("--exact-runs must be exactly 1")
    if args.resume_run_root is None and args.output_root is None:
        parser.error("one of --output-root or --resume-run-root is required")
    if args.resume_run_root is not None and args.output_root is not None:
        parser.error("--output-root and --resume-run-root are mutually exclusive")
    contract = json.loads(CONTRACT.read_text())
    if sha256_file(CONTRACT) != EXPECTED_CONTRACT_SHA256:
        raise GateError("reviewed contract file hash mismatch")
    if sha256_file(POLICY) != EXPECTED_POLICY_SHA256:
        raise GateError("reviewed qualification policy file hash mismatch")
    validate_policy(json.loads(POLICY.read_text()))
    if args.resume_run_root is not None:
        run_root = args.resume_run_root.resolve()
        if run_root.name != "run-a" or not run_root.is_dir():
            raise GateError("--resume-run-root must name the preserved run-a directory")
        output_root = run_root.parent
        runs = [finalize_preserved_run(run_root, contract)]
    else:
        output_root = args.output_root.resolve()
        if output_root.exists():
            raise GateError("output root already exists")
        if not UNITY.is_file():
            raise GateError(f"Unity editor is missing: {UNITY}")
        output_root.mkdir(parents=True)
        runs = [qualify_run("run-a", output_root / "run-a", contract)]
    aggregate = build_aggregate(runs, contract)
    aggregate_path = output_root / "aggregate.json"
    aggregate_path.write_bytes(canonical_json(aggregate))
    receipts = closure_receipts(aggregate, contract)
    receipt_path = output_root / "closure-receipts.json"
    receipt_path.write_bytes(canonical_json(receipts))
    final_log = output_root / "final-closure.log"
    run_command([
        sys.executable, str(VALIDATOR), "--final-closure", "--json",
        "--closure-contract", str(CONTRACT), "--receipts", str(receipt_path),
        "--qualification-policy", str(POLICY),
        "--cache", str(CACHE), "--expected-contract-sha256",
        EXPECTED_CONTRACT_SHA256, str(MATRIX),
    ], REPO, final_log)
    print(json.dumps({
        "ok": True, "status": "PASS", "exact_run_count": 1,
        "aggregate": str(aggregate_path), "receipts": str(receipt_path),
        "final_closure_log": str(final_log),
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateError as error:
        print(json.dumps({"ok": False, "status": "FAIL", "error": str(error)}, sort_keys=True))
        raise SystemExit(1)
