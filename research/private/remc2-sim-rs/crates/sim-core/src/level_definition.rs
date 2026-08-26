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
            Self::Villager => "villager",
            Self::Trader => "trader",
            Self::Firefly => "firefly",
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
}

impl LevelDefinition {
    pub(crate) fn new(entities: Vec<LevelEntityDefinition>) -> Result<Self, String> {
        let mut ids = BTreeSet::new();
        for entity in &entities {
            if !ids.insert(entity.id.clone()) {
                return Err(format!(
                    "duplicate semantic level entity {}",
                    entity.id.as_str()
                ));
            }
        }
        Ok(Self { entities })
    }

    #[must_use]
    pub fn entities(&self) -> &[LevelEntityDefinition] {
        &self.entities
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
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
    }
}
