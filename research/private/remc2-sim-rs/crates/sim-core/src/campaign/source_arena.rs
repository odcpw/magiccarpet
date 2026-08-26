//! Canonical source-slot arena for the campaign runtime.
//!
//! The decoded fixed records and the dynamic economy projections are one
//! allocation/topology domain in remc2. Keeping them in one owner prevents a
//! session-side entity mirror from drifting from the free stack, generation
//! counters, map links, and typed rows that reuse those source slots.

use crate::native_world::{
    EntitySnapshot, Level2DynamicArenaKind, Level2EconomyState, level2_nyphur_source_identity_parts,
};
use std::collections::BTreeSet;
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

/// The single owner of imported source records and dynamic slot sidecars.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceArena {
    pub(crate) entities: Box<[EntitySnapshot]>,
    pub(crate) economy: Level2EconomyState,
}

impl SourceArena {
    #[must_use]
    pub(crate) fn new(entities: Box<[EntitySnapshot]>, economy: Level2EconomyState) -> Self {
        Self { entities, economy }
    }

    #[must_use]
    pub(crate) fn entities(&self) -> &[EntitySnapshot] {
        &self.entities
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
            self.economy.authored_bees().iter().filter(|source| {
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
    use crate::{PlayerCommand, native_world::NativeWorld};
    use std::path::PathBuf;

    fn level_two_session() -> crate::native_world::Level2ResetSession {
        let package = std::env::var_os("MC_LEVEL2_PACKAGE_V5").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/l2-debug3.fJ3PaM/level2")
            },
            PathBuf::from,
        );
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component")
            .level2_reset_session()
            .expect("qualified Level 2 source arena session")
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
