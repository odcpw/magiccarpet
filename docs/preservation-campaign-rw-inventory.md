# Level 2 read/write/RNG/emission inventory

This document is a deterministic summary of the machine-readable artifact at research/private/remc2-atlas/artifacts/campaign-rw-inventory.json. It is a static, source-backed inventory; unresolved packed-byte aliases and indirect callbacks stay open.

## Route and evidence

- Route: ordinary_level2_source_route
- Rust entry: research/private/remc2-sim-rs/crates/sim-core/src/native_world.rs:24356 (step_outer_turn)
- Rust source pass: research/private/remc2-sim-rs/crates/sim-core/src/native_world.rs:22094 (step_level2_source_pass_with_player)
- Pinned remc2 schedule anchors: UpdateEntities_57730 (EventsFunctions.cpp:39928 in the current checkout) and ApplyEvents_498A0 (Events.cpp:410).
- Reached transition records: 54
- Open-question count: 224

The route follows the player input prepass, source-pass prologue, source-slot route kinds, same-pass cursor and epilogue, following-pass reclaim boundary, and objective/HUD tail. Each record carries ordered RNG and emission observations from its Rust and source slices.

## Transition schedule

| ID | Name | Schedule | Slot kind | Reads | Writes | RNG | Emissions | Open |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| outer-prepass-000-set_level2_entity_position | set_level2_entity_position | prepass |  | 9 | 4 | 0 | 0 | 2 |
| outer-prepass-001-set_beyond_sight_player_input | set_beyond_sight_player_input | prepass |  | 0 | 3 | 0 | 0 | 0 |
| outer-prepass-002-step_day_hud_player_events | step_day_hud_player_events | prepass |  | 3 | 1 | 0 | 0 | 1 |
| outer-prepass-003-sync_level2_day_hud_player_projection | sync_level2_day_hud_player_projection | prepass |  | 5 | 2 | 0 | 0 | 2 |
| outer-prepass-004-sync_level2_spell_fire_context | sync_level2_spell_fire_context | prepass |  | 5 | 0 | 0 | 0 | 1 |
| outer-prepass-005-reconcile_level2_spell_menu_command | reconcile_level2_spell_menu_command | prepass |  | 4 | 2 | 0 | 0 | 2 |
| following-pass-cleanup-000-reclaim_castle_upgrade_products_at_source_boundary | reclaim_castle_upgrade_products_at_source_boundary | following_pass_cleanup |  | 5 | 4 | 0 | 0 | 6 |
| source-prepass-000-refresh_level2_spell_owner_candidates | refresh_level2_spell_owner_candidates | prepass |  | 7 | 1 | 0 | 0 | 4 |
| source-prepass-001-advance_global_rng | advance_global_rng | prepass |  | 2 | 2 | 1 | 0 | 0 |
| source-prepass-002-reclaim_possession_rows_at_source_boundary | reclaim_possession_rows_at_source_boundary | prepass |  | 9 | 4 | 0 | 0 | 0 |
| source-prepass-003-reclaim_fireball_rows_at_source_boundary | reclaim_fireball_rows_at_source_boundary | prepass |  | 8 | 4 | 2 | 0 | 0 |
| source-prepass-004-reclaim_hidden_castle_rows_at_source_boundary | reclaim_hidden_castle_rows_at_source_boundary | prepass |  | 14 | 6 | 2 | 0 | 4 |
| source-prepass-005-rebuild_possession_guidance_source_slots | rebuild_possession_guidance_source_slots | prepass |  | 1 | 1 | 0 | 0 | 0 |
| source-prepass-006-rebuild_owner_mana_projection_at_source_boundary | rebuild_owner_mana_projection_at_source_boundary | prepass |  | 8 | 1 | 0 | 0 | 3 |
| source-prepass-007-rebuild_owner_max_mana_projection | rebuild_owner_max_mana_projection | prepass |  | 2 | 3 | 0 | 0 | 0 |
| source-prepass-008-synchronize_level2_terrain | synchronize_level2_terrain | prepass |  | 1 | 3 | 0 | 0 | 0 |
| source-prepass-009-step_level2_imported_switch_roots | step_level2_imported_switch_roots | prepass |  | 24 | 13 | 4 | 12 | 3 |
| source-prepass-010-activate_level2_authored_group8 | activate_level2_authored_group8 | prepass |  | 10 | 6 | 0 | 5 | 3 |
| source-prepass-011-activate_level2_authored_group31 | activate_level2_authored_group31 | prepass |  | 8 | 4 | 1 | 10 | 4 |
| source-prepass-012-step_level2_group10_switch25 | step_level2_group10_switch25 | prepass |  | 7 | 3 | 0 | 2 | 2 |
| source-prepass-013-activate_level2_authored_bees | activate_level2_authored_bees | prepass |  | 2 | 1 | 0 | 2 | 5 |
| source-prepass-014-activate_level2_authored_model13 | activate_level2_authored_model13 | prepass |  | 2 | 1 | 0 | 2 | 5 |
| source-prepass-015-activate_level2_authored_model76 | activate_level2_authored_model76 | prepass |  | 2 | 1 | 0 | 2 | 6 |
| source-prepass-016-activate_level2_authored_class14 | activate_level2_authored_class14 | prepass |  | 2 | 1 | 0 | 2 | 6 |
| source-prepass-017-sync_level2_model76_targets_from_entities | sync_level2_model76_targets_from_entities | prepass |  | 1 | 0 | 0 | 0 | 1 |
| source-prepass-018-begin_source_turn | begin_source_turn | prepass |  | 1 | 0 | 0 | 2 | 1 |
| source-route-construction | source_slot_route_construction | slot_visit |  | 8 | 0 | 0 | 0 | 3 |
| slot-kind-09 | local_player | slot_visit | 9 | 47 | 41 | 0 | 2 | 30 |
| slot-kind-00 | shared_spell_pickup | slot_visit | 0 | 4 | 7 | 0 | 2 | 0 |
| slot-kind-08 | generic_spell_pickup | slot_visit | 8 | 9 | 16 | 0 | 2 | 14 |
| slot-kind-01 | bad_stone | slot_visit | 1 | 1 | 2 | 0 | 2 | 1 |
| slot-kind-02 | ordinary_house | slot_visit | 2 | 4 | 8 | 2 | 10 | 0 |
| slot-kind-03 | ordinary_villager | slot_visit | 3 | 20 | 13 | 0 | 7 | 7 |
| slot-kind-04 | ordinary_dolmen | slot_visit | 4 | 4 | 6 | 0 | 2 | 2 |
| slot-kind-05 | castle | slot_visit | 5 | 26 | 17 | 0 | 6 | 3 |
| slot-kind-06 | dynamic_arena_row | slot_visit | 6 | 28 | 23 | 0 | 13 | 20 |
| slot-kind-07 | goat | slot_visit | 7 | 7 | 5 | 3 | 11 | 3 |
| slot-kind-10 | nyphur | slot_visit | 10 | 8 | 8 | 0 | 1 | 1 |
| slot-kind-11 | possession_holder | slot_visit | 11 | 16 | 4 | 0 | 0 | 3 |
| slot-kind-12 | castle_holder | slot_visit | 12 | 18 | 7 | 0 | 1 | 4 |
| slot-kind-13 | fireball_holder | slot_visit | 13 | 26 | 16 | 0 | 1 | 14 |
| same-pass-cursor-step_level2_pending_possession_dynamic_row-000-step_level2_pending_possession_dynamic_row | step_level2_pending_possession_dynamic_row | same_pass_epilogue |  | 25 | 20 | 0 | 4 | 16 |
| same-pass-cursor-schedule_new_possession_rows_in_source_order-000-schedule_new_possession_rows_in_source_order | schedule_new_possession_rows_in_source_order | same_pass_epilogue |  | 3 | 1 | 0 | 0 | 1 |
| same-pass-epilogue-000-activate_level2_authored_mana_attraction_fields | activate_level2_authored_mana_attraction_fields | same_pass_epilogue |  | 7 | 16 | 0 | 2 | 19 |
| same-pass-epilogue-001-reclaim_inactive_slots | reclaim_inactive_slots | same_pass_epilogue |  | 27 | 49 | 0 | 0 | 18 |
| same-pass-epilogue-002-live_arena_cursor_rows | live_arena_cursor_rows | same_pass_epilogue |  | 7 | 15 | 0 | 0 | 13 |
| same-pass-epilogue-003-activate_level2_authored_spell_group | activate_level2_authored_spell_group | same_pass_epilogue |  | 12 | 21 | 0 | 7 | 16 |
| same-pass-epilogue-004-step_level2_dynamic_spell_pickup | step_level2_dynamic_spell_pickup | same_pass_epilogue |  | 10 | 17 | 0 | 2 | 15 |
| same-pass-epilogue-005-step_level2_dynamic_spell_replacement | step_level2_dynamic_spell_replacement | same_pass_epilogue |  | 13 | 18 | 0 | 2 | 13 |
| same-pass-epilogue-006-step_level2_dynamic_arena | step_level2_dynamic_arena | same_pass_epilogue |  | 102 | 59 | 15 | 10 | 69 |
| same-pass-epilogue-007-append_new_economy_events | append_new_economy_events | same_pass_epilogue |  | 7 | 17 | 0 | 1 | 13 |
| same-pass-epilogue-008-finish_source_turn | finish_source_turn | same_pass_epilogue |  | 5 | 16 | 0 | 0 | 13 |
| outer-postpass-000-finish_level2_day_hud_frame | finish_level2_day_hud_frame | postpass |  | 33 | 14 | 0 | 0 | 28 |
| outer-postpass-001-advance_level2_objectives | advance_level2_objectives | postpass |  | 36 | 11 | 0 | 0 | 27 |

## Cell classification

Every observed cell has a classification and file:line evidence. A null classification is deliberate: the cell is listed in the artifact with an open question instead of being guessed.

- immutable_fact: decoded or compiled content that the transition does not author during the session.
- session_state: mutable simulation or session carrier.
- derived_index: cursor, projection, cache, route, or other recomputed index.
- presentation_only: event, trace, HUD, audio, or evidence output.

## Known unreached and open callbacks

- partially_unreached — sub_58F00_game_objectives -> PresentObjective_59820 -> PlayEntitySounds_6E150: The Rust route reaches its objective and HUD tail, but the renderer and audio callbacks remain Unity presentation responsibilities and are not executed by sim-core.
- unreached — all disabled or unselected action and constructor table rows: The ordinary route materializes only the Level 2 source-slot families and live dynamic rows; the complete 845-row dispatch denominator is outside this route inventory.
- open — indirect callback and packed-byte aliases: Static source parsing records the containing transition and the unresolved access, but does not guess an alias or callback target.

The full artifact records the exact evidence list and open questions for every transition. No sim-core, sim-capi, exporter, owner-data package, Unity, or pinned remc2 source file is modified by this measurement lane.
