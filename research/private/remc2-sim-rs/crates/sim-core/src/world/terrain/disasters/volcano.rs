//! Volcano deformation in source order. Controller fields are borrowed from
//! one arena row; the adapter publishes them back before other source visits.

use crate::{
    Position, SIN_16_16, legacy_sqrt,
    legacy_terrain::cave_perimeter_extrema,
    native_world::{MapType, SessionTerrain, cave_features::write_ground},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Controller {
    pub(crate) position: Position,
    pub(crate) radius_parameter: i32,
    pub(crate) pitch: i16,
    pub(crate) amplitude: i16,
    pub(crate) phase: u8,
    pub(crate) life: i32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Visit {
    pub(crate) hide: bool,
    pub(crate) terrain_mutated: bool,
    pub(crate) damage: bool,
    pub(crate) rumble: bool,
    pub(crate) eruption: bool,
}

fn index(x: u8, y: u8) -> usize {
    usize::from(x) + 256 * usize::from(y)
}

fn center_cells(terrain: &mut SessionTerrain, x: u8, y: u8, height: i32) {
    let (planes, map_type, _, _) = terrain.disaster_parts();
    for row in 0..2_u8 {
        for column in 0..2_u8 {
            let tile = index(
                x.wrapping_sub(1).wrapping_add(column),
                y.wrapping_sub(1).wrapping_add(row),
            );
            planes.ground_heights[tile] = height as u8;
            planes.shading[tile] = if map_type == MapType::Day { 63 } else { 1 };
        }
    }
}

/// Source sub_31940: minimum *perimeter* base, cosine profile, requested
/// absolute floor writes, independent cave ceiling interpolation and cap.
pub(crate) fn step(
    terrain: &mut SessionTerrain,
    controller: &mut Controller,
    alternate: bool,
) -> Visit {
    let x = ((u32::from(controller.position.x) + 128) >> 8) as u8;
    let y = ((u32::from(controller.position.y) + 128) >> 8) as u8;
    if controller.phase == 0 {
        let radius = controller.radius_parameter | 1;
        controller.pitch = (radius as i16).wrapping_shl(8);
        let (planes, _, _, _) = terrain.disaster_parts();
        let base = cave_perimeter_extrema(
            &planes.ground_heights,
            x.wrapping_sub(radius as u8),
            y.wrapping_sub(radius as u8),
            radius.wrapping_mul(2) as u16,
            radius.wrapping_mul(2) as u16,
        )
        .0;
        controller.position.z = base as i16;
        controller.amplitude = radius.wrapping_mul(2).wrapping_add(100) as i16;
        if base + i32::from(controller.amplitude) > 255 {
            controller.amplitude = (255 - base) as i16;
        }
        controller.phase = 1;
    } else if controller.phase > 1 {
        if controller.phase != 2 {
            return Visit::default();
        }
        let cap = i32::from(controller.position.z) + i32::from(controller.amplitude) - 24;
        let half = controller.pitch >> 8;
        let side = controller.pitch >> 7;
        let (planes, _, _, _) = terrain.disaster_parts();
        for row in 0..side.max(0) {
            for column in 0..side.max(0) {
                let tile = index(
                    x.wrapping_sub(half as u8).wrapping_add(column as u8),
                    y.wrapping_sub(half as u8).wrapping_add(row as u8),
                );
                if i32::from(planes.ground_heights[tile]) > cap {
                    planes.ground_heights[tile] = cap as u8;
                }
            }
        }
        center_cells(terrain, x, y, cap - 16);
        let (_, _, _, sequence) = terrain.disaster_parts();
        *sequence = sequence.wrapping_add(1);
        return Visit {
            hide: true,
            terrain_mutated: true,
            ..Visit::default()
        };
    }
    controller.life = controller.life.wrapping_sub(1);
    if controller.life <= 0 {
        controller.phase = 2;
        return Visit::default();
    }
    let pitch = i32::from(controller.pitch);
    let half = (controller.pitch as u16 >> 8) as u8;
    let side = pitch >> 7;
    let inner = pitch - ((((pitch >> 8) - 7) >> 1 << 8) + 512);
    for row in 0..side.max(0) {
        let tile_y = y.wrapping_sub(half).wrapping_add(row as u8);
        for column in 0..side.max(0) {
            let tile_x = x.wrapping_sub(half).wrapping_add(column as u8);
            let dx = i32::from((u16::from(tile_x) << 8).wrapping_sub(controller.position.x) as i16);
            let dy = i32::from((u16::from(tile_y) << 8).wrapping_sub(controller.position.y) as i16);
            let distance = legacy_sqrt((dx * dx) as u32 + (dy * dy) as u32) as i32;
            let tile = index(tile_x, tile_y);
            if distance < pitch {
                let factor = (65_536 + SIN_16_16[512 + ((distance << 10) / pitch) as usize]) >> 1;
                let desired = (i32::from(controller.amplitude) * factor >> 16)
                    + i32::from(controller.position.z);
                let old = i32::from(terrain.planes().ground_heights[tile]);
                let height = if desired > old {
                    (desired - old) / controller.life + old
                } else {
                    old
                };
                write_ground(terrain, tile_x, tile_y, height, distance <= inner, true);
                let (planes, map_type, _, _) = terrain.disaster_parts();
                if map_type == MapType::Cave {
                    let desired = (height + 64).min(254);
                    let old = i32::from(planes.secondary_heights[tile]);
                    if desired > old {
                        planes.secondary_heights[tile] =
                            (old - (old - desired) / controller.life) as u8;
                    }
                }
            }
            let (planes, map_type, _, _) = terrain.disaster_parts();
            if map_type == MapType::Cave {
                if planes.secondary_heights[tile] > planes.ground_heights[tile] {
                    planes.angles[tile] &= 0xf7;
                } else {
                    planes.angles[tile] |= 8;
                }
            }
        }
    }
    let (_, _, _, sequence) = terrain.disaster_parts();
    *sequence = sequence.wrapping_add(1);
    Visit {
        hide: false,
        terrain_mutated: true,
        damage: !alternate,
        rumble: true,
        eruption: controller.life == 3,
    }
}

/// The final center is written after area damage/experience and rumble, and
/// before the eruption child samples its actual terrain altitude.
pub(crate) fn finish_eruption(terrain: &mut SessionTerrain, controller: &Controller) {
    let x = ((u32::from(controller.position.x) + 128) >> 8) as u8;
    let y = ((u32::from(controller.position.y) + 128) >> 8) as u8;
    center_cells(
        terrain,
        x,
        y,
        i32::from(controller.position.z) + i32::from(controller.amplitude) - 40,
    );
}
