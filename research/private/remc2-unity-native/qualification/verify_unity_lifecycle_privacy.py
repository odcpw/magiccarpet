#!/usr/bin/env python3
"""Verify Unity lifecycle tests and the owner-data-free player boundary."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path


HERE = Path(__file__).resolve().parent
PROJECT = HERE.parent
REPO = HERE.parents[3]
CORPUS_PATH = HERE / "level1-reference-corpus.json"
BUILD = PROJECT / "Builds/Linux"
PLAYER = BUILD / "MagicCarpetNative.x86_64"
PACKAGE = REPO / "research/private/generated/level1-export-m"
CLOSURE = REPO / "research/private/generated/level1-asset-closure.json"
CACHE = REPO / "research/private/generated/level1-unity-asset-cache"

FORBIDDEN_OWNER_EXTENSIONS = {
    ".dat",
    ".tab",
    ".wav",
    ".mid",
    ".midi",
    ".xmi",
    ".ogg",
    ".mp3",
    ".flac",
    ".voc",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def parse_test_result(
    path: Path,
    expected_names: set[str],
    platform: str,
) -> None:
    require(path.is_file() and not path.is_symlink(), f"missing test XML: {path}")
    root = ET.parse(path).getroot()
    require(root.tag == "test-run", f"unexpected test XML root: {path}")
    require(root.attrib.get("result") == "Passed", f"{platform} tests did not pass")
    expected_total = len(expected_names)
    require(int(root.attrib.get("total", "-1")) == expected_total, f"{platform} total")
    require(int(root.attrib.get("passed", "-1")) == expected_total, f"{platform} passed")
    for field in ("failed", "inconclusive", "skipped"):
        require(int(root.attrib.get(field, "-1")) == 0, f"{platform} {field}")
    properties = {
        item.attrib.get("name"): item.attrib.get("value")
        for item in root.findall(".//property")
    }
    require(properties.get("platform") == platform, f"{platform} XML platform")
    names = {
        item.attrib["fullname"]
        for item in root.findall(".//test-case")
    }
    require(names == expected_names, f"{platform} test registry changed")


def verify_test_evidence(args: argparse.Namespace) -> int:
    edit_prefix = "MagicCarpet.Native.Tests.NativeSessionEditModeTests."
    parse_test_result(
        args.edit_results,
        {
            edit_prefix + "AbiAndManagedLayoutsArePinned",
            edit_prefix + "BulkTerrainCheckpointReusesBuffersWithinAnAllocationBudget",
            edit_prefix + "KnownInputAndCameraRouteMatchesTheQualifiedGolden",
            edit_prefix + "MissingOrTamperedOwnerInputsFailClosed",
            edit_prefix + "OpenResetDisposeAndTwoSessionsRemainIndependent",
            edit_prefix + "GoatBulkRowsAreExactReusableResettableAndSessionLocal",
        },
        "EditMode",
    )
    parse_test_result(
        args.play_results,
        {
            "MagicCarpet.Native.Tests.NativeSessionPlayModeTests."
            "QualifiedStartupCreatesOneCleanRuntimeAndDisposes",
        },
        "PlayMode",
    )
    edit_log = args.edit_log.read_text(encoding="utf-8")
    play_log = args.play_log.read_text(encoding="utf-8")
    require("MC_BULK_ABI PASS " in edit_log, "bulk ABI allocation marker missing")
    match = re.search(
        r"MC_BULK_ABI PASS iterations=32 allocatedBytes=(\d+) "
        r"elapsedMs=(\d+) gridVertices=840 framePixels=64000 "
        r"buffersReused=true",
        edit_log,
    )
    require(match is not None, "bulk ABI allocation marker malformed")
    allocated = int(match.group(1))
    require(allocated <= 64 * 1024, "bulk ABI managed allocation budget exceeded")
    require(
        "MC_ASSET_CACHE VERIFIED outputs=293 " in play_log
        and "MC_PRESENTATION_MODE mode=qualified ownerCache=True" in play_log
        and "GfxDevice: creating device client;" in play_log
        and "Renderer:" in play_log
        and "Renderer: Null Device" not in play_log,
        "qualified PlayMode startup markers missing",
    )
    require("MC_FATAL " not in play_log and "MC_RUNTIME_FATAL " not in play_log,
            "qualified PlayMode startup logged a fatal error")
    return allocated


def registered_owner_hashes(corpus: dict) -> set[str]:
    package_manifest = json.loads((PACKAGE / "manifest.json").read_text())
    closure = json.loads(CLOSURE.read_text())
    cache = json.loads((CACHE / "manifest.json").read_text())
    catalog_registration = closure["source"]["owner_catalog"]
    catalog_path = REPO / catalog_registration["path"]
    require(
        sha256_file(catalog_path) == catalog_registration["file_sha256"],
        "registered owner catalog file identity changed",
    )
    catalog = json.loads(catalog_path.read_text())
    require(
        catalog["catalog_sha256"] == catalog_registration["catalog_sha256"],
        "registered owner catalog semantic identity changed",
    )
    hashes = {
        corpus["runtime_pins"]["package_manifest_file_sha256"],
        corpus["runtime_pins"]["closure_file_sha256"],
        corpus["runtime_pins"]["owner_cache_manifest_file_sha256"],
    }
    hashes.update(
        item["sha256"]
        for item in catalog["files"]
        if item["provenance"] == "owner_data"
    )
    hashes.update(item["sha256"] for item in package_manifest["files"].values())
    hashes.update(item["sha256"] for item in cache["source_files"])
    hashes.update(item["sha256"] for item in cache["outputs"])
    for suite in corpus["suites"]:
        for fixture in suite.get("fixtures", []):
            if "sha256" in fixture:
                hashes.add(fixture["sha256"])
        for field, value in suite.items():
            if field.endswith("_sha256") and isinstance(value, str) and len(value) == 64:
                hashes.add(value)
    require(
        closure["scope"] == "PRIVATE_TRACK_P_OWNER_DATA",
        "closure privacy scope changed",
    )
    return hashes


def verify_build_surface(corpus: dict) -> tuple[int, int]:
    pins = corpus["runtime_pins"]
    require(BUILD.is_dir() and PLAYER.is_file(), "Unity player build is missing")
    require(not (PROJECT / "Assets/StreamingAssets").exists(),
            "owner data must not enter StreamingAssets")
    managed_directory = BUILD / "MagicCarpetNative_Data/Managed"
    require(
        not any("Tests" in path.name for path in managed_directory.glob("*.dll")),
        "Unity test assembly was included in the player",
    )
    scripting_assemblies = (
        BUILD / "MagicCarpetNative_Data/ScriptingAssemblies.json"
    ).read_text(encoding="utf-8")
    require(
        "MagicCarpet.Native.EditModeTests" not in scripting_assemblies
        and "MagicCarpet.Native.PlayModeTests" not in scripting_assemblies,
        "Unity test assembly is registered in the player",
    )
    resources = PROJECT / "Assets/Resources"
    require(
        all(
            path.suffix.lower() in {".shader", ".meta"}
            for path in resources.rglob("*")
            if path.is_file()
        ),
        "Assets/Resources contains a non-shader payload",
    )

    roots = (PROJECT / "Assets", BUILD)
    files: list[Path] = []
    for root in roots:
        for path in root.rglob("*"):
            require(not path.is_symlink(), f"symlink in player source/build surface: {path}")
            if path.is_file():
                files.append(path)
                require(
                    path.suffix.lower() not in FORBIDDEN_OWNER_EXTENSIONS,
                    f"owner audiovisual/container extension in player surface: {path}",
                )

    owner_hashes = registered_owner_hashes(corpus)
    intersections = [
        path for path in files
        if sha256_file(path) in owner_hashes
    ]
    require(
        not intersections,
        "player source/build contains a registered owner payload identity: "
        + ", ".join(str(path) for path in intersections),
    )

    require(sha256_file(PLAYER) == pins["unity_player_file_sha256"],
            "Unity launcher identity mismatch")
    managed = BUILD / pins["unity_managed_assembly_path"]
    require(sha256_file(managed) == pins["unity_managed_assembly_sha256"],
            "Unity runtime assembly identity mismatch")
    global_managers = BUILD / "MagicCarpetNative_Data/globalgamemanagers"
    require(
        sha256_file(global_managers) == pins["unity_globalgamemanagers_sha256"],
        "Unity globalgamemanagers identity mismatch",
    )
    plugin = BUILD / "MagicCarpetNative_Data/Plugins/x86_64/libsim_capi.so"
    require(sha256_file(plugin) == pins["built_native_plugin_file_sha256"],
            "built native plug-in identity mismatch")
    boot = (BUILD / "MagicCarpetNative_Data/boot.config").read_text()
    require(
        f"build-guid={pins['unity_build_guid']}" in boot,
        "Unity build GUID mismatch",
    )

    input_data = max(
        (PROJECT / "Library/Bee").glob("Player*-inputdata.json"),
        key=lambda path: path.stat().st_mtime_ns,
    )
    build_inputs = json.loads(input_data.read_text())
    config = build_inputs["PlayerBuildProgramLibrary.Data.PlayerBuildConfig"]
    plugins = build_inputs["PlayerBuildProgramLibrary.Data.PluginsData"]["Plugins"]
    require(config["StreamingAssetsFiles"] == [], "build metadata has StreamingAssets")
    require(config["Development"] is False, "qualified player is a development build")
    require(
        all(value is False for value in config["Services"].values()),
        "Unity online service is enabled",
    )
    require(
        plugins == [{
            "AssetPath": "Assets/Plugins/x86_64/libsim_capi.so",
            "DestinationPath": "x86_64/libsim_capi.so",
            "AddToEmbeddedBinaries": False,
        }],
        "unexpected native plug-in closure",
    )
    return len(files), len(owner_hashes)


def clean_environment() -> dict[str, str]:
    allowed = ("HOME", "PATH", "XDG_RUNTIME_DIR", "LANG", "LC_ALL")
    return {name: os.environ[name] for name in allowed if name in os.environ}


def launch(
    arguments: list[str],
    expected_reason: str,
    *,
    graphical: bool = False,
) -> None:
    with tempfile.NamedTemporaryFile(
        prefix="magiccarpet-player-negative-", suffix=".log", delete=False
    ) as stream:
        log_path = Path(stream.name)
    command = [str(PLAYER)]
    if not graphical:
        command.extend(["-batchmode", "-nographics"])
    command.extend(["-mc-qualified", *arguments, "-logFile", str(log_path)])
    if graphical:
        command = [
            "xvfb-run", "-a", "-s", "-screen 0 800x600x24", *command
        ]
    try:
        result = subprocess.run(
            command,
            cwd=PROJECT,
            env=clean_environment(),
            text=True,
            capture_output=True,
            timeout=12,
        )
        log = log_path.read_text(encoding="utf-8")
    finally:
        log_path.unlink(missing_ok=True)
    require(result.returncode == 2, f"negative player exit was {result.returncode}")
    require("MC_FATAL " in log and expected_reason in log,
            f"negative player reason missing: {expected_reason}\n{log[-4000:]}")
    require("MC_SELF_TEST PASS" not in log, "negative player emitted a PASS marker")


def verify_positive_player() -> None:
    with tempfile.NamedTemporaryFile(
        prefix="magiccarpet-player-positive-", suffix=".log", delete=False
    ) as stream:
        log_path = Path(stream.name)
    command = [
        str(PLAYER),
        "-batchmode",
        "-nographics",
        "-mc-qualified",
        "-mc-self-test",
        "-mc-package",
        str(PACKAGE),
        "-mc-closure",
        str(CLOSURE),
        "-mc-asset-cache",
        str(CACHE),
        "-logFile",
        str(log_path),
    ]
    try:
        result = subprocess.run(
            command,
            cwd=PROJECT,
            env=clean_environment(),
            text=True,
            capture_output=True,
        )
        log = log_path.read_text(encoding="utf-8")
    finally:
        log_path.unlink(missing_ok=True)
    require(result.returncode == 0, "qualified built-player self-test failed")
    for marker in (
        "MC_ASSET_CACHE VERIFIED outputs=293 ",
        "MC_IDLE_QUAL PASS ",
        "MC_ROUTE_QUAL PASS checkpoints=5 steps=440 ",
        "MC_SELF_TEST PASS abi=2 ",
    ):
        require(marker in log, f"built-player marker missing: {marker}")
    require("MC_FATAL " not in log and "MC_RUNTIME_FATAL " not in log,
            "built-player self-test logged a fatal error")


def copy_private_tree(source: Path, destination: Path) -> None:
    shutil.copytree(source, destination, copy_function=shutil.copy2)


def verify_negative_player_paths() -> int:
    launch([], "Owner data is never bundled")
    common = [
        "-mc-package", str(PACKAGE),
        "-mc-closure", str(CLOSURE),
    ]
    launch(common, "qualified mode requires the exact owner asset cache")
    launch([], "Owner data is never bundled", graphical=True)

    with tempfile.TemporaryDirectory(prefix="magiccarpet-player-tamper-") as temp:
        root = Path(temp)

        package = root / "package"
        copy_private_tree(PACKAGE, package)
        entities = package / "entities.bin"
        data = bytearray(entities.read_bytes())
        data[0] ^= 1
        entities.write_bytes(data)
        launch([
            "-mc-package", str(package),
            "-mc-closure", str(CLOSURE),
            "-mc-asset-cache", str(CACHE),
        ], "entities.bin hash mismatch")

        closure = root / "closure.json"
        closure.write_text("{}\n", encoding="utf-8")
        launch([
            "-mc-package", str(PACKAGE),
            "-mc-closure", str(closure),
            "-mc-asset-cache", str(CACHE),
        ], "closure is missing schema")

        cache = root / "cache"
        copy_private_tree(CACHE, cache)
        cache_manifest = json.loads((cache / "manifest.json").read_text())
        output = cache / cache_manifest["outputs"][0]["path"]
        output_data = bytearray(output.read_bytes())
        output_data[0] ^= 1
        output.write_bytes(output_data)
        launch([
            "-mc-package", str(PACKAGE),
            "-mc-closure", str(CLOSURE),
            "-mc-asset-cache", str(cache),
        ], "cache output hash mismatch")

        extra_cache = root / "extra-cache"
        copy_private_tree(CACHE, extra_cache)
        (extra_cache / "unexpected.bin").write_bytes(b"not owner data")
        launch([
            "-mc-package", str(PACKAGE),
            "-mc-closure", str(CLOSURE),
            "-mc-asset-cache", str(extra_cache),
        ], "owner asset cache tree mismatch")
    return 7


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--edit-results", type=Path, required=True)
    parser.add_argument("--edit-log", type=Path, required=True)
    parser.add_argument("--play-results", type=Path, required=True)
    parser.add_argument("--play-log", type=Path, required=True)
    args = parser.parse_args()

    corpus = json.loads(CORPUS_PATH.read_text())
    allocated = verify_test_evidence(args)
    surface_files, registered_hashes = verify_build_surface(corpus)
    verify_positive_player()
    negative_cases = verify_negative_player_paths()
    print(
        "MC_UNITY_LIFECYCLE_PRIVACY PASS "
        "editMode=6/6 playMode=1/1 "
        f"bulkAllocatedBytes={allocated} "
        f"surfaceFiles={surface_files} "
        f"registeredOwnerHashes={registered_hashes} "
        f"negativeCases={negative_cases} "
        "streamingAssets=0 symlinks=0 ownerPayloadIdentityIntersections=0 "
        "interactiveFailureExit=2"
    )


if __name__ == "__main__":
    main()
