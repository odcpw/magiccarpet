//! Private MC2 authored-record translation into the shared level vocabulary.
//!
//! Raw class/model identities terminate in this module. Runtime code consumes
//! semantic kinds and the immutable authored-record binding emitted here.

use std::collections::{BTreeMap, BTreeSet};

use sim_definition::adapter::{
    absence_countdown_definition, absence_countdown_entity_definition, activation_group_id,
    build_level_definition, creature_entity_definition, entity_definition_with_position_access,
    objective_definition, stable_entity_id, stable_objective_id, stable_rival_wizard_id,
};

use crate::{
    house::HouseParameter,
    level_definition::{
        BuildingKind, CreatureKind, ItemKind, LevelDefinition, LevelEntityId, LevelEntityKind,
        LevelEntityParameterAccess, LevelEntityPositionAccess, LevelObjectiveConditionAccess,
        LevelObjectiveId, MechanicKind, ObjectiveCondition, RivalWizardId, SceneryKind, SpellKind,
        SwitchKind, TilePosition, supports_complete_creature_lifecycle,
    },
    level1_mission::parse_authored_features,
};

use super::spec::{CampaignSelection, ObjectiveOperation};

const MC2_LEVEL_ENTITY_IDS: &str = include_str!("mc2_level_entity_ids.tsv");
const MC2_LEVEL_OBJECTIVE_IDS: &str = include_str!("mc2_level_objective_ids.tsv");
const NYPHUR_RIVAL_WIZARD_ID: &str = "rival_wizard.nyphur";
const LEVEL_TWO_NYPHUR_OBJECTIVE_SLOT: u8 = 1;
// MC2 stage type 3 stores a one-based player selector, not an entity slot.
// The qualified player census binds selector 2 to Nyphur's entity row 112.
const LEVEL_TWO_NYPHUR_PLAYER_SELECTOR: u16 = 2;
const LEVEL_TWO_NYPHUR_ENTITY_INDEX: u16 = 112;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Mc2MovableRuntimeHint {
    AuthoredRecord(u16),
    AuthoredFeature(u16),
    ResetKindAndPosition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Mc2LevelDefinition {
    pub(crate) semantic: LevelDefinition,
    pub(crate) creature_by_authored_record: BTreeMap<u16, CreatureKind>,
    pub(crate) creature_model_by_kind: BTreeMap<CreatureKind, u8>,
    pub(crate) spell_by_authored_record: BTreeMap<u16, SpellKind>,
    pub(crate) spell_family_by_kind: BTreeMap<SpellKind, u8>,
    pub(crate) building_by_authored_record: BTreeMap<u16, BuildingKind>,
    pub(crate) scenery_by_authored_record: BTreeMap<u16, SceneryKind>,
    pub(crate) item_by_authored_record: BTreeMap<u16, ItemKind>,
    pub(crate) switch_by_authored_record: BTreeMap<u16, SwitchKind>,
    pub(crate) mechanic_by_authored_record: BTreeMap<u16, MechanicKind>,
    pub(crate) mechanic_entity_by_authored_record: BTreeMap<u16, LevelEntityId>,
    pub(crate) movable_runtime_hint_by_entity_id: BTreeMap<LevelEntityId, Mc2MovableRuntimeHint>,
    pub(crate) standing_building_chains: BTreeMap<LevelObjectiveId, Box<[u8]>>,
    pub(crate) rival_wizard_by_id: BTreeMap<RivalWizardId, u16>,
}

impl Mc2LevelDefinition {
    pub(crate) const fn creature_for_model(model: u8) -> Option<CreatureKind> {
        match model {
            0 => Some(CreatureKind::Dragon),
            1 => Some(CreatureKind::Goat),
            2 => Some(CreatureKind::Bee),
            3 => Some(CreatureKind::Worm),
            4 => Some(CreatureKind::Archer),
            9 => Some(CreatureKind::Skeleton),
            12 => Some(CreatureKind::Builder),
            13 => Some(CreatureKind::Villager),
            14 => Some(CreatureKind::Trader),
            19 => Some(CreatureKind::Firefly),
            _ => None,
        }
    }

    pub(crate) const fn creature_for_reset_selector(
        class: u8,
        model: u8,
        action: u8,
    ) -> Option<CreatureKind> {
        if class != 5 || matches!(action, 180 | 232 | 234) {
            return None;
        }
        Self::creature_for_model(model)
    }

    pub(crate) const fn creature_for_house_resident(model: u8, action: u8) -> Option<CreatureKind> {
        match (model, action) {
            (4, 33) => Some(CreatureKind::Archer),
            (12, 97) => Some(CreatureKind::Builder),
            (13, 105) => Some(CreatureKind::Villager),
            (14, 113) => Some(CreatureKind::Trader),
            _ => None,
        }
    }

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
            (14, 3, 8) => Some(SceneryKind::ExitPortal),
            _ => None,
        }
    }

    pub(crate) const fn building_for_reset_selector(
        class: u8,
        model: u8,
        action: u8,
    ) -> Option<BuildingKind> {
        match (class, model, action) {
            (10, 45, 52) => Some(BuildingKind::House),
            _ => None,
        }
    }

    pub(crate) const fn switch_for_reset_selector(
        class: u8,
        model: u8,
        action: u8,
    ) -> Option<SwitchKind> {
        match (class, model, action) {
            (11, 0, 0) => Some(SwitchKind::AreaEntryTrigger),
            (11, 1, 1) => Some(SwitchKind::AreaExitTrigger),
            (11, 4, 4) => Some(SwitchKind::LevelExitGate),
            (11, 12, 12) => Some(SwitchKind::ExitSequenceTrigger),
            (11, 32, 32) => Some(SwitchKind::ObjectiveGate),
            _ => None,
        }
    }

    pub(crate) const fn spell_for_reset_selector(
        class: u8,
        model: u8,
        action: u8,
    ) -> Option<SpellKind> {
        match (class, model, action) {
            (15, 3, 10) => Some(SpellKind::Speed),
            (15, 6, 19) => Some(SpellKind::Shield),
            (15, 11, 34) => Some(SpellKind::Invisibility),
            (15, 12, 37) => Some(SpellKind::BeyondSight),
            _ => None,
        }
    }

    pub(crate) fn from_authored_bytes(
        bytes: &[u8],
        objective_operations: &[ObjectiveOperation],
        selection: CampaignSelection,
        building_parameters: Option<&BTreeMap<u8, HouseParameter>>,
    ) -> Result<Self, String> {
        let features = parse_authored_features(bytes)?;
        let mut stable_entity_ids = stable_entity_ids(selection.external_level)?;
        let mut stable_objective_ids = stable_objective_ids(selection.external_level)?;
        let mut creature_by_authored_record = BTreeMap::new();
        let mut creature_model_by_kind = BTreeMap::new();
        let mut spell_by_authored_record = BTreeMap::new();
        let mut spell_family_by_kind = BTreeMap::new();
        let mut building_by_authored_record = BTreeMap::new();
        let mut scenery_by_authored_record = BTreeMap::new();
        let mut item_by_authored_record = BTreeMap::new();
        let mut switch_by_authored_record = BTreeMap::new();
        let mut mechanic_by_authored_record = BTreeMap::new();
        let mut mechanic_entity_by_authored_record = BTreeMap::new();
        let mut authored_record_by_entity_id = BTreeMap::new();
        let mut movable_runtime_hint_by_entity_id = BTreeMap::new();
        let mut standing_building_chains = BTreeMap::new();
        let mut rival_wizard_by_id = BTreeMap::new();
        let mut entities = Vec::new();
        let activation_groups = features
            .iter()
            .filter(|feature| feature.kind != 0)
            .filter_map(|feature| u16::try_from(feature.activation_group).ok())
            .filter(|group| !matches!(*group, 0 | u16::MAX))
            .map(activation_group_id)
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
                (5, 12) => Some(CreatureKind::Builder),
                (5, 13) => Some(CreatureKind::Villager),
                (5, 14) => Some(CreatureKind::Trader),
                (5, 19) => Some(CreatureKind::Firefly),
                _ => None,
            };
            let activation_group = u16::try_from(feature.activation_group)
                .ok()
                .filter(|group| !matches!(*group, 0 | u16::MAX))
                .map(activation_group_id);
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
                let (id, position_access) = take_stable_entity_id(
                    &mut stable_entity_ids,
                    feature.index,
                    LevelEntityKind::Creature(kind),
                )?;
                bind_private_entity(
                    &mut authored_record_by_entity_id,
                    &mut movable_runtime_hint_by_entity_id,
                    feature,
                    &id,
                    LevelEntityKind::Creature(kind),
                    position_access,
                )?;
                if matches!(
                    kind,
                    CreatureKind::Firefly
                        | CreatureKind::Archer
                        | CreatureKind::Bee
                        | CreatureKind::Dragon
                        | CreatureKind::Skeleton
                ) {
                    let max_life = match kind {
                        CreatureKind::Firefly => {
                            crate::native_world::level2_class5::firefly::FIREFLY_LIFE
                        }
                        CreatureKind::Archer => crate::native_world::level2_class5::ARCHER_MAX_LIFE,
                        CreatureKind::Bee => {
                            i32::from(crate::native_world::level2_class5::BEE_MAX_LIFE)
                        }
                        CreatureKind::Dragon => crate::native_world::level2_class5::DRAGON_LIFE,
                        CreatureKind::Skeleton => 1_000,
                        _ => {
                            return Err(format!(
                                "semantic creature {} has no maximum-life binding",
                                kind.as_str()
                            ));
                        }
                    };
                    entities.push(creature_entity_definition(
                        id,
                        kind,
                        position,
                        activation_group,
                        position_access,
                        max_life.cast_unsigned(),
                        LevelEntityParameterAccess::SessionReset,
                    ));
                } else {
                    entities.push(entity_definition_with_position_access(
                        id,
                        LevelEntityKind::Creature(kind),
                        position,
                        activation_group,
                        position_access,
                    ));
                }
                creature_by_authored_record.insert(feature.index, kind);
                continue;
            }

            let scenery_kind = match (feature.kind, feature.subtype) {
                (2, 0) => Some(SceneryKind::Tree),
                (2, 1) => Some(SceneryKind::Stone),
                // `sub_51570` constructs the package's class-14 exit target.
                // Shared callers see the scenery role, never its MC2 selector.
                (14, 3) => Some(SceneryKind::ExitPortal),
                _ => None,
            };
            if let Some(kind) = scenery_kind {
                let (id, position_access) = take_stable_entity_id(
                    &mut stable_entity_ids,
                    feature.index,
                    LevelEntityKind::Scenery(kind),
                )?;
                bind_private_entity(
                    &mut authored_record_by_entity_id,
                    &mut movable_runtime_hint_by_entity_id,
                    feature,
                    &id,
                    LevelEntityKind::Scenery(kind),
                    position_access,
                )?;
                entities.push(entity_definition_with_position_access(
                    id,
                    LevelEntityKind::Scenery(kind),
                    position,
                    activation_group,
                    position_access,
                ));
                scenery_by_authored_record.insert(feature.index, kind);
                continue;
            }

            let building_kind = match (feature.kind, feature.subtype) {
                // `AddTerrainModification_50250` constructs the package's
                // House rows before their ordinary live-house callback.
                (10, 45) => Some(BuildingKind::House),
                _ => None,
            };
            if let Some(kind) = building_kind {
                let (id, position_access) = take_stable_entity_id(
                    &mut stable_entity_ids,
                    feature.index,
                    LevelEntityKind::Building(kind),
                )?;
                bind_private_entity(
                    &mut authored_record_by_entity_id,
                    &mut movable_runtime_hint_by_entity_id,
                    feature,
                    &id,
                    LevelEntityKind::Building(kind),
                    position_access,
                )?;
                entities.push(entity_definition_with_position_access(
                    id,
                    LevelEntityKind::Building(kind),
                    position,
                    activation_group,
                    position_access,
                ));
                building_by_authored_record.insert(feature.index, kind);
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
                let (id, position_access) = take_stable_entity_id(
                    &mut stable_entity_ids,
                    feature.index,
                    LevelEntityKind::Item(kind),
                )?;
                bind_private_entity(
                    &mut authored_record_by_entity_id,
                    &mut movable_runtime_hint_by_entity_id,
                    feature,
                    &id,
                    LevelEntityKind::Item(kind),
                    position_access,
                )?;
                entities.push(entity_definition_with_position_access(
                    id,
                    LevelEntityKind::Item(kind),
                    position,
                    activation_group,
                    position_access,
                ));
                item_by_authored_record.insert(feature.index, kind);
                continue;
            }

            let switch_kind = match (feature.kind, feature.subtype) {
                // These one-shot area triggers activate their authored group
                // on entry or exit respectively. `AddSwitch0B_04_6F150`
                // waits for the package's level-complete player state.
                (11, 0) => Some(SwitchKind::AreaEntryTrigger),
                (11, 1) => Some(SwitchKind::AreaExitTrigger),
                (11, 4) => Some(SwitchKind::LevelExitGate),
                // This overlap trigger starts the player-owned exit sequence;
                // it does not activate its stage tag as another area group.
                (11, 12) => Some(SwitchKind::ExitSequenceTrigger),
                // `AddSwitch0B_20_6F1C0` waits for one objective-state
                // selector, then activates the row's authored group.
                (11, 32) => Some(SwitchKind::ObjectiveGate),
                _ => None,
            };
            if let Some(kind) = switch_kind {
                let (id, position_access) = take_stable_entity_id(
                    &mut stable_entity_ids,
                    feature.index,
                    LevelEntityKind::Switch(kind),
                )?;
                bind_private_entity(
                    &mut authored_record_by_entity_id,
                    &mut movable_runtime_hint_by_entity_id,
                    feature,
                    &id,
                    LevelEntityKind::Switch(kind),
                    position_access,
                )?;
                entities.push(entity_definition_with_position_access(
                    id,
                    LevelEntityKind::Switch(kind),
                    position,
                    activation_group,
                    position_access,
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
                let (id, position_access) = take_stable_entity_id(
                    &mut stable_entity_ids,
                    feature.index,
                    LevelEntityKind::SpellPickup(kind),
                )?;
                bind_private_entity(
                    &mut authored_record_by_entity_id,
                    &mut movable_runtime_hint_by_entity_id,
                    feature,
                    &id,
                    LevelEntityKind::SpellPickup(kind),
                    position_access,
                )?;
                entities.push(entity_definition_with_position_access(
                    id,
                    LevelEntityKind::SpellPickup(kind),
                    position,
                    activation_group,
                    position_access,
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
            let (id, position_access) = take_stable_entity_id(
                &mut stable_entity_ids,
                feature.index,
                LevelEntityKind::Mechanic(mechanic_kind),
            )?;
            bind_private_entity(
                &mut authored_record_by_entity_id,
                &mut movable_runtime_hint_by_entity_id,
                feature,
                &id,
                LevelEntityKind::Mechanic(mechanic_kind),
                position_access,
            )?;
            entities.push(absence_countdown_entity_definition(
                id.clone(),
                LevelEntityKind::Mechanic(mechanic_kind),
                position,
                activation_group,
                position_access,
                absence_countdown_definition(16, LevelEntityParameterAccess::SessionReset),
            ));
            mechanic_by_authored_record.insert(feature.index, mechanic_kind);
            mechanic_entity_by_authored_record.insert(feature.index, id);
        }

        let objectives = objective_operations
            .iter()
            .map(|operation| {
                let (id, condition_access) = stable_objective_ids
                    .remove(&u16::from(operation.objective_slot))
                    .ok_or_else(|| {
                        format!(
                            "objective {} has no stable MC2 objective identity",
                            operation.objective_slot
                        )
                    })?;
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
                        let player_selector = u16::try_from(operation.objective_selector).map_err(|_| {
                            format!(
                                "rival-wizard objective {} has invalid target {}",
                                operation.objective_slot, operation.objective_selector
                            )
                        })?;
                        if selection.external_level != 2
                            || operation.objective_slot != LEVEL_TWO_NYPHUR_OBJECTIVE_SLOT
                            || player_selector != LEVEL_TWO_NYPHUR_PLAYER_SELECTOR
                        {
                            return Err(format!(
                                "rival-wizard objective {} does not match the qualified Nyphur target binding",
                                operation.objective_slot
                            ));
                        }
                        let target = stable_rival_wizard_id(NYPHUR_RIVAL_WIZARD_ID)
                            .map_err(|error| error.to_string())?;
                        if rival_wizard_by_id
                            .insert(target.clone(), LEVEL_TWO_NYPHUR_ENTITY_INDEX)
                            .is_some()
                        {
                            return Err("duplicate qualified rival-wizard target binding".to_owned());
                        }
                        ObjectiveCondition::DefeatRivalWizard(target)
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
                        let parameters = building_parameters.ok_or_else(|| {
                            format!(
                                "building objective {} has no validated BLDGPRM carrier",
                                operation.objective_slot
                            )
                        })?;
                        let selector = u8::try_from(building.parameter_1).map_err(|_| {
                            format!(
                                "building objective {} root selector exceeds u8",
                                operation.objective_slot
                            )
                        })?;
                        let chain = super::source_facts::building_parameter_chain(
                            parameters,
                            selector,
                        )
                        .map_err(|error| {
                            format!("building objective {} {error}", operation.objective_slot)
                        })?;
                        standing_building_chains.insert(id.clone(), chain);
                        ObjectiveCondition::NoStandingBuildings
                    }
                    kind => {
                        return Err(format!(
                            "objective {} has unsupported MC2 condition kind {}",
                            operation.objective_slot, kind
                        ));
                    }
                };
                if matches!(condition, ObjectiveCondition::CastleManaPercent(percent) if percent > 100)
                {
                    return Err(format!(
                        "semantic objective {} Castle-mana percentage is outside 0..=100",
                        id.as_str()
                    ));
                }
                if condition_access == LevelObjectiveConditionAccess::SessionReset {
                    match condition {
                        ObjectiveCondition::ReachLocation(_) => {}
                        ObjectiveCondition::CastleManaPercent(percent) if percent <= 100 => {}
                        ObjectiveCondition::NoLivingCreatures(_) => {}
                        _ => {
                            return Err(format!(
                                "writable semantic objective {} has no supported authoring consumer",
                                id.as_str()
                            ));
                        }
                    }
                }
                Ok(objective_definition(id, condition, condition_access))
            })
            .collect::<Result<Vec<_>, String>>()?;

        if !stable_entity_ids.is_empty() {
            return Err(format!(
                "stable MC2 entity identity manifest has {} unconsumed entries for external level {}",
                stable_entity_ids.len(),
                selection.external_level
            ));
        }
        if !stable_objective_ids.is_empty() {
            return Err(format!(
                "stable MC2 objective identity manifest has {} unconsumed entries for external level {}",
                stable_objective_ids.len(),
                selection.external_level
            ));
        }

        Ok(Self {
            semantic: build_level_definition(entities, objectives, activation_groups)?,
            creature_by_authored_record,
            creature_model_by_kind,
            spell_by_authored_record,
            spell_family_by_kind,
            building_by_authored_record,
            scenery_by_authored_record,
            item_by_authored_record,
            switch_by_authored_record,
            mechanic_by_authored_record,
            mechanic_entity_by_authored_record,
            movable_runtime_hint_by_entity_id,
            standing_building_chains,
            rival_wizard_by_id,
        })
    }
}

fn stable_objective_ids(
    external_level: u16,
) -> Result<BTreeMap<u16, (LevelObjectiveId, LevelObjectiveConditionAccess)>, String> {
    let mut ids = BTreeMap::new();
    for (line_index, line) in MC2_LEVEL_OBJECTIVE_IDS.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 4 {
            return Err(format!(
                "invalid MC2 objective identity manifest line {}",
                line_index + 1
            ));
        }
        let level = fields[0].parse::<u16>().map_err(|_| {
            format!(
                "invalid MC2 objective identity level at line {}",
                line_index + 1
            )
        })?;
        if level != external_level {
            continue;
        }
        let objective_slot = fields[1].parse::<u16>().map_err(|_| {
            format!(
                "invalid MC2 objective identity slot at line {}",
                line_index + 1
            )
        })?;
        let id = stable_objective_id(fields[2]).map_err(|error| error.to_string())?;
        if !id.as_str().starts_with("objective.") {
            return Err(format!(
                "stable MC2 objective identity {} is outside the objective namespace",
                id.as_str()
            ));
        }
        let access = match fields[3] {
            "read_only" => LevelObjectiveConditionAccess::ReadOnly,
            "session_reset" => LevelObjectiveConditionAccess::SessionReset,
            _ => {
                return Err(format!(
                    "invalid objective condition access at manifest line {}",
                    line_index + 1
                ));
            }
        };
        if ids.insert(objective_slot, (id, access)).is_some() {
            return Err(format!(
                "duplicate MC2 objective identity for external level {external_level} slot {objective_slot}"
            ));
        }
    }
    Ok(ids)
}

fn stable_entity_ids(
    external_level: u16,
) -> Result<BTreeMap<u16, (LevelEntityId, LevelEntityPositionAccess)>, String> {
    let mut ids = BTreeMap::new();
    for (line_index, line) in MC2_LEVEL_ENTITY_IDS.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 4 {
            return Err(format!(
                "invalid MC2 entity identity manifest line {}",
                line_index + 1
            ));
        }
        let level = fields[0]
            .parse::<u16>()
            .map_err(|_| format!("invalid MC2 identity level at line {}", line_index + 1))?;
        if level != external_level {
            continue;
        }
        let authored_record = fields[1]
            .parse::<u16>()
            .map_err(|_| format!("invalid MC2 identity row at line {}", line_index + 1))?;
        let id = stable_entity_id(fields[2]).map_err(|error| error.to_string())?;
        let access = match fields[3] {
            "read_only" => LevelEntityPositionAccess::ReadOnly,
            "session_reset" => LevelEntityPositionAccess::SessionReset,
            _ => {
                return Err(format!(
                    "invalid position access at manifest line {}",
                    line_index + 1
                ));
            }
        };
        if ids.insert(authored_record, (id, access)).is_some() {
            return Err(format!(
                "duplicate MC2 entity identity for external level {external_level} row {authored_record}"
            ));
        }
    }
    Ok(ids)
}

fn take_stable_entity_id(
    ids: &mut BTreeMap<u16, (LevelEntityId, LevelEntityPositionAccess)>,
    authored_record: u16,
    kind: LevelEntityKind,
) -> Result<(LevelEntityId, LevelEntityPositionAccess), String> {
    let (id, access) = ids.remove(&authored_record).ok_or_else(|| {
        format!(
            "semantic {:?} row {authored_record} has no stable MC2 entity identity",
            kind
        )
    })?;
    let namespace = match kind {
        LevelEntityKind::Creature(_) => "creature.",
        LevelEntityKind::SpellPickup(_) => "spell_pickup.",
        LevelEntityKind::Scenery(_) => "scenery.",
        LevelEntityKind::Item(_) => "item.",
        LevelEntityKind::Building(_) => "building.",
        LevelEntityKind::Switch(_) => "switch.",
        LevelEntityKind::Mechanic(_) => "mechanic.",
    };
    if !id.as_str().starts_with(namespace) {
        return Err(format!(
            "stable MC2 entity identity {} does not match semantic {:?}",
            id.as_str(),
            kind
        ));
    }
    Ok((id, access))
}

fn bind_private_entity(
    authored: &mut BTreeMap<LevelEntityId, u16>,
    movable_hints: &mut BTreeMap<LevelEntityId, Mc2MovableRuntimeHint>,
    feature: &crate::level1_mission::AuthoredFeature,
    id: &LevelEntityId,
    kind: LevelEntityKind,
    access: LevelEntityPositionAccess,
) -> Result<(), String> {
    if authored.insert(id.clone(), feature.index).is_some() {
        return Err(format!(
            "duplicate stable MC2 entity identity {}",
            id.as_str()
        ));
    }
    if access == LevelEntityPositionAccess::SessionReset {
        let hint = match kind {
            LevelEntityKind::Creature(kind)
                if feature.activation_group > 0 && supports_complete_creature_lifecycle(kind) =>
            {
                Mc2MovableRuntimeHint::AuthoredFeature(feature.index)
            }
            LevelEntityKind::Creature(kind)
                if feature.activation_group == 0 && supports_complete_creature_lifecycle(kind) =>
            {
                Mc2MovableRuntimeHint::ResetKindAndPosition
            }
            LevelEntityKind::Switch(_) if feature.activation_group != 0 => {
                Mc2MovableRuntimeHint::AuthoredFeature(feature.index)
            }
            LevelEntityKind::Switch(_) => {
                let source_id = u16::try_from(feature.stage_tag).map_err(|_| {
                    format!(
                        "movable semantic entity {} has invalid MC2 source identity {}",
                        id.as_str(),
                        feature.stage_tag
                    )
                })?;
                Mc2MovableRuntimeHint::AuthoredRecord(source_id)
            }
            LevelEntityKind::Item(ItemKind::ExperienceScroll)
            | LevelEntityKind::Scenery(SceneryKind::ExitPortal) => {
                Mc2MovableRuntimeHint::AuthoredFeature(feature.index)
            }
            LevelEntityKind::SpellPickup(_)
            | LevelEntityKind::Scenery(SceneryKind::Stone | SceneryKind::Tree)
                if feature.activation_group != 0 =>
            {
                Mc2MovableRuntimeHint::AuthoredFeature(feature.index)
            }
            LevelEntityKind::SpellPickup(_)
            | LevelEntityKind::Scenery(SceneryKind::Stone | SceneryKind::Tree) => {
                Mc2MovableRuntimeHint::ResetKindAndPosition
            }
            _ => {
                return Err(format!(
                    "semantic entity {} has no supported movable MC2 carrier class",
                    id.as_str()
                ));
            }
        };
        movable_hints.insert(id.clone(), hint);
    }
    Ok(())
}
