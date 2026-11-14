#[test]
fn positional_audio_qualification_vectors_cross_the_typed_sidecar_exactly() {
    assert_eq!(
        mc_native_audio_qualification_vector_count(),
        POSITIONAL_QUALIFICATION_VECTOR_COUNT
    );
    let expected_octants = [
        (43, 1_024, 30_036, 32_767, 0, 22_050),
        (37, 724, 30_560, 49_151, 0, 22_050),
        (39, 1_024, 29_126, 65_535, 0, 22_050),
        (37, 724, 29_678, 49_151, 0, 22_050),
        (37, 1_024, 27_305, 32_767, 0, 22_050),
        (37, 724, 29_678, 16_383, 0, 22_050),
        (37, 1_024, 29_126, 0, 0, 22_050),
        (37, 724, 30_560, 16_383, 0, 22_050),
    ];
    for (index, (sound, distance, volume, pan, rate_delta, rate_hz)) in
        expected_octants.into_iter().enumerate()
    {
        let mut command = McAudioCommandV2::default();
        assert_eq!(
            mc_native_audio_qualification_vector(
                u32::try_from(index).unwrap(),
                &raw mut command,
                struct_size::<McAudioCommandV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(command.abi_version, MC_NATIVE_ABI_VERSION);
        assert_eq!(command.struct_size, 88);
        assert_eq!(command.sound_id, sound);
        assert_eq!(
            sim_core::positional_sound_command(
                Position {
                    x: 32_768,
                    y: 32_768,
                    z: 1_000,
                },
                0,
                Position {
                    x: u16::try_from(32_768_i32 + command.relative_x).unwrap(),
                    y: u16::try_from(32_768_i32 + command.relative_y).unwrap(),
                    z: i16::try_from(1_000_i32 + command.relative_z).unwrap(),
                },
                command.sound_id,
                if index == 0 { 28 } else { 0 },
                321,
            )
            .unwrap()
            .distance_xy,
            distance
        );
        assert_eq!(command.volume_q15, volume);
        assert_eq!(command.pan_q16, pan);
        assert_eq!(command.rate_delta_percent, rate_delta);
        assert_eq!(command.playback_rate_hz, rate_hz);
        assert_eq!(command.play_type, 3);
        assert_eq!(command.loop_count, 0);
        assert_eq!(command.action, MC_AUDIO_ACTION_START);
        assert_eq!(command.voice_index, 0);
        assert_eq!(command.reserved, [0; 4]);
    }

    for (index, expected_volume, expected_action) in [
        (10, 514, MC_AUDIO_ACTION_START),
        (11, 511, MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME),
        (12, 515, MC_AUDIO_ACTION_START),
        (13, 511, MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME),
        (14, 517, MC_AUDIO_ACTION_START),
        (15, 511, MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME),
        (16, 0, MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME),
        (17, 0, MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME),
    ] {
        let mut command = McAudioCommandV2::default();
        assert_eq!(
            mc_native_audio_qualification_vector(
                index,
                &raw mut command,
                struct_size::<McAudioCommandV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(command.volume_q15, expected_volume);
        assert_eq!(command.action, expected_action);
    }

    for (index, delta, hz) in [(18, -15, 18_742), (19, 0, 22_050), (20, 14, 25_137)] {
        let mut command = McAudioCommandV2::default();
        assert_eq!(
            mc_native_audio_qualification_vector(
                index,
                &raw mut command,
                struct_size::<McAudioCommandV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(command.sound_flags, 0);
        assert_eq!(command.rate_delta_percent, delta);
        assert_eq!(command.playback_rate_hz, hz);
    }

    let mut duplicate = McAudioCommandV2::default();
    assert_eq!(
        mc_native_audio_qualification_vector(
            22,
            &raw mut duplicate,
            struct_size::<McAudioCommandV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(duplicate.sound_id, 43);
    assert_eq!(duplicate.action, MC_AUDIO_ACTION_SUPPRESS_EXISTING);
    assert_eq!(duplicate.voice_index, 0);

    let mut command = McAudioCommandV2::default();
    assert_eq!(
        mc_native_audio_qualification_vector(
            POSITIONAL_QUALIFICATION_VECTOR_COUNT,
            &raw mut command,
            struct_size::<McAudioCommandV2>(),
        ),
        MC_STATUS_BOUNDS
    );
}

#[test]
fn audio_sidecars_encode_original_loop_count_by_sound_class() {
    let handle = open_fixture();
    with_session_mut(handle, |session| {
        let entity_index = session.state.entity_index;
        let hash = state_hash(&session.state);
        for sound_id in [1, 2, 5, 31] {
            session.audio_commands.clear();
            session.audio_voices.fill(None);

            let start_event = session.encode_flight_event(
                FlightEvent::PrepareSound {
                    entity_index,
                    player_index: -1,
                    sound_id,
                },
                hash,
            );
            session.drain_pending_audio_commands();
            let start = session.audio_commands[&start_event.sequence];
            assert_eq!(start.loop_count, -1);
            assert_eq!(start.action, MC_AUDIO_ACTION_START);

            let update_event = session.encode_flight_event(
                FlightEvent::PrepareSound {
                    entity_index,
                    player_index: -1,
                    sound_id,
                },
                hash,
            );
            session.drain_pending_audio_commands();
            let update = session.audio_commands[&update_event.sequence];
            assert_eq!(update.loop_count, -1);
            assert_eq!(update.action, MC_AUDIO_ACTION_UPDATE_EXISTING);
            assert_eq!(update.voice_index, start.voice_index);

            let end_event = session.encode_flight_event(
                FlightEvent::EndLoop {
                    entity_index,
                    player_index: -1,
                    sound_id,
                },
                hash,
            );
            let end = session.audio_commands[&end_event.sequence];
            assert_eq!(end.loop_count, -1);
            assert_eq!(end.action, MC_AUDIO_ACTION_END_LOOP);
            assert_eq!(end.voice_index, start.voice_index);
        }

        let synthetic_event = McEventV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McEventV2>(),
            ..McEventV2::default()
        };
        assert_eq!(
            local_audio_command_record(&synthetic_event, 29, 0, 1,).loop_count,
            0
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn audio_command_discard_reclaims_resolved_sidecar_and_logical_voice() {
    let handle = open_fixture();
    let generation = with_session(handle, |session| Ok(session.generation)).unwrap();
    for sequence in 100..112 {
        with_session_mut(handle, |session| {
            let mut command = McAudioCommandV2 {
                generation,
                event_sequence: sequence,
                sound_id: 37,
                sound_flags: 0,
                play_type: 3,
                ..McAudioCommandV2::default()
            };
            let (action, voice_index) = session.allocate_audio_voice(command);
            command.action = action;
            command.voice_index = voice_index;
            assert_eq!(command.action, MC_AUDIO_ACTION_START);
            assert_eq!(command.voice_index, 0);
            session.audio_commands.insert(sequence, command);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            mc_native_audio_command_discard(handle, generation, sequence),
            MC_STATUS_OK
        );
        with_session(handle, |session| {
            assert!(!session.audio_commands.contains_key(&sequence));
            assert!(session.audio_voices.iter().all(Option::is_none));
            Ok(())
        })
        .unwrap();
    }

    with_session_mut(handle, |session| {
        session.audio_voices[0] = Some(AudioVoice {
            sound_id: 2,
            sound_flags: 0,
            event_sequence: 200,
        });
        for (sequence, action) in [
            (201, MC_AUDIO_ACTION_UPDATE_EXISTING),
            (202, MC_AUDIO_ACTION_END_LOOP),
        ] {
            session.audio_commands.insert(
                sequence,
                McAudioCommandV2 {
                    generation,
                    event_sequence: sequence,
                    sound_id: 2,
                    sound_flags: 0,
                    action,
                    voice_index: 0,
                    ..McAudioCommandV2::default()
                },
            );
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(
        mc_native_audio_command_discard(handle, generation, 201),
        MC_STATUS_OK
    );
    with_session_mut(handle, |session| {
        assert!(session.audio_voices[0].is_none());
        session.audio_voices[0] = Some(AudioVoice {
            sound_id: 2,
            sound_flags: 0,
            event_sequence: 200,
        });
        Ok(())
    })
    .unwrap();
    assert_eq!(
        mc_native_audio_command_discard(handle, generation, 202),
        MC_STATUS_OK
    );
    with_session(handle, |session| {
        assert!(session.audio_voices[0].is_none());
        Ok(())
    })
    .unwrap();

    for (sequence, action) in [
        (300, MC_AUDIO_ACTION_START),
        (301, MC_AUDIO_ACTION_UPDATE_EXISTING),
        (302, MC_AUDIO_ACTION_END_LOOP),
    ] {
        with_session_mut(handle, |session| {
            session.audio_voices[0] = Some(AudioVoice {
                sound_id: 2,
                sound_flags: 0,
                event_sequence: 400,
            });
            session.audio_commands.insert(
                sequence,
                McAudioCommandV2 {
                    generation,
                    event_sequence: sequence,
                    sound_id: 2,
                    sound_flags: 0,
                    action,
                    voice_index: 0,
                    ..McAudioCommandV2::default()
                },
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(
            mc_native_audio_command_discard(handle, generation, sequence),
            MC_STATUS_OK
        );
        with_session(handle, |session| {
            assert_eq!(
                session.audio_voices[0].map(|voice| voice.event_sequence),
                Some(400),
                "discarding stale start/update/end preserves a newer voice owner"
            );
            Ok(())
        })
        .unwrap();
    }
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn session_audio_sidecar_is_keyed_to_event_generation_and_cleared_on_reset() {
    let handle = open_fixture();
    let snapshot = with_session_mut(handle, |session| {
        session.state.entity_phase = 64;
        session
            .context
            .effects
            .as_mut()
            .expect("session has effects")
            .periodic_spell_effect = Some(0x13);
        session.step(PlayerCommand::default());
        Ok(session.snapshot())
    })
    .unwrap();
    let sequence = with_session(handle, |session| {
        session
            .events
            .iter()
            .find(|event| event.kind == MC_EVENT_PREPARE_SOUND && event.code == 43)
            .map(|event| event.sequence)
            .ok_or_else(|| AbiError::new(MC_STATUS_BOUNDS, "missing positional event"))
    })
    .unwrap();
    let mut command = McAudioCommandV2::default();
    assert_eq!(
        mc_native_audio_command_for_event(
            handle,
            snapshot.generation,
            sequence,
            &raw mut command,
            struct_size::<McAudioCommandV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(command.event_sequence, sequence);
    assert_eq!(command.state_hash, snapshot.state_hash);
    assert_eq!(command.sound_id, 43);
    assert_eq!(command.entity_index, 152);
    assert_eq!(command.sound_flags, 0);
    assert_eq!(
        (command.relative_x, command.relative_y, command.relative_z),
        (0, 0, 0)
    );
    assert_eq!(command.volume_q15, 0x7fff);
    assert_eq!(command.pan_q16, 0x7fff);
    assert_eq!(command.loop_count, 0);
    assert_eq!(command.action, MC_AUDIO_ACTION_START);
    assert_eq!(command.voice_index, 2);
    with_session(handle, |session| {
        let event_for = |kind, code| {
            session
                .events
                .iter()
                .find(|event| event.kind == kind && event.code == code)
                .copied()
                .expect("source-ledger event is present")
        };
        let periodic_43 = event_for(MC_EVENT_PREPARE_SOUND, 43);
        let land_2 = event_for(MC_EVENT_PREPARE_SOUND, 2);
        let land_end_1 = event_for(MC_EVENT_END_LOOP, 1);
        let ambient_fire_3 = session
            .audio_commands
            .iter()
            .find_map(|(sequence, audio)| {
                (audio.sound_id == 3
                    && audio.voice_index == 1
                    && audio.action == MC_AUDIO_ACTION_START)
                    .then(|| {
                        session
                            .events
                            .iter()
                            .find(|event| event.sequence == *sequence)
                            .copied()
                            .expect("winning sound-3 command retains its source event")
                    })
            })
            .expect("one ambient-fire source occurrence wins pooled sound 3");
        assert!(
            periodic_43.sequence < land_2.sequence
                && land_2.sequence < land_end_1.sequence
                && land_end_1.sequence < ambient_fire_3.sequence,
            "flight emits synthetic periodic 43 then pinned SurfaceLandLoop 2/1 before the pinned ambient-fire mission occurrence"
        );
        // Pinned source occurrence identities:
        // sound 2 SurfaceLandLoop = a8fdf305...3f1e8;
        // sound 3 AmbientFireAction01FirstEligibleSound = ac35f9a0...a9a9.
        let allocated = [land_2.sequence, ambient_fire_3.sequence, periodic_43.sequence]
            .map(|sequence| session.audio_commands[&sequence]);
        assert_eq!(
            allocated.map(|audio| {
                (
                    audio.sound_id,
                    audio.voice_index,
                    audio.pooled_flush_rank,
                    audio.action,
                )
            }),
            [
                (2, 0, 0, MC_AUDIO_ACTION_START),
                (3, 1, 1, MC_AUDIO_ACTION_START),
                (43, 2, 2, MC_AUDIO_ACTION_START),
            ],
            "direct sound 2 claims voice 0 before sound-id-ordered pooled 3 and synthetic/excluded 43 claim voices 1 and 2"
        );
        assert_eq!(
            session.audio_voices[..3]
                .iter()
                .map(|voice| voice.map(|voice| voice.sound_id))
                .collect::<Vec<_>>(),
            [Some(2), Some(3), Some(43)],
            "source-ordered land ambience and ambient-fire sound claim voices zero and one before pooled sound 43"
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(
        mc_native_audio_voice_complete(
            handle,
            snapshot.generation.wrapping_add(1),
            command.voice_index,
            command.event_sequence,
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(
        mc_native_audio_voice_complete(handle, snapshot.generation, 0, command.event_sequence,),
        MC_STATUS_BOUNDS
    );
    assert_eq!(
        mc_native_audio_voice_complete(
            handle,
            snapshot.generation,
            command.voice_index,
            command.event_sequence.wrapping_add(1),
        ),
        MC_STATUS_BOUNDS
    );
    assert_eq!(
        mc_native_audio_voice_complete(
            handle,
            snapshot.generation,
            command.voice_index,
            command.event_sequence,
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_audio_voice_complete(
            handle,
            snapshot.generation,
            command.voice_index,
            command.event_sequence,
        ),
        MC_STATUS_BOUNDS
    );

    let old_generation = snapshot.generation;
    let mut reset = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_reset(handle, &raw mut reset, struct_size::<McSessionSnapshotV2>(),),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_audio_command_for_event(
            handle,
            old_generation,
            sequence,
            &raw mut command,
            struct_size::<McAudioCommandV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(
        mc_native_audio_command_for_event(
            handle,
            reset.generation,
            sequence,
            &raw mut command,
            struct_size::<McAudioCommandV2>(),
        ),
        MC_STATUS_BOUNDS
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);

    let stored_rng = 0x9909_f858_u32;
    let nonstored_rng = stored_rng.wrapping_mul(9_377).wrapping_add(9_439);
    assert_eq!(nonstored_rng, 0xa631_b437);
    for (source_action, expected_delta) in [(0, 5), (14, 25)] {
        let prepared = prepare_event_sound_command(
            Position::default(),
            0,
            Position::default(),
            46,
            stored_rng,
            102,
            source_action,
        )
        .expect("sound46 has a source PrepareEventSound route");
        assert_eq!(
            prepared.command.rate_delta_percent, expected_delta,
            "sound46 derives its pitch from the nonstored next LCG value"
        );
        assert_eq!(
            stored_rng, 0x9909_f858,
            "pitch derivation never stores the derived LCG value"
        );
    }

    let snapshot_handle = open_fixture();
    let (snapshot_audio, listener, sound_position, mutated_position, captured_rng) =
        with_session_mut(snapshot_handle, |session| {
            const BLEATING_GOAT: u16 = 102;
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
                    i16::try_from(session.state.entity_index)
                        .expect("qualified player index fits signed source relation"),
                )
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
            session.mission.begin_source_turn_at(listener);
            let player = [GoatPlayerTarget {
                entity_index: session.state.entity_index,
                position: listener,
                control_flags: 0,
            }];
            let events =
                session
                    .live_world
                    .step_source_slot(BLEATING_GOAT, &session.terrain, &player);
            let (bleat, sound_position, sound_action) = events
                .into_iter()
                .find_map(|event| match event {
                    event @ GoatStepEvent::Bleat {
                        entity_index: BLEATING_GOAT,
                        sound_id: 46,
                        position,
                        action,
                    } => Some((event, position, action)),
                    _ => None,
                })
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "source-derived Goat slot 102 did not emit sound46",
                    )
                })?;
            assert_eq!(sound_action, 14);
            let captured_rng = session.mission.global_rng();
            assert_eq!(captured_rng, 0x9909_f858);

            let mutated_position = Position {
                x: sound_position.x.wrapping_add(4_096),
                y: sound_position.y.wrapping_add(2_048),
                z: sound_position.z.wrapping_add(512),
            };
            let goat = &mut entities[usize::from(BLEATING_GOAT)];
            goat.action = 0;
            goat.position = mutated_position;
            session.live_world = LiveWorldState::from_entities(
                &entities,
                session.world.goat_stage_point(),
                session.world.goat_config(),
            )
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;

            let event =
                session.encode_goat_event(bleat, session.live_world_hash_v2(), captured_rng);
            let audio = session
                .audio_commands
                .get(&event.sequence)
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "snapshotted sound46 event has no audio sidecar",
                    )
                })?;
            assert_eq!(session.mission.global_rng(), captured_rng);
            Ok((
                audio,
                listener,
                sound_position,
                mutated_position,
                captured_rng,
            ))
        })
        .expect("production Goat sound snapshot survives later live-row mutation");
    assert_eq!(captured_rng, 0x9909_f858);
    assert_eq!(snapshot_audio.rate_delta_percent, 25);
    assert_eq!(
        (
            snapshot_audio.relative_x,
            snapshot_audio.relative_y,
            snapshot_audio.relative_z,
        ),
        (
            i32::from(sound_position.x) - i32::from(listener.x),
            i32::from(sound_position.y) - i32::from(listener.y),
            i32::from(sound_position.z) - i32::from(listener.z),
        )
    );
    assert_ne!(sound_position, mutated_position);
    assert_eq!(mc_native_session_destroy(snapshot_handle), MC_STATUS_OK);

    let other_action = production_goat_pitch_case(false);
    let action_14 = production_goat_pitch_case(true);
    assert_eq!(
        (
            other_action.sound_id,
            other_action.rate_delta_percent,
            other_action.playback_rate_hz,
        ),
        (46, 5, 23_152),
        "stored 0x9909f858 derives nonstored 0xa631b437 and the other-action +5 family"
    );
    assert_eq!(
        (
            action_14.sound_id,
            action_14.rate_delta_percent,
            action_14.playback_rate_hz,
        ),
        (46, 25, 27_562),
        "stored 0x9909f858 derives nonstored 0xa631b437 and action-14 +25"
    );
}

#[test]
fn finite_source_sounds_reuse_stop_complete_and_reset_the_exact_shared_voice() {
    let mut commands = std::collections::BTreeMap::new();
    let mut voices = [None; AUDIO_VOICE_COUNT];
    let mut sequence = 0;
    for sound_id in [47, 49] {
        let prepared = sim_core::prepare_event_sound_command(
            Position::default(),
            0,
            Position {
                x: 1_024,
                ..Position::default()
            },
            sound_id,
            0,
            321,
            7,
        )
        .expect("real source preparation admits the finite terrain/tornado sample");
        assert_eq!(prepared.play_type, 4);
        assert_eq!(prepared.command.sound_flags, 0);
        let make_command = |event_sequence| {
            audio_command_record(
                7,
                event_sequence,
                123,
                u32::from(sound_id),
                sound_id,
                321,
                -1,
                prepared.command,
                prepared.play_type,
            )
        };
        sequence += 1;
        let start_sequence = sequence;
        commands.insert(sequence, make_command(sequence));
        super::audio_voice_pool::drain_pending_audio_commands(&mut commands, &mut voices);
        let start = commands[&sequence];
        assert_eq!(
            (start.action, start.voice_index, start.loop_count),
            (MC_AUDIO_ACTION_START, 0, 0)
        );
        assert_eq!(voices[0].unwrap().event_sequence, start_sequence);
        for _ in 0..AUDIO_VOICE_COUNT + 1 {
            sequence += 1;
            commands.insert(sequence, make_command(sequence));
            super::audio_voice_pool::drain_pending_audio_commands(&mut commands, &mut voices);
            assert_eq!(
                commands[&sequence].action,
                MC_AUDIO_ACTION_SUPPRESS_EXISTING
            );
            assert_eq!(commands[&sequence].voice_index, 0);
            assert_eq!(voices.iter().flatten().count(), 1);
            assert_eq!(voices[0].unwrap().event_sequence, start_sequence);
        }
        // Natural completion acknowledges the start, never a later suppressed
        // preparation or an EndLoop target request for this finite sample.
        assert!(super::audio_voice_pool::complete_audio_voice(&mut voices, 0, sequence).is_err());
        super::audio_voice_pool::complete_audio_voice(&mut voices, 0, start_sequence).unwrap();
        assert!(voices.iter().all(Option::is_none));

        sequence += 1;
        commands.insert(sequence, make_command(sequence));
        super::audio_voice_pool::drain_pending_audio_commands(&mut commands, &mut voices);
        let stopped_start = sequence;
        sequence += 1;
        let mut stop = make_command(sequence);
        stop.volume_q15 = 512;
        commands.insert(sequence, stop);
        super::audio_voice_pool::drain_pending_audio_commands(&mut commands, &mut voices);
        assert_eq!(
            (commands[&sequence].action, commands[&sequence].voice_index),
            (MC_AUDIO_ACTION_END_LOOP, 0)
        );
        assert_eq!(
            commands[&sequence].pooled_flush_rank, 1,
            "the device stop must be consumed in its source flush, after an earlier queued start"
        );
        assert!(voices.iter().all(Option::is_none));
        assert!(
            super::audio_voice_pool::complete_audio_voice(&mut voices, 0, stopped_start).is_err(),
            "source EndSample already retired the voice; the device must not complete it again"
        );

        sequence += 1;
        commands.insert(sequence, make_command(sequence));
        super::audio_voice_pool::drain_pending_audio_commands(&mut commands, &mut voices);
        assert_eq!(commands[&sequence].action, MC_AUDIO_ACTION_START);
        assert_eq!(voices[0].unwrap().event_sequence, sequence);
        super::audio_voice_pool::complete_audio_voice(&mut voices, 0, sequence).unwrap();
        sequence += 1;
        let mut stop_without_voice = make_command(sequence);
        stop_without_voice.volume_q15 = 512;
        commands.insert(sequence, stop_without_voice);
        super::audio_voice_pool::drain_pending_audio_commands(&mut commands, &mut voices);
        assert_eq!(
            (commands[&sequence].action, commands[&sequence].voice_index),
            (MC_AUDIO_ACTION_SUPPRESS_NO_EXISTING, MC_AUDIO_VOICE_NONE)
        );
    }

    let handle = open_fixture();
    with_session_mut(handle, |session| {
        session.audio_commands = commands;
        session.audio_voices[0] = Some(AudioVoice {
            sound_id: 47,
            sound_flags: 0,
            event_sequence: sequence,
        });
        session.reset();
        assert!(session.audio_voices.iter().all(Option::is_none));
        assert!(session.audio_commands.is_empty());
        Ok(())
    })
    .unwrap();
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn pending_slot_and_ten_voice_lifecycle_match_original_arbitration() {
    fn command(
        sequence: u64,
        sound_id: u16,
        sound_flags: i16,
        volume_q15: u16,
    ) -> McAudioCommandV2 {
        McAudioCommandV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: 88,
            generation: 1,
            event_sequence: sequence,
            sound_id,
            sound_flags,
            volume_q15,
            play_type: 3,
            voice_index: MC_AUDIO_VOICE_NONE,
            ..McAudioCommandV2::default()
        }
    }

    let handle = open_fixture();
    with_session_mut(handle, |session| {
        session.audio_commands.insert(1, command(1, 37, 20, 1_000));
        session.audio_commands.insert(2, command(2, 37, 21, 992));
        session.drain_pending_audio_commands();
        assert_eq!(
            session.audio_commands[&1].action,
            MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED
        );
        assert_eq!(session.audio_commands[&2].action, MC_AUDIO_ACTION_START);
        assert_eq!(session.audio_commands[&2].voice_index, 0);
        assert_eq!(session.audio_commands[&1].pooled_flush_rank, 0);
        assert_eq!(session.audio_commands[&2].pooled_flush_rank, 1);
        assert_eq!(session.audio_commands[&2].reserved[..2], [1, 0]);

        session.audio_commands.insert(3, command(3, 39, 30, 1_000));
        session.audio_commands.insert(4, command(4, 39, 31, 991));
        session.drain_pending_audio_commands();
        assert_eq!(session.audio_commands[&3].action, MC_AUDIO_ACTION_START);
        assert_eq!(
            session.audio_commands[&4].action,
            MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED
        );
        assert_eq!(session.audio_commands[&3].pooled_flush_rank, 1);
        assert_eq!(session.audio_commands[&4].pooled_flush_rank, 0);
        assert_eq!(session.audio_commands[&3].reserved[..2], [3, 0]);

        for index in 2..AUDIO_VOICE_COUNT {
            let sequence = u64::try_from(index + 10).unwrap();
            let flags = i16::try_from(index + 100).unwrap();
            session
                .audio_commands
                .insert(sequence, command(sequence, 37, flags, 2_000));
            session.drain_pending_audio_commands();
            assert_eq!(
                session.audio_commands[&sequence].voice_index,
                u16::try_from(index).unwrap()
            );
        }
        assert!(session.audio_voices.iter().all(Option::is_some));

        let full_sequence = 100;
        session
            .audio_commands
            .insert(full_sequence, command(full_sequence, 37, 999, 2_000));
        session.drain_pending_audio_commands();
        assert_eq!(
            session.audio_commands[&full_sequence].action,
            MC_AUDIO_ACTION_SUPPRESS_NO_VOICE
        );
        assert_eq!(
            session.audio_commands[&full_sequence].voice_index,
            MC_AUDIO_VOICE_NONE
        );

        let duplicate_sequence = 101;
        session.audio_commands.insert(
            duplicate_sequence,
            command(duplicate_sequence, 37, 21, 2_000),
        );
        session.drain_pending_audio_commands();
        assert_eq!(
            session.audio_commands[&duplicate_sequence].action,
            MC_AUDIO_ACTION_SUPPRESS_EXISTING
        );
        assert_eq!(session.audio_commands[&duplicate_sequence].voice_index, 0);

        session.audio_voices[0] = None;
        let reused_sequence = 102;
        session
            .audio_commands
            .insert(reused_sequence, command(reused_sequence, 37, 999, 2_000));
        session.drain_pending_audio_commands();
        assert_eq!(
            session.audio_commands[&reused_sequence].action,
            MC_AUDIO_ACTION_START
        );
        assert_eq!(session.audio_commands[&reused_sequence].voice_index, 0);

        let restart_sequence = 103;
        let mut restart = command(restart_sequence, 37, 999, 2_000);
        restart.play_type = 1;
        session.audio_commands.insert(restart_sequence, restart);
        session.drain_pending_audio_commands();
        assert_eq!(
            session.audio_commands[&restart_sequence].action,
            MC_AUDIO_ACTION_RESTART
        );
        assert_eq!(session.audio_commands[&restart_sequence].voice_index, 0);

        session.audio_commands.clear();
        session.audio_voices.fill(None);
        session
            .audio_commands
            .insert(200, command(200, 63, 63, 0x7fff));
        session
            .audio_commands
            .insert(201, command(201, 61, 61, 0x7fff));
        session.drain_pending_audio_commands();
        assert_eq!(session.audio_commands[&201].pooled_flush_rank, 1);
        assert_eq!(session.audio_commands[&200].pooled_flush_rank, 2);
        assert_eq!(session.audio_commands[&201].reserved[..2], [200, 0]);
        assert_eq!(session.audio_commands[&200].reserved[..2], [200, 0]);

        session.audio_commands.clear();
        session.audio_voices.fill(None);
        session.audio_commands.insert(210, command(210, 1, 0, 100));
        session.audio_commands.insert(211, command(211, 1, 0, 200));
        session.drain_pending_audio_commands();
        assert_eq!(session.audio_commands[&210].pooled_flush_rank, 0);
        assert_eq!(session.audio_commands[&211].pooled_flush_rank, 0);
        assert_ne!(
            session.audio_commands[&210].action,
            MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED
        );
        assert_ne!(
            session.audio_commands[&211].action,
            MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED
        );

        session.reset();
        assert!(session.audio_voices.iter().all(Option::is_none));
        assert!(session.audio_commands.is_empty());
        Ok(())
    })
    .unwrap();
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn paused_pending_audio_survives_ring_and_step_backlog_until_explicit_release() {
    let handle = open_fixture();
    let (generation, sequence) = with_session_mut(handle, |session| {
        session.game_paused = true;
        let mut event = session.next_event_base(MC_EVENT_PREPARE_SOUND, 1);
        event.code = 61;
        let command = local_audio_command_record(&event, 61, 0, 1);
        let sequence = event.sequence;
        session.insert_prepared_audio_command(sequence, command);
        session.append_event(event);
        for _ in 0..EVENT_CAPACITY {
            let event = session.next_event_base(MC_EVENT_MOVEMENT_RESOLUTION, 1);
            session.append_event(event);
        }
        assert!(
            !session
                .events
                .iter()
                .any(|event| event.sequence == sequence)
        );
        assert_eq!(session.audio_commands[&sequence].action, 0);
        assert!(session.audio_commands.len() <= EVENT_CAPACITY + 1);
        Ok((session.generation, sequence))
    })
    .unwrap();
    assert_eq!(
        mc_native_audio_command_release(handle, generation, sequence),
        MC_STATUS_INVALID_ARGUMENT,
        "clients cannot release an unresolved source-pool row"
    );

    with_session_mut(handle, |session| {
        session.game_paused = false;
        session.drain_pending_audio_commands();
        assert_ne!(session.audio_commands[&sequence].action, 0);
        session.step(PlayerCommand::default());
        session.step(PlayerCommand::default());
        assert!(session.audio_commands.contains_key(&sequence));
        Ok(())
    })
    .unwrap();
    assert_eq!(
        mc_native_audio_command_release(handle, generation, sequence),
        MC_STATUS_OK
    );
    let mut released = McAudioCommandV2::default();
    assert_eq!(
        mc_native_audio_command_for_event(
            handle,
            generation,
            sequence,
            &raw mut released,
            struct_size::<McAudioCommandV2>(),
        ),
        MC_STATUS_BOUNDS
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn versioned_struct_boundaries_reject_short_buffers_and_bad_strides() {
    let handle = open_fixture();
    let mut snapshot = McSessionSnapshotV2 {
        abi_version: 77,
        ..McSessionSnapshotV2::default()
    };
    let unchanged = snapshot;
    assert_eq!(
        mc_native_session_snapshot(
            handle,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>() - 1,
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    assert_eq!(snapshot, unchanged);

    let mut terrain_state = McTerrainStateV2 {
        abi_version: 77,
        ..McTerrainStateV2::default()
    };
    let unchanged_terrain_state = terrain_state;
    assert_eq!(
        mc_native_session_terrain_state(
            handle,
            &raw mut terrain_state,
            struct_size::<McTerrainStateV2>() - 1,
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    assert_eq!(terrain_state, unchanged_terrain_state);

    let mut lifecycle = McLevel1CastleLifecycleSnapshotV2 {
        abi_version: 77,
        ..McLevel1CastleLifecycleSnapshotV2::default()
    };
    let unchanged_lifecycle = lifecycle;
    assert_eq!(
        mc_native_level1_castle_lifecycle_snapshot(
            handle,
            &raw mut lifecycle,
            struct_size::<McLevel1CastleLifecycleSnapshotV2>() - 1,
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    assert_eq!(lifecycle, unchanged_lifecycle);

    let short_command_header = [MC_NATIVE_ABI_VERSION, 8];
    assert_eq!(
        mc_native_session_step(
            handle,
            short_command_header.as_ptr().cast::<McCommandV2>(),
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );

    let mut written = 99;
    assert_eq!(
        mc_native_entities_copy(
            handle,
            0,
            std::ptr::null_mut(),
            0,
            struct_size::<McEntitySnapshotV2>() - 1,
            &raw mut written,
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    assert_eq!(
        mc_native_entities_copy(
            handle,
            0,
            std::ptr::null_mut(),
            0,
            struct_size::<McEntitySnapshotV2>() + 1,
            &raw mut written,
        ),
        MC_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(
        mc_native_session_terrain_grid_copy(
            handle,
            std::ptr::null_mut(),
            0,
            struct_size::<McTerrainGridVertexV2>() - 1,
            &raw mut written,
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    assert_eq!(
        mc_native_session_terrain_grid_copy(
            handle,
            std::ptr::null_mut(),
            0,
            struct_size::<McTerrainGridVertexV2>() + 1,
            &raw mut written,
        ),
        MC_STATUS_INVALID_ARGUMENT
    );

    let short_query_header = AlignedAbiHeader {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: 8,
    };
    let mut batch = McEventBatchV2::default();
    assert_eq!(
        mc_native_events_copy_since(
            handle,
            (&raw const short_query_header).cast::<McEventQueryV2>(),
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn fast_event_journal_reports_expired_cursors() {
    const TEST_EVENT_CAPACITY: usize = 4;
    let handle = open_fixture_with_event_capacity(TEST_EVENT_CAPACITY);
    let command = valid_command();
    let mut snapshot = McSessionSnapshotV2::default();
    for _ in 0..=TEST_EVENT_CAPACITY {
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
    let query = McEventQueryV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McEventQueryV2>(),
        generation: snapshot.generation,
        cursor: 0,
        reserved: [0; 4],
    };
    let mut batch = McEventBatchV2::default();
    assert_eq!(
        mc_native_events_copy_since(
            handle,
            &raw const query,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_CURSOR_EXPIRED,
        "a deliberately too-old cursor remains rejected in the fast lane"
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
#[ignore = "production-capacity journal soak; run explicitly"]
fn bounded_event_journal_reports_expired_cursors() {
    let handle = open_fixture();
    let command = valid_command();
    let mut snapshot = McSessionSnapshotV2::default();
    for _ in 0..=EVENT_CAPACITY {
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
    let query = McEventQueryV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McEventQueryV2>(),
        generation: snapshot.generation,
        cursor: 0,
        reserved: [0; 4],
    };
    let mut batch = McEventBatchV2::default();
    assert_eq!(
        mc_native_events_copy_since(
            handle,
            &raw const query,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_CURSOR_EXPIRED
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn panic_is_contained_at_the_native_boundary() {
    assert_eq!(
        boundary(|| -> AbiResult<()> { panic!("intentional ABI containment probe") }),
        MC_STATUS_INTERNAL_PANIC
    );
    LAST_ERROR.with(|error| {
        assert!(error.borrow().contains("panic contained"));
    });
}

#[test]
fn zero_capacity_event_read_preserves_the_cursor() {
    let handle = open_fixture();
    let mut command = valid_command();
    command.flags = 0x10;
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
    let query = McEventQueryV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McEventQueryV2>(),
        generation: snapshot.generation,
        cursor: 0,
        reserved: [0; 4],
    };
    let mut batch = McEventBatchV2::default();
    assert_eq!(
        mc_native_events_copy_since(
            handle,
            &raw const query,
            std::ptr::null_mut(),
            0,
            struct_size::<McEventV2>(),
            &raw mut batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(batch.written, 0);
    assert_eq!(batch.next_cursor, query.cursor);
    assert_eq!(batch.has_more, 1);
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn null_utf8_version_size_reserved_and_unknown_handle_are_distinct() {
    let mut handle = 0_u64;
    assert_eq!(
        mc_native_session_open(std::ptr::null(), 1, std::ptr::null(), 1, &raw mut handle),
        MC_STATUS_NULL_POINTER
    );
    let bad_utf8 = [0xff_u8];
    assert_eq!(
        mc_native_session_open(bad_utf8.as_ptr(), 1, bad_utf8.as_ptr(), 1, &raw mut handle),
        MC_STATUS_INVALID_UTF8
    );
    let handle = open_fixture();
    let mut snapshot = McSessionSnapshotV2::default();
    let mut command = valid_command();
    command.abi_version = 99;
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_UNSUPPORTED_VERSION
    );
    command = valid_command();
    command.struct_size = 4;
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    command = valid_command();
    command.reserved[0] = 1;
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_RESERVED_NONZERO
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    assert_eq!(
        mc_native_session_snapshot(
            handle,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_UNKNOWN_HANDLE
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_UNKNOWN_HANDLE);
}

#[test]
fn level_one_exit_sidecar_holds_completed_action_and_resets_atomically() {
    let handle = open_fixture();
    let mut initial = McLevel1ExitPresentationV2::default();
    assert_eq!(
        mc_native_level1_exit_presentation(
            handle,
            &raw mut initial,
            struct_size::<McLevel1ExitPresentationV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(initial.authority, MC_AUTHORITY_LEVEL1_MISSION_LIVE);
    assert_eq!(
        initial.flags,
        MC_LEVEL1_EXIT_FLAG_PALETTE_VISUAL_CURRENT
            | MC_LEVEL1_EXIT_FLAG_FOV_POST_ACTION_NEXT_HISTORY
    );
    assert_eq!(
        (
            initial.player_action,
            initial.action_phase,
            initial.palette_submode,
            initial.palette_effect,
            initial.palette_weight,
            initial.palette_denominator,
            initial.camera_fov,
            initial.active,
            initial.complete,
        ),
        (0, 0, 0, 0, 0, 0, 128, 0, 0)
    );

    with_session_mut(handle, |session| {
        session.camera_fov = 121;
        session.mission.exit_player_action = 12;
        session.mission.exit_sequence_phase = 12;
        session.mission.exit_sequence_active = false;
        session.mission.exit_sequence_complete = true;
        session.mission.exit_result_code = 10;
        session.mission.exit_player_status_flags = 10;
        session.mission.exit_palette_effect = 2;
        session.mission.exit_palette_weight = 28;
        session.mission.exit_palette_denominator = 28;
        session.mission.exit_end_sequence_flag = true;
        session.mission.exit_display_blank = true;
        session.mission.player_mana = 123;
        Ok(())
    })
    .unwrap();

    let mut exit = McLevel1ExitPresentationV2::default();
    assert_eq!(
        mc_native_level1_exit_presentation(
            handle,
            &raw mut exit,
            struct_size::<McLevel1ExitPresentationV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (
            exit.player_action,
            exit.action_phase,
            exit.palette_effect,
            exit.palette_weight,
            exit.palette_denominator,
            exit.camera_fov,
            exit.player_status_flags,
            exit.input_suppressed,
            exit.display_blank,
            exit.result_code,
            exit.active,
            exit.complete,
        ),
        (12, 12, 2, 28, 28, 121, 10, 1, 1, 10, 0, 1)
    );
    assert_eq!(
        exit.flags,
        MC_LEVEL1_EXIT_FLAG_PALETTE_VISUAL_CURRENT
            | MC_LEVEL1_EXIT_FLAG_FOV_POST_ACTION_NEXT_HISTORY
            | MC_LEVEL1_EXIT_FLAG_END_SEQUENCE
    );

    let mut before = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_snapshot(
            handle,
            &raw mut before,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let command = McCommandV2 {
        flags: 1,
        roll: i8::MAX,
        pitch: i8::MAX,
        ..valid_command()
    };
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
    assert_eq!(
        (after.x, after.y, after.yaw, after.actual_speed),
        (before.x, before.y, before.yaw, before.actual_speed)
    );
    assert_eq!(after.tick, before.tick + 1);
    let held_mana = with_session(handle, |session| Ok(session.mission.player_mana)).unwrap();
    assert_eq!(held_mana, 123);

    let mut reset = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_reset(handle, &raw mut reset, struct_size::<McSessionSnapshotV2>(),),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_level1_exit_presentation(
            handle,
            &raw mut exit,
            struct_size::<McLevel1ExitPresentationV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(exit.generation, reset.generation);
    assert_eq!(exit.tick, reset.tick);
    assert_eq!(
        (
            exit.player_action,
            exit.action_phase,
            exit.palette_effect,
            exit.camera_fov,
            exit.input_suppressed,
            exit.result_code,
            exit.complete,
        ),
        (0, 0, 0, 128, 0, 0, 0)
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level_one_exit_sidecar_marks_post_action_fov_one_history_tick_ahead() {
    let handle = open_fixture();
    with_session_mut(handle, |session| {
        session.mission.exit_player_action = 12;
        session.mission.exit_sequence_active = true;
        session.mission.exit_sequence_phase = 5;
        session.mission.exit_sequence_timer = 12;
        session.mission.exit_player_status_flags = 0x20;
        Ok(())
    })
    .unwrap();
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
    let mut history = McPresentationStateV2::default();
    let mut exit = McLevel1ExitPresentationV2::default();
    assert_eq!(
        mc_native_presentation_state(
            handle,
            &raw mut history,
            struct_size::<McPresentationStateV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_level1_exit_presentation(
            handle,
            &raw mut exit,
            struct_size::<McLevel1ExitPresentationV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(history.camera_fov, 128);
    assert_eq!(exit.camera_fov, 127);
    assert_ne!(
        exit.flags & MC_LEVEL1_EXIT_FLAG_FOV_POST_ACTION_NEXT_HISTORY,
        0
    );

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
        mc_native_presentation_state(
            handle,
            &raw mut history,
            struct_size::<McPresentationStateV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_level1_exit_presentation(
            handle,
            &raw mut exit,
            struct_size::<McLevel1ExitPresentationV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(history.camera_fov, 127);
    assert_eq!(exit.camera_fov, 126);
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level_one_exit_sidecar_isolated_sessions_replay_independently() {
    let first = open_fixture();
    let second = open_fixture();
    with_session_mut(first, |session| {
        session.mission.exit_player_action = 12;
        session.mission.exit_sequence_active = true;
        session.mission.exit_sequence_phase = 5;
        session.mission.exit_sequence_timer = 12;
        session.mission.exit_player_status_flags = 0x20;
        Ok(())
    })
    .unwrap();
    let command = valid_command();
    let mut snapshot = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            first,
            &raw const command,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let mut first_exit = McLevel1ExitPresentationV2::default();
    let mut second_exit = McLevel1ExitPresentationV2::default();
    assert_eq!(
        mc_native_level1_exit_presentation(
            first,
            &raw mut first_exit,
            struct_size::<McLevel1ExitPresentationV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_level1_exit_presentation(
            second,
            &raw mut second_exit,
            struct_size::<McLevel1ExitPresentationV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (
            first_exit.player_action,
            first_exit.action_phase,
            first_exit.camera_fov,
            first_exit.active,
        ),
        (12, 5, 127, 1)
    );
    assert_eq!(
        (
            second_exit.player_action,
            second_exit.action_phase,
            second_exit.camera_fov,
            second_exit.active,
        ),
        (0, 0, 128, 0)
    );

    assert_eq!(
        mc_native_session_reset(
            first,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_level1_exit_presentation(
            first,
            &raw mut first_exit,
            struct_size::<McLevel1ExitPresentationV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (
            first_exit.player_action,
            first_exit.action_phase,
            first_exit.camera_fov,
            first_exit.active,
        ),
        (0, 0, 128, 0)
    );
    assert_eq!(mc_native_session_destroy(first), MC_STATUS_OK);
    assert_eq!(mc_native_session_destroy(second), MC_STATUS_OK);
}
