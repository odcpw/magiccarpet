//! Shared remc2 class-2 ballistic-scenery behavior.
//!
//! The behavior is selected by the compiled class/action registry and mutates
//! the lossless imported source row. Allocation, map topology, the common
//! byte-0x3E epilogue, and following-pass reclaim remain owned by the campaign
//! source scheduler and arena.

use super::*;

const SKIP_ONCE_CONTROL_BIT: u32 = 0x0000_0800;
const VERTICAL_ACCELERATION: i16 = 24;
const MIN_VERTICAL_SPEED: i16 = -192;
const MAX_VERTICAL_SPEED: i16 = 192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BallisticScenerySpawn {
    Model13Smoke,
    Model5WaterSplash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct BallisticSceneryStep {
    pub(super) spawn: Option<BallisticScenerySpawn>,
    pub(super) hide: bool,
}

fn tile_slope(terrain: &SessionTerrain, position: Position) -> i32 {
    let x = position.x.to_be_bytes()[0];
    let y = position.y.to_be_bytes()[0];
    let heights = terrain.ground_heights();
    let p1 = i32::from(heights[terrain_index(x, y)]);
    let p2 = i32::from(heights[terrain_index(x.wrapping_add(1), y)]);
    let p3 = i32::from(heights[terrain_index(x.wrapping_add(1), y.wrapping_add(1))]);
    let p4 = i32::from(heights[terrain_index(x, y.wrapping_add(1))]);
    (p1 + p4 - p2 - p3).abs().max((p1 + p2 - p4 - p3).abs())
}

fn avoid_steep_tile(source: &EntitySnapshot, terrain: &SessionTerrain, position: &mut Position) {
    if tile_slope(terrain, *position) <= 20 {
        return;
    }

    let mut direction = source.yaw;
    let mut lowest_height = u32::MAX;
    let mut selected = *position;
    for _ in 0..8 {
        let mut candidate = *position;
        crate::move_entity(&mut candidate, direction, 0, 64);
        let height = u32::from(terrain.ground_height(candidate).cast_unsigned());
        if height < lowest_height {
            lowest_height = height;
            selected = candidate;
        }
        direction = direction.wrapping_add(256) & 0x07ff;
    }
    *position = selected;
}

/// Execute `sub_652C0` without taking ownership of scheduler or arena work.
pub(super) fn step_ballistic_scenery(
    source: &mut EntitySnapshot,
    terrain: &SessionTerrain,
) -> BallisticSceneryStep {
    if source.control_flags & SKIP_ONCE_CONTROL_BIT != 0 {
        source.set_control_flags(source.control_flags & !SKIP_ONCE_CONTROL_BIT);
        return BallisticSceneryStep {
            spawn: None,
            hide: false,
        };
    }

    let mut position = source.position;
    let initial_ground = terrain.ground_height(position);
    if position.z > initial_ground {
        crate::move_entity(&mut position, source.yaw, 0, source.actual_speed);
    } else {
        source.set_actual_speed(0);
        avoid_steep_tile(source, terrain, &mut position);
    }
    source.set_position(position);
    if source.actual_speed != 0 {
        source.set_actual_speed(source.actual_speed.wrapping_sub(1));
    }

    let vertical_speed = source.source_word_2c();
    position.z = position.z.wrapping_add(vertical_speed);
    source.set_source_word_2c(
        vertical_speed
            .wrapping_sub(VERTICAL_ACCELERATION)
            .clamp(MIN_VERTICAL_SPEED, MAX_VERTICAL_SPEED),
    );
    let ground = terrain.ground_height(position);
    if position.z <= ground {
        position.z = ground;
    }
    source.set_position(position);

    let damage_owner = source.source_upkeep_word_62();
    if damage_owner != 0 {
        let damage = source.source_upkeep_payload();
        if position.z <= ground {
            let magnitude = (damage >> 2).clamp(2, 192);
            let first = level2_next_rng16(source.entity_rng);
            let bounce = i32::from(first) % magnitude + magnitude;
            source.set_source_word_2c(bounce as i16);

            let second = level2_next_rng16(first);
            let half_bounce =
                u16::try_from(bounce >> 1).expect("positive ballistic bounce half-width fits u16");
            source.set_actual_speed(
                i16::try_from(second % half_bounce + 1).expect("ballistic rebound speed fits i16"),
            );

            let third = level2_next_rng16(second);
            source.set_yaw((third & 0x07ff) as i16);
            source.set_entity_rng(third);
            position.z = position.z.wrapping_add(bounce as i16);
            source.set_position(position);
        }
        source.clear_source_upkeep_owner();
        source.set_life(source.life.wrapping_sub(damage));
    }

    let spawn = if source.life < 0 {
        Some(BallisticScenerySpawn::Model13Smoke)
    } else if source.position.z <= ground
        && terrain.material_mask(source.position) == crate::WATER_AMBIENCE_MASK
    {
        Some(BallisticScenerySpawn::Model5WaterSplash)
    } else {
        None
    };
    BallisticSceneryStep {
        spawn,
        hide: spawn.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> Mc2Simulation {
        let package = std::env::var_os("MC_LEVEL2_PACKAGE_V5").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/l2-debug3.fJ3PaM/level2")
            },
            PathBuf::from,
        );
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component")
            .level2_reset_session()
            .expect("qualified source session")
    }

    fn ballistic_row(session: &Mc2Simulation) -> (EntitySnapshot, SessionTerrain) {
        let mut source = session.session.state.live.economy.entities[100].clone();
        source.active = true;
        source.class = 2;
        source.model = 8;
        source.action = 21;
        source.record[2] = 1;
        source.record[67] = 2;
        source.record[68] = 8;
        source.record[73] = 21;
        source.set_control_flags(0);
        source.set_actual_speed(0);
        source.set_source_word_2c(-128);
        source.set_life(1_000);
        source.set_entity_rng(0x1234);
        let terrain = session.session.state.live.terrain.clone();
        let mut position = source.position;
        position.z = terrain.ground_height(position);
        source.set_position(position);
        (source, terrain)
    }

    #[test]
    fn cf14_ballistic_ground_damage_uses_exact_three_local_draws() {
        let session = fixture();
        let (mut source, terrain) = ballistic_row(&session);
        source.record[98..102].copy_from_slice(&400_i32.to_le_bytes());
        source.record[102..104].copy_from_slice(&7_u16.to_le_bytes());

        let first = level2_next_rng16(0x1234);
        let bounce = i32::from(first) % 100 + 100;
        let second = level2_next_rng16(first);
        let expected_speed = second % u16::try_from(bounce >> 1).unwrap() + 1;
        let third = level2_next_rng16(second);

        let step = step_ballistic_scenery(&mut source, &terrain);
        assert_eq!(step.spawn, None);
        assert!(!step.hide);
        assert_eq!(source.life, 600);
        assert_eq!(source.source_upkeep_word_62(), 0);
        assert_eq!(source.entity_rng, third);
        assert_eq!(source.source_word_2c(), bounce as i16);
        assert_eq!(source.actual_speed, expected_speed as i16);
        assert_eq!(source.yaw, (third & 0x07ff) as i16);
        assert_eq!(
            source.position.z,
            terrain
                .ground_height(source.position)
                .wrapping_add(bounce as i16)
        );
    }

    #[test]
    fn cf14_ballistic_airborne_lethal_damage_draws_no_local_rng_and_prefers_smoke() {
        let session = fixture();
        let (mut source, terrain) = ballistic_row(&session);
        source.set_life(50);
        source.set_source_word_2c(0);
        source.set_position(Position {
            z: source.position.z.wrapping_add(1_000),
            ..source.position
        });
        source.record[98..102].copy_from_slice(&100_i32.to_le_bytes());
        source.record[102..104].copy_from_slice(&7_u16.to_le_bytes());

        let step = step_ballistic_scenery(&mut source, &terrain);
        assert_eq!(step.spawn, Some(BallisticScenerySpawn::Model13Smoke));
        assert!(step.hide);
        assert_eq!(source.life, -50);
        assert_eq!(source.entity_rng, 0x1234);
        assert_eq!(source.source_upkeep_word_62(), 0);
    }

    #[test]
    fn cf14_ballistic_skip_once_and_water_contact_preserve_source_priority() {
        let session = fixture();
        let (mut skipped, terrain) = ballistic_row(&session);
        skipped.set_control_flags(SKIP_ONCE_CONTROL_BIT | 0x20);
        skipped.set_actual_speed(17);
        skipped.set_source_word_2c(91);
        let before = skipped.clone();

        let step = step_ballistic_scenery(&mut skipped, &terrain);
        assert_eq!(step.spawn, None);
        assert!(!step.hide);
        assert_eq!(skipped.position, before.position);
        assert_eq!(skipped.actual_speed, before.actual_speed);
        assert_eq!(skipped.source_word_2c(), before.source_word_2c());
        assert_eq!(skipped.life, before.life);
        assert_eq!(skipped.entity_rng, before.entity_rng);
        assert_eq!(skipped.control_flags, 0x20);

        let water_position = (0_u16..=u16::from(u8::MAX))
            .find_map(|tile_x| {
                (0_u16..=u16::from(u8::MAX)).find_map(|tile_y| {
                    let position = Position {
                        x: (tile_x << 8).wrapping_add(128),
                        y: (tile_y << 8).wrapping_add(128),
                        z: 0,
                    };
                    (terrain.material_mask(position) == crate::WATER_AMBIENCE_MASK)
                        .then_some(position)
                })
            })
            .expect("qualified outdoor terrain contains a water tile");
        let (mut water, _) = ballistic_row(&session);
        water.set_position(Position {
            z: terrain.ground_height(water_position),
            ..water_position
        });
        water.set_source_word_2c(0);
        let rng_before = water.entity_rng;

        let step = step_ballistic_scenery(&mut water, &terrain);
        assert_eq!(step.spawn, Some(BallisticScenerySpawn::Model5WaterSplash));
        assert!(step.hide);
        assert_eq!(water.entity_rng, rng_before);
    }
}
