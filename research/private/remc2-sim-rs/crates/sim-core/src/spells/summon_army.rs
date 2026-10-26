//! Summon Army's source-selected unit group and real construction fan-out.
//!
//! The MC2 adapter selects one of three semantic tiers from the acquired spell
//! holder. The selected group is constructed through the caller's live creature
//! constructors; a failed constructor is reported with any real members already
//! created, never as a successful full summon.

use crate::native_world::MapType;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SummonArmyTier {
    First,
    Second,
    Third,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SummonArmyMember {
    Bee,
    Firefly,
    Cymmerian,
    Wyvern,
}

/// Typed unit and group-size defaults for one Army tier. The MC2 definition
/// binds the source defaults below; authored definitions can replace these
/// values without changing the spell's construction path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SummonArmyTierRule {
    pub day_member: SummonArmyMember,
    pub night_or_cave_member: SummonArmyMember,
    pub quantity: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SummonArmyRules {
    pub first: SummonArmyTierRule,
    pub second: SummonArmyTierRule,
    pub third: SummonArmyTierRule,
}

/// Source SPELLS.DAT/default LevelInit products: Bee/Firefly x8,
/// Cymmerian x4, and Wyvern x2.
pub const MC2_SOURCE_SUMMON_ARMY_RULES: SummonArmyRules = SummonArmyRules {
    first: SummonArmyTierRule {
        day_member: SummonArmyMember::Bee,
        night_or_cave_member: SummonArmyMember::Firefly,
        quantity: 8,
    },
    second: SummonArmyTierRule {
        day_member: SummonArmyMember::Cymmerian,
        night_or_cave_member: SummonArmyMember::Cymmerian,
        quantity: 4,
    },
    third: SummonArmyTierRule {
        day_member: SummonArmyMember::Wyvern,
        night_or_cave_member: SummonArmyMember::Wyvern,
        quantity: 2,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SummonArmyProduct {
    member: SummonArmyMember,
    quantity: u8,
}

impl SummonArmyProduct {
    /// Select a member and group size from typed rules for the current map and
    /// spell tier. A zero-sized tier is invalid and constructs nothing.
    #[must_use]
    pub const fn for_tier(
        rules: SummonArmyRules,
        map_type: MapType,
        tier: SummonArmyTier,
    ) -> Option<Self> {
        let rule = match tier {
            SummonArmyTier::First => rules.first,
            SummonArmyTier::Second => rules.second,
            SummonArmyTier::Third => rules.third,
        };
        if rule.quantity == 0 {
            return None;
        }
        Some(Self {
            member: match map_type {
                MapType::Day => rule.day_member,
                MapType::Night | MapType::Cave => rule.night_or_cave_member,
            },
            quantity: rule.quantity,
        })
    }

    #[must_use]
    pub const fn member(self) -> SummonArmyMember {
        self.member
    }

    #[must_use]
    pub const fn quantity(self) -> u8 {
        self.quantity
    }
}

/// Result of constructing the requested group through real family owners.
/// Partial results carry only handles returned by successful constructors.
#[derive(Clone, Debug, Eq, PartialEq)]
#[must_use]
pub enum SummonArmySpawnOutcome<T> {
    Complete {
        product: SummonArmyProduct,
        members: Vec<T>,
    },
    ConstructionFailed {
        product: SummonArmyProduct,
        members: Vec<T>,
    },
}

/// Construct one source-sized Army group. `construct` is the live family
/// constructor supplied by the MC2 runtime; `None` stops the fan-out and is
/// observable as failure, including when the first member cannot be created.
pub fn construct_group<T>(
    product: SummonArmyProduct,
    mut construct: impl FnMut(SummonArmyMember) -> Option<T>,
) -> SummonArmySpawnOutcome<T> {
    let mut members = Vec::with_capacity(usize::from(product.quantity));
    while members.len() < usize::from(product.quantity) {
        match construct(product.member) {
            Some(member) => members.push(member),
            None => {
                return SummonArmySpawnOutcome::ConstructionFailed { product, members };
            }
        }
    }
    SummonArmySpawnOutcome::Complete { product, members }
}

#[cfg(test)]
mod tests {
    use super::{
        MC2_SOURCE_SUMMON_ARMY_RULES, SummonArmyMember, SummonArmyProduct, SummonArmySpawnOutcome,
        SummonArmyTier, construct_group,
    };
    use crate::native_world::MapType;

    #[test]
    fn source_tiers_select_the_source_member_and_group_size() {
        assert_eq!(
            SummonArmyProduct::for_tier(
                MC2_SOURCE_SUMMON_ARMY_RULES,
                MapType::Day,
                SummonArmyTier::First,
            )
            .unwrap(),
            SummonArmyProduct {
                member: SummonArmyMember::Bee,
                quantity: 8,
            }
        );
        for map_type in [MapType::Night, MapType::Cave] {
            assert_eq!(
                SummonArmyProduct::for_tier(
                    MC2_SOURCE_SUMMON_ARMY_RULES,
                    map_type,
                    SummonArmyTier::First,
                )
                .unwrap(),
                SummonArmyProduct {
                    member: SummonArmyMember::Firefly,
                    quantity: 8,
                }
            );
        }
        assert_eq!(
            SummonArmyProduct::for_tier(
                MC2_SOURCE_SUMMON_ARMY_RULES,
                MapType::Night,
                SummonArmyTier::Second,
            )
            .unwrap(),
            SummonArmyProduct {
                member: SummonArmyMember::Cymmerian,
                quantity: 4,
            }
        );
        assert_eq!(
            SummonArmyProduct::for_tier(
                MC2_SOURCE_SUMMON_ARMY_RULES,
                MapType::Cave,
                SummonArmyTier::Third,
            )
            .unwrap(),
            SummonArmyProduct {
                member: SummonArmyMember::Wyvern,
                quantity: 2,
            }
        );
    }

    #[test]
    fn zero_sized_definition_rejects_the_cast_before_any_construction() {
        let rules = super::SummonArmyRules {
            first: super::SummonArmyTierRule {
                quantity: 0,
                ..MC2_SOURCE_SUMMON_ARMY_RULES.first
            },
            ..MC2_SOURCE_SUMMON_ARMY_RULES
        };
        assert_eq!(
            SummonArmyProduct::for_tier(rules, MapType::Day, SummonArmyTier::First),
            None
        );
    }

    #[test]
    fn unavailable_family_constructor_never_reports_a_complete_army() {
        let product = SummonArmyProduct::for_tier(
            MC2_SOURCE_SUMMON_ARMY_RULES,
            MapType::Night,
            SummonArmyTier::First,
        )
        .unwrap();
        let result = construct_group(product, |_| None::<u16>);
        assert_eq!(
            result,
            SummonArmySpawnOutcome::ConstructionFailed {
                product,
                members: Vec::new(),
            }
        );
    }

    #[test]
    fn partial_allocation_reports_only_constructed_members() {
        let product = SummonArmyProduct::for_tier(
            MC2_SOURCE_SUMMON_ARMY_RULES,
            MapType::Day,
            SummonArmyTier::First,
        )
        .unwrap();
        let mut next = 0_u16;
        let result = construct_group(product, |_| {
            if next == 2 {
                return None;
            }
            next += 1;
            Some(next)
        });
        assert_eq!(
            result,
            SummonArmySpawnOutcome::ConstructionFailed {
                product,
                members: vec![1, 2],
            }
        );
    }
}
