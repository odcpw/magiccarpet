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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceManaRow {
    pub(crate) source_slot: u16,
    pub(crate) position: Position,
    pub(crate) player_entity_index: u16,
    pub(crate) control_flags: u32,
    pub(crate) mana: i32,
    pub(crate) collision_yaw: i16,
    pub(crate) collision_pitch: i16,
    pub(crate) collision_roll: i16,
    pub(crate) collision_fov: i16,
}

/// An UpdateEntities head. Links live in the fixed raw records,
/// including inactive shadows of typed rows. NewEvent resets a reused
/// record's next_0; sub57F20 alone leaves its old successor intact.
#[derive(Clone, Debug, Default)]
pub(crate) struct SourceSlotChain {
    slots: Vec<u16>,
    generations: BTreeMap<u16, u32>,
    epoch: u32,
}

impl SourceSlotChain {
    pub(crate) fn capture(arena: &mut SourceArena, slots: Vec<u16>) -> Self {
        let generations = slots.iter()
            .map(|&slot| (slot, arena.economy.slot_generation(slot)))
            .collect();
        for (index, &slot) in slots.iter().enumerate() {
            let next = slots.get(index + 1).copied().unwrap_or(0);
            let row = &mut arena.entities[usize::from(slot)];
            row.next_relation = Some(next);
            let encoded = row.next_relation.map_or(0, |slot| u32::from(slot) + 1);
            row.record[4..8].copy_from_slice(&encoded.to_le_bytes());
        }
        Self { slots, generations, epoch: arena.economy.source_list_epoch() }
    }

    pub(crate) fn slots(&self) -> &[u16] {
        &self.slots
    }

    pub(crate) fn current_slots(
        &self,
        economy: &Level2EconomyState,
        entities: &[EntitySnapshot],
    ) -> Vec<u16> {
        if self.epoch != economy.source_list_epoch() {
            return Vec::new();
        }
        let mut current = self.slots.first().copied().unwrap_or(0);
        let mut visited = BTreeSet::new();
        let mut rows = Vec::new();
        while current != 0 && visited.insert(current) {
            let Some(raw) = entities.get(usize::from(current)) else { break };
            rows.push(current);
            // NewEvent has already reset next_0 before projection retirement.
            current = if self.generations.get(&current)
                == Some(&economy.slot_generation(current))
            {
                raw.next_relation.unwrap_or(0)
            } else { 0 };
        }
        rows
    }
}

/// The dword38523 list resolves mana payloads from their current owners.
#[derive(Clone, Debug, Default)]
pub(crate) struct SourceManaChain(SourceSlotChain);

impl SourceManaChain {
    pub(crate) fn capture(arena: &mut SourceArena, slots: Vec<u16>) -> Self {
        Self(SourceSlotChain::capture(arena, slots))
    }

    pub(crate) fn slots(&self) -> &[u16] {
        self.0.slots()
    }

    pub(crate) fn rows(
        &self,
        economy: &Level2EconomyState,
        entities: &[EntitySnapshot],
    ) -> Vec<SourceManaRow> {
        let mut rows = Vec::new();
        for current in self.0.current_slots(economy, entities) {
            let raw = &entities[usize::from(current)];
            match economy.live_arena_kind(current) {
                Some(Level2DynamicArenaKind::ManaSphere) => {
                    if let Some(sphere) = economy.spheres().iter()
                        .find(|row| row.active && row.source_entity_index == current)
                    {
                        rows.push(SourceManaRow {
                            source_slot: current, position: sphere.position,
                            player_entity_index: sphere.player_entity_index,
                            control_flags: sphere.control_flags, mana: sphere.mana,
                            collision_yaw: sphere.collision_yaw, collision_pitch: sphere.collision_pitch,
                            collision_roll: sphere.collision_roll, collision_fov: sphere.collision_fov,
                        });
                    }
                }
                Some(_) => {}
                None if raw.model == 39 => rows.push(SourceManaRow {
                    source_slot: current, position: raw.position,
                    player_entity_index: raw.source_player_entity_index(),
                    control_flags: raw.control_flags, mana: raw.mana,
                    collision_yaw: raw.array_yaw(), collision_pitch: raw.array_pitch(),
                    collision_roll: raw.array_roll(), collision_fov: raw.array_fov(),
                }),
                None => {}
            }
        }
        rows
    }
}

fn raw_word(row: &EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
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
    SummonedSpell {
        holder_slot: u16,
    },
    /// Historical field producer, including its original arena generation.
    FieldSummon {
        controller: SourceEntityHandle,
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

/// Two-generation owner binding for raw multipart creature rows. The source
/// records remain authoritative for owner and chain links; this sidecar only
/// prevents a reused child or root slot from inheriting an old identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RawCreaturePartBinding {
    child_generation: u32,
    root_slot: u16,
    root_generation: u32,
    kind: CreatureKind,
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
    /// Source AddEvent2 light pool, retained independently of effect callbacks.
    pub(crate) dynamic_lighting: crate::dynamic_lighting::DynamicLightingState,
    /// Runtime setting consumed at admission, not at later light service.
    pub(crate) dynamic_lighting_enabled: bool,
    replacement_slots: Vec<u16>,
    // Producer provenance only; all mutable creature fields remain in entities.
    raw_creature_origins: BTreeMap<u16, (u32, CreatureKind, CreatureRootOrigin)>,
    raw_creature_parts: BTreeMap<u16, RawCreaturePartBinding>,
    creature_parents: BTreeMap<u16, (u32, SourceEntityHandle)>,
}

impl SourceArena {
    #[must_use]
    pub(crate) fn new(entities: Box<[EntitySnapshot]>, economy: Level2EconomyState) -> Self {
        let mut arena = Self {
            entities,
            economy,
            effects: crate::world::effects::State::default(),
            dynamic_lighting: crate::dynamic_lighting::DynamicLightingState::reset(),
            dynamic_lighting_enabled: true,
            replacement_slots: Vec::new(),
            raw_creature_origins: BTreeMap::new(),
            raw_creature_parts: BTreeMap::new(),
            creature_parents: BTreeMap::new(),
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
            && self.dynamic_lighting == crate::dynamic_lighting::DynamicLightingState::reset()
            && self.dynamic_lighting_enabled
            && self.replacement_slots.is_empty()
            && self.raw_creature_origins.is_empty()
            && self.raw_creature_parts.is_empty()
            && self.creature_parents.is_empty()
        {
            return arena;
        }
        let mut hash = Sha256::new();
        hash.update(b"source-arena-effects-v1");
        hash.update(arena.as_bytes());
        hash.update(self.effects.eruption.to_le_bytes());
        hash.update(self.effects.eruption_smoke.to_le_bytes());
        if !self.effects.death_burst_origins().is_empty() {
            hash.update(b"source-death-burst-provenance-v1");
            hash.update((self.effects.death_burst_origins().len() as u16).to_le_bytes());
            for (&slot, &(generation, parent)) in self.effects.death_burst_origins() {
                hash.update(slot.to_le_bytes());
                hash.update(generation.to_le_bytes());
                hash.update(parent.to_le_bytes());
            }
        }
        if self.dynamic_lighting != crate::dynamic_lighting::DynamicLightingState::reset()
            || !self.dynamic_lighting_enabled
        {
            hash.update(b"dynamic-lighting-v1");
            hash.update(self.dynamic_lighting.hash_v1().to_le_bytes());
            hash.update([u8::from(self.dynamic_lighting_enabled)]);
        }
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
                    CreatureRootOrigin::SummonedSpell { holder_slot } => (6, *holder_slot),
                    CreatureRootOrigin::FieldSummon { controller } => (7, controller.source_slot),
                    CreatureRootOrigin::HouseSpawn { house_slot } => (3, *house_slot),
                    CreatureRootOrigin::CastleSpawn { castle_slot } => (4, *castle_slot),
                    CreatureRootOrigin::CreatureConversion { producer_slot } => (5, *producer_slot),
                };
                hash.update([tag]);
                hash.update(producer.to_le_bytes());
                if let CreatureRootOrigin::FieldSummon { controller } = origin {
                    hash.update(controller.generation.to_le_bytes());
                }
            }
        }
        if !self.raw_creature_parts.is_empty() {
            hash.update(b"raw-creature-parts-v1");
            for (child_slot, binding) in &self.raw_creature_parts {
                hash.update(child_slot.to_le_bytes());
                hash.update(binding.child_generation.to_le_bytes());
                hash.update(binding.root_slot.to_le_bytes());
                hash.update(binding.root_generation.to_le_bytes());
                hash.update(binding.kind.as_str().as_bytes());
            }
        }
        if !self.creature_parents.is_empty() {
            hash.update(b"creature-parents-v1");
            for (slot, (generation, parent)) in &self.creature_parents {
                hash.update(slot.to_le_bytes());
                hash.update(generation.to_le_bytes());
                hash.update(parent.source_slot.to_le_bytes());
                hash.update(parent.generation.to_le_bytes());
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
            CreatureKind::Skeleton => 9,
            CreatureKind::FinalBoss => 10,
            CreatureKind::Wyvern => 16,
            CreatureKind::Creature17 => 17,
            CreatureKind::Creature18 => 18,
            CreatureKind::Spider => 20,
            CreatureKind::Creature21 => 21,
            CreatureKind::Creature22 => 22,
            CreatureKind::Creature23 => 23,
            CreatureKind::Creature24 => 24,
            CreatureKind::Cymmerian => 25,
            CreatureKind::Creature26 => 26,
            CreatureKind::Creature27 => 27,
            CreatureKind::Creature28 => 28,
            _ => return false,
        };
        if !matches!(self.resolve(handle), Some(SourceEntityView::Imported(row))
            if row.class == 5
                && row.model == expected_model
                && row.action != 180
                && (expected_model != 27
                    || ((216..=223).contains(&row.action)
                        && raw_word(row, 54) == 0)))
        {
            return false;
        }
        let binding = (handle.generation, kind, origin);
        if self.raw_creature_origins.get(&handle.source_slot) != Some(&binding) {
            self.clear_raw_creature_part_bindings_for_slot(handle.source_slot);
        }
        self.raw_creature_origins
            .insert(handle.source_slot, binding);
        true
    }

    /// Bind provenance for a live typed model19 Firefly. The typed
    /// `authored_class5` row remains authoritative; this map contributes only
    /// the cast origin and the source-slot generation that created it.
    pub(crate) fn bind_dynamic_creature_origin(
        &mut self,
        handle: SourceEntityHandle,
        kind: CreatureKind,
        origin: CreatureRootOrigin,
    ) -> bool {
        let expected_model = match kind {
            CreatureKind::Firefly => 19,
            CreatureKind::Dragon => 0,
            _ => return false,
        };
        if (kind == CreatureKind::Dragon
            && !matches!(origin, CreatureRootOrigin::CreatureConversion { .. }))
            || !matches!(
                origin,
                CreatureRootOrigin::SummonedSpell { .. }
                    | CreatureRootOrigin::CreatureConversion { .. }
            )
            || !matches!(
                self.resolve(handle),
                Some(SourceEntityView::Dynamic(
                    Level2DynamicArenaKind::AuthoredClass5
                ))
            )
            || !self.economy.authored_class5().iter().any(|source| {
                source.active
                    && source.drawing_enabled
                    && source.source_entity_index == handle.source_slot
                    && source.model == expected_model
                    && source.life >= 0
            })
        {
            return false;
        }
        let binding = (handle.generation, kind, origin);
        if self.raw_creature_origins.get(&handle.source_slot) != Some(&binding) {
            self.clear_raw_creature_part_bindings_for_slot(handle.source_slot);
        }
        self.raw_creature_origins
            .insert(handle.source_slot, binding);
        true
    }

    pub(crate) fn clear_raw_creature_origin(&mut self, source_slot: u16) {
        self.raw_creature_origins.remove(&source_slot);
        self.creature_parents.remove(&source_slot);
        self.clear_raw_creature_part_bindings_for_slot(source_slot);
    }

    /// Bind the live parent address separately from historical producer
    /// provenance. A reused numeric parent slot must never prolong a summon.
    pub(crate) fn bind_creature_parent(
        &mut self,
        child: SourceEntityHandle,
        parent: SourceEntityHandle,
    ) -> bool {
        if child.source_slot == parent.source_slot
            || self.resolve(child).is_none()
            || self.resolve(parent).is_none()
        {
            return false;
        }
        self.creature_parents
            .insert(child.source_slot, (child.generation, parent));
        true
    }

    pub(crate) fn creature_parent(&self, child: SourceEntityHandle) -> Option<SourceEntityHandle> {
        self.creature_parent_binding(child)
            .filter(|parent| self.resolve(*parent).is_some())
    }

    /// Preserve the original caster identity through intermediate flyers and
    /// rings even after that caster's numeric slot has been reclaimed.
    pub(crate) fn creature_parent_binding(
        &self,
        child: SourceEntityHandle,
    ) -> Option<SourceEntityHandle> {
        let &(generation, parent) = self.creature_parents.get(&child.source_slot)?;
        (generation == child.generation && self.resolve(child).is_some()).then_some(parent)
    }

    /// Copy an already-observed parent, including a stale generation. Binding
    /// the current occupant here would let a delayed birth adopt a new caster.
    pub(crate) fn bind_carried_creature_parent(
        &mut self,
        child: SourceEntityHandle,
        parent: SourceEntityHandle,
    ) -> bool {
        if child == parent
            || parent.source_slot == 0
            || usize::from(parent.source_slot) >= self.entities.len()
            || self.resolve(child).is_none()
        {
            return false;
        }
        self.creature_parents
            .insert(child.source_slot, (child.generation, parent));
        true
    }

    pub(crate) fn has_creature_parent_binding(&self, child: SourceEntityHandle) -> bool {
        self.creature_parents
            .get(&child.source_slot)
            .is_some_and(|(generation, _)| {
                *generation == child.generation
                    && self.slot_generation(child.source_slot) == child.generation
            })
    }

    fn clear_raw_creature_part_bindings_for_slot(&mut self, source_slot: u16) {
        self.raw_creature_parts.retain(|child_slot, binding| {
            *child_slot != source_slot && binding.root_slot != source_slot
        });
    }

    /// Bind one source model27 segment to its live root after the constructor
    /// has published the raw56 link. The source owner word at raw54 and the
    /// linked chain stay authoritative; the sidecar records only both slot
    /// generations and the typed root kind.
    pub(crate) fn bind_raw_creature_part(
        &mut self,
        child: SourceEntityHandle,
        root: SourceEntityHandle,
        kind: CreatureKind,
    ) -> bool {
        let killed_chain = matches!(self.resolve(root), Some(SourceEntityView::Imported(row))
            if row.class == 5 && row.model == 27 && row.action == 221);
        if child.source_slot == root.source_slot
            || !matches!(self.resolve(root), Some(SourceEntityView::Imported(row))
                if row.class == 5 && row.model == 27 && (216..=223).contains(&row.action))
            || !matches!(self.resolve(child), Some(SourceEntityView::Imported(row))
                if row.class == 5
                    && row.model == 27
                    && (matches!(row.action, 233 | 234) || (killed_chain && row.action == 221))
                    && row.record[63] < 5
                    && raw_word(row, 54) == root.source_slot
                    && raw_word(row, 30) == root.source_slot
                    && row.source_id == root.source_slot)
            || !matches!(self.raw_creature_origins.get(&root.source_slot),
                Some((generation, root_kind, _))
                    if *generation == root.generation && *root_kind == kind)
            || !self.raw_creature_part_chain_contains(root, child, kind, true)
        {
            return false;
        }
        self.raw_creature_parts.insert(
            child.source_slot,
            RawCreaturePartBinding {
                child_generation: child.generation,
                root_slot: root.source_slot,
                root_generation: root.generation,
                kind,
            },
        );
        true
    }

    fn raw_creature_part_chain_contains(
        &self,
        root: SourceEntityHandle,
        target: SourceEntityHandle,
        kind: CreatureKind,
        allow_unbound_target: bool,
    ) -> bool {
        let Some(SourceEntityView::Imported(root_row)) = self.resolve(root) else {
            return false;
        };
        if root_row.class != 5
            || root_row.model != 27
            || !(216..=223).contains(&root_row.action)
            || raw_word(root_row, 54) != 0
            || raw_word(root_row, 30) != root.source_slot
            || root_row.source_id != root.source_slot
        {
            return false;
        }
        let killed_chain = root_row.action == 221;
        let mut current = raw_word(root_row, 56);
        let mut visited = BTreeSet::new();
        while current != 0 && visited.insert(current) {
            let Some(row) = self.entities.get(usize::from(current)) else {
                return false;
            };
            if !row.active
                || row.class != 5
                || row.model != 27
                || row.record[63] >= 5
                || raw_word(row, 54) != root.source_slot
                || raw_word(row, 30) != root.source_slot
                || row.source_id != root.source_slot
                || !(matches!(row.action, 233 | 234) || (killed_chain && row.action == 221))
            {
                return false;
            }
            let generation = self.slot_generation(current);
            let is_unbound_target = allow_unbound_target && current == target.source_slot;
            if is_unbound_target {
                if generation != target.generation {
                    return false;
                }
                if let Some(binding) = self.raw_creature_parts.get(&current)
                    && (binding.child_generation != generation
                        || binding.root_slot != root.source_slot
                        || binding.root_generation != root.generation
                        || binding.kind != kind)
                {
                    return false;
                }
            } else if !matches!(self.raw_creature_parts.get(&current), Some(binding)
                if binding.child_generation == generation
                    && binding.root_slot == root.source_slot
                    && binding.root_generation == root.generation
                    && binding.kind == kind)
            {
                return false;
            }
            if current == target.source_slot {
                return generation == target.generation;
            }
            current = raw_word(row, 56);
        }
        false
    }

    fn raw_creature_part_identity(
        &self,
        source: &EntitySnapshot,
        binding: RawCreaturePartBinding,
    ) -> Option<CreatureRuntimeIdentity> {
        let child = SourceEntityHandle {
            source_slot: source.index,
            generation: self.slot_generation(source.index),
        };
        let root = SourceEntityHandle {
            source_slot: binding.root_slot,
            generation: binding.root_generation,
        };
        let origin = match self.raw_creature_origins.get(&binding.root_slot) {
            Some((generation, kind, origin))
                if *generation == binding.root_generation
                    && *kind == binding.kind
                    && binding.child_generation == child.generation
                    && source.active
                    && source.class == 5
                    && source.model == 27
                    && matches!(source.action, 233 | 234 | 221)
                    && source.record[63] < 5
                    && raw_word(source, 54) == binding.root_slot
                    && raw_word(source, 30) == binding.root_slot
                    && source.source_id == binding.root_slot
                    && (if source.action == 221 {
                        self.model27_part_root(child) == Some(root)
                    } else {
                        self.raw_creature_part_chain_contains(root, child, binding.kind, false)
                    }) =>
            {
                *origin
            }
            _ => return None,
        };
        Some(CreatureRuntimeIdentity {
            handle: child,
            kind: binding.kind,
            root_slot: binding.root_slot,
            role: CreatureRuntimeRole::MultipartPart,
            origin,
            raw_model: None,
            living: false,
        })
    }

    /// Resolve a retained model27 limb without requiring its already-hidden
    /// predecessors to remain in the chain. Kill221 visits higher slots in
    /// the same scan, and a following prologue can reclaim an earlier limb.
    pub(crate) fn model27_part_root(
        &self,
        child: SourceEntityHandle,
    ) -> Option<SourceEntityHandle> {
        let binding = self.raw_creature_parts.get(&child.source_slot)?;
        let row = self.entities.get(usize::from(child.source_slot))?;
        if binding.kind != CreatureKind::Creature27
            || binding.child_generation != child.generation
            || self.slot_generation(child.source_slot) != child.generation
            || self.slot_generation(binding.root_slot) != binding.root_generation
            || !row.active
            || row.class != 5
            || row.model != 27
            || !matches!(row.action, 233 | 234 | 221)
            || raw_word(row, 54) != binding.root_slot
            || raw_word(row, 30) != binding.root_slot
            || row.source_id != binding.root_slot
        {
            return None;
        }
        Some(SourceEntityHandle {
            source_slot: binding.root_slot,
            generation: binding.root_generation,
        })
    }

    fn raw_creature_identity(
        &self,
        source: &EntitySnapshot,
        initial: &BTreeMap<u16, CreatureKind>,
    ) -> Option<CreatureRuntimeIdentity> {
        if let Some(binding) = self.raw_creature_parts.get(&source.index)
            && binding.child_generation == self.slot_generation(source.index)
        {
            return self.raw_creature_part_identity(source, *binding);
        }
        if !source.active
            || source.class != 5
            || !matches!(
                source.model,
                9 | 10 | 16 | 17 | 18 | 20 | 21 | 22 | 23 | 24 | 25 | 26 | 27 | 28
            )
        {
            return None;
        }
        if source.model == 27 && matches!(source.action, 233 | 234) {
            return None;
        }
        if source.model == 27 && !(216..=223).contains(&source.action) {
            return None;
        }
        if source.model == 27
            && (raw_word(source, 54) != 0
                || raw_word(source, 30) != source.index
                || source.source_id != source.index)
        {
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
            9 => CreatureKind::Skeleton,
            10 => CreatureKind::FinalBoss,
            16 => CreatureKind::Wyvern,
            17 => CreatureKind::Creature17,
            18 => CreatureKind::Creature18,
            20 => CreatureKind::Spider,
            21 => CreatureKind::Creature21,
            22 => CreatureKind::Creature22,
            23 => CreatureKind::Creature23,
            24 => CreatureKind::Creature24,
            25 => CreatureKind::Cymmerian,
            26 => CreatureKind::Creature26,
            27 => CreatureKind::Creature27,
            28 => CreatureKind::Creature28,
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
        // 49F90 can also run inside5E310. Retain hidden sphere packets
        // before it frees them: the already-built head still follows these
        // addresses, and only NewEvent may overwrite their model/next fields.
        let hidden_spheres = self.economy.spheres().iter()
            .filter(|sphere| sphere.active && sphere.control_flags
                & crate::level1_mission::SOURCE_DISABLED_FLAG != 0)
            .filter(|sphere| self.economy.live_arena_kind(sphere.source_entity_index)
                == Some(Level2DynamicArenaKind::ManaSphere))
            .copied().collect::<Vec<_>>();
        for sphere in hidden_spheres {
            self.entities[usize::from(sphere.source_entity_index)].retain_mana_sphere_source(&sphere);
        }
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
            self.effects.forget_source_slot(slot);
            self.entities[usize::from(slot)].reclaim_source_row();
            self.economy.release_slot(slot, Some(rng));
            crate::native_world::creatures::morph::forget_holder(&mut self.economy, slot);
            self.raw_creature_origins.remove(&slot);
            self.creature_parents.remove(&slot);
            self.clear_raw_creature_part_bindings_for_slot(slot);
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
        crate::world::fields::retire_hidden_wind_projections(self, terrain)?;
        crate::native_world::area_damage::retire_hidden_player_death_markers(self, terrain)?;
        // Explicit source49F90 reclaims every hidden row before rebuilding
        // allocation stacks; bare allocator rebuilds keep their own policy.
        crate::world::effects::lighting::prepare_fireball_reclaim(self, terrain)?;
        self.economy.reclaim_fireball_rows_at_source_boundary();
        crate::native_world::terrain_strip::prepare_trigger_reclaim(self, terrain)?;
        self.economy.reclaim_death_burst_rows_at_source_boundary();
        crate::world::scenery::trees::prepare_reclaim(self, terrain)?;
        crate::native_world::prepare_model17_reclaim(self)?;
        self.economy.reclaim_inactive_slots_at_source_boundary();
        let retired_morph_holders = crate::native_world::creatures::morph::holder_generations(&self.economy)
            .filter_map(|(slot, generation)| {
                (self.slot_generation(slot) != generation || self.handle(slot).is_none())
                    .then_some(slot)
            }).collect::<Vec<_>>();
        for slot in retired_morph_holders {
            crate::native_world::creatures::morph::forget_holder(&mut self.economy, slot);
        }
        let retired_origins = self
            .raw_creature_origins
            .keys()
            .copied()
            .filter(|slot| self.handle(*slot).is_none())
            .collect::<Vec<_>>();
        for slot in retired_origins {
            self.clear_raw_creature_origin(slot);
        }
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

    /// Finish NewEvent's retirement of an already allocated address. Typed
    /// constructors retain their new carrier while the prior generation's
    /// raw packet and projections must not survive the allocation boundary.
    pub(crate) fn retire_previous_source_projection(&mut self, slot: u16) {
        self.effects.forget_source_slot(slot);
        let mut raw = crate::world::terrain::cave_features::new_event(
            slot, self.economy.new_event_rng(slot), Position::default(),
        );
        raw.reclaim_source_row();
        self.entities[usize::from(slot)] = raw;
        crate::native_world::creatures::morph::forget_holder(&mut self.economy, slot);
        self.raw_creature_origins.remove(&slot);
        self.creature_parents.remove(&slot);
        self.clear_raw_creature_part_bindings_for_slot(slot);
    }

    /// Preserve the current fixed-address target fields before its typed
    /// Castle carrier is reclaimed. sub57F20 clears class, not pose/model or
    /// next_0; a later flyer still dereferences that same physical record.
    pub(crate) fn retain_castle_target(&mut self, view: crate::native_world::source_target::View) {
        let kind = self.economy.live_arena_kind(view.slot);
        let resources = self.economy.castle_children().iter()
            .find(|row| row.active && row.source_entity_index == view.slot
                && matches!(kind, Some(Level2DynamicArenaKind::CastleChild
                    | Level2DynamicArenaKind::CastleController | Level2DynamicArenaKind::CastleDefender
                    | Level2DynamicArenaKind::CastleAuxiliary)))
            .map(|row| (row.mana, row.max_mana, row.player_entity_index, row.source_dword_10))
            .or_else(|| self.economy.castle_entry(view.slot)
                .filter(|row| row.slot_generation == self.economy.slot_generation(view.slot))
                .map(|row| {
                (row.mana, row.mana_capacity, row.player_entity_index, i32::from(row.level))
            }));
        let root_words = self.economy.castle_entry(view.slot)
            .map(|row| (row.auxiliary_cooldown, row.source_word_2e, row.source_word_30));
        let child_active = self.economy.castle_children().iter()
            .find(|row| row.active && row.source_entity_index == view.slot
                && matches!(kind, Some(Level2DynamicArenaKind::CastleChild
                    | Level2DynamicArenaKind::CastleController | Level2DynamicArenaKind::CastleDefender
                    | Level2DynamicArenaKind::CastleAuxiliary)))
            .map(|row| row.source_word_2e);
        let raw = &mut self.entities[usize::from(view.slot)];
        raw.source_id = view.source_id;
        raw.record[30..32].copy_from_slice(&view.source_id.to_le_bytes());
        raw.model = view.model;
        raw.record[68] = view.model;
        raw.set_position(view.position);
        raw.set_life(view.life);
        raw.set_control_flags(view.control_flags);
        if let Some((mana, maximum, owner, stage)) = resources {
            raw.mana = mana;
            raw.record[148..152].copy_from_slice(&mana.to_le_bytes());
            raw.max_mana = maximum;
            raw.record[144..148].copy_from_slice(&maximum.to_le_bytes());
            raw.set_source_player_entity_index(owner);
            raw.set_source_dword_10(stage);
        }
        if let Some((queued, active, activation)) = root_words {
            raw.record[48..50].copy_from_slice(&u16::from(queued).to_le_bytes());
            raw.active_value = active.cast_signed();
            raw.record[50..52].copy_from_slice(&active.to_le_bytes());
            raw.activation_value = activation;
            raw.record[52..54].copy_from_slice(&u16::from(activation).to_le_bytes());
        } else if let Some(active) = child_active {
            raw.active_value = active;
            raw.record[50..52].copy_from_slice(&active.to_le_bytes());
        }
        if let Some(yaw) = view.yaw { raw.set_yaw(yaw); }
        if let Some(shape) = view.collision {
            for (offset, value) in [(86, shape.vertical_offset), (88, shape.half_width),
                (90, shape.half_depth), (92, shape.half_height)]
            {
                raw.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
            }
        }
        raw.reclaim_source_row();
    }

    /// 389F0 writes the owner's current fixed address, including the sentinel
    /// or a reclaimed record. Only a current Castle root owns typed mailboxes.
    pub(crate) fn write_castle_upgrade_mailbox(&mut self, slot: u16, owner: u16) {
        if self.economy.live_arena_kind(slot) == Some(Level2DynamicArenaKind::CastleRoot)
            && let Some(root) = self.economy.castle_at_mut(slot)
        {
            root.source_word_7c = 10;
            root.source_word_80 = owner;
        }
        if let Some(raw) = self.entities.get_mut(usize::from(slot)) {
            raw.record[128..130].copy_from_slice(&10_u16.to_le_bytes());
            raw.record[132..134].copy_from_slice(&owner.to_le_bytes());
        }
    }

    /// 5E310 rewrites only model39 records reached through the current
    /// dword38523 chain after the marker's NewEvent has completed.
    pub(crate) fn redirect_mana_chain_to_marker(
        &mut self,
        chain: &SourceManaChain,
        player: u16,
        marker: u16,
    ) -> usize {
        let rows = chain.rows(&self.economy, &self.entities);
        let mut redirected = 0;
        for row in rows.into_iter().filter(|row| row.player_entity_index == player) {
            if self.economy.live_arena_kind(row.source_slot) == Some(Level2DynamicArenaKind::ManaSphere) {
                if let Some(sphere) = self.economy.spheres_mut().iter_mut()
                    .find(|sphere| sphere.active && sphere.source_entity_index == row.source_slot)
                {
                    sphere.player_entity_index = marker;
                    redirected += 1;
                }
            } else {
                self.entities[usize::from(row.source_slot)].set_source_player_entity_index(marker);
                redirected += 1;
            }
        }
        self.economy.refresh_owner_mana_rows();
        redirected
    }

    /// Capture the typed products which a Castle constructor can allocate.
    /// The callback owns allocation/RNG order; this join only identifies the
    /// generations whose prior raw projections NewEvent must retire.
    pub(crate) fn castle_source_allocations(&self) -> BTreeMap<u16, u32> {
        self.economy
            .live_arena_slots()
            .into_iter()
            .filter(|(_, kind)| {
                matches!(kind,
                    Level2DynamicArenaKind::CastleProjectile
                        | Level2DynamicArenaKind::CastleRoot
                        | Level2DynamicArenaKind::CastleChild
                        | Level2DynamicArenaKind::CastleController
                        | Level2DynamicArenaKind::CastleDefender
                        | Level2DynamicArenaKind::CastleAuxiliary
                        | Level2DynamicArenaKind::ManaSphere
                )
            })
            .map(|(slot, _)| (slot, self.economy.slot_generation(slot)))
            .collect()
    }

    pub(crate) fn retire_new_castle_source_projections(&mut self, prior: &BTreeMap<u16, u32>) {
        for (slot, generation) in self.castle_source_allocations() {
            if prior.get(&slot) != Some(&generation) {
                self.retire_previous_source_projection(slot);
            }
        }
    }

    fn allocate_source_slot_notifying(&mut self, mut publish: impl FnMut(u16, u16)) -> Option<u16> {
        if self.economy.free_slot_count() != 0 {
            let slot = self.economy.allocate_slot()?;
            self.retire_previous_source_projection(slot);
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
            .find_map(|(id, kind)| (id == slot).then_some(kind))
            .or_else(|| self.economy.retained_projectile_kind(slot));
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
            // NewEvent clears the fixed source address even when the current
            // owner is typed. Its imported mirror must not reappear after the
            // replacement's typed carrier is reclaimed.
            self.entities[usize::from(slot)].reclaim_source_row();
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
        self.effects.forget_source_slot(slot);
        crate::native_world::creatures::morph::forget_holder(&mut self.economy, slot);
        self.raw_creature_origins.remove(&slot);
        self.creature_parents.remove(&slot);
        self.clear_raw_creature_part_bindings_for_slot(slot);
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
        crate::world::effects::lighting::admit_source(
            self,
            terrain,
            slot,
            position,
            crate::world::effects::lighting::SourceLight::Fireball,
        );
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
        // Dynamic raw roots need not have a reset binding or a typed row.
        // The identity resolver below checks the current slot generation.
        slots.extend(self.raw_creature_origins.keys().copied());
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
                            || matches!(
                                source.model,
                                10 | 16 | 17 | 18 | 20 | 21 | 22 | 23 | 24 | 25 | 26 | 27 | 28
                            ))
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
        if matches!(
            source.model,
            10 | 16 | 17 | 18 | 20 | 21 | 22 | 23 | 24 | 25 | 26 | 27 | 28
        ) || self.raw_creature_origins.contains_key(&source_slot)
        {
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
            let (kind, origin) = if matches!(source.model, 0 | 19) {
                if let Some((generation, kind, origin)) =
                    self.raw_creature_origins.get(&source_slot)
                {
                    if *generation != dynamic_handle.generation
                        || *kind
                            != if source.model == 0 {
                                CreatureKind::Dragon
                            } else {
                                CreatureKind::Firefly
                            }
                        || !matches!(
                            *origin,
                            CreatureRootOrigin::SummonedSpell { .. }
                                | CreatureRootOrigin::CreatureConversion { .. }
                        )
                    {
                        return None;
                    }
                    return Some(CreatureRuntimeIdentity {
                        handle: dynamic_handle,
                        kind: *kind,
                        root_slot: source_slot,
                        role: CreatureRuntimeRole::Root,
                        origin: *origin,
                        raw_model: Some(source.model),
                        living: source.active
                            && source.drawing_enabled
                            && source.life >= 0
                            && !matches!(source.action, 180 | 232 | 234),
                    });
                }

                if source.model == 0 {
                    let authored_record = source.authored_index;
                    let &kind = authored_bindings.get(&authored_record)?;
                    if kind != CreatureKind::Dragon {
                        return None;
                    }
                    return Some(CreatureRuntimeIdentity {
                        handle: dynamic_handle,
                        kind,
                        root_slot: source_slot,
                        role: CreatureRuntimeRole::Root,
                        origin: CreatureRootOrigin::AuthoredRecord { authored_record },
                        raw_model: Some(0),
                        living: source.active && source.drawing_enabled && source.life >= 0,
                    });
                }

                let firefly = self
                    .economy
                    .authored_fireflies()
                    .iter()
                    .find(|state| state.source_entity_index == source_slot)?;
                if dynamic_handle.generation == 0 {
                    let &kind = initial_bindings.get(&source_slot)?;
                    if kind != CreatureKind::Firefly {
                        return None;
                    }
                    (kind, CreatureRootOrigin::InitialPackage)
                } else {
                    let authored_record = firefly.authored_index?;
                    let &kind = authored_bindings.get(&authored_record)?;
                    if kind != CreatureKind::Firefly {
                        return None;
                    }
                    (kind, CreatureRootOrigin::AuthoredRecord { authored_record })
                }
            } else {
                let authored_record = source.authored_index;
                (
                    *authored_bindings.get(&authored_record)?,
                    CreatureRootOrigin::AuthoredRecord { authored_record },
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
                BeeCreatureOrigin::SummonedSpell { holder_slot } => {
                    CreatureRootOrigin::SummonedSpell { holder_slot }
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
            if let Some((root_generation, kind, origin)) =
                self.raw_creature_origins.get(&source.entity_id)
                && *root_generation == self.slot_generation(source.entity_id)
                && *kind == CreatureKind::Dragon
            {
                return Some(CreatureRuntimeIdentity {
                    handle: dynamic_handle,
                    kind: *kind,
                    root_slot: source.entity_id,
                    role: CreatureRuntimeRole::MultipartPart,
                    origin: *origin,
                    raw_model: None,
                    living: false,
                });
            }
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
        if matches!(
            source.model,
            10 | 16 | 17 | 18 | 20 | 21 | 22 | 23 | 24 | 25 | 26 | 27 | 28
        ) || self.raw_creature_origins.contains_key(&source_slot)
        {
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
        FlightWorld, PlayerCommand,
        campaign::{CampaignSession, spec::CampaignSelection},
    };
    use std::path::PathBuf;

    fn step_opening_source_turn(session: &mut crate::native_world::Mc2Simulation) {
        let evidence = session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !evidence.is_empty(),
            "opening source turn must commit, events: {:?}",
            session.dispatch_events()
        );
    }

    fn level_two_session() -> crate::native_world::Mc2Simulation {
        let package = std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
            },
            PathBuf::from,
        );
        CampaignSession::open(package, CampaignSelection::LEVEL2)
            .expect("qualified Level 2 semantic source arena package")
            .simulation()
            .clone()
    }

    fn level_two_constructor_session() -> crate::native_world::Mc2Simulation {
        // MCR1 v2 retains the actual 29 constructor counters. The older
        // qualification package above intentionally predates that export.
        let package = std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-1pb-16-level2-full-build-v2")
            },
            PathBuf::from,
        );
        CampaignSession::open(package, CampaignSelection::LEVEL2)
            .expect("real Level 2 package with source constructor counters")
            .simulation()
            .clone()
    }

    #[test]
    fn mana_chain_keeps_freed_fields_and_cuts_new_event_successor() {
        let session = level_two_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.session.state.live.terrain.clone();
        let mut position = session.state.position;
        position.z = terrain.ground_height(position);
        let first = arena.allocate_authored_model39(0, 0, position).unwrap().source_entity_index;
        let marker = arena.allocate_player_death_marker(session.state.entity_index, position)
            .unwrap().source_entity_index;
        let middle = arena.allocate_authored_model39(0, 0, position).unwrap().source_entity_index;
        let last_position = Position { x: position.x.wrapping_add(4096), ..position };
        let last = arena.allocate_authored_model39(0, 0, last_position).unwrap().source_entity_index;
        let chain = SourceManaChain::capture(&mut arena, vec![first, marker, middle, last]);
        // The actual constructor starts with upward velocity128; let its
        // ordinary physics reach ground contact before expecting36D50.
        for _ in 0..64 {
            assert!(arena.economy.step_mana_sphere_slot(first, &terrain, &mut arena.entities, |_| 0));
            if arena.economy.spheres().iter().all(|row| row.source_entity_index != middle) { break; }
        }
        assert!(arena.economy.spheres().iter().all(|row| row.source_entity_index != middle));
        assert_eq!((arena.entities[usize::from(middle)].class,
            arena.entities[usize::from(middle)].model), (0, 39));
        assert_eq!(chain.rows(&arena.economy, &arena.entities).iter()
            .map(|row| row.source_slot).collect::<Vec<_>>(), vec![first, middle, last],
            "the frozen marker bridge and retained class-zero sphere keep their current next links");
        let reused = arena.allocate_source_slot().unwrap();
        assert_eq!(reused, middle);
        assert_eq!(arena.entities[usize::from(reused)].model, 0);
        assert_eq!(chain.rows(&arena.economy, &arena.entities).iter()
            .map(|row| row.source_slot).collect::<Vec<_>>(), vec![first],
            "NewEvent overwrites the model and cuts next_0; the old sphere cannot be credited");
        arena.economy.initialize_authored_model39(reused, 0, position).unwrap();
        assert_eq!(chain.rows(&arena.economy, &arena.entities).iter()
            .map(|row| row.source_slot).collect::<Vec<_>>(), vec![first, reused],
            "a genuine new model39 at the frozen address is read before its zero successor");
        arena.economy.spheres_mut().iter_mut()
            .find(|row| row.active && row.source_entity_index == reused).unwrap()
            .control_flags |= crate::level1_mission::SOURCE_DISABLED_FLAG;
        arena.rebuild_source_arena_with_terrain(&mut terrain, &[]).unwrap();
        assert_eq!((arena.entities[usize::from(reused)].class,
            arena.entities[usize::from(reused)].model), (0, 39),
            "49F90 retains the hidden model39 packet before marker allocation");
        let new_marker = arena.allocate_player_death_marker(session.state.entity_index, position).unwrap();
        assert_eq!(new_marker.source_entity_index, reused);
        arena.retire_previous_source_projection(reused);
        assert_eq!(chain.rows(&arena.economy, &arena.entities).iter()
            .map(|row| row.source_slot).collect::<Vec<_>>(), vec![first],
            "marker NewEvent cuts the current source link after49F90 reclaim");
    }

    #[test]
    fn level_four_group12_constructs_direct_and_subtype_routed_fireflies() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level4");
        let campaign =
            CampaignSession::open(package, CampaignSelection::ordinary(4).unwrap()).unwrap();
        let mut session = campaign.simulation().clone();
        let expected = session
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .filter(|feature| {
                feature.kind == 5 && feature.subtype == 19 && feature.activation_group == 12
            })
            .map(|feature| feature.index)
            .collect::<BTreeSet<_>>();
        assert_eq!(expected.len(), 20);
        session
            .activate_authored_group_prerequisite_for_test(12)
            .unwrap();
        let actual = session
            .source_arena()
            .economy
            .authored_class5()
            .iter()
            .filter(|row| row.active && row.model == 19 && expected.contains(&row.authored_index))
            .map(|row| row.authored_index)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual, expected,
            "direct routes must not silently omit three Fireflies"
        );
        let slots = session
            .source_arena()
            .economy
            .authored_class5()
            .iter()
            .filter(|row| [260, 263, 309].contains(&row.authored_index))
            .map(|row| (row.source_entity_index, row.source_byte_3e))
            .collect::<Vec<_>>();
        assert_eq!(slots.len(), 3);
        step_opening_source_turn(&mut session);
        for (slot, phase) in slots {
            let row = session
                .source_arena()
                .economy
                .authored_class5()
                .iter()
                .find(|row| row.source_entity_index == slot)
                .unwrap();
            assert!(row.active);
            assert_eq!(
                row.source_byte_3e,
                phase.wrapping_add(1),
                "direct-route birth reaches normal callback epilogue"
            );
        }
    }

    #[test]
    fn opening_authored_skeleton_groups_use_full_damage_death_and_reclaim_path() {
        for (level, package, group) in [
            (2, "mc-level2-vault-successor-closure-20261025", 22),
            (4, "mc-bhq6-ordinary-20261025-zclu5fjj/level4", 9),
        ] {
            let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated")
                .join(package);
            let campaign =
                CampaignSession::open(package, CampaignSelection::ordinary(level).unwrap())
                    .expect("actual opening campaign package");
            let mut session = campaign.simulation().clone();
            let authored = session
                .session
                .state
                .live
                .mission
                .authored_features()
                .iter()
                .filter(|row| {
                    row.kind == 5 && row.subtype == 9 && row.activation_group == group as i16
                })
                .map(|row| row.index)
                .collect::<BTreeSet<_>>();
            assert!(!authored.is_empty());
            session
                .activate_authored_group_prerequisite_for_test(group)
                .unwrap();
            let slot = session
                .source_arena()
                .entities
                .iter()
                .filter_map(|row| session.creature_identity_for_source_slot(row.index))
                .find_map(|identity| match identity.origin {
                    CreatureRootOrigin::AuthoredRecord { authored_record }
                        if identity.kind == CreatureKind::Skeleton
                            && authored.contains(&authored_record) =>
                    {
                        Some(identity.root_slot)
                    }
                    _ => None,
                })
                .expect("real authored group constructs a Skeleton");
            assert_eq!(
                session.source_arena().entities[usize::from(slot)].action,
                72
            );
            for _ in 0..17 {
                step_opening_source_turn(&mut session);
            }
            let row = &session.source_arena().entities[usize::from(slot)];
            assert_eq!(
                row.action, 73,
                "Level {level} emergence reaches full ordinary handler"
            );
            let phase = row.phase;
            let player = session.session.state.live.core.state.entity_index;
            session
                .stage_imported_skeleton_fireball_damage_for_test(slot, player, 500)
                .unwrap();
            step_opening_source_turn(&mut session);
            let row = &session.source_arena().entities[usize::from(slot)];
            assert_eq!(
                (row.life, row.action, row.word_26(), row.source_word_96()),
                (500, 74, player, player)
            );
            assert_eq!(row.phase, phase.wrapping_add(1));
            assert_eq!(u16::from_le_bytes([row.record[102], row.record[103]]), 0);
            assert_eq!(row.particle_parameter_index, Some(202));
            let spheres = session.source_arena().economy.spheres().len();
            let bursts = session.source_arena().economy.death_bursts().len();
            session
                .stage_imported_skeleton_fireball_damage_for_test(slot, player, 1000)
                .unwrap();
            step_opening_source_turn(&mut session);
            let row = &session.source_arena().entities[usize::from(slot)];
            assert_eq!((row.life, row.action, row.word_24()), (-500, 76, player));
            step_opening_source_turn(&mut session);
            assert_eq!(
                session.source_arena().entities[usize::from(slot)].action,
                77
            );
            for _ in 0..8 {
                step_opening_source_turn(&mut session);
                if session.source_arena().entities[usize::from(slot)].drawing_disabled() {
                    break;
                }
            }
            let row = &session.source_arena().entities[usize::from(slot)];
            assert!(
                row.active && row.drawing_disabled(),
                "death stays allocated through its epilogue"
            );
            let economy = &session.source_arena().economy;
            assert!(economy.spheres().len() > spheres);
            assert!(
                economy.death_bursts()[bursts..]
                    .iter()
                    .any(|burst| burst.parent_entity_index == slot)
            );
            step_opening_source_turn(&mut session);
            let row = &session.source_arena().entities[usize::from(slot)];
            assert!(
                !row.active && row.class == 0,
                "next pass reclaims the raw authored carrier"
            );
            assert!(session.creature_identity_for_source_slot(slot).is_none());
            assert!(
                !session
                    .source_arena()
                    .economy
                    .map_chains()
                    .linked_entity_indices()
                    .contains(&slot)
            );
        }
    }

    #[test]
    fn authored_skeleton_constructor_binds_raw_identity_and_proximity() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-level2-vault-successor-closure-20261025");
        let campaign = CampaignSession::open(package, CampaignSelection::LEVEL2).unwrap();
        let session = campaign.simulation();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.session.state.live.terrain.clone();
        let definition = &session.session.definition;
        let feature = session
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .find(|feature| feature.index == 239)
            .unwrap();
        let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
        let slot = arena
            .allocate_authored_skeleton_raw(
                239,
                position,
                1000,
                Some(7),
                &mut terrain,
                &definition.presentation.particles,
                &definition.presentation.entity_parameters[80],
            )
            .unwrap()
            .expect("actual Level 2 authored Skeleton placement is admissible");
        let identity = arena
            .creature_runtime_identity(slot, &BTreeMap::new(), &BTreeMap::new())
            .expect("dynamic raw Skeleton has its own semantic origin");
        assert_eq!(identity.kind, CreatureKind::Skeleton);
        assert_eq!(
            identity.origin,
            CreatureRootOrigin::AuthoredRecord {
                authored_record: 239
            }
        );
        let row = &mut arena.entities[usize::from(slot)];
        assert_eq!((row.class, row.model, row.action), (5, 9, 72));
        assert_eq!(row.source_dword_10(), 16);
        assert_eq!(u16::from_le_bytes([row.record[78], row.record[79]]), 7);
        assert_ne!(row.record[61], 0);
        let before = row.record[61];
        let mut models: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
        models[9].push(slot);
        arena.step_opening_creature_proximity(&models, position);
        assert_eq!(arena.entities[usize::from(slot)].record[61], before - 1);
        arena.entities[usize::from(slot)].record[61] = 0;
        arena.step_opening_creature_proximity(&models, position);
        assert_eq!(arena.entities[usize::from(slot)].record[61], 16);
        arena.entities[usize::from(slot)].set_life(-1);
        arena.step_opening_creature_proximity(&models, position);
        assert_eq!(arena.entities[usize::from(slot)].record[61], 250);
    }

    #[test]
    fn model17_origin_is_generation_bound_and_dead_root_is_not_living() {
        let session = level_two_session();
        let mut arena = session.source_arena().clone();
        let slot = arena.allocate_source_slot().unwrap();
        let mut row = crate::world::terrain::cave_features::new_event(
            slot,
            arena.new_event_rng(slot),
            Position::default(),
        );
        row.class = 5;
        row.model = 17;
        row.set_source_action(137);
        row.set_life(10_000);
        arena.entities[usize::from(slot)] = row;
        let handle = arena.handle(slot).unwrap();
        let origin = CreatureRootOrigin::AuthoredRecord {
            authored_record: 17,
        };
        assert!(arena.bind_raw_creature_origin(handle, CreatureKind::Creature17, origin));
        let empty = BTreeMap::new();
        let identity = arena
            .imported_creature_runtime_identity(slot, &empty)
            .unwrap();
        assert_eq!(identity.kind, CreatureKind::Creature17);
        assert!(identity.living);
        arena.entities[usize::from(slot)].set_life(-1);
        assert!(
            !arena
                .imported_creature_runtime_identity(slot, &empty)
                .unwrap()
                .living
        );
        arena.entities[usize::from(slot)].reclaim_source_row();
        arena.economy.release_slot(slot, None);
        assert!(
            arena
                .imported_creature_runtime_identity(slot, &empty)
                .is_none()
        );
        assert!(!arena.bind_raw_creature_origin(handle, CreatureKind::Creature17, origin));
    }

    #[test]
    fn final_boss_authored_identity_rejects_stale_and_reused_generation() {
        let session = level_two_session();
        let mut arena = session.source_arena().clone();
        let slot = arena.allocate_source_slot().unwrap();
        let mut row = crate::world::terrain::cave_features::new_event(
            slot,
            arena.new_event_rng(slot),
            Position::default(),
        );
        row.class = 5;
        row.model = 10;
        row.set_source_action(80);
        row.set_life(100_000);
        arena.entities[usize::from(slot)] = row;
        let handle = arena.handle(slot).unwrap();
        let origin = CreatureRootOrigin::AuthoredRecord {
            authored_record: 379,
        };
        assert!(arena.bind_raw_creature_origin(handle, CreatureKind::FinalBoss, origin));
        let empty = BTreeMap::new();
        let identity = arena
            .imported_creature_runtime_identity(slot, &empty)
            .expect("current-generation boss resolves its authored origin");
        assert_eq!(identity.kind, CreatureKind::FinalBoss);
        assert_eq!(identity.origin, origin);
        assert_eq!(identity.handle, handle);

        let replacement = arena.entities[usize::from(slot)].clone();
        arena.entities[usize::from(slot)].reclaim_source_row();
        arena.economy.release_slot(slot, None);
        assert_eq!(arena.allocate_source_slot(), Some(slot));
        arena.entities[usize::from(slot)] = replacement;
        assert!(!arena.bind_raw_creature_origin(handle, CreatureKind::FinalBoss, origin));
        assert!(
            arena
                .imported_creature_runtime_identity(slot, &empty)
                .is_none()
        );
    }

    #[test]
    fn authored_wyvern_identity_tracks_death_and_rejects_reused_origin() {
        let session = level_two_session();
        let mut arena = session.source_arena().clone();
        let slot = arena.allocate_source_slot().unwrap();
        let mut row = crate::world::terrain::cave_features::new_event(
            slot,
            arena.new_event_rng(slot),
            Position::default(),
        );
        row.class = 5;
        row.model = 16;
        row.set_source_action(129);
        row.set_life(60_000);
        arena.entities[usize::from(slot)] = row;
        let handle = arena.handle(slot).unwrap();
        let origin = CreatureRootOrigin::AuthoredRecord { authored_record: 1 };
        assert!(!arena.bind_raw_creature_origin(handle, CreatureKind::Creature24, origin));
        assert!(arena.bind_raw_creature_origin(handle, CreatureKind::Wyvern, origin));
        let empty = BTreeMap::new();
        let identity = arena
            .imported_creature_runtime_identity(slot, &empty)
            .unwrap();
        assert_eq!(identity.kind, CreatureKind::Wyvern);
        assert!(identity.living);
        arena.entities[usize::from(slot)].set_life(-1);
        assert!(
            !arena
                .imported_creature_runtime_identity(slot, &empty)
                .unwrap()
                .living
        );
        let mut replacement = arena.entities[usize::from(slot)].clone();
        replacement.set_life(60_000);
        arena.entities[usize::from(slot)].reclaim_source_row();
        arena.economy.release_slot(slot, None);
        assert_eq!(arena.allocate_source_slot(), Some(slot));
        arena.entities[usize::from(slot)] = replacement;
        assert!(!arena.bind_raw_creature_origin(handle, CreatureKind::Wyvern, origin));
        assert!(
            arena
                .imported_creature_runtime_identity(slot, &empty)
                .is_none()
        );
        let fresh = arena.handle(slot).unwrap();
        assert!(arena.bind_raw_creature_origin(fresh, CreatureKind::Wyvern, origin));
        assert!(
            arena
                .imported_creature_runtime_identity(slot, &empty)
                .unwrap()
                .living
        );
    }

    #[test]
    fn summoned_parent_handle_rejects_a_reused_parent_slot() {
        let session = level_two_constructor_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.session.state.live.terrain.clone();
        let mut runtime = session.session.state.live.core.runtime.clone();
        let particles = &session.session.definition.presentation.particles;
        let position = session.session.state.live.core.state.position;
        let parent_slot = crate::native_world::creatures::creature_18::mc2::allocate_authored(
            &mut arena,
            &mut runtime,
            &mut terrain,
            particles,
            position,
            12_000,
            None,
            1_018,
        )
        .unwrap()
        .unwrap();
        let child_slot = crate::native_world::creatures::creature_18::mc2::allocate_authored(
            &mut arena,
            &mut runtime,
            &mut terrain,
            particles,
            position,
            12_000,
            None,
            2_018,
        )
        .unwrap()
        .unwrap();
        let parent = arena.handle(parent_slot).unwrap();
        let child = arena.handle(child_slot).unwrap();
        let before = arena.reset_hash_with_terrain(&terrain);
        assert!(arena.bind_creature_parent(child, parent));
        assert_eq!(arena.creature_parent(child), Some(parent));
        assert_ne!(arena.reset_hash_with_terrain(&terrain), before);
        let replacement = arena.entities[usize::from(parent_slot)].clone();
        arena.entities[usize::from(parent_slot)].reclaim_source_row();
        arena.economy.release_slot(parent_slot, None);
        assert_eq!(arena.allocate_source_slot(), Some(parent_slot));
        arena.entities[usize::from(parent_slot)] = replacement;
        let fresh_parent = arena.handle(parent_slot).unwrap();
        assert_ne!(fresh_parent.generation, parent.generation);
        assert_eq!(arena.creature_parent(child), None);
        assert!(arena.has_creature_parent_binding(child));
        assert_eq!(arena.creature_parent_binding(child), Some(parent));
        assert!(!arena.bind_creature_parent(child, parent));

        let descendant_slot = crate::native_world::creatures::creature_18::mc2::allocate_authored(
            &mut arena,
            &mut runtime,
            &mut terrain,
            particles,
            position,
            12_000,
            None,
            3_018,
        )
        .unwrap()
        .unwrap();
        let descendant = arena.handle(descendant_slot).unwrap();
        assert!(arena.bind_carried_creature_parent(
            descendant,
            arena.creature_parent_binding(child).unwrap(),
        ));
        assert_eq!(arena.creature_parent_binding(descendant), Some(parent));
        assert_eq!(arena.creature_parent(descendant), None);
        assert!(arena.has_creature_parent_binding(descendant));
        assert!(!arena.bind_carried_creature_parent(descendant, descendant));

        assert!(arena.bind_creature_parent(child, fresh_parent));
        assert_eq!(arena.creature_parent(child), Some(fresh_parent));
        assert_eq!(arena.creature_parent(descendant), None);
        let replacement = arena.entities[usize::from(descendant_slot)].clone();
        arena.entities[usize::from(descendant_slot)].reclaim_source_row();
        arena.economy.release_slot(descendant_slot, None);
        assert_eq!(arena.allocate_source_slot(), Some(descendant_slot));
        arena.entities[usize::from(descendant_slot)] = replacement;
        let fresh_descendant = arena.handle(descendant_slot).unwrap();
        assert_ne!(fresh_descendant, descendant);
        assert_eq!(arena.creature_parent_binding(descendant), None);
        assert_eq!(arena.creature_parent_binding(fresh_descendant), None);
        assert!(!arena.bind_carried_creature_parent(descendant, fresh_parent));
    }

    #[test]
    fn authored_models18_21_and26_keep_allocator_generations_in_raw_census() {
        let session = level_two_constructor_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.session.state.live.terrain.clone();
        let mut runtime = session.session.state.live.core.runtime.clone();
        let particles = &session.session.definition.presentation.particles;
        let empty = BTreeMap::new();

        let model18 = crate::native_world::creatures::creature_18::mc2::allocate_authored(
            &mut arena,
            &mut runtime,
            &mut terrain,
            particles,
            Position {
                x: 20_000,
                y: 20_000,
                z: 5_000,
            },
            12_000,
            None,
            1_018,
        )
        .unwrap()
        .expect("actual model18 source allocation succeeds");
        let identity18 = arena
            .creature_runtime_identity(model18, &empty, &empty)
            .expect("allocated model18 row resolves through its origin");
        assert_eq!(identity18.kind, CreatureKind::Creature18);
        assert_eq!(identity18.root_slot, model18);
        assert_eq!(identity18.role, CreatureRuntimeRole::Root);
        assert_eq!(
            identity18.origin,
            CreatureRootOrigin::AuthoredRecord {
                authored_record: 1_018,
            }
        );
        assert_eq!(
            identity18.handle.generation,
            arena.slot_generation(model18),
            "model18 constructor and identity use the allocator generation"
        );
        assert_ne!(identity18.handle.generation, 0);

        let model21 = crate::native_world::creatures::creature_21::mc2::allocate_authored(
            &mut arena,
            &mut runtime,
            &mut terrain,
            particles,
            Position {
                x: 21_000,
                y: 20_000,
                z: 5_000,
            },
            12_000,
            None,
            1_021,
        )
        .unwrap()
        .expect("actual model21 source allocation succeeds");
        let identity21 = arena
            .creature_runtime_identity(model21, &empty, &empty)
            .expect("allocated model21 row resolves through its origin");
        assert_eq!(identity21.kind, CreatureKind::Creature21);
        assert_eq!(identity21.root_slot, model21);
        assert_eq!(identity21.role, CreatureRuntimeRole::Root);
        assert_eq!(
            identity21.origin,
            CreatureRootOrigin::AuthoredRecord {
                authored_record: 1_021
            }
        );
        assert_eq!(identity21.handle.generation, arena.slot_generation(model21));
        assert_ne!(identity21.handle.generation, 0);
        assert!(!arena.bind_raw_creature_origin(
            identity21.handle,
            CreatureKind::Creature18,
            identity21.origin,
        ));

        let model26 = crate::native_world::creatures::creature_26::mc2::allocate_authored(
            &mut arena,
            &mut runtime,
            &mut terrain,
            particles,
            Position {
                x: 22_000,
                y: 20_000,
                z: 5_000,
            },
            8_000,
            None,
            1_026,
        )
        .unwrap()
        .expect("actual model26 source allocation succeeds");
        let identity26 = arena
            .creature_runtime_identity(model26, &empty, &empty)
            .expect("allocated model26 row resolves through its origin");
        assert_eq!(identity26.kind, CreatureKind::Creature26);
        assert_eq!(identity26.root_slot, model26);
        assert_eq!(identity26.role, CreatureRuntimeRole::Root);
        assert_eq!(
            identity26.origin,
            CreatureRootOrigin::AuthoredRecord {
                authored_record: 1_026,
            }
        );
        assert_eq!(
            identity26.handle.generation,
            arena.slot_generation(model26),
            "model26 origin and identity use the allocator generation"
        );
        assert_ne!(identity26.handle.generation, 0);

        let census = arena.creature_runtime_census(&empty, &empty);
        assert_eq!(
            census
                .roots_of_kind(CreatureKind::Creature21)
                .collect::<Vec<_>>(),
            vec![identity21],
        );
        assert_eq!(
            census
                .roots_of_kind(CreatureKind::Creature18)
                .collect::<Vec<_>>(),
            vec![identity18]
        );
        assert_eq!(
            census
                .roots_of_kind(CreatureKind::Creature26)
                .collect::<Vec<_>>(),
            vec![identity26]
        );

        arena.entities[usize::from(model26)].reclaim_source_row();
        arena.economy.release_slot(model26, None);
        assert_eq!(arena.allocate_source_slot(), Some(model26));
        assert_ne!(arena.slot_generation(model26), identity26.handle.generation);
        assert!(
            arena
                .creature_runtime_identity(model26, &empty, &empty)
                .is_none()
        );
    }

    #[test]
    fn model27_parts_resolve_only_through_current_owner_and_linked_generations() {
        let session = level_two_session();
        let mut arena = session.source_arena().clone();
        let terrain = session.session.state.live.terrain.clone();
        let root_slot = arena.allocate_source_slot().unwrap();
        let head_slot = arena.allocate_source_slot().unwrap();
        let body_slot = arena.allocate_source_slot().unwrap();

        let mut root_row = crate::world::terrain::cave_features::new_event(
            root_slot,
            arena.new_event_rng(root_slot),
            Position::default(),
        );
        root_row.class = 5;
        root_row.model = 27;
        root_row.source_id = root_slot;
        root_row.record[30..32].copy_from_slice(&root_slot.to_le_bytes());
        root_row.record[54..56].fill(0);
        root_row.record[56..58].copy_from_slice(&head_slot.to_le_bytes());
        root_row.record[63] = 5;
        root_row.set_source_action(217);
        arena.entities[usize::from(root_slot)] = root_row;

        for (slot, action, next) in [(head_slot, 233, body_slot), (body_slot, 234, 0)] {
            let mut row = crate::world::terrain::cave_features::new_event(
                slot,
                arena.new_event_rng(slot),
                Position::default(),
            );
            row.class = 5;
            row.model = 27;
            row.source_id = root_slot;
            row.record[30..32].copy_from_slice(&root_slot.to_le_bytes());
            row.record[54..56].copy_from_slice(&root_slot.to_le_bytes());
            row.record[56..58].copy_from_slice(&next.to_le_bytes());
            row.record[63] = 0;
            row.set_source_action(action);
            arena.entities[usize::from(slot)] = row;
        }

        let root = arena.handle(root_slot).unwrap();
        let head = arena.handle(head_slot).unwrap();
        let body = arena.handle(body_slot).unwrap();
        let origin = CreatureRootOrigin::AuthoredRecord { authored_record: 1 };
        let unbound_hash = arena.reset_hash_with_terrain(&terrain);
        arena.entities[usize::from(root_slot)].record[54..56]
            .copy_from_slice(&root_slot.to_le_bytes());
        assert!(!arena.bind_raw_creature_origin(root, CreatureKind::Creature27, origin));
        arena.entities[usize::from(root_slot)].record[54..56].fill(0);
        assert!(arena.bind_raw_creature_origin(root, CreatureKind::Creature27, origin));

        assert!(arena.bind_raw_creature_part(head, root, CreatureKind::Creature27));
        arena.entities[usize::from(body_slot)].record[54..56]
            .copy_from_slice(&root_slot.wrapping_add(1).to_le_bytes());
        assert!(!arena.bind_raw_creature_part(body, root, CreatureKind::Creature27));
        arena.entities[usize::from(body_slot)].record[54..56]
            .copy_from_slice(&root_slot.to_le_bytes());
        arena.entities[usize::from(head_slot)].record[56..58].fill(0);
        assert!(!arena.bind_raw_creature_part(body, root, CreatureKind::Creature27));
        arena.entities[usize::from(head_slot)].record[56..58]
            .copy_from_slice(&body_slot.to_le_bytes());
        assert!(arena.bind_raw_creature_part(body, root, CreatureKind::Creature27));

        let empty = BTreeMap::new();
        let identity = arena
            .creature_runtime_identity(body_slot, &empty, &empty)
            .expect("linked model27 segment resolves to its typed root");
        assert_eq!(identity.handle, body);
        assert_eq!(identity.kind, CreatureKind::Creature27);
        assert_eq!(identity.root_slot, root_slot);
        assert_eq!(identity.role, CreatureRuntimeRole::MultipartPart);
        assert_eq!(identity.origin, origin);
        let census = arena.creature_runtime_census(&empty, &empty);
        assert_eq!(census.roots_of_kind(CreatureKind::Creature27).count(), 1);

        let bound_hash = arena.reset_hash_with_terrain(&terrain);
        assert_ne!(bound_hash, unbound_hash);
        assert_eq!(arena.clone().reset_hash_with_terrain(&terrain), bound_hash);

        let mut child_reuse = arena.clone();
        child_reuse.entities[usize::from(body_slot)].reclaim_source_row();
        child_reuse.economy.release_slot(body_slot, None);
        assert_eq!(child_reuse.allocate_source_slot(), Some(body_slot));
        assert_ne!(child_reuse.slot_generation(body_slot), body.generation);
        assert!(
            child_reuse
                .creature_runtime_identity(body_slot, &empty, &empty)
                .is_none()
        );

        let mut root_reuse = arena.clone();
        root_reuse.entities[usize::from(root_slot)].reclaim_source_row();
        root_reuse.economy.release_slot(root_slot, None);
        assert_eq!(root_reuse.allocate_source_slot(), Some(root_slot));
        assert_ne!(root_reuse.slot_generation(root_slot), root.generation);
        assert!(
            root_reuse
                .creature_runtime_identity(head_slot, &empty, &empty)
                .is_none()
        );

        arena.clear_raw_creature_origin(root_slot);
        assert!(
            arena
                .creature_runtime_identity(body_slot, &empty, &empty)
                .is_none()
        );
        assert_eq!(arena.reset_hash_with_terrain(&terrain), unbound_hash);
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
            let census = campaign.simulation().creature_runtime_census();
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
            .map(crate::native_world::Mc2DispatchEvent::campaign_projection)
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
