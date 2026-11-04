//! Scheduled terrain disasters. The arena owns identity and reclamation;
//! these rules write the same planes sampled by flight, buildings and render.

use crate::{
    Position,
    level1_mission::source_search_offsets,
    native_world::{MapType, SessionTerrain, add_building_to_terrain, rebuild_building_topology},
};

pub(crate) mod mc2;
pub(crate) mod paths;
pub(crate) mod ribbon;
pub(crate) mod travelling;
pub(crate) mod tremor;
pub(crate) mod volcano;

/// Immutable lowering-ring rules. An authored or spell producer supplies its
/// configured values before publication; subsequent ticks use the source row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RingProfile {
    pub(crate) max_life: i32,
    pub(crate) lifetime: i32,
    pub(crate) damage: u16,
    pub(crate) half_width: i16,
    pub(crate) half_height: i16,
}

impl From<crate::level_definition::TerrainDisasterDefinition> for RingProfile {
    fn from(definition: crate::level_definition::TerrainDisasterDefinition) -> Self {
        Self {
            max_life: definition.max_life(),
            lifetime: definition.lifetime(),
            damage: definition.damage(),
            half_width: definition.half_width(),
            half_height: definition.half_height(),
        }
    }
}

/// Signed source integer division by 256 truncates toward zero.
pub(crate) fn tile_radius(half_width: i16) -> i32 {
    i32::from(half_width) / 256
}

/// Source sub_56F10. Relative writes clamp floor heights to 200, unlike the
/// absolute cave writer's 255; the ceiling applies the *requested* delta.
pub(crate) fn relative_height(
    terrain: &mut SessionTerrain,
    source_x: i16,
    source_y: i16,
    delta: i16,
    protect: bool,
) -> bool {
    let (planes, map_type, rng, _) = terrain.disaster_parts();
    let x = source_x as u8;
    let y = source_y as u8;
    let index = usize::from(x) + 256 * usize::from(y);
    let requested = i16::from(planes.ground_heights[index]).wrapping_add(delta);
    let clipped_origin = (source_x == 0 && source_y == 0) && !(0..=200).contains(&requested);
    if protect && planes.angles[index] & 0x80 != 0 {
        return true;
    }
    let height = requested.clamp(0, 200) as u8;
    planes.ground_heights[index] = height;
    if map_type == MapType::Cave {
        let ceiling = i32::from(planes.secondary_heights[index]) - i32::from(delta);
        // Preserve the source's one-sided saturation and byte conversion.
        planes.secondary_heights[index] = if ceiling >= 255 { 255 } else { ceiling as u8 };
    }
    if height != 0 {
        planes.angles[index] = (planes.angles[index] & 0xf8) | 1;
    } else if [-1_i8, 0, 1].into_iter().all(|dy| {
        [-1_i8, 0, 1].into_iter().all(|dx| {
            (dx == 0 && dy == 0)
                || !matches!(
                    planes.angles[usize::from(x.wrapping_add_signed(dx))
                        + 256 * usize::from(y.wrapping_add_signed(dy))]
                        & 7,
                    2 | 3 | 5
                )
        })
    }) {
        planes.angles[index] &= 0xf0;
    }
    if protect {
        rebuild_building_topology(planes, x, y, x, y, map_type, rng);
    } else {
        add_building_to_terrain(planes, x, y, map_type, rng);
    }
    clipped_origin
}

/// Exact SEARCH.DAT class and row order, including sub_10130 returning 2
/// (rather than 1) for the last cell. Callers' while(==1) omits that cell.
fn search_band(first: i32, last: i32) -> Vec<(i8, i8)> {
    let mut offsets = source_search_offsets(last.max(first).clamp(0, 31) as u8);
    let start = if first > 0 {
        source_search_offsets((first - 1).min(31) as u8).len()
    } else {
        0
    };
    offsets.drain(..start);
    offsets.pop();
    offsets
}

/// sub_31F00: rounded center, bitmap footprint and literal per-cell rebuild.
pub(crate) fn lower_band(
    terrain: &mut SessionTerrain,
    position: Position,
    pitch: i16,
    first: i32,
    last: i32,
) {
    let x = ((u32::from(position.x) + 128) >> 8) as i16;
    let y = ((u32::from(position.y) + 128) >> 8) as i16;
    for (dx, dy) in search_band(first, last.min(tile_radius(pitch))) {
        relative_height(
            terrain,
            x.wrapping_add(i16::from(dx)),
            y.wrapping_add(i16::from(dy)),
            -3,
            false,
        );
    }
}

/// Source sub_572C0: its center casts the world coordinates to signed words
/// *before* rounding. Stop on the first saturated origin or protected cell.
pub(crate) fn deform_band(
    terrain: &mut SessionTerrain,
    position: Position,
    pitch: i16,
    first: i16,
    last: i16,
    delta: i16,
    protect: bool,
) -> bool {
    let x = (i32::from(position.x as i16) + 128) >> 8;
    let y = (i32::from(position.y as i16) + 128) >> 8;
    let mut mutated = false;
    let mut stopped = false;
    for (dx, dy) in search_band(i32::from(first), i32::from(last).min(tile_radius(pitch))) {
        let tile =
            usize::from((x + i32::from(dx)) as u8) + 256 * usize::from((y + i32::from(dy)) as u8);
        mutated |= !protect || terrain.planes().angles[tile] & 0x80 == 0;
        if relative_height(
            terrain,
            (x + i32::from(dx)) as i16,
            (y + i32::from(dy)) as i16,
            delta,
            protect,
        ) {
            stopped = true;
            break;
        }
    }
    if mutated {
        let (_, _, _, sequence) = terrain.disaster_parts();
        *sequence = sequence.wrapping_add(1);
    }
    stopped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FlightWorld,
        campaign::spec::{CampaignSelection, DecodedCampaignPackage},
    };
    use std::path::PathBuf;

    pub(super) fn mission_four() -> SessionTerrain {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261024-b/level4");
        let decoded = DecodedCampaignPackage::decode(
            &package,
            CampaignSelection::ordinary(4).expect("Mission4 identity"),
        )
        .expect("real Mission4 package decode");
        let mut terrain = decoded
            .generate_terrain_startup()
            .expect("ordinary source terrain construction")
            .terrain;
        let bytes = decoded
            .member("terrain-pre-features.bin")
            .expect("owner prefeature planes");
        let (planes, map_type, _, _) = terrain.disaster_parts();
        assert_eq!(map_type, MapType::Cave);
        planes.terrain_types.copy_from_slice(&bytes[..0x10000]);
        planes
            .ground_heights
            .copy_from_slice(&bytes[0x10000..0x20000]);
        planes.shading.copy_from_slice(&bytes[0x20000..0x30000]);
        planes.angles.copy_from_slice(&bytes[0x30000..0x40000]);
        planes
            .secondary_heights
            .copy_from_slice(&bytes[0x40000..0x50000]);
        terrain
    }

    #[test]
    fn relative_cave_write_preserves_delta_protection_and_live_sampling() {
        let mut terrain = mission_four();
        let x = 32_u8;
        let y = 32_u8;
        let index = usize::from(x) + usize::from(y) * 256;
        {
            let (planes, _, _, _) = terrain.disaster_parts();
            planes.ground_heights[index] = 199;
            planes.secondary_heights[index] = 100;
            planes.angles[index] = 0x81;
        }
        let protected_hash = terrain.deterministic_hash();
        assert!(relative_height(&mut terrain, 32, 32, 5, true));
        assert_eq!(terrain.deterministic_hash(), protected_hash);
        assert!(!relative_height(&mut terrain, 32, 32, 5, false));
        assert_eq!(terrain.planes().ground_heights[index], 200);
        assert_eq!(terrain.planes().secondary_heights[index], 199);
        assert_ne!(terrain.planes().angles[index] & 8, 0);
        // Topology closes the obstructed sample to floor-1. An open cell
        // independently checks application of the requested ceiling delta.
        {
            let (planes, _, _, _) = terrain.disaster_parts();
            planes.ground_heights[index] = 100;
            planes.secondary_heights[index] = 240;
            planes.angles[index] = 1;
        }
        relative_height(&mut terrain, 32, 32, -3, false);
        assert_eq!(terrain.planes().ground_heights[index], 97);
        assert_eq!(terrain.planes().secondary_heights[index], 243);
        let position = Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        };
        assert_eq!(terrain.ground_height(position), 97 * 32);
        assert!(terrain.ceiling_height(position).is_some());
    }

    #[test]
    fn bitmap_ring_preserves_source_last_cell_omission() {
        assert_eq!(search_band(0, 0), vec![(0, 0), (1, 0), (0, 1)]);
        let band = search_band(1, 1);
        assert_eq!(band.first(), Some(&(-1, -1)));
        assert_eq!(band.last(), Some(&(1, 2)));
        assert_eq!(band.len(), 11);
    }
}
