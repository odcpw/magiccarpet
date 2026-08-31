use super::*;

fn production_audio_tag_count(sources: &[&str], site: MissionAudioSite) -> usize {
    let test_module_marker = "\n#[cfg(test)]\nmod tests {";
    let variant = format!("MissionAudioSite::{site:?}");
    sources
        .iter()
        .map(|source| {
            let production = source
                .split_once(test_module_marker)
                .map_or(*source, |(production, _tests)| production);
            let mut rest = production;
            let mut count = 0;
            while let Some(field_start) = rest.find("audio_site:") {
                let expression = &rest[field_start + "audio_site:".len()..];
                let mut depth = 0_i32;
                let mut expression_end = expression.len();
                for (offset, byte) in expression.bytes().enumerate() {
                    match byte {
                        b'(' | b'[' | b'{' => depth += 1,
                        b')' | b']' | b'}' => depth -= 1,
                        b',' if depth == 0 => {
                            expression_end = offset;
                            break;
                        }
                        _ => {}
                    }
                }
                count += expression[..expression_end].matches(&variant).count();
                rest = expression
                    .get(expression_end.saturating_add(1)..)
                    .unwrap_or_default();
            }
            count
        })
        .sum()
}

#[test]
fn mission_audio_census_is_unique_complete_and_compiler_bound() {
    use std::collections::BTreeSet;

    assert_eq!(
        MissionAudioSite::ALL.len(),
        MissionAudioSite::Count as usize
    );
    assert_eq!(
        MissionAudioSite::ALL
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len(),
        MissionAudioSite::Count as usize,
        "the census must contain every site exactly once"
    );
    let tagged_sources = [
        include_str!("lib.rs"),
        include_str!("level1_mission.rs"),
        include_str!("spell_activation.rs"),
        include_str!("goat.rs"),
        include_str!("campaign/level2.rs"),
    ];
    let metadata_only = concat!(
        "Self::FireballLaunch,\n",
        "#[cfg(test)]\nmod tests {\n",
        "audio_site: crate::MissionAudioSite::FireballLaunch,\n",
        "}\n",
    );
    assert_eq!(
        production_audio_tag_count(&[metadata_only], MissionAudioSite::FireballLaunch),
        0,
        "metadata and test-only references must not satisfy production coverage"
    );
    assert_eq!(
        production_audio_tag_count(
            &["audio_site: crate::MissionAudioSite::FireballLaunch,"],
            MissionAudioSite::FireballLaunch,
        ),
        1,
        "an exact production event field must satisfy reverse binding"
    );
    assert_eq!(
        production_audio_tag_count(
            &[concat!(
                "audio_site: if selected {\n",
                "crate::MissionAudioSite::ExitSequenceSwitch\n",
                "} else {\n",
                "crate::MissionAudioSite::ExitAccelerationPhase6\n",
                "},\n",
            )],
            MissionAudioSite::ExitSequenceSwitch,
        ),
        1,
        "a variant in the assigned conditional expression remains compiler-bound"
    );
    assert_eq!(
        production_audio_tag_count(
            &["let metadata = crate::MissionAudioSite::ExitSequenceSwitch;"],
            MissionAudioSite::ExitSequenceSwitch,
        ),
        0,
        "a bare production metadata reference must not satisfy reverse binding"
    );
    for site in MissionAudioSite::ALL {
        let contract = site.contract();
        assert_eq!(contract.site, site);
        assert!(!contract.sounds.is_empty());
        assert!(!contract.owner.is_empty());
        assert!(!contract.play_type.is_empty());
        assert!(!contract.pitch.is_empty());
        assert!(!contract.source_anchor.is_empty());
        assert!(
            production_audio_tag_count(&tagged_sources, site) > 0,
            "{site:?} has metadata but no compiler-bound production tag"
        );
    }
    assert_eq!(
        MissionAudioSite::ALL
            .into_iter()
            .filter(|site| !site.contract().reachable)
            .collect::<Vec<_>>(),
        [
            MissionAudioSite::PeriodicSpellExcluded,
            MissionAudioSite::CaveAmbienceExcluded,
        ]
    );
    assert_eq!(
        MissionAudioSite::ALL
            .into_iter()
            .filter(|site| site.contract().sounds == "none")
            .collect::<Vec<_>>(),
        []
    );
}

#[test]
fn source_audio_occurrence_bindings_are_unique_and_exhaustive() {
    use std::collections::BTreeSet;

    let all_tags = SourceAudioCallTag::ALL.into_iter().collect::<BTreeSet<_>>();
    let bound_tags = SOURCE_AUDIO_CALL_BINDINGS
        .iter()
        .map(|(tag, _)| *tag)
        .collect::<BTreeSet<_>>();
    let bound_hashes = SOURCE_AUDIO_CALL_BINDINGS
        .iter()
        .map(|(_, hash)| *hash)
        .collect::<BTreeSet<_>>();

    assert_eq!(SourceAudioCallTag::ALL.len(), 53);
    assert_eq!(SOURCE_AUDIO_CALL_BINDINGS.len(), 53);
    assert_eq!(all_tags.len(), 53, "every source tag must be distinct");
    assert_eq!(
        bound_tags, all_tags,
        "every source tag must bind exactly once"
    );
    assert_eq!(
        bound_hashes.len(),
        53,
        "each reachable source occurrence must retain a distinct pinned hash"
    );
    assert!(SOURCE_AUDIO_CALL_BINDINGS.iter().all(|(_, hash)| {
        hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
    }));
}

#[derive(Clone, Copy)]
struct FlatWorld {
    ground: i16,
    ceiling: Option<i16>,
}

impl FlightWorld for FlatWorld {
    fn ground_height(&self, _position: Position) -> i16 {
        self.ground
    }
    fn ceiling_height(&self, _position: Position) -> Option<i16> {
        self.ceiling
    }
    fn material_mask(&self, _position: Position) -> u32 {
        0
    }
    fn angle_flags(&self, _tile: Tile) -> u8 {
        0
    }
}

const FLAT: FlatWorld = FlatWorld {
    ground: 0,
    ceiling: None,
};

struct WaterEdgeWorld {
    water_x: u16,
}

impl FlightWorld for WaterEdgeWorld {
    fn ground_height(&self, _position: Position) -> i16 {
        0
    }
    fn ceiling_height(&self, _position: Position) -> Option<i16> {
        None
    }
    fn material_mask(&self, position: Position) -> u32 {
        if position.x >= self.water_x {
            MOVEMENT_REDIRECT_MASK
        } else {
            0
        }
    }
    fn angle_flags(&self, _tile: Tile) -> u8 {
        0
    }
}

struct PointRedirectWorld {
    blocked: Position,
}

impl FlightWorld for PointRedirectWorld {
    fn ground_height(&self, _position: Position) -> i16 {
        0
    }
    fn ceiling_height(&self, _position: Position) -> Option<i16> {
        None
    }
    fn material_mask(&self, position: Position) -> u32 {
        if position == self.blocked {
            MOVEMENT_REDIRECT_MASK
        } else {
            0
        }
    }
    fn angle_flags(&self, _tile: Tile) -> u8 {
        0
    }
}

struct UniformCaveRedirectWorld;

impl FlightWorld for UniformCaveRedirectWorld {
    fn ground_height(&self, _position: Position) -> i16 {
        0
    }
    fn ceiling_height(&self, _position: Position) -> Option<i16> {
        Some(5_000)
    }
    fn material_mask(&self, _position: Position) -> u32 {
        MOVEMENT_REDIRECT_MASK
    }
    fn angle_flags(&self, _tile: Tile) -> u8 {
        0
    }
}

struct BlockedCaveWorld;

impl FlightWorld for BlockedCaveWorld {
    fn ground_height(&self, _position: Position) -> i16 {
        0
    }
    fn ceiling_height(&self, _position: Position) -> Option<i16> {
        Some(500)
    }
    fn material_mask(&self, _position: Position) -> u32 {
        0
    }
    fn angle_flags(&self, _tile: Tile) -> u8 {
        8
    }
}

struct SteeringCaveWorld {
    favor_left: bool,
}

impl FlightWorld for SteeringCaveWorld {
    fn ground_height(&self, _position: Position) -> i16 {
        0
    }
    fn ceiling_height(&self, position: Position) -> Option<i16> {
        Some(match position.y.cmp(&1_000) {
            Ordering::Less if self.favor_left => 2_000,
            Ordering::Greater if !self.favor_left => 2_000,
            Ordering::Less | Ordering::Greater => 1_000,
            Ordering::Equal => 800,
        })
    }
    fn material_mask(&self, _position: Position) -> u32 {
        0
    }
    fn angle_flags(&self, _tile: Tile) -> u8 {
        0
    }
}

struct SlopedWorld;

impl FlightWorld for SlopedWorld {
    fn ground_height(&self, position: Position) -> i16 {
        i16::try_from(position.x / 4).unwrap_or(i16::MAX)
    }
    fn ceiling_height(&self, _position: Position) -> Option<i16> {
        None
    }
    fn material_mask(&self, _position: Position) -> u32 {
        0
    }
    fn angle_flags(&self, _tile: Tile) -> u8 {
        0
    }
}

#[derive(Clone, Copy)]
struct UniformMaterialWorld {
    material: u32,
    ground: i16,
}

impl FlightWorld for UniformMaterialWorld {
    fn ground_height(&self, _position: Position) -> i16 {
        self.ground
    }
    fn ceiling_height(&self, _position: Position) -> Option<i16> {
        None
    }
    fn material_mask(&self, _position: Position) -> u32 {
        self.material
    }
    fn angle_flags(&self, _tile: Tile) -> u8 {
        0
    }
}

#[test]
fn command_layout_and_profiles_are_exactly_recorded() {
    assert_eq!(size_of::<PlayerCommand>(), 10);
    assert_eq!(FlightProfile::OUTDOOR.terrain_reference_height, 1_024);
    assert_eq!(FlightProfile::OUTDOOR.passive_vertical_step, -16);
    assert_eq!(FlightProfile::CAVE.terrain_reference_height, 3_072);
    assert_eq!(FlightProfile::CAVE.passive_vertical_step, -8);
    assert_eq!(FlightProfile::CAVE.ceiling_clearance, 384);

    let command = PlayerCommand {
        opcode: -1,
        argument_1: 2,
        argument_2: -3,
        roll: 4,
        pitch: -5,
        flags: -128,
        argument_6: 0x1234,
        argument_8: 0xabcd,
    };
    assert_eq!(PlayerCommand::from_bytes(command.to_bytes()), command);
}

#[test]
fn input_delta_truncates_toward_zero() {
    assert_eq!(derive_input_delta(7, 0), 3);
    assert_eq!(derive_input_delta(-7, 0), -3);
    assert_eq!(derive_input_delta(0, 7), -1);
    assert_eq!(derive_input_delta(0, -7), 1);
}

#[test]
fn full_stop_opcode_zeroes_rotation_speed_and_the_exact_spell_holder() {
    let mut state = FlightState {
        actual_speed: 64,
        target_speed: 80,
        roll_accumulator: 12,
        pitch_accumulator: -12,
        speed_spell_holder_index: 42,
        ..FlightState::default()
    };
    let mut context = FlightContext {
        spell_fire: Some(SpellFireContext {
            holders: vec![SpellActivationHolder {
                entity_index: 42,
                model: level1_mission::SPEED_SPELL_FAMILY,
                active_value: 9,
                ..SpellActivationHolder::default()
            }],
            ..SpellFireContext::default()
        }),
        speed_spell_holder: Some(SpellHolder {
            entity_index: 42,
            active_value: 9,
        }),
        ..FlightContext::default()
    };
    let latched = latch_command_with_context(
        &mut state,
        PlayerCommand {
            opcode: 0x27,
            argument_1: 1,
            roll: 100,
            pitch: -100,
            flags: 1,
            ..PlayerCommand::default()
        },
        &mut context,
    );

    assert_eq!(state.actual_speed, 0);
    assert_eq!(state.target_speed, 0);
    assert_eq!(state.roll_delta, -3);
    assert_eq!(state.pitch_delta, 3);
    assert_eq!(latched.flags, 1);
    assert_eq!(
        context.speed_spell_holder.unwrap().active_value,
        0,
        "opcode 0x27 writes entity word_0x2E_46"
    );
    assert_eq!(
        context.spell_fire.unwrap().holders[0].active_value,
        0,
        "the spell registry and movement projection alias the source entity"
    );
}

#[test]
fn full_stop_without_speed_argument_preserves_speed_and_holder() {
    let mut state = FlightState {
        actual_speed: 64,
        target_speed: 80,
        roll_accumulator: 12,
        pitch_accumulator: -12,
        speed_spell_holder_index: 42,
        ..FlightState::default()
    };
    let mut context = FlightContext {
        spell_fire: Some(SpellFireContext {
            holders: vec![SpellActivationHolder {
                entity_index: 42,
                model: level1_mission::SPEED_SPELL_FAMILY,
                active_value: 9,
                ..SpellActivationHolder::default()
            }],
            ..SpellFireContext::default()
        }),
        speed_spell_holder: Some(SpellHolder {
            entity_index: 42,
            active_value: 9,
        }),
        ..FlightContext::default()
    };
    latch_command_with_context(
        &mut state,
        PlayerCommand {
            opcode: 0x27,
            roll: 100,
            pitch: -100,
            ..PlayerCommand::default()
        },
        &mut context,
    );

    assert_eq!(state.actual_speed, 64);
    assert_eq!(state.target_speed, 80);
    assert_eq!(state.roll_delta, -3);
    assert_eq!(state.pitch_delta, 3);
    assert_eq!(context.speed_spell_holder.unwrap().active_value, 9);
    assert_eq!(context.spell_fire.unwrap().holders[0].active_value, 9);
}

#[test]
fn target_and_strafe_controls_obey_steps_limits_and_decay() {
    let mut state = FlightState::default();
    for _ in 0..10 {
        decode_movement(1, &mut state);
    }
    assert_eq!(state.target_speed, 80);
    for _ in 0..20 {
        decode_movement(2, &mut state);
    }
    assert_eq!(state.target_speed, -80);

    state.strafe_speed = 8;
    decode_movement(0, &mut state);
    assert_eq!(state.strafe_speed, 4);
    decode_movement(0, &mut state);
    decode_movement(0, &mut state);
    assert_eq!(state.strafe_speed, 0);
}

#[test]
fn definition_bound_forward_tuning_uses_the_ordinary_decoder() {
    let tuning = FlightTuning {
        max_forward_speed: 16,
        forward_acceleration: 8,
    };
    let mut authored = FlightState::default();
    let mut classic = FlightState::default();
    decode_movement_with_tuning(1, &mut authored, tuning);
    decode_movement(1, &mut classic);
    assert_eq!(authored.target_speed, 8);
    assert_eq!(classic.target_speed, 16);
    for _ in 0..3 {
        decode_movement_with_tuning(1, &mut authored, tuning);
        decode_movement(1, &mut classic);
    }
    assert_eq!(authored.target_speed, 16);
    assert_eq!(classic.target_speed, 64);

    for _ in 0..12 {
        decode_movement_with_tuning(2, &mut authored, tuning);
    }
    assert_eq!(authored.target_speed, -80);
}

#[test]
fn opposing_movement_flags_preserve_original_priority() {
    let mut state = FlightState::default();
    decode_movement(0b0011, &mut state);
    assert_eq!(state.target_speed, -16);

    state.target_speed = -80;
    decode_movement(0b0011, &mut state);
    assert_eq!(state.target_speed, -64);

    state.strafe_speed = 0;
    decode_movement(0b1100, &mut state);
    assert_eq!(state.strafe_speed, 16);
}

#[test]
fn negative_strafe_reaches_its_limit_and_decays_through_zero() {
    let mut state = FlightState::default();
    for _ in 0..10 {
        decode_movement(0x04, &mut state);
    }
    assert_eq!(state.strafe_speed, SOURCE_STRAFE_MINIMUM);

    state.strafe_speed = -5;
    decode_movement(0, &mut state);
    assert_eq!(state.strafe_speed, -1);
    decode_movement(0, &mut state);
    assert_eq!(state.strafe_speed, 0);
}

#[test]
fn target_speed_change_flag_is_cleared_and_set_in_source_order() {
    let mut state = FlightState {
        target_speed: SPEED_LIMIT,
        speed_changed_flag: 7,
        ..FlightState::default()
    };

    decode_movement(0x01, &mut state);
    assert_eq!(state.target_speed, SPEED_LIMIT);
    assert_eq!(state.speed_changed_flag, 0);

    decode_movement(0x02, &mut state);
    assert_eq!(state.target_speed, SPEED_LIMIT - SPEED_STEP);
    assert_eq!(state.speed_changed_flag, 1);

    decode_movement(0, &mut state);
    assert_eq!(state.speed_changed_flag, 0);
}

#[test]
fn redirect_distance_uses_the_source_low_word() {
    assert_eq!(low_u16(65_537), 1);
    assert_eq!(low_u16(u32::MAX), u16::MAX);
}

#[test]
fn positional_sound_octants_preserve_original_integer_volume_and_pan() {
    let listener = Position {
        x: 32_768,
        y: 32_768,
        z: 1_000,
    };
    let vectors = [
        ("front", 0, -1_024, 1_024, 0, 30_036, 32_767, 117, 63),
        ("front-right", 512, -512, 724, 256, 30_560, 49_151, 119, 95),
        ("right", 1_024, 0, 1_024, 512, 29_126, 65_535, 113, 127),
        ("back-right", 512, 512, 724, 768, 29_678, 49_151, 115, 95),
        ("back", 0, 1_024, 1_024, 1_024, 27_305, 32_767, 106, 63),
        ("back-left", -512, 512, 724, 768, 29_678, 16_383, 115, 31),
        ("left", -1_024, 0, 1_024, 512, 29_126, 0, 113, 0),
        ("front-left", -512, -512, 724, 256, 30_560, 16_383, 119, 31),
    ];
    for (name, dx, dy, distance, relative_yaw, volume, pan, playback_volume, playback_pan) in
        vectors
    {
        let source = Position {
            x: u16::try_from(i32::from(listener.x) + dx).unwrap(),
            y: u16::try_from(i32::from(listener.y) + dy).unwrap(),
            z: listener.z,
        };
        let command = positional_sound_command(listener, 0, source, 37, 0, 321).unwrap();
        assert!(command.audible, "{name}");
        assert_eq!(command.distance_xy, distance, "{name}");
        assert_eq!(command.relative_yaw, relative_yaw, "{name}");
        assert_eq!(command.volume_q15, volume, "{name}");
        assert_eq!(command.pan_q16, pan, "{name}");
        assert_eq!(command.volume_q15 >> 8, playback_volume, "{name}");
        assert_eq!(
            127 * u32::from(command.pan_q16) / 0xffff,
            playback_pan,
            "{name}"
        );
        assert_eq!(command.sound_flags, 321, "{name}");
    }
}

#[test]
fn positional_sound_distance_pan_and_cutoff_boundaries_are_exact() {
    let listener = Position {
        x: 32_768,
        y: 32_768,
        z: 1_000,
    };
    let at = |dx: i32, dy: i32, z: i16| Position {
        x: u16::try_from(i32::from(listener.x) + dx).unwrap(),
        y: u16::try_from(i32::from(listener.y) + dy).unwrap(),
        z,
    };
    let command = |source| positional_sound_command(listener, 0, source, 37, 0, 152).unwrap();

    assert_eq!(command(at(320, 0, 1_000)).pan_q16, 32_767);
    assert_eq!(command(at(321, 0, 1_000)).pan_q16, 65_535);

    let front_pass = command(at(0, -12_095, 1_000));
    let front_suppress = command(at(0, -12_096, 1_000));
    assert_eq!((front_pass.volume_q15, front_pass.audible), (514, true));
    assert_eq!(
        (front_suppress.volume_q15, front_suppress.audible),
        (511, false)
    );

    let side_pass = command(at(9_071, 0, 1_000));
    let side_suppress = command(at(9_072, 0, 1_000));
    assert_eq!((side_pass.volume_q15, side_pass.audible), (515, true));
    assert_eq!(
        (side_suppress.volume_q15, side_suppress.audible),
        (511, false)
    );

    let back_pass = command(at(0, 6_047, 1_000));
    let back_suppress = command(at(0, 6_048, 1_000));
    assert_eq!((back_pass.volume_q15, back_pass.audible), (517, true));
    assert_eq!(
        (back_suppress.volume_q15, back_suppress.audible),
        (511, false)
    );

    let squared_limit = command(at(12_288, 0, 1_000));
    let beyond_squared_limit = command(at(12_288, 1, 1_000));
    assert_eq!(squared_limit.distance_xy, 12_288);
    assert!(!squared_limit.audible);
    assert_eq!(beyond_squared_limit.distance_xy, 12_288);
    assert!(!beyond_squared_limit.audible);

    let high = command(at(1_024, 0, 30_000));
    let low = command(at(1_024, 0, -30_000));
    assert_eq!(high.distance_xy, low.distance_xy);
    assert_eq!(high.volume_q15, low.volume_q15);
    assert_eq!(high.pan_q16, low.pan_q16);
    assert_ne!(high.relative_z, low.relative_z);
}

#[test]
fn positional_sound_rate_flags_and_candidate_arbitration_are_exact() {
    let listener = Position {
        x: 32_768,
        y: 32_768,
        z: 1_000,
    };
    for (seed, expected_delta) in [(13, -15), (28, 0), (20, 14)] {
        let command = positional_sound_command(listener, 0, listener, 43, seed, 152).unwrap();
        assert_eq!(command.rate_delta_percent, expected_delta);
        assert_eq!(command.sound_flags, 0);
    }
    for sound_id in [37, 39] {
        let command = positional_sound_command(listener, 0, listener, sound_id, 13, 152).unwrap();
        assert_eq!(command.rate_delta_percent, 0);
        assert_eq!(command.sound_flags, 152);
    }
    assert!(positional_sound_command(listener, 0, listener, 29, 0, 0).is_none());

    assert!(positional_sound_should_replace(992, 1_000));
    assert!(!positional_sound_should_replace(991, 1_000));
    assert!(positional_sound_should_replace(1_001, 1_000));
}

#[test]
fn prepared_event_sound_preserves_source_play_types_flags_and_pitch_families() {
    let listener = Position {
        x: 32_768,
        y: 32_768,
        z: 1_000,
    };

    for sound_id in [9, 30, 40, 41, 63] {
        let prepared = prepare_event_sound_command(listener, 0, listener, sound_id, 28, 152, 0)
            .expect("restart-or-start sound is in the source table");
        assert_eq!(prepared.play_type, 1, "sound {sound_id}");
        assert_eq!(prepared.command.sound_flags, 152, "sound {sound_id}");
    }
    for sound_id in [8, 16, 33, 34, 37, 39, 43, 44, 46, 54, 55, 56, 57] {
        let prepared = prepare_event_sound_command(listener, 0, listener, sound_id, 28, 152, 0)
            .expect("if-not-playing sound is in the source table");
        assert_eq!(prepared.play_type, 3, "sound {sound_id}");
    }

    for sound_id in [43, 44, 46] {
        let prepared =
            prepare_event_sound_command(listener, 0, listener, sound_id, 28, 152, 0).unwrap();
        assert_eq!(prepared.command.sound_flags, 0, "sound {sound_id}");
    }
    assert_eq!(
        prepare_event_sound_command(listener, 0, listener, 43, 13, 152, 0)
            .unwrap()
            .command
            .rate_delta_percent,
        -15
    );
    assert_eq!(
        prepare_event_sound_command(listener, 0, listener, 44, 20, 152, 0)
            .unwrap()
            .command
            .rate_delta_percent,
        14
    );
    assert_eq!(
        prepare_event_sound_command(listener, 0, listener, 46, 13, 152, 14)
            .unwrap()
            .command
            .rate_delta_percent,
        10
    );
    assert_eq!(
        prepare_event_sound_command(listener, 0, listener, 46, 13, 152, 0)
            .unwrap()
            .command
            .rate_delta_percent,
        -10
    );
    assert_eq!(
        prepare_event_sound_command(listener, 0, listener, 46, 0, 152, 14)
            .unwrap()
            .command
            .rate_delta_percent,
        29
    );
    assert_eq!(
        prepare_event_sound_command(listener, 0, listener, 46, 0, 152, 0)
            .unwrap()
            .command
            .rate_delta_percent,
        9
    );
}

#[test]
fn positional_sound_coordinate_wrap_and_rotated_yaw_are_hard_vectors() {
    let wrapped_listener = Position {
        x: 65_530,
        y: 20_000,
        z: 1_000,
    };
    let wrapped_source = Position {
        x: 5,
        y: 20_000,
        z: 1_000,
    };
    let wrapped =
        positional_sound_command(wrapped_listener, 512, wrapped_source, 39, 0, 77).unwrap();
    assert_eq!(wrapped.relative_x, -65_525);
    assert_eq!(wrapped.distance_xy, 11);
    assert_eq!(wrapped.relative_yaw, 0);
    assert_eq!(wrapped.volume_q15, 32_737);
    assert_eq!(wrapped.pan_q16, 32_767);

    let listener = Position {
        x: 32_768,
        y: 32_768,
        z: 1_000,
    };
    let east = Position {
        x: 33_792,
        ..listener
    };
    let north = Position {
        y: 31_744,
        ..listener
    };
    let rotated_front = positional_sound_command(listener, 512, east, 37, 0, 152).unwrap();
    let rotated_left = positional_sound_command(listener, 512, north, 37, 0, 152).unwrap();
    assert_eq!(
        (
            rotated_front.relative_yaw,
            rotated_front.volume_q15,
            rotated_front.pan_q16,
        ),
        (0, 30_036, 32_767)
    );
    assert_eq!(
        (
            rotated_left.relative_yaw,
            rotated_left.volume_q15,
            rotated_left.pan_q16,
        ),
        (512, 29_126, 0)
    );
}

#[test]
fn input_is_latched_once_for_multiple_entity_passes() {
    let mut state = FlightState::default();
    let command = latch_command(
        &mut state,
        PlayerCommand {
            roll: 8,
            ..PlayerCommand::default()
        },
    );
    assert_eq!(state.roll_delta, 4);
    step_entity_pass(&mut state, command, FlightProfile::OUTDOOR, &FLAT);
    step_entity_pass(&mut state, command, FlightProfile::OUTDOOR, &FLAT);
    assert_eq!(state.roll_delta, 4);
    assert_eq!(state.roll_accumulator, 8);
}

#[test]
fn outer_speed_index_selects_exactly_one_four_eight_or_zero_passes() {
    let command = PlayerCommand {
        flags: 1,
        ..PlayerCommand::default()
    };
    for (speed_index, expected_passes, expected_speed) in
        [(0, 1, 16), (1, 4, 64), (2, 8, 80), (3, 0, 0), (255, 0, 0)]
    {
        let mut state = FlightState::default();
        let evidence = step_outer_turn_with_context(
            &mut state,
            command,
            speed_index,
            FlightProfile::OUTDOOR,
            &FLAT,
            &mut FlightContext::default(),
        );
        assert_eq!(evidence.len(), expected_passes);
        assert_eq!(state.tick, u64::try_from(expected_passes).unwrap());
        assert_eq!(state.target_speed, expected_speed);
        assert_eq!(state.actual_speed, expected_speed);
    }
}

#[test]
fn four_and_eight_pass_modes_match_every_intermediate_source_state() {
    let command = PlayerCommand {
        flags: 1,
        ..PlayerCommand::default()
    };
    for (pass_count, expected_y, expected_z) in [
        (
            4_usize,
            [32_752, 32_720, 32_672, 32_608, 0, 0, 0, 0],
            [1_264, 1_248, 1_232, 1_216, 0, 0, 0, 0],
        ),
        (
            8,
            [
                32_752, 32_720, 32_672, 32_608, 32_528, 32_448, 32_368, 32_288,
            ],
            [1_264, 1_248, 1_232, 1_216, 1_200, 1_184, 1_168, 1_152],
        ),
    ] {
        let mut state = FlightState::default();
        let latched = latch_command(&mut state, command);
        for pass in 0..pass_count {
            let evidence = step_entity_pass(&mut state, latched, FlightProfile::OUTDOOR, &FLAT);
            let expected_speed =
                i16::try_from(((pass + 1) * 16).min(80)).expect("bounded expected speed");
            assert_eq!(state.target_speed, expected_speed, "pass {}", pass + 1);
            assert_eq!(state.actual_speed, expected_speed, "pass {}", pass + 1);
            assert_eq!(state.speed_changed_flag, i16::from(pass < 5));
            assert_eq!(state.entity_phase, u8::try_from(pass + 1).unwrap());
            assert_eq!(state.tick, u64::try_from(pass + 1).unwrap());
            assert_eq!(evidence.committed.y, expected_y[pass]);
            assert_eq!(evidence.committed.z, expected_z[pass]);
        }
    }
}

#[test]
fn outdoor_profile_ignores_a_provider_ceiling() {
    let world = FlatWorld {
        ground: 0,
        ceiling: Some(500),
    };
    let mut state = FlightState {
        position: Position {
            x: 0,
            y: 0,
            z: 1_000,
        },
        ..FlightState::default()
    };
    step_turn(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::OUTDOOR,
        &world,
    );
    assert_eq!(state.position.z, 984);
}

#[test]
fn yaw_uses_signed_division_with_truncation_toward_zero() {
    for (roll, expected_yaw) in [(-7, 0), (-8, 2_047), (7, 0), (8, 1)] {
        let mut state = FlightState {
            roll_accumulator: roll,
            ..FlightState::default()
        };
        step_entity_pass(
            &mut state,
            LatchedCommand::default(),
            FlightProfile::OUTDOOR,
            &FLAT,
        );
        assert_eq!(state.yaw, expected_yaw, "roll accumulator {roll}");
    }
}

#[test]
fn deterministic_outdoor_sound_and_music_events_preserve_source_order() {
    let mut state = FlightState {
        entity_index: 42,
        ..FlightState::default()
    };
    let mut context = FlightContext {
        effects: Some(FlightEffectsContext {
            periodic_spell_effect: Some(2),
            ..FlightEffectsContext::default()
        }),
        ..FlightContext::default()
    };

    step_turn_with_context(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::OUTDOOR,
        &FLAT,
        &mut context,
    );

    assert_eq!(state.ambient_loop_5_distance, 2_048);
    assert_eq!(state.ambient_loop_31_distance, 2_048);
    assert_eq!(state.entity_phase, 1);
    assert_eq!(
        context.events,
        vec![
            FlightEvent::PrepareSound {
                audio_site: MissionAudioSite::PeriodicSpellExcluded,
                entity_index: 42,
                player_index: -1,
                sound_id: 12,
            },
            FlightEvent::PrepareSound {
                audio_site: MissionAudioSite::SurfaceLandLoop,
                entity_index: 0,
                player_index: 0,
                sound_id: 2,
            },
            FlightEvent::EndLoop {
                audio_site: MissionAudioSite::SurfaceLandLoop,
                entity_index: 0,
                player_index: 0,
                sound_id: 1,
            },
            FlightEvent::PrepareSound {
                audio_site: MissionAudioSite::DistanceLoop5,
                entity_index: 0,
                player_index: 0,
                sound_id: 5,
            },
            FlightEvent::PrepareSound {
                audio_site: MissionAudioSite::DistanceLoop31,
                entity_index: 0,
                player_index: 0,
                sound_id: 31,
            },
            FlightEvent::UpdateMusic {
                mode: 1,
                timer_multiplier: 3,
            },
        ]
    );
}

#[test]
fn cave_ambience_advances_the_exact_rng_and_skip_bypasses_all_effects() {
    let cave = FlatWorld {
        ground: 0,
        ceiling: Some(5_000),
    };
    let mut state = FlightState::default();
    let mut context = FlightContext {
        effects: Some(FlightEffectsContext {
            global_rng: 12,
            ..FlightEffectsContext::default()
        }),
        ..FlightContext::default()
    };
    step_turn_with_context(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::CAVE,
        &cave,
        &mut context,
    );
    let effects = context.effects.as_ref().unwrap();
    assert_eq!(effects.global_rng, 121_963);
    assert_eq!(
        context.events.first(),
        Some(&FlightEvent::PrepareSound {
            audio_site: MissionAudioSite::CaveAmbienceExcluded,
            entity_index: 0,
            player_index: 0,
            sound_id: 67,
        })
    );

    state.skip_flight_once = true;
    state.ambient_loop_5_distance = 0;
    context.events.clear();
    step_turn_with_context(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::CAVE,
        &cave,
        &mut context,
    );
    assert!(context.events.is_empty());
    assert_eq!(context.effects.as_ref().unwrap().global_rng, 121_963);
    assert_eq!(state.ambient_loop_5_distance, 0);
    assert_eq!(state.entity_phase, 2);
}

#[test]
fn quiet_effect_branches_preserve_loops_music_and_locality() {
    let water = UniformMaterialWorld {
        material: WATER_AMBIENCE_MASK,
        ground: 0,
    };
    let mut state = FlightState {
        entity_phase: 1,
        ambient_loop_5_distance: 1_536,
        ambient_loop_31_distance: 2_000,
        music_countdown: 2,
        ..FlightState::default()
    };
    let mut context = FlightContext {
        effects: Some(FlightEffectsContext {
            periodic_spell_effect: Some(3),
            ..FlightEffectsContext::default()
        }),
        ..FlightContext::default()
    };
    step_turn_with_context(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::OUTDOOR,
        &water,
        &mut context,
    );
    assert_eq!(
        context.events,
        vec![
            FlightEvent::PrepareSound {
                audio_site: MissionAudioSite::SurfaceWaterLoop,
                entity_index: 0,
                player_index: 0,
                sound_id: 1,
            },
            FlightEvent::EndLoop {
                audio_site: MissionAudioSite::SurfaceWaterLoop,
                entity_index: 0,
                player_index: 0,
                sound_id: 2,
            },
            FlightEvent::EndLoop {
                audio_site: MissionAudioSite::DistanceLoop5,
                entity_index: 0,
                player_index: 0,
                sound_id: 5,
            },
            FlightEvent::EndLoop {
                audio_site: MissionAudioSite::DistanceLoop31,
                entity_index: 0,
                player_index: 0,
                sound_id: 31,
            },
            FlightEvent::UpdateMusic {
                mode: 2,
                timer_multiplier: 3,
            },
        ]
    );
    assert_eq!(state.music_countdown, 1);

    state.player_color_index = 1;
    context.events.clear();
    step_turn_with_context(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::OUTDOOR,
        &water,
        &mut context,
    );
    assert!(
        !context
            .events
            .iter()
            .any(|event| matches!(event, FlightEvent::UpdateMusic { .. }))
    );

    let cave = FlatWorld {
        ground: 0,
        ceiling: Some(5_000),
    };
    state.player_color_index = 0;
    context.events.clear();
    context.effects.as_mut().unwrap().global_rng = 0;
    step_turn_with_context(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::CAVE,
        &cave,
        &mut context,
    );
    assert!(!context.events.iter().any(|event| matches!(
        event,
        FlightEvent::PrepareSound {
            sound_id: 65..=69,
            ..
        }
    )));
}

#[test]
fn periodic_spell_sound_table_covers_every_source_case() {
    assert_eq!(periodic_spell_sound(0x02), Some(12));
    assert_eq!(periodic_spell_sound(0x10), Some(39));
    assert_eq!(periodic_spell_sound(0x13), Some(43));
    assert_eq!(periodic_spell_sound(0x19), Some(37));
    assert_eq!(periodic_spell_sound(0), None);
    assert_eq!(periodic_spell_sound(u8::MAX), None);
}

#[test]
fn spell_fire_requests_preserve_source_order_and_precede_flight_skip() {
    let mut state = FlightState {
        skip_flight_once: true,
        ..FlightState::default()
    };
    let mut context = FlightContext {
        spell_fire: Some(SpellFireContext {
            left_holder_index: 11,
            right_holder_index: 12,
            selected_holder_index: 13,
            ..SpellFireContext::default()
        }),
        ..FlightContext::default()
    };

    let evidence = step_entity_pass_with_context(
        &mut state,
        LatchedCommand {
            flags: 0x70,
            ..LatchedCommand::default()
        },
        FlightProfile::OUTDOOR,
        &FLAT,
        &mut context,
    );

    assert_eq!(evidence.movement_resolution, "skipped");
    assert!(!state.skip_flight_once);
    assert_eq!(state.entity_phase, 1);
    assert_eq!(
        context.events,
        vec![
            FlightEvent::RequestSpellActivation {
                source: SpellFireSource::Left,
                holder_entity_index: 11,
                direction_mask: 256,
            },
            FlightEvent::RequestSpellActivation {
                source: SpellFireSource::Right,
                holder_entity_index: 12,
                direction_mask: 512,
            },
            FlightEvent::RequestSpellActivation {
                source: SpellFireSource::Selected,
                holder_entity_index: 13,
                direction_mask: 256,
            },
        ]
    );
}

#[test]
fn spell_fire_executes_synchronously_and_preserves_holder_aliasing() {
    let mut state = FlightState::default();
    let mut context = FlightContext {
        spell_fire: Some(SpellFireContext {
            left_holder_index: 11,
            right_holder_index: 11,
            caster: SpellCasterState {
                mana: 100,
                ..SpellCasterState::default()
            },
            holders: vec![SpellActivationHolder {
                entity_index: 11,
                model: 1,
                activation_value: 23,
                mana_cost: 50,
                ..SpellActivationHolder::default()
            }],
            ..SpellFireContext::default()
        }),
        ..FlightContext::default()
    };

    step_entity_pass_with_context(
        &mut state,
        LatchedCommand {
            flags: 0x30,
            ..LatchedCommand::default()
        },
        FlightProfile::OUTDOOR,
        &FLAT,
        &mut context,
    );

    let fire = context.spell_fire.as_ref().unwrap();
    assert_eq!(fire.holders[0].active_value, 23);
    assert_eq!(fire.holders[0].action_marker, 1);
    assert_eq!(fire.accepted_models, [1, 1]);
    assert_eq!(fire.caster.control_flags & 0x0300, 512);
    assert_eq!(
        context.events[..2],
        [
            FlightEvent::RequestSpellActivation {
                source: SpellFireSource::Left,
                holder_entity_index: 11,
                direction_mask: 256,
            },
            FlightEvent::RequestSpellActivation {
                source: SpellFireSource::Right,
                holder_entity_index: 11,
                direction_mask: 512,
            },
        ]
    );
}

#[test]
fn speed_spell_activation_updates_the_aliased_movement_projection() {
    let mut state = FlightState {
        speed_spell_holder_index: 42,
        ..FlightState::default()
    };
    let mut context = FlightContext {
        spell_fire: Some(SpellFireContext {
            selected_holder_index: 42,
            caster: SpellCasterState {
                mana: 1_000,
                ..SpellCasterState::default()
            },
            holders: vec![SpellActivationHolder {
                entity_index: 42,
                model: level1_mission::SPEED_SPELL_FAMILY,
                activation_value: level1_mission::SPEED_SPELL_ACTIVATION_TICKS,
                mana_cost: level1_mission::SPEED_SPELL_MANA_COST,
                ..SpellActivationHolder::default()
            }],
            ..SpellFireContext::default()
        }),
        speed_spell_holder: Some(SpellHolder {
            entity_index: 42,
            active_value: 0,
        }),
        ..FlightContext::default()
    };

    step_turn_with_context(
        &mut state,
        PlayerCommand {
            flags: 0x40,
            ..PlayerCommand::default()
        },
        FlightProfile::OUTDOOR,
        &FLAT,
        &mut context,
    );

    let fire = context.spell_fire.as_ref().expect("spell registry");
    assert_eq!(
        fire.holders[0].active_value,
        level1_mission::SPEED_SPELL_ACTIVATION_TICKS.cast_signed()
    );
    assert_eq!(
        context
            .speed_spell_holder
            .expect("movement projection")
            .active_value,
        fire.holders[0].active_value
    );
    assert_eq!(fire.accepted_models, [level1_mission::SPEED_SPELL_FAMILY]);
}

#[test]
fn speed_holder_alias_preserves_full_stop_and_cave_rejection_order() {
    fn context() -> FlightContext {
        FlightContext {
            spell_fire: Some(SpellFireContext {
                selected_holder_index: 42,
                caster: SpellCasterState {
                    mana: 1_000,
                    ..SpellCasterState::default()
                },
                holders: vec![SpellActivationHolder {
                    entity_index: 42,
                    model: level1_mission::SPEED_SPELL_FAMILY,
                    activation_value: level1_mission::SPEED_SPELL_ACTIVATION_TICKS,
                    mana_cost: level1_mission::SPEED_SPELL_MANA_COST,
                    ..SpellActivationHolder::default()
                }],
                ..SpellFireContext::default()
            }),
            speed_spell_holder: Some(SpellHolder {
                entity_index: 42,
                active_value: 0,
            }),
            ..FlightContext::default()
        }
    }

    let mut full_stop_state = FlightState {
        speed_spell_holder_index: 42,
        target_speed: 80,
        actual_speed: 80,
        ..FlightState::default()
    };
    let mut full_stop_context = context();
    step_turn_with_context(
        &mut full_stop_state,
        PlayerCommand {
            opcode: 0x27,
            argument_1: 1,
            flags: 0x40,
            ..PlayerCommand::default()
        },
        FlightProfile::OUTDOOR,
        &FLAT,
        &mut full_stop_context,
    );
    let expected = level1_mission::SPEED_SPELL_ACTIVATION_TICKS.cast_signed();
    assert_eq!(
        full_stop_context.spell_fire.as_ref().unwrap().holders[0].active_value,
        expected,
        "full-stop precedes the synchronous selected-spell cast"
    );
    assert_eq!(
        full_stop_context.speed_spell_holder.unwrap().active_value,
        expected
    );

    let mut cave_state = FlightState {
        speed_spell_holder_index: 42,
        target_speed: 64,
        actual_speed: 64,
        ..FlightState::default()
    };
    let mut cave_context = context();
    step_turn_with_context(
        &mut cave_state,
        PlayerCommand {
            flags: 0x40,
            ..PlayerCommand::default()
        },
        FlightProfile::CAVE,
        &BlockedCaveWorld,
        &mut cave_context,
    );
    assert_eq!(
        cave_context.spell_fire.as_ref().unwrap().holders[0].active_value,
        0,
        "the later rejected movement clears the source holder"
    );
    assert_eq!(cave_context.speed_spell_holder.unwrap().active_value, 0);
}

#[test]
fn repeated_low_tier_fireball_activation_is_not_lost_when_reset_value_matches() {
    let mut state = FlightState::default();
    let mut context = FlightContext {
        spell_fire: Some(SpellFireContext {
            left_holder_index: 11,
            caster: SpellCasterState {
                mana: 100,
                ..SpellCasterState::default()
            },
            holders: vec![SpellActivationHolder {
                entity_index: 11,
                model: 0,
                level_gate: 0,
                active_value: 23,
                activation_value: 23,
                mana_cost: 50,
                ..SpellActivationHolder::default()
            }],
            ..SpellFireContext::default()
        }),
        ..FlightContext::default()
    };

    step_turn_with_context(
        &mut state,
        PlayerCommand {
            flags: 0x10,
            ..PlayerCommand::default()
        },
        FlightProfile::OUTDOOR,
        &FLAT,
        &mut context,
    );

    let fire = context.spell_fire.as_ref().unwrap();
    assert_eq!(fire.holders[0].active_value, 23);
    assert_eq!(fire.accepted_models, [0]);
}

#[test]
fn movement_opcode_selects_spell_before_selected_fire_dispatch() {
    let mut state = FlightState::default();
    let mut enabled_holder_indices = [0_u16; 26];
    enabled_holder_indices[3] = 33;
    let mut context = FlightContext {
        spell_fire: Some(SpellFireContext {
            selected_holder_index: 99,
            enabled_holder_indices,
            caster: SpellCasterState {
                mana: 100,
                ..SpellCasterState::default()
            },
            holders: vec![SpellActivationHolder {
                entity_index: 33,
                activation_value: 7,
                mana_cost: 50,
                ..SpellActivationHolder::default()
            }],
            ..SpellFireContext::default()
        }),
        ..FlightContext::default()
    };

    step_turn_with_context(
        &mut state,
        PlayerCommand {
            opcode: 6,
            argument_1: 27,
            flags: 0x40,
            ..PlayerCommand::default()
        },
        FlightProfile::OUTDOOR,
        &FLAT,
        &mut context,
    );

    let fire = context.spell_fire.as_ref().unwrap();
    assert_eq!(SOURCE_SPELL_INDEX_MAP[27], 3);
    assert_eq!(fire.selected_spell_ui_index, 27);
    assert_eq!(fire.selected_holder_index, 33);
    assert_eq!(fire.holders[0].active_value, 7);
    assert_eq!(
        context.events.first(),
        Some(&FlightEvent::RequestSpellActivation {
            source: SpellFireSource::Selected,
            holder_entity_index: 33,
            direction_mask: 256,
        })
    );
}

fn spell_menu_assignment_fixture() -> (FlightState, FlightContext) {
    let mut enabled_holder_indices = [0_u16; 26];
    enabled_holder_indices[0] = 21;
    enabled_holder_indices[1] = 22;
    let mut spell_levels = [0_i8; 26];
    spell_levels[0] = 2;
    spell_levels[1] = 2;
    let mut family_selected_tiers = [0_i8; 26];
    family_selected_tiers[0] = 2;
    family_selected_tiers[1] = 1;
    let context = FlightContext {
        spell_fire: Some(SpellFireContext {
            left_holder_index: 21,
            right_holder_index: 22,
            selected_holder_index: 21,
            selected_subspell_index: 2,
            enabled_holder_indices,
            family_selected_tiers,
            spell_levels,
            holders: vec![
                SpellActivationHolder {
                    entity_index: 21,
                    activation_value: 1,
                    mana_cost: 1,
                    ..SpellActivationHolder::default()
                },
                SpellActivationHolder {
                    entity_index: 22,
                    model: 1,
                    activation_value: 1,
                    mana_cost: 1,
                    ..SpellActivationHolder::default()
                },
            ],
            caster: SpellCasterState {
                mana: 100,
                ..SpellCasterState::default()
            },
            ..SpellFireContext::default()
        }),
        ..FlightContext::default()
    };
    (FlightState::default(), context)
}

fn assert_family_press_routes_selected_fire() {
    let (mut state, mut context) = spell_menu_assignment_fixture();
    latch_command_with_context(
        &mut state,
        PlayerCommand {
            opcode: 40,
            argument_1: 1,
            argument_2: 1,
            ..PlayerCommand::default()
        },
        &mut context,
    );
    let fire = context.spell_fire.as_ref().unwrap();
    assert_eq!(
        (
            fire.selected_spell_ui_index,
            fire.selected_holder_index,
            fire.selected_subspell_index,
        ),
        (1, 22, 1),
        "family press synchronizes the selected holder before selected fire"
    );

    step_turn_with_context(
        &mut state,
        PlayerCommand {
            flags: 0x40,
            ..PlayerCommand::default()
        },
        FlightProfile::OUTDOOR,
        &FLAT,
        &mut context,
    );
    assert_eq!(
        context.events.first(),
        Some(&FlightEvent::RequestSpellActivation {
            source: SpellFireSource::Selected,
            holder_entity_index: 22,
            direction_mask: 256,
        })
    );
    assert_eq!(context.spell_fire.as_ref().unwrap().accepted_models, [1]);
}

fn assert_left_family_stage(state: &mut FlightState, context: &mut FlightContext) {
    let before_menu_hash = conformance_hash_v2(state, context);
    latch_command_with_context(
        state,
        PlayerCommand {
            opcode: 40,
            argument_1: 0,
            argument_2: 1,
            ..PlayerCommand::default()
        },
        context,
    );
    let fire = context.spell_fire.as_ref().unwrap();
    assert_eq!(
        (
            fire.selected_spell_ui_index,
            fire.selected_subspell_index,
            fire.spell_menu_stage,
        ),
        (0, 2, 1),
        "family press copies its current tier and enters left stage"
    );
    assert_ne!(
        conformance_hash_v2(state, context),
        before_menu_hash,
        "non-default menu stage/current tiers affect the live conformance hash"
    );
    latch_command_with_context(
        state,
        PlayerCommand {
            opcode: 41,
            argument_1: 7,
            ..PlayerCommand::default()
        },
        context,
    );
    latch_command_with_context(
        state,
        PlayerCommand {
            opcode: 31,
            argument_1: 0,
            argument_2: 2,
            ..PlayerCommand::default()
        },
        context,
    );
    let fire = context.spell_fire.as_ref().unwrap();
    assert_eq!(fire.left_holder_index, 21);
    assert_eq!(fire.right_holder_index, 22);
    assert_eq!(fire.left_subspell_index, 2);
    assert_eq!(fire.right_subspell_index, 0);
    assert_eq!(fire.selected_holder_index, 21);
    assert_eq!(fire.selected_spell_ui_index, 0);
    assert_eq!(
        fire.selected_subspell_index, 7,
        "opcode 31 reconfigures the physical hand holder but does not overwrite opcode 41's raw menu selector"
    );
    assert_eq!(fire.spell_menu_stage, 0);
    assert_eq!(fire.family_selected_tiers[0], 2);
    assert_eq!(fire.holders[0].level_gate, 2);
    assert_eq!(
        context.events,
        vec![FlightEvent::PrepareSound {
            audio_site: MissionAudioSite::SpellMenuAssignment,
            entity_index: 0,
            player_index: 0,
            sound_id: 14,
        }]
    );
}

fn assert_right_family_stage(state: &mut FlightState, context: &mut FlightContext) {
    latch_command_with_context(
        state,
        PlayerCommand {
            opcode: 40,
            argument_1: 1,
            argument_2: 2,
            ..PlayerCommand::default()
        },
        context,
    );
    assert_eq!(
        {
            let fire = context.spell_fire.as_ref().unwrap();
            (
                fire.selected_spell_ui_index,
                fire.selected_subspell_index,
                fire.spell_menu_stage,
            )
        },
        (1, 1, 2),
        "right press mirrors the current-family warp and stage"
    );
    latch_command_with_context(
        state,
        PlayerCommand {
            opcode: 41,
            argument_1: 2,
            ..PlayerCommand::default()
        },
        context,
    );
    latch_command_with_context(
        state,
        PlayerCommand {
            opcode: 32,
            argument_1: 1,
            argument_2: 2,
            ..PlayerCommand::default()
        },
        context,
    );
    let fire = context.spell_fire.as_ref().unwrap();
    assert_eq!(fire.left_holder_index, 21);
    assert_eq!(fire.right_holder_index, 22);
    assert_eq!(fire.left_subspell_index, 2);
    assert_eq!(fire.right_subspell_index, 2);
    assert_eq!(fire.selected_holder_index, 22);
    assert_eq!(fire.selected_spell_ui_index, 1);
    assert_eq!(fire.selected_subspell_index, 2);
    assert_eq!(context.events.len(), 2);
}

fn assert_raw_tier_no_op_and_eligibility(state: &mut FlightState, context: &mut FlightContext) {
    latch_command_with_context(
        state,
        PlayerCommand {
            opcode: 40,
            argument_1: 0,
            argument_2: 1,
            ..PlayerCommand::default()
        },
        context,
    );
    latch_command_with_context(
        state,
        PlayerCommand {
            opcode: 31,
            argument_1: 1,
            argument_2: 7,
            ..PlayerCommand::default()
        },
        context,
    );
    let fire = context.spell_fire.as_ref().unwrap();
    assert_eq!(fire.family_selected_tiers[1], 7);
    assert_eq!(fire.left_subspell_index, 7);
    assert_eq!(fire.holders[1].level_gate, 2);
    assert_eq!(
        (
            fire.selected_spell_ui_index,
            fire.selected_subspell_index,
            fire.selected_holder_index,
        ),
        (0, 2, 21),
        "final assignment does not mutate the selected cursor"
    );

    let before_no_op = context.clone();
    latch_command_with_context(
        state,
        PlayerCommand {
            opcode: 32,
            argument_1: -1,
            argument_2: 7,
            ..PlayerCommand::default()
        },
        context,
    );
    assert_eq!(&*context, &before_no_op, "family -1 is a strict no-op");

    context.events.clear();
    let before_eligibility_hash = conformance_hash_v2(state, context);
    latch_command_with_context(
        state,
        PlayerCommand {
            opcode: 38,
            argument_1: 1,
            argument_2: 1,
            ..PlayerCommand::default()
        },
        context,
    );
    let fire = context.spell_fire.as_ref().unwrap();
    assert_eq!(fire.spell_hand_eligibility[1], 1);
    assert_ne!(conformance_hash_v2(state, context), before_eligibility_hash);
    assert_eq!(
        context.events,
        vec![FlightEvent::PrepareSound {
            audio_site: MissionAudioSite::SpellMenuAssignment,
            entity_index: 0,
            player_index: 0,
            sound_id: 14,
        }]
    );
    latch_command_with_context(
        state,
        PlayerCommand {
            opcode: 38,
            argument_1: 1,
            argument_2: 0,
            ..PlayerCommand::default()
        },
        context,
    );
    assert_eq!(
        context.spell_fire.as_ref().unwrap().spell_hand_eligibility[1],
        0
    );
    context.events.clear();
    assert_eq!(
        conformance_hash_v2(state, context),
        before_eligibility_hash,
        "source-equivalent eligibility state restores the same hash"
    );
}

fn assert_spell_menu_cast_routes(state: &mut FlightState, context: &mut FlightContext) {
    latch_command_with_context(
        state,
        PlayerCommand {
            opcode: 31,
            argument_1: 1,
            argument_2: 0,
            ..PlayerCommand::default()
        },
        context,
    );
    latch_command_with_context(
        state,
        PlayerCommand {
            opcode: 32,
            argument_1: 0,
            argument_2: 0,
            ..PlayerCommand::default()
        },
        context,
    );
    context.events.clear();
    step_turn_with_context(
        state,
        PlayerCommand {
            flags: 0x10,
            ..PlayerCommand::default()
        },
        FlightProfile::OUTDOOR,
        &FLAT,
        context,
    );
    assert_eq!(
        context.events.first(),
        Some(&FlightEvent::RequestSpellActivation {
            source: SpellFireSource::Left,
            holder_entity_index: 22,
            direction_mask: 256,
        })
    );
    assert_eq!(
        context.spell_fire.as_ref().unwrap().caster.control_flags & 0x0300,
        256
    );
    {
        let fire = context.spell_fire.as_mut().unwrap();
        fire.holders[0].active_value = 0;
        fire.holders[1].active_value = 0;
    }
    context.events.clear();
    step_turn_with_context(
        state,
        PlayerCommand {
            flags: 0x20,
            ..PlayerCommand::default()
        },
        FlightProfile::OUTDOOR,
        &FLAT,
        context,
    );
    assert_eq!(
        context.events.first(),
        Some(&FlightEvent::RequestSpellActivation {
            source: SpellFireSource::Right,
            holder_entity_index: 21,
            direction_mask: 512,
        })
    );
    assert_eq!(
        context.spell_fire.as_ref().unwrap().caster.control_flags & 0x0300,
        512
    );
}

#[test]
fn spell_menu_assignment_opcodes_bind_independent_hands_and_clamp_tier() {
    assert_family_press_routes_selected_fire();
    let (mut state, mut context) = spell_menu_assignment_fixture();
    assert_left_family_stage(&mut state, &mut context);
    assert_right_family_stage(&mut state, &mut context);
    assert_raw_tier_no_op_and_eligibility(&mut state, &mut context);
    assert_spell_menu_cast_routes(&mut state, &mut context);
}

#[test]
fn player_toolbar_and_chat_commands_emit_their_distinct_local_sound_sites() {
    let (mut state, mut context) = spell_menu_assignment_fixture();
    latch_command_with_context(
        &mut state,
        PlayerCommand {
            opcode: 0x17,
            argument_1: 2,
            argument_2: 7,
            ..PlayerCommand::default()
        },
        &mut context,
    );
    latch_command_with_context(
        &mut state,
        PlayerCommand {
            opcode: 0x17,
            argument_1: 3,
            argument_2: 7,
            ..PlayerCommand::default()
        },
        &mut context,
    );
    let fire = context.spell_fire.as_ref().unwrap();
    assert_eq!(
        (fire.spell_toolbar_slots[2], fire.spell_toolbar_slots[3]),
        (u8::MAX, 7)
    );
    assert!(context.events.iter().all(|event| matches!(
        event,
        FlightEvent::PrepareSound {
            audio_site: MissionAudioSite::PlayerToolbarSlotWrite,
            sound_id: 14,
            ..
        }
    )));

    context.events.clear();
    latch_command_with_context(
        &mut state,
        PlayerCommand {
            opcode: 0x2c,
            ..PlayerCommand::default()
        },
        &mut context,
    );
    assert!(context.chat_clear_requested);
    assert_eq!(
        context.events,
        [FlightEvent::PrepareSound {
            audio_site: MissionAudioSite::PlayerChatClear,
            entity_index: 0,
            player_index: 0,
            sound_id: 14,
        }]
    );
}

#[test]
fn same_family_assignments_keep_hand_tiers_but_reconfigure_shared_holder() {
    let mut state = FlightState::default();
    let mut enabled_holder_indices = [0_u16; 26];
    enabled_holder_indices[1] = 22;
    let mut spell_levels = [0_i8; 26];
    spell_levels[1] = 2;
    let mut family_selected_tiers = [0_i8; 26];
    family_selected_tiers[1] = 2;
    let mut context = FlightContext {
        spell_fire: Some(SpellFireContext {
            left_holder_index: 22,
            right_holder_index: 22,
            enabled_holder_indices,
            family_selected_tiers,
            spell_levels,
            holders: vec![SpellActivationHolder {
                entity_index: 22,
                model: 1,
                ..SpellActivationHolder::default()
            }],
            ..SpellFireContext::default()
        }),
        ..FlightContext::default()
    };

    latch_command_with_context(
        &mut state,
        PlayerCommand {
            opcode: 40,
            argument_1: 1,
            argument_2: 1,
            ..PlayerCommand::default()
        },
        &mut context,
    );
    latch_command_with_context(
        &mut state,
        PlayerCommand {
            opcode: 41,
            argument_1: 2,
            ..PlayerCommand::default()
        },
        &mut context,
    );
    latch_command_with_context(
        &mut state,
        PlayerCommand {
            opcode: 31,
            argument_1: 1,
            argument_2: 2,
            ..PlayerCommand::default()
        },
        &mut context,
    );
    latch_command_with_context(
        &mut state,
        PlayerCommand {
            opcode: 40,
            argument_1: 1,
            argument_2: 2,
            ..PlayerCommand::default()
        },
        &mut context,
    );
    latch_command_with_context(
        &mut state,
        PlayerCommand {
            opcode: 41,
            argument_1: 1,
            ..PlayerCommand::default()
        },
        &mut context,
    );
    latch_command_with_context(
        &mut state,
        PlayerCommand {
            opcode: 32,
            argument_1: 1,
            argument_2: 1,
            ..PlayerCommand::default()
        },
        &mut context,
    );

    let fire = context.spell_fire.as_ref().unwrap();
    assert_eq!(fire.left_subspell_index, 2);
    assert_eq!(fire.right_subspell_index, 1);
    assert_eq!(fire.selected_subspell_index, 1);
    assert_eq!(fire.family_selected_tiers[1], 1);
    assert_eq!(fire.holders[0].level_gate, 1);
    assert_eq!(fire.spell_menu_stage, 0);
}

#[test]
fn movement_opcode_preserves_selection_when_selected_fire_flag_is_clear() {
    let mut state = FlightState::default();
    let mut enabled_holder_indices = [0_u16; 26];
    enabled_holder_indices[3] = 33;
    let mut context = FlightContext {
        spell_fire: Some(SpellFireContext {
            selected_holder_index: 99,
            enabled_holder_indices,
            caster: SpellCasterState {
                mana: 100,
                ..SpellCasterState::default()
            },
            holders: vec![SpellActivationHolder {
                entity_index: 33,
                activation_value: 7,
                mana_cost: 50,
                ..SpellActivationHolder::default()
            }],
            ..SpellFireContext::default()
        }),
        ..FlightContext::default()
    };

    step_turn_with_context(
        &mut state,
        PlayerCommand {
            opcode: 6,
            argument_1: 27,
            flags: 0,
            ..PlayerCommand::default()
        },
        FlightProfile::OUTDOOR,
        &FLAT,
        &mut context,
    );

    let fire = context.spell_fire.as_ref().unwrap();
    assert_eq!(fire.selected_spell_ui_index, 0);
    assert_eq!(fire.selected_holder_index, 99);
    assert_eq!(fire.holders[0].active_value, 0);
    assert!(context.events.is_empty());
}

#[test]
fn recoil_decay_zeroes_the_sub_step_remainder() {
    assert_eq!(decay_recoil(8), 4);
    assert_eq!(decay_recoil(5), 0);
    assert_eq!(decay_recoil(-5), 0);
    assert_eq!(decay_recoil(0), 0);
}

#[test]
fn forward_strafe_and_capped_recoil_displace_in_source_order() {
    let mut state = FlightState {
        position: Position {
            x: 1_000,
            y: 1_000,
            z: 1_000,
        },
        target_speed: 16,
        strafe_speed: 16,
        recoil_speed: 200,
        recoil_yaw: 1_024,
        ..FlightState::default()
    };
    let evidence = step_entity_pass(
        &mut state,
        LatchedCommand::default(),
        FlightProfile::OUTDOOR,
        &FLAT,
    );

    assert_eq!(
        evidence.proposed,
        Position {
            // sub_5F380 decays the existing strafe value from 16 to 12
            // before sub_5D530 applies translation.
            x: 1_012,
            y: 1_112,
            z: 1_000,
        }
    );
    assert_eq!(state.recoil_speed, 124);
    assert_eq!(state.position.z, 984);
}

#[test]
fn trig_table_and_angle_wrap_preserve_cardinals() {
    assert_eq!(SIN_16_16[0], 0);
    assert_eq!(SIN_16_16[512], 65_536);
    assert_eq!(SIN_16_16[1_024], 0);
    assert_eq!(SIN_16_16[1_536], -65_536);
    assert_eq!(LEGACY_ATAN[0], 0);
    assert_eq!(LEGACY_ATAN[256], 256);
    assert_eq!(wrap_angle(-1), 2_047);
    assert_eq!(wrap_angle(2_048), 0);
}

#[test]
fn recovered_tangent_helper_preserves_quadrants_and_diagonals() {
    assert_eq!(legacy_atan(0, 0), 0);
    assert_eq!(legacy_atan(0, -1), 0);
    assert_eq!(legacy_atan(1, -1), 256);
    assert_eq!(legacy_atan(1, 0), 512);
    assert_eq!(legacy_atan(1, 1), 768);
    assert_eq!(legacy_atan(0, 1), 1_024);
    assert_eq!(legacy_atan(-1, 1), 1_280);
    assert_eq!(legacy_atan(-1, 0), 1_536);
    assert_eq!(legacy_atan(-1, -1), 1_792);
    assert_eq!(legacy_atan(i16::MIN, 0), 1_536);
}

#[test]
fn movement_redirection_angles_use_the_recovered_helper() {
    assert_eq!(heading_for_delta(0, -20), 0);
    assert_eq!(heading_for_delta(20, -20), 256);
    assert_eq!(heading_for_delta(20, 0), 512);
    assert_eq!(pitch_for_delta(100, 0), 0);
    assert_eq!(pitch_for_delta(100, 100), 1_792);
    assert_eq!(pitch_for_delta(100, -100), 256);
}

#[test]
fn recovered_seeded_square_root_is_floor_exact_across_the_u32_domain() {
    let assert_floor = |value: u32| {
        let root = legacy_sqrt(value);
        assert!(u64::from(root) * u64::from(root) <= u64::from(value));
        assert!(
            u64::from(root + 1) * u64::from(root + 1) > u64::from(value),
            "root {root} was not maximal for {value}"
        );
    };
    for value in 0..=65_536 {
        assert_floor(value);
    }
    let mut value = 0x5eed_1234_u32;
    for _ in 0..100_000 {
        value = value.wrapping_mul(9_377).wrapping_add(9_439);
        assert_floor(value);
    }
    assert_eq!(
        legacy_distance_3d(
            i32::from(i16::MIN),
            i32::from(i16::MIN),
            i32::from(i16::MIN)
        ),
        56_755
    );
}

#[test]
fn actual_speed_moves_by_exact_steps_and_can_overshoot() {
    assert_eq!(step_speed_without_target_clamp(0, 10, SPEED_STEP), 16);
    assert_eq!(step_speed_without_target_clamp(16, 10, SPEED_STEP), 0);
    assert_eq!(step_speed_without_target_clamp(-5, -10, SPEED_STEP), -21);
}

#[test]
fn impulses_are_one_tick_and_ground_and_ceiling_clamp() {
    let cave = FlatWorld {
        ground: 100,
        ceiling: Some(1_000),
    };
    let mut state = FlightState {
        position: Position { x: 0, y: 0, z: 500 },
        impulse_x: 10,
        impulse_y: -20,
        impulse_z: 1_000,
        ..FlightState::default()
    };
    step_turn(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::CAVE,
        &cave,
    );
    assert_eq!(state.impulse_x, 0);
    assert_eq!(state.impulse_y, 0);
    assert_eq!(state.impulse_z, 0);
    assert_eq!(state.position.z, 616);
}

#[test]
fn cave_ground_repair_does_not_apply_ceiling_clamp_in_the_same_pass() {
    let narrow_cave = FlatWorld {
        ground: 0,
        ceiling: Some(100),
    };
    let mut state = FlightState {
        position: Position { x: 0, y: 0, z: 0 },
        // This unusual but source-valid footprint lets moveTest accept the
        // narrow column so the post-accept clamp ordering is observable.
        entity_fov: -1_000,
        ..FlightState::default()
    };

    step_turn_with_context(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::CAVE,
        &narrow_cave,
        &mut FlightContext::default(),
    );

    assert_eq!(state.position.z, FlightProfile::CAVE.ground_clearance);
}

#[test]
fn rejected_movement_on_a_forbidden_cave_tile_recovers_and_stops_target() {
    let mut state = FlightState {
        target_speed: 80,
        actual_speed: 64,
        ..FlightState::default()
    };
    let before = state.position;
    let evidence = step_turn(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::CAVE,
        &BlockedCaveWorld,
    );
    assert_eq!(evidence.movement_resolution, "recovered-after-reject");
    assert_eq!(
        state.position,
        Position {
            y: before.y.wrapping_sub(128),
            ..before
        }
    );
    assert_eq!(state.target_speed, 0);
    assert!(state.recovery_latch);
}

#[test]
fn cave_rejection_disables_the_projected_speed_spell_holder() {
    let mut state = FlightState {
        target_speed: 80,
        actual_speed: 64,
        speed_spell_holder_index: 42,
        ..FlightState::default()
    };
    let mut context = FlightContext {
        spell_fire: Some(SpellFireContext {
            holders: vec![SpellActivationHolder {
                entity_index: 42,
                model: level1_mission::SPEED_SPELL_FAMILY,
                active_value: 17,
                ..SpellActivationHolder::default()
            }],
            ..SpellFireContext::default()
        }),
        speed_spell_holder: Some(SpellHolder {
            entity_index: 42,
            active_value: 17,
        }),
        ..FlightContext::default()
    };

    step_turn_with_context(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::CAVE,
        &BlockedCaveWorld,
        &mut context,
    );

    assert_eq!(state.target_speed, 0);
    assert_eq!(
        context
            .speed_spell_holder
            .expect("projected holder remains present")
            .active_value,
        0
    );
    assert_eq!(context.spell_fire.unwrap().holders[0].active_value, 0);
}

#[test]
fn water_boundary_slides_along_a_dry_quantized_direction() {
    let world = WaterEdgeWorld { water_x: 1_050 };
    let mut state = FlightState::default();
    let from = Position {
        x: 1_000,
        y: 1_000,
        z: 1_000,
    };
    let proposed = Position {
        x: 1_100,
        y: 1_000,
        z: 1_000,
    };
    let result =
        resolve_legacy_movement(&mut state, from, proposed, FlightProfile::OUTDOOR, &world);
    let MovementResolution::Redirect(position) = result else {
        panic!("water entry should redirect along the dry boundary");
    };
    assert_ne!(world.material_mask(position), MOVEMENT_REDIRECT_MASK);
    assert_eq!(state.water_counter, 1);
}

#[test]
fn material_redirection_accepts_the_first_quantized_slide() {
    let from = Position {
        x: 1_000,
        y: 1_000,
        z: 1_000,
    };
    let proposed = Position {
        x: 1_100,
        y: 1_100,
        z: 1_000,
    };
    let world = PointRedirectWorld { blocked: proposed };
    let mut state = FlightState::default();
    let MovementResolution::Redirect(position) =
        resolve_legacy_movement(&mut state, from, proposed, FlightProfile::OUTDOOR, &world)
    else {
        panic!("first quantized direction should leave the blocked point");
    };
    assert_ne!(position, proposed);
    assert_eq!(world.material_mask(position), 0);
    assert_eq!(state.water_counter, 1);
}

#[test]
fn cave_material_failure_rejects_even_when_clearance_is_open() {
    let from = Position {
        x: 1_000,
        y: 1_000,
        z: 1_000,
    };
    let proposed = Position { x: 1_100, ..from };
    let mut state = FlightState::default();
    assert_eq!(
        resolve_legacy_movement(
            &mut state,
            from,
            proposed,
            FlightProfile::CAVE,
            &UniformCaveRedirectWorld,
        ),
        MovementResolution::RejectAndStop
    );
}

#[test]
fn blocked_cave_candidate_rejects_and_slope_clamps() {
    let mut state = FlightState::default();
    let from = state.position;
    let proposed = Position {
        x: from.x.wrapping_add(32),
        ..from
    };
    assert_eq!(
        resolve_legacy_movement(
            &mut state,
            from,
            proposed,
            FlightProfile::CAVE,
            &BlockedCaveWorld,
        ),
        MovementResolution::RejectAndStop
    );

    state.position = Position {
        x: 1_000,
        y: 1_000,
        z: 0,
    };
    step_turn(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::OUTDOOR,
        &SlopedWorld,
    );
    assert_eq!(
        state.position.z,
        SlopedWorld
            .ground_height(state.position)
            .saturating_add(FlightProfile::OUTDOOR.ground_clearance)
    );
}

#[test]
fn material_identity_is_not_collapsed_into_movement_redirection() {
    let from = Position {
        x: 1_000,
        y: 1_000,
        z: 1_000,
    };
    let proposed = Position { x: 1_100, ..from };
    let mut state = FlightState::default();
    let material_one = UniformMaterialWorld {
        material: WATER_AMBIENCE_MASK,
        ground: 0,
    };
    assert_eq!(
        resolve_legacy_movement(
            &mut state,
            from,
            proposed,
            FlightProfile::OUTDOOR,
            &material_one,
        ),
        MovementResolution::Accept(proposed)
    );
    assert_eq!(state.water_counter, 0);

    let redirect_class = UniformMaterialWorld {
        material: MOVEMENT_REDIRECT_MASK,
        ground: 0,
    };
    assert_eq!(
        resolve_legacy_movement(
            &mut state,
            from,
            proposed,
            FlightProfile::OUTDOOR,
            &redirect_class,
        ),
        MovementResolution::Reject
    );
    assert_eq!(state.water_counter, 1);
}

#[test]
fn outdoor_redirect_failure_recovers_without_stopping_or_post_accept_clamps() {
    let world = UniformMaterialWorld {
        material: MOVEMENT_REDIRECT_MASK,
        ground: 500,
    };
    let mut state = FlightState {
        position: Position {
            x: 1_000,
            y: 1_000,
            z: 0,
        },
        target_speed: 80,
        actual_speed: 64,
        ..FlightState::default()
    };
    let before = state.position;
    let evidence = step_turn(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::OUTDOOR,
        &world,
    );
    assert_eq!(evidence.movement_resolution, "recovered-after-reject");
    assert_eq!(evidence.proposed.z, before.z);
    assert_eq!(
        state.position,
        Position {
            y: before.y.wrapping_sub(128),
            ..before
        }
    );
    assert_eq!(state.target_speed, 80);
    assert_eq!(state.position.z, 0);
    assert!(state.recovery_latch);
}

#[test]
fn recovery_branch_uses_current_yaw_previous_latch_and_exact_clear_rules() {
    let before = Position {
        x: 1_000,
        y: 1_000,
        z: 500,
    };
    let material = UniformMaterialWorld {
        material: MOVEMENT_REDIRECT_MASK,
        ground: 0,
    };
    let mut material_state = FlightState {
        position: before,
        yaw: 512,
        ..FlightState::default()
    };
    let evidence = reject_step(
        &mut material_state,
        before,
        before,
        false,
        FlightProfile::OUTDOOR,
        &material,
        &mut FlightContext::default(),
    );
    assert_eq!(evidence.movement_resolution, "recovered-after-reject");
    assert_eq!(
        material_state.position,
        Position {
            x: before.x.wrapping_add(128),
            ..before
        }
    );
    assert!(material_state.recovery_latch);

    let overlap = FlatWorld {
        ground: 0,
        ceiling: Some(639),
    };
    let mut latched_state = FlightState {
        position: before,
        recovery_latch: true,
        ..FlightState::default()
    };
    let evidence = reject_step(
        &mut latched_state,
        before,
        before,
        true,
        FlightProfile::CAVE,
        &overlap,
        &mut FlightContext::default(),
    );
    assert_eq!(evidence.movement_resolution, "recovered-after-reject");
    assert!(latched_state.recovery_latch);
    assert_eq!(latched_state.target_speed, 0);

    let clear = FlatWorld {
        ground: 0,
        ceiling: Some(2_000),
    };
    let mut clear_state = FlightState {
        position: before,
        recovery_latch: true,
        ..FlightState::default()
    };
    let evidence = reject_step(
        &mut clear_state,
        before,
        before,
        true,
        FlightProfile::CAVE,
        &clear,
        &mut FlightContext::default(),
    );
    assert_eq!(evidence.movement_resolution, "rejected-and-stopped");
    assert_eq!(clear_state.position, before);
    assert!(!clear_state.recovery_latch);
}

#[test]
fn active_tether_turns_pulls_and_uses_the_source_speed_limit() {
    let before = Position {
        x: 1_000,
        y: 1_000,
        z: 1_000,
    };
    let mut proposed = before;
    let mut state = FlightState {
        position: before,
        tether_prior_distance: 1_024,
        tether_target_index: 7,
        tether_holder_index: 9,
        tether_tier: 0,
        ..FlightState::default()
    };
    let target = TetherTarget {
        entity_index: 7,
        position: Position {
            x: before.x.wrapping_add(2_048),
            ..before
        },
        life: 100,
        mana: 100,
        mana_regen: 5,
        life_regen: 3,
    };
    let mut context = TetherContext {
        holder: Some(SpellHolder {
            entity_index: 9,
            active_value: 1,
        }),
        target: Some(target),
    };

    apply_tether(&mut state, before, &mut proposed, &mut context);

    assert_eq!(state.yaw, 0x82);
    assert_eq!(
        proposed,
        Position {
            x: before.x.wrapping_add(TETHER_SPEED_LIMIT.cast_unsigned()),
            ..before
        }
    );
    assert_eq!(context.target, Some(target));
    assert_eq!(state.tether_target_index, 7);

    state.tether_prior_distance = 10_000;
    proposed = before;
    apply_tether(&mut state, before, &mut proposed, &mut context);
    assert_eq!(
        proposed,
        Position {
            x: before.x.wrapping_sub(TETHER_SPEED_LIMIT.cast_unsigned()),
            ..before
        }
    );
}

#[test]
fn tether_tiers_apply_exact_mana_and_life_mutations() {
    let before = Position {
        x: 1_000,
        y: 1_000,
        z: 1_000,
    };
    for (tier, expected_life) in [(1_u16, 100_i32), (2, 95)] {
        let mut state = FlightState {
            position: before,
            tether_prior_distance: 1_000,
            tether_target_index: 7,
            tether_holder_index: 9,
            tether_tier: tier,
            ..FlightState::default()
        };
        let mut context = TetherContext {
            holder: Some(SpellHolder {
                entity_index: 9,
                active_value: 1,
            }),
            target: Some(TetherTarget {
                entity_index: 7,
                position: Position {
                    y: before.y.wrapping_sub(1_000),
                    ..before
                },
                life: 100,
                mana: 10,
                mana_regen: 5,
                life_regen: 3,
            }),
        };
        let mut proposed = before;

        apply_tether(&mut state, before, &mut proposed, &mut context);

        let target = context.target.expect("valid target remains projected");
        assert_eq!(target.life, expected_life);
        assert_eq!(target.mana, 0);
        assert_eq!(proposed, before);
    }
}

#[test]
fn tether_clears_only_after_a_valid_target_fails_active_conditions() {
    let before = Position {
        x: 1_000,
        y: 1_000,
        z: 1_000,
    };
    let mut missing_state = FlightState {
        position: before,
        tether_target_index: 7,
        tether_holder_index: 9,
        ..FlightState::default()
    };
    let mut missing_proposed = before;
    apply_tether(
        &mut missing_state,
        before,
        &mut missing_proposed,
        &mut TetherContext::default(),
    );
    assert_eq!(missing_state.tether_target_index, 7);

    let mut inactive_state = missing_state;
    let mut context = TetherContext {
        holder: Some(SpellHolder {
            entity_index: 9,
            active_value: 0,
        }),
        target: Some(TetherTarget {
            entity_index: 7,
            position: before,
            life: 100,
            ..TetherTarget::default()
        }),
    };
    let mut inactive_proposed = before;
    apply_tether(
        &mut inactive_state,
        before,
        &mut inactive_proposed,
        &mut context,
    );
    assert_eq!(inactive_state.tether_target_index, 0);
}

#[test]
fn tether_range_boundary_and_dead_target_clear_the_target() {
    let before = Position {
        x: 1_000,
        y: 1_000,
        z: 1_000,
    };
    for (distance, life) in [(0x1432_u16, 100_i32), (100, -1)] {
        let mut state = FlightState {
            position: before,
            tether_target_index: 7,
            tether_holder_index: 9,
            tether_tier: 0,
            ..FlightState::default()
        };
        let mut proposed = before;
        let mut context = TetherContext {
            holder: Some(SpellHolder {
                entity_index: 9,
                active_value: 1,
            }),
            target: Some(TetherTarget {
                entity_index: 7,
                position: Position {
                    x: before.x.wrapping_add(distance),
                    ..before
                },
                life,
                ..TetherTarget::default()
            }),
        };
        apply_tether(&mut state, before, &mut proposed, &mut context);
        assert_eq!(
            state.tether_target_index, 0,
            "distance {distance}, life {life}"
        );
        assert_eq!(proposed, before);
    }
}

#[test]
fn tether_turn_direction_preserves_wrap_and_half_turn_ties() {
    assert_eq!(turn_toward(2_000, 20, 130), 68);
    assert_eq!(turn_toward(20, 2_000, 130), -68);
    assert_eq!(turn_toward(0, 1_024, 130), 130);
    assert_eq!(turn_toward(1_024, 0, 130), -130);
}

#[test]
fn terrain_relative_pitch_preserves_direction_and_surface_shaping() {
    let mut state = FlightState {
        position: Position {
            x: 0,
            y: 0,
            z: 1_024,
        },
        pitch_accumulator: 128,
        actual_speed: 16,
        ..FlightState::default()
    };
    update_terrain_relative_pitch(&mut state, FlightProfile::OUTDOOR, &FLAT);
    assert_eq!(state.display_pitch, 128);
    assert_eq!(state.effective_pitch, 128);

    state.actual_speed = -16;
    update_terrain_relative_pitch(&mut state, FlightProfile::OUTDOOR, &FLAT);
    assert_eq!(state.effective_pitch, 0);

    state.position.z = 2_048;
    update_terrain_relative_pitch(&mut state, FlightProfile::OUTDOOR, &FLAT);
    assert_eq!(state.effective_pitch, 1_920);

    state.pitch_accumulator = -128;
    update_terrain_relative_pitch(&mut state, FlightProfile::OUTDOOR, &FLAT);
    assert_eq!(state.display_pitch, 1_920);
    assert_eq!(state.effective_pitch, 1_920);
}

#[test]
fn terrain_relative_pitch_keeps_boundary_and_zero_input_state() {
    let mut state = FlightState {
        pitch_accumulator: 1_024,
        actual_speed: 16,
        effective_pitch: 77,
        ..FlightState::default()
    };
    update_terrain_relative_pitch(&mut state, FlightProfile::OUTDOOR, &FLAT);
    assert_eq!(state.display_pitch, 1_024);
    assert_eq!(state.effective_pitch, 1_024);

    state.actual_speed = 0;
    state.pitch_accumulator = 0;
    update_terrain_relative_pitch(&mut state, FlightProfile::OUTDOOR, &FLAT);
    assert_eq!(state.display_pitch, 0);
    assert_eq!(state.effective_pitch, 1_024);
}

#[test]
fn speed_steps_before_pitch_shaping_on_the_first_acceleration_tick() {
    let mut state = FlightState {
        pitch_accumulator: 128,
        target_speed: 16,
        actual_speed: 0,
        ..FlightState::default()
    };
    step_turn(
        &mut state,
        PlayerCommand {
            pitch: 64,
            ..PlayerCommand::default()
        },
        FlightProfile::OUTDOOR,
        &FLAT,
    );
    assert_eq!(state.actual_speed, 16);
    assert_eq!(state.effective_pitch, 128);
}

#[test]
fn material_contact_counter_decays_before_contact_refresh() {
    let mut dry_state = FlightState {
        water_counter: 1,
        ..FlightState::default()
    };
    step_turn(
        &mut dry_state,
        PlayerCommand::default(),
        FlightProfile::OUTDOOR,
        &FLAT,
    );
    assert_eq!(dry_state.water_counter, 0);

    let redirect_world = UniformMaterialWorld {
        material: MOVEMENT_REDIRECT_MASK,
        ground: 0,
    };
    let mut contact_state = FlightState {
        water_counter: 1,
        ..FlightState::default()
    };
    step_turn(
        &mut contact_state,
        PlayerCommand::default(),
        FlightProfile::OUTDOOR,
        &redirect_world,
    );
    assert_eq!(contact_state.water_counter, 1);
}

#[test]
fn cave_overlap_boundary_is_strict() {
    let equality = FlatWorld {
        ground: 0,
        ceiling: Some(640),
    };
    let state = FlightState::default();
    assert!(!cave_overlap(
        &equality,
        Position::default(),
        FlightProfile::CAVE,
        &state
    ));

    let one_unit_tighter = FlatWorld {
        ceiling: Some(639),
        ..equality
    };
    assert!(cave_overlap(
        &one_unit_tighter,
        Position::default(),
        FlightProfile::CAVE,
        &state
    ));
}

#[test]
fn open_cave_accepts_and_side_probe_steering_has_exact_yaw_sign() {
    let open = FlatWorld {
        ground: 0,
        ceiling: Some(2_000),
    };
    let from = Position {
        x: 1_000,
        y: 1_000,
        z: 1_000,
    };
    let proposed = Position { x: 1_100, ..from };
    let mut state = FlightState::default();
    assert_eq!(
        resolve_legacy_movement(&mut state, from, proposed, FlightProfile::CAVE, &open),
        MovementResolution::Accept(proposed)
    );

    let MovementResolution::Redirect(left) = resolve_legacy_movement(
        &mut state,
        from,
        proposed,
        FlightProfile::CAVE,
        &SteeringCaveWorld { favor_left: true },
    ) else {
        panic!("larger clear gap on the left should steer left");
    };
    assert!(left.y < proposed.y);
    assert_eq!(state.yaw, 2_046);

    state.yaw = 0;
    let MovementResolution::Redirect(right) = resolve_legacy_movement(
        &mut state,
        from,
        proposed,
        FlightProfile::CAVE,
        &SteeringCaveWorld { favor_left: false },
    ) else {
        panic!("larger clear gap on the right should steer right");
    };
    assert!(right.y > proposed.y);
    assert_eq!(state.yaw, 2);
}

#[test]
fn cave_equal_side_gaps_accept_when_the_candidate_does_not_overlap() {
    let marginal = FlatWorld {
        ground: 0,
        ceiling: Some(800),
    };
    let from = Position {
        x: 1_000,
        y: 1_000,
        z: 1_000,
    };
    let proposed = Position { x: 1_100, ..from };
    let mut state = FlightState::default();
    assert_eq!(
        resolve_legacy_movement(&mut state, from, proposed, FlightProfile::CAVE, &marginal,),
        MovementResolution::Accept(proposed)
    );
    assert_eq!(state.yaw, 0);
}

#[test]
fn repeated_trace_has_a_stable_hash() {
    let commands = [
        PlayerCommand {
            flags: 1,
            ..Default::default()
        },
        PlayerCommand {
            flags: 1,
            roll: 8,
            ..Default::default()
        },
        PlayerCommand {
            flags: 9,
            roll: 12,
            ..Default::default()
        },
        PlayerCommand::default(),
    ];
    let run = || {
        let mut state = FlightState::default();
        for command in commands {
            step_turn(&mut state, command, FlightProfile::OUTDOOR, &FLAT);
        }
        state_hash(&state)
    };
    assert_eq!(run(), run());
}

#[test]
fn recovered_speed_effect_arithmetic_truncates_toward_zero() {
    assert_eq!(scale_speed_effect(-3, 1), -2);
    assert_eq!(scale_speed_effect(-4, 1), -3);
    assert_eq!(scale_speed_effect(3, 1), 2);
    assert_eq!(scale_speed_effect(4, 1), 3);
}

#[test]
fn movement_statuses_modify_controls_translation_and_counters() {
    let mut state = FlightState {
        speed_modifier: 2,
        speed_modifier_counter: 1,
        ..FlightState::default()
    };
    assert_eq!(modified_control_delta(&state, 8), 4);
    assert_eq!(modified_translation_speed(&state, 64), 32);
    state.speed_modifier = 1;
    assert_eq!(modified_control_delta(&state, -3), -2);
    assert_eq!(modified_translation_speed(&state, -3), -2);
    state.speed_modifier = 2;
    advance_status_counters(&mut state, None);
    assert_eq!(state.speed_modifier, 1);
    assert_eq!(state.speed_modifier_counter, 8);

    state.speed_modifier_counter = 1;
    advance_status_counters(&mut state, None);
    assert_eq!(state.speed_modifier, 0);
    assert_eq!(state.speed_modifier_counter, 0);

    state.immobilize_counter = 1;
    state.immobilize_subcounter = 2;
    assert_eq!(modified_translation_speed(&state, 64), 0);
    state.speed_modifier = 1;
    assert_eq!(
        modified_translation_speed(&state, 64),
        48,
        "the source gives speed modification priority over immobilization"
    );
    state.speed_modifier = 0;
    advance_status_counters(&mut state, None);
    assert_eq!(state.immobilize_counter, 1);
    assert_eq!(state.immobilize_subcounter, 1);
    advance_status_counters(&mut state, None);
    assert_eq!(state.immobilize_counter, 0);
    assert_eq!(state.immobilize_subcounter, 0);
}

#[test]
fn speed_modifier_stages_emit_exact_global_palette_flags() {
    let mut state = FlightState {
        speed_modifier: 2,
        speed_modifier_counter: 1,
        player_color_index: 3,
        ..FlightState::default()
    };
    let mut effects = FlightEffectsContext {
        local_player_index: 3,
        ..FlightEffectsContext::default()
    };
    advance_status_counters(&mut state, Some(&mut effects));
    assert_eq!(effects.move_speed_flag, 1);
    assert_eq!(effects.palette_submode, 0);

    state.speed_modifier_counter = 1;
    advance_status_counters(&mut state, Some(&mut effects));
    assert_eq!(state.speed_modifier, 0);
    assert_eq!(effects.palette_submode, 1);

    state.speed_modifier = 1;
    state.speed_modifier_counter = 1;
    state.player_color_index = 4;
    effects.palette_submode = 0;
    advance_status_counters(&mut state, Some(&mut effects));
    assert_eq!(effects.palette_submode, 0);
}

#[test]
fn immobilization_uses_its_vertical_step_until_the_expiry_tick() {
    let run = |subcounter| {
        let mut state = FlightState {
            position: Position {
                x: 1_000,
                y: 1_000,
                z: 1_000,
            },
            immobilize_counter: 1,
            immobilize_subcounter: subcounter,
            ..FlightState::default()
        };
        step_turn(
            &mut state,
            PlayerCommand::default(),
            FlightProfile::OUTDOOR,
            &FLAT,
        );
        state
    };

    let active = run(2);
    assert_eq!(active.immobilize_counter, 1);
    assert_eq!(active.position.z, 949);

    let expired = run(1);
    assert_eq!(expired.immobilize_counter, 0);
    assert_eq!(expired.position.z, 984);
}
