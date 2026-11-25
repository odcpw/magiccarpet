//! Source spell marks and their bounded hand-specific cycle scan.

use crate::{
    PlayerCommand, SOURCE_SPELL_INDEX_MAP,
    game_session::{
        GameCommand, MarkedSpellCycle, SpellAssignmentHand, SpellCycleDirection, SpellMark,
        SpellMarkHand,
    },
    native_world::Level2SharedSpellResetSession,
};

use super::spells::{self, mc2_spell_family, semantic_spell_kind};

const SOURCE_MARKED_SPELL_SLOT_COUNT: usize = 26;

/// Translate semantic marks and cycles into the ordinary source player command.
///
/// Mark/cycle commands are exclusive with semantic select/assign actions. A
/// cycle with no live acquired spell marked for that hand leaves the incoming
/// player command untouched, so movement and other ordinary input still proceed.
pub(super) fn player_command(
    shared: &Level2SharedSpellResetSession,
    command: &GameCommand,
    player_command: PlayerCommand,
    has_current_holder: impl Fn(u8) -> bool,
) -> Result<PlayerCommand, String> {
    let mark = command.mark_spell.as_ref();
    let cycle = command.cycle_marked_spell.as_ref();
    if mark.is_none() && cycle.is_none() {
        return Ok(player_command);
    }
    if mark.is_some() && cycle.is_some() {
        return Err("spell mark and marked-spell cycle cannot share one input".to_owned());
    }
    if command.assign_spell.is_some() || command.select_spell.is_some() {
        return Err("spell mark/cycle conflicts with spell select/assign".to_owned());
    }

    if let Some(mark) = mark {
        return mark_player_command(shared, mark, player_command);
    }

    if let Some(cycle) = cycle {
        return cycle_player_command(shared, cycle, player_command, has_current_holder);
    }
    Ok(player_command)
}

fn mark_player_command(
    shared: &Level2SharedSpellResetSession,
    mark: &SpellMark,
    mut player_command: PlayerCommand,
) -> Result<PlayerCommand, String> {
    let kind = semantic_spell_kind(&mark.spell_id)
        .ok_or_else(|| format!("unknown spell {}", mark.spell_id.as_str()))?;
    let family = mc2_spell_family(kind).ok_or_else(|| "unbound spell family".to_owned())?;
    let family_index = usize::from(family);
    if family_index >= SOURCE_MARKED_SPELL_SLOT_COUNT {
        return Err(format!(
            "spell {} has no source mark slot",
            mark.spell_id.as_str()
        ));
    }
    if spells::acquired_holder(shared, family).is_none() {
        return Err(format!(
            "spell {} has not been acquired",
            mark.spell_id.as_str()
        ));
    }

    player_command.opcode = 38;
    player_command.argument_1 = i8::try_from(family)
        .map_err(|_| "spell source slot exceeds player-command range".to_owned())?;
    player_command.argument_2 = match mark.hand {
        SpellMarkHand::None => 0,
        SpellMarkHand::Left => 1,
        SpellMarkHand::Right => 2,
    };
    Ok(player_command)
}

fn cycle_player_command(
    shared: &Level2SharedSpellResetSession,
    cycle: &MarkedSpellCycle,
    mut player_command: PlayerCommand,
    has_current_holder: impl Fn(u8) -> bool,
) -> Result<PlayerCommand, String> {
    let player = shared.local_player();
    let current_family = match cycle.hand {
        SpellAssignmentHand::Left => player.left_spell_slot_raw,
        SpellAssignmentHand::Right => player.right_spell_slot_raw,
    };
    let marked_hand = match cycle.hand {
        SpellAssignmentHand::Left => 1,
        SpellAssignmentHand::Right => 2,
    };
    let valid_enabled_spell_families =
        derive_live_enabled_spell_families(&player.enabled_spell_holders, has_current_holder);
    let Some((ui_slot, family)) = find_next_marked_source_slot(
        current_family,
        cycle.direction,
        marked_hand,
        &valid_enabled_spell_families,
        &player.spell_hand_eligibility,
    ) else {
        return Ok(player_command);
    };

    let family_index = usize::from(family);
    let Some(selected_tier) = player.family_selected_tiers.get(family_index).copied() else {
        return Ok(player_command);
    };
    player_command.opcode = match cycle.hand {
        SpellAssignmentHand::Left => 31,
        SpellAssignmentHand::Right => 32,
    };
    player_command.argument_1 = i8::try_from(ui_slot)
        .map_err(|_| "spell menu slot exceeds player-command range".to_owned())?;
    player_command.argument_2 = selected_tier.clamp(0, 2);
    Ok(player_command)
}

/// Build the source family's scan mask from current local-owned holder rows.
/// Nonzero relation IDs alone are staleable and do not make a family enabled.
fn derive_live_enabled_spell_families(
    enabled_spell_holders: &[u16; SOURCE_MARKED_SPELL_SLOT_COUNT],
    mut has_live_owned_holder: impl FnMut(u8) -> bool,
) -> [bool; SOURCE_MARKED_SPELL_SLOT_COUNT] {
    std::array::from_fn(|family_index| {
        let Ok(family) = u8::try_from(family_index) else {
            return false;
        };
        enabled_spell_holders
            .get(family_index)
            .is_some_and(|holder| *holder != 0 && has_live_owned_holder(family))
    })
}

fn find_next_marked_source_slot(
    current_family: i16,
    direction: SpellCycleDirection,
    marked_hand: u8,
    valid_enabled_spell_families: &[bool; SOURCE_MARKED_SPELL_SLOT_COUNT],
    spell_hand_eligibility: &[u8; SOURCE_MARKED_SPELL_SLOT_COUNT],
) -> Option<(u8, u8)> {
    let source_slots = SOURCE_SPELL_INDEX_MAP.get(..SOURCE_MARKED_SPELL_SLOT_COUNT)?;
    let start = source_slots
        .iter()
        .position(|&family| i16::from(family) == current_family)
        .unwrap_or(match direction {
            SpellCycleDirection::Next => SOURCE_MARKED_SPELL_SLOT_COUNT - 1,
            SpellCycleDirection::Previous => 0,
        });

    for distance in 1..=SOURCE_MARKED_SPELL_SLOT_COUNT {
        let slot = match direction {
            SpellCycleDirection::Next => (start + distance) % SOURCE_MARKED_SPELL_SLOT_COUNT,
            SpellCycleDirection::Previous => {
                (start + SOURCE_MARKED_SPELL_SLOT_COUNT - distance % SOURCE_MARKED_SPELL_SLOT_COUNT)
                    % SOURCE_MARKED_SPELL_SLOT_COUNT
            }
        };
        let family = source_slots.get(slot).copied()?;
        let family_index = usize::from(family);
        let Some(is_live_acquired) = valid_enabled_spell_families.get(family_index) else {
            continue;
        };
        let Some(hand_mark) = spell_hand_eligibility.get(family_index) else {
            continue;
        };
        if *is_live_acquired && *hand_mark == marked_hand {
            return Some((u8::try_from(slot).ok()?, family));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::{
        Mc2CampaignAdapter, Mc2CampaignBackend, Mc2CampaignInputs, Mc2CampaignReplaySave,
        Mc2RuntimePackageResolver,
    };
    use super::{
        SOURCE_MARKED_SPELL_SLOT_COUNT, derive_live_enabled_spell_families,
        find_next_marked_source_slot,
    };
    use crate::{
        SOURCE_SPELL_INDEX_MAP,
        game_definition::SemanticId,
        game_runtime::GameRuntime,
        game_session::{
            CampaignSnapshot, GameCommand, GameSession, MarkedSpellCycle, RuntimeActorId,
            SpellAssignment, SpellAssignmentHand, SpellCycleDirection, SpellHand, SpellMark,
            SpellMarkHand, SpellObservation,
        },
    };
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn spell_holder_actor_wire_identity_is_opaque_and_optional() {
        use crate::game_session::{RuntimeActorId, SpellAssignment};

        let bound: SpellAssignment = serde_json::from_str(
            r#"{"spell_id":"spell.fireball","tier":0,"hand":"left","holder_actor":"18446744073709551615"}"#,
        )
        .unwrap();
        assert_eq!(bound.holder_actor, Some(RuntimeActorId(u64::MAX)));
        assert_eq!(
            serde_json::to_value(&bound).unwrap()["holder_actor"],
            u64::MAX.to_string()
        );
        let unbound: SpellAssignment =
            serde_json::from_str(r#"{"spell_id":"spell.fireball","tier":0,"hand":"left"}"#)
                .unwrap();
        assert_eq!(unbound.holder_actor, None);
        let unity_empty: SpellAssignment = serde_json::from_str(
            r#"{"spell_id":"spell.fireball","tier":0,"hand":"left","holder_actor":""}"#,
        )
        .unwrap();
        assert_eq!(unity_empty.holder_actor, None);
        for invalid in ["0", "01", "-1", "18446744073709551616"] {
            let value = format!(
                r#"{{"spell_id":"spell.fireball","tier":0,"hand":"left","holder_actor":"{invalid}"}}"#
            );
            assert!(serde_json::from_str::<SpellAssignment>(&value).is_err());
        }
    }

    fn fireball(snapshot: &CampaignSnapshot) -> &SpellObservation {
        snapshot
            .player_spells
            .iter()
            .find(|spell| spell.id.as_str() == "spell.fireball")
            .expect("the campaign spell observation includes Fireball")
    }

    fn source_player_record(session: &GameSession<Mc2CampaignBackend>) -> &[u8] {
        session
            .backend()
            .campaign
            .simulation()
            .shared_spells()
            .local_player()
            .record_bytes()
    }

    fn assert_rejected_mark_is_transactional(
        session: &mut GameSession<Mc2CampaignBackend>,
        spell_id: SemanticId,
    ) {
        let before = session.snapshot();
        let source_record_before = source_player_record(session).to_vec();
        let replay_operation_count = session.backend().replay_operations.len();
        let guard = super::super::tests::guard(session);
        assert!(
            session
                .step(
                    guard,
                    GameCommand {
                        mark_spell: Some(SpellMark {
                            spell_id,
                            hand: SpellMarkHand::Right,
                            holder_actor: None,
                        }),
                        ..GameCommand::default()
                    },
                )
                .is_err(),
            "invalid or unacquired marks must be rejected"
        );
        assert_eq!(session.snapshot(), before);
        assert_eq!(
            source_player_record(session),
            source_record_before.as_slice()
        );
        assert_eq!(
            session.backend().replay_operations.len(),
            replay_operation_count
        );
    }

    #[test]
    fn source_slots_seven_through_nine_select_the_requested_hand_in_both_directions() {
        assert_eq!(SOURCE_SPELL_INDEX_MAP.get(7..10), Some(&[7, 8, 9][..]));
        let mut enabled = [false; SOURCE_MARKED_SPELL_SLOT_COUNT];
        let mut marks = [0; SOURCE_MARKED_SPELL_SLOT_COUNT];
        enabled[7..10].fill(true);
        marks[7] = 1;
        marks[8] = 2;
        marks[9] = 2;

        assert_eq!(
            find_next_marked_source_slot(6, SpellCycleDirection::Next, 1, &enabled, &marks,),
            Some((7, 7))
        );
        assert_eq!(
            find_next_marked_source_slot(7, SpellCycleDirection::Next, 2, &enabled, &marks,),
            Some((8, 8))
        );
        assert_eq!(
            find_next_marked_source_slot(8, SpellCycleDirection::Next, 2, &enabled, &marks,),
            Some((9, 9))
        );
        assert_eq!(
            find_next_marked_source_slot(9, SpellCycleDirection::Previous, 2, &enabled, &marks,),
            Some((8, 8))
        );
    }

    #[test]
    fn scan_wraps_past_disabled_marks_and_has_a_no_match_case() {
        let mut enabled = [false; SOURCE_MARKED_SPELL_SLOT_COUNT];
        let mut marks = [0; SOURCE_MARKED_SPELL_SLOT_COUNT];
        marks[9] = 2;
        marks[0] = 2;
        enabled[0] = true;

        assert_eq!(
            find_next_marked_source_slot(8, SpellCycleDirection::Next, 2, &enabled, &marks,),
            Some((0, 0))
        );

        enabled[0] = false;
        assert_eq!(
            find_next_marked_source_slot(8, SpellCycleDirection::Next, 2, &enabled, &marks,),
            None
        );
    }

    #[test]
    fn stale_nonzero_holder_is_skipped_for_next_live_marked_family() {
        let mut holder_indices = [0; SOURCE_MARKED_SPELL_SLOT_COUNT];
        holder_indices[7] = 101;
        holder_indices[8] = 202;
        let valid_enabled =
            derive_live_enabled_spell_families(&holder_indices, |family| family == 8);
        let mut marks = [0; SOURCE_MARKED_SPELL_SLOT_COUNT];
        marks[7] = 2;
        marks[8] = 2;

        assert_eq!(valid_enabled[7], false);
        assert_eq!(valid_enabled[8], true);
        assert_eq!(
            find_next_marked_source_slot(6, SpellCycleDirection::Next, 2, &valid_enabled, &marks,),
            Some((8, 8))
        );
    }

    #[test]
    fn semantic_mark_cycle_and_replay_preserve_level_two_source_mark() {
        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let resolver = Mc2RuntimePackageResolver::load([
            Mc2CampaignInputs::new(generated.join("mc-bhq8-level1-collision-v5-night")),
            Mc2CampaignInputs::new(generated.join("level2-day-block32-campaign-v2")),
            Mc2CampaignInputs::new(generated.join("level3-day-block32-campaign-recipe-v2")),
        ])
        .unwrap();
        let level_two = resolver.flow().entries()[1].level_id().clone();
        let package = resolver
            .package(&level_two)
            .expect("installed opening flow contains Level 2");
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::clone(&package.definition),
            8801,
            package.inputs.clone(),
        )
        .unwrap();

        let baseline = session.snapshot().value;
        assert!(
            fireball(&baseline).acquired,
            "Level 2 baseline acquires family 0"
        );
        let holder = fireball(&baseline)
            .holder_actor
            .expect("live holder identity");
        let stale = RuntimeActorId(if holder.0 == u64::MAX {
            holder.0 - 1
        } else {
            holder.0 + 1
        });
        let before_stale = session.snapshot();
        for command in [
            GameCommand {
                mark_spell: Some(SpellMark {
                    spell_id: SemanticId::parse("spell.fireball").unwrap(),
                    hand: SpellMarkHand::Left,
                    holder_actor: Some(stale),
                }),
                ..GameCommand::default()
            },
            GameCommand {
                assign_spell: Some(SpellAssignment {
                    spell_id: SemanticId::parse("spell.fireball").unwrap(),
                    tier: 0,
                    hand: SpellAssignmentHand::Left,
                    holder_actor: Some(stale),
                }),
                ..GameCommand::default()
            },
        ] {
            let stale_guard = super::super::tests::guard(&session);
            assert!(session.step(stale_guard, command).is_err());
            assert_eq!(
                session.snapshot(),
                before_stale,
                "stale holder rejection is atomic"
            );
        }
        let mark_guard = super::super::tests::guard(&session);
        let marked = session
            .step(
                mark_guard,
                GameCommand {
                    mark_spell: Some(SpellMark {
                        spell_id: SemanticId::parse("spell.fireball").unwrap(),
                        hand: SpellMarkHand::Left,
                        holder_actor: Some(holder),
                    }),
                    ..GameCommand::default()
                },
            )
            .unwrap()
            .value;
        assert_eq!(fireball(&marked).marked_hand, SpellMarkHand::Left);
        assert_eq!(source_player_record(&session)[398], 1);

        let fireball_tier_before_cycle = fireball(&marked).tier;
        let cycle_guard = super::super::tests::guard(&session);
        let cycled = session
            .step(
                cycle_guard,
                GameCommand {
                    cycle_marked_spell: Some(MarkedSpellCycle {
                        hand: SpellAssignmentHand::Left,
                        direction: SpellCycleDirection::Next,
                    }),
                    ..GameCommand::default()
                },
            )
            .unwrap()
            .value;
        let cycled_fireball = fireball(&cycled);
        assert_eq!(cycled_fireball.hand, SpellHand::Left);
        assert_eq!(cycled_fireball.tier, fireball_tier_before_cycle);
        let source_player = session
            .backend()
            .campaign
            .simulation()
            .shared_spells()
            .local_player();
        assert_eq!(source_player.left_spell_slot_raw, 0);
        assert_eq!(
            source_player.left_subspell_index,
            source_player.family_selected_tiers[0]
        );
        assert_eq!(source_player.record_bytes()[398], 1);

        let unacquired = cycled
            .player_spells
            .iter()
            .find(|spell| !spell.acquired)
            .expect("Level 2 baseline has a known unacquired spell")
            .id
            .clone();
        assert_rejected_mark_is_transactional(&mut session, unacquired);
        assert_rejected_mark_is_transactional(
            &mut session,
            SemanticId::parse("spell.unknown").unwrap(),
        );

        let expected = session.snapshot().value;
        let expected_hash = expected.state_hash;
        // The public runtime enters at the campaign's Level 1 boundary. Seed its
        // replay loader from this real direct Level 2 session, then exercise the
        // existing resolver save/load path over the accepted semantic commands.
        let mut replay = Mc2CampaignReplaySave::new(
            false,
            session.definition().as_ref().clone(),
            session.backend().initial_semantic_spell_carry().unwrap(),
            Vec::new(),
            session.backend().replay_operations.clone(),
            &expected,
        );
        // Preserve the actual source HUD policy just as save_replay does;
        // omission requests the pre-source-HUD replay tail instead.
        replay.hud_admission_changes = session.backend().hud_admission_changes.clone();
        assert_eq!(replay.hud_admission_changes.len(), 1);
        assert_eq!(replay.hud_admission_changes[0].before_operation, 0);
        assert!(replay.hud_admission_changes[0].top_bar);
        replay.receipt_sha256 = replay.compute_receipt_sha256();
        let replay_bytes = serde_json::to_vec(&replay).unwrap();
        let mut loaded = GameRuntime::<Mc2CampaignBackend>::title(8802).unwrap();
        let title_guard = loaded.identity().guard();
        resolver
            .load_replay(&mut loaded, title_guard, 8803, &replay_bytes)
            .unwrap();
        let loaded_session = loaded.session().expect("replay opens Level 2");
        assert_eq!(loaded_session.snapshot().value, expected);
        assert_eq!(loaded_session.snapshot().value.state_hash, expected_hash);
        assert_eq!(source_player_record(loaded_session)[398], 1);

        let saved = resolver.save_replay(&loaded).unwrap();
        let mut restored = GameRuntime::<Mc2CampaignBackend>::title(8804).unwrap();
        let title_guard = restored.identity().guard();
        resolver
            .load_replay(&mut restored, title_guard, 8805, &saved)
            .unwrap();
        let restored_session = restored.session().expect("saved replay restores Level 2");
        assert_eq!(restored_session.snapshot().value, expected);
        assert_eq!(restored_session.snapshot().value.state_hash, expected_hash);
        assert_eq!(
            fireball(&restored_session.snapshot().value).marked_hand,
            SpellMarkHand::Left
        );
        assert_eq!(source_player_record(restored_session)[398], 1);

        // Controlled outer boundary: the real no-Castle death/recovery
        // producer is covered by player_lifecycle_tests. Here retain the
        // genuinely marked spell through retry and its cold replay.
        let before_retry = super::super::RetrySpellBook::from_campaign(
            &loaded.session().unwrap().backend().campaign);
        assert_eq!(before_retry.hand_marks[0], 1);
        loaded.session_mut().unwrap().backend_mut().campaign.simulation_mut()
            .stage_no_castle_retry_for_test();
        let runtime_guard = loaded.identity().guard();
        let session_guard = loaded.session().unwrap().identity().guard();
        assert!(loaded.retry_mission_if_ready(runtime_guard, session_guard).unwrap());
        let retried = loaded.session().unwrap();
        let after_retry = super::super::RetrySpellBook::from_campaign(&retried.backend().campaign);
        assert_eq!(after_retry, before_retry);
        assert_eq!(retried.backend().campaign.simulation().source_exit_result_byte(), 4);
        assert_eq!(fireball(&retried.snapshot().value).marked_hand, SpellMarkHand::Left);
        assert_eq!(retried.backend().campaign.simulation().shared_spells().local_player().runtime_spell_experience, [0; 26]);

        let retry_save = resolver.save_replay(&loaded).unwrap();
        let mut cold_retry = GameRuntime::<Mc2CampaignBackend>::title(8806).unwrap();
        let guard = cold_retry.identity().guard();
        resolver.load_replay(&mut cold_retry, guard, 8807, &retry_save).unwrap();
        let cold = cold_retry.session_mut().unwrap();
        assert_eq!(cold.backend().campaign.simulation().source_exit_result_byte(), 4);
        assert_eq!(super::super::RetrySpellBook::from_campaign(&cold.backend().campaign), before_retry);
        assert_eq!(fireball(&cold.snapshot().value).marked_hand, SpellMarkHand::Left);
        let guard = super::super::tests::guard(cold);
        let cycled = cold.step(guard, GameCommand {
            cycle_marked_spell: Some(MarkedSpellCycle {
                hand: SpellAssignmentHand::Left,
                direction: SpellCycleDirection::Next,
            }),
            ..GameCommand::default()
        }).unwrap().value;
        assert_eq!(fireball(&cycled).hand, SpellHand::Left);
        assert_eq!(fireball(&cycled).marked_hand, SpellMarkHand::Left);
    }
}
