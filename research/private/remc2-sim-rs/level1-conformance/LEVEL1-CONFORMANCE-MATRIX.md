# Level 1 conformance matrix (generated view)

Generated: 2026-07-28
Schema: `mc2-level1-conformance-matrix` v2
Specification: remc2 at `6806c7204d17220ecc0430759f784aea8ee2ddb2`
Canonical machine-readable matrix: [`level1-conformance-matrix.json`](level1-conformance-matrix.json) — every claim below links to a JSON row ID.
Implementation plan: [`IMPLEMENTATION-PLAN.md`](IMPLEMENTATION-PLAN.md)

This file is a generated readable view of the canonical JSON rows. Do not edit by hand.

## Method

independent traversal from Level 1 load/reset state: typed owner authored-table census (authored_census: 308 non-zero rows, partition-proved), reset arena census (177 actives, 14 class/model/action families), activation-graph traversal (load-time G-1, G0 at start, switch-driven groups incl. proximity ids 12-33), and typed dispatch census (dispatch_census: all 845 mechanical slots partitioned between rows and exclusions), with per-handler source reads at the pinned commit.

Line spans cite the committed revision. upstream/magic-carpet-2-hd (local quit-only patches; all line spans cite the committed revision; working tree adds +3/+5/+8 lines in EventsFunctions.cpp after pinned lines 31305/31482/31625).

Owner data: research/private/generated/level1-export-m (mc2-owner-level-export/v2, Night map, 1,200 authored records, 177 active reset entities).

Evidence bindings resolve against the 429-test inventory from `cd research/private/remc2-sim-rs && cargo test --workspace --locked -- --list`. Every `Exact` row carries at least one resolving `rust-test` binding and no non-executable binding; `qualification-record` entries are explicitly non-executable and appear only on non-Exact rows.

## Totals

| Classification | Rows |
|---|---:|
| Exact | 98 |
| Partial | 50 |
| Missing | 31 |
| Wrong | 1 |
| Unproved | 2 |
| **Total** | **182** |

Exclusions with mechanical proof: 18. Plan items: 22. Authored census: 308/308 indices partitioned. Dispatch census: 845/845 mechanical slots partitioned (168 to rows, 677 to exclusions).

| Subsystem | Exact | Partial | Missing | Wrong | Unproved | Total |
|---|---:|---:|---:|---:|---:|---:|
| activation | 0 | 3 | 0 | 0 | 0 | 3 |
| audio | 1 | 1 | 0 | 0 | 0 | 2 |
| authored | 12 | 6 | 5 | 0 | 0 | 23 |
| combat | 4 | 0 | 0 | 0 | 0 | 4 |
| dispatch | 51 | 34 | 26 | 1 | 2 | 114 |
| economy | 1 | 0 | 0 | 0 | 0 | 1 |
| flight | 13 | 0 | 0 | 0 | 0 | 13 |
| input | 1 | 1 | 0 | 0 | 0 | 2 |
| objective | 2 | 0 | 0 | 0 | 0 | 2 |
| presentation-abi | 2 | 1 | 0 | 0 | 0 | 3 |
| reset | 8 | 0 | 0 | 0 | 0 | 8 |
| rng | 0 | 1 | 0 | 0 | 0 | 1 |
| scheduler | 3 | 1 | 0 | 0 | 0 | 4 |
| targeting | 0 | 1 | 0 | 0 | 0 | 1 |
| terrain | 0 | 1 | 0 | 0 | 0 | 1 |

## Headline findings

- **[DSP-11-17] Wrong:** the Speed-jar release switch (class 11 model 17, authored row 109)
  gates on Archer extinction in source (`AddSwitch0B_11_6F4A0` passes `4` to `sub_6F300`,
  indexing the class-5 model-4 live list). The Rust implementation releases on a plain
  16-tick timer and its test asserts the wrong behavior.
- **Groups 13–32 are reachable and missing:** 21 class-11 model-0 proximity switches with
  ids 12–32 are active at reset; overlap activates village-expansion groups containing 99
  authored rows (34 houses, 34 fire effects, 31 splash/ash effects). None can activate in Rust.
- **Village ambient population missing:** nine chimney emitters (10/60→action 65), model-14
  smoke, eight 10/1 fire objects and their model-0 children are active at reset and unimplemented.
- **Resident action bodies frozen (gap mask 0x0f):** Builder 96–103, Villager 104–111,
  Trader 112–119 missing; house-spawned Archers partial.
- **Load-time behavior imported, not executed:** G-1 house generation and the two 10/29
  terrain-raise commands exist only via the frame-0 package import ([ACT-003], [DSP-10-29]).

## Rows

### reset

| Row | Title | Class/Model/Slot | Classification | Plan | Key source anchor | Key evidence |
|---|---|---|---|---|---|---|
| `RST-001` | Owner package load, member validation and fail-closed rejection | — | **Exact** | — | EventsFunctions.cpp:39441-39480 (sub_56D60 pinned) | level1_mission::tests::complete_reset_projection_preserves_fi... |
| `RST-002` | Authored feature table decode: 1,200 x 20-byte records at offset 1,091 | — | **Exact** | — | EventsFunctions.cpp:32966-32990 (sub_4A1E0 loop bounds 1.. | level1_mission::tests::exact_level1_actor_manifest_is_decoded |
| `RST-003` | Reset source arena: 1,000 slots, 177 active entities, free/replaceable stacks | — | **Exact** | — | Level.cpp:1271-1290 (sub_49F90) | level1_mission::tests::source_pool_preserves_reclaim_stack_an... |
| `RST-004` | Player reset: flight profile row 66 (outdoor), spells, mana, life, position | — | **Exact** | — | EventsFunctions.cpp:33199-33320 approx (AddPlayer_4A920) | tests::command_layout_and_profiles_are_exactly_recorded |
| `RST-005` | Global RNG seed and per-entity RNG seeds at reset | — | **Exact** | — | EventsFunctions.cpp:28154-28162 (9377*x+9439 idiom) | level1_mission::tests::mana_sphere_constructor_uses_source_id... |
| `RST-006` | Terrain reset: seven 65,536-byte planes and canonical hashes | — | **Exact** | — | Terrain.cpp:2058-2142 (material identity) | legacy_terrain::tests::owned_level_one_regression_frame_loads... |
| `RST-007` | Objective/stage initialization: InitStages_58940, InitStageVars_11EE0 | — | **Exact** | — | EventsFunctions.cpp:40567-40649 (InitStages pinned) | level1_mission::tests::complete_level1_route_reaches_every_ob... |
| `RST-008` | Reset presentation state: palette, camera, minimap, HUD, audio pool, music | — | **Exact** | — | EventsFunctions.cpp:39402, 39435-39439 (pinned) | native_v2::tests::native_level_one_palette_is_atomic_hashed_a... |

### scheduler

| Row | Title | Class/Model/Slot | Classification | Plan | Key source anchor | Key evidence |
|---|---|---|---|---|---|---|
| `SCH-001` | Outer turn order: TMAP animation, input, player events, lighting rollback, pass  | — | **Exact** | — | EventsFunctions.cpp:31800-31815 pinned (pass select) | tests::four_and_eight_pass_modes_match_every_intermediate_sou... |
| `SCH-002` | UpdateEntities_57730: RNG advance, reclaim, list rebuild, stage maintenance, asc | — | **Exact** | — | EventsFunctions.cpp:39928-40179 pinned | level1_mission::tests::source_pool_preserves_reclaim_stack_an... |
| `SCH-003` | Pre-dispatch proximity countdown pass sub_68BF0/sub_68C70 | — | **Exact** | — | EventsFunctions.cpp:55508-55582 pinned | goat::tests::zero_proximity_countdown_rearms_from_local_playe... |
| `SCH-004` | Per-turn global RNG draw census (ten advances per outer turn on preserved Level  | — | **Partial** | PLAN-26 | EventsFunctions.cpp:39928-40179 pinned + per-consumer site | sequence-002285FF-00356038.bin ten-advance observation |

### activation

| Row | Title | Class/Model/Slot | Classification | Plan | Key source anchor | Key evidence |
|---|---|---|---|---|---|---|
| `ACT-001` | sub_4A1E0 group activation: reset branch, stage broadcast, row scan, consume fla | — | **Partial** | PLAN-11 | EventsFunctions.cpp:32950-32997 pinned | level1_mission::tests::exhausted_authored_group_activation_ke... |
| `ACT-002` | sub_4A310 authored-record activation: terrain-centered position, constructor cal | — | **Partial** | PLAN-11 | EventsFunctions.cpp:32999-33195 pinned | level1_mission::tests::authored_allocation_only_records_prese... |
| `ACT-003` | Load-time feature generation (group -1 houses and terrain commands) via sub_4927 | — | **Partial** | PLAN-24 | EventsFunctions.cpp:39393/39464 call sites pinned | level1_mission::tests::complete_reset_projection_preserves_fi... |

### dispatch

| Row | Title | Class/Model/Slot | Classification | Plan | Key source anchor | Key evidence |
|---|---|---|---|---|---|---|
| `DSP-11-00` | Class 11 model 0 action 0: proximity switch (consume-activate) | 11/0/0 | **Partial** | PLAN-11 | EventsFunctions.cpp:44545-44554 pinned (handler) | level1_mission::tests::switch_305_uses_strict_wrapped_xy_cade... |
| `DSP-11-01` | Class 11 model 1 action 1: checkpoint-arrival switch (non-overlap comparator) | 11/1/1 | **Partial** | PLAN-11 | EventsFunctions.cpp:44557-44567 pinned | level1_mission::tests::spatial_and_objective_switch_chain_act... |
| `DSP-11-04` | Class 11 model 4 action 4: level-end gated activation (post-completion G4) | 11/4/4 | **Exact** | — | EventsFunctions.cpp:44569 area pinned 54369-54390 | level1_mission::tests::group_four_constructs_scroll_switches_... |
| `DSP-11-12` | Class 11 model 12 action 12: exit switch (player to end-game action 12) | 11/12/12 | **Exact** | — | EventsFunctions.cpp:54471-54495 pinned | level1_mission::tests::group4_exit_switch_runs_source_timers_... |
| `DSP-11-16` | Class 11 model 16 action 16: creature-list gate on class-5 model 3 (sub_6F300 a2 | 11/16/16 | **Exact** | — | EventsFunctions.cpp:54568-54572 (wrapper) | level1_mission::tests::switch_308_waits_for_dying_body_then_c... |
| `DSP-11-17` | Class 11 model 17 action 17: creature-list gate on class-5 model 4 = Archer (sub | 11/17/17 | **Wrong** | PLAN-10 | EventsFunctions.cpp:54574-54578 (wrapper passes 4u) | level1_mission::tests::speed_release_is_model17_timed_and_ind... |
| `DSP-11-32` | Class 11 model 32 action 32: objective-completion switch | 11/32/32 | **Exact** | — | EventsFunctions.cpp:54393-54420 pinned | level1_mission::tests::spatial_and_objective_switch_chain_act... |

### authored

| Row | Title | Class/Model/Slot | Classification | Plan | Key source anchor | Key evidence |
|---|---|---|---|---|---|---|
| `AUT-G-1-HOUSES` | Group -1: 47 load-time houses (kind 10/45, building parameter par1, xtype par2=0 | — | **Partial** | PLAN-13 | EventsFunctions.cpp:owner data | level1_mission::tests::complete_reset_projection_preserves_fi... |
| `AUT-G-1-TERRAIN29` | Group -1: two load-time rectangular terrain-raise commands (kind 10/29, rows 62- | — | **Partial** | PLAN-24 | EventsFunctions.cpp:owner data | frame-0 terrain planes embed the raise results |
| `AUT-G0-MARKER` | Group 0: player-start position marker (kind 3/4, row 1) | — | **Exact** | — | EventsFunctions.cpp:constructor table 1201-1231 pinned | level1_mission::tests::complete_reset_projection_preserves_fi... |
| `AUT-G0-GOATS` | Group 0: 33 Goats (kind 5/1, StageVar rows force action 15) | — | **Exact** | — | EventsFunctions.cpp:owner data | goat::tests::owner_package_reset_matches_preserved_frame_zero... |
| `AUT-G0-VILLAGERS` | Group 0: 28 Villagers (kind 5/13, action 111 at reset) | — | **Partial** | PLAN-16 | EventsFunctions.cpp:owner data | villager::tests::exact_level_one_reset_decodes_all_twenty_eig... |
| `AUT-G0-DOLMENS` | Group 0: 3 dolmens (kind 2/2, regeneration action 6) | — | **Exact** | — | EventsFunctions.cpp:owner data | native_world::tests::dolmen_contact_selects_high_recharge_on_... |
| `AUT-G0-SWITCHES` | Group 0: 23 switches (21x model 0 ids 12-32; model 1 id 1; model 4 id 4) | — | **Partial** | PLAN-11 | EventsFunctions.cpp:owner data | level1_mission::tests::spatial_and_objective_switch_chain_act... |
| `AUT-G0-EMITTERS` | Group 0: 9 village smoke emitters (kind 10/60 -> action 65, spawn model-14 smoke | — | **Missing** | PLAN-18 | EventsFunctions.cpp:owner data | — |
| `AUT-G0-FIRES` | Group 0: 8 village fire/effect objects (kind 10/1 -> action 1, spawn model-0 chi | — | **Missing** | PLAN-18 | EventsFunctions.cpp:owner data | — |
| `AUT-G1` | Group 1: 18 trees (2/0), Castle spell jar (15/2), objective switch 99, proximity | — | **Exact** | — | EventsFunctions.cpp:owner data | level1_mission::tests::group_one_trees_preserve_constructor_r... |
| `AUT-G2` | Group 2: objective switch 105, checkpoint switch 231 (id 8) | — | **Exact** | — | EventsFunctions.cpp:owner data | level1_mission::tests::spatial_and_objective_switch_chain_act... |
| `AUT-G3` | Group 3: 4 Archers (5/4), Archer-gate switch 109 (11/17), objective switch 153 | — | **Partial** | PLAN-10 | EventsFunctions.cpp:owner data | level1_mission::tests::archer_constructor_cadence_acquires_an... |
| `AUT-G4` | Group 4 (post-completion): 5 scrolls (14/5), exit target 132 (14/3), exit switch | — | **Exact** | — | EventsFunctions.cpp:owner data | level1_mission::tests::group_four_constructs_scroll_switches_... |
| `AUT-G5` | Group 5: objective switch 190 | — | **Exact** | — | EventsFunctions.cpp:owner data | level1_mission::tests::archer_absence_drives_group5_immediate... |
| `AUT-G6` | Group 6: 5 Fireflies (5/19) and stone 154 (2/1) | — | **Exact** | — | EventsFunctions.cpp:owner data | level1_mission::tests::fireflies_bind_to_level1_stage_row_and... |
| `AUT-G8` | Group 8: objective switch 232 | — | **Exact** | — | EventsFunctions.cpp:owner data | level1_mission::tests::complete_level1_route_reaches_every_ob... |
| `AUT-G11` | Group 11: Speed spell jar (15/3, row 100) | — | **Partial** | PLAN-10 | EventsFunctions.cpp:owner data | level1_mission::tests::speed_spell_uses_exact_countdown_speed... |
| `AUT-G12` | Group 12: checkpoint smoke emitter 253 (10/59->64) | — | **Exact** | — | EventsFunctions.cpp:owner data | level1_mission::tests::checkpoint_emitters_and_model_thirteen... |
| `AUT-G33` | Group 33: post-completion Firefly 306 (5/19) | — | **Exact** | — | EventsFunctions.cpp:owner data | level1_mission::tests::firefly_stage_arrival_hands_off_to_roa... |
| `AUT-G34` | Group 34: 3 multipart creatures 309-311 (5/3) | — | **Exact** | — | EventsFunctions.cpp:owner data | level1_mission::tests::group_four_constructs_scroll_switches_... |
| `AUT-G13-32-HOUSES` | Groups 13-32: 34 village-expansion houses (kind 10/45, building parameter 37) | — | **Missing** | PLAN-12 | EventsFunctions.cpp:owner data | — |
| `AUT-G13-32-FIRES` | Groups 13-32: 34 village fire/effect objects (kind 10/1) | — | **Missing** | PLAN-12 | EventsFunctions.cpp:owner data | — |
| `AUT-G13-32-SPLASH` | Groups 13-32: 31 kind 10/5 effect objects (water splash/ash family, par1=1) | — | **Missing** | PLAN-12 | EventsFunctions.cpp:owner data | — |

### flight

| Row | Title | Class/Model/Slot | Classification | Plan | Key source anchor | Key evidence |
|---|---|---|---|---|---|---|
| `FLT-001` | Command layout, latching and delta truncation (ten-byte Type_PlayerInput_0x6E3E) | 3/0/0 | **Exact** | — | EventsFunctions.cpp:37528-38084 pinned | tests::command_layout_and_profiles_are_exactly_recorded |
| `FLT-002` | Opcode 0x27 full stop and opcode 6 selected-spell resolution before fire | 3/0/0 | **Exact** | — | EventsFunctions.cpp:opcode cases in 37528-38084 pinned | tests::full_stop_opcode_zeroes_rotation_speed_and_the_exact_s... |
| `FLT-003` | Pass scheduler: speed index 0/1/2 -> 1/4/8 passes, other -> 0 | 3/0/0 | **Exact** | — | EventsFunctions.cpp:31800-31815 pinned | tests::four_and_eight_pass_modes_match_every_intermediate_sou... |
| `FLT-004` | Input decode sub_5F380: target/strafe steps, limits, decay, change marker | 3/0/0 | **Exact** | — | EventsFunctions.cpp:60788-60904 pinned | tests::target_and_strafe_controls_obey_steps_limits_and_decay |
| `FLT-005` | Spell-fire dispatch: left/right/selected order, masks 256/512/256, pre-skip, syn | 3/0/0 | **Exact** | — | EventsFunctions.cpp:60915-61010 pinned | tests::spell_fire_requests_preserve_source_order_and_precede_... |
| `FLT-006` | Spell activation state machine sub_5F660 complete branch families | 3/0/0 | **Exact** | — | EventsFunctions.cpp:60915-61010 pinned | spell_activation::tests::invalid_holder_is_the_source_sentine... |
| `FLT-007` | Physical integration steps 4-16 (skip flag, deltas, yaw, speed approach, terrain | 3/0/0 | **Exact** | — | EventsFunctions.cpp:59650-59790 pinned | tests::actual_speed_moves_by_exact_steps_and_can_overshoot |
| `FLT-008` | Tether sub_5DE30: target selection, caps, tiers, invalidation | 3/0/0 | **Exact** | — | EventsFunctions.cpp:59929-59989 pinned | tests::active_tether_turns_pulls_and_uses_the_source_speed_limit |
| `FLT-009` | Material redirection and cave collision moveTest_5D0A0 (all ledger branches) | 3/0/0 | **Exact** | — | EventsFunctions.cpp:59469-59647 pinned | tests::material_redirection_accepts_the_first_quantized_slide |
| `FLT-010` | Rejected-movement recovery sub_5DD50 and vertical repair ordering | 3/0/0 | **Exact** | — | EventsFunctions.cpp:59894-59922 | tests::recovery_branch_uses_current_yaw_previous_latch_and_ex... |
| `FLT-011` | Deterministic flight effects: periodic spell sounds, cave RNG ambience, loops, m | 3/0/0 | **Exact** | — | EventsFunctions.cpp:59816-59890 pinned | tests::periodic_spell_sound_table_covers_every_source_case |
| `FLT-012` | Exact numeric helpers: sin table, angle lookup, 3D distance, angular distance | 3/0/0 | **Exact** | — | EventsFunctions.cpp:Maths.cpp:757-833 | tests::recovered_seeded_square_root_is_floor_exact_across_the... |
| `FLT-013` | Terrain provider: exact array sampling, interpolation, material and angle flags | 3/0/0 | **Exact** | — | EventsFunctions.cpp:Terrain.cpp:2058-2142 and samplers | legacy_terrain::tests::owned_level_one_regression_frame_loads... |

### dispatch

| Row | Title | Class/Model/Slot | Classification | Plan | Key source anchor | Key evidence |
|---|---|---|---|---|---|---|
| `DSP-02-00` | Tree live update (class 2 slot 0) | 2/0/0 | **Exact** | — | EventsFunctions.cpp:62439-62497 pinned | level1_mission::tests::tree_damage_dying_and_stump_threshold_... |
| `DSP-02-01` | Tree dying phase (slot 1) | 2/0/1 | **Exact** | — | EventsFunctions.cpp:62502-62536 pinned | level1_mission::tests::tree_damage_dying_and_stump_threshold_... |
| `DSP-02-02` | Tree terminal/stump phase (slot 2) | 2/0/2 | **Exact** | — | EventsFunctions.cpp:62540-62555 pinned | level1_mission::tests::tree_damage_dying_and_stump_threshold_... |
| `DSP-02-03` | Standing-stone static update (slot 3) | 2/1/3 | **Exact** | — | EventsFunctions.cpp:62558-62562 pinned | level1_mission::tests::group_six_stone_uses_the_source_constr... |
| `DSP-02-06` | Dolmen regeneration-field update (slot 6) | 2/2/6 | **Exact** | — | EventsFunctions.cpp:62565-62574 pinned | native_world::tests::dolmen_contact_selects_high_recharge_on_... |
| `DSP-03-02` | Player death phase (class 3 slot 2) | 3/0/2 | **Partial** | PLAN-27 | EventsFunctions.cpp:60085-60222 pinned | level1_mission::tests::lethal_player_hit_retains_source_mailb... |
| `DSP-03-03` | Player banished/respawn phase (slot 3) | 3/0/3 | **Partial** | PLAN-27 | EventsFunctions.cpp:60294-60348 pinned | native_v2::tests::package_m_world_reset_and_copy_surfaces_are... |
| `DSP-03-04` | Castle settling and mana absorption (slot 4) | 3/2/4 | **Exact** | — | EventsFunctions.cpp:61095-61160 pinned | level1_mission::tests::castle_even_phase_absorbs_only_the_fir... |
| `DSP-03-05` | Castle creation state machine (slot 5) | 3/2/5 | **Exact** | — | EventsFunctions.cpp:61163-61220 pinned | level1_mission::tests::valid_castle_projectile_builds_exact_s... |
| `DSP-03-06` | Castle destruction state (slot 6) | 3/2/6 | **Exact** | — | EventsFunctions.cpp:61262-61278 pinned | level1_mission::tests::lethal_castle_retries_without_a_free_s... |
| `DSP-03-09` | Active balloon/mana-carrier update (slot 9) | 3/3/9 | **Exact** | — | EventsFunctions.cpp:61803-61911 pinned | level1_mission::tests::carrier_scheduler_uses_wrapped_3d_dist... |
| `DSP-03-12` | End-game sequence variant 12 (exit sequence) | 3/0/12 | **Exact** | — | EventsFunctions.cpp:60353-60636 pinned | level1_mission::tests::exit_action_phase_one_preserves_reject... |
| `DSP-05-008` | Goat idle (class 5 model 1 slot 8) | 5/1/8 | **Exact** | — | EventsFunctions.cpp:11378-11453 pinned (behavior) | goat::tests::tail_rng_and_sound_cadence_are_wrapping_and_dete... |
| `DSP-05-009` | Goat roam (class 5 model 1 slot 9) | 5/1/9 | **Exact** | — | EventsFunctions.cpp:11378-11453 pinned (behavior) | goat::tests::action_9_reacquires_on_cadence_and_damage_interr... |
| `DSP-05-010` | Goat redirect to hit (class 5 model 1 slot 10) | 5/1/10 | **Exact** | — | EventsFunctions.cpp:11378-11453 pinned (behavior) | goat::tests::action_15_hit_enters_action_14_and_retaliation_c... |
| `DSP-05-011` | Goat linked/reaction (class 5 model 1 slot 11) | 5/1/11 | **Exact** | — | EventsFunctions.cpp:11378-11453 pinned (behavior) | goat::tests::exact_level_one_family_matches_all_twenty_preser... |
| `DSP-05-012` | Goat pre-kill (class 5 model 1 slot 12) | 5/1/12 | **Exact** | — | EventsFunctions.cpp:11378-11453 pinned (behavior) | goat::tests::damage_prekill_phase_gated_removal_and_reclamati... |
| `DSP-05-013` | Goat kill/removal (class 5 model 1 slot 13) | 5/1/13 | **Exact** | — | EventsFunctions.cpp:11378-11453 pinned (behavior) | goat::tests::damage_prekill_phase_gated_removal_and_reclamati... |
| `DSP-05-014` | Goat hit/retaliation pursuit (class 5 model 1 slot 14) | 5/1/14 | **Exact** | — | EventsFunctions.cpp:11378-11453 pinned (behavior) | goat::tests::action_15_hit_enters_action_14_and_retaliation_c... |
| `DSP-05-015` | Goat scripted StageVar update (class 5 model 1 slot 15) | 5/1/15 | **Exact** | — | EventsFunctions.cpp:11378-11453 pinned (behavior) | goat::tests::acquisition_boundary_uses_countdown_phase_range_... |
| `DSP-05-024` | Multipart idle/wander (class 5 model 3 slot 24) | 5/3/24 | **Exact** | — | EventsFunctions.cpp:11573-11660 area pinned (model-3 band) | level1_mission::tests::group_four_constructs_scroll_switches_... |
| `DSP-05-025` | Multipart active roam (class 5 model 3 slot 25) | 5/3/25 | **Exact** | — | EventsFunctions.cpp:11573-11660 area pinned (model-3 band) | level1_mission::tests::group_four_constructs_scroll_switches_... |
| `DSP-05-026` | Multipart target attack (class 5 model 3 slot 26) | 5/3/26 | **Exact** | — | EventsFunctions.cpp:11573-11660 area pinned (model-3 band) | level1_mission::tests::group_four_constructs_scroll_switches_... |
| `DSP-05-027` | Multipart linked/hit reaction (class 5 model 3 slot 27) | 5/3/27 | **Exact** | — | EventsFunctions.cpp:11573-11660 area pinned (model-3 band) | level1_mission::tests::group_four_constructs_scroll_switches_... |
| `DSP-05-028` | Multipart pre-kill (class 5 model 3 slot 28) | 5/3/28 | **Exact** | — | EventsFunctions.cpp:11573-11660 area pinned (model-3 band) | level1_mission::tests::group_four_constructs_scroll_switches_... |
| `DSP-05-029` | Multipart kill/removal (class 5 model 3 slot 29) | 5/3/29 | **Exact** | — | EventsFunctions.cpp:11573-11660 area pinned (model-3 band) | level1_mission::tests::group_four_constructs_scroll_switches_... |
| `DSP-05-031` | Multipart scripted StageVar (class 5 model 3 slot 31) | 5/3/31 | **Exact** | — | EventsFunctions.cpp:11573-11660 area pinned (model-3 band) | level1_mission::tests::group_four_constructs_scroll_switches_... |
| `DSP-05-232` | Shared multipart-segment follow state (class 5 model 3 slot 232) | 5/3/232 | **Exact** | — | EventsFunctions.cpp:11573-11660 area pinned (model-3 band) | level1_mission::tests::group_four_constructs_scroll_switches_... |
| `DSP-05-032` | Archer idle (class 5 model 4 slot 32) | 5/4/32 | **Partial** | PLAN-14 | EventsFunctions.cpp:11617-11932 pinned (model-4 band) | level1_mission::tests::archer_constructor_cadence_acquires_an... |
| `DSP-05-033` | Archer roam/damage AI (class 5 model 4 slot 33) | 5/4/33 | **Partial** | PLAN-14 | EventsFunctions.cpp:11617-11932 pinned (model-4 band) | level1_mission::tests::archer_constructor_cadence_acquires_an... |
| `DSP-05-034` | Archer target attack (class 5 model 4 slot 34) | 5/4/34 | **Partial** | PLAN-14 | EventsFunctions.cpp:11617-11932 pinned (model-4 band) | level1_mission::tests::archer_constructor_cadence_acquires_an... |
| `DSP-05-035` | Archer linked/reaction (class 5 model 4 slot 35) | 5/4/35 | **Partial** | PLAN-14 | EventsFunctions.cpp:11617-11932 pinned (model-4 band) | level1_mission::tests::archer_constructor_cadence_acquires_an... |
| `DSP-05-036` | Archer death staging (class 5 model 4 slot 36) | 5/4/36 | **Partial** | PLAN-14 | EventsFunctions.cpp:11617-11932 pinned (model-4 band) | level1_mission::tests::archer_constructor_cadence_acquires_an... |
| `DSP-05-037` | Archer kill/removal (class 5 model 4 slot 37) | 5/4/37 | **Partial** | PLAN-14 | EventsFunctions.cpp:11617-11932 pinned (model-4 band) | level1_mission::tests::archer_constructor_cadence_acquires_an... |
| `DSP-05-039` | Archer scripted update (class 5 model 4 slot 39) | 5/4/39 | **Partial** | PLAN-14 | EventsFunctions.cpp:11617-11932 pinned (model-4 band) | level1_mission::tests::archer_constructor_cadence_acquires_an... |
| `DSP-05-096` | Builder building placement (class 5 model 12 slot 96) | 5/12/96 | **Missing** | PLAN-15 | EventsFunctions.cpp:13934-14436 pinned (model-12 band) | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-05-097` | Builder wander/damage (class 5 model 12 slot 97) | 5/12/97 | **Missing** | PLAN-15 | EventsFunctions.cpp:13934-14436 pinned (model-12 band) | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-05-098` | Builder target pursuit (class 5 model 12 slot 98) | 5/12/98 | **Missing** | PLAN-15 | EventsFunctions.cpp:13934-14436 pinned (model-12 band) | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-05-099` | Builder building search (class 5 model 12 slot 99) | 5/12/99 | **Missing** | PLAN-15 | EventsFunctions.cpp:13934-14436 pinned (model-12 band) | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-05-100` | Builder pre-kill (class 5 model 12 slot 100) | 5/12/100 | **Missing** | PLAN-15 | EventsFunctions.cpp:13934-14436 pinned (model-12 band) | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-05-101` | Builder kill/removal (class 5 model 12 slot 101) | 5/12/101 | **Missing** | PLAN-15 | EventsFunctions.cpp:13934-14436 pinned (model-12 band) | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-05-102` | Builder hit/retaliation (class 5 model 12 slot 102) | 5/12/102 | **Missing** | PLAN-15 | EventsFunctions.cpp:13934-14436 pinned (model-12 band) | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-05-103` | Builder reset-to-wander (class 5 model 12 slot 103) | 5/12/103 | **Missing** | PLAN-15 | EventsFunctions.cpp:13934-14436 pinned (model-12 band) | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-05-104` | Villager entry (class 5 model 13 slot 104) | 5/13/104 | **Missing** | PLAN-16 | EventsFunctions.cpp:14491-14785 pinned (model-13 band) | villager::tests::lethal_damage_uses_source_actions_108_then_1... |
| `DSP-05-105` | Villager civilian movement/damage (class 5 model 13 slot 105) | 5/13/105 | **Missing** | PLAN-16 | EventsFunctions.cpp:14491-14785 pinned (model-13 band) | villager::tests::lethal_damage_uses_source_actions_108_then_1... |
| `DSP-05-106` | Villager entry variant (class 5 model 13 slot 106) | 5/13/106 | **Missing** | PLAN-16 | EventsFunctions.cpp:14491-14785 pinned (model-13 band) | villager::tests::lethal_damage_uses_source_actions_108_then_1... |
| `DSP-05-107` | Villager entry variant (class 5 model 13 slot 107) | 5/13/107 | **Missing** | PLAN-16 | EventsFunctions.cpp:14491-14785 pinned (model-13 band) | villager::tests::lethal_damage_uses_source_actions_108_then_1... |
| `DSP-05-108` | Villager death staging (class 5 model 13 slot 108) | 5/13/108 | **Missing** | PLAN-16 | EventsFunctions.cpp:14491-14785 pinned (model-13 band) | villager::tests::lethal_damage_uses_source_actions_108_then_1... |
| `DSP-05-109` | Villager kill/removal (class 5 model 13 slot 109) | 5/13/109 | **Missing** | PLAN-16 | EventsFunctions.cpp:14491-14785 pinned (model-13 band) | villager::tests::lethal_damage_uses_source_actions_108_then_1... |
| `DSP-05-110` | Villager hit/retaliation (class 5 model 13 slot 110) | 5/13/110 | **Missing** | PLAN-16 | EventsFunctions.cpp:14491-14785 pinned (model-13 band) | villager::tests::lethal_damage_uses_source_actions_108_then_1... |
| `DSP-05-111` | Villager scripted update (class 5 model 13 slot 111) | 5/13/111 | **Partial** | PLAN-16 | EventsFunctions.cpp:14491-14785 pinned (model-13 band) | villager::tests::lethal_damage_uses_source_actions_108_then_1... |
| `DSP-05-112` | Trader entry (class 5 model 14 slot 112) | 5/14/112 | **Missing** | PLAN-17 | EventsFunctions.cpp:14713-14990 pinned (model-14 band) | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-05-113` | Trader movement/damage (class 5 model 14 slot 113) | 5/14/113 | **Missing** | PLAN-17 | EventsFunctions.cpp:14713-14990 pinned (model-14 band) | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-05-114` | Trader entry variant (class 5 model 14 slot 114) | 5/14/114 | **Missing** | PLAN-17 | EventsFunctions.cpp:14713-14990 pinned (model-14 band) | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-05-115` | Trader entry variant (class 5 model 14 slot 115) | 5/14/115 | **Missing** | PLAN-17 | EventsFunctions.cpp:14713-14990 pinned (model-14 band) | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-05-116` | Trader death staging (class 5 model 14 slot 116) | 5/14/116 | **Missing** | PLAN-17 | EventsFunctions.cpp:14713-14990 pinned (model-14 band) | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-05-117` | Trader kill/removal (class 5 model 14 slot 117) | 5/14/117 | **Missing** | PLAN-17 | EventsFunctions.cpp:14713-14990 pinned (model-14 band) | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-05-118` | Trader hit/retaliation (class 5 model 14 slot 118) | 5/14/118 | **Missing** | PLAN-17 | EventsFunctions.cpp:14713-14990 pinned (model-14 band) | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-05-119` | Trader scripted update (class 5 model 14 slot 119) | 5/14/119 | **Missing** | PLAN-17 | EventsFunctions.cpp:14713-14990 pinned (model-14 band) | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-05-152` | Firefly idle (class 5 model 19 slot 152) | 5/19/152 | **Exact** | — | EventsFunctions.cpp:16253-16605 pinned (model-19 band) | level1_mission::tests::fireflies_bind_to_level1_stage_row_and... |
| `DSP-05-153` | Firefly active/awareness (class 5 model 19 slot 153) | 5/19/153 | **Exact** | — | EventsFunctions.cpp:16253-16605 pinned (model-19 band) | level1_mission::tests::fireflies_bind_to_level1_stage_row_and... |
| `DSP-05-154` | Firefly orbit/approach/attack/recovery (class 5 model 19 slot 154) | 5/19/154 | **Exact** | — | EventsFunctions.cpp:16253-16605 pinned (model-19 band) | level1_mission::tests::fireflies_bind_to_level1_stage_row_and... |
| `DSP-05-155` | Firefly hit reaction (class 5 model 19 slot 155) | 5/19/155 | **Exact** | — | EventsFunctions.cpp:16253-16605 pinned (model-19 band) | level1_mission::tests::fireflies_bind_to_level1_stage_row_and... |
| `DSP-05-156` | Firefly pre-kill (class 5 model 19 slot 156) | 5/19/156 | **Exact** | — | EventsFunctions.cpp:16253-16605 pinned (model-19 band) | level1_mission::tests::fireflies_bind_to_level1_stage_row_and... |
| `DSP-05-157` | Firefly kill/removal (class 5 model 19 slot 157) | 5/19/157 | **Exact** | — | EventsFunctions.cpp:16253-16605 pinned (model-19 band) | level1_mission::tests::fireflies_bind_to_level1_stage_row_and... |
| `DSP-05-159` | Firefly scripted update (class 5 model 19 slot 159) | 5/19/159 | **Exact** | — | EventsFunctions.cpp:16253-16605 pinned (model-19 band) | level1_mission::tests::fireflies_bind_to_level1_stage_row_and... |
| `DSP-09-00` | Ordinary Fireball carrier (class 9 slot 0) | 9/0/0 | **Exact** | — | EventsFunctions.cpp:63045-63050 pinned | level1_mission::tests::fireball_uses_source_constructor_const... |
| `DSP-09-01` | Possession carrier (slot 1) | 9/1/1 | **Exact** | — | EventsFunctions.cpp:63250-63367 pinned | level1_mission::tests::possession_claims_neutral_mana_sphere_... |
| `DSP-09-10` | Castle-placement carrier (slot 10) | 9/10/10 | **Partial** | PLAN-22 | EventsFunctions.cpp:58501-58597 pinned | level1_mission::tests::castle_cast_uses_exact_cost_holder_and... |
| `DSP-09-13` | Arrow carrier (slot 13) | 9/13/13 | **Exact** | — | EventsFunctions.cpp:58892-58944 pinned | level1_mission::tests::archer_constructor_cadence_acquires_an... |
| `DSP-10-00` | Ordinary Fireball impact (slot 0) | 10/0/0 | **Exact** | — | EventsFunctions.cpp:class-10 table 1601-1796 pinned | level1_mission::tests::fireball_impact_advances_rng_motion_an... |
| `DSP-10-01` | Large explosion/death effect (slot 1) | 10/1/1 | **Missing** | PLAN-18 | EventsFunctions.cpp:class-10 table 1601-1796 pinned | — |
| `DSP-10-02` | Speed spell timed trail (slot 2) | 10/2/2 | **Exact** | — | EventsFunctions.cpp:class-10 table 1601-1796 pinned | level1_mission::tests::speed_holder_and_trail_follow_source_c... |
| `DSP-10-05` | Water splash / ash effect (slot 5) | 10/5/5 | **Partial** | PLAN-12 | EventsFunctions.cpp:class-10 table 1601-1796 pinned | level1_mission::tests::firefly_fireball_water_hit_creates_nin... |
| `DSP-10-13` | Smoke particle 3B (slot 13, model 13) | 10/13/13 | **Exact** | — | EventsFunctions.cpp:class-10 table 1601-1796 pinned | level1_mission::tests::checkpoint_emitters_and_model_thirteen... |
| `DSP-10-14` | Smoke particle 3C (slot 14, model 14) | 10/14/14 | **Missing** | PLAN-18 | EventsFunctions.cpp:class-10 table 1601-1796 pinned | — |
| `DSP-10-29` | Rectangular terrain raise command 29 | 10/29/29 | **Partial** | PLAN-24 | EventsFunctions.cpp:class-10 table 1601-1796 pinned | frame-0 terrain planes embed the raise results |
| `DSP-10-41` | Primary mana-sphere simulation (slot 41) | 10/4/41 | **Exact** | — | EventsFunctions.cpp:class-10 table 1601-1796 pinned | level1_mission::tests::mana_sphere_ground_bounce_and_slope_im... |
| `DSP-10-42` | Linked mana/death cleanup (slot 42) | 10/-/42 | **Exact** | — | EventsFunctions.cpp:class-10 table 1601-1796 pinned | level1_mission::tests::dead_carrier_converts_cargo_once_then_... |
| `DSP-10-44` | BUILD footprint growth (slot 44) | 10/-/44 | **Partial** | PLAN-13 | EventsFunctions.cpp:class-10 table 1601-1796 pinned | level1_mission::tests::castle_construction_allocation_is_atom... |
| `DSP-10-45` | Castle association effect (slot 45) | 10/-/45 | **Partial** | PLAN-22 | EventsFunctions.cpp:class-10 table 1601-1796 pinned | level1_mission::tests::accepted_castle_activation_reaches_the... |
| `DSP-10-48` | Progressive terrain modification (slot 48) | 10/-/48 | **Partial** | PLAN-13 | EventsFunctions.cpp:class-10 table 1601-1796 pinned | level1_mission::tests::castle_root_resamples_deformed_ground_... |
| `DSP-10-51` | Building terrain modification entry (slot 51) | 10/-/51 | **Partial** | PLAN-13 | EventsFunctions.cpp:class-10 table 1601-1796 pinned | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-10-52` | Live house/building update (slot 52) | 10/45/52 | **Partial** | PLAN-13 | EventsFunctions.cpp:class-10 table 1601-1796 pinned | level1_mission::tests::fireball_impact_writes_and_castle_slot... |
| `DSP-10-53` | Building/castle demolition (slot 53) | 10/45/53 | **Partial** | PLAN-13 | EventsFunctions.cpp:class-10 table 1601-1796 pinned | level1_mission::tests::house_resident_constructors_own_exact_... |
| `DSP-10-58` | Expanding explosion ring (slot 58) | 10/-/58 | **Unproved** | PLAN-25 | EventsFunctions.cpp:class-10 table 1601-1796 pinned | level1_mission::tests::group_four_constructs_scroll_switches_... |
| `DSP-10-64` | Smoke emitter 3B (slot 64) | 10/59/64 | **Exact** | — | EventsFunctions.cpp:class-10 table 1601-1796 pinned | level1_mission::tests::checkpoint_emitters_and_model_thirteen... |
| `DSP-10-65` | Smoke emitter 3C (slot 65) | 10/60/65 | **Missing** | PLAN-18 | EventsFunctions.cpp:class-10 table 1601-1796 pinned | — |
| `DSP-10-86` | Castle-defense controller (slot 86) | 10/-/86 | **Partial** | PLAN-19 | EventsFunctions.cpp:class-10 table 1601-1796 pinned | castle projectiles rendered in compositor and route-exercised |
| `DSP-10-98` | Reward/mana burst (slot 98) | 10/-/98 | **Partial** | PLAN-20 | EventsFunctions.cpp:class-10 table 1601-1796 pinned | level1_mission::tests::experience_scroll_awards_every_enabled... |
| `DSP-10-12` | Possession impact variant 0 (slot 12) | 10/12/12 | **Exact** | — | EventsFunctions.cpp:class-10 table 1601-1796 pinned | level1_mission::tests::expired_possession_spawns_model_twelve... |
| `DSP-10-77` | Possession impact variant 1 (slot 77) | 10/-/77 | **Partial** | PLAN-25 | EventsFunctions.cpp:class-10 table 1601-1796 pinned | level1_mission::tests::possession_claims_neutral_mana_sphere_... |
| `DSP-10-COND` | Conditional class-10 slots requiring reachability resolution: 6 (ground fire), 1 | 10/-/- | **Unproved** | PLAN-25 | EventsFunctions.cpp:class-10 table 1601-1796 pinned | — |
| `DSP-14-08` | Exit-target marker fixture (class 14 slot 8, sprite 338) | 14/3/8 | **Exact** | — | EventsFunctions.cpp:41138-41145 pinned | level1_mission::tests::exit_action_fov_target_timeout_and_axi... |
| `DSP-14-10` | Collectible spell scroll (class 14 slot 10) | 14/5/10 | **Exact** | — | EventsFunctions.cpp:41158-41196 pinned | level1_mission::tests::experience_scroll_awards_every_enabled... |
| `DSP-15-00` | Fireball equipped/active spell-holder state (class 15 slot 0) | 15/0/0 | **Exact** | — | EventsFunctions.cpp:table 1949-2059 pinned | level1_mission::tests::fireball_uses_source_constructor_const... |
| `DSP-15-01` | Fireball world-pickup spell-holder state (class 15 slot 1) | 15/1/1 | **Partial** | PLAN-20 | EventsFunctions.cpp:table 1949-2059 pinned | level1_mission::tests::spatial_and_objective_switch_chain_act... |
| `DSP-15-02` | Fireball respawning-pickup spell-holder state (class 15 slot 2) | 15/2/2 | **Partial** | PLAN-20 | EventsFunctions.cpp:table 1949-2059 pinned | — |
| `DSP-15-03` | Possession equipped/active spell-holder state (class 15 slot 3) | 15/3/3 | **Exact** | — | EventsFunctions.cpp:table 1949-2059 pinned | level1_mission::tests::possession_claims_neutral_mana_sphere_... |
| `DSP-15-04` | Possession world-pickup spell-holder state (class 15 slot 4) | 15/4/4 | **Partial** | PLAN-20 | EventsFunctions.cpp:table 1949-2059 pinned | — |
| `DSP-15-05` | Possession respawning-pickup spell-holder state (class 15 slot 5) | 15/5/5 | **Partial** | PLAN-20 | EventsFunctions.cpp:table 1949-2059 pinned | — |
| `DSP-15-06` | Castle equipped/active spell-holder state (class 15 slot 6) | 15/6/6 | **Partial** | PLAN-22 | EventsFunctions.cpp:table 1949-2059 pinned | level1_mission::tests::castle_cast_uses_exact_cost_holder_and... |
| `DSP-15-07` | Castle world-pickup spell-holder state (class 15 slot 7) | 15/7/7 | **Partial** | PLAN-20 | EventsFunctions.cpp:table 1949-2059 pinned | — |
| `DSP-15-08` | Castle respawning-pickup spell-holder state (class 15 slot 8) | 15/8/8 | **Partial** | PLAN-20 | EventsFunctions.cpp:table 1949-2059 pinned | — |
| `DSP-15-09` | Speed equipped/active spell-holder state (class 15 slot 9) | 15/9/9 | **Partial** | PLAN-23 | EventsFunctions.cpp:table 1949-2059 pinned | level1_mission::tests::speed_spell_uses_exact_countdown_speed... |
| `DSP-15-10` | Speed world-pickup spell-holder state (class 15 slot 10) | 15/10/10 | **Partial** | PLAN-20 | EventsFunctions.cpp:table 1949-2059 pinned | level1_mission::tests::speed_release_is_model17_timed_and_ind... |
| `DSP-15-11` | Speed respawning-pickup spell-holder state (class 15 slot 11) | 15/11/11 | **Partial** | PLAN-20 | EventsFunctions.cpp:table 1949-2059 pinned | — |

### combat

| Row | Title | Class/Model/Slot | Classification | Plan | Key source anchor | Key evidence |
|---|---|---|---|---|---|---|
| `CMB-001` | Pending-damage mailbox: write/overwrite/consume ordering | — | **Exact** | — | EventsFunctions.cpp:per-consumer bodies | level1_mission::tests::firefly_contact_mailbox_overwrites_an_... |
| `CMB-002` | Player hit reaction: RNG recoil, timers, palette, mailbox clear | — | **Exact** | — | EventsFunctions.cpp:60003-60090 working-8 | level1_mission::tests::player_hit_reaction_matches_source_rng... |
| `CMB-003` | Creature death staging: pre-kill wrappers, kill, mana conversion, attacker credi | — | **Exact** | — | EventsFunctions.cpp:9524-9561 | level1_mission::tests::death_burst_allocates_after_sphere_and... |
| `CMB-004` | Experience, spell-tier progression and score accounting | — | **Exact** | — | EventsFunctions.cpp:41158-41196 pinned + consumers | level1_mission::tests::experience_scroll_awards_every_enabled... |

### targeting

| Row | Title | Class/Model/Slot | Classification | Plan | Key source anchor | Key evidence |
|---|---|---|---|---|---|---|
| `TGT-001` | Per-model acquisition list ordering and tie-breaking across all seekers | — | **Partial** | PLAN-19 | EventsFunctions.cpp:3739-3782 pinned | level1_mission::tests::source_guidance_lists_use_model_then_a... |

### economy

| Row | Title | Class/Model/Slot | Classification | Plan | Key source anchor | Key evidence |
|---|---|---|---|---|---|---|
| `ECO-001` | Mana conservation: sphere creation, merge, collection, castle fill percent | — | **Exact** | — | EventsFunctions.cpp:28110-28150 pinned + castle body | level1_mission::tests::castle_gate_uses_only_secured_mana_and... |

### objective

| Row | Title | Class/Model/Slot | Classification | Plan | Key source anchor | Key evidence |
|---|---|---|---|---|---|---|
| `OBJ-001` | Six Level 1 objective predicates and stage progression | — | **Exact** | — | EventsFunctions.cpp:40567-40649 | level1_mission::tests::four_archers_plus_three_goats_fail_but... |
| `OBJ-002` | Completion sequence: exit collision, fade, result code 10, restart | — | **Exact** | — | EventsFunctions.cpp:60353-60636 pinned | level1_mission::tests::group4_exit_switch_runs_source_timers_... |

### rng

| Row | Title | Class/Model/Slot | Classification | Plan | Key source anchor | Key evidence |
|---|---|---|---|---|---|---|
| `RNG-001` | Complete per-turn ordered global RNG draw census | — | **Partial** | PLAN-26 | EventsFunctions.cpp:multiple | ten-advance observation |

### terrain

| Row | Title | Class/Model/Slot | Classification | Plan | Key source anchor | Key evidence |
|---|---|---|---|---|---|---|
| `TER-001` | Terrain deformation census: build/demolition/raise commands/exit interactions | — | **Partial** | PLAN-13 | EventsFunctions.cpp:28086-28180 pinned | level1_mission::tests::castle_root_resamples_deformed_ground_... |

### audio

| Row | Title | Class/Model/Slot | Classification | Plan | Key source anchor | Key evidence |
|---|---|---|---|---|---|---|
| `AUD-001` | Shared ten-voice effect mixer, arbitration, ambient loops, completion reporting | — | **Exact** | — | Sound.cpp:6254-6345, 6426-6452 | native_v2::tests::pending_slot_and_ten_voice_lifecycle_match_... |
| `AUD-002` | Reachable mission sound-emitter census with pitch families and play types | — | **Partial** | PLAN-26 | Sound.cpp:6254-6345 | readiness: every reachable mission sound tied to its remc2 em... |

### input

| Row | Title | Class/Model/Slot | Classification | Plan | Key source anchor | Key evidence |
|---|---|---|---|---|---|---|
| `INP-001` | Desktop input translation: mouse/keys to ten-byte commands, dead-input suppressi | — | **Exact** | — | EventsFunctions.cpp:31734-31762 pinned | tests::command_layout_and_profiles_are_exactly_recorded |
| `INP-002` | Spell hand/tier selection menu semantics (opcodes 31/32, two-stage family/tier) | — | **Partial** | PLAN-21 | PlayerInput.cpp:menu handling regions | desktop menu operates (controls section) |

### presentation-abi

| Row | Title | Class/Model/Slot | Classification | Plan | Key source anchor | Key evidence |
|---|---|---|---|---|---|---|
| `ABI-001` | Native ABI: snapshots, 72-byte events, 88-byte audio sidecar, generations, reset | — | **Exact** | — | EventsFunctions.cpp:n/a (implementation boundary | native_v2::tests::layouts_and_versions_are_stable |
| `PRS-001` | Indexed world compositor: source-slot order, reflection-before-primary, shadows, | — | **Partial** | PLAN-28 | GameRenderOriginal.cpp:sprite interleave region | five 1600x900 checkpoints, two byte-identical runs, zero magenta |
| `PRS-002` | Minimap, HUD, objective textbox, palette sidecar | — | **Exact** | — | GameUI.cpp:minimap/HUD regions | level1_mission::tests::minimap_classifier_preserves_night_fil... |

## Exclusions (mechanical reachability proofs)

| Exclusion | Scope | Excluded slots | Proof summary |
|---|---|---:|---|
| `EXC-CLS-EMPTY` | classes 4, 6, 7, 8, 12 | 10 | class registry rows are empty/null-sentinel tables at the pinned commit; no allocation path can produce these classes |
| `EXC-CLS-META` | classes 1, 13 | 33 | action/constructor tables are addressless; dispatcher performs no call for these classes |
| `EXC-C5-MODELS` | class 5 models 0, 2, 5-11, 15-18, 20-28 | 193 | owner authored table contains only kind-5 subtypes {1,3,4,13,19}; house residents spawn only models 4/12/13/14 (action-53 body); n |
| `EXC-C5-MODEL9` | class 5 model 9 (radial burst spawn) | 9 | no Level 1 reachable path to a class-10 entity with action 38 has been demonstrated; tracked as conditional row DSP-10-COND until  |
| `EXC-C3-WIZARD` | class 3 model 1 (opponent wizard AI) | 2 | owner data: exactly one class-3 entity at reset (the player); authored table has no kind-3 rows besides the position marker (subty |
| `EXC-C3-BALLOON-STATES` | class 3 slots 7, 8, 10, 11 | 4 | slots 7/8/10 dispatch to deliberate no-call cases; end-game variant 11 is selected only by non-Level-1 paths (exit switch writes a |
| `EXC-C9-MODELS` | class 9 models 2-9, 11, 12, 14-31 | 57 | Level 1 enables only Fireball/Possession/Castle/Speed (owner SPELLS.DAT + level spell table); Speed launches no projectile; arrow  |
| `EXC-C11-MODELS` | class 11 models 2, 3, 5-11, 13-15, 18-31, 33+ | 78 | authored census contains only kind-11 subtypes {0,1,4,12,16,17,32}; no runtime path constructs class-11 entities |
| `EXC-C14-SLOTS` | class 14 slots 0-7, 9 | 15 | slots 0-5 are deliberate no-call dispatcher cases; slots 6/7 (persistent terrain/cave controllers) and 9 (sprite-339 marker) have  |
| `EXC-C15-FAMILIES` | class 15 slots 12-78 (families 4-25) | 90 | Level 1 owner data enables only families 0-3; scrolls award only enabled spells (experience_scroll test proves the enabled set); n |
| `EXC-C2-SLOTS` | class 2 slots 4, 5, 7, 8, 9, 10-21 | 25 | slots 4/5/7/8 are enabled rows whose stored addresses are absent from the reconstructed dispatcher (no call at the pinned commit); |
| `EXC-C10-REST` | class 10 remaining slots (3,4,7-11,15,17-28,30-40,43,46-51 e | 148 | each slot's creators are spell carriers or handlers excluded above (non-enabled spells: volcano, meteor, crater, lightning, gravit |
| `EXC-C5-MISSING-SLOTS` | class 5 slots 30, 38, 158 (and equivalents in reachable band | 3 | remc2 at the pinned commit performs no call for these slots; matrix mirrors remc2 including its reconstruction gaps; any transitio |
| `EXC-MULTIPLAYER` | network/multiplayer paths | 0 | Level 1 owner desktop loop is single-player; PlayerEvents network substitution branches are gated by multiplayer mode flags that a |
| `EXC-CHEAT-DEBUG` | cheat/debug command paths | 0 | DoDebugSequences/command-line gates are disabled in the qualified owner configuration; not part of the ordinary-input mission loop |
| `EXC-GROUPS-EMPTY` | activation groups 7, 9, 10, 35+ | 0 | authored census: no rows carry DisId 7/9/10; activation is a stage-broadcast-only side effect (covered by ACT-001); no switch carr |
| `EXC-C3-MARKERS` | class 3 constructor slots 5-12 (marker writers 1-7 and trail | 8 | Level 1 owner data has exactly one populated player (players.bin slot 0); authored table has one kind-3 marker row (subtype 4, pla |
| `EXC-TABLE-NULL` | null-sentinel / disabled trailing table rows | 2 | the committed dispatch tables mark these rows as null/disabled; the dispatcher performs no call and no allocation path can select  |

## Consistency checks (recomputed by the validator on every run)

- [x] `unique-row-ids`: rows[].id unique; exclusions[].id unique; plan_items[].id unique
- [x] `totals`: meta.totals recomputable from rows[].classification and by_subsystem
- [x] `exact-evidence`: every Exact row has >=1 rust-test binding whose test is in the Cargo --list inventory, and zero non_executable bindings
- [x] `rust-test-resolution`: every rust-test binding on every row resolves exactly against the inventory
- [x] `non-exact-ownership`: every non-Exact row names exactly one plan_item that exists; owned_rows lists reproduce this mapping bijectively
- [x] `plan-acyclic`: plan_items depends_on references exist and form a DAG
- [x] `authored-census`: authored_census.row_coverage values are unique, pairwise disjoint, and union to nonzero_indices (308); each covering row's authored_indices equals its coverage entry
- [x] `dispatch-census`: slot_to_row and slot_to_exclusion are disjoint, union to slots (845); every mapped row/exclusion ID exists

## Reading guide

Full per-row detail — reachability chains, ordered behavior, field-level mismatch,
resolved evidence bindings, negative coverage, confidence, unresolved questions,
typed authored and dispatch censuses — lives in the JSON. Row IDs are stable and are
the only cross-reference keys used by `IMPLEMENTATION-PLAN.md` and the Beads graph.
