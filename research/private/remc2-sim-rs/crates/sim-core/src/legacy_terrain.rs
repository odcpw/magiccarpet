use crate::{FlightWorld, Position, Tile};

pub const LEGACY_MAP_CELLS: usize = 256 * 256;
pub const REGRESSION_TERRAIN_FRAME_BYTES: usize = 0x70000;

/// Source `sub_48F20` / `sub_48FD0`. The parameters are deliberately kept
/// in source order: the horizontal scan has `height` samples and the vertical
/// scan has `width` samples. Only the rectangle perimeter participates.
pub(crate) fn cave_perimeter_extrema(
    values: &[u8],
    x: u8,
    y: u8,
    width: u16,
    height: u16,
) -> (i32, i32) {
    let mut minimum = 250;
    let mut maximum = 0;
    let mut sample = |x, y| {
        let value = i32::from(values[usize::from(x) + 256 * usize::from(y)]);
        minimum = minimum.min(value);
        maximum = maximum.max(value);
    };
    for column in 0..height {
        let column = x.wrapping_add(column as u8);
        sample(column, y);
        sample(column, y.wrapping_add(width as u8));
    }
    for row in 0..width {
        let row = y.wrapping_add(row as u8);
        sample(x.wrapping_add(height as u8), row);
        sample(x, row);
    }
    (minimum, maximum)
}

/// Source `sub_43C60` uses a fresh local 32-bit stream for every rectangle.
/// Closed cells consume no draw; the second pass recomputes closure after
/// unsigned-byte perturbation. Neither session RNG is advanced here.
pub(crate) fn perturb_cave_ceiling(
    floor: &[u8],
    ceiling: &mut [u8],
    angles: &mut [u8],
    x: u8,
    y: u8,
    width: u16,
    height: u16,
) {
    let mut rng = 37_487_429_u32;
    for row in 0..width {
        for column in 0..height {
            let index = usize::from(x.wrapping_add(column as u8))
                + 256 * usize::from(y.wrapping_add(row as u8));
            if angles[index] & 8 == 0 {
                rng = rng.wrapping_mul(9_377).wrapping_add(9_439);
                ceiling[index] = ceiling[index].wrapping_add((rng % 7) as u8).wrapping_sub(3);
            }
        }
    }
    for row in 0..width {
        for column in 0..height {
            let index = usize::from(x.wrapping_add(column as u8))
                + 256 * usize::from(y.wrapping_add(row as u8));
            if ceiling[index] > floor[index] {
                angles[index] &= 0xf7;
            } else {
                ceiling[index] = floor[index].wrapping_sub(1);
                angles[index] |= 8;
            }
        }
    }
}

/// Owner-data terrain arrays with remc2's exact sampling rules.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyTerrain {
    terrain_types: Box<[u8]>,
    ground_heights: Box<[u8]>,
    cave_heights: Option<Box<[u8]>>,
    angle_flags: Box<[u8]>,
}

impl LegacyTerrain {
    /// Construct a provider from the four original 256-by-256 array surfaces.
    ///
    /// # Errors
    ///
    /// Returns an error when any supplied array is not exactly 65,536 bytes.
    pub fn new(
        terrain_types: Vec<u8>,
        ground_heights: Vec<u8>,
        cave_heights: Option<Vec<u8>>,
        angle_flags: Vec<u8>,
    ) -> Result<Self, String> {
        validate_len("terrain types", &terrain_types)?;
        validate_len("ground heights", &ground_heights)?;
        if let Some(cave) = &cave_heights {
            validate_len("cave heights", cave)?;
        }
        validate_len("angle flags", &angle_flags)?;
        Ok(Self {
            terrain_types: terrain_types.into_boxed_slice(),
            ground_heights: ground_heights.into_boxed_slice(),
            cave_heights: cave_heights.map(Vec::into_boxed_slice),
            angle_flags: angle_flags.into_boxed_slice(),
        })
    }

    /// Decode one original `DOSBox` regression terrain block.
    ///
    /// Each frame starts at DOS address `0x2dc4e0` and contains seven
    /// 65,536-byte surfaces. The required offsets are terrain type `0`,
    /// ground `1`, angle/topology `3`, and cave ceiling `4`.
    ///
    /// # Errors
    ///
    /// Returns an error when the sequence is not frame-aligned or the selected
    /// frame does not exist.
    pub fn from_regression_sequence(
        sequence: &[u8],
        frame_index: usize,
        cave_mode: bool,
    ) -> Result<Self, String> {
        if !sequence
            .len()
            .is_multiple_of(REGRESSION_TERRAIN_FRAME_BYTES)
        {
            return Err(format!(
                "terrain sequence length must be a multiple of {REGRESSION_TERRAIN_FRAME_BYTES}, got {}",
                sequence.len()
            ));
        }
        let frame_count = sequence.len() / REGRESSION_TERRAIN_FRAME_BYTES;
        if frame_index >= frame_count {
            return Err(format!(
                "terrain frame index {frame_index} is outside {frame_count} frames"
            ));
        }
        let start = frame_index * REGRESSION_TERRAIN_FRAME_BYTES;
        let frame = &sequence[start..start + REGRESSION_TERRAIN_FRAME_BYTES];
        let surface = |index: usize| {
            let start = index * LEGACY_MAP_CELLS;
            frame[start..start + LEGACY_MAP_CELLS].to_vec()
        };
        Self::new(
            surface(0),
            surface(1),
            cave_mode.then(|| surface(4)),
            surface(3),
        )
    }

    #[must_use]
    pub fn terrain_types(&self) -> &[u8] {
        &self.terrain_types
    }

    #[must_use]
    pub fn ground_heights(&self) -> &[u8] {
        &self.ground_heights
    }

    #[must_use]
    pub fn cave_heights(&self) -> Option<&[u8]> {
        self.cave_heights.as_deref()
    }

    #[must_use]
    pub fn angle_map(&self) -> &[u8] {
        &self.angle_flags
    }
}

impl FlightWorld for LegacyTerrain {
    fn ground_height(&self, position: Position) -> i16 {
        wrapping_i16(interpolate_ground(
            &self.ground_heights,
            position.x,
            position.y,
        ))
    }

    fn ceiling_height(&self, position: Position) -> Option<i16> {
        self.cave_heights
            .as_ref()
            .map(|heights| wrapping_i16(interpolate_cave(heights, position.x, position.y)))
    }

    fn material_mask(&self, position: Position) -> u32 {
        material_mask(self.terrain_types[tile_index(tile_for(position))])
    }

    fn terrain_type(&self, position: Position) -> u8 {
        self.terrain_types[tile_index(tile_for(position))]
    }

    fn angle_flags(&self, tile: Tile) -> u8 {
        self.angle_flags[tile_index(tile)]
    }
}

fn validate_len(name: &str, values: &[u8]) -> Result<(), String> {
    if values.len() == LEGACY_MAP_CELLS {
        Ok(())
    } else {
        Err(format!(
            "{name} must contain {LEGACY_MAP_CELLS} bytes, got {}",
            values.len()
        ))
    }
}

fn tile_for(position: Position) -> Tile {
    Tile {
        x: position.x.to_be_bytes()[0],
        y: position.y.to_be_bytes()[0],
    }
}

fn tile_index(tile: Tile) -> usize {
    usize::from(tile.x) + usize::from(tile.y) * 256
}

fn sample(values: &[u8], x: u8, y: u8) -> i32 {
    i32::from(values[usize::from(x) + usize::from(y) * 256])
}

#[allow(clippy::many_single_char_names)]
pub(crate) fn interpolate_ground(values: &[u8], x: u16, y: u16) -> i32 {
    let [x_fraction, tile_x] = x.to_le_bytes();
    let [y_fraction, tile_y] = y.to_le_bytes();
    let odd_diagonal = tile_x.wrapping_add(tile_y) & 1 != 0;

    let (point, delta) = if odd_diagonal {
        if y_fraction > y_fraction.wrapping_add(x_fraction) {
            let p1 = sample(values, tile_x, tile_y.wrapping_add(1));
            let p2 = sample(values, tile_x.wrapping_add(1), tile_y.wrapping_add(1));
            let diagonal = i32::from(255 - y_fraction)
                * (sample(values, tile_x.wrapping_add(1), tile_y) - p2)
                + i32::from(x_fraction) * (p2 - p1);
            (p1, diagonal)
        } else {
            let p1 = sample(values, tile_x, tile_y);
            let x_delta =
                i32::from(x_fraction) * (sample(values, tile_x.wrapping_add(1), tile_y) - p1);
            let diagonal = i32::from(y_fraction)
                * (sample(values, tile_x, tile_y.wrapping_add(1)) - p1)
                + x_delta;
            (p1, diagonal)
        }
    } else if x_fraction <= y_fraction {
        let p1 = sample(values, tile_x, tile_y);
        let p2 = sample(values, tile_x, tile_y.wrapping_add(1));
        let diagonal = i32::from(y_fraction) * (p2 - p1)
            + i32::from(x_fraction)
                * (sample(values, tile_x.wrapping_add(1), tile_y.wrapping_add(1)) - p2);
        (p1, diagonal)
    } else {
        let p1 = sample(values, tile_x, tile_y);
        let p2 = sample(values, tile_x.wrapping_add(1), tile_y);
        let diagonal = i32::from(y_fraction)
            * (sample(values, tile_x.wrapping_add(1), tile_y.wrapping_add(1)) - p2)
            + i32::from(x_fraction) * (p2 - p1);
        (p1, diagonal)
    };
    (delta >> 3) + 32 * point
}

#[allow(clippy::many_single_char_names)]
pub(crate) fn interpolate_cave(values: &[u8], x: u16, y: u16) -> i32 {
    let [x_fraction, tile_x] = x.to_le_bytes();
    let [y_fraction, tile_y] = y.to_le_bytes();
    let odd_diagonal = tile_x.wrapping_add(tile_y) & 1 != 0;

    let (point, delta) = if odd_diagonal {
        if y_fraction > y_fraction.wrapping_add(x_fraction) {
            let p1 = sample(values, tile_x, tile_y.wrapping_add(1));
            let p2 = sample(values, tile_x.wrapping_add(1), tile_y.wrapping_add(1));
            let diagonal = i32::from(255 - y_fraction)
                * (sample(values, tile_x.wrapping_add(1), tile_y) - p2)
                + i32::from(x_fraction) * (p2 - p1);
            (p1, diagonal)
        } else {
            let p1 = sample(values, tile_x, tile_y);
            let p2 = sample(values, tile_x.wrapping_add(1), tile_y);
            let diagonal = i32::from(y_fraction)
                * (sample(values, tile_x, tile_y.wrapping_add(1)) - p1)
                + i32::from(x_fraction) * (p2 - p1);
            (p1, diagonal)
        }
    } else if x_fraction <= y_fraction {
        let p1 = sample(values, tile_x, tile_y);
        let p2 = sample(values, tile_x, tile_y.wrapping_add(1));
        let diagonal = i32::from(y_fraction) * (p2 - p1)
            + i32::from(x_fraction)
                * (sample(values, tile_x.wrapping_add(1), tile_y.wrapping_add(1)) - p2);
        (p1, diagonal)
    } else {
        let p1 = sample(values, tile_x, tile_y);
        // This double x increment is present in remc2's `sub_B5D68`.
        let p2 = sample(values, tile_x.wrapping_add(2), tile_y);
        let diagonal = i32::from(y_fraction)
            * (sample(values, tile_x.wrapping_add(2), tile_y.wrapping_add(1)) - p2)
            + i32::from(x_fraction) * (p2 - p1);
        (p1, diagonal)
    };
    (delta >> 3) + 32 * point
}

pub(crate) fn material_mask(tile_type: u8) -> u32 {
    match tile_type {
        0 => 1,
        1 => 2,
        2 => 4,
        3 => 8,
        4 => 0x10,
        5 => 0x20,
        8 => 0x100,
        9 => 0x200,
        10 => 0x10_0000,
        11 => 0x20_0000,
        12 => 0x40_0000,
        13 | 14 => 0,
        15..=20 | 28..=34 => 0x400,
        21 | 22 | 24 => 0x2_0000,
        23 => 0x4_0000,
        25 | 27 => 0x8_0000,
        26 => 0x1_0000,
        _ => 0x80_0000,
    }
}

fn wrapping_i16(value: i32) -> i16 {
    let bytes = value.to_le_bytes();
    i16::from_le_bytes([bytes[0], bytes[1]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain(
        ground: Vec<u8>,
        cave: Option<Vec<u8>>,
        terrain_types: Vec<u8>,
        angles: Vec<u8>,
    ) -> LegacyTerrain {
        LegacyTerrain::new(terrain_types, ground, cave, angles).expect("valid fixture arrays")
    }

    #[test]
    fn constructor_locks_all_four_array_denominators() {
        let result = LegacyTerrain::new(vec![], vec![0; LEGACY_MAP_CELLS], None, vec![0; 1]);
        assert_eq!(
            result,
            Err("terrain types must contain 65536 bytes, got 0".to_owned())
        );
    }

    #[test]
    fn material_switch_and_tile_byte_order_match_the_source() {
        let mut types = vec![13; LEGACY_MAP_CELLS];
        types[tile_index(Tile { x: 0x12, y: 0x34 })] = 8;
        let mut angles = vec![0; LEGACY_MAP_CELLS];
        angles[tile_index(Tile { x: 0x12, y: 0x34 })] = 0xa5;
        let world = terrain(vec![0; LEGACY_MAP_CELLS], None, types, angles);
        let position = Position {
            x: 0x12ff,
            y: 0x3401,
            z: 0,
        };
        assert_eq!(world.material_mask(position), 0x100);
        assert_eq!(world.angle_flags(Tile { x: 0x12, y: 0x34 }), 0xa5);
    }

    #[test]
    fn ground_interpolation_uses_source_triangles_and_wraps_map_edges() {
        let mut heights = vec![0; LEGACY_MAP_CELLS];
        heights[tile_index(Tile { x: 10, y: 10 })] = 10;
        heights[tile_index(Tile { x: 11, y: 10 })] = 20;
        heights[tile_index(Tile { x: 10, y: 11 })] = 30;
        heights[tile_index(Tile { x: 11, y: 11 })] = 40;
        heights[tile_index(Tile { x: 255, y: 255 })] = 7;
        let world = terrain(
            heights,
            None,
            vec![0; LEGACY_MAP_CELLS],
            vec![0; LEGACY_MAP_CELLS],
        );
        assert_eq!(
            world.ground_height(Position {
                x: 10 << 8,
                y: 10 << 8,
                z: 0
            }),
            320
        );
        assert_eq!(
            world.ground_height(Position {
                x: (10 << 8) | 0x80,
                y: (10 << 8) | 0x80,
                z: 0
            }),
            800
        );
        assert_eq!(
            world.ground_height(Position {
                x: 0xffff,
                y: 0xffff,
                z: 0
            }),
            0
        );
    }

    #[test]
    fn cave_even_lower_triangle_preserves_the_reconstructed_double_x_step() {
        let mut cave = vec![0; LEGACY_MAP_CELLS];
        cave[tile_index(Tile { x: 10, y: 10 })] = 10;
        cave[tile_index(Tile { x: 11, y: 10 })] = 20;
        cave[tile_index(Tile { x: 12, y: 10 })] = 30;
        cave[tile_index(Tile { x: 12, y: 11 })] = 50;
        let world = terrain(
            vec![0; LEGACY_MAP_CELLS],
            Some(cave),
            vec![0; LEGACY_MAP_CELLS],
            vec![0; LEGACY_MAP_CELLS],
        );
        let position = Position {
            x: (10 << 8) | 0xc0,
            y: (10 << 8) | 0x40,
            z: 0,
        };
        assert_eq!(world.ceiling_height(position), Some(960));
    }

    #[test]
    fn owned_level_one_regression_frame_loads_real_original_arrays() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions/level1/sequence-002285FF-002DC4E0.bin",
        );
        let sequence = std::fs::read(path).expect("read preserved level-one terrain sequence");
        assert_eq!(sequence.len(), 21 * REGRESSION_TERRAIN_FRAME_BYTES);
        let terrain =
            LegacyTerrain::from_regression_sequence(&sequence, 0, true).expect("decode frame zero");

        let sum_and_nonzero = |bytes: &[u8]| {
            (
                bytes.iter().map(|value| u64::from(*value)).sum::<u64>(),
                bytes.iter().filter(|value| **value != 0).count(),
            )
        };
        assert_eq!(sum_and_nonzero(terrain.terrain_types()), (910_755, 22_753));
        assert_eq!(
            sum_and_nonzero(terrain.ground_heights()),
            (1_191_099, 16_321)
        );
        assert_eq!(sum_and_nonzero(terrain.angle_map()), (3_957_726, 64_972));
        assert_eq!(
            sum_and_nonzero(terrain.cave_heights().expect("cave surface projected")),
            (4_043_664, 65_536)
        );
    }
}
#[test]
fn cave_extrema_exclude_interior_and_ceiling_noise_restarts_local_rng() {
    let mut floor = vec![40; LEGACY_MAP_CELLS];
    floor[10 + 256 * 10] = 0;
    let (minimum, maximum) = cave_perimeter_extrema(&floor, 8, 8, 4, 4);
    assert_eq!((minimum, maximum), (40, 40));
    let mut first = vec![60; LEGACY_MAP_CELLS];
    let mut second = first.clone();
    let mut first_angles = vec![0; LEGACY_MAP_CELLS];
    let mut second_angles = first_angles.clone();
    first_angles[8 + 256 * 8] = 8;
    second_angles[8 + 256 * 8] = 8;
    perturb_cave_ceiling(&floor, &mut first, &mut first_angles, 8, 8, 4, 4);
    perturb_cave_ceiling(&floor, &mut second, &mut second_angles, 8, 8, 4, 4);
    assert_eq!(
        (first.as_slice(), first_angles.as_slice()),
        (second.as_slice(), second_angles.as_slice())
    );
    assert_eq!(
        first[8 + 256 * 8],
        60,
        "closed cells consume no perturbation draw"
    );
    let first_rng = 37_487_429_u32.wrapping_mul(9_377).wrapping_add(9_439);
    assert_eq!(
        first[9 + 256 * 8],
        60_u8.wrapping_add((first_rng % 7) as u8).wrapping_sub(3)
    );
    assert_eq!(
        first_angles[8 + 256 * 8] & 8,
        0,
        "closure is recomputed even for previously closed cells"
    );
}
