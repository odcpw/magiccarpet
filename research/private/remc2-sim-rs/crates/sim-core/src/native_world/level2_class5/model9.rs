//! Level 2 class-5/model-9 movement for the authored action-72 roots.

use crate::{FlightWorld, Position, Tile, WATER_AMBIENCE_MASK};

use super::super::*;

/// `str_D7BD6[80]`, the descriptor installed by `sub_4BBB0` for model 9.
/// The row's `dword_0x14` is `0xffff_ffff`, so the source material mask is
/// empty: model-9 movement rejects cave/terrain geometry, not a material
/// class.  The remaining values are the source's movement geometry and turn
/// contract.
const MODEL9_SLOPE_LIMIT: i32 = 0x0046;
const MODEL9_CAVE_CLEARANCE: i32 = 0x0200;
const MODEL9_PARTICLE_FOV: i32 = 125;
const MODEL9_TURN_LIMIT: i16 = 0x0016;
const MODEL9_YAW_MASK: i32 = 0x07ff;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Model9MoveResult {
    Accepted { heading: i16, position: Position },
    Blocked { heading: i16 },
}

impl Level2EconomyState {
    #[inline]
    pub(super) fn step_authored_class5_model9(&self, source: &mut Level2AuthoredClass5Source) {
        // The extracted Level 2 source cursor currently carries model-9 roots
        // as the action-72 production rows.  Keep that route live while
        // applying the source's movement/collision body to each callback.
        let _ = model9_step_movement(source, &self.terrain);
    }
}

/// Port the four candidate headings used by `sub_1B8C0`.
///
/// The third candidate deliberately preserves the source's split-byte write;
/// it is not equivalent to subtracting 341 from the original heading.
fn model9_retry_headings(yaw: i16) -> [i16; 4] {
    let yaw = i32::from(yaw);
    let first = yaw & MODEL9_YAW_MASK;
    let second = (yaw + 341) & MODEL9_YAW_MASK;
    let third = ((yaw.wrapping_sub(85) as u16 & 0x00ff)
        | ((((yaw.wrapping_sub(341) as u16) >> 8) & 7) << 8))
        & MODEL9_YAW_MASK as u16;
    let fourth = ((((yaw + 0x0400) as u16) & 0x0700) | (yaw as u8 as u16)) & MODEL9_YAW_MASK as u16;
    [
        i16::try_from(first).expect("model-9 retry heading fits i16"),
        i16::try_from(second).expect("model-9 retry heading fits i16"),
        i16::try_from(third).expect("model-9 retry heading fits i16"),
        i16::try_from(fourth).expect("model-9 retry heading fits i16"),
    ]
}

/// Exact shortest-angle turn used after a successful source candidate.
fn model9_turn_delta(from: i16, to: i16) -> i16 {
    let from = i32::from(from) & MODEL9_YAW_MASK;
    let to = i32::from(to) & MODEL9_YAW_MASK;
    let mut delta = to - from;
    if delta.abs() > 1024 {
        delta += if delta >= 0 { -2048 } else { 2048 };
    }
    if delta == 0 {
        return 0;
    }
    let magnitude = delta.abs().min(i32::from(MODEL9_TURN_LIMIT));
    i16::try_from(magnitude * delta.signum()).expect("model-9 turn delta fits i16")
}

fn model9_heading_after_turn(heading: i16, roll: i16) -> i16 {
    i16::try_from(
        (i32::from(heading) + i32::from(model9_turn_delta(heading, roll))) & MODEL9_YAW_MASK,
    )
    .expect("masked model-9 heading fits i16")
}

/// `sub_1B7A0`'s four-corner steep-terrain measure, expressed through the
/// typed terrain seam. Ground heights are stored in the source's 5-bit fixed
/// point scale, so the corner samples are shifted back before comparison.
fn model9_slope_measure<W: FlightWorld>(position: Position, world: &W) -> i32 {
    let tile_x = position.x.to_be_bytes()[0];
    let tile_y = position.y.to_be_bytes()[0];
    let height = |x: u8, y: u8| {
        i32::from(world.ground_height(Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        })) >> 5
    };
    let point_1 = height(tile_x, tile_y);
    let point_2 = height(tile_x.wrapping_add(1), tile_y);
    let point_3 = height(tile_x.wrapping_add(1), tile_y.wrapping_add(1));
    let point_4 = height(tile_x, tile_y.wrapping_add(1));
    let diagonal_1 = (point_1 + point_4 - point_2 - point_3).abs();
    let diagonal_2 = (point_1 + point_2 - point_4 - point_3).abs();
    diagonal_1.max(diagonal_2)
}

/// The model-9 `sub_102D0(..., 1)` cave part. Particle 220 supplies a 125
/// unit fov and descriptor row 80 supplies the 512-unit ground clearance.
/// There is no material rejection for this descriptor.
fn model9_cave_collision_blocked<W: FlightWorld>(candidate: Position, world: &W) -> bool {
    let Some(ceiling) = world.ceiling_height(candidate) else {
        return false;
    };
    let tile = Tile {
        x: candidate.x.to_be_bytes()[0],
        y: candidate.y.to_be_bytes()[0],
    };
    world.angle_flags(tile) & 8 != 0
        || i32::from(world.ground_height(candidate)) + MODEL9_PARTICLE_FOV + MODEL9_CAVE_CLEARANCE
            > i32::from(ceiling)
}

fn model9_candidate_blocked<W: FlightWorld>(candidate: Position, world: &W) -> bool {
    // `sub_102D0` samples the candidate for model 9's zero-width/zero-depth
    // particle footprint. Keep the trait hook in the predicate so cave
    // worlds can provide their source-backed topology check as well as the
    // explicit descriptor clearance check below.
    world.cave_collision_blocked(candidate)
        || model9_cave_collision_blocked(candidate, world)
        || model9_slope_measure(candidate, world) >= MODEL9_SLOPE_LIMIT
}

fn model9_step_movement<W: FlightWorld>(
    source: &mut Level2AuthoredClass5Source,
    world: &W,
) -> Model9MoveResult {
    let before = source.position;
    let headings = model9_retry_headings(source.yaw);
    for heading in headings {
        source.yaw = heading;
        let mut candidate = before;
        crate::move_entity(&mut candidate, heading, 0, source.actual_speed);
        candidate.z = world.ground_height(candidate);
        let crossed_tile = before.x >> 8 != candidate.x >> 8 || before.y >> 8 != candidate.y >> 8;
        if !crossed_tile || !model9_candidate_blocked(candidate, world) {
            source.position = candidate;
            source.yaw = model9_heading_after_turn(heading, source.roll);
            return Model9MoveResult::Accepted {
                heading,
                position: candidate,
            };
        }
    }

    // Descriptor row 80 does not kill on a geometric all-blocked result, but
    // the source still applies its common water death rule at the origin.
    if world.material_mask(before) == WATER_AMBIENCE_MASK {
        source.life = -1;
    }
    Model9MoveResult::Blocked {
        heading: source.yaw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, Default)]
    struct Model9TestWorld {
        block_all: bool,
        material: u32,
        ceiling: Option<i16>,
        angles: u8,
        blocked_tile: Option<(u8, u8)>,
        steep_corner: Option<(u8, u8, i16)>,
    }

    impl FlightWorld for Model9TestWorld {
        fn ground_height(&self, position: Position) -> i16 {
            self.steep_corner
                .filter(|(x, y, _)| {
                    position.x.to_be_bytes()[0] == *x && position.y.to_be_bytes()[0] == *y
                })
                .map_or(0, |(_, _, height)| height)
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            self.ceiling
        }

        fn material_mask(&self, _position: Position) -> u32 {
            self.material
        }

        fn angle_flags(&self, tile: Tile) -> u8 {
            self.angles
                | u8::from(self.block_all || self.blocked_tile == Some((tile.x, tile.y))) * 8
        }

        fn cave_collision_blocked(&self, _position: Position) -> bool {
            false
        }
    }

    fn model9_source(position: Position, yaw: i16, roll: i16) -> Level2AuthoredClass5Source {
        Level2AuthoredClass5Source {
            source_entity_index: 10,
            entity_rng: 0x1234,
            authored_index: 245,
            model: 9,
            action: 72,
            minimum_speed: 20,
            maximum_speed: 0,
            actual_speed: 20,
            max_life: 1_000,
            life: 1_000,
            mana: 500,
            max_mana: 0,
            subspell_index: 500,
            dword_0x10: 16,
            pending_word_0x62: 0,
            word_0x26: 0,
            word_0x24: 0,
            byte_0x3b: 0,
            word_0x96_150: 0,
            byte_0x46_70: 0,
            particle_index: 220,
            source_byte_3e: 0,
            source_byte_39: 0,
            yaw,
            pitch: 0,
            roll,
            fov: 0,
            shift_rot_x: 128,
            shift_rot_y: 128,
            position,
            phase: 0,
            drawing_enabled: true,
            active: true,
        }
    }

    #[test]
    fn level_two_model9_action72_matches_collision_and_retry_order() {
        assert_eq!(model9_retry_headings(0), [0, 341, 1707, 1024]);

        let mut world = Model9TestWorld::default();
        let mut source = model9_source(
            Position {
                x: 0x00f0,
                y: 0x01f0,
                z: 0,
            },
            0,
            1024,
        );
        source.actual_speed = 512;
        world.ceiling = Some(2_000);
        let mut first_position = source.position;
        crate::move_entity(&mut first_position, 0, 0, 512);
        world.blocked_tile = Some((
            first_position.x.to_be_bytes()[0],
            first_position.y.to_be_bytes()[0],
        ));
        let expected_heading = model9_retry_headings(0)[1];
        let mut expected_position = source.position;
        crate::move_entity(&mut expected_position, expected_heading, 0, 512);
        expected_position.z = 0;
        let result = model9_step_movement(&mut source, &world);
        assert_eq!(
            result,
            Model9MoveResult::Accepted {
                heading: expected_heading,
                position: expected_position,
            }
        );
        assert_eq!(source.position, expected_position);
        assert_eq!(source.yaw, expected_heading + MODEL9_TURN_LIMIT);

        // A 70-point corner delta is the descriptor's steep-terrain boundary.
        world.steep_corner = Some((2, 1, 2_240));
        assert!(model9_candidate_blocked(
            Position {
                x: 0x0200,
                y: 0x0100,
                z: 0,
            },
            &world,
        ));

        // The cave angle bit and the source's fov+clearance envelope are
        // independent rejection paths.
        world.steep_corner = None;
        world.ceiling = Some(600);
        assert!(model9_candidate_blocked(
            Position {
                x: 0x0200,
                y: 0x0100,
                z: 0,
            },
            &world,
        ));
        world.ceiling = Some(2_000);
        world.angles = 8;
        assert!(model9_candidate_blocked(
            Position {
                x: 0x0200,
                y: 0x0100,
                z: 0,
            },
            &world,
        ));

        // All candidates blocked leaves the source in place, retains the
        // fourth attempted yaw, and applies the source water death rule.
        let mut blocked_source = model9_source(
            Position {
                x: 0x00f0,
                y: 0x01f0,
                z: 0,
            },
            0,
            0,
        );
        blocked_source.actual_speed = 512;
        blocked_source.life = 7;
        world.block_all = true;
        world.material = WATER_AMBIENCE_MASK;
        let blocked_position = blocked_source.position;
        let result = model9_step_movement(&mut blocked_source, &world);
        assert_eq!(
            result,
            Model9MoveResult::Blocked {
                heading: model9_retry_headings(0)[3],
            }
        );
        assert_eq!(blocked_source.position, blocked_position);
        assert_eq!(blocked_source.yaw, model9_retry_headings(0)[3]);
        assert_eq!(blocked_source.life, -1);
    }
}
