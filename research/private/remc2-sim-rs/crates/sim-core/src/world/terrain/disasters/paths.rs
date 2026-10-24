//! Authored path marking over the same topology used by collision queries.

use crate::native_world::{SessionTerrain, rebuild_building_topology};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Segment {
    pub(crate) x: u16,
    pub(crate) y: u16,
    pub(crate) dx: i16,
    pub(crate) dy: i16,
    pub(crate) length: i32,
}

fn shortest(from: u16, to: u16) -> i32 {
    let delta = i32::from(to) - i32::from(from);
    if delta > 128 {
        delta - 256
    } else if delta < -128 {
        delta + 256
    } else {
        delta
    }
}

/// sub_48690 creates both controllers even when one segment has zero length.
pub(crate) fn segments(x: u16, y: u16, end_x: u16, end_y: u16) -> [Segment; 2] {
    let dx = shortest(x, end_x);
    let dy = shortest(y, end_y);
    let sx = dx.signum() as i16;
    let sy = dy.signum() as i16;
    let diagonal = dx.abs().min(dy.abs());
    let (tail_x, tail_y) = if dx.abs() <= dy.abs() {
        (0, sy)
    } else {
        (sx, 0)
    };
    [
        Segment {
            x,
            y,
            dx: sx,
            dy: sy,
            length: diagonal,
        },
        Segment {
            x: x.wrapping_add((diagonal * i32::from(sx)) as u16),
            y: y.wrapping_add((diagonal * i32::from(sy)) as u16),
            dx: tail_x,
            dy: tail_y,
            length: (dx.abs() - dy.abs()).abs(),
        },
    ]
}

/// ApplyPointToPath_343F0. Protected cells keep their source angle80 bit;
/// the protected topology writer then decides whether to rebuild that cell.
pub(crate) fn apply(
    terrain: &mut SessionTerrain,
    mut x: u8,
    mut y: u8,
    dx: i16,
    dy: i16,
    length: u16,
) {
    let (planes, map_type, rng, sequence) = terrain.disaster_parts();
    for _ in 0..length {
        let tile = usize::from(x) + 256 * usize::from(y);
        planes.angles[tile] = (planes.angles[tile] & 0xf0) | 1;
        rebuild_building_topology(planes, x, y, x, y, map_type, rng);
        x = x.wrapping_add(dx as u8);
        y = y.wrapping_add(dy as u8);
    }
    if length != 0 {
        *sequence = sequence.wrapping_add(1);
    }
}
