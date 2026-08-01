//! Source-ordered map-cell chains shared by native Level 1 render families.
//!
//! MC2 stores one zero-terminated entity list per terrain cell. Entities are
//! inserted at the cell head and moving entities are unlinked then inserted
//! at the destination head. This bounded projection retains only families
//! whose native raster path is complete, while preserving their relative
//! order through unsupported source entities at reset.

pub const SOURCE_MAP_CELLS: usize = 256 * 256;
pub const SOURCE_ENTITY_SLOTS: usize = 1_000;

/// One source entity reached by the unfiltered reset `oldMapEntity` walk.
///
/// `ordinal` is the zero-based position within the terrain cell. Keeping this
/// census separate from [`RenderableMapChains`] lets presentation
/// qualification prove that excluded middle nodes do not reorder their
/// renderable neighbours.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawMapChainEntry {
    pub tile: u16,
    pub ordinal: u16,
    pub entity_index: u16,
}

/// Traverse every raw reset map chain before presentation-family filtering.
///
/// # Errors
///
/// Rejects malformed extents, invalid identities/links, cycles, and duplicate
/// membership across terrain cells.
pub fn raw_source_map_chain_entries(
    terrain_heads: &[i16],
    mut old_map_entity: impl FnMut(u16) -> Option<u16>,
) -> Result<Vec<RawMapChainEntry>, String> {
    if terrain_heads.len() != SOURCE_MAP_CELLS {
        return Err(format!(
            "raw source map chain requires {SOURCE_MAP_CELLS} terrain heads, got {}",
            terrain_heads.len()
        ));
    }
    let mut entries = Vec::new();
    let mut membership = [false; SOURCE_ENTITY_SLOTS];
    let mut seen_generation = [0_u32; SOURCE_ENTITY_SLOTS];
    let mut generation = 0_u32;
    for (tile, raw_head) in terrain_heads.iter().copied().enumerate() {
        if raw_head <= 0 {
            continue;
        }
        generation = generation.wrapping_add(1);
        if generation == 0 {
            seen_generation.fill(0);
            generation = 1;
        }
        let mut source_entity = raw_head.cast_unsigned();
        let mut ordinal = 0_u16;
        while source_entity != 0 {
            let source_ordinal = usize::from(source_entity);
            if source_ordinal >= SOURCE_ENTITY_SLOTS {
                return Err(format!(
                    "terrain tile {tile} links invalid entity {source_entity}"
                ));
            }
            if seen_generation[source_ordinal] == generation {
                return Err(format!(
                    "terrain tile {tile} contains an entity-chain cycle at {source_entity}"
                ));
            }
            if membership[source_ordinal] {
                return Err(format!(
                    "source entity {source_entity} occurs in multiple terrain chains"
                ));
            }
            seen_generation[source_ordinal] = generation;
            membership[source_ordinal] = true;
            entries.push(RawMapChainEntry {
                tile: u16::try_from(tile).expect("source terrain tile fits u16"),
                ordinal,
                entity_index: source_entity,
            });
            ordinal = ordinal.checked_add(1).ok_or_else(|| {
                format!("terrain tile {tile} contains more than 65,536 source entities")
            })?;
            source_entity = old_map_entity(source_entity).ok_or_else(|| {
                format!("entity {source_entity} has no exported oldMapEntity link")
            })?;
        }
    }
    Ok(entries)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderableMapSeed {
    pub entity_index: u16,
    pub active: bool,
    pub x: u16,
    pub y: u16,
}

impl RenderableMapSeed {
    #[must_use]
    pub const fn tile_index(self) -> u16 {
        u16::from_le_bytes([self.x.to_le_bytes()[1], self.y.to_le_bytes()[1]])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderableMapChains {
    heads: Box<[u16]>,
    /// Source `oldMapEntity`: away from the tile head.
    next: Box<[u16]>,
    /// Source `nextEntity`: toward the tile head.
    previous: Box<[u16]>,
    tiles: Box<[u16]>,
    tracked: Box<[bool]>,
    linked: Box<[bool]>,
}

impl RenderableMapChains {
    /// Filter the raw reset chains to the supplied supported renderables.
    ///
    /// # Errors
    ///
    /// Rejects malformed extents, invalid identities/links, cycles, duplicate
    /// membership, position/chain disagreement, and missing active seeds.
    pub fn from_reset(
        seeds: &[RenderableMapSeed],
        terrain_heads: &[i16],
        old_map_entity: impl FnMut(u16) -> Option<u16>,
    ) -> Result<Self, String> {
        if terrain_heads.len() != SOURCE_MAP_CELLS {
            return Err(format!(
                "renderable map chain requires {SOURCE_MAP_CELLS} terrain heads, got {}",
                terrain_heads.len()
            ));
        }
        validate_seed_ids(seeds)?;
        let mut result = Self::empty();
        for seed in seeds.iter().copied() {
            let ordinal = usize::from(seed.entity_index);
            result.tracked[ordinal] = true;
            result.tiles[ordinal] = seed.tile_index();
        }
        let raw_entries = raw_source_map_chain_entries(terrain_heads, old_map_entity)?;
        let mut cursor = 0;
        while cursor < raw_entries.len() {
            let tile = usize::from(raw_entries[cursor].tile);
            let mut previous_renderable = None;
            while cursor < raw_entries.len() && usize::from(raw_entries[cursor].tile) == tile {
                let source_entity = raw_entries[cursor].entity_index;
                let source_ordinal = usize::from(source_entity);
                if result.tracked[source_ordinal] {
                    if result.linked[source_ordinal] {
                        return Err(format!(
                            "renderable entity {source_entity} occurs in multiple terrain chains"
                        ));
                    }
                    let tile_u16 = u16::try_from(tile).expect("source tile index fits u16");
                    if result.tiles[source_ordinal] != tile_u16 {
                        return Err(format!(
                            "renderable entity {source_entity} is linked under tile {tile_u16:#06x}, position requires {:#06x}",
                            result.tiles[source_ordinal]
                        ));
                    }
                    result.linked[source_ordinal] = true;
                    if let Some(previous) = previous_renderable {
                        result.next[usize::from(previous)] = source_entity;
                        result.previous[source_ordinal] = previous;
                    } else {
                        result.heads[tile] = source_entity;
                    }
                    previous_renderable = Some(source_entity);
                }
                cursor += 1;
            }
        }

        for seed in seeds {
            if seed.active && !result.linked[usize::from(seed.entity_index)] {
                return Err(format!(
                    "active renderable entity {} is missing from reset map chains",
                    seed.entity_index
                ));
            }
        }
        result.validate()?;
        Ok(result)
    }

    /// Bind the complete raw reset topology and verify both source link
    /// directions before gameplay starts.
    pub fn from_raw_reset(
        seeds: &[RenderableMapSeed],
        terrain_heads: &[i16],
        mut old_map_entity: impl FnMut(u16) -> Option<u16>,
        mut next_entity: impl FnMut(u16) -> Option<u16>,
    ) -> Result<Self, String> {
        let old_links = (0..SOURCE_ENTITY_SLOTS)
            .map(|index| old_map_entity(index as u16))
            .collect::<Vec<_>>();
        let result = Self::from_reset(seeds, terrain_heads, |entity_index| {
            old_links[usize::from(entity_index)]
        })?;
        for seed in seeds {
            let ordinal = usize::from(seed.entity_index);
            let expected_old = result.next[ordinal];
            let expected_new = result.previous[ordinal];
            let actual_old = old_links[ordinal].ok_or_else(|| {
                format!(
                    "entity {} has no exported oldMapEntity link",
                    seed.entity_index
                )
            })?;
            let actual_new = next_entity(seed.entity_index).ok_or_else(|| {
                format!(
                    "entity {} has no exported nextEntity link",
                    seed.entity_index
                )
            })?;
            if (actual_old, actual_new) != (expected_old, expected_new) {
                return Err(format!(
                    "raw entity {} neighbor pair ({actual_old}, {actual_new}) disagrees with terrain topology ({expected_old}, {expected_new})",
                    seed.entity_index
                ));
            }
        }
        result.validate()?;
        Ok(result)
    }

    /// Construct the empty dynamic chain used by synthetic worlds.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            heads: vec![0; SOURCE_MAP_CELLS].into_boxed_slice(),
            next: vec![0; SOURCE_ENTITY_SLOTS].into_boxed_slice(),
            previous: vec![0; SOURCE_ENTITY_SLOTS].into_boxed_slice(),
            tiles: vec![0; SOURCE_ENTITY_SLOTS].into_boxed_slice(),
            tracked: vec![false; SOURCE_ENTITY_SLOTS].into_boxed_slice(),
            linked: vec![false; SOURCE_ENTITY_SLOTS].into_boxed_slice(),
        }
    }

    /// Synchronize one source slot immediately after its source-order update.
    ///
    /// A previously unseen active slot is head-inserted. Same-cell motion
    /// keeps its order; tile changes unlink then head-insert. An inactive slot
    /// is unlinked but remains tracked so source-slot reuse can be observed.
    pub fn sync_entity(&mut self, seed: RenderableMapSeed) -> Result<(), String> {
        validate_entity_index(seed.entity_index)?;
        let ordinal = usize::from(seed.entity_index);
        let new_tile = seed.tile_index();
        if !self.tracked[ordinal] {
            self.tracked[ordinal] = true;
            self.tiles[ordinal] = new_tile;
        }
        if self.linked[ordinal] && (!seed.active || self.tiles[ordinal] != new_tile) {
            self.unlink(seed.entity_index)?;
        }
        self.tiles[ordinal] = new_tile;
        if seed.active && !self.linked[ordinal] {
            self.insert_at_head(seed.entity_index, new_tile);
        }
        Ok(())
    }

    /// Remove tracked slots not present in the current supported live set.
    pub fn retain_entities(&mut self, active_entity_indices: &[u16]) -> Result<(), String> {
        let mut active = [false; SOURCE_ENTITY_SLOTS];
        for entity_index in active_entity_indices {
            validate_entity_index(*entity_index)?;
            if active[usize::from(*entity_index)] {
                return Err(format!(
                    "renderable active set duplicates entity {entity_index}"
                ));
            }
            active[usize::from(*entity_index)] = true;
        }
        for entity_index in 1_u16..SOURCE_ENTITY_SLOTS as u16 {
            let ordinal = usize::from(entity_index);
            if self.tracked[ordinal] && !active[ordinal] {
                if self.linked[ordinal] {
                    self.unlink(entity_index)?;
                }
                self.tracked[ordinal] = false;
                self.next[ordinal] = 0;
                self.previous[ordinal] = 0;
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn entity_chain(&self, tile_x: u8, tile_y: u8) -> Vec<u16> {
        let tile = usize::from(tile_x) + usize::from(tile_y) * 256;
        let mut chain = Vec::new();
        let mut entity = self.heads[tile];
        while entity != 0 {
            chain.push(entity);
            entity = self.next[usize::from(entity)];
            if chain.len() >= SOURCE_ENTITY_SLOTS {
                break;
            }
        }
        chain
    }

    #[must_use]
    pub fn tracks(&self, entity_index: u16) -> bool {
        usize::from(entity_index) < SOURCE_ENTITY_SLOTS && self.tracked[usize::from(entity_index)]
    }

    #[must_use]
    pub fn linked_entity_indices(&self) -> Vec<u16> {
        (1_u16..SOURCE_ENTITY_SLOTS as u16)
            .filter(|entity_index| self.linked[usize::from(*entity_index)])
            .collect()
    }

    #[must_use]
    pub fn old_map_entity(&self, entity_index: u16) -> u16 {
        self.next[usize::from(entity_index)]
    }

    #[must_use]
    pub fn next_entity(&self, entity_index: u16) -> u16 {
        self.previous[usize::from(entity_index)]
    }

    #[must_use]
    pub fn tile_head(&self, tile: u16) -> u16 {
        self.heads[usize::from(tile)]
    }

    #[must_use]
    pub fn tile_of(&self, entity_index: u16) -> u16 {
        self.tiles[usize::from(entity_index)]
    }

    #[must_use]
    pub fn is_linked(&self, entity_index: u16) -> bool {
        self.linked[usize::from(entity_index)]
    }

    /// Exact validated `SetMapEntity_57E50` unlink. Validation happens while
    /// binding/resetting the topology, so the source transaction has no
    /// fallible point after allocation.
    pub fn set_map_entity_unlink(&mut self, entity_index: u16) {
        let ordinal = usize::from(entity_index);
        if !self.linked[ordinal] {
            return;
        }
        let newer = self.previous[ordinal];
        let older = self.next[ordinal];
        if newer != 0 {
            self.next[usize::from(newer)] = older;
        } else {
            self.heads[usize::from(self.tiles[ordinal])] = older;
        }
        if older != 0 {
            self.previous[usize::from(older)] = newer;
        }
        self.linked[ordinal] = false;
        self.previous[ordinal] = 0;
        self.next[ordinal] = 0;
    }

    /// Lossless canonical encoding of the complete mutable chain topology.
    ///
    /// Presentation caching must distinguish not only linked membership but
    /// also source painter order.  Encoding every head/link/tile and both
    /// membership flags makes same-tile head reinsertions observable without
    /// exposing mutable implementation storage.
    #[must_use]
    pub fn canonical_topology_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(
            32 + self.heads.len() * 2
                + self.next.len() * 2
                + self.tiles.len() * 2
                + self.tracked.len()
                + self.linked.len(),
        );
        bytes.extend_from_slice(b"mc2-renderable-map-topology-v1\0");
        bytes.extend_from_slice(
            &u32::try_from(self.heads.len())
                .expect("source map head extent fits u32")
                .to_le_bytes(),
        );
        for value in &self.heads {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for values in [&self.next, &self.tiles] {
            bytes.extend_from_slice(
                &u32::try_from(values.len())
                    .expect("source entity extent fits u32")
                    .to_le_bytes(),
            );
            for value in values.iter() {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        for values in [&self.tracked, &self.linked] {
            bytes.extend_from_slice(
                &u32::try_from(values.len())
                    .expect("source entity flag extent fits u32")
                    .to_le_bytes(),
            );
            bytes.extend(values.iter().map(|value| u8::from(*value)));
        }
        bytes
    }

    /// Lossless encoding of the raw mutable map state used by NewEvent and
    /// SetMapEntity conformance. Unlike the established presentation-cache
    /// encoding, this domain includes source `nextEntity` as well as
    /// `oldMapEntity` so either half of a splice changes the digest.
    #[must_use]
    pub fn raw_topology_bytes_v1(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(
            32 + self.heads.len() * 2
                + self.next.len() * 2
                + self.previous.len() * 2
                + self.tiles.len() * 2
                + self.tracked.len()
                + self.linked.len(),
        );
        bytes.extend_from_slice(b"mc2-raw-map-topology-v1\0");
        bytes.extend_from_slice(
            &u32::try_from(self.heads.len())
                .expect("source map head extent fits u32")
                .to_le_bytes(),
        );
        for value in &self.heads {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for values in [&self.next, &self.previous, &self.tiles] {
            bytes.extend_from_slice(
                &u32::try_from(values.len())
                    .expect("source entity extent fits u32")
                    .to_le_bytes(),
            );
            for value in values.iter() {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        for values in [&self.tracked, &self.linked] {
            bytes.extend_from_slice(
                &u32::try_from(values.len())
                    .expect("source entity flag extent fits u32")
                    .to_le_bytes(),
            );
            bytes.extend(values.iter().map(|value| u8::from(*value)));
        }
        bytes
    }

    fn insert_at_head(&mut self, entity_index: u16, tile: u16) {
        let ordinal = usize::from(entity_index);
        let tile_ordinal = usize::from(tile);
        self.next[ordinal] = self.heads[tile_ordinal];
        self.previous[ordinal] = 0;
        if self.heads[tile_ordinal] != 0 {
            self.previous[usize::from(self.heads[tile_ordinal])] = entity_index;
        }
        self.heads[tile_ordinal] = entity_index;
        self.tiles[ordinal] = tile;
        self.linked[ordinal] = true;
    }

    fn unlink(&mut self, entity_index: u16) -> Result<(), String> {
        let ordinal = usize::from(entity_index);
        if !self.linked[ordinal] {
            return Err(format!(
                "linked renderable entity {entity_index} is not marked linked"
            ));
        }
        self.set_map_entity_unlink(entity_index);
        Ok(())
    }

    fn validate(&self) -> Result<(), String> {
        let mut occurrences = [0_u8; SOURCE_ENTITY_SLOTS];
        for tile in 0..SOURCE_MAP_CELLS {
            let mut entity = self.heads[tile];
            let mut previous = 0_u16;
            for _ in 0..SOURCE_ENTITY_SLOTS {
                if entity == 0 {
                    break;
                }
                let ordinal = usize::from(entity);
                if ordinal >= SOURCE_ENTITY_SLOTS || !self.tracked[ordinal] {
                    return Err(format!(
                        "renderable map tile {tile:#06x} links unknown entity {entity}"
                    ));
                }
                if usize::from(self.tiles[ordinal]) != tile {
                    return Err(format!(
                        "renderable entity {entity} chain tile {tile:#06x} disagrees with tracked tile {:#06x}",
                        self.tiles[ordinal]
                    ));
                }
                if self.previous[ordinal] != previous {
                    return Err(format!(
                        "renderable entity {entity} nextEntity {} disagrees with chain predecessor {previous}",
                        self.previous[ordinal]
                    ));
                }
                occurrences[ordinal] = occurrences[ordinal].saturating_add(1);
                if occurrences[ordinal] > 1 {
                    return Err(format!(
                        "renderable entity {entity} occurs in multiple live chains"
                    ));
                }
                previous = entity;
                entity = self.next[ordinal];
            }
            if entity != 0 {
                return Err(format!("renderable map tile {tile:#06x} contains a cycle"));
            }
        }
        for ordinal in 1..SOURCE_ENTITY_SLOTS {
            if self.linked[ordinal] != (occurrences[ordinal] == 1) {
                return Err(format!(
                    "renderable entity {ordinal} linked flag disagrees with membership"
                ));
            }
            if self.linked[ordinal] && !self.tracked[ordinal] {
                return Err(format!("untracked renderable entity {ordinal} is linked"));
            }
            if !self.linked[ordinal] && (self.next[ordinal] != 0 || self.previous[ordinal] != 0) {
                return Err(format!(
                    "unlinked renderable entity {ordinal} retains raw neighbor links"
                ));
            }
        }
        Ok(())
    }
}

fn validate_entity_index(entity_index: u16) -> Result<(), String> {
    if entity_index == 0 || usize::from(entity_index) >= SOURCE_ENTITY_SLOTS {
        return Err(format!(
            "renderable entity index {entity_index} is outside 1..{}",
            SOURCE_ENTITY_SLOTS - 1
        ));
    }
    Ok(())
}

fn validate_seed_ids(seeds: &[RenderableMapSeed]) -> Result<(), String> {
    let mut seen = [false; SOURCE_ENTITY_SLOTS];
    for seed in seeds {
        validate_entity_index(seed.entity_index)?;
        let ordinal = usize::from(seed.entity_index);
        if seen[ordinal] {
            return Err(format!(
                "renderable map seed entity {} is duplicated",
                seed.entity_index
            ));
        }
        seen[ordinal] = true;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(entity_index: u16, tile_x: u8, tile_y: u8) -> RenderableMapSeed {
        RenderableMapSeed {
            entity_index,
            active: true,
            x: u16::from(tile_x) << 8,
            y: u16::from(tile_y) << 8,
        }
    }

    #[test]
    fn reset_filter_preserves_cross_family_order_through_unsupported_links() {
        let seeds = [seed(50, 4, 5), seed(81, 4, 5), seed(90, 4, 5)];
        let mut heads = vec![0_i16; SOURCE_MAP_CELLS];
        heads[4 + 5 * 256] = 50;
        let links = [(50, 17), (17, 81), (81, 22), (22, 90), (90, 0)];
        let chains = RenderableMapChains::from_reset(&seeds, &heads, |entity| {
            links
                .iter()
                .find_map(|(index, next)| (*index == entity).then_some(*next))
        })
        .expect("valid filtered reset chains");
        assert_eq!(chains.entity_chain(4, 5), [50, 81, 90]);
        let raw = raw_source_map_chain_entries(&heads, |entity| {
            links
                .iter()
                .find_map(|(index, next)| (*index == entity).then_some(*next))
        })
        .expect("valid raw reset chains");
        assert_eq!(
            raw,
            [
                RawMapChainEntry {
                    tile: 0x0504,
                    ordinal: 0,
                    entity_index: 50,
                },
                RawMapChainEntry {
                    tile: 0x0504,
                    ordinal: 1,
                    entity_index: 17,
                },
                RawMapChainEntry {
                    tile: 0x0504,
                    ordinal: 2,
                    entity_index: 81,
                },
                RawMapChainEntry {
                    tile: 0x0504,
                    ordinal: 3,
                    entity_index: 22,
                },
                RawMapChainEntry {
                    tile: 0x0504,
                    ordinal: 4,
                    entity_index: 90,
                },
            ],
            "the pre-filter census retains excluded middle nodes and exact source order"
        );
    }

    #[test]
    fn raw_reset_validates_both_neighbor_directions_and_hashes_splices() {
        let seeds = [seed(50, 4, 5), seed(60, 4, 5)];
        let mut heads = vec![0_i16; SOURCE_MAP_CELLS];
        heads[4 + 5 * 256] = 50;
        let old = |entity| match entity {
            50 => Some(60),
            60 => Some(0),
            _ => Some(0),
        };
        let next = |entity| match entity {
            50 => Some(0),
            60 => Some(50),
            _ => Some(0),
        };
        let mut chains = RenderableMapChains::from_raw_reset(&seeds, &heads, old, next)
            .expect("consistent oldMapEntity/nextEntity pair");
        let before = chains.raw_topology_bytes_v1();
        chains.set_map_entity_unlink(50);
        assert_ne!(chains.raw_topology_bytes_v1(), before);
        assert_eq!(chains.entity_chain(4, 5), [60]);
        assert_eq!((chains.next_entity(60), chains.old_map_entity(60)), (0, 0));

        let error = RenderableMapChains::from_raw_reset(&seeds, &heads, old, |entity| {
            (entity == 50 || entity == 60).then_some(0)
        })
        .unwrap_err();
        assert!(error.contains("neighbor pair"), "unexpected error: {error}");
    }

    #[test]
    fn activation_and_cross_tile_motion_use_source_head_insertion() {
        let mut chains = RenderableMapChains::empty();
        chains.sync_entity(seed(50, 4, 5)).unwrap();
        chains.sync_entity(seed(81, 4, 5)).unwrap();
        assert_eq!(chains.entity_chain(4, 5), [81, 50]);
        chains.sync_entity(seed(50, 9, 7)).unwrap();
        assert_eq!(chains.entity_chain(4, 5), [81]);
        assert_eq!(chains.entity_chain(9, 7), [50]);
        chains.sync_entity(seed(90, 9, 7)).unwrap();
        assert_eq!(chains.entity_chain(9, 7), [90, 50]);
    }

    #[test]
    fn same_tile_motion_preserves_order_and_retain_removes_reused_slots() {
        let mut chains = RenderableMapChains::empty();
        chains.sync_entity(seed(50, 255, 0)).unwrap();
        chains.sync_entity(seed(81, 255, 0)).unwrap();
        let mut moved = seed(50, 255, 0);
        moved.x = 0xffff;
        moved.y = 0x00ff;
        chains.sync_entity(moved).unwrap();
        assert_eq!(chains.entity_chain(255, 0), [81, 50]);
        chains.retain_entities(&[50]).unwrap();
        assert_eq!(chains.entity_chain(255, 0), [50]);
        chains.sync_entity(seed(81, 1, 2)).unwrap();
        assert_eq!(chains.entity_chain(1, 2), [81]);
    }
}
