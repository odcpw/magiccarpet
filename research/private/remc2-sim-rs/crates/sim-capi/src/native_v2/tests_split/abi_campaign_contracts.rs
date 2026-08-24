#[test]
fn layouts_and_versions_are_stable() {
    assert_eq!(mc_native_abi_version(), 2);
    assert_eq!(size_of::<McWorldInfoV2>(), 72);
    assert_eq!(size_of::<McCommandV2>(), 32);
    assert_eq!(size_of::<McCameraInputV2>(), 28);
    assert_eq!(size_of::<McSessionSnapshotV2>(), 120);
    assert_eq!(size_of::<McObjectiveSnapshotV2>(), 40);
    assert_eq!(size_of::<McCampaignObjectiveStageSnapshotV2>(), 48);
    assert_eq!(size_of::<McEntitySnapshotV2>(), 52);
    assert_eq!(size_of::<McCampaignSelectionV2>(), 32);
    assert_eq!(size_of::<McCampaignStepInputV2>(), 80);
    assert_eq!(size_of::<McCampaignEventV2>(), 48);
    assert_eq!(size_of::<McCampaignAuthoredGroupEventV2>(), 48);
    assert_eq!(size_of::<McCampaignEconomyEventV2>(), 64);
    assert_eq!(size_of::<McCampaignGoatEventV2>(), 64);
    assert_eq!(size_of::<McCampaignStepSnapshotV2>(), 112);
    assert_eq!(std::mem::offset_of!(McCampaignStepInputV2, command), 16);
    assert_eq!(std::mem::offset_of!(McCampaignEventV2, source_slot), 24);
    assert_eq!(
        std::mem::offset_of!(McCampaignAuthoredGroupEventV2, authored_index),
        28
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignEconomyEventV2, primary_id),
        28
    );
    assert_eq!(std::mem::offset_of!(McCampaignEconomyEventV2, value_0), 40);
    assert_eq!(std::mem::offset_of!(McCampaignGoatEventV2, entity_0), 28);
    assert_eq!(std::mem::offset_of!(McCampaignGoatEventV2, value_0), 32);
    assert_eq!(std::mem::offset_of!(McCampaignGoatEventV2, sound_id), 40);
    assert_eq!(
        std::mem::offset_of!(McCampaignGoatEventV2, asset_ref_index),
        52
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignStepSnapshotV2, dispatch_hash),
        40
    );
    assert_eq!(size_of::<McCampaignWorldInfoV2>(), 64);
    assert_eq!(size_of::<McCampaignTerrainStateV2>(), 64);
    assert_eq!(size_of::<McCampaignPlayerSnapshotV2>(), 48);
    assert_eq!(size_of::<McCampaignEntitySnapshotV2>(), 52);
    assert_eq!(size_of::<McCampaignShieldSnapshotV2>(), 72);
    assert_eq!(size_of::<McCampaignInvisibleSnapshotV2>(), 80);
    assert_eq!(size_of::<McCampaignBeyondSightSnapshotV2>(), 80);
    assert_eq!(size_of::<McCampaignOpponentSnapshotV2>(), 104);
    assert_eq!(size_of::<McCampaignDayHudSpellSlotV2>(), 48);
    assert_eq!(size_of::<McCampaignDayHudSnapshotV2>(), 272);
    assert_eq!(size_of::<McCampaignDayMusicSnapshotV2>(), 80);
    assert_eq!(size_of::<McCampaignDayIndexedFrameInfoV2>(), 152);
    assert_eq!(size_of::<McCampaignMinimapStateV2>(), 72);
    assert_eq!(size_of::<McCampaignMinimapEntityV2>(), 56);
    assert_eq!(size_of::<McCampaignMinimapMarkV2>(), 48);
    assert_eq!(
        std::mem::offset_of!(McCampaignMinimapStateV2, entity_count),
        28
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignMinimapStateV2, entity_hash),
        40
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignMinimapEntityV2, source_order),
        24
    );
    assert_eq!(std::mem::offset_of!(McCampaignMinimapEntityV2, x), 28);
    assert_eq!(
        std::mem::offset_of!(McCampaignMinimapEntityV2, selector),
        32
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignMinimapEntityV2, help_selector),
        38
    );
    assert_eq!(std::mem::offset_of!(McCampaignMinimapMarkV2, x), 28);
    assert_eq!(
        std::mem::offset_of!(McCampaignMinimapMarkV2, palette_index),
        32
    );
    assert_eq!(size_of::<McGoatSnapshotV2>(), 84);
    assert_eq!(size_of::<McCampaignGoatSnapshotV2>(), 84);
    assert_eq!(size_of::<McCampaignSpellPickupSnapshotV2>(), 64);
    assert_eq!(size_of::<McGoatBatchV2>(), 64);
    assert_eq!(size_of::<McCampaignManaBatchV2>(), 64);
    assert_eq!(size_of::<McLevel1MissionSnapshotV2>(), 96);
    assert_eq!(size_of::<McLevel1PlayerLifecycleSnapshotV2>(), 144);
    assert_eq!(size_of::<McLevel1HudSpellSlotV2>(), 36);
    assert_eq!(size_of::<McLevel1HudSnapshotV2>(), 128);
    assert_eq!(size_of::<McLevel1SpellMenuSnapshotV2>(), 64);
    assert_eq!(size_of::<McLevel1MinimapMarksV2>(), 168);
    assert_eq!(size_of::<McLevel1MinimapEntityCommandV2>(), 24);
    assert_eq!(size_of::<McLevel1MinimapEntitiesInfoV2>(), 64);
    assert_eq!(size_of::<McLevel1PaletteInfoV2>(), 80);
    assert_eq!(size_of::<McLevel1ExitPresentationV2>(), 64);
    assert_eq!(size_of::<McLevel1CastleSnapshotV2>(), 80);
    assert_eq!(size_of::<McTerrainStateV2>(), 64);
    assert_eq!(size_of::<McLevel1CastleLifecycleSnapshotV2>(), 80);
    assert_eq!(size_of::<McLevel1HouseCompletionEffectV2>(), 80);
    assert_eq!(size_of::<McManaSphereSnapshotV2>(), 64);
    assert_eq!(size_of::<McManaCarrierSnapshotV2>(), 80);
    assert_eq!(size_of::<McLevel1ActorSnapshotV2>(), 56);
    assert_eq!(size_of::<McLevel1MultipartSegmentSnapshotV2>(), 64);
    assert_eq!(size_of::<McFireballProjectileSnapshotV2>(), 56);
    assert_eq!(size_of::<McFireballImpactSnapshotV2>(), 52);
    assert_eq!(size_of::<McSelectorFrameStateV2>(), 64);
    assert_eq!(size_of::<McSelectorFrameBatchV2>(), 88);
    assert_eq!(size_of::<McSelectorDrawFeedbackV2>(), 48);
    assert_eq!(size_of::<McGoatDamageCommandV2>(), 40);
    assert_eq!(size_of::<McAssetRefInfoV2>(), 32);
    assert_eq!(size_of::<McEventQueryV2>(), 40);
    assert_eq!(size_of::<McEventBatchV2>(), 56);
    assert_eq!(size_of::<McEventV2>(), 72);
    assert_eq!(size_of::<McLevel1ResidentObservationV1>(), 40);
    assert_eq!(
        std::mem::offset_of!(McLevel1ResidentObservationV1, digest),
        16
    );
    assert_eq!(size_of::<McAudioCommandV2>(), 88);
    assert_eq!(size_of::<McPresentationStateV2>(), 64);
    assert_eq!(size_of::<McRenderCameraStateV2>(), 72);
    assert_eq!(size_of::<McTerrainPnt5VectorV2>(), 56);
    assert_eq!(size_of::<McTerrainGridVertexV2>(), 64);
    assert_eq!(size_of::<McTerrainFrameQueryV2>(), 40);
    assert_eq!(size_of::<McTerrainFrameInfoV2>(), 96);
    assert_eq!(size_of::<McPresentationFrameQueryV2>(), 64);
    assert_eq!(size_of::<McPresentationFrameInfoV2>(), 128);
    assert_eq!(std::mem::offset_of!(McWorldInfoV2, reset_state_hash), 40);
    assert_eq!(std::mem::offset_of!(McCommandV2, requested_spell), 14);
    assert_eq!(std::mem::offset_of!(McCameraInputV2, look_yaw), 8);
    assert_eq!(std::mem::offset_of!(McCameraInputV2, reserved), 12);
    assert_eq!(std::mem::offset_of!(McSessionSnapshotV2, generation), 8);
    assert_eq!(std::mem::offset_of!(McSessionSnapshotV2, entity_index), 48);
    assert_eq!(
        std::mem::offset_of!(McSessionSnapshotV2, left_spell_holder),
        82
    );
    assert_eq!(
        std::mem::offset_of!(McSessionSnapshotV2, objective_textbox_ticks),
        77
    );
    assert_eq!(
        std::mem::offset_of!(McSessionSnapshotV2, objectives_authority),
        79
    );
    assert_eq!(std::mem::offset_of!(McSessionSnapshotV2, reserved), 104);
    assert_eq!(
        std::mem::offset_of!(McObjectiveSnapshotV2, stage_selectors),
        13
    );
    assert_eq!(std::mem::offset_of!(McObjectiveSnapshotV2, reserved), 24);
    assert_eq!(
        std::mem::offset_of!(McCampaignObjectiveStageSnapshotV2, axis_x),
        24
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignObjectiveStageSnapshotV2, relation_value),
        30
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignObjectiveStageSnapshotV2, reserved),
        32
    );
    assert_eq!(std::mem::offset_of!(McEntitySnapshotV2, reserved), 36);
    assert_eq!(std::mem::offset_of!(McCampaignSelectionV2, reserved), 16);
    assert_eq!(
        std::mem::offset_of!(McCampaignWorldInfoV2, reset_state_hash),
        40
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignTerrainStateV2, mutation_sequence),
        24
    );
    assert_eq!(std::mem::offset_of!(McCampaignTerrainStateV2, width), 40);
    assert_eq!(std::mem::offset_of!(McCampaignTerrainStateV2, reserved), 48);
    assert_eq!(std::mem::offset_of!(McCampaignPlayerSnapshotV2, turn), 16);
    assert_eq!(
        std::mem::offset_of!(McCampaignPlayerSnapshotV2, reserved),
        32
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignEntitySnapshotV2, reserved),
        36
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignShieldSnapshotV2, parent_status_flags),
        48
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignInvisibleSnapshotV2, parent_status_flags),
        52
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignBeyondSightSnapshotV2, experience_awarded),
        52
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignOpponentSnapshotV2, entity_control_flags),
        36
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignOpponentSnapshotV2, objective_relation_value),
        76
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignOpponentSnapshotV2, visibility_flags),
        80
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignDayHudSpellSlotV2, max_mana),
        20
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignDayIndexedFrameInfoV2, index_bytes),
        32
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignDayIndexedFrameInfoV2, index_hash),
        40
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignDayIndexedFrameInfoV2, package_sha256),
        72
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignDayIndexedFrameInfoV2, cache_content_sha256),
        104
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignDayIndexedFrameInfoV2, reserved),
        136
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignDayHudSpellSlotV2, draw_admitted),
        37
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignDayHudSnapshotV2, global_mana_denominator),
        52
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignDayHudSnapshotV2, player_panel_selector),
        62
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignDayHudSnapshotV2, life_palette_index),
        68
    );
    assert_eq!(std::mem::offset_of!(McCampaignDayHudSnapshotV2, nyphur), 72);
    assert_eq!(std::mem::offset_of!(McCampaignDayHudSnapshotV2, left), 176);
    assert_eq!(std::mem::offset_of!(McCampaignDayHudSnapshotV2, right), 224);
    assert_eq!(std::mem::offset_of!(McGoatSnapshotV2, life), 48);
    assert_eq!(std::mem::offset_of!(McGoatSnapshotV2, reserved), 68);
    assert_eq!(std::mem::offset_of!(McCampaignGoatSnapshotV2, life), 48);
    assert_eq!(
        std::mem::offset_of!(McCampaignGoatSnapshotV2, partner_entity_index),
        68
    );
    assert_eq!(
        std::mem::offset_of!(McCampaignSpellPickupSnapshotV2, source_entity_index),
        8
    );
    assert_eq!(std::mem::offset_of!(McCampaignSpellPickupSnapshotV2, x), 16);
    assert_eq!(std::mem::offset_of!(McGoatBatchV2, reserved), 48);
    assert_eq!(
        std::mem::offset_of!(McLevel1MissionSnapshotV2, player_life),
        40
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1PlayerLifecycleSnapshotV2, player_action),
        41
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1PlayerLifecycleSnapshotV2, action_timer),
        56
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1PlayerLifecycleSnapshotV2, player_control_flags),
        72
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1PlayerLifecycleSnapshotV2, source_pool_rebuild_count),
        108
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1PlayerLifecycleSnapshotV2, marker_dispatch_phase),
        122
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1PlayerLifecycleSnapshotV2, source_turn_counter),
        128
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1PlayerLifecycleSnapshotV2, effective_pitch),
        134
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1PlayerLifecycleSnapshotV2, marker_constructor_phase),
        136
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1MissionSnapshotV2, actor_count),
        64
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1MissionSnapshotV2, possession_experience),
        84
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1MissionSnapshotV2, last_possessed_building_entity),
        88
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1MissionSnapshotV2, possessed_building_count),
        92
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1MissionSnapshotV2, speed_trail_count),
        95
    );
    assert_eq!(std::mem::offset_of!(McLevel1HudSpellSlotV2, max_mana), 12);
    assert_eq!(
        std::mem::offset_of!(McLevel1HudSnapshotV2, global_mana_denominator),
        32
    );
    assert_eq!(std::mem::offset_of!(McLevel1HudSnapshotV2, left), 48);
    assert_eq!(std::mem::offset_of!(McLevel1HudSnapshotV2, right), 84);
    assert_eq!(
        std::mem::offset_of!(McLevel1HudSnapshotV2, selected_holder_entity_index),
        120
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1SpellMenuSnapshotV2, family_enabled),
        32
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1SpellMenuSnapshotV2, holder_entity_index),
        48
    );
    assert_eq!(std::mem::offset_of!(McLevel1MinimapMarksV2, mark_x), 28);
    assert_eq!(std::mem::offset_of!(McLevel1MinimapMarksV2, arrow_xy), 124);
    assert_eq!(std::mem::offset_of!(McLevel1MinimapMarksV2, reserved), 152);
    assert_eq!(
        std::mem::offset_of!(McLevel1MinimapEntityCommandV2, reserved),
        16
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1MinimapEntitiesInfoV2, command_hash),
        40
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1MinimapEntitiesInfoV2, reserved),
        48
    );
    assert_eq!(std::mem::offset_of!(McLevel1PaletteInfoV2, rgb8_sha256), 32);
    assert_eq!(std::mem::offset_of!(McLevel1PaletteInfoV2, reserved), 64);
    assert_eq!(
        std::mem::offset_of!(McLevel1ExitPresentationV2, player_action),
        25
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1ExitPresentationV2, camera_fov),
        36
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1ExitPresentationV2, reserved),
        48
    );
    assert_eq!(std::mem::offset_of!(McLevel1CastleSnapshotV2, life), 48);
    assert_eq!(std::mem::offset_of!(McLevel1CastleSnapshotV2, reserved), 68);
    assert_eq!(
        std::mem::offset_of!(McTerrainStateV2, mutation_sequence),
        24
    );
    assert_eq!(std::mem::offset_of!(McTerrainStateV2, width), 40);
    assert_eq!(std::mem::offset_of!(McTerrainStateV2, reserved), 48);
    assert_eq!(
        std::mem::offset_of!(McLevel1CastleLifecycleSnapshotV2, root_life),
        44
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1CastleLifecycleSnapshotV2, construction_timer),
        64
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1CastleLifecycleSnapshotV2, terrain_mutation_sequence),
        72
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1HouseCompletionEffectV2, parent_word_2e),
        44
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1HouseCompletionEffectV2, max_life),
        64
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1HouseCompletionEffectV2, effect_count),
        76
    );
    assert_eq!(std::mem::offset_of!(McManaSphereSnapshotV2, mana), 44);
    assert_eq!(std::mem::offset_of!(McManaSphereSnapshotV2, reserved), 52);
    assert_eq!(
        std::mem::offset_of!(McManaCarrierSnapshotV2, carried_mana),
        60
    );
    assert_eq!(std::mem::offset_of!(McManaCarrierSnapshotV2, reserved), 64);
    assert_eq!(
        std::mem::offset_of!(McLevel1ActorSnapshotV2, authored_index),
        8
    );
    assert_eq!(std::mem::offset_of!(McLevel1ActorSnapshotV2, max_life), 28);
    assert_eq!(std::mem::offset_of!(McLevel1ActorSnapshotV2, base_tmap), 40);
    assert_eq!(std::mem::offset_of!(McLevel1ActorSnapshotV2, yaw), 44);
    assert_eq!(
        std::mem::offset_of!(McLevel1ActorSnapshotV2, state_value),
        48
    );
    assert_eq!(std::mem::offset_of!(McLevel1ActorSnapshotV2, reserved), 52);
    assert_eq!(
        std::mem::offset_of!(McLevel1MultipartSegmentSnapshotV2, x),
        26
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1MultipartSegmentSnapshotV2, life),
        40
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1MultipartSegmentSnapshotV2, base_tmap),
        50
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1MultipartSegmentSnapshotV2, reserved),
        60
    );
    assert_eq!(std::mem::offset_of!(McFireballProjectileSnapshotV2, x), 20);
    assert_eq!(
        std::mem::offset_of!(McFireballProjectileSnapshotV2, payload),
        36
    );
    assert_eq!(
        std::mem::offset_of!(McFireballProjectileSnapshotV2, logical_height),
        40
    );
    assert_eq!(
        std::mem::offset_of!(McFireballProjectileSnapshotV2, reserved),
        48
    );
    assert_eq!(std::mem::offset_of!(McFireballImpactSnapshotV2, x), 24);
    assert_eq!(
        std::mem::offset_of!(McFireballImpactSnapshotV2, payload),
        32
    );
    assert_eq!(
        std::mem::offset_of!(McFireballImpactSnapshotV2, base_tmap),
        36
    );
    assert_eq!(
        std::mem::offset_of!(McFireballImpactSnapshotV2, reserved),
        44
    );
    assert_eq!(std::mem::offset_of!(McSelectorFrameStateV2, pixel_hash), 40);
    assert_eq!(std::mem::offset_of!(McSelectorFrameStateV2, reserved), 48);
    assert_eq!(
        std::mem::offset_of!(McSelectorFrameBatchV2, start_ordinal),
        56
    );
    assert_eq!(std::mem::offset_of!(McSelectorFrameBatchV2, reserved), 72);
    assert_eq!(std::mem::offset_of!(McSelectorDrawFeedbackV2, reserved), 32);
    assert_eq!(std::mem::offset_of!(McGoatDamageCommandV2, reserved), 24);
    assert_eq!(std::mem::offset_of!(McEventV2, state_hash), 24);
    assert_eq!(std::mem::offset_of!(McEventV2, reserved), 56);
    assert_eq!(std::mem::offset_of!(McAudioCommandV2, playback_rate_hz), 48);
    assert_eq!(std::mem::offset_of!(McAudioCommandV2, sound_id), 52);
    assert_eq!(std::mem::offset_of!(McAudioCommandV2, volume_q15), 60);
    assert_eq!(std::mem::offset_of!(McAudioCommandV2, play_type), 68);
    assert_eq!(std::mem::offset_of!(McAudioCommandV2, reserved), 72);
    assert_eq!(std::mem::offset_of!(McPresentationStateV2, source_turn), 24);
    assert_eq!(std::mem::offset_of!(McPresentationStateV2, camera_x), 28);
    assert_eq!(
        std::mem::offset_of!(McPresentationStateV2, camera_authority),
        42
    );
    assert_eq!(std::mem::offset_of!(McPresentationStateV2, reserved), 48);
    assert_eq!(std::mem::offset_of!(McRenderCameraStateV2, history_x), 28);
    assert_eq!(
        std::mem::offset_of!(McRenderCameraStateV2, slope_target_x),
        42
    );
    assert_eq!(
        std::mem::offset_of!(McRenderCameraStateV2, render_offset_x),
        46
    );
    assert_eq!(std::mem::offset_of!(McRenderCameraStateV2, reserved), 56);
    assert_eq!(std::mem::offset_of!(McTerrainPnt5VectorV2, pnt5), 36);
    assert_eq!(std::mem::offset_of!(McTerrainGridVertexV2, view_x), 16);
    assert_eq!(
        std::mem::offset_of!(McTerrainGridVertexV2, feature_flags),
        44
    );
    assert_eq!(std::mem::offset_of!(McTerrainGridVertexV2, reserved), 48);
    assert_eq!(std::mem::offset_of!(McTerrainFrameQueryV2, reserved), 24);
    assert_eq!(std::mem::offset_of!(McTerrainFrameInfoV2, width), 24);
    assert_eq!(std::mem::offset_of!(McTerrainFrameInfoV2, index_bytes), 40);
    assert_eq!(std::mem::offset_of!(McTerrainFrameInfoV2, index_hash), 64);
    assert_eq!(std::mem::offset_of!(McTerrainFrameInfoV2, reserved), 88);
    assert_eq!(
        std::mem::offset_of!(McPresentationFrameQueryV2, configured_blur_mode),
        40
    );
    assert_eq!(
        std::mem::offset_of!(McPresentationFrameInfoV2, immutable_input_hash),
        32
    );
    assert_eq!(
        std::mem::offset_of!(McPresentationFrameInfoV2, index_hash),
        64
    );
    assert_eq!(
        std::mem::offset_of!(McPresentationFrameInfoV2, reserved),
        104
    );

    let weak_marker = FireballImpact {
        id: 1,
        source_entity_index: 2,
        source_projectile_id: 3,
        owner_entity_index: 152,
        external_target_entity_index: 0,
        model: POSSESSION_IMPACT_MODEL,
        position: Position {
            x: 20_000,
            y: 19_616,
            z: 128,
        },
        payload: 0,
        life_ticks: 8,
        entity_rng: 0,
        vertical_velocity: 0,
        phase: 77,
        animation_frame: 1,
        damage_submitted: false,
        active: true,
    };
    let strong_marker = FireballImpact {
        model: POSSESSION_STRONG_IMPACT_MODEL,
        payload: 1,
        ..weak_marker
    };
    let weak_snapshot = fireball_impact_snapshot(&weak_marker);
    let strong_snapshot = fireball_impact_snapshot(&strong_marker);
    let ground_fire_rows = [0_u32, 1, 2, 3, 4, 5, 6, 7, 6, 5, 4, 3, 2, 1, 0]
        .into_iter()
        .map(|phase| {
            let snapshot = fireball_impact_snapshot(&FireballImpact {
                model: GROUND_FIRE_MODEL,
                payload: 50,
                phase,
                animation_frame: 0,
                ..weak_marker
            });
            (
                snapshot.render_parameter_index,
                snapshot.logical_height,
                snapshot.base_tmap,
                snapshot.render_mode,
                snapshot.animation_frame,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ground_fire_rows,
        [
            (228, 50, 77, 1, 0),
            (229, 150, 77, 1, 0),
            (230, 300, 77, 1, 0),
            (231, 450, 77, 1, 0),
            (232, 600, 77, 1, 0),
            (233, 700, 77, 1, 0),
            (234, 800, 77, 1, 0),
            (235, 950, 77, 1, 0),
            (234, 800, 77, 1, 0),
            (233, 700, 77, 1, 0),
            (232, 600, 77, 1, 0),
            (231, 450, 77, 1, 0),
            (230, 300, 77, 1, 0),
            (229, 150, 77, 1, 0),
            (228, 50, 77, 1, 0),
        ],
        "model 6 projects sub_31760 expansion and contraction through rows 228..235"
    );
    assert_eq!(
        (
            strong_snapshot.class_id,
            strong_snapshot.base_tmap,
            strong_snapshot.render_parameter_index,
            strong_snapshot.render_mode,
            strong_snapshot.logical_height,
        ),
        (
            weak_snapshot.class_id,
            weak_snapshot.base_tmap,
            weak_snapshot.render_parameter_index,
            weak_snapshot.render_mode,
            weak_snapshot.logical_height,
        ),
        "model 70 exports the same possession marker tuple as model 12"
    );
    assert_eq!(
        (
            strong_snapshot.model,
            strong_snapshot.payload,
            weak_snapshot.animation_frame,
            strong_snapshot.animation_frame,
        ),
        (POSSESSION_STRONG_IMPACT_MODEL, 1, 1, 1),
        "CAPI projects the bounded render frame, never class-10 dword phase"
    );
    let attraction_snapshot = fireball_impact_snapshot(&FireballImpact {
        model: POSSESSION_ATTRACTION_MODEL,
        payload: i32::try_from(
            sim_core::level1_mission::POSSESSION_ATTRACTION_TIER_TWO_RADIUS_SQUARED,
        )
        .unwrap(),
        phase: 128,
        animation_frame: 0,
        ..weak_marker
    });
    assert_eq!(
        (
            attraction_snapshot.model,
            attraction_snapshot.class_id,
            attraction_snapshot.render_parameter_index,
            attraction_snapshot.base_tmap,
            attraction_snapshot.logical_height,
            attraction_snapshot.render_mode,
            attraction_snapshot.animation_frame,
            attraction_snapshot.reserved,
        ),
        (
            POSSESSION_ATTRACTION_MODEL,
            FIREBALL_IMPACT_CLASS,
            POSSESSION_ATTRACTION_RENDER_INDEX,
            POSSESSION_ATTRACTION_BASE_TMAP,
            POSSESSION_ATTRACTION_LOGICAL_HEIGHT,
            POSSESSION_ATTRACTION_RENDER_MODE,
            0,
            [2, 0],
        ),
        "model 54 remains in gameplay inventory with its literal row-zero tuple"
    );
}

#[test]
fn campaign_economy_event_kind_is_publicly_bound() {
    let event = Level2DispatchEvent::Economy {
        pass: 2,
        source_slot: 157,
        event: Level2EconomyEvent::ManaSphereSpawned {
            sphere_id: 157,
            source_entity_index: 39,
            mana: 321,
        },
    };
    let snapshot = campaign_event_snapshot(4, 7, &event);
    assert_eq!(MC_CAMPAIGN_EVENT_ECONOMY, 10);
    assert_eq!(snapshot.kind, MC_CAMPAIGN_EVENT_ECONOMY);
    assert_eq!(snapshot.family, 1);
    assert_eq!(snapshot.source_slot, 157);
    assert_eq!(snapshot.ordinal, 7);
}

#[test]
fn campaign_mana_sphere_snapshot_exposes_attraction_mailbox_without_layout_change() {
    let snapshot = mana_sphere_snapshot(&ManaSphere {
        id: 901,
        source_entity_index: 901,
        entity_rng: 0x1234,
        position: Position::default(),
        velocity_x: 0,
        velocity_y: 0,
        vertical_velocity: 0,
        yaw: 0,
        mana: 321,
        owner_entity_index: 111,
        player_entity_index: 111,
        incoming_owner_entity_index: 0,
        incoming_possession_payload: 0,
        attraction_source_entity_index: 877,
        attraction_step: 42,
        attraction_distance_high: 3,
        assigned_carrier_id: 0,
        assigned_defender_slot: 0,
        control_flags: 0,
        phase: 7,
        active: true,
        pending_delete: false,
    });
    assert_eq!(size_of::<McManaSphereSnapshotV2>(), 64);
    assert_eq!(snapshot.reserved, [877, 42 | (3 << 16), 0]);
}

#[test]
fn campaign_economy_snapshot_binds_castle_defender_sphere_collection() {
    let snapshot = campaign_economy_event_snapshot(
        11,
        19,
        3,
        157,
        Level2EconomyEvent::CastleDefenderCollectedSphere {
            defender_slot: 157,
            sphere_id: 39,
            mana: 321,
        },
    );

    assert_eq!(
        snapshot.code,
        MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_COLLECTED_SPHERE
    );
    assert_eq!(snapshot.family, 3);
    assert_eq!(snapshot.source_slot, 157);
    assert_eq!(snapshot.primary_id, 157);
    assert_eq!(snapshot.secondary_id, 39);
    assert_eq!(snapshot.entity_0, 157);
    assert_eq!(snapshot.entity_1, 0);
    assert_eq!(snapshot.value_0, 321);
    assert_eq!(snapshot.value_1, 0);
    assert_eq!(snapshot.flags, 0);
    assert_eq!(snapshot.reserved, [0; 4]);
}

#[test]
fn campaign_economy_snapshot_binds_castle_defender_cave_sound() {
    let snapshot = campaign_economy_event_snapshot(
        11,
        20,
        3,
        157,
        Level2EconomyEvent::CastleDefenderCaveSound {
            defender_slot: 157,
            sound_id: 22,
        },
    );
    assert_eq!(
        snapshot.code,
        MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_CAVE_SOUND
    );
    assert_eq!(snapshot.family, 3);
    assert_eq!(snapshot.source_slot, 157);
    assert_eq!(snapshot.primary_id, 157);
    assert_eq!(snapshot.secondary_id, 0);
    assert_eq!(snapshot.entity_0, 22);
    assert_eq!(snapshot.value_0, 0);
    assert_eq!(snapshot.value_1, 0);
    assert_eq!(snapshot.flags, 0);
    assert_eq!(snapshot.reserved, [0; 4]);
}

#[test]
fn campaign_economy_snapshot_binds_fire_sphere_collision_sound() {
    let snapshot = campaign_economy_event_snapshot(
        11,
        21,
        3,
        200,
        Level2EconomyEvent::FireSphereCollisionSound {
            source_entity_index: 200,
            sound_id: 3,
        },
    );
    assert_eq!(
        snapshot.code,
        MC_CAMPAIGN_ECONOMY_FIRE_SPHERE_COLLISION_SOUND
    );
    assert_eq!(snapshot.family, 6);
    assert_eq!(snapshot.source_slot, 200);
    assert_eq!(snapshot.primary_id, 200);
    assert_eq!(snapshot.secondary_id, 0);
    assert_eq!(snapshot.entity_0, 3);
    assert_eq!(snapshot.entity_1, 0);
    assert_eq!(snapshot.value_0, 0);
    assert_eq!(snapshot.value_1, 0);
    assert_eq!(snapshot.flags, 0);
    assert_eq!(snapshot.reserved, [0; 4]);
}

#[test]
fn campaign_economy_snapshot_binds_authored_class5_model3_sound_without_aliasing() {
    let source_slot = 238;
    let sound_id = 22;
    let snapshot = campaign_economy_event_snapshot(
        11,
        22,
        4,
        source_slot,
        Level2EconomyEvent::AuthoredClass5Model3Sound {
            source_entity_index: source_slot,
            sound_id,
        },
    );

    assert_eq!(
        snapshot.code,
        MC_CAMPAIGN_ECONOMY_AUTHORED_CLASS5_MODEL3_SOUND
    );
    assert_ne!(
        snapshot.code,
        MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_CAVE_SOUND
    );
    assert_ne!(
        snapshot.code,
        MC_CAMPAIGN_ECONOMY_FIRE_SPHERE_COLLISION_SOUND
    );
    assert_eq!(snapshot.family, 7);
    assert_eq!(snapshot.source_slot, source_slot);
    assert_eq!(snapshot.primary_id, u32::from(source_slot));
    assert_eq!(snapshot.secondary_id, 0);
    assert_eq!(snapshot.entity_0, sound_id);
    assert_eq!(snapshot.entity_1, 0);
    assert_eq!(snapshot.value_0, 0);
    assert_eq!(snapshot.value_1, 0);
    assert_eq!(snapshot.flags, 0);
    assert_eq!(snapshot.reserved, [0; 4]);
}

#[test]
fn campaign_goat_snapshot_maps_all_typed_variants() {
    let ordinal = 19;
    let pass = 3;
    let source_slot = 157;
    let snapshots = [
        campaign_goat_event_snapshot(
            11,
            ordinal,
            0,
            pass,
            source_slot,
            GoatStepEvent::Bleat {
                entity_index: source_slot,
                sound_id: 46,
                position: Position {
                    x: 20_000,
                    y: 19_616,
                    z: 128,
                },
                action: 11,
            },
        ),
        campaign_goat_event_snapshot(
            11,
            ordinal,
            1,
            pass,
            source_slot,
            GoatStepEvent::ManaConverted {
                entity_index: source_slot,
                mana: 321,
            },
        ),
        campaign_goat_event_snapshot(
            11,
            ordinal,
            2,
            pass,
            source_slot,
            GoatStepEvent::ExtraManaSphereRequested {
                entity_index: source_slot,
            },
        ),
        campaign_goat_event_snapshot(
            11,
            ordinal,
            3,
            pass,
            source_slot,
            GoatStepEvent::RemovalQueued {
                entity_index: source_slot,
            },
        ),
        campaign_goat_event_snapshot(
            11,
            ordinal,
            4,
            pass,
            source_slot,
            GoatStepEvent::Reclaimed {
                entity_index: source_slot,
            },
        ),
        campaign_goat_event_snapshot(
            11,
            ordinal,
            5,
            pass,
            source_slot,
            GoatStepEvent::EngagementBoundaryReached {
                entity_index: source_slot,
                target_entity_index: 166,
            },
        ),
    ];

    for (event_index, snapshot) in snapshots.iter().enumerate() {
        assert_eq!(snapshot.abi_version, MC_NATIVE_ABI_VERSION);
        assert_eq!(snapshot.struct_size, struct_size::<McCampaignGoatEventV2>());
        assert_eq!(snapshot.generation, 11);
        assert_eq!(snapshot.ordinal, ordinal as u32);
        assert_eq!(snapshot.pass, pass);
        assert_eq!(snapshot.source_slot, source_slot);
        assert_eq!(snapshot.event_index, event_index as u16);
        assert_eq!(snapshot.asset_ref_index, MC_ASSET_REF_NONE);
        assert_eq!(snapshot.reserved, [0; 2]);
    }

    assert_eq!(snapshots[0].code, MC_CAMPAIGN_GOAT_BLEAT);
    assert_eq!(snapshots[0].entity_0, source_slot);
    assert_eq!(snapshots[0].sound_id, 46);
    assert_eq!(snapshots[0].action, 11);
    assert_eq!(
        (snapshots[0].x, snapshots[0].y, snapshots[0].z),
        (20_000, 19_616, 128)
    );

    assert_eq!(snapshots[1].code, MC_CAMPAIGN_GOAT_MANA_CONVERTED);
    assert_eq!(snapshots[1].entity_0, source_slot);
    assert_eq!(snapshots[1].value_0, 321);

    assert_eq!(
        snapshots[2].code,
        MC_CAMPAIGN_GOAT_EXTRA_MANA_SPHERE_REQUEST
    );
    assert_eq!(snapshots[2].entity_0, source_slot);

    assert_eq!(snapshots[3].code, MC_CAMPAIGN_GOAT_REMOVAL_QUEUED);
    assert_eq!(snapshots[3].entity_0, source_slot);

    assert_eq!(snapshots[4].code, MC_CAMPAIGN_GOAT_RECLAIMED);
    assert_eq!(snapshots[4].entity_0, source_slot);

    assert_eq!(snapshots[5].code, MC_CAMPAIGN_GOAT_ENGAGEMENT_BOUNDARY);
    assert_eq!(snapshots[5].entity_0, source_slot);
    assert_eq!(snapshots[5].entity_1, 166);
    assert!(snapshots[5].event_index > 0);
}

#[test]
fn campaign_api_selects_level_profiles_and_preserves_old_level1_values() {
    let (level1_package, level1_closure) = fixture_paths();
    let package = level1_package.to_string_lossy();
    let closure = level1_closure.to_string_lossy();
    let selection = McCampaignSelectionV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignSelectionV2>(),
        external_level: 1,
        internal_level: 0,
        map_type: 1,
        ..McCampaignSelectionV2::default()
    };
    let mut level1_handle = 0;
    assert_eq!(
        mc_native_campaign_session_open(
            package.as_ptr(),
            package.len() as u32,
            closure.as_ptr(),
            closure.len() as u32,
            &raw const selection,
            &raw mut level1_handle,
        ),
        MC_STATUS_OK
    );
    assert_ne!(level1_handle, 0);
    let mut info = McCampaignWorldInfoV2::default();
    assert_eq!(
        mc_native_campaign_world_info(
            level1_handle,
            &raw mut info,
            struct_size::<McCampaignWorldInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (info.external_level, info.internal_level, info.map_type),
        (1, 0, 1)
    );
    assert_eq!(info.player_count, 8);
    let mut players = [McCampaignPlayerSnapshotV2::default(); 8];
    let mut written = 0;
    assert_eq!(
        mc_native_campaign_players_copy(
            level1_handle,
            0,
            players.as_mut_ptr(),
            players.len() as u32,
            struct_size::<McCampaignPlayerSnapshotV2>(),
            &raw mut written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(written, 8);
    assert_eq!(players[0].active, 1);
    assert_eq!(players[1].active, 0);
    let old_handle = open_paths(&level1_package, &level1_closure).expect("Level 1 open");
    let mut old_info = McWorldInfoV2::default();
    assert_eq!(
        mc_native_world_info(
            old_handle,
            &raw mut old_info,
            struct_size::<McWorldInfoV2>()
        ),
        MC_STATUS_OK
    );
    assert_eq!(info.entity_count, old_info.entity_count);
    assert_eq!(info.active_entity_count, old_info.active_entity_count);
    assert_eq!(info.player_count, old_info.player_count);
    assert_eq!(info.reset_state_hash, old_info.reset_state_hash);
    let mut old_objectives = [McObjectiveSnapshotV2::default(); 8];
    let mut old_objective_written = 0;
    assert_eq!(
        mc_native_objectives_copy(
            old_handle,
            0,
            old_objectives.as_mut_ptr(),
            8,
            struct_size::<McObjectiveSnapshotV2>(),
            &raw mut old_objective_written,
        ),
        MC_STATUS_OK
    );
    let mut generic_objectives = [McObjectiveSnapshotV2::default(); 8];
    let mut generic_objective_written = 0;
    assert_eq!(
        mc_native_campaign_objectives_copy(
            level1_handle,
            0,
            generic_objectives.as_mut_ptr(),
            8,
            struct_size::<McObjectiveSnapshotV2>(),
            &raw mut generic_objective_written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(old_objective_written, generic_objective_written);
    assert_eq!(old_objectives, generic_objectives);
    let mut old_entities = [McEntitySnapshotV2::default(); 4];
    let mut old_entity_written = 0;
    assert_eq!(
        mc_native_entities_copy(
            old_handle,
            0,
            old_entities.as_mut_ptr(),
            4,
            struct_size::<McEntitySnapshotV2>(),
            &raw mut old_entity_written,
        ),
        MC_STATUS_OK
    );
    let mut generic_entities = [McCampaignEntitySnapshotV2::default(); 4];
    let mut generic_entity_written = 0;
    assert_eq!(
        mc_native_campaign_entities_copy(
            level1_handle,
            0,
            generic_entities.as_mut_ptr(),
            4,
            struct_size::<McCampaignEntitySnapshotV2>(),
            &raw mut generic_entity_written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(old_entity_written, generic_entity_written);
    for (old, generic) in old_entities.iter().zip(generic_entities.iter()) {
        assert_eq!(old.entity_index, generic.entity_index);
        assert_eq!(old.class_id, generic.class_id);
        assert_eq!(old.model, generic.model);
        assert_eq!(old.action, generic.action);
        assert_eq!(old.phase, generic.phase);
        assert_eq!((old.x, old.y, old.z), (generic.x, generic.y, generic.z));
    }
    let mut level1_mana_batch = McCampaignManaBatchV2::default();
    let mut level1_mana_rows = [McManaSphereSnapshotV2::default(); 1];
    assert_eq!(
        mc_native_campaign_mana_spheres_copy(
            level1_handle,
            0,
            0,
            level1_mana_rows.as_mut_ptr(),
            1,
            struct_size::<McManaSphereSnapshotV2>(),
            &raw mut level1_mana_batch,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(mc_native_session_destroy(old_handle), MC_STATUS_OK);
    assert_eq!(
        mc_native_campaign_session_destroy(level1_handle),
        MC_STATUS_OK
    );

    let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/l2-debug3.fJ3PaM/level2");
    let package = level2_package.to_string_lossy();
    let selection = McCampaignSelectionV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignSelectionV2>(),
        external_level: 2,
        internal_level: 1,
        map_type: 0,
        ..McCampaignSelectionV2::default()
    };
    let mut level2_handle = 0;
    assert_eq!(
        mc_native_campaign_session_open(
            package.as_ptr(),
            package.len() as u32,
            std::ptr::null(),
            0,
            &raw const selection,
            &raw mut level2_handle,
        ),
        MC_STATUS_OK
    );
    let mut level2_info = McCampaignWorldInfoV2::default();
    assert_eq!(
        mc_native_campaign_world_info(
            level2_handle,
            &raw mut level2_info,
            struct_size::<McCampaignWorldInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (
            level2_info.external_level,
            level2_info.internal_level,
            level2_info.map_type
        ),
        (2, 1, 0)
    );
    assert_eq!(level2_info.active_entity_count, 114);
    assert_eq!(level2_info.player_count, 8);
    let mut level2_players = [McCampaignPlayerSnapshotV2::default(); 8];
    assert_eq!(
        mc_native_campaign_players_copy(
            level2_handle,
            0,
            level2_players.as_mut_ptr(),
            8,
            struct_size::<McCampaignPlayerSnapshotV2>(),
            &raw mut written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(level2_players[0].active + level2_players[1].active, 2);
    let mut entities = [McCampaignEntitySnapshotV2::default(); 4];
    assert_eq!(
        mc_native_campaign_entities_copy(
            level2_handle,
            0,
            entities.as_mut_ptr(),
            4,
            struct_size::<McCampaignEntitySnapshotV2>(),
            &raw mut written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(written, 4);
    assert!(
        entities
            .iter()
            .any(|entity| entity.next_relation != u16::MAX)
    );
    let independent_level2_handle = open_level2_campaign_handle();
    let mut independent_level2_info = McCampaignWorldInfoV2::default();
    assert_eq!(
        mc_native_campaign_world_info(
            independent_level2_handle,
            &raw mut independent_level2_info,
            struct_size::<McCampaignWorldInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        independent_level2_info.reset_state_hash,
        level2_info.reset_state_hash
    );
    assert_eq!(
        mc_native_campaign_session_destroy(independent_level2_handle),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_campaign_session_reset(level2_handle),
        MC_STATUS_OK
    );
    let mut level2_reset_info = McCampaignWorldInfoV2::default();
    assert_eq!(
        mc_native_campaign_world_info(
            level2_handle,
            &raw mut level2_reset_info,
            struct_size::<McCampaignWorldInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        level2_reset_info.reset_state_hash,
        level2_info.reset_state_hash
    );
    assert_eq!(
        mc_native_campaign_session_destroy(level2_handle),
        MC_STATUS_OK
    );
}

#[test]
fn campaign_api_rejects_unknown_profile_and_short_or_reserved_selection_without_handle() {
    let (package, closure) = fixture_paths();
    let package = package.to_string_lossy();
    let closure = closure.to_string_lossy();
    let mut selection = McCampaignSelectionV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignSelectionV2>(),
        external_level: 9,
        internal_level: 9,
        map_type: 9,
        ..McCampaignSelectionV2::default()
    };
    let mut handle = 42;
    assert_eq!(
        mc_native_campaign_session_open(
            package.as_ptr(),
            package.len() as u32,
            closure.as_ptr(),
            closure.len() as u32,
            &raw const selection,
            &raw mut handle,
        ),
        MC_STATUS_VALIDATION_FAILED
    );
    assert_eq!(handle, 0);
    selection.external_level = 1;
    selection.internal_level = 0;
    selection.map_type = 1;
    selection.reserved_0 = 1;
    handle = 42;
    assert_eq!(
        mc_native_campaign_session_open(
            package.as_ptr(),
            package.len() as u32,
            closure.as_ptr(),
            closure.len() as u32,
            &raw const selection,
            &raw mut handle,
        ),
        MC_STATUS_RESERVED_NONZERO
    );
    assert_eq!(handle, 0);
    selection.reserved_0 = 0;
    selection.struct_size = 8;
    handle = 42;
    assert_eq!(
        mc_native_campaign_session_open(
            package.as_ptr(),
            package.len() as u32,
            closure.as_ptr(),
            closure.len() as u32,
            &raw const selection,
            &raw mut handle,
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    assert_eq!(handle, 0);
}

#[test]
fn level2_campaign_objective_stage_state_is_live_generation_bound() {
    let handle = open_level2_campaign_handle();
    let mut state = McCampaignObjectiveStageSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_objective_stage_state(
            handle,
            0,
            0,
            3,
            &raw mut state,
            struct_size::<McCampaignObjectiveStageSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(state.generation, 0);
    assert_eq!((state.objective_slot, state.stage_slot), (0, 3));
    assert_eq!((state.stage_kind, state.stage_state), (5, 1));
    assert_eq!(state.done, 0);
    assert_eq!(state.relation_domain, 0);
    assert_eq!(state.reserved_0, 0);
    assert_eq!(state.reserved, [0; 4]);

    let step_at = |handle: u64, generation: u64, x: i16, y: i16| {
        let input = McCampaignStepInputV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCampaignStepInputV2>(),
            generation,
            command: valid_command(),
            player_x: x as u16,
            player_y: y as u16,
            player_z: 7,
            ..McCampaignStepInputV2::default()
        };
        let mut snapshot = McCampaignStepSnapshotV2::default();
        let mut events = [McCampaignEventV2::default(); 1024];
        let mut event_count = 0;
        assert_eq!(
            mc_native_campaign_session_step(
                handle,
                &raw const input,
                &raw mut snapshot,
                struct_size::<McCampaignStepSnapshotV2>(),
                events.as_mut_ptr(),
                events.len() as u32,
                struct_size::<McCampaignEventV2>(),
                &raw mut event_count,
            ),
            MC_STATUS_OK
        );
    };

    step_at(handle, 0, state.axis_x, state.axis_y);
    let mut progressed = McCampaignObjectiveStageSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_objective_stage_state(
            handle,
            0,
            0,
            3,
            &raw mut progressed,
            struct_size::<McCampaignObjectiveStageSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(progressed.stage_state, 2);
    assert_eq!(progressed.done, 0);

    step_at(handle, 0, state.axis_x, state.axis_y);
    let mut completed = McCampaignObjectiveStageSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_objective_stage_state(
            handle,
            0,
            0,
            3,
            &raw mut completed,
            struct_size::<McCampaignObjectiveStageSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(completed.stage_state, 2);
    assert_eq!(completed.done, 1);

    assert_eq!(
        mc_native_campaign_objective_stage_state(
            handle,
            0,
            0,
            3,
            &raw mut completed,
            struct_size::<McCampaignObjectiveStageSnapshotV2>() - 1,
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    assert_eq!(
        mc_native_campaign_objective_stage_state(
            handle,
            0,
            8,
            3,
            &raw mut completed,
            struct_size::<McCampaignObjectiveStageSnapshotV2>(),
        ),
        MC_STATUS_BOUNDS
    );
    assert_eq!(
        mc_native_campaign_objective_stage_state(
            handle,
            0,
            0,
            8,
            &raw mut completed,
            struct_size::<McCampaignObjectiveStageSnapshotV2>(),
        ),
        MC_STATUS_BOUNDS
    );
    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    assert_eq!(
        mc_native_campaign_objective_stage_state(
            handle,
            0,
            0,
            3,
            &raw mut completed,
            struct_size::<McCampaignObjectiveStageSnapshotV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level2_campaign_terrain_planes_are_generation_bound_and_reset_exact() {
    let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/l2-debug3.fJ3PaM/level2");
    let inspection =
        NativeWorld::inspect_level2(&level2_package).expect("qualified Level 2 package");
    let expected = inspection
        .level2_reset_session()
        .expect("Level 2 reset session");
    let expected_planes = expected.canonical_terrain().planes();
    let handle = open_level2_campaign_handle();

    let mut state = McCampaignTerrainStateV2::default();
    assert_eq!(
        mc_native_campaign_terrain_state(
            handle,
            0,
            &raw mut state,
            struct_size::<McCampaignTerrainStateV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(state.generation, 0);
    assert_eq!(state.tick, 0);
    assert_eq!(state.mutation_sequence, 0);
    assert_eq!(
        (state.width, state.height, state.plane_count),
        (256, 256, 6)
    );
    assert_eq!(state.hash_schema, 1);
    assert_eq!(state.authority, MC_AUTHORITY_SESSION_TERRAIN_LIVE);
    assert_eq!(state.terrain_hash, expected.terrain_hash());

    for (kind, expected_values) in [
        (MC_TERRAIN_TYPES_U8, expected_planes.terrain_types.as_ref()),
        (
            MC_TERRAIN_HEIGHT_U8,
            expected_planes.ground_heights.as_ref(),
        ),
        (MC_TERRAIN_SHADE_U8, expected_planes.shading.as_ref()),
        (MC_TERRAIN_ANGLE_U8, expected_planes.angles.as_ref()),
        (
            MC_TERRAIN_SECONDARY_U8,
            expected_planes.secondary_heights.as_ref(),
        ),
    ] {
        let mut count = 0;
        assert_eq!(
            mc_native_campaign_terrain_plane_length(handle, 0, kind, &raw mut count,),
            MC_STATUS_OK
        );
        assert_eq!(count as usize, expected_values.len());
        let mut values = vec![0_u8; count as usize];
        let mut written = 0;
        assert_eq!(
            mc_native_campaign_terrain_plane_copy_u8(
                handle,
                0,
                kind,
                values.as_mut_ptr(),
                count,
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, count);
        assert_eq!(values, expected_values);
    }

    let mut entity_count = 0;
    assert_eq!(
        mc_native_campaign_terrain_plane_length(
            handle,
            0,
            MC_TERRAIN_ENTITY_INDEX_I16,
            &raw mut entity_count,
        ),
        MC_STATUS_OK
    );
    let mut entity_indices = vec![0_i16; entity_count as usize];
    let mut entity_written = 0;
    assert_eq!(
        mc_native_campaign_terrain_entity_indices_copy(
            handle,
            0,
            entity_indices.as_mut_ptr(),
            entity_count,
            &raw mut entity_written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(entity_written, entity_count);
    assert_eq!(entity_indices, expected_planes.entity_indices.as_ref());

    let mut stale_state = McCampaignTerrainStateV2::default();
    assert_eq!(
        mc_native_campaign_terrain_state(
            handle,
            1,
            &raw mut stale_state,
            struct_size::<McCampaignTerrainStateV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );

    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    let mut reset_state = McCampaignTerrainStateV2::default();
    assert_eq!(
        mc_native_campaign_terrain_state(
            handle,
            1,
            &raw mut reset_state,
            struct_size::<McCampaignTerrainStateV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(reset_state.generation, 1);
    assert_eq!(reset_state.terrain_hash, state.terrain_hash);
    let mut reset_types = vec![0_u8; TERRAIN_CELLS];
    let mut reset_written = 0;
    assert_eq!(
        mc_native_campaign_terrain_plane_copy_u8(
            handle,
            1,
            MC_TERRAIN_TYPES_U8,
            reset_types.as_mut_ptr(),
            reset_types.len() as u32,
            &raw mut reset_written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(reset_written as usize, TERRAIN_CELLS);
    assert_eq!(reset_types, expected_planes.terrain_types.as_ref());
    assert_eq!(
        mc_native_campaign_terrain_state(
            handle,
            0,
            &raw mut stale_state,
            struct_size::<McCampaignTerrainStateV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn campaign_level2_gameplay_cache_tamper_is_rejected() {
    let cache = qualified_level2_day_owner_cache();
    let tampered_root = std::env::temp_dir().join(format!(
        "mc-bvihb1-capi-gameplay-tamper-{}",
        std::process::id()
    ));
    if tampered_root.exists() {
        fs::remove_dir_all(&tampered_root).expect("remove stale CAPI tamper cache");
    }
    fs::create_dir_all(tampered_root.join("components/gameplay/gameplay"))
        .expect("create CAPI tamper cache");
    fs::copy(
        cache.join("components/gameplay/manifest.json"),
        tampered_root.join("components/gameplay/manifest.json"),
    )
    .expect("copy gameplay manifest");
    let gameplay_tab = tampered_root.join("components/gameplay/gameplay/DATA_BUILD0-0.TAB.bin");
    fs::copy(
        cache.join("components/gameplay/gameplay/DATA_BUILD0-0.TAB.bin"),
        &gameplay_tab,
    )
    .expect("copy gameplay TAB");
    let mut bytes = fs::read(&gameplay_tab).expect("read gameplay TAB");
    bytes[0] ^= 0x01;
    fs::write(&gameplay_tab, bytes).expect("write tampered gameplay TAB");

    let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/l2-debug3.fJ3PaM/level2");
    let package = level2_package.to_string_lossy();
    let cache_path = tampered_root.to_string_lossy();
    let selection = McCampaignSelectionV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignSelectionV2>(),
        external_level: 2,
        internal_level: 1,
        map_type: 0,
        ..McCampaignSelectionV2::default()
    };
    let mut handle = 0x1111_2222_3333_4444;
    assert_eq!(
        mc_native_campaign_session_open_with_asset_cache(
            package.as_ptr(),
            package.len() as u32,
            std::ptr::null(),
            0,
            cache_path.as_ptr(),
            cache_path.len() as u32,
            &raw const selection,
            &raw mut handle,
        ),
        MC_STATUS_VALIDATION_FAILED
    );
    assert_eq!(handle, 0);
    fs::remove_dir_all(tampered_root).expect("remove CAPI tamper cache");
}

#[test]
fn campaign_day_indexed_frame_is_generation_bound_complete_and_fail_closed() {
    const INDEX_BYTES: usize = 320 * 200;
    const PALETTE_BYTES: usize = 256 * 3;

    let cache = qualified_level2_day_owner_cache();
    let selection = McCampaignSelectionV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignSelectionV2>(),
        external_level: 2,
        internal_level: 1,
        map_type: 0,
        ..McCampaignSelectionV2::default()
    };

    let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/l2-debug3.fJ3PaM/level2");
    let package = level2_package.to_string_lossy();
    let mut null_cache_handle = 0x1111_2222_3333_4444;
    assert_eq!(
        mc_native_campaign_session_open_with_asset_cache(
            package.as_ptr(),
            package.len() as u32,
            std::ptr::null(),
            0,
            std::ptr::null(),
            1,
            &raw const selection,
            &raw mut null_cache_handle,
        ),
        MC_STATUS_NULL_POINTER
    );
    assert_eq!(null_cache_handle, 0);

    let invalid_cache_path = [0xff_u8];
    let mut invalid_utf8_handle = 0x5555_6666_7777_8888;
    assert_eq!(
        mc_native_campaign_session_open_with_asset_cache(
            package.as_ptr(),
            package.len() as u32,
            std::ptr::null(),
            0,
            invalid_cache_path.as_ptr(),
            invalid_cache_path.len() as u32,
            &raw const selection,
            &raw mut invalid_utf8_handle,
        ),
        MC_STATUS_INVALID_UTF8
    );
    assert_eq!(invalid_utf8_handle, 0);

    let handle = open_level2_campaign_handle_with_asset_cache(&cache);

    let mut pre_render_indices = vec![0xa5; INDEX_BYTES];
    let mut pre_render_palette = vec![0x5a; PALETTE_BYTES];
    let mut pre_render_index_written = 7;
    let mut pre_render_palette_written = 9;
    assert_eq!(
        mc_native_campaign_day_indexed_frame_copy(
            handle,
            0,
            0,
            pre_render_indices.as_mut_ptr(),
            INDEX_BYTES as u32,
            pre_render_palette.as_mut_ptr(),
            PALETTE_BYTES as u32,
            &raw mut pre_render_index_written,
            &raw mut pre_render_palette_written,
        ),
        MC_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(pre_render_indices, vec![0xa5; INDEX_BYTES]);
    assert_eq!(pre_render_palette, vec![0x5a; PALETTE_BYTES]);
    assert_eq!(
        (pre_render_index_written, pre_render_palette_written),
        (0, 0)
    );

    let mut skipped_initial = McCampaignDayIndexedFrameInfoV2 {
        abi_version: 0x1234_5678,
        struct_size: 152,
        generation: 0x0102_0304_0506_0708,
        tick: 0x1112_1314_1516_1718,
        package_sha256: [0xa5; 32],
        cache_content_sha256: [0x5a; 32],
        reserved: [0x2468_ace0; 4],
        ..McCampaignDayIndexedFrameInfoV2::default()
    };
    let skipped_initial_before = skipped_initial;
    assert_eq!(
        mc_native_campaign_day_indexed_frame_render(
            handle,
            0,
            1,
            &raw mut skipped_initial,
            struct_size::<McCampaignDayIndexedFrameInfoV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(skipped_initial, skipped_initial_before);

    let mut first_info = McCampaignDayIndexedFrameInfoV2::default();
    assert_eq!(
        mc_native_campaign_day_indexed_frame_render(
            handle,
            0,
            0,
            &raw mut first_info,
            struct_size::<McCampaignDayIndexedFrameInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (
            first_info.abi_version,
            first_info.struct_size,
            first_info.generation,
            first_info.tick,
            first_info.width,
            first_info.height,
            first_info.stride,
            first_info.reserved_0,
            first_info.index_bytes,
            first_info.palette_bytes,
        ),
        (
            MC_NATIVE_ABI_VERSION,
            struct_size::<McCampaignDayIndexedFrameInfoV2>(),
            0,
            0,
            320,
            200,
            320,
            0,
            INDEX_BYTES as u32,
            PALETTE_BYTES as u32,
        )
    );
    assert_ne!(first_info.index_hash, 0);
    assert_ne!(first_info.palette_hash, 0);
    assert_ne!(first_info.terrain_input_hash, 0);
    assert_ne!(first_info.terrain_command_hash, 0);
    assert_ne!(first_info.package_sha256, [0; 32]);
    assert_ne!(first_info.cache_content_sha256, [0; 32]);
    assert_eq!(first_info.reserved, [0; 4]);

    let mut first_indices = vec![0; INDEX_BYTES];
    let mut first_palette = vec![0; PALETTE_BYTES];
    let mut first_index_written = 0;
    let mut first_palette_written = 0;
    assert_eq!(
        mc_native_campaign_day_indexed_frame_copy(
            handle,
            0,
            0,
            first_indices.as_mut_ptr(),
            INDEX_BYTES as u32,
            first_palette.as_mut_ptr(),
            PALETTE_BYTES as u32,
            &raw mut first_index_written,
            &raw mut first_palette_written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (first_index_written, first_palette_written),
        (INDEX_BYTES as u32, PALETTE_BYTES as u32)
    );
    assert_eq!(byte_hash(&first_indices), first_info.index_hash);
    assert_eq!(byte_hash(&first_palette), first_info.palette_hash);
    assert!(first_indices.iter().any(|value| *value != 0));

    let mut repeated_info = McCampaignDayIndexedFrameInfoV2::default();
    assert_eq!(
        mc_native_campaign_day_indexed_frame_render(
            handle,
            0,
            0,
            &raw mut repeated_info,
            struct_size::<McCampaignDayIndexedFrameInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(repeated_info, first_info);
    let mut repeated_indices = vec![0; INDEX_BYTES];
    let mut repeated_palette = vec![0; PALETTE_BYTES];
    let mut repeated_index_written = 0;
    let mut repeated_palette_written = 0;
    assert_eq!(
        mc_native_campaign_day_indexed_frame_copy(
            handle,
            0,
            0,
            repeated_indices.as_mut_ptr(),
            INDEX_BYTES as u32,
            repeated_palette.as_mut_ptr(),
            PALETTE_BYTES as u32,
            &raw mut repeated_index_written,
            &raw mut repeated_palette_written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(repeated_indices, first_indices);
    assert_eq!(repeated_palette, first_palette);

    let stale_before = skipped_initial;
    assert_eq!(
        mc_native_campaign_day_indexed_frame_render(
            handle,
            1,
            0,
            &raw mut skipped_initial,
            struct_size::<McCampaignDayIndexedFrameInfoV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(skipped_initial, stale_before);
    assert_eq!(
        mc_native_campaign_day_indexed_frame_render(
            handle,
            0,
            1,
            &raw mut skipped_initial,
            struct_size::<McCampaignDayIndexedFrameInfoV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(skipped_initial, stale_before);

    let (first_step, _, _) = step_level2_campaign_once(handle, 0);
    assert_eq!((first_step.generation, first_step.revision), (0, 1));
    let mut second_info = McCampaignDayIndexedFrameInfoV2::default();
    assert_eq!(
        mc_native_campaign_day_indexed_frame_render(
            handle,
            0,
            1,
            &raw mut second_info,
            struct_size::<McCampaignDayIndexedFrameInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!((second_info.generation, second_info.tick), (0, 1));

    let (second_step, _, _) = step_level2_campaign_once(handle, 0);
    let (third_step, _, _) = step_level2_campaign_once(handle, 0);
    assert_eq!((second_step.generation, second_step.revision), (0, 2));
    assert_eq!((third_step.generation, third_step.revision), (0, 3));
    let skipped_after = skipped_initial;
    assert_eq!(
        mc_native_campaign_day_indexed_frame_render(
            handle,
            0,
            3,
            &raw mut skipped_initial,
            struct_size::<McCampaignDayIndexedFrameInfoV2>(),
        ),
        MC_STATUS_VALIDATION_FAILED
    );
    assert_eq!(skipped_initial, skipped_after);

    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    assert_eq!(
        mc_native_campaign_day_indexed_frame_render(
            handle,
            0,
            0,
            &raw mut skipped_initial,
            struct_size::<McCampaignDayIndexedFrameInfoV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(skipped_initial, skipped_after);

    let mut reset_pre_indices = vec![0x11; INDEX_BYTES];
    let mut reset_pre_palette = vec![0x22; PALETTE_BYTES];
    let mut reset_pre_index_written = 4;
    let mut reset_pre_palette_written = 5;
    assert_eq!(
        mc_native_campaign_day_indexed_frame_copy(
            handle,
            1,
            0,
            reset_pre_indices.as_mut_ptr(),
            INDEX_BYTES as u32,
            reset_pre_palette.as_mut_ptr(),
            PALETTE_BYTES as u32,
            &raw mut reset_pre_index_written,
            &raw mut reset_pre_palette_written,
        ),
        MC_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(reset_pre_indices, vec![0x11; INDEX_BYTES]);
    assert_eq!(reset_pre_palette, vec![0x22; PALETTE_BYTES]);
    assert_eq!((reset_pre_index_written, reset_pre_palette_written), (0, 0));

    let mut replay_info = McCampaignDayIndexedFrameInfoV2::default();
    assert_eq!(
        mc_native_campaign_day_indexed_frame_render(
            handle,
            1,
            0,
            &raw mut replay_info,
            struct_size::<McCampaignDayIndexedFrameInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(replay_info.generation, 1);
    assert_eq!(replay_info.tick, 0);
    assert_eq!(replay_info.index_hash, first_info.index_hash);
    assert_eq!(replay_info.palette_hash, first_info.palette_hash);
    let mut replay_indices = vec![0; INDEX_BYTES];
    let mut replay_palette = vec![0; PALETTE_BYTES];
    let mut replay_index_written = 0;
    let mut replay_palette_written = 0;
    assert_eq!(
        mc_native_campaign_day_indexed_frame_copy(
            handle,
            1,
            0,
            replay_indices.as_mut_ptr(),
            INDEX_BYTES as u32,
            replay_palette.as_mut_ptr(),
            PALETTE_BYTES as u32,
            &raw mut replay_index_written,
            &raw mut replay_palette_written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(replay_indices, first_indices);
    assert_eq!(replay_palette, first_palette);

    let mut short_indices = vec![0x33; INDEX_BYTES - 1];
    let mut short_palette = vec![0x44; PALETTE_BYTES];
    let mut short_index_written = 6;
    let mut short_palette_written = 7;
    assert_eq!(
        mc_native_campaign_day_indexed_frame_copy(
            handle,
            1,
            0,
            short_indices.as_mut_ptr(),
            short_indices.len() as u32,
            short_palette.as_mut_ptr(),
            PALETTE_BYTES as u32,
            &raw mut short_index_written,
            &raw mut short_palette_written,
        ),
        MC_STATUS_BUFFER_TOO_SMALL
    );
    assert_eq!(short_indices, vec![0x33; INDEX_BYTES - 1]);
    assert_eq!(short_palette, vec![0x44; PALETTE_BYTES]);
    assert_eq!((short_index_written, short_palette_written), (0, 0));

    let mut null_palette = vec![0x55; PALETTE_BYTES];
    let mut null_index_written = 8;
    let mut null_palette_written = 9;
    assert_eq!(
        mc_native_campaign_day_indexed_frame_copy(
            handle,
            1,
            0,
            std::ptr::null_mut(),
            INDEX_BYTES as u32,
            null_palette.as_mut_ptr(),
            PALETTE_BYTES as u32,
            &raw mut null_index_written,
            &raw mut null_palette_written,
        ),
        MC_STATUS_NULL_POINTER
    );
    assert_eq!(null_palette, vec![0x55; PALETTE_BYTES]);
    assert_eq!((null_index_written, null_palette_written), (0, 0));
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);

    let open_with_cache = |cache: &Path, requested_selection: &McCampaignSelectionV2| {
        let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/l2-debug3.fJ3PaM/level2");
        let package = level2_package.to_string_lossy();
        let cache = cache.to_string_lossy();
        let mut opened_handle = 99;
        let status = mc_native_campaign_session_open_with_asset_cache(
            package.as_ptr(),
            package.len() as u32,
            std::ptr::null(),
            0,
            cache.as_ptr(),
            cache.len() as u32,
            requested_selection,
            &raw mut opened_handle,
        );
        (status, opened_handle)
    };

    let missing_cache =
        std::env::temp_dir().join(format!("mc-exzm3-missing-day-cache-{}", std::process::id()));
    if missing_cache.exists() {
        fs::remove_dir_all(&missing_cache).expect("remove stale focused-test cache path");
    }
    let (missing_status, missing_handle) = open_with_cache(&missing_cache, &selection);
    assert_eq!(missing_status, MC_STATUS_VALIDATION_FAILED);
    assert_eq!(missing_handle, 0);

    let unknown_selection = McCampaignSelectionV2 {
        external_level: 99,
        internal_level: 99,
        map_type: 99,
        ..selection
    };
    let (unknown_status, unknown_handle) = open_with_cache(&cache, &unknown_selection);
    assert_eq!(unknown_status, MC_STATUS_VALIDATION_FAILED);
    assert_eq!(unknown_handle, 0);

    let tampered_cache = std::env::temp_dir().join(format!(
        "mc-exzm3-tampered-day-cache-{}",
        std::process::id()
    ));
    if tampered_cache.exists() {
        fs::remove_dir_all(&tampered_cache).expect("remove stale tampered cache path");
    }
    copy_tree(&cache.join("components/material"), &tampered_cache);
    let tampered_sky_path = tampered_cache.join("sky/skyd.indices.u8");
    let mut tampered_sky = fs::read(&tampered_sky_path).expect("read copied Day sky");
    tampered_sky[0] ^= 1;
    fs::write(&tampered_sky_path, tampered_sky).expect("write tampered Day sky");
    let (tampered_status, tampered_handle) = open_with_cache(&tampered_cache, &selection);
    assert_eq!(tampered_status, MC_STATUS_VALIDATION_FAILED);
    assert_eq!(tampered_handle, 0);
    fs::remove_dir_all(&tampered_cache).expect("remove tampered focused-test cache");

    let (level1_package, level1_closure) = fixture_paths();
    let level1_package = level1_package.to_string_lossy();
    let level1_closure = level1_closure.to_string_lossy();
    let level1_selection = McCampaignSelectionV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignSelectionV2>(),
        external_level: 1,
        internal_level: 0,
        map_type: 1,
        ..McCampaignSelectionV2::default()
    };
    let mut level1_handle = 0;
    assert_eq!(
        mc_native_campaign_session_open(
            level1_package.as_ptr(),
            level1_package.len() as u32,
            level1_closure.as_ptr(),
            level1_closure.len() as u32,
            &raw const level1_selection,
            &raw mut level1_handle,
        ),
        MC_STATUS_OK
    );
    let non_level2_before = skipped_initial;
    assert_eq!(
        mc_native_campaign_day_indexed_frame_render(
            level1_handle,
            0,
            0,
            &raw mut skipped_initial,
            struct_size::<McCampaignDayIndexedFrameInfoV2>(),
        ),
        MC_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(skipped_initial, non_level2_before);
    assert_eq!(
        mc_native_campaign_session_destroy(level1_handle),
        MC_STATUS_OK
    );
}

fn open_cf12_descriptor_campaign() -> u64 {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-cf02-campaign-package-v1-a/qualified-level2");
    let cache = qualified_level2_day_owner_cache();
    let package = package.to_string_lossy();
    let cache = cache.to_string_lossy();
    let selection = McCampaignSelectionV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignSelectionV2>(),
        external_level: 2,
        internal_level: 1,
        map_type: 0,
        ..McCampaignSelectionV2::default()
    };
    let mut handle = 0;
    assert_eq!(
        mc_native_campaign_session_open_with_asset_cache(
            package.as_ptr(),
            package.len() as u32,
            std::ptr::null(),
            0,
            cache.as_ptr(),
            cache.len() as u32,
            &raw const selection,
            &raw mut handle,
        ),
        MC_STATUS_OK,
        "{}",
        LAST_ERROR.with(|value| value.borrow().clone())
    );
    assert_ne!(handle, 0);
    handle
}

fn cf12_frame_query(generation: u64, tick: u64) -> McPresentationFrameQueryV2 {
    McPresentationFrameQueryV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McPresentationFrameQueryV2>(),
        generation,
        tick,
        presentation_revision: tick,
        viewport_width: 320,
        viewport_height: 200,
        screen_stride: 320,
        vga_resolution: 1,
        ..McPresentationFrameQueryV2::default()
    }
}

fn cf12_world_info(handle: u64) -> McCampaignWorldInfoV2 {
    let mut info = McCampaignWorldInfoV2::default();
    assert_eq!(
        mc_native_campaign_world_info(
            handle,
            &raw mut info,
            struct_size::<McCampaignWorldInfoV2>(),
        ),
        MC_STATUS_OK
    );
    info
}

#[test]
fn cf12_generic_campaign_lifecycle_journal_and_frame_route() {
    const INDEX_BYTES: usize = 320 * 200;
    const EFFECT_CAPACITY: usize = 16_384;

    let handle = open_cf12_descriptor_campaign();
    let initial_world = cf12_world_info(handle);
    assert_eq!(
        (
            initial_world.external_level,
            initial_world.internal_level,
            initial_world.map_type,
        ),
        (2, 1, 0)
    );

    let initial_effect_query = McEventQueryV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McEventQueryV2>(),
        generation: 0,
        cursor: 0,
        reserved: [0; 4],
    };
    let mut initial_batch = McEventBatchV2::default();
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            &raw const initial_effect_query,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut initial_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (
            initial_batch.generation,
            initial_batch.next_cursor,
            initial_batch.event_head,
            initial_batch.written,
            initial_batch.has_more,
        ),
        (0, 0, 0, 0, 0)
    );

    let initial_frame_query = cf12_frame_query(0, 0);
    let mut initial_indices = vec![0; INDEX_BYTES];
    let mut initial_frame = McPresentationFrameInfoV2::default();
    assert_eq!(
        mc_native_campaign_presentation_frame_copy(
            handle,
            &raw const initial_frame_query,
            initial_indices.as_mut_ptr(),
            initial_indices.len() as u32,
            std::ptr::null_mut(),
            0,
            &raw mut initial_frame,
            struct_size::<McPresentationFrameInfoV2>(),
        ),
        MC_STATUS_OK,
        "{}",
        LAST_ERROR.with(|value| value.borrow().clone())
    );
    assert_eq!(
        (
            initial_frame.generation,
            initial_frame.tick,
            initial_frame.presentation_revision,
            initial_frame.width,
            initial_frame.height,
            initial_frame.stride,
            initial_frame.index_bytes,
            initial_frame.provenance_bytes,
            initial_frame.newly_committed,
        ),
        (0, 0, 0, 320, 200, 320, INDEX_BYTES as u32, 0, 1)
    );
    assert_ne!(initial_frame.index_hash, 0);
    assert_eq!(byte_hash(&initial_indices), initial_frame.index_hash);
    assert_eq!(
        initial_frame.flags,
        MC_TERRAIN_FRAME_ISOLATED_SKY_TERRAIN
            | MC_TERRAIN_FRAME_SKY_SEEDED
            | MC_TERRAIN_FRAME_NON_TERRAIN_OMITTED
    );
    assert_eq!(
        initial_frame.flags
            & (MC_PRESENTATION_FRAME_COMPOSITOR_COMPLETE
                | MC_PRESENTATION_FRAME_HAS_PROVENANCE
                | MC_PRESENTATION_FRAME_OVERLAYS_COMPOSITED
                | MC_PRESENTATION_FRAME_POSTPROCESS_COMPLETE
                | MC_PRESENTATION_FRAME_LIGHTING_COMPLETE),
        0,
        "the generic bridge must not overclaim the retained Day terrain frame"
    );

    let mut repeated_indices = vec![0; INDEX_BYTES];
    let mut repeated_frame = McPresentationFrameInfoV2::default();
    assert_eq!(
        mc_native_campaign_presentation_frame_copy(
            handle,
            &raw const initial_frame_query,
            repeated_indices.as_mut_ptr(),
            repeated_indices.len() as u32,
            std::ptr::null_mut(),
            0,
            &raw mut repeated_frame,
            struct_size::<McPresentationFrameInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(repeated_indices, initial_indices);
    assert_eq!(repeated_frame.index_hash, initial_frame.index_hash);
    assert_eq!(repeated_frame.newly_committed, 0);

    let (step, legacy_events, legacy_event_count) = step_level2_campaign_once(handle, 0);
    assert_eq!((step.generation, step.revision), (0, 1));
    assert!(legacy_event_count > 0);
    assert_eq!(legacy_events[0].generation, 0);

    let mut effects = vec![McEventV2::default(); EFFECT_CAPACITY];
    let mut effect_batch = McEventBatchV2::default();
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            &raw const initial_effect_query,
            effects.as_mut_ptr(),
            effects.len() as u32,
            struct_size::<McEventV2>(),
            &raw mut effect_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_OK
    );
    let effects = &effects[..effect_batch.written as usize];
    assert!(!effects.is_empty());
    assert_eq!(effect_batch.next_cursor, effect_batch.event_head);
    assert_eq!(effect_batch.has_more, 0);
    for (index, effect) in effects.iter().enumerate() {
        assert_eq!(effect.generation, 0);
        assert_eq!(effect.sequence, index as u64 + 1);
        assert_eq!(effect.state_hash, step.state_hash);
        assert!([
            campaign_facade::MC_CAMPAIGN_EFFECT_DISPATCH,
            campaign_facade::MC_CAMPAIGN_EFFECT_SHARED_SPELL,
            campaign_facade::MC_CAMPAIGN_EFFECT_FLIGHT,
            campaign_facade::MC_CAMPAIGN_EFFECT_SCENE_CHANGE,
        ]
        .contains(&effect.kind));
    }

    let stepped_frame_query = cf12_frame_query(0, 1);
    let mut stepped_indices = vec![0; INDEX_BYTES];
    let mut stepped_frame = McPresentationFrameInfoV2::default();
    assert_eq!(
        mc_native_campaign_presentation_frame_copy(
            handle,
            &raw const stepped_frame_query,
            stepped_indices.as_mut_ptr(),
            stepped_indices.len() as u32,
            std::ptr::null_mut(),
            0,
            &raw mut stepped_frame,
            struct_size::<McPresentationFrameInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!((stepped_frame.tick, stepped_frame.newly_committed), (1, 1));

    let mut entity_count = 0;
    assert_eq!(
        mc_native_campaign_entity_count(handle, &raw mut entity_count),
        MC_STATUS_OK,
        "the retained Level 2 adapter must observe the same descriptor handle"
    );
    assert_eq!(entity_count, initial_world.entity_count);

    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    let mut stale_batch = McEventBatchV2::default();
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            &raw const initial_effect_query,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut stale_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    let reset_effect_query = McEventQueryV2 {
        generation: 1,
        ..initial_effect_query
    };
    let mut reset_batch = McEventBatchV2::default();
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            &raw const reset_effect_query,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut reset_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!((reset_batch.generation, reset_batch.written), (1, 0));

    let reset_frame_query = cf12_frame_query(1, 0);
    let mut reset_indices = vec![0; INDEX_BYTES];
    let mut reset_frame = McPresentationFrameInfoV2::default();
    assert_eq!(
        mc_native_campaign_presentation_frame_copy(
            handle,
            &raw const reset_frame_query,
            reset_indices.as_mut_ptr(),
            reset_indices.len() as u32,
            std::ptr::null_mut(),
            0,
            &raw mut reset_frame,
            struct_size::<McPresentationFrameInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!((reset_frame.generation, reset_frame.tick), (1, 0));
    assert_eq!(reset_frame.index_hash, initial_frame.index_hash);
    assert_eq!(reset_indices, initial_indices);
    assert_eq!(cf12_world_info(handle).reset_state_hash, initial_world.reset_state_hash);
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn cf12_generic_campaign_abi_failures_are_atomic() {
    const INDEX_BYTES: usize = 320 * 200;
    const EFFECT_CAPACITY: usize = 16_384;

    let expired = campaign_abi::campaign_effect_page_error(EffectPageError::ExpiredCursor {
        first_available: 7,
        requested: 6,
    });
    assert_eq!(expired.status, MC_STATUS_CURSOR_EXPIRED);

    let handle = open_cf12_descriptor_campaign();
    let initial_world = cf12_world_info(handle);
    let step_input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command: valid_command(),
        player_slot: 0,
        player_x: 123,
        player_y: 456,
        player_z: 7,
        ..McCampaignStepInputV2::default()
    };
    let mut failed_snapshot = McCampaignStepSnapshotV2 {
        generation: u64::MAX,
        state_hash: u64::MAX,
        ..McCampaignStepSnapshotV2::default()
    };
    let failed_snapshot_before = failed_snapshot;
    let mut failed_event_count = 99;
    assert_eq!(
        mc_native_campaign_session_step(
            handle,
            &raw const step_input,
            &raw mut failed_snapshot,
            struct_size::<McCampaignStepSnapshotV2>(),
            std::ptr::null_mut(),
            0,
            struct_size::<McCampaignEventV2>(),
            &raw mut failed_event_count,
        ),
        MC_STATUS_BUFFER_TOO_SMALL
    );
    assert_eq!(failed_snapshot, failed_snapshot_before);
    assert_eq!(failed_event_count, 0);
    assert_eq!(cf12_world_info(handle), initial_world);

    let empty_query = McEventQueryV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McEventQueryV2>(),
        generation: 0,
        cursor: 0,
        reserved: [0; 4],
    };
    let mut empty_batch = McEventBatchV2::default();
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            &raw const empty_query,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut empty_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(empty_batch.written, 0, "failed step must not append effects");

    let (step, _, _) = step_level2_campaign_once(handle, 0);
    let valid_query = empty_query;
    let mut sentinel_batch = McEventBatchV2 {
        generation: u64::MAX,
        next_cursor: u64::MAX,
        event_head: u64::MAX,
        written: u32::MAX,
        ..McEventBatchV2::default()
    };
    let sentinel_before = sentinel_batch;
    let mut bad_version = valid_query;
    bad_version.abi_version += 1;
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            &raw const bad_version,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut sentinel_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_UNSUPPORTED_VERSION
    );
    assert_eq!(sentinel_batch, sentinel_before);
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            std::ptr::null(),
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut sentinel_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_NULL_POINTER
    );
    assert_eq!(sentinel_batch, sentinel_before);
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            &raw const valid_query,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>() - 1,
            &raw mut sentinel_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    assert_eq!(sentinel_batch, sentinel_before);
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            &raw const valid_query,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut sentinel_batch,
            struct_size::<McEventBatchV2>() - 1,
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    assert_eq!(sentinel_batch, sentinel_before);
    let mut short_query = valid_query;
    short_query.struct_size -= 1;
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            &raw const short_query,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut sentinel_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    assert_eq!(sentinel_batch, sentinel_before);
    let mut reserved_query = valid_query;
    reserved_query.reserved[2] = 1;
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            &raw const reserved_query,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut sentinel_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_RESERVED_NONZERO
    );
    assert_eq!(sentinel_batch, sentinel_before);
    let future_query = McEventQueryV2 {
        cursor: u64::MAX,
        ..valid_query
    };
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            &raw const future_query,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut sentinel_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_BOUNDS
    );
    assert_eq!(sentinel_batch, sentinel_before);
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            &raw const valid_query,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut sentinel_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_BUFFER_TOO_SMALL
    );
    assert_eq!(sentinel_batch, sentinel_before);
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            &raw const valid_query,
            std::ptr::null_mut(),
            EFFECT_CAPACITY as u32,
            struct_size::<McEventV2>(),
            &raw mut sentinel_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_NULL_POINTER
    );
    assert_eq!(sentinel_batch, sentinel_before);
    assert_eq!(
        mc_native_campaign_events_copy_since(
            u64::MAX,
            &raw const valid_query,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut sentinel_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_UNKNOWN_HANDLE
    );
    assert_eq!(sentinel_batch, sentinel_before);

    let mut query_storage = vec![0_u8; size_of::<McEventQueryV2>() + 1];
    let unaligned_query = unsafe { query_storage.as_mut_ptr().add(1).cast::<McEventQueryV2>() };
    unsafe { unaligned_query.write_unaligned(valid_query) };
    let mut event_storage = vec![0_u8; size_of::<McEventV2>() * EFFECT_CAPACITY + 1];
    let unaligned_events = unsafe { event_storage.as_mut_ptr().add(1).cast::<McEventV2>() };
    let mut batch_storage = vec![0_u8; size_of::<McEventBatchV2>() + 1];
    let unaligned_batch = unsafe { batch_storage.as_mut_ptr().add(1).cast::<McEventBatchV2>() };
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            unaligned_query,
            unaligned_events,
            EFFECT_CAPACITY as u32,
            struct_size::<McEventV2>(),
            unaligned_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_OK
    );
    let unaligned_batch = unsafe { unaligned_batch.read_unaligned() };
    assert!(unaligned_batch.written > 0);
    let first_unaligned_event = unsafe { unaligned_events.read_unaligned() };
    assert_eq!((first_unaligned_event.sequence, first_unaligned_event.state_hash), (1, step.state_hash));

    let frame_query = cf12_frame_query(0, 1);
    let mut short_indices = vec![0xa5; INDEX_BYTES - 1];
    let mut frame_info = McPresentationFrameInfoV2 {
        generation: u64::MAX,
        index_hash: u64::MAX,
        ..McPresentationFrameInfoV2::default()
    };
    let frame_info_before = frame_info;
    let mut versioned_frame_query = frame_query;
    versioned_frame_query.abi_version += 1;
    assert_eq!(
        mc_native_campaign_presentation_frame_copy(
            handle,
            &raw const versioned_frame_query,
            std::ptr::null_mut(),
            INDEX_BYTES as u32,
            std::ptr::null_mut(),
            0,
            &raw mut frame_info,
            struct_size::<McPresentationFrameInfoV2>(),
        ),
        MC_STATUS_UNSUPPORTED_VERSION
    );
    assert_eq!(frame_info, frame_info_before);
    let mut reserved_frame_query = frame_query;
    reserved_frame_query.reserved[0] = 1;
    assert_eq!(
        mc_native_campaign_presentation_frame_copy(
            handle,
            &raw const reserved_frame_query,
            std::ptr::null_mut(),
            INDEX_BYTES as u32,
            std::ptr::null_mut(),
            0,
            &raw mut frame_info,
            struct_size::<McPresentationFrameInfoV2>(),
        ),
        MC_STATUS_RESERVED_NONZERO
    );
    assert_eq!(frame_info, frame_info_before);
    assert_eq!(
        mc_native_campaign_presentation_frame_copy(
            handle,
            &raw const frame_query,
            std::ptr::null_mut(),
            INDEX_BYTES as u32,
            std::ptr::null_mut(),
            0,
            &raw mut frame_info,
            struct_size::<McPresentationFrameInfoV2>(),
        ),
        MC_STATUS_NULL_POINTER
    );
    assert_eq!(frame_info, frame_info_before);
    assert_eq!(
        mc_native_campaign_presentation_frame_copy(
            handle,
            &raw const frame_query,
            short_indices.as_mut_ptr(),
            short_indices.len() as u32,
            std::ptr::null_mut(),
            0,
            &raw mut frame_info,
            struct_size::<McPresentationFrameInfoV2>() - 1,
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    assert_eq!(frame_info, frame_info_before);
    assert_eq!(
        mc_native_campaign_presentation_frame_copy(
            handle,
            &raw const frame_query,
            short_indices.as_mut_ptr(),
            short_indices.len() as u32,
            std::ptr::null_mut(),
            0,
            &raw mut frame_info,
            struct_size::<McPresentationFrameInfoV2>(),
        ),
        MC_STATUS_BUFFER_TOO_SMALL
    );
    assert_eq!(short_indices, vec![0xa5; INDEX_BYTES - 1]);
    assert_eq!(frame_info, frame_info_before);
    let mut bad_frame_query = frame_query;
    bad_frame_query.menu_overlay = 1;
    let mut good_indices = vec![0x5a; INDEX_BYTES];
    assert_eq!(
        mc_native_campaign_presentation_frame_copy(
            handle,
            &raw const bad_frame_query,
            good_indices.as_mut_ptr(),
            good_indices.len() as u32,
            std::ptr::null_mut(),
            0,
            &raw mut frame_info,
            struct_size::<McPresentationFrameInfoV2>(),
        ),
        MC_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(good_indices, vec![0x5a; INDEX_BYTES]);
    assert_eq!(frame_info, frame_info_before);
    assert_eq!(
        mc_native_campaign_presentation_frame_copy(
            handle,
            &raw const frame_query,
            good_indices.as_mut_ptr(),
            good_indices.len() as u32,
            std::ptr::null_mut(),
            0,
            &raw mut frame_info,
            struct_size::<McPresentationFrameInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(frame_info.newly_committed, 1, "failed copies must not commit the frame");
    assert_eq!(byte_hash(&good_indices), frame_info.index_hash);

    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    let stale_frame_before = frame_info;
    assert_eq!(
        mc_native_campaign_presentation_frame_copy(
            handle,
            &raw const frame_query,
            good_indices.as_mut_ptr(),
            good_indices.len() as u32,
            std::ptr::null_mut(),
            0,
            &raw mut frame_info,
            struct_size::<McPresentationFrameInfoV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(frame_info, stale_frame_before);
    let stale_event_before = sentinel_batch;
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            &raw const valid_query,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut sentinel_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(sentinel_batch, stale_event_before);
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);

    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-cf02-campaign-package-v1-a/qualified-level2");
    let cache = qualified_level2_day_owner_cache();
    let package = package.to_string_lossy();
    let cache = cache.to_string_lossy();
    let mismatched_selection = McCampaignSelectionV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignSelectionV2>(),
        external_level: 1,
        internal_level: 0,
        map_type: 1,
        ..McCampaignSelectionV2::default()
    };
    let mut mismatched_handle = u64::MAX;
    assert_eq!(
        mc_native_campaign_session_open_with_asset_cache(
            package.as_ptr(),
            package.len() as u32,
            std::ptr::null(),
            0,
            cache.as_ptr(),
            cache.len() as u32,
            &raw const mismatched_selection,
            &raw mut mismatched_handle,
        ),
        MC_STATUS_VALIDATION_FAILED
    );
    assert_eq!(mismatched_handle, 0);
}
