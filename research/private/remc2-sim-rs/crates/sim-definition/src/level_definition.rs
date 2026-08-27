//! Semantic, package-authored level content.
//!
//! This vocabulary is deliberately independent of the private MC2 source
//! selectors. A package adapter may translate its native records into these
//! values, while the shared engine and read-only inspection remain expressed
//! in game concepts.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CreatureKind {
    Dragon,
    Goat,
    Bee,
    Worm,
    Archer,
    Skeleton,
    Builder,
    Villager,
    Trader,
    Firefly,
}

impl CreatureKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dragon => "dragon",
            Self::Goat => "goat",
            Self::Bee => "bee",
            Self::Worm => "worm",
            Self::Archer => "archer",
            Self::Skeleton => "skeleton",
            Self::Builder => "builder",
            Self::Villager => "villager",
            Self::Trader => "trader",
            Self::Firefly => "firefly",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpellKind {
    Fireball,
    Possession,
    Castle,
    Speed,
    Morph,
    Shield,
    Invisibility,
    BeyondSight,
}

impl SpellKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fireball => "fireball",
            Self::Possession => "possession",
            Self::Castle => "castle",
            Self::Speed => "speed",
            Self::Morph => "morph",
            Self::Shield => "shield",
            Self::Invisibility => "invisibility",
            Self::BeyondSight => "beyond_sight",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanicKind {
    AbsenceCountdown { creature: CreatureKind },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneryKind {
    Tree,
    Stone,
    ExitPortal,
}

impl SceneryKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::Stone => "stone",
            Self::ExitPortal => "exit_portal",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    ExperienceScroll,
}

impl ItemKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExperienceScroll => "experience_scroll",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildingKind {
    House,
}

impl BuildingKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::House => "house",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SwitchKind {
    AreaEntryTrigger,
    AreaExitTrigger,
    LevelExitGate,
    ExitSequenceTrigger,
    ObjectiveGate,
}

impl SwitchKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AreaEntryTrigger => "area_entry_trigger",
            Self::AreaExitTrigger => "area_exit_trigger",
            Self::LevelExitGate => "level_exit_gate",
            Self::ExitSequenceTrigger => "exit_sequence_trigger",
            Self::ObjectiveGate => "objective_gate",
        }
    }
}

impl MechanicKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AbsenceCountdown { .. } => "absence_countdown",
        }
    }

    #[must_use]
    pub const fn creature(self) -> CreatureKind {
        match self {
            Self::AbsenceCountdown { creature } => creature,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct LevelEntityId(String);

impl LevelEntityId {
    pub(crate) fn creature(kind: CreatureKind, ordinal: u16) -> Self {
        Self(format!("creature.{}.{}", kind.as_str(), ordinal + 1))
    }

    pub(crate) fn spell_pickup(kind: SpellKind, ordinal: u16) -> Self {
        Self(format!("spell_pickup.{}.{}", kind.as_str(), ordinal + 1))
    }

    pub(crate) fn mechanic(kind: MechanicKind, ordinal: u16) -> Self {
        Self(format!(
            "mechanic.{}.{}.{}",
            kind.as_str(),
            kind.creature().as_str(),
            ordinal + 1
        ))
    }

    pub(crate) fn scenery(kind: SceneryKind, ordinal: u16) -> Self {
        Self(format!("scenery.{}.{}", kind.as_str(), ordinal + 1))
    }

    pub(crate) fn item(kind: ItemKind, ordinal: u16) -> Self {
        Self(format!("item.{}.{}", kind.as_str(), ordinal + 1))
    }

    pub(crate) fn building(kind: BuildingKind, ordinal: u16) -> Self {
        Self(format!("building.{}.{}", kind.as_str(), ordinal + 1))
    }

    pub(crate) fn switch(kind: SwitchKind, ordinal: u16) -> Self {
        Self(format!("switch.{}.{}", kind.as_str(), ordinal + 1))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TilePosition {
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ActivationGroupId(u16);

impl ActivationGroupId {
    pub(crate) const fn new(value: u16) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LevelEntityKind {
    Creature(CreatureKind),
    SpellPickup(SpellKind),
    Scenery(SceneryKind),
    Item(ItemKind),
    Building(BuildingKind),
    Switch(SwitchKind),
    Mechanic(MechanicKind),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectiveCondition {
    ReachLocation(TilePosition),
    DefeatRivalWizard,
    NoLivingCreatures(CreatureKind),
    CastleManaPercent(u16),
    NoStandingBuildings,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LevelObjectiveDefinition {
    condition: ObjectiveCondition,
}

impl LevelObjectiveDefinition {
    pub(crate) const fn new(condition: ObjectiveCondition) -> Self {
        Self { condition }
    }

    #[must_use]
    pub const fn condition(self) -> ObjectiveCondition {
        self.condition
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LevelEntityDefinition {
    id: LevelEntityId,
    kind: LevelEntityKind,
    position: TilePosition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    activation_group: Option<ActivationGroupId>,
}

impl LevelEntityDefinition {
    pub(crate) const fn new(
        id: LevelEntityId,
        kind: LevelEntityKind,
        position: TilePosition,
        activation_group: Option<ActivationGroupId>,
    ) -> Self {
        Self {
            id,
            kind,
            position,
            activation_group,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &LevelEntityId {
        &self.id
    }

    #[must_use]
    pub const fn kind(&self) -> LevelEntityKind {
        self.kind
    }

    #[must_use]
    pub const fn position(&self) -> TilePosition {
        self.position
    }

    #[must_use]
    pub const fn activation_group(&self) -> Option<ActivationGroupId> {
        self.activation_group
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct LevelDefinition {
    entities: Vec<LevelEntityDefinition>,
    objectives: Vec<LevelObjectiveDefinition>,
    activation_groups: Vec<ActivationGroupId>,
}

impl LevelDefinition {
    pub(crate) fn new(
        entities: Vec<LevelEntityDefinition>,
        objectives: Vec<LevelObjectiveDefinition>,
        activation_groups: Vec<ActivationGroupId>,
    ) -> Result<Self, String> {
        let mut ids = BTreeSet::new();
        for entity in &entities {
            if !ids.insert(entity.id.clone()) {
                return Err(format!(
                    "duplicate semantic level entity {}",
                    entity.id.as_str()
                ));
            }
        }
        let unique_groups = activation_groups.iter().copied().collect::<BTreeSet<_>>();
        if unique_groups.len() != activation_groups.len()
            || activation_groups
                .windows(2)
                .any(|groups| groups[0] >= groups[1])
        {
            return Err("semantic activation groups must be unique and sorted".to_owned());
        }
        Ok(Self {
            entities,
            objectives,
            activation_groups,
        })
    }

    #[must_use]
    pub fn entities(&self) -> &[LevelEntityDefinition] {
        &self.entities
    }

    #[must_use]
    pub fn objectives(&self) -> &[LevelObjectiveDefinition] {
        &self.objectives
    }

    pub fn objective_conditions(&self) -> impl Iterator<Item = ObjectiveCondition> + '_ {
        self.objectives.iter().map(|objective| objective.condition)
    }

    pub fn activation_groups(&self) -> impl Iterator<Item = ActivationGroupId> + '_ {
        self.activation_groups.iter().copied()
    }

    #[must_use]
    pub fn authored_creature_count(&self, kind: CreatureKind) -> usize {
        self.entities
            .iter()
            .filter(|entity| entity.kind == LevelEntityKind::Creature(kind))
            .count()
    }

    pub fn creature_kinds(&self) -> impl Iterator<Item = CreatureKind> + '_ {
        self.entities
            .iter()
            .filter_map(|entity| match entity.kind {
                LevelEntityKind::Creature(kind) => Some(kind),
                LevelEntityKind::SpellPickup(_)
                | LevelEntityKind::Scenery(_)
                | LevelEntityKind::Item(_)
                | LevelEntityKind::Building(_)
                | LevelEntityKind::Switch(_)
                | LevelEntityKind::Mechanic(_) => None,
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
    }

    #[must_use]
    pub fn authored_spell_pickup_count(&self, kind: SpellKind) -> usize {
        self.entities
            .iter()
            .filter(|entity| entity.kind == LevelEntityKind::SpellPickup(kind))
            .count()
    }

    pub fn spell_kinds(&self) -> impl Iterator<Item = SpellKind> + '_ {
        self.entities
            .iter()
            .filter_map(|entity| match entity.kind {
                LevelEntityKind::Creature(_) => None,
                LevelEntityKind::SpellPickup(kind) => Some(kind),
                LevelEntityKind::Scenery(_)
                | LevelEntityKind::Item(_)
                | LevelEntityKind::Building(_)
                | LevelEntityKind::Switch(_)
                | LevelEntityKind::Mechanic(_) => None,
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
    }

    #[must_use]
    pub fn authored_mechanic_count(&self, kind: MechanicKind) -> usize {
        self.entities
            .iter()
            .filter(|entity| entity.kind == LevelEntityKind::Mechanic(kind))
            .count()
    }

    pub fn mechanic_kinds(&self) -> impl Iterator<Item = MechanicKind> + '_ {
        self.entities
            .iter()
            .filter_map(|entity| match entity.kind {
                LevelEntityKind::Creature(_)
                | LevelEntityKind::SpellPickup(_)
                | LevelEntityKind::Scenery(_)
                | LevelEntityKind::Item(_)
                | LevelEntityKind::Building(_)
                | LevelEntityKind::Switch(_) => None,
                LevelEntityKind::Mechanic(kind) => Some(kind),
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
    }

    #[must_use]
    pub fn authored_scenery_count(&self, kind: SceneryKind) -> usize {
        self.entities
            .iter()
            .filter(|entity| entity.kind == LevelEntityKind::Scenery(kind))
            .count()
    }

    pub fn scenery_kinds(&self) -> impl Iterator<Item = SceneryKind> + '_ {
        self.entities
            .iter()
            .filter_map(|entity| match entity.kind {
                LevelEntityKind::Creature(_)
                | LevelEntityKind::SpellPickup(_)
                | LevelEntityKind::Item(_)
                | LevelEntityKind::Building(_)
                | LevelEntityKind::Switch(_)
                | LevelEntityKind::Mechanic(_) => None,
                LevelEntityKind::Scenery(kind) => Some(kind),
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
    }

    #[must_use]
    pub fn authored_item_count(&self, kind: ItemKind) -> usize {
        self.entities
            .iter()
            .filter(|entity| entity.kind == LevelEntityKind::Item(kind))
            .count()
    }

    pub fn item_kinds(&self) -> impl Iterator<Item = ItemKind> + '_ {
        self.entities
            .iter()
            .filter_map(|entity| match entity.kind {
                LevelEntityKind::Creature(_)
                | LevelEntityKind::SpellPickup(_)
                | LevelEntityKind::Scenery(_)
                | LevelEntityKind::Building(_)
                | LevelEntityKind::Switch(_)
                | LevelEntityKind::Mechanic(_) => None,
                LevelEntityKind::Item(kind) => Some(kind),
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
    }

    #[must_use]
    pub fn authored_building_count(&self, kind: BuildingKind) -> usize {
        self.entities
            .iter()
            .filter(|entity| entity.kind == LevelEntityKind::Building(kind))
            .count()
    }

    pub fn building_kinds(&self) -> impl Iterator<Item = BuildingKind> + '_ {
        self.entities
            .iter()
            .filter_map(|entity| match entity.kind {
                LevelEntityKind::Creature(_)
                | LevelEntityKind::SpellPickup(_)
                | LevelEntityKind::Scenery(_)
                | LevelEntityKind::Item(_)
                | LevelEntityKind::Switch(_)
                | LevelEntityKind::Mechanic(_) => None,
                LevelEntityKind::Building(kind) => Some(kind),
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
    }

    #[must_use]
    pub fn authored_switch_count(&self, kind: SwitchKind) -> usize {
        self.entities
            .iter()
            .filter(|entity| entity.kind == LevelEntityKind::Switch(kind))
            .count()
    }

    pub fn switch_kinds(&self) -> impl Iterator<Item = SwitchKind> + '_ {
        self.entities
            .iter()
            .filter_map(|entity| match entity.kind {
                LevelEntityKind::Creature(_)
                | LevelEntityKind::SpellPickup(_)
                | LevelEntityKind::Scenery(_)
                | LevelEntityKind::Item(_)
                | LevelEntityKind::Building(_)
                | LevelEntityKind::Mechanic(_) => None,
                LevelEntityKind::Switch(kind) => Some(kind),
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
    }
}
