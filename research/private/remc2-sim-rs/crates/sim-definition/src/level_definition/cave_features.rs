//! Immutable cave construction properties, independent of MC2 selectors.

use super::TilePosition;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaveFeatureKind {
    PathNode,
    Passage,
    Room,
    RoundObstruction,
    CeilingPendant,
    FloorMound,
    GroundVent,
}

impl CaveFeatureKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PathNode => "cave_path_node",
            Self::Passage => "cave_passage",
            Self::Room => "cave_room",
            Self::RoundObstruction => "cave_round_obstruction",
            Self::CeilingPendant => "cave_ceiling_pendant",
            Self::FloorMound => "cave_floor_mound",
            Self::GroundVent => "ground_vent",
        }
    }
}

/// Properties consumed by the terrain constructor, rather than an editor-only
/// mirror. A zero height factor retains random source placement. Nonzero
/// factors use the retained 51/256 scaling of the available vertical span.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CaveFeatureDefinition {
    PathNode {
        half_width_tiles: u8,
    },
    Passage {
        start_radius_tiles: u8,
        end_radius_tiles: u8,
        destination: TilePosition,
    },
    Room {
        half_width_tiles: u8,
        half_depth_tiles: u8,
        clearance_steps: u8,
    },
    RoundObstruction {
        radius_tiles: i16,
    },
    CeilingPendant {
        radius_tiles: i16,
        height_factor: i16,
    },
    FloorMound {
        radius_tiles: i16,
        height_factor: i16,
    },
    GroundVent {
        lifetime: i32,
    },
}

impl CaveFeatureDefinition {
    #[must_use]
    pub const fn kind(self) -> CaveFeatureKind {
        match self {
            Self::PathNode { .. } => CaveFeatureKind::PathNode,
            Self::Passage { .. } => CaveFeatureKind::Passage,
            Self::Room { .. } => CaveFeatureKind::Room,
            Self::RoundObstruction { .. } => CaveFeatureKind::RoundObstruction,
            Self::CeilingPendant { .. } => CaveFeatureKind::CeilingPendant,
            Self::FloorMound { .. } => CaveFeatureKind::FloorMound,
            Self::GroundVent { .. } => CaveFeatureKind::GroundVent,
        }
    }

    pub(crate) fn validate(self) -> Result<(), String> {
        match self {
            Self::Passage { destination, .. } if destination.x > 255 || destination.y > 255 => {
                Err("cave passage destination exceeds the terrain tile domain".to_owned())
            }
            Self::PathNode { half_width_tiles } if half_width_tiles > 15 => {
                Err("cave path half width exceeds its retained four-bit range".to_owned())
            }
            Self::Passage {
                start_radius_tiles,
                end_radius_tiles,
                ..
            } if !(2..=17).contains(&start_radius_tiles)
                || !(2..=17).contains(&end_radius_tiles) =>
            {
                Err("cave passage endpoint radii must be in 2..=17 tiles".to_owned())
            }
            _ => Ok(()),
        }
    }
}
