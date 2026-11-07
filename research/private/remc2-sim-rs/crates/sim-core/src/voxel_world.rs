//! Bounded, deterministic voxel edits for an explicitly enabled session.
//!
//! Coordinates are native units (64 per presentation unit), with X/Y on the
//! map and Z vertical. Base geometry is supplied by the level adapter; this
//! module owns the sparse differences, not an independent simulation.

use std::collections::BTreeSet;
use std::sync::Arc;

const MAX_COLUMNS: usize = 262_144;
const MAX_CELLS: usize = 1_048_576;
const MAX_REMOVED: usize = 1_048_576;
const MAX_RADIUS: i32 = 256;
const MAX_SEGMENT_STEPS: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum VolumeId {
    Terrain,
    Building(u16),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CellId {
    pub volume: VolumeId,
    pub x: u16,
    pub y: u16,
    pub z: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Grid {
    pub origin: Point,
    pub cell_size: u16,
    pub dimensions: [u16; 3],
}

impl Grid {
    fn validate(self) -> Result<usize, &'static str> {
        if self.cell_size < 8 || self.dimensions.contains(&0) {
            return Err("voxel grid dimensions must be nonzero and cells at least 8 units");
        }
        let count = self
            .dimensions
            .into_iter()
            .try_fold(1_usize, |a, b| a.checked_mul(usize::from(b)))
            .ok_or("voxel grid size overflow")?;
        for (origin, dim) in [self.origin.x, self.origin.y, self.origin.z]
            .into_iter()
            .zip(self.dimensions)
        {
            let end = i64::from(origin) + i64::from(self.cell_size) * i64::from(dim);
            i32::try_from(end).map_err(|_| "voxel grid coordinate overflow")?;
        }
        Ok(count)
    }

    fn index(self, p: Point) -> Option<[u16; 3]> {
        let size = i32::from(self.cell_size);
        let mut out = [0; 3];
        for (i, (value, origin)) in [p.x, p.y, p.z]
            .into_iter()
            .zip([self.origin.x, self.origin.y, self.origin.z])
            .enumerate()
        {
            let n = (i64::from(value) - i64::from(origin)).div_euclid(i64::from(size));
            if n < 0 || n >= i64::from(self.dimensions[i]) {
                return None;
            }
            out[i] = n as u16;
        }
        Some(out)
    }

    fn center(self, index: [u16; 3]) -> Point {
        let s = i64::from(self.cell_size);
        Point {
            x: (i64::from(self.origin.x) + i64::from(index[0]) * s + s / 2) as i32,
            y: (i64::from(self.origin.y) + i64::from(index[1]) * s + s / 2) as i32,
            z: (i64::from(self.origin.z) + i64::from(index[2]) * s + s / 2) as i32,
        }
    }

    fn linear(self, p: [u16; 3]) -> usize {
        (usize::from(p[2]) * usize::from(self.dimensions[1]) + usize::from(p[1]))
            * usize::from(self.dimensions[0])
            + usize::from(p[0])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Base {
    // Topmost solid cell count per X/Y column. Cells below the bounded patch
    // remain solid; the adapter uses this only for the surface layer.
    Terrain(Vec<u16>),
    Building(Vec<u64>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Volume {
    pub id: VolumeId,
    pub grid: Grid,
    base: Base,
}

impl Volume {
    /// Immutable column counts in row-major X/Y order for terrain projection.
    pub fn terrain_columns(&self) -> Option<&[u16]> {
        match &self.base {
            Base::Terrain(columns) => Some(columns),
            Base::Building(_) => None,
        }
    }

    /// Immutable packed occupancy, indexed X-fastest, then Y, then Z.
    pub fn building_bits(&self) -> Option<&[u64]> {
        match &self.base {
            Base::Terrain(_) => None,
            Base::Building(bits) => Some(bits),
        }
    }

    pub fn terrain(grid: Grid, heights: Vec<u16>) -> Result<Self, &'static str> {
        grid.validate()?;
        let columns = usize::from(grid.dimensions[0]) * usize::from(grid.dimensions[1]);
        if columns > MAX_COLUMNS
            || heights.len() != columns
            || heights.iter().any(|h| *h > grid.dimensions[2])
        {
            return Err("invalid voxel terrain columns");
        }
        Ok(Self {
            id: VolumeId::Terrain,
            grid,
            base: Base::Terrain(heights),
        })
    }

    pub fn building(
        id: u16,
        grid: Grid,
        cells: impl IntoIterator<Item = [u16; 3]>,
    ) -> Result<Self, &'static str> {
        let count = grid.validate()?;
        if count > MAX_CELLS {
            return Err("voxel building exceeds cell limit");
        }
        let mut bits = vec![0_u64; count.div_ceil(64)];
        for p in cells {
            if p.into_iter()
                .zip(grid.dimensions)
                .any(|(value, limit)| value >= limit)
            {
                return Err("building cell outside voxel grid");
            }
            let index = grid.linear(p);
            bits[index / 64] |= 1_u64 << (index % 64);
        }
        Ok(Self {
            id: VolumeId::Building(id),
            grid,
            base: Base::Building(bits),
        })
    }

    fn solid(&self, p: [u16; 3]) -> bool {
        match &self.base {
            Base::Terrain(heights) => {
                p[2] < heights
                    [usize::from(p[1]) * usize::from(self.grid.dimensions[0]) + usize::from(p[0])]
            }
            Base::Building(bits) => {
                let index = self.grid.linear(p);
                bits[index / 64] & (1_u64 << (index % 64)) != 0
            }
        }
    }

    fn cell(&self, p: [u16; 3]) -> CellId {
        CellId {
            volume: self.id,
            x: p[0],
            y: p[1],
            z: p[2],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(generation: u64) -> VoxelWorld {
        let terrain = Grid {
            origin: Point { x: 0, y: 0, z: -32 },
            cell_size: 8,
            dimensions: [8, 8, 8],
        };
        let building = Grid {
            origin: Point {
                x: 16,
                y: 16,
                z: 32,
            },
            cell_size: 16,
            dimensions: [3, 3, 3],
        };
        // A solid outer shell with a missing central cell.
        let shell = (0..3).flat_map(|z| {
            (0..3).flat_map(move |y| {
                (0..3).filter_map(move |x| (x != 1 || y != 1 || z != 1).then_some([x, y, z]))
            })
        });
        VoxelWorld::new(
            generation,
            vec![
                Volume::terrain(terrain, vec![8; 64]).unwrap(),
                Volume::building(38, building, shell).unwrap(),
            ],
        )
        .unwrap()
    }

    #[test]
    fn crater_changes_surface_and_geometry_and_replay_is_stable() {
        let mut world = fixture(7);
        let mut replay = fixture(7);
        let before = world.clone();
        assert!(Arc::ptr_eq(&world.volumes, &before.volumes));
        assert!(Arc::ptr_eq(&world.removed, &before.removed));
        let point = Point {
            x: 28,
            y: 28,
            z: 28,
        };
        assert_eq!(world.terrain_height(point.x, point.y), Some(32));
        let edit = world.remove_sphere(point, 12).unwrap();
        assert!(Arc::ptr_eq(&world.volumes, &before.volumes));
        assert!(!Arc::ptr_eq(&world.removed, &before.removed));
        assert_eq!(before.terrain_height(point.x, point.y), Some(32));
        assert!(!edit.removed.is_empty());
        assert!(world.terrain_height(point.x, point.y).unwrap() < 32);
        assert_eq!(world.occupied(point), None);
        assert_eq!(world.terrain_height(-1, point.y), None);
        assert_eq!(edit, replay.remove_sphere(point, 12).unwrap());
        assert_eq!(world, replay);
        assert_eq!(world.deterministic_hash(), replay.deterministic_hash());
        assert_eq!(world.removed_cells().collect::<Vec<_>>(), edit.removed);
        let duplicate = world.remove_sphere(point, 12).unwrap();
        assert!(duplicate.removed.is_empty());
        assert_eq!(duplicate.revision, 1);
    }

    #[test]
    fn building_breach_changes_segment_collision_without_erasing_neighbor() {
        let mut world = fixture(11);
        let start = Point { x: 0, y: 40, z: 56 };
        let end = Point {
            x: 64,
            y: 40,
            z: 56,
        };
        let (_, first) = world.segment_contact(start, end).unwrap().unwrap();
        assert_eq!(first.volume, VolumeId::Building(38));
        let center = Point {
            x: 24,
            y: 40,
            z: 56,
        };
        let edit = world.remove_sphere(center, 9).unwrap();
        assert!(edit.removed.contains(&first));
        assert_eq!(world.occupied(center), None);
        assert!(
            world
                .occupied(Point {
                    x: 24,
                    y: 24,
                    z: 56
                })
                .is_some()
        );
        let (_, second) = world.segment_contact(start, end).unwrap().unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn segment_finds_thin_corner_crossing_in_both_directions() {
        let grid = Grid {
            origin: Point { x: 0, y: 0, z: 0 },
            cell_size: 8,
            dimensions: [3, 3, 1],
        };
        let world =
            VoxelWorld::new(1, vec![Volume::building(2, grid, [[1, 0, 0]]).unwrap()]).unwrap();
        // The segment clips cell (1,0,0) for less than one native unit.
        let a = Point { x: 0, y: 0, z: 4 };
        let b = Point { x: 16, y: 15, z: 4 };
        let forward = world.segment_contact(a, b).unwrap().unwrap().1;
        let reverse = world.segment_contact(b, a).unwrap().unwrap().1;
        assert_eq!(
            forward,
            CellId {
                volume: VolumeId::Building(2),
                x: 1,
                y: 0,
                z: 0
            }
        );
        assert_eq!(forward, reverse);
        let miss_a = Point { x: 0, y: 9, z: 4 };
        let miss_b = Point { x: 24, y: 9, z: 4 };
        assert_eq!(world.segment_contact(miss_a, miss_b).unwrap(), None);
        assert_eq!(
            world
                .building_sweep_contact(miss_a, miss_b, 2, 0)
                .unwrap()
                .unwrap()
                .1,
            forward
        );
        assert_eq!(
            world
                .building_sweep_contact(miss_b, miss_a, 2, 0)
                .unwrap()
                .unwrap()
                .1,
            forward
        );
    }

    #[test]
    fn content_hash_ignores_revision_and_crater_has_no_overhang() {
        let grid = Grid {
            origin: Point { x: 0, y: 0, z: 0 },
            cell_size: 8,
            dimensions: [2, 1, 4],
        };
        let make = || VoxelWorld::new(2, vec![Volume::terrain(grid, vec![4, 4]).unwrap()]).unwrap();
        let mut one = make();
        let mut two = make();
        one.remove_sphere(Point { x: 8, y: 4, z: 4 }, 5).unwrap();
        two.remove_sphere(Point { x: 4, y: 4, z: 4 }, 1).unwrap();
        two.remove_sphere(Point { x: 12, y: 4, z: 4 }, 1).unwrap();
        assert_ne!(one.revision(), two.revision());
        assert_eq!(
            one.removed_cells().collect::<Vec<_>>(),
            two.removed_cells().collect::<Vec<_>>()
        );
        assert_eq!(one.deterministic_hash(), two.deterministic_hash());
        assert_eq!(one.terrain_height(4, 4), Some(0));
        assert_eq!(one.occupied(Point { x: 4, y: 4, z: 28 }), None);
    }

    #[test]
    fn grid_checks_wide_extent_before_casting() {
        let valid = Grid {
            origin: Point {
                x: i32::MIN,
                y: 0,
                z: 0,
            },
            cell_size: u16::MAX,
            dimensions: [u16::MAX, 1, 1],
        };
        assert!(valid.validate().is_ok());
        assert!(valid.center([u16::MAX - 1, 0, 0]).x > 0);
        let invalid = Grid {
            origin: Point { x: 0, y: 0, z: 0 },
            ..valid
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn invalid_inputs_leave_world_intact_and_generation_distinguishes_reset() {
        let mut world = fixture(12);
        let before = world.clone();
        assert!(world.remove_sphere(Point { x: 0, y: 0, z: 0 }, 0).is_err());
        assert!(
            world
                .remove_sphere(Point { x: 0, y: 0, z: 0 }, MAX_RADIUS + 1)
                .is_err()
        );
        assert_eq!(world, before);
        let reset = fixture(13);
        assert_eq!(reset.deterministic_hash(), before.deterministic_hash());
        assert_ne!(reset.generation(), before.generation());
        assert_eq!(
            reset.occupied(Point {
                x: 24,
                y: 24,
                z: 56
            }),
            before.occupied(Point {
                x: 24,
                y: 24,
                z: 56
            })
        );
        let bad = Grid {
            origin: Point { x: 0, y: 0, z: 0 },
            cell_size: 8,
            dimensions: [2, 2, 2],
        };
        assert!(Volume::terrain(bad, vec![3; 4]).is_err());
        assert!(Volume::building(38, bad, [[2, 0, 0]]).is_err());
    }

    #[test]
    fn seed_strips_source_stamp_and_replays_geometry_from_serialized_inputs() {
        let mut plane = vec![100; 65_536];
        let x = 32_000_u16;
        let y = 40_000_u16;
        let sx = ((i32::from(x) + 128) >> 8) - 1;
        let sy = ((i32::from(y) + 128) >> 8) - 1;
        let mut cells = vec![[16, 0]; 9];
        cells[4][1] = 7;
        plane[(((sy + 1) & 255) * 256 + ((sx + 1) & 255)) as usize] = 107;
        let seed = VoxelWorldSeed {
            height_plane: plane,
            houses: vec![HouseSeed {
                slot: 38,
                build_index: 26,
                x,
                y,
                width: 3,
                height: 3,
                cells,
            }],
            selected_slot: 38,
            modern_heights: vec![50.0_f32.to_bits(); 17 * 21 + 256 * 320],
        };
        let mut world = seed.build_world(4).unwrap();
        let encoded = serde_json::to_vec(&seed).unwrap();
        let restored: VoxelWorldSeed = serde_json::from_slice(&encoded).unwrap();
        let replay = restored.build_world(4).unwrap();
        assert_eq!(world, replay);
        let building = world
            .volumes()
            .iter()
            .find(|v| v.id == VolumeId::Building(38))
            .unwrap();
        let center = Point {
            x: building.grid.origin.x + 264,
            y: building.grid.origin.y + 264,
            z: building.grid.origin.z + 8,
        };
        assert_eq!(world.terrain_height(center.x, center.y), Some(3200));
        assert_eq!(
            world.occupied(center).unwrap().volume,
            VolumeId::Building(38)
        );
        let edit = world.remove_sphere(center, 106).unwrap();
        assert!(!edit.removed.is_empty());
        assert_eq!(world.occupied(center), None);
        assert_ne!(world.deterministic_hash(), replay.deterministic_hash());
        let mut bad = restored;
        bad.height_plane[(((sy + 1) & 255) * 256 + ((sx + 1) & 255)) as usize] = 3;
        assert!(bad.build_world(4).is_err());
        let mut overlap = seed.clone();
        let mut neighbor = seed.houses[0].clone();
        neighbor.slot = 39;
        neighbor.x += 1280;
        let nx = ((i32::from(neighbor.x) + 128) >> 8) - 1;
        overlap.height_plane[(((sy + 1) & 255) * 256 + ((nx + 1) & 255)) as usize] = 107;
        overlap.houses.push(neighbor);
        assert_eq!(
            overlap.build_world(4).unwrap_err(),
            "another BUILD stamp intersects the voxel patch"
        );
        let mut invalid = seed;
        invalid.modern_heights[0] = f32::NAN.to_bits();
        assert!(invalid.build_world(4).is_err());
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelWorld {
    generation: u64,
    revision: u64,
    volumes: Arc<Vec<Volume>>,
    removed: Arc<BTreeSet<CellId>>,
    base_hash: u64,
    content_hash: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Edit {
    pub generation: u64,
    pub revision: u64,
    pub center: Point,
    pub removed: Vec<CellId>,
}

impl VoxelWorld {
    pub fn new(generation: u64, mut volumes: Vec<Volume>) -> Result<Self, &'static str> {
        volumes.sort_by_key(|volume| volume.id);
        if volumes.is_empty()
            || volumes.len() > 16
            || volumes.windows(2).any(|w| w[0].id == w[1].id)
        {
            return Err("voxel world needs unique bounded volumes");
        }
        for volume in &volumes {
            let count = volume.grid.validate()?;
            match (&volume.id, &volume.base) {
                (VolumeId::Terrain, Base::Terrain(heights))
                    if heights.len()
                        == usize::from(volume.grid.dimensions[0])
                            * usize::from(volume.grid.dimensions[1])
                        && heights.len() <= MAX_COLUMNS
                        && heights.iter().all(|h| *h <= volume.grid.dimensions[2]) => {}
                (VolumeId::Building(_), Base::Building(bits))
                    if count <= MAX_CELLS && bits.len() == count.div_ceil(64) => {}
                _ => return Err("voxel volume grid no longer matches its seed"),
            }
        }
        let base_hash = Self::hash_base(&volumes);
        let mut world = Self {
            generation,
            revision: 0,
            volumes: Arc::new(volumes),
            removed: Arc::new(BTreeSet::new()),
            base_hash,
            content_hash: 0,
        };
        world.content_hash = world.hash_contents();
        Ok(world)
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// Rebind observation identity after session reset; geometry is unchanged.
    pub fn set_generation(&mut self, generation: u64) {
        self.generation = generation;
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn removed_cells(&self) -> impl Iterator<Item = CellId> + '_ {
        self.removed.iter().copied()
    }
    pub fn removed_count(&self) -> usize {
        self.removed.len()
    }
    pub fn volumes(&self) -> &[Volume] {
        &self.volumes
    }

    pub fn occupied(&self, point: Point) -> Option<CellId> {
        self.volumes.iter().find_map(|volume| {
            let index = volume.grid.index(point)?;
            let cell = volume.cell(index);
            (volume.solid(index) && !self.removed.contains(&cell)).then_some(cell)
        })
    }

    /// Highest terrain surface in a column inside the bounded patch, in native
    /// Z units. Outside returns None so the caller can use its original terrain.
    pub fn terrain_height(&self, x: i32, y: i32) -> Option<i32> {
        let volume = self.volumes.iter().find(|v| v.id == VolumeId::Terrain)?;
        let grid = volume.grid;
        let probe = Point {
            x,
            y,
            z: grid.origin.z,
        };
        let [cx, cy, _] = grid.index(probe)?;
        let Base::Terrain(heights) = &volume.base else {
            return None;
        };
        let count = heights[usize::from(cy) * usize::from(grid.dimensions[0]) + usize::from(cx)];
        for cz in (0..count).rev() {
            if !self.removed.contains(&volume.cell([cx, cy, cz])) {
                return Some(
                    (i64::from(grid.origin.z) + (i64::from(cz) + 1) * i64::from(grid.cell_size))
                        as i32,
                );
            }
        }
        Some(grid.origin.z)
    }

    /// Removes initially solid cell centers inside a sphere. The change is
    /// atomic if its bounded sparse representation would exceed the limit.
    pub fn remove_sphere(&mut self, center: Point, radius: i32) -> Result<Edit, &'static str> {
        if !(1..=MAX_RADIUS).contains(&radius) {
            return Err("voxel radius outside allowed range");
        }
        let mut removed = BTreeSet::new();
        let r2 = i64::from(radius) * i64::from(radius);
        for volume in self.volumes.iter() {
            let g = volume.grid;
            let s = i64::from(g.cell_size);
            let mut lo = [0_i64; 3];
            let mut hi = [0_i64; 3];
            for (i, (c, origin)) in [center.x, center.y, center.z]
                .into_iter()
                .zip([g.origin.x, g.origin.y, g.origin.z])
                .enumerate()
            {
                lo[i] = (i64::from(c) - i64::from(radius) - i64::from(origin))
                    .div_euclid(s)
                    .max(0);
                hi[i] = (i64::from(c) + i64::from(radius) - i64::from(origin))
                    .div_euclid(s)
                    .min(i64::from(g.dimensions[i]) - 1);
            }
            if (0..3).any(|i| lo[i] > hi[i]) {
                continue;
            }
            for z in lo[2]..=hi[2] {
                for y in lo[1]..=hi[1] {
                    for x in lo[0]..=hi[0] {
                        let p = [x as u16, y as u16, z as u16];
                        if !volume.solid(p) {
                            continue;
                        }
                        let c = g.center(p);
                        let d = [c.x, c.y, c.z]
                            .into_iter()
                            .zip([center.x, center.y, center.z])
                            .map(|(a, b)| {
                                let delta = i64::from(a) - i64::from(b);
                                delta * delta
                            })
                            .sum::<i64>();
                        let cell = volume.cell(p);
                        if d <= r2 && !self.removed.contains(&cell) {
                            removed.insert(cell);
                            if removed.len() > MAX_REMOVED - self.removed.len() {
                                return Err("voxel removal limit exceeded");
                            }
                        }
                    }
                }
            }
        }
        // Craters are surface cuts: remove every remaining terrain cell above
        // the lowest cut in each touched column, so no floating ceiling remains.
        if let Some(terrain) = self.volumes.iter().find(|v| v.id == VolumeId::Terrain) {
            let mut cuts = std::collections::BTreeMap::<(u16, u16), u16>::new();
            for cell in removed.iter().filter(|c| c.volume == VolumeId::Terrain) {
                cuts.entry((cell.x, cell.y))
                    .and_modify(|z| *z = (*z).min(cell.z))
                    .or_insert(cell.z);
            }
            for ((x, y), bottom) in cuts {
                for z in bottom..terrain.grid.dimensions[2] {
                    let index = [x, y, z];
                    let cell = terrain.cell(index);
                    if terrain.solid(index) && !self.removed.contains(&cell) {
                        removed.insert(cell);
                        if removed.len() > MAX_REMOVED - self.removed.len() {
                            return Err("voxel removal limit exceeded");
                        }
                    }
                }
            }
        }
        if self
            .removed
            .len()
            .checked_add(removed.len())
            .is_none_or(|n| n > MAX_REMOVED)
        {
            return Err("voxel removal limit exceeded");
        }
        if !removed.is_empty() {
            self.revision = self
                .revision
                .checked_add(1)
                .ok_or("voxel revision exhausted")?;
            Arc::make_mut(&mut self.removed).extend(removed.iter().copied());
            self.content_hash = self.hash_contents();
        }
        Ok(Edit {
            generation: self.generation,
            revision: self.revision,
            center,
            removed: removed.into_iter().collect(),
        })
    }

    /// First occupied grid cell on a movement segment. Boundary crossings are
    /// compared as integer fractions so thin corner crossings are not skipped.
    pub fn segment_contact(
        &self,
        from: Point,
        to: Point,
    ) -> Result<Option<(Point, CellId)>, &'static str> {
        let start = [i64::from(from.x), i64::from(from.y), i64::from(from.z)];
        let delta = [
            i64::from(to.x) - start[0],
            i64::from(to.y) - start[1],
            i64::from(to.z) - start[2],
        ];
        let mut first: Option<(i64, i64, CellId)> = None;
        for volume in self.volumes.iter() {
            let grid = volume.grid;
            let size = i64::from(grid.cell_size);
            let origin = [
                i64::from(grid.origin.x),
                i64::from(grid.origin.y),
                i64::from(grid.origin.z),
            ];
            let mut index = [0_i64; 3];
            let mut distance = [0_i64; 3];
            for axis in 0..3 {
                index[axis] = (start[axis] - origin[axis]).div_euclid(size);
                let lower = origin[axis] + index[axis] * size;
                distance[axis] = if delta[axis] > 0 {
                    lower + size - start[axis]
                } else {
                    start[axis] - lower
                };
            }
            let mut at = (0_i64, 1_i64);
            let mut steps = 0;
            loop {
                if (0..3)
                    .all(|axis| index[axis] >= 0 && index[axis] < i64::from(grid.dimensions[axis]))
                {
                    let p = [index[0] as u16, index[1] as u16, index[2] as u16];
                    let cell = volume.cell(p);
                    if volume.solid(p) && !self.removed.contains(&cell) {
                        if first.is_none_or(|(n, d, _)| {
                            i128::from(at.0) * i128::from(d) < i128::from(n) * i128::from(at.1)
                        }) {
                            first = Some((at.0, at.1, cell));
                        }
                        break;
                    }
                }
                let mut next: Option<(i64, i64)> = None;
                for axis in 0..3 {
                    if delta[axis] == 0 {
                        continue;
                    }
                    let candidate = (distance[axis], delta[axis].abs());
                    if next.is_none_or(|(n, d)| {
                        i128::from(candidate.0) * i128::from(d)
                            < i128::from(n) * i128::from(candidate.1)
                    }) {
                        next = Some(candidate);
                    }
                }
                let Some(next) = next else {
                    break;
                };
                if next.0 > next.1 {
                    break;
                }
                steps += 1;
                if steps > MAX_SEGMENT_STEPS {
                    return Err("voxel movement segment exceeds limit");
                }
                for axis in 0..3 {
                    if delta[axis] != 0
                        && i128::from(distance[axis]) * i128::from(next.1)
                            == i128::from(next.0) * i128::from(delta[axis].abs())
                    {
                        index[axis] += delta[axis].signum();
                        distance[axis] += size;
                    }
                }
                at = next;
            }
        }
        Ok(first.map(|(n, d, cell)| {
            let at = |axis: usize| {
                (i128::from(start[axis]) + i128::from(delta[axis]) * i128::from(n) / i128::from(d))
                    as i32
            };
            (
                Point {
                    x: at(0),
                    y: at(1),
                    z: at(2),
                },
                cell,
            )
        }))
    }

    /// Sweep the source projectile's horizontal and vertical half extents
    /// against solid building cells. Terrain retains its source material path.
    pub fn building_sweep_contact(
        &self,
        from: Point,
        to: Point,
        half_width: i32,
        half_height: i32,
    ) -> Result<Option<(Point, CellId)>, &'static str> {
        if !(0..=1024).contains(&half_width) || !(0..=1024).contains(&half_height) {
            return Err("voxel projectile extents outside limit");
        }
        let start = [i64::from(from.x), i64::from(from.y), i64::from(from.z)];
        let end = [i64::from(to.x), i64::from(to.y), i64::from(to.z)];
        let delta = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
        let extent = [
            i64::from(half_width),
            i64::from(half_width),
            i64::from(half_height),
        ];
        let mut first: Option<(i64, i64, CellId)> = None;
        for volume in self
            .volumes
            .iter()
            .filter(|v| matches!(v.id, VolumeId::Building(_)))
        {
            let grid = volume.grid;
            let origin = [
                i64::from(grid.origin.x),
                i64::from(grid.origin.y),
                i64::from(grid.origin.z),
            ];
            let size = i64::from(grid.cell_size);
            let mut lo = [0_i64; 3];
            let mut hi = [0_i64; 3];
            for axis in 0..3 {
                let grid_end = origin[axis] + i64::from(grid.dimensions[axis]) * size;
                let segment_lo = start[axis].min(end[axis]) - extent[axis];
                let segment_hi = start[axis].max(end[axis]) + extent[axis];
                if segment_hi < origin[axis] || segment_lo > grid_end {
                    lo[axis] = 1;
                    hi[axis] = 0;
                    continue;
                }
                lo[axis] = (segment_lo - origin[axis])
                    .div_euclid(size)
                    .saturating_sub(1)
                    .max(0);
                hi[axis] = (segment_hi - origin[axis])
                    .div_euclid(size)
                    .min(i64::from(grid.dimensions[axis]) - 1);
            }
            if (0..3).any(|axis| lo[axis] > hi[axis]) {
                continue;
            }
            let candidates = (hi[0] - lo[0] + 1) * (hi[1] - lo[1] + 1) * (hi[2] - lo[2] + 1);
            if candidates > MAX_CELLS as i64 {
                return Err("voxel sweep exceeds candidate limit");
            }
            for z in lo[2]..=hi[2] {
                for y in lo[1]..=hi[1] {
                    for x in lo[0]..=hi[0] {
                        let index = [x as u16, y as u16, z as u16];
                        let cell = volume.cell(index);
                        if !volume.solid(index) || self.removed.contains(&cell) {
                            continue;
                        }
                        let mut entry = (0_i64, 1_i64);
                        let mut exit = (1_i64, 1_i64);
                        let mut hit = true;
                        for axis in 0..3 {
                            let c = [x, y, z][axis];
                            let low = origin[axis] + c * size - extent[axis];
                            let high = origin[axis] + (c + 1) * size + extent[axis];
                            if delta[axis] == 0 {
                                if start[axis] < low || start[axis] > high {
                                    hit = false;
                                    break;
                                }
                                continue;
                            }
                            let (enter, leave, den) = if delta[axis] > 0 {
                                (low - start[axis], high - start[axis], delta[axis])
                            } else {
                                (start[axis] - high, start[axis] - low, -delta[axis])
                            };
                            if i128::from(enter) * i128::from(entry.1)
                                > i128::from(entry.0) * i128::from(den)
                            {
                                entry = (enter, den);
                            }
                            if i128::from(leave) * i128::from(exit.1)
                                < i128::from(exit.0) * i128::from(den)
                            {
                                exit = (leave, den);
                            }
                            if i128::from(entry.0) * i128::from(exit.1)
                                > i128::from(exit.0) * i128::from(entry.1)
                            {
                                hit = false;
                                break;
                            }
                        }
                        if hit
                            && first.is_none_or(|(n, d, _)| {
                                i128::from(entry.0) * i128::from(d)
                                    < i128::from(n) * i128::from(entry.1)
                            })
                        {
                            first = Some((entry.0, entry.1, cell));
                        }
                    }
                }
            }
        }
        Ok(first.map(|(n, d, cell)| {
            let at = |axis: usize| {
                (i128::from(start[axis]) + i128::from(delta[axis]) * i128::from(n) / i128::from(d))
                    as i32
            };
            (
                Point {
                    x: at(0),
                    y: at(1),
                    z: at(2),
                },
                cell,
            )
        }))
    }

    /// Stable receipt for replay verification, independent of insertion order.
    pub fn deterministic_hash(&self) -> u64 {
        self.content_hash
    }

    fn hash_base(volumes: &[Volume]) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut feed = |bytes: &[u8]| {
            for b in bytes {
                hash ^= u64::from(*b);
                hash = hash.wrapping_mul(0x100_0000_01b3);
            }
        };
        feed(b"magiccarpet-voxel-edits-v1\0");
        for volume in volumes {
            match volume.id {
                VolumeId::Terrain => feed(&[0]),
                VolumeId::Building(id) => {
                    feed(&[1]);
                    feed(&id.to_le_bytes());
                }
            }
            for coordinate in [
                volume.grid.origin.x,
                volume.grid.origin.y,
                volume.grid.origin.z,
            ] {
                feed(&coordinate.to_le_bytes());
            }
            feed(&volume.grid.cell_size.to_le_bytes());
            for dimension in volume.grid.dimensions {
                feed(&dimension.to_le_bytes());
            }
            match &volume.base {
                Base::Terrain(columns) => {
                    for height in columns {
                        feed(&height.to_le_bytes());
                    }
                }
                Base::Building(bits) => {
                    for word in bits {
                        feed(&word.to_le_bytes());
                    }
                }
            }
        }
        hash
    }

    fn hash_contents(&self) -> u64 {
        let mut hash = self.base_hash;
        let mut feed = |bytes: &[u8]| {
            for b in bytes {
                hash ^= u64::from(*b);
                hash = hash.wrapping_mul(0x100_0000_01b3);
            }
        };
        for cell in self.removed.iter() {
            match cell.volume {
                VolumeId::Terrain => feed(&[0]),
                VolumeId::Building(id) => {
                    feed(&[1]);
                    feed(&id.to_le_bytes());
                }
            }
            feed(&cell.x.to_le_bytes());
            feed(&cell.y.to_le_bytes());
            feed(&cell.z.to_le_bytes());
        }
        hash
    }
}

/// A source house and its validated BUILD footprint. Positions and footprint
/// coordinates use the native MC2 map convention, not presentation axes.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HouseSeed {
    pub slot: u16,
    pub build_index: u8,
    pub x: u16,
    pub y: u16,
    pub width: u8,
    pub height: u8,
    pub cells: Vec<[u8; 2]>,
}

/// Reconstructible opt-in seed. `modern_heights` contains the unedited Modern
/// terrain sampler at the 17x21 two-unit extrema probes followed by the
/// 256x320 cell-center probes as IEEE-754 bits, in row-major order. These are needed only for
/// the three-unit patch seam: the patch interior uses the unstamped source
/// plane. Capturing the inputs in replay prevents presentation timing from
/// changing the authoritative initial surface.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoxelWorldSeed {
    pub height_plane: Vec<u8>,
    pub houses: Vec<HouseSeed>,
    pub selected_slot: u16,
    pub modern_heights: Vec<u32>,
}

impl VoxelWorldSeed {
    /// Derive the BUILD identities and footprint bytes from the validated
    /// owner package inspection. The presentation supplies only seam heights;
    /// current source terrain and house identities remain native authority.
    pub fn from_inspection(
        inspection: &crate::native_world::NativeWorldInspection,
        current_height_plane: &[u8],
        selected_slot: u16,
        modern_heights: Vec<u32>,
    ) -> Result<Self, &'static str> {
        let mut houses = Vec::new();
        for entity in inspection.entities().iter().filter(|row| {
            row.active
                && (row.class, row.model, row.action) == (10, 45, 52)
                && matches!(row.level_gate as u8, 26 | 29 | 30 | 42)
        }) {
            let selector = entity.level_gate as u8;
            let footprint = inspection
                .house_build_footprint(selector)
                .ok_or("validated owner package lacks a house BUILD footprint")?;
            houses.push(HouseSeed {
                slot: entity.index,
                build_index: selector,
                x: entity.position.x,
                y: entity.position.y,
                width: footprint.width,
                height: footprint.height,
                cells: footprint.cells().to_vec(),
            });
        }
        let seed = Self {
            height_plane: current_height_plane.to_vec(),
            houses,
            selected_slot,
            modern_heights,
        };
        // Validate before returning any seed that can be recorded into replay.
        seed.build_world(0)?;
        Ok(seed)
    }

    /// Validate and derive the accepted Level 1 patch and its selected house.
    /// All BUILD stamps are checked before the private height copy is changed.
    pub fn build_world(&self, generation: u64) -> Result<VoxelWorld, &'static str> {
        use std::collections::{BTreeMap, BTreeSet};
        const PROBES: usize = 17 * 21;
        const COLUMNS: usize = 256 * 320;
        if self.height_plane.len() != 65_536
            || self.houses.is_empty()
            || self.houses.len() > 128
            || self.modern_heights.len() != PROBES + COLUMNS
            || self
                .modern_heights
                .iter()
                .map(|v| f32::from_bits(*v))
                .any(|v| !v.is_finite() || v < -512.0 || v > 511.0)
        {
            return Err("invalid voxel world seed extent or terrain sample");
        }
        let mut patches = BTreeMap::new();
        let mut preserved = BTreeSet::new();
        let mut slots = BTreeSet::new();
        let mut selected = None;
        for house in &self.houses {
            if !slots.insert(house.slot)
                || house.width == 0
                || house.height == 0
                || house.cells.len() != usize::from(house.width) * usize::from(house.height)
                || house.cells.len() > 4096
            {
                return Err("invalid voxel BUILD footprint");
            }
            let w = i32::from(house.width);
            let h = i32::from(house.height);
            let sx = ((i32::from(house.x) + 128) >> 8) - w / 2;
            let sy = ((i32::from(house.y) + 128) >> 8) - h / 2;
            let mut base = None;
            let mut visible = Vec::new();
            for j in 0..h {
                for i in 0..w {
                    let [material, height] = house.cells[(j * w + i) as usize];
                    let index = (((sy + j) & 255) * 256 + ((sx + i) & 255)) as usize;
                    if (16..255).contains(&material) {
                        visible.push((i, j));
                    }
                    if height == 0 || height == 255 {
                        preserved.insert(index);
                        continue;
                    }
                    let actual = self.height_plane[index];
                    let candidate = actual
                        .checked_sub(height)
                        .ok_or("BUILD stamp wraps its base")?;
                    if base.is_some_and(|value| value != candidate) {
                        return Err("inconsistent BUILD stamp base");
                    }
                    base = Some(candidate);
                    if patches.insert(index, (candidate, house.slot)).is_some() {
                        return Err("overlapping BUILD stamps");
                    }
                }
            }
            let base = base.ok_or("empty BUILD stamp")?;
            if visible.is_empty() {
                return Err("empty visible BUILD footprint");
            }
            if house.slot == self.selected_slot {
                let minx = visible.iter().map(|v| v.0).min().unwrap();
                let maxx = visible.iter().map(|v| v.0).max().unwrap();
                let miny = visible.iter().map(|v| v.1).min().unwrap();
                let maxy = visible.iter().map(|v| v.1).max().unwrap();
                selected = Some((house, sx, sy, base, minx, maxx, miny, maxy));
            }
        }
        if patches.keys().any(|index| preserved.contains(index)) {
            return Err("BUILD stamp overlaps preserved cells");
        }
        let (house, sx, sy, base, minx, maxx, miny, maxy) =
            selected.ok_or("selected BUILD house missing")?;
        if base == 0 {
            return Err("selected BUILD house is not on dry ground");
        }
        let columns = (maxx - minx + 2) as usize;
        let rows = (maxy - miny + 2) as usize;
        if !(3..=33).contains(&columns) || !(3..=33).contains(&rows) {
            return Err("voxel BUILD profile extent exceeds limit");
        }
        let mut profile = Vec::with_capacity(columns * rows);
        for j in miny..=maxy + 1 {
            for i in minx..=maxx + 1 {
                let value = if i < i32::from(house.width) && j < i32::from(house.height) {
                    house.cells[(j * i32::from(house.width) + i) as usize][1]
                } else {
                    0
                };
                profile.push(if value == 255 { 0 } else { value });
            }
        }
        let max_profile = *profile.iter().max().ok_or("empty BUILD profile")?;
        if max_profile == 0 {
            return Err("empty BUILD profile");
        }
        // Profile cells are 4 presentation units wide at accepted scale 2.
        let width = (columns - 1) as f32 * 4.0;
        let depth = (rows - 1) as f32 * 4.0;
        let home_x =
            (((sx as f32 + (minx + maxx + 1) as f32 / 2.0) * 4.0) % 1024.0 + 1024.0) % 1024.0;
        let home_y =
            (((sy as f32 + (miny + maxy + 1) as f32 / 2.0) * 4.0) % 1024.0 + 1024.0) % 1024.0;
        let min_x = home_x - 16.0;
        let min_y = home_y - 20.0;
        if min_x < 0.0 || min_y < 0.0 || min_x + 32.0 > 1024.0 || min_y + 40.0 > 1024.0 {
            return Err("voxel patch crosses map wrap");
        }
        // A second house cannot be stripped within the patch or its one-tile
        // interpolation halo until it also has a physical voxel volume.
        for (&index, &(_, slot)) in &patches {
            let x = (index % 256) as f32 * 4.0;
            let y = (index / 256) as f32 * 4.0;
            if slot != house.slot
                && x >= min_x - 4.0
                && x <= min_x + 36.0
                && y >= min_y - 4.0
                && y <= min_y + 44.0
            {
                return Err("another BUILD stamp intersects the voxel patch");
            }
        }
        let mut plane = self.height_plane.clone();
        for (index, (height, _)) in patches {
            plane[index] = height;
        }
        let source_height = |x: f32, y: f32| -> f32 {
            let mut u = x / 4.0;
            let mut v = y / 4.0;
            let ix = u.floor() as i32;
            let iy = v.floor() as i32;
            u -= ix as f32;
            v -= iy as f32;
            let sample =
                |x: i32, y: i32| f32::from(plane[(((y & 255) * 256) + (x & 255)) as usize]) * 0.5;
            let a = sample(ix, iy);
            let b = sample(ix + 1, iy);
            let c = sample(ix, iy + 1);
            let d = sample(ix + 1, iy + 1);
            if (ix + iy) & 1 == 0 {
                if u < v {
                    a + (d - c) * u + (c - a) * v
                } else {
                    a + (b - a) * u + (d - b) * v
                }
            } else if u + v < 1.0 {
                a + (b - a) * u + (c - a) * v
            } else {
                d + (c - d) * (1.0 - u) + (b - d) * (1.0 - v)
            }
        };
        let base_height = |x: f32, y: f32, modern: f32| -> f32 {
            let edge = (x - min_x)
                .min(min_x + 32.0 - x)
                .min(y - min_y)
                .min(min_y + 40.0 - y);
            let t = (edge / 3.0).clamp(0.0, 1.0);
            let smooth = t * t * (3.0 - 2.0 * t);
            modern + (source_height(x, y) - modern) * smooth
        };
        let mut low = f32::INFINITY;
        let mut high = f32::NEG_INFINITY;
        for j in 0..21 {
            for i in 0..17 {
                let height = base_height(
                    min_x + i as f32 * 2.0,
                    min_y + j as f32 * 2.0,
                    f32::from_bits(self.modern_heights[j * 17 + i]),
                );
                low = low.min(height);
                high = high.max(height);
            }
        }
        low = ((low - 3.0) / 0.125).floor() * 0.125;
        let nz = ((high - low) / 0.125).ceil() as i32 + 1;
        if !(1..=u16::MAX as i32).contains(&nz)
            || low * 64.0 < -32768.0
            || (low + nz as f32 * 0.125) * 64.0 > 32767.0
        {
            return Err("voxel terrain elevation exceeds native bounds");
        }
        let terrain_grid = Grid {
            origin: Point {
                x: (min_x * 64.0) as i32,
                y: (min_y * 64.0) as i32,
                z: (low * 64.0) as i32,
            },
            cell_size: 8,
            dimensions: [256, 320, nz as u16],
        };
        let mut heights = Vec::with_capacity(COLUMNS);
        for j in 0..320 {
            for i in 0..256 {
                let x = min_x + (i as f32 + 0.5) * 0.125;
                let y = min_y + (j as f32 + 0.5) * 0.125;
                let height = base_height(
                    x,
                    y,
                    f32::from_bits(self.modern_heights[PROBES + j * 256 + i]),
                );
                heights.push(((height - low) / 0.125).floor().clamp(0.0, nz as f32) as u16);
            }
        }
        let terrain = Volume::terrain(terrain_grid, heights)?;
        let house_grid = Grid {
            origin: Point {
                x: ((home_x - width / 2.0) * 64.0) as i32,
                y: ((home_y - depth / 2.0) * 64.0) as i32,
                z: i32::from(base) * 32,
            },
            cell_size: 16,
            dimensions: [
                (width / 0.25).round() as u16,
                (depth / 0.25).round() as u16,
                (f32::from(max_profile) * 0.5 / 0.25).ceil() as u16,
            ],
        };
        if house_grid.validate()? > MAX_CELLS {
            return Err("voxel building exceeds cell limit");
        }
        let parity = (sx + sy + minx + miny) & 1;
        let top = |x: f32, y: f32| -> f32 {
            let mut u = x / 4.0;
            let mut v = y / 4.0;
            let ix = (u as usize).min(columns - 2);
            let iy = (v as usize).min(rows - 2);
            u -= ix as f32;
            v -= iy as f32;
            let a = f32::from(profile[ix + iy * columns]) * 0.5;
            let b = f32::from(profile[ix + 1 + iy * columns]) * 0.5;
            let c = f32::from(profile[ix + (iy + 1) * columns]) * 0.5;
            let d = f32::from(profile[ix + 1 + (iy + 1) * columns]) * 0.5;
            if ((ix + iy + parity as usize) & 1) == 0 {
                if u < v {
                    a + (d - c) * u + (c - a) * v
                } else {
                    a + (b - a) * u + (d - b) * v
                }
            } else if u + v < 1.0 {
                a + (b - a) * u + (c - a) * v
            } else {
                d + (c - d) * (1.0 - u) + (b - d) * (1.0 - v)
            }
        };
        let mut solids = Vec::new();
        for y in 0..house_grid.dimensions[1] {
            for x in 0..house_grid.dimensions[0] {
                let xx = (f32::from(x) + 0.5) * 0.25;
                let yy = (f32::from(y) + 0.5) * 0.25;
                let roof = top(xx, yy);
                for z in 0..house_grid.dimensions[2] {
                    let zz = (f32::from(z) + 0.5) * 0.25;
                    if zz > roof || (zz < roof - 3.2 && zz > 0.25) {
                        continue;
                    }
                    let door = (xx - width * 0.5).abs() < 1.3 && yy < 4.4 && zz < 4.4;
                    let window = ((yy - depth * 0.5 - 4.0).abs() < 1.1
                        || (yy - depth * 0.5 + 4.0).abs() < 1.1)
                        && zz > 4.4
                        && zz < 6.2
                        && (xx < 4.0 || xx > width - 4.0);
                    if !door && !window {
                        solids.push([x, y, z]);
                    }
                }
            }
        }
        let building = Volume::building(house.slot, house_grid, solids)?;
        VoxelWorld::new(generation, vec![terrain, building])
    }
}
