//! Duel's per-target acceptance and flight tether rule.

use crate::Position;
use crate::level_definition::{DUEL_TIERS, DirectionalSpellTier};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DuelActor {
    pub entity_index: u16,
    pub active: bool,
    pub class: u8,
    pub life: i32,
    pub position: Position,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DuelTetherRule {
    pub range: u16,
    pub drains_mana: bool,
    pub drains_life: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DuelTetherBinding {
    pub caster_entity_index: u16,
    pub target_entity_index: u16,
    pub holder_entity_index: u16,
    pub tier: u8,
    pub prior_distance: i32,
}

#[must_use]
pub fn tether_rule(tier: u8) -> Option<DuelTetherRule> {
    let Some(row) = DUEL_TIERS.get(tier as usize) else {
        return None;
    };
    Some(DuelTetherRule {
        range: row.power,
        drains_mana: row.product >= 1,
        drains_life: row.product == 2,
    })
}

/// Reconstruct the `5EFA0` link written onto the lane's caster before its
/// ordinary `5DE30` tether consumer runs. The six-byte lane repair itself is
/// deliberately not described as pointer-equivalent to pinned `sub_10C80`.
#[must_use]
pub fn bind_tether(
    tier_payload: i32,
    caster: DuelActor,
    target: DuelActor,
    holder_entity_index: u16,
) -> Option<DuelTetherBinding> {
    let tier = u8::try_from(tier_payload).ok()?;
    tether_rule(tier)?;
    if !caster.active
        || !target.active
        || caster.class != 3
        || target.class != 3
        || target.life < 0
        || caster.entity_index == target.entity_index
    {
        return None;
    }
    Some(DuelTetherBinding {
        caster_entity_index: caster.entity_index,
        target_entity_index: target.entity_index,
        holder_entity_index,
        tier,
        prior_distance: i32::try_from(crate::distance_3d_wrapped(caster.position, target.position))
            .unwrap_or(i32::MAX)
            .clamp(1_024, 3_072),
    })
}

#[must_use]
pub fn tier_rule(tier: u8) -> Option<DirectionalSpellTier> {
    let Some(rule) = DUEL_TIERS.get(tier as usize) else {
        return None;
    };
    Some(*rule)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(entity_index: u16, x: u16) -> DuelActor {
        DuelActor {
            entity_index,
            active: true,
            class: 3,
            life: 100,
            position: Position { x, y: 0, z: 0 },
        }
    }

    #[test]
    fn tiers_select_the_source_range_and_ordered_drain_effects() {
        assert_eq!(
            tether_rule(0),
            Some(DuelTetherRule {
                range: 5_170,
                drains_mana: false,
                drains_life: false
            })
        );
        assert_eq!(
            tether_rule(1),
            Some(DuelTetherRule {
                range: 7_720,
                drains_mana: true,
                drains_life: false
            })
        );
        assert_eq!(
            tether_rule(2),
            Some(DuelTetherRule {
                range: 7_720,
                drains_mana: true,
                drains_life: true
            })
        );
    }

    #[test]
    fn malformed_payload_or_invalid_actor_never_installs_a_tether() {
        let caster = actor(111, 1_000);
        let target = actor(207, 2_000);
        assert!(bind_tether(42, caster, target, 55).is_none());
        let mut non_actor = target;
        non_actor.class = 10;
        assert!(bind_tether(1, caster, non_actor, 55).is_none());
        assert!(bind_tether(1, caster, caster, 55).is_none());
    }

    #[test]
    fn valid_tier_binds_source_distance_and_direction() {
        let binding = bind_tether(2, actor(111, 1_000), actor(207, 4_000), 13).unwrap();
        assert_eq!(
            binding,
            DuelTetherBinding {
                caster_entity_index: 111,
                target_entity_index: 207,
                holder_entity_index: 13,
                tier: 2,
                prior_distance: 3_000,
            }
        );
    }
}
