//! Immutable rules for scheduled geometry changes. Source selectors belong
//! to the content adapter; gameplay consumes these semantic properties.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainDisasterKind {
    LoweringRing,
}

impl TerrainDisasterKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LoweringRing => "lowering_ring",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerrainDisasterDefinition {
    kind: TerrainDisasterKind,
    max_life: i32,
    lifetime: i32,
    damage: u16,
    half_width: i16,
    half_height: i16,
}

impl TerrainDisasterDefinition {
    pub(crate) const fn new(
        kind: TerrainDisasterKind,
        max_life: i32,
        lifetime: i32,
        damage: u16,
        half_width: i16,
        half_height: i16,
    ) -> Self {
        Self {
            kind,
            max_life,
            lifetime,
            damage,
            half_width,
            half_height,
        }
    }

    #[must_use]
    pub const fn kind(self) -> TerrainDisasterKind {
        self.kind
    }
    #[must_use]
    pub const fn max_life(self) -> i32 {
        self.max_life
    }
    #[must_use]
    pub const fn lifetime(self) -> i32 {
        self.lifetime
    }
    #[must_use]
    pub const fn damage(self) -> u16 {
        self.damage
    }
    #[must_use]
    pub const fn half_width(self) -> i16 {
        self.half_width
    }
    #[must_use]
    pub const fn half_height(self) -> i16 {
        self.half_height
    }
}
