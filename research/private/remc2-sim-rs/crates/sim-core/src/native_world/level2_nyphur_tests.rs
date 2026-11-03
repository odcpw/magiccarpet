use super::super::tests::{level_two_campaign_session, level_two_fixture_path};
use super::*;
use crate::native_world::{
    LEVEL2_NYPHUR_ENTITY_INDEX, Mc2Simulation, POSSESSION_PROJECTILE_MODEL,
    level2_possession_guidance_target_top, projectile_guidance_angles,
};

#[test]
fn ordinary_rival_castle_upgrade_gates_use_distinct_source_carriers() {
    let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-paj2-source4-campaign-package");
    let mut campaign = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::ordinary(5).unwrap(),
    ).expect("ordinary Level 5 owner package");
    let owner = 266_u16;
    let holder_slot = 269_u16;
    for _ in 0..128 {
        if campaign.simulation().economy().castle_for_owner(owner)
            .is_some_and(|root| root.action == 4) { break; }
        campaign.step(PlayerCommand::default(), 0);
    }
    let sim = campaign.simulation();
    let live = &sim.session.state.live;
    let root = *sim.economy().castle_for_owner(owner).expect("actual owner Castle");
    assert_eq!((root.level, root.action, root.source_word_30), (1, 4, 0));
    let handle = live.economy.handle(owner).unwrap();
    let mut ai = live.nyphur_dispatcher.get(handle).unwrap().common_upkeep.clone();
    let mut entities = live.economy.entities.clone();
    let mut castle = entities[usize::from(root.source_entity_index)].clone();
    level2_nyphur_project_castle_root(&mut castle, root);
    entities[usize::from(root.source_entity_index)] = castle.clone();
    let mut slots = entities.iter().filter(|row| row.active && row.class == 3 && row.life >= 0)
        .map(|row| row.index).collect::<Vec<_>>();
    slots.extend(sim.economy().live_castles().map(|row| row.source_entity_index));
    slots.sort_unstable();
    slots.dedup();
    let site = sim.economy().castle_upgrade_site_checks(root.source_entity_index, &live.terrain, &slots, &entities);
    if site != [true, true] {
        let terrain_only = sim.economy().castle_upgrade_site_checks(root.source_entity_index, &live.terrain, &[], &entities);
        let (nx, ny) = level2_castle_shift_axes(root.level + 1);
        let (nx, ny) = (nx >> 8, ny >> 8);
        let (dx, dy) = (nx - (root.shift_pitch >> 8), ny - (root.shift_roll >> 8));
        let x = ((root.position.x.wrapping_add(128) >> 8) as u8).wrapping_sub(nx as u8);
        let y = ((root.position.y.wrapping_add(128) >> 8) as u8).wrapping_sub(ny as u8);
        let strips = [(x, y, x, 2 * nx, dy),
            (x, y.wrapping_add((2 * ny - dy) as u8), x, 2 * nx, dy),
            (x, y.wrapping_add(dy as u8), x, dx, dy),
            (x.wrapping_add((nx - dx) as u8), y.wrapping_add(dy as u8), x, dx, dy)];
        let mut first = None;
        for (strip, (first_x, start_y, reset_x, width, rows)) in strips.into_iter().enumerate() {
            for row in 0..rows {
                for col in 0..width {
                    let cell_x = (if row == 0 { first_x } else { reset_x }).wrapping_add(col as u8);
                    let cell_y = start_y.wrapping_add(row as u8);
                    let angle = live.terrain.planes.angles[terrain_index(cell_x, cell_y)] as i8;
                    if angle < 0 || live.terrain.map_type == MapType::Cave && angle & 8 != 0 {
                        first.get_or_insert((strip, cell_x, cell_y, angle));
                    }
                }
            }
        }
        eprintln!("actual owner site diagnosis: full={site:?} terrain_only={terrain_only:?} first_source_strip_failure={first:?} root={:?} stage={} shifts=({},{}) other_live_roots={:?}",
            root.position, root.level, root.shift_pitch, root.shift_roll,
            sim.economy().live_castles().filter(|other| other.source_entity_index != root.source_entity_index)
                .map(|other| (other.source_entity_index, other.position)).collect::<Vec<_>>());
    }
    assert_eq!(site, [true, true], "this actual owner Castle must admit the source terrain/overlap predicate");
    let holder = entities[usize::from(holder_slot)].clone();
    assert_eq!((holder.model, holder.source_spell_parent_entity_index()), (2, owner));
    assert_eq!(holder.active_value, 0, "the real retained holder is cold");
    ai.castle_entity_index = root.source_entity_index;
    ai.max_mana = 20_000;
    ai.mana = 20_000;
    ai.spell_enabled[2] = 0;
    let (_, profile) = Level2NyphurCommonUpkeep::source_spell_profile(2, holder.level_gate).unwrap();
    let mut source = entities[usize::from(owner)].clone();
    source.active_value = 0;
    source.set_yaw(0);
    source.set_source_roll(0);
    entities[usize::from(owner)] = source.clone();
    let mut called = false;
    assert!(ai.step_sub_13c50_with_castle_site(&source, Some(&castle), &entities, site, &mut called));
    assert!(called);
    let mut dying_castle = castle.clone();
    dying_castle.life = -1;
    called = false;
    assert!(ai.step_sub_13c50_with_castle_site(&source, Some(&dying_castle), &entities, site, &mut called),
        "source 13C50 does not add a life gate while the bound root is still action4");
    assert!(called);

    source.active_value = 1;
    called = false;
    assert!(!ai.step_sub_13c50_with_castle_site(&source, Some(&castle), &entities, site, &mut called));
    assert!(!called, "wizard W rejects before the source site probe");
    source.active_value = 0;
    ai.spell_enabled[2] = 1;
    called = false;
    assert!(!ai.step_sub_13c50_with_castle_site(&source, Some(&castle), &entities, site, &mut called));
    assert!(!called, "cooldown rejects before the source site probe");
    ai.spell_enabled[2] = 0;

    let gap = 255_i32 - i32::from(ai.word_0x244);
    let sign = gap >> 31;
    let aperture = (((gap - (sign << 2) + sign) >> 2) + 20) * 2048 / 360;
    assert!((1..=1024).contains(&aperture));
    source.set_source_roll(aperture as i16);
    called = false;
    assert!(!ai.step_sub_13c50_with_castle_site(&source, Some(&castle), &entities, site, &mut called));
    assert!(called, "the exact angular boundary is rejected after the site probe");
    source.set_source_roll((aperture - 1) as i16);
    entities[usize::from(owner)] = source.clone();
    called = false;
    assert!(ai.step_sub_13c50_with_castle_site(&source, Some(&castle), &entities, site, &mut called));

    let mut busy_holder = holder.clone();
    busy_holder.active_value = 1;
    called = false;
    assert!(!ai.source_castle_spell_available(&busy_holder, profile, &entities, site, &mut called));
    assert!(!called, "holder W rejects activation before the source site probe");
    ai.spell_enabled[2] = 1;
    called = false;
    assert!(!ai.source_castle_spell_available(&holder, profile, &entities, site, &mut called));
    assert!(!called, "activation cooldown rejects before the source site probe");
    ai.spell_enabled[2] = 0;
    ai.mana = profile.max_mana - 1;
    called = false;
    assert!(!ai.source_castle_spell_available(&holder, profile, &entities, site, &mut called));
    assert!(!called, "the current tier's table-cost gate precedes the site probe");
    assert!(holder.max_mana > profile.max_mana);
    ai.mana = holder.max_mana - 1;
    called = false;
    assert!(!ai.source_castle_spell_available(&holder, profile, &entities, site, &mut called));
    assert!(called, "the dynamic holder-cost gate follows the source site probe");
    ai.mana = holder.max_mana;
    entities[usize::from(owner)].set_source_roll(aperture as i16);
    called = false;
    assert!(!ai.source_castle_spell_available(&holder, profile, &entities, site, &mut called));
    assert!(called, "the activation angle uses the same strict boundary");
}

#[test]
fn ordinary_mission18_rival_roster_uses_each_real_owner_and_shared_rng() {
    let root = std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj")
        },
        std::path::PathBuf::from,
    );
    let campaign = crate::campaign::CampaignSession::open(
        root.join("level18"),
        crate::campaign::spec::CampaignSelection::ordinary(18).unwrap(),
    )
    .expect("actual ordinary Mission 18 package");
    let session = campaign.simulation();
    let arena = session.source_arena();
    let mut roster =
        RivalWizardRoster::from_source_reset(arena, session.players(), session.mission())
            .expect("actual Mission 18 rival rows");
    let handles: Vec<_> = roster.iter().map(|(handle, _)| handle).collect();
    assert_eq!(
        handles
            .iter()
            .map(|handle| handle.source_slot)
            .collect::<Vec<_>>(),
        [92, 107, 123, 140, 162, 184]
    );
    let mut holder_count = 0;
    for handle in &handles {
        let player = session
            .players()
            .iter()
            .find(|player| player.entity_index == handle.source_slot)
            .unwrap();
        let dispatcher = roster.get(*handle).unwrap();
        let settings =
            &session.mission().wizard_map_settings()[usize::from(player.player_color_index())];
        assert_eq!(dispatcher.common_upkeep.word_0x244, settings.perception);
        assert_eq!(dispatcher.common_upkeep.word_0x246, settings.reflexes);
        assert_eq!(
            dispatcher.common_upkeep.spell_levels,
            player.spell_unlocked_levels
        );
        assert_eq!(
            dispatcher.common_upkeep.spell_holder_indices,
            player.enabled_spell_holders
        );
        holder_count += player
            .enabled_spell_holders
            .iter()
            .filter(|slot| **slot != 0)
            .count();
        let stale = SourceEntityHandle {
            generation: handle.generation.wrapping_add(1),
            ..*handle
        };
        assert!(roster.get(stale).is_none());
        assert!(roster.with_owner_mut(stale, |_| ()).is_none());
    }
    assert_eq!(holder_count, 111);
    let first_player = session
        .players()
        .iter()
        .find(|player| player.entity_index == handles[0].source_slot)
        .unwrap();
    let holder = roster
        .bound_live_holder(handles[0], first_player, arena, 0)
        .unwrap();
    let mut reused_arena = arena.clone();
    reused_arena
        .economy
        .slot_generations
        .insert(holder.source_slot, holder.generation.wrapping_add(1));
    assert!(
        roster
            .bound_live_holder(handles[0], first_player, &reused_arena, 0)
            .is_none()
    );
    assert!(roster.sync_owner_inventory(handles[0], first_player, &reused_arena));
    assert_eq!(
        roster
            .get(handles[0])
            .unwrap()
            .common_upkeep
            .spell_holder_indices[0],
        0
    );
    let mut source_rng = 1;
    for handle in &handles {
        let actual = roster
            .with_owner_mut(*handle, |dispatcher| {
                dispatcher.common_upkeep.step_watcomrand()
            })
            .unwrap();
        assert_eq!(actual, level2_nyphur_watcomrand(&mut source_rng));
    }
    let first = handles[0];
    let second = handles[1];
    roster
        .with_owner_mut(first, |dispatcher| dispatcher.state_byte = 7)
        .unwrap();
    assert_eq!(roster.get(first).unwrap().state_byte, 7);
    assert_eq!(roster.get(second).unwrap().state_byte, 0);
    assert_eq!(
        roster
            .get(handles[3])
            .unwrap()
            .common_upkeep
            .cadence_period(),
        3
    );
}

#[test]
fn ordinary_mission18_schedules_every_imported_rival_through_dispatch_and_tail() {
    let root = std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj")
        },
        std::path::PathBuf::from,
    );
    let mut session = crate::campaign::CampaignSession::open(
        root.join("level18"),
        crate::campaign::spec::CampaignSelection::ordinary(18).unwrap(),
    )
    .expect("actual ordinary Mission 18 package")
    .simulation()
    .clone();
    session.step_level2_source_pass(0);
    let owners = [92, 107, 123, 140, 162, 184];
    for owner in owners {
        assert!(
            session.dispatch_events().iter().all(|event| !matches!(
                event,
                Mc2DispatchEvent::Error { source_slot, .. } if *source_slot == owner
            )),
            "ordinary rival {owner} dispatch must succeed"
        );
        let trace = session
            .nyphur_dispatch_traces()
            .iter()
            .find(|trace| trace.source_slot == owner)
            .unwrap_or_else(|| panic!("ordinary rival {owner} was not dispatched"));
        assert_eq!(
            trace.tail_call_count(),
            if trace.state_byte == 0 { 2 } else { 1 },
            "ordinary rival {owner} reaches the source tail in its mode's source order"
        );
    }

    // Use the actual first rival and its imported Duel holder. Invoke the
    // real AI activation helper with controlled resources, then let the
    // ordinary source cursor deliver the holder callback. This does not
    // claim that the AI autonomously selected Duel in this fixture.
    let caster = 92;
    let holder = 106;
    {
        let live = &mut session.session.state.live;
        let player = live.core.players.iter()
            .find(|player| player.entity_index == caster).unwrap();
        assert_eq!(player.enabled_spell_holders[14], holder);
        let raw_holder = &live.economy.entities[usize::from(holder)];
        assert_eq!((raw_holder.class, raw_holder.model, raw_holder.action), (15, 14, 42));
        assert_eq!(raw_holder.source_spell_parent_entity_index(), caster);
        let castle = if let Some(root) = live.economy.castle_for_owner(caster) {
            root.source_entity_index
        } else {
            live.economy.economy.build_castle_at(
                Position::default(), Position::default(), caster, false, true,
            ).expect("ordinary rival Castle constructor")
        };
        assert!(live.economy.economy.set_castle_mana_prerequisite_for_test(castle, 20_000));
        let caster_row = &mut live.economy.entities[usize::from(caster)];
        caster_row.max_mana = 20_000;
        caster_row.record[144..148].copy_from_slice(&20_000_i32.to_le_bytes());
        caster_row.set_mana(20_000);
        let caster_model = caster_row.model;
        let mut flags = caster_row.control_flags;
        let handle = live.economy.handle(caster).unwrap();
        let rival = live.nyphur_dispatcher.get_mut(handle).unwrap();
        rival.common_upkeep.max_mana = 20_000;
        rival.common_upkeep.mana = 20_000;
        rival.common_upkeep.spell_enabled[14] = 0;
        assert!(rival.common_upkeep.step_sub_14e10(
            14, caster_model, &mut flags, &mut live.economy.entities,
        ), "actual rival AI activation accepts the Duel holder");
        live.economy.entities[usize::from(caster)].set_control_flags(flags);
        let source = &live.economy.entities[usize::from(holder)];
        assert_eq!(source.active_value, source.activation_value as i16);
    }
    session.step_level2_source_pass(0);
    let live = &session.session.state.live;
    assert!(session.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
        "ordinary rival holder dispatch failed: {:?}", session.dispatch_events());
    assert!(live.economy.entities.iter().any(|row| {
        row.active && (row.class, row.model, row.action) == (9, 7, 7)
            && row.source_id == caster
            && u16::from_le_bytes([row.record[42], row.record[43]]) == holder
    }), "the scheduled real rival holder creates its Duel flyer");
}

#[test]
fn ordinary_mission23_dying_rival_tether_uses_its_own_target_and_holder() {
    let package = std::env::var_os("MC_LEVEL23_PACKAGE").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-copper-house27-v3-oucmzgsa/level23"),
        std::path::PathBuf::from,
    );
    let mut session = crate::campaign::CampaignSession::open(
        package, crate::campaign::spec::CampaignSelection::ordinary(23).unwrap(),
    ).expect("actual ordinary Mission 23 package").simulation().clone();
    let mut profile = session.session.definition.profile;
    let parameter_index = session.session.state.live.economy.entities[425]
        .parameter_relation.expect("actual rival collision parameters");
    let parameter = &session.session.definition.presentation.entity_parameters[usize::from(parameter_index)];
    let word = |offset: usize| i16::from_le_bytes([parameter[offset], parameter[offset + 1]]);
    profile.terrain_reference_height = word(10);
    profile.ground_clearance = word(12);
    profile.passive_vertical_step = word(14);
    profile.raw_word_0x10 = word(16);
    profile.raw_word_0x12 = word(18);
    let live = &mut session.session.state.live;
    let caster = 425_u16;
    let target = 451_u16;
    let caster_handle = live.economy.handle(caster).unwrap();
    let target_handle = live.economy.handle(target).unwrap();
    let holder = live.core.players.iter().find(|player| player.entity_index == caster)
        .unwrap().enabled_spell_holders[14];
    assert_eq!(holder, 440);
    let caster_position = live.economy.entities[usize::from(caster)].position;
    let target_position = Position { x: caster_position.x.wrapping_add(1_024), ..caster_position };
    live.economy.entities[usize::from(target)].set_position(target_position);
    live.economy.entities[usize::from(target)].set_life(5_000);
    live.economy.entities[usize::from(target)].set_mana(500);
    live.economy.entities[usize::from(holder)].active_value = 1;
    live.economy.entities[usize::from(holder)].record[50..52].copy_from_slice(&1_i16.to_le_bytes());
    let mana_regen = live.economy.entities[usize::from(target)].mana_regen;
    let life_regen = live.nyphur_dispatcher.get(target_handle).unwrap()
        .common_upkeep(live.economy.entities[usize::from(target)].phase).life_regen;
    assert!(live.nyphur_dispatcher.get_mut(caster_handle).unwrap().install_duel_tether(
        crate::spells::directional::DuelTetherBinding {
            caster_entity_index: caster, target_entity_index: target, holder_entity_index: holder,
            tier: 2, prior_distance: 1_024,
        },
    ));
    let target_binding_before = live.nyphur_dispatcher.get(target_handle).unwrap().duel_tether();
    // Controlled binding on actual owner data exercises the tether kernel.
    live.nyphur_dispatcher.step_dying_tether(
        &mut live.economy, &mut live.shared_spells, live.core.context.spell_fire.as_mut(),
        &mut live.player_life, &live.core.players, live.core.state.entity_index, caster, caster_position,
    ).expect("real rival tether carriers resolve");
    assert_eq!(live.economy.entities[usize::from(target)].life, 5_000 - i32::from(life_regen) - 2);
    assert_eq!(live.economy.entities[usize::from(target)].mana, (500 - mana_regen - 8).max(0));
    assert_eq!(live.nyphur_dispatcher.get(target_handle).unwrap().duel_tether(), target_binding_before);
    assert!(live.nyphur_dispatcher.get(caster_handle).unwrap().has_duel_target());
    live.economy.entities[usize::from(holder)].active_value = 0;
    live.nyphur_dispatcher.step_dying_tether(
        &mut live.economy, &mut live.shared_spells, live.core.context.spell_fire.as_mut(),
        &mut live.player_life, &live.core.players, live.core.state.entity_index, caster, caster_position,
    ).unwrap();
    assert!(!live.nyphur_dispatcher.get(caster_handle).unwrap().has_duel_target());
    assert_eq!(live.nyphur_dispatcher.get(target_handle).unwrap().duel_tether(), target_binding_before);

    // The same source tether can hit the real local player. Its damage must
    // reach the session life consumer as well as the numeric actor row.
    let local = live.core.state.entity_index;
    let local_regen = live.shared_spells.player_life_regen as i16;
    let local_mana_regen = live.economy.entities[usize::from(local)].mana_regen;
    let row = &mut live.economy.entities[usize::from(local)];
    row.set_position(target_position);
    row.set_life(5_000);
    row.set_mana(500);
    live.shared_spells.sync_player_entity_from_authority(row);
    live.player_life = 5_000;
    live.economy.entities[usize::from(holder)].active_value = 1;
    assert!(live.nyphur_dispatcher.get_mut(caster_handle).unwrap().install_duel_tether(
        crate::spells::directional::DuelTetherBinding {
            caster_entity_index: caster, target_entity_index: local, holder_entity_index: holder,
            tier: 2, prior_distance: 1_024,
        },
    ));
    live.nyphur_dispatcher.step_dying_tether(
        &mut live.economy, &mut live.shared_spells, live.core.context.spell_fire.as_mut(),
        &mut live.player_life, &live.core.players, local, caster, caster_position,
    ).unwrap();
    let expected_life = 5_000 - i32::from(local_regen) - 2;
    let expected_mana = (500 - local_mana_regen - 8).max(0);
    assert_eq!(live.economy.entities[usize::from(local)].life, expected_life);
    assert_eq!(live.shared_spells.player_entity.life, expected_life);
    assert_eq!(live.player_life, expected_life);
    assert_eq!(live.shared_spells.player_mana, expected_mana);
    assert_eq!(live.shared_spells.caster.mana, expected_mana);
    if let Some(fire) = live.core.context.spell_fire.as_ref() {
        assert_eq!(fire.caster.mana, expected_mana);
    }

    // The production dying-flight body must also publish the same rival
    // target effects, rather than only the isolated tether kernel above.
    let row = &mut live.economy.entities[usize::from(target)];
    row.set_position(target_position);
    row.set_life(5_000);
    row.set_mana(500);
    live.economy.entities[usize::from(holder)].active_value = 1;
    assert!(live.nyphur_dispatcher.get_mut(caster_handle).unwrap().install_duel_tether(
        crate::spells::directional::DuelTetherBinding {
            caster_entity_index: caster, target_entity_index: target, holder_entity_index: holder,
            tier: 2, prior_distance: 1_024,
        },
    ));
    live.nyphur_dispatcher.step_dying_flight(
        &mut live.economy, &mut live.shared_spells, &mut live.core.players,
        &mut live.player_life, local, caster, &live.terrain, profile, &mut live.core.context,
    ).expect("real rival flight resolves its owner-bound tether");
    assert_eq!(live.economy.entities[usize::from(target)].life, 5_000 - i32::from(life_regen) - 2);
    assert_eq!(live.economy.entities[usize::from(target)].mana, (500 - mana_regen - 8).max(0));
}

#[test]
fn ordinary_mission23_dying_rival_moves_before_falling() {
    let package = std::env::var_os("MC_LEVEL23_PACKAGE").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-copper-house27-v3-oucmzgsa/level23"),
        std::path::PathBuf::from,
    );
    let mut session = crate::campaign::CampaignSession::open(
        package, crate::campaign::spec::CampaignSelection::ordinary(23).unwrap(),
    ).expect("actual ordinary Mission 23 package").simulation().clone();
    let owner = 425_u16;
    let mut expected;
    {
        let live = &mut session.session.state.live;
        let handle = live.economy.handle(owner).unwrap();
        let actor = &mut live.economy.entities[usize::from(owner)];
        let mut position = actor.position;
        position.z = live.terrain.ground_height(position).wrapping_add(2_000);
        actor.set_position(position);
        actor.set_life(-1);
        actor.set_source_action(2);
        actor.set_source_word_2c(0);
        actor.set_yaw(0);
        actor.set_pitch(0);
        actor.set_actual_speed(128);
        actor.set_control_flags(actor.control_flags & !0x800);
        let player = live.core.players.iter_mut().find(|player| player.entity_index == owner).unwrap();
        player.record[16..20].fill(0);
        player.record[38..40].fill(0);
        player.record[44..50].fill(0);
        player.record[54..60].fill(0);
        let common = &mut live.nyphur_dispatcher.get_mut(handle).unwrap().common_upkeep;
        common.speed = 128;
        common.strafe_speed = 0;
        common.move_boost = 0;
        common.move_speed = 0;
        expected = position;
        crate::move_entity(&mut expected, 0, 0, 128);
        crate::world::effects::sync_position(&mut live.economy, &mut live.terrain, owner, position, true, false).unwrap();
    }
    session.step_level2_source_pass(0);
    let actor = &session.session.state.live.economy.entities[usize::from(owner)];
    assert_eq!(actor.action, 2);
    assert_eq!((actor.position.x, actor.position.y), (expected.x, expected.y),
        "the scheduled source flight body moves this rival before its fall");
}

#[test]
fn ordinary_mission23_rival_lethal_contact_selects_dying_action_on_same_visit() {
    let package = std::env::var_os("MC_LEVEL23_PACKAGE").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-copper-house27-v3-oucmzgsa/level23"),
        std::path::PathBuf::from,
    );
    let mut session = crate::campaign::CampaignSession::open(
        package, crate::campaign::spec::CampaignSelection::ordinary(23).unwrap(),
    ).expect("actual ordinary Mission 23 package").simulation().clone();
    let owner = 425_u16;
    let handle = session.session.state.live.economy.handle(owner).unwrap();
    let attacker = session.session.state.live.core.state.entity_index;
    {
        let live = &mut session.session.state.live;
        let mut position = live.economy.entities[usize::from(owner)].position;
        // Move the actual actor away from its Castle; protection is allowed
        // to expire through normal source visits, never bypassed in the test.
        position.x = position.x.wrapping_add(0x5000);
        crate::world::effects::sync_position(
            &mut live.economy, &mut live.terrain, owner, position, true, false,
        ).unwrap();
        live.economy.entities[usize::from(owner)].set_position(position);
    }
    let mut admitted = false;
    for _ in 0..256 {
        session.step_level2_source_pass(0);
        let live = &session.session.state.live;
        assert_eq!(live.economy.handle(owner), Some(handle));
        assert!(live.economy.entities[usize::from(owner)].life >= 0);
        assert!(session.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
        let common = &live.nyphur_dispatcher.get(handle).unwrap().common_upkeep;
        if common.word_0x159 == 0 && !common.high_upkeep {
            admitted = true;
            break;
        }
    }
    assert!(admitted, "ordinary source visits expire the rival's protection away from its Castle");
    let (victim_color, score_before) = {
        let live = &session.session.state.live;
        let victim = live.core.players.iter().find(|player| player.entity_index == owner).unwrap();
        let killer = live.core.players.iter().find(|player| player.entity_index == attacker).unwrap();
        let color = usize::from(victim.player_color_index());
        (color, killer.source_kill_scores().expect("actual killer source score matrix")[color])
    };
    {
        let live = &mut session.session.state.live;
        // This is the real shared contact producer after its target was
        // selected; no projectile flight or natural aim is claimed here.
        crate::native_world::area_damage::contact(
            &mut live.economy, None, owner, u16::MAX, attacker,
        ).expect("real source contact writes the rival's damage mailbox");
        assert_eq!(live.economy.entities[usize::from(owner)].source_upkeep_word_62(), attacker);
    }
    session.step_level2_source_pass(0);
    let live = &session.session.state.live;
    let actor = &live.economy.entities[usize::from(owner)];
    assert_eq!(live.economy.handle(owner), Some(handle));
    assert!(actor.life < 0, "lethal damage reaches the numeric source life on the consuming visit");
    assert_eq!(actor.action, 2, "sub_12A70 selects the dying callback on the same visit");
    assert_eq!(u16::from_le_bytes([actor.record[40], actor.record[41]]), attacker);
    {
        // Bound the fall after the real lethal contact. Death, the shortened
        // banishment countdown and Castle revival remain scheduler visits.
        let live = &mut session.session.state.live;
        let actor = &live.economy.entities[usize::from(owner)];
        let parameter = actor.parameter_relation.expect("rival collision parameter");
        let offset = &session.session.definition.presentation.entity_parameters[usize::from(parameter)][12..14];
        let mut position = actor.position;
        position.z = live.terrain.ground_height(position)
            .wrapping_add(i16::from_le_bytes([offset[0], offset[1]]));
        live.economy.entities[usize::from(owner)].set_position(position);
        live.economy.entities[usize::from(owner)].set_source_word_2c(0);
        crate::world::effects::sync_position(
            &mut live.economy, &mut live.terrain, owner, position, true, false,
        ).unwrap();
    }
    session.step_level2_source_pass(0);
    assert_eq!(session.session.state.live.economy.entities[usize::from(owner)].action, 3);
    {
        let live = &session.session.state.live;
        let killer = live.core.players.iter().find(|player| player.entity_index == attacker).unwrap();
        let shared_killer = live.shared_spells.players().iter().find(|player| player.entity_index == attacker).unwrap();
        let expected = score_before.wrapping_add(1);
        assert_eq!(killer.source_kill_scores().unwrap()[victim_color], expected);
        assert_eq!(shared_killer.source_kill_scores().unwrap()[victim_color], expected);
    }
    session.session.state.live.economy.entities[usize::from(owner)].set_source_dword_10(1);
    session.session.state.live.nyphur_dispatcher.get_mut(handle).unwrap()
        .common_upkeep.move_boost = 37;
    session.step_level2_source_pass(0);
    assert_eq!(
        session.session.state.live.nyphur_dispatcher.get(handle).unwrap()
            .common_upkeep.move_boost,
        0,
        "the banished visit clears this owner's retained recoil before revival",
    );
    session.step_level2_source_pass(0);
    let live = &session.session.state.live;
    assert!(session.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
    assert_eq!(live.economy.handle(owner), Some(handle));
    assert_eq!(live.economy.entities[usize::from(owner)].action, 1);
    assert_eq!(live.economy.entities[usize::from(owner)].life, 10_000);
    let player = live.core.players.iter().find(|player| player.entity_index == owner).unwrap();
    assert!(live.nyphur_dispatcher.bound_live_holder(handle, player, &live.economy, 0).is_some());
}

#[test]
fn ordinary_mission23_rival_grounded_death_rebuilds_its_own_castle_holders() {
    let package = std::env::var_os("MC_LEVEL23_PACKAGE").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-copper-house27-v3-oucmzgsa/level23"),
        std::path::PathBuf::from,
    );
    let mut session = crate::campaign::CampaignSession::open(
        package, crate::campaign::spec::CampaignSelection::ordinary(23).unwrap(),
    ).expect("actual ordinary Mission 23 package").simulation().clone();
    let owner = 425_u16;
    let other = 451_u16;
    let handle = session.session.state.live.economy.handle(owner).unwrap();
    let before = session.players().iter().find(|player| player.entity_index == owner).unwrap().enabled_spell_holders;
    let other_before = session.players().iter().find(|player| player.entity_index == other).unwrap().enabled_spell_holders;
    assert_eq!(session.economy().castle_for_owner(owner).unwrap().source_entity_index, 450);
    {
        // Controlled entry into the real grounded dying callback; this test
        // verifies death→banishment→revival, not the preceding lethal hit.
        let live = &mut session.session.state.live;
        let row = &live.economy.entities[usize::from(owner)];
        let parameter = row.parameter_relation.expect("actual rival collision parameters");
        let offset = &session.session.definition.presentation.entity_parameters[usize::from(parameter)][12..14];
        let mut position = row.position;
        position.z = live.terrain.ground_height(position).wrapping_add(i16::from_le_bytes([offset[0], offset[1]]));
        let row = &mut live.economy.entities[usize::from(owner)];
        row.set_position(position);
        row.set_life(-1);
        row.set_source_action(2);
        row.set_source_word_2c(0);
        crate::world::effects::sync_position(&mut live.economy, &mut live.terrain, owner, position, true, false).unwrap();
    }
    session.step_level2_source_pass(0);
    // The real grounded-death callback just released this rival's held
    // Invisible spell with a finite source lifetime. On a clone, move the
    // existing eligible human to it and let normal source collision acquire
    // the same generation before expiry. The original continues its revival
    // checks below. Entry into the rival's dying action was controlled above;
    // this does not claim a public lethal-hit producer.
    let dropped_slot = before[11];
    assert_eq!(dropped_slot, 437);
    let dropped_handle = session.source_arena().handle(dropped_slot).unwrap();
    let dropped = session.entities()[usize::from(dropped_slot)].clone();
    assert_eq!((dropped.class, dropped.model, dropped.action), (15, 11, 34));
    assert!(dropped.life > 1 && dropped.life <= 289);
    let local = session.state.entity_index;
    assert_eq!(local, 424);
    assert_eq!(
        session.shared_spells().local_player().enabled_spell_holders[11],
        0
    );
    let local_handle = session.source_arena().handle(local).unwrap();
    let mut collected = session.clone();
    let mut acquired = false;
    for _ in 0..8 {
        let position = collected.entities()[usize::from(dropped_slot)].position;
        collected
            .set_level2_entity_position(local, position)
            .unwrap();
        collected.step_outer_turn(crate::PlayerCommand::default(), 0);
        if collected.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::SpellAcquisition { source_slot, family: 11, result: Some(outcome), .. }
                if *source_slot == dropped_slot && outcome.parent_entity_index == local
        )) {
            acquired = true;
            break;
        }
    }
    assert!(
        acquired,
        "the real local player must collect the positive-life rival drop"
    );
    assert_eq!(
        collected.source_arena().handle(dropped_slot),
        Some(dropped_handle)
    );
    assert_eq!(collected.source_arena().handle(local), Some(local_handle));
    let held = collected.shared_spells().pickup(dropped_slot).unwrap();
    assert_eq!((held.class, held.model, held.action), (15, 11, 33));
    assert_eq!(held.source_spell_parent_entity_index(), local);
    assert!(
        held.life > 0 && held.life < dropped.life,
        "source life decrements before the actual collision"
    );
    assert_eq!(
        collected
            .shared_spells()
            .local_player()
            .enabled_spell_holders[11],
        dropped_slot
    );
    {
        let live = &mut session.session.state.live;
        assert_eq!(live.economy.entities[usize::from(owner)].action, 3);
        assert_eq!(live.economy.handle(owner), Some(handle));
        assert_eq!(live.economy.entities[usize::from(owner)].source_dword_10(), 1_200);
        assert!(live.nyphur_dispatcher.get_mut(handle).unwrap().install_duel_tether(
            crate::spells::directional::DuelTetherBinding {
                caster_entity_index: owner, target_entity_index: other,
                holder_entity_index: before[14], tier: 0, prior_distance: 1_024,
            },
        ));
        // Bound the test after the actual death callback; source countdown
        // and the timer-zero revival still run through normal scheduling.
        live.economy.entities[usize::from(owner)].set_source_dword_10(1);
    }
    session.step_level2_source_pass(0);
    assert_eq!(session.session.state.live.economy.entities[usize::from(owner)].source_dword_10(), 0);
    let peer_roll = {
        let live = &mut session.session.state.live;
        let shared_roll = live.shared_spells.players().iter()
            .find(|player| player.entity_index == other).unwrap().roll_0x155();
        // A live wind write is carried by core.players; make that state
        // observably newer than the shared spell projection before revival.
        let marker = if shared_roll == 512 { 513 } else { 512 };
        live.core.players.iter_mut().find(|player| player.entity_index == other)
            .unwrap().set_roll_0x155(marker);
        marker
    };
    session.step_level2_source_pass(0);
    let live = &session.session.state.live;
    let actor = &live.economy.entities[usize::from(owner)];
    assert_eq!(live.economy.handle(owner), Some(handle));
    assert_eq!((actor.action, actor.life, actor.max_life), (1, 10_000, 10_000));
    assert!(!live.nyphur_dispatcher.get(handle).unwrap().has_duel_target(), "source revival clears the old caster target");
    let player = live.core.players.iter().find(|player| player.entity_index == owner).unwrap();
    for (family, old_slot) in before.into_iter().enumerate() {
        if old_slot != 0 {
            let holder = live.nyphur_dispatcher.bound_live_holder(handle, player, &live.economy, family)
                .expect("revival constructed and bound the actual owner's holder");
            assert_ne!(holder.source_slot, old_slot, "released pickup remains a separate source row");
        }
    }
    let other_player = live.core.players.iter().find(|player| player.entity_index == other).unwrap();
    assert_eq!(other_player.roll_0x155(), peer_roll, "revival must preserve the other rival's live movement state");
    assert_eq!(other_player.enabled_spell_holders[0], other_before[0]);
    let relation = 4 + 4 * usize::from(player.player_color_index());
    assert_eq!(
        live.shared_spells.local_player().type_str_164_auxiliary().unwrap()
            .array_0x1FC_508[relation],
        (-24_609_i16).cast_unsigned(),
        "the local player's hostility projection observes the revived rival",
    );
    let other_handle = live.economy.handle(other).unwrap();
    assert_eq!(live.nyphur_dispatcher.bound_live_holder(other_handle, other_player, &live.economy, 0).unwrap().source_slot, other_before[0]);
}

#[test]
#[ignore = "requires a fresh Mission 23 source export with the v5 player arena"]
fn ordinary_mission23_rival_reset_carries_captured_source_padding() {
    let package = std::path::PathBuf::from(
        std::env::var_os("MC_LEVEL23_PADDING_PACKAGE")
            .expect("MC_LEVEL23_PADDING_PACKAGE must name the freshly exported Mission 23 package"),
    );
    let source = std::fs::read(package.join("players.bin")).expect("captured player rows");
    assert_eq!(&source[..8], &[b'M', b'C', b'P', b'1', 5, 0, 157, 2]);
    let mut session = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::ordinary(23).unwrap(),
    ).expect("actual Mission 23 package with v5 player arena").simulation().clone();
    let live = &session.session.state.live;
    for (slot, player) in live.core.players.iter().enumerate() {
        let offset = 12 + slot * 669 + 663;
        let bytes = &source[offset..offset + 6];
        for index in 0..3 {
            let word = u16::from_le_bytes([bytes[2 * index], bytes[2 * index + 1]]);
            assert_eq!(player.type_str_164_array_word(32 + index), Some(word), "player slot {slot}");
        }
        if player.is_ai {
            let handle = live.economy.handle(player.entity_index).expect("rival generation");
            let rival = live.nyphur_dispatcher.get(handle).expect("rival roster member");
            assert_eq!(&rival.common_upkeep.padding_0x23c, bytes, "rival slot {slot}");
            for index in 0..32 {
                let word_offset = 12 + slot * 669 + 596 + 2 * index;
                let expected = u16::from_le_bytes([source[word_offset], source[word_offset + 1]]);
                assert_eq!(rival.common_upkeep.array_0x1fc[index], expected, "rival slot {slot}, word {index}");
            }
        }
    }
    // Exercise an actual source revival after the initial capture: the
    // color-7 threshold write must update each live player row as well as the
    // rival's own carrier.
    let owner = 585_u16;
    let live = &mut session.session.state.live;
    let handle = live.economy.handle(owner).expect("color-7 generation");
    let row = &live.economy.entities[usize::from(owner)];
    let parameter = row.parameter_relation.expect("rival collision parameters");
    let clearance = &session.session.definition.presentation.entity_parameters[usize::from(parameter)][12..14];
    let mut position = row.position;
    position.z = live.terrain.ground_height(position).wrapping_add(i16::from_le_bytes([clearance[0], clearance[1]]));
    let row = &mut live.economy.entities[usize::from(owner)];
    row.set_position(position);
    row.set_life(-1);
    row.set_source_action(2);
    row.set_source_word_2c(0);
    crate::world::effects::sync_position(&mut live.economy, &mut live.terrain, owner, position, true, false).unwrap();
    session.step_level2_source_pass(0);
    session.session.state.live.economy.entities[usize::from(owner)].set_source_dword_10(1);
    session.step_level2_source_pass(0);
    session.step_level2_source_pass(0);
    let live = &session.session.state.live;
    assert_eq!(live.economy.handle(owner), Some(handle));
    assert_eq!(live.economy.entities[usize::from(owner)].action, 1);
    for player in &live.core.players {
        let expected = if player.entity_index == owner { 24_607 } else { (-24_609_i16).cast_unsigned() };
        assert_eq!(player.type_str_164_array_word(32), Some(expected));
        assert_eq!(&player.record_bytes()[663..665], &expected.to_le_bytes());
    }
}

#[test]
fn ordinary_mission23_color7_revival_publishes_source_padding_word() {
    let package = std::env::var_os("MC_LEVEL23_PACKAGE").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-copper-house27-v3-oucmzgsa/level23"),
        std::path::PathBuf::from,
    );
    let mut session = crate::campaign::CampaignSession::open(
        package, crate::campaign::spec::CampaignSelection::ordinary(23).unwrap(),
    ).expect("actual ordinary Mission 23 package").simulation().clone();
    let owner = 585_u16;
    let other = 425_u16;
    let handle = session.session.state.live.economy.handle(owner).unwrap();
    let local = session.session.state.live.core.state.entity_index;
    {
        let live = &mut session.session.state.live;
        let player = live.core.players.iter().find(|player| player.entity_index == owner).unwrap();
        assert_eq!(player.player_color_index(), 7);
        assert_eq!(live.core.players.iter().find(|player| player.entity_index == local).unwrap()
            .type_str_164_array_word(32), None, "v5 omits the source padding word");
        assert!(live.economy.castle_for_owner(owner).is_some());
        let row = &live.economy.entities[usize::from(owner)];
        let parameter = row.parameter_relation.expect("actual rival collision parameters");
        let offset = &session.session.definition.presentation.entity_parameters[usize::from(parameter)][12..14];
        let mut position = row.position;
        position.z = live.terrain.ground_height(position).wrapping_add(i16::from_le_bytes([offset[0], offset[1]]));
        let row = &mut live.economy.entities[usize::from(owner)];
        row.set_position(position);
        row.set_life(-1);
        row.set_source_action(2);
        row.set_source_word_2c(0);
        crate::world::effects::sync_position(&mut live.economy, &mut live.terrain, owner, position, true, false).unwrap();
    }
    session.step_level2_source_pass(0);
    assert_eq!(session.session.state.live.economy.entities[usize::from(owner)].action, 3);
    session.session.state.live.economy.entities[usize::from(owner)].set_source_dword_10(1);
    session.step_level2_source_pass(0);
    session.step_level2_source_pass(0);
    let live = &session.session.state.live;
    assert_eq!(live.economy.handle(owner), Some(handle));
    assert_eq!(live.economy.entities[usize::from(owner)].action, 1);
    let player = |index| live.core.players.iter().find(|player| player.entity_index == index).unwrap();
    assert_eq!(player(owner).type_str_164_array_word(32), Some(24_607));
    assert_eq!(player(other).type_str_164_array_word(32), Some((-24_609_i16).cast_unsigned()));
    assert_eq!(player(local).type_str_164_array_word(32), Some((-24_609_i16).cast_unsigned()));
    assert_eq!(live.shared_spells.local_player().type_str_164_array_word(32), Some((-24_609_i16).cast_unsigned()));
    assert_eq!(player(local).type_str_164_array_word(33), None, "an unwritten source padding word stays unknown");
}

// Earlier fixtures assumed the first cadence always selected a creature.
// Ordinary Goat deaths now publish mana spheres, which sub_13CE0 correctly
// selects before sub_14530. Wait for the real class-5 relation without
// changing actor state or selector priority. This bounded prelude is separate
// from the existing 300-turn family-0 pursuit/cast window below.
fn advance_to_class5_target(session: &mut Mc2Simulation) -> (usize, usize) {
    for turn in 1..=4096 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().all(|event| !matches!(
            event,
            Mc2DispatchEvent::Error {
                source_slot: 112,
                ..
            }
        )));
        if session.nyphur_dispatch_state_byte() != 13 {
            continue;
        }
        let source = &session.economy.entities[112];
        let target_index = usize::from(source.source_word_96());
        let target = &session.economy.entities[target_index];
        assert_eq!(target.class, 5);
        assert!(matches!(target.model, 0 | 1));
        assert!(level2_nyphur_source_identity_matches(source, target));
        eprintln!("ordinary class-5 selection after {turn} turns: target {target_index}");
        return (turn, target_index);
    }
    panic!("ordinary Nyphur did not finish its higher-priority mana pursuit within 4096 turns");
}

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
    relation_tampered.session.state.live.economy.entities[113].record[26..28]
        .copy_from_slice(&114_u16.to_le_bytes());
    assert!(relation_tampered.nyphur_participant().is_err());

    let mut holder_owner_tampered = inspection
        .level2_reset_session()
        .expect("Nyphur ownership negative session");
    holder_owner_tampered.players[1].enabled_spell_holders[0] = 114;
    holder_owner_tampered.players[1].record[268..270].copy_from_slice(&114_u16.to_le_bytes());
    assert!(holder_owner_tampered.nyphur_participant().is_err());
}

#[test]
fn level_two_sub_159e0_accounts_an_ordinary_possession_relation_once() {
    let mut session = level_two_campaign_session();
    session.select_level2_campaign_entry_stage();
    let baseline_hash = session.campaign_state_hash();

    let row11_position = session
        .mission()
        .authored_features()
        .iter()
        .copied()
        .find(|feature| feature.index == 11)
        .expect("qualified group-1 producer row")
        .world_position(0);
    session
        .set_level2_entity_position(session.state.entity_index, row11_position)
        .expect("public player input reaches the group-1 producer");
    session.step_outer_turn(PlayerCommand::default(), 0);

    let aim_reference_slot = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.active && !sphere.pending_delete)
        .map(|sphere| sphere.source_entity_index)
        .expect("entry-stage package materializes model-39 mana");
    let target_position = {
        let mut economy = session.economy_mut();
        let target = economy
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == aim_reference_slot)
            .expect("selected package sphere remains live");
        level2_possession_guidance_target_top(target)
    };
    let mut player_position = target_position;
    // `sub_68490` rejects guidance candidates beyond 5,120 source units.
    // Stay comfortably inside that cone while remaining well outside the
    // contact footprint, so acquisition and impact are separate visits.
    player_position.x = player_position.x.wrapping_sub(4_096);
    player_position.z = session.canonical_terrain().ground_height(player_position);
    session.state.position = player_position;
    session
        .set_level2_entity_position(session.state.entity_index, player_position)
        .expect("local player input position remains map-linked");
    let mut launch_position = player_position;
    crate::move_entity(
        &mut launch_position,
        session.state.yaw.wrapping_sub(512),
        0,
        256,
    );
    if session.canonical_terrain().ground_height(launch_position) > launch_position.z {
        launch_position = player_position;
    }
    launch_position.z = launch_position.z.wrapping_add(session.state.entity_fov);
    let (launch_yaw, launch_pitch) = projectile_guidance_angles(launch_position, target_position);

    // The qualified local player owns family-1 Possession. Its ordinary
    // player -> holder -> class-9/model-1 path acquires a real model-39 row
    // during the same numeric source scan.
    let request = PlayerCommand {
        opcode: 6,
        argument_1: 1,
        flags: 0x40,
        argument_6: launch_yaw.wrapping_sub(session.state.yaw).cast_unsigned(),
        argument_8: launch_pitch
            .wrapping_sub(session.state.display_pitch)
            .cast_unsigned(),
        ..PlayerCommand::default()
    };
    session.step_outer_turn(request, 0);
    let mut projectile = session
        .economy()
        .possession_projectiles()
        .iter()
        .find(|projectile| {
            projectile.active
                && projectile.source_id_entity_index == session.state.entity_index
                && projectile.source_model == POSSESSION_PROJECTILE_MODEL
        })
        .copied()
        .unwrap_or_else(|| {
            panic!(
                "ordinary Possession flyer did not acquire a package target: {:?}",
                session.economy().possession_projectiles()
            )
        });
    if projectile.target_source_entity_index == 0 {
        assert!(
            projectile.source_entity_index < 116,
            "an earlier-slot birth is intentionally invisible until the next source scan"
        );
        session.step_outer_turn(PlayerCommand::default(), 0);
        projectile = session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|candidate| candidate.source_entity_index == projectile.source_entity_index)
            .copied()
            .expect("following scan visits the earlier-slot Possession flyer");
    }
    assert_ne!(
        projectile.target_source_entity_index, 0,
        "ordinary guidance must install a real model-39 relation: {projectile:?}"
    );
    assert!(
        session.economy().spheres().iter().any(|sphere| {
            sphere.active && sphere.source_entity_index == projectile.target_source_entity_index
        }),
        "the installed relation must name a live package sphere"
    );
    assert!(!projectile.threat_accounted);

    // The existing long authored-route test proves Nyphur can establish this
    // sphere ownership ordinarily; this focused test isolates the new prepass
    // rather than repeating that multi-minute pursuit.
    let mut economy = session.economy_mut();
    let target = economy
        .spheres
        .iter_mut()
        .find(|sphere| sphere.source_entity_index == projectile.target_source_entity_index)
        .expect("acquired target remains a live mana sphere");
    target.owner_entity_index = LEVEL2_NYPHUR_ENTITY_INDEX;
    target.player_entity_index = LEVEL2_NYPHUR_ENTITY_INDEX;
    target.mana = 2_048;

    let attacker_color = session
        .session
        .state
        .live
        .core
        .players
        .iter()
        .find(|player| player.entity_index == session.state.entity_index)
        .expect("local player source row")
        .player_color_index();
    let threshold_index = 4 * usize::from(attacker_color) + 4;
    let before_upkeep = session
        .nyphur_common_upkeep()
        .expect("Nyphur common carrier");
    let before = before_upkeep.array_0x1fc[threshold_index];
    assert_eq!(before, 0x601f);
    let decay = (256_i32 - i32::from(before_upkeep.word_0x242)) as u16;

    session.step_outer_turn(PlayerCommand::default(), 0);
    let accounted = session
        .economy()
        .possession_projectiles()
        .iter()
        .find(|candidate| candidate.source_entity_index == projectile.source_entity_index)
        .copied()
        .expect("accounted projectile remains in its source generation");
    assert!(accounted.active);
    assert!(accounted.threat_accounted);
    let after_accounting = session
        .nyphur_common_upkeep()
        .expect("post-accounting Nyphur common carrier");
    assert_eq!(
        after_accounting.array_0x1fc[threshold_index],
        before.wrapping_add(512).wrapping_sub(decay),
        "mana/4 is applied before the source-shaped same-pass common decay"
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|candidate| candidate.source_entity_index == projectile.source_entity_index)
            .is_none_or(|candidate| candidate.threat_accounted)
    );
    let following = session
        .nyphur_common_upkeep()
        .expect("following Nyphur common carrier");
    let following_decay = (256_i32 - i32::from(after_accounting.word_0x242)) as u16;
    assert_eq!(
        following.array_0x1fc[threshold_index],
        after_accounting.array_0x1fc[threshold_index].wrapping_sub(following_decay),
        "the live projectile cannot add its threat a second time"
    );

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_sub_159e0_accounts_an_ordinary_fireball_class3_relation_once() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 projectile-threat session");
    session.select_level2_campaign_entry_stage();
    let baseline_hash = session.campaign_state_hash();

    let nyphur = session
        .entities()
        .iter()
        .find(|entity| {
            entity.active
                && entity.index == LEVEL2_NYPHUR_ENTITY_INDEX
                && entity.class == 3
                && entity.model == 1
        })
        .cloned()
        .expect("qualified Nyphur source row");
    let collision = nyphur.collision_target();
    let target_position = Position {
        z: nyphur
            .position
            .z
            .wrapping_add(collision.collision_vertical_offset),
        ..nyphur.position
    };
    let mut player_position = target_position;
    player_position.x = player_position.x.wrapping_sub(4_096);
    player_position.z = session.canonical_terrain().ground_height(player_position);
    session
        .set_level2_entity_position(session.state.entity_index, player_position)
        .expect("local player input position remains map-linked");

    let mut launch_position = player_position;
    let direction_bits = session.shared_spells().player_entity.control_flags & 0x0300;
    let hand_yaw = if direction_bits & 0x0100 != 0 {
        Some(session.state.yaw.wrapping_sub(512))
    } else if direction_bits & 0x0200 != 0 {
        Some(session.state.yaw.wrapping_add(512))
    } else {
        None
    };
    if let Some(hand_yaw) = hand_yaw {
        crate::move_entity(&mut launch_position, hand_yaw, 0, 256);
        if session.canonical_terrain().ground_height(launch_position) > launch_position.z {
            launch_position = player_position;
        }
    }
    launch_position.z = launch_position.z.wrapping_add(session.state.entity_fov);
    let (launch_yaw, launch_pitch) = projectile_guidance_angles(launch_position, target_position);
    session.step_outer_turn(
        PlayerCommand {
            opcode: 6,
            argument_1: 0,
            flags: 0x40,
            argument_6: launch_yaw.wrapping_sub(session.state.yaw).cast_unsigned(),
            argument_8: launch_pitch
                .wrapping_sub(session.state.display_pitch)
                .cast_unsigned(),
            ..PlayerCommand::default()
        },
        0,
    );
    let mut projectile = session
        .economy()
        .fireball_projectiles()
        .iter()
        .find(|projectile| {
            projectile.active
                && projectile.owner_entity_index == session.state.entity_index
                && projectile.source_model == crate::level1_mission::FIREBALL_PROJECTILE_MODEL
        })
        .copied()
        .expect("ordinary family-0 holder creates a Fireball flyer");
    if projectile.target_source_entity_index == 0 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        projectile = session
            .economy()
            .fireball_projectiles()
            .iter()
            .find(|candidate| candidate.source_entity_index == projectile.source_entity_index)
            .copied()
            .expect("following numeric pass visits the earlier-slot Fireball");
    }
    assert_eq!(
        projectile.target_source_entity_index,
        LEVEL2_NYPHUR_ENTITY_INDEX
    );
    assert!(!projectile.threat_accounted);

    let attacker_color = session
        .players()
        .iter()
        .find(|player| player.entity_index == session.state.entity_index)
        .expect("local player source row")
        .player_color_index();
    let threshold_index = 4 * usize::from(attacker_color) + 4;
    let before_upkeep = session
        .nyphur_common_upkeep()
        .expect("Nyphur common carrier");
    let before = before_upkeep.array_0x1fc[threshold_index];
    let decay = (256_i32 - i32::from(before_upkeep.word_0x242)) as u16;

    session.step_outer_turn(PlayerCommand::default(), 0);
    let accounted = session
        .economy()
        .fireball_projectiles()
        .iter()
        .find(|candidate| candidate.source_entity_index == projectile.source_entity_index)
        .copied()
        .expect("accounted Fireball remains reserved for its source generation");
    assert!(accounted.threat_accounted);
    let after = session
        .nyphur_common_upkeep()
        .expect("post-accounting Nyphur common carrier");
    assert_eq!(
        after.array_0x1fc[threshold_index],
        before.wrapping_add(500).wrapping_sub(decay),
        "model-0 class-3 pressure is applied once before Nyphur common decay"
    );

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
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
    let baseline_economy_hash = session.session.state.live.economy.reset_hash();
    let baseline_shared_spells_hash = session.shared_spells.reset_hash();
    let baseline_global_rng = session.session.state.live.economy.global_rng;
    let baseline_runtime = session.runtime.clone();
    let baseline_entity_record = session.session.state.live.economy.entities[112]
        .record_bytes()
        .to_vec();
    let baseline_entity_phase = session.session.state.live.economy.entities[112].phase;

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
        session.economy.entities[112].phase,
        baseline_entity_phase.wrapping_add(1)
    );
    assert_eq!(
        session.economy.entities[112].record_bytes()[66],
        baseline_entity_phase.wrapping_add(1)
    );
    assert!(
        session.nyphur_participant().is_ok(),
        "the source-owned phase increment is the only mutable census byte"
    );
    let mut phase_record_mismatch = session.clone();
    phase_record_mismatch.session.state.live.economy.entities[112].record[66] ^= 1;
    assert!(phase_record_mismatch.nyphur_participant().is_err());
    let mut non_phase_record_mutation = session.clone();
    non_phase_record_mutation
        .session
        .state
        .live
        .economy
        .entities[112]
        .record[67] ^= 1;
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
    assert_eq!(
        session.economy.entities[112].record_bytes(),
        baseline_entity_record
    );
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
        Mc2DispatchEvent::Error {
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
    let source_before = session.session.state.live.economy.entities[112]
        .record_bytes()
        .to_vec();
    let fireball_holder = session
        .nyphur_common_upkeep()
        .expect("qualified Nyphur common carrier")
        .spell_holder_indices[0];
    assert_ne!(fireball_holder, 0);

    // Reusing the authored family-0 row as a family-4 table entry is a
    // malformed carrier, not a producer. The source-owned class/model/
    // action/parent checks must reject it without touching the source.
    session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .spell_holder_indices[4] = fireball_holder;
    assert!(
        !session
            .session
            .state
            .live
            .nyphur_dispatcher
            .step_sub_15fc0(112, &mut session.session.state.live.economy.entities)
    );
    assert_eq!(
        session.economy.entities[112].record_bytes(),
        source_before.as_slice()
    );
    assert!(
        session
            .nyphur_dispatcher
            .common_upkeep
            .source_family4_holder(&session.economy.entities)
            .is_none()
    );

    // A stale externally latched state is equally fail-closed at the
    // mode-14 handler boundary; it cannot consume the family-0 holder.
    session.session.state.live.nyphur_dispatcher.state_byte = 14;
    let terrain = session.canonical_terrain().clone();
    let players = session.players().to_vec();
    session
        .session
        .state
        .live
        .nyphur_dispatcher
        .step_sub_161a0(
            112,
            &mut session.session.state.live.economy.entities,
            &players,
            &terrain,
        )
        .expect("malformed mode-14 carrier fails through movement epilogue");
    assert_eq!(session.economy.entities[112].source_word_96(), 0);
    assert_eq!(session.economy.entities[112].source_word_98(), 0);
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
    let (_, target_index) = advance_to_class5_target(&mut related);
    assert!(level2_nyphur_source_identity_matches(
        &related.economy.entities[112],
        &related.economy.entities[target_index]
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
    dispatched_mode1
        .session
        .state
        .live
        .nyphur_dispatcher
        .state_byte = 1;
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
                Mc2DispatchEvent::Error {
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
    inclusive.session.state.live.economy.entities[112].set_position(source_position);
    inclusive.session.state.live.economy.entities[target_index].set_position(Position {
        x: source_position.x + 512,
        ..source_position
    });
    let inclusive_terrain = inclusive.canonical_terrain().clone();
    inclusive
        .session
        .state
        .live
        .nyphur_dispatcher
        .step_sub_12ff0(
            112,
            &mut inclusive.session.state.live.economy.entities,
            &inclusive_terrain,
        )
        .expect("inclusive sub_12FF0 threshold");
    assert_eq!(inclusive.nyphur_dispatcher.common_upkeep.speed, 0);
    assert_eq!(inclusive.nyphur_dispatcher.common_upkeep.word_0xe_14, 1);
    assert_eq!(
        inclusive.economy.entities[112].position.z,
        source_position.z - 1
    );

    let mut exact_height = related.clone();
    exact_height.session.state.live.economy.entities[112].set_position(source_position);
    exact_height.session.state.live.economy.entities[target_index].set_position(Position {
        z: source_position.z - 512,
        ..source_position
    });
    let exact_height_terrain = exact_height.canonical_terrain().clone();
    exact_height
        .session
        .state
        .live
        .nyphur_dispatcher
        .step_sub_12ff0(
            112,
            &mut exact_height.session.state.live.economy.entities,
            &exact_height_terrain,
        )
        .expect("equal target-height delta");
    assert_eq!(
        exact_height.economy.entities[112].position.z,
        source_position.z
    );

    let mut exclusive = related.clone();
    exclusive.session.state.live.economy.entities[112].set_position(source_position);
    exclusive.session.state.live.economy.entities[target_index].set_position(Position {
        x: source_position.x + 513,
        ..source_position
    });
    let exclusive_terrain = exclusive.canonical_terrain().clone();
    exclusive
        .session
        .state
        .live
        .nyphur_dispatcher
        .step_sub_12ff0(
            112,
            &mut exclusive.session.state.live.economy.entities,
            &exclusive_terrain,
        )
        .expect("exclusive sub_12FF0 threshold");
    assert_eq!(
        exclusive.nyphur_dispatcher.common_upkeep.speed,
        exclusive.economy.entities[112].minimum_speed()
    );
    assert_eq!(
        exclusive.economy.entities[112].position.z,
        source_position.z
    );

    let mut stale = inclusive.clone();
    let preserved_roll = stale.session.state.live.economy.entities[112].source_roll();
    let stale_identity =
        level2_nyphur_source_identity(&stale.session.state.live.economy.entities[target_index])
            .wrapping_add(1);
    stale.session.state.live.economy.entities[112].record[156..158]
        .copy_from_slice(&stale_identity.to_le_bytes());
    let stale_terrain = stale.canonical_terrain().clone();
    stale
        .session
        .state
        .live
        .nyphur_dispatcher
        .step_sub_12ff0(
            112,
            &mut stale.session.state.live.economy.entities,
            &stale_terrain,
        )
        .expect("stale identity is a source false result");
    assert_eq!(stale.economy.entities[112].source_roll(), preserved_roll);

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
    dispatched_mode2
        .session
        .state
        .live
        .nyphur_dispatcher
        .state_byte = 2;
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
                Mc2DispatchEvent::Error {
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
fn level_two_nyphur_replaced_target_keeps_the_ordinary_avoidance_tail() {
    let mut session = NativeWorld::inspect_level2(level_two_fixture_path())
        .unwrap()
        .level2_reset_session()
        .unwrap();
    advance_to_class5_target(&mut session);
    let live = &mut session.session.state.live;
    let target_slot = live.economy.entities[112].source_word_96();
    assert_ne!(
        target_slot, 0,
        "the ordinary selector must create the relation"
    );
    assert!(level2_nyphur_source_identity_matches(
        &live.economy.entities[112],
        &live.economy.entities[usize::from(target_slot)]
    ));
    // Only invalidate the stored generation. This models the actual reused
    // target which stopped the ordinary native run at tick 1764.
    let identity = live.economy.entities[112].source_word_98();
    live.economy.entities[112].record[156..158]
        .copy_from_slice(&identity.wrapping_add(1).to_le_bytes());
    let source_before = live.economy.entities[112].clone();
    live.nyphur_dispatcher.common_upkeep.byte_0x45e = 3;
    live.nyphur_dispatcher
        .step_sub_13850(
            112,
            &mut live.economy.entities,
            &live.core.players,
            &live.terrain,
            &session.session.definition.presentation,
        )
        .expect("a replaced target skips aiming and still performs avoidance");
    assert_eq!(live.economy.entities[112], source_before);
    assert_eq!(live.nyphur_dispatcher.common_upkeep.byte_0x45e, 4);
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
    let initial_entities = initial.session.state.live.economy.entities.clone();
    let initial_common = initial
        .nyphur_common_upkeep()
        .expect("initial Nyphur common carrier");

    // Let the ordinary selector consume higher-priority sphere targets
    // before it selects a class-5 relation for these mode-body vectors.
    let (selection_turn, target_index) = advance_to_class5_target(&mut initial);
    assert_eq!(initial.nyphur_dispatch_state_byte(), 13);
    let source = initial.session.state.live.economy.entities[112].clone();
    let target = initial.session.state.live.economy.entities[target_index].clone();
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
    stale_relation.session.state.live.economy.entities[112].record[156..158]
        .copy_from_slice(&source.source_word_98().wrapping_add(1).to_le_bytes());
    assert!(stale_relation.nyphur_participant().is_err());
    let mut unrelated_census_mutation = initial.clone();
    unrelated_census_mutation
        .session
        .state
        .live
        .economy
        .entities[112]
        .record[158] ^= 1;
    assert!(unrelated_census_mutation.nyphur_participant().is_err());

    // A later natural selection already has movement from earlier sphere
    // pursuit. Isolate the common callback so mode-body assertions do not
    // mistake that preceding movement for a write by sub_13890.
    let mut common_only = initial.clone();
    {
        let live = &mut common_only.session.state.live;
        live.nyphur_dispatcher
            .common_upkeep
            .step(
                112,
                &mut live.economy.entities,
                &live.terrain,
                &common_only.session.definition.presentation,
                live.core
                    .context
                    .effects
                    .as_ref()
                    .map_or(0, |effects| effects.move_speed_flag),
                live.shared_spells.local_player.player_color_index(),
                None,
            )
            .unwrap();
    }
    let common_source = common_only.economy.entities[112].clone();

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
        candidate.session.state.live.nyphur_dispatcher.state_byte = state_byte;
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
            Mc2DispatchEvent::Error {
                source_slot: 112,
                ..
            }
        )));
        let stepped_source = &candidate.session.state.live.economy.entities[112];
        assert_eq!(
            stepped_source.entity_rng, common_source.entity_rng,
            "far state-13 must not consume the movement RNG"
        );
        assert_eq!(
            stepped_source.yaw, common_source.yaw,
            "far state-13 must not enter the movement yaw branch"
        );
        assert_eq!(
            stepped_source.position, common_source.position,
            "far state-13 must not enter the movement position branch"
        );
        assert_eq!(
            stepped_source.source_roll(),
            crate::heading_for_positions(
                common_source.position,
                candidate.economy.entities[target_index].position
            ) as i16
        );
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
    direction_carrier
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .byte_0x45e = 3;
    let direction_terrain = direction_carrier.canonical_terrain().clone();
    let direction_players = direction_carrier.players().to_vec();
    direction_carrier
        .session
        .state
        .live
        .nyphur_dispatcher
        .step_sub_13850(
            112,
            &mut direction_carrier.session.state.live.economy.entities,
            &direction_players,
            &direction_terrain,
            &direction_carrier.session.definition.presentation,
        )
        .expect("sub_16580 direction carrier increment");
    assert_eq!(
        direction_carrier.nyphur_dispatcher.common_upkeep.byte_0x45e,
        4
    );

    let mut reset_carrier = initial.clone();
    reset_carrier
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .byte_0x45e = 8;
    let reset_terrain = reset_carrier.canonical_terrain().clone();
    let reset_players = reset_carrier.players().to_vec();
    reset_carrier
        .session
        .state
        .live
        .nyphur_dispatcher
        .step_sub_13850(
            112,
            &mut reset_carrier.session.state.live.economy.entities,
            &reset_players,
            &reset_terrain,
            &reset_carrier.session.definition.presentation,
        )
        .expect("sub_16580 terrain carrier reset");
    assert_eq!(reset_carrier.nyphur_dispatcher.common_upkeep.byte_0x45e, 0);

    // The source compares the stored sub_14C40 identity, not merely the
    // target slot. A mismatch skips aiming/casting, then executes sub_16580
    // just like the source false branch; it must not abort the outer turn.
    let mut wrong_identity = initial.clone();
    wrong_identity.nyphur_dispatcher.common_upkeep.byte_0x45e = 3;
    let source_before_negative = wrong_identity.session.state.live.economy.entities[112]
        .record_bytes()
        .to_vec();
    wrong_identity.session.state.live.economy.entities[target_index].model =
        wrong_identity.session.state.live.economy.entities[target_index]
            .model
            .wrapping_add(1);
    wrong_identity.session.state.live.economy.entities[target_index].record[68] =
        wrong_identity.session.state.live.economy.entities[target_index].model;
    let wrong_identity_terrain = wrong_identity.canonical_terrain().clone();
    let wrong_identity_players = wrong_identity.players().to_vec();
    wrong_identity
        .session
        .state
        .live
        .nyphur_dispatcher
        .step_sub_13850(
            112,
            &mut wrong_identity.session.state.live.economy.entities,
            &wrong_identity_players,
            &wrong_identity_terrain,
            &wrong_identity.session.definition.presentation,
        )
        .expect("replaced target runs the ordinary sub_16580 tail");
    assert_eq!(wrong_identity.nyphur_dispatcher.common_upkeep.byte_0x45e, 4);
    assert_eq!(
        wrong_identity.economy.entities[112].record_bytes(),
        source_before_negative.as_slice()
    );

    // Case 5 calls only the pinned weak `_nmemneed_0` hook before the
    // common tail. On this valid relation, a generic mode helper would
    // consume RNG and move the source; the structural branch changes only
    // the ordinary outer phase byte and emits the exact call sequence.
    let mut mode5 = initial.clone();
    mode5.session.state.live.nyphur_dispatcher.state_byte = 5;
    let mode5_source_before = common_source.record_bytes().to_vec();
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
    // An earlier cast can still have an active holder after its AI cooldown
    // expires. Its later ordinary callback consumes one tick and clears
    // positive parent regen (sub_693F0/sub_68DE0), independently of mode 5.
    let holder_ticks = [113, 114].map(|slot| {
        (
            initial.economy.entities[slot].active_value,
            mode5.economy.entities[slot].active_value,
        )
    });
    assert!(
        holder_ticks
            .iter()
            .any(|(before, after)| *before > 0 && *after == *before - 1),
        "an ordinary Nyphur holder consumes the pending spell tick: {holder_ticks:?}"
    );
    assert!(common_source.mana_regen > 0);
    mode5_expected_record[140..144].copy_from_slice(&0_i32.to_le_bytes());
    assert_eq!(
        mode5.economy.entities[112].record_bytes(),
        mode5_expected_record.as_slice()
    );
    assert!(mode5.dispatch_events().iter().all(|event| !matches!(
        event,
        Mc2DispatchEvent::Error {
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
    let stepped_entities = mode13_result.session.state.live.economy.entities.clone();
    let stepped_common = mode13_result
        .nyphur_common_upkeep()
        .expect("stepped mode-13 common carrier");
    let stepped_traces = mode13_result.nyphur_dispatch_traces().to_vec();
    let mut replay = mode13_result.clone();
    replay.reset();
    assert_eq!(replay.campaign_state_hash(), initial_campaign_hash);
    assert_eq!(replay.dispatch_observability_hash(), initial_dispatch_hash);
    assert_eq!(replay.economy.entities, initial_entities);
    assert_eq!(replay.nyphur_common_upkeep().unwrap(), initial_common);
    for _ in 0..=selection_turn {
        replay.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert_eq!(replay.campaign_state_hash(), stepped_campaign_hash);
    assert_eq!(replay.dispatch_observability_hash(), stepped_dispatch_hash);
    assert_eq!(replay.economy.entities, stepped_entities);
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
    let source = session.session.state.live.economy.entities[112].clone();

    // The qualified package intentionally has no sub_148E0 target rows;
    // the ordinary route must therefore leave both its source relation
    // and Watcom stream untouched rather than borrow sub_14530's model-1
    // target.
    let mut no_target = session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .clone();
    let rng_before = no_target.watcom_rand_state;
    assert!(
        no_target
            .step_sub_13ce0(&source, &session.economy.entities, &session.players)
            .is_none()
    );
    assert_eq!(no_target.watcom_rand_state, rng_before);
    assert!(level2_nyphur_class10_target_source_order(&session.economy.entities).is_empty());
    assert!(
        level2_nyphur_class5_bucket_order(&session.economy.entities)
            .into_iter()
            .filter_map(|index| session.economy.entities.get(usize::from(index)))
            .all(|entity| entity.model != 22)
    );

    // This source-index vector only represents the rebuilt dword_38523
    // links. It deliberately does not move a target, write word 0x96/0x98,
    // or enter the public dispatcher; it is focused order/nearest-helper
    // coverage, not ordinary activation evidence.
    let mut indexed_entities = session.session.state.live.economy.entities.clone();
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

    let mut indexed_selector = session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .clone();
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
    let mut model57_selector = session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .clone();
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

    // A genuine model39 constructor between UpdateEntities' list capture
    // and this callback must wait for the next pass's head. Projection of
    // its current typed row must not itself admit the new address.
    let mut born = session.clone();
    let players = born.players.clone();
    let live = &mut born.session.state.live;
    let before_birth = crate::campaign::source_arena::SourceManaChain::capture(
        &mut live.economy, Vec::new(),
    );
    let sphere_slot = live.economy.economy.spawn_mana_sphere_from_death(
        source.index, source.position, 0, 0x1234, 300, 0, &live.terrain,
    ).expect("real death-sphere constructor") as u16;
    let sphere = *live.economy.spheres().iter()
        .find(|sphere| sphere.active && sphere.source_entity_index == sphere_slot).unwrap();
    let mut current_entities = live.economy.entities.clone();
    level2_nyphur_project_mana_sphere(
        &mut current_entities[usize::from(sphere_slot)], sphere, sphere.id as u16,
    );
    level2_nyphur_rebuild_class10_source_order(&mut current_entities);
    let empty_class5: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let old_head = before_birth.current_slots(&live.economy, &live.economy.entities);
    assert!(old_head.is_empty());
    let mut selector = no_target.clone();
    let rng = selector.watcom_rand_state;
    assert_eq!(selector.step_sub_13ce0_with_buckets(
        &source, &current_entities, &players, Some(&empty_class5), Some(&old_head),
    ), None, "a mid-pass birth is absent from the frozen source mana head");
    assert_eq!(selector.watcom_rand_state, rng);
    let following = crate::campaign::source_arena::SourceManaChain::capture(
        &mut live.economy, vec![sphere_slot],
    );
    let current_head = following.current_slots(&live.economy, &live.economy.entities);
    assert_eq!(selector.step_sub_13ce0_with_buckets(
        &source, &current_entities, &players, Some(&empty_class5), Some(&current_head),
    ), Some(sphere_slot), "the following pass admits the same real sphere");
    current_entities[usize::from(sphere_slot)].set_control_flags(crate::native_world::ENTITY_DRAW_DISABLED_BIT);
    assert_eq!(selector.step_sub_13ce0_with_buckets(
        &source, &current_entities, &players, Some(&empty_class5), Some(&current_head),
    ), Some(sphere_slot), "sub148E0 still reads a current hidden member without a draw-flag gate");
}

#[test]
fn level_two_nyphur_sub_148e0_selects_authored_model39_from_ordinary_route() {
    let mut session = level_two_campaign_session();
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

    // The player reaches the imported row-11 switch through the public
    // campaign route. The switch constructs the genuine group below;
    // no target relation, AI state or sphere ownership is assigned.
    session
        .set_level2_entity_position(local_player, row11_position)
        .expect("public player input for authored row 11");
    session.step_outer_turn(PlayerCommand::default(), 0);

    let authored_model39_slots = session
        .session
        .state
        .live
        .economy
        .spheres()
        .iter()
        .filter(|sphere| {
            sphere.active && group1_positions.contains(&(sphere.position.x, sphere.position.y))
        })
        .map(|sphere| sphere.source_entity_index)
        .collect::<BTreeSet<_>>();
    assert_eq!(authored_model39_slots.len(), 28);
    let authored_handles = authored_model39_slots.iter().map(|&slot| {
        (slot, session.source_arena().handle(slot).expect("authored sphere generation"))
    }).collect::<BTreeMap<_, _>>();

    let mut selected = None;
    let mut traveled = false;
    for _ in 0..282 {
        // sub148E0 chooses from the entire live source head, including real
        // death rewards. It has no preference for this authored group.
        // Declare travel near one still-live original member on the next
        // natural decision cadence; the unfiltered selector must choose it.
        let live = &session.session.state.live;
        let phase = live.economy.entities[usize::from(LEVEL2_NYPHUR_ENTITY_INDEX)].phase;
        if !traveled && u16::from(phase) % live.nyphur_dispatcher.common_upkeep.cadence_period() == 0 {
            let sphere = live.economy.spheres().iter().filter(|sphere| {
                sphere.active && !sphere.pending_delete && sphere.player_entity_index == 0
                    && authored_handles.get(&sphere.source_entity_index)
                        .is_some_and(|handle| live.economy.handle(sphere.source_entity_index) == Some(*handle))
            }).min_by_key(|sphere| sphere.position.x)
                .expect("a genuine neutral group1 sphere remains before the decision");
            // Stay outside the whole cluster, rather than just one sphere.
            // The margin covers two 80-unit wizard moves and two clamped
            // 64-per-axis sphere moves before the neutral state6 observation.
            let clearance = 3_328_u32 + 512;
            let (mut approach, nearest_slot, nearest_distance) = [4_096_u16, 6_144, 8_192]
                .into_iter().find_map(|offset| {
                    let position = Position {
                        x: sphere.position.x.wrapping_sub(offset),
                        z: sphere.position.z.wrapping_add(512),
                        ..sphere.position
                    };
                    let nearest = live.economy.spheres().iter().filter(|row| row.active)
                        .min_by_key(|row| (level2_nyphur_source_distance_xy_euclidean_sq(position, row.position), row.source_entity_index))?;
                    let distance = level2_nyphur_source_distance_xy_euclidean_sq(position, nearest.position);
                    (distance > clearance * clearance && nearest.player_entity_index == 0
                        && !nearest.pending_delete && nearest.life == 300
                        && authored_handles.get(&nearest.source_entity_index)
                            .is_some_and(|handle| live.economy.handle(nearest.source_entity_index) == Some(*handle)))
                        .then_some((position, nearest.source_entity_index, distance))
                }).expect("travel outside every current sphere keeps an original authored member nearest");
            approach.z = approach.z.max(live.terrain.ground_height(approach).wrapping_add(256));
            eprintln!("authored mana travel: position={approach:?} nearest={nearest_slot} wrapped_xy_squared={nearest_distance} clearance_squared={}", clearance * clearance);
            // Leave the switch after its actual activation. The unchanged
            // Fireball+Possession inventory can attack a nearby human before
            // considering mana: parameter67's player perception is 8192 + 10.
            // Declared travel keeps that competing player outside its range.
            let mut departure = Position {
                x: approach.x.wrapping_add(16_384),
                y: approach.y.wrapping_add(16_384),
                z: 0,
            };
            departure.z = live.terrain.ground_height(departure).wrapping_add(512);
            session.set_level2_entity_position(local_player, departure)
                .expect("declared player travel away from the authored mana cluster");
            session.set_level2_entity_position(LEVEL2_NYPHUR_ENTITY_INDEX, approach)
                .expect("declared rival travel near the real authored sphere");
            traveled = true;
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        let target_slot = session.session.state.live.economy.entities
            [usize::from(LEVEL2_NYPHUR_ENTITY_INDEX)]
        .source_word_96();
        if authored_handles.get(&target_slot)
            .is_some_and(|handle| session.source_arena().handle(target_slot) == Some(*handle)) {
            selected = Some(target_slot);
            break;
        }
    }
    let selected_slot = selected.unwrap_or_else(|| {
        let live = &session.session.state.live;
        let source = &live.economy.entities[usize::from(LEVEL2_NYPHUR_ENTITY_INDEX)];
        let mut arena = live.economy.clone();
        let lists = super::super::area_damage::capture_effect_lists(&mut arena, None);
        let slots = lists.mana_chain().current_slots(&arena, &arena.entities);
        let mut entities = arena.entities.clone();
        for sphere in arena.spheres().iter().filter(|sphere| sphere.active) {
            level2_nyphur_project_mana_sphere(
                &mut entities[usize::from(sphere.source_entity_index)], *sphere, sphere.id as u16,
            );
        }
        let mut ai = live.nyphur_dispatcher.common_upkeep.clone();
        let holder1 = ai.sub_146c0(1, &entities).map(|row|
            (row.index, row.active, row.life, row.max_mana, row.active_value));
        let holder2 = ai.sub_146c0(2, &entities).map(|row|
            (row.index, row.active, row.life, row.max_mana, row.active_value));
        let buckets = lists.class5_current_slots(&arena);
        let admitted = ai.step_sub_13ce0_with_buckets(source, &entities, &live.core.players,
            Some(&buckets), Some(&slots));
        panic!("ordinary sub_148E0 selects an authored model-39 row: mana_slots={slots:?} admitted={admitted:?} state={} holder1={holder1:?} holder2={holder2:?} ai={:?} source={source:?} traces={:?}",
            live.nyphur_dispatcher.state_byte(), ai.snapshot(source.phase), session.nyphur_dispatch_traces());
    });
    assert!(traveled, "the declared selection prerequisite must have run");
    let selected_sphere = session
        .session
        .state
        .live
        .economy
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == selected_slot)
        .expect("selected authored model-39 remains in typed economy carrier");
    assert!(selected_sphere.active);
    assert_eq!((selected_sphere.max_life, selected_sphere.life), (300, 300));
    assert_eq!(selected_sphere.player_entity_index, 0);
    session
        .nyphur_participant()
        .expect("typed model-39 target remains a valid Nyphur relation");
    session
        .day_hud()
        .expect("typed model-39 target remains valid through the Day HUD projection");

    let mut wrong_identity = session.clone();
    let wrong_sphere = wrong_identity
        .session
        .state
        .live
        .economy
        .economy
        .spheres
        .iter_mut()
        .find(|sphere| sphere.source_entity_index == selected_slot)
        .expect("selected typed model-39 target for identity negative");
    wrong_sphere.id = wrong_sphere.id.wrapping_add(1);
    assert!(
        wrong_identity.nyphur_participant().is_err(),
        "the same live slot with a different typed source identity must not validate"
    );

    let mut dead = session.clone();
    dead.session
        .state
        .live
        .economy
        .economy
        .spheres
        .iter_mut()
        .find(|sphere| sphere.source_entity_index == selected_slot)
        .expect("selected typed model-39 target for life negative")
        .life = 0;
    dead.session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .castle_entity_index = 0;
    dead.session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .word_0x159 = 0;
    dead.step_outer_turn(PlayerCommand::default(), 0);
    assert_ne!(
        dead.economy.entities[usize::from(LEVEL2_NYPHUR_ENTITY_INDEX)].source_word_96(),
        selected_slot,
        "a projected typed sphere at source life zero must fail the following upkeep relation gate"
    );

    let mut retired = session.clone();
    let surviving_slot = retired
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.active && sphere.source_entity_index != selected_slot)
        .expect("second live sphere for source-owned merge retirement")
        .source_entity_index;
    assert!(
        retired
            .economy_mut()
            .merge_mana_spheres(surviving_slot, selected_slot)
    );
    assert!(
        retired.nyphur_participant().is_err(),
        "retirement does not erase the stored relation before Nyphur's following source visit"
    );
    // Isolate the ordinary sub_5EFA0 relation-cleanup branch from the Castle
    // overlap hold, which intentionally defers the mailbox callback.
    retired
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .castle_entity_index = 0;
    retired
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .word_0x159 = 0;
    retired.step_outer_turn(PlayerCommand::default(), 0);
    assert_ne!(
        retired.economy.entities[usize::from(LEVEL2_NYPHUR_ENTITY_INDEX)].source_word_96(),
        selected_slot,
        "the following Nyphur source visit clears the retired typed target before any same-callback reselection"
    );
    retired
        .nyphur_participant()
        .expect("participant validation recovers after source-owned relation cleanup");
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
        .session
        .state
        .live
        .economy
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == selected_slot)
        .expect("selected authored model-39 remains after the state-6 handoff");
    assert!(selected_sphere_after_dispatch.active);
    assert_eq!(selected_sphere_after_dispatch.player_entity_index, 0);
    assert_eq!(
        session.economy.entities[usize::from(LEVEL2_NYPHUR_ENTITY_INDEX)].source_word_96(),
        selected_slot
    );
}

#[test]
fn level_two_nyphur_authored_target_crosses_3328_from_ordinary_state6_route() {
    let mut session = level_two_campaign_session();
    let baseline_campaign_hash = session.campaign_state_hash();
    let authored_features = session.mission().authored_features().to_vec();
    let feature_position = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored switch-chain feature")
            .world_position(0)
    };
    let local_player = session.state.entity_index;
    let row342_position = feature_position(342);
    session
        .set_level2_entity_position(local_player, row342_position)
        .expect("public player input for imported row 342");
    let before_group24 = session
        .economy()
        .spheres()
        .iter()
        .filter(|sphere| sphere.active)
        .map(|sphere| sphere.source_entity_index)
        .collect::<BTreeSet<_>>();
    let mut group23_activated = false;
    for _ in 0..8 {
        session
            .set_level2_entity_position(local_player, row342_position)
            .expect("public player remains on imported row 342");
        session.step_outer_turn(PlayerCommand::default(), 0);
        group23_activated |= session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated {
                    group: 23,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == [343, 344]
            )
        });
        if group23_activated {
            break;
        }
    }
    assert!(group23_activated, "imported row 342 must activate group 23");

    let row344_position = feature_position(344);
    session
        .set_level2_entity_position(local_player, row344_position)
        .expect("public player moves to authored row 344");
    let mut authored_model39_slots = BTreeSet::new();
    // Row 344 can occupy a later source slot whose NewEvent seed is already
    // on cadence. The same group-23 pass then creates and consumes that
    // group-24 producer; observe its receipt before another pass reclaims it.
    let mut group24_activated = session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated {
                group: 24,
                authored_indices,
                ..
            } if authored_indices.as_ref()
                == [345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 361, 362, 363, 364]
        )
    });
    if group24_activated {
        authored_model39_slots.extend(
            session
                .economy()
                .spheres()
                .iter()
                .filter(|sphere| {
                    sphere.active && !before_group24.contains(&sphere.source_entity_index)
                })
                .map(|sphere| sphere.source_entity_index),
        );
    }
    for _ in 0..16 {
        if group24_activated {
            break;
        }
        session
            .set_level2_entity_position(local_player, row344_position)
            .expect("public player remains on authored row 344");
        session.step_outer_turn(PlayerCommand::default(), 0);
        group24_activated |= session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated {
                    group: 24,
                    authored_indices,
                    ..
                } if authored_indices.as_ref()
                    == [345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 361, 362, 363, 364]
            )
        });
        if group24_activated {
            authored_model39_slots.extend(
                session
                    .economy()
                    .spheres()
                    .iter()
                    .filter(|sphere| {
                        sphere.active && !before_group24.contains(&sphere.source_entity_index)
                    })
                    .map(|sphere| sphere.source_entity_index),
            );
            break;
        }
    }
    assert!(
        group24_activated,
        "authored row 344 must activate group 24: player {:?}, authoritative player {:?}, sources {:?}, imported roots {:?}, dispatch {:?}",
        (
            session.state.position,
            session.state.actual_speed,
            session.state.yaw,
            session.state.display_pitch
        ),
        session
            .economy
            .entities
            .get(usize::from(local_player))
            .map(|player| (
                player.active,
                player.class,
                player.model,
                player.action,
                player.life,
                player.position,
                player.actual_speed
            )),
        session.economy().authored_switch_producers(),
        session
            .level2_imported_switch_roots
            .iter()
            .filter(|source| source.authored_index == 342)
            .collect::<Vec<_>>(),
        session.dispatch_events(),
    );
    assert_eq!(authored_model39_slots.len(), 10);

    let row348_position = feature_position(348);
    let target_slot = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| {
            sphere.active
                && authored_model39_slots.contains(&sphere.source_entity_index)
                && (sphere.position.x, sphere.position.y) == (row348_position.x, row348_position.y)
        })
        .map(|sphere| sphere.source_entity_index)
        .expect("ordinary group-24 route materializes authored row 348");
    let target = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == target_slot)
        .expect("authored row 348 remains addressable after construction");
    assert_eq!((target.max_life, target.life), (300, 300));
    let pre_pursuit_projectiles = session
        .economy()
        .possession_projectiles()
        .iter()
        .filter(|projectile| {
            projectile.holder_entity_index == LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX
        })
        .map(|projectile| {
            (
                projectile.source_entity_index,
                session
                    .economy()
                    .slot_generation(projectile.source_entity_index),
            )
        })
        .collect::<BTreeSet<_>>();
    let target_xy = (row348_position.x, row348_position.y);
    let mut replay = session.clone();
    let mut selection = None;
    let mut crossed_3328 = None;
    let mut crossed_1024 = None;
    let mut activation = None;
    let mut minimum_distance = u32::MAX;
    let mut last_observation = None;
    let mut state6_visits_after_selection = 0_u8;
    for turn in 1..=240_u16 {
        let had_selection = selection.is_some();
        let source_before =
            session.economy.entities[usize::from(LEVEL2_NYPHUR_ENTITY_INDEX)].clone();
        session.step_outer_turn(PlayerCommand::default(), 0);
        replay.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(
            session.campaign_state_hash(),
            replay.campaign_state_hash(),
            "ordinary state-6 pursuit must replay identically at turn {turn}"
        );
        assert_eq!(session.dispatch_events(), replay.dispatch_events());
        assert_eq!(
            session.nyphur_dispatch_traces(),
            replay.nyphur_dispatch_traces()
        );
        let source = &session.economy.entities[usize::from(LEVEL2_NYPHUR_ENTITY_INDEX)];
        let selected_target = source.source_word_96();
        if selection.is_none() {
            if !authored_model39_slots.contains(&selected_target) {
                continue;
            }
            assert_eq!(
                selected_target, target_slot,
                "the first ordinary authored selection must be source-nearest row 348"
            );
        }
        assert_eq!(
            selected_target, target_slot,
            "ordinary selector must retain authored row 348 through pursuit"
        );
        if had_selection {
            state6_visits_after_selection = state6_visits_after_selection.saturating_add(1);
            let common = session
                .nyphur_common_upkeep()
                .expect("ordinary state-6 common carrier remains valid");
            match state6_visits_after_selection {
                1 => {
                    assert_eq!(
                        source.position, source_before.position,
                        "the first mode-6 visit cannot apply its newly requested speed"
                    );
                    assert_eq!(source.actual_speed, 0);
                    assert_eq!(common.speed, 80);
                }
                2 => {
                    assert_eq!(
                        (source.position.x, source.position.y),
                        (source_before.position.x, source_before.position.y),
                        "the following visit eases actual speed before XY displacement"
                    );
                    assert_eq!(source.actual_speed, 16);
                }
                3 => assert_ne!(
                    (source.position.x, source.position.y),
                    (source_before.position.x, source_before.position.y),
                    "the third visit must move using the prior visit's actual speed"
                ),
                _ => {}
            }
        }
        let Some(sphere) = session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.active && sphere.source_entity_index == target_slot)
        else {
            panic!("selected authored row 348 must remain a live typed sphere");
        };
        assert_eq!(
            (sphere.position.x, sphere.position.y),
            target_xy,
            "flat authored row 348 must remain stationary in XY"
        );
        let distance = level2_wrapping_target_distance(source.position, sphere.position);
        minimum_distance = minimum_distance.min(distance);
        last_observation = Some((
            turn,
            target_slot,
            source.position,
            sphere.position,
            distance,
        ));
        if selection.is_none() {
            selection = Some((turn, distance));
        }
        if distance <= 3_328 {
            crossed_3328.get_or_insert(turn);
        }
        if distance <= 1_024 {
            crossed_1024.get_or_insert(turn);
        }
        if sphere.player_entity_index == source.source_id {
            activation = Some((turn, target_slot, distance));
            break;
        }
    }

    let (_, initial_distance) = selection.unwrap_or_else(|| {
        panic!(
            "ordinary selector never chooses row 348; minimum={minimum_distance}, last={last_observation:?}, common={:?}, events={:?}",
            session.nyphur_common_upkeep(),
            session.dispatch_events()
        )
    });
    assert!(
        initial_distance > 3_328,
        "positive route must prove a natural far-to-near crossing, got {initial_distance}"
    );
    let crossed_3328 = crossed_3328.expect("state-6 pursuit naturally crosses 3328");
    let crossed_1024 = crossed_1024.expect("state-6 pursuit naturally crosses 1024");
    let (activation_turn, target_slot, activation_distance) =
        activation.expect("mode 6 claims the typed target after family-1 activation");
    assert!(crossed_3328 <= crossed_1024 && crossed_1024 <= activation_turn);
    assert!(activation_distance <= 1_024);
    assert!(session.nyphur_dispatch_traces().iter().any(|trace| {
        trace.source_slot == LEVEL2_NYPHUR_ENTITY_INDEX
            && trace.state_byte == 6
            && trace.calls.contains(&Level2NyphurDispatchCall::Helper(
                Level2NyphurDispatchHelper::Sub135c0,
            ))
    }));
    assert_eq!(
        session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == target_slot)
            .expect("claimed typed target remains addressable")
            .player_entity_index,
        LEVEL2_NYPHUR_ENTITY_INDEX
    );
    assert!(
        session.economy().spheres().iter().all(|sphere| {
            sphere.source_entity_index == target_slot
                || !authored_model39_slots.contains(&sphere.source_entity_index)
                || sphere.player_entity_index == 0
        }),
        "mode 6 must publish only the selected target relation"
    );

    let projectile = session
        .economy()
        .possession_projectiles()
        .iter()
        .find(|projectile| {
            projectile.holder_entity_index == LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX
                && !pre_pursuit_projectiles.contains(&(
                    projectile.source_entity_index,
                    session
                        .economy()
                        .slot_generation(projectile.source_entity_index),
                ))
        })
        .copied()
        .expect("the genuine holder-114 callback creates a new family-1 projectile generation");
    let projectile_generation = session
        .economy()
        .slot_generation(projectile.source_entity_index);
    let possession_profile = level2_direct_starting_spell_profile(1, 0)
        .expect("qualified Nyphur family-1 holder uses the source tier-0 profile");
    assert_eq!(
        session
            .nyphur_common_upkeep()
            .expect("ordinary Nyphur common carrier remains valid")
            .spell_enabled[1],
        LEVEL2_NYPHUR_SPELL_COOLDOWNS[1],
        "sub_14E10 installs the exact family-1 cooldown"
    );
    assert_eq!(
        session.economy.entities[usize::from(LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX)].active_value,
        possession_profile.word_0x30.cast_signed().wrapping_sub(1),
        "the same-pass holder callback consumes exactly one activation tick"
    );
    assert_eq!(
        session.economy.entities[usize::from(LEVEL2_NYPHUR_ENTITY_INDEX)].mana_regen,
        -possession_profile.max_mana,
        "fresh sub_69640/sub_68DE0 charges the exact family-1 mana cost"
    );
    let activation_upkeep = session
        .nyphur_common_upkeep()
        .expect("activation leaves the common carrier valid");
    assert_eq!(
        (activation_upkeep.mana, activation_upkeep.mana_regen),
        (1_000, -possession_profile.max_mana),
        "the charge is deferred until the following Nyphur visit"
    );
    assert_eq!(
        session
            .shared_spells()
            .activation_events()
            .iter()
            .filter(|event| matches!(
                event,
                crate::FlightEvent::PrepareSound {
                    entity_index,
                    player_index: -1,
                    sound_id: 40,
                    ..
                } if *entity_index == projectile.source_entity_index
            ))
            .count(),
        1,
        "the successful source allocation emits exactly one launch sound bound to the new row"
    );
    assert!(
        projectile.active,
        "a fresh family-1 flyer remains live after its first callback"
    );
    assert_eq!(
        projectile.phase, 1,
        "the scheduled callback advances byte 0x3e once"
    );
    let schedule_receipts = session.cf10_production_schedule_receipts();
    assert!(
        schedule_receipts.iter().any(|receipt| matches!(
            receipt,
            crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, .. }
                if *slot == projectile.source_entity_index
        )),
        "the new projectile must run through its scheduled source callback"
    );
    assert!(
        schedule_receipts.iter().any(|receipt| matches!(
            receipt,
            crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted { slot, .. }
                if *slot == projectile.source_entity_index
        )),
        "the new projectile callback must complete its source epilogue"
    );
    let callback_position = |slot| {
        schedule_receipts.iter().position(|receipt| {
            matches!(
                receipt,
                crate::campaign::schedule::ScheduleReceipt::CallbackCompleted {
                    slot: callback_slot,
                    ..
                } if *callback_slot == slot
            )
        })
    };
    let nyphur_callback = callback_position(LEVEL2_NYPHUR_ENTITY_INDEX)
        .expect("the activation pass dispatches Nyphur first");
    let holder_callback = callback_position(LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX)
        .expect("the activation pass dispatches holder 114");
    let projectile_callback = callback_position(projectile.source_entity_index)
        .expect("the activation pass dispatches the higher-slot projectile");
    assert!(nyphur_callback < holder_callback && holder_callback < projectile_callback);

    let mut terminal_phase = None;
    let mut verified_following_charge = false;
    for _ in 0..=u16::try_from(POSSESSION_PROJECTILE_LIFETIME_TICKS)
        .expect("possession lifetime fits a bounded test loop")
    {
        let before = session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|candidate| {
                candidate.source_entity_index == projectile.source_entity_index
                    && session
                        .economy()
                        .slot_generation(candidate.source_entity_index)
                        == projectile_generation
            })
            .copied()
            .expect("the active flyer generation remains addressable until its terminal callback");
        assert!(before.active);
        session.step_outer_turn(PlayerCommand::default(), 0);
        replay.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(session.campaign_state_hash(), replay.campaign_state_hash());
        assert!(
            session.dispatch_events() == replay.dispatch_events()
                && session.nyphur_dispatch_traces() == replay.nyphur_dispatch_traces()
        );
        let after = session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|candidate| {
                candidate.source_entity_index == projectile.source_entity_index
                    && session
                        .economy()
                        .slot_generation(candidate.source_entity_index)
                        == projectile_generation
            })
            .copied()
            .expect("a terminal callback hides the flyer before following-pass reclaim");
        assert_eq!(after.phase, before.phase.wrapping_add(1));
        let receipts = session.cf10_production_schedule_receipts();
        assert!(receipts.iter().any(|receipt| matches!(
            receipt,
            crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, .. }
                if *slot == projectile.source_entity_index
        )));
        assert!(receipts.iter().any(|receipt| matches!(
            receipt,
            crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted { slot, .. }
                if *slot == projectile.source_entity_index
        )));
        if !verified_following_charge {
            let following_upkeep = session
                .nyphur_common_upkeep()
                .expect("following Nyphur common carrier remains valid");
            assert_eq!(following_upkeep.mana, 900);
            assert_eq!(following_upkeep.mana_regen, 0);
            assert_eq!(following_upkeep.spell_enabled[1], 9);
            verified_following_charge = true;
        }
        if !after.active {
            terminal_phase = Some(after.phase);
            break;
        }
    }
    assert!(
        terminal_phase.is_some(),
        "the genuine family-1 flyer reaches its source terminal callback"
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    replay.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.campaign_state_hash(), replay.campaign_state_hash());
    assert_eq!(session.dispatch_events(), replay.dispatch_events());
    assert_eq!(
        session.nyphur_dispatch_traces(),
        replay.nyphur_dispatch_traces()
    );
    assert!(
        session
            .economy()
            .possession_projectiles()
            .iter()
            .all(|candidate| {
                candidate.source_entity_index != projectile.source_entity_index
                    || session
                        .economy()
                        .slot_generation(candidate.source_entity_index)
                        != projectile_generation
            }),
        "the following scheduler prologue reclaims the terminal flyer generation"
    );

    session.reset();
    replay.reset();
    assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);
    assert_eq!(replay.campaign_state_hash(), baseline_campaign_hash);
}

#[test]
fn level_two_nyphur_terminal_holder_epilogue_reads_the_raw_phase_authority() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    for holder_slot in [
        LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX,
        LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX,
    ] {
        let mut session = inspection
            .level2_reset_session()
            .expect("qualified Level 2 Nyphur holder session");
        {
            let holder = &mut session.session.state.live.economy.entities[usize::from(holder_slot)];
            holder.phase = 117;
            holder.record[66] = 117;
            holder.active_value = 1;
            holder.record[50..52].copy_from_slice(&1_i16.to_le_bytes());
            holder.record[48..50].copy_from_slice(&1_u16.to_le_bytes());
            holder.subspell_index = holder.subspell_index.wrapping_add(1);
        }
        let shared_holder = session
            .session
            .state
            .live
            .shared_spells
            .pickup_mut(holder_slot)
            .expect("the retained shared projection exists for the Nyphur holder");
        shared_holder.phase = 115;
        shared_holder.record[66] = 115;

        session.step_outer_turn(PlayerCommand::default(), 0);

        let raw_holder = &session.session.state.live.economy.entities[usize::from(holder_slot)];
        assert_eq!(raw_holder.active_value, 0);
        assert_eq!(raw_holder.phase, 118);
        assert_eq!(raw_holder.record[66], 118);
        assert!(session.dispatch_events().iter().all(|event| !matches!(
            event,
            Mc2DispatchEvent::Error { source_slot, .. } if *source_slot == holder_slot
        )));
        assert!(
            session
                .cf10_production_schedule_receipts()
                .iter()
                .any(|receipt| matches!(
                    receipt,
                    crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted { slot, .. }
                        if *slot == holder_slot
                ))
        );
    }
}

#[test]
fn level_two_nyphur_avoidance_packed_directions_los_and_zero_rng_are_exact() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("qualified Level 2 avoidance session");
    let mut terrain = session.canonical_terrain().clone();
    terrain.planes.terrain_types.fill(0);

    let origin = Position::default();
    terrain.planes.terrain_types[terrain_index(0, u8::MAX)] = 8;
    assert_eq!(level2_nyphur_sub_16730(&terrain, origin, 0, false), 1);
    assert_eq!(level2_nyphur_sub_16730(&terrain, origin, 0, true), 1);
    terrain.planes.terrain_types.fill(0);

    for (right_side, previous, blocker, expected) in [
        (true, 1_u8, (31_u8, 31_u8), 0x0608_u16),
        (true, 2, (33, 31), 0x0c01),
        (true, 4, (33, 33), 0x0902),
        (true, 8, (31, 33), 0x0304),
        (false, 1, (33, 31), 0x0302),
        (false, 2, (33, 33), 0x0604),
        (false, 4, (31, 33), 0x0c08),
        (false, 8, (31, 31), 0x0901),
    ] {
        terrain.planes.terrain_types.fill(0);
        terrain.planes.terrain_types[terrain_index(blocker.0, blocker.1)] = 8;
        assert_eq!(
            level2_nyphur_sub_16730(
                &terrain,
                Position {
                    x: 32 << 8,
                    y: 32 << 8,
                    z: 0,
                },
                previous,
                right_side,
            ),
            expected
        );
    }

    terrain.planes.terrain_types.fill(0);
    assert!(!level2_nyphur_sub_16e70(&terrain, (31, 31), (35, 31)));
    terrain.planes.terrain_types[terrain_index(35, 31)] = 8;
    assert!(level2_nyphur_sub_16e70(&terrain, (31, 31), (35, 31)));
    terrain.planes.terrain_types[terrain_index(35, 31)] = 0;
    terrain.planes.terrain_types[terrain_index(31, 31)] = 8;
    assert!(level2_nyphur_sub_16e70(&terrain, (31, 31), (35, 31)));

    terrain.planes.terrain_types.fill(0);
    session.session.state.live.economy.entities[112].set_position(Position {
        x: 32 << 8,
        y: 32 << 8,
        z: 256,
    });
    session.session.state.live.economy.entities[70].set_position(Position {
        x: 40 << 8,
        y: 33 << 8,
        z: 256,
    });
    let target = session.session.state.live.economy.entities[70].clone();
    session.session.state.live.economy.entities[112].set_source_relation(&target);
    session.session.state.live.economy.entities[112].set_yaw(0);
    session.session.state.live.economy.entities[112].set_source_roll(0);
    session.session.state.live.economy.entities[112].set_actual_speed(16);
    session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .speed = 80;
    session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .word_0xe_14 = 0;
    session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .byte_0x45e = 1;
    session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .byte_0x45f = 0;
    terrain.planes.terrain_types[terrain_index(32, 31)] = 8;
    let entity_rng = session.session.state.live.economy.entities[112].entity_rng;
    let watcom_rng = session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .watcom_rand_state;
    session
        .session
        .state
        .live
        .nyphur_dispatcher
        .step_sub_16580(
            112,
            &mut session.session.state.live.economy.entities,
            &terrain,
        )
        .expect("source avoidance turn");
    let source = &session.session.state.live.economy.entities[112];
    let common = &session.session.state.live.nyphur_dispatcher.common_upkeep;
    assert_eq!((common.byte_0x45e, common.byte_0x45f), (1, 1));
    assert_eq!((source.yaw, source.source_roll()), (1_536, 1_536));
    assert_eq!(source.actual_speed, 0);
    assert_eq!((common.speed, common.word_0xe_14), (0, 1));
    assert_eq!(source.entity_rng, entity_rng);
    assert_eq!(common.watcom_rand_state, watcom_rng);

    let mut left = session.clone();
    let mut right = session.clone();
    let mut symmetric = terrain.clone();
    symmetric.planes.terrain_types.fill(0);
    symmetric.planes.terrain_types[terrain_index(29, 31)] = 8;
    symmetric.planes.terrain_types[terrain_index(31, 32)] = 8;
    for (candidate, target_y, expected_state) in [
        (&mut left, 32_u16 << 8, 1_u8),
        (&mut right, 33_u16 << 8, 2_u8),
    ] {
        candidate.session.state.live.economy.entities[70].set_position(Position {
            x: 20 << 8,
            y: target_y,
            z: 256,
        });
        let target = candidate.session.state.live.economy.entities[70].clone();
        candidate.session.state.live.economy.entities[112].set_source_relation(&target);
        candidate
            .session
            .state
            .live
            .nyphur_dispatcher
            .common_upkeep
            .byte_0x45e = 0;
        candidate
            .session
            .state
            .live
            .nyphur_dispatcher
            .common_upkeep
            .byte_0x45f = 0;
        candidate
            .session
            .state
            .live
            .nyphur_dispatcher
            .step_sub_16580(
                112,
                &mut candidate.session.state.live.economy.entities,
                &symmetric,
            )
            .expect("forty-step obstacle side selection");
        assert_eq!(
            candidate
                .session
                .state
                .live
                .nyphur_dispatcher
                .common_upkeep
                .byte_0x45e,
            expected_state
        );
    }
}

#[test]
fn level_two_nyphur_state6_family1_gate_pitch_counter_and_relation_boundaries_are_exact() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let base = inspection
        .level2_reset_session()
        .expect("qualified Level 2 family-1 session");
    let source_parameter = base.economy.entities[112]
        .parameter_relation
        .expect("qualified Nyphur parameter relation");
    assert_eq!(source_parameter, 67);
    assert_eq!(
        i16::from_le_bytes([
            base.session.definition.presentation.entity_parameters[usize::from(source_parameter)]
                [14],
            base.session.definition.presentation.entity_parameters[usize::from(source_parameter)]
                [15],
        ]),
        -4,
        "sub_135C0 must use the decoded one-based source parameter row"
    );

    let prepare_family1 = |candidate: &mut super::super::Mc2Simulation| {
        let target = candidate.session.state.live.economy.entities[70].clone();
        let source = &mut candidate.session.state.live.economy.entities[112];
        source.set_source_relation(&target);
        source.set_yaw(0);
        source.set_source_roll(0);
        source.set_control_flags(source.control_flags | 0x0000_0100);
        let live = &mut candidate.session.state.live;
        assert_eq!(
            live.nyphur_dispatcher
                .common_upkeep
                .step_sub_15f20(1, 0, &mut live.economy.entities),
            1
        );
    };

    let mut angle_rejected = base.clone();
    prepare_family1(&mut angle_rejected);
    angle_rejected.session.state.live.economy.entities[112].set_source_roll(170);
    let target = angle_rejected.session.state.live.economy.entities[70].clone();
    let pitch_before = angle_rejected.session.state.live.economy.entities[112].pitch;
    let live = &mut angle_rejected.session.state.live;
    assert!(!live.nyphur_dispatcher.step_sub_14e10_case0(
        1,
        112,
        &target,
        &mut live.economy.entities,
    ));
    assert_eq!(live.economy.entities[112].control_flags & 0x0000_0100, 0);
    assert_eq!(live.economy.entities[112].pitch, pitch_before);
    assert_eq!(live.nyphur_dispatcher.common_upkeep.word_0x1a2, 0);

    let mut counter_rejected = base.clone();
    prepare_family1(&mut counter_rejected);
    counter_rejected
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .word_0x1a2 = -1;
    let target = counter_rejected.session.state.live.economy.entities[70].clone();
    let live = &mut counter_rejected.session.state.live;
    assert!(!live.nyphur_dispatcher.step_sub_14e10_case0(
        1,
        112,
        &target,
        &mut live.economy.entities,
    ));
    assert_eq!(live.economy.entities[112].control_flags & 0x0000_0100, 0);
    assert_eq!(live.nyphur_dispatcher.common_upkeep.word_0x1a2, -1);

    let mut accepted = base.clone();
    prepare_family1(&mut accepted);
    accepted.session.state.live.economy.entities[112].set_source_roll(169);
    let source_before = accepted.session.state.live.economy.entities[112].clone();
    let target = accepted.session.state.live.economy.entities[70].clone();
    let horizontal = crate::legacy_sqrt(super::super::level2_wrapping_xy_distance(
        source_before.position,
        target.position,
    ));
    let expected_pitch = crate::pitch_for_delta(
        i32::try_from(horizontal).expect("family-1 XY distance"),
        i32::from(target.position.z) - i32::from(source_before.position.z),
    );
    let live = &mut accepted.session.state.live;
    assert!(live.nyphur_dispatcher.step_sub_14e10_case0(
        1,
        112,
        &target,
        &mut live.economy.entities,
    ));
    assert_eq!(live.economy.entities[112].pitch, expected_pitch);
    assert_eq!(live.nyphur_dispatcher.common_upkeep.word_0x1a2, 1);
    assert_eq!(
        live.nyphur_dispatcher.common_upkeep.spell_enabled[1],
        LEVEL2_NYPHUR_SPELL_COOLDOWNS[1]
    );

    let mut wrapped_counter = base.clone();
    prepare_family1(&mut wrapped_counter);
    wrapped_counter.session.state.live.economy.entities[112].set_source_roll(169);
    wrapped_counter
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .word_0x1a2 = 7;
    let target = wrapped_counter.session.state.live.economy.entities[70].clone();
    let live = &mut wrapped_counter.session.state.live;
    assert!(live.nyphur_dispatcher.step_sub_14e10_case0(
        1,
        112,
        &target,
        &mut live.economy.entities,
    ));
    assert_eq!(live.nyphur_dispatcher.common_upkeep.word_0x1a2, -28);

    let mut clear_terrain = base.canonical_terrain().clone();
    clear_terrain.planes.terrain_types.fill(0);
    for (angle, initial_relation, expected_relation, wrote) in
        [(27_i16, 0_u16, 777_u16, true), (28, 0, 0, false), (27, 777, 777, true)]
    {
        let mut candidate = base.clone();
        let source_position = Position {
            x: 20_000,
            y: 20_000,
            z: 512,
        };
        let target_position = Position {
            x: 20_512,
            y: 20_000,
            z: 512,
        };
        candidate.session.state.live.economy.entities[112].set_position(source_position);
        candidate.session.state.live.economy.entities[112].source_id = 777;
        candidate.session.state.live.economy.entities[112].record[30..32]
            .copy_from_slice(&777_u16.to_le_bytes());
        candidate.session.state.live.economy.entities[70].set_position(target_position);
        candidate.session.state.live.economy.entities[70].set_source_player_entity_index(initial_relation);
        let target = candidate.session.state.live.economy.entities[70].clone();
        candidate.session.state.live.economy.entities[112].set_source_relation(&target);
        let heading = i16::try_from(heading_for_positions(source_position, target_position))
            .expect("state-6 boundary heading");
        candidate.session.state.live.economy.entities[112]
            .set_yaw(heading.wrapping_add(angle) & 0x07ff);
        candidate
            .session
            .state
            .live
            .nyphur_dispatcher
            .common_upkeep
            .spell_enabled[1] = 0;
        candidate
            .session
            .state
            .live
            .nyphur_dispatcher
            .common_upkeep
            .word_0x1a2 = 0;
        let presentation = candidate.session.definition.presentation.clone();
        let mut relation_write = None;
        candidate
            .session
            .state
            .live
            .nyphur_dispatcher
            .step_sub_135c0_with_write(
                112,
                &mut candidate.session.state.live.economy.entities,
                &clear_terrain,
                &presentation,
                &mut relation_write,
            )
            .expect("state-6 relation boundary");
        assert_eq!(
            candidate.session.state.live.economy.entities[70].source_player_entity_index(),
            expected_relation,
            "relation handoff boundary {angle}"
        );
        assert_eq!(relation_write, wrote.then_some((70, 777)),
            "receipt follows the actual source write even when its value is unchanged");
    }
}

#[test]
fn level_two_nyphur_sub_14c90_boundaries_match_pinned_source() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let base = inspection
        .level2_reset_session()
        .expect("qualified Level 2 Nyphur distance-gate session");
    let source_index = base
        .session
        .state
        .live
        .economy
        .entities
        .iter()
        .position(|entity| entity.index == 112)
        .expect("authored Nyphur source row");
    let source = base.session.state.live.economy.entities[source_index].clone();

    // These are direct gate probes only. They use a detached target value
    // and never rewrite target 70 or enter the public dispatcher; the
    // ordinary source route remains covered by the test above.
    let target_at_distance = |distance: u32| {
        let mut target = base.session.state.live.economy.entities[70].clone();
        target.set_position(Position {
            x: source
                .position
                .x
                .wrapping_add(u16::try_from(distance).expect("distance fits source coordinate")),
            y: source.position.y,
            z: source.position.z,
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
        let gate_result = candidate
            .session
            .state
            .live
            .nyphur_dispatcher
            .step_sub_14c90(
                source_index,
                Some(&target),
                None,
                near_distance,
                far_distance,
                &mut candidate.session.state.live.economy.entities,
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

    let mut vertical_only = base.clone();
    let mut vertical_target = vertical_only.session.state.live.economy.entities[70].clone();
    vertical_target.set_position(Position {
        x: source.position.x,
        y: source.position.y,
        z: source.position.z.wrapping_add(20_000),
    });
    assert!(
        vertical_only
            .session
            .state
            .live
            .nyphur_dispatcher
            .step_sub_14c90(
                source_index,
                Some(&vertical_target),
                None,
                1_024,
                3_072,
                &mut vertical_only.session.state.live.economy.entities,
            )
            .expect("source XY-only distance gate accepts a vertical-only delta")
    );
    assert_eq!(vertical_only.nyphur_dispatcher.common_upkeep.speed, 0);

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
        .session
        .state
        .live
        .economy
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
        .session
        .state
        .live
        .economy
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
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .spell_holder_indices[3] = speed_holder_slot;
    speed_up
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .spell_enabled[3] = 0;
    speed_up
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .max_mana = 1_000;
    speed_up
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .mana = 1_000;
    speed_up
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .speed = -1_234;
    assert!(
        speed_up
            .nyphur_dispatcher
            .source_spell_ready(3, &speed_up.economy.entities),
        "controlled family-3 carrier satisfies the source availability predicate"
    );
    let speed_target = target_at_distance(4_609);
    let close = speed_up
        .session
        .state
        .live
        .nyphur_dispatcher
        .step_sub_14c90(
            source_index,
            Some(&speed_target),
            None,
            3_328,
            4_608,
            &mut speed_up.session.state.live.economy.entities,
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
        .session
        .state
        .live
        .economy
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
        BTreeSet::from([0_i8, 6_i8]),
        "the qualified cursor selects the higher-priority live death sphere first"
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
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .spell_holder_indices[2] = 113;
    let selector_source = selector.session.state.live.economy.entities[112].clone();
    let expected_axis = selector
        .session
        .state
        .live
        .nyphur_dispatcher
        .sub_13b00_axis(
            &selector_source,
            &selector.session.state.live.economy.entities,
        )
        .expect("family-2 selector has a free arena cell");
    selector.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(selector.nyphur_dispatch_state_byte(), 3);
    assert_eq!(
        level2_nyphur_axis_position(&selector.economy.entities[112]).unwrap(),
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
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .spell_holder_indices[2] = 113;
    threshold_equal.session.state.live.economy.entities[111].model = 2;
    threshold_equal.session.state.live.economy.entities[111].position = Position {
        x: expected_axis.x.wrapping_add(0x3000),
        y: expected_axis.y,
        z: expected_axis.z,
    };
    assert_ne!(
        threshold_equal.nyphur_dispatcher.sub_13b00_axis(
            &threshold_equal.economy.entities[112],
            &threshold_equal.economy.entities
        ),
        Some(expected_axis)
    );
    let mut threshold_beyond = threshold_equal.clone();
    threshold_beyond.session.state.live.economy.entities[111]
        .position
        .x = expected_axis.x.wrapping_add(0x3001);
    assert_eq!(
        threshold_beyond.nyphur_dispatcher.sub_13b00_axis(
            &threshold_beyond.economy.entities[112],
            &threshold_beyond.economy.entities
        ),
        Some(expected_axis)
    );
    let mut unaffordable = ordinary.clone();
    unaffordable
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .spell_holder_indices[2] = 113;
    unaffordable.session.state.live.economy.entities[113].max_mana =
        unaffordable.session.state.live.economy.entities[112].max_mana + 1;
    assert_eq!(
        unaffordable.nyphur_dispatcher.sub_13b00_axis(
            &unaffordable.economy.entities[112],
            &unaffordable.economy.entities
        ),
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

    let source = ordinary.session.state.live.economy.entities[112].clone();
    let axis = level2_nyphur_axis_position(&source).expect("qualified axis carrier");
    let mut mode3 = ordinary.clone();
    mode3.session.state.live.nyphur_dispatcher.state_byte = 3;
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
        Mc2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));
    let mode3_source = &mode3.session.state.live.economy.entities[112];
    assert_eq!(
        mode3_source.source_roll(),
        i16::try_from(heading_for_positions(source.position, axis)).expect("mode-3 roll")
    );
    assert_eq!(
        mode3.nyphur_dispatcher.common_upkeep.word_0xe_14, 1,
        "sub_13100's sub_14C90 gate carries the vertical step"
    );
    let mode3_distance = level2_wrapping_target_distance(source.position, axis);
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
    let (_, mode4_target_index) = advance_to_class5_target(&mut mode4);
    let mode4_source = mode4.session.state.live.economy.entities[112].clone();
    assert_eq!(
        usize::from(mode4_source.source_word_96()),
        mode4_target_index
    );
    mode4.session.state.live.nyphur_dispatcher.state_byte = 4;
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
        Mc2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));
    assert_eq!(mode4.economy.entities[112].source_roll(), 0);
    assert_eq!(mode4.nyphur_dispatcher.common_upkeep.word_0xe_14, 1);
    assert_eq!(
        mode4.economy.entities[112].source_word_96(),
        mode4_source.source_word_96()
    );

    // A stale sub_14C60 identity is a source-level false result, not a
    // Rust error: sub_131F0 must tail once and leave the source roll
    // untouched. The byte-0x45e fast path makes the tail observable in
    // this private state-byte dispatcher vector.
    let mut stale_identity = mode4;
    let stale_source_roll = stale_identity.session.state.live.economy.entities[112].source_roll();
    let target_identity = level2_nyphur_source_identity(
        &stale_identity.session.state.live.economy.entities[mode4_target_index],
    );
    stale_identity.session.state.live.economy.entities[112].record[156..158]
        .copy_from_slice(&target_identity.wrapping_add(1).to_le_bytes());
    stale_identity
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .byte_0x45e = 3;
    stale_identity
        .session
        .state
        .live
        .nyphur_dispatcher
        .state_byte = 4;
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
                Mc2DispatchEvent::Error {
                    source_slot: 112,
                    ..
                }
            ))
    );
    assert_eq!(
        stale_identity.economy.entities[112].source_roll(),
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
    mode11.session.state.live.nyphur_dispatcher.state_byte = 11;
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
        Mc2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));
    assert_eq!(
        mode11.nyphur_dispatcher.common_upkeep.speed,
        mode11.economy.entities[112].minimum_speed()
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
    // A blocked terrain probe then enters the source avoidance state machine;
    // it is handled state, not an error/refusal path.
    let mut mode11_order = inspection
        .level2_reset_session()
        .expect("mode-11 order carrier");
    let order_source = mode11_order.session.state.live.economy.entities[112].clone();
    let blocked_x = order_source.position.x.to_be_bytes()[0];
    let blocked_y = order_source.position.y.to_be_bytes()[0].wrapping_sub(1);
    mode11_order.canonical_terrain_mut().planes.terrain_types
        [terrain_index(blocked_x, blocked_y)] = 8;
    mode11_order.session.state.live.nyphur_dispatcher.state_byte = 11;
    mode11_order.step_outer_turn(PlayerCommand::default(), 0);
    assert!(mode11_order.dispatch_events().iter().all(|event| !matches!(
        event,
        Mc2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));
    assert_eq!(
        mode11_order.nyphur_dispatcher.common_upkeep.speed, 0,
        "avoidance stops the source before installing the new heading"
    );
    assert_eq!(mode11_order.economy.entities[112].actual_speed, 0);
    assert_eq!(mode11_order.nyphur_dispatcher.common_upkeep.word_0xe_14, 1);
    assert_eq!(
        (
            mode11_order.nyphur_dispatcher.common_upkeep.byte_0x45e,
            mode11_order.nyphur_dispatcher.common_upkeep.byte_0x45f,
        ),
        (1, 1),
        "the blocked north probe chooses the pinned left-side avoidance row"
    );
    assert_eq!(
        (
            mode11_order.economy.entities[112].yaw,
            mode11_order.economy.entities[112].source_roll(),
        ),
        (1_536, 1_536)
    );

    let mut mode12 = inspection
        .level2_reset_session()
        .expect("mode-12 production carrier");
    let mode12_baseline_common = mode12
        .nyphur_common_upkeep()
        .expect("mode-12 baseline common carrier");
    assert_eq!(mode12.nyphur_dispatcher.common_upkeep.watcom_rand_state, 1);
    mode12.session.state.live.nyphur_dispatcher.state_byte = 12;
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
        Mc2DispatchEvent::Error {
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
        mode12.economy.entities[112].minimum_speed()
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
    let source = session.session.state.live.economy.entities[112].clone();
    let target = session.session.state.live.economy.entities[111].clone();

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

    let common = session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .clone();
    assert!(common.sub_15e60());
    assert!(!common.sub_156f0(2, &session.economy.entities));
    assert!(!common.sub_156f0(26, &session.economy.entities));
    assert!(common.sub_15760(0, &session.economy.entities).is_none());
    let class5_order =
        level2_nyphur_class5_bucket_order(&session.session.state.live.economy.entities);
    assert_eq!(class5_order.first().copied(), Some(39));
    assert_eq!(class5_order.last().copied(), Some(106));
    // The source's strict distance arbitration selects row 70, not the
    // first class-5 slot or an arbitrary vector-order entry.
    assert_eq!(
        session
            .nyphur_dispatcher
            .sub_14530_target(&source, &session.economy.entities),
        Some(70)
    );
    // Source sub_14530 follows the prepass bucket pointers and reads the
    // current occupant's id, mana and position. It does not re-admit rows by
    // their current class, action or life, or add rows absent from the list.
    let mut current = session.economy.entities.clone();
    current[70].class = 10;
    current[70].life = -1;
    current[70].action = 0xea;
    current[70].mana = 1;
    current[70].set_position(source.position);
    let mut frozen: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    frozen[2].push(70);
    assert_eq!(session.nyphur_dispatcher.sub_14530_target_with_slots(&source, &current, Some(&frozen)), Some(70));
    frozen[2].clear();
    assert_eq!(session.nyphur_dispatcher.sub_14530_target_with_slots(&source, &current, Some(&frozen)), None);
    let mut active_holder_entities = session.session.state.live.economy.entities.clone();
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

    let mut timer_source = session.session.state.live.economy.entities[111].clone();
    timer_source.record[58..60].fill(0);
    timer_source.apply_source_sub_5ef70();
    assert_eq!(
        i16::from_le_bytes([timer_source.record[58], timer_source.record[59]]),
        100
    );
    let mut timer_non_source = session.session.state.live.economy.entities[112].clone();
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
    let mut holder = session.session.state.live.economy.entities[114].clone();
    holder.activation_value = 3;
    let mut activation_flags = 0x320_u32;
    visibility_probe.step_sub_5f7b0(&mut holder, &mut activation_flags, 0x200);
    assert_eq!(holder.active_value, 3);
    assert_eq!(activation_flags, 0x200);

    let mut direct_holder = session.session.state.live.economy.entities[114].clone();
    direct_holder.active_value = 1;
    direct_holder.record[50..52].copy_from_slice(&1_i16.to_le_bytes());
    direct_holder.byte_3c = 0;
    direct_holder.record[64] = 0;
    let mut direct_flags = 0x320_u32;
    assert!(visibility_probe.step_sub_5f660(1, &mut direct_flags, &mut direct_holder, 0,));
    assert_eq!(direct_holder.byte_3c, 1);
    assert_eq!(direct_holder.record[64], 1);
    assert_eq!(direct_flags, 0);

    let mut non_success_holder = session.session.state.live.economy.entities[114].clone();
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

    let mut failed_holder = session.session.state.live.economy.entities[114].clone();
    failed_holder.active_value = 0;
    failed_holder.max_mana = 1;
    let mut failed_flags = 0x320_u32;
    visibility_probe.mana = 0;
    assert!(!visibility_probe.step_sub_5f660(0, &mut failed_flags, &mut failed_holder, 0,));
    assert_eq!(failed_flags, 0x320);
    assert_eq!(failed_holder.active_value, 0);

    // The owned player selector must execute before the still-unowned
    // model-3 selector. The qualified reset threshold declines the local
    // player, while the exact color latch admits it immediately.
    assert_eq!(
        session
            .nyphur_dispatcher
            .step_sub_14030(
                112,
                &session.economy.entities,
                &session.players,
                &session.session.definition.presentation,
            )
            .expect("qualified sub_14030 selector"),
        None
    );
    let mut player_latched = session.nyphur_dispatcher.clone();
    player_latched.common_upkeep.set_auxiliary_word(5, 1);
    assert_eq!(
        player_latched
            .step_sub_14030(
                112,
                &session.economy.entities,
                &session.players,
                &session.session.definition.presentation,
            )
            .expect("color-latched sub_14030 selector"),
        Some(111)
    );

    // The source's mana-advantage alternative is available only while the
    // candidate player's Type-164 carrier has no Castle relation. The live
    // dispatch view represents that Castle as class 3/model 2; treating it as
    // a class-2 scenery row would admit the player despite an owned Castle.
    let mut mana_advantage_entities = session.session.state.live.economy.entities.clone();
    mana_advantage_entities[112].mana = 10_000;
    mana_advantage_entities[111].mana = 0;
    mana_advantage_entities[111].position = mana_advantage_entities[112].position;
    let mut mana_advantage_players = session.players.clone();
    mana_advantage_players
        .iter_mut()
        .find(|player| player.entity_index == 111)
        .expect("qualified local player")
        .enabled_spell_holders[2] = 113;
    let candidate_source_id = mana_advantage_entities[111].source_id;
    assert!(
        mana_advantage_entities[113].index == 113
            && mana_advantage_entities[113].active
            && mana_advantage_entities[113].life >= 0,
        "family-2 holder setup must resolve through sub_146C0"
    );
    assert!(session.nyphur_dispatcher.common_upkeep.sub_15e60());
    assert!(
        session
            .nyphur_dispatcher
            .common_upkeep
            .sub_146c0(2, &mana_advantage_entities)
            .is_none(),
        "Nyphur's own family-2 holder would short-circuit sub_14030"
    );
    assert!(
        Level2NyphurCommonUpkeep::sub_146c0_slot(
            mana_advantage_players
                .iter()
                .find(|player| player.entity_index == 111)
                .expect("qualified local player")
                .enabled_spell_holders[2],
            &mana_advantage_entities,
        )
        .is_some(),
        "candidate family-2 holder must be live"
    );
    assert!(
        mana_advantage_entities[111].mana
            + 32 * (255 - i32::from(session.nyphur_dispatcher.common_upkeep.word_0x242,))
            < mana_advantage_entities[112].mana,
        "fixture must enter the mana-advantage alternative"
    );
    assert!(mana_advantage_entities.iter().all(|entity| {
        !(entity.active
            && entity.class == 3
            && entity.model == 2
            && entity.source_id == candidate_source_id)
    }));
    assert_eq!(
        session
            .nyphur_dispatcher
            .step_sub_14030(
                112,
                &mana_advantage_entities,
                &mana_advantage_players,
                &session.session.definition.presentation,
            )
            .expect("castle-free mana-advantage selector"),
        Some(111)
    );
    let mut castle_owned_entities = mana_advantage_entities.clone();
    let castle_slot = castle_owned_entities
        .iter()
        .position(|entity| entity.index != 0 && !entity.active)
        .expect("inactive source row for Castle projection");
    castle_owned_entities[castle_slot] = castle_owned_entities[111].clone();
    castle_owned_entities[castle_slot].index =
        u16::try_from(castle_slot).expect("Castle projection slot fits source index");
    castle_owned_entities[castle_slot].class = 3;
    castle_owned_entities[castle_slot].model = 2;
    assert_eq!(
        session
            .nyphur_dispatcher
            .step_sub_14030(
                112,
                &castle_owned_entities,
                &mana_advantage_players,
                &session.session.definition.presentation,
            )
            .expect("Castle-owned mana-advantage selector"),
        None
    );

    // The owned family-2 branch must produce its source axis before later
    // class-3 selectors are considered.
    let mut family_two_live = session.session.state.live.nyphur_dispatcher.clone();
    family_two_live.common_upkeep.spell_holder_indices[2] = 113;
    assert!(
        family_two_live
            .sub_13b00_axis(&source, &session.economy.entities)
            .is_some()
    );

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
    let baseline_economy_hash = session.session.state.live.economy.reset_hash();
    let baseline_economy_events = session.session.state.live.economy.events().to_vec();
    let baseline_global_rng = session.session.state.live.economy.global_rng;
    let baseline_runtime = session.runtime.clone();
    let baseline_entities = session.session.state.live.economy.entities.clone();
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
        Mc2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));

    // The exact cadence gate is 55 at the qualified reflex value. Goat
    // deaths now publish actual model-39 rows before the 53rd outer turn;
    // sub_13CE0 selects that higher-priority target before sub_14530.
    for _ in 1..53 {
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert_eq!(session.nyphur_dispatch_state_byte(), 6);
    let stepped_source = &session.session.state.live.economy.entities[112];
    let selected_target_index = usize::from(stepped_source.source_word_96());
    assert_eq!(selected_target_index, 119);
    let sphere = *session
        .economy
        .spheres()
        .iter()
        .find(|sphere| {
            sphere.active && usize::from(sphere.source_entity_index) == selected_target_index
        })
        .expect("the selected target is owned by the typed sphere arena");
    let mut selected_target = session.economy.entities[selected_target_index].clone();
    level2_nyphur_project_mana_sphere(
        &mut selected_target,
        sphere,
        u16::try_from(sphere.id).expect("source sphere identity"),
    );
    assert_eq!((selected_target.class, selected_target.model), (10, 39));
    assert!(session.economy.spheres().iter().any(|sphere| {
        sphere.active && usize::from(sphere.source_entity_index) == selected_target_index
    }));
    assert!(level2_nyphur_source_identity_matches(
        stepped_source,
        &selected_target
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
    // the persistent state to 6, so the source still performs both
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
        Mc2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));

    // The next ordinary cursor dispatch must execute the latched state-6
    // helper before its shared tail.  Capture the source carriers first so
    // a trace-only or tail-only implementation cannot satisfy this test.
    let transition_source = session.session.state.live.economy.entities[112].clone();
    let transition_target = selected_target;
    let transition_common = session
        .nyphur_common_upkeep()
        .expect("state-6 transition common carrier");
    assert_eq!(transition_common.byte_0x45d, 0);
    assert_eq!(transition_common.word_0xe_14, 0);

    session.step_outer_turn(PlayerCommand::default(), 0);
    let next_trace = session
        .nyphur_dispatch_traces()
        .last()
        .cloned()
        .expect("state-6 ordinary dispatch emits Nyphur trace");
    assert_eq!(next_trace.state_byte, 6);
    assert_eq!(next_trace.tail_call_count(), 1);
    assert_eq!(
        next_trace.calls.as_ref(),
        &[
            Level2NyphurDispatchCall::Sub12a70,
            Level2NyphurDispatchCall::Helper(Level2NyphurDispatchHelper::Sub135c0),
            Level2NyphurDispatchCall::Sub12e70,
        ]
    );
    assert!(session.dispatch_events().iter().all(|event| !matches!(
        event,
        Mc2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));
    let next_source = &session.session.state.live.economy.entities[112];
    assert_eq!(
        next_source.entity_rng, transition_source.entity_rng,
        "the far state-6 branch does not consume movement RNG"
    );
    assert_eq!(
        next_source.yaw, transition_source.yaw,
        "the far state-6 branch does not enter the yaw branch"
    );
    assert_eq!(
        next_source.position, transition_source.position,
        "the far state-6 branch does not enter the position branch"
    );
    assert_eq!(
        next_source.source_roll(),
        i16::try_from(crate::heading_for_positions(
            transition_source.position,
            transition_target.position,
        ))
        .expect("state-6 roll fits")
    );
    let next_common = session
        .nyphur_common_upkeep()
        .expect("state-6 ordinary dispatch common carrier");
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
    let stepped_economy_hash = session.session.state.live.economy.reset_hash();
    let stepped_economy_events = session.session.state.live.economy.events().to_vec();
    let stepped_global_rng = session.session.state.live.economy.global_rng;
    let stepped_runtime = session.runtime.clone();
    let stepped_entities = session.session.state.live.economy.entities.clone();
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
    assert_eq!(session.economy.entities, baseline_entities);
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
    assert_eq!(session.economy.entities, stepped_entities);
    assert_eq!(
        session.dispatch_events(),
        stepped_dispatch_events.as_slice()
    );
    assert_eq!(session.nyphur_dispatch_traces(), stepped_traces.as_slice());
}

#[test]
fn level_two_nyphur_sub_14250_uses_live_castle_defender_projection() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Castle-defender selector session");
    let owner_position = session.session.state.live.economy.entities[111].position;
    let source_position = session.session.state.live.economy.entities[112].position;
    let castle_slot = session
        .economy_mut()
        .build_castle(owner_position, 111)
        .expect("source-backed Castle root");
    {
        let mut economy = session.economy_mut();
        let castle = economy.castle.as_mut().expect("Castle root");
        castle.level = 1;
        castle.phase = 0;
        castle.mana = 0;
    }
    session.economy_mut().maintain_castle_defenders(castle_slot);
    let defender_slot = session
        .economy()
        .castle()
        .expect("Castle root")
        .defender_slots[0];
    assert_ne!(defender_slot, 0);
    {
        let mut economy = session.economy_mut();
        let defender = economy
            .castle_children
            .iter_mut()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("live Castle defender");
        defender.position = source_position;
        defender.mana = 10_000;
    }

    let castle = session.economy().castle();
    let castle_children = session.economy().castle_children().to_vec();
    let mut spheres = session.economy().spheres().to_vec();
    let mut entities = session.session.state.live.economy.entities.clone();
    entities[112].phase = 0;
    entities[112].record[66] = 0;
    entities[114].active_value = 1;
    let mut players = session.session.state.live.core.players.clone();
    let owner = players
        .iter_mut()
        .find(|player| player.entity_index == 111)
        .expect("Castle owner player");
    owner.enabled_spell_holders[11] = 114;
    let threshold_index = 4 * usize::from(owner.player_color_index()) + 4;
    let terrain = session.session.state.live.terrain.clone();
    let presentation = session.session.definition.presentation.clone();
    let active_level_color_index = session
        .session
        .state
        .live
        .shared_spells
        .local_player
        .player_color_index();
    let mut dispatcher = session.session.state.live.nyphur_dispatcher.clone();
    dispatcher.state_byte = 0;
    dispatcher
        .common_upkeep
        .set_auxiliary_word(threshold_index, u16::MAX);

    dispatcher
        .step_dispatch(
            0,
            112,
            &mut entities,
            &mut spheres,
            castle.as_slice(),
            &castle_children,
            &players,
            &terrain,
            &presentation,
            0,
            active_level_color_index,
            None,
        )
        .expect("eligible Castle defender dispatch");
    assert_eq!(dispatcher.state_byte, 9);
    assert_eq!(entities[112].source_word_96(), defender_slot);
    assert_eq!(
        entities[112].source_word_98(),
        level2_nyphur_source_identity_parts(111, 3, 3)
    );

    // The same source row is ineligible while it overlaps its owner's Castle;
    // its mere presence must neither select state 9 nor raise the old
    // unowned-selector error.
    let mut overlapping_children = castle_children;
    overlapping_children
        .iter_mut()
        .find(|child| child.source_entity_index == defender_slot)
        .expect("overlapping Castle defender")
        .position = owner_position;
    let mut overlapping_entities = session.session.state.live.economy.entities.clone();
    overlapping_entities[112].phase = 0;
    overlapping_entities[112].record[66] = 0;
    overlapping_entities[114].active_value = 1;
    let mut overlapping_spheres = session.economy().spheres().to_vec();
    let mut overlapping_dispatcher = session.session.state.live.nyphur_dispatcher.clone();
    overlapping_dispatcher.state_byte = 0;
    overlapping_dispatcher
        .common_upkeep
        .set_auxiliary_word(threshold_index, u16::MAX);
    overlapping_dispatcher
        .step_dispatch(
            0,
            112,
            &mut overlapping_entities,
            &mut overlapping_spheres,
            castle.as_slice(),
            &overlapping_children,
            &players,
            &terrain,
            &presentation,
            0,
            active_level_color_index,
            None,
        )
        .expect("overlapping Castle defender falls through");
    assert_ne!(overlapping_dispatcher.state_byte, 9);
    assert_ne!(overlapping_entities[112].source_word_96(), defender_slot);
}

#[test]
fn nyphur_dispatch_refreshes_castle_links_and_projects_both_owners() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
    let mut session = inspection.level2_reset_session().unwrap();
    let human_position = session.session.state.live.economy.entities[111].position;
    let rival_position = session.session.state.live.economy.entities[112].position;
    let human_castle = session
        .economy_mut()
        .build_castle(human_position, 111)
        .unwrap();
    let rival_castle = session
        .economy_mut()
        .build_castle(rival_position, 112)
        .unwrap();
    session.session.state.live.sync_castle_player_relations();
    let live = &session.session.state.live;
    for player in live
        .core
        .players
        .iter()
        .chain(live.shared_spells.players.iter())
    {
        if let Some(root) = live.economy.castle_for_owner(player.entity_index) {
            assert_eq!(
                u16::from_le_bytes([player.record[42], player.record[43]]),
                root.source_entity_index
            );
        }
    }
    let castles = live.economy.live_castles().copied().collect::<Vec<_>>();
    assert_eq!(castles.len(), 2);
    // These roots exercise the real AI projection/selector consumer. The
    // flyer constructor itself is covered by the Castle production checks.
    for (relation, expected) in [(rival_castle, rival_castle), (human_castle, 0), (0, 0)] {
        let mut entities = live.economy.entities.clone();
        let mut spheres = live.economy.spheres().to_vec();
        let mut players = live.core.players.clone();
        players
            .iter_mut()
            .find(|player| player.entity_index == 112)
            .unwrap()
            .record[42..44]
            .copy_from_slice(&relation.to_le_bytes());
        let mut dispatcher = live.nyphur_dispatcher.clone();
        dispatcher
            .step_dispatch(
                0,
                112,
                &mut entities,
                &mut spheres,
                &castles,
                live.economy.castle_children(),
                &players,
                &live.terrain,
                &session.session.definition.presentation,
                0,
                live.shared_spells.local_player.player_color_index(),
                None,
            )
            .unwrap();
        assert_eq!(dispatcher.common_upkeep.castle_entity_index, expected);
        assert!(!entities[usize::from(human_castle)].active);
        assert!(!entities[usize::from(rival_castle)].active);
    }
}

#[test]
fn nyphur_directional_prelude_resumes_same_visit_without_repeating_protection() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
    let session = inspection.level2_reset_session().unwrap();
    let live = &session.session.state.live;
    let players = &live.core.players;
    let castles = [];
    for (word159, admitted) in [(0, true), (1, false)] {
        let mut direct = live.nyphur_dispatcher.clone();
        let mut split = direct.clone();
        direct.common_upkeep.word_0x159 = word159;
        split.common_upkeep.word_0x159 = word159;
        let mut direct_entities = live.economy.entities.clone();
        let mut split_entities = direct_entities.clone();
        if !admitted {
            direct_entities[112].record[116..128].fill(0x5a);
            split_entities[112].record[116..128].fill(0x5a);
        }
        let mut direct_spheres = live.economy.spheres().to_vec();
        let mut split_spheres = direct_spheres.clone();
        let expected = direct
            .step_dispatch(
                0,
                112,
                &mut direct_entities,
                &mut direct_spheres,
                &castles,
                live.economy.castle_children(),
                players,
                &live.terrain,
                &session.session.definition.presentation,
                0,
                live.shared_spells.local_player.player_color_index(),
                None,
            )
            .unwrap();
        let prelude = split
            .prepare_directional_upkeep(112, &mut split_entities, &castles, players, &[])
            .unwrap();
        assert_eq!(prelude.admitted(), admitted);
        if !admitted {
            assert_eq!(&split_entities[112].record[98..134], &[0; 36]);
        }
        let actual = split
            .step_dispatch(
                0,
                112,
                &mut split_entities,
                &mut split_spheres,
                &castles,
                live.economy.castle_children(),
                players,
                &live.terrain,
                &session.session.definition.presentation,
                0,
                live.shared_spells.local_player.player_color_index(),
                Some(prelude),
            )
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(split.common_upkeep, direct.common_upkeep);
        assert_eq!(split_entities[112], direct_entities[112]);
    }
}

#[test]
fn nyphur_directional_feedback_tracks_consumed_lanes_not_effect_success() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
    let session = inspection.level2_reset_session().unwrap();
    let live = &session.session.state.live;
    let mut dispatcher = live.nyphur_dispatcher.clone();
    let mut actor = live.economy.entities[112].clone();
    assert!(dispatcher.apply_directional_feedback(&mut actor, false, true, false));
    assert_eq!(dispatcher.common_upkeep.player_hit_frame_time, 0);
    assert_eq!(dispatcher.common_upkeep.dword_0x18d, 0);
    assert_eq!(dispatcher.common_upkeep.word_0x24c, 0);
    assert!(dispatcher.apply_directional_feedback(&mut actor, true, false, false));
    assert_eq!(dispatcher.common_upkeep.player_hit_frame_time, 4);
    assert_eq!(dispatcher.common_upkeep.dword_0x18d, 16);
    assert_eq!(dispatcher.common_upkeep.word_0x24c, 64);
    let mut wrong_actor = actor.clone();
    wrong_actor.index = 111;
    assert!(!dispatcher.apply_directional_feedback(&mut wrong_actor, true, true, true));
}

#[test]
fn nyphur_directional_prelude_rejects_reused_typed_source_slot() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
    let session = inspection.level2_reset_session().unwrap();
    let live = &session.session.state.live;
    let mut dispatcher = live.nyphur_dispatcher.clone();
    let before = dispatcher.common_upkeep.clone();
    let mut entities = live.economy.entities.clone();
    let actor = entities[112].clone();
    assert!(
        dispatcher
            .prepare_directional_upkeep(112, &mut entities, &[], &live.core.players, &[112])
            .is_err()
    );
    assert_eq!(dispatcher.common_upkeep, before);
    assert_eq!(entities[112], actor);
}

#[test]
fn nyphur_directional_prelude_excludes_typed_occupant_shadow_from_player_scan() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
    let session = inspection.level2_reset_session().unwrap();
    let live = &session.session.state.live;
    let mut arena = live.economy.clone();
    let bee = arena
        .economy
        .allocate_authored_bee(1, live.core.state.position, None, 3_000, &live.terrain)
        .expect("ordinary Bee occupies a typed source slot");
    let shadow = &mut arena.entities[usize::from(bee.source_entity_index)];
    shadow.active = true;
    shadow.class = 3;
    shadow.model = 0;
    shadow.life = 1;
    let occupied = arena
        .economy
        .live_arena_slots()
        .iter()
        .map(|(slot, _)| *slot)
        .collect::<Vec<_>>();
    let expected = arena
        .entities
        .iter()
        .filter(|row| {
            row.active
                && row.life >= 0
                && row.class == 3
                && row.model == 0
                && !occupied.contains(&row.index)
        })
        .map(|row| row.index)
        .last();
    let mut dispatcher = live.nyphur_dispatcher.clone();
    let _prelude = dispatcher
        .prepare_directional_upkeep(112, &mut arena.entities, &[], &live.core.players, &occupied)
        .unwrap();
    assert_eq!(dispatcher.common_upkeep.source_list_model_zero, expected);
    assert_ne!(
        dispatcher.common_upkeep.source_list_model_zero,
        Some(bee.source_entity_index)
    );
}

#[test]
fn carried_steal_input_schedules_flyer_and_transfers_from_real_nyphur() {
    // Carry is a bounded initial-inventory setup; the hand command, holder,
    // flyer, field and recipient below all run on ordinary source passes.
    let mut session = level_two_campaign_session();
    session
        .apply_carried_spell_families(&[13], Some(13))
        .unwrap();
    let caster = session.session.state.live.core.state.entity_index;
    let target = LEVEL2_NYPHUR_ENTITY_INDEX;
    {
        let live = &mut session.session.state.live;
        let castle = live
            .economy
            .economy
            .build_castle_at(
                Position::default(),
                Position::default(),
                caster,
                false,
                true,
            )
            .expect("real local Castle constructor");
        // Stage the full Castle prerequisite: the default stage-one 8,500
        // capacity would otherwise release this mana before the hand cast.
        assert!(live.economy.economy.set_castle_mana_prerequisite_for_test(castle, 20_000));
        let parent = &mut live.economy.entities[usize::from(caster)];
        parent.max_mana = 20_000;
        parent.record[144..148].copy_from_slice(&20_000_i32.to_le_bytes());
        parent.set_mana(20_000);
        let parent = parent.clone();
        live.shared_spells
            .sync_player_entity_from_authority(&parent);
        live.shared_spells.player_mana = 20_000;
        live.shared_spells.caster.mana = 20_000;
        live.shared_spells
            .sync_typed_spell_parent_caches_from_player_entity();
        if let Some(fire) = live.core.context.spell_fire.as_mut() {
            fire.caster.mana = 20_000;
        }
        let target_row = &mut live.economy.entities[usize::from(target)];
        target_row.max_mana = 10_000;
        target_row.record[144..148].copy_from_slice(&10_000_i32.to_le_bytes());
        target_row.set_mana(8_000);
        let target_row = target_row.clone();
        let dispatcher = live.nyphur_dispatcher.as_mut().unwrap();
        assert!(dispatcher.sync_spell_target_resources(&target_row));
        dispatcher.common_upkeep.max_mana = 10_000;
        dispatcher.common_upkeep.word_0x159 = 0;
    }
    let ui_index = crate::SOURCE_SPELL_INDEX_MAP
        .iter()
        .position(|family| *family == 13)
        .unwrap() as i8;
    session.step_outer_turn(
        PlayerCommand {
            opcode: 31,
            argument_1: ui_index,
            argument_2: 0,
            ..PlayerCommand::default()
        },
        0,
    );
    let holder_slot = session.session.state.live.shared_spells.local_player
        .enabled_spell_holders[13];
    let mut cast_trace = Vec::new();
    {
        let live = &session.session.state.live;
        let cost = crate::level_definition::STEAL_MANA_TIERS[0].mana_cost;
        assert!(
            live.economy
                .owner_max_mana
                .get(&caster)
                .is_some_and(|capacity| *capacity >= cost)
        );
        assert!(live.economy.entities[usize::from(caster)].mana >= cost);
        assert!(
            live.economy.castle_for_owner(caster).is_some_and(|castle| {
                castle.mana >= crate::level_definition::STEAL_MANA_TIERS[0].castle_mana_required
            })
        );
        cast_trace.push(format!(
            "after hand selection: holder={:?} player={:?} capacity={:?} castle={:?} fire={:?}",
            live.shared_spells.pickup(holder_slot),
            live.economy.entities[usize::from(caster)],
            live.economy.owner_max_mana.get(&caster),
            live.economy.castle_for_owner(caster),
            live.core.context.spell_fire,
        ));
    }
    {
        // Stage the actual rival in the caster's forward arc before firing.
        // It remains under ordinary AI movement on every subsequent pass.
        let live = &mut session.session.state.live;
        let parent = live.economy.entities[usize::from(caster)].clone();
        let actor = crate::native_world::area_damage::source_actor(&mut live.economy, None, target)
            .expect("live source Nyphur target");
        let mut position = parent.position;
        crate::move_entity(&mut position, parent.yaw, parent.pitch, 1_536);
        position.z = parent
            .position
            .z
            .wrapping_add(parent.array_fov())
            .wrapping_sub(actor.shape.vertical_offset);
        live.economy.entities[usize::from(target)].set_position(position);
        crate::world::effects::sync_position(
            &mut live.economy,
            &mut live.terrain,
            target,
            position,
            true,
            true,
        )
        .unwrap();
    }
    let experience_before = session.session.state.live.shared_spells.spell_experience[13];
    session.step_outer_turn(
        PlayerCommand {
            flags: 0x10,
            ..PlayerCommand::default()
        },
        0,
    );
    let mut projectile = None;
    for _ in 0..4 {
        {
            let live = &session.session.state.live;
            cast_trace.push(format!(
                "after cast pass: holder={:?} numeric_holder={:?} player={:?} capacity={:?} castle={:?} fire={:?} events={:?} dispatch={:?}",
                live.shared_spells.pickup(holder_slot),
                live.economy.entities.get(usize::from(holder_slot)),
                live.economy.entities[usize::from(caster)],
                live.economy.owner_max_mana.get(&caster),
                live.economy.castle_for_owner(caster),
                live.core.context.spell_fire,
                live.core.context.events,
                session.dispatch_events(),
            ));
        }
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        projectile = session
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .find(|row| {
                row.active
                    && (row.class, row.model, row.action) == (9, 8, 8)
                    && row.source_id == caster
            })
            .map(|row| row.index);
        if projectile.is_some() {
            break;
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    let projectile = projectile.unwrap_or_else(|| {
        panic!(
            "ordinary hand input and scheduled holder produced no Steal flyer:\n{}",
            cast_trace.join("\n")
        )
    });
    let mut trace = Vec::new();
    for turn in 0..16 {
        let target_before = session.session.state.live.economy.entities[usize::from(target)].clone();
        session.step_outer_turn(PlayerCommand::default(), 0);
        let live = &session.session.state.live;
        let target_row = &live.economy.entities[usize::from(target)];
        trace.push(format!(
            "turn={turn} flyer={:?} target_before_mana={} target_before_regen={} target_before_lane={:?} target_after_mana={} target_after_regen={} target_after_max={} target_after_lane={:?} xp={} events={:?}",
            live.economy.entities[usize::from(projectile)],
            target_before.mana,
            target_before.mana_regen,
            target_before.pending_player_effect(crate::combat::actors::PlayerEffectLane::StealMana),
            target_row.mana,
            target_row.mana_regen,
            target_row.max_mana,
            target_row.pending_player_effect(crate::combat::actors::PlayerEffectLane::StealMana),
            live.shared_spells.spell_experience[13],
            session.dispatch_events()
        ));
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        // The flyer may write its hit after the rival's visit in this pass.
        // Wait for the next visit to consume that hit before checking mana.
        let pending_before = target_before
            .pending_player_effect(crate::combat::actors::PlayerEffectLane::StealMana);
        let pending_after = target_row
            .pending_player_effect(crate::combat::actors::PlayerEffectLane::StealMana);
        if live.shared_spells.spell_experience[13] > experience_before
            && pending_before.is_some_and(|hit| hit.attacker == caster && hit.payload == 0)
            && pending_after.is_none()
        {
            assert!(
                target_row.mana < 7_000,
                "actual recipient paid the tier-zero transfer:\n{}",
                trace.join("\n")
            );
            return;
        }
    }
    panic!(
        "scheduled carried Steal did not reach real Nyphur:\n{}",
        trace.join("\n")
    );
}

#[test]
fn scheduled_steal_contact_obeys_nyphur_protection_before_consumption() {
    use crate::combat::actors::PlayerEffectLane;

    // The holder and flyer callbacks stage a real impact; only the field and
    // recipient visits below are scheduler-driven. This does not exercise
    // normal spell acquisition or input selection.
    let (mut unprotected, caster, target, field) =
        crate::native_world::directional_spells::tests::staged_steal_mana_impact();
    assert_eq!(target, LEVEL2_NYPHUR_ENTITY_INDEX);
    assert!(
        field > target,
        "the field must run after this actor's first visit"
    );
    {
        let live = &mut unprotected.session.state.live;
        let target_row = live.economy.entities[usize::from(target)].clone();
        let dispatcher = live.nyphur_dispatcher.as_mut().unwrap();
        assert!(dispatcher.sync_spell_target_resources(&target_row));
        dispatcher.common_upkeep.max_mana = target_row.max_mana;
        dispatcher.common_upkeep.castle_entity_index = 0;
        dispatcher.common_upkeep.word_0x159 = 0;
    }
    let mut protected = unprotected.clone();
    protected
        .session
        .state
        .live
        .nyphur_dispatcher
        .as_mut()
        .unwrap()
        .common_upkeep
        .word_0x159 = 2;

    for session in [&mut unprotected, &mut protected] {
        let before_field = session.session.state.live.economy.entities[usize::from(field)].clone();
        let before_target =
            session.session.state.live.economy.entities[usize::from(target)].clone();
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        let live = &mut session.session.state.live;
        let actor = crate::native_world::area_damage::source_actor(&mut live.economy, None, target);
        let target_row = &live.economy.entities[usize::from(target)];
        let field_row = &live.economy.entities[usize::from(field)];
        let candidates = crate::level1_mission::source_map_search_slots(
            &live.economy.map_chains,
            field_row.position,
            &crate::level1_mission::source_search_offsets(2),
        );
        assert!(
            target_row
                .pending_player_effect(PlayerEffectLane::StealMana)
                .is_some(),
            "the scheduled field submitted its contact after the recipient's visit: \
             before_field={:?} before_target={:?} after_field={:?} after_target={:?} \
             actor={actor:?} target_linked={} target_tile={:#06x} candidates={candidates:?}",
            before_field,
            before_target,
            field_row,
            target_row,
            live.economy.map_chains.is_linked(target),
            live.economy.map_chains.tile_of(target),
        );
    }
    for session in [&mut unprotected, &mut protected] {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        assert!(
            session.session.state.live.economy.entities[usize::from(target)]
                .pending_player_effect(PlayerEffectLane::StealMana)
                .is_none(),
            "the next recipient visit must consume or clear the contact"
        );
    }
    let accepted = &unprotected.session.state.live;
    let rejected = &protected.session.state.live;
    assert_eq!(
        rejected.economy.entities[usize::from(target)].mana
            - accepted.economy.entities[usize::from(target)].mana,
        2_000,
        "only the unprotected recipient transfers mana"
    );
    assert_eq!(
        accepted.shared_spells.spell_experience[13],
        rejected.shared_spells.spell_experience[13] + 1
    );
    assert!(
        accepted.economy.entities[usize::from(caster)].mana
            >= rejected.economy.entities[usize::from(caster)].mana
    );
    assert_eq!(unprotected.nyphur_common_upkeep().unwrap().word_0x24c, 64);
    assert_ne!(protected.nyphur_common_upkeep().unwrap().word_0x24c, 64);
}

#[test]
fn direct_spell_resources_survive_actual_ai_upkeep() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
    let mut synchronized = inspection.level2_reset_session().unwrap();
    let baseline = synchronized.nyphur_common_upkeep().unwrap();
    let slot = usize::from(baseline.source_slot);
    let life = baseline.life - 200;
    let mana = baseline.mana - 400;
    {
        let live = &mut synchronized.session.state.live;
        live.economy.entities[slot].set_life(life);
        live.economy.entities[slot].set_mana(mana);
    }
    let mut unsynchronized = synchronized.clone();
    {
        let live = &mut synchronized.session.state.live;
        let dispatcher = live.nyphur_dispatcher.as_mut().unwrap();
        let before = dispatcher.common_upkeep(0);
        assert!(!dispatcher.sync_spell_target_resources(
            &live.economy.entities[usize::from(live.core.state.entity_index)],
        ));
        assert_eq!(dispatcher.common_upkeep(0), before);
        assert!(dispatcher.sync_spell_target_resources(&live.economy.entities[slot]));
    }
    // This checks the resource authority boundary and its actual next source
    // callback. It does not claim ordinary Steal Mana or Duel collision.
    synchronized.step_outer_turn(PlayerCommand::default(), 0);
    unsynchronized.step_outer_turn(PlayerCommand::default(), 0);
    let expected_life = life.wrapping_add(i32::from(baseline.life_regen));
    let expected_mana = mana
        .wrapping_add(baseline.mana_regen)
        .min(baseline.max_mana);
    let after = synchronized.nyphur_common_upkeep().unwrap();
    assert_eq!((after.life, after.mana), (expected_life, expected_mana));
    let row = &synchronized.session.state.live.economy.entities[slot];
    assert_eq!((row.life, row.mana), (expected_life, expected_mana));
    let control = unsynchronized.nyphur_common_upkeep().unwrap();
    assert_eq!((control.life, control.mana), (baseline.life, baseline.mana));
    assert_ne!((after.life, after.mana), (control.life, control.mana));
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
    let local_player_before = session.session.state.live.economy.entities
        [usize::from(session.state.entity_index)]
    .clone();
    assert_eq!((baseline.player_slot, baseline.player_color_index), (1, 1));
    assert_eq!(session.runtime.local_player_slot, 0);
    assert_eq!(baseline.source_slot, 112);
    assert_eq!(
        session.session.state.live.economy.entities[112].minimum_speed(),
        80
    );
    assert_eq!(
        &session.session.state.live.economy.entities[112].record_bytes()[132..138],
        &[0, 0, 0, 0, 80, 0],
        "the four-byte export prefix places source actualSpeed/minSpeed at 134/136"
    );
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
    let nyphur_before = session.session.state.live.economy.entities[112].clone();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let local_player_after =
        &session.session.state.live.economy.entities[usize::from(session.state.entity_index)];
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
    let nyphur_after = &session.session.state.live.economy.entities[112];
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
    assert_eq!(threshold_session.economy.entities[112].word_26(), 0);
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
    let holder_max = session.session.state.live.economy.entities[113].max_mana;
    let mut carrier = session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .clone();
    assert!(carrier.sub_146c0(1, &session.economy.entities).is_some());
    assert!(carrier.sub_146c0(2, &session.economy.entities).is_none());
    carrier.spell_holder_indices[6] = 113;
    carrier.max_mana = holder_max;
    assert!(carrier.sub_15730(6, &session.economy.entities));
    carrier.max_mana = holder_max - 1;
    assert!(!carrier.sub_15730(6, &session.economy.entities));
    carrier.spell_holder_indices[6] = u16::MAX;
    assert!(carrier.sub_146c0(6, &session.economy.entities).is_none());

    // sub_15F20 applies SetSpell before checking mana. The failed result
    // must still carry the tier-0 family-6 profile into the holder.
    let mut mutation_entities = session.session.state.live.economy.entities.clone();
    let before_holder = mutation_entities[113].clone();
    let mut mutation_probe = session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .clone();
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
    let mut fireball_profile_entities = session.session.state.live.economy.entities.clone();
    let mut fireball_profile_probe = session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .clone();
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

    let mut possession_profile_entities = session.session.state.live.economy.entities.clone();
    let mut possession_profile_probe = session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .clone();
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
    let mut rng_15790 = session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .clone();
    let mut rng_15910 = rng_15790.clone();
    rng_15790.word_0x244 = 255;
    let mut rng_entities = session.session.state.live.economy.entities.clone();
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
    let mut player_target_entities = session.session.state.live.economy.entities.clone();
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
    let mut rng_entities_without_optional = session.session.state.live.economy.entities.clone();
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
    let mut latch_falsifier_15790 = session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .clone();
    latch_falsifier_15790.max_mana = 1_000;
    latch_falsifier_15790.mana = 249;
    latch_falsifier_15790.word_0x1a4 = 0;
    assert!(!latch_falsifier_15790.update_spell_mana_latch_15790());
    assert_eq!(latch_falsifier_15790.word_0x1a4, 1);
    let mut latch_falsifier_15910 = session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .clone();
    latch_falsifier_15910.max_mana = 1_000;
    latch_falsifier_15910.mana = 249;
    latch_falsifier_15910.word_0x1a4 = 0;
    assert!(!latch_falsifier_15910.update_spell_mana_latch_15910());
    assert_eq!(latch_falsifier_15910.word_0x1a4, 1);

    // The 15790 latch clears at the inclusive recovery boundary after
    // its +6000 clamp reaches max/2.
    let mut latch = session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .clone();
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
    damaged
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .castle_entity_index = 70;
    damaged
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .life = damaged
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .max_life
        / 2
        - 1;
    damaged.session.state.live.economy.entities[112].phase = 1;
    let damaged_players = damaged.players().to_vec();
    damaged
        .session
        .state
        .live
        .nyphur_dispatcher
        .step_sub_12e70(
            112,
            &mut damaged.session.state.live.economy.entities,
            &damaged_players,
            &damaged.session.definition.presentation,
        )
        .expect("pre-cadence damaged Castle branch");
    assert_eq!(damaged.nyphur_dispatcher.state_byte, 11);
    assert_eq!(damaged.economy.entities[112].source_word_96(), 70);
    assert!(level2_nyphur_source_identity_matches(
        &damaged.economy.entities[112],
        &damaged.economy.entities[70]
    ));

    let mut at_half = session.clone();
    at_half
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .castle_entity_index = 70;
    at_half
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .life = at_half
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .max_life
        / 2;
    at_half.session.state.live.economy.entities[112].phase = 1;
    let source_before = at_half.session.state.live.economy.entities[112].clone();
    let at_half_players = at_half.players().to_vec();
    at_half
        .session
        .state
        .live
        .nyphur_dispatcher
        .step_sub_12e70(
            112,
            &mut at_half.session.state.live.economy.entities,
            &at_half_players,
            &at_half.session.definition.presentation,
        )
        .expect("cadence rejects full-health Castle branch");
    assert_eq!(at_half.nyphur_dispatcher.state_byte, 0);
    assert_eq!(
        at_half.economy.entities[112].source_word_96(),
        source_before.source_word_96()
    );

    // The owned entry gates are exercised without fabricating either of
    // the unowned dependent carriers: sub_13C50 needs family 2 and then
    // fails closed at sub_155E0; sub_13CE0 reaches its family-1 gate but
    // fails closed at sub_148E0.
    let mut castle = session.session.state.live.economy.entities[70].clone();
    castle.action = 4;
    castle.activation_value = 0;
    let source = session.session.state.live.economy.entities[112].clone();
    let mut branch_probe = session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .clone();
    branch_probe.castle_entity_index = 70;
    branch_probe.max_mana = 1_000;
    assert!(!branch_probe.step_sub_13c50(&source, Some(&castle), &session.economy.entities));
    assert!(
        branch_probe
            .step_sub_13ce0(&source, &session.economy.entities, &session.players)
            .is_none()
    );

    // The qualified constructor has no Castle, and its ordinary package
    // route currently reaches the existing state-13 selector. No honest
    // ordinary positive for sub_14630 is claimed in this focused vector;
    // that reachability remains open pending its owning upstream seam.

    // The new latch is part of the campaign digest and reset baseline.
    session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .word_0x1a4 = 1;
    assert_ne!(session.campaign_state_hash(), baseline_hash);
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

/// The ordinary Level 2 route reaches the genuine family-0 Fireball
/// producer with no selector, relation, position, or state coercion: the
/// production tail selects a class-5 row after sphere pursuit, then crosses the
/// 3,328 gate, and the pinned case-0 `sub_14E10` branch activates holder
/// 113. The proof continues through the same-pass holder producer, the
/// scheduled class-9 consumer and its epilogue, the following-visit mana
/// charge, the `word_0x1A2` burst counter, the terminal callback, the
/// following-pass reclaim, and reset/replay isolation.
#[test]
fn mode3_castle_seed_refreshes_following_tail_buckets() {
    let base = level_two_campaign_session();
    let terrain = base.canonical_terrain().clone();
    let position = base.economy.entities[112].position;
    let mut castle_world = base.clone();
    let castle_slot = castle_world.economy_mut().build_castle(position, 112)
        .expect("real Castle constructor supplies the successful-seed control");
    let castle = *castle_world.economy().castle_at(castle_slot).expect("constructed Castle");
    let profiles = std::array::from_fn(|tier| level2_set_spell_profile(2, tier as u8).unwrap());

    // The callback controls model the result returned by the allocator: a
    // failed allocation and a normal free-slot allocation retain the list;
    // exhausted-slot recycling clears it. The real allocator/list join has
    // its own source-scheduled check.
    for (name, seeded, clear_lists) in [("failure", false, false), ("normal", true, false), ("recycle", true, true)] {
        let mut candidate = base.clone();
        let mut case_terrain = terrain.clone();
        let live = &mut candidate.session.state.live;
        let source = &mut live.economy.entities[112];
        level2_nyphur_set_axis_position(source, position);
        let ai = &mut live.nyphur_dispatcher.common_upkeep;
        ai.spell_holder_indices[2] = 113;
        ai.spell_levels[2] = 0;
        ai.spell_enabled[2] = 0;
        ai.castle_entity_index = 0;
        ai.max_mana = 100_000;
        ai.mana = 100_000;
        let mut original: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
        original[1].push(70);
        let mut current = Some(original.clone());
        let mut newborn = None;
        let mut called = false;
        let mut seed = |owner: u16, axis: Position, _: &[EntitySnapshot], _: &mut SessionTerrain| {
            called = true;
            assert_eq!((owner, axis), (112, position));
            NyphurCastleSeed {
                castle: seeded.then_some(castle),
                class5_buckets: Some(if clear_lists { std::array::from_fn(|_| Vec::new()) } else { original.clone() }),
                mana_slots: Some(if clear_lists { Vec::new() } else { vec![1, 2] }),
            }
        };
        let mut current_mana = Some(vec![1, 2]);
        live.nyphur_dispatcher.step_sub_13100(112, &mut live.economy.entities, &mut case_terrain,
            &profiles, &mut seed, &mut newborn, &mut current, &mut current_mana).expect("mode3 seed callback");
        assert!(called, "{name} must reach the seed attempt");
        assert_eq!(newborn, seeded.then_some(castle_slot));
        assert_eq!(current_mana.as_deref(), Some(if clear_lists { &[][..] } else { &[1, 2][..] }),
            "{name} preserves the source mana-head allocation cut for the following tail");
        let selected = live.nyphur_dispatcher.sub_14530_target_with_slots(
            &live.economy.entities[112], &live.economy.entities, current.as_ref());
        assert_eq!(selected, (!clear_lists).then_some(70), "{name} following-tail list");
    }
}

#[test]
fn ordinary_rival_selects_typed_bee_and_fireball_reaches_damage_and_reclaim() {
    let mut session = level_two_campaign_session();
    // Activate the actual authored group-21 source. This stages the objective
    // condition only; the producer, Bee, rival and spell remain scheduled.
    let producer = session.economy.entities().iter().find(|row| {
        row.active && row.class == 11 && row.model == 32 && row.action == 32
            && row.source_id == 21 && !row.drawing_disabled()
    }).expect("ordinary group-21 producer");
    let producer_slot = producer.index;
    let stage = usize::try_from(producer.level_gate).expect("producer objective stage");
    for index in 0..session.runtime.objectives.len() {
        session.runtime.set_level2_objective_stage_selector_for(index, stage, 2);
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().any(|event| matches!(event,
        Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group: 21, .. }
            if *source_slot == producer_slot)));
    let bees = session.economy().bee_creatures().iter()
        .filter(|bee| bee.active && bee.authored_index().is_some())
        .map(|bee| (session.source_arena().handle(bee.source_entity_index).unwrap(), bee.life, bee.position))
        .collect::<Vec<_>>();
    assert!(!bees.is_empty(), "the source producer creates a typed Bee");

    // Declare a Fireball-only rival inventory for this combat check. The
    // ordinary package also grants Possession, whose higher-priority sphere
    // pursuit otherwise takes the rival away before it can select the Bee.
    // Keep both authoritative player projections and their records in sync.
    {
        let live = &mut session.session.state.live;
        for player in live.core.players.iter_mut().chain(live.shared_spells.players.iter_mut())
            .filter(|player| player.entity_index == 112)
        {
            assert!(player.set_enabled_spell_holder_index(1, 0));
        }
    }
    // A declared approach positions the actual rival near the produced group.
    // Target selection, mode, cadence, damage and products are not staged.
    let approach = Position {
        x: bees[0].2.x.wrapping_add(1_280),
        y: bees[0].2.y,
        z: bees[0].2.z.wrapping_add(512),
    };
    session.set_level2_entity_position(112, approach).expect("rival approach");
    let mut selected = None;
    let mut launched = None;
    let mut impact = None;
    let mut damaged = None;
    let mut observations = Vec::new();
    for turn in 0..600 {
        let source = &session.economy.entities[112];
        if turn % 55 == 0 {
            observations.push((turn, session.nyphur_dispatch_state_byte(), source.position,
                source.source_word_96(), session.nyphur_common_upkeep().unwrap().mana,
                session.economy().bee_creatures().iter().map(|bee|
                    (bee.source_entity_index, bee.position, bee.life, bee.action)).collect::<Vec<_>>()));
        }
        let target_before = source.source_word_96();
        let existing = session.economy().fireball_projectiles().iter()
            .filter_map(|row| session.source_arena().handle(row.source_entity_index))
            .collect::<Vec<_>>();
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().all(|event| !matches!(event,
            Mc2DispatchEvent::Error { source_slot: 112, .. })));
        for (handle, _, _) in &bees {
            if session.source_arena().handle(handle.source_slot) == Some(*handle) {
                assert!(!session.economy.entities[usize::from(handle.source_slot)].active,
                    "rival callback must not reactivate the typed Bee's raw shadow");
            }
        }
        let source = &session.economy.entities[112];
        if session.nyphur_dispatch_state_byte() == 13 {
            if let Some((handle, _, _)) = bees.iter().find(|(handle, _, _)|
                handle.source_slot == source.source_word_96()
                    && session.source_arena().handle(handle.source_slot) == Some(*handle))
            {
                selected.get_or_insert(*handle);
            }
        }
        if let Some(selected_handle) = selected {
            if launched.is_none() && target_before == selected_handle.source_slot {
                launched = session.economy().fireball_projectiles().iter()
                    .filter(|row| row.owner_entity_index == 112
                        && row.holder_entity_index == LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX)
                    .filter_map(|row| session.source_arena().handle(row.source_entity_index))
                    .find(|handle| !existing.contains(handle));
            }
            if launched.is_some() && impact.is_none() {
                impact = session.entities().iter().find(|row| row.active && row.class == 10
                    && row.model == 0 && row.source_id == 112
                    && row.source_word_96() == selected_handle.source_slot)
                    .and_then(|row| session.source_arena().handle(row.index));
            }
            if impact.is_some() {
                damaged = session.dispatch_events().iter().find_map(|event| match event {
                    Mc2DispatchEvent::Economy { source_slot, event: super::super::Level2EconomyEvent::DamageApplied {
                        target_entity_index, attacker_entity_index: 112, damage, ..
                    }, .. } if *source_slot == selected_handle.source_slot
                        && *target_entity_index == selected_handle.source_slot && *damage > 0 => Some(*damage),
                    _ => None,
                }).or(damaged);
            }
            if let (Some(flyer), Some(impact_handle), Some(_)) = (launched, impact, damaged) {
                if session.source_arena().handle(flyer.source_slot) != Some(flyer)
                    && session.source_arena().handle(impact_handle.source_slot) != Some(impact_handle)
                { break; }
            }
        }
    }
    let selected = selected.unwrap_or_else(|| panic!("ordinary rival selector must target the produced Bee: {observations:?}"));
    let flyer = launched.unwrap_or_else(|| panic!("ordinary selected Bee must lead to a real family-0 flyer: {observations:?}"));
    let impact = impact.unwrap_or_else(|| panic!("rival flyer {flyer:?} must impact selected Bee {selected:?}: {observations:?}"));
    assert!(damaged.is_some(), "scheduled Bee must consume rival Fireball damage: {observations:?}");
    let initial_life = bees.iter().find(|(handle, _, _)| *handle == selected).unwrap().1;
    assert!(session.economy().bee_creatures().iter().find(|bee|
        bee.source_entity_index == selected.source_slot).is_none_or(|bee| bee.life < initial_life));
    assert_ne!(session.source_arena().handle(flyer.source_slot), Some(flyer), "flyer generation reclaims");
    assert_ne!(session.source_arena().handle(impact.source_slot), Some(impact), "impact generation reclaims");
}

#[test]
fn level_two_nyphur_family0_activation_reaches_projectile_consumer_ordinarily() {
    let mut session = level_two_campaign_session();
    let baseline_campaign_hash = session.campaign_state_hash();
    advance_to_class5_target(&mut session);
    assert_eq!(session.nyphur_common_upkeep().unwrap().spell_enabled[0], 0);
    let mut replay = session.clone();

    let mut selection_turn = None;
    let mut crossed_3328 = None;
    let mut activation = None;
    for turn in 1..=300_u16 {
        let burst_before = session.nyphur_common_upkeep().unwrap().word_0x1a2;
        session.step_outer_turn(PlayerCommand::default(), 0);
        replay.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(
            session.campaign_state_hash(),
            replay.campaign_state_hash(),
            "ordinary family-0 pursuit must replay identically at turn {turn}"
        );
        assert_eq!(session.dispatch_events(), replay.dispatch_events());
        assert_eq!(
            session.nyphur_dispatch_traces(),
            replay.nyphur_dispatch_traces()
        );
        assert!(session.dispatch_events().iter().all(|event| !matches!(
            event,
            Mc2DispatchEvent::Error {
                source_slot: 112,
                ..
            }
        )));
        let source = &session.economy.entities[112];
        let target_slot = source.source_word_96();
        if target_slot == 0 {
            continue;
        }
        if selection_turn.is_none() {
            // Newly live death-sphere producers can select state 6 first.
            // Keep the original positive family-0 cast and 300-turn bound;
            // start pursuit observations only on the genuine class-5 route.
            if session.nyphur_dispatch_state_byte() != 13 {
                continue;
            }
            let target = &session.economy.entities[usize::from(target_slot)];
            assert_eq!(target.class, 5);
            assert!(matches!(target.model, 0 | 1));
            assert_eq!(session.nyphur_dispatch_state_byte(), 13);
            selection_turn = Some(turn);
        }
        let target = &session.economy.entities[usize::from(target_slot)];
        let distance = level2_wrapping_target_distance(source.position, target.position);
        if distance <= 3_328 {
            crossed_3328.get_or_insert(turn);
        }
        let common = session
            .nyphur_common_upkeep()
            .expect("ordinary Nyphur common carrier remains valid");
        if common.spell_enabled[0] != 0 {
            activation = Some((turn, distance, burst_before));
            break;
        }
    }
    let selection_turn = selection_turn.expect("ordinary selector chooses a class-5 target");
    let crossed_3328 = crossed_3328.expect("state-13 pursuit naturally crosses 3328");
    let (activation_turn, activation_distance, burst_before) =
        activation.expect("state-13 family-0 activation arms the source cooldown");
    assert!(selection_turn < crossed_3328 && crossed_3328 <= activation_turn);
    assert!(activation_distance <= 3_328);
    assert!(
        activation_distance > 2_048,
        "the state-13 activation fires outside the mode-7 gate, so the two \
         thresholds stay observably distinct"
    );

    // The activation trace is the ordinary state-13 wrapper; no mode-7 or
    // test-only state write participates.
    assert!(session.nyphur_dispatch_traces().iter().any(|trace| {
        trace.source_slot == 112
            && trace.state_byte == 13
            && trace.calls.contains(&Level2NyphurDispatchCall::Helper(
                Level2NyphurDispatchHelper::Sub13850,
            ))
    }));
    assert!(
        session
            .nyphur_dispatch_traces()
            .iter()
            .all(|trace| trace.state_byte != 7)
    );

    let common = session
        .nyphur_common_upkeep()
        .expect("activation leaves the common carrier valid");
    let tier = u8::try_from(common.spell_levels[0]).expect("authored family-0 tier");
    let fireball_profile = level2_direct_starting_spell_profile(0, tier)
        .expect("qualified Nyphur family-0 holder uses the authored source tier");
    assert_eq!(
        common.spell_enabled[0], LEVEL2_NYPHUR_SPELL_COOLDOWNS[0],
        "sub_14E10 installs the exact family-0 cooldown"
    );
    assert_eq!(
        common.word_0x1a2,
        if burst_before == 7 {
            -28
        } else {
            burst_before + 1
        },
        "the case-0 branch advances the source burst counter exactly once"
    );
    let source = &session.economy.entities[112];
    let target = &session.economy.entities[usize::from(source.source_word_96())];
    let horizontal = crate::legacy_sqrt(super::super::level2_wrapping_xy_distance(
        source.position,
        target.position,
    ));
    let expected_pitch = crate::pitch_for_delta(
        i32::try_from(horizontal).expect("family-0 XY distance fits signed 32 bits"),
        i32::from(target.position.z) - i32::from(source.position.z),
    );
    assert_eq!(
        source.pitch, expected_pitch,
        "the case-0 branch aims the caster at the stored target"
    );
    assert_eq!(
        session.economy.entities[113].active_value,
        fireball_profile.word_0x30.cast_signed().wrapping_sub(1),
        "the same-pass holder callback consumes exactly one activation tick"
    );
    assert_eq!(
        session.economy.entities[112].mana_regen, -fireball_profile.max_mana,
        "fresh sub_693F0/sub_68DE0 charges the exact family-0 mana cost"
    );

    let projectile = session
        .economy()
        .fireball_projectiles()
        .iter()
        .find(|projectile| projectile.holder_entity_index == LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX)
        .copied()
        .expect("the genuine holder-113 callback creates the family-0 projectile");
    let projectile_generation = session
        .economy()
        .slot_generation(projectile.source_entity_index);
    assert!(projectile.active);
    assert_eq!(
        projectile.phase,
        (projectile.source_entity_index as u8).wrapping_add(1),
        "the scheduled class-9 consumer advances its NewEvent slot-seeded byte 0x3e once on its own pass"
    );
    assert_eq!(projectile.owner_entity_index, 112);
    assert_eq!(
        session
            .shared_spells()
            .activation_events()
            .iter()
            .filter(|event| matches!(
                event,
                crate::FlightEvent::PrepareSound {
                    entity_index,
                    player_index: -1,
                    sound_id: 9,
                    ..
                } if *entity_index == 112
            ))
            .count(),
        1,
        "the successful source allocation emits exactly one launch sound bound to its parent"
    );
    let schedule_receipts = session.cf10_production_schedule_receipts();
    let callback_position = |slot| {
        schedule_receipts.iter().position(|receipt| {
            matches!(
                receipt,
                crate::campaign::schedule::ScheduleReceipt::CallbackCompleted {
                    slot: callback_slot,
                    ..
                } if *callback_slot == slot
            )
        })
    };
    let nyphur_callback =
        callback_position(112).expect("the activation pass dispatches the Nyphur first");
    let holder_callback = callback_position(LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX)
        .expect("the activation pass dispatches holder 113");
    let projectile_callback = callback_position(projectile.source_entity_index)
        .expect("the activation pass dispatches the higher-slot projectile");
    assert!(nyphur_callback < holder_callback && holder_callback < projectile_callback);
    assert!(
        schedule_receipts.iter().any(|receipt| matches!(
            receipt,
            crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted { slot, .. }
                if *slot == projectile.source_entity_index
        )),
        "the new projectile callback must complete its source epilogue"
    );

    // The following ordinary visit consumes the deferred holder charge and
    // continues the burst: eight case-0 casts advance `word_0x1A2` to the
    // pinned reset value, after which the state-13 body is locked out until
    // the common callback recovers the counter.
    let mana_before_charge = common.mana;
    assert_eq!(
        common.mana_regen, -fireball_profile.max_mana,
        "the deferred cost is published to the authoritative AI carrier"
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    replay.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.campaign_state_hash(), replay.campaign_state_hash());
    let following = session
        .nyphur_common_upkeep()
        .expect("following Nyphur common carrier remains valid");
    assert_eq!(
        following.mana,
        (mana_before_charge - fireball_profile.max_mana).clamp(0, common.max_mana),
        "the deferred family-0 charge lands on the following Nyphur visit"
    );
    let mut lockout = None;
    for _ in 0..200_u16 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        replay.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(session.campaign_state_hash(), replay.campaign_state_hash());
        let common = session
            .nyphur_common_upkeep()
            .expect("burst Nyphur common carrier remains valid");
        if common.word_0x1a2 < 0 {
            lockout = Some(common.word_0x1a2);
            break;
        }
    }
    assert_eq!(
        lockout,
        Some(-28),
        "eight case-0 casts reset word_0x1A2 to the pinned reflex expression"
    );

    // The tracked first projectile reaches its source terminal callback and
    // the following scheduler prologue reclaims the generation.
    let mut terminal = false;
    for _ in 0..200_u16 {
        let live = session
            .economy()
            .fireball_projectiles()
            .iter()
            .any(|candidate| {
                candidate.source_entity_index == projectile.source_entity_index
                    && session
                        .economy()
                        .slot_generation(candidate.source_entity_index)
                        == projectile_generation
                    && candidate.active
            });
        if !live {
            terminal = true;
            break;
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        replay.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(session.campaign_state_hash(), replay.campaign_state_hash());
    }
    assert!(
        terminal,
        "the tracked family-0 flyer reaches its terminal callback"
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    replay.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.campaign_state_hash(), replay.campaign_state_hash());

    session.reset();
    replay.reset();
    assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);
    assert_eq!(replay.campaign_state_hash(), baseline_campaign_hash);
}

/// Mode-7 body evidence on the production carriers. The target relation,
/// pursuit movement, cadence, holder, projectile, and cleanup are all the
/// ordinary machinery; only the dispatcher state byte is written, exactly
/// like the dispatched mode-1/2 coverage above, because the pinned state-7
/// selector `sub_13E40` requires an admitted enemy Castle that the
/// qualified fresh package cannot supply (see the sub_13E40 test below).
/// This is deliberately not an ordinary-selection claim for mode 7.
#[test]
fn level_two_nyphur_mode7_cadence_gate_and_family0_cast_are_exact() {
    let mut session = level_two_campaign_session();
    let baseline_campaign_hash = session.campaign_state_hash();
    let (_, mut target_index) = advance_to_class5_target(&mut session);

    // Let the ordinary selector and pursuit establish the genuine target
    // relation and bring the source into the (2048, 3584] band without any
    // position write.
    for _ in 0..300 {
        target_index = usize::from(session.economy.entities[112].source_word_96());
        if level2_wrapping_target_distance(
            session.economy.entities[112].position,
            session.economy.entities[target_index].position,
        ) <= 3_584
        {
            break;
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert!(level2_nyphur_source_identity_matches(
        &session.economy.entities[112],
        &session.economy.entities[target_index]
    ));
    let start_distance = level2_wrapping_target_distance(
        session.economy.entities[112].position,
        session.economy.entities[target_index].position,
    );
    assert!(start_distance > 2_048 && start_distance <= 3_584);
    assert_eq!(
        session
            .nyphur_common_upkeep()
            .expect("pre-mode-7 common carrier")
            .spell_enabled[0],
        0,
        "no family-0 cast has happened before the mode-7 takeover"
    );
    let mut replay = session.clone();

    let mut crossed_2048 = None;
    let mut near_visits_without_cast = 0_u16;
    let mut activation = None;
    let mut saw_tail_retarget = false;
    for turn in 1..=120_u16 {
        session.session.state.live.nyphur_dispatcher.state_byte = 7;
        replay.session.state.live.nyphur_dispatcher.state_byte = 7;
        let source_before = &session.economy.entities[112];
        let visit_target = source_before.source_word_96();
        let visit_handle = session.source_arena().handle(visit_target)
            .expect("the source target exists before this visit");
        let phase_before = session.economy.entities[112].phase;
        let burst_before = session.nyphur_common_upkeep().unwrap().word_0x1a2;
        session.step_outer_turn(PlayerCommand::default(), 0);
        replay.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(session.campaign_state_hash(), replay.campaign_state_hash());
        let trace = session
            .nyphur_dispatch_traces()
            .iter()
            .rev()
            .find(|trace| trace.source_slot == 112)
            .cloned()
            .expect("mode-7 source trace");
        assert_eq!(trace.state_byte, 7);
        assert!(trace.calls.contains(&Level2NyphurDispatchCall::Helper(
            Level2NyphurDispatchHelper::Sub13710,
        )));
        assert!(session.dispatch_events().iter().all(|event| !matches!(
            event,
            Mc2DispatchEvent::Error {
                source_slot: 112,
                ..
            }
        )));
        let source = &session.economy.entities[112];
        saw_tail_retarget |= source.source_word_96() != visit_target;
        // The mode callback reads word96 before its following 12E70 tail.
        // Measure the same generation only when that relation survived the
        // visit; a later selector can replace it before this observation.
        if source.source_word_96() != visit_target
            || session.source_arena().handle(visit_target) != Some(visit_handle)
        {
            continue;
        }
        let target = &session.economy.entities[usize::from(visit_target)];
        assert!(level2_nyphur_source_identity_matches(source, target));
        let distance = level2_wrapping_target_distance(
            source.position,
            target.position,
        );
        if distance <= 2_048 {
            crossed_2048.get_or_insert(turn);
        }
        let common = session
            .nyphur_common_upkeep()
            .expect("mode-7 common carrier remains valid");
        if common.spell_enabled[0] != 0 {
            activation = Some((turn, distance, phase_before, burst_before));
            break;
        }
        if distance <= 2_048 {
            // The target may leave and re-enter range while the source
            // eases its speed. Only a near visit tests this branch.
            near_visits_without_cast += 1;
            assert_eq!(common.speed, 0, "the near branch parks the source");
            assert_eq!(common.word_0xe_14, 1);
        }
    }
    assert!(saw_tail_retarget, "the source tail replaces the initial target during this scenario");
    let crossed_2048 = crossed_2048.expect("mode-7 pursuit crosses the 2048 gate");
    let (activation_turn, activation_distance, activation_phase, burst_before) =
        activation.expect("the mode-7 cadence tick activates the family-0 holder");
    assert!(crossed_2048 <= activation_turn);
    assert!(activation_distance <= 2_048, "the pinned mode-7 receipt");
    assert!(
        near_visits_without_cast > 0,
        "mode 7 must refuse the cast between cadence ticks"
    );
    let period = u16::from(activation_phase);
    assert_eq!(
        period % 55,
        0,
        "the cast lands exactly on the reflex cadence tick"
    );

    let common = session
        .nyphur_common_upkeep()
        .expect("post-cast common carrier");
    let tier = u8::try_from(common.spell_levels[0]).expect("authored family-0 tier");
    let fireball_profile = level2_direct_starting_spell_profile(0, tier)
        .expect("qualified Nyphur family-0 tier profile");
    assert_eq!(common.spell_enabled[0], LEVEL2_NYPHUR_SPELL_COOLDOWNS[0]);
    assert_eq!(
        common.word_0x1a2,
        if burst_before == 7 {
            -28
        } else {
            burst_before + 1
        }
    );
    assert_eq!(
        session.economy.entities[113].active_value,
        fireball_profile.word_0x30.cast_signed().wrapping_sub(1)
    );
    let projectile = session
        .economy()
        .fireball_projectiles()
        .iter()
        .find(|projectile| projectile.holder_entity_index == LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX)
        .copied()
        .expect("the mode-7 cast reaches the genuine holder-113 producer");
    assert!(projectile.active);
    // NewEvent seeds byte3e from the slot. UpdateEntities scans numeric
    // addresses: a birth below holder113 waits for the following pass.
    let same_pass = projectile.source_entity_index > LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX;
    assert_eq!(
        projectile.phase,
        (projectile.source_entity_index as u8).wrapping_add(u8::from(same_pass)),
        "the phase reflects whether the numeric scan reached the newborn slot"
    );
    let callback_seen = session.dispatch_events().iter().any(|event| matches!(event,
        Mc2DispatchEvent::DynamicArena { source_slot, kind: super::super::Level2DynamicArenaKind::FireballProjectile, .. }
            if *source_slot == projectile.source_entity_index));
    assert_eq!(callback_seen, same_pass, "newborn Fireball dispatch follows source slot order");
    assert_eq!(
        session
            .shared_spells()
            .activation_events()
            .iter()
            .filter(|event| matches!(
                event,
                crate::FlightEvent::PrepareSound {
                    entity_index,
                    player_index: -1,
                    sound_id: 9,
                    ..
                } if *entity_index == 112
            ))
            .count(),
        1
    );

    session.reset();
    replay.reset();
    assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);
    assert_eq!(replay.campaign_state_hash(), baseline_campaign_hash);

    // Direct body vectors discriminate the branches that the dispatched
    // flow above cannot isolate. They reuse the ordinary class-5 relation
    // and rewrite only detached geometry/phase carriers.
    let mut related = level_two_campaign_session();
    let (_, target_index) = advance_to_class5_target(&mut related);
    let source_position = Position {
        x: 10_000,
        y: 10_000,
        z: 2_048,
    };

    // A cadence miss inside the near gate refuses the cast without the
    // altitude write and without any spell-table draw.
    let mut cadence_miss = related.clone();
    cadence_miss.session.state.live.economy.entities[112].set_position(source_position);
    cadence_miss.session.state.live.economy.entities[112].phase = 1;
    cadence_miss.session.state.live.economy.entities[target_index].set_position(Position {
        x: source_position.x + 2_048,
        ..source_position
    });
    let cadence_terrain = cadence_miss.canonical_terrain().clone();
    cadence_miss
        .session
        .state
        .live
        .nyphur_dispatcher
        .step_sub_13710(
            112,
            &mut cadence_miss.session.state.live.economy.entities,
            &cadence_terrain,
            &cadence_miss.session.definition.presentation,
        )
        .expect("cadence-miss mode-7 visit");
    assert_eq!(
        cadence_miss.nyphur_dispatcher.common_upkeep.spell_enabled[0], 0,
        "no cast off the reflex cadence"
    );
    assert_eq!(cadence_miss.nyphur_dispatcher.common_upkeep.speed, 0);
    assert_eq!(cadence_miss.nyphur_dispatcher.common_upkeep.word_0xe_14, 1);
    assert_eq!(
        cadence_miss.economy.entities[112].position.z, source_position.z,
        "no altitude write on a cadence miss"
    );

    // The inclusive 2048 cadence visit casts; the successful branch skips
    // the altitude write entirely.
    let mut inclusive_cast = related.clone();
    let inclusive_target_position = Position {
        x: source_position.x + 2_048,
        ..source_position
    };
    inclusive_cast.session.state.live.economy.entities[112].set_position(source_position);
    inclusive_cast.session.state.live.economy.entities[112].phase = 0;
    inclusive_cast.session.state.live.economy.entities[112].set_yaw(
        i16::try_from(crate::heading_for_positions(
            source_position,
            inclusive_target_position,
        ))
        .expect("mode-7 vector heading fits the signed angle carrier"),
    );
    inclusive_cast.session.state.live.economy.entities[target_index]
        .set_position(inclusive_target_position);
    let inclusive_terrain = inclusive_cast.canonical_terrain().clone();
    // The ordinary prelude can arrive with a negative burst counter after
    // earlier casts. Source 14E10 refuses while it is negative, even at the
    // inclusive distance and cadence; prove that before testing the boundary
    // with an explicitly cold burst counter.
    let mut locked_cast = inclusive_cast.clone();
    let locked_before = locked_cast.nyphur_dispatcher.common_upkeep.word_0x1a2;
    assert!(locked_before < 0, "the observed prelude carries burst lockout");
    locked_cast.session.state.live.nyphur_dispatcher.step_sub_13710(
        112, &mut locked_cast.session.state.live.economy.entities,
        &inclusive_terrain, &locked_cast.session.definition.presentation,
    ).expect("locked mode-7 cadence visit");
    assert_eq!(locked_cast.nyphur_dispatcher.common_upkeep.spell_enabled[0], 0);
    assert_eq!(locked_cast.nyphur_dispatcher.common_upkeep.word_0x1a2, locked_before);
    inclusive_cast.nyphur_dispatcher.common_upkeep.word_0x1a2 = 0;
    let prior_burst = inclusive_cast.nyphur_dispatcher.common_upkeep.word_0x1a2;
    inclusive_cast
        .session
        .state
        .live
        .nyphur_dispatcher
        .step_sub_13710(
            112,
            &mut inclusive_cast.session.state.live.economy.entities,
            &inclusive_terrain,
            &inclusive_cast.session.definition.presentation,
        )
        .expect("inclusive mode-7 cadence cast");
    assert_eq!(
        inclusive_cast.nyphur_dispatcher.common_upkeep.spell_enabled[0],
        LEVEL2_NYPHUR_SPELL_COOLDOWNS[0],
        "the inclusive 2048 boundary reaches the family-0 activation"
    );
    assert_eq!(
        inclusive_cast.economy.entities[112].position.z, source_position.z,
        "a successful cast skips the altitude branch"
    );
    assert_eq!(
        inclusive_cast.nyphur_dispatcher.common_upkeep.word_0x1a2,
        if prior_burst == 7 {
            -28
        } else {
            prior_burst + 1
        }
    );

    // The exclusive 2049 cadence visit stays in the approach branch: no
    // cast, minimum-speed pursuit.
    let mut exclusive = related.clone();
    exclusive.session.state.live.economy.entities[112].set_position(source_position);
    exclusive.session.state.live.economy.entities[112].phase = 0;
    exclusive.session.state.live.economy.entities[target_index].set_position(Position {
        x: source_position.x + 2_049,
        ..source_position
    });
    let exclusive_terrain = exclusive.canonical_terrain().clone();
    exclusive
        .session
        .state
        .live
        .nyphur_dispatcher
        .step_sub_13710(
            112,
            &mut exclusive.session.state.live.economy.entities,
            &exclusive_terrain,
            &exclusive.session.definition.presentation,
        )
        .expect("exclusive mode-7 approach");
    assert_eq!(
        exclusive.nyphur_dispatcher.common_upkeep.spell_enabled[0],
        0
    );
    assert_eq!(
        exclusive.nyphur_dispatcher.common_upkeep.speed,
        exclusive.economy.entities[112].minimum_speed()
    );

    // A starved mana latch fails the sub_15910 offer on the cadence tick;
    // the failed offer descends toward the target altitude by the
    // parameter word-14 step with the requested speed zeroed.
    let mut starved = related.clone();
    starved.session.state.live.economy.entities[112].set_position(source_position);
    starved.session.state.live.economy.entities[112].phase = 0;
    starved.session.state.live.economy.entities[target_index].set_position(Position {
        x: source_position.x + 2_048,
        ..source_position
    });
    starved
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .word_0x1a4 = 1;
    starved
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .mana = 0;
    let parameter_index = starved.session.state.live.economy.entities[112]
        .parameter_relation
        .expect("Nyphur parameter relation");
    let vertical_step = i16::from_le_bytes([
        starved.session.definition.presentation.entity_parameters[usize::from(parameter_index)][14],
        starved.session.definition.presentation.entity_parameters[usize::from(parameter_index)][15],
    ]);
    let starved_terrain = starved.canonical_terrain().clone();
    starved
        .session
        .state
        .live
        .nyphur_dispatcher
        .step_sub_13710(
            112,
            &mut starved.session.state.live.economy.entities,
            &starved_terrain,
            &starved.session.definition.presentation,
        )
        .expect("starved mode-7 altitude branch");
    assert_eq!(starved.nyphur_dispatcher.common_upkeep.spell_enabled[0], 0);
    assert_eq!(starved.nyphur_dispatcher.common_upkeep.speed, 0);
    let expected_direction = i32::from(source_position.z)
        .wrapping_sub(i32::from(source_position.z) + 512)
        .signum() as i16;
    assert_eq!(
        starved.economy.entities[112].position.z,
        source_position
            .z
            .wrapping_add(expected_direction.wrapping_mul(vertical_step)),
        "the failed offer descends by the pinned parameter step"
    );

    // A stale stored identity is the source zero result: the roll is
    // preserved and no gate or cast write happens.
    let mut stale = related.clone();
    stale.session.state.live.economy.entities[112].set_position(source_position);
    stale.session.state.live.economy.entities[112].phase = 0;
    stale.session.state.live.economy.entities[target_index].set_position(Position {
        x: source_position.x + 2_048,
        ..source_position
    });
    let preserved_roll = stale.session.state.live.economy.entities[112].source_roll();
    let stale_identity =
        level2_nyphur_source_identity(&stale.session.state.live.economy.entities[target_index])
            .wrapping_add(1);
    stale.session.state.live.economy.entities[112].record[156..158]
        .copy_from_slice(&stale_identity.to_le_bytes());
    let stale_terrain = stale.canonical_terrain().clone();
    stale
        .session
        .state
        .live
        .nyphur_dispatcher
        .step_sub_13710(
            112,
            &mut stale.session.state.live.economy.entities,
            &stale_terrain,
            &stale.session.definition.presentation,
        )
        .expect("stale identity is a source zero result");
    assert_eq!(stale.economy.entities[112].source_roll(), preserved_roll);
    assert_eq!(stale.nyphur_dispatcher.common_upkeep.spell_enabled[0], 0);
}

/// Pinned `sub_13E40` evidence around a genuinely built player Castle.
/// The Castle arrives through the public spell route only; the ordinary
/// sessions below never write a selector, relation, or state byte. The
/// authored package cannot arm the owner-threat clause (the threshold word
/// is capped at 0x601F while the clause needs more than
/// `50000 - 243 * (1000 / 10) / 255 = 49905`), so the ordinary route must
/// refuse mode 7 with zero drift, and the armed positive below plants only
/// the two threshold/latch words that pinned `sub_159E0` itself writes.
#[test]
fn level_two_nyphur_sub_13e40_castle_selection_boundaries_are_exact() {
    let gameplay_cache = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level2-day-owner-cache/components/gameplay");
    let inspection =
        NativeWorld::inspect_level2_with_owner_cache(level_two_fixture_path(), &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
    let mut session = inspection
        .level2_reset_session()
        .expect("qualified Level 2 Castle selection session");
    session.select_level2_campaign_entry_stage();
    let baseline_campaign_hash = session.campaign_state_hash();

    // Public castle build on the qualified route: position input plus the
    // selected-hand Castle cast, then ordinary steps until the flyer roots.
    let launch = Position {
        x: 36_500,
        y: 12_500,
        z: session
            .canonical_terrain()
            .ground_height(Position {
                x: 36_500,
                y: 12_500,
                z: 0,
            })
            .saturating_add(300),
    };
    session.state.position = launch;
    session.state.yaw = 0;
    session.state.actual_speed = 0;
    session.state.target_speed = 0;
    session.state.entity_fov = 0;
    session.step_outer_turn(
        PlayerCommand {
            opcode: 6,
            argument_1: 2,
            flags: 0x40,
            ..PlayerCommand::default()
        },
        0,
    );
    let mut castle = None;
    for _ in 0..48_u16 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        castle = session.economy().castle().filter(|castle| castle.active);
        if castle.is_some() {
            break;
        }
    }
    let castle = castle.expect("the public spell route roots the Castle");
    let castle_slot = castle.source_entity_index;
    assert_eq!((castle.source_class, castle.source_model), (3, 2));
    assert_eq!(castle.source_id_entity_index, 111);

    // Ordinary refusal: with the live Castle projected into the class-3
    // list, the production tail visits sub_13E40 on every cadence tick and
    // declines it, because the authored threat words cannot cross the
    // clause. The state-13 pursuit continues exactly as without a Castle.
    let mut refusal = session.clone();
    for _ in 0..200_u16 {
        refusal.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert!(
        refusal
            .nyphur_dispatch_traces()
            .iter()
            .all(|trace| trace.state_byte != 7),
        "the qualified package cannot select mode 7 ordinarily"
    );
    assert_ne!(
        refusal.economy.entities[112].source_word_96(),
        castle_slot,
        "the ordinary selector never claims the Castle"
    );
    assert!(refusal.dispatch_events().iter().all(|event| !matches!(
        event,
        Mc2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));
    assert!(
        !refusal.economy.entities[usize::from(castle_slot)].active,
        "the Castle projection never leaks into the persistent entity arena"
    );
    let refusal_common = refusal
        .nyphur_common_upkeep()
        .expect("refusal common carrier");
    assert_eq!(
        refusal_common.array_0x1fc[4], 24_607,
        "the owner threshold word stays at the pinned 0x601F cap"
    );

    // A live foreign row on the Castle slot is a projection collision and
    // fails the dispatch closed instead of masking either authority.
    let mut collided = session.clone();
    collided.session.state.live.economy.entities[usize::from(castle_slot)].active = true;
    collided.step_outer_turn(PlayerCommand::default(), 0);
    assert!(collided.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::Error {
            source_slot: 112,
            ..
        }
    )));

    // Direct admission vectors over the projected view. These rewrite only
    // detached view geometry/threshold carriers to discriminate each pinned
    // clause; they make no ordinary-selection claim.
    let players = session.players().to_vec();
    let presentation = session.session.definition.presentation.clone();
    let dispatcher = session.session.state.live.nyphur_dispatcher.clone();
    let build_view = |owner_position: Position, nyphur_position: Position| {
        let mut view = session.session.state.live.economy.entities.clone();
        level2_nyphur_project_castle_root(&mut view[usize::from(castle_slot)], castle);
        view[111].set_position(owner_position);
        view[112].set_position(nyphur_position);
        view
    };
    let far_owner = Position {
        x: castle.position.x.wrapping_add(8_000),
        y: castle.position.y,
        z: castle.position.z,
    };
    let near_nyphur = Position {
        x: castle.position.x.wrapping_add(4_000),
        y: castle.position.y,
        z: castle.position.z,
    };
    let threat_lhs = 50_000_i32 - 243 * (1_000 / 10) / 255;
    assert_eq!(threat_lhs, 49_905);

    // The threat clause is strict: the exact left-hand side refuses, one
    // more admits (owner far, no overlap, inside perception).
    let view = build_view(far_owner, near_nyphur);
    let mut armed = dispatcher.clone();
    armed.common_upkeep.array_0x1fc[4] = 49_905;
    assert_eq!(
        armed
            .step_sub_13e40(112, &view, &players, &presentation)
            .expect("threat-boundary selector"),
        None
    );
    armed.common_upkeep.array_0x1fc[4] = 49_906;
    assert_eq!(
        armed
            .step_sub_13e40(112, &view, &players, &presentation)
            .expect("armed threat selector"),
        Some(castle_slot)
    );

    // sub_13E40 multiplies at source 32-bit width before signed division.
    // These detached owner-resource inputs discriminate wrapping from an
    // unbounded or saturating product while retaining the real selector,
    // owner geometry, player color, and Castle produced above.
    let mut high_mana = build_view(far_owner, near_nyphur);
    let mut high_threat = dispatcher.clone();
    high_threat.common_upkeep.array_0x1fc[4] = u16::MAX;
    high_mana[111].max_mana = 300_000_000;
    // 243 * (300_000_000 / 10) wraps to -1_299_934_592, giving
    // 50_000 - (-1_299_934_592 / 255) = 5_147_782, above any u16 threshold.
    assert_eq!(
        high_threat.step_sub_13e40(112, &high_mana, &players, &presentation)
            .expect("wrapped high-mana Castle selector"),
        None
    );
    high_mana[111].max_mana = 400_000_000;
    // At 400M the wrapped product is positive, so the same actual selector
    // admits the Castle. Widening/saturation produce different decisions.
    assert_eq!(
        high_threat.step_sub_13e40(112, &high_mana, &players, &presentation)
            .expect("wrapped positive-product Castle selector"),
        Some(castle_slot)
    );

    // Entry gates: no offensive holder, and a family-2 holder while the AI
    // still has no Castle of its own, both refuse before any scan.
    let mut holderless = armed.clone();
    holderless.common_upkeep.spell_holder_indices = [0; PLAYER_SPELL_COUNT];
    assert_eq!(
        holderless
            .step_sub_13e40(112, &view, &players, &presentation)
            .expect("holderless selector"),
        None
    );
    let mut family_two = armed.clone();
    family_two.common_upkeep.spell_holder_indices[2] = 113;
    assert_eq!(
        family_two
            .step_sub_13e40(112, &view, &players, &presentation)
            .expect("family-2 gated selector"),
        None
    );

    // The owner-distance clause is strictly greater than 0x3840000.
    let exact_owner = Position {
        x: castle.position.x.wrapping_add(7_680),
        y: castle.position.y,
        z: castle.position.z,
    };
    let view_exact = build_view(exact_owner, near_nyphur);
    assert_eq!(
        armed
            .step_sub_13e40(112, &view_exact, &players, &presentation)
            .expect("owner-distance boundary selector"),
        None
    );
    let past_owner = Position {
        x: castle.position.x.wrapping_add(7_681),
        y: castle.position.y,
        z: castle.position.z,
    };
    let view_past = build_view(past_owner, near_nyphur);
    assert_eq!(
        armed
            .step_sub_13e40(112, &view_past, &players, &presentation)
            .expect("owner-distance admitted selector"),
        Some(castle_slot)
    );

    // An owner standing inside the Castle footprint vetoes the threat
    // clause through the pinned sub_106C0 overlap.
    let view_overlap = build_view(castle.position, near_nyphur);
    assert_eq!(
        armed
            .step_sub_13e40(112, &view_overlap, &players, &presentation)
            .expect("owner-overlap selector"),
        None
    );

    // The own-castle mana clause admits without any threat word: the AI
    // Castle row must bank strictly more than 640 * (255 - 243) plus the
    // target's mana.
    let mut mana_clause = dispatcher.clone();
    mana_clause.common_upkeep.castle_entity_index = 70;
    let mut view_mana = build_view(castle.position, near_nyphur);
    view_mana[70].mana = 7_680;
    assert_eq!(
        mana_clause
            .step_sub_13e40(112, &view_mana, &players, &presentation)
            .expect("own-castle mana boundary selector"),
        None
    );
    view_mana[70].mana = 7_681;
    assert_eq!(
        mana_clause
            .step_sub_13e40(112, &view_mana, &players, &presentation)
            .expect("own-castle mana admitted selector"),
        Some(castle_slot)
    );

    // The perception admission is squared parameter word 0x1C and rejects
    // at equality.
    let mut view_perception = build_view(far_owner, near_nyphur);
    view_perception[112].set_position(Position {
        x: castle.position.x.wrapping_add(8_192),
        y: castle.position.y,
        z: castle.position.z,
    });
    assert_eq!(
        armed
            .step_sub_13e40(112, &view_perception, &players, &presentation)
            .expect("perception boundary selector"),
        None
    );
    view_perception[112].set_position(Position {
        x: castle.position.x.wrapping_add(8_191),
        y: castle.position.y,
        z: castle.position.z,
    });
    assert_eq!(
        armed
            .step_sub_13e40(112, &view_perception, &players, &presentation)
            .expect("perception admitted selector"),
        Some(castle_slot)
    );

    // Armed production flow: plant exactly the two words pinned sub_159E0
    // writes (the color-0 threshold and its war latch), move the owner away
    // through the public position input, and let the production tail latch
    // mode 7 and pursue the Castle through the exact 2048 receipt. This is
    // boundary evidence for the selector seam: the arming itself is not an
    // ordinary route, and is labelled as such.
    let mut armed_session = session.clone();
    for _ in 0..208_u16 {
        armed_session.step_outer_turn(PlayerCommand::default(), 0);
    }
    let far_position = Position {
        x: 36_500,
        y: 25_000,
        z: 1_000,
    };
    let local_player = armed_session.state.entity_index;
    armed_session
        .set_level2_entity_position(local_player, far_position)
        .expect("public player input moves the owner away from the Castle");
    armed_session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .array_0x1fc[4] = 55_000;
    armed_session
        .session
        .state
        .live
        .nyphur_dispatcher
        .common_upkeep
        .array_0x1fc[5] = 1;
    let mut selected_turn = None;
    let mut activation = None;
    for turn in 209..=320_u16 {
        armed_session.step_outer_turn(PlayerCommand::default(), 0);
        let source = &armed_session.economy.entities[112];
        if selected_turn.is_none() && source.source_word_96() == castle_slot {
            assert_eq!(armed_session.nyphur_dispatch_state_byte(), 7);
            assert_eq!(
                source.source_word_98(),
                level2_nyphur_source_identity_parts(111, 2, 3),
                "sub_13E40 stores the sub_14C40 Castle identity"
            );
            selected_turn = Some(turn);
        }
        if selected_turn.is_some() {
            let common = armed_session
                .nyphur_common_upkeep()
                .expect("armed common carrier");
            let castle_row = armed_session
                .economy()
                .castle()
                .expect("the pursued Castle stays live");
            let distance = level2_wrapping_target_distance(
                armed_session.economy.entities[112].position,
                castle_row.position,
            );
            if common.spell_enabled[0] != 0 && distance <= 2_048 {
                activation = Some((turn, distance));
                break;
            }
        }
    }
    let selected_turn = selected_turn.expect("the armed threshold latches mode 7");
    let (activation_turn, activation_distance) =
        activation.expect("mode 7 pursues the Castle through the 2048 receipt");
    assert!(selected_turn < activation_turn);
    assert!(activation_distance <= 2_048);
    assert!(armed_session.nyphur_dispatch_traces().iter().any(|trace| {
        trace.source_slot == 112
            && trace.state_byte == 7
            && trace.calls.contains(&Level2NyphurDispatchCall::Helper(
                Level2NyphurDispatchHelper::Sub13710,
            ))
    }));
    assert!(
        armed_session
            .economy()
            .fireball_projectiles()
            .iter()
            .any(|projectile| {
                projectile.active
                    && projectile.holder_entity_index == LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX
            }),
        "the mode-7 Castle attack reaches the genuine holder-113 producer"
    );

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);
    let mut replay_refusal = refusal.clone();
    replay_refusal.reset();
    assert_eq!(replay_refusal.campaign_state_hash(), baseline_campaign_hash);
}
