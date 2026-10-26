//! Typed source products for the three MC2 Summon Army tiers.
//!
//! The creature family constructors own class/model identities and runtime
//! state. This definition only selects the source creature family and group
//! size for the current environment.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SummonArmyMember {
    Bee,
    Firefly,
    Cymmerian,
    Wyvern,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SummonArmyTier {
    First,
    Second,
    Third,
}

impl SummonArmyTier {
    #[must_use]
    pub const fn from_level_gate(level_gate: i8) -> Option<Self> {
        match level_gate {
            0 => Some(Self::First),
            1 => Some(Self::Second),
            2 => Some(Self::Third),
            _ => None,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::First => 0,
            Self::Second => 1,
            Self::Third => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SummonArmyEnvironment {
    Day,
    Night,
    Cave,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SummonArmyTierProperties {
    pub day_member: SummonArmyMember,
    pub night_or_cave_member: SummonArmyMember,
    pub power: u16,
    pub mana_cost: i32,
    pub castle_mana_required: i32,
    pub cooldown: u16,
    pub single_player_experience: i32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SummonArmySelection {
    pub tier: SummonArmyTier,
    pub member: SummonArmyMember,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SummonArmyProperties {
    pub tiers: [SummonArmyTierProperties; 3],
}

impl SummonArmyProperties {
    /// Return the spell rules for one source tier so the runtime can apply
    /// its SetSpell product without duplicating tier-index mapping.
    #[must_use]
    pub const fn tier_properties(self, tier: SummonArmyTier) -> SummonArmyTierProperties {
        self.tiers[tier.index()]
    }

    /// Select the source family for an ordinary source tier. The flyer
    /// callback derives ring count from the selected family byte.
    #[must_use]
    pub const fn select(
        self,
        tier: SummonArmyTier,
        environment: SummonArmyEnvironment,
    ) -> Option<SummonArmySelection> {
        let properties = self.tier_properties(tier);
        let member = match environment {
            SummonArmyEnvironment::Day => properties.day_member,
            SummonArmyEnvironment::Night | SummonArmyEnvironment::Cave => {
                properties.night_or_cave_member
            }
        };
        Some(SummonArmySelection { tier, member })
    }
}

/// Pinned MC2 SPELLS.DAT/default LevelInit products: Bee/Firefly x8,
/// Cymmerian x4 and Wyvern x2.
pub const MC2_SOURCE_SUMMON_ARMY_PROPERTIES: SummonArmyProperties = SummonArmyProperties {
    tiers: [
        SummonArmyTierProperties {
            day_member: SummonArmyMember::Bee,
            night_or_cave_member: SummonArmyMember::Firefly,
            power: 100,
            mana_cost: 10_000,
            castle_mana_required: 150_000,
            cooldown: 299,
            single_player_experience: 0,
        },
        SummonArmyTierProperties {
            day_member: SummonArmyMember::Cymmerian,
            night_or_cave_member: SummonArmyMember::Cymmerian,
            power: 100,
            mana_cost: 20_000,
            castle_mana_required: 200_000,
            cooldown: 399,
            single_player_experience: 450,
        },
        SummonArmyTierProperties {
            day_member: SummonArmyMember::Wyvern,
            night_or_cave_member: SummonArmyMember::Wyvern,
            power: 100,
            mana_cost: 40_000,
            castle_mana_required: 300_000,
            cooldown: 499,
            single_player_experience: 1_200,
        },
    ],
};

#[cfg(test)]
mod tests {
    use super::{
        MC2_SOURCE_SUMMON_ARMY_PROPERTIES, SummonArmyEnvironment, SummonArmyMember, SummonArmyTier,
    };

    #[test]
    fn source_tiers_select_the_family_for_each_environment() {
        for environment in [
            SummonArmyEnvironment::Day,
            SummonArmyEnvironment::Night,
            SummonArmyEnvironment::Cave,
        ] {
            assert_eq!(
                MC2_SOURCE_SUMMON_ARMY_PROPERTIES.select(SummonArmyTier::Second, environment),
                Some(super::SummonArmySelection {
                    tier: SummonArmyTier::Second,
                    member: SummonArmyMember::Cymmerian,
                })
            );
        }
        assert_eq!(
            MC2_SOURCE_SUMMON_ARMY_PROPERTIES
                .select(SummonArmyTier::First, SummonArmyEnvironment::Day,),
            Some(super::SummonArmySelection {
                tier: SummonArmyTier::First,
                member: SummonArmyMember::Bee,
            })
        );
        assert_eq!(
            MC2_SOURCE_SUMMON_ARMY_PROPERTIES
                .select(SummonArmyTier::First, SummonArmyEnvironment::Cave,),
            Some(super::SummonArmySelection {
                tier: SummonArmyTier::First,
                member: SummonArmyMember::Firefly,
            })
        );
        assert_eq!(
            MC2_SOURCE_SUMMON_ARMY_PROPERTIES
                .select(SummonArmyTier::Third, SummonArmyEnvironment::Night,),
            Some(super::SummonArmySelection {
                tier: SummonArmyTier::Third,
                member: SummonArmyMember::Wyvern,
            })
        );
    }

    #[test]
    fn invalid_level_gate_does_not_produce_a_tier() {
        assert_eq!(SummonArmyTier::from_level_gate(-1), None);
        assert_eq!(SummonArmyTier::from_level_gate(3), None);
    }

    #[test]
    fn source_tiers_retain_single_player_spell_values() {
        let first = MC2_SOURCE_SUMMON_ARMY_PROPERTIES.tier_properties(SummonArmyTier::First);
        let second = MC2_SOURCE_SUMMON_ARMY_PROPERTIES.tier_properties(SummonArmyTier::Second);
        let third = MC2_SOURCE_SUMMON_ARMY_PROPERTIES.tier_properties(SummonArmyTier::Third);
        assert_eq!([first.power, second.power, third.power], [100, 100, 100]);
        assert_eq!(
            [first.mana_cost, second.mana_cost, third.mana_cost],
            [10_000, 20_000, 40_000]
        );
        assert_eq!(
            [
                first.castle_mana_required,
                second.castle_mana_required,
                third.castle_mana_required,
            ],
            [150_000, 200_000, 300_000]
        );
        assert_eq!(
            [first.cooldown, second.cooldown, third.cooldown],
            [299, 399, 499]
        );
        assert_eq!(
            [
                first.single_player_experience,
                second.single_player_experience,
                third.single_player_experience,
            ],
            [0, 450, 1_200]
        );
    }
}
