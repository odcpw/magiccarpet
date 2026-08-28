//! `sim-core` bridge to the shared `sim-ground-motion` owner crate.
//!
//! Creature families keep their established crate-private API and authoritative
//! [`crate::FlightWorld`] projection. The extracted crate owns only the pure
//! retry and terrain-transition algorithm.

use crate::{FlightWorld, Position};

pub(crate) use sim_ground_motion::{GroundCreatureMotionConfig, GroundCreatureMoveResult};

pub(crate) type GroundCreatureMotion = sim_ground_motion::GroundCreatureMotion<Position>;

impl sim_ground_motion::GroundPosition for Position {
    fn from_components(x: u16, y: u16, z: i16) -> Self {
        Self { x, y, z }
    }

    fn x(self) -> u16 {
        self.x
    }

    fn y(self) -> u16 {
        self.y
    }

    fn z(self) -> i16 {
        self.z
    }

    fn with_z(self, z: i16) -> Self {
        Self { z, ..self }
    }
}

struct GroundWorldAdapter<'a, W>(&'a W);

impl<W: FlightWorld> sim_ground_motion::GroundMotionWorld<Position> for GroundWorldAdapter<'_, W> {
    fn ground_height(&self, position: Position) -> i16 {
        self.0.ground_height(position)
    }

    fn material_mask(&self, position: Position) -> u32 {
        self.0.material_mask(position)
    }

    fn move_entity(&self, position: &mut Position, yaw: i16, pitch: i16, speed: i16) {
        crate::move_entity(position, yaw, pitch, speed);
    }

    fn turn_toward(&self, from: i16, to: i16, maximum: i16) -> i16 {
        crate::turn_toward(from, to, maximum)
    }
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
    sim_ground_motion::step_ground_creature_motion(
        position,
        yaw,
        desired_heading,
        actual_speed,
        &GroundWorldAdapter(world),
        config,
    )
}

pub(crate) fn relax_to_terrain<W: FlightWorld>(
    position: &mut Position,
    world: &W,
    ground_clearance: i16,
    vertical_relaxation: i16,
) {
    sim_ground_motion::relax_to_terrain(
        position,
        &GroundWorldAdapter(world),
        ground_clearance,
        vertical_relaxation,
    );
}

#[cfg(test)]
pub(crate) fn candidate_is_valid<W: FlightWorld>(
    world: &W,
    position: Position,
    terrain_mask: u32,
    maximum_tile_slope: i32,
) -> bool {
    sim_ground_motion::candidate_is_valid(
        &GroundWorldAdapter(world),
        position,
        terrain_mask,
        maximum_tile_slope,
    )
}

#[cfg(test)]
pub(crate) fn second_avoidance_heading(original: i16) -> i16 {
    sim_ground_motion::second_avoidance_heading(original)
}

#[cfg(test)]
pub(crate) fn opposite_heading(original: i16) -> i16 {
    sim_ground_motion::opposite_heading(original)
}

#[cfg(test)]
pub(crate) fn tile_slope_metric<W: FlightWorld>(world: &W, position: Position) -> i32 {
    sim_ground_motion::tile_slope_metric(&GroundWorldAdapter(world), position)
}
