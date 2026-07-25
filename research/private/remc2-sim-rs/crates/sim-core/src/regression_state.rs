use crate::{FlightState, LatchedCommand, PlayerCommand, Position};

pub const REGRESSION_STATE_FRAME_BYTES: usize = 224_790;
pub const LEGACY_STATE_BASE_ADDRESS: u32 = 0x0035_6038;
pub const PLAYER_RECORDS_OFFSET: usize = 0x2bde;
pub const PLAYER_RECORD_BYTES: usize = 0x084c;
pub const PLAYER_TRANSIENT_OFFSET: usize = 0x03e6;
pub const PLAYER_INPUTS_OFFSET: usize = 0x6e3e;
pub const PLAYER_INPUT_BYTES: usize = 10;
pub const ENTITY_ARRAY_OFFSET: usize = 0x6e8e;
pub const ENTITY_BYTES: usize = 0x00a8;
pub const ENTITY_COUNT: usize = 1_000;
pub const LEGACY_SPECIAL_SETTINGS_BASE_ADDRESS: u32 = 0x002a_8bd6;
pub const SPECIAL_SETTINGS_RECORD_BYTES: u32 = 34;

/// Output of `PlayerEvents_51BB0` consumed by the bounded flight slice.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DecodedFlightInput {
    pub command: LatchedCommand,
    pub roll_delta: i16,
    pub pitch_delta: i16,
}

impl DecodedFlightInput {
    pub fn apply(self, state: &mut FlightState) {
        state.roll_delta = self.roll_delta;
        state.pitch_delta = self.pitch_delta;
    }
}

/// Read-only view over the original `DOSBox` whole-state regression sequence.
#[derive(Clone, Copy, Debug)]
pub struct RegressionStateSequence<'a> {
    bytes: &'a [u8],
}

impl<'a> RegressionStateSequence<'a> {
    /// Validate and open a sequence of raw `0x356038` state frames.
    ///
    /// # Errors
    ///
    /// Returns an error when the sequence is not aligned to remc2's exact
    /// 224,790-byte comparison span.
    pub fn new(bytes: &'a [u8]) -> Result<Self, String> {
        if !bytes.len().is_multiple_of(REGRESSION_STATE_FRAME_BYTES) {
            return Err(format!(
                "state sequence length must be a multiple of {REGRESSION_STATE_FRAME_BYTES}, got {}",
                bytes.len()
            ));
        }
        Ok(Self { bytes })
    }

    #[must_use]
    pub fn frame_count(self) -> usize {
        self.bytes.len() / REGRESSION_STATE_FRAME_BYTES
    }

    /// Return a single captured outer-frame state.
    ///
    /// # Errors
    ///
    /// Returns an error when `index` is outside the sequence.
    pub fn frame(self, index: usize) -> Result<RegressionStateFrame<'a>, String> {
        if index >= self.frame_count() {
            return Err(format!(
                "state frame index {index} is outside {} frames",
                self.frame_count()
            ));
        }
        let start = index * REGRESSION_STATE_FRAME_BYTES;
        Ok(RegressionStateFrame {
            bytes: &self.bytes[start..start + REGRESSION_STATE_FRAME_BYTES],
        })
    }
}

/// One state captured at `DrawAndEventsInGame_47560` address `0x2285ff`.
#[derive(Clone, Copy, Debug)]
pub struct RegressionStateFrame<'a> {
    bytes: &'a [u8],
}

impl<'a> RegressionStateFrame<'a> {
    #[must_use]
    pub fn local_player_slot(self) -> usize {
        usize::from(self.read_u16(0x000c))
    }

    #[must_use]
    pub fn player_count(self) -> usize {
        usize::from(self.read_u16(0x000e))
    }

    #[must_use]
    pub fn map_type(self) -> u8 {
        self.read_u8(0x2fed4)
    }

    /// The ten raw input bytes present in the captured state.
    ///
    /// `PlayerEvents_51BB0` normally clears these bytes before the post-frame
    /// snapshot. Use [`Self::decoded_flight_input`] for the durable output of
    /// that stage.
    ///
    /// # Panics
    ///
    /// Panics when `player_slot` is outside the original eight-player array.
    #[must_use]
    pub fn player_command(self, player_slot: usize) -> PlayerCommand {
        assert!(player_slot < 8, "player slot must be below eight");
        let start = PLAYER_INPUTS_OFFSET + player_slot * PLAYER_INPUT_BYTES;
        let bytes: [u8; PLAYER_INPUT_BYTES] = self.bytes[start..start + PLAYER_INPUT_BYTES]
            .try_into()
            .expect("fixed player command span");
        PlayerCommand::from_bytes(bytes)
    }

    /// Read the movement flags and deltas left by `PlayerEvents_51BB0`.
    ///
    /// Use the values from target frame `n` to replay the bounded flight
    /// transition from captured state `n - 1` into state `n`.
    ///
    /// # Panics
    ///
    /// Panics when `player_slot` is outside the original eight-player array.
    #[must_use]
    pub fn decoded_flight_input(self, player_slot: usize) -> DecodedFlightInput {
        let transient = player_transient_offset(player_slot);
        DecodedFlightInput {
            command: LatchedCommand {
                flags: self.read_u8(transient),
                argument_6: self.read_u16(transient + 0x0018),
                argument_8: self.read_u16(transient + 0x001a),
            },
            roll_delta: self.read_i16(transient + 0x0004),
            pitch_delta: self.read_i16(transient + 0x0006),
        }
    }

    /// Entity index assigned to one player record.
    ///
    /// # Panics
    ///
    /// Panics when `player_slot` is outside the original eight-player array.
    #[must_use]
    pub fn player_entity_index(self, player_slot: usize) -> u16 {
        self.read_u16(player_record_offset(player_slot) + 0x000a)
    }

    /// Outer-loop turn counter from one player record.
    ///
    /// # Panics
    ///
    /// Panics when `player_slot` is outside the original eight-player array.
    #[must_use]
    pub fn player_turn(self, player_slot: usize) -> i32 {
        self.read_i32(player_record_offset(player_slot) + 0x0012)
    }

    #[must_use]
    pub fn player_control_mode(self, player_slot: usize) -> u8 {
        self.read_u8(player_transient_offset(player_slot) + 0x01bf)
    }

    #[must_use]
    pub fn player_left_spell_slot(self, player_slot: usize) -> usize {
        usize::from(self.read_u16(player_transient_offset(player_slot) + 0x0451))
    }

    /// Return the entity index stored in one source spell-holder slot.
    ///
    /// # Panics
    ///
    /// Panics when `player_slot` is outside the original eight-player array or
    /// `spell_slot` is outside the original 26-spell array.
    #[must_use]
    pub fn player_enabled_spell_holder(self, player_slot: usize, spell_slot: usize) -> u16 {
        assert!(spell_slot < 26, "spell slot must be below 26");
        self.read_u16(player_transient_offset(player_slot) + 0x0333 + spell_slot * 2)
    }

    /// Return an original 168-byte entity view.
    ///
    /// # Panics
    ///
    /// Panics when `entity_index` is outside the original entity array.
    #[must_use]
    pub fn entity(self, entity_index: u16) -> RegressionEntity<'a> {
        let index = usize::from(entity_index);
        assert!(index < ENTITY_COUNT, "entity index must be below 1000");
        let start = ENTITY_ARRAY_OFFSET + index * ENTITY_BYTES;
        RegressionEntity {
            bytes: &self.bytes[start..start + ENTITY_BYTES],
            entity_index,
        }
    }

    /// Project the fields owned by the bounded Rust player-flight slice.
    ///
    /// `tick` deliberately starts at zero: the DOS snapshots count outer
    /// frames, while the Rust field counts entity-update passes.
    ///
    /// # Panics
    ///
    /// Panics when `player_slot` is outside the original eight-player array.
    #[must_use]
    pub fn project_player_flight(self, player_slot: usize) -> FlightState {
        let entity = self.entity(self.player_entity_index(player_slot));
        let transient = player_transient_offset(player_slot);
        FlightState {
            entity_index: entity.entity_index(),
            position: entity.position(),
            yaw: entity.yaw(),
            display_pitch: entity.pitch(),
            effective_pitch: self.read_i16(transient + 0x0024),
            roll_accumulator: self.read_i16(transient + 0x0155),
            pitch_accumulator: self.read_i16(transient + 0x0157),
            target_speed: self.read_i16(transient + 0x000c),
            speed_changed_flag: self.read_i16(transient + 0x000e),
            actual_speed: entity.actual_speed(),
            strafe_speed: self.read_i16(transient + 0x0010),
            roll_delta: self.read_i16(transient + 0x0004),
            pitch_delta: self.read_i16(transient + 0x0006),
            recoil_speed: self.read_i16(transient + 0x001e),
            recoil_yaw: self.read_i16(transient + 0x0020),
            impulse_x: self.read_i16(transient + 0x01a6),
            impulse_y: self.read_i16(transient + 0x01a8),
            impulse_z: self.read_i16(transient + 0x01aa),
            entity_fov: entity.fov(),
            water_counter: self.read_i8(transient + 0x0262),
            speed_modifier: self.read_u8(transient + 0x014c),
            speed_modifier_counter: self.read_i8(transient + 0x014d),
            immobilize_counter: self.read_i8(transient + 0x014e),
            immobilize_subcounter: self.read_i8(transient + 0x014f),
            tether_prior_distance: self.read_i32(transient + 0x0142),
            tether_target_index: self.read_u16(transient + 0x0146),
            tether_holder_index: self.read_u16(transient + 0x034f),
            tether_tier: self.read_u16(transient + 0x014a),
            speed_spell_holder_index: self.read_u16(transient + 0x0339),
            entity_phase: entity.phase(),
            ambient_loop_5_distance: self.read_i32(transient + 0x019a),
            ambient_loop_31_distance: self.read_i32(transient + 0x019e),
            player_color_index: self.read_u8(transient + 0x0038),
            music_countdown: self.read_i16(transient + 0x0036),
            recovery_latch: self.read_i8(transient + 0x0261) != 0,
            skip_flight_once: entity.flight_skip_flag(),
            tick: 0,
        }
    }

    fn read_u8(self, offset: usize) -> u8 {
        self.bytes[offset]
    }

    fn read_i8(self, offset: usize) -> i8 {
        self.read_u8(offset).cast_signed()
    }

    fn read_u16(self, offset: usize) -> u16 {
        u16::from_le_bytes(
            self.bytes[offset..offset + 2]
                .try_into()
                .expect("validated state frame offset"),
        )
    }

    fn read_i16(self, offset: usize) -> i16 {
        self.read_u16(offset).cast_signed()
    }

    fn read_i32(self, offset: usize) -> i32 {
        i32::from_le_bytes(
            self.bytes[offset..offset + 4]
                .try_into()
                .expect("validated state frame offset"),
        )
    }
}

/// Read-only view over one original entity record.
#[derive(Clone, Copy, Debug)]
pub struct RegressionEntity<'a> {
    bytes: &'a [u8],
    entity_index: u16,
}

impl<'a> RegressionEntity<'a> {
    #[must_use]
    pub fn record_bytes(self) -> &'a [u8] {
        self.bytes
    }

    #[must_use]
    pub fn entity_index(self) -> u16 {
        self.entity_index
    }

    #[must_use]
    pub fn class(self) -> u8 {
        self.bytes[0x003f]
    }

    #[must_use]
    pub fn model(self) -> u8 {
        self.bytes[0x0040]
    }

    #[must_use]
    pub fn life(self) -> i32 {
        read_i32(self.bytes, 0x0008)
    }

    #[must_use]
    pub fn max_life(self) -> i32 {
        read_i32(self.bytes, 0x0004)
    }

    #[must_use]
    pub fn control_flags(self) -> u32 {
        read_u32(self.bytes, 0x000c)
    }

    #[must_use]
    pub fn action(self) -> u8 {
        self.bytes[0x0045]
    }

    #[must_use]
    pub fn level_gate(self) -> i8 {
        self.bytes[0x0046].cast_signed()
    }

    #[must_use]
    pub fn active_value(self) -> i16 {
        read_i16(self.bytes, 0x002e)
    }

    #[must_use]
    pub fn activation_value(self) -> u16 {
        read_u16(self.bytes, 0x0030)
    }

    #[must_use]
    pub fn action_marker(self) -> i8 {
        self.bytes[0x003c].cast_signed()
    }

    #[must_use]
    pub fn position(self) -> Position {
        Position {
            x: read_u16(self.bytes, 0x004c),
            y: read_u16(self.bytes, 0x004e),
            z: read_i16(self.bytes, 0x0050),
        }
    }

    #[must_use]
    pub fn yaw(self) -> i16 {
        read_i16(self.bytes, 0x001c)
    }

    #[must_use]
    pub fn rng(self) -> u16 {
        read_u16(self.bytes, 0x0014)
    }

    #[must_use]
    pub fn source_id(self) -> u16 {
        read_u16(self.bytes, 0x001a)
    }

    #[must_use]
    pub fn pitch(self) -> i16 {
        read_i16(self.bytes, 0x001e)
    }

    #[must_use]
    pub fn roll(self) -> i16 {
        read_i16(self.bytes, 0x0020)
    }

    #[must_use]
    pub fn fov(self) -> i16 {
        read_i16(self.bytes, 0x0022)
    }

    #[must_use]
    pub fn actual_speed(self) -> i16 {
        read_i16(self.bytes, 0x0082)
    }

    #[must_use]
    pub fn minimum_speed(self) -> i16 {
        read_i16(self.bytes, 0x0084)
    }

    #[must_use]
    pub fn maximum_speed(self) -> i16 {
        read_i16(self.bytes, 0x0086)
    }

    #[must_use]
    pub fn mana_cost(self) -> i32 {
        read_i32(self.bytes, 0x008c)
    }

    #[must_use]
    pub fn mana(self) -> i32 {
        read_i32(self.bytes, 0x0090)
    }

    #[must_use]
    pub fn phase(self) -> u8 {
        self.bytes[0x003e]
    }

    #[must_use]
    pub fn stage_var_1(self) -> i8 {
        self.bytes[0x0048].cast_signed()
    }

    #[must_use]
    pub fn stage_var_2(self) -> i8 {
        self.bytes[0x0049].cast_signed()
    }

    #[must_use]
    pub fn proximity_countdown(self) -> i8 {
        self.bytes[0x0039].cast_signed()
    }

    #[must_use]
    pub fn collision_half_width(self) -> i16 {
        read_i16(self.bytes, 0x0054)
    }

    #[must_use]
    pub fn pending_damage(self) -> i32 {
        read_i32(self.bytes, 0x005e)
    }

    #[must_use]
    pub fn pending_attacker(self) -> i16 {
        read_i16(self.bytes, 0x0062)
    }

    #[must_use]
    pub fn kill_credit_entity(self) -> i16 {
        read_i16(self.bytes, 0x0024)
    }

    #[must_use]
    pub fn current_attacker(self) -> i16 {
        read_i16(self.bytes, 0x0026)
    }

    #[must_use]
    pub fn target_entity_index(self) -> Option<u16> {
        match read_u16(self.bytes, 0x0096) {
            0 => None,
            value => Some(value),
        }
    }

    #[must_use]
    pub fn transient_address(self) -> u32 {
        read_u32(self.bytes, 0x00a4)
    }

    #[must_use]
    pub fn special_settings_address(self) -> u32 {
        read_u32(self.bytes, 0x00a0)
    }

    #[must_use]
    pub fn special_settings_index(self) -> Option<usize> {
        let relative = self
            .special_settings_address()
            .checked_sub(LEGACY_SPECIAL_SETTINGS_BASE_ADDRESS)?;
        if relative % SPECIAL_SETTINGS_RECORD_BYTES != 0 {
            return None;
        }
        usize::try_from(relative / SPECIAL_SETTINGS_RECORD_BYTES).ok()
    }

    #[must_use]
    pub fn flight_skip_flag(self) -> bool {
        self.bytes[0x000d] & 8 != 0
    }
}

fn player_record_offset(player_slot: usize) -> usize {
    assert!(player_slot < 8, "player slot must be below eight");
    PLAYER_RECORDS_OFFSET + player_slot * PLAYER_RECORD_BYTES
}

fn player_transient_offset(player_slot: usize) -> usize {
    player_record_offset(player_slot) + PLAYER_TRANSIENT_OFFSET
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("fixed entity field span"),
    )
}

fn read_i16(bytes: &[u8], offset: usize) -> i16 {
    read_u16(bytes, offset).cast_signed()
}

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("fixed entity field span"),
    )
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("fixed entity field span"),
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{
        FlightContext, FlightEvent, FlightProfile, SOURCE_CAVE_PLAYER_SETTINGS_INDEX,
        SOURCE_OUTDOOR_PLAYER_SETTINGS_INDEX, SpellHolder, TetherContext, TetherTarget,
        latch_command,
        legacy_terrain::LegacyTerrain,
        recording::InputRecording,
        spell_activation::{SpellActivationHolder, SpellCasterState, activate_spell},
        step_entity_pass_with_context,
    };

    const OWNED_LEVEL_ONE_STATE_SEQUENCE: &[u8] = include_bytes!(
        "../../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions/level1/sequence-002285FF-00356038.bin"
    );
    const OWNED_LEVEL_ONE_TERRAIN_SEQUENCE: &[u8] = include_bytes!(
        "../../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions/level1/sequence-002285FF-002DC4E0.bin"
    );
    const OWNED_AFTER_LOAD_STATE_SEQUENCE: &[u8] = include_bytes!(
        "../../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions/afterloadtest2/sequence-002285FF-00356038.bin"
    );
    const OWNED_AFTER_LOAD_TERRAIN_SEQUENCE: &[u8] = include_bytes!(
        "../../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions/afterloadtest2/sequence-002285FF-002DC4E0.bin"
    );
    const OWNED_AFTER_LOAD_RECORDING: &[u8] = include_bytes!(
        "../../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions/afterloadtest2/Levels-1-5-Recording.bin"
    );
    const NATIVE_FORWARD_TRACE: [(Position, i16, i16, i16, u8); 8] = [
        (
            Position {
                x: 19_840,
                y: 56_944,
                z: 5_024,
            },
            16,
            1,
            16,
            154,
        ),
        (
            Position {
                x: 19_840,
                y: 56_912,
                z: 5_024,
            },
            32,
            1,
            32,
            155,
        ),
        (
            Position {
                x: 19_840,
                y: 56_864,
                z: 5_024,
            },
            48,
            1,
            48,
            156,
        ),
        (
            Position {
                x: 19_840,
                y: 56_800,
                z: 5_028,
            },
            64,
            1,
            64,
            157,
        ),
        (
            Position {
                x: 19_840,
                y: 56_720,
                z: 5_038,
            },
            80,
            1,
            80,
            158,
        ),
        (
            Position {
                x: 19_840,
                y: 56_640,
                z: 5_040,
            },
            80,
            0,
            80,
            159,
        ),
        (
            Position {
                x: 19_840,
                y: 56_560,
                z: 5_040,
            },
            80,
            0,
            80,
            160,
        ),
        (
            Position {
                x: 19_840,
                y: 56_480,
                z: 5_040,
            },
            80,
            0,
            80,
            161,
        ),
    ];

    #[derive(Clone, Copy)]
    struct NativeControlPoint {
        position: Position,
        yaw: i16,
        roll_accumulator: i16,
        target_speed: i16,
        speed_changed_flag: i16,
        actual_speed: i16,
        strafe_speed: i16,
        entity_phase: u8,
    }

    const fn control_point(
        position: Position,
        yaw: i16,
        roll_accumulator: i16,
        speeds: (i16, i16, i16, i16),
        entity_phase: u8,
    ) -> NativeControlPoint {
        NativeControlPoint {
            position,
            yaw,
            roll_accumulator,
            target_speed: speeds.0,
            speed_changed_flag: speeds.1,
            actual_speed: speeds.2,
            strafe_speed: speeds.3,
            entity_phase,
        }
    }

    const NATIVE_STEER_FORWARD_TRACE: [NativeControlPoint; 4] = [
        control_point(
            Position {
                x: 19_840,
                y: 56_944,
                z: 5_024,
            },
            0,
            4,
            (16, 1, 16, 0),
            154,
        ),
        control_point(
            Position {
                x: 19_840,
                y: 56_912,
                z: 5_024,
            },
            1,
            8,
            (32, 1, 32, 0),
            155,
        ),
        control_point(
            Position {
                x: 19_840,
                y: 56_865,
                z: 5_024,
            },
            2,
            12,
            (48, 1, 48, 0),
            156,
        ),
        control_point(
            Position {
                x: 19_840,
                y: 56_802,
                z: 5_027,
            },
            4,
            16,
            (64, 1, 64, 0),
            157,
        ),
    ];
    const NATIVE_REVERSE_TRACE: [NativeControlPoint; 4] = [
        control_point(
            Position {
                x: 19_840,
                y: 56_976,
                z: 5_024,
            },
            0,
            0,
            (-16, 1, -16, 0),
            154,
        ),
        control_point(
            Position {
                x: 19_840,
                y: 57_008,
                z: 5_024,
            },
            0,
            0,
            (-32, 1, -32, 0),
            155,
        ),
        control_point(
            Position {
                x: 19_840,
                y: 57_056,
                z: 5_024,
            },
            0,
            0,
            (-48, 1, -48, 0),
            156,
        ),
        control_point(
            Position {
                x: 19_840,
                y: 57_120,
                z: 5_032,
            },
            0,
            0,
            (-64, 1, -64, 0),
            157,
        ),
    ];
    const NATIVE_STRAFE_NEGATIVE_TRACE: [NativeControlPoint; 4] = [
        control_point(
            Position {
                x: 19_824,
                y: 56_960,
                z: 5_024,
            },
            0,
            0,
            (0, 0, 0, -16),
            154,
        ),
        control_point(
            Position {
                x: 19_792,
                y: 56_960,
                z: 5_024,
            },
            0,
            0,
            (0, 0, 0, -32),
            155,
        ),
        control_point(
            Position {
                x: 19_744,
                y: 56_960,
                z: 5_024,
            },
            0,
            0,
            (0, 0, 0, -48),
            156,
        ),
        control_point(
            Position {
                x: 19_680,
                y: 56_960,
                z: 5_020,
            },
            0,
            0,
            (0, 0, 0, -64),
            157,
        ),
    ];
    const NATIVE_STRAFE_POSITIVE_TRACE: [NativeControlPoint; 4] = [
        control_point(
            Position {
                x: 19_856,
                y: 56_960,
                z: 5_024,
            },
            0,
            0,
            (0, 0, 0, 16),
            154,
        ),
        control_point(
            Position {
                x: 19_888,
                y: 56_960,
                z: 5_024,
            },
            0,
            0,
            (0, 0, 0, 32),
            155,
        ),
        control_point(
            Position {
                x: 19_936,
                y: 56_960,
                z: 5_024,
            },
            0,
            0,
            (0, 0, 0, 48),
            156,
        ),
        control_point(
            Position {
                x: 20_000,
                y: 56_960,
                z: 5_024,
            },
            0,
            0,
            (0, 0, 0, 64),
            157,
        ),
    ];

    #[derive(Clone, Copy)]
    struct NativePitchPoint {
        position: Position,
        pitch: i16,
        pitch_accumulator: i16,
        target_speed: i16,
        actual_speed: i16,
        entity_phase: u8,
    }

    const NATIVE_PITCH_FORWARD_TRACE: [NativePitchPoint; 4] = [
        NativePitchPoint {
            position: Position {
                x: 19_840,
                y: 56_945,
                z: 5_024,
            },
            pitch: 4,
            pitch_accumulator: 4,
            target_speed: 16,
            actual_speed: 16,
            entity_phase: 154,
        },
        NativePitchPoint {
            position: Position {
                x: 19_840,
                y: 56_914,
                z: 5_024,
            },
            pitch: 8,
            pitch_accumulator: 8,
            target_speed: 32,
            actual_speed: 32,
            entity_phase: 155,
        },
        NativePitchPoint {
            position: Position {
                x: 19_840,
                y: 56_867,
                z: 5_024,
            },
            pitch: 12,
            pitch_accumulator: 12,
            target_speed: 48,
            actual_speed: 48,
            entity_phase: 156,
        },
        NativePitchPoint {
            position: Position {
                x: 19_840,
                y: 56_804,
                z: 5_027,
            },
            pitch: 16,
            pitch_accumulator: 16,
            target_speed: 64,
            actual_speed: 64,
            entity_phase: 157,
        },
    ];

    #[derive(Clone, Copy)]
    struct NativeRecoilPoint {
        position: Position,
        recoil_speed: i16,
        entity_phase: u8,
    }

    const NATIVE_POSITIVE_RECOIL_TRACE: [NativeRecoilPoint; 4] = [
        NativeRecoilPoint {
            position: Position {
                x: 19_840,
                y: 57_088,
                z: 5_024,
            },
            recoil_speed: 124,
            entity_phase: 154,
        },
        NativeRecoilPoint {
            position: Position {
                x: 19_840,
                y: 57_212,
                z: 5_055,
            },
            recoil_speed: 120,
            entity_phase: 155,
        },
        NativeRecoilPoint {
            position: Position {
                x: 19_840,
                y: 57_332,
                z: 5_085,
            },
            recoil_speed: 116,
            entity_phase: 156,
        },
        NativeRecoilPoint {
            position: Position {
                x: 19_840,
                y: 57_448,
                z: 5_452,
            },
            recoil_speed: 112,
            entity_phase: 157,
        },
    ];
    const NATIVE_NEGATIVE_RECOIL_TRACE: [NativeRecoilPoint; 4] = [
        NativeRecoilPoint {
            position: Position {
                x: 19_760,
                y: 56_960,
                z: 5_024,
            },
            recoil_speed: -76,
            entity_phase: 154,
        },
        NativeRecoilPoint {
            position: Position {
                x: 19_684,
                y: 56_960,
                z: 5_020,
            },
            recoil_speed: -72,
            entity_phase: 155,
        },
        NativeRecoilPoint {
            position: Position {
                x: 19_612,
                y: 56_960,
                z: 5_011,
            },
            recoil_speed: -68,
            entity_phase: 156,
        },
        NativeRecoilPoint {
            position: Position {
                x: 19_544,
                y: 56_960,
                z: 5_003,
            },
            recoil_speed: -64,
            entity_phase: 157,
        },
    ];

    #[derive(Clone, Copy)]
    struct NativeStatusPoint {
        position: Position,
        yaw: i16,
        rotation: i16,
        target_speed: i16,
        actual_speed: i16,
        speed_modifier: u8,
        speed_modifier_counter: i8,
        immobilize_counter: i8,
        immobilize_subcounter: i8,
        entity_phase: u8,
    }

    const fn status_point(
        position: Position,
        yaw: i16,
        rotation: i16,
        speeds: (i16, i16),
        status: (u8, i8, i8, i8),
        entity_phase: u8,
    ) -> NativeStatusPoint {
        NativeStatusPoint {
            position,
            yaw,
            rotation,
            target_speed: speeds.0,
            actual_speed: speeds.1,
            speed_modifier: status.0,
            speed_modifier_counter: status.1,
            immobilize_counter: status.2,
            immobilize_subcounter: status.3,
            entity_phase,
        }
    }

    const NATIVE_SPEED_MODIFIER_TRACE: [NativeStatusPoint; 4] = [
        status_point(
            Position {
                x: 19_840,
                y: 56_956,
                z: 5_024,
            },
            0,
            1,
            (16, 16),
            (2, 8, 0, 0),
            154,
        ),
        status_point(
            Position {
                x: 19_840,
                y: 56_941,
                z: 5_024,
            },
            0,
            3,
            (32, 32),
            (2, 7, 0, 0),
            155,
        ),
        status_point(
            Position {
                x: 19_840,
                y: 56_918,
                z: 5_024,
            },
            0,
            5,
            (48, 48),
            (2, 6, 0, 0),
            156,
        ),
        status_point(
            Position {
                x: 19_840,
                y: 56_887,
                z: 5_024,
            },
            0,
            7,
            (64, 64),
            (2, 5, 0, 0),
            157,
        ),
    ];
    const NATIVE_IMMOBILIZED_TRACE: [NativeStatusPoint; 4] = [
        status_point(
            Position {
                x: 19_840,
                y: 56_960,
                z: 5_024,
            },
            0,
            4,
            (16, 16),
            (0, 0, 2, 1),
            154,
        ),
        status_point(
            Position {
                x: 19_840,
                y: 56_960,
                z: 5_024,
            },
            1,
            8,
            (32, 32),
            (0, 0, 1, 0),
            155,
        ),
        status_point(
            Position {
                x: 19_840,
                y: 56_960,
                z: 5_024,
            },
            2,
            12,
            (48, 48),
            (0, 0, 1, -1),
            156,
        ),
        status_point(
            Position {
                x: 19_840,
                y: 56_960,
                z: 5_024,
            },
            4,
            16,
            (64, 64),
            (0, 0, 1, -2),
            157,
        ),
    ];

    #[test]
    fn owned_level_one_sequence_has_twenty_exact_state_frames() {
        let sequence = RegressionStateSequence::new(OWNED_LEVEL_ONE_STATE_SEQUENCE).unwrap();
        assert_eq!(sequence.frame_count(), 20);
        assert!(sequence.frame(20).is_err());
    }

    #[test]
    fn snapshot_layout_offsets_are_self_consistent() {
        assert_eq!(
            PLAYER_INPUTS_OFFSET + 8 * PLAYER_INPUT_BYTES,
            ENTITY_ARRAY_OFFSET
        );
        assert_eq!(ENTITY_ARRAY_OFFSET + ENTITY_COUNT * ENTITY_BYTES, 0x2fece);
        assert_eq!(
            LEGACY_STATE_BASE_ADDRESS + u32::try_from(player_transient_offset(0)).unwrap(),
            0x0035_8ffc
        );
    }

    #[test]
    fn owned_level_one_local_player_projection_matches_raw_entity_links() {
        let sequence = RegressionStateSequence::new(OWNED_LEVEL_ONE_STATE_SEQUENCE).unwrap();
        for frame_index in 0..sequence.frame_count() {
            let frame = sequence.frame(frame_index).unwrap();
            let player_slot = frame.local_player_slot();
            let entity_index = frame.player_entity_index(player_slot);
            let entity = frame.entity(entity_index);
            let expected_transient = LEGACY_STATE_BASE_ADDRESS
                + u32::try_from(player_transient_offset(player_slot)).unwrap();

            assert_eq!(entity.class(), 3);
            assert_eq!(entity.model(), 0);
            assert_eq!(entity.transient_address(), expected_transient);

            let projected = frame.project_player_flight(player_slot);
            assert_eq!(projected.position, entity.position());
            assert_eq!(projected.yaw, entity.yaw());
            assert_eq!(projected.display_pitch, entity.pitch());
            assert_eq!(projected.actual_speed, entity.actual_speed());
        }
    }

    #[test]
    fn owned_native_four_and_eight_pass_forward_traces_match_every_pass() {
        // Captured by remc2-multipass-flight-trace.gdb from the pinned native
        // build. The first native outer turn establishes level-1 frame zero;
        // the second latches flags=1 once and runs either four or eight entity
        // passes. This checks every bounded player-flight state observed after
        // each native pass, rather than only the final outer-frame endpoint.
        let sequence = RegressionStateSequence::new(OWNED_LEVEL_ONE_STATE_SEQUENCE).unwrap();
        let frame = sequence.frame(0).unwrap();
        let player_slot = frame.local_player_slot();
        let terrain =
            LegacyTerrain::from_regression_sequence(OWNED_LEVEL_ONE_TERRAIN_SEQUENCE, 0, false)
                .unwrap();

        for pass_count in [4_usize, 8] {
            let mut actual = frame.project_player_flight(player_slot);
            let command = latch_command(
                &mut actual,
                PlayerCommand {
                    flags: 1,
                    ..PlayerCommand::default()
                },
            );
            let mut context = FlightContext::default();

            for (pass, &(position, target_speed, changed, actual_speed, phase)) in
                NATIVE_FORWARD_TRACE[..pass_count].iter().enumerate()
            {
                step_entity_pass_with_context(
                    &mut actual,
                    command,
                    FlightProfile::OUTDOOR,
                    &terrain,
                    &mut context,
                );
                assert_eq!(actual.position, position, "pass {}", pass + 1);
                assert_eq!(actual.yaw, 0, "pass {}", pass + 1);
                assert_eq!(actual.display_pitch, 0, "pass {}", pass + 1);
                assert_eq!(actual.effective_pitch, 0, "pass {}", pass + 1);
                assert_eq!(actual.roll_accumulator, 0, "pass {}", pass + 1);
                assert_eq!(actual.pitch_accumulator, 0, "pass {}", pass + 1);
                assert_eq!(actual.target_speed, target_speed, "pass {}", pass + 1);
                assert_eq!(actual.speed_changed_flag, changed, "pass {}", pass + 1);
                assert_eq!(actual.actual_speed, actual_speed, "pass {}", pass + 1);
                assert_eq!(actual.strafe_speed, 0, "pass {}", pass + 1);
                assert_eq!(actual.entity_phase, phase, "pass {}", pass + 1);
            }
        }
    }

    #[test]
    fn owned_native_active_control_traces_match_every_pass() {
        let sequence = RegressionStateSequence::new(OWNED_LEVEL_ONE_STATE_SEQUENCE).unwrap();
        let frame = sequence.frame(0).unwrap();
        let terrain =
            LegacyTerrain::from_regression_sequence(OWNED_LEVEL_ONE_TERRAIN_SEQUENCE, 0, false)
                .unwrap();

        for (command, expected) in [
            (
                PlayerCommand {
                    roll: 8,
                    flags: 1,
                    ..PlayerCommand::default()
                },
                &NATIVE_STEER_FORWARD_TRACE,
            ),
            (
                PlayerCommand {
                    flags: 2,
                    ..PlayerCommand::default()
                },
                &NATIVE_REVERSE_TRACE,
            ),
            (
                PlayerCommand {
                    flags: 4,
                    ..PlayerCommand::default()
                },
                &NATIVE_STRAFE_NEGATIVE_TRACE,
            ),
            (
                PlayerCommand {
                    flags: 8,
                    ..PlayerCommand::default()
                },
                &NATIVE_STRAFE_POSITIVE_TRACE,
            ),
        ] {
            assert_native_control_trace(frame, &terrain, command, expected);
        }
    }

    fn assert_native_control_trace(
        frame: RegressionStateFrame<'_>,
        terrain: &LegacyTerrain,
        command: PlayerCommand,
        expected: &[NativeControlPoint],
    ) {
        let mut actual = frame.project_player_flight(frame.local_player_slot());
        let command = latch_command(&mut actual, command);
        let mut context = FlightContext::default();
        for (pass, point) in expected.iter().enumerate() {
            step_entity_pass_with_context(
                &mut actual,
                command,
                FlightProfile::OUTDOOR,
                terrain,
                &mut context,
            );
            let message = format!("native control trace pass {}", pass + 1);
            assert_eq!(actual.position, point.position, "{message}");
            assert_eq!(actual.yaw, point.yaw, "{message}");
            assert_eq!(actual.display_pitch, 0, "{message}");
            assert_eq!(actual.effective_pitch, 0, "{message}");
            assert_eq!(actual.roll_accumulator, point.roll_accumulator, "{message}");
            assert_eq!(actual.pitch_accumulator, 0, "{message}");
            assert_eq!(actual.target_speed, point.target_speed, "{message}");
            assert_eq!(
                actual.speed_changed_flag, point.speed_changed_flag,
                "{message}"
            );
            assert_eq!(actual.actual_speed, point.actual_speed, "{message}");
            assert_eq!(actual.strafe_speed, point.strafe_speed, "{message}");
            assert_eq!(actual.entity_phase, point.entity_phase, "{message}");
        }
    }

    #[test]
    fn owned_native_pitch_and_recoil_traces_match_every_pass() {
        let sequence = RegressionStateSequence::new(OWNED_LEVEL_ONE_STATE_SEQUENCE).unwrap();
        let frame = sequence.frame(0).unwrap();
        let terrain =
            LegacyTerrain::from_regression_sequence(OWNED_LEVEL_ONE_TERRAIN_SEQUENCE, 0, false)
                .unwrap();

        assert_native_pitch_trace(frame, &terrain);
        assert_native_recoil_trace(frame, &terrain, 200, 1_024, &NATIVE_POSITIVE_RECOIL_TRACE);
        assert_native_recoil_trace(frame, &terrain, -80, 512, &NATIVE_NEGATIVE_RECOIL_TRACE);
    }

    fn assert_native_pitch_trace(frame: RegressionStateFrame<'_>, terrain: &LegacyTerrain) {
        let mut actual = frame.project_player_flight(frame.local_player_slot());
        let command = latch_command(
            &mut actual,
            PlayerCommand {
                pitch: 8,
                flags: 1,
                ..PlayerCommand::default()
            },
        );
        let mut context = FlightContext::default();
        for (pass, point) in NATIVE_PITCH_FORWARD_TRACE.iter().enumerate() {
            step_entity_pass_with_context(
                &mut actual,
                command,
                FlightProfile::OUTDOOR,
                terrain,
                &mut context,
            );
            let message = format!("native pitch trace pass {}", pass + 1);
            assert_eq!(actual.position, point.position, "{message}");
            assert_eq!(actual.display_pitch, point.pitch, "{message}");
            assert_eq!(actual.effective_pitch, point.pitch, "{message}");
            assert_eq!(
                actual.pitch_accumulator, point.pitch_accumulator,
                "{message}"
            );
            assert_eq!(actual.target_speed, point.target_speed, "{message}");
            assert_eq!(actual.actual_speed, point.actual_speed, "{message}");
            assert_eq!(actual.entity_phase, point.entity_phase, "{message}");
        }
    }

    fn assert_native_recoil_trace(
        frame: RegressionStateFrame<'_>,
        terrain: &LegacyTerrain,
        recoil_speed: i16,
        recoil_yaw: i16,
        expected: &[NativeRecoilPoint],
    ) {
        let mut actual = frame.project_player_flight(frame.local_player_slot());
        actual.recoil_speed = recoil_speed;
        actual.recoil_yaw = recoil_yaw;
        let command = latch_command(&mut actual, PlayerCommand::default());
        let mut context = FlightContext::default();
        for (pass, point) in expected.iter().enumerate() {
            step_entity_pass_with_context(
                &mut actual,
                command,
                FlightProfile::OUTDOOR,
                terrain,
                &mut context,
            );
            let message = format!("native recoil trace pass {}", pass + 1);
            assert_eq!(actual.position, point.position, "{message}");
            assert_eq!(actual.recoil_speed, point.recoil_speed, "{message}");
            assert_eq!(actual.recoil_yaw, recoil_yaw, "{message}");
            assert_eq!(actual.entity_phase, point.entity_phase, "{message}");
        }
    }

    #[test]
    fn owned_native_status_modifier_traces_match_every_pass() {
        let sequence = RegressionStateSequence::new(OWNED_LEVEL_ONE_STATE_SEQUENCE).unwrap();
        let frame = sequence.frame(0).unwrap();
        let terrain =
            LegacyTerrain::from_regression_sequence(OWNED_LEVEL_ONE_TERRAIN_SEQUENCE, 0, false)
                .unwrap();

        assert_native_status_trace(frame, &terrain, (3, 1, 0, 0), &NATIVE_SPEED_MODIFIER_TRACE);
        assert_native_status_trace(frame, &terrain, (0, 0, 2, 2), &NATIVE_IMMOBILIZED_TRACE);
    }

    #[test]
    fn owned_native_level_three_material_boundary_matches_reference() {
        let fixture_root = regression_root().join("level3");
        let state_bytes =
            std::fs::read(fixture_root.join("sequence-002285FF-00356038.bin")).unwrap();
        let terrain_bytes =
            std::fs::read(fixture_root.join("sequence-002285FF-002DC4E0.bin")).unwrap();
        let frame = RegressionStateSequence::new(&state_bytes)
            .unwrap()
            .frame(0)
            .unwrap();
        let terrain = LegacyTerrain::from_regression_sequence(&terrain_bytes, 0, false).unwrap();
        let mut actual = frame.project_player_flight(frame.local_player_slot());
        actual.position = Position {
            x: 23_168,
            y: 20_454,
            z: 5_000,
        };
        actual.yaw = 1_024;
        actual.target_speed = 80;
        actual.actual_speed = 80;
        let command = latch_command(&mut actual, PlayerCommand::default());
        let evidence = step_entity_pass_with_context(
            &mut actual,
            command,
            FlightProfile::OUTDOOR,
            &terrain,
            &mut FlightContext::default(),
        );

        assert_eq!(
            actual.position,
            Position {
                x: 23_168,
                y: 20_454,
                z: 4_984,
            }
        );
        assert_eq!(actual.water_counter, 1);
        assert!(!actual.recovery_latch);
        assert_eq!(actual.target_speed, 80);
        assert_eq!(actual.actual_speed, 80);
        assert_eq!(actual.entity_phase, 254);
        assert_eq!(evidence.movement_resolution, "redirected");
    }

    #[test]
    fn owned_native_level_one_tether_pull_matches_reference() {
        let sequence = RegressionStateSequence::new(OWNED_LEVEL_ONE_STATE_SEQUENCE).unwrap();
        let frame = sequence.frame(0).unwrap();
        let terrain =
            LegacyTerrain::from_regression_sequence(OWNED_LEVEL_ONE_TERRAIN_SEQUENCE, 0, false)
                .unwrap();
        let target_entity = frame.entity(155);

        assert_eq!(target_entity.class(), 10);
        assert_eq!(target_entity.model(), 14);
        assert_eq!(target_entity.life(), 31);

        let mut actual = frame.project_player_flight(frame.local_player_slot());
        actual.tether_prior_distance = 0;
        actual.tether_target_index = 155;
        actual.tether_holder_index = 153;
        actual.tether_tier = 0;
        let command = latch_command(&mut actual, PlayerCommand::default());
        let target = TetherTarget {
            entity_index: target_entity.entity_index(),
            position: target_entity.position(),
            life: target_entity.life(),
            ..TetherTarget::default()
        };
        let mut context = FlightContext {
            tether: TetherContext {
                holder: Some(SpellHolder {
                    entity_index: 153,
                    active_value: 1,
                }),
                target: Some(target),
            },
            ..FlightContext::default()
        };

        let evidence = step_entity_pass_with_context(
            &mut actual,
            command,
            FlightProfile::OUTDOOR,
            &terrain,
            &mut context,
        );

        assert_eq!(
            actual.position,
            Position {
                x: 19_856,
                y: 56_842,
                z: 5_024,
            }
        );
        assert_eq!(actual.yaw, 44);
        assert_eq!(actual.target_speed, 0);
        assert_eq!(actual.actual_speed, 0);
        assert_eq!(actual.tether_prior_distance, 0);
        assert_eq!(actual.tether_target_index, 155);
        assert_eq!(actual.tether_holder_index, 153);
        assert_eq!(actual.tether_tier, 0);
        assert_eq!(actual.entity_phase, 154);
        assert_eq!(context.tether.target, Some(target));
        assert_eq!(evidence.movement_resolution, "accepted");
    }

    #[test]
    fn owned_native_level_one_left_fire_activation_matches_reference() {
        let sequence = RegressionStateSequence::new(OWNED_LEVEL_ONE_STATE_SEQUENCE).unwrap();
        let frame = sequence.frame(0).unwrap();
        let player_slot = frame.local_player_slot();
        let player = frame.entity(frame.player_entity_index(player_slot));
        let left_slot = frame.player_left_spell_slot(player_slot);
        let holder_index = frame.player_enabled_spell_holder(player_slot, left_slot);
        let holder_entity = frame.entity(holder_index);
        let projected_flight = frame.project_player_flight(player_slot);

        let mut caster = SpellCasterState {
            entity_index: player.entity_index(),
            model: player.model(),
            mana: player.mana(),
            control_flags: player.control_flags(),
            player_color_index: projected_flight.player_color_index,
            control_mode: frame.player_control_mode(player_slot),
        };
        let mut holder = SpellActivationHolder {
            entity_index: holder_entity.entity_index(),
            model: holder_entity.model(),
            level_gate: holder_entity.level_gate(),
            active_value: holder_entity.active_value(),
            activation_value: holder_entity.activation_value(),
            mana_cost: holder_entity.mana_cost(),
            action_marker: holder_entity.action_marker(),
        };

        assert_eq!(left_slot, 0);
        assert_eq!(holder_index, 153);
        assert_eq!(
            caster,
            SpellCasterState {
                entity_index: 152,
                model: 0,
                mana: 1_000,
                control_flags: 13,
                player_color_index: 0,
                control_mode: 0,
            }
        );
        assert_eq!(
            holder,
            SpellActivationHolder {
                entity_index: 153,
                model: 0,
                level_gate: 0,
                active_value: 0,
                activation_value: 5,
                mana_cost: 100,
                action_marker: 0,
            }
        );

        let mut events = Vec::<FlightEvent>::new();
        let activated = activate_spell(&mut caster, Some(&mut holder), 256, false, &mut events);

        assert!(activated);
        assert_eq!(caster.mana, 1_000);
        assert_eq!(caster.control_flags, 269);
        assert_eq!(holder.active_value, 5);
        assert_eq!(holder.action_marker, 0);
        assert!(events.is_empty());
    }

    #[test]
    fn owned_native_level_four_cave_rejection_matches_reference() {
        let fixture_root = regression_root().join("level4");
        let state_bytes =
            std::fs::read(fixture_root.join("sequence-002285FF-00356038.bin")).unwrap();
        let terrain_bytes =
            std::fs::read(fixture_root.join("sequence-002285FF-002DC4E0.bin")).unwrap();
        let frame = RegressionStateSequence::new(&state_bytes)
            .unwrap()
            .frame(0)
            .unwrap();
        let terrain = LegacyTerrain::from_regression_sequence(&terrain_bytes, 0, true).unwrap();
        let mut actual = frame.project_player_flight(frame.local_player_slot());
        actual.position = Position {
            x: 23_168,
            y: 10_726,
            z: 4_500,
        };
        actual.yaw = 1_024;
        actual.target_speed = 80;
        actual.actual_speed = 80;
        let command = latch_command(&mut actual, PlayerCommand::default());
        let evidence = step_entity_pass_with_context(
            &mut actual,
            command,
            FlightProfile::CAVE,
            &terrain,
            &mut FlightContext::default(),
        );

        assert_eq!(
            actual.position,
            Position {
                x: 23_168,
                y: 10_726,
                z: 4_500,
            }
        );
        assert_eq!(actual.target_speed, 0);
        assert_eq!(actual.actual_speed, 80);
        assert_eq!(actual.water_counter, 0);
        assert!(!actual.recovery_latch);
        assert_eq!(actual.entity_phase, 169);
        assert_eq!(evidence.movement_resolution, "rejected-and-stopped");
    }

    #[test]
    fn owned_native_level_four_cave_recovery_push_matches_reference() {
        let fixture_root = regression_root().join("level4");
        let state_bytes =
            std::fs::read(fixture_root.join("sequence-002285FF-00356038.bin")).unwrap();
        let terrain_bytes =
            std::fs::read(fixture_root.join("sequence-002285FF-002DC4E0.bin")).unwrap();
        let frame = RegressionStateSequence::new(&state_bytes)
            .unwrap()
            .frame(0)
            .unwrap();
        let terrain = LegacyTerrain::from_regression_sequence(&terrain_bytes, 0, true).unwrap();
        let mut actual = frame.project_player_flight(frame.local_player_slot());
        actual.position = Position {
            x: 23_168,
            y: 10_880,
            z: 4_500,
        };
        actual.yaw = 1_024;
        actual.target_speed = 80;
        actual.actual_speed = 80;
        let command = latch_command(&mut actual, PlayerCommand::default());
        let evidence = step_entity_pass_with_context(
            &mut actual,
            command,
            FlightProfile::CAVE,
            &terrain,
            &mut FlightContext::default(),
        );

        assert_eq!(
            actual.position,
            Position {
                x: 23_168,
                y: 11_008,
                z: 4_500,
            }
        );
        assert_eq!(actual.target_speed, 0);
        assert_eq!(actual.actual_speed, 80);
        assert_eq!(actual.water_counter, 0);
        assert!(actual.recovery_latch);
        assert_eq!(actual.entity_phase, 169);
        assert_eq!(evidence.movement_resolution, "recovered-after-reject");
    }

    fn assert_native_status_trace(
        frame: RegressionStateFrame<'_>,
        terrain: &LegacyTerrain,
        status: (u8, i8, i8, i8),
        expected: &[NativeStatusPoint],
    ) {
        let mut actual = frame.project_player_flight(frame.local_player_slot());
        actual.speed_modifier = status.0;
        actual.speed_modifier_counter = status.1;
        actual.immobilize_counter = status.2;
        actual.immobilize_subcounter = status.3;
        let command = latch_command(
            &mut actual,
            PlayerCommand {
                roll: 8,
                pitch: 8,
                flags: 1,
                ..PlayerCommand::default()
            },
        );
        let mut context = FlightContext {
            effects: Some(crate::FlightEffectsContext::default()),
            ..FlightContext::default()
        };
        for (pass, point) in expected.iter().enumerate() {
            step_entity_pass_with_context(
                &mut actual,
                command,
                FlightProfile::OUTDOOR,
                terrain,
                &mut context,
            );
            let message = format!("native status trace pass {}", pass + 1);
            assert_eq!(actual.position, point.position, "{message}");
            assert_eq!(actual.yaw, point.yaw, "{message}");
            assert_eq!(actual.display_pitch, point.rotation, "{message}");
            assert_eq!(actual.effective_pitch, point.rotation, "{message}");
            assert_eq!(actual.roll_accumulator, point.rotation, "{message}");
            assert_eq!(actual.pitch_accumulator, point.rotation, "{message}");
            assert_eq!(actual.target_speed, point.target_speed, "{message}");
            assert_eq!(actual.speed_changed_flag, 1, "{message}");
            assert_eq!(actual.actual_speed, point.actual_speed, "{message}");
            assert_eq!(actual.speed_modifier, point.speed_modifier, "{message}");
            assert_eq!(
                actual.speed_modifier_counter, point.speed_modifier_counter,
                "{message}"
            );
            assert_eq!(
                actual.immobilize_counter, point.immobilize_counter,
                "{message}"
            );
            assert_eq!(
                actual.immobilize_subcounter, point.immobilize_subcounter,
                "{message}"
            );
            assert_eq!(actual.entity_phase, point.entity_phase, "{message}");
        }
    }

    #[test]
    fn owned_after_load_recording_matches_twenty_four_complete_flight_transitions() {
        let sequence = RegressionStateSequence::new(OWNED_AFTER_LOAD_STATE_SEQUENCE).unwrap();
        let recording =
            InputRecording::parse_legacy_regression(OWNED_AFTER_LOAD_RECORDING).unwrap();
        let recorded_player = recording.player(1, 0).unwrap();
        let player_slot = sequence.frame(0).unwrap().local_player_slot();
        let mut actual = sequence
            .frame(0)
            .unwrap()
            .project_player_flight(player_slot);

        for target_frame_index in 1..=24 {
            let after_frame = sequence.frame(target_frame_index).unwrap();
            assert_eq!(after_frame.local_player_slot(), player_slot);
            let decoded = after_frame.decoded_flight_input(player_slot);
            let recorded_turn =
                u32::try_from(after_frame.player_turn(player_slot)).expect("positive fixture turn");
            let raw_command = recorded_player.command(recorded_turn).unwrap();
            let latched = latch_command(&mut actual, raw_command);
            assert_eq!(latched, decoded.command);
            assert_eq!(actual.roll_delta, decoded.roll_delta);
            assert_eq!(actual.pitch_delta, decoded.pitch_delta);
            let terrain = LegacyTerrain::from_regression_sequence(
                OWNED_AFTER_LOAD_TERRAIN_SEQUENCE,
                target_frame_index,
                false,
            )
            .unwrap();
            let mut context = FlightContext::default();
            step_entity_pass_with_context(
                &mut actual,
                latched,
                FlightProfile::OUTDOOR,
                &terrain,
                &mut context,
            );

            let mut expected = after_frame.project_player_flight(player_slot);
            expected.tick = actual.tick;
            assert_eq!(
                actual, expected,
                "flight projection diverged at captured outer frame {target_frame_index}"
            );
        }
    }

    #[test]
    fn all_owned_level_starts_match_their_idle_flight_transitions() {
        let regression_root = regression_root();
        let mut compared_transitions = 0;
        let mut map_type_counts = [0_usize; 3];

        for level in 1..=25 {
            let level_root = regression_root.join(format!("level{level}"));
            let state_bytes =
                std::fs::read(level_root.join("sequence-002285FF-00356038.bin")).unwrap();
            let terrain_bytes =
                std::fs::read(level_root.join("sequence-002285FF-002DC4E0.bin")).unwrap();
            let sequence = RegressionStateSequence::new(&state_bytes).unwrap();
            let first_frame = sequence.frame(0).unwrap();
            let map_type = first_frame.map_type();
            map_type_counts[usize::from(map_type)] += 1;
            let player_slot = first_frame.local_player_slot();
            let player_entity = first_frame.entity(first_frame.player_entity_index(player_slot));
            assert_eq!(
                player_entity.special_settings_index(),
                Some(if map_type == 2 {
                    SOURCE_CAVE_PLAYER_SETTINGS_INDEX
                } else {
                    SOURCE_OUTDOOR_PLAYER_SETTINGS_INDEX
                }),
                "level {level} player special-settings pointer disagrees with AddPlayer_4A920"
            );
            let mut actual = sequence
                .frame(0)
                .unwrap()
                .project_player_flight(player_slot);

            for target_frame_index in 1..sequence.frame_count() {
                let after_frame = sequence.frame(target_frame_index).unwrap();
                let cave_mode = match after_frame.map_type() {
                    0 | 1 => false,
                    2 => true,
                    other => panic!("level {level} has unknown map type {other}"),
                };
                let decoded = after_frame.decoded_flight_input(player_slot);
                decoded.apply(&mut actual);
                let terrain = LegacyTerrain::from_regression_sequence(
                    &terrain_bytes,
                    target_frame_index,
                    cave_mode,
                )
                .unwrap();
                let mut context = FlightContext::default();
                step_entity_pass_with_context(
                    &mut actual,
                    decoded.command,
                    if cave_mode {
                        FlightProfile::CAVE
                    } else {
                        FlightProfile::OUTDOOR
                    },
                    &terrain,
                    &mut context,
                );
                let mut expected = after_frame.project_player_flight(player_slot);
                expected.tick = actual.tick;
                assert_eq!(
                    actual, expected,
                    "flight projection diverged at level {level}, captured outer frame {target_frame_index}"
                );
                compared_transitions += 1;
            }
        }
        assert_eq!(compared_transitions, 25 * 19);
        assert_eq!(map_type_counts, [8, 9, 8]);
    }

    #[test]
    fn owned_midgame_save_matches_nineteen_idle_flight_transitions() {
        let fixture_root = regression_root().join("afterloadtest1");
        let state_bytes =
            std::fs::read(fixture_root.join("sequence-002285FF-00356038.bin")).unwrap();
        let terrain_bytes =
            std::fs::read(fixture_root.join("sequence-002285FF-002DC4E0.bin")).unwrap();
        let sequence = RegressionStateSequence::new(&state_bytes).unwrap();
        let player_slot = sequence.frame(0).unwrap().local_player_slot();
        let mut actual = sequence
            .frame(0)
            .unwrap()
            .project_player_flight(player_slot);

        for target_frame_index in 1..sequence.frame_count() {
            let after_frame = sequence.frame(target_frame_index).unwrap();
            let cave_mode = after_frame.map_type() == 2;
            let decoded = after_frame.decoded_flight_input(player_slot);
            decoded.apply(&mut actual);
            let terrain = LegacyTerrain::from_regression_sequence(
                &terrain_bytes,
                target_frame_index,
                cave_mode,
            )
            .unwrap();
            let mut context = FlightContext::default();
            step_entity_pass_with_context(
                &mut actual,
                decoded.command,
                if cave_mode {
                    FlightProfile::CAVE
                } else {
                    FlightProfile::OUTDOOR
                },
                &terrain,
                &mut context,
            );
            let mut expected = after_frame.project_player_flight(player_slot);
            expected.tick = actual.tick;
            assert_eq!(
                actual, expected,
                "midgame flight projection diverged at captured outer frame {target_frame_index}"
            );
        }
        assert_eq!(sequence.frame_count() - 1, 19);
    }

    fn regression_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions",
        )
    }
}
