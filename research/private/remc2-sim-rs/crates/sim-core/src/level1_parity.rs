//! Machine-checkable Level 1 reachability and parity ledger.
//!
//! A row being decoded is not proof that its constructor, source action,
//! interactions, and Unity presentation are faithful.  The authored ledger
//! therefore covers all 55 reachable level rows separately from the system
//! ledger that records cross-row and human-input gaps.

use crate::level1_mission::AuthoredFeature;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParityDisposition {
    Exact,
    Partial,
    Missing,
    Wrong,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParityPriority {
    P0,
    P1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level1AuthoredParity {
    pub authored_index: u16,
    pub kind: u16,
    pub subtype: i16,
    pub activation_group: i16,
    pub family: &'static str,
    pub disposition: ParityDisposition,
    pub verification: &'static str,
}

macro_rules! row {
    ($index:literal, $kind:literal, $subtype:literal, $group:literal, $family:literal, $test:literal) => {
        Level1AuthoredParity {
            authored_index: $index,
            kind: $kind,
            subtype: $subtype,
            activation_group: $group,
            family: $family,
            disposition: ParityDisposition::Exact,
            verification: $test,
        }
    };
}

/// Every authored row reachable through Level 1's activation-group graph.
///
/// These dispositions cover row construction and the reachable row-local
/// action. Cross-family behavior (for example source-map targeting order) is
/// intentionally tracked in [`LEVEL1_SYSTEM_PARITY`].
pub const LEVEL1_AUTHORED_PARITY: [Level1AuthoredParity; 55] = [
    row!(
        21,
        14,
        5,
        4,
        "experience-scroll",
        "experience_scroll_awards_every_enabled_level_one_spell_and_recomputes_tiers"
    ),
    row!(
        31,
        15,
        2,
        1,
        "castle-spell-jar",
        "spatial_and_objective_switch_chain_activates_jar_then_archers"
    ),
    row!(
        99,
        11,
        32,
        1,
        "objective-switch",
        "spatial_and_objective_switch_chain_activates_jar_then_archers"
    ),
    row!(
        100,
        15,
        3,
        11,
        "speed-spell-jar",
        "speed_release_is_model17_timed_and_independent_of_archer_survival"
    ),
    row!(
        101,
        5,
        4,
        3,
        "archer",
        "archer_constructor_cadence_acquires_and_spawns_source_shaped_arrow"
    ),
    row!(
        102,
        5,
        4,
        3,
        "archer",
        "archer_constructor_cadence_acquires_and_spawns_source_shaped_arrow"
    ),
    row!(
        103,
        5,
        4,
        3,
        "archer",
        "archer_constructor_cadence_acquires_and_spawns_source_shaped_arrow"
    ),
    row!(
        104,
        11,
        0,
        1,
        "objective-switch",
        "spatial_and_objective_switch_chain_activates_jar_then_archers"
    ),
    row!(
        105,
        11,
        32,
        2,
        "objective-switch",
        "spatial_and_objective_switch_chain_activates_jar_then_archers"
    ),
    row!(
        106,
        5,
        4,
        3,
        "archer",
        "archer_constructor_cadence_acquires_and_spawns_source_shaped_arrow"
    ),
    row!(
        108,
        5,
        19,
        6,
        "firefly",
        "fireflies_bind_to_level1_stage_row_and_source_constructor_values"
    ),
    row!(
        109,
        11,
        17,
        3,
        "timed-switch",
        "speed_release_is_model17_timed_and_independent_of_archer_survival"
    ),
    row!(
        132,
        14,
        3,
        4,
        "exit-target",
        "complete_level1_route_reaches_every_objective_and_exit_result_10"
    ),
    row!(
        134,
        5,
        19,
        6,
        "firefly",
        "fireflies_bind_to_level1_stage_row_and_source_constructor_values"
    ),
    row!(
        135,
        5,
        19,
        6,
        "firefly",
        "fireflies_bind_to_level1_stage_row_and_source_constructor_values"
    ),
    row!(
        137,
        11,
        12,
        4,
        "exit-switch",
        "group4_exit_switch_runs_source_timers_and_writes_result_10"
    ),
    row!(
        138,
        10,
        59,
        4,
        "checkpoint-smoke-emitter",
        "checkpoint_emitters_and_model_thirteen_smoke_follow_source_rng_and_lifecycle"
    ),
    row!(
        139,
        10,
        59,
        4,
        "checkpoint-smoke-emitter",
        "checkpoint_emitters_and_model_thirteen_smoke_follow_source_rng_and_lifecycle"
    ),
    row!(
        140,
        14,
        5,
        4,
        "experience-scroll",
        "experience_scroll_awards_every_enabled_level_one_spell_and_recomputes_tiers"
    ),
    row!(
        141,
        14,
        5,
        4,
        "experience-scroll",
        "experience_scroll_awards_every_enabled_level_one_spell_and_recomputes_tiers"
    ),
    row!(
        153,
        11,
        32,
        3,
        "objective-switch",
        "archer_absence_drives_group5_immediately_and_speed_jar_after_source_delay"
    ),
    row!(
        154,
        2,
        1,
        6,
        "stone",
        "group_six_stone_uses_the_source_constructor_and_ground_clamp"
    ),
    row!(
        155,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        156,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        157,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        158,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        159,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        160,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        161,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        162,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        163,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        164,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        168,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        169,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        170,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        171,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        172,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        173,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        174,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        175,
        2,
        0,
        1,
        "tree",
        "group_one_trees_preserve_constructor_rng_jitter_life_and_render_variant"
    ),
    row!(
        189,
        5,
        19,
        6,
        "firefly",
        "fireflies_bind_to_level1_stage_row_and_source_constructor_values"
    ),
    row!(
        190,
        11,
        32,
        5,
        "objective-switch",
        "complete_level1_route_reaches_every_objective_and_exit_result_10"
    ),
    row!(
        193,
        5,
        19,
        6,
        "firefly",
        "fireflies_bind_to_level1_stage_row_and_source_constructor_values"
    ),
    row!(
        231,
        11,
        1,
        2,
        "spatial-switch",
        "spatial_and_objective_switch_chain_activates_jar_then_archers"
    ),
    row!(
        232,
        11,
        0,
        8,
        "objective-switch",
        "complete_level1_route_reaches_every_objective_and_exit_result_10"
    ),
    row!(
        233,
        14,
        5,
        4,
        "experience-scroll",
        "experience_scroll_awards_every_enabled_level_one_spell_and_recomputes_tiers"
    ),
    row!(
        253,
        10,
        59,
        12,
        "checkpoint-smoke-emitter",
        "checkpoint_emitters_and_model_thirteen_smoke_follow_source_rng_and_lifecycle"
    ),
    row!(
        304,
        14,
        5,
        4,
        "experience-scroll",
        "experience_scroll_awards_every_enabled_level_one_spell_and_recomputes_tiers"
    ),
    row!(
        305,
        11,
        0,
        4,
        "post-level-switch",
        "switch_305_uses_strict_wrapped_xy_cadence_and_releases_group_33_without_sound"
    ),
    row!(
        306,
        5,
        19,
        33,
        "firefly",
        "firefly_stage_arrival_hands_off_to_roam_before_same_turn_dispatch"
    ),
    row!(
        307,
        5,
        3,
        4,
        "multipart-creature",
        "group_four_constructs_scroll_switches_and_complete_multipart_chain_in_source_order"
    ),
    row!(
        308,
        11,
        16,
        4,
        "post-level-switch",
        "switch_308_waits_for_dying_body_then_counts_seventeen_eligible_dispatches"
    ),
    row!(
        309,
        5,
        3,
        34,
        "multipart-creature",
        "group_four_constructs_scroll_switches_and_complete_multipart_chain_in_source_order"
    ),
    row!(
        310,
        5,
        3,
        34,
        "multipart-creature",
        "group_four_constructs_scroll_switches_and_complete_multipart_chain_in_source_order"
    ),
    row!(
        311,
        5,
        3,
        34,
        "multipart-creature",
        "group_four_constructs_scroll_switches_and_complete_multipart_chain_in_source_order"
    ),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level1SystemParity {
    pub id: &'static str,
    pub priority: ParityPriority,
    pub disposition: ParityDisposition,
    pub remc2_evidence: &'static str,
    pub verification_or_gap: &'static str,
}

/// Cross-row systems, including every spell available in Level 1.
///
/// This is the authoritative readiness surface. The older eight-bit C ABI
/// mask covers only its historical subset and must not be read as “Level 1
/// parity complete” when this ledger still contains an incomplete P0 row.
pub const LEVEL1_SYSTEM_PARITY: &[Level1SystemParity] = &[
    Level1SystemParity {
        id: "spell.fireball",
        priority: ParityPriority::P0,
        disposition: ParityDisposition::Partial,
        remc2_evidence: "CastFireball_65C20; sub_67CB0; sub_68490; sub_102E0",
        verification_or_gap: "launch/homing/impact/damage/XP are tested; source-list acquisition ordering remains",
    },
    Level1SystemParity {
        id: "spell.possession",
        priority: ParityPriority::P0,
        disposition: ParityDisposition::Partial,
        remc2_evidence: "CastPosses_65F60; PossesHitMana_320E0; AddHouse0A_2D_38330",
        verification_or_gap: "terrain-following impact and ownership mailboxes are tested; source-list acquisition ordering remains",
    },
    Level1SystemParity {
        id: "spell.castle",
        priority: ParityPriority::P1,
        disposition: ParityDisposition::Partial,
        remc2_evidence: "CastCastle_663E0; sub_66D00; BuildCastle_2B710",
        verification_or_gap: "placement/construction/terrain/root lifecycle are tested; exact hand-specific launch offset remains",
    },
    Level1SystemParity {
        id: "spell.speed",
        priority: ParityPriority::P1,
        disposition: ParityDisposition::Partial,
        remc2_evidence: "SetSpell_6DFF0; GetScroll_69DB0; class-10/model-2 action 2",
        verification_or_gap: "tier-zero countdown/speed/trail/cancel are tested; higher owned tiers remain",
    },
    Level1SystemParity {
        id: "input.live-look-aim",
        priority: ParityPriority::P1,
        disposition: ParityDisposition::Exact,
        remc2_evidence: "ComputeMousePlayerMovement_17060; ReadGameUserInputs_89D10; sub_693F0/sub_69640",
        verification_or_gap: "ordinary mouse aim is player yaw/pitch; separate zero look tail matches the non-headset desktop path",
    },
    Level1SystemParity {
        id: "targeting.source-map-order",
        priority: ParityPriority::P0,
        disposition: ParityDisposition::Partial,
        remc2_evidence: "sub_67CB0; sub_68490; sub_10450/sub_104A0 map-chain traversal",
        verification_or_gap: "collision now uses source map chains; per-model acquisition-list ordering still needs qualification",
    },
    Level1SystemParity {
        id: "input.spell-hand-and-tier-menu",
        priority: ParityPriority::P0,
        disposition: ParityDisposition::Partial,
        remc2_evidence: "PlayerInput two-stage family/tier menu; opcodes 31/32",
        verification_or_gap: "independent backend hands work; Unity human family/tier selection is not qualified",
    },
    Level1SystemParity {
        id: "entity.villagers",
        priority: ParityPriority::P0,
        disposition: ParityDisposition::Partial,
        remc2_evidence: "class-5 actions 104..111",
        verification_or_gap: "live movement/damage exists; several actions and presentation transitions remain approximate",
    },
    Level1SystemParity {
        id: "entity.houses",
        priority: ParityPriority::P0,
        disposition: ParityDisposition::Partial,
        remc2_evidence: "AddHouse0A_2D_38330 action 45 and death action 53",
        verification_or_gap: "possession ownership marker exists; damage/death/villager and mana lifecycle remain",
    },
    Level1SystemParity {
        id: "entity.dolmen-recharge",
        priority: ParityPriority::P0,
        disposition: ParityDisposition::Exact,
        remc2_evidence: "AddDolmen_4BBD0; CheckPlayerInDolmen_596F0",
        verification_or_gap: "strict overlap and one-turn recharge mailbox are covered by Rust and C-ABI tests",
    },
    Level1SystemParity {
        id: "entity.experience-scrolls",
        priority: ParityPriority::P0,
        disposition: ParityDisposition::Exact,
        remc2_evidence: "sub_51610; UpdateScroll_59C80; GetScroll_69DB0",
        verification_or_gap: "all five reachable authored rows and owned SPELLS.DAT tier thresholds are tested",
    },
    Level1SystemParity {
        id: "effect.checkpoint-smoke",
        priority: ParityPriority::P0,
        disposition: ParityDisposition::Exact,
        remc2_evidence: "ArriveCheckpoint_4EB50; AddParticleSmoke0A_3D_32420; sub_32160",
        verification_or_gap: "all three emitters plus model-13 RNG, motion, lifetime, render rows and Unity bridge are tested",
    },
];

#[must_use]
pub fn level1_p0_ready() -> bool {
    LEVEL1_SYSTEM_PARITY.iter().all(|row| {
        row.priority != ParityPriority::P0 || row.disposition == ParityDisposition::Exact
    })
}

/// Validate that the owned Level 1 feature table still agrees with the
/// complete reachability ledger.
pub fn validate_authored_features(features: &[AuthoredFeature]) -> Result<(), String> {
    let mut indices = BTreeSet::new();
    for expected in LEVEL1_AUTHORED_PARITY {
        if !indices.insert(expected.authored_index) {
            return Err(format!(
                "duplicate Level 1 parity row {}",
                expected.authored_index
            ));
        }
        let feature = features
            .get(usize::from(expected.authored_index))
            .ok_or_else(|| {
                format!(
                    "Level 1 parity row {} is outside the authored feature table",
                    expected.authored_index
                )
            })?;
        if (feature.kind, feature.subtype, feature.activation_group)
            != (expected.kind, expected.subtype, expected.activation_group)
        {
            return Err(format!(
                "Level 1 parity row {} drifted: expected {}/{}/{}, found {}/{}/{}",
                expected.authored_index,
                expected.kind,
                expected.subtype,
                expected.activation_group,
                feature.kind,
                feature.subtype,
                feature.activation_group,
            ));
        }
        if expected.verification.is_empty() {
            return Err(format!(
                "Level 1 parity row {} lacks verification",
                expected.authored_index
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_registry_is_complete_unique_and_exactly_verified() {
        assert_eq!(LEVEL1_AUTHORED_PARITY.len(), 55);
        assert_eq!(
            LEVEL1_AUTHORED_PARITY
                .iter()
                .map(|row| row.authored_index)
                .collect::<BTreeSet<_>>()
                .len(),
            55
        );
        assert!(LEVEL1_AUTHORED_PARITY.iter().all(|row| {
            row.disposition == ParityDisposition::Exact && !row.verification.is_empty()
        }));
    }

    #[test]
    fn incomplete_p0_systems_keep_level_one_readiness_false() {
        assert!(!level1_p0_ready());
        assert!(LEVEL1_SYSTEM_PARITY.iter().all(|row| {
            !row.id.is_empty()
                && !row.remc2_evidence.is_empty()
                && !row.verification_or_gap.is_empty()
        }));
    }
}
