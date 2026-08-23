# Level 2 canonical state census

This is the live guard input for CF-09. It describes the post-cutover carrier
shape, not the retired pre-cutover `baseline_*` layout. Repository source and
the executable reset/replay tests remain authoritative.

The classification rule is structural:

- `CampaignDefinition` is immutable package/engine definition state.
- `Level2LiveState` and its nested domain carriers are resettable live state.
- `ScratchState` is derived or cursor-bound state and is excluded from the
  reset receipt.
- `PersistentState` is currently a unit type: no Level 2 field is proven to
  survive reset.
- `CampaignSession` owns exactly one shared definition and exactly one shared
  immutable reset baseline. `Level2ResetSession` is only its compatibility
  facade.

| Carrier | Fields | Classification |
|---|---|---|
| `CampaignDefinition` | `profile`, `package_sha256`, `collision_masks_present`, `identity_receipt`, `collision_masks`, `presentation` | immutable definition |
| `CanonicalRuntimeState` | `state`, `players`, `runtime`, `context`, `effect_journal` | resettable live |
| `ScratchState` | `validity_epoch`, `valid`, `dispatch_receipt`, `dispatch_events`, `economy_event_cursor`, `nyphur_dispatch_traces`, `level2_day_hud_color_phase_1`, `level2_day_hud_color_phase_2`, `level2_day_hud_hit_frame_time_pre_draw`, `level2_day_hud_player_panel_selector` | derived scratch |
| `CampaignState` | `persistent`, `live`, `scratch` | disjoint state classes |
| `CampaignSession` | `definition`, `baseline`, `baseline_hash`, `state`, `generation` | session shell and lifecycle |
| `Level2LiveState` | `core`, `terrain`, `economy`, `bad_stones`, `shared_spells`, `ordinary`, `goats`, `mission`, `nyphur_dispatcher`, `player_pending_damage`, `player_pending_attacker`, `player_life`, `level2_group8_activated`, `level2_selected_objective_stage_state`, `level2_objective_pause`, `level2_objective_done`, `level2_mana_attraction_activated`, `level2_model13_activated`, `level2_bees_activated`, `level2_switch_producers_activated`, `level2_model32_activated`, `level2_model76_activated`, `level2_model22_activated`, `level2_model39_activated`, `level2_model6_activated`, `level2_model59_activated`, `level2_model53_activated`, `level2_model1_activated`, `level2_model17_activated`, `level2_imported_switch_roots`, `level2_group10_switch25`, `level2_class14_activated`, `level2_tree_activated`, `level2_class5_activated`, `level2_model45_activated` | resettable live |
| `Level2ResetSession` | `session` | compatibility facade only |

Nested carriers `Level2EconomyState`, `Level2SharedSpellResetSession`,
`Level2OrdinaryResetSession`, `SourceArena`, and `MissionContentSession` are
wholly contained by `Level2LiveState`; they may not own `baseline_*` fields.
The guard checks that prohibition directly. Generation belongs to
`CampaignSession`, not `EffectJournal`, so `clone_from(baseline)` restores
identical reset bytes while old effect cursors still fail closed.

Focused proof:

- `campaign::state::tests::cf09_reset_hash_isomorphism_and_live_only_transaction_clone`
- `campaign::state::tests::cf09_repeated_reset_generation_and_replay_are_exact`
- `level2_state_census_guard` including its planted omission negative
