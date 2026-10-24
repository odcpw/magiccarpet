//! Moving terrain producers. Allocation stays in the MC2 arena adapter;
//! these rules retain the exact child pose at its production boundary.

use crate::{Position, move_entity, native_world::SessionTerrain};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Controller {
    pub(crate) position: Position,
    pub(crate) life: i32,
    pub(crate) water_clock: i32,
    pub(crate) yaw: i16,
    pub(crate) rng: u16,
    pub(crate) speed: i16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Visit {
    pub(crate) hide: bool,
    pub(crate) child_position: Option<Position>,
}

fn empty_surface(terrain: &SessionTerrain, position: Position) -> bool {
    let index = usize::from(position.x >> 8) + 256 * usize::from(position.y >> 8);
    terrain.planes().angles[index] & 15 == 0
}

/// sub_32530: shore counter, predecrement lifetime, yaw draw, move, ring.
pub(crate) fn earthquake(terrain: &SessionTerrain, source: &mut Controller) -> Visit {
    if empty_surface(terrain, source.position) {
        source.water_clock = source.water_clock.wrapping_add(1);
    } else if source.water_clock > 0 {
        source.water_clock = source.water_clock.wrapping_sub(1);
    }
    source.life = source.life.wrapping_sub(1);
    if source.life < -1 || source.water_clock > 8 {
        return Visit {
            hide: true,
            child_position: None,
        };
    }
    source.rng = source.rng.wrapping_mul(9377).wrapping_add(9439);
    source.yaw = ((i32::from(source.rng % 91) + i32::from(source.yaw) - 45) & 2047) as i16;
    move_entity(&mut source.position, source.yaw, 0, 256);
    Visit {
        hide: false,
        child_position: Some(source.position),
    }
}

/// sub_344A0: child at the old pose before the producer moves on. The
/// adapter copies FOV, owner and byte46 lifetime into the new lowering ring.
pub(crate) fn crater_trail(terrain: &SessionTerrain, source: &mut Controller) -> Visit {
    let old_life = source.life;
    source.life = source.life.wrapping_sub(1);
    if old_life < 0 || empty_surface(terrain, source.position) {
        return Visit {
            hide: true,
            child_position: None,
        };
    }
    let child_position = Some(source.position);
    Visit {
        hide: false,
        child_position,
    }
}

/// The source allocates the child before moving the parent. Keep this at a
/// separate boundary so allocator replacement can change that very row.
pub(crate) fn finish_crater_trail(source: &mut Controller) {
    move_entity(&mut source.position, source.yaw, 0, source.speed);
}

/// sub_352C0, before its terrain/damage/audio boundary. The caller applies
/// the generated relative write, then 10C80/sound if it did not terminate.
pub(crate) fn prepare_rising_trail(
    terrain: &SessionTerrain,
    source: &mut Controller,
) -> Option<i16> {
    let old_life = source.life;
    source.life = source.life.wrapping_sub(1);
    if old_life < 0 || empty_surface(terrain, source.position) {
        return None;
    }
    source.rng = source.rng.wrapping_mul(9377).wrapping_add(9439);
    Some((source.rng % 15 + 10) as i16)
}

pub(crate) fn finish_rising_trail(source: &mut Controller) {
    move_entity(&mut source.position, source.yaw, 0, source.speed);
}
