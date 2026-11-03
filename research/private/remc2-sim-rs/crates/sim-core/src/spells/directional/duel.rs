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

/// Source state needed by `sub_5DE30` for this caster's own tether.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DuelTetherStepInput {
    pub binding: DuelTetherBinding,
    pub caster_position: Position,
    pub predicted_position: Position,
    pub caster_yaw: i16,
    pub caster_pitch: i16,
    pub caster_minimum_speed: i16,
    pub holder_active_value: Option<i16>,
    pub target: Option<crate::TetherTarget>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DuelTetherStepOutcome {
    pub predicted_position: Position,
    pub caster_yaw: i16,
    pub target: Option<crate::TetherTarget>,
    pub clear_binding: bool,
}

/// Apply the ordinary source tether body without borrowing another player's
/// flight state. The caller supplies the generation-checked owner carriers
/// and commits the returned movement and resource writes in source order.
pub(crate) fn step_tether(input: DuelTetherStepInput) -> Result<DuelTetherStepOutcome, String> {
    let mut result = DuelTetherStepOutcome {
        predicted_position: input.predicted_position,
        caster_yaw: input.caster_yaw,
        target: input.target,
        clear_binding: false,
    };
    if input.binding.target_entity_index == 0 || input.target.is_none() {
        return Ok(result);
    }
    let mut target = input.target.expect("target presence checked above");
    if target.entity_index != input.binding.target_entity_index {
        return Err("Duel target does not match the caster's source relation".to_owned());
    }
    let rule = tether_rule(input.binding.tier)
        .ok_or_else(|| format!("Duel tether has invalid tier {}", input.binding.tier))?;
    let distance = crate::distance_3d_wrapped(input.caster_position, target.position);
    if input.binding.holder_entity_index == 0
        || !input.holder_active_value.is_some_and(|value| value != 0)
        || distance >= u32::from(rule.range)
        || target.life < 0
    {
        result.clear_binding = true;
        return Ok(result);
    }

    // Source v12 is 3 * minSpeed / 2. Its signed word is the divisor
    // input, unlike the local-player constant used by the flight adapter.
    let limit = (3 * i32::from(input.caster_minimum_speed) / 2) as i16;
    if limit <= 0 || limit > 1_024 {
        return Err(format!(
            "Duel caster has invalid tether speed limit {limit}"
        ));
    }
    let denominator = 1_024 / i32::from(limit);
    let raw_speed = i32::try_from(distance)
        .unwrap_or(i32::MAX)
        .wrapping_sub(input.binding.prior_distance)
        / denominator;
    let speed = crate::wrapping_i16(i64::from(raw_speed)).clamp(-limit, limit);
    let heading = i16::try_from(crate::heading_for_positions(
        input.caster_position,
        target.position,
    ))
    .expect("source heading fits the 11-bit angle");
    result.caster_yaw = crate::wrap_angle(
        i32::from(input.caster_yaw)
            + i32::from(crate::turn_toward(input.caster_yaw, heading, 0x82)),
    );
    crate::move_entity(
        &mut result.predicted_position,
        heading,
        input.caster_pitch,
        speed,
    );
    if rule.drains_life {
        target.life = target
            .life
            .wrapping_sub(i32::from(target.life_regen).wrapping_add(2));
    }
    if rule.drains_mana {
        target.mana = target
            .mana
            .wrapping_sub(target.mana_regen.wrapping_add(8))
            .max(0);
    }
    result.target = Some(target);
    Ok(result)
}

#[must_use]
pub fn tether_rule(tier: u8) -> Option<DuelTetherRule> {
    let Some(row) = DUEL_TIERS.get(tier as usize) else {
        return None;
    };
    Some(DuelTetherRule {
        range: row.power,
        // sub_5DE30 returns after movement for any product above 2.
        drains_mana: matches!(row.product, 1 | 2),
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

    #[test]
    fn caster_owned_tether_matches_flight_math_and_source_clear_gates() {
        let target = crate::TetherTarget {
            entity_index: 207,
            position: Position {
                x: 4_000,
                y: 2_000,
                z: 200,
            },
            life: 100,
            mana: 50,
            mana_regen: 10,
            life_regen: 3,
        };
        let binding = DuelTetherBinding {
            caster_entity_index: 111,
            target_entity_index: target.entity_index,
            holder_entity_index: 13,
            tier: 2,
            prior_distance: 1_024,
        };
        let input = DuelTetherStepInput {
            binding,
            caster_position: Position {
                x: 1_000,
                y: 1_000,
                z: 100,
            },
            predicted_position: Position {
                x: 1_050,
                y: 1_100,
                z: 100,
            },
            caster_yaw: 300,
            caster_pitch: 32,
            caster_minimum_speed: crate::PLAYER_MIN_SPEED,
            holder_active_value: Some(10),
            target: Some(target),
        };
        let actual = step_tether(input).unwrap();
        let mut flight = crate::FlightState {
            yaw: input.caster_yaw,
            display_pitch: input.caster_pitch,
            tether_target_index: binding.target_entity_index,
            tether_holder_index: binding.holder_entity_index,
            tether_prior_distance: binding.prior_distance,
            tether_tier: u16::from(binding.tier),
            ..Default::default()
        };
        let mut expected_position = input.predicted_position;
        let mut context = crate::TetherContext {
            holder: Some(crate::SpellHolder {
                entity_index: binding.holder_entity_index,
                active_value: 10,
            }),
            target: Some(target),
        };
        crate::apply_tether(
            &mut flight,
            input.caster_position,
            &mut expected_position,
            &mut context,
            crate::TETHER_SPEED_LIMIT,
        );
        assert!(!actual.clear_binding);
        assert_eq!(actual.predicted_position, expected_position);
        assert_eq!(actual.caster_yaw, flight.yaw);
        assert_eq!(actual.target, context.target);
        assert_eq!(actual.target.unwrap().life, 95);
        assert_eq!(actual.target.unwrap().mana, 32);

        let slower = step_tether(DuelTetherStepInput {
            caster_minimum_speed: 64,
            ..input
        })
        .unwrap();
        let mut slower_position = input.predicted_position;
        let heading = crate::heading_for_positions(input.caster_position, target.position) as i16;
        crate::move_entity(&mut slower_position, heading, input.caster_pitch, 96);
        assert_eq!(slower.predicted_position, slower_position);

        for holder in [None, Some(0)] {
            let out = step_tether(DuelTetherStepInput {
                holder_active_value: holder,
                ..input
            })
            .unwrap();
            assert!(out.clear_binding);
            assert_eq!(out.target, Some(target));
            assert_eq!(out.predicted_position, input.predicted_position);
        }
        let dead = crate::TetherTarget { life: -1, ..target };
        assert!(
            step_tether(DuelTetherStepInput {
                target: Some(dead),
                ..input
            })
            .unwrap()
            .clear_binding
        );
        let distant = crate::TetherTarget {
            position: Position {
                x: 20_000,
                ..target.position
            },
            ..target
        };
        assert!(
            step_tether(DuelTetherStepInput {
                target: Some(distant),
                ..input
            })
            .unwrap()
            .clear_binding
        );
        let missing = step_tether(DuelTetherStepInput {
            target: None,
            ..input
        })
        .unwrap();
        assert!(!missing.clear_binding);
        assert_eq!(missing.predicted_position, input.predicted_position);
        let sentinel = step_tether(DuelTetherStepInput {
            binding: DuelTetherBinding {
                target_entity_index: 0,
                ..binding
            },
            ..input
        })
        .unwrap();
        assert!(!sentinel.clear_binding);
        assert_eq!(sentinel.predicted_position, input.predicted_position);
        let wrong = crate::TetherTarget {
            entity_index: 208,
            ..target
        };
        assert!(
            step_tether(DuelTetherStepInput {
                target: Some(wrong),
                ..input
            })
            .is_err()
        );
        assert!(
            step_tether(DuelTetherStepInput {
                caster_minimum_speed: 0,
                ..input
            })
            .is_err()
        );
    }
}
