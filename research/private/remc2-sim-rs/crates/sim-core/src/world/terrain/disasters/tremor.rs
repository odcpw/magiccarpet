//! Tremor's alternating height field and phased burst envelope. Source
//! deliberately leaves materials/shading unchanged during this oscillation.

use super::search_band;
use crate::{Position, native_world::SessionTerrain};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Controller {
    pub(crate) position: Position,
    pub(crate) max_life: i32,
    pub(crate) life: i32,
    pub(crate) damage: u16,
    pub(crate) phase: u8,
    pub(crate) eighth_life: i16,
    pub(crate) clock: i32,
    pub(crate) rng: u16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Visit {
    pub(crate) hide: bool,
    pub(crate) terrain_mutated: bool,
    pub(crate) damage_radius: Option<i16>,
}

fn oscillate(terrain: &mut SessionTerrain, x: u8, y: u8, radius: i32, life: i32) {
    let (planes, _, _, _) = terrain.disaster_parts();
    for (dx, dy) in search_band(0, radius) {
        let index =
            usize::from(x.wrapping_add_signed(dx)) + 256 * usize::from(y.wrapping_add_signed(dy));
        let height = i32::from(planes.ground_heights[index]) + if life & 1 != 0 { 1 } else { -1 };
        planes.ground_heights[index] = height.clamp(0, 255) as u8;
    }
}

/// sub_3A2D0, through its geometry and immediate damage boundary. Caller
/// applies ordinary 10C80 damage/XP15 and sound10 before postdecrement.
pub(crate) fn prepare(
    terrain: &mut SessionTerrain,
    source: &mut Controller,
) -> Result<Visit, String> {
    if source.life < 0 {
        return Ok(Visit {
            hide: true,
            ..Visit::default()
        });
    }
    if source.phase > 3 {
        return Ok(Visit::default());
    }
    if source.phase == 0 {
        if source.max_life == 0 {
            return Err("Tremor maximum lifetime is zero".to_owned());
        }
        source.eighth_life = (source.max_life >> 3) as i16;
        source.clock = 0;
        source.phase = 1;
        source.damage =
            (4_u32.wrapping_mul((i32::from(source.damage) / source.max_life) as u32)) as u16;
    }
    let eighth = i32::from(source.eighth_life);
    let mut radius;
    if source.max_life.wrapping_sub(3 * eighth) >= source.life {
        if source.max_life.wrapping_sub(5 * eighth) > source.life {
            source.clock = source.clock.wrapping_sub(1);
            radius = source.clock;
        } else {
            radius = 3 * eighth;
            source.rng = source.rng.wrapping_mul(9377).wrapping_add(9439);
            if source.rng % 5 == 0 {
                source.phase = source.phase.wrapping_add(2);
            }
        }
    } else {
        source.clock = source.clock.wrapping_add(1);
        radius = source.clock;
    }
    radius = radius.max(0).min(3 * eighth).clamp(0, 15);
    let burst = source.phase > 1;
    if burst {
        source.phase -= 1;
    }
    if radius == 0 {
        return Ok(Visit::default());
    }
    let x = (source.position.x.wrapping_add(128) >> 8) as u8;
    let y = (source.position.y.wrapping_add(128) >> 8) as u8;
    oscillate(terrain, x, y, radius, source.life);
    if burst {
        oscillate(terrain, x, y, radius >> 1, source.life);
    }
    let (_, _, _, sequence) = terrain.disaster_parts();
    *sequence = sequence.wrapping_add(1);
    Ok(Visit {
        hide: false,
        terrain_mutated: true,
        damage_radius: (source.life & 3 == 0).then_some((radius as i16) << 8),
    })
}

pub(crate) fn finish(source: &mut Controller) {
    if source.life >= 0 {
        source.life = source.life.wrapping_sub(1);
    }
}
