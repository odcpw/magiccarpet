#[test]
fn package_m_world_reset_and_copy_surfaces_are_exact() {
    let handle = open_fixture();
    assert_ne!(handle, 0);
    let mut info = McWorldInfoV2::default();
    assert_eq!(
        mc_native_world_info(handle, &raw mut info, struct_size::<McWorldInfoV2>()),
        MC_STATUS_OK
    );
    assert_eq!((info.external_level, info.internal_level), (1, 0));
    assert_eq!(info.map_type, 1);
    assert_eq!(info.profile, MC_PROFILE_OUTDOOR);
    assert_eq!(info.entity_count, 1_000);
    assert_eq!(info.active_entity_count, 177);
    assert_eq!(info.player_count, 8);
    assert_eq!(info.asset_ref_count, 76);
    assert_eq!(info.reset_state_hash, 0x9da4_bc91_af58_f3ce);
    assert_eq!(info.reset_conformance_hash, 0x77b9_6b82_492d_6f4b);

    let mut snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_snapshot(
            handle,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(snapshot.state_hash, info.reset_state_hash);
    assert_eq!(
        snapshot.conformance_hash, 0x36bd_7f1d_df05_37b5,
        "live v2 conformance includes spell tiers and the additive Level 1 effects journal"
    );
    assert_ne!(snapshot.conformance_hash, info.reset_conformance_hash);
    assert_eq!(
        (snapshot.x, snapshot.y, snapshot.z),
        (19_840, 56_960, 5_024)
    );
    assert_eq!(
        snapshot.objective_textbox_ticks, 0,
        "package M captures remc2 before PresentObjective installs its 200-tick textbox"
    );
    assert_eq!(snapshot.objective_count, 8);
    assert_eq!(snapshot.objectives_authority, MC_AUTHORITY_FROZEN_OBSERVED);

    let mut terrain_state = McTerrainStateV2::default();
    assert_eq!(
        mc_native_session_terrain_state(
            handle,
            &raw mut terrain_state,
            struct_size::<McTerrainStateV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(terrain_state.generation, snapshot.generation);
    assert_eq!(terrain_state.tick, snapshot.tick);
    assert_eq!(terrain_state.mutation_sequence, 0);
    assert_eq!(
        (
            terrain_state.width,
            terrain_state.height,
            terrain_state.plane_count,
            terrain_state.hash_schema,
            terrain_state.authority,
        ),
        (256, 256, 6, 1, MC_AUTHORITY_SESSION_TERRAIN_LIVE)
    );
    assert_eq!(terrain_state.terrain_hash, 0x1cb0_31cc_76e2_140e);

    let mut lifecycle = McLevel1CastleLifecycleSnapshotV2::default();
    assert_eq!(
        mc_native_level1_castle_lifecycle_snapshot(
            handle,
            &raw mut lifecycle,
            struct_size::<McLevel1CastleLifecycleSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(lifecycle.generation, snapshot.generation);
    assert_eq!(lifecycle.tick, snapshot.tick);
    assert_eq!(lifecycle.root_active, 0);
    assert_eq!(lifecycle.construction_present, 0);
    assert_eq!(lifecycle.terrain_mutation_sequence, 0);

    let mut count = 0;
    assert_eq!(
        mc_native_terrain_plane_length(handle, MC_TERRAIN_TYPES_U8, &raw mut count),
        MC_STATUS_OK
    );
    assert_eq!(count, 65_536);
    let mut too_small = vec![0_u8; 1];
    let mut written = 99;
    assert_eq!(
        mc_native_terrain_plane_copy_u8(
            handle,
            MC_TERRAIN_TYPES_U8,
            too_small.as_mut_ptr(),
            1,
            &raw mut written,
        ),
        MC_STATUS_BUFFER_TOO_SMALL
    );
    assert_eq!(written, 0);
    let mut terrain = vec![0_u8; 65_536];
    assert_eq!(
        mc_native_terrain_plane_copy_u8(
            handle,
            MC_TERRAIN_TYPES_U8,
            terrain.as_mut_ptr(),
            65_536,
            &raw mut written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(written, 65_536);
    let mut entity_indices = vec![0_i16; 65_536];
    assert_eq!(
        mc_native_terrain_entity_indices_copy(
            handle,
            entity_indices.as_mut_ptr(),
            65_536,
            &raw mut written,
        ),
        MC_STATUS_OK
    );

    let mut entities = vec![McEntitySnapshotV2::default(); 1_000];
    assert_eq!(
        mc_native_entities_copy(
            handle,
            0,
            entities.as_mut_ptr(),
            1_000,
            struct_size::<McEntitySnapshotV2>(),
            &raw mut written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(written, 1_000);
    assert_eq!(entities[152].authority, MC_AUTHORITY_PLAYER_FLIGHT_LIVE);
    assert_eq!(entities[153].authority, MC_AUTHORITY_SPELL_ACTIVATION_LIVE);
    for index in LEVEL_ONE_GOAT_INDICES {
        assert_eq!(
            entities[usize::from(index)].authority,
            MC_AUTHORITY_GOAT_LIVE
        );
    }
    for index in 51_usize..=78 {
        assert_eq!(entities[index].authority, MC_AUTHORITY_VILLAGER_LIVE);
    }
    assert_eq!(
        entities
            .iter()
            .filter(|entity| entity.authority == MC_AUTHORITY_FROZEN_OBSERVED)
            .count(),
        74
    );
    let fire = entities[122];
    assert_eq!(
        (
            fire.authority,
            fire.active,
            fire.class_id,
            fire.model,
            fire.action,
            fire.phase,
            fire.has_parameter_record,
            fire.parameter_record_index,
        ),
        (
            MC_AUTHORITY_LEVEL1_MISSION_LIVE,
            1,
            10,
            1,
            1,
            123,
            1,
            LEVEL1_NEW_EVENT_ENTITY_PARAMETER_RECORD_INDEX,
        )
    );
    let emitter = entities[113];
    assert_eq!(
        (
            emitter.authority,
            emitter.active,
            emitter.class_id,
            emitter.model,
            emitter.action,
            emitter.phase,
            emitter.has_parameter_record,
            emitter.parameter_record_index,
        ),
        (
            MC_AUTHORITY_LEVEL1_MISSION_LIVE,
            1,
            10,
            60,
            65,
            114,
            1,
            LEVEL1_NEW_EVENT_ENTITY_PARAMETER_RECORD_INDEX,
        )
    );
    let smoke = entities[155];
    assert_eq!(
        (
            smoke.authority,
            smoke.active,
            smoke.class_id,
            smoke.model,
            smoke.action,
            smoke.phase,
            smoke.has_parameter_record,
            smoke.parameter_record_index,
        ),
        (
            MC_AUTHORITY_LEVEL1_MISSION_LIVE,
            1,
            10,
            14,
            14,
            156,
            1,
            LEVEL1_NEW_EVENT_ENTITY_PARAMETER_RECORD_INDEX,
        )
    );
    let child = entities[164];
    assert_eq!(
        (
            child.authority,
            child.active,
            child.class_id,
            child.model,
            child.action,
            child.phase,
            child.has_parameter_record,
            child.parameter_record_index,
        ),
        (
            MC_AUTHORITY_LEVEL1_MISSION_LIVE,
            1,
            10,
            0,
            0,
            165,
            1,
            LEVEL1_NEW_EVENT_ENTITY_PARAMETER_RECORD_INDEX,
        )
    );

    with_session_mut(handle, |session| {
        session.mission.begin_source_turn_at(session.state.position);
        session.mission.dispatch_source_slot(
            &session.terrain,
            &session.state,
            113,
            false,
            false,
            None,
        );
        assert_eq!(
            session.world.entities()[178].parameter_relation,
            None,
            "newly allocated ambient slot has no frozen/static entity-parameter relation"
        );
        let dynamic_smoke = session
            .mission
            .ambient_entities()
            .find(|ambient| ambient.source_entity_index == 178)
            .copied()
            .expect("emitter 113 allocates smoke slot 178");
        assert_eq!(dynamic_smoke.render_parameter_index, 9);
        Ok(())
    })
    .expect("source-turn emitter dispatch");
    assert_eq!(
        mc_native_entities_copy(
            handle,
            0,
            entities.as_mut_ptr(),
            1_000,
            struct_size::<McEntitySnapshotV2>(),
            &raw mut written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(written, 1_000);
    let dynamic_smoke = entities[178];
    assert_eq!(
        (
            dynamic_smoke.authority,
            dynamic_smoke.active,
            dynamic_smoke.class_id,
            dynamic_smoke.model,
            dynamic_smoke.action,
            dynamic_smoke.has_parameter_record,
            dynamic_smoke.parameter_record_index,
        ),
        (
            MC_AUTHORITY_LEVEL1_MISSION_LIVE,
            1,
            10,
            14,
            14,
            1,
            LEVEL1_NEW_EVENT_ENTITY_PARAMETER_RECORD_INDEX,
        )
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn goat_bulk_hash_damage_lifecycle_reset_and_session_isolation_are_atomic() {
    fn session_snapshot(handle: u64) -> McSessionSnapshotV2 {
        let mut snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_snapshot(
                handle,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        snapshot
    }

    fn goats(handle: u64, generation: u64) -> (Vec<McGoatSnapshotV2>, McGoatBatchV2) {
        let mut rows = vec![McGoatSnapshotV2::default(); LEVEL_ONE_GOAT_INDICES.len()];
        let mut batch = McGoatBatchV2::default();
        assert_eq!(
            mc_native_goats_copy(
                handle,
                generation,
                0,
                rows.as_mut_ptr(),
                u32::try_from(rows.len()).unwrap(),
                struct_size::<McGoatSnapshotV2>(),
                &raw mut batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(batch.written, 33);
        assert_eq!(batch.total_count, 33);
        assert_eq!(batch.has_more, 0);
        (rows, batch)
    }

    let first = open_fixture();
    let second = open_fixture();
    let rng_only = open_fixture();
    let first_session = session_snapshot(first);
    let second_session = session_snapshot(second);
    let rng_only_session = session_snapshot(rng_only);
    let (reset_rows, reset_batch) = goats(first, first_session.generation);
    let (second_rows, second_batch) = goats(second, second_session.generation);
    let (rng_only_reset_rows, rng_only_reset_batch) = goats(rng_only, rng_only_session.generation);
    assert_eq!(reset_rows, second_rows);
    assert_eq!(reset_batch.live_world_hash, second_batch.live_world_hash);
    // GOLDEN-CHANGE: generic imported class-11 root callback state is
    // reset/hash-bound alongside the existing Level 2 campaign state.
    assert_eq!(
        reset_batch.live_world_hash, 0xe029_6244_e51f_b914,
        "C ABI publishes the explicit v2 reset hash"
    );
    assert_eq!(rng_only_reset_rows, reset_rows);
    assert_eq!(
        rng_only_reset_batch.live_world_hash,
        reset_batch.live_world_hash
    );
    assert_eq!(reset_batch.tick, 0);
    assert_eq!(reset_rows[0].entity_index, 50);
    assert_eq!(reset_rows[0].authority, MC_AUTHORITY_GOAT_LIVE);
    assert_eq!(reset_rows[0].render_parameter_index, 238);
    assert_eq!(reset_rows[0].base_tmap, 152);
    assert_eq!(reset_rows[0].life, 600);
    assert_eq!(reset_rows[0].max_life, 600);

    with_session_mut(rng_only, |session| {
        assert_eq!(session.mission.global_rng(), 0xad6e_67d9);
        session.mission.begin_source_turn();
        assert_eq!(session.mission.global_rng(), 0x9909_f858);
        Ok(())
    })
    .expect("production UpdateEntities turn-start RNG draw");
    let (rng_only_rows, rng_only_batch) = goats(rng_only, rng_only_session.generation);
    assert_eq!(
        rng_only_rows, rng_only_reset_rows,
        "mission RNG changes while every C ABI Goat row remains identical"
    );
    assert_eq!(rng_only_batch.tick, rng_only_reset_batch.tick);
    assert_ne!(
        rng_only_batch.live_world_hash, rng_only_reset_batch.live_world_hash,
        "C ABI live_world_hash must admit an RNG-only mutation"
    );
    let mut rng_only_restored = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_reset(
            rng_only,
            &raw mut rng_only_restored,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let (restored_rng_rows, restored_rng_batch) = goats(rng_only, rng_only_restored.generation);
    assert_eq!(
        restored_rng_rows, second_rows,
        "reset restores every row after an RNG-only mutation"
    );
    assert_eq!(restored_rng_batch.tick, second_batch.tick);
    assert_eq!(
        restored_rng_batch.live_world_hash, second_batch.live_world_hash,
        "reset restores the v2 RNG composite to an untouched session"
    );
    assert_eq!(mc_native_session_destroy(rng_only), MC_STATUS_OK);

    let mut stepped = McSessionSnapshotV2::default();
    let idle_command = valid_command();
    assert_eq!(
        mc_native_session_step(
            first,
            &raw const idle_command,
            &raw mut stepped,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let (_, advanced_batch) = goats(first, stepped.generation);
    let (_, untouched_batch) = goats(second, second_session.generation);
    assert_eq!(advanced_batch.tick, 1);
    assert_ne!(advanced_batch.live_world_hash, reset_batch.live_world_hash);
    assert_eq!(untouched_batch.live_world_hash, reset_batch.live_world_hash);

    let damage = McGoatDamageCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McGoatDamageCommandV2>(),
        generation: stepped.generation,
        entity_index: 50,
        reserved_0: 0,
        damage: 601,
        reserved: [0; 4],
    };
    let mut damage_batch = McGoatBatchV2::default();
    let mut invalid_damage = damage;
    invalid_damage.reserved[0] = 1;
    assert_eq!(
        mc_native_session_goat_inject_damage(
            first,
            &raw const invalid_damage,
            &raw mut damage_batch,
            struct_size::<McGoatBatchV2>(),
        ),
        MC_STATUS_RESERVED_NONZERO
    );
    assert_eq!(
        goats(first, stepped.generation).1.live_world_hash,
        advanced_batch.live_world_hash
    );
    invalid_damage = damage;
    invalid_damage.reserved_0 = 1;
    assert_eq!(
        mc_native_session_goat_inject_damage(
            first,
            &raw const invalid_damage,
            &raw mut damage_batch,
            struct_size::<McGoatBatchV2>(),
        ),
        MC_STATUS_RESERVED_NONZERO
    );
    assert_eq!(
        goats(first, stepped.generation).1.live_world_hash,
        advanced_batch.live_world_hash
    );
    invalid_damage = damage;
    invalid_damage.damage = 0;
    assert_eq!(
        mc_native_session_goat_inject_damage(
            first,
            &raw const invalid_damage,
            &raw mut damage_batch,
            struct_size::<McGoatBatchV2>(),
        ),
        MC_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(
        goats(first, stepped.generation).1.live_world_hash,
        advanced_batch.live_world_hash
    );
    invalid_damage = damage;
    invalid_damage.entity_index = 999;
    assert_eq!(
        mc_native_session_goat_inject_damage(
            first,
            &raw const invalid_damage,
            &raw mut damage_batch,
            struct_size::<McGoatBatchV2>(),
        ),
        MC_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(
        goats(first, stepped.generation).1.live_world_hash,
        advanced_batch.live_world_hash
    );
    invalid_damage = damage;
    invalid_damage.generation = invalid_damage.generation.wrapping_add(1);
    assert_eq!(
        mc_native_session_goat_inject_damage(
            first,
            &raw const invalid_damage,
            &raw mut damage_batch,
            struct_size::<McGoatBatchV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(
        goats(first, stepped.generation).1.live_world_hash,
        advanced_batch.live_world_hash
    );
    assert_eq!(
        mc_native_session_goat_inject_damage(
            first,
            &raw const damage,
            &raw mut damage_batch,
            struct_size::<McGoatBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_ne!(damage_batch.live_world_hash, advanced_batch.live_world_hash);

    for _ in 0..12 {
        assert_eq!(
            mc_native_session_step(
                first,
                &raw const idle_command,
                &raw mut stepped,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let (rows, _) = goats(first, stepped.generation);
        if rows[0].active == 0 {
            break;
        }
        if rows[0].action == 12 {
            let kill_credit = with_session(first, |session| {
                Ok(session.live_world.goats()[0].kill_credit_entity)
            })
            .unwrap();
            assert_eq!(
                kill_credit,
                i16::try_from(stepped.entity_index).unwrap(),
                "qualification damage is always attributed to the local player"
            );
        }
    }
    let (dead_rows, _) = goats(first, stepped.generation);
    assert!(
        dead_rows[0].active == 0,
        "Goat must reach next-pass reclamation"
    );

    let query = McEventQueryV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McEventQueryV2>(),
        generation: stepped.generation,
        cursor: 0,
        reserved: [0; 4],
    };
    let mut events = vec![McEventV2::default(); 256];
    let mut event_batch = McEventBatchV2::default();
    assert_eq!(
        mc_native_events_copy_since(
            first,
            &raw const query,
            events.as_mut_ptr(),
            u32::try_from(events.len()).unwrap(),
            struct_size::<McEventV2>(),
            &raw mut event_batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_OK
    );
    events.truncate(usize::try_from(event_batch.written).unwrap());
    assert!(events.iter().any(|event| {
        event.kind == MC_EVENT_GOAT_MANA_CONVERTED
            && event.entity_index == 50
            && event.value_0 == 300
    }));
    assert!(
        events.iter().any(|event| {
            event.kind == MC_EVENT_GOAT_REMOVAL_QUEUED && event.entity_index == 50
        })
    );
    assert!(
        events
            .iter()
            .any(|event| { event.kind == MC_EVENT_GOAT_RECLAIMED && event.entity_index == 50 })
    );

    let mut reset_snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_reset(
            first,
            &raw mut reset_snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let mut rejected_batch = McGoatBatchV2::default();
    assert_eq!(
        mc_native_goats_copy(
            first,
            stepped.generation,
            0,
            std::ptr::null_mut(),
            0,
            struct_size::<McGoatSnapshotV2>(),
            &raw mut rejected_batch,
            struct_size::<McGoatBatchV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    let (restored_rows, restored_batch) = goats(first, reset_snapshot.generation);
    assert_eq!(restored_rows, reset_rows);
    assert_eq!(restored_batch.live_world_hash, reset_batch.live_world_hash);
    assert_eq!(restored_batch.tick, 0);

    assert_eq!(mc_native_session_destroy(first), MC_STATUS_OK);
    assert_eq!(mc_native_session_destroy(second), MC_STATUS_OK);
}

#[test]
fn selector_frames_latch_advance_freeze_reset_and_isolate_exactly() {
    fn snapshot(handle: u64) -> McSessionSnapshotV2 {
        let mut snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_snapshot(
                handle,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        snapshot
    }

    fn frames(
        handle: u64,
        generation: u64,
        tick: u64,
    ) -> ([McSelectorFrameStateV2; 3], McSelectorFrameBatchV2) {
        let mut rows = [McSelectorFrameStateV2::default(); 3];
        let mut batch = McSelectorFrameBatchV2::default();
        assert_eq!(
            mc_native_selector_frame_states_copy(
                handle,
                generation,
                tick,
                0,
                rows.as_mut_ptr(),
                3,
                struct_size::<McSelectorFrameStateV2>(),
                &raw mut batch,
                struct_size::<McSelectorFrameBatchV2>(),
            ),
            MC_STATUS_OK
        );
        (rows, batch)
    }

    fn submit(handle: u64, generation: u64, tick: u64, mask: u64) -> (i32, McSelectorFrameBatchV2) {
        let command = McSelectorDrawFeedbackV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McSelectorDrawFeedbackV2>(),
            generation,
            draw_tick: tick,
            selector_mask: mask,
            reserved: [0; 4],
        };
        let mut batch = McSelectorFrameBatchV2::default();
        let status = mc_native_session_selector_draw_feedback_submit(
            handle,
            &raw const command,
            &raw mut batch,
            struct_size::<McSelectorFrameBatchV2>(),
        );
        (status, batch)
    }

    let first = open_fixture_with_owner_cache();
    let second = open_fixture_with_owner_cache();
    let first_snapshot = snapshot(first);
    let second_snapshot = snapshot(second);
    let (reset_rows, reset_batch) = frames(first, first_snapshot.generation, first_snapshot.tick);
    let (second_rows, second_batch) =
        frames(second, second_snapshot.generation, second_snapshot.tick);
    assert_eq!(reset_rows, second_rows);
    assert_eq!(
        reset_batch.selector_state_hash,
        second_batch.selector_state_hash
    );
    assert_eq!(reset_rows.map(|row| row.selector), GOAT_TMAP_SELECTORS);
    assert_eq!(reset_rows.map(|row| row.source_frame_index), [1, 1, 1]);
    assert_eq!(reset_rows.map(|row| row.stream_cursor), [490, 710, 490]);
    assert_eq!(reset_rows.map(|row| row.pixel_bytes), [484, 704, 484]);
    assert!(
        reset_rows
            .iter()
            .all(|row| row.flags & MC_SELECTOR_FRAME_USED_BY_PRECEDING_DRAW == 0)
    );
    assert_eq!(reset_batch.valid_draw_mask, 7);
    assert_eq!(reset_batch.has_draw_feedback, 0);

    let (status, submitted) = submit(first, first_snapshot.generation, 0, 1);
    assert_eq!(status, MC_STATUS_OK);
    assert_eq!(submitted.last_draw_tick, 0);
    assert_eq!(submitted.last_draw_mask, 1);
    assert_eq!(submitted.has_draw_feedback, 1);
    let (latched_rows, latched_batch) = frames(first, first_snapshot.generation, 0);
    assert_ne!(
        latched_batch.selector_state_hash,
        reset_batch.selector_state_hash
    );
    assert_ne!(
        latched_rows[0].flags & MC_SELECTOR_FRAME_USED_BY_PRECEDING_DRAW,
        0
    );
    assert_eq!(
        latched_rows[1].flags & MC_SELECTOR_FRAME_USED_BY_PRECEDING_DRAW,
        0
    );

    let (status, duplicate) = submit(first, first_snapshot.generation, 0, 1);
    assert_eq!(status, MC_STATUS_OK);
    assert_eq!(
        duplicate.selector_state_hash,
        latched_batch.selector_state_hash
    );
    let (status, combined) = submit(first, first_snapshot.generation, 0, 2);
    assert_eq!(status, MC_STATUS_OK);
    assert_eq!(combined.last_draw_mask, 3);
    let (combined_rows, combined_batch) = frames(first, first_snapshot.generation, 0);
    assert_ne!(
        combined_rows[1].flags & MC_SELECTOR_FRAME_USED_BY_PRECEDING_DRAW,
        0
    );
    let (status, _) = submit(first, first_snapshot.generation, 0, 8);
    assert_eq!(status, MC_STATUS_INVALID_ARGUMENT);
    assert_eq!(
        frames(first, first_snapshot.generation, 0)
            .1
            .selector_state_hash,
        combined_batch.selector_state_hash
    );

    let mut stepped = McSessionSnapshotV2::default();
    let idle = valid_command();
    assert_eq!(
        mc_native_session_step(
            first,
            &raw const idle,
            &raw mut stepped,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let (advanced_rows, advanced_batch) = frames(first, stepped.generation, stepped.tick);
    assert_eq!(advanced_rows.map(|row| row.source_frame_index), [2, 2, 1]);
    assert_eq!(advanced_rows.map(|row| row.stream_cursor), [818, 1196, 490]);
    assert!(
        advanced_rows
            .iter()
            .all(|row| row.flags & MC_SELECTOR_FRAME_USED_BY_PRECEDING_DRAW == 0)
    );
    assert_ne!(
        advanced_batch.selector_state_hash,
        latched_batch.selector_state_hash
    );
    assert_eq!(
        submit(first, stepped.generation, stepped.tick - 1, 4).0,
        MC_STATUS_GENERATION_MISMATCH,
        "feedback binds the completed session/Goat tick, not the lagging camera-history tick"
    );
    assert_eq!(
        frames(first, stepped.generation, stepped.tick)
            .1
            .selector_state_hash,
        advanced_batch.selector_state_hash
    );
    assert_eq!(
        frames(second, second_snapshot.generation, second_snapshot.tick).0,
        reset_rows
    );

    assert_eq!(
        mc_native_session_step(
            first,
            &raw const idle,
            &raw mut stepped,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let (frozen_rows, _) = frames(first, stepped.generation, stepped.tick);
    assert_eq!(
        frozen_rows.map(|row| row.source_frame_index),
        [2, 2, 1],
        "no feedback freezes every selector"
    );

    let (status, zero) = submit(first, stepped.generation, stepped.tick, 0);
    assert_eq!(status, MC_STATUS_OK);
    assert_eq!(zero.last_draw_mask, 0);
    let (status, accumulated) = submit(first, stepped.generation, stepped.tick, 4);
    assert_eq!(status, MC_STATUS_OK);
    assert_eq!(accumulated.last_draw_mask, 4);
    assert_eq!(
        mc_native_session_step(
            first,
            &raw const idle,
            &raw mut stepped,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        frames(first, stepped.generation, stepped.tick)
            .0
            .map(|row| row.source_frame_index),
        [2, 2, 2],
        "zero then a later same-tick draw accumulates the new selector"
    );

    let (status, zero_only) = submit(first, stepped.generation, stepped.tick, 0);
    assert_eq!(status, MC_STATUS_OK);
    assert_eq!(zero_only.last_draw_mask, 0);
    assert_eq!(
        mc_native_session_step(
            first,
            &raw const idle,
            &raw mut stepped,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        frames(first, stepped.generation, stepped.tick)
            .0
            .map(|row| row.source_frame_index),
        [2, 2, 2],
        "an explicit zero-only feedback epoch is frozen"
    );

    let stale_generation = stepped.generation;
    let mut reset_snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_reset(
            first,
            &raw mut reset_snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_ne!(reset_snapshot.generation, stale_generation);
    let (restored_rows, restored_batch) =
        frames(first, reset_snapshot.generation, reset_snapshot.tick);
    assert_eq!(restored_rows, reset_rows);
    assert_eq!(
        restored_batch.selector_state_hash,
        reset_batch.selector_state_hash
    );
    assert_eq!(
        submit(first, stale_generation, stepped.tick, 1).0,
        MC_STATUS_GENERATION_MISMATCH
    );

    assert_eq!(
        submit(first, reset_snapshot.generation, reset_snapshot.tick, 1).0,
        MC_STATUS_OK
    );
    assert_eq!(
        submit(first, reset_snapshot.generation, reset_snapshot.tick, 2).0,
        MC_STATUS_OK
    );
    let mut replay_snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            first,
            &raw const idle,
            &raw mut replay_snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let (replay_rows, replay_batch) =
        frames(first, replay_snapshot.generation, replay_snapshot.tick);
    assert_eq!(replay_rows, advanced_rows);
    assert_eq!(
        replay_batch.selector_state_hash, advanced_batch.selector_state_hash,
        "reset plus identical feedback/step replay restores the full selector state"
    );
    assert_eq!(mc_native_session_destroy(first), MC_STATUS_OK);
    assert_eq!(mc_native_session_destroy(second), MC_STATUS_OK);
}

#[test]
fn native_level_one_palette_is_atomic_hashed_and_owner_cache_backed() {
    let handle = open_fixture_with_world_frame_assets();
    let mut rgb8 = [0_u8; 768];
    let mut info = McLevel1PaletteInfoV2::default();
    assert_eq!(
        mc_native_level1_palette_copy(
            handle,
            rgb8.as_mut_ptr(),
            rgb8.len() as u32,
            &raw mut info,
            struct_size::<McLevel1PaletteInfoV2>(),
        ),
        MC_STATUS_OK
    );
    let expected = fs::read(presentation_test_cache().join("palette/paln.rgb8")).unwrap();
    assert_eq!(rgb8.as_slice(), expected);
    assert_eq!(info.rgb8_sha256.as_slice(), Sha256::digest(rgb8).as_slice());
    assert_eq!(
        (
            info.submode,
            info.effect,
            info.weight,
            info.denominator,
            info.palette_count,
            info.flags,
        ),
        (0, 0, 0, 0, 0, 1)
    );
    assert_eq!(info.reserved, [0; 4]);
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn qualified_cpu_goat_draw_admission_is_authoritative_idempotent_and_tick_bound() {
    let handle = open_fixture_with_owner_cache();
    let mut reset = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_snapshot(handle, &raw mut reset, struct_size::<McSessionSnapshotV2>(),),
        MC_STATUS_OK
    );
    let mut receipt = McSelectorFrameBatchV2::default();
    assert_eq!(
        mc_native_session_goat_draw_admission_evaluate(
            handle,
            reset.generation,
            reset.tick,
            &raw mut receipt,
            struct_size::<McSelectorFrameBatchV2>(),
        ),
        MC_STATUS_OK
    );
    let exact_mask = with_session(handle, |session| {
        Ok(session
            .goat_draw_admission
            .as_ref()
            .expect("evaluation stores diagnostic rows")
            .selector_mask)
    })
    .unwrap();
    assert_eq!(receipt.has_draw_feedback, 1);
    assert_eq!(receipt.last_draw_tick, reset.tick);
    assert_eq!(receipt.last_draw_mask, u64::from(exact_mask));
    assert_eq!(receipt.valid_draw_mask, u64::from(GOAT_TMAP_DRAW_MASK));

    let first_hash = receipt.selector_state_hash;
    assert_eq!(
        mc_native_session_goat_draw_admission_evaluate(
            handle,
            reset.generation,
            reset.tick,
            &raw mut receipt,
            struct_size::<McSelectorFrameBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(receipt.selector_state_hash, first_hash);
    assert_eq!(receipt.last_draw_mask, u64::from(exact_mask));

    let idle = valid_command();
    let mut stepped = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const idle,
            &raw mut stepped,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let mut rows = [McSelectorFrameStateV2::default(); 3];
    assert_eq!(
        mc_native_selector_frame_states_copy(
            handle,
            stepped.generation,
            stepped.tick,
            0,
            rows.as_mut_ptr(),
            3,
            struct_size::<McSelectorFrameStateV2>(),
            &raw mut receipt,
            struct_size::<McSelectorFrameBatchV2>(),
        ),
        MC_STATUS_OK
    );
    for (ordinal, row) in rows.iter().enumerate() {
        let was_admitted = exact_mask & (1 << ordinal) != 0;
        assert_eq!(row.source_frame_index, if was_admitted { 2 } else { 1 });
        assert_eq!(
            row.flags & MC_SELECTOR_FRAME_USED_BY_PRECEDING_DRAW,
            0,
            "the preceding CPU draw is serviced exactly once"
        );
    }
    assert_eq!(
        mc_native_session_goat_draw_admission_evaluate(
            handle,
            stepped.generation,
            stepped.tick - 1,
            &raw mut receipt,
            struct_size::<McSelectorFrameBatchV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);

    let ownerless = open_fixture();
    let mut ownerless_snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_snapshot(
            ownerless,
            &raw mut ownerless_snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_session_goat_draw_admission_evaluate(
            ownerless,
            ownerless_snapshot.generation,
            ownerless_snapshot.tick,
            &raw mut receipt,
            struct_size::<McSelectorFrameBatchV2>(),
        ),
        MC_STATUS_VALIDATION_FAILED
    );
    assert_eq!(mc_native_session_destroy(ownerless), MC_STATUS_OK);
}

#[test]
fn objective_catalog_is_complete_frozen_and_bounds_checked() {
    let handle = open_fixture();
    let mut count = 0;
    assert_eq!(
        mc_native_objective_count(handle, &raw mut count),
        MC_STATUS_OK
    );
    assert_eq!(count, 8);

    let mut objectives = [McObjectiveSnapshotV2::default(); 8];
    let mut written = 0;
    assert_eq!(
        mc_native_objectives_copy(
            handle,
            0,
            objectives.as_mut_ptr(),
            8,
            struct_size::<McObjectiveSnapshotV2>(),
            &raw mut written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(written, 8);
    for (slot, objective) in objectives.iter().enumerate() {
        assert_eq!(usize::from(objective.slot), slot);
        assert_eq!(objective.authority, MC_AUTHORITY_FROZEN_OBSERVED);
        assert_eq!(objective.abi_version, MC_NATIVE_ABI_VERSION);
        assert_eq!(
            objective.struct_size,
            struct_size::<McObjectiveSnapshotV2>()
        );
    }
    assert_eq!(
        (
            objectives[0].is_level_end,
            objectives[0].text_selector,
            objectives[0].done,
        ),
        (0, 0, 0)
    );
    assert_eq!(objectives[0].stage_selectors, [1, 1, 1, 1, 1, 0, 0, 0]);

    assert_eq!(
        mc_native_objectives_copy(
            handle,
            0,
            std::ptr::null_mut(),
            0,
            struct_size::<McObjectiveSnapshotV2>() - 1,
            &raw mut written,
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    assert_eq!(
        mc_native_objectives_copy(
            handle,
            0,
            std::ptr::null_mut(),
            0,
            struct_size::<McObjectiveSnapshotV2>() + 1,
            &raw mut written,
        ),
        MC_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(
        mc_native_objectives_copy(
            handle,
            9,
            std::ptr::null_mut(),
            0,
            struct_size::<McObjectiveSnapshotV2>(),
            &raw mut written,
        ),
        MC_STATUS_BOUNDS
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn two_sessions_are_independent_and_repeat_deterministically() {
    let first = open_fixture();
    let second = open_fixture();
    assert_ne!(first, second);
    let mut first_terrain = McTerrainStateV2::default();
    let mut second_terrain = McTerrainStateV2::default();
    assert_eq!(
        mc_native_session_terrain_state(
            first,
            &raw mut first_terrain,
            struct_size::<McTerrainStateV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_session_terrain_state(
            second,
            &raw mut second_terrain,
            struct_size::<McTerrainStateV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(first_terrain.terrain_hash, second_terrain.terrain_hash);
    assert_eq!(first_terrain.mutation_sequence, 0);
    assert_eq!(second_terrain.mutation_sequence, 0);
    let mut command = valid_command();
    command.flags = 1;
    let mut first_snapshot = McSessionSnapshotV2::default();
    let mut second_snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            first,
            &raw const command,
            &raw mut first_snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_session_snapshot(
            second,
            &raw mut second_snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_ne!(first_snapshot.state_hash, second_snapshot.state_hash);
    let mut player_entity = McEntitySnapshotV2::default();
    let mut written = 0;
    assert_eq!(
        mc_native_entities_copy(
            first,
            u32::from(first_snapshot.entity_index),
            &raw mut player_entity,
            1,
            struct_size::<McEntitySnapshotV2>(),
            &raw mut written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(written, 1);
    assert_eq!(
        (player_entity.x, player_entity.y, player_entity.z),
        (first_snapshot.x, first_snapshot.y, first_snapshot.z)
    );
    assert_eq!(player_entity.yaw, first_snapshot.yaw);
    assert_eq!(player_entity.pitch, first_snapshot.display_pitch);
    assert_eq!(player_entity.actual_speed, first_snapshot.actual_speed);
    assert_eq!(player_entity.phase, first_snapshot.phase);
    let mut reset = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_reset(first, &raw mut reset, struct_size::<McSessionSnapshotV2>(),),
        MC_STATUS_OK
    );
    assert_eq!(reset.state_hash, second_snapshot.state_hash);
    assert_ne!(reset.generation, second_snapshot.generation);
    let mut reset_terrain = McTerrainStateV2::default();
    assert_eq!(
        mc_native_session_terrain_state(
            first,
            &raw mut reset_terrain,
            struct_size::<McTerrainStateV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(reset_terrain.generation, reset.generation);
    assert_eq!(reset_terrain.mutation_sequence, 0);
    assert_eq!(reset_terrain.terrain_hash, second_terrain.terrain_hash);
    assert_eq!(mc_native_session_destroy(first), MC_STATUS_OK);
    assert_eq!(mc_native_session_destroy(second), MC_STATUS_OK);
}

#[test]
fn source_projection_hash_has_complete_preimage_without_action_51_terrain_and_normalizes_tick() {
    let handle = open_fixture();
    let command = valid_command();
    let mut snapshot = McSessionSnapshotV2::default();
    for _ in 0..300 {
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
    }
    let mut projected_hash = 0_u64;
    assert_eq!(
        mc_native_session_source_projection_hash(handle, &raw mut projected_hash),
        MC_STATUS_OK
    );
    with_session(handle, |session| {
        assert_eq!(
            session.state,
            FlightState {
                entity_index: 152,
                position: Position {
                    x: 19_840,
                    y: 56_960,
                    z: 5_024,
                },
                yaw: 0,
                display_pitch: 0,
                effective_pitch: 0,
                roll_accumulator: 0,
                pitch_accumulator: 0,
                target_speed: 0,
                speed_changed_flag: 0,
                actual_speed: 0,
                strafe_speed: 0,
                roll_delta: 0,
                pitch_delta: 0,
                recoil_speed: 0,
                recoil_yaw: 0,
                impulse_x: 0,
                impulse_y: 0,
                impulse_z: 0,
                entity_fov: 0,
                water_counter: 0,
                speed_modifier: 0,
                speed_modifier_counter: 0,
                immobilize_counter: 0,
                immobilize_subcounter: 0,
                tether_prior_distance: 0,
                tether_target_index: 0,
                tether_holder_index: 0,
                tether_tier: 0,
                speed_spell_holder_index: 0,
                entity_phase: 197,
                ambient_loop_5_distance: 2_048,
                ambient_loop_31_distance: 1_746,
                player_color_index: 0,
                music_countdown: 0,
                recovery_latch: false,
                skip_flight_once: false,
                tick: 300,
            },
            "all 39 fixed-width state-hash fields have an explicit preimage"
        );
        assert_eq!(
            (
                session.terrain.deterministic_hash(),
                session.terrain.mutation_sequence(),
            ),
            (0x5acb_822a_a12a_962f, 2),
            "the paired terrain generation is independently identified"
        );
        assert!(session.mission.house_construction_effects().is_empty());
        assert!(session.pending_house_effects.iter().all(|effect| !matches!(
            effect,
            HouseStepEvent::ConstructionPass { .. } | HouseStepEvent::ConstructionCompleted { .. }
        )));
        Ok(())
    })
    .expect("complete 300-turn hash preimage");
    assert_eq!(
        (snapshot.state_hash, projected_hash),
        (0x5fec_9398_db9c_e681, 0x372c_5311_579c_f80a)
    );
    with_session(handle, |session| {
        let shifted = FlightState {
            tick: session.state.tick.wrapping_add(10_000),
            ..session.state
        };
        assert_ne!(state_hash(&shifted), snapshot.state_hash);
        assert_eq!(source_projection_hash(&shifted), projected_hash);
        Ok(())
    })
    .expect("source-projection tick normalization");
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn source_arena_hash_replays_resets_and_isolates_sessions() {
    let first = open_fixture();
    let second = open_fixture();
    let mut first_initial = 0_u64;
    let mut second_initial = 0_u64;
    assert_eq!(
        mc_native_session_source_arena_hash(first, &raw mut first_initial),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_session_source_arena_hash(second, &raw mut second_initial),
        MC_STATUS_OK
    );
    assert_eq!(first_initial, second_initial);
    assert_ne!(first_initial, 0);

    let command = valid_command();
    let mut snapshot = McSessionSnapshotV2::default();
    for _ in 0..8 {
        assert_eq!(
            mc_native_session_step(
                first,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_session_step(
                second,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
    }
    let mut first_replay = 0_u64;
    let mut second_replay = 0_u64;
    assert_eq!(
        mc_native_session_source_arena_hash(first, &raw mut first_replay),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_session_source_arena_hash(second, &raw mut second_replay),
        MC_STATUS_OK
    );
    assert_eq!(first_replay, second_replay);
    assert_ne!(first_replay, first_initial);

    assert_eq!(
        mc_native_session_step(
            first,
            &raw const command,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let mut first_diverged = 0_u64;
    assert_eq!(
        mc_native_session_source_arena_hash(first, &raw mut first_diverged),
        MC_STATUS_OK
    );
    assert_ne!(first_diverged, second_replay);

    assert_eq!(
        mc_native_session_reset(
            first,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let mut first_reset = 0_u64;
    assert_eq!(
        mc_native_session_source_arena_hash(first, &raw mut first_reset),
        MC_STATUS_OK
    );
    assert_eq!(first_reset, first_initial);
    assert_eq!(mc_native_session_destroy(first), MC_STATUS_OK);
    assert_eq!(mc_native_session_destroy(second), MC_STATUS_OK);
}

#[test]
fn direct_core_and_capi_source_dispatch_remain_in_lockstep() {
    const DISTANCE_WINNER_C_PUBLIC_AIM: (i16, i16) = (0, 0);
    const NUMERIC_FIRST_A_PUBLIC_AIM: (i16, i16) = (16, 0);

    let (package, closure) = fixture_paths();
    let world = NativeWorld::load(package, closure).expect("package M loads");
    let mut direct = world.session();
    let handle = open_fixture();
    let command = valid_command();
    let mut snapshot = McSessionSnapshotV2::default();

    // The native host supplies one complete typed turn-start snapshot to
    // every source dispatch. Make all relevant orders disagree:
    // numeric A/B/C/D, reversed host D/C/B/A, model-major B/A/C/D, nearest
    // C, plus a still-nearer same-turn insertion E that must remain absent
    // from the frozen guidance lists until the next rebuild.
    let exercise_targeting_vector =
        |mission: &mut Level1MissionState,
         state: &mut FlightState,
         terrain: &SessionTerrain,
         live_world: &LiveWorldState| {
            state.position = Position {
                x: 40_000,
                y: 40_000,
                z: 2_000,
            };
            state.yaw = 0;
            state.effective_pitch = 0;
            state.actual_speed = 0;
            mission.has_fireball = true;
            mission.player_mana = 10_000;
            let origin = state.position;
            let spawn_resident = |mission: &mut Level1MissionState,
                                  model,
                                  action,
                                  x_offset: u16,
                                  y_distance: u16,
                                  z_offset: i16| {
                mission
                    .spawn_house_resident(
                        Position {
                            x: origin.x.wrapping_add(x_offset),
                            y: origin.y.wrapping_sub(y_distance),
                            z: origin.z.wrapping_add(z_offset),
                        },
                        model,
                        action,
                        0,
                    )
                    .expect("qualified resident family")
                    .expect("resident source slot")
            };
            let numeric_first = spawn_resident(mission, 12, 97, 64, 1_000, 100);
            let model_first = spawn_resident(mission, 4, 33, 128, 1_000, 164);
            let distance_first = spawn_resident(mission, 12, 97, 0, 512, 100);
            let host_first = spawn_resident(mission, 13, 105, 256, 1_000, 36);
            assert!(
                numeric_first < model_first
                    && model_first < distance_first
                    && distance_first < host_first
            );

            mission.begin_source_turn_at(origin);
            let mut turn_start = level1_turn_start_source_views(mission, state, live_world);
            turn_start.reverse();
            mission.dispatch_source_slot_with_views_and_look(
                terrain,
                state,
                &turn_start,
                0,
                false,
                false,
                None,
                0,
                0,
            );

            let same_turn_insertion = spawn_resident(mission, 4, 33, 100, 283, 100);
            let mut later_views = level1_turn_start_source_views(mission, state, live_world);
            later_views.reverse();
            assert!(mission.spawn_fireball_with_look(state, 0, 0));
            let projectile_offset = mission.projectiles().len() - 1;
            let projectile_slot = mission.projectiles()[projectile_offset].source_entity_index();
            mission.dispatch_source_slot_with_views_and_look(
                terrain,
                state,
                &later_views,
                projectile_slot,
                false,
                false,
                None,
                0,
                0,
            );
            let Level1Projectile::PlayerFireball(projectile) =
                mission.projectiles()[projectile_offset]
            else {
                panic!("targeting vector spawns a player Fireball");
            };
            assert_eq!(
                projectile.target_entity_index, distance_first,
                "distance winner C replaces model-list head B, while same-turn insertion E remains frozen out"
            );
            (
                [
                    numeric_first,
                    model_first,
                    distance_first,
                    host_first,
                    same_turn_insertion,
                ],
                projectile_offset,
                projectile.source_entity_index,
                projectile.target_entity_index,
                projectile.yaw,
                projectile.pitch,
            )
        };
    let mut direct_vector = world.session();
    let vector_handle = open_fixture();
    let direct_outcome = exercise_targeting_vector(
        &mut direct_vector.mission,
        &mut direct_vector.state,
        &direct_vector.terrain,
        &direct_vector.live_world,
    );
    let capi_outcome = with_session_mut(vector_handle, |session| {
        Ok(exercise_targeting_vector(
            &mut session.mission,
            &mut session.state,
            &session.terrain,
            &session.live_world,
        ))
    })
    .expect("registered C API targeting-vector session");
    assert_eq!(direct_outcome, capi_outcome);
    assert_eq!(
        direct_outcome.3, direct_outcome.0[2],
        "direct core target identity proves that distance winner C defeated numeric A, model-list B, and reversed-host D"
    );
    let capi_vector_snapshot = with_session(vector_handle, |session| Ok(session.snapshot()))
        .expect("registered C API targeting-vector snapshot");
    let mut visible_projectile = McFireballProjectileSnapshotV2::default();
    let mut vector_written = 0;
    assert_eq!(
        mc_native_fireball_projectiles_copy(
            vector_handle,
            capi_vector_snapshot.generation,
            capi_vector_snapshot.tick,
            u32::try_from(capi_outcome.1).expect("projectile offset fits u32"),
            &raw mut visible_projectile,
            1,
            struct_size::<McFireballProjectileSnapshotV2>(),
            &raw mut vector_written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(vector_written, 1);
    assert_eq!(
        (visible_projectile.yaw, visible_projectile.pitch),
        DISTANCE_WINNER_C_PUBLIC_AIM,
        "C API projection independently exposes C's source-derived straight-ahead yaw/pitch"
    );
    assert_ne!(
        (visible_projectile.yaw, visible_projectile.pitch),
        NUMERIC_FIRST_A_PUBLIC_AIM,
        "candidate A's distinct in-cone yaw would hit the source first-turn +16 yaw limit"
    );
    assert_eq!(
        visible_projectile.reserved[0],
        u32::from(capi_outcome.2),
        "reserved[0] is the projectile source slot, not the selected target"
    );
    assert_eq!(mc_native_session_destroy(vector_handle), MC_STATUS_OK);

    let reset_villagers = direct.live_world.villagers().to_vec();
    assert_eq!(
        reset_villagers
            .iter()
            .map(|villager| villager.entity_index)
            .collect::<Vec<_>>(),
        (51_u16..=78).collect::<Vec<_>>()
    );

    for turn in 1..=24 {
        direct.step(PlayerCommand::default());
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        with_session(handle, |capi| {
            assert_eq!(
                state_hash(&direct.state),
                state_hash(&capi.state),
                "flight state diverged on turn {turn}"
            );
            assert_eq!(
                direct.live_world_hash_v2(),
                capi.live_world_hash_v2(),
                "typed imported world diverged on turn {turn}"
            );
            assert_eq!(
                direct.mission.source_arena_hash_v1(),
                capi.mission.source_arena_hash_v1(),
                "source arena diverged on turn {turn}"
            );
            Ok(())
        })
        .expect("registered C API session");
    }
    assert!(
        direct
            .live_world
            .villagers()
            .iter()
            .zip(&reset_villagers)
            .all(|(live, reset)| {
                live.active
                    && live.phase == reset.phase.wrapping_add(24)
                    && live.position != reset.position
                    && direct.mission.source_slots()[usize::from(live.entity_index)].action
                        == live.action
            }),
        "all 28 frame-zero Villagers execute live action-111 bodies"
    );
    with_session(handle, |capi| {
        assert_eq!(
            direct.live_world.villagers(),
            capi.live_world.villagers(),
            "direct and C API Villager bodies diverged"
        );
        for villager in capi.live_world.villagers() {
            let projection = capi
                .frozen_actors
                .iter()
                .find(|actor| actor.source_entity_index == villager.entity_index)
                .expect("live Villager presentation projection");
            assert_eq!(
                (projection.position, projection.yaw),
                (villager.position, villager.yaw),
                "Villager presentation must follow live simulation"
            );
            assert!(capi.renderable_map_chains.tracks(villager.entity_index));
        }
        Ok(())
    })
    .expect("registered C API session");

    let lethal_damage = direct
        .live_world
        .villagers()
        .iter()
        .find(|villager| villager.entity_index == 51)
        .unwrap()
        .life
        + 1;
    direct
        .live_world
        .inject_damage(51, lethal_damage, 152)
        .expect("direct Villager lethal mailbox");
    with_session_mut(handle, |capi| {
        capi.live_world
            .inject_damage(51, lethal_damage, 152)
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
    })
    .expect("C API Villager lethal mailbox");
    let mut removed_turn = None;
    for death_turn in 1..=10 {
        direct.step(PlayerCommand::default());
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        with_session(handle, |capi| {
            assert_eq!(direct.live_world.villagers(), capi.live_world.villagers());
            assert_eq!(
                direct.mission.source_arena_hash_v1(),
                capi.mission.source_arena_hash_v1()
            );
            Ok(())
        })
        .expect("registered C API session");
        let villager = direct
            .live_world
            .villagers()
            .iter()
            .find(|villager| villager.entity_index == 51)
            .unwrap();
        if death_turn == 1 {
            assert_eq!(villager.action, 108);
            assert_eq!(villager.pending_damage, lethal_damage);
            assert_eq!(villager.pending_attacker, 0);
            assert_eq!(villager.kill_credit_entity, 152);
        }
        if !villager.active {
            removed_turn = Some(death_turn);
            break;
        }
    }
    assert_eq!(
        removed_turn,
        Some(2),
        "frame-zero reset_counter=2 takes action 108's immediate-disable branch"
    );
    assert!(
        direct
            .mission
            .death_bursts()
            .iter()
            .all(|burst| burst.parent_entity_index != 51),
        "the immediate-disable branch does not enter action 109"
    );
    assert!(direct.mission.imported_source_slot_is_live(51));
    with_session(handle, |capi| {
        let dead = capi
            .live_world
            .villagers()
            .iter()
            .find(|villager| villager.entity_index == 51)
            .unwrap();
        assert!(!dead.active);
        assert!(
            !capi
                .renderable_map_chains
                .entity_chain(
                    dead.position.x.to_be_bytes()[0],
                    dead.position.y.to_be_bytes()[0]
                )
                .contains(&51),
            "death unlinks the live Villager presentation immediately"
        );
        Ok(())
    })
    .expect("registered C API session");
    direct.step(PlayerCommand::default());
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert!(
        !direct.mission.imported_source_slot_is_live(51),
        "the next turn reclaims the disabled imported source slot"
    );
    with_session(handle, |capi| {
        assert!(!capi.mission.imported_source_slot_is_live(51));
        assert_eq!(direct.live_world_hash_v2(), capi.live_world_hash_v2());
        assert_eq!(
            direct.mission.source_arena_hash_v1(),
            capi.mission.source_arena_hash_v1()
        );
        Ok(())
    })
    .expect("registered C API session");
    let retired_projection = with_session(handle, |capi| {
        Ok(entity_snapshot(
            &capi.world.entities()[51],
            &capi.state,
            &capi.live_world,
            &capi.mission,
        ))
    })
    .expect("retired imported Villager projection");
    assert_eq!(
        (
            retired_projection.entity_index,
            retired_projection.authority,
            retired_projection.active,
        ),
        (51, MC_AUTHORITY_LEVEL1_MISSION_LIVE, 1),
        "an existing MissionLive payload owns the projection as soon as the imported row retires"
    );

    let resident_position = direct.live_world.houses()[0].position;
    let resident_slot = direct
        .mission
        .spawn_house_resident(resident_position, 4, 33, 0)
        .expect("direct resident constructor")
        .expect("direct source arena resident slot");
    let capi_resident_slot = with_session_mut(handle, |capi| {
        Ok(capi
            .mission
            .spawn_house_resident(resident_position, 4, 33, 0)
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?
            .expect("C API source arena resident slot"))
    })
    .expect("registered C API session");
    assert_eq!(resident_slot, capi_resident_slot);
    let replacement_projection = with_session(handle, |capi| {
        Ok(entity_snapshot(
            &capi.world.entities()[usize::from(capi_resident_slot)],
            &capi.state,
            &capi.live_world,
            &capi.mission,
        ))
    })
    .expect("MissionLive replacement projection");
    assert_eq!(
        (
            replacement_projection.entity_index,
            replacement_projection.authority,
            replacement_projection.active,
            replacement_projection.class_id,
            replacement_projection.model,
            replacement_projection.action,
        ),
        (resident_slot, MC_AUTHORITY_LEVEL1_MISSION_LIVE, 1, 5, 4, 33,),
        "the allocated row belongs exclusively to its MissionLive resident"
    );
    for (model, action) in [(12, 97), (13, 105), (14, 113)] {
        let direct_slot = direct
            .mission
            .spawn_house_resident(resident_position, model, action, 0)
            .expect("direct resident-family constructor")
            .expect("direct source arena resident-family slot");
        let capi_slot = with_session_mut(handle, |capi| {
            Ok(capi
                .mission
                .spawn_house_resident(resident_position, model, action, 0)
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?
                .expect("C API source arena resident-family slot"))
        })
        .expect("registered C API session");
        assert_eq!(direct_slot, capi_slot);
    }
    direct.step(PlayerCommand::default());
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let mut observation = McLevel1ResidentObservationV1::default();
    assert_eq!(
        mc_native_level1_resident_observation_v1(
            handle,
            &raw mut observation,
            struct_size::<McLevel1ResidentObservationV1>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (
            observation.required_mask,
            observation.observed_mask,
            observation.remaining_mask,
            observation.digest,
        ),
        (
            direct.mission.required_resident_behavior_mask(),
            direct.mission.observed_resident_behavior_mask(),
            direct.mission.remaining_resident_behavior_mask(),
            direct.mission.resident_behavior_observation_digest_v1(),
        ),
        "the public C ABI observation receipt must exactly match direct Rust state"
    );
    with_session(handle, |capi| {
        assert!(direct.mission.resident_projection_is_linked(resident_slot));
        assert!(capi.mission.resident_projection_is_linked(resident_slot));
        assert!(
            capi.renderable_map_chains.tracks(resident_slot),
            "C API applies the resident render projection in the dispatch slot"
        );
        assert_eq!(
            direct.live_world_hash_v2(),
            capi.live_world_hash_v2(),
            "resident effect live-world hashes diverged"
        );
        assert_eq!(
            direct.mission.source_arena_hash_v1(),
            capi.mission.source_arena_hash_v1(),
            "resident effect source-arena hashes diverged"
        );
        assert_eq!(direct.mission.remaining_resident_behavior_mask(), 0);
        assert_eq!(capi.mission.remaining_resident_behavior_mask(), 0);
        Ok(())
    })
    .expect("registered C API session");

    direct.mission.has_fireball = true;
    direct.mission.player_mana = 10_000;
    with_session_mut(handle, |capi| {
        capi.mission.has_fireball = true;
        capi.mission.player_mana = 10_000;
        Ok(())
    })
    .expect("registered C API session");
    let mut fire_command = valid_command();
    fire_command.flags = 0x10;
    let mut saw_dynamic_family = false;
    for turn in 25..=36 {
        let command = if turn % 2 == 1 {
            fire_command
        } else {
            valid_command()
        };
        direct.step(player_command(command).expect("valid parity command"));
        saw_dynamic_family |=
            !direct.mission.projectiles().is_empty() || !direct.mission.impacts().is_empty();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        with_session(handle, |capi| {
            assert_eq!(
                state_hash(&direct.state),
                state_hash(&capi.state),
                "flight state diverged on dynamic turn {turn}"
            );
            assert_eq!(
                direct.live_world_hash_v2(),
                capi.live_world_hash_v2(),
                "typed imported world diverged on dynamic turn {turn}"
            );
            assert_eq!(
                direct.mission.source_arena_hash_v1(),
                capi.mission.source_arena_hash_v1(),
                "source arena diverged on dynamic turn {turn}"
            );
            assert_eq!(
                direct.mission.projectiles().len(),
                capi.mission.projectiles().len(),
                "dynamic family count diverged on turn {turn}"
            );
            Ok(())
        })
        .expect("registered C API session");
    }
    assert!(
        saw_dynamic_family,
        "dynamic parity phase must allocate and dispatch a Fireball family"
    );
    direct.reset();
    assert_eq!(
        mc_native_session_reset(
            handle,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let reset_projection = with_session(handle, |capi| {
        Ok(entity_snapshot(
            &capi.world.entities()[51],
            &capi.state,
            &capi.live_world,
            &capi.mission,
        ))
    })
    .expect("reset Villager projection");
    assert_eq!(
        (
            reset_projection.entity_index,
            reset_projection.authority,
            reset_projection.active,
            reset_projection.class_id,
            reset_projection.model,
        ),
        (51, MC_AUTHORITY_VILLAGER_LIVE, 1, 5, 13),
        "reset alone restores the exact imported Villager identity"
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn replaceable_fallback_hides_the_stale_generic_entity_projection() {
    let handle = open_fixture();
    let replaced_slot = with_session_mut(handle, |session| {
        let replaced_slot = 70;
        let stale_villager = session
            .live_world
            .villagers()
            .iter()
            .find(|villager| villager.entity_index == replaced_slot)
            .expect("row 70 imported Villager");
        assert!(stale_villager.active);
        assert!(
            session.mission.disable_imported_source_slot(replaced_slot),
            "production disable marks the imported source row"
        );
        let player = session.state;
        session.mission.begin_source_turn();
        session.mission.finish_source_turn(&player);
        assert!(
            !session.mission.imported_source_slot_is_live(replaced_slot),
            "production turn-start cleanup retires the imported source row"
        );
        Ok(replaced_slot)
    })
    .expect("replaceable fallback Villager slot");

    assert!(
        (51..=78).contains(&replaced_slot),
        "the discriminating fallback must retire one of the 28 Villager rows"
    );
    with_session(handle, |session| {
        let entity = &session.world.entities()[usize::from(replaced_slot)];
        assert!(
            entity.authority == EntityAuthority::VillagerLive
                && entity.active
                && !session.mission.imported_source_slot_is_live(replaced_slot),
            "the stale typed Villager remains present behind a retired source slot"
        );
        Ok(())
    })
    .expect("registered C API session");

    let mut snapshot = McEntitySnapshotV2::default();
    let mut written = 0;
    assert_eq!(
        mc_native_entities_copy(
            handle,
            u32::from(replaced_slot),
            &raw mut snapshot,
            1,
            struct_size::<McEntitySnapshotV2>(),
            &raw mut written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(written, 1);
    assert_eq!(snapshot.entity_index, replaced_slot);
    assert_eq!(snapshot.active, 0);
    assert_eq!(snapshot.authority, MC_AUTHORITY_INACTIVE);
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn dead_player_ignores_flight_view_and_spell_input_until_reset() {
    let handle = open_fixture();
    with_session_mut(handle, |session| {
        session.mission.objective_textbox_ticks = 4;
        let fire = session
            .context
            .spell_fire
            .get_or_insert_with(Default::default);
        fire.enabled_holder_indices[0] = 1;
        session
            .mission
            .capture_spell_holder_active_values(&session.context);
        let lethal_damage = session.mission.player_max_life.saturating_add(1);
        session.mission.submit_player_damage(lethal_damage, 10);
        Ok(())
    })
    .unwrap();
    assert_eq!(mc_native_level1_player_menu_state(handle, 5), MC_STATUS_OK);

    let mut retained_command = valid_command();
    retained_command.flags = 1;
    retained_command.roll = 72;
    retained_command.pitch = -48;
    let mut dying_entry = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const retained_command,
            &raw mut dying_entry,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let (retained_roll_delta, retained_pitch_delta, retained_target_speed) =
        with_session(handle, |session| {
            assert_eq!(
                session.mission.player_action,
                sim_core::level1_mission::PLAYER_ACTION_DYING
            );
            Ok((
                session.state.roll_delta,
                session.state.pitch_delta,
                session.state.target_speed,
            ))
        })
        .unwrap();

    let mut lifecycle = McLevel1PlayerLifecycleSnapshotV2::default();
    assert_eq!(
        mc_native_level1_player_lifecycle_snapshot(
            handle,
            &raw mut lifecycle,
            struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(lifecycle.player_action, 2);
    assert_eq!(
        (
            lifecycle.player_active,
            lifecycle.player_visible,
            lifecycle.player_targetable,
            lifecycle.input_suppressed,
        ),
        (1, 1, 0, 1)
    );

    let mut suppressed = valid_command();
    suppressed.flags = 0x12;
    suppressed.roll = -101;
    suppressed.pitch = 99;
    suppressed.requested_spell = 0;
    let fire_sequence_before = dying_entry.fire_sequence;
    let expected_burst_position = with_session(handle, |session| {
        let mut expected_state = session.state;
        let mut expected_context = session.context.clone();
        Ok(step_entity_body_with_context(
            &mut expected_state,
            session.world.profile(),
            &session.terrain,
            &mut expected_context,
        )
        .proposed)
    })
    .unwrap();
    let mut after_direct_body = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const suppressed,
            &raw mut after_direct_body,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    with_session(handle, |session| {
        assert_eq!(
            (
                session.state.roll_delta,
                session.state.pitch_delta,
                session.state.target_speed,
            ),
            (
                retained_roll_delta,
                retained_pitch_delta,
                retained_target_speed,
            ),
            "action 2 runs the direct retained flight body without relatching command axes"
        );
        assert!(
            !session.mission.death_bursts().is_empty(),
            "the production action-2 pass emits its per-tick burst"
        );
        assert_eq!(
            session.mission.death_bursts().last().unwrap().position,
            expected_burst_position,
            "the C ABI production route retains predictedAxis before the separate fall"
        );
        let burst = session.mission.death_bursts().last().unwrap();
        assert!(
            session
                .renderable_map_chains
                .entity_chain(
                    burst.position.x.to_be_bytes()[0],
                    burst.position.y.to_be_bytes()[0],
                )
                .contains(&burst.source_entity_index),
            "a live action-1 burst survives the production CAPI chain handoff before marker replacement"
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(after_direct_body.fire_sequence, fire_sequence_before);
    assert_eq!(
        mc_native_level1_player_lifecycle_snapshot(
            handle,
            &raw mut lifecycle,
            struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(lifecycle.menu_close_requested, 1);
    assert_eq!(lifecycle.menu_state, 0);

    let mut mission = McLevel1MissionSnapshotV2::default();
    const DYING_OUTER_FRAMES: u8 = 78;
    with_session(handle, |session| {
        assert_eq!(
            (
                session.state.position,
                session.mission.player_fall_velocity,
                session
                    .terrain
                    .ground_height(session.state.position)
                    .wrapping_add(100),
                session.terrain.mutation_sequence(),
            ),
            (
                Position {
                    x: 19_684,
                    y: 56_918,
                    z: 5_020,
                },
                -2,
                4_864,
                2,
            ),
            "fixed owner-data position, source fall velocity, +100 collision floor, and terrain generation form the independent action-2 preimage"
        );
        Ok(())
    })
    .unwrap();
    for dying_step in 0..DYING_OUTER_FRAMES {
        assert_eq!(
            mc_native_level1_mission_snapshot(
                handle,
                &raw mut mission,
                struct_size::<McLevel1MissionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_player_lifecycle_snapshot(
                handle,
                &raw mut lifecycle,
                struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(lifecycle.player_action, 2);
        let (expected_position, expected_velocity, expected_contact) =
            with_session(handle, |session| {
                let mut expected_state = session.state;
                let mut expected_context = session.context.clone();
                step_entity_body_with_context(
                    &mut expected_state,
                    session.world.profile(),
                    &session.terrain,
                    &mut expected_context,
                );
                expected_state.position.z = expected_state
                    .position
                    .z
                    .wrapping_add(session.mission.player_fall_velocity);
                let mut expected_velocity = session
                    .mission
                    .player_fall_velocity
                    .wrapping_sub(2)
                    .max(-256);
                if expected_velocity > 0 {
                    expected_velocity = 0;
                }
                let ground = session
                    .terrain
                    .ground_height(expected_state.position)
                    .wrapping_add(100);
                if expected_state.position.z < ground {
                    expected_state.position.z = ground;
                }
                Ok((
                    expected_state.position,
                    expected_velocity,
                    expected_state.position.z == ground,
                ))
            })
            .unwrap();
        assert_eq!(
            expected_contact,
            dying_step == DYING_OUTER_FRAMES - 1,
            "the source recurrence independently reaches contact only on outer frame 78"
        );
        let mut step = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const suppressed,
                &raw mut step,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        with_session(handle, |session| {
            assert_eq!(session.state.position, expected_position);
            assert_eq!(session.mission.player_fall_velocity, expected_velocity);
            assert_eq!(
                session.mission.player_action,
                if expected_contact { 3 } else { 2 }
            );
            Ok(())
        })
        .unwrap();
    }
    assert_eq!(
        mc_native_level1_mission_snapshot(
            handle,
            &raw mut mission,
            struct_size::<McLevel1MissionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_level1_player_lifecycle_snapshot(
            handle,
            &raw mut lifecycle,
            struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(lifecycle.player_action, 3);
    assert_eq!(mission.player_dead, 1);
    assert_eq!(
        mission.objective_textbox_ticks,
        200_u8 - DYING_OUTER_FRAMES,
        "PresentObjective remains frame-driven while dead-player input is suppressed"
    );
    assert_eq!(
        (
            lifecycle.player_active,
            lifecycle.player_visible,
            lifecycle.player_targetable,
            lifecycle.menu_close_requested,
            lifecycle.notification_id,
            lifecycle.notification_state,
            lifecycle.notification_timer,
            lifecycle.marker_present,
            lifecycle.marker_class,
            lifecycle.marker_model,
            lifecycle.marker_action,
            lifecycle.marker_byte_38,
        ),
        (1, 0, 0, 0, 374, 1, 100, 1, 10, 40, 42, 2)
    );
    assert_eq!(
        (lifecycle.marker_life, lifecycle.marker_max_life),
        (300, 300)
    );
    assert_eq!(
        lifecycle.marker_dispatch_phase,
        lifecycle.marker_source_entity_index.to_le_bytes()[0]
    );
    assert_eq!(
        lifecycle.marker_constructor_phase,
        u32::from(lifecycle.marker_source_entity_index % 11)
    );
    assert_eq!(
        lifecycle.marker_trigger_entity_index,
        dying_entry.entity_index
    );
    assert_eq!(lifecycle.marker_particle_parameter_index, 65);
    assert!(lifecycle.lifecycle_trace_count >= 33);
    assert_ne!(lifecycle.lifecycle_hash, lifecycle.source_arena_hash);
    assert_eq!(lifecycle.killer_score_awards, 0);
    with_session(handle, |session| {
        let marker = session
            .mission
            .player_death_marker()
            .expect("production action 2 allocated the death marker");
        assert!(
            session
                .renderable_map_chains
                .tracks(marker.source_entity_index),
            "the production C ABI retains model 40 in its live source map"
        );
        assert!(
            session
                .renderable_map_chains
                .entity_chain(
                    marker.position.x.to_le_bytes()[1],
                    marker.position.y.to_le_bytes()[1],
                )
                .contains(&marker.source_entity_index),
            "sub_501D0 map-chain insertion survives the C ABI presentation sync"
        );
        Ok(())
    })
    .unwrap();

    let (released_slot, released_position, moved_position) = with_session_mut(handle, |session| {
        let released = session
            .mission
            .released_spell_holders()
            .iter()
            .flatten()
            .find(|released| released.source_entity_index == 1)
            .copied()
            .expect("literal holder one survives the production contact path");
        let moved = Position {
            x: released.position.x.wrapping_add(1_024),
            ..released.position
        };
        session
            .renderable_map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: released.source_entity_index,
                active: true,
                x: moved.x,
                y: moved.y,
            })
            .expect("simulate the later live actor CopyPosition update");
        Ok((released.source_entity_index, released.position, moved))
    })
    .unwrap();
    let action_timer = lifecycle.action_timer;
    let mut banished = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const suppressed,
            &raw mut banished,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_level1_player_lifecycle_snapshot(
            handle,
            &raw mut lifecycle,
            struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(lifecycle.player_action, 3);
    assert_eq!(lifecycle.action_timer, action_timer);
    assert_eq!(lifecycle.banished_camera_active, 1);
    with_session(handle, |session| {
        let moved_tile = (
            moved_position.x.to_be_bytes()[0],
            moved_position.y.to_be_bytes()[0],
        );
        assert!(
            session
                .renderable_map_chains
                .entity_chain(moved_tile.0, moved_tile.1)
                .contains(&released_slot),
            "a later live holder position owns the action-3 CAPI chain"
        );
        let released_tile = (
            released_position.x.to_be_bytes()[0],
            released_position.y.to_be_bytes()[0],
        );
        if moved_tile != released_tile {
            assert!(
                !session
                    .renderable_map_chains
                    .entity_chain(released_tile.0, released_tile.1)
                    .contains(&released_slot),
                "historical release coordinates are not replayed on action-3 turns"
            );
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(
        (
            lifecycle.controller_pitch,
            lifecycle.controller_roll,
            lifecycle.recoil_speed,
        ),
        (0, 0, 0)
    );

    let mut reset = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_reset(handle, &raw mut reset, struct_size::<McSessionSnapshotV2>()),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_level1_mission_snapshot(
            handle,
            &raw mut mission,
            struct_size::<McLevel1MissionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(mission.player_dead, 0);
    assert_eq!(mission.player_life, mission.player_max_life);
    assert_eq!(
        mc_native_level1_player_lifecycle_snapshot(
            handle,
            &raw mut lifecycle,
            struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(lifecycle.player_action, 0);
    assert_eq!(lifecycle.marker_present, 0);
    assert_eq!(lifecycle.notification_id, 0);

    let (player_entity_index, victim_color, lethal_damage) = with_session_mut(handle, |session| {
        let position = session.state.position;
        session.state.position.z = session.terrain.ground_height(position).wrapping_add(100);
        Ok((
            session.state.entity_index,
            session.state.player_color_index,
            session.mission.player_max_life.saturating_add(1),
        ))
    })
    .unwrap();
    assert_eq!(
        mc_native_level1_player_inject_damage_for_qualification(
            handle,
            lethal_damage,
            player_entity_index,
        ),
        MC_STATUS_OK
    );
    let mut lethal_entry = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const suppressed,
            &raw mut lethal_entry,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    with_session_mut(handle, |session| {
        assert_eq!(
            session.mission.player_action,
            sim_core::level1_mission::PLAYER_ACTION_DYING
        );
        let position = session.state.position;
        session.state.position.z = session.terrain.ground_height(position).wrapping_add(100);
        session.state.recoil_speed = 0;
        session.state.skip_flight_once = true;
        Ok(())
    })
    .unwrap();
    for _ in 0..8 {
        let mut step = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const suppressed,
                &raw mut step,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_level1_player_lifecycle_snapshot(
                handle,
                &raw mut lifecycle,
                struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        if lifecycle.player_action == sim_core::level1_mission::PLAYER_ACTION_BANISHED {
            break;
        }
    }
    assert_eq!(
        lifecycle.player_action,
        sim_core::level1_mission::PLAYER_ACTION_BANISHED
    );
    assert_eq!(lifecycle.killer_entity_index, player_entity_index);
    assert_eq!(lifecycle.killer_score_victim_color, victim_color);
    assert_eq!(
        lifecycle.killer_score_awards, 1,
        "the C ABI projects the qualifying killer's own victim-color score"
    );
    with_session_mut(handle, |session| {
        session.state.effective_pitch = 321;
        Ok(())
    })
    .unwrap();
    let mut banished_pitch_step = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const suppressed,
            &raw mut banished_pitch_step,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_level1_player_lifecycle_snapshot(
            handle,
            &raw mut lifecycle,
            struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        lifecycle.effective_pitch, 321,
        "the lifecycle ABI distinguishes retained effective pitch from cleared controller pitch"
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);

    let exhausted_handle = open_fixture();
    let (exhausted_player, exhausted_damage, exhausted_holder, allocated_residents) =
        with_session_mut(exhausted_handle, |session| {
            let holder = session
                .world
                .entities()
                .iter()
                .find(|entity| {
                    entity.active
                        && entity.class == 10
                        && entity.model == 45
                        && session.renderable_map_chains.tracks(entity.index)
                })
                .expect("Level 1 reset exposes a source-mapped static holder row")
                .index;
            let fire = session
                .context
                .spell_fire
                .get_or_insert_with(Default::default);
            fire.enabled_holder_indices[0] = holder;
            session
                .mission
                .capture_spell_holder_active_values(&session.context);
            let resident_position = session.state.position;
            let mut allocated_residents = 0_u16;
            while session
                .mission
                .spawn_house_resident(resident_position, 13, 105, 0)
                .expect("qualified resident constructor")
                .is_some()
            {
                allocated_residents = allocated_residents.wrapping_add(1);
            }
            assert!(
                allocated_residents > 0,
                "the production source arena exposes allocatable rows before exhaustion"
            );
            Ok((
                session.state.entity_index,
                session.mission.player_max_life.saturating_add(1),
                holder,
                allocated_residents,
            ))
        })
        .unwrap();
    assert!(
        allocated_residents > 100,
        "the regression exhausts the real production source arena, not a one-slot stub"
    );
    assert_eq!(
        mc_native_level1_player_inject_damage_for_qualification(
            exhausted_handle,
            exhausted_damage,
            exhausted_player,
        ),
        MC_STATUS_OK
    );
    let mut exhausted_step = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            exhausted_handle,
            &raw const suppressed,
            &raw mut exhausted_step,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let force_contact = |handle| {
        with_session_mut(handle, |session| {
            let position = session.state.position;
            session.state.position.z = session.terrain.ground_height(position).wrapping_add(100);
            session.state.recoil_speed = 0;
            session.state.skip_flight_once = true;
            Ok(())
        })
        .unwrap();
    };
    force_contact(exhausted_handle);
    assert_eq!(
        mc_native_session_step(
            exhausted_handle,
            &raw const suppressed,
            &raw mut exhausted_step,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let first_failed_release = with_session(exhausted_handle, |session| {
        assert_eq!(
            session.mission.player_action,
            sim_core::level1_mission::PLAYER_ACTION_DYING,
            "failed marker allocation retains action 2"
        );
        assert!(session.mission.player_death_marker().is_none());
        let released = session.mission.released_spell_holders()[0]
            .expect("failed marker allocation still releases the live holder row");
        assert_eq!(released.source_entity_index, exhausted_holder);
        let tile = (
            released.position.x.to_be_bytes()[0],
            released.position.y.to_be_bytes()[0],
        );
        assert!(
            session
                .renderable_map_chains
                .entity_chain(tile.0, tile.1)
                .contains(&released.source_entity_index),
            "the CAPI handoff retains holder relinking when the marker allocator fails"
        );
        Ok(released)
    })
    .unwrap();
    with_session_mut(exhausted_handle, |session| {
        session.state.position.x = session.state.position.x.wrapping_add(4_096);
        Ok(())
    })
    .unwrap();
    let literal_one_prior_tile = with_session(exhausted_handle, |session| {
        Ok((0..=u8::MAX)
            .find_map(|tile_y| {
                (0..=u8::MAX)
                    .find(|tile_x| {
                        session
                            .renderable_map_chains
                            .entity_chain(*tile_x, tile_y)
                            .contains(&1)
                    })
                    .map(|tile_x| (tile_x, tile_y))
            })
            .expect("exhaustion leaves literal source row one map-linked"))
    })
    .unwrap();
    force_contact(exhausted_handle);
    assert_eq!(
        mc_native_session_step(
            exhausted_handle,
            &raw const suppressed,
            &raw mut exhausted_step,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    with_session(exhausted_handle, |session| {
        assert_eq!(
            session.mission.player_action,
            sim_core::level1_mission::PLAYER_ACTION_DYING
        );
        assert!(session.mission.player_death_marker().is_none());
        let retried = session.mission.released_spell_holders()[0]
            .expect("the failed marker constructor retries holder release");
        assert_ne!(
            retried.position, first_failed_release.position,
            "a retry records its current production release position"
        );
        assert_eq!(
            retried.source_entity_index, 1,
            "the source retry consumes the literal-one holder registry sentinel"
        );
        let retried_tile = (
            retried.position.x.to_be_bytes()[0],
            retried.position.y.to_be_bytes()[0],
        );
        assert_ne!(retried_tile, literal_one_prior_tile);
        assert!(
            session
                .immediate_released_holder_handoff
                .contains(&(0, retried.source_entity_index, retried_tile.0, retried_tile.1)),
            "the current action-2 invocation publishes CopyEntityPosition immediately before later source slots dispatch"
        );
        let final_alias = session
            .mission
            .released_spell_holders()
            .iter()
            .flatten()
            .filter(|released| released.source_entity_index == retried.source_entity_index)
            .last()
            .copied()
            .expect("at least one literal-one release");
        let final_tile = (
            final_alias.position.x.to_be_bytes()[0],
            final_alias.position.y.to_be_bytes()[0],
        );
        assert!(
            session
                .renderable_map_chains
                .entity_chain(final_tile.0, final_tile.1)
                .contains(&retried.source_entity_index),
            "the last family writing the aliased literal-one row owns its final map position"
        );
        if final_tile != retried_tile {
            assert!(
                !session
                    .renderable_map_chains
                    .entity_chain(retried_tile.0, retried_tile.1)
                    .contains(&retried.source_entity_index),
                "an earlier family alias cannot retain duplicate raw map membership"
            );
        }
        assert!(
            !session
                .renderable_map_chains
                .entity_chain(literal_one_prior_tile.0, literal_one_prior_tile.1)
                .contains(&retried.source_entity_index),
            "the failed-contact retry unlinks literal row one from its prior tile"
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(mc_native_session_destroy(exhausted_handle), MC_STATUS_OK);
}

#[test]
fn step_event_order_and_copy_since_survive_reset_generation() {
    let handle = open_fixture();
    let mut before = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_snapshot(
            handle,
            &raw mut before,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let mut command = valid_command();
    command.flags = 0x10;
    let mut after = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut after,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert!(after.event_head >= 2);
    let query = McEventQueryV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McEventQueryV2>(),
        generation: after.generation,
        cursor: 0,
        reserved: [0; 4],
    };
    let mut events = vec![McEventV2::default(); 32];
    let mut batch = McEventBatchV2::default();
    assert_eq!(
        mc_native_events_copy_since(
            handle,
            &raw const query,
            events.as_mut_ptr(),
            32,
            struct_size::<McEventV2>(),
            &raw mut batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert!(batch.written >= 2);
    let written = usize::try_from(batch.written).unwrap();
    for pair in events[..written].windows(2) {
        assert_eq!(pair[1].sequence, pair[0].sequence + 1);
    }
    assert!(
        events[..written]
            .iter()
            .any(|event| event.kind == MC_EVENT_REQUEST_SPELL_ACTIVATION)
    );
    assert!(
        events[..written - 1]
            .iter()
            .any(|event| event.kind == MC_EVENT_MOVEMENT_RESOLUTION)
    );
    let music_start = events[written - 1];
    assert_eq!(music_start.kind, MC_EVENT_START_MUSIC);
    assert_eq!(music_start.code, 1);
    assert_eq!(music_start.value_0, 0);
    assert_eq!(music_start.value_1, 1);

    let next_query = McEventQueryV2 {
        cursor: batch.next_cursor,
        ..query
    };
    assert_eq!(
        mc_native_events_copy_since(
            handle,
            &raw const next_query,
            events.as_mut_ptr(),
            32,
            struct_size::<McEventV2>(),
            &raw mut batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(batch.written, 0);

    let mut reset = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_reset(handle, &raw mut reset, struct_size::<McSessionSnapshotV2>(),),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_events_copy_since(
            handle,
            &raw const query,
            events.as_mut_ptr(),
            32,
            struct_size::<McEventV2>(),
            &raw mut batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level_one_music_starts_once_after_the_second_source_pass_and_rearms_on_reset() {
    let first = open_fixture();
    let second = open_fixture();

    for handle in [first, second] {
        with_session_mut(handle, |session| {
            assert_eq!(session.source_turn, 1);
            assert!(session.music_start_armed);
            assert!(
                !session
                    .events
                    .iter()
                    .any(|event| event.kind == MC_EVENT_START_MUSIC)
            );
            session.step(PlayerCommand::default());
            assert_eq!(session.source_turn, 2);
            let starts = session
                .events
                .iter()
                .filter(|event| event.kind == MC_EVENT_START_MUSIC)
                .collect::<Vec<_>>();
            assert_eq!(starts.len(), 1);
            let start = starts[0];
            assert_eq!(start.code, 1);
            assert_eq!(start.value_0, 0);
            assert_eq!(start.value_1, 1);
            assert_eq!(start.asset_ref_index, MC_ASSET_REF_NONE);
            assert_eq!(start.generation, 1);
            assert_eq!(session.events.back().unwrap().kind, MC_EVENT_START_MUSIC);
            assert!(!session.music_start_armed);
            session.step(PlayerCommand::default());
            assert_eq!(
                session
                    .events
                    .iter()
                    .filter(|event| event.kind == MC_EVENT_START_MUSIC)
                    .count(),
                1
            );
            Ok(())
        })
        .unwrap();
    }

    with_session_mut(first, |session| {
        session.reset();
        assert_eq!(session.generation, 2);
        assert!(session.music_start_armed);
        assert!(session.events.is_empty());
        session.step(PlayerCommand::default());
        let start = session
            .events
            .iter()
            .find(|event| event.kind == MC_EVENT_START_MUSIC)
            .unwrap();
        assert_eq!(start.generation, 2);
        assert_eq!(start.value_1, 1);
        Ok(())
    })
    .unwrap();

    assert_eq!(mc_native_session_destroy(first), MC_STATUS_OK);
    assert_eq!(mc_native_session_destroy(second), MC_STATUS_OK);
}

#[test]
fn capi_terrain_projection_exposes_target_list_block_and_action_51_planes() {
    let world = presentation_test_world();
    let mut direct = world.session();
    let handle = open_fixture_with_owner_cache();
    let configure = |live_world: &mut LiveWorldState, owner_entity_index: u16| {
        let mut house = live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11)
            .copied()
            .expect("house 11");
        house.action = sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION;
        house.max_life = 2;
        house.life = 2;
        house.owner_entity_index = owner_entity_index;
        house.control_flags |= 1;
        live_world.initialize_builder_house(house).unwrap();
    };
    configure(&mut direct.live_world, direct.state.entity_index);
    with_session_mut(handle, |session| {
        configure(&mut session.live_world, session.state.entity_index);
        Ok(())
    })
    .expect("configure registered C API session");

    let command = valid_command();
    let mut snapshot = McSessionSnapshotV2::default();
    let copy_plane_for = |session_handle: u64, kind: u32| {
        let mut plane = vec![0_u8; 65_536];
        let mut written = 0;
        assert_eq!(
            mc_native_terrain_plane_copy_u8(
                session_handle,
                kind,
                plane.as_mut_ptr(),
                65_536,
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 65_536);
        plane
    };
    let copy_plane = |kind: u32| copy_plane_for(handle, kind);
    let copy_entity_indices = || {
        let mut plane = vec![0_i16; 65_536];
        let mut written = 0;
        assert_eq!(
            mc_native_terrain_entity_indices_copy(
                handle,
                plane.as_mut_ptr(),
                65_536,
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 65_536);
        plane
    };

    let blocked_handle = open_fixture_with_owner_cache();
    let blocked_control_handle = open_fixture_with_owner_cache();
    assert_eq!(
        mc_native_level1_action_51_blocker_setup_for_qualification(
            blocked_handle,
            11,
            sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION,
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_level1_action_51_blocker_setup_for_qualification(
            blocked_control_handle,
            11,
            u8::MAX,
        ),
        MC_STATUS_OK
    );
    let blocked_before = with_session(blocked_handle, |session| {
        let house = session
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11)
            .expect("blocked house 11");
        let views = level1_turn_start_source_views(
            &session.mission,
            &session.state,
            &session.live_world,
        );
        assert!(
            Level1MissionState::house_action_51_blocked(&views, 11),
            "public setup must materialize a real overlapping model-42 source row: house={:?}, blockers={:?}, constructions={:?}",
            views.iter().find(|view| view.source_entity_index == 11),
            views
                .iter()
                .filter(|view| view.class == 10 && view.model == 42)
                .collect::<Vec<_>>(),
            session.mission.house_completion_constructions()
        );
        Ok((house.action, house.life, house.phase))
    })
    .expect("public blocked-session setup");
    let mut blocked_snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            blocked_handle,
            &raw const command,
            &raw mut blocked_snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let mut blocked_control_snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            blocked_control_handle,
            &raw const command,
            &raw mut blocked_control_snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let blocked_after = with_session(blocked_handle, |session| {
        let house = session
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11)
            .expect("blocked house 11 after public step");
        Ok((house.action, house.life, house.phase))
    })
    .expect("public blocked-session result");
    assert_eq!(
        blocked_before.0,
        sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION
    );
    assert_eq!(blocked_before.1, 2);
    assert_eq!(blocked_after.0, blocked_before.0);
    assert_eq!(blocked_after.1, blocked_before.1);
    assert_eq!(blocked_after.2, blocked_before.2.wrapping_add(1));
    for (kind, before) in [
        MC_TERRAIN_HEIGHT_U8,
        MC_TERRAIN_TYPES_U8,
        MC_TERRAIN_SHADE_U8,
        MC_TERRAIN_ANGLE_U8,
        MC_TERRAIN_SECONDARY_U8,
    ]
    .into_iter()
    .map(|kind| (kind, copy_plane_for(blocked_control_handle, kind)))
    {
        assert!(
            copy_plane_for(blocked_handle, kind) == before,
            "blocked action 51 changed terrain plane kind {kind}"
        );
    }
    assert_eq!(mc_native_session_destroy(blocked_handle), MC_STATUS_OK);
    assert_eq!(
        mc_native_session_destroy(blocked_control_handle),
        MC_STATUS_OK
    );

    direct.step(PlayerCommand::default());
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        copy_plane(MC_TERRAIN_HEIGHT_U8),
        direct.terrain.planes().ground_heights.as_ref()
    );
    assert_eq!(
        copy_plane(MC_TERRAIN_TYPES_U8),
        direct.terrain.planes().terrain_types.as_ref()
    );
    assert_eq!(
        copy_plane(MC_TERRAIN_ANGLE_U8),
        direct.terrain.planes().angles.as_ref()
    );
    assert_eq!(
        copy_plane(MC_TERRAIN_SHADE_U8),
        direct.terrain.planes().shading.as_ref()
    );
    assert_eq!(
        copy_plane(MC_TERRAIN_SECONDARY_U8),
        direct.terrain.planes().secondary_heights.as_ref()
    );
    assert_eq!(
        copy_entity_indices(),
        direct.terrain.planes().entity_indices.as_ref()
    );
    assert_eq!(
        direct
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11)
            .unwrap()
            .action,
        sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION
    );

    direct.step(PlayerCommand::default());
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        copy_plane(MC_TERRAIN_HEIGHT_U8),
        direct.terrain.planes().ground_heights.as_ref()
    );
    assert_eq!(
        copy_plane(MC_TERRAIN_TYPES_U8),
        direct.terrain.planes().terrain_types.as_ref()
    );
    assert_eq!(
        copy_plane(MC_TERRAIN_ANGLE_U8),
        direct.terrain.planes().angles.as_ref()
    );
    assert_eq!(
        copy_plane(MC_TERRAIN_SHADE_U8),
        direct.terrain.planes().shading.as_ref()
    );
    assert_eq!(
        copy_plane(MC_TERRAIN_SECONDARY_U8),
        direct.terrain.planes().secondary_heights.as_ref()
    );
    assert_eq!(
        copy_entity_indices(),
        direct.terrain.planes().entity_indices.as_ref()
    );
    with_session(handle, |session| {
        assert_eq!(session.terrain, direct.terrain);
        assert_eq!(session.live_world, direct.live_world);
        assert_eq!(
            session.mission.source_arena_hash_v2(),
            direct.mission.source_arena_hash_v2()
        );
        assert_eq!(
            session.mission.house_construction_effects(),
            direct.mission.house_construction_effects()
        );
        assert_eq!(
            session.mission.house_completion_constructions(),
            direct.mission.house_completion_constructions()
        );
        assert_eq!(session.pending_house_effects, direct.pending_house_effects);
        Ok(())
    })
    .expect("direct/C ABI action-51 full-state equality");
    let capi_action = with_session(handle, |session| {
        Ok(session
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11)
            .expect("C API house 11")
            .action)
    })
    .expect("registered C API house action");
    assert_eq!(capi_action, sim_core::house::HOUSE_LIVE_ACTION);
    assert_eq!(
        direct
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11)
            .unwrap()
            .action,
        capi_action
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn capi_action_51_completion_exposes_rich_model_42_effect() {
    let handle = open_fixture_with_owner_cache();
    let parent_entity_index = 152;
    let house_building_index = with_session(handle, |session| {
        Ok(session
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11)
            .expect("qualification house 11")
            .building_index)
    })
    .expect("qualification house selector");
    let construction_selector = [17, 26]
        .into_iter()
        .find(|selector| *selector != house_building_index)
        .expect("two distinct qualified construction selectors");
    let (parent_source_id, parent_position, expected_half_width, expected_half_depth) =
        with_session(handle, |session| {
            let parent = level1_turn_start_source_views(
                &session.mission,
                &session.state,
                &session.live_world,
            )
            .into_iter()
            .find(|view| view.source_entity_index == parent_entity_index)
            .expect("qualification class-3 parent");
            let footprint = session
                .world
                .house_build_footprint(construction_selector)
                .expect("qualification construction footprint");
            Ok((
                parent.source_id,
                parent.position,
                i16::try_from(((i32::from(footprint.width) << 8) + 1_280) >> 1)
                    .expect("qualification width shift fits i16"),
                i16::try_from(((i32::from(footprint.height) << 8) + 1_280) >> 1)
                    .expect("qualification depth shift fits i16"),
            ))
        })
        .expect("qualification parent state");
    let mut effect_count = u32::MAX;
    assert_eq!(
        mc_native_level1_house_completion_effect_count(handle, &raw mut effect_count),
        MC_STATUS_OK
    );
    assert_eq!(effect_count, 0);
    let mut hash_before = 0;
    assert_eq!(
        mc_native_session_source_arena_hash(handle, &raw mut hash_before),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_level1_action_51_completion_setup_for_qualification(
            handle,
            11,
            parent_entity_index,
            construction_selector,
        ),
        MC_STATUS_OK
    );
    let command = valid_command();
    let mut session_snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut session_snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_level1_house_completion_effect_count(handle, &raw mut effect_count),
        MC_STATUS_OK
    );
    assert!(effect_count >= 1);
    let mut effect = McLevel1HouseCompletionEffectV2::default();
    assert_eq!(
        mc_native_level1_house_completion_effect_snapshot(
            handle,
            0,
            &raw mut effect,
            struct_size::<McLevel1HouseCompletionEffectV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(effect.abi_version, MC_NATIVE_ABI_VERSION);
    assert_eq!(effect.struct_size, 80);
    assert_eq!(effect.kind, MC_LEVEL1_HOUSE_COMPLETION_ALLOCATED);
    assert_eq!(effect.house_entity_index, 11);
    assert_eq!(effect.parent_entity_index, parent_entity_index);
    assert_ne!(effect.source_entity_index, 0);
    assert_eq!(effect.source_id, parent_source_id);
    assert_eq!((effect.class_id, effect.model, effect.action), (10, 42, 44));
    assert_eq!(effect.construction_selector, construction_selector);
    assert_ne!(
        effect.house_building_index, effect.construction_selector,
        "house byte 0x46 must remain distinct from the parent dword-0x10 selector"
    );
    assert_eq!(effect.constructor_latch, 1);
    assert_eq!(effect.active, 1);
    assert_eq!(
        effect.phase,
        effect.source_entity_index.to_le_bytes()[0].wrapping_add(1),
        "the newly allocated higher slot receives its ordinary outer phase increment later in the same source pass"
    );
    assert_eq!(effect.parent_word_2e, 4);
    assert_eq!(
        effect.parent_construction_selector,
        i16::from(construction_selector)
    );
    assert_eq!(
        (effect.x, effect.y, effect.z),
        (
            parent_position.x.wrapping_add(0x0101),
            parent_position.y.wrapping_add(0x0202),
            parent_position.z.wrapping_add(33),
        )
    );
    assert_eq!(effect.collision_half_width, expected_half_width);
    assert_eq!(effect.collision_half_depth, expected_half_depth);
    assert_eq!((effect.yaw, effect.fov), (0, 256));
    assert_eq!(
        (effect.max_life, effect.life, effect.control_flags),
        (0, 0, 1)
    );
    assert_eq!(effect.effect_ordinal, 0);
    assert_eq!(effect.effect_count, effect_count);
    let mut hash_after = 0;
    assert_eq!(
        mc_native_session_source_arena_hash(handle, &raw mut hash_after),
        MC_STATUS_OK
    );
    assert_ne!(hash_after, hash_before);
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut session_snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_level1_house_completion_effect_count(handle, &raw mut effect_count),
        MC_STATUS_OK
    );
    assert_eq!(
        effect_count, 0,
        "completion-effect snapshots are scoped to their source turn"
    );
    assert_eq!(
        mc_native_level1_house_completion_effect_snapshot(
            handle,
            0,
            &raw mut effect,
            struct_size::<McLevel1HouseCompletionEffectV2>(),
        ),
        MC_STATUS_BOUNDS
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn action_51_raw_only_first_pass_survives_prior_public_finalization() {
    let world = presentation_test_world();
    let mut direct = world.session();
    let handle = open_fixture_with_owner_cache();
    let command = valid_command();
    let mut snapshot = McSessionSnapshotV2::default();

    direct.step(PlayerCommand::default());
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );

    let (
        class_2_slot,
        class_5_slot,
        building_index,
        house_position,
        house_source_id,
        raw_tile_before,
    ) = with_session_mut(handle, |session| {
        let raw_only = session
            .mission
            .source_map_chains()
            .linked_entity_indices()
            .into_iter()
            .filter(|entity_index| {
                *entity_index > 11
                    && session.mission.imported_source_slot_is_live(*entity_index)
                    && !session
                        .live_world
                        .houses()
                        .iter()
                        .any(|house| house.entity_index == *entity_index)
                    && !session
                        .live_world
                        .goats()
                        .iter()
                        .any(|goat| goat.entity_index == *entity_index)
                    && !session
                        .live_world
                        .villagers()
                        .iter()
                        .any(|villager| villager.entity_index == *entity_index)
            })
            .take(2)
            .collect::<Vec<_>>();
        assert_eq!(raw_only.len(), 2, "two raw-only qualification rows");
        let retained_presentation = session
            .renderable_map_chains
            .linked_entity_indices()
            .into_iter()
            .filter(|entity_index| !raw_only.contains(entity_index))
            .collect::<Vec<_>>();
        session
            .renderable_map_chains
            .retain_entities(&retained_presentation)
            .expect("remove qualification rows from presentation only");
        let footprint = (0..=u8::MAX)
            .filter_map(|index| session.world.house_build_footprint(index))
            .find(|footprint| footprint.cells().iter().any(|cell| cell[0] == 0xff))
            .cloned()
            .expect("qualification BUILD contains a 0xff material cell");
        let (start_x, start_y) = (0..=u8::MAX)
            .flat_map(|y| (0..=u8::MAX).map(move |x| (x, y)))
            .find(|(start_x, start_y)| {
                (0..footprint.height).all(|row| {
                    (0..footprint.width).all(|column| {
                        session
                            .mission
                            .source_map_chains()
                            .entity_chain(start_x.wrapping_add(column), start_y.wrapping_add(row))
                            .is_empty()
                    })
                })
            })
            .expect("empty wrapped BUILD footprint");
        let ff_ordinal = footprint
            .cells()
            .iter()
            .position(|cell| cell[0] == 0xff)
            .expect("0xff footprint cell");
        let ff_x = start_x.wrapping_add(
            u8::try_from(ff_ordinal % usize::from(footprint.width)).expect("footprint x fits u8"),
        );
        let ff_y = start_y.wrapping_add(
            u8::try_from(ff_ordinal / usize::from(footprint.width)).expect("footprint y fits u8"),
        );
        let raw_position = Position {
            x: u16::from(ff_x) << 8,
            y: u16::from(ff_y) << 8,
            z: 0,
        };
        let house_position = Position {
            x: u16::from(start_x.wrapping_add(footprint.width / 2)) << 8,
            y: u16::from(start_y.wrapping_add(footprint.height / 2)) << 8,
            z: 0,
        };
        assert!(
            session
                .mission
                .configure_action_51_raw_entity_for_qualification(
                    raw_only[0],
                    2,
                    1,
                    raw_only[0],
                    raw_position,
                )
        );
        assert!(
            session
                .mission
                .configure_action_51_raw_entity_for_qualification(
                    raw_only[1],
                    5,
                    25,
                    raw_only[1],
                    raw_position,
                )
        );
        assert!(!session.renderable_map_chains.tracks(raw_only[0]));
        assert!(!session.renderable_map_chains.tracks(raw_only[1]));

        let mut house = session
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11)
            .copied()
            .expect("qualification house 11");
        house.action = sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION;
        house.max_life = 2;
        house.life = 2;
        house.owner_entity_index = session.state.entity_index;
        house.control_flags |= 1;
        house.building_index = footprint.index;
        house.position = house_position;
        let house_source_id = house.source_id;
        session.live_world.initialize_builder_house(house).unwrap();
        Ok((
            raw_only[0],
            raw_only[1],
            footprint.index,
            house_position,
            house_source_id,
            session.mission.source_map_chains().entity_chain(ff_x, ff_y),
        ))
    })
    .expect("configure public raw-only action-51 pass");

    let footprint = world
        .house_build_footprint(building_index)
        .expect("direct qualification BUILD");
    let ff_ordinal = footprint
        .cells()
        .iter()
        .position(|cell| cell[0] == 0xff)
        .expect("direct 0xff footprint cell");
    let center_x = house_position.x.wrapping_add(128).to_be_bytes()[0];
    let center_y = house_position.y.wrapping_add(128).to_be_bytes()[0];
    let ff_x = center_x
        .wrapping_sub(footprint.width / 2)
        .wrapping_add(u8::try_from(ff_ordinal % usize::from(footprint.width)).unwrap());
    let ff_y = center_y
        .wrapping_sub(footprint.height / 2)
        .wrapping_add(u8::try_from(ff_ordinal / usize::from(footprint.width)).unwrap());
    let raw_position = Position {
        x: u16::from(ff_x) << 8,
        y: u16::from(ff_y) << 8,
        z: 0,
    };
    assert!(
        direct
            .mission
            .configure_action_51_raw_entity_for_qualification(
                class_2_slot,
                2,
                1,
                class_2_slot,
                raw_position,
            )
    );
    assert!(
        direct
            .mission
            .configure_action_51_raw_entity_for_qualification(
                class_5_slot,
                5,
                25,
                class_5_slot,
                raw_position,
            )
    );
    let mut direct_house = direct
        .live_world
        .houses()
        .iter()
        .find(|house| house.entity_index == 11)
        .copied()
        .expect("direct qualification house 11");
    direct_house.action = sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION;
    direct_house.max_life = 2;
    direct_house.life = 2;
    direct_house.owner_entity_index = direct.state.entity_index;
    direct_house.control_flags |= 1;
    direct_house.building_index = building_index;
    direct_house.position = house_position;
    direct
        .live_world
        .initialize_builder_house(direct_house)
        .unwrap();
    assert_eq!(
        direct.mission.source_map_chains().entity_chain(ff_x, ff_y),
        raw_tile_before
    );

    direct.step(PlayerCommand::default());
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    with_session(handle, |session| {
        assert_eq!(
            session.mission.house_construction_effects(),
            direct.mission.house_construction_effects(),
            "direct and public C ABI preserve identical raw-chain effect order"
        );
        assert_eq!(
            session.mission.source_map_chains().entity_chain(ff_x, ff_y),
            raw_tile_before,
            "sub_57390 leaves the raw head and old links untouched"
        );
        assert_eq!(
            session.mission.raw_house_construction_state(class_2_slot),
            direct.mission.raw_house_construction_state(class_2_slot)
        );
        assert_eq!(
            session.mission.raw_house_construction_state(class_5_slot),
            direct.mission.raw_house_construction_state(class_5_slot)
        );
        Ok(())
    })
    .expect("public raw-only action-51 result");
    let class_2 = direct
        .mission
        .raw_house_construction_state(class_2_slot)
        .unwrap();
    let class_5 = direct
        .mission
        .raw_house_construction_state(class_5_slot)
        .unwrap();
    assert!(
        !direct.mission.house_construction_effects().is_empty(),
        "raw action-51 emitted no effects: house={:?} pending={:?} raw_chain={:?}",
        direct
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11),
        direct.pending_house_effects,
        direct.mission.source_map_chains().entity_chain(ff_x, ff_y),
    );
    assert_ne!(
        class_2.0 & sim_core::level1_mission::SOURCE_DISABLED_FLAG,
        0
    );
    assert_eq!(
        (class_5.1, class_5.2, class_5.3),
        (-1, house_source_id, house_source_id)
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn public_turn_preserves_raw_map_rows_excluded_from_final_presentation() {
    let handle = open_fixture_with_owner_cache();
    let raw_only_entity = with_session_mut(handle, |session| {
        let seeds = session
            .world
            .raw_source_entity_indices()
            .iter()
            .copied()
            .map(|entity_index| {
                let entity = &session.world.entities()[usize::from(entity_index)];
                RenderableMapSeed {
                    entity_index,
                    active: true,
                    x: entity.position.x,
                    y: entity.position.y,
                }
            })
            .collect::<Vec<_>>();
        let raw = RenderableMapChains::from_reset(
            &seeds,
            &session.world.terrain_planes().entity_indices,
            |entity_index| {
                session
                    .world
                    .entities()
                    .get(usize::from(entity_index))
                    .map(EntitySnapshot::old_map_entity)
            },
        )
        .expect("qualified raw reset map topology");
        let raw_only_entity = raw
            .linked_entity_indices()
            .into_iter()
            .find(|entity_index| !session.renderable_map_chains.tracks(*entity_index))
            .expect("qualification fixture has a raw row excluded from presentation");
        session.mission.set_source_map_chains(raw);
        Ok(raw_only_entity)
    })
    .expect("install complete raw gameplay map");

    let command = valid_command();
    let mut snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    with_session(handle, |session| {
        assert!(
            session
                .mission
                .source_map_chains()
                .entity_chain(
                    session.world.entities()[usize::from(raw_only_entity)]
                        .position
                        .x
                        .to_be_bytes()[0],
                    session.world.entities()[usize::from(raw_only_entity)]
                        .position
                        .y
                        .to_be_bytes()[0],
                )
                .contains(&raw_only_entity),
            "public presentation finalization must not replace the raw gameplay chain"
        );
        Ok(())
    })
    .expect("raw gameplay topology after public turn");
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn public_action_51_ordinary_then_fallback_then_exhaustion_matches_direct_exact_state() {
    let world = presentation_test_world();
    let mut direct = world.session();
    let handle = open_fixture_with_owner_cache();
    let (first, second, last, ordinary, replacement) =
        (900_u16, 901_u16, 902_u16, 903_u16, 904_u16);
    let construction_selector = (0..=u8::MAX)
        .find(|index| world.house_build_footprint(*index).is_some())
        .expect("qualified construction selector");
    let construction_axis = Position { x: 0, y: 0, z: 33 };
    assert!(
        direct
            .mission
            .configure_action_51_ordinary_fallback_exhaustion_for_qualification(
                [first, second, last],
                ordinary,
                replacement,
                construction_selector,
                construction_axis,
            )
    );
    with_session_mut(handle, |session| {
        assert!(
            session
                .mission
                .configure_action_51_ordinary_fallback_exhaustion_for_qualification(
                    [first, second, last],
                    ordinary,
                    replacement,
                    construction_selector,
                    construction_axis,
                )
        );
        Ok(())
    })
    .expect("configure public fallback arena");

    let configure_house = |house: &mut sim_core::house::HouseState, owner: u16| {
        house.action = sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION;
        house.max_life = 1;
        house.life = 1;
        house.owner_entity_index = owner;
        house.control_flags |= 1;
        house.building_index = construction_selector;
        house.position = Position::default();
        house.collision_half_width = 40;
        house.collision_half_depth = 40;
    };
    let mut direct_house = direct
        .live_world
        .houses()
        .iter()
        .find(|house| house.entity_index == 11)
        .copied()
        .expect("direct fallback house");
    configure_house(&mut direct_house, direct.state.entity_index);
    direct
        .live_world
        .initialize_builder_house(direct_house)
        .unwrap();
    with_session_mut(handle, |session| {
        let mut house = session
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11)
            .copied()
            .expect("public fallback house");
        configure_house(&mut house, session.state.entity_index);
        session.live_world.initialize_builder_house(house).unwrap();
        Ok(())
    })
    .expect("configure public fallback house");

    direct.step(PlayerCommand::default());
    let command = valid_command();
    let mut snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert!(matches!(
        direct.mission.house_construction_effects(),
        [
            HouseConstructionSourceEffect::ConstructionAllocated {
                parent_entity_index,
                source_entity_index,
                ..
            },
            HouseConstructionSourceEffect::ConstructionAllocated {
                parent_entity_index: fallback_parent,
                source_entity_index: fallback_source,
                ..
            },
            HouseConstructionSourceEffect::ConstructionAllocationFailed {
                parent_entity_index: failed_parent,
                ..
            }
        ] if *parent_entity_index == first
            && *source_entity_index == ordinary
            && *fallback_parent == second
            && *fallback_source == replacement
            && *failed_parent == last
    ));
    assert_eq!(
        direct
            .mission
            .house_completion_constructions()
            .iter()
            .map(|child| (child.source_entity_index, child.parent_entity_index))
            .collect::<Vec<_>>(),
        [(ordinary, first), (replacement, second)]
    );
    assert_eq!(
        (
            direct
                .mission
                .raw_house_construction_state(first)
                .unwrap()
                .4,
            direct
                .mission
                .raw_house_construction_state(second)
                .unwrap()
                .4,
            direct.mission.raw_house_construction_state(last).unwrap().4,
            direct
                .mission
                .raw_house_construction_state(ordinary)
                .unwrap()
                .4,
            direct
                .mission
                .raw_house_construction_state(replacement)
                .unwrap()
                .4,
        ),
        (4, 4, 9, 0, 0)
    );

    let mut effect_count = 0;
    assert_eq!(
        mc_native_level1_house_completion_effect_count(handle, &raw mut effect_count),
        MC_STATUS_OK
    );
    assert_eq!(effect_count, 3);
    let mut ordinary_effect = McLevel1HouseCompletionEffectV2::default();
    let mut fallback_effect = McLevel1HouseCompletionEffectV2::default();
    let mut failed = McLevel1HouseCompletionEffectV2::default();
    assert_eq!(
        mc_native_level1_house_completion_effect_snapshot(
            handle,
            0,
            &raw mut ordinary_effect,
            struct_size::<McLevel1HouseCompletionEffectV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_level1_house_completion_effect_snapshot(
            handle,
            1,
            &raw mut fallback_effect,
            struct_size::<McLevel1HouseCompletionEffectV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_level1_house_completion_effect_snapshot(
            handle,
            2,
            &raw mut failed,
            struct_size::<McLevel1HouseCompletionEffectV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (
            ordinary_effect.kind,
            ordinary_effect.parent_entity_index,
            ordinary_effect.source_entity_index,
            ordinary_effect.parent_word_2e,
        ),
        (MC_LEVEL1_HOUSE_COMPLETION_ALLOCATED, first, ordinary, 4,)
    );
    assert_eq!(
        (
            fallback_effect.kind,
            fallback_effect.parent_entity_index,
            fallback_effect.source_entity_index,
            fallback_effect.parent_word_2e,
        ),
        (MC_LEVEL1_HOUSE_COMPLETION_ALLOCATED, second, replacement, 4,)
    );
    assert_eq!(
        (
            failed.kind,
            failed.parent_entity_index,
            failed.source_entity_index,
            failed.parent_word_2e,
        ),
        (MC_LEVEL1_HOUSE_COMPLETION_ALLOCATION_FAILED, last, 0, 9,)
    );
    let direct_digest = direct.mission.action_51_fallback_state_hash_v1();
    let mut public_digest = 0_u64;
    assert_eq!(
        mc_native_level1_action_51_fallback_state_hash_v1(handle, &raw mut public_digest,),
        MC_STATUS_OK
    );
    assert_eq!(public_digest, direct_digest);
    with_session(handle, |session| {
        assert_eq!(
            session.mission.house_construction_effects(),
            direct.mission.house_construction_effects()
        );
        assert_eq!(
            session.mission.house_completion_constructions(),
            direct.mission.house_completion_constructions()
        );
        assert_eq!(
            session.mission.action_51_fallback_state_hash_v1(),
            direct_digest,
            "the public digest covers exact direct/C-ABI allocator, raw-list, map, and construction state"
        );
        let views = level1_turn_start_source_views(
            &session.mission,
            &session.state,
            &session.live_world,
        );
        assert!(
            !session.mission.current_house_action_51_blocked(&views, 11),
            "fallback clears the later dword_38535 head as well as dword_38519"
        );
        Ok(())
    })
    .expect("public fallback result");
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn asset_strings_last_error_and_bounds_have_copy_contracts() {
    let handle = open_fixture();
    let mut count = 0;
    assert_eq!(
        mc_native_asset_ref_count(handle, &raw mut count),
        MC_STATUS_OK
    );
    assert_eq!(count, 76);
    let mut info = McAssetRefInfoV2::default();
    assert_eq!(
        mc_native_asset_ref_info(handle, 0, &raw mut info, struct_size::<McAssetRefInfoV2>(),),
        MC_STATUS_OK
    );
    assert_eq!(info.asset_index, 0);
    let mut length = 0;
    assert_eq!(
        mc_native_asset_string_length(
            handle,
            0,
            MC_ASSET_STRING_CONTENT_ADDRESS,
            0,
            &raw mut length,
        ),
        MC_STATUS_OK
    );
    assert_eq!(length, 71);
    let mut tiny = [0_u8; 1];
    assert_eq!(
        mc_native_asset_string_copy(
            handle,
            0,
            MC_ASSET_STRING_CONTENT_ADDRESS,
            0,
            tiny.as_mut_ptr(),
            1,
            &raw mut length,
        ),
        MC_STATUS_BUFFER_TOO_SMALL
    );
    let mut error_length = 0;
    assert_eq!(
        mc_native_last_error_length(&raw mut error_length),
        MC_STATUS_OK
    );
    assert!(error_length > 0);
    let mut error = vec![0_u8; usize::try_from(error_length).unwrap() + 1];
    assert_eq!(
        mc_native_last_error_copy(
            error.as_mut_ptr(),
            u32::try_from(error.len()).unwrap(),
            &raw mut error_length,
        ),
        MC_STATUS_OK
    );
    assert!(
        str::from_utf8(&error[..usize::try_from(error_length).unwrap()])
            .unwrap()
            .contains("requires")
    );
    assert_eq!(
        mc_native_asset_ref_info(handle, 76, &raw mut info, struct_size::<McAssetRefInfoV2>(),),
        MC_STATUS_BOUNDS
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn last_error_copy_can_resize_and_retry_without_losing_the_diagnostic() {
    let mut info = McWorldInfoV2::default();
    assert_eq!(
        mc_native_world_info(u64::MAX, &raw mut info, struct_size::<McWorldInfoV2>(),),
        MC_STATUS_UNKNOWN_HANDLE
    );
    let mut original_length = 0;
    assert_eq!(
        mc_native_last_error_length(&raw mut original_length),
        MC_STATUS_OK
    );
    assert!(original_length > 0);

    let mut tiny = [0_u8; 1];
    let mut reported_length = 0;
    assert_eq!(
        mc_native_last_error_copy(tiny.as_mut_ptr(), 1, &raw mut reported_length),
        MC_STATUS_BUFFER_TOO_SMALL
    );
    assert_eq!(reported_length, original_length);

    assert_eq!(
        mc_native_last_error_copy(
            std::ptr::null_mut(),
            original_length + 1,
            &raw mut reported_length,
        ),
        MC_STATUS_NULL_POINTER
    );
    assert_eq!(reported_length, original_length);

    let mut length_after_failure = 0;
    assert_eq!(
        mc_native_last_error_length(&raw mut length_after_failure),
        MC_STATUS_OK
    );
    assert_eq!(length_after_failure, original_length);
    let mut message = vec![0_u8; usize::try_from(original_length).unwrap() + 1];
    assert_eq!(
        mc_native_last_error_copy(
            message.as_mut_ptr(),
            u32::try_from(message.len()).unwrap(),
            &raw mut reported_length,
        ),
        MC_STATUS_OK
    );
    assert_eq!(reported_length, original_length);
    let message = str::from_utf8(&message[..usize::try_from(reported_length).unwrap()]).unwrap();
    assert!(message.contains("unknown session handle"));
    assert!(!message.contains("requires"));
}

#[test]
fn corrupt_package_propagates_validation_status_and_useful_error() {
    let (package, closure) = fixture_paths();
    let temp = TempPackage::copy_from(&package, &closure);
    let path = temp.package.join("entities.bin");
    let mut bytes = fs::read(&path).unwrap();
    bytes[0] ^= 0xff;
    fs::write(&path, bytes).unwrap();
    assert_eq!(
        open_paths(&temp.package, &temp.closure),
        Err(MC_STATUS_VALIDATION_FAILED)
    );
    let mut length = 0;
    assert_eq!(mc_native_last_error_length(&raw mut length), MC_STATUS_OK);
    let mut error = vec![0_u8; usize::try_from(length).unwrap() + 1];
    assert_eq!(
        mc_native_last_error_copy(
            error.as_mut_ptr(),
            u32::try_from(error.len()).unwrap(),
            &raw mut length,
        ),
        MC_STATUS_OK
    );
    let error = str::from_utf8(&error[..usize::try_from(length).unwrap()]).unwrap();
    assert!(error.contains("hash mismatch"));
}

struct TempPackage {
    root: PathBuf,
    package: PathBuf,
    closure: PathBuf,
}

impl TempPackage {
    fn copy_from(package: &Path, closure: &Path) -> Self {
        let root = std::env::temp_dir().join(format!(
            "mc2-capi-v2-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        let output = root.join("package");
        fs::create_dir_all(&output).unwrap();
        fs::copy(package.join("manifest.json"), output.join("manifest.json")).unwrap();
        for name in [
            "authored-level.bin",
            "entities.bin",
            "players.bin",
            "presentation.bin",
            "runtime.bin",
            "terrain.bin",
        ] {
            fs::copy(package.join(name), output.join(name)).unwrap();
        }
        let closure_output = root.join("closure.json");
        fs::copy(closure, &closure_output).unwrap();
        Self {
            root,
            package: output,
            closure: closure_output,
        }
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos()
}
