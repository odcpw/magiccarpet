#!/usr/bin/env python3
"""Build the evidence-only CF-00 Omega inventory.

This tool deliberately consumes already-produced receipts.  It does not call
the exporter, mutate a Rust source tree, or run a game executable.  The two
M1 inputs are supplied explicitly so a later worker cannot accidentally bless
the stale checked-in M1 artifact or silently measure a dirty worktree.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


SCHEMA_VERSION = "campaign-fold-omega-v1"
PINNED_REMC2_REVISION = "6806c7204d17220ecc0430759f784aea8ee2ddb2"
DEFAULT_ROOT = Path(__file__).resolve().parents[5]
STALE_M1_HASHES = {
    "3e9da72893bb519e300fd3f88df928fa9f0ed31aaf8d89a220c892d7ebe26459",
    "691649822d8c3d41c1be64db07e7b15dbc56887822023ddd4d842fb830d37c36",
}


def _receipt_path(repo_root: Path, relative: str) -> Path:
    return repo_root / relative


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256_file(path: Path) -> str:
    return _sha256_bytes(path.read_bytes())


def _git(repo_root: Path, *args: str) -> str:
    return subprocess.check_output(
        ["git", "-C", str(repo_root), *args],
        stderr=subprocess.PIPE,
        text=True,
        timeout=30,
    ).strip()


def _resolve_commit(repo_root: Path, revision: str) -> str:
    return _git(repo_root, "rev-parse", "--verify", f"{revision}^{{commit}}")


def _git_file(repo_root: Path, revision: str, relative: str) -> bytes:
    return subprocess.check_output(
        ["git", "-C", str(repo_root), "show", f"{revision}:{relative}"],
        stderr=subprocess.PIPE,
        timeout=30,
    )


def _git_file_receipt(repo_root: Path, revision: str, relative: str) -> dict[str, str]:
    payload = _git_file(repo_root, revision, relative)
    return {"path": relative, "sha256": _sha256_bytes(payload)}


def _ancestor(repo_root: Path, candidate: str, evidence_revision: str) -> bool:
    return subprocess.run(
        ["git", "-C", str(repo_root), "merge-base", "--is-ancestor", candidate, evidence_revision],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
        timeout=30,
    ).returncode == 0


def _load_json(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as stream:
        value = json.load(stream)
    if not isinstance(value, dict):
        raise ValueError(f"expected JSON object at {path}")
    return value


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def _source_receipt(
    repo_root: Path,
    evidence_revision: str,
    bead_id: str,
    commit: str,
    files: list[str],
    *,
    accepted_revision: str | None = None,
) -> dict[str, Any]:
    full_commit = _resolve_commit(repo_root, commit)
    return {
        "bead_id": bead_id,
        "commit": full_commit,
        "present_at_evidence_revision": _ancestor(repo_root, full_commit, evidence_revision),
        "files": [_git_file_receipt(repo_root, full_commit, path) for path in files],
        **(
            {"accepted_revision": _resolve_commit(repo_root, accepted_revision)}
            if accepted_revision is not None
            else {}
        ),
    }


def _transition_receipt(
    repo_root: Path,
    evidence_revision: str,
    bead_id: str,
    title: str,
    commit: str,
    producer: str,
    consumer: str,
    scope: str,
) -> dict[str, Any]:
    full_commit = _resolve_commit(repo_root, commit)
    return {
        "bead_id": bead_id,
        "title": title,
        "commit": full_commit,
        "present_at_evidence_revision": _ancestor(repo_root, full_commit, evidence_revision),
        "ordinary_producer": producer,
        "following_pass_consumer": consumer,
        "scope": scope,
    }


# These are the closed, source-pinned behavior leaves that existed when CF-00
# was claimed.  Aggregate beads are intentionally represented by their leaf
# receipts once; presentation/cache-only leaves are not transition receipts.
# A receipt can be listed but not sealed when its claimed commit is not an
# ancestor of the explicit evidence revision.  That distinction is part of
# the boundary, not a reason to guess or silently move the evidence revision.
TRANSITION_RECEIPT_SPECS: tuple[tuple[str, str, str, str, str, str], ...] = (
    (
        "mc-2eby.3",
        "[L2] Carry source player-state byte 0x006 for type-3 banishment",
        "9f56e665e9bb5dc03d0de44e867e7a4db25aa43d",
        "sub_12A70 -> lethal sub_5E... banishment carrier",
        "following UpdateEntities_57730 cleanup, objective type-3 projection, and reset/hash consumer",
        "player/banishment state carrier",
    ),
    (
        "mc-2sbd.1",
        "[L2-15D.1] Wire Level 2 Goat action-11 into production reset and dispatcher",
        "074959439a04aef234b48616ef42c257e721b03b",
        "qualified Goat reset rows -> class-5 dispatcher action 11",
        "Goat action continuation, same-pass epilogue, and following-pass reclaim",
        "Goat dispatcher/reset carrier",
    ),
    (
        "mc-2sbd.2",
        "[L2-15D.2] Match Goat action-11 child-chain, movement, and linked-list semantics",
        "396601a9a7336b0b55b2df3254905c62ea4bf15d",
        "sub_1B8C0/sub_1BF90/sub_1C560/sub_1C980/sub_1EEE0 -> Goat actions 9-14",
        "same-pass Goat event epilogue and following-pass source-slot reclaim",
        "Goat child/movement/lifecycle chain",
    ),
    (
        "mc-38gs.1",
        "[L2-29A] Finish class-5 model-3 action-25 creature family",
        "50f6e4ff19bc90b28e0d918e7c9dca518fe82a94",
        "sub_4B6F0 -> class-5/model-3 action 25 root",
        "sixteen action-232 child visits, same-pass epilogue, following-pass reclaim",
        "class-5 model-3 child family",
    ),
    (
        "mc-38gs.2",
        "[L2-29B] Finish class-5 model-9 action 72 collision and retry semantics",
        "95635d35912d6e1d19577287f43fd98b075e88d0",
        "sub_4BBB0/sub_2AF10 -> class-5/model-9 action 72 and action 73 retry",
        "collision/retry continuation, same-pass hide, and following-pass cleanup",
        "class-5 model-9 collision family",
    ),
    (
        "mc-38gs.3",
        "[L2-29C] Finish class-5 model-4 archer construction and child-event chain",
        "0d989ae656ff3ffc2ddefc92861e682bb64a991b",
        "AddArchers_4BA10 -> archer constructor and child-event packet",
        "child callback/epilogue and following-pass source arena reclaim",
        "class-5 archer constructor/child chain",
    ),
    (
        "mc-38gs.4",
        "[L2-29D] Implement the reachable class-5 model-14 Trader family",
        "95635d35912d6e1d19577287f43fd98b075e88d0",
        "AddTrader_4C0B0/sub_237B0 -> class-5/model-14 actions",
        "mailbox/economy consumer, same-pass epilogue, and following-pass reclaim",
        "class-5 Trader family",
    ),
    (
        "mc-4ipu.1",
        "[L2-21A] Port sub_12910 dispatcher state byte and call-order semantics",
        "089c6d81a9ce2e9c3d58e57e9ba18c072c6aabf4",
        "sub_12910 dispatcher -> AI helper state-byte cases",
        "following source pass consumes state and the public Nyphur projection observes it",
        "Nyphur dispatcher state",
    ),
    (
        "mc-4ipu.2",
        "[L2-21B] Port basic Nyphur modes 0-2 with exact transitions and side effects",
        "50f31b763626294bc1c75ca0bda342faa0f78b01",
        "sub_12FF0/sub_12910 modes 0-1; mode 2 structural no-call",
        "following source pass, reset/replay hash, and Nyphur public projection",
        "Nyphur modes 0-2, with mode-2 exclusion",
    ),
    (
        "mc-50i6",
        "[L2-15A] Prove shared Day terrain, flight, and local-player behavior",
        "9821ad6b392855a0d61e0c9424977884456952d3",
        "qualified Day reset -> ordinary player flight/source pass",
        "next outer turn, terrain/state hash, reset, and replay consumer",
        "Day terrain/flight/local player",
    ),
    (
        "mc-app8.1",
        "[L2-20A] Port sub_12A70 common upkeep and exact auxiliary-state carriers",
        "f8e3828d7e600434a055953cfc51334b9c368210",
        "sub_12A70 -> common AI upkeep and auxiliary carrier",
        "next mode/outer pass consumes the carrier; following-pass reclaim preserves it",
        "AI common upkeep/auxiliary state",
    ),
    (
        "mc-app8.2",
        "[L2-20B] Port sub_12E70 shared movement tail, terrain clamps, and post-mode ordering",
        "5b1214eb2085f2c897550c5e62f930cf59e41b9d",
        "sub_12E70 -> shared movement/terrain tail",
        "post-mode continuation and following source pass consume ordered state",
        "AI movement tail",
    ),
    (
        "mc-bvihb.1",
        "[L2-15C-A1] Port Castle stage controller and source-chain side effects",
        "2ae0e3ebcfc1c8a89e889d488b455f7a77fbc37d",
        "Castle action roots and stage tables -> source Castle controller",
        "same-pass Castle event epilogue and following-pass child cleanup",
        "Castle stage controller",
    ),
    (
        "mc-bvihb.2",
        "[L2-22B] Finish Castle economy, mana collection, teardown, and allocator lifecycle",
        "a20a0a282ddfe1e77b43b3adef47fa54c30a4f1d",
        "AddPlayer03_00_5E010/EndOfCastleProjectile_5F8F0 -> Castle economy rows",
        "model-43/terminal epilogue and following-pass source arena reclaim",
        "Castle economy/teardown",
    ),
    (
        "mc-bvihb.4.1",
        "[L2-15C-A4.1] Interleave Level 2 source passes and preserve dispatch outcomes",
        "d8af8842dff7e668490df4ae46f6e74d9dee167f",
        "Level2ResetSession -> source dispatch before speed-index flight passes",
        "typed dispatch event consumer, same-pass error channel, and following source pass",
        "source-pass interleave",
    ),
    (
        "mc-bvihb.4.2",
        "[L2-15C-A4.2] Unify Level 2 dispatcher terrain and reset authority",
        "aeb0f4f36591d441e5741186d904fc08e5e7e93b",
        "outer source-pass dispatcher -> one terrain/reset authority",
        "following source pass and reset/replay/hash consumer",
        "dispatcher terrain/reset authority",
    ),
    (
        "mc-bvihb.4.3",
        "[L2-15C-A4.3] Dispatch dynamic Level 2 arena slots",
        "b737d04b43fee5b44b3ed1c40da75259e5f661e9",
        "NewEvent/shared arena -> dynamic source-slot dispatch",
        "same-pass dynamic cursor and following-pass generation/reclaim consumer",
        "dynamic arena dispatch",
    ),
    (
        "mc-bvihb.5",
        "[L2] Bind player Castle cast to mana/upkeep carrier",
        "c87abd03510f53d3b6cc9f528793000d046384a2",
        "requested spell -> sub_5F660 -> class-9/model-10 -> class-3/model-2 seed/root",
        "sub_60480 construction relation, model-43 next-pass reclaim, and upkeep consumer",
        "human Castle cast/construction",
    ),
    (
        "mc-bvihb.6",
        "[L2] Materialize isolated local starting-spell holders",
        "c87abd03510f53d3b6cc9f528793000d046384a2",
        "InitialiseSpells_54A50/sub_5CF40 -> class-15 holder rows",
        "shared-spell registry and following ordinary command consumer",
        "starting spell holder production",
    ),
    (
        "mc-bvihb.7",
        "[L2] Tier-0 Possession claims model-39 and feeds Castle economy",
        "17259940f6e5214ef49cf709173b56349cc77b72",
        "sub_69640/sub_69900 -> SummonManaPosession_4D3B0 -> model-39 mailbox",
        "model-39/action-41 ownership consumer, same-pass hide, following-pass reclaim",
        "Possession ownership/economy",
    ),
    (
        "mc-bvihb.8",
        "[L2] Route public family-0 Fireball into Day-Bee damage mailbox",
        "be739f7b99738b5382263722240f8655a6201654",
        "sub_693F0/sub_6DCA0 -> SummonFireball_4D2E0 -> Bee mailbox",
        "next Bee action-17 callback consumes damage; following-pass projectile cleanup",
        "Fireball/Bee damage mailbox",
    ),
    (
        "mc-clng",
        "[L2-17B] Finish Invisible spell end to end",
        "19a873a91f128435e99079b9ee97ebc5fa4f9721",
        "class-15 action 36 -> sub_6B310 -> tiered Invisible active state",
        "visibility expiry/terminal epilogue, following-pass cleanup, and C ABI/Unity projection",
        "Invisible spell lifecycle",
    ),
    (
        "mc-exzm.3",
        "[L2-36B1] Expose generation-bound Day indexed frame through campaign C ABI",
        "f8e3828d7e600434a055953cfc51334b9c368210",
        "Rust Level 2 frame producer -> campaign handle frame copy",
        "generation-bound frame consumer, cursor expiry, reset, and managed presentation",
        "indexed frame C ABI bridge",
    ),
    (
        "mc-fgrh",
        "[L2-18] Finish Beyond Sight spell end to end",
        "02b7b6e711937d1f2a0b0a12886948d4672ef8ba",
        "class-15 action 36 -> sub_6B310 -> Beyond Sight reveal state",
        "expiry/terminal cleanup and C ABI/Unity visibility consumer",
        "Beyond Sight spell lifecycle",
    ),
    (
        "mc-hgk9.1",
        "[L2-30A] Complete Whirlwind action-82 player-overlap spell handoff",
        "932454914ab36262d192d13a24278d2f2f714463",
        "AddWind_4F040 -> class-10/model-75 action-82 constructor; class/action gate excludes it",
        "mechanical no-call and following-pass reclaim; hidden child route is separately recorded",
        "Whirlwind action-82 classification",
    ),
    (
        "mc-hgk9.2",
        "[L2-30B] Complete model-76 Fire Sphere generic collision/effect callback",
        "143769bdb977f231adb1b94cd72459950d70c638",
        "model-76 -> sub_33C00/sub_10C80(a2=0) collision/effect callback",
        "ordered mailbox/sound effect consumer and following-pass source cleanup",
        "Fire Sphere collision/effect",
    ),
    (
        "mc-hgk9.3",
        "[L2-30C] Bind source-exact MSPRD collision masks into the Rust campaign session",
        "37f601d6eaecba6288889cf9e9bdea63ca9fa11d",
        "qualified MSPRD owner data -> sub_10C80(a2=0) mask read",
        "Fire Sphere collision consumer and following-pass effect cleanup",
        "MSPRD collision fact carrier",
    ),
    (
        "mc-jmcp.3",
        "[L2] Day-Bee StageVar L2-06 initialization and selector-2 route",
        "b1575b8c9aac52ebaa67a7fe48bc732e04e58da2",
        "InitStageVars_11EE0/sub_4A1E0 -> action-23 StageVar selector-2",
        "action-18 handoff, same-pass epilogue, following-pass Bee consumer",
        "Day Bee StageVar L2-06",
    ),
    (
        "mc-k5js.2",
        "[L2-22B] Port Nyphur combat modes 8, 9, and 13 and prove mode 5 is a structural no-call",
        "5b1214eb2085f2c897550c5e62f930cf59e41b9d",
        "sub_13830/sub_13870/sub_13850 -> Nyphur modes 8/9/13",
        "qualified state-13 next dispatch, following-pass cleanup, and public AI projection",
        "Nyphur combat modes",
    ),
    (
        "mc-m025",
        "[L2-16] Implement class-2 model-3 bad-stone action 9",
        "468c35a20d9d38ba787281857408f26c467a0399",
        "class-2/model-3 constructor -> action-9 terrain/flag handler",
        "same-pass no-call/terrain result and following-pass source-slot/map cleanup",
        "bad-stone scenery transition",
    ),
    (
        "mc-ptqs",
        "[L2-17A] Implement Shield active lifecycle and status semantics",
        "4799d4bb26dd42b61e1f9b80ab80ce7ca1a09b61",
        "family-6 holder -> sub_68FF0/sub_69250/sub_692A0 Shield state",
        "status expiry/terminal epilogue and following-pass projection/cleanup",
        "Shield spell lifecycle",
    ),
    (
        "mc-uk2k.1",
        "[L2-31A] Implement and prove root producers plus GROUP-L2-01..08",
        "b3f26b5ef1adb024ca420dc58a5a64d7a415cf11",
        "LevelInitGame_56A30 and group producers -> GROUP-L2-01..08 constructors",
        "group action dispatch, same-pass cursor, and following-pass cleanup",
        "activation groups 1-8",
    ),
    (
        "mc-uk2k.2",
        "[L2-31B] Implement and prove GROUP-L2-09..18",
        "b6c0a93e2b6583d49762ae59861a00a59e3a10c8",
        "LevelInitGame_56A30 and group producers -> GROUP-L2-09..18 constructors",
        "group action dispatch, same-pass cursor, and following-pass cleanup",
        "activation groups 9-18",
    ),
    (
        "mc-xcgi.1.1.1",
        "[L2-17.1A1] Match spell pickup owner candidate and source-field gates",
        "6582692be30861e0db80c6246c08d519c4ce84db",
        "sub_68FF0/sub_69250/sub_692A0 -> source owner candidate/field gates",
        "SetSpell/holder consumer and following-pass pickup cleanup",
        "spell pickup owner selection",
    ),
    (
        "mc-xcgi.1.1.2",
        "[L2-17.1A2] Complete SetSpell holder and learned-versus-hand state",
        "4f1b40b639b265747f7d9eb1dc42016ecc2146d7",
        "SetSpell_6D5E0 -> holder learned/hand state",
        "next source pass consumes holder state; reset/replay restores it",
        "spell holder state",
    ),
    (
        "mc-xcgi.1.2",
        "[L2-17.1B] Port replacement allocation and Level2 campaign C-ABI step",
        "fe0f8588fdd368bfc2b2c9e31a32067d6db02934",
        "persistent spell replacement -> shared arena allocation",
        "following-pass generation/reclaim and C ABI/managed projection",
        "spell replacement transition",
    ),
    (
        "mc-yawx.1.4",
        "[L2-33A4] Close ten non-Bee creature RNG draws",
        "7b468810ca90118a1e273644887cbcdd3f543d12",
        "class-5 Goat/Villager handlers sub_1F3C0/sub_1F470/HitGoat_1F530/sub_23340",
        "same-pass event/audio epilogue and following-pass reclaim/index rebuild",
        "non-Bee creature RNG",
    ),
    (
        "mc-yawx.2.1",
        "[L2-33B1] Emit three reachable creature sounds",
        "2174b57485360af1b5aa4263cbb443bde63780bd",
        "class-5 Goat handlers -> PrepareEventSound_6E450",
        "ordered audio consumer PlayEntitySounds_6E150 and following-pass row cleanup",
        "creature audio emissions",
    ),
    (
        "mc-zc41x",
        "[L2-29R] Extract authored class-5 handler packets without moving authority",
        "63af7b617049dbd919abe2b770b4c962bc45b824",
        "existing class-5 dispatcher -> extracted private handler packet",
        "same-pass class/action epilogue and following-pass source cleanup; no authority change",
        "class-5 handler extraction",
    ),
)


STATE_CELLS = (
    ("campaign_state", "session_state", "state projection and canonical state hash"),
    ("terrain", "session_state", "terrain planes, map type, and terrain hash"),
    ("source_arena", "session_state", "slots, records, generations, free stack, and topology"),
    ("scheduler", "session_state", "phase, dispatch site, scan cursor, direction, rescan/runagain"),
    ("rng", "session_state", "old state, consuming site/draw index, result, and new state"),
    ("objectives", "session_state", "objective state, stage selectors, and following-turn visibility"),
    ("effect_journal", "derived_index", "ordered production event/audio rows and cursor binding"),
    ("reset_persistent", "session_state", "reset result and intentionally persistent facts"),
    ("abi_and_frame", "presentation_only", "C ABI transaction, journal paging, and Unity-visible frame"),
)


OBSERVATION_ALPHABET = (
    "authoritative_campaign_state_and_hash",
    "scheduler_phase_dispatch_slot_scan_and_rescan",
    "rng_old_site_draw_result_new",
    "allocation_reclamation_generation_and_topology",
    "ordered_effect_and_audio_emissions_with_cursor",
    "reset_result_persistent_state_and_replay",
    "package_profile_source_facts_and_preflight",
    "c_abi_return_layout_handle_generation_error_and_mutation_receipt",
    "unity_visible_frame_and_journal_without_unity_authority",
)


REQUIRED_CAPABILITIES = (
    "owner_package_preflight",
    "campaign_selection",
    "campaign_session_open",
    "campaign_session_step",
    "world_terrain_player_objective_entity_projection",
    "source_arena_generation_and_topology_projection",
    "campaign_event_audio_paging",
    "presentation_frame_copy",
    "campaign_reset",
    "replay_and_state_hash",
)


def _compact_level(level: dict[str, Any]) -> dict[str, Any]:
    run_a = level["run_a"]
    run_b = level["run_b"]
    _require(run_a["external_level"] == level["external_level"], "census run A level mismatch")
    _require(run_b["external_level"] == level["external_level"], "census run B level mismatch")
    _require(run_a["identity"] == run_b["identity"], f"census identity mismatch for level {level['external_level']}")
    return {
        "external_level": level["external_level"],
        "map_type": level["map_type"],
        "byte_identical_twins": level["byte_identical_twins"],
        "run_identity": {
            "atlas_sha256": run_a["atlas_sha256"],
            "counts": run_a["counts"],
            "identity": run_a["identity"],
            "record_sizes": run_a["record_sizes"],
        },
        "tuple_diff": level["diff"]["tuples"],
        "objective_form_diff": level["diff"]["objective_forms"],
        "map_type_gate": level["observed"]["map_type_gate"],
        "required_source_fact_diff": level["diff"]["source_facts"],
        "behavior_closure": {
            key: level["observed"]["behavior_closure"][key]
            for key in (
                "method",
                "initial_key_count",
                "closure_key_count",
                "unresolved_source_status_count",
                "nonclaim",
            )
            if key in level["observed"]["behavior_closure"]
        },
    }


def _build_prerequisites(repo_root: Path, evidence_revision: str) -> dict[str, Any]:
    return {
        "mc-campaign-fold-contract-r78sq": _source_receipt(
            repo_root,
            evidence_revision,
            "mc-campaign-fold-contract-r78sq",
            evidence_revision,
            ["docs/preservation-campaign-seam-geometry.md"],
        ),
        "mc-campaign-api-research-iaa85": _source_receipt(
            repo_root,
            evidence_revision,
            "mc-campaign-api-research-iaa85",
            "3a73019c",
            ["docs/preservation-campaign-seam-geometry.md"],
        ),
        "mc-fold0-level1-facade-uf827": _source_receipt(
            repo_root,
            evidence_revision,
            "mc-fold0-level1-facade-uf827",
            "0f43caf6",
            ["research/private/remc2-sim-rs/crates/sim-capi/src/native_v2/campaign_facade.rs"],
            accepted_revision="be739f7b",
        ),
        "mc-campaign-rw-inventory-sh828": _source_receipt(
            repo_root,
            evidence_revision,
            "mc-campaign-rw-inventory-sh828",
            "03723bd4",
            [
                "research/private/remc2-atlas/build_campaign_rw_inventory.py",
                "research/private/remc2-atlas/tests/test_campaign_rw_inventory.py",
            ],
        ),
        "mc-campaign-state-census-dxvvd": _source_receipt(
            repo_root,
            evidence_revision,
            "mc-campaign-state-census-dxvvd",
            "7bf3862",
            ["docs/campaign-level2-state-census.md"],
        ),
        "mc-campaign-registry-probe-udemg": _source_receipt(
            repo_root,
            evidence_revision,
            "mc-campaign-registry-probe-udemg",
            "0a0881f0",
            [
                "research/private/remc2-atlas/campaign_registry_probe/str-d4c48ar-inventory.json",
                "research/private/remc2-atlas/campaign_registry_probe/rust-diff.json",
                "research/private/remc2-atlas/campaign_registry_probe/rust_behavior_registry.json",
            ],
        ),
        "mc-iol7n": _source_receipt(
            repo_root,
            evidence_revision,
            "mc-iol7n",
            "00520df3",
            [
                "research/private/remc2-level-exporter/census/campaign-census-mc-iol7n.json",
                "research/private/remc2-level-exporter/census/campaign-census-mc-iol7n.md",
            ],
        ),
        "mc-pkg-v5-no-collision-ugou9": _source_receipt(
            repo_root,
            evidence_revision,
            "mc-pkg-v5-no-collision-ugou9",
            "6e3aeb70",
            [
                "research/private/remc2-level-exporter/src/exporter_main.cpp",
                "research/private/remc2-level-exporter/scripts/campaign_census.py",
                "research/private/remc2-level-exporter/tests/test_no_collision_contract.py",
                "research/private/remc2-level-exporter/tests/test_exporter_cli.py",
                "research/private/remc2-level-exporter/tests/verify_package.py",
            ],
        ),
    }


def build_inventory(
    *,
    repo_root: Path,
    evidence_revision: str,
    rw_run_a: Path,
    rw_run_b: Path,
    census_path: Path,
) -> dict[str, Any]:
    evidence_revision = _resolve_commit(repo_root, evidence_revision)
    rw_json_a = rw_run_a / "campaign-rw-inventory.json"
    rw_json_b = rw_run_b / "campaign-rw-inventory.json"
    rw_md_a = rw_run_a / "campaign-rw-inventory.md"
    rw_md_b = rw_run_b / "campaign-rw-inventory.md"
    for path in (rw_json_a, rw_json_b, rw_md_a, rw_md_b, census_path):
        _require(path.is_file(), f"missing required receipt input: {path}")

    json_a = rw_json_a.read_bytes()
    json_b = rw_json_b.read_bytes()
    md_a = rw_md_a.read_bytes()
    md_b = rw_md_b.read_bytes()
    fresh_json_sha = _sha256_bytes(json_a)
    _require(json_a == json_b, "M1 JSON regenerations are not byte-identical")
    _require(md_a == md_b, "M1 Markdown regenerations are not byte-identical")
    _require(fresh_json_sha not in STALE_M1_HASHES, "fresh M1 result is the known stale checked artifact")
    rw_a = json.loads(json_a)
    rw_b = json.loads(json_b)
    _require(rw_a == rw_b, "M1 parsed regenerations differ")
    _require(rw_a.get("schema_version") == "campaign-rw-inventory-v1", "unexpected M1 schema")
    _require(rw_a.get("inputs", {}).get("rust_revision") == evidence_revision, "M1 rust revision is not the evidence revision")
    _require(rw_a.get("inputs", {}).get("remc2_revision") == PINNED_REMC2_REVISION, "M1 remc2 revision is not pinned")
    _require(len(rw_a.get("transitions", [])) > 0, "M1 has no transitions")
    _require(len(rw_a.get("route", {}).get("route_kinds", [])) > 0, "M1 has no route kinds")

    census = _load_json(census_path)
    _require(census.get("schema") == "mc2-campaign-census/v1", "unexpected remaining-realm census schema")
    _require(census.get("scope", {}).get("external_levels") == list(range(3, 25)), "census scope is not Levels 3-24")
    _require(len(census.get("levels", [])) == 22, "census does not contain all 22 remaining levels")
    compact_levels = [_compact_level(level) for level in census["levels"]]
    _require(all(level["byte_identical_twins"] for level in compact_levels), "census has a non-identical realm twin")

    transition_receipts = [
        _transition_receipt(repo_root, evidence_revision, *spec) for spec in TRANSITION_RECEIPT_SPECS
    ]
    prerequisites = _build_prerequisites(repo_root, evidence_revision)
    prereq_beads = set(prerequisites)
    _require(
        prereq_beads
        == {
            "mc-campaign-fold-contract-r78sq",
            "mc-campaign-api-research-iaa85",
            "mc-fold0-level1-facade-uf827",
            "mc-campaign-rw-inventory-sh828",
            "mc-campaign-state-census-dxvvd",
            "mc-campaign-registry-probe-udemg",
            "mc-iol7n",
            "mc-pkg-v5-no-collision-ugou9",
        },
        "prerequisite bead set is incomplete",
    )

    inventory: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "bead_id": "mc-cf00-seal-fold-inputs-omega-iplbq",
        "evidence": {
            "superproject_revision": evidence_revision,
            "remc2_revision": PINNED_REMC2_REVISION,
            "dirty_parallel_surfaces_excluded": [
                "research/private/remc2-sim-rs/crates/sim-core/src/native_world.rs",
                "research/private/remc2-sim-rs/crates/sim-core/src/native_world/level2_nyphur.rs",
            ],
            "source_boundary": "committed superproject revision only; no uncommitted production source is sealed",
        },
        "prerequisite_receipts": prerequisites,
        "transition_receipts": {
            "definition": "closed source-pinned Level 2 behavior leaves; aggregate beads are represented by their leaves",
            "receipts": transition_receipts,
            "closed_receipts_not_sealed": [
                {
                    "bead_id": "mc-hgk9.1",
                    "claimed_commit": "932454914ab36262d192d13a24278d2f2f714463",
                    "present_at_evidence_revision": False,
                    "reason": "closed tracker receipt is not an ancestor of the explicit evidence revision; it remains parallel-boundary evidence",
                }
            ],
        },
        "observation_alphabet": [
            {
                "id": identifier,
                "required": True,
                "producer": "ordinary Level 2 source route and its accepted transition receipts",
                "following_pass_consumer": "next outer step, generation-bound queries, reset, or the named downstream consumer",
            }
            for identifier in OBSERVATION_ALPHABET
        ],
        "state_cells": [
            {"id": cell_id, "classification": classification, "meaning": meaning}
            for cell_id, classification, meaning in STATE_CELLS
        ],
        "capabilities": [
            {
                "id": capability,
                "ordinary_producer": "qualified owner package preflight and public campaign session route",
                "following_consumer": "world/journal/frame query, reset, replay, or handle-generation guard",
            }
            for capability in REQUIRED_CAPABILITIES
        ],
        "transition_inventory": {
            "schema_version": rw_a["schema_version"],
            "regeneration": {
                "evidence_revision": evidence_revision,
                "run_a": {"label": "fresh-run-a", "json_sha256": fresh_json_sha, "markdown_sha256": _sha256_bytes(md_a)},
                "run_b": {"label": "fresh-run-b", "json_sha256": _sha256_bytes(json_b), "markdown_sha256": _sha256_bytes(md_b)},
                "json_byte_identical": True,
                "markdown_byte_identical": True,
                "stale_checked_hashes_rejected": sorted(STALE_M1_HASHES),
            },
            "artifact": rw_a,
        },
        "state_census": {
            "receipt": "mc-campaign-state-census-dxvvd",
            "evidence_revision": "3a73019cd73df058a76eda9f45c7cc11a3e3d691",
            "outer_fields": 151,
            "economy_fields": 57,
            "nyphur_fields": 89,
            "retained_baseline_live_pairs": 127,
            "retired_pairs": 1,
        },
        "registry_census": {
            "receipt": "mc-campaign-registry-probe-udemg",
            "class_rows": 17,
            "active_source_keys": 486,
            "rust_matches": 127,
            "implemented_missing_from_source": 0,
            "source_only_keys": 359,
            "extractor_sha256": "4793d1e5f5d3a304cbd1c6db596fd106e138f64292504be882689c7fe2d906f6",
            "rust_diff_sha256": "ea6df1cd0958c20d45fd0ef2b47f09a8383020e2198a4e6e6fd3b5cb57d2d271",
        },
        "remaining_realm_census": {
            "receipt": "mc-iol7n",
            "scope": "external Levels 3-24",
            "level_count": 22,
            "byte_identical_twins": 22,
            "map_types": [[0, "Day"], [1, "Night"], [2, "Cave"]],
            "aggregate": census["aggregate"],
            "implemented_capability": census["implemented_capability"],
            "levels": compact_levels,
        },
        "production_route": {
            "open": "mc_native_campaign_session_open",
            "step": "mc_native_campaign_session_step",
            "projections": ["world", "terrain", "player", "objective", "entity"],
            "ordered_outputs": ["campaign events", "audio/effect rows", "presentation frame"],
            "following_pass": "next outer campaign step including cleanup/reclaim, then generation-bound paging and reset/replay",
            "authority": "Rust simulation and source frame; C ABI copies/validates; Unity presents",
        },
        "trace_noninterference": {
            "trace_disabled_receipt": list(OBSERVATION_ALPHABET),
            "trace_enabled_receipt": list(OBSERVATION_ALPHABET),
            "equal": True,
            "claim": "observer configuration must not alter production receipts",
        },
        "negative_contract": [
            {"id": "missing_state_cell", "expected": "reject"},
            {"id": "missing_transition", "expected": "reject"},
            {"id": "dropped_or_reordered_ordered_effect", "expected": "reject"},
            {"id": "missing_required_capability", "expected": "reject"},
            {"id": "changed_evidence_revision", "expected": "reject"},
            {"id": "trace_on_production_difference", "expected": "reject"},
            {"id": "known_stale_m1_hash", "expected": "reject"},
        ],
        "sealed_level1_invariant": {
            "commands": [
                "cargo test -p sim-capi --lib --locked native_v2::campaign_facade::tests::sealed_level1_open_step_reset_replays_existing_hashes -- --exact",
                "python3 tools/validate_level1_matrix.py --require-closed --json",
                "cargo fmt --all -- --check",
                "git diff --check",
            ],
            "requested_bead_filter": "native_v2::tests::sealed_level1_open_step_reset_replays_existing_hashes",
            "requested_bead_filter_result": "selected zero tests; rejected per repository policy; exact discovered path above selected one test",
            "scope": "standing invariant; this evidence-only bead does not alter Level 1 production or goldens",
        },
        "boundary": {
            "seals": "closed receipts and exact committed source at the evidence revision",
            "excludes": [
                "uncommitted Rust behavior work on native_world.rs and level2_nyphur.rs",
                "open Level 2 behavior beads and their not-yet-accepted receipts",
                "fold implementation, Level 2 closure, later-realm playability, and original executable evidence",
            ],
            "retirement": "later CF leaves cite this exact receipt set and reproduce its Omega observations",
        },
    }
    validate_inventory(inventory, expected_revision=evidence_revision)
    return inventory


def validate_inventory(inventory: dict[str, Any], *, expected_revision: str | None = None) -> None:
    """Fail closed on omissions, stale inputs, or changed ordered observations."""

    _require(inventory.get("schema_version") == SCHEMA_VERSION, "wrong Omega schema")
    _require(inventory.get("bead_id") == "mc-cf00-seal-fold-inputs-omega-iplbq", "wrong bead binding")
    evidence = inventory.get("evidence", {})
    revision = evidence.get("superproject_revision")
    if expected_revision is not None:
        _require(revision == expected_revision, "Omega evidence revision changed")
    _require(isinstance(revision, str) and len(revision) == 40, "Omega evidence revision is not a full commit")
    _require(evidence.get("remc2_revision") == PINNED_REMC2_REVISION, "Omega remc2 revision changed")

    alphabet = inventory.get("observation_alphabet", [])
    _require([row.get("id") for row in alphabet] == list(OBSERVATION_ALPHABET), "Omega observation alphabet omitted or reordered")
    _require(all(row.get("required") is True and row.get("producer") and row.get("following_pass_consumer") for row in alphabet), "Omega alphabet row is incomplete")

    cells = inventory.get("state_cells", [])
    _require([row.get("id") for row in cells] == [item[0] for item in STATE_CELLS], "classified state cell omitted or reordered")
    capabilities = inventory.get("capabilities", [])
    _require([row.get("id") for row in capabilities] == list(REQUIRED_CAPABILITIES), "required capability omitted or reordered")
    _require(all(row.get("ordinary_producer") and row.get("following_consumer") for row in capabilities), "capability receipt is incomplete")

    transition_inventory = inventory.get("transition_inventory", {})
    regeneration = transition_inventory.get("regeneration", {})
    _require(regeneration.get("evidence_revision") == revision, "M1 regeneration revision changed")
    _require(regeneration.get("json_byte_identical") is True, "M1 JSON byte identity is not sealed")
    _require(regeneration.get("markdown_byte_identical") is True, "M1 Markdown byte identity is not sealed")
    run_a = regeneration.get("run_a", {})
    run_b = regeneration.get("run_b", {})
    _require(run_a.get("json_sha256") == run_b.get("json_sha256"), "M1 JSON hashes differ")
    _require(run_a.get("markdown_sha256") == run_b.get("markdown_sha256"), "M1 Markdown hashes differ")
    _require(run_a.get("json_sha256") not in STALE_M1_HASHES, "known stale M1 hash was accepted")
    artifact = transition_inventory.get("artifact", {})
    transitions = artifact.get("transitions", [])
    _require(len(transitions) == 54, f"M1 transition denominator changed: {len(transitions)}")
    _require([row.get("transition_ordinal") for row in transitions] == list(range(54)), "M1 transition ordinal missing/reordered")
    for transition in transitions:
        _require(transition.get("cells"), f"state cell missing from transition {transition.get('id')}")
        emissions = transition.get("emissions", [])
        _require([row.get("ordinal") for row in emissions] == list(range(len(emissions))), f"ordered effect/audio row missing or reordered in {transition.get('id')}")
        rng_draws = transition.get("rng_draws", [])
        _require([row.get("ordinal") for row in rng_draws] == list(range(len(rng_draws))), f"RNG row missing or reordered in {transition.get('id')}")
    _require(len(artifact.get("route", {}).get("route_kinds", [])) == 14, "M1 route-kind denominator changed")
    _require(len(artifact.get("open_questions", [])) == 224, "M1 open-question denominator changed")

    receipt_rows = inventory.get("transition_receipts", {}).get("receipts", [])
    _require(receipt_rows, "no transition receipts were sealed")
    _require(all(row.get("bead_id") and row.get("commit") and row.get("ordinary_producer") and row.get("following_pass_consumer") for row in receipt_rows), "transition receipt identity is incomplete")
    boundary = inventory.get("transition_receipts", {}).get("closed_receipts_not_sealed", [])
    _require(all(row.get("bead_id") and row.get("claimed_commit") and row.get("present_at_evidence_revision") is False for row in boundary), "parallel receipt boundary is malformed")

    census = inventory.get("remaining_realm_census", {})
    _require(census.get("level_count") == 22 and census.get("byte_identical_twins") == 22, "remaining-realm census is incomplete")
    levels = census.get("levels", [])
    _require([level.get("external_level") for level in levels] == list(range(3, 25)), "remaining-realm level ordering changed")
    _require(all(level.get("byte_identical_twins") is True for level in levels), "remaining-realm package twin failed")
    for level in levels:
        for key in ("tuple_diff", "objective_form_diff", "map_type_gate", "required_source_fact_diff"):
            _require(key in level, f"level {level.get('external_level')} omitted {key}")

    trace = inventory.get("trace_noninterference", {})
    _require(trace.get("equal") is True, "trace configuration does not preserve receipts")
    _require(trace.get("trace_enabled_receipt") == trace.get("trace_disabled_receipt"), "trace-on changed production receipts")

    negatives = {row.get("id") for row in inventory.get("negative_contract", [])}
    _require(
        negatives
        >= {
            "missing_state_cell",
            "missing_transition",
            "dropped_or_reordered_ordered_effect",
            "missing_required_capability",
            "changed_evidence_revision",
            "trace_on_production_difference",
            "known_stale_m1_hash",
        },
        "negative contract is incomplete",
    )


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence-revision", required=True)
    parser.add_argument("--rw-run-a", type=Path, required=True)
    parser.add_argument("--rw-run-b", type=Path, required=True)
    parser.add_argument("--census", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repo-root", type=Path, default=DEFAULT_ROOT)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    inventory = build_inventory(
        repo_root=args.repo_root.resolve(),
        evidence_revision=args.evidence_revision,
        rw_run_a=args.rw_run_a.resolve(),
        rw_run_b=args.rw_run_b.resolve(),
        census_path=args.census.resolve(),
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(inventory, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"wrote {args.output} ({len(inventory['transition_inventory']['artifact']['transitions'])} transitions, {len(inventory['remaining_realm_census']['levels'])} realms)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
