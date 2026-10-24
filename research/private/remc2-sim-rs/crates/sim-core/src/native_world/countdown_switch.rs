//! World activation rules. The source scheduler supplies frozen membership
//! observations and performs publication synchronously before hide/rearm.
//!
//! This owner has no session access: a visit reads only the predicate inputs
//! below and mutates the controller's authoritative timer. Constructors,
//! scheduler epilogues and following-pass reclamation remain distinct visits.

pub(super) use sim_countdown_switch::{CountdownSwitchStep, step_absence_countdown};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ActivationSettings {
    pub secret_entry_suppressed: bool,
}

/// Authored activation controller. Allocation/RNG and the numeric dispatch
/// identity are retained for the MC2 adapter; the timer and predicate behavior
/// belong to this owner rather than to the shared scheduler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredSwitchProducerSource {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub stage_tag: u16,
    pub word_10: i16,
    pub dword_0x10: i32,
    pub position: crate::Position,
    pub phase: u8,
    pub cooldown: u8,
    pub fired: bool,
    pub drawing_enabled: bool,
    pub active: bool,
}

impl Level2AuthoredSwitchProducerSource {
    pub(crate) fn visit(
        &mut self,
        rule: ActivationRule,
        observations: ActivationObservations,
        arming_ticks: i32,
    ) -> ActivationStep {
        step_activation(
            rule,
            self.phase,
            &mut self.dword_0x10,
            observations,
            arming_ticks,
        )
    }

    /// Call after the synchronous publication returns. Allocation failure does
    /// not postpone hiding or rearming the producer in the source game.
    pub(crate) fn finish_publication(&mut self, step: ActivationStep) {
        if let Some(timer) = step.timer_after_publication {
            self.dword_0x10 = timer;
        }
        if step.hide_after_visit {
            self.fired = true;
            self.drawing_enabled = false;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActivationRule {
    AreaEntry,
    AreaExit,
    RepeatedAreaEntry,
    RepeatedAreaExit,
    LevelComplete,
    Noop,
    Exit,
    SecretEntry,
    ObjectiveComplete,
    AbsenceCountdown,
}

impl ActivationRule {
    pub(crate) const fn from_semantic_kind(kind: crate::level_definition::SwitchKind) -> Self {
        use crate::level_definition::SwitchKind;
        match kind {
            SwitchKind::AreaEntryTrigger => Self::AreaEntry,
            SwitchKind::AreaExitTrigger => Self::AreaExit,
            SwitchKind::RepeatingAreaEntryTrigger => Self::RepeatedAreaEntry,
            SwitchKind::RepeatingAreaExitTrigger => Self::RepeatedAreaExit,
            SwitchKind::LevelExitGate => Self::LevelComplete,
            SwitchKind::ExitSequenceTrigger => Self::Exit,
            SwitchKind::SecretEntryTrigger => Self::SecretEntry,
            SwitchKind::ObjectiveGate => Self::ObjectiveComplete,
            SwitchKind::CreatureAbsenceTrigger | SwitchKind::PopulationAbsenceTrigger => {
                Self::AbsenceCountdown
            }
            SwitchKind::EmptyTrigger => Self::Noop,
        }
    }
    /// The MC2 adapter evaluates the same overlap geometry with this expected
    /// result. An exit trigger means *one matching outside human*, not the
    /// negation of an aggregate inside predicate.
    pub(crate) const fn expected_overlap(self) -> Option<bool> {
        match self {
            Self::AreaEntry | Self::RepeatedAreaEntry | Self::Exit | Self::SecretEntry => {
                Some(true)
            }
            Self::AreaExit | Self::RepeatedAreaExit => Some(false),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ActivationObservations {
    /// A human in the frozen source player chain matches expected_overlap.
    pub matching_human: bool,
    /// A participant row matches expected_overlap. Rearm scans the participant
    /// table (including non-human models), unlike ordinary activation.
    pub matching_participant: bool,
    /// The actual level-completion or objective-completion predicate.
    pub completion: bool,
    /// The rebuilt source family list is nonempty. Do not substitute a
    /// rendered-child count or a mutable mid-pass family scan.
    pub blocking_family_present: bool,
    /// Source setting_38545 bit 8 hides an existing secret controller before
    /// it still performs this visit's normal overlap scan.
    pub suppress_secret: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ActivationBounds {
    pub position: crate::Position,
    pub half_width: i16,
    pub half_depth: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ActivationTarget {
    pub source_slot: u16,
    pub bounds: ActivationBounds,
}

/// Preserve source-list order and test each target independently, especially
/// for inverse overlap. The caller supplies humans for activation and the
/// participant table for repeated-switch rearming.
pub(crate) fn matching_target(
    bounds: ActivationBounds,
    expected_overlap: bool,
    targets: impl IntoIterator<Item = ActivationTarget>,
) -> Option<u16> {
    targets.into_iter().find_map(|target| {
        (crate::level1_mission::source_entities_overlap_xy(
            bounds.position,
            bounds.half_width,
            bounds.half_depth,
            target.bounds.position,
            target.bounds.half_width,
            target.bounds.half_depth,
        ) == expected_overlap)
            .then_some(target.source_slot)
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PublicationLifetime {
    Consume,
    Retain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlayerTransition {
    OrdinaryExit,
    SecretEntry,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ActivationSound {
    #[default]
    None,
    Controller,
    MatchingPlayer,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ActivationStep {
    pub publication: Option<PublicationLifetime>,
    pub transition: Option<PlayerTransition>,
    pub follow_terrain: bool,
    pub hide_before_visit: bool,
    pub hide_after_visit: bool,
    pub sound: ActivationSound,
    /// Write only after the synchronous group scan returns, even if all its
    /// allocation attempts fail. The controller stays live for repeats.
    pub timer_after_publication: Option<i32>,
}

/// Visit one live activation controller. The returned operations retain their
/// source order: pre-visit hide, sound/transition or group scan, post-visit
/// hide/rearm, then the scheduler's common phase epilogue.
pub(crate) fn step_activation(
    rule: ActivationRule,
    phase: u8,
    timer: &mut i32,
    observations: ActivationObservations,
    arming_ticks: i32,
) -> ActivationStep {
    let mut result = ActivationStep::default();
    match rule {
        ActivationRule::Noop => {}
        ActivationRule::AbsenceCountdown => {
            if step_absence_countdown(timer, observations.blocking_family_present, arming_ticks)
                == CountdownSwitchStep::Fire
            {
                result.publication = Some(PublicationLifetime::Consume);
                result.sound = ActivationSound::Controller;
                result.hide_after_visit = true;
            }
        }
        ActivationRule::LevelComplete | ActivationRule::ObjectiveComplete => {
            if observations.completion {
                result.publication = Some(PublicationLifetime::Consume);
                result.hide_after_visit = true;
            }
        }
        ActivationRule::RepeatedAreaEntry | ActivationRule::RepeatedAreaExit if *timer != 0 => {
            // sub_6F8E0 has no eight-phase cadence and scans participant rows,
            // so clearing the area rearms on every visit, including phase 7.
            if !observations.matching_participant {
                *timer = timer.wrapping_sub(1);
            }
        }
        _ => {
            result.hide_before_visit =
                rule == ActivationRule::SecretEntry && observations.suppress_secret;
            if phase & 7 != 0 {
                return result;
            }
            if !observations.matching_human {
                result.follow_terrain = true;
                return result;
            }
            match rule {
                ActivationRule::AreaEntry | ActivationRule::AreaExit => {
                    result.publication = Some(PublicationLifetime::Consume);
                    result.hide_after_visit = true;
                }
                ActivationRule::RepeatedAreaEntry | ActivationRule::RepeatedAreaExit => {
                    result.publication = Some(PublicationLifetime::Retain);
                    result.timer_after_publication = Some(10);
                }
                ActivationRule::Exit | ActivationRule::SecretEntry => {
                    result.transition = Some(if rule == ActivationRule::Exit {
                        PlayerTransition::OrdinaryExit
                    } else {
                        PlayerTransition::SecretEntry
                    });
                    result.sound = ActivationSound::MatchingPlayer;
                    result.hide_after_visit = true;
                }
                _ => unreachable!("non-overlap activation rules were handled above"),
            }
        }
    }
    result
}

/// Numeric identities are contained in this compatibility adapter. Gameplay
/// operates on ActivationRule and predicate observations above.
pub(crate) mod mc2 {
    use super::{ActivationRule, Level2AuthoredSwitchProducerSource};

    pub(crate) const fn authored_controller(
        source_entity_index: u16,
        entity_rng: u16,
        authored_index: u16,
        model: u8,
        stage_tag: u16,
        word_10: i16,
        position: crate::Position,
    ) -> Level2AuthoredSwitchProducerSource {
        Level2AuthoredSwitchProducerSource {
            source_entity_index,
            entity_id: stage_tag,
            entity_rng,
            authored_index,
            source_class: 11,
            model,
            action: model,
            stage_tag,
            word_10,
            dword_0x10: 0,
            position,
            // NewEvent_4A050 seeds byte_0x3E from the allocated entity slot.
            // AddSwitchXX and sub_4A310 preserve that allocation-time phase.
            phase: source_entity_index as u8,
            cooldown: 0,
            fired: false,
            drawing_enabled: true,
            active: true,
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum AbsenceTarget {
        Model(u8),
        SourceManaged,
    }

    pub(crate) const fn rule_for_selector(selector: u8) -> Option<ActivationRule> {
        Some(match selector {
            0 => ActivationRule::AreaEntry,
            1 => ActivationRule::AreaExit,
            2 => ActivationRule::RepeatedAreaEntry,
            3 => ActivationRule::RepeatedAreaExit,
            4 => ActivationRule::LevelComplete,
            5..=11 => ActivationRule::Noop,
            12 => ActivationRule::Exit,
            13..=30 | 33..=44 => ActivationRule::AbsenceCountdown,
            31 => ActivationRule::SecretEntry,
            32 => ActivationRule::ObjectiveComplete,
            _ => return None,
        })
    }

    /// x_DWORD_D4C52ar_strB0 binds selectors 13..29 to models 0..16,
    /// selector30 to sub_6F300(-1), and selectors33..44 to models17..28.
    pub(crate) const fn absence_target(selector: u8) -> Option<AbsenceTarget> {
        match selector {
            13..=29 => Some(AbsenceTarget::Model(selector - 13)),
            30 => Some(AbsenceTarget::SourceManaged),
            33..=44 => Some(AbsenceTarget::Model(selector - 16)),
            _ => None,
        }
    }

    pub(crate) fn blocking_family_present(selector: u8, memberships: &[bool; 29]) -> Option<bool> {
        Some(match absence_target(selector)? {
            AbsenceTarget::Model(model) => memberships[usize::from(model)],
            // Preserve the literal source loop bound <=16, even though its
            // inner condition mentions models up to28. Models12..15 are not
            // blockers here; models17..28 are never visited by that loop.
            AbsenceTarget::SourceManaged => {
                memberships[..12].iter().any(|present| *present) || memberships[16]
            }
        })
    }

    /// Empty source constructors5..11 must neither allocate a slot nor draw
    /// RNG. Their action callbacks also remain empty if such a row is live.
    pub(crate) const fn has_constructor(selector: u8) -> bool {
        matches!(selector, 0..=4 | 12..=44)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActivationObservations, ActivationRule, ActivationSound, PlayerTransition,
        PublicationLifetime, step_activation,
    };

    #[test]
    fn inverse_overlap_selects_the_first_outside_human_even_when_another_is_inside() {
        use super::{ActivationBounds, ActivationTarget, matching_target};
        let bounds = ActivationBounds {
            position: crate::Position {
                x: 2048,
                y: 2048,
                z: 0,
            },
            half_width: 256,
            half_depth: 256,
        };
        let inside = ActivationTarget {
            source_slot: 11,
            bounds,
        };
        let outside = ActivationTarget {
            source_slot: 19,
            bounds: ActivationBounds {
                position: crate::Position {
                    x: 8192,
                    ..bounds.position
                },
                ..bounds
            },
        };
        assert_eq!(matching_target(bounds, true, [inside, outside]), Some(11));
        assert_eq!(matching_target(bounds, false, [inside, outside]), Some(19));
        assert_eq!(matching_target(bounds, false, [inside]), None);
        assert_eq!(matching_target(bounds, false, []), None);
    }

    #[test]
    fn source_adapter_covers_every_selector_without_broadening_the_managed_absence_loop() {
        use super::mc2::{
            AbsenceTarget, absence_target, blocking_family_present, has_constructor,
            rule_for_selector,
        };
        for selector in 0..=44 {
            assert!(rule_for_selector(selector).is_some());
            assert_eq!(has_constructor(selector), !(5..=11).contains(&selector));
        }
        assert_eq!(rule_for_selector(45), None);
        for model in 0..29 {
            let selector = if model <= 16 { model + 13 } else { model + 16 };
            assert_eq!(absence_target(selector), Some(AbsenceTarget::Model(model)));
            let mut memberships = [false; 29];
            memberships[usize::from(model)] = true;
            assert_eq!(blocking_family_present(selector, &memberships), Some(true));
            assert_eq!(
                blocking_family_present(30, &memberships),
                Some(model <= 11 || model == 16)
            );
        }
    }

    #[test]
    fn authored_group_rebuild_preserves_hidden_activation_slots_until_following_boundary() {
        use super::super::{NativeWorld, tests::level_two_fixture_path};
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("actual owner Level 2 export");
        let mut session = inspection.level2_reset_session().expect("source arena");
        let terrain = session.canonical_terrain().clone();
        let position = session.state.position;
        let source = session
            .economy_mut()
            .allocate_authored_switch_producer(41, 1, 2, 1, position, &terrain)
            .expect("ordinary switch allocation");
        let generation = session
            .economy()
            .slot_generation(source.source_entity_index);
        assert!(
            session
                .economy_mut()
                .complete_authored_switch_producer_slot(source.source_entity_index)
        );
        session
            .economy_mut()
            .rebuild_free_arena(&[source.source_entity_index]);
        assert!(
            session
                .economy()
                .map_chains()
                .is_linked(source.source_entity_index)
        );
        assert_eq!(
            session
                .economy()
                .slot_generation(source.source_entity_index),
            generation
        );
        assert!(
            session
                .economy()
                .authored_switch_producers()
                .iter()
                .any(|row| row.source_entity_index == source.source_entity_index && row.fired)
        );
        let second = session
            .economy_mut()
            .allocate_authored_switch_producer(42, 22, 3, 1, position, &terrain)
            .expect("another group may allocate after the hidden switch");
        assert_ne!(second.source_entity_index, source.source_entity_index);
        session
            .economy_mut()
            .reclaim_inactive_slots_at_source_boundary();
        assert!(
            !session
                .economy()
                .map_chains()
                .is_linked(source.source_entity_index)
        );
        let replacement = session
            .economy_mut()
            .allocate_authored_switch_producer(43, 3, 0, 1, position, &terrain)
            .expect("following boundary exposes the reclaimed slot, including group zero");
        assert_eq!(replacement.source_entity_index, source.source_entity_index);
        assert_eq!(replacement.phase, replacement.source_entity_index as u8);
        assert!(
            session
                .economy()
                .slot_generation(replacement.source_entity_index)
                > generation
        );
    }

    #[test]
    fn living_root_blocks_absence_after_partial_constructor_allocation_failure() {
        use super::super::{NativeWorld, tests::level_two_fixture_path};
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("actual owner Level 2 export");
        let mut session = inspection.level2_reset_session().expect("source arena");
        let terrain = session.canonical_terrain().clone();
        let position = session.state.position;
        let slot = *session
            .economy()
            .free_slots
            .last()
            .expect("ordinary free source slot");
        session.economy_mut().free_slots = vec![slot];
        session.economy_mut().replacement_cursor_exclusions = (1..=1000).collect();
        let root = session
            .economy_mut()
            .allocate_authored_worm(84, position, 9000, &terrain)
            .expect("source retains the live root when following body allocations fail");
        assert_eq!(root.source_entity_index, slot);
        assert!(root.active && root.drawing_enabled && root.life >= 0);
        assert!(
            !session
                .economy()
                .authored_class5_model3_children()
                .iter()
                .any(|body| body.parent_entity_index == slot)
        );
        let membership = session.source_arena().rebuilt_class5_model_slots();
        assert!(membership[3].contains(&slot));
        let mut timer = 0;
        let step = step_activation(
            ActivationRule::AbsenceCountdown,
            0,
            &mut timer,
            ActivationObservations {
                blocking_family_present: !membership[3].is_empty(),
                ..Default::default()
            },
            16,
        );
        assert_eq!(step.publication, None);
        assert_eq!(
            timer, 0,
            "a living root prevents even arming the absence timer"
        );
    }

    #[test]
    fn repeat_preserves_rows_and_rearms_only_after_ten_clear_participant_visits() {
        for rule in [
            ActivationRule::RepeatedAreaEntry,
            ActivationRule::RepeatedAreaExit,
        ] {
            let mut timer = 0;
            let step = step_activation(
                rule,
                0,
                &mut timer,
                ActivationObservations {
                    matching_human: true,
                    ..Default::default()
                },
                16,
            );
            assert_eq!(step.publication, Some(PublicationLifetime::Retain));
            assert!(!step.hide_after_visit);
            assert_eq!(timer, 0, "rearm follows the synchronous constructors");
            timer = step.timer_after_publication.expect("repeat rearms");
            let held = step_activation(
                rule,
                7,
                &mut timer,
                ActivationObservations {
                    matching_participant: true,
                    ..Default::default()
                },
                16,
            );
            assert_eq!(held.publication, None);
            assert_eq!(timer, 10);
            for expected in (0..10).rev() {
                let clear = step_activation(rule, 7, &mut timer, Default::default(), 16);
                assert_eq!(timer, expected);
                assert_eq!(clear.publication, None);
            }
            let off_phase = step_activation(
                rule,
                7,
                &mut timer,
                ActivationObservations {
                    matching_human: true,
                    ..Default::default()
                },
                16,
            );
            assert_eq!(off_phase.publication, None);
            assert_eq!(
                step_activation(
                    rule,
                    8,
                    &mut timer,
                    ActivationObservations {
                        matching_human: true,
                        ..Default::default()
                    },
                    16,
                )
                .publication,
                Some(PublicationLifetime::Retain)
            );
        }
    }

    #[test]
    fn absence_freezes_at_one_and_publishes_with_sound_only_after_the_blocker_leaves() {
        let mut timer = 0;
        for _ in 0..16 {
            assert_eq!(
                step_activation(
                    ActivationRule::AbsenceCountdown,
                    3,
                    &mut timer,
                    Default::default(),
                    16,
                )
                .publication,
                None
            );
        }
        assert_eq!(timer, 1);
        assert_eq!(
            step_activation(
                ActivationRule::AbsenceCountdown,
                4,
                &mut timer,
                ActivationObservations {
                    blocking_family_present: true,
                    ..Default::default()
                },
                16,
            )
            .publication,
            None
        );
        assert_eq!(timer, 1);
        let fire = step_activation(
            ActivationRule::AbsenceCountdown,
            5,
            &mut timer,
            Default::default(),
            16,
        );
        assert_eq!(fire.publication, Some(PublicationLifetime::Consume));
        assert_eq!(fire.sound, ActivationSound::Controller);
        assert!(fire.hide_after_visit);
    }

    #[test]
    fn suppressed_secret_hides_before_still_transferring_the_matching_human() {
        let step = step_activation(
            ActivationRule::SecretEntry,
            0,
            &mut 0,
            ActivationObservations {
                matching_human: true,
                suppress_secret: true,
                ..Default::default()
            },
            16,
        );
        assert!(step.hide_before_visit && step.hide_after_visit);
        assert_eq!(step.transition, Some(PlayerTransition::SecretEntry));
        assert_eq!(step.sound, ActivationSound::MatchingPlayer);
        assert_eq!(step.publication, None);
        let off_phase = step_activation(
            ActivationRule::SecretEntry,
            1,
            &mut 0,
            ActivationObservations {
                suppress_secret: true,
                ..Default::default()
            },
            16,
        );
        assert!(off_phase.hide_before_visit);
        assert_eq!(off_phase.transition, None);
        assert!(!off_phase.follow_terrain);
    }

    #[test]
    fn overlap_miss_follows_terrain_only_on_source_cadence_and_empty_callbacks_stay_empty() {
        for rule in [
            ActivationRule::AreaEntry,
            ActivationRule::AreaExit,
            ActivationRule::Exit,
        ] {
            assert!(!step_activation(rule, 1, &mut 0, Default::default(), 16).follow_terrain);
            assert!(step_activation(rule, 8, &mut 0, Default::default(), 16).follow_terrain);
        }
        let empty = step_activation(
            ActivationRule::Noop,
            8,
            &mut 10,
            ActivationObservations {
                matching_human: true,
                completion: true,
                ..Default::default()
            },
            16,
        );
        assert_eq!(empty, Default::default());
    }
}
