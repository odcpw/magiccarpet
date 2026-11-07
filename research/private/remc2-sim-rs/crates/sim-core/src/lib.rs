//! Deterministic, renderer-free private simulation core.

use std::cmp::Ordering;

use spell_activation::{SpellActivationHolder, SpellCasterState, activate_spell};

pub mod archer_tmap;
pub mod campaign;
pub mod classic_hd;
pub mod classic_hd_display;
pub mod classic_hd_raster;
pub mod classic_hd_render_graph;
pub mod classic_hd_sprites;
pub mod classic_hd_terrain;
pub mod combat;
pub use sim_collision as collision_mask;
pub mod combat_tmap;
pub mod day_indexed_frame;
pub mod dynamic_lighting;
pub mod economy_tmap;
pub mod enhanced_presentation;
pub mod firefly_tmap;
pub mod frozen_actor_tmap;
pub mod game_definition;
pub mod game_runtime;
pub use sim_session as game_session;
pub mod goat;
pub mod goat_draw_admission;
pub mod goat_map_chain;
pub mod goat_sprite_raster;
pub mod goat_tmap;
mod ground_creature_motion;
pub mod house;
pub mod indexed_overlay;
pub mod indexed_postprocess;
pub mod legacy_terrain;
pub mod level1_mission;
pub mod level1_parity;
pub use sim_definition::level_definition;
pub mod level_exit;
pub mod mission_content;
/// Compatibility facade for the exact mode-26 terrain raster contract.
pub mod mode26_raster {
    pub use sim_terrain_raster::mode26_raster::{
        MODE26_BLEND_OFFSET, MODE26_DIRECT_TEXTURE_THRESHOLD, Mode26FramePixel, Mode26Pixel,
        NIGHT_TABLES_BYTES, rasterize_mode26_span, rasterize_mode26_span_from_atlas,
        rasterize_mode26_triangle_from_atlas_into_framebuffer,
        rasterize_mode26_triangle_into_framebuffer,
    };
    pub(crate) use sim_terrain_raster::mode26_raster::{
        rasterize_mode26_triangle_from_tile_atlas_into_framebuffer,
        visit_mode26_triangle_from_tile_atlas_into_framebuffer,
    };
}
/// Compatibility facade for the exact mode-5 terrain raster contract.
pub mod mode5_raster {
    pub use sim_terrain_raster::mode5_raster::{
        Mode5FramePixel, Mode5Pixel, Mode5Scanline, Mode5Span, Mode5TriangleSetup, Mode5Vertex,
        mode5_horizontal_increment, rasterize_mode5_span, rasterize_mode5_span_from_atlas,
        rasterize_mode5_triangle, rasterize_mode5_triangle_from_atlas, setup_mode5_triangle,
    };
    pub(crate) use sim_terrain_raster::mode5_raster::{
        rasterize_mode5_triangle_from_tile_atlas, visit_mode5_triangle_from_tile_atlas,
    };
}
pub mod music;
pub mod native_world;
pub mod presentation;
pub mod presentation_frame;
pub use sim_input::{PlayerCommand, recording};
pub mod regression_state;
pub mod render_camera;
pub mod renderable_map_chain;
pub mod source_sprite_tmap;
pub(crate) mod source_animation;
pub mod spell_activation;
pub mod spells;
mod ss2_tmap;
pub mod static_world_tmap;
pub mod terrain_frame;
pub mod terrain_grid;
pub mod terrain_presentation;
pub mod terrain_reflection;
pub mod villager;
pub(crate) mod world;
pub use sim_voxel::{voxel_collapse, voxel_damage, voxel_snapshot, voxel_state, voxel_structure};

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

/// Definition-bound tuning consumed by the ordinary player flight decoder.
///
/// This is immutable for one session generation. The classic value remains
/// the source-pinned MC2 limit; authoring publishes a new `GameDefinition`
/// and takes effect only when a session is explicitly opened or rebound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlightTuning {
    pub max_forward_speed: i16,
    pub forward_acceleration: i16,
}

impl FlightTuning {
    pub const CLASSIC: Self = Self {
        max_forward_speed: SPEED_LIMIT,
        forward_acceleration: SPEED_STEP,
    };
}

impl Default for FlightTuning {
    fn default() -> Self {
        Self::CLASSIC
    }
}

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
        7 | 8 | 12 | 13 | 16 | 17 | 32..=34 | 37 | 39 | 42..=46 | 54..=59 | 62 | 65..=69 => 3,
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
    /// Opcode 0x2c clears the source chat/name edit buffer before sound 14.
    pub chat_clear_requested: bool,
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
    /// Source toolbar slot -> spell-family bytes written by opcode 0x17.
    pub spell_toolbar_slots: [u8; 26],
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
        #[cfg(test)]
        audio_site: MissionAudioSite,
        entity_index: u16,
        player_index: i16,
        sound_id: u16,
    },
    EndLoop {
        #[cfg(test)]
        audio_site: MissionAudioSite,
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

#[must_use]
pub fn menu_cursor_transition_audio_event(entity_index: u16, player_index: i16) -> FlightEvent {
    mark_source_audio_call(SourceAudioCallTag::MenuCursorTransition);
    FlightEvent::PrepareSound {
        #[cfg(test)]
        audio_site: MissionAudioSite::MenuCursorTransition,
        entity_index,
        player_index,
        sound_id: 14,
    }
}

#[must_use]
pub fn volume_preview_audio_event(entity_index: u16, player_index: i16) -> FlightEvent {
    mark_source_audio_call(SourceAudioCallTag::VolumePreviewAdjustVolume);
    FlightEvent::PrepareSound {
        #[cfg(test)]
        audio_site: MissionAudioSite::VolumePreview,
        entity_index,
        player_index,
        sound_id: 8,
    }
}

/// Test-build identity for every source-shaped audio emitter reachable from
/// the Level 1 mission path, plus the two source branches excluded by the
/// selected normalized outdoor/single-player profile.
///
/// The tag fields that carry this type are compiled out of release builds.
/// This gives the conformance tests a compiler-enforced census without
/// widening the C ABI or changing the production event layout.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum MissionAudioSite {
    SpellMenuAssignment,
    PlayerToolbarSlotWrite,
    PlayerChatClear,
    MenuCursorTransition,
    VolumePreview,
    NotEnoughMana,
    PeriodicSpellExcluded,
    CaveAmbienceExcluded,
    SurfaceWaterLoop,
    SurfaceLandLoop,
    DistanceLoop5,
    DistanceLoop31,
    SpeedSpellActivation,
    MorphSpellActivation,
    GoatBleat,
    FireballLaunch,
    PossessionLaunch,
    InitialCastleLaunch,
    PlayerPain,
    PlayerDeath,
    AmbientFireStart,
    AmbientChildStart,
    PossessionResolution,
    ExitSequenceSwitch,
    ExitAccelerationPhase6,
    ExitAccelerationPhase8,
    SpellPickup,
    ExperienceScroll,
    AbilityImproved,
    ObjectiveAnnouncement,
    BuilderBuild,
    FireflyCharge,
    BeeAttack,
    DragonFireball,
    MultipartAttack,
    ArcherArrow,
    CastleConstruction,
    CastleDestruction,
    CastleCollectorCave,
    ManaSphereClaim,
    FireballImpact,
    WaterSplashFirstUpdate,
    ExitOverlap,
    CountdownSwitch,
    TerrainStrip,
    TerrainDisaster,
    CastleDefenseFirstShot,
    HealSpell,
    TeleportSpell,
    ReboundReflection,
    CaveGroundFeature,
    CaveInLaunch,
    MeteorLaunch,
    CaveInCapture,
    MeteorImpact,
    FireSphereCollision,
    CymmerianPursuit,
    BossCymmerianSummon,
    WyvernAttack,
    ManticorePursuit,
    Model21Jump,
    Creature22Ownership,
    Creature22Oscillation,
    Creature23Attack,
    Creature23ManaPursuit,
    Creature24Attack,
    Count,
}

/// One-to-one identity for each reachable pinned remc2
/// `PrepareEventSound_6E450` occurrence in the qualified owner campaign
/// profiles.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum SourceAudioCallTag {
    VolumePreviewAdjustVolume,
    GoatAction08Bleat,
    GoatAction09Bleat,
    GoatAction11Bleat,
    GoatAction14HitBleat,
    GoatAction15ConstructorBleat,
    MultipartAttackAction26,
    BuilderBuildAction96,
    FireflyChargeAction154,
    Class10Action00FirstEligibleSound,
    AmbientFireAction01FirstEligibleSound,
    WaterSplashAction05FirstUpdate,
    ManaSphereClaimFlaggedTransfer,
    ManaSphereClaimUnlatchedTransfer,
    HousePossessionFlaggedTransfer,
    HousePossessionUnlatchedTransfer,
    PlayerOpcode17ToolbarWrite,
    PlayerOpcode1F20HandAssignment,
    PlayerOpcode26EligibilityWrite,
    PlayerOpcode2CClearChatName,
    ObjectiveAnnouncementNoSpeech,
    ExperienceScrollPickup,
    AbilityImprovedTier,
    ProjectileDeliveryLaunch,
    ExitOverlapSwitch,
    CountdownSwitchModel16,
    CountdownSwitchModel17,
    TerrainStripOpening,
    TerrainStripClosing,
    TerrainRingRumble,
    SpellPickup,
    PossessionLaunchTiered,
    PossessionLaunchBasic,
    InitialCastleLaunch,
    SpeedSpellActivation,
    MorphSpellStart,
    MorphSpellExpiry,
    BeeAction17Acquire,
    BeeAction18Contact,
    DragonAction02Fireball,
    ArcherArrowLaunch,
    SurfaceWaterLoop,
    SurfaceLandLoop,
    DistanceLoop05,
    DistanceLoop31,
    PlayerDeath,
    ExitSequenceSwitch,
    ExitAcceleration,
    PlayerPain,
    NotEnoughManaImmediate,
    NotEnoughManaFallback,
    CastleConstruction,
    CastleDestruction,
    MenuCursorTransition,
    HealSpell,
    TeleportSpell,
    ReboundReflection,
    CaveGroundFeatureAction93,
    SpellProjectileCapture,
    MeteorAction17FirstUpdate,
    FireSpheresAction83Collision,
}

/// Compiler-checked binding from every reachable source occurrence identity
/// to the production tag used at its Rust call path.
pub const SOURCE_AUDIO_CALL_BINDINGS: [(SourceAudioCallTag, &str); 61] = [
    (
        SourceAudioCallTag::VolumePreviewAdjustVolume,
        "286982557778df1130f462a82b40225bf7e9cff0ef1a128b9d00f0a7a0d6ea2a",
    ),
    (
        SourceAudioCallTag::GoatAction08Bleat,
        "7f870dd57e8f6af332f57155d31403b38bb2abb5f7b34414a85b381edebf9421",
    ),
    (
        SourceAudioCallTag::GoatAction09Bleat,
        "a5bc1757ae53e535625ec192299a3bf15b05fec9a1e40df6785e35f2dbbf20e8",
    ),
    (
        SourceAudioCallTag::GoatAction11Bleat,
        "f3f3d032d98e88e37521f771935f8c8b266a96f22d3b9691b850a6c809b66b51",
    ),
    (
        SourceAudioCallTag::GoatAction14HitBleat,
        "34bc5537d72f5bd10286d7bdfe7a66383fff1ce3151209ff18aea4ada79f2037",
    ),
    (
        SourceAudioCallTag::GoatAction15ConstructorBleat,
        "ee8091e6791f4f854904ccf714a9e30086a5e7157957c779b6c63206c0f2cb2f",
    ),
    (
        SourceAudioCallTag::MultipartAttackAction26,
        "3111f5f836afcb5980c8634b79837d371971683ec6fd0e8ac4848b534e23e209",
    ),
    (
        SourceAudioCallTag::BuilderBuildAction96,
        "7fb2ca319b21448d2cc6082732be40d4a49a26f9e4ce42be947c35e16d221f3a",
    ),
    (
        SourceAudioCallTag::FireflyChargeAction154,
        "9939d4b873c02d67a41aa372eabba60d25afc45a3ff01f8644144ab2cc6af3d0",
    ),
    (
        SourceAudioCallTag::Class10Action00FirstEligibleSound,
        "d91d22ceb7c1208d6d0530942843c41933536dcfbf1d74151813e0a84778e7ba",
    ),
    (
        SourceAudioCallTag::AmbientFireAction01FirstEligibleSound,
        "ac35f9a0efc2e7b1105cd5d2b063e04ee4341e6e134de1c0141e63eb47bda9a9",
    ),
    (
        SourceAudioCallTag::WaterSplashAction05FirstUpdate,
        "020da55b4d42a559160d918b6018af36cc35fc6e32109b9728d0984fd279b228",
    ),
    (
        SourceAudioCallTag::ManaSphereClaimFlaggedTransfer,
        "8963e08b8055f98e2fc81186f402085810f1f4ecb745b4ec3268d062fdb2ac78",
    ),
    (
        SourceAudioCallTag::ManaSphereClaimUnlatchedTransfer,
        "b15d4e861446498abe1746ae72447384d7912f048bcd277328fff0277dad72d3",
    ),
    (
        SourceAudioCallTag::HousePossessionFlaggedTransfer,
        "cac0811953782b9212d03180debcd57c700efb72afdfcc9d5d07ef520a760997",
    ),
    (
        SourceAudioCallTag::HousePossessionUnlatchedTransfer,
        "8ea73bba2ee78440ec115784f179ed549094caca3e36f9541eaba9fca86de212",
    ),
    (
        SourceAudioCallTag::PlayerOpcode17ToolbarWrite,
        "a32668922aee8d452b64d01278a6ed5933452f6da65e9b5d32cf01be7daf1826",
    ),
    (
        SourceAudioCallTag::PlayerOpcode1F20HandAssignment,
        "a05ea31a5ef97b7e5bb19d0f76406f40ceb81ec0b38896ffd703e62874dd2920",
    ),
    (
        SourceAudioCallTag::PlayerOpcode26EligibilityWrite,
        "3a1c75288f08ff23a1085c4e7ca9a6b9140d396837f7c13d16208c3221e3124a",
    ),
    (
        SourceAudioCallTag::PlayerOpcode2CClearChatName,
        "6236b7feaced22a4455515c39bedb9f1de20ec72b4dbcae03bc4b1c4c75447a4",
    ),
    (
        SourceAudioCallTag::ObjectiveAnnouncementNoSpeech,
        "aa4c8fa1d00c2b299135de51ebfce2a43fb7d84d9468ffa1466a50136d723238",
    ),
    (
        SourceAudioCallTag::ExperienceScrollPickup,
        "a801373c898e6eb6b3729c51b72695a334a03b33a351c3e8e8f14d86ee0a6d5b",
    ),
    (
        SourceAudioCallTag::AbilityImprovedTier,
        "123646f1f5b25dde03f9f110e5c95243ccaa35c452421ffea9e1f146a9c9be64",
    ),
    (
        SourceAudioCallTag::ProjectileDeliveryLaunch,
        "14789ddae53163cfc0ea17ae80aad2bf9817551d26e3a9e335cd5a070006e178",
    ),
    (
        SourceAudioCallTag::ExitOverlapSwitch,
        "e1d44f6a3d054961fd0e86cc9fd1e6be18ac01dc66a7eb1ebe09b13b9fa45d98",
    ),
    (
        SourceAudioCallTag::CountdownSwitchModel16,
        "de95f5689d93053e2cb35ed20278024cb790084bbc9dbad17268181fb1c3798e",
    ),
    (
        SourceAudioCallTag::CountdownSwitchModel17,
        "cf09989e4b8bbf4bf23e5f267f7a53203cf7bfba48e31a1afc2effdb0c11ae85",
    ),
    (
        SourceAudioCallTag::TerrainStripOpening,
        "4832eea61c718bc09ea84e21ed2496c81c45a716e1070847c4c5fb825d7e6985",
    ),
    (
        SourceAudioCallTag::TerrainStripClosing,
        "ad0cb20ea6469d838fed921f5a0b67e9390c091bb9c605f164f92d504150753c",
    ),
    (
        SourceAudioCallTag::TerrainRingRumble,
        "917dbe7df9714946b307eb7e5e3c53d0129404abb5d9e5a8d7b4bb37e4ad7dab",
    ),
    (
        SourceAudioCallTag::SpellPickup,
        "35a9f61e359f3b2c1eaa79c5cef0ccfa51c892800e2452cbdbbc16aaca45afc7",
    ),
    (
        SourceAudioCallTag::PossessionLaunchTiered,
        "aacda3758e98e45a4351dd35d70eb9dc70deed12096eec51b21541a2701e0fc3",
    ),
    (
        SourceAudioCallTag::PossessionLaunchBasic,
        "63edb1da751bd2be1222b2938d72ea366e99a89a29c2d6545d4b067eceb68932",
    ),
    (
        SourceAudioCallTag::InitialCastleLaunch,
        "6aec7d2bfaf186589280c854c2b15af0456d9da7502481a6f93c63291f132ed6",
    ),
    (
        SourceAudioCallTag::SpeedSpellActivation,
        "c6b694c0a1b64535d469458085a0e119bcaf323fb9e160a7987b0d985cee2e0d",
    ),
    // GOLDEN-CHANGE: Level 3's package-present family-4/action-12 route makes
    // the previously Level-1-excluded sub_6A030 start and expiry sounds
    // reachable. Each occurrence retains its distinct frozen owner-ledger
    // identity rather than sharing the presentation-level Morph site.
    (
        SourceAudioCallTag::MorphSpellStart,
        "51d9ecbc8c8301f053d666cd4dd85e983569ca314534fa5e57d0a2d5014eaeaf",
    ),
    (
        SourceAudioCallTag::MorphSpellExpiry,
        "687653fd4ce5b3a30b90f6928339af0e948c20a8ad7f048e005ca8885c7887c6",
    ),
    (
        SourceAudioCallTag::BeeAction17Acquire,
        "2200cca745cfe455d9fbd0dc0624b00c43948b2f8e7ee9236b34fc4a0a86f4f4",
    ),
    (
        SourceAudioCallTag::BeeAction18Contact,
        "61ff6913f34e4284a2b98125cb5ea0915db5e5530e9ab5c567cb216e87cec4d6",
    ),
    (
        SourceAudioCallTag::DragonAction02Fireball,
        "328391c4a0c9cdb021d471d00d68353a68074f1301c07fe1aa3bd6c47991aa8d",
    ),
    (
        SourceAudioCallTag::ArcherArrowLaunch,
        "65fb54779566695f30c8e4f3d137f2348f13f6f08879194d4355463bb6c5a2db",
    ),
    (
        SourceAudioCallTag::SurfaceWaterLoop,
        "c161e969616f5441b9ea876d8fae797fd67c9e52a394708087a40e3cfff5e75c",
    ),
    (
        SourceAudioCallTag::SurfaceLandLoop,
        "a8fdf3052cfe2f51cb09144b238f0a889ad2729b0080b820d17a5ddf1da3f1e8",
    ),
    (
        SourceAudioCallTag::DistanceLoop05,
        "037c6976ac98582623cf21472dd890b6347529faa4f8bc081a54e4dc8f971cf5",
    ),
    (
        SourceAudioCallTag::DistanceLoop31,
        "914af17e9008fc2370c1cc668bd6c46fd5012e2f7b01d5a502605698b323106f",
    ),
    (
        SourceAudioCallTag::PlayerDeath,
        "1a8274b11b5140f6c939a34190b5690fe3ba1578b91ec3fe35cfc18839cf246e",
    ),
    (
        SourceAudioCallTag::ExitSequenceSwitch,
        "3da221ab8a35f8da74f829eca261a5a17b3c01a7e9def8fa334e2bd24a1ca4cc",
    ),
    (
        SourceAudioCallTag::ExitAcceleration,
        "637edc4d094d6ff4b51e0c64f50ed48c2128825b1803c2e048c51e7df325d7d3",
    ),
    (
        SourceAudioCallTag::PlayerPain,
        "4544ece689bb1eca18d56c6ea71cd43d4deb36366a3429d968a65bc6f5ed0ef2",
    ),
    (
        SourceAudioCallTag::NotEnoughManaImmediate,
        "467e5c361e5973cedc57a380dbcf17ea8f8be70f4ee3a5890529e88176a3c58c",
    ),
    (
        SourceAudioCallTag::NotEnoughManaFallback,
        "d82a33b9eb1aeff3b62a746b4e8b2e1baf614d561f4b3037ac82d5117bef3e36",
    ),
    (
        SourceAudioCallTag::CastleConstruction,
        "92b70b2b475ffa5d1201b730b462780b42fd95c98544b675b0f0a12870ecd181",
    ),
    (
        SourceAudioCallTag::CastleDestruction,
        "444a935744dda43a2a76ac528e115f10ec4a77780aa72f621736d5723357ca98",
    ),
    (
        SourceAudioCallTag::MenuCursorTransition,
        "5193f7cf7a2fb17aa2491272b35676b34c9d9b34439195d4b171a33fa6784856",
    ),
    (
        SourceAudioCallTag::CaveGroundFeatureAction93,
        "381c5435da6c0350032db982986eae352118bad47203ffd0e04d8f24ecff1c6b",
    ),
    (
        SourceAudioCallTag::HealSpell,
        "45e783b49af6367b85ef6740ee4fd0b6f818c6a7508868f0ca5eda7ababf237a",
    ),
    (
        SourceAudioCallTag::TeleportSpell,
        "3e8d2aa0ed292fba32bc8f020ae4345edf80cfe6eb885d4ea5c03547bd275477",
    ),
    (
        SourceAudioCallTag::ReboundReflection,
        "0f5e2005195aa14918a40423c8ccb0b8ab19f3f23674f660a82a4cc9dd7dec5b",
    ),
    (
        SourceAudioCallTag::SpellProjectileCapture,
        "a0282605de763727b9a5fabed2776e02f4bfb4b23bb17445169d865a49c2ac47",
    ),
    (
        SourceAudioCallTag::MeteorAction17FirstUpdate,
        "e6e8389397ec1c7fe9c33de907c7f63c2865200ab7cdd246b7e13b3afdd70d90",
    ),
    (
        SourceAudioCallTag::FireSpheresAction83Collision,
        "55766cf86f54e37aaec2b6a4e247184ba44814fcd1031784fd501ded05001063",
    ),
];

#[inline(always)]
pub fn mark_source_audio_call(_tag: SourceAudioCallTag) {}

#[cfg(test)]
impl SourceAudioCallTag {
    pub const ALL: [Self; 61] = [
        Self::VolumePreviewAdjustVolume,
        Self::GoatAction08Bleat,
        Self::GoatAction09Bleat,
        Self::GoatAction11Bleat,
        Self::GoatAction14HitBleat,
        Self::GoatAction15ConstructorBleat,
        Self::MultipartAttackAction26,
        Self::BuilderBuildAction96,
        Self::FireflyChargeAction154,
        Self::Class10Action00FirstEligibleSound,
        Self::AmbientFireAction01FirstEligibleSound,
        Self::WaterSplashAction05FirstUpdate,
        Self::ManaSphereClaimFlaggedTransfer,
        Self::ManaSphereClaimUnlatchedTransfer,
        Self::HousePossessionFlaggedTransfer,
        Self::HousePossessionUnlatchedTransfer,
        Self::PlayerOpcode17ToolbarWrite,
        Self::PlayerOpcode1F20HandAssignment,
        Self::PlayerOpcode26EligibilityWrite,
        Self::PlayerOpcode2CClearChatName,
        Self::ObjectiveAnnouncementNoSpeech,
        Self::ExperienceScrollPickup,
        Self::AbilityImprovedTier,
        Self::ProjectileDeliveryLaunch,
        Self::ExitOverlapSwitch,
        Self::CountdownSwitchModel16,
        Self::CountdownSwitchModel17,
        Self::TerrainStripOpening,
        Self::TerrainStripClosing,
        Self::TerrainRingRumble,
        Self::SpellPickup,
        Self::PossessionLaunchTiered,
        Self::PossessionLaunchBasic,
        Self::InitialCastleLaunch,
        Self::SpeedSpellActivation,
        Self::MorphSpellStart,
        Self::MorphSpellExpiry,
        Self::BeeAction17Acquire,
        Self::BeeAction18Contact,
        Self::DragonAction02Fireball,
        Self::ArcherArrowLaunch,
        Self::SurfaceWaterLoop,
        Self::SurfaceLandLoop,
        Self::DistanceLoop05,
        Self::DistanceLoop31,
        Self::PlayerDeath,
        Self::ExitSequenceSwitch,
        Self::ExitAcceleration,
        Self::PlayerPain,
        Self::NotEnoughManaImmediate,
        Self::NotEnoughManaFallback,
        Self::CastleConstruction,
        Self::CastleDestruction,
        Self::MenuCursorTransition,
        Self::HealSpell,
        Self::TeleportSpell,
        Self::ReboundReflection,
        Self::CaveGroundFeatureAction93,
        Self::SpellProjectileCapture,
        Self::MeteorAction17FirstUpdate,
        Self::FireSpheresAction83Collision,
    ];
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissionAudioContract {
    pub site: MissionAudioSite,
    pub sounds: &'static str,
    pub owner: &'static str,
    pub play_type: &'static str,
    pub pitch: &'static str,
    pub source_anchor: &'static str,
    pub reachable: bool,
}

#[cfg(test)]
impl MissionAudioSite {
    pub const ALL: [Self; Self::Count as usize] = [
        Self::SpellMenuAssignment,
        Self::PlayerToolbarSlotWrite,
        Self::PlayerChatClear,
        Self::MenuCursorTransition,
        Self::VolumePreview,
        Self::NotEnoughMana,
        Self::PeriodicSpellExcluded,
        Self::CaveAmbienceExcluded,
        Self::SurfaceWaterLoop,
        Self::SurfaceLandLoop,
        Self::DistanceLoop5,
        Self::DistanceLoop31,
        Self::SpeedSpellActivation,
        Self::MorphSpellActivation,
        Self::GoatBleat,
        Self::FireballLaunch,
        Self::PossessionLaunch,
        Self::InitialCastleLaunch,
        Self::PlayerPain,
        Self::PlayerDeath,
        Self::AmbientFireStart,
        Self::AmbientChildStart,
        Self::PossessionResolution,
        Self::ExitSequenceSwitch,
        Self::ExitAccelerationPhase6,
        Self::ExitAccelerationPhase8,
        Self::SpellPickup,
        Self::ExperienceScroll,
        Self::AbilityImproved,
        Self::ObjectiveAnnouncement,
        Self::BuilderBuild,
        Self::FireflyCharge,
        Self::BeeAttack,
        Self::DragonFireball,
        Self::MultipartAttack,
        Self::ArcherArrow,
        Self::CastleConstruction,
        Self::CastleDestruction,
        Self::CastleCollectorCave,
        Self::ManaSphereClaim,
        Self::FireballImpact,
        Self::WaterSplashFirstUpdate,
        Self::ExitOverlap,
        Self::CountdownSwitch,
        Self::TerrainStrip,
        Self::TerrainDisaster,
        Self::CastleDefenseFirstShot,
        Self::HealSpell,
        Self::TeleportSpell,
        Self::ReboundReflection,
        Self::CaveGroundFeature,
        Self::CaveInLaunch,
        Self::MeteorLaunch,
        Self::CaveInCapture,
        Self::MeteorImpact,
        Self::FireSphereCollision,
        Self::CymmerianPursuit,
        Self::BossCymmerianSummon,
        Self::WyvernAttack,
        Self::ManticorePursuit,
        Self::Model21Jump,
        Self::Creature22Ownership,
        Self::Creature22Oscillation,
        Self::Creature23Attack,
        Self::Creature23ManaPursuit,
        Self::Creature24Attack,
    ];

    #[must_use]
    pub const fn contract(self) -> MissionAudioContract {
        let (sounds, owner, play_type, pitch, source_anchor, reachable) = match self {
            Self::SpellMenuAssignment => (
                "14",
                "local player",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:37748,37902,37952",
                true,
            ),
            Self::PlayerToolbarSlotWrite => (
                "14",
                "local player",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:37739-37749",
                true,
            ),
            Self::PlayerChatClear => (
                "14",
                "local player",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:38050-38054",
                true,
            ),
            Self::MenuCursorTransition => (
                "14",
                "local player",
                "1",
                "fixed 0",
                "GameUI.cpp:753-789",
                true,
            ),
            Self::VolumePreview => (
                "8",
                "local player",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:8569-8651",
                true,
            ),
            Self::NotEnoughMana => (
                "29",
                "local player",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:60949-61008",
                true,
            ),
            Self::PeriodicSpellExcluded => (
                "12/39/43/37",
                "local player entity",
                "3",
                "fixed 0",
                "EventsFunctions.cpp:59816-59837; EXC-C15-FAMILIES",
                false,
            ),
            Self::CaveAmbienceExcluded => (
                "65-69",
                "level-local",
                "none in qualified outdoor profile",
                "fixed 0",
                "EventsFunctions.cpp:59840-59848",
                false,
            ),
            Self::SurfaceWaterLoop => (
                "prepare 1/end 2",
                "level-local",
                "2",
                "fixed 0",
                "EventsFunctions.cpp:59850-59854",
                true,
            ),
            Self::SurfaceLandLoop => (
                "prepare 2/end 1",
                "level-local",
                "2",
                "fixed 0",
                "EventsFunctions.cpp:59855-59859",
                true,
            ),
            Self::DistanceLoop5 => (
                "5",
                "level-local",
                "2",
                "fixed 0",
                "EventsFunctions.cpp:59860-59868",
                true,
            ),
            Self::DistanceLoop31 => (
                "31",
                "level-local",
                "2",
                "fixed 0",
                "EventsFunctions.cpp:59869-59877",
                true,
            ),
            Self::SpeedSpellActivation => (
                "19",
                "local player",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:56262-56271",
                true,
            ),
            Self::MorphSpellActivation => (
                "60",
                "local player",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:56382",
                true,
            ),
            Self::GoatBleat => (
                "46",
                "Goat entity",
                "3",
                "next global RNG %20, action14 +10 otherwise -10",
                "EventsFunctions.cpp:11386-11458; Sound.cpp:6331-6344",
                true,
            ),
            Self::FireballLaunch => (
                "9",
                "created projectile",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:57331-57356,57699-57721,58102-58126",
                true,
            ),
            Self::PossessionLaunch => (
                "40",
                "created projectile",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:55970-56023,56080-56109",
                true,
            ),
            Self::InitialCastleLaunch => (
                "15",
                "created projectile",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:56160-56198",
                true,
            ),
            Self::PlayerPain => (
                "54-57",
                "local player",
                "3",
                "fixed 0",
                "EventsFunctions.cpp:60730-60754",
                true,
            ),
            Self::PlayerDeath => (
                "16",
                "local player",
                "3",
                "fixed 0",
                "EventsFunctions.cpp:60050-60081",
                true,
            ),
            Self::AmbientFireStart => (
                "3",
                "ambient-fire entity",
                "1",
                "fixed 0",
                "EventsFunctions.cpp ambient model-1/action-1 first update",
                true,
            ),
            Self::AmbientChildStart => (
                "3",
                "ambient child",
                "1",
                "fixed 0",
                "EventsFunctions.cpp ambient model-0/action-0 first eligible update",
                true,
            ),
            Self::PossessionResolution => (
                "4",
                "new owner",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:17512-17524,26075-26089,28021-28034",
                true,
            ),
            Self::ExitSequenceSwitch => (
                "41",
                "local player",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:60370-60394",
                true,
            ),
            Self::ExitAccelerationPhase6 => (
                "19",
                "local player",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:60570-60593",
                true,
            ),
            Self::ExitAccelerationPhase8 => (
                "19",
                "local player",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:60570-60593",
                true,
            ),
            Self::SpellPickup => (
                "18",
                "matched local player",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:55720-55759",
                true,
            ),
            Self::ExperienceScroll => (
                "63",
                "sentinel/local player",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:41168-41184",
                true,
            ),
            Self::AbilityImproved => (
                "61",
                "entity-zero sentinel/full-volume",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:41168-41189,43898-44060; PlayerInput.cpp:2441-2476; Sound.cpp:6254,6282-6288,6406-6415",
                true,
            ),
            Self::ObjectiveAnnouncement => (
                "61",
                "entity-zero sentinel/full-volume",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:31818,40881-40912,40965-41063; Sound.cpp:6254,6282-6288,6406-6415",
                true,
            ),
            Self::BuilderBuild => (
                "10",
                "Builder entity",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:14096-14102",
                true,
            ),
            Self::FireflyCharge => (
                "42-44",
                "Firefly entity",
                "3",
                "next global RNG %30 -15",
                "EventsFunctions.cpp:16505-16513; Sound.cpp:6334-6339",
                true,
            ),
            Self::BeeAttack => (
                "12/13",
                "Bee entity",
                "3",
                "fixed 0",
                "EventsFunctions.cpp:11484-11524",
                true,
            ),
            Self::DragonFireball => (
                "8",
                "Dragon entity",
                "3",
                "fixed 0",
                "EventsFunctions.cpp:11203-11208",
                true,
            ),
            Self::MultipartAttack => (
                "8",
                "attacking actor",
                "3",
                "fixed 0",
                "EventsFunctions.cpp:11593-11597",
                true,
            ),
            Self::ArcherArrow => (
                "33/34",
                "Arrow entity",
                "3",
                "fixed 0",
                "EventsFunctions.cpp:58892-58907",
                true,
            ),
            Self::CastleConstruction => (
                "10",
                "Castle root",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:61604-61619",
                true,
            ),
            Self::CastleDestruction => (
                "30",
                "Castle root",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:61650-61668",
                true,
            ),
            Self::CastleCollectorCave => (
                "22",
                "Castle collector before position copy",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:61951-61956",
                true,
            ),
            Self::ManaSphereClaim => (
                "4",
                "new owner",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:17512-17524,26075-26089",
                true,
            ),
            Self::FireballImpact => (
                "3",
                "impact entity",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:22741-22750",
                true,
            ),
            Self::MeteorImpact => (
                "30",
                "meteor impact entity",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:23862 sub_32880 first eligible update",
                true,
            ),
            Self::FireSphereCollision => (
                "3",
                "Fire Spheres root",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:24713 sub_33C00 per colliding column-zero sector",
                true,
            ),
            Self::CymmerianPursuit => (
                "37",
                "current Cymmerian before pursuit movement",
                "3",
                "fixed 0",
                "EventsFunctions.cpp:19082 sub_28C60 before sub_1C310",
                true,
            ),
            Self::BossCymmerianSummon => (
                "37",
                "current record addressed by boss source ID after model25 construction",
                "3",
                "fixed 0",
                "EventsFunctions.cpp:13509 sub_21AB0 successful mode5",
                true,
            ),
            Self::WyvernAttack => (
                "39",
                "current Wyvern after projectile construction",
                "3",
                "fixed 0",
                "EventsFunctions.cpp:15503 sub_24510 before burst rearm",
                true,
            ),
            Self::ManticorePursuit => (
                "58",
                "current Manticore before damage and pursuit movement",
                "3",
                "fixed 0",
                "EventsFunctions.cpp:15613 sub_24930 callback entry",
                true,
            ),
            Self::Model21Jump => (
                "42",
                "current model21 before its jump movement tail",
                "3",
                "global RNG % 30 - 15",
                "EventsFunctions.cpp:17090 sub_265A0 state 9",
                true,
            ),
            Self::Creature22Ownership => (
                "4",
                "current player named by the model22 possession mailbox",
                "1",
                "fixed 0",
                "EventsFunctions.cpp sub_26F10/sub_26D20 ownership transition",
                true,
            ),
            Self::Creature22Oscillation => (
                "48",
                "current model22 after movement at its animation call",
                "1",
                "fixed 0",
                "EventsFunctions.cpp sub_272C0 previous animation frame1..15",
                true,
            ),
            Self::Creature23Attack => (
                "59",
                "current model23 before attack movement and target reads",
                "3",
                "fixed 0",
                "EventsFunctions.cpp sub_27E00 callback entry",
                true,
            ),
            Self::Creature23ManaPursuit => (
                "59",
                "current model23 before mana pursuit movement and target reads",
                "3",
                "fixed 0",
                "EventsFunctions.cpp sub_27C10 callback entry",
                true,
            ),
            Self::Creature24Attack => (
                "7",
                "current model24 before attack damage, movement and target reads",
                "3",
                "fixed 0",
                "EventsFunctions.cpp sub_28570 callback entry",
                true,
            ),
            Self::WaterSplashFirstUpdate => (
                "27",
                "splash entity",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:23168-23178",
                true,
            ),
            Self::ExitOverlap => (
                "41",
                "matched player",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:44570-44587",
                true,
            ),
            Self::CountdownSwitch => (
                "41",
                "model16/17 switch",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:54497-54539",
                true,
            ),
            Self::TerrainStrip => (
                "47",
                "terrain-strip controller",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:41802,42135,42145",
                true,
            ),
            Self::TerrainDisaster => (
                "10",
                "terrain lowering ring",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:23540",
                true,
            ),
            Self::CastleDefenseFirstShot => (
                "9/23/9/9/15",
                "Castle root",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:30168-30169,30283-30289,44271-44282",
                true,
            ),
            Self::HealSpell => (
                "25",
                "spell parent",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:56489",
                true,
            ),
            Self::TeleportSpell => (
                "22",
                "spell parent",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:57071",
                true,
            ),
            Self::ReboundReflection => (
                "28",
                "reflecting target",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:55312",
                true,
            ),
            Self::CaveGroundFeature => (
                "50/51/52",
                "ground-anchored cave effect",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:22845 (sub_31120)",
                true,
            ),
            Self::CaveInLaunch | Self::MeteorLaunch => (
                "15",
                "spell parent",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:44280 (sub_6DCA0)",
                true,
            ),
            Self::CaveInCapture => (
                "26",
                "captured projectile",
                "1",
                "fixed 0",
                "EventsFunctions.cpp:55477 (sub_68AC0)",
                true,
            ),
            Self::Count => panic!("sentinel has no audio contract"),
        };
        MissionAudioContract {
            site: self,
            sounds,
            owner,
            play_type,
            pitch,
            source_anchor,
            reachable,
        }
    }
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
    /// Source-global movement scratch, shared by ordered entity callbacks.
    fn source_predicted_position(&self) -> Position {
        Position { x: 0, y: 0, z: 0 }
    }
    fn set_source_predicted_position(&self, _position: Position) {}
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

    /// Source `sub_102D0(..., 1)` rejects cave angle-bit-8 cells while
    /// probing a movement candidate. World implementations may refine this
    /// with their entity/cave topology; the terrain-only default keeps the
    /// shared seam backward-compatible for non-cave worlds.
    fn cave_collision_blocked(&self, position: Position) -> bool {
        self.ceiling_height(position).is_some()
            && self.angle_flags(Tile {
                x: position.x.to_be_bytes()[0],
                y: position.y.to_be_bytes()[0],
            }) & 8
                != 0
    }
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
    context.chat_clear_requested = false;
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
                // PlayerEvents_51BB0 stores the raw requested tier.  The
                // source only applies the downward unlocked-tier projection
                // when sub_6D9C0 recomputes SpellLevels, never at this menu
                // command boundary.
                let tier = command.argument_1;
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
                    mark_source_audio_call(SourceAudioCallTag::PlayerOpcode1F20HandAssignment);
                    context.events.push(FlightEvent::PrepareSound {
                        #[cfg(test)]
                        audio_site: MissionAudioSite::SpellMenuAssignment,
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
                mark_source_audio_call(SourceAudioCallTag::PlayerOpcode26EligibilityWrite);
                context.events.push(FlightEvent::PrepareSound {
                    #[cfg(test)]
                    audio_site: MissionAudioSite::SpellMenuAssignment,
                    entity_index: 0,
                    player_index: i16::from(fire.caster.player_color_index),
                    sound_id: 14,
                });
            }
        }
        0x17 => {
            if let Some(fire) = context.spell_fire.as_mut()
                && let (Ok(slot), Ok(family)) = (
                    usize::try_from(command.argument_1),
                    u8::try_from(command.argument_2),
                )
                && slot < fire.spell_toolbar_slots.len()
            {
                for current in &mut fire.spell_toolbar_slots {
                    if *current == family {
                        *current = u8::MAX;
                    }
                }
                fire.spell_toolbar_slots[slot] = family;
                mark_source_audio_call(SourceAudioCallTag::PlayerOpcode17ToolbarWrite);
                context.events.push(FlightEvent::PrepareSound {
                    #[cfg(test)]
                    audio_site: MissionAudioSite::PlayerToolbarSlotWrite,
                    entity_index: 0,
                    player_index: i16::from(fire.caster.player_color_index),
                    sound_id: 14,
                });
            }
        }
        0x2c => {
            context.chat_clear_requested = true;
            if let Some(fire) = context.spell_fire.as_ref() {
                mark_source_audio_call(SourceAudioCallTag::PlayerOpcode2CClearChatName);
                context.events.push(FlightEvent::PrepareSound {
                    #[cfg(test)]
                    audio_site: MissionAudioSite::PlayerChatClear,
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
    decode_movement_with_tuning(flags, state, FlightTuning::CLASSIC);
}

/// Decode the existing movement command using one immutable definition-bound
/// limit. This is the single implementation used by both authored and classic
/// sessions; it does not write position or velocity directly.
pub fn decode_movement_with_tuning(flags: u8, state: &mut FlightState, tuning: FlightTuning) {
    state.speed_changed_flag = 0;
    let mut forward_direction = 0_i16;
    if flags & 0b0000_0001 != 0 && state.target_speed < tuning.max_forward_speed {
        forward_direction = 1;
    }
    if flags & 0b0000_0010 != 0 && state.target_speed > SOURCE_REVERSE_LIMIT {
        forward_direction = -1;
    }
    if forward_direction != 0 {
        state.target_speed = state
            .target_speed
            .saturating_add(
                tuning
                    .forward_acceleration
                    .saturating_mul(forward_direction),
            )
            .clamp(SOURCE_REVERSE_LIMIT, tuning.max_forward_speed);
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
    step_entity_pass_with_context_and_tuning(
        state,
        command,
        profile,
        world,
        context,
        FlightTuning::CLASSIC,
    )
}

/// Advance the ordinary entity pass with immutable definition-bound flight
/// tuning. All movement, collision, event, RNG, and epilogue work remains in
/// the existing flight body.
pub fn step_entity_pass_with_context_and_tuning<W: FlightWorld>(
    state: &mut FlightState,
    command: LatchedCommand,
    profile: FlightProfile,
    world: &W,
    context: &mut FlightContext,
    tuning: FlightTuning,
) -> StepEvidence {
    decode_movement_with_tuning(command.flags, state, tuning);
    journal_spell_fire(command.flags, context);
    step_entity_body_with_context_and_tuning(state, profile, world, context, tuning, TETHER_SPEED_LIMIT)
}

/// Execute the recovered `sub_5D530` flight/entity body without relatching
/// `PlayerEvents` input.
///
/// Player action 2 calls this body directly. It therefore preserves the
/// movement/control fields retained by the preceding live player turn and
/// suppresses any new command or spell-fire input while dying.
pub fn step_entity_body_with_context<W: FlightWorld>(
    state: &mut FlightState,
    profile: FlightProfile,
    world: &W,
    context: &mut FlightContext,
) -> StepEvidence {
    step_entity_body_with_context_and_tuning(
        state, profile, world, context, FlightTuning::CLASSIC, TETHER_SPEED_LIMIT,
    )
}

/// Run the same body for an owner whose source entity supplies its own
/// minimum speed. The tether divisor is `3 * minSpeed / 2` in `sub_5DE30`.
pub(crate) fn step_rival_entity_body_with_context<W: FlightWorld>(
    state: &mut FlightState,
    profile: FlightProfile,
    world: &W,
    context: &mut FlightContext,
    minimum_speed: i16,
) -> Result<StepEvidence, String> {
    let limit = (3 * i32::from(minimum_speed) / 2) as i16;
    let tether_will_move = !state.skip_flight_once && context.tether.target.is_some_and(|target| {
        target.entity_index == state.tether_target_index
            && target.life >= 0
            && context.tether.holder.is_some_and(|holder| {
                holder.entity_index == state.tether_holder_index && holder.active_value != 0
            })
            && u8::try_from(state.tether_tier)
                .ok()
                .and_then(crate::spells::directional::duel::tether_rule)
                .is_some_and(|spell| {
                    distance_3d_wrapped(state.position, target.position) < u32::from(spell.range)
                })
    });
    if tether_will_move && (limit <= 0 || limit > 1_024) {
        return Err(format!("rival tether has invalid source speed limit {limit}"));
    }
    Ok(step_entity_body_with_context_and_tuning(
        state, profile, world, context, FlightTuning::CLASSIC,
        if tether_will_move { limit } else { TETHER_SPEED_LIMIT },
    ))
}

fn step_entity_body_with_context_and_tuning<W: FlightWorld>(
    state: &mut FlightState,
    profile: FlightProfile,
    world: &W,
    context: &mut FlightContext,
    tuning: FlightTuning,
    tether_speed_limit: i16,
) -> StepEvidence {
    let before = state.position;

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

    // sub_5D530 leaves the previous source-global scratch untouched when
    // movement is held; an admitted visit first copies the player's pose.
    world.set_source_predicted_position(before);

    state.roll_accumulator = state
        .roll_accumulator
        .wrapping_add(modified_control_delta(state, state.roll_delta));
    state.pitch_accumulator = state
        .pitch_accumulator
        .wrapping_add(modified_control_delta(state, state.pitch_delta));
    state.yaw = wrap_angle(i32::from(state.yaw) + i32::from(state.roll_accumulator) / 8);
    state.actual_speed = step_speed_without_target_clamp(
        state.actual_speed,
        state.target_speed,
        tuning.forward_acceleration,
    );
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
    apply_tether(state, before, &mut proposed, &mut context.tether, tether_speed_limit);
    world.set_source_predicted_position(proposed);
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

    world.set_source_predicted_position(committed);
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
        world.set_source_predicted_position(recovered);
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
                #[cfg(test)]
                audio_site: MissionAudioSite::PeriodicSpellExcluded,
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
                    #[cfg(test)]
                    audio_site: MissionAudioSite::CaveAmbienceExcluded,
                    entity_index: 0,
                    player_index: local_player,
                    sound_id: u16::try_from(cave_sound + 65).expect("cave sound fits u16"),
                });
            }
        } else if world.material_mask(state.position) == WATER_AMBIENCE_MASK {
            mark_source_audio_call(SourceAudioCallTag::SurfaceWaterLoop);
            events.push(FlightEvent::PrepareSound {
                #[cfg(test)]
                audio_site: MissionAudioSite::SurfaceWaterLoop,
                entity_index: 0,
                player_index: local_player,
                sound_id: 1,
            });
            events.push(FlightEvent::EndLoop {
                #[cfg(test)]
                audio_site: MissionAudioSite::SurfaceWaterLoop,
                entity_index: 0,
                player_index: local_player,
                sound_id: 2,
            });
        } else {
            mark_source_audio_call(SourceAudioCallTag::SurfaceLandLoop);
            events.push(FlightEvent::PrepareSound {
                #[cfg(test)]
                audio_site: MissionAudioSite::SurfaceLandLoop,
                entity_index: 0,
                player_index: local_player,
                sound_id: 2,
            });
            events.push(FlightEvent::EndLoop {
                #[cfg(test)]
                audio_site: MissionAudioSite::SurfaceLandLoop,
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
    match sound_id {
        5 => mark_source_audio_call(SourceAudioCallTag::DistanceLoop05),
        31 => mark_source_audio_call(SourceAudioCallTag::DistanceLoop31),
        _ => panic!("distance-loop census only admits sounds 5 and 31"),
    }
    events.push(match (distance >= 1_536, sound_id) {
        (true, 5) => FlightEvent::EndLoop {
            #[cfg(test)]
            audio_site: MissionAudioSite::DistanceLoop5,
            entity_index: 0,
            player_index,
            sound_id,
        },
        (false, 5) => FlightEvent::PrepareSound {
            #[cfg(test)]
            audio_site: MissionAudioSite::DistanceLoop5,
            entity_index: 0,
            player_index,
            sound_id,
        },
        (true, 31) => FlightEvent::EndLoop {
            #[cfg(test)]
            audio_site: MissionAudioSite::DistanceLoop31,
            entity_index: 0,
            player_index,
            sound_id,
        },
        (false, 31) => FlightEvent::PrepareSound {
            #[cfg(test)]
            audio_site: MissionAudioSite::DistanceLoop31,
            entity_index: 0,
            player_index,
            sound_id,
        },
        _ => panic!("distance-loop census only admits sounds 5 and 31"),
    });
}

fn apply_tether(
    state: &mut FlightState,
    authoritative_position: Position,
    proposed: &mut Position,
    context: &mut TetherContext,
    speed_limit: i16,
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
    let spell = u8::try_from(state.tether_tier)
        .ok()
        .and_then(crate::spells::directional::duel::tether_rule)
        .expect("source-valid tether tier is in 0..=2");
    let distance = distance_3d_wrapped(authoritative_position, target.position);
    let holder_is_active = state.tether_holder_index != 0
        && context.holder.is_some_and(|holder| {
            holder.entity_index == state.tether_holder_index && holder.active_value != 0
        });
    if !holder_is_active
        || distance >= u32::from(spell.range)
        || target.life < 0
    {
        state.tether_target_index = 0;
        return;
    }

    let denominator = 1_024 / i32::from(speed_limit);
    let raw_speed =
        (i32::try_from(distance).unwrap_or(i32::MAX) - state.tether_prior_distance) / denominator;
    let tether_speed =
        wrapping_i16(i64::from(raw_speed)).clamp(-speed_limit, speed_limit);
    let heading = i16::try_from(heading_for_positions(
        authoritative_position,
        target.position,
    ))
    .expect("heading is an 11-bit angle");
    state.yaw = wrap_angle(i32::from(state.yaw) + i32::from(turn_toward(state.yaw, heading, 0x82)));
    move_entity(proposed, heading, state.display_pitch, tether_speed);

    if spell.drains_life {
        target.life = target
            .life
            .wrapping_sub(i32::from(target.life_regen).wrapping_add(2));
    }
    if spell.drains_mana {
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
            world.set_source_predicted_position(probe);
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
        // The Cave failure tail restores scratch before the separate recovery
        // callback; outdoor water rejection returns earlier with last_probe.
        world.set_source_predicted_position(from);
        MovementResolution::RejectAndStop
    } else if redirected {
        world.set_source_predicted_position(candidate);
        MovementResolution::Redirect(candidate)
    } else {
        world.set_source_predicted_position(candidate);
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
    step_outer_turn_with_context_and_tuning(
        state,
        command,
        speed_index,
        profile,
        world,
        context,
        FlightTuning::CLASSIC,
    )
}

/// Latch one raw command and execute the ordinary entity passes using the
/// tuning bound to this session generation.
pub fn step_outer_turn_with_context_and_tuning<W: FlightWorld>(
    state: &mut FlightState,
    command: PlayerCommand,
    speed_index: u8,
    profile: FlightProfile,
    world: &W,
    context: &mut FlightContext,
    tuning: FlightTuning,
) -> Vec<StepEvidence> {
    let command = latch_command_with_context(state, command, context);
    let pass_count = entity_passes_for_speed_index(speed_index);
    let mut evidence = Vec::with_capacity(usize::from(pass_count));
    for _ in 0..pass_count {
        evidence.push(step_entity_pass_with_context_and_tuning(
            state, command, profile, world, context, tuning,
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
    if context.chat_clear_requested {
        feed(b"chat-clear-requested\0");
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
            || fire.spell_toolbar_slots.iter().any(|family| *family != 0)
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
            feed(&fire.spell_toolbar_slots);
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
            ..
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
            ..
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

fn step_speed_without_target_clamp(value: i16, target: i16, step: i16) -> i16 {
    match value.cmp(&target) {
        Ordering::Less => value.wrapping_add(step),
        Ordering::Greater => value.wrapping_sub(step),
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

pub(crate) fn heading_for_positions(from: Position, to: Position) -> i32 {
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

pub(crate) fn turn_toward(from: i16, to: i16, maximum: i16) -> i16 {
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
#[path = "lib_tests.rs"]
mod tests;
