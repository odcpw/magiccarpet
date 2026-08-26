//! Shared source-shaped ground-creature movement.
//!
//! remc2's `sub_1B8C0` movement packet is used by several class-5 creature
//! families.  This module owns the integer retry/terrain transition; family
//! modules retain their own cadence, targeting, RNG, and lifecycle rules.

use crate::{FlightWorld, Position, move_entity, turn_toward};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GroundCreatureMoveResult {
    DirectSameTile,
    DirectCrossTile,
    Alternate,
    Blocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GroundCreatureMotion {
    pub(crate) position: Position,
    pub(crate) yaw: i16,
    pub(crate) movement_blocked: bool,
    pub(crate) killed: bool,
    pub(crate) result: GroundCreatureMoveResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GroundCreatureMotionConfig {
    pub(crate) ground_clearance: i16,
    pub(crate) descent: i16,
    pub(crate) turn_limit: i16,
    pub(crate) terrain_mask: u32,
    pub(crate) maximum_tile_slope: i32,
}

#[must_use]
pub(crate) fn step_ground_creature_motion<W: FlightWorld>(
    position: Position,
    yaw: i16,
    desired_heading: i16,
    actual_speed: i16,
    world: &W,
    config: GroundCreatureMotionConfig,
) -> GroundCreatureMotion {
    let headings = [
        yaw,
        yaw.wrapping_add(341) & 0x07ff,
        second_avoidance_heading(yaw),
        opposite_heading(yaw),
    ];

    for (attempt, heading) in headings.into_iter().enumerate() {
        let mut predicted = position;
        relax_to_terrain(
            &mut predicted,
            world,
            config.ground_clearance,
            config.descent,
        );
        move_entity(&mut predicted, heading, 0, actual_speed);

        let changed_tile =
            (position.x >> 8, position.y >> 8) != (predicted.x >> 8, predicted.y >> 8);
        if changed_tile
            && !candidate_is_valid(
                world,
                predicted,
                config.terrain_mask,
                config.maximum_tile_slope,
            )
        {
            continue;
        }

        return GroundCreatureMotion {
            position: predicted,
            yaw: heading.wrapping_add(turn_toward(heading, desired_heading, config.turn_limit))
                & 0x07ff,
            movement_blocked: attempt != 0,
            killed: false,
            result: if attempt == 0 && changed_tile {
                GroundCreatureMoveResult::DirectCrossTile
            } else if attempt == 0 {
                GroundCreatureMoveResult::DirectSameTile
            } else {
                GroundCreatureMoveResult::Alternate
            },
        };
    }

    GroundCreatureMotion {
        position,
        yaw: opposite_heading(yaw),
        movement_blocked: true,
        killed: true,
        result: GroundCreatureMoveResult::Blocked,
    }
}

pub(crate) fn relax_to_terrain<W: FlightWorld>(
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

pub(crate) fn candidate_is_valid<W: FlightWorld>(
    world: &W,
    position: Position,
    terrain_mask: u32,
    maximum_tile_slope: i32,
) -> bool {
    world.material_mask(position) & !terrain_mask == 0
        && tile_slope_metric(world, position) < maximum_tile_slope
}

pub(crate) fn second_avoidance_heading(original: i16) -> i16 {
    let original = original.cast_unsigned();
    let low = original.wrapping_sub(85) & 0x00ff;
    let high = original.wrapping_sub(341) & 0x0700;
    (high | low).cast_signed()
}

pub(crate) fn opposite_heading(original: i16) -> i16 {
    let original = original.cast_unsigned();
    (((original.wrapping_add(0x0400)) & 0x0700) | (original & 0x00ff)).cast_signed()
}

pub(crate) fn tile_slope_metric<W: FlightWorld>(world: &W, position: Position) -> i32 {
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
