//! Private MC2 authored-record translation into the shared level vocabulary.
//!
//! Raw class/model identities terminate in this module. Runtime code consumes
//! semantic kinds and the immutable authored-record binding emitted here.

use std::collections::{BTreeMap, BTreeSet};

use sim_definition::adapter::{
    absence_countdown_definition, absence_countdown_entity_definition, activation_group_id,
    activation_switch_entity_definition, area_entry_trigger_entity_definition,
    area_exit_trigger_entity_definition, ballistic_scenery_entity_definition, build_level_definition, cave_corridor_definition,
    cave_corridor_entity_definition, cave_feature_entity_definition, creature_entity_definition,
    dolmen_entity_definition, effect_entity_definition, entity_definition_with_position_access,
    exit_sequence_trigger_entity_definition, experience_scroll_entity_definition,
    field_entity_definition, house_construction_entity_definition,
    mc2_authored_terrain_disaster_definition, mc2_effect_definition, mc2_field_definition,
    mushroom_entity_definition, objective_definition, passive_item_entity_definition, stable_entity_id, stable_objective_id,
    terrain_disaster_entity_definition,
    static_scenery_entity_definition,
};

use crate::{
    house::HouseParameter,
    level_definition::{
        BallisticSceneryDefinition, BuildingKind, CaveFeatureDefinition, CaveFeatureKind, CorridorOrientation, CreatureKind, DolmenDefinition,
        EffectKind, FieldKind, ItemKind, LevelDefinition, LevelEntityId, LevelEntityKind,
        LevelEntityParameterAccess, LevelEntityPositionAccess, LevelObjectiveConditionAccess,
        LevelObjectiveId, MechanicKind, MushroomDefinition, ObjectiveCondition, RivalWizardId,
        SceneryKind, SpellKind, SwitchKind, TerrainDisasterKind, TilePosition,
        supports_authorable_creature_placement,
        PassiveItemDefinition, StaticSceneryDefinition,
    },
    level1_mission::{AuthoredFeature, parse_authored_features},
};

use super::spec::{CampaignSelection, ObjectiveOperation};

const MC2_LEVEL_ENTITY_IDS: &str = include_str!("mc2_level_entity_ids.tsv");
const MC2_LEVEL_OBJECTIVE_IDS: &str = include_str!("mc2_level_objective_ids.tsv");

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
    pub(crate) creature_entity_by_authored_record: BTreeMap<u16, LevelEntityId>,
    pub(crate) creature_model_by_kind: BTreeMap<CreatureKind, u8>,
    pub(crate) spell_by_authored_record: BTreeMap<u16, SpellKind>,
    pub(crate) spell_family_by_kind: BTreeMap<SpellKind, u8>,
    pub(crate) building_by_authored_record: BTreeMap<u16, BuildingKind>,
    pub(crate) scenery_by_authored_record: BTreeMap<u16, SceneryKind>,
    pub(crate) scenery_entity_by_authored_record: BTreeMap<u16, LevelEntityId>,
    pub(crate) item_by_authored_record: BTreeMap<u16, ItemKind>,
    pub(crate) item_entity_by_authored_record: BTreeMap<u16, LevelEntityId>,
    pub(crate) switch_by_authored_record: BTreeMap<u16, SwitchKind>,
    pub(crate) switch_entity_by_authored_record: BTreeMap<u16, LevelEntityId>,
    pub(crate) mechanic_by_authored_record: BTreeMap<u16, MechanicKind>,
    pub(crate) mechanic_entity_by_authored_record: BTreeMap<u16, LevelEntityId>,
    pub(crate) movable_runtime_hint_by_entity_id: BTreeMap<LevelEntityId, Mc2MovableRuntimeHint>,
    pub(crate) standing_building_chains: BTreeMap<LevelObjectiveId, Box<[u8]>>,
    pub(crate) rival_wizard_selectors: BTreeMap<RivalWizardId, u16>,
}

impl Mc2LevelDefinition {
    pub(crate) fn initial_mechanic_entity(
        &self,
        kind: MechanicKind,
        position: TilePosition,
    ) -> Result<&LevelEntityId, String> {
        let mut matching = self.semantic.entities().iter().filter(|entity| {
            entity.kind() == LevelEntityKind::Mechanic(kind)
                && entity.default_position() == position
        });
        let Some(entity) = matching.next() else {
            return Err(format!(
                "no semantic mechanic {kind:?} at ({}, {})",
                position.x, position.y
            ));
        };
        if matching.next().is_some() {
            return Err(format!(
                "ambiguous semantic mechanic {kind:?} at ({}, {})",
                position.x, position.y
            ));
        }
        Ok(entity.id())
    }

    pub(crate) const fn authored_kind(class: u8, subtype: u8) -> Option<LevelEntityKind> {
        if class == 10
            && let Some(kind) = Self::effect_kind(subtype)
        {
            return Some(LevelEntityKind::Mechanic(
                MechanicKind::EnvironmentalEffect { kind },
            ));
        }
        match (class, subtype) {
            (10, 22) => Some(LevelEntityKind::Mechanic(MechanicKind::Field {
                kind: FieldKind::Wind,
            })),
            (10, 33) => Some(LevelEntityKind::Mechanic(MechanicKind::Field {
                kind: FieldKind::TimedController,
            })),
            (10, 34) => Some(LevelEntityKind::Mechanic(MechanicKind::Field {
                kind: FieldKind::Transport,
            })),
            (10, 36) => Some(LevelEntityKind::Mechanic(MechanicKind::Field {
                kind: FieldKind::SkeletonSummon,
            })),
            (10, model @ (9 | 11 | 15 | 71)) => {
                Some(LevelEntityKind::Mechanic(MechanicKind::TerrainDisaster {
                    kind: Self::terrain_disaster_kind(model),
                }))
            }
            (5, model) => match Self::creature_for_model(model) {
                Some(kind) => Some(LevelEntityKind::Creature(kind)),
                None => None,
            },
            (2, 0) => Some(LevelEntityKind::Scenery(SceneryKind::Tree)),
            (2, 1 | 3) => Some(LevelEntityKind::Scenery(SceneryKind::Stone)),
            (2, 2) => Some(LevelEntityKind::Scenery(SceneryKind::Dolmen)),
            (2, 4 | 5) => Some(LevelEntityKind::Scenery(SceneryKind::StaticObject)),
            (2, 6) => Some(LevelEntityKind::Scenery(SceneryKind::Mushroom)),
            (2, 7 | 8) => Some(LevelEntityKind::Scenery(SceneryKind::BallisticObject)),
            (10, 45) => Some(LevelEntityKind::Building(BuildingKind::House)),
            (10, model @ 80..=86) => match Self::cave_feature_kind(model) {
                Some(kind) => Some(LevelEntityKind::Mechanic(MechanicKind::CaveFeature {
                    kind,
                })),
                None => None,
            },
            (14, 3) => Some(LevelEntityKind::Scenery(SceneryKind::ExitPortal)),
            (14, 2) => Some(LevelEntityKind::Mechanic(MechanicKind::CaveCorridor)),
            (14, 0) => Some(LevelEntityKind::Item(ItemKind::PassiveItem)),
            (14, 5) => Some(LevelEntityKind::Item(ItemKind::ExperienceScroll)),
            (11, 0) => Some(LevelEntityKind::Switch(SwitchKind::AreaEntryTrigger)),
            (11, 1) => Some(LevelEntityKind::Switch(SwitchKind::AreaExitTrigger)),
            (11, 2) => Some(LevelEntityKind::Switch(
                SwitchKind::RepeatingAreaEntryTrigger,
            )),
            (11, 3) => Some(LevelEntityKind::Switch(
                SwitchKind::RepeatingAreaExitTrigger,
            )),
            (11, 4) => Some(LevelEntityKind::Switch(SwitchKind::LevelExitGate)),
            (11, 5..=11) => Some(LevelEntityKind::Switch(SwitchKind::EmptyTrigger)),
            (11, 12) => Some(LevelEntityKind::Switch(SwitchKind::ExitSequenceTrigger)),
            (11, 32) => Some(LevelEntityKind::Switch(SwitchKind::ObjectiveGate)),
            (11, 31) => Some(LevelEntityKind::Switch(SwitchKind::SecretEntryTrigger)),
            (11, 13..=15 | 18..=29 | 33..=44) => {
                Some(LevelEntityKind::Switch(SwitchKind::CreatureAbsenceTrigger))
            }
            (11, 30) => Some(LevelEntityKind::Switch(
                SwitchKind::PopulationAbsenceTrigger,
            )),
            (11, 16) => Some(LevelEntityKind::Mechanic(MechanicKind::AbsenceCountdown {
                creature: CreatureKind::Worm,
            })),
            (11, 17) => Some(LevelEntityKind::Mechanic(MechanicKind::AbsenceCountdown {
                creature: CreatureKind::Archer,
            })),
            (15, 0) => Some(LevelEntityKind::SpellPickup(SpellKind::Fireball)),
            (15, 1) => Some(LevelEntityKind::SpellPickup(SpellKind::Possession)),
            (15, 2) => Some(LevelEntityKind::SpellPickup(SpellKind::Castle)),
            (15, 3) => Some(LevelEntityKind::SpellPickup(SpellKind::Speed)),
            (15, 4) => Some(LevelEntityKind::SpellPickup(SpellKind::Morph)),
            (15, 5) => Some(LevelEntityKind::SpellPickup(SpellKind::Heal)),
            (15, 6) => Some(LevelEntityKind::SpellPickup(SpellKind::Shield)),
            (15, 7) => Some(LevelEntityKind::SpellPickup(SpellKind::Lightning)),
            (15, 8) => Some(LevelEntityKind::SpellPickup(SpellKind::Rebound)),
            (15, 9) => Some(LevelEntityKind::SpellPickup(SpellKind::Meteor)),
            (15, 10) => Some(LevelEntityKind::SpellPickup(SpellKind::Teleport)),
            (15, 11) => Some(LevelEntityKind::SpellPickup(SpellKind::Invisibility)),
            (15, 12) => Some(LevelEntityKind::SpellPickup(SpellKind::BeyondSight)),
            (15, 13) => Some(LevelEntityKind::SpellPickup(SpellKind::StealMana)),
            (15, 14) => Some(LevelEntityKind::SpellPickup(SpellKind::Duel)),
            (15, 15) => Some(LevelEntityKind::SpellPickup(SpellKind::Tremor)),
            (15, 16) => Some(LevelEntityKind::SpellPickup(SpellKind::Crater)),
            (15, 17) => Some(LevelEntityKind::SpellPickup(SpellKind::Earthquake)),
            (15, 18) => Some(LevelEntityKind::SpellPickup(SpellKind::Volcano)),
            (15, 19) => Some(LevelEntityKind::SpellPickup(SpellKind::SummonArmy)),
            (15, 20) => Some(LevelEntityKind::SpellPickup(SpellKind::GravityWell)),
            (15, 21) => Some(LevelEntityKind::SpellPickup(SpellKind::Whirlwind)),
            (15, 22) => Some(LevelEntityKind::SpellPickup(SpellKind::FoolsMana)),
            (15, 23) => Some(LevelEntityKind::SpellPickup(SpellKind::MagicMine)),
            (15, 24) => Some(LevelEntityKind::SpellPickup(SpellKind::Alliance)),
            (15, 25) => Some(LevelEntityKind::SpellPickup(SpellKind::CaveIn)),
            _ => None,
        }
    }

    pub(crate) const fn creature_for_model(model: u8) -> Option<CreatureKind> {
        match model {
            0 => Some(CreatureKind::Dragon),
            1 => Some(CreatureKind::Goat),
            2 => Some(CreatureKind::Bee),
            3 => Some(CreatureKind::Worm),
            4 => Some(CreatureKind::Archer),
            9 => Some(CreatureKind::Skeleton),
            10 => Some(CreatureKind::FinalBoss),
            12 => Some(CreatureKind::Builder),
            13 => Some(CreatureKind::Villager),
            14 => Some(CreatureKind::Trader),
            15 => Some(CreatureKind::CastleDefender),
            16 => Some(CreatureKind::Wyvern),
            17 => Some(CreatureKind::Creature17),
            18 => Some(CreatureKind::Creature18),
            19 => Some(CreatureKind::Firefly),
            20 => Some(CreatureKind::Spider),
            21 => Some(CreatureKind::Creature21),
            22 => Some(CreatureKind::Creature22),
            23 => Some(CreatureKind::Creature23),
            24 => Some(CreatureKind::Creature24),
            25 => Some(CreatureKind::Cymmerian),
            26 => Some(CreatureKind::Creature26),
            27 => Some(CreatureKind::Creature27),
            28 => Some(CreatureKind::Creature28),
            _ => None,
        }
    }

    pub(crate) const fn creature_for_reset_selector(
        class: u8,
        model: u16,
        action: u8,
    ) -> Option<CreatureKind> {
        if class != 5 || model > 255 || matches!(action, 180 | 232 | 234) {
            return None;
        }
        if model == 15 && !matches!(action, 120..=125 | 127) {
            return None;
        }
        Self::creature_for_model(model as u8)
    }

    pub(crate) const fn creature_for_house_resident(model: u8, action: u8) -> Option<CreatureKind> {
        match (model, action) {
            (4, 32..=39) => Some(CreatureKind::Archer),
            (12, 97) => Some(CreatureKind::Builder),
            (13, 105) => Some(CreatureKind::Villager),
            (14, 113) => Some(CreatureKind::Trader),
            _ => None,
        }
    }

    pub(crate) const fn mechanic_for_reset_selector(
        class: u8,
        model: u16,
        action: u8,
    ) -> Option<MechanicKind> {
        match (class, model, action) {
            (10, 22, 22) => Some(MechanicKind::Field {
                kind: FieldKind::Wind,
            }),
            (10, 33, 35) => Some(MechanicKind::Field {
                kind: FieldKind::TimedController,
            }),
            (10, 34, 36) => Some(MechanicKind::Field {
                kind: FieldKind::Transport,
            }),
            (10, 36, 38) => Some(MechanicKind::Field {
                kind: FieldKind::SkeletonSummon,
            }),
            (10, model @ 80..=86, action) if action == model as u8 + 7 => {
                match Self::cave_feature_kind(model as u8) {
                    Some(kind) => Some(MechanicKind::CaveFeature { kind }),
                    None => None,
                }
            }
            (14, 2, 7) => Some(MechanicKind::CaveCorridor),
            (11, 16, 16) => Some(MechanicKind::AbsenceCountdown {
                creature: CreatureKind::Worm,
            }),
            (11, 17, 17) => Some(MechanicKind::AbsenceCountdown {
                creature: CreatureKind::Archer,
            }),
            _ => None,
        }
    }

    const fn terrain_disaster_kind(model: u8) -> TerrainDisasterKind {
        match model {
            9 => TerrainDisasterKind::Volcano,
            11 => TerrainDisasterKind::LoweringRing,
            15 => TerrainDisasterKind::Earthquake,
            71 => TerrainDisasterKind::Tremor,
            _ => unreachable!(),
        }
    }

    pub(crate) const fn effect_kind(model: u8) -> Option<EffectKind> {
        Some(match model {
            0 => EffectKind::Impact,
            1 => EffectKind::RadialExplosion,
            2 => EffectKind::Expiry,
            3 => EffectKind::ShortVisual,
            4 => EffectKind::DormantGroundMarker,
            5 => EffectKind::Ash,
            6 => EffectKind::GroundFire,
            7 => EffectKind::ImmediateVisual,
            13 => EffectKind::Smoke,
            14 => EffectKind::DarkSmoke,
            16 => EffectKind::BurningDebris,
            18 => EffectKind::Eruption,
            19 => EffectKind::EruptionSmoke,
            23 => EffectKind::LightningEndpoint,
            38 => EffectKind::LightningStorm,
            60 => EffectKind::AmbientSmoke,
            70 => EffectKind::PossessionContact,
            76 => EffectKind::FireSpheres,
            87 => EffectKind::FadingSmoke,
            _ => return None,
        })
    }

    pub(crate) const fn cave_feature_kind(model: u8) -> Option<CaveFeatureKind> {
        Some(match model {
            80 => CaveFeatureKind::PathNode,
            81 => CaveFeatureKind::Passage,
            82 => CaveFeatureKind::Room,
            83 => CaveFeatureKind::RoundObstruction,
            84 => CaveFeatureKind::CeilingPendant,
            85 => CaveFeatureKind::FloorMound,
            86 => CaveFeatureKind::GroundVent,
            _ => return None,
        })
    }

    pub(crate) const fn cave_feature_definition(
        feature: AuthoredFeature,
    ) -> Option<CaveFeatureDefinition> {
        if feature.kind != 10 {
            return None;
        }
        let startup = feature.activation_group == -1;
        Some(match feature.subtype {
            80 => CaveFeatureDefinition::PathNode {
                half_width_tiles: if startup {
                    feature.parameter_3 as u8 & 15
                } else {
                    0
                },
            },
            81 => CaveFeatureDefinition::Passage {
                start_radius_tiles: 2,
                end_radius_tiles: 4,
                destination: TilePosition { x: 0, y: 0 },
            },
            82 => CaveFeatureDefinition::Room {
                half_width_tiles: if startup {
                    feature.parameter_1 as u8
                } else {
                    3
                },
                half_depth_tiles: if startup {
                    feature.parameter_2 as u8
                } else {
                    3
                },
                clearance_steps: if startup {
                    feature.parameter_3 as u8
                } else {
                    2
                },
            },
            83 => CaveFeatureDefinition::RoundObstruction {
                radius_tiles: feature.word_10 as i16,
            },
            84 => CaveFeatureDefinition::CeilingPendant {
                radius_tiles: feature.word_10 as i16,
                height_factor: feature.parameter_3 as i16,
            },
            85 => CaveFeatureDefinition::FloorMound {
                radius_tiles: feature.word_10 as i16,
                height_factor: feature.parameter_3 as i16,
            },
            86 => CaveFeatureDefinition::GroundVent { lifetime: 9 },
            _ => return None,
        })
    }

    pub(crate) const fn scenery_for_reset_selector(
        class: u8,
        model: u16,
        action: u8,
    ) -> Option<SceneryKind> {
        match (class, model, action) {
            (2, 0, 0) => Some(SceneryKind::Tree),
            (2, 1, 3) | (2, 3, 9) => Some(SceneryKind::Stone),
            (2, 2, 6) => Some(SceneryKind::Dolmen),
            (2, 4, 12) | (2, 5, 15) => Some(SceneryKind::StaticObject),
            (2, 6, 18 | 19) => Some(SceneryKind::Mushroom),
            (2, 7, 20) | (2, 8, 21) => Some(SceneryKind::BallisticObject),
            (14, 3, 8) => Some(SceneryKind::ExitPortal),
            _ => None,
        }
    }

    pub(crate) const fn building_for_reset_selector(
        class: u8,
        model: u16,
        action: u8,
    ) -> Option<BuildingKind> {
        match (class, model, action) {
            (10, 45, 52) => Some(BuildingKind::House),
            _ => None,
        }
    }

    pub(crate) const fn switch_for_reset_selector(
        class: u8,
        model: u16,
        action: u8,
    ) -> Option<SwitchKind> {
        match (class, model, action) {
            (11, 0, 0) => Some(SwitchKind::AreaEntryTrigger),
            (11, 1, 1) => Some(SwitchKind::AreaExitTrigger),
            (11, 2, 2) => Some(SwitchKind::RepeatingAreaEntryTrigger),
            (11, 3, 3) => Some(SwitchKind::RepeatingAreaExitTrigger),
            (11, 4, 4) => Some(SwitchKind::LevelExitGate),
            (11, 5..=11, _) if model == action as u16 => Some(SwitchKind::EmptyTrigger),
            (11, 12, 12) => Some(SwitchKind::ExitSequenceTrigger),
            (11, 32, 32) => Some(SwitchKind::ObjectiveGate),
            (11, 31, 31) => Some(SwitchKind::SecretEntryTrigger),
            (11, 13..=15 | 18..=29 | 33..=44, _) if model == action as u16 => {
                Some(SwitchKind::CreatureAbsenceTrigger)
            }
            (11, 30, 30) => Some(SwitchKind::PopulationAbsenceTrigger),
            _ => None,
        }
    }

    pub(crate) const fn spell_for_reset_selector(
        class: u8,
        model: u16,
        action: u8,
    ) -> Option<SpellKind> {
        match (class, model, action) {
            (15, 0, 1 | 2) => Some(SpellKind::Fireball),
            (15, 1, 4 | 5) => Some(SpellKind::Possession),
            (15, 3, 10) => Some(SpellKind::Speed),
            (15, 5, 16) => Some(SpellKind::Heal),
            (15, 6, 19) => Some(SpellKind::Shield),
            (15, 8, 25) => Some(SpellKind::Rebound),
            (15, 9, 28 | 29) => Some(SpellKind::Meteor),
            (15, 7, 22 | 23) => Some(SpellKind::Lightning),
            (15, 10, 31) => Some(SpellKind::Teleport),
            (15, 11, 34) => Some(SpellKind::Invisibility),
            (15, 12, 37) => Some(SpellKind::BeyondSight),
            (15, 13, 40 | 41) => Some(SpellKind::StealMana),
            (15, 14, 43 | 44) => Some(SpellKind::Duel),
            (15, 15, 46 | 47) => Some(SpellKind::Tremor),
            (15, 16, 49 | 50) => Some(SpellKind::Crater),
            (15, 17, 52 | 53) => Some(SpellKind::Earthquake),
            (15, 18, 55 | 56) => Some(SpellKind::Volcano),
            (15, 19, 58 | 59) => Some(SpellKind::SummonArmy),
            (15, 20, 61 | 62) => Some(SpellKind::GravityWell),
            (15, 21, 64 | 65) => Some(SpellKind::Whirlwind),
            (15, 22, 67 | 68) => Some(SpellKind::FoolsMana),
            (15, 23, 70 | 71) => Some(SpellKind::MagicMine),
            (15, 24, 73 | 74) => Some(SpellKind::Alliance),
            (15, 25, 76 | 77) => Some(SpellKind::CaveIn),
            _ => None,
        }
    }

    pub(crate) fn from_authored_bytes(
        bytes: &[u8],
        objective_operations: &[ObjectiveOperation],
        selection: CampaignSelection,
        building_parameters: Option<&BTreeMap<u8, HouseParameter>>,
    ) -> Result<Self, String> {
        Self::bind_authored(
            bytes,
            objective_operations,
            selection,
            building_parameters,
            false,
        )
    }

    /// Non-executable projection of the known entity bindings. Unsupported
    /// source rows remain in the caller's complete decoded package.
    pub(crate) fn catalog_binding(
        bytes: &[u8],
        selection: CampaignSelection,
    ) -> Result<Self, String> {
        Self::bind_authored(bytes, &[], selection, None, true)
    }

    fn bind_authored(
        bytes: &[u8],
        objective_operations: &[ObjectiveOperation],
        selection: CampaignSelection,
        building_parameters: Option<&BTreeMap<u8, HouseParameter>>,
        entities_only: bool,
    ) -> Result<Self, String> {
        let features = parse_authored_features(bytes)?;
        let mut stable_entity_ids = stable_entity_ids(selection.external_level)?;
        let mut stable_objective_ids = stable_objective_ids(selection.external_level)?;
        if entities_only {
            stable_objective_ids.clear();
        }
        if selection.external_level > 3 {
            // The sealed opening identities retain their explicit manifests.
            // Further immutable packages use semantic role and authored place,
            // with an occurrence suffix for coincident placements. No source
            // arena address or selector becomes a public entity identity.
            let mut occurrences = BTreeMap::<String, u16>::new();
            for feature in &features {
                let (Ok(class), Ok(subtype)) =
                    (u8::try_from(feature.kind), u8::try_from(feature.subtype))
                else {
                    continue;
                };
                let Some(kind) = Self::authored_kind(class, subtype) else {
                    continue;
                };
                let role = match kind {
                    LevelEntityKind::Creature(kind) => format!("creature.{}", kind.as_str()),
                    LevelEntityKind::SpellPickup(kind) => format!("spell_pickup.{}", kind.as_str()),
                    LevelEntityKind::Scenery(kind) => format!("scenery.{}", kind.as_str()),
                    LevelEntityKind::Building(kind) => format!("building.{}", kind.as_str()),
                    LevelEntityKind::Item(kind) => format!("item.{}", kind.as_str()),
                    LevelEntityKind::Switch(kind) => format!("switch.{}", kind.as_str()),
                    LevelEntityKind::Mechanic(kind) => format!("mechanic.{}", kind.as_str()),
                };
                let mut label = format!("{role}.at_{}_{}", feature.tile_x, feature.tile_y);
                let ordinal = occurrences.entry(label.clone()).or_default();
                *ordinal += 1;
                if *ordinal > 1 {
                    label.push_str(&format!(".instance_{ordinal}"));
                }
                stable_entity_ids.insert(
                    feature.index,
                    (
                        stable_entity_id(&label).map_err(|error| error.to_string())?,
                        LevelEntityPositionAccess::ReadOnly,
                    ),
                );
            }
            for operation in objective_operations {
                let label = format!("objective.campaign.task_{}", operation.objective_slot + 1);
                stable_objective_ids.insert(
                    u16::from(operation.objective_slot),
                    (
                        stable_objective_id(&label).map_err(|error| error.to_string())?,
                        LevelObjectiveConditionAccess::ReadOnly,
                    ),
                );
            }
        }
        // These shared effect families were absent from the opening identity
        // manifest. Give their actual authored placements stable semantic IDs
        // without changing any established opening entity identity.
        let mut shared_occurrences = BTreeMap::<String, u16>::new();
        for feature in &features {
            let Ok(model) = u8::try_from(feature.subtype) else {
                continue;
            };
            let role = match (feature.kind, model) {
                (10, model @ (9 | 11 | 15 | 71)) => {
                    format!("mechanic.{}", Self::terrain_disaster_kind(model).as_str())
                }
                (10, 22) => "mechanic.wind".to_owned(),
                (10, 34) => "mechanic.transport".to_owned(),
                (10, model) => {
                    let Some(kind) = Self::effect_kind(model) else {
                        continue;
                    };
                    format!("mechanic.{}", kind.as_str())
                }
                (5, 15) => "creature.castle_defender".to_owned(),
                (2, 2) => "scenery.dolmen".to_owned(),
                (2, 4 | 5) => "scenery.static_object".to_owned(),
                (2, 6) => "scenery.mushroom".to_owned(),
                (2, 7 | 8) => "scenery.ballistic_object".to_owned(),
                _ => continue,
            };
            if stable_entity_ids.contains_key(&feature.index) {
                continue;
            }
            let mut label = format!("{}.at_{}_{}", role, feature.tile_x, feature.tile_y);
            let ordinal = shared_occurrences.entry(label.clone()).or_default();
            *ordinal += 1;
            if *ordinal > 1 {
                label.push_str(&format!(".instance_{ordinal}"));
            }
            stable_entity_ids.insert(
                feature.index,
                (
                    stable_entity_id(&label).map_err(|error| error.to_string())?,
                    LevelEntityPositionAccess::ReadOnly,
                ),
            );
        }
        let mut creature_by_authored_record = BTreeMap::new();
        let mut creature_model_by_kind = BTreeMap::new();
        let mut spell_by_authored_record = BTreeMap::new();
        let mut spell_family_by_kind = BTreeMap::new();
        let mut building_by_authored_record = BTreeMap::new();
        let mut scenery_by_authored_record = BTreeMap::new();
        let mut item_by_authored_record = BTreeMap::new();
        let mut item_entity_by_authored_record = BTreeMap::new();
        let mut switch_by_authored_record = BTreeMap::new();
        let mut mechanic_by_authored_record = BTreeMap::new();
        let mut mechanic_entity_by_authored_record = BTreeMap::new();
        let mut authored_record_by_entity_id = BTreeMap::new();
        let mut movable_runtime_hint_by_entity_id = BTreeMap::new();
        let mut standing_building_chains = BTreeMap::new();
        let mut rival_wizard_selectors = BTreeMap::new();
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
                (5, 10) => Some(CreatureKind::FinalBoss),
                (5, 18) => Some(CreatureKind::Creature18),
                (5, 21) => Some(CreatureKind::Creature21),
                (5, 27) => Some(CreatureKind::Creature27),
                (5, 12) => Some(CreatureKind::Builder),
                (5, 13) => Some(CreatureKind::Villager),
                (5, 14) => Some(CreatureKind::Trader),
                (5, 15) => Some(CreatureKind::CastleDefender),
                (5, 16) => Some(CreatureKind::Wyvern),
                (5, 17) => Some(CreatureKind::Creature17),
                (5, 23) => Some(CreatureKind::Creature23),
                (5, 19) => Some(CreatureKind::Firefly),
                (5, 20) => Some(CreatureKind::Spider),
                (5, 22) => Some(CreatureKind::Creature22),
                (5, 24) => Some(CreatureKind::Creature24),
                (5, 25) => Some(CreatureKind::Cymmerian),
                (5, 26) => Some(CreatureKind::Creature26),
                (5, 28) => Some(CreatureKind::Creature28),
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
                        | CreatureKind::FinalBoss
                        | CreatureKind::Goat
                        | CreatureKind::Archer
                        | CreatureKind::CastleDefender
                        | CreatureKind::Wyvern
                        | CreatureKind::Creature17
                        | CreatureKind::Creature18
                        | CreatureKind::Creature21
                        | CreatureKind::Creature27
                        | CreatureKind::Creature23
                        | CreatureKind::Creature26
                        | CreatureKind::Creature28
                        | CreatureKind::Cymmerian
                        | CreatureKind::Creature22
                        | CreatureKind::Creature24
                        | CreatureKind::Bee
                        | CreatureKind::Dragon
                        | CreatureKind::Skeleton
                        | CreatureKind::Spider
                        | CreatureKind::Trader
                        | CreatureKind::Villager
                        | CreatureKind::Builder
                        | CreatureKind::Worm
                ) {
                    let max_life = match kind {
                        CreatureKind::FinalBoss => {
                            crate::level_definition::FINAL_BOSS_CLASS_PROPERTIES.max_life
                        }
                        CreatureKind::Firefly => {
                            crate::level_definition::FIREFLY_CLASS_PROPERTIES.max_life
                        }
                        CreatureKind::Goat => crate::goat::GOAT_CONSTRUCTOR_LIFE,
                        CreatureKind::Archer => {
                            crate::level_definition::ARCHER_CLASS_PROPERTIES.max_life
                        }
                        CreatureKind::CastleDefender => {
                            crate::level_definition::CASTLE_DEFENDER_CLASS_PROPERTIES.max_life
                                as i32
                        }
                        CreatureKind::Wyvern => {
                            crate::level_definition::WYVERN_CLASS_PROPERTIES.max_life
                        }
                        CreatureKind::Creature17 => {
                            crate::level_definition::CREATURE17_CLASS_PROPERTIES.max_life
                        }
                        CreatureKind::Creature18 => {
                            crate::level_definition::CREATURE18_CLASS_PROPERTIES.max_life
                        }
                        CreatureKind::Creature21 => {
                            crate::level_definition::CREATURE21_CLASS_PROPERTIES.max_life
                        }
                        CreatureKind::Creature27 => {
                            crate::level_definition::CREATURE27_CLASS_PROPERTIES.root_max_life
                        }
                        CreatureKind::Creature23 => {
                            crate::level_definition::CREATURE23_CLASS_PROPERTIES.max_life
                        }
                        CreatureKind::Creature26 => {
                            crate::level_definition::CREATURE26_CLASS_PROPERTIES.max_life
                        }
                        CreatureKind::Creature28 => {
                            crate::level_definition::CREATURE28_CLASS_PROPERTIES.maximum_life
                        }
                        CreatureKind::Cymmerian => {
                            crate::level_definition::CYMMERIAN_CLASS_PROPERTIES.max_life
                        }
                        CreatureKind::Creature22 => {
                            crate::level_definition::CREATURE22_CLASS_PROPERTIES.max_life
                        }
                        CreatureKind::Creature24 => {
                            crate::level_definition::CREATURE24_CLASS_PROPERTIES.max_life
                        }
                        CreatureKind::Bee => {
                            i32::from(crate::native_world::level2_class5::BEE_MAX_LIFE)
                        }
                        CreatureKind::Dragon => crate::native_world::level2_class5::DRAGON_LIFE,
                        CreatureKind::Skeleton => {
                            crate::level_definition::SKELETON_CLASS_PROPERTIES.max_life
                        }
                        CreatureKind::Spider => {
                            crate::level_definition::SPIDER_CLASS_PROPERTIES.max_life
                        }
                        CreatureKind::Trader => crate::native_world::level2_class5::TRADER_MAX_LIFE,
                        CreatureKind::Villager => crate::villager::VILLAGER_MAX_LIFE,
                        CreatureKind::Builder => {
                            crate::level_definition::BUILDER_CLASS_PROPERTIES.max_life
                        }
                        CreatureKind::Worm => crate::native_world::level2_class5::WORM_MAX_LIFE,
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
                (2, 1 | 3) => Some(SceneryKind::Stone),
                (2, 2) => Some(SceneryKind::Dolmen),
                (2, 4 | 5) => Some(SceneryKind::StaticObject),
                (2, 6) => Some(SceneryKind::Mushroom),
                (2, 7 | 8) => Some(SceneryKind::BallisticObject),
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
                entities.push(if kind == SceneryKind::Mushroom {
                    mushroom_entity_definition(
                        id,
                        position,
                        activation_group,
                        position_access,
                        MushroomDefinition::default(),
                    )
                } else if kind == SceneryKind::Dolmen {
                    dolmen_entity_definition(
                        id, position, activation_group, position_access,
                        DolmenDefinition::default(),
                    )
                } else if kind == SceneryKind::BallisticObject {
                    ballistic_scenery_entity_definition(
                        id, position, activation_group, position_access,
                        BallisticSceneryDefinition::default(),
                    )
                } else if kind == SceneryKind::StaticObject {
                    static_scenery_entity_definition(
                        id, position, activation_group, position_access,
                        StaticSceneryDefinition::default(),
                    )
                } else {
                    entity_definition_with_position_access(
                        id,
                        LevelEntityKind::Scenery(kind),
                        position,
                        activation_group,
                        position_access,
                    )
                });
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
                let authored_group_house = selection == CampaignSelection::LEVEL1
                    && (13..=32).contains(&feature.activation_group)
                    && feature.parameter_1
                        == u16::from(crate::house::AUTHORED_HOUSE_BUILDING_INDEX);
                if authored_group_house {
                    let group = activation_group.ok_or_else(|| {
                        format!(
                            "authored grouped House {} has no activation group",
                            feature.index
                        )
                    })?;
                    let life_mana_scalar =
                        HouseParameter::level_one(crate::house::AUTHORED_HOUSE_BUILDING_INDEX)
                            .expect("qualified Level 1 BLDGPRM row 37")
                            .life_mana_scalar;
                    entities.push(house_construction_entity_definition(
                        id.clone(),
                        position,
                        group,
                        position_access,
                        life_mana_scalar,
                        LevelEntityParameterAccess::SessionReset,
                        crate::house::AUTHORED_HOUSE_CONSTRUCTION_VISITS,
                        LevelEntityParameterAccess::SessionReset,
                    ));
                } else {
                    entities.push(entity_definition_with_position_access(
                        id,
                        LevelEntityKind::Building(kind),
                        position,
                        activation_group,
                        position_access,
                    ));
                }
                building_by_authored_record.insert(feature.index, kind);
                continue;
            }

            // Model0 is the persistent passive item from sub_51530. Model5
            // uses UpdateScroll_59C80 to award experience on collection.
            // Portals share class14 but are scenery, not inventory items.
            let item_kind = match (feature.kind, feature.subtype) {
                (14, 0) => Some(ItemKind::PassiveItem),
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
                entities.push(match kind {
                    ItemKind::PassiveItem => passive_item_entity_definition(
                        id.clone(), position, activation_group, position_access,
                        PassiveItemDefinition::default(),
                    ),
                    ItemKind::ExperienceScroll => experience_scroll_entity_definition(
                        id.clone(),
                        position,
                        activation_group,
                        position_access,
                        crate::native_world::LEVEL2_EXPERIENCE_SCROLL_AWARD.cast_unsigned(),
                        LevelEntityParameterAccess::SessionReset,
                    ),
                });
                item_by_authored_record.insert(feature.index, kind);
                item_entity_by_authored_record.insert(feature.index, id);
                continue;
            }

            let switch_kind = match (feature.kind, feature.subtype) {
                // These one-shot area triggers activate their authored group
                // on entry or exit respectively. `AddSwitch0B_04_6F150`
                // waits for the package's level-complete player state.
                (11, 0) => Some(SwitchKind::AreaEntryTrigger),
                (11, 1) => Some(SwitchKind::AreaExitTrigger),
                (11, 2) => Some(SwitchKind::RepeatingAreaEntryTrigger),
                (11, 3) => Some(SwitchKind::RepeatingAreaExitTrigger),
                (11, 4) => Some(SwitchKind::LevelExitGate),
                (11, 5..=11) => Some(SwitchKind::EmptyTrigger),
                // This overlap trigger starts the player-owned exit sequence;
                // it does not activate its stage tag as another area group.
                (11, 12) => Some(SwitchKind::ExitSequenceTrigger),
                // `AddSwitch0B_20_6F1C0` waits for one objective-state
                // selector, then activates the row's authored group.
                (11, 32) => Some(SwitchKind::ObjectiveGate),
                (11, 31) => Some(SwitchKind::SecretEntryTrigger),
                (11, 13..=15 | 18..=29 | 33..=44) => Some(SwitchKind::CreatureAbsenceTrigger),
                (11, 30) => Some(SwitchKind::PopulationAbsenceTrigger),
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
                if matches!(
                    kind,
                    SwitchKind::RepeatingAreaEntryTrigger
                        | SwitchKind::RepeatingAreaExitTrigger
                        | SwitchKind::SecretEntryTrigger
                        | SwitchKind::CreatureAbsenceTrigger
                        | SwitchKind::PopulationAbsenceTrigger
                        | SwitchKind::EmptyTrigger
                ) {
                    let overlap = matches!(
                        kind,
                        SwitchKind::RepeatingAreaEntryTrigger
                            | SwitchKind::RepeatingAreaExitTrigger
                            | SwitchKind::SecretEntryTrigger
                    )
                    .then_some(feature.word_10);
                    let countdown = matches!(
                        kind,
                        SwitchKind::CreatureAbsenceTrigger | SwitchKind::PopulationAbsenceTrigger
                    )
                    .then(|| {
                        absence_countdown_definition(16, LevelEntityParameterAccess::SessionReset)
                    });
                    entities.push(activation_switch_entity_definition(
                        id,
                        kind,
                        position,
                        activation_group,
                        position_access,
                        overlap,
                        countdown,
                    ));
                } else if kind == SwitchKind::AreaEntryTrigger
                    // Level 2 row 111 is consumed by the dedicated switch-25
                    // model-76 route, which has no word-10 overlap carrier.
                    && !(selection.external_level == 2 && feature.index == 111)
                {
                    if !(1..=127).contains(&feature.word_10) {
                        return Err(format!(
                            "authored Area Entry trigger {} overlap extent is outside 1..=127",
                            feature.index
                        ));
                    }
                    entities.push(area_entry_trigger_entity_definition(
                        id,
                        position,
                        activation_group,
                        position_access,
                        feature.word_10,
                        LevelEntityParameterAccess::SessionReset,
                    ));
                } else if kind == SwitchKind::AreaExitTrigger {
                    if !(1..=127).contains(&feature.word_10) {
                        return Err(format!(
                            "authored Area Exit trigger {} overlap extent is outside 1..=127",
                            feature.index
                        ));
                    }
                    entities.push(area_exit_trigger_entity_definition(
                        id,
                        position,
                        activation_group,
                        position_access,
                        feature.word_10,
                        LevelEntityParameterAccess::SessionReset,
                    ));
                } else if kind == SwitchKind::ExitSequenceTrigger {
                    let group = activation_group.ok_or_else(|| {
                        format!(
                            "authored Exit Sequence trigger {} has no activation group",
                            feature.index
                        )
                    })?;
                    if !(1..=127).contains(&feature.word_10) {
                        return Err(format!(
                            "authored Exit Sequence trigger {} overlap extent is outside 1..=127",
                            feature.index
                        ));
                    }
                    entities.push(exit_sequence_trigger_entity_definition(
                        id,
                        position,
                        group,
                        position_access,
                        feature.word_10,
                        LevelEntityParameterAccess::SessionReset,
                    ));
                } else {
                    entities.push(entity_definition_with_position_access(
                        id,
                        LevelEntityKind::Switch(kind),
                        position,
                        activation_group,
                        position_access,
                    ));
                }
                switch_by_authored_record.insert(feature.index, kind);
                continue;
            }

            let spell_kind = match (feature.kind, feature.subtype) {
                // `AddSpellXX_XX_51120` receives these spell identities. Keep
                // the numeric selectors inside the private MC2 adapter.
                (15, 0) => Some(SpellKind::Fireball),
                (15, 1) => Some(SpellKind::Possession),
                (15, 2) => Some(SpellKind::Castle),
                (15, 3) => Some(SpellKind::Speed),
                (15, 4) => Some(SpellKind::Morph),
                (15, 5) => Some(SpellKind::Heal),
                (15, 6) => Some(SpellKind::Shield),
                (15, 8) => Some(SpellKind::Rebound),
                (15, 9) => Some(SpellKind::Meteor),
                (15, 7) => Some(SpellKind::Lightning),
                (15, 10) => Some(SpellKind::Teleport),
                (15, 11) => Some(SpellKind::Invisibility),
                (15, 12) => Some(SpellKind::BeyondSight),
                (15, 13) => Some(SpellKind::StealMana),
                (15, 14) => Some(SpellKind::Duel),
                (15, 15) => Some(SpellKind::Tremor),
                (15, 16) => Some(SpellKind::Crater),
                (15, 17) => Some(SpellKind::Earthquake),
                (15, 18) => Some(SpellKind::Volcano),
                (15, 19) => Some(SpellKind::SummonArmy),
                (15, 20) => Some(SpellKind::GravityWell),
                (15, 21) => Some(SpellKind::Whirlwind),
                (15, 22) => Some(SpellKind::FoolsMana),
                (15, 23) => Some(SpellKind::MagicMine),
                (15, 24) => Some(SpellKind::Alliance),
                (15, 25) => Some(SpellKind::CaveIn),
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
                (10, 22) => MechanicKind::Field {
                    kind: FieldKind::Wind,
                },
                (10, 33) => MechanicKind::Field {
                    kind: FieldKind::TimedController,
                },
                (10, 34) => MechanicKind::Field {
                    kind: FieldKind::Transport,
                },
                (10, 36) => MechanicKind::Field {
                    kind: FieldKind::SkeletonSummon,
                },
                (10, model @ 80..=86) => MechanicKind::CaveFeature {
                    kind: Self::cave_feature_kind(model as u8).expect("bounded cave model"),
                },
                (10, model)
                    if u8::try_from(model)
                        .ok()
                        .and_then(Self::effect_kind)
                        .is_some() =>
                {
                    MechanicKind::EnvironmentalEffect {
                        kind: Self::effect_kind(u8::try_from(model).expect("guarded effect model"))
                            .expect("guarded effect kind"),
                    }
                }
                (10, model @ (9 | 11 | 15 | 71)) => MechanicKind::TerrainDisaster {
                    kind: Self::terrain_disaster_kind(model as u8),
                },
                (14, 2) => MechanicKind::CaveCorridor,
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
            if let MechanicKind::Field { kind } = mechanic_kind {
                let config = mc2_field_definition(
                    kind,
                    TilePosition {
                        x: feature.parameter_2,
                        y: feature.parameter_1,
                    },
                );
                entities.push(field_entity_definition(
                    id.clone(),
                    position,
                    activation_group,
                    position_access,
                    config,
                ));
            } else if let MechanicKind::TerrainDisaster { kind } = mechanic_kind {
                entities.push(terrain_disaster_entity_definition(
                    id.clone(),
                    position,
                    activation_group,
                    position_access,
                    mc2_authored_terrain_disaster_definition(kind, feature.parameter_1)?,
                ));
            } else if let MechanicKind::EnvironmentalEffect { kind } = mechanic_kind {
                entities.push(effect_entity_definition(
                    id.clone(),
                    position,
                    activation_group,
                    position_access,
                    mc2_effect_definition(kind),
                ));
            } else if let Some(config) = Self::cave_feature_definition(*feature) {
                entities.push(cave_feature_entity_definition(
                    id.clone(),
                    position,
                    activation_group,
                    position_access,
                    config,
                ));
            } else if mechanic_kind == MechanicKind::CaveCorridor {
                let orientation = if feature.parameter_1 != 0 {
                    CorridorOrientation::AlongX
                } else {
                    CorridorOrientation::AlongY
                };
                entities.push(cave_corridor_entity_definition(
                    id.clone(),
                    position,
                    activation_group,
                    position_access,
                    cave_corridor_definition(orientation, feature.parameter_3 as i16),
                ));
            } else {
                entities.push(absence_countdown_entity_definition(
                    id.clone(),
                    LevelEntityKind::Mechanic(mechanic_kind),
                    position,
                    activation_group,
                    position_access,
                    absence_countdown_definition(16, LevelEntityParameterAccess::SessionReset),
                ));
            }
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
                    1 | 2 | 4 | 6 => {
                        let authored_record = u16::try_from(operation.objective_selector)
                            .map_err(|_| format!("objective {} has invalid authored target {}", operation.objective_slot, operation.objective_selector))?;
                        let target = authored_record_by_entity_id.iter()
                            .find(|(_, record)| **record == authored_record)
                            .map(|(id, _)| id.clone())
                            .ok_or_else(|| format!("objective {} targets unsupported authored entity {}", operation.objective_slot, authored_record))?;
                        match operation.stage_index {
                            1 => ObjectiveCondition::TargetDeath(target),
                            2 => ObjectiveCondition::TargetDeathWithoutFont(target),
                            4 => ObjectiveCondition::OwnedTargetAtLocation {
                                target,
                                location: TilePosition {
                                    x: u16::try_from(operation.axis_x).map_err(|_| "delivery objective has invalid x".to_owned())?,
                                    y: u16::try_from(operation.axis_y).map_err(|_| "delivery objective has invalid y".to_owned())?,
                                },
                            },
                            6 => ObjectiveCondition::CollectedTarget(target),
                            _ => unreachable!(),
                        }
                    }
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
                        let target = super::objectives::rival_wizard::target_from_selector(player_selector)
                            .map_err(|error| error.to_string())?;
                        rival_wizard_selectors.insert(target.id().clone(), target.selector());
                        ObjectiveCondition::DefeatRivalWizard(target.id().clone())
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
                                if let Some(feature) = features.iter().find(|feature| feature.index == authored_record && feature.kind == 5) {
                                    return format!("creature objective {} targets unsupported MC2 creature model {} at authored record {}", operation.objective_slot, feature.subtype, authored_record);
                                }
                                format!(
                                    "creature objective {} targets non-creature authored record {}",
                                    operation.objective_slot, authored_record
                                )
                            })?;
                        ObjectiveCondition::NoLivingCreatures(kind)
                    }
                    8 => ObjectiveCondition::AllOtherPlayersBanished,
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
            scenery_entity_by_authored_record: authored_record_by_entity_id
                .iter()
                .filter(|(_, record)| scenery_by_authored_record.contains_key(*record))
                .map(|(id, record)| (*record, id.clone()))
                .collect(),
            switch_entity_by_authored_record: authored_record_by_entity_id
                .iter()
                .filter(|(_, record)| switch_by_authored_record.contains_key(*record))
                .map(|(id, record)| (*record, id.clone()))
                .collect(),
            creature_entity_by_authored_record: authored_record_by_entity_id
                .into_iter()
                .filter(|(_, record)| creature_by_authored_record.contains_key(record))
                .map(|(id, record)| (record, id))
                .collect(),
            semantic: build_level_definition(entities, objectives, activation_groups)?,
            creature_by_authored_record,
            creature_model_by_kind,
            spell_by_authored_record,
            spell_family_by_kind,
            building_by_authored_record,
            scenery_by_authored_record,
            item_by_authored_record,
            item_entity_by_authored_record,
            switch_by_authored_record,
            mechanic_by_authored_record,
            mechanic_entity_by_authored_record,
            movable_runtime_hint_by_entity_id,
            standing_building_chains,
            rival_wizard_selectors,
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
                if feature.activation_group > 0 && supports_authorable_creature_placement(kind) =>
            {
                Mc2MovableRuntimeHint::AuthoredFeature(feature.index)
            }
            LevelEntityKind::Creature(kind)
                if feature.activation_group == 0
                    && supports_authorable_creature_placement(kind) =>
            {
                if kind == CreatureKind::Villager {
                    // Multiple qualified Level 2 Villagers share a package
                    // tile. Their validated authored/source record is the
                    // exact private carrier identity; position is not.
                    Mc2MovableRuntimeHint::AuthoredRecord(feature.index)
                } else {
                    Mc2MovableRuntimeHint::ResetKindAndPosition
                }
            }
            LevelEntityKind::Building(BuildingKind::House) => {
                if feature.activation_group > 0
                    && (13..=32).contains(&feature.activation_group)
                    && feature.parameter_1 == u16::from(crate::house::AUTHORED_HOUSE_BUILDING_INDEX)
                {
                    Mc2MovableRuntimeHint::AuthoredFeature(feature.index)
                } else {
                    // Authored record and reset source identities differ, and
                    // qualified packages may contain duplicate House tiles.
                    // The validated occurrence join resolves the private row.
                    Mc2MovableRuntimeHint::AuthoredRecord(feature.index)
                }
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
            LevelEntityKind::Mechanic(MechanicKind::Field { .. }) => {
                Mc2MovableRuntimeHint::AuthoredFeature(feature.index)
            }
            LevelEntityKind::Scenery(SceneryKind::Mushroom | SceneryKind::BallisticObject | SceneryKind::Dolmen | SceneryKind::StaticObject) => {
                if feature.activation_group != 0 {
                    Mc2MovableRuntimeHint::AuthoredFeature(feature.index)
                } else {
                    Mc2MovableRuntimeHint::ResetKindAndPosition
                }
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

#[cfg(test)]
mod spell_family_binding_tests {
    use super::*;

    #[test]
    fn ordinary_level_seven_builders_bind_authored_constructor_health() {
        let package = std::env::var_os("MC_LEVEL7_SETTLEMENT_PACKAGE").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/settlement-20261105-integration/owner-current/level7")
            },
            std::path::PathBuf::from,
        );
        let campaign = crate::campaign::CampaignSession::open(
            &package,
            CampaignSelection::ordinary(7).unwrap(),
        )
        .expect("current whole Level7 owner package");
        let mut simulation = campaign.simulation().clone();
        for authored in [139, 140] {
            let feature = simulation
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == authored)
                .unwrap();
            assert_eq!((feature.kind, feature.subtype), (5, 12));
            assert_eq!(
                simulation
                    .session
                    .definition
                    .authored_creature_max_life(authored),
                Some(crate::level_definition::BUILDER_CLASS_PROPERTIES.max_life as u32),
                "the ordinary constructor must receive its typed authored health binding"
            );
        }
        let feature = *simulation
            .mission()
            .authored_features()
            .iter()
            .find(|feature| feature.index == 139)
            .unwrap();
        let original = simulation.semantic_level_definition().unwrap();
        let entity = original
            .entities()
            .iter()
            .find(|entity| {
                entity.kind() == LevelEntityKind::Creature(CreatureKind::Builder)
                    && entity.position()
                        == TilePosition {
                            x: feature.tile_x,
                            y: feature.tile_y,
                        }
            })
            .unwrap();
        let id = entity.id().clone();
        let mut encoded = serde_json::to_value(original).unwrap();
        let supplied = encoded["entities"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|entity| entity["id"].as_str() == Some(id.as_str()))
            .unwrap();
        supplied["creature_health"]["max_life"] = serde_json::json!(40_000);
        let variant: LevelDefinition = serde_json::from_value(encoded).unwrap();
        variant.verify_session_reset_variant_of(original).unwrap();
        assert_eq!(
            variant
                .entity(&id)
                .unwrap()
                .creature_health()
                .unwrap()
                .default_max_life(),
            1_000
        );
        simulation
            .replace_semantic_level_definition(variant)
            .unwrap();
        let captured = simulation
            .economy()
            .house_residents()
            .iter()
            .filter(|resident| resident.initial_package && resident.model == 12)
            .map(|resident| {
                (
                    simulation
                        .session
                        .state
                        .live
                        .economy
                        .handle(resident.source_entity_index)
                        .unwrap(),
                    simulation.entities()[usize::from(resident.source_entity_index)].clone(),
                )
            })
            .collect::<Vec<_>>();
        assert!(
            !captured.is_empty(),
            "the actual package already captured its reset Builders"
        );
        for _ in 0..2 {
            // Explicit supplied-definition constructor control: row139 is a
            // real group0 record whose original actor was already captured.
            // Do not republish group0 or call this an ordinary second birth.
            let max_life = simulation
                .session
                .definition
                .authored_creature_max_life(feature.index)
                .unwrap();
            assert_eq!(max_life, 40_000);
            let live = &mut simulation.session.state.live;
            let slot = crate::campaign::authored_spawn::construct(
                &mut live.economy,
                &mut live.terrain,
                &mut live.core.runtime,
                feature,
                max_life,
                None,
            )
            .unwrap()
            .unwrap();
            let current = live
                .economy
                .house_residents()
                .iter()
                .find(|resident| resident.source_entity_index == slot)
                .unwrap();
            assert_eq!(
                (current.model, current.max_life, current.life),
                (12, 40_000, 40_000)
            );
            assert_eq!(
                (
                    live.economy.entities[usize::from(slot)].max_life,
                    live.economy.entities[usize::from(slot)].life
                ),
                (40_000, 40_000)
            );
            assert!(live.economy.handle(slot).is_some() && live.economy.map_chains.is_linked(slot));
            for (handle, original) in &captured {
                assert_eq!(live.economy.handle(handle.source_slot), Some(*handle));
                assert_eq!(
                    &live.economy.entities[usize::from(handle.source_slot)],
                    original
                );
            }
            simulation.reset();
        }
    }

    #[test]
    fn starting_spells_army_magic_mine_and_alliance_bind_ordinary_source_rows() {
        for (family, kind) in [
            (0, SpellKind::Fireball),
            (1, SpellKind::Possession),
            (19, SpellKind::SummonArmy),
            (23, SpellKind::MagicMine),
            (24, SpellKind::Alliance),
        ] {
            assert_eq!(
                Mc2LevelDefinition::authored_kind(15, family),
                Some(LevelEntityKind::SpellPickup(kind))
            );
            assert_eq!(
                Mc2LevelDefinition::spell_for_reset_selector(15, u16::from(family), family * 3 + 1),
                Some(kind)
            );
            assert_eq!(
                Mc2LevelDefinition::spell_for_reset_selector(15, u16::from(family), family * 3 + 2),
                Some(kind)
            );
            assert_eq!(
                Mc2LevelDefinition::spell_for_reset_selector(15, u16::from(family), family * 3),
                None,
                "active holder action is not a pickup source"
            );
        }
    }
}
