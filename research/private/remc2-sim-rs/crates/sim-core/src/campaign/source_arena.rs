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
                    source.active && source.class == 5 && matches!(source.action, 29 | 232)
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
        if !source.active {
            return None;
        }
        let handle = SourceEntityHandle {
            source_slot,
            generation: self.economy.slot_generation(source_slot),
        };
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
        let dynamic_handle = SourceEntityHandle {
            source_slot,
            generation: self.economy.slot_generation(source_slot),
        };

        if let Some(source) = self.economy.authored_class5().iter().find(|source| {
            (include_retained || source.active) && source.source_entity_index == source_slot
        }) {
            let &kind = authored_bindings.get(&source.authored_index)?;
            return Some(CreatureRuntimeIdentity {
                handle: dynamic_handle,
                kind,
                root_slot: source_slot,
                role: CreatureRuntimeRole::Root,
                origin: CreatureRootOrigin::AuthoredRecord {
                    authored_record: source.authored_index,
                },
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
                origin: CreatureRootOrigin::HouseSpawn {
                    house_slot: source.parent_house_entity_index,
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
        if !source.active || !matches!(self.resolve(handle), Some(SourceEntityView::Imported(_))) {
            return None;
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
            .map(|entity| (entity.index, entity.record_bytes()[74]))
            .chain(
                self.economy
                    .authored_model45()
                    .iter()
                    .filter(|source| source.active)
                    .map(|source| (source.source_entity_index, source.byte_0x46)),
            )
            .collect::<Vec<_>>();
        rows.sort_unstable_by_key(|(source_slot, _)| *source_slot);
        rows.into_iter().map(|(_, selector)| selector).collect()
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
        let mut include = |source_slot: u16, model: u8, action: u8, _life: i32| {
            // Source class-model membership retains lethal rows through their
            // visible death callback. Hide/reclaim, not the negative life
            // mailbox write, removes them from absence-objective census.
            if matches!(action, 180 | 232 | 234) {
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
        native_world::NativeWorld,
    };
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
    fn creature_runtime_census_binds_level_one_two_and_three_roots_and_parts() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let cases = [
            (
                manifest_dir.join("../../../generated/mc-cf15-level1-package-v3-current"),
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
