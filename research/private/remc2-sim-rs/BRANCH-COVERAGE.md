# Bounded flight branch coverage

> **Historical focused evidence ledger.** This remains valid proof for the
> original class-3/action-0 slice, but it is not a list of current Level 1
> gaps. Overall Level 1 status is governed by the canonical 182-row matrix and
> PLAN29 Run-A receipt.

This is the executable leaf ledger for the class-3/action-0 flight slice at
upstream pin `6806c7204d17220ecc0430759f784aea8ee2ddb2`. A row marked
`source fixture` means a focused state vector exercises the exact mapped
branch. A row marked `DOS transition` additionally compares projected state
from consecutive preserved DOS regression images. Neither label extends the
claim beyond the bounded state projection.

## Command latch and movement decode

| Source leaf | Proof |
|---|---|
| Ten-byte layout and signed fields | `command_layout_and_profiles_are_exactly_recorded` |
| Positive/negative input delta truncation | `input_delta_truncates_toward_zero` |
| Opcode `0x27`, argument set: clear raw rotation, speed, and holder | `full_stop_opcode_zeroes_rotation_speed_and_the_exact_spell_holder` |
| Opcode `0x27`, argument clear: preserve speed and holder | `full_stop_without_speed_argument_preserves_speed_and_holder` |
| Opcode `6` selection occurs before selected fire | `movement_opcode_selects_spell_before_selected_fire_dispatch` |
| Forward/reverse limits, reverse priority, change marker | `target_and_strafe_controls_obey_steps_limits_and_decay`; `opposing_movement_flags_preserve_original_priority`; `target_speed_change_flag_is_cleared_and_set_in_source_order` |
| Positive/negative strafe, both-key priority, clamp, and zero crossing | `opposing_movement_flags_preserve_original_priority`; `negative_strafe_reaches_its_limit_and_decays_through_zero` |
| One latch reused for many entity passes | `input_is_latched_once_for_multiple_entity_passes` |
| Four/eight-pass intermediate speed, marker, phase, position, and vertical state | `four_and_eight_pass_modes_match_every_intermediate_source_state`; `owned_native_four_and_eight_pass_forward_traces_match_every_pass` (pinned native trace) |
| Active forward steering, reverse, and both strafe directions after every pass | `owned_native_active_control_traces_match_every_pass` (pinned native trace) |
| Active pitch shaping and positive-capped/negative recoil after every pass | `owned_native_pitch_and_recoil_traces_match_every_pass` (pinned native trace) |
| Active staged speed modifier and immobilization after every pass | `owned_native_status_modifier_traces_match_every_pass` (pinned native trace) |

## Physical integration

| Source leaf | Proof |
|---|---|
| One-pass skip clears itself after input/fire and bypasses the body | `spell_fire_requests_preserve_source_order_and_precede_flight_skip`; `cave_ambience_advances_the_exact_rng_and_skip_bypasses_all_effects` |
| Modified and unmodified control deltas | `movement_statuses_modify_controls_translation_and_counters` |
| Signed yaw division around zero | `yaw_uses_signed_division_with_truncation_toward_zero` |
| Actual-speed approach, equality, and overshoot | `actual_speed_moves_by_exact_steps_and_can_overshoot` |
| Terrain-relative pitch sign matrix, clamp, boundary, and retention | `terrain_relative_pitch_preserves_direction_and_surface_shaping`; `terrain_relative_pitch_keeps_boundary_and_zero_input_state`; `speed_steps_before_pitch_shaping_on_the_first_acceleration_tick` |
| Forward, pre-decayed strafe, capped recoil, and source order | `forward_strafe_and_capped_recoil_displace_in_source_order` |
| Positive/negative recoil decay and sub-step zeroing | `recoil_decay_zeroes_the_sub_step_remainder` |
| Cartesian impulse is one pass only | `impulses_are_one_tick_and_ground_and_ceiling_clamp` |
| Material counter decrements before contact refresh | `material_contact_counter_decays_before_contact_refresh` |
| Speed modifier arithmetic, stages, global flag, local palette rule | `recovered_speed_effect_arithmetic_truncates_toward_zero`; `movement_statuses_modify_controls_translation_and_counters`; `speed_modifier_stages_emit_exact_global_palette_flags` |
| Modifier takes translation priority over immobilization | `movement_statuses_modify_controls_translation_and_counters` |
| Immobilization translation, counter, and vertical-expiry order | `movement_statuses_modify_controls_translation_and_counters`; `immobilization_uses_its_vertical_step_until_the_expiry_tick` |
| Ground repair and cave ceiling clamp are mutually exclusive | `impulses_are_one_tick_and_ground_and_ceiling_clamp`; `cave_ground_repair_does_not_apply_ceiling_clamp_in_the_same_pass` |

## Material, cave, and recovery resolver

| Source leaf | Proof |
|---|---|
| Non-`0x100` outdoor accept and material `1` stays distinct | `material_identity_is_not_collapsed_into_movement_redirection` |
| First quantized material slide succeeds | `material_redirection_accepts_the_first_quantized_slide` |
| First slide fails and second succeeds | `water_boundary_slides_along_a_dry_quantized_direction` |
| Both slides fail outdoors: reject without target stop | `outdoor_redirect_failure_recovers_without_stopping_or_post_accept_clamps` |
| Both slides fail in cave: reject and stop even with open clearance | `cave_material_failure_rejects_even_when_clearance_is_open` |
| Cave candidate directly clear | `open_cave_accepts_and_side_probe_steering_has_exact_yaw_sign` |
| Left and right side steering and yaw sign | `open_cave_accepts_and_side_probe_steering_has_exact_yaw_sign` |
| Equal side gaps, no overlap: accept unchanged | `cave_equal_side_gaps_accept_when_the_candidate_does_not_overlap` |
| Strict overlap boundary and forbidden-tile rejection | `cave_overlap_boundary_is_strict`; `blocked_cave_candidate_rejects_and_slope_clamps` |
| Cave rejection stops target and active speed holder | `rejected_movement_on_a_forbidden_cave_tile_recovers_and_stops_target`; `cave_rejection_disables_the_projected_speed_spell_holder` |
| Recovery from material, forbidden tile, and prior-latch overlap | `recovery_branch_uses_current_yaw_previous_latch_and_exact_clear_rules`; `rejected_movement_on_a_forbidden_cave_tile_recovers_and_stops_target` |
| Recovery failure clears the latch and retains old position | `recovery_branch_uses_current_yaw_previous_latch_and_exact_clear_rules` |
| Low-word 3D distance and recovered angle/root helpers | `redirect_distance_uses_the_source_low_word`; `movement_redirection_angles_use_the_recovered_helper`; `recovered_seeded_square_root_is_floor_exact_across_the_u32_domain` |
| Real level-3 `0x100` boundary redirect and real level-4 cave reject/recovery-push paths | `owned_native_level_three_material_boundary_matches_reference`; `owned_native_level_four_cave_rejection_matches_reference`; `owned_native_level_four_cave_recovery_push_matches_reference` (pinned native traces) |

## Cross-entity and deterministic effects

| Source leaf | Proof |
|---|---|
| Tether sentinel lookup, inactive/range/life invalidation | `tether_clears_only_after_a_valid_target_fails_active_conditions`; `tether_range_boundary_and_dead_target_clear_the_target` |
| Tether turn cap, positive/negative pull-speed caps, and angle wrapping | `active_tether_turns_pulls_and_uses_the_source_speed_limit`; `tether_turn_direction_preserves_wrap_and_half_turn_ties` |
| Tether tiers 0/1/2 and mana/life clamp | `active_tether_turns_pulls_and_uses_the_source_speed_limit`; `tether_tiers_apply_exact_mana_and_life_mutations` |
| Real level-1 active target/holder pull | `owned_native_level_one_tether_pull_matches_reference` (pinned native checkpoint) |
| Periodic spell-sound switch, including default | `periodic_spell_sound_table_covers_every_source_case` |
| Outdoor water/wind loop pair | `deterministic_outdoor_sound_and_music_events_preserve_source_order`; `quiet_effect_branches_preserve_loops_music_and_locality` |
| Cave RNG sound and no-sound leaves | `cave_ambience_advances_the_exact_rng_and_skip_bypasses_all_effects`; `quiet_effect_branches_preserve_loops_music_and_locality` |
| Near/far distance loops | `deterministic_outdoor_sound_and_music_events_preserve_source_order`; `quiet_effect_branches_preserve_loops_music_and_locality` |
| Music start, countdown, and non-local suppression | `deterministic_outdoor_sound_and_music_events_preserve_source_order`; `quiet_effect_branches_preserve_loops_music_and_locality` |
| Left/right/selected fire order, masks, aliasing, and synchronous activation | `spell_fire_requests_preserve_source_order_and_precede_flight_skip`; `spell_fire_executes_synchronously_and_preserves_holder_aliasing`; `spell_activation` test matrix |
| Real level-1 left-fire holder selection and cold activation | `owned_native_level_one_left_fire_activation_matches_reference` (pinned native checkpoint) |

## Preserved transition evidence

- 475 level-start transitions cover all 25 maps: eight day, nine night, eight
  cave.
- 19 mid-game after-load transitions cover nonzero orientation accumulators.
- 24 consecutive action transitions cover the bare recording, exact input
  latch, and five forward-acceleration passes.
- All 518 compare the complete declared `FlightState`.
- Two additional pinned-native active checkpoints cover the real level-1
  tether target/holder mutation and left-fire activation state.

## Gate disposition

The declared class-3/action-0 bounded simulation gate is closed. Every mapped
leaf has a source-derived executable test; the scheduler and major active
movement families have intermediate native checkpoints; real-map material,
cave, recovery, tether, and fire activation paths have owner-data native
checkpoints. No known ambiguity or divergence remains inside this projection.

This is not a claim that the complete MC2 engine or a rendered client is frame
perfect. Renderer, camera, input capture, and audiovisual behavior are the
next separate presentation gate.
