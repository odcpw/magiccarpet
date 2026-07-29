//! Deterministic, renderer-free private simulation core.

use std::cmp::Ordering;

use spell_activation::{SpellActivationHolder, SpellCasterState, activate_spell};

pub mod archer_tmap;
pub mod combat_tmap;
pub mod economy_tmap;
pub mod firefly_tmap;
pub mod frozen_actor_tmap;
pub mod goat;
pub mod goat_draw_admission;
pub mod goat_map_chain;
pub mod goat_sprite_raster;
pub mod goat_tmap;
pub mod house;
pub mod legacy_terrain;
pub mod level1_mission;
pub mod level1_parity;
pub mod mode26_raster;
pub mod mode5_raster;
pub mod music;
pub mod native_world;
pub mod recording;
pub mod regression_state;
pub mod render_camera;
pub mod renderable_map_chain;
pub mod spell_activation;
mod ss2_tmap;
pub mod static_world_tmap;
pub mod terrain_frame;
pub mod terrain_grid;
pub mod terrain_presentation;
pub mod terrain_reflection;
pub mod villager;

include!(concat!(env!("OUT_DIR"), "/trig_table.rs"));

pub const ANGLE_MASK: i16 = 0x07ff;
pub const POSITIONAL_SOUND_MAX_DISTANCE_SQUARED: u32 = 0x0900_0000;
pub const POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING: u8 = 2;
pub const WATER_AMBIENCE_MASK: u32 = 0x0001;
pub const MOVEMENT_REDIRECT_MASK: u32 = 0x0100;
pub const SPEED_STEP: i16 = SOURCE_SPEED_STEP;
pub const SPEED_LIMIT: i16 = SOURCE_FORWARD_LIMIT;
pub const STRAFE_STEP: i16 = SOURCE_STRAFE_STEP;
pub const STRAFE_DECAY: i16 = -SOURCE_STRAFE_DECAY_STEP;
pub const RECOIL_DECAY: i16 = -SOURCE_RECOIL_STEP;
pub const RECOIL_POSITIVE_CAP: i16 = 128;
pub const IMMOBILIZED_VERTICAL_STEP: i16 = -51;
pub const PLAYER_MIN_SPEED: i16 = SOURCE_FORWARD_LIMIT;
pub const TETHER_SPEED_LIMIT: i16 = 3 * PLAYER_MIN_SPEED / 2;

const _: () = assert!(SOURCE_REVERSE_LIMIT == -SOURCE_FORWARD_LIMIT);
const _: () = assert!(SOURCE_STRAFE_MINIMUM == -SOURCE_STRAFE_MAXIMUM);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Position {
    pub x: u16,
    pub y: u16,
    pub z: i16,
}

/// Original MC2 integer stereo command derived by `PrepareEventSound_6E450`.
///
/// Despite the upstream helper's `EuclideanDistXYZ` name, the recovered
/// implementation used by sound attenuation squares only X and Y. Z remains
/// part of the source-relative position for provenance and later consumers,
/// but it cannot change cutoff, attenuation, or pan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionalSoundCommand {
    pub relative_x: i32,
    pub relative_y: i32,
    pub relative_z: i32,
    pub distance_xy: u32,
    pub relative_yaw: u16,
    pub volume_q15: u16,
    pub pan_q16: u16,
    pub rate_delta_percent: i16,
    pub sound_flags: i16,
    pub audible: bool,
}

/// Source playback policy produced by `PrepareEventSound_6E450`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedEventSoundCommand {
    pub command: PositionalSoundCommand,
    pub play_type: u8,
}

/// Calculate the bounded positional command for a sound routed through
/// remc2's `PrepareEventSound_6E450`.
///
/// # Panics
///
/// Panics only if the recovered 11-bit angle-distance helper violates its
/// proven non-negative 0..1024 range.
#[must_use]
pub fn prepare_event_sound_command(
    listener: Position,
    listener_yaw: i16,
    source: Position,
    sound_id: u16,
    random_state: u32,
    source_entity_id: u16,
    source_action: u8,
) -> Option<PreparedEventSoundCommand> {
    let play_type = match sound_id {
        3
        | 4
        | 6
        | 9..=11
        | 15
        | 18..=28
        | 30
        | 38
        | 40
        | 41
        | 48
        | 50..=53
        | 60
        | 61
        | 63
        | 64 => 1,
        7 | 8 | 12 | 13 | 16 | 17 | 32..=34 | 37 | 39 | 42..=46 | 54..=59 | 62 => 3,
        47 | 49 => 4,
        // Select/CantUse use play type 1 for both the current-level and
        // entity-local branches. Ambient loops 1/2/5/31 are handled by their
        // separate level-local envelope contract.
        14 | 29 => 1,
        _ => return None,
    };

    let relative_x = i32::from(source.x) - i32::from(listener.x);
    let relative_y = i32::from(source.y) - i32::from(listener.y);
    let relative_z = i32::from(source.z) - i32::from(listener.z);
    let dx = i32::from(source.x.wrapping_sub(listener.x).cast_signed());
    let dy = i32::from(source.y.wrapping_sub(listener.y).cast_signed());
    let distance_squared = square_wrapped_i16(dx).wrapping_add(square_wrapped_i16(dy));
    let sound_flags = if matches!(
        sound_id,
        7 | 32 | 38 | 42..=44 | 46 | 47 | 49..=53 | 58 | 59 | 62
    ) {
        0
    } else {
        source_entity_id.cast_signed()
    };
    let random_value = random_state.wrapping_mul(9_377).wrapping_add(9_439);
    let rate_delta_percent = match sound_id {
        42..=44 => i16::try_from(random_value % 30).expect("modulo 30 fits i16") - 15,
        46 if source_action == 14 => {
            i16::try_from(random_value % 20).expect("modulo 20 fits i16") + 10
        }
        46 => i16::try_from(random_value % 20).expect("modulo 20 fits i16") - 10,
        _ => 0,
    };

    if distance_squared > POSITIONAL_SOUND_MAX_DISTANCE_SQUARED {
        return Some(PreparedEventSoundCommand {
            command: PositionalSoundCommand {
                relative_x,
                relative_y,
                relative_z,
                distance_xy: legacy_sqrt(distance_squared),
                relative_yaw: 0,
                volume_q15: 0,
                pan_q16: 0x7fff,
                rate_delta_percent: 0,
                sound_flags,
                audible: false,
            },
            play_type,
        });
    }

    let distance_xy = legacy_sqrt(distance_squared);
    let target_angle = legacy_atan(
        wrapping_i16(i64::from(relative_x)),
        wrapping_i16(i64::from(relative_y)),
    );
    let relative_yaw = u16::try_from(angle_distance(
        i32::from(listener_yaw),
        i32::from(target_angle),
    ))
    .expect("11-bit yaw distance is non-negative");
    let attenuation_radius = (12_288_i32 * (1_024 - i32::from(relative_yaw) / 2)) >> 10;
    // The original parent expression explicitly cast the wrapping 32-bit
    // product to signed before division. A later remc2 cleanup dropped that
    // cast; preserving it is what makes the 512 threshold vectors agree.
    let distance_delta = attenuation_radius
        .wrapping_sub(i32::try_from(distance_xy).expect("sound distance fits i32"));
    let numerator = 0x7fff_u32
        .wrapping_mul(distance_delta.cast_unsigned())
        .cast_signed();
    let volume = numerator / attenuation_radius;
    if volume < 512 {
        return Some(PreparedEventSoundCommand {
            command: PositionalSoundCommand {
                relative_x,
                relative_y,
                relative_z,
                distance_xy,
                relative_yaw,
                volume_q15: u16::try_from(volume.max(0)).unwrap_or(0),
                pan_q16: 0x7fff,
                rate_delta_percent: 0,
                sound_flags,
                audible: false,
            },
            play_type,
        });
    }
    let volume_q15 = u16::try_from(volume.min(0x7fff)).expect("clamped positional volume fits u16");

    let pan_q16 = if distance_xy > 320 {
        let folded_yaw = if relative_yaw > 512 {
            1_024 - relative_yaw
        } else {
            relative_yaw
        };
        let side = i32::from(turn_direction(listener_yaw, target_angle));
        let pan = 0x7fff_i32 + i32::from(folded_yaw) * 64 * side;
        u16::try_from(pan.clamp(0, 0xffff)).expect("clamped positional pan fits u16")
    } else {
        0x7fff
    };
    Some(PreparedEventSoundCommand {
        command: PositionalSoundCommand {
            relative_x,
            relative_y,
            relative_z,
            distance_xy,
            relative_yaw,
            volume_q15,
            pan_q16,
            rate_delta_percent,
            sound_flags,
            audible: true,
        },
        play_type,
    })
}

/// Compatibility projection for the three periodic Level 1 effect mappings
/// that were qualified before the complete event-sound contract.
#[must_use]
pub fn positional_sound_command(
    listener: Position,
    listener_yaw: i16,
    source: Position,
    sound_id: u16,
    random_state: u32,
    source_entity_id: u16,
) -> Option<PositionalSoundCommand> {
    matches!(sound_id, 37 | 39 | 43)
        .then(|| {
            prepare_event_sound_command(
                listener,
                listener_yaw,
                source,
                sound_id,
                random_state,
                source_entity_id,
                0,
            )
        })
        .flatten()
        .map(|prepared| prepared.command)
}

/// Exact `ShouldUpdateSound_6EA90` candidate rule used before a sound slot is
/// handed to the playback voice allocator.
#[must_use]
pub const fn positional_sound_should_replace(
    candidate_volume_q15: i32,
    current_volume_q15: i32,
) -> bool {
    candidate_volume_q15 - current_volume_q15 >= -8
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlayerCommand {
    pub opcode: i8,
    pub argument_1: i8,
    pub argument_2: i8,
    pub roll: i8,
    pub pitch: i8,
    pub flags: i8,
    pub argument_6: u16,
    pub argument_8: u16,
}

const _: () = assert!(size_of::<PlayerCommand>() == 10);

impl PlayerCommand {
    #[must_use]
    pub fn to_bytes(self) -> [u8; 10] {
        let argument_6 = self.argument_6.to_le_bytes();
        let argument_8 = self.argument_8.to_le_bytes();
        [
            self.opcode.cast_unsigned(),
            self.argument_1.cast_unsigned(),
            self.argument_2.cast_unsigned(),
            self.roll.cast_unsigned(),
            self.pitch.cast_unsigned(),
            self.flags.cast_unsigned(),
            argument_6[0],
            argument_6[1],
            argument_8[0],
            argument_8[1],
        ]
    }

    #[must_use]
    pub fn from_bytes(bytes: [u8; 10]) -> Self {
        Self {
            opcode: bytes[0].cast_signed(),
            argument_1: bytes[1].cast_signed(),
            argument_2: bytes[2].cast_signed(),
            roll: bytes[3].cast_signed(),
            pitch: bytes[4].cast_signed(),
            flags: bytes[5].cast_signed(),
            argument_6: u16::from_le_bytes([bytes[6], bytes[7]]),
            argument_8: u16::from_le_bytes([bytes[8], bytes[9]]),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LatchedCommand {
    pub flags: u8,
    pub argument_6: u16,
    pub argument_8: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlightProfile {
    pub cave_mode: bool,
    pub terrain_reference_height: i16,
    pub ground_clearance: i16,
    pub ceiling_clearance: i16,
    pub passive_vertical_step: i16,
    pub raw_word_0x10: i16,
    pub raw_word_0x12: i16,
}

impl FlightProfile {
    pub const OUTDOOR: Self = Self {
        cave_mode: false,
        terrain_reference_height: SOURCE_OUTDOOR_PLAYER_SETTINGS[5],
        ground_clearance: SOURCE_OUTDOOR_PLAYER_SETTINGS[6],
        ceiling_clearance: 384,
        passive_vertical_step: SOURCE_OUTDOOR_PLAYER_SETTINGS[7],
        raw_word_0x10: SOURCE_OUTDOOR_PLAYER_SETTINGS[8],
        raw_word_0x12: SOURCE_OUTDOOR_PLAYER_SETTINGS[9],
    };
    pub const CAVE: Self = Self {
        cave_mode: true,
        terrain_reference_height: SOURCE_CAVE_PLAYER_SETTINGS[5],
        ground_clearance: SOURCE_CAVE_PLAYER_SETTINGS[6],
        ceiling_clearance: 384,
        passive_vertical_step: SOURCE_CAVE_PLAYER_SETTINGS[7],
        raw_word_0x10: SOURCE_CAVE_PLAYER_SETTINGS[8],
        raw_word_0x12: SOURCE_CAVE_PLAYER_SETTINGS[9],
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlightState {
    pub entity_index: u16,
    pub position: Position,
    pub yaw: i16,
    pub display_pitch: i16,
    pub effective_pitch: i16,
    pub roll_accumulator: i16,
    pub pitch_accumulator: i16,
    pub target_speed: i16,
    /// Transient `word_0xe_14`; `sub_5F380` clears it before input decoding
    /// and sets it only when forward/reverse changes the target speed.
    pub speed_changed_flag: i16,
    pub actual_speed: i16,
    pub strafe_speed: i16,
    pub roll_delta: i16,
    pub pitch_delta: i16,
    pub recoil_speed: i16,
    pub recoil_yaw: i16,
    pub impulse_x: i16,
    pub impulse_y: i16,
    pub impulse_z: i16,
    pub entity_fov: i16,
    pub water_counter: i8,
    pub speed_modifier: u8,
    pub speed_modifier_counter: i8,
    pub immobilize_counter: i8,
    pub immobilize_subcounter: i8,
    pub tether_prior_distance: i32,
    pub tether_target_index: u16,
    pub tether_holder_index: u16,
    pub tether_tier: u16,
    pub speed_spell_holder_index: u16,
    /// Entity field `byte_0x3E_62`, incremented after every dispatch pass.
    pub entity_phase: u8,
    /// Transient `dword_0x19A_410`, consumed by ambient loop 5.
    pub ambient_loop_5_distance: i32,
    /// Transient `maxDistance_0x19E_414`, consumed by ambient loop 31.
    pub ambient_loop_31_distance: i32,
    pub player_color_index: u8,
    pub music_countdown: i16,
    /// `byte_0x261_609`, retained by the rejected-movement recovery path.
    pub recovery_latch: bool,
    pub skip_flight_once: bool,
    pub tick: u64,
}

impl Default for FlightState {
    fn default() -> Self {
        Self {
            entity_index: 0,
            position: Position {
                x: 32_768,
                y: 32_768,
                z: 1_280,
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
            entity_phase: 0,
            ambient_loop_5_distance: 0,
            ambient_loop_31_distance: 0,
            player_color_index: 0,
            music_countdown: 0,
            recovery_latch: false,
            skip_flight_once: false,
            tick: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Tile {
    pub x: u8,
    pub y: u8,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TetherTarget {
    pub entity_index: u16,
    pub position: Position,
    pub life: i32,
    pub mana: i32,
    pub mana_regen: i32,
    pub life_regen: i16,
}

/// The minimum cross-entity projection needed by `sub_5DE30`.
///
/// The normal step functions accept only inactive cross-entity interactions.
/// Active cases use the explicit `*_with_context` entry points so target
/// mutations cannot be silently discarded by a presentation host.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TetherContext {
    pub holder: Option<SpellHolder>,
    pub target: Option<TetherTarget>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SpellHolder {
    pub entity_index: u16,
    /// Original entity field `word_0x2E_46`; zero means inactive.
    pub active_value: i16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FlightContext {
    pub tether: TetherContext,
    pub speed_spell_holder: Option<SpellHolder>,
    pub effects: Option<FlightEffectsContext>,
    pub spell_fire: Option<SpellFireContext>,
    pub events: Vec<FlightEvent>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FlightEffectsContext {
    pub global_rng: u32,
    pub timing_salt: u8,
    pub local_player_index: u8,
    pub palette_submode: u8,
    pub move_speed_flag: u8,
    /// `life_0x1A` selected through active spell slot 4, when valid.
    pub periodic_spell_effect: Option<u8>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SpellFireContext {
    pub left_holder_index: u16,
    pub right_holder_index: u16,
    pub selected_holder_index: u16,
    /// Source `SubSpellIndexLeft_1109`.
    pub left_subspell_index: i8,
    /// Source `SubSpellIndexRight_1110`.
    pub right_subspell_index: i8,
    pub selected_spell_ui_index: u8,
    /// Source spell-menu cursor `subSpellIndex_0x459_1113`.
    pub selected_subspell_index: i8,
    /// Source two-stage menu state `byte_0x457_1111`: 0 idle, 1 left, 2 right.
    pub spell_menu_stage: u8,
    pub enabled_holder_indices: [u16; 26],
    /// Source per-family hand eligibility `array_0x3B5_949x`: 1 left, 2 right.
    pub spell_hand_eligibility: [u8; 26],
    /// Source per-family current tier `array_0x437_1079x`.
    ///
    /// This is deliberately distinct from each holder's applied
    /// `byte_0x46_70`: final assignment opcodes preserve their raw tier in
    /// player state while `SetSpell_6D5E0` clamps only the physical holder.
    pub family_selected_tiers: [i8; 26],
    /// Single-player `spellsExperience_0x2CB_715x`.
    pub spell_experience: [i32; 26],
    /// Recomputed `SpellLevels_0x41D_1053z`.
    pub spell_levels: [i8; 26],
    pub caster: SpellCasterState,
    /// One registry preserves source aliasing when multiple fire selections
    /// resolve to the same holder entity.
    pub holders: Vec<SpellActivationHolder>,
    /// Transient results from the current latched command. The source spell
    /// handlers consume successful `sub_5F660` activations later in the same
    /// entity update; keeping the result avoids mistaking a repeated reset
    /// value for a rejected cast.
    pub accepted_models: Vec<u8>,
    pub help_enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpellFireSource {
    Left,
    Right,
    Selected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlightEvent {
    PrepareSound {
        entity_index: u16,
        player_index: i16,
        sound_id: u16,
    },
    EndLoop {
        entity_index: u16,
        player_index: i16,
        sound_id: u16,
    },
    UpdateMusic {
        mode: u8,
        timer_multiplier: u8,
    },
    RequestSpellActivation {
        source: SpellFireSource,
        holder_entity_index: u16,
        direction_mask: u16,
    },
    ShowHelpHint {
        hint_index: u16,
        state: u8,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MovementResolution {
    Accept(Position),
    Redirect(Position),
    /// Reject without the cave-path target-speed mutation.
    Reject,
    /// Reject and apply the cave/custom collision stop mutation.
    RejectAndStop,
}

pub trait FlightWorld {
    fn ground_height(&self, position: Position) -> i16;
    fn ceiling_height(&self, position: Position) -> Option<i16>;
    fn material_mask(&self, position: Position) -> u32;
    fn terrain_type(&self, position: Position) -> u8 {
        match self.material_mask(position) {
            1 => 0,
            2 => 1,
            4 => 2,
            8 => 3,
            0x10 => 4,
            0x20 => 5,
            0x1_0000 => 26,
            0x10_0000 => 10,
            0x20_0000 => 11,
            _ => 6,
        }
    }
    fn angle_flags(&self, tile: Tile) -> u8;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepEvidence {
    pub before: Position,
    pub proposed: Position,
    pub committed: Position,
    pub movement_resolution: &'static str,
    pub state_hash: u64,
}

#[must_use]
pub fn derive_input_delta(command_axis: i8, accumulated: i16) -> i16 {
    let numerator = i32::from(command_axis) * 2 - i32::from(accumulated);
    let result = numerator / 4;
    i16::try_from(result).unwrap_or(if result.is_negative() {
        i16::MIN
    } else {
        i16::MAX
    })
}

/// Latch one raw command when no cross-entity full-stop mutation is active.
///
/// # Panics
///
/// Panics when opcode `0x27` must clear an active speed-spell holder. Use
/// [`latch_command_with_context`] for that source branch.
pub fn latch_command(state: &mut FlightState, command: PlayerCommand) -> LatchedCommand {
    assert!(
        !(command.opcode == 0x27 && command.argument_1 != 0 && state.speed_spell_holder_index != 0),
        "full-stop with an active speed spell requires latch_command_with_context"
    );
    let mut context = FlightContext::default();
    latch_command_with_context(state, command, &mut context)
}

/// Apply the movement-relevant portion of `PlayerEvents_51BB0`, including its
/// full-stop opcode and cross-entity speed-spell mutation.
///
/// # Panics
///
/// Panics when a source-valid active speed-spell index has no matching holder
/// projection in `context`.
pub fn latch_command_with_context(
    state: &mut FlightState,
    mut command: PlayerCommand,
    context: &mut FlightContext,
) -> LatchedCommand {
    if let Some(fire) = context.spell_fire.as_mut() {
        fire.accepted_models.clear();
    }
    if command.opcode == 6
        && command.flags & 0x40 != 0
        && let Some(fire) = context.spell_fire.as_mut()
    {
        fire.selected_spell_ui_index = command.argument_1.cast_unsigned();
        let spell_index = SOURCE_SPELL_INDEX_MAP
            .get(usize::from(fire.selected_spell_ui_index))
            .copied()
            .expect("source-valid selected-spell UI index");
        fire.selected_holder_index = fire.enabled_holder_indices[usize::from(spell_index)];
        fire.selected_subspell_index = fire
            .holders
            .iter()
            .find(|holder| holder.entity_index == fire.selected_holder_index)
            .map_or(0, |holder| holder.level_gate);
    }
    apply_spell_menu_command(command, context);
    if command.opcode == 0x27 {
        command.roll = 0;
        command.pitch = 0;
        if command.argument_1 != 0 {
            state.actual_speed = 0;
            state.target_speed = 0;
            if state.speed_spell_holder_index != 0 {
                let holder_index = state.speed_spell_holder_index;
                let holder = context
                    .speed_spell_holder
                    .as_mut()
                    .filter(|holder| holder.entity_index == holder_index)
                    .expect("source-valid speed-spell holder is projected");
                holder.active_value = 0;
                let fire_holder = context
                    .spell_fire
                    .as_mut()
                    .and_then(|fire| {
                        fire.holders
                            .iter_mut()
                            .find(|holder| holder.entity_index == holder_index)
                    })
                    .expect("source-valid speed-spell holder is in the spell registry");
                fire_holder.active_value = 0;
            }
        }
    }
    state.roll_delta = derive_input_delta(command.roll, state.roll_accumulator);
    state.pitch_delta = derive_input_delta(command.pitch, state.pitch_accumulator);
    LatchedCommand {
        flags: command.flags.cast_unsigned(),
        argument_6: command.argument_6,
        argument_8: command.argument_8,
    }
}

fn apply_spell_menu_command(command: PlayerCommand, context: &mut FlightContext) {
    match command.opcode {
        40 => {
            if let Some(fire) = context.spell_fire.as_mut()
                && let Some((ui_index, spell_index)) = spell_menu_indices(command.argument_1)
                && fire.enabled_holder_indices[spell_index] != 0
            {
                fire.selected_spell_ui_index = ui_index;
                fire.selected_holder_index = fire.enabled_holder_indices[spell_index];
                fire.selected_subspell_index = fire.family_selected_tiers[spell_index];
                fire.spell_menu_stage = command.argument_2.cast_unsigned();
            }
        }
        41 => {
            if let Some(fire) = context.spell_fire.as_mut()
                && let Some((_, spell_index)) =
                    spell_menu_indices(fire.selected_spell_ui_index.cast_signed())
                && fire.enabled_holder_indices[spell_index] != 0
            {
                let unlocked_tier = fire.spell_levels[spell_index].clamp(0, 2);
                let tier = command.argument_1.clamp(0, unlocked_tier);
                fire.selected_subspell_index = tier;
                fire.family_selected_tiers[spell_index] = tier;
            }
        }
        31 | 32 => {
            if let Some(fire) = context.spell_fire.as_mut()
                && let Some((_, spell_index)) = spell_menu_indices(command.argument_1)
            {
                let holder_index = fire.enabled_holder_indices[spell_index];
                if holder_index != 0 {
                    let raw_tier = command.argument_2;
                    let holder = fire
                        .holders
                        .iter_mut()
                        .find(|holder| holder.entity_index == holder_index)
                        .expect("enabled spell family has a projected holder");
                    holder.level_gate = raw_tier.clamp(0, 2);
                    fire.family_selected_tiers[spell_index] = raw_tier;
                    if command.opcode == 31 {
                        fire.left_holder_index = holder_index;
                        fire.left_subspell_index = raw_tier;
                    } else {
                        fire.right_holder_index = holder_index;
                        fire.right_subspell_index = raw_tier;
                    }
                    fire.spell_menu_stage = 0;
                    context.events.push(FlightEvent::PrepareSound {
                        entity_index: 0,
                        player_index: i16::from(fire.caster.player_color_index),
                        sound_id: 14,
                    });
                }
            }
        }
        38 => {
            if let Some(fire) = context.spell_fire.as_mut()
                && let Ok(spell_index) = usize::try_from(command.argument_1)
                && spell_index < fire.spell_hand_eligibility.len()
            {
                fire.spell_hand_eligibility[spell_index] = command.argument_2.cast_unsigned();
                context.events.push(FlightEvent::PrepareSound {
                    entity_index: 0,
                    player_index: i16::from(fire.caster.player_color_index),
                    sound_id: 14,
                });
            }
        }
        _ => {}
    }
}

fn spell_menu_indices(argument: i8) -> Option<(u8, usize)> {
    let ui_index = u8::try_from(argument).ok()?;
    let spell_index = SOURCE_SPELL_INDEX_MAP.get(usize::from(ui_index)).copied()?;
    Some((ui_index, usize::from(spell_index)))
}

pub fn decode_movement(flags: u8, state: &mut FlightState) {
    state.speed_changed_flag = 0;
    let mut forward_direction = 0_i16;
    if flags & 0b0000_0001 != 0 && state.target_speed < SPEED_LIMIT {
        forward_direction = 1;
    }
    if flags & 0b0000_0010 != 0 && state.target_speed > -SPEED_LIMIT {
        forward_direction = -1;
    }
    if forward_direction != 0 {
        state.target_speed = state
            .target_speed
            .saturating_add(SPEED_STEP * forward_direction)
            .clamp(-SPEED_LIMIT, SPEED_LIMIT);
        state.speed_changed_flag = 1;
    }

    let strafe_negative = flags & 0b0000_0100 != 0;
    let strafe_positive = flags & 0b0000_1000 != 0;
    let strafe_direction = if strafe_positive {
        1
    } else if strafe_negative {
        -1
    } else {
        0
    };
    if strafe_direction != 0 {
        state.strafe_speed = state
            .strafe_speed
            .saturating_add(STRAFE_STEP * strafe_direction)
            .clamp(SOURCE_STRAFE_MINIMUM, SOURCE_STRAFE_MAXIMUM);
    } else {
        state.strafe_speed = approach(state.strafe_speed, 0, STRAFE_DECAY);
    }
}

/// Advance one entity-update pass. Schedulers may invoke one, four, or eight
/// passes with the same latched command. Wall-clock delta time is never an
/// input.
///
/// # Panics
///
/// Panics when an active cross-entity flight interaction or spell-fire input
/// is present because this convenience entry point has no corresponding
/// projection. Use [`step_entity_pass_with_context`] for those source
/// branches.
pub fn step_entity_pass<W: FlightWorld>(
    state: &mut FlightState,
    command: LatchedCommand,
    profile: FlightProfile,
    world: &W,
) -> StepEvidence {
    assert_eq!(
        state.tether_target_index, 0,
        "active tether requires step_entity_pass_with_context"
    );
    assert_eq!(
        state.speed_spell_holder_index, 0,
        "active speed spell requires step_entity_pass_with_context"
    );
    assert_eq!(
        command.flags & 0x70,
        0,
        "spell-fire input requires step_entity_pass_with_context"
    );
    step_entity_pass_with_context(
        state,
        command,
        profile,
        world,
        &mut FlightContext::default(),
    )
}

/// Advance one entity pass with the exact cross-entity projection available.
///
/// # Panics
///
/// Panics when spell-fire input is present without the holder indices selected
/// by the source command-dispatch path in `context.spell_fire`.
pub fn step_entity_pass_with_context<W: FlightWorld>(
    state: &mut FlightState,
    command: LatchedCommand,
    profile: FlightProfile,
    world: &W,
    context: &mut FlightContext,
) -> StepEvidence {
    let before = state.position;
    decode_movement(command.flags, state);
    journal_spell_fire(command.flags, context);

    if state.skip_flight_once {
        state.skip_flight_once = false;
        finish_entity_pass(state);
        return StepEvidence {
            before,
            proposed: before,
            committed: before,
            movement_resolution: "skipped",
            state_hash: state_hash(state),
        };
    }

    state.roll_accumulator = state
        .roll_accumulator
        .wrapping_add(modified_control_delta(state, state.roll_delta));
    state.pitch_accumulator = state
        .pitch_accumulator
        .wrapping_add(modified_control_delta(state, state.pitch_delta));
    state.yaw = wrap_angle(i32::from(state.yaw) + i32::from(state.roll_accumulator) / 8);
    state.actual_speed = step_speed_without_target_clamp(state.actual_speed, state.target_speed);
    update_terrain_relative_pitch(state, profile, world);

    let mut proposed = before;
    move_entity(
        &mut proposed,
        state.yaw,
        state.effective_pitch,
        modified_translation_speed(state, state.actual_speed),
    );
    move_entity(
        &mut proposed,
        wrap_angle(i32::from(state.yaw) + 512),
        0,
        modified_translation_speed(state, state.strafe_speed),
    );

    state.recoil_speed = state.recoil_speed.min(RECOIL_POSITIVE_CAP);
    move_entity(&mut proposed, state.recoil_yaw, 0, state.recoil_speed);
    state.recoil_speed = decay_recoil(state.recoil_speed);

    proposed.x = proposed.x.wrapping_add_signed(state.impulse_x);
    proposed.y = proposed.y.wrapping_add_signed(state.impulse_y);
    proposed.z = proposed.z.wrapping_add(state.impulse_z);
    state.impulse_x = 0;
    state.impulse_y = 0;
    state.impulse_z = 0;
    if state.water_counter != 0 {
        state.water_counter = state.water_counter.wrapping_sub(1);
    }
    apply_tether(state, before, &mut proposed, &mut context.tether);
    advance_status_counters(state, context.effects.as_mut());

    let (mut committed, movement_resolution) =
        match resolve_legacy_movement(state, before, proposed, profile, world) {
            MovementResolution::Accept(position) => (position, "accepted"),
            MovementResolution::Redirect(position) => (position, "redirected"),
            MovementResolution::Reject => {
                return reject_step(state, before, proposed, false, profile, world, context);
            }
            MovementResolution::RejectAndStop => {
                return reject_step(state, before, proposed, true, profile, world, context);
            }
        };

    let ground = i32::from(world.ground_height(committed)) + i32::from(profile.ground_clearance);
    if state.immobilize_counter != 0 {
        committed.z = committed.z.wrapping_add(IMMOBILIZED_VERTICAL_STEP);
    } else if i32::from(committed.z) > ground {
        committed.z = committed.z.wrapping_add(profile.passive_vertical_step);
    }
    if i32::from(committed.z) < ground {
        committed.z = wrapping_i16(i64::from(ground));
    } else if profile.cave_mode
        && let Some(ceiling) = world.ceiling_height(committed)
    {
        let maximum = i32::from(ceiling) - i32::from(profile.ceiling_clearance);
        if i32::from(committed.z) > maximum {
            committed.z = wrapping_i16(i64::from(maximum));
        }
    }

    state.position = committed;
    finish_flight_body(state, profile, world, context);
    finish_entity_pass(state);
    StepEvidence {
        before,
        proposed,
        committed,
        movement_resolution,
        state_hash: state_hash(state),
    }
}

fn reject_step<W: FlightWorld>(
    state: &mut FlightState,
    before: Position,
    proposed: Position,
    stop_target: bool,
    profile: FlightProfile,
    world: &W,
    context: &mut FlightContext,
) -> StepEvidence {
    if stop_target {
        state.target_speed = 0;
        if state.speed_spell_holder_index != 0 {
            let holder_index = state.speed_spell_holder_index;
            let holder = context
                .speed_spell_holder
                .as_mut()
                .filter(|holder| holder.entity_index == holder_index)
                .expect("source-valid speed-spell holder is projected");
            holder.active_value = 0;
            let fire_holder = context
                .spell_fire
                .as_mut()
                .and_then(|fire| {
                    fire.holders
                        .iter_mut()
                        .find(|holder| holder.entity_index == holder_index)
                })
                .expect("source-valid speed-spell holder is in the spell registry");
            fire_holder.active_value = 0;
        }
    }
    let recover = world.material_mask(before) == MOVEMENT_REDIRECT_MASK
        || (profile.cave_mode && world.angle_flags(tile_for(before)) & 8 != 0)
        || (profile.cave_mode
            && state.recovery_latch
            && cave_overlap(world, before, profile, state));
    let (committed, movement_resolution) = if recover {
        state.recovery_latch = true;
        let mut recovered = before;
        move_entity(&mut recovered, state.yaw, 0, 128);
        state.position = recovered;
        (recovered, "recovered-after-reject")
    } else {
        state.recovery_latch = false;
        (
            before,
            if stop_target {
                "rejected-and-stopped"
            } else {
                "rejected"
            },
        )
    };
    finish_flight_body(state, profile, world, context);
    finish_entity_pass(state);
    StepEvidence {
        before,
        proposed,
        committed,
        movement_resolution,
        state_hash: state_hash(state),
    }
}

fn finish_flight_body<W: FlightWorld>(
    state: &mut FlightState,
    profile: FlightProfile,
    world: &W,
    context: &mut FlightContext,
) {
    let FlightContext {
        effects, events, ..
    } = context;
    if let Some(effects) = effects.as_mut() {
        if state.entity_phase.is_multiple_of(64)
            && let Some(effect) = effects.periodic_spell_effect
            && let Some(sound_id) = periodic_spell_sound(effect)
        {
            events.push(FlightEvent::PrepareSound {
                entity_index: state.entity_index,
                player_index: -1,
                sound_id,
            });
        }

        let local_player = i16::from(effects.local_player_index);
        if profile.cave_mode {
            effects.global_rng = effects
                .global_rng
                .wrapping_mul(9_377)
                .wrapping_add(9_439)
                .wrapping_add(u32::from(effects.timing_salt));
            let cave_sound = effects.global_rng % 0x83;
            if cave_sound < 5 && state.player_color_index == effects.local_player_index {
                events.push(FlightEvent::PrepareSound {
                    entity_index: 0,
                    player_index: local_player,
                    sound_id: u16::try_from(cave_sound + 65).expect("cave sound fits u16"),
                });
            }
        } else if world.material_mask(state.position) == WATER_AMBIENCE_MASK {
            events.push(FlightEvent::PrepareSound {
                entity_index: 0,
                player_index: local_player,
                sound_id: 1,
            });
            events.push(FlightEvent::EndLoop {
                entity_index: 0,
                player_index: local_player,
                sound_id: 2,
            });
        } else {
            events.push(FlightEvent::PrepareSound {
                entity_index: 0,
                player_index: local_player,
                sound_id: 2,
            });
            events.push(FlightEvent::EndLoop {
                entity_index: 0,
                player_index: local_player,
                sound_id: 1,
            });
        }

        journal_distance_loop(events, local_player, state.ambient_loop_5_distance, 5);
        journal_distance_loop(events, local_player, state.ambient_loop_31_distance, 31);
        if state.player_color_index == effects.local_player_index {
            let mode = if state.music_countdown <= 0 {
                1
            } else {
                state.music_countdown = state.music_countdown.wrapping_sub(1);
                2
            };
            events.push(FlightEvent::UpdateMusic {
                mode,
                timer_multiplier: music::LEVEL1_TIMER_MULTIPLIER,
            });
        }
    }

    if state.ambient_loop_5_distance < 1_536 {
        state.ambient_loop_5_distance = 2_048;
    }
    if state.ambient_loop_31_distance < 1_536 {
        state.ambient_loop_31_distance = 2_048;
    }
}

fn journal_spell_fire(flags: u8, context: &mut FlightContext) {
    if flags & 0x70 == 0 {
        return;
    }
    let indices = context
        .spell_fire
        .as_ref()
        .map(|fire| {
            (
                fire.left_holder_index,
                fire.right_holder_index,
                fire.selected_holder_index,
            )
        })
        .expect("spell-fire input requires source-selected holder indices");
    if flags & 0x10 != 0 {
        activate_selected_spell(context, SpellFireSource::Left, indices.0, 256);
    }
    if flags & 0x20 != 0 {
        activate_selected_spell(context, SpellFireSource::Right, indices.1, 512);
    }
    if flags & 0x40 != 0 {
        activate_selected_spell(context, SpellFireSource::Selected, indices.2, 256);
    }
}

fn activate_selected_spell(
    context: &mut FlightContext,
    source: SpellFireSource,
    holder_entity_index: u16,
    direction_mask: u16,
) {
    context.events.push(FlightEvent::RequestSpellActivation {
        source,
        holder_entity_index,
        direction_mask,
    });
    let (accepted, holder_model, holder_active_value) = {
        let fire = context
            .spell_fire
            .as_mut()
            .expect("spell-fire context remains present during dispatch");
        let holder = if holder_entity_index == 0 {
            None
        } else {
            fire.holders
                .iter_mut()
                .find(|holder| holder.entity_index == holder_entity_index)
        };
        let holder_model = holder.as_ref().map(|holder| holder.model);
        let accepted = activate_spell(
            &mut fire.caster,
            holder,
            direction_mask,
            fire.help_enabled,
            &mut context.events,
        );
        let holder_active_value = fire
            .holders
            .iter()
            .find(|holder| holder.entity_index == holder_entity_index)
            .map(|holder| holder.active_value);
        if accepted && let Some(model) = holder_model {
            fire.accepted_models.push(model);
        }
        (accepted, holder_model, holder_active_value)
    };
    if accepted
        && holder_model == Some(level1_mission::SPEED_SPELL_FAMILY)
        && let Some(active_value) = holder_active_value
        && let Some(projected) = context
            .speed_spell_holder
            .as_mut()
            .filter(|holder| holder.entity_index == holder_entity_index)
    {
        // The source registry and movement path point at the same holder
        // entity. Keep the two typed Rust projections aliased at mutation
        // time rather than reconciling one stale copy over the other later.
        projected.active_value = active_value;
    }
}

fn finish_entity_pass(state: &mut FlightState) {
    state.entity_phase = state.entity_phase.wrapping_add(1);
    state.tick = state.tick.wrapping_add(1);
}

fn periodic_spell_sound(effect: u8) -> Option<u16> {
    match effect {
        0x02 => Some(12),
        0x10 => Some(39),
        0x13 => Some(43),
        0x19 => Some(37),
        _ => None,
    }
}

fn journal_distance_loop(
    events: &mut Vec<FlightEvent>,
    player_index: i16,
    distance: i32,
    sound_id: u16,
) {
    events.push(if distance >= 1_536 {
        FlightEvent::EndLoop {
            entity_index: 0,
            player_index,
            sound_id,
        }
    } else {
        FlightEvent::PrepareSound {
            entity_index: 0,
            player_index,
            sound_id,
        }
    });
}

#[derive(Clone, Copy)]
struct TetherSpellSpec {
    range: i32,
    effect: u8,
}

// `SPELLS_BEGIN_BUFFER_str[14]` in the hash-pinned `Spells.cpp`.
const TETHER_SPELLS: [TetherSpellSpec; 3] = [
    TetherSpellSpec {
        range: 0x1432,
        effect: 0,
    },
    TetherSpellSpec {
        range: 0x1e28,
        effect: 1,
    },
    TetherSpellSpec {
        range: 0x1e28,
        effect: 2,
    },
];

fn apply_tether(
    state: &mut FlightState,
    authoritative_position: Position,
    proposed: &mut Position,
    context: &mut TetherContext,
) {
    if state.tether_target_index == 0 {
        return;
    }
    let Some(mut target) = context
        .target
        .filter(|target| target.entity_index == state.tether_target_index)
    else {
        // The source leaves the index untouched when the entity lookup itself
        // does not resolve beyond the sentinel.
        return;
    };
    let tier = usize::from(state.tether_tier);
    let spell = TETHER_SPELLS
        .get(tier)
        .copied()
        .expect("source-valid tether tier is in 0..=2");
    let distance = distance_3d_wrapped(authoritative_position, target.position);
    let holder_is_active = state.tether_holder_index != 0
        && context.holder.is_some_and(|holder| {
            holder.entity_index == state.tether_holder_index && holder.active_value != 0
        });
    if !holder_is_active
        || distance >= u32::try_from(spell.range).expect("spell range is non-negative")
        || target.life < 0
    {
        state.tether_target_index = 0;
        return;
    }

    let denominator = 1_024 / i32::from(TETHER_SPEED_LIMIT);
    let raw_speed =
        (i32::try_from(distance).unwrap_or(i32::MAX) - state.tether_prior_distance) / denominator;
    let tether_speed =
        wrapping_i16(i64::from(raw_speed)).clamp(-TETHER_SPEED_LIMIT, TETHER_SPEED_LIMIT);
    let heading = i16::try_from(heading_for_positions(
        authoritative_position,
        target.position,
    ))
    .expect("heading is an 11-bit angle");
    state.yaw = wrap_angle(i32::from(state.yaw) + i32::from(turn_toward(state.yaw, heading, 0x82)));
    move_entity(proposed, heading, state.display_pitch, tether_speed);

    if spell.effect == 2 {
        target.life = target
            .life
            .wrapping_sub(i32::from(target.life_regen).wrapping_add(2));
    }
    if spell.effect == 1 || spell.effect == 2 {
        target.mana = target.mana.wrapping_sub(target.mana_regen.wrapping_add(8));
        if target.mana < 0 {
            target.mana = 0;
        }
    }
    context.target = Some(target);
}

/// Resolve the reconstructed player movement boundary using only semantic
/// terrain queries. This preserves the two-stage material redirection and
/// bounded cave side-probe structure while allowing either a heightfield or
/// voxel provider.
#[allow(clippy::too_many_lines)]
pub fn resolve_legacy_movement<W: FlightWorld>(
    state: &mut FlightState,
    from: Position,
    proposed: Position,
    profile: FlightProfile,
    world: &W,
) -> MovementResolution {
    let mut candidate = proposed;
    let mut redirected = false;
    let cave_mode = profile.cave_mode;
    let mut accepted = true;

    if world.material_mask(candidate) == MOVEMENT_REDIRECT_MASK {
        state.water_counter = state.water_counter.wrapping_add(1);
        let dx = i32::from(candidate.x.wrapping_sub(from.x).cast_signed());
        let dy = i32::from(candidate.y.wrapping_sub(from.y).cast_signed());
        let dz = i32::from(candidate.z.wrapping_sub(from.z));
        let heading = heading_for_delta(dx, dy);
        let horizontal = legacy_distance_2d(dx, dy);
        let pitch = pitch_for_delta(wrapping_i16(i64::from(horizontal)).into(), dz);
        let distance = i32::from(low_u16(legacy_distance_3d(dx, dy, dz)));
        let sector = heading & !0x01ff;
        let directions = [sector, (sector + 512) & 0x07ff];
        let mut found = None;
        let mut last_probe = candidate;
        for direction in directions {
            let difference = angle_distance(heading, direction);
            let weighted = distance * (512 - i32::from(difference)) / 512;
            let mut probe = from;
            move_entity(
                &mut probe,
                i16::try_from(direction).unwrap_or_default(),
                pitch,
                wrapping_i16(i64::from(weighted)),
            );
            last_probe = probe;
            if world.material_mask(probe) != MOVEMENT_REDIRECT_MASK {
                found = Some(probe);
                break;
            }
        }
        if let Some(slid) = found {
            candidate = slid;
            redirected = true;
        } else if !cave_mode {
            return MovementResolution::Reject;
        } else {
            candidate = last_probe;
            accepted = false;
        }
    }

    if cave_mode {
        let ceiling = world.ceiling_height(candidate).unwrap_or(i16::MIN);
        let tile = tile_for(candidate);
        let minimum = i32::from(world.ground_height(candidate))
            + i32::from(profile.ground_clearance)
            + i32::from(state.entity_fov);
        let directly_clear = minimum < i32::from(ceiling) - 576 && world.angle_flags(tile) & 8 == 0;
        if !directly_clear {
            let heading = heading_for_positions(from, candidate);
            let left = (heading - 512) & 0x07ff;
            let right = (heading + 512) & 0x07ff;
            let distances = [16_i16, 32, 64, 112, 176, 256];
            let mut steered = None;
            for (index, distance) in distances.into_iter().enumerate() {
                let left_probe = cave_side_probe(candidate, left, distance);
                let right_probe = cave_side_probe(candidate, right, distance);
                let left_gap = cave_gap(world, left_probe);
                let right_gap = cave_gap(world, right_probe);
                let left_forbidden = world.angle_flags(tile_for(left_probe)) & 8 != 0;
                let right_forbidden = world.angle_flags(tile_for(right_probe)) & 8 != 0;
                let choice = if left_gap > right_gap && !left_forbidden {
                    (!cave_overlap(world, left_probe, profile, state))
                        .then_some((left_probe, -1_i16))
                } else if right_gap > left_gap && !right_forbidden {
                    (!cave_overlap(world, right_probe, profile, state))
                        .then_some((right_probe, 1_i16))
                } else {
                    None
                };
                if let Some((position, direction)) = choice {
                    let magnitude = i16::try_from((17 * (index + 1)) / 6).unwrap_or_default();
                    state.yaw = wrap_angle(i32::from(state.yaw) + i32::from(direction * magnitude));
                    steered = Some(position);
                    break;
                }
            }
            if let Some(position) = steered {
                candidate = position;
                redirected = true;
            } else if cave_overlap(world, candidate, profile, state) {
                accepted = false;
            }
        }
        if world.angle_flags(tile_for(candidate)) & 8 != 0 {
            accepted = false;
        }
    }

    if !accepted {
        MovementResolution::RejectAndStop
    } else if redirected {
        MovementResolution::Redirect(candidate)
    } else {
        MovementResolution::Accept(candidate)
    }
}

/// Latch one raw turn command and advance one entity-update pass.
pub fn step_turn<W: FlightWorld>(
    state: &mut FlightState,
    command: PlayerCommand,
    profile: FlightProfile,
    world: &W,
) -> StepEvidence {
    let command = latch_command(state, command);
    step_entity_pass(state, command, profile, world)
}

/// Latch one raw command and advance one pass with cross-entity state.
pub fn step_turn_with_context<W: FlightWorld>(
    state: &mut FlightState,
    command: PlayerCommand,
    profile: FlightProfile,
    world: &W,
    context: &mut FlightContext,
) -> StepEvidence {
    let command = latch_command_with_context(state, command, context);
    step_entity_pass_with_context(state, command, profile, world, context)
}

/// Translate remc2's outer-loop speed index into entity-update passes.
///
/// The source recognizes only `0`, `1`, and `2`. Every other byte skips the
/// entity-update loop entirely.
#[must_use]
pub const fn entity_passes_for_speed_index(speed_index: u8) -> u8 {
    match speed_index {
        0 => 1,
        1 => 4,
        2 => 8,
        _ => 0,
    }
}

/// Latch one raw outer-turn command and execute remc2's selected entity passes.
pub fn step_outer_turn_with_context<W: FlightWorld>(
    state: &mut FlightState,
    command: PlayerCommand,
    speed_index: u8,
    profile: FlightProfile,
    world: &W,
    context: &mut FlightContext,
) -> Vec<StepEvidence> {
    let command = latch_command_with_context(state, command, context);
    let pass_count = entity_passes_for_speed_index(speed_index);
    let mut evidence = Vec::with_capacity(usize::from(pass_count));
    for _ in 0..pass_count {
        evidence.push(step_entity_pass_with_context(
            state, command, profile, world, context,
        ));
    }
    evidence
}

#[must_use]
pub fn state_hash(state: &FlightState) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    macro_rules! feed {
        ($value:expr) => {
            for byte in $value.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
    }
    feed!(state.entity_index);
    feed!(state.position.x);
    feed!(state.position.y);
    feed!(state.position.z);
    feed!(state.yaw);
    feed!(state.display_pitch);
    feed!(state.effective_pitch);
    feed!(state.roll_accumulator);
    feed!(state.pitch_accumulator);
    feed!(state.target_speed);
    feed!(state.speed_changed_flag);
    feed!(state.actual_speed);
    feed!(state.strafe_speed);
    feed!(state.roll_delta);
    feed!(state.pitch_delta);
    feed!(state.recoil_speed);
    feed!(state.recoil_yaw);
    feed!(state.impulse_x);
    feed!(state.impulse_y);
    feed!(state.impulse_z);
    feed!(state.entity_fov);
    feed!(state.water_counter);
    feed!(state.speed_modifier);
    feed!(state.speed_modifier_counter);
    feed!(state.immobilize_counter);
    feed!(state.immobilize_subcounter);
    feed!(state.tether_prior_distance);
    feed!(state.tether_target_index);
    feed!(state.tether_holder_index);
    feed!(state.tether_tier);
    feed!(state.speed_spell_holder_index);
    feed!(state.entity_phase);
    feed!(state.ambient_loop_5_distance);
    feed!(state.ambient_loop_31_distance);
    feed!(state.player_color_index);
    feed!(state.music_countdown);
    feed!(u8::from(state.recovery_latch));
    feed!(u8::from(state.skip_flight_once));
    feed!(state.tick);
    hash
}

/// Hash only fields projected from the original game state.
///
/// `FlightState::tick` is a native-session lifecycle clock; it has no field in
/// the preserved MC2 entity/player projection. Source comparisons therefore
/// normalize that additive clock while live replay hashes retain it.
#[must_use]
pub fn source_projection_hash(state: &FlightState) -> u64 {
    state_hash(&FlightState { tick: 0, ..*state })
}

/// Hash the complete bounded conformance projection, including state owned by
/// other entities that the flight pass can observe or mutate.
#[must_use]
pub fn conformance_hash(state: &FlightState, context: &FlightContext) -> u64 {
    conformance_hash_with_spell_tiers(state, context, false)
}

/// Versioned live-session conformance projection. This extends the immutable
/// owner-export v1 reset contract with per-hand spell tiers and unlocked
/// family levels without rewriting the preserved package manifest.
#[must_use]
pub fn conformance_hash_v2(state: &FlightState, context: &FlightContext) -> u64 {
    conformance_hash_with_spell_tiers(state, context, true)
}

fn conformance_hash_with_spell_tiers(
    state: &FlightState,
    context: &FlightContext,
    include_spell_tiers: bool,
) -> u64 {
    let mut hash = state_hash(state);
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };

    match context.tether.holder {
        Some(holder) => {
            feed(&[1]);
            feed(&holder.entity_index.to_le_bytes());
            feed(&holder.active_value.to_le_bytes());
        }
        None => feed(&[0]),
    }
    match context.tether.target {
        Some(target) => {
            feed(&[1]);
            feed(&target.entity_index.to_le_bytes());
            feed(&target.position.x.to_le_bytes());
            feed(&target.position.y.to_le_bytes());
            feed(&target.position.z.to_le_bytes());
            feed(&target.life.to_le_bytes());
            feed(&target.mana.to_le_bytes());
            feed(&target.mana_regen.to_le_bytes());
            feed(&target.life_regen.to_le_bytes());
        }
        None => feed(&[0]),
    }
    match context.speed_spell_holder {
        Some(holder) => {
            feed(&[1]);
            feed(&holder.entity_index.to_le_bytes());
            feed(&holder.active_value.to_le_bytes());
        }
        None => feed(&[0]),
    }
    match &context.effects {
        Some(effects) => {
            feed(&[1]);
            feed(&effects.global_rng.to_le_bytes());
            feed(&[
                effects.timing_salt,
                effects.local_player_index,
                effects.palette_submode,
                effects.move_speed_flag,
            ]);
            match effects.periodic_spell_effect {
                Some(effect) => feed(&[1, effect]),
                None => feed(&[0]),
            }
        }
        None => feed(&[0]),
    }
    match &context.spell_fire {
        Some(fire) => {
            feed(&[1]);
            feed_spell_fire(&mut feed, fire, include_spell_tiers);
        }
        None => feed(&[0]),
    }
    feed(
        &u64::try_from(context.events.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for event in &context.events {
        feed_flight_event(&mut feed, *event);
    }
    hash
}

fn feed_spell_fire(
    feed: &mut impl FnMut(&[u8]),
    fire: &SpellFireContext,
    include_spell_tiers: bool,
) {
    feed(&fire.left_holder_index.to_le_bytes());
    feed(&fire.right_holder_index.to_le_bytes());
    feed(&fire.selected_holder_index.to_le_bytes());
    feed(&[fire.selected_spell_ui_index]);
    if include_spell_tiers {
        feed(&[
            fire.left_subspell_index.cast_unsigned(),
            fire.right_subspell_index.cast_unsigned(),
            fire.selected_subspell_index.cast_unsigned(),
        ]);
        if fire.spell_menu_stage != 0
            || fire.family_selected_tiers.iter().any(|tier| *tier != 0)
            || fire
                .spell_hand_eligibility
                .iter()
                .any(|eligibility| *eligibility != 0)
        {
            // Backward-compatible v2 extension: preserve the exact reset byte
            // stream while making every future-affecting non-default menu
            // state explicit and versioned.
            feed(&[1, fire.spell_menu_stage]);
            for tier in fire.family_selected_tiers {
                feed(&[tier.cast_unsigned()]);
            }
            for eligibility in fire.spell_hand_eligibility {
                feed(&[eligibility]);
            }
        }
    }
    for holder_index in fire.enabled_holder_indices {
        feed(&holder_index.to_le_bytes());
    }
    if include_spell_tiers {
        for level in fire.spell_levels {
            feed(&[level.cast_unsigned()]);
        }
    }
    feed(&fire.caster.entity_index.to_le_bytes());
    feed(&[fire.caster.model]);
    feed(&fire.caster.mana.to_le_bytes());
    feed(&fire.caster.control_flags.to_le_bytes());
    feed(&[
        fire.caster.player_color_index,
        fire.caster.control_mode,
        u8::from(fire.help_enabled),
    ]);
    feed(
        &u64::try_from(fire.holders.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for holder in &fire.holders {
        feed(&holder.entity_index.to_le_bytes());
        feed(&[holder.model, holder.level_gate.cast_unsigned()]);
        feed(&holder.active_value.to_le_bytes());
        feed(&holder.activation_value.to_le_bytes());
        feed(&holder.mana_cost.to_le_bytes());
        feed(&[holder.action_marker.cast_unsigned()]);
    }
}

fn feed_flight_event(feed: &mut impl FnMut(&[u8]), event: FlightEvent) {
    match event {
        FlightEvent::PrepareSound {
            entity_index,
            player_index,
            sound_id,
        } => {
            feed(&[0]);
            feed(&entity_index.to_le_bytes());
            feed(&player_index.to_le_bytes());
            feed(&sound_id.to_le_bytes());
        }
        FlightEvent::EndLoop {
            entity_index,
            player_index,
            sound_id,
        } => {
            feed(&[1]);
            feed(&entity_index.to_le_bytes());
            feed(&player_index.to_le_bytes());
            feed(&sound_id.to_le_bytes());
        }
        FlightEvent::UpdateMusic {
            mode,
            timer_multiplier,
        } => feed(&[2, mode, timer_multiplier]),
        FlightEvent::RequestSpellActivation {
            source,
            holder_entity_index,
            direction_mask,
        } => {
            let source_tag = match source {
                SpellFireSource::Left => 0,
                SpellFireSource::Right => 1,
                SpellFireSource::Selected => 2,
            };
            feed(&[3, source_tag]);
            feed(&holder_entity_index.to_le_bytes());
            feed(&direction_mask.to_le_bytes());
        }
        FlightEvent::ShowHelpHint { hint_index, state } => {
            feed(&[4]);
            feed(&hint_index.to_le_bytes());
            feed(&[state]);
        }
    }
}

fn approach(value: i16, target: i16, step: i16) -> i16 {
    match value.cmp(&target) {
        Ordering::Less => value.saturating_add(step).min(target),
        Ordering::Greater => value.saturating_sub(step).max(target),
        Ordering::Equal => value,
    }
}

fn modified_control_delta(state: &FlightState, value: i16) -> i16 {
    if state.speed_modifier == 0 {
        value
    } else {
        wrapping_i16(i64::from(scale_speed_effect(value, state.speed_modifier)))
    }
}

fn modified_translation_speed(state: &FlightState, value: i16) -> i16 {
    if state.speed_modifier != 0 {
        wrapping_i16(i64::from(scale_speed_effect(value, state.speed_modifier)))
    } else if state.immobilize_counter != 0 {
        0
    } else {
        value
    }
}

/// The reconstructed source writes this as a sign-corrected arithmetic shift.
/// `my_sign32` is `-1` only for negative inputs, making the expression exactly
/// a signed division by four with truncation toward zero.
fn scale_speed_effect(value: i16, stage: u8) -> i32 {
    i32::from(value).wrapping_mul(4 - i32::from(stage)) / 4
}

fn advance_status_counters(
    state: &mut FlightState,
    mut effects: Option<&mut FlightEffectsContext>,
) {
    if state.speed_modifier != 0 {
        state.speed_modifier_counter = state.speed_modifier_counter.wrapping_sub(1);
        if state.speed_modifier_counter == 0 {
            state.speed_modifier = state.speed_modifier.wrapping_sub(1);
            if state.speed_modifier != 0 {
                state.speed_modifier_counter = 8;
                if let Some(effects) = effects.as_mut() {
                    effects.move_speed_flag = 1;
                }
            } else if let Some(effects) = effects.as_mut()
                && state.player_color_index == effects.local_player_index
            {
                effects.palette_submode = 1;
            }
        }
    }
    if state.immobilize_counter != 0 {
        state.immobilize_subcounter = state.immobilize_subcounter.wrapping_sub(1);
        if state.immobilize_subcounter == 0 {
            state.immobilize_counter = state.immobilize_counter.wrapping_sub(1);
        }
    }
}

fn update_terrain_relative_pitch<W: FlightWorld>(
    state: &mut FlightState,
    profile: FlightProfile,
    world: &W,
) {
    state.display_pitch = wrap_angle(i32::from(state.pitch_accumulator));
    let signed_pitch = if state.display_pitch > 1_024 {
        state.display_pitch - 2_048
    } else {
        state.display_pitch
    };

    // The reconstructed routine retains the previous movement pitch when
    // either input is zero. This is observable state, not a convenience.
    if state.actual_speed == 0 || signed_pitch == 0 {
        return;
    }

    let reference_height = i32::from(profile.terrain_reference_height);
    if reference_height == 0 {
        return;
    }
    let ground = i32::from(world.ground_height(state.position));
    let altitude_delta = i32::from(state.position.z) - ground - reference_height;
    let altitude_factor = (altitude_delta * 1_024 / reference_height).clamp(-256, 256);
    let pitch = if (state.actual_speed < 0 && signed_pitch < 0)
        || (state.actual_speed > 0 && signed_pitch > 0)
    {
        i32::from(state.pitch_accumulator)
    } else {
        i32::from(signed_pitch) * -altitude_factor / 256
    };
    state.effective_pitch = wrap_angle(pitch);
}

fn step_speed_without_target_clamp(value: i16, target: i16) -> i16 {
    match value.cmp(&target) {
        Ordering::Less => value.wrapping_add(SPEED_STEP),
        Ordering::Greater => value.wrapping_sub(SPEED_STEP),
        Ordering::Equal => value,
    }
}

fn decay_recoil(value: i16) -> i16 {
    if value == 0 {
        return 0;
    }
    let next = if value.is_positive() {
        value.wrapping_sub(RECOIL_DECAY)
    } else {
        value.wrapping_add(RECOIL_DECAY)
    };
    if next.unsigned_abs() < RECOIL_DECAY.cast_unsigned() {
        0
    } else {
        next
    }
}

fn wrap_angle(value: i32) -> i16 {
    i16::try_from(value & i32::from(ANGLE_MASK)).expect("masked angle fits i16")
}

pub(crate) fn move_entity(position: &mut Position, yaw: i16, pitch: i16, speed: i16) {
    if speed == 0 {
        return;
    }
    let yaw = usize::from(yaw.cast_unsigned() & 0x07ff);
    let pitch = usize::from(pitch.cast_unsigned() & 0x07ff);
    let mut horizontal = i32::from(speed);
    if pitch != 0 {
        let dz = (i64::from(horizontal) * i64::from(SIN_16_16[pitch])) >> 16;
        position.z = position.z.wrapping_sub(wrapping_i16(dz));
        horizontal =
            i32::try_from((i64::from(horizontal) * i64::from(SIN_16_16[512 + pitch])) >> 16)
                .expect("projected speed fits i32");
    }
    let dx = (i64::from(horizontal) * i64::from(SIN_16_16[yaw])) >> 16;
    let dy = (i64::from(horizontal) * i64::from(SIN_16_16[512 + yaw])) >> 16;
    position.x = position.x.wrapping_add_signed(wrapping_i16(dx));
    position.y = position.y.wrapping_sub_signed(wrapping_i16(dy));
}

fn tile_for(position: Position) -> Tile {
    Tile {
        x: position.x.to_be_bytes()[0],
        y: position.y.to_be_bytes()[0],
    }
}

fn cave_side_probe(origin: Position, yaw: i32, distance: i16) -> Position {
    let mut probe = origin;
    move_entity(
        &mut probe,
        i16::try_from(yaw).expect("11-bit direction"),
        0,
        distance,
    );
    probe
}

fn cave_gap<W: FlightWorld>(world: &W, position: Position) -> i32 {
    world.ceiling_height(position).map_or(i32::MIN, |ceiling| {
        i32::from(ceiling) - i32::from(world.ground_height(position))
    })
}

fn cave_overlap<W: FlightWorld>(
    world: &W,
    position: Position,
    profile: FlightProfile,
    state: &FlightState,
) -> bool {
    let Some(ceiling) = world.ceiling_height(position) else {
        return false;
    };
    i32::from(world.ground_height(position))
        + i32::from(profile.ground_clearance)
        + i32::from(state.entity_fov)
        > i32::from(ceiling) - i32::from(profile.ceiling_clearance)
}

fn heading_for_positions(from: Position, to: Position) -> i32 {
    heading_for_delta(
        i32::from(to.x.wrapping_sub(from.x).cast_signed()),
        i32::from(to.y.wrapping_sub(from.y).cast_signed()),
    )
}

fn heading_for_delta(dx: i32, dy: i32) -> i32 {
    i32::from(legacy_atan(
        wrapping_i16(i64::from(dx)),
        wrapping_i16(i64::from(dy)),
    ))
}

fn pitch_for_delta(horizontal: i32, dz: i32) -> i16 {
    let horizontal = wrapping_i16(i64::from(horizontal));
    legacy_atan(
        wrapping_i16(-i64::from(wrapping_i16(i64::from(dz)))),
        horizontal.wrapping_neg(),
    )
}

/// Exact port of `Maths::sub_72633_maybe_tan`, including its 257-entry
/// recovered lookup table and quadrant conventions.
fn legacy_atan(first: i16, second: i16) -> i16 {
    if first == 0 && second == 0 {
        return 0;
    }

    let first = i32::from(first);
    let second = i32::from(second);
    let table = |numerator: i32, denominator: i32| -> i32 {
        let index = usize::try_from((numerator << 8) / denominator)
            .expect("legacy tangent index is non-negative");
        i32::from(LEGACY_ATAN[index])
    };

    let result = if first < 0 {
        if second < 0 {
            if -first < -second {
                0x800 - table(-first, -second)
            } else {
                table(-second, -first) + 0x600
            }
        } else if -first < second {
            table(-first, second) + 0x400
        } else {
            0x600 - table(second, -first)
        }
    } else if second < 0 {
        if first < -second {
            table(first, -second)
        } else {
            0x200 - table(-second, first)
        }
    } else if first < second {
        0x400 - table(first, second)
    } else {
        table(second, first) + 0x200
    };

    i16::try_from(result).expect("legacy tangent angle fits i16")
}

fn angle_distance(left: i32, right: i32) -> i16 {
    let direct = (left - right).unsigned_abs() & 0x07ff;
    let wrapped = 2_048_u32 - direct;
    i16::try_from(direct.min(wrapped)).expect("11-bit angle distance fits i16")
}

fn turn_direction(from: i16, to: i16) -> i16 {
    let from = i32::from(from) & 0x07ff;
    let to = i32::from(to) & 0x07ff;
    let mut difference = to - from;
    if difference.unsigned_abs() > 1_024 {
        if difference >= 0 {
            difference -= 2_048;
        } else {
            difference += 2_048;
        }
    }
    difference.signum() as i16
}

fn turn_toward(from: i16, to: i16, maximum: i16) -> i16 {
    if from == to {
        return 0;
    }
    turn_direction(from, to) * angle_distance(i32::from(from), i32::from(to)).min(maximum)
}

fn distance_3d_wrapped(from: Position, to: Position) -> u32 {
    let dx = i32::from(to.x.wrapping_sub(from.x).cast_signed());
    let dy = i32::from(to.y.wrapping_sub(from.y).cast_signed());
    let dz = i32::from(to.z.wrapping_sub(from.z));
    legacy_distance_3d(dx, dy, dz)
}

fn legacy_distance_2d(dx: i32, dy: i32) -> u32 {
    legacy_sqrt(square_wrapped_i16(dx).wrapping_add(square_wrapped_i16(dy)))
}

fn legacy_distance_3d(dx: i32, dy: i32, dz: i32) -> u32 {
    legacy_sqrt(
        square_wrapped_i16(dx)
            .wrapping_add(square_wrapped_i16(dy))
            .wrapping_add(square_wrapped_i16(dz)),
    )
}

/// Exact `Maths::sub_7277A_radix_3d` seeded integer-root loop.
pub(crate) fn legacy_sqrt(value: u32) -> u32 {
    if value == 0 {
        return 0;
    }
    let seed_index = usize::try_from(value.ilog2()).expect("bit index fits usize");
    let mut estimate = u32::from(LEGACY_SQRT_SEED[seed_index]);
    while value / estimate < estimate {
        estimate = (value / estimate + estimate) >> 1;
    }
    estimate
}

fn square_wrapped_i16(value: i32) -> u32 {
    let value = i32::from(wrapping_i16(i64::from(value)));
    u32::try_from(value * value).expect("signed 16-bit square fits u32")
}

fn wrapping_i16(value: i64) -> i16 {
    let bytes = value.to_le_bytes();
    i16::from_le_bytes([bytes[0], bytes[1]])
}

fn low_u16(value: u32) -> u16 {
    u16::try_from(value & u32::from(u16::MAX)).expect("masked value fits u16")
}

#[cfg(test)]
mod tests {
    use super::*;

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
            let command =
                positional_sound_command(listener, 0, listener, sound_id, 13, 152).unwrap();
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
                    entity_index: 42,
                    player_index: -1,
                    sound_id: 12,
                },
                FlightEvent::PrepareSound {
                    entity_index: 0,
                    player_index: 0,
                    sound_id: 2,
                },
                FlightEvent::EndLoop {
                    entity_index: 0,
                    player_index: 0,
                    sound_id: 1,
                },
                FlightEvent::PrepareSound {
                    entity_index: 0,
                    player_index: 0,
                    sound_id: 5,
                },
                FlightEvent::PrepareSound {
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
                    entity_index: 0,
                    player_index: 0,
                    sound_id: 1,
                },
                FlightEvent::EndLoop {
                    entity_index: 0,
                    player_index: 0,
                    sound_id: 2,
                },
                FlightEvent::EndLoop {
                    entity_index: 0,
                    player_index: 0,
                    sound_id: 5,
                },
                FlightEvent::EndLoop {
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
        assert_eq!(fire.selected_subspell_index, 2);
        assert_eq!(fire.spell_menu_stage, 0);
        assert_eq!(fire.family_selected_tiers[0], 2);
        assert_eq!(fire.holders[0].level_gate, 2);
        assert_eq!(
            context.events,
            vec![FlightEvent::PrepareSound {
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
        assert_eq!(step_speed_without_target_clamp(0, 10), 16);
        assert_eq!(step_speed_without_target_clamp(16, 10), 0);
        assert_eq!(step_speed_without_target_clamp(-5, -10), -21);
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
}
