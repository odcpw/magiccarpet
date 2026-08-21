use super::super::tests::level_two_fixture_path;
use super::*;

#[test]
fn level_two_nyphur_participant_chain_is_source_bound_and_reset_isolated() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut first = inspection
        .level2_reset_session()
        .expect("Level 2 Nyphur production session");
    let second = inspection
        .level2_reset_session()
        .expect("isolated Level 2 Nyphur production session");

    let participant = first
        .nyphur_participant()
        .expect("authored Nyphur participant chain");
    assert_eq!(participant.player_slot, 1);
    assert!(participant.is_ai);
    assert_eq!(participant.color_index, 1);
    assert_eq!(participant.entity_index, 112);
    assert_eq!(
        (
            participant.entity_class,
            participant.entity_model,
            participant.entity_action
        ),
        (3, 1, 1)
    );
    assert_eq!(participant.entity_source_id, 112);
    assert_eq!(participant.entity_next_relation, Some(0));
    assert_eq!(participant.entity_next_entity, 113);
    assert_eq!(participant.entity_control_flags, 12);
    assert_eq!(
        (participant.entity_life, participant.entity_max_life),
        (3_085, 3_085)
    );
    assert_eq!(
        (participant.entity_mana, participant.entity_max_mana),
        (1_000, 1_000)
    );
    assert_eq!(
        (
            participant.fireball_holder_index,
            participant.fireball_action,
            participant.fireball_parent_entity_index,
            participant.fireball_next_entity,
        ),
        (113, 0, 112, 114),
    );
    assert_eq!(
        (
            participant.possession_holder_index,
            participant.possession_action,
            participant.possession_parent_entity_index,
            participant.possession_next_entity,
        ),
        (114, 3, 113, 0),
    );
    assert_eq!(
        participant.objective_stage_relation,
        RuntimeRelation::LiveEntity(112),
    );

    let baseline_hash = first.nyphur_state_hash().expect("Nyphur hash");
    assert_eq!(
        baseline_hash,
        second.nyphur_state_hash().expect("second Nyphur hash")
    );
    let baseline_campaign_hash = first.campaign_state_hash();

    first.step_outer_turn(PlayerCommand::default(), 0);
    first.reset();
    assert_eq!(
        first.nyphur_state_hash().expect("reset Nyphur hash"),
        baseline_hash
    );
    assert_eq!(first.campaign_state_hash(), baseline_campaign_hash);

    for _ in 0..282 {
        first.step_outer_turn(PlayerCommand::default(), 0);
    }
    first
        .nyphur_participant()
        .expect("ordinary live target selection preserves the Nyphur participant census");

    let mut relation_tampered = inspection
        .level2_reset_session()
        .expect("Nyphur relation negative session");
    relation_tampered.entities[113].record[26..28].copy_from_slice(&114_u16.to_le_bytes());
    assert!(relation_tampered.nyphur_participant().is_err());

    let mut holder_owner_tampered = inspection
        .level2_reset_session()
        .expect("Nyphur ownership negative session");
    holder_owner_tampered.players[1].enabled_spell_holders[0] = 114;
    holder_owner_tampered.players[1].record[268..270].copy_from_slice(&114_u16.to_le_bytes());
    assert!(holder_owner_tampered.nyphur_participant().is_err());
}

#[test]
fn level_two_ai_dispatcher_0008_preserves_mode_and_tail_order() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 AI dispatcher production session");
    let participant = session
        .nyphur_participant()
        .expect("qualified Nyphur dispatcher participant");
    assert_eq!(participant.entity_index, 112);
    assert_eq!(session.nyphur_dispatch_state_byte(), 0);
    assert!(session.nyphur_dispatch_traces().is_empty());

    let baseline_campaign_hash = session.campaign_state_hash();
    let baseline_dispatch_hash = session.dispatch_observability_hash();
    let baseline_economy_hash = session.economy.reset_hash();
    let baseline_shared_spells_hash = session.shared_spells.reset_hash();
    let baseline_global_rng = session.economy.global_rng;
    let baseline_runtime = session.runtime.clone();
    let baseline_entity_record = session.entities[112].record_bytes().to_vec();
    let baseline_entity_phase = session.entities[112].phase;

    // This is the ordinary production route: the qualified class-3,
    // model-1, action-1 source row is discovered from the indexed entity
    // table and is not made live by a test-only selector or action write.
    session.step_outer_turn(PlayerCommand::default(), 0);
    let traces = session.nyphur_dispatch_traces();
    assert_eq!(traces.len(), 1);
    let trace = traces[0].clone();
    assert_eq!(
        (trace.pass, trace.source_slot, trace.state_byte),
        (0, 112, 0)
    );
    assert_eq!(trace.tail_call_count(), 2);
    assert_eq!(
        trace.calls.as_ref(),
        &[
            Level2NyphurDispatchCall::Sub12a70,
            Level2NyphurDispatchCall::Sub12e70,
            Level2NyphurDispatchCall::Sub12e70,
        ]
    );
    assert_eq!(
        session.entities[112].phase,
        baseline_entity_phase.wrapping_add(1)
    );
    assert_eq!(
        session.entities[112].record_bytes()[66],
        baseline_entity_phase.wrapping_add(1)
    );
    assert!(
        session.nyphur_participant().is_ok(),
        "the source-owned phase increment is the only mutable census byte"
    );
    let mut phase_record_mismatch = session.clone();
    phase_record_mismatch.entities[112].record[66] ^= 1;
    assert!(phase_record_mismatch.nyphur_participant().is_err());
    let mut non_phase_record_mutation = session.clone();
    non_phase_record_mutation.entities[112].record[67] ^= 1;
    assert!(non_phase_record_mutation.nyphur_participant().is_err());
    assert_ne!(session.campaign_state_hash(), baseline_campaign_hash);
    let stepped_campaign_hash = session.campaign_state_hash();
    let stepped_dispatch_hash = session.dispatch_observability_hash();

    session.reset();
    assert_eq!(session.nyphur_dispatch_state_byte(), 0);
    assert!(session.nyphur_dispatch_traces().is_empty());
    assert!(session.dispatch_events().is_empty());
    assert_eq!(session.economy.global_rng, baseline_global_rng);
    assert_eq!(session.economy.reset_hash(), baseline_economy_hash);
    assert_eq!(
        session.shared_spells.reset_hash(),
        baseline_shared_spells_hash
    );
    assert_eq!(session.runtime, baseline_runtime);
    assert_eq!(session.entities[112].record_bytes(), baseline_entity_record);
    assert_eq!(
        session.dispatch_observability_hash(),
        baseline_dispatch_hash
    );
    assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.nyphur_dispatch_traces(), &[trace]);
    assert_eq!(session.campaign_state_hash(), stepped_campaign_hash);
    assert_eq!(session.dispatch_observability_hash(), stepped_dispatch_hash);

    // Isolated selector probes cover the signed i8 carrier and every
    // denominator-owned case without mutating the qualified production
    // session. A generic "helper then one tail" implementation fails the
    // mode-0 case, while a thresholded u8 carrier fails the signed/default
    // cases below.
    let mode_helpers = [
        (1_i8, Level2NyphurDispatchHelper::Sub12ff0),
        (2, Level2NyphurDispatchHelper::Nmemneed),
        (3, Level2NyphurDispatchHelper::Sub13100),
        (4, Level2NyphurDispatchHelper::Sub131f0),
        (5, Level2NyphurDispatchHelper::Nmemneed0),
        (6, Level2NyphurDispatchHelper::Sub135c0),
        (7, Level2NyphurDispatchHelper::Sub13710),
        (8, Level2NyphurDispatchHelper::Sub13830),
        (9, Level2NyphurDispatchHelper::Sub13870),
        (11, Level2NyphurDispatchHelper::Sub133b0),
        (12, Level2NyphurDispatchHelper::Sub13270),
        (13, Level2NyphurDispatchHelper::Sub13850),
        (14, Level2NyphurDispatchHelper::Sub161a0),
    ];
    for (state_byte, helper) in mode_helpers {
        let trace = Level2NyphurDispatcher::trace_for_state(state_byte, 0, 112);
        assert_eq!(trace.state_byte, state_byte);
        assert_eq!(trace.tail_call_count(), 1);
        assert_eq!(
            trace.calls.as_ref(),
            &[
                Level2NyphurDispatchCall::Sub12a70,
                Level2NyphurDispatchCall::Helper(helper),
                Level2NyphurDispatchCall::Sub12e70,
            ]
        );
    }
    for state_byte in [10_i8, -1_i8, i8::MIN, i8::MAX] {
        let trace = Level2NyphurDispatcher::trace_for_state(state_byte, 0, 112);
        assert_eq!(trace.state_byte, state_byte);
        assert_eq!(trace.tail_call_count(), 1);
        assert_eq!(
            trace.calls.as_ref(),
            &[
                Level2NyphurDispatchCall::Sub12a70,
                Level2NyphurDispatchCall::Sub12e70,
            ]
        );
    }
}

#[test]
fn level_two_nyphur_mode14_distance_boundaries_match_pinned_source() {
    // These pure threshold checks document the three distinct source
    // comparisons. They are boundary evidence only; they do not create a
    // family-4 holder or claim an ordinary mode-14 activation.
    assert!(level2_nyphur_mode14_primary_admitted(0x1400));
    assert!(!level2_nyphur_mode14_primary_admitted(0x1401));
    assert!(level2_nyphur_mode14_bucket_admitted(0x13ff));
    assert!(!level2_nyphur_mode14_bucket_admitted(0x1400));
    assert!(!level2_nyphur_mode14_attack_band(0x0a00));
    assert!(level2_nyphur_mode14_attack_band(0x0a01));
    assert!(level2_nyphur_mode14_attack_band(0x13ff));
    assert!(!level2_nyphur_mode14_attack_band(0x1400));
}

#[test]
fn level_two_nyphur_mode14_ordinary_route_fails_closed_without_family4_holder() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("qualified Nyphur mode-14 negative session");
    let baseline_hash = session.campaign_state_hash();
    let baseline_common = session
        .nyphur_common_upkeep()
        .expect("qualified Nyphur common carrier");
    assert_eq!(baseline_common.spell_holder_indices[4], 0);

    // This is the ordinary production cursor, including every qualified
    // cadence opportunity in the focused window. With no source-owned
    // family-4 carrier, sub_15FC0 must refuse the transition rather than
    // borrow the authored family-0/1 holders or write state 14 directly.
    for _ in 0..282 {
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert!(
        session
            .nyphur_dispatch_traces()
            .iter()
            .all(|trace| trace.state_byte != 14)
    );
    assert!(
        session
            .nyphur_dispatch_traces()
            .iter()
            .flat_map(|trace| trace.calls.iter())
            .all(|call| {
                !matches!(
                    call,
                    Level2NyphurDispatchCall::Helper(Level2NyphurDispatchHelper::Sub161a0)
                )
            })
    );
    assert_eq!(
        session
            .nyphur_common_upkeep()
            .expect("post-cursor Nyphur common carrier")
            .spell_holder_indices[4],
        0
    );
    assert!(session.dispatch_events().iter().all(|event| !matches!(
        event,
        Level2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    assert_eq!(
        session
            .nyphur_common_upkeep()
            .expect("reset Nyphur common carrier")
            .spell_holder_indices[4],
        0
    );
}

#[test]
fn level_two_nyphur_mode14_rejects_non_family4_holder_carrier() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("qualified Nyphur malformed-carrier session");
    let source_before = session.entities[112].record_bytes().to_vec();
    let fireball_holder = session
        .nyphur_common_upkeep()
        .expect("qualified Nyphur common carrier")
        .spell_holder_indices[0];
    assert_ne!(fireball_holder, 0);

    // Reusing the authored family-0 row as a family-4 table entry is a
    // malformed carrier, not a producer. The source-owned class/model/
    // action/parent checks must reject it without touching the source.
    session.nyphur_dispatcher.common_upkeep.spell_holder_indices[4] = fireball_holder;
    assert!(
        !session
            .nyphur_dispatcher
            .step_sub_15fc0(112, &mut session.entities)
    );
    assert_eq!(
        session.entities[112].record_bytes(),
        source_before.as_slice()
    );
    assert!(
        session
            .nyphur_dispatcher
            .common_upkeep
            .source_family4_holder(&session.entities)
            .is_none()
    );

    // A stale externally latched state is equally fail-closed at the
    // mode-14 handler boundary; it cannot consume the family-0 holder.
    session.nyphur_dispatcher.state_byte = 14;
    let terrain = session.terrain.clone();
    session
        .nyphur_dispatcher
        .step_sub_161a0(112, &mut session.entities, &session.players, &terrain)
        .expect("malformed mode-14 carrier fails through movement epilogue");
    assert_eq!(session.entities[112].source_word_96(), 0);
    assert_eq!(session.entities[112].source_word_98(), 0);
}

#[test]
fn level_two_ai_modes_0_1_2_are_exact() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut ordinary = inspection
        .level2_reset_session()
        .expect("Level 2 basic-mode production session");
    let baseline_hash = ordinary.campaign_state_hash();

    // Qualified reset enters source case 0 without a state/action write by
    // the test. It executes the shared selector twice, including when the
    // first call changes the next-turn state byte.
    ordinary.step_outer_turn(PlayerCommand::default(), 0);
    let mode0_trace = ordinary
        .nyphur_dispatch_traces()
        .first()
        .expect("ordinary case-0 trace");
    assert_eq!(mode0_trace.state_byte, 0);
    assert_eq!(mode0_trace.tail_call_count(), 2);
    assert_eq!(
        mode0_trace.calls.as_ref(),
        &[
            Level2NyphurDispatchCall::Sub12a70,
            Level2NyphurDispatchCall::Sub12e70,
            Level2NyphurDispatchCall::Sub12e70,
        ]
    );
    let stepped_hash = ordinary.campaign_state_hash();
    ordinary.reset();
    assert_eq!(ordinary.campaign_state_hash(), baseline_hash);
    ordinary.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(ordinary.campaign_state_hash(), stepped_hash);

    // Let the ordinary selector establish a valid source slot/identity
    // pair. The helper vectors below change only geometry to discriminate
    // sub_12FF0's inclusive 512 threshold from a generic near-target test.
    let mut related = inspection
        .level2_reset_session()
        .expect("Level 2 mode-1 relation session");
    for _ in 0..53 {
        related.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert_eq!(related.entities[112].source_word_96(), 70);
    assert!(level2_nyphur_source_identity_matches(
        &related.entities[112],
        &related.entities[70]
    ));

    let mode1_trace = Level2NyphurDispatcher::trace_for_state(1, 0, 112);
    assert_eq!(
        mode1_trace.calls.as_ref(),
        &[
            Level2NyphurDispatchCall::Sub12a70,
            Level2NyphurDispatchCall::Helper(Level2NyphurDispatchHelper::Sub12ff0),
            Level2NyphurDispatchCall::Sub12e70,
        ]
    );
    assert_eq!(mode1_trace.tail_call_count(), 1);

    let mut dispatched_mode1 = related.clone();
    dispatched_mode1.nyphur_dispatcher.state_byte = 1;
    dispatched_mode1.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        dispatched_mode1
            .nyphur_dispatch_traces()
            .last()
            .expect("dispatched mode-1 trace")
            .calls,
        mode1_trace.calls
    );
    assert!(
        dispatched_mode1
            .dispatch_events()
            .iter()
            .all(|event| !matches!(
                event,
                Level2DispatchEvent::Error {
                    source_slot: 112,
                    ..
                }
            ))
    );

    let source_position = Position {
        x: 10_000,
        y: 10_000,
        z: 2_048,
    };
    let mut inclusive = related.clone();
    inclusive.entities[112].set_position(source_position);
    inclusive.entities[70].set_position(Position {
        x: source_position.x + 512,
        ..source_position
    });
    let inclusive_terrain = inclusive.terrain.clone();
    inclusive
        .nyphur_dispatcher
        .step_sub_12ff0(112, &mut inclusive.entities, &inclusive_terrain)
        .expect("inclusive sub_12FF0 threshold");
    assert_eq!(inclusive.nyphur_dispatcher.common_upkeep.speed, 0);
    assert_eq!(inclusive.nyphur_dispatcher.common_upkeep.word_0xe_14, 1);
    assert_eq!(inclusive.entities[112].position.z, source_position.z - 1);

    let mut exact_height = related.clone();
    exact_height.entities[112].set_position(source_position);
    exact_height.entities[70].set_position(Position {
        z: source_position.z - 512,
        ..source_position
    });
    let exact_height_terrain = exact_height.terrain.clone();
    exact_height
        .nyphur_dispatcher
        .step_sub_12ff0(112, &mut exact_height.entities, &exact_height_terrain)
        .expect("equal target-height delta");
    assert_eq!(exact_height.entities[112].position.z, source_position.z);

    let mut exclusive = related.clone();
    exclusive.entities[112].set_position(source_position);
    exclusive.entities[70].set_position(Position {
        x: source_position.x + 513,
        ..source_position
    });
    let exclusive_terrain = exclusive.terrain.clone();
    exclusive
        .nyphur_dispatcher
        .step_sub_12ff0(112, &mut exclusive.entities, &exclusive_terrain)
        .expect("exclusive sub_12FF0 threshold");
    assert_eq!(
        exclusive.nyphur_dispatcher.common_upkeep.speed,
        exclusive.entities[112].minimum_speed()
    );
    assert_eq!(exclusive.entities[112].position.z, source_position.z);

    let mut stale = inclusive.clone();
    let preserved_roll = stale.entities[112].source_roll();
    let stale_identity = level2_nyphur_source_identity(&stale.entities[70]).wrapping_add(1);
    stale.entities[112].record[156..158].copy_from_slice(&stale_identity.to_le_bytes());
    let stale_terrain = stale.terrain.clone();
    stale
        .nyphur_dispatcher
        .step_sub_12ff0(112, &mut stale.entities, &stale_terrain)
        .expect("stale identity is a source false result");
    assert_eq!(stale.entities[112].source_roll(), preserved_roll);

    // Case 2 exists in sub_12910, but the pinned complete assignment set
    // contains no write of value 2. Retain the structural hook and shared
    // tail without manufacturing a reachable transition.
    let mode2_trace = Level2NyphurDispatcher::trace_for_state(2, 0, 112);
    assert_eq!(
        mode2_trace.calls.as_ref(),
        &[
            Level2NyphurDispatchCall::Sub12a70,
            Level2NyphurDispatchCall::Helper(Level2NyphurDispatchHelper::Nmemneed),
            Level2NyphurDispatchCall::Sub12e70,
        ]
    );
    let mut dispatched_mode2 = related.clone();
    dispatched_mode2.nyphur_dispatcher.state_byte = 2;
    dispatched_mode2.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        dispatched_mode2
            .nyphur_dispatch_traces()
            .last()
            .expect("dispatched structural mode-2 trace")
            .calls,
        mode2_trace.calls
    );
    assert!(
        dispatched_mode2
            .dispatch_events()
            .iter()
            .all(|event| !matches!(
                event,
                Level2DispatchEvent::Error {
                    source_slot: 112,
                    ..
                }
            ))
    );
    let mut production = inspection
        .level2_reset_session()
        .expect("Level 2 production exclusion session");
    for _ in 0..282 {
        production.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert!(
        production
            .nyphur_dispatch_traces()
            .iter()
            .all(|trace| trace.state_byte != 2)
    );
}

#[test]
fn level_two_ai_modes_8_9_13_and_mode5_nocall_are_exact() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut initial = inspection
        .level2_reset_session()
        .expect("Level 2 Nyphur mode packet production session");
    let initial_campaign_hash = initial.campaign_state_hash();
    let initial_dispatch_hash = initial.dispatch_observability_hash();
    let initial_entities = initial.entities.clone();
    let initial_common = initial
        .nyphur_common_upkeep()
        .expect("initial Nyphur common carrier");

    // This is the ordinary production route: sub_12E70 selects the
    // source-backed class-5 target and latches mode 13. No test-only state
    // write creates the target relation or makes the AI source live.
    for _ in 0..53 {
        initial.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert_eq!(initial.nyphur_dispatch_state_byte(), 13);
    let source = initial.entities[112].clone();
    let target = initial.entities[70].clone();
    assert!(level2_nyphur_source_identity_matches(&source, &target));
    assert_eq!(source.source_word_96(), target.index);
    assert!(target.model == 0 || target.model == 1);
    let mode_common_before = initial
        .nyphur_common_upkeep()
        .expect("pre-mode common carrier");
    assert!(
        initial.nyphur_participant().is_ok(),
        "the source-owned live target relation remains a valid participant census row"
    );
    let mut stale_relation = initial.clone();
    stale_relation.entities[112].record[156..158]
        .copy_from_slice(&source.source_word_98().wrapping_add(1).to_le_bytes());
    assert!(stale_relation.nyphur_participant().is_err());
    let mut unrelated_census_mutation = initial.clone();
    unrelated_census_mutation.entities[112].record[158] ^= 1;
    assert!(unrelated_census_mutation.nyphur_participant().is_err());

    let expected_roll = i16::try_from(crate::heading_for_positions(
        source.position,
        target.position,
    ))
    .expect("mode helper roll fits signed source carrier");

    // All three pinned wrappers enter the same sub_13890 body, but their
    // wrapper symbols remain observable in the dispatcher trace. The far
    // branch's source roll, no movement RNG/position write, and
    // minimum-speed latch are deliberately checked on the real outer
    // production cursor.
    let mut mode13_result = None;
    for (state_byte, helper) in [
        (8_i8, Level2NyphurDispatchHelper::Sub13830),
        (9_i8, Level2NyphurDispatchHelper::Sub13870),
        (13_i8, Level2NyphurDispatchHelper::Sub13850),
    ] {
        let mut candidate = initial.clone();
        candidate.nyphur_dispatcher.state_byte = state_byte;
        candidate.step_outer_turn(PlayerCommand::default(), 0);
        let trace = candidate
            .nyphur_dispatch_traces()
            .iter()
            .rev()
            .find(|trace| trace.source_slot == 112)
            .cloned()
            .expect("mode helper ordinary source trace");
        assert_eq!(trace.state_byte, state_byte);
        assert_eq!(trace.tail_call_count(), 1);
        assert_eq!(
            trace.calls.as_ref(),
            &[
                Level2NyphurDispatchCall::Sub12a70,
                Level2NyphurDispatchCall::Helper(helper),
                Level2NyphurDispatchCall::Sub12e70,
            ]
        );
        assert!(candidate.dispatch_events().iter().all(|event| !matches!(
            event,
            Level2DispatchEvent::Error {
                source_slot: 112,
                ..
            }
        )));
        let stepped_source = &candidate.entities[112];
        assert_eq!(
            stepped_source.entity_rng, source.entity_rng,
            "far state-13 must not consume the movement RNG"
        );
        assert_eq!(
            stepped_source.yaw, source.yaw,
            "far state-13 must not enter the movement yaw branch"
        );
        assert_eq!(
            stepped_source.position, source.position,
            "far state-13 must not enter the movement position branch"
        );
        assert_eq!(stepped_source.source_roll(), expected_roll);
        let common = candidate
            .nyphur_common_upkeep()
            .expect("mode helper common carrier");
        assert_eq!(common.speed, source.minimum_speed());
        assert_eq!(common.word_0xe_14, 1);
        assert_eq!(common.byte_0x45e, mode_common_before.byte_0x45e);
        assert_eq!(common.byte_0x45c, mode_common_before.byte_0x45c);
        assert_eq!(common.byte_0x45d, mode_common_before.byte_0x45d);
        if state_byte == 13 {
            mode13_result = Some(candidate);
        }
    }

    let mut direction_carrier = initial.clone();
    direction_carrier.nyphur_dispatcher.common_upkeep.byte_0x45e = 3;
    direction_carrier
        .nyphur_dispatcher
        .step_sub_13850(
            112,
            &mut direction_carrier.entities,
            &direction_carrier.players,
            &direction_carrier.terrain,
            &direction_carrier.presentation,
        )
        .expect("sub_16580 direction carrier increment");
    assert_eq!(
        direction_carrier.nyphur_dispatcher.common_upkeep.byte_0x45e,
        4
    );

    let mut reset_carrier = initial.clone();
    reset_carrier.nyphur_dispatcher.common_upkeep.byte_0x45e = 8;
    reset_carrier
        .nyphur_dispatcher
        .step_sub_13850(
            112,
            &mut reset_carrier.entities,
            &reset_carrier.players,
            &reset_carrier.terrain,
            &reset_carrier.presentation,
        )
        .expect("sub_16580 terrain carrier reset");
    assert_eq!(reset_carrier.nyphur_dispatcher.common_upkeep.byte_0x45e, 0);

    // The source compares the stored sub_14C40 identity, not merely the
    // target slot. This planted negative rejects a naive slot-only helper
    // while leaving all source carriers untouched on failure.
    let mut wrong_identity = initial.clone();
    let source_before_negative = wrong_identity.entities[112].record_bytes().to_vec();
    wrong_identity.entities[70].model = wrong_identity.entities[70].model.wrapping_add(1);
    wrong_identity.entities[70].record[68] = wrong_identity.entities[70].model;
    assert!(
        wrong_identity
            .nyphur_dispatcher
            .step_sub_13850(
                112,
                &mut wrong_identity.entities,
                &wrong_identity.players,
                &wrong_identity.terrain,
                &wrong_identity.presentation,
            )
            .is_err()
    );
    assert_eq!(
        wrong_identity.entities[112].record_bytes(),
        source_before_negative.as_slice()
    );

    // Case 5 calls only the pinned weak `_nmemneed_0` hook before the
    // common tail. On this valid relation, a generic mode helper would
    // consume RNG and move the source; the structural branch changes only
    // the ordinary outer phase byte and emits the exact call sequence.
    let mut mode5 = initial.clone();
    mode5.nyphur_dispatcher.state_byte = 5;
    let mode5_source_before = mode5.entities[112].record_bytes().to_vec();
    mode5.step_outer_turn(PlayerCommand::default(), 0);
    let mode5_trace = mode5
        .nyphur_dispatch_traces()
        .iter()
        .rev()
        .find(|trace| trace.source_slot == 112)
        .cloned()
        .expect("mode-5 structural source trace");
    assert_eq!(mode5_trace.state_byte, 5);
    assert_eq!(mode5_trace.tail_call_count(), 1);
    assert_eq!(
        mode5_trace.calls.as_ref(),
        &[
            Level2NyphurDispatchCall::Sub12a70,
            Level2NyphurDispatchCall::Helper(Level2NyphurDispatchHelper::Nmemneed0),
            Level2NyphurDispatchCall::Sub12e70,
        ]
    );
    let mut mode5_expected_record = mode5_source_before;
    mode5_expected_record[66] = mode5_expected_record[66].wrapping_add(1);
    assert_eq!(
        mode5.entities[112].record_bytes(),
        mode5_expected_record.as_slice()
    );
    assert!(mode5.dispatch_events().iter().all(|event| !matches!(
        event,
        Level2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));

    // The ordinary mode-13 result is reset-isolated and replay-stable;
    // this covers the newly added carriers, source record, trace order,
    // and both observable hashes without treating a focused test as a
    // complete Level 2 equivalence claim.
    let mode13_result = mode13_result.expect("mode-13 production result");
    let stepped_campaign_hash = mode13_result.campaign_state_hash();
    let stepped_dispatch_hash = mode13_result.dispatch_observability_hash();
    let stepped_entities = mode13_result.entities.clone();
    let stepped_common = mode13_result
        .nyphur_common_upkeep()
        .expect("stepped mode-13 common carrier");
    let stepped_traces = mode13_result.nyphur_dispatch_traces().to_vec();
    let mut replay = mode13_result.clone();
    replay.reset();
    assert_eq!(replay.campaign_state_hash(), initial_campaign_hash);
    assert_eq!(replay.dispatch_observability_hash(), initial_dispatch_hash);
    assert_eq!(replay.entities, initial_entities);
    assert_eq!(replay.nyphur_common_upkeep().unwrap(), initial_common);
    for _ in 0..54 {
        replay.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert_eq!(replay.campaign_state_hash(), stepped_campaign_hash);
    assert_eq!(replay.dispatch_observability_hash(), stepped_dispatch_hash);
    assert_eq!(replay.entities, stepped_entities);
    assert_eq!(replay.nyphur_common_upkeep().unwrap(), stepped_common);
    assert_eq!(replay.nyphur_dispatch_traces(), stepped_traces.as_slice());
}

#[test]
fn level_two_nyphur_sub_148e0_source_order_and_fail_closed_are_exact() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let session = inspection
        .level2_reset_session()
        .expect("qualified Level 2 Nyphur selector session");
    let source = session.entities[112].clone();

    // The qualified package intentionally has no sub_148E0 target rows;
    // the ordinary route must therefore leave both its source relation
    // and Watcom stream untouched rather than borrow sub_14530's model-1
    // target.
    let mut no_target = session.nyphur_dispatcher.common_upkeep.clone();
    let rng_before = no_target.watcom_rand_state;
    assert!(
        no_target
            .step_sub_13ce0(&source, &session.entities, &session.players)
            .is_none()
    );
    assert_eq!(no_target.watcom_rand_state, rng_before);
    assert!(level2_nyphur_class10_target_source_order(&session.entities).is_empty());
    assert!(
        level2_nyphur_class5_bucket_order(&session.entities)
            .into_iter()
            .filter_map(|index| session.entities.get(usize::from(index)))
            .all(|entity| entity.model != 22)
    );

    // This source-index vector only represents the rebuilt dword_38523
    // links. It deliberately does not move a target, write word 0x96/0x98,
    // or enter the public dispatcher; it is focused order/nearest-helper
    // coverage, not ordinary activation evidence.
    let mut indexed_entities = session.entities.clone();
    indexed_entities[1].model = 39;
    indexed_entities[1].record[68] = 39;
    indexed_entities[1].next_relation = Some(2);
    indexed_entities[1].record[4..8].copy_from_slice(&3_u32.to_le_bytes());
    indexed_entities[2].model = 40;
    indexed_entities[2].record[68] = 40;
    indexed_entities[2].next_relation = None;
    indexed_entities[2].record[4..8].fill(0);
    assert_eq!(
        level2_nyphur_class10_target_source_order(&indexed_entities),
        vec![1, 2]
    );

    let mut indexed_selector = session.nyphur_dispatcher.common_upkeep.clone();
    indexed_selector.word_0x244 = 0;
    assert_eq!(
        indexed_selector.step_sub_13ce0(&source, &indexed_entities, &session.players,),
        Some(2),
        "strict nearest selection retains source-list order for later ties"
    );

    // Model 57 draws before its owner-row gates and breaks the primary
    // list when perception succeeds. With no model-22 fallback this is a
    // negative result, but the single draw remains observable.
    indexed_entities[1].model = 57;
    indexed_entities[1].record[68] = 57;
    let mut model57_selector = session.nyphur_dispatcher.common_upkeep.clone();
    model57_selector.word_0x244 = i16::MAX;
    let mut expected_rng = model57_selector.watcom_rand_state;
    expected_rng = expected_rng
        .wrapping_mul(1_103_515_245)
        .wrapping_add(12_345);
    assert!(
        model57_selector
            .step_sub_13ce0(&source, &indexed_entities, &session.players)
            .is_none()
    );
    assert_eq!(model57_selector.watcom_rand_state, expected_rng);
}

#[test]
fn level_two_nyphur_sub_148e0_selects_authored_model39_from_ordinary_route() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("qualified ordinary Nyphur model-39 route");
    let authored_features = session.mission().authored_features().to_vec();
    let feature_position = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored group-1 feature")
            .world_position(0)
    };
    let group1_positions = (12_u16..=15)
        .chain(17..=40)
        .map(feature_position)
        .map(|position| (position.x, position.y))
        .collect::<BTreeSet<_>>();
    let local_player = session.state.entity_index;
    let row11_position = feature_position(11);

    // The player input reaches the imported row-11 switch through the public
    // campaign route. It is not a target setter and does not rewrite any
    // Nyphur or target position.
    session
        .set_level2_entity_position(local_player, row11_position)
        .expect("public player input for authored row 11");
    session.step_outer_turn(PlayerCommand::default(), 0);

    let authored_model39_slots = session
        .economy
        .spheres()
        .iter()
        .filter(|sphere| {
            sphere.active && group1_positions.contains(&(sphere.position.x, sphere.position.y))
        })
        .map(|sphere| sphere.source_entity_index)
        .collect::<BTreeSet<_>>();
    assert_eq!(authored_model39_slots.len(), 28);

    let mut selected = None;
    for _ in 0..282 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        let target_slot =
            session.entities[usize::from(LEVEL2_NYPHUR_ENTITY_INDEX)].source_word_96();
        if authored_model39_slots.contains(&target_slot) {
            selected = Some(target_slot);
            break;
        }
    }
    let selected_slot = selected.expect("ordinary sub_148E0 selects an authored model-39 row");
    let selected_sphere = session
        .economy
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == selected_slot)
        .expect("selected authored model-39 remains in typed economy carrier");
    assert!(selected_sphere.active);
    assert_eq!(selected_sphere.player_entity_index, 0);
    // The shared tail latches state 6 after the selector callback. Its
    // dispatch trace is therefore emitted on the following ordinary pass,
    // preserving the source's state-byte handoff order.
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session.nyphur_dispatch_traces().iter().any(|trace| {
            trace.source_slot == LEVEL2_NYPHUR_ENTITY_INDEX && trace.state_byte == 6
        })
    );
    let selected_sphere_after_dispatch = session
        .economy
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == selected_slot)
        .expect("selected authored model-39 remains after the state-6 handoff");
    assert!(selected_sphere_after_dispatch.active);
    assert_eq!(selected_sphere_after_dispatch.player_entity_index, 0);
    assert_eq!(
        session.entities[usize::from(LEVEL2_NYPHUR_ENTITY_INDEX)].source_word_96(),
        selected_slot
    );
}

#[test]
fn level_two_nyphur_sub_14c90_boundaries_match_pinned_source() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let base = inspection
        .level2_reset_session()
        .expect("qualified Level 2 Nyphur distance-gate session");
    let source_index = base
        .entities
        .iter()
        .position(|entity| entity.index == 112)
        .expect("authored Nyphur source row");
    let source = base.entities[source_index].clone();

    // These are direct gate probes only. They use a detached target value
    // and never rewrite target 70 or enter the public dispatcher; the
    // ordinary source route remains covered by the test above.
    let target_at_distance = |distance: u32| {
        let mut target = base.entities[70].clone();
        target.set_position(Position {
            x: source.position.x,
            y: source.position.y,
            z: source
                .position
                .z
                .wrapping_add(i16::try_from(distance).expect("distance fits source coordinate")),
        });
        target
    };
    let assert_gate = |label: &str,
                       near_distance: u32,
                       far_distance: u32,
                       distance: u32,
                       expected_close: bool| {
        let mut candidate = base.clone();
        let target = target_at_distance(distance);
        let gate_result = candidate.nyphur_dispatcher.step_sub_14c90(
            source_index,
            Some(&target),
            None,
            near_distance,
            far_distance,
            &mut candidate.entities,
        );
        assert!(
            gate_result.is_ok(),
            "{label}: distance gate returned an error"
        );
        let close = gate_result.unwrap_or(false);
        assert_eq!(close, expected_close, "{label}: gate result");
        assert_eq!(
            candidate.nyphur_dispatcher.common_upkeep.word_0xe_14, 1,
            "{label}: near and failed-far paths latch movement"
        );
        if expected_close {
            assert_eq!(
                candidate.nyphur_dispatcher.common_upkeep.speed, 0,
                "{label}"
            );
        } else {
            assert_eq!(
                candidate.nyphur_dispatcher.common_upkeep.speed,
                source.minimum_speed(),
                "{label}"
            );
        }
    };

    // Pinned sub_135C0 (mode 6) calls sub_14C90 with 1024/3072.
    assert_gate("mode-6 distance == 1024", 1_024, 3_072, 1_024, true);
    assert_gate("mode-6 distance == 1025", 1_024, 3_072, 1_025, false);

    // Pinned sub_13710 (mode 7) calls sub_14C90 with 2048/3584.
    assert_gate("mode-7 distance == 2048", 2_048, 3_584, 2_048, true);
    assert_gate("mode-7 distance == 2049", 2_048, 3_584, 2_049, false);

    // Pinned sub_13890 calls sub_14C90 with 3328/4608. Equality at a3
    // returns one; every distance in (3328, 4608], including a4, takes
    // minimum-speed movement and does not reach family-0/1 selection.
    assert_gate("state-13 distance == 3328", 3_328, 4_608, 3_328, true);
    assert_gate("state-13 distance == 3329", 3_328, 4_608, 3_329, false);
    assert_gate("state-13 distance == 4608", 3_328, 4_608, 4_608, false);

    // The >a4 branch offers family-3 Speed Up before returning zero. This
    // is a controlled carrier vector for the helper's source branch only:
    // the qualified fresh package has no family-3 holder, so this is not
    // ordinary producer or activation proof.
    let mut speed_up = base.clone();
    let speed_profile =
        level2_direct_starting_spell_profile(3, 0).expect("bounded family-3 Speed Up profile");
    let speed_activation = speed_profile.word_0x30;
    let speed_holder_slot = base
        .entities
        .iter()
        .enumerate()
        .find_map(|(slot, entity)| {
            let slot = u16::try_from(slot).ok()?;
            (slot != 0
                && !matches!(slot, 112 | 113 | 114)
                && !entity.active
                && entity.index == slot)
                .then_some(slot)
        })
        .expect("controlled unused family-3 holder slot");
    let speed_holder = speed_up
        .entities
        .iter_mut()
        .find(|entity| entity.index == speed_holder_slot)
        .expect("controlled family-3 holder row");
    speed_holder.active_value = 0;
    speed_holder.record[50..52].fill(0);
    speed_holder.apply_nyphur_spell_profile(speed_profile, 0);
    speed_holder.class = 15;
    speed_holder.record[67] = 15;
    speed_holder.model = 3;
    speed_holder.record[68] = 3;
    speed_holder.active = true;
    speed_holder.life = 0;
    speed_up
        .nyphur_dispatcher
        .common_upkeep
        .spell_holder_indices[3] = speed_holder_slot;
    speed_up.nyphur_dispatcher.common_upkeep.spell_enabled[3] = 0;
    speed_up.nyphur_dispatcher.common_upkeep.max_mana = 1_000;
    speed_up.nyphur_dispatcher.common_upkeep.mana = 1_000;
    speed_up.nyphur_dispatcher.common_upkeep.speed = -1_234;
    assert!(
        speed_up
            .nyphur_dispatcher
            .source_spell_ready(3, &speed_up.entities),
        "controlled family-3 carrier satisfies the source availability predicate"
    );
    let speed_target = target_at_distance(4_609);
    let close = speed_up
        .nyphur_dispatcher
        .step_sub_14c90(
            source_index,
            Some(&speed_target),
            None,
            3_328,
            4_608,
            &mut speed_up.entities,
        )
        .expect("family-3 Speed Up boundary branch");
    assert!(!close, ">4608 remains a failed distance gate");
    assert_eq!(
        speed_up.nyphur_dispatcher.common_upkeep.word_0xe_14, 0,
        ">4608 successful Speed Up offer does not latch minimum-speed movement"
    );
    assert_eq!(
        speed_up.nyphur_dispatcher.common_upkeep.speed, -1_234,
        ">4608 successful Speed Up offer preserves the speed carrier"
    );
    assert_eq!(
        speed_up.nyphur_dispatcher.common_upkeep.spell_enabled[3], LEVEL2_NYPHUR_SPELL_COOLDOWNS[3],
        "family-3 Speed Up installs its source cooldown"
    );
    let live_speed_holder = speed_up
        .entities
        .iter()
        .find(|entity| entity.index == speed_holder_slot)
        .expect("live controlled family-3 holder row");
    assert_eq!(
        live_speed_holder.active_value,
        i16::try_from(speed_activation).expect("Speed Up activation fits source carrier")
    );
}

#[test]
fn level_two_ai_modes_3_4_11_12_are_exact() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut ordinary = inspection
        .level2_reset_session()
        .expect("qualified Level 2 Nyphur production session");

    // The public outer cursor is the production carrier.  It discovers
    // the authored source row and advances the common callback before
    // any mode helper is selected; no state or action byte is written by
    // this test setup.
    ordinary.step_outer_turn(PlayerCommand::default(), 0);
    let ordinary_trace = ordinary
        .nyphur_dispatch_traces()
        .first()
        .expect("ordinary source dispatch trace");
    assert_eq!(ordinary_trace.state_byte, 0);
    assert_eq!(ordinary_trace.tail_call_count(), 2);
    let mut production_probe = ordinary.clone();
    let mut production_states = BTreeSet::new();
    for _ in 0..220 {
        production_probe.step_outer_turn(PlayerCommand::default(), 0);
        if let Some(trace) = production_probe.nyphur_dispatch_traces().last() {
            production_states.insert(trace.state_byte);
        }
    }
    assert_eq!(
        production_states,
        BTreeSet::from([0_i8, 13_i8]),
        "the qualified cursor currently reaches only default and the earlier state-13 selector"
    );

    // sub_13B00 is a production selector, not a test-only state setter.
    // Supply its exact missing family-2 player relation while leaving the
    // dispatcher state/action untouched, then prove that the ordinary
    // source cursor writes the arena axis and latches mode 3 for the next
    // callback. The qualified package itself has no such holder relation,
    // so this is selector coverage rather than a natural-acquisition claim.
    let mut selector = ordinary.clone();
    assert_eq!(selector.nyphur_dispatch_state_byte(), 0);
    selector
        .nyphur_dispatcher
        .common_upkeep
        .spell_holder_indices[2] = 113;
    let selector_source = selector.entities[112].clone();
    let expected_axis = selector
        .nyphur_dispatcher
        .sub_13b00_axis(&selector_source, &selector.entities)
        .expect("family-2 selector has a free arena cell");
    selector.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(selector.nyphur_dispatch_state_byte(), 3);
    assert_eq!(
        level2_nyphur_axis_position(&selector.entities[112]).unwrap(),
        expected_axis
    );
    assert_eq!(
        selector
            .nyphur_dispatch_traces()
            .last()
            .expect("selector source trace")
            .state_byte,
        0,
        "sub_12910 latches the old byte before sub_12E70 selects mode 3"
    );
    selector.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        selector
            .nyphur_dispatch_traces()
            .last()
            .expect("mode-3 source trace")
            .state_byte,
        3
    );

    // sub_583B0's threshold is strict: equality blocks the first cell,
    // while one unit beyond it selects that cell. A generic nearest/free
    // test with a non-strict comparison fails this pair.
    let mut threshold_equal = ordinary.clone();
    threshold_equal
        .nyphur_dispatcher
        .common_upkeep
        .spell_holder_indices[2] = 113;
    threshold_equal.entities[111].model = 2;
    threshold_equal.entities[111].position = Position {
        x: expected_axis.x.wrapping_add(0x3000),
        y: expected_axis.y,
        z: expected_axis.z,
    };
    assert_ne!(
        threshold_equal
            .nyphur_dispatcher
            .sub_13b00_axis(&threshold_equal.entities[112], &threshold_equal.entities),
        Some(expected_axis)
    );
    let mut threshold_beyond = threshold_equal.clone();
    threshold_beyond.entities[111].position.x = expected_axis.x.wrapping_add(0x3001);
    assert_eq!(
        threshold_beyond
            .nyphur_dispatcher
            .sub_13b00_axis(&threshold_beyond.entities[112], &threshold_beyond.entities),
        Some(expected_axis)
    );
    let mut unaffordable = ordinary.clone();
    unaffordable
        .nyphur_dispatcher
        .common_upkeep
        .spell_holder_indices[2] = 113;
    unaffordable.entities[113].max_mana = unaffordable.entities[112].max_mana + 1;
    assert_eq!(
        unaffordable
            .nyphur_dispatcher
            .sub_13b00_axis(&unaffordable.entities[112], &unaffordable.entities),
        None
    );
    assert_eq!(level2_nyphur_sub_13b00_grid_quadrant(0), 0);
    assert_eq!(level2_nyphur_sub_13b00_grid_quadrant(0x4000), 1);
    assert_eq!(level2_nyphur_sub_13b00_grid_quadrant(0x8000), u8::MAX);
    assert_eq!(level2_nyphur_sub_13b00_grid_quadrant(0xffff), 1);

    // The mode map is the source switch, including signed-byte default
    // fallthrough. The helper bodies below are body-level vectors on
    // production entity carriers. They select the private state byte
    // only to exercise the real dispatcher; the assertion above is the
    // separate ordinary-cursor reachability result, not a natural-route
    // claim.
    for (state_byte, helper) in [
        (3_i8, Level2NyphurDispatchHelper::Sub13100),
        (4, Level2NyphurDispatchHelper::Sub131f0),
        (11, Level2NyphurDispatchHelper::Sub133b0),
        (12, Level2NyphurDispatchHelper::Sub13270),
    ] {
        let trace = Level2NyphurDispatcher::trace_for_state(state_byte, 0, 112);
        assert_eq!(
            trace.calls.as_ref(),
            &[
                Level2NyphurDispatchCall::Sub12a70,
                Level2NyphurDispatchCall::Helper(helper),
                Level2NyphurDispatchCall::Sub12e70,
            ]
        );
        assert_eq!(trace.tail_call_count(), 1);
    }
    for state_byte in [10_i8, -1_i8, i8::MIN, i8::MAX] {
        let trace = Level2NyphurDispatcher::trace_for_state(state_byte, 0, 112);
        assert_eq!(
            trace.calls.as_ref(),
            &[
                Level2NyphurDispatchCall::Sub12a70,
                Level2NyphurDispatchCall::Sub12e70,
            ]
        );
    }

    let source = ordinary.entities[112].clone();
    let axis = level2_nyphur_axis_position(&source).expect("qualified axis carrier");
    let mut mode3 = ordinary.clone();
    mode3.nyphur_dispatcher.state_byte = 3;
    mode3.step_outer_turn(PlayerCommand::default(), 0);
    let mode3_trace = mode3
        .nyphur_dispatch_traces()
        .iter()
        .rev()
        .find(|trace| trace.source_slot == 112)
        .expect("mode-3 dispatcher trace");
    assert_eq!(mode3_trace.state_byte, 3);
    assert_eq!(mode3_trace.tail_call_count(), 1);
    assert_eq!(
        mode3_trace.calls.as_ref(),
        &[
            Level2NyphurDispatchCall::Sub12a70,
            Level2NyphurDispatchCall::Helper(Level2NyphurDispatchHelper::Sub13100),
            Level2NyphurDispatchCall::Sub12e70,
        ]
    );
    assert!(mode3.dispatch_events().iter().all(|event| !matches!(
        event,
        Level2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));
    let mode3_source = &mode3.entities[112];
    assert_eq!(
        mode3_source.source_roll(),
        i16::try_from(heading_for_positions(source.position, axis)).expect("mode-3 roll")
    );
    assert_eq!(
        mode3.nyphur_dispatcher.common_upkeep.word_0xe_14, 1,
        "sub_13100's sub_14C90 gate carries the vertical step"
    );
    let mode3_distance = level2_nyphur_source_distance_3d(source.position, axis);
    if mode3_distance <= 2_048 {
        assert_eq!(mode3.nyphur_dispatcher.common_upkeep.speed, 0);
    } else {
        assert_eq!(
            mode3.nyphur_dispatcher.common_upkeep.speed,
            source.minimum_speed()
        );
    }
    assert_eq!(mode3_source.action, source.action);

    // Let the ordinary route produce the source relation used by mode 4;
    // the helper then preserves the source's self/self heading expression
    // and uses the 256/2048 target gate.
    let mut mode4 = inspection
        .level2_reset_session()
        .expect("mode-4 production carrier");
    for _ in 0..53 {
        mode4.step_outer_turn(PlayerCommand::default(), 0);
    }
    let mode4_source = mode4.entities[112].clone();
    assert_eq!(mode4_source.source_word_96(), 70);
    mode4.nyphur_dispatcher.state_byte = 4;
    mode4.step_outer_turn(PlayerCommand::default(), 0);
    let mode4_trace = mode4
        .nyphur_dispatch_traces()
        .iter()
        .rev()
        .find(|trace| trace.source_slot == 112)
        .expect("mode-4 dispatcher trace");
    assert_eq!(mode4_trace.state_byte, 4);
    assert_eq!(mode4_trace.tail_call_count(), 1);
    assert_eq!(
        mode4_trace.calls.as_ref(),
        &[
            Level2NyphurDispatchCall::Sub12a70,
            Level2NyphurDispatchCall::Helper(Level2NyphurDispatchHelper::Sub131f0),
            Level2NyphurDispatchCall::Sub12e70,
        ]
    );
    assert!(mode4.dispatch_events().iter().all(|event| !matches!(
        event,
        Level2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));
    assert_eq!(mode4.entities[112].source_roll(), 0);
    assert_eq!(mode4.nyphur_dispatcher.common_upkeep.word_0xe_14, 1);
    assert_eq!(
        mode4.entities[112].source_word_96(),
        mode4_source.source_word_96()
    );

    // A stale sub_14C60 identity is a source-level false result, not a
    // Rust error: sub_131F0 must tail once and leave the source roll
    // untouched. The byte-0x45e fast path makes the tail observable in
    // this private state-byte dispatcher vector.
    let mut stale_identity = mode4;
    let stale_source_roll = stale_identity.entities[112].source_roll();
    let target_identity = level2_nyphur_source_identity(&stale_identity.entities[70]);
    stale_identity.entities[112].record[156..158]
        .copy_from_slice(&target_identity.wrapping_add(1).to_le_bytes());
    stale_identity.nyphur_dispatcher.common_upkeep.byte_0x45e = 3;
    stale_identity.nyphur_dispatcher.state_byte = 4;
    stale_identity.step_outer_turn(PlayerCommand::default(), 0);
    let stale_trace = stale_identity
        .nyphur_dispatch_traces()
        .iter()
        .rev()
        .find(|trace| trace.source_slot == 112)
        .expect("stale mode-4 dispatcher trace");
    assert_eq!(stale_trace.state_byte, 4);
    assert_eq!(stale_trace.tail_call_count(), 1);
    assert_eq!(
        stale_trace.calls.as_ref(),
        &[
            Level2NyphurDispatchCall::Sub12a70,
            Level2NyphurDispatchCall::Helper(Level2NyphurDispatchHelper::Sub131f0),
            Level2NyphurDispatchCall::Sub12e70,
        ]
    );
    assert!(
        stale_identity
            .dispatch_events()
            .iter()
            .all(|event| !matches!(
                event,
                Level2DispatchEvent::Error {
                    source_slot: 112,
                    ..
                }
            ))
    );
    assert_eq!(
        stale_identity.entities[112].source_roll(),
        stale_source_roll,
        "stale identity skips the source roll expression"
    );
    assert_eq!(
        stale_identity.nyphur_dispatcher.common_upkeep.byte_0x45e, 4,
        "stale identity still executes one sub_16580 tail"
    );

    let mut mode11 = inspection
        .level2_reset_session()
        .expect("mode-11 production carrier");
    mode11.nyphur_dispatcher.state_byte = 11;
    mode11.step_outer_turn(PlayerCommand::default(), 0);
    let mode11_trace = mode11
        .nyphur_dispatch_traces()
        .iter()
        .rev()
        .find(|trace| trace.source_slot == 112)
        .expect("mode-11 dispatcher trace");
    assert_eq!(mode11_trace.state_byte, 11);
    assert_eq!(mode11_trace.tail_call_count(), 1);
    assert_eq!(
        mode11_trace.calls.as_ref(),
        &[
            Level2NyphurDispatchCall::Sub12a70,
            Level2NyphurDispatchCall::Helper(Level2NyphurDispatchHelper::Sub133b0),
            Level2NyphurDispatchCall::Sub12e70,
        ]
    );
    assert!(mode11.dispatch_events().iter().all(|event| !matches!(
        event,
        Level2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));
    assert_eq!(
        mode11.nyphur_dispatcher.common_upkeep.speed,
        mode11.entities[112].minimum_speed()
    );
    assert_eq!(mode11.nyphur_dispatcher.common_upkeep.word_0xe_14, 1);

    // sub_133B0's production far-castle predicate is strict: equality
    // stays on the near side and one source unit above it enters the far
    // branch. The old 0x0640_000 literal would classify equality as far.
    let threshold_source = Position { x: 0, y: 0, z: 0 };
    let threshold_boundary = Position {
        x: 10_240,
        y: 0,
        z: 0,
    };
    let threshold_above = Position {
        x: 10_241,
        y: 0,
        z: 0,
    };
    assert!(!level2_nyphur_sub_133b0_castle_is_far(
        threshold_source,
        threshold_boundary
    ));
    assert!(level2_nyphur_sub_133b0_castle_is_far(
        threshold_source,
        threshold_above
    ));

    // In the no-castle inactive-holder fallthrough, the source writes
    // minimum speed and word_0xe_14 before the single sub_16580 call.
    // A blocked terrain probe makes that ordering observable even though
    // the helper ultimately fails closed.
    let mut mode11_order = inspection
        .level2_reset_session()
        .expect("mode-11 order carrier");
    let order_source = mode11_order.entities[112].clone();
    let blocked_x = order_source.position.x.to_be_bytes()[0];
    let blocked_y = order_source.position.y.to_be_bytes()[0].wrapping_sub(1);
    mode11_order.terrain.planes.terrain_types[terrain_index(blocked_x, blocked_y)] = 8;
    mode11_order.nyphur_dispatcher.state_byte = 11;
    mode11_order.step_outer_turn(PlayerCommand::default(), 0);
    assert!(mode11_order.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));
    assert_eq!(
        mode11_order.nyphur_dispatcher.common_upkeep.speed,
        order_source.minimum_speed()
    );
    assert_eq!(mode11_order.nyphur_dispatcher.common_upkeep.word_0xe_14, 1);

    let mut mode12 = inspection
        .level2_reset_session()
        .expect("mode-12 production carrier");
    let mode12_baseline_common = mode12
        .nyphur_common_upkeep()
        .expect("mode-12 baseline common carrier");
    assert_eq!(mode12.nyphur_dispatcher.common_upkeep.watcom_rand_state, 1);
    mode12.nyphur_dispatcher.state_byte = 12;
    mode12.step_outer_turn(PlayerCommand::default(), 0);
    let mode12_trace = mode12
        .nyphur_dispatch_traces()
        .iter()
        .rev()
        .find(|trace| trace.source_slot == 112)
        .expect("mode-12 dispatcher trace");
    assert_eq!(mode12_trace.state_byte, 12);
    assert_eq!(mode12_trace.tail_call_count(), 1);
    assert_eq!(
        mode12_trace.calls.as_ref(),
        &[
            Level2NyphurDispatchCall::Sub12a70,
            Level2NyphurDispatchCall::Helper(Level2NyphurDispatchHelper::Sub13270),
            Level2NyphurDispatchCall::Sub12e70,
        ]
    );
    assert!(mode12.dispatch_events().iter().all(|event| !matches!(
        event,
        Level2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));
    assert_eq!(
        mode12.nyphur_dispatcher.common_upkeep.watcom_rand_state,
        1_103_527_590
    );
    assert_eq!(
        mode12.nyphur_dispatcher.common_upkeep.speed,
        mode12.entities[112].minimum_speed()
    );
    assert_eq!(mode12.nyphur_dispatcher.common_upkeep.word_0xe_14, 1);
    assert_ne!(
        mode12
            .nyphur_common_upkeep()
            .expect("mode-12 stepped common carrier"),
        mode12_baseline_common
    );
    mode12.reset();
    assert_eq!(
        mode12
            .nyphur_common_upkeep()
            .expect("reset mode-12 common carrier"),
        mode12_baseline_common
    );
    assert_eq!(mode12.nyphur_dispatcher.common_upkeep.watcom_rand_state, 1);
}

#[test]
fn level_two_ai_shared_tail_owned_rows_are_exact() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 shared-tail production session");
    let source = session.entities[112].clone();
    let target = session.entities[111].clone();

    // sub_14C40/sub_14C60 use the target's source id, model, and class at
    // signed-16 width. A slot-only comparison or an unwrapped sum accepts
    // the wrong target in these vectors.
    let expected_identity = i16::from_le_bytes(
        target
            .source_id
            .wrapping_add(u16::from(target.model))
            .wrapping_add(u16::from(target.class).wrapping_shl(7))
            .to_le_bytes(),
    );
    assert_eq!(level2_nyphur_source_identity(&target), expected_identity);
    assert!(!level2_nyphur_source_identity_matches(&source, &target));
    let mut relation = source.clone();
    relation.set_source_relation(&target);
    assert_eq!(relation.source_word_96(), target.index);
    assert!(level2_nyphur_source_identity_matches(&relation, &target));
    let mut wrong_identity = target.clone();
    wrong_identity.model = wrong_identity.model.wrapping_add(1);
    assert!(!level2_nyphur_source_identity_matches(
        &relation,
        &wrong_identity
    ));

    let mut wrapped_identity = target.clone();
    wrapped_identity.source_id = u16::MAX;
    wrapped_identity.model = u8::MAX;
    wrapped_identity.class = u8::MAX;
    let wrapped_sum = u16::MAX
        .wrapping_add(u16::from(u8::MAX))
        .wrapping_add(u16::from(u8::MAX).wrapping_shl(7));
    assert_eq!(
        level2_nyphur_source_identity(&wrapped_identity),
        i16::from_le_bytes(wrapped_sum.to_le_bytes())
    );

    // sub_583B0 is non-wrapping Chebyshev X/Y, while sub_583F0 first
    // narrows each coordinate delta to signed 16-bit.
    assert_eq!(
        level2_nyphur_source_distance_xy_max(
            Position { x: 0, y: 7, z: 12 },
            Position {
                x: u16::MAX,
                y: 3,
                z: -12,
            },
        ),
        i32::from(u16::MAX)
    );
    assert_eq!(
        level2_nyphur_source_distance_3d(
            Position { x: 0, y: 0, z: 0 },
            Position {
                x: u16::MAX,
                y: 0,
                z: 0
            },
        ),
        1
    );
    let component = 32_767_i32 * 32_767_i32;
    let wrapped_3d_sum = component.wrapping_add(component).wrapping_add(component) as u32;
    assert_eq!(
        level2_nyphur_source_distance_3d(
            Position { x: 0, y: 0, z: 0 },
            Position {
                x: 32_767,
                y: 32_767,
                z: 32_767,
            },
        ),
        crate::legacy_sqrt(wrapped_3d_sum)
    );

    // sub_580E0 probes above-ground movement before applying the lower
    // clamp. Equality at the lower boundary therefore advances, while a
    // point below the ground is clamped directly.
    let mut at_ground = Position { x: 0, y: 0, z: 100 };
    level2_nyphur_sub_580e0(&mut at_ground, 100, 4, 2);
    assert_eq!(at_ground.z, 104);
    let mut at_lower_boundary = Position { x: 0, y: 0, z: 104 };
    level2_nyphur_sub_580e0(&mut at_lower_boundary, 100, 4, 2);
    assert_eq!(at_lower_boundary.z, 106);
    let mut below_ground = Position { x: 0, y: 0, z: 90 };
    level2_nyphur_sub_580e0(&mut below_ground, 100, 4, 2);
    assert_eq!(below_ground.z, 104);

    // watcominitrandnext/watcomrand retain a wrapping 32-bit state and
    // expose only the source's 15-bit result.
    let mut watcom_state = 1_u32;
    assert_eq!(*level2_nyphur_watcominitrandnext(&mut watcom_state), 1);
    assert_eq!(level2_nyphur_watcomrand(&mut watcom_state), 16_838);
    assert_eq!(watcom_state, 1_103_527_590);
    assert_eq!(level2_nyphur_watcomrand(&mut watcom_state), 5_758);
    assert_eq!(watcom_state, 2_524_885_223);

    let common = session.nyphur_dispatcher.common_upkeep.clone();
    assert!(common.sub_15e60());
    assert!(!common.sub_156f0(2, &session.entities));
    assert!(!common.sub_156f0(26, &session.entities));
    assert!(common.sub_15760(0, &session.entities).is_none());
    let class5_order = level2_nyphur_class5_bucket_order(&session.entities);
    assert_eq!(class5_order.first().copied(), Some(39));
    assert_eq!(class5_order.last().copied(), Some(106));
    // The source's strict distance arbitration selects row 70, not the
    // first class-5 slot or an arbitrary vector-order entry.
    assert_eq!(
        session
            .nyphur_dispatcher
            .sub_14530_target(&source, &session.entities),
        Some(70)
    );
    let mut active_holder_entities = session.entities.clone();
    active_holder_entities[113].active_value = 1;
    active_holder_entities[113].record[50..52].copy_from_slice(&1_i16.to_le_bytes());
    assert!(common.sub_15760(0, &active_holder_entities).is_some());

    // The named common helpers keep their exact narrow writes and failure
    // behavior even when exercised outside the live source turn.
    let mut palette_probe = common.clone();
    palette_probe.step_sub_5c800(1, 1);
    assert_eq!(palette_probe.palette_sub_mod, 2);
    palette_probe.step_sub_5c800(0, 1);
    assert_eq!(palette_probe.palette_sub_mod, 2);

    let mut timer_source = session.entities[111].clone();
    timer_source.record[58..60].fill(0);
    timer_source.apply_source_sub_5ef70();
    assert_eq!(
        i16::from_le_bytes([timer_source.record[58], timer_source.record[59]]),
        100
    );
    let mut timer_non_source = session.entities[112].clone();
    timer_non_source.record[58..60].fill(0);
    timer_non_source.apply_source_sub_5ef70();
    assert_eq!(timer_non_source.record[58..60], [0, 0]);

    let mut visibility_probe = common.clone();
    visibility_probe.owner_control_mode = 2;
    let mut flags = 0x20_u32;
    visibility_probe.step_sub_5f7e0(1, &mut flags);
    assert_eq!(flags, 0x20);
    visibility_probe.step_sub_5f7e0(0, &mut flags);
    assert_eq!(flags, 0);
    visibility_probe.owner_control_mode = 1;
    let mut holder = session.entities[114].clone();
    holder.activation_value = 3;
    let mut activation_flags = 0x320_u32;
    visibility_probe.step_sub_5f7b0(&mut holder, &mut activation_flags, 0x200);
    assert_eq!(holder.active_value, 3);
    assert_eq!(activation_flags, 0x200);

    let mut direct_holder = session.entities[114].clone();
    direct_holder.active_value = 1;
    direct_holder.record[50..52].copy_from_slice(&1_i16.to_le_bytes());
    direct_holder.byte_3c = 0;
    direct_holder.record[64] = 0;
    let mut direct_flags = 0x320_u32;
    assert!(visibility_probe.step_sub_5f660(1, &mut direct_flags, &mut direct_holder, 0,));
    assert_eq!(direct_holder.byte_3c, 1);
    assert_eq!(direct_holder.record[64], 1);
    assert_eq!(direct_flags, 0);

    let mut non_success_holder = session.entities[114].clone();
    non_success_holder.model = 4;
    non_success_holder.record[68] = 4;
    non_success_holder.active_value = 1;
    non_success_holder.record[50..52].copy_from_slice(&1_i16.to_le_bytes());
    let mut non_success_flags = 0x320_u32;
    assert!(!visibility_probe.step_sub_5f660(
        0,
        &mut non_success_flags,
        &mut non_success_holder,
        0,
    ));
    assert_eq!(non_success_holder.active_value, 7);
    assert_eq!(non_success_flags, 0x320);

    let mut failed_holder = session.entities[114].clone();
    failed_holder.active_value = 0;
    failed_holder.max_mana = 1;
    let mut failed_flags = 0x320_u32;
    visibility_probe.mana = 0;
    assert!(!visibility_probe.step_sub_5f660(0, &mut failed_flags, &mut failed_holder, 0,));
    assert_eq!(failed_flags, 0x320);
    assert_eq!(failed_holder.active_value, 0);

    // A common tail must not silently absorb a newly live selector. The
    // now-owned family-2 branch must produce its source axis before the
    // remaining fail-closed gates are considered; the still-unowned
    // model-2 branch remains a rejection here.
    let mut family_two_live = session.nyphur_dispatcher.clone();
    family_two_live.common_upkeep.spell_holder_indices[2] = 113;
    assert!(
        family_two_live
            .sub_13b00_axis(&source, &session.entities)
            .is_some()
    );
    let mut model_two_live_entities = session.entities.clone();
    model_two_live_entities[111].model = 2;
    assert!(!session.nyphur_dispatcher.owned_tail_fallthrough_is_safe(
        &source,
        &model_two_live_entities,
        &session.players
    ));

    // Baseline captures include the AI record, arena/economy stream,
    // runtime bytes, terrain digest, typed event order, and hashes. The
    // source row is discovered by the ordinary production cursor below;
    // no selector/action field is written to make it live.
    let baseline_common = session
        .nyphur_common_upkeep()
        .expect("baseline common carrier");
    assert_eq!(baseline_common.source_list_model_zero, Some(111));
    let baseline_campaign_hash = session.campaign_state_hash();
    let baseline_terrain_hash = session.terrain_hash();
    let baseline_dispatch_hash = session.dispatch_observability_hash();
    let baseline_economy_hash = session.economy.reset_hash();
    let baseline_economy_events = session.economy.events().to_vec();
    let baseline_global_rng = session.economy.global_rng;
    let baseline_runtime = session.runtime.clone();
    let baseline_entities = session.entities.clone();
    let baseline_dispatch_events = session.dispatch_events().to_vec();

    session.step_outer_turn(PlayerCommand::default(), 0);
    let first_trace = session
        .nyphur_dispatch_traces()
        .first()
        .cloned()
        .expect("ordinary source cursor emits Nyphur trace");
    assert_eq!(
        first_trace.calls.as_ref(),
        &[
            Level2NyphurDispatchCall::Sub12a70,
            Level2NyphurDispatchCall::Sub12e70,
            Level2NyphurDispatchCall::Sub12e70,
        ]
    );
    assert_eq!(first_trace.state_byte, 0);
    assert_eq!(first_trace.tail_call_count(), 2);
    assert!(session.dispatch_events().iter().all(|event| !matches!(
        event,
        Level2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));

    // The exact cadence gate is 55 at the qualified reflex value. The
    // 53rd outer turn enters the pinned post-mode tail with the live
    // class-5 mana rows; sub_14530 therefore selects row 70 and latches
    // state 13 after the common callback. A generic one-tail dispatcher
    // or slot-order mover does not reach this same state/target pair.
    for _ in 1..53 {
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert_eq!(session.nyphur_dispatch_state_byte(), 13);
    let stepped_source = &session.entities[112];
    assert_eq!(stepped_source.source_word_96(), 70);
    assert!(level2_nyphur_source_identity_matches(
        stepped_source,
        &session.entities[70]
    ));
    let final_trace = session
        .nyphur_dispatch_traces()
        .first()
        .cloned()
        .expect("due-cadence source cursor emits Nyphur trace");
    // The trace records the selector byte latched by `sub_12910` before
    // its two source-case-0 tail calls. The post-tail state is asserted
    // above on the production dispatcher itself.
    assert_eq!(final_trace.state_byte, 0);
    // The selector was latched as case 0 before the first tail changed
    // the persistent state to 13, so the source still performs both
    // case-0 tail calls on this dispatch.
    assert_eq!(final_trace.tail_call_count(), 2);
    assert_eq!(
        final_trace.calls.as_ref(),
        &[
            Level2NyphurDispatchCall::Sub12a70,
            Level2NyphurDispatchCall::Sub12e70,
            Level2NyphurDispatchCall::Sub12e70,
        ]
    );
    assert!(session.dispatch_events().iter().all(|event| !matches!(
        event,
        Level2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));

    // The next ordinary cursor dispatch must execute the latched state-13
    // helper before its shared tail.  Capture the source carriers first so
    // a trace-only or tail-only implementation cannot satisfy this test.
    let transition_source = session.entities[112].clone();
    let transition_target = session.entities[70].clone();
    let transition_common = session
        .nyphur_common_upkeep()
        .expect("state-13 transition common carrier");
    assert_eq!(transition_common.byte_0x45d, 0);
    assert_eq!(transition_common.word_0xe_14, 0);

    session.step_outer_turn(PlayerCommand::default(), 0);
    let next_trace = session
        .nyphur_dispatch_traces()
        .last()
        .cloned()
        .expect("state-13 ordinary dispatch emits Nyphur trace");
    assert_eq!(next_trace.state_byte, 13);
    assert_eq!(next_trace.tail_call_count(), 1);
    assert_eq!(
        next_trace.calls.as_ref(),
        &[
            Level2NyphurDispatchCall::Sub12a70,
            Level2NyphurDispatchCall::Helper(Level2NyphurDispatchHelper::Sub13850),
            Level2NyphurDispatchCall::Sub12e70,
        ]
    );
    assert!(session.dispatch_events().iter().all(|event| !matches!(
        event,
        Level2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));
    let next_source = &session.entities[112];
    assert_eq!(
        next_source.entity_rng, transition_source.entity_rng,
        "the far state-13 branch does not consume movement RNG"
    );
    assert_eq!(
        next_source.yaw, transition_source.yaw,
        "the far state-13 branch does not enter the yaw branch"
    );
    assert_eq!(
        next_source.position, transition_source.position,
        "the far state-13 branch does not enter the position branch"
    );
    assert_eq!(
        next_source.source_roll(),
        i16::try_from(crate::heading_for_positions(
            transition_source.position,
            transition_target.position,
        ))
        .expect("state-13 roll fits")
    );
    let next_common = session
        .nyphur_common_upkeep()
        .expect("state-13 ordinary dispatch common carrier");
    assert_eq!(next_common.speed, transition_source.minimum_speed());
    assert_eq!(next_common.word_0xe_14, 1);
    assert_eq!(next_common.byte_0x45c, transition_common.byte_0x45c);
    assert_eq!(next_common.byte_0x45d, transition_common.byte_0x45d);
    assert_eq!(next_common.byte_0x45e, transition_common.byte_0x45e);

    let stepped_common = session
        .nyphur_common_upkeep()
        .expect("stepped common carrier");
    let stepped_campaign_hash = session.campaign_state_hash();
    let stepped_terrain_hash = session.terrain_hash();
    let stepped_dispatch_hash = session.dispatch_observability_hash();
    let stepped_economy_hash = session.economy.reset_hash();
    let stepped_economy_events = session.economy.events().to_vec();
    let stepped_global_rng = session.economy.global_rng;
    let stepped_runtime = session.runtime.clone();
    let stepped_entities = session.entities.clone();
    let stepped_dispatch_events = session.dispatch_events().to_vec();
    let stepped_traces = session.nyphur_dispatch_traces().to_vec();

    session.reset();
    assert_eq!(session.nyphur_common_upkeep().unwrap(), baseline_common);
    assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);
    assert_eq!(session.terrain_hash(), baseline_terrain_hash);
    assert_eq!(
        session.dispatch_observability_hash(),
        baseline_dispatch_hash
    );
    assert_eq!(session.economy.reset_hash(), baseline_economy_hash);
    assert_eq!(session.economy.events(), baseline_economy_events.as_slice());
    assert_eq!(session.economy.global_rng, baseline_global_rng);
    assert_eq!(session.runtime, baseline_runtime);
    assert_eq!(session.entities, baseline_entities);
    assert_eq!(
        session.dispatch_events(),
        baseline_dispatch_events.as_slice()
    );
    assert!(session.nyphur_dispatch_traces().is_empty());

    for _ in 0..54 {
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert_eq!(session.nyphur_common_upkeep().unwrap(), stepped_common);
    assert_eq!(session.campaign_state_hash(), stepped_campaign_hash);
    assert_eq!(session.terrain_hash(), stepped_terrain_hash);
    assert_eq!(session.dispatch_observability_hash(), stepped_dispatch_hash);
    assert_eq!(session.economy.reset_hash(), stepped_economy_hash);
    assert_eq!(session.economy.events(), stepped_economy_events.as_slice());
    assert_eq!(session.economy.global_rng, stepped_global_rng);
    assert_eq!(session.runtime, stepped_runtime);
    assert_eq!(session.entities, stepped_entities);
    assert_eq!(
        session.dispatch_events(),
        stepped_dispatch_events.as_slice()
    );
    assert_eq!(session.nyphur_dispatch_traces(), stepped_traces.as_slice());
}

#[test]
fn level_two_ai_common_upkeep_matches_0002_0007_0009() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 common-upkeep production session");
    let baseline = session
        .nyphur_common_upkeep()
        .expect("qualified Nyphur common carrier");
    let local_player_before = session.entities[usize::from(session.state.entity_index)].clone();
    assert_eq!((baseline.player_slot, baseline.player_color_index), (1, 1));
    assert_eq!(session.runtime.local_player_slot, 0);
    assert_eq!(baseline.source_slot, 112);
    assert_eq!(baseline.castle_entity_index, 0);
    assert_eq!(baseline.phase, 113);
    assert_eq!(baseline.word_0x1a2, 0);
    assert_eq!(baseline.byte_0x154, 1);
    assert_eq!(baseline.word_0x159, 100);
    assert_eq!(baseline.life_regen, 0);
    assert_eq!(baseline.dword_0x16d, 1_999);
    assert_eq!(baseline.move_speed, 0);
    assert_eq!(baseline.word_0x242, 243);
    assert_eq!(baseline.word_0x244, 90);
    assert_eq!(baseline.word_0x246, 39);
    assert_eq!(baseline.word_0x248, 0);
    assert_eq!(baseline.word_0x24a, 79);
    assert_eq!(baseline.word_0x24c, 0);
    assert_eq!(baseline.spell_enabled[2], 4);
    assert!(
        baseline
            .spell_enabled
            .iter()
            .enumerate()
            .all(|(index, value)| { index == 2 && *value == 4 || index != 2 && *value == 0 })
    );
    assert_eq!(&baseline.spell_holder_indices[..2], &[113, 114]);
    assert!(
        baseline.spell_holder_indices[2..]
            .iter()
            .all(|value| *value == 0)
    );
    assert_eq!(baseline.spell_levels, [0; PLAYER_SPELL_COUNT]);
    for family in [5_usize, 6, 8] {
        assert_eq!(baseline.spell_holder_indices[family], 0);
        assert_eq!(baseline.spell_levels[family], 0);
        assert_eq!(baseline.spell_enabled[family], 0);
    }
    assert_eq!(baseline.owner_control_mode, 1);
    assert_eq!((baseline.yaw_0x1e_30, baseline.fov_0x22_34), (0, 0));
    assert_eq!(baseline.move_speed_flag, 0);
    let captured_array = session.shared_spells().players[1]
        .type_str_164_auxiliary()
        .expect("captured Type_str_164 array")
        .array_0x1FC_508;
    assert_eq!(baseline.array_0x1fc, captured_array);
    for index in 0..7 {
        assert_eq!(baseline.array_0x1fc[4 * index + 4], 24_607);
    }
    assert_eq!(
        u16::from_le_bytes([baseline.padding_0x23c[0], baseline.padding_0x23c[1]]),
        24_607
    );
    assert_eq!(baseline.mana_regen, 100);
    assert_eq!((baseline.mana, baseline.max_mana), (1_000, 1_000));
    assert_eq!((baseline.life, baseline.max_life), (3_085, 3_085));
    assert!(!baseline.high_upkeep);
    assert!(!baseline.map_angle_allowed);
    assert!(!baseline.cadence_eligible);
    assert_eq!(baseline.cadence_candidate, None);
    assert_eq!(baseline.return_value, 0);
    assert_eq!(baseline.source_mailbox, [0; 36]);
    assert_eq!(baseline.player_hit_frame_time, 0);
    assert_eq!(baseline.dword_0x18d, 0);
    assert_eq!(baseline.palette_count, 256);
    assert!(!baseline.palette_modification_enabled);

    let baseline_campaign_hash = session.campaign_state_hash();
    let nyphur_before = session.entities[112].clone();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let local_player_after = &session.entities[usize::from(session.state.entity_index)];
    // Common upkeep belongs to the discovered AI row, not the local
    // player projection.  This read-only public-route assertion rejects
    // a local-player shortcut while allowing the player phase callback.
    assert_eq!(local_player_after.position, local_player_before.position);
    assert_eq!(
        local_player_after.control_flags,
        local_player_before.control_flags
    );
    assert_eq!(
        local_player_after.record[12..16],
        local_player_before.record[12..16]
    );
    assert_eq!(
        local_player_after.record[140..152],
        local_player_before.record[140..152]
    );
    assert_eq!(
        local_player_after.record[98..134],
        local_player_before.record[98..134]
    );
    let stepped = session
        .nyphur_common_upkeep()
        .expect("stepped Nyphur common carrier");
    // dword_38519 is the active class-3 linked list (111 -> 112), so
    // sub_15EE0 retains 111. Vector order would incorrectly return the
    // unrelated class-15/model-0 row 113.
    assert_eq!(stepped.source_list_model_zero, Some(111));
    assert_eq!(stepped.phase, 114);
    assert_eq!(stepped.word_0x1a2, 0);
    assert_eq!(stepped.byte_0x154, 2);
    assert_eq!(stepped.word_0x159, 99);
    assert_eq!(stepped.life_regen, 6);
    assert_eq!(stepped.dword_0x16d, 1_998);
    assert_eq!(stepped.move_speed, 0);
    assert_eq!(stepped.word_0x242, 243);
    assert_eq!(stepped.word_0x244, 90);
    assert_eq!(stepped.word_0x246, 39);
    assert_eq!(stepped.spell_enabled[2], 3);
    assert_eq!(&stepped.spell_holder_indices[..2], &[113, 114]);
    assert_eq!(stepped.spell_levels, [0; PLAYER_SPELL_COUNT]);
    for family in [5_usize, 6, 8] {
        assert_eq!(stepped.spell_holder_indices[family], 0);
        assert_eq!(stepped.spell_levels[family], 0);
        assert_eq!(stepped.spell_enabled[family], 0);
    }
    assert_eq!((stepped.yaw_0x1e_30, stepped.fov_0x22_34), (0, 0));
    assert_eq!(stepped.move_speed_flag, 0);
    for index in 0..7 {
        assert_eq!(stepped.array_0x1fc[4 * index + 4], 24_607);
    }
    assert_eq!(
        u16::from_le_bytes([stepped.padding_0x23c[0], stepped.padding_0x23c[1]]),
        24_607
    );
    assert_eq!(stepped.mana_regen, 100);
    assert_eq!((stepped.mana, stepped.max_mana), (1_000, 1_000));
    assert_eq!((stepped.life, stepped.max_life), (3_085, 3_085));
    assert!(!stepped.high_upkeep);
    assert!(stepped.map_angle_allowed);
    assert!(!stepped.cadence_eligible);
    assert_eq!(stepped.cadence_candidate, None);
    assert_eq!(stepped.return_value, 1);
    assert_eq!(stepped.source_mailbox, [0; 36]);
    assert_eq!(stepped.player_hit_frame_time, 0);
    assert_eq!(stepped.dword_0x18d, 0);
    assert_eq!(stepped.palette_count, 256);
    assert!(!stepped.palette_modification_enabled);
    // sub_146C0 can only activate a family through its authored holder
    // relation.  The qualified reset has no family-5/6/8 holder, and the
    // only pinned move-speed producer (sub_38E70) is not on this Nyphur
    // source route.  The ordinary route still performs its source
    // movement/clamp, so assert the unrelated source carrier fields rather
    // than treating movement as a forbidden mutation.
    let nyphur_after = &session.entities[112];
    assert_eq!(nyphur_after.action, nyphur_before.action);
    assert_eq!(nyphur_after.control_flags, nyphur_before.control_flags);
    assert_eq!(nyphur_after.subspell_index, nyphur_before.subspell_index);
    assert_eq!(nyphur_after.byte_3b, nyphur_before.byte_3b);
    assert_eq!(nyphur_after.byte_3c, nyphur_before.byte_3c);
    assert_eq!(nyphur_after.active_value, nyphur_before.active_value);
    assert_eq!(
        nyphur_after.activation_value,
        nyphur_before.activation_value
    );
    assert_eq!(nyphur_after.record[42..66], nyphur_before.record[42..66]);
    assert_eq!(session.nyphur_dispatch_traces().len(), 1);
    assert_eq!(session.nyphur_dispatch_traces()[0].state_byte, 0);
    assert_ne!(session.campaign_state_hash(), baseline_campaign_hash);
    let stepped_campaign_hash = session.campaign_state_hash();

    session.reset();
    assert_eq!(
        session
            .nyphur_common_upkeep()
            .expect("reset Nyphur common carrier"),
        baseline
    );
    assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        session
            .nyphur_common_upkeep()
            .expect("replayed Nyphur common carrier"),
        stepped
    );
    assert_eq!(session.campaign_state_hash(), stepped_campaign_hash);

    // The source keeps the mailbox dormant for word_0x159 turns and
    // enters the common helper only after that countdown reaches zero.
    // This public-route threshold proof rejects an implementation that
    // runs the helper on the allocation/first-visit turn or that lets a
    // local-player projection stand in for the authored AI row.
    let mut threshold_session = inspection
        .level2_reset_session()
        .expect("threshold Nyphur common-upkeep production session");
    for _ in 0..101 {
        threshold_session.step_outer_turn(PlayerCommand::default(), 0);
    }
    let threshold = threshold_session
        .nyphur_common_upkeep()
        .expect("threshold Nyphur common carrier");
    assert_eq!(threshold.word_0x159, 0);
    assert_eq!(threshold.return_value, 1);
    assert_eq!(threshold.source_mailbox, [0; 36]);
    assert_eq!(threshold_session.entities[112].word_26(), 0);
    assert_eq!(threshold_session.nyphur_dispatch_traces().len(), 1);
    let threshold_hash = threshold_session.campaign_state_hash();
    threshold_session.reset();
    assert_eq!(
        threshold_session
            .nyphur_common_upkeep()
            .expect("reset threshold Nyphur common carrier"),
        baseline
    );
    for _ in 0..101 {
        threshold_session.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert_eq!(
        threshold_session
            .nyphur_common_upkeep()
            .expect("replayed threshold Nyphur common carrier"),
        threshold
    );
    assert_eq!(threshold_session.campaign_state_hash(), threshold_hash);
}

#[test]
fn level_two_ai_cast_eligibility_owned_rows_are_exact() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("qualified Nyphur production session");
    let baseline_hash = session.campaign_state_hash();

    // sub_146C0 resolves the source list identity and rejects both the
    // absent and out-of-pool carriers. sub_15730 is inclusive at the
    // max-mana boundary and rejects a one-unit shortfall.
    let holder_max = session.entities[113].max_mana;
    let mut carrier = session.nyphur_dispatcher.common_upkeep.clone();
    assert!(carrier.sub_146c0(1, &session.entities).is_some());
    assert!(carrier.sub_146c0(2, &session.entities).is_none());
    carrier.spell_holder_indices[6] = 113;
    carrier.max_mana = holder_max;
    assert!(carrier.sub_15730(6, &session.entities));
    carrier.max_mana = holder_max - 1;
    assert!(!carrier.sub_15730(6, &session.entities));
    carrier.spell_holder_indices[6] = u16::MAX;
    assert!(carrier.sub_146c0(6, &session.entities).is_none());

    // sub_15F20 applies SetSpell before checking mana. The failed result
    // must still carry the tier-0 family-6 profile into the holder.
    let mut mutation_entities = session.entities.clone();
    let before_holder = mutation_entities[113].clone();
    let mut mutation_probe = session.nyphur_dispatcher.common_upkeep.clone();
    mutation_probe.spell_holder_indices[6] = 113;
    mutation_probe.max_mana = 1_000;
    mutation_probe.mana = 0;
    assert_eq!(
        mutation_probe.step_sub_15f20(6, 0, &mut mutation_entities),
        -1
    );
    assert_ne!(mutation_entities[113], before_holder);
    assert_eq!(mutation_entities[113].subspell_index, 100);
    assert_eq!(mutation_entities[113].max_mana, 1_000);

    // The qualified Nyphur rows use the authored family-0/1 source
    // profiles, which are intentionally outside the generic family
    // 6/11/12 table. SetSpell still commits the selected tier before the
    // insufficient-mana result, so both producers retain their exact
    // source payload without borrowing a neighboring family.
    let mut fireball_profile_entities = session.entities.clone();
    let mut fireball_profile_probe = session.nyphur_dispatcher.common_upkeep.clone();
    fireball_profile_probe.spell_enabled[0] = 0;
    fireball_profile_probe.max_mana = 1_000;
    fireball_profile_probe.mana = 0;
    assert_eq!(
        fireball_profile_probe.step_sub_15f20(0, 1, &mut fireball_profile_entities),
        -1
    );
    assert_eq!(fireball_profile_entities[113].subspell_index, 160);
    assert_eq!(fireball_profile_entities[113].activation_value, 11);
    assert_eq!(fireball_profile_entities[113].byte_3b, 0);
    assert_eq!(fireball_profile_entities[113].max_mana, 250);
    assert_eq!(fireball_profile_entities[113].mana, 22);

    let mut possession_profile_entities = session.entities.clone();
    let mut possession_profile_probe = session.nyphur_dispatcher.common_upkeep.clone();
    possession_profile_probe.spell_enabled[1] = 0;
    possession_profile_probe.max_mana = 1_000;
    possession_profile_probe.mana = 0;
    assert_eq!(
        possession_profile_probe.step_sub_15f20(1, 1, &mut possession_profile_entities),
        -1
    );
    assert_eq!(possession_profile_entities[114].subspell_index, 15);
    assert_eq!(possession_profile_entities[114].activation_value, 41);
    assert_eq!(possession_profile_entities[114].byte_3b, 1);
    assert_eq!(possession_profile_entities[114].max_mana, 250);
    assert_eq!(possession_profile_entities[114].mana, 6);

    // sub_15790's optional family-8 branch resolves the CURRENT TARGET's
    // own player/wizard carrier by target entity index, not the target's
    // 0x94 relation and not this AI's common-upkeep holder table. A
    // class-5 target has no such carrier and must not consume RNG. A
    // genuine player target with its own active holder consumes exactly
    // one source draw. sub_15910 has the distinct sentinel order but no
    // such draw.
    let mut rng_15790 = session.nyphur_dispatcher.common_upkeep.clone();
    let mut rng_15910 = rng_15790.clone();
    rng_15790.word_0x244 = 255;
    let mut rng_entities = session.entities.clone();
    rng_entities[113].active_value = 1;
    rng_entities[113].record[50..52].copy_from_slice(&1_i16.to_le_bytes());
    assert!(rng_entities[70].active);
    let player_zero_slot = session
        .players
        .iter()
        .position(|player| player.slot == 0)
        .expect("player-zero row");
    let player_zero_entity_index = session.players[player_zero_slot].entity_index;
    // Keep a misleading 0x94 relation to player zero: the target-owner
    // carrier must come from target entity 70 itself, not this relation.
    rng_entities[70].record[152..154].copy_from_slice(&player_zero_entity_index.to_le_bytes());
    let mut player_zero_active_players = session.players.clone();
    player_zero_active_players[player_zero_slot].enabled_spell_holders[8] = 113;
    let rng_before = rng_15790.watcom_rand_state;
    // Family zero is the pinned zero-terminated row: the source's
    // zero-valued failure result is therefore indistinguishable from a
    // family-zero match, even when its authored producer profile is not
    // admitted on this target.
    // Class-5 target 70 has no own player/wizard carrier. Player zero's
    // active family-8 holder and the misleading 0x94 relation are
    // irrelevant, so the target probe must not draw.
    assert_eq!(
        rng_15790.step_sub_15790(70, &mut rng_entities, &player_zero_active_players),
        0
    );
    assert_eq!(rng_15790.watcom_rand_state, rng_before);

    let player_target_slot = player_zero_entity_index;
    let mut player_target_entities = session.entities.clone();
    player_target_entities[usize::from(player_target_slot)].record[152..154]
        .copy_from_slice(&0_u16.to_le_bytes());
    player_target_entities[113].active_value = 1;
    player_target_entities[113].record[50..52].copy_from_slice(&1_i16.to_le_bytes());
    let mut player_target_players = session.players.clone();
    player_target_players[player_zero_slot].enabled_spell_holders[8] = 113;
    let player_target_rng_before = rng_15790.watcom_rand_state;
    let mut expected_player_target_rng = rng_15790.clone();
    expected_player_target_rng.step_watcomrand();
    // A genuine player target owns the active family-8 carrier. Its 0x94
    // relation is deliberately zero and the AI's own holder table remains
    // cold, so exactly the target-owned source draw must occur.
    assert_eq!(
        rng_15790.step_sub_15790(
            player_target_slot,
            &mut player_target_entities,
            &player_target_players,
        ),
        0
    );
    assert_ne!(rng_15790.watcom_rand_state, player_target_rng_before);
    assert_eq!(
        rng_15790.watcom_rand_state,
        expected_player_target_rng.watcom_rand_state
    );

    let rng_15910_before = rng_15910.watcom_rand_state;
    let mut rng_entities_without_optional = session.entities.clone();
    assert_eq!(
        rng_15910.step_sub_15910(&mut rng_entities_without_optional),
        0
    );
    assert_eq!(rng_15910.watcom_rand_state, rng_15910_before);

    // Both source helpers yield 250 for nonnegative max-mana 1000. Their
    // negative-input source expressions remain distinct, so retain both
    // narrow helpers: both source expressions yield -250 for -1000.
    assert_eq!(Level2NyphurCommonUpkeep::sub_15790_mana_quarter(1_000), 250);
    assert_eq!(Level2NyphurCommonUpkeep::sub_15910_mana_quarter(1_000), 250);
    assert_eq!(
        Level2NyphurCommonUpkeep::sub_15790_mana_quarter(-1_000),
        -250
    );
    assert_eq!(
        Level2NyphurCommonUpkeep::sub_15910_mana_quarter(-1_000),
        -250
    );
    let mut latch_falsifier_15790 = session.nyphur_dispatcher.common_upkeep.clone();
    latch_falsifier_15790.max_mana = 1_000;
    latch_falsifier_15790.mana = 249;
    latch_falsifier_15790.word_0x1a4 = 0;
    assert!(!latch_falsifier_15790.update_spell_mana_latch_15790());
    assert_eq!(latch_falsifier_15790.word_0x1a4, 1);
    let mut latch_falsifier_15910 = session.nyphur_dispatcher.common_upkeep.clone();
    latch_falsifier_15910.max_mana = 1_000;
    latch_falsifier_15910.mana = 249;
    latch_falsifier_15910.word_0x1a4 = 0;
    assert!(!latch_falsifier_15910.update_spell_mana_latch_15910());
    assert_eq!(latch_falsifier_15910.word_0x1a4, 1);

    // The 15790 latch clears at the inclusive recovery boundary after
    // its +6000 clamp reaches max/2.
    let mut latch = session.nyphur_dispatcher.common_upkeep.clone();
    latch.max_mana = 1_000;
    latch.mana = 248;
    assert!(!latch.update_spell_mana_latch_15790());
    assert_eq!(latch.word_0x1a4, 1);
    latch.mana = 249;
    assert!(!latch.update_spell_mana_latch_15790());
    assert_eq!(latch.word_0x1a4, 1);
    latch.mana = 500;
    assert!(latch.update_spell_mana_latch_15790());
    assert_eq!(latch.word_0x1a4, 0);

    // sub_13DC0 is before cadence: a damaged source reaches the valid
    // Castle relation even on a non-cadence phase. At equality it is a
    // strict negative and the cadence-gated tail remains untouched.
    let mut damaged = session.clone();
    damaged.nyphur_dispatcher.common_upkeep.castle_entity_index = 70;
    damaged.nyphur_dispatcher.common_upkeep.life =
        damaged.nyphur_dispatcher.common_upkeep.max_life / 2 - 1;
    damaged.entities[112].phase = 1;
    damaged
        .nyphur_dispatcher
        .step_sub_12e70(112, &mut damaged.entities, &damaged.players)
        .expect("pre-cadence damaged Castle branch");
    assert_eq!(damaged.nyphur_dispatcher.state_byte, 11);
    assert_eq!(damaged.entities[112].source_word_96(), 70);
    assert!(level2_nyphur_source_identity_matches(
        &damaged.entities[112],
        &damaged.entities[70]
    ));

    let mut at_half = session.clone();
    at_half.nyphur_dispatcher.common_upkeep.castle_entity_index = 70;
    at_half.nyphur_dispatcher.common_upkeep.life =
        at_half.nyphur_dispatcher.common_upkeep.max_life / 2;
    at_half.entities[112].phase = 1;
    let source_before = at_half.entities[112].clone();
    at_half
        .nyphur_dispatcher
        .step_sub_12e70(112, &mut at_half.entities, &at_half.players)
        .expect("cadence rejects full-health Castle branch");
    assert_eq!(at_half.nyphur_dispatcher.state_byte, 0);
    assert_eq!(
        at_half.entities[112].source_word_96(),
        source_before.source_word_96()
    );

    // The owned entry gates are exercised without fabricating either of
    // the unowned dependent carriers: sub_13C50 needs family 2 and then
    // fails closed at sub_155E0; sub_13CE0 reaches its family-1 gate but
    // fails closed at sub_148E0.
    let mut castle = session.entities[70].clone();
    castle.action = 4;
    castle.activation_value = 0;
    let source = session.entities[112].clone();
    let mut branch_probe = session.nyphur_dispatcher.common_upkeep.clone();
    branch_probe.castle_entity_index = 70;
    branch_probe.max_mana = 1_000;
    assert!(!branch_probe.step_sub_13c50(&source, Some(&castle), &session.entities));
    assert!(
        branch_probe
            .step_sub_13ce0(&source, &session.entities, &session.players)
            .is_none()
    );

    // The qualified constructor has no Castle, and its ordinary package
    // route currently reaches the existing state-13 selector. No honest
    // ordinary positive for sub_14630 is claimed in this focused vector;
    // that reachability remains open pending its owning upstream seam.

    // The new latch is part of the campaign digest and reset baseline.
    session.nyphur_dispatcher.common_upkeep.word_0x1a4 = 1;
    assert_ne!(session.campaign_state_hash(), baseline_hash);
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}
