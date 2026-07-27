//! Deterministic Level 1 mission kernel reconstructed from remc2 and the
//! exported owner-data level record.
//!
//! This module intentionally implements only the actors and predicates that
//! are required to play Level 1. It is not a generic replacement for the
//! complete remc2 entity dispatcher.

use crate::{
    FlightContext, FlightProfile, FlightState, FlightWorld, MovementResolution, Position,
    SIN_16_16, SpellActivationHolder, SpellHolder, Tile, angle_distance, heading_for_positions,
    legacy_sqrt, move_entity, pitch_for_delta,
    renderable_map_chain::{RenderableMapChains, RenderableMapSeed},
    resolve_legacy_movement, turn_toward,
};

pub const AUTHORED_FEATURE_COUNT: usize = 1_200;
pub const AUTHORED_FEATURE_BYTES: usize = 20;
pub const AUTHORED_FEATURE_OFFSET: usize = 1_091;
/// Original `struct_byte_0xc_12_15.byte[2] & 2`, used by `sub_49F90` to
/// populate the replaceable-entity fallback stack.
pub const SOURCE_REPLACEABLE_FLAG: u32 = 0x0002_0000;
pub const SOURCE_DISABLED_FLAG: u32 = 0x0000_0400;
const SOURCE_FADING_FLAG: u32 = 0x0000_2000;

/// Reset-time source fields required to reconstruct remc2's bounded entity
/// arena without treating every occupied slot as an indistinguishable blob.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level1ResetSourceSlot {
    pub index: u16,
    pub active: bool,
    pub class: u8,
    pub model: u8,
    pub action: u8,
    pub phase: u8,
    pub entity_rng: u16,
    pub control_flags: u32,
}

impl Level1ResetSourceSlot {
    #[must_use]
    pub const fn replaceable(self) -> bool {
        self.active && self.control_flags & SOURCE_REPLACEABLE_FLAG != 0
    }
}

pub const LEVEL1_FIRST_WAYPOINT: Position = Position {
    x: 29_440,
    y: 54_272,
    z: 0,
};
pub const LEVEL1_SECOND_WAYPOINT: Position = Position {
    x: 49_664,
    y: 54_528,
    z: 0,
};
pub const LEVEL1_REACH_TOLERANCE: u16 = 768;
pub const LEVEL1_CASTLE_MANA_PERCENT: u8 = 15;
pub const LEVEL1_MINIMAP_MAX_MARKS: usize = 16;
pub const LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS: usize = 1_152;
pub const LEVEL1_MINIMAP_COMMAND_DOT: u8 = 1;
pub const LEVEL1_MINIMAP_COMMAND_SPRITE: u8 = 2;
pub const LEVEL1_MINIMAP_COMMAND_BLEND_PIXEL: u8 = 3;
pub const LEVEL1_MINIMAP_ANCHOR_TOP_LEFT: u8 = 1;
pub const LEVEL1_MINIMAP_ANCHOR_BOTTOM_LEFT: u8 = 2;
pub const LEVEL1_MINIMAP_ANCHOR_BOTTOM_CENTER: u8 = 3;

/// Resolved source-arena row passed through the exact Level 1
/// `DrawMinimapEntities_B_61A00` visual classifier.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Level1MinimapEntityInput {
    pub source_slot: u16,
    pub class: u8,
    pub model: u8,
    pub action: u8,
    pub phase: u8,
    pub control_flags: u32,
    pub source_id: u16,
    pub source_id_is_player: bool,
    pub source_id_is_local_player: bool,
    pub player_link: u16,
    pub player_link_is_player: bool,
    pub player_link_is_local_player: bool,
    pub parent_id: u16,
    pub stage_var_2: i8,
    pub word_32: u16,
    pub position: Position,
    pub active: bool,
}

/// One source-ordered raster operation. Aperture admission is intentionally a
/// presentation concern because its scanline mask comes from owner HSPRN and
/// TABLESN data; classification, source order, projection and phase stay here.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Level1MinimapEntityCommand {
    pub source_slot: u16,
    pub x: i16,
    pub y: i16,
    pub selector: u16,
    pub kind: u8,
    pub palette_index: u8,
    pub size: u8,
    pub anchor: u8,
    pub help_selector: u8,
}

/// Exact Night-map visual classifier for the normal 128x128 Level 1 minimap.
#[must_use]
pub fn level1_minimap_entity_command(
    player: &FlightState,
    input: Level1MinimapEntityInput,
) -> Option<Level1MinimapEntityCommand> {
    if !input.active || input.source_slot == 0 {
        return None;
    }
    const NIGHT_GENERIC: u8 = 0x84;
    const NIGHT_ENTITY: u8 = 176;
    const PLAYER_PRIMARY: u8 = 164;
    const PLAYER_SECONDARY: u8 = 170;
    const CIVILIANS: u8 = 23;
    const MARKER_STONE: u8 = 23;
    const UNPOSSESSED_BUILDING: u8 = 177;
    const UNPOSSESSED_BUILDING_2: u8 = 215;
    const SPELLS: u8 = 119;
    const CREATURE: u8 = 176;
    let phase_2 = (player.tick / 2) & 1 != 0;
    let phase_3 = (player.tick / 3) & 1 != 0;
    let mut palette = 0_u8;
    let mut selector = 0_u16;
    let mut size = 1_u8;
    let mut help = 0_u8;

    match input.class {
        2 => match input.model {
            0 => palette = NIGHT_GENERIC,
            1 => {
                help = 27;
                if !phase_3 {
                    return None;
                }
                palette = MARKER_STONE;
            }
            2 => {
                palette = if phase_2 {
                    UNPOSSESSED_BUILDING
                } else {
                    NIGHT_GENERIC
                };
                help = 22;
            }
            3 => return None,
            _ => palette = NIGHT_GENERIC,
        },
        3 => match input.model {
            2 if input.source_id_is_player => {
                selector = 58;
                help = if input.source_id_is_local_player {
                    24
                } else {
                    25
                };
            }
            3 if input.source_id_is_local_player || input.player_link_is_local_player => {
                selector = 66;
                help = 23;
            }
            _ => return None,
        },
        4 | 6 | 7 | 8 | 13 => return None,
        5 => {
            if input.action == 180
                || (232..=234).contains(&input.action)
                || input.control_flags & 1 != 0
            {
                return None;
            }
            if input.source_id != input.source_slot && input.source_id_is_player {
                palette = PLAYER_SECONDARY;
            } else if (12..=14).contains(&input.model) {
                palette = CIVILIANS;
            } else {
                palette = NIGHT_ENTITY;
                if input.model == 22 {
                    if !input.player_link_is_local_player {
                        help = 18;
                    }
                } else if !((input.stage_var_2 == 13 || input.stage_var_2 == 14)
                    && input.parent_id == player.entity_index)
                {
                    help = 19;
                }
            }
        }
        9 => {
            palette = if input.source_id_is_player {
                PLAYER_PRIMARY
            } else {
                UNPOSSESSED_BUILDING_2
            };
        }
        10 => match input.model {
            18 | 86 | 87 => return None,
            34 => {
                size = 2;
                palette = if input.source_id_is_player {
                    PLAYER_PRIMARY
                } else {
                    UNPOSSESSED_BUILDING_2
                };
            }
            39 | 57 => {
                if input.player_link_is_player {
                    palette = if phase_3 {
                        PLAYER_PRIMARY
                    } else {
                        PLAYER_SECONDARY
                    };
                } else {
                    palette = 0xe8;
                    help = 18;
                }
            }
            45 => {
                if input.player_link_is_player {
                    palette = if phase_3 {
                        PLAYER_PRIMARY
                    } else {
                        PLAYER_SECONDARY
                    };
                } else {
                    palette = if input.source_id_is_player {
                        PLAYER_PRIMARY
                    } else {
                        UNPOSSESSED_BUILDING_2
                    };
                    help = 21;
                }
            }
            78 => {
                if input.word_32 != player.entity_index {
                    return None;
                }
                palette = if phase_3 {
                    PLAYER_PRIMARY
                } else {
                    PLAYER_SECONDARY
                };
            }
            _ => {
                palette = if input.source_id_is_player {
                    PLAYER_PRIMARY
                } else {
                    UNPOSSESSED_BUILDING_2
                };
            }
        },
        11 => match input.model {
            12 => {
                selector = 83;
                help = 83;
            }
            31 => {
                selector = 84;
                help = 83;
            }
            _ => return None,
        },
        12 | 15 => {
            if input.control_flags & 1 != 0 {
                return None;
            }
            palette = SPELLS;
            help = 20;
        }
        14 => {
            if input.model != 5 {
                return None;
            }
            palette = if phase_3 { SPELLS } else { CREATURE };
        }
        _ => return None,
    }

    let (x, y) = level1_minimap_project(player, input.position, 128, 256);
    Some(Level1MinimapEntityCommand {
        source_slot: input.source_slot,
        x,
        y,
        selector,
        kind: if selector == 0 {
            LEVEL1_MINIMAP_COMMAND_DOT
        } else {
            LEVEL1_MINIMAP_COMMAND_SPRITE
        },
        palette_index: palette,
        size,
        anchor: if selector == 0 {
            LEVEL1_MINIMAP_ANCHOR_TOP_LEFT
        } else if selector < 60 {
            LEVEL1_MINIMAP_ANCHOR_BOTTOM_LEFT
        } else {
            LEVEL1_MINIMAP_ANCHOR_BOTTOM_CENTER
        },
        help_selector: help,
    })
}

/// Source-style dotted castle rope samples drawn before the entity-slot walk.
/// The aperture owns the first-invalid-sample stop; the generated samples are
/// therefore bounded to the projected endpoint and remain presentation-data
/// independent.
#[must_use]
pub fn level1_minimap_castle_rope_pixels(
    player: &FlightState,
    castle: Position,
) -> Vec<(i16, i16)> {
    let (start_x, start_y) = level1_minimap_project(player, player.position, 128, 256);
    let (end_x, end_y) = level1_minimap_project(player, castle, 128, 256);
    let start = Position {
        x: start_x.cast_unsigned(),
        y: start_y.cast_unsigned(),
        z: 0,
    };
    let end = Position {
        x: end_x.cast_unsigned(),
        y: end_y.cast_unsigned(),
        z: 0,
    };
    let distance = legacy_minimap_distance(start, end);
    let heading =
        i16::try_from(heading_for_positions(start, end)).expect("screen heading fits i16");
    let mut pixels = Vec::new();
    let mut offset = i32::try_from(player.tick & 3).expect("rope phase fits i32");
    while offset <= i32::try_from(distance).expect("screen distance fits i32") {
        let mut sample = start;
        move_entity(
            &mut sample,
            heading,
            0,
            i16::try_from(offset).expect("128x128 rope distance fits i16"),
        );
        pixels.push((sample.x.cast_signed(), sample.y.cast_signed()));
        offset += 4;
    }
    pixels
}

/// Source-projected Level 1 objective marks consumed after the entity pass of
/// `DrawMinimap_B_61B20`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level1MinimapMarks {
    pub mark_count: u8,
    pub mark_pulse: bool,
    pub arrow_valid: bool,
    pub arrow_visible: bool,
    pub mark_x: [i16; LEVEL1_MINIMAP_MAX_MARKS],
    pub mark_y: [i16; LEVEL1_MINIMAP_MAX_MARKS],
    pub mark_colour: [u8; LEVEL1_MINIMAP_MAX_MARKS],
    pub mark_help_selector: [u8; LEVEL1_MINIMAP_MAX_MARKS],
    /// Seven `(x, y)` pairs in source viewport coordinates.
    pub arrow_xy: [i16; 14],
}

impl Default for Level1MinimapMarks {
    fn default() -> Self {
        Self {
            mark_count: 0,
            mark_pulse: false,
            arrow_valid: false,
            arrow_visible: false,
            mark_x: [0; LEVEL1_MINIMAP_MAX_MARKS],
            mark_y: [0; LEVEL1_MINIMAP_MAX_MARKS],
            mark_colour: [0; LEVEL1_MINIMAP_MAX_MARKS],
            mark_help_selector: [0; LEVEL1_MINIMAP_MAX_MARKS],
            arrow_xy: [0; 14],
        }
    }
}

fn legacy_minimap_distance(from: Position, to: Position) -> u32 {
    let dx = i32::from(to.x.wrapping_sub(from.x).cast_signed());
    let dy = i32::from(to.y.wrapping_sub(from.y).cast_signed());
    legacy_sqrt(
        u32::try_from(dx * dx)
            .expect("wrapped X square fits u32")
            .wrapping_add(u32::try_from(dy * dy).expect("wrapped Y square fits u32")),
    )
}

pub fn level1_minimap_project(
    player: &FlightState,
    target: Position,
    side: i32,
    scaling: i32,
) -> (i16, i16) {
    let inverse_scale = 0x1_0000 / scaling;
    let angle = usize::from(player.yaw.cast_unsigned() & 0x07ff);
    let axis_down = (-i64::from(inverse_scale) * i64::from(SIN_16_16[angle])) >> 16;
    let axis_right = (i64::from(inverse_scale) * i64::from(SIN_16_16[angle + 512])) >> 16;
    let dx = i64::from(target.x.wrapping_sub(player.position.x).cast_signed());
    let dy = i64::from(target.y.wrapping_sub(player.position.y).cast_signed());
    let x = i64::from(side / 2) + ((dx * axis_right - dy * axis_down) >> 16);
    let y = i64::from(side / 2) + ((axis_right * dy + axis_down * dx) >> 16);
    (x as i16, y as i16)
}

pub const FIREBALL_SPEED: i16 = 384;
pub const FIREBALL_LIFETIME_TICKS: i16 = 0x2000 / FIREBALL_SPEED;
pub const FIREBALL_PAYLOAD: i32 = 250;
pub const FIREBALL_MANA_COST: i32 = 100;
pub const FIREBALL_LAUNCH_SOUND: u16 = 9;
pub const FIREBALL_IMPACT_RADIUS: u16 = 128;
pub const FIREBALL_RENDER_INDEX: u16 = 340;
pub const FIREBALL_BASE_TMAP: u16 = 463;
pub const FIREBALL_RENDER_MODE: u8 = 21;
pub const FIREBALL_LOGICAL_HEIGHT: u16 = 100;
pub const FIREBALL_IMPACT_RENDER_INDEX: u16 = 7;
pub const FIREBALL_IMPACT_BASE_TMAP: u16 = 38;
pub const FIREBALL_IMPACT_RENDER_MODE: u8 = 36;
pub const FIREBALL_IMPACT_LOGICAL_HEIGHT: u16 = 250;
pub const FIREBALL_PROJECTILE_CLASS: u8 = 9;
pub const FIREBALL_PROJECTILE_MODEL: u8 = 0;
pub const FIREBALL_IMPACT_CLASS: u8 = 10;
pub const FIREBALL_IMPACT_MODEL: u8 = 0;
pub const FIREBALL_IMPACT_COLLISION_HALF_WIDTH: i16 = 128;
pub const FIREBALL_IMPACT_COLLISION_VERTICAL_OFFSET: i16 = 125;
pub const FIREBALL_IMPACT_COLLISION_HALF_HEIGHT: i16 = 128;
pub const WATER_SPLASH_MODEL: u8 = 5;
pub const WATER_SPLASH_RENDER_INDEX: u16 = 244;
pub const WATER_SPLASH_BASE_TMAP: u16 = 186;
/// Runtime mode is copied from TMAP 186's `02 0b` header by `sub_71410`.
pub const WATER_SPLASH_RENDER_MODE: u8 = 11;
pub const WATER_SPLASH_LOGICAL_HEIGHT: u16 = 300;
pub const WATER_SPLASH_SOUND: u16 = 27;
pub const POSSESSION_SPELL_FAMILY: u8 = 1;
pub const POSSESSION_MANA_COST: i32 = 100;
pub const POSSESSION_LAUNCH_SOUND: u16 = 40;
pub const POSSESSION_HOLDER_ACTIVATION_TICKS: i16 = 3;
pub const POSSESSION_PROJECTILE_CLASS: u8 = 9;
pub const POSSESSION_PROJECTILE_MODEL: u8 = 1;
pub const POSSESSION_PROJECTILE_ACTION: u8 = 1;
pub const POSSESSION_PROJECTILE_SPEED: i16 = 384;
pub const POSSESSION_PROJECTILE_LIFETIME_TICKS: i16 = 4_096 / POSSESSION_PROJECTILE_SPEED;
pub const POSSESSION_PROJECTILE_RENDER_INDEX: u16 = 209;
pub const POSSESSION_PROJECTILE_BASE_TMAP: u16 = 144;
pub const POSSESSION_PROJECTILE_RENDER_MODE: u8 = 21;
pub const POSSESSION_PROJECTILE_LOGICAL_HEIGHT: u16 = 150;
pub const POSSESSION_SOUND: u16 = 4;

pub const CASTLE_SPELL_FAMILY: u8 = 2;
pub const CASTLE_INITIAL_MANA_COST: i32 = 1_000;
pub const CASTLE_HOLDER_ACTIVATION_TICKS: i16 = 101;
pub const CASTLE_PROJECTILE_CLASS: u8 = 9;
pub const CASTLE_PROJECTILE_MODEL: u8 = 10;
pub const CASTLE_PROJECTILE_ACTION: u8 = 10;
pub const CASTLE_PROJECTILE_SPEED: i16 = 384;
pub const CASTLE_PROJECTILE_LIFETIME_TICKS: i16 = 0x2000 / CASTLE_PROJECTILE_SPEED;
pub const CASTLE_PROJECTILE_RENDER_INDEX: u16 = 18;
pub const CASTLE_PROJECTILE_BASE_TMAP: u16 = 61;
/// Runtime mode is copied from TMAP 61's `02 15` header by `sub_71410`.
pub const CASTLE_PROJECTILE_RENDER_MODE: u8 = 21;
pub const CASTLE_PROJECTILE_LOGICAL_HEIGHT: u16 = 150;
pub const CASTLE_PROJECTILE_TARGET_DISTANCE: i16 = 4_096;
pub const CASTLE_PROJECTILE_TURN_LIMIT: i16 = 22;
pub const CASTLE_PROJECTILE_SOUND: u16 = 15;
pub const CASTLE_ROOT_CLASS: u8 = 3;
pub const CASTLE_ROOT_MODEL: u8 = 2;
pub const CASTLE_ROOT_ACTION: u8 = 5;
pub const CASTLE_ROOT_RENDER_INDEX: u16 = 177;
pub const CASTLE_ROOT_BASE_TMAP: u16 = 96;
pub const CASTLE_ROOT_RENDER_MODE: u8 = 0;
pub const CASTLE_ROOT_LOGICAL_HEIGHT: u16 = 400;
pub const CASTLE_ROOT_CONSTRUCTOR_LIFE: i32 = 40_000;
pub const CASTLE_STAGE_ONE_LEVEL: u8 = 1;
pub const CASTLE_STAGE_ONE_LIFE: i32 = 20_000;
pub const CASTLE_STAGE_ONE_MANA_CAPACITY: i32 = 8_500;
/// BUILD0-0 index 1 is 8x8. `SetShiftByCastle_49EC0` adds five tiles before
/// halving, producing a 13-tile-wide active collision box.
pub const CASTLE_STAGE_ONE_COLLISION_HALF_WIDTH: i16 = 1_664;
pub const CASTLE_STAGE_ONE_PLACEMENT_HALF_Y: i16 = 1_664;
pub const CASTLE_STAGE_ONE_COLLISION_VERTICAL_OFFSET: i16 = -8_192;
pub const CASTLE_STAGE_ONE_COLLISION_HALF_HEIGHT: i16 = 16_384;
pub const CASTLE_CONSTRUCTION_SOUND: u16 = 10;
pub const CASTLE_DESTRUCTION_SOUND: u16 = 30;
pub const CASTLE_PLACEMENT_FOOTPRINT_TILES: u8 = 8;
pub const CASTLE_PLACEMENT_MARGIN: i32 = 2_560;
pub const CASTLE_CONSTRUCTION_CLASS: u8 = 10;
pub const CASTLE_CONSTRUCTION_MODEL: u8 = 42;
pub const CASTLE_CONSTRUCTION_ACTION: u8 = 44;
pub const CASTLE_CONSTRUCTION_INITIAL_TIMER: i16 = 19;
pub const PLAYER_OUTDOOR_MANA_REGEN: i32 = 100;
pub const PLAYER_OUTDOOR_LIFE_REGEN: i32 = 5;
pub const PLAYER_HIT_FRAME_TICKS: u8 = 4;
pub const PLAYER_REGEN_BLOCK_TICKS: u16 = 16;
pub const PLAYER_HIT_AUX_TICKS: u16 = 64;
pub const PLAYER_SURVIVED_FEEDBACK: u16 = 100;
pub const PLAYER_PAIN_SOUND_BASE: u16 = 54;
pub const PLAYER_DEATH_SOUND: u16 = 16;
pub const MANA_SPHERE_CLASS: u8 = 10;
pub const MANA_SPHERE_MODEL: u8 = 39;
pub const MANA_SPHERE_ACTION: u8 = 41;
pub const MANA_SPHERE_INITIAL_VERTICAL_VELOCITY: i16 = 128;
pub const MANA_SPHERE_INITIAL_SPEED: i16 = 32;
pub const MANA_SPHERE_MAX_XY_VELOCITY: i16 = 64;
pub const MANA_SPHERE_GRAVITY: i16 = 16;
pub const MANA_SPHERE_TERMINAL_VELOCITY: i16 = -128;
pub const MANA_SPHERE_NEUTRAL_RENDER_BASE: u16 = 52;
pub const MANA_SPHERE_PLAYER_ONE_RENDER_BASE: u16 = 105;
pub const MANA_SPHERE_NEUTRAL_BASE_TMAP: u16 = 58;
pub const MANA_SPHERE_PLAYER_ONE_BASE_TMAP: u16 = 67;
pub const MANA_SPHERE_RENDER_MODE: u8 = 21;
pub const MANA_SPHERE_SIZE_THRESHOLDS: [i32; 7] = [256, 512, 1_024, 2_048, 4_096, 9_192, 18_384];
pub const MANA_SPHERE_COLLISION_DIMENSIONS: [i16; 8] = [13, 28, 42, 56, 70, 84, 98, 112];
/// `sub_10A50` calls `AddE7EE0x_10080(0, 1)` for every reachable Level 1
/// sphere size. `sub_10130` therefore walks the four SEARCH.DAT class-0 cells
/// followed by the twelve class-1 cells, preserving their generated order.
const MANA_SPHERE_SEARCH_OFFSETS: [(i8, i8); 16] = [
    (0, 0),
    (1, 0),
    (0, 1),
    (1, 1),
    (-1, -1),
    (0, -1),
    (1, -1),
    (2, -1),
    (-1, 0),
    (2, 0),
    (-1, 1),
    (2, 1),
    (-1, 2),
    (0, 2),
    (1, 2),
    (2, 2),
];
pub const DEATH_BURST_CLASS: u8 = 10;
pub const DEATH_BURST_MODEL: u8 = 1;
pub const DEATH_BURST_ACTION: u8 = 1;
pub const MANA_CARRIER_CLASS: u8 = 3;
pub const MANA_CARRIER_MODEL: u8 = 3;
pub const MANA_CARRIER_ACTION: u8 = 9;
pub const MANA_CARRIER_SPEED: i16 = 48;
pub const MANA_CARRIER_MAX_LIFE: i32 = 10_000;
pub const MANA_CARRIER_MAX_MANA: i32 = 10_000;
pub const MANA_CARRIER_GROUND_CLEARANCE: i16 = 512;
pub const MANA_CARRIER_DESCENT: i16 = -16;
pub const MANA_CARRIER_ATTRACTION_RADIUS: u32 = 1_024;
pub const MANA_CARRIER_ATTRACTION_SPEED: i16 = 16;
pub const MANA_CARRIER_ATTRACTION_VERTICAL_STEP: i16 = 32;
// TMAP processing fills render row 169's initially-zero horizontal size from
// owned TMAP 88.  The qualified Level 1 presentation row is 545/800, and
// SetEntityIndexAndRot_49CD0 halves both values for the source AABB.
pub const MANA_CARRIER_COLLISION_HALF_WIDTH: i16 = 272;
pub const MANA_CARRIER_COLLISION_VERTICAL_OFFSET: i16 = 400;
pub const MANA_CARRIER_COLLISION_HALF_HEIGHT: i16 = 400;
pub const MANA_CARRIER_RENDER_INDEX: u16 = 169;
pub const MANA_CARRIER_BASE_TMAP: u16 = 88;
pub const MANA_CARRIER_RENDER_MODE: u8 = 0;
pub const MANA_CARRIER_LOGICAL_HEIGHT: u16 = 800;

#[must_use]
pub fn mana_sphere_size_ordinal(mana: i32) -> u8 {
    MANA_SPHERE_SIZE_THRESHOLDS
        .iter()
        .take_while(|threshold| mana > **threshold)
        .count() as u8
}

pub const ARCHER_MODEL: u8 = 4;
pub const ARCHER_ACTION: u8 = 33;
pub const ARCHER_LIFE: i32 = 1_000;
pub const ARCHER_MIN_SPEED: i16 = 30;
pub const ARCHER_DECISION_CADENCE: u8 = 30;
pub const ARCHER_ACQUISITION_CADENCE: u8 = 120;
pub const ARCHER_RANGE: u16 = 5_120;
pub const ARCHER_VIEW_HALF_ANGLE: i16 = 512;
pub const ARCHER_TURN_LIMIT: i16 = 22;
pub const ARCHER_TERRAIN_MASK: u32 = 0xfff0_80fe;
pub const ARCHER_MAX_TILE_SLOPE: i32 = 20;
pub const ARCHER_RENDER_INDEX: u16 = 0;
pub const ARCHER_BASE_TMAP: u16 = 8;
pub const ARCHER_RENDER_MODE: u8 = 20;
pub const ARCHER_LOGICAL_HEIGHT: u16 = 250;
pub const ARCHER_ATTACK_RENDER_INDEX_A: u16 = 1;
pub const ARCHER_ATTACK_BASE_TMAP_A: u16 = 11;
pub const ARCHER_ATTACK_RENDER_INDEX_B: u16 = 206;
pub const ARCHER_ATTACK_BASE_TMAP_B: u16 = 121;
pub const ARCHER_ATTACK_LOGICAL_HEIGHT_B: u16 = 240;
pub const ARCHER_ARROW_SPEED: i16 = 384;
pub const ARCHER_ARROW_LIFETIME_TICKS: i16 = 5_120 / ARCHER_ARROW_SPEED;
pub const ARCHER_ARROW_PAYLOAD: i32 = 250;
pub const ARCHER_ARROW_RENDER_INDEX: u16 = 195;
pub const ARCHER_ARROW_BASE_TMAP: u16 = 105;
pub const ARCHER_ARROW_RENDER_MODE: u8 = 19;
pub const ARCHER_ARROW_LOGICAL_HEIGHT: u16 = 60;
pub const ARCHER_ARROW_PROJECTILE_CLASS: u8 = 9;
pub const ARCHER_ARROW_PROJECTILE_MODEL: u8 = 13;
pub const FIREFLY_MODEL: u8 = 19;
pub const FIREFLY_ROAM_ACTION: u8 = 153;
pub const FIREFLY_ATTACK_ACTION: u8 = 154;
pub const FIREFLY_LINKED_ACTION: u8 = 155;
pub const FIREFLY_PRE_KILL_ACTION: u8 = 156;
pub const FIREFLY_KILL_ACTION: u8 = 157;
pub const FIREFLY_STAGE_ACTION: u8 = 159;
pub const FIREFLY_LIFE: i32 = 600;
pub const FIREFLY_MANA: i32 = 300;
pub const FIREFLY_MIN_SPEED: i16 = 76;
pub const FIREFLY_MAX_SPEED: i16 = 8;
pub const FIREFLY_TURN_LIMIT: i16 = 113;
pub const FIREFLY_GROUND_CLEARANCE: i16 = 51;
pub const FIREFLY_DESCENT: i16 = -8;
pub const FIREFLY_MAX_TILE_SLOPE: i32 = 20;
pub const FIREFLY_DECISION_CADENCE: u8 = 35;
pub const FIREFLY_RANGE: u16 = 5_120;
pub const FIREFLY_VIEW_HALF_ANGLE: i16 = 682;
pub const FIREFLY_CONTACT_DAMAGE: i32 = 300;
pub const FIREFLY_CONTACT_RANGE: u32 = 768;
pub const FIREFLY_STAGE_TARGET: Position = Position {
    x: 242 << 8,
    y: 184 << 8,
    z: 0,
};
pub const FIREFLY_RENDER_INDEX: u16 = 287;
pub const FIREFLY_BASE_TMAP: u16 = 279;
pub const FIREFLY_RENDER_MODE: u8 = 17;
pub const FIREFLY_LOGICAL_HEIGHT: u16 = 200;
pub const FIREFLY_FIREBALL_SPEED: i16 = 384;
pub const FIREFLY_FIREBALL_LIFETIME_TICKS: i16 = 0x2000 / FIREFLY_FIREBALL_SPEED;
pub const FIREFLY_FIREBALL_PAYLOAD: i32 = 500;
pub const FIREFLY_FIREBALL_PITCH_TURN_LIMIT: i16 = 22;
pub const MULTIPART_MODEL: u8 = 3;
pub const MULTIPART_ROAM_ACTION: u8 = 25;
pub const MULTIPART_ATTACK_ACTION: u8 = 26;
pub const MULTIPART_LINKED_ACTION: u8 = 27;
pub const MULTIPART_PRE_KILL_ACTION: u8 = 28;
pub const MULTIPART_KILL_ACTION: u8 = 29;
pub const MULTIPART_UNSUPPORTED_ACTION: u8 = 30;
pub const MULTIPART_STAGE_ACTION: u8 = 31;
pub const MULTIPART_SEGMENT_ACTION: u8 = 232;
pub const MULTIPART_LIFE: i32 = 9_000;
pub const MULTIPART_TOTAL_MANA: i32 = 4_500;
pub const MULTIPART_HEAD_MANA: i32 = MULTIPART_TOTAL_MANA / 2;
pub const MULTIPART_SEGMENT_MANA: i32 = MULTIPART_TOTAL_MANA >> 5;
pub const MULTIPART_MIN_SPEED: i16 = 64;
pub const MULTIPART_MAX_SPEED: i16 = 16;
pub const MULTIPART_INITIAL_SPEED: i16 = 30;
pub const MULTIPART_TURN_LIMIT: i16 = 34;
pub const MULTIPART_GROUND_CLEARANCE: i16 = 0;
pub const MULTIPART_DESCENT: i16 = -64;
pub const MULTIPART_MAX_TILE_SLOPE: i32 = 20;
pub const MULTIPART_ALTERNATE_HEADING: i16 = 341;
pub const MULTIPART_TERRAIN_MASK: u32 = 0xfff0_80fe;
pub const MULTIPART_DECISION_CADENCE: u8 = 30;
pub const MULTIPART_RANGE: u16 = 5_120;
pub const MULTIPART_VIEW_HALF_ANGLE: i16 = 170;
pub const MULTIPART_HEAD_RENDER_INDEX: u16 = 88;
pub const MULTIPART_HEAD_BASE_TMAP: u16 = 30;
pub const MULTIPART_HEAD_RENDER_MODE: u8 = 17;
pub const MULTIPART_HEAD_LOGICAL_HEIGHT: u16 = 180;
pub const MULTIPART_HEAD_COLLISION_HALF_WIDTH: i16 = 414;
pub const MULTIPART_HEAD_COLLISION_VERTICAL_OFFSET: i16 = 96;
pub const MULTIPART_HEAD_COLLISION_HALF_HEIGHT: i16 = 108;
pub const MULTIPART_SEGMENT_COUNT: usize = 16;
pub const MULTIPART_SEGMENT_BASE_TMAP: u16 = 66;
pub const MULTIPART_SEGMENT_RENDER_MODE: u8 = 0;
pub const MULTIPART_SEGMENT_COLLISION_HALF_WIDTHS: [i16; MULTIPART_SEGMENT_COUNT] = [
    119, 131, 147, 159, 167, 167, 159, 151, 135, 127, 119, 104, 87, 71, 55, 47,
];
pub const MULTIPART_SEGMENT_TRAILING_DISTANCES: [i16; MULTIPART_SEGMENT_COUNT] = [
    148, 131, 147, 159, 167, 167, 159, 151, 135, 127, 119, 104, 87, 71, 55, 47,
];
pub const MULTIPART_SEGMENT_HALF_HEIGHTS: [i16; MULTIPART_SEGMENT_COUNT] = [
    97, 107, 120, 130, 136, 136, 130, 123, 110, 104, 97, 84, 71, 58, 45, 39,
];
pub const MULTIPART_SEGMENT_LOGICAL_HEIGHTS: [u16; MULTIPART_SEGMENT_COUNT] = [
    150, 165, 185, 200, 210, 210, 200, 190, 170, 160, 150, 130, 110, 90, 70, 60,
];
pub const MULTIPART_ATTACK_SOUND: u16 = 8;
pub const EXPERIENCE_SCROLL_RENDER_INDEX: u16 = 280;
pub const EXPERIENCE_SCROLL_BASE_TMAP: u16 = 259;
pub const EXPERIENCE_SCROLL_RENDER_MODE: u8 = 1;
pub const EXPERIENCE_SCROLL_LOGICAL_HEIGHT: u16 = 75;
pub const EXPERIENCE_SCROLL_COLLISION_HALF_WIDTH: i16 = 768;
pub const EXPERIENCE_SCROLL_COLLISION_VERTICAL_OFFSET: i16 = 640;
pub const EXPERIENCE_SCROLL_COLLISION_HALF_HEIGHT: i16 = 1_280;
pub const EXPERIENCE_SCROLL_SOUND: u16 = 63;
pub const SPELL_JAR_RENDER_INDEX: u16 = 77;
pub const SPELL_JAR_BASE_TMAP: u16 = 59;
pub const SPELL_JAR_RENDER_MODE: u8 = 1;
pub const SPELL_JAR_LOGICAL_HEIGHT: u16 = 150;
pub const EXIT_TARGET_RENDER_INDEX: u16 = 338;
pub const EXIT_TARGET_BASE_TMAP: u16 = 461;
/// Runtime mode is copied from TMAP 461's `02 00` header by `sub_71410`.
pub const EXIT_TARGET_RENDER_MODE: u8 = 0;
pub const EXIT_TARGET_LOGICAL_HEIGHT: u16 = 700;
pub const SPEED_SPELL_FAMILY: u8 = 3;
pub const SPEED_SPELL_MANA_COST: i32 = 1_000;
pub const SPEED_SPELL_ACTIVATION_TICKS: u16 = 301;
pub const SPEED_SPELL_TIER_MULTIPLIER: i16 = 2;
pub const SPEED_PICKUP_SOUND: u16 = 18;
pub const EXIT_SWITCH_SOUND: u16 = 41;
pub const EXIT_ACCELERATION_SOUND: u16 = 19;

const MISSION_ACTOR_CLASS: u8 = 5;
const CASTLE_JAR_FEATURE: u16 = 31;
const SPEED_JAR_FEATURE: u16 = 100;
const PLAYER_COLLISION_HALF_WIDTH: i16 = 121;
const PLAYER_COLLISION_VERTICAL_OFFSET: i16 = 100;
const PLAYER_COLLISION_HALF_HEIGHT: i16 = 100;
const FIREBALL_COLLISION_HALF_WIDTH: i16 = 0;
const FIREBALL_COLLISION_VERTICAL_OFFSET: i16 = 50;
const FIREBALL_COLLISION_HALF_HEIGHT: i16 = 50;
const ENTITY_POOL_SIZE: usize = 1_000;
const LEVEL1_SYNTHETIC_RESET_OCCUPIED_SLOTS: [usize; 4] = [48, 112, 151, 152];
const ARCHER_COLLISION_HALF_WIDTH: i16 = 128;
const ARCHER_COLLISION_VERTICAL_OFFSET: i16 = 128;
const ARCHER_COLLISION_HALF_HEIGHT: i16 = 256;
const ARCHER_ARROW_COLLISION_HALF_WIDTH: i16 = 0;
const ARCHER_ARROW_COLLISION_VERTICAL_OFFSET: i16 = 30;
const ARCHER_ARROW_COLLISION_HALF_HEIGHT: i16 = 60;
const LEVEL1_ALLOCATION_GROUPS: [i16; 11] = [1, 2, 3, 4, 5, 6, 8, 11, 12, 33, 34];
const LEVEL1_ALLOCATION_AUTHORED_INDICES: [u16; 55] = [
    21, 31, 99, 100, 101, 102, 103, 104, 105, 106, 108, 109, 132, 134, 135, 137, 138, 139, 140,
    141, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 168, 169, 170, 171, 172, 173,
    174, 175, 189, 190, 193, 231, 232, 233, 253, 304, 305, 306, 307, 308, 309, 310, 311,
];
const LEVEL1_INITIAL_ZONE_SWITCH_CENTER: Position = Position {
    x: (74 << 8) + 128,
    y: (212 << 8) + 128,
    z: 4_896,
};
const LEVEL1_INITIAL_ZONE_SWITCH_RADIUS_XY: u16 = 64 << 8;
const LEVEL1_INITIAL_ZONE_SWITCH_RADIUS_Z: i16 = 4_096;
const LEVEL1_INITIAL_ZONE_SWITCH_PHASE: u8 = 49;
const LEVEL1_FIRST_WAYPOINT_SWITCH_CENTER: Position = Position {
    x: (116 << 8) + 128,
    y: (213 << 8) + 128,
    z: 0,
};
const LEVEL1_FIRST_WAYPOINT_SWITCH_RADIUS_XY: u16 = 22 << 8;
const LEVEL1_FIRST_WAYPOINT_SWITCH_RADIUS_Z: i16 = 4_096;
const LEVEL1_FIRST_WAYPOINT_SWITCH_PHASE: u8 = 152;
const LEVEL1_EXIT_SWITCH_FEATURE: u16 = 137;
const LEVEL1_EXIT_TARGET_DISTANCE: u32 = 0x180;
const EXIT_PLAYER_ACTION: u8 = 12;
const EXIT_TARGET_CLASS: u8 = 14;
const EXIT_TARGET_MODEL_PRIMARY: u8 = 3;
const EXIT_TARGET_MODEL_ALTERNATE: u8 = 4;
const EXIT_PALETTE_NONE: u8 = 0;
const EXIT_PALETTE_FADE_BACK: u8 = 1;
const LEVEL1_PALETTE_HIT_REQUEST: u8 = 2;
const LEVEL1_PALETTE_SPEED_REQUEST: u8 = 3;
const EXIT_PALETTE_FADE_TO_BLACK: u8 = 10;
const EXIT_PALETTE_ENTRY_FLASH_REQUEST: u8 = 6;
const EXIT_PALETTE_BLACK_REQUEST: u8 = 5;
const LEVEL1_PALETTE_OBJECTIVE_REQUEST: u8 = 8;
const LEVEL1_PALETTE_OBJECTIVE_FADE: u8 = 9;
const EXIT_PALETTE_EFFECT_NONE: u8 = 0;
const EXIT_PALETTE_EFFECT_ENTRY_FLASH: u8 = 1;
const EXIT_PALETTE_EFFECT_BLACK: u8 = 2;
const LEVEL1_PALETTE_EFFECT_HIT: u8 = 3;
const LEVEL1_PALETTE_EFFECT_SPEED: u8 = 4;
const LEVEL1_PALETTE_EFFECT_OBJECTIVE: u8 = 5;
const EXIT_MOVEMENT_NOT_ATTEMPTED: u8 = 0;
const EXIT_MOVEMENT_ACCEPTED: u8 = 1;
const EXIT_MOVEMENT_REDIRECTED: u8 = 2;
const EXIT_MOVEMENT_REJECTED: u8 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthoredFeature {
    pub index: u16,
    pub kind: u16,
    pub subtype: i16,
    pub tile_x: u16,
    pub tile_y: u16,
    pub activation_group: i16,
    pub word_10: u16,
    pub stage_tag: i16,
    pub parameter_1: u16,
    pub parameter_2: u16,
    pub parameter_3: u16,
}

impl AuthoredFeature {
    #[must_use]
    pub const fn world_position(self, z: i16) -> Position {
        Position {
            // `sub_4A310` activates authored records at the center of their
            // 256-unit map cell, not at its north-west corner.
            x: (self.tile_x << 8).wrapping_add(128),
            y: (self.tile_y << 8).wrapping_add(128),
            z,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Level1ActorKind {
    Archer = 1,
    Firefly = 2,
    CastleSpellJar = 3,
    SpeedSpellJar = 4,
    ExitTarget = 5,
    ExitSwitch = 6,
    MultipartHead = 7,
    ExperienceScroll = 8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level1Actor {
    pub authored_index: u16,
    /// Narrow projected source-pool slot; zero before activation. This becomes
    /// exact only after allocation-only tracking covers every authored record
    /// in the activated group, including switches, trees, and other actors.
    pub source_entity_index: u16,
    pub kind: Level1ActorKind,
    pub class: u8,
    pub model: u8,
    pub action: u8,
    pub activation_group: i16,
    pub owner_id: i16,
    pub position: Position,
    pub max_life: i32,
    pub life: i32,
    pub pending_damage: i32,
    pub pending_attacker: u16,
    pub entity_rng: u16,
    pub yaw: i16,
    pub desired_heading: i16,
    pub min_speed: i16,
    pub actual_speed: i16,
    pub max_speed: i16,
    pub target_entity_index: u16,
    pub linked_entity_index: u16,
    pub multipart_first_segment_index: u16,
    pub movement_blocked: bool,
    pub stage_var_1: i8,
    pub stage_var_2: i8,
    pub attack_substate: u8,
    pub state_value: i32,
    pub phase: u8,
    pub state_timer: i16,
    pub animation_frame: i8,
    pub collision_half_width: i16,
    pub collision_vertical_offset: i16,
    pub collision_half_height: i16,
    pub render_parameter_index: Option<u16>,
    pub base_tmap: Option<u16>,
    pub render_mode: Option<u8>,
    pub logical_height: Option<u16>,
    pub active: bool,
    pub collected: bool,
    pub visible: bool,
}

/// One source-arena-owned body node created immediately by the model-3
/// constructor. Unlike authored actors, body nodes have no authored row and
/// are therefore keyed by their source slot in the native/Unity bridge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level1MultipartSegment {
    pub source_entity_index: u16,
    pub head_authored_index: u16,
    pub head_source_entity_index: u16,
    pub predecessor_source_entity_index: u16,
    pub successor_source_entity_index: u16,
    pub ordinal: u8,
    pub class: u8,
    pub model: u8,
    pub action: u8,
    pub owner_id: i16,
    pub position: Position,
    pub life: i32,
    pub pending_damage: i32,
    pub pending_attacker: u16,
    pub yaw: i16,
    pub pitch: i16,
    pub phase: u8,
    pub trailing_distance: i16,
    pub collision_half_width: i16,
    pub collision_half_height: i16,
    pub render_parameter_index: u16,
    pub base_tmap: u16,
    pub render_mode: u8,
    pub logical_height: u16,
    pub active: bool,
    pub visible: bool,
}

impl Level1Actor {
    #[must_use]
    pub const fn alive(self) -> bool {
        self.active && !self.collected && self.life >= 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level1CollisionTarget {
    pub entity_index: u16,
    pub position: Position,
    pub collision_half_width: i16,
    pub collision_vertical_offset: i16,
    pub collision_half_height: i16,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthoredAllocationState {
    Dormant,
    Active,
    PendingDelete,
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceOccupant {
    Reserved,
    ImportedLive,
    Authored(u16),
    PlayerFireball(u32),
    ArcherArrow(u32),
    FireflyFireball(u32),
    CastleBuild(u32),
    CastleRoot(u32),
    CastleConstruction(u32),
    Possession(u32),
    FireballImpact(u32),
    WaterSplash(u32),
    ManaSphere(u32),
    ManaCarrier(u32),
    DeathBurst(u32),
    MultipartSegment(u16),
}

impl SourceOccupant {
    const fn dynamic_shape(self) -> Option<(u8, u8, u8)> {
        match self {
            Self::PlayerFireball(_) | Self::FireflyFireball(_) => {
                Some((FIREBALL_PROJECTILE_CLASS, FIREBALL_PROJECTILE_MODEL, 0))
            }
            Self::ArcherArrow(_) => Some((
                ARCHER_ARROW_PROJECTILE_CLASS,
                ARCHER_ARROW_PROJECTILE_MODEL,
                13,
            )),
            Self::CastleBuild(_) => Some((
                CASTLE_PROJECTILE_CLASS,
                CASTLE_PROJECTILE_MODEL,
                CASTLE_PROJECTILE_ACTION,
            )),
            Self::CastleRoot(_) => Some((CASTLE_ROOT_CLASS, CASTLE_ROOT_MODEL, CASTLE_ROOT_ACTION)),
            Self::CastleConstruction(_) => Some((
                CASTLE_CONSTRUCTION_CLASS,
                CASTLE_CONSTRUCTION_MODEL,
                CASTLE_CONSTRUCTION_ACTION,
            )),
            Self::Possession(_) => Some((
                POSSESSION_PROJECTILE_CLASS,
                POSSESSION_PROJECTILE_MODEL,
                POSSESSION_PROJECTILE_ACTION,
            )),
            Self::FireballImpact(_) => Some((FIREBALL_IMPACT_CLASS, FIREBALL_IMPACT_MODEL, 0)),
            Self::WaterSplash(_) => Some((FIREBALL_IMPACT_CLASS, WATER_SPLASH_MODEL, 5)),
            Self::ManaSphere(_) => Some((MANA_SPHERE_CLASS, MANA_SPHERE_MODEL, MANA_SPHERE_ACTION)),
            Self::ManaCarrier(_) => {
                Some((MANA_CARRIER_CLASS, MANA_CARRIER_MODEL, MANA_CARRIER_ACTION))
            }
            Self::DeathBurst(_) => Some((DEATH_BURST_CLASS, DEATH_BURST_MODEL, DEATH_BURST_ACTION)),
            Self::MultipartSegment(_) => Some((
                MISSION_ACTOR_CLASS,
                MULTIPART_MODEL,
                MULTIPART_SEGMENT_ACTION,
            )),
            Self::Reserved | Self::ImportedLive | Self::Authored(_) => None,
        }
    }
}

fn bind_dynamic_source_record(
    slots: &mut [Level1ResetSourceSlot],
    source_slot: u16,
    occupant: SourceOccupant,
    global_rng: u32,
) {
    let Some((class, model, action)) = occupant.dynamic_shape() else {
        return;
    };
    let record = &mut slots[usize::from(source_slot)];
    record.active = true;
    record.class = class;
    record.model = model;
    record.action = action;
    record.phase = 0;
    record.entity_rng = source_slot.wrapping_add(global_rng as u16);
    record.control_flags = 8;
}

/// The bounded remc2 entity arena for the Level 1 slice.
///
/// Gameplay objects remain in typed vectors while the port is being recovered,
/// but every live object must own exactly one source slot in this arena.  Slot
/// zero is the original sentinel.  The free stack is stored in the same order
/// as remc2: `pop()` returns the lowest available slot.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Level1SourcePool {
    occupants: Box<[Option<SourceOccupant>]>,
    disabled: Box<[bool]>,
    replaceable: Box<[bool]>,
    free_stack: Vec<u16>,
    replaceable_stack: Vec<u16>,
    replaced_imported_slots: Vec<u16>,
}

impl Level1SourcePool {
    #[cfg(test)]
    fn from_free_stack(free_stack: &[u16]) -> Result<Self, String> {
        let mut occupants = vec![Some(SourceOccupant::ImportedLive); ENTITY_POOL_SIZE];
        occupants[0] = Some(SourceOccupant::Reserved);
        for &slot in free_stack {
            let index = usize::from(slot);
            if slot == 0 || index >= ENTITY_POOL_SIZE {
                return Err("Level 1 free entity-slot stack contains an invalid slot".to_owned());
            }
            if occupants[index].is_none() {
                return Err(format!(
                    "Level 1 free entity-slot stack contains duplicate slot {slot}"
                ));
            }
            occupants[index] = None;
        }
        Ok(Self {
            occupants: occupants.into_boxed_slice(),
            disabled: vec![false; ENTITY_POOL_SIZE].into_boxed_slice(),
            replaceable: vec![false; ENTITY_POOL_SIZE].into_boxed_slice(),
            free_stack: free_stack.to_vec(),
            replaceable_stack: Vec::new(),
            replaced_imported_slots: Vec::new(),
        })
    }

    fn from_reset_slots(slots: &[Level1ResetSourceSlot]) -> Result<Self, String> {
        if slots.len() != ENTITY_POOL_SIZE {
            return Err(format!(
                "Level 1 reset source projection must contain exactly {ENTITY_POOL_SIZE} slots"
            ));
        }
        let mut occupants = Vec::with_capacity(ENTITY_POOL_SIZE);
        for (index, slot) in slots.iter().copied().enumerate() {
            if usize::from(slot.index) != index {
                return Err(format!(
                    "Level 1 reset source slot/index mismatch at {index}: {}",
                    slot.index
                ));
            }
            if index == 0 {
                occupants.push(Some(SourceOccupant::Reserved));
            } else if slot.active {
                occupants.push(Some(SourceOccupant::ImportedLive));
            } else {
                occupants.push(None);
            }
        }
        let free_stack = (1..slots.len())
            .rev()
            .filter(|index| !slots[*index].active)
            .map(|index| u16::try_from(index).expect("source pool index fits u16"))
            .collect();
        let replaceable_stack = (1..slots.len())
            .rev()
            .filter(|index| slots[*index].active && slots[*index].replaceable())
            .map(|index| u16::try_from(index).expect("source pool index fits u16"))
            .collect();
        Ok(Self {
            occupants: occupants.into_boxed_slice(),
            disabled: vec![false; ENTITY_POOL_SIZE].into_boxed_slice(),
            replaceable: slots
                .iter()
                .map(|slot| slot.replaceable())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            free_stack,
            replaceable_stack,
            replaced_imported_slots: Vec::new(),
        })
    }

    fn allocate(&mut self, occupant: SourceOccupant) -> Option<u16> {
        let slot = if let Some(slot) = self.free_stack.pop() {
            slot
        } else {
            self.replaceable_stack.pop()?
        };
        let entry = self
            .occupants
            .get_mut(usize::from(slot))
            .expect("validated source slot");
        assert!(
            entry.is_none() || *entry == Some(SourceOccupant::ImportedLive),
            "allocatable source slot {slot} has a live typed occupant"
        );
        if *entry == Some(SourceOccupant::ImportedLive) {
            self.replaced_imported_slots.push(slot);
        }
        *entry = Some(occupant);
        self.disabled[usize::from(slot)] = false;
        self.replaceable[usize::from(slot)] = false;
        Some(slot)
    }

    fn has_ordinary_free_slot(&self) -> bool {
        !self.free_stack.is_empty()
    }

    #[cfg(test)]
    fn release(&mut self, slot: u16) -> bool {
        if slot == 0 {
            return false;
        }
        let Some(entry) = self.occupants.get_mut(usize::from(slot)) else {
            return false;
        };
        if entry.is_none() {
            return false;
        }
        *entry = None;
        self.disabled[usize::from(slot)] = false;
        self.replaceable[usize::from(slot)] = false;
        self.replaceable_stack
            .retain(|candidate| *candidate != slot);
        self.free_stack.push(slot);
        true
    }

    #[cfg(test)]
    fn release_occupant(&mut self, occupant: SourceOccupant) -> bool {
        let Some(slot) = self
            .occupants
            .iter()
            .position(|candidate| *candidate == Some(occupant))
            .and_then(|index| u16::try_from(index).ok())
        else {
            return false;
        };
        self.release(slot)
    }

    fn disable_slot(&mut self, slot: u16) -> bool {
        if slot == 0 {
            return false;
        }
        let index = usize::from(slot);
        if self.occupants.get(index).is_none_or(Option::is_none) {
            return false;
        }
        self.disabled[index] = true;
        true
    }

    fn disable_occupant(&mut self, occupant: SourceOccupant) -> bool {
        let Some(slot) = self
            .occupants
            .iter()
            .position(|candidate| *candidate == Some(occupant))
            .and_then(|index| u16::try_from(index).ok())
        else {
            return false;
        };
        self.disable_slot(slot)
    }

    fn replace_occupant(&mut self, slot: u16, occupant: SourceOccupant) {
        let entry = self
            .occupants
            .get_mut(usize::from(slot))
            .expect("validated source slot");
        assert!(entry.is_some(), "cannot replace an unallocated source slot");
        *entry = Some(occupant);
    }

    /// `sub_57F20` is called directly for an absorbed mana sphere, so that
    /// source slot becomes allocatable immediately rather than at the next
    /// `UpdateEntities_57730` sweep.
    fn reclaim_slot_immediately(&mut self, slot: u16) -> bool {
        if slot == 0 {
            return false;
        }
        let Some(entry) = self.occupants.get_mut(usize::from(slot)) else {
            return false;
        };
        if entry.is_none() {
            return false;
        }
        *entry = None;
        self.disabled[usize::from(slot)] = false;
        self.replaceable[usize::from(slot)] = false;
        self.replaceable_stack
            .retain(|candidate| *candidate != slot);
        self.free_stack.push(slot);
        true
    }

    /// `UpdateEntities_57730` reclaims disabled slots in ascending order and
    /// appends them to the existing allocation stack.
    fn reclaim_disabled_ascending(&mut self) -> Vec<u16> {
        let mut reclaimed = Vec::new();
        for index in 1..self.occupants.len() {
            if !self.disabled[index] {
                continue;
            }
            self.occupants[index] = None;
            self.disabled[index] = false;
            self.replaceable[index] = false;
            let slot = u16::try_from(index).expect("source pool index fits u16");
            self.replaceable_stack
                .retain(|candidate| *candidate != slot);
            self.free_stack.push(slot);
            reclaimed.push(slot);
        }
        reclaimed
    }

    /// `sub_49F90` rebuilds the allocation stack by scanning 999 down to 1
    /// immediately before authored group activation.
    fn rebuild_free_stack(&mut self) {
        let _ = self.reclaim_disabled_ascending();
        self.free_stack.clear();
        self.free_stack.extend(
            (1..self.occupants.len())
                .rev()
                .filter(|index| self.occupants[*index].is_none())
                .map(|index| u16::try_from(index).expect("source pool index fits u16")),
        );
        self.replaceable_stack.clear();
        self.replaceable_stack.extend(
            (1..self.occupants.len())
                .rev()
                .filter(|index| {
                    self.replaceable[*index]
                        && self.occupants[*index] == Some(SourceOccupant::ImportedLive)
                })
                .map(|index| u16::try_from(index).expect("source pool index fits u16")),
        );
    }

    fn occupant(&self, slot: u16) -> Option<SourceOccupant> {
        self.occupants.get(usize::from(slot)).copied().flatten()
    }

    fn take_replaced_imported_slots(&mut self) -> Vec<u16> {
        std::mem::take(&mut self.replaced_imported_slots)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthoredAllocation {
    authored_index: u16,
    kind: u16,
    subtype: i16,
    activation_group: i16,
    word_10: u16,
    stage_tag: i16,
    parameter_1: u16,
    parameter_2: u16,
    parameter_3: u16,
    position: Position,
    source_entity_index: u16,
    phase: u8,
    state_timer: i16,
    state: AuthoredAllocationState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FireballProjectile {
    pub id: u32,
    /// Authoritative remc2 entity-arena slot. `id` remains presentation-only.
    pub source_entity_index: u16,
    pub owner_entity_index: u16,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub speed: i16,
    pub life_ticks: i16,
    pub payload: i32,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArcherArrowProjectile {
    pub id: u32,
    pub source_entity_index: u16,
    pub owner_entity_index: u16,
    pub target_entity_index: u16,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub speed: i16,
    pub life_ticks: i16,
    pub payload: i32,
    pub sound_initialized: bool,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FireflyFireballProjectile {
    pub id: u32,
    /// Authoritative remc2 entity-arena slot. `id` remains presentation-only.
    pub source_entity_index: u16,
    pub owner_entity_index: u16,
    pub target_entity_index: u16,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub speed: i16,
    pub life_ticks: i16,
    pub payload: i32,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CastleBuildProjectile {
    pub id: u32,
    /// Authoritative remc2 entity-arena slot. `id` remains presentation-only.
    pub source_entity_index: u16,
    pub owner_entity_index: u16,
    pub position: Position,
    pub target: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub speed: i16,
    pub life_ticks: i16,
    /// Source `struct_byte_0xc_12_15 & 2`: the initial placement check runs
    /// exactly once before the first movement tick.
    pub placement_checked: bool,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CastleConstruction {
    pub id: u32,
    pub source_entity_index: u16,
    pub parent_entity_index: u16,
    pub owner_entity_index: u16,
    pub position: Position,
    pub stage: u8,
    /// Source `dword_0x10`: zero before first dispatch, then 18..1, -1, 0.
    pub timer: i16,
    pub initialized: bool,
    pub active: bool,
}

/// One ordered Castle terrain mutation. The session applies each update
/// immediately after its source-slot dispatch so later entities in the same
/// turn observe it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CastleTerrainUpdate {
    ConstructionPass { position: Position, remaining: i16 },
    ConstructionCompleted { position: Position },
    StageDestroyed { position: Position, entity_rng: u16 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PossessionProjectile {
    pub id: u32,
    /// Authoritative remc2 entity-arena slot. `id` remains presentation-only.
    pub source_entity_index: u16,
    pub owner_entity_index: u16,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub speed: i16,
    pub life_ticks: i16,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level1Projectile {
    PlayerFireball(FireballProjectile),
    ArcherArrow(ArcherArrowProjectile),
    FireflyFireball(FireflyFireballProjectile),
    CastleBuild(CastleBuildProjectile),
    Possession(PossessionProjectile),
}

impl Level1Projectile {
    #[must_use]
    pub const fn id(self) -> u32 {
        match self {
            Self::PlayerFireball(projectile) => projectile.id,
            Self::ArcherArrow(projectile) => projectile.id,
            Self::FireflyFireball(projectile) => projectile.id,
            Self::CastleBuild(projectile) => projectile.id,
            Self::Possession(projectile) => projectile.id,
        }
    }

    #[must_use]
    pub const fn active(self) -> bool {
        match self {
            Self::PlayerFireball(projectile) => projectile.active,
            Self::ArcherArrow(projectile) => projectile.active,
            Self::FireflyFireball(projectile) => projectile.active,
            Self::CastleBuild(projectile) => projectile.active,
            Self::Possession(projectile) => projectile.active,
        }
    }

    #[must_use]
    pub const fn source_entity_index(self) -> u16 {
        match self {
            Self::PlayerFireball(projectile) => projectile.source_entity_index,
            Self::ArcherArrow(projectile) => projectile.source_entity_index,
            Self::FireflyFireball(projectile) => projectile.source_entity_index,
            Self::CastleBuild(projectile) => projectile.source_entity_index,
            Self::Possession(projectile) => projectile.source_entity_index,
        }
    }

    #[must_use]
    pub const fn position(self) -> Position {
        match self {
            Self::PlayerFireball(projectile) => projectile.position,
            Self::ArcherArrow(projectile) => projectile.position,
            Self::FireflyFireball(projectile) => projectile.position,
            Self::CastleBuild(projectile) => projectile.position,
            Self::Possession(projectile) => projectile.position,
        }
    }

    const fn source_occupant(self) -> SourceOccupant {
        match self {
            Self::PlayerFireball(projectile) => SourceOccupant::PlayerFireball(projectile.id),
            Self::ArcherArrow(projectile) => SourceOccupant::ArcherArrow(projectile.id),
            Self::FireflyFireball(projectile) => SourceOccupant::FireflyFireball(projectile.id),
            Self::CastleBuild(projectile) => SourceOccupant::CastleBuild(projectile.id),
            Self::Possession(projectile) => SourceOccupant::Possession(projectile.id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManaSphere {
    /// remc2's entity ID is the bounded source-arena slot. This is repeated
    /// as `u32` only to preserve the existing presentation ABI.
    pub id: u32,
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub position: Position,
    pub velocity_x: i16,
    pub velocity_y: i16,
    pub vertical_velocity: i16,
    pub yaw: i16,
    pub mana: i32,
    /// Zero is neutral; otherwise this is the owning player entity.
    pub owner_entity_index: u16,
    pub incoming_owner_entity_index: u16,
    pub assigned_carrier_id: u32,
    pub phase: u8,
    pub active: bool,
    /// `DisableEntityDrawing04_57F10` marks the source row for the next
    /// ascending reclaim pass; the typed row remains dispatchable for the
    /// rest of the current source turn.
    pub pending_delete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeathBurst {
    pub source_entity_index: u16,
    pub parent_entity_index: u16,
    pub position: Position,
    pub life_ticks: i16,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManaCarrier {
    /// Presentation identity is the authoritative source-arena slot. remc2's
    /// raw `id_0x1A_26` is overwritten with the owning player relation.
    pub id: u32,
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub owner_entity_index: u16,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub speed: i16,
    pub life: i32,
    pub max_life: i32,
    pub max_mana: i32,
    /// Source `word_0x96`: zero, a mana-sphere source slot, or the Castle
    /// source slot when `returning_to_castle` is true.
    pub target_sphere_id: u32,
    pub carried_mana: i32,
    pub returning_to_castle: bool,
    pub pending_damage: i32,
    pub pending_attacker_entity_index: u16,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FireballImpact {
    pub id: u32,
    /// Authoritative remc2 entity-arena slot. `id` remains presentation-only.
    pub source_entity_index: u16,
    pub source_projectile_id: u32,
    pub owner_entity_index: u16,
    pub external_target_entity_index: u16,
    pub model: u8,
    pub position: Position,
    pub payload: i32,
    pub life_ticks: i16,
    pub entity_rng: u16,
    pub vertical_velocity: i16,
    pub animation_frame: u8,
    pub damage_submitted: bool,
    pub active: bool,
}

/// The state installed by remc2's `sub_5EFA0` when the player consumes a
/// non-shielded damage mailbox. Flight has already advanced for the current
/// source turn; `recoil_*` therefore affects the next turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlayerHitReaction {
    pub attacker_entity_index: u16,
    pub raw_damage: i32,
    pub applied_damage: i32,
    pub recoil_yaw: i16,
    pub recoil_pitch: i16,
    pub recoil_speed: i16,
    pub pain_sound_id: u16,
    pub hit_frame_ticks: u8,
    pub regen_block_ticks: u16,
    pub hit_aux_ticks: u16,
    pub survived_feedback: u16,
    pub killer_entity_index: u16,
    pub lethal: bool,
}

impl FireballImpact {
    const fn source_occupant(self) -> SourceOccupant {
        if self.model == WATER_SPLASH_MODEL {
            SourceOccupant::WaterSplash(self.id)
        } else {
            SourceOccupant::FireballImpact(self.id)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level1MissionEvent {
    ObjectiveAdvanced {
        completed_stage: u8,
        text_selector: u8,
    },
    SpellCollected {
        family: u8,
        authored_index: u16,
    },
    FireballSpawned {
        projectile_id: u32,
    },
    PossessionSpawned {
        projectile_id: u32,
    },
    ManaSphereSpawned {
        sphere_id: u32,
        source_entity_index: u16,
        mana: i32,
    },
    ManaSphereClaimed {
        sphere_id: u32,
        owner_entity_index: u16,
        sound_id: u16,
    },
    ManaSphereCollected {
        sphere_id: u32,
        carrier_id: u32,
        mana: i32,
    },
    ArcherArrowSpawned {
        projectile_id: u32,
        archer_authored_index: u16,
    },
    ArcherArrowSound {
        projectile_id: u32,
        sound_id: u16,
        source_entity_index: u16,
        position: Position,
        random_state: u32,
    },
    FireflyFireballSpawned {
        projectile_id: u32,
        firefly_authored_index: u16,
    },
    FireflyChargeSound {
        firefly_authored_index: u16,
        sound_id: u16,
        random_state: u32,
    },
    FireballImpacted {
        projectile_id: u32,
        impact_id: u32,
        position: Position,
        sound_id: u16,
    },
    WaterSplashed {
        projectile_id: u32,
        splash_id: u32,
        position: Position,
        sound_id: u16,
    },
    ActorDamaged {
        authored_index: u16,
        damage: i32,
        life: i32,
    },
    ActorKilled {
        authored_index: u16,
        kind: Level1ActorKind,
    },
    CastleBuilt {
        level: u8,
    },
    CastleProjectileSpawned {
        projectile_id: u32,
        sound_id: u16,
    },
    CastlePlacementRejected {
        projectile_id: u32,
    },
    CastleConstructionSound {
        sound_id: u16,
        source_entity_index: u16,
        position: Position,
    },
    CastleManaChanged {
        mana: i32,
    },
    PlayerDamaged {
        damage: i32,
        life: i32,
    },
    PlayerDied,
    MissionSound {
        sound_id: u16,
        source_entity_index: u16,
        position: Position,
        random_state: u32,
    },
    ExitSequenceStarted,
    ExitSequenceCompleted {
        result_code: u8,
    },
    LevelCompleted,
}

/// One reset-time entry in either of remc2's two ordered castle-placement
/// blocker lists: live class-3/model-2 roots first, then class-10/model-45
/// authored buildings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level1CastlePlacementBlocker {
    pub source_entity_index: u16,
    pub position: Position,
    pub half_x: i16,
    pub half_y: i16,
}

/// Exact owner BUILD terrain cells selected by one castle stage.
///
/// Every cell is the source two-byte tuple consumed by
/// `AddTerrainMod0A_2A_37BC0`: material/control then target-height delta.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CastleBuildFootprint {
    pub index: u8,
    pub width: u8,
    pub height: u8,
    cells: Box<[[u8; 2]]>,
}

impl CastleBuildFootprint {
    /// Validate a materialized owner BUILD payload.
    ///
    /// # Errors
    ///
    /// Returns an error for zero dimensions, a non-two-byte-aligned payload,
    /// or a cell count that differs from `width * height`.
    pub fn from_owner_cells(
        index: u8,
        width: u8,
        height: u8,
        payload: &[u8],
    ) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("Castle BUILD dimensions must be nonzero".to_owned());
        }
        let expected = usize::from(width) * usize::from(height) * 2;
        if payload.len() != expected {
            return Err(format!(
                "Castle BUILD payload has {} bytes, expected {expected}",
                payload.len()
            ));
        }
        let cells = payload
            .chunks_exact(2)
            .map(|cell| [cell[0], cell[1]])
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(Self {
            index,
            width,
            height,
            cells,
        })
    }

    #[must_use]
    pub fn cells(&self) -> &[[u8; 2]] {
        &self.cells
    }

    #[cfg(test)]
    fn synthetic_stage_one() -> Self {
        Self {
            index: 1,
            width: 8,
            height: 8,
            cells: vec![[0xff, 0xff]; 64].into_boxed_slice(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level1MissionTemplate {
    actors: Box<[Level1Actor]>,
    authored_allocations: Box<[AuthoredAllocation]>,
    global_mana_denominator: i32,
    initial_global_rng: u32,
    initial_source_slots: Box<[Level1ResetSourceSlot]>,
    initial_castle_blockers: Box<[Level1CastlePlacementBlocker]>,
    initial_building_blockers: Box<[Level1CastlePlacementBlocker]>,
    castle_stage_one_footprint: Option<CastleBuildFootprint>,
}

impl Level1MissionTemplate {
    /// Decode and validate the narrow authored Level 1 content contract.
    ///
    /// # Errors
    ///
    /// Returns an error when the package no longer contains the exact four
    /// group-3 Archers, five group-6 Fireflies, or the two teaching jars.
    pub fn decode<W: FlightWorld>(bytes: &[u8], world: &W) -> Result<Self, String> {
        Self::decode_with_mana_denominator(bytes, world, 19_577)
    }

    /// Decode Level 1 while binding the denominator reconstructed by
    /// remc2's `sub_60F00` qualifying-entity scan.
    ///
    /// # Errors
    ///
    /// Returns an error when the denominator is non-positive or the authored
    /// Level 1 content no longer matches the narrow mission contract.
    pub fn decode_with_mana_denominator<W: FlightWorld>(
        bytes: &[u8],
        world: &W,
        global_mana_denominator: i32,
    ) -> Result<Self, String> {
        let initial_free_entity_slots = (1..ENTITY_POOL_SIZE)
            .rev()
            .filter(|index| !LEVEL1_SYNTHETIC_RESET_OCCUPIED_SLOTS.contains(index))
            .map(|index| u16::try_from(index).expect("entity pool index fits u16"))
            .collect::<Vec<_>>();
        Self::decode_with_runtime_context(
            bytes,
            world,
            global_mana_denominator,
            0,
            &initial_free_entity_slots,
        )
    }

    /// Decode Level 1 with the remc2 runtime RNG and the source-ordered free
    /// entity-slot stack reconstructed from the owner-data reset package.
    ///
    /// `initial_free_entity_slots` must be in allocation-stack order: the
    /// next (lowest available) source slot is the final element. The present
    /// mission state tracks every authored allocation in the reachable Level
    /// 1 group chain, including switches, trees, and the stone. The C ABI
    /// retains its allocation-gap bit until the already-live group-0 switches
    /// and transient effect allocations share this same pool.
    pub fn decode_with_runtime_context<W: FlightWorld>(
        bytes: &[u8],
        world: &W,
        global_mana_denominator: i32,
        initial_global_rng: u32,
        initial_free_entity_slots: &[u16],
    ) -> Result<Self, String> {
        let mut slots = (0..ENTITY_POOL_SIZE)
            .map(|index| Level1ResetSourceSlot {
                index: u16::try_from(index).expect("source pool index fits u16"),
                active: index != 0,
                class: u8::from(index != 0),
                model: 0,
                action: 0,
                phase: 0,
                entity_rng: 0,
                control_flags: 0,
            })
            .collect::<Vec<_>>();
        for &slot in initial_free_entity_slots {
            let index = usize::from(slot);
            if slot == 0 || index >= ENTITY_POOL_SIZE || !slots[index].active {
                return Err(format!(
                    "Level 1 free entity-slot stack contains invalid or duplicate slot {slot}"
                ));
            }
            slots[index].active = false;
            slots[index].class = 0;
        }
        Self::decode_with_source_slots(
            bytes,
            world,
            global_mana_denominator,
            initial_global_rng,
            &slots,
        )
    }

    /// Decode Level 1 with the complete reset-time source arena. This is the
    /// owner-data path used by `NativeWorld`; unlike the compatibility free
    /// stack entry point it preserves class/model/action/phase/entity RNG and
    /// the replaceable allocation flag for all 1,000 slots.
    pub fn decode_with_source_slots<W: FlightWorld>(
        bytes: &[u8],
        world: &W,
        global_mana_denominator: i32,
        initial_global_rng: u32,
        initial_source_slots: &[Level1ResetSourceSlot],
    ) -> Result<Self, String> {
        Self::decode_with_source_slots_and_placement_blockers(
            bytes,
            world,
            global_mana_denominator,
            initial_global_rng,
            initial_source_slots,
            &[],
            &[],
            None,
        )
    }

    /// Owner-data entry point that additionally retains the exact two
    /// reset-time blocker lists traversed by remc2's `sub_11CB0` and the
    /// dynamically selected BUILD stage-one footprint.
    pub fn decode_with_source_slots_and_placement_blockers<W: FlightWorld>(
        bytes: &[u8],
        world: &W,
        global_mana_denominator: i32,
        initial_global_rng: u32,
        initial_source_slots: &[Level1ResetSourceSlot],
        initial_castle_blockers: &[Level1CastlePlacementBlocker],
        initial_building_blockers: &[Level1CastlePlacementBlocker],
        castle_stage_one_footprint: Option<CastleBuildFootprint>,
    ) -> Result<Self, String> {
        if global_mana_denominator <= 0 {
            return Err("Level 1 global mana denominator must be positive".to_owned());
        }
        Level1SourcePool::from_reset_slots(initial_source_slots)?;
        let features = parse_authored_features(bytes)?;
        let mut actors = Vec::with_capacity(19);
        let mut authored_allocations = Vec::with_capacity(55);

        for feature in features {
            if feature.kind != 0 && LEVEL1_ALLOCATION_GROUPS.contains(&feature.activation_group) {
                let mut position = feature.world_position(0);
                position.z = world.ground_height(position);
                authored_allocations.push(AuthoredAllocation {
                    authored_index: feature.index,
                    kind: feature.kind,
                    subtype: feature.subtype,
                    activation_group: feature.activation_group,
                    word_10: feature.word_10,
                    stage_tag: feature.stage_tag,
                    parameter_1: feature.parameter_1,
                    parameter_2: feature.parameter_2,
                    parameter_3: feature.parameter_3,
                    position,
                    source_entity_index: 0,
                    phase: 0,
                    state_timer: 0,
                    state: AuthoredAllocationState::Dormant,
                });
            }
            let actor = match (
                feature.kind,
                feature.subtype,
                feature.activation_group,
                feature.index,
            ) {
                (5, 4, 3, _) => Some(actor_from_feature(
                    feature,
                    Level1ActorKind::Archer,
                    MISSION_ACTOR_CLASS,
                    ARCHER_MODEL,
                    ARCHER_ACTION,
                    ARCHER_LIFE,
                    world,
                )),
                (5, 19, 6 | 33, _) => Some(actor_from_feature(
                    feature,
                    Level1ActorKind::Firefly,
                    MISSION_ACTOR_CLASS,
                    FIREFLY_MODEL,
                    FIREFLY_STAGE_ACTION,
                    FIREFLY_LIFE,
                    world,
                )),
                (5, 3, 4 | 34, _) => Some(actor_from_feature(
                    feature,
                    Level1ActorKind::MultipartHead,
                    MISSION_ACTOR_CLASS,
                    MULTIPART_MODEL,
                    MULTIPART_ROAM_ACTION,
                    MULTIPART_LIFE,
                    world,
                )),
                (14, 5, 4, 304) => Some(actor_from_feature(
                    feature,
                    Level1ActorKind::ExperienceScroll,
                    14,
                    5,
                    10,
                    0,
                    world,
                )),
                (15, 2, 1, CASTLE_JAR_FEATURE) => Some(actor_from_feature(
                    feature,
                    Level1ActorKind::CastleSpellJar,
                    15,
                    2,
                    7,
                    0,
                    world,
                )),
                (15, 3, 11, SPEED_JAR_FEATURE) => Some(actor_from_feature(
                    feature,
                    Level1ActorKind::SpeedSpellJar,
                    15,
                    3,
                    10,
                    0,
                    world,
                )),
                (14, 3, 4, 132) => Some(actor_from_feature(
                    feature,
                    Level1ActorKind::ExitTarget,
                    14,
                    3,
                    8,
                    0,
                    world,
                )),
                (11, 12, 4, LEVEL1_EXIT_SWITCH_FEATURE) => Some(actor_from_feature(
                    feature,
                    Level1ActorKind::ExitSwitch,
                    11,
                    12,
                    12,
                    0,
                    world,
                )),
                _ => None,
            };
            if let Some(actor) = actor {
                actors.push(actor);
            }
        }

        let archers = actors
            .iter()
            .filter(|actor| actor.kind == Level1ActorKind::Archer)
            .count();
        let fireflies = actors
            .iter()
            .filter(|actor| actor.kind == Level1ActorKind::Firefly)
            .count();
        let castle_jars = actors
            .iter()
            .filter(|actor| actor.kind == Level1ActorKind::CastleSpellJar)
            .count();
        let speed_jars = actors
            .iter()
            .filter(|actor| actor.kind == Level1ActorKind::SpeedSpellJar)
            .count();
        let exit_targets = actors
            .iter()
            .filter(|actor| actor.kind == Level1ActorKind::ExitTarget)
            .count();
        let exit_switches = actors
            .iter()
            .filter(|actor| actor.kind == Level1ActorKind::ExitSwitch)
            .count();
        let multipart_heads = actors
            .iter()
            .filter(|actor| actor.kind == Level1ActorKind::MultipartHead)
            .count();
        let experience_scrolls = actors
            .iter()
            .filter(|actor| actor.kind == Level1ActorKind::ExperienceScroll)
            .count();
        if (
            archers,
            fireflies,
            castle_jars,
            speed_jars,
            exit_targets,
            exit_switches,
            multipart_heads,
            experience_scrolls,
        ) != (4, 6, 1, 1, 1, 1, 4, 1)
        {
            return Err(format!(
                "Level 1 mission content mismatch: {archers} Archers, {fireflies} Fireflies, {castle_jars} Castle jars, {speed_jars} Speed jars, {exit_targets} exit targets, {exit_switches} exit switches, {multipart_heads} multipart heads, {experience_scrolls} experience scrolls"
            ));
        }
        actors.sort_by_key(|actor| actor.authored_index);
        authored_allocations.sort_by_key(|allocation| allocation.authored_index);
        if authored_allocations
            .iter()
            .map(|allocation| allocation.authored_index)
            .ne(LEVEL1_ALLOCATION_AUTHORED_INDICES)
        {
            return Err(
                "Level 1 reachable authored-allocation manifest no longer matches the remc2 map"
                    .to_owned(),
            );
        }
        Ok(Self {
            actors: actors.into_boxed_slice(),
            authored_allocations: authored_allocations.into_boxed_slice(),
            global_mana_denominator,
            initial_global_rng,
            initial_source_slots: initial_source_slots.to_vec().into_boxed_slice(),
            initial_castle_blockers: initial_castle_blockers.to_vec().into_boxed_slice(),
            initial_building_blockers: initial_building_blockers.to_vec().into_boxed_slice(),
            castle_stage_one_footprint,
        })
    }

    #[must_use]
    pub fn session(&self, player_life: i32, player_mana: i32) -> Level1MissionState {
        let mut actors = self.actors.to_vec();
        for actor in &mut actors {
            actor.active = false;
        }
        Level1MissionState {
            current_stage: 0,
            current_text_selector: 48,
            objective_textbox_ticks: 33,
            level_completed: false,
            exit_sequence_active: false,
            exit_sequence_complete: false,
            exit_sequence_phase: 0,
            exit_sequence_timer: 0,
            exit_result_code: 0,
            exit_player_action: 0,
            exit_target_source_entity_index: 0,
            exit_target_valid: false,
            exit_end_sequence_flag: false,
            exit_player_status_flags: 0,
            exit_palette_submode: EXIT_PALETTE_NONE,
            exit_palette_effect: EXIT_PALETTE_EFFECT_NONE,
            exit_palette_weight: 0,
            exit_palette_denominator: 0,
            exit_palette_step: 0,
            exit_palette_fade_started: false,
            exit_palette_fade_effect: EXIT_PALETTE_EFFECT_NONE,
            exit_palette_count: 0,
            exit_last_movement_resolution: EXIT_MOVEMENT_NOT_ATTEMPTED,
            exit_display_blank: false,
            exit_blur_mode: 0,
            objective_states: [1; 5],
            objective_pause: 0,
            initial_zone_switch_active: true,
            initial_zone_switch_phase: LEVEL1_INITIAL_ZONE_SWITCH_PHASE,
            first_waypoint_switch_active: true,
            first_waypoint_switch_phase: LEVEL1_FIRST_WAYPOINT_SWITCH_PHASE,
            completion_switch_active: true,
            pending_source_slot_releases: Vec::new(),
            reclaimed_imported_source_slots: Vec::new(),
            player_life,
            player_max_life: player_life,
            player_pending_damage: 0,
            player_pending_attacker: 0,
            player_life_regen_cooldown: 0,
            player_hit_frame_ticks: 0,
            player_hit_aux_ticks: 0,
            player_survived_feedback: 0,
            player_killer_entity_index: 0,
            pending_player_hit_reaction: None,
            player_damage_suppression: 0,
            source_turn_player_start_position: Position::default(),
            player_mana,
            player_max_mana: player_mana,
            player_dead: false,
            has_fireball: true,
            has_possession: true,
            has_castle_spell: false,
            has_speed_spell: false,
            castle_exists: false,
            castle_source_entity_index: 0,
            castle_presentation_id: 0,
            castle_owner_entity_index: 0,
            castle_action: CASTLE_ROOT_ACTION,
            castle_build_state: 0,
            castle_level: 0,
            castle_position: Position::default(),
            castle_construction_anchor: Position::default(),
            castle_life: 0,
            castle_max_life: 0,
            castle_mana: 0,
            castle_mana_capacity: 0,
            castle_pending_damage: 0,
            castle_pending_attacker: 0,
            castle_killer_entity_index: 0,
            castle_owner_hit_feedback: 0,
            secured_mana_component: 0,
            global_mana_denominator: self.global_mana_denominator,
            initial_castle_blockers: self.initial_castle_blockers.to_vec(),
            initial_building_blockers: self.initial_building_blockers.to_vec(),
            actors,
            multipart_segments: Vec::new(),
            external_collision_targets: Vec::new(),
            authored_allocations: self.authored_allocations.to_vec(),
            projectiles: Vec::new(),
            impacts: Vec::new(),
            mana_spheres: Vec::new(),
            mana_sphere_map_chains: RenderableMapChains::empty(),
            mana_carrier: None,
            death_bursts: Vec::new(),
            castle_construction: None,
            next_effect_id: 1,
            global_rng: self.initial_global_rng,
            source_turn_active: false,
            archers_present_at_turn_start: false,
            fireflies_present_at_turn_start: false,
            source_slots: self.initial_source_slots.to_vec(),
            source_pool: Level1SourcePool::from_reset_slots(&self.initial_source_slots)
                .expect("template validated the Level 1 source pool"),
            model_phase_counters: [0; 256],
            events: Vec::new(),
            castle_terrain_updates: Vec::new(),
            castle_stage_one_footprint: self.castle_stage_one_footprint.clone(),
        }
    }

    #[must_use]
    pub fn actors(&self) -> &[Level1Actor] {
        &self.actors
    }

    #[must_use]
    pub fn castle_placement_blockers(
        &self,
    ) -> (
        &[Level1CastlePlacementBlocker],
        &[Level1CastlePlacementBlocker],
    ) {
        (
            &self.initial_castle_blockers,
            &self.initial_building_blockers,
        )
    }

    #[must_use]
    pub fn castle_stage_one_footprint(&self) -> Option<&CastleBuildFootprint> {
        self.castle_stage_one_footprint.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level1MissionState {
    pub current_stage: u8,
    pub current_text_selector: u8,
    pub objective_textbox_ticks: u8,
    pub level_completed: bool,
    pub exit_sequence_active: bool,
    pub exit_sequence_complete: bool,
    pub exit_sequence_phase: u8,
    pub exit_sequence_timer: i16,
    pub exit_result_code: u8,
    pub exit_player_action: u8,
    pub exit_target_source_entity_index: u16,
    pub exit_target_valid: bool,
    pub exit_end_sequence_flag: bool,
    pub exit_player_status_flags: u8,
    pub exit_palette_submode: u8,
    pub exit_palette_effect: u8,
    pub exit_palette_weight: u8,
    pub exit_palette_denominator: u8,
    exit_palette_step: u8,
    exit_palette_fade_started: bool,
    exit_palette_fade_effect: u8,
    exit_palette_count: i16,
    pub exit_last_movement_resolution: u8,
    pub exit_display_blank: bool,
    pub exit_blur_mode: u8,
    objective_states: [u8; 5],
    objective_pause: u8,
    initial_zone_switch_active: bool,
    initial_zone_switch_phase: u8,
    first_waypoint_switch_active: bool,
    first_waypoint_switch_phase: u8,
    completion_switch_active: bool,
    pending_source_slot_releases: Vec<u16>,
    reclaimed_imported_source_slots: Vec<u16>,
    pub player_life: i32,
    pub player_max_life: i32,
    player_pending_damage: i32,
    player_pending_attacker: u16,
    player_life_regen_cooldown: u16,
    player_hit_frame_ticks: u8,
    player_hit_aux_ticks: u16,
    player_survived_feedback: u16,
    player_killer_entity_index: u16,
    pending_player_hit_reaction: Option<PlayerHitReaction>,
    player_damage_suppression: u8,
    source_turn_player_start_position: Position,
    pub player_mana: i32,
    pub player_max_mana: i32,
    pub player_dead: bool,
    pub has_fireball: bool,
    pub has_possession: bool,
    pub has_castle_spell: bool,
    pub has_speed_spell: bool,
    pub castle_exists: bool,
    pub castle_source_entity_index: u16,
    pub castle_presentation_id: u32,
    pub castle_owner_entity_index: u16,
    pub castle_action: u8,
    pub castle_build_state: i16,
    pub castle_level: u8,
    pub castle_position: Position,
    pub castle_construction_anchor: Position,
    pub castle_life: i32,
    pub castle_max_life: i32,
    pub castle_mana: i32,
    pub castle_mana_capacity: i32,
    castle_pending_damage: i32,
    castle_pending_attacker: u16,
    castle_killer_entity_index: u16,
    castle_owner_hit_feedback: u8,
    pub secured_mana_component: i32,
    pub global_mana_denominator: i32,
    initial_castle_blockers: Vec<Level1CastlePlacementBlocker>,
    initial_building_blockers: Vec<Level1CastlePlacementBlocker>,
    castle_stage_one_footprint: Option<CastleBuildFootprint>,
    actors: Vec<Level1Actor>,
    multipart_segments: Vec<Level1MultipartSegment>,
    external_collision_targets: Vec<Level1CollisionTarget>,
    authored_allocations: Vec<AuthoredAllocation>,
    projectiles: Vec<Level1Projectile>,
    impacts: Vec<FireballImpact>,
    mana_spheres: Vec<ManaSphere>,
    mana_sphere_map_chains: RenderableMapChains,
    mana_carrier: Option<ManaCarrier>,
    death_bursts: Vec<DeathBurst>,
    castle_construction: Option<CastleConstruction>,
    next_effect_id: u32,
    global_rng: u32,
    source_turn_active: bool,
    archers_present_at_turn_start: bool,
    fireflies_present_at_turn_start: bool,
    source_slots: Vec<Level1ResetSourceSlot>,
    source_pool: Level1SourcePool,
    model_phase_counters: [u8; 256],
    events: Vec<Level1MissionEvent>,
    castle_terrain_updates: Vec<CastleTerrainUpdate>,
}

impl Level1MissionState {
    #[must_use]
    pub fn actors(&self) -> &[Level1Actor] {
        &self.actors
    }

    /// Reproduce the Level 1 branch of `DrawMinimapMarks_64A10` at the
    /// source 128x128 minimap resolution.
    ///
    /// Aperture rejection remains a presentation responsibility because the
    /// scanline mask is owned by HSPRN/TABLESN cache data. Target selection,
    /// source ordering, wrapped distance, blink gates, arrow placement and
    /// arrow vertices stay in the deterministic simulation.
    #[must_use]
    pub fn minimap_marks(&self, player: &FlightState) -> Level1MinimapMarks {
        const SIDE: i32 = 128;
        const SCALING: i32 = 256;
        const RED: u8 = 119;
        const GOLD: u8 = 184;
        const FIXED_HELP: u8 = 0x51;
        const ENTITY_HELP: u8 = 0x52;

        let mut result = Level1MinimapMarks {
            mark_pulse: player.tick & 3 == 0,
            ..Level1MinimapMarks::default()
        };
        let mut targets = Vec::<(Position, u8, u8)>::new();
        let mut arrow_target = None;
        let mut nearest_distance = u32::MAX;

        match self.current_stage {
            0 => {
                targets.push((LEVEL1_FIRST_WAYPOINT, GOLD, FIXED_HELP));
                arrow_target = Some(LEVEL1_FIRST_WAYPOINT);
            }
            1 => {
                targets.push((LEVEL1_SECOND_WAYPOINT, GOLD, FIXED_HELP));
                arrow_target = Some(LEVEL1_SECOND_WAYPOINT);
            }
            2 | 4 => {
                let wanted_model = if self.current_stage == 2 {
                    ARCHER_MODEL
                } else {
                    FIREFLY_MODEL
                };
                let mut actors = self
                    .actors
                    .iter()
                    .filter(|actor| {
                        actor.active
                            && actor.class == MISSION_ACTOR_CLASS
                            && actor.model == wanted_model
                            && actor.life >= 0
                            && !matches!(actor.action, 180 | 232 | 234)
                    })
                    .collect::<Vec<_>>();
                actors.sort_by_key(|actor| actor.source_entity_index);
                for actor in actors {
                    targets.push((actor.position, RED, ENTITY_HELP));
                    let distance = legacy_minimap_distance(player.position, actor.position);
                    if distance < nearest_distance {
                        nearest_distance = distance;
                        arrow_target = Some(actor.position);
                    }
                }
            }
            3 => {}
            _ => {
                let mut switches = self
                    .actors
                    .iter()
                    .filter(|actor| {
                        actor.active && actor.class == 11 && actor.model == 12 && actor.life >= 0
                    })
                    .collect::<Vec<_>>();
                switches.sort_by_key(|actor| actor.source_entity_index);
                arrow_target = switches.first().map(|actor| actor.position);
            }
        }

        assert!(
            targets.len() <= LEVEL1_MINIMAP_MAX_MARKS,
            "qualified Level 1 objective mark extent"
        );
        for (ordinal, (target, colour, help_selector)) in targets.iter().enumerate() {
            let (x, y) = level1_minimap_project(player, *target, SIDE, SCALING);
            result.mark_x[ordinal] = x;
            result.mark_y[ordinal] = y;
            result.mark_colour[ordinal] = *colour;
            result.mark_help_selector[ordinal] = *help_selector;
        }
        result.mark_count = u8::try_from(targets.len()).expect("Level 1 mark extent fits u8");

        if let Some(target) = arrow_target {
            let distance = legacy_minimap_distance(player.position, target);
            let heading = i16::try_from(heading_for_positions(player.position, target))
                .expect("legacy heading fits i16");
            let mut arrow_distance =
                i32::try_from(distance).expect("wrapped XY distance fits i32") - 512;
            if arrow_distance > 15_872 {
                arrow_distance = 15_872;
            }
            let mut anchor = player.position;
            move_entity(
                &mut anchor,
                heading,
                0,
                i16::try_from(arrow_distance).expect("clamped arrow distance fits i16"),
            );
            let (anchor_x, anchor_y) = level1_minimap_project(player, anchor, SIDE, SCALING);
            let relative = usize::from(heading.wrapping_sub(player.yaw).cast_unsigned() & 0x07ff);
            let sine = i64::from(SIN_16_16[relative]);
            let cosine = i64::from(SIN_16_16[relative + 512]);
            const BASE: [(i16, i16); 7] = [
                (0, 0),
                (9, 13),
                (-9, 13),
                (4, 13),
                (4, 23),
                (-4, 23),
                (-4, 13),
            ];
            for (ordinal, (base_x, base_y)) in BASE.into_iter().enumerate() {
                let (rotated_x, rotated_y) = if ordinal == 0 {
                    (0_i64, 0_i64)
                } else {
                    let scaled_x = i64::from(base_x) * 16;
                    let scaled_y = i64::from(base_y) * 16;
                    (
                        (cosine * scaled_x - sine * scaled_y) >> 20,
                        (cosine * scaled_y + sine * scaled_x) >> 20,
                    )
                };
                result.arrow_xy[ordinal * 2] = anchor_x.wrapping_add(rotated_x as i16);
                result.arrow_xy[ordinal * 2 + 1] = anchor_y.wrapping_add(rotated_y as i16);
            }
            result.arrow_valid = true;
            result.arrow_visible = player.tick & 0x40 != 0 && ((player.tick / 6) & 1) != 0;
        }

        result
    }

    #[must_use]
    pub fn multipart_segments(&self) -> &[Level1MultipartSegment] {
        &self.multipart_segments
    }

    pub fn set_external_collision_targets(
        &mut self,
        targets: impl IntoIterator<Item = Level1CollisionTarget>,
    ) {
        self.external_collision_targets.clear();
        self.external_collision_targets.extend(targets);
    }

    pub fn update_external_collision_target(&mut self, target: Level1CollisionTarget) {
        if let Some(existing) = self
            .external_collision_targets
            .iter_mut()
            .find(|existing| existing.entity_index == target.entity_index)
        {
            *existing = target;
        } else {
            self.external_collision_targets.push(target);
        }
    }

    #[must_use]
    pub fn projectiles(&self) -> &[Level1Projectile] {
        &self.projectiles
    }

    #[must_use]
    pub fn impacts(&self) -> &[FireballImpact] {
        &self.impacts
    }

    #[must_use]
    pub fn death_bursts(&self) -> &[DeathBurst] {
        &self.death_bursts
    }

    #[must_use]
    pub fn mana_spheres(&self) -> &[ManaSphere] {
        &self.mana_spheres
    }

    #[must_use]
    pub fn mana_carrier(&self) -> Option<&ManaCarrier> {
        self.mana_carrier.as_ref().filter(|carrier| carrier.active)
    }

    #[must_use]
    pub const fn castle_construction(&self) -> Option<&CastleConstruction> {
        self.castle_construction.as_ref()
    }

    #[must_use]
    pub fn castle_stage_one_footprint(&self) -> Option<&CastleBuildFootprint> {
        self.castle_stage_one_footprint.as_ref()
    }

    pub fn drain_castle_terrain_updates(
        &mut self,
    ) -> impl Iterator<Item = CastleTerrainUpdate> + '_ {
        self.castle_terrain_updates.drain(..)
    }

    #[must_use]
    pub fn events(&self) -> &[Level1MissionEvent] {
        &self.events
    }

    #[must_use]
    pub const fn global_rng(&self) -> u32 {
        self.global_rng
    }

    /// Take the hit reaction installed during the player's source-slot
    /// dispatch. The native session applies this to `FlightState` only after
    /// that dispatch, matching remc2's next-turn recoil timing.
    pub fn take_player_hit_reaction(&mut self) -> Option<PlayerHitReaction> {
        self.pending_player_hit_reaction.take()
    }

    #[must_use]
    pub const fn player_killer_entity_index(&self) -> u16 {
        self.player_killer_entity_index
    }

    pub fn clear_events(&mut self) {
        self.events.clear();
    }

    pub fn disable_imported_source_slot(&mut self, source_slot: u16) -> bool {
        let disabled = matches!(
            self.source_pool.occupants.get(usize::from(source_slot)),
            Some(Some(SourceOccupant::ImportedLive))
        ) && self.source_pool.disable_slot(source_slot);
        if disabled {
            self.source_slots[usize::from(source_slot)].control_flags |= SOURCE_DISABLED_FLAG;
        }
        disabled
    }

    /// Drain imported source slots reclaimed by the arena's single ascending
    /// deletion pass. Runtime-owned typed projections (currently Goats) use
    /// this to retire their payload without independently mutating free-stack
    /// order.
    pub fn take_reclaimed_imported_source_slots(&mut self) -> Vec<u16> {
        let mut source_slots = std::mem::take(&mut self.reclaimed_imported_source_slots);
        source_slots.extend(self.source_pool.take_replaced_imported_slots());
        source_slots
    }

    /// Whether the reset-imported entity still owns this source arena slot.
    ///
    /// Dynamic and authored replacements have typed mission projections, so
    /// presentation clients must stop exposing the stale imported row as soon
    /// as remc2's fallback allocator reuses it.
    #[must_use]
    pub fn imported_source_slot_is_live(&self, source_slot: u16) -> bool {
        self.source_pool.occupant(source_slot) == Some(SourceOccupant::ImportedLive)
    }

    /// Complete remc2-shaped source arena used by presentation-order and
    /// render-flag gates.
    #[must_use]
    pub fn source_slots(&self) -> &[Level1ResetSourceSlot] {
        &self.source_slots
    }

    /// Synchronize the mutable fields of a typed imported row back into the
    /// shared source arena. This keeps the arena projection authoritative
    /// while Goat/player payloads remain strongly typed during recovery.
    pub fn sync_imported_source_slot(
        &mut self,
        source_slot: u16,
        active: bool,
        action: u8,
        phase: u8,
        entity_rng: u16,
        control_flags: u32,
    ) {
        if !self.imported_source_slot_is_live(source_slot) {
            return;
        }
        let Some(record) = self.source_slots.get_mut(usize::from(source_slot)) else {
            return;
        };
        record.active = active;
        record.action = action;
        record.phase = phase;
        record.entity_rng = entity_rng;
        record.control_flags = control_flags;
    }

    /// Hash the complete bounded source-arena bookkeeping projection.
    ///
    /// Typed gameplay payloads have their own hashes; this digest pins the
    /// shared ownership, allocation, disable/reclaim, and source-row state
    /// that determines dispatch order and constructor identity.
    #[must_use]
    pub fn source_arena_hash_v1(&self) -> u64 {
        let mut hash = Level1Fnv1a64::new();
        hash.bytes(b"mc2-level1-source-arena-v1\0");
        hash.u32(u32::try_from(self.source_slots.len()).expect("source arena size fits u32"));
        for (index, record) in self.source_slots.iter().copied().enumerate() {
            hash.u16(u16::try_from(index).expect("source arena index fits u16"));
            hash.u16(record.index);
            hash.u8(u8::from(record.active));
            hash.u8(record.class);
            hash.u8(record.model);
            hash.u8(record.action);
            hash.u8(record.phase);
            hash.u16(record.entity_rng);
            hash.u32(record.control_flags);
            hash.source_occupant(self.source_pool.occupants[index]);
            hash.u8(u8::from(self.source_pool.disabled[index]));
            hash.u8(u8::from(self.source_pool.replaceable[index]));
        }
        hash.u32(
            u32::try_from(self.source_pool.free_stack.len()).expect("free stack length fits u32"),
        );
        for slot in &self.source_pool.free_stack {
            hash.u16(*slot);
        }
        hash.u32(
            u32::try_from(self.source_pool.replaceable_stack.len())
                .expect("replaceable stack length fits u32"),
        );
        for slot in &self.source_pool.replaceable_stack {
            hash.u16(*slot);
        }
        hash.u32(
            u32::try_from(self.source_pool.replaced_imported_slots.len())
                .expect("replacement queue length fits u32"),
        );
        for slot in &self.source_pool.replaced_imported_slots {
            hash.u16(*slot);
        }
        hash.u32(
            u32::try_from(self.pending_source_slot_releases.len())
                .expect("pending release queue length fits u32"),
        );
        for slot in &self.pending_source_slot_releases {
            hash.u16(*slot);
        }
        hash.u32(
            u32::try_from(self.reclaimed_imported_source_slots.len())
                .expect("reclaimed imported queue length fits u32"),
        );
        for slot in &self.reclaimed_imported_source_slots {
            hash.u16(*slot);
        }
        hash.u32(
            u32::try_from(self.authored_allocations.len())
                .expect("authored allocation count fits u32"),
        );
        for allocation in &self.authored_allocations {
            hash.u16(allocation.authored_index);
            hash.u16(allocation.source_entity_index);
            hash.u8(match allocation.state {
                AuthoredAllocationState::Dormant => 0,
                AuthoredAllocationState::Active => 1,
                AuthoredAllocationState::PendingDelete => 2,
                AuthoredAllocationState::Released => 3,
            });
            hash.u8(allocation.phase);
        }
        hash.u32(self.global_rng);
        hash.u32(self.next_effect_id);
        hash.bytes(&self.model_phase_counters);
        hash.finish()
    }

    pub fn spawn_fireball(&mut self, player: &FlightState) -> bool {
        if self.player_dead || !self.has_fireball || self.player_mana < FIREBALL_MANA_COST {
            return false;
        }
        let Some((id, source_entity_index)) =
            self.allocate_effect_source(SourceOccupant::PlayerFireball)
        else {
            return false;
        };
        self.player_mana -= FIREBALL_MANA_COST;
        let mut position = player.position;
        position.z = position.z.wrapping_add(100);
        self.projectiles
            .push(Level1Projectile::PlayerFireball(FireballProjectile {
                id,
                source_entity_index,
                owner_entity_index: player.entity_index,
                position,
                yaw: player.yaw,
                pitch: player.effective_pitch,
                speed: FIREBALL_SPEED,
                life_ticks: FIREBALL_LIFETIME_TICKS,
                payload: FIREBALL_PAYLOAD,
                active: true,
            }));
        self.events
            .push(Level1MissionEvent::FireballSpawned { projectile_id: id });
        self.events.push(Level1MissionEvent::MissionSound {
            sound_id: FIREBALL_LAUNCH_SOUND,
            source_entity_index: player.entity_index,
            position: player.position,
            random_state: self.global_rng,
        });
        true
    }

    pub fn spawn_possession(&mut self, player: &FlightState) -> bool {
        if self.player_dead || !self.has_possession || self.player_mana < POSSESSION_MANA_COST {
            return false;
        }
        let Some((id, source_entity_index)) =
            self.allocate_effect_source(SourceOccupant::Possession)
        else {
            return false;
        };
        self.player_mana -= POSSESSION_MANA_COST;
        let mut position = player.position;
        position.z = position.z.wrapping_add(100);
        self.projectiles
            .push(Level1Projectile::Possession(PossessionProjectile {
                id,
                source_entity_index,
                owner_entity_index: player.entity_index,
                position,
                yaw: player.yaw,
                pitch: player.effective_pitch,
                speed: POSSESSION_PROJECTILE_SPEED,
                life_ticks: POSSESSION_PROJECTILE_LIFETIME_TICKS,
                active: true,
            }));
        self.events
            .push(Level1MissionEvent::PossessionSpawned { projectile_id: id });
        self.events.push(Level1MissionEvent::MissionSound {
            sound_id: POSSESSION_LAUNCH_SOUND,
            source_entity_index,
            position,
            random_state: self.global_rng,
        });
        true
    }

    pub fn spawn_mana_sphere_from_death<W: FlightWorld>(
        &mut self,
        world: &W,
        source_entity_index: u16,
        position: Position,
        source_yaw: i16,
        source_rng_after_death: u16,
        mana: i32,
        owner_entity_index: u16,
    ) -> Option<u32> {
        if mana <= 0 {
            return None;
        }
        if let Some(source_record) = self.source_slots.get_mut(usize::from(source_entity_index)) {
            source_record.entity_rng = source_rng_after_death;
        }
        if let Some(source_actor) = self
            .actors
            .iter_mut()
            .find(|actor| actor.source_entity_index == source_entity_index)
        {
            source_actor.entity_rng = source_rng_after_death;
            source_actor.owner_id = 0;
        }
        let sphere_slot = self.allocate_mana_sphere_source()?;
        let id = u32::from(sphere_slot);
        let sphere_rng = next_rng16(self.source_slots[usize::from(sphere_slot)].entity_rng);
        let phase = sphere_slot as u8;
        self.source_slots[usize::from(sphere_slot)].phase = phase;
        let yaw =
            i16::try_from((i32::from(sphere_rng % 0x71) + i32::from(source_yaw) - 56) & 0x07ff)
                .expect("Mana sphere yaw is 11-bit");
        let speed_rng = next_rng16(sphere_rng);
        self.source_slots[usize::from(sphere_slot)].entity_rng = speed_rng;
        let speed = i16::try_from(speed_rng % 0x30 + 16).expect("Mana sphere speed fits i16");
        let mut velocity = Position::default();
        move_entity(&mut velocity, yaw, 0, speed);
        let height_above_ground = i32::from(position.z) - i32::from(world.ground_height(position));
        let vertical_velocity = (1_024 - height_above_ground).wrapping_div(8) as i16;
        self.mana_spheres.push(ManaSphere {
            id,
            source_entity_index: sphere_slot,
            entity_rng: speed_rng,
            position,
            velocity_x: velocity.x.cast_signed(),
            velocity_y: velocity.y.cast_signed(),
            vertical_velocity,
            yaw,
            mana,
            owner_entity_index,
            incoming_owner_entity_index: 0,
            assigned_carrier_id: 0,
            phase,
            active: true,
            pending_delete: false,
        });
        self.mana_sphere_map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: sphere_slot,
                active: true,
                x: position.x,
                y: position.y,
            })
            .expect("qualified mana sphere source slot fits the map chain");
        self.events.push(Level1MissionEvent::ManaSphereSpawned {
            sphere_id: id,
            source_entity_index,
            mana,
        });
        Some(id)
    }

    /// Allocate the class-10/model-1 burst created immediately after a
    /// creature's mana conversion. Although short-lived, this allocation is
    /// part of the source slot/RNG schedule.
    pub fn spawn_death_burst_from_death(
        &mut self,
        source_entity_index: u16,
        position: Position,
    ) -> Option<u16> {
        let burst_slot = self.allocate_death_burst_source()?;
        self.death_bursts.push(DeathBurst {
            source_entity_index: burst_slot,
            parent_entity_index: source_entity_index,
            position,
            life_ticks: 1,
            active: true,
        });
        Some(burst_slot)
    }

    /// Activate the tier-0 Castle holder and create remc2's class-9/model-10
    /// fixed-axis build projectile.
    ///
    /// The caller supplies the combined player/view yaw because the private
    /// renderer-free mission kernel does not own camera state.
    pub fn cast_initial_castle<W: FlightWorld>(
        &mut self,
        world: &W,
        player: &FlightState,
        combined_yaw: i16,
    ) -> bool {
        self.cast_initial_castle_inner(world, player, combined_yaw, false)
    }

    fn cast_accepted_initial_castle<W: FlightWorld>(
        &mut self,
        world: &W,
        player: &FlightState,
        combined_yaw: i16,
    ) -> bool {
        self.cast_initial_castle_inner(world, player, combined_yaw, true)
    }

    fn cast_initial_castle_inner<W: FlightWorld>(
        &mut self,
        world: &W,
        player: &FlightState,
        combined_yaw: i16,
        accepted_this_turn: bool,
    ) -> bool {
        let holder_active = self.castle_holder_active_value();
        if self.player_dead
            || !self.has_castle_spell
            || self.castle_exists
            || self.player_mana < CASTLE_INITIAL_MANA_COST
            || holder_active > 0
                && !(accepted_this_turn && holder_active == CASTLE_HOLDER_ACTIVATION_TICKS)
        {
            return false;
        }
        let Some(holder_index) = self
            .actors
            .iter()
            .position(|actor| actor.kind == Level1ActorKind::CastleSpellJar && actor.collected)
        else {
            return false;
        };

        let id = self.next_effect_id;
        let Some(source_entity_index) = self.source_pool.allocate(SourceOccupant::CastleBuild(id))
        else {
            return false;
        };
        bind_dynamic_source_record(
            &mut self.source_slots,
            source_entity_index,
            SourceOccupant::CastleBuild(id),
            self.global_rng,
        );
        self.next_effect_id = self.next_effect_id.wrapping_add(1).max(1);
        self.player_mana -= CASTLE_INITIAL_MANA_COST;
        self.actors[holder_index].state_value = i32::from(CASTLE_HOLDER_ACTIVATION_TICKS);
        let mut position = player.position;
        // `sub_68E50`'s exact hand-specific launch offset remains an explicit
        // presentation seam. The gameplay target and projectile update are
        // source-mapped; using the caster origin avoids inventing an offset.
        position.z = position.z.wrapping_add(100);
        let mut target = player.position;
        move_entity(
            &mut target,
            combined_yaw & 0x07ff,
            0,
            CASTLE_PROJECTILE_TARGET_DISTANCE,
        );
        target.z = world.ground_height(target);
        let horizontal = legacy_sqrt(squared_wrapping_xy_distance(position, target));
        let yaw =
            i16::try_from(heading_for_positions(position, target)).expect("11-bit Castle heading");
        let pitch = pitch_for_delta(
            i32::try_from(horizontal).expect("Castle target distance fits i32"),
            i32::from(target.z.wrapping_sub(position.z)),
        );
        self.projectiles
            .push(Level1Projectile::CastleBuild(CastleBuildProjectile {
                id,
                source_entity_index,
                owner_entity_index: player.entity_index,
                position,
                target,
                yaw,
                pitch,
                speed: CASTLE_PROJECTILE_SPEED,
                life_ticks: CASTLE_PROJECTILE_LIFETIME_TICKS,
                placement_checked: false,
                active: true,
            }));
        self.events
            .push(Level1MissionEvent::CastleProjectileSpawned {
                projectile_id: id,
                sound_id: CASTLE_PROJECTILE_SOUND,
            });
        true
    }

    #[must_use]
    pub fn castle_holder_active_value(&self) -> i16 {
        self.actors
            .iter()
            .find(|actor| actor.kind == Level1ActorKind::CastleSpellJar && actor.collected)
            .map_or(0, |actor| {
                i16::try_from(actor.state_value).expect("Castle holder active value fits i16")
            })
    }

    pub fn set_castle_holder_active_value(&mut self, value: i16) {
        if let Some(holder) = self
            .actors
            .iter_mut()
            .find(|actor| actor.kind == Level1ActorKind::CastleSpellJar && actor.collected)
        {
            holder.state_value = i32::from(value);
        }
    }

    #[must_use]
    pub fn speed_holder_active_value(&self) -> i16 {
        self.actors
            .iter()
            .find(|actor| actor.kind == Level1ActorKind::SpeedSpellJar && actor.collected)
            .map_or(0, |actor| {
                i16::try_from(actor.state_value).expect("Speed holder active value fits i16")
            })
    }

    pub fn capture_spell_holder_active_values(&mut self, context: &FlightContext) {
        let Some(fire) = context.spell_fire.as_ref() else {
            return;
        };
        for actor in self.actors.iter_mut().filter(|actor| actor.collected) {
            if let Some(holder) = fire
                .holders
                .iter()
                .find(|holder| holder.entity_index == actor.source_entity_index)
            {
                actor.state_value = i32::from(holder.active_value);
            }
        }
        if let Some(speed) = context.speed_spell_holder
            && let Some(actor) = self.actors.iter_mut().find(|actor| {
                actor.kind == Level1ActorKind::SpeedSpellJar
                    && actor.collected
                    && actor.source_entity_index == speed.entity_index
            })
        {
            actor.state_value = i32::from(speed.active_value);
        }
    }

    /// Project collected mission-owned spell jars into the live player spell
    /// registry without changing the immutable reset-conformance context.
    pub fn sync_collected_spell_holders(&self, context: &mut FlightContext) {
        let Some(fire) = context.spell_fire.as_mut() else {
            return;
        };
        fire.caster.mana = self.player_mana;
        for actor in self
            .actors
            .iter()
            .filter(|actor| actor.active && actor.collected && actor.source_entity_index != 0)
        {
            let (family, activation_value, mana_cost) = match actor.kind {
                Level1ActorKind::CastleSpellJar => (
                    CASTLE_SPELL_FAMILY,
                    CASTLE_HOLDER_ACTIVATION_TICKS.cast_unsigned(),
                    CASTLE_INITIAL_MANA_COST,
                ),
                Level1ActorKind::SpeedSpellJar => (
                    SPEED_SPELL_FAMILY,
                    SPEED_SPELL_ACTIVATION_TICKS,
                    SPEED_SPELL_MANA_COST,
                ),
                Level1ActorKind::Archer
                | Level1ActorKind::Firefly
                | Level1ActorKind::ExitTarget
                | Level1ActorKind::ExitSwitch
                | Level1ActorKind::MultipartHead
                | Level1ActorKind::ExperienceScroll => continue,
            };
            let holder_index = actor.source_entity_index;
            let existing = fire
                .holders
                .iter()
                .position(|holder| holder.entity_index == holder_index);
            if let Some(index) = existing {
                fire.holders[index].active_value =
                    i16::try_from(actor.state_value).expect("spell holder active value fits i16");
            } else {
                fire.holders.push(SpellActivationHolder {
                    entity_index: holder_index,
                    model: family,
                    level_gate: 0,
                    active_value: i16::try_from(actor.state_value)
                        .expect("spell holder active value fits i16"),
                    activation_value,
                    mana_cost,
                    action_marker: 0,
                });
                // SetSpell fills an empty hand; with both initial hands
                // occupied, each collected teaching jar replaces the left.
                if fire.left_holder_index == 0 {
                    fire.left_holder_index = holder_index;
                } else if fire.right_holder_index == 0 {
                    fire.right_holder_index = holder_index;
                } else {
                    fire.left_holder_index = holder_index;
                }
                fire.selected_holder_index = holder_index;
                fire.selected_spell_ui_index = family;
            }
            fire.enabled_holder_indices[usize::from(family)] = holder_index;
        }
    }

    pub fn sync_speed_holder_projection(
        &self,
        state: &mut FlightState,
        context: &mut FlightContext,
    ) {
        let Some(actor) = self.actors.iter().find(|actor| {
            actor.kind == Level1ActorKind::SpeedSpellJar
                && actor.active
                && actor.collected
                && actor.source_entity_index != 0
        }) else {
            state.speed_spell_holder_index = 0;
            context.speed_spell_holder = None;
            return;
        };
        state.speed_spell_holder_index = actor.source_entity_index;
        context.speed_spell_holder = Some(SpellHolder {
            entity_index: actor.source_entity_index,
            active_value: i16::try_from(actor.state_value)
                .expect("Speed holder active value fits i16"),
        });
    }

    /// Dispatch the collected tier-zero Speed holder (`GetScroll_69DB0`).
    ///
    /// The source writes both requested and actual speed, uses a one-tick
    /// three-times launch impulse, then sustains twice the normal minimum
    /// speed until the 301-tick holder counter expires.
    pub fn advance_speed_spell(&mut self, state: &mut FlightState, context: &mut FlightContext) {
        let Some(actor) = self.actors.iter_mut().find(|actor| {
            actor.kind == Level1ActorKind::SpeedSpellJar
                && actor.active
                && actor.collected
                && actor.source_entity_index != 0
        }) else {
            return;
        };
        let Some(fire) = context.spell_fire.as_mut() else {
            return;
        };
        let Some(holder) = fire
            .holders
            .iter_mut()
            .find(|holder| holder.entity_index == actor.source_entity_index)
        else {
            return;
        };
        actor.state_value = i32::from(holder.active_value);
        if actor.state_value <= 0 {
            return;
        }
        if state.speed_changed_flag != 0 {
            actor.state_value = 1;
        } else {
            let direction = if state.target_speed < 0 { -1 } else { 1 };
            let multiplier = if actor.state_value == i32::from(SPEED_SPELL_ACTIVATION_TICKS) {
                context.events.push(crate::FlightEvent::PrepareSound {
                    entity_index: state.entity_index,
                    player_index: i16::from(state.player_color_index),
                    sound_id: EXIT_ACCELERATION_SOUND,
                });
                SPEED_SPELL_TIER_MULTIPLIER + 1
            } else {
                SPEED_SPELL_TIER_MULTIPLIER
            };
            state.target_speed = direction * crate::PLAYER_MIN_SPEED * multiplier;
            state.actual_speed = state.target_speed;
        }
        actor.state_value -= 1;
        if actor.state_value == 0 {
            let direction = if state.target_speed < 0 { -1 } else { 1 };
            state.target_speed = direction * crate::PLAYER_MIN_SPEED;
            state.actual_speed = state.target_speed;
        }
        holder.active_value =
            i16::try_from(actor.state_value).expect("Speed holder countdown fits i16");
        if let Some(projected) = context.speed_spell_holder.as_mut() {
            projected.active_value = holder.active_value;
        }
        if actor.state_value > 0 && !self.exit_sequence_owns_player_action() {
            self.exit_palette_submode = LEVEL1_PALETTE_SPEED_REQUEST;
            // Level 1 owns the tier-one source speed state:
            // 171 * 1 / 3 + 85.
            self.exit_palette_count = 142;
        }
    }

    fn submit_player_damage(&mut self, damage: i32, attacker: u16) {
        if self.player_dead || damage <= 0 || attacker == 0 {
            return;
        }
        if self.player_pending_attacker == 0 {
            self.player_pending_damage = damage;
        } else {
            self.player_pending_damage = self.player_pending_damage.saturating_add(damage);
        }
        self.player_pending_attacker = attacker;
    }

    fn submit_player_contact_damage(&mut self, damage: i32, attacker: u16) {
        if self.player_dead || damage <= 0 || attacker == 0 {
            return;
        }
        // Firefly charge uses `sub_11900`, whose mailbox behavior differs
        // from the area writer: overwrite an already-owned mailbox, otherwise
        // add to the old payload.
        if self.player_pending_attacker != 0 {
            self.player_pending_damage = damage;
        } else {
            self.player_pending_damage = self.player_pending_damage.saturating_add(damage);
        }
        self.player_pending_attacker = attacker;
    }

    fn submit_castle_damage(&mut self, damage: i32, attacker: u16) {
        if !self.castle_exists || self.castle_life < 0 || damage <= 0 || attacker == 0 {
            return;
        }
        if self.castle_pending_attacker == 0 {
            self.castle_pending_damage = damage;
        } else {
            self.castle_pending_damage = self.castle_pending_damage.wrapping_add(damage);
        }
        self.castle_pending_attacker = attacker;
    }

    fn player_inside_stage_one_castle(&self, player_start_position: Position) -> bool {
        self.castle_exists
            && source_entities_overlap(
                player_start_position,
                PLAYER_COLLISION_HALF_WIDTH,
                PLAYER_COLLISION_VERTICAL_OFFSET,
                PLAYER_COLLISION_HALF_HEIGHT,
                self.castle_position,
                CASTLE_STAGE_ONE_COLLISION_HALF_WIDTH,
                CASTLE_STAGE_ONE_COLLISION_VERTICAL_OFFSET,
                CASTLE_STAGE_ONE_COLLISION_HALF_HEIGHT,
            )
    }

    fn forward_sheltered_player_damage_to_castle(&mut self) {
        if self.player_pending_attacker != 0 {
            self.submit_castle_damage(self.player_pending_damage, self.player_pending_attacker);
        }
        // `AddPlayer03_00` installs two suppression passes while the
        // start-of-turn player AABB overlaps its linked castle.
        self.player_damage_suppression = 2;
    }

    fn consume_castle_damage_mailbox(&mut self) {
        if !self.castle_exists || self.castle_pending_attacker == 0 {
            return;
        }
        let attacker = self.castle_pending_attacker;
        self.castle_life = self.castle_life.wrapping_sub(self.castle_pending_damage);
        if self.castle_life < 0 {
            self.castle_killer_entity_index = attacker;
            self.castle_owner_hit_feedback = 0;
        } else {
            self.castle_owner_hit_feedback = 4;
        }
        self.castle_pending_damage = 0;
        self.castle_pending_attacker = 0;
    }

    fn source_entity_position(&self, entity_index: u16, player: &FlightState) -> Option<Position> {
        if entity_index == player.entity_index {
            return Some(player.position);
        }
        if entity_index == self.castle_source_entity_index && self.castle_exists {
            return Some(self.castle_position);
        }
        self.actors
            .iter()
            .find(|actor| actor.source_entity_index == entity_index && actor.active)
            .map(|actor| actor.position)
            .or_else(|| {
                self.external_collision_targets
                    .iter()
                    .find(|target| target.entity_index == entity_index && target.active)
                    .map(|target| target.position)
            })
    }

    fn consume_player_damage_mailbox(&mut self, player: &FlightState) {
        if self.player_dead || self.player_pending_attacker == 0 {
            return;
        }
        let attacker = self.player_pending_attacker;
        let damage = self.player_pending_damage;
        let attacker_position = self
            .source_entity_position(attacker, player)
            .expect("Level 1 damage mailbox attacker must retain a typed position");
        let recoil_yaw = i16::try_from(heading_for_positions(attacker_position, player.position))
            .expect("11-bit player recoil yaw");
        let horizontal = i32::try_from(wrapping_xy_distance(attacker_position, player.position))
            .expect("wrapped Level 1 horizontal distance fits i32");
        let recoil_pitch = pitch_for_delta(
            horizontal,
            i32::from(player.position.z.wrapping_sub(attacker_position.z)),
        );
        let recoil_speed = i16::try_from((damage / 10).clamp(0, 80))
            .expect("clamped player recoil speed fits i16");

        self.player_life = self.player_life.wrapping_sub(damage);
        self.player_life_regen_cooldown = PLAYER_REGEN_BLOCK_TICKS;
        self.player_hit_frame_ticks = PLAYER_HIT_FRAME_TICKS;
        self.player_hit_aux_ticks = PLAYER_HIT_AUX_TICKS;
        if !self.exit_sequence_owns_player_action() {
            self.exit_palette_submode = LEVEL1_PALETTE_HIT_REQUEST;
        }
        let player_rng = &mut self.source_slots[usize::from(player.entity_index)].entity_rng;
        *player_rng = next_rng16(*player_rng);
        let pain_sound_id = PLAYER_PAIN_SOUND_BASE + (*player_rng & 3);
        let lethal = self.player_life < 0;
        let (survived_feedback, killer_entity_index) = if lethal {
            self.player_killer_entity_index = attacker;
            (0, attacker)
        } else {
            self.player_survived_feedback = PLAYER_SURVIVED_FEEDBACK;
            self.player_pending_damage = 0;
            self.player_pending_attacker = 0;
            (PLAYER_SURVIVED_FEEDBACK, 0)
        };
        self.pending_player_hit_reaction = Some(PlayerHitReaction {
            attacker_entity_index: attacker,
            raw_damage: damage,
            applied_damage: damage,
            recoil_yaw,
            recoil_pitch,
            recoil_speed,
            pain_sound_id,
            hit_frame_ticks: PLAYER_HIT_FRAME_TICKS,
            regen_block_ticks: PLAYER_REGEN_BLOCK_TICKS,
            hit_aux_ticks: PLAYER_HIT_AUX_TICKS,
            survived_feedback,
            killer_entity_index,
            lethal,
        });
        self.events.push(Level1MissionEvent::PlayerDamaged {
            damage,
            life: self.player_life,
        });
        self.events.push(Level1MissionEvent::MissionSound {
            sound_id: pain_sound_id,
            source_entity_index: player.entity_index,
            position: player.position,
            random_state: self.global_rng,
        });
        if lethal {
            self.player_dead = true;
            self.events.push(Level1MissionEvent::PlayerDied);
            self.events.push(Level1MissionEvent::MissionSound {
                sound_id: PLAYER_DEATH_SOUND,
                source_entity_index: player.entity_index,
                position: player.position,
                random_state: self.global_rng,
            });
        }
    }

    pub fn step<W: FlightWorld>(&mut self, world: &W, player: &FlightState) {
        self.step_with_fireball(world, player, false);
    }

    pub fn step_with_fireball<W: FlightWorld>(
        &mut self,
        world: &W,
        player: &FlightState,
        spawn_fireball: bool,
    ) {
        self.step_with_spells(world, player, spawn_fireball, false, None);
    }

    pub fn step_with_spells<W: FlightWorld>(
        &mut self,
        world: &W,
        player: &FlightState,
        spawn_fireball: bool,
        spawn_possession: bool,
        castle_cast_yaw: Option<i16>,
    ) {
        self.begin_source_turn_at(player.position);
        for source_slot in 1..ENTITY_POOL_SIZE {
            self.dispatch_source_slot(
                world,
                player,
                u16::try_from(source_slot).expect("Level 1 source cursor fits u16"),
                spawn_fireball,
                spawn_possession,
                castle_cast_yaw,
            );
        }
        self.finish_source_turn(player);
    }

    pub fn begin_source_turn(&mut self) {
        self.events.clear();
        if self.objective_textbox_ticks != 0 {
            self.objective_textbox_ticks -= 1;
        }
        self.source_turn_active = !self.player_dead;
        if !self.source_turn_active {
            return;
        }
        self.archers_present_at_turn_start = self.count_alive(Level1ActorKind::Archer) != 0;
        self.fireflies_present_at_turn_start = self.count_alive(Level1ActorKind::Firefly) != 0;

        // `UpdateEntities_57730` advances the shared 32-bit generator before
        // reclaim and source dispatch. Mission-owned effects retain that exact
        // order even though Unity never owns or advances simulation RNG.
        self.global_rng = next_rng32(self.global_rng);
        self.cleanup_pending_allocations();
        self.advance_firefly_stage_rows();
    }

    /// Begin a source turn while preserving the player position sampled before
    /// `sub_5F380` flight. Castle shelter uses this position even though hit
    /// recoil is computed from the post-flight position.
    pub fn begin_source_turn_at(&mut self, player_start_position: Position) {
        self.source_turn_player_start_position = player_start_position;
        self.begin_source_turn();
    }

    pub fn dispatch_source_slot<W: FlightWorld>(
        &mut self,
        world: &W,
        player: &FlightState,
        source_slot: u16,
        spawn_fireball: bool,
        spawn_possession: bool,
        castle_cast_yaw: Option<i16>,
    ) {
        if !self.source_turn_active {
            return;
        }
        if source_slot == player.entity_index {
            if self.imported_source_slot_is_live(source_slot) {
                let record = &mut self.source_slots[usize::from(source_slot)];
                record.active = true;
                record.phase = player.entity_phase;
            }
            // Player action 12 bypasses the normal action-0 damage, regen, and
            // spell branches. Whether it begins this turn depends on whether
            // the exit switch's source slot dispatched before this player.
            if !self.exit_sequence_owns_player_action() {
                // Shelter is sampled before `sub_5F380` moves the player, then
                // mailbox forwarding/clearing happens after flight.
                if self.player_inside_stage_one_castle(self.source_turn_player_start_position) {
                    self.forward_sheltered_player_damage_to_castle();
                }
                if self.player_damage_suppression != 0 {
                    self.player_pending_damage = 0;
                    self.player_pending_attacker = 0;
                    self.player_damage_suppression -= 1;
                } else {
                    self.consume_player_damage_mailbox(player);
                }
                self.player_mana = self
                    .player_mana
                    .saturating_add(PLAYER_OUTDOOR_MANA_REGEN)
                    .min(self.player_max_mana);
                if self.player_life_regen_cooldown != 0 {
                    self.player_life_regen_cooldown -= 1;
                } else {
                    self.player_life = self
                        .player_life
                        .saturating_add(PLAYER_OUTDOOR_LIFE_REGEN)
                        .min(self.player_max_life);
                }
                if self.player_hit_aux_ticks != 0 {
                    self.player_hit_aux_ticks -= 1;
                }
                if spawn_fireball {
                    self.spawn_fireball(player);
                }
                if spawn_possession {
                    self.spawn_possession(player);
                }
                if let Some(combined_yaw) = castle_cast_yaw {
                    self.cast_accepted_initial_castle(world, player, combined_yaw);
                }
            }
        }
        self.step_switch_slot(player, source_slot);
        self.update_spell_pickup_slot(player, source_slot);
        self.update_experience_scroll_slot(world, player, source_slot);
        let occupant = self.source_pool.occupant(source_slot);
        let handled = match occupant {
            Some(SourceOccupant::Authored(_)) => {
                self.step_actor_slot(world, player, source_slot);
                true
            }
            Some(
                SourceOccupant::PlayerFireball(_)
                | SourceOccupant::ArcherArrow(_)
                | SourceOccupant::FireflyFireball(_)
                | SourceOccupant::CastleBuild(_)
                | SourceOccupant::Possession(_),
            ) => {
                self.step_projectile_slot(world, player, source_slot);
                true
            }
            Some(SourceOccupant::FireballImpact(_) | SourceOccupant::WaterSplash(_)) => {
                self.step_impact_slot(world, player, source_slot);
                true
            }
            Some(SourceOccupant::ManaSphere(_)) => {
                self.step_mana_sphere_slot(world, player.entity_index, source_slot);
                true
            }
            Some(SourceOccupant::ManaCarrier(_)) => {
                self.step_mana_carrier_slot(world, source_slot);
                true
            }
            Some(SourceOccupant::DeathBurst(_)) => {
                self.step_death_burst_slot(source_slot);
                true
            }
            Some(SourceOccupant::MultipartSegment(_)) => {
                self.step_multipart_segment_slot(world, source_slot);
                true
            }
            Some(SourceOccupant::CastleRoot(_)) => {
                self.step_castle_root_slot(world, source_slot);
                true
            }
            Some(SourceOccupant::CastleConstruction(_)) => {
                self.step_castle_construction_slot(source_slot);
                true
            }
            Some(SourceOccupant::Reserved | SourceOccupant::ImportedLive) | None => false,
        };
        if handled && self.source_pool.occupant(source_slot) == occupant {
            if let Some(SourceOccupant::Authored(authored_index)) = occupant
                && let Some(actor) = self
                    .actors
                    .iter()
                    .find(|actor| actor.authored_index == authored_index)
            {
                let record = &mut self.source_slots[usize::from(source_slot)];
                record.action = actor.action;
                record.phase = actor.phase;
                record.entity_rng = actor.entity_rng;
                if !actor.active {
                    record.control_flags |= SOURCE_DISABLED_FLAG;
                }
            } else if let Some(SourceOccupant::MultipartSegment(_)) = occupant
                && let Some(segment) = self
                    .multipart_segments
                    .iter()
                    .find(|segment| segment.source_entity_index == source_slot)
            {
                let record = &mut self.source_slots[usize::from(source_slot)];
                record.action = segment.action;
                record.phase = segment.phase;
                if !segment.active {
                    record.control_flags |= SOURCE_DISABLED_FLAG;
                }
            } else {
                self.source_slots[usize::from(source_slot)].phase = self.source_slots
                    [usize::from(source_slot)]
                .phase
                .wrapping_add(1);
            }
        }
    }

    pub fn finish_source_turn(&mut self, player: &FlightState) {
        if !self.source_turn_active {
            return;
        }
        self.evaluate_objective(
            player.position,
            self.archers_present_at_turn_start,
            self.fireflies_present_at_turn_start,
        );
        self.projectiles.retain(|projectile| projectile.active());
        self.impacts.retain(|impact| impact.active);
        self.source_turn_active = false;
    }

    /// Advance remc2's frame-level palette state. `PaletteChanges_47760` runs
    /// once before input/entity dispatch, independently from the number of
    /// source-entity passes in that rendered frame.
    pub fn advance_exit_palette_frame(&mut self) {
        match self.exit_palette_submode {
            LEVEL1_PALETTE_HIT_REQUEST => {
                self.begin_palette_fade_back(LEVEL1_PALETTE_EFFECT_HIT);
            }
            LEVEL1_PALETTE_SPEED_REQUEST => {
                self.begin_palette_fade_back(LEVEL1_PALETTE_EFFECT_SPEED);
            }
            EXIT_PALETTE_ENTRY_FLASH_REQUEST => {
                self.begin_palette_fade_back(EXIT_PALETTE_EFFECT_ENTRY_FLASH);
            }
            EXIT_PALETTE_FADE_BACK => {
                if self.exit_palette_fade_started {
                    self.exit_palette_step = self.exit_palette_step.saturating_add(1).min(4);
                } else {
                    self.exit_palette_fade_started = true;
                }
                self.exit_palette_effect = self.exit_palette_fade_effect;
                self.exit_palette_weight = 4_u8.saturating_sub(self.exit_palette_step);
                self.exit_palette_denominator = 4;
                if self.exit_palette_step == 4 {
                    self.exit_palette_submode = EXIT_PALETTE_NONE;
                    self.exit_palette_effect = EXIT_PALETTE_EFFECT_NONE;
                    self.exit_palette_weight = 0;
                    self.exit_palette_denominator = 0;
                    self.exit_palette_fade_started = false;
                    self.exit_palette_fade_effect = EXIT_PALETTE_EFFECT_NONE;
                }
            }
            LEVEL1_PALETTE_OBJECTIVE_REQUEST => {
                self.exit_palette_submode = LEVEL1_PALETTE_OBJECTIVE_FADE;
                self.exit_palette_effect = LEVEL1_PALETTE_EFFECT_OBJECTIVE;
                self.exit_palette_weight = 16;
                self.exit_palette_denominator = 16;
                self.exit_palette_step = 0;
                self.exit_palette_fade_started = false;
                self.exit_palette_fade_effect = LEVEL1_PALETTE_EFFECT_OBJECTIVE;
            }
            LEVEL1_PALETTE_OBJECTIVE_FADE => {
                if self.exit_palette_fade_started {
                    self.exit_palette_step = self.exit_palette_step.saturating_add(1).min(16);
                } else {
                    self.exit_palette_fade_started = true;
                }
                self.exit_palette_effect = LEVEL1_PALETTE_EFFECT_OBJECTIVE;
                self.exit_palette_weight = 16_u8.saturating_sub(self.exit_palette_step);
                self.exit_palette_denominator = 16;
                if self.exit_palette_step == 16 {
                    self.exit_palette_submode = EXIT_PALETTE_NONE;
                    self.exit_palette_effect = EXIT_PALETTE_EFFECT_NONE;
                    self.exit_palette_weight = 0;
                    self.exit_palette_denominator = 0;
                    self.exit_palette_fade_started = false;
                    self.exit_palette_fade_effect = EXIT_PALETTE_EFFECT_NONE;
                }
            }
            EXIT_PALETTE_BLACK_REQUEST => {
                self.exit_palette_submode = EXIT_PALETTE_FADE_TO_BLACK;
                self.exit_palette_effect = EXIT_PALETTE_EFFECT_NONE;
                self.exit_palette_weight = 0;
                self.exit_palette_denominator = 28;
                self.exit_palette_step = 0;
                self.exit_palette_fade_started = false;
            }
            EXIT_PALETTE_FADE_TO_BLACK => {
                if self.exit_palette_fade_started {
                    self.exit_palette_step = self.exit_palette_step.saturating_add(1).min(28);
                } else {
                    self.exit_palette_fade_started = true;
                }
                self.exit_palette_effect = EXIT_PALETTE_EFFECT_BLACK;
                self.exit_palette_weight = self.exit_palette_step;
                self.exit_palette_denominator = 28;
                if self.exit_palette_step == 28 {
                    self.exit_palette_submode = EXIT_PALETTE_NONE;
                    self.exit_palette_fade_started = false;
                }
            }
            _ => {}
        }
    }

    fn begin_palette_fade_back(&mut self, effect: u8) {
        self.exit_palette_submode = EXIT_PALETTE_FADE_BACK;
        self.exit_palette_effect = effect;
        self.exit_palette_weight = 4;
        self.exit_palette_denominator = 4;
        self.exit_palette_step = 0;
        self.exit_palette_fade_started = false;
        self.exit_palette_fade_effect = effect;
    }

    /// Materialize the exact effective VGA palette for the current Level 1
    /// palette state. Input and output are RGB6 bytes; callers may expand each
    /// channel by four for SDL/Unity RGB8 upload.
    #[must_use]
    pub fn effective_palette_rgb6(&self, original: &[u8; 768]) -> [u8; 768] {
        let mut flashed = [0_u8; 768];
        if self.exit_palette_effect == EXIT_PALETTE_EFFECT_NONE {
            return *original;
        }
        for index in 1..256 {
            let offset = index * 3;
            let red = i32::from(original[offset]);
            let green = i32::from(original[offset + 1]);
            let blue = i32::from(original[offset + 2]);
            let (effect_red, effect_green, effect_blue) = match self.exit_palette_effect {
                EXIT_PALETTE_EFFECT_ENTRY_FLASH => (
                    (blue + 48).clamp(0, 63),
                    (green + 32).clamp(0, 63),
                    (blue + 32).clamp(0, 63),
                ),
                EXIT_PALETTE_EFFECT_BLACK => (0, 0, 0),
                LEVEL1_PALETTE_EFFECT_HIT => ((red + 40).clamp(0, 63), green, blue),
                LEVEL1_PALETTE_EFFECT_SPEED => {
                    let offset = (-56_i32 * i32::from(self.exit_palette_count)) >> 8;
                    (
                        (red + offset).clamp(0, 63),
                        green,
                        (blue + offset).clamp(0, 63),
                    )
                }
                LEVEL1_PALETTE_EFFECT_OBJECTIVE => (
                    (red + 48).clamp(0, 63),
                    (green + 48).clamp(0, 63),
                    (blue + 48).clamp(0, 63),
                ),
                _ => (red, green, blue),
            };
            let denominator = i32::from(self.exit_palette_denominator.max(1));
            let weight = i32::from(self.exit_palette_weight);
            flashed[offset] = u8::try_from(red + (effect_red - red) * weight / denominator)
                .expect("red RGB6 channel");
            flashed[offset + 1] =
                u8::try_from(green + (effect_green - green) * weight / denominator)
                    .expect("green RGB6 channel");
            flashed[offset + 2] = u8::try_from(blue + (effect_blue - blue) * weight / denominator)
                .expect("blue RGB6 channel");
        }
        flashed
    }

    #[must_use]
    pub const fn palette_count(&self) -> i16 {
        self.exit_palette_count
    }

    #[must_use]
    pub const fn exit_sequence_owns_player_action(&self) -> bool {
        self.exit_sequence_active || self.exit_sequence_complete
    }

    /// Dispatch remc2 player action 12 at the player's source-arena slot.
    /// Palette advancement is deliberately separate because the source runs
    /// it once per rendered frame, before any entity update.
    pub fn advance_exit_sequence<W: FlightWorld>(
        &mut self,
        world: &W,
        player: &mut FlightState,
        context: &mut FlightContext,
        camera_fov: &mut i16,
    ) {
        if !self.exit_sequence_active || self.exit_sequence_complete {
            return;
        }
        self.exit_last_movement_resolution = EXIT_MOVEMENT_NOT_ATTEMPTED;
        loop {
            match self.exit_sequence_phase {
                0 => {
                    self.events.push(Level1MissionEvent::MissionSound {
                        sound_id: EXIT_SWITCH_SOUND,
                        source_entity_index: player.entity_index,
                        position: player.position,
                        random_state: self.global_rng,
                    });
                    self.exit_palette_submode = EXIT_PALETTE_ENTRY_FLASH_REQUEST;
                    self.exit_player_status_flags |= 0x20;
                    player.immobilize_counter = 0;
                    player.speed_modifier = 0;
                    player.target_speed = 0;
                    player.recoil_speed = 0;
                    player.strafe_speed = 0;
                    if let Some(holder) = context.speed_spell_holder.as_mut() {
                        holder.active_value = 0;
                    }
                    if let Some(actor) = self.actors.iter_mut().find(|actor| {
                        actor.kind == Level1ActorKind::SpeedSpellJar && actor.collected
                    }) {
                        actor.state_value = 0;
                    }
                    self.exit_target_valid = self.actors.iter().any(|actor| {
                        actor.source_entity_index == self.exit_target_source_entity_index
                            && actor.active
                            && actor.class == EXIT_TARGET_CLASS
                            && matches!(
                                actor.model,
                                EXIT_TARGET_MODEL_PRIMARY | EXIT_TARGET_MODEL_ALTERNATE
                            )
                    });
                    if !self.exit_target_valid {
                        self.exit_target_source_entity_index = 0;
                    }
                    self.exit_sequence_phase = 1;
                }
                1 => {
                    let old_speed = player.actual_speed;
                    if !self.exit_try_move(world, player, player.yaw, old_speed) {
                        player.actual_speed = 0;
                    }
                    if old_speed.unsigned_abs() <= 4 {
                        player.actual_speed = 0;
                    } else if old_speed <= 0 {
                        player.actual_speed = player.actual_speed.wrapping_add(4);
                    } else {
                        player.actual_speed = player.actual_speed.wrapping_sub(4);
                    }
                    if player.actual_speed == 0 {
                        self.exit_sequence_phase = if self.exit_target_source_entity_index == 0 {
                            4
                        } else {
                            3
                        };
                    }
                }
                3 => {
                    let Some(target) = self.exit_target_position() else {
                        self.exit_target_source_entity_index = 0;
                        self.exit_target_valid = false;
                        self.exit_sequence_phase = 4;
                        continue;
                    };
                    let target_yaw = i16::try_from(heading_for_positions(player.position, target))
                        .expect("11-bit exit heading");
                    if angle_distance(i32::from(player.yaw), i32::from(target_yaw)) <= 11 {
                        player.yaw = target_yaw;
                        self.exit_sequence_phase = 4;
                    } else {
                        player.yaw = player
                            .yaw
                            .wrapping_add(turn_toward(player.yaw, target_yaw, 11))
                            & 0x07ff;
                    }
                }
                4 => {
                    self.exit_sequence_timer = 12;
                    self.exit_sequence_phase = 5;
                    self.exit_end_sequence_flag = true;
                    continue;
                }
                5 => {
                    self.exit_sequence_timer = self.exit_sequence_timer.wrapping_sub(1);
                    if self.exit_sequence_timer > 4 {
                        *camera_fov = camera_fov.wrapping_sub(1);
                    }
                    if self.exit_sequence_timer == 0 {
                        self.exit_sequence_phase = if self.exit_target_source_entity_index == 0 {
                            8
                        } else {
                            6
                        };
                    }
                }
                6 => {
                    self.exit_sequence_timer = 512;
                    player.actual_speed = 100;
                    self.exit_sequence_phase = 7;
                    self.exit_blur_mode = 2;
                    self.events.push(Level1MissionEvent::MissionSound {
                        sound_id: EXIT_ACCELERATION_SOUND,
                        source_entity_index: player.entity_index,
                        position: player.position,
                        random_state: self.global_rng,
                    });
                    continue;
                }
                7 => {
                    self.exit_sequence_timer = self.exit_sequence_timer.wrapping_sub(1);
                    if self.exit_sequence_timer == 0 {
                        self.exit_sequence_phase = 10;
                    } else if let Some(target) = self.exit_target_position() {
                        player.yaw = i16::try_from(heading_for_positions(player.position, target))
                            .expect("11-bit exit heading");
                        let speed = player.actual_speed;
                        player.actual_speed = exit_accelerated_speed(player.actual_speed);
                        if !self.exit_try_move(world, player, player.yaw, speed)
                            || wrapping_xy_distance(player.position, target)
                                < LEVEL1_EXIT_TARGET_DISTANCE
                        {
                            self.exit_sequence_phase = 10;
                        }
                    } else {
                        self.exit_sequence_phase = 10;
                    }
                }
                8 => {
                    self.exit_sequence_timer = 128;
                    player.actual_speed = 100;
                    self.exit_sequence_phase = 9;
                    self.exit_blur_mode = 2;
                    self.events.push(Level1MissionEvent::MissionSound {
                        sound_id: EXIT_ACCELERATION_SOUND,
                        source_entity_index: player.entity_index,
                        position: player.position,
                        random_state: self.global_rng,
                    });
                    continue;
                }
                9 => {
                    self.exit_sequence_timer = self.exit_sequence_timer.wrapping_sub(1);
                    if self.exit_sequence_timer == 0 {
                        self.exit_sequence_phase = 10;
                    } else {
                        let speed = player.actual_speed;
                        player.actual_speed = exit_accelerated_speed(player.actual_speed);
                        if !self.exit_try_move(world, player, player.yaw, speed) {
                            self.exit_sequence_phase = 10;
                        }
                    }
                }
                10 => {
                    self.exit_sequence_timer = 32;
                    self.exit_sequence_phase = 11;
                    self.exit_palette_submode = EXIT_PALETTE_BLACK_REQUEST;
                    continue;
                }
                11 => {
                    self.exit_sequence_timer = self.exit_sequence_timer.wrapping_sub(1);
                    if self.exit_sequence_timer != 0
                        && self.exit_palette_submode != EXIT_PALETTE_NONE
                    {
                        let _ = self.exit_try_move(world, player, player.yaw, 2);
                    } else {
                        self.exit_sequence_phase = 12;
                    }
                }
                12 => {
                    self.exit_result_code = 10;
                    self.exit_player_status_flags = 10;
                    self.exit_display_blank = true;
                    self.exit_blur_mode = 0;
                    self.exit_sequence_active = false;
                    self.exit_sequence_complete = true;
                    self.events.push(Level1MissionEvent::ExitSequenceCompleted {
                        result_code: self.exit_result_code,
                    });
                }
                _ => {}
            }
            break;
        }

        let ground = world.ground_height(player.position);
        if i32::from(player.position.z) <= i32::from(ground) + 256 {
            if player.position.z >= ground {
                player.position.z = ground.wrapping_add(128);
            } else {
                player.position.z = player.position.z.wrapping_add(128);
            }
        } else {
            player.position.z = player.position.z.wrapping_sub(128);
        }
        player.roll_accumulator = exit_relax_axis(player.roll_accumulator, 7, 3);
        player.pitch_accumulator = exit_relax_axis(player.pitch_accumulator, 3, 2);
    }

    fn exit_target_position(&self) -> Option<Position> {
        (self.exit_target_source_entity_index != 0)
            .then(|| {
                self.actors
                    .iter()
                    .find(|actor| actor.source_entity_index == self.exit_target_source_entity_index)
            })
            .flatten()
            .map(|actor| actor.position)
    }

    fn exit_try_move<W: FlightWorld>(
        &mut self,
        world: &W,
        player: &mut FlightState,
        yaw: i16,
        speed: i16,
    ) -> bool {
        let from = player.position;
        let mut proposed = from;
        move_entity(&mut proposed, yaw, 0, speed);
        match resolve_legacy_movement(player, from, proposed, FlightProfile::OUTDOOR, world) {
            MovementResolution::Accept(position) => {
                player.position = position;
                self.exit_last_movement_resolution = EXIT_MOVEMENT_ACCEPTED;
                true
            }
            MovementResolution::Redirect(position) => {
                player.position = position;
                self.exit_last_movement_resolution = EXIT_MOVEMENT_REDIRECTED;
                true
            }
            MovementResolution::Reject | MovementResolution::RejectAndStop => {
                self.exit_last_movement_resolution = EXIT_MOVEMENT_REJECTED;
                false
            }
        }
    }

    #[must_use]
    pub fn secured_mana_percent(&self) -> i32 {
        if !self.castle_exists || self.global_mana_denominator <= 0 {
            return 0;
        }
        100 * self.secured_mana_component.saturating_add(self.castle_mana)
            / self.global_mana_denominator
    }

    /// Source `PlayerHitFrameTime_406`, consumed by the HUD hit-panel draw.
    #[must_use]
    pub const fn player_hit_frame_ticks(&self) -> u8 {
        self.player_hit_frame_ticks
    }

    /// Source owner-side castle hit feedback consumed by the top-bar draw.
    #[must_use]
    pub const fn castle_hit_frame_ticks(&self) -> u8 {
        self.castle_owner_hit_feedback
    }

    /// Apply the state mutation performed by the preceding qualified HUD draw.
    ///
    /// `DrawTopStatusBar_2D710` decrements hit counters only on animated
    /// `colorIndex[1]` phases. Calling this at the next source turn preserves
    /// the just-rendered value while keeping simulation state independent of
    /// Unity frame rate.
    pub fn finish_hud_draw(&mut self, completed_turn: u64) {
        if completed_turn == 0 || completed_turn & 1 == 0 {
            return;
        }
        self.player_hit_frame_ticks = self.player_hit_frame_ticks.saturating_sub(1);
        if self.castle_exists {
            self.castle_owner_hit_feedback = self.castle_owner_hit_feedback.saturating_sub(1);
        }
    }

    fn update_spell_pickup_slot(&mut self, player: &FlightState, source_slot: u16) {
        for actor in &mut self.actors {
            if actor.source_entity_index != source_slot {
                continue;
            }
            if !actor.active
                || !matches!(
                    actor.kind,
                    Level1ActorKind::CastleSpellJar | Level1ActorKind::SpeedSpellJar
                )
            {
                continue;
            }
            if actor.state_timer > 0 {
                actor.state_timer -= 1;
            }
            let pickup_tick = actor.phase & 3 == 0;
            actor.phase = actor.phase.wrapping_add(1);
            if actor.collected
                || !pickup_tick
                || !overlaps_source_entities(
                    actor.position,
                    actor.collision_half_width,
                    actor.collision_vertical_offset,
                    actor.collision_half_height,
                    player.position,
                    PLAYER_COLLISION_HALF_WIDTH,
                    PLAYER_COLLISION_VERTICAL_OFFSET,
                    PLAYER_COLLISION_HALF_HEIGHT,
                )
            {
                continue;
            }
            let family = match actor.kind {
                Level1ActorKind::CastleSpellJar => {
                    self.has_castle_spell = true;
                    actor.action = 6;
                    2
                }
                Level1ActorKind::SpeedSpellJar => {
                    self.has_speed_spell = true;
                    actor.action = 9;
                    SPEED_SPELL_FAMILY
                }
                Level1ActorKind::Archer
                | Level1ActorKind::Firefly
                | Level1ActorKind::ExitTarget
                | Level1ActorKind::ExitSwitch
                | Level1ActorKind::MultipartHead
                | Level1ActorKind::ExperienceScroll => continue,
            };
            actor.collected = true;
            actor.visible = false;
            // remc2 preserves the class-15 holder, attaches it to the player
            // and returns it to the family's active/charging action.
            actor.state_timer = 64;
            actor.state_value = 0;
            self.events.push(Level1MissionEvent::SpellCollected {
                family,
                authored_index: actor.authored_index,
            });
            self.events.push(Level1MissionEvent::MissionSound {
                sound_id: SPEED_PICKUP_SOUND,
                source_entity_index: player.entity_index,
                position: player.position,
                random_state: self.global_rng,
            });
        }
    }

    fn update_experience_scroll_slot<W: FlightWorld>(
        &mut self,
        world: &W,
        player: &FlightState,
        source_slot: u16,
    ) {
        let Some(index) = self.actors.iter().position(|actor| {
            actor.source_entity_index == source_slot
                && actor.kind == Level1ActorKind::ExperienceScroll
                && actor.active
                && !actor.collected
        }) else {
            return;
        };
        let mut position = self.actors[index].position;
        position.z = world.ground_height(position);
        self.actors[index].position = position;
        let actor = self.actors[index];
        if !overlaps_source_entities(
            actor.position,
            actor.collision_half_width,
            actor.collision_vertical_offset,
            actor.collision_half_height,
            player.position,
            PLAYER_COLLISION_HALF_WIDTH,
            PLAYER_COLLISION_VERTICAL_OFFSET,
            PLAYER_COLLISION_HALF_HEIGHT,
        ) {
            return;
        }
        self.actors[index].collected = true;
        self.actors[index].active = false;
        self.actors[index].visible = false;
        self.events.push(Level1MissionEvent::MissionSound {
            sound_id: EXPERIENCE_SCROLL_SOUND,
            // `GetScroll_69DB0` routes the local player's colour index through
            // `PrepareEventSound`; Level 1 player zero therefore uses the
            // sentinel/local source slot while retaining the player's
            // position for spatialisation.
            source_entity_index: 0,
            position: player.position,
            random_state: self.global_rng,
        });
        self.mark_allocation_pending_delete(actor.authored_index);
    }

    fn step_actor_slot<W: FlightWorld>(
        &mut self,
        world: &W,
        player: &FlightState,
        source_slot: u16,
    ) {
        let mut spawned_arrows = Vec::new();
        let mut spawned_firefly_fireballs = Vec::new();
        let mut player_contact_damage = None;
        let mut killed_allocations = Vec::new();
        let mut multipart_pre_kill_heads = Vec::new();
        let mut actor_views = self.actors.clone();
        let mut dispatch_order = (0..self.actors.len()).collect::<Vec<_>>();
        dispatch_order.retain(|index| self.actors[*index].source_entity_index == source_slot);
        dispatch_order.sort_by_key(|index| {
            let actor = self.actors[*index];
            if actor.active && actor.source_entity_index != 0 {
                actor.source_entity_index
            } else {
                u16::MAX
            }
        });
        for actor_index in dispatch_order {
            if self.actors[actor_index].action == 35
                && self.actors[actor_index].linked_entity_index == 0
            {
                let actor = &mut self.actors[actor_index];
                actor.action = ARCHER_ACTION;
                actor.phase = actor.phase.wrapping_add(1);
                actor_views[actor_index] = *actor;
                continue;
            }

            let linked_damage = {
                let actor = self.actors[actor_index];
                (actor.action == 35 && actor.linked_entity_index != 0 && actor.pending_damage != 0)
                    .then_some((
                        actor.linked_entity_index,
                        actor.pending_attacker,
                        actor.class,
                        actor.model,
                    ))
            };
            if let Some((peer_slot, attacker, class, model)) = linked_damage
                && let Some(peer_index) = self.actors.iter().position(|candidate| {
                    candidate.source_entity_index == peer_slot
                        && candidate.active
                        && candidate.life >= 0
                        && candidate.class == class
                        && candidate.model == model
                })
            {
                let peer = &mut self.actors[peer_index];
                peer.target_entity_index = attacker;
                peer.linked_entity_index = 0;
                peer.action = 34;
                actor_views[peer_index] = *peer;
            }

            if self.actors[actor_index].kind == Level1ActorKind::MultipartHead
                && let Some(child) = self
                    .multipart_segments
                    .iter()
                    .filter(|segment| {
                        segment.head_authored_index == self.actors[actor_index].authored_index
                            && segment.active
                    })
                    .min_by_key(|segment| segment.life)
                    .copied()
                && child.life < self.actors[actor_index].life
            {
                self.actors[actor_index].life = child.life;
                self.actors[actor_index].pending_attacker = child.pending_attacker;
                if child.life >= 0 && child.pending_attacker == player.entity_index {
                    self.actors[actor_index].target_entity_index = player.entity_index;
                    self.actors[actor_index].linked_entity_index = 0;
                    self.actors[actor_index].action = MULTIPART_ATTACK_ACTION;
                }
            }

            let actor = &mut self.actors[actor_index];
            if !actor.active
                || !matches!(
                    actor.kind,
                    Level1ActorKind::Archer
                        | Level1ActorKind::Firefly
                        | Level1ActorKind::MultipartHead
                )
            {
                continue;
            }

            if actor.pending_damage != 0 {
                let damage = actor.pending_damage;
                actor.pending_damage = 0;
                actor.life = actor.life.saturating_sub(damage);
                self.events.push(Level1MissionEvent::ActorDamaged {
                    authored_index: actor.authored_index,
                    damage,
                    life: actor.life,
                });
                if actor.life < 0 {
                    actor.linked_entity_index = 0;
                    actor.action = match actor.kind {
                        Level1ActorKind::Archer => 36,
                        Level1ActorKind::Firefly => FIREFLY_PRE_KILL_ACTION,
                        Level1ActorKind::CastleSpellJar | Level1ActorKind::SpeedSpellJar => {
                            unreachable!("spell jars were filtered above")
                        }
                        Level1ActorKind::ExitTarget | Level1ActorKind::ExitSwitch => {
                            unreachable!("exit actors were filtered above")
                        }
                        Level1ActorKind::MultipartHead => MULTIPART_PRE_KILL_ACTION,
                        Level1ActorKind::ExperienceScroll => {
                            unreachable!("experience scroll was filtered above")
                        }
                    };
                } else if actor.kind == Level1ActorKind::Archer
                    && actor.pending_attacker == player.entity_index
                {
                    actor.target_entity_index = player.entity_index;
                    actor.linked_entity_index = 0;
                    actor.action = 34;
                    enter_archer_attack_pose(actor);
                } else if actor.kind == Level1ActorKind::Firefly
                    && actor.pending_attacker != 0
                    && actor.pending_attacker == player.entity_index
                {
                    actor.target_entity_index = player.entity_index;
                    actor.linked_entity_index = 0;
                    actor.stage_var_2 = 10;
                    actor.action = FIREFLY_ATTACK_ACTION;
                    // HitFirebug_25610 sends every surviving hostile hit
                    // through its v1 <= 1 branch and forces byte_0x46 to the
                    // charge substate, regardless of the prior action.
                    actor.attack_substate = 7;
                }
                actor.pending_attacker = 0;
                // The action that consumed the mailbox completes here. A
                // newly selected attack or pre-kill action is dispatched on
                // this source slot's following update.
                actor.phase = actor.phase.wrapping_add(1);
                actor_views[actor_index] = *actor;
                continue;
            }

            let pre_kill_action = match actor.kind {
                Level1ActorKind::Archer => 36,
                Level1ActorKind::Firefly => FIREFLY_PRE_KILL_ACTION,
                Level1ActorKind::CastleSpellJar | Level1ActorKind::SpeedSpellJar => {
                    unreachable!("spell jars were filtered above")
                }
                Level1ActorKind::ExitTarget | Level1ActorKind::ExitSwitch => {
                    unreachable!("exit actors were filtered above")
                }
                Level1ActorKind::MultipartHead => MULTIPART_PRE_KILL_ACTION,
                Level1ActorKind::ExperienceScroll => {
                    unreachable!("experience scroll was filtered above")
                }
            };
            let final_kill_action = pre_kill_action + 1;
            if actor.life < 0 && actor.action < pre_kill_action {
                actor.action = pre_kill_action;
            } else if actor.action == pre_kill_action {
                actor.action = final_kill_action;
                if actor.kind == Level1ActorKind::MultipartHead {
                    multipart_pre_kill_heads.push(actor.authored_index);
                }
            } else if actor.action == final_kill_action && actor.phase & 7 == 0 {
                actor.active = false;
                // `TransformEntityToManaSphere_36BA0(false)` advances the
                // dying entity seed before it attempts to allocate a sphere.
                // The advance survives allocation exhaustion.
                actor.entity_rng = next_rng16(actor.entity_rng);
                killed_allocations.push((
                    actor.authored_index,
                    actor.source_entity_index,
                    actor.kind,
                    actor.position,
                    actor.yaw,
                    actor.entity_rng,
                    actor.owner_id,
                ));
                self.events.push(Level1MissionEvent::ActorKilled {
                    authored_index: actor.authored_index,
                    kind: actor.kind,
                });
            } else if actor.kind == Level1ActorKind::Archer {
                match actor.action {
                    33 => step_archer_roam(actor, world, player, &actor_views),
                    34 => {
                        if let Some(mut arrow) =
                            step_archer_attack(actor, world, player, self.next_effect_id)
                        {
                            if let Some(source_slot) = self
                                .source_pool
                                .allocate(SourceOccupant::ArcherArrow(arrow.id))
                            {
                                bind_dynamic_source_record(
                                    &mut self.source_slots,
                                    source_slot,
                                    SourceOccupant::ArcherArrow(arrow.id),
                                    self.global_rng,
                                );
                                self.next_effect_id = self.next_effect_id.wrapping_add(1).max(1);
                                arrow.source_entity_index = source_slot;
                                self.events.push(Level1MissionEvent::ArcherArrowSpawned {
                                    projectile_id: arrow.id,
                                    archer_authored_index: actor.authored_index,
                                });
                                spawned_arrows.push(arrow);
                            }
                        }
                    }
                    35 => step_archer_linked(actor, world, &actor_views),
                    // Actions 36/37 are handled above.
                    36 | 37 => {}
                    _ => {}
                }
            } else if actor.kind == Level1ActorKind::Firefly {
                match actor.action {
                    FIREFLY_STAGE_ACTION => {
                        step_firefly_stage_patrol(actor, world, &actor_views);
                    }
                    FIREFLY_ROAM_ACTION => {
                        step_firefly_roam(actor, world, player, &actor_views);
                    }
                    FIREFLY_ATTACK_ACTION => {
                        let (projectile, charge_sound, contact_damage) = step_firefly_attack(
                            actor,
                            world,
                            player,
                            &actor_views,
                            self.next_effect_id,
                        );
                        if let Some(mut projectile) = projectile {
                            if let Some(source_entity_index) = self
                                .source_pool
                                .allocate(SourceOccupant::FireflyFireball(projectile.id))
                            {
                                bind_dynamic_source_record(
                                    &mut self.source_slots,
                                    source_entity_index,
                                    SourceOccupant::FireflyFireball(projectile.id),
                                    self.global_rng,
                                );
                                self.next_effect_id = self.next_effect_id.wrapping_add(1).max(1);
                                projectile.source_entity_index = source_entity_index;
                                self.events
                                    .push(Level1MissionEvent::FireflyFireballSpawned {
                                        projectile_id: projectile.id,
                                        firefly_authored_index: actor.authored_index,
                                    });
                                spawned_firefly_fireballs.push(projectile);
                            }
                        }
                        if let Some(sound_id) = charge_sound {
                            self.events.push(Level1MissionEvent::FireflyChargeSound {
                                firefly_authored_index: actor.authored_index,
                                sound_id,
                                random_state: self.global_rng,
                            });
                        }
                        if contact_damage != 0 {
                            player_contact_damage =
                                Some((contact_damage, actor.source_entity_index));
                        }
                    }
                    FIREFLY_LINKED_ACTION => {
                        step_firefly_linked(actor, world, &actor_views);
                    }
                    FIREFLY_PRE_KILL_ACTION | FIREFLY_KILL_ACTION => {}
                    _ => {}
                }
            } else if actor.kind == Level1ActorKind::MultipartHead {
                match actor.action {
                    24 => {
                        move_multipart_head(actor, world);
                    }
                    MULTIPART_ROAM_ACTION => {
                        step_multipart_roam(actor, world, player, &actor_views);
                    }
                    MULTIPART_ATTACK_ACTION => {
                        if let Some(mut projectile) = step_multipart_attack(
                            actor,
                            world,
                            player,
                            &actor_views,
                            self.next_effect_id,
                        ) {
                            if let Some(source_entity_index) = self
                                .source_pool
                                .allocate(SourceOccupant::FireflyFireball(projectile.id))
                            {
                                bind_dynamic_source_record(
                                    &mut self.source_slots,
                                    source_entity_index,
                                    SourceOccupant::FireflyFireball(projectile.id),
                                    self.global_rng,
                                );
                                self.next_effect_id = self.next_effect_id.wrapping_add(1).max(1);
                                projectile.source_entity_index = source_entity_index;
                                self.events
                                    .push(Level1MissionEvent::FireflyFireballSpawned {
                                        projectile_id: projectile.id,
                                        firefly_authored_index: actor.authored_index,
                                    });
                                self.events.push(Level1MissionEvent::MissionSound {
                                    sound_id: MULTIPART_ATTACK_SOUND,
                                    source_entity_index: actor.source_entity_index,
                                    position: actor.position,
                                    random_state: self.global_rng,
                                });
                                spawned_firefly_fireballs.push(projectile);
                            }
                        }
                    }
                    MULTIPART_LINKED_ACTION => {
                        step_multipart_linked(actor, world, &actor_views);
                    }
                    MULTIPART_UNSUPPORTED_ACTION => {
                        // remc2 retains an address-table hole for model-3
                        // action 30. Keep it inert and observable.
                    }
                    MULTIPART_STAGE_ACTION => {
                        actor.action = MULTIPART_ROAM_ACTION;
                    }
                    MULTIPART_PRE_KILL_ACTION | MULTIPART_KILL_ACTION => {}
                    _ => {}
                }
            }
            actor.phase = actor.phase.wrapping_add(1);
            actor_views[actor_index] = *actor;
        }
        self.projectiles.extend(
            spawned_arrows
                .into_iter()
                .map(Level1Projectile::ArcherArrow),
        );
        self.projectiles.extend(
            spawned_firefly_fireballs
                .into_iter()
                .map(Level1Projectile::FireflyFireball),
        );
        for head_authored_index in multipart_pre_kill_heads {
            for segment in self.multipart_segments.iter_mut().filter(|segment| {
                segment.head_authored_index == head_authored_index && segment.active
            }) {
                segment.action = MULTIPART_KILL_ACTION;
                self.source_slots[usize::from(segment.source_entity_index)].action =
                    MULTIPART_KILL_ACTION;
            }
        }
        if let Some((damage, attacker)) = player_contact_damage {
            self.submit_player_contact_damage(damage, attacker);
        }
        for (authored_index, source_entity_index, kind, position, yaw, rng, owner_id) in
            killed_allocations
        {
            self.source_slots[usize::from(source_entity_index)].entity_rng = rng;
            let mana = match kind {
                Level1ActorKind::Archer => ARCHER_LIFE / 2,
                Level1ActorKind::Firefly => FIREFLY_MANA,
                Level1ActorKind::MultipartHead => MULTIPART_HEAD_MANA,
                Level1ActorKind::CastleSpellJar | Level1ActorKind::SpeedSpellJar => 0,
                Level1ActorKind::ExitTarget
                | Level1ActorKind::ExitSwitch
                | Level1ActorKind::ExperienceScroll => 0,
            };
            // remc2 GetManaSphereColorIndexFromEntityId_369F0 treats the
            // source field as an entity relation, not as a player/color
            // ordinal: only a live class-3 entity owns the resulting mana.
            // Level 1 actors commonly carry relations such as slot 3, whose
            // reset entity is class 10/model 45; those deaths produce neutral
            // mana even though the relation itself is non-zero.
            let owner_entity_index = resolved_mana_owner_entity_index(&self.source_slots, owner_id);
            self.spawn_mana_sphere_from_death(
                world,
                source_entity_index,
                position,
                yaw,
                rng,
                mana,
                owner_entity_index,
            );
            self.spawn_death_burst_from_death(source_entity_index, position);
            self.mark_allocation_pending_delete(authored_index);
        }
    }

    fn step_multipart_segment_slot<W: FlightWorld>(&mut self, world: &W, source_slot: u16) {
        let Some(index) = self
            .multipart_segments
            .iter()
            .position(|segment| segment.source_entity_index == source_slot)
        else {
            return;
        };
        let segment = self.multipart_segments[index];
        if !segment.active {
            return;
        }

        if segment.action == MULTIPART_KILL_ACTION {
            if segment.phase & 7 == 0 {
                let mut dead = segment;
                dead.active = false;
                dead.visible = false;
                dead.phase = dead.phase.wrapping_add(1);
                self.multipart_segments[index] = dead;
                let rng = next_rng16(source_slot.wrapping_add(self.global_rng as u16));
                let owner_entity_index =
                    resolved_mana_owner_entity_index(&self.source_slots, dead.owner_id);
                self.spawn_mana_sphere_from_death(
                    world,
                    source_slot,
                    dead.position,
                    dead.yaw,
                    rng,
                    MULTIPART_SEGMENT_MANA,
                    owner_entity_index,
                );
                self.spawn_death_burst_from_death(source_slot, dead.position);
                self.source_pool.disable_slot(source_slot);
                self.source_slots[usize::from(source_slot)].control_flags |= SOURCE_DISABLED_FLAG;
            } else {
                self.multipart_segments[index].phase =
                    self.multipart_segments[index].phase.wrapping_add(1);
            }
            return;
        }
        if segment.action != MULTIPART_SEGMENT_ACTION {
            return;
        }

        let predecessor = self
            .actors
            .iter()
            .find(|actor| {
                actor.source_entity_index == segment.predecessor_source_entity_index
                    && actor.class == MISSION_ACTOR_CLASS
                    && actor.active
            })
            .map(|actor| (actor.position, actor.class))
            .or_else(|| {
                self.multipart_segments
                    .iter()
                    .find(|candidate| {
                        candidate.source_entity_index == segment.predecessor_source_entity_index
                            && candidate.class == MISSION_ACTOR_CLASS
                            && candidate.active
                    })
                    .map(|candidate| (candidate.position, candidate.class))
            });
        let Some((predecessor_position, predecessor_class)) = predecessor else {
            self.multipart_segments[index].active = false;
            self.multipart_segments[index].visible = false;
            self.source_pool.disable_slot(source_slot);
            self.source_slots[usize::from(source_slot)].control_flags |= SOURCE_DISABLED_FLAG;
            return;
        };
        if predecessor_class != MISSION_ACTOR_CLASS {
            self.multipart_segments[index].active = false;
            self.multipart_segments[index].visible = false;
            self.source_pool.disable_slot(source_slot);
            self.source_slots[usize::from(source_slot)].control_flags |= SOURCE_DISABLED_FLAG;
            return;
        }

        let current_position = self.multipart_segments[index].position;
        let yaw = i16::try_from(heading_for_positions(
            current_position,
            predecessor_position,
        ))
        .expect("multipart segment heading is 11-bit");
        let horizontal = legacy_sqrt(squared_wrapping_xy_distance(
            current_position,
            predecessor_position,
        ));
        let pitch = pitch_for_delta(
            i32::try_from(horizontal).expect("multipart segment distance fits i32"),
            i32::from(predecessor_position.z.wrapping_sub(current_position.z)),
        );
        let mut trailing_position = predecessor_position;
        move_entity(
            &mut trailing_position,
            yaw,
            pitch,
            -self.multipart_segments[index].trailing_distance,
        );
        let body = &mut self.multipart_segments[index];
        body.yaw = yaw;
        body.pitch = pitch;
        body.position = trailing_position;
        if body.pending_damage != 0 {
            body.life = body.life.saturating_sub(body.pending_damage);
            body.pending_damage = 0;
        } else {
            body.pending_attacker = 0;
        }
        body.phase = body.phase.wrapping_add(1);
    }

    #[cfg(test)]
    fn step_projectiles<W: FlightWorld>(&mut self, world: &W, player: &FlightState) {
        let slots = self
            .projectiles
            .iter()
            .filter(|projectile| projectile.active())
            .map(|projectile| projectile.source_entity_index())
            .collect::<Vec<_>>();
        for slot in slots {
            self.step_projectile_slot(world, player, slot);
        }
    }

    fn step_projectile_slot<W: FlightWorld>(
        &mut self,
        world: &W,
        player: &FlightState,
        source_slot: u16,
    ) {
        let actors = &self.actors;
        let multipart_segments = &self.multipart_segments;
        let external_collision_targets = &self.external_collision_targets;
        let impacts = &mut self.impacts;
        let mana_spheres = &mut self.mana_spheres;
        let events = &mut self.events;
        let source_pool = &mut self.source_pool;
        let source_slots = &mut self.source_slots;
        let mut next_effect_id = self.next_effect_id;
        let mut player_damage = Vec::new();
        let mut castle_root_request = None;
        let mut castle_rejected = false;
        for projectile in &mut self.projectiles {
            if projectile.source_entity_index() != source_slot {
                continue;
            }
            match projectile {
                Level1Projectile::PlayerFireball(projectile) => {
                    if !projectile.active {
                        continue;
                    }
                    let previous = projectile.position;
                    move_entity(
                        &mut projectile.position,
                        projectile.yaw,
                        projectile.pitch,
                        projectile.speed,
                    );
                    let actor_hit = actors.iter().find(|actor| {
                        actor.alive()
                            && matches!(
                                actor.kind,
                                Level1ActorKind::Archer
                                    | Level1ActorKind::Firefly
                                    | Level1ActorKind::MultipartHead
                            )
                            && overlaps_projectile(projectile.position, actor)
                    });
                    let segment_hit = multipart_segments.iter().find(|segment| {
                        segment.active
                            && segment.life >= 0
                            && overlaps_source_entities(
                                projectile.position,
                                FIREBALL_COLLISION_HALF_WIDTH,
                                FIREBALL_COLLISION_VERTICAL_OFFSET,
                                FIREBALL_COLLISION_HALF_HEIGHT,
                                segment.position,
                                segment.collision_half_width,
                                0,
                                segment.collision_half_height,
                            )
                    });
                    let external_hit = external_collision_targets.iter().find(|target| {
                        target.active
                            && target.entity_index != projectile.owner_entity_index
                            && overlaps_source_entities(
                                projectile.position,
                                FIREBALL_COLLISION_HALF_WIDTH,
                                FIREBALL_COLLISION_VERTICAL_OFFSET,
                                FIREBALL_COLLISION_HALF_HEIGHT,
                                target.position,
                                target.collision_half_width,
                                target.collision_vertical_offset,
                                target.collision_half_height,
                            )
                    });
                    let hit_position = actor_hit
                        .map(|actor| actor.position)
                        .or_else(|| segment_hit.map(|segment| segment.position))
                        .or_else(|| external_hit.map(|target| target.position));
                    let ground = world.ground_height(projectile.position);
                    let terrain_hit = projectile.position.z < ground;
                    let old_life = projectile.life_ticks;
                    if hit_position.is_none() && !terrain_hit {
                        projectile.life_ticks = projectile.life_ticks.wrapping_sub(1);
                        if old_life != 0 {
                            continue;
                        }
                    }
                    if let Some(position) = hit_position {
                        projectile.position = position;
                    } else if terrain_hit {
                        projectile.position = previous;
                        projectile.position.z = ground;
                        if world.material_mask(projectile.position) == 1 {
                            let splash_id = next_effect_id;
                            if let Some(source_entity_index) =
                                source_pool.allocate(SourceOccupant::WaterSplash(splash_id))
                            {
                                bind_dynamic_source_record(
                                    source_slots,
                                    source_entity_index,
                                    SourceOccupant::WaterSplash(splash_id),
                                    self.global_rng,
                                );
                                next_effect_id = next_effect_id.wrapping_add(1).max(1);
                                impacts.push(FireballImpact {
                                    id: splash_id,
                                    source_entity_index,
                                    source_projectile_id: projectile.id,
                                    owner_entity_index: projectile.owner_entity_index,
                                    external_target_entity_index: 0,
                                    model: WATER_SPLASH_MODEL,
                                    position: projectile.position,
                                    payload: 0,
                                    life_ticks: 8,
                                    entity_rng: source_slots[usize::from(source_entity_index)]
                                        .entity_rng,
                                    vertical_velocity: 0,
                                    animation_frame: 0,
                                    damage_submitted: false,
                                    active: true,
                                });
                            }
                            // Water disables the projectile even when the
                            // model-5 child cannot be allocated.
                            projectile.active = false;
                            continue;
                        }
                    }
                    {
                        let impact_id = next_effect_id;
                        if let Some(source_entity_index) =
                            source_pool.allocate(SourceOccupant::FireballImpact(impact_id))
                        {
                            bind_dynamic_source_record(
                                source_slots,
                                source_entity_index,
                                SourceOccupant::FireballImpact(impact_id),
                                self.global_rng,
                            );
                            next_effect_id = next_effect_id.wrapping_add(1).max(1);
                            impacts.push(FireballImpact {
                                id: impact_id,
                                source_entity_index,
                                source_projectile_id: projectile.id,
                                owner_entity_index: projectile.owner_entity_index,
                                external_target_entity_index: external_hit
                                    .map_or(0, |target| target.entity_index),
                                model: FIREBALL_IMPACT_MODEL,
                                position: projectile.position,
                                payload: projectile.payload,
                                life_ticks: 8,
                                entity_rng: source_slots[usize::from(source_entity_index)]
                                    .entity_rng,
                                vertical_velocity: 0,
                                animation_frame: 0,
                                damage_submitted: false,
                                active: true,
                            });
                            projectile.active = false;
                        }
                    }
                }
                Level1Projectile::ArcherArrow(projectile) => {
                    if !projectile.active {
                        continue;
                    }
                    if !projectile.sound_initialized {
                        projectile.sound_initialized = true;
                        self.global_rng = next_rng32(self.global_rng);
                        events.push(Level1MissionEvent::ArcherArrowSound {
                            projectile_id: projectile.id,
                            sound_id: 33 + (self.global_rng & 1) as u16,
                            source_entity_index: projectile.source_entity_index,
                            position: projectile.position,
                            random_state: self.global_rng,
                        });
                    }

                    let collided_with_player = overlaps_source_entities(
                        projectile.position,
                        ARCHER_ARROW_COLLISION_HALF_WIDTH,
                        ARCHER_ARROW_COLLISION_VERTICAL_OFFSET,
                        ARCHER_ARROW_COLLISION_HALF_HEIGHT,
                        player.position,
                        PLAYER_COLLISION_HALF_WIDTH,
                        PLAYER_COLLISION_VERTICAL_OFFSET,
                        PLAYER_COLLISION_HALF_HEIGHT,
                    );
                    let mut predicted = projectile.position;
                    move_entity(
                        &mut predicted,
                        projectile.yaw,
                        projectile.pitch,
                        projectile.speed,
                    );
                    let terrain_hit = predicted.z < world.ground_height(predicted);
                    let old_life = projectile.life_ticks;
                    projectile.life_ticks = projectile.life_ticks.wrapping_sub(1);
                    if !terrain_hit && old_life != 0 && !collided_with_player {
                        projectile.position = predicted;
                        continue;
                    }
                    if collided_with_player {
                        projectile.position = player.position;
                        player_damage.push((projectile.payload, projectile.owner_entity_index));
                    }
                    projectile.active = false;
                }
                Level1Projectile::FireflyFireball(projectile) => {
                    if !projectile.active {
                        continue;
                    }
                    if projectile.target_entity_index == player.entity_index {
                        let horizontal = legacy_sqrt(squared_wrapping_xy_distance(
                            projectile.position,
                            player.position,
                        ));
                        let desired_pitch = pitch_for_delta(
                            i32::try_from(horizontal).expect("legacy XY distance fits i32"),
                            i32::from(player.position.z.wrapping_sub(projectile.position.z)),
                        );
                        projectile.pitch = projectile.pitch.wrapping_add(turn_toward(
                            projectile.pitch,
                            desired_pitch,
                            FIREFLY_FIREBALL_PITCH_TURN_LIMIT,
                        )) & 0x07ff;
                    }

                    let previous = projectile.position;
                    move_entity(
                        &mut projectile.position,
                        projectile.yaw,
                        projectile.pitch,
                        projectile.speed,
                    );
                    let collided_with_player = overlaps_source_entities(
                        projectile.position,
                        FIREBALL_COLLISION_HALF_WIDTH,
                        FIREBALL_COLLISION_VERTICAL_OFFSET,
                        FIREBALL_COLLISION_HALF_HEIGHT,
                        player.position,
                        PLAYER_COLLISION_HALF_WIDTH,
                        PLAYER_COLLISION_VERTICAL_OFFSET,
                        PLAYER_COLLISION_HALF_HEIGHT,
                    );
                    let actor_hit = actors.iter().find(|actor| {
                        actor.alive()
                            && actor.source_entity_index != projectile.owner_entity_index
                            && overlaps_source_entities(
                                projectile.position,
                                FIREBALL_COLLISION_HALF_WIDTH,
                                FIREBALL_COLLISION_VERTICAL_OFFSET,
                                FIREBALL_COLLISION_HALF_HEIGHT,
                                actor.position,
                                actor.collision_half_width,
                                actor.collision_vertical_offset,
                                actor.collision_half_height,
                            )
                    });
                    let external_hit = external_collision_targets.iter().find(|target| {
                        target.active
                            && target.entity_index != projectile.owner_entity_index
                            && overlaps_source_entities(
                                projectile.position,
                                FIREBALL_COLLISION_HALF_WIDTH,
                                FIREBALL_COLLISION_VERTICAL_OFFSET,
                                FIREBALL_COLLISION_HALF_HEIGHT,
                                target.position,
                                target.collision_half_width,
                                target.collision_vertical_offset,
                                target.collision_half_height,
                            )
                    });
                    let hit_position = collided_with_player
                        .then_some(player.position)
                        .or_else(|| actor_hit.map(|actor| actor.position))
                        .or_else(|| external_hit.map(|target| target.position));
                    let ground = world.ground_height(projectile.position);
                    let terrain_hit = projectile.position.z < ground;
                    let old_life = projectile.life_ticks;
                    if hit_position.is_none() && !terrain_hit {
                        projectile.life_ticks = projectile.life_ticks.wrapping_sub(1);
                        if old_life != 0 {
                            continue;
                        }
                    }
                    if let Some(position) = hit_position {
                        projectile.position = position;
                    } else if terrain_hit {
                        projectile.position = previous;
                        projectile.position.z = ground;
                        if world.material_mask(projectile.position) == 1 {
                            let splash_id = next_effect_id;
                            if let Some(source_entity_index) =
                                source_pool.allocate(SourceOccupant::WaterSplash(splash_id))
                            {
                                bind_dynamic_source_record(
                                    source_slots,
                                    source_entity_index,
                                    SourceOccupant::WaterSplash(splash_id),
                                    self.global_rng,
                                );
                                next_effect_id = next_effect_id.wrapping_add(1).max(1);
                                impacts.push(FireballImpact {
                                    id: splash_id,
                                    source_entity_index,
                                    source_projectile_id: projectile.id,
                                    owner_entity_index: projectile.owner_entity_index,
                                    external_target_entity_index: 0,
                                    model: WATER_SPLASH_MODEL,
                                    position: projectile.position,
                                    payload: 0,
                                    life_ticks: 8,
                                    entity_rng: source_slots[usize::from(source_entity_index)]
                                        .entity_rng,
                                    vertical_velocity: 0,
                                    animation_frame: 0,
                                    damage_submitted: false,
                                    active: true,
                                });
                            }
                            projectile.active = false;
                            continue;
                        }
                    }
                    let impact_id = next_effect_id;
                    if let Some(source_entity_index) =
                        source_pool.allocate(SourceOccupant::FireballImpact(impact_id))
                    {
                        bind_dynamic_source_record(
                            source_slots,
                            source_entity_index,
                            SourceOccupant::FireballImpact(impact_id),
                            self.global_rng,
                        );
                        next_effect_id = next_effect_id.wrapping_add(1).max(1);
                        impacts.push(FireballImpact {
                            id: impact_id,
                            source_entity_index,
                            source_projectile_id: projectile.id,
                            owner_entity_index: projectile.owner_entity_index,
                            external_target_entity_index: external_hit
                                .map_or(0, |target| target.entity_index),
                            model: FIREBALL_IMPACT_MODEL,
                            position: projectile.position,
                            payload: projectile.payload,
                            life_ticks: 8,
                            entity_rng: source_slots[usize::from(source_entity_index)].entity_rng,
                            vertical_velocity: 0,
                            animation_frame: 0,
                            damage_submitted: false,
                            active: true,
                        });
                        projectile.active = false;
                    }
                }
                Level1Projectile::CastleBuild(projectile) => {
                    if !projectile.active {
                        continue;
                    }
                    if !projectile.placement_checked {
                        projectile.placement_checked = true;
                        if self.castle_exists
                            || !castle_placement_valid(
                                world,
                                projectile.position,
                                &self.initial_castle_blockers,
                                &self.initial_building_blockers,
                            )
                        {
                            projectile.active = false;
                            castle_rejected = true;
                            events.push(Level1MissionEvent::CastlePlacementRejected {
                                projectile_id: projectile.id,
                            });
                        }
                        // `sub_66D00` returns immediately after this one-time
                        // validity check; movement begins on the next update.
                        continue;
                    }

                    let horizontal = legacy_sqrt(squared_wrapping_xy_distance(
                        projectile.position,
                        projectile.target,
                    ));
                    let desired_yaw = i16::try_from(heading_for_positions(
                        projectile.position,
                        projectile.target,
                    ))
                    .expect("11-bit Castle heading");
                    let desired_pitch = pitch_for_delta(
                        i32::try_from(horizontal).expect("Castle target distance fits i32"),
                        i32::from(projectile.target.z.wrapping_sub(projectile.position.z)),
                    );
                    projectile.yaw = projectile.yaw.wrapping_add(turn_toward(
                        projectile.yaw,
                        desired_yaw,
                        CASTLE_PROJECTILE_TURN_LIMIT,
                    )) & 0x07ff;
                    projectile.pitch = projectile.pitch.wrapping_add(turn_toward(
                        projectile.pitch,
                        desired_pitch,
                        CASTLE_PROJECTILE_TURN_LIMIT,
                    )) & 0x07ff;
                    projectile.speed =
                        approach_i16_by(projectile.speed, CASTLE_PROJECTILE_SPEED, 2);
                    move_entity(
                        &mut projectile.position,
                        projectile.yaw,
                        projectile.pitch,
                        projectile.speed,
                    );

                    let ground = world.ground_height(projectile.position);
                    let terrain_hit = ground > projectile.position.z;
                    projectile.life_ticks = projectile.life_ticks.wrapping_sub(1);
                    let expired = projectile.life_ticks < 0;
                    let placement_blocked = self.castle_exists
                        || !castle_placement_valid(
                            world,
                            projectile.position,
                            &self.initial_castle_blockers,
                            &self.initial_building_blockers,
                        );
                    if !terrain_hit && !expired && !placement_blocked {
                        continue;
                    }

                    if placement_blocked && !self.castle_exists {
                        // The fixed-axis route backs up one movement step from
                        // the first invalid footprint before creating 3/2.
                        move_entity(
                            &mut projectile.position,
                            (projectile.yaw + 1_024) & 0x07ff,
                            projectile.pitch,
                            projectile.speed,
                        );
                    }
                    projectile.position.z = world.ground_height(projectile.position);
                    if self.castle_exists {
                        projectile.active = false;
                        castle_rejected = true;
                        events.push(Level1MissionEvent::CastlePlacementRejected {
                            projectile_id: projectile.id,
                        });
                    } else {
                        castle_root_request = Some((
                            projectile.id,
                            projectile.position,
                            projectile.owner_entity_index,
                        ));
                    }
                }
                Level1Projectile::Possession(projectile) => {
                    if !projectile.active {
                        continue;
                    }
                    move_entity(
                        &mut projectile.position,
                        projectile.yaw,
                        projectile.pitch,
                        projectile.speed,
                    );
                    let hit_sphere = mana_spheres.iter_mut().find(|sphere| {
                        sphere.active
                            && distance_3d_wrapped(projectile.position, sphere.position) < 256
                    });
                    let ground = world.ground_height(projectile.position);
                    let terrain_hit = ground > projectile.position.z;
                    projectile.life_ticks = projectile.life_ticks.wrapping_sub(1);
                    if let Some(sphere) = hit_sphere {
                        sphere.incoming_owner_entity_index = projectile.owner_entity_index;
                        projectile.position = sphere.position;
                        projectile.active = false;
                    } else if terrain_hit || projectile.life_ticks < 0 {
                        projectile.position.z = ground;
                        projectile.active = false;
                    }
                }
            }
        }
        if let Some((projectile_id, position, owner_entity_index)) = castle_root_request
            && let Some(footprint) = self.castle_stage_one_footprint.as_ref()
            && let Some(castle_source_entity_index) =
                source_pool.allocate(SourceOccupant::CastleRoot(next_effect_id))
        {
            let castle_presentation_id = next_effect_id;
            next_effect_id = next_effect_id.wrapping_add(1).max(1);
            bind_dynamic_source_record(
                source_slots,
                castle_source_entity_index,
                SourceOccupant::CastleRoot(castle_presentation_id),
                self.global_rng,
            );
            source_slots[usize::from(castle_source_entity_index)].phase =
                castle_source_entity_index as u8;
            let (castle_position, construction_anchor) =
                castle_root_positions(world, footprint, position);
            self.castle_exists = true;
            self.castle_source_entity_index = castle_source_entity_index;
            self.castle_presentation_id = castle_presentation_id;
            self.castle_owner_entity_index = owner_entity_index;
            self.castle_action = CASTLE_ROOT_ACTION;
            self.castle_build_state = 0;
            self.castle_level = 0;
            self.castle_position = castle_position;
            self.castle_construction_anchor = construction_anchor;
            self.castle_life = CASTLE_ROOT_CONSTRUCTOR_LIFE;
            self.castle_max_life = CASTLE_ROOT_CONSTRUCTOR_LIFE;
            self.castle_mana = 0;
            self.castle_mana_capacity = 0;
            self.castle_pending_damage = 0;
            self.castle_pending_attacker = 0;
            self.castle_killer_entity_index = 0;
            self.castle_owner_hit_feedback = 0;
            if let Some(Level1Projectile::CastleBuild(projectile)) = self
                .projectiles
                .iter_mut()
                .find(|projectile| projectile.id() == projectile_id)
            {
                projectile.active = false;
            }
        }
        for projectile in self
            .projectiles
            .iter()
            .copied()
            .filter(|projectile| !projectile.active())
        {
            source_pool.disable_occupant(projectile.source_occupant());
            source_slots[usize::from(projectile.source_entity_index())].control_flags |=
                SOURCE_DISABLED_FLAG;
        }
        self.next_effect_id = next_effect_id;
        if castle_rejected {
            self.set_castle_holder_active_value(0);
        }
        for (damage, attacker) in player_damage {
            self.submit_player_damage(damage, attacker);
        }
    }

    fn step_castle_root_slot<W: FlightWorld>(&mut self, world: &W, source_slot: u16) {
        if !self.castle_exists || self.castle_source_entity_index != source_slot {
            return;
        }
        match self.castle_action {
            CASTLE_ROOT_ACTION => match self.castle_build_state {
                0 if self.castle_level == 0 => {
                    let child_id = self.next_effect_id;
                    let Some(child_slot) = self
                        .source_pool
                        .allocate(SourceOccupant::CastleConstruction(child_id))
                    else {
                        // `sub_60480` retries on the next root dispatch and
                        // commits no stage, sound, stats, or child state.
                        return;
                    };
                    self.next_effect_id = self.next_effect_id.wrapping_add(1).max(1);
                    bind_dynamic_source_record(
                        &mut self.source_slots,
                        child_slot,
                        SourceOccupant::CastleConstruction(child_id),
                        self.global_rng,
                    );
                    // Dynamic entity counters are initialized from the arena
                    // slot's low byte, not from zero.
                    self.source_slots[usize::from(child_slot)].phase = child_slot as u8;
                    self.castle_construction = Some(CastleConstruction {
                        id: child_id,
                        source_entity_index: child_slot,
                        parent_entity_index: source_slot,
                        owner_entity_index: self.castle_owner_entity_index,
                        position: self.castle_construction_anchor,
                        stage: CASTLE_STAGE_ONE_LEVEL,
                        timer: 0,
                        initialized: false,
                        active: true,
                    });
                    self.events
                        .push(Level1MissionEvent::CastleConstructionSound {
                            sound_id: CASTLE_CONSTRUCTION_SOUND,
                            source_entity_index: source_slot,
                            position: self.castle_position,
                        });
                    self.castle_level = CASTLE_STAGE_ONE_LEVEL;
                    self.castle_action = CASTLE_ROOT_ACTION;
                    self.castle_build_state = 4;
                    self.castle_life = CASTLE_STAGE_ONE_LIFE;
                    self.castle_max_life = CASTLE_STAGE_ONE_LIFE;
                    self.castle_mana_capacity = CASTLE_STAGE_ONE_MANA_CAPACITY;
                }
                2 => {
                    self.castle_action = 4;
                    self.castle_build_state = 0;
                    self.events.push(Level1MissionEvent::CastleBuilt {
                        level: self.castle_level,
                    });
                }
                4 => {}
                _ => {}
            },
            4 => {
                self.consume_castle_damage_mailbox();
                if self.castle_life < 0 {
                    self.castle_action = 6;
                    return;
                }
                let phase = self.source_slots[usize::from(source_slot)].phase;
                // EndOfCastleProjectile_5F8F0 calls the overflow, carrier, and
                // direct-sphere paths only on even Castle phases.
                if phase & 1 != 0 || self.castle_level != 1 {
                    return;
                }
                let direct_sphere_candidates = self
                    .mana_spheres
                    .iter()
                    .filter(|sphere| sphere.active)
                    .map(|sphere| sphere.source_entity_index)
                    .collect::<Vec<_>>();
                self.eject_castle_mana_overflow(world);
                let carrier_was_dead = self
                    .mana_carrier
                    .as_ref()
                    .is_some_and(|carrier| carrier.active && carrier.life < 0);
                if carrier_was_dead {
                    self.release_mana_carrier_as_sphere(world);
                } else if self
                    .mana_carrier
                    .as_ref()
                    .is_none_or(|carrier| !carrier.active)
                {
                    self.create_stage_one_mana_carrier();
                } else {
                    self.schedule_stage_one_mana_carrier();
                }
                self.absorb_direct_owned_mana_sphere(&direct_sphere_candidates);
            }
            6 => {
                // `sub_5FCA0_destroy_castle_level` requires an ordinary free
                // entity record even though its sentinel-backed terrain
                // removal does not consume that record. With none available,
                // the root returns to action 4 and its still-negative life
                // selects action 6 again on the following dispatch.
                if !self.source_pool.has_ordinary_free_slot() {
                    self.castle_action = 4;
                    return;
                }
                self.events.push(Level1MissionEvent::MissionSound {
                    sound_id: CASTLE_DESTRUCTION_SOUND,
                    source_entity_index: self.castle_source_entity_index,
                    position: self.castle_position,
                    random_state: self.global_rng,
                });
                self.castle_terrain_updates
                    .push(CastleTerrainUpdate::StageDestroyed {
                        position: self.castle_construction_anchor,
                        // `sub_605E0` invokes removal through sentinel
                        // `Entities[0]`; its RNG is distinct from the root.
                        entity_rng: self.source_slots[0].entity_rng,
                    });
                self.release_mana_carrier_as_sphere(world);
                self.castle_level = 0;
                self.castle_action = 4;
                self.castle_build_state = 0;
                self.castle_mana = 0;
                self.castle_mana_capacity = 5_000;
                self.castle_owner_entity_index = 0;
                self.castle_exists = false;
                self.source_pool
                    .disable_occupant(SourceOccupant::CastleRoot(self.castle_presentation_id));
                self.source_slots[usize::from(source_slot)].control_flags |= SOURCE_DISABLED_FLAG;
            }
            _ => {}
        }
    }

    fn create_stage_one_mana_carrier(&mut self) {
        let Some(carrier_slot) = self.allocate_mana_carrier_source() else {
            return;
        };
        self.source_slots[usize::from(carrier_slot)].phase = carrier_slot as u8;
        let entity_rng = self.source_slots[usize::from(carrier_slot)].entity_rng;
        self.mana_carrier = Some(ManaCarrier {
            id: u32::from(carrier_slot),
            source_entity_index: carrier_slot,
            entity_rng,
            owner_entity_index: self.castle_owner_entity_index,
            position: self.castle_position,
            yaw: 0,
            pitch: 0,
            speed: MANA_CARRIER_SPEED,
            life: MANA_CARRIER_MAX_LIFE,
            max_life: MANA_CARRIER_MAX_LIFE,
            max_mana: MANA_CARRIER_MAX_MANA,
            target_sphere_id: 0,
            carried_mana: 0,
            returning_to_castle: false,
            pending_damage: 0,
            pending_attacker_entity_index: 0,
            active: true,
        });
    }

    fn schedule_stage_one_mana_carrier(&mut self) {
        let Some(carrier) = self
            .mana_carrier
            .as_ref()
            .copied()
            .filter(|carrier| carrier.active)
        else {
            return;
        };
        let must_return = self.secured_mana_component.saturating_add(self.castle_mana)
            >= self.castle_mana_capacity
            || carrier.carried_mana >= carrier.max_mana;
        let target = (!must_return)
            .then(|| {
                self.mana_spheres
                    .iter()
                    .filter(|sphere| {
                        sphere.active
                            && sphere.owner_entity_index == carrier.owner_entity_index
                            && (self.source_slots[usize::from(sphere.source_entity_index)]
                                .control_flags
                                & (SOURCE_DISABLED_FLAG | SOURCE_FADING_FLAG))
                                == 0
                    })
                    .min_by_key(|sphere| {
                        (
                            squared_wrapping_xyz_distance(carrier.position, sphere.position),
                            sphere.source_entity_index,
                        )
                    })
                    .map(|sphere| sphere.source_entity_index)
            })
            .flatten();
        let carrier = self
            .mana_carrier
            .as_mut()
            .expect("copied active carrier remains installed");
        if let Some(target_slot) = target {
            carrier.target_sphere_id = u32::from(target_slot);
            carrier.returning_to_castle = false;
        } else {
            carrier.target_sphere_id = u32::from(self.castle_source_entity_index);
            carrier.returning_to_castle = true;
        }
    }

    fn eject_castle_mana_overflow<W: FlightWorld>(&mut self, world: &W) {
        let mut overflow = if self.secured_mana_component.saturating_add(self.castle_mana)
            > self.castle_mana_capacity
        {
            self.castle_mana.wrapping_sub(self.castle_mana_capacity)
        } else {
            0
        };
        if self.castle_level == 0 {
            overflow = self.castle_mana;
        }
        if overflow <= 0 {
            return;
        }

        if !self.source_pool.has_ordinary_free_slot() {
            self.source_pool.rebuild_free_stack();
        }
        let available = i32::try_from(self.source_pool.free_stack.len())
            .expect("bounded source-pool free count fits i32");
        if available <= 0 {
            return;
        }
        let requested = (overflow / 1_000).clamp(1, 32);
        let count = available.min(requested);
        let mut share = overflow / count;
        for _ in 0..count {
            if share <= 0 {
                break;
            }
            if self.spawn_castle_overflow_sphere(world, share).is_some() {
                overflow = overflow.wrapping_sub(share);
                self.castle_mana = self.castle_mana.wrapping_sub(share);
                if overflow < share {
                    share = overflow;
                }
            }
        }
    }

    fn spawn_castle_overflow_sphere<W: FlightWorld>(
        &mut self,
        world: &W,
        mana: i32,
    ) -> Option<u32> {
        let sphere_slot = self.allocate_mana_sphere_source()?;
        let id = u32::from(sphere_slot);
        let phase = sphere_slot as u8;
        self.source_slots[usize::from(sphere_slot)].phase = phase;
        let sphere_rng = next_rng16(self.source_slots[usize::from(sphere_slot)].entity_rng);
        self.source_slots[usize::from(sphere_slot)].entity_rng = sphere_rng;
        let _source_act_speed =
            i16::try_from(sphere_rng % 0x30 + 16).expect("Castle overflow sphere speed fits i16");
        let height_above_ground = i32::from(self.castle_position.z)
            - i32::from(world.ground_height(self.castle_position));
        let vertical_velocity = (1_024 - height_above_ground).wrapping_div(8) as i16;

        let castle_record = &mut self.source_slots[usize::from(self.castle_source_entity_index)];
        castle_record.entity_rng = next_rng16(castle_record.entity_rng);
        let radius = i16::try_from(castle_record.entity_rng % 0x1400 + 3_840)
            .expect("Castle overflow radius fits i16");
        castle_record.entity_rng = next_rng16(castle_record.entity_rng);
        let yaw = i16::try_from(castle_record.entity_rng & 0x07ff)
            .expect("Castle overflow yaw is 11-bit");
        let mut position = self.castle_position;
        move_entity(&mut position, yaw, 0, radius);

        self.mana_spheres.push(ManaSphere {
            id,
            source_entity_index: sphere_slot,
            entity_rng: sphere_rng,
            position,
            velocity_x: 0,
            velocity_y: 0,
            vertical_velocity,
            yaw: 0,
            mana,
            owner_entity_index: self.castle_owner_entity_index,
            incoming_owner_entity_index: 0,
            assigned_carrier_id: 0,
            phase,
            active: true,
            pending_delete: false,
        });
        self.mana_sphere_map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: sphere_slot,
                active: true,
                x: position.x,
                y: position.y,
            })
            .expect("qualified Castle overflow sphere fits the map chain");
        self.events.push(Level1MissionEvent::ManaSphereSpawned {
            sphere_id: id,
            source_entity_index: self.castle_source_entity_index,
            mana,
        });
        Some(id)
    }

    fn absorb_direct_owned_mana_sphere(&mut self, candidates: &[u16]) {
        if self.castle_mana >= self.castle_mana_capacity {
            return;
        }
        let absorbed_index = self
            .mana_spheres
            .iter()
            .enumerate()
            .filter(|(_, sphere)| {
                sphere.active
                    && sphere.owner_entity_index == self.castle_owner_entity_index
                    && candidates.contains(&sphere.source_entity_index)
                    && {
                        let sphere_extent = mana_sphere_collision_dimension(sphere.mana);
                        let dx = i32::from(
                            sphere
                                .position
                                .x
                                .wrapping_sub(self.castle_position.x)
                                .cast_signed(),
                        )
                        .abs();
                        let dy = i32::from(
                            sphere
                                .position
                                .y
                                .wrapping_sub(self.castle_position.y)
                                .cast_signed(),
                        )
                        .abs();
                        dx < i32::from(CASTLE_STAGE_ONE_COLLISION_HALF_WIDTH + sphere_extent)
                            && dy < i32::from(CASTLE_STAGE_ONE_COLLISION_HALF_WIDTH + sphere_extent)
                    }
            })
            .min_by_key(|(_, sphere)| sphere.source_entity_index)
            .map(|(index, _)| index);
        let Some(absorbed_index) = absorbed_index else {
            return;
        };
        let sphere = self.mana_spheres[absorbed_index];
        self.castle_mana = self.castle_mana.wrapping_add(sphere.mana);
        self.mana_spheres[absorbed_index].pending_delete = true;
        self.source_pool
            .disable_occupant(SourceOccupant::ManaSphere(sphere.id));
        self.source_slots[usize::from(sphere.source_entity_index)].control_flags |=
            SOURCE_DISABLED_FLAG;
        self.events.push(Level1MissionEvent::CastleManaChanged {
            mana: self.castle_mana,
        });
    }

    fn release_mana_carrier_as_sphere<W: FlightWorld>(&mut self, world: &W) {
        let Some(carrier) = self
            .mana_carrier
            .as_ref()
            .copied()
            .filter(|carrier| carrier.active)
        else {
            return;
        };
        if carrier.carried_mana > 0 {
            let rng_after_conversion = next_rng16(carrier.entity_rng);
            if let Some(installed) = self.mana_carrier.as_mut() {
                installed.entity_rng = rng_after_conversion;
            }
            self.spawn_mana_sphere_from_death(
                world,
                carrier.source_entity_index,
                carrier.position,
                carrier.yaw,
                rng_after_conversion,
                carrier.carried_mana,
                carrier.owner_entity_index,
            );
        }
        if let Some(installed) = self.mana_carrier.as_mut() {
            if carrier.carried_mana > 0 {
                installed.owner_entity_index = 0;
            }
            installed.active = false;
        }
        self.source_pool
            .disable_occupant(SourceOccupant::ManaCarrier(u32::from(
                carrier.source_entity_index,
            )));
        self.source_slots[usize::from(carrier.source_entity_index)].control_flags |=
            SOURCE_DISABLED_FLAG;
    }

    fn step_castle_construction_slot(&mut self, source_slot: u16) {
        let Some(child) = self
            .castle_construction
            .as_mut()
            .filter(|child| child.active && child.source_entity_index == source_slot)
        else {
            return;
        };
        if !child.initialized {
            child.initialized = true;
            child.timer = CASTLE_CONSTRUCTION_INITIAL_TIMER;
        }
        if child.timer > 0 {
            child.timer -= 1;
            if child.timer == 0 {
                // `sub_60480` clears byte_0x3B, selecting the short -1 tail.
                child.timer = -1;
            } else {
                self.castle_terrain_updates
                    .push(CastleTerrainUpdate::ConstructionPass {
                        position: child.position,
                        remaining: child.timer,
                    });
            }
            return;
        }
        child.timer += 1;
        if child.timer != 0 {
            return;
        }
        child.active = false;
        self.castle_build_state = 2;
        self.castle_terrain_updates
            .push(CastleTerrainUpdate::ConstructionCompleted {
                position: child.position,
            });
        self.source_pool
            .disable_occupant(SourceOccupant::CastleConstruction(child.id));
        self.source_slots[usize::from(source_slot)].control_flags |= SOURCE_DISABLED_FLAG;
    }

    /// Test-only stable fixture for systems whose subject is shelter, damage,
    /// or carrier behavior rather than construction ordering.
    #[cfg(test)]
    fn install_stable_stage_one_fixture<W: FlightWorld>(
        &mut self,
        world: &W,
        position: Position,
        owner_entity_index: u16,
    ) {
        let footprint = self
            .castle_stage_one_footprint
            .clone()
            .unwrap_or_else(CastleBuildFootprint::synthetic_stage_one);
        self.castle_stage_one_footprint = Some(footprint.clone());
        let (castle_position, construction_anchor) =
            castle_root_positions(world, &footprint, position);
        let (castle_presentation_id, castle_source_entity_index) = self
            .allocate_effect_source(SourceOccupant::CastleRoot)
            .expect("stable fixture Castle root");
        self.source_slots[usize::from(castle_source_entity_index)].phase =
            castle_source_entity_index as u8;
        self.castle_exists = true;
        self.castle_source_entity_index = castle_source_entity_index;
        self.castle_presentation_id = castle_presentation_id;
        self.castle_owner_entity_index = owner_entity_index;
        self.castle_action = 4;
        self.castle_build_state = 0;
        self.castle_level = CASTLE_STAGE_ONE_LEVEL;
        self.castle_position = castle_position;
        self.castle_construction_anchor = construction_anchor;
        self.castle_life = CASTLE_STAGE_ONE_LIFE;
        self.castle_max_life = CASTLE_STAGE_ONE_LIFE;
        self.castle_mana = 0;
        self.castle_mana_capacity = CASTLE_STAGE_ONE_MANA_CAPACITY;
        let carrier_slot = self
            .allocate_mana_carrier_source()
            .expect("stable fixture mana carrier");
        self.source_slots[usize::from(carrier_slot)].phase = carrier_slot as u8;
        let entity_rng = self.source_slots[usize::from(carrier_slot)].entity_rng;
        self.mana_carrier = Some(ManaCarrier {
            id: u32::from(carrier_slot),
            source_entity_index: carrier_slot,
            entity_rng,
            owner_entity_index,
            position: castle_position,
            yaw: 0,
            pitch: 0,
            speed: MANA_CARRIER_SPEED,
            life: MANA_CARRIER_MAX_LIFE,
            max_life: MANA_CARRIER_MAX_LIFE,
            max_mana: MANA_CARRIER_MAX_MANA,
            target_sphere_id: 0,
            carried_mana: 0,
            returning_to_castle: false,
            pending_damage: 0,
            pending_attacker_entity_index: 0,
            active: true,
        });
    }

    #[cfg(test)]
    fn step_mana_spheres<W: FlightWorld>(&mut self, world: &W, player_entity_index: u16) {
        let slots = self
            .mana_spheres
            .iter()
            .filter(|sphere| sphere.active)
            .map(|sphere| sphere.source_entity_index)
            .collect::<Vec<_>>();
        for slot in slots {
            self.step_mana_sphere_slot(world, player_entity_index, slot);
        }
    }

    fn step_mana_sphere_slot<W: FlightWorld>(
        &mut self,
        world: &W,
        _player_entity_index: u16,
        source_slot: u16,
    ) {
        let Some(sphere_index) = self
            .mana_spheres
            .iter()
            .position(|sphere| sphere.active && sphere.source_entity_index == source_slot)
        else {
            return;
        };
        let grounded;
        {
            let sphere = &mut self.mana_spheres[sphere_index];
            if sphere.incoming_owner_entity_index != 0 {
                let owner = sphere.incoming_owner_entity_index;
                sphere.incoming_owner_entity_index = 0;
                if sphere.owner_entity_index != owner {
                    sphere.owner_entity_index = owner;
                    self.events.push(Level1MissionEvent::ManaSphereClaimed {
                        sphere_id: sphere.id,
                        owner_entity_index: owner,
                        sound_id: POSSESSION_SOUND,
                    });
                }
            }

            if sphere.assigned_carrier_id != 0 {
                let carrier = self.mana_carrier.as_ref().copied().filter(|carrier| {
                    carrier.active
                        && u32::from(carrier.source_entity_index) == sphere.assigned_carrier_id
                });
                if let Some(carrier) = carrier {
                    sphere.vertical_velocity = 128;
                    sphere.yaw =
                        i16::try_from(heading_for_positions(sphere.position, carrier.position))
                            .expect("carrier attraction heading is 11-bit");
                    let distance = wrapping_xy_distance(sphere.position, carrier.position);
                    if distance <= MANA_CARRIER_ATTRACTION_RADIUS {
                        if distance >= 16 {
                            move_entity(
                                &mut sphere.position,
                                sphere.yaw,
                                0,
                                MANA_CARRIER_ATTRACTION_SPEED,
                            );
                        } else {
                            sphere.position.x = carrier.position.x;
                            sphere.position.y = carrier.position.y;
                            if sphere.position.z >= carrier.position.z {
                                if sphere.position.z > carrier.position.z.wrapping_add(512) {
                                    sphere.position.z = sphere
                                        .position
                                        .z
                                        .wrapping_sub(MANA_CARRIER_ATTRACTION_VERTICAL_STEP);
                                }
                            } else {
                                sphere.position.z = sphere
                                    .position
                                    .z
                                    .wrapping_add(MANA_CARRIER_ATTRACTION_VERTICAL_STEP);
                            }
                        }
                        let ground = world.ground_height(sphere.position);
                        if sphere.position.z < ground {
                            sphere.position.z = ground;
                        }
                    } else {
                        sphere.assigned_carrier_id = 0;
                    }
                    sphere.phase = sphere.phase.wrapping_add(1);
                    let sphere_position = sphere.position;
                    self.mana_sphere_map_chains
                        .sync_entity(RenderableMapSeed {
                            entity_index: source_slot,
                            active: true,
                            x: sphere_position.x,
                            y: sphere_position.y,
                        })
                        .expect("qualified attracted sphere source slot fits the map chain");
                    return;
                }
                // Invalid links are cleared, but action 41 does not fall
                // through to ordinary gravity until its next source turn.
                sphere.assigned_carrier_id = 0;
                sphere.phase = sphere.phase.wrapping_add(1);
                return;
            }

            sphere.velocity_x = sphere
                .velocity_x
                .clamp(-MANA_SPHERE_MAX_XY_VELOCITY, MANA_SPHERE_MAX_XY_VELOCITY);
            sphere.velocity_y = sphere
                .velocity_y
                .clamp(-MANA_SPHERE_MAX_XY_VELOCITY, MANA_SPHERE_MAX_XY_VELOCITY);
            sphere.position.x = sphere.position.x.wrapping_add_signed(sphere.velocity_x);
            sphere.position.y = sphere.position.y.wrapping_add_signed(sphere.velocity_y);
            sphere.position.z = sphere.position.z.wrapping_add(sphere.vertical_velocity);
            sphere.vertical_velocity = sphere.vertical_velocity.wrapping_sub(MANA_SPHERE_GRAVITY);
            if sphere.vertical_velocity < MANA_SPHERE_TERMINAL_VELOCITY {
                sphere.vertical_velocity = MANA_SPHERE_TERMINAL_VELOCITY;
            }
            let ground = world.ground_height(sphere.position);
            if sphere.position.z < ground {
                sphere.position.z = ground;
                sphere.vertical_velocity =
                    i16::try_from(-(i32::from(sphere.vertical_velocity) / 4))
                        .expect("quarter of i16 velocity fits i16");
                if sphere.vertical_velocity <= 16 {
                    sphere.vertical_velocity = 0;
                }
            }
            grounded = sphere.position.z == ground;
            if grounded {
                let (slope_x, slope_y) = mana_sphere_terrain_slope_impulse(world, sphere.position);
                sphere.velocity_x = sphere.velocity_x.wrapping_add(slope_x);
                sphere.velocity_y = sphere.velocity_y.wrapping_add(slope_y);
                sphere.velocity_x =
                    i16::try_from(i32::from(sphere.velocity_x) * 250 / 256).unwrap_or(0);
                sphere.velocity_y =
                    i16::try_from(i32::from(sphere.velocity_y) * 250 / 256).unwrap_or(0);
            }
            sphere.phase = sphere.phase.wrapping_add(1);
        }
        let sphere_position = self.mana_spheres[sphere_index].position;
        self.mana_sphere_map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: sphere_position.x,
                y: sphere_position.y,
            })
            .expect("qualified mana sphere source slot fits the map chain");
        if !grounded {
            return;
        }

        let Some(absorbed_slot) = self.first_colliding_mana_sphere_slot(source_slot) else {
            return;
        };
        let absorbed_index = self
            .mana_spheres
            .iter()
            .position(|sphere| sphere.active && sphere.source_entity_index == absorbed_slot)
            .expect("map chain collision references a live sphere");
        let absorbed = self.mana_spheres[absorbed_index];
        let survivor_index = self
            .mana_spheres
            .iter()
            .position(|sphere| sphere.active && sphere.source_entity_index == source_slot)
            .expect("dispatched sphere remains live");
        let survivor_owner = self.mana_spheres[survivor_index].owner_entity_index;
        self.mana_spheres[survivor_index].mana = self.mana_spheres[survivor_index]
            .mana
            .wrapping_add(absorbed.mana);
        self.mana_spheres[survivor_index].owner_entity_index = merged_mana_owner(
            &self.source_slots,
            survivor_owner,
            absorbed.owner_entity_index,
        );

        self.mana_sphere_map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: absorbed_slot,
                active: false,
                x: absorbed.position.x,
                y: absorbed.position.y,
            })
            .expect("qualified mana sphere source slot fits the map chain");
        assert!(
            self.source_pool.reclaim_slot_immediately(absorbed_slot),
            "absorbed sphere owns its source slot"
        );
        clear_source_record(&mut self.source_slots[usize::from(absorbed_slot)]);
        self.mana_spheres.remove(absorbed_index);
    }

    fn first_colliding_mana_sphere_slot(&self, source_slot: u16) -> Option<u16> {
        let sphere = self
            .mana_spheres
            .iter()
            .find(|sphere| sphere.active && sphere.source_entity_index == source_slot)?;
        // The source computes `(position + 128) >> 8`, then narrows the map
        // lookup back to one byte. Do the addition wide so 0xffff rounds
        // through tile 0 exactly as the promoted C expression does.
        let tile_x = ((u32::from(sphere.position.x) + 128) >> 8) as u8;
        let tile_y = ((u32::from(sphere.position.y) + 128) >> 8) as u8;
        for (delta_x, delta_y) in MANA_SPHERE_SEARCH_OFFSETS {
            let search_x = tile_x.wrapping_add_signed(delta_x);
            let search_y = tile_y.wrapping_add_signed(delta_y);
            for candidate_slot in self.mana_sphere_map_chains.entity_chain(search_x, search_y) {
                if candidate_slot == source_slot {
                    continue;
                }
                let candidate = self.mana_spheres.iter().find(|candidate| {
                    candidate.active && candidate.source_entity_index == candidate_slot
                })?;
                if mana_spheres_overlap(*sphere, *candidate) {
                    return Some(candidate_slot);
                }
            }
        }
        None
    }

    fn step_death_burst_slot(&mut self, source_slot: u16) {
        let Some(burst) = self
            .death_bursts
            .iter_mut()
            .find(|burst| burst.active && burst.source_entity_index == source_slot)
        else {
            return;
        };
        burst.life_ticks -= 1;
        if burst.life_ticks <= 0 {
            burst.active = false;
            self.source_pool.disable_slot(source_slot);
            self.source_slots[usize::from(source_slot)].control_flags |= SOURCE_DISABLED_FLAG;
        }
        self.death_bursts.retain(|burst| burst.active);
    }

    #[cfg(test)]
    fn step_mana_carrier<W: FlightWorld>(&mut self, world: &W) {
        let Some(source_slot) = self
            .mana_carrier
            .as_ref()
            .filter(|carrier| carrier.active)
            .map(|carrier| carrier.source_entity_index)
        else {
            return;
        };
        self.step_mana_carrier_slot(world, source_slot);
    }

    fn step_mana_carrier_slot<W: FlightWorld>(&mut self, world: &W, source_slot: u16) {
        let Some(original) = self
            .mana_carrier
            .as_ref()
            .copied()
            .filter(|carrier| carrier.active)
        else {
            return;
        };
        if original.source_entity_index != source_slot {
            return;
        }

        let mut position = original.position;
        let mut yaw = original.yaw;
        let mut carried_mana = original.carried_mana;
        let mut owner_entity_index = original.owner_entity_index;
        let mut life = original.life;
        let mut target_sphere_id = original.target_sphere_id;
        let returning_to_castle = original.returning_to_castle;
        let mut valid_target = false;

        if returning_to_castle && self.castle_exists {
            valid_target = true;
            yaw = i16::try_from(heading_for_positions(position, self.castle_position))
                .expect("carrier Castle heading is 11-bit");
            let distance = wrapping_xy_distance(position, self.castle_position);
            let arrival = u32::from(original.speed.cast_unsigned())
                .saturating_mul(u32::from(self.castle_level));
            if distance <= arrival {
                let ground = world.ground_height(position);
                if position.z <= ground.wrapping_add(MANA_CARRIER_GROUND_CLEARANCE)
                    && self.castle_level > 0
                {
                    position.x = self.castle_position.x;
                    position.y = self.castle_position.y;
                    let deposited = carried_mana;
                    self.castle_mana = self.castle_mana.wrapping_add(deposited);
                    carried_mana = 0;
                    owner_entity_index = self.castle_owner_entity_index;
                    life = original.max_life;
                    if deposited != 0 {
                        self.events.push(Level1MissionEvent::CastleManaChanged {
                            mana: self.castle_mana,
                        });
                    }
                }
            } else {
                move_entity(&mut position, yaw, original.pitch, original.speed);
            }
        } else if target_sphere_id != 0 {
            let sphere_index = self.mana_spheres.iter().position(|sphere| {
                sphere.active
                    && sphere.source_entity_index
                        == u16::try_from(target_sphere_id).unwrap_or(u16::MAX)
            });
            if let Some(sphere_index) = sphere_index {
                valid_target = true;
                let sphere = self.mana_spheres[sphere_index];
                yaw = i16::try_from(heading_for_positions(position, sphere.position))
                    .expect("carrier sphere heading is 11-bit");
                if sphere.owner_entity_index == owner_entity_index {
                    let distance = wrapping_xy_distance(position, sphere.position);
                    if distance > MANA_CARRIER_ATTRACTION_RADIUS {
                        self.mana_spheres[sphere_index].assigned_carrier_id = 0;
                    } else {
                        self.mana_spheres[sphere_index].assigned_carrier_id =
                            u32::from(source_slot);
                        let sphere_dimension = mana_sphere_collision_dimension(sphere.mana);
                        if overlaps_source_entities(
                            position,
                            MANA_CARRIER_COLLISION_HALF_WIDTH,
                            MANA_CARRIER_COLLISION_VERTICAL_OFFSET,
                            MANA_CARRIER_COLLISION_HALF_HEIGHT,
                            sphere.position,
                            sphere_dimension,
                            sphere_dimension,
                            sphere_dimension,
                        ) {
                            carried_mana = carried_mana.wrapping_add(sphere.mana);
                            owner_entity_index = sphere.owner_entity_index;
                            life = original.max_life;
                            target_sphere_id = 0;
                            self.mana_spheres[sphere_index].pending_delete = true;
                            self.source_pool
                                .disable_occupant(SourceOccupant::ManaSphere(sphere.id));
                            self.source_slots[usize::from(sphere.source_entity_index)]
                                .control_flags |= SOURCE_DISABLED_FLAG;
                            self.events.push(Level1MissionEvent::ManaSphereCollected {
                                sphere_id: sphere.id,
                                carrier_id: u32::from(source_slot),
                                mana: sphere.mana,
                            });
                        }
                    }
                    if distance <= u32::from(original.speed.cast_unsigned()) {
                        position.x = sphere.position.x;
                        position.y = sphere.position.y;
                    } else {
                        move_entity(&mut position, yaw, original.pitch, original.speed);
                    }
                }
            }
        }

        if valid_target {
            relax_to_terrain(
                &mut position,
                world,
                MANA_CARRIER_GROUND_CLEARANCE,
                MANA_CARRIER_DESCENT,
            );
        }

        let carrier = self
            .mana_carrier
            .as_mut()
            .expect("active carrier remains installed during its source action");
        carrier.position = position;
        carrier.yaw = yaw;
        carrier.carried_mana = carried_mana;
        carrier.owner_entity_index = owner_entity_index;
        carrier.life = life;
        carrier.target_sphere_id = target_sphere_id;

        // sub_60EA0 consumes damage after movement, collection, deposit, and
        // outdoor altitude handling.
        if carrier.life >= 0 && carrier.pending_attacker_entity_index != 0 {
            carrier.life = carrier.life.wrapping_sub(carrier.pending_damage);
            if carrier.life < 0 {
                // The source preserves the attacker relation on death.
            } else {
                carrier.pending_attacker_entity_index = 0;
            }
        }
    }

    fn step_impact_slot<W: FlightWorld>(
        &mut self,
        world: &W,
        player: &FlightState,
        source_slot: u16,
    ) {
        let mut player_damage = Vec::new();
        let mut castle_damage = Vec::new();
        let mut carrier_damage = Vec::new();
        let castle_collision = self
            .castle_exists
            .then_some((self.castle_source_entity_index, self.castle_position));
        let carrier_collision = self
            .mana_carrier
            .as_ref()
            .copied()
            .filter(|carrier| carrier.active && carrier.life >= 0);
        for impact in &mut self.impacts {
            if impact.source_entity_index != source_slot {
                continue;
            }
            if !impact.active {
                continue;
            }
            impact.life_ticks = impact.life_ticks.wrapping_sub(1);
            if impact.life_ticks < -1 {
                impact.active = false;
                continue;
            }
            if impact.model == WATER_SPLASH_MODEL {
                if !impact.damage_submitted {
                    self.events.push(Level1MissionEvent::WaterSplashed {
                        projectile_id: impact.source_projectile_id,
                        splash_id: impact.id,
                        position: impact.position,
                        sound_id: WATER_SPLASH_SOUND,
                    });
                    // The shared field is the class-10 first-update latch.
                    // Model 5 never submits damage.
                    impact.damage_submitted = true;
                }
                impact.animation_frame = impact.animation_frame.wrapping_add(1);
                continue;
            }
            if !impact.damage_submitted {
                if impact.owner_entity_index != player.entity_index
                    && overlaps_source_entities(
                        impact.position,
                        FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                        FIREBALL_IMPACT_COLLISION_VERTICAL_OFFSET,
                        FIREBALL_IMPACT_COLLISION_HALF_HEIGHT,
                        player.position,
                        PLAYER_COLLISION_HALF_WIDTH,
                        PLAYER_COLLISION_VERTICAL_OFFSET,
                        PLAYER_COLLISION_HALF_HEIGHT,
                    )
                {
                    player_damage.push((impact.payload, impact.owner_entity_index));
                }
                if let Some((castle_source_entity_index, castle_position)) = castle_collision
                    && impact.owner_entity_index != castle_source_entity_index
                    && overlaps_source_entities(
                        impact.position,
                        FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                        FIREBALL_IMPACT_COLLISION_VERTICAL_OFFSET,
                        FIREBALL_IMPACT_COLLISION_HALF_HEIGHT,
                        castle_position,
                        CASTLE_STAGE_ONE_COLLISION_HALF_WIDTH,
                        CASTLE_STAGE_ONE_COLLISION_VERTICAL_OFFSET,
                        CASTLE_STAGE_ONE_COLLISION_HALF_HEIGHT,
                    )
                {
                    castle_damage.push((impact.payload, impact.owner_entity_index));
                }
                if let Some(carrier) = carrier_collision
                    && impact.owner_entity_index != carrier.owner_entity_index
                    && overlaps_source_entities(
                        impact.position,
                        FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                        FIREBALL_IMPACT_COLLISION_VERTICAL_OFFSET,
                        FIREBALL_IMPACT_COLLISION_HALF_HEIGHT,
                        carrier.position,
                        MANA_CARRIER_COLLISION_HALF_WIDTH,
                        MANA_CARRIER_COLLISION_VERTICAL_OFFSET,
                        MANA_CARRIER_COLLISION_HALF_HEIGHT,
                    )
                {
                    carrier_damage.push((impact.payload, impact.owner_entity_index));
                }
                for actor in &mut self.actors {
                    if actor.alive()
                        && actor.source_entity_index != impact.owner_entity_index
                        && matches!(
                            actor.kind,
                            Level1ActorKind::Archer
                                | Level1ActorKind::Firefly
                                | Level1ActorKind::MultipartHead
                        )
                        && overlaps_source_entities(
                            impact.position,
                            FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                            FIREBALL_IMPACT_COLLISION_VERTICAL_OFFSET,
                            FIREBALL_IMPACT_COLLISION_HALF_HEIGHT,
                            actor.position,
                            actor.collision_half_width,
                            actor.collision_vertical_offset,
                            actor.collision_half_height,
                        )
                    {
                        actor.pending_damage = actor.pending_damage.saturating_add(impact.payload);
                        actor.pending_attacker = impact.owner_entity_index;
                    }
                }
                for segment in &mut self.multipart_segments {
                    if segment.active
                        && segment.life >= 0
                        && segment.owner_id.cast_unsigned() != impact.owner_entity_index
                        && overlaps_source_entities(
                            impact.position,
                            FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                            FIREBALL_IMPACT_COLLISION_VERTICAL_OFFSET,
                            FIREBALL_IMPACT_COLLISION_HALF_HEIGHT,
                            segment.position,
                            segment.collision_half_width,
                            0,
                            segment.collision_half_height,
                        )
                    {
                        segment.pending_damage =
                            segment.pending_damage.saturating_add(impact.payload);
                        segment.pending_attacker = impact.owner_entity_index;
                    }
                }
                self.events.push(Level1MissionEvent::FireballImpacted {
                    projectile_id: impact.source_projectile_id,
                    impact_id: impact.id,
                    position: impact.position,
                    sound_id: 3,
                });
                impact.entity_rng = next_rng16(impact.entity_rng);
                impact.vertical_velocity =
                    i16::try_from(impact.entity_rng % 65).expect("impact velocity fits i16") - 32;
                self.source_slots[usize::from(impact.source_entity_index)].entity_rng =
                    impact.entity_rng;
                impact.damage_submitted = true;
            }
            let ground = world.ground_height(impact.position);
            if impact.position.z > ground {
                impact.position.z = impact.position.z.wrapping_add(impact.vertical_velocity);
            }
            if impact.position.z <= ground {
                impact.position.z = ground;
            }
            impact.animation_frame = impact.animation_frame.wrapping_add(1);
        }
        for (damage, attacker) in player_damage {
            self.submit_player_damage(damage, attacker);
        }
        for (damage, attacker) in castle_damage {
            self.submit_castle_damage(damage, attacker);
        }
        for (damage, attacker) in carrier_damage {
            if let Some(carrier) = self.mana_carrier.as_mut().filter(|carrier| carrier.active) {
                carrier.pending_damage = carrier.pending_damage.wrapping_add(damage);
                carrier.pending_attacker_entity_index = attacker;
            }
        }
        for impact in self.impacts.iter().filter(|impact| !impact.active) {
            self.source_pool.disable_occupant(impact.source_occupant());
            self.source_slots[usize::from(impact.source_entity_index)].control_flags |=
                SOURCE_DISABLED_FLAG;
        }
    }

    fn evaluate_objective(
        &mut self,
        player: Position,
        archers_present_at_update_start: bool,
        fireflies_present_at_update_start: bool,
    ) {
        if self.objective_pause != 0 {
            self.objective_pause -= 1;
            return;
        }

        // remc2's type-0 castle/mana predicate is the one Level 1 objective
        // that may complete before it becomes the current presentation row.
        if self.objective_states[3] == 1
            && self.castle_exists
            && self.secured_mana_percent() >= i32::from(LEVEL1_CASTLE_MANA_PERCENT)
        {
            self.objective_states[3] = 2;
        }

        let completed_stage = self.current_stage;
        let complete = match completed_stage {
            0 => is_near_xy(player, LEVEL1_FIRST_WAYPOINT, LEVEL1_REACH_TOLERANCE),
            1 => is_near_xy(player, LEVEL1_SECOND_WAYPOINT, LEVEL1_REACH_TOLERANCE),
            2 => !archers_present_at_update_start,
            3 => self.objective_states[3] == 2,
            4 => !fireflies_present_at_update_start,
            _ => false,
        };
        if !complete {
            return;
        }

        self.objective_states[usize::from(completed_stage)] = 2;
        self.current_stage = self
            .objective_states
            .iter()
            .position(|state| *state == 1)
            .map_or(5, |index| {
                u8::try_from(index).expect("five Level 1 objectives")
            });
        self.objective_textbox_ticks = 200;
        self.exit_palette_submode = LEVEL1_PALETTE_OBJECTIVE_REQUEST;
        if self.current_stage >= 5 {
            self.current_text_selector = 53;
            self.level_completed = true;
            self.events.push(Level1MissionEvent::LevelCompleted);
        } else {
            self.current_text_selector = 48 + self.current_stage;
        }
        self.events.push(Level1MissionEvent::ObjectiveAdvanced {
            completed_stage,
            text_selector: self.current_text_selector,
        });
    }

    #[cfg(test)]
    fn step_switches(&mut self, player: &FlightState) {
        for source_slot in 1..ENTITY_POOL_SIZE {
            let source_slot = u16::try_from(source_slot).expect("entity source slot fits u16");
            self.step_switch_slot(player, source_slot);
        }
    }

    fn step_switch_slot(&mut self, player: &FlightState, source_slot: u16) {
        match source_slot {
            48 if self.initial_zone_switch_active => {
                if self.initial_zone_switch_phase & 7 == 0
                    && !inside_switch_bounds(
                        player.position,
                        LEVEL1_INITIAL_ZONE_SWITCH_CENTER,
                        LEVEL1_INITIAL_ZONE_SWITCH_RADIUS_XY,
                        LEVEL1_INITIAL_ZONE_SWITCH_RADIUS_Z,
                    )
                {
                    self.activate_group(1);
                    self.initial_zone_switch_active = false;
                    self.source_slots[48].control_flags |= SOURCE_DISABLED_FLAG;
                    self.pending_source_slot_releases.push(48);
                }
                self.initial_zone_switch_phase = self.initial_zone_switch_phase.wrapping_add(1);
                self.source_slots[48].phase = self.initial_zone_switch_phase;
            }
            112 if self.completion_switch_active => {
                if self.level_completed {
                    self.activate_group(4);
                    self.completion_switch_active = false;
                    self.source_slots[112].control_flags |= SOURCE_DISABLED_FLAG;
                    self.pending_source_slot_releases.push(112);
                }
                self.source_slots[112].phase = self.source_slots[112].phase.wrapping_add(1);
            }
            151 if self.first_waypoint_switch_active => {
                if self.first_waypoint_switch_phase & 7 == 0
                    && inside_switch_bounds(
                        player.position,
                        LEVEL1_FIRST_WAYPOINT_SWITCH_CENTER,
                        LEVEL1_FIRST_WAYPOINT_SWITCH_RADIUS_XY,
                        LEVEL1_FIRST_WAYPOINT_SWITCH_RADIUS_Z,
                    )
                {
                    self.activate_group(12);
                    self.first_waypoint_switch_active = false;
                    self.source_slots[151].control_flags |= SOURCE_DISABLED_FLAG;
                    self.pending_source_slot_releases.push(151);
                }
                self.first_waypoint_switch_phase = self.first_waypoint_switch_phase.wrapping_add(1);
                self.source_slots[151].phase = self.first_waypoint_switch_phase;
            }
            _ => {}
        }

        let Some(index) = self.authored_allocations.iter().position(|allocation| {
            allocation.source_entity_index == source_slot
                && allocation.state == AuthoredAllocationState::Active
        }) else {
            return;
        };
        let authored_index = self.authored_allocations[index].authored_index;
        let phase = self.authored_allocations[index].phase;
        let mut target_group = None;
        let mut objective_switch = false;
        match authored_index {
            99 if self.objective_states[0] == 2 => {
                target_group = Some(2);
                objective_switch = true;
            }
            105 if self.objective_states[1] == 2 => {
                target_group = Some(3);
                objective_switch = true;
            }
            153 if self.objective_states[2] == 2 => {
                target_group = Some(5);
                objective_switch = true;
            }
            190 if self.objective_states[3] == 2 => {
                target_group = Some(6);
                objective_switch = true;
            }
            104 if phase & 7 == 0 => {
                let center = self.authored_allocations[index].position;
                if inside_switch_bounds(player.position, center, 10 << 8, 4_096) {
                    target_group = Some(7);
                }
            }
            231 if phase & 7 == 0 => {
                let center = self.authored_allocations[index].position;
                if !inside_switch_bounds(player.position, center, 52 << 8, 4_096) {
                    target_group = Some(8);
                }
            }
            232 if phase & 7 == 0 => {
                let center = self.authored_allocations[index].position;
                if inside_switch_bounds(player.position, center, 40 << 8, 4_096) {
                    target_group = Some(9);
                }
            }
            // Class-11/model-0 action 0: strict wrapping XY overlap, sampled
            // every eighth entity phase. Unlike the ordinary zone helpers it
            // has no altitude predicate and model 0 emits no activation sound.
            305 if phase & 7 == 0 => {
                let allocation = self.authored_allocations[index];
                let switch_half_width = i16::try_from(allocation.word_10 << 8)
                    .expect("Level 1 group-33 switch extent fits i16");
                if inside_exit_switch_xy(player.position, allocation.position, switch_half_width) {
                    target_group = Some(allocation.stage_tag);
                }
            }
            // Class-11/model-16 action 16 (`sub_6F480`): model-3 heads occupy
            // the family list while alive. Action-232 body nodes are omitted,
            // but pre-kill changes them to action 29, so the countdown cannot
            // begin until the complete dying chain has left the list.
            308 => {
                let model_three_present = self.actors.iter().any(|actor| {
                    actor.kind == Level1ActorKind::MultipartHead && actor.active && actor.life >= 0
                }) || self.multipart_segments.iter().any(|segment| {
                    segment.active
                        && segment.life >= 0
                        && segment.action != MULTIPART_SEGMENT_ACTION
                });
                if !model_three_present {
                    let timer = self.authored_allocations[index].state_timer;
                    if timer == 0 {
                        self.authored_allocations[index].state_timer = 16;
                    } else if timer == 1 {
                        target_group = Some(self.authored_allocations[index].stage_tag);
                        self.events.push(Level1MissionEvent::MissionSound {
                            sound_id: EXIT_SWITCH_SOUND,
                            source_entity_index: self.authored_allocations[index]
                                .source_entity_index,
                            position: self.authored_allocations[index].position,
                            random_state: self.global_rng,
                        });
                    } else {
                        self.authored_allocations[index].state_timer -= 1;
                    }
                }
            }
            // Action 17 waits on creature model 17. Level 1 authors no
            // model-17 creature, so the 16-tick release timer starts as
            // soon as this switch begins dispatching; Archer survival is
            // irrelevant.
            109 => {
                let timer = self.authored_allocations[index].state_timer;
                if timer == 0 {
                    self.authored_allocations[index].state_timer = 16;
                } else if timer == 1 {
                    target_group = Some(11);
                    self.events.push(Level1MissionEvent::MissionSound {
                        sound_id: EXIT_SWITCH_SOUND,
                        source_entity_index: self.authored_allocations[index].source_entity_index,
                        position: self.authored_allocations[index].position,
                        random_state: self.global_rng,
                    });
                } else {
                    self.authored_allocations[index].state_timer -= 1;
                }
            }
            LEVEL1_EXIT_SWITCH_FEATURE
                if !self.exit_sequence_active && !self.exit_sequence_complete && phase & 7 == 0 =>
            {
                let center = self.authored_allocations[index].position;
                let switch_half_width = self
                    .actors
                    .iter()
                    .find(|actor| actor.kind == Level1ActorKind::ExitSwitch)
                    .map_or(0, |actor| actor.collision_half_width);
                if inside_exit_switch_xy(player.position, center, switch_half_width) {
                    self.exit_sequence_active = true;
                    self.exit_sequence_phase = 0;
                    self.exit_sequence_timer = 0;
                    self.exit_player_action = EXIT_PLAYER_ACTION;
                    self.exit_target_source_entity_index = self
                        .actors
                        .iter()
                        .find(|actor| {
                            actor.kind == Level1ActorKind::ExitTarget
                                && actor.active
                                && actor.class == EXIT_TARGET_CLASS
                                && matches!(
                                    actor.model,
                                    EXIT_TARGET_MODEL_PRIMARY | EXIT_TARGET_MODEL_ALTERNATE
                                )
                        })
                        .map_or(0, |actor| actor.source_entity_index);
                    self.exit_target_valid = self.exit_target_source_entity_index != 0;
                    self.events.push(Level1MissionEvent::MissionSound {
                        sound_id: EXIT_SWITCH_SOUND,
                        source_entity_index: self.authored_allocations[index].source_entity_index,
                        position: self.authored_allocations[index].position,
                        random_state: self.global_rng,
                    });
                    self.events.push(Level1MissionEvent::ExitSequenceStarted);
                    if let Some(target) = self
                        .actors
                        .iter_mut()
                        .find(|actor| actor.kind == Level1ActorKind::ExitTarget)
                    {
                        target.visible = true;
                    }
                    self.mark_allocation_pending_delete(authored_index);
                }
            }
            _ => {}
        }

        if let Some(group) = target_group {
            self.activate_group(group);
            if objective_switch {
                self.objective_pause = 1;
            }
            self.mark_allocation_pending_delete(authored_index);
        }
        if let Some(allocation) = self
            .authored_allocations
            .iter_mut()
            .find(|allocation| allocation.authored_index == authored_index)
        {
            allocation.phase = allocation.phase.wrapping_add(1);
        }
    }

    fn activate_group(&mut self, group: i16) {
        // `sub_4A1E0` begins with `sub_49F90`: reclaim disabled slots, rebuild
        // the free stack, then scan authored records in ascending index order.
        self.cleanup_pending_allocations();
        self.source_pool.rebuild_free_stack();
        let activating = self
            .authored_allocations
            .iter()
            .enumerate()
            .filter_map(|(index, allocation)| {
                (allocation.activation_group == group
                    && allocation.state == AuthoredAllocationState::Dormant)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        for index in activating {
            let authored_index = self.authored_allocations[index].authored_index;
            let Some(source_entity_index) = self
                .source_pool
                .allocate(SourceOccupant::Authored(authored_index))
            else {
                // Every remc2 authored constructor returns immediately when
                // `NewEvent_4A050` cannot allocate. Slot zero is the sentinel,
                // never a degraded live allocation.
                continue;
            };
            self.authored_allocations[index].source_entity_index = source_entity_index;
            self.authored_allocations[index].state = AuthoredAllocationState::Active;
            let actor_shape = self
                .actors
                .iter()
                .find(|actor| actor.authored_index == authored_index)
                .map(|actor| (actor.class, actor.model, actor.action));
            let (class, model, action) = actor_shape.unwrap_or((
                u8::try_from(self.authored_allocations[index].kind).unwrap_or(0),
                0,
                0,
            ));
            let record = &mut self.source_slots[usize::from(source_entity_index)];
            record.active = true;
            record.class = class;
            record.model = model;
            record.action = action;
            record.phase = 0;
            record.entity_rng = source_entity_index.wrapping_add(self.global_rng as u16);
            record.control_flags = 8;
            let Some(actor) = self
                .actors
                .iter_mut()
                .find(|actor| actor.authored_index == authored_index)
            else {
                continue;
            };
            actor.source_entity_index = source_entity_index;
            actor.active = true;
            actor.owner_id = if actor.class == 11 {
                self.authored_allocations[index].stage_tag
            } else {
                source_entity_index.cast_signed()
            };
            if matches!(
                actor.kind,
                Level1ActorKind::Archer | Level1ActorKind::Firefly | Level1ActorKind::MultipartHead
            ) {
                let model_counter = &mut self.model_phase_counters[usize::from(actor.model)];
                actor.phase = *model_counter;
                *model_counter = model_counter.wrapping_add(1);
                let seed = source_entity_index.wrapping_add(self.global_rng as u16);
                actor.entity_rng = next_rng16(seed);
                let heading = ((actor.entity_rng & 0x07ff) as i16).wrapping_sub(1);
                actor.yaw = heading;
                actor.desired_heading = heading;
                actor.actual_speed = if actor.kind == Level1ActorKind::MultipartHead {
                    MULTIPART_INITIAL_SPEED
                } else {
                    actor.min_speed
                };
                if actor.kind == Level1ActorKind::Firefly {
                    // The Level 1 row-8 stage variable broadcasts subtype 19
                    // to kind 1 at destination (242,184). `sub_12330` replaces
                    // constructor action 153 before the first dispatch.
                    actor.action = FIREFLY_STAGE_ACTION;
                    actor.stage_var_1 = 8;
                    actor.stage_var_2 = 1;
                    actor.attack_substate = 0;
                    actor.state_value = 0;
                }
            }
            let record = &mut self.source_slots[usize::from(source_entity_index)];
            record.action = actor.action;
            record.phase = actor.phase;
            record.entity_rng = actor.entity_rng;
            let multipart_head = (actor.kind == Level1ActorKind::MultipartHead)
                .then_some((actor.authored_index, actor.source_entity_index));
            if let Some((head_authored_index, head_source_entity_index)) = multipart_head {
                self.construct_multipart_segments(head_authored_index, head_source_entity_index);
            }
        }
    }

    fn construct_multipart_segments(
        &mut self,
        head_authored_index: u16,
        head_source_entity_index: u16,
    ) {
        let Some(head) = self
            .actors
            .iter()
            .find(|actor| {
                actor.authored_index == head_authored_index
                    && actor.source_entity_index == head_source_entity_index
                    && actor.active
            })
            .copied()
        else {
            return;
        };
        let mut predecessor_source_entity_index = head_source_entity_index;
        for ordinal in 0..MULTIPART_SEGMENT_COUNT {
            let Some(source_entity_index) = self
                .source_pool
                .allocate(SourceOccupant::MultipartSegment(0))
            else {
                break;
            };
            let occupant = SourceOccupant::MultipartSegment(source_entity_index);
            self.source_pool
                .replace_occupant(source_entity_index, occupant);
            bind_dynamic_source_record(
                &mut self.source_slots,
                source_entity_index,
                occupant,
                self.global_rng,
            );
            let record = &mut self.source_slots[usize::from(source_entity_index)];
            // `qmemcpy` copies the head seed and family ID after the allocator
            // initialized the child slot, then the constructor overwrites the
            // animation ordinal and action.
            record.entity_rng = head.entity_rng;
            record.phase = u8::try_from(ordinal).expect("multipart ordinal fits u8");
            record.action = MULTIPART_SEGMENT_ACTION;

            let segment = Level1MultipartSegment {
                source_entity_index,
                head_authored_index,
                head_source_entity_index,
                predecessor_source_entity_index,
                successor_source_entity_index: 0,
                ordinal: u8::try_from(ordinal).expect("multipart ordinal fits u8"),
                class: MISSION_ACTOR_CLASS,
                model: MULTIPART_MODEL,
                action: MULTIPART_SEGMENT_ACTION,
                owner_id: head_source_entity_index.cast_signed(),
                position: head.position,
                life: MULTIPART_LIFE,
                pending_damage: 0,
                pending_attacker: 0,
                yaw: head.yaw,
                pitch: head.yaw,
                phase: u8::try_from(ordinal).expect("multipart ordinal fits u8"),
                trailing_distance: MULTIPART_SEGMENT_TRAILING_DISTANCES[ordinal],
                collision_half_width: MULTIPART_SEGMENT_COLLISION_HALF_WIDTHS[ordinal],
                collision_half_height: MULTIPART_SEGMENT_HALF_HEIGHTS[ordinal],
                render_parameter_index: 89
                    + u16::try_from(ordinal).expect("multipart ordinal fits u16"),
                base_tmap: MULTIPART_SEGMENT_BASE_TMAP,
                render_mode: MULTIPART_SEGMENT_RENDER_MODE,
                logical_height: MULTIPART_SEGMENT_LOGICAL_HEIGHTS[ordinal],
                active: true,
                visible: true,
            };
            if predecessor_source_entity_index == head_source_entity_index {
                if let Some(head) = self
                    .actors
                    .iter_mut()
                    .find(|actor| actor.authored_index == head_authored_index)
                {
                    head.multipart_first_segment_index = source_entity_index;
                }
            } else if let Some(predecessor) = self
                .multipart_segments
                .iter_mut()
                .find(|segment| segment.source_entity_index == predecessor_source_entity_index)
            {
                predecessor.successor_source_entity_index = source_entity_index;
            }
            self.multipart_segments.push(segment);
            predecessor_source_entity_index = source_entity_index;
        }
    }

    fn advance_firefly_stage_rows(&mut self) {
        // `UpdateEntities_57730` runs `sub_12500` for stage-bound creatures
        // before their ordinary per-entity action dispatch. Level 1 row 8 is
        // stage type 1: reaching both signed, wrapping axes within 2,048 clears
        // the stage binding and selects model action `8 * 19 + 1 == 153`.
        // Doing this in `begin_source_turn` lets action 153 dispatch on the
        // same source turn, matching that source ordering.
        for actor in &mut self.actors {
            if !actor.active
                || actor.kind != Level1ActorKind::Firefly
                || actor.stage_var_1 != 8
                || actor.stage_var_2 != 1
            {
                continue;
            }
            let delta_x = i32::from(
                FIREFLY_STAGE_TARGET
                    .x
                    .wrapping_sub(actor.position.x)
                    .cast_signed(),
            )
            .abs();
            let delta_y = i32::from(
                FIREFLY_STAGE_TARGET
                    .y
                    .wrapping_sub(actor.position.y)
                    .cast_signed(),
            )
            .abs();
            if delta_x > 2_048 || delta_y > 2_048 {
                continue;
            }
            actor.stage_var_1 = 0;
            actor.stage_var_2 = 0;
            actor.linked_entity_index = 0;
            actor.action = FIREFLY_ROAM_ACTION;
        }
    }

    fn mark_allocation_pending_delete(&mut self, authored_index: u16) {
        let source_slot = if let Some(allocation) = self
            .authored_allocations
            .iter_mut()
            .find(|allocation| allocation.authored_index == authored_index)
            && allocation.state == AuthoredAllocationState::Active
        {
            allocation.state = AuthoredAllocationState::PendingDelete;
            allocation.source_entity_index
        } else {
            0
        };
        if source_slot != 0 {
            self.source_slots[usize::from(source_slot)].control_flags |= SOURCE_DISABLED_FLAG;
        }
    }

    fn cleanup_pending_allocations(&mut self) {
        for source_slot in self.pending_source_slot_releases.drain(..) {
            self.source_pool.disable_slot(source_slot);
        }
        for allocation in &mut self.authored_allocations {
            if allocation.state != AuthoredAllocationState::PendingDelete {
                continue;
            }
            if allocation.source_entity_index != 0 {
                self.source_pool
                    .disable_slot(allocation.source_entity_index);
            }
        }
        self.reclaimed_imported_source_slots.extend(
            self.source_pool
                .occupants
                .iter()
                .zip(self.source_pool.disabled.iter())
                .enumerate()
                .filter_map(|(index, (occupant, disabled))| {
                    (*disabled && *occupant == Some(SourceOccupant::ImportedLive))
                        .then(|| u16::try_from(index).expect("source pool index fits u16"))
                }),
        );
        let reclaimed = self.source_pool.reclaim_disabled_ascending();
        for source_slot in reclaimed {
            clear_source_record(&mut self.source_slots[usize::from(source_slot)]);
        }
        self.multipart_segments.retain(|segment| segment.active);
        let active_sphere_slots = self
            .mana_spheres
            .iter()
            .filter(|sphere| sphere.active && !sphere.pending_delete)
            .map(|sphere| sphere.source_entity_index)
            .collect::<Vec<_>>();
        self.mana_sphere_map_chains
            .retain_entities(&active_sphere_slots)
            .expect("qualified mana sphere source slots fit the map chain");
        self.mana_spheres
            .retain(|sphere| sphere.active && !sphere.pending_delete);
        if self
            .mana_carrier
            .as_ref()
            .is_some_and(|carrier| !carrier.active)
        {
            self.mana_carrier = None;
        }
        for allocation in &mut self.authored_allocations {
            if allocation.state != AuthoredAllocationState::PendingDelete {
                continue;
            }
            allocation.source_entity_index = 0;
            allocation.state = AuthoredAllocationState::Released;
        }
    }

    #[cfg(test)]
    fn allocation_slot(&self, authored_index: u16) -> Option<u16> {
        self.authored_allocations
            .iter()
            .find(|allocation| allocation.authored_index == authored_index)
            .map(|allocation| allocation.source_entity_index)
    }

    fn count_alive(&self, kind: Level1ActorKind) -> usize {
        self.actors
            .iter()
            .filter(|actor| actor.kind == kind && actor.active && !actor.collected)
            .count()
    }

    fn allocate_effect_source(
        &mut self,
        occupant: fn(u32) -> SourceOccupant,
    ) -> Option<(u32, u16)> {
        let id = self.next_effect_id;
        let occupant = occupant(id);
        let slot = self.source_pool.allocate(occupant)?;
        bind_dynamic_source_record(&mut self.source_slots, slot, occupant, self.global_rng);
        self.next_effect_id = self.next_effect_id.wrapping_add(1).max(1);
        Some((id, slot))
    }

    fn allocate_mana_sphere_source(&mut self) -> Option<u16> {
        let slot = self.source_pool.allocate(SourceOccupant::ManaSphere(0))?;
        let occupant = SourceOccupant::ManaSphere(u32::from(slot));
        self.source_pool.replace_occupant(slot, occupant);
        bind_dynamic_source_record(&mut self.source_slots, slot, occupant, self.global_rng);
        Some(slot)
    }

    fn allocate_mana_carrier_source(&mut self) -> Option<u16> {
        let slot = self.source_pool.allocate(SourceOccupant::ManaCarrier(0))?;
        let occupant = SourceOccupant::ManaCarrier(u32::from(slot));
        self.source_pool.replace_occupant(slot, occupant);
        bind_dynamic_source_record(&mut self.source_slots, slot, occupant, self.global_rng);
        Some(slot)
    }

    fn allocate_death_burst_source(&mut self) -> Option<u16> {
        let slot = self.source_pool.allocate(SourceOccupant::DeathBurst(0))?;
        let occupant = SourceOccupant::DeathBurst(u32::from(slot));
        self.source_pool.replace_occupant(slot, occupant);
        bind_dynamic_source_record(&mut self.source_slots, slot, occupant, self.global_rng);
        Some(slot)
    }
}

fn actor_from_feature<W: FlightWorld>(
    feature: AuthoredFeature,
    kind: Level1ActorKind,
    class: u8,
    model: u8,
    action: u8,
    life: i32,
    world: &W,
) -> Level1Actor {
    let mut position = feature.world_position(0);
    position.z = world.ground_height(position);
    let (render_parameter_index, base_tmap, render_mode, logical_height) = match kind {
        Level1ActorKind::Archer => (
            Some(ARCHER_RENDER_INDEX),
            Some(ARCHER_BASE_TMAP),
            Some(ARCHER_RENDER_MODE),
            Some(ARCHER_LOGICAL_HEIGHT),
        ),
        Level1ActorKind::Firefly => (
            Some(FIREFLY_RENDER_INDEX),
            Some(FIREFLY_BASE_TMAP),
            Some(FIREFLY_RENDER_MODE),
            Some(FIREFLY_LOGICAL_HEIGHT),
        ),
        Level1ActorKind::CastleSpellJar | Level1ActorKind::SpeedSpellJar => (
            Some(SPELL_JAR_RENDER_INDEX),
            Some(SPELL_JAR_BASE_TMAP),
            Some(SPELL_JAR_RENDER_MODE),
            Some(SPELL_JAR_LOGICAL_HEIGHT),
        ),
        Level1ActorKind::ExitTarget => (
            Some(EXIT_TARGET_RENDER_INDEX),
            Some(EXIT_TARGET_BASE_TMAP),
            Some(EXIT_TARGET_RENDER_MODE),
            Some(EXIT_TARGET_LOGICAL_HEIGHT),
        ),
        Level1ActorKind::ExitSwitch => (None, None, None, None),
        Level1ActorKind::MultipartHead => (
            Some(MULTIPART_HEAD_RENDER_INDEX),
            Some(MULTIPART_HEAD_BASE_TMAP),
            Some(MULTIPART_HEAD_RENDER_MODE),
            Some(MULTIPART_HEAD_LOGICAL_HEIGHT),
        ),
        Level1ActorKind::ExperienceScroll => (
            Some(EXPERIENCE_SCROLL_RENDER_INDEX),
            Some(EXPERIENCE_SCROLL_BASE_TMAP),
            Some(EXPERIENCE_SCROLL_RENDER_MODE),
            Some(EXPERIENCE_SCROLL_LOGICAL_HEIGHT),
        ),
    };
    Level1Actor {
        authored_index: feature.index,
        source_entity_index: 0,
        kind,
        class,
        model,
        action,
        activation_group: feature.activation_group,
        owner_id: feature.stage_tag,
        position,
        max_life: life,
        life,
        pending_damage: 0,
        pending_attacker: 0,
        entity_rng: 0,
        // Source RNG initialization is applied at activation, when the source
        // entity-pool slot and current global RNG are both known.
        yaw: 0,
        desired_heading: 0,
        min_speed: match kind {
            Level1ActorKind::Archer => ARCHER_MIN_SPEED,
            Level1ActorKind::Firefly => FIREFLY_MIN_SPEED,
            Level1ActorKind::MultipartHead => MULTIPART_MIN_SPEED,
            Level1ActorKind::CastleSpellJar | Level1ActorKind::SpeedSpellJar => 0,
            Level1ActorKind::ExitTarget
            | Level1ActorKind::ExitSwitch
            | Level1ActorKind::ExperienceScroll => 0,
        },
        actual_speed: 0,
        max_speed: match kind {
            Level1ActorKind::Firefly => FIREFLY_MAX_SPEED,
            Level1ActorKind::MultipartHead => MULTIPART_MAX_SPEED,
            _ => 0,
        },
        target_entity_index: 0,
        linked_entity_index: 0,
        multipart_first_segment_index: 0,
        movement_blocked: false,
        stage_var_1: if kind == Level1ActorKind::Firefly {
            8
        } else {
            0
        },
        stage_var_2: if kind == Level1ActorKind::Firefly {
            1
        } else {
            0
        },
        attack_substate: 0,
        state_value: 0,
        phase: 0,
        state_timer: 0,
        animation_frame: 0,
        collision_half_width: match kind {
            Level1ActorKind::Archer => ARCHER_COLLISION_HALF_WIDTH,
            Level1ActorKind::Firefly => 85,
            Level1ActorKind::MultipartHead => MULTIPART_HEAD_COLLISION_HALF_WIDTH,
            Level1ActorKind::ExperienceScroll => EXPERIENCE_SCROLL_COLLISION_HALF_WIDTH,
            Level1ActorKind::CastleSpellJar | Level1ActorKind::SpeedSpellJar => 768,
            Level1ActorKind::ExitTarget => 0,
            Level1ActorKind::ExitSwitch => {
                i16::try_from(feature.word_10 << 8).expect("switch extent fits i16")
            }
        },
        collision_vertical_offset: match kind {
            Level1ActorKind::Archer => ARCHER_COLLISION_VERTICAL_OFFSET,
            Level1ActorKind::Firefly => 100,
            Level1ActorKind::MultipartHead => MULTIPART_HEAD_COLLISION_VERTICAL_OFFSET,
            Level1ActorKind::ExperienceScroll => EXPERIENCE_SCROLL_COLLISION_VERTICAL_OFFSET,
            Level1ActorKind::CastleSpellJar | Level1ActorKind::SpeedSpellJar => 75,
            Level1ActorKind::ExitTarget | Level1ActorKind::ExitSwitch => 0,
        },
        collision_half_height: match kind {
            Level1ActorKind::Archer => ARCHER_COLLISION_HALF_HEIGHT,
            Level1ActorKind::Firefly => 51,
            Level1ActorKind::MultipartHead => MULTIPART_HEAD_COLLISION_HALF_HEIGHT,
            Level1ActorKind::ExperienceScroll => EXPERIENCE_SCROLL_COLLISION_HALF_HEIGHT,
            Level1ActorKind::CastleSpellJar | Level1ActorKind::SpeedSpellJar => 1_280,
            Level1ActorKind::ExitTarget | Level1ActorKind::ExitSwitch => 0,
        },
        render_parameter_index,
        base_tmap,
        render_mode,
        logical_height,
        active: false,
        collected: false,
        visible: !matches!(
            kind,
            Level1ActorKind::ExitTarget | Level1ActorKind::ExitSwitch
        ),
    }
}

fn step_archer_roam<W: FlightWorld>(
    actor: &mut Level1Actor,
    world: &W,
    player: &FlightState,
    actor_views: &[Level1Actor],
) {
    move_archer(actor, world);
    if actor.phase % ARCHER_DECISION_CADENCE != 0 {
        return;
    }

    actor.entity_rng = next_rng16(actor.entity_rng);
    let direction_selector = actor.entity_rng;
    actor.entity_rng = next_rng16(actor.entity_rng);
    let sign = if direction_selector % 157 < 79 { -1 } else { 1 };
    let delta =
        i16::try_from((actor.entity_rng & 0x00ff) + 85).expect("Archer wander delta fits i16");
    actor.desired_heading = actor.desired_heading.wrapping_add(sign * delta) & 0x07ff;

    if actor.phase % ARCHER_ACQUISITION_CADENCE != 0 {
        return;
    }

    let range_squared = u32::from(ARCHER_RANGE).pow(2);
    if squared_wrapping_xy_distance(actor.position, player.position) <= range_squared {
        let player_heading = heading_for_positions(actor.position, player.position);
        if angle_distance(i32::from(actor.yaw), player_heading) < ARCHER_VIEW_HALF_ANGLE {
            actor.target_entity_index = player.entity_index;
            actor.action = 34;
            enter_archer_attack_pose(actor);
            return;
        }
    }

    let linked = actor_views
        .iter()
        .filter(|candidate| {
            candidate.kind == Level1ActorKind::Archer
                && candidate.active
                && candidate.life >= 0
                && candidate.source_entity_index != actor.source_entity_index
                && candidate.linked_entity_index == 0
        })
        .filter_map(|candidate| {
            let distance = squared_wrapping_xy_distance(actor.position, candidate.position);
            let heading = heading_for_positions(actor.position, candidate.position);
            (distance <= range_squared
                && angle_distance(i32::from(actor.yaw), heading) < ARCHER_VIEW_HALF_ANGLE)
                .then_some((distance, candidate.source_entity_index))
        })
        .min_by_key(|(distance, _)| *distance);
    if let Some((_, source_entity_index)) = linked {
        actor.linked_entity_index = source_entity_index;
        actor.action = 35;
    }
}

fn step_archer_attack<W: FlightWorld>(
    actor: &mut Level1Actor,
    world: &W,
    player: &FlightState,
    projectile_id: u32,
) -> Option<ArcherArrowProjectile> {
    move_archer(actor, world);
    if actor.target_entity_index == 0 || actor.target_entity_index != player.entity_index {
        leave_archer_attack_pose(actor);
        return None;
    }

    if actor.phase & 3 == 0 {
        actor.desired_heading =
            i16::try_from(heading_for_positions(actor.position, player.position))
                .expect("Archer heading is 11-bit");
    }
    if actor.phase % ARCHER_DECISION_CADENCE != 0 {
        return None;
    }
    if distance_3d_wrapped(actor.position, player.position) >= u32::from(ARCHER_RANGE) {
        leave_archer_attack_pose(actor);
        return None;
    }

    let horizontal = legacy_sqrt(squared_wrapping_xy_distance(
        actor.position,
        player.position,
    ));
    let dz = i32::from(player.position.z.wrapping_sub(actor.position.z));
    let mut position = actor.position;
    position.z = position.z.wrapping_add(actor.collision_half_height / 2);
    Some(ArcherArrowProjectile {
        id: projectile_id,
        // The mission effect allocator is deliberately separate from the
        // source pool until all Level 1 entity families share one dispatcher.
        source_entity_index: 0,
        owner_entity_index: actor.source_entity_index,
        target_entity_index: player.entity_index,
        position,
        yaw: i16::try_from(heading_for_positions(actor.position, player.position))
            .expect("Archer arrow yaw is 11-bit"),
        pitch: pitch_for_delta(
            i32::try_from(horizontal).expect("legacy XY distance fits i32"),
            dz,
        ),
        speed: ARCHER_ARROW_SPEED,
        life_ticks: ARCHER_ARROW_LIFETIME_TICKS,
        payload: ARCHER_ARROW_PAYLOAD,
        sound_initialized: false,
        active: true,
    })
}

fn enter_archer_attack_pose(actor: &mut Level1Actor) {
    actor.entity_rng = next_rng16(actor.entity_rng);
    actor.actual_speed = 0;
    if actor.entity_rng % 20 <= 10 {
        actor.render_parameter_index = Some(ARCHER_ATTACK_RENDER_INDEX_B);
        actor.base_tmap = Some(ARCHER_ATTACK_BASE_TMAP_B);
        actor.logical_height = Some(ARCHER_ATTACK_LOGICAL_HEIGHT_B);
    } else {
        actor.render_parameter_index = Some(ARCHER_ATTACK_RENDER_INDEX_A);
        actor.base_tmap = Some(ARCHER_ATTACK_BASE_TMAP_A);
        actor.logical_height = Some(ARCHER_LOGICAL_HEIGHT);
    }
}

fn leave_archer_attack_pose(actor: &mut Level1Actor) {
    actor.action = ARCHER_ACTION;
    actor.actual_speed = actor.min_speed;
    actor.target_entity_index = 0;
    actor.render_parameter_index = Some(ARCHER_RENDER_INDEX);
    actor.base_tmap = Some(ARCHER_BASE_TMAP);
    actor.logical_height = Some(ARCHER_LOGICAL_HEIGHT);
}

fn step_archer_linked<W: FlightWorld>(
    actor: &mut Level1Actor,
    world: &W,
    actor_views: &[Level1Actor],
) {
    if actor.linked_entity_index == 0 {
        actor.action = ARCHER_ACTION;
        return;
    }
    let Some(peer) = actor_views.iter().find(|candidate| {
        candidate.source_entity_index == actor.linked_entity_index
            && candidate.active
            && candidate.life >= 0
            && candidate.class == actor.class
            && candidate.model == actor.model
    }) else {
        actor.linked_entity_index = 0;
        actor.action = ARCHER_ACTION;
        return;
    };

    move_archer(actor, world);
    if actor.phase % ARCHER_DECISION_CADENCE != 0 {
        return;
    }

    match peer.action.wrapping_sub(32) {
        0 | 1 => {}
        2 => {
            actor.target_entity_index = peer.target_entity_index;
            actor.linked_entity_index = 0;
            actor.action = 34;
            enter_archer_attack_pose(actor);
            return;
        }
        3 => {
            actor.linked_entity_index = peer.linked_entity_index;
        }
        6 => {
            actor.target_entity_index = peer.target_entity_index;
            actor.linked_entity_index = 0;
            actor.action = 38;
            return;
        }
        _ => {
            actor.linked_entity_index = 0;
            actor.action = ARCHER_ACTION;
            return;
        }
    }

    let Some(linked) = actor_views
        .iter()
        .find(|candidate| candidate.source_entity_index == actor.linked_entity_index)
    else {
        actor.linked_entity_index = 0;
        actor.action = ARCHER_ACTION;
        return;
    };
    actor.desired_heading = i16::try_from(heading_for_positions(actor.position, linked.position))
        .expect("Archer linked heading is 11-bit");
    if let Some(nearby) = actor_views.iter().find(|candidate| {
        candidate.kind == Level1ActorKind::Archer
            && candidate.active
            && candidate.source_entity_index != actor.source_entity_index
            && candidate.owner_id != actor.owner_id
            && i32::from(
                candidate
                    .position
                    .x
                    .wrapping_sub(actor.position.x)
                    .cast_signed(),
            )
            .abs()
                < 256
            && i32::from(
                candidate
                    .position
                    .y
                    .wrapping_sub(actor.position.y)
                    .cast_signed(),
            )
            .abs()
                < 256
    }) {
        actor.desired_heading =
            i16::try_from(heading_for_positions(nearby.position, actor.position))
                .expect("Archer separation heading is 11-bit");
    }
    actor.actual_speed = peer.actual_speed.wrapping_add(peer.max_speed);
}

fn step_firefly_stage_patrol<W: FlightWorld>(
    actor: &mut Level1Actor,
    world: &W,
    actor_views: &[Level1Actor],
) {
    move_firefly(actor, world);
    if actor.phase & 7 != 0 {
        return;
    }

    actor.desired_heading =
        i16::try_from(heading_for_positions(actor.position, FIREFLY_STAGE_TARGET))
            .expect("Firefly heading is 11-bit");
    if actor.phase & 0x3f == 0 {
        apply_creature_wander(actor);
    }
    apply_same_model_separation(actor, actor_views, 85);
}

fn step_firefly_roam<W: FlightWorld>(
    actor: &mut Level1Actor,
    world: &W,
    player: &FlightState,
    actor_views: &[Level1Actor],
) {
    move_firefly(actor, world);
    if actor.phase % FIREFLY_DECISION_CADENCE != 0 {
        return;
    }
    apply_creature_wander(actor);

    let range_squared = u32::from(FIREFLY_RANGE).pow(2);
    let player_heading = heading_for_positions(actor.position, player.position);
    if squared_wrapping_xy_distance(actor.position, player.position) <= range_squared
        && angle_distance(i32::from(actor.yaw), player_heading) < FIREFLY_VIEW_HALF_ANGLE
    {
        actor.target_entity_index = player.entity_index;
        actor.linked_entity_index = 0;
        actor.stage_var_2 = 10;
        actor.action = FIREFLY_ATTACK_ACTION;
        actor.attack_substate = 0;
        return;
    }

    let linked = actor_views
        .iter()
        .filter(|candidate| {
            candidate.kind == Level1ActorKind::Firefly
                && candidate.active
                && candidate.life >= 0
                && candidate.source_entity_index != actor.source_entity_index
                && candidate.linked_entity_index == 0
        })
        .filter_map(|candidate| {
            let distance = squared_wrapping_xy_distance(actor.position, candidate.position);
            let heading = heading_for_positions(actor.position, candidate.position);
            (distance <= range_squared
                && angle_distance(i32::from(actor.yaw), heading) < FIREFLY_VIEW_HALF_ANGLE)
                .then_some((distance, candidate.source_entity_index))
        })
        .min_by_key(|(distance, _)| *distance);
    if let Some((_, source_entity_index)) = linked {
        actor.linked_entity_index = source_entity_index;
        actor.action = FIREFLY_LINKED_ACTION;
    }
}

fn step_firefly_linked<W: FlightWorld>(
    actor: &mut Level1Actor,
    world: &W,
    actor_views: &[Level1Actor],
) {
    move_firefly(actor, world);
    if actor.phase % FIREFLY_DECISION_CADENCE != 0 {
        return;
    }
    let Some(peer) = actor_views.iter().find(|candidate| {
        candidate.source_entity_index == actor.linked_entity_index
            && candidate.active
            && candidate.life >= 0
            && candidate.class == actor.class
            && candidate.model == actor.model
    }) else {
        leave_firefly_attack(actor);
        return;
    };

    match peer.action.wrapping_sub(152) {
        0 | 1 => {}
        2 => {
            actor.target_entity_index = peer.target_entity_index;
            actor.linked_entity_index = 0;
            actor.stage_var_2 = 10;
            actor.action = FIREFLY_ATTACK_ACTION;
            actor.attack_substate = 0;
            return;
        }
        3 => actor.linked_entity_index = peer.linked_entity_index,
        _ => {
            leave_firefly_attack(actor);
            return;
        }
    }
    let Some(linked) = actor_views
        .iter()
        .find(|candidate| candidate.source_entity_index == actor.linked_entity_index)
    else {
        leave_firefly_attack(actor);
        return;
    };
    actor.desired_heading = i16::try_from(heading_for_positions(actor.position, linked.position))
        .expect("Firefly linked heading is 11-bit");
    apply_same_model_separation(actor, actor_views, 256);
    actor.actual_speed = peer.actual_speed.wrapping_add(peer.max_speed);
}

fn step_firefly_attack<W: FlightWorld>(
    actor: &mut Level1Actor,
    world: &W,
    player: &FlightState,
    actor_views: &[Level1Actor],
    projectile_id: u32,
) -> (Option<FireflyFireballProjectile>, Option<u16>, i32) {
    move_firefly(actor, world);
    if actor.target_entity_index != player.entity_index || actor.target_entity_index == 0 {
        leave_firefly_attack(actor);
        return (None, None, 0);
    }

    let mut projectile = None;
    let mut charge_sound = None;
    let mut contact_damage = 0;
    loop {
        match actor.attack_substate {
            0 => {
                actor.actual_speed = FIREFLY_MIN_SPEED;
                actor.attack_substate = 1;
            }
            1 => {
                actor.entity_rng = next_rng16(actor.entity_rng);
                let orbit_yaw = player.yaw.wrapping_sub(256).wrapping_add(
                    i16::try_from((u32::from(actor.entity_rng % 90) << 11) / 360)
                        .expect("Firefly orbit delta fits i16"),
                ) & 0x07ff;
                let mut point = player.position;
                move_entity(&mut point, orbit_yaw, 0, 2_048);
                if distance_3d_wrapped(actor.position, point) <= 1_280 {
                    actor.attack_substate = 2;
                    continue;
                }
                actor.desired_heading = i16::try_from(heading_for_positions(actor.position, point))
                    .expect("Firefly orbit heading is 11-bit");
                if actor.phase & 3 == 0 {
                    apply_same_model_separation(actor, actor_views, 85);
                }
                break;
            }
            2 => {
                actor.actual_speed = FIREFLY_MAX_SPEED;
                actor.entity_rng = next_rng16(actor.entity_rng);
                actor.state_value =
                    i32::from(player.position.z) + i32::from(actor.entity_rng & 0x03ff);
                actor.attack_substate = 3;
            }
            3 => {
                actor.desired_heading =
                    i16::try_from(heading_for_positions(actor.position, player.position))
                        .expect("Firefly tracking heading is 11-bit");
                if actor.phase & 3 != 0 {
                    break;
                }
                apply_same_model_separation(actor, actor_views, 85);
                let mut point = player.position;
                move_entity(&mut point, player.yaw, 0, 2_048);
                if distance_3d_wrapped(actor.position, point) > 1_280 {
                    actor.attack_substate = 0;
                    break;
                }
                actor.entity_rng = next_rng16(actor.entity_rng);
                let maneuver = actor.entity_rng % 287;
                if maneuver == 0 {
                    actor.attack_substate = 4;
                } else if maneuver & 0x1f == 0 {
                    actor.attack_substate = 7;
                }
                if maneuver & 3 == 0 {
                    move_firefly_altitude_toward(actor, actor.state_value);
                }
                break;
            }
            4 => {
                actor.actual_speed = FIREFLY_MIN_SPEED;
                actor.attack_substate = 5;
            }
            5 => {
                if actor.phase & 3 == 0 {
                    actor.desired_heading =
                        i16::try_from(heading_for_positions(actor.position, player.position))
                            .expect("Firefly firing heading is 11-bit");
                    apply_same_model_separation(actor, actor_views, 85);
                }
                if actor.phase % FIREFLY_DECISION_CADENCE == 0 {
                    if distance_3d_wrapped(actor.position, player.position)
                        < u32::from(FIREFLY_RANGE)
                    {
                        projectile = Some(spawn_firefly_fireball(
                            actor,
                            player.position,
                            projectile_id,
                        ));
                    } else {
                        actor.attack_substate = 6;
                    }
                }
                break;
            }
            6 => {
                leave_firefly_attack(actor);
                break;
            }
            7 => {
                actor.actual_speed = FIREFLY_MIN_SPEED.saturating_mul(3);
                actor.entity_rng = next_rng16(actor.entity_rng);
                charge_sound = Some(43 + (actor.entity_rng & 1));
                actor.state_value = 24;
                actor.attack_substate = 8;
            }
            8 | 9 => {
                actor.state_value -= 1;
                if actor.state_value == 0 {
                    actor.attack_substate = 0;
                    break;
                }
                if actor.phase & 3 == 0 && actor.state_value > 16 {
                    actor.desired_heading =
                        i16::try_from(heading_for_positions(actor.position, player.position))
                            .expect("Firefly charge heading is 11-bit");
                    apply_same_model_separation(actor, actor_views, 85);
                }
                move_firefly_altitude_toward(actor, i32::from(player.position.z));
                if actor.attack_substate == 8
                    && distance_3d_wrapped(actor.position, player.position) < FIREFLY_CONTACT_RANGE
                {
                    contact_damage = FIREFLY_CONTACT_DAMAGE;
                    actor.attack_substate = 9;
                }
                if actor.phase % FIREFLY_DECISION_CADENCE == 0
                    && distance_3d_wrapped(actor.position, player.position)
                        >= u32::from(FIREFLY_RANGE)
                {
                    actor.attack_substate = 6;
                }
                break;
            }
            _ => {
                actor.attack_substate = 0;
                break;
            }
        }
    }
    (projectile, charge_sound, contact_damage)
}

fn spawn_firefly_fireball(
    actor: &Level1Actor,
    target: Position,
    id: u32,
) -> FireflyFireballProjectile {
    let angle_origin = actor.position;
    let horizontal = legacy_sqrt(squared_wrapping_xy_distance(angle_origin, target));
    let yaw = i16::try_from(heading_for_positions(angle_origin, target))
        .expect("Firefly fireball heading is 11-bit");
    let pitch = pitch_for_delta(
        i32::try_from(horizontal).expect("legacy XY distance fits i32"),
        i32::from(target.z.wrapping_sub(angle_origin.z)),
    );
    let mut position = actor.position;
    position.z = position.z.wrapping_add(actor.collision_half_height);
    FireflyFireballProjectile {
        id,
        source_entity_index: 0,
        owner_entity_index: actor.source_entity_index,
        target_entity_index: actor.target_entity_index,
        position,
        yaw,
        pitch,
        speed: FIREFLY_FIREBALL_SPEED,
        life_ticks: FIREFLY_FIREBALL_LIFETIME_TICKS,
        payload: FIREFLY_FIREBALL_PAYLOAD,
        active: true,
    }
}

fn leave_firefly_attack(actor: &mut Level1Actor) {
    actor.action = FIREFLY_ROAM_ACTION;
    actor.actual_speed = FIREFLY_MIN_SPEED;
    actor.target_entity_index = 0;
    actor.linked_entity_index = 0;
    actor.stage_var_2 = 1;
    actor.attack_substate = 0;
}

fn step_multipart_roam<W: FlightWorld>(
    actor: &mut Level1Actor,
    world: &W,
    player: &FlightState,
    actor_views: &[Level1Actor],
) {
    move_multipart_head(actor, world);
    if actor.phase % MULTIPART_DECISION_CADENCE != 0 {
        return;
    }
    apply_creature_wander(actor);
    apply_same_model_separation(
        actor,
        actor_views,
        MULTIPART_HEAD_COLLISION_HALF_WIDTH.into(),
    );

    let range_squared = u32::from(MULTIPART_RANGE).pow(2);
    let player_heading = heading_for_positions(actor.position, player.position);
    if squared_wrapping_xy_distance(actor.position, player.position) <= range_squared
        && angle_distance(i32::from(actor.yaw), player_heading) < MULTIPART_VIEW_HALF_ANGLE
    {
        actor.target_entity_index = player.entity_index;
        actor.linked_entity_index = 0;
        actor.action = MULTIPART_ATTACK_ACTION;
        return;
    }

    let linked = actor_views
        .iter()
        .filter(|candidate| {
            candidate.kind == Level1ActorKind::MultipartHead
                && candidate.active
                && candidate.life >= 0
                && candidate.source_entity_index != actor.source_entity_index
                && candidate.linked_entity_index == 0
        })
        .filter_map(|candidate| {
            let distance = squared_wrapping_xy_distance(actor.position, candidate.position);
            let heading = heading_for_positions(actor.position, candidate.position);
            (distance <= range_squared
                && angle_distance(i32::from(actor.yaw), heading) < MULTIPART_VIEW_HALF_ANGLE)
                .then_some((distance, candidate.source_entity_index))
        })
        .min_by_key(|(distance, _)| *distance);
    if let Some((_, source_entity_index)) = linked {
        actor.linked_entity_index = source_entity_index;
        actor.action = MULTIPART_LINKED_ACTION;
    }
}

fn step_multipart_attack<W: FlightWorld>(
    actor: &mut Level1Actor,
    world: &W,
    player: &FlightState,
    actor_views: &[Level1Actor],
    projectile_id: u32,
) -> Option<FireflyFireballProjectile> {
    move_multipart_head(actor, world);
    if actor.target_entity_index == 0 || actor.target_entity_index != player.entity_index {
        actor.target_entity_index = 0;
        actor.action = MULTIPART_ROAM_ACTION;
        return None;
    }
    if actor.phase & 3 == 0 {
        actor.desired_heading =
            i16::try_from(heading_for_positions(actor.position, player.position))
                .expect("multipart heading is 11-bit");
        apply_same_model_separation(
            actor,
            actor_views,
            MULTIPART_HEAD_COLLISION_HALF_WIDTH.into(),
        );
    }
    if actor.phase % MULTIPART_DECISION_CADENCE != 0 {
        return None;
    }
    if distance_3d_wrapped(actor.position, player.position) >= u32::from(MULTIPART_RANGE) {
        actor.target_entity_index = 0;
        actor.action = MULTIPART_ROAM_ACTION;
        return None;
    }
    Some(spawn_multipart_fireball(
        actor,
        player.position,
        projectile_id,
    ))
}

fn spawn_multipart_fireball(
    actor: &Level1Actor,
    target: Position,
    id: u32,
) -> FireflyFireballProjectile {
    let horizontal = legacy_sqrt(squared_wrapping_xy_distance(actor.position, target));
    let yaw = i16::try_from(heading_for_positions(actor.position, target))
        .expect("multipart fireball heading is 11-bit");
    let pitch = pitch_for_delta(
        i32::try_from(horizontal).expect("multipart fireball distance fits i32"),
        i32::from(target.z.wrapping_sub(actor.position.z)),
    );
    let mut position = actor.position;
    position.z = position
        .z
        .wrapping_add(MULTIPART_HEAD_COLLISION_HALF_HEIGHT);
    FireflyFireballProjectile {
        id,
        source_entity_index: 0,
        owner_entity_index: actor.source_entity_index,
        target_entity_index: actor.target_entity_index,
        position,
        yaw,
        pitch,
        speed: FIREFLY_FIREBALL_SPEED,
        life_ticks: FIREFLY_FIREBALL_LIFETIME_TICKS,
        payload: FIREFLY_FIREBALL_PAYLOAD,
        active: true,
    }
}

fn step_multipart_linked<W: FlightWorld>(
    actor: &mut Level1Actor,
    world: &W,
    actor_views: &[Level1Actor],
) {
    move_multipart_head(actor, world);
    if actor.phase % MULTIPART_DECISION_CADENCE != 0 {
        return;
    }
    let Some(peer) = actor_views.iter().find(|candidate| {
        candidate.source_entity_index == actor.linked_entity_index
            && candidate.kind == Level1ActorKind::MultipartHead
            && candidate.active
            && candidate.life >= 0
    }) else {
        actor.linked_entity_index = 0;
        actor.action = MULTIPART_ROAM_ACTION;
        return;
    };
    match peer.action.wrapping_sub(24) {
        0 | 1 => {}
        2 => {
            actor.target_entity_index = peer.target_entity_index;
            actor.linked_entity_index = 0;
            actor.action = MULTIPART_ATTACK_ACTION;
            return;
        }
        3 => actor.linked_entity_index = peer.linked_entity_index,
        _ => {
            actor.linked_entity_index = 0;
            actor.action = MULTIPART_ROAM_ACTION;
            return;
        }
    }
    if let Some(linked) = actor_views
        .iter()
        .find(|candidate| candidate.source_entity_index == actor.linked_entity_index)
    {
        actor.desired_heading =
            i16::try_from(heading_for_positions(actor.position, linked.position))
                .expect("multipart linked heading is 11-bit");
    }
    apply_same_model_separation(
        actor,
        actor_views,
        MULTIPART_HEAD_COLLISION_HALF_WIDTH.into(),
    );
    actor.actual_speed = peer.actual_speed.wrapping_add(peer.max_speed);
}

fn apply_creature_wander(actor: &mut Level1Actor) {
    actor.entity_rng = next_rng16(actor.entity_rng);
    let direction_selector = actor.entity_rng;
    actor.entity_rng = next_rng16(actor.entity_rng);
    let sign = if direction_selector % 157 < 79 { -1 } else { 1 };
    let delta =
        i16::try_from((actor.entity_rng & 0x00ff) + 85).expect("creature wander delta fits i16");
    actor.desired_heading = actor.desired_heading.wrapping_add(sign * delta) & 0x07ff;
}

fn apply_same_model_separation(
    actor: &mut Level1Actor,
    actor_views: &[Level1Actor],
    threshold: i32,
) {
    if let Some(nearby) = actor_views.iter().find(|candidate| {
        candidate.kind == actor.kind
            && candidate.active
            && candidate.life >= 0
            && candidate.source_entity_index != actor.source_entity_index
            && i32::from(
                candidate
                    .position
                    .x
                    .wrapping_sub(actor.position.x)
                    .cast_signed(),
            )
            .abs()
                < threshold
            && i32::from(
                candidate
                    .position
                    .y
                    .wrapping_sub(actor.position.y)
                    .cast_signed(),
            )
            .abs()
                < threshold
    }) {
        actor.desired_heading =
            i16::try_from(heading_for_positions(nearby.position, actor.position))
                .expect("creature separation heading is 11-bit");
    }
}

fn move_firefly_altitude_toward(actor: &mut Level1Actor, target_z: i32) {
    let delta = (target_z - i32::from(actor.position.z)).clamp(-64, 64);
    actor.position.z = actor.position.z.wrapping_add(delta as i16);
}

fn move_firefly<W: FlightWorld>(actor: &mut Level1Actor, world: &W) -> ArcherMoveResult {
    move_ground_actor(
        actor,
        world,
        FIREFLY_GROUND_CLEARANCE,
        FIREFLY_DESCENT,
        FIREFLY_TURN_LIMIT,
        u32::MAX,
        FIREFLY_MAX_TILE_SLOPE,
    )
}

fn move_multipart_head<W: FlightWorld>(actor: &mut Level1Actor, world: &W) -> ArcherMoveResult {
    move_ground_actor(
        actor,
        world,
        MULTIPART_GROUND_CLEARANCE,
        MULTIPART_DESCENT,
        MULTIPART_TURN_LIMIT,
        MULTIPART_TERRAIN_MASK,
        MULTIPART_MAX_TILE_SLOPE,
    )
}

fn move_archer<W: FlightWorld>(actor: &mut Level1Actor, world: &W) -> ArcherMoveResult {
    move_ground_actor(
        actor,
        world,
        0,
        -128,
        ARCHER_TURN_LIMIT,
        ARCHER_TERRAIN_MASK,
        ARCHER_MAX_TILE_SLOPE,
    )
}

fn move_ground_actor<W: FlightWorld>(
    actor: &mut Level1Actor,
    world: &W,
    ground_clearance: i16,
    descent: i16,
    turn_limit: i16,
    terrain_mask: u32,
    maximum_tile_slope: i32,
) -> ArcherMoveResult {
    let original_position = actor.position;
    let original_yaw = actor.yaw;
    let headings = [
        original_yaw,
        original_yaw.wrapping_add(341) & 0x07ff,
        archer_second_avoidance_heading(original_yaw),
        archer_opposite_heading(original_yaw),
    ];

    for (attempt, heading) in headings.into_iter().enumerate() {
        let mut predicted = original_position;
        relax_to_terrain(&mut predicted, world, ground_clearance, descent);
        move_entity(&mut predicted, heading, 0, actor.actual_speed);

        let changed_tile = (original_position.x >> 8, original_position.y >> 8)
            != (predicted.x >> 8, predicted.y >> 8);
        if changed_tile
            && !ground_actor_candidate_is_valid(world, predicted, terrain_mask, maximum_tile_slope)
        {
            continue;
        }

        actor.position = predicted;
        actor.movement_blocked = attempt != 0;
        actor.yaw =
            heading.wrapping_add(turn_toward(heading, actor.desired_heading, turn_limit)) & 0x07ff;
        return if attempt == 0 && changed_tile {
            ArcherMoveResult::DirectCrossTile
        } else if attempt == 0 {
            ArcherMoveResult::DirectSameTile
        } else {
            ArcherMoveResult::Alternate
        };
    }

    actor.yaw = archer_opposite_heading(original_yaw);
    actor.movement_blocked = true;
    // Archer row 75 has byte_0x20 bit 0 set, so exhausting every candidate
    // kills it regardless of the current terrain material.
    actor.life = -1;
    ArcherMoveResult::Blocked
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArcherMoveResult {
    DirectSameTile,
    DirectCrossTile,
    Alternate,
    Blocked,
}

fn ground_actor_candidate_is_valid<W: FlightWorld>(
    world: &W,
    position: Position,
    terrain_mask: u32,
    maximum_tile_slope: i32,
) -> bool {
    let forbidden_material = world.material_mask(position) & !terrain_mask != 0;
    !forbidden_material && tile_slope_metric(world, position) < maximum_tile_slope
}

#[cfg(test)]
fn archer_candidate_is_valid<W: FlightWorld>(world: &W, position: Position) -> bool {
    ground_actor_candidate_is_valid(world, position, ARCHER_TERRAIN_MASK, ARCHER_MAX_TILE_SLOPE)
}

fn archer_second_avoidance_heading(original: i16) -> i16 {
    let original = original.cast_unsigned();
    let low = original.wrapping_sub(85) & 0x00ff;
    let high = original.wrapping_sub(341) & 0x0700;
    (high | low).cast_signed()
}

fn archer_opposite_heading(original: i16) -> i16 {
    let original = original.cast_unsigned();
    (((original.wrapping_add(0x0400)) & 0x0700) | (original & 0x00ff)).cast_signed()
}

fn tile_slope_metric<W: FlightWorld>(world: &W, position: Position) -> i32 {
    let x = position.x & 0xff00;
    let y = position.y & 0xff00;
    let sample = |sample_x: u16, sample_y: u16| {
        i32::from(world.ground_height(Position {
            x: sample_x,
            y: sample_y,
            z: 0,
        }))
    };
    let point_1 = sample(x, y);
    let point_2 = sample(x.wrapping_add(0x0100), y);
    let point_3 = sample(x.wrapping_add(0x0100), y.wrapping_add(0x0100));
    let point_4 = sample(x, y.wrapping_add(0x0100));
    let diagonal_1 = (point_1 + point_4 - point_2 - point_3).abs();
    let diagonal_2 = (point_1 + point_2 - point_4 - point_3).abs();
    diagonal_1.max(diagonal_2) / 32
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CastlePlacementRejection {
    CastleRoot(u16),
    Building(u16),
    Terrain(Tile),
}

fn castle_blocker_overlaps(position: Position, blocker: Level1CastlePlacementBlocker) -> bool {
    let wrapped_abs = |candidate: u16, obstacle: u16| {
        i32::from(obstacle.wrapping_sub(candidate).cast_signed()).abs()
    };
    wrapped_abs(position.x, blocker.position.x)
        <= i32::from(blocker.half_x) + CASTLE_PLACEMENT_MARGIN
        && wrapped_abs(position.y, blocker.position.y)
            <= i32::from(blocker.half_y) + CASTLE_PLACEMENT_MARGIN
}

fn castle_placement_rejection<W: FlightWorld>(
    world: &W,
    position: Position,
    castle_blockers: &[Level1CastlePlacementBlocker],
    building_blockers: &[Level1CastlePlacementBlocker],
) -> Option<CastlePlacementRejection> {
    if let Some(blocker) = castle_blockers
        .iter()
        .copied()
        .find(|blocker| castle_blocker_overlaps(position, *blocker))
    {
        return Some(CastlePlacementRejection::CastleRoot(
            blocker.source_entity_index,
        ));
    }
    if let Some(blocker) = building_blockers
        .iter()
        .copied()
        .find(|blocker| castle_blocker_overlaps(position, *blocker))
    {
        return Some(CastlePlacementRejection::Building(
            blocker.source_entity_index,
        ));
    }

    let start_x = position.x.to_be_bytes()[0].wrapping_sub(8);
    let start_y = position.y.to_be_bytes()[0].wrapping_sub(8);
    for row in 0..CASTLE_PLACEMENT_FOOTPRINT_TILES {
        for column in 0..CASTLE_PLACEMENT_FOOTPRINT_TILES {
            let tile = Tile {
                x: start_x.wrapping_add(column),
                y: start_y.wrapping_add(row),
            };
            let flags = world.angle_flags(tile);
            // Outdoor `sub_11C80` reads mapAngle as signed.
            if flags.cast_signed() < 0 {
                return Some(CastlePlacementRejection::Terrain(tile));
            }
        }
    }
    None
}

fn castle_root_positions<W: FlightWorld>(
    world: &W,
    footprint: &CastleBuildFootprint,
    projectile_position: Position,
) -> (Position, Position) {
    let mut root_position = Position {
        x: projectile_position.x & 0xff00,
        y: projectile_position.y & 0xff00,
        // `sub_4AA40` samples before replacing x/y with their snapped values.
        z: world.ground_height(projectile_position),
    };
    if ((root_position.x >> 8) + (root_position.y >> 8)) & 1 != 0 {
        root_position.x = root_position.x.wrapping_add(256);
    }

    let center_x = (root_position.x >> 8) as u8;
    let center_y = (root_position.y >> 8) as u8;
    let start_x = center_x.wrapping_sub(footprint.width / 2);
    let start_y = center_y.wrapping_sub(footprint.height / 2);
    let mut minimum = 250_u8;
    let mut sample = |x: u8, y: u8| {
        let height = world.ground_height(Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        });
        minimum = minimum.min((height >> 5) as u8);
    };
    for offset in 0..footprint.width {
        let x = start_x.wrapping_add(offset);
        sample(x, start_y);
        sample(x, start_y.wrapping_add(footprint.height));
    }
    for offset in 0..footprint.height {
        let y = start_y.wrapping_add(offset);
        sample(start_x.wrapping_add(footprint.width), y);
        sample(start_x, y);
    }
    let construction_anchor = Position {
        z: i16::from(minimum) * 32,
        ..root_position
    };
    (root_position, construction_anchor)
}

pub(crate) fn castle_placement_valid<W: FlightWorld>(
    world: &W,
    position: Position,
    castle_blockers: &[Level1CastlePlacementBlocker],
    building_blockers: &[Level1CastlePlacementBlocker],
) -> bool {
    castle_placement_rejection(world, position, castle_blockers, building_blockers).is_none()
}

fn approach_i16_by(value: i16, target: i16, step: i16) -> i16 {
    match value.cmp(&target) {
        std::cmp::Ordering::Less => value.saturating_add(step).min(target),
        std::cmp::Ordering::Greater => value.saturating_sub(step).max(target),
        std::cmp::Ordering::Equal => value,
    }
}

#[cfg(test)]
fn move_toward_position(position: &mut Position, yaw: &mut i16, target: Position, speed: i16) {
    *yaw =
        i16::try_from(heading_for_positions(*position, target)).expect("carrier heading is 11-bit");
    // remc2 AddBallon_60AB0 recomputes yaw toward the linked entity, but moves
    // with the carrier's existing pitch. The carrier constructor leaves that
    // pitch at zero; outdoor altitude is handled separately by sub_580E0.
    // Deriving pitch from the full diagonal here is not source behavior and
    // reverses horizontal motion when that diagonal exceeds i16::MAX.
    move_entity(position, *yaw, 0, speed);
}

fn next_rng16(seed: u16) -> u16 {
    seed.wrapping_mul(9_377).wrapping_add(9_439)
}

fn mana_sphere_collision_dimension(mana: i32) -> i16 {
    MANA_SPHERE_COLLISION_DIMENSIONS[usize::from(mana_sphere_size_ordinal(mana))]
}

fn mana_spheres_overlap(left: ManaSphere, right: ManaSphere) -> bool {
    let left_dimension = mana_sphere_collision_dimension(left.mana);
    let right_dimension = mana_sphere_collision_dimension(right.mana);
    overlaps_source_entities(
        left.position,
        left_dimension,
        left_dimension,
        left_dimension,
        right.position,
        right_dimension,
        right_dimension,
        right_dimension,
    )
}

fn mana_sphere_terrain_slope_impulse<W: FlightWorld>(world: &W, position: Position) -> (i16, i16) {
    let [_, tile_x] = position.x.to_le_bytes();
    let [_, tile_y] = position.y.to_le_bytes();
    let sample = |x: u8, y: u8| {
        i32::from(world.ground_height(Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        })) / 32
    };
    let height_00 = sample(tile_x, tile_y);
    let height_10 = sample(tile_x.wrapping_add(1), tile_y);
    let height_11 = sample(tile_x.wrapping_add(1), tile_y.wrapping_add(1));
    let height_01 = sample(tile_x, tile_y.wrapping_add(1));
    (
        i16::try_from(height_00 - height_10 - height_11 + height_01)
            .expect("u8 terrain slope impulse fits i16"),
        i16::try_from(height_00 + height_10 - height_11 - height_01)
            .expect("u8 terrain slope impulse fits i16"),
    )
}

fn merged_mana_owner(
    source_slots: &[Level1ResetSourceSlot],
    survivor_owner: u16,
    absorbed_owner: u16,
) -> u16 {
    if survivor_owner == 0 {
        return absorbed_owner;
    }
    if absorbed_owner == 0 || survivor_owner == absorbed_owner {
        return survivor_owner;
    }
    let survivor_class = source_slots
        .get(usize::from(survivor_owner))
        .filter(|owner| owner.active)
        .map_or(0, |owner| owner.class);
    let absorbed_class = source_slots
        .get(usize::from(absorbed_owner))
        .filter(|owner| owner.active)
        .map_or(0, |owner| owner.class);
    match (survivor_class == 10, absorbed_class == 10) {
        (true, false) => absorbed_owner,
        (false, true) | (true, true) => survivor_owner,
        // Distinct Level 1 class-3 owners have equal source max-mana, so the
        // source tie rule selects the absorbed sphere's owner.
        (false, false) => absorbed_owner,
    }
}

fn clear_source_record(record: &mut Level1ResetSourceSlot) {
    record.active = false;
    record.class = 0;
    record.model = 0;
    record.action = 0;
    record.phase = 0;
    record.entity_rng = 0;
    record.control_flags = 0;
}

fn resolved_mana_owner_entity_index(
    source_slots: &[Level1ResetSourceSlot],
    raw_owner_id: i16,
) -> u16 {
    let Ok(owner_entity_index) = u16::try_from(raw_owner_id) else {
        return 0;
    };
    source_slots
        .get(usize::from(owner_entity_index))
        .filter(|owner| owner.active && owner.class == 3)
        .map_or(0, |owner| owner.index)
}

fn next_rng32(seed: u32) -> u32 {
    seed.wrapping_mul(9_377).wrapping_add(9_439)
}

fn squared_wrapping_xy_distance(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    dx.wrapping_mul(dx)
        .cast_unsigned()
        .wrapping_add(dy.wrapping_mul(dy).cast_unsigned())
}

fn squared_wrapping_xyz_distance(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    let dz = i32::from(right.z.wrapping_sub(left.z));
    dx.wrapping_mul(dx)
        .wrapping_add(dy.wrapping_mul(dy))
        .wrapping_add(dz.wrapping_mul(dz))
        .cast_unsigned()
}

fn wrapping_xy_distance(left: Position, right: Position) -> u32 {
    legacy_sqrt(squared_wrapping_xy_distance(left, right))
}

fn exit_accelerated_speed(value: i16) -> i16 {
    let value = value.wrapping_add(8);
    if value < 0 { 0 } else { value.min(200) }
}

fn exit_relax_axis(value: i16, negative_bias: i16, shift: u32) -> i16 {
    let source_sign = if value < 0 { -1_i32 } else { 0 };
    let delta = (i32::from(value) - source_sign * i32::from(negative_bias)) >> shift;
    value.wrapping_sub(i16::try_from(delta).expect("exit axis relaxation fits i16"))
}

fn distance_3d_wrapped(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    let dz = i32::from(right.z.wrapping_sub(left.z));
    legacy_sqrt(
        dx.wrapping_mul(dx)
            .wrapping_add(dy.wrapping_mul(dy))
            .wrapping_add(dz.wrapping_mul(dz))
            .cast_unsigned(),
    )
}

fn relax_to_terrain<W: FlightWorld>(
    position: &mut Position,
    world: &W,
    ground_clearance: i16,
    vertical_relaxation: i16,
) {
    let ground = world.ground_height(*position);
    if position.z > ground {
        position.z = position.z.wrapping_add(vertical_relaxation);
    }
    if position.z <= ground.wrapping_add(ground_clearance) {
        position.z = ground.wrapping_add(ground_clearance);
    }
}

fn parse_authored_features(bytes: &[u8]) -> Result<Vec<AuthoredFeature>, String> {
    let required = AUTHORED_FEATURE_OFFSET + AUTHORED_FEATURE_COUNT * AUTHORED_FEATURE_BYTES;
    if bytes.len() < required {
        return Err(format!(
            "authored level too short for Level 1 feature table: {} < {required}",
            bytes.len()
        ));
    }
    let mut features = Vec::with_capacity(AUTHORED_FEATURE_COUNT);
    for index in 0..AUTHORED_FEATURE_COUNT {
        let offset = AUTHORED_FEATURE_OFFSET + index * AUTHORED_FEATURE_BYTES;
        let bytes = &bytes[offset..offset + AUTHORED_FEATURE_BYTES];
        features.push(AuthoredFeature {
            index: u16::try_from(index).expect("1,200 authored features fit u16"),
            kind: u16::from_le_bytes([bytes[0], bytes[1]]),
            subtype: i16::from_le_bytes([bytes[2], bytes[3]]),
            tile_x: u16::from_le_bytes([bytes[4], bytes[5]]),
            tile_y: u16::from_le_bytes([bytes[6], bytes[7]]),
            activation_group: i16::from_le_bytes([bytes[8], bytes[9]]),
            word_10: u16::from_le_bytes([bytes[10], bytes[11]]),
            stage_tag: i16::from_le_bytes([bytes[12], bytes[13]]),
            parameter_1: u16::from_le_bytes([bytes[14], bytes[15]]),
            parameter_2: u16::from_le_bytes([bytes[16], bytes[17]]),
            parameter_3: u16::from_le_bytes([bytes[18], bytes[19]]),
        });
    }
    Ok(features)
}

fn overlaps_projectile(projectile: Position, actor: &Level1Actor) -> bool {
    overlaps_source_entities(
        projectile,
        FIREBALL_COLLISION_HALF_WIDTH,
        FIREBALL_COLLISION_VERTICAL_OFFSET,
        FIREBALL_COLLISION_HALF_HEIGHT,
        actor.position,
        actor.collision_half_width,
        actor.collision_vertical_offset,
        actor.collision_half_height,
    )
}

fn overlaps_source_entities(
    left_position: Position,
    left_half_width: i16,
    left_vertical_offset: i16,
    left_half_height: i16,
    right_position: Position,
    right_half_width: i16,
    right_vertical_offset: i16,
    right_half_height: i16,
) -> bool {
    let dx = i32::from(right_position.x.wrapping_sub(left_position.x).cast_signed()).abs();
    let dy = i32::from(right_position.y.wrapping_sub(left_position.y).cast_signed()).abs();
    let left_z = left_position.z.wrapping_add(left_vertical_offset);
    let right_z = right_position.z.wrapping_add(right_vertical_offset);
    let dz = i32::from(right_z.wrapping_sub(left_z)).abs();
    dx < i32::from(left_half_width) + i32::from(right_half_width)
        && dy < i32::from(left_half_width) + i32::from(right_half_width)
        && dz < i32::from(left_half_height) + i32::from(right_half_height)
}

fn is_near_xy(left: Position, right: Position, tolerance: u16) -> bool {
    left.x.abs_diff(right.x) <= tolerance && left.y.abs_diff(right.y) <= tolerance
}

#[must_use]
pub fn positions_within_xy(left: Position, right: Position, tolerance: u16) -> bool {
    is_near_xy(left, right, tolerance)
}

#[must_use]
pub fn source_entities_overlap(
    left: Position,
    left_half_width: i16,
    left_vertical_offset: i16,
    left_half_height: i16,
    right: Position,
    right_half_width: i16,
    right_vertical_offset: i16,
    right_half_height: i16,
) -> bool {
    overlaps_source_entities(
        left,
        left_half_width,
        left_vertical_offset,
        left_half_height,
        right,
        right_half_width,
        right_vertical_offset,
        right_half_height,
    )
}

fn inside_switch_bounds(
    position: Position,
    center: Position,
    radius_xy: u16,
    radius_z: i16,
) -> bool {
    position.x.abs_diff(center.x) <= radius_xy
        && position.y.abs_diff(center.y) <= radius_xy
        && i32::from(position.z.wrapping_sub(center.z)).abs() <= i32::from(radius_z)
}

fn inside_exit_switch_xy(player: Position, center: Position, switch_half_width: i16) -> bool {
    let threshold = i32::from(switch_half_width) + i32::from(PLAYER_COLLISION_HALF_WIDTH);
    let dx = i32::from(player.x.wrapping_sub(center.x).cast_signed()).abs();
    let dy = i32::from(player.y.wrapping_sub(center.y).cast_signed()).abs();
    dx < threshold && dy < threshold
}

struct Level1Fnv1a64(u64);

impl Level1Fnv1a64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    const fn new() -> Self {
        Self(Self::OFFSET_BASIS)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn source_occupant(&mut self, occupant: Option<SourceOccupant>) {
        let (kind, id) = match occupant {
            None => (0, 0),
            Some(SourceOccupant::Reserved) => (1, 0),
            Some(SourceOccupant::ImportedLive) => (2, 0),
            Some(SourceOccupant::Authored(id)) => (3, u32::from(id)),
            Some(SourceOccupant::PlayerFireball(id)) => (4, id),
            Some(SourceOccupant::ArcherArrow(id)) => (5, id),
            Some(SourceOccupant::FireflyFireball(id)) => (6, id),
            Some(SourceOccupant::CastleBuild(id)) => (7, id),
            Some(SourceOccupant::CastleRoot(id)) => (8, id),
            Some(SourceOccupant::CastleConstruction(id)) => (9, id),
            Some(SourceOccupant::Possession(id)) => (10, id),
            Some(SourceOccupant::FireballImpact(id)) => (11, id),
            Some(SourceOccupant::WaterSplash(id)) => (12, id),
            Some(SourceOccupant::ManaSphere(id)) => (13, id),
            Some(SourceOccupant::ManaCarrier(id)) => (14, id),
            Some(SourceOccupant::DeathBurst(id)) => (15, id),
            Some(SourceOccupant::MultipartSegment(id)) => (16, u32::from(id)),
        };
        self.u8(kind);
        self.u32(id);
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objective_textbox_clock_matches_source_outer_turn_order() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 2_000);
        assert_eq!(mission.objective_textbox_ticks, 33);

        mission.begin_source_turn();
        assert_eq!(
            mission.objective_textbox_ticks, 32,
            "reset 33 is decremented once before the first stepped frame is drawn"
        );
        mission.objective_textbox_ticks = 1;
        mission.begin_source_turn();
        assert_eq!(
            mission.objective_textbox_ticks, 0,
            "one remaining turn expires without a persistent textbox frame"
        );

        mission.evaluate_objective(LEVEL1_FIRST_WAYPOINT, true, true);
        assert_eq!(
            (
                mission.current_stage,
                mission.current_text_selector,
                mission.objective_textbox_ticks,
            ),
            (1, 49, 200),
            "objective advancement resets the same drawn frame to 200"
        );
        mission.begin_source_turn();
        assert_eq!(
            mission.objective_textbox_ticks, 199,
            "the following outer turn performs the next single decrement"
        );
    }

    #[test]
    fn minimap_classifier_preserves_night_filters_phases_and_anchors() {
        let mut player = FlightState {
            entity_index: 50,
            tick: 6,
            position: Position {
                x: 30_000,
                y: 40_000,
                z: 0,
            },
            ..FlightState::default()
        };
        let base = Level1MinimapEntityInput {
            source_slot: 100,
            class: 5,
            model: 13,
            action: 0,
            source_id: 100,
            position: player.position,
            active: true,
            ..Level1MinimapEntityInput::default()
        };
        let civilian = level1_minimap_entity_command(&player, base).unwrap();
        assert_eq!(
            (
                civilian.kind,
                civilian.palette_index,
                civilian.size,
                civilian.anchor,
            ),
            (
                LEVEL1_MINIMAP_COMMAND_DOT,
                23,
                1,
                LEVEL1_MINIMAP_ANCHOR_TOP_LEFT,
            )
        );

        let castle = level1_minimap_entity_command(
            &player,
            Level1MinimapEntityInput {
                class: 3,
                model: 2,
                source_id: player.entity_index,
                source_id_is_player: true,
                source_id_is_local_player: true,
                ..base
            },
        )
        .unwrap();
        assert_eq!(
            (
                castle.kind,
                castle.selector,
                castle.anchor,
                castle.help_selector
            ),
            (
                LEVEL1_MINIMAP_COMMAND_SPRITE,
                58,
                LEVEL1_MINIMAP_ANCHOR_BOTTOM_LEFT,
                24,
            )
        );
        let carrier = level1_minimap_entity_command(
            &player,
            Level1MinimapEntityInput {
                class: 3,
                model: 3,
                source_id: player.entity_index,
                source_id_is_player: true,
                source_id_is_local_player: true,
                ..base
            },
        )
        .unwrap();
        assert_eq!(
            (carrier.selector, carrier.anchor),
            (66, LEVEL1_MINIMAP_ANCHOR_BOTTOM_CENTER)
        );

        let sphere = Level1MinimapEntityInput {
            class: 10,
            model: 39,
            player_link: player.entity_index,
            player_link_is_player: true,
            player_link_is_local_player: true,
            ..base
        };
        assert_eq!(
            level1_minimap_entity_command(&player, sphere)
                .unwrap()
                .palette_index,
            170
        );
        player.tick = 3;
        assert_eq!(
            level1_minimap_entity_command(&player, sphere)
                .unwrap()
                .palette_index,
            164
        );
        assert!(
            level1_minimap_entity_command(
                &player,
                Level1MinimapEntityInput {
                    action: 232,
                    ..base
                }
            )
            .is_none()
        );
        assert_eq!(
            level1_minimap_entity_command(
                &player,
                Level1MinimapEntityInput {
                    class: 10,
                    model: 34,
                    ..base
                }
            )
            .unwrap()
            .size,
            2
        );
    }

    #[derive(Clone, Copy)]
    struct FlatWorld;

    impl FlightWorld for FlatWorld {
        fn ground_height(&self, _position: Position) -> i16 {
            0
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            None
        }

        fn material_mask(&self, _position: Position) -> u32 {
            0
        }

        fn angle_flags(&self, _tile: crate::Tile) -> u8 {
            0
        }
    }

    #[derive(Clone, Copy)]
    struct UniformMaterialWorld {
        material_mask: u32,
    }

    impl FlightWorld for UniformMaterialWorld {
        fn ground_height(&self, _position: Position) -> i16 {
            0
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            None
        }

        fn material_mask(&self, _position: Position) -> u32 {
            self.material_mask
        }

        fn angle_flags(&self, _tile: crate::Tile) -> u8 {
            0
        }
    }

    struct OneBlockedTileWorld {
        blocked_x: u8,
        blocked_y: u8,
    }

    struct InvalidCastleTileWorld;

    struct CastleAngleWorld {
        first: Tile,
        second: Tile,
    }

    #[derive(Clone, Copy)]
    struct RaisedMaterialWorld {
        ground_height: i16,
        material_mask: u32,
    }

    impl FlightWorld for RaisedMaterialWorld {
        fn ground_height(&self, _position: Position) -> i16 {
            self.ground_height
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            None
        }

        fn material_mask(&self, _position: Position) -> u32 {
            self.material_mask
        }

        fn angle_flags(&self, _tile: crate::Tile) -> u8 {
            0
        }
    }

    impl FlightWorld for InvalidCastleTileWorld {
        fn ground_height(&self, _position: Position) -> i16 {
            0
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            None
        }

        fn material_mask(&self, _position: Position) -> u32 {
            0
        }

        fn angle_flags(&self, _tile: crate::Tile) -> u8 {
            0x80
        }
    }

    impl FlightWorld for CastleAngleWorld {
        fn ground_height(&self, _position: Position) -> i16 {
            0
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            None
        }

        fn material_mask(&self, _position: Position) -> u32 {
            u32::MAX
        }

        fn angle_flags(&self, tile: crate::Tile) -> u8 {
            u8::from(tile == self.first || tile == self.second) << 7
        }
    }

    impl FlightWorld for OneBlockedTileWorld {
        fn ground_height(&self, _position: Position) -> i16 {
            0
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            None
        }

        fn material_mask(&self, position: Position) -> u32 {
            if (position.x >> 8) as u8 == self.blocked_x
                && (position.y >> 8) as u8 == self.blocked_y
            {
                1
            } else {
                0
            }
        }

        fn angle_flags(&self, _tile: crate::Tile) -> u8 {
            0
        }
    }

    #[derive(Clone, Copy)]
    struct OneCornerSlopeWorld {
        tile_x: u8,
        tile_y: u8,
        raw_height: i16,
    }

    impl FlightWorld for OneCornerSlopeWorld {
        fn ground_height(&self, position: Position) -> i16 {
            if position.x >> 8 == u16::from(self.tile_x)
                && position.y >> 8 == u16::from(self.tile_y)
                && position.x & 0x00ff == 0
                && position.y & 0x00ff == 0
            {
                self.raw_height * 32
            } else {
                0
            }
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            None
        }

        fn material_mask(&self, _position: Position) -> u32 {
            0
        }

        fn angle_flags(&self, _tile: crate::Tile) -> u8 {
            0
        }
    }

    fn authored_bytes() -> Vec<u8> {
        let mut bytes =
            vec![0_u8; AUTHORED_FEATURE_OFFSET + AUTHORED_FEATURE_COUNT * AUTHORED_FEATURE_BYTES];
        for (index, kind, subtype, x, y, group, tag) in [
            (21, 14, 5, 0, 0, 4, 4),
            (31, 15, 2, 194, 213, 1, 1),
            (99, 11, 32, 0, 0, 1, 2),
            (104, 11, 0, 0, 0, 1, 7),
            (100, 15, 3, 202, 212, 11, 1),
            (101, 5, 4, 202, 209, 3, 3),
            (102, 5, 4, 202, 217, 3, 3),
            (103, 5, 4, 195, 217, 3, 3),
            (105, 11, 32, 0, 0, 2, 3),
            (106, 5, 4, 195, 209, 3, 3),
            (108, 5, 19, 217, 126, 6, 6),
            (109, 11, 17, 0, 0, 3, 11),
            (132, 14, 3, 97, 221, 4, 4),
            (134, 5, 19, 213, 125, 6, 6),
            (135, 5, 19, 211, 132, 6, 6),
            (137, 11, 12, 75, 218, 4, 10),
            (138, 10, 59, 0, 0, 4, 4),
            (139, 10, 59, 0, 0, 4, 4),
            (140, 14, 5, 0, 0, 4, 4),
            (141, 14, 5, 0, 0, 4, 4),
            (153, 11, 32, 0, 0, 3, 5),
            (154, 2, 1, 0, 0, 6, 0),
            (155, 2, 0, 0, 0, 1, 0),
            (156, 2, 0, 0, 0, 1, 0),
            (157, 2, 0, 0, 0, 1, 0),
            (158, 2, 0, 0, 0, 1, 0),
            (159, 2, 0, 0, 0, 1, 0),
            (160, 2, 0, 0, 0, 1, 0),
            (161, 2, 0, 0, 0, 1, 0),
            (162, 2, 0, 0, 0, 1, 0),
            (163, 2, 0, 0, 0, 1, 0),
            (164, 2, 0, 0, 0, 1, 0),
            (168, 2, 0, 0, 0, 1, 0),
            (169, 2, 0, 0, 0, 1, 0),
            (170, 2, 0, 0, 0, 1, 0),
            (171, 2, 0, 0, 0, 1, 0),
            (172, 2, 0, 0, 0, 1, 0),
            (173, 2, 0, 0, 0, 1, 0),
            (174, 2, 0, 0, 0, 1, 0),
            (175, 2, 0, 0, 0, 1, 0),
            (189, 5, 19, 209, 129, 6, 6),
            (190, 11, 32, 0, 0, 5, 6),
            (193, 5, 19, 215, 129, 6, 6),
            (231, 11, 1, 0, 0, 2, 8),
            (232, 11, 0, 0, 0, 8, 9),
            (233, 14, 5, 0, 0, 4, 4),
            (253, 10, 59, 0, 0, 12, 12),
            (304, 14, 5, 218, 215, 4, 4),
            (305, 11, 0, 217, 213, 4, 33),
            (306, 5, 19, 217, 210, 33, 33),
            (307, 5, 3, 110, 97, 4, 4),
            (308, 11, 16, 134, 98, 4, 34),
            (309, 5, 3, 90, 37, 34, 34),
            (310, 5, 3, 51, 32, 34, 34),
            (311, 5, 3, 53, 147, 34, 34),
        ] {
            let offset = AUTHORED_FEATURE_OFFSET + index * AUTHORED_FEATURE_BYTES;
            bytes[offset..offset + 2].copy_from_slice(&u16::to_le_bytes(kind));
            bytes[offset + 2..offset + 4].copy_from_slice(&i16::to_le_bytes(subtype));
            bytes[offset + 4..offset + 6].copy_from_slice(&u16::to_le_bytes(x));
            bytes[offset + 6..offset + 8].copy_from_slice(&u16::to_le_bytes(y));
            bytes[offset + 8..offset + 10].copy_from_slice(&i16::to_le_bytes(group));
            if index == usize::from(LEVEL1_EXIT_SWITCH_FEATURE) {
                bytes[offset + 10..offset + 12].copy_from_slice(&u16::to_le_bytes(2));
            } else if index == 305 {
                bytes[offset + 10..offset + 12].copy_from_slice(&u16::to_le_bytes(8));
            } else if index == 308 {
                bytes[offset + 10..offset + 12].copy_from_slice(&u16::to_le_bytes(7));
            }
            bytes[offset + 12..offset + 14].copy_from_slice(&i16::to_le_bytes(tag));
        }
        bytes
    }

    fn isolated_constructing_castle(
        root_slot: u16,
        child_slot: Option<u16>,
    ) -> (Level1MissionState, FlightState) {
        let mut template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        template.castle_stage_one_footprint = Some(CastleBuildFootprint::synthetic_stage_one());
        let mut mission = template.session(10_000, 1_000);
        let free_stack = child_slot.into_iter().collect::<Vec<_>>();
        mission.source_pool =
            Level1SourcePool::from_free_stack(&free_stack).expect("isolated source-pool fixture");
        mission.castle_exists = true;
        mission.castle_source_entity_index = root_slot;
        mission.castle_presentation_id = 41;
        mission.castle_owner_entity_index = 152;
        mission.castle_action = CASTLE_ROOT_ACTION;
        mission.castle_build_state = 0;
        mission.castle_level = 0;
        mission.castle_position = Position {
            x: 20 << 8,
            y: 22 << 8,
            z: 0,
        };
        mission.castle_construction_anchor = mission.castle_position;
        mission.castle_life = CASTLE_ROOT_CONSTRUCTOR_LIFE;
        mission.castle_max_life = CASTLE_ROOT_CONSTRUCTOR_LIFE;
        mission.source_pool.occupants[usize::from(root_slot)] =
            Some(SourceOccupant::CastleRoot(mission.castle_presentation_id));
        bind_dynamic_source_record(
            &mut mission.source_slots,
            root_slot,
            SourceOccupant::CastleRoot(mission.castle_presentation_id),
            mission.global_rng,
        );
        mission.source_slots[usize::from(root_slot)].phase = root_slot as u8;
        (
            mission,
            FlightState {
                entity_index: 152,
                position: Position {
                    x: 40 << 8,
                    y: 40 << 8,
                    z: 2_000,
                },
                ..FlightState::default()
            },
        )
    }

    fn dispatch_isolated_source_turn(mission: &mut Level1MissionState, player: &FlightState) {
        mission.begin_source_turn_at(player.position);
        for source_slot in 1..ENTITY_POOL_SIZE {
            mission.dispatch_source_slot(
                &FlatWorld,
                player,
                u16::try_from(source_slot).expect("source slot fits u16"),
                false,
                false,
                None,
            );
        }
    }

    #[test]
    fn source_pool_preserves_reclaim_stack_and_explicit_rebuild_order() {
        let mut pool = Level1SourcePool::from_free_stack(&[3, 2, 1]).unwrap();
        assert_eq!(pool.allocate(SourceOccupant::PlayerFireball(10)), Some(1));
        assert_eq!(pool.allocate(SourceOccupant::Possession(11)), Some(2));
        assert_eq!(pool.occupant(1), Some(SourceOccupant::PlayerFireball(10)));

        assert!(pool.release_occupant(SourceOccupant::PlayerFireball(10)));
        assert_eq!(
            pool.allocate(SourceOccupant::FireballImpact(12)),
            Some(1),
            "ordinary reclamation pushes without rebuilding the stack"
        );

        assert!(pool.release_occupant(SourceOccupant::Possession(11)));
        assert!(pool.release_occupant(SourceOccupant::FireballImpact(12)));
        pool.rebuild_free_stack();
        assert_eq!(
            pool.allocate(SourceOccupant::ManaSphere(13)),
            Some(1),
            "authored activation rebuild restores lowest-free allocation"
        );
        assert_eq!(pool.allocate(SourceOccupant::ManaCarrier(14)), Some(2));
        assert_eq!(pool.occupant(0), Some(SourceOccupant::Reserved));
    }

    #[test]
    fn disabled_slots_remain_occupied_until_ascending_next_turn_reclaim() {
        let mut pool = Level1SourcePool::from_free_stack(&[9, 3]).unwrap();
        assert_eq!(pool.allocate(SourceOccupant::PlayerFireball(10)), Some(3));
        assert_eq!(pool.allocate(SourceOccupant::Possession(11)), Some(9));
        assert!(pool.disable_slot(9));
        assert!(pool.disable_slot(3));
        assert_eq!(pool.occupant(3), Some(SourceOccupant::PlayerFireball(10)));
        assert_eq!(pool.occupant(9), Some(SourceOccupant::Possession(11)));
        assert_eq!(
            pool.allocate(SourceOccupant::FireballImpact(12)),
            None,
            "disabled records are not allocatable during the deletion pass"
        );

        assert_eq!(pool.reclaim_disabled_ascending(), [3, 9]);
        assert_eq!(
            pool.allocate(SourceOccupant::FireballImpact(12)),
            Some(9),
            "ascending reclaim appends slots so the highest reclaimed slot pops first"
        );
    }

    #[test]
    fn source_pool_rejects_duplicate_or_sentinel_free_slots() {
        assert!(Level1SourcePool::from_free_stack(&[2, 2, 1]).is_err());
        assert!(Level1SourcePool::from_free_stack(&[2, 0, 1]).is_err());
    }

    #[test]
    fn castle_construction_allocation_is_atomic_and_child_cursor_order_is_exact() {
        let (mut exhausted, player) = isolated_constructing_castle(100, None);
        dispatch_isolated_source_turn(&mut exhausted, &player);
        assert_eq!(exhausted.castle_level, 0);
        assert_eq!(exhausted.castle_build_state, 0);
        assert!(exhausted.castle_construction().is_none());
        assert!(
            exhausted
                .events()
                .iter()
                .all(|event| !matches!(event, Level1MissionEvent::CastleConstructionSound { .. })),
            "a failed child allocation must not commit sound or stage state"
        );

        for (root_slot, child_slot, first_pass_same_turn, completion_turn) in
            [(100_u16, 200_u16, true, 20_u8), (200, 100, false, 21)]
        {
            let (mut mission, player) = isolated_constructing_castle(root_slot, Some(child_slot));
            dispatch_isolated_source_turn(&mut mission, &player);
            assert_eq!(
                mission
                    .events()
                    .iter()
                    .filter(|event| matches!(
                        event,
                        Level1MissionEvent::CastleConstructionSound {
                            sound_id: CASTLE_CONSTRUCTION_SOUND,
                            source_entity_index,
                            ..
                        } if *source_entity_index == root_slot
                    ))
                    .count(),
                1
            );
            let child = mission
                .castle_construction()
                .expect("successful root dispatch creates the typed child");
            assert_eq!(child.source_entity_index, child_slot);
            assert_eq!(child.parent_entity_index, root_slot);
            assert_eq!(
                (
                    mission.source_slots[usize::from(child_slot)].class,
                    mission.source_slots[usize::from(child_slot)].model,
                    mission.source_slots[usize::from(child_slot)].action,
                ),
                (
                    CASTLE_CONSTRUCTION_CLASS,
                    CASTLE_CONSTRUCTION_MODEL,
                    CASTLE_CONSTRUCTION_ACTION,
                )
            );
            let first_updates = mission.drain_castle_terrain_updates().collect::<Vec<_>>();
            assert_eq!(
                first_updates,
                if first_pass_same_turn {
                    vec![CastleTerrainUpdate::ConstructionPass {
                        position: mission.castle_construction_anchor,
                        remaining: 18,
                    }]
                } else {
                    Vec::new()
                }
            );

            let mut observed_completion = None;
            let mut observed_stable = None;
            let mut observed_carrier = None;
            for turn in 2_u8..=24 {
                dispatch_isolated_source_turn(&mut mission, &player);
                if mission.drain_castle_terrain_updates().any(|update| {
                    matches!(update, CastleTerrainUpdate::ConstructionCompleted { .. })
                }) {
                    observed_completion = Some(turn);
                }
                if observed_stable.is_none() && mission.castle_action == 4 {
                    observed_stable = Some(turn);
                }
                if observed_carrier.is_none() && mission.mana_carrier().is_some() {
                    observed_carrier = Some(turn);
                }
            }
            assert_eq!(observed_completion, Some(completion_turn));
            assert_eq!(observed_stable, Some(21));
            assert_eq!(
                observed_carrier,
                Some(if root_slot & 1 == 0 { 23 } else { 22 })
            );
            assert_eq!(mission.castle_build_state, 0);
            assert_eq!(mission.castle_action, 4);
        }
    }

    #[test]
    fn lethal_castle_retries_without_a_free_slot_then_tears_down_atomically() {
        let root_slot = 100_u16;
        let (mut mission, player) = isolated_constructing_castle(root_slot, Some(200));
        for _ in 0..24 {
            dispatch_isolated_source_turn(&mut mission, &player);
            let _ = mission.drain_castle_terrain_updates().count();
        }
        assert_eq!(mission.castle_action, 4);
        assert!(mission.mana_carrier().is_some());
        for slot in mission.source_pool.free_stack.drain(..) {
            mission.source_pool.occupants[usize::from(slot)] = Some(SourceOccupant::ImportedLive);
        }
        assert!(!mission.source_pool.has_ordinary_free_slot());

        mission.castle_life = -1;
        dispatch_isolated_source_turn(&mut mission, &player);
        assert_eq!(mission.castle_action, 6);
        dispatch_isolated_source_turn(&mut mission, &player);
        assert_eq!(mission.castle_action, 4);
        assert!(mission.castle_exists);
        assert!(mission.drain_castle_terrain_updates().next().is_none());
        dispatch_isolated_source_turn(&mut mission, &player);
        assert_eq!(mission.castle_action, 6);

        let free_slot = 300_u16;
        mission.source_pool.occupants[usize::from(free_slot)] = None;
        mission.source_pool.free_stack.push(free_slot);
        dispatch_isolated_source_turn(&mut mission, &player);
        assert!(!mission.castle_exists);
        assert_eq!(mission.castle_level, 0);
        assert_eq!(mission.castle_owner_entity_index, 0);
        assert!(mission.mana_carrier().is_none());
        assert_eq!(
            mission.drain_castle_terrain_updates().collect::<Vec<_>>(),
            vec![CastleTerrainUpdate::StageDestroyed {
                position: mission.castle_construction_anchor,
                entity_rng: mission.source_slots[0].entity_rng,
            }]
        );
        assert_eq!(
            mission
                .events()
                .iter()
                .filter(|event| matches!(
                    event,
                    Level1MissionEvent::MissionSound {
                        sound_id: CASTLE_DESTRUCTION_SOUND,
                        ..
                    }
                ))
                .count(),
            1
        );
        assert!(mission.source_pool.disabled[usize::from(root_slot)]);
    }

    #[test]
    fn mana_owner_relation_requires_a_live_class_three_source_entity() {
        let mut slots = (0..=152)
            .map(|index| Level1ResetSourceSlot {
                index,
                active: index != 0,
                class: 10,
                model: 45,
                action: 52,
                phase: 0,
                entity_rng: 0,
                control_flags: 0,
            })
            .collect::<Vec<_>>();
        slots[152].class = 3;
        slots[152].model = 0;

        assert_eq!(resolved_mana_owner_entity_index(&slots, -1), 0);
        assert_eq!(resolved_mana_owner_entity_index(&slots, 0), 0);
        assert_eq!(
            resolved_mana_owner_entity_index(&slots, 3),
            0,
            "Level 1 reset slot 3 is class 10/model 45, so remc2 renders its mana as neutral"
        );
        assert_eq!(resolved_mana_owner_entity_index(&slots, 152), 152);
        slots[152].active = false;
        assert_eq!(resolved_mana_owner_entity_index(&slots, 152), 0);
    }

    #[test]
    fn complete_reset_projection_preserves_fields_and_uses_replaceable_fallback() {
        let mut slots = (0..ENTITY_POOL_SIZE)
            .map(|index| Level1ResetSourceSlot {
                index: u16::try_from(index).unwrap(),
                active: index != 0,
                class: 7,
                model: 8,
                action: 9,
                phase: 10,
                entity_rng: 11,
                control_flags: 0,
            })
            .collect::<Vec<_>>();
        slots[3].control_flags = SOURCE_REPLACEABLE_FLAG;
        slots[9].control_flags = SOURCE_REPLACEABLE_FLAG;

        let mut pool = Level1SourcePool::from_reset_slots(&slots).unwrap();
        assert_eq!(
            pool.allocate(SourceOccupant::PlayerFireball(10)),
            Some(3),
            "with no free slots NewEvent_4A050 uses the lowest replaceable source slot"
        );
        assert_eq!(
            pool.allocate(SourceOccupant::Possession(11)),
            Some(9),
            "replaceable slots retain the descending-scan/pop allocation order"
        );

        let template = Level1MissionTemplate::decode_with_source_slots(
            &authored_bytes(),
            &FlatWorld,
            19_577,
            123,
            &slots,
        )
        .unwrap();
        let state = template.session(30_000, 5_000);
        assert_eq!(state.source_slots[3], slots[3]);
        assert_eq!(state.source_slots[9], slots[9]);
    }

    #[test]
    fn inactive_replaceable_reset_slot_is_allocated_only_once() {
        let mut slots = (0..ENTITY_POOL_SIZE)
            .map(|index| Level1ResetSourceSlot {
                index: u16::try_from(index).unwrap(),
                active: index != 0,
                class: u8::from(index != 0),
                model: 0,
                action: 0,
                phase: 0,
                entity_rng: 0,
                control_flags: 0,
            })
            .collect::<Vec<_>>();
        slots[3].active = false;
        slots[3].class = 0;
        slots[3].control_flags = SOURCE_REPLACEABLE_FLAG;

        let mut pool = Level1SourcePool::from_reset_slots(&slots).unwrap();
        assert_eq!(pool.allocate(SourceOccupant::PlayerFireball(10)), Some(3));
        assert_eq!(
            pool.allocate(SourceOccupant::Possession(11)),
            None,
            "an inactive reset row cannot also remain in the replaceable stack"
        );
    }

    #[test]
    fn replaceable_fallback_retires_the_imported_projection_once() {
        let mut slots = (0..ENTITY_POOL_SIZE)
            .map(|index| Level1ResetSourceSlot {
                index: u16::try_from(index).unwrap(),
                active: index != 0,
                class: u8::from(index != 0),
                model: 0,
                action: 0,
                phase: 0,
                entity_rng: 0,
                control_flags: 0,
            })
            .collect::<Vec<_>>();
        slots[3].control_flags = SOURCE_REPLACEABLE_FLAG;

        let template = Level1MissionTemplate::decode_with_source_slots(
            &authored_bytes(),
            &FlatWorld,
            19_577,
            0,
            &slots,
        )
        .unwrap();
        let mut mission = template.session(10_000, 1_000);
        assert!(mission.imported_source_slot_is_live(3));

        assert_eq!(
            mission
                .source_pool
                .allocate(SourceOccupant::PlayerFireball(10)),
            Some(3)
        );
        assert!(!mission.imported_source_slot_is_live(3));
        assert_eq!(mission.take_reclaimed_imported_source_slots(), vec![3]);
        assert!(
            mission.take_reclaimed_imported_source_slots().is_empty(),
            "the typed imported payload must be retired exactly once"
        );
    }

    #[test]
    fn mutable_imported_rows_are_synchronized_into_the_source_arena() {
        let slots = (0..ENTITY_POOL_SIZE)
            .map(|index| Level1ResetSourceSlot {
                index: u16::try_from(index).unwrap(),
                active: index != 0,
                class: u8::from(index != 0),
                model: 0,
                action: 0,
                phase: 0,
                entity_rng: u16::try_from(index).unwrap(),
                control_flags: 8,
            })
            .collect::<Vec<_>>();
        let template = Level1MissionTemplate::decode_with_source_slots(
            &authored_bytes(),
            &FlatWorld,
            19_577,
            0,
            &slots,
        )
        .unwrap();
        let mut mission = template.session(10_000, 1_000);

        mission.sync_imported_source_slot(50, true, 14, 9, 0x1234, 0x5678);
        assert_eq!(
            (
                mission.source_slots[50].action,
                mission.source_slots[50].phase,
                mission.source_slots[50].entity_rng,
                mission.source_slots[50].control_flags,
            ),
            (14, 9, 0x1234, 0x5678)
        );

        let player = FlightState {
            entity_index: 152,
            entity_phase: 37,
            position: LEVEL1_INITIAL_ZONE_SWITCH_CENTER,
            ..FlightState::default()
        };
        mission.begin_source_turn();
        mission.dispatch_source_slot(&FlatWorld, &player, 48, false, false, None);
        assert_eq!(
            mission.source_slots[48].phase,
            mission.initial_zone_switch_phase
        );
        mission.dispatch_source_slot(&FlatWorld, &player, 112, false, false, None);
        assert_eq!(mission.source_slots[112].phase, 1);
        mission.dispatch_source_slot(&FlatWorld, &player, 151, false, false, None);
        assert_eq!(
            mission.source_slots[151].phase,
            mission.first_waypoint_switch_phase
        );
        mission.dispatch_source_slot(&FlatWorld, &player, 152, false, false, None);
        assert_eq!(mission.source_slots[152].phase, player.entity_phase);
    }

    #[test]
    fn source_arena_hash_covers_pending_ownership_and_constructor_state() {
        let mission = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld)
            .unwrap()
            .session(10_000, 1_000);
        let baseline = mission.source_arena_hash_v1();

        let mut replaced = mission.clone();
        replaced.source_pool.replaced_imported_slots.push(50);
        assert_ne!(replaced.source_arena_hash_v1(), baseline);

        let mut pending = mission.clone();
        pending.pending_source_slot_releases.push(48);
        assert_ne!(pending.source_arena_hash_v1(), baseline);

        let mut reclaimed = mission.clone();
        reclaimed.reclaimed_imported_source_slots.push(50);
        assert_ne!(reclaimed.source_arena_hash_v1(), baseline);

        let mut authored = mission.clone();
        authored.authored_allocations[0].state = AuthoredAllocationState::PendingDelete;
        assert_ne!(authored.source_arena_hash_v1(), baseline);

        let mut rng = mission.clone();
        rng.global_rng = rng.global_rng.wrapping_add(1);
        assert_ne!(rng.source_arena_hash_v1(), baseline);

        let mut identity = mission.clone();
        identity.next_effect_id = identity.next_effect_id.wrapping_add(1);
        assert_ne!(identity.source_arena_hash_v1(), baseline);

        let mut phase = mission;
        phase.model_phase_counters[1] = 1;
        assert_ne!(phase.source_arena_hash_v1(), baseline);
    }

    #[test]
    fn exhausted_authored_group_activation_keeps_rows_dormant_and_slot_zero_reserved() {
        let slots = (0..ENTITY_POOL_SIZE)
            .map(|index| Level1ResetSourceSlot {
                index: u16::try_from(index).unwrap(),
                active: index != 0,
                class: u8::from(index != 0),
                model: 0,
                action: 0,
                phase: 0,
                entity_rng: 0,
                control_flags: 0,
            })
            .collect::<Vec<_>>();
        let template = Level1MissionTemplate::decode_with_source_slots(
            &authored_bytes(),
            &FlatWorld,
            19_577,
            0,
            &slots,
        )
        .unwrap();
        let mut mission = template.session(10_000, 1_000);

        mission.activate_group(3);

        assert_eq!(
            mission.source_pool.occupant(0),
            Some(SourceOccupant::Reserved)
        );
        assert!(mission.authored_allocations.iter().all(|allocation| {
            allocation.activation_group != 3
                || (allocation.state == AuthoredAllocationState::Dormant
                    && allocation.source_entity_index == 0)
        }));
        assert!(mission.actors.iter().all(|actor| {
            actor.kind != Level1ActorKind::Archer
                || (!actor.active && actor.source_entity_index == 0)
        }));
    }

    #[test]
    fn ascending_cursor_steps_new_higher_slots_and_defers_new_lower_slots() {
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 20_000,
                y: 20_000,
                z: 2_000,
            },
            yaw: 0,
            effective_pitch: 0,
            ..FlightState::default()
        };
        let make_mission = |free_slot| {
            Level1MissionTemplate::decode_with_runtime_context(
                &authored_bytes(),
                &FlatWorld,
                19_577,
                0,
                &[free_slot],
            )
            .unwrap()
            .session(30_000, 1_000)
        };

        let mut lower = make_mission(100);
        lower.step_with_fireball(&FlatWorld, &player, true);
        let lower_projectile = match lower.projectiles[0] {
            Level1Projectile::PlayerFireball(projectile) => projectile,
            _ => unreachable!(),
        };
        assert_eq!(lower_projectile.source_entity_index, 100);
        assert_eq!(
            lower_projectile.position.z,
            player.position.z.wrapping_add(100),
            "a source slot below the current player cursor waits for the next pass"
        );

        let mut higher = make_mission(200);
        higher.step_with_fireball(&FlatWorld, &player, true);
        let higher_projectile = match higher.projectiles[0] {
            Level1Projectile::PlayerFireball(projectile) => projectile,
            _ => unreachable!(),
        };
        assert_eq!(higher_projectile.source_entity_index, 200);
        assert_ne!(
            higher_projectile.position, lower_projectile.position,
            "a newly allocated higher source slot dispatches later in the same pass"
        );
    }

    #[test]
    fn carrier_and_mana_sphere_identity_are_their_source_slots() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 10_000);
        mission.next_effect_id = 700;
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 20_000,
                y: 20_000,
                z: 1_000,
            },
            ..FlightState::default()
        };

        assert!(mission.spawn_fireball(&player));
        assert!(mission.spawn_possession(&player));
        mission.install_stable_stage_one_fixture(&FlatWorld, player.position, player.entity_index);

        let Level1Projectile::PlayerFireball(fireball) = mission.projectiles[0] else {
            panic!("first dynamic entity must be the player Fireball");
        };
        let Level1Projectile::Possession(possession) = mission.projectiles[1] else {
            panic!("second dynamic entity must be Possession");
        };
        let carrier = mission
            .mana_carrier
            .expect("castle creates its mana carrier");

        assert_eq!((fireball.id, fireball.source_entity_index), (700, 1));
        assert_eq!((possession.id, possession.source_entity_index), (701, 2));
        assert_eq!(
            (
                mission.castle_presentation_id,
                mission.castle_source_entity_index
            ),
            (702, 3)
        );
        assert_eq!((carrier.id, carrier.source_entity_index), (4, 4));
        assert_eq!(
            mission.source_pool.occupant(fireball.source_entity_index),
            Some(SourceOccupant::PlayerFireball(fireball.id))
        );
        assert_eq!(
            mission.source_pool.occupant(possession.source_entity_index),
            Some(SourceOccupant::Possession(possession.id))
        );
        assert_eq!(
            mission
                .source_pool
                .occupant(mission.castle_source_entity_index),
            Some(SourceOccupant::CastleRoot(mission.castle_presentation_id))
        );
        assert_eq!(
            mission.source_pool.occupant(carrier.source_entity_index),
            Some(SourceOccupant::ManaCarrier(carrier.id))
        );
        assert_eq!(
            mission.next_effect_id, 703,
            "the class-3/model-3 carrier does not consume a synthetic presentation ID"
        );
    }

    #[test]
    fn failed_dynamic_allocation_does_not_burn_presentation_identity() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 10_000);
        mission.next_effect_id = 700;
        mission.source_pool.free_stack.clear();
        let player = FlightState {
            entity_index: 152,
            ..FlightState::default()
        };
        let mana_before = mission.player_mana;

        assert!(!mission.spawn_fireball(&player));
        assert_eq!(mission.next_effect_id, 700);
        assert_eq!(mission.player_mana, mana_before);
        assert!(mission.projectiles.is_empty());
    }

    #[test]
    fn exact_level1_actor_manifest_is_decoded() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        assert_eq!(template.actors().len(), 19);
        let first_archer = template
            .actors()
            .iter()
            .find(|actor| actor.authored_index == 101)
            .unwrap();
        assert_eq!(
            first_archer.position,
            Position {
                x: (202 << 8) + 128,
                y: (209 << 8) + 128,
                z: 0,
            }
        );
        assert_eq!(
            template
                .actors()
                .iter()
                .filter(|actor| actor.kind == Level1ActorKind::Archer)
                .count(),
            4
        );
        assert_eq!(
            template
                .actors()
                .iter()
                .filter(|actor| actor.kind == Level1ActorKind::Firefly)
                .count(),
            6
        );
        assert_eq!(
            template
                .actors()
                .iter()
                .filter(|actor| actor.kind == Level1ActorKind::MultipartHead)
                .count(),
            4
        );
        assert_eq!(
            template
                .actors()
                .iter()
                .filter(|actor| actor.kind == Level1ActorKind::ExperienceScroll)
                .count(),
            1
        );
    }

    #[test]
    fn spatial_and_objective_switch_chain_activates_jar_then_archers() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        let mut player = FlightState {
            position: LEVEL1_FIRST_WAYPOINT,
            ..FlightState::default()
        };
        mission.step(&FlatWorld, &player);
        assert_eq!(mission.current_stage, 1);
        assert!(
            !mission
                .actors()
                .iter()
                .any(|actor| actor.kind == Level1ActorKind::CastleSpellJar && actor.active)
        );

        // O0 itself does not spawn G1. Leaving the large group-0 switch
        // volume does; idx99 then observes completed O0 and chains G2.
        player.position = Position {
            x: 36_000,
            y: LEVEL1_FIRST_WAYPOINT.y,
            z: LEVEL1_INITIAL_ZONE_SWITCH_CENTER.z,
        };
        for _ in 0..10 {
            mission.step(&FlatWorld, &player);
            if mission.allocation_slot(105).is_some_and(|slot| slot != 0) {
                break;
            }
        }
        assert_eq!(mission.current_stage, 1);

        player.position = Position {
            z: LEVEL1_INITIAL_ZONE_SWITCH_CENTER.z,
            ..LEVEL1_SECOND_WAYPOINT
        };
        for _ in 0..4 {
            mission.step(&FlatWorld, &player);
            if mission
                .actors()
                .iter()
                .any(|actor| actor.kind == Level1ActorKind::Archer && actor.active)
            {
                break;
            }
        }
        assert_eq!(mission.current_stage, 2);
        assert!(
            mission
                .actors()
                .iter()
                .any(|actor| actor.kind == Level1ActorKind::CastleSpellJar && actor.active)
        );
        assert_eq!(
            mission
                .actors()
                .iter()
                .filter(|actor| actor.kind == Level1ActorKind::Archer && actor.active)
                .count(),
            4
        );

        // FlatWorld puts the jar at z=0; descend into its source pickup
        // volume after the switch-chain assertions.
        player.position.z = 0;
        for _ in 0..4 {
            mission.step(&FlatWorld, &player);
            if mission.has_castle_spell {
                break;
            }
        }
        assert!(mission.has_castle_spell);
        let castle_holder = mission
            .actors()
            .iter()
            .find(|actor| actor.kind == Level1ActorKind::CastleSpellJar)
            .unwrap();
        assert!(castle_holder.active);
        assert!(castle_holder.collected);
        assert_eq!(castle_holder.action, 6);
        assert_eq!(castle_holder.state_timer, 64);
    }

    #[test]
    fn fireball_uses_source_constructor_constants_and_kills_after_five_hits() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.activate_group(3);
        let target = mission
            .actors()
            .iter()
            .find(|actor| actor.kind == Level1ActorKind::Archer)
            .copied()
            .unwrap();
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: target.position.x,
                y: target
                    .position
                    .y
                    .wrapping_add(FIREBALL_SPEED.cast_unsigned()),
                z: target.position.z.wrapping_add(100),
            },
            yaw: 0,
            effective_pitch: 0,
            ..FlightState::default()
        };

        for _ in 0..5 {
            assert!(mission.spawn_fireball(&player));
            mission.step(&FlatWorld, &player);
        }
        // Reclaimed lower slots can place a newly created impact behind the
        // current cursor, so its mailbox submission and the target's later
        // consumption may require two following source passes.
        for _ in 0..3 {
            mission.step(&FlatWorld, &player);
            if mission
                .actors()
                .iter()
                .find(|actor| actor.authored_index == target.authored_index)
                .is_some_and(|actor| actor.life < 0)
            {
                break;
            }
        }
        let target = mission
            .actors()
            .iter()
            .find(|actor| actor.authored_index == target.authored_index)
            .unwrap();
        let target_index = target.authored_index;
        assert_eq!(target.life, -250);
        assert!(target.active);
        assert_eq!(target.action, 36);

        for _ in 0..9 {
            mission.step(&FlatWorld, &player);
        }
        let target = mission
            .actors()
            .iter()
            .find(|actor| actor.authored_index == target_index)
            .unwrap();
        assert!(!target.active);
        assert_eq!(target.action, 37);
    }

    #[test]
    fn archer_constructor_cadence_acquires_and_spawns_source_shaped_arrow() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.activate_group(3);
        let first = mission
            .actors()
            .iter()
            .find(|actor| actor.authored_index == 101)
            .copied()
            .unwrap();
        assert_eq!(first.source_entity_index, 1);
        assert_eq!(first.phase, 0);
        assert_eq!(first.actual_speed, ARCHER_MIN_SPEED);
        let expected_rng = next_rng16(1);
        assert_eq!(first.entity_rng, expected_rng);
        assert_eq!(first.yaw, ((expected_rng & 0x07ff) as i16).wrapping_sub(1));

        let mut target = first.position;
        move_entity(&mut target, first.yaw, 0, 1_024);
        let player = FlightState {
            entity_index: 152,
            position: target,
            ..FlightState::default()
        };
        mission.step(&FlatWorld, &player);
        let first = mission
            .actors()
            .iter()
            .find(|actor| actor.authored_index == 101)
            .unwrap();
        assert_eq!(first.action, 34);
        assert_eq!(first.actual_speed, 0);
        assert!(matches!(first.render_parameter_index, Some(1 | 206)));

        let mut arrow_id = None;
        for _ in 0..ARCHER_DECISION_CADENCE {
            mission.step(&FlatWorld, &player);
            if let Some(Level1MissionEvent::ArcherArrowSpawned { projectile_id, .. }) = mission
                .events()
                .iter()
                .find(|event| matches!(event, Level1MissionEvent::ArcherArrowSpawned { .. }))
            {
                arrow_id = Some(*projectile_id);
                break;
            }
        }
        let arrow_id = arrow_id.expect("Archer attack cadence must create an arrow");
        assert!(mission.projectiles().iter().any(|projectile| {
            matches!(
                projectile,
                Level1Projectile::ArcherArrow(arrow)
                    if arrow.id == arrow_id
                        && arrow.speed == ARCHER_ARROW_SPEED
                        && arrow.life_ticks <= ARCHER_ARROW_LIFETIME_TICKS
                        && arrow.payload == ARCHER_ARROW_PAYLOAD
            )
        }));
    }

    #[test]
    fn archer_ground_clamp_and_avoidance_follow_source_heading_order() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut actor = template
            .actors()
            .iter()
            .find(|actor| actor.kind == Level1ActorKind::Archer)
            .copied()
            .unwrap();
        actor.position = Position {
            x: 0x10f8,
            y: 0x1080,
            z: 256,
        };
        actor.yaw = 512;
        actor.desired_heading = 512;
        actor.actual_speed = 30;

        let mut direct = actor.position;
        relax_to_terrain(&mut direct, &FlatWorld, 0, -128);
        move_entity(&mut direct, actor.yaw, 0, actor.actual_speed);
        let world = OneBlockedTileWorld {
            blocked_x: (direct.x >> 8) as u8,
            blocked_y: (direct.y >> 8) as u8,
        };
        assert_ne!(
            (actor.position.x >> 8, actor.position.y >> 8),
            (direct.x >> 8, direct.y >> 8)
        );

        let expected_heading = [
            actor.yaw.wrapping_add(341) & 0x07ff,
            archer_second_avoidance_heading(actor.yaw),
            archer_opposite_heading(actor.yaw),
        ]
        .into_iter()
        .find(|heading| {
            let mut candidate = actor.position;
            relax_to_terrain(&mut candidate, &world, 0, -128);
            move_entity(&mut candidate, *heading, 0, actor.actual_speed);
            let changed_tile = (actor.position.x >> 8, actor.position.y >> 8)
                != (candidate.x >> 8, candidate.y >> 8);
            !changed_tile || archer_candidate_is_valid(&world, candidate)
        })
        .expect("one source avoidance heading is clear");
        assert_eq!(move_archer(&mut actor, &world), ArcherMoveResult::Alternate);
        assert_eq!(actor.position.z, 128);
        assert_ne!(
            (actor.position.x >> 8, actor.position.y >> 8),
            (direct.x >> 8, direct.y >> 8)
        );
        assert_eq!(
            actor.yaw,
            expected_heading.wrapping_add(turn_toward(
                expected_heading,
                actor.desired_heading,
                ARCHER_TURN_LIMIT,
            )) & 0x07ff
        );
    }

    #[test]
    fn archer_altitude_descends_by_128_and_clamps_to_ground_zero() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut actor = template
            .actors()
            .iter()
            .find(|actor| actor.kind == Level1ActorKind::Archer)
            .copied()
            .unwrap();
        actor.position = Position {
            x: 10_000,
            y: 10_000,
            z: 300,
        };
        actor.actual_speed = 0;

        assert_eq!(
            move_archer(&mut actor, &FlatWorld),
            ArcherMoveResult::DirectSameTile
        );
        assert_eq!(actor.position.z, 172);
        move_archer(&mut actor, &FlatWorld);
        assert_eq!(actor.position.z, 44);
        move_archer(&mut actor, &FlatWorld);
        assert_eq!(actor.position.z, 0);
    }

    #[test]
    fn archer_slope_threshold_accepts_19_and_rejects_20() {
        let position = Position {
            x: 0x1080,
            y: 0x1080,
            z: 0,
        };
        let slope_19 = OneCornerSlopeWorld {
            tile_x: 0x10,
            tile_y: 0x10,
            raw_height: 19,
        };
        let slope_20 = OneCornerSlopeWorld {
            raw_height: 20,
            ..slope_19
        };
        assert_eq!(tile_slope_metric(&slope_19, position), 19);
        assert!(archer_candidate_is_valid(&slope_19, position));
        assert_eq!(tile_slope_metric(&slope_20, position), 20);
        assert!(!archer_candidate_is_valid(&slope_20, position));
    }

    #[test]
    fn archer_exhausting_all_source_candidates_sets_life_to_minus_one() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut actor = template
            .actors()
            .iter()
            .find(|actor| actor.kind == Level1ActorKind::Archer)
            .copied()
            .unwrap();
        actor.position = Position {
            x: 0x1080,
            y: 0x1080,
            z: 0,
        };
        actor.yaw = 0;
        actor.desired_heading = 0;
        actor.actual_speed = 384;

        assert_eq!(
            move_archer(&mut actor, &UniformMaterialWorld { material_mask: 1 }),
            ArcherMoveResult::Blocked
        );
        assert_eq!(
            actor.position,
            Position {
                x: 0x1080,
                y: 0x1080,
                z: 0
            }
        );
        assert_eq!(actor.yaw, 0x0400);
        assert_eq!(actor.life, -1);
    }

    #[test]
    fn archer_byte_preserving_avoidance_headings_match_reconstructed_c() {
        assert_eq!(archer_second_avoidance_heading(0x0123), 0x07ce);
        assert_eq!(archer_second_avoidance_heading(0x0000), 0x06ab);
        assert_eq!(archer_opposite_heading(0x0123), 0x0523);
        assert_eq!(archer_opposite_heading(0x07ff), 0x03ff);
    }

    #[test]
    fn archer_roam_links_nearest_unlinked_peer_when_no_target_is_available() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut archers = template
            .actors()
            .iter()
            .filter(|actor| actor.kind == Level1ActorKind::Archer)
            .copied()
            .take(2)
            .collect::<Vec<_>>();
        archers[0].active = true;
        archers[0].source_entity_index = 10;
        archers[0].position = Position {
            x: 10_000,
            y: 10_000,
            z: 0,
        };
        archers[0].yaw = 0;
        archers[0].desired_heading = 0;
        archers[0].phase = 0;
        archers[0].actual_speed = 0;
        archers[1].active = true;
        archers[1].source_entity_index = 11;
        archers[1].position = Position {
            x: 10_000,
            y: 9_488,
            z: 0,
        };
        let views = archers.clone();
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 40_000,
                y: 40_000,
                z: 0,
            },
            ..FlightState::default()
        };

        step_archer_roam(&mut archers[0], &FlatWorld, &player, &views);
        assert_eq!(archers[0].action, 35);
        assert_eq!(archers[0].linked_entity_index, 11);
    }

    #[test]
    fn linked_archer_copies_peer_target_and_enters_attack_pose_on_source_cadence() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut archers = template
            .actors()
            .iter()
            .filter(|actor| actor.kind == Level1ActorKind::Archer)
            .copied()
            .take(2)
            .collect::<Vec<_>>();
        archers[0].active = true;
        archers[0].source_entity_index = 10;
        archers[0].action = 35;
        archers[0].linked_entity_index = 11;
        archers[0].phase = ARCHER_DECISION_CADENCE;
        archers[0].actual_speed = ARCHER_MIN_SPEED;
        archers[1].active = true;
        archers[1].source_entity_index = 11;
        archers[1].action = 34;
        archers[1].target_entity_index = 152;
        let views = archers.clone();

        step_archer_linked(&mut archers[0], &FlatWorld, &views);
        assert_eq!(archers[0].action, 34);
        assert_eq!(archers[0].target_entity_index, 152);
        assert_eq!(archers[0].linked_entity_index, 0);
        assert_eq!(archers[0].actual_speed, 0);
        assert!(matches!(archers[0].render_parameter_index, Some(1 | 206)));
    }

    #[test]
    fn linked_archer_damage_immediately_alerts_valid_partner() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.activate_group(3);
        let archer_indices = mission
            .actors
            .iter()
            .enumerate()
            .filter_map(|(index, actor)| (actor.kind == Level1ActorKind::Archer).then_some(index))
            .take(2)
            .collect::<Vec<_>>();
        let follower_index = archer_indices[0];
        let partner_index = archer_indices[1];
        let partner_slot = mission.actors[partner_index].source_entity_index;
        mission.actors[follower_index].action = 35;
        mission.actors[follower_index].linked_entity_index = partner_slot;
        mission.actors[follower_index].pending_damage = 250;
        mission.actors[follower_index].pending_attacker = 152;
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 40_000,
                y: 40_000,
                z: 0,
            },
            ..FlightState::default()
        };

        mission.step(&FlatWorld, &player);
        let follower = mission.actors[follower_index];
        let partner = mission.actors[partner_index];
        assert_eq!(follower.action, 34);
        assert_eq!(follower.target_entity_index, 152);
        assert_eq!(follower.linked_entity_index, 0);
        assert_eq!(partner.action, 34);
        assert_eq!(partner.target_entity_index, 152);
        assert_eq!(partner.linked_entity_index, 0);
    }

    #[test]
    fn authored_allocation_only_records_preserve_source_slot_order_and_reuse() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);

        mission.activate_group(1);
        assert_eq!(mission.allocation_slot(31), Some(1));
        assert_eq!(mission.allocation_slot(99), Some(2));
        assert_eq!(mission.allocation_slot(175), Some(21));

        // The triggering switch remains live while its target group allocates.
        mission.activate_group(2);
        assert_eq!(mission.allocation_slot(105), Some(22));
        assert_eq!(mission.allocation_slot(231), Some(23));

        // Disable happens only after nested activation. The following source
        // cleanup makes slot 2 the lowest free slot for group 3.
        mission.mark_allocation_pending_delete(99);
        mission.activate_group(3);
        assert_eq!(mission.allocation_slot(99), Some(0));
        assert_eq!(mission.allocation_slot(101), Some(2));
        assert_eq!(mission.allocation_slot(102), Some(24));
        assert_eq!(mission.allocation_slot(103), Some(25));
        assert_eq!(mission.allocation_slot(106), Some(26));
        assert_eq!(mission.allocation_slot(109), Some(27));
        assert_eq!(mission.allocation_slot(153), Some(28));
        assert_eq!(
            mission
                .actors()
                .iter()
                .find(|actor| actor.authored_index == 101)
                .unwrap()
                .source_entity_index,
            2
        );
    }

    #[test]
    fn archer_absence_drives_group5_immediately_and_speed_jar_after_source_delay() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 4_000);
        mission.activate_group(3);
        mission.objective_states = [2, 2, 1, 1, 1];
        mission.current_stage = 2;
        mission.current_text_selector = 50;

        // idx109 arms to 16 in the G3 spawn update because that update's
        // rebuilt model-4 list predates the newly allocated Archers.
        mission
            .authored_allocations
            .iter_mut()
            .find(|allocation| allocation.authored_index == 109)
            .unwrap()
            .state_timer = 16;
        let archer_indices = mission
            .actors
            .iter_mut()
            .filter_map(|actor| {
                (actor.kind == Level1ActorKind::Archer).then(|| {
                    actor.active = false;
                    actor.authored_index
                })
            })
            .collect::<Vec<_>>();
        for authored_index in archer_indices {
            mission.mark_allocation_pending_delete(authored_index);
        }

        mission.step(&FlatWorld, &FlightState::default());
        assert_eq!(mission.objective_states[2], 2);
        assert_eq!(mission.current_stage, 3);
        assert_eq!(
            mission
                .authored_allocations
                .iter()
                .find(|allocation| allocation.authored_index == 109)
                .unwrap()
                .state_timer,
            15
        );

        // The next update's objective switch idx153 creates G5. Its sole
        // switch normally reuses a lower slot and therefore waits for the
        // following source scan before it can create G6.
        mission.step(&FlatWorld, &FlightState::default());
        assert!(mission.allocation_slot(190).is_some_and(|slot| slot != 0));
        assert_eq!(mission.count_alive(Level1ActorKind::Firefly), 0);

        mission.objective_states[3] = 2;
        mission.step(&FlatWorld, &FlightState::default());
        assert_eq!(mission.count_alive(Level1ActorKind::Firefly), 5);

        for _ in 0..12 {
            mission.step(&FlatWorld, &FlightState::default());
            assert!(
                !mission
                    .actors()
                    .iter()
                    .any(|actor| actor.kind == Level1ActorKind::SpeedSpellJar && actor.active)
            );
        }
        mission.step(&FlatWorld, &FlightState::default());
        assert!(
            mission
                .actors()
                .iter()
                .any(|actor| actor.kind == Level1ActorKind::SpeedSpellJar && actor.active)
        );
    }

    #[test]
    fn fireflies_bind_to_level1_stage_row_and_source_constructor_values() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.activate_group(6);
        let fireflies = mission
            .actors()
            .iter()
            .filter(|actor| actor.kind == Level1ActorKind::Firefly && actor.active)
            .collect::<Vec<_>>();
        assert_eq!(fireflies.len(), 5);
        for (phase, firefly) in fireflies.into_iter().enumerate() {
            assert!(firefly.active);
            assert_eq!(firefly.action, FIREFLY_STAGE_ACTION);
            assert_eq!(firefly.stage_var_1, 8);
            assert_eq!(firefly.stage_var_2, 1);
            assert_eq!(firefly.max_life, FIREFLY_LIFE);
            assert_eq!(firefly.life, FIREFLY_LIFE);
            assert_eq!(firefly.min_speed, FIREFLY_MIN_SPEED);
            assert_eq!(firefly.actual_speed, FIREFLY_MIN_SPEED);
            assert_eq!(firefly.max_speed, FIREFLY_MAX_SPEED);
            assert_eq!(firefly.phase, phase as u8);
            assert_eq!(firefly.collision_half_width, 85);
            assert_eq!(firefly.collision_vertical_offset, 100);
            assert_eq!(firefly.collision_half_height, 51);
        }
    }

    #[test]
    fn firefly_stage_patrol_moves_before_eight_tick_steering_and_rng() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut actor = template
            .actors()
            .iter()
            .find(|actor| actor.kind == Level1ActorKind::Firefly)
            .copied()
            .unwrap();
        actor.active = true;
        actor.source_entity_index = 1;
        actor.position = Position {
            x: 40_000,
            y: 40_000,
            z: 0,
        };
        actor.phase = 0;
        actor.yaw = 0;
        actor.desired_heading = 0;
        actor.actual_speed = FIREFLY_MIN_SPEED;
        actor.entity_rng = next_rng16(1);
        let views = [actor];
        let expected_rng = next_rng16(next_rng16(actor.entity_rng));

        step_firefly_stage_patrol(&mut actor, &FlatWorld, &views);

        assert_eq!(actor.position.z, FIREFLY_GROUND_CLEARANCE);
        assert_ne!(
            (actor.position.x, actor.position.y),
            (views[0].position.x, views[0].position.y)
        );
        assert_eq!(actor.entity_rng, expected_rng);
        assert_eq!(actor.action, FIREFLY_STAGE_ACTION);
    }

    #[test]
    fn firefly_stage_arrival_hands_off_to_roam_before_same_turn_dispatch() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.activate_group(6);
        let actor_index = mission
            .actors
            .iter()
            .position(|actor| actor.kind == Level1ActorKind::Firefly)
            .unwrap();
        let source_slot = mission.actors[actor_index].source_entity_index;
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: FIREFLY_STAGE_TARGET.x.wrapping_add(1_000),
                y: FIREFLY_STAGE_TARGET.y,
                z: FIREFLY_STAGE_TARGET.z,
            },
            ..FlightState::default()
        };
        {
            let actor = &mut mission.actors[actor_index];
            actor.position = FIREFLY_STAGE_TARGET;
            actor.phase = FIREFLY_DECISION_CADENCE;
            actor.yaw =
                i16::try_from(heading_for_positions(actor.position, player.position)).unwrap();
            actor.desired_heading = actor.yaw;
            actor.action = FIREFLY_STAGE_ACTION;
            actor.stage_var_1 = 8;
            actor.stage_var_2 = 1;
        }

        mission.begin_source_turn();

        let actor = mission.actors[actor_index];
        assert_eq!(actor.action, FIREFLY_ROAM_ACTION);
        assert_eq!(actor.stage_var_1, 0);
        assert_eq!(actor.stage_var_2, 0);
        assert_eq!(actor.linked_entity_index, 0);

        mission.dispatch_source_slot(&FlatWorld, &player, source_slot, false, false, None);

        assert_eq!(
            mission.actors[actor_index].action, FIREFLY_ATTACK_ACTION,
            "the newly selected roam action must acquire the visible player on the same source turn"
        );
        assert_eq!(mission.actors[actor_index].attack_substate, 0);
        assert_eq!(
            mission.source_slots[usize::from(source_slot)].action,
            FIREFLY_ATTACK_ACTION
        );
    }

    #[test]
    fn hostile_hit_enters_firefly_attack_and_death_waits_for_phase_gate() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.activate_group(6);
        let actor_index = mission
            .actors
            .iter()
            .position(|actor| actor.kind == Level1ActorKind::Firefly)
            .unwrap();
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 60_000,
                y: 47_000,
                z: 1_000,
            },
            ..FlightState::default()
        };
        mission.actors[actor_index].pending_damage = 250;
        mission.actors[actor_index].pending_attacker = player.entity_index;
        mission.step(&FlatWorld, &player);
        assert_eq!(mission.actors[actor_index].life, 350);
        assert_eq!(mission.actors[actor_index].action, FIREFLY_ATTACK_ACTION);
        assert_eq!(
            mission.actors[actor_index].attack_substate, 7,
            "every source-valid surviving hit forces the Firefly charge branch"
        );

        mission.actors[actor_index].pending_damage = 500;
        mission.actors[actor_index].pending_attacker = player.entity_index;
        mission.step(&FlatWorld, &player);
        assert!(mission.actors[actor_index].life < 0);
        assert_eq!(mission.actors[actor_index].action, FIREFLY_PRE_KILL_ACTION);
        mission.step(&FlatWorld, &player);
        assert_eq!(mission.actors[actor_index].action, FIREFLY_KILL_ACTION);
        for _ in 0..8 {
            if !mission.actors[actor_index].active {
                break;
            }
            mission.step(&FlatWorld, &player);
        }
        assert!(!mission.actors[actor_index].active);
    }

    #[test]
    fn firefly_ranged_and_charge_attacks_use_distinct_source_constants() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut actor = template
            .actors()
            .iter()
            .find(|actor| actor.kind == Level1ActorKind::Firefly)
            .copied()
            .unwrap();
        actor.active = true;
        actor.source_entity_index = 10;
        actor.position = Position {
            x: 20_000,
            y: 20_000,
            z: 1_000,
        };
        actor.target_entity_index = 152;
        actor.action = FIREFLY_ATTACK_ACTION;
        actor.attack_substate = 5;
        actor.phase = FIREFLY_DECISION_CADENCE;
        actor.actual_speed = 0;
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 20_000,
                y: 18_000,
                z: 1_000,
            },
            ..FlightState::default()
        };
        let views = [actor];
        let (projectile, sound, damage) =
            step_firefly_attack(&mut actor, &FlatWorld, &player, &views, 77);
        let projectile = projectile.expect("state 5 must fire in range on cadence");
        assert_eq!(sound, None);
        assert_eq!(damage, 0);
        assert_eq!(projectile.id, 77);
        assert_eq!(projectile.speed, FIREFLY_FIREBALL_SPEED);
        assert_eq!(projectile.life_ticks, FIREFLY_FIREBALL_LIFETIME_TICKS);
        assert_eq!(projectile.payload, FIREFLY_FIREBALL_PAYLOAD);
        assert_eq!(projectile.position.z, actor.position.z + 51);
        assert_eq!(projectile.target_entity_index, player.entity_index);
        assert_eq!(
            projectile.pitch,
            pitch_for_delta(
                i32::try_from(legacy_sqrt(squared_wrapping_xy_distance(
                    actor.position,
                    player.position,
                )))
                .unwrap(),
                i32::from(player.position.z.wrapping_sub(actor.position.z)),
            ),
            "source calculates aim from the Firefly origin before raising the projectile by 51"
        );

        actor.position = player.position;
        actor.actual_speed = 0;
        actor.attack_substate = 7;
        actor.state_value = 0;
        let rng_before = actor.entity_rng;
        let (_, sound, damage) = step_firefly_attack(&mut actor, &FlatWorld, &player, &views, 78);
        assert_eq!(sound, Some(43 + (next_rng16(rng_before) & 1)));
        assert_eq!(damage, FIREFLY_CONTACT_DAMAGE);
        assert_eq!(actor.attack_substate, 9);
        let (_, _, repeated_damage) =
            step_firefly_attack(&mut actor, &FlatWorld, &player, &views, 79);
        assert_eq!(repeated_damage, 0);
    }

    #[test]
    fn firefly_fireball_uses_zero_width_and_strict_source_collision_boundaries() {
        let projectile = Position {
            x: 20_000,
            y: 20_000,
            z: 1_050,
        };
        let touching_player = Position {
            x: projectile
                .x
                .wrapping_add(PLAYER_COLLISION_HALF_WIDTH.cast_unsigned()),
            y: projectile.y,
            z: 1_000,
        };
        assert!(!overlaps_source_entities(
            projectile,
            FIREBALL_COLLISION_HALF_WIDTH,
            FIREBALL_COLLISION_VERTICAL_OFFSET,
            FIREBALL_COLLISION_HALF_HEIGHT,
            touching_player,
            PLAYER_COLLISION_HALF_WIDTH,
            PLAYER_COLLISION_VERTICAL_OFFSET,
            PLAYER_COLLISION_HALF_HEIGHT,
        ));
        assert!(overlaps_source_entities(
            projectile,
            FIREBALL_COLLISION_HALF_WIDTH,
            FIREBALL_COLLISION_VERTICAL_OFFSET,
            FIREBALL_COLLISION_HALF_HEIGHT,
            Position {
                x: touching_player.x.wrapping_sub(1),
                ..touching_player
            },
            PLAYER_COLLISION_HALF_WIDTH,
            PLAYER_COLLISION_VERTICAL_OFFSET,
            PLAYER_COLLISION_HALF_HEIGHT,
        ));
    }

    #[test]
    fn firefly_fireball_water_hit_creates_nine_update_sound_only_splash() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        let projectile_id = 77;
        let projectile_slot = mission
            .source_pool
            .allocate(SourceOccupant::FireflyFireball(projectile_id))
            .expect("Firefly projectile source slot");
        mission.projectiles.push(Level1Projectile::FireflyFireball(
            FireflyFireballProjectile {
                id: projectile_id,
                source_entity_index: projectile_slot,
                owner_entity_index: 10,
                target_entity_index: 152,
                position: Position {
                    x: 20_000,
                    y: 20_000,
                    z: 50,
                },
                yaw: 0,
                pitch: 0,
                speed: 0,
                life_ticks: FIREFLY_FIREBALL_LIFETIME_TICKS,
                payload: FIREFLY_FIREBALL_PAYLOAD,
                active: true,
            },
        ));
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 30_000,
                y: 30_000,
                z: 1_000,
            },
            ..FlightState::default()
        };

        mission.step_projectiles(
            &RaisedMaterialWorld {
                ground_height: 100,
                material_mask: 1,
            },
            &player,
        );

        assert!(!mission.projectiles[0].active());
        assert_eq!(mission.impacts.len(), 1);
        let splash_slot = mission.impacts[0].source_entity_index;
        let splash_id = mission.impacts[0].id;
        assert_eq!(mission.impacts[0].model, WATER_SPLASH_MODEL);
        assert_eq!(mission.impacts[0].position.z, 100);
        assert_eq!(mission.impacts[0].payload, 0);
        assert_eq!(
            mission.source_pool.occupant(splash_slot),
            Some(SourceOccupant::WaterSplash(splash_id))
        );
        assert!(
            mission
                .events
                .iter()
                .all(|event| !matches!(event, Level1MissionEvent::FireballImpacted { .. }))
        );
        assert_eq!(
            mission.source_pool.occupant(projectile_slot),
            Some(SourceOccupant::FireflyFireball(projectile_id))
        );
        assert!(mission.source_pool.disabled[usize::from(projectile_slot)]);

        mission.step_impact_slot(
            &RaisedMaterialWorld {
                ground_height: 100,
                material_mask: 1,
            },
            &player,
            splash_slot,
        );
        assert_eq!(mission.impacts[0].life_ticks, 7);
        assert_eq!(mission.impacts[0].animation_frame, 1);
        assert_eq!(mission.player_pending_damage, 0);
        assert_eq!(
            mission
                .events
                .iter()
                .filter(|event| matches!(
                    event,
                    Level1MissionEvent::WaterSplashed {
                        projectile_id: 77,
                        sound_id: WATER_SPLASH_SOUND,
                        ..
                    }
                ))
                .count(),
            1
        );
        for _ in 0..8 {
            mission.step_impact_slot(&FlatWorld, &player, splash_slot);
        }
        assert_eq!(mission.impacts[0].life_ticks, -1);
        assert_eq!(mission.impacts[0].animation_frame, 9);
        assert!(mission.impacts[0].active);
        mission.step_impact_slot(&FlatWorld, &player, splash_slot);
        assert!(!mission.impacts[0].active);
        assert!(mission.source_pool.disabled[usize::from(splash_slot)]);
        assert_eq!(
            mission
                .events
                .iter()
                .filter(|event| matches!(event, Level1MissionEvent::WaterSplashed { .. }))
                .count(),
            1
        );

        mission.cleanup_pending_allocations();
        assert_eq!(mission.source_pool.occupant(projectile_slot), None);
        assert_eq!(mission.source_pool.occupant(splash_slot), None);
    }

    #[test]
    fn shared_actor_collision_precedes_water_and_creates_damaging_impact() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        let projectile_id = 77;
        let projectile_slot = mission
            .source_pool
            .allocate(SourceOccupant::PlayerFireball(projectile_id))
            .expect("player projectile source slot");
        let target_position = Position {
            x: 20_000,
            y: 20_000,
            z: 0,
        };
        mission.set_external_collision_targets([Level1CollisionTarget {
            entity_index: 50,
            position: target_position,
            collision_half_width: 128,
            collision_vertical_offset: 75,
            collision_half_height: 128,
            active: true,
        }]);
        mission
            .projectiles
            .push(Level1Projectile::PlayerFireball(FireballProjectile {
                id: projectile_id,
                source_entity_index: projectile_slot,
                owner_entity_index: 152,
                position: Position {
                    z: 50,
                    ..target_position
                },
                yaw: 0,
                pitch: 0,
                speed: 0,
                life_ticks: FIREBALL_LIFETIME_TICKS,
                payload: FIREBALL_PAYLOAD,
                active: true,
            }));
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 30_000,
                y: 30_000,
                z: 1_000,
            },
            ..FlightState::default()
        };

        mission.step_projectiles(
            &RaisedMaterialWorld {
                ground_height: 100,
                material_mask: 1,
            },
            &player,
        );

        assert!(!mission.projectiles[0].active());
        assert_eq!(mission.impacts.len(), 1);
        assert_eq!(mission.impacts[0].model, FIREBALL_IMPACT_MODEL);
        assert_eq!(mission.impacts[0].position, target_position);
        assert_eq!(mission.impacts[0].external_target_entity_index, 50);
        assert!(
            mission
                .events
                .iter()
                .all(|event| !matches!(event, Level1MissionEvent::WaterSplashed { .. }))
        );
    }

    #[test]
    fn firefly_fireball_retries_impact_when_source_pool_is_exhausted() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        let projectile_id = 78;
        let projectile_slot = mission
            .source_pool
            .allocate(SourceOccupant::FireflyFireball(projectile_id))
            .expect("Firefly projectile source slot");
        mission.source_pool.free_stack.clear();
        mission.projectiles.push(Level1Projectile::FireflyFireball(
            FireflyFireballProjectile {
                id: projectile_id,
                source_entity_index: projectile_slot,
                owner_entity_index: 10,
                target_entity_index: 152,
                position: Position {
                    x: 20_000,
                    y: 20_000,
                    z: 50,
                },
                yaw: 0,
                pitch: 0,
                speed: 0,
                life_ticks: FIREFLY_FIREBALL_LIFETIME_TICKS,
                payload: FIREFLY_FIREBALL_PAYLOAD,
                active: true,
            },
        ));
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 30_000,
                y: 30_000,
                z: 1_000,
            },
            ..FlightState::default()
        };

        mission.step_projectiles(
            &RaisedMaterialWorld {
                ground_height: 100,
                material_mask: 0,
            },
            &player,
        );

        assert!(mission.projectiles[0].active());
        assert!(mission.impacts.is_empty());
    }

    #[test]
    fn hostile_fireball_impact_damages_player_and_emits_sound_once() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        let impact_id = 80;
        let impact_slot = mission
            .source_pool
            .allocate(SourceOccupant::FireballImpact(impact_id))
            .expect("Fireball impact source slot");
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 20_000,
                y: 20_000,
                z: 1_000,
            },
            ..FlightState::default()
        };
        mission.set_external_collision_targets([Level1CollisionTarget {
            entity_index: 10,
            position: Position {
                x: 19_000,
                ..player.position
            },
            collision_half_width: 85,
            collision_vertical_offset: 100,
            collision_half_height: 51,
            active: true,
        }]);
        mission.impacts.push(FireballImpact {
            id: impact_id,
            source_entity_index: impact_slot,
            source_projectile_id: 79,
            owner_entity_index: 10,
            external_target_entity_index: 0,
            model: FIREBALL_IMPACT_MODEL,
            position: player.position,
            payload: FIREFLY_FIREBALL_PAYLOAD,
            life_ticks: 8,
            entity_rng: 123,
            vertical_velocity: 0,
            animation_frame: 0,
            damage_submitted: false,
            active: true,
        });

        mission.begin_source_turn();
        mission.dispatch_source_slot(&FlatWorld, &player, impact_slot, false, false, None);
        mission.dispatch_source_slot(&FlatWorld, &player, player.entity_index, false, false, None);

        assert_eq!(mission.player_life, 9_500);
        assert_eq!(mission.player_life_regen_cooldown, 15);
        assert_eq!(mission.impacts[0].life_ticks, 7);
        assert_eq!(
            mission
                .events
                .iter()
                .filter(|event| matches!(
                    event,
                    Level1MissionEvent::FireballImpacted {
                        projectile_id: 79,
                        impact_id: 80,
                        sound_id: 3,
                        ..
                    }
                ))
                .count(),
            1
        );
        assert_eq!(
            mission
                .events
                .iter()
                .filter(|event| matches!(
                    event,
                    Level1MissionEvent::PlayerDamaged {
                        damage: FIREFLY_FIREBALL_PAYLOAD,
                        life: 9_500,
                    }
                ))
                .count(),
            1
        );

        mission.clear_events();
        mission.dispatch_source_slot(&FlatWorld, &player, impact_slot, false, false, None);
        assert_eq!(mission.player_life, 9_500);
        assert!(mission.events.is_empty());
        for _ in 0..8 {
            mission.dispatch_source_slot(&FlatWorld, &player, impact_slot, false, false, None);
        }
        assert!(!mission.impacts[0].active);
        assert_eq!(
            mission.source_pool.occupant(impact_slot),
            Some(SourceOccupant::FireballImpact(80))
        );
        assert!(mission.source_pool.disabled[usize::from(impact_slot)]);
        mission.cleanup_pending_allocations();
        assert_eq!(mission.source_pool.occupant(impact_slot), None);
    }

    #[test]
    fn fireball_impact_advances_rng_motion_animation_and_exact_lifetime() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        let impact_id = 80;
        let impact_slot = mission
            .source_pool
            .allocate(SourceOccupant::FireballImpact(impact_id))
            .expect("Fireball impact source slot");
        let seed = 123;
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 40_000,
                y: 40_000,
                z: 1_000,
            },
            ..FlightState::default()
        };
        mission.impacts.push(FireballImpact {
            id: impact_id,
            source_entity_index: impact_slot,
            source_projectile_id: 79,
            owner_entity_index: 10,
            external_target_entity_index: 0,
            model: FIREBALL_IMPACT_MODEL,
            position: Position {
                x: 20_000,
                y: 20_000,
                z: 1_000,
            },
            payload: FIREFLY_FIREBALL_PAYLOAD,
            life_ticks: 8,
            entity_rng: seed,
            vertical_velocity: 0,
            animation_frame: 0,
            damage_submitted: false,
            active: true,
        });

        mission.step_impact_slot(&FlatWorld, &player, impact_slot);

        let expected_rng = next_rng16(seed);
        let expected_velocity =
            i16::try_from(expected_rng % 65).expect("impact velocity fits i16") - 32;
        assert_eq!(mission.impacts[0].life_ticks, 7);
        assert_eq!(mission.impacts[0].entity_rng, expected_rng);
        assert_eq!(mission.impacts[0].vertical_velocity, expected_velocity);
        assert_eq!(
            mission.impacts[0].position.z,
            1_000i16.wrapping_add(expected_velocity)
        );
        assert_eq!(mission.impacts[0].animation_frame, 1);
        assert_eq!(
            mission.source_slots[usize::from(impact_slot)].entity_rng,
            expected_rng
        );
        assert!(mission.impacts[0].damage_submitted);
        assert_eq!(
            mission
                .events
                .iter()
                .filter(|event| matches!(event, Level1MissionEvent::FireballImpacted { .. }))
                .count(),
            1
        );

        for _ in 0..8 {
            mission.step_impact_slot(&FlatWorld, &player, impact_slot);
        }
        assert_eq!(mission.impacts[0].life_ticks, -1);
        assert_eq!(mission.impacts[0].animation_frame, 9);
        assert!(mission.impacts[0].active);
        assert_eq!(
            mission
                .events
                .iter()
                .filter(|event| matches!(event, Level1MissionEvent::FireballImpacted { .. }))
                .count(),
            1
        );

        mission.step_impact_slot(&FlatWorld, &player, impact_slot);
        assert_eq!(mission.impacts[0].life_ticks, -2);
        assert_eq!(mission.impacts[0].animation_frame, 9);
        assert!(!mission.impacts[0].active);
        assert!(mission.source_pool.disabled[usize::from(impact_slot)]);
    }

    #[test]
    fn impact_area_mailbox_hits_every_overlapping_non_owner() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.activate_group(6);
        let fireflies = mission
            .actors
            .iter()
            .enumerate()
            .filter(|(_, actor)| actor.kind == Level1ActorKind::Firefly)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let owner_index = fireflies[0];
        let target_index = fireflies[1];
        let position = Position {
            x: 20_000,
            y: 20_000,
            z: 1_000,
        };
        mission.actors[owner_index].position = position;
        mission.actors[target_index].position = position;
        let owner_entity_index = mission.actors[owner_index].source_entity_index;
        let impact_id = 80;
        let impact_slot = mission
            .source_pool
            .allocate(SourceOccupant::FireballImpact(impact_id))
            .expect("impact source slot");
        mission.impacts.push(FireballImpact {
            id: impact_id,
            source_entity_index: impact_slot,
            source_projectile_id: 79,
            owner_entity_index,
            external_target_entity_index: 0,
            model: FIREBALL_IMPACT_MODEL,
            position,
            payload: FIREFLY_FIREBALL_PAYLOAD,
            life_ticks: 8,
            entity_rng: 123,
            vertical_velocity: 0,
            animation_frame: 0,
            damage_submitted: false,
            active: true,
        });
        let player = FlightState {
            entity_index: 152,
            position,
            ..FlightState::default()
        };

        mission.step_impact_slot(&FlatWorld, &player, impact_slot);

        assert_eq!(mission.actors[owner_index].pending_damage, 0);
        assert_eq!(
            mission.actors[target_index].pending_damage,
            FIREFLY_FIREBALL_PAYLOAD
        );
        assert_eq!(
            mission.actors[target_index].pending_attacker,
            owner_entity_index
        );
        assert_eq!(mission.player_pending_damage, FIREFLY_FIREBALL_PAYLOAD);
        assert_eq!(mission.player_pending_attacker, owner_entity_index);
    }

    #[test]
    fn firefly_contact_mailbox_overwrites_an_existing_payload() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);

        mission.submit_player_contact_damage(100, 20);
        assert_eq!(mission.player_pending_damage, 100);
        mission.submit_player_contact_damage(FIREFLY_CONTACT_DAMAGE, 10);

        assert_eq!(mission.player_pending_damage, FIREFLY_CONTACT_DAMAGE);
        assert_eq!(mission.player_pending_attacker, 10);
    }

    #[test]
    fn hostile_damage_mailbox_obeys_player_source_slot_order() {
        fn mission_with_impact(slot: u16) -> (Level1MissionState, FlightState) {
            let template = Level1MissionTemplate::decode_with_runtime_context(
                &authored_bytes(),
                &FlatWorld,
                19_577,
                0,
                &[slot],
            )
            .unwrap();
            let mut mission = template.session(10_000, 1_000);
            let impact_id = 80;
            let impact_slot = mission
                .source_pool
                .allocate(SourceOccupant::FireballImpact(impact_id))
                .expect("reserved test impact source slot");
            assert_eq!(impact_slot, slot);
            let player = FlightState {
                entity_index: 152,
                position: Position {
                    x: 20_000,
                    y: 20_000,
                    z: 1_000,
                },
                ..FlightState::default()
            };
            mission.set_external_collision_targets([Level1CollisionTarget {
                entity_index: 10,
                position: Position {
                    x: 19_000,
                    ..player.position
                },
                collision_half_width: 85,
                collision_vertical_offset: 100,
                collision_half_height: 51,
                active: true,
            }]);
            mission.impacts.push(FireballImpact {
                id: impact_id,
                source_entity_index: impact_slot,
                source_projectile_id: 79,
                owner_entity_index: 10,
                external_target_entity_index: 0,
                model: FIREBALL_IMPACT_MODEL,
                position: player.position,
                payload: FIREFLY_FIREBALL_PAYLOAD,
                life_ticks: 8,
                entity_rng: 123,
                vertical_velocity: 0,
                animation_frame: 0,
                damage_submitted: false,
                active: true,
            });
            (mission, player)
        }

        let (mut lower, player) = mission_with_impact(100);
        lower.begin_source_turn();
        for source_slot in 1..=player.entity_index {
            lower.dispatch_source_slot(&FlatWorld, &player, source_slot, false, false, None);
        }
        assert_eq!(lower.player_pending_damage, 0);
        assert_eq!(lower.player_pending_attacker, 0);
        assert_eq!(lower.player_life, 9_500);
        assert_eq!(lower.player_life_regen_cooldown, 15);

        let (mut higher, player) = mission_with_impact(200);
        higher.begin_source_turn();
        for source_slot in 1..ENTITY_POOL_SIZE {
            higher.dispatch_source_slot(
                &FlatWorld,
                &player,
                u16::try_from(source_slot).unwrap(),
                false,
                false,
                None,
            );
        }
        higher.finish_source_turn(&player);
        assert_eq!(higher.player_life, 10_000);
        assert_eq!(higher.player_pending_damage, FIREFLY_FIREBALL_PAYLOAD);
        assert_eq!(higher.player_pending_attacker, 10);

        higher.begin_source_turn();
        for source_slot in 1..=player.entity_index {
            higher.dispatch_source_slot(&FlatWorld, &player, source_slot, false, false, None);
        }
        assert_eq!(higher.player_pending_damage, 0);
        assert_eq!(higher.player_pending_attacker, 0);
        assert_eq!(higher.player_life, 9_500);
        assert_eq!(higher.player_life_regen_cooldown, 15);
    }

    #[test]
    fn lethal_hostile_mailbox_skips_regeneration_and_session_reset_restores_player() {
        let template = Level1MissionTemplate::decode_with_runtime_context(
            &authored_bytes(),
            &FlatWorld,
            19_577,
            0,
            &[100],
        )
        .unwrap();
        let mut mission = template.session(499, 1_000);
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 20_000,
                y: 20_000,
                z: 1_000,
            },
            ..FlightState::default()
        };
        mission.set_external_collision_targets([Level1CollisionTarget {
            entity_index: 10,
            position: Position {
                x: 19_000,
                ..player.position
            },
            collision_half_width: 85,
            collision_vertical_offset: 100,
            collision_half_height: 51,
            active: true,
        }]);
        let impact_id = 80;
        let impact_slot = mission
            .source_pool
            .allocate(SourceOccupant::FireballImpact(impact_id))
            .expect("lower hostile impact source slot");
        assert_eq!(impact_slot, 100);
        mission.impacts.push(FireballImpact {
            id: impact_id,
            source_entity_index: impact_slot,
            source_projectile_id: 79,
            owner_entity_index: 10,
            external_target_entity_index: 0,
            model: FIREBALL_IMPACT_MODEL,
            position: player.position,
            payload: FIREFLY_FIREBALL_PAYLOAD,
            life_ticks: 8,
            entity_rng: 123,
            vertical_velocity: 0,
            animation_frame: 0,
            damage_submitted: false,
            active: true,
        });

        mission.step(&FlatWorld, &player);

        assert_eq!(mission.player_life, -1);
        assert!(mission.player_dead);
        assert!(mission.events.iter().any(|event| matches!(
            event,
            Level1MissionEvent::PlayerDamaged {
                damage: FIREFLY_FIREBALL_PAYLOAD,
                life: -1,
            }
        )));
        assert!(mission.events.contains(&Level1MissionEvent::PlayerDied));

        let reset = template.session(499, 1_000);
        assert_eq!(reset.player_life, 499);
        assert_eq!(reset.player_pending_damage, 0);
        assert_eq!(reset.player_pending_attacker, 0);
        assert_eq!(reset.player_life_regen_cooldown, 0);
        assert!(!reset.player_dead);
    }

    #[test]
    fn player_hit_reaction_matches_source_rng_recoil_timers_and_mailbox_clear() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 20_000,
                y: 20_000,
                z: 1_000,
            },
            ..FlightState::default()
        };
        let attacker_position = Position {
            x: 19_000,
            y: 20_000,
            z: 500,
        };
        mission.set_external_collision_targets([Level1CollisionTarget {
            entity_index: 10,
            position: attacker_position,
            collision_half_width: 85,
            collision_vertical_offset: 100,
            collision_half_height: 51,
            active: true,
        }]);
        let seed = 0x1234;
        mission.source_slots[usize::from(player.entity_index)].entity_rng = seed;
        mission.submit_player_damage(FIREFLY_FIREBALL_PAYLOAD, 10);

        mission.begin_source_turn();
        mission.dispatch_source_slot(&FlatWorld, &player, player.entity_index, false, false, None);

        let expected_rng = next_rng16(seed);
        let reaction = mission
            .take_player_hit_reaction()
            .expect("player source dispatch installs one reaction");
        assert_eq!(
            reaction,
            PlayerHitReaction {
                attacker_entity_index: 10,
                raw_damage: FIREFLY_FIREBALL_PAYLOAD,
                applied_damage: FIREFLY_FIREBALL_PAYLOAD,
                recoil_yaw: i16::try_from(heading_for_positions(
                    attacker_position,
                    player.position
                ))
                .unwrap(),
                recoil_pitch: pitch_for_delta(
                    i32::try_from(wrapping_xy_distance(attacker_position, player.position))
                        .unwrap(),
                    i32::from(player.position.z.wrapping_sub(attacker_position.z)),
                ),
                recoil_speed: 50,
                pain_sound_id: PLAYER_PAIN_SOUND_BASE + (expected_rng & 3),
                hit_frame_ticks: PLAYER_HIT_FRAME_TICKS,
                regen_block_ticks: PLAYER_REGEN_BLOCK_TICKS,
                hit_aux_ticks: PLAYER_HIT_AUX_TICKS,
                survived_feedback: PLAYER_SURVIVED_FEEDBACK,
                killer_entity_index: 0,
                lethal: false,
            }
        );
        assert_eq!(
            mission.source_slots[usize::from(player.entity_index)].entity_rng,
            expected_rng
        );
        assert_eq!(mission.player_life, 9_500);
        assert_eq!(mission.player_pending_damage, 0);
        assert_eq!(mission.player_pending_attacker, 0);
        assert_eq!(mission.player_life_regen_cooldown, 15);
        assert_eq!(mission.player_hit_frame_ticks, 4);
        assert_eq!(mission.player_hit_aux_ticks, 63);
        assert_eq!(mission.player_survived_feedback, PLAYER_SURVIVED_FEEDBACK);
        assert!(mission.events.iter().any(|event| matches!(
            event,
            Level1MissionEvent::MissionSound {
                sound_id,
                source_entity_index,
                position,
                ..
            } if *sound_id == reaction.pain_sound_id
                && *source_entity_index == player.entity_index
                && *position == player.position
        )));
        assert!(mission.take_player_hit_reaction().is_none());
    }

    #[test]
    fn lethal_player_hit_retains_source_mailbox_and_records_killer_and_death_sound() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(499, 1_000);
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 20_000,
                y: 20_000,
                z: 1_000,
            },
            ..FlightState::default()
        };
        mission.set_external_collision_targets([Level1CollisionTarget {
            entity_index: 10,
            position: Position {
                x: 19_000,
                ..player.position
            },
            collision_half_width: 85,
            collision_vertical_offset: 100,
            collision_half_height: 51,
            active: true,
        }]);
        mission.submit_player_damage(FIREFLY_FIREBALL_PAYLOAD, 10);

        mission.begin_source_turn();
        mission.dispatch_source_slot(&FlatWorld, &player, player.entity_index, false, false, None);

        let reaction = mission.take_player_hit_reaction().unwrap();
        assert!(reaction.lethal);
        assert_eq!(reaction.killer_entity_index, 10);
        assert_eq!(mission.player_killer_entity_index(), 10);
        assert_eq!(mission.player_pending_damage, FIREFLY_FIREBALL_PAYLOAD);
        assert_eq!(mission.player_pending_attacker, 10);
        assert!(mission.events.iter().any(|event| matches!(
            event,
            Level1MissionEvent::MissionSound {
                sound_id: PLAYER_DEATH_SOUND,
                source_entity_index,
                position,
                ..
            } if *source_entity_index == player.entity_index && *position == player.position
        )));
    }

    #[test]
    fn castle_cast_uses_exact_cost_holder_and_projectile_constants() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 999);
        mission.has_castle_spell = true;
        let holder = mission
            .actors
            .iter_mut()
            .find(|actor| actor.kind == Level1ActorKind::CastleSpellJar)
            .unwrap();
        holder.active = true;
        holder.collected = true;
        holder.action = 6;
        holder.state_timer = 0;
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 20_000,
                y: 20_000,
                z: 2_000,
            },
            yaw: 0,
            ..FlightState::default()
        };
        assert!(!mission.cast_initial_castle(&FlatWorld, &player, 0));
        assert_eq!(mission.player_mana, 999);
        assert!(mission.projectiles().is_empty());

        mission.player_mana = 1_000;
        assert!(mission.cast_initial_castle(&FlatWorld, &player, 0));
        assert_eq!(mission.player_mana, 0);
        assert_eq!(
            mission.castle_holder_active_value(),
            CASTLE_HOLDER_ACTIVATION_TICKS
        );
        let Level1Projectile::CastleBuild(projectile) = mission.projectiles()[0] else {
            panic!("Castle cast must create a build projectile");
        };
        assert_eq!(projectile.owner_entity_index, player.entity_index);
        assert_eq!(projectile.speed, CASTLE_PROJECTILE_SPEED);
        assert_eq!(projectile.life_ticks, CASTLE_PROJECTILE_LIFETIME_TICKS);
        assert_eq!(
            legacy_sqrt(squared_wrapping_xy_distance(
                player.position,
                projectile.target
            )),
            u32::from(CASTLE_PROJECTILE_TARGET_DISTANCE.cast_unsigned())
        );
        assert!(mission.events().iter().any(|event| matches!(
            event,
            Level1MissionEvent::CastleProjectileSpawned {
                sound_id: CASTLE_PROJECTILE_SOUND,
                ..
            }
        )));
    }

    #[test]
    fn accepted_castle_activation_reaches_the_handler_with_its_reset_timer() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.has_castle_spell = true;
        let holder = mission
            .actors
            .iter_mut()
            .find(|actor| actor.kind == Level1ActorKind::CastleSpellJar)
            .unwrap();
        holder.active = true;
        holder.collected = true;
        holder.action = 6;
        holder.state_value = i32::from(CASTLE_HOLDER_ACTIVATION_TICKS);
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 20_000,
                y: 20_000,
                z: 2_000,
            },
            ..FlightState::default()
        };

        mission.step_with_spells(&FlatWorld, &player, false, false, Some(0));

        assert!(matches!(
            mission.projectiles().first(),
            Some(Level1Projectile::CastleBuild(_))
        ));
        assert_eq!(mission.player_mana, 0);
    }

    #[test]
    fn carrier_motion_advances_across_the_wrapped_half_world_boundary() {
        let mut position = Position {
            x: 42_988,
            y: 42_039,
            z: 799,
        };
        let target = Position {
            x: 10_187,
            y: 9_269,
            z: 874,
        };
        let mut yaw = 0;
        let before_distance = distance_3d_wrapped(position, target);

        for _ in 0..250 {
            move_toward_position(&mut position, &mut yaw, target, MANA_CARRIER_SPEED);
        }

        assert!(
            distance_3d_wrapped(position, target) < before_distance,
            "carrier must close wrapped distance, ended at {position:?} yaw {yaw}"
        );
    }

    #[test]
    fn carrier_collects_only_after_strict_3d_overlap_and_waits_for_castle_scheduler() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.install_stable_stage_one_fixture(
            &FlatWorld,
            Position {
                x: 50_048,
                y: 51_584,
                z: 0,
            },
            152,
        );
        mission
            .spawn_mana_sphere_from_death(
                &FlatWorld,
                50,
                Position {
                    x: 53_418,
                    y: 53_040,
                    z: 72,
                },
                0,
                50,
                500,
                152,
            )
            .unwrap();
        let sphere = mission.mana_spheres.last_mut().unwrap();
        sphere.velocity_x = 0;
        sphere.velocity_y = 0;
        sphere.vertical_velocity = 0;
        let carrier = mission.mana_carrier.as_mut().unwrap();
        carrier.position = Position {
            x: 53_399,
            y: 53_033,
            z: 0,
        };
        carrier.target_sphere_id = sphere.id;
        sphere.assigned_carrier_id = carrier.id;

        mission.step_mana_carrier(&FlatWorld);

        let carrier = mission.mana_carrier().unwrap();
        assert!(
            !carrier.returning_to_castle,
            "collection clears the target but does not itself select the Castle"
        );
        assert_eq!(carrier.carried_mana, 500);
        assert!(mission.mana_spheres()[0].pending_delete);

        mission.source_slots[usize::from(mission.castle_source_entity_index)].phase = 2;
        mission.step_castle_root_slot(&FlatWorld, mission.castle_source_entity_index);
        let carrier = mission.mana_carrier().unwrap();
        assert!(carrier.returning_to_castle);
        assert_eq!(
            carrier.target_sphere_id,
            u32::from(mission.castle_source_entity_index)
        );
    }

    #[test]
    fn carrier_scheduler_uses_wrapped_3d_distance_source_slot_ties_and_exact_return_gates() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.install_stable_stage_one_fixture(
            &FlatWorld,
            Position {
                x: 20_000,
                y: 20_000,
                z: 0,
            },
            152,
        );
        let carrier_position = mission.mana_carrier().unwrap().position;
        let high_slot = mission
            .spawn_mana_sphere_from_death(
                &FlatWorld,
                50,
                Position {
                    x: carrier_position.x.wrapping_add(10),
                    y: carrier_position.y,
                    z: 30_000,
                },
                0,
                50,
                100,
                152,
            )
            .unwrap();
        let farther_xy_slot = mission
            .spawn_mana_sphere_from_death(
                &FlatWorld,
                51,
                Position {
                    x: carrier_position.x.wrapping_add(1_000),
                    y: carrier_position.y,
                    z: MANA_CARRIER_GROUND_CLEARANCE,
                },
                0,
                51,
                100,
                152,
            )
            .unwrap();
        mission.schedule_stage_one_mana_carrier();
        assert_eq!(
            mission.mana_carrier().unwrap().target_sphere_id,
            farther_xy_slot,
            "sub_5F810 ranks by wrapped XYZ squared distance, not XY"
        );

        for sphere in &mut mission.mana_spheres {
            sphere.position = Position {
                x: carrier_position.x.wrapping_add(2_000),
                y: carrier_position.y,
                z: MANA_CARRIER_GROUND_CLEARANCE,
            };
        }
        mission.schedule_stage_one_mana_carrier();
        assert_eq!(
            mission.mana_carrier().unwrap().target_sphere_id,
            high_slot.min(farther_xy_slot),
            "strict-less source traversal keeps the lowest source slot on a tie"
        );

        mission.mana_carrier.as_mut().unwrap().carried_mana =
            MANA_CARRIER_MAX_MANA.saturating_sub(1);
        mission.schedule_stage_one_mana_carrier();
        assert!(!mission.mana_carrier().unwrap().returning_to_castle);
        mission.mana_carrier.as_mut().unwrap().carried_mana = MANA_CARRIER_MAX_MANA;
        mission.schedule_stage_one_mana_carrier();
        assert!(mission.mana_carrier().unwrap().returning_to_castle);
        mission.mana_carrier.as_mut().unwrap().carried_mana = 0;
        mission.secured_mana_component = mission.castle_mana_capacity;
        mission.schedule_stage_one_mana_carrier();
        assert!(mission.mana_carrier().unwrap().returning_to_castle);
    }

    #[test]
    fn carrier_without_a_valid_target_does_not_move_or_relax_to_terrain() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.install_stable_stage_one_fixture(
            &FlatWorld,
            Position {
                x: 20_000,
                y: 20_000,
                z: 0,
            },
            152,
        );
        let carrier = mission.mana_carrier.as_mut().unwrap();
        carrier.position.z = 0;
        carrier.target_sphere_id = 0;
        carrier.returning_to_castle = false;
        let before = carrier.position;
        mission.step_mana_carrier(&FlatWorld);
        assert_eq!(mission.mana_carrier().unwrap().position, before);
    }

    #[test]
    fn castle_even_phase_absorbs_only_the_first_overlapping_owned_sphere() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.install_stable_stage_one_fixture(
            &FlatWorld,
            Position {
                x: 20_000,
                y: 20_000,
                z: 0,
            },
            152,
        );
        let castle = mission.castle_position;
        let neutral = mission
            .spawn_mana_sphere_from_death(&FlatWorld, 50, castle, 0, 50, 100, 0)
            .unwrap();
        let first_owned = mission
            .spawn_mana_sphere_from_death(&FlatWorld, 51, castle, 0, 51, 200, 152)
            .unwrap();
        let second_owned = mission
            .spawn_mana_sphere_from_death(&FlatWorld, 52, castle, 0, 52, 300, 152)
            .unwrap();
        mission.source_slots[usize::from(mission.castle_source_entity_index)].phase = 2;
        mission.step_castle_root_slot(&FlatWorld, mission.castle_source_entity_index);
        assert_eq!(mission.castle_mana, 200);
        assert!(
            mission
                .mana_spheres
                .iter()
                .find(|sphere| sphere.id == first_owned)
                .unwrap()
                .pending_delete
        );
        for retained in [neutral, second_owned] {
            assert!(
                !mission
                    .mana_spheres
                    .iter()
                    .find(|sphere| sphere.id == retained)
                    .unwrap()
                    .pending_delete
            );
        }
    }

    #[test]
    fn castle_overflow_splits_floor_shares_and_keeps_the_integer_remainder() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.install_stable_stage_one_fixture(
            &FlatWorld,
            Position {
                x: 20_000,
                y: 20_000,
                z: 0,
            },
            152,
        );
        mission.castle_mana = mission.castle_mana_capacity + 2_401;
        mission.source_slots[usize::from(mission.castle_source_entity_index)].phase = 2;
        mission.step_castle_root_slot(&FlatWorld, mission.castle_source_entity_index);
        assert_eq!(mission.castle_mana, mission.castle_mana_capacity + 1);
        let overflow = mission
            .mana_spheres
            .iter()
            .filter(|sphere| sphere.active)
            .collect::<Vec<_>>();
        assert_eq!(overflow.len(), 2);
        assert!(overflow.iter().all(|sphere| {
            sphere.mana == 1_200
                && sphere.owner_entity_index == 152
                && sphere.velocity_x == 0
                && sphere.velocity_y == 0
                && sphere.phase == sphere.source_entity_index as u8
        }));
    }

    #[test]
    fn dead_carrier_converts_cargo_once_then_reclaims_and_recreates_its_source_slot() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.install_stable_stage_one_fixture(
            &FlatWorld,
            Position {
                x: 20_000,
                y: 20_000,
                z: 0,
            },
            152,
        );
        let carrier_slot = mission.mana_carrier().unwrap().source_entity_index;
        let carrier = mission.mana_carrier.as_mut().unwrap();
        carrier.carried_mana = 700;
        carrier.life = -1;
        mission.source_slots[usize::from(mission.castle_source_entity_index)].phase = 2;
        mission.step_castle_root_slot(&FlatWorld, mission.castle_source_entity_index);
        assert!(mission.mana_carrier().is_none());
        assert_eq!(
            mission
                .mana_spheres
                .iter()
                .filter(|sphere| sphere.active && sphere.mana == 700)
                .count(),
            1
        );

        mission.begin_source_turn();
        assert!(mission.mana_carrier.is_none());
        mission.source_slots[usize::from(mission.castle_source_entity_index)].phase = 2;
        mission.step_castle_root_slot(&FlatWorld, mission.castle_source_entity_index);
        assert_eq!(
            mission.mana_carrier().unwrap().source_entity_index,
            carrier_slot,
            "ascending reclaim pushes the released carrier slot back for the next allocation"
        );
    }

    #[test]
    fn carrier_damage_excludes_its_owner_and_consumes_hostile_mail_after_motion() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.install_stable_stage_one_fixture(
            &FlatWorld,
            Position {
                x: 20_000,
                y: 20_000,
                z: 0,
            },
            152,
        );
        let carrier_position = mission.mana_carrier().unwrap().position;
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 40_000,
                y: 40_000,
                z: 4_000,
            },
            ..FlightState::default()
        };
        for (id, owner) in [(900_u32, 152_u16), (901, 77)] {
            let source_slot = mission
                .source_pool
                .allocate(SourceOccupant::FireballImpact(id))
                .unwrap();
            bind_dynamic_source_record(
                &mut mission.source_slots,
                source_slot,
                SourceOccupant::FireballImpact(id),
                mission.global_rng,
            );
            mission.impacts.push(FireballImpact {
                id,
                source_entity_index: source_slot,
                source_projectile_id: id,
                owner_entity_index: owner,
                external_target_entity_index: 0,
                model: FIREBALL_IMPACT_MODEL,
                position: carrier_position,
                payload: 250,
                life_ticks: 2,
                entity_rng: mission.source_slots[usize::from(source_slot)].entity_rng,
                vertical_velocity: 0,
                animation_frame: 0,
                damage_submitted: false,
                active: true,
            });
            mission.step_impact_slot(&FlatWorld, &player, source_slot);
        }
        let carrier = mission.mana_carrier().unwrap();
        assert_eq!(carrier.pending_damage, 250);
        assert_eq!(carrier.pending_attacker_entity_index, 77);
        mission.step_mana_carrier(&FlatWorld);
        let carrier = mission.mana_carrier().unwrap();
        assert_eq!(carrier.life, MANA_CARRIER_MAX_LIFE - 250);
        assert_eq!(carrier.pending_attacker_entity_index, 0);
        assert_eq!(
            carrier.pending_damage, 250,
            "sub_60EA0 clears only the surviving hit's attacker relation"
        );
    }

    #[test]
    fn speed_release_is_model17_timed_and_independent_of_archer_survival() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 2_000);
        mission.activate_group(3);
        assert_eq!(mission.count_alive(Level1ActorKind::Archer), 4);
        assert!(
            !mission
                .actors()
                .iter()
                .any(|actor| actor.kind == Level1ActorKind::SpeedSpellJar && actor.active)
        );
        let player = FlightState {
            position: Position {
                x: 1_000,
                y: 1_000,
                z: 128,
            },
            ..FlightState::default()
        };
        for _ in 0..17 {
            mission.step_switches(&player);
        }
        assert_eq!(mission.count_alive(Level1ActorKind::Archer), 4);
        assert!(
            mission
                .actors()
                .iter()
                .any(|actor| actor.kind == Level1ActorKind::SpeedSpellJar && actor.active)
        );
    }

    #[test]
    fn exit_switch_uses_strict_wrapped_xy_bounds_and_ignores_altitude() {
        fn triggered_at(offset_x: i16, offset_y: i16, z: i16) -> bool {
            let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
            let mut mission = template.session(10_000, 2_000);
            mission.level_completed = true;
            mission.current_stage = 5;
            mission.current_text_selector = 53;
            let mut player = FlightState {
                entity_index: 152,
                position: Position {
                    x: 4_000,
                    y: 4_000,
                    z: 128,
                },
                ..FlightState::default()
            };
            mission.step_switches(&player);
            let switch = mission
                .actors()
                .iter()
                .find(|actor| actor.kind == Level1ActorKind::ExitSwitch)
                .copied()
                .unwrap();
            assert_eq!(switch.collision_half_width, 512);
            player.position = Position {
                x: switch.position.x.wrapping_add_signed(offset_x),
                y: switch.position.y.wrapping_add_signed(offset_y),
                z,
            };
            mission
                .authored_allocations
                .iter_mut()
                .find(|allocation| allocation.authored_index == LEVEL1_EXIT_SWITCH_FEATURE)
                .unwrap()
                .phase = 0;
            mission.step_switches(&player);
            mission.exit_sequence_active
        }

        assert!(triggered_at(632, -632, i16::MIN));
        assert!(!triggered_at(633, 0, 128));
        assert!(!triggered_at(0, -633, 128));
        assert!(inside_exit_switch_xy(
            Position {
                x: u16::MAX - 100,
                y: 0,
                z: 0,
            },
            Position {
                x: 100,
                y: 0,
                z: i16::MAX,
            },
            512,
        ));
    }

    #[test]
    fn exit_palette_requests_follow_the_source_frame_trace() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 2_000);
        mission.exit_palette_submode = EXIT_PALETTE_ENTRY_FLASH_REQUEST;

        let mut entry_trace = Vec::new();
        for _ in 0..6 {
            mission.advance_exit_palette_frame();
            entry_trace.push((
                mission.exit_palette_submode,
                mission.exit_palette_effect,
                mission.exit_palette_weight,
                mission.exit_palette_denominator,
            ));
        }
        assert_eq!(
            entry_trace,
            vec![
                (1, EXIT_PALETTE_EFFECT_ENTRY_FLASH, 4, 4),
                (1, EXIT_PALETTE_EFFECT_ENTRY_FLASH, 4, 4),
                (1, EXIT_PALETTE_EFFECT_ENTRY_FLASH, 3, 4),
                (1, EXIT_PALETTE_EFFECT_ENTRY_FLASH, 2, 4),
                (1, EXIT_PALETTE_EFFECT_ENTRY_FLASH, 1, 4),
                (0, EXIT_PALETTE_EFFECT_NONE, 0, 0),
            ]
        );

        mission.exit_palette_submode = EXIT_PALETTE_BLACK_REQUEST;
        mission.advance_exit_palette_frame();
        assert_eq!(
            (
                mission.exit_palette_submode,
                mission.exit_palette_effect,
                mission.exit_palette_weight,
                mission.exit_palette_denominator,
            ),
            (10, EXIT_PALETTE_EFFECT_NONE, 0, 28)
        );
        mission.advance_exit_palette_frame();
        assert_eq!(
            (
                mission.exit_palette_submode,
                mission.exit_palette_effect,
                mission.exit_palette_weight,
                mission.exit_palette_denominator,
            ),
            (10, EXIT_PALETTE_EFFECT_BLACK, 0, 28)
        );
        for expected in 1..=28 {
            mission.advance_exit_palette_frame();
            assert_eq!(mission.exit_palette_weight, expected);
        }
        assert_eq!(mission.exit_palette_submode, EXIT_PALETTE_NONE);
        assert_eq!(mission.exit_palette_effect, EXIT_PALETTE_EFFECT_BLACK);
    }

    #[test]
    fn level_one_hit_speed_and_objective_palettes_preserve_rgb6_integer_rules() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 2_000);
        let mut original = [0_u8; 768];
        original[0..6].copy_from_slice(&[63, 1, 2, 1, 2, 3]);

        mission.exit_palette_submode = LEVEL1_PALETTE_HIT_REQUEST;
        mission.advance_exit_palette_frame();
        assert_eq!(
            (
                mission.exit_palette_effect,
                mission.exit_palette_weight,
                mission.exit_palette_denominator,
            ),
            (LEVEL1_PALETTE_EFFECT_HIT, 4, 4)
        );
        assert_eq!(
            &mission.effective_palette_rgb6(&original)[0..6],
            &[0, 0, 0, 41, 2, 3]
        );

        mission.exit_palette_submode = LEVEL1_PALETTE_SPEED_REQUEST;
        mission.exit_palette_count = 142;
        mission.advance_exit_palette_frame();
        assert_eq!(
            &mission.effective_palette_rgb6(&original)[0..6],
            &[0, 0, 0, 0, 2, 0]
        );

        mission.exit_palette_submode = LEVEL1_PALETTE_OBJECTIVE_REQUEST;
        mission.advance_exit_palette_frame();
        assert_eq!(
            (
                mission.exit_palette_submode,
                mission.exit_palette_effect,
                mission.exit_palette_weight,
                mission.exit_palette_denominator,
            ),
            (
                LEVEL1_PALETTE_OBJECTIVE_FADE,
                LEVEL1_PALETTE_EFFECT_OBJECTIVE,
                16,
                16,
            )
        );
        assert_eq!(
            &mission.effective_palette_rgb6(&original)[0..6],
            &[0, 0, 0, 49, 50, 51]
        );
        mission.advance_exit_palette_frame();
        assert_eq!(mission.exit_palette_weight, 16);
        mission.advance_exit_palette_frame();
        assert_eq!(mission.exit_palette_weight, 15);
    }

    #[test]
    fn exit_action_phase_one_preserves_rejected_movement_deceleration_quirk() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 2_000);
        mission.exit_sequence_active = true;
        mission.exit_sequence_phase = 1;
        mission.exit_target_source_entity_index = 0;
        let mut player = FlightState {
            position: Position {
                x: 4_000,
                y: 4_000,
                z: 128,
            },
            actual_speed: 8,
            ..FlightState::default()
        };
        let mut context = FlightContext::default();
        let mut camera_fov = 128;
        mission.advance_exit_sequence(
            &UniformMaterialWorld {
                material_mask: 0x100,
            },
            &mut player,
            &mut context,
            &mut camera_fov,
        );
        assert_eq!(
            mission.exit_last_movement_resolution,
            EXIT_MOVEMENT_REJECTED
        );
        assert_eq!(player.actual_speed, -4);
        assert_eq!(mission.exit_sequence_phase, 1);

        mission.advance_exit_sequence(
            &UniformMaterialWorld {
                material_mask: 0x100,
            },
            &mut player,
            &mut context,
            &mut camera_fov,
        );
        assert_eq!(player.actual_speed, 0);
        assert_eq!(mission.exit_sequence_phase, 4);
    }

    #[test]
    fn exit_action_fov_target_timeout_and_axis_relaxation_use_exact_boundaries() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 2_000);
        mission.exit_sequence_active = true;
        mission.exit_sequence_phase = 4;
        mission.exit_target_source_entity_index = 42;
        let mut player = FlightState {
            position: Position {
                x: 4_000,
                y: 4_000,
                z: 128,
            },
            roll_accumulator: 15,
            pitch_accumulator: -9,
            ..FlightState::default()
        };
        let mut context = FlightContext::default();
        let mut camera_fov = 128;

        mission.advance_exit_sequence(&FlatWorld, &mut player, &mut context, &mut camera_fov);
        assert_eq!(mission.exit_sequence_phase, 5);
        assert_eq!(mission.exit_sequence_timer, 11);
        assert_eq!(camera_fov, 127);
        assert_eq!(player.roll_accumulator, 14);
        assert_eq!(player.pitch_accumulator, -7);
        while mission.exit_sequence_phase == 5 {
            mission.advance_exit_sequence(&FlatWorld, &mut player, &mut context, &mut camera_fov);
        }
        assert_eq!(camera_fov, 121);
        assert_eq!(mission.exit_sequence_phase, 6);

        mission.exit_sequence_phase = 7;
        mission.exit_sequence_timer = 1;
        player.actual_speed = 100;
        let before_xy = (player.position.x, player.position.y);
        mission.advance_exit_sequence(&FlatWorld, &mut player, &mut context, &mut camera_fov);
        assert_eq!(mission.exit_sequence_phase, 10);
        assert_eq!((player.position.x, player.position.y), before_xy);
        assert_eq!(
            mission.exit_last_movement_resolution,
            EXIT_MOVEMENT_NOT_ATTEMPTED
        );
    }

    #[test]
    fn exit_action_without_a_valid_target_uses_the_straight_flight_branch() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 2_000);
        mission.exit_sequence_active = true;
        mission.exit_sequence_phase = 0;
        mission.exit_target_source_entity_index = u16::MAX;
        mission.exit_target_valid = true;
        let mut player = FlightState {
            position: Position {
                x: 4_000,
                y: 4_000,
                z: 128,
            },
            ..FlightState::default()
        };
        let mut context = FlightContext::default();
        let mut camera_fov = 128;
        mission.advance_exit_sequence(&FlatWorld, &mut player, &mut context, &mut camera_fov);
        assert_eq!(mission.exit_target_source_entity_index, 0);
        assert!(!mission.exit_target_valid);
        assert_eq!(mission.exit_sequence_phase, 1);
        assert_eq!(mission.exit_player_status_flags & 0x20, 0x20);

        mission.exit_sequence_phase = 8;
        mission.advance_exit_sequence(&FlatWorld, &mut player, &mut context, &mut camera_fov);
        assert_eq!(mission.exit_sequence_phase, 9);
        assert_eq!(mission.exit_sequence_timer, 127);
        assert_eq!(mission.exit_blur_mode, 2);
        assert_eq!(player.actual_speed, 108);
        assert_eq!(
            mission.exit_last_movement_resolution,
            EXIT_MOVEMENT_ACCEPTED
        );
    }

    #[test]
    fn group4_exit_switch_runs_source_timers_and_writes_result_10() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 2_000);
        mission.level_completed = true;
        mission.current_stage = 5;
        mission.current_text_selector = 53;
        let mut player = FlightState {
            entity_index: 152,
            position: Position {
                x: 4_000,
                y: 4_000,
                z: 128,
            },
            ..FlightState::default()
        };
        mission.step_switches(&player);
        let target = mission
            .actors()
            .iter()
            .find(|actor| actor.kind == Level1ActorKind::ExitTarget)
            .copied()
            .unwrap();
        let exit_switch = mission
            .actors()
            .iter()
            .find(|actor| actor.kind == Level1ActorKind::ExitSwitch)
            .copied()
            .unwrap();
        assert!(target.active);
        assert!(!target.visible);
        assert!(exit_switch.active);

        player.position = exit_switch.position;
        let allocation = mission
            .authored_allocations
            .iter_mut()
            .find(|allocation| allocation.authored_index == LEVEL1_EXIT_SWITCH_FEATURE)
            .unwrap();
        allocation.phase = 0;
        mission.step_switches(&player);
        assert!(mission.exit_sequence_active);
        assert!(
            mission
                .actors()
                .iter()
                .find(|actor| actor.kind == Level1ActorKind::ExitTarget)
                .unwrap()
                .visible
        );

        let mut context = FlightContext::default();
        let mut camera_fov = 128;
        mission.advance_exit_sequence(&FlatWorld, &mut player, &mut context, &mut camera_fov);
        assert_eq!(mission.exit_sequence_phase, 1);
        mission.advance_exit_sequence(&FlatWorld, &mut player, &mut context, &mut camera_fov);
        assert_eq!(mission.exit_sequence_phase, 3);
        while mission.exit_sequence_phase == 3 {
            mission.advance_exit_sequence(&FlatWorld, &mut player, &mut context, &mut camera_fov);
        }
        assert_eq!(mission.exit_sequence_phase, 4);
        mission.advance_exit_sequence(&FlatWorld, &mut player, &mut context, &mut camera_fov);
        assert_eq!(mission.exit_sequence_phase, 5);
        assert_eq!(mission.exit_sequence_timer, 11);
        while mission.exit_sequence_phase == 5 {
            mission.advance_exit_sequence(&FlatWorld, &mut player, &mut context, &mut camera_fov);
        }
        assert_eq!(mission.exit_sequence_phase, 6);
        assert_eq!(camera_fov, 121);
        mission.advance_exit_sequence(&FlatWorld, &mut player, &mut context, &mut camera_fov);
        assert_eq!(mission.exit_sequence_phase, 7);
        assert_eq!(mission.exit_sequence_timer, 511);
        for _ in 0..700 {
            mission.advance_exit_palette_frame();
            mission.advance_exit_sequence(&FlatWorld, &mut player, &mut context, &mut camera_fov);
            if mission.exit_sequence_complete {
                break;
            }
        }
        assert!(mission.level_completed, "tasks remain complete during exit");
        assert!(mission.exit_sequence_complete);
        assert!(!mission.exit_sequence_active);
        assert_eq!(mission.exit_result_code, 10);
        assert!(mission.events().iter().any(|event| matches!(
            event,
            Level1MissionEvent::ExitSequenceCompleted { result_code: 10 }
        )));
    }

    #[test]
    fn complete_level1_route_reaches_every_objective_and_exit_result_10() {
        fn record_selector(mission: &Level1MissionState, selectors: &mut Vec<u8>) {
            if selectors.last().copied() != Some(mission.current_text_selector) {
                selectors.push(mission.current_text_selector);
            }
        }

        fn remove_live_family(
            mission: &mut Level1MissionState,
            player: &FlightState,
            kind: Level1ActorKind,
        ) {
            for actor in mission
                .actors
                .iter_mut()
                .filter(|actor| actor.kind == kind && actor.active)
            {
                actor.pending_damage = actor.max_life.saturating_add(1);
                actor.pending_attacker = player.entity_index;
            }
            for _ in 0..24 {
                mission.step(&FlatWorld, player);
                if mission.count_alive(kind) == 0 {
                    return;
                }
            }
            panic!("Level 1 family {kind:?} did not complete its source death path");
        }

        let mut template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        template.castle_stage_one_footprint = Some(CastleBuildFootprint::synthetic_stage_one());
        let mut mission = template.session(10_000, 50_000);
        let mut selectors = vec![mission.current_text_selector];
        let mut player = FlightState {
            entity_index: 152,
            position: LEVEL1_FIRST_WAYPOINT,
            ..FlightState::default()
        };

        mission.step(&FlatWorld, &player);
        record_selector(&mission, &mut selectors);
        assert_eq!(mission.current_stage, 1);

        player.position = Position {
            x: 36_000,
            y: LEVEL1_FIRST_WAYPOINT.y,
            z: LEVEL1_INITIAL_ZONE_SWITCH_CENTER.z,
        };
        for _ in 0..16 {
            mission.step(&FlatWorld, &player);
            if mission.allocation_slot(105).is_some_and(|slot| slot != 0) {
                break;
            }
        }
        player.position = Position {
            z: LEVEL1_INITIAL_ZONE_SWITCH_CENTER.z,
            ..LEVEL1_SECOND_WAYPOINT
        };
        for _ in 0..16 {
            mission.step(&FlatWorld, &player);
            if mission.current_stage == 2 {
                break;
            }
        }
        record_selector(&mission, &mut selectors);
        assert_eq!(mission.current_stage, 2);
        for _ in 0..4 {
            mission.step(&FlatWorld, &player);
            if mission.count_alive(Level1ActorKind::Archer) == 4 {
                break;
            }
        }
        assert_eq!(mission.count_alive(Level1ActorKind::Archer), 4);

        let castle_jar_position = mission
            .actors
            .iter()
            .find(|actor| actor.kind == Level1ActorKind::CastleSpellJar)
            .expect("Level 1 Castle jar")
            .position;
        player.position = castle_jar_position;
        for _ in 0..4 {
            mission.step(&FlatWorld, &player);
            if mission.has_castle_spell {
                break;
            }
        }
        assert!(mission.has_castle_spell);

        remove_live_family(&mut mission, &player, Level1ActorKind::Archer);
        for _ in 0..4 {
            mission.step(&FlatWorld, &player);
            if mission.current_stage == 3 {
                break;
            }
        }
        record_selector(&mission, &mut selectors);
        assert_eq!(mission.current_stage, 3);
        assert_eq!(
            mission
                .mana_spheres
                .iter()
                .filter(|sphere| sphere.active)
                .map(|sphere| sphere.mana)
                .sum::<i32>(),
            2_000
        );

        for _ in 0..80 {
            let holder = mission
                .actors
                .iter()
                .find(|actor| actor.kind == Level1ActorKind::CastleSpellJar)
                .expect("collected Castle holder");
            if holder.state_timer == 0 {
                break;
            }
            mission.step(&FlatWorld, &player);
        }
        player.position = Position {
            x: 50_123,
            y: 50_321,
            z: 2_000,
        };
        mission.step_with_spells(&FlatWorld, &player, false, false, Some(0));
        for _ in 0..40 {
            if mission.castle_exists {
                break;
            }
            mission.step(&FlatWorld, &player);
        }
        assert!(mission.castle_exists);
        assert_eq!(mission.castle_level, CASTLE_STAGE_ONE_LEVEL);
        for _ in 0..24 {
            if mission.castle_action == 4 && mission.mana_carrier().is_some() {
                break;
            }
            mission.step(&FlatWorld, &player);
        }
        assert_eq!(mission.castle_action, 4);
        assert!(mission.mana_carrier().is_some());

        for source in 60_u16..64 {
            mission
                .spawn_mana_sphere_from_death(
                    &FlatWorld,
                    source,
                    Position {
                        x: mission.castle_position.x.wrapping_add(512 * (source - 59)),
                        y: mission.castle_position.y,
                        z: mission.castle_position.z,
                    },
                    0,
                    source,
                    300,
                    0,
                )
                .expect("synthetic Goat death routes through the real sphere constructor");
        }
        for sphere in &mut mission.mana_spheres {
            sphere.velocity_x = 0;
            sphere.velocity_y = 0;
            sphere.vertical_velocity = 0;
        }
        let claimable_spheres = mission
            .mana_spheres
            .iter()
            .filter(|sphere| sphere.active && sphere.owner_entity_index != player.entity_index)
            .map(|sphere| sphere.id)
            .collect::<Vec<_>>();
        // The authored enemy Archers retain their faction owner; the four
        // synthetic Goat deaths are neutral. Possession claims both through
        // the same source ownership transition.
        assert_eq!(claimable_spheres.len(), 8);
        for sphere_id in claimable_spheres {
            let sphere_position = mission
                .mana_spheres
                .iter()
                .find(|sphere| sphere.id == sphere_id)
                .expect("neutral sphere")
                .position;
            player.position = Position {
                x: sphere_position.x,
                y: sphere_position.y.wrapping_add(384),
                z: sphere_position.z,
            };
            assert!(mission.spawn_possession(&player));
            for _ in 0..8 {
                mission.step_projectiles(&FlatWorld, &player);
                mission.step_mana_spheres(&FlatWorld, player.entity_index);
                if mission
                    .mana_spheres
                    .iter()
                    .find(|sphere| sphere.id == sphere_id)
                    .is_some_and(|sphere| sphere.owner_entity_index == player.entity_index)
                {
                    break;
                }
            }
            assert_eq!(
                mission
                    .mana_spheres
                    .iter()
                    .find(|sphere| sphere.id == sphere_id)
                    .expect("claimed sphere")
                    .owner_entity_index,
                player.entity_index
            );
        }

        for _ in 0..10_000 {
            mission.step(&FlatWorld, &player);
            if mission.current_stage >= 4 && mission.count_alive(Level1ActorKind::Firefly) == 5 {
                break;
            }
        }
        record_selector(&mission, &mut selectors);
        assert_eq!(
            mission.current_stage,
            4,
            "castle_mana={} percent={} spheres={:?} carrier={:?}",
            mission.castle_mana,
            mission.secured_mana_percent(),
            mission.mana_spheres,
            mission.mana_carrier
        );
        assert!(mission.secured_mana_percent() >= i32::from(LEVEL1_CASTLE_MANA_PERCENT));
        assert_eq!(mission.count_alive(Level1ActorKind::Firefly), 5);

        for _ in 0..24 {
            if mission
                .actors
                .iter()
                .any(|actor| actor.kind == Level1ActorKind::SpeedSpellJar && actor.active)
            {
                break;
            }
            mission.step(&FlatWorld, &player);
        }
        let speed_jar_position = mission
            .actors
            .iter()
            .find(|actor| actor.kind == Level1ActorKind::SpeedSpellJar)
            .expect("Level 1 Speed jar")
            .position;
        player.position = speed_jar_position;
        for _ in 0..4 {
            mission.step(&FlatWorld, &player);
            if mission.has_speed_spell {
                break;
            }
        }
        assert!(mission.has_speed_spell);

        remove_live_family(&mut mission, &player, Level1ActorKind::Firefly);
        for _ in 0..4 {
            mission.step(&FlatWorld, &player);
            if mission.level_completed {
                break;
            }
        }
        record_selector(&mission, &mut selectors);
        assert_eq!(selectors, [48, 49, 50, 51, 52, 53]);
        assert!(mission.level_completed);
        assert!(!mission.exit_sequence_complete);
        assert_eq!(mission.exit_result_code, 0);

        mission.step(&FlatWorld, &player);
        let exit_switch_position = mission
            .actors
            .iter()
            .find(|actor| actor.kind == Level1ActorKind::ExitSwitch && actor.active)
            .expect("group 4 must activate the authored exit switch")
            .position;
        player.position = exit_switch_position;
        for _ in 0..8 {
            mission.step(&FlatWorld, &player);
            if mission.exit_sequence_active {
                break;
            }
        }
        assert!(mission.exit_sequence_active);
        assert!(!mission.exit_sequence_complete);

        let mut context = FlightContext::default();
        let mut camera_fov = 128;
        for _ in 0..800 {
            mission.advance_exit_palette_frame();
            mission.advance_exit_sequence(&FlatWorld, &mut player, &mut context, &mut camera_fov);
            if mission.exit_sequence_complete {
                break;
            }
        }
        assert!(mission.exit_sequence_complete);
        assert_eq!(mission.exit_result_code, 10);
        assert_eq!(
            mission
                .events()
                .iter()
                .filter(|event| matches!(
                    event,
                    Level1MissionEvent::ExitSequenceCompleted { result_code: 10 }
                ))
                .count(),
            1
        );
    }

    #[test]
    fn invalid_castle_footprint_rejects_without_creating_a_structure() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.has_castle_spell = true;
        let holder = mission
            .actors
            .iter_mut()
            .find(|actor| actor.kind == Level1ActorKind::CastleSpellJar)
            .unwrap();
        holder.active = true;
        holder.collected = true;
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 20_000,
                y: 20_000,
                z: 2_000,
            },
            ..FlightState::default()
        };
        assert!(mission.cast_initial_castle(&InvalidCastleTileWorld, &player, 0));
        mission.step_projectiles(&InvalidCastleTileWorld, &player);
        assert!(!mission.castle_exists);
        assert_eq!(mission.castle_holder_active_value(), 0);
        assert!(
            mission
                .events()
                .iter()
                .any(|event| matches!(event, Level1MissionEvent::CastlePlacementRejected { .. }))
        );
    }

    #[test]
    fn castle_placement_preserves_blocker_pass_order_wrap_and_inclusive_boundary() {
        let candidate = Position {
            x: 0,
            y: 10_000,
            z: 500,
        };
        let castle = Level1CastlePlacementBlocker {
            source_entity_index: 12,
            position: Position {
                x: 65_535,
                y: 10_000,
                z: -2_000,
            },
            half_x: 0,
            half_y: 0,
        };
        let building = Level1CastlePlacementBlocker {
            source_entity_index: 34,
            position: candidate,
            half_x: 0,
            half_y: 0,
        };
        assert_eq!(
            castle_placement_rejection(&InvalidCastleTileWorld, candidate, &[castle], &[building]),
            Some(CastlePlacementRejection::CastleRoot(12))
        );
        assert_eq!(
            castle_placement_rejection(&InvalidCastleTileWorld, candidate, &[], &[building]),
            Some(CastlePlacementRejection::Building(34))
        );

        let inclusive = Level1CastlePlacementBlocker {
            position: Position {
                x: candidate.x.wrapping_add(2_560),
                ..candidate
            },
            ..building
        };
        assert!(castle_blocker_overlaps(candidate, inclusive));
        let outside = Level1CastlePlacementBlocker {
            position: Position {
                x: candidate.x.wrapping_add(2_561),
                ..candidate
            },
            ..building
        };
        assert!(!castle_blocker_overlaps(candidate, outside));
        let half_world = Level1CastlePlacementBlocker {
            position: Position {
                x: 32_768,
                ..candidate
            },
            half_x: i16::MAX,
            ..building
        };
        assert!(castle_blocker_overlaps(candidate, half_world));
    }

    #[test]
    fn castle_terrain_scan_is_asymmetric_wrapped_row_major_and_angle_only() {
        let candidate = Position {
            x: 4 << 8,
            y: 3 << 8,
            z: 0,
        };
        let first = Tile { x: 253, y: 251 };
        let later = Tile { x: 252, y: 252 };
        let world = CastleAngleWorld {
            first,
            second: later,
        };
        assert_eq!(
            castle_placement_rejection(&world, candidate, &[], &[]),
            Some(CastlePlacementRejection::Terrain(first))
        );
        // Material and height do not participate in sub_11CB0 outdoors; only
        // the signed map-angle byte does.
        assert!(castle_placement_valid(
            &UniformMaterialWorld {
                material_mask: u32::MAX,
            },
            candidate,
            &[],
            &[]
        ));
    }

    #[test]
    fn valid_castle_projectile_builds_exact_stage_one_stats_and_snap() {
        let mut template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        template.castle_stage_one_footprint = Some(CastleBuildFootprint::synthetic_stage_one());
        let mut mission = template.session(10_000, 1_000);
        mission.has_castle_spell = true;
        let holder = mission
            .actors
            .iter_mut()
            .find(|actor| actor.kind == Level1ActorKind::CastleSpellJar)
            .unwrap();
        holder.active = true;
        holder.collected = true;
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 20_123,
                y: 20_321,
                z: 2_000,
            },
            ..FlightState::default()
        };
        assert!(mission.cast_initial_castle(&FlatWorld, &player, 0));
        for _ in 0..32 {
            mission.step_projectiles(&FlatWorld, &player);
            if mission.castle_exists {
                break;
            }
        }
        assert!(mission.castle_exists);
        assert_eq!(mission.castle_level, 0);
        assert_eq!(mission.castle_life, CASTLE_ROOT_CONSTRUCTOR_LIFE);
        assert_eq!(mission.castle_mana_capacity, 0);
        for _ in 0..24 {
            if mission.castle_action == 4 && mission.mana_carrier().is_some() {
                break;
            }
            mission.step(&FlatWorld, &player);
        }
        assert_eq!(mission.castle_level, CASTLE_STAGE_ONE_LEVEL);
        assert_eq!(mission.castle_action, 4);
        assert_eq!(mission.castle_build_state, 0);
        assert_eq!(mission.castle_life, CASTLE_STAGE_ONE_LIFE);
        assert_eq!(mission.castle_max_life, CASTLE_STAGE_ONE_LIFE);
        assert_eq!(mission.castle_mana_capacity, CASTLE_STAGE_ONE_MANA_CAPACITY);
        assert_eq!(mission.castle_position.x & 0x00ff, 0);
        assert_eq!(mission.castle_position.y & 0x00ff, 0);
        assert_eq!(
            ((mission.castle_position.x >> 8) + (mission.castle_position.y >> 8)) & 1,
            0
        );
        assert_ne!(mission.castle_source_entity_index, 0);
        assert_eq!(
            mission
                .source_pool
                .occupant(mission.castle_source_entity_index),
            Some(SourceOccupant::CastleRoot(mission.castle_presentation_id))
        );
        let castle_record = mission.source_slots[usize::from(mission.castle_source_entity_index)];
        assert_eq!(
            (
                castle_record.class,
                castle_record.model,
                castle_record.action
            ),
            (CASTLE_ROOT_CLASS, CASTLE_ROOT_MODEL, CASTLE_ROOT_ACTION)
        );
        assert_ne!(
            mission
                .mana_carrier()
                .expect("stage-one carrier")
                .source_entity_index,
            mission.castle_source_entity_index
        );
    }

    #[test]
    fn stage_one_castle_shelter_uses_strict_source_aabb_and_preflight_position() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        let owner = FlightState {
            entity_index: 152,
            position: Position {
                x: 20_000,
                y: 20_000,
                z: 0,
            },
            ..FlightState::default()
        };
        mission.install_stable_stage_one_fixture(&FlatWorld, owner.position, owner.entity_index);
        let castle = mission.castle_position;
        let inside = Position {
            x: castle.x.wrapping_add(1_784),
            y: castle.y,
            z: castle.z,
        };
        let boundary = Position {
            x: castle.x.wrapping_add(1_785),
            ..inside
        };
        assert!(mission.player_inside_stage_one_castle(inside));
        assert!(!mission.player_inside_stage_one_castle(boundary));

        mission.set_external_collision_targets([Level1CollisionTarget {
            entity_index: 10,
            position: Position {
                x: castle.x.wrapping_sub(1_000),
                y: castle.y,
                z: castle.z,
            },
            collision_half_width: 85,
            collision_vertical_offset: 100,
            collision_half_height: 51,
            active: true,
        }]);
        mission.submit_player_damage(FIREFLY_FIREBALL_PAYLOAD, 10);
        let postflight_player = FlightState {
            position: Position {
                x: castle.x.wrapping_add(5_000),
                ..inside
            },
            ..owner
        };

        mission.begin_source_turn_at(inside);
        mission.dispatch_source_slot(
            &FlatWorld,
            &postflight_player,
            owner.entity_index,
            false,
            false,
            None,
        );

        assert_eq!(mission.player_life, 10_000);
        assert_eq!(mission.player_pending_damage, 0);
        assert_eq!(mission.player_pending_attacker, 0);
        assert_eq!(mission.player_damage_suppression, 1);
        assert_eq!(mission.castle_pending_damage, FIREFLY_FIREBALL_PAYLOAD);
        assert_eq!(mission.castle_pending_attacker, 10);

        mission.dispatch_source_slot(
            &FlatWorld,
            &postflight_player,
            mission.castle_source_entity_index,
            false,
            false,
            None,
        );
        assert_eq!(
            mission.castle_life,
            CASTLE_STAGE_ONE_LIFE - FIREFLY_FIREBALL_PAYLOAD
        );
        assert_eq!(mission.castle_pending_damage, 0);
        assert_eq!(mission.castle_pending_attacker, 0);
        assert_eq!(mission.castle_owner_hit_feedback, 4);
    }

    #[test]
    fn fireball_impact_writes_and_castle_slot_consumes_its_own_mailbox() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 30_000,
                y: 30_000,
                z: 0,
            },
            ..FlightState::default()
        };
        mission.install_stable_stage_one_fixture(
            &FlatWorld,
            Position {
                x: 20_000,
                y: 20_000,
                z: 0,
            },
            player.entity_index,
        );
        let impact_id = 80;
        let impact_slot = mission
            .source_pool
            .allocate(SourceOccupant::FireballImpact(impact_id))
            .unwrap();
        mission.impacts.push(FireballImpact {
            id: impact_id,
            source_entity_index: impact_slot,
            source_projectile_id: 79,
            owner_entity_index: 10,
            external_target_entity_index: 0,
            model: FIREBALL_IMPACT_MODEL,
            position: mission.castle_position,
            payload: FIREFLY_FIREBALL_PAYLOAD,
            life_ticks: 8,
            entity_rng: 123,
            vertical_velocity: 0,
            animation_frame: 0,
            damage_submitted: false,
            active: true,
        });

        mission.begin_source_turn_at(player.position);
        mission.dispatch_source_slot(&FlatWorld, &player, impact_slot, false, false, None);
        assert_eq!(mission.castle_pending_damage, FIREFLY_FIREBALL_PAYLOAD);
        mission.dispatch_source_slot(
            &FlatWorld,
            &player,
            mission.castle_source_entity_index,
            false,
            false,
            None,
        );
        assert_eq!(
            mission.castle_life,
            CASTLE_STAGE_ONE_LIFE - FIREFLY_FIREBALL_PAYLOAD
        );
    }

    #[test]
    fn possession_claims_neutral_mana_sphere_and_emits_source_sound() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 20_000,
                y: 20_000,
                z: 0,
            },
            yaw: 0,
            ..FlightState::default()
        };
        let sphere_id = mission
            .spawn_mana_sphere_from_death(
                &FlatWorld,
                50,
                Position {
                    x: 20_000,
                    y: 19_616,
                    z: 100,
                },
                0,
                1,
                300,
                0,
            )
            .unwrap();
        let sphere = mission.mana_spheres.last_mut().unwrap();
        sphere.velocity_x = 0;
        sphere.velocity_y = 0;
        sphere.vertical_velocity = 0;
        assert!(mission.spawn_possession(&player));
        assert_eq!(mission.player_mana, 900);
        mission.step_projectiles(&FlatWorld, &player);
        assert_eq!(
            mission.mana_spheres[0].incoming_owner_entity_index,
            player.entity_index
        );
        mission.step_mana_spheres(&FlatWorld, player.entity_index);
        assert_eq!(
            mission.mana_spheres[0].owner_entity_index,
            player.entity_index
        );
        assert!(mission.events.iter().any(|event| matches!(
            event,
            Level1MissionEvent::ManaSphereClaimed {
                sphere_id: id,
                owner_entity_index: 152,
                sound_id: POSSESSION_SOUND,
            } if *id == sphere_id
        )));
    }

    #[test]
    fn stage_one_carrier_selects_only_owned_spheres_and_deposits_without_loss() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.install_stable_stage_one_fixture(
            &FlatWorld,
            Position {
                x: 10_000,
                y: 10_000,
                z: 0,
            },
            152,
        );
        let player = FlightState {
            entity_index: 152,
            position: Position {
                x: 10_000,
                y: 10_000,
                z: 2_000,
            },
            ..FlightState::default()
        };
        let castle = mission.castle_position;
        for (source, delta_x, mana, owner) in [
            (50, 2_400_u16, 500, 152),
            (89, 3_200_u16, 300, 152),
            (90, 400_u16, 300, 0),
        ] {
            mission
                .spawn_mana_sphere_from_death(
                    &FlatWorld,
                    source,
                    Position {
                        x: castle.x.wrapping_add(delta_x),
                        y: castle.y,
                        z: MANA_CARRIER_GROUND_CLEARANCE,
                    },
                    0,
                    source,
                    mana,
                    owner,
                )
                .unwrap();
            let sphere = mission.mana_spheres.last_mut().unwrap();
            sphere.velocity_x = 0;
            sphere.velocity_y = 0;
            sphere.vertical_velocity = 0;
        }
        for _ in 0..400 {
            mission.step(&FlatWorld, &player);
            if mission.castle_mana == 800 {
                break;
            }
        }
        assert_eq!(mission.castle_mana, 800);
        assert_eq!(
            mission
                .mana_spheres
                .iter()
                .filter(|sphere| sphere.owner_entity_index == 0)
                .map(|sphere| sphere.mana)
                .sum::<i32>(),
            300
        );
        let carrier = mission.mana_carrier().unwrap();
        assert_eq!(carrier.carried_mana, 0);
        assert!(carrier.returning_to_castle);
        assert_eq!(
            carrier.target_sphere_id,
            u32::from(mission.castle_source_entity_index)
        );
    }

    #[test]
    fn four_archers_plus_three_goats_fail_but_four_goats_pass() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.castle_exists = true;
        mission.castle_level = 1;
        mission.castle_mana_capacity = CASTLE_STAGE_ONE_MANA_CAPACITY;
        mission.castle_mana = 4 * (ARCHER_LIFE / 2) + 3 * 300;
        assert_eq!(mission.castle_mana, 2_900);
        assert_eq!(mission.secured_mana_percent(), 14);
        mission.castle_mana += 300;
        assert_eq!(mission.castle_mana, 3_200);
        assert_eq!(mission.secured_mana_percent(), 16);
    }

    #[test]
    fn castle_gate_uses_only_secured_mana_and_exact_integer_percentage() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 50_000);
        mission.current_stage = 3;
        mission.objective_states = [2, 2, 2, 1, 1];
        mission.current_text_selector = 51;
        mission.castle_exists = true;
        mission.castle_level = 1;
        mission.castle_mana = 2_936;
        mission.step(&FlatWorld, &FlightState::default());
        assert_eq!(mission.current_stage, 3);
        assert_eq!(mission.player_mana, 50_000);
        mission.castle_mana = 2_937;
        mission.step(&FlatWorld, &FlightState::default());
        assert_eq!(mission.current_stage, 4);
    }

    #[test]
    fn mana_sphere_constructor_uses_source_identity_and_exact_rng_schedule() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.source_pool = Level1SourcePool::from_free_stack(&[17]).unwrap();
        mission.global_rng = 0x1234_5678;
        let source_rng_after_death = next_rng16(0x1111);
        assert_eq!(source_rng_after_death, 0x4490);
        let next_effect_id = mission.next_effect_id;

        let sphere_id = mission
            .spawn_mana_sphere_from_death(
                &FlatWorld,
                50,
                Position {
                    x: 10_000,
                    y: 20_000,
                    z: 768,
                },
                0x07f0,
                source_rng_after_death,
                500,
                0,
            )
            .unwrap();

        assert_eq!(sphere_id, 17);
        assert_eq!(mission.next_effect_id, next_effect_id);
        let sphere = mission.mana_spheres[0];
        assert_eq!(sphere.source_entity_index, 17);
        assert_eq!(sphere.entity_rng, 0x3ee7);
        assert_eq!(sphere.yaw, 2_046);
        assert_eq!(sphere.vertical_velocity, 32);
        assert_eq!(mission.source_slots[17].entity_rng, 0x3ee7);
        assert_eq!(mission.source_slots[50].entity_rng, 0x4490);
        let mut expected_velocity = Position::default();
        move_entity(&mut expected_velocity, 2_046, 0, 39);
        assert_eq!(sphere.velocity_x, expected_velocity.x.cast_signed());
        assert_eq!(sphere.velocity_y, expected_velocity.y.cast_signed());
    }

    #[test]
    fn failed_mana_sphere_allocation_preserves_death_rng_and_does_not_burn_id() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.source_pool = Level1SourcePool::from_free_stack(&[]).unwrap();
        let next_effect_id = mission.next_effect_id;
        assert_eq!(
            mission.spawn_mana_sphere_from_death(
                &FlatWorld,
                50,
                Position::default(),
                0,
                0x4490,
                300,
                0,
            ),
            None
        );
        assert_eq!(mission.source_slots[50].entity_rng, 0x4490);
        assert_eq!(mission.next_effect_id, next_effect_id);
        assert!(mission.mana_spheres.is_empty());
    }

    #[test]
    fn death_burst_allocates_after_sphere_and_preserves_free_stack_order() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.source_pool = Level1SourcePool::from_free_stack(&[19, 18, 17]).unwrap();
        let position = Position {
            x: 10_000,
            y: 10_000,
            z: 0,
        };

        assert_eq!(
            mission.spawn_mana_sphere_from_death(&FlatWorld, 50, position, 0, 0x4490, 300, 0,),
            Some(17)
        );
        assert_eq!(mission.spawn_death_burst_from_death(50, position), Some(18));
        let (_, later_slot) = mission
            .allocate_effect_source(SourceOccupant::Possession)
            .unwrap();
        assert_eq!(later_slot, 19);
        assert_eq!(
            mission.source_pool.occupant(17),
            Some(SourceOccupant::ManaSphere(17))
        );
        assert_eq!(
            mission.source_pool.occupant(18),
            Some(SourceOccupant::DeathBurst(18))
        );
    }

    #[test]
    fn mana_sphere_ground_bounce_and_slope_impulse_match_source_integer_order() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.source_pool = Level1SourcePool::from_free_stack(&[17]).unwrap();
        mission
            .spawn_mana_sphere_from_death(&FlatWorld, 50, Position::default(), 0, 0x4490, 300, 0)
            .unwrap();
        mission.mana_spheres[0].velocity_x = 0;
        mission.mana_spheres[0].velocity_y = 0;
        mission.mana_spheres[0].vertical_velocity = -128;
        mission.step_mana_sphere_slot(&FlatWorld, 152, 17);
        assert_eq!(mission.mana_spheres[0].vertical_velocity, 32);

        let mut slope = template.session(10_000, 1_000);
        slope.source_pool = Level1SourcePool::from_free_stack(&[17]).unwrap();
        let world = OneCornerSlopeWorld {
            tile_x: 0x20,
            tile_y: 0x30,
            raw_height: 4,
        };
        slope
            .spawn_mana_sphere_from_death(
                &world,
                50,
                Position {
                    x: 0x2000,
                    y: 0x3000,
                    z: 128,
                },
                0,
                0x4490,
                300,
                0,
            )
            .unwrap();
        slope.mana_spheres[0].velocity_x = 0;
        slope.mana_spheres[0].velocity_y = 0;
        slope.mana_spheres[0].vertical_velocity = 0;
        slope.step_mana_sphere_slot(&world, 152, 17);
        assert_eq!(slope.mana_spheres[0].velocity_x, 3);
        assert_eq!(slope.mana_spheres[0].velocity_y, 3);
    }

    #[test]
    fn grounded_merge_keeps_dispatched_slot_and_immediately_reuses_absorbed_slot() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.source_pool = Level1SourcePool::from_free_stack(&[18, 17]).unwrap();
        for (source, mana, owner) in [(50, 500, 0), (51, 300, 152)] {
            mission
                .spawn_mana_sphere_from_death(
                    &FlatWorld,
                    source,
                    Position {
                        x: 10_000,
                        y: 10_000,
                        z: 0,
                    },
                    0,
                    next_rng16(source),
                    mana,
                    owner,
                )
                .unwrap();
            let sphere = mission.mana_spheres.last_mut().unwrap();
            sphere.velocity_x = 0;
            sphere.velocity_y = 0;
            sphere.vertical_velocity = 0;
        }

        mission.step_mana_sphere_slot(&FlatWorld, 152, 17);

        assert_eq!(mission.mana_spheres.len(), 1);
        assert_eq!(mission.mana_spheres[0].source_entity_index, 17);
        assert_eq!(mission.mana_spheres[0].id, 17);
        assert_eq!(mission.mana_spheres[0].mana, 800);
        assert_eq!(mission.mana_spheres[0].owner_entity_index, 152);
        assert_eq!(mission.source_pool.occupant(18), None);
        assert!(!mission.source_slots[18].active);
        assert_eq!(
            mission.allocate_mana_sphere_source(),
            Some(18),
            "sub_57F20 makes the absorbed slot immediately allocatable"
        );
    }

    #[test]
    fn mana_sphere_merge_uses_strict_source_collision_boundary() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.source_pool = Level1SourcePool::from_free_stack(&[18, 17]).unwrap();
        for (source, x) in [(50, 10_000), (51, 10_056)] {
            mission
                .spawn_mana_sphere_from_death(
                    &FlatWorld,
                    source,
                    Position { x, y: 10_000, z: 0 },
                    0,
                    next_rng16(source),
                    500,
                    0,
                )
                .unwrap();
            let sphere = mission.mana_spheres.last_mut().unwrap();
            sphere.velocity_x = 0;
            sphere.velocity_y = 0;
            sphere.vertical_velocity = 0;
        }
        assert_eq!(mana_sphere_collision_dimension(500), 28);
        mission.step_mana_sphere_slot(&FlatWorld, 152, 17);
        assert_eq!(
            mission.mana_spheres.len(),
            2,
            "sub_10630 uses strict less-than at the summed extent"
        );
    }

    #[test]
    fn mana_sphere_merge_uses_search_dat_center_first_order_and_rounded_tile() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.source_pool = Level1SourcePool::from_free_stack(&[19, 18, 17]).unwrap();
        for (source, position) in [
            (
                50,
                Position {
                    x: 240,
                    y: 240,
                    z: 0,
                },
            ),
            (
                51,
                Position {
                    x: 256,
                    y: 256,
                    z: 0,
                },
            ),
            (
                52,
                Position {
                    x: 241,
                    y: 241,
                    z: 0,
                },
            ),
        ] {
            mission
                .spawn_mana_sphere_from_death(
                    &FlatWorld,
                    source,
                    position,
                    0,
                    next_rng16(source),
                    300,
                    0,
                )
                .unwrap();
            let sphere = mission.mana_spheres.last_mut().unwrap();
            sphere.velocity_x = 0;
            sphere.velocity_y = 0;
            sphere.vertical_velocity = 0;
        }

        mission.step_mana_sphere_slot(&FlatWorld, 152, 17);

        assert_eq!(
            mission
                .mana_spheres
                .iter()
                .map(|sphere| sphere.source_entity_index)
                .collect::<Vec<_>>(),
            vec![17, 19],
            "rounded SEARCH.DAT class-0 tile 0x0101 is visited before class-1 tile 0x0000"
        );
        assert_eq!(mission.mana_spheres[0].mana, 600);
        assert_eq!(mission.source_pool.occupant(18), None);
        assert_eq!(
            mission.source_pool.occupant(19),
            Some(SourceOccupant::ManaSphere(19))
        );
    }

    #[test]
    fn mana_sphere_reset_and_parallel_sessions_are_isolated_and_deterministic() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut first = template.session(10_000, 1_000);
        let mut second = template.session(10_000, 1_000);
        for mission in [&mut first, &mut second] {
            mission.source_pool = Level1SourcePool::from_free_stack(&[17]).unwrap();
            mission.global_rng = 0x1234_5678;
            mission
                .spawn_mana_sphere_from_death(
                    &FlatWorld,
                    50,
                    Position {
                        x: 10_000,
                        y: 20_000,
                        z: 768,
                    },
                    0x07f0,
                    0x4490,
                    500,
                    0,
                )
                .unwrap();
        }
        assert_eq!(first.mana_spheres, second.mana_spheres);
        assert_eq!(first.source_arena_hash_v1(), second.source_arena_hash_v1());
        first.mana_spheres[0].mana += 1;
        assert_ne!(first.mana_spheres, second.mana_spheres);

        let reset = template.session(10_000, 1_000);
        assert!(reset.mana_spheres.is_empty());
        assert!(reset.death_bursts.is_empty());
        assert_eq!(reset.next_effect_id, 1);
    }

    #[test]
    fn group_four_constructs_scroll_switches_and_complete_multipart_chain_in_source_order() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.activate_group(4);

        assert_eq!(mission.allocation_slot(304), Some(9));
        assert_eq!(mission.allocation_slot(305), Some(10));
        assert_eq!(mission.allocation_slot(307), Some(11));
        assert_eq!(mission.allocation_slot(308), Some(28));
        let head = mission
            .actors()
            .iter()
            .find(|actor| actor.authored_index == 307)
            .copied()
            .unwrap();
        assert_eq!(head.kind, Level1ActorKind::MultipartHead);
        assert_eq!(head.source_entity_index, 11);
        assert_eq!(head.owner_id, 11);
        assert_eq!(head.action, MULTIPART_ROAM_ACTION);
        assert_eq!(head.actual_speed, MULTIPART_INITIAL_SPEED);
        assert_eq!(head.multipart_first_segment_index, 12);
        assert_eq!(mission.multipart_segments().len(), MULTIPART_SEGMENT_COUNT);
        for (ordinal, segment) in mission.multipart_segments().iter().enumerate() {
            assert_eq!(segment.source_entity_index, 12 + ordinal as u16);
            assert_eq!(segment.ordinal, ordinal as u8);
            assert_eq!(
                segment.predecessor_source_entity_index,
                if ordinal == 0 {
                    head.source_entity_index
                } else {
                    segment.source_entity_index - 1
                }
            );
            assert_eq!(
                segment.successor_source_entity_index,
                if ordinal + 1 == MULTIPART_SEGMENT_COUNT {
                    0
                } else {
                    segment.source_entity_index + 1
                }
            );
            assert_eq!(segment.action, MULTIPART_SEGMENT_ACTION);
            assert_eq!(segment.owner_id, head.source_entity_index.cast_signed());
            assert_eq!(
                segment.render_parameter_index,
                89 + u16::try_from(ordinal).unwrap()
            );
            assert_eq!(
                segment.collision_half_width,
                MULTIPART_SEGMENT_COLLISION_HALF_WIDTHS[ordinal]
            );
            assert_eq!(
                segment.trailing_distance,
                MULTIPART_SEGMENT_TRAILING_DISTANCES[ordinal]
            );
            assert_eq!(
                mission.source_pool.occupant(segment.source_entity_index),
                Some(SourceOccupant::MultipartSegment(
                    segment.source_entity_index
                ))
            );
        }
    }

    #[test]
    fn multipart_constructor_keeps_head_and_allocated_prefix_when_the_arena_exhausts() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        let free_stack = (1_u16..=14).rev().collect::<Vec<_>>();
        mission.source_pool = Level1SourcePool::from_free_stack(&free_stack).unwrap();
        mission.activate_group(4);

        assert_eq!(mission.allocation_slot(307), Some(11));
        assert_eq!(mission.multipart_segments().len(), 3);
        assert_eq!(
            mission
                .multipart_segments()
                .iter()
                .map(|segment| segment.source_entity_index)
                .collect::<Vec<_>>(),
            vec![12, 13, 14]
        );
        assert_eq!(mission.allocation_slot(308), Some(0));
        assert!(
            mission
                .actors()
                .iter()
                .find(|actor| actor.authored_index == 307)
                .is_some_and(|head| head.active),
            "the remc2 constructor does not roll back a partially built body"
        );
    }

    #[test]
    fn switch_305_uses_strict_wrapped_xy_cadence_and_releases_group_33_without_sound() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut accepted = template.session(10_000, 1_000);
        accepted.activate_group(4);
        let switch_slot = accepted.allocation_slot(305).unwrap();
        let center = accepted
            .authored_allocations
            .iter()
            .find(|allocation| allocation.authored_index == 305)
            .unwrap()
            .position;
        let player = FlightState {
            position: Position {
                x: center.x.wrapping_add(2_168),
                y: center.y,
                z: i16::MAX,
            },
            ..FlightState::default()
        };
        accepted.step_switch_slot(&player, switch_slot);
        assert!(
            accepted
                .actors()
                .iter()
                .find(|actor| actor.authored_index == 306)
                .is_some_and(|firefly| firefly.active)
        );
        assert!(
            accepted
                .events()
                .iter()
                .all(|event| !matches!(event, Level1MissionEvent::MissionSound { .. }))
        );

        let mut rejected = template.session(10_000, 1_000);
        rejected.activate_group(4);
        let switch_slot = rejected.allocation_slot(305).unwrap();
        let player = FlightState {
            position: Position {
                x: center.x.wrapping_add(2_169),
                y: center.y,
                z: i16::MIN,
            },
            ..FlightState::default()
        };
        rejected.step_switch_slot(&player, switch_slot);
        assert!(
            rejected
                .actors()
                .iter()
                .find(|actor| actor.authored_index == 306)
                .is_some_and(|firefly| !firefly.active)
        );
        rejected
            .authored_allocations
            .iter_mut()
            .find(|allocation| allocation.authored_index == 305)
            .unwrap()
            .phase = 1;
        let player = FlightState {
            position: center,
            ..FlightState::default()
        };
        rejected.step_switch_slot(&player, switch_slot);
        assert!(
            rejected
                .actors()
                .iter()
                .find(|actor| actor.authored_index == 306)
                .is_some_and(|firefly| !firefly.active),
            "non-cadence phases must not sample overlap"
        );
    }

    #[test]
    fn switch_308_waits_for_dying_body_then_counts_seventeen_eligible_dispatches() {
        let template = Level1MissionTemplate::decode(&authored_bytes(), &FlatWorld).unwrap();
        let mut mission = template.session(10_000, 1_000);
        mission.activate_group(4);
        let gate_slot = mission.allocation_slot(308).unwrap();
        let player = FlightState::default();

        mission.step_switch_slot(&player, gate_slot);
        assert_eq!(
            mission
                .authored_allocations
                .iter()
                .find(|allocation| allocation.authored_index == 308)
                .unwrap()
                .state_timer,
            0,
            "the live head blocks timer arming"
        );

        let head_index = mission
            .actors
            .iter()
            .position(|actor| actor.authored_index == 307)
            .unwrap();
        mission.actors[head_index].active = false;
        mission.actors[head_index].life = -1;
        for segment in &mut mission.multipart_segments {
            segment.action = MULTIPART_KILL_ACTION;
        }
        mission.step_switch_slot(&player, gate_slot);
        assert_eq!(
            mission
                .authored_allocations
                .iter()
                .find(|allocation| allocation.authored_index == 308)
                .unwrap()
                .state_timer,
            0,
            "live action-29 body nodes remain on the model-3 family list"
        );

        let segment_slots = mission
            .multipart_segments
            .iter()
            .map(|segment| segment.source_entity_index)
            .collect::<Vec<_>>();
        for slot in segment_slots {
            mission.source_pool.disable_slot(slot);
            mission.source_slots[usize::from(slot)].control_flags |= SOURCE_DISABLED_FLAG;
        }
        mission.multipart_segments.clear();
        mission.mark_allocation_pending_delete(307);
        mission.cleanup_pending_allocations();

        mission.step_switch_slot(&player, gate_slot);
        assert_eq!(
            mission
                .authored_allocations
                .iter()
                .find(|allocation| allocation.authored_index == 308)
                .unwrap()
                .state_timer,
            16
        );
        for _ in 0..15 {
            mission.step_switch_slot(&player, gate_slot);
        }
        assert_eq!(
            mission
                .authored_allocations
                .iter()
                .find(|allocation| allocation.authored_index == 308)
                .unwrap()
                .state_timer,
            1
        );
        assert_eq!(mission.count_alive(Level1ActorKind::MultipartHead), 0);

        mission.step_switch_slot(&player, gate_slot);
        assert_eq!(mission.count_alive(Level1ActorKind::MultipartHead), 3);
        assert_eq!(mission.multipart_segments().len(), 48);
        assert_eq!(
            mission
                .events()
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        Level1MissionEvent::MissionSound {
                            sound_id: EXIT_SWITCH_SOUND,
                            ..
                        }
                    )
                })
                .count(),
            1
        );
    }
}
