//! Creature content consumed by the family constructors and action kernels.
//!
//! MC2 selector numbers, environment admission and sprite/audio bindings stay
//! in the adapters. These values describe the actual movement and combat rules.

use serde::{Deserialize, Serialize};

/// Bee constructor, flying motion, target selection and contact attack rules.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BeeClassProperties {
    pub max_life: i32,
    pub initial_mana: i32,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub initial_speed: i16,
    pub attack_damage: u16,
    pub scan_period: u8,
    pub target_range: u32,
    pub target_half_angle: i16,
    pub turn_limit: i16,
    pub turn_response: i16,
    pub ground_clearance: i16,
    pub descent: i16,
    pub maximum_slope: i32,
    pub collision_half_width: i16,
    pub collision_half_height: i16,
    pub collision_vertical_offset: i16,
    pub behavior_flags: u8,
}

/// `sub_4B590` and descriptor 73 in the pinned MC2 source.
pub const BEE_CLASS_PROPERTIES: BeeClassProperties = BeeClassProperties {
    max_life: 3000,
    initial_mana: 1500,
    minimum_speed: 64,
    maximum_speed: 30,
    initial_speed: 32,
    attack_damage: 200,
    scan_period: 30,
    target_range: 5120,
    target_half_angle: 682,
    turn_limit: 113,
    turn_response: 17,
    ground_clearance: 51,
    descent: -32,
    maximum_slope: 20,
    collision_half_width: 128,
    collision_half_height: 128,
    // Particle 3 keeps rotSpeed_8 / 2 after SetEntityShiftRot replaces the other axes.
    collision_vertical_offset: 75,
    behavior_flags: 1,
};

/// Linked creature construction, growth, terrain following and body geometry.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Creature22ClassProperties {
    pub max_life: i32,
    pub initial_mana: i32,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub default_length: i8,
    pub maximum_length: i8,
    pub growth_period: u16,
    pub mana_growth: i32,
    pub mana_growth_limit: i32,
    pub ground_clearance: i16,
    pub descent: i16,
    pub climb_step: i16,
    pub steep_climb_step: i16,
    pub climb_hold: i16,
    pub maximum_slope: i32,
    pub terrain_mask: u32,
    pub turn_limit: i16,
    pub turn_response: i16,
    pub scan_period: u8,
    pub minimum_rotation: i16,
    pub maximum_rotation: i16,
    pub shrink_delay: i32,
    pub scale_per_mille: i32,
}

/// `sub_4CA00`, descriptor 90 and the `sub_26FF0`/`sub_27xxx` family.
pub const CREATURE22_CLASS_PROPERTIES: Creature22ClassProperties = Creature22ClassProperties {
    max_life: 2000,
    initial_mana: 1000,
    minimum_speed: 128,
    maximum_speed: 16,
    default_length: 15,
    maximum_length: 15,
    growth_period: 1024,
    mana_growth: 1000,
    mana_growth_limit: 50000,
    ground_clearance: 384,
    descent: 2,
    climb_step: 64,
    steep_climb_step: 256,
    climb_hold: 64,
    maximum_slope: 50,
    terrain_mask: 0xffff_ffff,
    turn_limit: 256,
    turn_response: 5,
    scan_period: 20,
    minimum_rotation: 11,
    maximum_rotation: 227,
    shrink_delay: 128,
    scale_per_mille: 550,
};

/// Cave creature movement, target acquisition and melee rules.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Creature24ClassProperties {
    pub max_life: i32,
    pub initial_mana: i32,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub attack_damage: u16,
    pub melee_range: u32,
    pub scan_period: u8,
    /// The target scan has its own cadence, independent of the descriptor.
    pub targeting_scan_period: u8,
    pub target_range: u32,
    pub target_half_angle: i16,
    pub turn_limit: i16,
    pub turn_response: i16,
    pub pursuit_turn_limit: i16,
    pub pursuit_turn_response: i16,
    pub ground_clearance: i16,
    pub descent: i16,
    pub maximum_slope: i32,
    pub terrain_mask: u32,
    pub collision_half_width: i16,
    pub collision_half_height: i16,
    pub behavior_flags: u8,
}

/// `sub_4CCF0`, descriptor 102, `sub_28690` and melee helper `sub_1CF20`.
pub const CREATURE24_CLASS_PROPERTIES: Creature24ClassProperties = Creature24ClassProperties {
    max_life: 16000,
    initial_mana: 8000,
    minimum_speed: 80,
    maximum_speed: 24,
    attack_damage: 1500,
    melee_range: 1536,
    scan_period: 8,
    targeting_scan_period: 16,
    target_range: 3072,
    target_half_angle: 853,
    turn_limit: 170,
    turn_response: 5,
    pursuit_turn_limit: 22,
    pursuit_turn_response: 5,
    ground_clearance: 0,
    descent: -256,
    maximum_slope: 20,
    terrain_mask: 0xfff0_80fe,
    collision_half_width: 256,
    collision_half_height: 640,
    behavior_flags: 7,
};
