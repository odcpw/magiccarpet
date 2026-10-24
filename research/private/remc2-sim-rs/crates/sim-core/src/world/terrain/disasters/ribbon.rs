//! Linked raised ribbons: wrapped segment subdivision and live geometry.
//! Source callbacks traverse packed tile words, whose X overflow carries Y.

use crate::native_world::{MapType, SessionTerrain, TerrainPlanes};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Direction {
    North,
    South,
    East,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Segment {
    pub(crate) x: u16,
    pub(crate) y: u16,
    pub(crate) direction: Direction,
    pub(crate) length: i16,
    pub(crate) end_x: u16,
    pub(crate) end_y: u16,
}

fn shortest(start: u16, end: u16) -> i32 {
    let distance = i32::from(end) - i32::from(start);
    if distance > 128 {
        distance - 256
    } else if distance < -128 {
        distance + 256
    } else {
        distance
    }
}

/// sub_48400 creates alternating vertical/horizontal model27 descendants.
/// The first subdivision owns integer remainders; allocation order is part
/// of the ordinary shared source arena's subsequent traversal order.
pub(crate) fn segments(mut x: u16, mut y: u16, mut end_x: u16, mut end_y: u16) -> Vec<Segment> {
    let mut dx = shortest(x, end_x);
    let mut dy = shortest(y, end_y);
    if dx == 0 && dy == 0 {
        return Vec::new();
    }
    if dx < 0 {
        dx = -dx;
        dy = -dy;
        std::mem::swap(&mut x, &mut end_x);
        std::mem::swap(&mut y, &mut end_y);
    }
    let count = if dx <= dy.abs() {
        (dy / 10).abs() + 1
    } else {
        dx / 10 + 1
    };
    let step_x = dx / count;
    let step_y = dy / count;
    let mut remainder_x = dx - count * step_x;
    let mut remainder_y = dy - count * step_y;
    let mut output = Vec::new();
    for _ in 0..count {
        let sx = step_x + remainder_x;
        let sy = step_y + remainder_y;
        let vertical = |x, y| Segment {
            x,
            y,
            direction: if step_y >= 0 {
                Direction::South
            } else {
                Direction::North
            },
            length: sy.abs() as i16,
            end_x,
            end_y,
        };
        if dx <= dy.abs() {
            output.push(vertical(x, y));
            y = y.wrapping_add(sy as u16);
            output.push(Segment {
                x,
                y,
                direction: Direction::East,
                length: sx as i16,
                end_x,
                end_y,
            });
            x = x.wrapping_add(sx as u16);
        } else {
            output.push(Segment {
                x,
                y,
                direction: Direction::East,
                length: sx as i16,
                end_x,
                end_y,
            });
            x = x.wrapping_add(sx as u16);
            output.push(vertical(x, y));
            y = y.wrapping_add(sy as u16);
        }
        remainder_x = 0;
        remainder_y = 0;
    }
    output
}

fn index(x: u8, y: u8) -> usize {
    usize::from(x) + 256 * usize::from(y)
}

/// sub_33F70 deliberately tests the center again after the west neighbor,
/// rather than substituting the east neighbor for the source's expression.
fn should_raise(planes: &TerrainPlanes, tile: u16) -> bool {
    let x = tile as u8;
    let y = (tile >> 8) as u8;
    let center = usize::from(tile);
    let limit = u16::from(planes.ground_heights[center]) + 30;
    planes.terrain_types[index(x.wrapping_sub(1), y)] != 8
        || u16::from(planes.ground_heights[index(x.wrapping_sub(1), y)]) > limit
        || u16::from(planes.ground_heights[index(x, y.wrapping_add(1))]) > limit
        || u16::from(planes.ground_heights[index(x, y.wrapping_sub(1))]) > limit
}

/// sub_46180 writes four material cells, then shades/normalizes a 3x3 area.
fn material(planes: &mut TerrainPlanes, tile: u16, map_type: MapType) {
    let x = tile as u8;
    let y = (tile >> 8) as u8;
    for (x, y) in [
        (x, y),
        (x.wrapping_sub(1), y),
        (x.wrapping_sub(1), y.wrapping_sub(1)),
        (x, y.wrapping_sub(1)),
    ] {
        planes.terrain_types[index(x, y)] = 8;
    }
    for dy in -1_i8..=1 {
        for dx in -1_i8..=1 {
            let x = x.wrapping_add_signed(dx);
            let y = y.wrapping_add_signed(dy);
            let tile = index(x, y);
            let raw = planes.ground_heights[index(x.wrapping_sub(1), y.wrapping_sub(1))]
                .wrapping_sub(planes.ground_heights[index(x.wrapping_add(1), y.wrapping_add(1))])
                .wrapping_add(32) as i8;
            let shade = if raw >= 28 {
                if raw > 40 { (raw & 7) + 40 } else { raw }
            } else {
                (raw & 3) + 28
            };
            planes.shading[tile] = if map_type == MapType::Day {
                shade as u8
            } else {
                64_i8.wrapping_sub(shade) as u8
            };
            if map_type == MapType::Cave
                && planes.secondary_heights[tile] <= planes.ground_heights[tile]
            {
                planes.secondary_heights[tile] = planes.ground_heights[tile].wrapping_sub(1);
                planes.angles[tile] |= 8;
            } else {
                planes.angles[tile] &= 0xf7;
            }
        }
    }
}

fn raise(planes: &mut TerrainPlanes, tile: u16, map_type: MapType) {
    let index = usize::from(tile);
    if planes.terrain_types[index] != 8 || should_raise(planes, tile) {
        planes.ground_heights[index] = planes.ground_heights[index].wrapping_add(48);
    }
    material(planes, tile, map_type);
}

/// sub_34000/sub_34110/sub_34210. These callbacks retire after one visit;
/// the scheduler performs draw disable and the next-pass arena reclamation.
pub(crate) fn apply(
    terrain: &mut SessionTerrain,
    x: u8,
    y: u8,
    width: i16,
    length: i16,
    direction: Direction,
) {
    let (planes, map_type, _, sequence) = terrain.disaster_parts();
    match direction {
        Direction::North | Direction::South => {
            let mut x = x;
            let mut y = y;
            if direction == Direction::South {
                if x & 1 != 0 {
                    x = x.wrapping_add(1);
                }
                x = x.wrapping_sub(width as u8).wrapping_add(1);
            } else {
                y = y.wrapping_add(2);
            }
            for _ in 0..width.wrapping_add(length) as u16 {
                planes.angles[index(x.wrapping_sub(1), y)] |= 0x80;
                let mut tile = index(x, y) as u16;
                for _ in 0..width as u16 {
                    raise(planes, tile, map_type);
                    tile = tile.wrapping_add(1);
                }
                planes.angles[usize::from(tile)] |= 0x80;
                y = if direction == Direction::South {
                    y.wrapping_add(1)
                } else {
                    y.wrapping_sub(1)
                };
            }
        }
        Direction::East => {
            let x = x.wrapping_add((x.wrapping_add(y) & 1 != 0) as u8);
            let mut tile = index(x, y.wrapping_sub(1)) as u16;
            for _ in 0..length as u16 {
                planes.angles[usize::from(tile)] |= 0x80;
                tile = tile.wrapping_add(1);
            }
            let mut y = y;
            for _ in 0..width as u16 {
                let mut tile = index(x, y) as u16;
                for _ in 0..length as u16 {
                    raise(planes, tile, map_type);
                    tile = tile.wrapping_add(1);
                }
                y = y.wrapping_add(1);
            }
            let mut tile = index(x, y) as u16;
            for _ in 0..length as u16 {
                planes.angles[usize::from(tile)] |= 0x80;
                tile = tile.wrapping_add(1);
            }
        }
    }
    *sequence = sequence.wrapping_add(1);
}
