//! Steal Mana's source-ordered mana transfer and tier-2 castle conversion.

use crate::level_definition::{DirectionalSpellTier, STEAL_MANA_TIERS};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManaActor {
    pub entity_index: u16,
    pub active: bool,
    pub class: u8,
    pub life: i32,
    pub mana: i32,
    pub max_mana: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CastleMana {
    pub entity_index: u16,
    pub active: bool,
    pub mana: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManaSphereGrant {
    pub mana: i32,
    /// Raw source `playerEntityIndex_0x94_148` written to each model-39 row.
    pub player_entity_index: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StealManaOutcome {
    pub tier: u8,
    pub experience_awarded: bool,
    pub fixed_transfer: i32,
    /// Tier 2 consumes the hit actor's Castle mana and converts it into spheres.
    pub target_castle_mana_after: Option<i32>,
    pub mana_spheres: Vec<ManaSphereGrant>,
}

/// Consume one admitted family-13 actor hit. `thief` is the source-id owner in
/// the per-target lane; `victim` is the actor whose callback consumes it.
/// Returns `None` for an invalid tier, actor, or self-hit without changing
/// either actor or castle.
pub fn apply_steal_mana(
    tier: u8,
    thief: &mut ManaActor,
    victim: &mut ManaActor,
    thief_castle: Option<&mut CastleMana>,
    victim_castle: Option<&mut CastleMana>,
) -> Option<StealManaOutcome> {
    let rule = *STEAL_MANA_TIERS.get(usize::from(tier))?;
    if !thief.active
        || !victim.active
        || thief.class != 3
        || victim.class != 3
        || victim.life < 0
        || thief.entity_index == victim.entity_index
    {
        return None;
    }

    let mut fixed_transfer = i32::from(rule.power);
    let mut target_castle_mana_after = None;
    let mut mana_spheres = Vec::new();

    if (1..=2).contains(&rule.product)
        && let (Some(thief_castle), Some(mut victim_castle)) = (thief_castle, victim_castle)
        && thief_castle.active
        && victim_castle.active
        && victim_castle.mana > 0
    {
        // sub_61050 drains the hit actor's Castle, then emits model-39 spheres
        // around the casting actor's Castle. The raw source player relation is
        // set only for product 2; current tier 2 has product 1 and remains 0.
        let sphere_total = (i64::from(victim_castle.mana) * i64::from(rule.power) / 100) as i32;
        victim_castle.mana = victim_castle.mana.wrapping_sub(sphere_total);
        target_castle_mana_after = Some(victim_castle.mana);
        fixed_transfer = 0;

        let mut remainder = sphere_total;
        while remainder > 0 {
            let mana = remainder.min(500);
            mana_spheres.push(ManaSphereGrant {
                mana,
                player_entity_index: if rule.product == 2 {
                    thief.entity_index
                } else {
                    0
                },
            });
            remainder -= mana;
        }
    }

    if fixed_transfer != 0 {
        thief.mana = thief.mana.wrapping_add(fixed_transfer);
        victim.mana = victim.mana.wrapping_sub(fixed_transfer);
    }
    thief.mana = thief.mana.clamp(0, thief.max_mana.max(0));
    victim.mana = victim.mana.clamp(0, victim.max_mana.max(0));

    Some(StealManaOutcome {
        tier,
        experience_awarded: true,
        fixed_transfer,
        target_castle_mana_after,
        mana_spheres,
    })
}

#[must_use]
pub fn tier_rule(tier: u8) -> Option<DirectionalSpellTier> {
    let Some(rule) = STEAL_MANA_TIERS.get(tier as usize) else {
        return None;
    };
    Some(*rule)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(entity_index: u16, mana: i32) -> ManaActor {
        ManaActor {
            entity_index,
            active: true,
            class: 3,
            life: 100,
            mana,
            max_mana: 10_000,
        }
    }

    #[test]
    fn tier_zero_hit_transfers_to_thief_and_clamps_both_real_actors() {
        let (mut thief, mut victim) = (actor(111, 9_900), actor(207, 1_000));
        let outcome = apply_steal_mana(0, &mut thief, &mut victim, None, None).unwrap();
        assert_eq!(outcome.fixed_transfer, 2_000);
        assert_eq!((thief.mana, victim.mana), (10_000, 0));
        assert!(outcome.experience_awarded);
    }

    #[test]
    fn tier_one_hit_transfers_its_typed_power_from_victim_to_thief() {
        let (mut thief, mut victim) = (actor(111, 1_000), actor(207, 6_000));
        let outcome = apply_steal_mana(1, &mut thief, &mut victim, None, None).unwrap();
        assert_eq!(outcome.fixed_transfer, 4_000);
        assert_eq!((thief.mana, victim.mana), (5_000, 2_000));
    }

    #[test]
    fn tier_two_drains_target_castle_and_mints_bounded_spheres_without_actor_transfer() {
        let (mut thief, mut victim) = (actor(111, 4_000), actor(207, 6_000));
        let (mut thief_castle, mut victim_castle) = (
            CastleMana {
                entity_index: 301,
                active: true,
                mana: 9_000,
            },
            CastleMana {
                entity_index: 302,
                active: true,
                mana: 12_345,
            },
        );
        let outcome = apply_steal_mana(
            2,
            &mut thief,
            &mut victim,
            Some(&mut thief_castle),
            Some(&mut victim_castle),
        )
        .unwrap();
        assert_eq!(outcome.fixed_transfer, 0);
        assert_eq!(outcome.target_castle_mana_after, Some(11_111));
        assert_eq!(
            outcome
                .mana_spheres
                .iter()
                .map(|sphere| sphere.mana)
                .sum::<i32>(),
            900
        );
        assert!(
            outcome
                .mana_spheres
                .iter()
                .all(|sphere| (1..=500).contains(&sphere.mana))
        );
        assert!(
            outcome
                .mana_spheres
                .iter()
                .all(|sphere| sphere.player_entity_index == 0)
        );
        assert_eq!((thief.mana, victim.mana), (4_000, 6_000));
        assert_eq!(thief_castle.mana, 9_000);
    }

    #[test]
    fn tier_two_with_an_invalid_castle_falls_back_to_its_typed_actor_transfer() {
        let (mut thief, mut victim) = (actor(111, 1_000), actor(207, 100));
        let mut thief_castle = CastleMana {
            entity_index: 301,
            active: true,
            mana: 9_000,
        };
        let mut victim_castle = CastleMana {
            entity_index: 302,
            active: false,
            mana: 12_345,
        };
        let outcome = apply_steal_mana(
            2,
            &mut thief,
            &mut victim,
            Some(&mut thief_castle),
            Some(&mut victim_castle),
        )
        .unwrap();
        assert_eq!(outcome.fixed_transfer, 10);
        assert_eq!(outcome.target_castle_mana_after, None);
        assert!(outcome.mana_spheres.is_empty());
        assert_eq!((thief.mana, victim.mana), (1_010, 90));
        assert_eq!(thief_castle.mana, 9_000);
        assert_eq!(victim_castle.mana, 12_345);
    }

    #[test]
    fn invalid_target_and_out_of_range_tier_do_not_transfer_mana() {
        let (mut thief, mut victim) = (actor(111, 1_000), actor(207, 2_000));
        victim.class = 5;
        assert!(apply_steal_mana(0, &mut thief, &mut victim, None, None).is_none());
        assert!(apply_steal_mana(3, &mut thief, &mut victim, None, None).is_none());
        assert_eq!((thief.mana, victim.mana), (1_000, 2_000));
    }
}
