#[test]
fn level1_scroll_entity_projection_uses_live_class14_carrier() {
    // Remote compilation embeds the worker's manifest path. Owner-data checks
    // run locally, so accept the same explicit Level 1 package as the client.
    let package = std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/settlement-20261105-integration/owner-closed-v6/level1")
        },
        PathBuf::from,
    );
    let package = package.to_string_lossy();
    let selection = McCampaignSelectionV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignSelectionV2>(),
        external_level: 1,
        internal_level: 0,
        map_type: 1,
        ..Default::default()
    };
    let mut handle = 0;
    assert_eq!(
        mc_native_campaign_session_open(
            package.as_ptr(),
            package.len() as u32,
            std::ptr::null(),
            0,
            &raw const selection,
            &raw mut handle,
        ),
        MC_STATUS_OK
    );
    // Exercise only the read-only ABI projection here. The ordinary group-4
    // producer and scroll collection are verified by the public gameplay run.
    let scroll = with_campaign_session_mut(handle, |record| {
        let CampaignSessionKind::Campaign { reset, .. } = &mut record.kind else {
            unreachable!()
        };
        Ok(reset
            .allocate_level2_authored_class14(21)
            .expect("Level 1 source scroll constructor must allocate"))
    })
    .unwrap();
    let mut row = McCampaignEntitySnapshotV2::default();
    let mut written = 0;
    assert_eq!(
        mc_native_campaign_entities_copy(
            handle,
            u32::from(scroll.source_entity_index),
            &raw mut row,
            1,
            struct_size::<McCampaignEntitySnapshotV2>(),
            &raw mut written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(written, 1);
    assert_eq!(
        (
            row.authority,
            row.active,
            row.class_id,
            row.model,
            row.action
        ),
        (MC_AUTHORITY_CAMPAIGN_LIVE, 1, 14, 5, 10)
    );
    assert_eq!(
        (row.entity_index, row.source_id, row.x, row.y, row.z),
        (
            scroll.source_entity_index,
            scroll.source_entity_index,
            scroll.position.x,
            scroll.position.y,
            scroll.position.z
        )
    );
    assert!(with_campaign_session_mut(handle, |record| {
        let CampaignSessionKind::Campaign { reset, .. } = &mut record.kind else {
            unreachable!()
        };
        Ok(reset
            .economy_mut()
            .deactivate_authored_class14_slot(scroll.source_entity_index))
    })
    .unwrap());
    assert_eq!(
        mc_native_campaign_entities_copy(
            handle,
            u32::from(scroll.source_entity_index),
            &raw mut row,
            1,
            struct_size::<McCampaignEntitySnapshotV2>(),
            &raw mut written,
        ),
        MC_STATUS_OK
    );
    assert_eq!((row.class_id, row.model, row.active), (14, 5, 0));
    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    assert_eq!(
        mc_native_campaign_entities_copy(
            handle,
            u32::from(scroll.source_entity_index),
            &raw mut row,
            1,
            struct_size::<McCampaignEntitySnapshotV2>(),
            &raw mut written,
        ),
        MC_STATUS_OK
    );
    assert_ne!(row.authority, MC_AUTHORITY_CAMPAIGN_LIVE);
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn campaign_entity_projection_follows_live_player_and_reset() {
    let handle = open_level2_campaign_handle();
    let live_player = || {
        with_campaign_session(handle, |record| {
            let CampaignSessionKind::Campaign { reset, .. } = &record.kind else {
                unreachable!()
            };
            let slot = reset.players()[0].entity_index;
            let row = &reset.entities()[usize::from(slot)];
            Ok((slot, (row.position.x, row.position.y, row.position.z)))
        })
        .unwrap()
    };
    let copy_player = |slot: u16| {
        let mut row = McCampaignEntitySnapshotV2::default();
        let mut written = 0;
        assert_eq!(
            mc_native_campaign_entities_copy(
                handle,
                u32::from(slot),
                &raw mut row,
                1,
                struct_size::<McCampaignEntitySnapshotV2>(),
                &raw mut written
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 1);
        (row.x, row.y, row.z)
    };
    let (slot, initial) = live_player();
    assert_eq!(copy_player(slot), initial);
    step_level2_campaign_once(handle, 0);
    let (_, moved) = live_player();
    assert_ne!(
        moved, initial,
        "the public native turn moved the authoritative player row"
    );
    assert_eq!(
        copy_player(slot),
        moved,
        "presentation must not copy the immutable package row"
    );
    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    assert_eq!(live_player().1, initial);
    assert_eq!(copy_player(slot), initial);
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn campaign_native_projection_copies_live_sphere_and_castle_balloon_and_clears_on_reset() {
    let handle = open_level2_campaign_handle();
    // Isolate the native projection using the production death-sphere and
    // Castle constructors. The core Possession test proves the upstream kill,
    // claim, upkeep, collection and deposit lifecycle through ordinary turns.
    let (sphere_slot, player_slot, camera) = with_campaign_session_mut(handle, |record| {
        let CampaignSessionKind::Campaign { reset, .. } = &mut record.kind else {
            unreachable!()
        };
        let player = reset.players()[0].entity_index;
        let mut sphere_position = reset.state.position;
        sphere_position.y = sphere_position.y.wrapping_sub(2_048);
        let terrain = reset.canonical_terrain().clone();
        let sphere_slot = reset
            .economy_mut()
            .spawn_mana_sphere_from_death(player, sphere_position, 0, 17, 500, player, &terrain)
            .expect("production sphere allocation") as u16;
        let mut castle_position = sphere_position;
        castle_position.x = castle_position.x.wrapping_add(2_304);
        reset
            .economy_mut()
            .build_castle(castle_position, player, &terrain)
            .expect("production stage-one Castle allocation");
        Ok((sphere_slot, player, reset.state.position))
    })
    .unwrap();
    let (step, _) = step_level2_campaign_public(
        handle,
        McCampaignStepInputV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCampaignStepInputV2>(),
            generation: 0,
            command: valid_command(),
            player_slot: 0,
            player_x: camera.x,
            player_y: camera.y,
            player_z: camera.z,
            ..McCampaignStepInputV2::default()
        },
    );
    let (sphere, castle, balloon, before_hash) = with_campaign_session(handle, |record| {
        let CampaignSessionKind::Campaign { reset, .. } = &record.kind else {
            unreachable!()
        };
        let sphere = *reset
            .economy()
            .spheres()
            .iter()
            .find(|row| row.source_entity_index == sphere_slot)
            .unwrap();
        let balloon = *reset
            .economy()
            .castle_children()
            .iter()
            .find(|row| row.active && (row.source_class, row.model, row.action) == (3, 3, 9))
            .expect("ordinary upkeep creates the live balloon");
        Ok((
            sphere,
            reset.economy().castle().unwrap(),
            balloon,
            reset.state_hash(),
        ))
    })
    .unwrap();
    let entities = campaign_entities_public(handle);
    let castle_row = entities[usize::from(castle.source_entity_index)];
    assert_eq!(
        (
            castle_row.authority,
            castle_row.active,
            castle_row.class_id,
            castle_row.model,
            castle_row.action
        ),
        (MC_AUTHORITY_CAMPAIGN_LIVE, 1, 3, 2, 4)
    );
    assert_eq!(
        (
            castle_row.source_id,
            castle_row.x,
            castle_row.y,
            castle_row.z
        ),
        (
            player_slot,
            castle.position.x,
            castle.position.y,
            castle.position.z
        )
    );
    let sphere_row = entities[usize::from(sphere_slot)];
    assert_eq!(
        (
            sphere_row.authority,
            sphere_row.active,
            sphere_row.class_id,
            sphere_row.model,
            sphere_row.action
        ),
        (MC_AUTHORITY_CAMPAIGN_LIVE, 1, 10, 39, 41)
    );
    assert_eq!(
        (
            sphere_row.source_id,
            sphere_row.x,
            sphere_row.y,
            sphere_row.z,
            sphere_row.yaw
        ),
        (
            sphere_slot,
            sphere.position.x,
            sphere.position.y,
            sphere.position.z,
            sphere.yaw
        )
    );
    let balloon_row = entities[usize::from(balloon.source_entity_index)];
    // The same real upkeep product must be observable with its own current
    // incarnation, owner colour, mana and Castle relation. The generic entity
    // projection lacks these fields and cannot safely tint the 3D canopy.
    {
        use super::castle_child_abi::{
            McCampaignCastleChildV1, mc_native_campaign_castle_children_copy_v1,
        };
        let mut children = [McCampaignCastleChildV1::default(); 16];
        let mut count = u32::MAX;
        assert_eq!(
            mc_native_campaign_castle_children_copy_v1(
                handle,
                0,
                step.revision,
                children.as_mut_ptr(),
                children.len() as u32,
                struct_size::<McCampaignCastleChildV1>(),
                &raw mut count,
            ),
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        let child = children[..count as usize]
            .iter()
            .find(|row| row.entity_index == balloon.source_entity_index)
            .expect("the actual Castle balloon is in the typed observation");
        let owner_color = with_campaign_session(handle, |record| {
            let CampaignSessionKind::Campaign { reset, .. } = &record.kind else {
                unreachable!()
            };
            Ok(reset.shared_spells().local_player().player_color_index())
        })
        .unwrap();
        assert_eq!((child.class_id, child.model, child.action), (3, 3, 9));
        assert_eq!(
            (child.source_id, child.slot_generation, child.x, child.y, child.z),
            (
                balloon.id_entity_index,
                with_campaign_session(handle, |record| {
                    let CampaignSessionKind::Campaign { reset, .. } = &record.kind else {
                        unreachable!()
                    };
                    Ok(reset.economy().slot_generation(balloon.source_entity_index))
                })
                .unwrap(),
                balloon.position.x,
                balloon.position.y,
                balloon.position.z,
            )
        );
        assert_eq!((child.life, child.max_life, child.mana, child.max_mana),
            (balloon.life, balloon.max_life, balloon.mana, balloon.max_mana));
        assert_eq!(child.castle_index, castle.source_entity_index);
        assert_eq!(child.owner_player_color, owner_color);
        assert_eq!(child.relation_flags & 3, 3);
        assert_eq!((child.active, child.drawing, child.render_parameter), (1, 1, 169));
        assert!(child.render_height > 0);
        assert_eq!(
            mc_native_campaign_castle_children_copy_v1(
                handle, 0, step.revision + 1, children.as_mut_ptr(),
                children.len() as u32, struct_size::<McCampaignCastleChildV1>(), &raw mut count,
            ),
            MC_STATUS_GENERATION_MISMATCH,
            "stale tick must not read a later incarnation"
        );
    }
    assert_eq!(
        (
            balloon_row.authority,
            balloon_row.active,
            balloon_row.class_id,
            balloon_row.model,
            balloon_row.action
        ),
        (MC_AUTHORITY_CAMPAIGN_LIVE, 1, 3, 3, 9)
    );
    assert_eq!(
        (
            balloon_row.source_id,
            balloon_row.x,
            balloon_row.y,
            balloon_row.z,
            balloon_row.actual_speed
        ),
        (
            player_slot,
            balloon.position.x,
            balloon.position.y,
            balloon.position.z,
            balloon.speed
        )
    );
    let mut sprites = vec![McCampaignEntityPresentationV2::default(); 1_000];
    let mut written = 0;
    assert_eq!(
        mc_native_campaign_entity_presentations_copy(
            handle,
            0,
            step.revision,
            0,
            sprites.as_mut_ptr(),
            sprites.len() as u32,
            struct_size::<McCampaignEntityPresentationV2>(),
            &raw mut written,
        ),
        MC_STATUS_OK,
        "{}",
        LAST_ERROR.with(|value| value.borrow().clone())
    );
    let sprites = &sprites[..written as usize];
    let castle_sprite = sprites
        .iter()
        .find(|row| row.entity_index == castle.source_entity_index)
        .expect("the live Castle reaches the native visible selector");
    assert_eq!(
        (
            castle_sprite.render_parameter_index,
            castle_sprite.base_tmap
        ),
        (177, 96)
    );
    let sphere_sprite = sprites
        .iter()
        .find(|row| row.entity_index == sphere_slot)
        .expect("sphere reaches the native visible selector");
    assert_eq!(sphere_sprite.render_parameter_index, 106);
    let balloon_sprite = sprites
        .iter()
        .find(|row| row.entity_index == balloon.source_entity_index)
        .expect("Castle balloon reaches the native visible selector");
    assert_eq!(
        (
            balloon_sprite.render_parameter_index,
            balloon_sprite.base_tmap
        ),
        (169, 88)
    );
    with_campaign_session(handle, |record| {
        let CampaignSessionKind::Campaign { reset, .. } = &record.kind else {
            unreachable!()
        };
        assert_eq!(
            reset.state_hash(),
            before_hash,
            "native presentation reads are observational"
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    let reset_entities = campaign_entities_public(handle);
    assert_eq!(
        reset_entities[usize::from(castle.source_entity_index)].active,
        0
    );
    assert_eq!(reset_entities[usize::from(sphere_slot)].active, 0);
    assert_eq!(
        reset_entities[usize::from(balloon.source_entity_index)].active,
        0
    );
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

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
    assert_eq!(size_of::<McCampaignEntityPresentationV2>(), 64);
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
    assert_eq!(std::mem::offset_of!(McCampaignDayHudSpellSlotV2, help_unavailable), 38);
    assert_eq!(std::mem::offset_of!(McCampaignDayHudSpellSlotV2, reserved_padding), 39);
    assert_eq!(std::mem::offset_of!(McCampaignDayHudSpellSlotV2, reserved), 40);
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
    let event = Mc2DispatchEvent::Economy {
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
        action: 41,
        entity_rng: 0x1234,
        position: Position::default(),
        velocity_x: 0,
        velocity_y: 0,
        vertical_velocity: 0,
        yaw: 0,
        roll: 0,
        actual_speed: 32,
        max_life: 300,
        life: 300,
        mana: 321,
        particle_index: 53,
        collision_yaw: 28,
        collision_pitch: 28,
        collision_roll: 28,
        collision_fov: 28,
        owner_entity_index: 111,
        player_entity_index: 59,
        incoming_owner_entity_index: 0,
        incoming_possession_payload: 0,
        ordinary_damage_payload: 0,
        ordinary_damage_owner: 0,
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
    assert_eq!(snapshot.reserved, [877, 42 | (3 << 16), 59]);
    assert_eq!(
        (
            snapshot.owner_entity_index,
            snapshot.render_parameter_index,
            snapshot.base_tmap,
            snapshot.logical_height,
        ),
        (111, 53, 58, 100),
        "the ABI retains constructor visuals after the death producer copies owner111"
    );
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
fn descriptor_campaign_counts_follow_the_level_definition() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-bhq8-level1-collision-v5-night");
    let package = package.to_string_lossy();
    let selection = McCampaignSelectionV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignSelectionV2>(),
        external_level: 1,
        internal_level: 0,
        map_type: 1,
        ..McCampaignSelectionV2::default()
    };
    let mut handle = 0;
    assert_eq!(
        mc_native_campaign_session_open(
            package.as_ptr(),
            package.len() as u32,
            std::ptr::null(),
            0,
            &raw const selection,
            &raw mut handle,
        ),
        MC_STATUS_OK
    );

    let mut info = McCampaignWorldInfoV2::default();
    assert_eq!(
        mc_native_campaign_world_info(
            handle,
            &raw mut info,
            struct_size::<McCampaignWorldInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (
            info.external_level,
            info.internal_level,
            info.map_type,
            info.entity_count,
            info.active_entity_count,
            info.player_count,
            info.objective_count,
        ),
        (1, 0, 1, 1_000, 177, 1, 5)
    );

    let mut player_count = 0;
    assert_eq!(
        mc_native_campaign_player_count(handle, &raw mut player_count),
        MC_STATUS_OK
    );
    assert_eq!(player_count, 1);
    let mut players = [McCampaignPlayerSnapshotV2::default(); 8];
    let mut players_written = 0;
    assert_eq!(
        mc_native_campaign_players_copy(
            handle,
            0,
            players.as_mut_ptr(),
            players.len() as u32,
            struct_size::<McCampaignPlayerSnapshotV2>(),
            &raw mut players_written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(players_written, 1);
    assert_eq!(
        (players[0].slot, players[0].active, players[0].is_ai),
        (0, 1, 0)
    );

    let mut objective_count = 0;
    assert_eq!(
        mc_native_campaign_objective_count(handle, &raw mut objective_count),
        MC_STATUS_OK
    );
    assert_eq!(objective_count, 5);
    let mut objectives = [McObjectiveSnapshotV2::default(); 8];
    let mut objectives_written = 0;
    assert_eq!(
        mc_native_campaign_objectives_copy(
            handle,
            0,
            objectives.as_mut_ptr(),
            objectives.len() as u32,
            struct_size::<McObjectiveSnapshotV2>(),
            &raw mut objectives_written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(objectives_written, 5);
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
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
    assert_eq!(written, 8);
    assert_eq!(level2_players[0].active + level2_players[1].active, 2);
    assert!(level2_players[2..].iter().all(|player| player.active == 0));
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
    // Compare independent opens of the same package and profile. The shared
    // gameplay helper opens a descriptor-backed package, whose reset hash and
    // semantic player census differ from this retained inspection input.
    let mut independent_level2_handle = 0;
    assert_eq!(
        mc_native_campaign_session_open(
            package.as_ptr(),
            package.len() as u32,
            std::ptr::null(),
            0,
            &raw const selection,
            &raw mut independent_level2_handle,
        ),
        MC_STATUS_OK
    );
    assert_ne!(independent_level2_handle, 0);
    assert_ne!(independent_level2_handle, level2_handle);
    let mut independent_level2_info = McCampaignWorldInfoV2::default();
    assert_eq!(
        mc_native_campaign_world_info(
            independent_level2_handle,
            &raw mut independent_level2_info,
            struct_size::<McCampaignWorldInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(independent_level2_info, level2_info);
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
        events[..event_count as usize].to_vec()
    };

    let _ = step_at(handle, 0, state.axis_x, state.axis_y);
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

    let second_events = step_at(handle, 0, state.axis_x, state.axis_y);
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
    assert_eq!(completed.done, 0);
    assert!(
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "objective proof requires a Level 2 campaign session",
                ));
            };
            Ok(reset.level2_objective_done())
        })
        .expect("read the persistent ObjectiveDone production carrier"),
        "the frame-local public byte is zero only after the real persistent ObjectiveDone carrier was produced"
    );
    assert_eq!(
        campaign_authored_group_events(handle)
            .into_iter()
            .filter(|event| event.group == 8 && event.source_slot == 32)
            .map(|event| event.authored_index)
            .collect::<Vec<_>>(),
        [153, 154],
        "the frame-local ObjectiveDone pulse is consumed only after the real group-8 producer publishes its exact package rows"
    );
    assert!(second_events.iter().any(|event| {
        event.kind == MC_CAMPAIGN_EVENT_AUTHORED_GROUP
            && event.family == 8
            && event.flags == 2
            && event.source_slot == 32
    }));

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
fn campaign_palette_copy_tracks_opening_objective_flash_and_recovery() {
    const PALETTE_BYTES: usize = 768;
    let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
    let handle = open_level2_campaign_handle_with_package_and_asset_cache(
        &generated.join("mc-level2-vault-successor-closure-20261025"),
        &generated.join("mc-b9s2-cursor-level2-vault-20261029"),
    );
    let mut palette = [0_u8; PALETTE_BYTES];
    let mut written = 99;
    // The palette-only API must work before any indexed frame has rendered.
    assert_eq!(mc_native_campaign_palette_copy(handle, 0, 0, palette.as_mut_ptr(),
        PALETTE_BYTES as u32, &raw mut written), MC_STATUS_OK);
    assert_eq!(written, PALETTE_BYTES as u32);
    let original = palette;
    let mut sentinel = [0xa5_u8; PALETTE_BYTES];
    for (generation, tick, capacity, expected) in [
        (1, 0, 768, MC_STATUS_GENERATION_MISMATCH),
        (0, 1, 768, MC_STATUS_GENERATION_MISMATCH),
        (0, 0, 767, MC_STATUS_BUFFER_TOO_SMALL),
    ] {
        written = 99;
        assert_eq!(mc_native_campaign_palette_copy(handle, generation, tick,
            sentinel.as_mut_ptr(), capacity, &raw mut written), expected);
        assert_eq!(written, 0);
        assert_eq!(sentinel, [0xa5; PALETTE_BYTES]);
    }
    assert_eq!(mc_native_campaign_palette_copy(handle, 0, 0, std::ptr::null_mut(),
        PALETTE_BYTES as u32, &raw mut written), MC_STATUS_NULL_POINTER);
    assert_eq!(written, 0);

    let mut index = vec![0_u8; 320 * 200];
    let mut rendered_palette = [0_u8; PALETTE_BYTES];
    let mut flashed = false;
    let mut recovered = false;
    for tick in 0..96_u64 {
        if tick != 0 { step_level2_campaign_once(handle, 0); }
        written = 0;
        assert_eq!(mc_native_campaign_palette_copy(handle, 0, tick, palette.as_mut_ptr(),
            PALETTE_BYTES as u32, &raw mut written), MC_STATUS_OK);
        assert_eq!(written, PALETTE_BYTES as u32);
        let mut info = McCampaignDayIndexedFrameInfoV2::default();
        assert_eq!(mc_native_campaign_day_indexed_frame_render(handle, 0, tick,
            &raw mut info, struct_size::<McCampaignDayIndexedFrameInfoV2>()), MC_STATUS_OK);
        let (mut index_written, mut palette_written) = (0, 0);
        assert_eq!(mc_native_campaign_day_indexed_frame_copy(handle, 0, tick,
            index.as_mut_ptr(), index.len() as u32, rendered_palette.as_mut_ptr(),
            PALETTE_BYTES as u32, &raw mut index_written, &raw mut palette_written), MC_STATUS_OK);
        assert_eq!((index_written, palette_written), (index.len() as u32, PALETTE_BYTES as u32));
        assert_eq!(palette, rendered_palette, "palette-only and indexed owner must agree at tick {tick}");
        flashed |= palette != original;
        if flashed && palette == original { recovered = true; break; }
    }
    assert!(flashed && recovered, "ordinary opening objective flash must change and restore the actual owner palette");
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
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

    let (material_only_status, material_only_handle) =
        open_with_cache(&cache.join("components/material"), &selection);
    assert_eq!(material_only_status, MC_STATUS_OK);
    assert_ne!(material_only_handle, 0);
    assert_eq!(
        mc_native_campaign_session_destroy(material_only_handle),
        MC_STATUS_OK
    );

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
        .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2");
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
fn direct_material_root_campaign_open_binds_primary_frame_and_rejects_wrong_cache() {
    let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
    let cases = [
        (
            5,
            4,
            0,
            "dragon-bee-20261105-integration/level5-owner-current/level5",
            "secret-lifecycle-20261111-source4-environment",
        ),
        (
            31,
            30,
            2,
            "mc-paj2-source30-campaign-package",
            "mc-paj2-environment-source30-audio-20261026",
        ),
    ];
    for (index, &(external_level, internal_level, map_type, package, cache)) in
        cases.iter().enumerate()
    {
        let package = generated.join(package);
        let cache = generated.join(cache);
        assert!(cache.join("materials/descriptor.json").is_file());
        assert!(!cache.join("components/gameplay/manifest.json").is_file());
        let package = package.to_string_lossy();
        let selection = McCampaignSelectionV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCampaignSelectionV2>(),
            external_level,
            internal_level,
            map_type,
            ..McCampaignSelectionV2::default()
        };
        let open = |root: &Path, handle: &mut u64| {
            let root = root.to_string_lossy();
            mc_native_campaign_session_open_with_asset_cache(
                package.as_ptr(),
                package.len() as u32,
                std::ptr::null(),
                0,
                root.as_ptr(),
                root.len() as u32,
                &raw const selection,
                handle,
            )
        };
        let mut handle = 0;
        assert_eq!(
            open(&cache, &mut handle),
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_ne!(handle, 0);
        let query = cf12_frame_query(0, 0);
        let mut pixels = vec![0; 320 * 200];
        let mut frame = McPresentationFrameInfoV2::default();
        assert_eq!(
            mc_native_campaign_presentation_frame_copy(
                handle,
                &raw const query,
                pixels.as_mut_ptr(),
                pixels.len() as u32,
                std::ptr::null_mut(),
                0,
                &raw mut frame,
                struct_size::<McPresentationFrameInfoV2>(),
            ),
            MC_STATUS_OK,
            "source {internal_level}: {}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!((frame.generation, frame.tick), (0, 0));
        assert_ne!(frame.index_hash, 0);
        let wrong_cache = generated.join(cases[1 - index].4);
        let mut rejected = u64::MAX;
        assert_eq!(
            open(&wrong_cache, &mut rejected),
            MC_STATUS_VALIDATION_FAILED
        );
        assert_eq!(rejected, 0);
        assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
    }
}

#[test]
fn same_tick_camera_refresh_keeps_generic_and_retained_frames_identical() {
    let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
    let package = generated.join("level3-day-block32-campaign-recipe-v2");
    let cache = generated.join("mc-level3-environment-exit-20261025");
    let package = package.to_string_lossy();
    let cache = cache.to_string_lossy();
    let selection = McCampaignSelectionV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignSelectionV2>(),
        external_level: 3,
        internal_level: 2,
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
        MC_STATUS_OK
    );
    let query = cf12_frame_query(0, 0);
    let copy = || {
        let mut pixels = vec![0; 320 * 200];
        let mut info = McPresentationFrameInfoV2::default();
        assert_eq!(
            mc_native_campaign_presentation_frame_copy(
                handle,
                &raw const query,
                pixels.as_mut_ptr(),
                pixels.len() as u32,
                std::ptr::null_mut(),
                0,
                &raw mut info,
                struct_size::<McPresentationFrameInfoV2>()
            ),
            MC_STATUS_OK
        );
        (pixels, info)
    };
    let (initial, _) = copy();
    let mut scene = vec![0xa5; 320 * 200];
    let mut written = 99;
    let mut revision = 99;
    let mut blur = 99;
    let mut exit_tables = 99;
    let mut sprites_composited = 99;
    assert_eq!(
        mc_native_campaign_scene_frame_copy(
            handle,
            0,
            0,
            scene.as_mut_ptr(),
            1,
            &raw mut written,
            &raw mut revision,
            &raw mut blur,
            &raw mut exit_tables,
            &raw mut sprites_composited
        ),
        MC_STATUS_BUFFER_TOO_SMALL
    );
    assert_eq!(
        (written, revision, blur, exit_tables, sprites_composited),
        (0, 0, 0, 0, 0)
    );
    assert!(scene.iter().all(|&index| index == 0xa5));
    assert_eq!(
        mc_native_campaign_scene_frame_copy(
            handle,
            0,
            0,
            scene.as_mut_ptr(),
            scene.len() as u32,
            &raw mut written,
            &raw mut revision,
            &raw mut blur,
            &raw mut exit_tables,
            &raw mut sprites_composited
        ),
        MC_STATUS_OK
    );
    assert_eq!(written, 64000);
    assert_eq!(sprites_composited, 1);
    let initial_scene_revision = revision;
    let mut map = vec![0; 64000];
    let mut map_palette = vec![0; 768];
    let mut entities = vec![McCampaignMinimapEntityV2::default(); 1000];
    let mut marks = vec![McCampaignMinimapMarkV2::default(); 64];
    let mut map_state = McCampaignMinimapStateV2::default();
    let mut map_copy = |frame: &mut Vec<u8>,
                        entities: &mut Vec<McCampaignMinimapEntityV2>,
                        marks: &mut Vec<McCampaignMinimapMarkV2>,
                        state: &mut McCampaignMinimapStateV2| {
        super::campaign_world_map::mc_native_campaign_world_map_copy(
            handle,
            0,
            0,
            frame.as_mut_ptr(),
            frame.len() as u32,
            map_palette.as_mut_ptr(),
            map_palette.len() as u32,
            entities.as_mut_ptr(),
            entities.len() as u32,
            marks.as_mut_ptr(),
            marks.len() as u32,
            state,
            struct_size::<McCampaignMinimapStateV2>(),
        )
    };
    assert_eq!(
        map_copy(&mut map, &mut entities, &mut marks, &mut map_state),
        MC_STATUS_OK
    );
    assert_eq!(map_state.generation, 0);
    assert_eq!(map_state.tick, 0);
    assert!(map.iter().any(|&index| index != map[0]));
    assert!(map.chunks_exact(320).all(|row| row[191] == row[192]));
    let repeated_map = map.clone();
    assert_eq!(
        map_copy(&mut map, &mut entities, &mut marks, &mut map_state),
        MC_STATUS_OK
    );
    assert_eq!(
        map, repeated_map,
        "repeated map reads retain the committed narrow source view"
    );
    // Isolate the same-tick camera-cache boundary. The sim-core exit test
    // exercises the real action12 producer; this fixture earns no transition.
    with_campaign_session_mut(handle, |record| {
        let CampaignSessionKind::Campaign { reset, .. } = &mut record.kind else {
            unreachable!()
        };
        reset.state.position.x = reset.state.position.x.wrapping_add(1024);
        Ok(())
    })
    .unwrap();
    assert_eq!(
        mc_native_campaign_scene_frame_copy(
            handle,
            0,
            0,
            scene.as_mut_ptr(),
            scene.len() as u32,
            &raw mut written,
            &raw mut revision,
            &raw mut blur,
            &raw mut exit_tables,
            &raw mut sprites_composited
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(
        (written, revision, blur, exit_tables, sprites_composited),
        (0, 0, 0, 0, 0)
    );
    let (refreshed, generic) = copy();
    assert_ne!(initial, refreshed);
    assert_eq!(generic.newly_committed, 1);
    let mut retained = McCampaignDayIndexedFrameInfoV2::default();
    assert_eq!(
        mc_native_campaign_day_indexed_frame_render(
            handle,
            0,
            0,
            &raw mut retained,
            struct_size::<McCampaignDayIndexedFrameInfoV2>()
        ),
        MC_STATUS_OK
    );
    assert_eq!(generic.index_hash, retained.index_hash);
    let (repeated, repeated_info) = copy();
    assert_eq!(repeated, refreshed);
    assert_eq!(repeated_info.newly_committed, 0);
    assert_eq!(
        mc_native_campaign_scene_frame_copy(
            handle,
            0,
            0,
            scene.as_mut_ptr(),
            scene.len() as u32,
            &raw mut written,
            &raw mut revision,
            &raw mut blur,
            &raw mut exit_tables,
            &raw mut sprites_composited
        ),
        MC_STATUS_OK
    );
    assert_eq!(revision, initial_scene_revision + 1);
    assert_eq!((blur, exit_tables), (0, 0));
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
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
        assert!(
            [
                campaign_facade::MC_CAMPAIGN_EFFECT_DISPATCH,
                campaign_facade::MC_CAMPAIGN_EFFECT_SHARED_SPELL,
                campaign_facade::MC_CAMPAIGN_EFFECT_FLIGHT,
                campaign_facade::MC_CAMPAIGN_EFFECT_SCENE_CHANGE,
            ]
            .contains(&effect.kind)
        );
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
    assert_eq!(
        cf12_world_info(handle).reset_state_hash,
        initial_world.reset_state_hash
    );
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
    assert_eq!(
        empty_batch.written, 0,
        "failed step must not append effects"
    );

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
    assert_eq!(
        (
            first_unaligned_event.sequence,
            first_unaligned_event.state_hash
        ),
        (1, step.state_hash)
    );

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
    assert_eq!(
        frame_info.newly_committed, 1,
        "failed copies must not commit the frame"
    );
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
        .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2");
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

fn cf14_level3_descriptor_package() -> PathBuf {
    std::env::var_os("MC_CAMPAIGN_PACKAGE_LEVEL3_V1").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3")
        },
        PathBuf::from,
    )
}

#[test]
fn level2_campaign_hud_and_minimap_keep_exact_nyphur_reset_identity() {
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
    let mut handle = 0;
    assert_eq!(
        mc_native_campaign_session_open(
            package.as_ptr(),
            package.len() as u32,
            std::ptr::null(),
            0,
            &raw const selection,
            &raw mut handle,
        ),
        MC_STATUS_OK
    );
    let mut hud = McCampaignDayHudSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_day_hud_state(
            handle,
            0,
            0,
            &raw mut hud,
            struct_size::<McCampaignDayHudSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (
            hud.nyphur.active,
            hud.nyphur.player_slot,
            hud.nyphur.entity_index,
            hud.nyphur.fireball_holder_index,
            hud.nyphur.possession_holder_index,
        ),
        (1, 1, 112, 113, 114)
    );
    let mut minimap = McCampaignMinimapStateV2::default();
    assert_eq!(
        mc_native_campaign_minimap_state(
            handle,
            0,
            0,
            &raw mut minimap,
            struct_size::<McCampaignMinimapStateV2>(),
        ),
        MC_STATUS_OK
    );
    assert!(minimap.entity_count > 0);
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

fn cf14_campaign_day_material_cache() -> PathBuf {
    std::env::var_os("MC_CAMPAIGN_ASSET_CACHE_LEVEL3_V1").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/level3-day-material-cache")
        },
        PathBuf::from,
    )
}

#[test]
fn cf14_level3_uses_generic_campaign_abi() {
    const EVENT_CAPACITY: usize = 1_024;
    const EFFECT_CAPACITY: usize = 16_384;
    const INDEX_BYTES: usize = 320 * 200;

    // The census-profiled Level 3 selection travels the same frozen campaign
    // layouts and entry points as every descriptor-backed campaign.  No
    // Level-3 handle kind, symbol, selector layout, or event layout exists.
    let package = cf14_level3_descriptor_package();
    let direct = DescriptorCampaignSession::open(
        &package,
        CampaignSelection {
            external_level: 3,
            internal_level: 2,
            map_type_id: 0,
        },
    )
    .expect("open the same Level 3 descriptor authority directly");
    let authored_reset_hash = direct.state_hash();
    let package = package.to_string_lossy();
    let cache = cf14_campaign_day_material_cache();
    let cache = cache.to_string_lossy();
    let selection = McCampaignSelectionV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignSelectionV2>(),
        external_level: 3,
        internal_level: 2,
        map_type: 0,
        ..McCampaignSelectionV2::default()
    };
    let mut handle = u64::MAX;
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

    let initial_world = cf12_world_info(handle);
    assert_eq!(
        (
            initial_world.external_level,
            initial_world.internal_level,
            initial_world.map_type,
            initial_world.active_entity_count,
        ),
        (3, 2, 0, 252)
    );
    assert_eq!(
        initial_world.reset_state_hash, authored_reset_hash,
        "generic open must preserve the Level 3 package's authored reset state"
    );

    let initial_query = McEventQueryV2 {
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
            &raw const initial_query,
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
            initial_frame.width,
            initial_frame.height,
            initial_frame.index_bytes,
            initial_frame.newly_committed,
        ),
        (0, 0, 320, 200, INDEX_BYTES as u32, 1)
    );
    assert_ne!(initial_frame.index_hash, 0);
    assert_eq!(byte_hash(&initial_indices), initial_frame.index_hash);
    assert_eq!(
        initial_frame.flags
            & (MC_PRESENTATION_FRAME_COMPOSITOR_COMPLETE
                | MC_PRESENTATION_FRAME_HAS_PROVENANCE
                | MC_PRESENTATION_FRAME_OVERLAYS_COMPOSITED
                | MC_PRESENTATION_FRAME_POSTPROCESS_COMPLETE
                | MC_PRESENTATION_FRAME_LIGHTING_COMPLETE),
        0
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

    let mut players = [McCampaignPlayerSnapshotV2::default(); 8];
    let mut player_count = 0;
    assert_eq!(
        mc_native_campaign_players_copy(
            handle,
            0,
            players.as_mut_ptr(),
            players.len() as u32,
            struct_size::<McCampaignPlayerSnapshotV2>(),
            &raw mut player_count,
        ),
        MC_STATUS_OK
    );
    assert_eq!(player_count, 1);
    let local = players[0];
    assert_eq!((local.slot, local.active, local.is_ai), (0, 1, 0));

    let mut local_entity = McCampaignEntitySnapshotV2::default();
    let mut entity_count = 0;
    assert_eq!(
        mc_native_campaign_entities_copy(
            handle,
            u32::from(local.entity_index),
            &raw mut local_entity,
            1,
            struct_size::<McCampaignEntitySnapshotV2>(),
            &raw mut entity_count,
        ),
        MC_STATUS_OK
    );
    assert_eq!(entity_count, 1);
    assert_eq!(
        (local_entity.entity_index, local_entity.active),
        (local.entity_index, 1)
    );

    let assert_level3_surfaces = |tick: u64, visible: bool| {
        let mut hud = McCampaignDayHudSnapshotV2::default();
        assert_eq!(
            mc_native_campaign_day_hud_state(
                handle,
                0,
                tick,
                &raw mut hud,
                struct_size::<McCampaignDayHudSnapshotV2>(),
            ),
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!((hud.generation, hud.tick), (0, tick));
        assert_eq!(hud.local_player_entity_index, local.entity_index);
        assert_eq!(hud.minimap_panel_selector, 40);

        let mut minimap = McCampaignMinimapStateV2::default();
        assert_eq!(
            mc_native_campaign_minimap_state(
                handle,
                0,
                tick,
                &raw mut minimap,
                struct_size::<McCampaignMinimapStateV2>(),
            ),
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!((minimap.generation, minimap.tick), (0, tick));
        assert!(minimap.entity_count > 0);

        let mut sprites = vec![McCampaignEntityPresentationV2::default(); 1_000];
        let mut sprite_count = 0;
        assert_eq!(
            mc_native_campaign_entity_presentations_copy(
                handle,
                0,
                tick,
                0,
                sprites.as_mut_ptr(),
                sprites.len() as u32,
                struct_size::<McCampaignEntityPresentationV2>(),
                &raw mut sprite_count,
            ),
            MC_STATUS_OK,
            "{}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!(
            sprite_count > 0,
            visible,
            "source range admission at this camera"
        );
        assert!(sprites[..sprite_count as usize].iter().all(|row| {
            row.abi_version == MC_NATIVE_ABI_VERSION
                && row.struct_size == struct_size::<McCampaignEntityPresentationV2>()
                && row.generation == 0
                && row.tick == tick
                && row.selected_tmap >= row.base_tmap
                && row.pixel_mode <= 7
                && row.brightness <= 0x2000
                && row.owner_color == 0x7b
                && row.reserved == 0
        }));
    };
    assert_level3_surfaces(0, false);

    let nearby_actor = direct
        .simulation()
        .entities()
        .iter()
        .find(|row| row.active && row.class == 5 && row.particle_parameter_index.is_some())
        .expect("real package creature for positive draw admission");

    let step_input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command: valid_command(),
        player_slot: local.slot,
        player_x: nearby_actor.position.x,
        player_y: nearby_actor.position.y.wrapping_add(1024),
        player_z: nearby_actor.position.z,
        ..McCampaignStepInputV2::default()
    };
    let mut step = McCampaignStepSnapshotV2::default();
    let mut events = vec![McCampaignEventV2::default(); EVENT_CAPACITY];
    let mut event_count = 0;
    assert_eq!(
        mc_native_campaign_session_step(
            handle,
            &raw const step_input,
            &raw mut step,
            struct_size::<McCampaignStepSnapshotV2>(),
            events.as_mut_ptr(),
            events.len() as u32,
            struct_size::<McCampaignEventV2>(),
            &raw mut event_count,
        ),
        MC_STATUS_OK,
        "{}",
        LAST_ERROR.with(|value| value.borrow().clone())
    );
    assert_eq!((step.generation, step.revision), (0, 1));
    assert_eq!(step.player_slot, local.slot);
    assert_eq!(step.player_entity_index, local.entity_index);
    assert_ne!(step.state_hash, initial_world.reset_state_hash);
    assert_ne!(step.terrain_hash, 0);
    assert!(event_count > 0);
    assert_eq!(step.event_count, event_count);
    let events = &events[..event_count as usize];
    assert!(
        events
            .iter()
            .enumerate()
            .all(|(ordinal, event)| { event.generation == 0 && event.ordinal == ordinal as u32 })
    );

    let mut effects = vec![McEventV2::default(); EFFECT_CAPACITY];
    let mut effect_batch = McEventBatchV2::default();
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            &raw const initial_query,
            effects.as_mut_ptr(),
            effects.len() as u32,
            struct_size::<McEventV2>(),
            &raw mut effect_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert!(effect_batch.written > 0);
    assert_eq!(effect_batch.next_cursor, effect_batch.event_head);
    assert_eq!(effect_batch.has_more, 0);
    let effects = &effects[..effect_batch.written as usize];
    assert!(effects.iter().enumerate().all(|(ordinal, effect)| {
        effect.generation == 0
            && effect.sequence == ordinal as u64 + 1
            && effect.state_hash == step.state_hash
    }));

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
    assert_eq!((stepped_frame.generation, stepped_frame.tick), (0, 1));
    assert_eq!(byte_hash(&stepped_indices), stepped_frame.index_hash);
    assert_level3_surfaces(1, true);

    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    let mut stale_batch = McEventBatchV2::default();
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            &raw const initial_query,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut stale_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    let reset_query = McEventQueryV2 {
        generation: 1,
        ..initial_query
    };
    let mut reset_batch = McEventBatchV2::default();
    assert_eq!(
        mc_native_campaign_events_copy_since(
            handle,
            &raw const reset_query,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut reset_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (
            reset_batch.generation,
            reset_batch.next_cursor,
            reset_batch.event_head,
            reset_batch.written,
        ),
        (1, 0, 0, 0)
    );
    assert_eq!(
        cf12_world_info(handle).reset_state_hash,
        initial_world.reset_state_hash
    );
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
    assert_eq!(reset_indices, initial_indices);
    assert_eq!(reset_frame.index_hash, initial_frame.index_hash);

    let replay_input = McCampaignStepInputV2 {
        generation: 1,
        ..step_input
    };
    let mut replay = McCampaignStepSnapshotV2::default();
    let mut replay_events = vec![McCampaignEventV2::default(); EVENT_CAPACITY];
    let mut replay_event_count = 0;
    assert_eq!(
        mc_native_campaign_session_step(
            handle,
            &raw const replay_input,
            &raw mut replay,
            struct_size::<McCampaignStepSnapshotV2>(),
            replay_events.as_mut_ptr(),
            replay_events.len() as u32,
            struct_size::<McCampaignEventV2>(),
            &raw mut replay_event_count,
        ),
        MC_STATUS_OK
    );
    assert_eq!(replay.generation, 1);
    assert_eq!(
        (
            replay.revision,
            replay.state_hash,
            replay.terrain_hash,
            replay.dispatch_hash,
            replay.event_count,
            replay.event_kind_mask,
            replay.player_slot,
            replay.player_entity_index,
            replay.x,
            replay.y,
            replay.z,
        ),
        (
            step.revision,
            step.state_hash,
            step.terrain_hash,
            step.dispatch_hash,
            step.event_count,
            step.event_kind_mask,
            step.player_slot,
            step.player_entity_index,
            step.x,
            step.y,
            step.z,
        )
    );
    assert_eq!(replay_event_count, event_count);
    let replay_events = &replay_events[..replay_event_count as usize];
    assert!(events.iter().zip(replay_events).all(|(first, replay)| {
        (
            first.ordinal,
            first.pass,
            first.kind,
            first.family,
            first.flags,
            first.source_slot,
        ) == (
            replay.ordinal,
            replay.pass,
            replay.kind,
            replay.family,
            replay.flags,
            replay.source_slot,
        )
    }));
    let replay_frame_query = cf12_frame_query(1, 1);
    let mut replay_indices = vec![0; INDEX_BYTES];
    let mut replay_frame = McPresentationFrameInfoV2::default();
    assert_eq!(
        mc_native_campaign_presentation_frame_copy(
            handle,
            &raw const replay_frame_query,
            replay_indices.as_mut_ptr(),
            replay_indices.len() as u32,
            std::ptr::null_mut(),
            0,
            &raw mut replay_frame,
            struct_size::<McPresentationFrameInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(replay_indices, stepped_indices);
    assert_eq!(replay_frame.index_hash, stepped_frame.index_hash);
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);

    // A mismatched caller triple against the same package fails at the outer
    // manifest stage, before behavior closure.
    let mismatched = McCampaignSelectionV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignSelectionV2>(),
        external_level: 2,
        internal_level: 1,
        map_type: 0,
        ..McCampaignSelectionV2::default()
    };
    let mut mismatched_handle = u64::MAX;
    assert_eq!(
        mc_native_campaign_session_open(
            package.as_ptr(),
            package.len() as u32,
            std::ptr::null(),
            0,
            &raw const mismatched,
            &raw mut mismatched_handle,
        ),
        MC_STATUS_VALIDATION_FAILED
    );
    assert_eq!(mismatched_handle, 0);
    let mismatch_error = LAST_ERROR.with(|value| value.borrow().clone());
    assert!(
        mismatch_error.contains("campaign preflight stage 1"),
        "selection mismatch must fail at the outer manifest: {mismatch_error}"
    );

    // A material cache is bound to the owner package it was derived from.
    // Matching Day selectors are not enough to borrow Level 2 presentation
    // bytes for this Level 3 campaign.
    let level2_cache = qualified_level2_day_owner_cache();
    let level2_cache = level2_cache.to_string_lossy();
    let mut cross_bound_handle = u64::MAX;
    assert_eq!(
        mc_native_campaign_session_open_with_asset_cache(
            package.as_ptr(),
            package.len() as u32,
            std::ptr::null(),
            0,
            level2_cache.as_ptr(),
            level2_cache.len() as u32,
            &raw const selection,
            &raw mut cross_bound_handle,
        ),
        MC_STATUS_VALIDATION_FAILED
    );
    assert_eq!(cross_bound_handle, 0);

    // An unprofiled selection can never borrow the generic route.
    let unprofiled = McCampaignSelectionV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignSelectionV2>(),
        external_level: 4,
        internal_level: 3,
        map_type: 0,
        ..McCampaignSelectionV2::default()
    };
    let mut unprofiled_handle = u64::MAX;
    assert_eq!(
        mc_native_campaign_session_open(
            package.as_ptr(),
            package.len() as u32,
            std::ptr::null(),
            0,
            &raw const unprofiled,
            &raw mut unprofiled_handle,
        ),
        MC_STATUS_VALIDATION_FAILED
    );
    assert_eq!(unprofiled_handle, 0);
    let unprofiled_error = LAST_ERROR.with(|value| value.borrow().clone());
    assert!(
        unprofiled_error.contains("campaign preflight stage 1"),
        "unprofiled selections must fail closed at the outer manifest: {unprofiled_error}"
    );

    // rg-style guard: the generic ABI stays free of per-level Level 3 symbols.
    // The needles are assembled at runtime so this test cannot match itself.
    let forbidden = [
        format!("mc_native_{}", "level3"),
        format!("Level{}Session", 3),
        format!("mc_native_{}", "level2"),
    ];
    let mut sources = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(directory) = sources.pop() {
        for entry in std::fs::read_dir(&directory).expect("enumerate sim-capi sources") {
            let entry = entry.expect("inspect sim-capi source entry");
            let path = entry.path();
            if path.is_dir() {
                sources.push(path);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read sim-capi source");
            for needle in &forbidden {
                assert!(
                    !source.contains(needle.as_str()),
                    "forbidden per-level ABI symbol {needle} in {}",
                    path.display()
                );
            }
        }
    }
}
