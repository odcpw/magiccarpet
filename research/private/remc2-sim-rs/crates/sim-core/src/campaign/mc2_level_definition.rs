//! Private MC2 authored-record translation into the shared level vocabulary.
//!
//! Raw class/model identities terminate in this module. Runtime code consumes
//! semantic kinds and the immutable authored-record binding emitted here.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    level_definition::{
        ActivationGroupId, CreatureKind, ItemKind, LevelDefinition, LevelEntityDefinition,
        LevelEntityId, LevelEntityKind, LevelObjectiveDefinition, MechanicKind, ObjectiveCondition,
        SceneryKind, SpellKind, SwitchKind, TilePosition,
    },
    level1_mission::parse_authored_features,
};

use super::spec::ObjectiveOperation;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Mc2LevelDefinition {
    pub(crate) semantic: LevelDefinition,
    pub(crate) creature_by_authored_record: BTreeMap<u16, CreatureKind>,
    pub(crate) creature_model_by_kind: BTreeMap<CreatureKind, u8>,
    pub(crate) spell_by_authored_record: BTreeMap<u16, SpellKind>,
    pub(crate) spell_family_by_kind: BTreeMap<SpellKind, u8>,
    pub(crate) scenery_by_authored_record: BTreeMap<u16, SceneryKind>,
    pub(crate) item_by_authored_record: BTreeMap<u16, ItemKind>,
    pub(crate) switch_by_authored_record: BTreeMap<u16, SwitchKind>,
    pub(crate) mechanic_by_authored_record: BTreeMap<u16, MechanicKind>,
}

impl Mc2LevelDefinition {
    pub(crate) const fn mechanic_for_reset_selector(
        class: u8,
        model: u8,
        action: u8,
    ) -> Option<MechanicKind> {
        match (class, model, action) {
            (11, 16, 16) => Some(MechanicKind::AbsenceCountdown {
                creature: CreatureKind::Worm,
            }),
            (11, 17, 17) => Some(MechanicKind::AbsenceCountdown {
                creature: CreatureKind::Archer,
            }),
            _ => None,
        }
    }

    pub(crate) const fn scenery_for_reset_selector(
        class: u8,
        model: u8,
        action: u8,
    ) -> Option<SceneryKind> {
        match (class, model, action) {
            (2, 0, 0) => Some(SceneryKind::Tree),
            (2, 1, 3) => Some(SceneryKind::Stone),
            _ => None,
        }
    }

    pub(crate) const fn switch_for_reset_selector(
        class: u8,
        model: u8,
        action: u8,
    ) -> Option<SwitchKind> {
        match (class, model, action) {
            (11, 32, 32) => Some(SwitchKind::ObjectiveGate),
            _ => None,
        }
    }

    pub(crate) fn from_authored_bytes(
        bytes: &[u8],
        objective_operations: &[ObjectiveOperation],
    ) -> Result<Self, String> {
        let features = parse_authored_features(bytes)?;
        let mut creature_ordinals = BTreeMap::<CreatureKind, u16>::new();
        let mut spell_ordinals = BTreeMap::<SpellKind, u16>::new();
        let mut scenery_ordinals = BTreeMap::<SceneryKind, u16>::new();
        let mut item_ordinals = BTreeMap::<ItemKind, u16>::new();
        let mut switch_ordinals = BTreeMap::<SwitchKind, u16>::new();
        let mut mechanic_ordinals = BTreeMap::<MechanicKind, u16>::new();
        let mut creature_by_authored_record = BTreeMap::new();
        let mut creature_model_by_kind = BTreeMap::new();
        let mut spell_by_authored_record = BTreeMap::new();
        let mut spell_family_by_kind = BTreeMap::new();
        let mut scenery_by_authored_record = BTreeMap::new();
        let mut item_by_authored_record = BTreeMap::new();
        let mut switch_by_authored_record = BTreeMap::new();
        let mut mechanic_by_authored_record = BTreeMap::new();
        let mut entities = Vec::new();
        let activation_groups = features
            .iter()
            .filter(|feature| feature.kind != 0)
            .filter_map(|feature| u16::try_from(feature.activation_group).ok())
            .filter(|group| !matches!(*group, 0 | u16::MAX))
            .map(ActivationGroupId::new)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        for feature in &features {
            let creature_kind = match (feature.kind, feature.subtype) {
                // `sub_4A310` constructor identities. These numbers remain
                // private to the MC2 adapter; shared callers see only kinds.
                (5, 0) => Some(CreatureKind::Dragon),
                (5, 1) => Some(CreatureKind::Goat),
                (5, 2) => Some(CreatureKind::Bee),
                (5, 3) => Some(CreatureKind::Worm),
                (5, 4) => Some(CreatureKind::Archer),
                (5, 9) => Some(CreatureKind::Skeleton),
                (5, 13) => Some(CreatureKind::Villager),
                (5, 14) => Some(CreatureKind::Trader),
                (5, 19) => Some(CreatureKind::Firefly),
                _ => None,
            };
            let activation_group = u16::try_from(feature.activation_group)
                .ok()
                .filter(|group| !matches!(*group, 0 | u16::MAX))
                .map(ActivationGroupId::new);
            let position = TilePosition {
                x: feature.tile_x,
                y: feature.tile_y,
            };

            if let Some(kind) = creature_kind {
                let model = u8::try_from(feature.subtype)
                    .map_err(|_| format!("authored {} model does not fit u8", kind.as_str()))?;
                if creature_model_by_kind
                    .insert(kind, model)
                    .is_some_and(|prior| prior != model)
                {
                    return Err(format!(
                        "authored {} maps to more than one MC2 creature model",
                        kind.as_str()
                    ));
                }
                let ordinal = creature_ordinals.entry(kind).or_default();
                let id = LevelEntityId::creature(kind, *ordinal);
                *ordinal = ordinal
                    .checked_add(1)
                    .ok_or_else(|| format!("too many authored {} creatures", kind.as_str()))?;
                entities.push(LevelEntityDefinition::new(
                    id,
                    LevelEntityKind::Creature(kind),
                    position,
                    activation_group,
                ));
                creature_by_authored_record.insert(feature.index, kind);
                continue;
            }

            let scenery_kind = match (feature.kind, feature.subtype) {
                (2, 0) => Some(SceneryKind::Tree),
                (2, 1) => Some(SceneryKind::Stone),
                _ => None,
            };
            if let Some(kind) = scenery_kind {
                let ordinal = scenery_ordinals.entry(kind).or_default();
                let id = LevelEntityId::scenery(kind, *ordinal);
                *ordinal = ordinal
                    .checked_add(1)
                    .ok_or_else(|| format!("too many authored {} rows", kind.as_str()))?;
                entities.push(LevelEntityDefinition::new(
                    id,
                    LevelEntityKind::Scenery(kind),
                    position,
                    activation_group,
                ));
                scenery_by_authored_record.insert(feature.index, kind);
                continue;
            }

            // `sub_51610` constructs model-5 rows and `UpdateScroll_59C80`
            // awards spell experience on collection. Model 3 shares the raw
            // class-14 carrier but is an exit target, not an inventory item.
            let item_kind = match (feature.kind, feature.subtype) {
                (14, 5) => Some(ItemKind::ExperienceScroll),
                _ => None,
            };
            if let Some(kind) = item_kind {
                let ordinal = item_ordinals.entry(kind).or_default();
                let id = LevelEntityId::item(kind, *ordinal);
                *ordinal = ordinal
                    .checked_add(1)
                    .ok_or_else(|| format!("too many authored {} items", kind.as_str()))?;
                entities.push(LevelEntityDefinition::new(
                    id,
                    LevelEntityKind::Item(kind),
                    position,
                    activation_group,
                ));
                item_by_authored_record.insert(feature.index, kind);
                continue;
            }

            let switch_kind = match (feature.kind, feature.subtype) {
                // `AddSwitch0B_20_6F1C0` waits for one objective-state
                // selector, then activates the row's authored group.
                (11, 32) => Some(SwitchKind::ObjectiveGate),
                _ => None,
            };
            if let Some(kind) = switch_kind {
                let ordinal = switch_ordinals.entry(kind).or_default();
                let id = LevelEntityId::switch(kind, *ordinal);
                *ordinal = ordinal
                    .checked_add(1)
                    .ok_or_else(|| format!("too many authored {} switches", kind.as_str()))?;
                entities.push(LevelEntityDefinition::new(
                    id,
                    LevelEntityKind::Switch(kind),
                    position,
                    activation_group,
                ));
                switch_by_authored_record.insert(feature.index, kind);
                continue;
            }

            let spell_kind = match (feature.kind, feature.subtype) {
                // `AddSpellXX_XX_51120` receives these spell identities. Keep
                // the numeric selectors inside the private MC2 adapter.
                (15, 2) => Some(SpellKind::Castle),
                (15, 3) => Some(SpellKind::Speed),
                (15, 4) => Some(SpellKind::Morph),
                (15, 6) => Some(SpellKind::Shield),
                (15, 11) => Some(SpellKind::Invisibility),
                (15, 12) => Some(SpellKind::BeyondSight),
                _ => None,
            };
            if let Some(kind) = spell_kind {
                let family = u8::try_from(feature.subtype)
                    .map_err(|_| format!("authored {} family does not fit u8", kind.as_str()))?;
                if spell_family_by_kind
                    .insert(kind, family)
                    .is_some_and(|prior| prior != family)
                {
                    return Err(format!(
                        "authored {} maps to more than one MC2 spell family",
                        kind.as_str()
                    ));
                }
                let ordinal = spell_ordinals.entry(kind).or_default();
                let id = LevelEntityId::spell_pickup(kind, *ordinal);
                *ordinal = ordinal
                    .checked_add(1)
                    .ok_or_else(|| format!("too many authored {} spell pickups", kind.as_str()))?;
                entities.push(LevelEntityDefinition::new(
                    id,
                    LevelEntityKind::SpellPickup(kind),
                    position,
                    activation_group,
                ));
                spell_by_authored_record.insert(feature.index, kind);
                continue;
            }

            let mechanic_kind = match (feature.kind, feature.subtype) {
                // The two source callbacks observe the rebuilt class-5 model
                // lists for Worms and Archers respectively. Shared callers
                // see the creature being observed, never those selectors.
                (11, 16) => MechanicKind::AbsenceCountdown {
                    creature: CreatureKind::Worm,
                },
                (11, 17) => MechanicKind::AbsenceCountdown {
                    creature: CreatureKind::Archer,
                },
                _ => continue,
            };
            let ordinal = mechanic_ordinals.entry(mechanic_kind).or_default();
            let id = LevelEntityId::mechanic(mechanic_kind, *ordinal);
            *ordinal = ordinal
                .checked_add(1)
                .ok_or_else(|| format!("too many authored {} mechanics", mechanic_kind.as_str()))?;
            entities.push(LevelEntityDefinition::new(
                id,
                LevelEntityKind::Mechanic(mechanic_kind),
                position,
                activation_group,
            ));
            mechanic_by_authored_record.insert(feature.index, mechanic_kind);
        }

        let objectives = objective_operations
            .iter()
            .map(|operation| {
                let condition = match operation.stage_index {
                    0 => ObjectiveCondition::CastleManaPercent(
                        u16::try_from(operation.objective_selector).map_err(|_| {
                            format!(
                                "castle-mana objective {} has invalid percentage {}",
                                operation.objective_slot, operation.objective_selector
                            )
                        })?,
                    ),
                    3 => {
                        if operation.objective_selector <= 0 {
                            return Err(format!(
                                "rival-wizard objective {} has invalid target {}",
                                operation.objective_slot, operation.objective_selector
                            ));
                        }
                        ObjectiveCondition::DefeatRivalWizard
                    }
                    5 => ObjectiveCondition::ReachLocation(TilePosition {
                        x: u16::try_from(operation.axis_x).map_err(|_| {
                            format!(
                                "location objective {} has invalid x {}",
                                operation.objective_slot, operation.axis_x
                            )
                        })?,
                        y: u16::try_from(operation.axis_y).map_err(|_| {
                            format!(
                                "location objective {} has invalid y {}",
                                operation.objective_slot, operation.axis_y
                            )
                        })?,
                    }),
                    7 => {
                        let authored_record =
                            u16::try_from(operation.objective_selector).map_err(|_| {
                                format!(
                                    "creature objective {} has invalid authored selector {}",
                                    operation.objective_slot, operation.objective_selector
                                )
                            })?;
                        let kind = creature_by_authored_record
                            .get(&authored_record)
                            .copied()
                            .ok_or_else(|| {
                                format!(
                                    "creature objective {} targets non-creature authored record {}",
                                    operation.objective_slot, authored_record
                                )
                            })?;
                        ObjectiveCondition::NoLivingCreatures(kind)
                    }
                    9 => {
                        let authored_record =
                            u16::try_from(operation.objective_selector).map_err(|_| {
                                format!(
                                    "building objective {} has invalid authored selector {}",
                                    operation.objective_slot, operation.objective_selector
                                )
                            })?;
                        let building = features
                            .iter()
                            .find(|feature| feature.index == authored_record)
                            .ok_or_else(|| {
                                format!(
                                    "building objective {} targets missing authored record {}",
                                    operation.objective_slot, authored_record
                                )
                            })?;
                        if (building.kind, building.subtype) != (10, 45) {
                            return Err(format!(
                                "building objective {} targets unsupported authored kind {}/{}",
                                operation.objective_slot, building.kind, building.subtype
                            ));
                        }
                        ObjectiveCondition::NoStandingBuildings
                    }
                    kind => {
                        return Err(format!(
                            "objective {} has unsupported MC2 condition kind {}",
                            operation.objective_slot, kind
                        ));
                    }
                };
                Ok(LevelObjectiveDefinition::new(condition))
            })
            .collect::<Result<Vec<_>, String>>()?;

        Ok(Self {
            semantic: LevelDefinition::new(entities, objectives, activation_groups)?,
            creature_by_authored_record,
            creature_model_by_kind,
            spell_by_authored_record,
            spell_family_by_kind,
            scenery_by_authored_record,
            item_by_authored_record,
            switch_by_authored_record,
            mechanic_by_authored_record,
        })
    }
}
