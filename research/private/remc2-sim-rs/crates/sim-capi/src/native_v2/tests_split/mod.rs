use super::*;
use sim_core::{
    FlightWorld,
    level1_mission::{
        ABILITY_IMPROVED_NAME_SELECTOR_BASE, ABILITY_IMPROVED_NOTIFICATION_STATE,
        ABILITY_IMPROVED_NOTIFICATION_TIMER, ABILITY_IMPROVED_TEMPLATE_SELECTOR,
    },
};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[repr(C, align(8))]
struct AlignedAbiHeader {
    abi_version: u32,
    struct_size: u32,
}

fn source_presentation_input() -> McLevel1PresentationInputV2 {
    let mut input = McLevel1PresentationInputV2 {
        authority_mask: LIVE_MENU_AUTH_ALL,
        load_available: 1,
        sound_available: 1,
        music_available: 1,
        options_enabled_mask: 0x1ffff,
        sound_volume: 127,
        music_volume: 127,
        volume_target: 1,
        text_colour_phase: 1,
        debug_clock_bucket: 17,
        sound_number: 46,
        debug_heap_size: 0,
        debug_memory_used: 0,
        debug_memory_free: 0,
        ..McLevel1PresentationInputV2::default()
    };
    for (index, name) in [
        b"Zanzamar".as_slice(),
        b"Nyphur",
        b"Rahn",
        b"Belix",
        b"Jark",
        b"Elyssia",
        b"Yragore",
        b"Prish",
    ]
    .into_iter()
    .enumerate()
    {
        let start = index * 16;
        input.chat_names[start..start + name.len()].copy_from_slice(name);
    }
    input.notification_player_name[..8].copy_from_slice(b"Zanzamar");
    input
}

fn submit_source_presentation_input(handle: u64, input: &McLevel1PresentationInputV2) {
    assert_eq!(
        mc_native_level1_presentation_input(handle, input),
        MC_STATUS_OK,
        "{}",
        LAST_ERROR.with(|value| value.borrow().clone())
    );
}

#[test]
fn world_frame_capability_flags_follow_the_passes_that_actually_ran() {
    let without_reflection = world_frame_flags(false, true);
    assert_eq!(
        without_reflection & MC_TERRAIN_FRAME_REFLECTED_SPRITES_COMPOSITED,
        0
    );
    assert_ne!(
        without_reflection & MC_TERRAIN_FRAME_SPRITE_SHADOWS_COMPOSITED,
        0
    );

    let without_shadows = world_frame_flags(true, false);
    assert_ne!(
        without_shadows & MC_TERRAIN_FRAME_REFLECTED_SPRITES_COMPOSITED,
        0
    );
    assert_eq!(
        without_shadows & MC_TERRAIN_FRAME_SPRITE_SHADOWS_COMPOSITED,
        0
    );
}

#[test]
fn paused_native_steps_store_only_the_pre_gate_update_rng_draw() {
    let handle = open_fixture();
    let command = valid_command();
    let mut before = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_snapshot(
            handle,
            &raw mut before,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let (start_rng, start_turn, emitter_life) = with_session(handle, |session| {
        Ok((
            session.mission.global_rng(),
            session.mission.source_turn_counter(),
            session
                .mission
                .ambient_entities()
                .find(|entity| entity.source_entity_index == 113)
                .expect("reset model-60 source slot 113")
                .life,
        ))
    })
    .unwrap();
    assert_eq!(mc_native_level1_game_paused(handle, 1), MC_STATUS_OK);

    let mut expected_rng = start_rng;
    for paused_index in 0..3 {
        let mut paused = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut paused,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        expected_rng = expected_rng.wrapping_mul(9_377).wrapping_add(9_439);
        assert_eq!(
            (paused.tick, paused.state_hash, paused.event_head),
            (before.tick, before.state_hash, before.event_head)
        );
        with_session(handle, |session| {
            assert_eq!(session.mission.global_rng(), expected_rng);
            assert_eq!(session.mission.source_turn_counter(), start_turn);
            let expected_presentation = match paused_index {
                0 => (2, 1, 0),
                1 => (3, 0, 200),
                2 => (3, 0, 199),
                _ => unreachable!(),
            };
            assert_eq!(
                (
                    session.mission.objective_presentation_palette_mod,
                    session.mission.objective_presentation_phase,
                    session.mission.objective_textbox_ticks,
                ),
                expected_presentation,
                "paused outer presentation remains live exactly once per frame"
            );
            assert_eq!(
                session
                    .mission
                    .ambient_entities()
                    .find(|entity| entity.source_entity_index == 113)
                    .unwrap()
                    .life,
                emitter_life,
                "paused UpdateEntities must not dispatch the later model-60 row"
            );
            Ok(())
        })
        .unwrap();
    }

    assert_eq!(mc_native_level1_game_paused(handle, 0), MC_STATUS_OK);
    let mut resumed = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut resumed,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    for _ in 0..10 {
        expected_rng = expected_rng.wrapping_mul(9_377).wrapping_add(9_439);
    }
    with_session(handle, |session| {
        assert_eq!(session.mission.global_rng(), expected_rng);
        assert_eq!(session.mission.source_turn_counter(), start_turn + 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(resumed.tick, before.tick + 1);
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn paused_objective_sound_stays_prepared_until_the_first_unpaused_pool_flush() {
    let handle = open_fixture();
    let command = valid_command();
    with_session_mut(handle, |session| {
        session.mission.current_stage = 1;
        session.mission.objective_presentation_phase = 1;
        session.mission.objective_presentation_palette_mod = 2;
        Ok(())
    })
    .unwrap();
    assert_eq!(mc_native_level1_game_paused(handle, 1), MC_STATUS_OK);
    let mut paused = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut paused,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let (sequence, prepared) = with_session(handle, |session| {
        let (sequence, command) = session
            .audio_commands
            .iter()
            .find(|(_, command)| command.sound_id == 61)
            .map(|(sequence, command)| (*sequence, *command))
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "paused stage-one presentation did not prepare sound61",
                )
            })?;
        Ok((sequence, command))
    })
    .unwrap();
    assert_eq!(
        prepared.action, 0,
        "paused PlayEntitySounds gate retains the row"
    );
    assert_eq!(prepared.pooled_flush_rank, 0);
    assert_eq!(prepared.reserved, [0; 4]);

    assert_eq!(mc_native_level1_game_paused(handle, 0), MC_STATUS_OK);
    let mut resumed = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut resumed,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    with_session(handle, |session| {
        let resolved = session.audio_commands[&sequence];
        assert_ne!(resolved.action, 0);
        assert_eq!(
            resolved.pooled_flush_rank, 2,
            "retained sound61 follows the lower-ID current-frame winner"
        );
        assert_ne!(resolved.reserved[0], 0);
        assert_eq!(resolved.reserved[1..], [0; 3]);
        Ok(())
    })
    .unwrap();
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn playable_menu_transition_and_sound_slider_preview_use_distinct_source_flushes() {
    let handle = open_fixture();
    let initial = source_presentation_input();
    submit_source_presentation_input(handle, &initial);
    assert_eq!(mc_native_level1_player_menu_state(handle, 6), MC_STATUS_OK);
    let menu_sequence = with_session(handle, |session| {
        let (sequence, command) = session
            .audio_commands
            .iter()
            .find(|(_, command)| command.sound_id == 14)
            .map(|(sequence, command)| (*sequence, *command))
            .expect("menu transition prepares local sound14");
        assert_eq!(command.action, 0);
        Ok(sequence)
    })
    .unwrap();
    with_session_mut(handle, |session| {
        session.drain_pending_audio_commands();
        assert_ne!(session.audio_commands[&menu_sequence].action, 0);
        Ok(())
    })
    .unwrap();

    assert_eq!(mc_native_level1_player_menu_state(handle, 10), MC_STATUS_OK);
    let mut changed = initial;
    changed.sound_volume = 126;
    submit_source_presentation_input(handle, &changed);
    with_session(handle, |session| {
        let preview = session
            .audio_commands
            .values()
            .filter(|command| command.sound_id == 8)
            .max_by_key(|command| command.event_sequence)
            .expect("sound slider change prepares preview sound8");
        assert_ne!(preview.action, 0);
        assert_ne!(preview.pooled_flush_rank, 0);
        assert_ne!(preview.reserved[0], 0);
        Ok(())
    })
    .unwrap();
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn presentation_frame_abi_layout_is_stable() {
    assert_eq!(size_of::<McPresentationFrameQueryV2>(), 64);
    assert_eq!(size_of::<McLevel1PresentationInputV2>(), 740);
    assert_eq!(size_of::<McPresentationFrameInfoV2>(), 128);
    assert_eq!(
        std::mem::offset_of!(McPresentationFrameQueryV2, configured_blur_mode),
        40
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1PresentationInputV2, mouse_x),
        38
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1PresentationInputV2, debug_clock_bucket),
        704
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
}

#[test]
fn presentation_frame_copy_is_idempotent_for_same_generation_and_revision() {
    let handle = open_fixture_with_presentation_cache();
    let query = valid_presentation_frame_query();
    let mut indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
    let mut provenance = vec![0_u8; LEVEL1_VIEW_PIXELS];
    let mut info = McPresentationFrameInfoV2::default();
    let copy = |indices: &mut [u8], provenance: &mut [u8], info: &mut McPresentationFrameInfoV2| {
        mc_native_session_presentation_frame_copy(
            handle,
            &raw const query,
            indices.as_mut_ptr(),
            indices.len() as u32,
            provenance.as_mut_ptr(),
            provenance.len() as u32,
            info,
            struct_size::<McPresentationFrameInfoV2>(),
        )
    };

    assert_eq!(copy(&mut indices, &mut provenance, &mut info), MC_STATUS_OK);
    assert_eq!(info.history_state, 1);
    assert_eq!(info.newly_committed, 1);
    let first_indices = indices.clone();
    let first_provenance = provenance.clone();
    let first_info = info;

    assert_eq!(copy(&mut indices, &mut provenance, &mut info), MC_STATUS_OK);
    assert_eq!(info.history_state, first_info.history_state);
    assert_eq!(info.newly_committed, 0);
    assert_eq!(info.immutable_input_hash, first_info.immutable_input_hash);
    assert_eq!(info.index_hash, first_info.index_hash);
    assert_eq!(info.provenance_hash, first_info.provenance_hash);
    assert_eq!(indices, first_indices);
    assert_eq!(provenance, first_provenance);
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn presentation_frame_rejects_stale_generation_without_committing() {
    let handle = open_fixture_with_presentation_cache();
    let mut before_reset = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_snapshot(
            handle,
            &raw mut before_reset,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let mut after_reset = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_reset(
            handle,
            &raw mut after_reset,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_ne!(after_reset.generation, before_reset.generation);

    let mut query = valid_presentation_frame_query();
    query.generation = before_reset.generation;
    let mut indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
    let mut provenance = vec![0_u8; LEVEL1_VIEW_PIXELS];
    let sentinel = McPresentationFrameInfoV2 {
        generation: 0xdead_beef,
        ..McPresentationFrameInfoV2::default()
    };
    let mut info = sentinel;
    assert_eq!(
        mc_native_session_presentation_frame_copy(
            handle,
            &raw const query,
            indices.as_mut_ptr(),
            indices.len() as u32,
            provenance.as_mut_ptr(),
            provenance.len() as u32,
            &raw mut info,
            struct_size::<McPresentationFrameInfoV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(info, sentinel, "stale generation leaves output untouched");
    with_session(handle, |session| {
        assert_eq!(session.generation, after_reset.generation);
        assert_eq!(session.presentation_frame.history(), MainHistory::Unprimed);
        assert!(session.presentation_frame.cached().is_none());
        Ok(())
    })
    .expect("stale presentation generation remains fail-closed");
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
#[ignore = "production-capacity presentation rendering/coverage soak; run explicitly"]
fn presentation_frame_abi_is_revisioned_complete_idempotent_and_fail_closed() {
    assert_eq!(size_of::<McPresentationFrameQueryV2>(), 64);
    assert_eq!(size_of::<McLevel1PresentationInputV2>(), 740);
    assert_eq!(size_of::<McPresentationFrameInfoV2>(), 128);
    assert_eq!(
        std::mem::offset_of!(McPresentationFrameQueryV2, configured_blur_mode),
        40
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1PresentationInputV2, mouse_x),
        38
    );
    assert_eq!(
        std::mem::offset_of!(McLevel1PresentationInputV2, debug_clock_bucket),
        704
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

    let handle = open_fixture_with_presentation_cache();
    let mut query = McPresentationFrameQueryV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McPresentationFrameQueryV2>(),
        generation: 1,
        tick: 0,
        presentation_revision: 1,
        viewport_width: LEVEL1_VIEW_WIDTH as u16,
        viewport_height: LEVEL1_VIEW_HEIGHT as u16,
        screen_stride: LEVEL1_VIEW_WIDTH as u16,
        reserved_0: 0,
        configured_blur_mode: 0,
        blur_capability: 0,
        high_speed_system: 1,
        scratch_present: 1,
        vga_resolution: 1,
        spatial_smoothing: 0,
        minimap_enabled: 1,
        top_bar_enabled: 1,
        menu_overlay: 0,
        game_paused: 0,
        level_end: 0,
        objective_selector: u8::MAX,
        objective_textbox: 0,
        tutorial_or_briefing: 0,
        help_popup: 0,
        debug_text: 0,
        reserved: [0; 2],
    };
    let mut indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
    let mut provenance = vec![0_u8; LEVEL1_VIEW_PIXELS];
    let mut info = McPresentationFrameInfoV2::default();
    let copy = |query: &McPresentationFrameQueryV2,
                indices: &mut [u8],
                provenance: &mut [u8],
                info: &mut McPresentationFrameInfoV2| {
        mc_native_session_presentation_frame_copy(
            handle,
            query,
            indices.as_mut_ptr(),
            indices.len() as u32,
            provenance.as_mut_ptr(),
            provenance.len() as u32,
            info,
            struct_size::<McPresentationFrameInfoV2>(),
        )
    };
    let capture = |capture_handle: u64, capture_query: &McPresentationFrameQueryV2| {
        let mut capture_indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut capture_provenance = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut capture_info = McPresentationFrameInfoV2::default();
        let status = mc_native_session_presentation_frame_copy(
            capture_handle,
            capture_query,
            capture_indices.as_mut_ptr(),
            capture_indices.len() as u32,
            capture_provenance.as_mut_ptr(),
            capture_provenance.len() as u32,
            &raw mut capture_info,
            struct_size::<McPresentationFrameInfoV2>(),
        );
        (status, capture_indices, capture_provenance, capture_info)
    };
    let semantic_vectors = with_session(handle, |session| {
        crate::presentation_bridge::assert_independent_source_vectors(
            &session
                .terrain_frame_assets
                .as_ref()
                .expect("qualified presentation cache")
                .indexed_overlays,
        );
        Ok(session
            .terrain_frame_assets
            .as_ref()
            .expect("qualified presentation cache")
            .indexed_overlays
            .source_semantic_vectors()
            .clone())
    })
    .expect("source-semantic vector ledger");
    assert_eq!(
        semantic_vectors["palette_indirection"]["indexed_values"]
            .as_array()
            .expect("palette vector")
            .iter()
            .map(|value| value.as_i64().expect("palette integer"))
            .collect::<Vec<_>>(),
        [61, 23, 171, 87, 177, 119, 176]
    );
    assert_eq!(
        semantic_vectors["pause"]["selector_order"]
            .as_array()
            .expect("pause vector")
            .iter()
            .map(|value| value.as_i64().expect("pause selector integer"))
            .collect::<Vec<_>>(),
        [174, 175, 176, 177, 178]
    );
    let button_rows = semantic_vectors["chat"]["button_order"]
        .as_array()
        .expect("button vector rows");
    assert_eq!(
        button_rows
            .iter()
            .flat_map(|row| {
                row.as_array()
                    .expect("button tuple")
                    .iter()
                    .map(|value| value.as_i64().expect("button integer"))
            })
            .collect::<Vec<_>>(),
        [259, 192, 166, 261, 237, 166, 260, 282, 166]
    );
    assert_eq!(
        semantic_vectors["options"]["on_ids"]
            .as_array()
            .map(Vec::len),
        Some(17)
    );
    assert_eq!(
        semantic_vectors["volume"]["source_bar"]
            .as_array()
            .expect("volume vector")
            .iter()
            .map(|value| value.as_i64().expect("volume integer"))
            .collect::<Vec<_>>(),
        [274, 210, 90, 12]
    );
    assert_eq!(
        semantic_vectors["ok_cancel"]["fixed_prompts"]
            .as_array()
            .expect("prompt vector")
            .iter()
            .map(|value| value.as_str().expect("prompt string"))
            .collect::<Vec<_>>(),
        ["Abandon level.?", "Load Level?", "Save Level?"]
    );
    assert_eq!(
        semantic_vectors["pause_or_end"]["end_text"],
        "Tasks completed. Fly to the exit point.."
    );
    assert_eq!(
        semantic_vectors["map"]["divider_order"]
            .as_array()
            .expect("divider vector")
            .iter()
            .map(|value| value.as_i64().expect("divider integer"))
            .collect::<Vec<_>>(),
        [192, 191]
    );
    assert_eq!(semantic_vectors["bottom"]["tier_spacing"], 33);
    assert_eq!(semantic_vectors["debug"]["version"], "Beta");
    assert_eq!(semantic_vectors["cursor"]["active_policy"], "fail_closed");
    assert_eq!(
        copy(&query, &mut indices, &mut provenance, &mut info),
        MC_STATUS_OK
    );
    let complete_flags = MC_PRESENTATION_FRAME_COMPOSITOR_COMPLETE
        | MC_PRESENTATION_FRAME_HAS_PROVENANCE
        | MC_PRESENTATION_FRAME_OVERLAYS_COMPOSITED
        | MC_PRESENTATION_FRAME_POSTPROCESS_COMPLETE
        | MC_PRESENTATION_FRAME_LIGHTING_COMPLETE;
    assert_eq!(info.flags & complete_flags, complete_flags);
    assert_eq!(
        info.flags & (MC_TERRAIN_FRAME_ISOLATED_SKY_TERRAIN | MC_TERRAIN_FRAME_NON_TERRAIN_OMITTED),
        0
    );
    assert_eq!((info.history_state, info.newly_committed), (1, 1));
    assert_eq!(
        sim_core::presentation_frame::byte_hash(&indices),
        info.index_hash
    );
    assert_eq!(
        sim_core::presentation_frame::byte_hash(&provenance),
        info.provenance_hash
    );
    assert!(provenance.iter().any(|value| value & 1 != 0));
    assert!(provenance.iter().any(|value| value & 8 != 0));
    assert_ne!(info.immutable_input_hash, 0);
    let first_indices = indices.clone();
    let first_provenance = provenance.clone();
    let first_info = info;

    assert_eq!(
        copy(&query, &mut indices, &mut provenance, &mut info),
        MC_STATUS_OK
    );
    assert_eq!(info.newly_committed, 0);
    assert_eq!(indices, first_indices);
    assert_eq!(provenance, first_provenance);
    assert_eq!(info.immutable_input_hash, first_info.immutable_input_hash);

    let baseline_source_render_hash =
        with_session(handle, |session| session.source_render_input_hash())
            .expect("baseline source-render hash");
    with_session(handle, |session| {
        let baseline = session.source_render_inputs(false)?;
        let assert_changed = |label: &str, changed: SourceRenderInputs| -> AbiResult<()> {
            assert_ne!(
                session.source_render_input_hash_for(&changed)?,
                baseline_source_render_hash,
                "{label} participates in the complete source-render key"
            );
            Ok(())
        };

        let mut changed = baseline.clone();
        changed.map_right_viewport = !changed.map_right_viewport;
        assert_changed("map render branch", changed)?;
        for (label, mutate) in [
            ("composed camera render_x", 0_u8),
            ("composed camera render_y", 1),
            ("composed camera eye_z", 2),
            ("composed camera yaw", 3),
            ("composed camera pitch", 4),
            ("composed camera roll", 5),
            ("composed camera fov", 6),
            ("composed camera source_turn", 7),
            ("composed camera reflections", 8),
        ] {
            let mut changed = baseline.clone();
            match mutate {
                0 => changed.camera.render_x ^= 1,
                1 => changed.camera.render_y ^= 1,
                2 => changed.camera.eye_z ^= 1,
                3 => changed.camera.yaw ^= 1,
                4 => changed.camera.pitch ^= 1,
                5 => changed.camera.roll ^= 1,
                6 => changed.camera.fov ^= 1,
                7 => changed.camera.source_turn ^= 1,
                8 => changed.camera.reflections = !changed.camera.reflections,
                _ => unreachable!(),
            }
            assert_changed(label, changed)?;
        }
        let mut changed = baseline.clone();
        changed.generation_asset_digest ^= 1;
        assert_changed("generation renderer-asset digest", changed)?;
        let mut changed = baseline.clone();
        changed.local_player_entity_index ^= 1;
        assert_changed("local player identity", changed)?;
        let mut changed = baseline.clone();
        changed.goats[0].position.x ^= 1;
        assert_changed("Goat record", changed)?;
        let mut changed = baseline.clone();
        changed.frozen_actors[0].position.y ^= 1;
        assert_changed("frozen actor record", changed)?;
        let mut changed = baseline.clone();
        changed.ambient_entities[0].phase ^= 1;
        assert_changed("ambient record", changed)?;
        let mut changed = baseline.clone();
        changed.dynamic_residents.push(DynamicResidentState {
            source_entity_index: 983,
            source_id: 983,
            kind: sim_core::level1_mission::DynamicResidentKind::Builder,
            class: 5,
            model: 4,
            action: 96,
            position: Position {
                x: 0x1212,
                y: 0x3434,
                z: 56,
            },
            max_life: 100,
            life: 90,
            pending_damage: 0,
            pending_attacker: 0,
            current_attacker: 0,
            kill_credit_entity: 0,
            entity_rng: 17,
            yaw: 18,
            desired_heading: 19,
            minimum_speed: 20,
            actual_speed: 21,
            maximum_speed: 22,
            target_entity_index: 0,
            linked_entity_index: 0,
            owner_entity_index: 1,
            reset_counter: 23,
            mana: 24,
            stage_var_2: 0,
            builder_placement_latch: 0,
            movement_blocked: false,
            suppress_death_burst: false,
            phase: 25,
            collision_half_width: 26,
            collision_vertical_offset: 27,
            collision_half_height: 28,
            render_parameter_index: 83,
            base_tmap: 80,
            render_mode: 20,
            logical_height: 200,
            active: true,
            visible: true,
        });
        assert_changed("dynamic resident record", changed)?;

        let mut changed = baseline.clone();
        changed.mission_actors[0].position.x ^= 1;
        changed.mission_actors[0].yaw ^= 1;
        changed.mission_actors[0].visible = !changed.mission_actors[0].visible;
        assert_changed("actor position/yaw/visibility", changed)?;

        let mut changed = baseline.clone();
        changed.mission_segments.push(Level1MultipartSegment {
            source_entity_index: 991,
            head_authored_index: 9,
            head_source_entity_index: 990,
            predecessor_source_entity_index: 990,
            successor_source_entity_index: 0,
            ordinal: 1,
            class: 5,
            model: 3,
            action: 0,
            owner_id: 1,
            position: Position {
                x: 0x1234,
                y: 0x5678,
                z: 90,
            },
            life: 100,
            pending_damage: 0,
            pending_attacker: 0,
            yaw: 321,
            pitch: -17,
            phase: 4,
            trailing_distance: 64,
            collision_half_width: 30,
            collision_half_height: 40,
            render_parameter_index: 83,
            base_tmap: 80,
            render_mode: 20,
            logical_height: 200,
            active: true,
            visible: true,
        });
        assert_changed("multipart segment", changed)?;

        let mut changed = baseline.clone();
        changed.castle_root = Some(Level1CastleRootRender {
            source_entity_index: 989,
            position: Position {
                x: 0x4321,
                y: 0x8765,
                z: 111,
            },
            active: true,
        });
        assert_changed("castle root", changed)?;

        let mut changed = baseline.clone();
        changed.mana_spheres.push(ManaSphere {
            id: 988,
            source_entity_index: 988,
            entity_rng: 17,
            position: Position {
                x: 0x1111,
                y: 0x2222,
                z: 33,
            },
            velocity_x: 1,
            velocity_y: 2,
            vertical_velocity: 3,
            yaw: 4,
            mana: 5,
            owner_entity_index: 0,
            player_entity_index: 0,
            incoming_owner_entity_index: 0,
            incoming_possession_payload: 0,
            attraction_source_entity_index: 0,
            attraction_step: 0,
            attraction_distance_high: 0,
            assigned_carrier_id: 0,
            assigned_defender_slot: 0,
            control_flags: 0,
            phase: 6,
            active: true,
            pending_delete: false,
        });
        assert_changed("mana sphere", changed)?;

        let mut changed = baseline.clone();
        changed.mana_carrier = Some(ManaCarrier {
            id: 987,
            source_entity_index: 987,
            entity_rng: 18,
            owner_entity_index: 1,
            position: Position {
                x: 0x3333,
                y: 0x4444,
                z: 55,
            },
            yaw: 6,
            pitch: 7,
            speed: 8,
            life: 9,
            max_life: 10,
            max_mana: 11,
            target_sphere_id: 988,
            carried_mana: 12,
            returning_to_castle: false,
            pending_damage: 13,
            pending_attacker_entity_index: 14,
            active: true,
        });
        assert_changed("mana carrier", changed)?;

        let mut changed = baseline.clone();
        changed.projectiles.push(Level1Projectile::PlayerFireball(
            sim_core::level1_mission::FireballProjectile {
                id: 1,
                source_entity_index: 986,
                owner_entity_index: 1,
                position: Position {
                    x: 0x5555,
                    y: 0x6666,
                    z: 77,
                },
                yaw: 8,
                pitch: 9,
                speed: 10,
                life_ticks: 11,
                payload: 12,
                target_entity_index: 0,
                acquisition_initialized: true,
                active: true,
            },
        ));
        assert_changed("projectile", changed)?;

        let mut changed = baseline.clone();
        changed.impacts.push(FireballImpact {
            id: 2,
            source_entity_index: 985,
            source_projectile_id: 1,
            owner_entity_index: 1,
            external_target_entity_index: 0,
            model: FIREBALL_IMPACT_MODEL,
            position: Position {
                x: 0x7777,
                y: 0x8888,
                z: 99,
            },
            payload: 14,
            life_ticks: 15,
            entity_rng: 16,
            vertical_velocity: 17,
            phase: 18,
            animation_frame: 19,
            damage_submitted: false,
            active: true,
        });
        assert_changed("impact", changed)?;

        let mut changed = baseline.clone();
        changed.possessed_buildings.push(Level1PossessedBuilding {
            entity_index: 984,
            position: Position {
                x: 0x9999,
                y: 0xaaaa,
                z: 121,
            },
            owner_entity_index: 1,
            player_color_index: 2,
            render_parameter_index: 177,
            base_tmap: 96,
            render_mode: 20,
            logical_height: 250,
            active: true,
        });
        assert_changed("possessed building", changed)?;

        let mut changed = baseline.clone();
        changed.source_slots[0].phase ^= 1;
        assert_changed("source-slot record", changed)?;

        let mut changed = baseline.clone();
        changed.dispositions.push(SourceSpriteDisposition::Legacy {
            source_entity_index: 983,
            family: SourceSpriteLegacyFamily::ImportedReset,
        });
        assert_changed("normalized source disposition", changed)?;

        let mut ordered = baseline.clone();
        for entity_index in [980_u16, 981] {
            ordered
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index,
                    active: true,
                    x: 0x2222,
                    y: 0x3333,
                })
                .expect("synthetic same-tile source-map insert");
        }
        let ordered_hash = session.source_render_input_hash_for(&ordered)?;
        let mut reordered = ordered.clone();
        reordered
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: 980,
                active: true,
                x: 0x2322,
                y: 0x3333,
            })
            .expect("temporary adjacent-tile move");
        reordered
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: 980,
                active: true,
                x: 0x2222,
                y: 0x3333,
            })
            .expect("same-tile head reinsert");
        assert_ne!(
            session.source_render_input_hash_for(&reordered)?,
            ordered_hash,
            "same membership and positions with different source-map head order changes the key"
        );
        assert_changed("same-tile source-map painter order", reordered)?;

        let mut changed = baseline;
        changed.night = !changed.night;
        assert_changed("night palette profile", changed)?;
        Ok(())
    })
    .expect("complete source-render projection mutation vectors");
    let generation_asset_digest = with_session_mut(handle, |session| {
        let digest = session
            .source_render_asset_digest
            .expect("qualified generation binds its renderer assets");
        session.source_render_asset_digest = Some(digest ^ 1);
        Ok(digest)
    })
    .expect("mutate only the generation-bound renderer digest");
    assert_eq!(
        copy(&query, &mut indices, &mut provenance, &mut info),
        MC_STATUS_GENERATION_MISMATCH,
        "renderer asset-bank drift rejects before cached return"
    );
    with_session(handle, |session| {
        assert_eq!(
            session
                .presentation_frame
                .cached()
                .expect("asset drift preserves the prior COMPLETE cache")
                .immutable_input_hash,
            first_info.immutable_input_hash
        );
        Ok(())
    })
    .expect("asset drift is atomic");
    with_session_mut(handle, |session| {
        session.source_render_asset_digest = Some(generation_asset_digest);
        Ok(())
    })
    .expect("restore generation-bound renderer digest");
    assert_eq!(
        copy(&query, &mut indices, &mut provenance, &mut info),
        MC_STATUS_OK,
        "restored renderer identity permits the exact cached retry"
    );
    assert_eq!(info.newly_committed, 0);
    with_session_mut(handle, |session| {
        let baseline = session.terrain.clone();
        session.terrain.dynamic_lighting_planes().1[0] ^= 1;
        assert_ne!(
            session.source_render_input_hash()?,
            baseline_source_render_hash,
            "terrain content participates in the source-render key"
        );
        session.terrain = baseline;

        let baseline = session.live_world.clone();
        session.live_world.release_villager_stage_group_7();
        assert_ne!(
            session.source_render_input_hash()?,
            baseline_source_render_hash,
            "live-world content participates in the source-render key"
        );
        session.live_world = baseline;

        let baseline = session.camera_slope_sample;
        session.camera_slope_sample.offset_x ^= 1;
        assert_ne!(
            session.source_render_input_hash()?,
            baseline_source_render_hash,
            "camera-slope input participates in the source-render key"
        );
        session.camera_slope_sample = baseline;

        let baseline = session.frozen_actors.clone();
        session.frozen_actors[0].position.x ^= 1;
        assert_ne!(
            session.source_render_input_hash()?,
            baseline_source_render_hash,
            "frozen render records participate in the source-render key"
        );
        session.frozen_actors = baseline;

        let assets = Arc::clone(
            session
                .terrain_frame_assets
                .as_ref()
                .expect("qualified source banks"),
        );
        let goat_bank = session
            .world
            .goat_tmap_bank()
            .expect("qualified Goat bank")
            .clone();
        let baseline = session.selector_frames.clone();
        session
            .selector_frames
            .as_mut()
            .expect("Goat clocks")
            .latch_draw_attempts(GOAT_TMAP_DRAW_MASK)
            .expect("valid Goat draw mask");
        session
            .selector_frames
            .as_mut()
            .expect("Goat clocks")
            .service_unpaused_outer_turn(&goat_bank);
        assert_ne!(
            session.source_render_input_hash()?,
            baseline_source_render_hash,
            "Goat current TMAP participates in the source-render key"
        );
        session.selector_frames = baseline;

        let baseline = session.archer_selector_frames.clone();
        session
            .archer_selector_frames
            .as_mut()
            .expect("Archer clocks")
            .latch_draw_attempts(sim_core::archer_tmap::ARCHER_TMAP_DRAW_MASK)
            .expect("valid Archer draw mask");
        session
            .archer_selector_frames
            .as_mut()
            .expect("Archer clocks")
            .service_unpaused_outer_turn(&assets.archer_tmap_bank);
        assert_ne!(
            session.source_render_input_hash()?,
            baseline_source_render_hash,
            "Archer current TMAP participates in the source-render key"
        );
        session.archer_selector_frames = baseline;

        let baseline = session.firefly_selector_frames.clone();
        session
            .firefly_selector_frames
            .as_mut()
            .expect("Firefly clocks")
            .latch_draw_attempts(sim_core::firefly_tmap::FIREFLY_TMAP_DRAW_MASK);
        session
            .firefly_selector_frames
            .as_mut()
            .expect("Firefly clocks")
            .service_unpaused_outer_turn(&assets.firefly_tmap_bank);
        assert_ne!(
            session.source_render_input_hash()?,
            baseline_source_render_hash,
            "Firefly current TMAP participates in the source-render key"
        );
        session.firefly_selector_frames = baseline;

        let baseline = session.villager_selector_frames.clone();
        session
            .villager_selector_frames
            .as_mut()
            .expect("villager clocks")
            .latch_draw_attempts(sim_core::frozen_actor_tmap::VILLAGER_TMAP_DRAW_MASK)
            .expect("valid villager draw mask");
        session
            .villager_selector_frames
            .as_mut()
            .expect("villager clocks")
            .service_unpaused_outer_turn(&assets.frozen_actor_tmap_bank);
        assert_ne!(
            session.source_render_input_hash()?,
            baseline_source_render_hash,
            "villager current TMAP participates in the source-render key"
        );
        session.villager_selector_frames = baseline;

        let baseline = session.fireball_selector_frame.clone();
        session
            .fireball_selector_frame
            .as_mut()
            .expect("Fireball clock")
            .latch_draw_attempt(true);
        session
            .fireball_selector_frame
            .as_mut()
            .expect("Fireball clock")
            .service_unpaused_outer_turn(&assets.combat_tmap_bank);
        assert_ne!(
            session.source_render_input_hash()?,
            baseline_source_render_hash,
            "Fireball current TMAP participates in the source-render key"
        );
        session.fireball_selector_frame = baseline;

        let baseline = session.possession_selector_frame.clone();
        session
            .possession_selector_frame
            .as_mut()
            .expect("Possession clock")
            .latch_draw_attempt(true);
        session
            .possession_selector_frame
            .as_mut()
            .expect("Possession clock")
            .service_unpaused_outer_turn(&assets.combat_tmap_bank);
        assert_ne!(
            session.source_render_input_hash()?,
            baseline_source_render_hash,
            "Possession current TMAP participates in the source-render key"
        );
        session.possession_selector_frame = baseline;

        let baseline = session.castle_selector_frame.clone();
        session
            .castle_selector_frame
            .as_mut()
            .expect("Castle clock")
            .latch_draw_attempt(true);
        session
            .castle_selector_frame
            .as_mut()
            .expect("Castle clock")
            .service_unpaused_outer_turn(&assets.economy_tmap_bank);
        assert_ne!(
            session.source_render_input_hash()?,
            baseline_source_render_hash,
            "Castle current TMAP participates in the source-render key"
        );
        session.castle_selector_frame = baseline;
        assert_eq!(
            session.source_render_input_hash()?,
            baseline_source_render_hash,
            "restoring every renderer input restores the exact key"
        );
        Ok(())
    })
    .expect("source-render domain mutation witnesses");

    let baseline_slope =
        with_session(handle, |session| Ok(session.camera_slope_sample)).expect("baseline slope");
    with_session_mut(handle, |session| {
        session.camera_slope_sample.offset_y ^= 1;
        Ok(())
    })
    .expect("same-tick slope mutation");
    assert_eq!(
        copy(&query, &mut indices, &mut provenance, &mut info),
        MC_STATUS_GENERATION_MISMATCH,
        "same-tick source-render drift rejects before cached reuse"
    );
    with_session(handle, |session| {
        assert_eq!(
            session
                .presentation_frame
                .cached()
                .expect("prior COMPLETE frame remains cached")
                .immutable_input_hash,
            first_info.immutable_input_hash
        );
        Ok(())
    })
    .expect("same-tick rejection preserves the prior cache");
    with_session_mut(handle, |session| {
        session.camera_slope_sample = baseline_slope;
        Ok(())
    })
    .expect("restore slope input");
    assert_eq!(
        copy(&query, &mut indices, &mut provenance, &mut info),
        MC_STATUS_OK,
        "restored same-tick source input permits exact cached retry"
    );
    assert_eq!(info.newly_committed, 0);

    let chain_handle = open_fixture_with_presentation_cache();
    let mut ordered_chains = with_session(chain_handle, |session| {
        Ok(session.renderable_map_chains.clone())
    })
    .expect("isolated baseline painter-map topology");
    for entity_index in [113_u16, 114] {
        ordered_chains
            .sync_entity(RenderableMapSeed {
                entity_index,
                active: true,
                x: 0x2222,
                y: 0x3333,
            })
            .expect("live same-tile source-map insert");
    }
    with_session_mut(chain_handle, |session| {
        session.renderable_map_chains = ordered_chains.clone();
        Ok(())
    })
    .expect("install ordered live painter map");
    let mut chain_indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
    let mut chain_provenance = vec![0_u8; LEVEL1_VIEW_PIXELS];
    let mut chain_info = McPresentationFrameInfoV2::default();
    let chain_copy =
        |indices: &mut [u8], provenance: &mut [u8], info: &mut McPresentationFrameInfoV2| {
            mc_native_session_presentation_frame_copy(
                chain_handle,
                &raw const query,
                indices.as_mut_ptr(),
                indices.len() as u32,
                provenance.as_mut_ptr(),
                provenance.len() as u32,
                info,
                struct_size::<McPresentationFrameInfoV2>(),
            )
        };
    assert_eq!(
        chain_copy(&mut chain_indices, &mut chain_provenance, &mut chain_info),
        MC_STATUS_OK,
        "ordered same-tile live map establishes the immutable baseline"
    );
    let ordered_info = chain_info;
    let mut reordered_chains = ordered_chains.clone();
    reordered_chains
        .sync_entity(RenderableMapSeed {
            entity_index: 113,
            active: true,
            x: 0x2322,
            y: 0x3333,
        })
        .expect("temporary live adjacent-tile move");
    reordered_chains
        .sync_entity(RenderableMapSeed {
            entity_index: 113,
            active: true,
            x: 0x2222,
            y: 0x3333,
        })
        .expect("live same-tile head reinsert");
    with_session_mut(chain_handle, |session| {
        session.renderable_map_chains = reordered_chains;
        Ok(())
    })
    .expect("install reordered live painter map");
    assert_eq!(
        chain_copy(&mut chain_indices, &mut chain_provenance, &mut chain_info),
        MC_STATUS_GENERATION_MISMATCH,
        "same-tick painter order drift rejects before cached return"
    );
    with_session(chain_handle, |session| {
        assert_eq!(
            session
                .presentation_frame
                .cached()
                .expect("prior frame survives chain rejection")
                .immutable_input_hash,
            ordered_info.immutable_input_hash
        );
        Ok(())
    })
    .expect("chain rejection preserves history/cache/clocks");
    with_session_mut(chain_handle, |session| {
        session.renderable_map_chains = ordered_chains;
        Ok(())
    })
    .expect("restore painter-map order");
    assert_eq!(
        chain_copy(&mut chain_indices, &mut chain_provenance, &mut chain_info),
        MC_STATUS_OK,
        "restored same-tile order returns the exact cached frame"
    );
    assert_eq!(chain_info.newly_committed, 0);
    assert_eq!(
        mc_native_session_destroy(chain_handle),
        MC_STATUS_OK,
        "isolated source-map reorder session closes"
    );

    let baseline_castle = with_session(handle, |session| {
        Ok((
            session.mission.castle_exists,
            session.mission.castle_source_entity_index,
            session.mission.castle_position,
        ))
    })
    .expect("baseline castle root");
    with_session_mut(handle, |session| {
        session.mission.castle_exists = true;
        session.mission.castle_source_entity_index = 989;
        session.mission.castle_position = Position {
            x: 0x3456,
            y: 0x789a,
            z: 88,
        };
        Ok(())
    })
    .expect("mutate a typed castle render record");
    assert_eq!(
        copy(&query, &mut indices, &mut provenance, &mut info),
        MC_STATUS_GENERATION_MISMATCH,
        "same-tick typed castle record drift rejects before cached return"
    );
    with_session_mut(handle, |session| {
        (
            session.mission.castle_exists,
            session.mission.castle_source_entity_index,
            session.mission.castle_position,
        ) = baseline_castle;
        Ok(())
    })
    .expect("restore typed castle render record");
    assert_eq!(
        copy(&query, &mut indices, &mut provenance, &mut info),
        MC_STATUS_OK
    );
    assert_eq!(info.newly_committed, 0);

    let isolated_handle = open_fixture_with_presentation_cache();
    let mut isolated_indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
    let mut isolated_provenance = vec![0_u8; LEVEL1_VIEW_PIXELS];
    let mut isolated_info = McPresentationFrameInfoV2::default();
    assert_eq!(
        mc_native_session_presentation_frame_copy(
            isolated_handle,
            &raw const query,
            isolated_indices.as_mut_ptr(),
            isolated_indices.len() as u32,
            isolated_provenance.as_mut_ptr(),
            isolated_provenance.len() as u32,
            &raw mut isolated_info,
            struct_size::<McPresentationFrameInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(isolated_indices, first_indices);
    assert_eq!(isolated_provenance, first_provenance);
    assert_eq!(
        isolated_info.immutable_input_hash,
        first_info.immutable_input_hash
    );

    let unauthoritative_debug_handle = open_fixture_with_presentation_cache();
    let mut unauthoritative_debug_query = query;
    unauthoritative_debug_query.debug_text = 8;
    assert_eq!(
        capture(unauthoritative_debug_handle, &unauthoritative_debug_query).0,
        MC_STATUS_GENERATION_MISMATCH,
        "active debug metrics cannot earn COMPLETE from reset zeros"
    );
    assert_eq!(
        mc_native_session_destroy(unauthoritative_debug_handle),
        MC_STATUS_OK
    );

    let debug_handle = open_fixture_with_presentation_cache();
    let mut debug_input = source_presentation_input();
    submit_source_presentation_input(debug_handle, &debug_input);
    with_session_mut(debug_handle, |session| {
        session.source_turn = session.source_turn.wrapping_add(37);
        Ok(())
    })
    .expect("advance authoritative source turn within the next clock bucket");
    debug_input.debug_clock_bucket += 1;
    submit_source_presentation_input(debug_handle, &debug_input);
    with_session(debug_handle, |session| {
        assert_eq!(
            session.debug_fps, 37,
            "source clock bucket transition records turn delta as FPS"
        );
        assert_eq!(session.debug_heap_size, 0x0140_0000);
        assert_eq!(session.debug_memory_used, 0);
        assert_eq!(session.debug_memory_free, 0);
        Ok(())
    })
    .expect("debug FPS transition is authoritative");
    let mut debug_query = query;
    debug_query.debug_text = 8;
    let (status, debug_indices, debug_provenance, debug_info) = capture(debug_handle, &debug_query);
    assert_eq!(
        status,
        MC_STATUS_OK,
        "{}",
        LAST_ERROR.with(|value| value.borrow().clone())
    );
    assert_ne!(
        debug_info.immutable_input_hash,
        first_info.immutable_input_hash
    );
    assert_ne!(debug_info.index_hash, first_info.index_hash);
    assert_ne!(debug_indices, first_indices);
    assert_ne!(debug_provenance, first_provenance);
    assert_eq!(mc_native_session_destroy(debug_handle), MC_STATUS_OK);

    let entity_debug_handle = open_fixture_with_presentation_cache();
    let mut entity_debug_query = query;
    entity_debug_query.debug_text = 0x40;
    let (entity_debug_status, entity_debug_indices, _, _) =
        capture(entity_debug_handle, &entity_debug_query);
    assert_eq!(
        entity_debug_status,
        MC_STATUS_OK,
        "{}",
        LAST_ERROR.with(|value| value.borrow().clone())
    );
    assert_ne!(
        entity_debug_indices, first_indices,
        "debug flag0x40 traverses the selected live entity rows"
    );
    assert_eq!(mc_native_session_destroy(entity_debug_handle), MC_STATUS_OK);

    let hud_handle = open_fixture_with_presentation_cache();
    with_session_mut(hud_handle, |session| {
        session.mission.player_mana -= 16_384;
        Ok(())
    })
    .expect("mutate live HUD mana before the production bridge");
    let (status, hud_indices, _, hud_info) = capture(hud_handle, &query);
    assert_eq!(status, MC_STATUS_OK);
    assert_ne!(
        hud_info.immutable_input_hash,
        first_info.immutable_input_hash
    );
    assert_ne!(hud_info.index_hash, first_info.index_hash);
    assert_ne!(hud_indices, first_indices);
    assert_eq!(mc_native_session_destroy(hud_handle), MC_STATUS_OK);

    let objective_48 = open_fixture_with_presentation_cache();
    let objective_49 = open_fixture_with_presentation_cache();
    for (handle, selector) in [(objective_48, 48_u8), (objective_49, 49_u8)] {
        with_session_mut(handle, |session| {
            session.mission.current_text_selector = selector;
            session.mission.objective_textbox_ticks = 33;
            Ok(())
        })
        .expect("configure a live objective textbox");
    }
    let (status_48, objective_indices_48, objective_provenance_48, info_48) =
        capture(objective_48, &query);
    let (status_49, objective_indices_49, objective_provenance_49, info_49) =
        capture(objective_49, &query);
    assert_eq!(
        (status_48, status_49),
        (MC_STATUS_OK, MC_STATUS_OK),
        "{}",
        LAST_ERROR.with(|value| value.borrow().clone())
    );
    assert_ne!(info_48.immutable_input_hash, info_49.immutable_input_hash);
    assert_ne!(info_48.index_hash, info_49.index_hash);
    assert_ne!(objective_indices_48, objective_indices_49);
    assert_ne!(objective_provenance_48, objective_provenance_49);
    assert_eq!(mc_native_session_destroy(objective_48), MC_STATUS_OK);
    assert_eq!(mc_native_session_destroy(objective_49), MC_STATUS_OK);

    let menu_handle = open_fixture_with_presentation_cache();
    assert_eq!(
        mc_native_level1_player_menu_state(menu_handle, 5),
        MC_STATUS_OK
    );
    let (menu_status, menu_indices, _, menu_info) = capture(menu_handle, &query);
    assert_eq!(menu_status, MC_STATUS_OK);
    assert_ne!(
        menu_info.immutable_input_hash,
        first_info.immutable_input_hash
    );
    assert_ne!(menu_info.index_hash, first_info.index_hash);
    assert_ne!(menu_indices, first_indices);
    assert_eq!(mc_native_session_destroy(menu_handle), MC_STATUS_OK);

    let unauthoritative_chat_handle = open_fixture_with_presentation_cache();
    assert_eq!(
        mc_native_level1_player_menu_state(unauthoritative_chat_handle, 3),
        MC_STATUS_OK
    );
    assert_eq!(
        capture(unauthoritative_chat_handle, &query).0,
        MC_STATUS_GENERATION_MISMATCH,
        "active chat cannot earn COMPLETE from the inert reset UI state"
    );
    assert_eq!(
        mc_native_session_destroy(unauthoritative_chat_handle),
        MC_STATUS_OK
    );

    let acquired_handle = open_fixture_with_presentation_cache();
    assert_eq!(
        mc_native_level1_player_menu_state(acquired_handle, 5),
        MC_STATUS_OK
    );
    with_session_mut(acquired_handle, |session| {
        let fire = session
            .context
            .spell_fire
            .as_mut()
            .expect("Level 1 spell registry");
        fire.selected_spell_ui_index = 4;
        fire.spell_experience[4] = 1;
        fire.enabled_holder_indices[4] = 0;
        Ok(())
    })
    .expect("configure acquired family without a holder");
    let (acquired_status, _, _, _) = capture(acquired_handle, &query);
    assert_eq!(
        acquired_status,
        MC_STATUS_OK,
        "{}",
        LAST_ERROR.with(|value| value.borrow().clone())
    );
    assert_eq!(mc_native_session_destroy(acquired_handle), MC_STATUS_OK);

    let cannot_handle = open_fixture_with_presentation_cache();
    assert_eq!(
        mc_native_level1_player_menu_state(cannot_handle, 5),
        MC_STATUS_OK
    );
    with_session_mut(cannot_handle, |session| {
        let fire = session
            .context
            .spell_fire
            .as_mut()
            .expect("Level 1 spell registry");
        let mut holder = fire.holders[0];
        holder.entity_index = 990;
        holder.model = 4;
        holder.active_value = 0;
        fire.holders.push(holder);
        fire.enabled_holder_indices[4] = holder.entity_index;
        fire.selected_spell_ui_index = 4;
        fire.selected_subspell_index = 0;
        fire.family_selected_tiers[4] = 0;
        fire.spell_levels[4] = 2;
        session.mission.castle_mana = 0;
        Ok(())
    })
    .expect("configure existing holder below its Castle-mana gate");
    let (cannot_status, _, _, _) = capture(cannot_handle, &query);
    assert_eq!(
        cannot_status,
        MC_STATUS_OK,
        "{}",
        LAST_ERROR.with(|value| value.borrow().clone())
    );
    assert_eq!(mc_native_session_destroy(cannot_handle), MC_STATUS_OK);

    for (menu_state, expected_stages) in [
        (3_u8, 13_u16),
        (9, 13),
        (10, 13),
        (13, 13),
        (6, 14),
        (8, 15),
        (11, 14),
        (12, 14),
        (14, 14),
    ] {
        let branch_handle = open_fixture_with_presentation_cache();
        assert_eq!(
            mc_native_level1_player_menu_state(branch_handle, menu_state),
            MC_STATUS_OK
        );
        submit_source_presentation_input(branch_handle, &source_presentation_input());
        let (status, _, _, branch_info) = capture(branch_handle, &query);
        assert_eq!(
            status,
            MC_STATUS_OK,
            "menu state {menu_state}: {}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!(
            branch_info.overlay_stage_count, expected_stages,
            "menu state {menu_state} consumes its source-ledger receipt census"
        );
        assert_eq!(mc_native_session_destroy(branch_handle), MC_STATUS_OK);
    }
    let map_frame = |pitch_delta: i16, fov_delta: i16| {
        let map_handle = open_fixture_with_presentation_cache();
        assert_eq!(
            mc_native_level1_player_menu_state(map_handle, 6),
            MC_STATUS_OK
        );
        with_session_mut(map_handle, |session| {
            session.camera_history.pitch = session.camera_history.pitch.wrapping_add(pitch_delta);
            session.camera_history.fov = session.camera_history.fov.wrapping_add(fov_delta);
            Ok(())
        })
        .expect("configure the real map DrawWorld camera");
        let result = capture(map_handle, &query);
        assert_eq!(
            result.0,
            MC_STATUS_OK,
            "map pitch {pitch_delta} fov {fov_delta}: {}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!(mc_native_session_destroy(map_handle), MC_STATUS_OK);
        result.1
    };
    let map_base = map_frame(0, 0);
    for (label, changed) in [("pitch", map_frame(64, 0)), ("fov", map_frame(0, 16))] {
        let base_left = map_base
            .chunks_exact(LEVEL1_VIEW_WIDTH)
            .flat_map(|row| &row[..191])
            .copied()
            .collect::<Vec<_>>();
        let changed_left = changed
            .chunks_exact(LEVEL1_VIEW_WIDTH)
            .flat_map(|row| &row[..191])
            .copied()
            .collect::<Vec<_>>();
        let base_right = map_base
            .chunks_exact(LEVEL1_VIEW_WIDTH)
            .flat_map(|row| &row[192..])
            .copied()
            .collect::<Vec<_>>();
        let changed_right = changed
            .chunks_exact(LEVEL1_VIEW_WIDTH)
            .flat_map(|row| &row[192..])
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            changed_left, base_left,
            "map {label} changes only the real second-world viewport"
        );
        assert_ne!(
            changed_right, base_right,
            "map {label} must reach the real 128x200 DrawWorld projection"
        );
    }
    let scores_handle = open_fixture_with_presentation_cache();
    assert_eq!(
        mc_native_level1_player_menu_state(scores_handle, 7),
        MC_STATUS_OK
    );
    let (scores_status, _, _, _) = capture(scores_handle, &query);
    assert_eq!(
        scores_status, MC_STATUS_GENERATION_MISMATCH,
        "scores fail closed until the live all-player/help-RNG matrix is typed"
    );
    assert_eq!(mc_native_session_destroy(scores_handle), MC_STATUS_OK);

    let chat_handle = open_fixture_with_presentation_cache();
    assert_eq!(
        mc_native_level1_player_menu_state(chat_handle, 3),
        MC_STATUS_OK
    );
    let mut chat_input = source_presentation_input();
    chat_input.chat_lines[5 * 64..5 * 64 + 5].copy_from_slice(b"HELLO");
    chat_input.chat_mode = 3;
    chat_input.chat_player_mask = 1 << 1;
    // Source coordinates (20,376) enter fixed recipient cell 5 after the
    // VGA1 divide, overriding the stored recipient.
    chat_input.mouse_x = 20;
    chat_input.mouse_y = 376;
    submit_source_presentation_input(chat_handle, &chat_input);
    let (chat_status, chat_indices, _, _) = capture(chat_handle, &query);
    assert_eq!(
        chat_status,
        MC_STATUS_OK,
        "{}",
        LAST_ERROR.with(|value| value.borrow().clone())
    );
    assert_ne!(
        chat_indices, first_indices,
        "the selected live chat buffer traverses the exact source writer"
    );
    assert_eq!(mc_native_session_destroy(chat_handle), MC_STATUS_OK);

    let cursor_handle = open_fixture_with_presentation_cache();
    assert_eq!(
        mc_native_level1_player_menu_state(cursor_handle, 3),
        MC_STATUS_OK
    );
    let mut cursor_input = source_presentation_input();
    cursor_input.cursor_visible = 1;
    cursor_input.joystick = 8;
    submit_source_presentation_input(cursor_handle, &cursor_input);
    let (cursor_status, _, _, _) = capture(cursor_handle, &query);
    assert_eq!(
        cursor_status, MC_STATUS_GENERATION_MISMATCH,
        "active selector9 must fail closed until its source bank is proven"
    );
    with_session(cursor_handle, |session| {
        assert_eq!(session.presentation_frame.history(), MainHistory::Unprimed);
        assert!(session.presentation_frame.cached().is_none());
        Ok(())
    })
    .expect("cursor provenance rejection is atomic");
    assert_eq!(mc_native_session_destroy(cursor_handle), MC_STATUS_OK);

    let options_handle = open_fixture_with_presentation_cache();
    assert_eq!(
        mc_native_level1_player_menu_state(options_handle, 9),
        MC_STATUS_OK
    );
    let mut options_input = source_presentation_input();
    options_input.options_selected_mask = 1;
    options_input.options_enabled_mask &= !1;
    options_input.mouse_x = 2 * 120;
    options_input.mouse_y = 2 * 36;
    submit_source_presentation_input(options_handle, &options_input);
    let (options_status, _, _, _) = capture(options_handle, &query);
    assert_eq!(
        options_status,
        MC_STATUS_OK,
        "{}",
        LAST_ERROR.with(|value| value.borrow().clone())
    );
    assert_eq!(mc_native_session_destroy(options_handle), MC_STATUS_OK);

    let volume_handle = open_fixture_with_presentation_cache();
    assert_eq!(
        mc_native_level1_player_menu_state(volume_handle, 10),
        MC_STATUS_OK
    );
    let mut volume_input = source_presentation_input();
    volume_input.volume_target = 2;
    volume_input.music_volume = 63;
    submit_source_presentation_input(volume_handle, &volume_input);
    let (volume_status, _, _, _) = capture(volume_handle, &query);
    assert_eq!(
        volume_status,
        MC_STATUS_OK,
        "{}",
        LAST_ERROR.with(|value| value.borrow().clone())
    );
    assert_eq!(mc_native_session_destroy(volume_handle), MC_STATUS_OK);

    let invalid_volume_handle = open_fixture_with_presentation_cache();
    assert_eq!(
        mc_native_level1_player_menu_state(invalid_volume_handle, 10),
        MC_STATUS_OK
    );
    let mut invalid_volume_input = source_presentation_input();
    invalid_volume_input.volume_target = 0;
    submit_source_presentation_input(invalid_volume_handle, &invalid_volume_input);
    let (invalid_volume_status, _, _, _) = capture(invalid_volume_handle, &query);
    assert_eq!(
        invalid_volume_status, MC_STATUS_GENERATION_MISMATCH,
        "volume target outside sound/music fails before history mutation"
    );
    with_session(invalid_volume_handle, |session| {
        assert_eq!(session.presentation_frame.history(), MainHistory::Unprimed);
        assert!(session.presentation_frame.cached().is_none());
        Ok(())
    })
    .expect("invalid volume rejection is atomic");
    assert_eq!(
        mc_native_session_destroy(invalid_volume_handle),
        MC_STATUS_OK
    );

    for selected_item in 1..=3 {
        let ok_handle = open_fixture_with_presentation_cache();
        assert_eq!(
            mc_native_level1_player_menu_state(ok_handle, 13),
            MC_STATUS_OK
        );
        let mut ok_input = source_presentation_input();
        ok_input.selected_menu_item = selected_item;
        submit_source_presentation_input(ok_handle, &ok_input);
        let (ok_status, _, _, _) = capture(ok_handle, &query);
        assert_eq!(
            ok_status,
            MC_STATUS_OK,
            "OK/cancel item {selected_item}: {}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!(mc_native_session_destroy(ok_handle), MC_STATUS_OK);
    }

    let pause_handle = open_fixture_with_presentation_cache();
    let mut pause_query = query;
    let mut before_pause_step = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_snapshot(
            pause_handle,
            &raw mut before_pause_step,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(mc_native_level1_game_paused(pause_handle, 1), MC_STATUS_OK);
    let mut paused_step = McSessionSnapshotV2::default();
    let paused_command = valid_command();
    assert_eq!(
        mc_native_session_step(
            pause_handle,
            &raw const paused_command,
            &raw mut paused_step,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(paused_step.tick, before_pause_step.tick);
    assert_eq!(paused_step.state_hash, before_pause_step.state_hash);
    assert_eq!(paused_step.event_head, before_pause_step.event_head);
    pause_query.game_paused = 1;
    let mut pause_input = source_presentation_input();
    pause_input.load_available = 0;
    pause_input.sound_available = 0;
    pause_input.music_available = 0;
    submit_source_presentation_input(pause_handle, &pause_input);
    let (pause_status, pause_indices, _, pause_info) = capture(pause_handle, &pause_query);
    assert_eq!(pause_status, MC_STATUS_OK);
    assert_eq!(pause_info.overlay_stage_count, 13);
    assert_ne!(pause_indices, first_indices, "Pause writes its source text");
    assert_eq!(mc_native_level1_game_paused(pause_handle, 0), MC_STATUS_OK);
    let mut resumed_step = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            pause_handle,
            &raw const paused_command,
            &raw mut resumed_step,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(resumed_step.tick, paused_step.tick + 1);
    assert_eq!(mc_native_session_destroy(pause_handle), MC_STATUS_OK);

    let recursive_pause_handle = open_fixture_with_presentation_cache();
    assert_eq!(
        mc_native_level1_game_paused(recursive_pause_handle, 1),
        MC_STATUS_OK
    );
    let mut recursive_pause_input = source_presentation_input();
    recursive_pause_input.recursive_pause_writer = 1;
    submit_source_presentation_input(recursive_pause_handle, &recursive_pause_input);
    let (recursive_pause_status, _, _, _) = capture(recursive_pause_handle, &pause_query);
    assert_eq!(
        recursive_pause_status, MC_STATUS_GENERATION_MISMATCH,
        "recursive sub_30870 state fails closed rather than fabricating output"
    );
    with_session(recursive_pause_handle, |session| {
        assert_eq!(session.presentation_frame.history(), MainHistory::Unprimed);
        assert!(session.presentation_frame.cached().is_none());
        Ok(())
    })
    .expect("recursive pause rejection is atomic");
    assert_eq!(
        mc_native_session_destroy(recursive_pause_handle),
        MC_STATUS_OK
    );

    let fabricated_end_handle = open_fixture_with_presentation_cache();
    let mut level_end_query = query;
    level_end_query.level_end = 1;
    let (fabricated_status, _, _, _) = capture(fabricated_end_handle, &level_end_query);
    assert_eq!(
        fabricated_status, MC_STATUS_GENERATION_MISMATCH,
        "a render-side query cannot fabricate authoritative completion"
    );
    with_session(fabricated_end_handle, |session| {
        assert_eq!(session.presentation_frame.history(), MainHistory::Unprimed);
        assert!(session.presentation_frame.cached().is_none());
        Ok(())
    })
    .expect("fabricated completion rejection is atomic");
    assert_eq!(
        mc_native_session_destroy(fabricated_end_handle),
        MC_STATUS_OK
    );

    let completed_handle = open_fixture_with_presentation_cache();
    with_session_mut(completed_handle, |session| {
        session.mission.level_completed = true;
        Ok(())
    })
    .expect("configure authoritative completion");
    submit_source_presentation_input(completed_handle, &source_presentation_input());
    let (completed_status, completed_indices, _, completed_info) =
        capture(completed_handle, &level_end_query);
    assert_eq!(completed_status, MC_STATUS_OK);
    assert_eq!(completed_info.overlay_stage_count, 13);
    assert_ne!(
        completed_indices, first_indices,
        "authoritative completion writes the two source strings"
    );
    assert_eq!(mc_native_session_destroy(completed_handle), MC_STATUS_OK);

    let notification_handle = open_fixture_with_presentation_cache();
    with_session_mut(notification_handle, |session| {
        session.mission.player_notification_active = true;
        session.mission.player_notification_id =
            sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_ID;
        session.mission.player_notification_state =
            sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_STATE;
        session.mission.player_notification_timer =
            sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_TIMER;
        Ok(())
    })
    .expect("configure the source player notification");
    submit_source_presentation_input(notification_handle, &source_presentation_input());
    let (notification_status, notification_indices, _, notification_info) =
        capture(notification_handle, &query);
    assert_eq!(notification_status, MC_STATUS_OK);
    assert_eq!(notification_info.overlay_stage_count, 13);
    assert_ne!(
        notification_indices, first_indices,
        "the live source notification writes its fixed-profile text"
    );
    with_session(notification_handle, |session| {
        assert_eq!(
            (
                session.mission.player_notification_state,
                session.mission.player_notification_timer
            ),
            (
                sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_STATE,
                sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_TIMER,
            ),
            "presentation observation leaves same-tick notification state unchanged"
        );
        Ok(())
    })
    .expect("notification feedback committed");
    let (repeat_status, _, _, repeat_info) = capture(notification_handle, &query);
    assert_eq!(repeat_status, MC_STATUS_OK);
    assert_eq!(repeat_info.newly_committed, 0);
    with_session(notification_handle, |session| {
        assert_eq!(
            session.mission.player_notification_timer,
            sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_TIMER,
            "same-key cached retry remains observational"
        );
        Ok(())
    })
    .expect("notification retry is atomic");
    with_session_mut(notification_handle, |session| {
        session.step(player_command(valid_command()).expect("valid native command"));
        assert_eq!(
            session.mission.player_notification_timer,
            sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_TIMER - 1,
            "the next outer turn applies the single accepted writer feedback"
        );
        Ok(())
    })
    .expect("notification feedback applies once");
    assert_eq!(mc_native_session_destroy(notification_handle), MC_STATUS_OK);

    for state in 1..=5 {
        let state_handle = open_fixture_with_presentation_cache();
        with_session_mut(state_handle, |session| {
            session.mission.player_notification_active = true;
            session.mission.player_notification_id =
                sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_ID;
            session.mission.player_notification_state = state;
            session.mission.player_notification_timer = 1;
            Ok(())
        })
        .expect("configure one exact notification state");
        submit_source_presentation_input(state_handle, &source_presentation_input());
        let (state_status, _, _, _) = capture(state_handle, &query);
        assert_eq!(
            state_status,
            MC_STATUS_OK,
            "notification state {state}: {}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        with_session(state_handle, |session| {
            assert_eq!(
                (
                    session.mission.player_notification_active,
                    session.mission.player_notification_state,
                    session.mission.player_notification_timer,
                ),
                (true, state, 1),
                "notification rendering cannot advance the authoritative timer"
            );
            Ok(())
        })
        .expect("notification edge feedback");
        assert_eq!(mc_native_session_destroy(state_handle), MC_STATUS_OK);
    }

    let expired_notification_handle = open_fixture_with_presentation_cache();
    with_session_mut(expired_notification_handle, |session| {
        session.mission.player_notification_active = true;
        session.mission.player_notification_id =
            sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_ID;
        session.mission.player_notification_state = 1;
        session.mission.player_notification_timer = 0;
        Ok(())
    })
    .expect("configure notification timer zero");
    submit_source_presentation_input(expired_notification_handle, &source_presentation_input());
    let (expired_status, _, _, _) = capture(expired_notification_handle, &query);
    assert_eq!(expired_status, MC_STATUS_OK);
    with_session(expired_notification_handle, |session| {
        assert_eq!(
            (
                session.mission.player_notification_active,
                session.mission.player_notification_state,
                session.mission.player_notification_timer,
            ),
            (true, 1, 0),
            "presentation does not clear expired authoritative state"
        );
        Ok(())
    })
    .expect("expired notification observation");
    with_session_mut(expired_notification_handle, |session| {
        session.step(player_command(valid_command()).expect("valid native command"));
        assert_eq!(
            (
                session.mission.player_notification_active,
                session.mission.player_notification_state,
                session.mission.player_notification_timer,
            ),
            (false, 0, -1),
            "the Rust scheduler clears the notification on the next outer turn"
        );
        Ok(())
    })
    .expect("expired notification lifecycle advances independently of polling");
    assert_eq!(
        mc_native_session_destroy(expired_notification_handle),
        MC_STATUS_OK
    );

    let phase_notification = |timer: i16| {
        let phase_handle = open_fixture_with_presentation_cache();
        with_session_mut(phase_handle, |session| {
            session.mission.player_notification_active = true;
            session.mission.player_notification_id =
                sim_core::level1_mission::PLAYER_DIED_NOTIFICATION_ID;
            session.mission.player_notification_state = 2;
            session.mission.player_notification_timer = timer;
            Ok(())
        })
        .expect("configure state2 timer/phase edge");
        submit_source_presentation_input(phase_handle, &source_presentation_input());
        let result = capture(phase_handle, &query);
        assert_eq!(
            result.0,
            MC_STATUS_OK,
            "state2 timer {timer}: {}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!(mc_native_session_destroy(phase_handle), MC_STATUS_OK);
        result
    };
    let (_, timer_101_indices, _, _) = phase_notification(101);
    let (_, timer_102_phase_indices, _, _) = phase_notification(102);
    assert_ne!(
        timer_101_indices, timer_102_phase_indices,
        "state2 uses CLRN3840 through post-decrement 100 then the phase colour above it"
    );

    let help_handle = open_fixture_with_presentation_cache();
    let mut help_query = query;
    help_query.help_popup = 1;
    let (help_status, _, _, _) = capture(help_handle, &help_query);
    assert_eq!(
        help_status, MC_STATUS_GENERATION_MISMATCH,
        "help cannot earn COMPLETE without cached E2A74 recursive state"
    );
    with_session(help_handle, |session| {
        assert_eq!(session.presentation_frame.history(), MainHistory::Unprimed);
        assert!(session.presentation_frame.cached().is_none());
        Ok(())
    })
    .expect("help prerequisite rejection is atomic");
    let (help_retry_status, _, _, help_retry_info) = capture(help_handle, &query);
    assert_eq!(
        help_retry_status, MC_STATUS_OK,
        "clearing the public help query retries the rejected tick"
    );
    assert_eq!(
        (
            help_retry_info.history_state,
            help_retry_info.newly_committed
        ),
        (1, 1),
        "a failed help request does not mutate its sentinel/history before the valid retry"
    );
    assert_eq!(mc_native_session_destroy(help_handle), MC_STATUS_OK);

    for label in ["tutorial sentinel stream", "spider sprite sheet"] {
        let prerequisite_handle = open_fixture_with_presentation_cache();
        let mut prerequisite_query = query;
        if label == "tutorial sentinel stream" {
            prerequisite_query.tutorial_or_briefing = 1;
        } else {
            with_session_mut(prerequisite_handle, |session| {
                session.state.immobilize_counter = 1;
                Ok(())
            })
            .expect(label);
        }
        let (status, _, _, _) = capture(prerequisite_handle, &prerequisite_query);
        assert_eq!(
            status, MC_STATUS_GENERATION_MISMATCH,
            "{label} must fail closed until the independent prerequisite is cached"
        );
        with_session(prerequisite_handle, |session| {
            assert_eq!(session.presentation_frame.history(), MainHistory::Unprimed);
            assert!(session.presentation_frame.cached().is_none());
            Ok(())
        })
        .expect("missing prerequisite is atomic");
        assert_eq!(mc_native_session_destroy(prerequisite_handle), MC_STATUS_OK);
    }

    let missing_source_handle = open_fixture_with_presentation_cache();
    with_session_mut(missing_source_handle, |session| {
        session.selector_frames = None;
        Ok(())
    })
    .expect("remove one current source compositor producer");
    let (missing_status, _, _, _) = capture(missing_source_handle, &query);
    assert_eq!(missing_status, MC_STATUS_GENERATION_MISMATCH);
    with_session(missing_source_handle, |session| {
        assert_eq!(session.presentation_frame.history(), MainHistory::Unprimed);
        assert!(session.presentation_frame.cached().is_none());
        Ok(())
    })
    .expect("missing producer fails before committing history");
    assert_eq!(
        mc_native_session_destroy(missing_source_handle),
        MC_STATUS_OK
    );

    query.presentation_revision = 2;
    assert_eq!(
        copy(&query, &mut indices, &mut provenance, &mut info),
        MC_STATUS_GENERATION_MISMATCH,
        "same finalized tick with a changed revision rejects"
    );
    query.presentation_revision = 1;
    query.tick = 2;
    assert_eq!(
        copy(&query, &mut indices, &mut provenance, &mut info),
        MC_STATUS_GENERATION_MISMATCH,
        "out-of-order presentation rejects before rendering"
    );

    let command = valid_command();
    let mut stepped = McSessionSnapshotV2::default();
    for _ in 0..2 {
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut stepped,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
    }
    with_session(handle, |session| {
        assert!(
            session
                .world
                .raw_source_entity_indices()
                .iter()
                .copied()
                .any(|entity_index| {
                    session.mission.source_map_chains().tracks(entity_index)
                        && !session.renderable_map_chains.tracks(entity_index)
                }),
            "FINAL MAIN keeps its filtered painter chain separate from the durable raw gameplay topology"
        );
        Ok(())
    })
    .expect("registered C API session");
    query.tick = 1;
    query.configured_blur_mode = 2;
    query.blur_capability = 1;
    let temporal_status = copy(&query, &mut indices, &mut provenance, &mut info);
    assert_eq!(
        temporal_status,
        MC_STATUS_OK,
        "the next contiguous stored camera tick consumes prior finalized MAIN: {}",
        LAST_ERROR.with(|value| value.borrow().clone())
    );
    assert_eq!(
        (info.effective_postprocess_mode, info.newly_committed),
        (2, 1)
    );
    assert!(
        provenance.iter().any(|value| value & 2 != 0),
        "temporal provenance records prior finalized MAIN"
    );
    query.tick = 0;
    assert_eq!(
        copy(&query, &mut indices, &mut provenance, &mut info),
        MC_STATUS_GENERATION_MISMATCH,
        "a stale tick rejects after contiguous advancement"
    );

    let mut snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_reset(
            handle,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    query.generation = snapshot.generation;
    query.tick = 0;
    query.configured_blur_mode = 2;
    query.blur_capability = 1;
    assert_eq!(
        copy(&query, &mut indices, &mut provenance, &mut info),
        MC_STATUS_GENERATION_MISMATCH,
        "reset restores Unprimed and rejects a temporal first frame"
    );
    query.configured_blur_mode = 0;
    query.blur_capability = 0;
    assert_eq!(
        copy(&query, &mut indices, &mut provenance, &mut info),
        MC_STATUS_OK
    );
    assert_eq!((info.history_state, info.newly_committed), (1, 1));
    let mut isolated_snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_reset(
            isolated_handle,
            &raw mut isolated_snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_level1_player_menu_state(isolated_handle, 6),
        MC_STATUS_OK
    );
    let mut incomplete_query = query;
    incomplete_query.generation = isolated_snapshot.generation;
    incomplete_query.configured_blur_mode = 0;
    incomplete_query.blur_capability = 0;
    assert_eq!(
        mc_native_session_presentation_frame_copy(
            isolated_handle,
            &raw const incomplete_query,
            isolated_indices.as_mut_ptr(),
            isolated_indices.len() as u32,
            isolated_provenance.as_mut_ptr(),
            isolated_provenance.len() as u32,
            &raw mut isolated_info,
            struct_size::<McPresentationFrameInfoV2>(),
        ),
        MC_STATUS_OK,
        "{}",
        LAST_ERROR.with(|value| value.borrow().clone())
    );
    assert_eq!(isolated_info.overlay_stage_count, 14);
    with_session(isolated_handle, |session| {
        assert_eq!(session.presentation_frame.history(), MainHistory::Primed);
        assert!(session.presentation_frame.cached().is_some());
        Ok(())
    })
    .expect("map source branch commits only after all typed receipts");

    let zero_life_handle = open_fixture_with_presentation_cache();
    with_session_mut(zero_life_handle, |session| {
        session.mission.player_life = 0;
        Ok(())
    })
    .expect("configure exact source alive boundary");
    let mut boundary_query = query;
    boundary_query.generation = 1;
    boundary_query.tick = 0;
    let (zero_status, _, _, zero_info) = capture(zero_life_handle, &boundary_query);
    assert_eq!(
        zero_status,
        MC_STATUS_OK,
        "life zero remains alive: {}",
        LAST_ERROR.with(|value| value.borrow().clone())
    );
    assert_eq!(zero_info.overlay_stage_count, 13);
    assert_eq!(mc_native_session_destroy(zero_life_handle), MC_STATUS_OK);

    let dead_handle = open_fixture_with_presentation_cache();
    with_session_mut(dead_handle, |session| {
        session.mission.player_life = -1;
        Ok(())
    })
    .expect("configure dead source branch");
    let (dead_status, _, _, dead_info) = capture(dead_handle, &boundary_query);
    assert_eq!(dead_status, MC_STATUS_OK);
    assert_eq!(
        dead_info.overlay_stage_count, 5,
        "dead ordinary branch is menu then objective/tutorial/help/debug only"
    );
    assert_eq!(mc_native_session_destroy(dead_handle), MC_STATUS_OK);
    for menu_state in [9_u8, 10, 13] {
        let dead_menu_handle = open_fixture_with_presentation_cache();
        with_session_mut(dead_menu_handle, |session| {
            session.mission.player_life = -1;
            session.mission.player_menu_state = menu_state;
            Ok(())
        })
        .expect("configure dead source menu branch");
        submit_source_presentation_input(dead_menu_handle, &source_presentation_input());
        let (status, _, _, dead_menu_info) = capture(dead_menu_handle, &boundary_query);
        assert_eq!(
            status,
            MC_STATUS_OK,
            "dead menu state {menu_state}: {}",
            LAST_ERROR.with(|value| value.borrow().clone())
        );
        assert_eq!(
            dead_menu_info.overlay_stage_count, 5,
            "dead menu state {menu_state} closes its source-ledger receipt sequence"
        );
        assert_eq!(mc_native_session_destroy(dead_menu_handle), MC_STATUS_OK);
    }
    assert_eq!(mc_native_session_destroy(isolated_handle), MC_STATUS_OK);
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn player_hit_bridge_installs_only_next_turn_directional_recoil() {
    let handle = open_fixture();
    let expected_life = with_session_mut(handle, |session| {
        let attacker = 10;
        session.mission.submit_player_damage(500, attacker);
        Ok(session.mission.player_life - 500)
    })
    .expect("registered C API session");
    let mut snapshot = McSessionSnapshotV2::default();
    let command = valid_command();
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let control_handle = open_fixture();
    let mut control_snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            control_handle,
            &raw const command,
            &raw mut control_snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let control_position =
        with_session(control_handle, |session| Ok(session.state.position)).unwrap();
    with_session(handle, |session| {
        assert_eq!(session.mission.player_life, expected_life);
        assert_eq!(
            session.state.recoil_speed, 46,
            "the same-turn eight-pass flight consumes four units of the installed boost"
        );
        assert_ne!(
            session.state.position, control_position,
            "sub_5EFA0 recoil participates in this same production flight turn"
        );
        Ok(())
    })
    .expect("registered C API session");
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
        mc_native_session_step(
            control_handle,
            &raw const command,
            &raw mut control_snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    with_session(handle, |session| {
        assert_eq!(session.mission.player_life, expected_life);
        assert_eq!(
            session.state.recoil_speed, 42,
            "the next eight-pass turn decays the one-shot boost instead of replaying 50"
        );
        Ok(())
    })
    .expect("registered C API session");
    assert_eq!(mc_native_session_destroy(control_handle), MC_STATUS_OK);
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

fn fixture_paths() -> (PathBuf, PathBuf) {
    let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    (
        private.join("generated/level1-export-m"),
        private.join("generated/level1-asset-closure.json"),
    )
}

fn level2_objective_point(handle: u64) -> (u16, u16) {
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
    assert_eq!((state.stage_kind, state.stage_state), (5, 1));
    (state.axis_x as u16, state.axis_y as u16)
}

fn open_fixture() -> u64 {
    let (package, closure) = fixture_paths();
    let world = NativeWorld::load_without_owner_cache(package, closure)
        .expect("package M opens without an owner cache");
    let session = SessionRecord::new_with_terrain_frame_assets(world, None)
        .expect("cache-free fixture session constructs");
    with_registry_mut(|registry| registry.insert(session))
        .expect("cache-free fixture session registers")
}

fn open_fixture_with_event_capacity(capacity: usize) -> u64 {
    assert!(capacity > 0, "event journal capacity must be non-zero");
    let handle = open_fixture();
    with_session_mut(handle, |session| {
        session.event_capacity = capacity;
        Ok(())
    })
    .expect("test event journal capacity installs");
    handle
}

fn configured_goat_pitch_handle(retain_action_14: bool) -> u64 {
    const BLEATING_GOAT: u16 = 102;
    const BLEATING_GOAT_RESET_RNG: u16 = 32_616;

    let handle = open_fixture();
    with_session_mut(handle, |session| {
        let mut entities = session.world.entities().to_vec();
        let goat = &mut entities[usize::from(BLEATING_GOAT)];
        goat.action = 14;
        goat.position = session.state.position;
        session.live_world = LiveWorldState::from_entities(
            &entities,
            session.world.goat_stage_point(),
            session.world.goat_config(),
        )
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
        let configured = session
            .live_world
            .goats()
            .iter()
            .find(|goat| goat.entity_index == BLEATING_GOAT)
            .expect("source-derived Goat slot 102");
        assert_eq!(configured.rng, BLEATING_GOAT_RESET_RNG);
        assert_eq!(configured.action, 14);
        assert_eq!(session.mission.global_rng(), 0xad6e_67d9);
        Ok(())
    })
    .expect("configure the source-derived action-14 pitch discriminator");

    let mut before = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_snapshot(
            handle,
            &raw mut before,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    if retain_action_14 {
        let damage = McGoatDamageCommandV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McGoatDamageCommandV2>(),
            generation: before.generation,
            entity_index: BLEATING_GOAT,
            reserved_0: 0,
            damage: 1,
            reserved: [0; 4],
        };
        let mut batch = McGoatBatchV2::default();
        assert_eq!(
            mc_native_session_goat_inject_damage(
                handle,
                &raw const damage,
                &raw mut batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_OK
        );
    }
    handle
}

fn goat_pitch_audio_after_step(handle: u64) -> McAudioCommandV2 {
    const BLEATING_GOAT: u16 = 102;
    let mut after = McSessionSnapshotV2::default();
    let idle = valid_command();
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const idle,
            &raw mut after,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let query = McEventQueryV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McEventQueryV2>(),
        generation: after.generation,
        cursor: 0,
        reserved: [0; 4],
    };
    let mut events = vec![McEventV2::default(); 256];
    let mut event_batch = McEventBatchV2::default();
    assert_eq!(
        mc_native_events_copy_since(
            handle,
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
    let bleat = events
        .iter()
        .find(|event| {
            event.kind == MC_EVENT_GOAT_SOUND_REQUEST
                && event.entity_index == BLEATING_GOAT
                && event.code == 46
        })
        .expect("production Goat action emits sound 46 before slot 113");
    let mut audio = McAudioCommandV2::default();
    assert_eq!(
        mc_native_audio_command_for_event(
            handle,
            after.generation,
            bleat.sequence,
            &raw mut audio,
            struct_size::<McAudioCommandV2>(),
        ),
        MC_STATUS_OK
    );
    audio
}

fn production_goat_pitch_case(retain_action_14: bool) -> McAudioCommandV2 {
    const BLEATING_GOAT: u16 = 102;
    let handle = configured_goat_pitch_handle(retain_action_14);
    let audio = goat_pitch_audio_after_step(handle);
    with_session(handle, |session| {
        let goat = session
            .live_world
            .goats()
            .iter()
            .find(|goat| goat.entity_index == BLEATING_GOAT)
            .expect("source-derived Goat slot 102 remains projected");
        assert_eq!(session.mission.global_rng(), 0x43e9_ccaf);
        assert_eq!(goat.action == 14, retain_action_14);
        Ok(())
    })
    .expect("inspect the post-turn source discriminator");
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    audio
}

#[test]
fn goat_sound_46_peeks_the_slot_102_stored_rng_without_consuming_it() {
    const BLEATING_GOAT: u16 = 102;
    let handle = open_fixture();
    let (audio, stored_rng) = with_session_mut(handle, |session| {
        let listener = session.state.position;
        let mut entities = session.world.entities().to_vec();
        let goat = &mut entities[usize::from(BLEATING_GOAT)];
        goat.action = 14;
        goat.position = listener;
        session.live_world = LiveWorldState::from_entities(
            &entities,
            session.world.goat_stage_point(),
            session.world.goat_config(),
        )
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
        session
            .live_world
            .inject_damage(
                BLEATING_GOAT,
                1,
                i16::try_from(session.state.entity_index).unwrap(),
            )
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;

        session.mission.begin_source_turn_at(listener);
        let stored_rng = session.mission.global_rng();
        assert_eq!(stored_rng, 0x9909_f858);
        let events = session.live_world.step_source_slot(
            BLEATING_GOAT,
            &session.terrain,
            &[GoatPlayerTarget {
                entity_index: session.state.entity_index,
                position: listener,
                control_flags: 0,
            }],
        );
        assert_eq!(session.mission.global_rng(), stored_rng);
        let bleat = events
            .into_iter()
            .find(|event| matches!(event, GoatStepEvent::Bleat { sound_id: 46, .. }))
            .ok_or_else(|| AbiError::new(MC_STATUS_VALIDATION_FAILED, "slot 102 did not bleat"))?;
        let event = session.encode_goat_event(bleat, session.live_world_hash_v2(), stored_rng);
        let audio = *session.audio_commands.get(&event.sequence).ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "slot 102 bleat has no audio command",
            )
        })?;
        assert_eq!(session.mission.global_rng(), stored_rng);
        Ok((audio, stored_rng))
    })
    .unwrap();
    assert_eq!(audio.rate_delta_percent, 25);
    let nonstored = stored_rng.wrapping_mul(9_377).wrapping_add(9_439);
    assert_eq!(nonstored, 0xa631_b437);
    let other_action = prepare_event_sound_command(
        Position::default(),
        0,
        Position::default(),
        46,
        stored_rng,
        BLEATING_GOAT,
        0,
    )
    .unwrap();
    assert_eq!(other_action.command.rate_delta_percent, 5);
    assert_eq!(stored_rng, 0x9909_f858, "the pitch peek is never stored");
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn dynamic_house_resident_overlays_the_source_entity_projection() {
    let (package, closure) = fixture_paths();
    let world = NativeWorld::load(package, closure).expect("package M loads");
    let mut session = SessionRecord::new(world).expect("fixture session constructs");
    let position = Position {
        x: 12_345,
        y: 23_456,
        z: 640,
    };
    let source_slot = session
        .mission
        .spawn_house_resident(position, 4, 33, session.state.entity_index)
        .expect("qualified Archer resident constructor")
        .expect("fixture source pool has one allocatable row");
    let entity = &session.world.entities()[usize::from(source_slot)];
    let snapshot = entity_snapshot(
        entity,
        &session.state,
        &session.live_world,
        &session.mission,
    );
    assert_eq!(snapshot.entity_index, source_slot);
    assert_eq!(snapshot.authority, MC_AUTHORITY_LEVEL1_MISSION_LIVE);
    assert_eq!(snapshot.active, 1);
    assert_eq!(
        (snapshot.class_id, snapshot.model, snapshot.action),
        (5, 4, 33)
    );
    assert_eq!((snapshot.x, snapshot.y, snapshot.z), (12_345, 23_456, 640));
    assert_eq!(snapshot.parameter_record_index, 75);
}

fn open_fixture_with_terrain_frame_assets() -> u64 {
    let world = presentation_test_world();
    let mut session = SessionRecord::new(world).expect("fixture session constructs");
    let assets = Arc::new(terrain_frame_assets_fixture());
    session.archer_selector_frames = Some(ArcherTmapClocks::from_bank(&assets.archer_tmap_bank));
    session.firefly_selector_frames = Some(FireflyTmapClocks::from_bank(&assets.firefly_tmap_bank));
    session.villager_selector_frames = Some(VillagerTmapClocks::from_bank(
        &assets.frozen_actor_tmap_bank,
    ));
    session.fireball_selector_frame = Some(FireballTmapClock::from_bank(&assets.combat_tmap_bank));
    session.possession_selector_frame =
        Some(PossessionTmapClock::from_bank(&assets.combat_tmap_bank));
    session.castle_selector_frame = Some(CastleTmapClock::from_bank(&assets.economy_tmap_bank));
    session.source_render_asset_digest = Some(
        source_render_asset_digest(session.world.as_ref(), &assets)
            .expect("fixture source-render asset digest"),
    );
    session.terrain_frame_assets = Some(assets);
    with_registry_mut(|registry| registry.insert(session)).expect("fixture session registers")
}

fn presentation_test_cache() -> PathBuf {
    let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    resolve_level1_presentation_cache_root(private)
}

fn presentation_test_world() -> NativeWorld {
    let (package, closure) = fixture_paths();
    NativeWorld::load_with_owner_cache(package, closure, presentation_test_cache())
        .expect("package M and manifest/hash-qualified presentation cache load")
}

fn terrain_frame_assets_fixture() -> TerrainFrameAssets {
    load_terrain_frame_assets(&presentation_test_cache())
        .expect("manifest/hash-qualified presentation assets load")
}

fn open_fixture_with_owner_cache() -> u64 {
    let world = presentation_test_world();
    let session = SessionRecord::new(world).expect("owner-cache fixture session constructs");
    with_registry_mut(|registry| registry.insert(session))
        .expect("owner-cache fixture session registers")
}

fn open_fixture_with_presentation_cache() -> u64 {
    let world = presentation_test_world();
    let mut session = SessionRecord::new(world).expect("presentation session constructs");
    let assets = Arc::new(terrain_frame_assets_fixture());
    session.selector_frames = session
        .world
        .goat_tmap_bank()
        .map(GoatTmapClocks::from_bank);
    session.archer_selector_frames = Some(ArcherTmapClocks::from_bank(&assets.archer_tmap_bank));
    session.firefly_selector_frames = Some(FireflyTmapClocks::from_bank(&assets.firefly_tmap_bank));
    session.villager_selector_frames = Some(VillagerTmapClocks::from_bank(
        &assets.frozen_actor_tmap_bank,
    ));
    session.fireball_selector_frame = Some(FireballTmapClock::from_bank(&assets.combat_tmap_bank));
    session.possession_selector_frame =
        Some(PossessionTmapClock::from_bank(&assets.combat_tmap_bank));
    session.castle_selector_frame = Some(CastleTmapClock::from_bank(&assets.economy_tmap_bank));
    session.source_render_asset_digest = Some(
        source_render_asset_digest(session.world.as_ref(), &assets)
            .expect("fixture source-render asset digest"),
    );
    session.terrain_frame_assets = Some(assets);
    with_registry_mut(|registry| registry.insert(session)).expect("presentation session registers")
}

fn open_fixture_with_owner_cache_and_terrain_frame_assets() -> u64 {
    let world = presentation_test_world();
    let mut session = SessionRecord::new(world).expect("owner-cache fixture session constructs");
    let assets = Arc::new(terrain_frame_assets_fixture());
    session.archer_selector_frames = Some(ArcherTmapClocks::from_bank(&assets.archer_tmap_bank));
    session.firefly_selector_frames = Some(FireflyTmapClocks::from_bank(&assets.firefly_tmap_bank));
    session.villager_selector_frames = Some(VillagerTmapClocks::from_bank(
        &assets.frozen_actor_tmap_bank,
    ));
    session.fireball_selector_frame = Some(FireballTmapClock::from_bank(&assets.combat_tmap_bank));
    session.possession_selector_frame =
        Some(PossessionTmapClock::from_bank(&assets.combat_tmap_bank));
    session.castle_selector_frame = Some(CastleTmapClock::from_bank(&assets.economy_tmap_bank));
    session.source_render_asset_digest = Some(
        source_render_asset_digest(session.world.as_ref(), &assets)
            .expect("fixture source-render asset digest"),
    );
    session.terrain_frame_assets = Some(assets);
    with_registry_mut(|registry| registry.insert(session))
        .expect("complete owner-cache fixture session registers")
}

fn open_fixture_with_world_frame_assets() -> u64 {
    let world = presentation_test_world();
    let mut session = SessionRecord::new(world).expect("owner-cache fixture session constructs");
    let assets = Arc::new(terrain_frame_assets_fixture());
    session.archer_selector_frames = Some(ArcherTmapClocks::from_bank(&assets.archer_tmap_bank));
    session.firefly_selector_frames = Some(FireflyTmapClocks::from_bank(&assets.firefly_tmap_bank));
    session.villager_selector_frames = Some(VillagerTmapClocks::from_bank(
        &assets.frozen_actor_tmap_bank,
    ));
    session.fireball_selector_frame = Some(FireballTmapClock::from_bank(&assets.combat_tmap_bank));
    session.possession_selector_frame =
        Some(PossessionTmapClock::from_bank(&assets.combat_tmap_bank));
    session.castle_selector_frame = Some(CastleTmapClock::from_bank(&assets.economy_tmap_bank));
    session.source_render_asset_digest = Some(
        source_render_asset_digest(session.world.as_ref(), &assets)
            .expect("fixture source-render asset digest"),
    );
    session.terrain_frame_assets = Some(assets);
    with_registry_mut(|registry| registry.insert(session))
        .expect("world-frame fixture session registers")
}

fn open_paths(package: &Path, closure: &Path) -> Result<u64, i32> {
    let package = package.to_string_lossy();
    let closure = closure.to_string_lossy();
    let mut handle = 0_u64;
    let status = mc_native_session_open(
        package.as_ptr(),
        u32::try_from(package.len()).unwrap(),
        closure.as_ptr(),
        u32::try_from(closure.len()).unwrap(),
        &raw mut handle,
    );
    if status == MC_STATUS_OK {
        Ok(handle)
    } else {
        Err(status)
    }
}

fn valid_command() -> McCommandV2 {
    McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        requested_spell: MC_REQUESTED_SPELL_NONE,
        ..McCommandV2::default()
    }
}

fn valid_presentation_frame_query() -> McPresentationFrameQueryV2 {
    McPresentationFrameQueryV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McPresentationFrameQueryV2>(),
        generation: 1,
        tick: 0,
        presentation_revision: 1,
        viewport_width: LEVEL1_VIEW_WIDTH as u16,
        viewport_height: LEVEL1_VIEW_HEIGHT as u16,
        screen_stride: LEVEL1_VIEW_WIDTH as u16,
        reserved_0: 0,
        configured_blur_mode: 0,
        blur_capability: 0,
        high_speed_system: 1,
        scratch_present: 1,
        vga_resolution: 1,
        spatial_smoothing: 0,
        minimap_enabled: 1,
        top_bar_enabled: 1,
        menu_overlay: 0,
        game_paused: 0,
        level_end: 0,
        objective_selector: u8::MAX,
        objective_textbox: 0,
        tutorial_or_briefing: 0,
        help_popup: 0,
        debug_text: 0,
        reserved: [0; 2],
    }
}

fn valid_camera_input() -> McCameraInputV2 {
    McCameraInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCameraInputV2>(),
        ..McCameraInputV2::default()
    }
}

fn open_level2_campaign_handle() -> u64 {
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
    handle
}

fn qualified_level2_day_owner_cache() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated/level2-day-owner-cache")
}

fn open_level2_campaign_handle_with_asset_cache(cache: &Path) -> u64 {
    let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/l2-debug3.fJ3PaM/level2");
    let package = level2_package.to_string_lossy();
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
        MC_STATUS_OK
    );
    handle
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create temporary cache directory");
    for entry in fs::read_dir(source).expect("read source cache directory") {
        let entry = entry.expect("read source cache entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path).expect("inspect source cache entry");
        if metadata.is_dir() {
            copy_tree(&source_path, &destination_path);
        } else {
            assert!(metadata.is_file(), "source cache contains a special entry");
            fs::copy(&source_path, &destination_path).expect("copy source cache file");
        }
    }
}

fn step_level2_campaign_once(
    handle: u64,
    generation: u64,
) -> (McCampaignStepSnapshotV2, [McCampaignEventV2; 1024], usize) {
    let input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation,
        command: valid_command(),
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
    (snapshot, events, event_count as usize)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RecordedCampaignStep {
    input: McCampaignStepInputV2,
    snapshot: McCampaignStepSnapshotV2,
    events: Vec<McCampaignEventV2>,
}

thread_local! {
    static CAMPAIGN_STEP_RECORDER: RefCell<Option<(u64, Vec<RecordedCampaignStep>)>> =
        const { RefCell::new(None) };
}

fn start_campaign_step_recording(handle: u64) {
    CAMPAIGN_STEP_RECORDER.with(|recorder| {
        assert!(
            recorder.replace(Some((handle, Vec::new()))).is_none(),
            "one test thread records at most one campaign stream"
        );
    });
}

fn finish_campaign_step_recording(handle: u64) -> Vec<RecordedCampaignStep> {
    CAMPAIGN_STEP_RECORDER.with(|recorder| {
        let (recorded_handle, steps) = recorder
            .replace(None)
            .expect("campaign stream recording was started");
        assert_eq!(recorded_handle, handle);
        steps
    })
}

fn normalize_recorded_campaign_step(
    mut snapshot: McCampaignStepSnapshotV2,
    mut events: Vec<McCampaignEventV2>,
) -> (McCampaignStepSnapshotV2, Vec<McCampaignEventV2>) {
    snapshot.generation = 0;
    for event in &mut events {
        event.generation = 0;
    }
    (snapshot, events)
}

/// Test-only C-ABI route for production Level 2 turns.  Possession and
/// Castle proofs deliberately use this rather than mutating a reset
/// session, a holder, or an economy row behind the native boundary.
fn step_level2_campaign_public(
    handle: u64,
    input: McCampaignStepInputV2,
) -> (McCampaignStepSnapshotV2, Vec<McCampaignEventV2>) {
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
    let events = events[..event_count as usize].to_vec();
    CAMPAIGN_STEP_RECORDER.with(|recorder| {
        if let Some((recorded_handle, steps)) = recorder.borrow_mut().as_mut()
            && *recorded_handle == handle
        {
            steps.push(RecordedCampaignStep {
                input,
                snapshot,
                events: events.clone(),
            });
        }
    });
    (snapshot, events)
}

fn campaign_day_hud_public(handle: u64, generation: u64, tick: u64) -> McCampaignDayHudSnapshotV2 {
    let mut hud = McCampaignDayHudSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_day_hud_state(
            handle,
            generation,
            tick,
            &raw mut hud,
            struct_size::<McCampaignDayHudSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    hud
}

fn campaign_players_public(handle: u64) -> Vec<McCampaignPlayerSnapshotV2> {
    let mut players = [McCampaignPlayerSnapshotV2::default(); 8];
    let mut written = 0;
    assert_eq!(
        mc_native_campaign_players_copy(
            handle,
            0,
            players.as_mut_ptr(),
            players.len() as u32,
            struct_size::<McCampaignPlayerSnapshotV2>(),
            &raw mut written,
        ),
        MC_STATUS_OK
    );
    players[..written as usize].to_vec()
}

fn campaign_mana_spheres_public(handle: u64, generation: u64) -> Vec<McManaSphereSnapshotV2> {
    let mut rows = [McManaSphereSnapshotV2::default(); 64];
    let mut batch = McCampaignManaBatchV2::default();
    assert_eq!(
        mc_native_campaign_mana_spheres_copy(
            handle,
            generation,
            0,
            rows.as_mut_ptr(),
            rows.len() as u32,
            struct_size::<McManaSphereSnapshotV2>(),
            &raw mut batch,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_OK
    );
    rows[..batch.written as usize].to_vec()
}

fn campaign_spell_pickups_public(
    handle: u64,
    generation: u64,
) -> Vec<McCampaignSpellPickupSnapshotV2> {
    let mut rows = [McCampaignSpellPickupSnapshotV2::default(); 64];
    let mut batch = McCampaignManaBatchV2::default();
    assert_eq!(
        mc_native_campaign_spell_pickups_copy(
            handle,
            generation,
            0,
            rows.as_mut_ptr(),
            rows.len() as u32,
            struct_size::<McCampaignSpellPickupSnapshotV2>(),
            &raw mut batch,
            struct_size::<McCampaignManaBatchV2>(),
        ),
        MC_STATUS_OK
    );
    rows[..batch.written as usize].to_vec()
}

fn campaign_invisible_state_public(handle: u64, generation: u64) -> McCampaignInvisibleSnapshotV2 {
    let mut state = McCampaignInvisibleSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_invisible_state(
            handle,
            generation,
            &raw mut state,
            struct_size::<McCampaignInvisibleSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    state
}

fn campaign_beyond_sight_state_public(
    handle: u64,
    generation: u64,
) -> McCampaignBeyondSightSnapshotV2 {
    let mut state = McCampaignBeyondSightSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_beyond_sight_state(
            handle,
            generation,
            &raw mut state,
            struct_size::<McCampaignBeyondSightSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    state
}

fn campaign_economy_events_public(handle: u64) -> Vec<McCampaignEconomyEventV2> {
    let mut rows = [McCampaignEconomyEventV2::default(); 128];
    let mut count = 0;
    assert_eq!(
        mc_native_campaign_economy_events(
            handle,
            rows.as_mut_ptr(),
            rows.len() as u32,
            struct_size::<McCampaignEconomyEventV2>(),
            &raw mut count,
        ),
        MC_STATUS_OK
    );
    rows[..count as usize].to_vec()
}

fn campaign_world_hash_public(handle: u64) -> u64 {
    let mut info = McCampaignWorldInfoV2::default();
    assert_eq!(
        mc_native_campaign_world_info(
            handle,
            &raw mut info,
            struct_size::<McCampaignWorldInfoV2>(),
        ),
        MC_STATUS_OK
    );
    info.reset_state_hash
}

struct Level2PublicPossessionClaimRoute {
    input: McCampaignStepInputV2,
    latest_snapshot: McCampaignStepSnapshotV2,
    player_entity_index: u16,
    new_claimed_group1_sphere_ids: BTreeSet<u32>,
    merge_lineage: BTreeSet<(u32, u32)>,
}

/// Shared public route used by the Possession predecessor proof and the
/// Invisible consumer. It retains the predecessor's strict-flight and
/// six-claim assertions so later capability tests never replace that
/// evidence with a synthetic mana setup.
fn drive_level2_public_group1_possession_claims(
    handle: u64,
    input: McCampaignStepInputV2,
    selected_target_sphere_id: u32,
    group1_sphere_ids: &BTreeSet<u32>,
) -> Level2PublicPossessionClaimRoute {
    drive_level2_public_group1_possession_claims_with_expected(
        handle,
        input,
        selected_target_sphere_id,
        group1_sphere_ids,
        Some(6),
    )
}

/// Drive one ordinary public Possession request/effect window. The first
/// predecessor proof starts quiescent and requires one exact six-claim
/// chain. The Beyond Sight capacity proof intentionally permits earlier
/// ordinary projectiles/effects to overlap later requests and validates
/// only the resulting aggregate live ownership outside this helper.
fn drive_level2_public_group1_possession_claims_with_expected(
    handle: u64,
    mut input: McCampaignStepInputV2,
    selected_target_sphere_id: u32,
    group1_sphere_ids: &BTreeSet<u32>,
    expected_claim_count: Option<usize>,
) -> Level2PublicPossessionClaimRoute {
    let possession_holder = |hud: McCampaignDayHudSnapshotV2| {
        [hud.left, hud.right]
            .into_iter()
            .find(|slot| slot.raw_spell_selector == 1)
            .expect("Day HUD exposes the Possession holder")
    };
    let initial_target = campaign_mana_spheres_public(handle, 0)
        .into_iter()
        .find(|sphere| sphere.sphere_id == selected_target_sphere_id)
        .expect("selected Possession target remains visible through the public ABI");
    assert_eq!(
        initial_target.owner_entity_index, 0,
        "each public Possession flight starts from a neutral target identity"
    );
    let player_entity_index = campaign_players_public(handle)
        .into_iter()
        .find(|player| player.active != 0 && player.slot == 0)
        .expect("public player projection exposes the local Possession caster")
        .entity_index;
    let mut observed_claim_receipt_ids = BTreeSet::new();
    let mut observed_merge_lineage = BTreeSet::new();
    let capture_public_economy_receipts =
        |receipts: &mut BTreeSet<u32>, merge_lineage: &mut BTreeSet<(u32, u32)>| {
            for event in campaign_economy_events_public(handle) {
                if event.code == MC_CAMPAIGN_ECONOMY_MANA_SPHERE_CLAIMED
                    && event.entity_0 == player_entity_index
                    && group1_sphere_ids.contains(&event.primary_id)
                {
                    receipts.insert(event.primary_id);
                } else if event.code == MC_CAMPAIGN_ECONOMY_MANA_SPHERES_MERGED {
                    // The dispatched a1/primary sphere survives and absorbs
                    // the a2/secondary sphere. Ownership lineage therefore
                    // flows from the claimed source into the surviving target.
                    merge_lineage.insert((event.secondary_id, event.primary_id));
                }
            }
        };

    // Drive one ordinary public flight geometry. In the strict predecessor
    // the model-12 callback fans its mailbox to six overlapping neutral
    // model-39 spheres; the capacity route instead accumulates live public
    // ownership across overlapping ordinary effect windows.
    input.player_x = initial_target.x;
    input.player_y = initial_target.y;
    input.player_z = initial_target.z;
    input.command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        pitch: 127,
        flags: 1,
        requested_spell: MC_REQUESTED_SPELL_NONE,
        ..McCommandV2::default()
    };
    let (approach_snapshot, approach_events) = step_level2_campaign_public(handle, input);
    capture_public_economy_receipts(&mut observed_claim_receipt_ids, &mut observed_merge_lineage);
    if expected_claim_count.is_some() {
        assert!(
            approach_events
                .iter()
                .all(|event| !(event.kind == 8 && matches!(event.family, 39 | 40))),
            "the strict predecessor starts from a publicly quiescent Possession arena"
        );
    }
    let approach_holder = possession_holder(campaign_day_hud_public(
        handle,
        approach_snapshot.generation,
        approach_snapshot.revision,
    ));
    assert_eq!(
        approach_holder.active_value, 0,
        "an input without a selected hand or requested spell cannot re-arm Possession"
    );
    assert_eq!(approach_snapshot.player_entity_index, player_entity_index);
    // Player slot 111 arms holder 116 before the holder's callback in the
    // same source pass. Install the strict-flight launch geometry before
    // the selected request so that the freshly allocated flyer is born at
    // the source-valid position rather than being repositioned one turn
    // too late.
    let target = campaign_mana_spheres_public(handle, 0)
        .into_iter()
        .find(|sphere| sphere.sphere_id == selected_target_sphere_id)
        .expect("armed Possession target remains visible through the public ABI");
    let (yaw, pitch, actual_speed, entity_fov, a4_yaw, a4_pitch) =
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "Possession proof requires a Level 2 campaign session",
                ));
            };
            let auxiliary = reset.shared_spells().beyond_sight_auxiliary();
            Ok((
                reset.state.yaw,
                reset.state.effective_pitch,
                reset.state.actual_speed,
                reset.state.entity_fov,
                auxiliary.next_entity,
                auxiliary.entity_index2,
            ))
        })
        .expect("read public-input flight state");
    assert_eq!(yaw, 0, "the pinned reset route faces source yaw zero");
    assert_eq!(
        (a4_yaw, a4_pitch),
        (0, 0),
        "the campaign C ABI selected-cast route binds the source A4 launch offsets to zero"
    );
    assert!(
        pitch > 0 && actual_speed > 0,
        "public pitch/forward input arms descent"
    );
    input.command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        pitch: 127,
        flags: 0x41,
        requested_spell: 1,
        ..McCommandV2::default()
    };
    let launch_command = player_command(input.command).expect("valid public Possession command");
    let launch_player_speed = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Possession proof requires a Level 2 campaign session",
            ));
        };
        let mut state = reset.state;
        let mut context = reset.context.clone();
        sim_core::step_turn_with_context(
            &mut state,
            launch_command,
            sim_core::FlightProfile::OUTDOOR,
            reset.canonical_terrain(),
            &mut context,
        );
        Ok(state.actual_speed)
    })
    .expect("predict source-ordered Possession constructor speed");
    let projectile_speed = 384_i16.wrapping_add(launch_player_speed);
    // The existing lower source row advances before the newly allocated
    // higher Possession flyer on the launch pass. Predict that public
    // sphere visit, then put the carrier one strict collision radius
    // short of the flyer's first move. The one-shot acquisition thereby
    // observes live source-ordered geometry instead of a stale position.
    let mut contact = Position {
        x: target.x,
        y: target.y,
        z: target.z,
    };
    let attraction_step = u16::try_from(target.reserved[1] & 0xffff)
        .expect("public attraction step occupies the low source word");
    let attraction_yaw = usize::from(target.yaw.cast_unsigned() & 0x07ff);
    let attraction_dx = if target.reserved[0] == 0 {
        0
    } else {
        i16::try_from(
            (i64::from(attraction_step) * i64::from(sim_core::SIN_16_16[attraction_yaw])) >> 16,
        )
        .expect("public attraction X displacement fits i16")
    };
    let attraction_dy = if target.reserved[0] == 0 {
        0
    } else {
        i16::try_from(
            (i64::from(attraction_step) * i64::from(sim_core::SIN_16_16[512 + attraction_yaw]))
                >> 16,
        )
        .expect("public attraction Y displacement fits i16")
    };
    let sphere_velocity_x = target.velocity_x.clamp(
        -sim_core::level1_mission::MANA_SPHERE_MAX_XY_VELOCITY,
        sim_core::level1_mission::MANA_SPHERE_MAX_XY_VELOCITY,
    );
    let sphere_velocity_y = target.velocity_y.clamp(
        -sim_core::level1_mission::MANA_SPHERE_MAX_XY_VELOCITY,
        sim_core::level1_mission::MANA_SPHERE_MAX_XY_VELOCITY,
    );
    let mut sphere_vertical_velocity = target.vertical_velocity;
    for _ in 0..1 {
        contact.x = contact
            .x
            .wrapping_add_signed(attraction_dx)
            .wrapping_add_signed(sphere_velocity_x);
        contact.y = contact
            .y
            .wrapping_sub_signed(attraction_dy)
            .wrapping_add_signed(sphere_velocity_y);
        contact.z = contact.z.wrapping_add(sphere_vertical_velocity);
        sphere_vertical_velocity = sphere_vertical_velocity
            .wrapping_sub(sim_core::level1_mission::MANA_SPHERE_GRAVITY)
            .max(sim_core::level1_mission::MANA_SPHERE_TERMINAL_VELOCITY);
    }

    let target_dimension = sim_core::level1_mission::MANA_SPHERE_COLLISION_DIMENSIONS[usize::from(
        sim_core::level1_mission::mana_sphere_size_ordinal(target.mana),
    )];
    let strict_collision_reach =
        sim_core::level1_mission::POSSESSION_PROJECTILE_COLLISION_HALF_WIDTH
            .wrapping_add(target_dimension)
            .wrapping_sub(1);
    let launch_distance = projectile_speed.wrapping_sub(strict_collision_reach);
    assert!(
        launch_distance > 0,
        "the source flyer step must exceed the strict target collision reach"
    );
    let launch_yaw = usize::from(yaw.cast_unsigned() & 0x07ff);
    let launch_dx = i16::try_from(
        (i64::from(launch_distance) * i64::from(sim_core::SIN_16_16[launch_yaw])) >> 16,
    )
    .expect("Possession launch X offset fits i16");
    let launch_dy = i16::try_from(
        (i64::from(launch_distance) * i64::from(sim_core::SIN_16_16[512 + launch_yaw])) >> 16,
    )
    .expect("Possession launch Y offset fits i16");
    let launch_position = Position {
        // `sub_108B0` rounds the flyer cell while map linkage uses the
        // raw high byte and class-0 searches only current/+X. A one-unit
        // high-side origin makes the acquired move finish on the target's
        // searchable low side at this exact 0x80 boundary.
        x: contact.x.wrapping_sub_signed(launch_dx).wrapping_add(1),
        y: contact.y.wrapping_add_signed(launch_dy),
        z: contact.z.wrapping_add(target_dimension),
    };
    // Selected/left `sub_68E50` displaces the raw parent 256 units at
    // yaw-512 before adding parent FOV. Supply the inverse public origin
    // so the production flyer retains the predecessor's pinned strict
    // collision geometry after that source carrier was restored.
    let desired_player_position = Position {
        x: launch_position.x.wrapping_add(256),
        y: launch_position.y,
        z: launch_position.z.wrapping_sub(entity_fov),
    };
    let (corrected_player_position, predicted_player_position) =
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "Possession proof requires a Level 2 campaign session",
                ));
            };
            let predict_player_position = |position: Position| {
                let mut state = reset.state;
                state.position = position;
                let mut context = reset.context.clone();
                sim_core::step_turn_with_context(
                    &mut state,
                    launch_command,
                    sim_core::FlightProfile::OUTDOOR,
                    reset.canonical_terrain(),
                    &mut context,
                );
                state.position
            };
            let trial = predict_player_position(desired_player_position);
            let corrected = Position {
                x: desired_player_position
                    .x
                    .wrapping_add(desired_player_position.x.wrapping_sub(trial.x)),
                y: desired_player_position
                    .y
                    .wrapping_add(desired_player_position.y.wrapping_sub(trial.y)),
                z: desired_player_position
                    .z
                    .wrapping_add(desired_player_position.z.wrapping_sub(trial.z)),
            };
            Ok((corrected, predict_player_position(corrected)))
        })
        .expect("predict source-ordered Possession launch carrier");
    input.player_x = corrected_player_position.x;
    input.player_y = corrected_player_position.y;
    input.player_z = corrected_player_position.z;
    let (armed_snapshot, arm_events) = step_level2_campaign_public(handle, input);
    assert_eq!(
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "Possession proof requires a Level 2 campaign session",
                ));
            };
            Ok(reset.state.position)
        })
        .expect("read armed public-input flight state"),
        predicted_player_position,
        "the real C-ABI arm pass must match the canonical committed launch carrier"
    );
    capture_public_economy_receipts(&mut observed_claim_receipt_ids, &mut observed_merge_lineage);
    let mut saw_effect = arm_events
        .iter()
        .any(|event| event.kind == 8 && event.family == 40);
    let arm_projectile_events = arm_events
        .iter()
        .filter(|event| event.kind == 8 && event.family == 39)
        .collect::<Vec<_>>();
    if expected_claim_count.is_some() {
        assert!(
            arm_projectile_events.len() <= 1 && (!arm_projectile_events.is_empty() || saw_effect),
            "the quiescent selected request either publishes its one live guided flyer or its same-pass terminal effect"
        );
    }
    let armed_holder = possession_holder(campaign_day_hud_public(
        handle,
        armed_snapshot.generation,
        armed_snapshot.revision,
    ));
    assert_eq!(
        armed_holder.active_value, 2,
        "the same-pass holder callback advances Tier-0 Possession from three to two"
    );
    assert_eq!(armed_holder.activation_value, 3);
    let mut possession_projectile_source_slots = arm_projectile_events
        .into_iter()
        .map(|event| event.source_slot)
        .collect::<BTreeSet<_>>();

    input.command = valid_command();
    let (mut latest_snapshot, no_second_events) = step_level2_campaign_public(handle, input);
    capture_public_economy_receipts(&mut observed_claim_receipt_ids, &mut observed_merge_lineage);
    possession_projectile_source_slots.extend(
        no_second_events
            .iter()
            .filter(|event| event.kind == 8 && event.family == 39)
            .map(|event| event.source_slot),
    );
    if expected_claim_count.is_some() {
        assert!(
            possession_projectile_source_slots.len() <= 1,
            "the active-repeat path cannot publish a second guided flyer source slot"
        );
    }
    saw_effect |= no_second_events
        .iter()
        .any(|event| event.kind == 8 && event.family == 40);
    for _ in 0..9 {
        if expected_claim_count.is_some_and(|expected| observed_claim_receipt_ids.len() >= expected)
            || (expected_claim_count.is_none() && !observed_claim_receipt_ids.is_empty())
        {
            break;
        }
        let (snapshot, events) = step_level2_campaign_public(handle, input);
        capture_public_economy_receipts(
            &mut observed_claim_receipt_ids,
            &mut observed_merge_lineage,
        );
        possession_projectile_source_slots.extend(
            events
                .iter()
                .filter(|event| event.kind == 8 && event.family == 39)
                .map(|event| event.source_slot),
        );
        if expected_claim_count.is_some() {
            assert!(
                possession_projectile_source_slots.len() <= 1,
                "only the activated guided Possession flyer may remain in the strict predecessor route"
            );
        }
        saw_effect |= events
            .iter()
            .any(|event| event.kind == 8 && event.family == 40);
        latest_snapshot = snapshot;
    }
    if expected_claim_count.is_some() {
        assert!(
            saw_effect,
            "strict model-39 flight collision allocates a typed model-12 effect"
        );
    }
    let mut holder = possession_holder(campaign_day_hud_public(
        handle,
        latest_snapshot.generation,
        latest_snapshot.revision,
    ));
    for _ in 0..3 {
        if holder.active_value == 0 {
            break;
        }
        let (snapshot, events) = step_level2_campaign_public(handle, input);
        capture_public_economy_receipts(
            &mut observed_claim_receipt_ids,
            &mut observed_merge_lineage,
        );
        possession_projectile_source_slots.extend(
            events
                .iter()
                .filter(|event| event.kind == 8 && event.family == 39)
                .map(|event| event.source_slot),
        );
        if expected_claim_count.is_some() {
            assert!(
                possession_projectile_source_slots.len() <= 1,
                "cooldown ticks do not publish a second Possession projectile source slot"
            );
        }
        holder = possession_holder(campaign_day_hud_public(
            handle,
            snapshot.generation,
            snapshot.revision,
        ));
        latest_snapshot = snapshot;
    }
    assert_eq!(
        holder.active_value, 0,
        "the public Possession cooldown reaches zero before re-arm"
    );
    let claimed_group1_sphere_ids = campaign_mana_spheres_public(handle, 0)
        .into_iter()
        .filter(|sphere| {
            sphere.active != 0
                && sphere.model == 39
                && sphere.owner_entity_index == player_entity_index
                && group1_sphere_ids.contains(&sphere.sphere_id)
        })
        .map(|sphere| sphere.sphere_id)
        .collect::<BTreeSet<_>>();
    if let Some(expected_claim_count) = expected_claim_count {
        assert_eq!(
            observed_claim_receipt_ids.len(),
            expected_claim_count,
            "one C-ABI source/effect chain emits exactly the expected number of distinct public claim receipts (target={selected_target_sphere_id} at {:?}, receipts={observed_claim_receipt_ids:?}, live={claimed_group1_sphere_ids:?})",
            (target.x, target.y, target.z)
        );
    }

    Level2PublicPossessionClaimRoute {
        input,
        latest_snapshot,
        player_entity_index,
        new_claimed_group1_sphere_ids: observed_claim_receipt_ids,
        merge_lineage: observed_merge_lineage,
    }
}

fn campaign_goat_events(handle: u64) -> Vec<McCampaignGoatEventV2> {
    let mut events = [McCampaignGoatEventV2::default(); 1024];
    let mut event_count = 0;
    assert_eq!(
        mc_native_campaign_goat_events(
            handle,
            events.as_mut_ptr(),
            events.len() as u32,
            struct_size::<McCampaignGoatEventV2>(),
            &raw mut event_count,
        ),
        MC_STATUS_OK
    );
    events[..event_count as usize].to_vec()
}

fn campaign_authored_group_events(handle: u64) -> Vec<McCampaignAuthoredGroupEventV2> {
    let mut events = [McCampaignAuthoredGroupEventV2::default(); 4096];
    let mut event_count = 0;
    assert_eq!(
        mc_native_campaign_authored_group_events(
            handle,
            events.as_mut_ptr(),
            events.len() as u32,
            struct_size::<McCampaignAuthoredGroupEventV2>(),
            &raw mut event_count,
        ),
        MC_STATUS_OK
    );
    events[..event_count as usize].to_vec()
}

include!("abi_campaign_contracts.rs");
include!("campaign_runtime.rs");
include!("campaign_spells.rs");
include!("presentation_terrain.rs");
include!("audio_errors.rs");
include!("level1_spells.rs");
include!("level1_world.rs");
