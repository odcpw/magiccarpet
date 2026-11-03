//! Morph's controlled creature selections. Source timing and payment remain
//! in the ordinary shared spell profiles.

use super::CreatureKind;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MorphTierProperties {
    pub day_product: CreatureKind,
    pub night_cave_product: CreatureKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MorphProperties {
    pub tiers: [MorphTierProperties; 3],
}

impl MorphProperties {
    pub fn validate(self) -> Result<(), &'static str> {
        for tier in self.tiers {
            if tier.night_cave_product == CreatureKind::Bee {
                return Err("controlled Bee construction requires a Day map");
            }
            for product in [tier.day_product, tier.night_cave_product] {
                if !matches!(
                    product,
                    CreatureKind::Bee
                        | CreatureKind::Firefly
                        | CreatureKind::Cymmerian
                        | CreatureKind::Wyvern
                ) {
                    return Err("Morph product has no controlled creature constructor");
                }
            }
        }
        Ok(())
    }
}

/// Owner SPELLS.DAT family4, including LevelInit's Day-map Bee override.
pub const MORPH_PROPERTIES: MorphProperties = MorphProperties {
    tiers: [
        MorphTierProperties {
            day_product: CreatureKind::Bee,
            night_cave_product: CreatureKind::Firefly,
        },
        MorphTierProperties {
            day_product: CreatureKind::Cymmerian,
            night_cave_product: CreatureKind::Cymmerian,
        },
        MorphTierProperties {
            day_product: CreatureKind::Wyvern,
            night_cave_product: CreatureKind::Wyvern,
        },
    ],
};
