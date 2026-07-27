//! Live source-ordered map-cell chains for the bounded Level 1 Goat family.
//!
//! The original renderer reads one entity-list head per terrain cell and
//! follows `oldMapEntity`. Moving an entity to another cell unlinks it and
//! inserts it at the new head. This compact model retains only Goat members,
//! while preserving their relative order through non-Goat links.

pub const SOURCE_MAP_CELLS: usize = 256 * 256;
const SOURCE_ENTITY_SLOTS: usize = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GoatMapSeed {
    pub entity_index: u16,
    pub active: bool,
    pub x: u16,
    pub y: u16,
}

impl GoatMapSeed {
    #[must_use]
    pub const fn tile_index(self) -> u16 {
        let tile_x = self.x.to_le_bytes()[1];
        let tile_y = self.y.to_le_bytes()[1];
        u16::from_le_bytes([tile_x, tile_y])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoatMapChains {
    entity_indices: Box<[u16]>,
    heads: Box<[u16]>,
    next: Box<[u16]>,
    tiles: Box<[u16]>,
    linked: Box<[bool]>,
}

impl GoatMapChains {
    /// Decode the Goat-only view of the source map chains.
    ///
    /// `old_map_entity` must return the raw zero-terminated source link for
    /// every active entity reachable from `terrain_heads`.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed extents, duplicate/zero Goat IDs,
    /// invalid heads/links, cycles, duplicate membership, missing active
    /// Goats, or a Goat linked under a tile that disagrees with its position.
    pub fn from_reset(
        seeds: &[GoatMapSeed],
        terrain_heads: &[i16],
        mut old_map_entity: impl FnMut(u16) -> Option<u16>,
    ) -> Result<Self, String> {
        if terrain_heads.len() != SOURCE_MAP_CELLS {
            return Err(format!(
                "Goat map chain requires {SOURCE_MAP_CELLS} terrain heads, got {}",
                terrain_heads.len()
            ));
        }
        validate_seed_ids(seeds)?;
        let entity_indices = seeds
            .iter()
            .map(|seed| seed.entity_index)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let mut result = Self {
            entity_indices,
            heads: vec![0; SOURCE_MAP_CELLS].into_boxed_slice(),
            next: vec![0; seeds.len()].into_boxed_slice(),
            tiles: seeds
                .iter()
                .map(|seed| seed.tile_index())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            linked: vec![false; seeds.len()].into_boxed_slice(),
        };
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
            let mut previous_goat = None;
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
                if let Some(goat_ordinal) = result.ordinal(source_entity) {
                    if result.linked[goat_ordinal] {
                        return Err(format!(
                            "Goat entity {source_entity} occurs in multiple terrain chains"
                        ));
                    }
                    let tile_u16 = u16::try_from(tile).expect("source tile index fits u16");
                    if result.tiles[goat_ordinal] != tile_u16 {
                        return Err(format!(
                            "Goat entity {source_entity} is linked under tile {tile_u16:#06x}, position requires {:#06x}",
                            result.tiles[goat_ordinal]
                        ));
                    }
                    result.linked[goat_ordinal] = true;
                    if let Some(previous_ordinal) = previous_goat {
                        result.next[previous_ordinal] = source_entity;
                    } else {
                        result.heads[tile] = source_entity;
                    }
                    previous_goat = Some(goat_ordinal);
                }
                source_entity = old_map_entity(source_entity).ok_or_else(|| {
                    format!("entity {source_entity} has no exported oldMapEntity link")
                })?;
            }
        }

        for (ordinal, seed) in seeds.iter().enumerate() {
            if seed.active && !result.linked[ordinal] {
                return Err(format!(
                    "active Goat entity {} is missing from the reset map chains",
                    seed.entity_index
                ));
            }
        }
        result.validate()?;
        Ok(result)
    }

    /// Construct a deterministic fallback used only by synthetic unit worlds
    /// that do not carry reset terrain heads.
    ///
    /// Insertion follows source head-insertion semantics in seed order.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate or zero Goat IDs.
    pub fn from_positions(seeds: &[GoatMapSeed]) -> Result<Self, String> {
        validate_seed_ids(seeds)?;
        let mut result = Self {
            entity_indices: seeds
                .iter()
                .map(|seed| seed.entity_index)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            heads: vec![0; SOURCE_MAP_CELLS].into_boxed_slice(),
            next: vec![0; seeds.len()].into_boxed_slice(),
            tiles: seeds
                .iter()
                .map(|seed| seed.tile_index())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            linked: vec![false; seeds.len()].into_boxed_slice(),
        };
        for (ordinal, seed) in seeds.iter().copied().enumerate() {
            if seed.active {
                result.insert_at_head(ordinal, seed.tile_index());
            }
        }
        result.validate()?;
        Ok(result)
    }

    /// Synchronize chain membership after live Goat movement or removal.
    ///
    /// # Errors
    ///
    /// Returns an error if the seed identity/order differs from construction
    /// or an existing chain is corrupt.
    pub fn sync(&mut self, seeds: &[GoatMapSeed]) -> Result<(), String> {
        if seeds.len() != self.entity_indices.len() {
            return Err(format!(
                "Goat map sync count changed from {} to {}",
                self.entity_indices.len(),
                seeds.len()
            ));
        }
        for (ordinal, seed) in seeds.iter().copied().enumerate() {
            if seed.entity_index != self.entity_indices[ordinal] {
                return Err(format!(
                    "Goat map sync identity changed at ordinal {ordinal}: {} != {}",
                    seed.entity_index, self.entity_indices[ordinal]
                ));
            }
            let new_tile = seed.tile_index();
            if self.linked[ordinal] && (!seed.active || new_tile != self.tiles[ordinal]) {
                self.unlink(ordinal)?;
            }
            self.tiles[ordinal] = new_tile;
            if seed.active && !self.linked[ordinal] {
                self.insert_at_head(ordinal, new_tile);
            }
        }
        self.validate()
    }

    #[must_use]
    pub fn entity_chain(&self, tile_x: u8, tile_y: u8) -> Vec<u16> {
        let tile = usize::from(tile_x) + usize::from(tile_y) * 256;
        let mut chain = Vec::new();
        let mut entity = self.heads[tile];
        while entity != 0 {
            chain.push(entity);
            let Some(ordinal) = self.ordinal(entity) else {
                break;
            };
            entity = self.next[ordinal];
            if chain.len() > self.entity_indices.len() {
                break;
            }
        }
        chain
    }

    fn ordinal(&self, entity_index: u16) -> Option<usize> {
        self.entity_indices
            .iter()
            .position(|candidate| *candidate == entity_index)
    }

    fn insert_at_head(&mut self, ordinal: usize, tile: u16) {
        let tile_index = usize::from(tile);
        self.next[ordinal] = self.heads[tile_index];
        self.heads[tile_index] = self.entity_indices[ordinal];
        self.tiles[ordinal] = tile;
        self.linked[ordinal] = true;
    }

    fn unlink(&mut self, ordinal: usize) -> Result<(), String> {
        let entity_index = self.entity_indices[ordinal];
        let tile = usize::from(self.tiles[ordinal]);
        let mut current = self.heads[tile];
        let mut previous = None;
        for _ in 0..=self.entity_indices.len() {
            if current == 0 {
                break;
            }
            let current_ordinal = self
                .ordinal(current)
                .ok_or_else(|| format!("Goat map tile {tile:#06x} links unknown Goat {current}"))?;
            if current == entity_index {
                if let Some(previous_ordinal) = previous {
                    self.next[previous_ordinal] = self.next[current_ordinal];
                } else {
                    self.heads[tile] = self.next[current_ordinal];
                }
                self.next[current_ordinal] = 0;
                self.linked[current_ordinal] = false;
                return Ok(());
            }
            previous = Some(current_ordinal);
            current = self.next[current_ordinal];
        }
        Err(format!(
            "linked Goat entity {entity_index} is absent from tile {tile:#06x}"
        ))
    }

    fn validate(&self) -> Result<(), String> {
        let mut occurrences = vec![0_u8; self.entity_indices.len()];
        for tile in 0..SOURCE_MAP_CELLS {
            let mut entity = self.heads[tile];
            for _ in 0..=self.entity_indices.len() {
                if entity == 0 {
                    break;
                }
                let ordinal = self.ordinal(entity).ok_or_else(|| {
                    format!("Goat map tile {tile:#06x} links unknown Goat {entity}")
                })?;
                if usize::from(self.tiles[ordinal]) != tile {
                    return Err(format!(
                        "Goat entity {entity} chain tile {tile:#06x} disagrees with tracked tile {:#06x}",
                        self.tiles[ordinal]
                    ));
                }
                occurrences[ordinal] = occurrences[ordinal].saturating_add(1);
                if occurrences[ordinal] > 1 {
                    return Err(format!(
                        "Goat entity {entity} occurs in multiple live chains"
                    ));
                }
                entity = self.next[ordinal];
            }
            if entity != 0 {
                return Err(format!("Goat map tile {tile:#06x} contains a cycle"));
            }
        }
        for (ordinal, linked) in self.linked.iter().copied().enumerate() {
            if linked != (occurrences[ordinal] == 1) {
                return Err(format!(
                    "Goat entity {} linked flag disagrees with chain membership",
                    self.entity_indices[ordinal]
                ));
            }
        }
        Ok(())
    }
}

fn validate_seed_ids(seeds: &[GoatMapSeed]) -> Result<(), String> {
    for (ordinal, seed) in seeds.iter().enumerate() {
        if seed.entity_index == 0 {
            return Err(format!(
                "Goat map seed ordinal {ordinal} uses null entity ID"
            ));
        }
        if seeds[..ordinal]
            .iter()
            .any(|prior| prior.entity_index == seed.entity_index)
        {
            return Err(format!(
                "Goat map seed entity {} is duplicated",
                seed.entity_index
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(entity_index: u16, tile_x: u8, tile_y: u8) -> GoatMapSeed {
        GoatMapSeed {
            entity_index,
            active: true,
            x: u16::from(tile_x) << 8,
            y: u16::from(tile_y) << 8,
        }
    }

    #[test]
    fn reset_decode_filters_non_goats_without_changing_goat_order() {
        let seeds = [seed(50, 4, 5), seed(81, 4, 5), seed(90, 9, 7)];
        let mut heads = vec![0_i16; SOURCE_MAP_CELLS];
        heads[4 + 5 * 256] = 50;
        heads[9 + 7 * 256] = 90;
        let links = [(50, 17), (17, 81), (81, 0), (90, 0)];
        let chains = GoatMapChains::from_reset(&seeds, &heads, |entity| {
            links
                .iter()
                .find_map(|(index, next)| (*index == entity).then_some(*next))
        })
        .expect("valid filtered reset chains");
        assert_eq!(chains.entity_chain(4, 5), [50, 81]);
        assert_eq!(chains.entity_chain(9, 7), [90]);
    }

    #[test]
    fn tile_crossing_unlinks_then_inserts_at_new_head() {
        let mut seeds = [seed(50, 4, 5), seed(81, 4, 5), seed(90, 9, 7)];
        let mut chains = GoatMapChains::from_positions(&seeds).expect("fallback chains");
        assert_eq!(chains.entity_chain(4, 5), [81, 50]);
        seeds[0].x = 9 << 8;
        seeds[0].y = 7 << 8;
        chains.sync(&seeds).expect("cross-tile sync");
        assert_eq!(chains.entity_chain(4, 5), [81]);
        assert_eq!(chains.entity_chain(9, 7), [50, 90]);
    }

    #[test]
    fn same_tile_motion_preserves_order_and_inactive_removal_is_exact() {
        let mut seeds = [seed(50, 255, 0), seed(81, 255, 0)];
        let mut chains = GoatMapChains::from_positions(&seeds).expect("fallback chains");
        assert_eq!(chains.entity_chain(255, 0), [81, 50]);
        seeds[0].x = 0xffff;
        seeds[0].y = 0x00ff;
        chains.sync(&seeds).expect("same-tile sync");
        assert_eq!(chains.entity_chain(255, 0), [81, 50]);
        seeds[1].active = false;
        chains.sync(&seeds).expect("inactive removal");
        assert_eq!(chains.entity_chain(255, 0), [50]);
    }

    #[test]
    fn wrapped_tile_crossing_uses_source_high_bytes() {
        let mut seeds = [seed(50, 255, 255)];
        let mut chains = GoatMapChains::from_positions(&seeds).expect("fallback chains");
        seeds[0].x = 0;
        seeds[0].y = 0;
        chains.sync(&seeds).expect("seam crossing");
        assert!(chains.entity_chain(255, 255).is_empty());
        assert_eq!(chains.entity_chain(0, 0), [50]);
    }
}
