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

    fn clearance_blocked(&self, position: Position, required_height: i32) -> bool {
        self.0.cave_collision_blocked(position)
            || self.0.ceiling_height(position).is_some_and(|ceiling| {
                i32::from(self.0.ground_height(position)) + required_height > i32::from(ceiling)
            })
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
    let motion = sim_ground_motion::step_ground_creature_motion(
        position,
        yaw,
        desired_heading,
        actual_speed,
        &GroundWorldAdapter(world),
        config,
    );
    // sub_1B8C0 retains the last proposal on both acceptance and rejection.
    // Callers implementing its 0x800 hold return before reaching this bridge.
    world.set_source_predicted_position(motion.last_predicted);
    motion
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

#[cfg(test)]
mod tests {
    use super::*;
    use sim_ground_motion::GroundMotionWorld;

    #[test]
    fn blocked_motion_retains_last_proposal_without_changing_geometry_identity() {
        use crate::native_world::{MapType, SessionTerrain, TerrainPlanes};

        let terrain = SessionTerrain::from_planes(
            MapType::Day,
            TerrainPlanes {
                terrain_types: vec![0; 65_536].into_boxed_slice(),
                ground_heights: vec![0; 65_536].into_boxed_slice(),
                shading: vec![0; 65_536].into_boxed_slice(),
                angles: vec![0; 65_536].into_boxed_slice(),
                secondary_heights: vec![0; 65_536].into_boxed_slice(),
                entity_indices: vec![0; 65_536].into_boxed_slice(),
            },
            0,
        );
        let geometry_hash = terrain.deterministic_hash();
        let origin = Position {
            x: 0x1000,
            y: 0x2000,
            z: 256,
        };
        let motion = step_ground_creature_motion(
            origin,
            0,
            0,
            256,
            &terrain,
            GroundCreatureMotionConfig {
                half_width: 0,
                half_height: 0,
                ground_clearance: 0,
                descent: -128,
                turn_limit: 0,
                terrain_mask: 0,
                maximum_tile_slope: 1,
            },
        );
        assert_eq!(motion.result, GroundCreatureMoveResult::Blocked);
        assert_eq!(motion.position, origin);
        assert_eq!(
            motion.last_predicted,
            Position {
                x: 0x1000,
                y: 0x2100,
                z: 128,
            }
        );
        assert_eq!(terrain.source_predicted_position(), motion.last_predicted);
        assert_eq!(terrain.deterministic_hash(), geometry_hash);
        let restored = terrain.clone();
        terrain.set_source_predicted_position(origin);
        assert_eq!(restored.source_predicted_position(), motion.last_predicted);
        assert_eq!(terrain.source_predicted_position(), origin);
    }

    struct CaveWorld {
        ceiling: Option<i16>,
        angle: u8,
    }

    impl FlightWorld for CaveWorld {
        fn ground_height(&self, _position: Position) -> i16 {
            100
        }
        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            self.ceiling
        }
        fn material_mask(&self, _position: Position) -> u32 {
            0
        }
        fn angle_flags(&self, _tile: crate::Tile) -> u8 {
            self.angle
        }
    }

    #[test]
    fn cave_probe_uses_angle_bit_and_exact_ground_relative_body_clearance() {
        let position = Position {
            x: 0x1000,
            y: 0x2000,
            z: 999,
        };
        // The source predicate uses terrain height, not current flight Z.
        assert!(
            !GroundWorldAdapter(&CaveWorld {
                ceiling: Some(202),
                angle: 0
            })
            .clearance_blocked(position, 102)
        );
        assert!(
            GroundWorldAdapter(&CaveWorld {
                ceiling: Some(201),
                angle: 0
            })
            .clearance_blocked(position, 102)
        );
        assert!(
            GroundWorldAdapter(&CaveWorld {
                ceiling: Some(2000),
                angle: 8
            })
            .clearance_blocked(position, 102)
        );
        assert!(
            !GroundWorldAdapter(&CaveWorld {
                ceiling: None,
                angle: 8
            })
            .clearance_blocked(position, 102)
        );
    }
}
