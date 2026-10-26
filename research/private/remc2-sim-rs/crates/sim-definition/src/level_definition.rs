//! Semantic, package-authored level content.
//!
//! This vocabulary is deliberately independent of the private MC2 source
//! selectors. A package adapter may translate its native records into these
//! values, while the shared engine and read-only inspection remain expressed
//! in game concepts.

use crate::{DefinitionError, DefinitionErrorCode, SemanticId};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;

mod cave_features;
pub use cave_features::{CaveFeatureDefinition, CaveFeatureKind};
mod creatures;
pub use creatures::{
    FINAL_BOSS_CLASS_PROPERTIES, FinalBossClassProperties,
    CYMMERIAN_CLASS_PROPERTIES, CymmerianClassProperties,
    CREATURE28_CLASS_PROPERTIES, Creature28ClassProperties,
    BEE_CLASS_PROPERTIES, BeeClassProperties, CREATURE17_CLASS_PROPERTIES,
    CREATURE18_CLASS_PROPERTIES, CREATURE21_CLASS_PROPERTIES, CREATURE22_CLASS_PROPERTIES,
    CREATURE23_CLASS_PROPERTIES, CREATURE24_CLASS_PROPERTIES, CREATURE26_CLASS_PROPERTIES,
    Creature17ClassProperties, Creature18ClassProperties, Creature21ClassProperties,
    Creature22ClassProperties, Creature23ClassProperties, Creature24ClassProperties,
    Creature26ClassProperties, WYVERN_CLASS_PROPERTIES, WyvernClassProperties,
};
mod effects;
pub use effects::{EffectDefinition, EffectKind};
mod directional_spells;
mod disaster_spells;
pub use directional_spells::{DUEL_TIERS, DirectionalSpellTier, STEAL_MANA_TIERS};
pub use disaster_spells::{
    CRATER_PROPERTIES, DisasterSpellProperties, DisasterSpellTier, EARTHQUAKE_PROPERTIES,
    TREMOR_PROPERTIES, VOLCANO_PROPERTIES,
};
mod fields;
pub use fields::{FieldDefinition, FieldKind};
mod field_spells;
pub use field_spells::{FieldSpellTier, GRAVITY_WELL_TIERS, WHIRLWIND_TIERS};
pub mod late_spells;
mod summon_army;
pub use summon_army::{
    MC2_SOURCE_SUMMON_ARMY_PROPERTIES, SummonArmyEnvironment, SummonArmyMember,
    SummonArmyProperties, SummonArmySelection, SummonArmyTier, SummonArmyTierProperties,
};
mod meteor;
pub use meteor::{METEOR_PROPERTIES, MeteorProperties, MeteorTier};
mod lightning;
pub use lightning::{LIGHTNING_PROPERTIES, LightningProperties, LightningTier};
mod scenery;
pub use scenery::MushroomDefinition;
mod spider;
pub use spider::{SPIDER_CLASS_PROPERTIES, SpiderClassProperties};
mod terrain_disasters;
pub use terrain_disasters::{TerrainDisasterDefinition, TerrainDisasterKind};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CreatureKind {
    Dragon,
    Goat,
    Bee,
    Worm,
    Archer,
    CastleDefender,
    Skeleton,
    Builder,
    Villager,
    Trader,
    Wyvern,
    #[serde(rename = "creature_17")]
    Creature17,
    #[serde(rename = "creature_18")]
    Creature18,
    #[serde(rename = "creature_26")]
    Creature26,
    #[serde(rename = "creature_27")]
    Creature27,
    Cymmerian,
    Firefly,
    Spider,
    #[serde(rename = "creature_22")]
    Creature22,
    #[serde(rename = "creature_23")]
    Creature23,
    #[serde(rename = "creature_24")]
    Creature24,
    FinalBoss,
    #[serde(rename = "creature_21")]
    Creature21,
    #[serde(rename = "creature_28")]
    Creature28,
}

/// Typed class properties consumed by Archer construction, ground motion,
/// target selection and its real arrow producer. MC2 numeric selectors and
/// retained sprite/audio bindings remain in the package adapter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcherClassProperties {
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub attack_damage: u16,
    pub arrow_damage: u16,
    pub scan_period: u8,
    pub target_range: u32,
    pub target_half_angle: i16,
    pub turn_limit: i16,
    pub descent: i16,
    pub terrain_mask: u32,
    pub maximum_slope: i32,
    pub collision_half_width: i16,
    pub collision_half_height: i16,
}

pub const ARCHER_CLASS_PROPERTIES: ArcherClassProperties = ArcherClassProperties {
    minimum_speed: 30,
    maximum_speed: 0,
    attack_damage: 500,
    arrow_damage: 250,
    scan_period: 30,
    target_range: 5120,
    target_half_angle: 512,
    turn_limit: 22,
    descent: -128,
    terrain_mask: 0xfff0_80fe,
    maximum_slope: 20,
    collision_half_width: 128,
    collision_half_height: 256,
};

/// Properties of the Castle's ground defender. Source parameter and asset
/// identities remain in the MC2 adapter; the behavior owner consumes these.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CastleDefenderClassProperties {
    pub max_life: u32,
    pub mana: i32,
    pub max_mana: i32,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub attack_damage: u16,
    pub arrow_damage: u16,
    pub scan_period: u8,
    pub target_range: u32,
    pub target_half_angle: i16,
    pub turn_limit: i16,
    pub descent: i16,
    pub terrain_mask: u32,
    pub maximum_slope: i32,
    pub collision_half_width: i16,
    pub collision_half_height: i16,
}

pub const CASTLE_DEFENDER_CLASS_PROPERTIES: CastleDefenderClassProperties =
    CastleDefenderClassProperties {
        max_life: 1000,
        mana: 0,
        max_mana: 0,
        minimum_speed: 30,
        maximum_speed: 0,
        attack_damage: 500,
        arrow_damage: 100,
        scan_period: 15,
        target_range: 3840,
        target_half_angle: 512,
        turn_limit: 22,
        descent: -256,
        terrain_mask: 0x20000,
        maximum_slope: 20,
        collision_half_width: 128,
        collision_half_height: 128,
    };

/// Shared settlement ground rules, consumed by the population constructor
/// and its movement, household and damage callbacks.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SettlementClassProperties {
    pub max_life: i32,
    pub mana: i32,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub scan_period: u8,
    pub target_range: u16,
    pub separation_radius: i16,
    pub turn_limit: i16,
    pub ground_clearance: i16,
    pub descent: i16,
    pub terrain_mask: u32,
    pub maximum_slope: i32,
    pub behavior_flags: u8,
    pub collision_half_width: i16,
    pub collision_half_height: i16,
}

pub const VILLAGER_CLASS_PROPERTIES: SettlementClassProperties = SettlementClassProperties {
    max_life: 1000,
    mana: 0,
    minimum_speed: 54,
    maximum_speed: 18,
    scan_period: 40,
    target_range: 3584,
    separation_radius: 512,
    turn_limit: 22,
    ground_clearance: 0,
    descent: -256,
    terrain_mask: 0xffff_fefe,
    maximum_slope: 15,
    behavior_flags: 9,
    collision_half_width: 128,
    collision_half_height: 128,
};

pub const TRADER_CLASS_PROPERTIES: SettlementClassProperties = VILLAGER_CLASS_PROPERTIES;
pub const BUILDER_CLASS_PROPERTIES: SettlementClassProperties = SettlementClassProperties {
    maximum_speed: 24,
    ..VILLAGER_CLASS_PROPERTIES
};

impl CreatureKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dragon => "dragon",
            Self::Goat => "goat",
            Self::Bee => "bee",
            Self::Worm => "worm",
            Self::Archer => "archer",
            Self::CastleDefender => "castle_defender",
            Self::Skeleton => "skeleton",
            Self::Builder => "builder",
            Self::Villager => "villager",
            Self::Trader => "trader",
            Self::Wyvern => "wyvern",
            Self::Creature17 => "creature_17",
            Self::Creature18 => "creature_18",
            Self::Creature26 => "creature_26",
            Self::Creature27 => "creature_27",
            Self::Cymmerian => "cymmerian",
            Self::Firefly => "firefly",
            Self::Spider => "spider",
            Self::Creature22 => "creature_22",
            Self::Creature23 => "creature_23",
            Self::Creature24 => "creature_24",
            Self::FinalBoss => "final_boss",
            Self::Creature21 => "creature_21",
            Self::Creature28 => "creature_28",
        }
    }
}

/// Whether the shared runtime has a complete production, damage, hide, and
/// following-pass cleanup lifecycle for this authored creature family.
#[must_use]
pub const fn supports_complete_creature_lifecycle(kind: CreatureKind) -> bool {
    matches!(
        kind,
        CreatureKind::Bee
            | CreatureKind::Dragon
            | CreatureKind::Goat
            | CreatureKind::Archer
            | CreatureKind::Firefly
            | CreatureKind::Skeleton
            | CreatureKind::Spider
    )
}

/// Whether a package-present creature placement can be rebound to the shared
/// runtime carrier without claiming that family as an objective-absence target.
#[must_use]
pub const fn supports_authorable_creature_placement(kind: CreatureKind) -> bool {
    supports_complete_creature_lifecycle(kind)
        || matches!(
            kind,
            CreatureKind::FinalBoss
                | CreatureKind::Goat
                | CreatureKind::Worm
                | CreatureKind::Villager
                | CreatureKind::Trader
        )
}

/// Largest authored maximum-life value supported by the family's ordinary
/// constructor and damage carrier.
#[must_use]
pub const fn creature_max_life_upper_bound(kind: CreatureKind) -> Option<u32> {
    match kind {
        CreatureKind::Bee => Some(32_767),
        CreatureKind::Archer
        | CreatureKind::CastleDefender
        | CreatureKind::Creature22
        | CreatureKind::Creature23
        | CreatureKind::Creature24
        | CreatureKind::FinalBoss
        | CreatureKind::Creature21
        | CreatureKind::Creature28
        | CreatureKind::Dragon
        | CreatureKind::Firefly
        | CreatureKind::Goat
        | CreatureKind::Skeleton
        | CreatureKind::Spider
        | CreatureKind::Trader
        | CreatureKind::Villager
        | CreatureKind::Worm => Some(2_147_483_647),
        CreatureKind::Wyvern | CreatureKind::Creature17 | CreatureKind::Creature18
        | CreatureKind::Creature26 | CreatureKind::Creature27 | CreatureKind::Cymmerian => Some(2_147_483_647),
        _ => None,
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
    Heal,
    Shield,
    Rebound,
    Lightning,
    Meteor,
    Teleport,
    Invisibility,
    BeyondSight,
    StealMana,
    Duel,
    Tremor,
    Crater,
    Earthquake,
    Volcano,
    SummonArmy,
    GravityWell,
    Whirlwind,
    FoolsMana,
    MagicMine,
    Alliance,
    CaveIn,
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
            Self::Heal => "heal",
            Self::Shield => "shield",
            Self::Rebound => "rebound",
            Self::Lightning => "lightning",
            Self::Meteor => "meteor",
            Self::Teleport => "teleport",
            Self::Invisibility => "invisibility",
            Self::BeyondSight => "beyond_sight",
            Self::StealMana => "steal_mana",
            Self::Duel => "duel",
            Self::Tremor => "tremor",
            Self::Crater => "crater",
            Self::Earthquake => "earthquake",
            Self::Volcano => "volcano",
            Self::SummonArmy => "summon_army",
            Self::FoolsMana => "fools_mana",
            Self::MagicMine => "magic_mine",
            Self::Alliance => "alliance",
            Self::GravityWell => "gravity_well",
            Self::Whirlwind => "whirlwind",
            Self::CaveIn => "cave_in",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanicKind {
    Field { kind: FieldKind },
    AbsenceCountdown { creature: CreatureKind },
    CaveCorridor,
    CaveFeature { kind: CaveFeatureKind },
    EnvironmentalEffect { kind: EffectKind },
    TerrainDisaster { kind: TerrainDisasterKind },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CorridorOrientation {
    AlongX,
    AlongY,
}

/// Immutable corridor construction properties. The source length is twice
/// `extent + 2` tiles; retaining the signed extent also preserves source no-ops.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaveCorridorDefinition {
    orientation: CorridorOrientation,
    extent: i16,
}

impl CaveCorridorDefinition {
    pub(crate) const fn new(orientation: CorridorOrientation, extent: i16) -> Self {
        Self {
            orientation,
            extent,
        }
    }

    #[must_use]
    pub const fn orientation(self) -> CorridorOrientation {
        self.orientation
    }

    #[must_use]
    pub const fn extent(self) -> i16 {
        self.extent
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneryKind {
    Tree,
    Stone,
    Mushroom,
    ExitPortal,
}

impl SceneryKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::Stone => "stone",
            Self::Mushroom => "mushroom",
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
    RepeatingAreaEntryTrigger,
    RepeatingAreaExitTrigger,
    LevelExitGate,
    ExitSequenceTrigger,
    SecretEntryTrigger,
    ObjectiveGate,
    CreatureAbsenceTrigger,
    PopulationAbsenceTrigger,
    EmptyTrigger,
}

impl SwitchKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AreaEntryTrigger => "area_entry_trigger",
            Self::AreaExitTrigger => "area_exit_trigger",
            Self::RepeatingAreaEntryTrigger => "repeating_area_entry_trigger",
            Self::RepeatingAreaExitTrigger => "repeating_area_exit_trigger",
            Self::LevelExitGate => "level_exit_gate",
            Self::ExitSequenceTrigger => "exit_sequence_trigger",
            Self::SecretEntryTrigger => "secret_entry_trigger",
            Self::ObjectiveGate => "objective_gate",
            Self::CreatureAbsenceTrigger => "creature_absence_trigger",
            Self::PopulationAbsenceTrigger => "population_absence_trigger",
            Self::EmptyTrigger => "empty_trigger",
        }
    }
}

impl MechanicKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AbsenceCountdown { .. } => "absence_countdown",
            Self::CaveCorridor => "cave_corridor",
            Self::CaveFeature { kind } => kind.as_str(),
            Self::EnvironmentalEffect { kind } => kind.as_str(),
            Self::TerrainDisaster { kind } => kind.as_str(),
            Self::Field { kind } => kind.as_str(),
        }
    }

    #[must_use]
    pub const fn creature(self) -> Option<CreatureKind> {
        match self {
            Self::AbsenceCountdown { creature } => Some(creature),
            Self::CaveCorridor
            | Self::CaveFeature { .. }
            | Self::EnvironmentalEffect { .. }
            | Self::Field { .. }
            | Self::TerrainDisaster { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct LevelEntityId(String);

impl LevelEntityId {
    /// Parse one stable author-facing entity identity.
    ///
    /// # Errors
    ///
    /// Rejects IDs which expose private source selectors or Level-N layout.
    pub fn parse(value: &str) -> crate::Result<Self> {
        let semantic = SemanticId::parse(value)?;
        if semantic
            .as_str()
            .split('.')
            .any(is_private_source_identity_segment)
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidSemanticId,
                value,
                "level entity ID must be stable game meaning, not source selector identity",
            ));
        }
        Ok(Self(semantic.as_str().to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct LevelObjectiveId(String);

impl LevelObjectiveId {
    /// Parse one stable author-facing objective identity.
    ///
    /// # Errors
    ///
    /// Rejects IDs which expose private source selectors or Level-N layout.
    pub fn parse(value: &str) -> crate::Result<Self> {
        let semantic = SemanticId::parse(value)?;
        if semantic
            .as_str()
            .split('.')
            .any(is_private_source_identity_segment)
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidSemanticId,
                value,
                "level objective ID must be stable game meaning, not source selector identity",
            ));
        }
        Ok(Self(semantic.as_str().to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for LevelObjectiveId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for LevelEntityId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

/// Stable semantic identity for a rival wizard targeted by an objective.
///
/// The identity is package meaning, not a player slot or source-entity index.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RivalWizardId(String);

impl RivalWizardId {
    /// Parse one stable rival-wizard identity.
    ///
    /// # Errors
    ///
    /// Rejects source-shaped IDs and IDs outside the `rival_wizard` namespace.
    pub fn parse(value: &str) -> crate::Result<Self> {
        let semantic = SemanticId::parse(value)?;
        if !semantic.as_str().starts_with("rival_wizard.")
            || semantic
                .as_str()
                .split('.')
                .any(is_private_source_identity_segment)
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidSemanticId,
                value,
                "rival-wizard ID must use stable rival_wizard.* game meaning",
            ));
        }
        Ok(Self(semantic.as_str().to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for RivalWizardId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

pub(crate) fn is_private_source_identity_segment(segment: &str) -> bool {
    let components = segment.split('_').collect::<Vec<_>>();
    components
        .iter()
        .copied()
        .any(is_private_source_identity_component)
        || components.windows(2).any(|window| {
            window[0] == "level"
                && !window[1].is_empty()
                && window[1].bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn is_private_source_identity_component(component: &str) -> bool {
    if ["source", "raw"].into_iter().any(|prefix| {
        component == prefix
            || component.strip_prefix(prefix).is_some_and(|suffix| {
                !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
            })
    }) {
        return true;
    }
    if component.strip_prefix("level").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    }) {
        return true;
    }
    ["class", "model", "action", "selector", "family", "slot"]
        .into_iter()
        .any(|prefix| {
            component == prefix
                || component.strip_prefix(prefix).is_some_and(|suffix| {
                    !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
                })
        })
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
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

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LevelEntityPositionAccess {
    #[default]
    ReadOnly,
    SessionReset,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LevelEntityParameterAccess {
    #[default]
    ReadOnly,
    SessionReset,
}

/// Authorable configuration for one package-present absence countdown.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AbsenceCountdownDefinition {
    default_arming_ticks: u32,
    arming_ticks: u32,
    #[serde(default, skip_serializing_if = "is_read_only_entity_parameter")]
    arming_ticks_access: LevelEntityParameterAccess,
}

/// Immutable per-instance health configuration for one package-present
/// creature whose constructor and complete damage/death lifecycle are bound.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CreatureHealthDefinition {
    default_max_life: u32,
    max_life: u32,
    #[serde(default, skip_serializing_if = "is_read_only_entity_parameter")]
    max_life_access: LevelEntityParameterAccess,
}

/// Immutable per-instance reward carried by one package-present Experience Scroll.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExperienceScrollDefinition {
    default_experience_award: u32,
    experience_award: u32,
    #[serde(default, skip_serializing_if = "is_read_only_entity_parameter")]
    experience_award_access: LevelEntityParameterAccess,
}

/// Immutable construction parameter for one package-present grouped House
/// whose action-51 completion and live damage lifecycle are bound.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HouseConstructionDefinition {
    default_life_mana_scalar: u16,
    life_mana_scalar: u16,
    #[serde(default, skip_serializing_if = "is_read_only_entity_parameter")]
    life_mana_scalar_access: LevelEntityParameterAccess,
    default_construction_visits: u16,
    construction_visits: u16,
    #[serde(default, skip_serializing_if = "is_read_only_entity_parameter")]
    construction_visits_access: LevelEntityParameterAccess,
}

/// Immutable strict-overlap extent for one package-present Exit Sequence trigger.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SwitchOverlapDefinition {
    default_overlap_extent_tiles: u16,
    overlap_extent_tiles: u16,
    #[serde(default, skip_serializing_if = "is_read_only_entity_parameter")]
    overlap_extent_tiles_access: LevelEntityParameterAccess,
}

impl SwitchOverlapDefinition {
    pub(crate) const fn new(
        overlap_extent_tiles: u16,
        overlap_extent_tiles_access: LevelEntityParameterAccess,
    ) -> Self {
        Self {
            default_overlap_extent_tiles: overlap_extent_tiles,
            overlap_extent_tiles,
            overlap_extent_tiles_access,
        }
    }

    #[must_use]
    pub const fn default_overlap_extent_tiles(self) -> u16 {
        self.default_overlap_extent_tiles
    }

    #[must_use]
    pub const fn overlap_extent_tiles(self) -> u16 {
        self.overlap_extent_tiles
    }

    #[must_use]
    pub const fn overlap_extent_tiles_access(self) -> LevelEntityParameterAccess {
        self.overlap_extent_tiles_access
    }
}

impl HouseConstructionDefinition {
    pub(crate) const fn new(
        life_mana_scalar: u16,
        life_mana_scalar_access: LevelEntityParameterAccess,
        construction_visits: u16,
        construction_visits_access: LevelEntityParameterAccess,
    ) -> Self {
        Self {
            default_life_mana_scalar: life_mana_scalar,
            life_mana_scalar,
            life_mana_scalar_access,
            default_construction_visits: construction_visits,
            construction_visits,
            construction_visits_access,
        }
    }

    #[must_use]
    pub const fn default_life_mana_scalar(self) -> u16 {
        self.default_life_mana_scalar
    }

    #[must_use]
    pub const fn life_mana_scalar(self) -> u16 {
        self.life_mana_scalar
    }

    #[must_use]
    pub const fn life_mana_scalar_access(self) -> LevelEntityParameterAccess {
        self.life_mana_scalar_access
    }

    #[must_use]
    pub const fn default_construction_visits(self) -> u16 {
        self.default_construction_visits
    }

    #[must_use]
    pub const fn construction_visits(self) -> u16 {
        self.construction_visits
    }

    #[must_use]
    pub const fn construction_visits_access(self) -> LevelEntityParameterAccess {
        self.construction_visits_access
    }
}

impl ExperienceScrollDefinition {
    pub(crate) const fn new(
        experience_award: u32,
        experience_award_access: LevelEntityParameterAccess,
    ) -> Self {
        Self {
            default_experience_award: experience_award,
            experience_award,
            experience_award_access,
        }
    }

    #[must_use]
    pub const fn default_experience_award(self) -> u32 {
        self.default_experience_award
    }

    #[must_use]
    pub const fn experience_award(self) -> u32 {
        self.experience_award
    }

    #[must_use]
    pub const fn experience_award_access(self) -> LevelEntityParameterAccess {
        self.experience_award_access
    }
}

impl CreatureHealthDefinition {
    pub(crate) const fn new(max_life: u32, max_life_access: LevelEntityParameterAccess) -> Self {
        Self {
            default_max_life: max_life,
            max_life,
            max_life_access,
        }
    }

    #[must_use]
    pub const fn default_max_life(self) -> u32 {
        self.default_max_life
    }

    #[must_use]
    pub const fn max_life(self) -> u32 {
        self.max_life
    }

    #[must_use]
    pub const fn max_life_access(self) -> LevelEntityParameterAccess {
        self.max_life_access
    }
}

impl AbsenceCountdownDefinition {
    pub(crate) const fn new(
        arming_ticks: u32,
        arming_ticks_access: LevelEntityParameterAccess,
    ) -> Self {
        Self {
            default_arming_ticks: arming_ticks,
            arming_ticks,
            arming_ticks_access,
        }
    }

    #[must_use]
    pub const fn default_arming_ticks(self) -> u32 {
        self.default_arming_ticks
    }

    #[must_use]
    pub const fn arming_ticks(self) -> u32 {
        self.arming_ticks
    }

    #[must_use]
    pub const fn arming_ticks_access(self) -> LevelEntityParameterAccess {
        self.arming_ticks_access
    }
}

const fn is_read_only_entity_parameter(access: &LevelEntityParameterAccess) -> bool {
    matches!(access, LevelEntityParameterAccess::ReadOnly)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectiveCondition {
    ReachLocation(TilePosition),
    TargetDeath(LevelEntityId),
    TargetDeathWithoutFont(LevelEntityId),
    OwnedTargetAtLocation {
        target: LevelEntityId,
        location: TilePosition,
    },
    CollectedTarget(LevelEntityId),
    DefeatRivalWizard(RivalWizardId),
    AllOtherPlayersBanished,
    NoLivingCreatures(CreatureKind),
    CastleManaPercent(u16),
    NoStandingBuildings,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LevelObjectiveConditionAccess {
    #[default]
    ReadOnly,
    SessionReset,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LevelObjectiveDefinition {
    id: LevelObjectiveId,
    default_condition: ObjectiveCondition,
    condition: ObjectiveCondition,
    #[serde(default, skip_serializing_if = "is_read_only_objective_condition")]
    condition_access: LevelObjectiveConditionAccess,
}

impl LevelObjectiveDefinition {
    pub(crate) fn new(
        id: LevelObjectiveId,
        condition: ObjectiveCondition,
        condition_access: LevelObjectiveConditionAccess,
    ) -> Self {
        Self {
            id,
            default_condition: condition.clone(),
            condition,
            condition_access,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &LevelObjectiveId {
        &self.id
    }

    #[must_use]
    pub fn default_condition(&self) -> ObjectiveCondition {
        self.default_condition.clone()
    }

    #[must_use]
    pub fn condition(&self) -> ObjectiveCondition {
        self.condition.clone()
    }

    #[must_use]
    pub const fn condition_access(&self) -> LevelObjectiveConditionAccess {
        self.condition_access
    }
}

const fn is_read_only_objective_condition(access: &LevelObjectiveConditionAccess) -> bool {
    matches!(access, LevelObjectiveConditionAccess::ReadOnly)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LevelEntityDefinition {
    id: LevelEntityId,
    kind: LevelEntityKind,
    default_position: TilePosition,
    position: TilePosition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    activation_group: Option<ActivationGroupId>,
    #[serde(default, skip_serializing_if = "is_read_only_position")]
    position_access: LevelEntityPositionAccess,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    absence_countdown: Option<AbsenceCountdownDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cave_corridor: Option<CaveCorridorDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cave_feature: Option<CaveFeatureDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    effect: Option<EffectDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    terrain_disaster: Option<TerrainDisasterDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    field: Option<FieldDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mushroom: Option<MushroomDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    creature_health: Option<CreatureHealthDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    experience_scroll: Option<ExperienceScrollDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    house_construction: Option<HouseConstructionDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    switch_overlap: Option<SwitchOverlapDefinition>,
}

impl LevelEntityDefinition {
    pub(crate) const fn new(
        id: LevelEntityId,
        kind: LevelEntityKind,
        position: TilePosition,
        activation_group: Option<ActivationGroupId>,
        position_access: LevelEntityPositionAccess,
        absence_countdown: Option<AbsenceCountdownDefinition>,
        creature_health: Option<CreatureHealthDefinition>,
        experience_scroll: Option<ExperienceScrollDefinition>,
        house_construction: Option<HouseConstructionDefinition>,
        switch_overlap: Option<SwitchOverlapDefinition>,
    ) -> Self {
        Self {
            id,
            kind,
            default_position: position,
            position,
            activation_group,
            position_access,
            absence_countdown,
            cave_corridor: None,
            cave_feature: None,
            effect: None,
            terrain_disaster: None,
            field: None,
            mushroom: None,
            creature_health,
            experience_scroll,
            house_construction,
            switch_overlap,
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

    /// Package-authored position before any immutable authoring variant.
    #[must_use]
    pub const fn default_position(&self) -> TilePosition {
        self.default_position
    }

    #[must_use]
    pub const fn activation_group(&self) -> Option<ActivationGroupId> {
        self.activation_group
    }

    #[must_use]
    pub const fn position_access(&self) -> LevelEntityPositionAccess {
        self.position_access
    }

    #[must_use]
    pub const fn absence_countdown(&self) -> Option<AbsenceCountdownDefinition> {
        self.absence_countdown
    }

    #[must_use]
    pub const fn cave_corridor(&self) -> Option<CaveCorridorDefinition> {
        self.cave_corridor
    }

    pub(crate) const fn with_cave_corridor(mut self, config: CaveCorridorDefinition) -> Self {
        self.cave_corridor = Some(config);
        self
    }

    #[must_use]
    pub const fn cave_feature(&self) -> Option<CaveFeatureDefinition> {
        self.cave_feature
    }

    pub(crate) const fn with_cave_feature(mut self, config: CaveFeatureDefinition) -> Self {
        self.cave_feature = Some(config);
        self
    }

    #[must_use]
    pub const fn effect(&self) -> Option<EffectDefinition> {
        self.effect
    }

    pub(crate) const fn with_effect(mut self, config: EffectDefinition) -> Self {
        self.effect = Some(config);
        self
    }

    #[must_use]
    pub const fn terrain_disaster(&self) -> Option<TerrainDisasterDefinition> {
        self.terrain_disaster
    }

    pub(crate) const fn with_terrain_disaster(mut self, config: TerrainDisasterDefinition) -> Self {
        self.terrain_disaster = Some(config);
        self
    }

    #[must_use]
    pub const fn creature_health(&self) -> Option<CreatureHealthDefinition> {
        self.creature_health
    }

    #[must_use]
    pub const fn field(&self) -> Option<FieldDefinition> {
        self.field
    }

    pub(crate) const fn with_field(mut self, config: FieldDefinition) -> Self {
        self.field = Some(config);
        self
    }

    #[must_use]
    pub const fn mushroom(&self) -> Option<MushroomDefinition> {
        self.mushroom
    }

    pub(crate) const fn with_mushroom(mut self, config: MushroomDefinition) -> Self {
        self.mushroom = Some(config);
        self
    }

    #[must_use]
    pub const fn experience_scroll(&self) -> Option<ExperienceScrollDefinition> {
        self.experience_scroll
    }

    #[must_use]
    pub const fn house_construction(&self) -> Option<HouseConstructionDefinition> {
        self.house_construction
    }

    #[must_use]
    pub const fn switch_overlap(&self) -> Option<SwitchOverlapDefinition> {
        self.switch_overlap
    }
}

const fn is_read_only_position(access: &LevelEntityPositionAccess) -> bool {
    matches!(access, LevelEntityPositionAccess::ReadOnly)
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
            match (entity.kind, entity.mushroom) {
                (LevelEntityKind::Scenery(SceneryKind::Mushroom), Some(config)) => {
                    config.validate()?
                }
                (LevelEntityKind::Scenery(SceneryKind::Mushroom), _) => {
                    return Err(format!(
                        "mushroom {} lacks valid construction properties",
                        entity.id.as_str()
                    ));
                }
                (_, Some(_)) => {
                    return Err(format!(
                        "non-mushroom {} carries mushroom properties",
                        entity.id.as_str()
                    ));
                }
                (_, None) => {}
            }
            match (entity.kind, entity.field) {
                (LevelEntityKind::Mechanic(MechanicKind::Field { kind }), Some(config))
                    if kind == config.kind() =>
                {
                    config.validate()?
                }
                (LevelEntityKind::Mechanic(MechanicKind::Field { .. }), _) => {
                    return Err(format!(
                        "field {} lacks matching construction properties",
                        entity.id.as_str()
                    ));
                }
                (_, Some(_)) => {
                    return Err(format!(
                        "non-field {} carries field properties",
                        entity.id.as_str()
                    ));
                }
                (_, None) => {}
            }
            match (entity.kind, entity.terrain_disaster) {
                (
                    LevelEntityKind::Mechanic(MechanicKind::TerrainDisaster { kind }),
                    Some(config),
                ) if kind == config.kind()
                    && config.max_life() >= 0
                    && config.lifetime() >= 0
                    && config.half_width() >= 0
                    && config.half_height() >= 0 => {}
                (LevelEntityKind::Mechanic(MechanicKind::TerrainDisaster { .. }), _) => {
                    return Err(format!(
                        "terrain disaster {} lacks valid matching construction properties",
                        entity.id.as_str()
                    ));
                }
                (_, Some(_)) => {
                    return Err(format!(
                        "non-terrain entity {} carries disaster properties",
                        entity.id.as_str()
                    ));
                }
                (_, None) => {}
            }
            match (entity.kind, entity.effect) {
                (
                    LevelEntityKind::Mechanic(MechanicKind::EnvironmentalEffect { kind }),
                    Some(config),
                ) if kind == config.kind() => config.validate()?,
                (LevelEntityKind::Mechanic(MechanicKind::EnvironmentalEffect { .. }), _) => {
                    return Err(format!(
                        "effect {} lacks matching construction properties",
                        entity.id.as_str()
                    ));
                }
                (_, Some(_)) => {
                    return Err(format!(
                        "non-effect {} carries effect properties",
                        entity.id.as_str()
                    ));
                }
                (_, None) => {}
            }
            match (entity.kind, entity.cave_feature) {
                (LevelEntityKind::Mechanic(MechanicKind::CaveFeature { kind }), Some(config))
                    if kind == config.kind() =>
                {
                    config.validate()?
                }
                (LevelEntityKind::Mechanic(MechanicKind::CaveFeature { .. }), _) => {
                    return Err(format!(
                        "cave feature {} lacks matching construction properties",
                        entity.id.as_str()
                    ));
                }
                (_, Some(_)) => {
                    return Err(format!(
                        "non-cave entity {} carries cave construction properties",
                        entity.id.as_str()
                    ));
                }
                (_, None) => {}
            }
            if !ids.insert(entity.id.clone()) {
                return Err(format!(
                    "duplicate semantic level entity {}",
                    entity.id.as_str()
                ));
            }
            match (entity.kind, entity.cave_corridor) {
                (LevelEntityKind::Mechanic(MechanicKind::CaveCorridor), Some(_)) => {}
                (LevelEntityKind::Mechanic(MechanicKind::CaveCorridor), None) => {
                    return Err(format!(
                        "semantic cave corridor {} has no geometry configuration",
                        entity.id.as_str()
                    ));
                }
                (_, Some(_)) => {
                    return Err(format!(
                        "non-corridor entity {} carries cave-corridor configuration",
                        entity.id.as_str()
                    ));
                }
                (_, None) => {}
            }
            match (entity.kind, entity.absence_countdown) {
                (
                    LevelEntityKind::Mechanic(MechanicKind::AbsenceCountdown { .. })
                    | LevelEntityKind::Switch(
                        SwitchKind::CreatureAbsenceTrigger | SwitchKind::PopulationAbsenceTrigger,
                    ),
                    Some(config),
                ) if (1..=i32::MAX.cast_unsigned()).contains(&config.default_arming_ticks)
                    && (1..=i32::MAX.cast_unsigned()).contains(&config.arming_ticks) => {}
                (
                    LevelEntityKind::Mechanic(MechanicKind::AbsenceCountdown { .. })
                    | LevelEntityKind::Switch(
                        SwitchKind::CreatureAbsenceTrigger | SwitchKind::PopulationAbsenceTrigger,
                    ),
                    Some(_),
                ) => {
                    return Err(format!(
                        "semantic absence countdown {} has an invalid arming delay",
                        entity.id.as_str()
                    ));
                }
                (
                    LevelEntityKind::Mechanic(MechanicKind::AbsenceCountdown { .. })
                    | LevelEntityKind::Switch(
                        SwitchKind::CreatureAbsenceTrigger | SwitchKind::PopulationAbsenceTrigger,
                    ),
                    None,
                ) => {
                    return Err(format!(
                        "semantic absence countdown {} has no arming configuration",
                        entity.id.as_str()
                    ));
                }
                (_, Some(_)) => {
                    return Err(format!(
                        "non-countdown entity {} carries absence-countdown configuration",
                        entity.id.as_str()
                    ));
                }
                (_, None) => {}
            }
            match (entity.kind, entity.creature_health) {
                (LevelEntityKind::Creature(kind), Some(config))
                    if creature_max_life_upper_bound(kind).is_some_and(|maximum| {
                        (1..=maximum).contains(&config.default_max_life)
                            && (1..=maximum).contains(&config.max_life)
                    }) => {}
                (LevelEntityKind::Creature(kind), Some(_))
                    if creature_max_life_upper_bound(kind).is_some() =>
                {
                    return Err(format!(
                        "semantic {} {} has an invalid maximum life",
                        kind.as_str(),
                        entity.id.as_str()
                    ));
                }
                (LevelEntityKind::Creature(_), Some(_)) => {
                    return Err(format!(
                        "semantic creature {} exposes a maximum-life parameter without a bound family consumer",
                        entity.id.as_str()
                    ));
                }
                (_, Some(_)) => {
                    return Err(format!(
                        "non-creature entity {} carries creature-health configuration",
                        entity.id.as_str()
                    ));
                }
                (_, None) => {}
            }
            match (entity.kind, entity.experience_scroll) {
                (LevelEntityKind::Item(ItemKind::ExperienceScroll), Some(config))
                    if (1..=i32::MAX.cast_unsigned())
                        .contains(&config.default_experience_award)
                        && (1..=i32::MAX.cast_unsigned()).contains(&config.experience_award) => {}
                (LevelEntityKind::Item(ItemKind::ExperienceScroll), Some(_)) => {
                    return Err(format!(
                        "semantic Experience Scroll {} has an invalid experience award",
                        entity.id.as_str()
                    ));
                }
                (LevelEntityKind::Item(ItemKind::ExperienceScroll), None) => {
                    return Err(format!(
                        "semantic Experience Scroll {} has no reward configuration",
                        entity.id.as_str()
                    ));
                }
                (_, Some(_)) => {
                    return Err(format!(
                        "non-Experience-Scroll entity {} carries reward configuration",
                        entity.id.as_str()
                    ));
                }
                (_, None) => {}
            }
            match (entity.kind, entity.house_construction) {
                (LevelEntityKind::Building(BuildingKind::House), Some(config))
                    if config.default_life_mana_scalar > 0
                        && config.life_mana_scalar > 0
                        && config.default_construction_visits > 0
                        && config.construction_visits > 0 => {}
                (LevelEntityKind::Building(BuildingKind::House), Some(_)) => {
                    return Err(format!(
                        "semantic House {} has an invalid life/mana scalar",
                        entity.id.as_str()
                    ));
                }
                (_, Some(_)) => {
                    return Err(format!(
                        "non-House entity {} carries House construction configuration",
                        entity.id.as_str()
                    ));
                }
                (_, None) => {}
            }
            match (entity.kind, entity.switch_overlap) {
                (
                    LevelEntityKind::Switch(
                        SwitchKind::AreaEntryTrigger
                        | SwitchKind::AreaExitTrigger
                        | SwitchKind::RepeatingAreaEntryTrigger
                        | SwitchKind::RepeatingAreaExitTrigger
                        | SwitchKind::SecretEntryTrigger
                        | SwitchKind::ExitSequenceTrigger,
                    ),
                    Some(config),
                ) if (1..=127).contains(&config.default_overlap_extent_tiles)
                    && (1..=127).contains(&config.overlap_extent_tiles) => {}
                (
                    LevelEntityKind::Switch(
                        SwitchKind::AreaEntryTrigger
                        | SwitchKind::AreaExitTrigger
                        | SwitchKind::RepeatingAreaEntryTrigger
                        | SwitchKind::RepeatingAreaExitTrigger
                        | SwitchKind::SecretEntryTrigger
                        | SwitchKind::ExitSequenceTrigger,
                    ),
                    Some(_),
                ) => {
                    return Err(format!(
                        "semantic switch {} has an invalid overlap extent",
                        entity.id.as_str()
                    ));
                }
                (
                    LevelEntityKind::Switch(
                        SwitchKind::ExitSequenceTrigger
                        | SwitchKind::RepeatingAreaEntryTrigger
                        | SwitchKind::RepeatingAreaExitTrigger
                        | SwitchKind::SecretEntryTrigger,
                    ),
                    None,
                ) => {
                    return Err(format!(
                        "semantic switch {} has no overlap configuration",
                        entity.id.as_str()
                    ));
                }
                (LevelEntityKind::Switch(SwitchKind::AreaEntryTrigger), None) => {}
                (LevelEntityKind::Switch(SwitchKind::AreaExitTrigger), None) => {}
                (_, Some(_)) => {
                    return Err(format!(
                        "unsupported entity {} carries switch overlap configuration",
                        entity.id.as_str()
                    ));
                }
                (_, None) => {}
            }
        }
        let mut objective_ids = BTreeSet::new();
        let package_creature_kinds = entities
            .iter()
            .filter_map(|entity| match entity.kind {
                LevelEntityKind::Creature(kind) => Some(kind),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        for objective in &objectives {
            if !objective_ids.insert(objective.id.clone()) {
                return Err(format!(
                    "duplicate semantic level objective {}",
                    objective.id.as_str()
                ));
            }
            if matches!(
                objective.condition,
                ObjectiveCondition::CastleManaPercent(percent) if percent > 100
            ) {
                return Err(format!(
                    "semantic objective {} Castle-mana percentage is outside 0..=100",
                    objective.id.as_str()
                ));
            }
            let target = match &objective.condition {
                ObjectiveCondition::TargetDeath(target)
                | ObjectiveCondition::TargetDeathWithoutFont(target)
                | ObjectiveCondition::CollectedTarget(target)
                | ObjectiveCondition::OwnedTargetAtLocation { target, .. } => Some(target),
                _ => None,
            };
            if let Some(target) = target
                && !entities.iter().any(|entity| entity.id() == target)
            {
                return Err(format!(
                    "semantic objective {} references missing target {}",
                    objective.id.as_str(),
                    target.as_str()
                ));
            }
            if let ObjectiveCondition::NoLivingCreatures(kind) = objective.condition
                && (!supports_complete_creature_lifecycle(kind)
                    || !package_creature_kinds.contains(&kind))
            {
                return Err(format!(
                    "semantic objective {} targets a creature without a package-present complete absence lifecycle",
                    objective.id.as_str()
                ));
            }
            if objective.condition_access == LevelObjectiveConditionAccess::SessionReset {
                match objective.condition {
                    ObjectiveCondition::ReachLocation(_) => {}
                    ObjectiveCondition::CastleManaPercent(percent) if percent <= 100 => {}
                    ObjectiveCondition::NoLivingCreatures(_) => {}
                    _ => {
                        return Err(format!(
                            "semantic objective {} exposes an unsupported writable condition",
                            objective.id.as_str()
                        ));
                    }
                }
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
    pub fn entity(&self, id: &LevelEntityId) -> Option<&LevelEntityDefinition> {
        self.entities.iter().find(|entity| entity.id == *id)
    }

    pub(crate) fn with_moved_entity(
        &self,
        id: &LevelEntityId,
        position: TilePosition,
    ) -> Result<Self, String> {
        if position.x > u16::from(u8::MAX) || position.y > u16::from(u8::MAX) {
            return Err(format!(
                "semantic level entity {} position is outside the 256x256 world",
                id.as_str()
            ));
        }
        let mut candidate = self.clone();
        let entity = candidate
            .entities
            .iter_mut()
            .find(|entity| entity.id == *id)
            .ok_or_else(|| format!("unknown semantic level entity {}", id.as_str()))?;
        if entity.position_access != LevelEntityPositionAccess::SessionReset {
            return Err(format!(
                "semantic level entity {} position has no runtime consumer",
                id.as_str()
            ));
        }
        if entity.position == position {
            return Err(format!(
                "semantic level entity {} already has the requested position",
                id.as_str()
            ));
        }
        entity.position = position;
        Ok(candidate)
    }

    pub(crate) fn with_put_objective(
        &self,
        id: &LevelObjectiveId,
        condition: ObjectiveCondition,
    ) -> Result<Self, String> {
        let allowed_no_living_kinds = self.no_living_creatures_objective_kinds();
        let mut candidate = self.clone();
        let objective = candidate
            .objectives
            .iter_mut()
            .find(|objective| objective.id == *id)
            .ok_or_else(|| format!("unknown semantic level objective {}", id.as_str()))?;
        if objective.condition_access != LevelObjectiveConditionAccess::SessionReset {
            return Err(format!(
                "semantic objective {} condition has no runtime consumer",
                id.as_str()
            ));
        }
        match (objective.default_condition.clone(), condition.clone()) {
            (ObjectiveCondition::ReachLocation(_), ObjectiveCondition::ReachLocation(position)) => {
                if position.x > u16::from(u8::MAX) || position.y > u16::from(u8::MAX) {
                    return Err(format!(
                        "semantic objective {} location is outside the 256x256 world",
                        id.as_str()
                    ));
                }
            }
            (
                ObjectiveCondition::CastleManaPercent(_),
                ObjectiveCondition::CastleManaPercent(percent),
            ) => {
                if percent > 100 {
                    return Err(format!(
                        "semantic objective {} Castle-mana percentage is outside 0..=100",
                        id.as_str()
                    ));
                }
            }
            (
                ObjectiveCondition::NoLivingCreatures(_),
                ObjectiveCondition::NoLivingCreatures(kind),
            ) => {
                if !allowed_no_living_kinds.contains(&kind) {
                    return Err(format!(
                        "semantic objective {} targets a creature outside this level's package-present complete absence lifecycles",
                        id.as_str()
                    ));
                }
            }
            _ => {
                return Err(format!(
                    "semantic objective {} cannot change condition family",
                    id.as_str()
                ));
            }
        }
        if objective.condition == condition {
            return Err(format!(
                "semantic objective {} already has the requested condition",
                id.as_str()
            ));
        }
        objective.condition = condition;
        Ok(candidate)
    }

    pub(crate) fn with_countdown_arming_ticks(
        &self,
        id: &LevelEntityId,
        arming_ticks: u32,
    ) -> Result<Self, String> {
        if !(1..=i32::MAX.cast_unsigned()).contains(&arming_ticks) {
            return Err(format!(
                "semantic absence countdown {} arming delay is outside 1..={}",
                id.as_str(),
                i32::MAX
            ));
        }
        let mut candidate = self.clone();
        let entity = candidate
            .entities
            .iter_mut()
            .find(|entity| entity.id == *id)
            .ok_or_else(|| format!("unknown semantic level entity {}", id.as_str()))?;
        let config = entity.absence_countdown.as_mut().ok_or_else(|| {
            format!(
                "semantic level entity {} is not an absence countdown",
                id.as_str()
            )
        })?;
        if config.arming_ticks_access != LevelEntityParameterAccess::SessionReset {
            return Err(format!(
                "semantic absence countdown {} arming delay has no runtime consumer",
                id.as_str()
            ));
        }
        if config.arming_ticks == arming_ticks {
            return Err(format!(
                "semantic absence countdown {} already has the requested arming delay",
                id.as_str()
            ));
        }
        config.arming_ticks = arming_ticks;
        Ok(candidate)
    }

    pub(crate) fn with_creature_max_life(
        &self,
        id: &LevelEntityId,
        max_life: u32,
    ) -> Result<Self, String> {
        let mut candidate = self.clone();
        let entity = candidate
            .entities
            .iter_mut()
            .find(|entity| entity.id == *id)
            .ok_or_else(|| format!("unknown semantic level entity {}", id.as_str()))?;
        let kind = match entity.kind {
            LevelEntityKind::Creature(kind) => kind,
            _ => {
                return Err(format!(
                    "semantic level entity {} is not a creature",
                    id.as_str()
                ));
            }
        };
        let maximum = creature_max_life_upper_bound(kind).ok_or_else(|| {
            format!(
                "semantic creature {} has no package-bound health consumer",
                id.as_str()
            )
        })?;
        if !(1..=maximum).contains(&max_life) {
            return Err(format!(
                "semantic creature {} maximum life is outside 1..={maximum}",
                id.as_str()
            ));
        }
        let config = entity.creature_health.as_mut().ok_or_else(|| {
            format!(
                "semantic level entity {} has no package-bound health consumer",
                id.as_str()
            )
        })?;
        if config.max_life_access != LevelEntityParameterAccess::SessionReset {
            return Err(format!(
                "semantic creature {} maximum life has no runtime consumer",
                id.as_str()
            ));
        }
        if config.max_life == max_life {
            return Err(format!(
                "semantic creature {} already has the requested maximum life",
                id.as_str()
            ));
        }
        config.max_life = max_life;
        Ok(candidate)
    }

    pub(crate) fn with_experience_scroll_award(
        &self,
        id: &LevelEntityId,
        experience_award: u32,
    ) -> Result<Self, String> {
        if !(1..=i32::MAX.cast_unsigned()).contains(&experience_award) {
            return Err(format!(
                "semantic Experience Scroll {} award is outside 1..={}",
                id.as_str(),
                i32::MAX
            ));
        }
        let mut candidate = self.clone();
        let entity = candidate
            .entities
            .iter_mut()
            .find(|entity| entity.id == *id)
            .ok_or_else(|| format!("unknown semantic level entity {}", id.as_str()))?;
        let config = entity.experience_scroll.as_mut().ok_or_else(|| {
            format!(
                "semantic level entity {} is not an Experience Scroll",
                id.as_str()
            )
        })?;
        if config.experience_award_access != LevelEntityParameterAccess::SessionReset {
            return Err(format!(
                "semantic Experience Scroll {} award has no runtime consumer",
                id.as_str()
            ));
        }
        if config.experience_award == experience_award {
            return Err(format!(
                "semantic Experience Scroll {} already has the requested award",
                id.as_str()
            ));
        }
        config.experience_award = experience_award;
        Ok(candidate)
    }

    pub(crate) fn with_house_life_mana_scalar(
        &self,
        id: &LevelEntityId,
        life_mana_scalar: u16,
    ) -> Result<Self, String> {
        if life_mana_scalar == 0 {
            return Err(format!(
                "semantic House {} life/mana scalar is outside 1..={}",
                id.as_str(),
                u16::MAX
            ));
        }
        let mut candidate = self.clone();
        let entity = candidate
            .entities
            .iter_mut()
            .find(|entity| entity.id == *id)
            .ok_or_else(|| format!("unknown semantic level entity {}", id.as_str()))?;
        let config = entity.house_construction.as_mut().ok_or_else(|| {
            format!(
                "semantic level entity {} is not a construction-bound House",
                id.as_str()
            )
        })?;
        if config.life_mana_scalar_access != LevelEntityParameterAccess::SessionReset {
            return Err(format!(
                "semantic House {} life/mana scalar has no runtime consumer",
                id.as_str()
            ));
        }
        if config.life_mana_scalar == life_mana_scalar {
            return Err(format!(
                "semantic House {} already has the requested life/mana scalar",
                id.as_str()
            ));
        }
        config.life_mana_scalar = life_mana_scalar;
        Ok(candidate)
    }

    pub(crate) fn with_house_construction_visits(
        &self,
        id: &LevelEntityId,
        construction_visits: u16,
    ) -> Result<Self, String> {
        if construction_visits == 0 {
            return Err(format!(
                "semantic House {} construction visits is outside 1..={}",
                id.as_str(),
                u16::MAX
            ));
        }
        let mut candidate = self.clone();
        let entity = candidate
            .entities
            .iter_mut()
            .find(|entity| entity.id == *id)
            .ok_or_else(|| format!("unknown semantic level entity {}", id.as_str()))?;
        let config = entity.house_construction.as_mut().ok_or_else(|| {
            format!(
                "semantic level entity {} is not a construction-bound House",
                id.as_str()
            )
        })?;
        if config.construction_visits_access != LevelEntityParameterAccess::SessionReset {
            return Err(format!(
                "semantic House {} construction visits has no runtime consumer",
                id.as_str()
            ));
        }
        if config.construction_visits == construction_visits {
            return Err(format!(
                "semantic House {} already has the requested construction visits",
                id.as_str()
            ));
        }
        config.construction_visits = construction_visits;
        Ok(candidate)
    }

    pub(crate) fn with_switch_overlap_extent_tiles(
        &self,
        id: &LevelEntityId,
        overlap_extent_tiles: u16,
    ) -> Result<Self, String> {
        if !(1..=127).contains(&overlap_extent_tiles) {
            return Err(format!(
                "semantic switch {} overlap extent is outside 1..=127",
                id.as_str()
            ));
        }
        let mut candidate = self.clone();
        let entity = candidate
            .entities
            .iter_mut()
            .find(|entity| entity.id == *id)
            .ok_or_else(|| format!("unknown semantic level entity {}", id.as_str()))?;
        let config = entity.switch_overlap.as_mut().ok_or_else(|| {
            format!(
                "semantic level entity {} has no switch overlap configuration",
                id.as_str()
            )
        })?;
        if config.overlap_extent_tiles_access != LevelEntityParameterAccess::SessionReset {
            return Err(format!(
                "semantic switch {} overlap extent has no runtime consumer",
                id.as_str()
            ));
        }
        if config.overlap_extent_tiles == overlap_extent_tiles {
            return Err(format!(
                "semantic switch {} already has the requested overlap extent",
                id.as_str()
            ));
        }
        config.overlap_extent_tiles = overlap_extent_tiles;
        Ok(candidate)
    }

    /// Verify that this definition differs from `base` only through semantic
    /// changes explicitly admitted at session reset.
    ///
    /// This is public for engine adapters that materialize a validated
    /// definition in another crate. Authoring callers should use the guarded
    /// transaction API rather than invoking this validator directly.
    #[doc(hidden)]
    pub fn verify_session_reset_variant_of(&self, base: &Self) -> Result<(), String> {
        if self.activation_groups != base.activation_groups
            || self.entities.len() != base.entities.len()
            || self.objectives.len() != base.objectives.len()
        {
            return Err("semantic level variant changes level structure".to_owned());
        }
        for (candidate, original) in self.entities.iter().zip(&base.entities) {
            if candidate.id != original.id
                || candidate.kind != original.kind
                || candidate.default_position != original.default_position
                || candidate.activation_group != original.activation_group
                || candidate.position_access != original.position_access
                || candidate.cave_corridor != original.cave_corridor
                || candidate.effect != original.effect
                || candidate.terrain_disaster != original.terrain_disaster
                || candidate.field != original.field
                || candidate.mushroom != original.mushroom
                || candidate.cave_feature != original.cave_feature
            {
                return Err(format!(
                    "semantic level variant changes entity identity or structure at {}",
                    original.id.as_str()
                ));
            }
            match (candidate.absence_countdown, original.absence_countdown) {
                (Some(candidate_config), Some(original_config)) => {
                    if candidate_config.default_arming_ticks != original_config.default_arming_ticks
                        || candidate_config.arming_ticks_access
                            != original_config.arming_ticks_access
                    {
                        return Err(format!(
                            "semantic level variant changes countdown structure at {}",
                            original.id.as_str()
                        ));
                    }
                    if candidate_config.arming_ticks != original_config.arming_ticks
                        && original_config.arming_ticks_access
                            != LevelEntityParameterAccess::SessionReset
                    {
                        return Err(format!(
                            "semantic level variant changes read-only countdown {}",
                            original.id.as_str()
                        ));
                    }
                    if !(1..=i32::MAX.cast_unsigned()).contains(&candidate_config.arming_ticks) {
                        return Err(format!(
                            "semantic level variant gives countdown {} an invalid arming delay",
                            original.id.as_str()
                        ));
                    }
                }
                (None, None) => {}
                _ => {
                    return Err(format!(
                        "semantic level variant adds or removes countdown configuration at {}",
                        original.id.as_str()
                    ));
                }
            }
            match (candidate.creature_health, original.creature_health) {
                (Some(candidate_config), Some(original_config)) => {
                    if candidate_config.default_max_life != original_config.default_max_life
                        || candidate_config.max_life_access != original_config.max_life_access
                    {
                        return Err(format!(
                            "semantic level variant changes creature-health structure at {}",
                            original.id.as_str()
                        ));
                    }
                    if candidate_config.max_life != original_config.max_life
                        && original_config.max_life_access
                            != LevelEntityParameterAccess::SessionReset
                    {
                        return Err(format!(
                            "semantic level variant changes read-only creature health {}",
                            original.id.as_str()
                        ));
                    }
                    let Some(maximum) = (match original.kind {
                        LevelEntityKind::Creature(kind) => creature_max_life_upper_bound(kind),
                        _ => None,
                    }) else {
                        return Err(format!(
                            "semantic level variant gives non-creature {} health",
                            original.id.as_str()
                        ));
                    };
                    if !(1..=maximum).contains(&candidate_config.max_life) {
                        return Err(format!(
                            "semantic level variant gives creature {} invalid maximum life",
                            original.id.as_str()
                        ));
                    }
                }
                (None, None) => {}
                _ => {
                    return Err(format!(
                        "semantic level variant adds or removes creature-health configuration at {}",
                        original.id.as_str()
                    ));
                }
            }
            match (candidate.experience_scroll, original.experience_scroll) {
                (Some(candidate_config), Some(original_config)) => {
                    if candidate_config.default_experience_award
                        != original_config.default_experience_award
                        || candidate_config.experience_award_access
                            != original_config.experience_award_access
                    {
                        return Err(format!(
                            "semantic level variant changes Experience Scroll reward structure at {}",
                            original.id.as_str()
                        ));
                    }
                    if candidate_config.experience_award != original_config.experience_award
                        && original_config.experience_award_access
                            != LevelEntityParameterAccess::SessionReset
                    {
                        return Err(format!(
                            "semantic level variant changes read-only Experience Scroll reward {}",
                            original.id.as_str()
                        ));
                    }
                    if !(1..=i32::MAX.cast_unsigned()).contains(&candidate_config.experience_award)
                    {
                        return Err(format!(
                            "semantic level variant gives Experience Scroll {} an invalid award",
                            original.id.as_str()
                        ));
                    }
                }
                (None, None) => {}
                _ => {
                    return Err(format!(
                        "semantic level variant adds or removes Experience Scroll reward configuration at {}",
                        original.id.as_str()
                    ));
                }
            }
            match (candidate.house_construction, original.house_construction) {
                (Some(candidate_config), Some(original_config)) => {
                    if candidate_config.default_life_mana_scalar
                        != original_config.default_life_mana_scalar
                        || candidate_config.life_mana_scalar_access
                            != original_config.life_mana_scalar_access
                        || candidate_config.default_construction_visits
                            != original_config.default_construction_visits
                        || candidate_config.construction_visits_access
                            != original_config.construction_visits_access
                    {
                        return Err(format!(
                            "semantic level variant changes House construction structure at {}",
                            original.id.as_str()
                        ));
                    }
                    if candidate_config.life_mana_scalar != original_config.life_mana_scalar
                        && original_config.life_mana_scalar_access
                            != LevelEntityParameterAccess::SessionReset
                    {
                        return Err(format!(
                            "semantic level variant changes read-only House construction parameter {}",
                            original.id.as_str()
                        ));
                    }
                    if candidate_config.life_mana_scalar == 0 {
                        return Err(format!(
                            "semantic level variant gives House {} an invalid life/mana scalar",
                            original.id.as_str()
                        ));
                    }
                    if candidate_config.construction_visits != original_config.construction_visits
                        && original_config.construction_visits_access
                            != LevelEntityParameterAccess::SessionReset
                    {
                        return Err(format!(
                            "semantic level variant changes read-only House construction visits {}",
                            original.id.as_str()
                        ));
                    }
                    if candidate_config.construction_visits == 0 {
                        return Err(format!(
                            "semantic level variant gives House {} invalid construction visits",
                            original.id.as_str()
                        ));
                    }
                }
                (None, None) => {}
                _ => {
                    return Err(format!(
                        "semantic level variant adds or removes House construction configuration at {}",
                        original.id.as_str()
                    ));
                }
            }
            match (candidate.switch_overlap, original.switch_overlap) {
                (Some(candidate_config), Some(original_config)) => {
                    if candidate_config.default_overlap_extent_tiles
                        != original_config.default_overlap_extent_tiles
                        || candidate_config.overlap_extent_tiles_access
                            != original_config.overlap_extent_tiles_access
                    {
                        return Err(format!(
                            "semantic level variant changes switch overlap structure at {}",
                            original.id.as_str()
                        ));
                    }
                    if candidate_config.overlap_extent_tiles != original_config.overlap_extent_tiles
                        && original_config.overlap_extent_tiles_access
                            != LevelEntityParameterAccess::SessionReset
                    {
                        return Err(format!(
                            "semantic level variant changes read-only switch overlap extent {}",
                            original.id.as_str()
                        ));
                    }
                    if !(1..=127).contains(&candidate_config.overlap_extent_tiles) {
                        return Err(format!(
                            "semantic level variant gives switch {} an invalid overlap extent",
                            original.id.as_str()
                        ));
                    }
                }
                (None, None) => {}
                _ => {
                    return Err(format!(
                        "semantic level variant adds or removes switch overlap configuration at {}",
                        original.id.as_str()
                    ));
                }
            }
            if candidate.position != original.position
                && original.position_access != LevelEntityPositionAccess::SessionReset
            {
                return Err(format!(
                    "semantic level variant moves read-only entity {}",
                    original.id.as_str()
                ));
            }
            if candidate.position.x > u16::from(u8::MAX)
                || candidate.position.y > u16::from(u8::MAX)
            {
                return Err(format!(
                    "semantic level variant moves entity {} outside the 256x256 world",
                    original.id.as_str()
                ));
            }
        }
        for (candidate, original) in self.objectives.iter().zip(&base.objectives) {
            if candidate.id != original.id
                || candidate.default_condition != original.default_condition
                || candidate.condition_access != original.condition_access
            {
                return Err(format!(
                    "semantic level variant changes objective identity or structure at {}",
                    original.id.as_str()
                ));
            }
            if candidate.condition != original.condition
                && (original.condition_access != LevelObjectiveConditionAccess::SessionReset
                    || !matches!(
                        (
                            original.default_condition.clone(),
                            candidate.condition.clone()
                        ),
                        (
                            ObjectiveCondition::ReachLocation(_),
                            ObjectiveCondition::ReachLocation(_)
                        ) | (
                            ObjectiveCondition::CastleManaPercent(_),
                            ObjectiveCondition::CastleManaPercent(_)
                        ) | (
                            ObjectiveCondition::NoLivingCreatures(_),
                            ObjectiveCondition::NoLivingCreatures(_)
                        )
                    ))
            {
                return Err(format!(
                    "semantic level variant changes read-only objective {}",
                    original.id.as_str()
                ));
            }
            if let ObjectiveCondition::ReachLocation(position) = candidate.condition
                && (position.x > u16::from(u8::MAX) || position.y > u16::from(u8::MAX))
            {
                return Err(format!(
                    "semantic level variant moves objective {} outside the 256x256 world",
                    original.id.as_str()
                ));
            }
            if let ObjectiveCondition::CastleManaPercent(percent) = candidate.condition
                && percent > 100
            {
                return Err(format!(
                    "semantic level variant gives objective {} a Castle-mana percentage outside 0..=100",
                    original.id.as_str()
                ));
            }
            if let ObjectiveCondition::NoLivingCreatures(kind) = candidate.condition
                && !base.no_living_creatures_objective_kinds().contains(&kind)
            {
                return Err(format!(
                    "semantic level variant gives objective {} a creature outside this level's package-present complete absence lifecycles",
                    original.id.as_str()
                ));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn objectives(&self) -> &[LevelObjectiveDefinition] {
        &self.objectives
    }

    #[must_use]
    pub fn objective(&self, id: &LevelObjectiveId) -> Option<&LevelObjectiveDefinition> {
        self.objectives.iter().find(|objective| objective.id == *id)
    }

    pub fn objective_conditions(&self) -> impl Iterator<Item = ObjectiveCondition> + '_ {
        self.objectives
            .iter()
            .map(|objective| objective.condition.clone())
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
    pub fn no_living_creatures_objective_kinds(&self) -> BTreeSet<CreatureKind> {
        self.creature_kinds()
            .filter(|kind| supports_complete_creature_lifecycle(*kind))
            .collect()
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

#[cfg(test)]
mod tests {
    use super::{ObjectiveCondition, RivalWizardId};

    #[test]
    fn target_objectives_require_a_package_present_target() {
        use super::*;
        use crate::adapter::{entity_definition, objective_definition};
        let target = LevelEntityId::parse("scenery.target.test").unwrap();
        let entity = entity_definition(
            target.clone(),
            LevelEntityKind::Scenery(SceneryKind::Tree),
            TilePosition { x: 10, y: 20 },
            None,
        );
        for condition in [
            ObjectiveCondition::TargetDeath(target.clone()),
            ObjectiveCondition::TargetDeathWithoutFont(target.clone()),
            ObjectiveCondition::CollectedTarget(target.clone()),
            ObjectiveCondition::OwnedTargetAtLocation {
                target: target.clone(),
                location: TilePosition { x: 30, y: 40 },
            },
        ] {
            let objective = objective_definition(
                LevelObjectiveId::parse("objective.target.test").unwrap(),
                condition,
                LevelObjectiveConditionAccess::ReadOnly,
            );
            assert!(
                LevelDefinition::new(vec![entity.clone()], vec![objective.clone()], vec![]).is_ok()
            );
            let error = LevelDefinition::new(vec![], vec![objective], vec![]).unwrap_err();
            assert!(error.contains("references missing target"));
        }
    }

    #[test]
    fn retained_field_and_mushroom_rules_survive_serialization_and_reject_rebinding() {
        use super::*;
        use crate::adapter::{field_entity_definition, mushroom_entity_definition};
        let field = field_entity_definition(
            LevelEntityId::parse("mechanic.transport.test").unwrap(),
            TilePosition { x: 10, y: 20 },
            None,
            LevelEntityPositionAccess::ReadOnly,
            FieldDefinition::Transport {
                destination: TilePosition { x: 30, y: 40 },
                lifetime: 0,
            },
        );
        let mushroom = mushroom_entity_definition(
            LevelEntityId::parse("scenery.mushroom.test").unwrap(),
            TilePosition { x: 12, y: 14 },
            None,
            LevelEntityPositionAccess::ReadOnly,
            MushroomDefinition::default(),
        );
        let base =
            LevelDefinition::new(vec![field.clone(), mushroom.clone()], vec![], vec![]).unwrap();
        let decoded: LevelDefinition =
            serde_json::from_slice(&serde_json::to_vec(&base).unwrap()).unwrap();
        assert_eq!(decoded, base);
        assert!(decoded.verify_session_reset_variant_of(&base).is_ok());
        let mut rebound = decoded.clone();
        rebound.entities[0].field = Some(FieldDefinition::Transport {
            destination: TilePosition { x: 31, y: 40 },
            lifetime: 0,
        });
        assert!(rebound.verify_session_reset_variant_of(&base).is_err());
        let mut rebound = decoded;
        rebound.entities[1].mushroom.as_mut().unwrap().minimum_life += 1;
        assert!(rebound.verify_session_reset_variant_of(&base).is_err());

        let mut invalid = field.clone();
        invalid.kind = LevelEntityKind::Mechanic(MechanicKind::Field {
            kind: FieldKind::Wind,
        });
        assert!(LevelDefinition::new(vec![invalid], vec![], vec![]).is_err());
        let mut invalid = field.clone();
        invalid.field = None;
        assert!(LevelDefinition::new(vec![invalid], vec![], vec![]).is_err());
        let mut invalid = field;
        invalid.kind = LevelEntityKind::Scenery(SceneryKind::Tree);
        assert!(LevelDefinition::new(vec![invalid], vec![], vec![]).is_err());
        let mut invalid = mushroom.clone();
        invalid.mushroom.as_mut().unwrap().life_variation = 0;
        assert!(LevelDefinition::new(vec![invalid], vec![], vec![]).is_err());
        let mut invalid = mushroom;
        invalid.kind = LevelEntityKind::Scenery(SceneryKind::Tree);
        assert!(LevelDefinition::new(vec![invalid], vec![], vec![]).is_err());
    }

    #[test]
    fn goat_absence_objective_requires_a_package_present_root() {
        use super::{
            CreatureKind, LevelDefinition, LevelEntityId, LevelEntityKind,
            LevelObjectiveConditionAccess, LevelObjectiveId, TilePosition,
        };
        use crate::adapter::{entity_definition, objective_definition};
        let goat = entity_definition(
            LevelEntityId::parse("creature.goat.test").unwrap(),
            LevelEntityKind::Creature(CreatureKind::Goat),
            TilePosition { x: 10, y: 11 },
            None,
        );
        let objective = objective_definition(
            LevelObjectiveId::parse("objective.goat.absence").unwrap(),
            ObjectiveCondition::NoLivingCreatures(CreatureKind::Goat),
            LevelObjectiveConditionAccess::SessionReset,
        );
        let level = LevelDefinition::new(vec![goat], vec![objective.clone()], vec![]).unwrap();
        assert!(
            level
                .no_living_creatures_objective_kinds()
                .contains(&CreatureKind::Goat)
        );
        assert!(level.verify_session_reset_variant_of(&level).is_ok());
        assert!(LevelDefinition::new(vec![], vec![objective], vec![]).is_err());
        assert!(!super::supports_complete_creature_lifecycle(
            CreatureKind::Worm
        ));
    }

    #[test]
    fn activation_switch_properties_require_their_live_rule_and_survive_variants() {
        use super::{
            LevelDefinition, LevelEntityId, LevelEntityParameterAccess, LevelEntityPositionAccess,
            SwitchKind, TilePosition,
        };
        use crate::adapter::{absence_countdown_definition, activation_switch_entity_definition};
        for kind in [
            SwitchKind::RepeatingAreaEntryTrigger,
            SwitchKind::RepeatingAreaExitTrigger,
            SwitchKind::SecretEntryTrigger,
            SwitchKind::CreatureAbsenceTrigger,
            SwitchKind::PopulationAbsenceTrigger,
        ] {
            let id = LevelEntityId::parse(&format!("switch.{}.test", kind.as_str())).unwrap();
            let absence = matches!(
                kind,
                SwitchKind::CreatureAbsenceTrigger | SwitchKind::PopulationAbsenceTrigger
            );
            let entity = activation_switch_entity_definition(
                id.clone(),
                kind,
                TilePosition { x: 4, y: 5 },
                None,
                LevelEntityPositionAccess::ReadOnly,
                (!absence).then_some(3),
                absence.then(|| {
                    absence_countdown_definition(16, LevelEntityParameterAccess::SessionReset)
                }),
            );
            let base = LevelDefinition::new(vec![entity.clone()], vec![], vec![]).unwrap();
            let candidate = if absence {
                base.with_countdown_arming_ticks(&id, 7).unwrap()
            } else {
                base.with_switch_overlap_extent_tiles(&id, 7).unwrap()
            };
            assert!(candidate.verify_session_reset_variant_of(&base).is_ok());
            let mut missing = entity.clone();
            missing.absence_countdown = None;
            missing.switch_overlap = None;
            assert!(LevelDefinition::new(vec![missing], vec![], vec![]).is_err());
            let mut wrong_kind = entity;
            wrong_kind.kind = super::LevelEntityKind::Switch(SwitchKind::EmptyTrigger);
            assert!(LevelDefinition::new(vec![wrong_kind], vec![], vec![]).is_err());
        }
    }

    #[test]
    fn cave_corridor_configuration_requires_its_kind_and_is_retained_in_variants() {
        use super::{
            CaveCorridorDefinition, CorridorOrientation, LevelDefinition, LevelEntityDefinition,
            LevelEntityId, LevelEntityKind, LevelEntityPositionAccess, MechanicKind, SceneryKind,
            TilePosition,
        };
        let entity = LevelEntityDefinition::new(
            LevelEntityId::parse("mechanic.cave_corridor.at_182_43").unwrap(),
            LevelEntityKind::Mechanic(MechanicKind::CaveCorridor),
            TilePosition { x: 182, y: 43 },
            None,
            LevelEntityPositionAccess::ReadOnly,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(LevelDefinition::new(vec![entity.clone()], vec![], vec![]).is_err());
        let configured =
            entity.with_cave_corridor(CaveCorridorDefinition::new(CorridorOrientation::AlongX, 2));
        let base = LevelDefinition::new(vec![configured.clone()], vec![], vec![]).unwrap();
        let encoded = serde_json::to_value(&base).unwrap();
        let decoded: LevelDefinition = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, base);
        assert!(decoded.verify_session_reset_variant_of(&base).is_ok());
        let mut changed = configured.clone();
        changed.cave_corridor = Some(CaveCorridorDefinition::new(CorridorOrientation::AlongY, 2));
        let changed = LevelDefinition::new(vec![changed], vec![], vec![]).unwrap();
        assert!(changed.verify_session_reset_variant_of(&base).is_err());
        let mut wrong_kind = configured;
        wrong_kind.kind = LevelEntityKind::Scenery(SceneryKind::Tree);
        assert!(LevelDefinition::new(vec![wrong_kind], vec![], vec![]).is_err());
    }

    #[test]
    fn rival_wizard_objective_uses_a_closed_semantic_target_identity() {
        let target = RivalWizardId::parse("rival_wizard.nyphur").unwrap();
        let condition = ObjectiveCondition::DefeatRivalWizard(target);
        assert_eq!(
            serde_json::to_value(&condition).unwrap(),
            serde_json::json!({"defeat_rival_wizard": "rival_wizard.nyphur"})
        );
        assert_eq!(
            serde_json::from_value::<ObjectiveCondition>(serde_json::json!({
                "defeat_rival_wizard": "rival_wizard.nyphur"
            }))
            .unwrap(),
            condition
        );
        assert!(RivalWizardId::parse("rival_wizard").is_err());
        assert!(RivalWizardId::parse("wizard.nyphur").is_err());
        assert!(RivalWizardId::parse("rival_wizard.source112").is_err());
        assert!(
            serde_json::from_value::<ObjectiveCondition>(serde_json::json!({
                "defeat_rival_wizard": "rival_wizard.source112"
            }))
            .is_err()
        );
    }
}
