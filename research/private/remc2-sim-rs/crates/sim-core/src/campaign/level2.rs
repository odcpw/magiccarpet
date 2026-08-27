// Moved mechanically from native_world.rs for CF-01. This file is included as a child of native_world so its internal visibility and parent state access remain unchanged.
use super::*;

use std::{collections::BTreeMap, sync::Arc};

use crate::campaign::source_arena::SourceArena;
use crate::campaign::state::{
    CampaignDefinition, CampaignSession, CampaignState, CampaignStepError, CanonicalRuntimeState,
    ResetReceipt, ResetState, SemanticLevelBindings, digest_level_exit_sequence,
    encode_reset_v1_into,
};
use crate::campaign::{
    dispatch::{CAMPAIGN_BEHAVIOR_REGISTRY, DispatchSite, ExistingHandler},
    schedule::{
        CurrentOccupant, PassPhase, ScheduleError, ScheduleReceipt, SlotResolution, SourcePass,
        SourceSchedulerWorld, VisitKind, VisitPlan, run_update_entities,
    },
    spec::{BehaviorKey, BehaviorSelector},
};
use crate::level_definition::{
    BuildingKind, CreatureKind, ItemKind, MechanicKind, ObjectiveCondition, SceneryKind, SpellKind,
    SwitchKind,
};
use crate::{PLAYER_MIN_SPEED, SpellHolder};

/// Typed payload selected from the fixed numeric source slot at visit time.
///
/// This replaces the former friendly `u8` route tags.  The payload locates
/// the authoritative carrier; the compiled registry remains the authority
/// that selects which accepted handler family may receive it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Level2SourceRoute {
    SpeedPickup,
    BadStone,
    House,
    Villager,
    Dolmen,
    ExitSwitch,
    CastleRoot,
    Dynamic(Level2DynamicArenaCursorRow),
    Goat,
    /// A package-imported class-15 row. The row starts as an ordinary pickup
    /// and becomes the active spell holder in place after acquisition.
    ImportedSpellSource,
    Player,
    Nyphur,
    PossessionHolder,
    CastleHolder,
    FireballHolder,
    /// A descriptor-backed lossless source row whose behavior is selected by
    /// its exact registry key rather than a level-specific typed slot.
    Imported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Level2ResolvedSourceVisit {
    key: BehaviorKey,
    handler: ExistingHandler,
    route: Level2SourceRoute,
    phase: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(private_interfaces)]
pub(crate) enum Level2ResolvedSourcePlan {
    Callback(Level2ResolvedSourceVisit),
    /// A live selector whose UpdateEntities table word matches but whose
    /// callback address is null. The numeric cursor retains it unchanged.
    NoCallback {
        key: BehaviorKey,
        route: Level2SourceRoute,
        phase: Option<u8>,
    },
    /// A live row whose class/action selector does not match any entry in its
    /// source action table. `UpdateEntities_57730` disables drawing without
    /// invoking a callback or advancing byte 0x3E, then the following pass
    /// prologue reclaims the row.
    InvalidSelectorHide {
        key: BehaviorKey,
        route: Level2SourceRoute,
        phase: Option<u8>,
    },
    /// A live row whose selector is not admitted by the compiled registry, or
    /// whose retained carrier cannot lawfully feed the selected handler.
    Unknown {
        key: BehaviorKey,
        route: Level2SourceRoute,
        phase: Option<u8>,
        message: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Level2EpiloguePolicy {
    /// The source callback body is deliberately phase-free; the scheduler
    /// advances the current occupant after callback return.
    SchedulerAdvance {
        before: u8,
        route: Level2SourceRoute,
    },
    /// Retained delegates still contain their historical common increment.
    /// The scheduler verifies exactly one advance and supplies it only when a
    /// bounded callback exit returned before that common tail. Dynamic slot
    /// generation distinguishes a retained row from NewEvent replacement.
    EnsureDelegateAdvanced {
        before: u8,
        route: Level2SourceRoute,
        generation: Option<u32>,
    },
}

fn validate_semantic_creature_dispatch(
    source_slot: u16,
    creature_kind: Option<CreatureKind>,
    handler: ExistingHandler,
) -> std::result::Result<(), String> {
    match (creature_kind, handler) {
        (Some(kind), actual) => {
            let expected = match kind {
                CreatureKind::Dragon => None,
                CreatureKind::Goat => Some(&[ExistingHandler::GoatFamily][..]),
                CreatureKind::Bee => Some(&[ExistingHandler::BeeFamily][..]),
                CreatureKind::Worm => Some(&[ExistingHandler::MultipartFamily][..]),
                CreatureKind::Archer => Some(&[ExistingHandler::ArcherFamily][..]),
                CreatureKind::Skeleton => Some(&[ExistingHandler::Model9Family][..]),
                CreatureKind::Villager => Some(
                    &[
                        ExistingHandler::VillagerFamily,
                        ExistingHandler::BuilderVillagerPostBuild,
                    ][..],
                ),
                CreatureKind::Trader => Some(&[ExistingHandler::TraderFamily][..]),
                CreatureKind::Firefly => Some(&[ExistingHandler::FireflyFamily][..]),
            };
            if expected.is_some_and(|handlers| handlers.contains(&actual)) {
                Ok(())
            } else {
                Err(format!(
                    "creature source slot {source_slot} is semantic {}, but the MC2 selector resolved {actual:?}",
                    kind.as_str()
                ))
            }
        }
        (
            None,
            ExistingHandler::GoatFamily
            | ExistingHandler::BeeFamily
            | ExistingHandler::MultipartFamily
            | ExistingHandler::ArcherFamily
            | ExistingHandler::Model9Family
            | ExistingHandler::VillagerFamily
            | ExistingHandler::BuilderVillagerPostBuild
            | ExistingHandler::TraderFamily
            | ExistingHandler::FireflyFamily,
        ) => Err(format!(
            "creature source slot {source_slot} has no package-authored semantic identity"
        )),
        _ => Ok(()),
    }
}

fn validate_semantic_scenery_dispatch(
    source_slot: u16,
    scenery_kind: Option<SceneryKind>,
    handler: ExistingHandler,
) -> std::result::Result<(), String> {
    match (scenery_kind, handler) {
        (Some(SceneryKind::Tree), ExistingHandler::TreeLifecycle)
        | (Some(SceneryKind::Stone), ExistingHandler::Stone)
        | (Some(SceneryKind::ExitPortal), ExistingHandler::Class14Events) => Ok(()),
        (Some(kind), actual) => Err(format!(
            "scenery source slot {source_slot} is semantic {}, but the MC2 selector resolved {actual:?}",
            kind.as_str()
        )),
        (None, ExistingHandler::TreeLifecycle) => Err(format!(
            "tree source slot {source_slot} has no package-authored semantic identity"
        )),
        _ => Ok(()),
    }
}

fn validate_semantic_item_dispatch(
    source_slot: u16,
    item_kind: Option<ItemKind>,
    raw_model: u8,
    raw_action: u8,
    handler: ExistingHandler,
) -> std::result::Result<(), String> {
    match (item_kind, raw_model, raw_action, handler) {
        (Some(ItemKind::ExperienceScroll), 5, 10, ExistingHandler::Class14Events) => Ok(()),
        (Some(kind), _, _, actual) => Err(format!(
            "item source slot {source_slot} is semantic {}, but the MC2 selector resolved model {raw_model} action {raw_action} through {actual:?}",
            kind.as_str()
        )),
        (None, 5, 10, ExistingHandler::Class14Events) => Err(format!(
            "experience-scroll source slot {source_slot} has no package-authored semantic identity"
        )),
        _ => Ok(()),
    }
}

fn validate_semantic_switch_dispatch(
    source_slot: u16,
    switch_kind: Option<SwitchKind>,
    raw_class: u8,
    raw_model: u8,
    raw_action: u8,
    handler: ExistingHandler,
) -> std::result::Result<(), String> {
    match (switch_kind, raw_class, raw_model, raw_action, handler) {
        (Some(SwitchKind::AreaEntryTrigger), 11, 0, 0, ExistingHandler::SwitchProducers)
        | (Some(SwitchKind::AreaExitTrigger), 11, 1, 1, ExistingHandler::SwitchProducers)
        | (Some(SwitchKind::LevelExitGate), 11, 4, 4, ExistingHandler::SwitchProducers)
        | (Some(SwitchKind::ExitSequenceTrigger), 11, 12, 12, ExistingHandler::SwitchProducers)
        | (Some(SwitchKind::ObjectiveGate), 11, 32, 32, ExistingHandler::ObjectiveSwitch) => Ok(()),
        (Some(kind), _, _, _, actual) => Err(format!(
            "switch source slot {source_slot} is semantic {}, but the MC2 selector resolved class {raw_class} model {raw_model} action {raw_action} through {actual:?}",
            kind.as_str()
        )),
        (None, 11, 32, 32, ExistingHandler::ObjectiveSwitch) => Err(format!(
            "objective-gate source slot {source_slot} has no package-authored semantic identity"
        )),
        (None, 11, 0 | 1 | 4 | 12, _, _) => Err(format!(
            "switch source slot {source_slot} has package-supported model {raw_model} action {raw_action} but no semantic identity"
        )),
        _ => Ok(()),
    }
}

fn validate_semantic_spell_dispatch(
    source_slot: u16,
    spell_kind: Option<SpellKind>,
    expected_family: Option<u8>,
    raw_family: u8,
    handler: ExistingHandler,
) -> std::result::Result<(), String> {
    match (spell_kind, expected_family, handler) {
        (Some(_kind), Some(family), ExistingHandler::SpellHolderAndPickup)
            if family == raw_family =>
        {
            Ok(())
        }
        (Some(kind), expected, actual) => Err(format!(
            "spell source slot {source_slot} is semantic {}, but the MC2 selector resolved family {raw_family} through {actual:?} (expected package family {expected:?})",
            kind.as_str()
        )),
        (None, _, ExistingHandler::SpellHolderAndPickup) => Err(format!(
            "spell source slot {source_slot} has no package-authored semantic identity"
        )),
        _ => Ok(()),
    }
}

fn validate_semantic_building_dispatch(
    source_slot: u16,
    building_kind: Option<BuildingKind>,
    route: Level2SourceRoute,
    raw_action: u8,
    handler: ExistingHandler,
) -> std::result::Result<(), String> {
    let source_is_house = matches!(route, Level2SourceRoute::House);
    let source_is_house_construction = matches!(
        route,
        Level2SourceRoute::Dynamic(Level2DynamicArenaCursorRow {
            kind: Level2DynamicArenaKind::AuthoredModel45,
            ..
        })
    );
    match (building_kind, handler) {
        (Some(BuildingKind::House), ExistingHandler::Class10Effects)
            if (source_is_house && raw_action == 52)
                || (source_is_house_construction && raw_action == 51) =>
        {
            Ok(())
        }
        (Some(kind), actual) => Err(format!(
            "building source slot {source_slot} is semantic {}, but the MC2 selector resolved action {raw_action} through {actual:?}",
            kind.as_str()
        )),
        (None, ExistingHandler::Class10Effects)
            if source_is_house || source_is_house_construction =>
        {
            Err(format!(
                "house source slot {source_slot} has no package-authored semantic identity"
            ))
        }
        _ => Ok(()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Level2CleanupPolicy {
    HideInvalidSelector,
}

impl Level2ResetSession {
    /// Source `sub_159E0`, scheduled after class-list rebuild and before the
    /// numeric `UpdateEntities` cursor. The qualified Level-2 package reaches
    /// both class-9/model-0 Fireball pressure against class-3 targets and
    /// class-9/model-1 Possession pressure against class-10/model-39 mana.
    /// Each resolved projectile is marked before model/owner admission and
    /// can contribute at most once.
    fn step_level2_projectile_threat_prepass(&mut self) {
        #[derive(Clone, Copy)]
        struct ThreatWrite {
            target_owner: u16,
            attacker_color: u8,
            delta: i32,
            latch_threshold: Option<i32>,
        }

        #[derive(Clone, Copy)]
        enum ThreatProjectile {
            Possession(usize),
            Fireball(usize),
        }

        #[derive(Clone, Copy)]
        enum ThreatTarget {
            Class3 { model: u8, owner: u16 },
            ManaSphere { owner: u16, mana: i32 },
            Other,
        }

        let (entities, players, spheres, castle) = {
            let live = &self.session.state.live;
            (
                live.economy.entities.clone(),
                live.core.players.clone(),
                live.economy.economy.spheres().to_vec(),
                live.economy.economy.castle(),
            )
        };
        let mut projectile_indices = self
            .session
            .state
            .live
            .economy
            .economy
            .possession_projectiles
            .iter()
            .enumerate()
            .filter(|(_, projectile)| projectile.active && !projectile.threat_accounted)
            .map(|(index, projectile)| {
                (
                    projectile.source_entity_index,
                    ThreatProjectile::Possession(index),
                )
            })
            .collect::<Vec<_>>();
        projectile_indices.extend(
            self.session
                .state
                .live
                .economy
                .economy
                .fireball_projectiles
                .iter()
                .enumerate()
                .filter(|(_, projectile)| projectile.active && !projectile.threat_accounted)
                .map(|(index, projectile)| {
                    (
                        projectile.source_entity_index,
                        ThreatProjectile::Fireball(index),
                    )
                }),
        );
        projectile_indices.sort_unstable_by_key(|(source_slot, _)| *source_slot);

        let mut writes = Vec::new();
        for (_, projectile_source) in projectile_indices {
            let (projectile_model, projectile_owner, target_slot) = match projectile_source {
                ThreatProjectile::Possession(index) => {
                    let projectile = self
                        .session
                        .state
                        .live
                        .economy
                        .economy
                        .possession_projectiles[index];
                    (
                        projectile.source_model,
                        projectile.source_id_entity_index,
                        projectile.target_source_entity_index,
                    )
                }
                ThreatProjectile::Fireball(index) => {
                    let projectile =
                        self.session.state.live.economy.economy.fireball_projectiles[index];
                    (
                        projectile.source_model,
                        projectile.owner_entity_index,
                        projectile.target_source_entity_index,
                    )
                }
            };
            let owner_is_live_player = entities
                .get(usize::from(projectile_owner))
                .is_some_and(|owner| owner.active && owner.class == 3);
            let target = entities
                .get(usize::from(target_slot))
                .filter(|target| target.index != 0 && target.active)
                .map(|target| match (target.class, target.model) {
                    (3, model) => ThreatTarget::Class3 {
                        model,
                        owner: target.source_id,
                    },
                    (10, 39) => ThreatTarget::ManaSphere {
                        owner: target.source_player_entity_index(),
                        mana: target.mana,
                    },
                    _ => ThreatTarget::Other,
                })
                .or_else(|| {
                    castle
                        .filter(|target| target.active && target.source_entity_index == target_slot)
                        .map(|target| ThreatTarget::Class3 {
                            model: 2,
                            owner: target.source_id_entity_index,
                        })
                })
                .or_else(|| {
                    spheres
                        .iter()
                        .find(|sphere| {
                            sphere.active
                                && !sphere.pending_delete
                                && sphere.source_entity_index == target_slot
                        })
                        .map(|target| ThreatTarget::ManaSphere {
                            owner: target.player_entity_index,
                            mana: target.mana,
                        })
                });
            if !owner_is_live_player || target.is_none() {
                continue;
            }

            // The source sets byte-1 bit 0x20 immediately after resolving the
            // owner and target, before checking the projectile/target models.
            match projectile_source {
                ThreatProjectile::Possession(index) => {
                    self.session
                        .state
                        .live
                        .economy
                        .economy
                        .possession_projectiles[index]
                        .threat_accounted = true;
                }
                ThreatProjectile::Fireball(index) => {
                    self.session.state.live.economy.economy.fireball_projectiles[index]
                        .threat_accounted = true;
                }
            }

            let Some(attacker) = players
                .iter()
                .find(|player| player.entity_index == projectile_owner)
            else {
                continue;
            };
            let attacker_color = attacker.player_color_index();
            let attacker_max_mana = entities
                .get(usize::from(projectile_owner))
                .map_or(0, |owner| owner.max_mana);
            match target.expect("resolved above") {
                ThreatTarget::Class3 { model, owner } => {
                    let Some(target_owner) =
                        players.iter().find(|player| player.entity_index == owner)
                    else {
                        continue;
                    };
                    let heavy = matches!(projectile_model, 3 | 4 | 11);
                    let delta = if model == 2 {
                        if heavy { 5_000 } else { 1_000 }
                    } else if heavy {
                        3_000
                    } else {
                        500
                    };
                    let latch_threshold = (model == 2).then(|| {
                        let pressure = target_owner
                            .type_str_164_auxiliary()
                            .map_or(0, |auxiliary| i32::from(auxiliary.word_0x242_578));
                        50_000 - attacker_max_mana / 10 * pressure / 255
                    });
                    writes.push(ThreatWrite {
                        target_owner: owner,
                        attacker_color,
                        delta,
                        latch_threshold,
                    });
                }
                ThreatTarget::ManaSphere { owner, mana } => {
                    if projectile_model != POSSESSION_PROJECTILE_MODEL
                        || !entities
                            .get(usize::from(owner))
                            .is_some_and(|owner| owner.active && owner.class == 3)
                    {
                        continue;
                    }
                    writes.push(ThreatWrite {
                        target_owner: owner,
                        attacker_color,
                        // Signed division truncates toward zero, matching the
                        // source's sign-corrected quarter-mana expression.
                        delta: mana / 4,
                        latch_threshold: None,
                    });
                }
                ThreatTarget::Other => {}
            }
        }

        for write in writes {
            let live = &mut self.session.state.live;
            let nyphur_owns_target = live.nyphur_dispatcher.as_ref().is_some_and(|dispatcher| {
                dispatcher.common_upkeep.source_slot == write.target_owner
            });
            if nyphur_owns_target {
                let _ = live.nyphur_dispatcher.as_mut().and_then(|dispatcher| {
                    dispatcher.apply_projectile_threat(write.attacker_color, write.delta)
                });
                continue;
            }

            let index = 4 * usize::from(write.attacker_color) + 4;
            let Some(target_player) = live
                .core
                .players
                .iter_mut()
                .find(|player| player.entity_index == write.target_owner)
            else {
                continue;
            };
            let Some(current) = target_player.type_str_164_array_word(index) else {
                continue;
            };
            let next = i32::from(current)
                .saturating_add(write.delta)
                .clamp(0, i32::from(u16::MAX)) as u16;
            let _ = target_player.set_type_str_164_array_word(index, next);
            let set_latch = write
                .latch_threshold
                .is_some_and(|threshold| i32::from(next) > threshold);
            if set_latch {
                let _ = target_player.set_type_str_164_array_word(index + 1, 1);
            }
            for player in &mut live.shared_spells.players {
                if player.entity_index == write.target_owner {
                    let _ = player.set_type_str_164_array_word(index, next);
                    if set_latch {
                        let _ = player.set_type_str_164_array_word(index + 1, 1);
                    }
                }
            }
            if live.shared_spells.local_player.entity_index == write.target_owner {
                let _ = live
                    .shared_spells
                    .local_player
                    .set_type_str_164_array_word(index, next);
                if set_latch {
                    let _ = live
                        .shared_spells
                        .local_player
                        .set_type_str_164_array_word(index + 1, 1);
                }
            }
        }
    }

    fn hide_level2_invalid_selector(
        &mut self,
        source_slot: u16,
    ) -> std::result::Result<(), String> {
        self.session
            .state
            .live
            .economy
            .hide_authored_class5_archer_child_invalid_selector(source_slot)
            .then_some(())
            .ok_or_else(|| {
                format!(
                    "invalid-selector cleanup lost the live Archer child at source slot {source_slot}"
                )
            })
    }

    #[cfg(test)]
    pub(crate) fn cf10_probe_invalid_selector_cleanup(
        &mut self,
        source_slot: u16,
    ) -> std::result::Result<(), String> {
        self.hide_level2_invalid_selector(source_slot)
    }
}

#[cfg(test)]
std::thread_local! {
    static CF10_PRODUCTION_SCHEDULE_RECEIPTS:
        std::cell::RefCell<Vec<ScheduleReceipt<Level2ResolvedSourcePlan>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

struct Level2SourceSchedulerWorld<'session, 'evidence> {
    owner: &'session mut Level2ResetSession,
    pass: u8,
    latched_command: crate::LatchedCommand,
    evidence: &'evidence mut Vec<crate::StepEvidence>,
    source_player_order: Vec<usize>,
    player_entity_index: u16,
    player_position: Position,
    goat_players: Vec<GoatPlayerTarget>,
    pass_start_mana_sphere_slots: BTreeSet<u16>,
    /// Fixed UpdateEntities class-5 model buckets. Membership is rebuilt once
    /// before the numeric scan; callbacks read member fields live.
    pass_start_class5_model_slots: [Vec<u16>; 29],
    /// The same pass-start census across imported and typed class-5 carriers.
    /// Post-pass objectives consume this frozen presence rather than rescanning
    /// rows after callbacks may have hidden or created them.
    pass_start_class5_model_present: [bool; 29],
    pass_start_castle_auxiliary_membership: level2_dynamic_economy::Level2CastleAuxiliaryMembership,
    pass_start_castle_root_slot: Option<u16>,
    goat_begin_events: BTreeMap<u16, Vec<GoatStepEvent>>,
}

fn level2_update_key(class: u8, action: u8) -> BehaviorKey {
    BehaviorKey {
        dispatch_site: DispatchSite::UpdateEntities.id(),
        selector: BehaviorSelector::ClassAction { class, action },
    }
}

impl Level2ResolvedSourcePlan {
    fn route(&self) -> Level2SourceRoute {
        match self {
            Self::Callback(visit) => visit.route,
            Self::NoCallback { route, .. }
            | Self::InvalidSelectorHide { route, .. }
            | Self::Unknown { route, .. } => *route,
        }
    }

    fn phase(&self) -> Option<u8> {
        match self {
            Self::Callback(visit) => visit.phase,
            Self::NoCallback { phase, .. }
            | Self::InvalidSelectorHide { phase, .. }
            | Self::Unknown { phase, .. } => *phase,
        }
    }

    #[cfg(test)]
    pub(crate) fn registry_key(&self) -> &BehaviorKey {
        match self {
            Self::Callback(visit) => &visit.key,
            Self::NoCallback { key, .. }
            | Self::InvalidSelectorHide { key, .. }
            | Self::Unknown { key, .. } => key,
        }
    }

    #[cfg(test)]
    pub(crate) fn observed_phase(&self) -> Option<u8> {
        self.phase()
    }
}

impl SourceSchedulerWorld for Level2SourceSchedulerWorld<'_, '_> {
    type Selector = Level2ResolvedSourcePlan;
    type Epilogue = Level2EpiloguePolicy;
    type Cleanup = Level2CleanupPolicy;
    type Progress = ();
    type Error = String;

    fn phase(
        &mut self,
        pass: SourcePass,
        phase: PassPhase,
    ) -> std::result::Result<(), Self::Error> {
        debug_assert_eq!(pass, SourcePass::UpdateEntities);
        match phase {
            PassPhase::RngPrepass => {
                self.owner.flush_shared_effects();
                self.source_player_order = self.owner.refresh_level2_spell_owner_candidates();
                self.player_entity_index = self.owner.session.state.live.core.state.entity_index;
                self.player_position = self.owner.session.state.live.core.state.position;
                // Retain event chronology across all source slots in this pass;
                // individual handlers must not erase an earlier pickup row.
                self.owner
                    .session
                    .state
                    .live
                    .shared_spells
                    .pickup_events
                    .clear();
                self.goat_players = self.owner.level2_goat_player_targets();
                self.owner.session.state.live.economy.advance_global_rng();
            }
            PassPhase::FollowingPassCleanup => {
                let hidden_house_slots = {
                    let live = &self.owner.session.state.live;
                    live.ordinary
                        .houses()
                        .iter()
                        .filter(|house| {
                            !house.active
                                && live
                                    .economy
                                    .entities
                                    .get(usize::from(house.entity_index))
                                    .is_some_and(|entity| {
                                        entity.active && entity.drawing_disabled()
                                    })
                        })
                        .map(|house| (house.entity_index, house.rng))
                        .collect::<Vec<_>>()
                };
                let hidden_house_slot_set = hidden_house_slots
                    .iter()
                    .map(|(source_slot, _)| *source_slot)
                    .collect::<BTreeSet<_>>();
                let dynamic_slots = self
                    .owner
                    .session
                    .state
                    .live
                    .economy
                    .live_arena_slots()
                    .into_iter()
                    .map(|(slot, _)| slot)
                    .collect::<BTreeSet<_>>();
                let hidden_imported = self
                    .owner
                    .session
                    .state
                    .live
                    .economy
                    .entities
                    .iter()
                    .filter(|entity| {
                        entity.active
                            && entity.drawing_disabled()
                            && !dynamic_slots.contains(&entity.index)
                            && !hidden_house_slot_set.contains(&entity.index)
                    })
                    .map(|entity| (entity.index, entity.entity_rng))
                    .collect::<Vec<_>>();
                for (source_slot, source_rng) in hidden_imported {
                    let live = &mut self.owner.session.state.live;
                    live.economy.entities[usize::from(source_slot)].reclaim_source_row();
                    let reclaimed = live.economy.entities[usize::from(source_slot)].clone();
                    live.shared_spells.sync_pickup_from_authority(&reclaimed);
                    if let Some(root) = live
                        .level2_imported_switch_roots
                        .iter_mut()
                        .find(|root| root.source_slot == source_slot)
                    {
                        // This compatibility mirror is derived from the raw
                        // imported row; it never owns callback scheduling.
                        root.active = false;
                        root.drawing_enabled = false;
                    }
                    live.economy
                        .economy
                        .release_slot(source_slot, Some(source_rng));
                }
                for (source_slot, source_rng) in hidden_house_slots {
                    let live = &mut self.owner.session.state.live;
                    live.economy.entities[usize::from(source_slot)].reclaim_source_row();
                    live.economy
                        .economy
                        .reclaim_house_source_slot_at_source_boundary(source_slot, source_rng);
                }
                let economy = &mut self.owner.session.state.live.economy;
                economy.reclaim_deferred_authored_bees_at_source_boundary();
                economy.reclaim_possession_rows_at_source_boundary();
                economy.reclaim_fireball_rows_at_source_boundary();
                economy.reclaim_death_burst_rows_at_source_boundary();
                economy.reclaim_hidden_authored_model45_rows_at_source_boundary();
                // `UpdateEntities_57730` runs sub_57F20 for hidden rows before
                // rebuilding lists or dispatching any callback.
                economy.reclaim_hidden_castle_rows_at_source_boundary();
                economy.reclaim_hidden_archer_rows_at_source_boundary();
                economy.reclaim_inactive_slots_at_source_boundary();
            }
            PassPhase::RebuildMemberships => {
                let economy = &mut self.owner.session.state.live.economy;
                economy.rebuild_possession_guidance_source_slots();
                economy.rebuild_owner_mana_projection_at_source_boundary();
                let reset_owner_max_mana = self
                    .owner
                    .session
                    .baseline
                    .economy
                    .economy
                    .owner_max_mana
                    .clone();
                economy.rebuild_owner_max_mana_projection(&reset_owner_max_mana);

                self.pass_start_class5_model_slots = economy.rebuilt_class5_model_slots();
                self.pass_start_class5_model_present = std::array::from_fn(|model| {
                    !self.pass_start_class5_model_slots[model].is_empty()
                });
            }
            PassPhase::PreDispatch => {
                self.owner
                    .session
                    .state
                    .live
                    .economy
                    .advance_authored_firefly_stage_rows();
                self.owner.step_level2_projectile_threat_prepass();
                // Authored groups are consumed synchronously by the numeric
                // source callback which fires their class-11 producer.  A
                // live producer row is not itself proof that the callback has
                // fired: eager family scans here used to construct later
                // groups before slot 1, outside remc2's UpdateEntities walk.
                self.owner.sync_level2_model76_targets_from_entities();
                self.pass_start_mana_sphere_slots = self
                    .owner
                    .session
                    .state
                    .live
                    .economy
                    .live_arena_cursor_rows()
                    .into_iter()
                    .filter(|row| row.kind == Level2DynamicArenaKind::ManaSphere)
                    .map(|row| row.source_slot)
                    .collect();
                self.pass_start_castle_auxiliary_membership = self
                    .owner
                    .session
                    .state
                    .live
                    .economy
                    .economy
                    .castle_auxiliary_membership(&self.owner.session.state.live.economy.entities);
                self.pass_start_castle_root_slot = self
                    .owner
                    .session
                    .state
                    .live
                    .economy
                    .castle()
                    .filter(|castle| castle.active)
                    .map(|castle| castle.source_entity_index);
                self.goat_begin_events.clear();
                let begin_events = self
                    .owner
                    .session
                    .state
                    .live
                    .goats
                    .begin_source_turn(&self.goat_players);
                for event in begin_events {
                    if let GoatStepEvent::Reclaimed { entity_index } = event {
                        // The reclaimed row is no longer eligible for the
                        // numeric callback scan, so publish its prologue
                        // receipt at the real reclaim seam rather than
                        // waiting for a callback that cannot occur.
                        self.owner.push_dispatch_effect(Level2DispatchEvent::Goat {
                            pass: self.pass,
                            source_slot: entity_index,
                            events: vec![event].into_boxed_slice(),
                        });
                    } else {
                        self.goat_begin_events.entry(0).or_default().push(event);
                    }
                }
            }
            PassPhase::PostDispatch => {
                self.owner.session.state.live.goats.finish_source_turn();
                self.owner.flush_shared_effects();
            }
        }
        Ok(())
    }

    fn resolve(
        &self,
        _pass: SourcePass,
        _scan: u16,
        slot: u16,
    ) -> SlotResolution<Self::Selector, Self::Epilogue, Self::Cleanup> {
        let Some(plan) = self
            .owner
            .resolve_level2_source_plan(slot, self.player_entity_index)
        else {
            return SlotResolution::Inactive;
        };
        let semantic_error = match &plan {
            Level2ResolvedSourcePlan::Callback(visit) => {
                self.owner.validate_semantic_source_visit(slot, visit).err()
            }
            _ => None,
        };
        match plan {
            Level2ResolvedSourcePlan::Callback(ref visit)
                if semantic_error.is_none()
                    && Level2ResetSession::level2_route_accepts_handler(
                        visit.route,
                        visit.handler,
                    ) =>
            {
                let Some(before) = visit.phase else {
                    let message = format!(
                        "UpdateEntities selector {:?} at source slot {slot} has no authoritative byte-0x3E carrier",
                        visit.key.selector
                    );
                    return SlotResolution::Unknown {
                        selector: Level2ResolvedSourcePlan::Unknown {
                            key: visit.key.clone(),
                            route: visit.route,
                            phase: None,
                            message,
                        },
                    };
                };
                let epilogue = match visit.route {
                    Level2SourceRoute::Nyphur
                    | Level2SourceRoute::Dolmen
                    | Level2SourceRoute::ExitSwitch
                    | Level2SourceRoute::PossessionHolder
                    | Level2SourceRoute::CastleHolder
                    | Level2SourceRoute::FireballHolder => Level2EpiloguePolicy::SchedulerAdvance {
                        before,
                        route: visit.route,
                    },
                    Level2SourceRoute::Player
                        if visit.key.selector
                            == (BehaviorSelector::ClassAction {
                                class: 3,
                                action: 0,
                            }) =>
                    {
                        Level2EpiloguePolicy::SchedulerAdvance {
                            before,
                            route: visit.route,
                        }
                    }
                    Level2SourceRoute::Dynamic(row)
                        if matches!(
                            row.kind,
                            Level2DynamicArenaKind::AuthoredClass5Model3Child
                                | Level2DynamicArenaKind::AuthoredModel32
                                | Level2DynamicArenaKind::AuthoredModel45
                                | Level2DynamicArenaKind::CastleDefender
                                | Level2DynamicArenaKind::CastleProjectile
                                | Level2DynamicArenaKind::FireballProjectile
                                | Level2DynamicArenaKind::ManaCarrier
                                | Level2DynamicArenaKind::Model17SpawnedChild
                                | Level2DynamicArenaKind::Model6SpawnedChild
                                | Level2DynamicArenaKind::BeyondSightPulseEffect
                                | Level2DynamicArenaKind::ManaAttraction
                                | Level2DynamicArenaKind::AuthoredSwitchProducer
                        ) =>
                    {
                        Level2EpiloguePolicy::SchedulerAdvance {
                            before,
                            route: visit.route,
                        }
                    }
                    Level2SourceRoute::Imported => Level2EpiloguePolicy::SchedulerAdvance {
                        before,
                        route: visit.route,
                    },
                    _ => Level2EpiloguePolicy::EnsureDelegateAdvanced {
                        before,
                        route: visit.route,
                        generation: match visit.route {
                            Level2SourceRoute::Dynamic(row) => Some(row.generation),
                            _ => None,
                        },
                    },
                };
                SlotResolution::Eligible(VisitPlan {
                    selector: plan,
                    kind: VisitKind::Callback,
                    epilogue: Some(epilogue),
                    cleanup: None,
                })
            }
            Level2ResolvedSourcePlan::Callback(visit) => {
                let message = semantic_error.unwrap_or_else(|| {
                    format!(
                        "registry handler {:?} cannot consume {:?} for {:?}",
                        visit.handler, visit.route, visit.key.selector
                    )
                });
                SlotResolution::Unknown {
                    selector: Level2ResolvedSourcePlan::Unknown {
                        key: visit.key,
                        route: visit.route,
                        phase: visit.phase,
                        message,
                    },
                }
            }
            Level2ResolvedSourcePlan::NoCallback { .. } => SlotResolution::Eligible(VisitPlan {
                selector: plan,
                kind: VisitKind::NoCallback,
                epilogue: None,
                cleanup: None,
            }),
            Level2ResolvedSourcePlan::InvalidSelectorHide { .. } => {
                SlotResolution::Eligible(VisitPlan {
                    selector: plan,
                    kind: VisitKind::Terminal,
                    epilogue: None,
                    cleanup: Some(Level2CleanupPolicy::HideInvalidSelector),
                })
            }
            Level2ResolvedSourcePlan::Unknown { .. } => SlotResolution::Unknown { selector: plan },
        }
    }

    fn callback(
        &mut self,
        _pass: SourcePass,
        _scan: u16,
        slot: u16,
        selector: &Self::Selector,
    ) -> std::result::Result<(), Self::Error> {
        let Level2ResolvedSourcePlan::Callback(visit) = selector else {
            return Err("scheduler attempted callback for a non-callback selector".to_owned());
        };
        self.owner.dispatch_level2_source_visit(
            self.pass,
            self.latched_command,
            self.evidence,
            slot,
            visit,
            self.player_entity_index,
            &mut self.player_position,
            &self.source_player_order,
            &self.goat_players,
            &self.pass_start_mana_sphere_slots,
            &self.pass_start_class5_model_slots,
            &self.pass_start_castle_auxiliary_membership,
            self.pass_start_castle_root_slot,
            &mut self.goat_begin_events,
        )
    }

    fn epilogue(
        &mut self,
        _pass: SourcePass,
        _scan: u16,
        slot: u16,
        instruction: &Self::Epilogue,
        current: &CurrentOccupant<Self::Selector>,
    ) -> std::result::Result<(), Self::Error> {
        match *instruction {
            Level2EpiloguePolicy::SchedulerAdvance { before, route } => {
                let current_route = match current {
                    CurrentOccupant::Known { selector, .. } => selector.route(),
                    CurrentOccupant::Inactive => route,
                };
                self.owner
                    .advance_level2_outer_owned_phase(slot, current_route)?;
                let after = self
                    .owner
                    .resolve_level2_source_plan(slot, self.player_entity_index)
                    .and_then(|plan| plan.phase())
                    .or_else(|| self.owner.level2_retained_source_phase(slot, current_route))
                    .ok_or_else(|| format!("source slot {slot} lost its phase after epilogue"))?;
                if after != before.wrapping_add(1) {
                    return Err(format!(
                        "scheduler epilogue at source slot {slot} advanced phase {before} -> {after}, expected {}",
                        before.wrapping_add(1)
                    ));
                }
            }
            Level2EpiloguePolicy::EnsureDelegateAdvanced {
                before,
                route,
                generation,
            } => {
                let current_generation = generation.map(|_| {
                    self.owner
                        .session
                        .state
                        .live
                        .economy
                        .economy
                        .slot_generation(slot)
                });
                let replaced = generation
                    .zip(current_generation)
                    .is_some_and(|(original, current)| current != original);
                let current_route = match current {
                    CurrentOccupant::Known { selector, .. } => selector.route(),
                    CurrentOccupant::Inactive if replaced => {
                        let economy = &self.owner.session.state.live.economy.economy;
                        if economy
                            .death_bursts()
                            .iter()
                            .any(|row| row.source_entity_index == slot)
                        {
                            Level2SourceRoute::Dynamic(Level2DynamicArenaCursorRow {
                                source_slot: slot,
                                generation: current_generation.unwrap_or_default(),
                                kind: Level2DynamicArenaKind::DeathBurst,
                            })
                        } else {
                            route
                        }
                    }
                    CurrentOccupant::Inactive => route,
                };
                let mut after = match current {
                    CurrentOccupant::Known { selector, .. } => selector.phase(),
                    CurrentOccupant::Inactive => None,
                }
                .or_else(|| {
                    self.owner
                        .level2_retained_source_phase(slot, current_route)
                })
                .ok_or_else(|| {
                    format!(
                        "delegate-owned epilogue at source slot {slot} has no observable current phase: original={route:?} current={current:?} generation={generation:?}->{current_generation:?}"
                    )
                })?;
                let current_before = if replaced { slot as u8 } else { before };
                if after == current_before {
                    self.owner
                        .advance_level2_outer_owned_phase(slot, current_route)?;
                    after = self
                        .owner
                        .level2_retained_source_phase(slot, current_route)
                        .or_else(|| {
                            self.owner
                                .resolve_level2_source_plan(slot, self.player_entity_index)
                                .and_then(|plan| plan.phase())
                        })
                        .ok_or_else(|| {
                            format!(
                                "source slot {slot} lost its current phase after the scheduler supplied the common epilogue"
                            )
                        })?;
                }
                if after != current_before.wrapping_add(1) {
                    return Err(format!(
                        "delegate-owned epilogue at source slot {slot} advanced phase {current_before} -> {after}, expected {}",
                        current_before.wrapping_add(1)
                    ));
                }
            }
        }
        Ok(())
    }

    fn cleanup(
        &mut self,
        _pass: SourcePass,
        _scan: u16,
        slot: u16,
        instruction: &Self::Cleanup,
        _current: &CurrentOccupant<Self::Selector>,
    ) -> std::result::Result<(), Self::Error> {
        match instruction {
            Level2CleanupPolicy::HideInvalidSelector => {
                self.owner.hide_level2_invalid_selector(slot)
            }
        }
    }

    fn progress(&self, _pass: SourcePass) -> Self::Progress {}

    fn record(&mut self, receipt: ScheduleReceipt<Self::Selector>) {
        #[cfg(test)]
        CF10_PRODUCTION_SCHEDULE_RECEIPTS.with(|receipts| receipts.borrow_mut().push(receipt));
        #[cfg(not(test))]
        let _ = receipt;
    }
}

impl Level2ResetSession {
    #[cfg(test)]
    pub(crate) fn cf10_production_schedule_receipts(
        &self,
    ) -> Vec<ScheduleReceipt<Level2ResolvedSourcePlan>> {
        CF10_PRODUCTION_SCHEDULE_RECEIPTS.with(|receipts| receipts.borrow().clone())
    }

    #[cfg(test)]
    pub(crate) fn cf10_set_model22_child_action(&mut self, source_slot: u16, action: u8) -> bool {
        let Some(child) = self
            .session
            .state
            .live
            .economy
            .economy
            .model22_spawned_children
            .iter_mut()
            .find(|child| child.active && child.source_entity_index == source_slot)
        else {
            return false;
        };
        child.action = action;
        true
    }

    #[cfg(test)]
    pub(crate) fn cf10_first_model22_child_slot(&self) -> Option<u16> {
        self.session
            .state
            .live
            .economy
            .economy
            .model22_spawned_children
            .iter()
            .find(|child| child.active)
            .map(|child| child.source_entity_index)
    }

    #[cfg(test)]
    pub(crate) fn cf10_live_source_phase(&self, source_slot: u16) -> Option<u8> {
        self.resolve_level2_source_plan(
            source_slot,
            self.session.state.live.core.state.entity_index,
        )
        .and_then(|plan| plan.phase())
    }

    #[cfg(test)]
    pub(crate) fn cf07_rng_observation(&self) -> (u32, u32, u32, u16, Box<[(u16, u16)]>) {
        let effect_rng = self
            .session
            .state
            .live
            .core
            .context
            .effects
            .as_ref()
            .map_or(0, |effects| effects.global_rng);
        let entity_streams = self
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .map(|entity| (entity.index, entity.entity_rng))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        (
            self.session.state.live.economy.global_rng,
            self.session.state.live.core.runtime.random_state,
            effect_rng,
            self.canonical_terrain().terrain_rng(),
            entity_streams,
        )
    }

    #[must_use]
    pub(crate) fn source_global_rng(&self) -> u32 {
        self.session.state.live.economy.global_rng
    }

    pub(super) fn level2_bee_class3_targets(&self) -> Vec<(u16, Position, u32)> {
        // `UpdateEntities_57730` rebuilds `dword_38519` from every live
        // class-3 row. Reconstruct that linked order from the lossless
        // `next_relation` fields instead of using entity-pool order.
        let eligible = self
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .filter(|entity| {
                entity.index != 0 && entity.active && entity.class == 3 && entity.life >= 0
            })
            .map(|entity| entity.index)
            .collect::<BTreeSet<_>>();
        let linked = eligible
            .iter()
            .filter_map(|index| {
                self.session.state.live.economy.entities[usize::from(*index)].next_relation
            })
            .collect::<BTreeSet<_>>();
        let Some(mut current) = eligible
            .iter()
            .copied()
            .find(|index| !linked.contains(index))
        else {
            return Vec::new();
        };
        let mut visited = BTreeSet::new();
        let mut targets = Vec::with_capacity(eligible.len());
        while current != 0 && visited.insert(current) {
            let Some(entity) = self
                .session
                .state
                .live
                .economy
                .entities
                .get(usize::from(current))
            else {
                break;
            };
            if eligible.contains(&current) {
                targets.push((
                    entity.index,
                    if entity.index == self.session.state.live.core.state.entity_index {
                        self.session.state.live.core.state.position
                    } else {
                        entity.position
                    },
                    entity.control_flags,
                ));
            }
            current = entity.next_relation.unwrap_or(0);
        }
        targets
    }

    pub(super) fn level2_model3_class3_target(
        &self,
        source: &Level2AuthoredClass5Source,
    ) -> Option<EntitySnapshot> {
        let radius_squared = MODEL3_TARGET_RANGE * MODEL3_TARGET_RANGE;
        let mut nearest: Option<(u32, EntitySnapshot)> = None;
        for (entity_index, position, control_flags) in self.level2_bee_class3_targets() {
            let Some(entity) = self
                .session
                .state
                .live
                .economy
                .entities
                .get(usize::from(entity_index))
            else {
                continue;
            };
            let dx = i32::from(position.x.wrapping_sub(source.position.x).cast_signed());
            let dy = i32::from(position.y.wrapping_sub(source.position.y).cast_signed());
            let distance = dx
                .wrapping_mul(dx)
                .wrapping_add(dy.wrapping_mul(dy))
                .cast_unsigned();
            if distance > radius_squared
                || control_flags & MODEL3_RAW_SOURCE_DISABLED_BIT != 0
                || angle_distance(
                    i32::from(source.yaw),
                    heading_for_positions(source.position, position),
                ) >= MODEL3_TARGET_HALF_ANGLE
            {
                continue;
            }
            if nearest
                .as_ref()
                .is_some_and(|(nearest_distance, _)| distance >= *nearest_distance)
            {
                continue;
            }
            let mut target = entity.clone();
            target.position = position;
            target.control_flags = control_flags;
            nearest = Some((distance, target));
        }
        nearest.map(|(_, target)| target)
    }

    /// Resolve the source `word_0x96` relation used by Bee action 18. The
    /// source falls back to action 17 for a missing, dead, or draw-disabled
    /// target, but retains the stale relation so the next action-17 scan may
    /// overwrite it. The second tuple field is the target's
    /// `array_0x52_82.yaw` height offset, not its orientation yaw; the third
    /// field preserves validity so action 18 can apply the source Z step
    /// before taking its invalid-target fallback.
    pub(super) fn level2_bee_entity_target(
        &self,
        target_index: u16,
    ) -> Option<(Position, i16, bool)> {
        let target = self
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .find(|entity| entity.index == target_index)?;
        let valid = target.active
            && target.life >= 0
            && target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0;
        if target.index == self.session.state.live.core.state.entity_index {
            let local = self
                .session
                .state
                .live
                .economy
                .entities
                .get(usize::from(self.session.state.live.core.state.entity_index))
                .expect("local player entity row");
            Some((
                self.session.state.live.core.state.position,
                local.array_yaw(),
                valid,
            ))
        } else {
            Some((target.position, target.array_yaw(), valid))
        }
    }

    pub(super) fn level2_bee_action18_target(
        &self,
        source: &Level2AuthoredBeeSource,
    ) -> Option<(Position, i16, bool)> {
        source
            .target_entity_index
            .and_then(|target_index| self.level2_bee_entity_target(target_index))
    }

    pub(super) fn level2_archer_target(
        &self,
        entity_index: u16,
    ) -> Option<level2_class5::archer::ArcherTarget> {
        if entity_index == 0 {
            return None;
        }
        if let Some(entity) = self
            .session
            .state
            .live
            .economy
            .entities
            .get(usize::from(entity_index))
            .filter(|entity| entity.active)
        {
            let record = entity.record_bytes();
            return Some(level2_class5::archer::ArcherTarget {
                entity_index: entity.index,
                class: entity.class,
                model: entity.model,
                action: entity.action,
                active: entity.active,
                life: entity.life,
                control_flags: entity.control_flags,
                minimum_speed: entity.minimum_speed(),
                maximum_speed: 0,
                actual_speed: entity.actual_speed,
                dword_0x10: entity.source_dword_10(),
                word_0x248: u16::from_le_bytes([record[58], record[59]]),
                word_0x32: entity.word_32(),
                word_0x96_150: entity.source_word_96(),
                position: entity.position,
            });
        }
        if let Some(source) = self
            .session
            .state
            .live
            .economy
            .authored_class5()
            .iter()
            .find(|source| source.active && source.source_entity_index == entity_index)
        {
            let state = self
                .session
                .state
                .live
                .economy
                .authored_class5_archer()
                .iter()
                .find(|state| state.source_entity_index == entity_index)?;
            return Some(level2_class5::archer::ArcherTarget {
                entity_index,
                class: level2_class5::archer::ARCHER_SOURCE_CLASS,
                model: source.model,
                action: source.action,
                active: source.active,
                life: source.life,
                control_flags: if source.drawing_enabled {
                    0
                } else {
                    ENTITY_DRAW_DISABLED_BIT
                },
                minimum_speed: source.minimum_speed,
                maximum_speed: source.maximum_speed,
                actual_speed: source.actual_speed,
                dword_0x10: source.dword_0x10,
                word_0x248: 0,
                word_0x32: state.word_0x32,
                word_0x96_150: source.word_0x96_150,
                position: source.position,
            });
        }
        if let Some(child) = self
            .session
            .state
            .live
            .economy
            .authored_class5_archer_children()
            .iter()
            .find(|child| child.active && child.source_entity_index == entity_index)
        {
            return Some(level2_class5::archer::ArcherTarget {
                entity_index,
                class: child.source_class,
                model: child.model,
                action: child.action,
                active: child.active,
                life: i32::from(child.life),
                control_flags: child.control_flags
                    | if child.drawing_enabled {
                        0
                    } else {
                        ENTITY_DRAW_DISABLED_BIT
                    },
                minimum_speed: child.minimum_speed,
                maximum_speed: child.maximum_speed,
                actual_speed: child.actual_speed,
                dword_0x10: child.dword_0x10,
                word_0x248: 0,
                word_0x32: child.word_0x32,
                word_0x96_150: child.word_0x96,
                position: child.position,
            });
        }
        self.session
            .state
            .live
            .economy
            .authored_model45()
            .iter()
            .find(|source| source.active && source.source_entity_index == entity_index)
            .map(|source| level2_class5::archer::ArcherTarget {
                entity_index,
                class: source.source_class,
                model: source.model,
                action: source.action,
                active: source.active,
                life: source.life,
                control_flags: source.control_flags,
                minimum_speed: source.minimum_speed,
                maximum_speed: 0,
                actual_speed: source.actual_speed,
                dword_0x10: source.dword_0x10,
                word_0x248: 0,
                word_0x32: 0,
                word_0x96_150: 0,
                position: source.position,
            })
    }

    pub(super) fn level2_archer_entity_rows(&self) -> Vec<level2_class5::archer::ArcherTarget> {
        let mut rows = self
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .filter(|entity| entity.active)
            .filter_map(|entity| self.level2_archer_target(entity.index))
            .collect::<Vec<_>>();
        rows.extend(
            self.session
                .state
                .live
                .economy
                .authored_class5()
                .iter()
                .filter(|source| {
                    source.active && source.model == level2_class5::archer::ARCHER_MODEL
                })
                .filter_map(|source| self.level2_archer_target(source.source_entity_index)),
        );
        rows.extend(
            self.session
                .state
                .live
                .economy
                .authored_class5_archer_children()
                .iter()
                .filter(|child| child.active)
                .filter_map(|child| self.level2_archer_target(child.source_entity_index)),
        );
        rows
    }

    pub(super) fn level2_archer_linked_rows(
        &self,
        linked_head: u16,
    ) -> Vec<level2_class5::archer::ArcherLinkedState> {
        let mut rows = Vec::new();
        let mut current = linked_head;
        let mut visits = 0_usize;
        let bound = self
            .session
            .state
            .live
            .economy
            .authored_class5_archer_children()
            .len()
            + 1;
        while current != 0 && visits <= bound {
            if let Some(entity) = self
                .session
                .state
                .live
                .economy
                .entities
                .get(usize::from(current))
                .filter(|entity| entity.active)
            {
                rows.push(level2_class5::archer::ArcherLinkedState {
                    entity_index: entity.index,
                    next_entity_index: entity.word_34(),
                    life: entity.life,
                    current_attacker: entity.word_26(),
                });
                current = entity.word_34();
            } else if let Some(child) = self
                .session
                .state
                .live
                .economy
                .authored_class5_archer_children()
                .iter()
                .find(|child| child.active && child.source_entity_index == current)
            {
                rows.push(level2_class5::archer::ArcherLinkedState {
                    entity_index: child.source_entity_index,
                    next_entity_index: child.word_0x34,
                    life: i32::from(child.life),
                    current_attacker: child.word_0x26,
                });
                current = child.word_0x34;
            } else {
                break;
            }
            visits = visits.wrapping_add(1);
        }
        rows
    }

    pub(super) fn apply_level2_archer_target(
        &mut self,
        target: level2_class5::archer::ArcherTarget,
    ) {
        if let Some(entity) = self
            .session
            .state
            .live
            .economy
            .entities
            .get_mut(usize::from(target.entity_index))
        {
            entity.set_source_dword_10(target.dword_0x10);
            return;
        }
        if let Some(source) = self
            .session
            .state
            .live
            .economy
            .authored_class5
            .iter_mut()
            .find(|source| source.source_entity_index == target.entity_index)
        {
            source.dword_0x10 = target.dword_0x10;
            return;
        }
        if let Some(child) = self
            .session
            .state
            .live
            .economy
            .authored_class5_archer_children
            .iter_mut()
            .find(|child| child.source_entity_index == target.entity_index)
        {
            child.dword_0x10 = target.dword_0x10;
        }
    }

    /// Project the source class-5 model list consumed by `sub_1C310`'s
    /// peer-steering pass. `UpdateEntities_57730` excludes dead rows and the
    /// action 180/232/234 no-call states, retains source-arena order, and
    /// appends dynamically allocated authored Bees in allocation order. The
    /// source compares `id_0x1A_26`, not the pool slot.
    pub(super) fn level2_bee_peer_rows(&self, model: u8) -> Vec<(u16, Position)> {
        let mut peers = self
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .filter(|entity| {
                entity.active
                    && entity.life >= 0
                    && entity.class == 5
                    && entity.model == model
                    && !matches!(entity.action, 0xB4 | 0xE8 | 0xEA)
            })
            .map(|entity| (entity.source_id, entity.position))
            .collect::<Vec<_>>();
        peers.extend(
            self.session
                .state
                .live
                .economy
                .authored_bees()
                .iter()
                .filter(|source| source.active && source.life >= 0 && source.model == model)
                .map(|source| (source.entity_id, source.position)),
        );
        peers
    }

    pub(super) fn level2_trader_target(
        &self,
        entity_index: u16,
    ) -> Option<level2_class5::trader::TraderTarget> {
        if entity_index == 0 {
            return None;
        }
        if let Some(entity) = self
            .session
            .state
            .live
            .economy
            .entities
            .get(usize::from(entity_index))
        {
            return Some(level2_class5::trader::TraderTarget {
                entity_index: entity.index,
                class: entity.class,
                model: entity.model,
                active: entity.active,
                minimum_speed: entity.minimum_speed(),
                dword_0x10: entity.source_dword_10(),
                position: entity.position,
            });
        }
        self.session
            .state
            .live
            .economy
            .authored_model45()
            .iter()
            .find(|source| source.source_entity_index == entity_index)
            .map(|source| level2_class5::trader::TraderTarget {
                entity_index: source.source_entity_index,
                class: source.source_class,
                model: source.model,
                active: source.active,
                minimum_speed: source.minimum_speed,
                dword_0x10: source.dword_0x10,
                position: source.position,
            })
    }

    pub(super) fn apply_level2_trader_target(
        &mut self,
        target: level2_class5::trader::TraderTarget,
    ) {
        if let Some(entity) = self
            .session
            .state
            .live
            .economy
            .entities
            .get_mut(usize::from(target.entity_index))
        {
            entity.set_source_dword_10(target.dword_0x10);
            return;
        }
        if let Some(source) = self
            .session
            .state
            .live
            .economy
            .authored_model45
            .iter_mut()
            .find(|source| source.source_entity_index == target.entity_index)
        {
            source.dword_0x10 = target.dword_0x10;
        }
    }

    pub(super) fn level2_trader_linked_rows(
        &self,
        linked_head: u16,
    ) -> Vec<level2_class5::trader::TraderLinkedState> {
        let mut current = linked_head;
        let mut visited = BTreeSet::new();
        let mut linked = Vec::new();
        while current != 0 && visited.insert(current) {
            let Some(entity) = self
                .session
                .state
                .live
                .economy
                .entities
                .get(usize::from(current))
            else {
                break;
            };
            linked.push(level2_class5::trader::TraderLinkedState {
                entity_index: entity.index,
                next_entity_index: entity.word_34(),
                life: entity.life,
                current_attacker: entity.word_26(),
                model: entity.model,
            });
            current = entity.word_34();
        }
        linked
    }

    pub(super) fn level2_trader_building_candidates(
        &self,
    ) -> Vec<level2_class5::trader::TraderBuildingCandidate> {
        let eligible = self
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .filter(|entity| entity.index != 0 && entity.active && entity.class == 10)
            .map(|entity| entity.index)
            .collect::<BTreeSet<_>>();
        let linked = eligible
            .iter()
            .filter_map(|index| {
                self.session
                    .state
                    .live
                    .economy
                    .entities
                    .get(usize::from(*index))
            })
            .filter_map(|entity| entity.next_relation)
            .collect::<BTreeSet<_>>();
        let Some(mut current) = eligible
            .iter()
            .copied()
            .find(|index| !linked.contains(index))
        else {
            return Vec::new();
        };
        let mut visited = BTreeSet::new();
        let mut buildings = Vec::new();
        while current != 0 && visited.insert(current) {
            let Some(entity) = self
                .session
                .state
                .live
                .economy
                .entities
                .get(usize::from(current))
            else {
                break;
            };
            if eligible.contains(&current) && entity.model == HOUSE_MODEL {
                let building_index = entity.level_gate.cast_unsigned();
                buildings.push(level2_class5::trader::TraderBuildingCandidate {
                    entity_index: entity.index,
                    position: entity.position,
                    eligible: HouseParameter::level_two(building_index)
                        .is_some_and(HouseParameter::produces_villagers),
                });
            }
            current = entity.next_relation.unwrap_or(0);
        }
        buildings
    }

    pub(super) fn level2_goat_player_targets(&self) -> Vec<GoatPlayerTarget> {
        // remc2's `dword_38519` is the live class-3 source list, not an
        // entity-pool sweep.  Action 10 narrows that list to model-0 rows
        // before applying its strict cone/distance predicate. Preserve the
        // remaining linked-list order because it is the tie-breaker for equal
        // distances. The qualified reset's list starts at the first live
        // player and each entity carries the next relation.
        let eligible = self
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .filter(|entity| {
                entity.index != 0
                    && entity.active
                    && entity.class == 3
                    && entity.model == 0
                    && entity.life >= 0
            })
            .map(|entity| entity.index)
            .collect::<BTreeSet<_>>();
        let linked = eligible
            .iter()
            .filter_map(|index| {
                self.session.state.live.economy.entities[usize::from(*index)].next_relation
            })
            .collect::<BTreeSet<_>>();
        let Some(mut current) = eligible
            .iter()
            .copied()
            .find(|index| !linked.contains(index))
        else {
            return Vec::new();
        };
        let mut visited = BTreeSet::new();
        let mut targets = Vec::with_capacity(eligible.len());
        while current != 0 && visited.insert(current) {
            let Some(entity) = self
                .session
                .state
                .live
                .economy
                .entities
                .get(usize::from(current))
            else {
                break;
            };
            if eligible.contains(&current) {
                targets.push(GoatPlayerTarget {
                    entity_index: entity.index,
                    position: if entity.index == self.session.state.live.core.state.entity_index {
                        self.session.state.live.core.state.position
                    } else {
                        entity.position
                    },
                    control_flags: entity.control_flags,
                });
            }
            current = entity.next_relation.unwrap_or(0);
        }
        targets
    }

    fn firefly_attack_target(
        &self,
        entity_index: u16,
    ) -> Option<level2_class5::firefly::FireflyAttackTarget> {
        let entity = self
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .find(|entity| entity.index == entity_index)?;
        let local = entity_index == self.session.state.live.core.state.entity_index;
        Some(level2_class5::firefly::FireflyAttackTarget {
            entity_index,
            position: if local {
                self.session.state.live.core.state.position
            } else {
                entity.position
            },
            yaw: if local {
                self.session.state.live.core.state.yaw
            } else {
                entity.yaw
            },
            live: entity.active
                && entity.life >= 0
                && entity.class == 3
                && entity.model == 0
                && !entity.drawing_disabled(),
        })
    }

    /// Refresh the shared spell scan from the live source entity arena at the
    /// source-cursor boundary. Movement has already committed to `state`, so
    /// the local source row is synchronized before the linked-list walk; the
    /// remaining candidate projections receive only the source-owned fields
    /// used by `sub_68FF0`, leaving holder/mana state in its existing owner.
    pub(super) fn refresh_level2_spell_owner_candidates(&mut self) -> Vec<usize> {
        let local_entity_index = self.session.state.live.core.state.entity_index;
        let local_position = self.session.state.live.core.state.position;
        if let Some(local_entity) = self
            .session
            .state
            .live
            .economy
            .entities
            .iter_mut()
            .find(|entity| entity.index == local_entity_index)
        {
            local_entity.set_position(local_position);
        }
        for player_index in 0..self.session.state.live.shared_spells.players.len() {
            let entity_index =
                self.session.state.live.shared_spells.players[player_index].entity_index;
            let Some(live_entity) = self
                .session
                .state
                .live
                .economy
                .entities
                .iter()
                .find(|entity| entity.index == entity_index)
                .cloned()
            else {
                continue;
            };
            if let Some(projected) = self
                .session
                .state
                .live
                .shared_spells
                .player_entities
                .get_mut(player_index)
            {
                sync_level2_spell_candidate_projection(projected, &live_entity);
            }
            if entity_index == local_entity_index {
                sync_level2_spell_candidate_projection(
                    &mut self.session.state.live.shared_spells.player_entity,
                    &live_entity,
                );
            }
        }

        // `next_relation` is the source list's order carrier. Keep every
        // active class-3 row in the traversal, then admit only live model-0
        // rows that map to a shared player snapshot. There is deliberately no
        // pool-order fallback when the source chain is absent or malformed.
        let chain_entities = self
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .filter(|entity| entity.index != 0 && entity.active && entity.class == 3)
            .map(|entity| entity.index)
            .collect::<BTreeSet<_>>();
        let linked = chain_entities
            .iter()
            .filter_map(|index| {
                self.session
                    .state
                    .live
                    .economy
                    .entities
                    .get(usize::from(*index))
            })
            .filter_map(|entity| entity.next_relation)
            .collect::<BTreeSet<_>>();
        let Some(mut current) = chain_entities
            .iter()
            .copied()
            .find(|index| !linked.contains(index))
        else {
            return Vec::new();
        };
        let mut visited = BTreeSet::new();
        let mut player_order = Vec::new();
        while current != 0 && visited.insert(current) {
            let Some(entity) = self
                .session
                .state
                .live
                .economy
                .entities
                .get(usize::from(current))
            else {
                break;
            };
            if chain_entities.contains(&current)
                && entity.model == 0
                && entity.life >= 0
                && let Some(player_index) = self
                    .session
                    .state
                    .live
                    .shared_spells
                    .players
                    .iter()
                    .position(|player| player.entity_index == current)
            {
                player_order.push(player_index);
            }
            current = entity.next_relation.unwrap_or(0);
        }
        player_order
    }

    /// Commit only the spell holder(s) the latched input could have mutated.
    ///
    /// Source handlers run before the generic entity pass.  Writing every
    /// pre-source `SpellFireContext` holder back afterwards would replay stale
    /// timer state over a source-owned pulse (notably family 11's 181 -> 180
    /// transition).  The command flags identify the only holder mirrors the
    /// generic pass is permitted to hand back to the source authority.
    pub(super) fn sync_level2_spell_fire_authority(&mut self, command_flags: u8) {
        let Some(fire) = self.session.state.live.core.context.spell_fire.as_ref() else {
            return;
        };
        let mut holder_indices = Vec::with_capacity(3);
        for (flag, holder_index) in [
            (0x10, fire.left_holder_index),
            (0x20, fire.right_holder_index),
            (0x40, fire.selected_holder_index),
        ] {
            if command_flags & flag != 0 && !holder_indices.contains(&holder_index) {
                holder_indices.push(holder_index);
            }
        }
        let holders = fire
            .holders
            .iter()
            .copied()
            .filter(|holder| holder_indices.contains(&holder.entity_index))
            .collect::<Vec<_>>();
        if holders.is_empty() {
            return;
        }
        let caster = fire.caster;
        self.session.state.live.shared_spells.caster = caster;
        self.session.state.live.shared_spells.player_mana = caster.mana;
        self.session
            .state
            .live
            .shared_spells
            .player_entity
            .set_mana(caster.mana);
        self.session
            .state
            .live
            .shared_spells
            .player_entity
            .control_flags = caster.control_flags;
        self.session.state.live.shared_spells.player_entity.record[16..20]
            .copy_from_slice(&caster.control_flags.to_le_bytes());
        for entity in &mut self.session.state.live.shared_spells.player_entities {
            if entity.index == caster.entity_index {
                entity.set_mana(caster.mana);
                entity.control_flags = caster.control_flags;
                entity.record[16..20].copy_from_slice(&caster.control_flags.to_le_bytes());
            }
        }
        for holder in &holders {
            for authoritative in self
                .session
                .state
                .live
                .shared_spells
                .pickups
                .iter_mut()
                .chain(self.session.state.live.shared_spells.holders.iter_mut())
                .chain(
                    self.session
                        .state
                        .live
                        .shared_spells
                        .dynamic_pickups
                        .iter_mut(),
                )
            {
                if authoritative.index != holder.entity_index {
                    continue;
                }
                authoritative.active_value = holder.active_value;
                authoritative.activation_value = holder.activation_value;
                authoritative.action_marker = holder.action_marker;
                authoritative.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
                authoritative.record[52..54]
                    .copy_from_slice(&holder.activation_value.to_le_bytes());
                authoritative.record[64] = holder.action_marker.cast_unsigned();
            }
        }
        self.session
            .state
            .live
            .shared_spells
            .sync_shield_from_spell_fire(caster, &holders);
        self.session
            .state
            .live
            .shared_spells
            .sync_invisible_from_spell_fire(caster, &holders);
        self.session
            .state
            .live
            .shared_spells
            .sync_beyond_sight_from_spell_fire(caster, &holders);
    }

    /// Persist the source ordinary menu commands after `latch_command` has
    /// applied them to the transient Fire context. The generic latch owns the
    /// command ABI projection; this Level 2 seam owns only the local acquired
    /// holder, its raw player record, and the cold/active SetSpell distinction.
    /// Remote or unowned relations fail closed and cannot borrow a local row.
    pub(super) fn reconcile_level2_spell_menu_command(
        &mut self,
        command: PlayerCommand,
    ) -> std::result::Result<(), (u16, String)> {
        let is_menu_command = matches!(command.opcode, 31 | 32 | 40 | 41);
        if !is_menu_command {
            return Ok(());
        }
        let Some(fire_snapshot) = self
            .session
            .state
            .live
            .core
            .context
            .spell_fire
            .as_ref()
            .cloned()
        else {
            return Ok(());
        };

        let (ui_index, requested_tier, hand_side) = match command.opcode {
            31 | 32 => {
                let Ok(ui_index) = usize::try_from(command.argument_1) else {
                    return Ok(());
                };
                if SOURCE_SPELL_INDEX_MAP.get(ui_index).is_none() {
                    return Ok(());
                }
                (ui_index, command.argument_2, Some(command.opcode == 31))
            }
            40 => {
                let Ok(ui_index) = usize::try_from(command.argument_1) else {
                    return Ok(());
                };
                let Some(&family) = SOURCE_SPELL_INDEX_MAP.get(ui_index) else {
                    return Ok(());
                };
                (
                    ui_index,
                    fire_snapshot.family_selected_tiers[usize::from(family)],
                    None,
                )
            }
            41 => {
                let ui_index = usize::from(fire_snapshot.selected_spell_ui_index);
                if SOURCE_SPELL_INDEX_MAP.get(ui_index).is_none() {
                    return Ok(());
                }
                (ui_index, command.argument_1, None)
            }
            _ => return Ok(()),
        };
        let Some(&family) = SOURCE_SPELL_INDEX_MAP.get(ui_index) else {
            return Ok(());
        };
        let family_index = usize::from(family);
        let holder_index = fire_snapshot.enabled_holder_indices[family_index];
        if holder_index == 0
            || !self
                .session
                .state
                .live
                .shared_spells
                .local_player
                .enabled_spell_holders
                .contains(&holder_index)
            || !self
                .session
                .state
                .live
                .shared_spells
                .pickup(holder_index)
                .is_some_and(|holder| {
                    holder.active
                        && holder.class == 15
                        && holder.model == family
                        && holder.source_spell_parent_entity_index()
                            == self
                                .session
                                .state
                                .live
                                .shared_spells
                                .local_player
                                .entity_index
                })
        {
            return Ok(());
        }

        // PlayerEvents_51BB0 preserves opcode-41's raw requested tier.  The
        // source's downward unlocked-tier projection belongs to the later
        // sub_6D9C0 XP recomputation, not this input boundary.
        let selected_tier = requested_tier;
        // `SetSpell_6D5E0` only applies the modeled family profile range for
        // hand assignment (31/32). Opcode 41 is a raw PlayerEvents write;
        // its selected tier remains raw until sub_6D9C0 recomputes the XP
        // projection and applies any downward clamp.
        let physical_tier = if hand_side.is_some() {
            requested_tier.clamp(0, 2)
        } else {
            requested_tier
        };
        let local_player_index = self
            .session
            .state
            .live
            .shared_spells
            .players
            .iter()
            .position(|player| {
                player.entity_index
                    == self
                        .session
                        .state
                        .live
                        .shared_spells
                        .local_player
                        .entity_index
            });
        let Some(local_player_index) = local_player_index else {
            return Ok(());
        };

        if let Some(fire) = self.session.state.live.core.context.spell_fire.as_mut() {
            match command.opcode {
                31 | 32 => {
                    fire.family_selected_tiers[family_index] = selected_tier;
                    if let Some(left) = hand_side {
                        if left {
                            fire.left_holder_index = holder_index;
                            fire.left_subspell_index = selected_tier;
                        } else {
                            fire.right_holder_index = holder_index;
                            fire.right_subspell_index = selected_tier;
                        }
                    }
                    if let Some(holder) = fire
                        .holders
                        .iter_mut()
                        .find(|holder| holder.entity_index == holder_index)
                    {
                        holder.level_gate = physical_tier;
                    }
                }
                40 | 41 => {
                    fire.selected_spell_ui_index = u8::try_from(ui_index)
                        .expect("source spell UI index fits the exported selector");
                    fire.selected_subspell_index = selected_tier;
                    if command.opcode == 41 {
                        fire.family_selected_tiers[family_index] = selected_tier;
                    }
                }
                _ => unreachable!("menu command was checked above"),
            }
        }

        let mut owner = self.session.state.live.shared_spells.players[local_player_index].clone();
        match command.opcode {
            31 | 32 => {
                // The player/hand fields retain the raw selector. SetSpell's
                // physical holder projection is clamped separately below.
                owner.family_selected_tiers[family_index] = selected_tier;
                owner.record[528 + family_index] = selected_tier.cast_unsigned();
                let family_i16 = i16::from(family);
                if hand_side == Some(true) {
                    owner.left_spell_slot_raw = family_i16;
                    owner.left_spell_slot = family;
                    owner.left_subspell_index = selected_tier;
                    owner.record[554..556].copy_from_slice(&family_i16.to_le_bytes());
                    owner.record[558] = selected_tier.cast_unsigned();
                } else {
                    owner.right_spell_slot_raw = family_i16;
                    owner.right_spell_slot = family;
                    owner.right_subspell_index = selected_tier;
                    owner.record[556..558].copy_from_slice(&family_i16.to_le_bytes());
                    owner.record[559] = selected_tier.cast_unsigned();
                }
            }
            40 => {
                owner.selected_spell_ui_index = u8::try_from(ui_index)
                    .expect("source spell UI index fits the exported selector");
                owner.record[560] = command.argument_2.cast_unsigned();
                owner.record[561] = owner.selected_spell_ui_index;
                // Source opcode 0x28 writes only menu stage and UI index. It
                // deliberately leaves subSpellIndex459/record[562] alone.
            }
            41 => {
                owner.selected_spell_ui_index = u8::try_from(ui_index)
                    .expect("source spell UI index fits the exported selector");
                owner.selected_subspell_index = selected_tier;
                owner.record[561] = owner.selected_spell_ui_index;
                // Opcode 0x29 stores the requested argument byte in the raw
                // subspell field and selected-family byte. No physical
                // holder projection occurs here; sub_6D9C0 performs any
                // downward clamp when XP recomputation later runs.
                owner.record[562] = command.argument_1.cast_unsigned();
                owner.family_selected_tiers[family_index] = selected_tier;
                owner.record[528 + family_index] = selected_tier.cast_unsigned();
            }
            _ => unreachable!("menu command was checked above"),
        }
        self.session.state.live.shared_spells.players[local_player_index] = owner.clone();
        self.session.state.live.shared_spells.local_player = owner;

        // Only op31/32 call SetSpell. op40/41 change the menu selection but
        // leave the holder's physical profile and queued word untouched.
        if hand_side.is_none() {
            return Ok(());
        }

        let active_word = self
            .session
            .state
            .live
            .shared_spells
            .pickup(holder_index)
            .map(|holder| {
                u16::from_le_bytes(
                    holder.record[50..52]
                        .try_into()
                        .expect("menu holder active word extent"),
                )
            })
            .unwrap_or_default();
        if let Some(holder) = self
            .session
            .state
            .live
            .shared_spells
            .pickup_mut(holder_index)
        {
            holder.level_gate = physical_tier;
            holder.record[74] = physical_tier.cast_unsigned();
            if active_word != 0 {
                let queued_spell = u16::from(physical_tier.cast_unsigned()) + 1;
                holder.record[48..50].copy_from_slice(&queued_spell.to_le_bytes());
            } else if let Some(profile) =
                level2_set_spell_profile(family, physical_tier.cast_unsigned())
            {
                holder.subspell_index = profile.subspell_index;
                holder.activation_value = profile.word_0x30;
                holder.byte_3b = profile.byte_0x3b;
                holder.mana_regen = profile.mana_regen;
                holder.max_mana = profile.max_mana;
                holder.mana_cost = profile.max_mana;
                holder.mana = profile.mana;
                holder.record[46..48].copy_from_slice(&profile.subspell_index.to_le_bytes());
                holder.record[52..54].copy_from_slice(&profile.word_0x30.to_le_bytes());
                holder.record[63] = profile.byte_0x3b;
                holder.record[140..144].copy_from_slice(&profile.mana_regen.to_le_bytes());
                holder.record[144..148].copy_from_slice(&profile.max_mana.to_le_bytes());
                holder.record[148..152].copy_from_slice(&profile.mana.to_le_bytes());
            }
        }
        if family == BEYOND_SIGHT_FAMILY && active_word == 0 {
            if let Some(profile) = level2_set_spell_profile(family, physical_tier.cast_unsigned())
                && let Some(beyond_sight) =
                    self.session.state.live.shared_spells.beyond_sight.as_mut()
            {
                beyond_sight.tier = physical_tier.cast_unsigned();
                beyond_sight.activation_value = profile.word_0x30.cast_signed();
                beyond_sight.mana_cost = profile.max_mana;
                beyond_sight.mana_regen_requirement = profile.mana_regen;
            }
        }

        let imported_source = self
            .session
            .state
            .live
            .economy
            .entities
            .get(usize::from(holder_index))
            .is_some_and(|entity| {
                entity.active && entity.index == holder_index && entity.class == 15
            })
            && !self
                .session
                .state
                .live
                .economy
                .live_arena_cursor_rows()
                .iter()
                .any(|row| {
                    row.source_slot == holder_index
                        && matches!(
                            row.kind,
                            Level2DynamicArenaKind::SpellPickup
                                | Level2DynamicArenaKind::SpellReplacement
                        )
                });
        if imported_source {
            self.commit_imported_spell_source_projection(holder_index)
                .map_err(|error| (holder_index, error.to_string()))?;
        }
        Ok(())
    }

    /// Source `sub_69640` for the ordinary direct Level 2 holder 116. The
    /// holder's allocation attempt is deliberately separate from its
    /// expenditure/decrement: remc2 advances the countdown even when the
    /// class-9/model-1 arena allocation fails.
    pub(super) fn step_level2_possession_holder_source_slot(
        &mut self,
        holder_index: u16,
    ) -> Option<u16> {
        let Some(holder) = self
            .session
            .state
            .live
            .shared_spells
            .pickup(holder_index)
            .filter(|holder| {
                holder.index == holder_index
                    && holder.active
                    && holder.class == 15
                    && holder.model == POSSESSION_SPELL_FAMILY
                    && holder.action == POSSESSION_SPELL_FAMILY.saturating_mul(3)
                    && holder.source_spell_parent_entity_index()
                        == self.session.state.live.core.state.entity_index
                    && holder.active_value > 0
            })
            .cloned()
        else {
            return None;
        };

        let activation_value = i16::try_from(holder.activation_value).unwrap_or(i16::MAX);
        let mana_regen = if holder.active_value == activation_value {
            Some(
                if self
                    .session
                    .state
                    .live
                    .shared_spells
                    .player_entity
                    .mana_regen
                    >= 0
                {
                    -holder.mana_cost
                } else {
                    self.session
                        .state
                        .live
                        .shared_spells
                        .player_entity
                        .mana_regen
                        .wrapping_sub(holder.mana_cost)
                },
            )
        } else if holder.active_value != 0
            && self
                .session
                .state
                .live
                .shared_spells
                .player_entity
                .mana_regen
                > 0
        {
            Some(0)
        } else {
            None
        };
        if let Some(mana_regen) = mana_regen {
            self.session
                .state
                .live
                .shared_spells
                .player_entity
                .mana_regen = mana_regen;
            self.session.state.live.shared_spells.player_entity.record[140..144]
                .copy_from_slice(&mana_regen.to_le_bytes());
            for entity in &mut self.session.state.live.shared_spells.player_entities {
                if entity.index == self.session.state.live.shared_spells.player_entity.index {
                    entity.mana_regen = mana_regen;
                    entity.record[140..144].copy_from_slice(&mana_regen.to_le_bytes());
                }
            }
        }

        let spawned = if holder.active_value == activation_value && holder.life == 0 {
            // `sub_69900` applies the same `sub_68E50` hand carrier as the
            // Castle source before the model-1 flyer's first guided callback:
            // left/selected moves at yaw-512, right at yaw+512, strict ground
            // penetration rolls the displacement back, then parent FOV is
            // added with source-width wrapping.
            let mut launch_position = self.session.state.live.core.state.position;
            let direction_bits = self
                .session
                .state
                .live
                .shared_spells
                .player_entity
                .control_flags
                & 0x0300;
            let hand_yaw = if direction_bits & 0x0100 != 0 {
                Some(self.session.state.live.core.state.yaw.wrapping_sub(512))
            } else if direction_bits & 0x0200 != 0 {
                Some(self.session.state.live.core.state.yaw.wrapping_add(512))
            } else {
                None
            };
            if let Some(hand_yaw) = hand_yaw {
                crate::move_entity(&mut launch_position, hand_yaw, 0, 256);
                if self.canonical_terrain().ground_height(launch_position) > launch_position.z {
                    launch_position = self.session.state.live.core.state.position;
                }
            }
            launch_position.z = launch_position
                .z
                .wrapping_add(self.session.state.live.core.state.entity_fov);
            let auxiliary = self
                .session
                .state
                .live
                .shared_spells
                .beyond_sight_auxiliary();
            let launch_yaw = auxiliary
                .next_entity
                .wrapping_add(self.session.state.live.core.state.yaw);
            let launch_pitch = auxiliary
                .entity_index2
                .wrapping_add(self.session.state.live.core.state.display_pitch);
            let mut axis_position = self.session.state.live.core.state.position;
            crate::move_entity(&mut axis_position, launch_yaw, launch_pitch, 10_240);
            let player_entity_index = self.session.state.live.core.state.entity_index;
            let actual_speed = self.session.state.live.core.state.actual_speed;
            self.session.state.live.economy.spawn_possession_projectile(
                launch_position,
                player_entity_index,
                holder_index,
                launch_yaw,
                launch_pitch,
                actual_speed,
                axis_position,
            )
        } else {
            None
        };
        let next_active_value = holder.active_value.saturating_sub(1);
        if let Some(authoritative) = self
            .session
            .state
            .live
            .shared_spells
            .pickup_mut(holder_index)
        {
            authoritative.active_value = next_active_value;
            authoritative.record[50..52].copy_from_slice(&next_active_value.to_le_bytes());
        }
        if let Some(fire) = self.session.state.live.core.context.spell_fire.as_mut()
            && let Some(in_flight) = fire
                .holders
                .iter_mut()
                .find(|candidate| candidate.entity_index == holder_index)
        {
            in_flight.active_value = next_active_value;
        }
        spawned
    }

    /// Re-run the queued `SetSpell_6D5E0` write when the Castle holder's
    /// source countdown reaches zero. The active holder keeps its current
    /// physical profile while W is nonzero; only the source cleanup boundary
    /// consumes `word_0x2E` and reinstalls the selected profile.
    pub(super) fn apply_castle_spell_holder_cleanup(&mut self, holder_index: u16) {
        let queued_spell = self
            .session
            .state
            .live
            .shared_spells
            .pickup(holder_index)
            .map(|holder| {
                u16::from_le_bytes(
                    holder.record[48..50]
                        .try_into()
                        .expect("queued Castle SetSpell word extent"),
                )
            })
            .unwrap_or_default();
        if queued_spell != 0 {
            let tier = queued_spell.saturating_sub(1).min(2) as u8;
            if let Some(profile) = level2_direct_starting_spell_profile(CASTLE_SPELL_FAMILY, tier)
                && let Some(holder) = self
                    .session
                    .state
                    .live
                    .shared_spells
                    .pickup_mut(holder_index)
            {
                holder.level_gate = tier.cast_signed();
                holder.subspell_index = profile.subspell_index;
                holder.activation_value = profile.word_0x30;
                holder.byte_3b = profile.byte_0x3b;
                holder.mana_regen = profile.mana_regen;
                holder.max_mana = profile.max_mana;
                holder.mana_cost = profile.max_mana;
                holder.mana = profile.mana;
                holder.record[46..48].copy_from_slice(&profile.subspell_index.to_le_bytes());
                holder.record[48..50].fill(0);
                holder.record[52..54].copy_from_slice(&profile.word_0x30.to_le_bytes());
                holder.record[63] = profile.byte_0x3b;
                holder.record[74] = tier;
                holder.record[140..144].copy_from_slice(&profile.mana_regen.to_le_bytes());
                holder.record[144..148].copy_from_slice(&profile.max_mana.to_le_bytes());
                holder.record[148..152].copy_from_slice(&profile.mana.to_le_bytes());
            }
        }
    }

    /// `sub_6D880` cleanup for the direct family-0 holder.  Keep the profile
    /// lookup explicit: a queued unsupported tier or a non-family-0 row is a
    /// closed no-op rather than a neighboring-family fallback.
    pub(super) fn apply_fireball_spell_holder_cleanup(&mut self, holder_index: u16) {
        let queued_spell = self
            .session
            .state
            .live
            .shared_spells
            .pickup(holder_index)
            .map(|holder| {
                u16::from_le_bytes(
                    holder.record[48..50]
                        .try_into()
                        .expect("queued Fireball SetSpell word extent"),
                )
            })
            .unwrap_or_default();
        if queued_spell == 0 {
            return;
        }
        let Some(tier) = queued_spell
            .checked_sub(1)
            .and_then(|value| u8::try_from(value).ok())
            .filter(|tier| *tier <= 2)
        else {
            return;
        };
        let Some(profile) = level2_direct_starting_spell_profile(FIREBALL_SPELL_FAMILY, tier)
        else {
            return;
        };
        if let Some(holder) = self
            .session
            .state
            .live
            .shared_spells
            .pickup_mut(holder_index)
            .filter(|holder| {
                holder.class == 15 && holder.model == FIREBALL_SPELL_FAMILY && holder.action == 0
            })
        {
            holder.level_gate = tier.cast_signed();
            holder.subspell_index = profile.subspell_index;
            holder.activation_value = profile.word_0x30;
            holder.byte_3b = profile.byte_0x3b;
            holder.mana_regen = profile.mana_regen;
            holder.max_mana = profile.max_mana;
            holder.mana_cost = profile.max_mana;
            holder.mana = profile.mana;
            holder.record[46..48].copy_from_slice(&profile.subspell_index.to_le_bytes());
            holder.record[48..50].fill(0);
            holder.record[52..54].copy_from_slice(&profile.word_0x30.to_le_bytes());
            holder.record[63] = profile.byte_0x3b;
            holder.record[74] = tier;
            holder.record[140..144].copy_from_slice(&profile.mana_regen.to_le_bytes());
            holder.record[144..148].copy_from_slice(&profile.max_mana.to_le_bytes());
            holder.record[148..152].copy_from_slice(&profile.mana.to_le_bytes());
        }
    }

    /// Source `sub_693F0` for the ordinary class-15/model-0/action-0
    /// Fireball holder. The parent gate, allocator, A4 transfer, launch sound,
    /// source charge, W decrement, and word-36 epilogue retain their source
    /// order. In particular, an exhausted projectile allocation still charges
    /// the admitted cast but publishes neither a flyer nor launch sound.
    pub(super) fn step_level2_fireball_holder_source_slot(
        &mut self,
        holder_index: u16,
    ) -> Option<u16> {
        let Some(holder) = self
            .session
            .state
            .live
            .shared_spells
            .pickup(holder_index)
            .filter(|holder| {
                holder.index == holder_index
                    && holder.active
                    && holder.class == 15
                    && holder.model == FIREBALL_SPELL_FAMILY
                    && holder.action == 0
            })
            .cloned()
        else {
            return None;
        };
        let parent_entity_index = self.session.state.live.shared_spells.player_entity.index;
        if holder.source_spell_parent_entity_index() != parent_entity_index {
            return None;
        }

        let mut next_active_value = holder.active_value;
        let mut spawned = None;
        if holder.active_value <= 0 {
            self.apply_fireball_spell_holder_cleanup(holder_index);
        } else {
            let fresh = holder.active_value == holder.activation_value.cast_signed();
            let parent_life = self.session.state.live.shared_spells.player_entity.life;
            let parent_mana = self.session.state.live.shared_spells.player_entity.mana;
            let parent_mana_regen = self
                .session
                .state
                .live
                .shared_spells
                .player_entity
                .mana_regen;
            let castle_mana = self
                .session
                .state
                .live
                .economy
                .castle()
                .filter(|castle| {
                    castle.active
                        && self.session.state.live.economy.player_castle_entity_index()
                            == castle.source_entity_index
                })
                .map(|castle| castle.mana);
            let predicate = holder.level_gate >= 0
                && holder.level_gate <= 2
                && parent_life >= 0
                && parent_mana >= 0
                && (holder.mana_regen == 0
                    || castle_mana.is_some_and(|mana| mana >= holder.mana_regen))
                && (!fresh || parent_mana >= holder.max_mana);
            if !predicate {
                // `sub_693F0` sets W=1 before its unconditional decrement on
                // a failed `sub_68D50` gate.
                next_active_value = 0;
            } else {
                let mana_regen = if fresh {
                    Some(if parent_mana_regen >= 0 {
                        -holder.max_mana
                    } else {
                        parent_mana_regen.wrapping_sub(holder.max_mana)
                    })
                } else if parent_mana_regen > 0 {
                    Some(0)
                } else {
                    None
                };
                if fresh {
                    let mut launch_position = self.session.state.live.core.state.position;
                    let direction_bits = self
                        .session
                        .state
                        .live
                        .shared_spells
                        .player_entity
                        .control_flags
                        & 0x0300;
                    let hand_yaw = if direction_bits & 0x0100 != 0 {
                        Some(self.session.state.live.core.state.yaw.wrapping_sub(512))
                    } else if direction_bits & 0x0200 != 0 {
                        Some(self.session.state.live.core.state.yaw.wrapping_add(512))
                    } else {
                        None
                    };
                    if let Some(hand_yaw) = hand_yaw {
                        crate::move_entity(&mut launch_position, hand_yaw, 0, 256);
                        if self.canonical_terrain().ground_height(launch_position)
                            > launch_position.z
                        {
                            launch_position = self.session.state.live.core.state.position;
                        }
                    }
                    launch_position.z = launch_position
                        .z
                        .wrapping_add(self.session.state.live.core.state.entity_fov);
                    let auxiliary = self
                        .session
                        .state
                        .live
                        .shared_spells
                        .beyond_sight_auxiliary();
                    let launch_yaw = auxiliary
                        .next_entity
                        .wrapping_add(self.session.state.live.core.state.yaw);
                    let launch_pitch = auxiliary
                        .entity_index2
                        .wrapping_add(self.session.state.live.core.state.display_pitch);
                    let mut axis_position = self.session.state.live.core.state.position;
                    crate::move_entity(&mut axis_position, launch_yaw, launch_pitch, 0x4000);
                    let source_dword_0x10 = self
                        .session
                        .state
                        .live
                        .shared_spells
                        .players
                        .iter()
                        .find(|player| player.entity_index == parent_entity_index)
                        .and_then(PlayerSnapshot::type_str_164_auxiliary)
                        .map_or(0, |carrier| i32::from(carrier.byte_0x154_340));
                    let actual_speed = self.session.state.live.core.state.actual_speed;
                    spawned = self
                        .session
                        .state
                        .live
                        .economy
                        .spawn_fireball_projectile_with_payload(
                            level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
                                origin: FireballProjectileOrigin::PlayerSpell,
                                position: launch_position,
                                yaw: launch_yaw,
                                pitch: launch_pitch,
                                axis_position,
                                source_id_entity_index: parent_entity_index,
                                holder_entity_index: holder_index,
                                source_subspell_index: holder.subspell_index,
                                source_mana: holder.mana,
                                source_dword_0x10,
                                target_source_entity_index: 0,
                            },
                            actual_speed,
                        );
                    if let Some(spawned_slot) = spawned {
                        if let Some(player) = self
                            .session
                            .state
                            .live
                            .shared_spells
                            .players
                            .iter_mut()
                            .find(|player| player.entity_index == parent_entity_index)
                        {
                            if let Some(carrier) = player.type_str_164_auxiliary.as_mut() {
                                carrier.byte_0x154_340 = 0;
                                player.record[PLAYER_LEGACY_RECORD_BYTES + 16] = 0;
                            }
                            if self
                                .session
                                .state
                                .live
                                .shared_spells
                                .local_player
                                .entity_index
                                == parent_entity_index
                            {
                                self.session.state.live.shared_spells.local_player = player.clone();
                            }
                        }
                        mark_source_audio_call(SourceAudioCallTag::ProjectileDeliveryLaunch);
                        self.push_flight_effect(FlightEvent::PrepareSound {
                            #[cfg(test)]
                            audio_site: crate::MissionAudioSite::FireballLaunch,
                            entity_index: spawned_slot,
                            player_index: -1,
                            sound_id: 9,
                        });
                    }
                }
                // Source `sub_693F0` attempts the constructor and publishes
                // launch sound first, then charges the parent through
                // `sub_68DE0` even when allocation failed.
                if let Some(mana_regen) = mana_regen {
                    self.session
                        .state
                        .live
                        .shared_spells
                        .player_entity
                        .mana_regen = mana_regen;
                    self.session.state.live.shared_spells.player_entity.record[140..144]
                        .copy_from_slice(&mana_regen.to_le_bytes());
                    for entity in &mut self.session.state.live.shared_spells.player_entities {
                        if entity.index == parent_entity_index {
                            entity.mana_regen = mana_regen;
                            entity.record[140..144].copy_from_slice(&mana_regen.to_le_bytes());
                        }
                    }
                }
                next_active_value = holder.active_value.saturating_sub(1);
            }
        }
        if let Some(authoritative) = self
            .session
            .state
            .live
            .shared_spells
            .pickup_mut(holder_index)
        {
            authoritative.active_value = next_active_value;
            authoritative.record[50..52].copy_from_slice(&next_active_value.to_le_bytes());
        }
        if let Some(holder) = self
            .session
            .state
            .live
            .shared_spells
            .pickup_mut(holder_index)
        {
            let word_36 = u16::from_le_bytes(
                holder.record[58..60]
                    .try_into()
                    .expect("Fireball holder word-36 extent"),
            );
            if word_36 != 0 {
                holder.record[58..60].copy_from_slice(&word_36.saturating_sub(1).to_le_bytes());
            }
        }
        if let Some(fire) = self.session.state.live.core.context.spell_fire.as_mut()
            && let Some(in_flight) = fire
                .holders
                .iter_mut()
                .find(|candidate| candidate.entity_index == holder_index)
        {
            in_flight.active_value = next_active_value;
        }
        spawned
    }

    /// Read the two mutable Type_str_164 orientation words needed by the
    /// Nyphur spell constructors.  The Level 2 player row is the lossless
    /// owner of these words; the common-upkeep carrier owns the separately
    /// ticking byte_0x154 value.  A truncated or non-Level-2 carrier is a
    /// closed producer admission failure.
    pub(super) fn level2_nyphur_type_str_orientation(&self) -> Option<(i16, i16)> {
        let player = self
            .session
            .state
            .live
            .core
            .players
            .iter()
            .find(|player| player.entity_index == LEVEL2_NYPHUR_ENTITY_INDEX)?;
        player.type_str_164_auxiliary.as_ref()?;
        let carrier_start = PLAYER_LEGACY_RECORD_BYTES;
        let next_entity = i16::from_le_bytes(
            player
                .record
                .get(carrier_start + 24..carrier_start + 26)?
                .try_into()
                .ok()?,
        );
        let entity_index2 = i16::from_le_bytes(
            player
                .record
                .get(carrier_start + 26..carrier_start + 28)?
                .try_into()
                .ok()?,
        );
        Some((next_entity, entity_index2))
    }

    /// Apply the queued SetSpell write made by the source holder's terminal
    /// countdown.  Only the canonical fresh Nyphur rows are admitted here;
    /// a malformed queued tier or neighboring family remains untouched.
    pub(super) fn apply_level2_nyphur_holder_cleanup(
        &mut self,
        holder_index: u16,
        family: u8,
        action: u8,
    ) {
        let Some(queued_spell) = self
            .session
            .state
            .live
            .economy
            .entities
            .get(usize::from(holder_index))
            .filter(|holder| {
                holder.active
                    && holder.class == 15
                    && holder.index == holder_index
                    && holder.model == family
                    && holder.action == action
            })
            .map(|holder| u16::from_le_bytes([holder.record[48], holder.record[49]]))
        else {
            return;
        };
        let Some(tier) = queued_spell
            .checked_sub(1)
            .and_then(|tier| u8::try_from(tier).ok())
            .filter(|tier| *tier <= 2)
        else {
            return;
        };
        let Some(profile) = level2_direct_starting_spell_profile(family, tier) else {
            return;
        };
        if let Some(holder) = self
            .session
            .state
            .live
            .economy
            .entities
            .get_mut(usize::from(holder_index))
        {
            holder.apply_nyphur_spell_profile(profile, tier);
            holder.record[48..50].fill(0);
        }
    }

    /// Source `sub_693F0` for the canonical Nyphur class-15/model-0/action-0
    /// holder.  The handler owns only the holder/parent transition and calls
    /// the shared class-9 arena for allocation; the following callback stays
    /// on the generic dynamic cursor.
    pub(super) fn step_level2_nyphur_fireball_holder_source_slot(
        &mut self,
        holder_index: u16,
    ) -> Option<u16> {
        if holder_index != LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX {
            return None;
        }
        let holder = self
            .session
            .state
            .live
            .economy
            .entities
            .get(usize::from(holder_index))
            .filter(|holder| {
                holder.active
                    && holder.index == holder_index
                    && holder.class == 15
                    && holder.model == FIREBALL_SPELL_FAMILY
                    && holder.action == 0
                    && holder.source_spell_parent_entity_index() == LEVEL2_NYPHUR_ENTITY_INDEX
                    && holder.old_map_entity() == LEVEL2_NYPHUR_ENTITY_INDEX
                    && holder.next_entity() == LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX
            })
            .cloned()?;
        let parent = self
            .session
            .state
            .live
            .economy
            .entities
            .get(usize::from(LEVEL2_NYPHUR_ENTITY_INDEX))
            .filter(|parent| {
                parent.active
                    && parent.index == LEVEL2_NYPHUR_ENTITY_INDEX
                    && parent.source_id == LEVEL2_NYPHUR_ENTITY_INDEX
                    && parent.class == 3
                    && parent.model == 1
                    && parent.action == 1
                    && parent.next_entity() == LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX
            })
            .cloned()?;

        let mut next_active_value = holder.active_value;
        let mut spawned = None;
        if holder.active_value > 0 {
            let tier = u8::try_from(holder.level_gate).ok();
            let profile = tier
                .and_then(|tier| level2_direct_starting_spell_profile(FIREBALL_SPELL_FAMILY, tier));
            let profile_matches = profile.as_ref().is_some_and(|profile| {
                profile.subspell_index == holder.subspell_index
                    && profile.word_0x30 == holder.activation_value
                    && profile.byte_0x3b == holder.byte_3b
                    && profile.mana_regen == holder.mana_regen
                    && profile.max_mana == holder.max_mana
                    && profile.mana == holder.mana
            });
            let activation_value = i16::try_from(holder.activation_value).ok();
            let fresh =
                activation_value.is_some_and(|activation| holder.active_value == activation);
            let common_upkeep = self
                .session
                .state
                .live
                .nyphur_dispatcher
                .common_upkeep(parent.phase);
            let castle_mana = (common_upkeep.castle_entity_index != 0)
                .then(|| {
                    self.session
                        .state
                        .live
                        .economy
                        .entities
                        .get(usize::from(common_upkeep.castle_entity_index))
                })
                .flatten()
                .filter(|castle| castle.active && castle.life >= 0)
                .map(|castle| castle.mana);
            let admitted = profile_matches
                && (holder.mana_regen == 0
                    || castle_mana.is_some_and(|mana| mana >= holder.mana_regen))
                && parent.life >= 0
                && parent.mana >= 0
                && (!fresh || parent.mana >= holder.max_mana);
            if !admitted {
                // `sub_693F0` forces W to one before its unconditional
                // decrement when `sub_68D50` rejects the parent.
                next_active_value = 0;
            } else {
                let orientation = if fresh {
                    self.level2_nyphur_type_str_orientation()
                } else {
                    Some((0, 0))
                };
                let Some((next_entity, entity_index2)) = orientation else {
                    return None;
                };
                let parent_mana_regen = parent.mana_regen;
                let mana_regen = if fresh {
                    if parent_mana_regen >= 0 {
                        -holder.max_mana
                    } else {
                        parent_mana_regen.wrapping_sub(holder.max_mana)
                    }
                } else if parent_mana_regen > 0 {
                    0
                } else {
                    parent_mana_regen
                };
                if fresh {
                    let mut launch_position = parent.position;
                    let direction_bits = parent.control_flags & 0x0300;
                    let hand_yaw = if direction_bits & 0x0100 != 0 {
                        Some(parent.yaw.wrapping_sub(512))
                    } else if direction_bits & 0x0200 != 0 {
                        Some(parent.yaw.wrapping_add(512))
                    } else {
                        None
                    };
                    if let Some(hand_yaw) = hand_yaw {
                        crate::move_entity(&mut launch_position, hand_yaw, 0, 256);
                        if self.canonical_terrain().ground_height(launch_position)
                            > launch_position.z
                        {
                            launch_position = parent.position;
                        }
                    }
                    launch_position.z = launch_position.z.wrapping_add(parent.fov);
                    let launch_yaw = next_entity.wrapping_add(parent.yaw);
                    let launch_pitch = entity_index2.wrapping_add(parent.pitch);
                    let mut axis_position = parent.position;
                    crate::move_entity(&mut axis_position, launch_yaw, launch_pitch, 0x4000);
                    let source_dword_0x10 = i32::from(
                        self.session
                            .state
                            .live
                            .nyphur_dispatcher
                            .source_byte_0x154(),
                    );
                    spawned = self
                        .session
                        .state
                        .live
                        .economy
                        .spawn_fireball_projectile_with_payload(
                            level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
                                origin: FireballProjectileOrigin::PlayerSpell,
                                position: launch_position,
                                yaw: launch_yaw,
                                pitch: launch_pitch,
                                axis_position,
                                source_id_entity_index: LEVEL2_NYPHUR_ENTITY_INDEX,
                                holder_entity_index: holder_index,
                                source_subspell_index: holder.subspell_index,
                                source_mana: holder.mana,
                                source_dword_0x10,
                                target_source_entity_index: 0,
                            },
                            parent.actual_speed,
                        );
                    if let Some(spawned_slot) = spawned {
                        self.session
                            .state
                            .live
                            .nyphur_dispatcher
                            .clear_source_byte_0x154();
                        mark_source_audio_call(SourceAudioCallTag::ProjectileDeliveryLaunch);
                        self.push_flight_effect(FlightEvent::PrepareSound {
                            #[cfg(test)]
                            audio_site: crate::MissionAudioSite::FireballLaunch,
                            entity_index: spawned_slot,
                            player_index: -1,
                            sound_id: 9,
                        });
                    }
                }
                // Pinned `sub_693F0` charges the parent only after the
                // constructor/sound attempt. Keep the common carrier and the
                // fixed entity projection in lockstep for the next Nyphur
                // upkeep visit, including allocation failure.
                self.session
                    .state
                    .live
                    .nyphur_dispatcher
                    .apply_holder_mana_regen(mana_regen);
                if let Some(parent) = self
                    .session
                    .state
                    .live
                    .economy
                    .entities
                    .get_mut(usize::from(LEVEL2_NYPHUR_ENTITY_INDEX))
                {
                    parent.set_mana_regen(mana_regen);
                }
                next_active_value = holder.active_value.wrapping_sub(1);
            }
        }

        if let Some(holder) = self
            .session
            .state
            .live
            .economy
            .entities
            .get_mut(usize::from(holder_index))
        {
            holder.active_value = next_active_value;
            holder.record[50..52].copy_from_slice(&next_active_value.to_le_bytes());
            let word_36 = u16::from_le_bytes([holder.record[58], holder.record[59]]);
            if word_36 != 0 {
                holder.record[58..60].copy_from_slice(&word_36.wrapping_sub(1).to_le_bytes());
            }
        }
        if next_active_value == 0 && holder.active_value > 0 {
            self.apply_level2_nyphur_holder_cleanup(holder_index, FIREBALL_SPELL_FAMILY, 0);
        }
        spawned
    }

    /// Source `sub_69640`/`sub_69900` for the canonical Nyphur
    /// class-15/model-1/action-3 holder. Family 1 shares the existing
    /// Possession arena, but the physical tier selects the source's distinct
    /// class-9/model-17 tiered constructor versus the class-9/model-1 basic
    /// constructor. This source-shaped handler owns the launch sound and
    /// countdown writes.
    pub(super) fn step_level2_nyphur_possession_holder_source_slot(
        &mut self,
        holder_index: u16,
    ) -> Option<u16> {
        if holder_index != LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX {
            return None;
        }
        let holder = self
            .session
            .state
            .live
            .economy
            .entities
            .get(usize::from(holder_index))
            .filter(|holder| {
                holder.active
                    && holder.index == holder_index
                    && holder.class == 15
                    && holder.model == POSSESSION_SPELL_FAMILY
                    && holder.action == POSSESSION_SPELL_FAMILY.saturating_mul(3)
                    && holder.source_spell_parent_entity_index() == LEVEL2_NYPHUR_ENTITY_INDEX
                    && holder.old_map_entity() == LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX
                    && holder.next_entity() == 0
            })
            .cloned()?;
        let parent = self
            .session
            .state
            .live
            .economy
            .entities
            .get(usize::from(LEVEL2_NYPHUR_ENTITY_INDEX))
            .filter(|parent| {
                parent.active
                    && parent.index == LEVEL2_NYPHUR_ENTITY_INDEX
                    && parent.source_id == LEVEL2_NYPHUR_ENTITY_INDEX
                    && parent.class == 3
                    && parent.model == 1
                    && parent.action == 1
                    && parent.next_entity() == LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX
            })
            .cloned()?;

        let mut next_active_value = holder.active_value;
        let mut spawned = None;
        if holder.active_value > 0 {
            let physical_tier = u8::try_from(holder.level_gate).ok().map(|tier| tier.min(2));
            let profile = physical_tier.and_then(|tier| {
                level2_direct_starting_spell_profile(POSSESSION_SPELL_FAMILY, tier)
            });
            let profile_matches = profile.is_some_and(|profile| {
                profile.subspell_index == holder.subspell_index
                    && profile.word_0x30 == holder.activation_value
                    && profile.byte_0x3b == holder.byte_3b
                    && profile.mana_regen == holder.mana_regen
                    && profile.max_mana == holder.max_mana
                    && profile.mana == holder.mana
            });
            let activation_value = i16::try_from(holder.activation_value).ok();
            let fresh =
                activation_value.is_some_and(|activation| holder.active_value == activation);
            let admitted = profile_matches
                && holder.mana_regen == 0
                && parent.life >= 0
                && parent.mana >= 0
                && (!fresh || parent.mana >= holder.max_mana);
            if !admitted {
                next_active_value = 0;
            } else {
                let orientation = if fresh {
                    self.level2_nyphur_type_str_orientation()
                } else {
                    Some((0, 0))
                };
                let Some((next_entity, entity_index2)) = orientation else {
                    return None;
                };
                let parent_mana_regen = parent.mana_regen;
                let mana_regen = if fresh {
                    if parent_mana_regen >= 0 {
                        -holder.max_mana
                    } else {
                        parent_mana_regen.wrapping_sub(holder.max_mana)
                    }
                } else if parent_mana_regen > 0 {
                    0
                } else {
                    parent_mana_regen
                };
                if fresh {
                    let mut launch_position = parent.position;
                    let direction_bits = parent.control_flags & 0x0300;
                    let hand_yaw = if direction_bits & 0x0100 != 0 {
                        Some(parent.yaw.wrapping_sub(512))
                    } else if direction_bits & 0x0200 != 0 {
                        Some(parent.yaw.wrapping_add(512))
                    } else {
                        None
                    };
                    if let Some(hand_yaw) = hand_yaw {
                        crate::move_entity(&mut launch_position, hand_yaw, 0, 256);
                        if self.canonical_terrain().ground_height(launch_position)
                            > launch_position.z
                        {
                            launch_position = parent.position;
                        }
                    }
                    launch_position.z = launch_position.z.wrapping_add(parent.fov);
                    let launch_yaw = next_entity.wrapping_add(parent.yaw);
                    let launch_pitch = entity_index2.wrapping_add(parent.pitch);
                    let axis_distance =
                        if physical_tier.expect("profile admission has a physical tier") == 0 {
                            10_240
                        } else {
                            0x4000
                        };
                    let mut axis_position = parent.position;
                    crate::move_entity(&mut axis_position, launch_yaw, launch_pitch, axis_distance);
                    spawned = self
                        .session
                        .state
                        .live
                        .economy
                        .spawn_possession_projectile_with_physical_tier(
                            launch_position,
                            LEVEL2_NYPHUR_ENTITY_INDEX,
                            holder_index,
                            launch_yaw,
                            launch_pitch,
                            parent.actual_speed,
                            axis_position,
                            physical_tier.expect("profile admission has a physical tier"),
                        );
                    if let Some(spawned_slot) = spawned {
                        self.session
                            .state
                            .live
                            .nyphur_dispatcher
                            .clear_source_byte_0x154();
                        mark_source_audio_call(
                            if physical_tier.expect("profile admission has a physical tier") == 0 {
                                SourceAudioCallTag::PossessionLaunchBasic
                            } else {
                                SourceAudioCallTag::PossessionLaunchTiered
                            },
                        );
                        self.push_flight_effect(FlightEvent::PrepareSound {
                            #[cfg(test)]
                            audio_site: crate::MissionAudioSite::PossessionLaunch,
                            entity_index: spawned_slot,
                            player_index: -1,
                            sound_id: 40,
                        });
                    }
                }
                // Pinned `sub_69640` attempts the allocation and emits its
                // launch sound before `sub_68DE0` charges the parent. Keep
                // the common carrier and the fixed entity projection in
                // lockstep so the next Nyphur visit consumes that charge.
                self.session
                    .state
                    .live
                    .nyphur_dispatcher
                    .apply_holder_mana_regen(mana_regen);
                if let Some(parent) = self
                    .session
                    .state
                    .live
                    .economy
                    .entities
                    .get_mut(usize::from(LEVEL2_NYPHUR_ENTITY_INDEX))
                {
                    parent.set_mana_regen(mana_regen);
                }
                next_active_value = holder.active_value.wrapping_sub(1);
            }
        }

        if let Some(holder) = self
            .session
            .state
            .live
            .economy
            .entities
            .get_mut(usize::from(holder_index))
        {
            holder.active_value = next_active_value;
            holder.record[50..52].copy_from_slice(&next_active_value.to_le_bytes());
            let word_36 = u16::from_le_bytes([holder.record[58], holder.record[59]]);
            if word_36 != 0 {
                holder.record[58..60].copy_from_slice(&word_36.wrapping_sub(1).to_le_bytes());
            }
        }
        if next_active_value == 0 && holder.active_value > 0 {
            self.apply_level2_nyphur_holder_cleanup(
                holder_index,
                POSSESSION_SPELL_FAMILY,
                POSSESSION_SPELL_FAMILY.saturating_mul(3),
            );
        }
        spawned
    }

    /// Dispatch the source class-15/model-2/action-6 Castle holder at slot
    /// 117 through `sub_69AB0`. W==0 enters immediate cleanup; a valid fresh
    /// pulse charges before attempting allocation and only a successful
    /// class-9/model-10 allocation writes W=D-1. Allocation failure and an
    /// invalid parent leave W unchanged. The route epilogue owns phase.
    pub(super) fn step_level2_castle_holder_source_slot(
        &mut self,
        holder_index: u16,
    ) -> Option<u16> {
        let Some(holder) = self
            .session
            .state
            .live
            .shared_spells
            .pickup(holder_index)
            .filter(|holder| {
                holder.index == holder_index
                    && holder.active
                    && holder.class == 15
                    && holder.model == CASTLE_SPELL_FAMILY
                    && holder.action == CASTLE_SPELL_FAMILY.saturating_mul(3)
            })
            .cloned()
        else {
            return None;
        };

        let parent_entity_index = self.session.state.live.shared_spells.player_entity.index;
        let parent_relation_valid =
            holder.source_spell_parent_entity_index() == parent_entity_index;
        let mut next_active_value = holder.active_value;
        let mut spawned = None;
        if holder.active_value <= 0 {
            // `sub_6D880` consumes the queued SetSpell word immediately at
            // the zero-W callback. word36 still decrements below.
            self.apply_castle_spell_holder_cleanup(holder_index);
        } else if parent_relation_valid {
            let fresh = holder.active_value == holder.activation_value.cast_signed();
            let parent_life = self.session.state.live.shared_spells.player_entity.life;
            let parent_mana = self.session.state.live.shared_spells.player_entity.mana;
            let parent_mana_regen = self
                .session
                .state
                .live
                .shared_spells
                .player_entity
                .mana_regen;
            let castle_mana = self
                .session
                .state
                .live
                .economy
                .castle()
                .filter(|castle| {
                    castle.active
                        && self.session.state.live.economy.player_castle_entity_index()
                            == castle.source_entity_index
                })
                .map(|castle| castle.mana);
            let predicate = parent_life >= 0
                && parent_mana >= 0
                && (holder.mana_regen == 0
                    || castle_mana.is_some_and(|mana| mana >= holder.mana_regen))
                && (!fresh || parent_mana >= holder.max_mana);
            if !predicate {
                // Source leaves cleanup for the next zero-W callback.
                next_active_value = 0;
            } else if fresh {
                let stage = self
                    .session
                    .state
                    .live
                    .economy
                    .castle()
                    .map_or(1, |castle| castle.level.saturating_add(1));
                // Spells.cpp's Castle profile conversion feeds the source
                // Type_str_164 arrays: stage coefficient is 256 plus the
                // selected profile subspell index shifted by eight, while
                // the controller coefficient is the physical tier byte.
                let stage_coefficient =
                    256_i32.wrapping_add(i32::from(holder.subspell_index as i8) << 8);
                let _ = self
                    .session
                    .state
                    .live
                    .economy
                    .set_castle_stage_coefficient(stage, stage_coefficient as u16);
                let _ = self
                    .session
                    .state
                    .live
                    .economy
                    .set_castle_controller_coefficient(
                        stage,
                        holder.level_gate.clamp(0, 2).cast_unsigned(),
                    );
                // `sub_68DE0` charges before `sub_69AB0` attempts NewEvent;
                // allocation failure therefore retains the charge but leaves
                // W unchanged.
                let mana_regen = if parent_mana_regen >= 0 {
                    -holder.max_mana
                } else {
                    parent_mana_regen.wrapping_sub(holder.max_mana)
                };
                self.session
                    .state
                    .live
                    .shared_spells
                    .player_entity
                    .mana_regen = mana_regen;
                self.session.state.live.shared_spells.player_entity.record[140..144]
                    .copy_from_slice(&mana_regen.to_le_bytes());
                for entity in &mut self.session.state.live.shared_spells.player_entities {
                    if entity.index == parent_entity_index {
                        entity.mana_regen = mana_regen;
                        entity.record[140..144].copy_from_slice(&mana_regen.to_le_bytes());
                    }
                }

                // `sub_69AB0` calls `sub_68E50(parent, flyer, holder)` after
                // NewEvent succeeds.  The selected/left hand displaces the
                // raw parent position by yaw-512, the right hand by yaw+512;
                // a strict ground penetration rolls the displacement back.
                // The parent FOV and two A4 angle words are then copied with
                // source-width wrapping into the new class-9/model-10 row.
                let parent_position = self.session.state.live.core.state.position;
                let mut launch_position = parent_position;
                let direction_bits = self
                    .session
                    .state
                    .live
                    .shared_spells
                    .player_entity
                    .control_flags
                    & 0x0300;
                let hand_yaw = if direction_bits & 0x0100 != 0 {
                    Some(self.session.state.live.core.state.yaw.wrapping_sub(512))
                } else if direction_bits & 0x0200 != 0 {
                    Some(self.session.state.live.core.state.yaw.wrapping_add(512))
                } else {
                    None
                };
                if let Some(hand_yaw) = hand_yaw {
                    crate::move_entity(&mut launch_position, hand_yaw, 0, 256);
                    let terrain = &self.session.state.live.terrain;
                    if terrain.ground_height(launch_position) > launch_position.z {
                        launch_position = self.session.state.live.core.state.position;
                    }
                }
                launch_position.z = launch_position
                    .z
                    .wrapping_add(self.session.state.live.core.state.entity_fov);

                let auxiliary = self
                    .session
                    .state
                    .live
                    .shared_spells
                    .beyond_sight_auxiliary();
                let source_a4_byte_154 = self
                    .session
                    .state
                    .live
                    .shared_spells
                    .players
                    .iter()
                    .find(|player| player.entity_index == parent_entity_index)
                    .and_then(PlayerSnapshot::type_str_164_auxiliary)
                    .map_or(0, |carrier| carrier.byte_0x154_340);
                let launch_yaw = auxiliary
                    .next_entity
                    .wrapping_add(self.session.state.live.core.state.yaw);
                let launch_pitch = auxiliary
                    .entity_index2
                    .wrapping_add(self.session.state.live.core.state.display_pitch);
                let actual_speed = self.session.state.live.core.state.actual_speed;
                let terrain = &self.session.state.live.terrain;
                let mut axis_position = parent_position;
                crate::move_entity(&mut axis_position, launch_yaw, 0, 4_096);
                axis_position.z = terrain.ground_height(axis_position);
                spawned = self
                    .session
                    .state
                    .live
                    .economy
                    .spawn_castle_projectile_with_payload(
                        level2_dynamic_economy::Level2CastleProjectileLaunchPayload {
                            position: launch_position,
                            axis_position,
                            yaw: launch_yaw,
                            pitch: launch_pitch,
                            source_id_entity_index: parent_entity_index,
                            source_subspell_index: holder.subspell_index,
                            source_mana: holder.mana,
                            source_a4_byte_154,
                        },
                        holder_index,
                        actual_speed,
                        terrain,
                    );
                if let Some(spawned_slot) = spawned {
                    // The A4 byte transfer and sound are success-only source
                    // epilogue writes; predicate or allocator failure leaves
                    // the parent byte and audio journal untouched.
                    if let Some(player) = self
                        .session
                        .state
                        .live
                        .shared_spells
                        .players
                        .iter_mut()
                        .find(|player| player.entity_index == parent_entity_index)
                    {
                        if let Some(carrier) = player.type_str_164_auxiliary.as_mut() {
                            carrier.byte_0x154_340 = 0;
                            player.record[PLAYER_LEGACY_RECORD_BYTES + 16] = 0;
                        }
                        if self
                            .session
                            .state
                            .live
                            .shared_spells
                            .local_player
                            .entity_index
                            == parent_entity_index
                        {
                            self.session.state.live.shared_spells.local_player = player.clone();
                        }
                    }
                    mark_source_audio_call(SourceAudioCallTag::InitialCastleLaunch);
                    self.push_flight_effect(FlightEvent::PrepareSound {
                        #[cfg(test)]
                        audio_site: crate::MissionAudioSite::InitialCastleLaunch,
                        entity_index: spawned_slot,
                        player_index: -1,
                        sound_id: 15,
                    });
                    next_active_value = holder.activation_value.cast_signed().wrapping_sub(1);
                }
            }
        }
        if let Some(authoritative) = self
            .session
            .state
            .live
            .shared_spells
            .pickup_mut(holder_index)
        {
            authoritative.active_value = next_active_value;
            authoritative.record[50..52].copy_from_slice(&next_active_value.to_le_bytes());
        }
        // word36 is the package record word at bytes 58..60. It is consumed
        // by every active action-6 callback, including invalid-parent and
        // zero-W cleanup callbacks; bytes 40..42 are the unrelated word24.
        if let Some(holder) = self
            .session
            .state
            .live
            .shared_spells
            .pickup_mut(holder_index)
        {
            let word_36 = u16::from_le_bytes(
                holder.record[58..60]
                    .try_into()
                    .expect("Castle holder word-36 extent"),
            );
            if word_36 != 0 {
                holder.record[58..60].copy_from_slice(&word_36.saturating_sub(1).to_le_bytes());
            }
        }
        if let Some(fire) = self.session.state.live.core.context.spell_fire.as_mut()
            && let Some(in_flight) = fire
                .holders
                .iter_mut()
                .find(|candidate| candidate.entity_index == holder_index)
        {
            in_flight.active_value = next_active_value;
        }
        spawned
    }

    /// `sub_5F890(caster, 0)` clears the originating model-2 holder's
    /// cooldown when `sub_11CB0` rejects the flyer site.
    pub(super) fn clear_castle_spell_holder_cooldown(&mut self, holder_index: u16) {
        for holder in self
            .session
            .state
            .live
            .shared_spells
            .pickups
            .iter_mut()
            .chain(self.session.state.live.shared_spells.holders.iter_mut())
            .chain(
                self.session
                    .state
                    .live
                    .shared_spells
                    .dynamic_pickups
                    .iter_mut(),
            )
        {
            if holder.index == holder_index {
                holder.active_value = 0;
                holder.record[50..52].fill(0);
            }
        }
        // The source rejection occurs before the same outer pass's
        // `sync_level2_spell_fire_authority` write-back.  Clear the in-flight
        // mirror as well so that authority cannot replay the pre-rejection
        // cooldown into the shared holder on that pass.
        if let Some(fire) = self.session.state.live.core.context.spell_fire.as_mut()
            && let Some(holder) = fire
                .holders
                .iter_mut()
                .find(|holder| holder.entity_index == holder_index)
        {
            holder.active_value = 0;
        }
    }

    pub(super) fn sync_level2_spell_fire_context(&mut self) {
        // `SetSpell_6D5E0` owns the player hand relations and may add a
        // dynamic class-15 holder after the reset-time command mirror was
        // constructed. Rebuild that mirror from the authoritative local
        // player relation on every outer-pass boundary: the next ordinary
        // opcode-6/bit-0x40 latch must be able to select a newly acquired
        // holder, while a remote player's acquisition must remain absent from
        // the local command path.
        let local_player = self.session.state.live.shared_spells.local_player.clone();
        let enabled_holder_indices = local_player.enabled_spell_holders;
        let left_slot = usize::from(local_player.left_spell_slot);
        let right_slot = usize::from(local_player.right_spell_slot);
        let left_holder_index = enabled_holder_indices[left_slot];
        let right_holder_index = enabled_holder_indices[right_slot];
        let selected_family = SOURCE_SPELL_INDEX_MAP
            .get(usize::from(local_player.selected_spell_ui_index))
            .copied()
            .unwrap_or(CASTLE_SPELL_FAMILY);
        let selected_holder_index = enabled_holder_indices[usize::from(selected_family)];
        let mut holders = Vec::new();
        for authoritative in self
            .session
            .state
            .live
            .shared_spells
            .pickups
            .iter()
            .chain(self.session.state.live.shared_spells.holders.iter())
            .chain(self.session.state.live.shared_spells.dynamic_pickups.iter())
        {
            if !enabled_holder_indices.contains(&authoritative.index)
                || holders.iter().any(|holder: &SpellActivationHolder| {
                    holder.entity_index == authoritative.index
                })
            {
                continue;
            }
            holders.push(SpellActivationHolder {
                entity_index: authoritative.index,
                model: authoritative.model,
                level_gate: authoritative.level_gate,
                active_value: authoritative.active_value,
                activation_value: authoritative.activation_value,
                mana_cost: authoritative.mana_cost,
                action_marker: authoritative.action_marker,
            });
        }
        let caster = self.session.state.live.shared_spells.caster;
        let spell_experience = self.session.state.live.shared_spells.spell_experience;
        let spell_levels = self.session.state.live.shared_spells.spell_levels;
        let speed_holder_index = enabled_holder_indices[3];
        let speed_holder = holders
            .iter()
            .find(|holder| holder.entity_index == speed_holder_index && holder.model == 3)
            .map(|holder| SpellHolder {
                entity_index: holder.entity_index,
                active_value: holder.active_value,
            });
        self.session.state.live.core.state.speed_spell_holder_index =
            speed_holder.map_or(0, |holder| holder.entity_index);
        self.session.state.live.core.context.speed_spell_holder = speed_holder;
        let Some(fire) = self.session.state.live.core.context.spell_fire.as_mut() else {
            return;
        };
        fire.caster = caster;
        fire.left_holder_index = left_holder_index;
        fire.right_holder_index = right_holder_index;
        fire.selected_holder_index = selected_holder_index;
        fire.left_subspell_index = local_player.left_subspell_index;
        fire.right_subspell_index = local_player.right_subspell_index;
        fire.selected_spell_ui_index = local_player.selected_spell_ui_index;
        fire.selected_subspell_index = local_player.selected_subspell_index;
        fire.spell_hand_eligibility = local_player.spell_hand_eligibility;
        fire.family_selected_tiers = local_player.family_selected_tiers;
        fire.spell_experience = spell_experience;
        fire.spell_levels = spell_levels;
        fire.enabled_holder_indices = enabled_holder_indices;
        fire.holders = holders;
    }

    /// Refresh source-owned holder fields after the source pass without
    /// rebuilding hand or selected-command relations.  The latter were latched
    /// before the pass and must remain intact until the generic command pass
    /// consumes them.
    pub(super) fn refresh_level2_spell_fire_holder_projection(&mut self) {
        let caster = self.session.state.live.shared_spells.caster;
        let holders = self
            .session
            .state
            .live
            .shared_spells
            .pickups
            .iter()
            .chain(self.session.state.live.shared_spells.holders.iter())
            .chain(self.session.state.live.shared_spells.dynamic_pickups.iter())
            .map(|authoritative| SpellActivationHolder {
                entity_index: authoritative.index,
                model: authoritative.model,
                level_gate: authoritative.level_gate,
                active_value: authoritative.active_value,
                activation_value: authoritative.activation_value,
                mana_cost: authoritative.mana_cost,
                action_marker: authoritative.action_marker,
            })
            .collect::<Vec<_>>();
        let Some(fire) = self.session.state.live.core.context.spell_fire.as_mut() else {
            return;
        };
        fire.caster = caster;
        for holder in &mut fire.holders {
            if let Some(authoritative) = holders
                .iter()
                .find(|authoritative| authoritative.entity_index == holder.entity_index)
            {
                *holder = *authoritative;
            }
        }
    }

    pub(super) fn new(
        profile: FlightProfile,
        package_sha256: String,
        state: FlightState,
        terrain: SessionTerrain,
        entities: Box<[EntitySnapshot]>,
        players: Box<[PlayerSnapshot]>,
        runtime: RuntimeSnapshot,
        presentation: PresentationSelectors,
        bad_stones: Box<[Level2BadStone]>,
        shared_spells: Level2SharedSpellResetSession,
        economy: Level2EconomyState,
        ordinary: Level2OrdinaryResetSession,
        goats: GoatFamily,
        mission: MissionContentSession,
        nyphur_dispatcher: NyphurCapability,
        collision_masks: Option<CollisionMaskBank>,
    ) -> Self {
        // Pinned `sub_56D60` reset order (6806c7204d17):
        // ClearSettings -> LevelDecompress -> GenerateLevelMap /
        // GenerateFeatures -> InitStages -> InitStageVars -> Init0x3664C /
        // sub_4A1E0(0,1) -> sub_53160 -> sub_60F00. The package export has
        // already captured the resulting six terrain planes, arenas, runtime
        // stages, and player rows; this constructor only projects that
        // qualified frame zero into the generic flight session.
        let local_player = shared_spells.local_player.clone();
        let caster = shared_spells.caster;
        let enabled_holder_indices = local_player.enabled_spell_holders;
        let selected_family = SOURCE_SPELL_INDEX_MAP
            .get(usize::from(local_player.selected_spell_ui_index))
            .copied()
            .unwrap_or(CASTLE_SPELL_FAMILY);
        let mut holders = Vec::new();
        for holder in shared_spells
            .pickups
            .iter()
            .chain(shared_spells.holders.iter())
            .chain(shared_spells.dynamic_pickups.iter())
        {
            if !enabled_holder_indices.contains(&holder.index)
                || holders
                    .iter()
                    .any(|candidate: &SpellActivationHolder| candidate.entity_index == holder.index)
            {
                continue;
            }
            holders.push(SpellActivationHolder {
                entity_index: holder.index,
                model: holder.model,
                level_gate: holder.level_gate,
                active_value: holder.active_value,
                activation_value: holder.activation_value,
                mana_cost: holder.mana_cost,
                action_marker: holder.action_marker,
            });
        }
        let context = FlightContext {
            effects: Some(FlightEffectsContext {
                global_rng: runtime.random_state,
                local_player_index: runtime.local_player_slot,
                ..FlightEffectsContext::default()
            }),
            spell_fire: Some(SpellFireContext {
                left_holder_index: enabled_holder_indices
                    [usize::from(local_player.left_spell_slot)],
                right_holder_index: enabled_holder_indices
                    [usize::from(local_player.right_spell_slot)],
                selected_holder_index: enabled_holder_indices[usize::from(selected_family)],
                left_subspell_index: local_player.left_subspell_index,
                right_subspell_index: local_player.right_subspell_index,
                selected_spell_ui_index: local_player.selected_spell_ui_index,
                selected_subspell_index: local_player.selected_subspell_index,
                enabled_holder_indices,
                family_selected_tiers: local_player.family_selected_tiers,
                spell_experience: shared_spells.spell_experience,
                spell_levels: shared_spells.spell_levels,
                caster: SpellCasterState {
                    entity_index: caster.entity_index,
                    model: caster.model,
                    mana: caster.mana,
                    control_flags: caster.control_flags,
                    player_color_index: state.player_color_index,
                    control_mode: 0,
                },
                holders,
                ..SpellFireContext::default()
            }),
            ..FlightContext::default()
        };
        let initial_level2_objective_stage_state = runtime
            .objectives
            .first()
            .map_or(1, |objective| objective.stage_selectors[3]);
        let initial_level2_group10_switch25 = entities
            .iter()
            .find(|entity| {
                entity.index == 25
                    && entity.active
                    && entity.class == 11
                    && entity.model == 0
                    && entity.source_id == 10
                    && !entity.drawing_disabled()
            })
            .map(|entity| Level2Group10Switch25State {
                source_slot: entity.index,
                authored_index: 112,
                target_group: 10,
                position: entity.position,
                phase: entity.phase,
            });
        let initial_level2_imported_switch_roots = mission
            .authored_features()
            .iter()
            .copied()
            .filter(|feature| {
                feature.activation_group == 0
                    && feature.kind == 11
                    && matches!(feature.subtype, 0 | 1)
                    // Row 111 is the imported source for the dedicated
                    // model-76 switch-25 path below.
                    && feature.index != 111
            })
            .filter_map(|feature| {
                let model = u8::try_from(feature.subtype).ok()?;
                let target_group = u16::try_from(feature.stage_tag).ok()?;
                let position = feature.world_position(0);
                let entity = entities.iter().find(|entity| {
                    entity.active
                        && entity.class == 11
                        && entity.model == model
                        && entity.source_id == target_group
                        && entity.position.x == position.x
                        && entity.position.y == position.y
                })?;
                Some(Level2ImportedSwitchRootState {
                    source_slot: entity.index,
                    authored_index: feature.index,
                    model,
                    target_group,
                    word_10: i16::try_from(feature.word_10).unwrap_or(i16::MAX),
                    position: entity.position,
                    phase: entity.phase,
                    cooldown: 0,
                    drawing_enabled: !entity.drawing_disabled(),
                    active: true,
                })
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let initial_player = &entities[usize::from(state.entity_index)];
        let initial_player_pending_damage = i32::from_le_bytes(
            initial_player.record_bytes()[98..102]
                .try_into()
                .expect("player damage mailbox bytes"),
        );
        let initial_player_pending_attacker = u16::from_le_bytes(
            initial_player.record_bytes()[102..104]
                .try_into()
                .expect("player attacker mailbox bytes"),
        );
        let initial_player_life = initial_player.life;
        let definition =
            CampaignDefinition::new(profile, package_sha256, collision_masks, presentation);
        let live = Level2LiveState {
            core: CanonicalRuntimeState {
                state,
                players,
                runtime,
                context,
                effect_journal: crate::campaign::journal::EffectJournal::default(),
            },
            terrain,
            economy: SourceArena::new(entities, economy),
            bad_stones,
            shared_spells,
            ordinary,
            goats,
            mission,
            nyphur_dispatcher,
            player_pending_damage: initial_player_pending_damage,
            player_pending_attacker: initial_player_pending_attacker,
            player_life: initial_player_life,
            player_menu_state: 0,
            player_death_notification: None,
            level_exit: LevelExitSequence::default(),
            level2_group8_activated: false,
            // The qualified frame-zero model-32 row observes stage index 3's
            // source state 1; the switch completes only after that state is 2.
            level2_selected_objective_stage_state: initial_level2_objective_stage_state,
            level2_objective_pause: 0,
            level2_objective_done: false,
            level2_mana_attraction_activated: BTreeSet::new(),
            level2_model13_activated: BTreeSet::new(),
            level2_bees_activated: BTreeSet::new(),
            level2_switch_producers_activated: BTreeSet::new(),
            level2_model32_activated: BTreeSet::new(),
            level2_model76_activated: BTreeSet::new(),
            level2_model22_activated: BTreeSet::new(),
            level2_model39_activated: BTreeSet::new(),
            level2_model6_activated: BTreeSet::new(),
            level2_model59_activated: BTreeSet::new(),
            level2_model53_activated: BTreeSet::new(),
            level2_model1_activated: BTreeSet::new(),
            level2_model17_activated: BTreeSet::new(),
            authored_features_consumed: BTreeSet::new(),
            level2_imported_switch_roots: initial_level2_imported_switch_roots,
            level2_group10_switch25: initial_level2_group10_switch25,
            level2_class14_activated: BTreeSet::new(),
            level2_tree_activated: BTreeSet::new(),
            level2_class5_activated: BTreeSet::new(),
            level2_model45_activated: BTreeSet::new(),
        };
        Self {
            session: CampaignSession::new(definition, live),
        }
    }

    pub(crate) fn bind_semantic_level_definition(&mut self, bindings: SemanticLevelBindings) {
        let mut definition = (*self.session.definition).clone();
        definition.bind_semantic_level(bindings);
        self.session.definition = Arc::new(definition);
    }

    pub(crate) fn creature_kind_for_source_slot(&self, source_slot: u16) -> Option<CreatureKind> {
        let authored_record = self
            .session
            .state
            .live
            .economy
            .economy
            .authored_class5()
            .iter()
            .find(|source| source.source_entity_index == source_slot)
            .map(|source| source.authored_index)
            .or_else(|| {
                self.session
                    .state
                    .live
                    .economy
                    .economy
                    .authored_bees()
                    .iter()
                    .find(|source| source.source_entity_index == source_slot)
                    .map(|source| source.authored_index)
            })?;
        self.session
            .definition
            .creature_kind_for_authored_record(authored_record)
    }

    fn scenery_kind_for_source_slot(&self, source_slot: u16) -> Option<SceneryKind> {
        self.session
            .state
            .live
            .economy
            .authored_trees()
            .iter()
            .find(|source| source.active && source.source_entity_index == source_slot)
            .and_then(|source| {
                self.session
                    .definition
                    .scenery_for_authored_record(source.authored_index)
            })
            .or_else(|| {
                self.session
                    .state
                    .live
                    .economy
                    .economy
                    .authored_class14()
                    .iter()
                    .find(|source| source.active && source.source_entity_index == source_slot)
                    .and_then(|source| {
                        self.session
                            .definition
                            .scenery_for_authored_record(source.authored_index)
                    })
            })
            .or_else(|| {
                self.session
                    .definition
                    .scenery_for_initial_source_slot(source_slot)
            })
    }

    fn item_kind_for_source_slot(&self, source_slot: u16) -> Option<ItemKind> {
        self.session
            .state
            .live
            .economy
            .economy
            .authored_class14()
            .iter()
            .find(|source| source.active && source.source_entity_index == source_slot)
            .and_then(|source| {
                self.session
                    .definition
                    .item_for_authored_record(source.authored_index)
            })
    }

    fn spell_kind_for_source_slot(&self, source_slot: u16) -> Option<SpellKind> {
        self.session
            .state
            .live
            .economy
            .economy
            .spell_pickups()
            .iter()
            .find(|source| source.active && source.source_entity_index == source_slot)
            .and_then(|source| {
                self.session
                    .definition
                    .spell_for_authored_record(source.authored_index)
            })
    }

    fn building_kind_for_source_slot(
        &self,
        source_slot: u16,
        route: Level2SourceRoute,
    ) -> Option<BuildingKind> {
        match route {
            Level2SourceRoute::House => self
                .session
                .definition
                .building_for_initial_source_slot(source_slot),
            Level2SourceRoute::Dynamic(Level2DynamicArenaCursorRow {
                kind: Level2DynamicArenaKind::AuthoredModel45,
                ..
            }) => self
                .session
                .state
                .live
                .economy
                .economy
                .authored_model45()
                .iter()
                .find(|source| source.active && source.source_entity_index == source_slot)
                .and_then(|source| {
                    self.session
                        .definition
                        .building_for_authored_record(source.authored_index)
                }),
            _ => None,
        }
    }

    fn switch_kind_for_source_slot(&self, source_slot: u16) -> Option<SwitchKind> {
        self.session
            .state
            .live
            .economy
            .economy
            .authored_model32()
            .iter()
            .find(|source| source.active && source.source_entity_index == source_slot)
            .and_then(|source| {
                self.session
                    .definition
                    .switch_for_authored_record(source.authored_index)
            })
            .or_else(|| {
                self.session
                    .state
                    .live
                    .economy
                    .economy
                    .authored_switch_producers()
                    .iter()
                    .find(|source| source.active && source.source_entity_index == source_slot)
                    .and_then(|source| {
                        self.session
                            .definition
                            .switch_for_authored_record(source.authored_index)
                    })
            })
            .or_else(|| {
                self.session
                    .definition
                    .switch_for_initial_source_slot(source_slot)
            })
    }

    fn mechanic_kind_for_source_slot(&self, source_slot: u16) -> Option<MechanicKind> {
        self.session
            .state
            .live
            .economy
            .economy
            .authored_switch_producers
            .iter()
            .find(|source| source.source_entity_index == source_slot)
            .and_then(|source| {
                self.session
                    .definition
                    .mechanic_for_authored_record(source.authored_index)
            })
            .or_else(|| {
                self.session
                    .definition
                    .mechanic_for_initial_source_slot(source_slot)
            })
    }

    fn countdown_observed_creature_present(
        &self,
        source_slot: u16,
        pass_start_class5_model_slots: &[Vec<u16>; 29],
    ) -> std::result::Result<bool, String> {
        if self.session.definition.semantic_level().is_none() {
            // The pre-descriptor owner-export adapter has no semantic
            // definition to bind. Preserve its two private source selectors
            // until those compatibility callers move to the campaign package.
            let legacy_model = self
                .session
                .state
                .live
                .economy
                .entities
                .get(usize::from(source_slot))
                .filter(|source| {
                    source.active && source.class == 11 && source.model == 16 && source.action == 16
                })
                .map(|_| 3_usize)
                .or_else(|| {
                    self.session
                        .state
                        .live
                        .economy
                        .economy
                        .authored_switch_producers
                        .iter()
                        .find(|source| {
                            source.active
                                && source.source_entity_index == source_slot
                                && source.source_class == 11
                                && source.model == 17
                                && source.action == 17
                        })
                        .map(|_| 4_usize)
                })
                .ok_or_else(|| {
                    format!(
                        "legacy countdown source slot {source_slot} has no accepted MC2 selector"
                    )
                })?;
            return Ok(pass_start_class5_model_slots
                .get(legacy_model)
                .is_some_and(|slots| !slots.is_empty()));
        }
        let MechanicKind::AbsenceCountdown { creature } = self
            .mechanic_kind_for_source_slot(source_slot)
            .ok_or_else(|| {
                format!(
                    "countdown source slot {source_slot} has no package-derived semantic mechanic"
                )
            })?;
        let model = self
            .session
            .definition
            .creature_model(creature)
            .ok_or_else(|| {
                format!(
                    "countdown source slot {source_slot} observes {}, but the package has no creature-model binding",
                    creature.as_str()
                )
            })?;
        Ok(pass_start_class5_model_slots
            .get(usize::from(model))
            .is_some_and(|slots| !slots.is_empty()))
    }

    /// Borrow the canonical campaign carrier.
    pub(crate) fn campaign_state(&self) -> &CampaignState {
        &self.session.state
    }

    /// Mutably borrow the canonical campaign carrier.
    pub(crate) fn campaign_state_mut(&mut self) -> &mut CampaignState {
        &mut self.session.state
    }

    /// Borrow the one generation-bound production journal.
    #[must_use]
    pub(crate) fn effect_journal(&self) -> &crate::campaign::journal::EffectJournal {
        &self.session.state.live.core.effect_journal
    }

    fn effect_journal_mut(&mut self) -> &mut crate::campaign::journal::EffectJournal {
        &mut self.session.state.live.core.effect_journal
    }

    /// Append to the canonical journal before updating the legacy Level 2
    /// event slice. The latter remains a compatibility projection until the
    /// generic CF-12 reader replaces it.
    fn push_dispatch_effect(&mut self, event: Level2DispatchEvent) {
        self.flush_shared_effects();
        let turn = self.session.state.live.core.runtime.global_turn;
        self.effect_journal_mut()
            .append_dispatch(turn, event.clone());
        Vec::push(&mut self.session.state.scratch.dispatch_events, event);
    }

    /// Record an authoritative flight/audio effect before retaining the old
    /// per-subsystem projection consumed by existing qualification tools.
    fn push_flight_effect(&mut self, event: FlightEvent) {
        self.flush_shared_effects();
        let turn = self.session.state.live.core.runtime.global_turn;
        self.effect_journal_mut().append_flight(turn, event);
        Vec::push(&mut self.session.state.live.shared_spells.events, event);
    }

    fn push_scene_change_effect(&mut self, scene: u8) {
        self.flush_shared_effects();
        let turn = self.session.state.live.core.runtime.global_turn;
        self.effect_journal_mut().append_scene_change(turn, scene);
    }

    fn append_context_effects(&mut self, events: Vec<FlightEvent>) {
        if events.is_empty() {
            return;
        }
        self.flush_shared_effects();
        let turn = self.session.state.live.core.runtime.global_turn;
        let journal = self.effect_journal_mut();
        for event in events {
            journal.append_flight(turn, event);
        }
    }

    fn flush_shared_effects(&mut self) {
        let effects = self
            .session
            .state
            .live
            .shared_spells
            .drain_pending_effects();
        if effects.is_empty() {
            return;
        }
        let turn = self.session.state.live.core.runtime.global_turn;
        let journal = self.effect_journal_mut();
        for effect in effects {
            match effect {
                crate::campaign::journal::CampaignEffect::Dispatch(event) => {
                    journal.append_dispatch(turn, event);
                }
                crate::campaign::journal::CampaignEffect::SharedSpell(event) => {
                    journal.append_shared_spell(turn, event);
                }
                crate::campaign::journal::CampaignEffect::Flight(event) => {
                    journal.append_flight(turn, event);
                }
                crate::campaign::journal::CampaignEffect::SceneChange { scene } => {
                    journal.append_scene_change(turn, scene);
                }
            }
        }
    }

    #[must_use]
    pub(crate) fn source_arena(&self) -> &SourceArena {
        &self.session.state.live.economy
    }

    pub(crate) fn source_arena_mut(&mut self) -> &mut SourceArena {
        &mut self.session.state.live.economy
    }

    /// Borrow the one terrain owner used by all Level 2 production handlers.
    pub fn canonical_terrain(&self) -> &SessionTerrain {
        self.campaign_state().terrain()
    }

    /// Mutably borrow the one terrain owner used by terrain-changing handlers.
    pub(crate) fn canonical_terrain_mut(&mut self) -> &mut SessionTerrain {
        self.campaign_state_mut().terrain_mut()
    }

    #[must_use]
    pub fn profile(&self) -> FlightProfile {
        self.session.definition.profile
    }

    /// Immutable owner-package identity carried from the validated Level 2
    /// inspection into this live reset session.
    #[must_use]
    pub fn package_sha256(&self) -> &str {
        &self.session.definition.package_sha256
    }

    /// Borrow the immutable source-exact Level 2 collision bank retained by
    /// this reset session. A historical v3 session has no collision authority.
    #[must_use]
    pub fn collision_mask_bank(&self) -> Option<&CollisionMaskBank> {
        self.session.definition.collision_masks.as_ref()
    }

    /// Query remc2 model-45 occupancy through the authoritative session bank.
    pub fn model45_occupancy(&self, selector: u16, x: i32, y: i32) -> Result<bool> {
        let bank = self.collision_mask_bank().ok_or_else(|| {
            NativeWorldError::new("Level 2 collision occupancy requires a qualified v4 package")
        })?;
        bank.model45_occupancy(selector, x, y)
            .map_err(|error: CollisionMaskLookupError| {
                NativeWorldError::new(format!("model-45 occupancy lookup failed: {error:?}"))
            })
    }

    /// Select the authored Level 2 campaign entry objective for the C ABI
    /// campaign route. Generic inspection resets intentionally preserve the
    /// package's raw selector; the campaign route binds its own entry state
    /// before the first generation is exposed.
    pub fn select_level2_campaign_entry_stage(&mut self) {
        let rebase_initial_definition = self.session.generation == 0
            && ResetReceipt::from_state(&self.session.state.live) == self.session.baseline_hash;
        self.session
            .state
            .live
            .core
            .runtime
            .set_level2_objective_text_selector_for(0, LEVEL2_SELECTED_OBJECTIVE_STAGE_INDEX);
        if rebase_initial_definition {
            self.session.rebase();
        }
    }

    #[must_use]
    pub fn entities(&self) -> &[EntitySnapshot] {
        self.session.state.live.economy.entities()
    }

    /// Return the source `str_D93C0` byte-2 flags for one entity-parameter
    /// selector. Level 2 presentation carries the selector from the raw
    /// entity record's byte 74; consumers need this single source-owned byte
    /// for model-45 help admission and must not inspect the table or
    /// substitute an entity-pool slot.
    #[must_use]
    pub fn level2_entity_parameter_flags(&self, selector: u8) -> Option<u8> {
        self.session
            .definition
            .presentation
            .entity_parameters
            .get(usize::from(selector))
            .map(|record| record[2])
    }

    #[must_use]
    pub fn players(&self) -> &[PlayerSnapshot] {
        &self.session.state.live.core.players
    }

    /// Set the selected objective stage value consumed by source model-32
    /// switches. Values other than the source's completion sentinel are
    /// retained for negative-path testing but never complete the switch.
    pub fn set_level2_selected_objective_stage_state(&mut self, state: u8) -> Result<()> {
        if state > 2 {
            return Err(NativeWorldError::new(format!(
                "Level 2 objective stage state {state} is out of bounds"
            )));
        }
        self.session
            .state
            .live
            .level2_selected_objective_stage_state = state;
        self.session
            .state
            .live
            .core
            .runtime
            .set_level2_objective_stage_state(state);
        Ok(())
    }

    /// Stage one real package activation-group boundary for a focused
    /// downstream production test. Constructors, allocator behavior, source
    /// ordering, callbacks, epilogues, and cleanup remain owned by the normal
    /// shared engine path.
    #[cfg(test)]
    pub(crate) fn activate_authored_group_prerequisite_for_test(
        &mut self,
        target_group: u16,
    ) -> std::result::Result<(), String> {
        self.consume_authored_group(0, 0, target_group)
    }

    /// Evaluate every participating owner's remaining Level 2 objective
    /// stage after the entity pass. This mirrors the source
    /// `sub_58F00_game_objectives` walk: stage selectors at one are eligible,
    /// a successful predicate writes two, and the first remaining one becomes
    /// the next objective-text selector (or sets `IsLevelEnd` when none remain).
    /// Type 9 is deliberately deferred because the compact runtime does not
    /// yet expose the exact active BUILD-chain relation used by remc2.
    pub(super) fn advance_level2_objectives(
        &mut self,
        pass_start_class5_model_present: &[bool; 29],
    ) {
        let owner_colors = self
            .session
            .state
            .live
            .core
            .players
            .iter()
            .take(usize::from(
                self.session.state.live.core.runtime.player_count,
            ))
            .filter(|player| player.active_player_index >= 0)
            .map(|player| usize::from(player.player_color_index()))
            .collect::<Vec<_>>();
        let stage_count = usize::from(self.session.state.live.core.runtime.stage_index)
            .min(self.session.state.live.core.runtime.stages.len());
        let player_x = i32::from(self.session.state.live.core.state.position.x as i16);
        let player_y = i32::from(self.session.state.live.core.state.position.y as i16);

        for objective_index in owner_colors {
            let Some(objective) = self
                .session
                .state
                .live
                .core
                .runtime
                .objectives
                .get(objective_index)
                .copied()
            else {
                continue;
            };
            if objective.done != 0 {
                self.session
                    .state
                    .live
                    .core
                    .runtime
                    .consume_objective_done_pause_for(objective_index);
                continue;
            }
            if objective.is_level_end != 0 {
                continue;
            }
            let mut completed = Vec::new();
            for stage_index in 0..stage_count {
                if objective.stage_selectors[stage_index] != 1 {
                    continue;
                }
                let Some(stage) = self.session.state.live.core.runtime.stages.get(stage_index)
                else {
                    continue;
                };
                let condition = self
                    .session
                    .definition
                    .semantic_level()
                    .and_then(|level| level.objectives().get(stage_index))
                    .map(|objective| objective.condition());
                let achieved = if let Some(condition) = condition {
                    match condition {
                        ObjectiveCondition::CastleManaPercent(required_percent) => {
                            let required_percent = i32::from(required_percent);
                            let player_entity_index = self
                                .session
                                .state
                                .live
                                .core
                                .players
                                .iter()
                                .find(|player| {
                                    usize::from(player.player_color_index()) == objective_index
                                })
                                .map_or(0, |player| player.entity_index);
                            let economy = &self.session.state.live.economy.economy;
                            let castle_slot = economy.player_castle_entity_index();
                            let denominator = economy.owner_mana_global();
                            castle_slot != 0
                                && denominator > 0
                                && economy.castle().is_some_and(|castle| {
                                    castle.active
                                        && castle.source_entity_index == castle_slot
                                        && castle.owner_entity_index == player_entity_index
                                        && 100_i32
                                            * economy
                                                .owner_mana_aggregate(player_entity_index)
                                                .saturating_add(castle.mana)
                                            / denominator
                                            >= required_percent
                                })
                        }
                        ObjectiveCondition::DefeatRivalWizard => {
                            // The source type-3 predicate is independent of
                            // ObjectiveText and reads the bound player's
                            // Type_str_0x2BDE byte, not the compact
                            // ActPlayerIndex bytes or linked entity life/drawing
                            // fields.
                            if stage.flags & 1 == 0 {
                                false
                            } else if let RuntimeRelation::LiveEntity(target_index) = stage.relation
                            {
                                self.session
                                    .state
                                    .live
                                    .core
                                    .players
                                    .iter()
                                    .find(|player| player.entity_index == target_index)
                                    .is_some_and(|player| {
                                        player.objective_player_state() == Some(0)
                                    })
                            } else {
                                false
                            }
                        }
                        ObjectiveCondition::ReachLocation(location) => {
                            if objective.text_selector as usize != stage_index {
                                false
                            } else {
                                let stage_x = i32::from(i16::from_le_bytes(
                                    location.x.wrapping_mul(256).to_le_bytes(),
                                ));
                                let stage_y = i32::from(i16::from_le_bytes(
                                    location.y.wrapping_mul(256).to_le_bytes(),
                                ));
                                (stage_x - player_x).abs() <= LEVEL2_OBJECTIVE_POINT_RADIUS
                                    && (stage_y - player_y).abs() <= LEVEL2_OBJECTIVE_POINT_RADIUS
                            }
                        }
                        ObjectiveCondition::NoLivingCreatures(kind) => {
                            objective.text_selector as usize == stage_index
                            // Admit a semantic absence objective only after
                            // that creature's complete production, damage,
                            // hide, and following-pass cleanup lifecycle is
                            // shared by the authoritative arena.
                            && matches!(
                                kind,
                                CreatureKind::Bee
                                    | CreatureKind::Archer
                                    | CreatureKind::Firefly
                            )
                            && self
                                .session
                                .definition
                                .creature_model(kind)
                                .and_then(|model| {
                                    pass_start_class5_model_present.get(usize::from(model))
                                })
                                .is_some_and(|present| !present)
                        }
                        // The source scans BLDGPRM/BUILD chains here. Until the
                        // runtime exports those chain links, do not infer success
                        // from the temporary model-45 producer row.
                        ObjectiveCondition::NoStandingBuildings => false,
                    }
                } else {
                    // Legacy owner-export sessions predate the campaign
                    // definition envelope. Keep their private MC2 objective
                    // adapter until those compatibility callers are retired;
                    // descriptor sessions above always use semantic input.
                    match stage.kind {
                        0 => {
                            let RuntimeRelation::Scalar(required_percent) = stage.relation else {
                                continue;
                            };
                            let Ok(required_percent) = i32::try_from(required_percent) else {
                                continue;
                            };
                            let player_entity_index = self
                                .session
                                .state
                                .live
                                .core
                                .players
                                .iter()
                                .find(|player| {
                                    usize::from(player.player_color_index()) == objective_index
                                })
                                .map_or(0, |player| player.entity_index);
                            let economy = &self.session.state.live.economy.economy;
                            let castle_slot = economy.player_castle_entity_index();
                            let denominator = economy.owner_mana_global();
                            castle_slot != 0
                                && denominator > 0
                                && economy.castle().is_some_and(|castle| {
                                    castle.active
                                        && castle.source_entity_index == castle_slot
                                        && castle.owner_entity_index == player_entity_index
                                        && 100_i32
                                            * economy
                                                .owner_mana_aggregate(player_entity_index)
                                                .saturating_add(castle.mana)
                                            / denominator
                                            >= required_percent
                                })
                        }
                        3 => {
                            if stage.flags & 1 == 0 {
                                false
                            } else if let RuntimeRelation::LiveEntity(target_index) = stage.relation
                            {
                                self.session
                                    .state
                                    .live
                                    .core
                                    .players
                                    .iter()
                                    .find(|player| player.entity_index == target_index)
                                    .is_some_and(|player| {
                                        player.objective_player_state() == Some(0)
                                    })
                            } else {
                                false
                            }
                        }
                        5 => {
                            objective.text_selector as usize == stage_index
                                && (i32::from(stage.axis_x) - player_x).abs()
                                    <= LEVEL2_OBJECTIVE_POINT_RADIUS
                                && (i32::from(stage.axis_y) - player_y).abs()
                                    <= LEVEL2_OBJECTIVE_POINT_RADIUS
                        }
                        7 => {
                            objective.text_selector as usize == stage_index
                                && match stage.relation {
                                    RuntimeRelation::Scalar(model) if matches!(model, 2 | 4) => {
                                        usize::try_from(model)
                                            .ok()
                                            .and_then(|model| {
                                                pass_start_class5_model_present.get(model)
                                            })
                                            .is_some_and(|present| !present)
                                    }
                                    _ => false,
                                }
                        }
                        9 => false,
                        _ => false,
                    }
                };
                if achieved {
                    completed.push(stage_index);
                }
            }
            if completed.is_empty() {
                continue;
            }
            for stage_index in completed {
                self.session
                    .state
                    .live
                    .core
                    .runtime
                    .set_level2_objective_stage_selector_for(objective_index, stage_index, 2);
                if objective_index == 0 && stage_index == LEVEL2_SELECTED_OBJECTIVE_STAGE_INDEX {
                    self.session
                        .state
                        .live
                        .level2_selected_objective_stage_state = 2;
                }
            }
            let next_stage = self.session.state.live.core.runtime.objectives[objective_index]
                .stage_selectors
                .iter()
                .take(stage_count)
                .position(|selector| *selector == 1);
            if let Some(next_stage) = next_stage {
                self.session
                    .state
                    .live
                    .core
                    .runtime
                    .set_level2_objective_text_selector_for(objective_index, next_stage);
            } else {
                self.session
                    .state
                    .live
                    .core
                    .runtime
                    .set_level2_objective_level_end_for(objective_index, 1);
            }
        }
    }

    #[must_use]
    pub const fn level2_selected_objective_stage_state(&self) -> u8 {
        self.session
            .state
            .live
            .level2_selected_objective_stage_state
    }

    #[must_use]
    pub const fn level2_objective_pause(&self) -> u8 {
        self.session.state.live.level2_objective_pause
    }

    #[must_use]
    pub const fn level2_objective_done(&self) -> bool {
        self.session.state.live.level2_objective_done
    }

    #[must_use]
    pub const fn level_exit_sequence(&self) -> LevelExitSequence {
        self.session.state.live.level_exit
    }

    /// Bind the source per-player `MenuState_0x3DF` consumed by the dying
    /// callback. This is deliberately not the spell holder's menu-stage byte.
    pub fn set_level2_player_menu_state(&mut self, menu_state: u8) {
        self.session.state.live.player_menu_state = menu_state;
    }

    #[must_use]
    pub const fn level2_player_menu_state(&self) -> u8 {
        self.session.state.live.player_menu_state
    }

    /// Move one imported source entity through the lossless Level 2 seam.
    /// Callers cannot create an entity or alter its class/model identity. The
    /// local player's flight state and entity projection are one public input
    /// carrier; keep them coherent so source overlap callbacks observe the
    /// same position whichever projection they use.
    pub fn set_level2_entity_position(
        &mut self,
        entity_index: u16,
        position: Position,
    ) -> Result<()> {
        let Some(entity) = self
            .session
            .state
            .live
            .economy
            .entities
            .get_mut(usize::from(entity_index))
        else {
            return Err(NativeWorldError::new(format!(
                "Level 2 entity index {entity_index} is out of bounds"
            )));
        };
        let active = entity.active;
        entity.set_position(position);
        self.session
            .state
            .live
            .economy
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index,
                active,
                x: position.x,
                y: position.y,
            })
            .map_err(NativeWorldError::new)?;
        if entity_index == self.session.state.live.core.state.entity_index {
            self.session.state.live.core.state.position = position;
        }
        Ok(())
    }

    /// Project the canonical imported row into the typed spell view before
    /// dispatch. The family view is a convenience carrier, never a second
    /// independently mutable entity authority.
    fn refresh_imported_spell_source_projection(&mut self, source_slot: u16) -> Result<()> {
        let source = self
            .session
            .state
            .live
            .economy
            .entities
            .get(usize::from(source_slot))
            .filter(|entity| entity.active && entity.index == source_slot)
            .cloned()
            .ok_or_else(|| {
                NativeWorldError::new(format!(
                    "imported spell pickup source slot {source_slot} is not live"
                ))
            })?;
        self.session
            .state
            .live
            .shared_spells
            .sync_pickup_from_authority(&source);
        Ok(())
    }

    /// Commit a typed spell callback back into the imported arena row and
    /// refresh the canonical player record. Scheduler cleanup and later
    /// source slots consume these canonical rows.
    fn commit_imported_spell_source_projection(&mut self, source_slot: u16) -> Result<()> {
        let source = self
            .session
            .state
            .live
            .shared_spells
            .pickups()
            .iter()
            .chain(self.session.state.live.shared_spells.holders.iter())
            .chain(self.session.state.live.shared_spells.dynamic_pickups())
            .find(|entity| entity.index == source_slot)
            .cloned()
            .ok_or_else(|| {
                NativeWorldError::new(format!(
                    "generic spell pickup source slot {source_slot} lost its typed projection"
                ))
            })?;
        let live = &mut self.session.state.live;
        let target = live
            .economy
            .entities
            .get_mut(usize::from(source_slot))
            .filter(|entity| entity.index == source_slot)
            .ok_or_else(|| {
                NativeWorldError::new(format!(
                    "generic spell pickup source slot {source_slot} lost its imported row"
                ))
            })?;
        target.clone_from(&source);
        live.economy
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .map_err(NativeWorldError::new)?;

        let shared_players = live.shared_spells.players().to_vec();
        for player in &mut live.core.players {
            if let Some(source_player) = shared_players
                .iter()
                .find(|source_player| source_player.entity_index == player.entity_index)
            {
                player.sync_spell_acquisition_from(source_player);
            }
        }
        Ok(())
    }

    pub(super) fn level2_model0_overlap_matches<F>(
        &self,
        switch_position: Position,
        switch_half_width: i16,
        switch_half_depth: i16,
        expected_overlap: bool,
        mut entity_position: F,
    ) -> bool
    where
        F: FnMut(&EntitySnapshot) -> Position,
    {
        let overlap = self
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .any(|entity| {
                entity.active
                    && entity.class == 3
                    && entity.model == 0
                    && entity.life >= 0
                    && crate::level1_mission::source_entities_overlap_xy(
                        switch_position,
                        switch_half_width,
                        switch_half_depth,
                        entity_position(entity),
                        i16::from_le_bytes(
                            entity.record_bytes()[88..90]
                                .try_into()
                                .expect("player render half-width extent"),
                        ),
                        i16::from_le_bytes(
                            entity.record_bytes()[90..92]
                                .try_into()
                                .expect("player render half-depth extent"),
                        ),
                    )
            });
        overlap == expected_overlap
    }

    pub(super) fn level2_model0_overlap_owner<F>(
        &self,
        switch_position: Position,
        switch_half_width: i16,
        switch_half_depth: i16,
        mut entity_position: F,
    ) -> Option<u16>
    where
        F: FnMut(&EntitySnapshot) -> Position,
    {
        self.session
            .state
            .live
            .economy
            .entities
            .iter()
            .find(|entity| {
                entity.active
                    && entity.class == 3
                    && entity.model == 0
                    && entity.life >= 0
                    && crate::level1_mission::source_entities_overlap_xy(
                        switch_position,
                        switch_half_width,
                        switch_half_depth,
                        entity_position(entity),
                        i16::from_le_bytes(
                            entity.record_bytes()[88..90]
                                .try_into()
                                .expect("player render half-width extent"),
                        ),
                        i16::from_le_bytes(
                            entity.record_bytes()[90..92]
                                .try_into()
                                .expect("player render half-depth extent"),
                        ),
                    )
            })
            .map(|entity| entity.index)
    }

    /// Evaluate one authored model-0/model-1 switch against the live model-0
    /// source list using remc2's strict XY comparator. The source scan is the
    /// live class-3/model-0 arena rebuilt by the source turn pre-pass; dead
    /// rows (`life < 0`) are not linked into that player list. Model 0 requires
    /// an overlap; model 1 requires a strict non-overlap. No Z value
    /// participates in this source callback.
    pub fn level2_authored_switch_matches(&self, authored_index: u16) -> Result<bool> {
        let feature = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .find(|feature| feature.index == authored_index)
            .ok_or_else(|| {
                NativeWorldError::new(format!(
                    "Level 2 authored switch row {authored_index} is absent"
                ))
            })?;
        let expected_overlap = match feature.subtype {
            0 => true,
            1 => false,
            other => {
                return Err(NativeWorldError::new(format!(
                    "Level 2 authored switch row {authored_index} has unsupported model {other}"
                )));
            }
        };
        let switch_position = feature.world_position(0);
        let switch_extent = feature.word_10.wrapping_shl(8).cast_signed();
        Ok(self.level2_model0_overlap_matches(
            switch_position,
            switch_extent,
            switch_extent,
            expected_overlap,
            |entity| entity.position,
        ))
    }

    /// Validate and expose the authored Level 2 Nyphur participant chain for
    /// the mutable reset session. The typed view is deliberately fallible so
    /// a stale or cross-player holder relation cannot become authority.
    pub fn nyphur_participant(&self) -> Result<Level2NyphurParticipant> {
        level2_nyphur_participant(
            &self.session.state.live.core.players,
            &self.session.state.live.economy.entities,
            &self.session.state.live.core.runtime,
            &self.session.baseline.economy.entities,
            |source_slot| {
                self.session
                    .state
                    .live
                    .economy
                    .relation_identity(source_slot)
            },
        )
    }

    /// Build the generation-bound Day HUD projection from the live shared
    /// spell/session state. Unknown hand selectors and broken parent links are
    /// rejected so presentation cannot fall back to a neighboring asset row.
    pub fn day_hud(&self) -> Result<Level2DayHudSnapshot> {
        let local_player_slot = self.session.state.live.core.runtime.local_player_slot;
        let local_player = &self.session.state.live.shared_spells.local_player;
        let local_entity = &self.session.state.live.shared_spells.player_entity;
        if local_player.slot != local_player_slot
            || local_player.entity_index != self.session.state.live.core.state.entity_index
            || local_entity.index != self.session.state.live.core.state.entity_index
        {
            return Err(NativeWorldError::new(
                "Level 2 Day HUD local player/entity relation is invalid",
            ));
        }

        let global_mana_denominator = self.session.state.live.economy.owner_mana_global();
        if global_mana_denominator <= 0 {
            return Err(NativeWorldError::new(
                "Level 2 Day HUD global mana denominator is not positive",
            ));
        }

        let local_player_color_index = local_player.player_color_index();
        let [
            mana_palette_index,
            mana_max_palette_index,
            life_palette_index,
        ] = level2_day_hud_palette_indices(local_player_color_index)?;
        let spell_slot = |raw_spell_selector: i16| -> Result<Level2DayHudSpellSlot> {
            if raw_spell_selector < 0 {
                return Ok(Level2DayHudSpellSlot::empty(
                    raw_spell_selector,
                    local_player_color_index,
                ));
            }
            let family_index = usize::try_from(raw_spell_selector).map_err(|_| {
                NativeWorldError::new("Level 2 Day HUD spell selector does not fit usize")
            })?;
            let holder_index = *local_player
                .enabled_spell_holders
                .get(family_index)
                .ok_or_else(|| {
                    NativeWorldError::new(format!(
                        "Level 2 Day HUD spell selector {raw_spell_selector} is unknown"
                    ))
                })?;
            if holder_index == 0 {
                return Err(NativeWorldError::new(format!(
                    "Level 2 Day HUD spell selector {raw_spell_selector} has no holder"
                )));
            }
            let holder = self
                .session
                .state
                .live
                .shared_spells
                .holders
                .iter()
                .chain(self.session.state.live.shared_spells.pickups().iter())
                .chain(
                    self.session
                        .state
                        .live
                        .shared_spells
                        .dynamic_pickups()
                        .iter(),
                )
                .find(|holder| holder.index == holder_index)
                .ok_or_else(|| {
                    NativeWorldError::new(format!(
                        "Level 2 Day HUD holder {holder_index} is absent"
                    ))
                })?;
            let parent_entity_index = holder.source_spell_parent_entity_index();
            if parent_entity_index != local_entity.index {
                return Err(NativeWorldError::new(format!(
                    "Level 2 Day HUD holder {holder_index} is owned by entity {parent_entity_index}"
                )));
            }
            let level_text_selector = u8::try_from(holder.level_gate).map_err(|_| {
                NativeWorldError::new(format!(
                    "Level 2 Day HUD holder {holder_index} has an unknown level selector"
                ))
            })?;
            let icon_selector = 123_u16
                .checked_add(u16::from(holder.model))
                .ok_or_else(|| {
                    NativeWorldError::new(format!(
                        "Level 2 Day HUD holder {holder_index} icon selector overflow"
                    ))
                })?;
            Ok(Level2DayHudSpellSlot {
                holder_entity_index: holder.index,
                parent_entity_index,
                raw_spell_selector,
                color_index: local_player_color_index,
                model: holder.model,
                tier: holder.level_gate,
                action: holder.action,
                action_marker: holder.action_marker,
                active_value: holder.active_value,
                activation_value: holder.activation_value,
                control_flags: holder.control_flags,
                max_mana: holder.max_mana,
                mana_regen: holder.mana_regen,
                caster_mana: self.session.state.live.shared_spells.player_mana(),
                frame_selector: if holder.active_value == 0 { 1 } else { 2 },
                icon_selector,
                level_text_selector,
                draw_admitted: level2_day_spell_draw_admitted(
                    holder.model,
                    holder.active_value,
                    self.session.state.scratch.level2_day_hud_color_phase_1,
                ),
            })
        };

        Ok(Level2DayHudSnapshot {
            tick: self.session.state.live.core.state.tick,
            local_player_slot,
            local_player_entity_index: local_entity.index,
            local_player_color_index,
            local_player_active: local_player.active_player_index >= 0 && local_entity.active,
            hit_frame_time_pre_draw: self
                .session
                .state
                .scratch
                .level2_day_hud_hit_frame_time_pre_draw,
            color_phase_1: self.session.state.scratch.level2_day_hud_color_phase_1,
            color_phase_2: self.session.state.scratch.level2_day_hud_color_phase_2,
            local_player_control_flags: local_entity.control_flags,
            local_player_life: local_entity.life,
            local_player_max_life: local_entity.max_life,
            local_player_mana: self.session.state.live.shared_spells.player_mana(),
            local_player_max_mana: local_entity.max_mana,
            global_mana_denominator,
            // DrawTopStatusBar_2D710's direct MSPRD0-0 roots and geometry.
            minimap_panel_selector: 40,
            castle_panel_selector: 54,
            balloon_panel_selector: 54,
            player_panel_selector: self
                .session
                .state
                .scratch
                .level2_day_hud_player_panel_selector,
            player_icon_selector: 43,
            life_mana_icon_selector: 42,
            life_palette_index,
            mana_palette_index,
            mana_max_palette_index,
            left: spell_slot(local_player.left_spell_slot_raw)?,
            right: spell_slot(local_player.right_spell_slot_raw)?,
            nyphur: self.nyphur_participant()?,
            nyphur_visibility: self.nyphur_visibility()?,
        })
    }

    /// Focused reset/replay digest for the typed Nyphur participant chain.
    pub fn nyphur_state_hash(&self) -> Result<String> {
        Ok(level2_nyphur_state_hash(&self.nyphur_participant()?))
    }

    /// Read the signed one-byte `Type_str_164::byte_0x1C1_449` carrier used by
    /// the production Nyphur dispatcher.
    #[must_use]
    pub const fn nyphur_dispatch_state_byte(&self) -> i8 {
        self.session.state.live.nyphur_dispatcher.state_byte()
    }

    /// Read the source-owned common upkeep carrier after the Nyphur source
    /// row has run. The phase is read from the live entity because the outer
    /// dispatcher increments byte `0x3E` after the callback returns.
    pub fn nyphur_common_upkeep(&self) -> Result<Level2NyphurCommonUpkeepSnapshot> {
        let entity = self
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .find(|entity| {
                entity.index
                    == self
                        .session
                        .state
                        .live
                        .nyphur_dispatcher
                        .common_upkeep
                        .source_slot
            })
            .ok_or_else(|| NativeWorldError::new("Level 2 Nyphur common source row is absent"))?;
        Ok(self
            .session
            .state
            .live
            .nyphur_dispatcher
            .common_upkeep(entity.phase))
    }

    /// Ordered `sub_12910` traces emitted by the real Level 2 source pass.
    #[must_use]
    pub fn nyphur_dispatch_traces(&self) -> &[Level2NyphurDispatchTrace] {
        &self.session.state.scratch.nyphur_dispatch_traces
    }

    /// Evaluate the pinned `sub_63570` visibility consumer for Nyphur.
    ///
    /// The owner mode is the player record byte at source offset 0x1BF.  The
    /// local family-12 holder's selected tier is the reveal gate: tier 0 does
    /// not reveal an Invisible owner, while tiers 1 and 2 do, subject to the
    /// target's source family-4 active-holder gate for tier 1.
    pub fn nyphur_visibility(&self) -> Result<Level2NyphurVisibility> {
        let participant = self.nyphur_participant()?;
        let owner = self
            .session
            .state
            .live
            .shared_spells
            .players
            .get(usize::from(participant.player_slot))
            .ok_or_else(|| NativeWorldError::new("Level 2 Nyphur visibility owner is absent"))?;
        let owner_visibility_mode = owner.record_bytes()[447];
        let beyond_sight = self.session.state.live.shared_spells.beyond_sight;
        let local_entity_index = self
            .session
            .state
            .live
            .shared_spells
            .local_player
            .entity_index;
        let beyond_sight_holder_present =
            beyond_sight.is_some_and(|state| state.parent_entity_index == local_entity_index);
        let beyond_sight_active = beyond_sight.is_some_and(|state| {
            state.parent_entity_index == local_entity_index && state.active_value > 0
        });
        let beyond_sight_tier = beyond_sight
            .filter(|state| state.parent_entity_index == local_entity_index)
            .map(|state| state.tier);
        let family4_holder_index = owner.enabled_spell_holders[4];
        let family4_gate_active = family4_holder_index != 0
            && (self
                .session
                .state
                .live
                .economy
                .entities
                .get(usize::from(family4_holder_index))
                .is_some_and(|entity| entity.active_value != 0)
                || self
                    .session
                    .state
                    .live
                    .shared_spells
                    .pickup(family4_holder_index)
                    .is_some_and(|entity| entity.active_value != 0)
                || self
                    .session
                    .state
                    .live
                    .shared_spells
                    .holders
                    .iter()
                    .find(|entity| entity.index == family4_holder_index)
                    .is_some_and(|entity| entity.active_value != 0));
        let visible_to_local_player = if owner_visibility_mode == 0 {
            true
        } else {
            match beyond_sight_tier {
                Some(1) => !family4_gate_active,
                Some(2..) => true,
                _ => false,
            }
        };
        Ok(Level2NyphurVisibility {
            owner_visibility_mode,
            beyond_sight_holder_present,
            beyond_sight_active,
            beyond_sight_tier,
            family4_gate_active,
            visible_to_local_player,
        })
    }

    #[must_use]
    pub fn runtime(&self) -> &RuntimeSnapshot {
        &self.session.state.live.core.runtime
    }

    #[must_use]
    pub fn bad_stones(&self) -> &[Level2BadStone] {
        &self.session.state.live.bad_stones
    }

    #[must_use]
    pub fn shared_spells(&self) -> &Level2SharedSpellResetSession {
        &self.session.state.live.shared_spells
    }

    #[cfg(not(test))]
    #[must_use]
    pub fn shared_spells_mut(&mut self) -> &mut Level2SharedSpellResetSession {
        &mut self.session.state.live.shared_spells
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn shared_spells_mut(&mut self) -> Level2SharedSpellTestView<'_> {
        Level2SharedSpellTestView { session: self }
    }

    #[must_use]
    pub fn economy(&self) -> &Level2EconomyState {
        &self.session.state.live.economy
    }

    #[cfg(not(test))]
    #[must_use]
    pub fn economy_mut(&mut self) -> &mut Level2EconomyState {
        &mut self.session.state.live.economy
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn economy_mut(&mut self) -> Level2EconomyTestView<'_> {
        Level2EconomyTestView { session: self }
    }

    /// Production seam for a rooted `sub_69250` replacement constructor. The
    /// allocator is intentionally owned by the Level 2 economy so callers
    /// cannot create a second free-slot pool beside the source arena.
    pub fn allocate_level2_spell_replacement(
        &mut self,
        family: u8,
        parent_entity_index: u16,
        position: Position,
    ) -> Option<Level2SpellReplacement> {
        let economy_before = self.session.state.live.economy.clone();
        let replacement = self.session.state.live.economy.allocate_spell_replacement(
            family,
            parent_entity_index,
            position,
        )?;
        if self
            .session
            .state
            .live
            .shared_spells
            .materialize_spell_replacement_pickup(replacement)
            .is_err()
        {
            self.session.state.live.economy = economy_before;
            return None;
        }
        Some(replacement)
    }

    /// Production entry for the authored class-10/model-54 temporary field.
    /// The caller is the rooted Level 2 activation/objective path; the
    /// feature's stage tag supplies the exact source radius/lifetime and the
    /// economy owns allocation, map linkage, reset, and reclaim.
    pub fn allocate_level2_mana_attraction_field(
        &mut self,
        authored_index: u16,
        action: u8,
    ) -> Option<u16> {
        let feature = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .find(|feature| {
                feature.index == authored_index
                    && feature.kind == u16::from(MANA_ATTRACTION_CLASS)
                    && feature.subtype == i16::from(MANA_ATTRACTION_MODEL)
            })?;
        let position = feature.world_position(
            self.canonical_terrain()
                .ground_height(feature.world_position(0)),
        );
        self.session
            .state
            .live
            .economy
            .allocate_mana_attraction_field(authored_index, position, feature.stage_tag, action)
    }

    /// Materialize one authored class-14 subtype-3/5 row through the pinned
    /// generic constructor.  This is deliberately an explicit production
    /// seam: groups 5/28/31 still need their switch/objective trigger logic,
    /// but once that logic selects a row this path performs the real arena
    /// allocation and map linkage.
    pub fn allocate_level2_authored_class14(
        &mut self,
        authored_index: u16,
    ) -> Option<Level2AuthoredClass14Source> {
        let feature = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .find(|feature| {
                feature.index == authored_index
                    && feature.kind == 14
                    && matches!(feature.subtype, 3 | 5)
            })?;
        let position = feature.world_position(
            self.canonical_terrain()
                .ground_height(feature.world_position(0)),
        );
        self.session.state.live.economy.allocate_authored_class14(
            authored_index,
            u8::try_from(feature.subtype).ok()?,
            feature.activation_group,
            feature.stage_tag,
            position,
        )
    }

    /// Materialize one authored class-2/model-0/action-0 tree through the
    /// pinned `AddTree_4AC40` constructor. The authored-group scan owns the
    /// producer timing; this seam owns the exact authored position lookup and
    /// shared arena/map allocation for every package group.
    pub fn allocate_level2_authored_tree(
        &mut self,
        authored_index: u16,
    ) -> Option<Level2AuthoredTreeSource> {
        let feature = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .find(|feature| {
                feature.index == authored_index && feature.kind == 2 && feature.subtype == 0
            })?;
        let position = feature.world_position(
            self.canonical_terrain()
                .ground_height(feature.world_position(0)),
        );
        self.session
            .state
            .live
            .economy
            .allocate_authored_tree(authored_index, position)
    }

    /// Materialize one authored class-2/model-1/action-3 stone through the
    /// shared lossless source arena. The authored-group producer owns when
    /// this constructor runs; the ordinary numeric scheduler owns its
    /// callback, phase epilogue, and any later reclaim.
    fn allocate_authored_stone_feature(&mut self, feature: AuthoredFeature) -> Option<u16> {
        if (feature.kind, feature.subtype) != (2, 1) {
            return None;
        }
        let position = feature.world_position(
            self.canonical_terrain()
                .ground_height(feature.world_position(0)),
        );
        let particle = *self.session.definition.presentation.particles.get(79)?;
        let particle_pose = campaign_shared_families::ParticlePose {
            animation_limit: campaign_shared_families::particle_animation_limit(
                particle.render_mode,
            )?,
            array_yaw: i16::try_from(particle.rotation_speed / 2).ok()?,
            array_pitch: i16::try_from(particle.speed / 2).ok()?,
            array_roll: i16::try_from(particle.speed / 2).ok()?,
            array_fov: i16::try_from(particle.rotation_speed / 2).ok()?,
        };

        let live = &mut self.session.state.live;
        let source_slot = live.economy.economy.allocate_slot()?;
        let entity_rng = live.economy.economy.new_event_rng(source_slot);
        if live
            .economy
            .economy
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            live.economy.economy.release_slot(source_slot, None);
            return None;
        }
        let map_links = (
            live.economy.economy.map_chains.old_map_entity(source_slot),
            live.economy.economy.map_chains.next_entity(source_slot),
        );
        let old_head_is_dynamic = live
            .economy
            .economy
            .live_arena_slots()
            .iter()
            .any(|(slot, _)| *slot == map_links.0);
        live.economy
            .economy
            .source_rng
            .insert(source_slot, entity_rng);
        live.economy.entities[usize::from(source_slot)] =
            campaign_shared_families::construct_stone_source(
                source_slot,
                entity_rng,
                position,
                map_links,
                particle_pose,
            );
        // AddEventToMap head-inserts the new row and updates the prior raw
        // head's link toward it. Typed dynamic heads retain that link in the
        // canonical map-chain carrier instead of a stale imported record.
        if map_links.0 != 0
            && !old_head_is_dynamic
            && let Some(old_head) = live
                .economy
                .entities
                .get_mut(usize::from(map_links.0))
                .filter(|entity| entity.active)
        {
            old_head.record[28..30].copy_from_slice(&source_slot.to_le_bytes());
        }
        Some(source_slot)
    }

    /// Materialize one authored class-10/model-6 row through the pinned
    /// `NewAdd0A06_4E5F0` constructor. The Level 2 census contains four such
    /// rows (indices 361..=364); their mission trigger remains owned by the
    /// aggregate authored-switch bead, while this seam performs the real
    /// source-arena allocation and callback setup.
    pub fn allocate_level2_authored_model6(
        &mut self,
        authored_index: u16,
    ) -> Option<Level2AuthoredModel6Source> {
        let feature = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .find(|feature| {
                feature.index == authored_index && feature.kind == 10 && feature.subtype == 6
            })?;
        let position = feature.world_position(
            self.session
                .state
                .live
                .terrain
                .ground_height(feature.world_position(0)),
        );
        self.session.state.live.economy.allocate_authored_model6(
            authored_index,
            position,
            &self.session.state.live.terrain,
        )
    }

    /// Materialize one authored class-10/model-59 checkpoint row through the
    /// pinned `ArriveCheckpoint_4EB50` constructor.
    pub fn allocate_level2_authored_model59(
        &mut self,
        authored_index: u16,
    ) -> Option<Level2AuthoredModel59Source> {
        let feature = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .find(|feature| {
                feature.index == authored_index && feature.kind == 10 && feature.subtype == 59
            })?;
        let position = feature.world_position(
            self.canonical_terrain()
                .ground_height(feature.world_position(0)),
        );
        self.session
            .state
            .live
            .economy
            .allocate_authored_model59(authored_index, position)
    }

    /// Materialize the activation-presence subset of the rooted
    /// class-10/subtype-58 row through remc2 `sub_504B0` (class 10/model
    /// 53/action 58).
    pub fn allocate_level2_authored_model53(
        &mut self,
        authored_index: u16,
    ) -> Option<Level2AuthoredModel53Source> {
        let feature = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .find(|feature| {
                feature.index == authored_index && feature.kind == 10 && feature.subtype == 58
            })?;
        let position = feature.world_position(
            self.canonical_terrain()
                .ground_height(feature.world_position(0)),
        );
        let source = self
            .session
            .state
            .live
            .economy
            .allocate_authored_model53(authored_index, position)?;
        self.session
            .state
            .live
            .core
            .runtime
            .bind_authored_stage_entity(authored_index, source.source_entity_index);
        Some(source)
    }

    /// Materialize one authored class-11/model-32 objective switch through
    /// the pinned `AddSwitch32_50B80` constructor and the authored row's
    /// stage-tag/objective-selector fields.
    pub fn allocate_level2_authored_model32(
        &mut self,
        authored_index: u16,
    ) -> Option<Level2AuthoredModel32Source> {
        let feature = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .find(|feature| {
                feature.index == authored_index && feature.kind == 11 && feature.subtype == 32
            })?;
        let stage_tag = u16::try_from(feature.stage_tag).ok()?;
        let objective_stage_index = u8::try_from(feature.parameter_1).ok()?;
        let position = feature.world_position(
            self.canonical_terrain()
                .ground_height(feature.world_position(0)),
        );
        self.session.state.live.economy.allocate_authored_model32(
            authored_index,
            stage_tag,
            objective_stage_index,
            position,
            &self.session.state.live.terrain,
        )
    }

    /// Materialize one authored class-10/model-17 Meteor row through the
    /// pinned `AddMeteor_4ED70` constructor. The callback's source-search
    /// child terrain behavior remains a separate follow-up; this seam owns
    /// the authored parent allocation and its explicit spawn branch.
    pub fn allocate_level2_authored_model17(
        &mut self,
        authored_index: u16,
    ) -> Option<Level2AuthoredModel17Source> {
        let feature = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .find(|feature| {
                feature.index == authored_index && feature.kind == 10 && feature.subtype == 17
            })?;
        let position = feature.world_position(
            self.canonical_terrain()
                .ground_height(feature.world_position(0)),
        );
        self.session.state.live.economy.allocate_authored_model17(
            authored_index,
            position,
            &self.session.state.live.terrain,
        )
    }

    /// Materialize one authored class-10/model-22 Wind row through the
    /// pinned `AddWind_4F040` constructor. The generic authored path derives
    /// the chain axis from parameter-2/parameter-1 before entering the
    /// model-specific constructor.
    pub fn allocate_level2_authored_model22(
        &mut self,
        authored_index: u16,
    ) -> Option<Level2AuthoredModel22Source> {
        let feature = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .find(|feature| {
                feature.index == authored_index && feature.kind == 10 && feature.subtype == 22
            })?;
        let position = feature.world_position(
            self.canonical_terrain()
                .ground_height(feature.world_position(0)),
        );
        let axis_position = Position {
            x: feature.parameter_2.wrapping_shl(8).wrapping_add(128),
            y: feature.parameter_1.wrapping_shl(8).wrapping_add(128),
            z: position.z,
        };
        self.session.state.live.economy.allocate_authored_model22(
            authored_index,
            axis_position,
            position,
            &self.session.state.live.terrain,
        )
    }

    /// Materialize one authored class-10/model-76 Fire Spheres row through
    /// the pinned `AddFireSpheres_4F2A0` constructor.
    pub fn allocate_level2_authored_model76(
        &mut self,
        authored_index: u16,
    ) -> Option<Level2AuthoredModel76Source> {
        let feature = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .find(|feature| {
                feature.index == authored_index && feature.kind == 10 && feature.subtype == 76
            })?;
        let position = feature.world_position(
            self.canonical_terrain()
                .ground_height(feature.world_position(0)),
        );
        let source = self.session.state.live.economy.allocate_authored_model76(
            authored_index,
            position,
            &self.session.state.live.terrain,
        )?;
        // `AddFireSpheres_4F2A0` calls `SetEntityIndexAndRot_49CD0` with
        // particle 340 for every linked child. The presentation table is the
        // authoritative owner of that selector row, so bind its four axes
        // once after the atomic economy allocation.
        if let Some(particle) = self.session.definition.presentation.particles.get(340) {
            let collision_yaw =
                i16::try_from(particle.rotation_speed / 2).expect("model-76 particle yaw fits i16");
            let collision_pitch =
                i16::try_from(particle.speed / 2).expect("model-76 particle pitch fits i16");
            let collision_roll = collision_pitch;
            let collision_fov = collision_yaw;
            debug_assert!(self.economy.bind_model76_collision_axes(
                source.source_entity_index,
                collision_yaw,
                collision_pitch,
                collision_roll,
                collision_fov,
            ));
        }
        Some(source)
    }

    /// Materialize one authored class-10/model-13 row through the pinned
    /// `sub_4EF90` constructor. The group-26 mission trigger is deliberately
    /// kept at this production seam; no objective state is inferred here.
    pub fn allocate_level2_authored_model13(
        &mut self,
        authored_index: u16,
    ) -> Option<Level2AuthoredModel13Source> {
        let feature = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .find(|feature| {
                feature.index == authored_index && feature.kind == 10 && feature.subtype == 13
            })?;
        let position = feature.world_position(
            self.canonical_terrain()
                .ground_height(feature.world_position(0)),
        );
        self.session.state.live.economy.allocate_authored_model13(
            authored_index,
            position,
            &self.session.state.live.terrain,
        )
    }

    pub(super) fn level2_bee_stage_route(
        &self,
        feature: AuthoredFeature,
    ) -> Option<Level2BeeStageRoute> {
        // The class/model constraint belongs to the `sub_4A310` producer.
        // The typed StageVar matcher itself remains source-generic: it is a
        // direct-or-subtype lookup, not a Bee-specific table.
        if feature.kind != 5 || feature.subtype != 2 {
            return None;
        }
        let stage_var = self
            .session
            .state
            .live
            .mission
            .level2_authored_stage_variable_for(feature)?;
        if stage_var.kind != 2 || !stage_var.subtype_match() || stage_var.flags & 0x60 != 0 {
            return None;
        }
        let runtime_stage_var = self
            .session
            .state
            .live
            .core
            .runtime
            .stage_variables
            .get(usize::from(stage_var.index))?;
        let (point_x, point_y) = stage_var.point();
        let expected_relation = u32::from(point_x) | (u32::from(point_y) << 16);
        if runtime_stage_var.kind != i8::try_from(stage_var.kind).expect("StageVar kind fits i8")
            || runtime_stage_var.flags as u8 != stage_var.flags
            || runtime_stage_var.word_2 & 0x00ff != u16::from(stage_var.stage)
            || runtime_stage_var.relation != RuntimeRelation::AxisScalar(expected_relation)
        {
            return None;
        }
        Some(Level2BeeStageRoute {
            stage_var1: stage_var.index,
            stage_var2: stage_var.kind,
            stage_target: Position {
                x: point_x,
                y: point_y,
                z: 0,
            },
        })
    }

    fn authored_firefly_stage_route(
        &self,
        feature: AuthoredFeature,
    ) -> Option<level2_class5::firefly::FireflyStageRoute> {
        if feature.kind != 5 || feature.subtype != i16::from(level2_class5::firefly::FIREFLY_MODEL)
        {
            return None;
        }
        let stage_var = self
            .session
            .state
            .live
            .mission
            .level2_authored_stage_variable_for(feature)?;
        if stage_var.kind != 1 || !stage_var.subtype_match() || stage_var.flags & 0x60 != 0 {
            return None;
        }
        let runtime_stage_var = self
            .session
            .state
            .live
            .core
            .runtime
            .stage_variables
            .get(usize::from(stage_var.index))?;
        let (point_x, point_y) = stage_var.point();
        let expected_relation = u32::from(point_x) | (u32::from(point_y) << 16);
        if runtime_stage_var.kind != i8::try_from(stage_var.kind).expect("StageVar kind fits i8")
            || runtime_stage_var.flags as u8 != stage_var.flags
            || runtime_stage_var.word_2 & 0x00ff != u16::from(stage_var.stage)
            || runtime_stage_var.relation != RuntimeRelation::AxisScalar(expected_relation)
        {
            return None;
        }
        Some(level2_class5::firefly::FireflyStageRoute {
            stage_var1: stage_var.index,
            stage_var2: stage_var.kind,
            target: Position {
                x: point_x,
                y: point_y,
                z: 0,
            },
        })
    }

    fn allocate_authored_firefly_feature(
        &mut self,
        feature: AuthoredFeature,
    ) -> Option<Level2AuthoredClass5Source> {
        let route = self.authored_firefly_stage_route(feature)?;
        let position = feature.world_position(
            self.canonical_terrain()
                .ground_height(feature.world_position(0)),
        );
        let source = self.session.state.live.economy.allocate_authored_firefly(
            feature.index,
            position,
            route,
            &self.session.state.live.terrain,
        )?;
        assert!(
            self.runtime
                .advance_level2_stage_variable_counter(route.stage_var1),
            "validated Firefly StageVar must retain its runtime counter carrier"
        );
        Some(source)
    }

    pub(super) fn allocate_level2_authored_bee_feature(
        &mut self,
        feature: AuthoredFeature,
    ) -> Option<Level2AuthoredBeeSource> {
        let stage_route = self.level2_bee_stage_route(feature);
        let position = feature.world_position(
            self.canonical_terrain()
                .ground_height(feature.world_position(0)),
        );
        let source = self.session.state.live.economy.allocate_authored_bee(
            feature.index,
            position,
            stage_route,
            &self.session.state.live.terrain,
        )?;
        if let Some(route) = stage_route {
            // This was checked above against the same bounded runtime table.
            // A failed increment would mean an internal carrier corruption,
            // not an allocation miss (which must leave the counter untouched).
            assert!(
                self.runtime
                    .advance_level2_stage_variable_counter(route.stage_var1),
                "validated Bee StageVar must retain its runtime counter carrier"
            );
        }
        Some(source)
    }

    /// Materialize one authored Day Bee row through remc2's class-5/model-2
    /// constructor. Mission activation owns the producer/group predicate;
    /// this seam performs the source-arena allocation and exact defaults.
    pub fn allocate_level2_authored_bee(
        &mut self,
        authored_index: u16,
    ) -> Option<Level2AuthoredBeeSource> {
        let feature = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .find(|feature| {
                feature.index == authored_index
                    && feature.kind == 5
                    && feature.subtype == 2
                    && matches!(feature.activation_group, 4 | 17 | 21 | 29)
            })?;
        self.allocate_level2_authored_bee_feature(feature)
    }

    #[must_use]
    pub fn ordinary(&self) -> &Level2OrdinaryResetSession {
        &self.session.state.live.ordinary
    }

    #[cfg(not(test))]
    #[must_use]
    pub fn ordinary_mut(&mut self) -> &mut Level2OrdinaryResetSession {
        &mut self.session.state.live.ordinary
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn ordinary_mut(&mut self) -> Level2OrdinaryTestView<'_> {
        Level2OrdinaryTestView { session: self }
    }
}

#[cfg(test)]
pub(crate) struct Level2EconomyTestView<'a> {
    session: &'a mut Level2ResetSession,
}

#[cfg(test)]
impl std::ops::Deref for Level2EconomyTestView<'_> {
    type Target = Level2EconomyState;

    fn deref(&self) -> &Self::Target {
        &self.session.economy
    }
}

#[cfg(test)]
impl std::ops::DerefMut for Level2EconomyTestView<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.session.economy
    }
}

#[cfg(test)]
macro_rules! terrain_method {
    ($name:ident, ($($arg:ident : $ty:ty),* $(,)?) -> $ret:ty) => {
        pub(crate) fn $name(&mut self, $($arg: $ty),*) -> $ret {
            self.with_terrain(|economy, terrain| economy.$name($($arg),*, terrain))
        }
    };
}

#[cfg(test)]
impl Level2EconomyTestView<'_> {
    fn with_terrain<R>(
        &mut self,
        callback: impl FnOnce(&mut Level2EconomyState, &mut SessionTerrain) -> R,
    ) -> R {
        let live = &mut self.session.session.state.live;
        callback(&mut live.economy.economy, &mut live.terrain)
    }

    pub(crate) fn reset(self) {
        self.session.reset();
    }

    pub(crate) fn hide_authored_class5_rows_for_test(&mut self, authored_indices: &[u16]) -> usize {
        self.session
            .session
            .state
            .live
            .economy
            .economy
            .hide_authored_class5_rows_for_test(authored_indices)
    }

    pub(crate) fn retire_authored_multipart_prerequisite_for_test(
        &mut self,
        authored_index: u16,
    ) -> bool {
        self.session
            .session
            .state
            .live
            .economy
            .economy
            .retire_authored_multipart_prerequisite_for_test(authored_index)
    }

    pub(crate) fn exhaust_new_event_capacity_for_test(&mut self) -> Option<u16> {
        let baseline_slot = std::sync::Arc::make_mut(&mut self.session.session.baseline)
            .economy
            .economy
            .exhaust_new_event_capacity_for_test();
        let live_slot = self
            .session
            .session
            .state
            .live
            .economy
            .economy
            .exhaust_new_event_capacity_for_test();
        live_slot.or(baseline_slot)
    }

    pub(crate) fn restore_new_event_slot_for_test(&mut self, source_slot: u16) -> bool {
        self.session
            .session
            .state
            .live
            .economy
            .economy
            .restore_new_event_slot_for_test(source_slot)
    }

    pub(crate) fn step_castle_action(&mut self, castle_slot: u16) -> bool {
        let reset_free_slots = self
            .session
            .session
            .baseline
            .economy
            .economy
            .free_slots
            .clone();
        let live = &mut self.session.session.state.live;
        live.economy
            .economy
            .step_castle_action(castle_slot, &mut live.terrain, &reset_free_slots)
    }

    terrain_method!(allocate_authored_archer,
        (authored_index: u16, position: Position) -> Option<Level2AuthoredClass5Source>);
    terrain_method!(allocate_authored_class5,
        (authored_index: u16, model: u8, position: Position) -> Option<Level2AuthoredClass5Source>);
    terrain_method!(allocate_authored_model6,
        (authored_index: u16, position: Position) -> Option<Level2AuthoredModel6Source>);
    terrain_method!(allocate_authored_model13,
        (authored_index: u16, position: Position) -> Option<Level2AuthoredModel13Source>);
    terrain_method!(allocate_authored_model22,
        (authored_index: u16, axis_position: Position, position: Position) -> Option<Level2AuthoredModel22Source>);
    terrain_method!(allocate_authored_model45,
        (authored_index: u16, target_group: u16, parameter_1: u16, position: Position) -> Option<Level2AuthoredModel45Source>);
    terrain_method!(allocate_authored_trader,
        (authored_index: u16, position: Position) -> Option<Level2AuthoredClass5Source>);
    terrain_method!(acquire_authored_bee_target,
        (source_slot: u16, candidates: &[(u16, Position, u32)]) -> Option<u16>);
    terrain_method!(build_castle,
        (position: Position, owner_entity_index: u16) -> Option<u16>);
    terrain_method!(build_castle_with_footprint,
        (projectile_position: Position, owner_entity_index: u16, footprint: &CastleBuildFootprint) -> Option<u16>);
    terrain_method!(complete_castle_construction,
        (castle_slot: u16) -> bool);
    terrain_method!(maintain_castle_defenders,
        (castle_slot: u16) -> ());
    terrain_method!(spawn_castle_projectile,
        (projectile_position: Position, owner_entity_index: u16, holder_entity_index: u16, player_yaw: i16, player_next_entity: i16, player_actual_speed: i16) -> Option<u16>);
    terrain_method!(spawn_castle_projectile_with_payload,
        (payload: level2_dynamic_economy::Level2CastleProjectileLaunchPayload, holder_entity_index: u16, player_actual_speed: i16) -> Option<u16>);
    terrain_method!(spawn_mana_sphere_from_death,
        (source_entity_index: u16, position: Position, source_yaw: i16, source_rng_after_death: u16, mana: i32, owner_entity_index: u16) -> Option<u32>);
    terrain_method!(spawn_mana_spheres_from_death,
        (source_entity_index: u16, position: Position, source_yaw: i16, source_rng_after_death: u16, mana: i32, owner_entity_index: u16, use_mana_fraction: bool) -> Vec<u32>);
    terrain_method!(step_authored_bee_death_tail,
        (source: &mut Level2AuthoredBeeSource) -> ());
    terrain_method!(step_authored_bee_slot,
        (source_slot: u16, target_position: Option<Position>) -> bool);
    terrain_method!(step_authored_class5_slot,
        (source_slot: u16) -> bool);
    terrain_method!(step_authored_model6_slot,
        (source_slot: u16) -> bool);
    terrain_method!(step_authored_model76_slot,
        (source_slot: u16) -> bool);
    terrain_method!(step_authored_model13_slot,
        (source_slot: u16) -> bool);
    terrain_method!(step_castle_defender_slot,
        (source_slot: u16) -> bool);
    terrain_method!(step_castle_projectile_slot,
        (source_slot: u16, entities: &[EntitySnapshot]) -> bool);
    terrain_method!(step_carrier_to_castle,
        (carrier_slot: u16, position: Position) -> bool);
    terrain_method!(step_model13_spawned_child_slot,
        (source_slot: u16) -> bool);
    terrain_method!(step_model59_smoke_child_slot,
        (source_slot: u16) -> bool);
    terrain_method!(step_model6_spawned_child_slot,
        (source_slot: u16) -> bool);
    terrain_method!(step_possession_projectile_slot,
        (source_slot: u16) -> bool);
    terrain_method!(step_mana_sphere_slot,
        (sphere_slot: u16) -> bool);
    terrain_method!(step_mana_carrier_slot,
        (carrier_slot: u16) -> bool);

    pub(crate) fn set_castle_mana_prerequisite_for_test(
        &mut self,
        castle_slot: u16,
        mana: i32,
    ) -> bool {
        self.session
            .session
            .state
            .live
            .economy
            .economy
            .set_castle_mana_prerequisite_for_test(castle_slot, mana)
    }

    pub(crate) fn step_model17_spawned_child_slot(&mut self, source_slot: u16) -> bool {
        let live = &mut self.session.session.state.live;
        live.economy
            .economy
            .step_model17_spawned_child_slot(source_slot, &live.terrain)
            .live
    }

    pub(crate) fn step_castle_child_terminal_slot(&mut self, source_slot: u16) -> bool {
        let live = &mut self.session.session.state.live;
        let auxiliary_membership = live
            .economy
            .economy
            .castle_auxiliary_membership(&live.economy.entities);
        live.economy.economy.step_castle_child_terminal_slot(
            source_slot,
            &mut live.economy.entities,
            &auxiliary_membership,
            &mut live.terrain,
        )
    }
}

#[cfg(test)]
pub(crate) struct Level2SharedSpellTestView<'a> {
    session: &'a mut Level2ResetSession,
}

#[cfg(test)]
impl std::ops::Deref for Level2SharedSpellTestView<'_> {
    type Target = Level2SharedSpellResetSession;

    fn deref(&self) -> &Self::Target {
        &self.session.shared_spells
    }
}

#[cfg(test)]
impl std::ops::DerefMut for Level2SharedSpellTestView<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.session.shared_spells
    }
}

#[cfg(test)]
impl Level2SharedSpellTestView<'_> {
    fn with_terrain<R>(
        &mut self,
        callback: impl FnOnce(&mut Level2SharedSpellResetSession, &SessionTerrain) -> R,
    ) -> R {
        let live = &mut self.session.session.state.live;
        callback(&mut live.shared_spells, &live.terrain)
    }

    pub(crate) fn reset(self) {
        self.session.reset();
    }

    pub(crate) fn step_speed_pickup(
        &mut self,
        player_entity_index: u16,
        player_position: Position,
    ) -> Result<Level2SpeedPickupStep> {
        self.with_terrain(|shared, terrain| {
            shared.step_speed_pickup(player_entity_index, player_position, terrain)
        })
    }

    pub(crate) fn step_shared_pickup(
        &mut self,
        source_slot: u16,
        player_entity_index: u16,
        player_position: Position,
    ) -> Result<Option<Level2SpeedPickupStep>> {
        self.with_terrain(|shared, terrain| {
            shared.step_shared_pickup(source_slot, player_entity_index, player_position, terrain)
        })
    }

    pub(crate) fn step_level2_spell_acquisition(
        &mut self,
        family: u8,
        player_entity_index: u16,
        player_position: Position,
    ) -> Result<Option<SpellHolderAcquisitionOutcome>> {
        self.with_terrain(|shared, terrain| {
            shared.step_level2_spell_acquisition(
                family,
                player_entity_index,
                player_position,
                terrain,
            )
        })
    }

    pub(crate) fn step_level2_spell_acquisition_with_economy(
        &mut self,
        economy: &mut Level2EconomyState,
        family: u8,
        player_entity_index: u16,
        player_position: Position,
    ) -> Result<Option<SpellHolderAcquisitionOutcome>> {
        self.with_terrain(|shared, terrain| {
            shared.step_level2_spell_acquisition_with_economy(
                economy,
                family,
                player_entity_index,
                player_position,
                terrain,
            )
        })
    }
}

#[cfg(test)]
pub(crate) struct Level2OrdinaryTestView<'a> {
    session: &'a mut Level2ResetSession,
}

#[cfg(test)]
impl std::ops::Deref for Level2OrdinaryTestView<'_> {
    type Target = Level2OrdinaryResetSession;

    fn deref(&self) -> &Self::Target {
        &self.session.ordinary
    }
}

#[cfg(test)]
impl std::ops::DerefMut for Level2OrdinaryTestView<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.session.ordinary
    }
}

#[cfg(test)]
impl Level2OrdinaryTestView<'_> {
    fn with_terrain<R>(
        &mut self,
        callback: impl FnOnce(&mut Level2OrdinaryResetSession, &SessionTerrain) -> R,
    ) -> R {
        let live = &mut self.session.session.state.live;
        callback(&mut live.ordinary, &live.terrain)
    }

    pub(crate) fn reset(self) {
        self.session.reset();
    }

    pub(crate) fn step_house_action_52(&mut self, source_slot: u16) -> Vec<HouseStepEvent> {
        self.with_terrain(|ordinary, terrain| ordinary.step_house_action_52(source_slot, terrain))
    }

    pub(crate) fn step_villager_action_105(&mut self, source_slot: u16) -> Vec<VillagerStepEffect> {
        self.with_terrain(|ordinary, terrain| {
            ordinary.step_villager_action_105(source_slot, terrain)
        })
    }

    pub(crate) fn step_dolmen_action_6(&mut self, source_slot: u16) -> Result<Level2DolmenStep> {
        self.with_terrain(|ordinary, terrain| ordinary.step_dolmen_action_6(source_slot, terrain))
    }
}

impl Level2ResetSession {
    #[must_use]
    pub fn goats(&self) -> &[GoatState] {
        self.session.state.live.goats.goats()
    }

    /// Queue one source-authoritative Goat hit for the next outer turn.
    /// Damage is written into the Goat family's pending mailbox; the normal
    /// Level 2 source pass consumes it and owns life/death/economy effects.
    pub fn submit_goat_damage(
        &mut self,
        target_entity_index: u16,
        damage: i32,
        attacker_entity_index: u16,
    ) -> Result<()> {
        self.session
            .state
            .live
            .goats
            .inject_damage(
                target_entity_index,
                damage,
                attacker_entity_index.cast_signed(),
            )
            .map_err(NativeWorldError::new)
    }

    #[must_use]
    pub const fn goat_outer_tick(&self) -> u32 {
        self.session.state.live.goats.outer_tick()
    }

    #[must_use]
    pub fn goat_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(REMC2_SPEC_COMMIT.as_bytes());
        hasher.update(self.session.state.live.goats.outer_tick().to_le_bytes());
        for (family_index, goat) in self.session.state.live.goats.goats().iter().enumerate() {
            hasher.update(goat.entity_index.to_le_bytes());
            hasher.update(goat.source_id.to_le_bytes());
            hasher.update(u8::from(goat.active).to_le_bytes());
            hasher.update(goat.control_flags.to_le_bytes());
            hasher.update(goat.max_life.to_le_bytes());
            hasher.update(goat.life.to_le_bytes());
            hasher.update(goat.pending_damage.to_le_bytes());
            hasher.update(goat.pending_attacker.to_le_bytes());
            hasher.update(goat.kill_credit_entity.to_le_bytes());
            hasher.update(goat.current_attacker.to_le_bytes());
            hasher.update(goat.rng.to_le_bytes());
            hasher.update(goat.yaw.to_le_bytes());
            hasher.update(goat.pitch.to_le_bytes());
            hasher.update(goat.roll.to_le_bytes());
            hasher.update(goat.position.x.to_le_bytes());
            hasher.update(goat.position.y.to_le_bytes());
            hasher.update(goat.position.z.to_le_bytes());
            hasher.update(goat.collision_half_width.to_le_bytes());
            hasher.update(goat.collision_half_depth.to_le_bytes());
            hasher.update(goat.collision_vertical_offset.to_le_bytes());
            hasher.update(goat.collision_half_height.to_le_bytes());
            hasher.update(goat.render_parameter_index.to_le_bytes());
            hasher.update(goat.animation_frame.to_le_bytes());
            hasher.update(goat.action.to_le_bytes());
            hasher.update(goat.phase.to_le_bytes());
            hasher.update(goat.proximity_countdown.to_le_bytes());
            hasher.update(goat.proximity_subcounter.to_le_bytes());
            hasher.update(goat.stage_var_1.to_le_bytes());
            hasher.update(goat.stage_var_2.to_le_bytes());
            hasher.update(goat.actual_speed.to_le_bytes());
            hasher.update(goat.minimum_speed.to_le_bytes());
            hasher.update(goat.maximum_speed.to_le_bytes());
            hasher.update(goat.mana.to_le_bytes());
            hasher.update(goat.partner_entity_index.unwrap_or(u16::MAX).to_le_bytes());
            hasher.update(goat.target_entity_index.unwrap_or(u16::MAX).to_le_bytes());
            // `word_0x34_52` is outside GoatState but affects the source
            // action-11 child-chain damage propagation.  Hash its null
            // sentinel explicitly so reset/replay detects link mutations.
            hasher.update(
                self.session
                    .state
                    .live
                    .goats
                    .child_links()
                    .get(family_index)
                    .copied()
                    .flatten()
                    .unwrap_or(u16::MAX)
                    .to_le_bytes(),
            );
        }
        format!("{:x}", hasher.finalize())
    }

    #[must_use]
    pub fn bad_stone_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(REMC2_SPEC_COMMIT.as_bytes());
        for stone in &self.session.state.live.bad_stones {
            hasher.update(stone.entity.record_bytes());
        }
        format!("{:x}", hasher.finalize())
    }

    #[must_use]
    pub fn bad_stone_presentation(
        &self,
        source_slot: u16,
    ) -> Option<&[u8; ENTITY_PARAMETER_BYTES]> {
        self.session
            .state
            .live
            .bad_stones
            .iter()
            .find(|stone| stone.entity.index == source_slot)
            .and_then(|stone| stone.entity.parameter_relation)
            .and_then(|index| {
                self.session
                    .definition
                    .presentation
                    .entity_parameters
                    .get(usize::from(index))
            })
    }

    #[must_use]
    pub fn bad_stone_particle_presentation(&self, source_slot: u16) -> Option<&ParticleParameter> {
        let index = self
            .session
            .state
            .live
            .bad_stones
            .iter()
            .find(|stone| stone.entity.index == source_slot)
            .and_then(|stone| stone.entity.particle_parameter_index)?;
        self.session
            .definition
            .presentation
            .particles
            .get(usize::from(index))
    }

    #[must_use]
    pub fn bad_stone_sprite_selector(&self, source_slot: u16) -> Option<u8> {
        let particle = self.bad_stone_particle_presentation(source_slot)?;
        self.session
            .definition
            .presentation
            .level_sprite_selectors
            .get(usize::from(particle.base_tmap))
            .copied()
    }

    pub fn bad_stone_sprite_disposition(
        &self,
        source_slot: u16,
    ) -> std::result::Result<SourceSpriteTemplateDisposition, String> {
        let stone = self
            .session
            .state
            .live
            .bad_stones
            .iter()
            .find(|stone| stone.entity.index == source_slot)
            .ok_or_else(|| format!("unknown Level 2 bad stone source slot {source_slot}"))?;
        level2_bad_stone_sprite_disposition(ImportedResetSpriteTuple {
            class: stone.entity.class,
            model: stone.entity.model,
            action: stone.entity.action,
            entity_parameter_index: stone.entity.parameter_relation.unwrap_or_default(),
            particle_parameter_index: stone.entity.particle_parameter_index.unwrap_or_default(),
            animation_frame: stone.entity.animation_frame(),
            control_flags: stone.entity.control_flags,
        })
    }

    pub fn step_bad_stone_action_9(&mut self, source_slot: u16) -> Result<Level2BadStoneStep> {
        let Some(stone) = self
            .session
            .state
            .live
            .bad_stones
            .iter_mut()
            .find(|stone| stone.entity.index == source_slot)
        else {
            return Err(NativeWorldError::new(format!(
                "Level 2 bad stone source slot {source_slot} is not active"
            )));
        };
        let terrain = &self.session.state.live.terrain;
        Ok(stone.step_action_9(terrain))
    }

    pub(super) fn sync_level2_model76_targets_from_entities(&mut self) {
        let source_arena = &mut self.session.state.live.economy;
        for source in &mut source_arena.economy.authored_model76 {
            let Some(target) = source_arena
                .entities
                .get(usize::from(source.target_entity_index))
                .filter(|target| target.active)
            else {
                source.target_control_flags = ENTITY_DRAW_DISABLED_BIT;
                source.target_life = -1;
                continue;
            };
            source.target_position = target.position;
            source.target_pitch = target.array_pitch();
            source.target_height_offset = target.array_yaw();
            source.target_control_flags = target.control_flags;
            source.target_life = target.life;
        }
    }

    /// Publish only economy events created since the last source-handler
    /// boundary. Direct economy API calls may populate the retained event
    /// history before an outer turn; the turn-start cursor deliberately
    /// excludes those stale entries from dispatcher output.
    pub(super) fn append_new_economy_events(&mut self, pass: u8, fallback_source_slot: u16) {
        let events = self.session.state.live.economy.events();
        let start = self
            .session
            .state
            .scratch
            .economy_event_cursor
            .min(events.len());
        let pending = events[start..].to_vec();
        self.session.state.scratch.economy_event_cursor = events.len();
        for event in pending {
            let source_slot = level2_economy_event_source_slot(event, fallback_source_slot);
            self.push_dispatch_effect(Level2DispatchEvent::Economy {
                pass,
                source_slot,
                event,
            });
        }
    }

    /// Run the one authored Level 2 stage-tag root which is live at reset.
    /// Pinned `sub_4A1E0(0,1)` consumes row 152's class-11 stage tag 8 and
    /// constructs the two group-8 model-45 producer rows.  Those producers
    /// then activate groups 22 and 27, where rows 155/156 become live.  The
    /// static outer route deliberately does not include those authored rows.
    pub(super) fn activate_level2_authored_group8(&mut self, pass: u8) {
        // Pinned `AddSwitch0B_20_6F1C0` scans the participating player rows and
        // broadcasts when any color's selected objective stage is exactly 2.
        // A live model-32 producer is not completion proof: the old
        // existence-only path activated group 8 on the first turn and could
        // manufacture spell sources prematurely.
        let completed_owner_color = self
            .session
            .state
            .live
            .core
            .players
            .iter()
            // The qualified campaign sessions are offline. Pinned
            // `AddSwitch0B_20_6F1C0` inspects player row zero only outside
            // multiplayer, even though Level 2 retains an AI opponent row.
            // A future multiplayer adapter needs the source setting bit; it
            // must not infer multiplayer from the reset player count.
            .take(1)
            .filter_map(|player| {
                if player.active_player_index < 0 {
                    return None;
                }
                let color = player.player_color_index();
                self.session
                    .state
                    .live
                    .core
                    .runtime
                    .objectives
                    .get(usize::from(color))
                    .filter(|objective| {
                        objective.stage_selectors[LEVEL2_SELECTED_OBJECTIVE_STAGE_INDEX] == 2
                    })
                    .map(|_| color)
            })
            .next();
        let Some(completed_owner_color) = completed_owner_color else {
            return;
        };
        if self.session.state.live.level2_group8_activated {
            return;
        }
        let Some(root_slot) = self
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .filter(|entity| {
                entity.active
                    && entity.class == 11
                    && entity.model == 32
                    && entity.action == 32
                    && entity.source_id == 8
            })
            .min_by_key(|entity| entity.index)
            .map(|entity| entity.index)
        else {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: 0,
                message: "Level 2 stage-tag root source row is absent".to_owned(),
            });
            return;
        };
        let Some(group_row_indices) = self
            .session
            .state
            .live
            .mission
            .level2_dispatch_group_rows(8)
        else {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: root_slot,
                message: "Level 2 activation artifact has no group-8 membership".to_owned(),
            });
            return;
        };
        let group_rows = group_row_indices
            .iter()
            .filter_map(|authored_index| {
                self.session
                    .state
                    .live
                    .mission
                    .authored_features()
                    .iter()
                    .copied()
                    .find(|feature| feature.index == *authored_index)
            })
            .collect::<Vec<_>>();
        if group_rows.len() != 2 {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: root_slot,
                message: format!(
                    "Level 2 stage-tag group 8 has {} rows, expected 2",
                    group_rows.len()
                ),
            });
            return;
        }
        let economy_before = self.session.state.live.economy.clone();
        let shared_before = self.session.state.live.shared_spells.clone();
        let terrain = &self.session.state.live.terrain;
        let mut constructed = Vec::with_capacity(group_rows.len());
        let result = (|| -> Result<()> {
            for feature in group_rows.iter().copied() {
                let position =
                    feature.world_position(terrain.ground_height(feature.world_position(0)));
                if (feature.kind, feature.subtype) != (10, 45) {
                    return Err(NativeWorldError::new(format!(
                        "unsupported Level 2 group-8 authored row {} ({}/{})",
                        feature.index, feature.kind, feature.subtype
                    )));
                }
                let source = self
                    .session
                    .state
                    .live
                    .economy
                    .allocate_authored_model45(
                        feature.index,
                        feature.parameter_2,
                        feature.parameter_1,
                        position,
                        terrain,
                    )
                    .ok_or_else(|| {
                        NativeWorldError::new(format!(
                            "Level 2 authored model45 row {} could not allocate",
                            feature.index
                        ))
                    })?;
                constructed.push((feature.index, source.source_entity_index));
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.session.state.live.economy = economy_before;
            self.session.state.live.shared_spells = shared_before;
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: root_slot,
                message: error.to_string(),
            });
            return;
        }
        self.session
            .state
            .live
            .level2_model45_activated
            .extend(constructed.iter().map(|(index, _)| *index));
        if let Some(root) = self
            .session
            .state
            .live
            .economy
            .entities
            .get_mut(usize::from(root_slot))
        {
            root.disable_drawing();
        }
        self.session.state.live.level2_group8_activated = true;
        self.session.state.live.level2_objective_pause = 1;
        self.session.state.live.level2_objective_done = true;
        self.session
            .state
            .live
            .core
            .runtime
            .mark_level2_objective_done_for(completed_owner_color);
        self.push_dispatch_effect(Level2DispatchEvent::AuthoredGroupActivated {
            pass,
            source_slot: root_slot,
            group: 8,
            authored_indices: constructed
                .into_iter()
                .map(|(index, _)| index)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        });
    }

    /// Run the bounded Level 2 model-4 exit-switch trigger for group 31.
    /// Pinned `AddSwitch0B_04_6F150` scans the live player list and consumes
    /// the switch when any live model-0 player owns an `IsLevelEnd != 0`
    /// objective. The resulting group contains one class-14/model-3 row and
    /// the twelve class-2/model-0 tree rows and the model-0 row-274 switch.
    /// Their constructors are kept on the shared typed arena so the source
    /// order and all-or-nothing allocation boundary remain observable.
    pub(super) fn activate_level2_authored_group31(&mut self, pass: u8, root_slot: u16) {
        if self
            .session
            .state
            .live
            .level2_class14_activated
            .contains(&244)
            || self.session.state.live.level2_tree_activated.contains(&260)
        {
            return;
        }
        let any_player_is_level_end_candidate =
            self.session.state.live.core.players.iter().any(|player| {
                if player.active_player_index < 0 {
                    return false;
                }
                let Some(entity) = self
                    .session
                    .state
                    .live
                    .economy
                    .entities
                    .get(usize::from(player.entity_index))
                else {
                    return false;
                };
                entity.active
                    && entity.class == 3
                    && entity.model == 0
                    && entity.life >= 0
                    && self
                        .session
                        .state
                        .live
                        .core
                        .runtime
                        .objectives
                        .get(usize::from(player.player_color_index()))
                        .is_some_and(|objective| objective.is_level_end != 0)
            });
        if !any_player_is_level_end_candidate {
            return;
        }
        let root_is_live = self
            .session
            .state
            .live
            .economy
            .entities
            .get(usize::from(root_slot))
            .is_some_and(|entity| {
                entity.active
                    && entity.index == root_slot
                    && entity.class == 11
                    && entity.model == 4
                    && entity.action == 4
                    && entity.source_id == 31
                    && !entity.drawing_disabled()
            });
        if !root_is_live {
            return;
        }
        let group_rows = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .filter(|feature| feature.activation_group == 31)
            .filter(|feature| {
                (feature.kind == 14 && feature.subtype == 3)
                    || (feature.kind == 11 && feature.subtype == 12 && feature.index == 209)
                    || (feature.kind == 2 && feature.subtype == 0)
                    || (feature.kind == 11 && feature.subtype == 0 && feature.index == 274)
            })
            .collect::<Vec<_>>();
        let mut group_rows = group_rows;
        group_rows.sort_unstable_by_key(|feature| feature.index);
        if group_rows.len() != 15 {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: root_slot,
                message: format!(
                    "Level 2 stage-tag group 31 has {} bounded rows, expected 15",
                    group_rows.len()
                ),
            });
            return;
        }
        let Some(model12_row) = group_rows
            .iter()
            .copied()
            .find(|feature| feature.kind == 11 && feature.subtype == 12 && feature.index == 209)
        else {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: root_slot,
                message: "Level 2 stage-tag group 31 is missing model-12 row 209".to_string(),
            });
            return;
        };
        let Some(feature) = group_rows
            .iter()
            .copied()
            .find(|feature| feature.kind == 14 && feature.subtype == 3 && feature.index == 244)
        else {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: root_slot,
                message: "Level 2 stage-tag group 31 is missing class-14/model-3 row 244"
                    .to_string(),
            });
            return;
        };
        const GROUP31_TREE_ROWS: [u16; 12] =
            [260, 261, 262, 264, 265, 266, 267, 268, 269, 270, 271, 272];
        let tree_rows = group_rows
            .iter()
            .copied()
            .filter(|feature| feature.kind == 2 && feature.subtype == 0)
            .collect::<Vec<_>>();
        if tree_rows
            .iter()
            .map(|feature| feature.index)
            .ne(GROUP31_TREE_ROWS.into_iter())
        {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: root_slot,
                message: "Level 2 stage-tag group 31 tree rows are not the qualified source set"
                    .to_string(),
            });
            return;
        }
        let Some(switch_row) = group_rows
            .iter()
            .copied()
            .find(|feature| feature.kind == 11 && feature.subtype == 0 && feature.index == 274)
        else {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: root_slot,
                message: "Level 2 stage-tag group 31 is missing model-0 switch row 274".to_string(),
            });
            return;
        };
        let Some(switch_stage_tag) = u16::try_from(switch_row.stage_tag).ok() else {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: root_slot,
                message: "Level 2 row 274 has an invalid stage tag".to_string(),
            });
            return;
        };
        let Some(switch_word_10) = i16::try_from(switch_row.word_10).ok() else {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: root_slot,
                message: "Level 2 row 274 has an invalid word_10".to_string(),
            });
            return;
        };
        let economy_before = self.session.state.live.economy.clone();
        let switch_producers_before = self
            .session
            .state
            .live
            .level2_switch_producers_activated
            .clone();
        let class14_before = self.session.state.live.level2_class14_activated.clone();
        let tree_before = self.session.state.live.level2_tree_activated.clone();
        let model12_position = model12_row.world_position(
            self.canonical_terrain()
                .ground_height(model12_row.world_position(0)),
        );
        let Some(model12_source) = self
            .session
            .state
            .live
            .economy
            .allocate_authored_switch_producer(
                model12_row.index,
                12,
                u16::try_from(model12_row.stage_tag).unwrap_or_default(),
                i16::try_from(model12_row.word_10).unwrap_or(i16::MAX),
                model12_position,
                &self.session.state.live.terrain,
            )
        else {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: root_slot,
                message: "Level 2 authored model-12 row 209 could not allocate".to_string(),
            });
            return;
        };
        self.session
            .state
            .live
            .level2_switch_producers_activated
            .insert(model12_row.index);
        let Some(class14_source) = self.allocate_level2_authored_class14(feature.index) else {
            self.session.state.live.economy = economy_before;
            self.session.state.live.level2_switch_producers_activated = switch_producers_before;
            self.session.state.live.level2_class14_activated = class14_before;
            self.session.state.live.level2_tree_activated = tree_before;
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: root_slot,
                message: format!(
                    "Level 2 authored class14 row {} could not allocate",
                    feature.index
                ),
            });
            return;
        };
        self.session
            .state
            .live
            .level2_class14_activated
            .insert(feature.index);
        let mut tree_sources = Vec::with_capacity(tree_rows.len());
        for tree_row in tree_rows {
            let Some(tree_source) = self.allocate_level2_authored_tree(tree_row.index) else {
                self.session.state.live.economy = economy_before;
                self.session.state.live.level2_switch_producers_activated = switch_producers_before;
                self.session.state.live.level2_class14_activated = class14_before;
                self.session.state.live.level2_tree_activated = tree_before;
                self.push_dispatch_effect(Level2DispatchEvent::Error {
                    pass,
                    source_slot: root_slot,
                    message: format!(
                        "Level 2 authored tree row {} could not allocate",
                        tree_row.index
                    ),
                });
                return;
            };
            self.session
                .state
                .live
                .level2_tree_activated
                .insert(tree_row.index);
            tree_sources.push(tree_source);
        }
        let switch_position = switch_row.world_position(
            self.canonical_terrain()
                .ground_height(switch_row.world_position(0)),
        );
        let Some(_) = self
            .session
            .state
            .live
            .economy
            .allocate_authored_switch_producer(
                switch_row.index,
                0,
                switch_stage_tag,
                switch_word_10,
                switch_position,
                &self.session.state.live.terrain,
            )
        else {
            self.session.state.live.economy = economy_before;
            self.session.state.live.level2_switch_producers_activated = switch_producers_before;
            self.session.state.live.level2_class14_activated = class14_before;
            self.session.state.live.level2_tree_activated = tree_before;
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: root_slot,
                message: "Level 2 authored model-0 row 274 could not allocate".to_string(),
            });
            return;
        };
        self.session
            .state
            .live
            .level2_switch_producers_activated
            .insert(switch_row.index);
        if let Some(root) = self
            .session
            .state
            .live
            .economy
            .entities
            .get_mut(usize::from(root_slot))
        {
            root.disable_drawing();
        }
        self.push_dispatch_effect(Level2DispatchEvent::AuthoredGroupActivated {
            pass,
            source_slot: root_slot,
            group: 31,
            authored_indices: group_rows
                .iter()
                .map(|feature| feature.index)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        });
        debug_assert!(
            self.economy()
                .authored_class14()
                .iter()
                .any(|row| row.source_entity_index == class14_source.source_entity_index)
        );
        debug_assert_eq!(model12_source.model, 12);
        debug_assert_eq!(tree_sources.len(), 12);
        debug_assert!(
            tree_sources
                .iter()
                .zip(GROUP31_TREE_ROWS)
                .all(|(source, authored_index)| source.authored_index == authored_index)
        );
    }

    pub(super) fn activate_level2_authored_class14_group(
        &mut self,
        pass: u8,
        producer_slot: u16,
        target_group: u16,
        subtype: i16,
    ) {
        let all_rows = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .filter(|feature| {
                feature.activation_group == i16::try_from(target_group).unwrap_or(-1)
                    && feature.kind == 14
                    && feature.subtype == subtype
            })
            .collect::<Vec<_>>();
        if all_rows.is_empty() {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: producer_slot,
                message: format!(
                    "Level 2 group {target_group} has no class-14/model-{subtype} rows"
                ),
            });
            return;
        }
        let pending_rows = all_rows
            .into_iter()
            .filter(|feature| {
                !self
                    .session
                    .state
                    .live
                    .level2_class14_activated
                    .contains(&feature.index)
                    && !self
                        .session
                        .state
                        .live
                        .economy
                        .authored_class14()
                        .iter()
                        .any(|row| row.authored_index == feature.index)
            })
            .collect::<Vec<_>>();
        if pending_rows.is_empty() {
            return;
        }
        let economy_before = self.session.state.live.economy.clone();
        let activated_before = self.session.state.live.level2_class14_activated.clone();
        let mut activated_rows = Vec::with_capacity(pending_rows.len());
        for feature in pending_rows {
            let Some(source_slot) = self.allocate_level2_authored_class14(feature.index) else {
                self.session.state.live.economy = economy_before;
                self.session.state.live.level2_class14_activated = activated_before;
                self.push_dispatch_effect(Level2DispatchEvent::Error {
                    pass,
                    source_slot: producer_slot,
                    message: format!(
                        "Level 2 authored class14 row {} could not allocate",
                        feature.index
                    ),
                });
                return;
            };
            self.session
                .state
                .live
                .level2_class14_activated
                .insert(feature.index);
            activated_rows.push((feature.index, source_slot.source_entity_index));
        }
        self.push_dispatch_effect(Level2DispatchEvent::AuthoredGroupActivated {
            pass,
            source_slot: producer_slot,
            group: target_group,
            authored_indices: activated_rows
                .iter()
                .map(|(index, _)| *index)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        });
        debug_assert!(activated_rows.iter().all(|(_, source_slot)| {
            self.economy
                .authored_class14()
                .iter()
                .any(|row| row.source_entity_index == *source_slot)
        }));
    }

    /// Consume one authored source group through remc2 `sub_4A1E0(id, 1)`.
    ///
    /// This is deliberately not a family activation transaction. The source
    /// rebuilds the allocator once, scans authored rows in numeric order, and
    /// clears every matching row after its individual constructor attempt.
    /// A later constructor failure therefore cannot restore an earlier row.
    /// Families not yet admitted by the shared campaign adapter are rejected
    /// before mutation instead of being silently skipped.
    fn consume_authored_group(
        &mut self,
        pass: u8,
        producer_slot: u16,
        target_group: u16,
    ) -> std::result::Result<(), String> {
        let target_group = i16::try_from(target_group)
            .map_err(|_| format!("authored group id {target_group} does not fit source i16"))?;

        // remc2 `sub_4A1E0` always rebuilds `NewEvent`'s free arena and
        // broadcasts the group through `sub_122C0` before scanning authored
        // rows. Those side effects still happen for an empty or already
        // consumed group.
        let reset_free_slots = self.session.baseline.economy.economy.free_slots.clone();
        self.session
            .state
            .live
            .economy
            .rebuild_free_arena(&reset_free_slots);
        self.session
            .state
            .live
            .core
            .runtime
            .broadcast_authored_group(target_group);

        let mut rows = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .filter(|feature| feature.activation_group == target_group && feature.kind != 0)
            .filter(|feature| {
                !self
                    .session
                    .state
                    .live
                    .authored_features_consumed
                    .contains(&feature.index)
            })
            .collect::<Vec<_>>();
        rows.sort_unstable_by_key(|feature| feature.index);
        if rows.is_empty() {
            return Ok(());
        }

        // Admit only constructors whose complete arena/map lifecycle is
        // already shared. Keep this preflight exhaustive so a newly reached
        // authored family is a visible capability error, never a no-op.
        if let Some(feature) = rows.iter().find(|feature| {
            !matches!(
                (feature.kind, feature.subtype),
                (2, 0 | 1)
                    | (5, 2 | 3 | 4 | 9 | 14 | 19)
                    | (10, 1 | 5 | 6 | 13 | 17 | 22 | 39 | 45 | 54 | 58 | 59 | 76)
                    | (11, 0 | 1 | 12 | 16 | 17 | 32)
                    | (14, 3 | 5)
                    | (15, 2 | 3 | 6 | 11)
            )
        }) {
            return Err(format!(
                "authored group {target_group} reaches unsupported constructor {}/{} at row {}",
                feature.kind, feature.subtype, feature.index
            ));
        }

        for feature in rows
            .iter()
            .filter(|feature| feature.kind == 11 && matches!(feature.subtype, 16 | 17))
        {
            let target_group = u16::try_from(feature.stage_tag).map_err(|_| {
                format!(
                    "authored countdown row {} has invalid target group {}",
                    feature.index, feature.stage_tag
                )
            })?;
            if target_group == 0 {
                return Err(format!(
                    "authored countdown row {} has zero target group",
                    feature.index
                ));
            }
            i16::try_from(feature.word_10).map_err(|_| {
                format!(
                    "authored countdown row {} has invalid word_10 {}",
                    feature.index, feature.word_10
                )
            })?;
        }
        for feature in rows
            .iter()
            .filter(|feature| (feature.kind, feature.subtype) == (5, 19))
        {
            if self.authored_firefly_stage_route(*feature).is_none() {
                return Err(format!(
                    "authored Firefly row {} has no valid StageVar-1 route",
                    feature.index
                ));
            }
        }

        let mut consumed = Vec::with_capacity(rows.len());
        for feature in rows {
            // Source consumes the authored row whether NewEvent succeeds.
            self.session
                .state
                .live
                .authored_features_consumed
                .insert(feature.index);
            consumed.push(feature.index);
            match (feature.kind, feature.subtype) {
                (2, 0) => {
                    if self.allocate_level2_authored_tree(feature.index).is_some() {
                        self.session
                            .state
                            .live
                            .level2_tree_activated
                            .insert(feature.index);
                    }
                }
                (2, 1) => {
                    let _ = self.allocate_authored_stone_feature(feature);
                }
                (5, 2) => {
                    if let Some(source) = self.allocate_level2_authored_bee_feature(feature) {
                        self.session
                            .state
                            .live
                            .level2_bees_activated
                            .insert(feature.index);
                        debug_assert!(
                            self.economy
                                .authored_bees()
                                .iter()
                                .any(|row| row.source_entity_index == source.source_entity_index)
                        );
                    }
                }
                (5, 4) => {
                    let position = feature.world_position(
                        self.canonical_terrain()
                            .ground_height(feature.world_position(0)),
                    );
                    if self
                        .session
                        .state
                        .live
                        .economy
                        .economy
                        .allocate_campaign_archer(
                            feature.index,
                            position,
                            &self.session.state.live.terrain,
                        )
                        .is_some()
                    {
                        self.session
                            .state
                            .live
                            .level2_class5_activated
                            .insert(feature.index);
                    }
                }
                (5, 19) => {
                    if self.allocate_authored_firefly_feature(feature).is_some() {
                        self.session
                            .state
                            .live
                            .level2_class5_activated
                            .insert(feature.index);
                    }
                }
                (5, model @ (3 | 9 | 14)) => {
                    let position = feature.world_position(
                        self.canonical_terrain()
                            .ground_height(feature.world_position(0)),
                    );
                    if self
                        .session
                        .state
                        .live
                        .economy
                        .allocate_authored_class5(
                            feature.index,
                            u8::try_from(model).expect("authored class-5 model fits u8"),
                            position,
                            &self.session.state.live.terrain,
                        )
                        .is_some()
                    {
                        self.session
                            .state
                            .live
                            .level2_class5_activated
                            .insert(feature.index);
                    }
                }
                (10, 1) => {
                    let position = feature.world_position(
                        self.canonical_terrain()
                            .ground_height(feature.world_position(0)),
                    );
                    if self
                        .session
                        .state
                        .live
                        .economy
                        .allocate_authored_model1(
                            feature.index,
                            position,
                            &self.session.state.live.terrain,
                        )
                        .is_some()
                    {
                        self.session
                            .state
                            .live
                            .level2_model1_activated
                            .insert(feature.index);
                    }
                }
                (10, 5) => {
                    let position = feature.world_position(0);
                    let live = &mut self.session.state.live;
                    let _ = live
                        .economy
                        .allocate_authored_model5_water_splash(position, &live.terrain);
                }
                (10, 6) => {
                    if self
                        .allocate_level2_authored_model6(feature.index)
                        .is_some()
                    {
                        self.session
                            .state
                            .live
                            .level2_model6_activated
                            .insert(feature.index);
                    }
                }
                (10, 13) => {
                    if self
                        .allocate_level2_authored_model13(feature.index)
                        .is_some()
                    {
                        self.session
                            .state
                            .live
                            .level2_model13_activated
                            .insert(feature.index);
                    }
                }
                (10, 17) => {
                    if self
                        .allocate_level2_authored_model17(feature.index)
                        .is_some()
                    {
                        self.session
                            .state
                            .live
                            .level2_model17_activated
                            .insert(feature.index);
                    }
                }
                (10, 22) => {
                    if self
                        .allocate_level2_authored_model22(feature.index)
                        .is_some()
                    {
                        self.session
                            .state
                            .live
                            .level2_model22_activated
                            .insert(feature.index);
                    }
                }
                (10, 39) => {
                    let position = feature.world_position(
                        self.canonical_terrain()
                            .ground_height(feature.world_position(0)),
                    );
                    if self
                        .session
                        .state
                        .live
                        .economy
                        .allocate_authored_model39(feature.index, producer_slot, position)
                        .is_some()
                    {
                        self.session
                            .state
                            .live
                            .level2_model39_activated
                            .insert(feature.index);
                    }
                }
                (10, 45) => {
                    let position = feature.world_position(
                        self.session
                            .state
                            .live
                            .terrain
                            .ground_height(feature.world_position(0)),
                    );
                    if let Some(source) = self.session.state.live.economy.allocate_authored_model45(
                        feature.index,
                        feature.parameter_2,
                        feature.parameter_1,
                        position,
                        &self.session.state.live.terrain,
                    ) {
                        self.session
                            .state
                            .live
                            .level2_model45_activated
                            .insert(feature.index);
                        debug_assert!(
                            self.economy
                                .authored_model45()
                                .iter()
                                .any(|row| row.source_entity_index == source.source_entity_index)
                        );
                    }
                }
                (10, 54) => {
                    if self
                        .allocate_level2_mana_attraction_field(
                            feature.index,
                            MANA_ATTRACTION_ACTION,
                        )
                        .is_some()
                    {
                        self.session
                            .state
                            .live
                            .level2_mana_attraction_activated
                            .insert(feature.index);
                    }
                }
                (10, 58) => {
                    if self
                        .allocate_level2_authored_model53(feature.index)
                        .is_some()
                    {
                        self.session
                            .state
                            .live
                            .level2_model53_activated
                            .insert(feature.index);
                    }
                }
                (10, 59) => {
                    if self
                        .allocate_level2_authored_model59(feature.index)
                        .is_some()
                    {
                        self.session
                            .state
                            .live
                            .level2_model59_activated
                            .insert(feature.index);
                    }
                }
                (10, 76) => {
                    if self
                        .allocate_level2_authored_model76(feature.index)
                        .is_some()
                    {
                        self.session
                            .state
                            .live
                            .level2_model76_activated
                            .insert(feature.index);
                    }
                }
                (11, model @ (0 | 1 | 12)) => {
                    let position = feature.world_position(
                        self.canonical_terrain()
                            .ground_height(feature.world_position(0)),
                    );
                    if self
                        .session
                        .state
                        .live
                        .economy
                        .allocate_authored_switch_producer(
                            feature.index,
                            u8::try_from(model).expect("authored switch model fits u8"),
                            u16::try_from(feature.stage_tag).unwrap_or_default(),
                            i16::try_from(feature.word_10).unwrap_or(i16::MAX),
                            position,
                            &self.session.state.live.terrain,
                        )
                        .is_some()
                    {
                        self.session
                            .state
                            .live
                            .level2_switch_producers_activated
                            .insert(feature.index);
                    }
                }
                (11, model @ (16 | 17)) => {
                    let target_group = u16::try_from(feature.stage_tag)
                        .expect("authored countdown target was preflighted");
                    let word_10 = i16::try_from(feature.word_10)
                        .expect("authored countdown word_10 was preflighted");
                    let position = feature.world_position(
                        self.canonical_terrain()
                            .ground_height(feature.world_position(0)),
                    );
                    if self
                        .session
                        .state
                        .live
                        .economy
                        .allocate_authored_switch_producer(
                            feature.index,
                            u8::try_from(model).expect("authored countdown model fits u8"),
                            target_group,
                            word_10,
                            position,
                            &self.session.state.live.terrain,
                        )
                        .is_some()
                    {
                        self.session
                            .state
                            .live
                            .level2_switch_producers_activated
                            .insert(feature.index);
                    }
                }
                (11, 32) => {
                    if self
                        .allocate_level2_authored_model32(feature.index)
                        .is_some()
                    {
                        self.session
                            .state
                            .live
                            .level2_model32_activated
                            .insert(feature.index);
                    }
                }
                (14, 3 | 5) => {
                    if let Some(source) = self.allocate_level2_authored_class14(feature.index) {
                        self.session
                            .state
                            .live
                            .level2_class14_activated
                            .insert(feature.index);
                        debug_assert!(
                            self.economy
                                .authored_class14()
                                .iter()
                                .any(|row| row.source_entity_index == source.source_entity_index)
                        );
                    }
                }
                (15, 2 | 3 | 6 | 11) => {
                    let terrain = &self.session.state.live.terrain;
                    self.session
                        .state
                        .live
                        .shared_spells
                        .materialize_authored_spell_pickup(
                            &mut self.session.state.live.economy,
                            feature,
                            terrain,
                        )
                        .map_err(|error| error.to_string())?;
                }
                _ => unreachable!("authored group constructor was preflighted"),
            }
        }
        self.push_dispatch_effect(Level2DispatchEvent::AuthoredGroupActivated {
            pass,
            source_slot: producer_slot,
            group: target_group.cast_unsigned(),
            authored_indices: consumed.into_boxed_slice(),
        });
        Ok(())
    }

    /// Step the group-zero class-11 model-0/model-1 roots that the qualified
    /// reset already imported as live entities.  The entity remains the
    /// source/map authority; this wrapper owns only the callback phase and
    /// ten-tick rebroadcast cooldown. Dedicated slot-25/group-10 ownership is
    /// intentionally excluded at construction, so it cannot race this path.
    pub(super) fn step_level2_imported_switch_roots(&mut self, pass: u8) {
        let mut source_slots = self
            .session
            .state
            .live
            .level2_imported_switch_roots
            .iter()
            .map(|root| root.source_slot)
            .collect::<Vec<_>>();
        source_slots.sort_unstable();
        for source_slot in source_slots {
            let Some(root) = self
                .session
                .state
                .live
                .level2_imported_switch_roots
                .iter()
                .find(|root| root.source_slot == source_slot && root.active)
                .copied()
            else {
                continue;
            };
            let (position, model, drawing_enabled, phase, cooldown, target_group_id) = (
                root.position,
                root.model,
                root.drawing_enabled,
                root.phase,
                root.cooldown,
                root.target_group,
            );
            if !drawing_enabled {
                continue;
            }
            let switch_extent = root.word_10.cast_unsigned().wrapping_shl(8).cast_signed();
            let overlap = self.level2_model0_overlap_matches(
                position,
                switch_extent,
                switch_extent,
                model == 0,
                |entity| {
                    if entity.index == self.session.state.live.core.state.entity_index {
                        self.session.state.live.core.state.position
                    } else {
                        entity.position
                    }
                },
            );
            let matched_owner = if model == 0 && overlap {
                self.level2_model0_overlap_owner(position, switch_extent, switch_extent, |entity| {
                    if entity.index == self.session.state.live.core.state.entity_index {
                        self.session.state.live.core.state.position
                    } else {
                        entity.position
                    }
                })
            } else {
                None
            };
            let target_group = if cooldown == 0 && phase & 7 == 0 && overlap {
                Some(target_group_id)
            } else {
                None
            };
            let next_cooldown = if target_group.is_some() {
                10
            } else if cooldown != 0 && !overlap {
                cooldown.saturating_sub(1)
            } else {
                cooldown
            };
            let next_phase = phase.wrapping_add(1);
            if let Some(root) = self
                .session
                .state
                .live
                .level2_imported_switch_roots
                .iter_mut()
                .find(|root| root.source_slot == source_slot)
            {
                root.cooldown = next_cooldown;
                root.phase = next_phase;
            }
            if let Some(entity) = self
                .session
                .state
                .live
                .economy
                .entities
                .get_mut(usize::from(source_slot))
            {
                entity.set_phase(next_phase);
                entity.set_source_word_10(u16::from(next_cooldown));
            }
            if let Some(target_group) = target_group {
                if let Some(owner_entity_index) = matched_owner {
                    self.session.state.live.economy.prepare_sound(
                        source_slot,
                        owner_entity_index,
                        -1,
                        41,
                    );
                    self.append_new_economy_events(pass, source_slot);
                }
                self.activate_level2_authored_switch_group(pass, source_slot, target_group);
                // `AddSwitch0B_00_6F030` (model 0) and
                // `CheckpointArrived_6F070` (model 1) both consume their
                // rooted switch after a successful source overlap decision.
                if let Some(root) = self
                    .session
                    .state
                    .live
                    .level2_imported_switch_roots
                    .iter_mut()
                    .find(|root| root.source_slot == source_slot)
                {
                    root.drawing_enabled = false;
                }
                if let Some(entity) = self
                    .session
                    .state
                    .live
                    .economy
                    .entities
                    .get_mut(usize::from(source_slot))
                {
                    entity.disable_drawing();
                }
            }
        }
    }

    /// Consume authored model-45 rows whose source `xtype` is zero.  The
    /// pinned `GenerateEvents_49290` path visits these rows but returns before
    /// `NewEvent`; retain the authored-row dispatch evidence without coercing
    /// the row into the BUILD[21] model-45 allocator.
    pub(super) fn activate_level2_authored_model45_noop(
        &mut self,
        pass: u8,
        producer_slot: u16,
        target_group: u16,
    ) {
        let Some(group_row_indices) = self
            .session
            .state
            .live
            .mission
            .level2_dispatch_group_rows(target_group)
        else {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: producer_slot,
                message: format!(
                    "Level 2 activation artifact has no group-{target_group} membership"
                ),
            });
            return;
        };
        let rows = group_row_indices
            .iter()
            .filter_map(|authored_index| {
                self.session
                    .state
                    .live
                    .mission
                    .authored_features()
                    .iter()
                    .copied()
                    .find(|feature| feature.index == *authored_index)
            })
            .filter(|feature| {
                feature.kind == 10
                    && feature.subtype == 45
                    && feature.parameter_2 == 0
                    && !self
                        .session
                        .state
                        .live
                        .level2_model45_activated
                        .contains(&feature.index)
            })
            .collect::<Vec<_>>();
        if rows.is_empty() {
            return;
        }
        let authored_indices = rows
            .iter()
            .map(|feature| {
                self.session
                    .state
                    .live
                    .level2_model45_activated
                    .insert(feature.index);
                feature.index
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        self.push_dispatch_effect(Level2DispatchEvent::AuthoredGroupActivated {
            pass,
            source_slot: producer_slot,
            group: target_group,
            authored_indices,
        });
    }

    /// Consume authored class-5/model-14 rows through the typed AddTrader
    /// constructor.  This is a distinct same-pass consumer from the retained
    /// switch/model-39 batch, preserving source event grouping and making
    /// allocation failure transactional for the complete trader band.
    pub(super) fn activate_level2_authored_class5_model14_group(
        &mut self,
        pass: u8,
        producer_slot: u16,
        target_group: u16,
    ) {
        let Some(group_row_indices) = self
            .session
            .state
            .live
            .mission
            .level2_dispatch_group_rows(target_group)
        else {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: producer_slot,
                message: format!(
                    "Level 2 activation artifact has no group-{target_group} membership"
                ),
            });
            return;
        };
        let rows = group_row_indices
            .iter()
            .filter_map(|authored_index| {
                self.session
                    .state
                    .live
                    .mission
                    .authored_features()
                    .iter()
                    .copied()
                    .find(|feature| feature.index == *authored_index)
            })
            .filter(|feature| {
                feature.kind == 5
                    && feature.subtype == 14
                    && !self
                        .session
                        .state
                        .live
                        .level2_class5_activated
                        .contains(&feature.index)
            })
            .collect::<Vec<_>>();
        if rows.is_empty() {
            return;
        }
        let economy_before = self.session.state.live.economy.clone();
        let activated_before = self.session.state.live.level2_class5_activated.clone();
        let mut activated = Vec::with_capacity(rows.len());
        for feature in rows {
            let terrain = &self.session.state.live.terrain;
            let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
            let Some(source) = self.session.state.live.economy.allocate_authored_class5(
                feature.index,
                14,
                position,
                terrain,
            ) else {
                self.session.state.live.economy = economy_before;
                self.session.state.live.level2_class5_activated = activated_before;
                self.push_dispatch_effect(Level2DispatchEvent::Error {
                    pass,
                    source_slot: producer_slot,
                    message: format!(
                        "Level 2 authored class5 model-14 row {} could not allocate",
                        feature.index
                    ),
                });
                return;
            };
            self.session
                .state
                .live
                .level2_class5_activated
                .insert(feature.index);
            activated.push((feature.index, source.source_entity_index));
        }
        self.push_dispatch_effect(Level2DispatchEvent::AuthoredGroupActivated {
            pass,
            source_slot: producer_slot,
            group: target_group,
            authored_indices: activated
                .into_iter()
                .map(|(index, _)| index)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        });
    }

    /// Materialize the next authored producer group selected by a live
    /// switch. Group 28 is the typed class-14/model-5 branch selected by the
    /// real group-31 model-0 producer; all other groups retain their existing
    /// source-family dispatch.
    pub(super) fn activate_level2_authored_switch_group(
        &mut self,
        pass: u8,
        producer_slot: u16,
        target_group: u16,
    ) {
        if target_group == 28 {
            self.activate_level2_authored_class14_group(pass, producer_slot, target_group, 5);
            return;
        }
        let rows = if let Some(group_row_indices) = self
            .session
            .state
            .live
            .mission
            .level2_dispatch_group_rows(target_group)
        {
            group_row_indices
                .iter()
                .filter_map(|authored_index| {
                    self.session
                        .state
                        .live
                        .mission
                        .authored_features()
                        .iter()
                        .copied()
                        .find(|feature| feature.index == *authored_index)
                })
                .collect::<Vec<_>>()
        } else {
            self.session
                .state
                .live
                .mission
                .authored_features()
                .iter()
                .copied()
                .filter(|feature| {
                    let is_switch = feature.kind == 11
                        && matches!(feature.subtype, 0 | 1)
                        && feature.index != 274;
                    let is_model1 =
                        feature.kind == 10 && feature.subtype == 1 && target_group == 20;
                    let is_class5_model4 =
                        feature.kind == 5 && feature.subtype == 4 && target_group == 20;
                    let is_class5_model3 = feature.kind == 5
                        && feature.subtype == 3
                        && matches!(target_group, 1 | 5 | 7);
                    let is_model22 = feature.kind == 10 && feature.subtype == 22;
                    let is_model59 = feature.kind == 10
                        && feature.subtype == 59
                        && matches!(target_group, 19 | 23);
                    let is_model53 =
                        feature.kind == 10 && feature.subtype == 58 && target_group == 3;
                    let is_model39 = feature.kind == 10
                        && feature.subtype == 39
                        && matches!(target_group, 1 | 3 | 5 | 7 | 24 | 26);
                    let is_model6 =
                        feature.kind == 10 && feature.subtype == 6 && target_group == 24;
                    let is_model32 = feature.kind == 11
                        && feature.subtype == 32
                        && matches!(target_group, 1 | 17 | 18);
                    feature.activation_group == i16::try_from(target_group).unwrap_or(-1)
                        && (is_switch
                            || is_model1
                            || is_class5_model4
                            || is_class5_model3
                            || is_model22
                            || is_model39
                            || is_model6
                            || is_model59
                            || is_model53
                            || is_model32)
                })
                .collect::<Vec<_>>()
        };
        let mut activated = Vec::new();
        for feature in rows {
            if feature.kind == 10 && feature.subtype == 1 && target_group == 20 {
                if self
                    .session
                    .state
                    .live
                    .level2_model1_activated
                    .contains(&feature.index)
                {
                    continue;
                }
                let terrain = &self.session.state.live.terrain;
                let position =
                    feature.world_position(terrain.ground_height(feature.world_position(0)));
                let Some(source) = self.session.state.live.economy.allocate_authored_model1(
                    feature.index,
                    position,
                    terrain,
                ) else {
                    self.push_dispatch_effect(Level2DispatchEvent::Error {
                        pass,
                        source_slot: producer_slot,
                        message: format!(
                            "Level 2 authored model1 group {target_group} row {} could not allocate",
                            feature.index
                        ),
                    });
                    continue;
                };
                self.session
                    .state
                    .live
                    .level2_model1_activated
                    .insert(feature.index);
                activated.push((feature.index, source.source_entity_index));
                continue;
            }
            if feature.kind == 5 && feature.subtype == 4 && target_group == 20 {
                if self
                    .session
                    .state
                    .live
                    .level2_class5_activated
                    .contains(&feature.index)
                {
                    continue;
                }
                let terrain = &self.session.state.live.terrain;
                let position =
                    feature.world_position(terrain.ground_height(feature.world_position(0)));
                let Some(source) = self.session.state.live.economy.allocate_authored_class5(
                    feature.index,
                    4,
                    position,
                    terrain,
                ) else {
                    self.push_dispatch_effect(Level2DispatchEvent::Error {
                        pass,
                        source_slot: producer_slot,
                        message: format!(
                            "Level 2 authored class5 group {target_group} row {} could not allocate",
                            feature.index
                        ),
                    });
                    continue;
                };
                self.session
                    .state
                    .live
                    .level2_class5_activated
                    .insert(feature.index);
                activated.push((feature.index, source.source_entity_index));
                continue;
            }
            if feature.kind == 5 && feature.subtype == 3 && matches!(target_group, 1 | 5 | 7) {
                if self
                    .session
                    .state
                    .live
                    .level2_class5_activated
                    .contains(&feature.index)
                {
                    continue;
                }
                let terrain = &self.session.state.live.terrain;
                let position =
                    feature.world_position(terrain.ground_height(feature.world_position(0)));
                let Some(source) = self.session.state.live.economy.allocate_authored_class5(
                    feature.index,
                    3,
                    position,
                    terrain,
                ) else {
                    self.push_dispatch_effect(Level2DispatchEvent::Error {
                        pass,
                        source_slot: producer_slot,
                        message: format!(
                            "Level 2 authored class5 model-3 group {target_group} row {} could not allocate",
                            feature.index
                        ),
                    });
                    continue;
                };
                self.session
                    .state
                    .live
                    .level2_class5_activated
                    .insert(feature.index);
                activated.push((feature.index, source.source_entity_index));
                continue;
            }
            if feature.kind == 10 && feature.subtype == 22 {
                if self
                    .session
                    .state
                    .live
                    .level2_model22_activated
                    .contains(&feature.index)
                {
                    continue;
                }
                let Some(source) = self.allocate_level2_authored_model22(feature.index) else {
                    self.push_dispatch_effect(Level2DispatchEvent::Error {
                        pass,
                        source_slot: producer_slot,
                        message: format!(
                            "Level 2 authored wind group {target_group} row {} could not allocate",
                            feature.index
                        ),
                    });
                    continue;
                };
                self.session
                    .state
                    .live
                    .level2_model22_activated
                    .insert(feature.index);
                activated.push((feature.index, source.source_entity_index));
                continue;
            }
            if feature.kind == 10 && feature.subtype == 76 {
                if self
                    .session
                    .state
                    .live
                    .level2_model76_activated
                    .contains(&feature.index)
                {
                    continue;
                }
                let Some(source) = self.allocate_level2_authored_model76(feature.index) else {
                    self.push_dispatch_effect(Level2DispatchEvent::Error {
                        pass,
                        source_slot: producer_slot,
                        message: format!(
                            "Level 2 authored model76 group {target_group} row {} could not allocate",
                            feature.index
                        ),
                    });
                    continue;
                };
                self.session
                    .state
                    .live
                    .level2_model76_activated
                    .insert(feature.index);
                activated.push((feature.index, source.source_entity_index));
                continue;
            }
            if feature.kind == 10
                && feature.subtype == 39
                && matches!(target_group, 1 | 3 | 5 | 7 | 24 | 26)
            {
                if self
                    .session
                    .state
                    .live
                    .level2_model39_activated
                    .contains(&feature.index)
                {
                    continue;
                }
                let position = feature.world_position(
                    self.canonical_terrain()
                        .ground_height(feature.world_position(0)),
                );
                let Some(source) = self.session.state.live.economy.allocate_authored_model39(
                    feature.index,
                    producer_slot,
                    position,
                ) else {
                    self.push_dispatch_effect(Level2DispatchEvent::Error {
                        pass,
                        source_slot: producer_slot,
                        message: format!(
                            "Level 2 authored mana sphere group {target_group} row {} could not allocate",
                            feature.index
                        ),
                    });
                    continue;
                };
                self.session
                    .state
                    .live
                    .level2_model39_activated
                    .insert(feature.index);
                activated.push((feature.index, source.source_entity_index));
                continue;
            }
            if feature.kind == 10 && feature.subtype == 6 && target_group == 24 {
                if self
                    .session
                    .state
                    .live
                    .level2_model6_activated
                    .contains(&feature.index)
                {
                    continue;
                }
                let Some(source) = self.allocate_level2_authored_model6(feature.index) else {
                    self.push_dispatch_effect(Level2DispatchEvent::Error {
                        pass,
                        source_slot: producer_slot,
                        message: format!(
                            "Level 2 authored model6 group {target_group} row {} could not allocate",
                            feature.index
                        ),
                    });
                    continue;
                };
                self.session
                    .state
                    .live
                    .level2_model6_activated
                    .insert(feature.index);
                activated.push((feature.index, source.source_entity_index));
                continue;
            }
            if feature.kind == 10 && feature.subtype == 59 && matches!(target_group, 19 | 23) {
                if self
                    .session
                    .state
                    .live
                    .level2_model59_activated
                    .contains(&feature.index)
                {
                    continue;
                }
                let Some(source) = self.allocate_level2_authored_model59(feature.index) else {
                    self.push_dispatch_effect(Level2DispatchEvent::Error {
                        pass,
                        source_slot: producer_slot,
                        message: format!(
                            "Level 2 authored checkpoint group {target_group} row {} could not allocate",
                            feature.index
                        ),
                    });
                    continue;
                };
                self.session
                    .state
                    .live
                    .level2_model59_activated
                    .insert(feature.index);
                activated.push((feature.index, source.source_entity_index));
                continue;
            }
            if feature.kind == 10 && feature.subtype == 58 && target_group == 3 {
                if self
                    .session
                    .state
                    .live
                    .level2_model53_activated
                    .contains(&feature.index)
                {
                    continue;
                }
                let Some(source) = self.allocate_level2_authored_model53(feature.index) else {
                    self.push_dispatch_effect(Level2DispatchEvent::Error {
                        pass,
                        source_slot: producer_slot,
                        message: format!(
                            "Level 2 authored model53 group {target_group} row {} could not allocate",
                            feature.index
                        ),
                    });
                    continue;
                };
                self.session
                    .state
                    .live
                    .level2_model53_activated
                    .insert(feature.index);
                activated.push((feature.index, source.source_entity_index));
                continue;
            }
            if feature.kind == 11 && feature.subtype == 32 {
                if self
                    .session
                    .state
                    .live
                    .level2_model32_activated
                    .contains(&feature.index)
                    || self
                        .session
                        .state
                        .live
                        .economy
                        .authored_model32()
                        .iter()
                        .any(|source| source.authored_index == feature.index)
                {
                    continue;
                }
                let Some(source) = self.allocate_level2_authored_model32(feature.index) else {
                    self.push_dispatch_effect(Level2DispatchEvent::Error {
                        pass,
                        source_slot: producer_slot,
                        message: format!(
                            "Level 2 authored model32 group {target_group} row {} could not allocate",
                            feature.index
                        ),
                    });
                    continue;
                };
                self.session
                    .state
                    .live
                    .level2_model32_activated
                    .insert(feature.index);
                activated.push((feature.index, source.source_entity_index));
                continue;
            }
            if self
                .session
                .state
                .live
                .level2_switch_producers_activated
                .contains(&feature.index)
                || self
                    .session
                    .state
                    .live
                    .economy
                    .authored_switch_producers()
                    .iter()
                    .any(|source| source.authored_index == feature.index)
            {
                continue;
            }
            if feature.kind != 11 || !matches!(feature.subtype, 0 | 1) {
                continue;
            }
            let model = u8::try_from(feature.subtype).unwrap_or(u8::MAX);
            let position = feature.world_position(
                self.canonical_terrain()
                    .ground_height(feature.world_position(0)),
            );
            let Some(source) = self
                .session
                .state
                .live
                .economy
                .allocate_authored_switch_producer(
                    feature.index,
                    model,
                    u16::try_from(feature.stage_tag).unwrap_or_default(),
                    i16::try_from(feature.word_10).unwrap_or(i16::MAX),
                    position,
                    &self.session.state.live.terrain,
                )
            else {
                self.push_dispatch_effect(Level2DispatchEvent::Error {
                    pass,
                    source_slot: producer_slot,
                    message: format!(
                        "Level 2 authored switch group {target_group} row {} could not allocate",
                        feature.index
                    ),
                });
                continue;
            };
            self.session
                .state
                .live
                .level2_switch_producers_activated
                .insert(feature.index);
            activated.push((feature.index, source.source_entity_index));
        }
        if !activated.is_empty() {
            self.push_dispatch_effect(Level2DispatchEvent::AuthoredGroupActivated {
                pass,
                source_slot: producer_slot,
                group: target_group,
                authored_indices: activated
                    .into_iter()
                    .map(|(index, _)| index)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            });
            // A source callback invokes `sub_4A1E0` synchronously. Re-run the
            // already-owned downstream stage-tag consumers so rows such as
            // group-5 class-14/model-54 become visible in this same pass.
            self.activate_level2_authored_mana_attraction_fields(pass);
            self.activate_level2_authored_class14(pass);
            self.activate_level2_authored_bees(pass);
            self.activate_level2_authored_model45_noop(pass, producer_slot, target_group);
            self.activate_level2_authored_class5_model14_group(pass, producer_slot, target_group);
        }
    }

    pub(super) fn live_level2_class11_producers<F>(
        &self,
        mut producer_matches: F,
    ) -> Vec<(u16, u16)>
    where
        F: FnMut(&EntitySnapshot) -> bool,
    {
        let mut producers = self
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .filter(|entity| {
                entity.active
                    && entity.class == 11
                    // Model-4 exit switches consume objective IsLevelEnd in
                    // their own source callback; they are not stage-tag
                    // producers for this generic class-14 path. Their full
                    // group-31 dispatch remains a separate production gap.
                    && entity.model != 4
                    && !entity.drawing_disabled()
                    && entity.source_id != 0
                    && producer_matches(entity)
            })
            .map(|entity| (entity.index, entity.source_id))
            .collect::<Vec<_>>();
        // Authored switch rows live in the shared economy arena rather than
        // the imported reset entity array. Their stage_tag is the same source
        // group value that `sub_4A310` writes into an imported class-11 row;
        // include them in the producer view so all existing data-driven
        // consumers observe one source list.
        producers.extend(
            self.session
                .state
                .live
                .economy
                .authored_switch_producers()
                .iter()
                // A fired switch has already run its stage-tag scan but stays
                // in the source arena. Its drawing bit is hidden by the
                // source callback; retain the fired row for downstream
                // source-order bookkeeping just as model-32 does below.
                .filter(|source| source.active && source.fired)
                .map(|source| (source.source_entity_index, source.stage_tag)),
        );
        // Model-32 disables drawing as part of its successful objective
        // callback, but `sub_4A1E0` has already consumed its stage tag. Keep
        // the fired source in the producer view so the same downstream group
        // remains observable after the source entity is hidden.
        producers.extend(
            self.session
                .state
                .live
                .economy
                .authored_model32()
                .iter()
                .filter(|source| source.active && source.fired)
                .map(|source| (source.source_entity_index, source.stage_tag)),
        );
        producers.sort_unstable_by_key(|(slot, _)| *slot);
        producers
    }

    pub(super) fn activate_level2_rows_from_live_class11_producers<
        FProducer,
        FMatch,
        FSeen,
        FAllocate,
    >(
        &mut self,
        pass: u8,
        allocation_label: &str,
        producer_matches: FProducer,
        mut matches_feature: FMatch,
        mut already_seen: FSeen,
        mut allocate: FAllocate,
    ) where
        FProducer: FnMut(&EntitySnapshot) -> bool,
        FMatch: FnMut(AuthoredFeature) -> bool,
        FSeen: FnMut(&Self, u16) -> bool,
        FAllocate: FnMut(&mut Self, u16) -> Option<u16>,
    {
        let producer_groups = self.live_level2_class11_producers(producer_matches);
        for (producer_slot, producer_group) in producer_groups {
            let candidate_rows = self
                .session
                .state
                .live
                .mission
                .level2_dispatch_group_rows(producer_group)
                .map_or_else(
                    || {
                        self.session
                            .state
                            .live
                            .mission
                            .authored_features()
                            .iter()
                            .copied()
                            .filter(|feature| {
                                feature.activation_group == producer_group.cast_signed()
                            })
                            .collect::<Vec<_>>()
                    },
                    |authored_indices| {
                        authored_indices
                            .iter()
                            .filter_map(|authored_index| {
                                self.session
                                    .state
                                    .live
                                    .mission
                                    .authored_features()
                                    .iter()
                                    .copied()
                                    .find(|feature| feature.index == *authored_index)
                            })
                            .collect::<Vec<_>>()
                    },
                );
            let rows = candidate_rows
                .into_iter()
                .filter(|feature| matches_feature(*feature) && !already_seen(self, feature.index))
                .collect::<Vec<_>>();
            for feature in rows {
                let Some(source_slot) = allocate(self, feature.index) else {
                    self.push_dispatch_effect(Level2DispatchEvent::Error {
                        pass,
                        source_slot: producer_slot,
                        message: format!(
                            "Level 2 authored {allocation_label} row {} could not allocate",
                            feature.index
                        ),
                    });
                    continue;
                };
                self.push_dispatch_effect(Level2DispatchEvent::AuthoredGroupActivated {
                    pass,
                    source_slot: producer_slot,
                    group: producer_group,
                    authored_indices: vec![feature.index].into_boxed_slice(),
                });
                debug_assert!(already_seen(self, feature.index));
                debug_assert!(source_slot != 0);
            }
        }
    }

    /// Consume authored model-54 rows when their retained class-11
    /// stage-tag producer is live.  This is the smallest production bridge
    /// for the source `sub_4A1E0` group filter: the class-11 constructor
    /// stores `stageTag` in the live entity id, and the next authored group
    /// uses that id as its exact activation group.  The full switch/objective
    /// state machine remains separate; this hook does not invent a trigger
    /// for a group whose class-11 producer is not live.
    pub(super) fn activate_level2_authored_mana_attraction_fields(&mut self, pass: u8) {
        self.activate_level2_rows_from_live_class11_producers(
            pass,
            "model-54",
            |entity| matches!(entity.model, 0 | 1 | 12 | 32),
            |feature| {
                feature.kind == u16::from(MANA_ATTRACTION_CLASS)
                    && feature.subtype == i16::from(MANA_ATTRACTION_MODEL)
            },
            |session, authored_index| {
                session
                    .level2_mana_attraction_activated
                    .contains(&authored_index)
            },
            |session, authored_index| {
                let source_slot = session.allocate_level2_mana_attraction_field(
                    authored_index,
                    MANA_ATTRACTION_ACTION,
                )?;
                session
                    .level2_mana_attraction_activated
                    .insert(authored_index);
                // The source cursor will see this newly allocated row in the
                // same pass. Keep the local binding explicit so a future
                // extension cannot accidentally discard the source identity.
                debug_assert!(
                    session
                        .economy
                        .mana_attraction_fields()
                        .iter()
                        .any(|field| field.source_entity_index == source_slot)
                );
                Some(source_slot)
            },
        );
    }

    /// Consume authored class-10/model-13 rows when the source class-11
    /// stage-tag producer for group 26 is live. This mirrors the retained
    /// `sub_4A1E0` group filter without pretending to implement the broader
    /// objective/switch state machine.
    pub(super) fn activate_level2_authored_model13(&mut self, pass: u8) {
        self.activate_level2_rows_from_live_class11_producers(
            pass,
            "model13",
            |_| true,
            |feature| feature.kind == 10 && feature.subtype == 13,
            |session, authored_index| session.level2_model13_activated.contains(&authored_index),
            |session, authored_index| {
                let source_slot = session
                    .allocate_level2_authored_model13(authored_index)
                    .map(|source| source.source_entity_index)?;
                session.level2_model13_activated.insert(authored_index);
                debug_assert!(
                    session
                        .economy
                        .authored_model13()
                        .iter()
                        .any(|source| source.source_entity_index == source_slot)
                );
                Some(source_slot)
            },
        );
    }

    /// Consume only authored Day Bee rows (`kind=5/subtype=2`) rooted by a
    /// live class-11 stage-tag producer.  Unrelated kind-5 rows in the same
    /// groups remain latent.
    pub(super) fn activate_level2_authored_bees(&mut self, pass: u8) {
        self.activate_level2_rows_from_live_class11_producers(
            pass,
            "Bee",
            |_| true,
            |feature| {
                matches!(feature.activation_group, 4 | 17 | 21 | 29)
                    && feature.kind == 5
                    && feature.subtype == 2
            },
            |session, authored_index| session.level2_bees_activated.contains(&authored_index),
            |session, authored_index| {
                let feature = session
                    .mission
                    .authored_features()
                    .iter()
                    .copied()
                    .find(|feature| feature.index == authored_index)?;
                let source = session.allocate_level2_authored_bee_feature(feature)?;
                session.level2_bees_activated.insert(authored_index);
                Some(source.source_entity_index)
            },
        );
    }

    /// Consume authored class-10/model-76 rows when the existing live
    /// class-11 producer rows for groups 10/12/14/16 are active.
    pub(super) fn activate_level2_authored_model76(&mut self, pass: u8) {
        self.activate_level2_rows_from_live_class11_producers(
            pass,
            "model76",
            |entity| entity.model == 0 && matches!(entity.source_id, 12 | 14 | 16),
            |feature| feature.kind == 10 && feature.subtype == 76,
            |session, authored_index| session.level2_model76_activated.contains(&authored_index),
            |session, authored_index| {
                let source_slot = session
                    .allocate_level2_authored_model76(authored_index)
                    .map(|source| source.source_entity_index)?;
                session.level2_model76_activated.insert(authored_index);
                debug_assert!(
                    session
                        .economy
                        .authored_model76()
                        .iter()
                        .any(|source| source.source_entity_index == source_slot)
                );
                Some(source_slot)
            },
        );
    }

    /// Consume authored class-14 subtype-3/5 rows when an active class-11
    /// stage-tag producer selects their activation group.  This is the
    /// production counterpart to the rooted `sub_4A1E0` group scan: the
    /// existing class-14 constructor owns the exact model payload and map
    /// linkage, while this hook supplies the missing source-pass reachability.
    pub(super) fn activate_level2_authored_class14(&mut self, pass: u8) {
        self.activate_level2_rows_from_live_class11_producers(
            pass,
            "class14",
            |_| true,
            |feature| feature.kind == 14 && matches!(feature.subtype, 3 | 5),
            |session, authored_index| {
                session.level2_class14_activated.contains(&authored_index)
                    || session
                        .economy
                        .authored_class14()
                        .iter()
                        .any(|source| source.authored_index == authored_index)
            },
            |session, authored_index| {
                let source_slot = session
                    .allocate_level2_authored_class14(authored_index)
                    .map(|source| source.source_entity_index)?;
                session.level2_class14_activated.insert(authored_index);
                debug_assert!(
                    session
                        .economy
                        .authored_class14()
                        .iter()
                        .any(|row| row.source_entity_index == source_slot)
                );
                Some(source_slot)
            },
        );
    }

    /// Consume the authored class-5/model-3/9 rows behind the live
    /// class-10/model-45 row 153.  The pinned group scan reaches all seven
    /// rows in source order; allocation is transactional so a source-pool
    /// failure cannot expose a partial group.
    pub(super) fn activate_level2_authored_class5_group(
        &mut self,
        pass: u8,
        producer_slot: u16,
        target_group: u16,
    ) {
        if target_group != 22 {
            return;
        }
        let rows = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .filter(|feature| {
                feature.activation_group == 22
                    && feature.kind == 5
                    && matches!(feature.subtype, 3 | 9)
                    && !self
                        .session
                        .state
                        .live
                        .level2_class5_activated
                        .contains(&feature.index)
            })
            .collect::<Vec<_>>();
        if rows.is_empty() {
            return;
        }
        let economy_before = self.session.state.live.economy.clone();
        let activated_before = self.session.state.live.level2_class5_activated.clone();
        let mut activated = Vec::with_capacity(rows.len());
        for feature in rows {
            let terrain = &self.session.state.live.terrain;
            let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
            let Some(source) = self.session.state.live.economy.allocate_authored_class5(
                feature.index,
                u8::try_from(feature.subtype).unwrap_or(u8::MAX),
                position,
                terrain,
            ) else {
                self.session.state.live.economy = economy_before;
                self.session.state.live.level2_class5_activated = activated_before;
                self.push_dispatch_effect(Level2DispatchEvent::Error {
                    pass,
                    source_slot: producer_slot,
                    message: format!(
                        "Level 2 authored class5 group {target_group} row {} could not allocate",
                        feature.index
                    ),
                });
                return;
            };
            self.session
                .state
                .live
                .level2_class5_activated
                .insert(feature.index);
            activated.push((feature.index, source.source_entity_index));
        }
        self.push_dispatch_effect(Level2DispatchEvent::AuthoredGroupActivated {
            pass,
            source_slot: producer_slot,
            group: target_group,
            authored_indices: activated
                .into_iter()
                .map(|(index, _)| index)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        });
    }

    pub(super) fn activate_level2_authored_spell_group(
        &mut self,
        pass: u8,
        producer_slot: u16,
        target_group: u16,
    ) {
        if target_group == 22 {
            self.activate_level2_authored_class5_group(pass, producer_slot, target_group);
        }
        let rows = self
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .filter(|feature| {
                feature.activation_group == i16::try_from(target_group).unwrap_or(-1)
                    && feature.kind == 15
                    && matches!(feature.subtype, 6 | 11)
            })
            .collect::<Vec<_>>();
        if rows.len() != 1 {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: producer_slot,
                message: format!(
                    "Level 2 producer group {target_group} has {} rows, expected 1",
                    rows.len()
                ),
            });
            return;
        }
        let feature = rows[0];
        if !matches!((feature.kind, feature.subtype), (15, 6 | 11)) {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot: producer_slot,
                message: format!(
                    "Level 2 producer group {target_group} row {} is not a spell source",
                    feature.index
                ),
            });
            return;
        }
        let economy_before = self.session.state.live.economy.clone();
        let shared_before = self.session.state.live.shared_spells.clone();
        let terrain = &self.session.state.live.terrain;
        let source_slot = match self
            .session
            .state
            .live
            .shared_spells
            .materialize_authored_spell_pickup(
                &mut self.session.state.live.economy,
                feature,
                terrain,
            ) {
            Ok(Some(source_slot)) => source_slot,
            Ok(None) => {
                self.session.state.live.economy = economy_before;
                self.session.state.live.shared_spells = shared_before;
                self.push_dispatch_effect(Level2DispatchEvent::Error {
                    pass,
                    source_slot: producer_slot,
                    message: format!(
                        "Level 2 producer group {target_group} row {} could not allocate",
                        feature.index
                    ),
                });
                return;
            }
            Err(error) => {
                self.session.state.live.economy = economy_before;
                self.session.state.live.shared_spells = shared_before;
                self.push_dispatch_effect(Level2DispatchEvent::Error {
                    pass,
                    source_slot: producer_slot,
                    message: error.to_string(),
                });
                return;
            }
        };
        let companion = if target_group == 27 {
            self.session
                .state
                .live
                .mission
                .authored_features()
                .iter()
                .copied()
                .find(|candidate| {
                    candidate.activation_group == 27
                        && candidate.kind == 10
                        && candidate.subtype == 17
                        && !self
                            .session
                            .state
                            .live
                            .level2_model17_activated
                            .contains(&candidate.index)
                })
        } else {
            None
        };
        if let Some(companion) = companion {
            if self
                .allocate_level2_authored_model17(companion.index)
                .is_none()
            {
                self.session.state.live.economy = economy_before;
                self.session.state.live.shared_spells = shared_before;
                self.push_dispatch_effect(Level2DispatchEvent::Error {
                    pass,
                    source_slot: producer_slot,
                    message: format!(
                        "Level 2 producer group {target_group} model17 row {} could not allocate",
                        companion.index
                    ),
                });
                return;
            }
            self.session
                .state
                .live
                .level2_model17_activated
                .insert(companion.index);
        }
        let mut activated_indices = vec![feature.index];
        if let Some(companion) = companion {
            activated_indices.push(companion.index);
        }
        self.push_dispatch_effect(Level2DispatchEvent::AuthoredGroupActivated {
            pass,
            source_slot: producer_slot,
            group: target_group,
            authored_indices: activated_indices.into_boxed_slice(),
        });
        debug_assert!(
            self.economy
                .spell_pickups()
                .iter()
                .any(|source| source.source_entity_index == source_slot)
        );
        if target_group == 27 {
            debug_assert!(
                self.economy
                    .authored_model17()
                    .iter()
                    .any(|source| source.authored_index == 157)
            );
        }
    }

    fn resolve_level2_source_plan(
        &self,
        source_slot: u16,
        player_entity_index: u16,
    ) -> Option<Level2ResolvedSourcePlan> {
        let live = &self.session.state.live;

        let route_and_selector = if source_slot == player_entity_index {
            live.economy
                .entities
                .get(usize::from(source_slot))
                .filter(|row| row.active && row.index == source_slot)
                .map(|row| (Level2SourceRoute::Player, row.class, row.action))
        } else if let Some(row) = live
            .economy
            .live_arena_cursor_rows()
            .into_iter()
            .find(|row| {
                row.source_slot == source_slot
                    && matches!(
                        row.kind,
                        Level2DynamicArenaKind::SpellPickup
                            | Level2DynamicArenaKind::SpellReplacement
                    )
            })
        {
            // A NewEvent-backed pickup or replacement is already authoritative in the
            // shared dynamic arena. Keep it on that route so the imported-row
            // projection below cannot replace it with the inactive reset row
            // at the same numeric slot. Descriptor-imported pickups have no
            // dynamic arena identity and continue through ImportedSpellSource.
            live.economy
                .economy
                .live_arena_class_action(source_slot, row.kind)
                .map(|(class, action)| (Level2SourceRoute::Dynamic(row), class, action))
        } else if let Some(pickup) = live
            .shared_spells
            .pickups()
            .iter()
            .chain(live.shared_spells.dynamic_pickups())
            .find(|row| {
                row.active && row.index == source_slot && row.class == 15 && row.action == 10
            })
        {
            // Reset-imported family-3 pickups retain the legacy shared row.
            // NewEvent-backed family-3 rows were resolved above through the
            // dynamic arena and therefore use the generic acquisition path.
            Some((Level2SourceRoute::SpeedPickup, pickup.class, pickup.action))
        } else if let Some(pickup) = live
            .shared_spells
            .pickups()
            .iter()
            .chain(live.shared_spells.dynamic_pickups())
            .find(|row| {
                row.active
                    && row.index == source_slot
                    && row.class == 15
                    && matches!(
                        row.model,
                        SHIELD_FAMILY | INVISIBLE_FAMILY | BEYOND_SIGHT_FAMILY
                    )
                    && (physical_spell_action(row.model, PhysicalSpellActionRole::Active)
                        == Some(row.action)
                        || physical_spell_action(
                            row.model,
                            PhysicalSpellActionRole::OrdinaryPickup,
                        ) == Some(row.action))
            })
        {
            Some((
                Level2SourceRoute::ImportedSpellSource,
                pickup.class,
                pickup.action,
            ))
        } else if let Some(holder) = live
            .shared_spells
            .dynamic_pickups()
            .iter()
            .chain(live.shared_spells.holders.iter())
            .find(|holder| {
                holder.active
                    && holder.index == source_slot
                    && holder.class == 15
                    && holder.model == POSSESSION_SPELL_FAMILY
                    && holder.action == POSSESSION_SPELL_FAMILY.saturating_mul(3)
                    && holder.source_spell_parent_entity_index() == player_entity_index
            })
        {
            Some((
                Level2SourceRoute::PossessionHolder,
                holder.class,
                holder.action,
            ))
        } else if let Some(holder) = live
            .shared_spells
            .dynamic_pickups()
            .iter()
            .chain(live.shared_spells.holders.iter())
            .find(|holder| {
                holder.active
                    && holder.index == source_slot
                    && holder.class == 15
                    && holder.model == FIREBALL_SPELL_FAMILY
                    && holder.action == 0
                    && holder.source_spell_parent_entity_index() == player_entity_index
            })
        {
            Some((
                Level2SourceRoute::FireballHolder,
                holder.class,
                holder.action,
            ))
        } else if let Some(holder) = live.economy.entities.iter().find(|holder| {
            holder.active
                && holder.index == source_slot
                && holder.index == LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX
                && holder.class == 15
                && holder.model == POSSESSION_SPELL_FAMILY
                && holder.action == POSSESSION_SPELL_FAMILY.saturating_mul(3)
                && holder.source_spell_parent_entity_index() == LEVEL2_NYPHUR_ENTITY_INDEX
                && holder.active_value > 0
        }) {
            Some((
                Level2SourceRoute::PossessionHolder,
                holder.class,
                holder.action,
            ))
        } else if let Some(holder) = live.economy.entities.iter().find(|holder| {
            holder.active
                && holder.index == source_slot
                && holder.index == LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX
                && holder.class == 15
                && holder.model == FIREBALL_SPELL_FAMILY
                && holder.action == 0
                && holder.source_spell_parent_entity_index() == LEVEL2_NYPHUR_ENTITY_INDEX
                && holder.active_value > 0
        }) {
            Some((
                Level2SourceRoute::FireballHolder,
                holder.class,
                holder.action,
            ))
        } else if let Some(holder) = live
            .shared_spells
            .dynamic_pickups()
            .iter()
            .chain(live.shared_spells.holders.iter())
            .find(|holder| {
                holder.active
                    && holder.index == source_slot
                    && holder.class == 15
                    && holder.model == CASTLE_SPELL_FAMILY
                    && holder.action == CASTLE_SPELL_FAMILY.saturating_mul(3)
            })
        {
            Some((Level2SourceRoute::CastleHolder, holder.class, holder.action))
        } else if let Some(stone) = live
            .bad_stones
            .iter()
            .find(|stone| stone.entity.active && stone.entity.index == source_slot)
        {
            Some((
                Level2SourceRoute::BadStone,
                stone.entity.class,
                stone.entity.action,
            ))
        } else if let Some(exit_switch) = live.economy.entities.iter().find(|entity| {
            entity.active && entity.index == source_slot && entity.class == 11 && entity.model == 4
        }) {
            Some((
                Level2SourceRoute::ExitSwitch,
                exit_switch.class,
                exit_switch.action,
            ))
        } else if let Some(house) = live
            .ordinary
            .houses()
            .iter()
            .find(|house| house.active && house.entity_index == source_slot)
        {
            Some((Level2SourceRoute::House, 10, house.action))
        } else if let Some(villager) = live
            .ordinary
            .villagers()
            .iter()
            .find(|villager| villager.active && villager.entity_index == source_slot)
        {
            Some((Level2SourceRoute::Villager, 5, villager.action))
        } else if let Some(dolmen) = live
            .ordinary
            .dolmens()
            .iter()
            .find(|dolmen| dolmen.entity.active && dolmen.entity.index == source_slot)
        {
            Some((
                Level2SourceRoute::Dolmen,
                dolmen.entity.class,
                dolmen.entity.action,
            ))
        } else if let Some(castle) = live
            .economy
            .castle()
            .filter(|castle| castle.active && castle.source_entity_index == source_slot)
        {
            Some((
                Level2SourceRoute::CastleRoot,
                castle.source_class,
                castle.action,
            ))
        } else if let Some(goat) = live
            .goats
            .goats()
            .iter()
            .find(|goat| goat.active && goat.entity_index == source_slot)
        {
            Some((Level2SourceRoute::Goat, 5, goat.action))
        } else if let Some(nyphur) = live.economy.entities.iter().find(|entity| {
            entity.active && entity.index == source_slot && entity.class == 3 && entity.model == 1
        }) {
            Some((Level2SourceRoute::Nyphur, nyphur.class, nyphur.action))
        } else if let Some(row) = live
            .economy
            .live_arena_cursor_rows()
            .into_iter()
            .find(|row| row.source_slot == source_slot)
        {
            live.economy
                .economy
                .live_arena_class_action(source_slot, row.kind)
                .map(|(class, action)| (Level2SourceRoute::Dynamic(row), class, action))
        } else if let Some(entity) =
            live.economy
                .entities
                .get(usize::from(source_slot))
                .filter(|entity| {
                    entity.active
                        && matches!(
                            (entity.class, entity.action),
                            (2, 0 | 3 | 21)
                                | (5, 31 | 72 | 74 | 76 | 77 | 79 | 232)
                                | (10, 68 | 69)
                                | (11, 0 | 1 | 16 | 32)
                                | (14, 6)
                        )
                })
        {
            Some((Level2SourceRoute::Imported, entity.class, entity.action))
        } else {
            None
        };

        let Some((route, class, action)) = route_and_selector else {
            return None;
        };
        let key = level2_update_key(class, action);
        let phase = self.level2_source_phase(source_slot, route);
        if matches!(
            route,
            Level2SourceRoute::Dynamic(Level2DynamicArenaCursorRow {
                kind: Level2DynamicArenaKind::AuthoredClass5ArcherChild,
                ..
            })
        ) && class == level2_class5::archer::ARCHER_CHILD_SOURCE_CLASS
            && action == level2_class5::archer::ARCHER_ACTION_KILL
        {
            return Some(Level2ResolvedSourcePlan::InvalidSelectorHide { key, route, phase });
        }
        if class == 10 && matches!(action, 82 | 84) {
            return Some(Level2ResolvedSourcePlan::NoCallback { key, route, phase });
        }
        let Some(entry) = CAMPAIGN_BEHAVIOR_REGISTRY.entry(&key) else {
            return Some(Level2ResolvedSourcePlan::Unknown {
                key,
                route,
                phase,
                message: format!(
                    "unregistered UpdateEntities selector {class}:{action} at source slot {source_slot}"
                ),
            });
        };
        Some(Level2ResolvedSourcePlan::Callback(
            Level2ResolvedSourceVisit {
                key,
                handler: entry.handler,
                route,
                phase,
            },
        ))
    }

    fn level2_source_phase(&self, source_slot: u16, route: Level2SourceRoute) -> Option<u8> {
        let live = &self.session.state.live;
        match route {
            Level2SourceRoute::Player
            | Level2SourceRoute::Nyphur
            | Level2SourceRoute::ExitSwitch => live
                .economy
                .entities
                .iter()
                .find(|row| row.active && row.index == source_slot)
                .map(|row| row.phase),
            Level2SourceRoute::SpeedPickup
            | Level2SourceRoute::ImportedSpellSource
            | Level2SourceRoute::CastleHolder => live
                .shared_spells
                .pickups()
                .iter()
                .chain(live.shared_spells.holders.iter())
                .chain(live.shared_spells.dynamic_pickups())
                .find(|row| row.active && row.index == source_slot)
                .map(|row| row.phase)
                .or_else(|| {
                    live.economy
                        .entities
                        .iter()
                        .find(|row| row.active && row.index == source_slot)
                        .map(|row| row.phase)
                }),
            Level2SourceRoute::PossessionHolder | Level2SourceRoute::FireballHolder => {
                if source_slot == LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX
                    || source_slot == LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX
                {
                    live.economy
                        .entities
                        .iter()
                        .find(|row| row.active && row.index == source_slot)
                        .map(|row| row.phase)
                } else {
                    live.shared_spells
                        .pickup(source_slot)
                        .filter(|row| row.active)
                        .map(|row| row.phase)
                }
            }
            Level2SourceRoute::BadStone => live
                .bad_stones
                .iter()
                .find(|row| row.entity.active && row.entity.index == source_slot)
                .map(|row| row.entity.phase),
            Level2SourceRoute::House => live
                .ordinary
                .houses()
                .iter()
                .find(|row| row.active && row.entity_index == source_slot)
                .map(|row| row.phase),
            Level2SourceRoute::Villager => live
                .ordinary
                .villagers()
                .iter()
                .find(|row| row.active && row.entity_index == source_slot)
                .map(|row| row.phase),
            Level2SourceRoute::Dolmen => live
                .ordinary
                .dolmens()
                .iter()
                .find(|row| row.entity.active && row.entity.index == source_slot)
                .map(|row| row.entity.phase),
            Level2SourceRoute::CastleRoot => live
                .economy
                .castle()
                .filter(|row| row.active && row.source_entity_index == source_slot)
                .map(|row| row.phase),
            Level2SourceRoute::Goat => live
                .goats
                .goats()
                .iter()
                .find(|row| row.active && row.entity_index == source_slot)
                .map(|row| row.phase),
            Level2SourceRoute::Dynamic(cursor) => {
                let economy = &live.economy.economy;
                macro_rules! row_phase {
                    ($rows:expr, $field:ident) => {
                        $rows
                            .iter()
                            .find(|row| row.active && row.source_entity_index == source_slot)
                            .map(|row| row.$field)
                    };
                }
                match cursor.kind {
                    Level2DynamicArenaKind::CastleProjectile
                    | Level2DynamicArenaKind::ManaCarrier
                    | Level2DynamicArenaKind::Model6SpawnedChild
                    | Level2DynamicArenaKind::BeyondSightPulseEffect
                    | Level2DynamicArenaKind::ManaAttraction => {
                        economy.source_phase_sidecar(source_slot)
                    }
                    Level2DynamicArenaKind::FireballProjectile => {
                        row_phase!(economy.fireball_projectiles, phase)
                    }
                    Level2DynamicArenaKind::PossessionProjectile => {
                        row_phase!(economy.possession_projectiles, phase)
                    }
                    Level2DynamicArenaKind::PossessionEffect => {
                        row_phase!(economy.possession_effects, phase)
                    }
                    Level2DynamicArenaKind::CastleRoot => economy
                        .castle
                        .filter(|row| row.active && row.source_entity_index == source_slot)
                        .map(|row| row.phase),
                    Level2DynamicArenaKind::CastleChild
                    | Level2DynamicArenaKind::CastleAuxiliary
                    | Level2DynamicArenaKind::CastleDefender
                    | Level2DynamicArenaKind::CastleController => {
                        row_phase!(economy.castle_children, source_byte_3e)
                    }
                    Level2DynamicArenaKind::ManaSphere => row_phase!(economy.spheres, phase),
                    Level2DynamicArenaKind::DeathBurst => {
                        row_phase!(economy.death_bursts, source_byte_3e)
                    }
                    Level2DynamicArenaKind::PlayerDeathMarker => {
                        row_phase!(economy.player_death_markers, dispatch_phase)
                    }
                    Level2DynamicArenaKind::SpellReplacement => {
                        row_phase!(economy.spell_replacements, phase)
                    }
                    Level2DynamicArenaKind::SpellPickup => {
                        row_phase!(economy.spell_pickups, phase)
                    }
                    Level2DynamicArenaKind::SpeedTrail => {
                        row_phase!(economy.speed_trails, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel1 => {
                        row_phase!(economy.authored_model1, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel45 => {
                        row_phase!(economy.authored_model45, source_byte_3e)
                    }
                    Level2DynamicArenaKind::AuthoredModel13 => {
                        row_phase!(economy.authored_model13, phase)
                    }
                    Level2DynamicArenaKind::AuthoredBee => {
                        row_phase!(economy.authored_bees, phase)
                    }
                    Level2DynamicArenaKind::Model13SpawnedChild => {
                        row_phase!(economy.model13_spawned_children, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel6 => {
                        row_phase!(economy.authored_model6, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel17 => {
                        row_phase!(economy.authored_model17, phase)
                    }
                    Level2DynamicArenaKind::Model17SpawnedChild => {
                        row_phase!(economy.model17_spawned_children, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel22 => {
                        row_phase!(economy.authored_model22, phase)
                    }
                    Level2DynamicArenaKind::Model22SpawnedChild => {
                        row_phase!(economy.model22_spawned_children, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel76 => {
                        row_phase!(economy.authored_model76, phase)
                    }
                    Level2DynamicArenaKind::Model76Child => {
                        row_phase!(economy.model76_children, phase)
                    }
                    Level2DynamicArenaKind::Model76TailSpawn => {
                        row_phase!(economy.model76_tail_spawns, phase)
                    }
                    Level2DynamicArenaKind::AuthoredClass14 => {
                        row_phase!(economy.authored_class14, phase)
                    }
                    Level2DynamicArenaKind::AuthoredClass5 => {
                        row_phase!(economy.authored_class5, source_byte_3e)
                    }
                    Level2DynamicArenaKind::AuthoredClass5ArcherChild => {
                        row_phase!(economy.authored_class5_archer_children, source_byte_3e)
                    }
                    Level2DynamicArenaKind::AuthoredClass5Model3Child => {
                        row_phase!(economy.authored_class5_model3_children, source_byte_3e)
                    }
                    Level2DynamicArenaKind::HouseResident => {
                        row_phase!(economy.house_residents, phase)
                    }
                    Level2DynamicArenaKind::AuthoredSwitchProducer => {
                        row_phase!(economy.authored_switch_producers, phase)
                    }
                    Level2DynamicArenaKind::AuthoredTree => {
                        row_phase!(economy.authored_trees, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel32 => {
                        row_phase!(economy.authored_model32, source_byte_3e)
                    }
                    Level2DynamicArenaKind::AuthoredModel59 => {
                        row_phase!(economy.authored_model59, phase)
                    }
                    Level2DynamicArenaKind::Model59SmokeChild => {
                        row_phase!(economy.model59_smoke_children, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel53 => {
                        row_phase!(economy.authored_model53, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel5WaterSplash => {
                        row_phase!(economy.authored_model5_water_splashes, phase)
                    }
                }
            }
            Level2SourceRoute::Imported => live
                .economy
                .entities
                .get(usize::from(source_slot))
                .filter(|row| row.active && row.index == source_slot)
                .map(|row| row.phase),
        }
    }

    /// Read byte 0x3E from a retained tombstone after a callback disables its
    /// row. `UpdateEntities_57730` advances that same numeric row before the
    /// following pass reclaims it, so active-only selector resolution is too
    /// narrow for epilogue verification.
    fn level2_retained_source_phase(
        &self,
        source_slot: u16,
        route: Level2SourceRoute,
    ) -> Option<u8> {
        let live = &self.session.state.live;
        match route {
            Level2SourceRoute::SpeedPickup
            | Level2SourceRoute::ImportedSpellSource
            | Level2SourceRoute::PossessionHolder
            | Level2SourceRoute::CastleHolder
            | Level2SourceRoute::FireballHolder => live
                .shared_spells
                .pickups()
                .iter()
                .chain(live.shared_spells.holders.iter())
                .chain(live.shared_spells.dynamic_pickups())
                .find(|row| row.index == source_slot)
                .map(|row| row.phase)
                .or_else(|| {
                    live.economy
                        .entities
                        .iter()
                        .find(|row| row.index == source_slot)
                        .map(|row| row.phase)
                }),
            Level2SourceRoute::BadStone => live
                .bad_stones
                .iter()
                .find(|row| row.entity.index == source_slot)
                .map(|row| row.entity.phase),
            Level2SourceRoute::House => live
                .ordinary
                .houses()
                .iter()
                .find(|row| row.entity_index == source_slot)
                .map(|row| row.phase),
            Level2SourceRoute::Villager => live
                .ordinary
                .villagers()
                .iter()
                .find(|row| row.entity_index == source_slot)
                .map(|row| row.phase),
            Level2SourceRoute::Goat => live
                .goats
                .goats()
                .iter()
                .find(|row| row.entity_index == source_slot)
                .map(|row| row.phase),
            Level2SourceRoute::Player
            | Level2SourceRoute::Nyphur
            | Level2SourceRoute::ExitSwitch => live
                .economy
                .entities
                .iter()
                .find(|row| row.index == source_slot)
                .map(|row| row.phase),
            Level2SourceRoute::Dolmen => live
                .ordinary
                .dolmens()
                .iter()
                .find(|row| row.entity.index == source_slot)
                .map(|row| row.entity.phase),
            Level2SourceRoute::CastleRoot => live
                .economy
                .economy
                .castle
                .filter(|row| row.source_entity_index == source_slot)
                .map(|row| row.phase),
            Level2SourceRoute::Dynamic(cursor) => {
                let economy = &live.economy.economy;
                macro_rules! retained_phase {
                    ($rows:expr, $field:ident) => {
                        $rows
                            .iter()
                            .find(|row| row.source_entity_index == source_slot)
                            .map(|row| row.$field)
                    };
                }
                match cursor.kind {
                    Level2DynamicArenaKind::CastleProjectile
                    | Level2DynamicArenaKind::ManaCarrier
                    | Level2DynamicArenaKind::Model6SpawnedChild
                    | Level2DynamicArenaKind::BeyondSightPulseEffect
                    | Level2DynamicArenaKind::ManaAttraction => {
                        economy.source_phase_sidecar(source_slot)
                    }
                    Level2DynamicArenaKind::FireballProjectile => {
                        retained_phase!(economy.fireball_projectiles, phase)
                    }
                    Level2DynamicArenaKind::PossessionProjectile => {
                        retained_phase!(economy.possession_projectiles, phase)
                    }
                    Level2DynamicArenaKind::PossessionEffect => {
                        retained_phase!(economy.possession_effects, phase)
                    }
                    Level2DynamicArenaKind::CastleRoot => economy
                        .castle
                        .filter(|row| row.source_entity_index == source_slot)
                        .map(|row| row.phase),
                    Level2DynamicArenaKind::CastleChild
                    | Level2DynamicArenaKind::CastleAuxiliary
                    | Level2DynamicArenaKind::CastleDefender
                    | Level2DynamicArenaKind::CastleController => {
                        retained_phase!(economy.castle_children, source_byte_3e)
                    }
                    Level2DynamicArenaKind::ManaSphere => {
                        retained_phase!(economy.spheres, phase)
                    }
                    Level2DynamicArenaKind::DeathBurst => {
                        retained_phase!(economy.death_bursts, source_byte_3e)
                    }
                    Level2DynamicArenaKind::PlayerDeathMarker => {
                        retained_phase!(economy.player_death_markers, dispatch_phase)
                    }
                    Level2DynamicArenaKind::SpellReplacement => {
                        retained_phase!(economy.spell_replacements, phase)
                    }
                    Level2DynamicArenaKind::SpellPickup => {
                        retained_phase!(economy.spell_pickups, phase)
                    }
                    Level2DynamicArenaKind::SpeedTrail => {
                        retained_phase!(economy.speed_trails, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel1 => {
                        retained_phase!(economy.authored_model1, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel45 => {
                        retained_phase!(economy.authored_model45, source_byte_3e)
                    }
                    Level2DynamicArenaKind::AuthoredModel13 => {
                        retained_phase!(economy.authored_model13, phase)
                    }
                    Level2DynamicArenaKind::AuthoredBee => {
                        retained_phase!(economy.authored_bees, phase)
                    }
                    Level2DynamicArenaKind::Model13SpawnedChild => {
                        retained_phase!(economy.model13_spawned_children, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel6 => {
                        retained_phase!(economy.authored_model6, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel17 => {
                        retained_phase!(economy.authored_model17, phase)
                    }
                    Level2DynamicArenaKind::Model17SpawnedChild => {
                        retained_phase!(economy.model17_spawned_children, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel22 => {
                        retained_phase!(economy.authored_model22, phase)
                    }
                    Level2DynamicArenaKind::Model22SpawnedChild => {
                        retained_phase!(economy.model22_spawned_children, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel76 => {
                        retained_phase!(economy.authored_model76, phase)
                    }
                    Level2DynamicArenaKind::Model76Child => {
                        retained_phase!(economy.model76_children, phase)
                    }
                    Level2DynamicArenaKind::Model76TailSpawn => {
                        retained_phase!(economy.model76_tail_spawns, phase)
                    }
                    Level2DynamicArenaKind::AuthoredClass14 => {
                        retained_phase!(economy.authored_class14, phase)
                    }
                    Level2DynamicArenaKind::AuthoredClass5 => {
                        retained_phase!(economy.authored_class5, source_byte_3e)
                    }
                    Level2DynamicArenaKind::AuthoredClass5ArcherChild => {
                        retained_phase!(economy.authored_class5_archer_children, source_byte_3e)
                    }
                    Level2DynamicArenaKind::AuthoredClass5Model3Child => {
                        retained_phase!(economy.authored_class5_model3_children, source_byte_3e)
                    }
                    Level2DynamicArenaKind::HouseResident => {
                        retained_phase!(economy.house_residents, phase)
                    }
                    Level2DynamicArenaKind::AuthoredSwitchProducer => {
                        retained_phase!(economy.authored_switch_producers, phase)
                    }
                    Level2DynamicArenaKind::AuthoredTree => {
                        retained_phase!(economy.authored_trees, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel32 => {
                        retained_phase!(economy.authored_model32, source_byte_3e)
                    }
                    Level2DynamicArenaKind::AuthoredModel59 => {
                        retained_phase!(economy.authored_model59, phase)
                    }
                    Level2DynamicArenaKind::Model59SmokeChild => {
                        retained_phase!(economy.model59_smoke_children, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel53 => {
                        retained_phase!(economy.authored_model53, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel5WaterSplash => {
                        retained_phase!(economy.authored_model5_water_splashes, phase)
                    }
                }
            }
            Level2SourceRoute::Imported => live
                .economy
                .entities
                .get(usize::from(source_slot))
                .filter(|row| row.index == source_slot)
                .map(|row| row.phase),
        }
    }

    fn advance_level2_outer_owned_phase(
        &mut self,
        source_slot: u16,
        route: Level2SourceRoute,
    ) -> std::result::Result<(), String> {
        let live = &mut self.session.state.live;
        let phase = match route {
            Level2SourceRoute::SpeedPickup | Level2SourceRoute::ImportedSpellSource => live
                .shared_spells
                .pickups
                .iter_mut()
                .chain(live.shared_spells.dynamic_pickups.iter_mut())
                .find(|row| row.index == source_slot)
                .map(|row| &mut row.phase),
            Level2SourceRoute::Nyphur
            | Level2SourceRoute::Player
            | Level2SourceRoute::ExitSwitch => live
                .economy
                .entities
                .iter_mut()
                .find(|row| row.index == source_slot)
                .map(|row| &mut row.phase),
            Level2SourceRoute::House => {
                return live
                    .ordinary
                    .advance_house_source_phase(source_slot)
                    .map(|_| ())
                    .ok_or_else(|| {
                        format!(
                            "scheduler cannot resolve House source slot {source_slot} for its common phase epilogue"
                        )
                    });
            }
            Level2SourceRoute::Dolmen => live
                .ordinary
                .dolmens
                .iter_mut()
                .find(|row| row.entity.index == source_slot)
                .map(|row| &mut row.entity.phase),
            Level2SourceRoute::CastleRoot => live
                .economy
                .economy
                .castle
                .as_mut()
                .filter(|row| row.source_entity_index == source_slot)
                .map(|row| &mut row.phase),
            Level2SourceRoute::PossessionHolder | Level2SourceRoute::FireballHolder => {
                if source_slot == LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX
                    || source_slot == LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX
                {
                    live.economy
                        .entities
                        .iter_mut()
                        .find(|row| row.index == source_slot)
                        .map(|row| &mut row.phase)
                } else {
                    live.shared_spells
                        .pickup_mut(source_slot)
                        .map(|row| &mut row.phase)
                }
            }
            Level2SourceRoute::CastleHolder => live
                .shared_spells
                .pickup_mut(source_slot)
                .map(|row| &mut row.phase),
            Level2SourceRoute::Dynamic(row) => {
                macro_rules! dynamic_phase_mut {
                    ($rows:expr, $field:ident) => {
                        $rows
                            .iter_mut()
                            .find(|row| row.source_entity_index == source_slot)
                            .map(|row| &mut row.$field)
                    };
                }
                match row.kind {
                    Level2DynamicArenaKind::CastleProjectile
                    | Level2DynamicArenaKind::ManaCarrier
                    | Level2DynamicArenaKind::Model6SpawnedChild
                    | Level2DynamicArenaKind::BeyondSightPulseEffect
                    | Level2DynamicArenaKind::ManaAttraction => {
                        return live
                            .economy
                            .economy
                            .advance_source_phase_sidecar(source_slot)
                            .map(|_| ())
                            .ok_or_else(|| {
                                format!(
                                    "scheduler cannot resolve the source byte-0x3E sidecar for {:?} at source slot {source_slot}",
                                    row.kind
                                )
                            });
                    }
                    Level2DynamicArenaKind::FireballProjectile => {
                        dynamic_phase_mut!(live.economy.economy.fireball_projectiles, phase)
                    }
                    Level2DynamicArenaKind::PossessionProjectile => {
                        dynamic_phase_mut!(live.economy.economy.possession_projectiles, phase)
                    }
                    Level2DynamicArenaKind::PossessionEffect => {
                        dynamic_phase_mut!(live.economy.economy.possession_effects, phase)
                    }
                    Level2DynamicArenaKind::CastleRoot => live
                        .economy
                        .economy
                        .castle
                        .as_mut()
                        .filter(|row| row.source_entity_index == source_slot)
                        .map(|row| &mut row.phase),
                    Level2DynamicArenaKind::CastleChild
                    | Level2DynamicArenaKind::CastleAuxiliary
                    | Level2DynamicArenaKind::CastleDefender
                    | Level2DynamicArenaKind::CastleController => dynamic_phase_mut!(
                        live.economy.economy.castle_children,
                        source_byte_3e
                    ),
                    Level2DynamicArenaKind::ManaSphere => {
                        dynamic_phase_mut!(live.economy.economy.spheres, phase)
                    }
                    Level2DynamicArenaKind::DeathBurst => dynamic_phase_mut!(
                        live.economy.economy.death_bursts,
                        source_byte_3e
                    ),
                    Level2DynamicArenaKind::PlayerDeathMarker => dynamic_phase_mut!(
                        live.economy.economy.player_death_markers,
                        dispatch_phase
                    ),
                    Level2DynamicArenaKind::SpellReplacement => {
                        dynamic_phase_mut!(live.economy.economy.spell_replacements, phase)
                    }
                    Level2DynamicArenaKind::SpellPickup => {
                        dynamic_phase_mut!(live.economy.economy.spell_pickups, phase)
                    }
                    Level2DynamicArenaKind::SpeedTrail => {
                        dynamic_phase_mut!(live.economy.economy.speed_trails, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel1 => {
                        dynamic_phase_mut!(live.economy.economy.authored_model1, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel45 => dynamic_phase_mut!(
                        live.economy.economy.authored_model45,
                        source_byte_3e
                    ),
                    Level2DynamicArenaKind::AuthoredModel13 => {
                        dynamic_phase_mut!(live.economy.economy.authored_model13, phase)
                    }
                    Level2DynamicArenaKind::AuthoredBee => {
                        dynamic_phase_mut!(live.economy.economy.authored_bees, phase)
                    }
                    Level2DynamicArenaKind::Model13SpawnedChild => dynamic_phase_mut!(
                        live.economy.economy.model13_spawned_children,
                        phase
                    ),
                    Level2DynamicArenaKind::AuthoredModel6 => {
                        dynamic_phase_mut!(live.economy.economy.authored_model6, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel17 => {
                        dynamic_phase_mut!(live.economy.economy.authored_model17, phase)
                    }
                    Level2DynamicArenaKind::Model17SpawnedChild => dynamic_phase_mut!(
                        live.economy.economy.model17_spawned_children,
                        phase
                    ),
                    Level2DynamicArenaKind::AuthoredModel22 => {
                        dynamic_phase_mut!(live.economy.economy.authored_model22, phase)
                    }
                    Level2DynamicArenaKind::Model22SpawnedChild => dynamic_phase_mut!(
                        live.economy.economy.model22_spawned_children,
                        phase
                    ),
                    Level2DynamicArenaKind::AuthoredModel76 => {
                        dynamic_phase_mut!(live.economy.economy.authored_model76, phase)
                    }
                    Level2DynamicArenaKind::Model76Child => {
                        dynamic_phase_mut!(live.economy.economy.model76_children, phase)
                    }
                    Level2DynamicArenaKind::Model76TailSpawn => dynamic_phase_mut!(
                        live.economy.economy.model76_tail_spawns,
                        phase
                    ),
                    Level2DynamicArenaKind::AuthoredClass14 => {
                        dynamic_phase_mut!(live.economy.economy.authored_class14, phase)
                    }
                    Level2DynamicArenaKind::AuthoredClass5 => dynamic_phase_mut!(
                        live.economy.economy.authored_class5,
                        source_byte_3e
                    ),
                    Level2DynamicArenaKind::AuthoredClass5ArcherChild => dynamic_phase_mut!(
                        live.economy.economy.authored_class5_archer_children,
                        source_byte_3e
                    ),
                    Level2DynamicArenaKind::AuthoredClass5Model3Child => dynamic_phase_mut!(
                        live.economy.economy.authored_class5_model3_children,
                        source_byte_3e
                    ),
                    Level2DynamicArenaKind::HouseResident => {
                        dynamic_phase_mut!(live.economy.economy.house_residents, phase)
                    }
                    Level2DynamicArenaKind::AuthoredSwitchProducer => dynamic_phase_mut!(
                        live.economy.economy.authored_switch_producers,
                        phase
                    ),
                    Level2DynamicArenaKind::AuthoredTree => {
                        dynamic_phase_mut!(live.economy.economy.authored_trees, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel32 => {
                        dynamic_phase_mut!(
                            live.economy.economy.authored_model32,
                            source_byte_3e
                        )
                    }
                    Level2DynamicArenaKind::AuthoredModel59 => {
                        dynamic_phase_mut!(live.economy.economy.authored_model59, phase)
                    }
                    Level2DynamicArenaKind::Model59SmokeChild => dynamic_phase_mut!(
                        live.economy.economy.model59_smoke_children,
                        phase
                    ),
                    Level2DynamicArenaKind::AuthoredModel53 => {
                        dynamic_phase_mut!(live.economy.economy.authored_model53, phase)
                    }
                    Level2DynamicArenaKind::AuthoredModel5WaterSplash => dynamic_phase_mut!(
                        live.economy.economy.authored_model5_water_splashes,
                        phase
                    ),
                }
            }
            Level2SourceRoute::Imported => live
                .economy
                .entities
                .get_mut(usize::from(source_slot))
                .filter(|row| row.active && row.index == source_slot)
                .map(|row| &mut row.phase),
            _ => None,
        }
        .ok_or_else(|| {
            format!(
                "scheduler cannot resolve the current phase carrier for {route:?} at source slot {source_slot}"
            )
        })?;
        *phase = phase.wrapping_add(1);
        let advanced = *phase;
        match route {
            Level2SourceRoute::SpeedPickup | Level2SourceRoute::ImportedSpellSource => {
                if let Some(row) = live
                    .shared_spells
                    .pickups
                    .iter_mut()
                    .chain(live.shared_spells.dynamic_pickups.iter_mut())
                    .find(|row| row.index == source_slot)
                {
                    row.record[66] = advanced;
                }
            }
            Level2SourceRoute::Nyphur
            | Level2SourceRoute::Player
            | Level2SourceRoute::ExitSwitch
            | Level2SourceRoute::Imported => {
                if let Some(row) = live
                    .economy
                    .entities
                    .iter_mut()
                    .find(|row| row.index == source_slot)
                {
                    row.record[66] = advanced;
                }
            }
            Level2SourceRoute::Dolmen => {
                if let Some(row) = live
                    .ordinary
                    .dolmens
                    .iter_mut()
                    .find(|row| row.entity.index == source_slot)
                {
                    row.entity.record[66] = advanced;
                }
            }
            Level2SourceRoute::PossessionHolder | Level2SourceRoute::FireballHolder => {
                if source_slot == LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX
                    || source_slot == LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX
                {
                    if let Some(row) = live
                        .economy
                        .entities
                        .iter_mut()
                        .find(|row| row.index == source_slot)
                    {
                        row.record[66] = advanced;
                    }
                } else if let Some(row) = live.shared_spells.pickup_mut(source_slot) {
                    row.record[66] = advanced;
                }
            }
            Level2SourceRoute::CastleHolder => {
                if let Some(row) = live.shared_spells.pickup_mut(source_slot) {
                    row.record[66] = advanced;
                }
            }
            _ => {}
        }
        if route == Level2SourceRoute::Imported
            && let Some(entity) = live
                .economy
                .entities
                .get(usize::from(source_slot))
                .filter(|entity| entity.index == source_slot)
            && let Some(root) = live
                .level2_imported_switch_roots
                .iter_mut()
                .find(|root| root.source_slot == source_slot)
        {
            // Keep the legacy inspection view isomorphic with the one raw
            // source-row authority after the scheduler-owned epilogue.
            root.phase = advanced;
            root.model = entity.model;
            root.target_group = entity.source_id;
            root.word_10 = i16::from_le_bytes([entity.record[20], entity.record[21]]);
            root.position = entity.position;
            root.drawing_enabled = !entity.drawing_disabled();
            root.active = entity.active;
        }
        if route == Level2SourceRoute::Imported
            && live
                .level2_group10_switch25
                .is_some_and(|switch| switch.source_slot == source_slot)
            && let Some((position, drawing_disabled)) = live
                .economy
                .entities
                .get(usize::from(source_slot))
                .filter(|entity| entity.index == source_slot)
                .map(|entity| (entity.position, entity.drawing_disabled()))
        {
            // Slot 25 is an imported source row. Keep the retained test/UI
            // projection output-only until it can be removed completely.
            // A consumed switch has no live callback projection even though
            // its hidden raw row remains reserved until the next prepass.
            if drawing_disabled {
                live.level2_group10_switch25 = None;
            } else if let Some(switch) = live.level2_group10_switch25.as_mut() {
                switch.phase = advanced;
                switch.position = position;
            }
        }
        let sync_imported_holder = matches!(
            route,
            Level2SourceRoute::PossessionHolder
                | Level2SourceRoute::CastleHolder
                | Level2SourceRoute::FireballHolder
        ) && source_slot != LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX
            && source_slot != LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX
            && live
                .economy
                .entities
                .get(usize::from(source_slot))
                .is_some_and(|row| row.index == source_slot && row.class == 15);
        if sync_imported_holder {
            self.commit_imported_spell_source_projection(source_slot)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn level2_route_accepts_handler(route: Level2SourceRoute, handler: ExistingHandler) -> bool {
        match route {
            Level2SourceRoute::SpeedPickup
            | Level2SourceRoute::ImportedSpellSource
            | Level2SourceRoute::PossessionHolder
            | Level2SourceRoute::CastleHolder
            | Level2SourceRoute::FireballHolder => handler == ExistingHandler::SpellHolderAndPickup,
            Level2SourceRoute::BadStone => handler == ExistingHandler::BadStone,
            Level2SourceRoute::House => handler == ExistingHandler::Class10Effects,
            Level2SourceRoute::Villager => matches!(
                handler,
                ExistingHandler::BuilderFamily
                    | ExistingHandler::VillagerFamily
                    | ExistingHandler::BuilderVillagerPostBuild
                    | ExistingHandler::TraderFamily
            ),
            Level2SourceRoute::Dolmen => handler == ExistingHandler::Dolmen,
            Level2SourceRoute::ExitSwitch => handler == ExistingHandler::SwitchProducers,
            Level2SourceRoute::CastleRoot => handler == ExistingHandler::CastleRoot,
            Level2SourceRoute::Goat => handler == ExistingHandler::GoatFamily,
            Level2SourceRoute::Player => handler == ExistingHandler::PlayerFlight,
            Level2SourceRoute::Nyphur => handler == ExistingHandler::NyphurPlayer,
            Level2SourceRoute::Imported => matches!(
                handler,
                ExistingHandler::BallisticScenery
                    | ExistingHandler::CountdownSwitch
                    | ExistingHandler::TerrainStripController
                    | ExistingHandler::SwitchProducers
                    | ExistingHandler::ObjectiveSwitch
                    | ExistingHandler::TreeLifecycle
                    | ExistingHandler::Stone
                    | ExistingHandler::Model9Family
                    | ExistingHandler::MultipartFamily
            ),
            Level2SourceRoute::Dynamic(row) => match row.kind {
                Level2DynamicArenaKind::CastleProjectile
                | Level2DynamicArenaKind::FireballProjectile
                | Level2DynamicArenaKind::PossessionProjectile
                | Level2DynamicArenaKind::AuthoredClass5ArcherChild => {
                    handler == ExistingHandler::ProjectileFamily
                }
                Level2DynamicArenaKind::PossessionEffect
                | Level2DynamicArenaKind::CastleChild
                | Level2DynamicArenaKind::CastleController
                | Level2DynamicArenaKind::ManaSphere
                | Level2DynamicArenaKind::DeathBurst
                | Level2DynamicArenaKind::PlayerDeathMarker
                | Level2DynamicArenaKind::AuthoredModel1
                | Level2DynamicArenaKind::AuthoredModel45
                | Level2DynamicArenaKind::AuthoredModel13
                | Level2DynamicArenaKind::Model13SpawnedChild
                | Level2DynamicArenaKind::AuthoredModel6
                | Level2DynamicArenaKind::AuthoredModel17
                | Level2DynamicArenaKind::Model17SpawnedChild
                | Level2DynamicArenaKind::AuthoredModel22
                | Level2DynamicArenaKind::AuthoredModel76
                | Level2DynamicArenaKind::Model76TailSpawn
                | Level2DynamicArenaKind::AuthoredModel59
                | Level2DynamicArenaKind::Model59SmokeChild
                | Level2DynamicArenaKind::AuthoredModel53
                | Level2DynamicArenaKind::AuthoredModel5WaterSplash
                | Level2DynamicArenaKind::BeyondSightPulseEffect
                | Level2DynamicArenaKind::ManaAttraction => {
                    handler == ExistingHandler::Class10Effects
                }
                Level2DynamicArenaKind::SpeedTrail => handler == ExistingHandler::Class10Effects,
                Level2DynamicArenaKind::CastleRoot => handler == ExistingHandler::CastleRoot,
                Level2DynamicArenaKind::CastleAuxiliary => {
                    handler == ExistingHandler::CastleAuxiliary
                }
                Level2DynamicArenaKind::CastleDefender | Level2DynamicArenaKind::ManaCarrier => {
                    handler == ExistingHandler::ManaCarrier
                }
                Level2DynamicArenaKind::SpellReplacement | Level2DynamicArenaKind::SpellPickup => {
                    handler == ExistingHandler::SpellHolderAndPickup
                }
                Level2DynamicArenaKind::AuthoredBee => handler == ExistingHandler::BeeFamily,
                Level2DynamicArenaKind::Model6SpawnedChild => {
                    handler == ExistingHandler::Model6ProjectileChild
                }
                Level2DynamicArenaKind::AuthoredClass14 => {
                    handler == ExistingHandler::Class14Events
                }
                Level2DynamicArenaKind::AuthoredClass5
                | Level2DynamicArenaKind::AuthoredClass5Model3Child
                | Level2DynamicArenaKind::HouseResident => matches!(
                    handler,
                    ExistingHandler::MultipartFamily
                        | ExistingHandler::ArcherFamily
                        | ExistingHandler::Model9Family
                        | ExistingHandler::BuilderFamily
                        | ExistingHandler::VillagerFamily
                        | ExistingHandler::BuilderVillagerPostBuild
                        | ExistingHandler::TraderFamily
                        | ExistingHandler::FireflyFamily
                ),
                Level2DynamicArenaKind::AuthoredSwitchProducer => {
                    matches!(
                        handler,
                        ExistingHandler::SwitchProducers | ExistingHandler::CountdownSwitch
                    )
                }
                Level2DynamicArenaKind::AuthoredTree => handler == ExistingHandler::TreeLifecycle,
                Level2DynamicArenaKind::AuthoredModel32 => {
                    handler == ExistingHandler::ObjectiveSwitch
                }
                Level2DynamicArenaKind::Model22SpawnedChild
                | Level2DynamicArenaKind::Model76Child => false,
            },
        }
    }

    fn validate_semantic_source_visit(
        &self,
        source_slot: u16,
        visit: &Level2ResolvedSourceVisit,
    ) -> std::result::Result<(), String> {
        if matches!(
            visit.route,
            Level2SourceRoute::Dynamic(Level2DynamicArenaCursorRow {
                kind: Level2DynamicArenaKind::AuthoredClass5 | Level2DynamicArenaKind::AuthoredBee,
                ..
            })
        ) {
            validate_semantic_creature_dispatch(
                source_slot,
                self.creature_kind_for_source_slot(source_slot),
                visit.handler,
            )?;
        }
        let semantic_scenery = self.scenery_kind_for_source_slot(source_slot);
        if semantic_scenery.is_some()
            || matches!(
                visit.route,
                Level2SourceRoute::Dynamic(Level2DynamicArenaCursorRow {
                    kind: Level2DynamicArenaKind::AuthoredTree,
                    ..
                })
            )
            || visit.handler == ExistingHandler::TreeLifecycle
        {
            validate_semantic_scenery_dispatch(source_slot, semantic_scenery, visit.handler)?;
        }
        if let Level2SourceRoute::Dynamic(Level2DynamicArenaCursorRow {
            kind: Level2DynamicArenaKind::AuthoredClass14,
            ..
        }) = visit.route
        {
            let source = self
                .session
                .state
                .live
                .economy
                .economy
                .authored_class14()
                .iter()
                .find(|source| source.active && source.source_entity_index == source_slot)
                .ok_or_else(|| {
                    format!("item source slot {source_slot} has no live class-14 carrier")
                })?;
            match source.model {
                3 => validate_semantic_scenery_dispatch(
                    source_slot,
                    self.scenery_kind_for_source_slot(source_slot),
                    visit.handler,
                )?,
                5 => validate_semantic_item_dispatch(
                    source_slot,
                    self.item_kind_for_source_slot(source_slot),
                    source.model,
                    source.action,
                    visit.handler,
                )?,
                model => {
                    return Err(format!(
                        "class-14 source slot {source_slot} has unsupported model {model}"
                    ));
                }
            }
        }
        if let Level2SourceRoute::Dynamic(Level2DynamicArenaCursorRow {
            kind: Level2DynamicArenaKind::SpellPickup,
            ..
        }) = visit.route
        {
            let source = self
                .session
                .state
                .live
                .economy
                .economy
                .spell_pickups()
                .iter()
                .find(|source| source.active && source.source_entity_index == source_slot)
                .ok_or_else(|| {
                    format!("spell source slot {source_slot} has no live pickup carrier")
                })?;
            let spell_kind = self.spell_kind_for_source_slot(source_slot);
            validate_semantic_spell_dispatch(
                source_slot,
                spell_kind,
                spell_kind.and_then(|kind| self.session.definition.spell_family(kind)),
                source.family,
                visit.handler,
            )?;
        }
        if self.session.definition.semantic_level().is_some()
            && (matches!(visit.route, Level2SourceRoute::House)
                || matches!(
                    visit.route,
                    Level2SourceRoute::Dynamic(Level2DynamicArenaCursorRow {
                        kind: Level2DynamicArenaKind::AuthoredModel45,
                        ..
                    })
                ))
        {
            let raw_action = match visit.route {
                Level2SourceRoute::House => self
                    .session
                    .state
                    .live
                    .ordinary
                    .houses()
                    .iter()
                    .find(|house| house.active && house.entity_index == source_slot)
                    .map(|house| house.action),
                Level2SourceRoute::Dynamic(Level2DynamicArenaCursorRow {
                    kind: Level2DynamicArenaKind::AuthoredModel45,
                    ..
                }) => self
                    .session
                    .state
                    .live
                    .economy
                    .economy
                    .authored_model45()
                    .iter()
                    .find(|source| source.active && source.source_entity_index == source_slot)
                    .map(|source| source.action),
                _ => None,
            }
            .ok_or_else(|| format!("building source slot {source_slot} has no live carrier"))?;
            validate_semantic_building_dispatch(
                source_slot,
                self.building_kind_for_source_slot(source_slot, visit.route),
                visit.route,
                raw_action,
                visit.handler,
            )?;
        }
        if self.session.definition.semantic_level().is_some() {
            let semantic_switch = self.switch_kind_for_source_slot(source_slot);
            let raw_selector = match visit.route {
                Level2SourceRoute::Imported | Level2SourceRoute::ExitSwitch => self
                    .session
                    .state
                    .live
                    .economy
                    .entities
                    .get(usize::from(source_slot))
                    .map(|source| (source.class, source.model, source.action)),
                Level2SourceRoute::Dynamic(Level2DynamicArenaCursorRow {
                    kind: Level2DynamicArenaKind::AuthoredSwitchProducer,
                    ..
                }) => self
                    .session
                    .state
                    .live
                    .economy
                    .economy
                    .authored_switch_producers()
                    .iter()
                    .find(|source| source.active && source.source_entity_index == source_slot)
                    .map(|source| (source.source_class, source.model, source.action)),
                Level2SourceRoute::Dynamic(Level2DynamicArenaCursorRow {
                    kind: Level2DynamicArenaKind::AuthoredModel32,
                    ..
                }) => self
                    .session
                    .state
                    .live
                    .economy
                    .economy
                    .authored_model32()
                    .iter()
                    .find(|source| source.active && source.source_entity_index == source_slot)
                    .map(|source| (source.source_class, source.model, source.action)),
                _ => None,
            };
            if let Some((raw_class, raw_model, raw_action)) = raw_selector
                && (semantic_switch.is_some()
                    || (raw_class == 11 && matches!(raw_model, 0 | 1 | 4 | 12 | 32)))
            {
                validate_semantic_switch_dispatch(
                    source_slot,
                    semantic_switch,
                    raw_class,
                    raw_model,
                    raw_action,
                    visit.handler,
                )?;
            }
        }
        Ok(())
    }

    fn dispatch_level2_source_visit(
        &mut self,
        pass: u8,
        latched_command: crate::LatchedCommand,
        evidence: &mut Vec<crate::StepEvidence>,
        source_slot: u16,
        visit: &Level2ResolvedSourceVisit,
        player_entity_index: u16,
        player_position: &mut Position,
        source_player_order: &[usize],
        goat_players: &[GoatPlayerTarget],
        pass_start_mana_sphere_slots: &BTreeSet<u16>,
        pass_start_class5_model_slots: &[Vec<u16>; 29],
        pass_start_castle_auxiliary_membership:
            &level2_dynamic_economy::Level2CastleAuxiliaryMembership,
        pass_start_castle_root_slot: Option<u16>,
        goat_begin_events: &mut BTreeMap<u16, Vec<GoatStepEvent>>,
    ) -> std::result::Result<(), String> {
        match (visit.handler, visit.route) {
            (ExistingHandler::BallisticScenery, Level2SourceRoute::Imported) => {
                self.step_imported_ballistic_scenery(source_slot)?;
            }
            (ExistingHandler::CountdownSwitch, Level2SourceRoute::Imported) => {
                let observed_creature_present = self.countdown_observed_creature_present(
                    source_slot,
                    pass_start_class5_model_slots,
                )?;
                self.step_imported_countdown_switch(pass, source_slot, observed_creature_present)?;
            }
            (
                ExistingHandler::CountdownSwitch,
                Level2SourceRoute::Dynamic(Level2DynamicArenaCursorRow {
                    kind: Level2DynamicArenaKind::AuthoredSwitchProducer,
                    ..
                }),
            ) => {
                let observed_creature_present = self.countdown_observed_creature_present(
                    source_slot,
                    pass_start_class5_model_slots,
                )?;
                self.step_authored_countdown_switch(pass, source_slot, observed_creature_present)?;
            }
            (ExistingHandler::TerrainStripController, Level2SourceRoute::Imported) => {
                self.step_imported_terrain_strip(source_slot)?;
            }
            (ExistingHandler::SwitchProducers, Level2SourceRoute::Imported) => {
                self.step_imported_switch(pass, source_slot, source_player_order)?;
            }
            (ExistingHandler::ObjectiveSwitch, Level2SourceRoute::Imported) => {
                self.step_imported_switch(pass, source_slot, source_player_order)?;
            }
            (ExistingHandler::TreeLifecycle, Level2SourceRoute::Imported) => {
                self.step_imported_tree(source_slot)?;
            }
            (ExistingHandler::Stone, Level2SourceRoute::Imported) => {
                self.step_imported_stone(source_slot)?;
            }
            (ExistingHandler::Model9Family, Level2SourceRoute::Imported) => {
                self.step_imported_model9(source_slot)?;
            }
            (ExistingHandler::MultipartFamily, Level2SourceRoute::Imported) => {
                self.step_imported_multipart(source_slot, pass_start_class5_model_slots)?;
            }
            (ExistingHandler::PlayerFlight, Level2SourceRoute::Player) => {
                // The player source owns mailbox consumption.  A Bee
                // contact written by a later source slot therefore stays
                // pending until the next outer pass, while an earlier
                // source consumes it in this pass.
                // The qualified local player body is class 3/model 0;
                // model-2 diagnostic rows use the same entity slot for a
                // wind/mailbox target and must remain on that callback.
                let local_player_body = self
                    .session
                    .state
                    .live
                    .economy
                    .entities
                    .get(usize::from(player_entity_index))
                    .is_some_and(|entity| {
                        entity.index == player_entity_index
                            && entity.class == 3
                            && entity.model == 0
                    });
                if local_player_body {
                    let player_action_at_visit = self.session.state.live.economy.entities
                        [usize::from(player_entity_index)]
                    .action;
                    let mut lethal_at_visit = false;
                    // `AddPlayer03_00_5E010` tests the Castle shift AABB
                    // against the entity-111 position before the generic
                    // body updates position. Preserve that source-time
                    // observation for the following upkeep write.
                    let castle_shift_overlap = self
                        .session
                        .state
                        .live
                        .economy
                        .entities
                        .iter()
                        .find(|entity| {
                            entity.index == self.session.state.live.core.state.entity_index
                        })
                        .is_some_and(|player| {
                            self.session
                                .state
                                .live
                                .economy
                                .player_castle_shift_aabb_overlap(player)
                        });
                    if player_action_at_visit == 0 {
                        if let Some((damage, attacker, lethal)) =
                            self.consume_level2_player_damage()
                        {
                            lethal_at_visit = lethal;
                            self.session.state.live.economy.events.push(
                                Level2EconomyEvent::DamageApplied {
                                    target_entity_index: player_entity_index,
                                    attacker_entity_index: attacker,
                                    damage,
                                    lethal,
                                },
                            );
                        }
                    }
                    self.append_new_economy_events(pass, source_slot);

                    if player_action_at_visit == 2 {
                        // remc2 action 2 calls the recovered sub_5D530 body
                        // directly: no new PlayerEvents input or spell-fire
                        // command is latched while the player is dying.
                        let profile = self.session.definition.profile;
                        let context_event_start = self.session.state.live.core.context.events.len();
                        let body = {
                            let campaign = &mut self.session.state;
                            let core = &mut campaign.live.core;
                            let terrain = &campaign.live.terrain;
                            crate::step_entity_body_with_context(
                                &mut core.state,
                                profile,
                                terrain,
                                &mut core.context,
                            )
                        };
                        let context_events = self.session.state.live.core.context.events
                            [context_event_start..]
                            .to_vec();
                        self.append_context_effects(context_events);
                        self.step_level2_player_dying_action(pass, source_slot, body.proposed);
                        *player_position = self.session.state.live.core.state.position;
                        evidence.push(body);
                    } else if player_action_at_visit == crate::level_exit::EXIT_PLAYER_ACTION {
                        self.step_level_exit_player_action(pass, source_slot)?;
                        *player_position = self.session.state.live.core.state.position;
                    } else if player_action_at_visit == 0 {
                        // `UpdateEntities_57730` reaches the local player at
                        // source slot 111 before the later Possession holder
                        // (116) and Castle (117). Keep the complete generic
                        // flight/player callback here so its selected spell,
                        // movement, and upkeep writes are visible to those
                        // later source handlers in this same pass.
                        self.refresh_level2_spell_fire_holder_projection();
                        let profile = self.session.definition.profile;
                        let context_event_start = self.session.state.live.core.context.events.len();
                        let campaign = Some(&mut self.session.state)
                            .expect("Level 2 campaign state owns canonical runtime");
                        let core = &mut campaign.live.core;
                        let terrain = &campaign.live.terrain;
                        evidence.push(crate::step_entity_pass_with_context(
                            &mut core.state,
                            latched_command,
                            profile,
                            terrain,
                            &mut core.context,
                        ));
                        let context_events = self.session.state.live.core.context.events
                            [context_event_start..]
                            .to_vec();
                        self.append_context_effects(context_events);
                        self.sync_level2_spell_fire_authority(latched_command.flags);
                        self.set_level2_entity_position(
                            self.session.state.live.core.state.entity_index,
                            self.session.state.live.core.state.position,
                        )
                        .expect("qualified Level 2 local-player map carrier remains valid");
                        self.session
                            .state
                            .live
                            .shared_spells
                            .step_local_player_unboosted_upkeep(
                                self.session.state.live.economy.player_castle_entity_index(),
                                castle_shift_overlap,
                                self.session
                                    .state
                                    .live
                                    .economy
                                    .owner_max_mana
                                    .get(&self.session.state.live.core.state.entity_index)
                                    .copied(),
                            );
                        self.session
                            .state
                            .live
                            .shared_spells
                            .sync_typed_spell_parent_caches_from_player_entity();
                        self.sync_level2_spell_fire_context();
                        if lethal_at_visit {
                            mark_source_audio_call(SourceAudioCallTag::PlayerDeath);
                            self.push_flight_effect(FlightEvent::PrepareSound {
                                #[cfg(test)]
                                audio_site: crate::MissionAudioSite::PlayerDeath,
                                entity_index: player_entity_index,
                                player_index: -1,
                                sound_id: 16,
                            });
                            // `Scene::DEAD` is the fifth variant in the pinned remc2 enum.
                            self.push_scene_change_effect(4);
                        }
                    } else if player_action_at_visit == 3 {
                        self.step_level2_player_banished_action();
                    }
                }
            }
            (ExistingHandler::SpellHolderAndPickup, Level2SourceRoute::SpeedPickup) => {
                let terrain = &self.session.state.live.terrain;
                let result = self.session.state.live.shared_spells.step_shared_pickup(
                    source_slot,
                    player_entity_index,
                    *player_position,
                    terrain,
                );
                match result {
                    Ok(result) => self.push_dispatch_effect(Level2DispatchEvent::SharedPickup {
                        pass,
                        source_slot,
                        result,
                    }),
                    Err(error) => self.push_dispatch_effect(Level2DispatchEvent::Error {
                        pass,
                        source_slot,
                        message: error.to_string(),
                    }),
                }
            }
            (ExistingHandler::SpellHolderAndPickup, Level2SourceRoute::ImportedSpellSource) => {
                self.refresh_imported_spell_source_projection(source_slot)
                    .map_err(|error| error.to_string())?;
                let family = self
                    .session
                    .state
                    .live
                    .shared_spells
                    .pickups()
                    .iter()
                    .chain(self.session.state.live.shared_spells.dynamic_pickups())
                    .find(|pickup| pickup.active && pickup.index == source_slot)
                    .map(|pickup| pickup.model)
                    .ok_or_else(|| {
                        format!(
                            "generic spell pickup source slot {source_slot} lost its authoritative carrier"
                        )
                    })?;
                let terrain = &self.session.state.live.terrain;
                match self
                    .session
                    .state
                    .live
                    .shared_spells
                    .step_level2_spell_acquisition_with_economy_in_source_order(
                        &mut self.session.state.live.economy,
                        family,
                        player_entity_index,
                        *player_position,
                        terrain,
                        source_player_order,
                    ) {
                    Ok(result) => {
                        self.push_dispatch_effect(Level2DispatchEvent::SpellAcquisition {
                            pass,
                            source_slot,
                            family,
                            result,
                        })
                    }
                    Err(error) => self.push_dispatch_effect(Level2DispatchEvent::Error {
                        pass,
                        source_slot,
                        message: error.to_string(),
                    }),
                }
                // The outer entity dispatcher advances byte_0x3E after
                // the action-bearing handler returns; keep the live
                // pickup record in that source cadence rather than
                // relying on test-only phase mutation.
                if let Some(pickup) = self
                    .session
                    .state
                    .live
                    .shared_spells
                    .pickups
                    .iter_mut()
                    .find(|entity| entity.index == source_slot)
                {
                    pickup.phase = pickup.phase.wrapping_add(1);
                    pickup.record[66] = pickup.phase;
                }
                self.commit_imported_spell_source_projection(source_slot)
                    .map_err(|error| error.to_string())?;
            }
            (ExistingHandler::BadStone, Level2SourceRoute::BadStone) => {
                match self.step_bad_stone_action_9(source_slot) {
                    Ok(step) => {
                        self.push_dispatch_effect(Level2DispatchEvent::BadStone { pass, step })
                    }
                    Err(error) => self.push_dispatch_effect(Level2DispatchEvent::Error {
                        pass,
                        source_slot,
                        message: error.to_string(),
                    }),
                }
            }
            (ExistingHandler::Class10Effects, Level2SourceRoute::House) => {
                let ordinary_before = self.session.state.live.ordinary.clone();
                let economy_before = self.session.state.live.economy.clone();
                let terrain_before = self.canonical_terrain().clone();
                let terrain = &self.session.state.live.terrain;
                let events = self
                    .session
                    .state
                    .live
                    .ordinary
                    .step_house_action_52(source_slot, terrain);
                let result = {
                    let terrain = &mut self.session.state.live.terrain;
                    self.session.state.live.ordinary.apply_house_terrain_events(
                        &mut self.session.state.live.economy,
                        &events,
                        terrain,
                    )
                };
                match result {
                    Ok(()) => {
                        for event in &events {
                            if let HouseStepEvent::RemoveFootprint { entity_index, .. } = *event
                                && let Some(entity) = self
                                    .session
                                    .state
                                    .live
                                    .economy
                                    .entities
                                    .get_mut(usize::from(entity_index))
                            {
                                // Source action 53 calls DisableEntityDrawing;
                                // class-zero/map unlink belongs to the next
                                // UpdateEntities prologue.
                                entity.disable_drawing();
                            }
                        }
                        self.append_new_economy_events(pass, source_slot);
                        self.push_dispatch_effect(Level2DispatchEvent::House {
                            pass,
                            source_slot,
                            events: events.into_boxed_slice(),
                        });
                    }
                    Err(error) => {
                        self.session.state.live.ordinary = ordinary_before;
                        self.session.state.live.economy = economy_before;
                        *&mut self.session.state.live.terrain = terrain_before;
                        return Err(format!("Level 2 House sink failed: {error}"));
                    }
                }
            }
            (
                ExistingHandler::BuilderFamily
                | ExistingHandler::VillagerFamily
                | ExistingHandler::BuilderVillagerPostBuild
                | ExistingHandler::TraderFamily,
                Level2SourceRoute::Villager,
            ) => {
                let terrain = &self.session.state.live.terrain;
                let effects = self
                    .session
                    .state
                    .live
                    .ordinary
                    .step_villager_action_105(source_slot, terrain);
                self.session
                    .state
                    .live
                    .ordinary
                    .apply_villager_effects(&effects);
                // `KillEntity_1C930` allocates the class-10/model-1
                // death burst before disabling the dying villager's
                // source row. Keep the typed villager source id for this
                // relation; imported ordinary villagers have no owner
                // entry in the Level 2 economy map.
                for effect in &effects {
                    if let VillagerStepEffect::DeathBurst {
                        source_entity_index,
                        position,
                    } = effect
                    {
                        if let Some(villager) = self
                            .session
                            .state
                            .live
                            .ordinary
                            .villagers()
                            .iter()
                            .find(|villager| villager.entity_index == *source_entity_index)
                        {
                            let _ = self
                                .session
                                .state
                                .live
                                .economy
                                .spawn_death_burst_from_death_with_id(
                                    *source_entity_index,
                                    villager.source_id,
                                    *position,
                                );
                        }
                    }
                }
                self.append_new_economy_events(pass, source_slot);
                self.push_dispatch_effect(Level2DispatchEvent::Villager {
                    pass,
                    source_slot,
                    effects: effects.into_boxed_slice(),
                });
            }
            (ExistingHandler::Dolmen, Level2SourceRoute::Dolmen) => {
                let terrain = &self.session.state.live.terrain;
                match self
                    .session
                    .state
                    .live
                    .ordinary
                    .step_dolmen_action_6(source_slot, terrain)
                {
                    Ok(step) => {
                        self.push_dispatch_effect(Level2DispatchEvent::Dolmen { pass, step })
                    }
                    Err(error) => self.push_dispatch_effect(Level2DispatchEvent::Error {
                        pass,
                        source_slot,
                        message: error.to_string(),
                    }),
                }
            }
            (ExistingHandler::SwitchProducers, Level2SourceRoute::ExitSwitch) => {
                self.step_imported_switch(pass, source_slot, source_player_order)?;
            }
            (ExistingHandler::CastleRoot, Level2SourceRoute::CastleRoot) => {
                let construction_completion =
                    self.session
                        .state
                        .live
                        .economy
                        .castle()
                        .is_some_and(|castle| {
                            castle.active && castle.action == 5 && castle.build_state == 2
                        });
                let reset_free_slots = self.session.baseline.economy.economy.free_slots.clone();
                let terrain = &mut self.session.state.live.terrain;
                let advanced = self.session.state.live.economy.step_castle_action(
                    source_slot,
                    terrain,
                    &reset_free_slots,
                );
                // Publish the source handler's economy sidecars before
                // the coarse Castle wrapper, matching the handler-first
                // ApplyEvents cursor order used by dynamic rows.
                self.append_new_economy_events(pass, source_slot);
                if pass_start_castle_root_slot == Some(source_slot) {
                    self.push_dispatch_effect(Level2DispatchEvent::Castle {
                        pass,
                        source_slot,
                        advanced,
                    });
                } else {
                    // Preserve the established public observation for a
                    // root born into a higher numeric slot during this
                    // pass. The handler still runs immediately; only its
                    // compatibility receipt remains the dynamic family-1
                    // projection used before the scheduler cutover.
                    self.push_dispatch_effect(Level2DispatchEvent::DynamicArena {
                        pass,
                        source_slot,
                        kind: Level2DynamicArenaKind::CastleRoot,
                    });
                }
                if construction_completion
                    && self
                        .session
                        .state
                        .live
                        .economy
                        .castle()
                        .is_some_and(|castle| castle.action == 4)
                    && let Some(holder_index) = self
                        .session
                        .state
                        .live
                        .shared_spells
                        .holders
                        .iter()
                        .chain(self.session.state.live.shared_spells.dynamic_pickups.iter())
                        .find(|holder| holder.model == CASTLE_SPELL_FAMILY)
                        .map(|holder| holder.index)
                {
                    // `BeginOfCastleCreation_5FA70` state 2 calls
                    // `sub_5F890(caster, 0)` before leaving action 5;
                    // construction, rather than a timer tick, releases
                    // the holder for the next public upgrade cast.
                    self.clear_castle_spell_holder_cooldown(holder_index);
                }
            }
            (
                ExistingHandler::FireflyFamily,
                Level2SourceRoute::Dynamic(Level2DynamicArenaCursorRow {
                    kind: Level2DynamicArenaKind::AuthoredClass5,
                    ..
                }),
            ) => {
                let (action, target_entity_index) = self
                    .session
                    .state
                    .live
                    .economy
                    .authored_class5()
                    .iter()
                    .find(|source| source.source_entity_index == source_slot)
                    .map(|source| (source.action, source.word_0x96_150))
                    .ok_or_else(|| format!("Firefly source slot {source_slot} is absent"))?;
                let terrain = &self.session.state.live.terrain;
                let lifecycle = self
                    .session
                    .state
                    .live
                    .economy
                    .step_authored_firefly_lifecycle_slot(source_slot, terrain)?;
                if lifecycle.handled {
                    if let Some(killer_entity_index) = lifecycle.pre_kill_credit {
                        let qualifying_killer = killer_entity_index != source_slot
                            && self
                                .session
                                .state
                                .live
                                .economy
                                .entities
                                .iter()
                                .any(|entity| {
                                    entity.index == killer_entity_index
                                        && entity.active
                                        && entity.life >= 0
                                        && entity.class == 3
                                        && entity.model == 0
                                        && !entity.drawing_disabled()
                                });
                        if qualifying_killer {
                            let _ = self
                                .session
                                .state
                                .live
                                .economy
                                .record_authored_creature_kill(killer_entity_index);
                        }
                    }
                    self.append_new_economy_events(pass, source_slot);
                    if lifecycle.retained_for_epilogue {
                        self.push_dispatch_effect(Level2DispatchEvent::DynamicArena {
                            pass,
                            source_slot,
                            kind: Level2DynamicArenaKind::AuthoredClass5,
                        });
                    }
                    return Ok(());
                }
                let frozen_model19_slots = &pass_start_class5_model_slots
                    [usize::from(level2_class5::firefly::FIREFLY_MODEL)];
                let attack_target = self.firefly_attack_target(target_entity_index);
                let live = match action {
                    level2_class5::firefly::FIREFLY_STAGE_ACTION => self
                        .session
                        .state
                        .live
                        .economy
                        .step_authored_firefly_stage_slot(
                            source_slot,
                            frozen_model19_slots,
                            terrain,
                        ),
                    level2_class5::firefly::FIREFLY_ROAM_ACTION => self
                        .session
                        .state
                        .live
                        .economy
                        .step_authored_firefly_roam_slot(
                            source_slot,
                            goat_players,
                            frozen_model19_slots,
                            terrain,
                        ),
                    level2_class5::firefly::FIREFLY_LINKED_ACTION => self
                        .session
                        .state
                        .live
                        .economy
                        .step_authored_firefly_linked_slot(
                            source_slot,
                            frozen_model19_slots,
                            terrain,
                        ),
                    level2_class5::firefly::FIREFLY_ATTACK_ACTION => {
                        let step = self
                            .session
                            .state
                            .live
                            .economy
                            .step_authored_firefly_charge_slot(
                                source_slot,
                                attack_target,
                                frozen_model19_slots,
                                terrain,
                            )?;
                        if let Some(sound_id) = step.sound_id {
                            mark_source_audio_call(SourceAudioCallTag::FireflyChargeAction154);
                            self.push_flight_effect(FlightEvent::PrepareSound {
                                #[cfg(test)]
                                audio_site: crate::MissionAudioSite::FireflyCharge,
                                entity_index: source_slot,
                                player_index: -1,
                                sound_id,
                            });
                        }
                        if let Some(launch) = step.fireball {
                            let source_arena = &mut self.session.state.live.economy;
                            let spawned =
                                source_arena.economy.spawn_fireball_projectile_with_payload(
                                    level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
                                        origin: FireballProjectileOrigin::Firefly,
                                        position: launch.position,
                                        yaw: launch.yaw,
                                        pitch: launch.pitch,
                                        axis_position: launch.target_position,
                                        source_id_entity_index: launch.owner_entity_index,
                                        holder_entity_index: 0,
                                        source_subspell_index: launch.payload,
                                        source_mana: 0,
                                        source_dword_0x10: 0,
                                        target_source_entity_index: launch.target_entity_index,
                                    },
                                    0,
                                );
                            if spawned.is_some() {
                                let _ = Level2EconomyState::apply_firefly_fireball_target_feedback(
                                    &mut source_arena.entities,
                                    launch.target_entity_index,
                                );
                            }
                        }
                        if let Some(contact) = step.contact {
                            let local_player = self.session.state.live.core.state.entity_index;
                            if contact.target_entity_index != local_player {
                                return Err(format!(
                                    "Firefly source slot {source_slot} produced contact for unsupported nonlocal player {}",
                                    contact.target_entity_index
                                ));
                            }
                            if contact.attacker_entity_index != source_slot {
                                return Err(format!(
                                    "Firefly source slot {source_slot} produced mismatched attacker {}",
                                    contact.attacker_entity_index
                                ));
                            }
                            self.queue_level2_player_contact_damage(
                                contact.damage,
                                contact.attacker_entity_index,
                            );
                        }
                        step.live
                    }
                    _ => {
                        return Err(format!(
                            "Firefly source slot {source_slot} reached unimplemented action {action}"
                        ));
                    }
                };
                if live {
                    self.push_dispatch_effect(Level2DispatchEvent::DynamicArena {
                        pass,
                        source_slot,
                        kind: Level2DynamicArenaKind::AuthoredClass5,
                    });
                }
            }
            (
                ExistingHandler::TreeLifecycle
                | ExistingHandler::CastleRoot
                | ExistingHandler::ManaCarrier
                | ExistingHandler::MultipartFamily
                | ExistingHandler::ArcherFamily
                | ExistingHandler::Model9Family
                | ExistingHandler::BuilderFamily
                | ExistingHandler::VillagerFamily
                | ExistingHandler::BuilderVillagerPostBuild
                | ExistingHandler::TraderFamily
                | ExistingHandler::ProjectileFamily
                | ExistingHandler::Model6ProjectileChild
                | ExistingHandler::Class10Effects
                | ExistingHandler::SwitchProducers
                | ExistingHandler::ObjectiveSwitch
                | ExistingHandler::Class14Events
                | ExistingHandler::SpellHolderAndPickup
                | ExistingHandler::BeeFamily
                | ExistingHandler::CastleAuxiliary,
                Level2SourceRoute::Dynamic(row),
            ) => {
                if row.kind == Level2DynamicArenaKind::AuthoredModel45 {
                    if let Some(target_group) = self
                        .session
                        .state
                        .live
                        .economy
                        .authored_model45()
                        .iter()
                        .find(|source| source.source_entity_index == source_slot)
                        .map(|source| source.target_group)
                        .filter(|target_group| *target_group != 0)
                    {
                        self.activate_level2_authored_spell_group(pass, source_slot, target_group);
                    }
                }
                let live = if row.kind == Level2DynamicArenaKind::SpellPickup {
                    let is_active_speed_holder = self
                        .session
                        .state
                        .live
                        .shared_spells
                        .pickup(source_slot)
                        .is_some_and(|holder| holder.model == 3 && holder.action == 9);
                    if is_active_speed_holder {
                        self.step_dynamic_speed_holder(pass, source_slot)
                    } else {
                        self.step_level2_dynamic_spell_pickup(
                            pass,
                            source_slot,
                            player_entity_index,
                            *player_position,
                            source_player_order,
                        )
                    }
                } else if row.kind == Level2DynamicArenaKind::SpeedTrail {
                    self.session
                        .state
                        .live
                        .economy
                        .economy
                        .step_speed_trail(source_slot)
                } else if row.kind == Level2DynamicArenaKind::SpellReplacement {
                    self.step_level2_dynamic_spell_replacement(
                        pass,
                        source_slot,
                        player_entity_index,
                        *player_position,
                        source_player_order,
                    )
                } else {
                    self.step_level2_dynamic_arena_with_sphere_membership(
                        pass,
                        source_slot,
                        row.kind,
                        Some(pass_start_mana_sphere_slots),
                        Some(pass_start_castle_auxiliary_membership),
                    )
                };
                self.append_new_economy_events(pass, source_slot);
                if live {
                    self.push_dispatch_effect(Level2DispatchEvent::DynamicArena {
                        pass,
                        source_slot,
                        kind: row.kind,
                    });
                }
            }
            (ExistingHandler::GoatFamily, Level2SourceRoute::Goat) => {
                let mut events = goat_begin_events.remove(&source_slot).unwrap_or_default();
                let terrain = &self.session.state.live.terrain;
                events.extend(self.session.state.live.goats.step_source_slot(
                    source_slot,
                    terrain,
                    goat_players,
                ));
                self.apply_level2_goat_economy_events(&events);
                self.append_new_economy_events(pass, source_slot);
                self.push_dispatch_effect(Level2DispatchEvent::Goat {
                    pass,
                    source_slot,
                    events: events.into_boxed_slice(),
                });
            }
            (ExistingHandler::NyphurPlayer, Level2SourceRoute::Nyphur) => {
                let move_speed_flag = self
                    .session
                    .state
                    .live
                    .core
                    .context
                    .effects
                    .as_ref()
                    .map_or(0, |effects| effects.move_speed_flag);
                let active_level_color_index = self
                    .session
                    .state
                    .live
                    .shared_spells
                    .local_player
                    .player_color_index();
                let terrain = &self.session.state.live.terrain;
                let players = &self.session.state.live.core.players;
                let source_arena = &mut self.session.state.live.economy;
                let (entities, economy) = (&mut source_arena.entities, &mut source_arena.economy);
                let castle = economy.castle();
                let castle_children = economy.castle_children().to_vec();
                match self.session.state.live.nyphur_dispatcher.step_dispatch(
                    pass,
                    source_slot,
                    entities,
                    economy.spheres_mut(),
                    castle,
                    &castle_children,
                    players,
                    terrain,
                    &self.session.definition.presentation,
                    move_speed_flag,
                    active_level_color_index,
                ) {
                    Ok((_snapshot, trace)) => self
                        .session
                        .state
                        .scratch
                        .nyphur_dispatch_traces
                        .push(trace),
                    Err(error) => self.push_dispatch_effect(Level2DispatchEvent::Error {
                        pass,
                        source_slot,
                        message: error.to_string(),
                    }),
                }
                // The scheduler epilogue advances the current occupant.
            }
            (ExistingHandler::SpellHolderAndPickup, Level2SourceRoute::PossessionHolder) => {
                // The holder is a constructor/producer only. The
                // newly allocated row is dispatched by the source cursor
                // below, and emits DynamicArena only if its callback
                // returns live.
                if source_slot == LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX {
                    self.step_level2_nyphur_possession_holder_source_slot(source_slot);
                } else {
                    self.step_level2_possession_holder_source_slot(source_slot);
                }
                // The scheduler epilogue advances the current occupant
                // after this constructor callback returns.
            }
            (ExistingHandler::SpellHolderAndPickup, Level2SourceRoute::CastleHolder) => {
                // The holder is a constructor/producer only. The
                // higher-slot flyer callback owns the DynamicArena
                // receipt after step_level2_dynamic_arena returns live.
                self.step_level2_castle_holder_source_slot(source_slot);
                // The scheduler epilogue advances byte 0x3e, including W=0.
            }
            (ExistingHandler::SpellHolderAndPickup, Level2SourceRoute::FireballHolder) => {
                // Family-0's class/action keyed handler owns the
                // producer transition; the higher-slot Fireball callback
                // receives the ordinary dynamic-arena dispatch below.
                if source_slot == LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX {
                    self.step_level2_nyphur_fireball_holder_source_slot(source_slot);
                } else {
                    self.step_level2_fireball_holder_source_slot(source_slot);
                }
            }
            (handler, route) => {
                return Err(format!(
                    "registry handler {handler:?} has no Level 2 delegate for {route:?}"
                ));
            }
        }
        Ok(())
    }

    fn step_imported_ballistic_scenery(
        &mut self,
        source_slot: u16,
    ) -> std::result::Result<(), String> {
        let live = &mut self.session.state.live;
        let outcome = {
            let source = live
                .economy
                .entities
                .get_mut(usize::from(source_slot))
                .filter(|source| {
                    source.active
                        && source.index == source_slot
                        && source.class == 2
                        && source.action == 21
                })
                .ok_or_else(|| {
                    format!(
                        "ballistic-scenery callback lost class-2/action-21 source slot {source_slot}"
                    )
                })?;
            ballistic_scenery::step_ballistic_scenery(source, &live.terrain)
        };

        let position = live.economy.entities[usize::from(source_slot)].position;
        live.economy
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: position.x,
                y: position.y,
            })
            .map_err(|error| {
                format!("ballistic-scenery source slot {source_slot} map sync failed: {error}")
            })?;

        match outcome.spawn {
            Some(ballistic_scenery::BallisticScenerySpawn::Model13Smoke) => {
                let _ = live
                    .economy
                    .economy
                    .allocate_ballistic_death_smoke(position);
            }
            Some(ballistic_scenery::BallisticScenerySpawn::Model5WaterSplash) => {
                let _ = live
                    .economy
                    .economy
                    .allocate_authored_model5_water_splash(position, &live.terrain);
            }
            None => {}
        }
        if outcome.hide {
            live.economy.entities[usize::from(source_slot)].disable_drawing();
        }
        Ok(())
    }

    /// Run remc2 `AddTree02_00_64E20` against the lossless imported source
    /// row. Callback-created class-10 rows use the existing shared arena, so
    /// a higher numeric birth is naturally visible to this pass's cursor.
    fn step_imported_tree(&mut self, source_slot: u16) -> std::result::Result<(), String> {
        let live = &mut self.session.state.live;
        let source = live
            .economy
            .entities
            .get(usize::from(source_slot))
            .filter(|source| {
                source.active
                    && source.index == source_slot
                    && source.class == 2
                    && source.model == 0
                    && source.action == 0
            })
            .cloned()
            .ok_or_else(|| format!("tree callback lost source slot {source_slot}"))?;
        let ground_height = live.terrain.ground_height(source.position);
        let is_water = live.terrain.material_mask(source.position) == crate::WATER_AMBIENCE_MASK;
        let mut source_after = source;
        let (entities, economy) = (&live.economy.entities, &mut live.economy.economy);
        let outcome = campaign_shared_families::step_tree_action0(
            &mut source_after,
            ground_height,
            is_water,
            |request| match request.kind {
                campaign_shared_families::ScenerySpawnKind::TreeDeathSphere => {
                    let inherited_id = request
                        .inherit_source_id_from
                        .and_then(|slot| entities.get(usize::from(slot)))
                        .filter(|row| row.active && row.index != 0)
                        .map_or(request.fallback_source_id, |row| row.source_id);
                    let spawned = economy.allocate_authored_model6(
                        source_slot,
                        request.source_position,
                        &live.terrain,
                    )?;
                    let row = economy
                        .authored_model6
                        .iter_mut()
                        .find(|row| row.source_entity_index == spawned.source_entity_index)?;
                    row.entity_id = inherited_id;
                    row.height_offset = request.word_0x2c;
                    row.position.z = request.position_z;
                    Some(row.source_entity_index)
                }
                campaign_shared_families::ScenerySpawnKind::WaterSplash => {
                    let spawned = economy.allocate_authored_model5_water_splash(
                        request.source_position,
                        &live.terrain,
                    )?;
                    let row = economy
                        .authored_model5_water_splashes
                        .iter_mut()
                        .find(|row| row.source_entity_index == spawned.source_entity_index)?;
                    row.entity_id = request.fallback_source_id;
                    Some(row.source_entity_index)
                }
            },
        )
        .map_err(|error| error.to_string())?;
        if let Some(spawned_slot) = outcome.death_sphere_slot
            && let Some(spawned) = economy
                .authored_model6
                .iter_mut()
                .find(|row| row.source_entity_index == spawned_slot)
        {
            spawned.life = source_after.life as i16;
        }
        live.economy.entities[usize::from(source_slot)] = source_after;
        Ok(())
    }

    fn step_imported_stone(&mut self, source_slot: u16) -> std::result::Result<(), String> {
        let live = &mut self.session.state.live;
        let source = live
            .economy
            .entities
            .get_mut(usize::from(source_slot))
            .filter(|source| {
                source.active
                    && source.index == source_slot
                    && source.class == 2
                    && source.model == 1
                    && source.action == 3
            })
            .ok_or_else(|| format!("stone callback lost source slot {source_slot}"))?;
        let ground_height = live.terrain.ground_height(source.position);
        campaign_shared_families::step_stone_action3(source, ground_height)
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn step_imported_model9(&mut self, source_slot: u16) -> std::result::Result<(), String> {
        let action = self
            .session
            .state
            .live
            .economy
            .entities
            .get(usize::from(source_slot))
            .filter(|source| {
                source.active
                    && source.index == source_slot
                    && source.class == 5
                    && source.model == 9
            })
            .map(|source| source.action)
            .ok_or_else(|| format!("model-9 callback lost source slot {source_slot}"))?;
        match action {
            79 => return self.step_imported_model9_action79(source_slot),
            74 => return self.step_imported_model9_action74(source_slot),
            76 => return self.step_imported_model9_pre_kill(source_slot),
            77 => return self.step_imported_model9_kill(source_slot),
            72 => {}
            _ => {
                return Err(format!(
                    "model-9 handler rejects action {action} at source slot {source_slot}"
                ));
            }
        }

        let pose = self.model9_particle_pose(201)?;
        let _source = self
            .session
            .state
            .live
            .economy
            .entities
            .get(usize::from(source_slot))
            .filter(|source| {
                source.active
                    && source.index == source_slot
                    && source.class == 5
                    && source.model == 9
                    && source.action == 72
            })
            .ok_or_else(|| format!("model-9 callback lost source slot {source_slot}"))?;
        let source = &mut self.session.state.live.economy.entities[usize::from(source_slot)];
        let outcome = campaign_shared_families::step_model9_action72(source, pose)
            .map_err(|error| error.to_string())?;
        let Some(stage_relation) = outcome.stage_relation_to_advance else {
            return Ok(());
        };

        let stage_index = u8::try_from(stage_relation).map_err(|_| {
            format!("model-9 StageVar relation {stage_relation} exceeds source byte width")
        })?;
        let stage_variable = self
            .session
            .state
            .live
            .core
            .runtime
            .stage_variables
            .get(usize::from(stage_index))
            .copied()
            .ok_or_else(|| format!("model-9 StageVar relation {stage_index} is out of bounds"))?;
        let old_counter = (stage_variable.word_2 >> 8) as u8;
        if !self
            .session
            .state
            .live
            .core
            .runtime
            .advance_level2_stage_variable_counter(stage_index)
        {
            return Err(format!(
                "model-9 StageVar relation {stage_index} could not advance its raw carrier"
            ));
        }
        let reject = match stage_variable.flags as u8 & 0x60 {
            0x20 => old_counter & 3 == 3,
            0x40 => old_counter & 1 != 0,
            0x60 => old_counter & 3 != 0,
            _ => false,
        };
        if reject {
            return Err(format!(
                "model-9 StageVar relation {stage_index} reached source thinning before sub_12470 is admitted"
            ));
        }
        let word_0x4a = if stage_variable.kind == 6 {
            match stage_variable.relation {
                RuntimeRelation::Scalar(value) | RuntimeRelation::AxisScalar(value) => value as u16,
                RuntimeRelation::AuthoredFeature(value)
                | RuntimeRelation::LiveEntity(value)
                | RuntimeRelation::LiveEntityIndex(value) => value,
                RuntimeRelation::Null => 0,
            }
        } else {
            0
        };
        let source = &mut self.session.state.live.economy.entities[usize::from(source_slot)];
        source.set_source_action(source.model.wrapping_mul(8).wrapping_add(7));
        source.record[76] = stage_index;
        source.record[77] = stage_variable.kind as u8;
        source.record[78..80].copy_from_slice(&word_0x4a.to_le_bytes());
        Ok(())
    }

    fn model9_particle_pose(
        &self,
        particle_index: usize,
    ) -> std::result::Result<campaign_shared_families::ParticlePose, String> {
        let particle = self
            .session
            .definition
            .presentation
            .particles
            .get(particle_index)
            .copied()
            .ok_or_else(|| {
                format!("model-9 transition needs presentation particle {particle_index}")
            })?;
        let animation_limit = campaign_shared_families::particle_animation_limit(
            particle.render_mode,
        )
        .ok_or_else(|| {
            format!(
                "model-9 particle {particle_index} render mode {} has no remc2 animation limit",
                particle.render_mode,
            )
        })?;
        Ok(campaign_shared_families::ParticlePose {
            animation_limit,
            array_yaw: i16::try_from(particle.rotation_speed / 2)
                .map_err(|_| format!("model-9 particle {particle_index} yaw exceeds i16"))?,
            array_pitch: i16::try_from(particle.speed / 2)
                .map_err(|_| format!("model-9 particle {particle_index} pitch exceeds i16"))?,
            array_roll: i16::try_from(particle.speed / 2)
                .map_err(|_| format!("model-9 particle {particle_index} roll exceeds i16"))?,
            array_fov: i16::try_from(particle.rotation_speed / 2)
                .map_err(|_| format!("model-9 particle {particle_index} fov exceeds i16"))?,
        })
    }

    fn step_imported_model9_action79(
        &mut self,
        source_slot: u16,
    ) -> std::result::Result<(), String> {
        let reaction_pose = self.model9_particle_pose(202)?;
        let (stage_index, callback_start_entities) = {
            let live = &self.session.state.live;
            let source = &live.economy.entities[usize::from(source_slot)];
            (
                usize::from(source.record_bytes()[76]),
                live.economy.entities.clone(),
            )
        };
        let stage_target = match self
            .session
            .state
            .live
            .core
            .runtime
            .stage_variables
            .get(stage_index)
            .map(|stage| stage.relation)
        {
            Some(RuntimeRelation::AxisScalar(packed)) => Position {
                x: packed as u16,
                y: (packed >> 16) as u16,
                z: 0,
            },
            relation => {
                return Err(format!(
                    "model-9 source slot {source_slot} StageVar1 {stage_index} needs a packed axis target, found {relation:?}"
                ));
            }
        };
        let position = {
            let live = &mut self.session.state.live;
            let source = &mut live.economy.entities[usize::from(source_slot)];
            campaign_shared_families::step_model9_action79(
                source,
                &live.terrain,
                stage_target,
                &callback_start_entities,
                reaction_pose,
            )
            .map_err(|error| error.to_string())?;
            source.position
        };
        self.session
            .state
            .live
            .economy
            .economy
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: position.x,
                y: position.y,
            })
            .map_err(|error| {
                format!("model-9 source slot {source_slot} map sync failed: {error}")
            })?;
        Ok(())
    }

    fn step_imported_model9_action74(
        &mut self,
        source_slot: u16,
    ) -> std::result::Result<(), String> {
        let movement_pose = self.model9_particle_pose(201)?;
        let reaction_pose = self.model9_particle_pose(202)?;
        let callback_start_entities = self.session.state.live.economy.entities.clone();
        let position = {
            let live = &mut self.session.state.live;
            let source = &mut live.economy.entities[usize::from(source_slot)];
            campaign_shared_families::step_model9_action74(
                source,
                &live.terrain,
                &callback_start_entities,
                movement_pose,
                reaction_pose,
            )
            .map_err(|error| error.to_string())?;
            source.position
        };
        self.session
            .state
            .live
            .economy
            .economy
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: position.x,
                y: position.y,
            })
            .map_err(|error| {
                format!("model-9 source slot {source_slot} map sync failed: {error}")
            })?;
        Ok(())
    }

    fn step_imported_model9_pre_kill(
        &mut self,
        source_slot: u16,
    ) -> std::result::Result<(), String> {
        let mut child_slot =
            self.session.state.live.economy.entities[usize::from(source_slot)].word_34();
        let mut visited = 0_usize;
        while child_slot != 0 {
            let child = self
                .session
                .state
                .live
                .economy
                .entities
                .get_mut(usize::from(child_slot))
                .filter(|child| child.active && child.index == child_slot)
                .ok_or_else(|| {
                    format!(
                        "model-9 source slot {source_slot} has invalid pre-kill child {child_slot}"
                    )
                })?;
            child.set_source_action(77);
            child_slot = child.word_34();
            visited += 1;
            if visited >= self.session.state.live.economy.entities.len() {
                return Err(format!(
                    "model-9 source slot {source_slot} has a cyclic pre-kill child chain"
                ));
            }
        }
        self.session.state.live.economy.entities[usize::from(source_slot)].set_source_action(77);
        Ok(())
    }

    fn step_imported_model9_kill(&mut self, source_slot: u16) -> std::result::Result<(), String> {
        let source = self
            .session
            .state
            .live
            .economy
            .entities
            .get(usize::from(source_slot))
            .filter(|source| {
                source.active
                    && source.index == source_slot
                    && source.class == 5
                    && source.model == 9
                    && source.action == 77
            })
            .cloned()
            .ok_or_else(|| format!("model-9 kill callback lost source slot {source_slot}"))?;
        if source.phase & 7 != 0 {
            return Ok(());
        }

        let source_rng = if source.mana > 0 {
            campaign_shared_families::next_source_rng16(source.entity_rng)
        } else {
            source.entity_rng
        };
        if source.mana > 0 {
            let terrain = &self.session.state.live.terrain;
            let _ = self
                .session
                .state
                .live
                .economy
                .economy
                .spawn_mana_sphere_from_death(
                    source_slot,
                    source.position,
                    source.yaw,
                    source_rng,
                    source.mana,
                    source.source_player_entity_index(),
                    terrain,
                );
        }
        if source.control_flags & 0x0010_0000 == 0 {
            let _ = self
                .session
                .state
                .live
                .economy
                .economy
                .spawn_death_burst_from_death_with_id(
                    source_slot,
                    source.source_id,
                    source.position,
                );
        }
        let source = &mut self.session.state.live.economy.entities[usize::from(source_slot)];
        source.set_entity_rng(source_rng);
        source.set_source_player_entity_index(0);
        source.disable_drawing();
        Ok(())
    }

    fn step_imported_multipart(
        &mut self,
        source_slot: u16,
        pass_start_class5_model_slots: &[Vec<u16>; 29],
    ) -> std::result::Result<(), String> {
        let selector = self
            .session
            .state
            .live
            .economy
            .entities
            .get(usize::from(source_slot))
            .filter(|source| {
                source.active
                    && source.index == source_slot
                    && source.class == 5
                    && source.model == 3
            })
            .map(|source| source.action)
            .ok_or_else(|| format!("multipart callback lost source slot {source_slot}"))?;

        match selector {
            31 => {
                let stage_target = {
                    let source =
                        &self.session.state.live.economy.entities[usize::from(source_slot)];
                    let stage_index = usize::from(source.record_bytes()[76]);
                    let stage_variable = self
                        .session
                        .state
                        .live
                        .core
                        .runtime
                        .stage_variables
                        .get(stage_index)
                        .ok_or_else(|| {
                            format!(
                                "multipart source slot {source_slot} StageVar1 {stage_index} is out of bounds"
                            )
                        })?;
                    let target = if source.phase & 7 != 0 {
                        0
                    } else if stage_variable.flags as u8 & 2 != 0 {
                        let existing = u16::from_le_bytes([
                            source.record_bytes()[78],
                            source.record_bytes()[79],
                        ]);
                        if existing != 0 {
                            existing
                        } else {
                            let shared = (stage_variable.flags as u8 & 1 != 0)
                                .then(|| {
                                    pass_start_class5_model_slots
                                        .get(usize::from(source.model))?
                                        .iter()
                                        .copied()
                                        .find_map(|candidate_slot| {
                                            let candidate = self
                                                .session
                                                .state
                                                .live
                                                .economy
                                                .entities
                                                .get(usize::from(candidate_slot))?;
                                            let candidate_target = u16::from_le_bytes([
                                                candidate.record_bytes()[78],
                                                candidate.record_bytes()[79],
                                            ]);
                                            (candidate.record_bytes()[76]
                                                == source.record_bytes()[76]
                                                && candidate_target != 0)
                                                .then_some(candidate_target)
                                        })
                                })
                                .flatten();
                            shared.unwrap_or_else(|| {
                                let RuntimeRelation::AxisScalar(model) = stage_variable.relation
                                else {
                                    return 0;
                                };
                                let Ok(model) = u8::try_from(model) else {
                                    return 0;
                                };
                                let mut best = None;
                                let mut best_distance = u32::MAX;
                                for candidate in pass_start_class5_model_slots
                                    .get(usize::from(model))
                                    .into_iter()
                                    .flatten()
                                    .copied()
                                {
                                    let Some(candidate_row) = self
                                        .session
                                        .state
                                        .live
                                        .economy
                                        .entities
                                        .get(usize::from(candidate))
                                    else {
                                        continue;
                                    };
                                    let dx = i32::from(
                                        source.position.x.wrapping_sub(candidate_row.position.x)
                                            as i16,
                                    );
                                    let dy = i32::from(
                                        source.position.y.wrapping_sub(candidate_row.position.y)
                                            as i16,
                                    );
                                    let distance = (dx * dx + dy * dy) as u32;
                                    if distance < best_distance {
                                        best = Some(candidate);
                                        best_distance = distance;
                                    }
                                }
                                best.unwrap_or(0)
                            })
                        }
                    } else {
                        match stage_variable.relation {
                            RuntimeRelation::LiveEntity(slot)
                            | RuntimeRelation::LiveEntityIndex(slot) => slot,
                            _ => 0,
                        }
                    };
                    if target == 0 {
                        campaign_multipart::MultipartStageTarget::None
                    } else {
                        self.session.state.live.economy.entities[usize::from(source_slot)].record
                            [78..80]
                            .copy_from_slice(&target.to_le_bytes());
                        campaign_multipart::MultipartStageTarget::Entity(target)
                    }
                };
                let outcome = campaign_multipart::step_raw_multipart_action31(
                    &mut self.session.state.live.economy.entities,
                    source_slot,
                    &self.session.state.live.terrain,
                    stage_target,
                )
                .map_err(|error| error.to_string())?;
                if outcome.external_effect
                    == campaign_multipart::MultipartRootExternalEffect::DecrementLiveEntityCounter
                {
                    return Err(format!(
                        "multipart source slot {source_slot} reached the source creature-counter decrement before that shared carrier was imported"
                    ));
                }
            }
            232 => {
                campaign_multipart::step_raw_multipart_action232(
                    &mut self.session.state.live.economy.entities,
                    source_slot,
                )
                .map_err(|error| error.to_string())?;
            }
            action => {
                return Err(format!(
                    "multipart handler rejects action {action} at source slot {source_slot}"
                ));
            }
        }

        let source = &self.session.state.live.economy.entities[usize::from(source_slot)];
        self.session
            .state
            .live
            .economy
            .economy
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .map_err(|error| {
                format!("multipart source slot {source_slot} map sync failed: {error}")
            })?;
        Ok(())
    }

    fn step_imported_countdown_switch(
        &mut self,
        pass: u8,
        source_slot: u16,
        observed_creature_present: bool,
    ) -> std::result::Result<(), String> {
        let (target_group, outcome) = {
            let source = self
                .session
                .state
                .live
                .economy
                .entities
                .get_mut(usize::from(source_slot))
                .filter(|source| {
                    source.active
                        && source.index == source_slot
                        && source.class == 11
                        && source.model == 16
                        && source.action == 16
                })
                .ok_or_else(|| {
                    format!(
                        "countdown-switch callback lost class-11/model-16/action-16 source slot {source_slot}"
                    )
                })?;
            let target_group = source.source_id;
            let mut timer = source.source_dword_10();
            let outcome =
                countdown_switch::step_absence_countdown(&mut timer, observed_creature_present);
            source.set_source_dword_10(timer);
            (target_group, outcome)
        };

        if outcome != countdown_switch::CountdownSwitchStep::Fire {
            return Ok(());
        }

        // `sub_65370` prepares sound 41 before synchronously consuming the
        // authored group and only then disables the producer. Keep that
        // chronology in the canonical effect journal.
        mark_source_audio_call(SourceAudioCallTag::CountdownSwitchModel16);
        self.push_flight_effect(FlightEvent::PrepareSound {
            #[cfg(test)]
            audio_site: crate::MissionAudioSite::CountdownSwitch,
            entity_index: source_slot,
            player_index: -1,
            sound_id: 41,
        });
        self.consume_authored_group(pass, source_slot, target_group)?;
        self.session.state.live.economy.entities[usize::from(source_slot)].disable_drawing();
        Ok(())
    }

    fn step_authored_countdown_switch(
        &mut self,
        pass: u8,
        source_slot: u16,
        observed_creature_present: bool,
    ) -> std::result::Result<(), String> {
        let (model, target_group, outcome) = {
            let source = self
                .session
                .state
                .live
                .economy
                .economy
                .authored_switch_producers
                .iter_mut()
                .find(|source| {
                    source.active
                        && source.source_entity_index == source_slot
                        && source.source_class == 11
                        && matches!(source.model, 16 | 17)
                        && source.action == source.model
                })
                .ok_or_else(|| {
                    format!(
                        "countdown-switch callback lost authored class-11 action source slot {source_slot}"
                    )
                })?;
            let outcome = countdown_switch::step_absence_countdown(
                &mut source.dword_0x10,
                observed_creature_present,
            );
            (source.model, source.stage_tag, outcome)
        };

        if outcome != countdown_switch::CountdownSwitchStep::Fire {
            return Ok(());
        }

        mark_source_audio_call(if model == 16 {
            SourceAudioCallTag::CountdownSwitchModel16
        } else {
            SourceAudioCallTag::CountdownSwitchModel17
        });
        self.push_flight_effect(FlightEvent::PrepareSound {
            #[cfg(test)]
            audio_site: crate::MissionAudioSite::CountdownSwitch,
            entity_index: source_slot,
            player_index: -1,
            sound_id: 41,
        });
        self.consume_authored_group(pass, source_slot, target_group)?;
        self.session
            .state
            .live
            .economy
            .economy
            .complete_authored_switch_producer_slot(source_slot);
        Ok(())
    }

    /// Dispatch one reset-imported class-11 switch at its numeric source
    /// slot. This replaces the old pre-dispatch side wrapper: source player
    /// membership is rebuilt before the scan, the live selector is read at
    /// visit time, and the scheduler owns the common phase epilogue.
    fn step_imported_switch(
        &mut self,
        pass: u8,
        source_slot: u16,
        source_player_order: &[usize],
    ) -> std::result::Result<(), String> {
        let source = self
            .session
            .state
            .live
            .economy
            .entities
            .get(usize::from(source_slot))
            .filter(|source| {
                source.active
                    && source.index == source_slot
                    && source.class == 11
                    && matches!(
                        (source.model, source.action),
                        (0, 0) | (1, 1) | (4, 4) | (32, 32)
                    )
            })
            .cloned()
            .ok_or_else(|| format!("switch callback lost source slot {source_slot}"))?;

        let completed_objective_owner = (source.action == 32)
            .then(|| {
                let objective_stage_index = u8::try_from(source.level_gate).ok()?;
                if objective_stage_index >= self.session.state.live.core.runtime.stage_index {
                    return None;
                }
                self.session
                    .state
                    .live
                    .core
                    .players
                    .iter()
                    .take(1)
                    .find_map(|player| {
                        if player.active_player_index < 0 {
                            return None;
                        }
                        let color = player.player_color_index();
                        (self
                            .session
                            .state
                            .live
                            .core
                            .runtime
                            .objectives
                            .get(usize::from(color))?
                            .stage_selectors
                            .get(usize::from(objective_stage_index))
                            == Some(&2))
                        .then_some(color)
                    })
            })
            .flatten();
        let fired = match source.action {
            0 | 1 => {
                if source.phase & 7 != 0 {
                    false
                } else {
                    self.level2_model0_overlap_matches(
                        source.position,
                        i16::from_le_bytes(
                            source.record_bytes()[88..90]
                                .try_into()
                                .expect("switch render half-width extent"),
                        ),
                        i16::from_le_bytes(
                            source.record_bytes()[90..92]
                                .try_into()
                                .expect("switch render half-depth extent"),
                        ),
                        source.action == 0,
                        |entity| {
                            if entity.index == self.session.state.live.core.state.entity_index {
                                self.session.state.live.core.state.position
                            } else {
                                entity.position
                            }
                        },
                    )
                }
            }
            4 => source_player_order.iter().copied().any(|player_index| {
                let Some(player) = self.session.state.live.core.players.get(player_index) else {
                    return false;
                };
                if player.active_player_index < 0 {
                    return false;
                }
                let Some(entity) = self
                    .session
                    .state
                    .live
                    .economy
                    .entities
                    .get(usize::from(player.entity_index))
                else {
                    return false;
                };
                entity.active
                    && entity.class == 3
                    && entity.model == 0
                    && entity.life >= 0
                    && self
                        .session
                        .state
                        .live
                        .core
                        .runtime
                        .objectives
                        .get(usize::from(player.player_color_index()))
                        .is_some_and(|objective| objective.is_level_end != 0)
            }),
            32 => completed_objective_owner.is_some(),
            _ => unreachable!("switch selector was checked above"),
        };
        if !fired {
            return Ok(());
        }

        if source.action == 32 {
            // `AddSwitch0B_20_6F1C0` publishes ObjectiveDone before the
            // synchronous authored-group scan. The group constructors may
            // inspect that live mission state, so this ordering belongs to
            // the shared producer contract rather than to a Level-2 helper.
            self.session.state.live.level2_objective_pause = 1;
            self.session.state.live.level2_objective_done = true;
            if let Some(owner_color) = completed_objective_owner {
                self.session
                    .state
                    .live
                    .core
                    .runtime
                    .mark_level2_objective_done_for(owner_color);
            }
        }

        // The older group-31 helper still owns constructors that have not yet
        // been folded into the generic authored dispatcher. It is selected by
        // source data, not campaign identity; every other group uses the new
        // consume-on-attempt contract.
        if source.action == 4 && source.source_id == 31 {
            self.activate_level2_authored_group31(pass, source_slot);
        } else {
            self.consume_authored_group(pass, source_slot, source.source_id)?;
            self.session.state.live.economy.entities[usize::from(source_slot)].disable_drawing();
        }
        Ok(())
    }

    fn step_imported_terrain_strip(&mut self, source_slot: u16) -> std::result::Result<(), String> {
        let selector = self
            .session
            .state
            .live
            .economy
            .entities
            .get(usize::from(source_slot))
            .filter(|source| source.active && source.index == source_slot)
            .map(|source| (source.class, source.model, source.action))
            .ok_or_else(|| format!("terrain-strip callback lost source slot {source_slot}"))?;
        match selector {
            (14, 1, 6) => self.step_imported_terrain_strip_controller(source_slot),
            (10, 63, 68) => self.step_imported_terrain_strip_trigger(source_slot, 2),
            (10, 64, 69) => self.step_imported_terrain_strip_trigger(source_slot, 1),
            _ => Err(format!(
                "terrain-strip handler rejects source selector {}/{}/{} at slot {source_slot}",
                selector.0, selector.1, selector.2
            )),
        }
    }

    fn step_imported_terrain_strip_controller(
        &mut self,
        source_slot: u16,
    ) -> std::result::Result<(), String> {
        let (position, life_before, mut controller) = {
            let source = &self.session.state.live.economy.entities[usize::from(source_slot)];
            (
                source.position,
                source.life,
                terrain_strip::TerrainStripControllerState {
                    life: source.life,
                    subspell_index: source.subspell_index,
                    direction: source.level_gate,
                    span: source.source_dword_10(),
                },
            )
        };
        let visit = self
            .session
            .state
            .live
            .terrain
            .apply_terrain_strip_visit(position, &mut controller);
        {
            let source = &mut self.session.state.live.economy.entities[usize::from(source_slot)];
            source.set_life(controller.life);
            source.set_subspell_index(controller.subspell_index);
            source.set_source_dword_10(controller.span);
        }
        match visit.audio {
            Some(terrain_strip::TerrainStripAudioOutcome::Prepare47) => {
                mark_source_audio_call(if life_before == 1 {
                    SourceAudioCallTag::TerrainStripOpening
                } else {
                    SourceAudioCallTag::TerrainStripClosing
                });
                self.push_flight_effect(FlightEvent::PrepareSound {
                    #[cfg(test)]
                    audio_site: crate::MissionAudioSite::TerrainStrip,
                    entity_index: source_slot,
                    player_index: -1,
                    sound_id: 47,
                });
            }
            Some(terrain_strip::TerrainStripAudioOutcome::EndLoop47) => {
                self.push_flight_effect(FlightEvent::EndLoop {
                    #[cfg(test)]
                    audio_site: crate::MissionAudioSite::TerrainStrip,
                    entity_index: source_slot,
                    player_index: -1,
                    sound_id: 47,
                });
            }
            None => {}
        }
        Ok(())
    }

    fn step_imported_terrain_strip_trigger(
        &mut self,
        source_slot: u16,
        target_life: i32,
    ) -> std::result::Result<(), String> {
        let trigger_position =
            self.session.state.live.economy.entities[usize::from(source_slot)].position;
        let tile_x = trigger_position.x.wrapping_sub(128).to_be_bytes()[0];
        let tile_y = trigger_position.y.wrapping_sub(128).to_be_bytes()[0];
        let chain = self
            .session
            .state
            .live
            .economy
            .economy
            .map_chains()
            .entity_chain(tile_x, tile_y);
        let target = chain.into_iter().find(|candidate| {
            self.session.state.live.economy.entities[usize::from(*candidate)].active
                && self.session.state.live.economy.entities[usize::from(*candidate)].class == 14
                && matches!(
                    self.session.state.live.economy.entities[usize::from(*candidate)].model,
                    1 | 2
                )
        });
        if let Some(target) = target {
            self.session.state.live.economy.entities[usize::from(target)].set_life(target_life);
        }
        self.session.state.live.economy.entities[usize::from(source_slot)].disable_drawing();
        Ok(())
    }

    /// Run the production Level 2 entity pass through the literal source
    /// scheduler. The adapter owns phase placement, live resolution, registry
    /// selection, callback invocation, epilogue policy, and following cleanup;
    /// retained family methods own only their typed callback bodies.
    pub(super) fn step_level2_source_pass_with_player(
        &mut self,
        pass: u8,
        latched_command: crate::LatchedCommand,
        evidence: &mut Vec<crate::StepEvidence>,
    ) -> std::result::Result<[bool; 29], (u16, String)> {
        let player_entity_index = self.session.state.live.core.state.entity_index;
        let player_position = self.session.state.live.core.state.position;
        let mut world = Level2SourceSchedulerWorld {
            owner: self,
            pass,
            latched_command,
            evidence,
            source_player_order: Vec::new(),
            player_entity_index,
            player_position,
            goat_players: Vec::new(),
            pass_start_mana_sphere_slots: BTreeSet::new(),
            pass_start_class5_model_slots: std::array::from_fn(|_| Vec::new()),
            pass_start_class5_model_present: [false; 29],
            pass_start_castle_auxiliary_membership: Default::default(),
            pass_start_castle_root_slot: None,
            goat_begin_events: BTreeMap::new(),
        };
        run_update_entities(&mut world)
            .map(|_| world.pass_start_class5_model_present)
            .map_err(|error| match error {
                ScheduleError::UnknownDispatch { slot, selector, .. } => {
                    let message = match selector {
                        Level2ResolvedSourcePlan::Unknown { message, .. } => message,
                        selector => format!("unadmitted production selector {selector:?}"),
                    };
                    (slot, message)
                }
                ScheduleError::Adapter { slot, source, .. } => (slot.unwrap_or(0), source),
                ScheduleError::ApplyNoProgress { .. } | ScheduleError::ApplyScanLimit { .. } => (
                    0,
                    "ordinary UpdateEntities unexpectedly returned an ApplyEvents error".to_owned(),
                ),
            })
    }

    pub(super) fn step_level2_dynamic_spell_pickup(
        &mut self,
        pass: u8,
        source_slot: u16,
        player_entity_index: u16,
        player_position: Position,
        source_player_order: &[usize],
    ) -> bool {
        let Some(family) = self
            .session
            .state
            .live
            .economy
            .spell_pickups()
            .iter()
            .find(|source| source.source_entity_index == source_slot && source.active)
            .map(|source| source.family)
        else {
            return false;
        };
        let terrain = &self.session.state.live.terrain;
        match self
            .session
            .state
            .live
            .shared_spells
            .step_level2_spell_acquisition_with_economy_in_source_order(
                &mut self.session.state.live.economy,
                family,
                player_entity_index,
                player_position,
                terrain,
                source_player_order,
            ) {
            Ok(result) => self.push_dispatch_effect(Level2DispatchEvent::SpellAcquisition {
                pass,
                source_slot,
                family,
                result,
            }),
            Err(error) => self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot,
                message: error.to_string(),
            }),
        }
        if let Some(pickup) = self
            .session
            .state
            .live
            .shared_spells
            .pickup_mut(source_slot)
        {
            pickup.phase = pickup.phase.wrapping_add(1);
            pickup.record[66] = pickup.phase;
        }
        let Some((action, position)) = self
            .session
            .state
            .live
            .shared_spells
            .pickup(source_slot)
            .map(|pickup| (pickup.action, pickup.position))
        else {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot,
                message: "dynamic spell pickup lost its shared entity projection".to_owned(),
            });
            return false;
        };
        if !self
            .session
            .state
            .live
            .economy
            .sync_spell_pickup_projection(source_slot, action, position)
        {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot,
                message: "dynamic spell pickup lost its source-arena projection".to_owned(),
            });
            return false;
        }
        let remains_live = self
            .session
            .state
            .live
            .economy
            .step_spell_pickup_slot(source_slot);
        if let Err(error) = self.commit_imported_spell_source_projection(source_slot) {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot,
                message: error.to_string(),
            });
            return false;
        }
        remains_live
    }

    /// Dispatch the acquired family-3 holder through `GetScroll_69DB0` at
    /// its real numeric source slot.
    fn step_speed_holder_source_slot(&mut self, source_slot: u16) -> bool {
        let Some(holder) = self
            .session
            .state
            .live
            .shared_spells
            .pickup(source_slot)
            .filter(|holder| holder.active && holder.model == 3 && holder.action == 9)
            .cloned()
        else {
            return false;
        };
        let parent_entity_index = u16::from_le_bytes([holder.record[44], holder.record[45]]);
        let parent = self
            .session
            .state
            .live
            .shared_spells
            .player_entities
            .iter()
            .find(|entity| entity.index == parent_entity_index && entity.active)
            .cloned();
        let first_active_tick = holder.active_value == holder.activation_value.cast_signed();
        let castle_mana = self
            .session
            .state
            .live
            .economy
            .economy
            .castle
            .filter(|castle| castle.active && castle.owner_entity_index == parent_entity_index)
            .map(|castle| castle.mana);
        let source_gate_passed = parent.as_ref().is_some_and(|parent| {
            parent.life >= 0
                && parent.mana >= 0
                && (holder.mana_regen == 0
                    || castle_mana.is_some_and(|mana| mana >= holder.mana_regen))
                && (!first_active_tick || parent.mana >= holder.max_mana)
        });
        let presentation_timer = u16::from_le_bytes([holder.record[58], holder.record[59]]);
        let parent_mana_regen = parent.as_ref().map_or(0, |parent| parent.mana_regen);
        let minimum_speed = parent
            .as_ref()
            .map_or(PLAYER_MIN_SPEED, EntitySnapshot::minimum_speed);
        let transition = crate::spell_activation::step_speed_spell(
            crate::spell_activation::SpeedSpellStepInput {
                active_value: holder.active_value,
                activation_value: holder.activation_value.cast_signed(),
                holder_phase: holder.phase,
                holder_control_flags: holder.control_flags,
                presentation_timer,
                parent_valid: parent.is_some(),
                source_gate_passed,
                speed_changed: self.session.state.live.core.state.speed_changed_flag != 0,
                direction: if self.session.state.live.core.state.target_speed < 0 {
                    -1
                } else {
                    1
                },
                minimum_speed,
                first_multiplier: i16::try_from(holder.subspell_index)
                    .unwrap_or(i16::MAX)
                    .wrapping_add(1),
                sustain_multiplier: i16::try_from(holder.subspell_index).unwrap_or(i16::MAX),
                parent_mana_regen,
                mana_cost: holder.max_mana,
                latch_clear_offset: if parent.as_ref().is_some_and(|parent| parent.model == 1) {
                    64
                } else {
                    2
                },
            },
        );

        if transition.prepare_sound_19 {
            mark_source_audio_call(SourceAudioCallTag::SpeedSpellActivation);
            self.session
                .state
                .live
                .shared_spells
                .emit_flight_effect(FlightEvent::PrepareSound {
                    #[cfg(test)]
                    audio_site: crate::MissionAudioSite::SpeedSpellActivation,
                    entity_index: parent_entity_index,
                    player_index: -1,
                    sound_id: 19,
                });
        }
        if transition.award_experience {
            self.session
                .state
                .live
                .shared_spells
                .award_speed_experience(parent_entity_index, 1);
            if let Some(source_player) = self
                .session
                .state
                .live
                .shared_spells
                .players
                .iter()
                .find(|player| player.entity_index == parent_entity_index)
                .cloned()
                && let Some(player) = self
                    .session
                    .state
                    .live
                    .core
                    .players
                    .iter_mut()
                    .find(|player| player.entity_index == parent_entity_index)
            {
                player.sync_spell_progression_family_from(&source_player, 3);
            }
            if let Some(fire) = self.session.state.live.core.context.spell_fire.as_mut() {
                fire.spell_experience[3] =
                    self.session.state.live.shared_spells.spell_experience[3];
                fire.spell_levels[3] = self.session.state.live.shared_spells.spell_levels[3];
            }
        }
        if let Some(speed) = transition.requested_speed {
            self.session.state.live.core.state.target_speed = speed;
        }
        if let Some(speed) = transition.actual_speed {
            self.session.state.live.core.state.actual_speed = speed;
            if let Some(entity) = self
                .session
                .state
                .live
                .economy
                .entities
                .iter_mut()
                .find(|entity| entity.index == parent_entity_index)
            {
                entity.set_actual_speed(speed);
            }
        }
        if transition.request_trail
            && let Some(parent) = parent.as_ref()
        {
            let position = self.session.state.live.core.state.position;
            let _ = self
                .session
                .state
                .live
                .economy
                .economy
                .allocate_speed_trail(position, parent.source_id);
        }
        if let Some(mana_regen) = transition.parent_mana_regen {
            self.session
                .state
                .live
                .shared_spells
                .player_entity
                .set_mana_regen(mana_regen);
            for entity in &mut self.session.state.live.shared_spells.player_entities {
                if entity.index == parent_entity_index {
                    entity.set_mana_regen(mana_regen);
                }
            }
            if let Some(entity) = self
                .session
                .state
                .live
                .economy
                .entities
                .iter_mut()
                .find(|entity| entity.index == parent_entity_index)
            {
                entity.set_mana_regen(mana_regen);
            }
        }
        if let Some(authoritative) = self
            .session
            .state
            .live
            .shared_spells
            .pickup_mut(source_slot)
        {
            authoritative.active_value = transition.active_value;
            authoritative.record[50..52].copy_from_slice(&transition.active_value.to_le_bytes());
            authoritative.set_control_flags(transition.holder_control_flags);
            authoritative.record[58..60]
                .copy_from_slice(&transition.presentation_timer.to_le_bytes());
            if transition.expired {
                let queued_tier =
                    u16::from_le_bytes([authoritative.record[48], authoritative.record[49]]);
                if queued_tier != 0
                    && let Some(profile) = level2_set_spell_profile(3, (queued_tier - 1) as u8)
                {
                    authoritative.record[48..50].fill(0);
                    authoritative.apply_nyphur_spell_profile(profile, (queued_tier - 1) as u8);
                }
            }
        }
        if let Some(speed_holder) = self
            .session
            .state
            .live
            .core
            .context
            .speed_spell_holder
            .as_mut()
            .filter(|holder| holder.entity_index == source_slot)
        {
            speed_holder.active_value = transition.active_value;
        }
        if let Some(fire) = self.session.state.live.core.context.spell_fire.as_mut()
            && let Some(holder) = fire
                .holders
                .iter_mut()
                .find(|holder| holder.entity_index == source_slot)
        {
            holder.active_value = transition.active_value;
        }
        true
    }

    fn step_dynamic_speed_holder(&mut self, pass: u8, source_slot: u16) -> bool {
        if !self.step_speed_holder_source_slot(source_slot) {
            return false;
        }
        if let Some(holder) = self
            .session
            .state
            .live
            .shared_spells
            .pickup_mut(source_slot)
        {
            holder.phase = holder.phase.wrapping_add(1);
            holder.record[66] = holder.phase;
        }
        let Some((action, position)) = self
            .session
            .state
            .live
            .shared_spells
            .pickup(source_slot)
            .map(|holder| (holder.action, holder.position))
        else {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot,
                message: "dynamic Speed holder lost its shared entity projection".to_owned(),
            });
            return false;
        };
        if !self
            .session
            .state
            .live
            .economy
            .sync_spell_pickup_projection(source_slot, action, position)
        {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot,
                message: "dynamic Speed holder lost its source-arena projection".to_owned(),
            });
            return false;
        }
        let remains_live = self
            .session
            .state
            .live
            .economy
            .step_spell_pickup_slot(source_slot);
        if let Err(error) = self.commit_imported_spell_source_projection(source_slot) {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot,
                message: error.to_string(),
            });
            return false;
        }
        remains_live
    }

    pub(super) fn step_level2_dynamic_spell_replacement(
        &mut self,
        pass: u8,
        source_slot: u16,
        player_entity_index: u16,
        player_position: Position,
        source_player_order: &[usize],
    ) -> bool {
        let Some((family, active)) = self
            .session
            .state
            .live
            .economy
            .spell_replacements()
            .iter()
            .find(|replacement| replacement.source_entity_index == source_slot)
            .map(|replacement| (replacement.family, replacement.active))
        else {
            return false;
        };
        if !active {
            return false;
        }
        // The dynamic source remains the same class-15 row after pickup; its
        // live action selects either another persistent-pickup transition or
        // the family's active callback. The source-specific wrapper preserves
        // that row's cadence/lifecycle state for both roles.
        let terrain = &self.session.state.live.terrain;
        match self
            .session
            .state
            .live
            .shared_spells
            .step_level2_spell_replacement_acquisition_with_economy_in_source_order(
                &mut self.session.state.live.economy,
                source_slot,
                family,
                player_entity_index,
                player_position,
                terrain,
                source_player_order,
            ) {
            Ok(result) => self.push_dispatch_effect(Level2DispatchEvent::SpellAcquisition {
                pass,
                source_slot,
                family,
                result,
            }),
            Err(error) => self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot,
                message: error.to_string(),
            }),
        }
        if let Some(pickup) = self
            .session
            .state
            .live
            .shared_spells
            .pickup_mut(source_slot)
        {
            pickup.phase = pickup.phase.wrapping_add(1);
            pickup.record[66] = pickup.phase;
        }
        self.session
            .state
            .live
            .economy
            .step_spell_replacement_slot(source_slot)
    }

    pub(super) fn step_level2_authored_class14(&mut self, source_slot: u16) -> bool {
        let Some((model, position)) = self
            .session
            .state
            .live
            .economy
            .authored_class14()
            .iter()
            .find(|source| source.active && source.source_entity_index == source_slot)
            .map(|source| (source.model, source.position))
        else {
            return false;
        };
        let ground_height = self.canonical_terrain().ground_height(position);
        if !self
            .session
            .state
            .live
            .economy
            .step_authored_class14_slot(source_slot, ground_height)
        {
            return false;
        }
        let Some(snapped_position) = self
            .session
            .state
            .live
            .economy
            .authored_class14()
            .iter()
            .find(|source| source.active && source.source_entity_index == source_slot)
            .map(|source| source.position)
        else {
            return false;
        };
        let collected = model == 5
            && self
                .session
                .state
                .live
                .shared_spells
                .collect_level2_experience_scroll(
                    source_slot,
                    snapped_position,
                    self.session.state.live.core.runtime.local_player_slot,
                    self.session.state.live.core.state.entity_index,
                    self.session.state.live.core.state.position,
                    self.session
                        .state
                        .live
                        .economy
                        .entities
                        .iter()
                        .find(|entity| {
                            entity.index == self.session.state.live.core.state.entity_index
                        })
                        .map_or(0, |entity| entity.life),
                    &self.session.state.live.economy.entities,
                );
        if collected {
            self.session
                .state
                .live
                .economy
                .deactivate_authored_class14_slot(source_slot);
            false
        } else {
            true
        }
    }

    /// Run the source `AddTree02_00_64E20` callback.  The owner/life/sphere
    /// branch is deliberately committed in this order: failed sphere
    /// allocation clears the owner mailbox but does not advance the tree RNG,
    /// while a successful sphere allocation is the only path that consumes
    /// RNG-L2-0464/0465.
    pub(super) fn step_level2_authored_tree(&mut self, source_slot: u16) -> bool {
        let Some(source_index) = self
            .session
            .state
            .live
            .economy
            .authored_trees
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.session.state.live.economy.authored_trees[source_index];
        source.control_flags |= 0x0002_0000;
        if source.owner_entity_index != 0 {
            source.life = source.life.wrapping_sub(source.pending_damage);
            if source.life < 0
                && let Some(sphere) = self.session.state.live.economy.allocate_authored_model39(
                    source.authored_index,
                    source_slot,
                    source.position,
                )
            {
                let owner_id = self
                    .session
                    .state
                    .live
                    .economy
                    .entities
                    .get(usize::from(source.owner_entity_index))
                    .map_or(source.owner_entity_index, |entity| entity.source_id);
                let sphere_word_0x2c =
                    i16::try_from((i32::from(source.fov) * 3) >> 2).expect("tree sphere word fits");
                let sphere_position_z = if sphere.position.z <= 128 {
                    0
                } else {
                    sphere.position.z - 128
                };
                if let Some(sphere_row) = self
                    .session
                    .state
                    .live
                    .economy
                    .spheres
                    .iter_mut()
                    .find(|row| row.source_entity_index == sphere.source_entity_index)
                {
                    sphere_row.id = u32::from(owner_id);
                    sphere_row.position.z = sphere_position_z;
                }
                source.last_sphere_source_entity_index = sphere.source_entity_index;
                source.last_sphere_word_0x2c = sphere_word_0x2c;
                source.last_sphere_position_z = sphere_position_z;
                source.entity_rng = level2_next_rng16(source.entity_rng);
                source.life = i32::from(source.entity_rng % 0x3c) + 130;
                source.control_flags &= !0x0002_0008;
                source.action = 1;
                source.control_flags |= 0x0002_0000;
            }
            source.owner_entity_index = 0;
        }
        let terrain = &self.session.state.live.terrain;
        source.position.z = terrain.ground_height(source.position);
        if terrain.material_mask(source.position) == crate::WATER_AMBIENCE_MASK {
            if let Some(splash) = self
                .session
                .state
                .live
                .economy
                .allocate_authored_model5_water_splash(source.position, terrain)
                && let Some(splash) = self
                    .session
                    .state
                    .live
                    .economy
                    .authored_model5_water_splashes
                    .iter_mut()
                    .find(|row| row.source_entity_index == splash.source_entity_index)
            {
                splash.entity_id = source.entity_id;
            }
            source.drawing_enabled = false;
        }
        source.phase = source.phase.wrapping_add(1);
        self.session
            .state
            .live
            .economy
            .source_rng
            .insert(source_slot, source.entity_rng);
        self.session.state.live.economy.authored_trees[source_index] = source;
        true
    }

    pub(super) fn step_level2_authored_switch_producer(
        &mut self,
        pass: u8,
        source_slot: u16,
    ) -> bool {
        let Some((position, word_10)) = self
            .session
            .state
            .live
            .economy
            .authored_switch_producers()
            .iter()
            .find(|source| source.active && source.source_entity_index == source_slot)
            .map(|source| (source.position, source.word_10))
        else {
            return false;
        };
        let switch_kind = self.switch_kind_for_source_slot(source_slot);
        if switch_kind == Some(SwitchKind::ExitSequenceTrigger)
            && (self.session.state.live.level_exit.active
                || self.session.state.live.level_exit.complete)
        {
            return false;
        }
        let switch_extent = word_10.cast_unsigned().wrapping_shl(8).cast_signed();
        let overlap = self.level2_model0_overlap_matches(
            position,
            switch_extent,
            switch_extent,
            matches!(
                switch_kind,
                Some(SwitchKind::AreaEntryTrigger | SwitchKind::ExitSequenceTrigger)
            ),
            |entity| {
                if entity.index == self.session.state.live.core.state.entity_index {
                    self.session.state.live.core.state.position
                } else {
                    entity.position
                }
            },
        );
        let Some(target_group) = self
            .session
            .state
            .live
            .economy
            .step_authored_switch_producer_slot(source_slot, overlap)
        else {
            return false;
        };
        if switch_kind == Some(SwitchKind::ExitSequenceTrigger) {
            let portal_entity_index = self
                .session
                .state
                .live
                .economy
                .economy
                .authored_class14()
                .iter()
                .find(|source| {
                    source.active
                        && source.model == 3
                        && self.scenery_kind_for_source_slot(source.source_entity_index)
                            == Some(SceneryKind::ExitPortal)
                })
                .map_or(0, |source| source.source_entity_index);
            let player_entity_index = self.session.state.live.core.state.entity_index;
            let player_is_present = self
                .session
                .state
                .live
                .economy
                .entities
                .get(usize::from(player_entity_index))
                .is_some_and(|player| {
                    player.index == player_entity_index && player.class == 3 && player.model == 0
                });
            if !player_is_present {
                return false;
            }
            if !self
                .session
                .state
                .live
                .level_exit
                .start(source_slot, portal_entity_index)
            {
                return false;
            }
            if portal_entity_index != 0
                && !self
                    .session
                    .state
                    .live
                    .economy
                    .reveal_authored_class14_slot(portal_entity_index)
            {
                return false;
            }
            let Some(player) = self
                .session
                .state
                .live
                .economy
                .entities
                .get_mut(usize::from(player_entity_index))
                .filter(|player| {
                    player.index == player_entity_index && player.class == 3 && player.model == 0
                })
            else {
                unreachable!("the exit trigger preflighted the authoritative player row");
            };
            player.set_source_action(crate::level_exit::EXIT_PLAYER_ACTION);
            player.set_source_byte_46(0);
            mark_source_audio_call(SourceAudioCallTag::ExitOverlapSwitch);
            self.push_flight_effect(FlightEvent::PrepareSound {
                #[cfg(test)]
                audio_site: crate::MissionAudioSite::ExitOverlap,
                entity_index: player_entity_index,
                player_index: -1,
                sound_id: crate::level_exit::EXIT_SWITCH_SOUND,
            });
            self.push_dispatch_effect(Level2DispatchEvent::LevelExitSequenceStarted {
                pass,
                trigger_entity_index: source_slot,
                portal_entity_index,
            });
            return self
                .session
                .state
                .live
                .economy
                .complete_authored_switch_producer_slot(source_slot);
        }
        if let Err(error) = self.consume_authored_group(pass, source_slot, target_group) {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot,
                message: error,
            });
            return false;
        }
        self.session
            .state
            .live
            .economy
            .complete_authored_switch_producer_slot(source_slot)
    }

    fn step_level_exit_player_action(
        &mut self,
        pass: u8,
        source_slot: u16,
    ) -> std::result::Result<(), String> {
        if !self.session.state.live.level_exit.active
            && !self.session.state.live.level_exit.complete
        {
            return Err(format!(
                "player source slot {source_slot} selected exit action 12 without an active semantic exit sequence"
            ));
        }

        let portal_entity_index = self.session.state.live.level_exit.portal_entity_index;
        let portal_position = self
            .session
            .state
            .live
            .economy
            .economy
            .authored_class14()
            .iter()
            .find(|portal| {
                portal.active
                    && portal.source_entity_index == portal_entity_index
                    && self.scenery_kind_for_source_slot(portal.source_entity_index)
                        == Some(SceneryKind::ExitPortal)
            })
            .map(|portal| portal.position);
        let phase_before = self.session.state.live.level_exit.phase;
        let speed_holder_index = self.session.state.live.core.state.speed_spell_holder_index;
        let effects = {
            let live = &mut self.session.state.live;
            live.level_exit.advance(
                &live.terrain,
                self.session.definition.profile,
                &mut live.core.state,
                &mut live.core.context,
                portal_position,
            )
        };

        if phase_before == 0 && speed_holder_index != 0 {
            if let Some(holder) = self
                .session
                .state
                .live
                .shared_spells
                .pickup_mut(speed_holder_index)
            {
                holder.active_value = 0;
                holder.record[50..52].fill(0);
            }
            if let Some(fire) = self.session.state.live.core.context.spell_fire.as_mut()
                && let Some(holder) = fire
                    .holders
                    .iter_mut()
                    .find(|holder| holder.entity_index == speed_holder_index)
            {
                holder.active_value = 0;
            }
        }

        if let Some(effect_state) = self.session.state.live.core.context.effects.as_mut() {
            effect_state.palette_submode = self.session.state.live.level_exit.palette_submode;
        }
        let player_entity_index = self.session.state.live.core.state.entity_index;
        let player_position = self.session.state.live.core.state.position;
        let player_yaw = self.session.state.live.core.state.yaw;
        let player_speed = self.session.state.live.core.state.actual_speed;
        let exit_phase = self.session.state.live.level_exit.phase;
        for player in self
            .session
            .state
            .live
            .economy
            .entities
            .iter_mut()
            .filter(|player| player.index == player_entity_index)
        {
            player.set_position(player_position);
            player.set_yaw(player_yaw);
            player.set_actual_speed(player_speed);
            player.set_source_action(crate::level_exit::EXIT_PLAYER_ACTION);
            player.set_source_byte_46(exit_phase);
        }
        for player in std::iter::once(&mut self.session.state.live.shared_spells.player_entity)
            .chain(
                self.session
                    .state
                    .live
                    .shared_spells
                    .player_entities
                    .iter_mut(),
            )
            .filter(|player| player.index == player_entity_index)
        {
            player.set_position(player_position);
            player.set_yaw(player_yaw);
            player.set_actual_speed(player_speed);
            player.set_source_action(crate::level_exit::EXIT_PLAYER_ACTION);
            player.set_source_byte_46(exit_phase);
        }
        self.session
            .state
            .live
            .economy
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: player_entity_index,
                active: true,
                x: player_position.x,
                y: player_position.y,
            })?;

        for effect in effects {
            match effect {
                crate::level_exit::LevelExitEffect::PrepareSound { sound_id } => {
                    if sound_id == crate::level_exit::EXIT_SWITCH_SOUND {
                        mark_source_audio_call(SourceAudioCallTag::ExitSequenceSwitch);
                    } else {
                        mark_source_audio_call(SourceAudioCallTag::ExitAcceleration);
                    }
                    self.push_flight_effect(FlightEvent::PrepareSound {
                        #[cfg(test)]
                        audio_site: if sound_id == crate::level_exit::EXIT_SWITCH_SOUND {
                            crate::MissionAudioSite::ExitSequenceSwitch
                        } else if exit_phase == 7 {
                            crate::MissionAudioSite::ExitAccelerationPhase6
                        } else {
                            crate::MissionAudioSite::ExitAccelerationPhase8
                        },
                        entity_index: player_entity_index,
                        player_index: -1,
                        sound_id,
                    });
                }
                crate::level_exit::LevelExitEffect::Completed { result_code } => {
                    self.push_dispatch_effect(Level2DispatchEvent::LevelExitSequenceCompleted {
                        pass,
                        player_entity_index,
                        result_code,
                    });
                }
            }
        }
        Ok(())
    }

    /// Apply the cadence-gated target mailbox half of remc2 `sub_33710`.
    /// The wind parent movement is already committed by the economy callback;
    /// this seam owns only the source collision-box test and the two target
    /// mailbox branches. `sub_33340` remains a separate movement/effect pass.
    pub(super) fn step_level2_authored_model22_wind_pulse(&mut self, source_slot: u16) {
        let Some(parent) = self
            .session
            .state
            .live
            .economy
            .authored_model22()
            .iter()
            .find(|source| source.active && source.source_entity_index == source_slot)
            .copied()
        else {
            return;
        };
        // Economy stepping advances byte 0x3E after the callback. Recover the
        // callback's pre-increment phase for `byte_0x3E_62 & 7 == 0`.
        if parent.phase.wrapping_sub(1) & 7 != 0 {
            return;
        }
        let local_player_index = self.session.state.live.core.state.entity_index;
        let local_player_position = self.session.state.live.core.state.position;
        let target_position = |entity: &EntitySnapshot| {
            if entity.index == local_player_index {
                local_player_position
            } else {
                entity.position
            }
        };
        let overlaps = |entity: &EntitySnapshot| {
            level2_compare_axis_with_shift(
                parent.position,
                parent.shift_rot_x,
                parent.shift_rot_y,
                target_position(entity),
                entity.array_pitch(),
                entity.array_roll(),
            )
        };
        for entity in self
            .session
            .state
            .live
            .economy
            .entities
            .iter_mut()
            .filter(|entity| entity.active && entity.class == 10 && entity.model == 45)
        {
            if overlaps(entity) {
                entity.apply_wind_model45_mailbox(parent.entity_id, parent.subspell_index);
            }
        }
        for entity in self
            .session
            .state
            .live
            .economy
            .entities
            .iter_mut()
            .filter(|entity| {
                entity.active && entity.life >= 0 && entity.class == 3 && entity.model == 2
            })
        {
            if overlaps(entity) {
                entity.apply_wind_model2_mailbox(
                    parent.source_entity_index,
                    parent.entity_id,
                    parent.subspell_index,
                );
            }
        }
    }

    /// Apply the shared `sub_10C80(a2 = 0)` damage pulse emitted by a
    /// class-10/model-0 impact. Imported rows retain their lossless mailbox;
    /// typed Castle, Bee, and Firefly rows receive the same additive owner/payload
    /// write through their authoritative carriers.
    fn apply_level2_class10_model0_damage(
        &mut self,
        step: level2_dynamic_economy::Level2Class10Model0Step,
    ) {
        let overlaps_raw = |target: &EntitySnapshot| {
            let target_position = if target.index == self.session.state.live.core.state.entity_index
            {
                self.session.state.live.core.state.position
            } else {
                target.position
            };
            crate::level1_mission::source_entities_overlap(
                step.position,
                LEVEL2_FIREBALL_COLLISION_HALF_WIDTH,
                LEVEL2_FIREBALL_COLLISION_VERTICAL_OFFSET,
                LEVEL2_FIREBALL_COLLISION_HALF_HEIGHT,
                target_position,
                target.array_pitch(),
                target.array_yaw(),
                target.array_fov(),
            )
        };

        let class3_model2 = self
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .filter(|target| {
                target.active
                    && target.life >= 0
                    && target.class == 3
                    && target.model == 2
                    && target.source_id != step.owner_entity_index
                    && overlaps_raw(target)
            })
            .map(|target| target.index)
            .collect::<Vec<_>>();
        for target_slot in class3_model2 {
            if let Some(target) = self
                .session
                .state
                .live
                .economy
                .entities
                .get_mut(usize::from(target_slot))
            {
                target.apply_model76_mailbox(step.owner_entity_index, step.payload);
            }
        }

        if let Some(castle) = self.session.state.live.economy.castle().filter(|castle| {
            castle.active
                && castle.life >= 0
                && castle.source_id_entity_index != step.owner_entity_index
                && crate::level1_mission::source_entities_overlap(
                    step.position,
                    LEVEL2_FIREBALL_COLLISION_HALF_WIDTH,
                    LEVEL2_FIREBALL_COLLISION_VERTICAL_OFFSET,
                    LEVEL2_FIREBALL_COLLISION_HALF_HEIGHT,
                    castle.position,
                    castle.shift_pitch,
                    castle.shift_yaw,
                    castle.shift_fov,
                )
        }) {
            let _ = self.session.state.live.economy.submit_damage(
                castle.source_entity_index,
                i32::from(step.payload),
                step.owner_entity_index,
            );
        }

        let bee_slots = self
            .session
            .state
            .live
            .economy
            .authored_bees()
            .iter()
            .filter(|bee| {
                bee.active
                    && bee.drawing_enabled
                    && bee.life >= 0
                    && bee.entity_id != step.owner_entity_index
                    && crate::level1_mission::source_entities_overlap(
                        step.position,
                        LEVEL2_FIREBALL_COLLISION_HALF_WIDTH,
                        LEVEL2_FIREBALL_COLLISION_VERTICAL_OFFSET,
                        LEVEL2_FIREBALL_COLLISION_HALF_HEIGHT,
                        bee.position,
                        LEVEL2_BEE_COLLISION_HALF_EXTENT,
                        0,
                        LEVEL2_BEE_COLLISION_HALF_EXTENT,
                    )
            })
            .map(|bee| bee.source_entity_index)
            .collect::<Vec<_>>();
        for target_slot in bee_slots {
            let _ = self.session.state.live.economy.submit_authored_bee_damage(
                target_slot,
                i32::from(step.payload),
                step.owner_entity_index,
            );
        }

        let firefly_slots = self
            .session
            .state
            .live
            .economy
            .authored_class5()
            .iter()
            .filter(|target| {
                target.active
                    && target.drawing_enabled
                    && target.life >= 0
                    && target.model == level2_class5::firefly::FIREFLY_MODEL
                    && target.source_entity_index != step.owner_entity_index
                    && crate::level1_mission::source_entities_overlap(
                        step.position,
                        LEVEL2_FIREBALL_COLLISION_HALF_WIDTH,
                        LEVEL2_FIREBALL_COLLISION_VERTICAL_OFFSET,
                        LEVEL2_FIREBALL_COLLISION_HALF_HEIGHT,
                        target.position,
                        target.shift_rot_x,
                        0,
                        target.shift_rot_y,
                    )
            })
            .map(|target| target.source_entity_index)
            .collect::<Vec<_>>();
        for target_slot in firefly_slots {
            let _ = self
                .session
                .state
                .live
                .economy
                .submit_authored_firefly_damage(
                    target_slot,
                    i32::from(step.payload),
                    step.owner_entity_index,
                );
        }

        let generic_slots = source_map_search_slots(
            &self.session.state.live.economy.map_chains,
            step.position,
            &source_search_offsets(1),
        );
        for target_slot in generic_slots {
            let hit = self
                .session
                .state
                .live
                .economy
                .entities
                .get(usize::from(target_slot))
                .is_some_and(|target| {
                    target.active
                        && target.life >= 0
                        && target.source_id != step.owner_entity_index
                        && target.source_byte_38() & 1 != 0
                        && target.control_flags & 8 != 0
                        && (target.class != 3 || target.model != 2)
                        && overlaps_raw(target)
                });
            if hit
                && let Some(target) = self
                    .session
                    .state
                    .live
                    .economy
                    .entities
                    .get_mut(usize::from(target_slot))
            {
                target.apply_model76_mailbox(step.owner_entity_index, step.payload);
            }
        }
    }

    /// Feed one already-produced class-10/model-0 callback result through the
    /// real overlap/mailbox adapter. The distinct class-9 -> class-10 numeric
    /// lifecycle is covered separately; this seam keeps the Level-1 package
    /// test focused on the Firefly target and death consumer.
    #[cfg(test)]
    pub(crate) fn apply_fireball_impact_to_firefly_for_test(
        &mut self,
        position: Position,
        owner_entity_index: u16,
        payload: u16,
    ) {
        self.apply_level2_class10_model0_damage(level2_dynamic_economy::Level2Class10Model0Step {
            live: true,
            first_impact: true,
            damage_pulse: true,
            owner_entity_index,
            payload,
            position,
        });
    }

    /// Complete `sub_33C00`/`sub_10C80(a2 = 0)` owner path for authored
    /// Fire-Spheres. Each column-zero child scans the model-2 list, the
    /// source-exact MSPRD-backed model-45 list, then the generic map chains.
    /// One sound-3 event follows that child's scans iff any target was hit.
    pub(super) fn step_level2_authored_model76_collision_effect(&mut self, source_slot: u16) {
        let Some(parent) = self
            .session
            .state
            .live
            .economy
            .authored_model76()
            .iter()
            .find(|source| source.active && source.source_entity_index == source_slot)
            .copied()
        else {
            return;
        };
        let source_children = self
            .session
            .state
            .live
            .economy
            .model76_children()
            .iter()
            .filter(|child| {
                child.active && child.parent_entity_index == source_slot && child.column_index == 0
            })
            .copied()
            .collect::<Vec<_>>();
        if source_children.is_empty() {
            return;
        }
        let local_player_index = self.session.state.live.core.state.entity_index;
        let local_player_position = self.session.state.live.core.state.position;
        for child in source_children {
            let mut hit_count = 0_u16;
            let model2_slots = self
                .session
                .state
                .live
                .economy
                .entities
                .iter()
                .filter(|entity| {
                    entity.active
                        && entity.class == 3
                        && entity.model == 2
                        && entity.source_id != child.entity_id
                })
                .map(|entity| entity.index)
                .collect::<Vec<_>>();
            for target_index in model2_slots {
                let hit = self
                    .session
                    .state
                    .live
                    .economy
                    .entities
                    .get(usize::from(target_index))
                    .is_some_and(|target| {
                        let target_position = if target.index == local_player_index {
                            local_player_position
                        } else {
                            target.position
                        };
                        level2_model76_sub106c0_matches(
                            child.position,
                            child.collision_yaw,
                            child.collision_pitch,
                            child.collision_roll,
                            child.collision_fov,
                            target_position,
                            target.array_yaw(),
                            target.array_pitch(),
                            target.array_roll(),
                            target.array_fov(),
                        )
                    });
                if hit {
                    if let Some(target) = self
                        .session
                        .state
                        .live
                        .economy
                        .entities
                        .get_mut(usize::from(target_index))
                    {
                        target.apply_model76_mailbox(child.entity_id, parent.subspell_index);
                        hit_count = hit_count.wrapping_add(1);
                    }
                }
            }

            let model45_slots = self
                .session
                .state
                .live
                .economy
                .entities
                .iter()
                .filter(|entity| entity.active && entity.class == 10 && entity.model == 45)
                .map(|entity| entity.index)
                .collect::<Vec<_>>();
            for target_index in model45_slots {
                let hit = self
                    .session
                    .state
                    .live
                    .economy
                    .entities
                    .get(usize::from(target_index))
                    .is_some_and(|target| {
                        let selector = u16::from(target.level_gate.cast_unsigned());
                        level2_compare_axis_with_shift(
                            child.position,
                            child.collision_pitch,
                            child.collision_roll,
                            target.position,
                            target.array_pitch(),
                            target.array_roll(),
                        ) && self
                            .session
                            .definition
                            .collision_masks
                            .as_ref()
                            .is_some_and(|bank| {
                                level2_model76_model45_occupancy(bank, selector, child.position)
                            })
                    });
                if hit {
                    if let Some(target) = self
                        .session
                        .state
                        .live
                        .economy
                        .entities
                        .get_mut(usize::from(target_index))
                    {
                        target.apply_model76_mailbox(child.entity_id, parent.subspell_index);
                        hit_count = hit_count.wrapping_add(1);
                    }
                }
            }

            let generic_slots =
                level2_model76_map_search_slots(&self.session.state.live.economy.map_chains, child);
            for target_index in generic_slots {
                let hit = self
                    .session
                    .state
                    .live
                    .economy
                    .entities
                    .get(usize::from(target_index))
                    .is_some_and(|target| {
                        let target_position = if target.index == local_player_index {
                            local_player_position
                        } else {
                            target.position
                        };
                        target.active
                            && target.source_id != child.entity_id
                            && (target.class != 3 || target.model != 2)
                            && target.source_byte_38() & 1 != 0
                            && target.control_flags & 8 != 0
                            && (target.class != 10 || target.model != 45)
                            && level2_model76_sub106c0_matches(
                                child.position,
                                child.collision_yaw,
                                child.collision_pitch,
                                child.collision_roll,
                                child.collision_fov,
                                target_position,
                                target.array_yaw(),
                                target.array_pitch(),
                                target.array_roll(),
                                target.array_fov(),
                            )
                    });
                if hit {
                    if let Some(target) = self
                        .session
                        .state
                        .live
                        .economy
                        .entities
                        .get_mut(usize::from(target_index))
                    {
                        target.apply_model76_mailbox(child.entity_id, parent.subspell_index);
                        hit_count = hit_count.wrapping_add(1);
                    }
                }
            }

            if hit_count != 0 {
                self.session.state.live.economy.events.push(
                    Level2EconomyEvent::FireSphereCollisionSound {
                        source_entity_index: parent.source_entity_index,
                        sound_id: 3,
                    },
                );
            }
        }
    }

    /// Bounded eligible-target branch of remc2 `sub_33340`.
    ///
    /// The source scans map-cell candidates, applies `sub_33810`, then runs a
    /// distance-banded movement/hit callback for the admitted class-2,
    /// class-3, class-5, and class-10 targets. Model 0 retains its
    /// linked-player roll/speed feedback; all other models use the source's
    /// wider turn/RNG bands.
    pub(super) fn step_level2_authored_model22_sub33340_eligible_targets(
        &mut self,
        source_slot: u16,
    ) {
        let Some(parent) = self
            .session
            .state
            .live
            .economy
            .authored_model22()
            .iter()
            .find(|source| source.active && source.source_entity_index == source_slot)
            .copied()
        else {
            return;
        };
        let search_offsets = source_search_offsets(12);
        let candidates = source_map_search_slots(
            &self.session.state.live.economy.map_chains,
            parent.position,
            &search_offsets,
        );
        let targets = candidates
            .into_iter()
            .filter_map(|entity_index| {
                self.session
                    .state
                    .live
                    .economy
                    .entities
                    .get(usize::from(entity_index))
            })
            .filter(|entity| {
                entity.active
                    && matches!(entity.class, 2 | 3 | 5 | 10)
                    && level2_wind_sub33810_accepts(parent.entity_id, entity)
            })
            .map(|entity| entity.index)
            .collect::<Vec<_>>();
        for target_index in targets {
            let Some(snapshot) = self
                .session
                .state
                .live
                .economy
                .entities
                .get(usize::from(target_index))
                .cloned()
            else {
                continue;
            };
            let target_position = if target_index == self.session.state.live.core.state.entity_index
            {
                self.session.state.live.core.state.position
            } else {
                snapshot.position
            };
            let distance = level2_wrapping_target_distance(parent.axis_position, target_position);
            let airborne = snapshot.control_flags & 0x1000_0000 != 0;
            let model_zero = snapshot.model == 0;
            let turn_delta = if model_zero { 56 } else { 204 };
            let latch_band = if model_zero { 384_u16 } else { 768_u16 };
            let mut predicted = target_position;
            let mut hit = false;
            let mut move_speed = 0_i16;
            // Source `sub_33340` carries `v37` as the optional descent
            // adjustment passed to `sub_580E0`. It is reset for every
            // candidate and populated only by the far-distance airborne
            // branch; all other eligible-target branches pass zero.
            let mut settle_descent = 0_i16;
            if distance >= 3_211_264 {
                if airborne {
                    let control_flags = snapshot.control_flags | 0x0000_0800;
                    let yaw = wrap_source_angle(snapshot.yaw.wrapping_add(turn_delta));
                    settle_descent = snapshot
                        .parameter_relation
                        .and_then(|index| {
                            self.session
                                .definition
                                .presentation
                                .entity_parameters
                                .get(usize::from(index))
                        })
                        .map_or(0, |parameter| {
                            i16::from_le_bytes([parameter[14], parameter[15]])
                        });
                    if distance >= 5_308_416 {
                        self.session.state.live.economy.entities[usize::from(target_index)]
                            .set_control_flags(control_flags & !0x1000_0000);
                    } else {
                        self.session.state.live.economy.entities[usize::from(target_index)]
                            .set_control_flags(control_flags);
                    }
                    self.session.state.live.economy.entities[usize::from(target_index)]
                        .set_yaw(yaw);
                    move_speed = 64;
                    hit = true;
                }
            } else {
                let desired_yaw =
                    i16::try_from(heading_for_positions(parent.axis_position, target_position))
                        .expect("wind target heading fits");
                if airborne {
                    let control_flags = snapshot.control_flags | 0x0000_0800;
                    let yaw = wrap_source_angle(snapshot.yaw.wrapping_add(turn_delta));
                    predicted.z = predicted.z.wrapping_add(114);
                    self.session.state.live.economy.entities[usize::from(target_index)]
                        .set_control_flags(control_flags);
                    self.session.state.live.economy.entities[usize::from(target_index)]
                        .set_yaw(yaw);
                    move_speed = 128;
                    hit = true;
                } else {
                    if model_zero {
                        if let EntitySpecialRelation::PlayerState(player_slot) =
                            snapshot.special_relation
                        {
                            if let Some(player) = self
                                .session
                                .state
                                .live
                                .core
                                .players
                                .iter_mut()
                                .find(|player| player.slot == player_slot)
                            {
                                let roll = player.roll_0x155();
                                if roll < 256 {
                                    player.set_roll_0x155(roll.saturating_add(28));
                                }
                            }
                        }
                        self.session.state.live.economy.entities[usize::from(target_index)]
                            .set_actual_speed(80);
                    }
                    let yaw = if distance >= 0x40000 {
                        let yaw = wrap_source_angle(desired_yaw.wrapping_add(591));
                        self.session.state.live.economy.entities[usize::from(target_index)]
                            .set_source_word_30(yaw.cast_unsigned());
                        move_speed = 96;
                        yaw
                    } else {
                        let control_flags = snapshot.control_flags | 0x0000_0800;
                        let ground = self.canonical_terrain().ground_height(parent.axis_position);
                        let relative_z =
                            i32::from(target_position.z) - i32::from(parent.word_0x30) + 57;
                        predicted = parent.axis_position;
                        predicted.z =
                            i16::try_from((relative_z + i32::from(ground)).max(i32::from(ground)))
                                .unwrap_or(ground);
                        let yaw = wrap_source_angle(snapshot.yaw.wrapping_add(turn_delta));
                        self.session.state.live.economy.entities[usize::from(target_index)]
                            .set_control_flags(control_flags);
                        let mut entity_rng = snapshot.entity_rng;
                        entity_rng = entity_rng.wrapping_mul(9_377).wrapping_add(9_439);
                        self.session.state.live.economy.entities[usize::from(target_index)]
                            .set_entity_rng(entity_rng);
                        if relative_z >= i32::from(latch_band) + i32::from(entity_rng % latch_band)
                        {
                            self.session.state.live.economy.entities[usize::from(target_index)]
                                .set_control_flags(control_flags | 0x1000_0000);
                            self.session.state.live.economy.entities[usize::from(target_index)]
                                .set_source_word_30(yaw.cast_unsigned());
                        }
                        yaw
                    };
                    self.session.state.live.economy.entities[usize::from(target_index)]
                        .set_yaw(yaw);
                }
            }
            if move_speed != 0 {
                let move_yaw = self.session.state.live.economy.entities[usize::from(target_index)]
                    .source_word_30();
                crate::move_entity(
                    &mut predicted,
                    i16::try_from(move_yaw).expect("wind target move yaw fits"),
                    0,
                    move_speed,
                );
            }
            let ground = self.canonical_terrain().ground_height(predicted);
            let ground_clearance = snapshot
                .parameter_relation
                .and_then(|index| {
                    self.session
                        .definition
                        .presentation
                        .entity_parameters
                        .get(usize::from(index))
                })
                .map_or(0_i16, |parameter| {
                    i16::from_le_bytes([parameter[12], parameter[13]])
                });
            if predicted.z > ground {
                predicted.z = predicted.z.wrapping_add(settle_descent);
            }
            let settle_floor = ground.wrapping_add(ground_clearance);
            if predicted.z <= settle_floor {
                predicted.z = settle_floor;
            }
            if self.session.definition.profile == FlightProfile::CAVE {
                if let Some(ceiling) = self.canonical_terrain().ceiling_height(predicted) {
                    let ceiling_limit = ceiling.wrapping_sub(snapshot.array_fov());
                    if predicted.z > ceiling_limit {
                        predicted.z = ceiling_limit;
                    }
                }
            }
            let entity = &mut self.session.state.live.economy.entities[usize::from(target_index)];
            entity.set_position(predicted);
            if target_index == self.session.state.live.core.state.entity_index {
                self.session.state.live.core.state.position = predicted;
            }
            let _ = self
                .session
                .state
                .live
                .economy
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: target_index,
                    active: true,
                    x: predicted.x,
                    y: predicted.y,
                });
            if hit {
                self.session.state.live.economy.entities[usize::from(target_index)]
                    .apply_wind_model45_mailbox(parent.entity_id, parent.subspell_index);
            }
        }
    }

    /// Run one authored model-32 objective switch. Pinned
    /// `AddSwitch0B_20_6F1C0` gates on `byte_0x46 < stageIndex`, scans only
    /// participating owners, marks the matching objective done, invokes the
    /// stage-tag group scan, and disables drawing exactly once.
    pub(super) fn step_level2_authored_model32(&mut self, pass: u8, source_slot: u16) -> bool {
        let Some(source) = self
            .session
            .state
            .live
            .economy
            .authored_model32()
            .iter()
            .find(|source| source.active && source.source_entity_index == source_slot)
            .copied()
        else {
            return false;
        };
        if source.fired
            || source.objective_stage_index >= self.session.state.live.core.runtime.stage_index
        {
            return true;
        }
        let Some(completed_owner_color) = self
            .session
            .state
            .live
            .core
            .players
            .iter()
            .take(1)
            .filter(|player| player.active_player_index >= 0)
            .find_map(|player| {
                let color = player.player_color_index();
                self.session
                    .state
                    .live
                    .core
                    .runtime
                    .objectives
                    .get(usize::from(color))
                    .filter(|objective| {
                        objective
                            .stage_selectors
                            .get(usize::from(source.objective_stage_index))
                            == Some(&2)
                    })
                    .map(|_| color)
            })
        else {
            return true;
        };
        self.session
            .state
            .live
            .level2_model32_activated
            .insert(source.authored_index);
        self.session.state.live.level2_objective_pause = 1;
        self.session.state.live.level2_objective_done = true;
        self.session
            .state
            .live
            .core
            .runtime
            .mark_level2_objective_done_for(completed_owner_color);
        let consume_result = self.consume_authored_group(pass, source_slot, source.stage_tag);
        let Some(_fired_source) = self
            .session
            .state
            .live
            .economy
            .complete_authored_model32(source_slot)
        else {
            return true;
        };
        if let Err(error) = consume_result {
            self.push_dispatch_effect(Level2DispatchEvent::Error {
                pass,
                source_slot,
                message: error,
            });
            return false;
        }
        true
    }

    /// Dispatch one live dynamic row. The source class-10/model-1 burst
    /// handler applies its `life-- < 0` drawing gate; all dynamic families
    /// remain on the shared cursor and reclaim policy.
    pub(super) fn step_level2_dynamic_arena(
        &mut self,
        pass: u8,
        source_slot: u16,
        kind: Level2DynamicArenaKind,
    ) -> bool {
        let auxiliary_membership = self
            .session
            .state
            .live
            .economy
            .economy
            .castle_auxiliary_membership(&self.session.state.live.economy.entities);
        self.step_level2_dynamic_arena_with_sphere_membership(
            pass,
            source_slot,
            kind,
            None,
            Some(&auxiliary_membership),
        )
    }

    fn step_level2_dynamic_arena_with_sphere_membership(
        &mut self,
        pass: u8,
        source_slot: u16,
        kind: Level2DynamicArenaKind,
        sphere_membership: Option<&BTreeSet<u16>>,
        auxiliary_membership: Option<&level2_dynamic_economy::Level2CastleAuxiliaryMembership>,
    ) -> bool {
        let live = match kind {
            Level2DynamicArenaKind::CastleProjectile => {
                let terrain = &self.session.state.live.terrain;
                let source_arena = &mut self.session.state.live.economy;
                source_arena.economy.step_castle_projectile_slot(
                    source_slot,
                    &source_arena.entities,
                    terrain,
                )
            }
            Level2DynamicArenaKind::FireballProjectile => {
                let terrain = &self.session.state.live.terrain;
                let source_arena = &mut self.session.state.live.economy;
                let step = source_arena.economy.step_fireball_projectile_slot(
                    source_slot,
                    terrain,
                    &source_arena.entities,
                );
                step.live
            }
            Level2DynamicArenaKind::PossessionProjectile => {
                let terrain = &self.session.state.live.terrain;
                self.session
                    .state
                    .live
                    .economy
                    .step_possession_projectile_slot(source_slot, terrain)
            }
            Level2DynamicArenaKind::PossessionEffect => self
                .session
                .state
                .live
                .economy
                .step_possession_effect_slot(source_slot),
            Level2DynamicArenaKind::CastleDefender => {
                let terrain = &self.session.state.live.terrain;
                self.session
                    .state
                    .live
                    .economy
                    .step_castle_defender_slot(source_slot, terrain)
            }
            Level2DynamicArenaKind::ManaSphere => {
                let terrain = &self.session.state.live.terrain;
                self.session
                    .state
                    .live
                    .economy
                    .step_mana_sphere_slot(source_slot, terrain)
            }
            Level2DynamicArenaKind::ManaCarrier => {
                let terrain = &self.session.state.live.terrain;
                self.session
                    .state
                    .live
                    .economy
                    .step_mana_carrier_slot(source_slot, terrain)
            }
            Level2DynamicArenaKind::DeathBurst => self
                .session
                .state
                .live
                .economy
                .step_death_burst_slot(source_slot),
            Level2DynamicArenaKind::PlayerDeathMarker => self
                .session
                .state
                .live
                .economy
                .step_player_death_marker_slot(source_slot, &self.session.state.live.terrain),
            Level2DynamicArenaKind::SpellReplacement => self
                .session
                .state
                .live
                .economy
                .step_spell_replacement_slot(source_slot),
            Level2DynamicArenaKind::SpellPickup => self
                .session
                .state
                .live
                .economy
                .step_spell_pickup_slot(source_slot),
            Level2DynamicArenaKind::SpeedTrail => self
                .session
                .state
                .live
                .economy
                .step_speed_trail(source_slot),
            Level2DynamicArenaKind::AuthoredModel1 => self
                .session
                .state
                .live
                .economy
                .step_authored_model1_slot(source_slot),
            Level2DynamicArenaKind::AuthoredModel45 => self
                .session
                .state
                .live
                .economy
                .step_authored_model45_slot(source_slot),
            Level2DynamicArenaKind::AuthoredModel13 => {
                let terrain = &self.session.state.live.terrain;
                self.session
                    .state
                    .live
                    .economy
                    .step_authored_model13_slot(source_slot, terrain)
            }
            Level2DynamicArenaKind::AuthoredBee => {
                let source_before = self
                    .session
                    .state
                    .live
                    .economy
                    .authored_bees()
                    .iter()
                    .find(|source| source.active && source.source_entity_index == source_slot)
                    .copied();
                let Some(source_before) = source_before else {
                    return false;
                };
                let pre_kill_credit = (source_before.action == 20)
                    .then_some(source_before.kill_credit_entity_index)
                    .filter(|killer| *killer != 0);
                let class3_candidates = if source_before.action == 17
                    || (source_before.action == 23 && source_before.stage_var2 == 2)
                {
                    self.level2_bee_class3_targets()
                } else {
                    Vec::new()
                };
                let acquired_target = if source_before.action == 17 {
                    let terrain = &self.session.state.live.terrain;
                    self.session.state.live.economy.acquire_authored_bee_target(
                        source_slot,
                        &class3_candidates,
                        terrain,
                    )
                } else {
                    None
                };
                let source = self
                    .session
                    .state
                    .live
                    .economy
                    .authored_bees()
                    .iter()
                    .find(|source| source.active && source.source_entity_index == source_slot)
                    .copied()
                    .expect("authored Bee remains live after target scan");
                let target = if source_before.action == 17 {
                    acquired_target
                        .and_then(|target_index| self.level2_bee_entity_target(target_index))
                } else if source.action == 18 {
                    self.level2_bee_action18_target(&source)
                } else {
                    source
                        .target_entity_index
                        .and_then(|target_index| self.level2_bee_entity_target(target_index))
                };
                let contact_candidate = source_before.action == 18
                    && source_before.target_entity_index
                        == Some(self.session.state.live.core.state.entity_index)
                    && target.is_some_and(|(_, _, valid)| valid)
                    && source_before.action_window > 0
                    && source_before.source_byte_3e
                        % u8::try_from(source_before.action_window)
                            .expect("positive Bee action window")
                        == 0;
                let peers = self.level2_bee_peer_rows(source_before.model);
                let terrain = &self.session.state.live.terrain;
                let live = self
                    .session
                    .state
                    .live
                    .economy
                    .step_authored_bee_slot_with_target_and_peers(
                        source_slot,
                        target,
                        &peers,
                        &class3_candidates,
                        terrain,
                    );
                if let Some(killer_entity_index) = pre_kill_credit {
                    // `PreKillEntity_1C890` credits only a live, drawn
                    // class-3/model-0 owner distinct from the killed root.
                    // Keep stale, disabled, wrong-class/model, and self-owned
                    // identities fail-closed before changing the reset census.
                    let qualifying_killer = killer_entity_index != source_before.entity_id
                        && self
                            .session
                            .state
                            .live
                            .economy
                            .entities
                            .iter()
                            .any(|entity| {
                                entity.index == killer_entity_index
                                    && entity.active
                                    && entity.life >= 0
                                    && entity.class == 3
                                    && entity.model == 0
                                    && !entity.drawing_disabled()
                            });
                    if qualifying_killer {
                        let _ = self
                            .session
                            .state
                            .live
                            .economy
                            .record_authored_bee_kill(killer_entity_index);
                    }
                }
                if contact_candidate {
                    if let (Some((target_position, _, true)), Some(after)) = (
                        target,
                        self.session
                            .state
                            .live
                            .economy
                            .authored_bees()
                            .iter()
                            .find(|candidate| {
                                candidate.active && candidate.source_entity_index == source_slot
                            })
                            .copied(),
                    ) {
                        let distance_squared =
                            level2_wrapping_source_distance_3d(after.position, target_position);
                        if after.action == 18
                            && distance_squared
                                < u32::from(LEVEL2_BEE_CONTACT_RADIUS)
                                    .saturating_mul(u32::from(LEVEL2_BEE_CONTACT_RADIUS))
                        {
                            // `sub_1C310` invokes `sub_1CE80`, which queues
                            // subspell 200 through the real player mailbox;
                            // the player source callback consumes it at its
                            // next source-ordered visit.
                            let damage = i32::from(after.subspell_index);
                            self.queue_level2_player_contact_damage(damage, after.entity_id);
                        }
                    }
                }
                live
            }
            Level2DynamicArenaKind::Model13SpawnedChild => {
                let terrain = &self.session.state.live.terrain;
                self.session
                    .state
                    .live
                    .economy
                    .step_model13_spawned_child_slot(source_slot, terrain)
            }
            Level2DynamicArenaKind::AuthoredModel6 => {
                let terrain = &self.session.state.live.terrain;
                self.session
                    .state
                    .live
                    .economy
                    .step_authored_model6_slot(source_slot, terrain)
            }
            Level2DynamicArenaKind::Model6SpawnedChild => {
                let terrain = &self.session.state.live.terrain;
                self.session
                    .state
                    .live
                    .economy
                    .step_model6_spawned_child_slot(source_slot, terrain)
            }
            Level2DynamicArenaKind::AuthoredModel17 => self
                .session
                .state
                .live
                .economy
                .step_authored_model17_slot(source_slot),
            Level2DynamicArenaKind::Model17SpawnedChild => {
                let terrain = &self.session.state.live.terrain;
                let step = self
                    .session
                    .state
                    .live
                    .economy
                    .step_model17_spawned_child_slot(source_slot, terrain);
                if step.first_impact {
                    if step.damage_pulse {
                        self.apply_level2_class10_model0_damage(step);
                    }
                    mark_source_audio_call(SourceAudioCallTag::Class10Action00FirstEligibleSound);
                    self.push_flight_effect(FlightEvent::PrepareSound {
                        #[cfg(test)]
                        audio_site: crate::MissionAudioSite::FireballImpact,
                        entity_index: source_slot,
                        player_index: -1,
                        sound_id: 3,
                    });
                }
                step.live
            }
            Level2DynamicArenaKind::AuthoredModel22 => {
                let terrain = &self.session.state.live.terrain;
                let live = self
                    .session
                    .state
                    .live
                    .economy
                    .step_authored_model22_slot(source_slot, terrain);
                if live {
                    self.step_level2_authored_model22_sub33340_eligible_targets(source_slot);
                    self.step_level2_authored_model22_wind_pulse(source_slot);
                }
                live
            }
            Level2DynamicArenaKind::Model22SpawnedChild => self
                .session
                .state
                .live
                .economy
                .step_model22_spawned_child_slot(source_slot),
            Level2DynamicArenaKind::AuthoredModel76 => {
                // `sub_339B0` calls `sub_33C00` only on its state 0/1 route;
                // state 2 runs the terminal countdown and returns before the
                // collision/effect callback. Capture the pre-callback state
                // because a state-1 row may become state 2 after life--.
                let probe_collision = self
                    .session
                    .state
                    .live
                    .economy
                    .authored_model76()
                    .iter()
                    .find(|source| source.active && source.source_entity_index == source_slot)
                    .is_some_and(|source| source.state != 2);
                let terrain = &self.session.state.live.terrain;
                let live = self
                    .session
                    .state
                    .live
                    .economy
                    .step_authored_model76_slot(source_slot, terrain);
                if live && probe_collision {
                    self.step_level2_authored_model76_collision_effect(source_slot);
                }
                live
            }
            Level2DynamicArenaKind::Model76Child => self
                .session
                .state
                .live
                .economy
                .step_model76_child_slot(source_slot),
            Level2DynamicArenaKind::Model76TailSpawn => self
                .session
                .state
                .live
                .economy
                .step_model76_tail_spawn_slot(source_slot),
            Level2DynamicArenaKind::AuthoredClass14 => {
                self.step_level2_authored_class14(source_slot)
            }
            Level2DynamicArenaKind::AuthoredClass5 => {
                let source = self
                    .session
                    .state
                    .live
                    .economy
                    .authored_class5()
                    .iter()
                    .find(|source| source.active && source.source_entity_index == source_slot)
                    .copied();
                if let Some(source) =
                    source.filter(|source| source.model == level2_class5::archer::ARCHER_MODEL)
                {
                    let archer_state = self
                        .session
                        .state
                        .live
                        .economy
                        .authored_class5_archer()
                        .iter()
                        .find(|state| state.source_entity_index == source_slot)
                        .copied();
                    let Some(archer_state) = archer_state else {
                        return false;
                    };
                    let target = self.level2_archer_target(source.word_0x96_150);
                    let rows = self.level2_archer_entity_rows();
                    let linked = self.level2_archer_linked_rows(archer_state.word_0x34);
                    let context = level2_class5::archer::ArcherActionContext {
                        pending_damage: archer_state.pending_damage,
                        linked_head: archer_state.word_0x34,
                        linked: &linked,
                        target,
                        entities: &rows,
                        fallback_entities: &rows,
                        peers: &rows,
                    };
                    let terrain = &self.session.state.live.terrain;
                    let Some(step) = self.session.state.live.economy.step_authored_archer_slot(
                        source_slot,
                        context,
                        terrain,
                    ) else {
                        return false;
                    };
                    if let Some(target) = step.target {
                        self.apply_level2_archer_target(target);
                    }
                    if let Some(entity_index) = step.effects.target_feedback_entity {
                        self.session
                            .state
                            .live
                            .nyphur_dispatcher
                            .apply_class5_player_feedback(entity_index);
                    }
                    return step.map_synced;
                }
                if let Some(source) = source
                    && source.model == level2_class5::trader::TRADER_MODEL
                    && source.action == level2_class5::trader::TRADER_ACTION
                {
                    let Some(runtime) = self
                        .session
                        .state
                        .live
                        .economy
                        .authored_class5_runtime()
                        .iter()
                        .find(|state| state.source_entity_index == source_slot)
                        .copied()
                    else {
                        return false;
                    };
                    let target_index = if source.pending_word_0x62 != 0 {
                        source.pending_word_0x62
                    } else {
                        source.word_0x96_150
                    };
                    let target = self.level2_trader_target(target_index);
                    let linked = self.level2_trader_linked_rows(runtime.linked_head);
                    let buildings = self.level2_trader_building_candidates();
                    let terrain = &self.session.state.live.terrain;
                    let Some(step) = self.session.state.live.economy.step_authored_trader_slot(
                        source_slot,
                        target,
                        runtime.pending_damage,
                        runtime.linked_head,
                        &linked,
                        &buildings,
                        terrain,
                    ) else {
                        return false;
                    };
                    if let Some(target) = step.target {
                        self.apply_level2_trader_target(target);
                    }
                    if let Some(entity_index) = step.effects.player_feedback_entity {
                        self.session
                            .state
                            .live
                            .nyphur_dispatcher
                            .apply_class5_player_feedback(entity_index);
                    }
                    return step.map_synced;
                }
                let target = source.and_then(|source| {
                    if source.model == 3 && source.action == 25 {
                        return self.level2_model3_class3_target(&source);
                    }
                    let target_index = if source.pending_word_0x62 != 0 && source.byte_0x3b != 0 {
                        source.pending_word_0x62
                    } else {
                        source.word_0x96_150
                    };
                    (target_index != 0)
                        .then(|| {
                            self.session
                                .state
                                .live
                                .economy
                                .entities
                                .get(usize::from(target_index))
                                .cloned()
                        })
                        .flatten()
                });
                if source.is_some_and(|source| {
                    source.model == 3 && matches!(source.action, 25 | 26 | 27 | 28 | 30)
                }) {
                    let event_start = self.session.state.live.economy.events.len();
                    let hit_target = source
                        .filter(|source| source.action == 26)
                        .and_then(|_| target.as_ref().map(|target| target.index));
                    let terrain = &self.session.state.live.terrain;
                    let live = self
                        .session
                        .state
                        .live
                        .economy
                        .step_authored_class5_model3_slot_with_target(source_slot, target, terrain);
                    let hit = self.session.state.live.economy.events[event_start..]
                        .iter()
                        .any(|event| {
                            matches!(
                                event,
                                Level2EconomyEvent::AuthoredClass5Model3Sound {
                                    source_entity_index,
                                    sound_id: 8,
                                } if *source_entity_index == source_slot
                            )
                        });
                    if hit {
                        if let Some(target_index) = hit_target {
                            self.session
                                .state
                                .live
                                .nyphur_dispatcher
                                .apply_class5_player_feedback(target_index);
                        }
                    }
                    return live;
                }
                let terrain = &self.session.state.live.terrain;
                self.session
                    .state
                    .live
                    .economy
                    .step_authored_class5_slot_with_target(source_slot, target, terrain)
            }
            Level2DynamicArenaKind::AuthoredClass5Model3Child => self
                .session
                .state
                .live
                .economy
                .step_authored_class5_model3_child_slot(source_slot),
            Level2DynamicArenaKind::AuthoredClass5ArcherChild => {
                let terrain = &self.session.state.live.terrain;
                self.session
                    .state
                    .live
                    .economy
                    .step_authored_class5_archer_child_slot(source_slot, terrain)
            }
            Level2DynamicArenaKind::HouseResident => self
                .session
                .state
                .live
                .economy
                .step_house_resident_slot(source_slot),
            Level2DynamicArenaKind::AuthoredSwitchProducer => {
                self.step_level2_authored_switch_producer(pass, source_slot)
            }
            Level2DynamicArenaKind::AuthoredTree => self.step_level2_authored_tree(source_slot),
            Level2DynamicArenaKind::AuthoredModel32 => {
                self.step_level2_authored_model32(pass, source_slot)
            }
            Level2DynamicArenaKind::AuthoredModel59 => self
                .session
                .state
                .live
                .economy
                .step_authored_model59_slot(source_slot),
            Level2DynamicArenaKind::Model59SmokeChild => {
                let terrain = &self.session.state.live.terrain;
                self.session
                    .state
                    .live
                    .economy
                    .step_model59_smoke_child_slot(source_slot, terrain)
            }
            Level2DynamicArenaKind::AuthoredModel53 => self
                .session
                .state
                .live
                .economy
                .step_authored_model53_slot(source_slot),
            Level2DynamicArenaKind::AuthoredModel5WaterSplash => self
                .session
                .state
                .live
                .economy
                .step_authored_model5_water_splash_slot(source_slot),
            Level2DynamicArenaKind::BeyondSightPulseEffect => self
                .session
                .state
                .live
                .economy
                .step_beyond_sight_pulse_effect_slot(source_slot),
            Level2DynamicArenaKind::ManaAttraction => match sphere_membership {
                Some(membership) => self
                    .session
                    .state
                    .live
                    .economy
                    .step_mana_attraction_slot_with_sphere_membership(source_slot, membership),
                None => self
                    .session
                    .state
                    .live
                    .economy
                    .step_mana_attraction_slot(source_slot),
            },
            Level2DynamicArenaKind::CastleAuxiliary
            | Level2DynamicArenaKind::CastleChild
            | Level2DynamicArenaKind::CastleController => {
                let live = &mut self.session.state.live;
                let current_membership;
                let auxiliary_membership = match auxiliary_membership {
                    Some(membership) => membership,
                    None => {
                        current_membership = live
                            .economy
                            .economy
                            .castle_auxiliary_membership(&live.economy.entities);
                        &current_membership
                    }
                };
                live.economy.economy.step_castle_child_terminal_slot(
                    source_slot,
                    &mut live.economy.entities,
                    auxiliary_membership,
                    &mut live.terrain,
                )
            }
            Level2DynamicArenaKind::CastleRoot => true,
        };
        if !live
            && kind == Level2DynamicArenaKind::CastleProjectile
            && let Some(holder_index) = self
                .session
                .state
                .live
                .economy
                .rejected_castle_projectile_holder(source_slot)
        {
            self.clear_castle_spell_holder_cooldown(holder_index);
        }
        if matches!(
            kind,
            Level2DynamicArenaKind::CastleRoot
                | Level2DynamicArenaKind::CastleChild
                | Level2DynamicArenaKind::CastleAuxiliary
                | Level2DynamicArenaKind::CastleController
        ) && self
            .session
            .state
            .live
            .economy
            .take_castle_upgrade_holder_clear_pending()
        {
            if let Some(holder_index) = self
                .session
                .state
                .live
                .shared_spells
                .holders
                .iter()
                .chain(self.session.state.live.shared_spells.dynamic_pickups.iter())
                .find(|holder| holder.model == CASTLE_SPELL_FAMILY)
                .map(|holder| holder.index)
            {
                self.clear_castle_spell_holder_cooldown(holder_index);
            }
        }
        live
    }

    /// Apply the source `KillEntity_1C930` side effects emitted by the typed
    /// Goat family in the same event order: mana conversion allocates the
    /// model-39 sphere first, the extra-sphere request allocates the
    /// class-10/model-1 death burst second, and the removal marker then leaves
    /// both rows for the live-arena cursor/reclaim pass.
    pub(super) fn apply_level2_goat_economy_events(&mut self, events: &[GoatStepEvent]) {
        for event in events {
            match *event {
                GoatStepEvent::ManaConverted { entity_index, mana } => {
                    if let Some((position, yaw, rng)) = self
                        .session
                        .state
                        .live
                        .goats
                        .goats()
                        .iter()
                        .find(|goat| goat.entity_index == entity_index)
                        .map(|goat| (goat.position, goat.yaw, goat.rng))
                    {
                        let owner_entity_index =
                            self.session.state.live.economy.source_owner(entity_index);
                        let terrain = &self.session.state.live.terrain;
                        let _ = self
                            .session
                            .state
                            .live
                            .economy
                            .spawn_mana_sphere_from_death(
                                entity_index,
                                position,
                                yaw,
                                rng,
                                mana,
                                owner_entity_index,
                                terrain,
                            );
                    }
                }
                GoatStepEvent::ExtraManaSphereRequested { entity_index } => {
                    if let Some((position, source_id, _control_flags)) = self
                        .session
                        .state
                        .live
                        .goats
                        .goats()
                        .iter()
                        .find(|goat| goat.entity_index == entity_index)
                        .map(|goat| (goat.position, goat.source_id, goat.control_flags))
                        // `KillEntity_1C930` suppresses the class-10/model-1
                        // burst when struct byte[2] bit 0x10 is set.  Goat
                        // control flags preserve that raw little-endian
                        // dword, so the gate is 0x0010_0000 here.
                        .filter(|(_, _, control_flags)| control_flags & 0x0010_0000 == 0)
                    {
                        let _ = self
                            .session
                            .state
                            .live
                            .economy
                            .spawn_death_burst_from_death_with_id(
                                entity_index,
                                source_id,
                                position,
                            );
                    }
                }
                _ => {}
            }
        }
    }

    #[must_use]
    pub fn mission(&self) -> &MissionContentSession {
        &self.session.state.live.mission
    }

    #[must_use]
    pub fn dispatch_events(&self) -> &[Level2DispatchEvent] {
        &self.session.state.scratch.dispatch_events
    }

    /// Stable only as a session observability digest, not as a conformance
    /// hash: the typed event stream is the authoritative A4.1 evidence.
    #[must_use]
    pub fn dispatch_observability_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(REMC2_SPEC_COMMIT.as_bytes());
        hasher.update(b"level2-campaign-events-v1");
        for (ordinal, event) in self
            .session
            .state
            .scratch
            .dispatch_events
            .iter()
            .enumerate()
        {
            let (pass, kind, family, flags, source_slot) = event.campaign_projection();
            hasher.update(u32::try_from(ordinal).unwrap_or(u32::MAX).to_le_bytes());
            hasher.update([pass, kind, family, flags]);
            hasher.update(source_slot.to_le_bytes());
        }
        // This digest is the stable typed event/dispatch-trace contract. The
        // mutable common-upkeep carrier belongs to campaign_state_hash; adding
        // it here makes the C ABI dispatch digest depend on unrelated movement
        // and regeneration fields that are not present in its canonical ledger.
        hasher.update(b"level2-nyphur-dispatch-traces-v1");
        for trace in &self.session.state.scratch.nyphur_dispatch_traces {
            hasher.update([trace.pass]);
            hasher.update(trace.source_slot.to_le_bytes());
            hasher.update(trace.state_byte.to_le_bytes());
            hasher.update(
                u32::try_from(trace.calls.len())
                    .expect("sub_12910 trace call count is bounded")
                    .to_le_bytes(),
            );
            for call in &trace.calls {
                hasher.update([call.hash_tag()]);
            }
        }
        format!("{:x}", hasher.finalize())
    }

    /// Composite Level 2 state fingerprint for the additive campaign ABI.
    /// The legacy `state_hash()` remains the flight-only Level 1-compatible
    /// value; this hash binds the flight state to every mutable Level 2
    /// subsystem and the exported player/entity/runtime records.
    #[must_use]
    pub fn campaign_state_hash(&self) -> u64 {
        self.campaign_state_hash_with_terrain(self.canonical_terrain())
    }

    #[must_use]
    pub(crate) fn campaign_state_hash_with_terrain(&self, terrain: &SessionTerrain) -> u64 {
        let mut hasher = Sha256::new();
        hasher.update(REMC2_SPEC_COMMIT.as_bytes());
        hasher.update(b"level2-campaign-state-v1");
        hasher.update(state_hash(&self.session.state.live.core.state).to_le_bytes());
        hasher.update(terrain.deterministic_hash().to_le_bytes());
        for component in [
            self.session
                .state
                .live
                .shared_spells
                .reset_hash_with_terrain(terrain),
            self.session
                .state
                .live
                .economy
                .reset_hash_with_terrain(terrain),
            self.session.state.live.ordinary.reset_hash(),
            self.goat_hash(),
            self.bad_stone_hash(),
            self.session.state.live.mission.state_hash(),
        ] {
            hasher.update(
                u32::try_from(component.len())
                    .unwrap_or(u32::MAX)
                    .to_le_bytes(),
            );
            hasher.update(component.as_bytes());
        }
        hasher.update(b"level2-mana-attraction-activation-v1");
        for authored_index in &self.session.state.live.level2_mana_attraction_activated {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"level2-model13-activation-v1");
        for authored_index in &self.session.state.live.level2_model13_activated {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"level2-bee-activation-v1");
        for authored_index in &self.session.state.live.level2_bees_activated {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"level2-switch-producer-activation-v1");
        for authored_index in &self.session.state.live.level2_switch_producers_activated {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"level2-model32-activation-v1");
        for authored_index in &self.session.state.live.level2_model32_activated {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"level2-imported-switch-root-v1");
        for root in &self.session.state.live.level2_imported_switch_roots {
            hasher.update(root.source_slot.to_le_bytes());
            hasher.update(root.authored_index.to_le_bytes());
            hasher.update([
                root.model,
                root.phase,
                root.cooldown,
                u8::from(root.drawing_enabled),
                u8::from(root.active),
            ]);
            hasher.update(root.target_group.to_le_bytes());
            hasher.update(root.word_10.to_le_bytes());
            hasher.update(root.position.x.to_le_bytes());
            hasher.update(root.position.y.to_le_bytes());
            hasher.update(root.position.z.to_le_bytes());
        }
        hasher.update(b"level2-model76-activation-v1");
        for authored_index in &self.session.state.live.level2_model76_activated {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"level2-model22-activation-v1");
        for authored_index in &self.session.state.live.level2_model22_activated {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"level2-model39-activation-v1");
        for authored_index in &self.session.state.live.level2_model39_activated {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"level2-model6-activation-v1");
        for authored_index in &self.session.state.live.level2_model6_activated {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"level2-model59-activation-v1");
        for authored_index in &self.session.state.live.level2_model59_activated {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"level2-model53-activation-v1");
        for authored_index in &self.session.state.live.level2_model53_activated {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"level2-model1-activation-v1");
        for authored_index in &self.session.state.live.level2_model1_activated {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"level2-model17-activation-v1");
        for authored_index in &self.session.state.live.level2_model17_activated {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"campaign-authored-feature-consumption-v1");
        for authored_index in &self.session.state.live.authored_features_consumed {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"level2-group10-switch25-v1");
        match self.session.state.live.level2_group10_switch25 {
            Some(switch) => {
                hasher.update([1, switch.phase]);
                hasher.update(switch.source_slot.to_le_bytes());
                hasher.update(switch.authored_index.to_le_bytes());
                hasher.update(switch.target_group.to_le_bytes());
                hasher.update(switch.position.x.to_le_bytes());
                hasher.update(switch.position.y.to_le_bytes());
                hasher.update(switch.position.z.to_le_bytes());
            }
            None => hasher.update([0]),
        }
        hasher.update(b"level2-class14-activation-v1");
        for authored_index in &self.session.state.live.level2_class14_activated {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"level2-tree-activation-v1");
        for authored_index in &self.session.state.live.level2_tree_activated {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"level2-class5-activation-v1");
        for authored_index in &self.session.state.live.level2_class5_activated {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"level2-model45-activation-v1");
        for authored_index in &self.session.state.live.level2_model45_activated {
            hasher.update(authored_index.to_le_bytes());
        }
        hasher.update(b"level2-objective-switch-state-v1");
        hasher.update([
            self.session
                .state
                .live
                .level2_selected_objective_stage_state,
            self.session.state.live.level2_objective_pause,
            u8::from(self.session.state.live.level2_objective_done),
        ]);
        digest_level_exit_sequence(&mut hasher, self.session.state.live.level_exit);
        hasher.update(b"level2-nyphur-dispatcher-v1");
        if let Some(dispatcher) = self.session.state.live.nyphur_dispatcher.as_ref() {
            hasher.update([1]);
            hasher.update(dispatcher.state_byte().to_le_bytes());
            if let Some(entity) = self
                .session
                .state
                .live
                .economy
                .entities
                .iter()
                .find(|entity| entity.index == dispatcher.common_upkeep.source_slot)
            {
                dispatcher
                    .common_upkeep
                    .hash_into(&mut hasher, entity.phase);
            }
        } else {
            hasher.update([0]);
        }
        for trace in &self.session.state.scratch.nyphur_dispatch_traces {
            hasher.update([trace.pass]);
            hasher.update(trace.source_slot.to_le_bytes());
            hasher.update(trace.state_byte.to_le_bytes());
            hasher.update(
                u32::try_from(trace.calls.len())
                    .expect("sub_12910 trace call count is bounded")
                    .to_le_bytes(),
            );
            for call in &trace.calls {
                hasher.update([call.hash_tag()]);
            }
        }
        hasher.update(b"level2-player-contact-mailbox-v1");
        hasher.update(self.session.state.live.player_pending_damage.to_le_bytes());
        hasher.update(
            self.session
                .state
                .live
                .player_pending_attacker
                .to_le_bytes(),
        );
        hasher.update(self.session.state.live.player_life.to_le_bytes());
        hasher.update(b"level2-day-hud-frame-state-v1");
        hasher.update([
            self.session.state.scratch.level2_day_hud_color_phase_1,
            self.session.state.scratch.level2_day_hud_color_phase_2,
        ]);
        hasher.update(
            self.session
                .state
                .scratch
                .level2_day_hud_player_panel_selector
                .to_le_bytes(),
        );
        hasher.update(self.session.state.live.core.runtime.bytes());
        for entity in &self.session.state.live.economy.entities {
            hasher.update(entity.record_bytes());
        }
        for player in &self.session.state.live.core.players {
            hasher.update(player.record_bytes());
        }
        // The byte at offset 662 is decoded into an authoritative mutable
        // carrier rather than read back from ActPlayerIndex's compact record.
        // Hash its presence as well as its value so an inspection-only v4 row
        // cannot collide with a qualified v5 session and ordinary mutations
        // are reset/replay-visible.
        hasher.update(b"level2-player-banishment-state-v1");
        for player in &self.session.state.live.core.players {
            hasher.update([
                player.slot,
                u8::from(player.banishment_state_0x006.is_some()),
            ]);
            if let Some(value) = player.banishment_state_0x006 {
                hasher.update([value]);
            }
        }
        let digest = hasher.finalize();
        u64::from_le_bytes(
            digest[..8]
                .try_into()
                .expect("SHA-256 prefix is eight bytes"),
        )
    }

    #[must_use]
    pub fn state_hash(&self) -> u64 {
        state_hash(&self.session.state.live.core.state)
    }

    #[must_use]
    pub fn terrain_hash(&self) -> u64 {
        self.canonical_terrain().deterministic_hash()
    }

    pub(super) fn sync_level2_day_hud_player_projection(&mut self) {
        let local_player_slot = self.session.state.live.shared_spells.local_player.slot;
        let turn = self.session.state.live.shared_spells.local_player.turn;
        let hit_frame_time = self
            .session
            .state
            .live
            .shared_spells
            .local_player
            .record_bytes()[406];
        if let Some(player) = self
            .session
            .state
            .live
            .core
            .players
            .iter_mut()
            .find(|player| player.slot == local_player_slot)
        {
            player.turn = turn;
            player.record[8..12].copy_from_slice(&turn.to_le_bytes());
            player.record[406] = hit_frame_time;
        }
    }

    /// Complete the source DrawGameFrame HUD phase after all entity passes.
    /// DrawTopStatus selects the hit panel from the pre-decrement timer, then
    /// decrements only when colorIndex[1] is set.  Store that final panel
    /// decision so repeated ABI reads are idempotent.
    pub(super) fn finish_level2_day_hud_frame(&mut self) {
        let hit_frame_time = self
            .session
            .state
            .live
            .shared_spells
            .local_player
            .record_bytes()[406];
        self.session
            .state
            .scratch
            .level2_day_hud_hit_frame_time_pre_draw = hit_frame_time;
        self.session
            .state
            .scratch
            .level2_day_hud_player_panel_selector = level2_day_player_panel_selector(
            hit_frame_time,
            self.session.state.scratch.level2_day_hud_color_phase_1,
        );
        if hit_frame_time > 0 && self.session.state.scratch.level2_day_hud_color_phase_1 != 0 {
            self.session
                .state
                .live
                .shared_spells
                .set_player_hit_frame_time(hit_frame_time - 1);
        }
        self.sync_level2_day_hud_player_projection();
    }

    /// Queue the source `sub_11900` contact payload for the local player.
    /// Contact writes overwrite an already-owned player mailbox and otherwise
    /// add to an unowned payload, matching the source primitive.  Shield
    /// status is evaluated later by the player's source callback.
    pub(super) fn queue_level2_player_contact_damage(&mut self, damage: i32, attacker: u16) {
        if self.session.state.live.player_life < 0 || damage <= 0 || attacker == 0 {
            return;
        }
        let player_entity_index = self.session.state.live.core.state.entity_index;
        let entity = self
            .session
            .state
            .live
            .economy
            .entities
            .iter_mut()
            .find(|entity| entity.index == player_entity_index)
            .expect("Level 2 local player entity remains present");
        let raw_attacker = u16::from_le_bytes(
            entity.record[102..104]
                .try_into()
                .expect("player attacker mailbox bytes"),
        );
        let raw_damage = i32::from_le_bytes(
            entity.record[98..102]
                .try_into()
                .expect("player damage mailbox bytes"),
        );
        let next_damage = if raw_attacker != 0 {
            damage
        } else {
            raw_damage.wrapping_add(damage)
        };
        entity.record[98..102].copy_from_slice(&next_damage.to_le_bytes());
        entity.record[102..104].copy_from_slice(&attacker.to_le_bytes());
        self.session.state.live.player_pending_damage = next_damage;
        self.session.state.live.player_pending_attacker = attacker;
        self.session
            .state
            .live
            .shared_spells
            .set_player_entity_damage_mailbox(next_damage, attacker);
    }

    /// Consume the local-player mailbox on the player's source-cursor visit.
    /// Return the applied payload for the typed dispatcher proof.
    pub(super) fn consume_level2_player_damage(&mut self) -> Option<(i32, u16, bool)> {
        let player_entity_index = self.session.state.live.core.state.entity_index;
        let (attacker, damage, previous_killer) = {
            let entity = self
                .session
                .state
                .live
                .economy
                .entities
                .iter()
                .find(|entity| entity.index == player_entity_index)
                .expect("Level 2 local player entity remains present");
            (
                u16::from_le_bytes(
                    entity.record[102..104]
                        .try_into()
                        .expect("player attacker mailbox bytes"),
                ),
                i32::from_le_bytes(
                    entity.record[98..102]
                        .try_into()
                        .expect("player damage mailbox bytes"),
                ),
                entity.word_24(),
            )
        };
        let attacker_is_model67 = self
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .any(|entity| {
                entity.index == attacker
                    && entity.active
                    && entity.class == 10
                    && entity.model == 67
            });
        if attacker == 0 {
            let entity = self
                .session
                .state
                .live
                .economy
                .entities
                .iter_mut()
                .find(|entity| entity.index == player_entity_index)
                .expect("Level 2 local player entity remains present");
            entity.record[42..44].fill(0);
            self.session
                .state
                .live
                .shared_spells
                .set_player_entity_damage_result(previous_killer, 0, false);
            return None;
        }

        // `sub_5EFA0` leaves a lethal player's owner mailbox intact and only
        // clears the per-visit attacker word. It does not apply the payload
        // again on later visits because the life guard has already failed.
        if self.session.state.live.player_life < 0 {
            let entity = self
                .session
                .state
                .live
                .economy
                .entities
                .iter_mut()
                .find(|entity| entity.index == player_entity_index)
                .expect("Level 2 local player entity remains present");
            entity.record[42..44].fill(0);
            self.session
                .state
                .live
                .shared_spells
                .set_player_entity_damage_result(previous_killer, 0, false);
            return None;
        }

        let shield_damage = self
            .session
            .state
            .live
            .shared_spells
            .absorb_player_shield_damage(player_entity_index, damage, attacker);
        let applied_damage = shield_damage.map_or(damage, |damage| damage.applied_damage);
        self.session.state.live.player_life = self
            .session
            .state
            .live
            .player_life
            .wrapping_sub(applied_damage);
        self.session
            .state
            .live
            .shared_spells
            .set_player_hit_frame_time(4);
        let lethal = self.session.state.live.player_life < 0;
        let killer = if lethal && attacker_is_model67 {
            0
        } else if lethal {
            attacker
        } else {
            previous_killer
        };
        let player_life = self.session.state.live.player_life;
        let shield_projection = self
            .session
            .state
            .live
            .shared_spells
            .shield_state()
            .filter(|shield| shield.parent_entity_index == player_entity_index)
            .map(|shield| {
                (
                    shield.parent_mana,
                    shield.parent_mana_regen,
                    shield.parent_status_flags,
                )
            });
        {
            let entity = self
                .session
                .state
                .live
                .economy
                .entities
                .iter_mut()
                .find(|entity| entity.index == player_entity_index)
                .expect("Level 2 local player entity remains present");
            entity.set_damage_result_words(killer, attacker);
            if lethal {
                entity.set_player_death_state();
            } else {
                // `sub_5EFA0` clears only the owner word after a nonlethal
                // visit. The payload remains for the next ownerless add.
                entity.record[102..104].copy_from_slice(&0_u16.to_le_bytes());
            }
            entity.life = player_life;
            entity.record[12..16].copy_from_slice(&player_life.to_le_bytes());
            entity.record[98..102].copy_from_slice(&applied_damage.to_le_bytes());
            if let Some((mana, mana_regen, status_flags)) = shield_projection {
                entity.mana = mana;
                entity.mana_regen = mana_regen;
                entity.control_flags = status_flags;
                entity.record[16..20].copy_from_slice(&status_flags.to_le_bytes());
                entity.record[140..144].copy_from_slice(&mana_regen.to_le_bytes());
                entity.record[148..152].copy_from_slice(&mana.to_le_bytes());
            }
        }
        self.session.state.live.player_pending_attacker = if lethal { attacker } else { 0 };
        self.session.state.live.player_pending_damage = applied_damage;
        self.session
            .state
            .live
            .shared_spells
            .set_player_entity_life(self.session.state.live.player_life);
        self.session
            .state
            .live
            .shared_spells
            .set_player_entity_damage_mailbox(applied_damage, if lethal { attacker } else { 0 });
        self.session
            .state
            .live
            .shared_spells
            .set_player_entity_damage_result(killer, attacker, lethal);
        shield_damage
            .is_none()
            .then_some((damage, attacker, lethal))
    }

    fn release_level2_player_spell_holders(&mut self, player_position: Position) {
        let player_entity_index = self.session.state.live.core.state.entity_index;
        let enabled = self
            .session
            .state
            .live
            .shared_spells
            .enabled_spell_holder_indices();
        let mut rng =
            self.session.state.live.economy.entities[usize::from(player_entity_index)].entity_rng;

        for (family, holder_index) in enabled.into_iter().enumerate() {
            if holder_index == 0
                || usize::from(holder_index) >= self.session.state.live.economy.entities.len()
            {
                self.session
                    .state
                    .live
                    .shared_spells
                    .set_enabled_spell_holder_index(family, 0);
                continue;
            }

            self.session
                .state
                .live
                .shared_spells
                .set_enabled_spell_holder_index(family, 1);
            {
                let holder =
                    &mut self.session.state.live.economy.entities[usize::from(holder_index)];
                holder.set_control_flags(holder.control_flags & !1);
                holder.set_source_action(holder.action.wrapping_add(1));
            }

            rng = level2_next_rng16(rng);
            self.session.state.live.economy.entities[usize::from(player_entity_index)]
                .set_entity_rng(rng);
            let x_offset = i16::try_from(i32::from(rng & 0x01ff) - 256)
                .expect("nine-bit holder x offset fits i16");
            rng = level2_next_rng16(rng);
            self.session.state.live.economy.entities[usize::from(player_entity_index)]
                .set_entity_rng(rng);
            let y_offset = i16::try_from(i32::from(rng & 0x01ff) - 256)
                .expect("nine-bit holder y offset fits i16");
            let position = Position {
                x: player_position.x.wrapping_add_signed(x_offset),
                y: player_position.y.wrapping_add_signed(y_offset),
                z: player_position.z,
            };

            let mut snapshot = {
                let holder =
                    &mut self.session.state.live.economy.entities[usize::from(holder_index)];
                holder.set_position(position);
                holder.clone()
            };
            self.session
                .state
                .live
                .economy
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: holder_index,
                    active: true,
                    x: position.x,
                    y: position.y,
                })
                .expect("released Level 2 holder source slot fits map chain");
            self.session
                .state
                .live
                .shared_spells
                .sync_released_holder(&snapshot);

            // Pinned sub_5E310 copies the released holder position before
            // drawing the third random value and assigning its life.
            rng = level2_next_rng16(rng);
            self.session.state.live.economy.entities[usize::from(player_entity_index)]
                .set_entity_rng(rng);
            let life = i32::from(rng % 90) + 200;
            snapshot.set_life(life);
            self.session.state.live.economy.entities[usize::from(holder_index)] = snapshot.clone();
            self.session
                .state
                .live
                .shared_spells
                .sync_released_holder(&snapshot);
        }

        self.session.state.live.economy.entities[usize::from(player_entity_index)]
            .set_entity_rng(rng);
        if let Some(fire) = self.session.state.live.core.context.spell_fire.as_mut() {
            fire.enabled_holder_indices = self
                .session
                .state
                .live
                .shared_spells
                .enabled_spell_holder_indices();
        }
    }

    fn step_level2_player_dying_action(
        &mut self,
        pass: u8,
        source_slot: u16,
        burst_position: Position,
    ) {
        const PLAYER_DEATH_NOTIFICATION_ID: u16 = 374;
        const PLAYER_DEATH_NOTIFICATION_STATE: u8 = 1;
        const PLAYER_DEATH_NOTIFICATION_TIMER: i16 = 100;
        const PLAYER_BANISHED_ACTION_TIMER: i32 = 1_200;

        let player_entity_index = self.session.state.live.core.state.entity_index;
        let (old_fall_velocity, source_id, collision_vertical_offset) = {
            let player =
                &self.session.state.live.economy.entities[usize::from(player_entity_index)];
            let collision_vertical_offset = player
                .parameter_relation
                .and_then(|index| {
                    self.session
                        .definition
                        .presentation
                        .entity_parameters
                        .get(usize::from(index))
                })
                .map(|row| i16::from_le_bytes([row[12], row[13]]))
                .expect("qualified Level 2 player parameter row remains present");
            (
                player.source_word_2c(),
                player.source_id,
                collision_vertical_offset,
            )
        };
        let color_phase_6 =
            ((self.session.state.live.shared_spells.local_player.turn / 6) & 1) != 0;
        if color_phase_6
            && let Some(effects) = self.session.state.live.core.context.effects.as_mut()
        {
            effects.palette_submode = 7;
        }
        if self.session.state.live.player_menu_state != 0 {
            self.session.state.live.player_menu_state = 0;
        }
        let mut position = self.session.state.live.core.state.position;
        position.z = position.z.wrapping_add(old_fall_velocity);
        let next_fall_velocity = old_fall_velocity.wrapping_sub(2).clamp(-256, 0);
        let ground = self
            .session
            .state
            .live
            .terrain
            .ground_height(position)
            .wrapping_add(collision_vertical_offset);
        if position.z < ground {
            position.z = ground;
        }
        {
            let player =
                &mut self.session.state.live.economy.entities[usize::from(player_entity_index)];
            player.set_source_word_2c(next_fall_velocity);
        }
        self.set_level2_entity_position(player_entity_index, position)
            .expect("qualified Level 2 player map carrier remains valid");

        let _ = self.session.state.live.economy.spawn_player_dying_burst(
            player_entity_index,
            source_id,
            burst_position,
        );

        if position.z == ground {
            let reset_free_slots = self.session.baseline.economy.economy.free_slots.clone();
            self.session
                .state
                .live
                .economy
                .rebuild_free_arena(&reset_free_slots);

            let killer = self.session.state.live.economy.entities[usize::from(player_entity_index)]
                .word_24();
            let victim_color = usize::from(
                self.session
                    .state
                    .live
                    .shared_spells
                    .local_player
                    .player_color_index(),
            );
            let qualifying_killer = self
                .session
                .state
                .live
                .economy
                .entities
                .get(usize::from(killer))
                .is_some_and(|entity| {
                    killer != 0 && entity.class == 3 && matches!(entity.model, 0 | 1)
                });
            if victim_color < 8
                && qualifying_killer
                && let Some(player) = self
                    .session
                    .state
                    .live
                    .core
                    .players
                    .iter_mut()
                    .find(|player| player.entity_index == killer)
            {
                let _ = player.increment_kill_score_for_color(victim_color);
                let _ = self
                    .session
                    .state
                    .live
                    .shared_spells
                    .increment_player_kill_score(killer, victim_color);
            }

            self.session.state.live.player_pending_damage = 0;
            self.session.state.live.player_pending_attacker = 0;
            self.session.state.live.economy.entities[usize::from(player_entity_index)]
                .clear_source_upkeep_mailbox();
            self.session
                .state
                .live
                .shared_spells
                .set_player_entity_damage_mailbox(0, 0);
            self.session.state.live.player_death_notification =
                Some(Level2PlayerDeathNotification {
                    id: PLAYER_DEATH_NOTIFICATION_ID,
                    state: PLAYER_DEATH_NOTIFICATION_STATE,
                    timer: PLAYER_DEATH_NOTIFICATION_TIMER,
                });
            self.release_level2_player_spell_holders(position);

            if let Some(marker) = self
                .session
                .state
                .live
                .economy
                .allocate_player_death_marker(player_entity_index, position)
            {
                let player =
                    &mut self.session.state.live.economy.entities[usize::from(player_entity_index)];
                player.set_source_action(3);
                player.set_source_dword_10(PLAYER_BANISHED_ACTION_TIMER);
                self.session
                    .state
                    .live
                    .economy
                    .redirect_player_mana_spheres_to_death_marker(
                        player_entity_index,
                        marker.source_entity_index,
                    );
            }
            self.session.state.live.economy.entities[usize::from(player_entity_index)]
                .set_player_death_hidden_bit();
            self.session
                .state
                .live
                .economy
                .decrement_replacement_cursor_after_player_contact();
        }

        let phase = self.session.state.live.core.state.entity_phase;
        self.session.state.live.economy.entities[usize::from(player_entity_index)].set_phase(phase);
        let player =
            self.session.state.live.economy.entities[usize::from(player_entity_index)].clone();
        self.session
            .state
            .live
            .shared_spells
            .sync_player_entity_from_authority(&player);
        self.append_new_economy_events(pass, source_slot);
    }

    fn step_level2_player_banished_action(&mut self) {
        let player_entity_index = self.session.state.live.core.state.entity_index;
        let is_ai = self.session.state.live.shared_spells.local_player.is_ai;
        if is_ai {
            // The qualified owner route is human. Refuse to run the active
            // flight body for an unclosed AI action-3 branch.
            return;
        }
        if let Some(effects) = self.session.state.live.core.context.effects.as_mut() {
            effects.palette_submode = 7;
        }
        self.session.state.live.core.state.recoil_speed = 0;
        for player in &mut self.session.state.live.core.players {
            if player.entity_index == player_entity_index {
                player.clear_banished_camera_feedback();
            }
        }
        self.session
            .state
            .live
            .shared_spells
            .clear_local_player_banished_camera_feedback();
        let killer =
            self.session.state.live.economy.entities[usize::from(player_entity_index)].word_24();
        let target = (killer != 0)
            .then(|| {
                self.session
                    .state
                    .live
                    .economy
                    .entities
                    .get(usize::from(killer))
                    .map(|entity| entity.position)
            })
            .flatten();
        let state = &mut self.session.state.live.core.state;
        let mut entity_roll = None;
        let mut entity_fov = None;
        if let Some(target) = target {
            let desired_yaw = i16::try_from(heading_for_positions(state.position, target))
                .expect("11-bit banished camera yaw");
            let dx = i32::from(target.x.wrapping_sub(state.position.x).cast_signed());
            let dy = i32::from(target.y.wrapping_sub(state.position.y).cast_signed());
            let horizontal = i32::try_from(crate::legacy_sqrt(
                dx.wrapping_mul(dx)
                    .cast_unsigned()
                    .wrapping_add(dy.wrapping_mul(dy).cast_unsigned()),
            ))
            .expect("wrapped Level 2 camera distance fits i32");
            let desired_pitch = crate::pitch_for_delta(
                horizontal,
                i32::from(target.z.wrapping_sub(state.position.z)),
            );
            entity_roll = Some(desired_yaw);
            entity_fov = Some(desired_pitch);
            state.entity_fov = desired_pitch;
            state.yaw = state
                .yaw
                .wrapping_add(crate::turn_toward(state.yaw, desired_yaw, 22))
                & 0x07ff;
            state.pitch_accumulator = state.pitch_accumulator.wrapping_add(crate::turn_toward(
                state.pitch_accumulator & 0x07ff,
                desired_pitch,
                22,
            )) & 0x07ff;
            state.position.z = self
                .session
                .state
                .live
                .terrain
                .ground_height(state.position);
        } else {
            state.yaw = state.yaw.wrapping_add(5) & 0x07ff;
            state.position.z = self
                .session
                .state
                .live
                .terrain
                .ground_height(state.position)
                .wrapping_add(256);
        }
        state.display_pitch = 0;
        state.pitch_accumulator = 0;
        state.roll_accumulator = 0;
        state.entity_phase = state.entity_phase.wrapping_add(1);
        state.tick = state.tick.wrapping_add(1);
        let position = state.position;
        let yaw = state.yaw;
        let phase = state.entity_phase;
        self.set_level2_entity_position(player_entity_index, position)
            .expect("qualified Level 2 player map carrier remains valid");
        let player =
            &mut self.session.state.live.economy.entities[usize::from(player_entity_index)];
        player.set_yaw(yaw);
        player.set_pitch(0);
        if let Some(roll) = entity_roll {
            player.set_source_roll(roll);
        }
        if let Some(fov) = entity_fov {
            player.set_fov(fov);
        }
        player.set_phase(phase);
        let snapshot = player.clone();
        self.session
            .state
            .live
            .shared_spells
            .sync_player_entity_from_authority(&snapshot);
    }

    #[cfg(test)]
    pub(super) fn step_level2_source_pass(&mut self, pass: u8) {
        let mut evidence = Vec::new();
        self.step_level2_source_pass_with_player(
            pass,
            crate::LatchedCommand::default(),
            &mut evidence,
        )
        .map(|_| ())
        .expect("test source pass has only admitted selectors");
    }

    /// Execute one source outer turn against the Day terrain and reset state.
    pub fn step_outer_turn(
        &mut self,
        command: PlayerCommand,
        speed_index: u8,
    ) -> Vec<crate::StepEvidence> {
        #[cfg(test)]
        CF10_PRODUCTION_SCHEDULE_RECEIPTS.with(|receipts| receipts.borrow_mut().clear());
        let state_before = self.session.state.clone();
        self.session.state.scratch.dispatch_events.clear();
        self.session.state.scratch.nyphur_dispatch_traces.clear();
        self.session.state.scratch.economy_event_cursor =
            self.session.state.live.economy.events().len();
        // The campaign/C ABI flight carrier writes `state.position` before
        // entering the source outer turn. `UpdateEntities_57730` observes
        // that same local-player position from the entity row, including the
        // Castle shift-AABB probe that precedes the player body at slot 111.
        // Synchronize the row before any source callback; the later slot-111
        // write remains responsible for publishing the post-body position.
        self.set_level2_entity_position(
            self.session.state.live.core.state.entity_index,
            self.session.state.live.core.state.position,
        )
        .expect("qualified Level 2 local-player map carrier remains valid");
        // `PlayerEvents_51BB0` handles cheat command 0x1e/9 before
        // `UpdateEntities_57730`: toggle the same session-global bit before
        // any source-pass SetSpell callback can observe it. Other 0x1e
        // subcommands remain outside this bounded packet.
        if command.opcode == 0x1e && command.argument_1 == 9 {
            self.session.state.live.shared_spells.free_spell_usage =
                !self.session.state.live.shared_spells.free_spell_usage;
        }
        self.session
            .state
            .live
            .shared_spells
            .set_beyond_sight_player_input(
                self.session.state.live.core.state.entity_index,
                command.argument_6,
                command.argument_8,
            );
        let (color_phase_1, color_phase_2) = self
            .session
            .state
            .live
            .shared_spells
            .step_day_hud_player_events();
        self.session.state.scratch.level2_day_hud_color_phase_1 = color_phase_1;
        self.session.state.scratch.level2_day_hud_color_phase_2 = color_phase_2;
        self.sync_level2_day_hud_player_projection();
        self.sync_level2_spell_fire_context();
        let context_event_start = self.session.state.live.core.context.events.len();
        let core = self.campaign_state_mut().core_mut();
        let latched_command =
            crate::latch_command_with_context(&mut core.state, command, &mut core.context);
        let context_events =
            self.session.state.live.core.context.events[context_event_start..].to_vec();
        self.append_context_effects(context_events);
        if let Err((source_slot, message)) = self.reconcile_level2_spell_menu_command(command) {
            self.session.state = state_before;
            self.session.state.scratch.dispatch_events.clear();
            self.session.state.scratch.nyphur_dispatch_traces.clear();
            self.session
                .state
                .scratch
                .dispatch_events
                .push(Level2DispatchEvent::Error {
                    pass: 0,
                    source_slot,
                    message,
                });
            return Vec::new();
        }
        let mut evidence = Vec::new();
        // Unsupported speed bytes skip UpdateEntities but the following
        // objective service still observes the extant class-5 lists. Seed the
        // snapshot from current source rows, then replace it with each real
        // pass-start rebuild for the supported 1/4/8-pass modes.
        let mut pass_start_class5_model_present = self
            .session
            .state
            .live
            .economy
            .rebuilt_class5_model_presence();
        for pass in 0..crate::entity_passes_for_speed_index(speed_index) {
            // UpdateEntities performs the source 57F20 reclaim prologue
            // immediately before each entity pass. A landed 10/43/45
            // product therefore remains live and mapped for the rest of its
            // landing pass, then is released at the next pass boundary.
            self.session
                .state
                .live
                .economy
                .reclaim_castle_upgrade_products_at_source_boundary();
            match self.step_level2_source_pass_with_player(
                u8::try_from(pass).unwrap_or(u8::MAX),
                latched_command,
                &mut evidence,
            ) {
                Ok(present) => pass_start_class5_model_present = present,
                Err((source_slot, message)) => {
                    self.session.state = state_before;
                    self.session.state.scratch.dispatch_events.clear();
                    self.session.state.scratch.nyphur_dispatch_traces.clear();
                    self.session
                        .state
                        .scratch
                        .dispatch_events
                        .push(Level2DispatchEvent::Error {
                            pass: u8::try_from(pass).unwrap_or(u8::MAX),
                            source_slot,
                            message,
                        });
                    return Vec::new();
                }
            }
        }
        self.finish_level2_day_hud_frame();
        // remc2 evaluates objectives after the complete entity update and
        // before the next outer turn's authored switch pass.  Keep that
        // ordering so model-32 sees the newly completed stage only on the
        // following turn, exactly as the source does.
        self.advance_level2_objectives(&pass_start_class5_model_present);
        self.flush_shared_effects();
        evidence
    }

    /// Execute one outer turn against an isolated candidate and publish it
    /// only after the complete reset receipt fits the caller's buffer.
    pub(crate) fn step_outer_turn_transactional(
        &mut self,
        expected_generation: u64,
        command: PlayerCommand,
        speed_index: u8,
        output: &mut [u8],
    ) -> std::result::Result<(Vec<crate::StepEvidence>, usize), CampaignStepError> {
        let actual_generation = self.campaign_state().scratch.validity_epoch;
        if expected_generation != actual_generation {
            return Err(CampaignStepError::StaleGeneration {
                expected: expected_generation,
                actual: actual_generation,
            });
        }

        let slot = self.session.state.live.core.runtime.local_player_slot;
        let expected_entity = self.session.state.live.core.state.entity_index;
        let actual_entity = self
            .session
            .state
            .live
            .core
            .players
            .get(usize::from(slot))
            .map(|player| player.entity_index);
        if actual_entity != Some(expected_entity) {
            return Err(CampaignStepError::InvalidLocalPlayerBinding {
                slot,
                expected_entity,
                actual_entity,
            });
        }

        let mut candidate = self.clone();
        let evidence = candidate.step_outer_turn(command, speed_index);
        let observed = ResetState::from_level2(&candidate);
        let written = encode_reset_v1_into(&observed, output).map_err(CampaignStepError::Encode)?;
        let campaign = candidate.campaign_state_mut();
        campaign.scratch.valid = true;
        campaign.scratch.dispatch_receipt = observed.core.effect_journal.receipt();
        campaign.live = observed;
        *self = candidate;
        Ok((evidence, written))
    }

    /// Restore every mutable session projection to the decoded frame zero.
    pub fn reset(&mut self) {
        self.session.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};

    fn level_one_descriptor_fixture_path() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE_V1").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf15-level1-package-current")
            },
            PathBuf::from,
        )
    }

    #[test]
    fn semantic_creature_dispatch_rejects_raw_selector_divergence_in_source_pass() {
        let package = level_one_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor opens");
        let session = campaign.level2_compatibility_mut();
        session
            .consume_authored_group(0, 0, 3)
            .expect("real package group 3 materializes through the shared arena");
        let archer = session
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.authored_index == 101)
            .copied()
            .expect("package-authored Archer is present in the shared arena");
        {
            let mut economy = session.economy_mut();
            let source = economy
                .authored_class5
                .iter_mut()
                .find(|source| source.source_entity_index == archer.source_entity_index)
                .expect("allocated Archer remains in the shared arena");
            source.model = 9;
            source.action = 72;
        }

        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Error {
                    source_slot,
                    message,
                    ..
                } if *source_slot == archer.source_entity_index
                    && message.contains("semantic archer")
                    && message.contains("Model9Family")
            )
        }));
    }

    #[test]
    fn semantic_speed_pickup_becomes_the_same_held_spell_and_rejects_family_divergence() {
        let package = level_one_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor opens");
        let reset_hash = campaign.state_hash();
        let session = campaign.level2_compatibility_mut();

        // Group 11's natural Archer-absence countdown is a separate proven
        // producer. Stage only that package activation boundary here; the
        // ordinary source scheduler owns pickup cadence and acquisition.
        session
            .consume_authored_group(0, 0, 11)
            .expect("real package group 11 materializes its Speed pickup");
        let pickup = session
            .economy()
            .spell_pickups()
            .iter()
            .find(|source| source.authored_index == 100)
            .copied()
            .expect("package row 100 is a live spell pickup");
        assert_eq!(
            session.spell_kind_for_source_slot(pickup.source_entity_index),
            Some(SpellKind::Speed)
        );
        assert_eq!((pickup.family, pickup.action), (3, 10));

        let player_entity_index = session.state.entity_index;
        session
            .set_level2_entity_position(player_entity_index, pickup.position)
            .expect("stage the real player at the real package pickup position");
        let turns_until_cadence = usize::from((4_u8.wrapping_sub(pickup.phase & 3)) & 3) + 1;
        for _ in 1..turns_until_cadence {
            session.step_outer_turn(PlayerCommand::default(), 0);
            assert!(session.dispatch_events().iter().any(|event| matches!(
                event,
                Level2DispatchEvent::SpellAcquisition {
                    source_slot,
                    family: 3,
                    result: None,
                    ..
                } if *source_slot == pickup.source_entity_index
            )));
            let live = session
                .shared_spells()
                .pickup(pickup.source_entity_index)
                .expect("off-cadence pickup remains live");
            assert_eq!((live.model, live.action), (3, 10));
            assert_eq!(session.players()[0].enabled_spell_holders[3], 0);
        }

        session.step_outer_turn(PlayerCommand::default(), 0);
        let acquired = session
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Level2DispatchEvent::SpellAcquisition {
                    source_slot,
                    family: 3,
                    result: Some(outcome),
                    ..
                } if *source_slot == pickup.source_entity_index => Some(*outcome),
                _ => None,
            })
            .expect("the qualifying source visit acquires the package Speed pickup");
        assert_eq!(
            (
                acquired.source_slot,
                acquired.family,
                acquired.holder_action,
                acquired.parent_entity_index,
            ),
            (pickup.source_entity_index, 3, 9, player_entity_index)
        );
        let holder = session
            .shared_spells()
            .pickup(pickup.source_entity_index)
            .expect("the acquired spell preserves the pickup entity");
        assert_eq!(
            (holder.index, holder.model, holder.action),
            (pickup.source_entity_index, 3, 9)
        );
        assert_eq!(
            session.players()[0].enabled_spell_holders[3],
            pickup.source_entity_index
        );
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Level2DispatchEvent::Error { .. }))
        );

        let acquired_phase = holder.phase;
        session.step_outer_turn(PlayerCommand::default(), 0);
        let holder = session
            .shared_spells()
            .pickup(pickup.source_entity_index)
            .expect("the following action-9 visit retains the same holder");
        assert_eq!((holder.model, holder.action), (3, 9));
        assert_eq!(holder.phase, acquired_phase.wrapping_add(1));
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Level2DispatchEvent::Error { .. }))
        );

        campaign.reset();
        assert_eq!(campaign.state_hash(), reset_hash);

        let mut mismatch = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor reopens for the planted negative");
        let mismatch_session = mismatch.level2_compatibility_mut();
        mismatch_session
            .consume_authored_group(0, 0, 11)
            .expect("real package group 11 materializes for the planted negative");
        let mismatch_slot = mismatch_session
            .economy()
            .spell_pickups()
            .iter()
            .find(|source| source.authored_index == 100)
            .map(|source| source.source_entity_index)
            .expect("the semantic Speed pickup has a source slot");
        let mut mismatch_economy = mismatch_session.economy_mut();
        let source = mismatch_economy
            .spell_pickups
            .iter_mut()
            .find(|source| source.source_entity_index == mismatch_slot)
            .expect("the package spell remains in the shared arena");
        source.family = 6;
        source.action = 19;

        mismatch_session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            mismatch_session
                .dispatch_events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::Error {
                        source_slot,
                        message,
                        ..
                    } if *source_slot == mismatch_slot
                        && message.contains("semantic speed")
                        && message.contains("family 6")
                ))
        );
        assert_eq!(mismatch_session.players()[0].enabled_spell_holders[3], 0);
    }

    #[test]
    fn semantic_house_uses_the_ordinary_source_lifecycle_and_rejects_effect_substitution() {
        let package = level_one_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor opens");
        let reset_hash = campaign.state_hash();
        let session = campaign.level2_compatibility_mut();
        let house = session
            .ordinary()
            .houses()
            .iter()
            .find(|house| house.active && house.action == 52)
            .copied()
            .expect("real package contains a live House");
        assert_eq!(
            session.building_kind_for_source_slot(house.entity_index, Level2SourceRoute::House),
            Some(BuildingKind::House)
        );

        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| matches!(
            event,
            Level2DispatchEvent::House { source_slot, .. }
                if *source_slot == house.entity_index
        )));
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Level2DispatchEvent::Error { .. }))
        );
        let visited_house = session
            .ordinary()
            .houses()
            .iter()
            .find(|candidate| candidate.entity_index == house.entity_index)
            .expect("ordinary House remains live after its callback");
        assert_eq!(visited_house.phase, house.phase.wrapping_add(1));

        campaign.reset();
        assert_eq!(campaign.state_hash(), reset_hash);

        let mut mismatch = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor reopens for the planted negative");
        let mismatch_session = mismatch.level2_compatibility_mut();
        let mismatch_slot = mismatch_session
            .ordinary()
            .houses()
            .iter()
            .find(|house| house.active && house.action == 52)
            .map(|house| house.entity_index)
            .expect("real package House has a source slot");
        mismatch_session
            .ordinary_mut()
            .house_mut(mismatch_slot)
            .expect("semantic House carrier remains live")
            .action = 5;

        mismatch_session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            mismatch_session
                .dispatch_events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::Error {
                        source_slot,
                        message,
                        ..
                    } if *source_slot == mismatch_slot
                        && message.contains("semantic house")
                        && message.contains("action 5")
                ))
        );
        assert!(
            mismatch_session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(
                    event,
                    Level2DispatchEvent::House { source_slot, .. } if *source_slot == mismatch_slot
                ))
        );
    }

    #[test]
    fn semantic_area_exit_trigger_activates_its_package_group_and_rejects_entry_substitution() {
        let package = level_one_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor opens");
        let reset_hash = campaign.state_hash();
        let session = campaign.level2_compatibility_mut();
        let trigger = session
            .entities()
            .iter()
            .find(|source| {
                source.active
                    && session.switch_kind_for_source_slot(source.index)
                        == Some(SwitchKind::AreaExitTrigger)
                    && source.source_id == 1
            })
            .cloned()
            .expect("real Level 1 reset contains the group-1 area-exit trigger");
        assert_eq!((trigger.model, trigger.action), (1, 1));

        let turns_until_inside_cadence =
            usize::from((8_u8.wrapping_sub(trigger.phase & 7)) & 7) + 1;
        for _ in 0..turns_until_inside_cadence {
            session.step_outer_turn(PlayerCommand::default(), 0);
        }
        assert!(session.dispatch_events().iter().all(|event| !matches!(
            event,
            Level2DispatchEvent::AuthoredGroupActivated {
                source_slot,
                group: 1,
                ..
            } if *source_slot == trigger.index
        )));
        let inside = &session.entities()[usize::from(trigger.index)];
        assert!(inside.active);
        assert!(!inside.drawing_disabled());

        let player_entity_index = session.state.entity_index;
        session
            .set_level2_entity_position(
                player_entity_index,
                Position {
                    x: trigger.position.x.wrapping_add(20_000),
                    y: trigger.position.y,
                    z: trigger.position.z,
                },
            )
            .expect("stage the real player outside the package trigger extent");
        let phase_before_exit = session.entities()[usize::from(trigger.index)].phase;
        let turns_until_exit_cadence =
            usize::from((8_u8.wrapping_sub(phase_before_exit & 7)) & 7) + 1;
        for _ in 0..turns_until_exit_cadence {
            session.step_outer_turn(PlayerCommand::default(), 0);
        }

        assert!(session.dispatch_events().iter().any(|event| matches!(
            event,
            Level2DispatchEvent::AuthoredGroupActivated {
                source_slot,
                group: 1,
                authored_indices,
                ..
            } if *source_slot == trigger.index
                && authored_indices.starts_with(&[31, 99, 104])
        )));
        assert!(session.dispatch_events().iter().all(|event| !matches!(
            event,
            Level2DispatchEvent::Error { source_slot, .. } if *source_slot == trigger.index
        )));
        let fired = &session.entities()[usize::from(trigger.index)];
        assert!(fired.active);
        assert!(fired.drawing_disabled());
        assert_eq!(
            fired.phase,
            phase_before_exit
                .wrapping_add(u8::try_from(turns_until_exit_cadence).expect("one cadence fits u8"))
        );

        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(!session.entities()[usize::from(trigger.index)].active);
        campaign.reset();
        assert_eq!(campaign.state_hash(), reset_hash);

        let mut mismatch = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor reopens for the planted negative");
        let mismatch_session = mismatch.level2_compatibility_mut();
        let mismatch_slot = mismatch_session
            .entities()
            .iter()
            .find(|source| {
                source.active
                    && mismatch_session.switch_kind_for_source_slot(source.index)
                        == Some(SwitchKind::AreaExitTrigger)
                    && source.source_id == 1
            })
            .map(|source| source.index)
            .expect("semantic area-exit trigger has a source slot");
        mismatch_session.session.state.live.economy.entities[usize::from(mismatch_slot)].action = 0;

        mismatch_session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            mismatch_session
                .dispatch_events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::Error {
                        source_slot,
                        message,
                        ..
                    } if *source_slot == mismatch_slot
                        && message.contains("semantic area_exit_trigger")
                        && message.contains("model 1 action 0")
                ))
        );
        assert!(
            mismatch_session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated {
                        source_slot,
                        group: 1,
                        ..
                    } if *source_slot == mismatch_slot
                ))
        );
    }

    #[test]
    fn semantic_exit_portal_trigger_starts_once_then_reclaims_and_rejects_raw_substitution() {
        let package = level_one_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor opens");
        let reset_hash = campaign.state_hash();
        let session = campaign.level2_compatibility_mut();

        // The preceding objective gate is a separate package-present chain.
        // Stage only its real group-4 boundary; construction, numeric visits,
        // trigger cadence, audio, hide, and reclaim remain production-owned.
        session
            .consume_authored_group(0, 0, 4)
            .expect("real package group 4 materializes through the shared arena");
        let portal = session
            .economy()
            .authored_class14()
            .iter()
            .find(|source| source.authored_index == 132)
            .copied()
            .expect("package row 132 is the exit portal");
        let trigger = session
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|source| source.authored_index == 137)
            .copied()
            .expect("package row 137 is the exit-sequence trigger");
        assert_eq!(
            session.scenery_kind_for_source_slot(portal.source_entity_index),
            Some(SceneryKind::ExitPortal)
        );
        assert_eq!(
            session.switch_kind_for_source_slot(trigger.source_entity_index),
            Some(SwitchKind::ExitSequenceTrigger)
        );
        assert_eq!((portal.model, portal.action), (3, 8));
        assert_eq!(portal.control_flags & 1, 1);
        assert_eq!((trigger.model, trigger.action), (12, 12));

        let player_entity_index = session.state.entity_index;
        assert!(
            player_entity_index < trigger.source_entity_index,
            "the package player cursor must precede the newly allocated exit trigger"
        );
        session
            .set_level2_entity_position(player_entity_index, trigger.position)
            .expect("stage the real player inside the real package trigger");
        let turns_until_cadence = usize::from((8_u8.wrapping_sub(trigger.phase & 7)) & 7) + 1;
        for _ in 1..turns_until_cadence {
            session.step_outer_turn(PlayerCommand::default(), 0);
            assert!(!session.level_exit_sequence().active);
            assert!(session.dispatch_events().iter().all(|event| !matches!(
                event,
                Level2DispatchEvent::LevelExitSequenceStarted { .. }
            )));
        }

        let journal_start = session.effect_journal().records().len();
        session.step_outer_turn(PlayerCommand::default(), 0);
        let exit = session.level_exit_sequence();
        assert_eq!(
            (
                exit.active,
                exit.complete,
                exit.phase,
                exit.timer,
                exit.trigger_entity_index,
                exit.portal_entity_index,
            ),
            (
                true,
                false,
                0,
                0,
                trigger.source_entity_index,
                portal.source_entity_index,
            )
        );
        assert_eq!(
            session
                .economy()
                .authored_class14()
                .iter()
                .find(|source| source.source_entity_index == portal.source_entity_index)
                .expect("the captured exit portal remains in the shared arena")
                .control_flags
                & 1,
            0,
            "the trigger reveals the authoritative portal row"
        );
        assert!(session.dispatch_events().iter().any(|event| matches!(
            event,
            Level2DispatchEvent::LevelExitSequenceStarted {
                trigger_entity_index,
                portal_entity_index,
                ..
            } if *trigger_entity_index == trigger.source_entity_index
                && *portal_entity_index == portal.source_entity_index
        )));
        let new_records = &session.effect_journal().records()[journal_start..];
        let sound_ordinal = new_records
            .iter()
            .position(|record| {
                matches!(
                    record.effect,
                    crate::campaign::journal::CampaignEffect::Flight(FlightEvent::PrepareSound {
                        entity_index,
                        sound_id: 41,
                        ..
                    }) if entity_index == player_entity_index
                )
            })
            .expect("the overlap sound is journaled against the player");
        let start_ordinal = new_records
            .iter()
            .position(|record| {
                matches!(
                    record.effect,
                    crate::campaign::journal::CampaignEffect::Dispatch(
                        Level2DispatchEvent::LevelExitSequenceStarted {
                            trigger_entity_index,
                            ..
                        }
                    ) if trigger_entity_index == trigger.source_entity_index
                )
            })
            .expect("the semantic exit start is journaled");
        assert!(sound_ordinal < start_ordinal);
        assert!(
            new_records.iter().all(|record| !matches!(
                record.effect,
                crate::campaign::journal::CampaignEffect::Flight(FlightEvent::PrepareSound {
                    audio_site: crate::MissionAudioSite::ExitSequenceSwitch,
                    ..
                })
            )),
            "the already-visited player slot cannot run action 12 in the trigger pass"
        );
        let armed_player = &session.entities()[usize::from(player_entity_index)];
        assert_eq!(armed_player.action, crate::level_exit::EXIT_PLAYER_ACTION);
        assert_eq!(armed_player.record_bytes()[74], 0);
        let retained = session
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|source| source.source_entity_index == trigger.source_entity_index)
            .expect("the hidden trigger is retained through its firing pass");
        assert!(retained.active);
        assert!(retained.fired);
        assert!(!retained.drawing_enabled);
        assert_eq!(
            retained.phase,
            trigger
                .phase
                .wrapping_add(u8::try_from(turns_until_cadence).expect("one cadence fits u8"))
        );

        let phase_zero_journal_start = session.effect_journal().records().len();
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .economy()
                .authored_switch_producers()
                .iter()
                .all(|source| source.source_entity_index != trigger.source_entity_index)
        );
        let phase_one = session.level_exit_sequence();
        assert_eq!(
            (
                phase_one.active,
                phase_one.complete,
                phase_one.phase,
                phase_one.palette_submode,
                phase_one.player_status_flags,
            ),
            (
                true,
                false,
                1,
                crate::level_exit::EXIT_PALETTE_ENTRY_FLASH_REQUEST,
                0x20
            )
        );
        let phase_one_player = &session.entities()[usize::from(player_entity_index)];
        assert_eq!(
            phase_one_player.action,
            crate::level_exit::EXIT_PLAYER_ACTION
        );
        assert_eq!(phase_one_player.record_bytes()[74], 1);
        assert!(
            session.effect_journal().records()[phase_zero_journal_start..]
                .iter()
                .any(|record| matches!(
                    record.effect,
                    crate::campaign::journal::CampaignEffect::Flight(FlightEvent::PrepareSound {
                        audio_site: crate::MissionAudioSite::ExitSequenceSwitch,
                        entity_index,
                        sound_id: crate::level_exit::EXIT_SWITCH_SOUND,
                        ..
                    }) if entity_index == player_entity_index
                ))
        );
        assert_eq!(
            session
                .campaign_state()
                .live
                .core
                .context
                .effects
                .as_ref()
                .expect("campaign flight effects remain present")
                .palette_submode,
            crate::level_exit::EXIT_PALETTE_ENTRY_FLASH_REQUEST
        );

        let generation = session.session.generation;
        let mut effect_cursor = session.effect_journal().cursor(generation);
        let mut acceleration_sound_count = 0;
        for _ in 0..700 {
            if session.level_exit_sequence().complete {
                break;
            }
            session.step_outer_turn(PlayerCommand::default(), 0);
            let page = session
                .effect_journal()
                .page(generation, effect_cursor, usize::MAX)
                .expect("exit sequence effects remain page-readable");
            acceleration_sound_count += page
                .records
                .iter()
                .filter(|record| {
                    matches!(
                        record.effect,
                        crate::campaign::journal::CampaignEffect::Flight(
                            FlightEvent::PrepareSound {
                                sound_id: crate::level_exit::EXIT_ACCELERATION_SOUND,
                                ..
                            }
                        )
                    )
                })
                .count();
            effect_cursor = page.next_cursor;
        }
        let completed = session.level_exit_sequence();
        assert_eq!(
            (
                completed.active,
                completed.complete,
                completed.phase,
                completed.result_code,
                completed.player_status_flags,
                completed.display_blank,
                completed.blur_mode,
                completed.camera_fov,
                completed.palette_submode,
            ),
            (
                false,
                true,
                12,
                10,
                10,
                true,
                0,
                121,
                crate::level_exit::EXIT_PALETTE_BLACK_REQUEST,
            )
        );
        assert_eq!(acceleration_sound_count, 1);
        assert_eq!(
            session
                .dispatch_events()
                .iter()
                .filter(|event| matches!(
                    event,
                    Level2DispatchEvent::LevelExitSequenceCompleted {
                        player_entity_index: completed_player,
                        result_code: 10,
                        ..
                    } if *completed_player == player_entity_index
                ))
                .count(),
            1
        );
        let completed_player = &session.entities()[usize::from(player_entity_index)];
        assert_eq!(
            completed_player.action,
            crate::level_exit::EXIT_PLAYER_ACTION
        );
        assert_eq!(completed_player.record_bytes()[74], 12);
        campaign.reset();
        assert_eq!(campaign.state_hash(), reset_hash);
        assert_eq!(
            campaign.level2_compatibility().level_exit_sequence(),
            LevelExitSequence::default()
        );

        let mut mismatch = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor reopens for the planted negative");
        let mismatch_session = mismatch.level2_compatibility_mut();
        mismatch_session
            .consume_authored_group(0, 0, 4)
            .expect("real package group 4 materializes for the planted negative");
        let mismatch_slot = mismatch_session
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|source| source.authored_index == 137)
            .map(|source| source.source_entity_index)
            .expect("the semantic exit trigger has a source slot");
        mismatch_session
            .economy_mut()
            .authored_switch_producers
            .iter_mut()
            .find(|source| source.source_entity_index == mismatch_slot)
            .expect("the package trigger remains in the shared arena")
            .action = 0;

        mismatch_session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            mismatch_session
                .dispatch_events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::Error {
                        source_slot,
                        message,
                        ..
                    } if *source_slot == mismatch_slot
                        && message.contains("semantic exit_sequence_trigger")
                        && message.contains("model 12 action 0")
                ))
        );
        assert_eq!(
            mismatch_session.level_exit_sequence(),
            LevelExitSequence::default()
        );
    }

    #[test]
    fn semantic_tree_dispatch_rejects_raw_stone_selector_in_source_pass() {
        let package = level_one_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor opens");
        let session = campaign.level2_compatibility_mut();
        session
            .consume_authored_group(0, 0, 1)
            .expect("real package group 1 materializes through the shared arena");
        let tree = session
            .economy()
            .authored_trees()
            .iter()
            .find(|source| source.authored_index == 155)
            .copied()
            .expect("package-authored Tree is present in the shared arena");
        {
            let mut economy = session.economy_mut();
            let source = economy
                .authored_trees
                .iter_mut()
                .find(|source| source.source_entity_index == tree.source_entity_index)
                .expect("allocated Tree remains in the shared arena");
            source.action = 3;
        }

        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session.dispatch_events().iter().any(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::Error {
                        source_slot,
                        message,
                        ..
                    } if *source_slot == tree.source_entity_index
                        && message.contains("semantic tree")
                        && message.contains("Stone")
                )
            }),
            "dispatch events: {:?}",
            session.dispatch_events()
        );
    }

    #[test]
    fn package_stone_is_not_misclassified_as_an_area_exit_switch() {
        let package = level_one_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor opens");
        let session = campaign.level2_compatibility_mut();
        let stone_feature = session
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .copied()
            .find(|feature| feature.index == 154)
            .expect("package group 6 contains its authored Stone");
        assert_eq!((stone_feature.kind, stone_feature.subtype), (2, 1));
        let stone_slot = session
            .allocate_authored_stone_feature(stone_feature)
            .expect("the real package Stone materializes in the shared source arena");
        let phase_before = session.entities()[usize::from(stone_slot)].phase;

        session.step_outer_turn(PlayerCommand::default(), 0);

        assert!(session.dispatch_events().iter().all(|event| !matches!(
            event,
            Level2DispatchEvent::Error { source_slot, .. } if *source_slot == stone_slot
        )));
        let stone = &session.entities()[usize::from(stone_slot)];
        assert_eq!((stone.class, stone.model, stone.action), (2, 1, 3));
        assert_eq!(stone.phase, phase_before.wrapping_add(1));
    }

    #[test]
    fn semantic_experience_scroll_collects_and_reclaims_through_the_source_scheduler() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-cf02-campaign-package-v1-a/qualified-level2");
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL2)
            .expect("real Level 2 descriptor opens");
        let reset_hash = campaign.state_hash();
        let session = campaign.level2_compatibility_mut();
        session
            .consume_authored_group(0, 0, 5)
            .expect("real package group 5 materializes through the shared arena");
        let scroll = session
            .economy()
            .authored_class14()
            .iter()
            .find(|source| source.authored_index == 248)
            .copied()
            .expect("package-authored experience scroll is present in the shared arena");
        assert_eq!(
            session.item_kind_for_source_slot(scroll.source_entity_index),
            Some(ItemKind::ExperienceScroll)
        );
        let experience_before = *session.shared_spells().spell_experience();
        let player_entity_index = session.state.entity_index;
        session
            .set_level2_entity_position(player_entity_index, scroll.position)
            .expect("stage the real player at the real package scroll position");

        session.step_outer_turn(PlayerCommand::default(), 0);

        assert!(session.shared_spells().pickup_events().iter().any(|event| {
            matches!(
                event,
                Level2SharedSpellEvent::ExperienceScrollCollected {
                    source_slot,
                    player_entity_index: observed_player,
                } if *source_slot == scroll.source_entity_index
                    && *observed_player == player_entity_index
            )
        }));
        assert_ne!(
            session.shared_spells().spell_experience(),
            &experience_before,
            "the semantic item callback must reach the existing XP consumer"
        );
        let retained = session
            .economy()
            .authored_class14()
            .iter()
            .find(|source| source.source_entity_index == scroll.source_entity_index)
            .expect("collected scroll remains allocated through its callback pass");
        assert!(retained.active);
        assert!(!retained.drawing_enabled);
        assert_eq!(retained.phase, scroll.phase.wrapping_add(1));
        assert!(
            session
                .economy()
                .map_chains()
                .is_linked(scroll.source_entity_index)
        );

        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .economy()
                .authored_class14()
                .iter()
                .any(|source| source.source_entity_index == scroll.source_entity_index)
        );
        assert!(
            !session
                .economy()
                .map_chains()
                .is_linked(scroll.source_entity_index)
        );
        assert!(
            session
                .economy()
                .free_slots
                .contains(&scroll.source_entity_index)
        );

        campaign.reset();
        assert_eq!(campaign.state_hash(), reset_hash);

        let mut mismatch = CampaignSession::open(&package, CampaignSelection::LEVEL2)
            .expect("real Level 2 descriptor reopens for the planted negative");
        let mismatch_session = mismatch.level2_compatibility_mut();
        mismatch_session
            .consume_authored_group(0, 0, 5)
            .expect("real package group 5 materializes for the planted negative");
        let mismatch_scroll_slot = mismatch_session
            .economy()
            .authored_class14()
            .iter()
            .find(|source| source.authored_index == 248)
            .map(|source| source.source_entity_index)
            .expect("the real package scroll has a source slot");
        let mut mismatch_economy = mismatch_session.economy_mut();
        let source = mismatch_economy
            .authored_class14
            .iter_mut()
            .find(|source| source.source_entity_index == mismatch_scroll_slot)
            .expect("the semantic scroll carrier remains mutable in the shared arena");
        source.model = 3;
        source.action = 8;

        mismatch_session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(mismatch_session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Error {
                    source_slot,
                    message,
                    ..
                } if *source_slot == mismatch_scroll_slot
                    && message.contains("semantic experience_scroll")
                    && message.contains("model 3 action 8")
            )
        }));
        assert!(!mismatch_session
            .shared_spells()
            .pickup_events()
            .iter()
            .any(|event| matches!(event, Level2SharedSpellEvent::ExperienceScrollCollected { source_slot, .. } if *source_slot == mismatch_scroll_slot)));
    }

    #[test]
    fn semantic_objective_gate_activates_its_package_group_and_rejects_unbound_rows() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-cf02-campaign-package-v1-a/qualified-level2");
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL2)
            .expect("real Level 2 descriptor opens");
        let reset_hash = campaign.state_hash();
        let session = campaign.level2_compatibility_mut();
        let (gate_slot, gate_stage_index) = session
            .entities()
            .iter()
            .find(|entity| {
                entity.active
                    && entity.class == 11
                    && entity.model == 32
                    && entity.action == 32
                    && entity.source_id == 21
            })
            .map(|entity| {
                (
                    entity.index,
                    usize::try_from(entity.level_gate)
                        .expect("objective-gate stage index fits usize"),
                )
            })
            .expect("the real reset contains the group-21 objective gate");
        assert_eq!(
            session.switch_kind_for_source_slot(gate_slot),
            Some(SwitchKind::ObjectiveGate)
        );

        session.step_outer_turn(PlayerCommand::default(), 0);

        assert!(session.dispatch_events().iter().all(|event| !matches!(
            event,
            Level2DispatchEvent::AuthoredGroupActivated { group: 21, .. }
        )));
        assert!(!session.entities()[usize::from(gate_slot)].drawing_disabled());

        for objective_index in 0..session.session.state.live.core.runtime.objectives.len() {
            session
                .session
                .state
                .live
                .core
                .runtime
                .set_level2_objective_stage_selector_for(objective_index, gate_stage_index, 2);
        }
        session.step_outer_turn(PlayerCommand::default(), 0);

        assert!(session.dispatch_events().iter().any(|event| matches!(
            event,
            Level2DispatchEvent::AuthoredGroupActivated {
                source_slot,
                group: 21,
                authored_indices,
                ..
            } if *source_slot == gate_slot && authored_indices.as_ref() == [229, 230, 231]
        )));
        assert!(session.dispatch_events().iter().all(|event| !matches!(
            event,
            Level2DispatchEvent::Error { source_slot, .. } if *source_slot == gate_slot
        )));
        assert!(session.entities()[usize::from(gate_slot)].drawing_disabled());

        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(!session.entities()[usize::from(gate_slot)].active);
        campaign.reset();
        assert_eq!(campaign.state_hash(), reset_hash);

        let level_one = level_one_descriptor_fixture_path();
        let mut mismatch = CampaignSession::open(&level_one, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor opens for the planted negative");
        let mismatch_session = mismatch.level2_compatibility_mut();
        mismatch_session
            .consume_authored_group(0, 0, 1)
            .expect("real package group 1 materializes its first objective gate");
        let gate = mismatch_session
            .economy()
            .authored_model32()
            .iter()
            .find(|source| source.authored_index == 99)
            .copied()
            .expect("package row 99 is a live objective gate");
        let mut mismatch_economy = mismatch_session.economy_mut();
        let source = mismatch_economy
            .authored_model32
            .iter_mut()
            .find(|source| source.source_entity_index == gate.source_entity_index)
            .expect("the objective gate remains in the shared arena");
        source.authored_index = u16::MAX;

        mismatch_session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            mismatch_session
                .dispatch_events()
                .iter()
                .any(|event| matches!(
                    event,
                    Level2DispatchEvent::Error {
                        source_slot,
                        message,
                        ..
                    } if *source_slot == gate.source_entity_index
                        && message.contains("objective-gate")
                        && message.contains("no package-authored semantic identity")
                ))
        );
    }

    fn level_three_descriptor_fixture_path() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_PACKAGE_LEVEL3_V1").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3")
            },
            PathBuf::from,
        )
    }

    #[test]
    fn cf14_level3_spell_pickups_share_one_entity_and_player_authority() {
        for (source_slot, family, pickup_action, active_action) in
            [(124_u16, 6_u8, 19_u8, 18_u8), (126, 11, 34, 33)]
        {
            let package = level_three_descriptor_fixture_path();
            let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL3)
                .expect("qualified Level 3 descriptor opens");
            let baseline_hash = campaign.state_hash();
            let player_entity_index = campaign
                .level2_compatibility()
                .session
                .state
                .live
                .core
                .state
                .entity_index;
            let baseline_pickup = campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .entities[usize::from(source_slot)]
            .clone();
            assert_eq!(
                (
                    baseline_pickup.class,
                    baseline_pickup.model,
                    baseline_pickup.action
                ),
                (15, family, pickup_action)
            );
            let baseline_player = campaign
                .level2_compatibility()
                .session
                .state
                .live
                .core
                .players
                .iter()
                .find(|player| player.entity_index == player_entity_index)
                .cloned()
                .expect("local Level 3 player row");
            let family_index = usize::from(family);
            let unchanged_offsets = [398, 424, 450, 502, 528];

            campaign
                .level2_compatibility_mut()
                .set_level2_entity_position(player_entity_index, baseline_pickup.position)
                .expect("public local-player input reaches the pickup position");
            for _ in 0..4 {
                let _ = campaign.step(PlayerCommand::default(), 0);
                if campaign
                    .level2_compatibility()
                    .session
                    .state
                    .live
                    .economy
                    .entities[usize::from(source_slot)]
                .action
                    == active_action
                {
                    break;
                }
            }

            let live = &campaign.level2_compatibility().session.state.live;
            let raw_pickup = &live.economy.entities[usize::from(source_slot)];
            let typed_pickup = live
                .shared_spells
                .pickups()
                .iter()
                .find(|pickup| pickup.index == source_slot)
                .expect("typed pickup projection remains addressable");
            assert_eq!(raw_pickup.action, active_action);
            assert_eq!(raw_pickup.record_bytes(), typed_pickup.record_bytes());
            let canonical_player = live
                .core
                .players
                .iter()
                .find(|player| player.entity_index == player_entity_index)
                .expect("canonical local player remains addressable");
            let shared_player = live
                .shared_spells
                .players()
                .iter()
                .find(|player| player.entity_index == player_entity_index)
                .expect("shared local player remains addressable");
            assert_eq!(
                canonical_player.record_bytes(),
                shared_player.record_bytes()
            );
            assert_eq!(
                canonical_player.enabled_spell_holders[family_index],
                source_slot
            );
            assert_eq!(canonical_player.acquired_spell_flags[family_index], 1);
            assert_eq!(canonical_player.record_bytes()[476 + family_index], 1);
            assert_eq!(canonical_player.left_spell_slot_raw, i16::from(family));
            assert_eq!(
                canonical_player.left_subspell_index,
                baseline_player.family_selected_tiers[family_index]
            );
            for offset in unchanged_offsets {
                assert_eq!(
                    canonical_player.record_bytes()[offset + family_index],
                    baseline_player.record_bytes()[offset + family_index],
                    "sub_68FF0 must not write compact player offset {} for family {family}",
                    offset + family_index
                );
            }

            let effects = campaign
                .effects_after(
                    crate::campaign::journal::EffectCursor {
                        generation: campaign.generation(),
                        next_sequence: 0,
                    },
                    16_384,
                )
                .expect("pickup journal")
                .records;
            let sound_sequence = effects
                .iter()
                .find_map(|record| match record.effect {
                    crate::campaign::journal::CampaignEffect::Flight(
                        crate::FlightEvent::PrepareSound {
                            entity_index,
                            player_index: -1,
                            sound_id: 18,
                            ..
                        },
                    ) if entity_index == player_entity_index => Some(record.sequence),
                    _ => None,
                })
                .expect("source pickup sound 18");
            let acquisition_sequence = effects
                .iter()
                .find_map(|record| match record.effect {
                    crate::campaign::journal::CampaignEffect::SharedSpell(
                        Level2SharedSpellEvent::SpellAcquired {
                            source_slot: event_slot,
                            family: event_family,
                            ..
                        },
                    ) if event_slot == source_slot && event_family == family => {
                        Some(record.sequence)
                    }
                    _ => None,
                })
                .expect("typed spell-acquisition receipt");
            assert!(sound_sequence < acquisition_sequence);

            let phase_before = raw_pickup.phase;
            let timer_before = u16::from_le_bytes(
                raw_pickup.record_bytes()[58..60]
                    .try_into()
                    .expect("pickup timer bytes"),
            );
            let _ = campaign.step(PlayerCommand::default(), 0);
            let raw_pickup = &campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .entities[usize::from(source_slot)];
            assert_eq!(raw_pickup.action, active_action);
            assert_eq!(raw_pickup.phase, phase_before.wrapping_add(1));
            assert_eq!(
                u16::from_le_bytes(
                    raw_pickup.record_bytes()[58..60]
                        .try_into()
                        .expect("pickup timer bytes")
                ),
                timer_before.wrapping_sub(1)
            );

            campaign.reset();
            assert_eq!(campaign.state_hash(), baseline_hash);
            assert!(
                campaign
                    .effects_after(
                        crate::campaign::journal::EffectCursor {
                            generation: campaign.generation(),
                            next_sequence: 0,
                        },
                        1,
                    )
                    .expect("reset pickup journal")
                    .records
                    .is_empty()
            );
            let reset_live = &campaign.level2_compatibility().session.state.live;
            assert_eq!(
                reset_live.economy.entities[usize::from(source_slot)].record_bytes(),
                baseline_pickup.record_bytes()
            );
            assert_eq!(
                reset_live
                    .core
                    .players
                    .iter()
                    .find(|player| player.entity_index == player_entity_index)
                    .expect("reset local player")
                    .record_bytes(),
                baseline_player.record_bytes()
            );
        }
    }

    #[test]
    fn cf14_level3_skeleton_damage_death_and_reclaim_follow_source_boundaries() {
        const SKELETON_SLOT: u16 = 93;
        const FIREBALL_DAMAGE: u16 = 500;

        fn skeleton(campaign: &CampaignSession) -> &EntitySnapshot {
            &campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .entities[usize::from(SKELETON_SLOT)]
        }

        fn apply_hit(campaign: &mut CampaignSession, owner_entity_index: u16) {
            let position = skeleton(campaign).position;
            campaign
                .level2_compatibility_mut()
                .apply_level2_class10_model0_damage(
                    level2_dynamic_economy::Level2Class10Model0Step {
                        live: true,
                        first_impact: true,
                        damage_pulse: true,
                        owner_entity_index,
                        payload: FIREBALL_DAMAGE,
                        position,
                    },
                );
            assert_eq!(
                skeleton(campaign).source_upkeep_word_62(),
                owner_entity_index
            );
            assert_eq!(skeleton(campaign).source_upkeep_payload(), 500);
        }

        fn step(campaign: &mut CampaignSession) {
            let _ = campaign.step(PlayerCommand::default(), 0);
            assert!(
                campaign
                    .level2_compatibility()
                    .dispatch_events()
                    .iter()
                    .all(|event| !matches!(event, Level2DispatchEvent::Error { .. })),
                "Skeleton lifecycle dispatch errors: {:?}",
                campaign.level2_compatibility().dispatch_events()
            );
        }

        let package = level_three_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL3)
            .expect("qualified Level 3 descriptor opens");
        let baseline_hash = campaign.state_hash();
        let player_entity_index = campaign
            .level2_compatibility()
            .session
            .state
            .live
            .core
            .state
            .entity_index;
        assert_eq!(
            (skeleton(&campaign).class, skeleton(&campaign).model),
            (5, 9)
        );
        assert_eq!(skeleton(&campaign).life, 1_000);
        assert_eq!(skeleton(&campaign).mana, 500);

        for _ in 0..24 {
            if skeleton(&campaign).action == 79 {
                break;
            }
            step(&mut campaign);
        }
        assert_eq!(
            skeleton(&campaign).action,
            79,
            "the real Level 3 countdown reaches its source movement callback"
        );

        let first_phase = skeleton(&campaign).phase;
        let sphere_count = campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .economy
            .spheres()
            .len();
        let burst_count = campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .economy
            .death_bursts()
            .len();
        apply_hit(&mut campaign, player_entity_index);
        step(&mut campaign);
        let first_hit = skeleton(&campaign);
        assert_eq!(first_hit.life, 500);
        assert_eq!(first_hit.action, 74);
        assert_eq!(first_hit.word_26(), player_entity_index);
        assert_eq!(first_hit.particle_parameter_index, Some(202));
        assert_eq!(first_hit.phase, first_phase.wrapping_add(1));
        assert_eq!(first_hit.source_upkeep_word_62(), 0);
        assert!(!first_hit.drawing_disabled());
        assert_eq!(
            campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .economy
                .spheres()
                .len(),
            sphere_count,
            "a nonlethal hit must not create death outputs"
        );
        assert_eq!(
            campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .economy
                .death_bursts()
                .len(),
            burst_count
        );

        apply_hit(&mut campaign, player_entity_index);
        let second_phase = skeleton(&campaign).phase;
        step(&mut campaign);
        assert_eq!(
            (
                skeleton(&campaign).life,
                skeleton(&campaign).action,
                skeleton(&campaign).source_upkeep_word_62(),
                skeleton(&campaign).source_upkeep_payload(),
                skeleton(&campaign).phase,
            ),
            (0, 74, 0, 500, second_phase.wrapping_add(1))
        );
        assert!(!skeleton(&campaign).drawing_disabled());

        apply_hit(&mut campaign, player_entity_index);
        let lethal_phase = skeleton(&campaign).phase;
        step(&mut campaign);
        assert_eq!(skeleton(&campaign).life, -500);
        assert_eq!(skeleton(&campaign).action, 76);
        assert_eq!(skeleton(&campaign).word_24(), player_entity_index);
        assert_eq!(skeleton(&campaign).word_26(), player_entity_index);
        assert_eq!(skeleton(&campaign).phase, lethal_phase.wrapping_add(1));
        assert!(!skeleton(&campaign).drawing_disabled());

        let pre_kill_phase = skeleton(&campaign).phase;
        step(&mut campaign);
        assert_eq!(skeleton(&campaign).action, 77);
        assert_eq!(skeleton(&campaign).phase, pre_kill_phase.wrapping_add(1));

        let source_rng_before_kill = skeleton(&campaign).entity_rng;
        let mut kill_phase = None;
        for _ in 0..8 {
            let phase_before = skeleton(&campaign).phase;
            step(&mut campaign);
            if skeleton(&campaign).drawing_disabled() {
                kill_phase = Some(phase_before);
                break;
            }
            assert_eq!(skeleton(&campaign).action, 77);
            assert_eq!(skeleton(&campaign).phase, phase_before.wrapping_add(1));
            assert_eq!(
                campaign
                    .level2_compatibility()
                    .session
                    .state
                    .live
                    .economy
                    .economy
                    .spheres()
                    .len(),
                sphere_count
            );
        }
        let kill_phase = kill_phase.expect("action 77 reaches its eight-phase kill cadence");
        assert_eq!(kill_phase & 7, 0);
        let hidden = skeleton(&campaign);
        assert!(
            hidden.active,
            "hide is retained for the rest of the source pass"
        );
        assert_eq!(hidden.action, 77);
        assert_eq!(hidden.phase, kill_phase.wrapping_add(1));
        assert_eq!(
            hidden.entity_rng,
            campaign_shared_families::next_source_rng16(source_rng_before_kill)
        );
        assert_eq!(hidden.source_player_entity_index(), 0);

        let economy = &campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .economy;
        assert_eq!(economy.spheres().len(), sphere_count + 1);
        let sphere = economy.spheres().last().expect("Skeleton mana sphere");
        assert_eq!(sphere.mana, 500);
        assert_eq!(sphere.owner_entity_index, 0);
        assert_eq!(economy.death_bursts().len(), burst_count + 1);
        let burst = economy.death_bursts().last().expect("Skeleton death burst");
        assert_eq!(burst.parent_entity_index, SKELETON_SLOT);
        assert_eq!(burst.id_entity_index, SKELETON_SLOT);

        step(&mut campaign);
        let reclaimed = skeleton(&campaign);
        assert!(!reclaimed.active);
        assert_eq!(reclaimed.class, 0);
        assert!(
            !campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .economy
                .map_chains()
                .linked_entity_indices()
                .contains(&SKELETON_SLOT)
        );

        campaign.reset();
        assert_eq!(campaign.state_hash(), baseline_hash);
        assert_eq!(
            (skeleton(&campaign).class, skeleton(&campaign).model),
            (5, 9)
        );
        assert_eq!(skeleton(&campaign).life, 1_000);
        assert_eq!(skeleton(&campaign).action, 72);
    }

    #[test]
    fn cf14_level3_ballistic_death_spawns_then_reclaims_on_production_boundaries() {
        fn arm_lethal_contact(campaign: &mut CampaignSession) -> (u8, u16, u32, usize) {
            let live = &mut campaign.level2_compatibility_mut().session.state.live;
            let source = &mut live.economy.entities[245];
            assert_eq!((source.class, source.model, source.action), (2, 8, 21));
            let mut position = source.position;
            position.z = live.terrain.ground_height(position);
            source.set_position(position);
            source.set_control_flags(0);
            source.set_actual_speed(0);
            source.set_source_word_2c(0);
            source.set_life(50);
            source.set_source_upkeep_payload(100);
            source.record[102..104].copy_from_slice(&7_u16.to_le_bytes());
            (
                source.phase,
                source.entity_rng,
                live.economy.global_rng,
                live.economy.economy.model59_smoke_children().len(),
            )
        }

        let package = level_three_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL3)
            .expect("qualified Level 3 descriptor opens");
        let reset_hash = campaign.state_hash();
        let (phase_before, entity_rng_before, global_rng_before, smoke_count_before) =
            arm_lethal_contact(&mut campaign);
        let next16 = |seed: u16| seed.wrapping_mul(9_377).wrapping_add(9_439);
        let next32 = |seed: u32| seed.wrapping_mul(9_377).wrapping_add(9_439);

        let first_evidence = campaign.step(PlayerCommand::default(), 0);
        let first_state_hash = campaign.state_hash();
        let first_events = campaign.events().to_vec();
        let source = &campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[245];
        assert_eq!(source.life, -50);
        assert!(source.drawing_disabled());
        assert!(source.active, "hide is retained until the next prepass");
        assert_eq!(source.phase, phase_before.wrapping_add(1));
        assert_eq!(
            source.entity_rng,
            next16(next16(next16(entity_rng_before))),
            "grounded lethal contact consumes the exact three local draws"
        );
        assert_eq!(source.source_upkeep_word_62(), 0);
        assert_eq!(
            campaign.level2_compatibility().source_global_rng(),
            next32(next32(global_rng_before)),
            "UpdateEntities and the smoke constructor each consume one global draw"
        );
        let smoke_children = campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .economy
            .model59_smoke_children();
        assert_eq!(smoke_children.len(), smoke_count_before + 1);
        assert!(smoke_children.last().is_some_and(|child| child.active));

        let _ = campaign.step(PlayerCommand::default(), 0);
        let reclaimed = &campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[245];
        assert!(!reclaimed.active);
        assert_eq!(reclaimed.class, 0);
        assert!(
            !campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .economy
                .map_chains()
                .linked_entity_indices()
                .contains(&245)
        );

        campaign.reset();
        assert_eq!(campaign.state_hash(), reset_hash);
        let _ = arm_lethal_contact(&mut campaign);
        let replay_evidence = campaign.step(PlayerCommand::default(), 0);
        assert_eq!(replay_evidence, first_evidence);
        assert_eq!(campaign.events(), first_events.as_slice());
        assert_eq!(campaign.state_hash(), first_state_hash);

        // `SetParticleSmoke3B` advances the global stream before NewEvent.
        // When both allocation paths are exhausted, the source still hides,
        // phase-advances, and reclaims the ballistic row; only the child is
        // absent and the constructor draw remains observable.
        campaign.reset();
        let (phase_before, entity_rng_before, global_rng_before, smoke_count_before) =
            arm_lethal_contact(&mut campaign);
        {
            let economy = &mut campaign
                .level2_compatibility_mut()
                .session
                .state
                .live
                .economy
                .economy;
            economy.free_slots.clear();
            for _ in 0..1_000 {
                economy.decrement_replacement_cursor_after_player_contact();
            }
        }
        let _ = campaign.step(PlayerCommand::default(), 0);
        let live = &campaign.level2_compatibility().session.state.live;
        let source = &live.economy.entities[245];
        assert!(source.active && source.drawing_disabled());
        assert_eq!(source.phase, phase_before.wrapping_add(1));
        assert_eq!(source.entity_rng, next16(next16(next16(entity_rng_before))));
        assert_eq!(
            live.economy.economy.model59_smoke_children().len(),
            smoke_count_before
        );
        assert_eq!(
            live.economy.economy.global_rng,
            next32(next32(global_rng_before))
        );
        let _ = campaign.step(PlayerCommand::default(), 0);
        let source = &campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[245];
        assert!(!source.active);
        assert_eq!(source.class, 0);
    }

    #[test]
    fn cf14_level3_countdown_fires_group_once_then_reclaims_on_production_boundary() {
        fn remove_blocking_roots(campaign: &mut CampaignSession) {
            let roots = campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .entities
                .iter()
                .filter(|row| row.active && row.class == 5 && row.model == 3 && row.action == 31)
                .map(|row| row.index)
                .collect::<Vec<_>>();
            assert_eq!(roots.len(), 5);
            for slot in roots {
                campaign
                    .level2_compatibility_mut()
                    .session
                    .state
                    .live
                    .economy
                    .entities[usize::from(slot)]
                .disable_drawing();
            }
        }

        fn fire(campaign: &mut CampaignSession) {
            remove_blocking_roots(campaign);
            for _ in 0..17 {
                let _ = campaign.step(PlayerCommand::default(), 0);
            }
        }

        fn exhaust_source_allocator(campaign: &mut CampaignSession) {
            let session = campaign.level2_compatibility_mut();
            std::sync::Arc::make_mut(&mut session.session.baseline)
                .economy
                .economy
                .free_slots
                .clear();
            let economy = &mut session.session.state.live.economy.economy;
            economy.free_slots.clear();
            for row in &mut economy.castle_children {
                row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
            }
            macro_rules! make_nonreplaceable {
                ($rows:expr) => {
                    for row in &mut $rows {
                        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
                    }
                };
            }
            make_nonreplaceable!(economy.spheres);
            make_nonreplaceable!(economy.death_bursts);
            make_nonreplaceable!(economy.authored_model1);
            make_nonreplaceable!(economy.model59_smoke_children);
            make_nonreplaceable!(economy.authored_model5_water_splashes);
            make_nonreplaceable!(economy.authored_model13);
            make_nonreplaceable!(economy.authored_model6);
            make_nonreplaceable!(economy.model13_spawned_children);
            make_nonreplaceable!(economy.model17_spawned_children);
            make_nonreplaceable!(economy.model76_tail_spawns);
        }

        let package = level_three_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL3)
            .expect("qualified Level 3 descriptor opens");
        let reset_hash = campaign.state_hash();
        let phase_before = campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[247]
            .phase;
        fire(&mut campaign);

        let producer = &campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[247];
        assert_eq!(producer.source_dword_10(), 1);
        assert!(producer.drawing_disabled());
        assert!(producer.active);
        assert_eq!(producer.phase, phase_before.wrapping_add(17));
        let constructed = campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .economy
            .authored_class14()
            .iter()
            .filter(|row| (247..=256).contains(&row.authored_index))
            .count();
        assert_eq!(constructed, 10);

        let effects = campaign
            .effects_after(
                crate::campaign::journal::EffectCursor {
                    generation: campaign.generation(),
                    next_sequence: 0,
                },
                16_384,
            )
            .expect("complete countdown journal page");
        let sound_sequence = effects
            .records
            .iter()
            .find_map(|record| match record.effect {
                crate::campaign::journal::CampaignEffect::Flight(
                    crate::FlightEvent::PrepareSound {
                        entity_index: 247,
                        player_index: -1,
                        sound_id: 41,
                        ..
                    },
                ) => Some(record.sequence),
                _ => None,
            })
            .expect("countdown sound 41");
        let activation_sequence = effects
            .records
            .iter()
            .find_map(|record| match &record.effect {
                crate::campaign::journal::CampaignEffect::Dispatch(
                    crate::native_world::Level2DispatchEvent::AuthoredGroupActivated {
                        source_slot: 247,
                        group: 11,
                        authored_indices,
                        ..
                    },
                ) if authored_indices.as_ref() == (247_u16..=256).collect::<Vec<_>>() => {
                    Some(record.sequence)
                }
                _ => None,
            })
            .expect("countdown group-11 activation");
        assert!(sound_sequence < activation_sequence);

        let _ = campaign.step(PlayerCommand::default(), 0);
        let final_hash = campaign.state_hash();
        let final_effects = campaign
            .effects_after(
                crate::campaign::journal::EffectCursor {
                    generation: campaign.generation(),
                    next_sequence: 0,
                },
                16_384,
            )
            .expect("complete post-cleanup journal page")
            .records;
        let producer = &campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[247];
        assert!(!producer.active);
        assert_eq!(producer.class, 0);
        assert_eq!(
            final_effects
                .iter()
                .filter(|record| matches!(
                    record.effect,
                    crate::campaign::journal::CampaignEffect::Flight(
                        crate::FlightEvent::PrepareSound {
                            entity_index: 247,
                            sound_id: 41,
                            ..
                        }
                    )
                ))
                .count(),
            1
        );

        campaign.reset();
        assert_eq!(campaign.state_hash(), reset_hash);
        fire(&mut campaign);
        let _ = campaign.step(PlayerCommand::default(), 0);
        assert_eq!(campaign.state_hash(), final_hash);
        assert_eq!(
            campaign
                .effects_after(
                    crate::campaign::journal::EffectCursor {
                        generation: campaign.generation(),
                        next_sequence: 0,
                    },
                    16_384,
                )
                .expect("replayed countdown journal")
                .records,
            final_effects
        );

        // `sub_4A1E0(id, 1)` consumes every authored row even when NewEvent
        // cannot allocate it. Exhaust both the reconstructed reset pool and
        // the live pool after the five multipart roots have been reclaimed;
        // the countdown still sounds, journals all ten attempted rows, hides,
        // phase-advances, and reclaims on the next source boundary.
        campaign.reset();
        remove_blocking_roots(&mut campaign);
        let _ = campaign.step(PlayerCommand::default(), 0);
        assert_eq!(
            campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .entities[247]
                .source_dword_10(),
            16
        );
        {
            let live = &mut campaign.level2_compatibility_mut().session.state.live;
            for row in &mut live.economy.entities {
                if row.class == 5 && row.model == 3 {
                    row.reclaim_source_row();
                }
            }
            live.economy.entities[247].set_source_dword_10(1);
        }
        exhaust_source_allocator(&mut campaign);
        let _ = campaign.step(PlayerCommand::default(), 0);
        let producer = &campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[247];
        assert!(producer.active && producer.drawing_disabled());
        assert_eq!(producer.phase, phase_before.wrapping_add(2));
        assert_eq!(
            campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .economy
                .authored_class14()
                .iter()
                .filter(|row| (247..=256).contains(&row.authored_index))
                .count(),
            0
        );
        let exhausted_effects = campaign
            .effects_after(
                crate::campaign::journal::EffectCursor {
                    generation: campaign.generation(),
                    next_sequence: 0,
                },
                16_384,
            )
            .expect("exhausted countdown journal")
            .records;
        assert!(exhausted_effects.iter().any(|record| matches!(
            &record.effect,
            crate::campaign::journal::CampaignEffect::Dispatch(
                crate::native_world::Level2DispatchEvent::AuthoredGroupActivated {
                    source_slot: 247,
                    group: 11,
                    authored_indices,
                    ..
                }
            ) if authored_indices.as_ref() == (247_u16..=256).collect::<Vec<_>>()
        )));
        let _ = campaign.step(PlayerCommand::default(), 0);
        assert!(
            !campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .entities[247]
                .active
        );
    }

    #[test]
    fn cf14_level3_terrain_controller_and_triggers_preserve_numeric_order() {
        fn install_trigger(
            campaign: &mut CampaignSession,
            slot: u16,
            model: u8,
            action: u8,
            position: crate::Position,
        ) {
            {
                let row = &mut campaign
                    .level2_compatibility_mut()
                    .session
                    .state
                    .live
                    .economy
                    .entities[usize::from(slot)];
                row.active = true;
                row.record[2] = 1;
                row.class = 10;
                row.model = model;
                row.action = action;
                row.record[67] = 10;
                row.record[68] = model;
                row.record[73] = action;
                row.set_control_flags(0);
                row.set_life(1);
            }
            campaign
                .level2_compatibility_mut()
                .set_level2_entity_position(slot, position)
                .expect("trigger joins the controller map cell");
        }

        let package = level_three_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL3)
            .expect("qualified Level 3 descriptor opens");
        let reset_hash = campaign.state_hash();
        let controller_position = campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[94]
            .position;
        let terrain_rng = campaign
            .level2_compatibility()
            .session
            .state
            .live
            .terrain
            .terrain_rng();
        let controller_rng = campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[94]
            .entity_rng;

        // A lower-slot close trigger mutates the controller before its visit,
        // so the first callback performs one close tick in this same scan.
        install_trigger(&mut campaign, 93, 63, 68, controller_position);
        let terrain_before = campaign.terrain_hash();
        let _ = campaign.step(PlayerCommand::default(), 0);
        let controller = &campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[94];
        assert_eq!((controller.life, controller.subspell_index), (2, 47));
        assert_ne!(campaign.terrain_hash(), terrain_before);
        assert_eq!(controller.entity_rng, controller_rng);
        assert_eq!(
            campaign
                .level2_compatibility()
                .session
                .state
                .live
                .terrain
                .terrain_rng(),
            terrain_rng
        );
        assert!(
            campaign
                .effects_after(
                    crate::campaign::journal::EffectCursor {
                        generation: campaign.generation(),
                        next_sequence: 0,
                    },
                    16_384,
                )
                .expect("terrain close journal")
                .records
                .iter()
                .any(|record| matches!(
                    record.effect,
                    crate::campaign::journal::CampaignEffect::Flight(
                        crate::FlightEvent::PrepareSound {
                            entity_index: 94,
                            sound_id: 47,
                            ..
                        }
                    )
                ))
        );
        assert!(
            campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .entities[93]
                .drawing_disabled()
        );
        let _ = campaign.step(PlayerCommand::default(), 0);
        assert!(
            !campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .entities[93]
                .active
        );
        let close_hash = campaign.state_hash();
        let close_effects = campaign
            .effects_after(
                crate::campaign::journal::EffectCursor {
                    generation: campaign.generation(),
                    next_sequence: 0,
                },
                16_384,
            )
            .expect("complete terrain close replay journal")
            .records;

        campaign.reset();
        assert_eq!(campaign.state_hash(), reset_hash);
        {
            let controller = &mut campaign
                .level2_compatibility_mut()
                .session
                .state
                .live
                .economy
                .entities[94];
            controller.set_life(4);
            controller.set_subspell_index(0);
        }
        // A higher-slot open trigger runs after the controller. Its life write
        // becomes visible to the controller only on the following scan.
        install_trigger(&mut campaign, 95, 64, 69, controller_position);
        let _ = campaign.step(PlayerCommand::default(), 0);
        let controller = &campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[94];
        assert_eq!((controller.life, controller.subspell_index), (1, 0));
        let terrain_before_open = campaign.terrain_hash();
        let _ = campaign.step(PlayerCommand::default(), 0);
        let controller = &campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .entities[94];
        assert_eq!((controller.life, controller.subspell_index), (1, 1));
        assert_ne!(campaign.terrain_hash(), terrain_before_open);
        assert!(
            !campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .entities[95]
                .active
        );

        campaign.reset();
        assert_eq!(campaign.state_hash(), reset_hash);
        install_trigger(&mut campaign, 93, 63, 68, controller_position);
        let _ = campaign.step(PlayerCommand::default(), 0);
        let _ = campaign.step(PlayerCommand::default(), 0);
        assert_eq!(campaign.state_hash(), close_hash);
        assert_eq!(
            campaign
                .effects_after(
                    crate::campaign::journal::EffectCursor {
                        generation: campaign.generation(),
                        next_sequence: 0,
                    },
                    16_384,
                )
                .expect("replayed terrain close journal")
                .records,
            close_effects
        );

        // The registry key is class/action, but the imported callback still
        // validates the constructor model. A model-63 row carrying action 69
        // is a malformed carrier: the whole pass rolls back before callback,
        // epilogue, terrain, RNG, or journal mutation.
        campaign.reset();
        install_trigger(&mut campaign, 93, 63, 69, controller_position);
        let invalid_hash = campaign.state_hash();
        let invalid_global_rng = campaign.level2_compatibility().source_global_rng();
        let evidence = campaign.step(PlayerCommand::default(), 0);
        assert!(evidence.is_empty());
        assert_eq!(campaign.state_hash(), invalid_hash);
        assert_eq!(
            campaign.level2_compatibility().source_global_rng(),
            invalid_global_rng
        );
        assert!(
            !campaign
                .level2_compatibility()
                .session
                .state
                .live
                .economy
                .entities[93]
                .drawing_disabled()
        );
        assert!(matches!(
            campaign.level2_compatibility().dispatch_events(),
            [crate::native_world::Level2DispatchEvent::Error {
                source_slot: 93,
                message,
                ..
            }] if message.contains("rejects source selector 10/63/69")
        ));
    }

    fn level_two_nyphur_possession_session() -> Level2ResetSession {
        let package = super::super::tests::level_two_fixture_path();
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component")
            .level2_reset_session()
            .expect("qualified Level 2 Nyphur Possession session")
    }

    fn install_nyphur_possession_profile(session: &mut Level2ResetSession, tier: u8) {
        let profile = level2_direct_starting_spell_profile(POSSESSION_SPELL_FAMILY, tier)
            .expect("family-1 profile exists for the physical tier");
        let holder = session
            .economy
            .entities
            .get_mut(usize::from(LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX))
            .expect("qualified Nyphur Possession holder row");
        holder.level_gate = tier.cast_signed();
        holder.subspell_index = profile.subspell_index;
        holder.activation_value = profile.word_0x30;
        holder.byte_3b = profile.byte_0x3b;
        holder.mana_regen = profile.mana_regen;
        holder.max_mana = profile.max_mana;
        holder.mana = profile.mana;
        holder.active_value = profile.word_0x30.cast_signed();
        holder.record[46..48].copy_from_slice(&profile.subspell_index.to_le_bytes());
        holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
        holder.record[52..54].copy_from_slice(&profile.word_0x30.to_le_bytes());
        holder.record[63] = profile.byte_0x3b;
        holder.record[74] = tier;
        holder.record[140..144].copy_from_slice(&profile.mana_regen.to_le_bytes());
        holder.record[144..148].copy_from_slice(&profile.max_mana.to_le_bytes());
        holder.record[148..152].copy_from_slice(&profile.mana.to_le_bytes());
    }

    #[test]
    fn level_two_nyphur_possession_distinguishes_tiered_and_basic_launches() {
        for (tier, expected_model, expected_action, expected_byte_0x44) in
            [(0, 1, 1, 12), (1, 17, 18, 54)]
        {
            let mut session = level_two_nyphur_possession_session();
            session.select_level2_campaign_entry_stage();
            install_nyphur_possession_profile(&mut session, tier);
            let baseline_hash = session.campaign_state_hash();

            session.step_outer_turn(PlayerCommand::default(), 0);

            let projectile = session
                .economy()
                .possession_projectiles()
                .iter()
                .find(|projectile| {
                    projectile.holder_entity_index == LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX
                })
                .copied()
                .expect("ordinary Nyphur holder callback allocates one live projectile");
            assert_eq!(
                (
                    projectile.source_class,
                    projectile.source_model,
                    projectile.action,
                    projectile.output_model,
                ),
                (9, expected_model, expected_action, expected_byte_0x44),
                "physical tier {tier} preserves the pinned constructor identity"
            );
            assert_eq!(
                session
                    .shared_spells()
                    .activation_events()
                    .iter()
                    .filter(|event| matches!(
                        event,
                        FlightEvent::PrepareSound {
                            entity_index,
                            player_index: -1,
                            sound_id: 40,
                            ..
                        } if *entity_index == projectile.source_entity_index
                    ))
                    .count(),
                1,
                "physical tier {tier} emits the source sound only after allocation"
            );
            assert_ne!(session.campaign_state_hash(), baseline_hash);
        }
    }

    #[test]
    fn level_two_nyphur_possession_tiered_callback_eases_speed() {
        let mut session = level_two_nyphur_possession_session();
        session.select_level2_campaign_entry_stage();
        install_nyphur_possession_profile(&mut session, 1);
        session.step_outer_turn(PlayerCommand::default(), 0);
        let projectile_slot = session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|projectile| {
                projectile.holder_entity_index == LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX
            })
            .map(|projectile| projectile.source_entity_index)
            .expect("ordinary tiered launch remains in the dynamic arena");
        session
            .economy_mut()
            .possession_projectiles
            .iter_mut()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .expect("ordinary tiered projectile remains addressable")
            .actual_speed = 400;

        session.step_outer_turn(PlayerCommand::default(), 0);
        let projectile = session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .copied()
            .expect("following-pass tiered callback preserves the source row");
        assert_eq!(projectile.source_model, 17);
        assert_eq!(projectile.action, 18);
        assert_eq!(projectile.actual_speed, 398);
    }

    #[test]
    fn level_two_nyphur_possession_tiered_launch_replays_stably() {
        let mut session = level_two_nyphur_possession_session();
        let mut first_receipt = None;
        for _ in 0..2 {
            session.select_level2_campaign_entry_stage();
            install_nyphur_possession_profile(&mut session, 1);
            session.step_outer_turn(PlayerCommand::default(), 0);
            let projectile = session
                .economy()
                .possession_projectiles()
                .iter()
                .find(|projectile| {
                    projectile.holder_entity_index == LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX
                })
                .copied()
                .expect("ordinary tiered launch remains in the source arena");
            let sound_entities = session
                .shared_spells()
                .activation_events()
                .iter()
                .filter_map(|event| match event {
                    FlightEvent::PrepareSound {
                        entity_index,
                        player_index: -1,
                        sound_id: 40,
                        ..
                    } => Some(*entity_index),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(sound_entities, vec![projectile.source_entity_index]);
            let receipt = (
                projectile.source_entity_index,
                projectile.source_model,
                projectile.action,
                projectile.output_model,
                projectile.position,
                projectile.axis_position,
                projectile.yaw,
                projectile.pitch,
                projectile.actual_speed,
                session.campaign_state_hash(),
            );
            if let Some(expected) = first_receipt {
                assert_eq!(
                    receipt, expected,
                    "tiered launch resets and replays identically"
                );
            } else {
                first_receipt = Some(receipt);
            }
            session.reset();
        }
    }

    #[test]
    fn level_two_nyphur_possession_tiered_negatives_emit_no_sound_or_projectile() {
        let mut invalid = level_two_nyphur_possession_session();
        invalid.select_level2_campaign_entry_stage();
        install_nyphur_possession_profile(&mut invalid, 0);
        let holder = invalid
            .economy
            .entities
            .get_mut(usize::from(LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX))
            .expect("qualified Nyphur Possession holder row");
        holder.level_gate = 1;
        holder.record[74] = 1;
        invalid.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            invalid
                .shared_spells()
                .activation_events()
                .iter()
                .all(|event| !matches!(event, FlightEvent::PrepareSound { sound_id: 40, .. }))
        );
        assert!(
            invalid
                .economy()
                .possession_projectiles()
                .iter()
                .all(|projectile| !projectile.active)
        );

        let mut allocation_failure = level_two_nyphur_possession_session();
        allocation_failure.select_level2_campaign_entry_stage();
        install_nyphur_possession_profile(&mut allocation_failure, 1);
        allocation_failure.economy.free_slots.clear();
        allocation_failure.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            allocation_failure
                .shared_spells()
                .activation_events()
                .iter()
                .all(|event| !matches!(event, FlightEvent::PrepareSound { sound_id: 40, .. }))
        );
        assert!(
            allocation_failure
                .economy()
                .possession_projectiles()
                .iter()
                .all(|projectile| !projectile.active)
        );

        let mut tier_two_without_castle = level_two_nyphur_possession_session();
        tier_two_without_castle.select_level2_campaign_entry_stage();
        install_nyphur_possession_profile(&mut tier_two_without_castle, 2);
        tier_two_without_castle.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            tier_two_without_castle
                .shared_spells()
                .activation_events()
                .iter()
                .all(|event| !matches!(event, FlightEvent::PrepareSound { sound_id: 40, .. }))
        );
        assert!(
            tier_two_without_castle
                .economy()
                .possession_projectiles()
                .iter()
                .all(|projectile| !projectile.active)
        );
    }

    #[test]
    fn level_two_player_death_marker_failure_commits_contact_tail_and_retries_action2() {
        let package = super::super::tests::level_two_fixture_path();
        let mut session = NativeWorld::inspect_level2(package)
            .expect("qualified Level 2 package")
            .level2_reset_session()
            .expect("Level 2 marker-failure session");
        let player_entity_index = session.state.entity_index;
        let player_index = usize::from(player_entity_index);
        let parameter_relation = session.economy.entities[player_index]
            .parameter_relation
            .expect("player collision parameter");
        let collision_offset = i16::from_le_bytes(
            session.session.definition.presentation.entity_parameters
                [usize::from(parameter_relation)][12..14]
                .try_into()
                .unwrap(),
        );
        let mut position = session.state.position;
        position.z = session
            .terrain
            .ground_height(position)
            .wrapping_add(collision_offset);
        session.state.position = position;
        session
            .set_level2_entity_position(player_entity_index, position)
            .expect("player map projection");
        session.economy.entities[player_index].set_source_action(2);
        session.economy.entities[player_index].set_source_word_2c(0);

        // Force both NewEvent paths to fail. The source tail is deliberately
        // not transactional: notification/holder/control writes commit and
        // action 2 retries on the next visit.
        std::sync::Arc::make_mut(&mut session.session.baseline)
            .economy
            .free_slots
            .clear();
        session.economy.free_slots.clear();
        session.economy.replacement_cursor_exclusions.clear();
        session.economy.castle_children.clear();
        session.economy.spheres.clear();
        session.economy.death_bursts.clear();
        session.economy.authored_model1.clear();
        session.economy.model59_smoke_children.clear();
        session.economy.authored_model5_water_splashes.clear();
        session.economy.authored_model13.clear();
        session.economy.authored_model6.clear();
        session.economy.model13_spawned_children.clear();
        session.economy.model17_spawned_children.clear();
        session.economy.model76_tail_spawns.clear();
        let sphere_relations = session
            .economy
            .spheres
            .iter()
            .map(|sphere| sphere.player_entity_index)
            .collect::<Vec<_>>();

        session.step_level2_player_dying_action(0, player_entity_index, position);
        assert_eq!(session.economy.entities[player_index].action, 2);
        assert_ne!(
            session.economy.entities[player_index].control_flags & 0x20,
            0
        );
        assert!(session.economy.player_death_markers.is_empty());
        assert!(session.economy.death_bursts.is_empty());
        assert_eq!(
            session.player_death_notification,
            Some(Level2PlayerDeathNotification {
                id: 374,
                state: 1,
                timer: 100,
            })
        );
        assert_eq!(
            session
                .economy
                .spheres
                .iter()
                .map(|sphere| sphere.player_entity_index)
                .collect::<Vec<_>>(),
            sphere_relations
        );

        session.step_level2_player_dying_action(0, player_entity_index, position);
        assert_eq!(session.economy.entities[player_index].action, 2);
        assert!(session.economy.player_death_markers.is_empty());
        assert_eq!(
            session.player_death_notification,
            Some(Level2PlayerDeathNotification {
                id: 374,
                state: 1,
                timer: 100,
            })
        );
    }
}
