//! Canonical source-slot arena for the campaign runtime.
//!
//! The decoded fixed records and the dynamic economy projections are one
//! allocation/topology domain in remc2. Keeping them in one owner prevents a
//! session-side entity mirror from drifting from the free stack, generation
//! counters, map links, and typed rows that reuse those source slots.

use crate::{
    Position,
    level_definition::CreatureKind,
    native_world::{
        BeeCreatureOrigin, EntitySnapshot, Level2DynamicArenaKind, Level2EconomyState,
        level2_nyphur_source_identity_parts,
    },
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::{Deref, DerefMut};

use super::state::{SourceLink, SourceTopology, digest_topology};

/// A generation-bound identity for a source slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceEntityHandle {
    pub(crate) source_slot: u16,
    pub(crate) generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceEntityView<'a> {
    Imported(&'a EntitySnapshot),
    Dynamic(Level2DynamicArenaKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceCursorRow {
    pub(crate) handle: SourceEntityHandle,
    pub(crate) kind: Level2DynamicArenaKind,
}

/// The production boundary which created one live creature root.
///
/// This is a read-only semantic view over the existing source arena. It does
/// not own mutable creature state and therefore cannot drift from the numeric
/// scheduler, allocator, map chains, or source records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CreatureRootOrigin {
    InitialPackage,
    AuthoredRecord {
        authored_record: u16,
    },
    /// Historical producer provenance. This deliberately is not a live
    /// handle: the producer slot may be reclaimed and reused while the child
    /// still exists.
    MorphSpell {
        holder_slot: u16,
    },
    HouseSpawn {
        house_slot: u16,
    },
    CastleSpawn {
        castle_slot: u16,
    },
    CreatureConversion {
        producer_slot: u16,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CreatureRuntimeRole {
    Root,
    MultipartPart,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CreatureRuntimeIdentity {
    pub(crate) handle: SourceEntityHandle,
    pub(crate) kind: CreatureKind,
    pub(crate) root_slot: u16,
    pub(crate) role: CreatureRuntimeRole,
    pub(crate) origin: CreatureRootOrigin,
    /// Current raw MC2 model selector for a root. Multipart rows use their
    /// root's semantic identity and therefore have no independent model.
    pub(crate) raw_model: Option<u8>,
    pub(crate) living: bool,
}

/// Source-ordered creature roots frozen by a caller for one observation or
/// UpdateEntities pass. Multipart limbs and projectile children remain owned
/// by their roots and are deliberately not counted as additional creatures.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg(test)]
pub(crate) struct CreatureRuntimeCensus {
    entries: Box<[CreatureRuntimeIdentity]>,
}

#[cfg(test)]
impl CreatureRuntimeCensus {
    pub(crate) fn entries(&self) -> &[CreatureRuntimeIdentity] {
        &self.entries
    }

    pub(crate) fn roots_of_kind(
        &self,
        kind: CreatureKind,
    ) -> impl Iterator<Item = CreatureRuntimeIdentity> + '_ {
        self.entries.iter().copied().filter(move |identity| {
            identity.role == CreatureRuntimeRole::Root && identity.kind == kind
        })
    }
}

/// The single owner of imported source records and dynamic slot sidecars.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceArena {
    pub(crate) entities: Box<[EntitySnapshot]>,
    pub(crate) economy: Level2EconomyState,
    pub(crate) effects: crate::world::effects::State,
    replacement_slots: Vec<u16>,
    // Producer provenance only; all mutable creature fields remain in entities.
    raw_creature_origins: BTreeMap<u16, (u32, CreatureKind, CreatureRootOrigin)>,
}

impl SourceArena {
    #[must_use]
    pub(crate) fn new(entities: Box<[EntitySnapshot]>, economy: Level2EconomyState) -> Self {
        let mut arena = Self {
            entities,
            economy,
            effects: crate::world::effects::State::default(),
            replacement_slots: Vec::new(),
            raw_creature_origins: BTreeMap::new(),
        };
        arena.adopt_reset_settlement_sources();
        arena
    }

    /// The effect owner contributes its source-global relations to the same
    /// reset/replay receipt as the arena. Default state preserves old receipts.
    pub(crate) fn reset_hash_with_terrain(
        &self,
        terrain: &crate::native_world::SessionTerrain,
    ) -> String {
        let arena = self.economy.reset_hash_with_terrain(terrain);
        if self.effects == crate::world::effects::State::default()
            && self.replacement_slots.is_empty()
            && self.raw_creature_origins.is_empty()
        {
            return arena;
        }
        let mut hash = Sha256::new();
        hash.update(b"source-arena-effects-v1");
        hash.update(arena.as_bytes());
        hash.update(self.effects.eruption.to_le_bytes());
        hash.update(self.effects.eruption_smoke.to_le_bytes());
        hash.update((self.replacement_slots.len() as u16).to_le_bytes());
        for slot in &self.replacement_slots {
            hash.update(slot.to_le_bytes());
        }
        if !self.raw_creature_origins.is_empty() {
            hash.update(b"raw-creature-origins-v1");
            for (slot, (generation, kind, origin)) in &self.raw_creature_origins {
                hash.update(slot.to_le_bytes());
                hash.update(generation.to_le_bytes());
                hash.update(kind.as_str().as_bytes());
                let (tag, producer) = match origin {
                    CreatureRootOrigin::InitialPackage => (0u8, 0),
                    CreatureRootOrigin::AuthoredRecord { authored_record } => (1, *authored_record),
                    CreatureRootOrigin::MorphSpell { holder_slot } => (2, *holder_slot),
                    CreatureRootOrigin::HouseSpawn { house_slot } => (3, *house_slot),
                    CreatureRootOrigin::CastleSpawn { castle_slot } => (4, *castle_slot),
                    CreatureRootOrigin::CreatureConversion { producer_slot } => (5, *producer_slot),
                };
                hash.update([tag]);
                hash.update(producer.to_le_bytes());
            }
        }
        format!("{:x}", hash.finalize())
    }

    pub(crate) fn bind_raw_creature_origin(
        &mut self,
        handle: SourceEntityHandle,
        kind: CreatureKind,
        origin: CreatureRootOrigin,
    ) -> bool {
        let expected_model = match kind {
            CreatureKind::Spider => 20,
            CreatureKind::Creature22 => 22,
            CreatureKind::Creature24 => 24,
            _ => return false,
        };
        if !matches!(self.resolve(handle), Some(SourceEntityView::Imported(row))
            if row.class == 5 && row.model == expected_model && row.action != 180)
        {
            return false;
        }
        self.raw_creature_origins
            .insert(handle.source_slot, (handle.generation, kind, origin));
        true
    }

    pub(crate) fn clear_raw_creature_origin(&mut self, source_slot: u16) {
        self.raw_creature_origins.remove(&source_slot);
    }

    fn raw_creature_identity(
        &self,
        source: &EntitySnapshot,
        initial: &BTreeMap<u16, CreatureKind>,
    ) -> Option<CreatureRuntimeIdentity> {
        if !source.active || source.class != 5 || !matches!(source.model, 20 | 22 | 24) {
            return None;
        }
        let part = source.model == 22 && source.action == 180;
        let root_slot = if part {
            u16::from_le_bytes([source.record[154], source.record[155]])
        } else {
            source.index
        };
        let root = self.entities.get(usize::from(root_slot))?;
        if !root.active || root.class != 5 || root.model != source.model || root.action == 180 {
            return None;
        }
        if !matches!(
            self.resolve(SourceEntityHandle {
                source_slot: root_slot,
                generation: self.slot_generation(root_slot)
            }),
            Some(SourceEntityView::Imported(_))
        ) {
            return None;
        }
        let expected = match source.model {
            20 => CreatureKind::Spider,
            22 => CreatureKind::Creature22,
            24 => CreatureKind::Creature24,
            _ => return None,
        };
        let origin = match self.raw_creature_origins.get(&root_slot) {
            Some((generation, kind, origin))
                if *generation == self.slot_generation(root_slot) && *kind == expected =>
            {
                *origin
            }
            Some(_) => return None,
            None if initial.get(&root_slot) == Some(&expected)
                && self.slot_generation(root_slot) == 0 =>
            {
                CreatureRootOrigin::InitialPackage
            }
            None => return None,
        };
        Some(CreatureRuntimeIdentity {
            handle: SourceEntityHandle {
                source_slot: source.index,
                generation: self.slot_generation(source.index),
            },
            kind: expected,
            root_slot,
            role: if part {
                CreatureRuntimeRole::MultipartPart
            } else {
                CreatureRuntimeRole::Root
            },
            origin,
            raw_model: (!part).then_some(source.model),
            living: !part && !source.drawing_disabled() && source.life >= 0,
        })
    }

    #[must_use]
    pub(crate) fn entities(&self) -> &[EntitySnapshot] {
        &self.entities
    }

    /// Source 68BF0/68C70 over raw family roots. The caller supplies the
    /// source-frozen order after StageVar preparation; numeric child links
    /// deliberately address the current occupant, just as the source does.
    pub(crate) fn step_raw_creature_proximity(&mut self, frozen: &[u16], player: Position) {
        for &slot in frozen {
            let Some(row) = self
                .entities
                .get_mut(usize::from(slot))
                .filter(|row| row.active && row.class == 5)
            else {
                continue;
            };
            let mut child = u16::from_le_bytes([row.record[56], row.record[57]]);
            let inherited = if row.life < 0 {
                row.record[61] = 250;
                row.record[62] = 0;
                None
            } else if row.record[61] != 0 {
                let prior = row.record[61];
                row.record[61] = prior.wrapping_sub(1);
                Some(prior)
            } else if row.control_flags & 1 != 0 {
                None
            } else if row.record[62] != 0 {
                row.record[62] -= 1;
                None
            } else {
                let dx = i32::from(row.position.x.wrapping_sub(player.x) as i16);
                let dy = i32::from(row.position.y.wrapping_sub(player.y) as i16);
                let distance = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32;
                if distance < 0x0240_0000 {
                    row.record[61] = 16;
                    Some(18)
                } else {
                    None
                }
            };
            if let Some(value) = inherited {
                for _ in 0..self.entities.len() {
                    let Some(row) = self
                        .entities
                        .get_mut(usize::from(child))
                        .filter(|_| child != 0)
                    else {
                        break;
                    };
                    row.record[61] = value;
                    child = u16::from_le_bytes([row.record[56], row.record[57]]);
                }
            }
        }
    }

    /// NewEvent over both canonical carrier domains. Raw controller records
    /// participate in the same lowest-slot fallback as typed arena rows.
    pub(crate) fn allocate_source_slot(&mut self) -> Option<u16> {
        self.allocate_source_slot_notifying(|_, _| {})
    }

    /// Source 49F90 reconstructs the current ordinary free stack in
    /// descending slot order before GenerateEvents consumes the group.
    pub(crate) fn rebuild_source_free_arena(&mut self) {
        self.rebuild_free_arena(&[]);
    }

    /// 49F90 freezes addresses, rather than continuously admitting newborn
    /// rows. The highest stack entry is the lowest captured source address.
    pub(crate) fn rebuild_free_arena(&mut self, reset_free_slots: &[u16]) {
        self.economy.rebuild_free_arena(reset_free_slots);
        let dynamic = self
            .economy
            .live_arena_slots()
            .into_iter()
            .map(|(slot, _)| slot)
            .collect::<BTreeSet<_>>();
        let mut slots = self
            .entities
            .iter()
            .filter(|s| {
                s.index != 0
                    && s.active
                    && s.class != 0
                    && !dynamic.contains(&s.index)
                    && s.control_flags & crate::level1_mission::SOURCE_REPLACEABLE_FLAG != 0
            })
            .map(|s| s.index)
            .collect::<Vec<_>>();
        self.economy
            .for_each_live_replaceable_arena_row(|slot, _| slots.push(slot));
        slots.sort_unstable_by(|a, b| b.cmp(a));
        slots.dedup();
        self.replacement_slots = slots;
    }

    pub(crate) fn rebuild_source_arena_with_terrain(
        &mut self,
        terrain: &mut crate::native_world::SessionTerrain,
        reset_free_slots: &[u16],
    ) -> Result<(), String> {
        let dynamic = self
            .economy
            .live_arena_slots()
            .into_iter()
            .map(|(slot, _)| slot)
            .collect::<BTreeSet<_>>();
        let hidden = self
            .entities
            .iter()
            .filter(|s| {
                s.index != 0 && s.active && s.drawing_disabled() && !dynamic.contains(&s.index)
            })
            .map(|s| (s.index, s.position, s.entity_rng))
            .collect::<Vec<_>>();
        for (slot, position, rng) in hidden {
            crate::world::effects::sync_position(self, terrain, slot, position, false, false)?;
            self.entities[usize::from(slot)].reclaim_source_row();
            self.economy.release_slot(slot, Some(rng));
            self.raw_creature_origins.remove(&slot);
        }
        let mut hidden_typed = crate::native_world::hidden_effect_slots(self);
        // The economy removes these sphere carriers below. Publish their
        // unlink to terrain and neighboring raw records before releasing the
        // slot, while the old map cell and links are still authoritative.
        hidden_typed.extend(
            self.economy
                .spheres()
                .iter()
                .filter(|sphere| {
                    !sphere.active
                        || sphere.control_flags & crate::level1_mission::SOURCE_DISABLED_FLAG != 0
                })
                .map(|sphere| (sphere.source_entity_index, sphere.position)),
        );
        hidden_typed.sort_unstable_by_key(|(slot, _)| *slot);
        for (slot, position) in hidden_typed {
            crate::world::effects::sync_position(self, terrain, slot, position, false, false)?;
        }
        self.economy.reclaim_inactive_slots_at_source_boundary();
        self.rebuild_free_arena(reset_free_slots);
        Ok(())
    }

    /// sub4A1E0 disables the fallback cursor after its authored group scan.
    pub(crate) fn finish_source_group(&mut self) {
        self.replacement_slots.clear();
    }

    pub(crate) fn decrement_replacement_cursor_after_player_contact(&mut self) {
        self.replacement_slots.pop();
    }

    /// Publish the old cell's head in the same allocation transaction. An
    /// unlinked constructor has no later AddEventToMap which could repair it.
    pub(crate) fn allocate_source_slot_with_terrain(
        &mut self,
        terrain: &mut crate::native_world::SessionTerrain,
    ) -> Option<u16> {
        self.allocate_source_slot_notifying(|tile, head| {
            terrain.disaster_parts().0.entity_indices[usize::from(tile)] = head as i16;
        })
    }

    fn allocate_source_slot_notifying(&mut self, mut publish: impl FnMut(u16, u16)) -> Option<u16> {
        if self.economy.free_slot_count() != 0 {
            let slot = self.economy.allocate_slot()?;
            // The free stack owns this address now. A typed constructor can
            // replace its occupant without publishing a raw record; retaining
            // the old active packet would resurrect it when that owner retires.
            self.entities[usize::from(slot)].reclaim_source_row();
            self.raw_creature_origins.remove(&slot);
            return Some(slot);
        }
        let dynamic = self
            .economy
            .live_arena_slots()
            .into_iter()
            .map(|(slot, _)| slot)
            .collect::<BTreeSet<_>>();
        let slot = self.replacement_slots.pop()?;
        let typed = self
            .economy
            .live_arena_slots()
            .into_iter()
            .find_map(|(id, kind)| (id == slot).then_some(kind));
        let tile = self.economy.map_chains.tile_of(slot);
        let neighbors = [
            self.economy.map_chains.old_map_entity(slot),
            self.economy.map_chains.next_entity(slot),
        ];
        if typed.is_none() {
            self.entities[usize::from(slot)].reclaim_source_row();
            self.economy.claim_raw_replacement_slot(slot);
        } else {
            let allocated = self.economy.replace_typed_source_slot(slot, typed?)?;
            debug_assert_eq!(allocated, slot);
        }
        for neighbor in neighbors
            .into_iter()
            .filter(|id| *id != 0 && !dynamic.contains(id))
        {
            let links = (
                self.economy.map_chains.old_map_entity(neighbor),
                self.economy.map_chains.next_entity(neighbor),
            );
            self.entities[usize::from(neighbor)].set_map_links(links.0, links.1);
        }
        publish(tile, self.economy.map_chains.tile_head(tile));
        self.raw_creature_origins.remove(&slot);
        Some(slot)
    }

    /// SetParticleSmoke3B -> SetSmoke4 over the real shared source allocator.
    /// The process-global draw precedes NewEvent and survives allocation failure.
    pub(crate) fn allocate_smoke13(&mut self, position: Position) -> Option<u16> {
        self.economy.advance_global_rng();
        let life = (self.economy.global_rng % 23 + 17) as i16;
        let slot = self.allocate_source_slot()?;
        self.initialize_smoke13_source_slot(slot, position, life)
    }

    pub(crate) fn allocate_smoke13_with_terrain(
        &mut self,
        position: Position,
        terrain: &mut crate::native_world::SessionTerrain,
    ) -> Option<u16> {
        self.economy.advance_global_rng();
        let life = (self.economy.global_rng % 23 + 17) as i16;
        let slot = self.allocate_source_slot_with_terrain(terrain)?;
        self.initialize_smoke13_source_slot(slot, position, life)?;
        crate::world::effects::sync_position(self, terrain, slot, position, true, false)
            .expect("ordinary Smoke13 retains its fixed map slot");
        Some(slot)
    }

    fn initialize_smoke13_source_slot(
        &mut self,
        slot: u16,
        position: Position,
        life: i16,
    ) -> Option<u16> {
        let slot = self
            .economy
            .initialize_model13_smoke_child(slot, position, life)?;
        for neighbor in [
            self.economy.map_chains.old_map_entity(slot),
            self.economy.map_chains.next_entity(slot),
        ] {
            if let Some(row) = self.entities.get_mut(usize::from(neighbor))
                && neighbor != 0
                && row.active
            {
                row.set_map_links(
                    self.economy.map_chains.old_map_entity(neighbor),
                    self.economy.map_chains.next_entity(neighbor),
                );
            }
        }
        Some(slot)
    }

    pub(crate) fn spawn_eruption_projectile(
        &mut self,
        terrain: &mut crate::native_world::SessionTerrain,
        position: Position,
        yaw: i16,
        destination: Position,
        owner: u16,
        holder: u16,
    ) -> Option<u16> {
        let slot = self.allocate_source_slot_with_terrain(terrain)?;
        self.economy.initialize_eruption_projectile(
            slot,
            position,
            yaw,
            destination,
            owner,
            holder,
        )?;
        crate::world::effects::sync_position(self, terrain, slot, position, true, false)
            .expect("ordinary Eruption flyer has a valid fixed map slot");
        Some(slot)
    }

    #[must_use]
    pub(crate) fn topology(&self) -> SourceTopology {
        let links = self
            .entities
            .iter()
            .map(|entity| SourceLink {
                source_slot: entity.index,
                old_map_entity: entity.old_map_entity(),
                next_entity: entity.next_entity(),
                active: entity.active,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        SourceTopology {
            receipt: digest_topology(&links),
            links,
        }
    }

    #[must_use]
    pub(crate) fn handle(&self, source_slot: u16) -> Option<SourceEntityHandle> {
        self.economy
            .live_arena_slots()
            .iter()
            .any(|(slot, _)| *slot == source_slot)
            .then(|| SourceEntityHandle {
                source_slot,
                generation: self.economy.slot_generation(source_slot),
            })
            .or_else(|| {
                self.entities
                    .get(usize::from(source_slot))
                    .filter(|entity| entity.active)
                    .map(|_| SourceEntityHandle {
                        source_slot,
                        generation: self.economy.slot_generation(source_slot),
                    })
            })
    }

    #[must_use]
    pub(crate) fn resolve(&self, handle: SourceEntityHandle) -> Option<SourceEntityView<'_>> {
        if self.economy.slot_generation(handle.source_slot) != handle.generation {
            return None;
        }
        if let Some((_, kind)) = self
            .economy
            .live_arena_slots()
            .into_iter()
            .find(|(slot, _)| *slot == handle.source_slot)
        {
            return Some(SourceEntityView::Dynamic(kind));
        }
        self.entities
            .get(usize::from(handle.source_slot))
            .filter(|entity| entity.active)
            .map(SourceEntityView::Imported)
    }

    /// Rebuild the semantic creature-root view from the authoritative live
    /// carriers. Package selectors are supplied by the immutable MC2 adapter;
    /// mutable identity and generation always come from this arena.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn creature_runtime_census(
        &self,
        initial_bindings: &BTreeMap<u16, CreatureKind>,
        authored_bindings: &BTreeMap<u16, CreatureKind>,
    ) -> CreatureRuntimeCensus {
        let mut slots = initial_bindings.keys().copied().collect::<BTreeSet<_>>();
        slots.extend(
            self.economy
                .authored_class5()
                .iter()
                .map(|source| source.source_entity_index),
        );
        slots.extend(
            self.economy
                .bee_creatures()
                .iter()
                .map(|source| source.source_entity_index),
        );
        slots.extend(
            self.economy
                .house_residents()
                .iter()
                .map(|source| source.source_entity_index),
        );
        slots.extend(
            self.entities
                .iter()
                .filter(|source| {
                    source.active
                        && source.class == 5
                        && (matches!(source.action, 29 | 232)
                            || matches!(source.model, 20 | 22 | 24))
                })
                .map(|source| source.index),
        );
        slots.extend(
            self.economy
                .authored_class5_model3_children()
                .iter()
                .map(|source| source.source_entity_index),
        );
        slots.extend(
            self.economy
                .authored_dragon_parts()
                .iter()
                .map(|source| source.source_entity_index),
        );

        CreatureRuntimeCensus {
            entries: slots
                .into_iter()
                .filter_map(|source_slot| {
                    self.creature_runtime_identity_inner(
                        source_slot,
                        initial_bindings,
                        authored_bindings,
                        true,
                    )
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    /// Resolve one live creature identity without rebuilding the full census.
    /// Dynamic occupants win over a package row that previously used the same
    /// numeric slot; the returned handle binds that identity to its generation.
    #[must_use]
    pub(crate) fn creature_runtime_identity(
        &self,
        source_slot: u16,
        initial_bindings: &BTreeMap<u16, CreatureKind>,
        authored_bindings: &BTreeMap<u16, CreatureKind>,
    ) -> Option<CreatureRuntimeIdentity> {
        self.creature_runtime_identity_inner(source_slot, initial_bindings, authored_bindings, true)
    }

    /// Resolve identity after the scheduler has already selected an imported
    /// source route. Route resolution proves that no dynamic occupant shadows
    /// this numeric slot, so this path avoids rebuilding the dynamic arena.
    #[must_use]
    pub(crate) fn imported_creature_runtime_identity(
        &self,
        source_slot: u16,
        initial_bindings: &BTreeMap<u16, CreatureKind>,
    ) -> Option<CreatureRuntimeIdentity> {
        let source = self.entities.get(usize::from(source_slot))?;
        if !source.active || source.class != 5 {
            return None;
        }
        if matches!(source.model, 20 | 22 | 24) {
            return self.raw_creature_identity(source, initial_bindings);
        }
        let handle = SourceEntityHandle {
            source_slot,
            generation: self.economy.slot_generation(source_slot),
        };
        if source.class == 5
            && source.model == 9
            && self
                .economy
                .converted_skeleton_origins
                .contains_key(&source_slot)
        {
            return self.creature_runtime_identity_inner(
                source_slot,
                initial_bindings,
                &BTreeMap::new(),
                false,
            );
        }
        if source.class == 5
            && matches!(source.action, 29 | 232)
            && !initial_bindings.contains_key(&source_slot)
        {
            let root_slot = source.source_id;
            let &kind = initial_bindings.get(&root_slot)?;
            return Some(CreatureRuntimeIdentity {
                handle,
                kind,
                root_slot,
                role: CreatureRuntimeRole::MultipartPart,
                origin: CreatureRootOrigin::InitialPackage,
                raw_model: None,
                living: false,
            });
        }
        let &kind = initial_bindings.get(&source_slot)?;
        Some(CreatureRuntimeIdentity {
            handle,
            kind,
            root_slot: source_slot,
            role: CreatureRuntimeRole::Root,
            origin: CreatureRootOrigin::InitialPackage,
            raw_model: Some(source.model),
            living: !source.drawing_disabled()
                && source.life >= 0
                && !matches!(source.action, 180 | 232 | 234),
        })
    }

    fn creature_runtime_identity_inner(
        &self,
        source_slot: u16,
        initial_bindings: &BTreeMap<u16, CreatureKind>,
        authored_bindings: &BTreeMap<u16, CreatureKind>,
        include_retained: bool,
    ) -> Option<CreatureRuntimeIdentity> {
        if let Some(identity) = crate::native_world::creatures::creature_15::mc2::runtime_identity(
            self,
            source_slot,
            initial_bindings,
            authored_bindings,
            include_retained,
        ) {
            return Some(identity);
        }
        let dynamic_handle = SourceEntityHandle {
            source_slot,
            generation: self.economy.slot_generation(source_slot),
        };

        if let Some(source) = self.economy.authored_class5().iter().find(|source| {
            (include_retained || source.active) && source.source_entity_index == source_slot
        }) {
            if source.model == crate::native_world::creatures::archer::ARCHER_MODEL {
                return crate::native_world::archer_creature_runtime_identity(
                    self,
                    source_slot,
                    initial_bindings,
                    authored_bindings,
                    include_retained,
                );
            }
            let (kind, origin) = if source.model
                == crate::native_world::level2_class5::firefly::FIREFLY_MODEL
                && source.authored_index
                    == crate::native_world::level2_class5::firefly::IMPORTED_FIREFLY_AUTHORED_INDEX
            {
                let &kind = initial_bindings.get(&source_slot)?;
                if kind != CreatureKind::Firefly {
                    return None;
                }
                (kind, CreatureRootOrigin::InitialPackage)
            } else {
                (
                    *authored_bindings.get(&source.authored_index)?,
                    CreatureRootOrigin::AuthoredRecord {
                        authored_record: source.authored_index,
                    },
                )
            };
            return Some(CreatureRuntimeIdentity {
                handle: dynamic_handle,
                kind,
                root_slot: source_slot,
                role: CreatureRuntimeRole::Root,
                origin,
                raw_model: Some(source.model),
                living: source.active
                    && source.drawing_enabled
                    && source.life >= 0
                    && !matches!(source.action, 180 | 232 | 234),
            });
        }

        if let Some(source) = self.economy.bee_creatures().iter().find(|source| {
            (include_retained || source.active) && source.source_entity_index == source_slot
        }) {
            let origin = match source.origin {
                BeeCreatureOrigin::InitialPackage => CreatureRootOrigin::InitialPackage,
                BeeCreatureOrigin::AuthoredRecord { authored_index } => {
                    if authored_bindings.get(&authored_index) != Some(&CreatureKind::Bee) {
                        return None;
                    }
                    CreatureRootOrigin::AuthoredRecord {
                        authored_record: authored_index,
                    }
                }
                BeeCreatureOrigin::MorphSpell { holder_slot } => {
                    CreatureRootOrigin::MorphSpell { holder_slot }
                }
            };
            return Some(CreatureRuntimeIdentity {
                handle: dynamic_handle,
                kind: CreatureKind::Bee,
                root_slot: source_slot,
                role: CreatureRuntimeRole::Root,
                origin,
                raw_model: Some(source.model),
                living: source.active
                    && source.drawing_enabled
                    && source.life >= 0
                    && !matches!(source.action, 180 | 232 | 234),
            });
        }

        if let Some(source) = self.economy.house_residents().iter().find(|source| {
            (include_retained || source.active) && source.source_entity_index == source_slot
        }) {
            return Some(CreatureRuntimeIdentity {
                handle: dynamic_handle,
                kind: source.semantic_kind,
                root_slot: source_slot,
                role: CreatureRuntimeRole::Root,
                origin: if source.initial_package {
                    CreatureRootOrigin::InitialPackage
                } else {
                    CreatureRootOrigin::HouseSpawn {
                        house_slot: source.parent_house_entity_index,
                    }
                },
                raw_model: Some(source.model),
                living: source.active
                    && source.drawing_enabled
                    && source.life >= 0
                    && !matches!(source.action, 180 | 232 | 234),
            });
        }

        if let Some(source) = self
            .economy
            .authored_class5_model3_children()
            .iter()
            .find(|source| {
                (include_retained || source.active) && source.source_entity_index == source_slot
            })
        {
            let &kind = authored_bindings.get(&source.root_authored_index)?;
            return Some(CreatureRuntimeIdentity {
                handle: dynamic_handle,
                kind,
                root_slot: source.parent_entity_index,
                role: CreatureRuntimeRole::MultipartPart,
                origin: CreatureRootOrigin::AuthoredRecord {
                    authored_record: source.root_authored_index,
                },
                raw_model: None,
                living: false,
            });
        }

        if let Some(source) = self.economy.authored_dragon_parts().iter().find(|source| {
            (include_retained || source.active) && source.source_entity_index == source_slot
        }) {
            let &kind = authored_bindings.get(&source.root_authored_index)?;
            return Some(CreatureRuntimeIdentity {
                handle: dynamic_handle,
                kind,
                root_slot: source.entity_id,
                role: CreatureRuntimeRole::MultipartPart,
                origin: CreatureRootOrigin::AuthoredRecord {
                    authored_record: source.root_authored_index,
                },
                raw_model: None,
                living: false,
            });
        }

        let handle = self.handle(source_slot)?;
        let source = self.entities.get(usize::from(source_slot))?;
        if !source.active
            || source.class != 5
            || !matches!(self.resolve(handle), Some(SourceEntityView::Imported(_)))
        {
            return None;
        }
        if matches!(source.model, 20 | 22 | 24) {
            return self.raw_creature_identity(source, initial_bindings);
        }
        if source.class == 5
            && source.model == 9
            && let Some(&producer_slot) = self.economy.converted_skeleton_origins.get(&source_slot)
        {
            return Some(CreatureRuntimeIdentity {
                handle,
                kind: CreatureKind::Skeleton,
                root_slot: source_slot,
                role: CreatureRuntimeRole::Root,
                origin: CreatureRootOrigin::CreatureConversion { producer_slot },
                raw_model: Some(9),
                living: !source.drawing_disabled() && source.life >= 0,
            });
        }
        if source.class == 5
            && matches!(source.action, 29 | 232)
            && !initial_bindings.contains_key(&source_slot)
        {
            let root_slot = source.source_id;
            let &kind = initial_bindings.get(&root_slot)?;
            return Some(CreatureRuntimeIdentity {
                handle,
                kind,
                root_slot,
                role: CreatureRuntimeRole::MultipartPart,
                origin: CreatureRootOrigin::InitialPackage,
                raw_model: None,
                living: false,
            });
        }
        let &kind = initial_bindings.get(&source_slot)?;
        Some(CreatureRuntimeIdentity {
            handle,
            kind,
            root_slot: source_slot,
            role: CreatureRuntimeRole::Root,
            origin: CreatureRootOrigin::InitialPackage,
            raw_model: Some(source.model),
            living: !source.drawing_disabled()
                && source.life >= 0
                && !matches!(source.action, 180 | 232 | 234),
        })
    }

    /// Resolve the source-width identity stored by Nyphur's target relation
    /// from the authoritative carrier for the live slot. Imported entities and
    /// typed model-39 births share the same identity primitive; inactive or
    /// unrelated dynamic rows do not validate a stale relation.
    #[must_use]
    pub(crate) fn relation_identity(&self, source_slot: u16) -> Option<i16> {
        if let Some(sphere) = self
            .economy
            .spheres()
            .iter()
            .find(|sphere| sphere.active && sphere.source_entity_index == source_slot)
        {
            let source_id = u16::try_from(sphere.id).ok()?;
            return Some(level2_nyphur_source_identity_parts(source_id, 39, 10));
        }
        self.entities
            .get(usize::from(source_slot))
            .filter(|entity| entity.active)
            .and_then(|entity| {
                // Package class-10/model-39 rows seed the typed sphere arena.
                // Once that typed row retires, the imported record must not
                // resurrect it as a valid relation target.
                (entity.class != 10 || entity.model != 39).then(|| {
                    level2_nyphur_source_identity_parts(
                        entity.source_id,
                        entity.model,
                        entity.class,
                    )
                })
            })
    }

    /// Resolve the live position behind a frozen class-5 model-list member.
    /// Dynamic rows own reused slots, so their typed carriers take precedence
    /// over the decoded imported record at the same numeric index.
    #[must_use]
    pub(crate) fn class5_position(&self, source_slot: u16) -> Option<Position> {
        if let Some(source) = self
            .economy
            .castle_defenders()
            .iter()
            .find(|source| source.active && source.source_entity_index == source_slot)
        {
            return Some(source.position);
        }
        self.economy
            .authored_class5()
            .iter()
            .find(|source| source.source_entity_index == source_slot)
            .map(|source| source.position)
            .or_else(|| {
                self.economy
                    .house_residents()
                    .iter()
                    .find(|source| source.source_entity_index == source_slot)
                    .map(|source| source.position)
            })
            .or_else(|| {
                self.economy
                    .bee_creatures()
                    .iter()
                    .find(|source| source.source_entity_index == source_slot)
                    .map(|source| source.position)
            })
            .or_else(|| {
                self.economy
                    .castle_children()
                    .iter()
                    .find(|source| source.source_entity_index == source_slot)
                    .map(|source| source.position)
            })
            .or_else(|| {
                self.entities
                    .get(usize::from(source_slot))
                    .filter(|entity| entity.class == 5)
                    .map(|entity| entity.position)
            })
    }

    #[must_use]
    pub(crate) fn cursor_rows(&self) -> Vec<SourceCursorRow> {
        self.economy
            .live_arena_slots()
            .into_iter()
            .map(|(source_slot, kind)| SourceCursorRow {
                handle: SourceEntityHandle {
                    source_slot,
                    generation: self.economy.slot_generation(source_slot),
                },
                kind,
            })
            .collect()
    }

    /// Rebuild remc2's class-5 model presence from every live carrier in the
    /// shared source arena. The result is frozen for one UpdateEntities pass;
    /// callbacks and the following objective service consume the same census.
    #[must_use]
    pub(crate) fn rebuilt_class5_model_presence(&self) -> [bool; 29] {
        self.rebuilt_class5_model_slots()
            .map(|source_slots| !source_slots.is_empty())
    }

    /// Rebuild remc2's source-ordered class-10/model-45 membership as the
    /// raw `byte_0x46` selectors consumed by the type-9 objective service.
    /// Drawing state is deliberately ignored: a callback may hide a member
    /// after this pass-start list has been built, but reclaim occurs only at
    /// the following UpdateEntities prologue.
    #[must_use]
    pub(crate) fn rebuilt_model45_building_selectors(&self) -> Vec<u8> {
        self.live_model45_buildings()
            .into_iter()
            .map(|(_, selector, _)| selector)
            .collect()
    }

    /// One observational view across imported and dynamically allocated
    /// building carriers. Dynamic slot occupants shadow stale imported rows.
    pub(crate) fn live_model45_buildings(&self) -> Vec<(u16, u8, Position)> {
        let dynamic_slots = self
            .economy
            .live_arena_slots()
            .into_iter()
            .map(|(source_slot, _)| source_slot)
            .collect::<BTreeSet<_>>();
        let mut rows = self
            .entities
            .iter()
            .filter(|entity| {
                entity.active
                    && !dynamic_slots.contains(&entity.index)
                    && entity.class == 10
                    && entity.model == 45
            })
            .map(|entity| (entity.index, entity.record_bytes()[74], entity.position))
            .chain(
                self.economy
                    .authored_model45()
                    .iter()
                    .filter(|source| source.active)
                    .map(|source| {
                        (
                            source.source_entity_index,
                            source.byte_0x46,
                            source.position,
                        )
                    }),
            )
            .collect::<Vec<_>>();
        rows.sort_unstable_by_key(|(source_slot, _, _)| *source_slot);
        rows
    }

    /// Rebuild remc2's source-ordered class-5 model lists from every live
    /// carrier.  Numeric callbacks consume this frozen pre-pass membership;
    /// same-pass births do not time-travel into it.
    #[must_use]
    pub(crate) fn rebuilt_class5_model_slots(&self) -> [Vec<u16>; 29] {
        let dynamic_slots = self
            .economy
            .live_arena_slots()
            .into_iter()
            .map(|(source_slot, _)| source_slot)
            .collect::<BTreeSet<_>>();
        let mut slots: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
        let mut include = |source_slot: u16, model: u8, action: u8, life: i32| {
            // EventsFunctions.cpp's pre-pass class-5 list excludes negative
            // life before action filtering. The visible death callback can
            // remain live without retaining an absence-objective target.
            if life < 0 || matches!(action, 180 | 232 | 234) {
                return;
            }
            if let Some(bucket) = slots.get_mut(usize::from(model)) {
                bucket.push(source_slot);
            }
        };

        for entity in self.entities.iter().filter(|entity| {
            entity.active
                && !dynamic_slots.contains(&entity.index)
                && entity.class == 5
                && !entity.drawing_disabled()
        }) {
            include(entity.index, entity.model, entity.action, entity.life);
        }
        for source in self
            .economy
            .authored_class5()
            .iter()
            .filter(|source| source.active && source.drawing_enabled)
        {
            include(
                source.source_entity_index,
                source.model,
                source.action,
                source.life,
            );
        }
        for source in self
            .economy
            .authored_class5_model3_children()
            .iter()
            .filter(|source| source.active && source.drawing_enabled)
        {
            include(
                source.source_entity_index,
                source.model,
                source.action,
                source.life,
            );
        }
        for source in self
            .economy
            .authored_dragon_parts()
            .iter()
            .filter(|source| source.active && source.drawing_enabled)
        {
            include(
                source.source_entity_index,
                source.model,
                source.action,
                source.life,
            );
        }
        for source in self
            .economy
            .house_residents()
            .iter()
            .filter(|source| source.active && source.drawing_enabled)
        {
            include(
                source.source_entity_index,
                source.model,
                source.action,
                source.life,
            );
        }
        for source in
            self.economy.bee_creatures().iter().filter(|source| {
                source.active && source.drawing_enabled && source.source_class == 5
            })
        {
            include(
                source.source_entity_index,
                source.model,
                source.action,
                i32::from(source.life),
            );
        }
        for source in self.economy.castle_children().iter().filter(|source| {
            source.active && source.source_class == 5 && !source.drawing_disabled()
        }) {
            include(
                source.source_entity_index,
                source.model,
                source.action,
                source.life,
            );
        }
        for source in self
            .economy
            .castle_defenders()
            .iter()
            .filter(|source| source.active && source.drawing_enabled())
        {
            include(source.source_entity_index, 15, source.action, source.life);
        }
        for bucket in &mut slots {
            bucket.sort_unstable();
            bucket.dedup();
        }
        slots
    }
}

impl Deref for SourceArena {
    type Target = Level2EconomyState;

    fn deref(&self) -> &Self::Target {
        &self.economy
    }
}

impl DerefMut for SourceArena {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.economy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PlayerCommand,
        campaign::{CampaignSession, spec::CampaignSelection},
    };
    use std::path::PathBuf;

    fn level_two_session() -> crate::native_world::Level2ResetSession {
        let package = std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
            },
            PathBuf::from,
        );
        CampaignSession::open(package, CampaignSelection::LEVEL2)
            .expect("qualified Level 2 semantic source arena package")
            .level2_compatibility()
            .clone()
    }

    #[test]
    fn authored_raw_creature_identity_counts_roots_not_parts_and_rejects_reuse() {
        let session = level_two_session();
        let mut arena = session.source_arena().clone();
        let terrain = session.session.state.live.terrain.clone();
        let root = arena.allocate_source_slot().unwrap();
        let part = arena.allocate_source_slot().unwrap();
        for (slot, action) in [(root, 176), (part, 180)] {
            let mut row = crate::world::terrain::cave_features::new_event(
                slot,
                arena.new_event_rng(slot),
                Position::default(),
            );
            row.class = 5;
            row.model = 22;
            row.record[67] = 5;
            row.record[68] = 22;
            row.set_source_action(action);
            row.set_life(3000);
            row.record[154..156].copy_from_slice(&root.to_le_bytes());
            arena.entities[usize::from(slot)] = row;
        }
        let handle = arena.handle(root).unwrap();
        let unbound_hash = arena.reset_hash_with_terrain(&terrain);
        let origin = CreatureRootOrigin::AuthoredRecord {
            authored_record: 494,
        };
        assert!(arena.bind_raw_creature_origin(handle, CreatureKind::Creature22, origin));
        let empty = BTreeMap::new();
        let census = arena.creature_runtime_census(&empty, &empty);
        assert_eq!(census.roots_of_kind(CreatureKind::Creature22).count(), 1);
        let identity = arena
            .creature_runtime_identity(part, &empty, &empty)
            .unwrap();
        assert_eq!(identity.root_slot, root);
        assert_eq!(identity.origin, origin);
        assert_eq!(identity.role, CreatureRuntimeRole::MultipartPart);
        assert!(!identity.living);
        let bound_hash = arena.reset_hash_with_terrain(&terrain);
        assert_ne!(bound_hash, unbound_hash);
        assert_eq!(arena.clone().reset_hash_with_terrain(&terrain), bound_hash);
        arena.entities[usize::from(root)].reclaim_source_row();
        arena.economy.release_slot(root, None);
        let reused = arena.allocate_source_slot().unwrap();
        assert_eq!(reused, root);
        assert!(!arena.bind_raw_creature_origin(handle, CreatureKind::Creature22, origin));
        assert!(
            arena
                .creature_runtime_identity(part, &empty, &empty)
                .is_none()
        );
        let mut replacement =
            crate::world::terrain::cave_features::new_event(root, 1, Position::default());
        replacement.class = 5;
        replacement.model = 24;
        replacement.set_source_action(192);
        arena.entities[usize::from(root)] = replacement;
        // A stale immutable initial selector cannot manufacture a new origin.
        let stale = BTreeMap::from([(root, CreatureKind::Creature24)]);
        assert!(
            arena
                .imported_creature_runtime_identity(root, &stale)
                .is_none()
        );
        let fresh = arena.handle(root).unwrap();
        assert!(arena.bind_raw_creature_origin(fresh, CreatureKind::Creature24, origin));
        assert_eq!(
            arena
                .imported_creature_runtime_identity(root, &stale)
                .unwrap()
                .kind,
            CreatureKind::Creature24
        );
    }

    #[test]
    fn replaced_raw_effect_does_not_inherit_the_old_creature_identity() {
        let session = level_two_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.session.state.live.terrain.clone();
        let bindings = session.session.definition.initial_creature_bindings();
        let slot = *bindings
            .iter()
            .find(|(slot, kind)| {
                **kind == CreatureKind::Goat && arena.entities[usize::from(**slot)].class == 5
            })
            .unwrap()
            .0;
        for row in &mut arena.entities {
            row.set_control_flags(
                row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
            );
        }
        arena.entities[usize::from(slot)]
            .set_control_flags(crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
        let position = arena.entities[usize::from(slot)].position;
        arena.rebuild_source_free_arena();
        while arena.economy.free_slot_count() != 0 {
            arena
                .allocate_source_slot()
                .expect("consume one actual ordinary free slot");
        }
        let birth = crate::world::effects::allocate(
            &mut arena,
            &mut terrain,
            sim_definition::adapter::mc2_effect_definition(
                crate::level_definition::EffectKind::Impact,
            ),
            position,
            &session.session.definition.presentation.particles,
        )
        .unwrap()
        .unwrap();
        assert_eq!(birth, slot);
        assert_eq!(
            (
                arena.entities[usize::from(slot)].class,
                arena.entities[usize::from(slot)].model
            ),
            (10, 0)
        );
        assert!(
            arena
                .imported_creature_runtime_identity(slot, bindings)
                .is_none()
        );
        assert!(
            arena
                .creature_runtime_identity(slot, bindings, &BTreeMap::new())
                .is_none()
        );
        // The same initial identity still rejects an actual wrong creature.
        arena.entities[usize::from(slot)].class = 5;
        assert_eq!(
            arena
                .imported_creature_runtime_identity(slot, bindings)
                .unwrap()
                .kind,
            CreatureKind::Goat
        );
        assert_eq!(
            arena
                .imported_creature_runtime_identity(slot, bindings)
                .unwrap()
                .raw_model,
            Some(0)
        );
    }

    #[test]
    fn replacement_unlink_publishes_terrain_head_for_raw_and_typed_effects() {
        for raw in [true, false] {
            let session = level_two_session();
            let mut arena = session.source_arena().clone();
            let mut terrain = session.session.state.live.terrain.clone();
            for row in &mut arena.entities {
                row.set_control_flags(
                    row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
                );
            }
            assert!(
                arena.economy.typed_replacement_candidate().is_none(),
                "fixture has no pre-existing replaceable typed row"
            );
            let tile = (0..=u16::MAX)
                .find(|tile| arena.economy.map_chains.tile_head(*tile) == 0)
                .unwrap();
            let position = Position {
                x: ((tile as u8 as u16) << 8) + 128,
                y: ((tile >> 8) << 8) + 128,
                z: 128,
            };
            let slot = if raw {
                crate::world::effects::allocate(
                    &mut arena,
                    &mut terrain,
                    sim_definition::adapter::mc2_effect_definition(
                        crate::level_definition::EffectKind::Impact,
                    ),
                    position,
                    &session.session.definition.presentation.particles,
                )
                .unwrap()
                .unwrap()
            } else {
                arena.allocate_smoke13(position).unwrap()
            };
            terrain.disaster_parts().0.entity_indices[usize::from(tile)] = slot as i16;
            arena.rebuild_source_free_arena();
            let generation = arena.economy.slot_generation(slot);
            // Reserve the remaining ordinary rows without manufacturing any
            // additional replaceable object. This is an allocator regression,
            // not a simulated full-pool gameplay capture.
            while arena.economy.free_slot_count() > 0 {
                arena.allocate_source_slot().unwrap();
            }
            assert_eq!(
                arena.allocate_source_slot_with_terrain(&mut terrain),
                Some(slot)
            );
            assert_eq!(
                arena.economy.slot_generation(slot),
                generation.wrapping_add(1)
            );
            assert_eq!(arena.economy.free_slot_count(), 0);
            assert_eq!(arena.economy.map_chains.tile_head(tile), 0);
            assert_eq!(
                terrain.planes().entity_indices[usize::from(tile)],
                0,
                "unlinked constructor must observe the old cell already repaired, raw={raw}"
            );
        }
    }

    #[test]
    fn creature_runtime_census_binds_level_one_two_and_three_roots_and_parts() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let cases = [
            (
                manifest_dir.join("../../../generated/mc-bhq8-level1-collision-v5-night"),
                CampaignSelection::LEVEL1,
                vec![(CreatureKind::Goat, 33), (CreatureKind::Villager, 28)],
            ),
            (
                manifest_dir
                    .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2"),
                CampaignSelection::LEVEL2,
                vec![(CreatureKind::Goat, 36), (CreatureKind::Villager, 29)],
            ),
            (
                manifest_dir
                    .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3"),
                CampaignSelection::LEVEL3,
                vec![(CreatureKind::Worm, 5), (CreatureKind::Skeleton, 11)],
            ),
        ];

        for (package, selection, expected_roots) in cases {
            let campaign = CampaignSession::open(package, selection)
                .expect("qualified campaign opens through the shared engine");
            let census = campaign.level2_compatibility().creature_runtime_census();
            assert!(
                census
                    .entries()
                    .windows(2)
                    .all(|pair| { pair[0].handle.source_slot < pair[1].handle.source_slot })
            );
            for (kind, count) in expected_roots {
                assert_eq!(
                    census.roots_of_kind(kind).count(),
                    count,
                    "{selection:?} {kind:?} root count"
                );
            }
            assert!(census.roots_of_kind(CreatureKind::Builder).next().is_none());

            if selection == CampaignSelection::LEVEL3 {
                let worm_parts = census
                    .entries()
                    .iter()
                    .filter(|identity| {
                        identity.kind == CreatureKind::Worm
                            && identity.role == CreatureRuntimeRole::MultipartPart
                    })
                    .count();
                assert_eq!(worm_parts, 80);
                assert!(census.entries().iter().all(|identity| {
                    identity.role != CreatureRuntimeRole::MultipartPart || !identity.living
                }));
            }
        }
    }

    #[test]
    fn cf06_allocation_reclaim_reuse_preserves_source_topology() {
        let mut session = level_two_session();
        let baseline_free = session.economy().free_slot_count();
        let baseline_hash = session.economy().reset_hash();

        let first_slot = session
            .allocate_level2_authored_class14(248)
            .expect("ordinary authored class-14 allocation")
            .source_entity_index;
        let first = session
            .source_arena()
            .handle(first_slot)
            .expect("allocated slot has a generation-bound view");
        assert_eq!(session.economy().free_slot_count(), baseline_free - 1);
        assert!(matches!(
            session.source_arena().resolve(first),
            Some(SourceEntityView::Dynamic(
                Level2DynamicArenaKind::AuthoredClass14
            ))
        ));

        assert!(
            session
                .source_arena_mut()
                .economy
                .deactivate_authored_class14_slot(first_slot)
        );
        session.source_arena_mut().economy.reclaim_inactive_slots();
        assert_eq!(session.economy().free_slot_count(), baseline_free);
        assert!(session.source_arena().resolve(first).is_none());
        session.source_arena_mut().economy.reclaim_inactive_slots();
        assert_eq!(
            session.economy().free_slot_count(),
            baseline_free,
            "a duplicate reclaim must not duplicate the source free-stack row"
        );

        let reused_slot = session
            .allocate_level2_authored_class14(244)
            .expect("following allocation reuses the source free stack")
            .source_entity_index;
        let reused = session
            .source_arena()
            .handle(reused_slot)
            .expect("reused slot has a fresh generation-bound view");
        assert_eq!(reused_slot, first_slot);
        assert_ne!(reused.generation, first.generation);
        assert!(session.source_arena().resolve(first).is_none());
        assert!(session.source_arena().resolve(reused).is_some());
        assert_ne!(session.economy().reset_hash(), baseline_hash);
    }

    #[test]
    fn cf06_earlier_later_slot_and_generation_views_match_source() {
        let mut session = level_two_session();
        let earlier_slot = session
            .allocate_level2_authored_class14(248)
            .expect("first source allocation")
            .source_entity_index;
        let later_slot = session
            .allocate_level2_authored_class14(244)
            .expect("second source allocation")
            .source_entity_index;
        assert!(earlier_slot < later_slot);

        let rows = session.source_arena().cursor_rows();
        assert!(
            rows.windows(2)
                .all(|rows| { rows[0].handle.source_slot < rows[1].handle.source_slot })
        );
        let after_earlier = rows
            .iter()
            .filter(|row| row.handle.source_slot > earlier_slot)
            .map(|row| row.handle.source_slot)
            .collect::<Vec<_>>();
        assert!(after_earlier.contains(&later_slot));
        assert!(!after_earlier.contains(&earlier_slot));

        let live = session
            .source_arena()
            .handle(earlier_slot)
            .expect("earlier slot is live");
        let wrong_generation = SourceEntityHandle {
            source_slot: live.source_slot,
            generation: live.generation.wrapping_add(1),
        };
        assert!(session.source_arena().resolve(wrong_generation).is_none());

        let evidence = session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(!evidence.is_empty());
        let projections = session
            .dispatch_events()
            .iter()
            .map(crate::native_world::Level2DispatchEvent::campaign_projection)
            .collect::<Vec<_>>();
        assert!(
            projections
                .windows(2)
                .all(|rows| { rows[0].0 < rows[1].0 || rows[0].0 == rows[1].0 })
        );
    }

    #[test]
    fn cf06_reset_replay_restores_topology_receipt() {
        let mut session = level_two_session();
        let mut replay = session.clone();
        let initial_campaign_hash = session.campaign_state_hash();
        let initial_arena_hash = session.economy().reset_hash();

        let first_slot = session
            .allocate_level2_authored_class14(248)
            .expect("first authored allocation")
            .source_entity_index;
        let replay_slot = replay
            .allocate_level2_authored_class14(248)
            .expect("replayed authored allocation")
            .source_entity_index;
        assert_eq!(first_slot, replay_slot);
        assert_eq!(
            session.economy().reset_hash(),
            replay.economy().reset_hash()
        );
        assert_eq!(session.campaign_state_hash(), replay.campaign_state_hash());

        session.reset();
        replay.reset();
        assert_eq!(session.campaign_state_hash(), initial_campaign_hash);
        assert_eq!(session.economy().reset_hash(), initial_arena_hash);
        assert_eq!(
            session.economy().reset_hash(),
            replay.economy().reset_hash()
        );

        let second_slot = session
            .allocate_level2_authored_class14(248)
            .expect("post-reset authored allocation")
            .source_entity_index;
        assert_eq!(second_slot, first_slot);
        assert_eq!(
            session
                .source_arena()
                .handle(second_slot)
                .expect("post-reset generation view")
                .generation,
            1
        );
    }
}
