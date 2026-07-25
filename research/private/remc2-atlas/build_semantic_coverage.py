#!/usr/bin/env python3
"""Build denominator-driven semantic coverage over the complete remc2 atlas.

This does not pretend that a filename cluster or a renamed decompiler function
proves intent. It gives every mechanically recovered function a stable place in
the program map, links evidence-backed claims to the source functions that
contain their anchors, and ranks the remaining raw-address functions for
further archaeology.
"""

from __future__ import annotations

import csv
import json
import math
import re
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path


HERE = Path(__file__).resolve().parent
UPSTREAM = HERE.parents[2] / "upstream/magic-carpet-2-hd"
GENERATED = HERE / "generated"
SEMANTIC_GENERATED = HERE / "semantics/generated"
FUNCTION_MATRIX = GENERATED / "function-matrix.csv"
SUBSYSTEM_SEEDS = GENERATED / "subsystem-seeds.csv"
STATE_TARGETS = GENERATED / "state-targets.csv"
CLAIMS = SEMANTIC_GENERATED / "claims.csv"
DISPATCH_SUMMARY = SEMANTIC_GENERATED / "dispatch-atlas-summary.json"
OUTPUT = SEMANTIC_GENERATED

RAW_ADDRESS_RE = re.compile(r"^sub_([0-9A-Fa-f]+)(?:_.*)?$")
RECOVERED_ADDRESS_RE = re.compile(r"_([0-9A-Fa-f]{4,7})(?:_.*)?$")
ANCHOR_RE = re.compile(
    r"(?P<file>[A-Za-z0-9_./+-]+\.(?:c|cc|cpp|h|hpp)):"
    r"(?P<start>\d+)(?:-(?P<end>\d+))?"
)
OFFSET_SUFFIX_RE = re.compile(r"(?:_0x[0-9A-Fa-f]+|_[0-9A-Fa-f]{4,})(?:x|ar)?$")
RAW_STATE_TOKEN_RE = re.compile(
    r"^(?:x_)?(?:byte|word|dword|qword|array|field|stub|unk|unknown|str|"
    r"data|value|var|flag|counter|index)(?:_|$)",
    re.IGNORECASE,
)

GAMEPLAY_SUBSYSTEMS = {
    "entity_ai_and_gameplay",
    "player_flight_and_actions",
    "terrain_and_world_mutation",
    "spells_and_projectiles",
    "mana_castle_and_economy",
    "objectives_and_progression",
    "entity_pool_and_dispatch",
    "frame_and_level_scheduler",
    "camera",
    "level_data_and_persistence",
    "world_and_hud_rendering",
    "menus_hud_and_frontend",
    "audio_and_music",
    "network",
    "replay_and_regression",
}

SUBSYSTEM_LABELS = {
    "entity_ai_and_gameplay": "Entity AI and general gameplay",
    "player_flight_and_actions": "Player flight and actions",
    "terrain_and_world_mutation": "Terrain and world mutation",
    "spells_and_projectiles": "Spells and projectiles",
    "mana_castle_and_economy": "Mana, castle, and economy",
    "objectives_and_progression": "Objectives and progression",
    "entity_pool_and_dispatch": "Entity pool and dispatch",
    "frame_and_level_scheduler": "Frame and level scheduler",
    "camera": "Camera",
    "level_data_and_persistence": "Level data and persistence",
    "world_and_hud_rendering": "World and HUD rendering",
    "menus_hud_and_frontend": "Menus, HUD, and frontend",
    "audio_and_music": "Audio and music",
    "network": "Network",
    "replay_and_regression": "Replay and regression",
    "host_input_and_display": "Host input and display",
    "configuration": "Configuration",
    "host_timing_and_threads": "Host timing and threads",
    "host_platform": "Host platform",
    "math_support": "Math support",
    "data_and_diagnostics_utilities": "Data and diagnostics utilities",
    "process_lifecycle_and_compatibility": "Process lifecycle and compatibility",
    "recovered_types_tables_and_unclassified": "Recovered types, tables, and unclassified",
}

# Exact-name semantic promotions for state fields whose lineage is already
# evidenced by the backbone. Everything else remains UNKNOWN rather than being
# guessed from spelling. Raw offset-bearing names remain as immutable keys.
STATE_SEMANTICS = {
    "Type_str_164::rollDelta_0x4_4": ("RUNTIME_STATE", "player roll input delta", "authoritative input", "signed angle delta", "flight-roll-input"),
    "Type_str_164::pitchDelta_0x6_6": ("RUNTIME_STATE", "player pitch input delta", "authoritative input", "signed angle delta", "flight-pitch-input"),
    "Type_str_164::speed_0xc_12": ("RUNTIME_STATE", "player target forward speed", "authoritative", "fixed-point speed", "flight-target-speed"),
    "_str_0x6E8E::actSpeed_0x82_130": ("RUNTIME_STATE", "entity actual forward speed", "authoritative", "fixed-point speed", "flight-actual-speed"),
    "Type_str_164::strafeSpeed_0x10_16": ("RUNTIME_STATE", "player lateral strafe speed", "authoritative", "fixed-point speed", "flight-strafe"),
    "Type_str_164::moveBoost_0x1E_30": ("RUNTIME_STATE", "directional recoil or hit impulse", "authoritative", "signed impulse", "flight-recoil"),
    "Type_str_164::xAdd_0x1A6_422": ("RUNTIME_STATE", "one-tick X impulse", "authoritative", "world-position delta", "flight-transient-impulse"),
    "Type_str_164::yAdd_0x1A8_424": ("RUNTIME_STATE", "one-tick Y impulse", "authoritative", "world-position delta", "flight-transient-impulse"),
    "Type_str_164::zAdd_0x1AA_426": ("RUNTIME_STATE", "one-tick Z impulse", "authoritative", "world-position delta", "flight-transient-impulse"),
    "Type_str_164::moveSpeed_0x14C_332": ("RUNTIME_STATE", "speed-spell movement modifier", "authoritative", "fixed-point multiplier/state", "flight-speed-modifier"),
    "Type_str_164::mobilizeCounter_0x14E_334": ("RUNTIME_STATE", "movement immobilization counter", "authoritative", "ticks", "flight-immobilize"),
    "_str_0x6E8E::minSpeed_0x84_132": ("RUNTIME_STATE", "common actor cruise target and fast cap", "authoritative", "fixed-point speed", "creature-speed-target-ceiling"),
    "_str_0x6E8E::maxSpeed_0x86_134": ("RUNTIME_STATE", "common actor slow floor", "authoritative", "fixed-point speed", "creature-speed-floor"),
    "mapTerrainType_10B4E0": ("RUNTIME_STATE", "terrain material/type grid", "authoritative", "256x256 byte cells", "terrain-type-grid"),
    "mapHeightmap_11B4E0": ("RUNTIME_STATE", "ground height grid", "authoritative", "256x256 byte vertices", "terrain-height-grid"),
    "mapShading_12B4E0": ("DERIVED_VALUE", "terrain shading grid", "presentation-derived", "256x256 byte cells", "terrain-shading-grid"),
    "mapAngle_13B4E0": ("RUNTIME_STATE", "terrain topology, rotation, and dirty flags", "authoritative and derived", "256x256 byte cells", "terrain-angle-water-grid"),
    "x_BYTE_14B4E0_second_heightmap": ("RUNTIME_STATE", "cave ceiling or second height grid", "authoritative", "256x256 byte vertices", "terrain-cave-height-grid"),
    "mapEntityIndex_15B4E0": ("RUNTIME_STATE", "per-tile runtime entity-chain head", "authoritative spatial index", "256x256 signed entity indices", "terrain-cell-entity-grid"),
    "building_F2CD0x": ("DERIVED_VALUE", "terrain topology to rendered tile/rotation lookup", "derived presentation and repair", "2800x2 lookup", "terrain-topology-render-lookup"),
    "Entities_EA3E4": ("RUNTIME_STATE", "fixed runtime entity pool/index surface", "authoritative", "1000 entity pointers/records", "entity-pool-lifecycle"),
    "str_D4C48ar": ("STRUCTURAL_CONSTANT", "runtime class action/constructor registry", "authoritative dispatch", "16 class descriptors", "entity-constructor-dispatch;entity-action-dispatch"),
    "_str_0x6E8E::class_0x3F_63": ("ID_SELECTOR_MASK_OR_SENTINEL", "runtime entity class selector", "authoritative", "unsigned class id", "entity-runtime-selectors"),
    "_str_0x6E8E::model_0x40_64": ("ID_SELECTOR_MASK_OR_SENTINEL", "runtime entity model selector", "authoritative", "unsigned model id", "entity-runtime-selectors"),
    "_str_0x6E8E::actionIndex_0x45_69": ("ID_SELECTOR_MASK_OR_SENTINEL", "runtime entity current action selector", "authoritative", "unsigned action id", "entity-runtime-selectors"),
    "_str_0x6E8E::position_0x4C_76": ("RUNTIME_STATE", "runtime entity world position", "authoritative", "fixed-point axis_3d", "flight-forward;entity-action-dispatch"),
    "_str_0x6E8E::life_0x8": ("RUNTIME_STATE", "runtime entity current life or overloaded lifetime", "authoritative by class/model", "signed integer", "creature-pending-damage-consumer"),
    "_str_0x6E8E::maxLife_0x4": ("INITIAL_CONDITION_OR_CONTENT", "runtime entity maximum life or overloaded duration", "authoritative by class/model", "signed integer", "runtime-class-05"),
    "type_str_0x5E_94::dword_0x5E_94": ("RUNTIME_STATE", "pending damage accumulator in proven damage contexts", "authoritative by class/model", "signed payload accumulator", "fireball-area-damage-writer"),
    "type_str_0x5E_94::word_0x62_98": ("RUNTIME_STATE", "pending damage source entity in proven damage contexts", "authoritative by class/model", "entity index", "fireball-area-damage-writer"),
    "type_D41A0_BYTESTR_0::rand_0x8": ("RUNTIME_STATE", "deterministic simulation random state", "authoritative", "32-bit Watcom-compatible state", ""),
    "type_str_0x2BDE::Turn_2BE0_11248": ("RUNTIME_STATE", "per-player simulation turn", "authoritative", "turn counter", ""),
    "type_D41A0_BYTESTR_0::playerInputs_0x6E3E": ("RUNTIME_STATE", "per-player deterministic action bytes", "authoritative input", "Type_PlayerInput array", ""),
    "Type_Level_2FECE::MapType": ("INITIAL_CONDITION_OR_CONTENT", "authored day/night/cave realm", "authoritative content", "enum Day=0 Night=1 Cave=2", "level-map-realm"),
    "Type_CompressedLevel_2FECE::MapType": ("INITIAL_CONDITION_OR_CONTENT", "compressed authored day/night/cave realm", "authoritative content", "enum Day=0 Night=1 Cave=2", "level-map-realm"),
    "Type_Level_2FECE::seed_0x2FEE5": ("INITIAL_CONDITION_OR_CONTENT", "procedural terrain seed", "authoritative content", "unsigned 16-bit seed", "terrain-generation-pipeline"),
    "Type_Level_2FECE::river_0x2FEF5": ("INITIAL_CONDITION_OR_CONTENT", "procedural river-generation parameter", "authoritative content", "32-bit packed parameter", "terrain-generation-pipeline"),
    "SPELLS_BEGIN_BUFFER_str": ("INITIAL_CONDITION_OR_CONTENT", "live spell family/tier table", "authoritative content", "26 families x 80 bytes", "spell-family-00;spell-family-25"),
    "type_SPELLS_BEGIN_BUFFER_str_sub::subSpellIndex_2": ("INITIAL_CONDITION_OR_CONTENT", "family-specific spell effect selector", "authoritative content", "signed 32-bit family-specific value", "runtime-spell-holder"),
    "type_SPELLS_BEGIN_BUFFER_str_sub::manaCost_6": ("GAMEPLAY_PARAMETER", "spell tier mana cost", "authoritative content", "signed 32-bit mana", "spell-tier-00-0"),
    "type_SPELLS_BEGIN_BUFFER_str_sub::maxManaLimit_A": ("GAMEPLAY_PARAMETER", "spell tier maximum-mana requirement", "authoritative content", "signed 32-bit mana", "spell-tier-00-0"),
    "type_SPELLS_BEGIN_BUFFER_str_sub::life_0x1A": ("INITIAL_CONDITION_OR_CONTENT", "family-specific spell life/timing selector", "authoritative content", "signed byte family-specific value", "runtime-spell-holder"),
    "_str_0x6E8E::mana_0x90_144": ("RUNTIME_STATE", "runtime entity current mana", "authoritative", "signed mana", ""),
    "_str_0x6E8E::maxMana_0x8C_140": ("INITIAL_CONDITION_OR_CONTENT", "runtime entity maximum mana", "authoritative", "signed mana", ""),
    "_str_0x6E8E::manaRegen_0x88_136": ("GAMEPLAY_PARAMETER", "runtime entity mana regeneration rate", "authoritative", "signed mana per update", ""),
    "Type_WizardMapSettings_0x360D2::StartingSpells_0x360E1x": ("INITIAL_CONDITION_OR_CONTENT", "authored wizard starting spell flags", "authoritative content", "26-byte family array", "level-record-layout"),
    "Type_WizardMapSettings_0x360D2::BlockedSpells_0x36115x": ("INITIAL_CONDITION_OR_CONTENT", "authored wizard blocked spell flags", "authoritative content", "26-byte family array", "level-record-layout"),
    "type_D41A0_BYTESTR_0::str_0x3664C": ("PRESENTATION", "night/cave dynamic-light record pool", "derived presentation", "50 light records", "dynamic-light-registration"),
    "type_D41A0_BYTESTR_0::word_0x36DFA": ("RUNTIME_STATE", "active dynamic-light count", "derived presentation", "0..50 count", "dynamic-light-registration"),
    "type_D41A0_BYTESTR_0::dword_0x36DF2": ("RUNTIME_STATE", "dynamic-light flicker random state", "derived presentation", "32-bit deterministic state", "dynamic-light-apply-current"),
    "type_str_0x3664C::byte_0": ("RUNTIME_STATE", "dynamic-light active flag", "derived presentation", "boolean", "dynamic-light-registration"),
    "type_str_0x3664C::byte_1": ("INITIAL_CONDITION_OR_CONTENT", "dynamic-light life-gating flags", "derived presentation", "bit flags", "dynamic-light-registration"),
    "type_str_0x3664C::byte_2": ("GAMEPLAY_PARAMETER", "dynamic-light intensity scale", "presentation parameter", "unsigned byte scale", "dynamic-light-registration"),
    "type_str_0x3664C::byte_3": ("GAMEPLAY_PARAMETER", "dynamic-light flicker variation", "presentation parameter", "positive unsigned range", "dynamic-light-registration"),
    "type_str_0x3664C::axis3d_4": ("DERIVED_VALUE", "dynamic-light current world position", "derived presentation", "fixed-point axis_3d", "dynamic-light-apply-current"),
    "type_str_0x3664C::event_A": ("RUNTIME_STATE", "dynamic-light owner entity link", "derived presentation", "entity pointer or index", "dynamic-light-registration"),
    "type_str_0x3664C::array_E": ("DERIVED_VALUE", "previous 5x5 dynamic-light contribution", "derived presentation", "25 shading deltas", "dynamic-light-remove-previous;dynamic-light-apply-current"),
    "type_substr_3659C::IsLevelEnd_0": ("RUNTIME_STATE", "current objective/level completion state", "authoritative progression", "boolean or bitfield", "level-end-promotion"),
    "type_str_0x2BDE::dw_w_b_0_2BDE_11230": ("RUNTIME_STATE", "packed per-player session and level-transition flags", "authoritative progression", "packed word/bytes", "level-end-promotion;checkpoint-conditional-reload"),
    "Type_x_D41A0_BYTEARRAY_4_struct::setting_38545": ("RUNTIME_STATE", "packed session flags including saved-checkpoint state", "authoritative progression", "bitfield", "checkpoint-conditional-reload"),
    "Type_x_D41A0_BYTEARRAY_4_struct::setting_byte2_23": ("RUNTIME_STATE", "packed session flags including level-end suppression", "authoritative progression", "bitfield", "level-end-promotion"),
    "type_D41A0_BYTESTR_0::array_0x2362": ("INITIAL_CONDITION_OR_CONTENT", "resolved player spawn positions", "authoritative content", "eight axis_3d positions", "player-spawn-respawn-initialization"),
    "type_str_0x2BDE::IsAiPlayer_0x009_2BE4_11239": ("ID_SELECTOR_MASK_OR_SENTINEL", "player slot human/AI selector", "authoritative", "boolean or small selector", "player-spawn-respawn-initialization"),
    "Type_WizardMapSettings_0x360D2::Aggression_0x360D5": ("GAMEPLAY_PARAMETER", "authored AI wizard aggression", "authoritative content", "signed 16-bit tuning value", "player-spawn-respawn-initialization"),
    "Type_WizardMapSettings_0x360D2::Reflexes_0x360D9": ("GAMEPLAY_PARAMETER", "authored AI wizard reflexes", "authoritative content", "signed 16-bit tuning value", "player-spawn-respawn-initialization"),
    "Type_WizardMapSettings_0x360D2::Perception_0x360DD": ("GAMEPLAY_PARAMETER", "authored AI wizard perception", "authoritative content", "signed 16-bit tuning value", "player-spawn-respawn-initialization"),
    "Type_WizardMapSettings_0x360D2::Life_0x3612F": ("GAMEPLAY_PARAMETER", "authored AI wizard life scaling", "authoritative content", "Q8 multiplier applied with shift 8", "player-spawn-respawn-initialization"),
    "dw_w_b::byte": ("CONTEXT_DEPENDENT_STORAGE_VIEW", "byte view over a four-byte packed union", "representation only", "four overlapping unsigned bytes", "packed-storage-union-views"),
    "dw_w_b::word": ("CONTEXT_DEPENDENT_STORAGE_VIEW", "word view over a four-byte packed union", "representation only", "two overlapping unsigned 16-bit words", "packed-storage-union-views"),
    "dw_w_b::dword": ("CONTEXT_DEPENDENT_STORAGE_VIEW", "dword view over a four-byte packed union", "representation only", "one unsigned 32-bit word", "packed-storage-union-views"),
    "uaxis_2d::_axis_2d": ("CONTEXT_DEPENDENT_STORAGE_VIEW", "two-byte coordinate view", "representation only", "two unsigned 8-bit coordinates", "packed-coordinate-union-view"),
    "uaxis_2d::word": ("CONTEXT_DEPENDENT_STORAGE_VIEW", "packed word view of two byte coordinates", "representation only", "one unsigned 16-bit alias", "packed-coordinate-union-view"),
    "str_F2C20ar": ("PRESENTATION", "software world-renderer working state", "derived presentation", "projection, fog, viewport, and draw temporaries", "renderer-working-state"),
    "_str_0x6E8E::dword_0xA4_164x": ("RUNTIME_STATE", "class/model-specific auxiliary-state link", "context-dependent authoritative runtime", "entity pointer", "entity-auxiliary-state-pointer"),
    "type_D41A0_BYTESTR_0::dword_0x11e6": ("RUNTIME_STATE", "top index of the secondary reclaimable-entity pointer stack", "authoritative allocator bookkeeping", "signed index with -1 empty sentinel", "reclaimable-entity-stack-top"),
    "type_D41A0_BYTESTR_0::dword_0x35": ("RUNTIME_STATE", "top index of the ordinary entity free-pool stack", "authoritative allocator bookkeeping", "signed index with -1 empty sentinel", "entity-free-stack-top"),
    "_str_0x6E8E::word_0x4A_74": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "action-local polymorphic 16-bit scratch slot", "authoritative interpretation selected by class, model, and action", "stage selector, countdown, target id, angle step, hand preference, or building discriminator", "entity-action-local-word-4a"),
    "_str_0x6E8E::byte_0x3B_59": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "class/model/action-dependent entity byte at offset 0x3B", "authoritative contextual entity state", "input gate, animation/motion selector, or action-local counter", "entity-polymorphic-byte-3b"),
    "_str_0x6E8E::word_0x98_152": ("DERIVED_VALUE", "cached target entity identity fingerprint", "derived runtime slot-reuse guard", "id plus model plus class shifted by seven", "ai-target-reuse-fingerprint"),
    "type_str_0x5E_94::word_0x68_104": ("ID_SELECTOR_MASK_OR_SENTINEL", "pending interaction-source entity identifier", "authoritative pending-event mailbox", "signed 16-bit entity index or empty value", "pending-interaction-source"),
    "Type_x_D41A0_BYTEARRAY_4_struct::dword_38523": ("DERIVED_VALUE", "per-tick mana-related entity-list head", "derived runtime category cache", "linked-list head for class-10 models 39, 40, and 57", "derived-entity-category-lists"),
    "Type_x_D41A0_BYTEARRAY_4_struct::dword_38527": ("DERIVED_VALUE", "per-tick building entity-list head", "derived runtime category cache", "linked-list head for class-10 model 45", "derived-entity-category-lists"),
    "Type_str_164::array_0x1FC_508": ("RUNTIME_STATE", "per-opponent AI hostility score and forced-hostile latch records", "authoritative AI strategic state", "four-word stride; declaration is undersized for player 7 accesses", "ai-opponent-hostility-records"),
    "Type_str_164::word_0x248_584": ("RUNTIME_STATE", "creature-detection or engagement attention window", "authoritative AI targeting countdown", "unsigned tick counter usually set to 200", "creature-attention-window"),
    "Type_str_164::word_0xe_14": ("RUNTIME_STATE", "per-update locomotion-command or activity latch", "authoritative transient movement state", "16-bit boolean-like latch", "locomotion-command-latch"),
    "x_DWORD_17DE38str": ("PLATFORM_OR_COMPATIBILITY", "host palette, frontend, input, save-dialog, and legacy-network aggregate", "mixed host/frontend state", "packed 613-byte aggregate", "host-frontend-state-aggregate"),
    "unk_18058Cstr": ("PLATFORM_OR_COMPATIBILITY", "host input-device snapshot", "host input translated into player commands", "mouse, button, keyboard, and joystick aggregate", "host-input-device-state"),
    "type_unk_18058Cstr::x_DWORD_1805B0_mouse": ("PLATFORM_OR_COMPATIBILITY", "current host mouse cursor coordinates", "host input and frontend presentation", "two signed 32-bit screen coordinates", "host-input-device-state"),
    "type_unk_18058Cstr::MouseButtonState_18059C": ("PLATFORM_OR_COMPATIBILITY", "current mouse-button and click-state mask", "host input translated into player commands", "32-bit bit mask", "host-input-device-state"),
    "type_unk_18058Cstr::x_WORD_1805C0_arrow_keys": ("PLATFORM_OR_COMPATIBILITY", "current directional-key mask", "host input translated into player commands", "16-bit bit mask", "host-input-device-state"),
    "type_unk_18058Cstr::x_WORD_1805C2_joystick": ("ID_SELECTOR_MASK_OR_SENTINEL", "active host control-device mode", "host input selection", "16-bit device/mode selector", "host-input-device-state"),
    "type_unk_18058Cstr::x_DWORD_1805B8_mouse_position_x": ("PLATFORM_OR_COMPATIBILITY", "saved or recentered host mouse X position", "host input and frontend presentation", "signed 32-bit screen coordinate", "host-input-device-state"),
    "type_unk_18058Cstr::x_DWORD_1805BC_mouse_position_y": ("PLATFORM_OR_COMPATIBILITY", "saved or recentered host mouse Y position", "host input and frontend presentation", "signed 32-bit screen coordinate", "host-input-device-state"),
    "Type_str_164::str_611": ("RUNTIME_STATE", "per-player spell progression, availability, selection, and HUD state", "authoritative player spell state plus presentation selection", "505-byte nested spell-state block", "player-spell-state-block"),
    "Type_str_164::byte_0x154_340": ("RUNTIME_STATE", "bounded time since the previous spell release", "authoritative per-player cast payload", "unsigned byte counter capped at 200 and transferred to projectile payloads", "player-spell-charge-counter"),
    "type_str_611::SpellExperience_0x263_611x": ("RUNTIME_STATE", "packed per-family spell experience state", "authoritative player progression", "26-entry packed progression array", "player-spell-state-block"),
    "type_str_611::spellsExperience_0x2CB_715x": ("RUNTIME_STATE", "per-family spell experience values", "authoritative player progression", "26 signed 32-bit values", "player-spell-state-block"),
    "type_str_611::SpellsEnabled_0x333_819x": ("RUNTIME_STATE", "per-family enabled spell-holder entity links", "authoritative player inventory", "26 signed 16-bit entity indices", "player-spell-state-block"),
    "type_str_611::SpellIndexes_0x39B_923x": ("RUNTIME_STATE", "per-family spell index lookup", "authoritative player inventory", "26 signed 8-bit indices", "player-spell-state-block"),
    "type_str_611::SpellLevels_0x41D_1053z": ("RUNTIME_STATE", "per-family attained spell levels", "authoritative player progression", "26 signed 8-bit levels", "player-spell-state-block"),
    "type_str_611::array_0x437_1079x": ("RUNTIME_STATE", "ordered spell-index table used by selection UI", "player spell selection and presentation", "26 signed 8-bit indices", "player-spell-state-block"),
    "type_str_611::SpellIndexLeft_0x451_1105": ("ID_SELECTOR_MASK_OR_SENTINEL", "selected left-cast spell family", "authoritative player input selection", "signed 16-bit spell index or -1", "player-spell-state-block"),
    "type_str_611::SpellIndexRight_0x453_1107": ("ID_SELECTOR_MASK_OR_SENTINEL", "selected right-cast spell family", "authoritative player input selection", "signed 16-bit spell index or -1", "player-spell-state-block"),
    "type_str_611::SubSpellIndexLeft_1109": ("ID_SELECTOR_MASK_OR_SENTINEL", "selected left-cast spell tier", "authoritative player input selection", "signed 8-bit tier selector", "player-spell-state-block"),
    "type_str_611::SubSpellIndexRight_1110": ("ID_SELECTOR_MASK_OR_SENTINEL", "selected right-cast spell tier", "authoritative player input selection", "signed 8-bit tier selector", "player-spell-state-block"),
    "type_str_611::spellIndex_0x458_1112": ("ID_SELECTOR_MASK_OR_SENTINEL", "spell family currently being selected", "transient player selection state", "signed 8-bit spell index", "player-spell-state-block"),
    "type_str_611::subSpellIndex_0x459_1113": ("ID_SELECTOR_MASK_OR_SENTINEL", "spell tier currently being selected", "transient player selection state", "signed 8-bit tier selector", "player-spell-state-block"),
    "type_str_3654C::str_36552_un": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "objective-stage scalar or entity-reference payload", "authoritative interpretation selected by objective kind", "four-byte scalar, authored entity pointer, live entity pointer, or 16-bit content pointer", "objective-polymorphic-payload"),
    "un_str_36552::dword": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "objective-kind-selected scalar view", "authoritative interpretation selected by objective kind", "32-bit mana threshold, player color, entity index, subtype/list selector, or building-chain head", "objective-polymorphic-payload"),
    "un_str_36552::ptr0x30311": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "objective-kind-selected authored entity-reference view", "authoritative content reference before live binding", "pointer into authored level entity records", "objective-polymorphic-payload"),
    "un_str_36552::ptr0x6E8E": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "objective-kind-selected live entity-reference view", "authoritative runtime reference after binding", "pointer into live runtime entity pool", "objective-polymorphic-payload"),
    "un_str_36552::ptr16u": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "objective-kind-selected 16-bit authored content-reference view", "authoritative content reference", "pointer to leading 16-bit field of authored entity record", "objective-polymorphic-payload"),
    "type_str_3654C::stages_3654C_byte0": ("ID_SELECTOR_MASK_OR_SENTINEL", "objective predicate kind selector", "authoritative objective evaluation selector", "unsigned kind 0 through 9", "objective-kind-and-runtime-flags"),
    "type_str_3654C::str_3654D_byte1": ("RUNTIME_STATE", "objective live-binding and force-complete flags", "authoritative objective runtime state", "bit 0 reference bound; bit 1 force-complete pulse; other bits unresolved", "objective-kind-and-runtime-flags"),
    "type_shadow_str_3654C::str_36552_un": ("CONTEXT_DEPENDENT_STORAGE_VIEW", "serialized objective-stage payload", "persistence representation translated during load and save", "four-byte stored offset, scalar, or packed pointer representation", "objective-polymorphic-payload"),
    "type_shadow_str_3654C::str_3654D_byte1": ("CONTEXT_DEPENDENT_STORAGE_VIEW", "serialized objective runtime-flag byte", "persistence representation", "stored objective binding/completion flags", "objective-kind-and-runtime-flags"),
    "type_str_0x3647Ac::str_0x3647C_4": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "scripted stage-variable scalar, coordinate, or entity-reference payload", "authoritative interpretation selected by stage-variable kind", "four-byte scalar, axis pair, or live entity pointer", "level-stage-variable-records"),
    "type_shadow_str_0x3647Ac::str_0x3647C_4": ("CONTEXT_DEPENDENT_STORAGE_VIEW", "serialized scripted stage-variable payload", "persistence representation translated during load and save", "four-byte scalar, axis pair, or stored entity offset", "level-stage-variable-records"),
    "x_BYTE_EB39E_keys": ("PLATFORM_OR_COMPATIBILITY", "legacy host key-binding scancode table", "host configuration translated into local control input", "ten unsigned DOS-style scancodes", "host-key-binding-and-capture-state"),
    "type_x_DWORD_17DE38str::x_BYTE_17DF10_get_key_scancode": ("PLATFORM_OR_COMPATIBILITY", "currently captured key-rebinding scancode", "host frontend interaction state", "unsigned DOS-style scancode or zero", "host-key-binding-and-capture-state"),
    "type_x_DWORD_17DE38str::x_BYTE_17DF11_last_key_status": ("PLATFORM_OR_COMPATIBILITY", "most recently captured translated character code", "host frontend text-entry latch", "unsigned character or control code; zero when empty", "host-key-binding-and-capture-state"),
    "x_DWORD_E9C3C": ("PRESENTATION", "secondary off-screen 8-bit render-target pointer", "derived presentation buffer view", "mode-dependent interior pointer into a separately owned allocation", "secondary-render-target-buffer"),
    "type_x_DWORD_17DE38str::array_BYTE_17DE68x": ("RUNTIME_STATE", "eight multiplayer lobby synchronization and action records", "authoritative lobby/network state", "eight packed records containing update flag, mappings, opcode, and selected level", "multiplayer-lobby-action-records"),
    "type_x_DWORD_17DE38str::unk_17E078x": ("PRESENTATION", "campaign-map player marker animation state", "derived frontend presentation", "position, directional sprite, and animation frame range", "campaign-map-player-marker-animation"),
    "type_menuButtons_E1F84::str_26": ("PRESENTATION", "embedded scroll-dialog layout and animation state", "frontend presentation and interaction state", "position, target/current height, text/sprite mode, and close state", "scroll-dialog-animation-state"),
    "type_str_word_26::word_36_5": ("PRESENTATION", "scroll-dialog current animated open height", "transient frontend presentation", "signed height/progress moving toward target word_34_4", "scroll-dialog-animation-state"),
    "type_menuButtons_E1F84::byte_25": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "menu-entry terminator, enable discriminator, or joystick-dialog phase", "interpretation selected by owning menu flow", "unsigned byte including phase values 0 through 3", "menu-button-context-state"),
    "type_unk_F0E20x::dword_1": ("DERIVED_VALUE", "rolled-billboard destination framebuffer byte offset", "derived transient raster lookup", "signed byte offset", "billboard-raster-lookup"),
    "type_unk_F0E20x::dword_0": ("DERIVED_VALUE", "delta from prior rolled-billboard framebuffer offset", "derived transient raster lookup", "signed byte-offset step", "billboard-raster-lookup"),
    "type_unk_F0E20x::dword_2": ("DERIVED_VALUE", "rolled-billboard orthogonal step accumulator", "derived unused or dead raster cache in current renderers", "signed accumulated step/index", "billboard-raster-lookup"),
    "unk_180560x": ("PLATFORM_OR_COMPATIBILITY", "legacy analog gameport joystick sample and calibration packet", "host input compatibility state", "44-byte raw axes, calibration, mode, buttons, and normalized axes", "legacy-gameport-joystick-state"),
    "str_WORD_E20A4": ("PRESENTATION", "campaign-map ambient scripted animation table", "frontend presentation content plus current animation state", "twelve records including sentinel", "campaign-map-ambient-animations"),
    "type_WORD_E20A4::array_word_18": ("PRESENTATION", "campaign-map ambient route, sprite-sequence, and current animation words", "mixed presentation content and runtime state", "packed route parameters plus mutable position and sprite", "campaign-map-ambient-animations"),
    "x_DWORD_F42A0": ("DERIVED_VALUE", "ephemeral palette-channel arithmetic and clamp accumulator", "derived transient presentation", "signed channel scratch clamped to 0 through 63", "palette-channel-scratch"),
    "x_DWORD_17DE28str": ("PRESENTATION", "next-level objective text and speech presentation state machine", "campaign-map presentation state", "timestamps, portal position, and mode 0 through 3", "campaign-next-level-presentation"),
    "str_E1BAC": ("PRESENTATION", "sentinel-terminated main-menu button descriptors and live selection state", "frontend presentation and dispatch state", "button array with handler, geometry, sprites, action, and dialog", "main-menu-button-array"),
    "type_str_E2A74::dword_12": ("RUNTIME_STATE", "current entity candidate for entity-backed dynamic help", "authoritative local help-trigger association", "entity pointer or empty", "dynamic-help-trigger-state"),
    "type_str_E2A74::byte_29": ("RUNTIME_STATE", "dynamic help-popup priority tier", "local help scheduling state", "unsigned priority normally clamped 0 through 5 with forced value 6", "dynamic-help-trigger-state"),
    "type_str_E2A74::dword_20": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "dynamic-help nearest distance or special persistence counter", "interpretation selected by help entry", "signed distance or tick counter", "dynamic-help-trigger-state"),
    "type_D41A0_BYTESTR_0::byte_0x36E0B": ("RUNTIME_STATE", "secret-level and end-sequence lock flags", "authoritative progression and presentation coordination", "bit 0 secret level; bit 1 ending lock; other bits unresolved", "level-mode-flags"),
    "type_D41A0_BYTESTR_0::byte_0x36E02": ("PRESENTATION", "objective narration and presentation phase", "runtime objective presentation state", "small phase counter or state machine", "objective-presentation-state"),
    "type_D41A0_BYTESTR_0::byte_counter_current_objective_box_0x36E04": ("PRESENTATION", "remaining objective-textbox display ticks", "runtime objective presentation state", "unsigned tick countdown commonly set to 200", "objective-presentation-state"),
    "type_D41A0_BYTESTR_0::str_0x364D2": ("RUNTIME_STATE", "end-level creature and collectible-spell statistic denominators", "authoritative progression statistics", "creature total plus 26-entry pickup-subtype histogram", "end-level-statistic-denominators"),
    "type_str_0x2BDE::word_0x04d_2C2B_11307": ("PRESENTATION", "per-player notification remaining display ticks", "runtime HUD presentation state", "unsigned tick countdown", "player-notification-timer"),
    "type_str_0x2BDE::byte_0x006_2BE4_11236": ("RUNTIME_STATE", "per-player alive and participating-in-realm flag", "authoritative player participation state", "boolean-like byte", "player-active-in-realm-flag"),
    "Type_x_D41A0_BYTEARRAY_4_struct::byte_38591": ("PRESENTATION", "active audio-volume adjustment modal", "local UI interaction state", "0 none, 1 sound effects, 2 music", "audio-volume-modal-selector"),
    "type_x_DWORD_17DE38str::x_WORD_17DEEE_mouse_buttons": ("PLATFORM_OR_COMPATIBILITY", "menu-frame mouse-button bit mask", "host input snapshot", "bit 0 left and bit 1 right", "frontend-pointer-snapshot"),
    "type_x_DWORD_17DE38str::x_DWORD_17DEE4_mouse_positionx": ("PLATFORM_OR_COMPATIBILITY", "menu-frame normalized pointer X coordinate", "host input snapshot", "signed engine/menu-space coordinate", "frontend-pointer-snapshot"),
    "type_x_DWORD_17DE38str::x_DWORD_17DEE6_mouse_positiony": ("PLATFORM_OR_COMPATIBILITY", "menu-frame normalized pointer Y coordinate", "host input snapshot", "signed engine/menu-space coordinate", "frontend-pointer-snapshot"),
    "type_x_DWORD_17DE38str::x_DWORD_17DE48c": ("CONTEXT_DEPENDENT_STORAGE_VIEW", "current frontend asset arena data-base pointer", "presentation resource view", "mutable interior pointer into shared asset heap", "frontend-asset-pack-source-views"),
    "type_x_DWORD_17DE38str::x_DWORD_17DE54": ("CONTEXT_DEPENDENT_STORAGE_VIEW", "current frontend sprite pixel-data source base", "presentation resource view", "arena-relative DAT blob pointer", "frontend-asset-pack-source-views"),
    "type_x_DWORD_17DE38str::x_DWORD_17DEC0": ("CONTEXT_DEPENDENT_STORAGE_VIEW", "first current frontend sprite table source stream", "presentation resource view", "arena-relative raw TAB stream pointer", "frontend-asset-pack-source-views"),
    "type_x_DWORD_17DE38str::x_DWORD_17DEC4": ("CONTEXT_DEPENDENT_STORAGE_VIEW", "second current frontend sprite table source stream", "presentation resource view", "arena-relative raw TAB stream pointer", "frontend-asset-pack-source-views"),
    "type_x_DWORD_17DE38str::x_DWORD_17DE3C": ("CONTEXT_DEPENDENT_STORAGE_VIEW", "frontend palette and color-remap resource view", "presentation resource storage", "palette prefix followed by lookup and remap data", "frontend-color-remap-resource"),
    "x_WORD_E3760_mouse": ("PLATFORM_OR_COMPATIBILITY", "current normalized engine-space pointer coordinate", "host input translated into UI and player commands", "two signed engine-space coordinates", "normalized-engine-pointer-coordinate"),
    "type_x_DWORD_17DB70str::x_WORD_17DB8A": ("ID_SELECTOR_MASK_OR_SENTINEL", "current campaign-map level or portal selector", "campaign-map presentation and progression selection", "signed selector; -1 means no fixed selected portal", "campaign-map-level-selector"),
    "Str_E9C38_smalltit": ("PRESENTATION", "terrain projection and clipping tile buffer", "derived per-frame presentation", "fixed array of projected terrain vertices and texture flags", "terrain-projection-tile-buffer"),
    "GameRenderHD::m_ptrStr_E9C38_smalltit": ("PRESENTATION", "HD renderer terrain-projection buffer pointer", "derived per-frame presentation", "pointer to projected terrain tile records", "terrain-projection-tile-buffer"),
    "x_DWORD_E9C38_smalltit": ("PRESENTATION", "shared animation or frame byte buffer", "presentation resource storage", "byte buffer pointer", "terrain-projection-tile-buffer"),
    "str_unk_1804B0ar": ("PRESENTATION", "HUD text-box, popup, and score presentation state", "presentation and local interaction state", "packed 171-byte UI aggregate", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::type_sub_0": ("PRESENTATION", "two HUD text-box layout records", "presentation", "two packed text layout records", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::PopupStatusByte_0x9e": ("PRESENTATION", "HUD popup status flags", "presentation and local interaction state", "bit mask", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::Index_0x86": ("ID_SELECTOR_MASK_OR_SENTINEL", "active HUD popup or UI actor index", "presentation linkage", "signed 16-bit index", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::byte_0x9f": ("PRESENTATION", "secondary HUD popup status flags", "presentation and local interaction state", "bit mask", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::word_0x88": ("PRESENTATION", "HUD popup timing or layout word", "presentation", "signed 16-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::byte_0xa8": ("ID_SELECTOR_MASK_OR_SENTINEL", "wizard number used by HUD score text", "presentation selector", "signed 8-bit wizard index", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::showPlayerScores_0xa7": ("PRESENTATION", "show-player-scores flag", "presentation", "boolean-like byte", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::drawTextType_0xa3": ("ID_SELECTOR_MASK_OR_SENTINEL", "HUD text drawing mode", "presentation selector", "signed 8-bit mode", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::uni_0x8a": ("PRESENTATION", "packed HUD popup layout values", "presentation", "overlapping dword/word layout values", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::dword_0x66": ("PRESENTATION", "HUD popup working value at 0x66", "presentation", "signed 32-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::dword_0x6a": ("PRESENTATION", "HUD popup working value at 0x6a", "presentation", "signed 32-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::dword_0x6e": ("PRESENTATION", "HUD popup working value at 0x6e", "presentation", "signed 32-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::dword_0x72": ("PRESENTATION", "HUD popup working value at 0x72", "presentation", "signed 32-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::dword_0x76": ("PRESENTATION", "HUD popup working value at 0x76", "presentation", "signed 32-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::dword_0x7a": ("PRESENTATION", "HUD popup working value at 0x7a", "presentation", "signed 32-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::dword_0x82": ("PRESENTATION", "HUD popup working value at 0x82", "presentation", "signed 32-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::word_0x96": ("PRESENTATION", "HUD popup working word at 0x96", "presentation", "signed 16-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::word_0x98": ("PRESENTATION", "HUD popup working word at 0x98", "presentation", "signed 16-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::word_0x9a": ("PRESENTATION", "HUD popup working word at 0x9a", "presentation", "signed 16-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::word_0x9c": ("PRESENTATION", "HUD popup working word at 0x9c", "presentation", "signed 16-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::byte_0xa0": ("PRESENTATION", "HUD popup working flag at 0xa0", "presentation", "signed 8-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::byte_0xa1": ("PRESENTATION", "HUD popup working flag at 0xa1", "presentation", "signed 8-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::byte_0xa2": ("PRESENTATION", "HUD popup working flag at 0xa2", "presentation", "signed 8-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::byte_0xa4": ("PRESENTATION", "HUD popup working flag at 0xa4", "presentation", "signed 8-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::byte_0xa5": ("PRESENTATION", "HUD popup working flag at 0xa5", "presentation", "signed 8-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::byte_0xa6": ("PRESENTATION", "HUD popup working flag at 0xa6", "presentation", "signed 8-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::byte_0xa9": ("PRESENTATION", "HUD popup working flag at 0xa9", "presentation", "signed 8-bit value", "hud-popup-textbox-state"),
    "type_str_unk_1804B0ar::byte_0xaa": ("PRESENTATION", "HUD popup working flag at 0xaa", "presentation", "signed 8-bit value", "hud-popup-textbox-state"),
    "x_WORD_180660_VGA_type_resolution": ("PLATFORM_OR_COMPATIBILITY", "legacy VGA/display resolution mode", "host display configuration", "16-bit mode and flag field", "video-resolution-mode"),
    "GameSettingsStruct_t::str_0x2192": ("PRESENTATION", "local smoothing, stereo, legacy-resolution mirror, and reserved-shadow option block", "host presentation configuration", "four packed option bytes", "local-visual-option-blocks"),
    "GameSettingsStruct_t::str_0x2196": ("PRESENTATION", "local dynamic-lighting, reserved, HUD-transparency, and flat-shading option block", "host presentation configuration", "four packed option bytes", "local-visual-option-blocks"),
    "type_D41A0_BYTESTR_0::str_0x21AA": ("PLATFORM_OR_COMPATIBILITY", "reflection, shadow, sky, and unresolved graphics capability flags", "host presentation capability gates", "four packed availability bytes", "graphics-option-capability-blocks"),
    "type_D41A0_BYTESTR_0::str_0x21AE": ("PLATFORM_OR_COMPATIBILITY", "graphics capability block with proven blur-availability final byte", "host presentation capability gates", "four packed bytes; first three unresolved", "graphics-option-capability-blocks"),
    "type_D41A0_BYTESTR_0::str_0x21B6": ("PLATFORM_OR_COMPATIBILITY", "dynamic-light, transparency, flat-shading, and unresolved capability flags", "host presentation capability gates", "four packed availability bytes", "graphics-option-capability-blocks"),
    "x_WORD_E36D4": ("PRESENTATION", "legacy software bitmap-blitter mode and effect mask", "transient presentation state", "bit mask including colorized draw bit 0x40", "bitmap-blitter-mode"),
    "x_DWORD_18073A": ("PLATFORM_OR_COMPATIBILITY", "cursor blit loop and bank-selection scratch", "legacy software/VGA host state", "low-word loop counter and high-word VGA bank in banked paths", "legacy-vga-cursor-scratch"),
    "GameRenderOriginal::x_BYTE_D4750": ("INITIAL_CONDITION_OR_CONTENT", "immutable directional sprite and frame-offset lookup", "compiled presentation table", "60 unsigned orientation/frame entries", "directional-sprite-lookup"),
    "GameRenderHD::x_BYTE_D4750": ("INITIAL_CONDITION_OR_CONTENT", "immutable directional sprite and frame-offset lookup", "compiled presentation table", "60 unsigned orientation/frame entries", "directional-sprite-lookup"),
    "GameRenderNG::x_BYTE_D4750": ("INITIAL_CONDITION_OR_CONTENT", "immutable directional sprite and frame-offset lookup", "compiled presentation table", "60 unsigned orientation/frame entries", "directional-sprite-lookup"),
    "type_E9C08::dword_2": ("RUNTIME_STATE", "owning pointer to fixed-capacity texture-animation descriptors", "structural runtime resource state", "pointer to 504-entry descriptor array", "texture-animation-descriptor-array"),
    "type_x_DWORD_E9C28_str::str_8_data": ("RUNTIME_STATE", "fixed-capacity texture-heap allocation descriptor table", "structural runtime resource state", "inline or separately allocated descriptor-array pointer", "texture-heap-descriptor-table"),
    "type_str_0x3647Ac::index_0x3647A_0": ("ID_SELECTOR_MASK_OR_SENTINEL", "stage-variable kind selector with normalized initialization flags", "authoritative script/content selector", "low-nibble kind plus normalized runtime flags", "stage-variable-kind-selector"),
    "bitmap_pos_struct_t::data": ("PRESENTATION", "derived pointer to indexed bitmap or sprite pixel data", "runtime asset view rebuilt from DAT base and TAB offset", "pointer to unsigned 8-bit pixels", "derived-bitmap-pixel-view"),
    "type_particle_str::word_0": ("PRESENTATION", "texture header and runtime animation flags", "asset metadata plus transient presentation state", "bit 0 animated, bit 3 recently rendered, bit 5 grouped update; other bits unresolved", "texture-header-runtime-flags"),
    "GameRenderOriginal::x_BYTE_E126D": ("PRESENTATION", "current software polygon raster opcode and interpolation mode", "transient per-polygon rasterizer state", "unsigned mode selector including 5, 7, and 26", "software-rasterizer-mode"),
    "GameRenderHD::x_BYTE_E126D": ("PRESENTATION", "current software polygon raster opcode and interpolation mode", "transient per-polygon rasterizer state", "unsigned mode selector including 5, 7, and 26", "software-rasterizer-mode"),
    "GameRenderNG::x_BYTE_E126D": ("PRESENTATION", "current software polygon raster opcode and interpolation mode", "transient per-polygon rasterizer state", "unsigned mode selector including 5, 7, and 26", "software-rasterizer-mode"),
    "str_TMAPS00TAB_BEGIN_BUFFER": ("INITIAL_CONDITION_OR_CONTENT", "loaded texture-map metadata table", "asset metadata", "array of offset and grouping records", "texture-map-metadata-and-cache"),
    "type_TMAPS00TAB_BEGIN_BUFFER::dword_0": ("INITIAL_CONDITION_OR_CONTENT", "texture-map metadata value at offset 0", "asset metadata", "32-bit metadata value", "texture-map-metadata-and-cache"),
    "type_TMAPS00TAB_BEGIN_BUFFER::dword_4": ("INITIAL_CONDITION_OR_CONTENT", "texture-map data-file offset", "asset metadata", "32-bit byte offset", "texture-map-metadata-and-cache"),
    "type_TMAPS00TAB_BEGIN_BUFFER::word_8": ("ID_SELECTOR_MASK_OR_SENTINEL", "texture animation or allocation group index", "asset metadata selector", "unsigned 16-bit index", "texture-map-metadata-and-cache"),
    "str_DWORD_F66F0x": ("PRESENTATION", "runtime texture-map frame pointer cache", "derived loaded presentation resources", "504 frame-pointer entries", "texture-map-metadata-and-cache"),
    "x_DWORD_F5730": ("PRESENTATION", "texture-map frame last-use timestamps", "derived cache bookkeeping", "504 frame counters", "texture-map-metadata-and-cache"),
    "particlesParameters_D951C": ("INITIAL_CONDITION_OR_CONTENT", "347-entry sprite and particle visual parameter table", "compiled content table with load-time adjustments", "347 packed 14-byte records", "particle-visual-parameter-table"),
    "type_WORD_D951C::word_0": ("ID_SELECTOR_MASK_OR_SENTINEL", "base texture-map frame index", "asset selector", "unsigned 16-bit texture index", "particle-visual-parameter-table"),
    "type_WORD_D951C::speed_6": ("GAMEPLAY_PARAMETER", "particle sprite vertical or frame dimension scale", "visual/effect parameter", "unsigned 16-bit fixed scale", "particle-visual-parameter-table"),
    "type_WORD_D951C::rotSpeed_8": ("GAMEPLAY_PARAMETER", "particle sprite horizontal or rotation dimension scale", "visual/effect parameter", "unsigned 16-bit fixed scale", "particle-visual-parameter-table"),
    "type_WORD_D951C::byte_10": ("INITIAL_CONDITION_OR_CONTENT", "particle visual parameter byte 10", "asset/effect metadata", "unsigned 8-bit value", "particle-visual-parameter-table"),
    "type_WORD_D951C::byte_11": ("INITIAL_CONDITION_OR_CONTENT", "particle level-preload grouping selector", "asset/effect metadata", "signed 8-bit selector", "particle-visual-parameter-table"),
    "type_WORD_D951C::byte_12": ("ID_SELECTOR_MASK_OR_SENTINEL", "particle color or shade lookup selector", "asset selector", "unsigned 8-bit lookup index", "particle-visual-parameter-table"),
    "Type_PlayerInput_0x6E3E::PlayerAction_byte0": ("ID_SELECTOR_MASK_OR_SENTINEL", "per-turn player command opcode", "authoritative deterministic input", "signed 8-bit action selector", "deterministic-player-command-record"),
    "Type_PlayerInput_0x6E3E::str_0x6E3E_byte1": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "first per-command byte argument", "authoritative deterministic input", "signed 8-bit opcode-dependent payload", "deterministic-player-command-record"),
    "Type_PlayerInput_0x6E3E::str_0x6E3E_byte2": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "second per-command byte argument", "authoritative deterministic input", "signed 8-bit opcode-dependent payload", "deterministic-player-command-record"),
    "Type_PlayerInput_0x6E3E::roll": ("RUNTIME_STATE", "per-turn quantized roll command", "authoritative deterministic input", "signed 8-bit flight input", "deterministic-player-command-record;flight-roll-input"),
    "Type_PlayerInput_0x6E3E::pitch": ("RUNTIME_STATE", "per-turn quantized pitch command", "authoritative deterministic input", "signed 8-bit flight input", "deterministic-player-command-record;flight-pitch-input"),
    "Type_PlayerInput_0x6E3E::entityIndex_0x6E3E_byte5": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "per-command flags or compact entity argument", "authoritative deterministic input", "signed 8-bit opcode-dependent payload", "deterministic-player-command-record"),
    "Type_PlayerInput_0x6E3E::nextEntity_0x6E3E_word6": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "first per-command entity or packed argument", "authoritative deterministic input", "unsigned 16-bit opcode-dependent payload", "deterministic-player-command-record"),
    "Type_PlayerInput_0x6E3E::entityIndex2_0x6E3E_word8": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "second per-command entity or packed argument", "authoritative deterministic input", "unsigned 16-bit opcode-dependent payload", "deterministic-player-command-record"),
    "type_D41A0_BYTESTR_0::byte_0x36DEA_fly_asistant": ("GAMEPLAY_PARAMETER", "local flight-assistant enable setting", "input-assistance configuration", "boolean-like byte", "flight-assistant-setting"),
    "x_WORD_180744_mouse_right_button": ("PLATFORM_OR_COMPATIBILITY", "latched host right-mouse-button press", "host input/frontend", "signed 16-bit boolean-like value", "host-mouse-button-latches"),
    "x_WORD_180746_mouse_left_button": ("PLATFORM_OR_COMPATIBILITY", "latched host left-mouse-button press", "host input/frontend", "signed 16-bit boolean-like value", "host-mouse-button-latches"),
    "type_str_0x2BDE::dword_0x3E6_2BE4_12228": ("RUNTIME_STATE", "embedded per-player flight, combat, camera, and spell runtime state", "authoritative player state", "Type_str_164 packed record", "per-player-runtime-state"),
    "Type_x_D41A0_BYTEARRAY_4_struct::dword_38519": ("RUNTIME_STATE", "live-entity linked-list traversal head", "authoritative entity-pool index", "entity pointer", "live-entity-list-head"),
    "x_DWORD_17DB70str": ("PRESENTATION", "campaign map-screen selection and portal animation state", "frontend presentation and local interaction", "56-byte packed map-screen aggregate", "campaign-map-screen-state"),
    "x_DWORD_181BF4": ("PLATFORM_OR_COMPATIBILITY", "legacy Miles AIL debug/logging enabled flag", "audio compatibility and diagnostics", "32-bit boolean-like flag", "legacy-ail-diagnostic-state"),
    "x_DWORD_181BF8": ("PLATFORM_OR_COMPATIBILITY", "legacy Miles AIL nested-call logging mode", "audio compatibility and diagnostics", "32-bit boolean-like flag", "legacy-ail-diagnostic-state"),
    "_str_0x6E8E::dword_0x10_16": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic entity payload at offset 0x10", "class/model/action dependent authoritative runtime", "signed 32-bit slot", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::byte_0x46_70": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic entity byte at offset 0x46", "class/model/action dependent authoritative runtime", "signed 8-bit slot; bitmap/sequence and other roles", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::word_0x96_150": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic entity word at offset 0x96", "class/model/action dependent authoritative runtime", "signed 16-bit slot; often a linked subentity", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::dword_0xA0_160x": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "class/model-specific settings or payload link at offset 0xA0", "class/model/action dependent authoritative runtime", "pointer", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::array_0x52_82": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic four-axis entity payload at offset 0x52", "class/model/action dependent authoritative runtime", "axis_4d packed values", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::str_0x5E_94": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic nested entity payload at offset 0x5E", "class/model/action dependent authoritative runtime", "packed nested structure", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::word_0x26_38": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic entity word at offset 0x26", "class/model/action dependent authoritative runtime", "signed 16-bit slot", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::byte_0x3E_62": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic entity selector byte at offset 0x3E", "class/model/action dependent authoritative runtime", "unsigned 8-bit slot", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::word_0x2E_46": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic entity word at offset 0x2E", "class/model/action dependent authoritative runtime", "signed 16-bit slot", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::word_0x34_52": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic entity word at offset 0x34", "class/model/action dependent authoritative runtime", "signed 16-bit slot", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::word_0x2C_44": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic entity word at offset 0x2C", "class/model/action dependent authoritative runtime", "signed 16-bit slot", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::byte_0x44_68": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic entity byte at offset 0x44", "class/model/action dependent authoritative runtime", "signed 8-bit slot", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::byte_0x43_67": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic entity byte at offset 0x43", "class/model/action dependent authoritative runtime", "signed 8-bit slot", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::word_0x36_54": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic entity word at offset 0x36", "class/model/action dependent authoritative runtime", "signed 16-bit slot", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::word_0x24_36": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic entity word at offset 0x24", "class/model/action dependent authoritative runtime", "signed 16-bit slot", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::word_0x30_48": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic entity word at offset 0x30", "class/model/action dependent authoritative runtime", "signed 16-bit slot", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::byte_0x38_56": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic entity byte at offset 0x38", "class/model/action dependent authoritative runtime", "signed 8-bit slot", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::byte_0x39_57": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic entity byte at offset 0x39", "class/model/action dependent authoritative runtime", "signed 8-bit slot", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::word_0x32_50": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic entity word at offset 0x32", "class/model/action dependent authoritative runtime", "signed 16-bit slot", "entity-polymorphic-payload-slots"),
    "_str_0x6E8E::word_0x5A_90": ("CONTEXT_DEPENDENT_RUNTIME_SLOT", "polymorphic entity word at offset 0x5A", "class/model/action dependent authoritative runtime", "signed 16-bit slot", "entity-polymorphic-payload-slots"),
}

# Curated architectural landmarks. These are deliberately sparse: the complete
# denominator remains function-semantic-map.csv. This registry makes the
# recovered program traversable from boot through simulation and presentation
# without pretending that every helper below each landmark is understood.
SYSTEM_SURFACES = [
    ("boot", 10, "sub_main", "native process entry and top-level compatibility lifecycle", "PROVISIONAL"),
    ("session", 20, "sub_46830_main_loop", "outer frontend, level, play, and transition scheduler", "STRONG"),
    ("spells", 25, "SetDefaultSpells_5C0A0", "install compiled spell seed before loaded-data and level patches", "STRONG"),
    ("level", 30, "LoadLevel_555D0", "load saved or packaged level state", "PROVISIONAL"),
    ("level", 40, "LevelDecompress_533B0", "expand loaded level representation into runtime working state", "PROVISIONAL"),
    ("level", 50, "LevelInitGame_56A30", "initialize gameplay state around a selected level", "PROVISIONAL"),
    ("level", 60, "LevelInit_56C00", "apply level and day/night/cave initialization patches", "STRONG"),
    ("session", 80, "InGameLoop_47320", "authoritative in-game loop boundary", "STRONG"),
    ("tick", 90, "DrawAndEventsInGame_47560", "ordered per-frame gameplay and draw orchestration", "STRONG"),
    ("input", 100, "PlayerEvents_51BB0", "network/replay boundary and per-player action dispatch", "STRONG"),
    ("flight", 110, "sub_5F380", "decode current input into player movement controls", "STRONG"),
    ("flight", 120, "sub_5D530", "ordered carpet flight integrator and collision response", "STRONG"),
    ("flight", 130, "AddPlayer03_00_5E010", "apply player/carpet update slice", "PROVISIONAL"),
    ("player", 135, "sub_5C950", "spawn or respawn a player/AI wizard and initialize its state", "STRONG"),
    ("flight", 140, "MoveEntity_57FA0", "fixed-point trigonometric entity displacement primitive", "STRONG"),
    ("camera", 150, "sub_57B20", "copy controlled-entity state into the active player camera slot", "STRONG"),
    ("events", 160, "GenerateEvents_49290", "generate runtime entities/events from level records", "PROVISIONAL"),
    ("events", 170, "PrepareEvents_49540", "prepare level events for activation", "PROVISIONAL"),
    ("events", 180, "ApplyEvents_498A0", "apply prepared level events to runtime state", "PROVISIONAL"),
    ("entities", 190, "NewEvent_4A050", "allocate and initialize a runtime entity record", "PROVISIONAL"),
    ("entities", 200, "IfSubtypeCallCreatingManaSphere_4A190", "dispatch entity constructor by class and model", "STRONG"),
    ("entities", 210, "UpdateEntities_57730", "walk the entity pool and dispatch current actions", "STRONG"),
    ("entities", 220, "DisableEntityDrawing04_57F10", "retire or hide an entity from rendering", "PROVISIONAL"),
    ("terrain", 230, "getTerrainAlt_10C40", "interpolate authoritative terrain height", "STRONG"),
    ("terrain", 240, "sub_104D0_terrain_tile_is_water", "query terrain water/material state", "STRONG"),
    ("spells", 250, "GetSpellManaCost_6D710", "obtain current tier mana cost", "PROVISIONAL"),
    ("lighting", 252, "AddEvent2_847D0", "register a runtime entity as one of fifty night/cave dynamic lights", "STRONG"),
    ("lighting", 254, "sub_848A0", "remove previous dynamic-light contributions before simulation movement", "STRONG"),
    ("lighting", 256, "sub_84B80", "follow light owners and apply current 5x5 shading contributions", "STRONG"),
    ("render", 260, "DrawGameFrame_2BE30", "draw the current world and game frame", "PROVISIONAL"),
    ("hud", 270, "DrawMinimap_63600", "draw tactical minimap and its overlays", "PROVISIONAL"),
    ("audio", 280, "PlayEntitySounds_6E150", "select and emit entity-associated sounds", "PROVISIONAL"),
    ("audio", 290, "PrepareEventSound_6E450", "prepare a positional/event sound for playback", "PROVISIONAL"),
    ("music", 300, "StartMusic_8E160", "start or change streamed/MIDI music playback", "PROVISIONAL"),
    ("music", 310, "StopMusic_8E020", "stop current music playback", "PROVISIONAL"),
    ("persistence", 320, "SaveLevel_55080", "serialize current level/game state", "PROVISIONAL"),
    ("progression", 322, "sub_57570", "promote objective completion into the level-end transition state", "STRONG"),
    ("progression", 324, "sub_575C0", "conditionally restore a saved single-player checkpoint", "PROVISIONAL"),
    ("replay", 330, "RecordPlayerActions", "capture deterministic per-turn input bytes", "STRONG"),
    ("replay", 340, "SaveRecordingToFile", "persist deterministic input recording", "STRONG"),
    ("replay", 350, "LoadRecordingFile", "load deterministic input recording for playback", "STRONG"),
    ("regression", 360, "SaveCompare", "save native state checkpoint for exact-build comparison", "STRONG"),
    ("regression", 370, "CompareWith", "compare native state against oracle checkpoint", "STRONG"),
    ("network", 380, "NetworkSendMessage_74EF1", "chunk and send one reconstructed legacy network message", "STRONG"),
    ("network", 390, "NetworkReceiveMessage_74D41", "receive and reassemble one reconstructed legacy network message", "STRONG"),
    ("determinism", 400, "watcomsrand", "seed the Watcom-compatible pseudo-random generator", "STRONG"),
    ("determinism", 410, "watcomrand", "advance the Watcom-compatible pseudo-random generator", "STRONG"),
]

# Exact lexical order inside DrawAndEventsInGame_47560. Conditional and
# repeated stages carry their conditions explicitly; this is the closest
# static answer to the authoritative tick order without a runtime trace.
TICK_PIPELINE = [
    (10, "PaletteChanges_47760", "presentation", "always", "advance palette transitions"),
    (20, "sub_715B0", "presentation", "when not paused", "advance sprite animation"),
    (30, "ReadGameUserInputs_89D10", "input", "always", "sample host keyboard/mouse input"),
    (40, "MouseAndKeysEvents_17A00", "input", "always", "translate host input into turn actions"),
    (50, "PlayerEvents_51BB0", "authoritative", "always", "network/replay action boundary and player action update"),
    (60, "sub_848A0", "presentation", "night/cave with active dynamic lights", "remove prior dynamic-light contributions from the shading grid"),
    (70, "UpdateEntities_57730", "authoritative", "speed mode dependent", "advance entity AI/actions; may run 0, 1, 4, or 8 times"),
    (80, "sub_84B80", "derived_presentation", "night/cave with active dynamic lights", "move, retire, and reapply dynamic-light contributions"),
    (90, "sub_58F00_game_objectives", "authoritative", "always", "evaluate level objective predicates"),
    (100, "PresentObjective_59820", "presentation", "always", "present current objective state"),
    (110, "sub_57570", "authoritative", "when not paused", "promote completed objective state to the level-end flag"),
    (120, "sub_575C0", "authoritative", "when defeat/reload and checkpoint flags are set", "conditionally restore the saved single-player checkpoint"),
    (130, "PlayEntitySounds_6E150", "presentation", "always", "select and emit entity sounds"),
    (140, "DrawGameFrame_2BE30", "presentation", "always", "render the final game frame"),
]


@dataclass(frozen=True)
class Anchor:
    claim_id: str
    namespace: str
    source_file: str
    start: int
    end: int
    confidence: str


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def write_csv(path: Path, rows: list[dict[str, object]]) -> None:
    if not rows:
        raise ValueError(f"refusing to write empty output: {path}")
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)


def normalized_file(path: str) -> str:
    return path.replace("\\", "/").removeprefix("./")


def same_source_file(function_file: str, anchor_file: str) -> bool:
    left = normalized_file(function_file)
    right = normalized_file(anchor_file)
    return left == right or left.endswith("/" + right) or right.endswith("/" + left)


def claim_anchors(claims: list[dict[str, str]]) -> list[Anchor]:
    output: list[Anchor] = []
    for claim in claims:
        for match in ANCHOR_RE.finditer(claim.get("evidence_anchors", "")):
            start = int(match.group("start"))
            output.append(
                Anchor(
                    claim_id=claim["claim_id"],
                    namespace=claim["namespace"],
                    source_file=match.group("file"),
                    start=start,
                    end=int(match.group("end") or start),
                    confidence=claim["confidence"],
                )
            )
    return output


def function_intervals(
    functions: list[dict[str, str]],
) -> dict[str, tuple[int, int]]:
    by_file: dict[str, list[dict[str, str]]] = defaultdict(list)
    for row in functions:
        by_file[row["file"]].append(row)
    intervals: dict[str, tuple[int, int]] = {}
    for rows in by_file.values():
        rows.sort(key=lambda row: (int(row["line"]), row["function_usr"]))
        source_path = UPSTREAM / rows[0]["file"]
        if not source_path.is_file():
            raise ValueError(f"function source file does not exist: {source_path}")
        source_line_count = len(
            source_path.read_text(encoding="utf-8", errors="replace").splitlines()
        )
        distinct_lines = sorted({int(row["line"]) for row in rows})
        next_line = {
            line: (
                distinct_lines[index + 1]
                if index + 1 < len(distinct_lines)
                else source_line_count + 1
            )
            for index, line in enumerate(distinct_lines)
        }
        for row in rows:
            start = int(row["line"])
            intervals[row["function_usr"]] = (start, next_line[start] - 1)
    return intervals


def validate_anchor_bounds(
    anchors: list[Anchor], functions: list[dict[str, str]]
) -> None:
    function_files = sorted({row["file"] for row in functions})
    line_count_cache: dict[Path, int] = {}
    errors: list[str] = []
    for anchor in anchors:
        candidates = [
            UPSTREAM / relative
            for relative in function_files
            if same_source_file(relative, anchor.source_file)
        ]
        if not candidates:
            candidates = [
                path
                for path in UPSTREAM.rglob(Path(anchor.source_file).name)
                if same_source_file(
                    str(path.relative_to(UPSTREAM)), anchor.source_file
                )
            ]
        candidates = sorted(set(candidates))
        if not candidates:
            errors.append(
                f"{anchor.claim_id}:{anchor.source_file}:{anchor.start}-{anchor.end} "
                "source file not found"
            )
            continue
        valid = False
        bounds: list[int] = []
        for path in candidates:
            if path not in line_count_cache:
                line_count_cache[path] = len(
                    path.read_text(encoding="utf-8", errors="replace").splitlines()
                )
            line_count = line_count_cache[path]
            bounds.append(line_count)
            if 1 <= anchor.start <= anchor.end <= line_count:
                valid = True
        if not valid:
            errors.append(
                f"{anchor.claim_id}:{anchor.source_file}:{anchor.start}-{anchor.end} "
                f"outside source bounds {bounds}"
            )
    if errors:
        raise ValueError("invalid semantic evidence anchors:\n" + "\n".join(errors))


def recovery_kind(name: str, file: str, linked_claims: list[str]) -> tuple[str, str]:
    if linked_claims:
        return "EVIDENCE_LINKED", "STRONG_OR_CLAIM_SPECIFIC"
    if RAW_ADDRESS_RE.match(name):
        return "RAW_ADDRESS_ONLY", "MECHANICAL"
    if file.startswith(("remc2/lib/", "remc2/portability/", "remc2/utilities/")):
        return "PLATFORM_LIBRARY_NAMED", "MECHANICAL"
    if RECOVERED_ADDRESS_RE.search(name):
        return "RECOVERED_DESCRIPTIVE_NAME", "PROVISIONAL"
    return "DESCRIPTIVE_NAME", "PROVISIONAL"


def recovered_address(name: str) -> str:
    match = RAW_ADDRESS_RE.match(name) or RECOVERED_ADDRESS_RE.search(name)
    return match.group(1).upper() if match else ""


def priority_score(row: dict[str, str], kind: str) -> int:
    calls = int(row["incoming_call_edges"]) + int(row["outgoing_call_edges"])
    state = (
        int(row["read_edges"])
        + int(row["write_edges"])
        + int(row["readwrite_edges"])
        + int(row["possible_pointer_or_escape_edges"])
    )
    bulk = int(row["bulk_operation_count"])
    score = round(12 * math.log2(1 + calls) + 10 * math.log2(1 + state) + 5 * bulk)
    if row["primary_subsystem"] in GAMEPLAY_SUBSYSTEMS:
        score += 25
    if kind == "RAW_ADDRESS_ONLY":
        score += 20
    return score


def next_action(kind: str, linked_claims: list[str], subsystem: str) -> str:
    if linked_claims:
        return "audit claim boundaries and promote only with runtime or independent corroboration"
    if kind == "RAW_ADDRESS_ONLY" and subsystem in GAMEPLAY_SUBSYSTEMS:
        return "trace callers, callees, state fields, dispatch selectors, and asset/data lineage"
    if kind == "RAW_ADDRESS_ONLY":
        return "identify host/support role or prove gameplay reachability before deep analysis"
    if kind in {"RECOVERED_DESCRIPTIVE_NAME", "DESCRIPTIVE_NAME"}:
        return "verify descriptive name against behavior before treating it as semantic proof"
    return "retain as named support surface unless it enters authoritative simulation"


def state_leaf_name(name: str) -> str:
    return name.rsplit("::", 1)[-1]


def state_semantic_maturity(name: str) -> str:
    leaf = state_leaf_name(name)
    semantic_stem = OFFSET_SUFFIX_RE.sub("", leaf)
    if RAW_STATE_TOKEN_RE.match(semantic_stem):
        return "RAW_STORAGE_OR_OFFSET"
    if semantic_stem != leaf:
        return "DESCRIPTIVE_NAME_WITH_PRESERVED_OFFSET"
    return "DESCRIPTIVE_NAME"


def state_priority(row: dict[str, str], maturity: str) -> int:
    edges = int(row["access_edge_count"])
    functions = int(row["touching_function_count"])
    score = round(15 * math.log2(1 + edges) + 10 * math.log2(1 + functions))
    if maturity == "RAW_STORAGE_OR_OFFSET":
        score += 20
    if any(item in GAMEPLAY_SUBSYSTEMS for item in row["touching_subsystems"].split("|")):
        score += 20
    if int(row["write_edges"]) + int(row["readwrite_edges"]) > 0:
        score += 15
    if row["state_region_seed"] == "U0_unclassified_or_shared_runtime":
        score += 10
    return score


def main() -> int:
    functions = read_csv(FUNCTION_MATRIX)
    state_targets = read_csv(STATE_TARGETS)
    seeds = {row["function_usr"]: row["seed_cluster"] for row in read_csv(SUBSYSTEM_SEEDS)}
    claims = read_csv(CLAIMS)
    dispatch_summary = json.loads(DISPATCH_SUMMARY.read_text(encoding="utf-8"))
    if (
        dispatch_summary["declared_dispatch_row_denominator"]
        != dispatch_summary["mechanically_parsed_dispatch_rows"]
    ):
        raise ValueError("runtime dispatch atlas does not close its declared denominator")
    state_target_names = {row["target_name"] for row in state_targets}
    missing_state_semantics = sorted(set(STATE_SEMANTICS) - state_target_names)
    if missing_state_semantics:
        raise ValueError(
            "curated state semantics no longer resolve in the atlas: "
            + ", ".join(missing_state_semantics)
        )
    claim_ids = {row["claim_id"] for row in claims}
    missing_state_claims = sorted(
        {
            claim_id
            for semantic in STATE_SEMANTICS.values()
            for claim_id in semantic[4].split(";")
            if claim_id and claim_id not in claim_ids
        }
    )
    if missing_state_claims:
        raise ValueError(
            "curated state semantics reference unknown claims: "
            + ", ".join(missing_state_claims)
        )
    anchors = claim_anchors(claims)
    validate_anchor_bounds(anchors, functions)
    intervals = function_intervals(functions)

    links_by_function: dict[str, list[Anchor]] = defaultdict(list)
    claim_link_rows: list[dict[str, object]] = []
    for function in functions:
        start, end = intervals[function["function_usr"]]
        for anchor in anchors:
            if not same_source_file(function["file"], anchor.source_file):
                continue
            if anchor.end < start or anchor.start > end:
                continue
            links_by_function[function["function_usr"]].append(anchor)
            claim_link_rows.append(
                {
                    "claim_id": anchor.claim_id,
                    "claim_namespace": anchor.namespace,
                    "claim_confidence": anchor.confidence,
                    "function_usr": function["function_usr"],
                    "function_name": function["name"],
                    "function_file": function["file"],
                    "function_start_line": start,
                    "function_end_line": end,
                    "anchor_file": anchor.source_file,
                    "anchor_start_line": anchor.start,
                    "anchor_end_line": anchor.end,
                    "link_method": "source anchor intersects mechanical function interval",
                }
            )

    semantic_rows: list[dict[str, object]] = []
    for function in functions:
        linked = sorted({item.claim_id for item in links_by_function[function["function_usr"]]})
        kind, confidence = recovery_kind(function["name"], function["file"], linked)
        start, end = intervals[function["function_usr"]]
        score = priority_score(function, kind)
        semantic_rows.append(
            {
                "function_usr": function["function_usr"],
                "qualified_name": function["qualified_name"],
                "name": function["name"],
                "recovered_address_hex": recovered_address(function["name"]),
                "file": function["file"],
                "start_line": start,
                "end_line_inclusive": end,
                "primary_subsystem": function["primary_subsystem"],
                "subsystem_seed": seeds.get(function["function_usr"], "unclassified"),
                "semantic_maturity": kind,
                "semantic_confidence": confidence,
                "linked_claim_ids": ";".join(linked),
                "incoming_call_edges": function["incoming_call_edges"],
                "outgoing_call_edges": function["outgoing_call_edges"],
                "read_edges": function["read_edges"],
                "write_edges": function["write_edges"],
                "readwrite_edges": function["readwrite_edges"],
                "possible_pointer_or_escape_edges": function["possible_pointer_or_escape_edges"],
                "literal_count": function["literal_count"],
                "bulk_operation_count": function["bulk_operation_count"],
                "archaeology_priority_score": score,
                "next_action": next_action(kind, linked, function["primary_subsystem"]),
            }
        )

    semantic_rows.sort(
        key=lambda row: (
            str(row["primary_subsystem"]),
            -int(row["archaeology_priority_score"]),
            str(row["file"]),
            int(row["start_line"]),
        )
    )

    subsystem_rows: list[dict[str, object]] = []
    by_subsystem: dict[str, list[dict[str, object]]] = defaultdict(list)
    for row in semantic_rows:
        by_subsystem[str(row["primary_subsystem"])].append(row)
    for subsystem, rows in sorted(by_subsystem.items()):
        maturity = Counter(str(row["semantic_maturity"]) for row in rows)
        claim_linked = sum(bool(row["linked_claim_ids"]) for row in rows)
        raw = maturity["RAW_ADDRESS_ONLY"]
        subsystem_rows.append(
            {
                "subsystem": subsystem,
                "label": SUBSYSTEM_LABELS.get(subsystem, subsystem.replace("_", " ").title()),
                "function_denominator": len(rows),
                "mechanically_placed": len(rows),
                "evidence_linked_functions": claim_linked,
                "recovered_descriptive_names": maturity["RECOVERED_DESCRIPTIVE_NAME"],
                "other_descriptive_names": maturity["DESCRIPTIVE_NAME"],
                "platform_library_named": maturity["PLATFORM_LIBRARY_NAMED"],
                "raw_address_only": raw,
                "raw_address_percent": f"{100 * raw / len(rows):.1f}",
                "semantic_coverage_percent_claim_linked": f"{100 * claim_linked / len(rows):.1f}",
                "status": (
                    "SEMANTIC_CLAIMS_PRESENT"
                    if claim_linked
                    else "MECHANICAL_PLACEMENT_ONLY"
                ),
            }
        )

    gap_rows = [
        {
            "rank": rank,
            "function_usr": row["function_usr"],
            "name": row["name"],
            "recovered_address_hex": row["recovered_address_hex"],
            "primary_subsystem": row["primary_subsystem"],
            "file": row["file"],
            "start_line": row["start_line"],
            "incoming_call_edges": row["incoming_call_edges"],
            "outgoing_call_edges": row["outgoing_call_edges"],
            "state_edges": (
                int(row["read_edges"])
                + int(row["write_edges"])
                + int(row["readwrite_edges"])
                + int(row["possible_pointer_or_escape_edges"])
            ),
            "archaeology_priority_score": row["archaeology_priority_score"],
            "next_action": row["next_action"],
        }
        for rank, row in enumerate(
            sorted(
                (
                    row
                    for row in semantic_rows
                    if row["semantic_maturity"] == "RAW_ADDRESS_ONLY"
                ),
                key=lambda row: (
                    -int(row["archaeology_priority_score"]),
                    str(row["file"]),
                    int(row["start_line"]),
                ),
            ),
            start=1,
        )
    ]

    state_rows: list[dict[str, object]] = []
    for row in state_targets:
        semantic = STATE_SEMANTICS.get(row["target_name"])
        maturity = (
            "EVIDENCE_LINKED"
            if semantic is not None
            else state_semantic_maturity(row["target_name"])
        )
        touching = row["touching_subsystems"].split("|")
        writable = int(row["write_edges"]) + int(row["readwrite_edges"]) > 0
        gameplay = any(item in GAMEPLAY_SUBSYSTEMS for item in touching)
        score = state_priority(row, maturity)
        state_rows.append(
            {
                **row,
                "semantic_maturity": maturity,
                "authoritative_simulation_candidate": gameplay and writable,
                "semantic_confidence": "STRONG" if semantic else "MECHANICAL",
                "parameter_class": (
                    semantic[0] if semantic else "UNKNOWN_NOT_YET_CLASSIFIED"
                ),
                "semantic_label": semantic[1] if semantic else "",
                "authority": semantic[2] if semantic else "",
                "unit_or_scale": semantic[3] if semantic else "",
                "linked_claim_ids": semantic[4] if semantic else "",
                "archaeology_priority_score": score,
                "next_action": (
                    "verify initialization, load override, and representative runtime behavior"
                    if semantic
                    else
                    "partition writers/readers and trace initialization/load/patch/consumer lineage"
                    if gameplay and writable
                    else "retain as structural or presentation state until gameplay authority is proven"
                ),
            }
        )
    state_rows.sort(
        key=lambda row: (
            -int(row["archaeology_priority_score"]),
            str(row["target_kind"]),
            str(row["target_name"]),
        )
    )
    state_gap_rows = [
        {
            "rank": rank,
            "target_usr": row["target_usr"],
            "target_kind": row["target_kind"],
            "target_name": row["target_name"],
            "state_region_seed": row["state_region_seed"],
            "touching_subsystems": row["touching_subsystems"],
            "access_edge_count": row["access_edge_count"],
            "write_edges": row["write_edges"],
            "readwrite_edges": row["readwrite_edges"],
            "touching_function_count": row["touching_function_count"],
            "archaeology_priority_score": row["archaeology_priority_score"],
            "next_action": row["next_action"],
        }
        for rank, row in enumerate(
            (
                row
                for row in state_rows
                if row["semantic_maturity"] == "RAW_STORAGE_OR_OFFSET"
                and row["authoritative_simulation_candidate"]
            ),
            start=1,
        )
    ]

    functions_by_name: dict[str, list[dict[str, object]]] = defaultdict(list)
    for row in semantic_rows:
        functions_by_name[str(row["name"])].append(row)
    surface_rows: list[dict[str, object]] = []
    missing_surfaces: list[str] = []
    for phase, order, name, role, confidence in SYSTEM_SURFACES:
        matches = functions_by_name.get(name, [])
        if len(matches) != 1:
            missing_surfaces.append(f"{name}:{len(matches)}")
            continue
        row = matches[0]
        surface_rows.append(
            {
                "execution_order_hint": order,
                "phase": phase,
                "function_name": name,
                "recovered_address_hex": row["recovered_address_hex"],
                "role": role,
                "primary_subsystem": row["primary_subsystem"],
                "file": row["file"],
                "start_line": row["start_line"],
                "linked_claim_ids": row["linked_claim_ids"],
                "semantic_confidence": confidence,
                "guardrail": (
                    "order is an architectural reading aid, not proof that every "
                    "surface executes once or in this exact linear sequence"
                ),
            }
        )
    if missing_surfaces:
        raise ValueError(
            "system surface registry must resolve each function exactly once: "
            + ", ".join(missing_surfaces)
        )

    tick_rows: list[dict[str, object]] = []
    missing_tick_functions: list[str] = []
    for order, name, authority, condition, role in TICK_PIPELINE:
        matches = functions_by_name.get(name, [])
        if len(matches) != 1:
            missing_tick_functions.append(f"{name}:{len(matches)}")
            continue
        row = matches[0]
        tick_rows.append(
            {
                "tick_order": order,
                "function_name": name,
                "recovered_address_hex": row["recovered_address_hex"],
                "authority_class": authority,
                "execution_condition": condition,
                "role": role,
                "file": row["file"],
                "start_line": row["start_line"],
                "semantic_confidence": (
                    "STRONG"
                    if name
                    in {
                        "PlayerEvents_51BB0",
                        "UpdateEntities_57730",
                        "sub_58F00_game_objectives",
                    }
                    else "PROVISIONAL"
                ),
            }
        )
    if missing_tick_functions:
        raise ValueError(
            "tick pipeline must resolve each function exactly once: "
            + ", ".join(missing_tick_functions)
        )

    write_csv(OUTPUT / "function-semantic-map.csv", semantic_rows)
    write_csv(OUTPUT / "subsystem-semantic-coverage.csv", subsystem_rows)
    write_csv(OUTPUT / "semantic-gap-queue.csv", gap_rows)
    write_csv(OUTPUT / "claim-function-links.csv", claim_link_rows)
    write_csv(OUTPUT / "state-semantic-map.csv", state_rows)
    write_csv(OUTPUT / "state-gap-queue.csv", state_gap_rows)
    write_csv(OUTPUT / "system-surface-registry.csv", surface_rows)
    write_csv(OUTPUT / "simulation-tick-pipeline.csv", tick_rows)

    linked_claim_ids = {row["claim_id"] for row in claim_link_rows}
    function_maturity_counts = Counter(
        str(row["semantic_maturity"]) for row in semantic_rows
    )
    state_maturity_counts = Counter(
        str(row["semantic_maturity"]) for row in state_rows
    )
    state_parameter_class_counts = Counter(
        str(row["parameter_class"]) for row in state_rows
    )
    claim_confidence_counts = Counter(row["confidence"] for row in claims)
    claim_namespace_counts = Counter(row["namespace"] for row in claims)
    summary = {
        "schema_version": 1,
        "classification": "PRIVATE_ANALYSIS",
        "function_denominator": len(functions),
        "mechanically_placed_functions": len(semantic_rows),
        "raw_address_named_functions_total": sum(
            bool(RAW_ADDRESS_RE.match(str(row["name"]))) for row in semantic_rows
        ),
        "raw_address_functions_without_claim_links": sum(
            row["semantic_maturity"] == "RAW_ADDRESS_ONLY" for row in semantic_rows
        ),
        "raw_address_functions_with_claim_links": sum(
            bool(RAW_ADDRESS_RE.match(str(row["name"]))) and bool(row["linked_claim_ids"])
            for row in semantic_rows
        ),
        "descriptively_named_functions": sum(
            row["semantic_maturity"]
            in {
                "RECOVERED_DESCRIPTIVE_NAME",
                "DESCRIPTIVE_NAME",
                "PLATFORM_LIBRARY_NAMED",
            }
            for row in semantic_rows
        ),
        "evidence_linked_functions": sum(bool(row["linked_claim_ids"]) for row in semantic_rows),
        "claim_denominator": len(claims),
        "claims_with_source_function_links": len(linked_claim_ids),
        "claims_without_source_function_links": sorted(
            set(row["claim_id"] for row in claims) - linked_claim_ids
        ),
        "subsystem_denominator": len(by_subsystem),
        "function_semantic_maturity_counts": dict(
            sorted(function_maturity_counts.items())
        ),
        "state_semantic_maturity_counts": dict(sorted(state_maturity_counts.items())),
        "state_parameter_class_counts": dict(
            sorted(state_parameter_class_counts.items())
        ),
        "claim_confidence_counts": dict(sorted(claim_confidence_counts.items())),
        "claim_namespace_counts": dict(sorted(claim_namespace_counts.items())),
        "architectural_surface_count": len(surface_rows),
        "tick_pipeline_stage_count": len(tick_rows),
        "runtime_dispatch_row_denominator": dispatch_summary[
            "declared_dispatch_row_denominator"
        ],
        "mechanically_parsed_runtime_dispatch_rows": dispatch_summary[
            "mechanically_parsed_dispatch_rows"
        ],
        "runtime_dispatch_action_rows_including_sentinels": dispatch_summary[
            "action_rows_including_sentinels"
        ],
        "runtime_dispatch_constructor_rows_including_sentinels": dispatch_summary[
            "constructor_rows_including_sentinels"
        ],
        "runtime_constructor_non_sentinel_rows": dispatch_summary[
            "constructor_non_sentinel_rows"
        ],
        "runtime_constructor_non_sentinel_rows_with_semantic_labels": dispatch_summary[
            "constructor_non_sentinel_rows_with_semantic_labels"
        ],
        "runtime_constructor_non_sentinel_rows_without_semantic_labels": dispatch_summary[
            "constructor_non_sentinel_rows_without_semantic_labels"
        ],
        "runtime_action_non_sentinel_rows": dispatch_summary[
            "action_non_sentinel_rows"
        ],
        "runtime_action_non_sentinel_rows_with_semantic_labels": dispatch_summary[
            "action_non_sentinel_rows_with_semantic_labels"
        ],
        "runtime_dispatch_resolution_status_counts": dispatch_summary[
            "resolution_status_counts"
        ],
        "runtime_dispatch_enabled_unresolved_count": dispatch_summary[
            "enabled_unresolved_count"
        ],
        "state_target_denominator": len(state_rows),
        "mechanically_placed_state_targets": len(state_rows),
        "raw_storage_or_offset_state_targets": sum(
            row["semantic_maturity"] == "RAW_STORAGE_OR_OFFSET" for row in state_rows
        ),
        "evidence_linked_state_targets": sum(
            row["semantic_maturity"] == "EVIDENCE_LINKED" for row in state_rows
        ),
        "classified_state_targets": sum(
            row["parameter_class"] != "UNKNOWN_NOT_YET_CLASSIFIED"
            for row in state_rows
        ),
        "authoritative_simulation_state_candidates": sum(
            bool(row["authoritative_simulation_candidate"]) for row in state_rows
        ),
        "raw_authoritative_state_gap_count": len(state_gap_rows),
        "method_guardrail": (
            "mechanical placement and descriptive names are navigation aids; "
            "only evidence-linked claims carry claim-specific confidence"
        ),
        "outputs": {
            "function_semantic_map": "function-semantic-map.csv",
            "subsystem_semantic_coverage": "subsystem-semantic-coverage.csv",
            "semantic_gap_queue": "semantic-gap-queue.csv",
            "claim_function_links": "claim-function-links.csv",
            "state_semantic_map": "state-semantic-map.csv",
            "state_gap_queue": "state-gap-queue.csv",
            "system_surface_registry": "system-surface-registry.csv",
            "simulation_tick_pipeline": "simulation-tick-pipeline.csv",
        },
    }
    (OUTPUT / "semantic-coverage-summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    for path in (
        OUTPUT / "function-semantic-map.csv",
        OUTPUT / "subsystem-semantic-coverage.csv",
        OUTPUT / "semantic-gap-queue.csv",
        OUTPUT / "claim-function-links.csv",
        OUTPUT / "state-semantic-map.csv",
        OUTPUT / "state-gap-queue.csv",
        OUTPUT / "system-surface-registry.csv",
        OUTPUT / "simulation-tick-pipeline.csv",
        OUTPUT / "semantic-coverage-summary.json",
    ):
        path.chmod(0o600)
    Path(__file__).chmod(0o700)
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
