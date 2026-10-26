#[test]
fn level_two_local_player_unboosted_mana_upkeep_follows_entity_pass_count_and_reset() {
    let mut session = level_two_campaign_session();
    let baseline_campaign_hash = session.campaign_state_hash();
    let seed = |session: &mut Mc2Simulation, mana: i32, mana_regen: i32| {
        // The numeric player owns the next source visit; presentation copies
        // alone are overwritten when that authoritative carrier is read.
        let player_slot = usize::from(session.state.entity_index);
        session.economy.entities[player_slot].set_mana(mana);
        session.economy.entities[player_slot].set_mana_regen(mana_regen);
        let shared = &mut session.shared_spells;
        shared.player_entity.mana = mana;
        shared.player_entity.record[148..152].copy_from_slice(&mana.to_le_bytes());
        shared.player_entity.mana_regen = mana_regen;
        shared.player_entity.record[140..144].copy_from_slice(&mana_regen.to_le_bytes());
        shared.caster.mana = mana;
        shared.player_mana = mana;
        shared.player_entities[0] = shared.player_entity.clone();
    };

    // Unknown outer-loop speed bytes skip the entity update entirely.
    seed(&mut session, 500, 100);
    session.step_outer_turn(PlayerCommand::default(), u8::MAX);
    assert_eq!(session.shared_spells().player_mana(), 500);

    // The real Level2 production route applies one AddPlayer pass after
    // the flight body: old mana + old regen, then the next outdoor rate.
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.shared_spells().player_mana(), 600);
    assert_eq!(session.shared_spells().player_entity.mana_regen, 100);
    assert_eq!(
        i32::from_le_bytes(
            session.shared_spells().player_entity.record_bytes()[140..144]
                .try_into()
                .expect("mana regen field")
        ),
        100
    );

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);
    seed(&mut session, 500, 100);
    session.step_outer_turn(PlayerCommand::default(), 1);
    assert_eq!(session.shared_spells().player_mana(), 900);

    session.reset();
    seed(&mut session, 500, 100);
    session.step_outer_turn(PlayerCommand::default(), 2);
    assert_eq!(session.shared_spells().player_mana(), 1_000);
    assert_eq!(session.shared_spells().player_entity.mana, 1_000);

    // A negative-regeneration vector is the source post-active-handler
    // prestate (`sub_68DE0`), not a claim that the bounded activation seam
    // itself owns that later callback.
    session.reset();
    seed(&mut session, 1_000, -1_000);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.shared_spells().player_mana(), 0);
    assert_eq!(session.shared_spells().player_entity.mana_regen, 100);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.shared_spells().player_mana(), 100);
}

#[test]
fn level_two_shared_spells_bind_activation_mana_events_and_reset() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut first = inspection
        .level2_reset_session()
        .expect("Level 2 reset session");
    let second = inspection
        .level2_reset_session()
        .expect("isolated Level 2 reset session");
    assert_eq!(
        first
            .shared_spells()
            .pickups()
            .iter()
            .map(|entity| entity.index)
            .collect::<Vec<_>>(),
        vec![38, 75]
    );
    assert_eq!(
        first
            .shared_spells()
            .holders()
            .iter()
            .map(|entity| entity.index)
            .collect::<Vec<_>>(),
        vec![113, 114]
    );
    assert_eq!(first.shared_spells().player_mana(), 1_000);
    assert!(first.shared_spells_mut().activate_fireball(256, true));
    assert_eq!(first.shared_spells().fireball_holder().active_value, 5);
    assert_eq!(first.shared_spells().activation_events(), &[]);
    assert!(first.shared_spells_mut().activate_possession(512, true));
    assert_eq!(first.shared_spells().possession_holder().active_value, 3);
    first.shared_spells_mut().reset();
    assert!(first.shared_spells_mut().activate_speed(256, true));
    assert_eq!(
        first.shared_spells().speed_holder().active_value,
        i16::try_from(SPEED_SPELL_ACTIVATION_TICKS).expect("speed timer fits i16")
    );
    first.shared_spells_mut().reset();
    assert_eq!(second.shared_spells().fireball_holder().active_value, 0);
    first.shared_spells_mut().caster.mana = 0;
    first.shared_spells_mut().fireball.active_value = 0;
    assert!(!first.shared_spells_mut().activate_fireball(256, true));
    assert!(!first.shared_spells().activation_events().is_empty());
    first.shared_spells_mut().reset();
    assert_eq!(first.shared_spells().fireball_holder().active_value, 0);
    assert_eq!(first.shared_spells().player_mana(), 1_000);
    assert_eq!(first.shared_spells(), second.shared_spells());
}

#[test]
fn level_two_speed_pickup_converts_in_source_order_and_rejects_excluded_family() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut first = inspection
        .level2_reset_session()
        .expect("Level 2 reset session");
    let second = inspection
        .level2_reset_session()
        .expect("isolated Level 2 reset session");
    let pickup_position = first.shared_spells().pickups()[1].position;
    let baseline_hash = first.shared_spells().reset_hash();

    let step = first
        .shared_spells_mut()
        .step_speed_pickup(111, pickup_position)
        .expect("Speed pickup source dispatch");
    assert!(step.collected);
    assert_eq!(step.source_slot, 75);
    assert_eq!(step.previous_action, 10);
    assert_eq!(step.action, 9);
    assert_eq!(step.phase, step.previous_phase.wrapping_add(1));
    assert_eq!(first.shared_spells().pickups()[1].action, 9);
    let collected_pickup = &first.shared_spells().pickups()[1];
    assert_eq!(
        u16::from_le_bytes([
            collected_pickup.record_bytes()[44],
            collected_pickup.record_bytes()[45]
        ]),
        111
    );
    assert_ne!(collected_pickup.control_flags & 1, 0);
    assert_eq!(collected_pickup.record_bytes()[19] & 2, 0);
    assert_eq!(
        i16::from_le_bytes([
            collected_pickup.record_bytes()[58],
            collected_pickup.record_bytes()[59]
        ]),
        64
    );
    assert_eq!(first.shared_spells().speed_holder().entity_index, 75);
    assert_eq!(
        first.shared_spells().speed_pickup_state(),
        Level2SpeedPickupState {
            owner_entity_index: 111,
            collected: true,
            visible: false,
            state_timer: 64,
        }
    );
    assert_ne!(first.shared_spells().reset_hash(), baseline_hash);
    assert_eq!(
        first.shared_spells().pickup_events(),
        &[Level2SharedSpellEvent::SpellCollected {
            source_slot: 75,
            player_entity_index: 111,
            family: SPEED_SPELL_FAMILY,
        }]
    );
    assert!(matches!(
        first.shared_spells().activation_events(),
        [FlightEvent::PrepareSound {
            sound_id: 18,
            entity_index: 111,
            player_index: 0,
            ..
        }]
    ));

    // A second source turn sees the converted holder, so no duplicate
    // acquisition or second sound is emitted.
    let second_step = first
        .shared_spells_mut()
        .step_speed_pickup(111, pickup_position)
        .expect("converted holder source dispatch");
    assert!(!second_step.collected);
    assert!(first.shared_spells().activation_events().is_empty());

    first.shared_spells_mut().reset();
    let far_position = Position {
        x: pickup_position.x.wrapping_add(0x2000),
        y: pickup_position.y,
        z: pickup_position.z,
    };
    let non_overlap = first
        .shared_spells_mut()
        .step_speed_pickup(111, far_position)
        .expect("non-overlapping source dispatch");
    assert!(!non_overlap.collected);
    assert_eq!(first.shared_spells().pickups()[1].action, 10);
    first.shared_spells_mut().reset();
    first.shared_spells_mut().pickups[1].phase = 77;
    first.shared_spells_mut().pickups[1].record[66] = 77;
    let direct_update = first
        .shared_spells_mut()
        .step_speed_pickup(111, pickup_position)
        .expect("non-cadence source dispatch");
    assert!(!direct_update.collected);
    assert_eq!(first.shared_spells().pickups()[1].action, 10);

    // The model-12 row is a later Level 2 family, not a Level 1 Speed
    // pickup; this bounded adapter never silently coerces it.
    let mut excluded = inspection
        .level2_reset_session()
        .expect("Level 2 reset session");
    let slot38 = excluded.shared_spells().pickups()[0].position;
    let excluded_step = excluded
        .shared_spells_mut()
        .step_shared_pickup(38, 111, slot38)
        .expect("Speed source dispatch remains well formed");
    assert!(excluded_step.is_none());
    assert_eq!(excluded.shared_spells().pickups()[0].action, 37);

    first.shared_spells_mut().reset();
    assert_eq!(first.shared_spells(), second.shared_spells());
    assert!(
        first
            .shared_spells_mut()
            .step_shared_pickup(38, 112, pickup_position)
            .is_err()
    );
    assert!(
        first
            .shared_spells_mut()
            .step_speed_pickup(112, pickup_position)
            .is_err()
    );
}

#[test]
fn level_two_generic_spell_acquisition_is_reset_hashed_and_source_reachable() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 acquisition production session");
    let baseline_hash = session.shared_spells().reset_hash();
    assert_eq!(
        session
            .shared_spells()
            .acquisition_states()
            .iter()
            .map(|state| (state.family, state.source_slot, state.source_action))
            .collect::<Vec<_>>(),
        vec![(6, 155, 19), (11, 156, 34), (12, 38, 37), (3, 75, 10)]
    );

    // Row 38 is the live frame-zero family-12 pickup.  Its generic
    // source transition writes the holder action/relation and the local
    // player's first hand, then becomes part of the reset digest.
    let pickup_position = session.shared_spells().pickups()[0].position;
    session.shared_spells_mut().pickups[0].phase = 0;
    session.shared_spells_mut().pickups[0].record[66] = 0;
    let outcome = session
        .shared_spells_mut()
        .step_level2_spell_acquisition(12, 111, pickup_position)
        .expect("family-12 source acquisition")
        .expect("family-12 pickup overlaps player");
    assert_eq!(outcome.holder_action, 36);
    assert_eq!(outcome.parent_entity_index, 111);
    assert_eq!(outcome.presentation_timer, 64);
    assert_eq!(session.shared_spells().pickups()[0].action, 36);
    assert_eq!(
        session
            .shared_spells()
            .acquisition_states()
            .iter()
            .find(|state| state.family == 12)
            .expect("family-12 state")
            .holder_relation,
        38
    );
    assert_ne!(session.shared_spells().reset_hash(), baseline_hash);
    assert!(matches!(
        session.shared_spells().pickup_events(),
        [Level2SharedSpellEvent::SpellAcquired {
            source_slot: 38,
            player_entity_index: 111,
            family: 12,
            holder_action: 36,
        }]
    ));

    // Authored rows 155/156 are not live reset entities.  A direct child
    // projection remains bounded until the production stage constructor
    // installs their collision bodies.
    assert!(
        session
            .shared_spells_mut()
            .step_level2_spell_acquisition(6, 111, Position::default())
            .expect("family-6 authored row remains bounded")
            .is_none()
    );
    assert!(
        session
            .shared_spells_mut()
            .step_level2_spell_acquisition(11, 111, Position::default())
            .expect("family-11 authored row remains bounded")
            .is_none()
    );

    session.reset();
    assert_eq!(session.shared_spells().reset_hash(), baseline_hash);
    assert_eq!(session.shared_spells().pickups()[0].action, 37);
    assert!(session.shared_spells().pickup_events().is_empty());
    assert!(
        session
            .shared_spells()
            .acquisition_states()
            .iter()
            .all(|state| state.holder_relation == 0)
    );

    // SetSpell_6D5E0 derives the hand from the source return-side and the
    // owner's live selectors.  Keep this oracle source-shaped: direct
    // 54A50 initialization means the first acquisition starts from 0/1,
    // rather than from an empty pre-init hand.
    let expected_hand = |return_side: i16, left: i16, right: i16| {
        if return_side == 2 || (return_side == 0 && (left < 0 || right >= 0)) {
            crate::spell_activation::SpellHand::Left
        } else {
            crate::spell_activation::SpellHand::Right
        }
    };
    let expected_first_hand = {
        let state = session
            .shared_spells()
            .acquisition_states()
            .iter()
            .find(|state| state.family == 12)
            .expect("family-12 acquisition state");
        let player = &session.shared_spells().players[0];
        expected_hand(
            state.return_side,
            player.left_spell_slot_raw,
            player.right_spell_slot_raw,
        )
    };
    let first = session
        .shared_spells_mut()
        .acquire_level2_spell_holder(12, PhysicalSpellActionRole::OrdinaryPickup, true)
        .expect("family-12 generic acquisition");
    let expected_second_hand = {
        let state = session
            .shared_spells()
            .acquisition_states()
            .iter()
            .find(|state| state.family == 6)
            .expect("family-6 acquisition state");
        let player = &session.shared_spells().players[0];
        expected_hand(
            state.return_side,
            player.left_spell_slot_raw,
            player.right_spell_slot_raw,
        )
    };
    let second = session
        .shared_spells_mut()
        .acquire_level2_spell_holder(6, PhysicalSpellActionRole::OrdinaryPickup, true)
        .expect("family-6 generic acquisition");
    assert_eq!(first.hand, expected_first_hand);
    assert_eq!(second.hand, expected_second_hand);
}

#[test]
fn level_two_spell_holder_source_prefix_matches_life_draw_and_return_hand() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 acquisition production session");
    let source_position = session.shared_spells().pickups()[0].position;
    let family_index = session
        .shared_spells()
        .acquisition_states()
        .iter()
        .position(|state| state.family == 12)
        .expect("family-12 acquisition state");

    // `sub_68FF0` consumes positive life before it ever scans players,
    // disabling drawing on the final decrement and only admitting the
    // source on the subsequent zero-life pulse.
    session.shared_spells_mut().acquisitions[family_index].source_life = 2;
    assert!(
        session
            .shared_spells_mut()
            .step_level2_spell_acquisition(12, 111, source_position)
            .expect("life pulse 1")
            .is_none()
    );
    assert_eq!(
        session.shared_spells().acquisition_states()[family_index].source_life,
        1
    );
    assert_eq!(session.shared_spells().pickups()[0].life, 1);
    assert!(
        session
            .shared_spells_mut()
            .step_level2_spell_acquisition(12, 111, source_position)
            .expect("life pulse 2")
            .is_none()
    );
    assert_eq!(
        session.shared_spells().acquisition_states()[family_index].source_life,
        0
    );
    assert_eq!(session.shared_spells().pickups()[0].life, 0);
    assert!(session.shared_spells().pickups()[0].drawing_disabled());
    assert!(!session.shared_spells().acquisition_states()[family_index].source_draw_enabled);
    assert!(session.shared_spells().pickups()[0].drawing_disabled());
    assert_eq!(
        session.shared_spells().pickups()[0].record_bytes()[17] & 4,
        4
    );

    // The source's non-zero word_0x4A_74 return-side flag wins even with
    // both hands occupied (2 means left); the source clears it and writes
    // SetSpell's selected tier before exposing the holder.
    session.shared_spells_mut().players[0].left_spell_slot_raw = 4;
    session.shared_spells_mut().players[0].right_spell_slot_raw = 5;
    session.shared_spells_mut().pickups[0].record[78..80].copy_from_slice(&2_i16.to_le_bytes());
    let outcome = session
        .shared_spells_mut()
        .acquire_level2_spell_holder(12, PhysicalSpellActionRole::OrdinaryPickup, true)
        .expect("return-side holder acquisition");
    assert_eq!(outcome.hand, crate::spell_activation::SpellHand::Left);
    assert_eq!(
        session.shared_spells().pickups()[0].record_bytes()[78..80],
        [0, 0]
    );
    assert_eq!(
        session.shared_spells().pickups()[0].record_bytes()[74],
        session.shared_spells().acquisition_states()[family_index]
            .selected_tier
            .to_le_bytes()[0]
    );

    // The independent player-colour pass is observable even when the
    // collision owner is absent: a matching LevelIndex colour marks the
    // pickup byte0 flag before the source returns without acquisition.
    session.shared_spells_mut().reset();
    session.shared_spells_mut().pickups[0].phase = 0;
    session.shared_spells_mut().pickups[0].record[66] = 0;
    session.shared_spells_mut().players[0].enabled_spell_holders[12] = 999;
    let far_position = Position {
        x: source_position.x.wrapping_add(0x2000),
        y: source_position.y,
        z: source_position.z,
    };
    assert!(
        session
            .shared_spells_mut()
            .step_level2_spell_acquisition(12, 111, far_position)
            .expect("colour-only source pass")
            .is_none()
    );
    assert_ne!(session.shared_spells().pickups()[0].control_flags & 1, 0);

    // The source duplicate gate is a per-candidate continue, not a
    // terminal rejection: an overlapping local player who already owns
    // this family is skipped and the next overlapping player can claim
    // the same source row.
    session.shared_spells_mut().reset();
    session.shared_spells_mut().pickups[0].phase = 0;
    session.shared_spells_mut().pickups[0].record[66] = 0;
    session.shared_spells_mut().players[0].enabled_spell_holders[12] = 999;
    session.shared_spells_mut().player_entities[0].active = true;
    session.shared_spells_mut().player_entities[0].model = 0;
    session.shared_spells_mut().player_entities[0].life = 1;
    session.shared_spells_mut().player_entities[0].position = source_position;
    session.shared_spells_mut().player_entities[0].record[80..82]
        .copy_from_slice(&source_position.x.to_le_bytes());
    session.shared_spells_mut().player_entities[0].record[82..84]
        .copy_from_slice(&source_position.y.to_le_bytes());
    session.shared_spells_mut().player_entities[0].record[84..86]
        .copy_from_slice(&source_position.z.to_le_bytes());
    session.shared_spells_mut().player_entities[1].active = true;
    session.shared_spells_mut().player_entities[1].model = 0;
    session.shared_spells_mut().player_entities[1].life = 1;
    session.shared_spells_mut().player_entities[1].position = source_position;
    session.shared_spells_mut().player_entities[1].record[80..82]
        .copy_from_slice(&source_position.x.to_le_bytes());
    session.shared_spells_mut().player_entities[1].record[82..84]
        .copy_from_slice(&source_position.y.to_le_bytes());
    session.shared_spells_mut().player_entities[1].record[84..86]
        .copy_from_slice(&source_position.z.to_le_bytes());
    let duplicate_then_remote = session
        .shared_spells_mut()
        .step_level2_spell_acquisition(12, 111, source_position)
        .expect("duplicate candidate source pass")
        .expect("next overlapping candidate acquires");
    assert_eq!(duplicate_then_remote.parent_entity_index, 112);
    assert_eq!(
        session.shared_spells().players[0].enabled_spell_holders[12],
        999
    );
    assert_eq!(
        session.shared_spells().players[1].enabled_spell_holders[12],
        38
    );
    assert!(matches!(
        session.shared_spells().pickup_events(),
        [Level2SharedSpellEvent::SpellAcquired {
            player_entity_index: 112,
            source_slot: 38,
            family: 12,
            ..
        }]
    ));

    // A second model-0 player at the source wins ownership when local
    // player 0 is far away; the event and SpellEnabled relation carry
    // that overlapping candidate instead of hard-coding entity 111.
    session.shared_spells_mut().reset();
    session.shared_spells_mut().pickups[0].phase = 0;
    session.shared_spells_mut().pickups[0].record[66] = 0;
    session.shared_spells_mut().player_entities[1].active = true;
    session.shared_spells_mut().player_entities[1].model = 0;
    session.shared_spells_mut().player_entities[1].record[2] = 1;
    session.shared_spells_mut().player_entities[1].record[68] = 0;
    session.shared_spells_mut().player_entities[1].position = source_position;
    session.shared_spells_mut().player_entities[1].record[80..82]
        .copy_from_slice(&source_position.x.to_le_bytes());
    session.shared_spells_mut().player_entities[1].record[82..84]
        .copy_from_slice(&source_position.y.to_le_bytes());
    session.shared_spells_mut().player_entities[1].record[84..86]
        .copy_from_slice(&source_position.z.to_le_bytes());
    let far_position = Position {
        x: source_position.x.wrapping_add(0x2000),
        y: source_position.y,
        z: source_position.z,
    };
    let remote = session
        .shared_spells_mut()
        .step_level2_spell_acquisition(12, 111, far_position)
        .expect("remote owner source pass")
        .expect("remote model-0 candidate overlaps");
    assert_eq!(remote.parent_entity_index, 112);
    assert_eq!(
        session.shared_spells().players[1].enabled_spell_holders[12],
        38
    );
    assert_eq!(
        session.shared_spells().players[0].enabled_spell_holders[12],
        0
    );
    assert!(matches!(
        session.shared_spells().pickup_events(),
        [Level2SharedSpellEvent::SpellAcquired {
            player_entity_index: 112,
            source_slot: 38,
            family: 12,
            ..
        }]
    ));
}

#[test]
fn level_two_setspell_projection_and_nonlocal_acquisition_are_isolated() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 acquisition production session");

    let family_index = session
        .shared_spells()
        .acquisition_states()
        .iter()
        .position(|state| state.family == 12)
        .expect("family-12 acquisition state");
    let source = &session.shared_spells().pickups()[0];
    let source_state = &session.shared_spells().acquisition_states()[family_index];
    assert_eq!(source.subspell_index, source_state.subspell_index);
    assert_eq!(
        u16::from_le_bytes(source.record_bytes()[52..54].try_into().unwrap()),
        source_state.word_0x30
    );
    assert_eq!(source.byte_3b, source_state.byte_0x3b);
    assert_eq!(source.byte_3c, source_state.byte_0x3c);
    assert_eq!(source.font_type_index, source_state.font_type_index);
    assert_eq!(source.mana_regen, source_state.mana_regen);
    assert_eq!(source.max_mana, source_state.max_mana);
    assert_eq!(source.mana, source_state.mana);

    // `sub_68FF0` updates only the chosen hand's spell/subspell pair and
    // leaves the player's current menu selection untouched. Seed a
    // non-default selection so a pickup cannot pass by retaining the
    // frame-zero zeros accidentally.
    session.shared_spells_mut().players[0].selected_spell_ui_index = 24;
    session.shared_spells_mut().players[0].selected_subspell_index = 2;
    session.shared_spells_mut().players[0].record[561] = 24;
    session.shared_spells_mut().players[0].record[562] = 2;
    let selected_menu_before = (
        session.shared_spells().players[0].selected_spell_ui_index,
        session.shared_spells().players[0].selected_subspell_index,
        session.shared_spells().players[0].record_bytes()[561],
        session.shared_spells().players[0].record_bytes()[562],
    );

    // Change the owner's selected tier after reset. SetSpell reads the
    // current array_0x437 value and must derive the holder payload from
    // the pinned family-12 tier-1 row, not copy the tier-0 pickup bytes.
    session.shared_spells_mut().players[0].family_selected_tiers[12] = 1;
    session.shared_spells_mut().players[0].record[528 + 12] = 1;
    let local_player_before = session.shared_spells().local_player.record_bytes().to_vec();
    let local_entity_before = session
        .shared_spells()
        .player_entity
        .record_bytes()
        .to_vec();
    let outcome = session
        .shared_spells_mut()
        .acquire_level2_spell_holder_for_player(
            12,
            PhysicalSpellActionRole::OrdinaryPickup,
            true,
            0,
        )
        .expect("local SetSpell projection");
    assert_eq!(outcome.parent_entity_index, 111);
    assert!(matches!(
        session.shared_spells().activation_events(),
        [FlightEvent::PrepareSound {
            entity_index: 111,
            player_index: -1,
            sound_id: 18,
            ..
        }]
    ));
    let pickup = &session.shared_spells().pickups()[0];
    assert_eq!(pickup.record_bytes()[46..48], [0, 0]);
    assert_eq!(pickup.record_bytes()[52..54], [0x05, 0x01]);
    assert_eq!(pickup.record_bytes()[63], 1);
    assert_eq!(pickup.record_bytes()[64], 0);
    assert_eq!(pickup.record_bytes()[65], 0);
    assert_eq!(pickup.record_bytes()[74], 1);
    assert_eq!(
        session.shared_spells().players[0].record_bytes()[476 + 12],
        1,
        "sub_68FF0 marks the source array_0x403 collected byte"
    );
    assert_eq!(
        session.shared_spells().players[0].record_bytes()[398 + 12],
        0,
        "acquisition must not rewrite the array_0x3B5 hand-side cache"
    );
    assert_eq!(
        session.shared_spells().players[0].record_bytes()[424 + 12],
        0,
        "acquisition must not rewrite the array_0x3CF blocked-spell byte"
    );
    assert_eq!(pickup.record_bytes()[140..144], 10_000_i32.to_le_bytes());
    assert_eq!(pickup.record_bytes()[144..148], 2_400_i32.to_le_bytes());
    assert_eq!(pickup.record_bytes()[148..152], 9_i32.to_le_bytes());
    let state = &session.shared_spells().acquisition_states()[family_index];
    assert_eq!(state.selected_tier, 1);
    assert_eq!(state.subspell_index, 0);
    assert_eq!(state.word_0x30, 261);
    assert_eq!(state.mana_regen, 10_000);
    assert_eq!(state.max_mana, 2_400);
    assert_eq!(state.mana, 9);
    assert_eq!(
        (
            session.shared_spells().players[0].selected_spell_ui_index,
            session.shared_spells().players[0].selected_subspell_index,
            session.shared_spells().players[0].record_bytes()[561],
            session.shared_spells().players[0].record_bytes()[562],
        ),
        selected_menu_before
    );
    assert!(!pickup.drawing_disabled());
    assert_eq!(pickup.record_bytes()[17] & 4, 0);
    assert_eq!(pickup.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);

    session.shared_spells_mut().reset();
    assert_eq!(
        session.shared_spells().local_player.record_bytes(),
        local_player_before
    );
    assert_eq!(
        session.shared_spells().player_entity.record_bytes(),
        local_entity_before
    );

    // A direct remote-owner call is allowed to update the remote player
    // and shared source row, but it must never rewrite the local player or
    // local player-entity projection.
    let local_player_before_remote = session.shared_spells().local_player.record_bytes().to_vec();
    let local_entity_before_remote = session
        .shared_spells()
        .player_entity
        .record_bytes()
        .to_vec();
    session
        .shared_spells_mut()
        .acquire_level2_spell_holder_for_player(
            12,
            PhysicalSpellActionRole::OrdinaryPickup,
            true,
            1,
        )
        .expect("remote direct holder acquisition");
    assert_eq!(
        session.shared_spells().local_player.record_bytes(),
        local_player_before_remote
    );
    assert_eq!(
        session.shared_spells().player_entity.record_bytes(),
        local_entity_before_remote
    );
    assert_eq!(
        session.shared_spells().players[1].enabled_spell_holders[12],
        38
    );
}

#[test]
fn level_two_setspell_active_branch_preserves_cold_payload_projection() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 acquisition production session");
    let family_index = session
        .shared_spells()
        .acquisition_states()
        .iter()
        .position(|state| state.family == 12)
        .expect("family-12 acquisition state");
    session.shared_spells_mut().players[0].family_selected_tiers[12] = 7;
    session.shared_spells_mut().players[0].record[528 + 12] = 7;
    let before_state = session.shared_spells().acquisition_states()[family_index];
    let before_record = session.shared_spells().pickups()[0].record_bytes().to_vec();
    session.shared_spells_mut().pickups[0].record[50..52].copy_from_slice(&7_u16.to_le_bytes());
    let outcome = session
        .shared_spells_mut()
        .acquire_level2_spell_holder_for_player(
            12,
            PhysicalSpellActionRole::OrdinaryPickup,
            true,
            0,
        )
        .expect("active SetSpell branch");
    assert_eq!(outcome.parent_entity_index, 111);
    let pickup = &session.shared_spells().pickups()[0];
    assert_eq!(pickup.record_bytes()[46..48], before_record[46..48]);
    assert_eq!(pickup.record_bytes()[52..54], before_record[52..54]);
    assert_eq!(pickup.record_bytes()[63..66], before_record[63..66]);
    assert_eq!(pickup.record_bytes()[140..152], before_record[140..152]);
    assert_eq!(pickup.record_bytes()[48..50], [3, 0]);
    assert_eq!(pickup.record_bytes()[50..52], [7, 0]);
    let after_state = session.shared_spells().acquisition_states()[family_index];
    assert_eq!(after_state.subspell_index, before_state.subspell_index);
    assert_eq!(after_state.word_0x30, before_state.word_0x30);
    assert_eq!(after_state.byte_0x3b, before_state.byte_0x3b);
    assert_eq!(after_state.mana_regen, before_state.mana_regen);
    assert_eq!(after_state.max_mana, before_state.max_mana);
    assert_eq!(after_state.mana, before_state.mana);
    assert_eq!(after_state.selected_tier, 7);
    assert_eq!(session.shared_spells().players[0].left_subspell_index, 7);
}

#[test]
fn level_two_stage_tag_materializes_authored_spell_sources_through_shared_arena() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 authored constructor session");
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    let baseline_shared_hash = session.shared_spells().reset_hash();
    let baseline_economy_hash = session.economy().reset_hash();
    let baseline_dynamic_holders = session.shared_spells().dynamic_pickups().to_vec();
    assert_eq!(
        baseline_dynamic_holders
            .iter()
            .map(|holder| holder.model)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    let global_before = session.economy().global_rng;

    session.step_outer_turn(PlayerCommand::default(), 0);

    let pickups = session.economy().spell_pickups();
    let constructor_global = level2_next_rng32(global_before);
    let hidden_model45_slots = session
        .economy()
        .authored_model45()
        .iter()
        .map(|source| {
            assert!(!source.drawing_enabled);
            assert!(
                session
                    .economy()
                    .map_chains()
                    .is_linked(source.source_entity_index)
            );
            source.source_entity_index
        })
        .collect::<Vec<_>>();
    assert_eq!(hidden_model45_slots.len(), 2);
    assert_eq!(pickups.len(), 2, "events: {:?}", session.dispatch_events());
    assert_eq!(pickups[0].authored_index, 155);
    assert_eq!(pickups[1].authored_index, 156);
    assert_eq!(pickups[0].family, 6);
    assert_eq!(pickups[1].family, 11);
    assert_eq!(pickups[0].action, 19);
    assert_eq!(pickups[1].action, 34);
    for source in pickups {
        assert_eq!(
            source.entity_rng,
            source
                .source_entity_index
                .wrapping_add(constructor_global as u16)
        );
        assert!(
            session
                .economy()
                .map_chains()
                .linked_entity_indices()
                .contains(&source.source_entity_index)
        );
    }
    assert_eq!(
        session.shared_spells().authored_pickup_slots().get(&155),
        Some(&pickups[0].source_entity_index)
    );
    assert_eq!(
        session.shared_spells().authored_pickup_slots().get(&156),
        Some(&pickups[1].source_entity_index)
    );
    assert!(
        session
            .shared_spells()
            .dynamic_pickups()
            .iter()
            .all(|entity| { entity.active && entity.class == 15 && entity.control_flags & 4 != 0 })
    );
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            group: 8,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [153, 154]
    )));
    assert!(session.level2_objective_done());
    assert_eq!(session.level2_objective_pause(), 1);
    assert!(session.economy.entities()[32].drawing_disabled());
    assert_eq!(session.runtime().objectives[0].stage_selectors[3], 2);
    assert_eq!(session.runtime().objectives[0].done, 0);
    assert_eq!(session.runtime().bytes()[306], 0);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::DynamicArena {
            kind: Level2DynamicArenaKind::SpellPickup,
            source_slot,
            ..
        } if *source_slot == pickups[0].source_entity_index
    )));

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.economy().authored_model45().is_empty());
    assert!(hidden_model45_slots.iter().all(|slot| {
        session.economy().free_slots.contains(slot) || session.economy().slot_generation(*slot) > 1
    }));

    session.reset();
    assert!(session.economy().spell_pickups().is_empty());
    assert!(session.economy().authored_model45().is_empty());
    assert_eq!(
        session.shared_spells().dynamic_pickups(),
        baseline_dynamic_holders.as_slice()
    );
    assert_eq!(session.shared_spells().reset_hash(), baseline_shared_hash);
    assert_eq!(session.economy().reset_hash(), baseline_economy_hash);
    assert_eq!(session.level2_selected_objective_stage_state(), 1);
    assert!(!session.level2_objective_done());
    assert_eq!(session.level2_objective_pause(), 0);
    assert!(!session.economy.entities()[32].drawing_disabled());
    assert_eq!(session.runtime().objectives[0].stage_selectors[3], 1);
    assert_eq!(session.runtime().objectives[0].done, 0);
}

#[test]
fn level_two_spell_owner_scan_and_source_gates_are_complete() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let local_entity_index = session.state.entity_index;
    let local_player_index = session
        .shared_spells()
        .players
        .iter()
        .position(|player| player.entity_index == local_entity_index)
        .expect("local shared player");
    let remote_player_index = session
        .shared_spells()
        .players
        .iter()
        .enumerate()
        .find(|(_, player)| player.entity_index != local_entity_index)
        .map(|(index, _)| index)
        .expect("remote shared player");
    let remote_entity_index = session.shared_spells().players[remote_player_index].entity_index;

    // Let the real group-8 source constructor reveal its authored row
    // position, then reset before the acquisition run.
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let source_155_position = session
        .economy()
        .spell_pickups()
        .iter()
        .find(|source| source.authored_index == 155)
        .expect("authored row 155 materialized")
        .position;
    assert!(
        session
            .economy()
            .spell_pickups()
            .iter()
            .any(|source| source.authored_index == 156)
    );
    session.reset();

    let configure_players = |session: &mut Mc2Simulation,
                             local_position: Position,
                             remote_position: Position,
                             remote_model: u8,
                             remote_life: i32,
                             full_hand: bool| {
        for entity in &mut session.economy.entities {
            if entity.class == 3
                && entity.index != local_entity_index
                && entity.index != remote_entity_index
            {
                entity.active = false;
                entity.record[2] = 0;
                entity.next_relation = None;
                entity.record[4..8].fill(0);
            }
        }
        let local = &mut session.economy.entities[usize::from(local_entity_index)];
        local.active = true;
        local.class = 3;
        local.model = 0;
        local.life = 100;
        local.position = local_position;
        local.record[2] = 1;
        local.record[67] = 3;
        local.record[68] = 0;
        local.record[12..16].copy_from_slice(&100_i32.to_le_bytes());
        local.record[80..86].copy_from_slice(&[
            local_position.x.to_le_bytes()[0],
            local_position.x.to_le_bytes()[1],
            local_position.y.to_le_bytes()[0],
            local_position.y.to_le_bytes()[1],
            local_position.z.to_le_bytes()[0],
            local_position.z.to_le_bytes()[1],
        ]);
        local.next_relation = None;
        local.record[4..8].fill(0);

        let remote = &mut session.economy.entities[usize::from(remote_entity_index)];
        remote.active = true;
        remote.class = 3;
        remote.model = remote_model;
        remote.life = remote_life;
        remote.position = remote_position;
        remote.record[2] = 1;
        remote.record[67] = 3;
        remote.record[68] = remote_model;
        remote.record[12..16].copy_from_slice(&remote_life.to_le_bytes());
        remote.record[80..86].copy_from_slice(&[
            remote_position.x.to_le_bytes()[0],
            remote_position.x.to_le_bytes()[1],
            remote_position.y.to_le_bytes()[0],
            remote_position.y.to_le_bytes()[1],
            remote_position.z.to_le_bytes()[0],
            remote_position.z.to_le_bytes()[1],
        ]);
        // The source head is remote even though storage keeps local
        // player 0 first.
        remote.next_relation = Some(local_entity_index);
        remote.record[4..8].copy_from_slice(&(u32::from(local_entity_index) + 1).to_le_bytes());
        session
            .set_level2_entity_position(session.state.entity_index, local_position)
            .expect("fixture player position updates the authoritative row and map link");

        if full_hand {
            let player = &mut session.shared_spells.players[remote_player_index];
            player.left_spell_slot_raw = 4;
            player.right_spell_slot_raw = 5;
            player.record[554..556].copy_from_slice(&4_i16.to_le_bytes());
            player.record[556..558].copy_from_slice(&5_i16.to_le_bytes());
        }
    };

    // The linked source order is [remote, local], while storage order is
    // [local, remote]. Both overlap row 155, so only source-first remote
    // may acquire it. Existing full-hand SetSpell behavior is exercised.
    configure_players(
        &mut session,
        source_155_position,
        source_155_position,
        0,
        100,
        true,
    );
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    let mut positive = None;
    for _ in 0..4 {
        session
            .set_level2_entity_position(session.state.entity_index, source_155_position)
            .expect("fixture player position updates the authoritative row and map link");
        session.step_outer_turn(PlayerCommand::default(), 0);
        positive = session
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Mc2DispatchEvent::SpellAcquisition {
                    source_slot,
                    family: 6,
                    result: Some(outcome),
                    ..
                } => Some((*source_slot, *outcome)),
                _ => None,
            });
        if positive.is_some() {
            break;
        }
    }
    let (source_slot, outcome) = positive.expect("ordinary route acquires authored row 155");
    assert_eq!(outcome.parent_entity_index, remote_entity_index);
    assert_eq!(outcome.hand, crate::spell_activation::SpellHand::Left);
    assert_eq!(
        session.shared_spells().players[local_player_index].enabled_spell_holders[6],
        0
    );
    assert_eq!(
        session.shared_spells().players[remote_player_index].enabled_spell_holders[6],
        source_slot
    );
    let source = session
        .shared_spells()
        .dynamic_pickups()
        .iter()
        .find(|source| source.index == source_slot)
        .expect("live authored row 155 source");
    assert_ne!(source.control_flags & 1, 0);
    assert_eq!(
        source.record_bytes()[44..46],
        remote_entity_index.to_le_bytes()
    );
    assert_eq!(
        session.shared_spells().players[remote_player_index].right_spell_slot_raw,
        5
    );
    assert!(
        session
            .shared_spells()
            .pickup_events()
            .iter()
            .any(|event| matches!(
                event,
                Level2SharedSpellEvent::SpellAcquired {
                    source_slot: observed,
                    player_entity_index,
                    family: 6,
                    ..
                } if *observed == source_slot && *player_entity_index == remote_entity_index
            ))
    );
    assert!(
        session
            .shared_spells()
            .activation_events()
            .iter()
            .any(|event| matches!(
                event,
                FlightEvent::PrepareSound {
                    entity_index,
                    player_index: -1,
                    sound_id: 18,
                    ..
                } if *entity_index == remote_entity_index
            ))
    );
    assert!(
        session
            .economy()
            .spell_pickups()
            .iter()
            .any(|source| source.authored_index == 155)
    );
    assert!(
        session
            .economy()
            .spell_pickups()
            .iter()
            .any(|source| source.authored_index == 156)
    );
    let stepped_hash = session.campaign_state_hash();

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    configure_players(
        &mut session,
        source_155_position,
        source_155_position,
        0,
        100,
        true,
    );
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    let mut replay = None;
    for _ in 0..4 {
        session
            .set_level2_entity_position(session.state.entity_index, source_155_position)
            .expect("fixture player position updates the authoritative row and map link");
        session.step_outer_turn(PlayerCommand::default(), 0);
        replay = session
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Mc2DispatchEvent::SpellAcquisition {
                    source_slot,
                    family: 6,
                    result: Some(outcome),
                    ..
                } => Some((*source_slot, *outcome)),
                _ => None,
            });
        if replay.is_some() {
            break;
        }
    }
    assert_eq!(replay, Some((source_slot, outcome)));
    assert_eq!(session.campaign_state_hash(), stepped_hash);

    for (remote_model, remote_life) in [(1_u8, 100_i32), (0, -1)] {
        session.reset();
        configure_players(
            &mut session,
            Position {
                x: source_155_position.x.wrapping_add(0x2000),
                ..source_155_position
            },
            source_155_position,
            remote_model,
            remote_life,
            false,
        );
        session
            .set_level2_selected_objective_stage_state(2)
            .expect("source objective completion sentinel");
        for _ in 0..4 {
            session.step_outer_turn(PlayerCommand::default(), 0);
        }
        assert!(session.dispatch_events().iter().all(|event| {
            !matches!(
                event,
                Mc2DispatchEvent::SpellAcquisition {
                    family: 6,
                    result: Some(_),
                    ..
                }
            )
        }));
        assert!(
            session
                .shared_spells()
                .players
                .iter()
                .all(|player| player.enabled_spell_holders[6] == 0)
        );
    }

    // Existing duplicate/no-overlap gates remain nonmutating; this test
    // only proves the source-order route does not bypass them.
    session.reset();
    let source_position = session.shared_spells().pickups()[0].position;
    for player_index in 0..session.shared_spells().players.len() {
        session.shared_spells_mut().players[player_index].enabled_spell_holders[12] = 999;
        session.shared_spells_mut().players[player_index].record[268 + 12 * 2..270 + 12 * 2]
            .copy_from_slice(&999_u16.to_le_bytes());
        session.shared_spells_mut().player_entities[player_index].active = true;
        session.shared_spells_mut().player_entities[player_index].model = 0;
        session.shared_spells_mut().player_entities[player_index].life = 1;
        session.shared_spells_mut().player_entities[player_index].position = source_position;
    }
    session.shared_spells_mut().pickups[0].phase = 0;
    session.shared_spells_mut().pickups[0].record[66] = 0;
    let before_duplicate_players = session.shared_spells().players.clone();
    let before_duplicate_states = session.shared_spells().acquisitions.clone();
    let before_duplicate_action = session.shared_spells().pickups()[0].action;
    assert!(
        session
            .shared_spells_mut()
            .step_level2_spell_acquisition(12, local_entity_index, source_position)
            .expect("duplicate source gate")
            .is_none()
    );
    assert_eq!(session.shared_spells().players, before_duplicate_players);
    assert_eq!(
        session.shared_spells().acquisitions,
        before_duplicate_states
    );
    assert_eq!(
        session.shared_spells().pickups()[0].action,
        before_duplicate_action
    );
    assert!(session.shared_spells().pickup_events().is_empty());

    session.shared_spells_mut().reset();
    let far_position = Position {
        x: source_position.x.wrapping_add(0x4000),
        ..source_position
    };
    session.shared_spells_mut().pickups[0].phase = 0;
    session.shared_spells_mut().pickups[0].record[66] = 0;
    let before_no_overlap_players = session.shared_spells().players.clone();
    let before_no_overlap_states = session.shared_spells().acquisitions.clone();
    let before_no_overlap_action = session.shared_spells().pickups()[0].action;
    assert!(
        session
            .shared_spells_mut()
            .step_level2_spell_acquisition(12, local_entity_index, far_position)
            .expect("no-overlap source gate")
            .is_none()
    );
    assert_eq!(session.shared_spells().players, before_no_overlap_players);
    assert_eq!(
        session.shared_spells().acquisitions,
        before_no_overlap_states
    );
    assert_eq!(
        session.shared_spells().pickups()[0].action,
        before_no_overlap_action
    );
    assert!(session.shared_spells().pickup_events().is_empty());

    // A full source arena rejects authored construction without leaving a
    // partial row or holder relation behind.
    session.reset();
    let reset_free_slots = session.economy().free_slots.clone();
    // The group consumer rebuilds the free arena. Occupy its real reset-free
    // slots using the package's protected portal constructor so that rebuild
    // cannot recover the slots or a flagged replacement candidate.
    for _ in 1_u16..1_000 {
        if session.allocate_level2_authored_class14(244).is_none() {
            break;
        }
    }
    assert!(session.allocate_level2_authored_class14(244).is_none());
    assert!(session.economy().free_slots.is_empty());
    assert!(
        reset_free_slots
            .iter()
            .all(|slot| session.economy().live_arena_cursor_row(*slot).is_some()),
        "every reset-free source slot has a real live carrier before arena rebuild"
    );
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.economy().spell_pickups().is_empty());
    assert!(session.shared_spells().authored_pickup_slots().is_empty());
    assert!(
        session
            .shared_spells()
            .players
            .iter()
            .all(|player| player.enabled_spell_holders[6] == 0)
    );
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_experience_scroll_uses_wrapping_runtime_xp_and_downward_selection() {
    let mut session = level_two_campaign_session();
    let source = session
        .allocate_level2_authored_class14(248)
        .expect("authored subtype-5 row");
    let scroll_position = source.position;
    session
        .shared_spells_mut()
        .acquire_level2_spell_holder(12, PhysicalSpellActionRole::OrdinaryPickup, true)
        .expect("source-shaped family-12 acquisition");
    let local_entity_index = session.state.entity_index;
    {
        let mut shared = session.shared_spells_mut();
        let owner_index = shared
            .players
            .iter()
            .position(|player| player.entity_index == local_entity_index)
            .expect("local player row");
        let owner = &mut shared.players[owner_index];
        owner.runtime_spell_experience[12] = i32::MAX - 2;
        owner.record[212..216].copy_from_slice(&(i32::MAX - 2).to_le_bytes());
        owner.spell_unlocked_levels[12] = 2;
        owner.record[514] = 2;
        owner.family_selected_tiers[12] = 2;
        owner.record[540] = 2;
        shared.local_player = owner.clone();
        shared.spell_experience[12] = i32::MAX - 2;
        shared.spell_levels[12] = 2;
        shared.events.clear();
        shared.pickup_events.clear();
    }
    session.state.position = scroll_position;
    session.step_outer_turn(PlayerCommand::default(), 0);

    let expected_runtime = (i32::MAX - 2).wrapping_add(LEVEL2_EXPERIENCE_SCROLL_AWARD);
    let mut shared = session.shared_spells();
    let owner = shared
        .players
        .iter()
        .find(|player| player.entity_index == local_entity_index)
        .expect("local player row after scroll");
    assert_eq!(owner.runtime_spell_experience[12], expected_runtime);
    assert_eq!(
        owner.record_bytes()[212..216],
        expected_runtime.to_le_bytes()
    );
    assert_eq!(owner.spell_unlocked_levels[12], 0);
    assert_eq!(owner.record_bytes()[514], 0);
    assert_eq!(owner.family_selected_tiers[12], 0);
    assert_eq!(owner.record_bytes()[540], 0);
    assert_eq!(shared.spell_experience()[12], expected_runtime);
    assert_eq!(shared.spell_levels()[12], 0);
    assert_eq!(
        shared
            .pickup_events()
            .iter()
            .filter(|event| matches!(
                event,
                Level2SharedSpellEvent::AbilityImproved {
                    source_family: 12,
                    tier: 0,
                }
            ))
            .count(),
        1,
        "scroll level projection notifies on the source's wrapping 2->0 change"
    );
    assert_eq!(
        shared
            .activation_events()
            .iter()
            .filter(|event| matches!(event, FlightEvent::PrepareSound { sound_id: 61, .. }))
            .count(),
        1
    );
}

#[test]
fn level_two_materialized_authored_spell_reaches_generic_acquisition_path() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 authored acquisition session");
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let source = session
        .economy()
        .spell_pickups()
        .iter()
        .find(|source| source.authored_index == 155)
        .copied()
        .expect("authored family-6 source");
    session.state.position = source.position;
    let mut acquired = None;
    for _ in 0..4 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        acquired = session
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Mc2DispatchEvent::SpellAcquisition {
                    family: 6,
                    result: Some(result),
                    source_slot,
                    ..
                } => Some((*source_slot, *result)),
                _ => None,
            });
        if acquired.is_some() {
            break;
        }
    }
    let (source_slot, outcome) = acquired.expect("authored family-6 acquisition");
    assert_eq!(source_slot, source.source_entity_index);
    assert_eq!(outcome.family, 6);
    assert_eq!(outcome.source_slot, source.source_entity_index);
    assert_eq!(outcome.holder_action, 18);
    assert_eq!(outcome.replacement_action, None);
    assert_eq!(
        session
            .shared_spells()
            .acquisition_states()
            .iter()
            .find(|state| state.family == 6)
            .expect("family-6 acquisition state")
            .authored_source_index,
        155
    );
}

#[test]
fn level_two_setspell_clamps_holder_payload_but_preserves_raw_hand_tier() {
    let mut session = level_two_campaign_session();
    let pickup_position = session.shared_spells().pickups()[0].position;
    session.shared_spells_mut().players[0].family_selected_tiers[12] = 7;
    session.shared_spells_mut().players[0].record[528 + 12] = 7;
    session.shared_spells_mut().pickups[0].phase = 0;
    session.shared_spells_mut().pickups[0].record[66] = 0;
    let pickup_slot = session.shared_spells().pickups()[0].index;
    session.economy.entities[usize::from(pickup_slot)].set_phase(0);
    session
        .set_level2_entity_position(session.state.entity_index, pickup_position)
        .expect("fixture player position updates the authoritative row and map link");

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::SpellAcquisition {
                source_slot: 38,
                family: 12,
                result: Some(_),
                ..
            }
        )
    }));
    let state = session
        .shared_spells()
        .acquisition_states()
        .iter()
        .find(|state| state.family == 12)
        .expect("family-12 raw-tier state");
    let pickup = session
        .shared_spells()
        .pickups()
        .iter()
        .find(|pickup| pickup.index == 38)
        .expect("family-12 holder row");
    assert_eq!(state.selected_tier, 7);
    assert_eq!(
        session.shared_spells().players[0].family_selected_tiers[12],
        7
    );
    assert_eq!(session.shared_spells().players[0].record_bytes()[540], 7);
    assert_eq!(session.shared_spells().players[0].left_subspell_index, 7);
    assert_eq!(pickup.record_bytes()[74], 2);
    assert_eq!(pickup.level_gate, 2);
    assert_eq!(pickup.record_bytes()[52..54], 361_u16.to_le_bytes());
    assert_eq!(pickup.record_bytes()[144..148], 5_000_i32.to_le_bytes());
    assert_eq!(pickup.record_bytes()[148..152], 13_i32.to_le_bytes());

    session.reset();
    assert_eq!(session.shared_spells().players[0].record_bytes()[540], 0);
    assert_eq!(session.shared_spells().pickups()[0].record_bytes()[74], 0);
}

#[test]
fn level_two_setspell_options_tier_and_hand_state_are_exact() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();

    // Drive two otherwise identical zero-pass outer-turn sequences so
    // the option bit is the only shared-spell state difference. This
    // proves both hash binding and the source's second-command OFF
    // transition without manufacturing an options setter.
    let mut option_control = level_two_campaign_session();
    session.step_outer_turn(
        PlayerCommand {
            opcode: 0x1e,
            argument_1: 9,
            ..PlayerCommand::default()
        },
        u8::MAX,
    );
    option_control.step_outer_turn(PlayerCommand::default(), u8::MAX);
    assert!(session.shared_spells.free_spell_usage);
    assert_ne!(
        session.shared_spells().reset_hash(),
        option_control.shared_spells().reset_hash()
    );
    session.step_outer_turn(
        PlayerCommand {
            opcode: 0x1e,
            argument_1: 9,
            ..PlayerCommand::default()
        },
        u8::MAX,
    );
    option_control.step_outer_turn(PlayerCommand::default(), u8::MAX);
    assert!(!session.shared_spells.free_spell_usage);
    assert_eq!(
        session.shared_spells().reset_hash(),
        option_control.shared_spells().reset_hash()
    );
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);

    // Tier 7 and the independent UI cursor are authoritative prestate
    // discriminators: the Level 2 input contract cannot legally create
    // a tier above two before this unowned family is collected. The
    // behavior under test still enters only through PlayerCommand and
    // the ordinary source cursor; there is no test-only production hook.
    let configure_cold_pickup = |session: &mut Mc2Simulation| {
        let pickup_position = session.shared_spells().pickups()[0].position;
        let player = &mut session.shared_spells_mut().players[0];
        player.family_selected_tiers[12] = 7;
        player.record[528 + 12] = 7;
        player.selected_spell_ui_index = 24;
        player.selected_subspell_index = 2;
        player.record[561] = 24;
        player.record[562] = 2;
        session.shared_spells_mut().pickups[0].phase = 0;
        session.shared_spells_mut().pickups[0].record[66] = 0;
        let pickup_slot = session.shared_spells().pickups()[0].index;
        session.economy.entities[usize::from(pickup_slot)].set_phase(0);
        session
            .set_level2_entity_position(session.state.entity_index, pickup_position)
            .expect("fixture player position updates the authoritative row and map link");
    };

    configure_cold_pickup(&mut session);
    session.step_outer_turn(
        PlayerCommand {
            opcode: 0x1e,
            argument_1: 9,
            ..PlayerCommand::default()
        },
        0,
    );
    assert!(session.shared_spells.free_spell_usage);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::SpellAcquisition {
            source_slot: 38,
            family: 12,
            result: Some(_),
            ..
        }
    )));
    let state = session
        .shared_spells()
        .acquisition_states()
        .iter()
        .find(|state| state.family == 12)
        .expect("family-12 SetSpell state");
    let pickup = &session.shared_spells().pickups()[0];
    let player = &session.shared_spells().players[0];
    assert_eq!((state.learned, state.eligibility), (1, 1));
    assert_eq!(state.selected_tier, 7);
    assert_eq!(
        (player.left_spell_slot_raw, player.left_subspell_index),
        (12, 7)
    );
    assert_eq!(
        (
            player.selected_spell_ui_index,
            player.selected_subspell_index
        ),
        (24, 2)
    );
    assert_eq!(pickup.level_gate, 2);
    assert_eq!(pickup.mana_regen, 0);
    assert_eq!(pickup.max_mana, 5_000);
    assert_eq!(pickup.mana, 1);
    assert_eq!(pickup.record_bytes()[140..144], 0_i32.to_le_bytes());
    assert_eq!(pickup.record_bytes()[144..148], 5_000_i32.to_le_bytes());
    assert_eq!(pickup.record_bytes()[148..152], 1_i32.to_le_bytes());
    let first_events = session.dispatch_events().to_vec();
    let first_hash = session.campaign_state_hash();

    session.reset();
    assert!(!session.shared_spells.free_spell_usage);
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    configure_cold_pickup(&mut session);
    session.step_outer_turn(
        PlayerCommand {
            opcode: 0x1e,
            argument_1: 9,
            ..PlayerCommand::default()
        },
        0,
    );
    assert_eq!(session.dispatch_events(), first_events.as_slice());
    assert_eq!(session.campaign_state_hash(), first_hash);

    // With the option clear, the same source-ordered production route
    // retains the pinned family-12 tier-2 mana fields. This near-identical
    // negative prevents the free-spell values from becoming a hard-coded
    // SetSpell profile.
    session.reset();
    configure_cold_pickup(&mut session);
    session.step_outer_turn(PlayerCommand::default(), 0);
    let mut ordinary = &session.shared_spells().pickups()[0];
    assert!(!session.shared_spells.free_spell_usage);
    assert_eq!(ordinary.max_mana, 5_000);
    assert_eq!(ordinary.mana_regen, 10_000);
    assert_eq!(ordinary.mana, 13);

    // The source active-holder branch only queues word 0x2c. Even while
    // free-spell usage is enabled it must not rewrite the retained cold
    // payload.
    let mut active = level_two_campaign_session();
    configure_cold_pickup(&mut active);
    let active_slot = active.shared_spells().pickups()[0].index;
    // The imported callback refreshes its projection from the numeric row.
    // Seed the retained active-holder branch on that authoritative carrier.
    let active_source = &mut active.economy.entities[usize::from(active_slot)];
    active_source.active_value = 7;
    active_source.record[50..52].copy_from_slice(&7_u16.to_le_bytes());
    active.shared_spells_mut().pickups[0].active_value = 7;
    active.shared_spells_mut().pickups[0].record[50..52].copy_from_slice(&7_u16.to_le_bytes());
    let active_payload_before =
        active.shared_spells().pickups()[0].record_bytes()[52..152].to_vec();
    active.step_outer_turn(
        PlayerCommand {
            opcode: 0x1e,
            argument_1: 9,
            ..PlayerCommand::default()
        },
        0,
    );
    assert!(active.shared_spells.free_spell_usage);
    assert!(active.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::SpellAcquisition {
            source_slot: 38,
            family: 12,
            result: Some(_),
            ..
        }
    )));
    let active_record = active.shared_spells().pickups()[0].record_bytes();
    assert_eq!(active_record[52..54], active_payload_before[0..2]);
    assert_eq!(active_record[63..66], active_payload_before[11..14]);
    assert_eq!(active_record[140..152], active_payload_before[88..100]);
}

#[test]
fn level_two_dynamic_economy_slots_dispatch_and_reclaim_in_source_order() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Goat economy session");
    let baseline_economy_hash = session.economy().reset_hash();
    let goat_slot = LEVEL_TWO_GOAT_INDICES[0];
    let goat_source_id = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == goat_slot)
        .expect("Level 2 Goat source row")
        .source_id;
    let attacker = session.state.entity_index.cast_signed();
    session
        .session
        .state
        .live
        .goats
        .inject_damage(goat_slot, 1_000_000, attacker)
        .expect("inject lethal Goat damage");

    let mut observed_death = false;
    for _ in 0..32 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        observed_death = session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Mc2DispatchEvent::Goat {
                    source_slot,
                    events,
                    ..
                } if *source_slot == goat_slot
                    && events.iter().any(|event| matches!(
                        event,
                        GoatStepEvent::ManaConverted { .. }
                            | GoatStepEvent::ExtraManaSphereRequested { .. }
                    ))
            )
        });
        if observed_death {
            break;
        }
    }
    assert!(
        observed_death,
        "lethal Level 2 Goat reached KillEntity tail"
    );
    let goat_events = session
        .dispatch_events()
        .iter()
        .find_map(|event| match event {
            Mc2DispatchEvent::Goat {
                source_slot,
                events,
                ..
            } if *source_slot == goat_slot => Some(events),
            _ => None,
        })
        .expect("Goat dispatch row");
    let converted_mana = goat_events
        .iter()
        .find_map(|event| match event {
            GoatStepEvent::ManaConverted { mana, .. } => Some(*mana),
            _ => None,
        })
        .expect("Goat mana conversion payload");
    let mana_index = goat_events
        .iter()
        .position(|event| matches!(event, GoatStepEvent::ManaConverted { .. }))
        .expect("Goat mana conversion event");
    let extra_index = goat_events
        .iter()
        .position(|event| matches!(event, GoatStepEvent::ExtraManaSphereRequested { .. }))
        .expect("Goat burst request event");
    let removal_index = goat_events
        .iter()
        .position(|event| matches!(event, GoatStepEvent::RemovalQueued { .. }))
        .expect("Goat removal event");
    assert!(mana_index < extra_index && extra_index < removal_index);
    let economy_payloads = session
        .dispatch_events()
        .iter()
        .filter_map(|event| match event {
            Mc2DispatchEvent::Economy {
                source_slot, event, ..
            } => Some((*source_slot, *event)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let sphere_payload_index = economy_payloads
        .iter()
        .position(|(_, event)| {
            matches!(
                event,
                Level2EconomyEvent::ManaSphereSpawned {
                    source_entity_index,
                    mana,
                    ..
                } if *source_entity_index == goat_slot && *mana == converted_mana
            )
        })
        .expect("Goat dispatch includes sphere payload");
    let burst_payload_index = economy_payloads
        .iter()
        .position(|(_, event)| {
            matches!(
                event,
                Level2EconomyEvent::DeathBurstSpawned {
                    parent_entity_index,
                    ..
                } if *parent_entity_index == goat_slot
            )
        })
        .expect("Goat dispatch includes burst payload");
    assert!(sphere_payload_index < burst_payload_index);
    assert_eq!(
        economy_payloads[sphere_payload_index].0, goat_slot,
        "sphere payload keeps the Goat source slot"
    );
    assert!(session.economy().events().iter().any(|event| {
        matches!(event, Level2EconomyEvent::ManaSphereSpawned {
                source_entity_index,
                ..
            } if *source_entity_index == goat_slot)
    }));
    let burst_slot = session
        .economy()
        .events()
        .iter()
        .find_map(|event| match event {
            Level2EconomyEvent::DeathBurstSpawned {
                source_entity_index,
                parent_entity_index,
                id_entity_index,
            } if *parent_entity_index == goat_slot => {
                assert_ne!(*id_entity_index, 0);
                assert_eq!(*id_entity_index, goat_source_id);
                Some(*source_entity_index)
            }
            _ => None,
        })
        .expect("Goat KillEntity allocated death burst");
    let sphere_slot = session
        .economy()
        .events()
        .iter()
        .find_map(|event| match event {
            Level2EconomyEvent::ManaSphereSpawned {
                sphere_id,
                source_entity_index,
                ..
            } if *source_entity_index == goat_slot => Some(*sphere_id as u16),
            _ => None,
        })
        .expect("Goat mana sphere slot");
    assert!(sphere_slot < burst_slot, "source allocations retain order");
    let burst_expired_payload_index = economy_payloads
        .iter()
        .position(|(_, event)| {
            matches!(
                event,
                Level2EconomyEvent::DeathBurstExpired {
                    source_entity_index,
                    parent_entity_index,
                } if *source_entity_index == burst_slot
                    && *parent_entity_index == goat_slot
            )
        })
        .expect("Goat dispatch includes burst expiry payload");
    assert!(burst_payload_index < burst_expired_payload_index);
    assert_eq!(
        economy_payloads[burst_expired_payload_index].0, burst_slot,
        "burst expiry keeps the burst source slot"
    );
    let dynamic_slots = session
        .dispatch_events()
        .iter()
        .filter_map(|event| match event {
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::ManaSphere,
                ..
            }
            | Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::DeathBurst,
                ..
            } if *source_slot == sphere_slot || *source_slot == burst_slot => Some(*source_slot),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(dynamic_slots.first().copied(), Some(sphere_slot));
    assert_eq!(dynamic_slots.last().copied(), Some(burst_slot));
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::DeathBurst,
                ..
            } if *source_slot == burst_slot
        )
    }));
    assert!(session.economy().events().iter().any(|event| {
        matches!(event, Level2EconomyEvent::DeathBurstExpired { source_entity_index, .. }
                if *source_entity_index == burst_slot)
    }));

    session.reset();
    assert_eq!(session.economy().reset_hash(), baseline_economy_hash);
    assert!(session.economy().death_bursts().is_empty());
    assert!(session.economy().spheres().is_empty());

    // DSP-L2-178 binds action 105/sub_23340 to the ordinary Villager
    // route, not to a dynamic HouseResident row. Prove that the same
    // production cursor dispatches the qualified source row and that
    // reset/replay restores its exact state without inventing a dynamic
    // allocation for this callback.
    let villager_slot = LEVEL_TWO_VILLAGER_INDICES[0];
    let baseline_villager = session
        .ordinary()
        .villagers()
        .iter()
        .find(|villager| villager.entity_index == villager_slot)
        .cloned()
        .expect("qualified action-105 Villager source row");
    assert_eq!(baseline_villager.action, 105);
    let baseline_ordinary_hash = session.ordinary().reset_hash();
    let baseline_campaign_hash = session.campaign_state_hash();
    let baseline_live_arena = session.economy().live_arena_slots();

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::Villager {
            pass: 0,
            source_slot,
            ..
        } if *source_slot == villager_slot
    )));
    let stepped_villager = session
        .ordinary()
        .villagers()
        .iter()
        .find(|villager| villager.entity_index == villager_slot)
        .cloned()
        .expect("action-105 Villager after outer dispatch");
    assert_eq!(stepped_villager.action, 105);
    assert_eq!(
        stepped_villager.phase,
        baseline_villager.phase.wrapping_add(1)
    );
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::DynamicArena { source_slot, .. }
            if *source_slot == villager_slot
    )));
    let stepped_ordinary_hash = session.ordinary().reset_hash();

    session.reset();
    assert_eq!(session.ordinary().reset_hash(), baseline_ordinary_hash);
    assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);
    assert_eq!(session.economy().live_arena_slots(), baseline_live_arena);
    assert_eq!(
        session
            .ordinary()
            .villagers()
            .iter()
            .find(|villager| villager.entity_index == villager_slot),
        Some(&baseline_villager)
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.ordinary().reset_hash(), stepped_ordinary_hash);
    assert_eq!(
        session
            .ordinary()
            .villagers()
            .iter()
            .find(|villager| villager.entity_index == villager_slot),
        Some(&stepped_villager)
    );
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::Villager {
            pass: 0,
            source_slot,
            ..
        } if *source_slot == villager_slot
    )));
}

#[test]
fn level_two_economy_dispatch_cursor_isolated_and_reset_replays_payloads() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");

    // Direct API history is retained for economy auditing but is not a
    // source-dispatch payload for the next outer turn.
    let mut stale = inspection
        .level2_reset_session()
        .expect("stale economy session");
    stale
        .economy_mut()
        .spawn_mana_sphere_from_death(901, Position::default(), 0, 0x3456, 10, 111)
        .expect("pre-step sphere allocation");
    stale.step_outer_turn(PlayerCommand::default(), 0);
    assert!(!stale.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::Economy {
                event: Level2EconomyEvent::ManaSphereSpawned {
                    source_entity_index,
                    ..
                },
                ..
            } if *source_entity_index == 901
        )
    }));

    // A dynamic handler-created event is published once, not again on a
    // later source turn, and reset returns the cursor to frame zero.
    let mut session = inspection
        .level2_reset_session()
        .expect("economy cursor session");
    let carrier_slot = session
        .economy_mut()
        .spawn_mana_carrier(Position::default(), 111, 0)
        .expect("carrier allocation");
    let sphere_slot = session
        .economy_mut()
        .spawn_mana_sphere_from_death(902, Position::default(), 0, 0x4567, 10, 111)
        .expect("sphere allocation");
    session
        .economy_mut()
        .carriers
        .iter_mut()
        .find(|carrier| carrier.source_entity_index == carrier_slot)
        .expect("carrier row")
        .target_sphere_id = sphere_slot;
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::Economy {
                source_slot,
                event: Level2EconomyEvent::ManaSphereCollected { sphere_id, .. },
                ..
            } if *source_slot == carrier_slot && *sphere_id == sphere_slot
        )
    }));
    let first_payloads = session
        .dispatch_events()
        .iter()
        .filter_map(|event| match event {
            Mc2DispatchEvent::Economy { event, .. } => Some(*event),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(first_payloads.iter().any(|event| {
        matches!(
            event,
            Level2EconomyEvent::ManaSphereCollected { sphere_id, .. }
                if *sphere_id == sphere_slot
        )
    }));

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(!session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::Economy {
                event:
                    Level2EconomyEvent::ManaSphereCollected {
                        sphere_id,
                        carrier_id,
                        ..
                    },
                ..
            } if *sphere_id == sphere_slot && *carrier_id == u32::from(carrier_slot)
        )
    }));

    session.reset();
    let replay_carrier_slot = session
        .economy_mut()
        .spawn_mana_carrier(Position::default(), 111, 0)
        .expect("replay carrier allocation");
    let replay_sphere_slot = session
        .economy_mut()
        .spawn_mana_sphere_from_death(902, Position::default(), 0, 0x4567, 10, 111)
        .expect("replay sphere allocation");
    session
        .economy_mut()
        .carriers
        .iter_mut()
        .find(|carrier| carrier.source_entity_index == replay_carrier_slot)
        .expect("replay carrier row")
        .target_sphere_id = replay_sphere_slot;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let replay_payloads = session
        .dispatch_events()
        .iter()
        .filter_map(|event| match event {
            Mc2DispatchEvent::Economy { event, .. } => Some(*event),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(first_payloads, replay_payloads);
}

#[test]
fn level_two_raw_source_phase_sidecars_initialize_hash_and_reset_for_exact_carriers() {
    let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level2-day-owner-cache/components/gameplay");
    let inspection =
        NativeWorld::inspect_level2_with_owner_cache(level_two_fixture_path(), &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
    let mut session = inspection
        .level2_reset_session()
        .expect("raw source-phase carrier session");
    let baseline_hash = session.economy().reset_hash();
    let owner_entity_index = session.state.entity_index;
    let holder_slot = session
        .shared_spells()
        .dynamic_pickups()
        .iter()
        .find(|holder| holder.model == 2)
        .expect("qualified family-2 holder")
        .index;

    let projectile_slot = session
        .economy_mut()
        .spawn_castle_projectile(
            Position::default(),
            owner_entity_index,
            holder_slot,
            0,
            0,
            0,
        )
        .expect("Castle projectile carrier");
    let carrier_slot = session
        .economy_mut()
        .spawn_mana_carrier(Position::default(), owner_entity_index, 0)
        .expect("mana-carrier source phase");
    let pulse_slot = session
        .economy_mut()
        .allocate_beyond_sight_pulse_effect(
            holder_slot,
            owner_entity_index,
            owner_entity_index,
            0,
            16,
            0,
            0,
            Position::default(),
            Position::default(),
            0,
            0,
        )
        .expect("Beyond Sight pulse carrier")
        .source_entity_index;
    let attraction_slot = session
        .allocate_level2_mana_attraction_field(16, MANA_ATTRACTION_ACTION)
        .expect("mana-attraction source phase");
    let slots = [projectile_slot, carrier_slot, pulse_slot, attraction_slot];
    for slot in slots {
        assert_eq!(
            session.economy().source_phase_sidecar(slot),
            Some(slot as u8),
            "NewEvent initializes byte 0x3E from the exact numeric source slot"
        );
    }

    let allocated_hash = session.economy().reset_hash();
    assert_ne!(allocated_hash, baseline_hash);
    assert_eq!(
        session
            .economy_mut()
            .advance_source_phase_sidecar(projectile_slot),
        Some((projectile_slot as u8).wrapping_add(1))
    );
    assert_ne!(
        session.economy().reset_hash(),
        allocated_hash,
        "the raw source phase is part of deterministic state identity"
    );

    session.reset();
    assert_eq!(session.economy().reset_hash(), baseline_hash);
    for slot in slots {
        assert_eq!(session.economy().source_phase_sidecar(slot), None);
    }
}

#[test]
fn level_two_mana_attraction_field_targets_spheres_in_source_order_and_expires() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let authored = session
        .mission
        .authored_features()
        .iter()
        .copied()
        .find(|feature| feature.index == 16)
        .expect("authored model-54 row");
    assert_eq!(
        (authored.kind, authored.subtype, authored.stage_tag),
        (10, 54, 33)
    );

    // Allocate the spheres first so the authored field's later source slot
    // writes a mailbox that remains observable after this outer pass.
    let near_slot = u16::try_from(
        session
            .economy_mut()
            .spawn_mana_sphere_from_death(
                901,
                Position {
                    x: 640,
                    y: 39_168,
                    z: 0,
                },
                0,
                0x1234,
                10,
                111,
            )
            .expect("near sphere allocation"),
    )
    .expect("near sphere slot fits u16");
    let far_slot = u16::try_from(
        session
            .economy_mut()
            .spawn_mana_sphere_from_death(
                902,
                Position {
                    x: 30_000,
                    y: 50_000,
                    z: 0,
                },
                0,
                0x2345,
                10,
                111,
            )
            .expect("far sphere allocation"),
    )
    .expect("far sphere slot fits u16");
    let field_producer_position = session
        .level2_imported_switch_roots
        .iter()
        .find(|source| source.authored_index == 11)
        .map(|source| source.position)
        .expect("actual group-1 overlap producer for authored field row 16");
    session
        .set_level2_entity_position(session.state.entity_index, field_producer_position)
        .expect("public player-position input activates the field's actual group-1 producer");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let field = session
        .economy()
        .mana_attraction_fields()
        .iter()
        .find(|field| field.authored_index == 16)
        .copied()
        .expect("production model-54 field allocation");
    let field_slot = field.source_entity_index;
    assert_eq!(field.action, MANA_ATTRACTION_ACTION);
    assert_eq!(field.radius_squared, (33_u32 * 256).pow(2));
    assert_eq!(field.max_life, 280);
    assert!(field_slot > near_slot && field_slot > far_slot);
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::ManaAttraction,
                ..
            } if *source_slot == field_slot
        )
    }));
    let near = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == near_slot)
        .copied()
        .expect("near sphere");
    let far = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == far_slot)
        .copied()
        .expect("far sphere");
    assert_eq!(near.attraction_source_entity_index, field_slot);
    assert!(near.attraction_step <= 42);
    assert_eq!(far.attraction_source_entity_index, 0);

    // The sphere callback consumes the mailbox and moves toward the
    // source. The next source pass may re-target it, but the direct
    // handler proves the one-shot consume/clear ordering.
    let before = near.position;
    assert!(session.economy_mut().step_mana_sphere_slot(near_slot));
    let consumed = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == near_slot)
        .copied()
        .expect("consumed near sphere");
    assert_eq!(consumed.attraction_source_entity_index, 0);
    assert_ne!(consumed.position, before);

    // Force the signed lifetime sentinel, exercise the real dynamic
    // cursor/reclaim route, and verify reset returns the sealed baseline.
    session
        .economy_mut()
        .mana_attraction_fields
        .iter_mut()
        .find(|field| field.source_entity_index == field_slot)
        .expect("field remains live")
        .life = -1;
    let phase_before = session
        .economy()
        .source_phase_sidecar(field_slot)
        .expect("field common source phase");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let hidden = session
        .economy()
        .mana_attraction_fields()
        .iter()
        .find(|field| field.source_entity_index == field_slot)
        .expect("field remains allocated through its callback pass");
    assert!(!hidden.active);
    assert!(session.economy().map_chains().is_linked(field_slot));
    assert!(!session.economy().free_slots.contains(&field_slot));
    assert_eq!(
        session.economy().source_phase_sidecar(field_slot),
        Some(phase_before.wrapping_add(1))
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .mana_attraction_fields()
            .iter()
            .all(|field| field.source_entity_index != field_slot)
    );
    assert!(!session.economy().map_chains().is_linked(field_slot));
    assert!(session.economy().free_slots.contains(&field_slot));
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    assert!(session.economy().mana_attraction_fields().is_empty());

    let mut alias = level_two_campaign_session();
    let alias_slot = alias
        .allocate_level2_mana_attraction_field(16, MANA_ATTRACTION_ALIAS_ACTION)
        .expect("action-76 alias allocation");
    let alias_field = alias
        .economy()
        .mana_attraction_fields()
        .iter()
        .find(|field| field.source_entity_index == alias_slot)
        .copied()
        .expect("alias field row");
    assert_eq!(
        (alias_field.radius_squared, alias_field.max_life),
        (field.radius_squared, field.max_life)
    );
    assert_eq!(alias_field.action, MANA_ATTRACTION_ALIAS_ACTION);
}

#[test]
fn level_two_economy_global_new_event_rng_advances_per_source_pass() {
    // Pinned remc2 Events.cpp:419-420 (ApplyEvents_498A0) advances the
    // full D41A0.rand_0x8 stream once before walking source rows. Each
    // NewEvent_4A050 then seeds rand_0x14 with the allocated slot plus
    // the low word of that pass-global value.
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("global NewEvent RNG session");
    let baseline_global = session.economy().global_rng;
    let changed_global = 0xabcd_1234_u32;
    session.economy_mut().global_rng = changed_global;

    session.step_outer_turn(PlayerCommand::default(), 0);
    let pass_one_global = level2_next_rng32(changed_global);
    assert_eq!(session.economy().global_rng, pass_one_global);
    let first_slot = session
        .economy()
        .free_slots
        .last()
        .copied()
        .expect("first free source slot");
    let first_id = session
        .economy_mut()
        .spawn_mana_sphere_from_death(901, Position::default(), 0, 0x1111, 321, 111)
        .expect("first NewEvent allocation");
    assert_eq!(first_id, u32::from(first_slot));
    let first_seed = first_slot.wrapping_add(pass_one_global as u16);
    let first_rng = level2_next_rng16(first_seed);
    let first_speed_rng = level2_next_rng16(first_rng);
    assert_eq!(
        session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.id == first_id)
            .expect("first sphere")
            .entity_rng,
        first_speed_rng
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    let pass_two_global = level2_next_rng32(pass_one_global);
    assert_eq!(session.economy().global_rng, pass_two_global);
    let second_slot = session
        .economy()
        .free_slots
        .last()
        .copied()
        .expect("second free source slot");
    let second_id = session
        .economy_mut()
        .spawn_mana_sphere_from_death(902, Position::default(), 0, 0x2222, 654, 111)
        .expect("second NewEvent allocation");
    assert_eq!(second_id, u32::from(second_slot));
    let second_seed = second_slot.wrapping_add(pass_two_global as u16);
    let second_rng = level2_next_rng16(second_seed);
    let second_speed_rng = level2_next_rng16(second_rng);
    assert_eq!(
        session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.id == second_id)
            .expect("second sphere")
            .entity_rng,
        second_speed_rng
    );
    assert_ne!(pass_one_global, pass_two_global);

    session.reset();
    assert_eq!(session.economy().global_rng, baseline_global);
    assert!(session.economy().spheres().is_empty());
}

#[test]
fn level_two_mana_sphere_dynamic_handler_moves_and_updates_source_chain() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 sphere handler session");
    let sphere_slot = u16::try_from(
        session
            .economy_mut()
            .spawn_mana_sphere_from_death(901, Position::default(), 0, 0x3456, 10, 111)
            .expect("sphere allocation"),
    )
    .expect("sphere source slot fits u16");
    let before = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == sphere_slot)
        .copied()
        .expect("live sphere");
    {
        let mut economy_view = session.economy_mut();
        let sphere = economy_view
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == sphere_slot)
            .expect("mutable live sphere");
        sphere.velocity_x = 32;
        sphere.velocity_y = 0;
        sphere.vertical_velocity = 0;
    }
    assert!(session.economy_mut().step_mana_sphere_slot(sphere_slot));
    let after = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == sphere_slot)
        .copied()
        .expect("stepped sphere");
    assert_eq!(after.phase, before.phase.wrapping_add(1));
    assert_eq!(after.position.x, before.position.x.wrapping_add(32));
    assert_eq!(
        session
            .economy()
            .map_chains()
            .entity_chain(
                ((after.position.x.wrapping_add(128)) >> 8) as u8,
                ((after.position.y.wrapping_add(128)) >> 8) as u8
            )
            .first(),
        Some(&sphere_slot)
    );
}

#[test]
fn level_two_mana_carrier_dynamic_handler_collects_and_reclaims_sphere() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 carrier handler session");
    let baseline_hash = session.economy().reset_hash();
    let carrier_slot = session
        .economy_mut()
        .spawn_mana_carrier(Position::default(), 111, 0)
        .expect("carrier allocation");
    let sphere_slot = session
        .economy_mut()
        .spawn_mana_sphere_from_death(901, Position::default(), 0, 0x3456, 10, 111)
        .expect("sphere allocation");
    let sphere_id = sphere_slot;
    // The raw AddBallon target relation is the carrier's owner field;
    // `ManaCarrier::id` is the presentation/source slot and differs.
    assert_ne!(session.economy().carrier().expect("live carrier").id, 111);
    session
        .economy_mut()
        .carriers
        .iter_mut()
        .find(|carrier| carrier.source_entity_index == carrier_slot)
        .expect("live carrier")
        .target_sphere_id = sphere_id;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let carrier = session.economy().carrier().expect("live carrier");
    assert_eq!(carrier.source_entity_index, carrier_slot);
    assert_eq!(carrier.carried_mana, 10);
    let sphere_slot = u16::try_from(sphere_slot).expect("sphere slot fits");
    let hidden_sphere = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == sphere_slot)
        .copied()
        .expect("collected sphere remains allocated through the callback pass");
    assert_ne!(hidden_sphere.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
    assert!(hidden_sphere.pending_delete);
    assert!(session.economy().map_chains().is_linked(sphere_slot));
    assert!(!session.economy().free_slots.contains(&sphere_slot));
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::ManaSphere,
                ..
            } if *source_slot == sphere_slot
        )
    }));
    assert!(session.economy().events().iter().any(|event| {
        matches!(
            event,
            Level2EconomyEvent::ManaSphereCollected { carrier_id, mana, .. }
                if *carrier_id == u32::from(carrier_slot) && *mana == 10
        )
    }));
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .spheres()
            .iter()
            .all(|sphere| sphere.source_entity_index != sphere_slot)
    );
    assert!(!session.economy().map_chains().is_linked(sphere_slot));
    assert!(session.economy().free_slots.contains(&sphere_slot));
    assert_ne!(session.economy().reset_hash(), baseline_hash);
    session.reset();
    assert_eq!(session.economy().reset_hash(), baseline_hash);
}

#[test]
fn level_two_mana_carrier_arrival_clears_return_target_once() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 carrier arrival session");
    let castle_slot = session
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("Castle allocation");
    let castle = session.economy().castle().expect("live Castle");
    let carrier_slot = session
        .economy_mut()
        .spawn_mana_carrier(castle.position, 111, u32::from(castle_slot))
        .expect("carrier allocation");
    {
        let mut economy_view = session.economy_mut();
        let carrier = economy_view
            .carriers
            .iter_mut()
            .find(|carrier| carrier.source_entity_index == carrier_slot)
            .expect("live carrier");
        carrier.returning_to_castle = true;
        carrier.target_sphere_id = u32::from(castle_slot);
        carrier.carried_mana = 7;
    }

    session.economy_mut().begin_turn();
    assert!(session.economy_mut().step_mana_carrier_slot(carrier_slot));
    let carrier = session.economy().carrier().expect("arrived carrier");
    assert!(!carrier.returning_to_castle);
    assert_eq!(carrier.target_sphere_id, 0);
    assert_eq!(carrier.carried_mana, 0);
    assert_eq!(session.economy().castle().expect("Castle").mana, 7);
    assert_eq!(
        session
            .economy()
            .events()
            .iter()
            .filter(|event| matches!(event, Level2EconomyEvent::ManaCarrierArrived { .. }))
            .count(),
        1
    );

    session.economy_mut().begin_turn();
    assert!(session.economy_mut().step_mana_carrier_slot(carrier_slot));
    assert!(session.economy().events().is_empty());
}

#[test]
fn level_two_mana_carrier_bank_keeps_multiple_source_rows_isolated() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 carrier-bank session");
    let baseline_hash = session.economy().reset_hash();
    let first_slot = session
        .economy_mut()
        .spawn_mana_carrier(Position::default(), 111, 0)
        .expect("first carrier allocation");
    let second_slot = session
        .economy_mut()
        .spawn_mana_carrier(Position::default(), 112, 0)
        .expect("second carrier allocation");
    assert_eq!(session.economy().carriers().len(), 2);
    assert!(
        session
            .economy()
            .live_arena_slots()
            .contains(&(first_slot, Level2DynamicArenaKind::ManaCarrier))
    );
    assert!(
        session
            .economy()
            .live_arena_slots()
            .contains(&(second_slot, Level2DynamicArenaKind::ManaCarrier))
    );

    assert!(session.economy_mut().step_mana_carrier_slot(first_slot));
    assert!(session.economy_mut().step_mana_carrier_slot(second_slot));
    assert!(
        session
            .economy_mut()
            .submit_damage(first_slot, MANA_CARRIER_MAX_LIFE + 1, 77)
    );
    assert_eq!(session.economy_mut().resolve_damage(first_slot), Some(true));
    session.economy_mut().reclaim_inactive_slots();
    assert!(
        session
            .economy()
            .carriers()
            .iter()
            .all(|carrier| carrier.source_entity_index != first_slot)
    );
    assert!(
        session
            .economy()
            .carriers()
            .iter()
            .any(|carrier| carrier.source_entity_index == second_slot)
    );
    assert!(
        session
            .economy()
            .live_arena_slots()
            .contains(&(second_slot, Level2DynamicArenaKind::ManaCarrier))
    );

    session.reset();
    assert!(session.economy().carriers().is_empty());
    assert_eq!(session.economy().reset_hash(), baseline_hash);
}

#[test]
fn level_two_mana_carrier_outside_attraction_retries_target_and_releases_claim() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 carrier retry session");
    let carrier_slot = session
        .economy_mut()
        .spawn_mana_carrier(Position::default(), 111, 0)
        .expect("carrier allocation");
    let sphere_slot = session
        .economy_mut()
        .spawn_mana_sphere_from_death(
            901,
            Position {
                x: 4_096,
                y: 0,
                z: 0,
            },
            0,
            0x3456,
            10,
            111,
        )
        .expect("sphere allocation");
    session
        .economy_mut()
        .carriers
        .iter_mut()
        .find(|carrier| carrier.source_entity_index == carrier_slot)
        .expect("live carrier")
        .target_sphere_id = sphere_slot;
    session
        .economy_mut()
        .spheres
        .iter_mut()
        .find(|sphere| sphere.source_entity_index == u16::try_from(sphere_slot).unwrap())
        .expect("live sphere")
        .assigned_carrier_id = u32::from(carrier_slot);

    assert!(session.economy_mut().step_mana_carrier_slot(carrier_slot));
    let carrier = session.economy().carrier().expect("live carrier");
    assert_eq!(carrier.target_sphere_id, sphere_slot);
    assert_eq!(
        session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == u16::try_from(sphere_slot).unwrap())
            .expect("live sphere")
            .assigned_carrier_id,
        0
    );
}

#[test]
fn level_two_spell_replacement_uses_shared_arena_and_outer_cursor() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 spell-replacement session");
    let baseline_hash = session.economy().reset_hash();
    let first_free = *session
        .economy()
        .free_slots
        .last()
        .expect("source free slot");
    let consumed_slot = session
        .economy_mut()
        .spawn_mana_carrier(Position::default(), 111, 0)
        .expect("shared arena carrier allocation");
    assert_eq!(consumed_slot, first_free);
    let expected_slot = *session
        .economy()
        .free_slots
        .last()
        .expect("next source free slot");
    let global_rng = session.economy().global_rng;
    let position = Position {
        x: 2_048,
        y: 4_096,
        z: 128,
    };
    let replacement = session
        .allocate_level2_spell_replacement(11, 38, position)
        .expect("family-11 replacement allocation");
    assert_eq!(replacement.source_entity_index, expected_slot);
    assert_eq!(
        replacement.entity_rng,
        expected_slot.wrapping_add(global_rng as u16)
    );
    assert_eq!(replacement.parent_entity_index, 38);
    assert_eq!(replacement.position, position);
    assert_eq!(replacement.family, 11);
    assert_eq!(replacement.action, 35);
    assert_eq!(replacement.phase, expected_slot as u8);
    assert_eq!(replacement.control_flags, 0);
    assert!(replacement.active);
    assert!(
        session
            .economy()
            .map_chains()
            .linked_entity_indices()
            .contains(&expected_slot)
    );
    assert!(
        session
            .economy()
            .live_arena_slots()
            .contains(&(expected_slot, Level2DynamicArenaKind::SpellReplacement))
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::DynamicArena {
            source_slot,
            kind: Level2DynamicArenaKind::SpellReplacement,
            ..
        } if *source_slot == expected_slot
    )));
    assert_eq!(
        session
            .economy()
            .spell_replacements()
            .iter()
            .find(|row| row.source_entity_index == expected_slot)
            .expect("live replacement")
            .phase,
        replacement.phase.wrapping_add(1)
    );

    session
        .economy_mut()
        .spell_replacements
        .iter_mut()
        .find(|row| row.source_entity_index == expected_slot)
        .expect("replacement to reclaim")
        .active = false;
    session.economy_mut().reclaim_inactive_slots();
    assert!(session.economy().spell_replacements().is_empty());
    assert!(session.economy().free_slots.contains(&expected_slot));
    assert!(
        !session
            .economy()
            .map_chains()
            .linked_entity_indices()
            .contains(&expected_slot)
    );

    session.economy_mut().free_slots.clear();
    let rows_before_failure = session.economy().spell_replacements().len();
    assert!(
        session
            .allocate_level2_spell_replacement(6, 155, position)
            .is_none()
    );
    assert_eq!(
        session.economy().spell_replacements().len(),
        rows_before_failure
    );
    session.reset();
    assert_eq!(session.economy().reset_hash(), baseline_hash);
    assert!(session.economy().spell_replacements().is_empty());
    assert!(session.economy().carriers().is_empty());
}

#[test]
fn level_two_shared_spell_census_rejects_tampered_rows_and_presentation() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let players = inspection.players();
    let mut entities = inspection.entities().to_vec();
    entities[38].record[68] = 3;
    assert!(
        Level2SharedSpellResetSession::from_parts_with_starting_spells(
            SessionTerrain::from_inspection(&inspection),
            &entities,
            players,
            inspection.presentation(),
            None,
        )
        .is_err()
    );
    let mut entities = inspection.entities().to_vec();
    entities[113].record[28] ^= 1;
    assert!(
        Level2SharedSpellResetSession::from_parts_with_starting_spells(
            SessionTerrain::from_inspection(&inspection),
            &entities,
            players,
            inspection.presentation(),
            None,
        )
        .is_err()
    );
    let mut entities = inspection.entities().to_vec();
    entities[75].record[73] = 9;
    assert!(
        Level2SharedSpellResetSession::from_parts_with_starting_spells(
            SessionTerrain::from_inspection(&inspection),
            &entities,
            players,
            inspection.presentation(),
            None,
        )
        .is_err()
    );
    let mut presentation = inspection.presentation().clone();
    presentation.level_sprite_selectors[59] = 0;
    assert!(
        Level2SharedSpellResetSession::from_parts_with_starting_spells(
            SessionTerrain::from_inspection(&inspection),
            inspection.entities(),
            players,
            &presentation,
            None,
        )
        .is_err()
    );
}

#[test]
fn player_fireball_crosses_the_shared_arena_into_goat_death_and_one_mana_sphere() {
    let (package, closure) = fixture_paths();
    let world = NativeWorld::load(package, closure).expect("validated package M");
    let mut session = world.session();
    let goat = session
        .live_world
        .goats()
        .iter()
        .find(|goat| goat.active)
        .expect("Level 1 has an active Goat")
        .clone();

    session.state.position = Position {
        x: goat.position.x,
        y: goat.position.y.wrapping_add(384),
        z: goat.position.z.wrapping_sub(100),
    };
    session.state.yaw = 0;
    session.state.effective_pitch = 0;
    session.mission.has_fireball = true;
    session.mission.player_mana = 1_000;
    for _ in 0..3 {
        assert!(session.mission.spawn_fireball(&session.state));
    }

    for _ in 0..24 {
        session.step(PlayerCommand::default());
        if session
            .live_world
            .goats()
            .iter()
            .find(|candidate| candidate.entity_index == goat.entity_index)
            .is_some_and(|candidate| !candidate.active)
        {
            break;
        }
    }
    let retired = session
        .live_world
        .goats()
        .iter()
        .find(|candidate| candidate.entity_index == goat.entity_index)
        .expect("typed Goat row remains addressable after retirement");
    assert!(!retired.active);
    let sphere_count = session.mission.mana_spheres().len();
    assert_eq!(sphere_count, 1);
    let sphere = session.mission.mana_spheres()[0];
    assert_eq!(sphere.mana, goat.mana);
    assert_eq!(sphere.id, u32::from(sphere.source_entity_index));
    assert_eq!(
        session.mission.source_slots()[usize::from(sphere.source_entity_index)].entity_rng,
        sphere.entity_rng
    );

    for _ in 0..8 {
        session.step(PlayerCommand::default());
    }
    assert_eq!(
        session.mission.mana_spheres().len(),
        sphere_count,
        "retiring the imported Goat must not emit duplicate mana"
    );
    assert!(
        !session
            .mission
            .imported_source_slot_is_live(goat.entity_index)
    );
}

#[test]
fn carrier_presentation_extent_mutations_fail_closed() {
    let (package, _) = fixture_paths();
    let original = fs::read(package.join("presentation.bin")).unwrap();
    let row = 12 + usize::from(MANA_CARRIER_RENDER_INDEX) * PARTICLE_RECORD_BYTES;
    for (offset, bytes) in [
        (6_usize, 544_u16.to_le_bytes()),
        (8_usize, 799_u16.to_le_bytes()),
    ] {
        let mut presentation = original.clone();
        presentation[row + offset..row + offset + 2].copy_from_slice(&bytes);
        let parsed = parse_presentation(&presentation).expect("well-formed mutation");
        let error = validate_level_one_mission_presentation(&parsed).unwrap_err();
        assert!(error.to_string().contains("169 mismatch"));
    }
    let mut presentation = original.clone();
    presentation[row + 11] = (-15_i8).cast_unsigned();
    let parsed = parse_presentation(&presentation).expect("well-formed mutation");
    let error = validate_level_one_mission_presentation(&parsed).unwrap_err();
    assert!(error.to_string().contains("169 mismatch"));

    for index in [
        POSSESSION_ATTRACTION_RENDER_INDEX,
        POSSESSION_IMPACT_RENDER_INDEX,
        GROUND_FIRE_FIRST_RENDER_INDEX,
        GROUND_FIRE_FIRST_RENDER_INDEX + 7,
    ] {
        let mut presentation = original.clone();
        let row = 12 + usize::from(index) * PARTICLE_RECORD_BYTES;
        presentation[row] ^= 1;
        let parsed = parse_presentation(&presentation).expect("well-formed mutation");
        let error = validate_level_one_mission_presentation(&parsed).unwrap_err();
        assert!(
            error.to_string().contains(&format!("{index} mismatch")),
            "row {index} mutation reached the exact Level 1 presentation gate: {error}"
        );
    }
}

#[test]
fn level_two_direct_starting_spell_holders_materialize_from_package() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 direct starting-spell session");
    let baseline_hash = session.campaign_state_hash();
    let baseline_economy_hash = session.economy().reset_hash();
    let player_entity_index = session.shared_spells().local_player.entity_index;
    assert_eq!(
        &session.shared_spells().local_player.enabled_spell_holders[..4],
        &[115, 116, 117, 118]
    );
    assert_eq!(
        session
            .shared_spells()
            .dynamic_pickups
            .iter()
            .map(|holder| (holder.model, holder.action, holder.class))
            .collect::<Vec<_>>(),
        vec![(0, 0, 15), (1, 3, 15), (2, 6, 15), (3, 9, 15)]
    );
    assert!(
        session
            .shared_spells()
            .dynamic_pickups
            .iter()
            .all(|holder| holder.source_spell_parent_entity_index() == player_entity_index)
    );
    assert!(session.economy().spell_pickups().is_empty());
    for holder in &session.shared_spells().dynamic_pickups {
        let tier = u8::try_from(holder.level_gate).expect("starting holder tier is non-negative");
        let profile = level2_direct_starting_spell_profile(holder.model, tier)
            .expect("materialized holder has a bounded profile");
        assert_eq!(holder.source_id, holder.index);
        assert_eq!(
            u16::from_le_bytes([holder.record_bytes()[30], holder.record_bytes()[31]]),
            holder.index
        );
        assert_eq!(holder.class, 15);
        assert_eq!(holder.control_flags, 0x5);
        assert_eq!(holder.action, holder.model.saturating_mul(3));
        assert_eq!(holder.subspell_index, profile.subspell_index);
        assert_eq!(holder.activation_value, profile.word_0x30);
        assert_eq!(
            holder.source_spell_parent_entity_index(),
            player_entity_index
        );
        assert_eq!(holder.particle_parameter_index, Some(77));
        assert_eq!(holder.parameter_relation, Some(59));
        assert_eq!(holder.array_yaw(), 75);
        assert_eq!(holder.array_pitch(), 768);
        assert_eq!(holder.array_roll(), 768);
        assert_eq!(holder.array_fov(), 1280);
        assert_eq!(holder.record_bytes()[94..96], 77_i16.to_le_bytes());
        assert_eq!(
            holder.old_map_entity(),
            session.economy().map_chains().old_map_entity(holder.index)
        );
        assert_eq!(
            holder.next_entity(),
            session.economy().map_chains().next_entity(holder.index)
        );
    }

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    assert_eq!(session.economy().reset_hash(), baseline_economy_hash);
    assert!(session.economy().spell_pickups().is_empty());
}

#[test]
fn level_two_direct_starting_spell_allocation_is_transactional_when_full() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mission = inspection
        .mission_content_template()
        .expect("Level 2 content template")
        .session();
    let mut economy = Level2EconomyState::from_parts_with_castle_stage_footprints(
        SessionTerrain::from_inspection(&inspection),
        inspection.entities(),
        inspection.entities()[usize::from(inspection.players()[0].entity_index)].mana,
        inspection.entities()[usize::from(inspection.players()[0].entity_index)].model,
        0,
        inspection.players()[0].enabled_spell_holders[2],
        inspection.runtime().random_state,
        inspection.level2_castle_stage_footprints.clone(),
        inspection.runtime.constructor_counters(),
    )
    .expect("Level 2 economy reset");
    economy.free_slots.clear();
    let before = economy.clone();
    let error = Level2SharedSpellResetSession::from_parts_with_starting_spells(
        SessionTerrain::from_inspection(&inspection),
        inspection.entities(),
        inspection.players(),
        inspection.presentation(),
        Some((&mut economy, &mission.wizard_map_settings()[0])),
    )
    .expect_err("an exhausted source arena must reject direct spell init");
    assert!(error.to_string().contains("could not allocate a holder"));
    assert_eq!(
        economy, before,
        "failed init must not consume allocator state"
    );

    let mut blocked_wizard = mission.wizard_map_settings()[0].clone();
    blocked_wizard.starting_spells[0] = 0;
    blocked_wizard.blocked_spells[1] = 1;
    let mut blocked_economy = Level2EconomyState::from_parts_with_castle_stage_footprints(
        SessionTerrain::from_inspection(&inspection),
        inspection.entities(),
        inspection.entities()[usize::from(inspection.players()[0].entity_index)].mana,
        inspection.entities()[usize::from(inspection.players()[0].entity_index)].model,
        0,
        inspection.players()[0].enabled_spell_holders[2],
        inspection.runtime().random_state,
        inspection.level2_castle_stage_footprints.clone(),
        inspection.runtime.constructor_counters(),
    )
    .expect("Level 2 economy reset for blocked/zero families");
    let blocked = Level2SharedSpellResetSession::from_parts_with_starting_spells(
        SessionTerrain::from_inspection(&inspection),
        inspection.entities(),
        inspection.players(),
        inspection.presentation(),
        Some((&mut blocked_economy, &blocked_wizard)),
    )
    .expect("blocked and zero families fail closed without poisoning init");
    assert_eq!(
        &blocked.local_player.enabled_spell_holders[..4],
        &[0, 0, 115, 116]
    );
    assert_eq!(
        blocked
            .dynamic_pickups
            .iter()
            .map(|holder| holder.model)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
}
