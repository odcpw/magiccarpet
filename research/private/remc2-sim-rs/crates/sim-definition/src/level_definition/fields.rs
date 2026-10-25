//! Immutable construction and player-status properties consumed by fields.

use super::TilePosition;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Wind,
    Transport,
    Slowing,
    Web,
}

impl FieldKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Wind => "wind",
            Self::Transport => "transport",
            Self::Slowing => "slowing",
            Self::Web => "web",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FieldDefinition {
    Wind {
        lifetime: i16,
        damage: u16,
    },
    Transport {
        destination: TilePosition,
        lifetime: i32,
    },
    Slowing {
        max_stages: u8,
        interval: i8,
        first_boost: i16,
    },
    Web {
        duration: i8,
        damage: u16,
        first_boost: i16,
    },
}

impl FieldDefinition {
    #[must_use]
    pub const fn kind(self) -> FieldKind {
        match self {
            Self::Wind { .. } => FieldKind::Wind,
            Self::Transport { .. } => FieldKind::Transport,
            Self::Slowing { .. } => FieldKind::Slowing,
            Self::Web { .. } => FieldKind::Web,
        }
    }

    pub(super) fn validate(self) -> Result<(), String> {
        match self {
            Self::Slowing {
                max_stages,
                interval,
                ..
            } if max_stages == 0 || max_stages > 3 || interval <= 0 => Err(
                "slowing needs one to three stages and a positive signed-byte interval".to_owned(),
            ),
            Self::Web { duration, .. } if duration <= 0 => {
                Err("web needs a positive signed-byte duration".to_owned())
            }
            _ => Ok(()),
        }
    }
}
