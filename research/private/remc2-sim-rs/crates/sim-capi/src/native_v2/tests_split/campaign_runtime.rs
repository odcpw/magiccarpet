#[test]
fn level2_campaign_spell_pickups_copy_is_paged_generation_bound_and_reset_safe() {
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

    let mut rows = [McCampaignSpellPickupSnapshotV2::default(); 1];
    let mut batch = McCampaignManaBatchV2::default();
    assert_eq!(
        mc_native_campaign_spell_pickups_copy(
            handle,
            0,
            0,
            rows.as_mut_ptr(),
            1,
            struct_size::<McCampaignSpellPickupSnapshotV2>(),
            &raw mut batch,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (
            batch.generation,
            batch.written,
            batch.total_count,
            batch.has_more,
        ),
        (0, 0, 0, 0)
    );

    let (objective_x, objective_y) = level2_objective_point(handle);
    let input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command: valid_command(),
        player_slot: 0,
        player_x: objective_x,
        player_y: objective_y,
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
    // Objective evaluation writes the completion sentinel after this
    // turn; model-32 materializes group 8 on the following source turn.
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

    let mut first_page = McCampaignManaBatchV2::default();
    assert_eq!(
        mc_native_campaign_spell_pickups_copy(
            handle,
            0,
            0,
            rows.as_mut_ptr(),
            1,
            struct_size::<McCampaignSpellPickupSnapshotV2>(),
            &raw mut first_page,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(first_page.generation, 0);
    assert_eq!(first_page.start_ordinal, 0);
    assert_eq!(first_page.written, 1);
    assert_eq!(first_page.total_count, 2);
    assert_eq!(first_page.has_more, 1);
    assert_eq!(rows[0].abi_version, MC_NATIVE_ABI_VERSION);
    assert_eq!(
        rows[0].struct_size,
        struct_size::<McCampaignSpellPickupSnapshotV2>()
    );
    assert_eq!(rows[0].active, 1);

    let first_tick = first_page.tick;
    let first_hash = first_page.live_world_hash;
    let first_authored_index = rows[0].authored_index;
    let mut second_page = McCampaignManaBatchV2::default();
    assert_eq!(
        mc_native_campaign_spell_pickups_copy(
            handle,
            0,
            1,
            rows.as_mut_ptr(),
            1,
            struct_size::<McCampaignSpellPickupSnapshotV2>(),
            &raw mut second_page,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(second_page.generation, 0);
    assert_eq!(second_page.start_ordinal, 1);
    assert_eq!(second_page.written, 1);
    assert_eq!(second_page.total_count, 2);
    assert_eq!(second_page.has_more, 0);
    assert_eq!(second_page.tick, first_tick);
    assert_eq!(second_page.live_world_hash, first_hash);
    assert_ne!(rows[0].authored_index, first_authored_index);
    assert_eq!(
        BTreeSet::from([first_authored_index, rows[0].authored_index]),
        BTreeSet::from([155, 156])
    );

    batch = second_page;
    let unchanged_batch = batch;
    assert_eq!(
        mc_native_campaign_spell_pickups_copy(
            handle,
            0,
            0,
            rows.as_mut_ptr(),
            1,
            struct_size::<McCampaignSpellPickupSnapshotV2>() - 1,
            &raw mut batch,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    assert_eq!(batch, unchanged_batch);
    assert_eq!(
        mc_native_campaign_spell_pickups_copy(
            handle,
            0,
            3,
            rows.as_mut_ptr(),
            1,
            struct_size::<McCampaignSpellPickupSnapshotV2>(),
            &raw mut batch,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_BOUNDS
    );

    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    let mut reset_batch = McCampaignManaBatchV2::default();
    assert_eq!(
        mc_native_campaign_spell_pickups_copy(
            handle,
            0,
            0,
            rows.as_mut_ptr(),
            1,
            struct_size::<McCampaignSpellPickupSnapshotV2>(),
            &raw mut reset_batch,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(reset_batch, McCampaignManaBatchV2::default());
    assert_eq!(
        mc_native_campaign_spell_pickups_copy(
            handle,
            1,
            0,
            rows.as_mut_ptr(),
            1,
            struct_size::<McCampaignSpellPickupSnapshotV2>(),
            &raw mut reset_batch,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (
            reset_batch.generation,
            reset_batch.written,
            reset_batch.total_count,
            reset_batch.has_more,
        ),
        (1, 0, 0, 0)
    );
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level2_campaign_goat_damage_submit_is_generation_bound_and_consumed() {
    let handle = open_level2_campaign_handle();

    let mut goats = [McCampaignGoatSnapshotV2::default(); 36];
    let mut batch = McGoatBatchV2::default();
    assert_eq!(
        mc_native_campaign_goats_copy(
            handle,
            0,
            0,
            goats.as_mut_ptr(),
            goats.len() as u32,
            struct_size::<McCampaignGoatSnapshotV2>(),
            &raw mut batch,
            struct_size::<McGoatBatchV2>(),
        ),
        MC_STATUS_OK
    );
    let before = goats
        .iter()
        .find(|goat| goat.entity_index == 39)
        .copied()
        .expect("authored action-11 Goat row");
    assert_eq!(before.pending_damage, 0);

    assert_eq!(
        mc_native_campaign_goat_damage_submit(handle, 0, before.entity_index, 0, 152),
        MC_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(
        mc_native_campaign_goat_damage_submit(handle, 0, before.entity_index, 100, 0),
        MC_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(
        mc_native_campaign_goat_damage_submit(handle, 0, before.entity_index, 100, 152),
        MC_STATUS_OK
    );

    assert_eq!(
        mc_native_campaign_goats_copy(
            handle,
            0,
            0,
            goats.as_mut_ptr(),
            goats.len() as u32,
            struct_size::<McCampaignGoatSnapshotV2>(),
            &raw mut batch,
            struct_size::<McGoatBatchV2>(),
        ),
        MC_STATUS_OK
    );
    let queued = goats
        .iter()
        .find(|goat| goat.entity_index == before.entity_index)
        .copied()
        .expect("queued Goat row");
    assert_eq!(queued.pending_damage, 100);

    let input_command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 1,
        requested_spell: MC_REQUESTED_SPELL_NONE,
        ..McCommandV2::default()
    };
    let input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command: input_command,
        player_slot: 0,
        player_x: 123,
        player_y: 456,
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
    assert!(events[..event_count as usize].iter().any(|event| {
        event.kind == MC_CAMPAIGN_EVENT_GOAT && event.source_slot == before.entity_index
    }));

    assert_eq!(
        mc_native_campaign_goats_copy(
            handle,
            0,
            0,
            goats.as_mut_ptr(),
            goats.len() as u32,
            struct_size::<McCampaignGoatSnapshotV2>(),
            &raw mut batch,
            struct_size::<McGoatBatchV2>(),
        ),
        MC_STATUS_OK
    );
    let after = goats
        .iter()
        .find(|goat| goat.entity_index == before.entity_index)
        .copied()
        .expect("stepped Goat row");
    assert_eq!(after.pending_damage, 0);
    assert_eq!(after.life, before.life - 100);

    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    assert_eq!(
        mc_native_campaign_goat_damage_submit(handle, 0, before.entity_index, 100, 152),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level2_campaign_goat_events_publish_action11_bleat_payloads() {
    let handle = open_level2_campaign_handle();
    let mut sidecars = [McCampaignGoatEventV2::default(); 8];
    let mut sidecar_count = 0;
    assert_eq!(
        mc_native_campaign_goat_events(
            handle,
            sidecars.as_mut_ptr(),
            sidecars.len() as u32,
            struct_size::<McCampaignGoatEventV2>() - 1,
            &raw mut sidecar_count,
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );

    let mut observed = None;
    for _ in 0..256 {
        let (snapshot, generic_events, generic_count) = step_level2_campaign_once(handle, 0);
        let goat_events = campaign_goat_events(handle);
        if let Some(row) = goat_events
            .iter()
            .find(|row| {
                row.code == MC_CAMPAIGN_GOAT_BLEAT
                    && row.action == 11
                    && matches!(row.source_slot, 39 | 71)
            })
            .copied()
        {
            let generic = generic_events
                .get(row.ordinal as usize)
                .expect("Goat sidecar ordinal joins generic event stream");
            assert!(row.ordinal < generic_count as u32);
            assert_eq!(row.abi_version, MC_NATIVE_ABI_VERSION);
            assert_eq!(row.struct_size, struct_size::<McCampaignGoatEventV2>());
            assert_eq!(row.generation, snapshot.generation);
            assert_eq!(row.sound_id, 46);
            assert_eq!(row.entity_0, row.source_slot);
            assert_eq!(generic.kind, MC_CAMPAIGN_EVENT_GOAT);
            assert_eq!(generic.pass, row.pass);
            assert_eq!(generic.family, row.family);
            assert_eq!(generic.source_slot, row.source_slot);
            assert_eq!(row.asset_ref_index, MC_ASSET_REF_NONE);
            observed = Some(row);
            break;
        }
    }

    let row = observed.expect("bounded natural Level 2 turns expose an action-11 Goat bleat");
    assert!(matches!(row.source_slot, 39 | 71));
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level2_campaign_goat_events_publish_removal_and_reclaim_payloads() {
    let handle = open_level2_campaign_handle();
    let before = {
        let mut goats = [McCampaignGoatSnapshotV2::default(); 36];
        let mut batch = McGoatBatchV2::default();
        assert_eq!(
            mc_native_campaign_goats_copy(
                handle,
                0,
                0,
                goats.as_mut_ptr(),
                goats.len() as u32,
                struct_size::<McCampaignGoatSnapshotV2>(),
                &raw mut batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_OK
        );
        goats
            .iter()
            .find(|goat| goat.entity_index == 39)
            .copied()
            .expect("authored Level 2 Goat row")
    };
    assert_eq!(
        mc_native_campaign_goat_damage_submit(handle, 0, before.entity_index, 1_000_000, 152),
        MC_STATUS_OK
    );

    let mut saw_removal = false;
    let mut saw_reclaim = false;
    for _ in 0..32 {
        let (snapshot, generic_events, generic_count) = step_level2_campaign_once(handle, 0);
        let goat_events = campaign_goat_events(handle);
        for row in goat_events
            .iter()
            .filter(|row| row.source_slot == before.entity_index)
        {
            let generic = generic_events
                .get(row.ordinal as usize)
                .expect("Goat sidecar ordinal joins generic event stream");
            assert!(row.ordinal < generic_count as u32);
            assert_eq!(row.generation, snapshot.generation);
            assert_eq!(generic.kind, MC_CAMPAIGN_EVENT_GOAT);
            assert_eq!(generic.pass, row.pass);
            assert_eq!(generic.family, row.family);
            assert_eq!(generic.source_slot, row.source_slot);
            match row.code {
                MC_CAMPAIGN_GOAT_REMOVAL_QUEUED => {
                    assert_eq!(row.entity_0, before.entity_index);
                    saw_removal = true;
                }
                MC_CAMPAIGN_GOAT_RECLAIMED => {
                    assert_eq!(row.entity_0, before.entity_index);
                    saw_reclaim = true;
                }
                _ => {}
            }
        }
        if saw_removal && saw_reclaim {
            break;
        }
    }

    assert!(saw_removal, "lethal Level 2 Goat path must publish removal");
    assert!(saw_reclaim, "lethal Level 2 Goat path must publish reclaim");
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level2_campaign_step_is_revisioned_reset_safe_and_keeps_level1_abi_untouched() {
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
    let mut initial_info = McCampaignWorldInfoV2::default();
    assert_eq!(
        mc_native_campaign_world_info(
            handle,
            &raw mut initial_info,
            struct_size::<McCampaignWorldInfoV2>(),
        ),
        MC_STATUS_OK
    );
    let mut initial_goats = [McCampaignGoatSnapshotV2::default(); 36];
    let mut initial_goat_batch = McGoatBatchV2::default();
    assert_eq!(
        mc_native_campaign_goats_copy(
            handle,
            0,
            0,
            initial_goats.as_mut_ptr(),
            initial_goats.len() as u32,
            struct_size::<McCampaignGoatSnapshotV2>(),
            &raw mut initial_goat_batch,
            struct_size::<McGoatBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(initial_goat_batch.written, 36);
    assert_eq!(initial_goat_batch.total_count, 36);
    assert_eq!(initial_goat_batch.has_more, 0);
    assert_eq!(
        initial_goats
            .iter()
            .map(|goat| goat.entity_index)
            .collect::<Vec<_>>(),
        LEVEL_TWO_GOAT_INDICES.to_vec(),
        "Level 2 Goat ABI rows preserve authored source-slot order"
    );
    assert!(
        initial_goats
            .iter()
            .all(|goat| goat.class_id == GOAT_CLASS && goat.model == GOAT_MODEL)
    );
    let initial_action11_slots = initial_goats
        .iter()
        .filter(|goat| goat.action == 11)
        .map(|goat| goat.entity_index)
        .collect::<BTreeSet<_>>();
    assert_eq!(initial_action11_slots, BTreeSet::from([39, 71]));
    assert!(
        initial_goats
            .iter()
            .filter(|goat| goat.action == 11)
            .all(|goat| goat.partner_entity_index == 66),
        "Level 2 action-11 reset rows expose authored partner 66"
    );
    let mut initial_spheres = [McManaSphereSnapshotV2::default(); 64];
    let mut initial_sphere_batch = McCampaignManaBatchV2::default();
    assert_eq!(
        mc_native_campaign_mana_spheres_copy(
            handle,
            0,
            0,
            initial_spheres.as_mut_ptr(),
            initial_spheres.len() as u32,
            struct_size::<McManaSphereSnapshotV2>(),
            &raw mut initial_sphere_batch,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_OK
    );
    // The qualified reset fixture has no authored model-39 sphere row;
    // the positive route below allocates one through the production
    // `TransformEntityToManaSphere` projection before copying again.
    assert_eq!(initial_sphere_batch.written, 0);
    assert_eq!(initial_sphere_batch.total_count, 0);
    assert_eq!(initial_sphere_batch.generation, 0);
    assert_eq!(initial_sphere_batch.tick, 0);
    let (sphere_id, carrier_slots) = with_campaign_session_mut(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        let terrain = reset.canonical_terrain().clone();
        let sphere_id = reset
            .economy_mut()
            .spawn_mana_sphere_from_death(
                42,
                Position {
                    x: 384,
                    y: 62_080,
                    z: 1_024,
                },
                0x120,
                0x4321,
                321,
                111,
                &terrain,
            )
            .expect("Level 2 mana sphere allocation");
        let first = reset
            .economy_mut()
            .spawn_mana_carrier(Position::default(), 111, 0)
            .expect("first Level 2 carrier allocation");
        let second = reset
            .economy_mut()
            .spawn_mana_carrier(Position::default(), 112, 0)
            .expect("second Level 2 carrier allocation");
        Ok((sphere_id, [first, second]))
    })
    .expect("install Level 2 sphere and carrier rows");
    assert_eq!(
        mc_native_campaign_mana_spheres_copy(
            handle,
            0,
            0,
            initial_spheres.as_mut_ptr(),
            initial_spheres.len() as u32,
            struct_size::<McManaSphereSnapshotV2>(),
            &raw mut initial_sphere_batch,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(initial_sphere_batch.written, 1);
    assert_eq!(initial_sphere_batch.total_count, 1);
    assert_eq!(initial_spheres[0].sphere_id, sphere_id as u32);
    assert_eq!(
        (
            initial_spheres[0].class_id,
            initial_spheres[0].model,
            initial_spheres[0].action,
            initial_spheres[0].mana,
            initial_spheres[0].active,
        ),
        (10, 39, 41, 321, 1)
    );
    let mut carriers = [McManaCarrierSnapshotV2::default(); 4];
    let mut carrier_batch = McCampaignManaBatchV2::default();
    assert_eq!(
        mc_native_campaign_mana_carriers_copy(
            handle,
            0,
            0,
            carriers.as_mut_ptr(),
            carriers.len() as u32,
            struct_size::<McManaCarrierSnapshotV2>(),
            &raw mut carrier_batch,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(carrier_batch.written, 2);
    assert_eq!(carrier_batch.total_count, 2);
    assert_eq!(
        carriers[..2]
            .iter()
            .map(|carrier| carrier.reserved[0] as u16)
            .collect::<Vec<_>>(),
        carrier_slots.to_vec()
    );
    assert!(carriers[..2].iter().all(|carrier| carrier.active != 0));
    let mut carrier_page = [McManaCarrierSnapshotV2::default(); 1];
    let mut first_carrier_page = McCampaignManaBatchV2::default();
    assert_eq!(
        mc_native_campaign_mana_carriers_copy(
            handle,
            0,
            0,
            carrier_page.as_mut_ptr(),
            1,
            struct_size::<McManaCarrierSnapshotV2>(),
            &raw mut first_carrier_page,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(first_carrier_page.written, 1);
    assert_eq!(first_carrier_page.total_count, 2);
    assert_eq!(first_carrier_page.has_more, 1);
    let first_carrier_tick = first_carrier_page.tick;
    let first_carrier_hash = first_carrier_page.live_world_hash;
    let mut second_carrier_page = McCampaignManaBatchV2::default();
    assert_eq!(
        mc_native_campaign_mana_carriers_copy(
            handle,
            0,
            1,
            carrier_page.as_mut_ptr(),
            1,
            struct_size::<McManaCarrierSnapshotV2>(),
            &raw mut second_carrier_page,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(second_carrier_page.written, 1);
    assert_eq!(second_carrier_page.has_more, 0);
    assert_eq!(second_carrier_page.tick, first_carrier_tick);
    assert_eq!(second_carrier_page.live_world_hash, first_carrier_hash);
    let mut pre_step_info = McCampaignWorldInfoV2::default();
    assert_eq!(
        mc_native_campaign_world_info(
            handle,
            &raw mut pre_step_info,
            struct_size::<McCampaignWorldInfoV2>(),
        ),
        MC_STATUS_OK
    );
    let input_command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 1,
        requested_spell: MC_REQUESTED_SPELL_NONE,
        ..McCommandV2::default()
    };
    let mut input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command: input_command,
        player_slot: 0,
        speed_index: 0,
        player_x: 123,
        player_y: 456,
        player_z: 7,
        ..McCampaignStepInputV2::default()
    };
    let mut snapshot = McCampaignStepSnapshotV2::default();
    let mut events = [McCampaignEventV2::default(); 1024];
    let mut event_count = 0;
    let mut rejected_snapshot = McCampaignStepSnapshotV2::default();
    let mut rejected_event_count = 99;
    assert_eq!(
        mc_native_campaign_session_step(
            handle,
            &raw const input,
            &raw mut rejected_snapshot,
            struct_size::<McCampaignStepSnapshotV2>(),
            std::ptr::null_mut(),
            0,
            struct_size::<McCampaignEventV2>(),
            &raw mut rejected_event_count,
        ),
        MC_STATUS_BUFFER_TOO_SMALL
    );
    assert_eq!(rejected_event_count, 0);
    let mut after_rejected_info = McCampaignWorldInfoV2::default();
    assert_eq!(
        mc_native_campaign_world_info(
            handle,
            &raw mut after_rejected_info,
            struct_size::<McCampaignWorldInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        after_rejected_info.reset_state_hash, pre_step_info.reset_state_hash,
        "short event output must not commit the candidate turn"
    );
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
    assert_eq!(snapshot.generation, 0);
    assert!(snapshot.revision > 0);
    assert_eq!(snapshot.event_count, event_count);
    assert!(event_count > 0, "Level 2 source pass must expose events");
    assert_ne!(snapshot.state_hash, initial_info.reset_state_hash);
    assert!(
        events[..event_count as usize]
            .iter()
            .all(|event| event.abi_version == MC_NATIVE_ABI_VERSION)
    );
    let authored_group_events = campaign_authored_group_events(handle);
    assert!(
        !authored_group_events.is_empty(),
        "Level 2 source pass must expose authored-group activation"
    );
    for row in &authored_group_events {
        assert_eq!(row.generation, snapshot.generation);
        assert_eq!(
            row.struct_size,
            struct_size::<McCampaignAuthoredGroupEventV2>()
        );
        let generic = events
            .get(row.ordinal as usize)
            .expect("authored-group sidecar ordinal joins generic event stream");
        assert_eq!(generic.kind, MC_CAMPAIGN_EVENT_AUTHORED_GROUP);
        assert_eq!(generic.pass, row.pass);
        assert_eq!(generic.family, u8::try_from(row.group).unwrap_or(u8::MAX));
        assert_eq!(generic.source_slot, row.source_slot);
        assert!(row.event_index < u16::from(row.flags));
    }
    let group21_indices = authored_group_events
        .iter()
        .filter(|row| row.group == 21 && row.source_slot == 76)
        .map(|row| row.authored_index)
        .collect::<Vec<_>>();
    assert_eq!(
        group21_indices,
        vec![229, 230, 231],
        "group-21 model-32 producer preserves the authored source order"
    );
    let mut authored_group_buffer = [McCampaignAuthoredGroupEventV2::default(); 4096];
    let mut authored_group_count = 0;
    assert_eq!(
        mc_native_campaign_authored_group_events(
            handle,
            authored_group_buffer.as_mut_ptr(),
            authored_group_buffer.len() as u32,
            struct_size::<McCampaignAuthoredGroupEventV2>() - 1,
            &raw mut authored_group_count,
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    let goat_slots = events[..event_count as usize]
        .iter()
        .filter(|event| event.kind == MC_CAMPAIGN_EVENT_GOAT)
        .map(|event| event.source_slot)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        goat_slots,
        LEVEL_TWO_GOAT_INDICES.into_iter().collect::<BTreeSet<_>>(),
        "public Level 2 C ABI must expose every authored Goat source slot"
    );
    assert!(
        goat_slots.contains(&39) && goat_slots.contains(&71),
        "public Level 2 C ABI must expose Goat source slots 39 and 71"
    );
    let mut economy_events = [McCampaignEconomyEventV2::default(); 1024];
    let mut economy_event_count = 0;
    assert_eq!(
        mc_native_campaign_economy_events(
            handle,
            economy_events.as_mut_ptr(),
            economy_events.len() as u32,
            struct_size::<McCampaignEconomyEventV2>(),
            &raw mut economy_event_count,
        ),
        MC_STATUS_OK
    );
    let allocated_economy_codes = [
        MC_CAMPAIGN_ECONOMY_MANA_SPHERE_SPAWNED,
        MC_CAMPAIGN_ECONOMY_MANA_SPHERE_CLAIMED,
        MC_CAMPAIGN_ECONOMY_MANA_SPHERE_COLLECTED,
        MC_CAMPAIGN_ECONOMY_MANA_SPHERES_MERGED,
        MC_CAMPAIGN_ECONOMY_MANA_CARRIER_SPAWNED,
        MC_CAMPAIGN_ECONOMY_MANA_CARRIER_ARRIVED,
        MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_ARRIVED,
        MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_COLLECTED_SPHERE,
        MC_CAMPAIGN_ECONOMY_DEATH_BURST_SPAWNED,
        MC_CAMPAIGN_ECONOMY_DEATH_BURST_EXPIRED,
        MC_CAMPAIGN_ECONOMY_CASTLE_BUILT,
        MC_CAMPAIGN_ECONOMY_CASTLE_DESTROYED,
        MC_CAMPAIGN_ECONOMY_CASTLE_CONSTRUCTION_SOUND,
        MC_CAMPAIGN_ECONOMY_CASTLE_DESTRUCTION_SOUND,
        MC_CAMPAIGN_ECONOMY_CASTLE_STAGE_DESTROYED,
        MC_CAMPAIGN_ECONOMY_DAMAGE_APPLIED,
        MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_CAVE_SOUND,
        MC_CAMPAIGN_ECONOMY_FIRE_SPHERE_COLLISION_SOUND,
        MC_CAMPAIGN_ECONOMY_AUTHORED_CLASS5_MODEL3_SOUND,
    ];
    assert!(!allocated_economy_codes.contains(&0));
    assert!(!allocated_economy_codes.contains(&20));
    assert!(!allocated_economy_codes.contains(&u8::MAX));
    for row in &economy_events[..economy_event_count as usize] {
        assert_eq!(row.abi_version, MC_NATIVE_ABI_VERSION);
        assert_eq!(row.generation, snapshot.generation);
        let generic = events
            .get(row.ordinal as usize)
            .expect("economy sidecar ordinal joins generic event stream");
        assert_eq!(generic.kind, MC_CAMPAIGN_EVENT_ECONOMY);
        assert_eq!(generic.pass, row.pass);
        assert_eq!(generic.family, row.family);
        assert_eq!(generic.source_slot, row.source_slot);
        assert!(
            allocated_economy_codes.contains(&row.code),
            "unexpected Level 2 economy event code {}",
            row.code
        );
    }
    let mut canonical_events = Sha256::new();
    canonical_events.update(sim_core::mission_content::REMC2_SPEC_COMMIT.as_bytes());
    canonical_events.update(b"level2-campaign-events-v1");
    for (ordinal, event) in events[..event_count as usize].iter().enumerate() {
        canonical_events.update((ordinal as u32).to_le_bytes());
        canonical_events.update([event.pass, event.kind, event.family, event.flags]);
        canonical_events.update(event.source_slot.to_le_bytes());
    }
    let nyphur_dispatch_traces = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        Ok(reset.nyphur_dispatch_traces().to_vec())
    })
    .expect("read Level 2 Nyphur dispatch traces");
    canonical_events.update(b"level2-nyphur-dispatch-traces-v1");
    for trace in &nyphur_dispatch_traces {
        canonical_events.update([trace.pass]);
        canonical_events.update(trace.source_slot.to_le_bytes());
        canonical_events.update(trace.state_byte.to_le_bytes());
        canonical_events.update(
            u32::try_from(trace.calls.len())
                .expect("sub_12910 trace call count is bounded")
                .to_le_bytes(),
        );
        for call in &trace.calls {
            canonical_events.update([call.hash_tag()]);
        }
    }
    assert_eq!(
        snapshot.dispatch_hash,
        canonical_events.finalize().as_slice()
    );
    let composite_hash = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        Ok(reset.campaign_state_hash())
    })
    .expect("read composite Level 2 state hash");
    assert_eq!(snapshot.state_hash, composite_hash);
    let stepped_hash = snapshot.state_hash;

    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    authored_group_count = 99;
    assert_eq!(
        mc_native_campaign_authored_group_events(
            handle,
            authored_group_buffer.as_mut_ptr(),
            authored_group_buffer.len() as u32,
            struct_size::<McCampaignAuthoredGroupEventV2>(),
            &raw mut authored_group_count,
        ),
        MC_STATUS_OK
    );
    assert_eq!(authored_group_count, 0);
    let mut reset_info = McCampaignWorldInfoV2::default();
    assert_eq!(
        mc_native_campaign_world_info(
            handle,
            &raw mut reset_info,
            struct_size::<McCampaignWorldInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(reset_info.reset_state_hash, initial_info.reset_state_hash);
    let mut reset_goats = [McCampaignGoatSnapshotV2::default(); 36];
    let mut reset_goat_batch = McGoatBatchV2::default();
    assert_eq!(
        mc_native_campaign_goats_copy(
            handle,
            1,
            0,
            reset_goats.as_mut_ptr(),
            reset_goats.len() as u32,
            struct_size::<McCampaignGoatSnapshotV2>(),
            &raw mut reset_goat_batch,
            struct_size::<McGoatBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(reset_goats, initial_goats);
    assert_eq!(reset_goat_batch.generation, 1);
    assert_eq!(reset_goat_batch.tick, 0);
    assert_eq!(
        reset_goat_batch.live_world_hash, reset_info.reset_state_hash,
        "reset batch hash must match the reset world hash"
    );
    assert_eq!(reset_goat_batch.written, 36);
    assert_eq!(reset_goat_batch.total_count, 36);
    assert_eq!(reset_goat_batch.has_more, 0);
    let mut stale_goat_batch = McGoatBatchV2::default();
    assert_eq!(
        mc_native_campaign_goats_copy(
            handle,
            0,
            0,
            initial_goats.as_mut_ptr(),
            initial_goats.len() as u32,
            struct_size::<McCampaignGoatSnapshotV2>(),
            &raw mut stale_goat_batch,
            struct_size::<McGoatBatchV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    let mut reset_carrier_batch = McCampaignManaBatchV2::default();
    assert_eq!(
        mc_native_campaign_mana_carriers_copy(
            handle,
            1,
            0,
            carriers.as_mut_ptr(),
            carriers.len() as u32,
            struct_size::<McManaCarrierSnapshotV2>(),
            &raw mut reset_carrier_batch,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(reset_carrier_batch.total_count, 0);
    let mut stale_sphere_batch = McCampaignManaBatchV2::default();
    assert_eq!(
        mc_native_campaign_mana_spheres_copy(
            handle,
            0,
            0,
            initial_spheres.as_mut_ptr(),
            initial_spheres.len() as u32,
            struct_size::<McManaSphereSnapshotV2>(),
            &raw mut stale_sphere_batch,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    let mut stale_carrier_batch = McCampaignManaBatchV2::default();
    assert_eq!(
        mc_native_campaign_mana_carriers_copy(
            handle,
            0,
            0,
            carriers.as_mut_ptr(),
            carriers.len() as u32,
            struct_size::<McManaCarrierSnapshotV2>(),
            &raw mut stale_carrier_batch,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    input.generation = 0;
    event_count = 0;
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
        MC_STATUS_GENERATION_MISMATCH
    );
    input.generation = 1;
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
    assert_ne!(snapshot.state_hash, 0);
    // The first step ran after installing a sphere and two carriers; the
    // post-reset generation starts from the qualified empty arena, so its
    // deterministic state hash must differ while the reset baseline above
    // remains unchanged.
    assert_ne!(snapshot.state_hash, stepped_hash);
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);

    let (level1_package, level1_closure) = fixture_paths();
    let level1_handle = open_paths(&level1_package, &level1_closure).expect("Level 1 open");
    let legacy_command = valid_command();
    let mut legacy_snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            level1_handle,
            &raw const legacy_command,
            &raw mut legacy_snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(mc_native_session_destroy(level1_handle), MC_STATUS_OK);
}

#[test]
fn level_two_day_music_lifecycle_is_generation_and_tick_bound() {
    let handle = open_level2_campaign_handle_with_asset_cache(&qualified_level2_day_owner_cache());
    let mut initial = McCampaignDayMusicSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_day_music_state(
            handle,
            0,
            0,
            &raw mut initial,
            struct_size::<McCampaignDayMusicSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(initial.generation, 0);
    assert_eq!(initial.tick, 0);
    assert_eq!(initial.asset_slot, 0);
    assert_eq!(initial.track, 2);
    assert_eq!(initial.volume, 127);
    assert_eq!(initial.start_count, 1);
    assert_eq!(initial.initialized, 1);
    assert_eq!(initial.loaded, 1);
    assert_eq!(initial.active, 1);
    assert_eq!(initial.playing, 1);
    assert_eq!(initial.updates_enabled, 1);
    assert_eq!(initial.target_mode, 1);
    assert_eq!(initial.current_units, 0);
    assert_eq!(initial.direction, -1);
    assert_eq!(initial.timer_hz, 90);
    assert_ne!(initial.state_hash, 0);

    let mut stale = McCampaignDayMusicSnapshotV2 {
        state_hash: u64::MAX,
        ..McCampaignDayMusicSnapshotV2::default()
    };
    assert_eq!(
        mc_native_campaign_day_music_state(
            handle,
            1,
            0,
            &raw mut stale,
            struct_size::<McCampaignDayMusicSnapshotV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(stale.state_hash, u64::MAX);

    let (step, _, _) = step_level2_campaign_once(handle, 0);
    let mut stepped = McCampaignDayMusicSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_day_music_state(
            handle,
            0,
            step.revision,
            &raw mut stepped,
            struct_size::<McCampaignDayMusicSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(stepped.update_count, 1);
    assert_eq!(stepped.target_mode, 1);
    assert_eq!(stepped.timer_active, 0);

    let mut advanced = McCampaignDayMusicSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_day_music_advance(
            handle,
            0,
            step.revision,
            1,
            &raw mut advanced,
            struct_size::<McCampaignDayMusicSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(advanced.fade_callback_count, 0);
    assert_eq!(advanced.current_units, 0);
    assert_eq!(
        mc_native_campaign_day_music_advance(
            handle,
            0,
            step.revision,
            1_025,
            &raw mut advanced,
            struct_size::<McCampaignDayMusicSnapshotV2>(),
        ),
        MC_STATUS_BOUNDS
    );

    let mut stopped = McCampaignDayMusicSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_day_music_stop(
            handle,
            0,
            step.revision,
            &raw mut stopped,
            struct_size::<McCampaignDayMusicSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(stopped.stop_count, 1);
    assert_eq!(stopped.playing, 0);
    assert_eq!(stopped.loaded, 1);
    assert_eq!(stopped.active, 1);
    assert_eq!(stopped.updates_enabled, 0);

    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    let mut restarted = McCampaignDayMusicSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_day_music_state(
            handle,
            1,
            0,
            &raw mut restarted,
            struct_size::<McCampaignDayMusicSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(restarted.start_count, 1);
    assert_eq!(restarted.stop_count, 0);
    assert_eq!(restarted.state_hash, initial.state_hash);

    let mut ended = McCampaignDayMusicSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_day_music_end(
            handle,
            1,
            0,
            &raw mut ended,
            struct_size::<McCampaignDayMusicSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(ended.end_count, 1);
    assert_eq!(ended.playing, 0);
    assert_eq!(ended.loaded, 0);
    assert_eq!(ended.active, 0);
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);

    let no_cache_handle = open_level2_campaign_handle();
    let mut unavailable = McCampaignDayMusicSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_day_music_state(
            no_cache_handle,
            0,
            0,
            &raw mut unavailable,
            struct_size::<McCampaignDayMusicSnapshotV2>(),
        ),
        MC_STATUS_VALIDATION_FAILED
    );
    assert_eq!(
        mc_native_campaign_session_destroy(no_cache_handle),
        MC_STATUS_OK
    );
}

#[test]
fn level2_campaign_public_requested_castle_step_reaches_day_construction() {
    let handle = open_level2_campaign_handle_with_asset_cache(&qualified_level2_day_owner_cache());
    let mut input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command: McCommandV2 {
            flags: 0x40,
            requested_spell: 2,
            ..valid_command()
        },
        player_slot: 0,
        speed_index: 0,
        player_x: 384,
        player_y: 62_080,
        player_z: 256,
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
    assert_eq!(snapshot.generation, 0);
    assert_eq!(snapshot.revision, 1);
    assert!(event_count > 0);
    assert!(
        events[..event_count as usize]
            .iter()
            .all(|event| event.abi_version == MC_NATIVE_ABI_VERSION)
    );
    let first_step_flyer = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Castle proof requires a Level 2 campaign session",
            ));
        };
        Ok((
            reset
                .economy()
                .castle_projectiles()
                .iter()
                .find(|projectile| projectile.active)
                .copied(),
            reset.economy().castle().is_none(),
            reset.economy().player_castle_entity_index(),
        ))
    })
    .expect("read public Castle state after the first step");
    let (first_step_flyer, no_root_after_first_step, first_step_relation) = first_step_flyer;
    let first_step_flyer =
        first_step_flyer.expect("the first public step leaves the class-9/model-10 flyer live");
    assert_eq!(
        (first_step_flyer.source_class, first_step_flyer.source_model),
        (9, 10)
    );
    assert_eq!(
        (
            first_step_flyer.source_byte_43,
            first_step_flyer.source_byte_44
        ),
        (3, 2)
    );
    assert!(
        no_root_after_first_step,
        "the first public step does not allocate the class-3/model-2 root"
    );
    assert_eq!(
        first_step_relation, 0,
        "flyer-only public step keeps Castle relation zero"
    );
    assert!(
        events[..event_count as usize]
            .iter()
            .all(|event| !(event.kind == 8 && event.family == 1)),
        "the first public request does not allocate the Castle root"
    );
    assert_eq!(
        events[..event_count as usize]
            .iter()
            .filter(|event| event.pass == 0 && event.kind == 8 && event.family == 38)
            .count(),
        1,
        "the higher-slot Castle flyer receives its first gate-only callback in the request pass"
    );

    input.command = valid_command();
    event_count = 0;
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
    assert!(
        events[..event_count as usize]
            .iter()
            .all(|event| !(event.kind == 8 && event.family == 38)),
        "the following callback does not republish the already-observed gate-only flyer"
    );
    assert!(
        events[..event_count as usize]
            .iter()
            .any(|event| event.kind == 8 && event.family == 1),
        "the following landing callback allocates the Castle root"
    );
    let second_step_relation = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Castle proof requires a Level 2 campaign session",
            ));
        };
        Ok(reset.economy().player_castle_entity_index())
    })
    .expect("read public Castle relation after the flyer gate callback");
    let mut root_event = events[..event_count as usize]
        .iter()
        .find(|event| event.kind == 8 && event.family == 1)
        .copied();
    for _ in 0..32 {
        if root_event.is_some() {
            break;
        }
        event_count = 0;
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
        root_event = events[..event_count as usize]
            .iter()
            .find(|event| event.kind == 8 && event.family == 1)
            .copied();
        if root_event.is_some() {
            break;
        }
    }
    let root_event = root_event.expect("a later flyer landing allocates the Castle root");
    assert_eq!(root_event.generation, 0);
    assert_ne!(root_event.source_slot, 0);
    assert_eq!(
        second_step_relation, root_event.source_slot,
        "the higher-slot Castle root receives its construction callback in the landing pass"
    );
    let seed_relation = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Castle proof requires a Level 2 campaign session",
            ));
        };
        Ok(reset.economy().player_castle_entity_index())
    })
    .expect("read public Castle relation after seed allocation");
    assert_eq!(
        seed_relation, root_event.source_slot,
        "the live ascending cursor binds the Castle relation in the root allocation pass"
    );

    event_count = 0;
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
    assert!(
        events[..event_count as usize]
            .iter()
            .any(|event| { event.kind == 7 && event.source_slot == root_event.source_slot })
    );
    let construction_relation = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Castle proof requires a Level 2 campaign session",
            ));
        };
        Ok(reset.economy().player_castle_entity_index())
    })
    .expect("read public Castle relation after sub_60480 child allocation");
    assert_eq!(
        construction_relation, root_event.source_slot,
        "successful construction child binds the public Castle relation"
    );
    let mut economy_events = [McCampaignEconomyEventV2::default(); 64];
    let mut economy_event_count = 0;
    assert_eq!(
        mc_native_campaign_economy_events(
            handle,
            economy_events.as_mut_ptr(),
            economy_events.len() as u32,
            struct_size::<McCampaignEconomyEventV2>(),
            &raw mut economy_event_count,
        ),
        MC_STATUS_OK
    );
    assert!(
        economy_events[..economy_event_count as usize]
            .iter()
            .all(|event| event.code != MC_CAMPAIGN_ECONOMY_CASTLE_BUILT)
    );
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level2_campaign_public_family0_fireball_reaches_authored_bee_mailbox() {
    let handle = open_level2_campaign_handle_with_asset_cache(&qualified_level2_day_owner_cache());
    let mut input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command: valid_command(),
        player_slot: 0,
        speed_index: 0,
        ..McCampaignStepInputV2::default()
    };
    let read_bee = || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "Fireball proof requires a Level 2 campaign session",
                ));
            };
            Ok(reset
                .economy()
                .authored_bees()
                .iter()
                .find(|bee| bee.source_entity_index == 119)
                .copied())
        })
        .expect("read public authored Bee state")
    };

    // Qualify the real authored action-23 -> action-18 -> action-17 route
    // using only public player positions. The eight compass probes cover
    // the source Bee's live yaw without a second movement implementation
    // in the C-ABI test.
    let approach_offsets = [
        (1_000_i16, 0_i16),
        (707, 707),
        (0, 1_000),
        (-707, 707),
        (-1_000, 0),
        (-707, -707),
        (0, -1_000),
        (707, -707),
    ];
    for attempt in 0..100_usize {
        let Some(bee) = read_bee() else {
            let _ = step_level2_campaign_public(handle, input);
            continue;
        };
        if bee.action == 17 {
            break;
        }
        if bee.action == 23 {
            let (dx, dy) = approach_offsets[attempt % approach_offsets.len()];
            input.player_x = bee.position.x.wrapping_add_signed(dx);
            input.player_y = bee.position.y.wrapping_add_signed(dy);
            input.player_z = bee.position.z;
        } else {
            input.player_x = 0;
            input.player_y = 0;
            input.player_z = 0;
        }
        let _ = step_level2_campaign_public(handle, input);
    }
    // Keep the public cast pass on a non-acquisition action-17 cadence.
    // The source Bee still performs its ordinary movement callback, but
    // it must not retarget the local player before the Fireball cursor
    // reaches the real target row.
    for _ in 0..32 {
        let Some(bee) = read_bee() else {
            break;
        };
        if bee.action == 17
            && (bee.action_window == 0
                || bee.source_byte_3e
                    % u8::try_from(bee.action_window).expect("positive Bee action window")
                    != 0)
        {
            break;
        }
        input.player_x = 0;
        input.player_y = 0;
        input.player_z = 0;
        input.command = valid_command();
        let _ = step_level2_campaign_public(handle, input);
    }
    let bee = read_bee().expect("action-17 production Bee");
    assert_eq!(bee.action, 17);

    let (entity_fov, player_entity_index) = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Fireball proof requires a Level 2 campaign session",
            ));
        };
        Ok((reset.state.entity_fov, reset.state.entity_index))
    })
    .expect("read public player source state");
    input.player_x = bee.position.x.wrapping_add(256);
    input.player_y = bee.position.y.wrapping_add(360);
    input.player_z = bee.position.z.wrapping_add(280).wrapping_sub(entity_fov);
    input.command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 0x40,
        requested_spell: 0,
        ..valid_command()
    };
    let (_, cast_events) = step_level2_campaign_public(handle, input);
    let projectile = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Fireball proof requires a Level 2 campaign session",
            ));
        };
        reset
            .economy()
            .fireball_projectiles()
            .iter()
            .find(|projectile| projectile.active)
            .copied()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "public family-0 cast did not allocate a Fireball",
                )
            })
    })
    .expect("public family-0 cast allocates a live Fireball");
    assert_eq!(
        (
            projectile.source_class,
            projectile.source_model,
            projectile.action,
            projectile.owner_entity_index,
            projectile.holder_entity_index,
            projectile.source_subspell_index,
            projectile.source_mana,
        ),
        (9, 0, 0, player_entity_index, 115, 250, 20)
    );
    assert!(cast_events.iter().any(|event| {
        event.kind == 8 && event.family == 41 && event.source_slot == projectile.source_entity_index
    }));

    input.command = valid_command();
    let (_, collision_events) = step_level2_campaign_public(handle, input);
    let pending = read_bee().expect("target Bee remains public after collision");
    assert_eq!(pending.pending_damage, 250);
    assert_eq!(pending.pending_attacker_entity_index, player_entity_index);
    assert!(collision_events.iter().any(|event| {
        event.kind == 8 && event.family == 41 && event.source_slot == projectile.source_entity_index
    }));

    let (_, _) = step_level2_campaign_public(handle, input);
    let consumed = read_bee().expect("target Bee remains public after mailbox consume");
    assert_eq!(consumed.life, pending.life - 250);
    // GOLDEN-CHANGE: pinned sub_1F660 -> sub_1BF90 clears word 0x62 (the
    // attacker mailbox) but retains dword 0x5E as the last applied damage
    // payload. The life delta and cleared attacker assertions below
    // discriminate a retained receipt from an unconsumed duplicate hit.
    assert_eq!(consumed.pending_damage, 250);
    assert_eq!(consumed.pending_attacker_entity_index, 0);
    assert_eq!(consumed.damage_attacker_entity_index, player_entity_index);
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level2_campaign_public_castle_requires_mana_then_emits_family38() {
    let handle = open_level2_campaign_handle_with_asset_cache(&qualified_level2_day_owner_cache());
    let mut input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command: valid_command(),
        player_slot: 0,
        speed_index: 0,
        player_x: 384,
        player_y: 62_080,
        player_z: 256,
        ..McCampaignStepInputV2::default()
    };
    let castle_holder_values = || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "Castle proof requires a Level 2 campaign session",
                ));
            };
            reset
                .shared_spells()
                .dynamic_pickups()
                .iter()
                .find(|holder| holder.model == CASTLE_SPELL_FAMILY)
                .map(|holder| (holder.active_value, holder.activation_value))
                .ok_or_else(|| AbiError::new(MC_STATUS_BOUNDS, "missing Castle holder"))
        })
        .expect("read-only Castle holder inspection")
    };
    let initial_hud = campaign_day_hud_public(handle, 0, 0);
    assert!(
        initial_hud.local_player_mana >= 1_000,
        "the qualified Day session starts with enough mana to pay the public Possession cost"
    );
    input.command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 0x41,
        requested_spell: 1,
        ..McCommandV2::default()
    };
    let (_, possession_arm_events) = step_level2_campaign_public(handle, input);
    assert!(
        possession_arm_events
            .iter()
            .any(|event| event.pass == 0 && event.kind == 8 && event.family == 39),
        "the public Possession request reaches holder 116 and its live flyer callback in the same source pass"
    );
    input.command = valid_command();
    let (mut below_threshold_snapshot, _) = step_level2_campaign_public(handle, input);
    let mut below_threshold_hud = campaign_day_hud_public(
        handle,
        below_threshold_snapshot.generation,
        below_threshold_snapshot.revision,
    );
    for _ in 0..2 {
        if below_threshold_hud.local_player_mana < 1_000 {
            break;
        }
        (below_threshold_snapshot, _) = step_level2_campaign_public(handle, input);
        below_threshold_hud = campaign_day_hud_public(
            handle,
            below_threshold_snapshot.generation,
            below_threshold_snapshot.revision,
        );
    }
    assert!(
        below_threshold_hud.local_player_mana < 1_000,
        "an ordinary public Possession source/upkeep turn exposes the below-1000 Castle gate"
    );
    input.command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 0x40,
        requested_spell: 2,
        ..McCommandV2::default()
    };
    let (below_threshold_snapshot, below_threshold_events) =
        step_level2_campaign_public(handle, input);
    assert!(
        below_threshold_events
            .iter()
            .all(|event| !(event.kind == 8 && event.family == 38)),
        "Castle must fail closed before the public Day HUD reaches 1000 mana"
    );
    assert_eq!(
        castle_holder_values().0,
        0,
        "below-threshold public Castle request leaves the holder inactive"
    );

    let mut snapshot = below_threshold_snapshot;
    input.command = valid_command();
    for _ in 0..16 {
        let hud = campaign_day_hud_public(handle, snapshot.generation, snapshot.revision);
        if hud.local_player_mana >= 1_000 {
            break;
        }
        (snapshot, _) = step_level2_campaign_public(handle, input);
    }
    let recovered_hud = campaign_day_hud_public(handle, snapshot.generation, snapshot.revision);
    assert!(
        recovered_hud.local_player_mana >= 1_000,
        "ordinary idle public turns recover the current Day mana to the Castle threshold"
    );

    input.command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 0x40,
        requested_spell: 2,
        ..McCommandV2::default()
    };
    let (_armed_snapshot, arm_events) = step_level2_campaign_public(handle, input);
    assert_eq!(
        arm_events
            .iter()
            .filter(|event| event.pass == 0 && event.kind == 8 && event.family == 38)
            .count(),
        1,
        "the accepted Castle request dispatches one live flyer callback in the same source pass"
    );
    assert!(
        arm_events
            .iter()
            .all(|event| !(event.kind == 8 && event.family == 1))
    );
    assert_eq!(
        castle_holder_values().1,
        101,
        "the Castle source activation field retains its source value"
    );
    assert_eq!(
        castle_holder_values().0,
        100,
        "the successful public Castle step decrements 101 to 100"
    );
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}
