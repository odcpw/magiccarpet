//! Shared source-shaped ground-creature movement.
//!
//! This crate owns the package-neutral retry and terrain-transition algorithm.
//! Callers supply their authoritative terrain and movement kernel through
//! [`GroundMotionWorld`]; creature families retain cadence, targeting, RNG,
//! damage, and lifecycle state.

#![forbid(unsafe_code)]

/// Fixed-width position operations consumed by the ground-motion algorithm.
pub trait GroundPosition: Copy {
    fn from_components(x: u16, y: u16, z: i16) -> Self;

    fn x(self) -> u16;

    fn y(self) -> u16;

    fn z(self) -> i16;

    fn with_z(self, z: i16) -> Self;
}

/// Authoritative terrain and movement operations borrowed by one motion step.
pub trait GroundMotionWorld<P: GroundPosition> {
    fn ground_height(&self, position: P) -> i16;

    fn material_mask(&self, position: P) -> u32;

    /// Source cave topology and ground-relative body clearance at one probe.
    fn clearance_blocked(&self, position: P, required_height: i32) -> bool;

    fn move_entity(&self, position: &mut P, yaw: i16, pitch: i16, speed: i16);

    fn turn_toward(&self, from: i16, to: i16, maximum: i16) -> i16;
}

/// Which source retry admitted the movement candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroundCreatureMoveResult {
    DirectSameTile,
    DirectCrossTile,
    Alternate,
    Blocked,
}

/// Complete result of one ground-creature motion packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroundCreatureMotion<P> {
    pub position: P,
    pub yaw: i16,
    pub movement_blocked: bool,
    pub killed: bool,
    pub result: GroundCreatureMoveResult,
}

/// Package-bound constants used by the shared motion packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroundCreatureMotionConfig {
    /// Maximum horizontal half extent (`max(array.pitch, array.roll)`).
    pub half_width: i16,
    /// Vertical half extent (`array.fov`).
    pub half_height: i16,
    pub ground_clearance: i16,
    pub descent: i16,
    pub turn_limit: i16,
    pub terrain_mask: u32,
    pub maximum_tile_slope: i32,
}

/// Execute the exact four-heading retry and terrain transition.
#[must_use]
#[inline]
pub fn step_ground_creature_motion<P, W>(
    position: P,
    yaw: i16,
    desired_heading: i16,
    actual_speed: i16,
    world: &W,
    config: GroundCreatureMotionConfig,
) -> GroundCreatureMotion<P>
where
    P: GroundPosition,
    W: GroundMotionWorld<P>,
{
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
        world.move_entity(&mut predicted, heading, 0, actual_speed);

        let changed_tile =
            (position.x() >> 8, position.y() >> 8) != (predicted.x() >> 8, predicted.y() >> 8);
        if (attempt != 0 || changed_tile)
            && !body_candidate_is_valid(world, predicted, heading, config)
        {
            continue;
        }

        return GroundCreatureMotion {
            position: predicted,
            yaw: heading.wrapping_add(world.turn_toward(
                heading,
                desired_heading,
                config.turn_limit,
            )) & 0x07ff,
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

fn body_candidate_is_valid<P: GroundPosition, W: GroundMotionWorld<P>>(
    world: &W,
    position: P,
    heading: i16,
    config: GroundCreatureMotionConfig,
) -> bool {
    // sub_102D0(..., 1) checks each forward extent probe before the caller's
    // slope test. Retry candidates are checked even inside the original tile.
    let mut probe = position;
    let mut advance = 0;
    while advance <= i32::from(config.half_width) {
        if world.material_mask(probe) & !config.terrain_mask != 0
            || world.clearance_blocked(
                probe,
                i32::from(config.half_height) + i32::from(config.ground_clearance),
            )
        {
            return false;
        }
        advance += 256;
        if advance <= i32::from(config.half_width) {
            world.move_entity(&mut probe, heading, 0, 256);
        }
    }
    tile_slope_metric(world, position) < config.maximum_tile_slope
}

pub fn relax_to_terrain<P, W>(
    position: &mut P,
    world: &W,
    ground_clearance: i16,
    vertical_relaxation: i16,
) where
    P: GroundPosition,
    W: GroundMotionWorld<P>,
{
    let ground = world.ground_height(*position);
    let mut z = position.z();
    if z > ground {
        z = z.wrapping_add(vertical_relaxation);
    }
    if z <= ground.wrapping_add(ground_clearance) {
        z = ground.wrapping_add(ground_clearance);
    }
    *position = position.with_z(z);
}

#[must_use]
pub fn candidate_is_valid<P, W>(
    world: &W,
    position: P,
    terrain_mask: u32,
    maximum_tile_slope: i32,
) -> bool
where
    P: GroundPosition,
    W: GroundMotionWorld<P>,
{
    world.material_mask(position) & !terrain_mask == 0
        && tile_slope_metric(world, position) < maximum_tile_slope
}

#[must_use]
pub const fn second_avoidance_heading(original: i16) -> i16 {
    let original = original.cast_unsigned();
    let low = original.wrapping_sub(85) & 0x00ff;
    let high = original.wrapping_sub(341) & 0x0700;
    (high | low).cast_signed()
}

#[must_use]
pub const fn opposite_heading(original: i16) -> i16 {
    let original = original.cast_unsigned();
    (((original.wrapping_add(0x0400)) & 0x0700) | (original & 0x00ff)).cast_signed()
}

#[must_use]
pub fn tile_slope_metric<P, W>(world: &W, position: P) -> i32
where
    P: GroundPosition,
    W: GroundMotionWorld<P>,
{
    let x = position.x() & 0xff00;
    let y = position.y() & 0xff00;
    let sample = |sample_x: u16, sample_y: u16| {
        i32::from(world.ground_height(P::from_components(sample_x, sample_y, 0)))
    };
    let point_1 = sample(x, y);
    let point_2 = sample(x.wrapping_add(0x0100), y);
    let point_3 = sample(x.wrapping_add(0x0100), y.wrapping_add(0x0100));
    let point_4 = sample(x, y.wrapping_add(0x0100));
    let diagonal_1 = (point_1 + point_4 - point_2 - point_3).abs();
    let diagonal_2 = (point_1 + point_2 - point_4 - point_3).abs();
    diagonal_1.max(diagonal_2) / 32
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::{
        GroundCreatureMotionConfig, GroundCreatureMoveResult, GroundMotionWorld, GroundPosition,
        opposite_heading, step_ground_creature_motion,
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct TestPosition {
        x: u16,
        y: u16,
        z: i16,
    }

    impl GroundPosition for TestPosition {
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

    struct RetryWorld {
        headings: RefCell<Vec<i16>>,
        reject_cross_tile_until: usize,
    }

    impl GroundMotionWorld<TestPosition> for RetryWorld {
        fn ground_height(&self, _position: TestPosition) -> i16 {
            0
        }

        fn material_mask(&self, _position: TestPosition) -> u32 {
            let attempt = self.headings.borrow().len();
            u32::from(attempt <= self.reject_cross_tile_until)
        }

        fn clearance_blocked(&self, _position: TestPosition, _required_height: i32) -> bool {
            false
        }

        fn move_entity(&self, position: &mut TestPosition, yaw: i16, _pitch: i16, _speed: i16) {
            self.headings.borrow_mut().push(yaw);
            position.x = position.x.wrapping_add(0x0100);
        }

        fn turn_toward(&self, _from: i16, _to: i16, maximum: i16) -> i16 {
            maximum
        }
    }

    fn config() -> GroundCreatureMotionConfig {
        GroundCreatureMotionConfig {
            half_width: 0,
            half_height: 0,
            ground_clearance: 128,
            descent: -128,
            turn_limit: 7,
            terrain_mask: 0,
            maximum_tile_slope: 1,
        }
    }

    #[test]
    fn first_invalid_cross_tile_retries_the_source_alternate_heading() {
        let world = RetryWorld {
            headings: RefCell::new(Vec::new()),
            reject_cross_tile_until: 1,
        };
        let motion = step_ground_creature_motion(
            TestPosition {
                x: 0x1000,
                y: 0x2000,
                z: 256,
            },
            100,
            600,
            256,
            &world,
            config(),
        );

        assert_eq!(&*world.headings.borrow(), &[100, 441]);
        assert_eq!(motion.result, GroundCreatureMoveResult::Alternate);
        assert!(motion.movement_blocked);
        assert!(!motion.killed);
        assert_eq!(motion.position.x, 0x1100);
        assert_eq!(motion.position.z, 128);
        assert_eq!(motion.yaw, 448);
    }

    #[test]
    fn four_invalid_cross_tile_candidates_preserve_origin_and_kill() {
        let world = RetryWorld {
            headings: RefCell::new(Vec::new()),
            reject_cross_tile_until: usize::MAX,
        };
        let origin = TestPosition {
            x: 0x1000,
            y: 0x2000,
            z: 256,
        };
        let motion = step_ground_creature_motion(origin, 100, 600, 256, &world, config());

        assert_eq!(world.headings.borrow().len(), 4);
        assert_eq!(motion.position, origin);
        assert_eq!(motion.yaw, opposite_heading(100));
        assert_eq!(motion.result, GroundCreatureMoveResult::Blocked);
        assert!(motion.movement_blocked);
        assert!(motion.killed);
    }

    struct BodyProbeWorld {
        retry_stays_in_tile: bool,
        materials: RefCell<Vec<TestPosition>>,
        clearances: RefCell<Vec<(TestPosition, i32)>>,
    }

    impl GroundMotionWorld<TestPosition> for BodyProbeWorld {
        fn ground_height(&self, _position: TestPosition) -> i16 {
            0
        }

        fn material_mask(&self, position: TestPosition) -> u32 {
            self.materials.borrow_mut().push(position);
            u32::from(self.retry_stays_in_tile)
        }

        fn clearance_blocked(&self, position: TestPosition, required_height: i32) -> bool {
            self.clearances
                .borrow_mut()
                .push((position, required_height));
            position.x == 0x1200
        }

        fn move_entity(&self, position: &mut TestPosition, yaw: i16, _pitch: i16, speed: i16) {
            let speed = if self.retry_stays_in_tile && yaw != 100 {
                -speed
            } else {
                speed
            };
            position.x = position.x.wrapping_add(speed as u16);
        }

        fn turn_toward(&self, _from: i16, _to: i16, _maximum: i16) -> i16 {
            0
        }
    }

    #[test]
    fn same_tile_retries_still_probe_after_first_cross_tile_rejection() {
        let world = BodyProbeWorld {
            retry_stays_in_tile: true,
            materials: RefCell::default(),
            clearances: RefCell::default(),
        };
        let origin = TestPosition {
            x: 0x10f0,
            y: 0x2000,
            z: 256,
        };
        let motion = step_ground_creature_motion(origin, 100, 100, 32, &world, config());
        assert_eq!(motion.result, GroundCreatureMoveResult::Blocked);
        assert_eq!(motion.position, origin);
        let probes = world.materials.borrow();
        assert_eq!(probes.len(), 4);
        assert_eq!(probes[0].x, 0x1110);
        assert!(probes[1..].iter().all(|probe| probe.x == 0x10d0));
    }

    #[test]
    fn forward_body_extent_checks_cave_clearance_before_accepting_motion() {
        let world = BodyProbeWorld {
            retry_stays_in_tile: false,
            materials: RefCell::default(),
            clearances: RefCell::default(),
        };
        let origin = TestPosition {
            x: 0x1000,
            y: 0x2000,
            z: 256,
        };
        let mut config = config();
        config.half_width = 256;
        config.half_height = 74;
        let motion = step_ground_creature_motion(origin, 100, 100, 256, &world, config);
        assert_eq!(motion.result, GroundCreatureMoveResult::Blocked);
        assert_eq!(motion.position, origin);
        let probes = world.clearances.borrow();
        assert_eq!(probes.len(), 8);
        for pair in probes.chunks_exact(2) {
            assert_eq!((pair[0].0.x, pair[1].0.x), (0x1100, 0x1200));
            assert_eq!((pair[0].1, pair[1].1), (202, 202));
        }
    }
}
