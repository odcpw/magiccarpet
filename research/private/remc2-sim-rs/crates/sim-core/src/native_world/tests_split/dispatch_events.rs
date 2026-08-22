#[test]
fn level_two_wind_sub33710_writes_strict_target_mailboxes_on_cadence() {
    assert!(level2_compare_axis_with_shift(
        Position {
            x: 10_000,
            y: 10_000,
            z: 0
        },
        38,
        31,
        Position {
            x: 10_000,
            y: 10_000,
            z: 0
        },
        123,
        123,
    ));
    assert!(!level2_compare_axis_with_shift(
        Position {
            x: 10_000,
            y: 10_000,
            z: 0
        },
        38,
        31,
        Position {
            x: 10_161,
            y: 10_000,
            z: 0
        },
        123,
        123,
    ));
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 wind pulse session");
    let parent = session
        .economy_mut()
        .allocate_authored_model22(
            110,
            Position {
                x: 20_000,
                y: 20_000,
                z: 0,
            },
            Position {
                x: 20_000,
                y: 20_000,
                z: 0,
            },
        )
        .expect("wind parent allocation");
    let parent_slot = parent.source_entity_index;
    session
        .economy_mut()
        .authored_model22
        .iter_mut()
        .find(|source| source.source_entity_index == parent_slot)
        .expect("live wind parent")
        .phase = 8;

    // Row 1 is a qualified imported class-10/model-45 target. Rebind the
    // remote player row to model 2 so the same production scan exercises
    // the second `sub_33710` branch without adding a synthetic arena row.
    session.economy.entities[1].set_position(parent.position);
    session.economy.entities[112].model = 2;
    session.economy.entities[112].record[68] = 2;
    session.economy.entities[112].set_position(parent.position);

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.economy.entities[1].source_wind_spell_payload(), 1_000);
    assert_eq!(
        session.economy.entities[1].source_wind_spell_owner(),
        parent.entity_id
    );
    assert_eq!(session.economy.entities[112].source_spell_word_30(), 30);
    assert_eq!(session.economy.entities[112].activation_value, 30);
    assert_eq!(session.economy.entities[112].word_26(), parent_slot);
    assert_eq!(session.economy.entities[112].source_wind_spell_payload(), 1_000);
    assert_eq!(
        session.economy.entities[112].source_wind_spell_owner(),
        parent.entity_id
    );

    // The model-45 branch is `sub_11900`: a non-zero owner replaces its
    // payload on the next firing cadence rather than accumulating it.
    session.economy.entities[1].record[98..102].copy_from_slice(&7_i32.to_le_bytes());
    session.economy.entities[1].record[102..104].copy_from_slice(&55_u16.to_le_bytes());
    let next_parent_position = session
        .economy()
        .authored_model22()
        .iter()
        .find(|source| source.source_entity_index == parent_slot)
        .expect("live wind parent")
        .position;
    session.economy.entities[1].set_position(next_parent_position);
    session.economy.entities[112].set_position(next_parent_position);
    session
        .economy_mut()
        .authored_model22
        .iter_mut()
        .find(|source| source.source_entity_index == parent_slot)
        .expect("live wind parent")
        .phase = 16;
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.economy.entities[1].source_wind_spell_payload(), 1_000);
    assert_eq!(
        session.economy.entities[1].source_wind_spell_owner(),
        parent.entity_id
    );

    // A non-firing phase leaves both target mailboxes untouched.
    let payload_before = session.economy.entities[1].source_wind_spell_payload();
    let owner_before = session.economy.entities[1].source_wind_spell_owner();
    session
        .economy_mut()
        .authored_model22
        .iter_mut()
        .find(|source| source.source_entity_index == parent_slot)
        .expect("live wind parent")
        .phase = 17;
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        session.economy.entities[1].source_wind_spell_payload(),
        payload_before
    );
    assert_eq!(session.economy.entities[1].source_wind_spell_owner(), owner_before);
}

#[test]
fn level_two_wind_sub33340_filters_and_moves_model0_target_on_production_turn() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 wind movement session");
    let parent = session
        .economy_mut()
        .allocate_authored_model22(
            110,
            Position {
                x: 20_000,
                y: 20_000,
                z: 0,
            },
            Position {
                x: 20_000,
                y: 20_000,
                z: 0,
            },
        )
        .expect("wind parent allocation");
    let target_index = 112_u16;
    let parent_source_id = parent.entity_id;
    let spell_experience_before = *session.shared_spells().spell_experience();
    let (target_snapshot, target_speed_before) = {
        let target = session.economy.entities
            .get_mut(usize::from(target_index))
            .expect("imported player target");
        assert_eq!(target.class, 3);
        target.model = 0;
        target.record[68] = 0;
        target.source_id = parent_source_id.wrapping_add(1);
        target.record[30..32].copy_from_slice(&target.source_id.to_le_bytes());
        target.set_position(parent.position);
        target.set_yaw(0);
        target.set_control_flags(0x1000_0000);
        (target.clone(), target.actual_speed)
    };
    session
        .economy_mut()
        .map_chains
        .sync_entity(RenderableMapSeed {
            entity_index: target_index,
            active: true,
            x: parent.position.x,
            y: parent.position.y,
        })
        .expect("target map chain follows its source position");

    assert!(level2_wind_sub33810_accepts(
        parent_source_id,
        &target_snapshot
    ));
    let mut rejected = target_snapshot.clone();
    rejected.model = 2;
    rejected.record[68] = 2;
    assert!(!level2_wind_sub33810_accepts(parent_source_id, &rejected));
    session.step_outer_turn(PlayerCommand::default(), 0);
    let target = &session.economy.entities[usize::from(target_index)];
    assert_ne!(target.yaw, 0);
    assert_eq!(target.actual_speed, target_speed_before);
    assert_ne!(target.control_flags & 0x0000_0800, 0);
    assert_eq!(
        target.source_wind_spell_payload(),
        i32::from(parent.subspell_index)
    );
    assert_eq!(target.source_wind_spell_owner(), parent.entity_id);
    assert!(session.economy().map_chains().is_linked(target_index));
    // `sub_33340` terminates with `sub_6D8B0(parent_id, 0x15, count)`,
    // but AddWind's parent is class-10/model-22. The source helper only
    // awards a player entity (class 3/model 0), so this production route
    // must remain an observable no-op rather than inventing XP for the
    // wind parent.
    assert_eq!(
        *session.shared_spells().spell_experience(),
        spell_experience_before
    );

    let player_slot = match target_snapshot.special_relation {
        EntitySpecialRelation::PlayerState(slot) => slot,
        relation => panic!("model-0 target must retain player-state relation: {relation:?}"),
    };
    let roll_before = session
        .players
        .iter()
        .find(|player| player.slot == player_slot)
        .expect("target player-state row")
        .roll_0x155();
    let current_parent = session
        .economy()
        .authored_model22()
        .iter()
        .find(|source| source.source_entity_index == parent.source_entity_index)
        .copied()
        .expect("wind parent after first callback");
    let second_target_position = Position {
        x: current_parent.position.x.wrapping_add(1_024),
        y: current_parent.position.y,
        z: current_parent.position.z,
    };
    session.economy.entities[usize::from(target_index)].set_control_flags(0);
    session.economy.entities[usize::from(target_index)].set_position(second_target_position);
    session
        .economy_mut()
        .map_chains
        .sync_entity(RenderableMapSeed {
            entity_index: target_index,
            active: true,
            x: second_target_position.x,
            y: second_target_position.y,
        })
        .expect("second target map chain follows its source position");
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.economy.entities[usize::from(target_index)].actual_speed, 80);
    let roll_after = session
        .players
        .iter()
        .find(|player| player.slot == player_slot)
        .expect("target player-state row after callback")
        .roll_0x155();
    assert_eq!(roll_after, roll_before.saturating_add(28));

    // Non-zero class-3 models use the source's wider v38/v34 bands but
    // do not receive model-0 player roll/speed feedback.
    let current_parent = session
        .economy()
        .authored_model22()
        .iter()
        .find(|source| source.source_entity_index == parent.source_entity_index)
        .copied()
        .expect("wind parent after second callback");
    let nonzero_model_position = Position {
        x: current_parent.position.x.wrapping_add(1_024),
        y: current_parent.position.y,
        z: current_parent.position.z,
    };
    {
        let target = &mut session.economy.entities[usize::from(target_index)];
        target.model = 1;
        target.record[68] = 1;
        target.set_actual_speed(17);
        target.set_yaw(0);
        target.set_control_flags(0);
        target.set_position(nonzero_model_position);
    }
    session
        .economy_mut()
        .map_chains
        .sync_entity(RenderableMapSeed {
            entity_index: target_index,
            active: true,
            x: nonzero_model_position.x,
            y: nonzero_model_position.y,
        })
        .expect("non-zero model target map chain follows its source position");
    let nonzero_roll_before = session
        .players
        .iter()
        .find(|player| player.slot == player_slot)
        .expect("non-zero model target player-state row")
        .roll_0x155();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let nonzero_model_target = &session.economy.entities[usize::from(target_index)];
    // Source-slot 112 runs Nyphur sub_12910/sub_12A70 before the later
    // wind-parent callback: sub_12A70 changes 17 to 1, and nonzero-model
    // sub_33340 preserves that combined full-turn value.
    assert_eq!(nonzero_model_target.actual_speed, 1);
    assert_eq!(nonzero_model_target.yaw, 204);
    assert_ne!(nonzero_model_target.control_flags & 0x0000_0800, 0);
    assert_eq!(
        session
            .players
            .iter()
            .find(|player| player.slot == player_slot)
            .expect("non-zero model target player-state row after callback")
            .roll_0x155(),
        nonzero_roll_before
    );

    // The class-10 allowlist in sub_33810 is independent of the
    // class-3 model check. Model 39 takes the same non-player movement
    // branch and still receives the source hit mailbox when airborne.
    let class10_parent = session
        .economy()
        .authored_model22()
        .iter()
        .find(|source| source.source_entity_index == parent.source_entity_index)
        .copied()
        .expect("wind parent after non-zero model callback");
    let class10_position = Position {
        x: class10_parent.position.x.wrapping_add(1_024),
        y: class10_parent.position.y,
        z: class10_parent.position.z,
    };
    {
        let target = &mut session.economy.entities[usize::from(target_index)];
        target.class = 10;
        target.record[67] = 10;
        target.model = 39;
        target.record[68] = 39;
        target.set_actual_speed(23);
        target.set_yaw(0);
        target.set_control_flags(0x1000_0000);
        target.set_position(class10_position);
    }
    session
        .economy_mut()
        .map_chains
        .sync_entity(RenderableMapSeed {
            entity_index: target_index,
            active: true,
            x: class10_position.x,
            y: class10_position.y,
        })
        .expect("class-10 target map chain follows its source position");
    let class10_snapshot = session.economy.entities[usize::from(target_index)].clone();
    assert!(level2_wind_sub33810_accepts(
        parent_source_id,
        &class10_snapshot
    ));
    let mut rejected_class10 = class10_snapshot.clone();
    rejected_class10.model = 45;
    rejected_class10.record[68] = 45;
    assert!(!level2_wind_sub33810_accepts(
        parent_source_id,
        &rejected_class10
    ));
    session.step_outer_turn(PlayerCommand::default(), 0);
    let class10_target = &session.economy.entities[usize::from(target_index)];
    assert_eq!(class10_target.actual_speed, 23);
    assert_eq!(class10_target.yaw, 204);
    assert_ne!(class10_target.control_flags & 0x0000_0800, 0);
    assert_eq!(
        class10_target.source_wind_spell_payload(),
        i32::from(parent.subspell_index)
    );
    assert_eq!(class10_target.source_wind_spell_owner(), parent.entity_id);

    // The remaining class-2 allowlist is just models 7/8 and shares the
    // same non-v40 movement branch.
    let class2_parent = session
        .economy()
        .authored_model22()
        .iter()
        .find(|source| source.source_entity_index == parent.source_entity_index)
        .copied()
        .expect("wind parent after class-10 callback");
    let class2_position = Position {
        x: class2_parent.position.x.wrapping_add(1_024),
        y: class2_parent.position.y,
        z: class2_parent.position.z,
    };
    {
        let target = &mut session.economy.entities[usize::from(target_index)];
        target.class = 2;
        target.record[67] = 2;
        target.model = 7;
        target.record[68] = 7;
        target.action = 0;
        target.record[73] = 0;
        target.set_actual_speed(31);
        target.set_yaw(0);
        target.set_control_flags(0);
        target.set_position(class2_position);
    }
    session
        .economy_mut()
        .map_chains
        .sync_entity(RenderableMapSeed {
            entity_index: target_index,
            active: true,
            x: class2_position.x,
            y: class2_position.y,
        })
        .expect("class-2 target map chain follows its source position");
    let class2_snapshot = session.economy.entities[usize::from(target_index)].clone();
    assert!(level2_wind_sub33810_accepts(
        parent_source_id,
        &class2_snapshot
    ));
    let mut rejected_class2 = class2_snapshot.clone();
    rejected_class2.model = 6;
    rejected_class2.record[68] = 6;
    assert!(!level2_wind_sub33810_accepts(
        parent_source_id,
        &rejected_class2
    ));
    session.step_outer_turn(PlayerCommand::default(), 0);
    let class2_target = &session.economy.entities[usize::from(target_index)];
    assert_eq!(class2_target.actual_speed, 31);
    assert_eq!(class2_target.yaw, 204);
    assert_ne!(class2_target.control_flags & 0x0000_0800, 0);
    assert_eq!(class2_target.source_wind_spell_owner(), parent.entity_id);

    // Class 5 admits model 13 when its action is not one of the two
    // source-excluded action values; action 180 must remain untouched.
    let class5_parent = session
        .economy()
        .authored_model22()
        .iter()
        .find(|source| source.source_entity_index == parent.source_entity_index)
        .copied()
        .expect("wind parent after class-2 callback");
    let class5_position = Position {
        x: class5_parent.position.x.wrapping_add(1_024),
        y: class5_parent.position.y,
        z: class5_parent.position.z,
    };
    {
        let target = &mut session.economy.entities[usize::from(target_index)];
        target.class = 5;
        target.record[67] = 5;
        target.model = 13;
        target.record[68] = 13;
        target.action = 0;
        target.record[73] = 0;
        target.set_actual_speed(37);
        target.set_yaw(0);
        target.set_control_flags(0);
        target.set_position(class5_position);
    }
    session
        .economy_mut()
        .map_chains
        .sync_entity(RenderableMapSeed {
            entity_index: target_index,
            active: true,
            x: class5_position.x,
            y: class5_position.y,
        })
        .expect("class-5 target map chain follows its source position");
    let class5_snapshot = session.economy.entities[usize::from(target_index)].clone();
    assert!(level2_wind_sub33810_accepts(
        parent_source_id,
        &class5_snapshot
    ));
    let mut rejected_class5 = class5_snapshot.clone();
    rejected_class5.action = 180;
    rejected_class5.record[73] = 180;
    assert!(!level2_wind_sub33810_accepts(
        parent_source_id,
        &rejected_class5
    ));
    session.step_outer_turn(PlayerCommand::default(), 0);
    let class5_target = &session.economy.entities[usize::from(target_index)];
    assert_eq!(class5_target.actual_speed, 37);
    assert_eq!(class5_target.yaw, 204);
    assert_ne!(class5_target.control_flags & 0x0000_0800, 0);
    assert_eq!(class5_target.source_wind_spell_owner(), parent.entity_id);

    // Exercise the negative through the real production turn as well:
    // action 180 is rejected before any movement/mailbox mutation.
    let rejected_parent = session
        .economy()
        .authored_model22()
        .iter()
        .find(|source| source.source_entity_index == parent.source_entity_index)
        .copied()
        .expect("wind parent after class-5 callback");
    let rejected_position = Position {
        x: rejected_parent.position.x.wrapping_add(1_024),
        y: rejected_parent.position.y,
        z: rejected_parent.position.z,
    };
    {
        let target = &mut session.economy.entities[usize::from(target_index)];
        target.action = 180;
        target.record[73] = 180;
        target.set_actual_speed(41);
        target.set_yaw(0);
        target.set_control_flags(0);
        target.set_position(rejected_position);
    }
    session
        .economy_mut()
        .map_chains
        .sync_entity(RenderableMapSeed {
            entity_index: target_index,
            active: true,
            x: rejected_position.x,
            y: rejected_position.y,
        })
        .expect("rejected class-5 target map chain follows its source position");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let rejected_target = &session.economy.entities[usize::from(target_index)];
    assert_eq!(rejected_target.actual_speed, 41);
    assert_eq!(rejected_target.yaw, 0);
    assert_eq!(rejected_target.control_flags & 0x0000_0800, 0);
}

#[test]
fn level_two_day_bee_l2_06_selector2_boundaries_and_handoff_are_source_shaped() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 selector-2 Bee session");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let source_slot = session
        .economy()
        .authored_bees()
        .iter()
        .find(|source| source.authored_index == 229)
        .map(|source| source.source_entity_index)
        .expect("ordinary group-21 row 229 Bee");
    let stage_target = Position {
        x: 0x6500,
        y: 0x1000,
        z: 0,
    };
    let configure = |session: &mut Level2ResetSession,
                     position: Position,
                     source_byte_3e: u8,
                     source_byte_39: i16| {
        let mut source = session
            .economy()
            .authored_bees()
            .iter()
            .find(|source| source.source_entity_index == source_slot)
            .copied()
            .expect("live ordinary Bee");
        assert_eq!(
            (source.action, source.stage_var1, source.stage_var2),
            (23, 6, 2)
        );
        source.position = position;
        source.source_byte_3e = source_byte_3e;
        source.source_byte_39 = source_byte_39;
        source.control_flags |= LEVEL2_BEE_MOVEMENT_SKIP_BIT;
        assert!(session.economy_mut().replace_authored_bee_for_test(source));
        source
    };

    // The signed, individual source casts make exactly +3072 inside the
    // point band, where selector 2 takes `sub_1E1C0` and one RNG draw.
    let inside = configure(
        &mut session,
        Position {
            x: stage_target.x.wrapping_add(3_072),
            ..stage_target
        },
        0,
        0,
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    let after_inside = session
        .economy()
        .authored_bees()
        .iter()
        .find(|source| source.source_entity_index == source_slot)
        .copied()
        .expect("inside-band Bee");
    let inside_helper_rng = level2_next_rng16(inside.entity_rng);
    let inside_tail_first = level2_next_rng16(inside_helper_rng);
    let inside_tail_second = level2_next_rng16(inside_tail_first);
    let inside_tail_direction = 2 * (u32::from(inside_tail_first % 0x9d) / 79);
    let expected_inside_roll = (i32::from(inside.roll)
        + i32::from(inside_helper_rng % 0x71)
        + 142
        + i32::from(inside_tail_second % 0x55)
            * (i32::try_from(inside_tail_direction).expect("Bee roll direction fits") - 1))
        as i16
        & 0x07ff;
    assert_eq!(after_inside.action, 23);
    assert_eq!(after_inside.stage_var2, 2);
    assert_eq!(after_inside.entity_rng, inside_tail_second);
    assert_eq!(after_inside.roll, expected_inside_roll);

    // At +3073 the callback takes `sub_1DDA0`: it steers to the authored
    // point and, at phase zero, consumes exactly its two source LCG draws.
    let outside = configure(
        &mut session,
        Position {
            x: stage_target.x.wrapping_add(3_073),
            ..stage_target
        },
        0,
        0,
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    let after_outside = session
        .economy()
        .authored_bees()
        .iter()
        .find(|source| source.source_entity_index == source_slot)
        .copied()
        .expect("outside-band Bee");
    let first = level2_next_rng16(outside.entity_rng);
    let second = level2_next_rng16(first);
    let direction = 2 * (u32::from(first % 0x9d) / 79);
    let helper_outside_roll = (i32::from(
        i16::try_from(heading_for_positions(outside.position, stage_target))
            .expect("Bee StageVar heading fits i16"),
    ) + (i32::from((second & 0xff) as u8) + 85)
        * (i32::try_from(direction).expect("Bee roll direction fits") - 1))
        as i16
        & 0x07ff;
    let outside_tail_first = level2_next_rng16(second);
    let outside_tail_second = level2_next_rng16(outside_tail_first);
    let outside_tail_direction = 2 * (u32::from(outside_tail_first % 0x9d) / 79);
    let expected_outside_roll = (i32::from(helper_outside_roll)
        + i32::from(outside_tail_second % 0x55)
            * (i32::try_from(outside_tail_direction).expect("Bee roll direction fits") - 1))
        as i16
        & 0x07ff;
    assert_eq!(after_outside.action, 23);
    assert_eq!(after_outside.stage_var2, 2);
    assert_eq!(after_outside.entity_rng, outside_tail_second);
    assert_eq!(after_outside.roll, expected_outside_roll);

    // Use the real local class-3 entity as the only visible target. The
    // first visit misses the 30-tick cadence, the second is hidden, and
    // the third reaches the source selector-10/action-18 handoff.
    let local_entity_index = session.state.entity_index;
    let local_index = usize::from(local_entity_index);
    for entity in &mut session.economy.entities {
        if entity.index != local_entity_index && entity.class == 3 {
            entity.control_flags |= 0x20;
        }
    }
    session.state.position = Position {
        x: stage_target.x.wrapping_add(1_024),
        ..stage_target
    };
    let local_target = session.state.position;
    let off_cadence = configure(&mut session, stage_target, 1, 1);
    let target_heading = i16::try_from(heading_for_positions(off_cadence.position, local_target))
        .expect("class-3 target heading fits");
    {
        let mut economy_view = session.economy_mut();
        let source = economy_view
            .authored_bees
            .iter_mut()
            .find(|source| source.source_entity_index == source_slot)
            .expect("ordinary Bee for cadence check");
        source.yaw = target_heading;
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    let after_off_cadence = session
        .economy()
        .authored_bees()
        .iter()
        .find(|source| source.source_entity_index == source_slot)
        .copied()
        .expect("off-cadence Bee");
    assert_eq!(
        (after_off_cadence.action, after_off_cadence.stage_var2),
        (23, 2)
    );
    assert_eq!(after_off_cadence.target_entity_index, None);

    session.economy.entities[local_index].control_flags |= 0x20;
    let hidden = configure(&mut session, stage_target, 0, 1);
    {
        let mut economy_view = session.economy_mut();
        let source = economy_view
            .authored_bees
            .iter_mut()
            .find(|source| source.source_entity_index == source_slot)
            .expect("ordinary Bee for hidden target");
        source.yaw = i16::try_from(heading_for_positions(hidden.position, local_target))
            .expect("hidden target heading fits");
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    let after_hidden = session
        .economy()
        .authored_bees()
        .iter()
        .find(|source| source.source_entity_index == source_slot)
        .copied()
        .expect("hidden-target Bee");
    assert_eq!((after_hidden.action, after_hidden.stage_var2), (23, 2));
    assert_eq!(after_hidden.target_entity_index, None);

    session.economy.entities[local_index].control_flags &= !0x20;
    let visible = configure(&mut session, stage_target, 0, 1);
    {
        let mut economy_view = session.economy_mut();
        let source = economy_view
            .authored_bees
            .iter_mut()
            .find(|source| source.source_entity_index == source_slot)
            .expect("ordinary Bee for visible target");
        source.yaw = i16::try_from(heading_for_positions(visible.position, local_target))
            .expect("visible target heading fits");
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    let handed_off = session
        .economy()
        .authored_bees()
        .iter()
        .find(|source| source.source_entity_index == source_slot)
        .copied()
        .expect("handed-off Bee");
    assert_eq!(handed_off.action, 18);
    assert_eq!(handed_off.stage_var2, 10);
    assert_eq!(handed_off.target_entity_index, Some(local_entity_index));
    assert_eq!(handed_off.dword_0x10, 1);
    assert_eq!(handed_off.entity_rng, level2_next_rng16(visible.entity_rng));

    // The existing action-18 path consumes target loss only after the
    // selector-defined handoff; no action or StageVar was seeded above.
    session.economy.entities[local_index].control_flags |= ENTITY_DRAW_DISABLED_BIT;
    {
        let mut economy_view = session.economy_mut();
        let source = economy_view
            .authored_bees
            .iter_mut()
            .find(|source| source.source_entity_index == source_slot)
            .expect("action-18 Bee before target loss");
        source.control_flags |= LEVEL2_BEE_MOVEMENT_SKIP_BIT;
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    let target_lost = session
        .economy()
        .authored_bees()
        .iter()
        .find(|source| source.source_entity_index == source_slot)
        .copied()
        .expect("target-lost Bee");
    assert_eq!(target_lost.action, 17);
    assert_eq!(target_lost.actual_speed, target_lost.minimum_speed);
}

#[test]
fn level_two_day_bee_l2_06_terminal_selector2_defers_action20_until_next_callback() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 terminal selector-2 Bee session");

    // The primary witness is still the ordinary row-228/group-21 route;
    // this test changes no action or StageVar field after construction.
    session.step_outer_turn(PlayerCommand::default(), 0);
    let source_slot = session
        .economy()
        .authored_bees()
        .iter()
        .find(|source| source.authored_index == 229)
        .map(|source| source.source_entity_index)
        .expect("ordinary group-21 row-229 Bee");
    let mut configured = session
        .economy()
        .authored_bees()
        .iter()
        .find(|source| source.source_entity_index == source_slot)
        .copied()
        .expect("live ordinary Bee");
    assert_eq!(
        (
            configured.action,
            configured.stage_var1,
            configured.stage_var2
        ),
        (23, 6, 2)
    );
    configured.position = Position {
        x: 0x2080,
        y: 0x1080,
        z: 0,
    };
    configured.yaw = 0;
    configured.roll = 0;
    configured.actual_speed = 512;
    configured.source_byte_3e = 1;
    configured.source_byte_39 = 0;
    configured.control_flags &= !(LEVEL2_BEE_MOVEMENT_SKIP_BIT | LEVEL2_BEE_OBSTRUCTION_BIT);
    assert!(
        session
            .economy_mut()
            .replace_authored_bee_for_test(configured)
    );

    // A vertical 0/255 terrain stripe makes every crossed tile source-
    // steep. This forces `sub_1B8C0`'s four retry headings to exhaust
    // without using a fabricated Bee or a direct terminal action.
    for y in 0..=u8::MAX {
        for x in 0..=u8::MAX {
            session.canonical_terrain_mut().planes.ground_heights[terrain_index(x, y)] =
                if x & 1 == 0 { 0 } else { u8::MAX };
        }
    }
    for heading in level2_bee_retry_headings(configured.yaw) {
        let mut candidate = configured.position;
        let ground = session.canonical_terrain().ground_height(candidate);
        if candidate.z > ground {
            candidate = Position {
                z: candidate.z.wrapping_add(LEVEL2_BEE_GROUND_RELAXATION),
                ..candidate
            };
        }
        if candidate.z <= ground.wrapping_add(LEVEL2_BEE_GROUND_CLEARANCE) {
            candidate = Position {
                z: ground.wrapping_add(LEVEL2_BEE_GROUND_CLEARANCE),
                ..candidate
            };
        }
        crate::move_entity(&mut candidate, heading, 0, configured.actual_speed);
        assert_ne!(
            level2_bee_source_tile(candidate),
            level2_bee_source_tile(configured.position),
            "each forced retry crosses into a source-slope tile"
        );
        assert!(
            level2_bee_slope_measure(session.canonical_terrain(), candidate) >= LEVEL2_BEE_TERRAIN_THRESHOLD,
            "each retry candidate is source-blocked"
        );
    }

    // The helper itself may make the Bee terminal, but `sub_1DDA0` picked
    // v2=0 before it called the helper. The first callback therefore
    // remains action 23 and only the normal outer epilogue advances.
    session.step_outer_turn(PlayerCommand::default(), 0);
    let helper_terminal = session
        .economy()
        .authored_bees()
        .iter()
        .find(|source| source.source_entity_index == source_slot)
        .copied()
        .expect("movement-terminal Bee remains live for the next callback");
    assert_eq!((helper_terminal.action, helper_terminal.life), (23, -1));
    assert_eq!(helper_terminal.position, configured.position);
    assert_eq!(helper_terminal.entity_rng, configured.entity_rng);
    assert_eq!(helper_terminal.roll, configured.roll);
    assert_eq!(
        (helper_terminal.source_byte_3e, helper_terminal.phase),
        (
            configured.source_byte_3e.wrapping_add(1),
            configured.phase.wrapping_add(1)
        )
    );

    // On the next visit both source helpers see entry-dead state and take
    // v2=2 before `sub_1B8C0`: action becomes 20 with no movement-helper
    // position/yaw/RNG/roll/control changes, while dispatcher epilogue
    // still advances the source cursor and phase.
    session.step_outer_turn(PlayerCommand::default(), 0);
    let entry_terminal = session
        .economy()
        .authored_bees()
        .iter()
        .find(|source| source.source_entity_index == source_slot)
        .copied()
        .expect("entry-terminal Bee remains for action-20 callback");
    assert_eq!((entry_terminal.action, entry_terminal.life), (20, -1));
    assert_eq!(
        (
            entry_terminal.position,
            entry_terminal.yaw,
            entry_terminal.roll,
            entry_terminal.entity_rng,
            entry_terminal.control_flags,
        ),
        (
            helper_terminal.position,
            helper_terminal.yaw,
            helper_terminal.roll,
            helper_terminal.entity_rng,
            helper_terminal.control_flags,
        )
    );
    assert_eq!(
        (entry_terminal.source_byte_3e, entry_terminal.phase),
        (
            helper_terminal.source_byte_3e.wrapping_add(1),
            helper_terminal.phase.wrapping_add(1)
        )
    );
}

#[test]
fn level_two_bee_action17_moves_before_acquisition_rng() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Bee movement session");
    let source = session
        .allocate_level2_authored_bee(210)
        .expect("authored Day Bee row 210");
    let mut configured = source;
    configured.position = Position {
        x: 0x1000,
        y: 0x1010,
        z: 0,
    };
    configured.yaw = 0;
    configured.roll = 0;
    configured.action = 17;
    configured.source_byte_3e = 0;
    configured.source_byte_39 = 0;
    configured.control_flags = 8;
    configured.entity_rng = 0x1234;
    assert!(session.economy.replace_authored_bee_for_test(configured));

    let ground = session.canonical_terrain().ground_height(configured.position);
    let mut expected_position = configured.position;
    if expected_position.z > ground {
        expected_position.z = expected_position
            .z
            .wrapping_add(LEVEL2_BEE_GROUND_RELAXATION);
    }
    if expected_position.z <= ground.wrapping_add(LEVEL2_BEE_GROUND_CLEARANCE) {
        expected_position.z = ground.wrapping_add(LEVEL2_BEE_GROUND_CLEARANCE);
    }
    crate::move_entity(
        &mut expected_position,
        configured.yaw,
        0,
        configured.actual_speed,
    );
    let first = level2_next_rng16(configured.entity_rng);
    let second = level2_next_rng16(first);
    let direction = 2 * (u32::from(first % 0x9d) / 79);
    let expected_roll = (i32::from(configured.roll)
        + (i32::from((second & 0xff) as u8) + 85)
            * (i32::try_from(direction).expect("Bee roll direction fits") - 1))
        as i16
        & 0x07ff;

    // The cadence is active, but byte_0x39 suppresses only the target
    // scan. The source must still move first and consume both wander
    // draws after that movement.
    let terrain = session.canonical_terrain().clone();
    assert!(
        session
            .economy
            .acquire_authored_bee_target(configured.source_entity_index, &[], &terrain)
            .is_none()
    );
    let moved = session
        .economy
        .authored_bees
        .iter()
        .find(|bee| bee.source_entity_index == configured.source_entity_index)
        .copied()
        .expect("action-17 Bee remains live");
    assert_eq!(moved.position, expected_position);
    assert_eq!(moved.entity_rng, second);
    assert_eq!(moved.roll, expected_roll);
    assert_eq!(moved.control_flags, 8);
    assert_eq!(moved.action, 17);
}

#[test]
fn level_two_bee_action17_blocked_heading_retries_source_order() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Bee retry session");
    let source = session
        .allocate_level2_authored_bee(210)
        .expect("authored Day Bee row 210");
    let mut configured = source;
    configured.position = Position {
        x: 0x20f0,
        y: 0x1010,
        z: 0,
    };
    configured.yaw = 0;
    configured.roll = 0;
    configured.action = 17;
    configured.source_byte_3e = 1;
    configured.source_byte_39 = 0;
    configured.control_flags = 8;
    assert!(session.economy.replace_authored_bee_for_test(configured));

    let headings = level2_bee_retry_headings(configured.yaw);
    let candidate_tile = |heading: i16| {
        let mut candidate = configured.position;
        crate::move_entity(&mut candidate, heading, 0, configured.actual_speed);
        level2_bee_source_tile(candidate)
    };
    let first_tile = candidate_tile(headings[0]);
    let second_tile = candidate_tile(headings[1]);
    assert_ne!(
        first_tile, second_tile,
        "retry candidates must cross distinct tiles"
    );

    // Give the first candidate a source-slope rejection while leaving the
    // second candidate flat. The shared edge is kept at 255 so the first
    // tile's diagonal remains steep and the second tile remains level.
    let (first_x, first_y) = first_tile;
    let (second_x, second_y) = second_tile;
    for (x, y) in [
        (second_x, second_y),
        (second_x.wrapping_add(1), second_y),
        (second_x.wrapping_add(1), second_y.wrapping_add(1)),
        (second_x, second_y.wrapping_add(1)),
    ] {
        session.canonical_terrain_mut().planes.ground_heights[terrain_index(x, y)] = 255;
    }
    for (x, y, height) in [
        (first_x, first_y, 0),
        (first_x.wrapping_add(1), first_y, 255),
        (first_x.wrapping_add(1), first_y.wrapping_add(1), 255),
        (first_x, first_y.wrapping_add(1), 0),
    ] {
        session.canonical_terrain_mut().planes.ground_heights[terrain_index(x, y)] = height;
    }

    let mut expected_position = configured.position;
    let ground = session.canonical_terrain().ground_height(expected_position);
    if expected_position.z > ground {
        expected_position.z = expected_position
            .z
            .wrapping_add(LEVEL2_BEE_GROUND_RELAXATION);
    }
    if expected_position.z <= ground.wrapping_add(LEVEL2_BEE_GROUND_CLEARANCE) {
        expected_position.z = ground.wrapping_add(LEVEL2_BEE_GROUND_CLEARANCE);
    }
    crate::move_entity(
        &mut expected_position,
        headings[1],
        0,
        configured.actual_speed,
    );
    let expected_yaw =
        headings[1].wrapping_add(level2_bee_turn_delta(headings[1], configured.roll)) & 0x07ff;

    // Non-cadence still exercises the movement prelude but consumes no
    // wander RNG, isolating retry order from acquisition bookkeeping.
    let terrain = session.canonical_terrain().clone();
    assert!(
        session
            .economy
            .acquire_authored_bee_target(configured.source_entity_index, &[], &terrain)
            .is_none()
    );
    let retried = session
        .economy
        .authored_bees
        .iter()
        .find(|bee| bee.source_entity_index == configured.source_entity_index)
        .copied()
        .expect("retried Bee remains live");
    assert_eq!(retried.position, expected_position);
    assert_eq!(retried.yaw, expected_yaw);
    assert_ne!(retried.control_flags & LEVEL2_BEE_OBSTRUCTION_BIT, 0);
    assert_eq!(retried.entity_rng, configured.entity_rng);
}

#[test]
fn level_two_bee_action18_moves_and_steers_off_cadence_without_damage_or_rng() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Bee action-18 movement session");
    let source = session
        .allocate_level2_authored_bee(210)
        .expect("authored Day Bee row 210");
    let mut configured = source;
    configured.action = 18;
    configured.source_byte_3e = 4;
    configured.dword_0x10 = 0;
    configured.pitch = 512;
    configured.target_entity_index = Some(session.state.entity_index);
    configured.control_flags &= !LEVEL2_BEE_MOVEMENT_SKIP_BIT;
    configured.position = Position {
        x: 0x1200,
        y: 0x1200,
        z: 0,
    };
    let target = Position {
        x: configured.position.x.wrapping_add(1024),
        y: configured.position.y,
        z: configured.position.z,
    };
    assert!(session.economy.replace_authored_bee_for_test(configured));
    let before = configured;
    assert!(
        session
            .economy_mut()
            .step_authored_bee_slot(before.source_entity_index, Some(target))
    );
    let after = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == before.source_entity_index)
        .copied()
        .expect("off-cadence action-18 Bee remains live");
    assert_eq!(after.action, 18);
    assert_eq!(after.entity_rng, before.entity_rng);
    assert_ne!(after.position, before.position);
    assert_eq!(
        after.roll,
        i16::try_from(heading_for_positions(after.position, target))
            .expect("Bee target heading fits i16")
    );
    assert_eq!(after.dword_0x10, 0);
}

#[test]
fn level_two_authored_bee_constructor_and_action18_target_band() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 authored Bee session");
    let baseline_hash = session.campaign_state_hash();
    let source = session
        .allocate_level2_authored_bee(210)
        .expect("authored Day Bee row 210");
    assert_eq!(
        (
            source.source_class,
            source.model,
            source.action,
            source.minimum_speed,
            source.maximum_speed,
            source.actual_speed,
            source.max_life,
            source.life,
        ),
        (5, 2, 23, 64, 30, 32, 3000, 3000)
    );
    assert_eq!(
        (
            source.subspell_index,
            source.particle_index,
            source.shift_rot_x,
            source.shift_rot_y,
            source.source_byte_3e,
            source.source_byte_39,
            source.vertical_step,
            source.action_window,
        ),
        (200, 3, 128, 128, 0, 34, -32, 30)
    );
    assert_eq!(source.pitch, source.roll);
    assert_eq!(source.yaw, source.roll);
    assert_eq!(source.mana, 1500);
    assert_eq!(
        (source.stage_var1, source.stage_var2, source.stage_target),
        (
            6,
            2,
            Some(Position {
                x: 0x6500,
                y: 0x1000,
                z: 0
            })
        )
    );
    assert!((-1..=2046).contains(&source.roll));
    assert_eq!(
        source.dword_0x10,
        i32::from(source.source_entity_index % 100)
    );
    let second = session
        .allocate_level2_authored_bee(211)
        .expect("second authored Day Bee row");
    assert_eq!(second.source_byte_3e, 1);
    assert_eq!(second.source_byte_39, 33);

    let target = session.state.position;
    {
        let mut economy_view = session.economy_mut();
        let bee = economy_view
            .authored_bees
            .iter_mut()
            .find(|bee| bee.source_entity_index == second.source_entity_index)
            .expect("second Bee remains live");
        bee.action = 16;
        bee.position.x = target.x;
        bee.position.y = target.y;
    }
    assert!(
        session
            .economy_mut()
            .step_authored_bee_slot(second.source_entity_index, Some(target))
    );
    assert_eq!(
        session
            .economy()
            .authored_bees()
            .iter()
            .find(|bee| bee.source_entity_index == second.source_entity_index)
            .expect("action-16 Bee remains live")
            .action,
        18
    );
    let third = session
        .allocate_level2_authored_bee(212)
        .expect("third authored Day Bee row");
    {
        let mut economy_view = session.economy_mut();
        let bee = economy_view
            .authored_bees
            .iter_mut()
            .find(|bee| bee.source_entity_index == third.source_entity_index)
            .expect("third Bee remains live");
        bee.action = 19;
        bee.position.x = target.x;
        bee.position.y = target.y;
    }
    assert!(
        session
            .economy_mut()
            .step_authored_bee_slot(third.source_entity_index, Some(target))
    );
    assert_eq!(
        session
            .economy()
            .authored_bees()
            .iter()
            .find(|bee| bee.source_entity_index == third.source_entity_index)
            .expect("action-19 Bee remains live")
            .action,
        19
    );
    for _ in 0..27 {
        assert!(
            session
                .economy_mut()
                .step_authored_bee_slot(third.source_entity_index, Some(target))
        );
    }
    assert_eq!(
        session
            .economy()
            .authored_bees()
            .iter()
            .find(|bee| bee.source_entity_index == third.source_entity_index)
            .expect("windowed action-19 Bee remains live")
            .action,
        19
    );
    assert!(
        session
            .economy_mut()
            .step_authored_bee_slot(third.source_entity_index, Some(target))
    );
    assert_eq!(
        session
            .economy()
            .authored_bees()
            .iter()
            .find(|bee| bee.source_entity_index == third.source_entity_index)
            .expect("windowed action-19 Bee remains live")
            .action,
        18
    );
    {
        let mut economy_view = session.economy_mut();
        let bee = economy_view
            .authored_bees
            .iter_mut()
            .find(|bee| bee.source_entity_index == source.source_entity_index)
            .expect("first Bee remains live");
        bee.action = 17;
        bee.position.x = target.x;
        bee.position.y = target.y;
    }
    assert!(
        session
            .economy_mut()
            .step_authored_bee_slot(source.source_entity_index, Some(target))
    );
    let transitioned = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == source.source_entity_index)
        .copied()
        .expect("transitioned Bee remains live");
    assert_eq!(transitioned.action, 18);
    assert_eq!(transitioned.dword_0x10, 1);
    assert!(matches!(transitioned.last_sound_id, 12 | 13));
    {
        let mut economy_view = session.economy_mut();
        let bee = economy_view
            .authored_bees
            .iter_mut()
            .find(|bee| bee.source_entity_index == source.source_entity_index)
            .expect("Bee remains live");
        bee.action = 18;
        bee.position = Position {
            x: target.x,
            y: target.y,
            z: target.z.wrapping_add(40),
        };
        bee.dword_0x10 = 1;
        bee.source_byte_3e = 0;
        bee.control_flags |= LEVEL2_BEE_MOVEMENT_SKIP_BIT;
    }
    assert!(
        session
            .economy_mut()
            .step_authored_bee_slot(source.source_entity_index, Some(target))
    );
    let stepped = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == source.source_entity_index)
        .copied()
        .expect("stepped Bee remains live");
    assert_eq!(stepped.action, 18);
    assert_eq!(stepped.actual_speed, -30);
    assert_eq!(stepped.dword_0x10, 90);
    assert_eq!(stepped.position.z, target.z.wrapping_add(8));
    assert!(matches!(stepped.last_sound_id, 12 | 13));
    assert_ne!(session.campaign_state_hash(), baseline_hash);
    session.reset();
    assert!(session.economy().authored_bees().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_authored_bee_actions20_to23_preserve_death_and_no_call_semantics() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 authored Bee session");
    let baseline_hash = session.campaign_state_hash();

    let dying = session
        .allocate_level2_authored_bee(229)
        .expect("objective-linked group-21 Bee");
    session
        .economy_mut()
        .authored_bees
        .iter_mut()
        .find(|bee| bee.source_entity_index == dying.source_entity_index)
        .expect("dying Bee remains in source arena")
        .action = 20;
    // The action-21 kill tail admits the next byte-0x3E phase-zero callback.
    // Seed the direct callback test immediately before that boundary so the
    // two calls cover pre-kill followed by the admitted death callback.
    session
        .economy_mut()
        .authored_bees
        .iter_mut()
        .find(|bee| bee.source_entity_index == dying.source_entity_index)
        .expect("dying Bee remains in source arena")
        .source_byte_3e = 7;
    assert!(
        session
            .economy_mut()
            .step_authored_bee_slot(dying.source_entity_index, None)
    );
    assert_eq!(
        session
            .economy()
            .authored_bees()
            .iter()
            .find(|bee| bee.source_entity_index == dying.source_entity_index)
            .expect("pre-kill Bee remains until kill callback")
            .action,
        21
    );

    assert!(
        session
            .economy_mut()
            .step_authored_bee_slot(dying.source_entity_index, None)
    );
    let dead = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == dying.source_entity_index)
        .copied()
        .expect("kill callback leaves a reclaimable source row");
    assert!(!dead.active);
    assert!(!dead.drawing_enabled);
    assert_eq!(dead.life, -1);
    assert!(session.economy().events().iter().any(|event| matches!(
        event,
        Level2EconomyEvent::ManaSphereSpawned {
            source_entity_index,
            mana: 1500,
            ..
        } if *source_entity_index == dying.source_entity_index
    )));
    assert!(session.economy().events().iter().any(|event| matches!(
        event,
        Level2EconomyEvent::DeathBurstSpawned {
            parent_entity_index,
            id_entity_index,
            ..
        } if *parent_entity_index == dying.source_entity_index
            && *id_entity_index == dying.entity_id
    )));
    session.economy_mut().reclaim_inactive_slots();
    session
        .economy_mut()
        .reclaim_deferred_authored_bees_at_source_boundary();
    assert!(
        session
            .economy()
            .authored_bees()
            .iter()
            .all(|bee| bee.source_entity_index != dying.source_entity_index)
    );

    let no_call = session
        .allocate_level2_authored_bee(230)
        .expect("second objective-linked Bee");
    {
        let mut economy_view = session.economy_mut();
        let bee = economy_view
            .authored_bees
            .iter_mut()
            .find(|bee| bee.source_entity_index == no_call.source_entity_index)
            .expect("no-call Bee remains in source arena");
        bee.action = 22;
    }
    let before_no_call = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == no_call.source_entity_index)
        .copied()
        .expect("no-call Bee state");
    assert!(
        session
            .economy_mut()
            .step_authored_bee_slot(no_call.source_entity_index, None)
    );
    let after_no_call = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == no_call.source_entity_index)
        .copied()
        .expect("no-call Bee remains live");
    assert_eq!(after_no_call.action, before_no_call.action);
    assert_eq!(
        after_no_call.source_byte_3e,
        before_no_call.source_byte_3e.wrapping_add(1)
    );
    assert_eq!(after_no_call.phase, before_no_call.phase.wrapping_add(1));
    assert_eq!(after_no_call.entity_rng, before_no_call.entity_rng);
    assert_eq!(after_no_call.roll, before_no_call.roll);

    let scripted = session
        .allocate_level2_authored_bee(231)
        .expect("third objective-linked Bee");
    let (expected_rng, expected_roll) = {
        let mut economy_view = session.economy_mut();
        let bee = economy_view
            .authored_bees
            .iter_mut()
            .find(|bee| bee.source_entity_index == scripted.source_entity_index)
            .expect("scripted Bee remains in source arena");
        bee.action = 23;
        bee.stage_var2 = 1;
        bee.source_byte_3e = 0;
        let first = level2_next_rng16(bee.entity_rng);
        let direction = 2 * (u32::from(first % 0x9d) / 79);
        let second = level2_next_rng16(first);
        let delta = i32::from(second % 0x55)
            * (i32::try_from(direction).expect("Bee roll direction fits") - 1);
        (second, ((i32::from(bee.roll) + delta) as i16) & 0x07ff)
    };
    assert!(
        session
            .economy_mut()
            .step_authored_bee_slot(scripted.source_entity_index, None)
    );
    let stepped_scripted = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == scripted.source_entity_index)
        .copied()
        .expect("scripted Bee remains live");
    assert_eq!(stepped_scripted.entity_rng, expected_rng);
    assert_eq!(stepped_scripted.roll, expected_roll);

    session.reset();
    assert!(session.economy().authored_bees().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_authored_bee_action23_invalid_selector_is_production_noop() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
        .expect("qualified Level 2 owner package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 scripted Bee session");
    let baseline_hash = session.campaign_state_hash();

    // The real group-21 producer materializes action 23. The following
    // mutations select only an invalid StageVar2 selector; dispatch still
    // enters through the live dynamic cursor.
    session.step_outer_turn(PlayerCommand::default(), 0);
    let source_slot = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.authored_index == 231)
        .map(|bee| bee.source_entity_index)
        .expect("production group-21 row 231 Bee");

    for invalid_stage_var in [0_u8, 11_u8] {
        let before = session
            .economy()
            .authored_bees()
            .iter()
            .find(|bee| bee.source_entity_index == source_slot)
            .copied()
            .expect("live scripted Bee before invalid-selector step");
        let mut economy_view = session.economy_mut();
        let bee = economy_view
            .authored_bees
            .iter_mut()
            .find(|bee| bee.source_entity_index == source_slot)
            .expect("live scripted Bee selector");
        bee.stage_var2 = invalid_stage_var;
        bee.source_byte_3e = 0;

        session.step_outer_turn(PlayerCommand::default(), 0);
        let after = session
            .economy()
            .authored_bees()
            .iter()
            .find(|bee| bee.source_entity_index == source_slot)
            .copied()
            .expect("live scripted Bee after invalid-selector step");
        assert_eq!(after.action, 23);
        assert_eq!(after.stage_var2, invalid_stage_var);
        assert_eq!(after.entity_rng, before.entity_rng);
        assert_eq!(after.roll, before.roll);
        assert_eq!(after.position, before.position);
        assert_eq!(after.dword_0x10, before.dword_0x10);
        assert_eq!(after.last_sound_id, before.last_sound_id);
        assert_eq!(after.actual_speed, before.actual_speed);
        assert_eq!(after.source_byte_3e, 1);
        assert_eq!(after.phase, before.phase.wrapping_add(1));
    }

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_authored_bee_action18_target_loss_uses_source_validity_and_cadence() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Bee target-loss session");
    let baseline_hash = session.campaign_state_hash();
    let local_index = usize::from(session.state.entity_index);

    // Dead target: action 18 falls back immediately, without a callback
    // RNG draw, while retaining the source relation for action 17.
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row229_slot = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.authored_index == 229)
        .map(|bee| bee.source_entity_index)
        .expect("production group-21 Bee");
    let mut source = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("production Bee source");
    let initial_rng = source.entity_rng;
    let initial_xy = (source.position.x, source.position.y);
    source.action = 18;
    source.target_entity_index = Some(session.state.entity_index);
    source.source_byte_3e = 1;
    source.control_flags &= !LEVEL2_BEE_MOVEMENT_SKIP_BIT;
    assert!(session.economy.replace_authored_bee_for_test(source));
    session.economy.entities[local_index].life = -1;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let dead_target = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("dead-target Bee remains live");
    assert_eq!(dead_target.action, 17);
    assert_eq!(dead_target.actual_speed, dead_target.minimum_speed);
    assert_eq!(
        dead_target.target_entity_index,
        Some(session.state.entity_index)
    );
    assert_eq!(dead_target.entity_rng, initial_rng);
    assert_ne!((dead_target.position.x, dead_target.position.y), initial_xy);

    // Draw-disabled target is the same source-invalid branch.
    session.reset();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row229_slot = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.authored_index == 229)
        .map(|bee| bee.source_entity_index)
        .expect("production group-21 Bee after reset");
    let mut source = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("production Bee source after reset");
    let initial_rng = source.entity_rng;
    let target_height_offset = session.economy.entities[local_index].array_yaw();
    let desired_z = target_height_offset.wrapping_add(session.state.position.z);
    let direction: i16 = if source.position.z == desired_z {
        0
    } else if source.position.z.wrapping_sub(desired_z) <= 0 {
        -1
    } else {
        1
    };
    let expected_z = source
        .position
        .z
        .wrapping_add(direction.wrapping_mul(source.vertical_step));
    source.action = 18;
    source.target_entity_index = Some(session.state.entity_index);
    source.source_byte_3e = 1;
    source.control_flags |= LEVEL2_BEE_MOVEMENT_SKIP_BIT;
    assert!(session.economy.replace_authored_bee_for_test(source));
    session.economy.entities[local_index].control_flags |= ENTITY_DRAW_DISABLED_BIT;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let hidden_target = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("hidden-target Bee remains live");
    assert_eq!(hidden_target.action, 17);
    assert_eq!(hidden_target.actual_speed, hidden_target.minimum_speed);
    assert_eq!(
        hidden_target.target_entity_index,
        Some(session.state.entity_index)
    );
    assert_eq!(hidden_target.entity_rng, initial_rng);
    assert_eq!(hidden_target.position.z, expected_z);

    // A live target beyond the 0x1400 3-D range is retained off cadence,
    // then lost on the exact source cadence.
    session.reset();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row229_slot = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.authored_index == 229)
        .map(|bee| bee.source_entity_index)
        .expect("production group-21 Bee for range case");
    let mut source = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("production Bee range source");
    let far_position = Position {
        x: source.position.x.wrapping_add(1_024),
        y: source.position.y,
        z: source.position.z.wrapping_add(0x1401),
    };
    session.state.position = far_position;
    source.action = 18;
    source.target_entity_index = Some(session.state.entity_index);
    source.source_byte_3e = 29;
    source.control_flags |= LEVEL2_BEE_MOVEMENT_SKIP_BIT;
    assert!(session.economy.replace_authored_bee_for_test(source));
    session.step_outer_turn(PlayerCommand::default(), 0);
    let off_cadence = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("off-cadence range Bee remains live");
    assert_eq!(off_cadence.action, 18);
    assert_eq!(
        off_cadence.target_entity_index,
        Some(session.state.entity_index)
    );

    let mut cadence = off_cadence;
    cadence.source_byte_3e = 30;
    assert!(session.economy.replace_authored_bee_for_test(cadence));
    session.step_outer_turn(PlayerCommand::default(), 0);
    let lost_on_cadence = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("cadence-loss Bee remains live");
    assert_eq!(lost_on_cadence.action, 17);
    assert_eq!(lost_on_cadence.actual_speed, lost_on_cadence.minimum_speed);
    assert_eq!(
        lost_on_cadence.target_entity_index,
        Some(session.state.entity_index)
    );

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_authored_bee_action18_contact_is_strictly_below_1024_and_cadence_gated() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Bee contact session");

    session.step_outer_turn(PlayerCommand::default(), 0);
    let row229_slot = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.authored_index == LEVEL2_BEE_OBJECTIVE_AUTHORED_INDEX)
        .map(|bee| bee.source_entity_index)
        .expect("production row-229 Bee");
    assert!(row229_slot > session.state.entity_index);
    let local_target = session.state.position;
    let configure = |session: &mut Level2ResetSession, delta: u16, byte_3e: u8| {
        let bee = session
            .economy()
            .authored_bees()
            .iter()
            .find(|bee| bee.source_entity_index == row229_slot)
            .copied()
            .expect("production row-229 Bee source");
        let mut configured = bee;
        configured.action = 18;
        configured.target_entity_index = Some(session.state.entity_index);
        configured.source_byte_3e = byte_3e;
        configured.control_flags |= LEVEL2_BEE_MOVEMENT_SKIP_BIT;
        configured.position = Position {
            x: local_target.x.wrapping_add(delta),
            y: local_target.y,
            z: local_target.z,
        };
        assert!(session.economy.replace_authored_bee_for_test(configured));
        configured
    };

    let contact = configure(&mut session, 1023, 0);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.player_pending_damage, LEVEL2_BEE_CONTACT_SUBSPELL);
    assert_eq!(session.player_pending_attacker, contact.entity_id);
    let after_contact = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("contact Bee remains live");
    assert_ne!(after_contact.entity_rng, contact.entity_rng);
    assert_eq!(after_contact.actual_speed, -after_contact.maximum_speed);
    assert_eq!(
        after_contact.dword_0x10,
        3 * i32::from(after_contact.action_window)
    );
    // `sub_11900` overwrites an already-owned contact mailbox rather
    // than accumulating a second owner payload.
    session.queue_level2_player_contact_damage(77, 222);
    assert_eq!(session.player_pending_damage, 77);
    assert_eq!(session.player_pending_attacker, 222);
    let player_record = session.economy.entities[usize::from(session.state.entity_index)].record_bytes();
    assert_eq!(
        i32::from_le_bytes(
            player_record[98..102]
                .try_into()
                .expect("player payload bytes")
        ),
        77
    );
    assert_eq!(
        u16::from_le_bytes(
            player_record[102..104]
                .try_into()
                .expect("player owner bytes")
        ),
        222
    );

    session.reset();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let edge = configure(&mut session, 1024, 0);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.player_pending_damage, 0);
    let after_edge = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("edge Bee remains live");
    assert_eq!(after_edge.entity_rng, edge.entity_rng);
    assert_eq!(after_edge.action, 18);

    session.reset();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let off_cadence = configure(&mut session, 1023, 1);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.player_pending_damage, 0);
    let after_off_cadence = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("off-cadence Bee remains live");
    assert_eq!(after_off_cadence.entity_rng, off_cadence.entity_rng);
    assert_eq!(after_off_cadence.action, 18);
}

#[test]
fn level_two_authored_bee_action18_contact_is_consumed_at_next_player_source_visit() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Bee contact ordering session");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row229_slot = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.authored_index == LEVEL2_BEE_OBJECTIVE_AUTHORED_INDEX)
        .map(|bee| bee.source_entity_index)
        .expect("production row-229 Bee");
    assert!(row229_slot > session.state.entity_index);
    let target = session.state.position;
    let bee = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("production row-229 Bee source");
    let mut configured = bee;
    configured.action = 18;
    configured.target_entity_index = Some(session.state.entity_index);
    configured.source_byte_3e = 0;
    configured.control_flags |= LEVEL2_BEE_MOVEMENT_SKIP_BIT;
    configured.position = Position {
        x: target.x.wrapping_add(1023),
        y: target.y,
        z: target.z,
    };
    assert!(session.economy.replace_authored_bee_for_test(configured));
    let life_before = session.player_life;

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.player_pending_damage, LEVEL2_BEE_CONTACT_SUBSPELL);
    assert_eq!(session.player_pending_attacker, bee.entity_id);
    assert_eq!(session.player_life, life_before);
    let first_hud = session.day_hud().expect("Day HUD after queued contact");
    assert_eq!(first_hud.color_phase_1, 1);
    assert_eq!(first_hud.player_panel_selector, 41);
    assert_eq!(first_hud.hit_frame_time_pre_draw, 0);

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.player_pending_damage, LEVEL2_BEE_CONTACT_SUBSPELL);
    assert_eq!(session.player_pending_attacker, 0);
    assert_eq!(
        session.player_life,
        life_before - LEVEL2_BEE_CONTACT_SUBSPELL
    );
    let player_record = session.economy.entities[usize::from(session.state.entity_index)].record_bytes();
    assert_eq!(
        i32::from_le_bytes(
            player_record[98..102]
                .try_into()
                .expect("player payload bytes")
        ),
        LEVEL2_BEE_CONTACT_SUBSPELL
    );
    assert_eq!(
        u16::from_le_bytes(
            player_record[102..104]
                .try_into()
                .expect("player owner bytes")
        ),
        0
    );
    let damage_events = session
        .dispatch_events()
        .iter()
        .filter_map(|event| match event {
            Level2DispatchEvent::Economy {
                event:
                    Level2EconomyEvent::DamageApplied {
                        target_entity_index,
                        attacker_entity_index,
                        damage,
                        lethal,
                    },
                ..
            } => Some((
                *target_entity_index,
                *attacker_entity_index,
                *damage,
                *lethal,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        damage_events.last().copied(),
        Some((
            session.state.entity_index,
            bee.entity_id,
            LEVEL2_BEE_CONTACT_SUBSPELL,
            false
        ))
    );
    let second_hud = session.day_hud().expect("Day HUD after damage consume");
    assert_eq!(second_hud.color_phase_1, 0);
    assert_eq!(second_hud.player_panel_selector, 41);
    assert_eq!(second_hud.hit_frame_time_pre_draw, 4);
    let second_player_record = session.shared_spells().local_player.record_bytes();
    assert_eq!(second_player_record[406], 4);

    session.step_outer_turn(PlayerCommand::default(), 0);
    let third_hud = session.day_hud().expect("Day HUD during hit flash");
    assert_eq!(third_hud.color_phase_1, 1);
    assert_eq!(third_hud.player_panel_selector, 55);
    assert_eq!(third_hud.hit_frame_time_pre_draw, 4);
    let third_player_record = session.shared_spells().local_player.record_bytes();
    assert_eq!(third_player_record[406], 3);
    assert_eq!(
        session.day_hud().expect("idempotent Day HUD read"),
        third_hud
    );
}

#[test]
fn level_two_player_contact_lethal_visit_retains_owner_and_records_killer() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 lethal contact session");
    let local_index = usize::from(session.state.entity_index);
    let attacker = 222_u16;
    session.player_life = 100;
    session.economy.entities[local_index].life = 100;
    session.economy.entities[local_index].record[12..16].copy_from_slice(&100_i32.to_le_bytes());
    session.shared_spells.set_player_entity_life(100);
    session.queue_level2_player_contact_damage(200, attacker);
    session.step_outer_turn(PlayerCommand::default(), 0);

    let player = &session.economy.entities[local_index];
    assert_eq!(session.player_life, -100);
    assert_eq!(player.action, 2);
    assert_eq!(player.word_24(), attacker);
    assert_eq!(player.word_26(), attacker);
    assert_eq!(
        u16::from_le_bytes(player.record_bytes()[102..104].try_into().unwrap()),
        attacker
    );
    assert_eq!(
        u16::from_le_bytes(player.record_bytes()[48..50].try_into().unwrap()),
        0
    );
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::Economy {
            event: Level2EconomyEvent::DamageApplied {
                attacker_entity_index,
                lethal: true,
                ..
            },
            ..
        } if *attacker_entity_index == attacker
    )));

    // Later source visits clear word-0x26 but do not reapply damage or
    // clear the lethal mailbox owner, matching the life<0 branch.
    session.step_outer_turn(PlayerCommand::default(), 0);
    let player = &session.economy.entities[local_index];
    assert_eq!(player.word_26(), 0);
    assert_eq!(
        u16::from_le_bytes(player.record_bytes()[102..104].try_into().unwrap()),
        attacker
    );
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::Economy {
            event: Level2EconomyEvent::DamageApplied { .. },
            ..
        }
    )));
}

#[test]
fn level_two_authored_bee_action17_overwrites_stale_target_on_production_reacquisition() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Bee reacquisition session");
    let baseline_hash = session.campaign_state_hash();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row229_slot = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.authored_index == 229)
        .map(|bee| bee.source_entity_index)
        .expect("production group-21 Bee");
    let local_index = usize::from(session.state.entity_index);
    let candidate_index = session.economy.entities
        .iter()
        .position(|entity| entity.index != 0 && !entity.active)
        .expect("inactive class-3 candidate row");
    let candidate_entity_index = session.economy.entities[candidate_index].index;
    let local_entity_index = session.state.entity_index;
    for entity in &mut session.economy.entities {
        if entity.class == 3
            && entity.index != local_entity_index
            && entity.index != candidate_entity_index
        {
            entity.active = false;
            entity.next_relation = None;
            entity.record[4..8].fill(0);
        }
    }
    let source = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("production Bee source");
    let far_position = Position {
        x: source.position.x.wrapping_add(1_024),
        y: source.position.y,
        z: source.position.z.wrapping_add(0x1401),
    };
    session.state.position = far_position;
    let local_entity = &mut session.economy.entities[local_index];
    local_entity.active = true;
    local_entity.class = 3;
    local_entity.model = 0;
    local_entity.life = 100;
    local_entity.control_flags = 0;
    local_entity.next_relation = Some(candidate_entity_index);
    local_entity.record[4..8]
        .copy_from_slice(&(u32::from(candidate_entity_index) + 1).to_le_bytes());
    let candidate_position = Position {
        x: source.position.x.wrapping_add(512),
        y: source.position.y,
        z: source.position.z,
    };
    let candidate = &mut session.economy.entities[candidate_index];
    candidate.active = true;
    candidate.class = 3;
    candidate.model = 0;
    candidate.life = 100;
    candidate.control_flags = 0;
    candidate.position = candidate_position;
    candidate.next_relation = None;
    candidate.record[4..8].fill(0);

    let mut action18 = source;
    action18.action = 18;
    action18.target_entity_index = Some(session.state.entity_index);
    action18.source_byte_3e = 30;
    assert!(session.economy.replace_authored_bee_for_test(action18));
    session.step_outer_turn(PlayerCommand::default(), 0);
    let lost = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("range-lost Bee remains live");
    assert_eq!(lost.action, 17);
    assert_eq!(lost.target_entity_index, Some(session.state.entity_index));

    // Hide the stale local relation from the next scan while preserving
    // it in word_0x96; source action 17 must scan and overwrite it.
    session.economy.entities[local_index].control_flags = ENTITY_DRAW_DISABLED_BIT;
    let mut reacquire = lost;
    reacquire.source_byte_3e = 30;
    reacquire.source_byte_39 = 1;
    reacquire.yaw = i16::try_from(heading_for_positions(
        reacquire.position,
        candidate_position,
    ))
    .expect("Bee candidate heading fits");
    let first = level2_next_rng16(reacquire.entity_rng);
    let second = level2_next_rng16(first);
    let third = level2_next_rng16(second);
    assert!(session.economy.replace_authored_bee_for_test(reacquire));
    session.step_outer_turn(PlayerCommand::default(), 0);
    let reacquired = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("reacquired Bee remains live");
    assert_eq!(reacquired.action, 18);
    assert_eq!(reacquired.target_entity_index, Some(candidate_entity_index));
    assert_eq!(reacquired.entity_rng, third);
    assert_eq!(reacquired.last_sound_id, (third & 1) + 12);
    assert_eq!(reacquired.dword_0x10, 1);

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_authored_bee_action17_naturally_acquires_source_order_target() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-21 Bee session");
    let baseline_hash = session.campaign_state_hash();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row229_slot = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.authored_index == LEVEL2_BEE_OBJECTIVE_AUTHORED_INDEX)
        .map(|bee| bee.source_entity_index)
        .expect("production group-21 row 229 Bee");

    let local_index = usize::from(session.state.entity_index);
    let candidate_indices = session.economy.entities
        .iter()
        .enumerate()
        .filter(|(_, entity)| entity.index != 0 && !entity.active)
        .map(|(index, _)| index)
        .take(5)
        .collect::<Vec<_>>();
    assert_eq!(candidate_indices.len(), 5);
    // Keep pool indices ascending but make the two equal-distance
    // eligible candidates appear in the opposite linked-list order.
    let linked_order = [
        candidate_indices[0],
        candidate_indices[2],
        candidate_indices[1],
        candidate_indices[3],
        candidate_indices[4],
    ];
    let source_position = Position {
        x: 30_000,
        y: 30_000,
        z: 0,
    };
    let target_positions = [
        Position {
            x: 30_000,
            y: 29_500,
            z: 0,
        },
        Position {
            x: 30_000,
            y: 29_000,
            z: 0,
        },
        Position {
            x: 30_000,
            y: 29_000,
            z: 0,
        },
        Position {
            x: 31_000,
            y: 31_000,
            z: 0,
        },
        Position {
            x: 30_000,
            y: 24_000,
            z: 0,
        },
    ];
    let initial_rng = 0x1234;
    let configure = |session: &mut Level2ResetSession, all_blocked: bool| {
        let player_entity_index = session.state.entity_index;
        for entity in &mut session.economy.entities {
            if entity.class == 3 && entity.index != player_entity_index {
                entity.active = false;
                entity.next_relation = None;
                entity.record[4..8].fill(0);
            }
        }
        session.state.position = target_positions[0];
        let first_candidate_id = session.economy.entities[candidate_indices[0]].index;
        let local = &mut session.economy.entities[local_index];
        local.active = true;
        local.class = 3;
        local.life = 100;
        local.control_flags = 0x20;
        local.next_relation = Some(first_candidate_id);
        local.record[4..8].copy_from_slice(&(u32::from(first_candidate_id) + 1).to_le_bytes());
        for (offset, candidate_index) in candidate_indices.iter().copied().enumerate() {
            let candidate = &mut session.economy.entities[candidate_index];
            candidate.active = true;
            candidate.class = 3;
            candidate.model = 0;
            candidate.life = 100;
            candidate.control_flags = if all_blocked || offset == 0 { 0x20 } else { 0 };
            candidate.position = target_positions[offset];
            candidate.next_relation = None;
            candidate.record[4..8].fill(0);
        }
        for (offset, candidate_index) in linked_order.iter().copied().enumerate() {
            let next = linked_order
                .get(offset + 1)
                .map(|next_index| session.economy.entities[*next_index].index);
            let candidate = &mut session.economy.entities[candidate_index];
            candidate.next_relation = next;
            if let Some(next) = next {
                candidate.record[4..8].copy_from_slice(&(u32::from(next) + 1).to_le_bytes());
            } else {
                candidate.record[4..8].fill(0);
            }
        }
        let heading = heading_for_positions(source_position, target_positions[1]);
        let source = session
            .economy
            .authored_bees
            .iter_mut()
            .find(|bee| bee.source_entity_index == row229_slot)
            .expect("production row 229 source");
        source.position = source_position;
        source.yaw = i16::try_from(heading).expect("Bee heading fits");
        source.action = 17;
        source.target_entity_index = None;
        source.source_byte_3e = 0;
        source.source_byte_39 = 1;
        source.entity_rng = initial_rng;
        source.roll = 0;
    };

    configure(&mut session, false);
    let first = level2_next_rng16(initial_rng);
    let second = level2_next_rng16(first);
    let third = level2_next_rng16(second);
    let expected_roll = (i32::from((second & 0xff) as u8) + 85)
        * (i32::try_from(2 * (u32::from(first % 0x9d) / 79)).expect("Bee roll direction fits") - 1);
    session.step_outer_turn(PlayerCommand::default(), 0);
    let acquired = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("naturally targeted Bee remains live");
    assert_eq!(acquired.action, 18);
    assert_eq!(
        acquired.target_entity_index,
        Some(session.economy.entities[candidate_indices[2]].index)
    );
    assert_eq!(acquired.entity_rng, third);
    assert_eq!(acquired.last_sound_id, (third & 1) + 12);
    assert_eq!(acquired.roll, expected_roll as i16 & 0x07ff);
    assert_eq!(acquired.source_byte_3e, 1);
    assert_eq!(acquired.dword_0x10, 1);

    session.reset();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row229_slot_after_reset = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.authored_index == LEVEL2_BEE_OBJECTIVE_AUTHORED_INDEX)
        .map(|bee| bee.source_entity_index)
        .expect("production group-21 Bee after reset");
    assert_eq!(row229_slot_after_reset, row229_slot);
    configure(&mut session, true);
    session.step_outer_turn(PlayerCommand::default(), 0);
    let no_target = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("no-target Bee remains live");
    assert_eq!(no_target.action, 17);
    assert_eq!(no_target.target_entity_index, None);
    assert_eq!(no_target.entity_rng, second);
    assert_eq!(no_target.roll, expected_roll as i16 & 0x07ff);
    assert_eq!(no_target.source_byte_3e, 1);

    // With eligible candidates present, a zero byte-0x39 suppresses the
    // class-3 scan but does not suppress the two preceding wander draws.
    for candidate_index in [candidate_indices[1], candidate_indices[2]] {
        session.economy.entities[candidate_index].control_flags = 0;
    }
    let source = session
        .economy
        .authored_bees
        .iter_mut()
        .find(|bee| bee.source_entity_index == row229_slot)
        .expect("no-target Bee source");
    source.source_byte_3e = 0;
    source.source_byte_39 = 0;
    source.entity_rng = initial_rng;
    source.roll = 0;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let gated = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("byte-0x39-gated Bee remains live");
    assert_eq!(gated.action, 17);
    assert_eq!(gated.target_entity_index, None);
    assert_eq!(gated.entity_rng, second);
    assert_eq!(gated.roll, expected_roll as i16 & 0x07ff);
    assert_eq!(gated.source_byte_3e, 1);
    let no_target_rng = no_target.entity_rng;
    let no_target_roll = no_target.roll;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let off_cadence = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("off-cadence Bee remains live");
    assert_eq!(off_cadence.action, 17);
    assert_eq!(off_cadence.target_entity_index, None);
    assert_eq!(off_cadence.entity_rng, no_target_rng);
    assert_eq!(off_cadence.roll, no_target_roll);
    assert_eq!(off_cadence.source_byte_3e, 2);
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_bee_activation_filters_kind_subtype_and_preserves_source_order() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 authored Bee session");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let bees = session.economy().authored_bees();
    assert!(
        !bees.is_empty(),
        "rooted Bee producer groups should activate"
    );
    assert!(
        bees.windows(2)
            .all(|rows| { rows[0].source_entity_index < rows[1].source_entity_index })
    );
    assert!(bees.iter().all(|bee| {
        session.mission().authored_features().iter().any(|feature| {
            feature.index == bee.authored_index
                && feature.kind == 5
                && feature.subtype == 2
                && matches!(feature.activation_group, 4 | 17 | 21 | 29)
        })
    }));
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(event, Level2DispatchEvent::AuthoredGroupActivated { group, .. }
                if matches!(group, 4 | 17 | 21 | 29))
    }));
}

#[test]
fn level_two_authored_model6_runs_source_callback_and_spawn_branch() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 authored model-6 session");
    let baseline_hash = session.campaign_state_hash();

    // The qualified census has four authored class-10/model-6 rows. The
    // constructor payload comes from NewAdd0A06_4E5F0, not from a test
    // fixture or a generic alias.
    let first = session
        .allocate_level2_authored_model6(361)
        .expect("authored model-6 row 361");
    let second = session
        .allocate_level2_authored_model6(362)
        .expect("authored model-6 row 362");
    assert_ne!(first.source_entity_index, second.source_entity_index);
    assert_eq!(first.entity_id, first.source_entity_index);
    assert_eq!(
        (
            first.model,
            first.action,
            first.subspell_index,
            first.max_life,
            first.life,
            first.height_offset,
            first.dword_0x10,
            first.particle_index,
            first.control_flags,
            first.shift_rot_x,
            first.shift_rot_y,
        ),
        (6, 6, 50, 240, 240, 0, 0, 228, 0x0002_0000, 272, 1536)
    );
    let before_turn = first;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let stepped = session
        .economy()
        .authored_model6()
        .iter()
        .find(|source| source.source_entity_index == first.source_entity_index)
        .copied()
        .expect("model-6 source survives first callback");
    assert_eq!(stepped.life, before_turn.life - 1);
    assert_eq!(stepped.dword_0x10, 1);
    assert_eq!(stepped.particle_index, before_turn.particle_index + 1);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::DynamicArena {
            kind: Level2DynamicArenaKind::AuthoredModel6,
            source_slot,
            ..
        } if *source_slot == first.source_entity_index
    )));

    // Drive the late-life branch until the pinned RNG `% 7` child branch
    // fires. Resetting only the callback words keeps this a discriminating
    // source-RNG test while avoiding a probabilistic assertion.
    let mut child = None;
    for _ in 0..32 {
        if let Some(found) = session.economy().model6_spawned_children().first().copied() {
            child = Some(found);
            break;
        }
        let mut economy_view = session.economy_mut();
        let source = economy_view
            .authored_model6
            .iter_mut()
            .find(|source| source.source_entity_index == first.source_entity_index)
            .expect("model-6 source remains live");
        source.life = 11;
        source.dword_0x10 = 1;
        assert!(
            session
                .economy_mut()
                .step_authored_model6_slot(first.source_entity_index)
        );
    }
    let child = child.expect("model-6 source eventually emits subtype-14 child");
    assert_eq!(
        (
            child.parent_entity_index,
            child.parent_entity_id,
            child.source_class,
            child.model,
            child.action,
            child.max_life,
            child.life,
            child.actual_speed,
            child.min_speed,
            child.dword_0x10,
            child.particle_index,
            child.control_flags,
        ),
        (
            first.source_entity_index,
            first.entity_id,
            9,
            14,
            15,
            32,
            15,
            128,
            128,
            100,
            198,
            0,
        )
    );
    assert!(
        session
            .economy_mut()
            .step_model6_spawned_child_slot(child.source_entity_index)
    );
    let stepped_child = session
        .economy()
        .model6_spawned_children()
        .iter()
        .find(|candidate| candidate.source_entity_index == child.source_entity_index)
        .copied()
        .expect("spawned child remains live");
    assert_eq!(stepped_child.actual_speed, 124);
    assert!(stepped_child.position.z >= child.position.z);

    // `sub_104D0_terrain_tile_is_water` disables drawing after the
    // height projection. Force the live second source onto a source water
    // tile to exercise that real branch before reclaim.
    let water_tile = terrain_index(
        second.position.x.to_be_bytes()[0],
        second.position.y.to_be_bytes()[0],
    );
    session.canonical_terrain_mut().planes.terrain_types[water_tile] = 0;
    assert!(
        session
            .economy_mut()
            .step_authored_model6_slot(second.source_entity_index)
    );
    assert!(
        !session
            .economy()
            .authored_model6()
            .iter()
            .find(|source| source.source_entity_index == second.source_entity_index)
            .expect("water-tested model-6 source")
            .active
    );
    session.economy_mut().reclaim_inactive_slots();
    assert!(
        session
            .economy()
            .authored_model6()
            .iter()
            .all(|source| source.source_entity_index != second.source_entity_index)
    );

    // Exhaustion is a real allocator failure, not a mocked success path.
    let mut exhausted = session.economy().clone();
    exhausted.free_slots.clear();
    for source in &mut exhausted.authored_model6 {
        source.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    let before_exhaustion = exhausted.clone();
    let terrain = session.canonical_terrain().clone();
    assert!(
        exhausted
            .allocate_authored_model6(363, Position::default(), &terrain)
            .is_none()
    );
    assert_eq!(exhausted, before_exhaustion);

    session
        .economy_mut()
        .authored_model6
        .iter_mut()
        .find(|source| source.source_entity_index == first.source_entity_index)
        .expect("model-6 parent for expiry")
        .life = -1;
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .authored_model6()
            .iter()
            .all(|source| source.source_entity_index != first.source_entity_index)
    );

    session.reset();
    assert!(session.economy().authored_model6().is_empty());
    assert!(session.economy().model6_spawned_children().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_authored_model17_runs_meteor_constructor_and_spawn_branch() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 authored model-17 session");
    let baseline_hash = session.campaign_state_hash();

    // AUT-L2-0157 is a qualified class-10/subtype-17 row. Its parent
    // fields are the explicit AddMeteor_4ED70 constructor payload.
    let meteor = session
        .allocate_level2_authored_model17(157)
        .expect("authored model-17 row 157");
    assert_eq!(
        (
            meteor.entity_id,
            meteor.source_class,
            meteor.model,
            meteor.action,
            meteor.subspell_index,
            meteor.max_life,
            meteor.life,
            meteor.control_flags,
            meteor.shift_rot_x,
            meteor.shift_rot_y,
        ),
        (
            meteor.source_entity_index,
            10,
            17,
            17,
            3000,
            10,
            10,
            0,
            0,
            0
        )
    );

    // The production outer cursor visits the parent and materializes the
    // three source SEARCH.DAT shape-0 child cells.
    session.step_outer_turn(PlayerCommand::default(), 0);
    let parent = session
        .economy()
        .authored_model17()
        .iter()
        .find(|source| source.source_entity_index == meteor.source_entity_index)
        .copied()
        .expect("Meteor parent survives first callback");
    assert_eq!(
        (
            parent.life,
            parent.dword_0x10,
            parent.control_flags,
            parent.shift_rot_x,
            parent.shift_rot_y,
        ),
        (9, 2, 0x0001_0002, 0, 512)
    );
    assert_eq!(session.economy().model17_spawned_children().len(), 3);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::DynamicArena {
            kind: Level2DynamicArenaKind::AuthoredModel17,
            source_slot,
            ..
        } if *source_slot == meteor.source_entity_index
    )));

    let child = session.economy().model17_spawned_children()[0];
    assert_eq!(
        (
            child.parent_entity_index,
            child.parent_entity_id,
            child.source_class,
            child.model,
            child.action,
            child.subspell_index,
            child.max_life,
            child.life,
            child.control_flags,
            child.shift_rot_x,
            child.shift_rot_y,
            child.yaw,
        ),
        (
            meteor.source_entity_index,
            meteor.entity_id,
            10,
            0,
            0,
            400,
            8,
            7,
            0x0003_0080,
            512,
            512,
            meteor.yaw,
        )
    );
    assert!(
        session
            .economy_mut()
            .step_model17_spawned_child_slot(child.source_entity_index)
    );
    let stepped_child = session
        .economy()
        .model17_spawned_children()
        .iter()
        .find(|candidate| candidate.source_entity_index == child.source_entity_index)
        .copied()
        .expect("Meteor child remains live");
    assert_eq!(stepped_child.life, 6);
    assert_eq!(stepped_child.phase, child.phase.wrapping_add(1));

    // The source child callback decrements first and disables on the same
    // visit when the new life value is below -1.
    session
        .economy_mut()
        .model17_spawned_children
        .iter_mut()
        .find(|candidate| candidate.source_entity_index == child.source_entity_index)
        .expect("Meteor child for negative-life sentinel")
        .life = -1;
    assert!(
        session
            .economy_mut()
            .step_model17_spawned_child_slot(child.source_entity_index)
    );
    assert!(
        !session
            .economy()
            .model17_spawned_children()
            .iter()
            .find(|candidate| candidate.source_entity_index == child.source_entity_index)
            .expect("Meteor child after negative-life sentinel")
            .active
    );
    session.economy_mut().reclaim_inactive_slots();
    assert!(
        session
            .economy()
            .model17_spawned_children()
            .iter()
            .all(|candidate| candidate.source_entity_index != child.source_entity_index)
    );

    // Parent expiry follows the source negative-life sentinel and the
    // ordinary reclaim pass releases its map-linked arena slot.
    session
        .economy_mut()
        .authored_model17
        .iter_mut()
        .find(|source| source.source_entity_index == meteor.source_entity_index)
        .expect("Meteor parent for expiry")
        .life = -1;
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .authored_model17()
            .iter()
            .all(|source| source.source_entity_index != meteor.source_entity_index)
    );
    assert!(
        session
            .economy()
            .model17_spawned_children()
            .iter()
            .all(|child| child.active)
    );

    session.reset();
    assert!(session.economy().authored_model17().is_empty());
    assert!(session.economy().model17_spawned_children().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_authored_model22_runs_wind_constructor_chain_and_expiry() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 authored model-22 session");
    let baseline_hash = session.campaign_state_hash();

    // AUT-L2-0110 is a qualified class-10/subtype-22 row. The authored
    // path derives the wind axis from parameters before AddWind_4F040
    // creates its model-22 parent and eleven model-75 chain rows.
    let wind = session
        .allocate_level2_authored_model22(110)
        .expect("authored model-22 row 110");
    assert_eq!(
        (
            wind.entity_id,
            wind.source_class,
            wind.model,
            wind.action,
            wind.word_0x2c,
            wind.word_0x2e,
            wind.actual_speed,
            wind.minimum_speed,
        ),
        (wind.source_entity_index, 10, 22, 22, 0, 1, 50, 20,)
    );
    assert_eq!(
        (
            wind.maximum_speed,
            wind.max_life,
            wind.life,
            wind.subspell_index,
            wind.particle_index,
            wind.shift_rot_x,
            wind.shift_rot_y,
        ),
        (10, 500, 500, 1000, 293, 38, 31,)
    );
    assert_eq!(session.economy().model22_spawned_children().len(), 11);
    // `sub_4A310` constructs at the authored tile center, then rewrites
    // only the model-22 axis x/y from par2/par1.
    assert_eq!((wind.position.x, wind.position.y), (640, 49_792));
    assert_eq!((wind.axis_position.x, wind.axis_position.y), (128, 128));
    assert_eq!(wind.position.z, wind.axis_position.z);
    let first_child = session.economy().model22_spawned_children()[0];
    assert_eq!(
        (
            first_child.parent_entity_index,
            first_child.parent_entity_id,
            first_child.source_class,
            first_child.model,
            first_child.action,
            first_child.segment_index,
            first_child.word_0x2c,
            first_child.word_0x36,
            first_child.particle_index,
            first_child.shift_rot_x,
            first_child.shift_rot_y,
            first_child.phase,
        ),
        (
            wind.source_entity_index,
            wind.entity_id,
            10,
            75,
            82,
            1,
            1,
            62,
            294,
            44,
            36,
            0,
        )
    );
    assert_eq!((first_child.max_life, first_child.life), (500, 500));
    assert!(first_child.drawing_enabled);

    let before_position = wind.position;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let stepped = session
        .economy()
        .authored_model22()
        .iter()
        .find(|source| source.source_entity_index == wind.source_entity_index)
        .copied()
        .expect("Wind parent survives first callback");
    assert_eq!(stepped.life, 499);
    assert_eq!(stepped.phase, wind.phase.wrapping_add(1));
    assert_ne!(stepped.position, before_position);
    assert_eq!(
        session
            .economy()
            .model22_spawned_children()
            .iter()
            .find(|child| child.source_entity_index == first_child.source_entity_index)
            .expect("Wind child remains linked")
            .phase,
        first_child.phase.wrapping_add(1)
    );
    let stepped_child = session
        .economy()
        .model22_spawned_children()
        .iter()
        .find(|child| child.source_entity_index == first_child.source_entity_index)
        .copied()
        .expect("Wind child after action-82 writeback");
    assert_eq!(stepped_child.action, 82);
    assert_eq!((stepped_child.max_life, stepped_child.life), (500, 500));
    assert!(!stepped_child.drawing_enabled);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::DynamicArena {
            kind: Level2DynamicArenaKind::AuthoredModel22,
            source_slot,
            ..
        } if *source_slot == wind.source_entity_index
    )));
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::DynamicArena {
            kind: Level2DynamicArenaKind::Model22SpawnedChild,
            ..
        }
    )));

    // Exhaustion is a real allocator failure, not a mocked success path.
    let saved_free_slots = session.economy().free_slots.clone();
    session.economy_mut().free_slots.clear();
    let parent_count = session.economy().authored_model22().len();
    assert!(session.allocate_level2_authored_model22(114).is_none());
    assert_eq!(session.economy().authored_model22().len(), parent_count);
    session.economy_mut().free_slots = saved_free_slots;

    // The parent callback disables the entire linked chain; reclaim then
    // returns every map-linked source slot to the shared arena.
    session
        .economy_mut()
        .authored_model22
        .iter_mut()
        .find(|source| source.source_entity_index == wind.source_entity_index)
        .expect("Wind parent for expiry")
        .life = -1;
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .authored_model22()
            .iter()
            .all(|source| source.source_entity_index != wind.source_entity_index)
    );
    assert!(session.economy().model22_spawned_children().is_empty());

    session.reset();
    assert!(session.economy().authored_model22().is_empty());
    assert!(session.economy().model22_spawned_children().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_whirlwind_action82_player_handoff_is_exact() {
    let ledger: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../level2-conformance/denominator/level2-dispatch-rng-audio-presentation.json"
    ))
    .expect("pinned Level 2 dispatch ledger");
    let rows = ledger["dispatch"]["rows"]
        .as_array()
        .expect("dispatch rows");
    let class5_row = rows
        .iter()
        .find(|row| row["id"] == "DSP-L2-155")
        .expect("class-5 action-82 dispatch row");
    assert_eq!(class5_row["runtime_class"], 5);
    assert_eq!(class5_row["physical_slot"], 82);
    assert_eq!(class5_row["resolved_function"], "sub_22540");
    assert_eq!(class5_row["stored_address_hex"], "0x00203540");
    assert_eq!(
        class5_row["classification"],
        "mechanically_unreachable_level2"
    );
    let class10_row = rows
        .iter()
        .find(|row| row["id"] == "DSP-L2-492")
        .expect("class-10 action-82 dispatch row");
    assert_eq!(class10_row["runtime_class"], 10);
    assert_eq!(class10_row["physical_slot"], 82);
    assert_eq!(class10_row["resolved_function"], "");
    assert_eq!(class10_row["classification"], "disabled_slot");

    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Whirlwind session");
    let baseline_hash = session.campaign_state_hash();
    let wind = session
        .allocate_level2_authored_model22(110)
        .expect("authored Whirlwind row 110");
    assert_eq!(wind.authored_index, 110);
    let child = session.economy().model22_spawned_children()[0];
    assert_eq!((child.model, child.action), (75, 82));

    // Deliberately combine the packet's supposed rejection predicates:
    // occupied family, no free hand, and an ineligible player row. The
    // pinned model-range gate reads none of them and dispatches no
    // callback, so the complete spell projection must remain byte-for-byte
    // unchanged.
    {
        let mut shared = session.shared_spells_mut();
        shared.players[0].enabled_spell_holders[10] = 999;
        shared.players[0].left_spell_slot_raw = 4;
        shared.players[0].right_spell_slot_raw = 5;
        shared.player_entities[0].active = false;
    }
    let spells_before = session.shared_spells().clone();
    let economy_events_before = session.economy().events.clone();
    let global_rng_before = session.economy().global_rng;
    assert!(
        session
            .economy_mut()
            .step_model22_spawned_child_slot(child.source_entity_index)
    );
    let directly_stepped = session
        .economy()
        .model22_spawned_children()
        .iter()
        .find(|candidate| candidate.source_entity_index == child.source_entity_index)
        .copied()
        .expect("directly stepped action-82 child");
    assert_eq!(directly_stepped.action, 82);
    assert_eq!(directly_stepped.phase, child.phase.wrapping_add(1));
    assert_eq!(directly_stepped.life, child.life);
    assert_eq!(directly_stepped.position, child.position);
    assert_eq!(directly_stepped.entity_rng, child.entity_rng);
    assert!(!directly_stepped.drawing_enabled);
    assert_eq!(session.shared_spells(), &spells_before);
    assert_eq!(session.economy().events, economy_events_before);
    assert_eq!(session.economy().global_rng, global_rng_before);

    // The ordinary Level 2 source cursor reaches the same row, then reset
    // and replay reproduce the complete campaign hash.
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    let replay_once = session
        .allocate_level2_authored_model22(110)
        .expect("first production Whirlwind replay");
    let replay_child_slot = session.economy().model22_spawned_children()[0].source_entity_index;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let first_hash = session.campaign_state_hash();
    assert!(
        session
            .economy()
            .model22_spawned_children()
            .iter()
            .any(
                |candidate| candidate.source_entity_index == replay_child_slot
                    && candidate.action == 82
                    && candidate.life == 500
                    && !candidate.drawing_enabled
            )
    );
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::DynamicArena {
            kind: Level2DynamicArenaKind::Model22SpawnedChild,
            source_slot,
            ..
        } if *source_slot == replay_child_slot
    )));
    assert!(session.dispatch_events().iter().all(|event| !matches!(
        event,
        Level2DispatchEvent::SpellAcquisition {
            source_slot,
            result: Some(_),
            ..
        } if *source_slot == replay_child_slot
    )));

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    let replay_twice = session
        .allocate_level2_authored_model22(110)
        .expect("second production Whirlwind replay");
    assert_eq!(replay_twice, replay_once);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.campaign_state_hash(), first_hash);
}

#[test]
fn level_two_model59_checkpoint_smoke_constructor_rng_and_capacity_gate() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-59 smoke session");
    let position = session
        .mission()
        .authored_features()
        .iter()
        .find(|feature| feature.index == 343)
        .expect("authored checkpoint row")
        .world_position(0);
    let initial_global = session.economy().global_rng;
    let checkpoint = session
        .economy_mut()
        .allocate_authored_model59(343, position)
        .expect("checkpoint constructor has the source 32-row capacity");
    let expected_life_rng = level2_next_rng16(
        checkpoint
            .source_entity_index
            .wrapping_add(initial_global as u16),
    );
    let expected_parent_rng = level2_next_rng16(expected_life_rng);
    assert_eq!(checkpoint.max_life, (expected_life_rng % 0x64) as i16 + 800);
    assert_eq!(checkpoint.actual_speed, (expected_parent_rng % 0x11) as i16);

    let parent_rng_before_callback = checkpoint.entity_rng;
    let global_before_callback = session.economy().global_rng;
    assert!(
        session
            .economy_mut()
            .step_authored_model59_slot(checkpoint.source_entity_index)
    );
    let parent_rng_after_first = level2_next_rng16(parent_rng_before_callback);
    let parent_rng_after_two = level2_next_rng16(parent_rng_after_first);
    let parent_rng_after_child = level2_next_rng16(parent_rng_after_two);
    let child = session
        .economy()
        .model59_smoke_children()
        .first()
        .copied()
        .expect("checkpoint callback allocates a smoke child");
    let child_initial_rng = level2_next_rng16(
        child
            .source_entity_index
            .wrapping_add(level2_next_rng32(global_before_callback) as u16),
    );
    let expected_child_speed = (child_initial_rng % 0x35) as i16
        + 51
        + checkpoint.actual_speed
        + (parent_rng_after_child % 0x4D) as i16;
    assert_eq!(
        (
            session.economy().global_rng,
            parent_rng_after_child,
            child.entity_rng,
            child.actual_speed,
            child.life,
            child.particle_index,
        ),
        (
            level2_next_rng32(global_before_callback),
            parent_rng_after_child,
            child_initial_rng,
            expected_child_speed,
            32,
            67,
        )
    );
    assert!(
        session
            .economy_mut()
            .step_model59_smoke_child_slot(child.source_entity_index)
    );
    let stepped_child = session
        .economy()
        .model59_smoke_children()
        .first()
        .copied()
        .expect("smoke child remains live after its first callback");
    assert_eq!(
        (
            stepped_child.life,
            stepped_child.actual_speed,
            stepped_child.dword_0x10,
            stepped_child.particle_index,
        ),
        (31, (expected_child_speed - 4).clamp(64, 128), 1, 67)
    );
    let child_position_before_zero = stepped_child.position;
    let child_phase_before_zero = stepped_child.phase;
    let child_rng_before_zero = stepped_child.entity_rng;
    session
        .economy_mut()
        .model59_smoke_children
        .first_mut()
        .expect("model-59 child for life-zero callback")
        .life = 0;
    assert!(
        session
            .economy_mut()
            .step_model59_smoke_child_slot(stepped_child.source_entity_index)
    );
    let child_after_zero = session
        .economy()
        .model59_smoke_children()
        .first()
        .copied()
        .expect("model-59 child survives life-zero callback");
    assert_eq!(child_after_zero.life, -1);
    assert!(child_after_zero.drawing_enabled);
    assert_eq!(
        child_after_zero.phase,
        child_phase_before_zero.wrapping_add(1)
    );
    assert_ne!(child_after_zero.position, child_position_before_zero);
    assert_eq!(child_after_zero.entity_rng, child_rng_before_zero);

    let child_position_before_negative = child_after_zero.position;
    let child_phase_before_negative = child_after_zero.phase;
    session
        .economy_mut()
        .model59_smoke_children
        .first_mut()
        .expect("model-59 child for terminal callback")
        .life = -1;
    assert!(
        session
            .economy_mut()
            .step_model59_smoke_child_slot(child_after_zero.source_entity_index)
    );
    let child_after_negative = session
        .economy()
        .model59_smoke_children()
        .first()
        .copied()
        .expect("model-59 child remains retained after terminal callback");
    assert_eq!(child_after_negative.life, -2);
    assert!(!child_after_negative.drawing_enabled);
    assert_eq!(
        child_after_negative.phase,
        child_phase_before_negative.wrapping_add(1)
    );
    assert_eq!(
        child_after_negative.position,
        child_position_before_negative
    );

    let mut zero_parent = inspection
        .level2_reset_session()
        .expect("life-zero model-59 parent session");
    let zero_parent_position = zero_parent
        .mission()
        .authored_features()
        .iter()
        .find(|feature| feature.index == 343)
        .expect("authored checkpoint row for life-zero parent")
        .world_position(0);
    let zero_parent_source = zero_parent
        .economy_mut()
        .allocate_authored_model59(343, zero_parent_position)
        .expect("life-zero parent constructor");
    let zero_parent_phase = zero_parent_source.phase;
    zero_parent
        .economy_mut()
        .authored_model59
        .first_mut()
        .expect("life-zero parent source")
        .life = 0;
    assert!(
        zero_parent
            .economy_mut()
            .step_authored_model59_slot(zero_parent_source.source_entity_index)
    );
    let zero_parent_after = zero_parent
        .economy()
        .authored_model59()
        .first()
        .copied()
        .expect("life-zero parent remains live");
    assert_eq!(zero_parent_after.life, -1);
    assert!(zero_parent_after.drawing_enabled);
    assert_eq!(zero_parent_after.phase, zero_parent_phase.wrapping_add(1));
    assert_eq!(zero_parent.economy().model59_smoke_children().len(), 1);

    let mut negative_parent = inspection
        .level2_reset_session()
        .expect("terminal model-59 parent session");
    let negative_parent_position = negative_parent
        .mission()
        .authored_features()
        .iter()
        .find(|feature| feature.index == 343)
        .expect("authored checkpoint row for terminal parent")
        .world_position(0);
    let negative_parent_source = negative_parent
        .economy_mut()
        .allocate_authored_model59(343, negative_parent_position)
        .expect("terminal parent constructor");
    let negative_parent_global = negative_parent.economy().global_rng;
    let negative_parent_rng = negative_parent_source.entity_rng;
    let negative_parent_phase = negative_parent_source.phase;
    negative_parent
        .economy_mut()
        .authored_model59
        .first_mut()
        .expect("terminal parent source")
        .life = -1;
    assert!(
        negative_parent
            .economy_mut()
            .step_authored_model59_slot(negative_parent_source.source_entity_index)
    );
    let negative_parent_after = negative_parent
        .economy()
        .authored_model59()
        .first()
        .copied()
        .expect("terminal parent remains retained");
    assert_eq!(negative_parent_after.life, -2);
    assert!(!negative_parent_after.drawing_enabled);
    assert_eq!(
        negative_parent_after.phase,
        negative_parent_phase.wrapping_add(1)
    );
    assert_eq!(negative_parent_after.entity_rng, negative_parent_rng);
    assert_eq!(negative_parent.economy().global_rng, negative_parent_global);
    assert!(
        negative_parent
            .economy()
            .model59_smoke_children()
            .is_empty()
    );

    let mut exhausted = inspection
        .level2_reset_session()
        .expect("second Level 2 model-59 smoke session");
    let exhausted_position = exhausted
        .mission()
        .authored_features()
        .iter()
        .find(|feature| feature.index == 343)
        .expect("authored checkpoint row")
        .world_position(0);
    let exhausted_checkpoint = exhausted
        .economy_mut()
        .allocate_authored_model59(343, exhausted_position)
        .expect("checkpoint constructor allocates before exhaustion");
    let global_before_exhaustion_callback = exhausted.economy().global_rng;
    let parent_rng_before_exhaustion_callback = exhausted_checkpoint.entity_rng;
    exhausted.economy_mut().free_slots.clear();
    assert!(
        exhausted
            .economy_mut()
            .step_authored_model59_slot(exhausted_checkpoint.source_entity_index)
    );
    let exhausted_parent = exhausted
        .economy()
        .authored_model59()
        .first()
        .copied()
        .expect("exhausted checkpoint remains live");
    assert!(exhausted.economy().model59_smoke_children().is_empty());
    assert_eq!(
        (exhausted.economy().global_rng, exhausted_parent.entity_rng,),
        (
            level2_next_rng32(global_before_exhaustion_callback),
            level2_next_rng16(level2_next_rng16(parent_rng_before_exhaustion_callback)),
        )
    );

    let mut gated = inspection
        .level2_reset_session()
        .expect("third Level 2 model-59 smoke session");
    let gated_position = gated
        .mission()
        .authored_features()
        .iter()
        .find(|feature| feature.index == 343)
        .expect("authored checkpoint row")
        .world_position(0);
    gated.economy_mut().free_slots.truncate(31);
    let gated_global = gated.economy().global_rng;
    assert!(
        gated
            .economy_mut()
            .allocate_authored_model59(343, gated_position)
            .is_none()
    );
    assert_eq!(gated.economy().global_rng, gated_global);
    assert_eq!(gated.economy().free_slot_count(), 31);
}

#[test]
fn level_two_goat_action11_nonlethal_hit_reaches_partner_and_terrain_settle() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
        .expect("qualified Level 2 owner package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Goat action-11 hit session");
    let baseline_goat_hash = session.goat_hash();
    let attacker = session.state.entity_index;
    let mut primary = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 39)
        .cloned()
        .expect("authored action-11 Goat slot 39");
    let partner_baseline = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 66)
        .cloned()
        .expect("authored action-9 partner Goat slot 66");
    let original_xy = (primary.position.x, primary.position.y);
    let ground = session.canonical_terrain().ground_height(primary.position);
    primary.action = 11;
    primary.phase = 0;
    primary.partner_entity_index = Some(66);
    primary.position.z = ground.wrapping_add(300);
    assert!(session.goats.replace_goat_for_test(primary));
    session
        .submit_goat_damage(39, 100, attacker)
        .expect("nonlethal Level 2 action-11 hit");

    session.step_outer_turn(PlayerCommand::default(), 0);

    let primary = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 39)
        .expect("stepped action-11 Goat slot 39");
    assert_eq!(primary.life, 500);
    assert_eq!(primary.action, 14);
    assert_eq!(primary.target_entity_index, Some(attacker));
    assert_eq!(primary.partner_entity_index, None);
    assert_eq!((primary.position.x, primary.position.y), original_xy);
    assert_eq!(primary.position.z, ground.wrapping_add(44));
    assert_eq!(primary.pending_damage, 0);
    assert_eq!(primary.pending_attacker, 0);

    let partner = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 66)
        .expect("stepped partner Goat slot 66");
    assert_eq!(partner.life, partner_baseline.life);
    assert_eq!(partner.action, 14);
    assert_eq!(partner.target_entity_index, Some(attacker));
    assert_eq!(partner.partner_entity_index, None);

    session.reset();
    assert_eq!(session.goat_hash(), baseline_goat_hash);
}

#[test]
fn level_two_goat_action11_partner_follow_and_linked_scan_match_source() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
        .expect("qualified Level 2 owner package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Goat action-11 partner-follow session");
    let baseline_hash = session.goat_hash();
    let mut primary = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 39)
        .cloned()
        .expect("authored action-11 primary Goat");
    let mut partner = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 66)
        .cloned()
        .expect("authored action-9 partner Goat");
    let mut linked_peer = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 71)
        .cloned()
        .expect("authored linked-list peer Goat");

    let primary_source_id = primary.source_id;
    let primary_position = primary.position;
    let far_position = Position {
        x: primary_position.x.wrapping_add(0x1000),
        y: primary_position.y.wrapping_add(0x1000),
        z: primary_position.z,
    };
    for mut goat in session.goats().to_vec() {
        if matches!(goat.entity_index, 39 | 66 | 71) {
            continue;
        }
        // Source sub_1C560 rejects equal source IDs and then scans the
        // global model-1 linked list, so keep every other row out of the
        // strict 256-unit window and use the primary source ID.
        goat.source_id = primary_source_id;
        goat.position = far_position;
        assert!(session.goats.replace_goat_for_test(goat));
    }

    primary.action = 11;
    primary.phase = 0;
    primary.partner_entity_index = Some(66);
    primary.position = primary_position;
    primary.actual_speed = 0;
    primary.minimum_speed = 0;
    primary.maximum_speed = 0;
    assert!(session.goats.replace_goat_for_test(primary));

    partner.source_id = primary_source_id;
    partner.action = 9;
    partner.phase = 0;
    partner.partner_entity_index = None;
    partner.position = Position {
        x: primary_position.x.wrapping_add(80),
        y: primary_position.y.wrapping_add(40),
        z: primary_position.z,
    };
    partner.maximum_speed = 64;
    partner.actual_speed = 12;
    assert!(session.goats.replace_goat_for_test(partner));

    linked_peer.source_id = primary_source_id.wrapping_add(1);
    linked_peer.action = 9;
    linked_peer.phase = 1;
    linked_peer.partner_entity_index = None;
    linked_peer.position = Position {
        x: primary_position.x.wrapping_add(20),
        y: primary_position.y.wrapping_add(24),
        z: primary_position.z,
    };
    let linked_peer_position = linked_peer.position;
    assert!(session.goats.replace_goat_for_test(linked_peer));

    let expected_roll = i16::try_from(heading_for_positions(
        linked_peer_position,
        primary_position,
    ))
    .expect("linked peer heading fits i16");
    session.step_outer_turn(PlayerCommand::default(), 0);

    let stepped = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 39)
        .expect("primary Goat remains live after partner-follow callback");
    assert_eq!(stepped.action, 11);
    assert_eq!(stepped.partner_entity_index, Some(66));
    assert_eq!(stepped.actual_speed, 64 + 12);
    assert_eq!(stepped.roll, expected_roll);
    assert_eq!(stepped.phase, 1);

    session.reset();
    assert_eq!(session.goat_hash(), baseline_hash);
}

#[test]
fn level_two_goat_dispatch_uses_live_class3_player_feed_in_source_order() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Goat player-feed session");
    let local_player_index = usize::from(session.state.entity_index);
    let injected_index = session.economy.entities
        .iter()
        .position(|entity| entity.index != 0 && !entity.active)
        .expect("inactive source row for synthetic class-3 candidate");
    let second_injected_index = session.economy.entities
        .iter()
        .enumerate()
        .skip(injected_index + 1)
        .find(|(_, entity)| entity.index != 0 && !entity.active)
        .map(|(index, _)| index)
        .expect("second inactive source row for tie-break candidate");
    let mut primary = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 39)
        .cloned()
        .expect("authored Goat slot 39");
    primary.action = 9;
    primary.phase = 0;
    primary.position = Position {
        x: 10_000,
        y: 10_000,
        z: 0,
    };
    primary.yaw = 0;
    primary.roll = 0;
    primary.actual_speed = 0;
    primary.proximity_countdown = 2;
    primary.target_entity_index = None;
    primary.partner_entity_index = None;
    assert!(session.goats.replace_goat_for_test(primary));

    session.state.position = Position {
        x: 10_000,
        y: 9_100,
        z: 0,
    };
    session.economy.entities[local_player_index].control_flags = 0x20;
    session.economy.entities[local_player_index].position = Position {
        x: 40_000,
        y: 40_000,
        z: 0,
    };
    session.economy.entities[injected_index].active = true;
    session.economy.entities[injected_index].class = 3;
    // A non-model-0 class-3 row remains in the source linked list but is
    // not an action-10 player target (`sub_61620`'s `!model` gate).
    session.economy.entities[injected_index].model = 2;
    session.economy.entities[injected_index].life = 100;
    session.economy.entities[injected_index].position = Position {
        x: 10_000,
        y: 9_400,
        z: 0,
    };
    session.economy.entities[injected_index].control_flags = 0;
    session.economy.entities[second_injected_index].active = true;
    session.economy.entities[second_injected_index].class = 3;
    session.economy.entities[second_injected_index].model = 0;
    session.economy.entities[second_injected_index].life = 100;
    session.economy.entities[second_injected_index].position = Position {
        x: 10_000,
        y: 9_400,
        z: 0,
    };
    session.economy.entities[second_injected_index].control_flags = 0;
    session.economy.entities[112].model = 0;
    session.economy.entities[local_player_index].next_relation =
        Some(session.economy.entities[second_injected_index].index);
    session.economy.entities[local_player_index].record[4..8].copy_from_slice(
        &(u32::from(session.economy.entities[second_injected_index].index) + 1).to_le_bytes(),
    );
    session.economy.entities[second_injected_index].next_relation = Some(session.economy.entities[112].index);
    session.economy.entities[second_injected_index].record[4..8]
        .copy_from_slice(&(u32::from(session.economy.entities[112].index) + 1).to_le_bytes());
    session.economy.entities[112].next_relation = Some(session.economy.entities[injected_index].index);
    session.economy.entities[112].record[4..8]
        .copy_from_slice(&(u32::from(session.economy.entities[injected_index].index) + 1).to_le_bytes());
    session.economy.entities[injected_index].next_relation = None;
    session.economy.entities[injected_index].record[4..8].fill(0);
    let expected_player_slots = session.economy.entities
        .iter()
        .filter(|entity| {
            entity.index != 0
                && entity.active
                && entity.class == 3
                && entity.model == 0
                && entity.life >= 0
        })
        .map(|entity| entity.index)
        .collect::<Vec<_>>();
    let goat_player_targets = session.level2_goat_player_targets();
    let source_order = vec![
        session.state.entity_index,
        session.economy.entities[second_injected_index].index,
        session.economy.entities[112].index,
    ];
    assert_ne!(expected_player_slots, source_order);
    assert_eq!(
        goat_player_targets
            .iter()
            .map(|player| player.entity_index)
            .collect::<Vec<_>>(),
        source_order
    );
    assert!(
        !goat_player_targets
            .iter()
            .any(|player| player.entity_index == session.economy.entities[injected_index].index)
    );
    assert_eq!(
        goat_player_targets
            .iter()
            .find(|player| player.entity_index == session.state.entity_index)
            .expect("local class-3 row")
            .position,
        session.state.position
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    let goat = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 39)
        .expect("stepped Goat slot 39");
    assert_eq!(goat.action, 14);
    assert_eq!(
        goat.target_entity_index,
        Some(session.economy.entities[second_injected_index].index)
    );
    assert_eq!(goat.partner_entity_index, None);
}

#[test]
fn level_two_goat_action9_uses_source_movement_cleanup_on_same_tile_production_path() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
        .expect("qualified Level 2 owner package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Goat action-9 obstruction-marker session");
    let baseline_goat_hash = session.goat_hash();
    let mut primary = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 40)
        .cloned()
        .expect("authored action-9 Goat slot 40");
    let original_position = primary.position;
    primary.action = 9;
    // Avoid action-9's cadence-gated acquisition; this slice isolates
    // `sub_1B8C0`'s direct same-tile movement cleanup.
    primary.phase = 1;
    primary.actual_speed = 0;
    primary.proximity_countdown = 0;
    primary.target_entity_index = None;
    primary.partner_entity_index = None;
    primary.control_flags |= 0x0004_0000;
    assert!(session.goats.replace_goat_for_test(primary));

    session.step_outer_turn(PlayerCommand::default(), 0);
    let goat = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 40)
        .expect("stepped action-9 Goat slot 40");
    assert_eq!(goat.control_flags & 0x0004_0000, 0);
    assert_eq!(goat.position.x, original_position.x);
    assert_eq!(goat.position.y, original_position.y);
    assert_eq!(goat.action, 9);

    session.reset();
    assert_eq!(session.goat_hash(), baseline_goat_hash);
}

#[test]
fn level_two_goat_dispatch_falls_back_to_peer_when_all_live_players_are_ineligible() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Goat peer-fallback session");
    let local_player_index = usize::from(session.state.entity_index);
    let injected_index = session.economy.entities
        .iter()
        .position(|entity| entity.index != 0 && !entity.active)
        .expect("inactive source row for synthetic class-3 candidate");

    let mut primary = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 39)
        .cloned()
        .expect("authored Goat slot 39");
    primary.action = 9;
    primary.phase = 0;
    primary.position = Position {
        x: 10_000,
        y: 10_000,
        z: 0,
    };
    primary.yaw = 0;
    primary.roll = 0;
    primary.actual_speed = 0;
    // `begin_source_turn` consumes one countdown tick before action 9's
    // cadence gate, so seed two to exercise the live fallback at one.
    primary.proximity_countdown = 2;
    primary.target_entity_index = None;
    primary.partner_entity_index = None;
    assert!(session.goats.replace_goat_for_test(primary));

    let mut peer40 = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 40)
        .cloned()
        .expect("peer Goat slot 40");
    let mut peer41 = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 41)
        .cloned()
        .expect("peer Goat slot 41");
    peer40.position = Position {
        x: 10_000,
        y: 9_400,
        z: 0,
    };
    peer41.position = Position {
        x: 10_000,
        y: 9_200,
        z: 0,
    };
    peer40.partner_entity_index = None;
    peer41.partner_entity_index = Some(66);
    assert!(session.goats.replace_goat_for_test(peer40));
    assert!(session.goats.replace_goat_for_test(peer41));

    session.state.position = Position {
        x: 10_000,
        y: 9_100,
        z: 0,
    };
    session.economy.entities[local_player_index].control_flags = 0x20;
    session.economy.entities[local_player_index].position = session.state.position;
    session.economy.entities[injected_index].active = true;
    session.economy.entities[injected_index].class = 3;
    session.economy.entities[injected_index].model = 0;
    session.economy.entities[injected_index].life = 100;
    session.economy.entities[injected_index].position = Position {
        x: 10_000,
        y: 9_300,
        z: 0,
    };
    session.economy.entities[injected_index].control_flags = 0x20;
    session.economy.entities[112].next_relation = Some(session.economy.entities[injected_index].index);
    session.economy.entities[112].record[4..8]
        .copy_from_slice(&(u32::from(session.economy.entities[injected_index].index) + 1).to_le_bytes());
    session.economy.entities[injected_index].next_relation = None;
    session.economy.entities[injected_index].record[4..8].fill(0);

    session.step_outer_turn(PlayerCommand::default(), 0);
    let goat = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 39)
        .expect("stepped Goat slot 39");
    assert_eq!(goat.action, 11);
    assert_eq!(goat.target_entity_index, None);
    assert_eq!(goat.partner_entity_index, Some(40));
}

#[test]
fn level_two_goat_action9_peer_fallback_requires_proximity_countdown() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
        .expect("qualified Level 2 owner package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 action-9 peer-fallback session");

    let configure = |session: &mut Level2ResetSession, proximity_countdown: i8| {
        let mut rows = session.goats().to_vec();
        for goat in &mut rows {
            if !matches!(goat.entity_index, 40 | 41) {
                goat.position = Position {
                    x: 30_000,
                    y: 30_000,
                    z: 0,
                };
                goat.partner_entity_index = Some(66);
            }
        }
        let primary = rows
            .iter_mut()
            .find(|goat| goat.entity_index == 40)
            .expect("authored action-9 Goat slot 40");
        primary.action = 9;
        primary.phase = 0;
        primary.position = Position {
            x: 10_000,
            y: 10_000,
            z: 0,
        };
        primary.yaw = 0;
        primary.actual_speed = 0;
        primary.proximity_countdown = proximity_countdown;
        primary.target_entity_index = None;
        primary.partner_entity_index = None;
        let peer = rows
            .iter_mut()
            .find(|goat| goat.entity_index == 41)
            .expect("authored unpaired peer slot 41");
        peer.position = Position {
            x: 10_000,
            y: 9_400,
            z: 0,
        };
        peer.partner_entity_index = None;
        for goat in rows {
            assert!(session.goats.replace_goat_for_test(goat));
        }
        for entity in session.economy.entities.iter_mut().filter(|entity| {
            entity.active && entity.class == 3 && entity.model == 0 && entity.life >= 0
        }) {
            entity.control_flags |= 0x20;
        }
    };

    // `begin_source_turn` consumes one countdown tick before the entity
    // callback, so seed two to enter action-9's nonzero source branch.
    configure(&mut session, 2);
    session.step_outer_turn(PlayerCommand::default(), 0);
    let goat = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 40)
        .expect("stepped action-9 Goat slot 40");
    assert_eq!(goat.action, 11);
    assert_eq!(goat.partner_entity_index, Some(41));
    assert_eq!(goat.target_entity_index, None);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::Goat {
            source_slot: 40,
            ..
        }
    )));

    session.reset();
    configure(&mut session, 0);
    session.step_outer_turn(PlayerCommand::default(), 0);
    let gated = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 40)
        .expect("gated action-9 Goat slot 40");
    assert_eq!(gated.action, 9);
    assert_eq!(gated.partner_entity_index, None);
    assert_eq!(gated.target_entity_index, None);
}

#[test]
fn level_two_goat_action14_production_distinguishes_ordinary_and_direct_hit_tails() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
        .expect("qualified Level 2 owner package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 action-14 tail session");
    let local_player_index = session.state.entity_index;
    let initial_position = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 39)
        .expect("authored Goat slot 39")
        .position;
    session.state.position = initial_position;
    session.economy.entities[usize::from(local_player_index)].position = initial_position;

    let mut ordinary = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 39)
        .cloned()
        .expect("authored Goat slot 39");
    ordinary.action = 14;
    ordinary.phase = 1;
    ordinary.rng = 129;
    ordinary.actual_speed = 0;
    ordinary.target_entity_index = Some(local_player_index);
    assert!(session.goats.replace_goat_for_test(ordinary));
    session.step_outer_turn(PlayerCommand::default(), 0);
    let ordinary_after = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 39)
        .expect("ordinary action-14 Goat");
    assert_eq!(ordinary_after.action, 14);
    assert_eq!(ordinary_after.rng, 39_424);
    assert_eq!(ordinary_after.actual_speed, ordinary_after.minimum_speed);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::Goat {
            source_slot: 39,
            events,
            ..
        } if events.iter().any(|event| matches!(
            event,
            GoatStepEvent::Bleat {
                entity_index: 39,
                sound_id: 46,
                action: 14,
                ..
            }
        ))
    )));

    session.reset();
    session.state.position = initial_position;
    session.economy.entities[usize::from(local_player_index)].position = initial_position;
    let mut direct_hit = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 39)
        .cloned()
        .expect("authored Goat slot 39 after reset");
    direct_hit.action = 14;
    direct_hit.phase = 1;
    direct_hit.rng = 62;
    direct_hit.target_entity_index = Some(local_player_index);
    assert!(session.goats.replace_goat_for_test(direct_hit));
    session
        .submit_goat_damage(39, 1, local_player_index)
        .expect("direct action-14 Goat hit");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let hit_after = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 39)
        .expect("direct-hit action-14 Goat");
    assert_eq!(hit_after.action, 14);
    assert_eq!(hit_after.rng, 989);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::Goat {
            source_slot: 39,
            events,
            ..
        } if events.iter().any(|event| matches!(
            event,
            GoatStepEvent::Bleat {
                entity_index: 39,
                sound_id: 46,
                action: 14,
                ..
            }
        ))
    )));
}

#[test]
fn level_two_goat_death_respects_source_burst_suppression_bit() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 suppressed Goat session");
    let goat_slot = LEVEL_TWO_GOAT_INDICES[0];
    assert!(session.goats.set_control_flags(goat_slot, 0x0010_0000));
    session
        .goats
        .inject_damage(
            goat_slot,
            1_000_000,
            session.state.entity_index.cast_signed(),
        )
        .expect("inject lethal Goat damage");

    let mut observed_death = false;
    for _ in 0..32 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        if session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Goat {
                    source_slot,
                    events,
                    ..
                } if *source_slot == goat_slot
                    && events.iter().any(|event| matches!(
                        event,
                        GoatStepEvent::ExtraManaSphereRequested { .. }
                    ))
            )
        }) {
            observed_death = true;
            break;
        }
    }
    assert!(observed_death, "suppressed Goat reached KillEntity tail");
    assert!(!session.economy().events().iter().any(|event| {
        matches!(event, Level2EconomyEvent::DeathBurstSpawned {
                parent_entity_index,
                ..
            } if *parent_entity_index == goat_slot)
    }));
}

#[test]
fn level_two_goat_production_rejects_tampered_parameter_row() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut tampered = inspection.clone();
    tampered.presentation.entity_parameters[98][2] ^= 1;
    let error = tampered
        .level2_reset_session()
        .expect_err("row 98 tamper must fail closed");
    assert!(error.to_string().contains("row 98 mismatch"));
}

#[test]
fn level_two_goat_production_rejects_wrong_source_row_order() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut tampered = inspection;
    tampered.entities.swap(39, 40);
    let error = tampered
        .level2_reset_session()
        .expect_err("swapped authored Goat rows must fail closed");
    assert!(
        error
            .to_string()
            .contains("reset source slot order mismatch")
    );
}

#[test]
fn dolmen_contact_selects_high_recharge_on_the_following_player_action() {
    let (package, closure) = fixture_paths();
    let world = NativeWorld::load(package, closure).expect("validated package M");
    let dolmen = world
        .entities()
        .iter()
        .find(|entity| {
            entity.active && entity.class == 2 && entity.model == 2 && entity.action == 6
        })
        .expect("Level 1 dolmen");
    let mut session = world.session();
    session.state.position = dolmen.position;
    session.state.actual_speed = 0;
    session.state.target_speed = 0;
    session.mission.player_mana = 0;
    session.mission.player_max_mana = 10_000;

    session.step(PlayerCommand::default());
    assert_eq!(
        session.mission.player_mana,
        crate::level1_mission::PLAYER_OUTDOOR_MANA_REGEN,
        "the dolmen status bit changes the next regen value after this tick's regen"
    );

    session.state.position.x = session.state.position.x.wrapping_add(4_096);
    session.step(PlayerCommand::default());
    assert_eq!(
        session.mission.player_mana,
        crate::level1_mission::PLAYER_OUTDOOR_MANA_REGEN
            + crate::level1_mission::PLAYER_RECHARGE_MINIMUM_MANA_REGEN,
        "the latched high recharge value is consumed once after leaving"
    );

    session.step(PlayerCommand::default());
    assert_eq!(
        session.mission.player_mana,
        crate::level1_mission::PLAYER_OUTDOOR_MANA_REGEN * 2
            + crate::level1_mission::PLAYER_RECHARGE_MINIMUM_MANA_REGEN,
        "the following out-of-range tick returns to outdoor recharge"
    );
}

#[test]
fn level_two_public_family0_fireball_reaches_authored_bee_mailbox() {
    let package = level_two_fixture_path();
    let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level2-day-owner-cache/components/gameplay");
    let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
        .expect("qualified Level 2 gameplay component");
    let mut session = inspection
        .level2_reset_session()
        .expect("qualified Level 2 session");
    session.select_level2_campaign_entry_stage();
    let baseline_hash = session.campaign_state_hash();
    let bee_slot = 119;

    // Use the ordinary authored action-23 -> action-18 -> action-17
    // transitions to qualify the real Day-Bee row. No Bee field is
    // written by this setup; only the public local-player position is
    // supplied to the source turn.
    for _ in 0..100 {
        let bee = session
            .economy()
            .authored_bees()
            .iter()
            .find(|bee| bee.source_entity_index == bee_slot)
            .copied();
        if let Some(bee) = bee {
            if bee.action == 17 {
                break;
            }
            let player_position = if bee.action == 23 {
                let mut position = bee.position;
                crate::move_entity(&mut position, bee.yaw, 0, 1_000);
                position.z = bee.position.z;
                position
            } else {
                Position { x: 0, y: 0, z: 0 }
            };
            session
                .set_level2_entity_position(session.state.entity_index, player_position)
                .expect("local player position remains map-linked");
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        if session
            .economy()
            .authored_bees()
            .iter()
            .find(|bee| bee.source_entity_index == bee_slot)
            .is_some_and(|bee| bee.action == 17)
        {
            break;
        }
    }
    let bee = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == bee_slot)
        .copied()
        .expect("action-17 production Bee");
    assert_eq!(bee.action, 17);

    // The public Fireball is launched close enough for the source's
    // first non-delayed collision scan, while the player remains the
    // ordinary owner (entity 111). The Bee stays on its source callback;
    // the following callback consumes the mailbox.
    let player_position = Position {
        x: bee.position.x.wrapping_add(256),
        y: bee.position.y.wrapping_add(360),
        z: bee
            .position
            .z
            .wrapping_add(280)
            .wrapping_sub(session.state.entity_fov),
    };
    session
        .set_level2_entity_position(session.state.entity_index, player_position)
        .expect("Fireball launch position remains map-linked");
    session.state.yaw = 0;
    session.state.display_pitch = 0;
    session.state.effective_pitch = 0;
    session.state.actual_speed = 0;
    session.state.target_speed = 0;
    let launch_event_start = session.shared_spells().activation_events().len();
    session.step_outer_turn(
        PlayerCommand {
            opcode: 6,
            argument_1: 0,
            flags: 0x40,
            ..PlayerCommand::default()
        },
        0,
    );
    let projectile = session
        .economy()
        .fireball_projectiles()
        .iter()
        .find(|projectile| projectile.active)
        .copied()
        .expect("ordinary family-0 cast allocates a live Fireball row");
    assert_eq!(
        (
            projectile.source_class,
            projectile.source_model,
            projectile.action
        ),
        (9, 0, 0)
    );
    assert_eq!(projectile.owner_entity_index, session.state.entity_index);
    assert_eq!(projectile.holder_entity_index, 115);
    assert_eq!(projectile.source_subspell_index, 250);
    assert_eq!(projectile.source_mana, 20);
    assert_eq!(projectile.phase, 1);
    assert_eq!(projectile.life, projectile.max_life);
    assert_eq!(projectile.control_flags, 0);
    assert_eq!(projectile.dword_0x10, 0);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::DynamicArena {
            source_slot,
            kind: Level2DynamicArenaKind::FireballProjectile,
            ..
        } if *source_slot == projectile.source_entity_index
    )));
    let launch_events = &session.shared_spells().activation_events()[launch_event_start..];
    assert!(launch_events.iter().any(|event| matches!(
        event,
        FlightEvent::PrepareSound {
            audio_site: crate::MissionAudioSite::FireballLaunch,
            entity_index,
            player_index: -1,
            sound_id: 9,
        } if *entity_index == projectile.source_entity_index
    )));

    let launch_rng = projectile.entity_rng;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let hit_projectile = session
        .economy()
        .fireball_projectiles()
        .iter()
        .find(|candidate| candidate.source_entity_index == projectile.source_entity_index)
        .copied()
        .expect("Fireball remains inspectable after its first collision callback");
    assert_eq!(hit_projectile.control_flags & 0x2, 0x2);
    assert_eq!(
        hit_projectile.entity_rng,
        level2_next_rng16(launch_rng),
        "the first non-delayed callback consumes exactly one source RNG draw"
    );
    assert_eq!(
        hit_projectile.source_word_2c,
        i16::try_from(hit_projectile.entity_rng % 0x41).expect("Fireball RNG fits") - 32
    );
    let pending = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == bee_slot)
        .copied()
        .expect("target Bee remains live after mailbox write");
    assert_eq!(pending.pending_damage, 250);
    assert_eq!(
        pending.pending_attacker_entity_index,
        session.state.entity_index
    );
    assert_eq!(pending.damage_attacker_entity_index, 0);
    assert!(
        session
            .shared_spells()
            .activation_events()
            .iter()
            .any(|event| {
                matches!(
                    event,
                    FlightEvent::PrepareSound {
                        audio_site: crate::MissionAudioSite::FireballImpact,
                        entity_index,
                        player_index: -1,
                        sound_id: 3,
                    } if *entity_index == projectile.source_entity_index
                )
            })
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    let consumed = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == bee_slot)
        .copied()
        .expect("target Bee remains addressable after mailbox consumption");
    assert_eq!(consumed.life, pending.life - 250);
    assert_eq!(consumed.pending_damage, 250);
    assert_eq!(consumed.pending_attacker_entity_index, 0);
    assert_eq!(
        consumed.damage_attacker_entity_index,
        session.state.entity_index
    );
    assert_eq!(consumed.kill_credit_entity_index, 0);
    assert_eq!(consumed.action, 18);
    assert_eq!(
        consumed.target_entity_index,
        Some(session.state.entity_index)
    );
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::Economy {
            source_slot,
            event: Level2EconomyEvent::DamageApplied {
                target_entity_index,
                attacker_entity_index,
                damage: 250,
                lethal: false,
            },
            ..
        } if *source_slot == bee_slot
            && *target_entity_index == bee_slot
            && *attacker_entity_index == session.state.entity_index
    )));

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    assert!(session.economy().fireball_projectiles().is_empty());
}

fn drive_level_two_bee_to_action17_for_fireball(
    session: &mut Level2ResetSession,
    bee_slot: u16,
) -> Level2AuthoredBeeSource {
    for _ in 0..160 {
        let bee = session
            .economy()
            .authored_bees()
            .iter()
            .find(|bee| bee.source_entity_index == bee_slot)
            .copied()
            .expect("production Bee remains during Fireball qualification");
        if bee.action == 17 {
            return bee;
        }
        assert!(
            !matches!(bee.action, 20 | 21),
            "Bee reached death before hit"
        );
        let player_position = if bee.action == 23 {
            let mut position = bee.position;
            crate::move_entity(&mut position, bee.yaw, 0, 1_000);
            position.z = bee.position.z;
            position
        } else {
            // Keep the real player row valid but outside both the action-18
            // awareness threshold and the next action-17 scan while the
            // source cursor lets the Bee return to its firing state. This is
            // public flight positioning, not a mutation of the Bee row.
            Position {
                x: bee.position.x.wrapping_add(20_000),
                y: bee.position.y.wrapping_add(20_000),
                z: bee.position.z,
            }
        };
        session
            .set_level2_entity_position(session.state.entity_index, player_position)
            .expect("local player remains map-linked while qualifying Fireball");
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    let bee = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == bee_slot)
        .copied();
    panic!("production Bee did not return to action 17 for Fireball: {bee:?}");
}

fn cast_level_two_family0_fireball_at_bee(
    session: &mut Level2ResetSession,
    bee_slot: u16,
) -> Level2AuthoredBeeSource {
    let bee = drive_level_two_bee_to_action17_for_fireball(session, bee_slot);
    let expected_life = bee.life.wrapping_sub(250);
    // Both source hand directions move the launch carrier 256 units left at
    // yaw zero. The holder adds no Y displacement; a 120-unit Z line keeps
    // the first callback inside the strict Bee volume across the authored
    // terrain heights.
    let (x_offset, y_offset, z_offset) = (256, 360, 120);
    let player_position = Position {
        x: bee.position.x.wrapping_add(x_offset),
        y: bee.position.y.wrapping_add(y_offset),
        z: bee
            .position
            .z
            .wrapping_add(z_offset)
            .wrapping_sub(session.state.entity_fov),
    };
    session
        .set_level2_entity_position(session.state.entity_index, player_position)
        .expect("Fireball launch position remains map-linked");
    session.state.yaw = 0;
    session.state.display_pitch = 0;
    session.state.effective_pitch = 0;
    session.state.roll_accumulator = 0;
    session.state.pitch_accumulator = 0;
    session.state.roll_delta = 0;
    session.state.pitch_delta = 0;
    session.state.actual_speed = 0;
    session.state.target_speed = 0;
    session.state.strafe_speed = 0;
    session.state.recoil_speed = 0;
    session.state.impulse_x = 0;
    session.state.impulse_y = 0;
    session.state.impulse_z = 0;
    let previous_projectile_slots = session
        .economy()
        .fireball_projectiles()
        .iter()
        .map(|projectile| projectile.source_entity_index)
        .collect::<BTreeSet<_>>();
    session.step_outer_turn(
        PlayerCommand {
            opcode: 6,
            argument_1: 0,
            flags: 0x40,
            ..PlayerCommand::default()
        },
        0,
    );
    let _projectile = session
        .economy()
        .fireball_projectiles()
        .iter()
        .find(|projectile| {
            projectile.active
                && !previous_projectile_slots.contains(&projectile.source_entity_index)
        })
        .copied()
        .expect("ordinary family-0 cast allocates a fresh Fireball row");
    let player_entity_index = session.state.entity_index;
    let mut consumed = None;
    for _pass in 0..=16 {
        let current = session
            .economy()
            .authored_bees()
            .iter()
            .find(|candidate| candidate.source_entity_index == bee_slot)
            .copied()
            .expect("Fireball leaves the target Bee addressable after collision");
        if current.life == expected_life
            && current.pending_damage == 250
            && current.pending_attacker_entity_index == 0
            && current.damage_attacker_entity_index == player_entity_index
            && matches!(current.action, 18 | 20)
        {
            consumed = Some(current);
            break;
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    let consumed = consumed.unwrap_or_else(|| {
        let bee = session
            .economy()
            .authored_bees()
            .iter()
            .find(|candidate| candidate.source_entity_index == bee_slot)
            .copied();
        panic!("Fireball mailbox missing: expected_life={expected_life}, bee={bee:?}");
    });
    assert_eq!(consumed.pending_damage, 250);
    assert_eq!(consumed.pending_attacker_entity_index, 0);
    assert_eq!(consumed.damage_attacker_entity_index, player_entity_index);
    consumed
}

#[test]
fn level_two_public_fireball_lethal_bee_waits_for_phase_zero_and_credits_objective() {
    let inspection = NativeWorld::inspect_level2_with_owner_cache(
        &level_two_fixture_path(),
        &level_two_gameplay_cache_path(),
    )
    .expect("qualified Level 2 gameplay component");
    let mut session = inspection
        .level2_reset_session()
        .expect("qualified Level 2 session");
    let baseline_hash = session.campaign_state_hash();
    session
        .runtime
        .set_level2_objective_text_selector(LEVEL2_BEE_OBJECTIVE_STAGE_INDEX);
    session.step_outer_turn(PlayerCommand::default(), 0);
    let bee_slot = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.authored_index == LEVEL2_BEE_OBJECTIVE_AUTHORED_INDEX)
        .map(|bee| bee.source_entity_index)
        .expect("ordinary reset group-21 route materializes row 229");

    let mut last_consumed = None;
    for _ in 0..13 {
        last_consumed = Some(cast_level_two_family0_fireball_at_bee(
            &mut session,
            bee_slot,
        ));
    }
    let lethal = last_consumed.expect("thirteenth Fireball consumption");
    assert_eq!(lethal.action, 20);
    assert_eq!(lethal.life, -250);
    assert_eq!(lethal.pending_damage, 250);
    assert_eq!(lethal.pending_attacker_entity_index, 0);
    assert_eq!(
        lethal.damage_attacker_entity_index,
        session.state.entity_index
    );
    assert_eq!(lethal.kill_credit_entity_index, session.state.entity_index);

    session.step_outer_turn(PlayerCommand::default(), 0);
    let pre_kill = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == bee_slot)
        .copied()
        .expect("action-21 Bee remains before phase-gated kill");
    assert_eq!(pre_kill.action, 21);
    assert!(pre_kill.active);
    assert!(pre_kill.drawing_enabled);
    assert_eq!(
        session
            .economy()
            .authored_bee_kill_census(session.state.entity_index),
        1
    );
    assert_ne!(pre_kill.source_byte_3e & 7, 0);
    let output_counts = |session: &Level2ResetSession| {
        (
            session
                .economy()
                .events()
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        Level2EconomyEvent::ManaSphereSpawned { source_entity_index, .. }
                            if *source_entity_index == bee_slot
                    )
                })
                .count(),
            session
                .economy()
                .events()
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        Level2EconomyEvent::DeathBurstSpawned { parent_entity_index, .. }
                            if *parent_entity_index == bee_slot
                    )
                })
                .count(),
        )
    };
    assert_eq!(output_counts(&session), (0, 0));
    let mut phase_zero_callback_seen = false;
    let mut reached_reclaim = false;
    for _ in 0..10 {
        let before = session
            .economy()
            .authored_bees()
            .iter()
            .find(|bee| bee.source_entity_index == bee_slot)
            .copied();
        let before_counts = output_counts(&session);
        session.step_outer_turn(PlayerCommand::default(), 0);
        let after = session
            .economy()
            .authored_bees()
            .iter()
            .find(|bee| bee.source_entity_index == bee_slot)
            .copied();
        let Some(after) = after else {
            assert!(phase_zero_callback_seen);
            reached_reclaim = true;
            break;
        };
        let after_counts = output_counts(&session);
        if before
            .expect("Bee remains until the following reclaim boundary")
            .source_byte_3e
            & 7
            == 0
        {
            phase_zero_callback_seen = true;
            assert!(!after.active);
            assert!(!after.drawing_enabled);
            assert_eq!(after_counts, (1, 1));
        } else {
            assert!(after.active);
            assert!(after.drawing_enabled);
            assert_eq!(after_counts, before_counts);
        }
    }
    assert!(reached_reclaim, "phase-zero Bee callback reclaims the row");
    assert!(phase_zero_callback_seen);
    assert_eq!(
        session
            .economy()
            .authored_bee_kill_census(session.state.entity_index),
        1
    );
    assert_eq!(
        session.runtime().objectives[0].stage_selectors[LEVEL2_BEE_OBJECTIVE_STAGE_INDEX],
        2
    );
    assert_eq!(output_counts(&session), (1, 1));
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    assert_eq!(
        session
            .economy()
            .authored_bee_kill_census(session.state.entity_index),
        0
    );
}

#[test]
fn level_two_family0_fireball_rejects_invalid_producer_admission_and_capacity() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("qualified Level 2 session");
    session.select_level2_campaign_entry_stage();

    // A public family-0 opcode without the source cast flag is a closed
    // no-op: the holder is not armed and no dynamic row or event is
    // published.
    session.step_outer_turn(
        PlayerCommand {
            opcode: 6,
            argument_1: 0,
            flags: 0,
            ..PlayerCommand::default()
        },
        0,
    );
    assert!(session.economy().fireball_projectiles().is_empty());
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::DynamicArena {
            kind: Level2DynamicArenaKind::FireballProjectile,
            ..
        }
    )));
    session.reset();

    let source_rng_before = session.economy().source_rng.clone();
    let free_slots_before = session.economy().free_slots.clone();
    macro_rules! assert_no_launch {
        () => {
            assert!(session.economy().fireball_projectiles().is_empty());
            assert_eq!(session.economy().source_rng, source_rng_before);
            assert_eq!(session.economy().free_slots, free_slots_before);
            assert!(
                session
                    .shared_spells()
                    .activation_events()
                    .iter()
                    .all(|event| !matches!(
                        event,
                        FlightEvent::PrepareSound {
                            audio_site: crate::MissionAudioSite::FireballLaunch,
                            ..
                        }
                    ))
            );
        };
    }

    // The producer is keyed by the acquired holder's exact class/family
    // and supported tier. Wrong family, an unsupported tier, or an absent
    // holder cannot borrow a neighboring route.
    session
        .shared_spells_mut()
        .dynamic_pickups
        .retain(|holder| holder.index != 115);
    assert!(
        session
            .step_level2_fireball_holder_source_slot(115)
            .is_none()
    );
    assert_no_launch!();
    session.reset();

    {
        let mut shared_view = session.shared_spells_mut();
        let holder = shared_view
            .dynamic_pickups
            .iter_mut()
            .find(|holder| holder.index == 115)
            .expect("materialized family-0 holder");
        holder.model = POSSESSION_SPELL_FAMILY;
        holder.active_value = holder.activation_value.cast_signed();
        holder.record[40] = POSSESSION_SPELL_FAMILY;
        holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
    }
    assert!(
        session
            .step_level2_fireball_holder_source_slot(115)
            .is_none()
    );
    assert_no_launch!();
    session.reset();

    {
        let mut shared_view = session.shared_spells_mut();
        let holder = shared_view
            .dynamic_pickups
            .iter_mut()
            .find(|holder| holder.index == 115)
            .expect("materialized family-0 holder");
        holder.level_gate = 3;
        holder.active_value = holder.activation_value.cast_signed();
        holder.record[74] = 3;
        holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
    }
    assert!(
        session
            .step_level2_fireball_holder_source_slot(115)
            .is_none()
    );
    assert_no_launch!();
    session.reset();

    // Fresh Fireball admission requires the source max-mana threshold;
    // failure consumes only the source holder countdown, never an arena
    // slot, NewEvent RNG draw, or launch sound.
    {
        let mut shared = session.shared_spells_mut();
        let max_mana = {
            let holder = shared
                .dynamic_pickups
                .iter_mut()
                .find(|holder| holder.index == 115)
                .expect("materialized family-0 holder");
            holder.active_value = holder.activation_value.cast_signed();
            holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
            holder.max_mana
        };
        shared.player_entity.mana = max_mana - 1;
        let mana = shared.player_entity.mana;
        shared.player_entity.record[148..152]
            .copy_from_slice(&mana.to_le_bytes());
    }
    assert!(
        session
            .step_level2_fireball_holder_source_slot(115)
            .is_none()
    );
    assert_no_launch!();
    assert_eq!(
        session
            .shared_spells()
            .dynamic_pickups
            .iter()
            .find(|holder| holder.index == 115)
            .expect("family-0 holder remains after failed gate")
            .active_value,
        0
    );
    session.reset();

    // Exhausting the ordinary free stack and disabling every existing
    // replaceable carrier makes NewEvent fail before it consumes its RNG
    // or publishes a partial Fireball row.
    let mut exhausted = session.economy().clone();
    exhausted.free_slots.clear();
    for row in &mut exhausted.castle_children {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.spheres {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.death_bursts {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.authored_model1 {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.model59_smoke_children {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.authored_model5_water_splashes {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.authored_model13 {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.authored_model6 {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.model13_spawned_children {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.model17_spawned_children {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.model76_tail_spawns {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    let before_exhaustion = exhausted.clone();
    assert!(
        exhausted
            .spawn_fireball_projectile_with_payload(
                level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
                    position: Position::default(),
                    yaw: 0,
                    pitch: 0,
                    axis_position: Position::default(),
                    source_id_entity_index: 111,
                    holder_entity_index: 115,
                    source_subspell_index: 250,
                    source_mana: 20,
                    source_dword_0x10: 0,
                },
                0,
            )
            .is_none()
    );
    assert_eq!(exhausted, before_exhaustion);
}

#[test]
fn level_two_fireball_collision_rejects_invalid_targets_and_duplicate_mailbox() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("qualified Level 2 session");
    session.select_level2_campaign_entry_stage();
    let bee_slot = 119;

    // Produce the authored Bee through its ordinary source transition so
    // every collision negative below still resolves against the live Day
    // row and its reset map relation.
    for _ in 0..100 {
        let bee = session
            .economy()
            .authored_bees()
            .iter()
            .find(|bee| bee.source_entity_index == bee_slot)
            .copied();
        if let Some(bee) = bee {
            if bee.action == 17 {
                break;
            }
            let player_position = if bee.action == 23 {
                let mut position = bee.position;
                crate::move_entity(&mut position, bee.yaw, 0, 1_000);
                position.z = bee.position.z;
                position
            } else {
                Position { x: 0, y: 0, z: 0 }
            };
            session
                .set_level2_entity_position(session.state.entity_index, player_position)
                .expect("local player position remains map-linked");
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        if session
            .economy()
            .authored_bees()
            .iter()
            .find(|bee| bee.source_entity_index == bee_slot)
            .is_some_and(|bee| bee.action == 17)
        {
            break;
        }
    }
    let bee = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.source_entity_index == bee_slot)
        .copied()
        .expect("action-17 production Bee");
    assert_eq!(bee.action, 17);
    let qualified_economy = session.economy().clone();
    let terrain = session.canonical_terrain().clone();
    let edge_x = u16::try_from(
        i32::from(LEVEL2_FIREBALL_COLLISION_HALF_WIDTH)
            + i32::from(LEVEL2_BEE_COLLISION_HALF_EXTENT),
    )
    .expect("Fireball/Bee edge extent fits");

    let new_projectile = || {
        let mut economy = qualified_economy.clone();
        let source_slot = economy
            .spawn_fireball_projectile_with_payload(
                level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
                    position: bee.position,
                    yaw: 0,
                    pitch: 0,
                    axis_position: bee.position,
                    source_id_entity_index: session.state.entity_index,
                    holder_entity_index: 115,
                    source_subspell_index: 250,
                    source_mana: 20,
                    source_dword_0x10: 0,
                },
                0,
            )
            .expect("qualified negative case has an arena slot");
        let projectile = economy
            .fireball_projectiles
            .iter_mut()
            .find(|projectile| projectile.source_entity_index == source_slot)
            .expect("new Fireball row is stored in the shared arena");
        projectile.actual_speed = 0;
        (economy, source_slot)
    };

    // A zero/edge overlap, same owner, disabled/absent/stale target, and
    // every wrong class/model/action are all closed without a mailbox.
    for case in 0..9_u8 {
        let (mut economy, source_slot) = new_projectile();
        match case {
            0 => {
                economy
                    .fireball_projectiles
                    .iter_mut()
                    .find(|projectile| projectile.source_entity_index == source_slot)
                    .expect("Fireball row")
                    .position
                    .x = bee.position.x.wrapping_add(1_000);
            }
            1 => {
                economy
                    .fireball_projectiles
                    .iter_mut()
                    .find(|projectile| projectile.source_entity_index == source_slot)
                    .expect("Fireball row")
                    .position
                    .x = bee.position.x.wrapping_add(edge_x);
            }
            2 => {
                economy
                    .fireball_projectiles
                    .iter_mut()
                    .find(|projectile| projectile.source_entity_index == source_slot)
                    .expect("Fireball row")
                    .owner_entity_index = bee.entity_id;
            }
            3 => {
                economy
                    .authored_bees
                    .iter_mut()
                    .find(|target| target.source_entity_index == bee_slot)
                    .expect("target Bee")
                    .drawing_enabled = false;
            }
            4 => economy
                .authored_bees
                .retain(|target| target.source_entity_index != bee_slot),
            5 => economy.map_chains.set_map_entity_unlink(bee_slot),
            6 => {
                economy
                    .authored_bees
                    .iter_mut()
                    .find(|target| target.source_entity_index == bee_slot)
                    .expect("target Bee")
                    .source_class = 4;
            }
            7 => {
                economy
                    .authored_bees
                    .iter_mut()
                    .find(|target| target.source_entity_index == bee_slot)
                    .expect("target Bee")
                    .model = 1;
            }
            8 => {
                economy
                    .authored_bees
                    .iter_mut()
                    .find(|target| target.source_entity_index == bee_slot)
                    .expect("target Bee")
                    .action = 18;
            }
            _ => unreachable!("bounded Fireball negative case"),
        }
        let before_rng = economy
            .source_rng
            .get(&source_slot)
            .copied()
            .expect("NewEvent source RNG");
        let _ = economy.step_fireball_projectile_slot(source_slot, &terrain);
        let pending_damage = economy
            .authored_bees
            .iter()
            .find(|target| target.source_entity_index == bee_slot)
            .map_or(0, |target| target.pending_damage);
        assert_eq!(pending_damage, 0, "negative Fireball case {case}");
        assert_eq!(
            economy.source_rng.get(&source_slot).copied(),
            Some(before_rng.wrapping_mul(9_377).wrapping_add(9_439)),
            "the source callback still consumes its pinned one-shot RNG tail for case {case}"
        );
    }

    // A second callback cannot duplicate a previously occupied mailbox or
    // consume another source RNG draw. Direct invalid mailbox arguments
    // are rejected for the same reason as the source-shaped scan.
    let (mut economy, source_slot) = new_projectile();
    let first_rng = economy
        .source_rng
        .get(&source_slot)
        .copied()
        .expect("NewEvent source RNG");
    let first_step = economy.step_fireball_projectile_slot(source_slot, &terrain);
    assert!(first_step.impact_sound);
    let pending_after_first = economy
        .authored_bees
        .iter()
        .find(|target| target.source_entity_index == bee_slot)
        .copied()
        .expect("target Bee after first hit");
    assert_eq!(pending_after_first.pending_damage, 250);
    let hit_rng = economy
        .source_rng
        .get(&source_slot)
        .copied()
        .expect("Fireball RNG after first callback");
    assert_eq!(hit_rng, first_rng.wrapping_mul(9_377).wrapping_add(9_439));
    assert!(
        !economy
            .step_fireball_projectile_slot(source_slot, &terrain)
            .impact_sound
    );
    let pending_after_duplicate = economy
        .authored_bees
        .iter()
        .find(|target| target.source_entity_index == bee_slot)
        .copied()
        .expect("target Bee after duplicate callback");
    assert_eq!(pending_after_duplicate.pending_damage, 250);
    assert_eq!(economy.source_rng.get(&source_slot).copied(), Some(hit_rng));
    assert!(!economy.submit_authored_bee_damage(bee_slot, 1, 0));
    assert!(!economy.submit_authored_bee_damage(999, 1, session.state.entity_index));
    assert!(!economy.submit_authored_bee_damage(bee_slot, 0, session.state.entity_index));
    assert_eq!(
        economy
            .authored_bees
            .iter()
            .find(|target| target.source_entity_index == bee_slot)
            .expect("target Bee after invalid mailbox writes")
            .pending_damage,
        250
    );
}
