//! Source-ordered map-cell chains shared by native Level 1 render families.
//!
//! MC2 stores one zero-terminated entity list per terrain cell. Entities are
//! inserted at the cell head and moving entities are unlinked then inserted
//! at the destination head. This bounded projection retains only families
//! whose native raster path is complete, while preserving their relative
//! order through unsupported source entities at reset.

pub const SOURCE_MAP_CELLS: usize = 256 * 256;
pub const SOURCE_ENTITY_SLOTS: usize = 1_000;

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
    next: Box<[u16]>,
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
        mut old_map_entity: impl FnMut(u16) -> Option<u16>,
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
            let mut previous_renderable = None;
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
                seen_generation[source_ordinal] = generation;
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
                    } else {
                        result.heads[tile] = source_entity;
                    }
                    previous_renderable = Some(source_entity);
                }
                source_entity = old_map_entity(source_entity).ok_or_else(|| {
                    format!("entity {source_entity} has no exported oldMapEntity link")
                })?;
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

    /// Construct the empty dynamic chain used by synthetic worlds.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            heads: vec![0; SOURCE_MAP_CELLS].into_boxed_slice(),
            next: vec![0; SOURCE_ENTITY_SLOTS].into_boxed_slice(),
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

    fn insert_at_head(&mut self, entity_index: u16, tile: u16) {
        let ordinal = usize::from(entity_index);
        let tile_ordinal = usize::from(tile);
        self.next[ordinal] = self.heads[tile_ordinal];
        self.heads[tile_ordinal] = entity_index;
        self.tiles[ordinal] = tile;
        self.linked[ordinal] = true;
    }

    fn unlink(&mut self, entity_index: u16) -> Result<(), String> {
        let ordinal = usize::from(entity_index);
        let tile = usize::from(self.tiles[ordinal]);
        let mut current = self.heads[tile];
        let mut previous = 0_u16;
        for _ in 0..SOURCE_ENTITY_SLOTS {
            if current == 0 {
                break;
            }
            let current_ordinal = usize::from(current);
            if current_ordinal >= SOURCE_ENTITY_SLOTS || !self.tracked[current_ordinal] {
                return Err(format!(
                    "renderable map tile {tile:#06x} links unknown entity {current}"
                ));
            }
            if current == entity_index {
                if previous == 0 {
                    self.heads[tile] = self.next[current_ordinal];
                } else {
                    self.next[usize::from(previous)] = self.next[current_ordinal];
                }
                self.next[current_ordinal] = 0;
                self.linked[current_ordinal] = false;
                return Ok(());
            }
            previous = current;
            current = self.next[current_ordinal];
        }
        Err(format!(
            "linked renderable entity {entity_index} is absent from tile {tile:#06x}"
        ))
    }

    fn validate(&self) -> Result<(), String> {
        let mut occurrences = [0_u8; SOURCE_ENTITY_SLOTS];
        for tile in 0..SOURCE_MAP_CELLS {
            let mut entity = self.heads[tile];
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
                occurrences[ordinal] = occurrences[ordinal].saturating_add(1);
                if occurrences[ordinal] > 1 {
                    return Err(format!(
                        "renderable entity {entity} occurs in multiple live chains"
                    ));
                }
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
