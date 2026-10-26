#[test]
fn camera_history_preserves_raw_wrapping_yaw_until_render_mapping() {
    let mut state = FlightState {
        yaw: 2_040,
        ..FlightState::default()
    };
    let positive = CameraHistory::capture(&state, 0, 128, 32, 0);
    assert_eq!(positive.yaw, 2_072);
    assert_eq!(positive.yaw & 0x07ff, 24);

    state.yaw = 8;
    let negative = CameraHistory::capture(&state, 0, 128, -32, 0);
    assert_eq!(negative.yaw, -24);
    assert_eq!(negative.yaw & 0x07ff, 2_024);

    state.yaw = i16::MAX;
    let signed_overflow = CameraHistory::capture(&state, 0, 128, 1, 0);
    assert_eq!(signed_overflow.yaw, i16::MIN);
    assert_eq!(signed_overflow.yaw & 0x07ff, 0);
}

#[test]
fn zero_one_and_many_presentation_reads_cannot_advance_notification_gameplay_state() {
    let handles = [
        open_fixture_with_presentation_cache(),
        open_fixture_with_presentation_cache(),
        open_fixture_with_presentation_cache(),
    ];
    let presentation_input = source_presentation_input();
    for handle in handles {
        with_session_mut(handle, |session| {
            session.apply_presentation_input(presentation_input)?;
            session.mission.player_notification_active = true;
            session.mission.player_notification_id =
                sim_core::level1_mission::ABILITY_IMPROVED_NAME_SELECTOR_BASE + 16;
            session.mission.player_notification_state =
                sim_core::level1_mission::ABILITY_IMPROVED_NOTIFICATION_STATE;
            session.mission.player_notification_timer = 2;
            Ok(())
        })
        .unwrap();
    }
    let query = valid_presentation_frame_query();
    with_session_mut(handles[1], |session| {
        session.presentation_frame(query).map(|_| ())
    })
    .unwrap();
    for _ in 0..4 {
        with_session_mut(handles[2], |session| {
            session.presentation_frame(query).map(|_| ())
        })
        .unwrap();
    }

    let command = PlayerCommand::default();
    for handle in handles {
        with_session_mut(handle, |session| {
            session.step(command);
            Ok(())
        })
        .unwrap();
    }
    let observations = handles.map(|handle| {
        with_session(handle, |session| {
            Ok((
                session.snapshot(),
                session.live_world_hash_v2(),
                session.mission.global_rng(),
                session.events.clone(),
                session.selector_frames.clone(),
                session.archer_selector_frames.clone(),
                session.firefly_selector_frames.clone(),
                session.villager_selector_frames.clone(),
                session.fireball_selector_frame.clone(),
                session.possession_selector_frame.clone(),
                session.castle_selector_frame.clone(),
                session.selector_draw_feedback,
            ))
        })
        .unwrap()
    });
    assert_eq!(observations[0], observations[1]);
    assert_eq!(observations[0], observations[2]);
    for handle in handles {
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }
}

#[test]
fn render_camera_sidecar_exposes_stateful_slope_offset_and_reset() {
    let handle = open_fixture();
    let read = || {
        let mut render = McRenderCameraStateV2::default();
        assert_eq!(
            mc_native_render_camera_state(
                handle,
                &raw mut render,
                struct_size::<McRenderCameraStateV2>(),
            ),
            MC_STATUS_OK
        );
        render
    };
    let initial = read();
    let (initial_history, initial_slope) = with_session(handle, |session| {
        Ok((session.camera_history, session.camera_slope_sample))
    })
    .unwrap();
    assert_eq!(
        (
            initial.history_x,
            initial.history_y,
            initial.eye_z,
            initial.slope_target_x,
            initial.slope_target_y,
            initial.render_offset_x,
            initial.render_offset_y,
        ),
        (
            initial_history.x,
            initial_history.y,
            initial_history.z,
            initial_slope.target_x,
            initial_slope.target_y,
            initial_slope.offset_x,
            initial_slope.offset_y,
        )
    );
    assert_eq!(initial.authority, MC_AUTHORITY_PLAYER_FLIGHT_LIVE);
    assert_eq!(initial.flags, MC_RENDER_CAMERA_TERRAIN_SLOPE_XY);

    let command = McCommandV2 {
        roll: 38,
        flags: 1,
        ..valid_command()
    };
    let camera_input = valid_camera_input();
    let mut snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step_with_camera(
            handle,
            &raw const command,
            &raw const camera_input,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let advanced = read();
    let advanced_slope = with_session(handle, |session| Ok(session.camera_slope_sample)).unwrap();
    assert_eq!(
        (
            advanced.slope_target_x,
            advanced.slope_target_y,
            advanced.render_offset_x,
            advanced.render_offset_y,
        ),
        (
            advanced_slope.target_x,
            advanced_slope.target_y,
            advanced_slope.offset_x,
            advanced_slope.offset_y,
        )
    );

    assert_eq!(
        mc_native_session_reset(
            handle,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let reset = read();
    assert_eq!(
        (
            reset.history_x,
            reset.history_y,
            reset.eye_z,
            reset.slope_target_x,
            reset.slope_target_y,
            reset.render_offset_x,
            reset.render_offset_y,
        ),
        (
            initial.history_x,
            initial.history_y,
            initial.eye_z,
            initial.slope_target_x,
            initial.slope_target_y,
            initial.render_offset_x,
            initial.render_offset_y,
        ),
        "reset zeros the persistent accumulators before the initial draw update"
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn presentation_camera_is_the_pre_entity_update_history_record() {
    let handle = open_fixture();
    let (before_state, before_turn) =
        with_session(handle, |session| Ok((session.state, session.source_turn))).unwrap();
    let command = McCommandV2 {
        roll: 38,
        flags: 1,
        ..valid_command()
    };
    let camera_input = McCameraInputV2 {
        look_yaw: 64,
        look_pitch: (-32_i16).cast_unsigned(),
        ..valid_camera_input()
    };
    let expected = CameraHistory::capture(&before_state, before_turn.wrapping_add(1), 128, 64, -32);
    let mut snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step_with_camera(
            handle,
            &raw const command,
            &raw const camera_input,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let mut presentation = McPresentationStateV2::default();
    assert_eq!(
        mc_native_presentation_state(
            handle,
            &raw mut presentation,
            struct_size::<McPresentationStateV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(presentation.tick, expected.tick);
    assert_eq!(presentation.source_turn, expected.source_turn);
    assert_eq!(
        (
            presentation.camera_x,
            presentation.camera_y,
            presentation.camera_z,
            presentation.camera_yaw,
            presentation.camera_pitch,
            presentation.camera_roll,
            presentation.camera_fov,
        ),
        (
            expected.x,
            expected.y,
            expected.z,
            expected.yaw,
            expected.pitch,
            expected.roll,
            expected.fov,
        )
    );
    assert_eq!(snapshot.tick, before_state.tick + 1);
    assert_ne!(
        (snapshot.x, snapshot.y, snapshot.z, snapshot.yaw),
        (
            presentation.camera_x,
            presentation.camera_y,
            presentation.camera_z.wrapping_sub(128),
            presentation.camera_yaw,
        ),
        "movement advances after the stored camera record"
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn presentation_camera_history_tracks_consecutive_turns_and_reset() {
    let handle = open_fixture();
    let first_command = McCommandV2 {
        roll: 38,
        flags: 1,
        ..valid_command()
    };
    let first_camera = McCameraInputV2 {
        look_yaw: 64,
        look_pitch: (-32_i16).cast_unsigned(),
        ..valid_camera_input()
    };
    let mut first_snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step_with_camera(
            handle,
            &raw const first_command,
            &raw const first_camera,
            &raw mut first_snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );

    let (before_second, before_second_turn) =
        with_session(handle, |session| Ok((session.state, session.source_turn))).unwrap();
    let second_command = McCommandV2 {
        roll: -19,
        flags: 1,
        ..valid_command()
    };
    let second_camera = McCameraInputV2 {
        look_yaw: 21,
        look_pitch: 9,
        ..valid_camera_input()
    };
    let expected_second = CameraHistory::capture(
        &before_second,
        before_second_turn.wrapping_add(1),
        128,
        21,
        9,
    );
    let mut second_snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step_with_camera(
            handle,
            &raw const second_command,
            &raw const second_camera,
            &raw mut second_snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let mut presentation = McPresentationStateV2::default();
    assert_eq!(
        mc_native_presentation_state(
            handle,
            &raw mut presentation,
            struct_size::<McPresentationStateV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (
            presentation.tick,
            presentation.source_turn,
            presentation.camera_x,
            presentation.camera_y,
            presentation.camera_z,
            presentation.camera_yaw,
            presentation.camera_pitch,
            presentation.camera_roll,
        ),
        (
            expected_second.tick,
            expected_second.source_turn,
            expected_second.x,
            expected_second.y,
            expected_second.z,
            expected_second.yaw,
            expected_second.pitch,
            expected_second.roll,
        )
    );
    assert_eq!(presentation.tick, first_snapshot.tick);
    assert_eq!(
        (
            presentation.camera_x,
            presentation.camera_y,
            presentation.camera_z.wrapping_sub(128),
        ),
        (first_snapshot.x, first_snapshot.y, first_snapshot.z)
    );
    assert_eq!(second_snapshot.tick, first_snapshot.tick + 1);

    let mut reset_snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_reset(
            handle,
            &raw mut reset_snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_presentation_state(
            handle,
            &raw mut presentation,
            struct_size::<McPresentationStateV2>(),
        ),
        MC_STATUS_OK
    );
    let reset_history = with_session(handle, |session| Ok(session.camera_history)).unwrap();
    assert_eq!(presentation.generation, reset_snapshot.generation);
    assert_eq!(presentation.tick, reset_snapshot.tick);
    assert_eq!(
        (
            presentation.camera_x,
            presentation.camera_y,
            presentation.camera_z.wrapping_sub(128),
        ),
        (reset_snapshot.x, reset_snapshot.y, reset_snapshot.z)
    );
    assert_eq!(
        (
            presentation.source_turn,
            presentation.camera_yaw,
            presentation.camera_pitch,
            presentation.camera_roll,
            presentation.camera_fov,
        ),
        (
            reset_history.source_turn,
            reset_history.yaw,
            reset_history.pitch,
            reset_history.roll,
            reset_history.fov,
        )
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn presentation_camera_base_preserves_source_ground_clearance() {
    let handle = open_fixture();
    for turn in 0..192 {
        let command = McCommandV2 {
            roll: match turn % 4 {
                0 => 38,
                1 => 0,
                2 => -38,
                _ => 0,
            },
            flags: 1,
            ..valid_command()
        };
        let camera_input = valid_camera_input();
        let mut snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step_with_camera(
                handle,
                &raw const command,
                &raw const camera_input,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut presentation = McPresentationStateV2::default();
        assert_eq!(
            mc_native_presentation_state(
                handle,
                &raw mut presentation,
                struct_size::<McPresentationStateV2>(),
            ),
            MC_STATUS_OK
        );
        let base = Position {
            x: presentation.camera_x,
            y: presentation.camera_y,
            z: presentation.camera_z.wrapping_sub(128),
        };
        let (ground, clearance) = with_session(handle, |session| {
            Ok((
                session.world.ground_height(base),
                session.world.profile().ground_clearance,
            ))
        })
        .unwrap();
        assert!(
            i32::from(base.z) >= i32::from(ground) + i32::from(clearance),
            "turn {turn}: camera base z={} is below ground {} + clearance {}",
            base.z,
            ground,
            clearance
        );
    }
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
#[cfg(not(windows))]
fn c_header_compiles_and_locks_the_rust_layout() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("mc-sim-header-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let source = directory.join("layout.c");
    let object = directory.join("layout.o");
    fs::write(
        &source,
        r#"
#include <stddef.h>
#include "mc_sim_campaign.h"

_Static_assert(sizeof(McWorldInfoV2) == 72, "McWorldInfoV2 size");
_Static_assert(sizeof(McCommandV2) == 32, "McCommandV2 size");
_Static_assert(sizeof(McCameraInputV2) == 28, "McCameraInputV2 size");
_Static_assert(sizeof(McSessionSnapshotV2) == 120, "McSessionSnapshotV2 size");
_Static_assert(sizeof(McObjectiveSnapshotV2) == 40, "McObjectiveSnapshotV2 size");
_Static_assert(sizeof(McEntitySnapshotV2) == 52, "McEntitySnapshotV2 size");
_Static_assert(sizeof(McCampaignSelectionV2) == 32, "McCampaignSelectionV2 size");
_Static_assert(sizeof(McCampaignStepInputV2) == 80, "McCampaignStepInputV2 size");
_Static_assert(sizeof(McCampaignEventV2) == 48, "McCampaignEventV2 size");
_Static_assert(sizeof(McCampaignAuthoredGroupEventV2) == 48, "McCampaignAuthoredGroupEventV2 size");
_Static_assert(sizeof(McCampaignEconomyEventV2) == 64, "McCampaignEconomyEventV2 size");
_Static_assert(sizeof(McCampaignGoatEventV2) == 64, "McCampaignGoatEventV2 size");
_Static_assert(sizeof(McCampaignStepSnapshotV2) == 112, "McCampaignStepSnapshotV2 size");
_Static_assert(sizeof(McCampaignEntityPresentationV2) == 64, "campaign sprite size");
_Static_assert(offsetof(McCampaignEntityPresentationV2, pixel_mode) == 48, "campaign sprite pixel operation offset");
_Static_assert(offsetof(McCampaignEntityPresentationV2, draw_order) == 52, "campaign sprite source order offset");
_Static_assert(offsetof(McCampaignEntityPresentationV2, reserved) == 60, "campaign sprite reserved offset");
_Static_assert(offsetof(McCampaignStepInputV2, command) == 16, "campaign step command offset");
_Static_assert(offsetof(McCampaignEventV2, source_slot) == 24, "campaign event source offset");
_Static_assert(offsetof(McCampaignAuthoredGroupEventV2, authored_index) == 28, "campaign authored-group index offset");
_Static_assert(offsetof(McCampaignEconomyEventV2, primary_id) == 28, "campaign economy primary offset");
_Static_assert(offsetof(McCampaignEconomyEventV2, value_0) == 40, "campaign economy value offset");
_Static_assert(offsetof(McCampaignGoatEventV2, entity_0) == 28, "campaign goat entity offset");
_Static_assert(offsetof(McCampaignGoatEventV2, value_0) == 32, "campaign goat value offset");
_Static_assert(offsetof(McCampaignGoatEventV2, sound_id) == 40, "campaign goat sound offset");
_Static_assert(offsetof(McCampaignGoatEventV2, asset_ref_index) == 52, "campaign goat asset offset");
_Static_assert(offsetof(McCampaignStepSnapshotV2, dispatch_hash) == 40, "campaign step hash offset");
_Static_assert(sizeof(McCampaignWorldInfoV2) == 64, "McCampaignWorldInfoV2 size");
_Static_assert(sizeof(McCampaignTerrainStateV2) == 64, "McCampaignTerrainStateV2 size");
_Static_assert(sizeof(McCampaignDayMusicSnapshotV2) == 80, "McCampaignDayMusicSnapshotV2 size");
_Static_assert(sizeof(McCampaignDayIndexedFrameInfoV2) == 152, "McCampaignDayIndexedFrameInfoV2 size");
_Static_assert(offsetof(McCampaignDayIndexedFrameInfoV2, index_bytes) == 32, "Day indexed frame bytes offset");
_Static_assert(offsetof(McCampaignDayIndexedFrameInfoV2, index_hash) == 40, "Day indexed frame index hash offset");
_Static_assert(offsetof(McCampaignDayIndexedFrameInfoV2, package_sha256) == 72, "Day indexed frame package hash offset");
_Static_assert(offsetof(McCampaignDayIndexedFrameInfoV2, cache_content_sha256) == 104, "Day indexed frame cache hash offset");
_Static_assert(offsetof(McCampaignDayIndexedFrameInfoV2, reserved) == 136, "Day indexed frame reserved offset");
_Static_assert(sizeof(McCampaignPlayerSnapshotV2) == 48, "McCampaignPlayerSnapshotV2 size");
_Static_assert(sizeof(McCampaignEntitySnapshotV2) == 52, "McCampaignEntitySnapshotV2 size");
_Static_assert(sizeof(McCampaignShieldSnapshotV2) == 72, "McCampaignShieldSnapshotV2 size");
_Static_assert(sizeof(McCampaignInvisibleSnapshotV2) == 80, "McCampaignInvisibleSnapshotV2 size");
_Static_assert(sizeof(McCampaignBeyondSightSnapshotV2) == 80, "McCampaignBeyondSightSnapshotV2 size");
_Static_assert(sizeof(McCampaignOpponentSnapshotV2) == 104, "McCampaignOpponentSnapshotV2 size");
_Static_assert(sizeof(McCampaignDayHudSpellSlotV2) == 48, "McCampaignDayHudSpellSlotV2 size");
_Static_assert(sizeof(McCampaignDayHudSnapshotV2) == 272, "McCampaignDayHudSnapshotV2 size");
_Static_assert(sizeof(McCampaignMinimapStateV2) == 72, "McCampaignMinimapStateV2 size");
_Static_assert(sizeof(McCampaignMinimapEntityV2) == 56, "McCampaignMinimapEntityV2 size");
_Static_assert(sizeof(McCampaignMinimapMarkV2) == 48, "McCampaignMinimapMarkV2 size");
_Static_assert(offsetof(McCampaignMinimapStateV2, entity_count) == 28, "campaign minimap entity count offset");
_Static_assert(offsetof(McCampaignMinimapStateV2, entity_hash) == 40, "campaign minimap entity hash offset");
_Static_assert(offsetof(McCampaignMinimapEntityV2, source_order) == 24, "campaign minimap entity order offset");
_Static_assert(offsetof(McCampaignMinimapEntityV2, x) == 28, "campaign minimap entity x offset");
_Static_assert(offsetof(McCampaignMinimapEntityV2, selector) == 32, "campaign minimap entity selector offset");
_Static_assert(offsetof(McCampaignMinimapEntityV2, help_selector) == 38, "campaign minimap entity help offset");
_Static_assert(offsetof(McCampaignMinimapMarkV2, x) == 28, "campaign minimap mark x offset");
_Static_assert(offsetof(McCampaignMinimapMarkV2, palette_index) == 32, "campaign minimap mark palette offset");
_Static_assert(offsetof(McCampaignBeyondSightSnapshotV2, experience_awarded) == 52, "campaign Beyond Sight experience offset");
_Static_assert(offsetof(McCampaignOpponentSnapshotV2, entity_control_flags) == 36, "campaign opponent control flags offset");
_Static_assert(offsetof(McCampaignOpponentSnapshotV2, objective_relation_value) == 76, "campaign opponent objective relation offset");
_Static_assert(offsetof(McCampaignOpponentSnapshotV2, visibility_flags) == 80, "campaign opponent visibility flags offset");
_Static_assert(offsetof(McCampaignDayHudSpellSlotV2, max_mana) == 20, "campaign Day HUD spell max mana offset");
_Static_assert(offsetof(McCampaignDayHudSpellSlotV2, draw_admitted) == 37, "campaign Day HUD spell draw admission offset");
_Static_assert(offsetof(McCampaignDayHudSnapshotV2, global_mana_denominator) == 52, "campaign Day HUD denominator offset");
_Static_assert(offsetof(McCampaignDayHudSnapshotV2, player_panel_selector) == 62, "campaign Day HUD player panel offset");
_Static_assert(offsetof(McCampaignDayHudSnapshotV2, life_palette_index) == 68, "campaign Day HUD palette offset");
_Static_assert(offsetof(McCampaignDayHudSnapshotV2, nyphur) == 72, "campaign Day HUD Nyphur offset");
_Static_assert(offsetof(McCampaignDayHudSnapshotV2, left) == 176, "campaign Day HUD left offset");
_Static_assert(offsetof(McCampaignDayHudSnapshotV2, right) == 224, "campaign Day HUD right offset");
_Static_assert(sizeof(McGoatSnapshotV2) == 84, "McGoatSnapshotV2 size");
_Static_assert(sizeof(McCampaignGoatSnapshotV2) == 84, "McCampaignGoatSnapshotV2 size");
_Static_assert(sizeof(McCampaignSpellPickupSnapshotV2) == 64, "McCampaignSpellPickupSnapshotV2 size");
_Static_assert(sizeof(McGoatBatchV2) == 64, "McGoatBatchV2 size");
_Static_assert(sizeof(McCampaignManaBatchV2) == 64, "McCampaignManaBatchV2 size");
_Static_assert(sizeof(McLevel1MissionSnapshotV2) == 96, "McLevel1MissionSnapshotV2 size");
_Static_assert(sizeof(McLevel1PlayerLifecycleSnapshotV2) == 144, "McLevel1PlayerLifecycleSnapshotV2 size");
_Static_assert(sizeof(McLevel1HudSpellSlotV2) == 36, "McLevel1HudSpellSlotV2 size");
_Static_assert(sizeof(McLevel1HudSnapshotV2) == 128, "McLevel1HudSnapshotV2 size");
_Static_assert(sizeof(McLevel1SpellMenuSnapshotV2) == 64, "McLevel1SpellMenuSnapshotV2 size");
_Static_assert(sizeof(McLevel1MinimapMarksV2) == 168, "McLevel1MinimapMarksV2 size");
_Static_assert(sizeof(McLevel1MinimapEntityCommandV2) == 24, "McLevel1MinimapEntityCommandV2 size");
_Static_assert(sizeof(McLevel1MinimapEntitiesInfoV2) == 64, "McLevel1MinimapEntitiesInfoV2 size");
_Static_assert(sizeof(McLevel1PaletteInfoV2) == 80, "McLevel1PaletteInfoV2 size");
_Static_assert(sizeof(McLevel1ExitPresentationV2) == 64, "McLevel1ExitPresentationV2 size");
_Static_assert(sizeof(McLevel1CastleSnapshotV2) == 80, "McLevel1CastleSnapshotV2 size");
_Static_assert(sizeof(McTerrainStateV2) == 64, "McTerrainStateV2 size");
_Static_assert(sizeof(McLevel1CastleLifecycleSnapshotV2) == 80, "McLevel1CastleLifecycleSnapshotV2 size");
_Static_assert(sizeof(McLevel1HouseCompletionEffectV2) == 80, "McLevel1HouseCompletionEffectV2 size");
_Static_assert(sizeof(McManaSphereSnapshotV2) == 64, "McManaSphereSnapshotV2 size");
_Static_assert(sizeof(McManaCarrierSnapshotV2) == 80, "McManaCarrierSnapshotV2 size");
_Static_assert(sizeof(McLevel1ActorSnapshotV2) == 56, "McLevel1ActorSnapshotV2 size");
_Static_assert(sizeof(McLevel1MultipartSegmentSnapshotV2) == 64, "McLevel1MultipartSegmentSnapshotV2 size");
_Static_assert(sizeof(McFireballProjectileSnapshotV2) == 56, "McFireballProjectileSnapshotV2 size");
_Static_assert(sizeof(McFireballImpactSnapshotV2) == 52, "McFireballImpactSnapshotV2 size");
_Static_assert(sizeof(McSelectorFrameStateV2) == 64, "McSelectorFrameStateV2 size");
_Static_assert(sizeof(McSelectorFrameBatchV2) == 88, "McSelectorFrameBatchV2 size");
_Static_assert(sizeof(McSelectorDrawFeedbackV2) == 48, "McSelectorDrawFeedbackV2 size");
_Static_assert(sizeof(McGoatDamageCommandV2) == 40, "McGoatDamageCommandV2 size");
_Static_assert(sizeof(McAssetRefInfoV2) == 32, "McAssetRefInfoV2 size");
_Static_assert(sizeof(McEventQueryV2) == 40, "McEventQueryV2 size");
_Static_assert(sizeof(McEventBatchV2) == 56, "McEventBatchV2 size");
_Static_assert(sizeof(McEventV2) == 72, "McEventV2 size");
_Static_assert(sizeof(McCampaignObjectiveStageSnapshotV2) == 48, "McCampaignObjectiveStageSnapshotV2 size");
_Static_assert(sizeof(McLevel1ResidentObservationV1) == 40, "McLevel1ResidentObservationV1 size");
_Static_assert(offsetof(McLevel1ResidentObservationV1, digest) == 16, "resident observation digest offset");
_Static_assert(sizeof(McAudioCommandV2) == 88, "McAudioCommandV2 size");
_Static_assert(sizeof(McPresentationStateV2) == 64, "McPresentationStateV2 size");
_Static_assert(sizeof(McRenderCameraStateV2) == 72, "McRenderCameraStateV2 size");
_Static_assert(sizeof(McTerrainPnt5VectorV2) == 56, "McTerrainPnt5VectorV2 size");
_Static_assert(sizeof(McTerrainGridVertexV2) == 64, "McTerrainGridVertexV2 size");
_Static_assert(sizeof(McTerrainFrameQueryV2) == 40, "McTerrainFrameQueryV2 size");
_Static_assert(sizeof(McTerrainFrameInfoV2) == 96, "McTerrainFrameInfoV2 size");
_Static_assert(sizeof(McPresentationFrameQueryV2) == 64, "McPresentationFrameQueryV2 size");
_Static_assert(sizeof(McLevel1PresentationInputV2) == 740, "McLevel1PresentationInputV2 size");
_Static_assert(sizeof(McPresentationFrameInfoV2) == 128, "McPresentationFrameInfoV2 size");
_Static_assert(offsetof(McWorldInfoV2, reset_state_hash) == 40, "world hash offset");
_Static_assert(offsetof(McCommandV2, requested_spell) == 14, "spell offset");
_Static_assert(offsetof(McCameraInputV2, look_yaw) == 8, "camera yaw offset");
_Static_assert(offsetof(McCameraInputV2, reserved) == 12, "camera reserved offset");
_Static_assert(offsetof(McSessionSnapshotV2, entity_index) == 48, "entity offset");
_Static_assert(offsetof(McSessionSnapshotV2, objective_textbox_ticks) == 77, "objective ticks offset");
_Static_assert(offsetof(McSessionSnapshotV2, objectives_authority) == 79, "objective authority offset");
_Static_assert(offsetof(McSessionSnapshotV2, left_spell_holder) == 82, "holder offset");
_Static_assert(offsetof(McSessionSnapshotV2, reserved) == 104, "snapshot reserved offset");
_Static_assert(offsetof(McObjectiveSnapshotV2, stage_selectors) == 13, "objective stages offset");
_Static_assert(offsetof(McObjectiveSnapshotV2, reserved) == 24, "objective reserved offset");
_Static_assert(offsetof(McCampaignObjectiveStageSnapshotV2, axis_x) == 24, "campaign objective stage axis offset");
_Static_assert(offsetof(McCampaignObjectiveStageSnapshotV2, relation_value) == 30, "campaign objective stage relation offset");
_Static_assert(offsetof(McCampaignObjectiveStageSnapshotV2, reserved) == 32, "campaign objective stage reserved offset");
_Static_assert(offsetof(McEntitySnapshotV2, reserved) == 36, "entity reserved offset");
_Static_assert(offsetof(McCampaignSelectionV2, reserved) == 16, "campaign selection reserved offset");
_Static_assert(offsetof(McCampaignWorldInfoV2, reset_state_hash) == 40, "campaign world hash offset");
_Static_assert(offsetof(McCampaignTerrainStateV2, mutation_sequence) == 24, "campaign terrain mutation offset");
_Static_assert(offsetof(McCampaignTerrainStateV2, width) == 40, "campaign terrain dimensions offset");
_Static_assert(offsetof(McCampaignTerrainStateV2, reserved) == 48, "campaign terrain reserved offset");
_Static_assert(offsetof(McCampaignPlayerSnapshotV2, turn) == 16, "campaign player turn offset");
_Static_assert(offsetof(McCampaignPlayerSnapshotV2, reserved) == 32, "campaign player reserved offset");
_Static_assert(offsetof(McCampaignEntitySnapshotV2, reserved) == 36, "campaign entity reserved offset");
_Static_assert(offsetof(McCampaignShieldSnapshotV2, parent_status_flags) == 48, "campaign Shield flags offset");
_Static_assert(offsetof(McCampaignInvisibleSnapshotV2, parent_status_flags) == 52, "campaign Invisible flags offset");
_Static_assert(offsetof(McGoatSnapshotV2, life) == 48, "Goat life offset");
_Static_assert(offsetof(McGoatSnapshotV2, reserved) == 68, "Goat reserved offset");
_Static_assert(offsetof(McCampaignGoatSnapshotV2, partner_entity_index) == 68, "campaign Goat partner offset");
_Static_assert(offsetof(McCampaignSpellPickupSnapshotV2, source_entity_index) == 8, "campaign spell pickup source offset");
_Static_assert(offsetof(McCampaignSpellPickupSnapshotV2, x) == 16, "campaign spell pickup position offset");
_Static_assert(offsetof(McGoatBatchV2, reserved) == 48, "Goat batch reserved offset");
_Static_assert(offsetof(McLevel1MissionSnapshotV2, player_life) == 40, "Level 1 mission life offset");
_Static_assert(offsetof(McLevel1PlayerLifecycleSnapshotV2, player_action) == 41, "player lifecycle action offset");
_Static_assert(offsetof(McLevel1PlayerLifecycleSnapshotV2, action_timer) == 56, "player lifecycle timer offset");
_Static_assert(offsetof(McLevel1PlayerLifecycleSnapshotV2, player_control_flags) == 72, "player lifecycle flags offset");
_Static_assert(offsetof(McLevel1PlayerLifecycleSnapshotV2, source_pool_rebuild_count) == 108, "player lifecycle rebuild offset");
_Static_assert(offsetof(McLevel1PlayerLifecycleSnapshotV2, marker_dispatch_phase) == 122, "player lifecycle marker dispatch offset");
_Static_assert(offsetof(McLevel1PlayerLifecycleSnapshotV2, source_turn_counter) == 128, "player lifecycle source turn offset");
_Static_assert(offsetof(McLevel1PlayerLifecycleSnapshotV2, effective_pitch) == 134, "player lifecycle effective pitch offset");
_Static_assert(offsetof(McLevel1PlayerLifecycleSnapshotV2, marker_constructor_phase) == 136, "player lifecycle marker constructor offset");
_Static_assert(offsetof(McLevel1MissionSnapshotV2, actor_count) == 64, "Level 1 mission actor count offset");
_Static_assert(offsetof(McLevel1MissionSnapshotV2, exit_sequence_phase) == 76, "Level 1 mission exit phase offset");
_Static_assert(offsetof(McLevel1MissionSnapshotV2, exit_sequence_timer) == 80, "Level 1 mission exit timer offset");
_Static_assert(offsetof(McLevel1MissionSnapshotV2, possession_experience) == 84, "Level 1 possession experience offset");
_Static_assert(offsetof(McLevel1MissionSnapshotV2, last_possessed_building_entity) == 88, "Level 1 last possessed building offset");
_Static_assert(offsetof(McLevel1MissionSnapshotV2, possessed_building_count) == 92, "Level 1 possessed building count offset");
_Static_assert(offsetof(McLevel1MissionSnapshotV2, speed_trail_count) == 95, "Level 1 speed trail count offset");
_Static_assert(offsetof(McLevel1HudSpellSlotV2, max_mana) == 12, "Level 1 HUD spell max mana offset");
_Static_assert(offsetof(McLevel1HudSnapshotV2, global_mana_denominator) == 32, "Level 1 HUD denominator offset");
_Static_assert(offsetof(McLevel1HudSnapshotV2, left) == 48, "Level 1 HUD left offset");
_Static_assert(offsetof(McLevel1HudSnapshotV2, right) == 84, "Level 1 HUD right offset");
_Static_assert(offsetof(McLevel1HudSnapshotV2, selected_holder_entity_index) == 120, "Level 1 HUD selected holder offset");
_Static_assert(offsetof(McLevel1SpellMenuSnapshotV2, family_enabled) == 32, "Level 1 spell menu families offset");
_Static_assert(offsetof(McLevel1SpellMenuSnapshotV2, holder_entity_index) == 48, "Level 1 spell menu holders offset");
_Static_assert(offsetof(McLevel1MinimapMarksV2, mark_x) == 28, "Level 1 minimap mark X offset");
_Static_assert(offsetof(McLevel1MinimapMarksV2, arrow_xy) == 124, "Level 1 minimap arrow offset");
_Static_assert(offsetof(McLevel1MinimapMarksV2, reserved) == 152, "Level 1 minimap reserved offset");
_Static_assert(offsetof(McLevel1MinimapEntityCommandV2, reserved) == 16, "Level 1 minimap entity command reserved offset");
_Static_assert(offsetof(McLevel1MinimapEntitiesInfoV2, command_hash) == 40, "Level 1 minimap entities hash offset");
_Static_assert(offsetof(McLevel1MinimapEntitiesInfoV2, reserved) == 48, "Level 1 minimap entities reserved offset");
_Static_assert(offsetof(McLevel1PaletteInfoV2, rgb8_sha256) == 32, "Level 1 palette hash offset");
_Static_assert(offsetof(McLevel1PaletteInfoV2, reserved) == 64, "Level 1 palette reserved offset");
_Static_assert(offsetof(McLevel1ExitPresentationV2, player_action) == 25, "Level 1 exit action offset");
_Static_assert(offsetof(McLevel1ExitPresentationV2, camera_fov) == 36, "Level 1 exit FOV offset");
_Static_assert(offsetof(McLevel1ExitPresentationV2, reserved) == 48, "Level 1 exit reserved offset");
_Static_assert(offsetof(McLevel1CastleSnapshotV2, life) == 48, "Level 1 castle life offset");
_Static_assert(offsetof(McLevel1CastleSnapshotV2, reserved) == 68, "Level 1 castle reserved offset");
_Static_assert(offsetof(McTerrainStateV2, mutation_sequence) == 24, "terrain state mutation offset");
_Static_assert(offsetof(McTerrainStateV2, width) == 40, "terrain state dimensions offset");
_Static_assert(offsetof(McTerrainStateV2, reserved) == 48, "terrain state reserved offset");
_Static_assert(offsetof(McLevel1CastleLifecycleSnapshotV2, root_life) == 44, "castle lifecycle life offset");
_Static_assert(offsetof(McLevel1CastleLifecycleSnapshotV2, construction_timer) == 64, "castle lifecycle timer offset");
_Static_assert(offsetof(McLevel1CastleLifecycleSnapshotV2, terrain_mutation_sequence) == 72, "castle lifecycle terrain sequence offset");
_Static_assert(offsetof(McLevel1HouseCompletionEffectV2, parent_word_2e) == 44, "house completion parent state offset");
_Static_assert(offsetof(McLevel1HouseCompletionEffectV2, max_life) == 64, "house completion life offset");
_Static_assert(offsetof(McLevel1HouseCompletionEffectV2, effect_count) == 76, "house completion count offset");
_Static_assert(offsetof(McManaSphereSnapshotV2, mana) == 44, "mana sphere mana offset");
_Static_assert(offsetof(McManaSphereSnapshotV2, reserved) == 52, "mana sphere reserved offset");
_Static_assert(offsetof(McManaCarrierSnapshotV2, carried_mana) == 60, "mana carrier payload offset");
_Static_assert(offsetof(McManaCarrierSnapshotV2, reserved) == 64, "mana carrier reserved offset");
_Static_assert(offsetof(McLevel1ActorSnapshotV2, max_life) == 28, "Level 1 actor life offset");
_Static_assert(offsetof(McLevel1ActorSnapshotV2, base_tmap) == 40, "Level 1 actor base TMAP offset");
_Static_assert(offsetof(McLevel1ActorSnapshotV2, yaw) == 44, "Level 1 actor yaw offset");
_Static_assert(offsetof(McLevel1ActorSnapshotV2, state_value) == 48, "Level 1 actor state value offset");
_Static_assert(offsetof(McLevel1ActorSnapshotV2, reserved) == 52, "Level 1 actor reserved offset");
_Static_assert(offsetof(McLevel1MultipartSegmentSnapshotV2, x) == 26, "multipart segment x offset");
_Static_assert(offsetof(McLevel1MultipartSegmentSnapshotV2, life) == 40, "multipart segment life offset");
_Static_assert(offsetof(McLevel1MultipartSegmentSnapshotV2, base_tmap) == 50, "multipart segment TMAP offset");
_Static_assert(offsetof(McLevel1MultipartSegmentSnapshotV2, reserved) == 60, "multipart segment reserved offset");
_Static_assert(offsetof(McFireballProjectileSnapshotV2, payload) == 36, "Fireball projectile payload offset");
_Static_assert(offsetof(McFireballProjectileSnapshotV2, logical_height) == 40, "Fireball projectile height offset");
_Static_assert(offsetof(McFireballProjectileSnapshotV2, reserved) == 48, "Fireball projectile reserved offset");
_Static_assert(offsetof(McFireballImpactSnapshotV2, payload) == 32, "Fireball impact payload offset");
_Static_assert(offsetof(McFireballImpactSnapshotV2, base_tmap) == 36, "Fireball impact base TMAP offset");
_Static_assert(offsetof(McFireballImpactSnapshotV2, reserved) == 44, "Fireball impact reserved offset");
_Static_assert(offsetof(McSelectorFrameStateV2, pixel_hash) == 40, "selector frame pixel hash offset");
_Static_assert(offsetof(McSelectorFrameStateV2, reserved) == 48, "selector frame reserved offset");
_Static_assert(offsetof(McSelectorFrameBatchV2, start_ordinal) == 56, "selector batch start offset");
_Static_assert(offsetof(McSelectorFrameBatchV2, reserved) == 72, "selector batch reserved offset");
_Static_assert(offsetof(McSelectorDrawFeedbackV2, reserved) == 32, "selector feedback reserved offset");
_Static_assert(offsetof(McGoatDamageCommandV2, reserved) == 24, "Goat damage reserved offset");
_Static_assert(offsetof(McEventV2, state_hash) == 24, "event hash offset");
_Static_assert(offsetof(McEventV2, reserved) == 56, "event reserved offset");
_Static_assert(offsetof(McAudioCommandV2, playback_rate_hz) == 48, "audio rate offset");
_Static_assert(offsetof(McAudioCommandV2, sound_id) == 52, "audio sound offset");
_Static_assert(offsetof(McAudioCommandV2, volume_q15) == 60, "audio volume offset");
_Static_assert(offsetof(McAudioCommandV2, play_type) == 68, "audio type offset");
_Static_assert(offsetof(McAudioCommandV2, reserved) == 72, "audio reserved offset");
_Static_assert(offsetof(McPresentationStateV2, source_turn) == 24, "presentation turn offset");
_Static_assert(offsetof(McPresentationStateV2, camera_x) == 28, "presentation camera offset");
_Static_assert(offsetof(McPresentationStateV2, camera_authority) == 42, "presentation authority offset");
_Static_assert(offsetof(McPresentationStateV2, reserved) == 48, "presentation reserved offset");
_Static_assert(offsetof(McRenderCameraStateV2, history_x) == 28, "render camera history offset");
_Static_assert(offsetof(McRenderCameraStateV2, slope_target_x) == 42, "render camera slope target offset");
_Static_assert(offsetof(McRenderCameraStateV2, render_offset_x) == 46, "render camera offset");
_Static_assert(offsetof(McRenderCameraStateV2, reserved) == 56, "render camera reserved offset");
_Static_assert(offsetof(McTerrainPnt5VectorV2, pnt5) == 36, "terrain pnt5 offset");
_Static_assert(offsetof(McTerrainGridVertexV2, view_x) == 16, "terrain grid view offset");
_Static_assert(offsetof(McTerrainGridVertexV2, feature_flags) == 44, "terrain grid flags offset");
_Static_assert(offsetof(McTerrainGridVertexV2, reserved) == 48, "terrain grid reserved offset");
_Static_assert(offsetof(McTerrainFrameQueryV2, reserved) == 24, "terrain frame query reserved offset");
_Static_assert(offsetof(McTerrainFrameInfoV2, width) == 24, "terrain frame width offset");
_Static_assert(offsetof(McTerrainFrameInfoV2, index_bytes) == 40, "terrain frame bytes offset");
_Static_assert(offsetof(McTerrainFrameInfoV2, index_hash) == 64, "terrain frame hash offset");
_Static_assert(offsetof(McTerrainFrameInfoV2, reserved) == 88, "terrain frame reserved offset");
_Static_assert(offsetof(McPresentationFrameQueryV2, configured_blur_mode) == 40, "presentation frame blur offset");
_Static_assert(offsetof(McLevel1PresentationInputV2, mouse_x) == 38, "presentation input mouse offset");
_Static_assert(offsetof(McLevel1PresentationInputV2, debug_clock_bucket) == 704, "presentation input debug clock offset");
_Static_assert(offsetof(McPresentationFrameInfoV2, immutable_input_hash) == 32, "presentation frame input hash offset");
_Static_assert(offsetof(McPresentationFrameInfoV2, index_hash) == 64, "presentation frame index hash offset");
_Static_assert(offsetof(McPresentationFrameInfoV2, reserved) == 104, "presentation frame reserved offset");
"#,
    )
    .unwrap();
    let include = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("include");
    let output = std::process::Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-I"])
        .arg(include)
        .arg("-c")
        .arg(&source)
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    let _ = fs::remove_dir_all(&directory);
    assert!(
        output.status.success(),
        "C header layout probe failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn terrain_pnt5_qualification_vectors_cross_the_typed_abi_exactly() {
    assert_eq!(
        mc_native_terrain_pnt5_vector_count(),
        TERRAIN_PNT5_QUALIFICATION_VECTOR_COUNT
    );
    let inputs = [
        (0, 4, 4, 0, 8, 128, 0),
        (0, 12, 4, 40, 8, 128, 0),
        (1, 0, 0, 40, 8, 128, 0),
        (1, 1, 1, 40, 8, 128, 14_745_600),
        (1, 2, 3, 40, 8, 128, 14_745_601),
        (1, 5, 7, 40, 8, 128, 19_202_048),
        (1, 255, 255, 40, 8, 128, 23_658_495),
        (1, 255, 255, 40, 8, 128, 23_658_496),
        (1, 255, 255, 40, 8, 128, 26_214_399),
        (1, 255, 255, 40, 8, 128, 26_214_400),
        (1, 42, 99, 56, 8, 128, 0),
        (65_535, 256, -1, 55, 8, 128, 14_757_945),
        (1, 0, 0, 40, 8, -256, 0),
    ];
    for (index, (turn, tile_x, tile_y, shade, angle, view_y, distance_squared)) in
        inputs.into_iter().enumerate()
    {
        let expected =
            outdoor_terrain_pnt5(turn, tile_x, tile_y, shade, angle, view_y, distance_squared);
        let mut vector = McTerrainPnt5VectorV2::default();
        assert_eq!(
            mc_native_terrain_pnt5_vector(
                u32::try_from(index).unwrap(),
                &raw mut vector,
                struct_size::<McTerrainPnt5VectorV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(vector.abi_version, MC_NATIVE_ABI_VERSION);
        assert_eq!(
            (
                vector.struct_size,
                vector.turn,
                vector.tile_x,
                vector.tile_y,
                vector.shade,
                vector.angle,
                vector.view_y,
                vector.distance_squared,
            ),
            (
                56,
                turn,
                tile_x,
                tile_y,
                shade,
                angle,
                view_y,
                distance_squared,
            )
        );
        assert_eq!(vector.visible, u8::from(expected.visible));
        assert_eq!(vector.tablesn_row, expected.tablesn_row);
        assert_eq!(vector.wave, expected.wave);
        assert_eq!(vector.land_displacement, expected.land_displacement);
        assert_eq!(vector.pnt5, expected.pnt5);
        assert_eq!(vector.lighting_wave, expected.lighting_wave);
        assert_eq!(vector.reserved, [0; 2]);
    }
    let mut out_of_bounds = McTerrainPnt5VectorV2::default();
    assert_eq!(
        mc_native_terrain_pnt5_vector(
            TERRAIN_PNT5_QUALIFICATION_VECTOR_COUNT,
            &raw mut out_of_bounds,
            struct_size::<McTerrainPnt5VectorV2>(),
        ),
        MC_STATUS_BOUNDS
    );
}

#[test]
fn owner_level_one_terrain_grid_crosses_the_bulk_abi_exactly() {
    let handle = open_fixture();
    assert_eq!(mc_native_terrain_grid_vertex_count(), 840);

    let mut too_small = vec![McTerrainGridVertexV2::default(); 839];
    let mut written = 99;
    assert_eq!(
        mc_native_session_terrain_grid_copy(
            handle,
            too_small.as_mut_ptr(),
            839,
            struct_size::<McTerrainGridVertexV2>(),
            &raw mut written,
        ),
        MC_STATUS_BUFFER_TOO_SMALL
    );
    assert_eq!(written, 0);

    let mut vertices = vec![McTerrainGridVertexV2::default(); 840];
    assert_eq!(
        mc_native_session_terrain_grid_copy(
            handle,
            vertices.as_mut_ptr(),
            840,
            struct_size::<McTerrainGridVertexV2>(),
            &raw mut written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(written, 840);
    let direct = with_session(handle, |session| Ok(session.terrain_grid())).unwrap();
    assert_eq!(vertices.as_slice(), direct.as_slice());
    for (index, vertex) in vertices.iter().enumerate() {
        assert_eq!(vertex.abi_version, MC_NATIVE_ABI_VERSION);
        assert_eq!(vertex.struct_size, 64);
        assert_eq!(usize::from(vertex.index), index);
        assert_eq!(usize::from(vertex.row), index / 40);
        assert_eq!(usize::from(vertex.column), index % 40);
        assert_eq!(vertex.pnt5_written, u8::from(vertex.feature_flags & 2 == 0));
        if vertex.pnt5_written != 0 {
            assert_eq!(vertex.tablesn_row, ((vertex.pnt5 >> 16) & 0xff) as u8);
        }
        assert_eq!(vertex.reserved, [0; 4]);
    }
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn isolated_terrain_frame_crosses_generation_tick_checked_bulk_abi() {
    let handle = open_fixture_with_terrain_frame_assets();
    let mut camera = McRenderCameraStateV2::default();
    assert_eq!(
        mc_native_render_camera_state(
            handle,
            &raw mut camera,
            struct_size::<McRenderCameraStateV2>(),
        ),
        MC_STATUS_OK
    );
    let query = McTerrainFrameQueryV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McTerrainFrameQueryV2>(),
        generation: camera.generation,
        tick: camera.tick,
        reserved: [0; 4],
    };
    let mut indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
    let mut coverage = vec![0_u8; LEVEL1_VIEW_PIXELS];
    let mut info = McTerrainFrameInfoV2::default();
    assert_eq!(
        mc_native_session_terrain_frame_copy(
            handle,
            &raw const query,
            indices.as_mut_ptr(),
            indices.len() as u32,
            coverage.as_mut_ptr(),
            coverage.len() as u32,
            &raw mut info,
            struct_size::<McTerrainFrameInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (info.width, info.height, info.stride),
        (
            LEVEL1_VIEW_WIDTH as u16,
            LEVEL1_VIEW_HEIGHT as u16,
            LEVEL1_VIEW_WIDTH as u16,
        )
    );
    assert_eq!(
        (info.index_bytes, info.coverage_bytes),
        (LEVEL1_VIEW_PIXELS as u32, LEVEL1_VIEW_PIXELS as u32)
    );
    assert_eq!(
        info.flags,
        MC_TERRAIN_FRAME_ISOLATED_SKY_TERRAIN
            | MC_TERRAIN_FRAME_SKY_SEEDED
            | MC_TERRAIN_FRAME_HAS_COVERAGE
            | MC_TERRAIN_FRAME_NON_TERRAIN_OMITTED
    );
    assert_eq!(
        sim_core::terrain_frame::byte_hash(&indices),
        info.index_hash
    );
    assert_eq!(
        sim_core::terrain_frame::byte_hash(&coverage),
        info.coverage_hash
    );
    assert!(coverage.contains(&255));

    let first_indices = indices.clone();
    let first_coverage = coverage.clone();
    let mut repeated = McTerrainFrameInfoV2::default();
    assert_eq!(
        mc_native_session_terrain_frame_copy(
            handle,
            &raw const query,
            indices.as_mut_ptr(),
            indices.len() as u32,
            coverage.as_mut_ptr(),
            coverage.len() as u32,
            &raw mut repeated,
            struct_size::<McTerrainFrameInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(info, repeated);
    assert_eq!(indices, first_indices);
    assert_eq!(coverage, first_coverage);

    assert_eq!(
        mc_native_session_terrain_frame_copy(
            handle,
            &raw const query,
            indices.as_mut_ptr(),
            (LEVEL1_VIEW_PIXELS - 1) as u32,
            coverage.as_mut_ptr(),
            coverage.len() as u32,
            &raw mut repeated,
            struct_size::<McTerrainFrameInfoV2>(),
        ),
        MC_STATUS_BUFFER_TOO_SMALL
    );
    let stale_tick = McTerrainFrameQueryV2 {
        tick: query.tick.wrapping_add(1),
        ..query
    };
    assert_eq!(
        mc_native_session_terrain_frame_copy(
            handle,
            &raw const stale_tick,
            indices.as_mut_ptr(),
            indices.len() as u32,
            coverage.as_mut_ptr(),
            coverage.len() as u32,
            &raw mut repeated,
            struct_size::<McTerrainFrameInfoV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn reset_world_chain_contains_all_frozen_actors_in_source_relative_order() {
    let handle = open_fixture_with_world_frame_assets();
    with_session(handle, |session| {
        let frozen = LEVEL1_FROZEN_ACTOR_ENTITY_INDICES
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(session.frozen_actors.len(), frozen.len());
        assert!(session.frozen_actors.iter().all(|actor| {
            frozen.contains(&actor.source_entity_index)
                && session
                    .renderable_map_chains
                    .tracks(actor.source_entity_index)
        }));

        let supported = session
            .world
            .reset_live_world()
            .goats()
            .iter()
            .map(|goat| goat.entity_index)
            .chain(frozen.iter().copied())
            .collect::<std::collections::BTreeSet<_>>();
        let planes = session.world.terrain_planes();
        let mut seen_frozen = std::collections::BTreeSet::new();
        for (tile, raw_head) in planes.entity_indices.iter().copied().enumerate() {
            if raw_head <= 0 {
                continue;
            }
            let mut expected = Vec::new();
            let mut source_entity = raw_head.cast_unsigned();
            while source_entity != 0 {
                if supported.contains(&source_entity) {
                    expected.push(source_entity);
                    if frozen.contains(&source_entity) {
                        seen_frozen.insert(source_entity);
                    }
                }
                source_entity =
                    session.world.entities()[usize::from(source_entity)].old_map_entity();
            }
            if expected.is_empty() {
                continue;
            }
            let tile_x = u8::try_from(tile & 0xff).expect("tile X");
            let tile_y = u8::try_from(tile >> 8).expect("tile Y");
            assert_eq!(
                session.renderable_map_chains.entity_chain(tile_x, tile_y),
                expected,
                "filtered source-relative order changed at tile {tile:#06x}"
            );
        }
        assert_eq!(seen_frozen, frozen);
        Ok(())
    })
    .expect("qualified reset renderable chain");
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn reset_restores_live_villager_draw_records_before_rebuilding_map_chains() {
    let handle = open_fixture_with_world_frame_assets();
    with_session_mut(handle, |session| {
        let reset_actors = level1_frozen_actor_renders(session.world.as_ref())?;
        for _ in 0..16 {
            session.step(PlayerCommand::default());
        }
        assert!(
            session
                .frozen_actors
                .iter()
                .zip(reset_actors.iter())
                .any(|(live, reset)| {
                    live.source_entity_index < 130 && live.position != reset.position
                }),
            "at least one live Villager moved before reset"
        );

        session.reset();

        assert_eq!(session.frozen_actors, reset_actors);
        let expected = reset_renderable_map_chains(session.world.as_ref(), reset_actors.as_ref())?;
        assert_eq!(session.renderable_map_chains, expected);
        Ok(())
    })
    .expect("stepped live Villager reset");
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn source_ordered_world_frame_exposes_supported_families_without_changing_terrain_contract() {
    let handle = open_fixture_with_world_frame_assets();
    let native_frame = with_session_mut(handle, |session| {
        assert_eq!(session.frozen_actors.len(), 31);
        assert!(session.frozen_actors.iter().all(|actor| {
            session
                .renderable_map_chains
                .tracks(actor.source_entity_index)
        }));
        assert_eq!(session.mission.ambient_entities().count(), 40);
        let reset_ambient = session
            .mission
            .ambient_entities()
            .copied()
            .collect::<Vec<_>>();
        let render_ambient_ablation =
            |ambient_entities: &[Level1AmbientEntity]| -> AbiResult<Level1TerrainFrame> {
                let mut map_chains = session.renderable_map_chains.clone();
                for ambient in &reset_ambient {
                    if ambient_entities
                        .iter()
                        .any(|selected| selected.source_entity_index == ambient.source_entity_index)
                    {
                        continue;
                    }
                    map_chains
                        .sync_entity(RenderableMapSeed {
                            entity_index: ambient.source_entity_index,
                            active: false,
                            x: ambient.position.x,
                            y: ambient.position.y,
                        })
                        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
                }
                session.render_world_frame(ambient_entities, &map_chains)
            };
        let reset_full = render_ambient_ablation(&reset_ambient)?;
        let reset_without_ambient = render_ambient_ablation(&[])?;
        assert_eq!(reset_full.index_hash, 0xaca8_5f81_ab89_d6ef);
        assert_eq!(
            reset_full.terrain_coverage,
            reset_without_ambient.terrain_coverage
        );
        assert_eq!(
            reset_full.coverage_hash,
            reset_without_ambient.coverage_hash
        );
        assert_eq!(reset_full.stats, reset_without_ambient.stats);

        let changed_summary = |frame: &Level1TerrainFrame,
                               baseline: &Level1TerrainFrame|
         -> (usize, Option<(usize, usize, usize, usize)>) {
            assert_eq!(frame.terrain_coverage, baseline.terrain_coverage);
            assert_eq!(frame.coverage_hash, baseline.coverage_hash);
            assert_eq!(frame.stats, baseline.stats);
            assert_eq!(frame.indices.len(), baseline.indices.len());
            let mut count = 0;
            let mut unchanged_count = 0;
            let mut bounds: Option<(usize, usize, usize, usize)> = None;
            for (index, (&actual, &without_ambient)) in frame
                .indices
                .iter()
                .zip(baseline.indices.iter())
                .enumerate()
            {
                if actual == without_ambient {
                    unchanged_count += 1;
                    continue;
                }
                let x = index % LEVEL1_VIEW_WIDTH;
                let y = index / LEVEL1_VIEW_WIDTH;
                count += 1;
                bounds = Some(bounds.map_or((x, y, x, y), |(min_x, min_y, max_x, max_y)| {
                    (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
                }));
            }
            assert_eq!(count + unchanged_count, frame.indices.len());
            (count, bounds)
        };

        assert_eq!(reset_without_ambient.index_hash, 0x2f38_beef_8e4a_62fb);
        assert_eq!(
            reset_without_ambient
                .ambient_raster_stats
                .framebuffer_writes,
            0
        );
        assert_eq!(reset_full.ambient_raster_stats.framebuffer_writes, 16_000);
        assert_eq!(
            changed_summary(&reset_full, &reset_without_ambient),
            (10_989, Some((0, 114, 252, 199)))
        );

        for (model, row_count, index_hash, framebuffer_writes, delta) in [
            (
                1,
                8,
                0xfba7_2c67_ec5b_9bac,
                10_627,
                (8_894, Some((0, 128, 242, 199))),
            ),
            (
                14,
                9,
                0x734e_f184_f4c0_cdaa,
                259,
                (256, Some((66, 114, 211, 168))),
            ),
            (
                0,
                14,
                0x4831_3b7f_97f3_30ed,
                5_114,
                (4_539, Some((25, 134, 252, 199))),
            ),
        ] {
            let family = reset_ambient
                .iter()
                .copied()
                .filter(|ambient| ambient.model == model)
                .collect::<Vec<_>>();
            let frame = render_ambient_ablation(&family)?;
            assert_eq!(
                (
                    family.len(),
                    frame.index_hash,
                    frame.ambient_raster_stats.framebuffer_writes,
                    changed_summary(&frame, &reset_without_ambient),
                ),
                (row_count, index_hash, framebuffer_writes, delta),
                "reset ambient family model {model}"
            );
        }
        let emitter_113 = reset_ambient
            .iter()
            .copied()
            .filter(|ambient| ambient.source_entity_index == 113)
            .collect::<Vec<_>>();
        assert_eq!(emitter_113.len(), 1);
        assert!(!session.renderable_map_chains.tracks(113));
        let emitter_frame = render_ambient_ablation(&emitter_113)?;
        assert_eq!(
            (
                emitter_frame.index_hash,
                emitter_frame.ambient_raster_stats.framebuffer_writes,
                changed_summary(&emitter_frame, &reset_without_ambient),
            ),
            (0x2f38_beef_8e4a_62fb, 0, (0, None))
        );
        let fire = session
            .mission
            .ambient_entities()
            .find(|ambient| ambient.source_entity_index == 122)
            .copied()
            .expect("frame-zero village fire");
        let child_before = session
            .mission
            .ambient_entities()
            .find(|ambient| ambient.source_entity_index == 164)
            .copied()
            .expect("frame-zero village fire child");
        session.camera_slope_sample = TerrainSlopeCameraSample::default();
        let mut ambient_before = None;
        let mut ambient_camera = None;
        for yaw in (0_i16..2_048_i16).step_by(256) {
            let camera = CameraHistory {
                tick: session.state.tick,
                source_turn: session.source_turn,
                x: fire.position.x.wrapping_sub(768),
                y: fire.position.y,
                z: fire.position.z.wrapping_add(128),
                yaw,
                pitch: 0,
                roll: 0,
                fov: 128,
            };
            session.camera_history = camera;
            let frame = session.world_frame()?;
            if frame.ambient_raster_stats.framebuffer_writes > 0 {
                ambient_camera = Some(camera);
                ambient_before = Some(frame);
                break;
            }
        }
        let ambient_before = ambient_before.ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "no cardinal test view rasterized village fire entity 122",
            )
        })?;
        assert!(ambient_before.ambient_entities_composited);
        assert!(ambient_before.ambient_raster_stats.framebuffer_writes > 0);
        session.step(PlayerCommand::default());
        session.camera_history = ambient_camera.expect("visible ambient camera");
        let ambient_after = session.world_frame()?;
        assert_ne!(
            ambient_before.index_hash, ambient_after.index_hash,
            "ambient action/reclamation must change production render pixels"
        );
        assert!(
            session
                .mission
                .ambient_entities()
                .all(|entity| entity.model != 14)
        );
        assert!(
            session
                .mission
                .ambient_entities()
                .find(|entity| entity.source_entity_index == 164)
                .is_some_and(|entity| {
                    entity.life == 6
                        && entity.animation_frame == 2
                        && entity.position != child_before.position
                })
        );
        session.reset();
        assert_eq!(session.mission.ambient_entities().count(), 40);
        let actor = session.frozen_actors[0];
        session.camera_slope_sample = TerrainSlopeCameraSample::default();
        let mut visible = None;
        for yaw in (0_i16..2_048_i16).step_by(256) {
            session.camera_history = CameraHistory {
                tick: session.state.tick,
                source_turn: session.source_turn,
                x: actor.position.x.wrapping_sub(1_024),
                y: actor.position.y,
                z: actor.position.z.wrapping_add(256),
                yaw,
                pitch: 0,
                roll: 0,
                fov: 128,
            };
            let frame = session.world_frame()?;
            if frame.frozen_actor_raster_stats.framebuffer_writes > 0
                && frame.frozen_actor_selector_mask != 0
            {
                visible = Some(frame);
                break;
            }
        }
        visible.ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "no cardinal test view rasterized frozen villager entity 51",
            )
        })
    })
    .expect("native frame");
    assert!(native_frame.frozen_actors_composited);
    assert!(
        native_frame.frozen_actor_raster_stats.framebuffer_writes > 0,
        "the reset view must raster at least one of the 31 frozen actors"
    );
    assert_ne!(
        native_frame.frozen_actor_selector_mask, 0,
        "the reset view must latch at least one villager facing"
    );
    assert_eq!(native_frame.index_hash, 0x3a86_3b45_c134_1d6c);
    assert_eq!(
        native_frame.frozen_actor_raster_stats,
        sim_core::goat_sprite_raster::Mode20SpriteRasterStats {
            candidate_pixels: 6_439,
            transparent_pixels: 1_277,
            clipped_pixels: 3_575,
            framebuffer_writes: 960,
        }
    );
    assert_eq!(
        native_frame.reflected_sprite_raster_stats,
        sim_core::goat_sprite_raster::Mode20SpriteRasterStats {
            candidate_pixels: 6_439,
            transparent_pixels: 1_172,
            clipped_pixels: 3_750,
            framebuffer_writes: 890,
        }
    );
    assert_eq!(
        native_frame.shadow_sprite_raster_stats,
        sim_core::goat_sprite_raster::Mode20SpriteRasterStats {
            candidate_pixels: 1_519,
            transparent_pixels: 287,
            clipped_pixels: 767,
            framebuffer_writes: 186,
        }
    );
    let camera = with_session(handle, |session| Ok(session.render_camera_state())).expect("camera");
    let query = McTerrainFrameQueryV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McTerrainFrameQueryV2>(),
        generation: camera.generation,
        tick: camera.tick,
        reserved: [0; 4],
    };
    let mut terrain_indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
    let mut terrain_coverage = vec![0_u8; LEVEL1_VIEW_PIXELS];
    let mut terrain_info = McTerrainFrameInfoV2::default();
    assert_eq!(
        mc_native_session_terrain_frame_copy(
            handle,
            &raw const query,
            terrain_indices.as_mut_ptr(),
            terrain_indices.len() as u32,
            terrain_coverage.as_mut_ptr(),
            terrain_coverage.len() as u32,
            &raw mut terrain_info,
            struct_size::<McTerrainFrameInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(terrain_info.flags & MC_TERRAIN_FRAME_GOATS_COMPOSITED, 0);
    assert_eq!(terrain_info.flags & MC_TERRAIN_FRAME_ARCHERS_COMPOSITED, 0);
    assert_eq!(
        terrain_info.flags & MC_TERRAIN_FRAME_FIREFLIES_COMPOSITED,
        0
    );
    assert_eq!(terrain_info.flags & MC_TERRAIN_FRAME_COMBAT_COMPOSITED, 0);
    assert_eq!(
        terrain_info.flags & MC_TERRAIN_FRAME_OBJECTIVE_PROPS_COMPOSITED,
        0
    );
    assert_eq!(terrain_info.flags & MC_TERRAIN_FRAME_ECONOMY_COMPOSITED, 0);
    assert_eq!(
        terrain_info.flags & MC_TERRAIN_FRAME_REFLECTED_SPRITES_COMPOSITED,
        0
    );
    assert_eq!(
        terrain_info.flags & MC_TERRAIN_FRAME_SPRITE_SHADOWS_COMPOSITED,
        0
    );
    assert_eq!(
        terrain_info.flags & MC_TERRAIN_FRAME_FROZEN_ACTORS_COMPOSITED,
        0
    );

    let feedback_before_world_read =
        with_session(handle, |session| Ok(session.selector_draw_feedback)).unwrap();
    let selector_frames_before_world_read = with_session(handle, |session| {
        Ok((
            session.selector_frames.clone(),
            session.archer_selector_frames.clone(),
            session.firefly_selector_frames.clone(),
            session.villager_selector_frames.clone(),
            session.fireball_selector_frame.clone(),
            session.possession_selector_frame.clone(),
            session.castle_selector_frame.clone(),
        ))
    })
    .unwrap();
    let mut world_indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
    let mut world_coverage = vec![0_u8; LEVEL1_VIEW_PIXELS];
    let mut world_info = McTerrainFrameInfoV2::default();
    let world_status = mc_native_session_world_frame_copy(
        handle,
        &raw const query,
        world_indices.as_mut_ptr(),
        world_indices.len() as u32,
        world_coverage.as_mut_ptr(),
        world_coverage.len() as u32,
        &raw mut world_info,
        struct_size::<McTerrainFrameInfoV2>(),
    );
    assert_eq!(
        world_status,
        MC_STATUS_OK,
        "{}",
        LAST_ERROR.with(|error| error.borrow().clone())
    );
    assert_ne!(world_info.flags & MC_TERRAIN_FRAME_GOATS_COMPOSITED, 0);
    assert_ne!(world_info.flags & MC_TERRAIN_FRAME_ARCHERS_COMPOSITED, 0);
    assert_ne!(world_info.flags & MC_TERRAIN_FRAME_FIREFLIES_COMPOSITED, 0);
    assert_ne!(world_info.flags & MC_TERRAIN_FRAME_COMBAT_COMPOSITED, 0);
    assert_ne!(
        world_info.flags & MC_TERRAIN_FRAME_OBJECTIVE_PROPS_COMPOSITED,
        0
    );
    assert_ne!(world_info.flags & MC_TERRAIN_FRAME_ECONOMY_COMPOSITED, 0);
    assert_eq!(
        world_info.flags & MC_TERRAIN_FRAME_REFLECTED_SPRITES_COMPOSITED != 0,
        camera.eye_z < sim_core::terrain_reflection::REFLECTION_EYE_Z_LIMIT as i16
    );
    assert_ne!(
        world_info.flags & MC_TERRAIN_FRAME_SPRITE_SHADOWS_COMPOSITED,
        0
    );
    assert_ne!(
        world_info.flags & MC_TERRAIN_FRAME_FROZEN_ACTORS_COMPOSITED,
        0
    );
    assert_eq!(world_coverage, terrain_coverage);
    assert_eq!(
        world_info.index_hash,
        sim_core::terrain_frame::byte_hash(&world_indices)
    );
    assert_eq!(
        world_info.coverage_hash,
        sim_core::terrain_frame::byte_hash(&world_coverage)
    );
    assert_eq!(
        with_session(handle, |session| Ok(session.selector_draw_feedback)).unwrap(),
        feedback_before_world_read
    );
    assert_eq!(
        with_session(handle, |session| {
            Ok((
                session.selector_frames.clone(),
                session.archer_selector_frames.clone(),
                session.firefly_selector_frames.clone(),
                session.villager_selector_frames.clone(),
                session.fireball_selector_frame.clone(),
                session.possession_selector_frame.clone(),
                session.castle_selector_frame.clone(),
            ))
        })
        .unwrap(),
        selector_frames_before_world_read
    );

    let first_world_indices = world_indices.clone();
    let first_world_info = world_info;
    assert_eq!(
        mc_native_session_world_frame_copy(
            handle,
            &raw const query,
            world_indices.as_mut_ptr(),
            world_indices.len() as u32,
            world_coverage.as_mut_ptr(),
            world_coverage.len() as u32,
            &raw mut world_info,
            struct_size::<McTerrainFrameInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(world_info, first_world_info);
    assert_eq!(world_indices, first_world_indices);
    assert_eq!(
        with_session(handle, |session| Ok(session.selector_draw_feedback))
            .expect("repeated draw feedback"),
        feedback_before_world_read
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}
