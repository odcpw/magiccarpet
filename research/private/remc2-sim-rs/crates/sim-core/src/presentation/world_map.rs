//! Original split world-map geometry and indexed terrain writer.
//!
//! DrawGameFrame_2BE30 replaces the normal world viewport while menu 6/7/8
//! is open. Its VGA1 map is 191x200 at scale 408, followed by a real 128x200
//! DrawWorld viewport at x192. The caller supplies that viewport; stretching
//! an ordinary world frame is not the source projection.

use crate::{Position, SIN_16_16};

pub const FRAME_WIDTH: usize = 320;
pub const FRAME_HEIGHT: usize = 200;
pub const MAP_WIDTH: usize = 191;
pub const WORLD_X: usize = 192;
pub const WORLD_WIDTH: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldMapView {
    pub x: u16,
    pub y: u16,
    pub yaw: i16,
}

impl From<sim_session::CameraPose> for WorldMapView {
    fn from(camera: sim_session::CameraPose) -> Self {
        Self {
            x: camera.x,
            y: camera.y,
            yaw: camera.yaw,
        }
    }
}

impl crate::native_world::Mc2Simulation {
    /// Immutable map and world passes consume the same retained source view.
    #[must_use]
    pub fn world_map_camera(&self) -> sim_session::CameraPose {
        self.source_camera_pose()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MapGeometry {
    pub width: i32,
    pub height: i32,
    pub scaling: i32,
}

impl MapGeometry {
    pub const MINIMAP: Self = Self {
        width: 128,
        height: 128,
        scaling: 256,
    };
    pub const FULL_MAP: Self = Self {
        width: MAP_WIDTH as i32,
        height: FRAME_HEIGHT as i32,
        scaling: 408,
    };

    /// DrawMinimapEntities' wrapped signed world delta. Entity and objective
    /// producers share this transform; the client never rescales old pixels.
    #[must_use]
    pub fn project(self, view: WorldMapView, target: Position) -> (i16, i16) {
        let inverse_scale = 0x1_0000 / self.scaling;
        let angle = usize::from(view.yaw.cast_unsigned() & 0x07ff);
        let axis_down = (-i64::from(inverse_scale) * i64::from(SIN_16_16[angle])) >> 16;
        let axis_right = (i64::from(inverse_scale) * i64::from(SIN_16_16[angle + 512])) >> 16;
        let dx = i64::from(target.x.wrapping_sub(view.x).cast_signed());
        let dy = i64::from(target.y.wrapping_sub(view.y).cast_signed());
        let x = i64::from(self.width / 2) + ((dx * axis_right - dy * axis_down) >> 16);
        let y = i64::from(self.height / 2) + ((axis_right * dy + axis_down * dx) >> 16);
        (x as i16, y as i16)
    }
}

/// sub_63670_draw_minimap_a's opaque full rectangle (a10=1). The selected
/// environment supplies its own TABLES bank and live terrain planes. Cave
/// cells marked with angle bit 8 are index zero, even when they have terrain.
pub fn render_full_map_terrain(
    view: WorldMapView,
    terrain_types: &[u8],
    shading: &[u8],
    angles: &[u8],
    cave: bool,
    tables: &[u8],
) -> Result<Vec<u8>, String> {
    if terrain_types.len() != 65_536 || shading.len() != 65_536 || angles.len() != 65_536 {
        return Err("source world map requires complete 256x256 terrain planes".to_owned());
    }
    if tables.len() < 0x14100 {
        return Err("source world map requires the selected terrain shade table".to_owned());
    }
    let angle = usize::from(view.yaw.cast_unsigned() & 0x07ff);
    let sin = ((408_i64 * i64::from(SIN_16_16[angle])) >> 16) as i32;
    let cos = ((408_i64 * i64::from(SIN_16_16[angle + 512])) >> 16) as i32;
    let step_x = FRAME_HEIGHT as i32 * cos / MAP_WIDTH as i32;
    let step_y = FRAME_HEIGHT as i32 * sin / MAP_WIDTH as i32;
    let mut row_x = i32::from(view.x) - (MAP_WIDTH as i32 * step_x - FRAME_HEIGHT as i32 * sin) / 2;
    let mut row_y = i32::from(view.y) - (FRAME_HEIGHT as i32 * cos + MAP_WIDTH as i32 * step_y) / 2;
    let mut indices = vec![0; MAP_WIDTH * FRAME_HEIGHT];
    for row in indices.chunks_exact_mut(MAP_WIDTH) {
        let mut x = row_x;
        let mut y = row_y;
        for pixel in row {
            let tile = (((y >> 8) & 255) * 256 + ((x >> 8) & 255)) as usize;
            if !cave || angles[tile] & 8 == 0 {
                let base = usize::from(tables[0x14000 + usize::from(terrain_types[tile])]);
                *pixel = tables[(usize::from(shading[tile]) << 8) + base];
            }
            x = x.wrapping_add(step_x);
            y = y.wrapping_add(step_y);
        }
        row_x = row_x.wrapping_sub(sin);
        row_y = row_y.wrapping_add(cos);
    }
    Ok(indices)
}

pub fn compose_full_map(map: &[u8], world: &[u8], divider: u8) -> Result<Vec<u8>, String> {
    if map.len() != MAP_WIDTH * FRAME_HEIGHT || world.len() != WORLD_WIDTH * FRAME_HEIGHT {
        return Err("source world map has an incomplete map or narrow world viewport".to_owned());
    }
    let mut result = vec![divider; FRAME_WIDTH * FRAME_HEIGHT];
    for y in 0..FRAME_HEIGHT {
        let row = &mut result[y * FRAME_WIDTH..(y + 1) * FRAME_WIDTH];
        row[..MAP_WIDTH].copy_from_slice(&map[y * MAP_WIDTH..(y + 1) * MAP_WIDTH]);
        row[WORLD_X..].copy_from_slice(&world[y * WORLD_WIDTH..(y + 1) * WORLD_WIDTH]);
        // Source sub_9025C draws these after the second DrawWorld call.
        row[191] = divider;
        row[192] = divider;
    }
    Ok(result)
}

/// The map entity pass blends its centre once and then steps each of four
/// arms outwards. Blending is not generally idempotent, so four coincident
/// arm-zero writes would change the original centre pixel.
pub fn blend_full_map_cross(frame: &mut [u8], tables: &[u8], colour: u8) -> Result<(), String> {
    if frame.len() != FRAME_WIDTH * FRAME_HEIGHT || tables.len() < 0x14000 {
        return Err("source world-map cross requires a full frame and blend table".to_owned());
    }
    let center = &mut frame[100 * FRAME_WIDTH + 94];
    *center = tables[0x4000 + (usize::from(colour) << 8) + usize::from(*center)];
    for arm in 1..=15 {
        for (x, y) in [
            (94 - arm, 100),
            (94 + arm, 100),
            (94, 100 - arm),
            (94, 100 + arm),
        ] {
            let pixel = &mut frame[y * FRAME_WIDTH + x];
            *pixel = tables[0x4000 + (usize::from(colour) << 8) + usize::from(*pixel)];
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_map_keeps_source_rectangular_projection_and_two_dividers() {
        let view = WorldMapView {
            x: 65_400,
            y: 100,
            yaw: 0,
        };
        assert_eq!(
            MapGeometry::FULL_MAP.project(
                view,
                Position {
                    x: view.x,
                    y: view.y,
                    z: 0
                }
            ),
            (95, 100)
        );
        assert_eq!(
            MapGeometry::FULL_MAP.project(
                view,
                Position {
                    x: view.x.wrapping_add(408),
                    y: view.y,
                    z: 0
                }
            ),
            (95, 100)
        );
        let frame = compose_full_map(&vec![7; 191 * 200], &vec![9; 128 * 200], 3).unwrap();
        for row in frame.chunks_exact(320) {
            assert!(row[..191].iter().all(|&index| index == 7));
            assert_eq!(&row[191..193], &[3, 3]);
            assert!(row[193..].iter().all(|&index| index == 9));
        }
    }

    #[test]
    fn cave_voids_are_zero_and_map_reads_the_live_selected_planes() {
        let mut terrain = vec![0; 65_536];
        let mut shades = vec![0; 65_536];
        let mut angles = vec![0; 65_536];
        let mut tables = vec![0; 83_456];
        tables[0x14000 + 5] = 7;
        tables[0x0307] = 119;
        terrain.fill(5);
        shades.fill(3);
        let view = WorldMapView {
            x: 65_500,
            y: 10,
            yaw: -1,
        };
        assert!(
            render_full_map_terrain(view, &terrain, &shades, &angles, false, &tables)
                .unwrap()
                .iter()
                .all(|&index| index == 119)
        );
        angles.fill(8);
        assert!(
            render_full_map_terrain(view, &terrain, &shades, &angles, true, &tables)
                .unwrap()
                .iter()
                .all(|&index| index == 0)
        );
    }

    #[test]
    fn centre_cross_blends_each_of_its_source_pixels_once() {
        let mut frame = vec![0; FRAME_WIDTH * FRAME_HEIGHT];
        let mut tables = vec![0; 83_456];
        for value in 0..256 {
            tables[0x4000 + (7 << 8) + value] = (value as u8).wrapping_add(1);
        }
        blend_full_map_cross(&mut frame, &tables, 7).unwrap();
        assert_eq!(frame[100 * FRAME_WIDTH + 94], 1);
        assert_eq!(frame.iter().filter(|&&pixel| pixel == 1).count(), 61);
        assert!(frame.iter().all(|&pixel| pixel <= 1));
    }
}
