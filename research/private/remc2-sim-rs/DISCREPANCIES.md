# Known conformance divergences

Nothing in this file is silently normalized into a claim of parity. The
entries preserve historical investigations, terminology uncertainty, and
upstream harness defects. Resolved entries and unresolved semantic names are
not behavioral divergences in the closed bounded Level 1 profile. Current
status is governed by the canonical matrix: 182/182 Exact with 29 explicit
exclusions.

## DISC-001: Negative speed-effect arithmetic

- **Reference:** The reconstructed branches use a helper named `__CFSHL__`.
- **Our implementation:** The expression was traced through `my_sign32`, which
  is `-1` for negative operands and `0` otherwise. The Rust path now uses signed
  division by four with truncation toward zero and has no selectable arithmetic
  profile.
- **Impact:** Resolved; negative operands are covered directly.
- **Resolution:** RESOLVED by static source proof.
- **Tests affected:** `recovered_speed_effect_arithmetic_truncates_toward_zero`.
- **Review date:** 2026-07-24.

## DISC-002: Terrain storage and interpolation

- **Reference:** Ground, cave ceiling, material, and topology queries read the
  original terrain arrays and interpolation routines.
- **Our implementation:** `FlightWorld` defines the semantic provider contract,
  and `LegacyTerrain` implements the exact array samplers. It now loads original
  seven-surface DOSBox regression frames from the owner-local corpus.
- **Impact:** Resolved for the bounded ground/cave/material/angle provider.
- **Resolution:** RESOLVED by source port plus real regression-array fixtures.
- **Tests affected:** `owned_level_one_regression_frame_loads_real_original_arrays`,
  `owned_after_load_recording_matches_twenty_four_complete_flight_transitions`.
- **Review date:** 2026-07-24.

## DISC-003: Movement direction recovery helpers

- **Reference:** Movement redirection uses the reconstructed angle, angular
  distance, 3D distance, and rejected-recovery helpers.
- **Our implementation:** The exact 257-entry angle table, original quadrant
  branches, 224-entry square-root seed table, bit-scan/Newton loop, wrapped
  distance sums, and rejected recovery are ported.
- **Impact:** Resolved for the bounded flight helper domain.
- **Resolution:** RESOLVED by source port plus sampled full-`u32` property
  checks and signed-16 extreme coverage.
- **Tests affected:** `water_boundary_slides_along_a_dry_quantized_direction`.
- **Review date:** 2026-07-24.

## DISC-004: Material class naming

- **Reference:** Material class `1` is strongly identified as water, while the
  movement boundary tests exact class `0x100`.
- **Our implementation:** These remain separate as `WATER_AMBIENCE_MASK` and
  `MOVEMENT_REDIRECT_MASK`.
- **Impact:** No false claim is made that every movement-redirection cell is
  water. The higher-level semantic identity of `0x100` remains unresolved.
- **Resolution:** INVESTIGATING.
- **Tests affected:**
  `material_identity_is_not_collapsed_into_movement_redirection`,
  `outdoor_redirect_failure_recovers_without_stopping_or_post_accept_clamps`.
- **Review date:** 2026-07-24.

## DISC-005: Unmodeled flight side effects

- **Reference:** The larger flight routine includes a tether interaction,
  palette restoration, audio triggers, rejected recovery, and a speed-spell
  holder side effect.
- **Our implementation:** Tether, recovery, collision-time spell-holder
  mutation, periodic/ambient audio, cave RNG, loop control, and music events
  are explicit and tested. Speed-modifier palette restoration is explicit in
  the effects context. Fire input now produces typed left/right/selected
  activation requests in source order before the flight skip, and executes
  the complete `sub_5F660` activation state machine synchronously against an
  alias-preserving holder registry.
- **Impact:** No known source side-effect family in the bounded flight routine
  remains unmodeled. The slice remains `Partial`, never `Implemented` or
  `Verified`, until active exact-build transitions cover these branches.
- **Resolution:** RESOLVED for source mapping; exact-build evidence remains
  tracked by DISC-006.
- **Tests affected:**
  `spell_fire_requests_preserve_source_order_and_precede_flight_skip`,
  `spell_fire_executes_synchronously_and_preserves_holder_aliasing`, and the
  `spell_activation` branch matrix.
- **Review date:** 2026-07-24.

## DISC-006: Exact-build branch breadth

- **Reference:** The source flight routine contains ordinary movement,
  modifiers, recoil, impulses, material redirection, cave collision/recovery,
  tether, and deterministic side effects.
- **Our implementation:** Every physical branch is source-mapped and has
  focused branch tests. The exact-build corpus proves 475 level-start idle
  transitions across all 25 maps, 19 mid-game idle transitions, and 24
  consecutive recorded outdoor passes ending in five acceleration steps.
  Pinned-native probes additionally cover intermediate scheduler/control
  states and active real-map material, cave, recovery, tether, and left-fire
  activation paths.
- **Impact:** No known divergence or unresolved source ambiguity remains in the
  declared class-3/action-0 projection. This does not extend to the complete
  MC2 engine or presentation state.
- **Resolution:** RESOLVED for the bounded simulation gate by source-led branch
  coverage plus targeted owner-data native checkpoints.
- **Tests affected:**
  `owned_after_load_recording_matches_twenty_four_complete_flight_transitions`,
  `owned_native_level_one_tether_pull_matches_reference`,
  `owned_native_level_one_left_fire_activation_matches_reference`.
- **Review date:** 2026-07-24.

## DISC-007: Source-width and branch-order audit corrections

- **Reference:** `sub_5F380` writes transient `word_0xe_14`; opcode 6 updates
  selected spell before `sub_5F380`; material redirection passes only the low
  16 bits of 3D distance; post-accept ground repair and cave ceiling clamp are
  mutually exclusive.
- **Our implementation:** The target-speed marker is now authoritative state
  decoded from original frames, the pinned 29-entry UI-to-spell map runs
  during command latching, redirect distance is truncated to its source width,
  and vertical repair preserves the exact `if/else if` ordering.
- **Impact:** These were real static-source mismatches or omissions found
  during the serial audit. All are corrected; the existing 518 exact-build
  transitions remain green with the expanded state projection.
- **Resolution:** RESOLVED by source-led implementation and focused ordering
  tests.
- **Tests affected:**
  `target_speed_change_flag_is_cleared_and_set_in_source_order`,
  `movement_opcode_selects_spell_before_selected_fire_dispatch`,
  `redirect_distance_uses_the_source_low_word`,
  `cave_ground_repair_does_not_apply_ceiling_clamp_in_the_same_pass`.
- **Review date:** 2026-07-24.

## DISC-008: Pinned native regression-harness defects

- **Reference:** The pinned remc2 tree includes a 64-bit native regression
  executable, level/after-load snapshots, and
  `afterloadtest2/Levels-1-5-Recording.bin`.
- **Observed behavior:** All 23 enabled level-start regressions pass natively,
  and the after-load idle regression passes when isolated in a fresh process.
  Running all cases in one process makes that idle case fail immediately,
  demonstrating leaked shared state between cases. The recorded action case
  fails independently at captured frame 18 (`pitchDelta` expected `1`, native
  value `0`).
- **Cause:** The action file uses the older bare layout: turn records begin
  immediately after the player header. Current
  `InputRecorder::LoadRecordingFile` unconditionally consumes 208 bytes of
  spell metadata before reading turns. The two layouts retain the same
  16-byte signature, so the native loader does not reject the incompatible
  file and becomes misaligned. At turn 19 the real bare-layout command has
  pitch byte `2`, which statically yields `pitchDelta = 1`; the misparsed
  native playback supplies zero.
- **Impact:** A failure from the current monolithic native regression runner
  is not automatically a simulation divergence. The preserved DOS state
  frames and explicitly parsed bare recording remain valid evidence; the Rust
  transition test reproduces all 24 captured passes exactly.
- **Resolution:** CONTAINED. The Rust parser requires callers to select
  `legacy` or `current`, rejects cross-format parsing, and `sim-cli
  inspect-recording` exposes exact turn bytes. Private GDB recipes run the
  after-load cases independently without editing the pinned upstream
  checkout. Do not weaken the simulator to match the native loader defect.
- **Tests affected:**
  `owned_legacy_regression_recording_has_explicit_bare_layout`,
  `owned_after_load_recording_matches_twenty_four_complete_flight_transitions`.
- **Review date:** 2026-07-24.

## Verification policy

Static source and atlas mappings are the main construction path. Differential
comparison with an owned exact build is optional evidence for a real
ambiguity; it is not required for every normal simulator change. Camera,
rendering, sound, HUD, and asset behavior remain outside authoritative state
and its hash.
