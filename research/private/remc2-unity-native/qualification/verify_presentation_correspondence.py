#!/usr/bin/env python3
"""Verify the private Level 1 structural presentation comparison."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
CORPUS_PATH = HERE / "level1-reference-corpus.json"
CONTRACT_PATH = HERE / "presentation-correspondence.json"
RUNTIME = REPO / "research/private/remc2-unity-native"
CACHE = REPO / "research/private/generated/level1-unity-asset-cache"
PACKAGE_SHA256 = "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8"
CLOSURE_SHA256 = "1c3774ca5c032c1a7b8b1c48870840eee33011a2830d08c51f3104fb5c5bcc12"
CLASSIFICATIONS = ("exact", "frozen", "adapted", "omitted", "deferred")
REQUIRED_LAYER_IDS = {
    "original.framebuffer.indexed",
    "owner.palette.base",
    "native.terrain.indexed",
    "unity.route.pngs",
    "actors.reset_observed",
    "hud.objective.reset",
    "hud.owner_layout.reset",
    "original.review_decode.base_paln",
    "unity.composition.1600x900",
    "camera.unity",
    "actors.billboard_geometry",
    "hud.uniform_transform",
    "minimap.mixed_authority",
    "audio.unity_playback",
    "projectile.debug_proxy",
    "carpet.approximate_proxy",
    "diagnostic.presentation",
    "sprite.source_hotspot_anchor",
    "renderer.dos_depth_interleaving_aspect",
    "palette.transient_runtime",
    "hud.hsprn_and_live_progression",
    "projectile.presentation",
    "simulation.ai_spells_creatures_progression",
}
REQUIRED_LAYER_AUTHORITIES = {
    "original.framebuffer.indexed": "original-remc2-hook-002285ff",
    "owner.palette.base": "owner-cache",
    "native.terrain.indexed": "unity-synthetic-route",
    "unity.route.pngs": "unity-synthetic-route",
    "actors.reset_observed": "unity-synthetic-route",
    "hud.objective.reset": "owner-cache",
    "hud.owner_layout.reset": "owner-cache",
    "original.review_decode.base_paln": "original-remc2-hook-002285ff",
    "unity.composition.1600x900": "unity-synthetic-route",
    "camera.unity": "unity-synthetic-route",
    "actors.billboard_geometry": "controlled-debug-gallery",
    "hud.uniform_transform": "unity-synthetic-route",
    "minimap.mixed_authority": "unity-synthetic-route",
    "audio.unity_playback": "unity-synthetic-route",
    "projectile.debug_proxy": "unity-synthetic-route",
    "carpet.approximate_proxy": "unity-synthetic-route",
    "diagnostic.presentation": "unity-synthetic-route",
    "sprite.source_hotspot_anchor": "owner-cache",
    "renderer.dos_depth_interleaving_aspect": "original-remc2-hook-002285ff",
    "palette.transient_runtime": "original-remc2-hook-002285ff",
    "hud.hsprn_and_live_progression": "owner-cache",
    "projectile.presentation": "unity-synthetic-route",
    "simulation.ai_spells_creatures_progression": "unity-synthetic-route",
}
EXACT_LAYER_TARGETS = {
    "original.framebuffer.indexed": "pinned remc2 regression fixture slices",
    "owner.palette.base": "content-addressed owner cache",
    "native.terrain.indexed": "same native build route registry",
    "unity.route.pngs": "two independent same-build Unity runs",
}
REQUIRED_EXACT_NOT_CLAIMED = {
    "original.framebuffer.indexed": {"active DAC RGB", "Unity pixel equality"},
    "owner.palette.base": {"per-frame active DAC palette"},
    "native.terrain.indexed": {"original full-frame pixel equality"},
    "unity.route.pngs": {"remc2 pixel equality", "remc2 clock equality"},
}
REQUIRED_LAYER_CLASSIFICATIONS = {
    "original.framebuffer.indexed": "exact",
    "owner.palette.base": "exact",
    "native.terrain.indexed": "exact",
    "unity.route.pngs": "exact",
    "actors.reset_observed": "adapted",
    "hud.objective.reset": "frozen",
    "hud.owner_layout.reset": "frozen",
    "original.review_decode.base_paln": "adapted",
    "unity.composition.1600x900": "adapted",
    "camera.unity": "adapted",
    "actors.billboard_geometry": "adapted",
    "hud.uniform_transform": "adapted",
    "minimap.mixed_authority": "adapted",
    "audio.unity_playback": "adapted",
    "projectile.debug_proxy": "omitted",
    "carpet.approximate_proxy": "omitted",
    "diagnostic.presentation": "omitted",
    "sprite.source_hotspot_anchor": "deferred",
    "renderer.dos_depth_interleaving_aspect": "deferred",
    "palette.transient_runtime": "deferred",
    "hud.hsprn_and_live_progression": "deferred",
    "projectile.presentation": "deferred",
    "simulation.ai_spells_creatures_progression": "deferred",
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def resolve_private_directory(raw: Path, label: str) -> Path:
    require(not raw.is_symlink(), f"unsafe {label} symlink")
    directory = raw.resolve(strict=True)
    temporary_root = Path(tempfile.gettempdir()).resolve()
    require(
        directory != temporary_root and temporary_root in directory.parents,
        f"{label} must be a dedicated child of {temporary_root}",
    )
    require(directory.is_dir() and not directory.is_symlink(), f"unsafe {label}")
    return directory


def resolve_private_file(raw: Path, label: str) -> Path:
    require(not raw.is_symlink(), f"unsafe {label} symlink")
    path = raw.resolve(strict=True)
    temporary_root = Path(tempfile.gettempdir()).resolve()
    require(
        temporary_root in path.parents,
        f"{label} must be below {temporary_root}",
    )
    require(path.is_file() and not path.is_symlink(), f"unsafe {label}")
    return path


def run_gate(name: str, command: list[str], marker: str) -> None:
    result = subprocess.run(
        command,
        cwd=REPO,
        text=True,
        capture_output=True,
    )
    output = result.stdout + result.stderr
    require(
        result.returncode == 0,
        f"{name} gate failed with exit {result.returncode}\n{output}",
    )
    require(marker in output, f"{name} PASS marker missing\n{output}")
    print(f"MC_PRESENTATION_GATE PASS name={name}")


def cross_timeline_claim_counts(contract: dict) -> tuple[int, int]:
    pixel_claims = 0
    clock_claims = 0
    pairing = contract["pairing"]
    if pairing["one_to_one_pairs"]:
        pixel_claims += len(pairing["one_to_one_pairs"])
        clock_claims += len(pairing["one_to_one_pairs"])
    expected_pairing_rule = (
        "the timelines are displayed independently; no reference frame is "
        "paired with a Unity route checkpoint"
    )
    if pairing["rule"] != expected_pairing_rule:
        lowered = pairing["rule"].lower()
        if (
            "unity" in lowered
            and ("remc2" in lowered or "original" in lowered)
            and any(token in lowered for token in ("tick", "frame", "clock", "turn"))
        ):
            clock_claims += 1
        if (
            "unity" in lowered
            and ("remc2" in lowered or "original" in lowered)
            and any(token in lowered for token in ("pixel", "framebuffer", "rgb", "image"))
        ):
            pixel_claims += 1
    for comparison in contract["comparisons"]:
        if comparison["checkpoint_pairs"]:
            pixel_claims += len(comparison["checkpoint_pairs"])
            clock_claims += len(comparison["checkpoint_pairs"])
        policy = comparison["comparison_policy"].lower()
        if "pixel-equality" in policy:
            pixel_claims += 1
        if "clock-equality" in policy:
            clock_claims += 1
    for layer in contract["layers"]:
        if layer["classification"] != "exact":
            continue
        text = " ".join(
            str(layer.get(field, ""))
            for field in ("exact_domain", "exact_against")
        ).lower()
        tokens = set(re.findall(r"[a-z0-9_-]+", text))
        authority = layer["authority_domain"]
        if authority == "unity-synthetic-route":
            crosses_domain = any(
                token in text for token in ("remc2", "original", "dos")
            )
        elif authority == "original-remc2-hook-002285ff":
            crosses_domain = "unity" in text
        else:
            crosses_domain = False
        if not crosses_domain:
            continue
        if tokens.intersection({"pixel", "pixels", "framebuffer", "rgb", "image"}):
            pixel_claims += 1
        if tokens.intersection({"tick", "frame", "clock", "turn"}):
            clock_claims += 1
    return pixel_claims, clock_claims


def verify_contract(contract: dict) -> None:
    require(
        contract["schema"] == "mc2-private-level1-presentation-correspondence/v1",
        "presentation contract schema mismatch",
    )
    require(
        contract["comparison_mode"]
        == "structural_only_independent_clocks_no_pixel_equivalence",
        "presentation comparison mode overclaims equivalence",
    )
    require(
        contract["pairing"]["one_to_one_pairs"] == []
        and contract["pairing"]["rule"]
        == (
            "the timelines are displayed independently; no reference frame is "
            "paired with a Unity route checkpoint"
        ),
        "one-to-one original/Unity pairing is forbidden",
    )
    require(
        contract["original_timeline"]["display_decode"]["active_dac_palette"]
        == "not captured at these checkpoints",
        "active DAC palette must remain explicitly uncaptured",
    )
    timelines = contract["timelines"]
    timeline_ids = [timeline["timeline_id"] for timeline in timelines]
    require(
        timeline_ids == [
            "original-remc2-hook-002285ff",
            "unity-synthetic-route",
            "controlled-debug-gallery",
            "owner-cache",
        ],
        "typed timeline registry changed or is duplicated",
    )
    original_timeline = timelines[0]
    unity_timeline = timelines[1]
    gallery_timeline = timelines[2]
    cache_timeline = timelines[3]
    require(
        original_timeline["clock_domain"] == "reference_frame"
        and [row["checkpoint_id"] for row in original_timeline["checkpoints"]]
        == contract["original_timeline"]["checkpoint_ids"],
        "typed original timeline mismatch",
    )
    require(
        unity_timeline["clock_domain"] == "unity_checkpoint_tick"
        and [row["checkpoint_id"] for row in unity_timeline["checkpoints"]]
        == contract["unity_timeline"]["checkpoint_ids"],
        "typed Unity timeline mismatch",
    )
    require(
        gallery_timeline["clock_domain"] == "none"
        and gallery_timeline["simulation_mutation"] is False
        and gallery_timeline["checkpoints"] == [],
        "controlled gallery must remain clockless and non-mutating",
    )
    require(
        cache_timeline["clock_domain"] == "timeless"
        and cache_timeline["checkpoints"] == [],
        "owner cache must remain a timeless content authority",
    )
    gallery = contract["controlled_gallery"]
    require(
        gallery["authority"] == "controlled-debug-gallery"
        and gallery["simulation_mutation"] is False
        and gallery["qualified_view"] is False
        and gallery["selectors"] == [79, 152, 154, 154, 155, 180, 183, 199]
        and gallery["views"] == 8,
        "controlled gallery authority/selectors changed",
    )

    layers = contract["layers"]
    layer_ids = [layer["id"] for layer in layers]
    require(
        len(layer_ids) == len(set(layer_ids))
        and set(layer_ids) == REQUIRED_LAYER_IDS,
        "typed presentation layers are incomplete or duplicated",
    )
    for layer in layers:
        classification = layer["classification"]
        require(
            classification in CLASSIFICATIONS,
            f"{layer['id']}: unknown classification {classification}",
        )
        require(
            classification == REQUIRED_LAYER_CLASSIFICATIONS[layer["id"]],
            f"{layer['id']}: classification mismatch",
        )
        require(
            layer["authority_domain"] == REQUIRED_LAYER_AUTHORITIES[layer["id"]],
            f"{layer['id']}: authority domain mismatch",
        )
        require(
            layer["comparison_policy"] in {
                "within-timeline-identity",
                "structural-correspondence",
                "side-by-side-non-equivalence",
                "absence-only",
                "deferred-no-pixels",
            },
            f"{layer['id']}: invalid comparison policy",
        )
        require(
            isinstance(layer["evidence"], list)
            and isinstance(layer["not_claimed"], list),
            f"{layer['id']}: evidence/claim limits must be typed lists",
        )
        if classification == "exact":
            require(
                layer["present"] is True
                and layer.get("exact_domain")
                and layer.get("exact_against"),
                f"{layer['id']}: exact requires a bounded domain and target",
            )
            require(
                layer["exact_against"] == EXACT_LAYER_TARGETS[layer["id"]],
                f"{layer['id']}: exact target crosses or changes authority domain",
            )
            require(
                REQUIRED_EXACT_NOT_CLAIMED[layer["id"]].issubset(
                    set(layer["not_claimed"])
                ),
                f"{layer['id']}: exact claim limits are incomplete",
            )
        elif classification == "frozen":
            require(
                layer["present"] is True
                and layer.get("source_checkpoint")
                and layer.get("immutability_gate"),
                f"{layer['id']}: frozen requires source and immutability gate",
            )
        elif classification == "adapted":
            require(
                layer["present"] is True
                and layer.get("transform")
                and layer.get("not_equivalent_to"),
                f"{layer['id']}: adapted requires transform and non-equivalence",
            )
        elif classification == "omitted":
            require(
                layer["present"] is False
                and layer.get("replacement") == "none"
                and layer.get("absence_gate"),
                f"{layer['id']}: omitted requires no replacement and an absence gate",
            )
        elif classification == "deferred":
            require(
                layer["present"] is False
                and layer.get("reason")
                and layer.get("follow_on")
                and layer["comparison_policy"] == "deferred-no-pixels",
                f"{layer['id']}: deferred requires reason/follow-on and no pixels",
            )
            require(
                not any(
                    any(token in evidence.lower() for token in ("png", "rgba", "pixel"))
                    for evidence in layer["evidence"]
                ),
                f"{layer['id']}: deferred layer carries pixel evidence",
            )
    hotspot = next(
        layer for layer in layers if layer["id"] == "sprite.source_hotspot_anchor"
    )
    review_decode = next(
        layer for layer in layers if layer["id"] == "original.review_decode.base_paln"
    )
    require(
        hotspot["classification"] == "deferred" and hotspot["present"] is False,
        "source hotspot/anchor cannot be claimed exact",
    )
    require(
        review_decode["classification"] == "adapted"
        and "exact displayed RGB" in review_decode["not_claimed"],
        "base-PALN review decode cannot be claimed as exact displayed RGB",
    )

    comparison_ids: set[str] = set()
    for comparison in contract["comparisons"]:
        require(
            comparison["comparison_id"] not in comparison_ids,
            "duplicate comparison id",
        )
        comparison_ids.add(comparison["comparison_id"])
        require(
            comparison["left_timeline"] != comparison["right_timeline"]
            and comparison["comparison_policy"] in {
                "structural-correspondence",
                "side-by-side-non-equivalence",
            }
            and comparison["checkpoint_pairs"] == [],
            f"{comparison['comparison_id']}: cross-timeline equivalence is forbidden",
        )
    require(
        comparison_ids == {
            "original-vs-unity-structural",
            "owner-cache-to-unity-structural",
        },
        "cross-timeline comparison registry mismatch",
    )
    forbidden_claims = set(contract["forbidden_claims"])
    require(
        {
            "cross-timeline pixel-equality",
            "cross-timeline clock-equality",
            "implicit checkpoint pairing",
            "base-PALN review decode is exact active-DAC RGB",
            "adapted billboard pivot is an exact source hotspot",
        }.issubset(forbidden_claims),
        "required forbidden claims are missing",
    )
    require(
        set(contract["required_gates"]) == {
            "reference_corpus",
            "owner_asset_cache",
            "qualified_full_frames",
            "owner_hud",
            "entity_gallery",
            "no_magenta",
            "no_placeholder_claim",
            "independent_timeline_labels",
        },
        "required presentation gates changed",
    )
    pixel_claims, clock_claims = cross_timeline_claim_counts(contract)
    require(
        pixel_claims == 0,
        f"cross-timeline pixel claims detected: {pixel_claims}",
    )
    require(
        clock_claims == 0,
        f"cross-timeline clock claims detected: {clock_claims}",
    )

    expected_surfaces = {
        "palette",
        "terrain_material",
        "tmap_actors",
        "sprite_hotspot",
        "hud",
        "minimap",
        "objective",
        "audio",
    }
    surfaces = contract["surfaces"]
    require(
        {surface["id"] for surface in surfaces} == expected_surfaces
        and len(surfaces) == len(expected_surfaces),
        "presentation surface registry is incomplete or duplicated",
    )
    for surface in surfaces:
        seen: set[str] = set()
        for classification in CLASSIFICATIONS:
            values = surface[classification]
            require(
                isinstance(values, list)
                and all(isinstance(value, str) and value for value in values),
                f"{surface['id']}: invalid {classification} rows",
            )
            require(
                not seen.intersection(values),
                f"{surface['id']}: a claim appears in multiple classifications",
            )
            seen.update(values)


def verify_no_qualified_placeholders(contract: dict) -> None:
    forbidden = set(contract["forbidden_placeholder_selectors"])
    require(forbidden == set(range(426, 437)), "placeholder selector register changed")
    cache_manifest = json.loads((CACHE / "manifest.json").read_text())
    entity_descriptor = json.loads(
        (CACHE / cache_manifest["assets"]["entity_presentations"]["path"]).read_text()
    )
    qualified_selectors: set[int] = set()
    for group in entity_descriptor["groups"]:
        if group["qualified_view_count"] > 0:
            qualified_selectors.update(group["selector"]["qualified_selectors"])
    require(
        not forbidden.intersection(qualified_selectors),
        "placeholder selector entered the qualified actor view",
    )
    hud_descriptor = json.loads(
        (CACHE / cache_manifest["assets"]["level1_hud"]["path"]).read_text()
    )
    hud_selectors = {row["selector"] for row in hud_descriptor["bitmap_assets"]}
    require(
        not forbidden.intersection(hud_selectors),
        "placeholder selector entered the qualified HUD",
    )


def verify_runtime_logs(
    unity_logs: list[Path],
    gallery_log: Path,
    corpus: dict,
) -> None:
    expected_autoplay_tokens = [
        "tick=440",
        "fire=2",
        "registry=177",
        "ownerSprites=64",
        "selectorMaterials=16",
        "presentationClean=True",
        "physicsClean=True",
        "terrainFrameClean=True",
        "carpetOverlayClean=True",
        "firePresentationClean=True",
        "diagnosticObjectsClean=True",
        "captures=5/5",
        "magentaClean=5/5",
        "captureFilesReady=True",
    ]
    expected_structural_lines = [
        (
            "MC_ASSET_CACHE VERIFIED outputs=293 "
            f"content={corpus['runtime_pins']['owner_cache_content_sha256']}"
        ),
        "MC_HUD_DESCRIPTOR VERIFIED bitmaps=11 bank=MSPRN-scale2 objective=48/",
        (
            "MC_AUDIO VERIFIED bank=1 quality=822 refs=10 "
            "music=C2GAME1-adlmidi-bank59.wav"
        ),
        (
            "MC_ENTITY_PRESENTATION VERIFIED registry=177 qualifiedSprites=64 "
            "goatLive=33 frozenOwnerSprites=31 resetEffectDebug=0 "
            "unresolvedDebug=0 mode=qualified"
        ),
        "MC_OWNER_HUD READY textures=11 objective=selector48/",
    ]
    for log_path in unity_logs:
        log = log_path.read_text(encoding="utf-8")
        pass_lines = [
            line for line in log.splitlines()
            if line.startswith("MC_QUALIFIED_AUTOPLAY PASS ")
        ]
        require(
            len(pass_lines) == 1,
            f"{log_path.name}: expected exactly one qualified autoplay PASS",
        )
        for token in expected_autoplay_tokens:
            require(
                token in pass_lines[0],
                f"{log_path.name}: missing qualified clean token {token}",
            )
        for marker in expected_structural_lines:
            require(marker in log, f"{log_path.name}: missing marker {marker}")
        require(
            log.count(
                "visual_suppressed sequence="
            ) == 2
            and log.count(
                "reason=projectile-presentation-not-qualified"
            ) == 2,
            f"{log_path.name}: projectile omission evidence mismatch",
        )

    gallery = gallery_log.read_text(encoding="utf-8")
    require(
        gallery.count("MC_ENTITY_GALLERY VERIFIED ") == 1
        and (
            "items=8 dolmen=79 goat=152,154,154-mirror "
            "villagers=155,180,183,199 neutralBackground=true "
            "simulationMutation=false noPhysics=true"
        ) in gallery,
        "controlled gallery identity/non-mutation marker mismatch",
    )
    require(
        gallery.count("MC_ENTITY_GALLERY PASS ") == 1
        and "items=8 selectorMaterials=14 captured=True physicsClean=True" in gallery,
        "controlled gallery PASS mismatch",
    )


def verify_controlled_gallery(
    contract: dict,
    corpus: dict,
    gallery_path: Path,
) -> None:
    gallery = contract["controlled_gallery"]
    pins = corpus["runtime_pins"]
    require(
        gallery["source_player_sha256"] == pins["unity_player_file_sha256"]
        and gallery["owner_cache_content_sha256"]
        == pins["owner_cache_content_sha256"],
        "controlled gallery is not bound to the qualified player/cache",
    )
    require(
        sha256_file(gallery_path) == gallery["png_sha256"],
        "controlled gallery PNG identity mismatch",
    )


def verify_materialized_evidence(
    evidence_directory: Path,
    unity_captures: Path,
    corpus: dict,
    contract: dict,
) -> None:
    report_path = evidence_directory / "presentation-correspondence-evidence.json"
    html_path = evidence_directory / "presentation-correspondence.html"
    require(report_path.is_file() and not report_path.is_symlink(), "missing evidence JSON")
    require(html_path.is_file() and not html_path.is_symlink(), "missing evidence HTML")
    expected_files = {
        "presentation-correspondence-evidence.json",
        "presentation-correspondence.html",
        *{
            f"{checkpoint_id}-base-paln.png"
            for checkpoint_id in contract["original_timeline"]["checkpoint_ids"]
        },
        *{
            f"{checkpoint_id}.png"
            for checkpoint_id in contract["unity_timeline"]["checkpoint_ids"]
        },
    }
    actual_files = {
        path.name for path in evidence_directory.iterdir() if path.is_file()
    }
    require(
        actual_files in (
            expected_files,
            expected_files | {"presentation-qualification-result.json"},
        ),
        "unexpected or missing presentation evidence artifacts",
    )
    require(
        all(path.is_file() and not path.is_symlink() for path in evidence_directory.iterdir()),
        "presentation evidence contains a directory or symlink",
    )
    report = json.loads(report_path.read_text())
    require(report["schema"] == contract["schema"], "evidence schema mismatch")
    require(
        report["comparison_mode"] == contract["comparison_mode"],
        "evidence comparison mode mismatch",
    )
    require(report["one_to_one_pairs"] == [], "materialized evidence implies pairing")
    require(
        report["controlled_gallery"] == contract["controlled_gallery"],
        "controlled gallery registry changed",
    )
    require(report["timelines"] == contract["timelines"], "typed timelines changed")
    require(report["layers"] == contract["layers"], "typed layers changed")
    require(report["comparisons"] == contract["comparisons"], "comparisons changed")
    require(
        report["forbidden_claims"] == contract["forbidden_claims"],
        "forbidden-claim registry changed",
    )
    require(report["surfaces"] == contract["surfaces"], "evidence matrix changed")
    require(report["corpus_sha256"] == sha256_file(CORPUS_PATH), "corpus identity mismatch")
    require(
        report["contract_sha256"] == sha256_file(CONTRACT_PATH),
        "presentation contract identity mismatch",
    )
    pins = corpus["runtime_pins"]
    require(
        report["package_sha256"] == pins["package_identity_sha256"]
        and report["closure_sha256"] == pins["closure_identity_sha256"]
        and report["owner_cache_content_sha256"] == pins["owner_cache_content_sha256"]
        and report["unity_build_guid"] == pins["unity_build_guid"],
        "materialized runtime identities mismatch",
    )

    expected_original = contract["original_timeline"]["derived_png_sha256"]
    require(
        [row["checkpoint_id"] for row in report["original_timeline"]]
        == contract["original_timeline"]["checkpoint_ids"],
        "original timeline order mismatch",
    )
    for row in report["original_timeline"]:
        require(
            row["decode_authority"] == "base-PALN structural decode",
            f"{row['checkpoint_id']}: RGB authority overclaimed",
        )
        png_path = evidence_directory / row["png_file"]
        require(
            png_path.is_file()
            and sha256_file(png_path) == row["png_sha256"]
            == expected_original[row["checkpoint_id"]],
            f"{row['checkpoint_id']}: derived PNG mismatch",
        )

    unity_suite = next(
        item for item in corpus["suites"] if item["id"] == contract["unity_suite"]
    )
    checkpoints = {item["checkpoint_id"]: item for item in unity_suite["checkpoints"]}
    require(
        [row["checkpoint_id"] for row in report["unity_timeline"]]
        == contract["unity_timeline"]["checkpoint_ids"],
        "Unity timeline order mismatch",
    )
    for row in report["unity_timeline"]:
        checkpoint = checkpoints[row["checkpoint_id"]]
        tick = checkpoint["unity_checkpoint_tick"]
        source_png = unity_captures / f"magiccarpet-autoplay-{tick:03d}.png"
        copied_png = evidence_directory / row["png_file"]
        expected_hash = checkpoint["unity_frame_identity"]["png_sha256"]
        require(
            row["authority"] == "synthetic-native-unity-regression"
            and row["unity_checkpoint_tick"] == tick
            and row["native_render_tick"]
            == checkpoint["camera_identity"]["render"]["tick"],
            f"{row['checkpoint_id']}: Unity clock/authority mismatch",
        )
        require(
            sha256_file(source_png) == sha256_file(copied_png)
            == row["png_sha256"] == expected_hash,
            f"{row['checkpoint_id']}: Unity PNG mismatch",
        )

    page = html_path.read_text()
    require(
        "Independent clocks. No one-to-one frame pairing" in page
        and "active-DAC RGB claim" in page
        and "pixel-equality claim" in page,
        "HTML equivalence warning missing",
    )
    for checkpoint_id in (
        contract["original_timeline"]["checkpoint_ids"]
        + contract["unity_timeline"]["checkpoint_ids"]
    ):
        require(checkpoint_id in page, f"HTML timeline missing {checkpoint_id}")
    for surface in contract["surfaces"]:
        require(surface["id"] in page, f"HTML matrix missing {surface['id']}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("evidence_directory", type=Path)
    parser.add_argument("--unity-captures", required=True, type=Path)
    parser.add_argument("--second-unity-captures", required=True, type=Path)
    parser.add_argument("--unity-log-a", required=True, type=Path)
    parser.add_argument("--unity-log-b", required=True, type=Path)
    parser.add_argument("--entity-gallery", required=True, type=Path)
    parser.add_argument("--gallery-log", required=True, type=Path)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    evidence_directory = resolve_private_directory(
        args.evidence_directory, "evidence directory"
    )
    unity_captures = resolve_private_directory(
        args.unity_captures, "Unity capture directory"
    )
    second_unity_captures = resolve_private_directory(
        args.second_unity_captures, "second Unity capture directory"
    )
    entity_gallery = resolve_private_file(args.entity_gallery, "entity gallery")
    unity_logs = [
        resolve_private_file(args.unity_log_a, "Unity log A"),
        resolve_private_file(args.unity_log_b, "Unity log B"),
    ]
    gallery_log = resolve_private_file(args.gallery_log, "entity gallery log")
    corpus = json.loads(CORPUS_PATH.read_text())
    contract = json.loads(CONTRACT_PATH.read_text())

    verify_contract(contract)
    verify_no_qualified_placeholders(contract)
    verify_runtime_logs(unity_logs, gallery_log, corpus)
    verify_controlled_gallery(contract, corpus, entity_gallery)
    verify_materialized_evidence(
        evidence_directory, unity_captures, corpus, contract
    )

    run_gate(
        "reference_corpus",
        [sys.executable, str(HERE / "verify_reference_corpus.py")],
        "MC_LEVEL1_REFERENCE_CORPUS PASS",
    )
    run_gate(
        "owner_asset_cache",
        [
            sys.executable,
            str(REPO / "research/private/remc2-unity-asset-cache/build_cache.py"),
            "validate",
            "--cache",
            str(CACHE),
            "--package-sha256",
            PACKAGE_SHA256,
            "--closure-sha256",
            CLOSURE_SHA256,
        ],
        "valid schema=mc2-owner-unity-asset-cache/v1",
    )
    full_frame_command = [
        sys.executable,
        str(RUNTIME / "verify_qualified_full_frames.py"),
        str(unity_captures),
    ]
    full_frame_command.append(str(second_unity_captures))
    run_gate(
        "qualified_full_frames",
        full_frame_command,
        "MC_QUALIFIED_FULL_FRAMES PASS",
    )
    run_gate(
        "owner_hud",
        [
            sys.executable,
            str(RUNTIME / "verify_owner_hud.py"),
            str(unity_captures / "magiccarpet-autoplay-145.png"),
            "--comparison",
            str(unity_captures / "magiccarpet-autoplay-400.png"),
            "--json",
        ],
        '"status": "PASS"',
    )
    run_gate(
        "entity_gallery",
        [
            sys.executable,
            str(RUNTIME / "verify_entity_gallery.py"),
            str(entity_gallery),
            "--cache",
            str(CACHE),
            "--json",
        ],
        '"status": "PASS"',
    )
    pixel_claims, clock_claims = cross_timeline_claim_counts(contract)
    result = {
        "schema": "mc2-private-level1-presentation-qualification-result/v1",
        "status": "PASS",
        "original_indexed": 3,
        "original_review_decodes": 3,
        "unity_registered": 5,
        "unity_deterministic_runs": 2,
        "hud_source_bitmaps": 11,
        "hud_placements": 10,
        "owner_tmap_selectors": 16,
        "gallery_views": 8,
        "missing_owner_refs": 0,
        "magenta": 0,
        "placeholders": 0,
        "cross_timeline_pixel_claims": pixel_claims,
        "cross_timeline_clock_claims": clock_claims,
        "typed_layers": len(contract["layers"]),
        "typed_timelines": len(contract["timelines"]),
        "runtime_pins": corpus["runtime_pins"],
    }
    result_path = evidence_directory / "presentation-qualification-result.json"
    result_path.write_text(json.dumps(result, indent=2) + "\n")
    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    print(
        "MC_LEVEL1_PRESENTATION PASS "
        f"original_indexed={result['original_indexed']} "
        f"original_review_decodes={result['original_review_decodes']} "
        f"unity_registered={result['unity_registered']} "
        f"unity_deterministic_runs={result['unity_deterministic_runs']} "
        f"hud_source_bitmaps={result['hud_source_bitmaps']} "
        f"hud_placements={result['hud_placements']} "
        f"owner_tmap_selectors={result['owner_tmap_selectors']} "
        f"gallery_views={result['gallery_views']} missing_owner_refs=0 "
        "magenta=0 placeholders=0 "
        f"cross_timeline_pixel_claims={result['cross_timeline_pixel_claims']} "
        f"cross_timeline_clock_claims={result['cross_timeline_clock_claims']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
