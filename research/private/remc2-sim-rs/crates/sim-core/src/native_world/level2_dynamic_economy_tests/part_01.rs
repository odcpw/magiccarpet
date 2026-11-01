#[test]
fn ordinary_public_creature_reward_reaches_owned_castle_and_funds_spell() {
    use crate::campaign::spec::CampaignSelection;

    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-paj2-source4-campaign-package");
    let selection = CampaignSelection::ordinary(5).unwrap();
    let mut campaign = crate::campaign::CampaignSession::open(package, selection)
        .expect("ordinary Level 5 owner package");
    // Source4 starts after the opening missions and its human reset row has
    // no direct starting holders. Declare only ordinary carried progression;
    // this does not grant mana, a Castle, damage or a sphere.
    campaign.simulation_mut().apply_carried_spell_families(&[0, 1, 2], Some(0))
        .expect("declared opening-spell carry materializes the actual owner holders");
    let player = campaign.simulation().state.entity_index;
    let rival = 266_u16;
    let rival_root = campaign.simulation().economy().castle_for_owner(rival)
        .expect("the other actual owner starts with a Castle").source_entity_index;
    let rival_handle = campaign.simulation().source_arena().handle(rival_root).unwrap();
    let initial_player_max = campaign.simulation().shared_spells().player_entity().max_mana;
    let initial_rival_max = campaign.simulation().entities()[usize::from(rival)].max_mana;
    assert_eq!((initial_player_max, initial_rival_max), (1_000, 1_000));
    let goat = campaign.simulation().goats().iter().find(|goat| goat.active && goat.entity_index == 94)
        .expect("the actual ordinary reset Goat 94").clone();
    assert_eq!((goat.life, goat.mana), (600, 300));

    // Controlled travel and aim start the ordinary public Castle producer.
    // Its holder, flyer, root and construction remain source-scheduled.
    let mut launch = Position { x: goat.position.x.wrapping_add(2_304),
        y: goat.position.y.wrapping_add(4_096), z: 0 };
    launch.z = campaign.simulation().canonical_terrain().ground_height(launch).wrapping_add(512);
    campaign.simulation_mut().set_level2_entity_position(player, launch).unwrap();
    campaign.simulation_mut().set_level2_player_aim_for_test(0, 0);
    let castle_ui = crate::SOURCE_SPELL_INDEX_MAP.iter().position(|family| *family == 2).unwrap() as i8;
    campaign.step(PlayerCommand { opcode: 6, argument_1: castle_ui, flags: 0x40,
        ..PlayerCommand::default() }, 0);
    let mut saw_castle_flyer = campaign.simulation().economy().castle_projectiles().iter()
        .any(|row| row.owner_entity_index == player);
    for _ in 0..512 {
        if campaign.simulation().economy().castle_for_owner(player)
            .is_some_and(|castle| castle.action == 4 && castle.level == 1) { break; }
        campaign.step(PlayerCommand::default(), 0);
        assert!(campaign.simulation().dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
        saw_castle_flyer |= campaign.simulation().economy().castle_projectiles().iter()
            .any(|row| row.owner_entity_index == player);
    }
    assert!(saw_castle_flyer, "public Castle cast must allocate its real flyer");
    let root = *campaign.simulation().economy().castle_for_owner(player)
        .filter(|castle| castle.action == 4 && castle.level == 1)
        .expect("public Castle flyer must finish the actual owner's stage-one root");
    let root_handle = campaign.simulation().source_arena().handle(root.source_entity_index).unwrap();
    assert_eq!(campaign.simulation().source_arena().handle(rival_root), Some(rival_handle));
    eprintln!("earned-mana public Castle: owner={player} root={} position={:?} rival={rival} rival_root={rival_root}", root.source_entity_index, root.position);

    // Reposition only the real player for each public Fireball. Goat life,
    // damage, movement, phase and death are never assigned by this check.
    let goat_slot = goat.entity_index;
    let fire_ui = crate::SOURCE_SPELL_INDEX_MAP.iter().position(|family| *family == 0).unwrap() as i8;
    let fire_holder = campaign.simulation().shared_spells().local_player().enabled_spell_holders[0];
    let events_before = campaign.simulation().economy().events().len();
    let mut shots = 0;
    let mut saw_fireball = false;
    let mut saw_goat_damage = false;
    let mut saw_player_goat_attacker = false;
    let mut saw_goat_impact = false;
    let mut reward_slot = None;
    for _ in 0..16 {
        for _ in 0..64 {
            let sim = campaign.simulation();
            let holder = sim.shared_spells().pickup(fire_holder).unwrap();
            if holder.active_value == 0 && sim.shared_spells().player_entity().mana >= holder.max_mana { break; }
            campaign.step(PlayerCommand::default(), 0);
        }
        let Some(current) = campaign.simulation().goats().iter()
            .find(|row| row.entity_index == goat_slot && row.active).cloned() else { break; };
        if current.life < 0 { break; }
        let holder = campaign.simulation().shared_spells().pickup(fire_holder).unwrap();
        assert!(holder.active_value == 0 && campaign.simulation().shared_spells().player_entity().mana >= holder.max_mana,
            "the normal owner must be ready to pay for its public Fireball");
        let position = Position { x: current.position.x.wrapping_add(256),
            y: current.position.y.wrapping_add(384),
            z: current.position.z.wrapping_sub(campaign.simulation().state.entity_fov) };
        campaign.simulation_mut().set_level2_entity_position(player, position).unwrap();
        campaign.simulation_mut().set_level2_player_aim_for_test(0, 0);
        campaign.step(PlayerCommand { opcode: 6, argument_1: fire_ui, flags: 0x40,
            ..PlayerCommand::default() }, 0);
        shots += 1;
        for _ in 0..24 {
            let sim = campaign.simulation();
            saw_fireball |= sim.economy().fireball_projectiles().iter()
                .any(|row| row.owner_entity_index == player);
            saw_goat_impact |= sim.entities().iter().any(|row| row.active
                && row.class == 10 && row.model == 0 && row.source_id == player
                && row.source_word_96() == goat_slot);
            saw_goat_damage |= sim.goats().iter().find(|row| row.entity_index == goat_slot)
                .is_some_and(|row| row.life < 600);
            saw_player_goat_attacker |= sim.goats().iter().find(|row| row.entity_index == goat_slot)
                .is_some_and(|row| (row.pending_damage > 0 && row.pending_attacker == player as i16)
                    || row.current_attacker == player as i16 || row.kill_credit_entity == player as i16);
            reward_slot = sim.economy().events()[events_before..].iter().find_map(|event| match event {
                Level2EconomyEvent::ManaSphereSpawned { source_entity_index, sphere_id, mana: 300, .. }
                    if *source_entity_index == goat_slot => u16::try_from(*sphere_id).ok(),
                _ => None,
            });
            if reward_slot.is_some() || sim.goats().iter().find(|row| row.entity_index == goat_slot)
                .is_some_and(|row| row.life < 0) { break; }
            campaign.step(PlayerCommand::default(), 0);
        }
        if reward_slot.is_some() || campaign.simulation().goats().iter()
            .find(|row| row.entity_index == goat_slot).is_some_and(|row| row.life < 0) { break; }
    }
    assert!(shots > 0 && saw_fireball && saw_goat_impact && saw_goat_damage && saw_player_goat_attacker,
        "the real Goat must receive a public Fireball target impact and the actual player's damage relation");
    for _ in 0..32 {
        let sim = campaign.simulation();
        reward_slot = reward_slot.or_else(|| sim.economy().events()[events_before..].iter().find_map(|event| match event {
            Level2EconomyEvent::ManaSphereSpawned { source_entity_index, sphere_id, mana: 300, .. }
                if *source_entity_index == goat_slot => u16::try_from(*sphere_id).ok(),
            _ => None,
        }));
        if reward_slot.is_some() { break; }
        campaign.step(PlayerCommand::default(), 0);
    }
    let reward_slot = reward_slot.expect("actual Fireball damage and scheduled Goat death must produce its 300-mana sphere");
    let reward_event_start = campaign.simulation().economy().events().iter().rposition(|event| matches!(event,
        Level2EconomyEvent::ManaSphereSpawned { source_entity_index, sphere_id, mana: 300, .. }
            if *source_entity_index == goat_slot && *sphere_id == u32::from(reward_slot)))
        .expect("the death receipt bounds this sphere generation's event history");
    let reward_handle = campaign.simulation().source_arena().handle(reward_slot).unwrap();
    for _ in 0..128 {
        let sphere = campaign.simulation().economy().spheres().iter()
            .find(|row| row.source_entity_index == reward_slot && row.active && !row.pending_delete)
            .expect("real death sphere remains available for ordinary Possession");
        if sphere.velocity_x == 0 && sphere.velocity_y == 0 && sphere.vertical_velocity == 0 { break; }
        campaign.step(PlayerCommand::default(), 0);
    }
    assert_eq!(campaign.simulation().source_arena().handle(reward_slot), Some(reward_handle));
    let sphere = *campaign.simulation().economy().spheres().iter()
        .find(|row| row.source_entity_index == reward_slot && row.active && !row.pending_delete)
        .expect("real death sphere remains live");
    assert_eq!((sphere.owner_entity_index, sphere.player_entity_index, sphere.mana), (0, 0, 300));
    eprintln!("earned-mana Goat death: goat={goat_slot} shots={shots} sphere={reward_slot} position={:?} mana={} generation={reward_handle:?}", sphere.position, sphere.mana);
    let capacity_before = campaign.simulation().shared_spells().player_entity().max_mana;
    let castle_mana_before = campaign.simulation().economy().castle_for_owner(player).unwrap().mana;
    let possession_ui = crate::SOURCE_SPELL_INDEX_MAP.iter().position(|family| *family == 1).unwrap() as i8;
    let possession_holder = campaign.simulation().shared_spells().local_player().enabled_spell_holders[1];
    for _ in 0..64 {
        let sim = campaign.simulation();
        let holder = sim.shared_spells().pickup(possession_holder).unwrap();
        if holder.active_value == 0 && sim.shared_spells().player_entity().mana >= holder.max_mana { break; }
        campaign.step(PlayerCommand::default(), 0);
    }
    let sim = campaign.simulation();
    let holder = sim.shared_spells().pickup(possession_holder).unwrap();
    assert!(holder.active_value == 0 && sim.shared_spells().player_entity().mana >= holder.max_mana,
        "the real owner must afford its public Possession holder");
    let target = campaign.simulation().possession_guidance_target_top_for_test(reward_slot).unwrap();
    let mut approach = Position { x: target.x.wrapping_sub(768), y: target.y, z: target.z.wrapping_add(512) };
    approach.z = approach.z.max(campaign.simulation().canonical_terrain().ground_height(approach).wrapping_add(256));
    campaign.simulation_mut().set_level2_entity_position(player, approach).unwrap();
    let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(approach, target);
    campaign.simulation_mut().set_level2_player_aim_for_test(yaw, pitch);
    campaign.step(PlayerCommand { opcode: 6, argument_1: possession_ui, flags: 0x40,
        ..PlayerCommand::default() }, 0);
    let mut saw_possession_flyer = false;
    let mut saw_possession_effect = false;
    let mut saw_claim = false;
    let mut possession_flyer_handle = None;
    let mut possession_effect_handle = None;
    for _ in 0..64 {
        let sim = campaign.simulation();
        if let Some(row) = sim.economy().possession_projectiles().iter().find(|row| row.source_id_entity_index == player) {
            saw_possession_flyer = true;
            possession_flyer_handle = possession_flyer_handle.or_else(|| sim.source_arena().handle(row.source_entity_index));
        }
        if let Some(row) = sim.economy().possession_effects().iter().find(|row| row.source_id_entity_index == player
            && possession_flyer_handle.is_some_and(|handle| handle.source_slot == row.projectile_entity_index)) {
            saw_possession_effect = true;
            possession_effect_handle = possession_effect_handle.or_else(|| sim.source_arena().handle(row.source_entity_index));
        }
        saw_claim |= sim.economy().events()[events_before..].iter().any(|event| matches!(event,
            Level2EconomyEvent::ManaSphereClaimed { sphere_id, owner_entity_index }
                if *sphere_id == u32::from(reward_slot) && *owner_entity_index == player));
        if saw_claim { break; }
        campaign.step(PlayerCommand::default(), 0);
    }
    assert!(saw_possession_flyer && saw_possession_effect && saw_claim,
        "the real public Possession flyer/effect must claim this exact death sphere");
    let possession_flyer_handle = possession_flyer_handle.expect("actual Possession flyer generation");
    let possession_effect_handle = possession_effect_handle.expect("actual Possession effect generation");
    assert_eq!(campaign.simulation().source_arena().handle(reward_slot), Some(reward_handle));
    assert!(campaign.simulation().economy().spheres().iter().any(|row| {
        row.source_entity_index == reward_slot && row.active
            && row.owner_entity_index == player && row.player_entity_index == player
    }), "the claimed generation must carry the actual player's source relations");
    eprintln!("earned-mana public claim: root={} sphere={} owner={player}", root.source_entity_index, reward_slot);
    let mut collected = None;
    let mut deposited = false;
    let mut saw_capacity = false;
    let mut collector_trace = Vec::new();
    let mut reward_retirement_event_end = None;
    for turn in 0..512 {
        let sim = campaign.simulation();
        let original_live = sim.source_arena().handle(reward_slot) == Some(reward_handle);
        if !original_live && reward_retirement_event_end.is_none() {
            reward_retirement_event_end = Some(sim.economy().events().len());
        }
        if matches!(turn, 0 | 1 | 7 | 31 | 127 | 511) {
            collector_trace.push((turn,
                sim.economy().castle_for_owner(player).map(|row|
                    (row.source_entity_index, row.position, row.level, row.phase, row.mana, row.defender_slots)),
                sim.economy().spheres().iter().find(|row| row.source_entity_index == reward_slot).map(|row|
                    (row.position, row.velocity_x, row.velocity_y, row.vertical_velocity, row.player_entity_index,
                        row.assigned_defender_slot, row.pending_delete, row.control_flags)),
                sim.economy().castle_children().iter().filter(|row| row.source_class == 3 && row.model == 3)
                    .map(|row| (row.source_entity_index, row.owner_entity_index, row.id_entity_index,
                        row.position, row.action, row.source_byte_3e, row.target_sphere_slot, row.mana, row.life))
                    .collect::<Vec<_>>(),
                sim.shared_spells().player_entity().max_mana));
            eprintln!("earned-mana collector snapshot: {:#?}", collector_trace.last().unwrap());
        }
        if original_live {
            assert!(sim.economy().owner_mana_rows.iter().filter(|row| row.source_entity_index == reward_slot)
                .all(|row| row.player_entity_index != rival),
                "the actual claimed sphere generation must never enter the other owner's census");
        }
        saw_capacity |= sim.shared_spells().player_entity().max_mana >= capacity_before + sphere.mana;
        let collection_end = reward_retirement_event_end.unwrap_or_else(|| sim.economy().events().len());
        for event in &sim.economy().events()[reward_event_start..collection_end] {
            match event {
                Level2EconomyEvent::CastleDefenderCollectedSphere { sphere_id, defender_slot, mana }
                    if *sphere_id == u32::from(reward_slot) && *mana == sphere.mana
                        && sim.economy().castle_children().iter().any(|row| row.source_entity_index == *defender_slot
                            && row.source_class == 3 && row.model == 3 && row.id_entity_index == player) => collected = Some(*defender_slot),
                _ => {}
            }
        }
        deposited |= sim.economy().events()[reward_event_start..].iter().any(|event| matches!(event,
            Level2EconomyEvent::CastleDefenderArrived { defender_slot, castle_id, mana }
                if Some(*defender_slot) == collected && *castle_id == root.source_entity_index && *mana == sphere.mana));
        if deposited && saw_capacity && sim.source_arena().handle(reward_slot) != Some(reward_handle)
            && sim.source_arena().handle(possession_flyer_handle.source_slot) != Some(possession_flyer_handle)
            && sim.source_arena().handle(possession_effect_handle.source_slot) != Some(possession_effect_handle) { break; }
        campaign.step(PlayerCommand::default(), 0);
        assert!(campaign.simulation().dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
    }
    assert!(collected.is_some() && deposited,
        "the actual owned collector must deliver this sphere to its Castle: claim_position={:?}, trace={collector_trace:#?}, matching_events={:?}",
        sphere.position, campaign.simulation().economy().events()[reward_event_start..].iter().filter(|event| matches!(event,
            Level2EconomyEvent::ManaSphereClaimed { .. } | Level2EconomyEvent::ManaSphereCollected { .. }
                | Level2EconomyEvent::CastleDefenderCollectedSphere { .. } | Level2EconomyEvent::CastleDefenderArrived { .. }))
            .collect::<Vec<_>>());
    assert!(saw_capacity, "the source owner census must expose the earned mana as capacity");
    assert_ne!(campaign.simulation().source_arena().handle(reward_slot), Some(reward_handle),
        "the collected sphere's original generation must cross ordinary reclaim");
    assert_ne!(campaign.simulation().source_arena().handle(possession_flyer_handle.source_slot), Some(possession_flyer_handle),
        "the actual Possession flyer generation must cross ordinary reclaim");
    assert_ne!(campaign.simulation().source_arena().handle(possession_effect_handle.source_slot), Some(possession_effect_handle),
        "the actual Possession effect generation must cross ordinary reclaim");
    assert_eq!(campaign.simulation().source_arena().handle(root.source_entity_index), Some(root_handle));
    assert_eq!(campaign.simulation().source_arena().handle(rival_root), Some(rival_handle));
    assert_eq!(campaign.simulation().economy().castle_for_owner(rival).unwrap().source_entity_index, rival_root);
    let economy = campaign.simulation().economy();
    assert_eq!(economy.castle_for_owner(player).unwrap().mana, castle_mana_before + sphere.mana,
        "the actual owned Castle receives this one sphere once");
    let reward_retirement_event_end = reward_retirement_event_end
        .expect("the original sphere generation crosses an observed source boundary");
    eprintln!("earned-mana original sphere event interval: {:?}",
        economy.events()[reward_event_start..reward_retirement_event_end].iter().filter(|event| matches!(event,
            Level2EconomyEvent::ManaSphereSpawned { sphere_id, .. }
                | Level2EconomyEvent::ManaSphereClaimed { sphere_id, .. }
                | Level2EconomyEvent::CastleDefenderCollectedSphere { sphere_id, .. }
                if *sphere_id == u32::from(reward_slot))).collect::<Vec<_>>());
    assert_eq!(economy.events()[reward_event_start..reward_retirement_event_end].iter().filter(|event| matches!(event,
        Level2EconomyEvent::CastleDefenderCollectedSphere { sphere_id, .. }
            if *sphere_id == u32::from(reward_slot))).count(), 1,
        "the original sphere generation must be collected exactly once");
    assert_eq!(economy.events()[reward_event_start..].iter().filter(|event| matches!(event,
        Level2EconomyEvent::CastleDefenderArrived { defender_slot, castle_id, mana }
            if Some(*defender_slot) == collected && *castle_id == root.source_entity_index && *mana == sphere.mana)).count(), 1,
        "the owned collector must deposit this payload exactly once");
    let live = &campaign.simulation().session.state.live;
    if let Some(row) = economy.owner_mana_rows.iter().find(|row| row.source_entity_index == reward_slot) {
        let replacement = campaign.simulation().source_arena().handle(reward_slot)
            .expect("an owner-mana row at the retired slot must belong to a live replacement");
        assert_ne!(replacement, reward_handle, "a retired sphere generation cannot retain owner credit");
        let raw = &live.economy.entities[usize::from(reward_slot)];
        assert_eq!((row.source_class, row.source_model, row.player_entity_index, row.mana),
            (raw.class, raw.model, raw.source_player_entity_index(), raw.mana),
            "a reused slot's census must describe its current physical occupant");
    }
    assert_eq!(live.economy.entities[usize::from(player)].max_mana,
        economy.owner_max_mana_capacity(player), "the actual player uses the source census");
    assert_eq!(live.economy.entities[usize::from(rival)].max_mana,
        economy.owner_max_mana_capacity(rival), "the other actual owner uses its own source census");
    for (owner, baseline) in [(player, initial_player_max), (rival, initial_rival_max)] {
        let expected = economy.owner_mana_rows.iter()
            .filter(|row| row.active && row.player_entity_index == owner)
            .fold(baseline, |total, row| total.saturating_add(row.mana));
        assert_eq!(economy.owner_max_mana_capacity(owner), expected,
            "each owner receives its current source rows exactly once: owner={owner}");
    }

    // Capacity is not current mana. Refill through the ordinary in-Castle
    // source upkeep, then spend the actually earned excess on a public cast.
    let root_position = campaign.simulation().economy().castle_for_owner(player).unwrap().position;
    let inside = Position { z: root_position.z.wrapping_add(512), ..root_position };
    campaign.simulation_mut().set_level2_entity_position(player, inside).unwrap();
    for _ in 0..128 {
        if campaign.simulation().shared_spells().player_entity().mana > initial_player_max { break; }
        campaign.step(PlayerCommand::default(), 0);
    }
    let before = campaign.simulation().shared_spells().player_entity().clone();
    assert!(before.mana > initial_player_max, "normal source regeneration must reach earned capacity");
    let hud = campaign.simulation().day_hud_for(selection).expect("actual campaign HUD observation");
    assert_eq!((hud.local_player_mana, hud.local_player_max_mana), (before.mana, before.max_mana));
    let fire_holder_state = campaign.simulation().shared_spells().pickup(fire_holder).unwrap();
    let fire_cost = fire_holder_state.max_mana;
    assert!(fire_holder_state.active_value == 0 && fire_cost > 0 && before.mana >= fire_cost,
        "the actual physical Fireball holder must be cold and affordable");
    assert!(before.mana_regen >= 0, "the owner must not carry an unrelated pending charge");
    let existing_flyers = campaign.simulation().economy().fireball_projectiles().iter()
        .filter_map(|row| campaign.simulation().source_arena().handle(row.source_entity_index))
        .collect::<Vec<_>>();
    campaign.simulation_mut().set_level2_player_aim_for_test(0, 0);
    campaign.step(PlayerCommand { opcode: 6, argument_1: fire_ui, flags: 0x40,
        ..PlayerCommand::default() }, 0);
    let after = campaign.simulation().shared_spells().player_entity().clone();
    assert_eq!(after.mana_regen, -fire_cost,
        "the fresh Fireball holder must charge its physical price to actual parent upkeep");
    let fresh_flyer = campaign.simulation().economy().fireball_projectiles().iter()
        .filter(|row| row.active && row.owner_entity_index == player)
        .filter_map(|row| campaign.simulation().source_arena().handle(row.source_entity_index))
        .find(|handle| !existing_flyers.contains(handle))
        .expect("the paid public cast must produce a fresh active owner flyer generation");
    let hud = campaign.simulation().day_hud_for(selection).expect("HUD after public spend");
    assert_eq!((hud.local_player_mana, hud.local_player_max_mana), (after.mana, after.max_mana));
    campaign.step(PlayerCommand::default(), 0);
    let player_handle = campaign.simulation().source_arena().handle(player).unwrap();
    let (before_upkeep, after_upkeep) = campaign.simulation().cf10_production_schedule_receipts().into_iter()
        .find_map(|receipt| match receipt {
            crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted {
                slot, original, current: crate::campaign::schedule::CurrentOccupant::Known { selector, .. }, ..
            } if slot == player => {
                let before = original.observed_player_resources()?;
                let after = selector.observed_player_resources()?;
                (before.0 == player_handle && before.2 == -fire_cost).then_some((before, after))
            }
            _ => None,
        }).expect("the actual player callback must consume this holder's pending charge");
    assert_eq!(after_upkeep.0, player_handle);
    assert_eq!(after_upkeep.1, before_upkeep.1.wrapping_add(before_upkeep.2).clamp(0, before_upkeep.3),
        "the scheduled player consumes the actual pending cost before ordinary regeneration");
    let debited = campaign.simulation().shared_spells().player_entity().clone();
    let hud = campaign.simulation().day_hud_for(selection).expect("HUD after source mana debit");
    assert_eq!((hud.local_player_mana, hud.local_player_max_mana), (debited.mana, debited.max_mana));
    for _ in 0..128 {
        let sim = campaign.simulation();
        if sim.shared_spells().pickup(fire_holder).unwrap().active_value == 0
            && sim.source_arena().handle(fresh_flyer.source_slot) != Some(fresh_flyer) { break; }
        campaign.step(PlayerCommand::default(), 0);
        assert!(campaign.simulation().dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
    }
    assert_eq!(campaign.simulation().shared_spells().pickup(fire_holder).unwrap().active_value, 0,
        "the real paid holder must expire through its ordinary callbacks");
    assert_ne!(campaign.simulation().source_arena().handle(fresh_flyer.source_slot), Some(fresh_flyer),
        "the paid Fireball generation must cross normal source reclaim");
}

#[test]
fn final_castle_loss_hook_precedes_link_and_root_cleanup() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).expect("Level 2 package");
    let mut session = inspection.level2_reset_session().expect("Level 2 session");
    let owner = inspection.players()[0].entity_index;
    let root = session.economy_mut().build_castle(Position::default(), owner).unwrap();
    {
        let mut economy = session.economy_mut();
        let castle = economy.castle_at_mut(root).unwrap();
        castle.action = 6;
        castle.life = -1;
        castle.timer = 3;
        castle.source_word_2e = 7;
        castle.source_word_30 = 3;
    }
    let reset_free_slots = session.session.baseline.economy.economy.free_slots.clone();
    let live = &mut session.session.state.live;
    let mut visited = false;
    assert!(live.economy.economy.step_castle_action_with_final_loss(
        root,
        &mut live.terrain,
        &reset_free_slots,
        &mut |economy, castle| {
            visited = true;
            assert_eq!(castle.level, 0);
            assert!(castle.active);
            assert_eq!((castle.action, castle.life, castle.max_life), (6, -1, CASTLE_STAGE_ONE_LIFE));
            assert_eq!((castle.timer, castle.source_word_2e, castle.source_word_30), (3, 7, 3));
            assert_eq!(economy.castle_for_owner(owner).unwrap().source_entity_index, root);
            assert!(economy.castle_at(root).is_some());
            assert!(!economy.events().iter().any(|event| matches!(event, Level2EconomyEvent::CastleStageDestroyed { source_entity_index, .. } if *source_entity_index == root)));
        },
    ));
    assert!(visited);
    assert!(live.economy.economy.castle_for_owner(owner).is_none());
    assert!(live.economy.economy.castle_at(root).is_none());
    let retired = live.economy.economy.castle_entry(root).unwrap();
    assert_eq!((retired.action, retired.life, retired.max_life), (4, -1, CASTLE_STAGE_ONE_LIFE));
    assert_eq!((retired.timer, retired.source_word_2e, retired.source_word_30), (5, 0, 5));
    assert!(live.economy.economy.events().iter().any(|event| matches!(event, Level2EconomyEvent::CastleStageDestroyed { source_entity_index, next_level: 0, .. } if *source_entity_index == root)));
}

#[test]
fn castle_mana_credit_uses_source_player_relation_and_action4_timing() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).expect("Level 2 package");
    let mut session = inspection.level2_reset_session().expect("Level 2 session");
    let owners = [inspection.players()[0].entity_index, inspection.players()[1].entity_index];
    let first = session.economy_mut().build_castle(Position::default(), owners[0]).unwrap();
    let second = session.economy_mut().build_castle(Position { x: 0x5000, y: 0x5000, z: 0 }, owners[1]).unwrap();
    {
        let mut economy = session.economy_mut();
        let castle = economy.castle_at_mut(first).unwrap();
        castle.mana = 1500;
        castle.action = 5;
        castle.build_state = 4;
        castle.player_entity_index = 0;
        let castle = economy.castle_at_mut(second).unwrap();
        castle.mana = 2400;
        castle.player_entity_index = owners[1];
        economy.refresh_owner_mana_rows();
    }
    let rows = session.economy().owner_mana_rows.iter().filter(|row| row.source_entity_index == first || row.source_entity_index == second).copied().collect::<Vec<_>>();
    assert_eq!(rows.iter().find(|row| row.source_entity_index == first).unwrap().player_entity_index, 0);
    assert_eq!(rows.iter().find(|row| row.source_entity_index == second).unwrap().player_entity_index, owners[1]);
    let projected = level2_owner_max_mana_projection(&BTreeMap::new(), &rows);
    assert_eq!(projected.get(&owners[0]), None);
    assert_eq!(projected.get(&owners[1]), Some(&2400));
    {
        let mut economy = session.economy_mut();
        let castle = economy.castle_at_mut(first).unwrap();
        castle.action = 4;
        castle.timer = 1;
        castle.control_flags |= 0x0010_0000;
        castle.position.z = 12345;
        castle.source_word_80 = owners[0];
        castle.source_word_7c = 10;
    }
    assert!(session.economy_mut().step_castle_action(first));
    let held = session.economy().castle_at(first).unwrap();
    assert_eq!((held.action, held.timer, held.position.z), (4, 1, 12345));
    assert_eq!((held.source_word_80, held.source_word_7c), (owners[0], 10));
    assert_eq!(held.player_entity_index, 0);
    {
        let mut economy = session.economy_mut();
        let castle = economy.castle_at_mut(first).unwrap();
        castle.control_flags &= !0x0010_0000;
    }
    assert!(session.economy_mut().step_castle_action(first));
    let released = session.economy().castle_at(first).unwrap();
    assert_eq!((released.action, released.timer, released.build_state, released.position.z), (5, 0, 3, 12345));
    assert_eq!((released.source_word_80, released.source_word_7c), (owners[0], 10));
    // A later ordinary action-4 visit performs the source mana relation write.
    session.economy_mut().castle_at_mut(first).unwrap().action = 4;
    assert!(session.economy_mut().step_castle_action(first));
    assert_eq!(session.economy().castle_at(first).unwrap().player_entity_index, owners[0]);
    session.economy_mut().refresh_owner_mana_rows();
    let rows = session.economy().owner_mana_rows.iter().filter(|row| row.source_entity_index == first || row.source_entity_index == second).copied().collect::<Vec<_>>();
    let projected = level2_owner_max_mana_projection(&BTreeMap::new(), &rows);
    assert_eq!(projected.get(&owners[0]), Some(&1500));
    assert_eq!(projected.get(&owners[1]), Some(&2400));
    {
        let mut economy = session.economy_mut();
        let castle = economy.castle_at_mut(second).unwrap();
        castle.player_entity_index = 0;
        castle.life = -1;
        castle.action = 4;
        castle.timer = 0;
        castle.source_word_80 = owners[1];
        castle.source_word_7c = 10;
    }
    assert!(session.economy_mut().step_castle_action(second));
    let lethal = session.economy().castle_at(second).unwrap();
    assert_eq!(lethal.action, 6);
    assert_eq!(lethal.player_entity_index, owners[1], "lethal action-4 exit still writes the source relation");
    assert_eq!((lethal.source_word_80, lethal.source_word_7c), (owners[1], 10), "lethal damage precedes upgrade mailbox consumption");
}

#[test]
fn castle_construction_uses_each_owners_generation_bound_coefficients() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).expect("Level 2 package");
    let mut session = inspection.level2_reset_session().expect("Level 2 session");
    let owner_a = inspection.players()[0].entity_index;
    let owner_b = inspection.players()[1].entity_index;
    let a = session.economy_mut().build_castle(Position::default(), owner_a).unwrap();
    let b = session.economy_mut().build_castle(Position { x: 0x5000, y: 0x5000, z: 0 }, owner_b).unwrap();
    assert!(session.economy_mut().set_castle_stage_coefficient_for_owner(owner_a, 1, 128));
    assert!(session.economy_mut().set_castle_stage_coefficient_for_owner(owner_b, 1, 512));
    assert!(session.economy_mut().set_castle_controller_coefficient_for_owner(owner_a, 1, 3));
    assert!(session.economy_mut().set_castle_controller_coefficient_for_owner(owner_b, 1, 7));
    assert!(session.economy_mut().start_castle_construction(a));
    assert!(session.economy_mut().start_castle_construction(b));
    assert!(session.economy_mut().step_castle_action(a));
    assert!(session.economy_mut().step_castle_action(b));
    assert_eq!(session.economy().castle_at(a).unwrap().max_life, 10_000);
    assert_eq!(session.economy().castle_at(b).unwrap().max_life, 40_000);
    for (owner, expected) in [(owner_a, 3), (owner_b, 7)] {
        let controller = session.economy().castle_children().iter()
            .find(|row| row.active && row.model == 79 && row.owner_entity_index == owner)
            .expect("owner's stage controller");
        assert_eq!(controller.source_byte_43, expected);
    }
    let generation = session.economy().slot_generation(owner_b);
    session.economy_mut().slot_generations.insert(owner_b, generation.wrapping_add(1));
    assert_eq!(session.economy().castle_stage_coefficient_for_owner(owner_b, 1), 256);
    assert_eq!(session.economy().castle_controller_coefficients_for_owner(owner_b)[1], 1);
    assert_eq!(session.economy().castle_stage_coefficient_for_owner(owner_a, 1), 128);
}

#[test]
fn ordinary_level_23_materializes_source_house_27_and_runs_construction() {
    let package = std::env::var_os("MC_LEVEL23_PACKAGE").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-copper-house27-v3-oucmzgsa/level23"),
        PathBuf::from,
    );
    let mut campaign = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::ordinary(23).unwrap(),
    ).expect("ordinary Level 23 owner package");
    let house = campaign.simulation().ordinary().houses().iter()
        .find(|house| house.entity_index == 1)
        .copied().expect("actual reset house at source slot 1");
    assert_eq!((house.building_index, house.action), (27, crate::house::HOUSE_TERRAIN_TO_LIVE_ACTION));
    assert_eq!((house.parameter.life_mana_scalar, house.parameter.flags, house.parameter.destruction_successor), (138, 8, 41));
    for selector in [27, 41, 49] {
        assert!(campaign.simulation().ordinary().house_build_footprints.contains_key(&selector), "real BUILD closure lacks {selector}");
    }
    let empty = &campaign.simulation().ordinary().house_build_footprints[&16];
    assert_eq!((empty.width, empty.height, empty.cells()), (1, 1, &[[0xff, 0xff]][..]), "source BUILD loader doubles dimensions before VGA halves them");
    campaign.step(PlayerCommand::default(), 0);
    let dispatch_errors = campaign.simulation().dispatch_events().iter().filter(|event| matches!(event,
        Mc2DispatchEvent::Error { .. }
    )).collect::<Vec<_>>();
    assert!(!dispatch_errors.iter().any(|event| matches!(event,
        Mc2DispatchEvent::Error { source_slot: 1, .. }
    )), "house 27 dispatch errors: {dispatch_errors:?}");
    assert!(campaign.simulation().dispatch_events().iter().any(|event| matches!(event,
        Mc2DispatchEvent::House { source_slot: 1, events, .. }
            if events.iter().any(|event| matches!(event,
                HouseStepEvent::ConstructionPass { .. } | HouseStepEvent::ConstructionCompleted { .. }
            ))
    )), "ordinary source scheduler must run house 27's real BUILD consumer; errors: {dispatch_errors:?}; house: {:?}", campaign.simulation().ordinary().houses().iter().find(|house| house.entity_index == 1));
    assert!(!campaign.simulation().dispatch_events().iter().any(|event| matches!(event,
        Mc2DispatchEvent::Error { source_slot: 1, .. }
    )));
}

#[test]
fn ordinary_level_seven_materializes_owner_house_39_and_runs_its_build() {
    let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
    let narrow = generated.join("mc-bhq6-ordinary-20261024-b/level7");
    let geometry = std::env::var_os("MC_LEVEL7_HOUSE_PACKAGE").map_or_else(
        || generated.join("mc-bhq6-ordinary-20261025-zclu5fjj/level7"),
        PathBuf::from,
    );
    let read = |root: &PathBuf, name: &str| std::fs::read(root.join(name)).unwrap();
    for name in ["authored-level.bin", "entities.bin", "building-parameters.bin"] {
        assert_eq!(read(&narrow, name), read(&geometry, name), "the two packages retain the same owner {name}");
    }
    let parameters = crate::campaign::source_facts::decode_building_parameters(
        &read(&narrow, "building-parameters.bin"),
    ).expect("actual owner building parameters");
    let expected = *parameters.get(&39).expect("actual source House parameter");
    assert!(!crate::house::decode_house_construction_inputs(
        &read(&narrow, "feature-construction-inputs.bin"),
    ).unwrap().contains_key(&39), "narrow package must exercise the full-table join");
    assert!(crate::house::decode_house_construction_inputs(
        &read(&geometry, "feature-construction-inputs.bin"),
    ).unwrap().contains_key(&39), "the second real package retains BUILD 39 geometry");

    let narrow_campaign = crate::campaign::CampaignSession::open(
        narrow,
        crate::campaign::spec::CampaignSelection::ordinary(7).unwrap(),
    ).expect("real Night 7 opens with its full owner table");
    let narrow_house = narrow_campaign.simulation().ordinary().houses().iter()
        .find(|house| house.entity_index == 82).expect("real source House 82");
    assert_eq!((narrow_house.building_index, narrow_house.parameter), (39, expected));
    assert!(!narrow_campaign.simulation().ordinary().house_build_footprints.contains_key(&39),
        "missing geometry must not be synthesized from the parameter table");

    let mut campaign = crate::campaign::CampaignSession::open(
        geometry,
        crate::campaign::spec::CampaignSelection::ordinary(7).unwrap(),
    ).expect("real Night 7 with retained BUILD geometry");
    let house = campaign.simulation().ordinary().houses().iter()
        .find(|house| house.entity_index == 82).expect("real source House 82");
    assert_eq!((house.building_index, house.action, house.parameter),
        (39, crate::house::HOUSE_TERRAIN_TO_LIVE_ACTION, expected));
    assert!(campaign.simulation().ordinary().house_build_footprints.contains_key(&39));
    let mut built = false;
    for _ in 0..64 {
        campaign.step(PlayerCommand::default(), 0);
        assert!(!campaign.simulation().dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::Error { source_slot: 82, .. }
        )), "actual House 82 dispatch failed: {:?}", campaign.simulation().dispatch_events());
        built |= campaign.simulation().dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::House { source_slot: 82, events, .. }
                if events.iter().any(|event| matches!(event,
                    HouseStepEvent::ConstructionPass { .. } | HouseStepEvent::ConstructionCompleted { .. }
                ))
        ));
        if built { break; }
    }
    assert!(built, "the ordinary source scheduler must run real House 82's BUILD consumer");
}

#[test]
fn ordinary_level_23_imports_linked_castles_and_construction_children() {
    let package = std::env::var_os("MC_LEVEL23_PACKAGE").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-copper-house27-v3-oucmzgsa/level23")
        },
        PathBuf::from,
    );
    let campaign = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::ordinary(23).unwrap(),
    )
    .expect("ordinary Level 23 owner package");
    let session = campaign.simulation();
    let economy = session.economy();
    let expected = [
        (425, 450, 1, 8_500, 0),
        (451, 476, 1, 5_000, 612),
        (477, 503, 5, 78_600, 613),
        (504, 530, 5, 78_600, 614),
        (531, 557, 5, 78_600, 615),
        (558, 584, 6, 158_200, 616),
        (585, 611, 7, 317_400, 617),
    ];
    for (owner, slot, level, mana, child) in expected {
        let root = economy
            .castle_for_owner(owner)
            .expect("imported owner Castle");
        assert_eq!(
            (
                root.source_entity_index,
                root.level,
                root.mana,
                root.construction_slot
            ),
            (slot, level, mana, child)
        );
        assert_eq!(root.player_entity_index, 0, "retain the actual source action-5 mana relation");
        assert!(!session.source_arena().entities[usize::from(slot)].active);
        if child != 0 {
            assert!(economy.castle_children().iter().any(|row| row.active
                && row.source_entity_index == child
                && row.parent_entity_index == slot
                && (row.source_class, row.model, row.action) == (10, 42, 44)));
            assert!(!session.source_arena().entities[usize::from(child)].active);
        }
    }
    assert_eq!(economy.live_castles().count(), expected.len());
}

#[test]
fn ordinary_level_23_rival_last_castle_loss_removes_only_its_real_holder() {
    let package = std::env::var_os("MC_LEVEL23_PACKAGE").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-copper-house27-v3-oucmzgsa/level23"),
        PathBuf::from,
    );
    let mut campaign = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::ordinary(23).unwrap(),
    ).expect("ordinary Level 23 owner package");
    let owner = 425;
    let other_owner = 451;
    let root_slot = 450;
    let holder_slot = 428;
    let other_holder = 454;
    let players = &campaign.simulation().shared_spells().players;
    assert_eq!(players.iter().find(|p| p.entity_index == owner).unwrap().enabled_spell_holders[2], holder_slot);
    assert_eq!(players.iter().find(|p| p.entity_index == other_owner).unwrap().enabled_spell_holders[2], other_holder);
    let holder = campaign.simulation().shared_spells().pickup(holder_slot).expect("real rival Castle holder");
    assert!(holder.active && holder.class == 15 && holder.model == 2 && holder.source_spell_parent_entity_index() == owner);
    let holder_generation = campaign.simulation().economy().slot_generation(holder_slot);
    assert_eq!(campaign.simulation().economy().castle_for_owner(owner).unwrap().source_entity_index, root_slot);
    let mut root_trace = Vec::new();
    for turn in 0..16 {
        let simulation = campaign.simulation();
        let root = simulation.economy().castle_at(root_slot).map(|root| {
            (root.action, root.build_state, root.source_word_2e, root.phase,
                root.control_flags, root.level, root.mana, root.construction_slot)
        });
        let children = simulation.economy().castle_children().iter().filter(|child| {
            child.parent_entity_index == root_slot
        }).map(|child| (child.source_entity_index, child.model, child.action,
            child.active, child.control_flags, child.life)).collect::<Vec<_>>();
        let receipts = simulation.dispatch_events().iter().filter(|event| matches!(event,
            Mc2DispatchEvent::Castle { source_slot, .. } if *source_slot == root_slot
        ) || matches!(event, Mc2DispatchEvent::Error { .. })).cloned().collect::<Vec<_>>();
        root_trace.push((turn, root, children, receipts, simulation.economy().free_slot_count()));
        if campaign.simulation().economy().castle_at(root_slot).is_some_and(|root| root.action == 4) {
            break;
        }
        campaign.step(PlayerCommand::default(), 0);
    }
    assert_eq!(campaign.simulation().economy().castle_at(root_slot).unwrap().action, 4,
        "actual imported Castle must reach action4; source trace: {root_trace:#?}");
    assert!(campaign.simulation_mut().economy_mut().submit_damage(root_slot, i32::MAX, 424));
    for _ in 0..16 {
        campaign.step(PlayerCommand::default(), 0);
        if campaign.simulation().economy().castle_for_owner(owner).is_none() {
            break;
        }
    }
    let simulation = campaign.simulation();
    assert!(simulation.economy().castle_for_owner(owner).is_none());
    assert!(simulation.economy().castle_for_owner(other_owner).is_some());
    let players = &simulation.shared_spells().players;
    assert_eq!(players.iter().find(|p| p.entity_index == owner).unwrap().enabled_spell_holders[2], 0);
    assert_eq!(players.iter().find(|p| p.entity_index == other_owner).unwrap().enabled_spell_holders[2], other_holder);
    assert!(simulation.shared_spells().pickup(holder_slot).is_none_or(|holder| !holder.active || holder.control_flags & ENTITY_DRAW_DISABLED_BIT != 0));
    assert!(simulation.shared_spells().pickup(other_holder).is_some_and(|holder| holder.active && holder.control_flags & ENTITY_DRAW_DISABLED_BIT == 0));
    campaign.step(PlayerCommand::default(), 0);
    let simulation = campaign.simulation();
    assert!(simulation.economy().free_source_slots().contains(&holder_slot)
        || simulation.economy().slot_generation(holder_slot) != holder_generation,
        "the hidden holder must cross the next source reclaim boundary");

    // Check both the generation-bound relation and the AI's actual A4
    // carrier, whose family-2 entry is consumed by sub_146c0 decisions.
    // Keep the other rival as a positive control and run ordinary source
    // visits; no holder, Castle, target or AI outcome is injected.
    let live = &simulation.session.state.live;
    let owner_handle = live.economy.handle(owner).expect("source22 rival remains live");
    let other_handle = live.economy.handle(other_owner).expect("other rival remains live");
    let player = live.core.players.iter().find(|player| player.entity_index == owner).unwrap();
    let other_player = live.core.players.iter().find(|player| player.entity_index == other_owner).unwrap();
    assert!(live.nyphur_dispatcher.bound_live_holder(owner_handle, player, &live.economy, 2).is_none(),
        "the affected rival's real family-2 lookup must reject the removed holder");
    assert_eq!(live.nyphur_dispatcher.bound_live_holder(other_handle, other_player, &live.economy, 2)
        .map(|holder| holder.source_slot), Some(other_holder),
        "the unrelated rival retains its real family-2 lookup");
    let owner_ai = live.nyphur_dispatcher.get(owner_handle).unwrap().common_upkeep(0);
    let other_ai = live.nyphur_dispatcher.get(other_handle).unwrap().common_upkeep(0);
    assert_eq!((owner_ai.castle_entity_index, owner_ai.spell_holder_indices[2]), (0, 0));
    assert_eq!((other_ai.castle_entity_index, other_ai.spell_holder_indices[2]), (476, other_holder));
    let mut saw_owner_visit = false;
    for _ in 0..8 {
        campaign.step(PlayerCommand::default(), 0);
        let simulation = campaign.simulation();
        assert!(simulation.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
        for trace in simulation.nyphur_dispatch_traces().iter().filter(|trace| trace.source_slot == owner) {
            saw_owner_visit = true;
            assert_ne!(trace.state_byte, 3,
                "feature4 must not enter the no-Castle rebuild mode after removing the holder");
            assert!(trace.calls.iter().any(|call| matches!(call,
                crate::native_world::level2_nyphur::Level2NyphurDispatchCall::Sub12e70)),
                "the ordinary AI visit must execute the decision tail");
        }
        let live = &simulation.session.state.live;
        assert_eq!(live.economy.handle(owner), Some(owner_handle));
        let player = live.core.players.iter().find(|player| player.entity_index == owner).unwrap();
        assert_eq!(player.enabled_spell_holders[2], 0);
        assert!(live.nyphur_dispatcher.bound_live_holder(owner_handle, player, &live.economy, 2).is_none());
        assert!(live.economy.economy.castle_for_owner(owner).is_none());
        let owner_ai = live.nyphur_dispatcher.get(owner_handle).unwrap().common_upkeep(0);
        assert_eq!((owner_ai.castle_entity_index, owner_ai.spell_holder_indices[2]), (0, 0),
            "ordinary AI visits must not reconstruct the lost Castle relation or holder");
    }
    assert!(saw_owner_visit, "the affected real rival must reach its ordinary AI consumer");
}

#[test]
fn ordinary_level_5_rival_last_castle_loss_preserves_holder_without_feature4() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-paj2-source4-campaign-package");
    let mut campaign = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::ordinary(5).unwrap(),
    ).expect("ordinary Level 5 owner package");
    let owner = 266;
    let root_slot = 271;
    let holder_slot = 269;
    let player = campaign.simulation().shared_spells().players.iter().find(|p| p.entity_index == owner).unwrap();
    assert_eq!(player.enabled_spell_holders[2], holder_slot);
    assert!(campaign.simulation().shared_spells().pickup(holder_slot).is_some_and(|holder| holder.active && holder.model == 2 && holder.source_spell_parent_entity_index() == owner));
    assert_eq!(campaign.simulation().economy().castle_for_owner(owner).unwrap().source_entity_index, root_slot);
    for _ in 0..128 {
        if campaign.simulation().economy().castle_at(root_slot).is_some_and(|root| root.action == 4) {
            break;
        }
        campaign.step(PlayerCommand::default(), 0);
    }
    assert_eq!(campaign.simulation().economy().castle_at(root_slot).unwrap().action, 4);
    let old_root = campaign.simulation().source_arena().handle(root_slot).expect("imported root generation");
    let controllers = campaign.simulation().economy().castle_at(root_slot).unwrap()
        .controller_chain_slots.iter().copied().filter(|slot| *slot != 0)
        .map(|slot| (slot, campaign.simulation().economy().slot_generation(slot)))
        .collect::<Vec<_>>();
    eprintln!("real old Castle controller count: {}", controllers.len());
    let holder = campaign.simulation().shared_spells().pickup(holder_slot).unwrap();
    assert!(holder.max_mana > 0);
    // Destruction reprices the retained holder from stage one to stage zero,
    // so a value just below the old price would not prove the later gate.
    let below_capacity = 0_i32;
    let owner_handle = campaign.simulation().source_arena().handle(owner).unwrap();
    {
        // This existing refusal control starts with an explicitly depleted
        // Castle as well as zero source base/current mana. Otherwise the
        // real destruction sphere retains its 5,000 owned mana and the new
        // source census correctly makes the 1,000-price holder affordable.
        let live = &mut campaign.simulation_mut().session.state.live;
        live.economy.economy.castle_at_mut(root_slot).unwrap().mana = 0;
        live.economy.economy.owner_base_max_mana.insert(owner, below_capacity);
        let actor = &mut live.economy.entities[usize::from(owner)];
        actor.max_mana = below_capacity;
        actor.record[144..148].copy_from_slice(&below_capacity.to_le_bytes());
        actor.set_mana(0);
        actor.set_mana_regen(0);
        let actor = actor.clone();
        assert!(live.nyphur_dispatcher.get_mut(owner_handle).unwrap()
            .sync_support_parent_resources(&actor, false));
    }
    assert!(campaign.simulation_mut().economy_mut().submit_damage(root_slot, i32::MAX, 265));
    for _ in 0..16 {
        campaign.step(PlayerCommand::default(), 0);
        if campaign.simulation().economy().castle_for_owner(owner).is_none() {
            break;
        }
    }
    let simulation = campaign.simulation();
    assert!(simulation.economy().castle_for_owner(owner).is_none());
    assert_eq!(simulation.shared_spells().players.iter().find(|p| p.entity_index == owner).unwrap().enabled_spell_holders[2], holder_slot);
    assert!(simulation.shared_spells().pickup(holder_slot).is_some_and(|holder| holder.active && holder.control_flags & ENTITY_DRAW_DISABLED_BIT == 0));
    // The old generation must cross the normal following-pass reclaim before
    // the rival is allowed the resources to select a replacement site.
    campaign.step(PlayerCommand::default(), 0);
    assert_ne!(campaign.simulation().source_arena().handle(root_slot), Some(old_root),
        "the destroyed root must reach its next source reclaim boundary");
    for (slot, generation) in controllers {
        assert!(campaign.simulation().economy().free_source_slots().contains(&slot)
            || campaign.simulation().economy().slot_generation(slot) != generation,
            "the retired controller must cross its source reclaim boundary");
    }
    assert!(campaign.simulation().economy().castle_for_owner(owner).is_none());
    let mut saw_below_gate_visit = false;
    for _ in 0..8 {
        campaign.step(PlayerCommand::default(), 0);
        let sim = campaign.simulation();
        for trace in sim.nyphur_dispatch_traces().iter().filter(|trace| trace.source_slot == owner) {
            saw_below_gate_visit = true;
            let live = &sim.session.state.live;
            let ai = live.nyphur_dispatcher.get(owner_handle).unwrap().common_upkeep(0);
            let actor = &live.economy.entities[usize::from(owner)];
            let current_holder = &live.economy.entities[usize::from(holder_slot)];
            assert_eq!(ai.max_mana, actor.max_mana,
                "the decision must use the same capacity as the real actor");
            assert!(ai.max_mana < current_holder.max_mana,
                "below-capacity premise: AI {} vs current holder {}", ai.max_mana, current_holder.max_mana);
            assert_ne!(trace.state_byte, 3, "insufficient capacity cannot enter rebuild mode");
            assert!(trace.calls.iter().any(|call| matches!(call,
                crate::native_world::level2_nyphur::Level2NyphurDispatchCall::Sub12e70)));
            assert_ne!(live.nyphur_dispatcher.get(owner_handle).unwrap().state_byte(), 3,
                "the actual selector must reject the retained holder below its capacity gate");
        }
    }
    assert!(saw_below_gate_visit, "the owner must reach the actual below-capacity decision");

    {
        // Declare learned tier two in the real player's source progression;
        // the retained physical holder remains at tier zero until mode3
        // invokes the ordinary source SetSpell path.
        let live = &mut campaign.simulation_mut().session.state.live;
        let player = live.shared_spells.players.iter_mut()
            .find(|player| player.entity_index == owner).unwrap();
        let runtime = LEVEL2_SPELL_TIER_THRESHOLDS[2][2]
            .wrapping_sub(player.persistent_spell_experience[2]);
        player.runtime_spell_experience[2] = runtime;
        player.spell_unlocked_levels[2] = 2;
        player.record[172..176].copy_from_slice(&runtime.to_le_bytes());
        player.record[504] = 2;
        let player = player.clone();
        live.core.players.iter_mut().find(|row| row.entity_index == owner).unwrap()
            .sync_spell_progression_family_from(&player, 2);
        assert_eq!(live.economy.entities[usize::from(holder_slot)].level_gate, 0);
    }
    {
        // Declared resource prerequisite, not earned mana: provide a normal
        // 20,000 capacity and balance so the actual holder's 1,000/4,000
        // source gates can run without inventing a decision or Castle.
        let live = &mut campaign.simulation_mut().session.state.live;
        live.economy.economy.owner_base_max_mana.insert(owner, 20_000);
        let actor = &mut live.economy.entities[usize::from(owner)];
        actor.max_mana = 20_000;
        actor.record[144..148].copy_from_slice(&20_000_i32.to_le_bytes());
        actor.set_mana(20_000);
        actor.set_mana_regen(0);
        let actor = actor.clone();
        assert!(live.nyphur_dispatcher.get_mut(owner_handle).unwrap()
            .sync_support_parent_resources(&actor, false));
    }
    let mut saw_selection = false;
    for _ in 0..64 {
        campaign.step(PlayerCommand::default(), 0);
        let sim = campaign.simulation();
        assert!(sim.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
        let live = &sim.session.state.live;
        if live.nyphur_dispatcher.get(owner_handle).unwrap().state_byte() == 3 {
            let ai = live.nyphur_dispatcher.get(owner_handle).unwrap().common_upkeep(0);
            let player = live.core.players.iter().find(|player| player.entity_index == owner).unwrap();
            assert_eq!((player.spell_unlocked_levels[2], ai.spell_levels[2]), (2, 2),
                "the real selector must retain declared player progression through the roster");
            assert!(ai.mana >= 8_000 && ai.max_mana >= 8_000,
                "the tier2 no-Castle gate must have its actual owner mana and capacity");
            saw_selection = true;
            break;
        }
    }
    assert!(saw_selection, "the real no-Castle selector must choose mode3 after its gate passes");

    // Control the rival's travel to the AI-selected axis after, and only
    // after, the real selector chose it. The mode3 distance gate and seed
    // constructor still execute through ordinary scheduled visits.
    let selected = &campaign.simulation().entities()[usize::from(owner)].record;
    let x = u16::from_le_bytes([selected[158], selected[159]]);
    let y = u16::from_le_bytes([selected[160], selected[161]]);
    let site = Position { x, y, z: i16::from_le_bytes([selected[162], selected[163]]) };
    let (expected_root, expected_anchor) = {
        let live = &campaign.simulation().session.state.live;
        let footprint = live.economy.economy.castle_stage_footprints.get(&1)
            .expect("ordinary map has the source Castle footprint");
        level2_castle_root_positions(&live.terrain, footprint, site)
    };
    let initial_holder_tier = campaign.simulation().entities()[usize::from(holder_slot)].level_gate;
    assert_eq!(campaign.simulation().entities()[usize::from(holder_slot)].active_value, 0,
        "this control exercises cold SetSpell rather than the active queued-tier branch");
    assert_eq!(&campaign.simulation().entities()[usize::from(holder_slot)].record[50..52], &[0, 0],
        "the real source word2E must also be cold before the next mode3 visit");
    campaign.simulation_mut().set_level2_entity_position(owner, site).unwrap();
    let mut saw_mode3 = false;
    let mut seed = None;
    let mut mana_before_seed = None;
    for _ in 0..32 {
        let before = campaign.simulation().session.state.live.nyphur_dispatcher
            .get(owner_handle).unwrap().common_upkeep(0).mana;
        campaign.step(PlayerCommand::default(), 0);
        let sim = campaign.simulation();
        assert!(sim.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
        saw_mode3 |= sim.nyphur_dispatch_traces().iter().any(|trace| trace.source_slot == owner && trace.state_byte == 3);
        if let Some(root) = sim.economy().castle_for_owner(owner) {
            seed = Some(*root);
            mana_before_seed = Some(before);
            break;
        }
    }
    let seed = seed.expect("the real mode3 decision must create its owned Castle seed");
    assert!(saw_mode3);
    assert_eq!((seed.source_class, seed.source_model, seed.owner_entity_index), (3, 2, owner));
    assert_eq!(seed.position, expected_root, "the selected site uses source terrain placement");
    assert_eq!(seed.anchor_position, expected_anchor, "construction uses the source perimeter anchor");
    let sim = campaign.simulation();
    let live = &sim.session.state.live;
    let ai = live.nyphur_dispatcher.get(owner_handle).unwrap().common_upkeep(0);
    assert_eq!(ai.castle_entity_index, seed.source_entity_index,
        "the actual AI state must see the new root by its same-visit tail");
    assert_eq!(ai.spell_enabled[2], 0, "the source no-Castle seed sets no spell cooldown");
    assert_eq!(Some(ai.mana), mana_before_seed, "the source direct seed does not charge mana");
    assert_eq!(live.core.players.iter().find(|player| player.entity_index == owner).unwrap().enabled_spell_holders[2], holder_slot);
    let raw_holder = &live.economy.entities[usize::from(holder_slot)];
    let shared_holder = live.shared_spells.pickup(holder_slot).unwrap();
    let expected_profile = live.source_spell_profile(2, 2, owner)
        .expect("real owner tier2 profile for the current Castle stage");
    eprintln!("real rival Castle holder tier: {initial_holder_tier} -> {}; AI learned tier {}, root level {} action {}",
        raw_holder.level_gate, ai.spell_levels[2], seed.level, seed.action);
    assert_eq!((initial_holder_tier, raw_holder.level_gate), (0, 2),
        "mode3 must apply the real learned tier to the retained physical holder");
    assert_eq!(raw_holder.level_gate, ai.spell_levels[2], "actual mode3 SetSpell applies the source-selected tier");
    assert_eq!((raw_holder.max_mana, raw_holder.mana, raw_holder.mana_regen),
        (expected_profile.max_mana, expected_profile.mana, expected_profile.mana_regen),
        "the current physical tier must carry the current Castle-stage price");
    assert_eq!((shared_holder.level_gate, shared_holder.max_mana, shared_holder.mana, shared_holder.mana_regen),
        (raw_holder.level_gate, raw_holder.max_mana, raw_holder.mana, raw_holder.mana_regen),
        "the following Castle holder callback must consume the authoritative SetSpell profile");
    assert_eq!(&shared_holder.record[48..54], &raw_holder.record[48..54],
        "queued tier, active word and activation duration must be published together");
    assert_eq!(shared_holder.active_value, 0,
        "the direct seed does not activate the player Castle projectile holder");
    let initial_holder_phase = shared_holder.phase;
    let seed_handle = live.economy.handle(seed.source_entity_index).expect("new source root is live");
    let mut built = false;
    let mut saw_following_holder_visit = false;
    let mut construction_child = None;
    let mut before_completion = None;
    for _ in 0..2048 {
        campaign.step(PlayerCommand::default(), 0);
        let sim = campaign.simulation();
        assert!(sim.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
        assert_eq!(sim.source_arena().handle(seed.source_entity_index), Some(seed_handle));
        let live = &sim.session.state.live;
        let raw = &live.economy.entities[usize::from(holder_slot)];
        let shared = live.shared_spells.pickup(holder_slot).unwrap();
        assert_eq!((raw.phase, raw.active_value, &raw.record[48..54], &raw.record[58..60]),
            (shared.phase, shared.active_value, &shared.record[48..54], &shared.record[58..60]),
            "following actual holder visits must preserve phase, active word, queued tier and timer");
        saw_following_holder_visit |= shared.phase != initial_holder_phase;
        if let Some(root) = sim.economy().castle_for_owner(owner) {
            if root.level == 1 && root.action == 5 && before_completion.is_none() {
                before_completion = Some(campaign.clone());
            }
            if root.construction_slot != 0 && construction_child.is_none()
                && let Some(child) = sim.economy().castle_children().iter()
                    .find(|child| child.source_entity_index == root.construction_slot && child.active)
            {
                assert_eq!((child.model, child.parent_entity_index, child.owner_entity_index),
                    (42, root.source_entity_index, owner));
                construction_child = sim.source_arena().handle(root.construction_slot);
            }
            if root.level >= 1 && root.action == 4 {
                built = true;
            }
        }
        if built && saw_following_holder_visit {
            break;
        }
    }
    assert!(built, "the actual seed must finish construction and reach Castle action4");
    assert!(saw_following_holder_visit, "the retained holder must reach a following source visit");
    let construction_child = construction_child.expect("the rebuilt Castle must create a real construction child");
    if campaign.simulation().source_arena().handle(construction_child.source_slot) == Some(construction_child) {
        eprintln!("first-stage construction child before following reclaim: {:?}",
            campaign.simulation().economy().castle_children().iter().find(|row| row.source_entity_index == construction_child.source_slot)
                .map(|row| (row.model, row.action, row.active, row.control_flags, row.source_byte_3e)));
    }
    for _ in 0..16 {
        if campaign.simulation().source_arena().handle(construction_child.source_slot) != Some(construction_child) { break; }
        campaign.step(PlayerCommand::default(), 0);
        if campaign.simulation().dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })) {
            let sim = campaign.simulation();
            let arena = &sim.session.state.live.economy;
            let occupants = sim.economy().castle_children().iter()
                .filter(|child| child.active && (child.source_class, child.model) == (3, 3))
                .map(|child| {
                    let slot = child.source_entity_index;
                    let raw = &arena.entities[usize::from(slot)];
                    (slot, child.parent_entity_index, child.owner_entity_index, child.id_entity_index,
                        raw.active, raw.class, raw.model, raw.action, raw.source_id, raw.control_flags,
                        arena.handle(slot), arena.economy.live_arena_cursor_row(slot))
                }).collect::<Vec<_>>();
            eprintln!("first-stage child failure: typed defender/current raw/arena occupants={occupants:?}");
        }
        assert!(campaign.simulation().dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "first-stage child following-pass dispatch errors: {:?}", campaign.simulation().dispatch_events().iter()
                .filter(|event| matches!(event, Mc2DispatchEvent::Error { .. })).collect::<Vec<_>>());
    }
    assert_ne!(campaign.simulation().source_arena().handle(construction_child.source_slot),
        Some(construction_child), "the real construction child must reach source reclamation");

    // The pre-completion snapshot is a real campaign state. With the same
    // retained holder but insufficient owner capacity, actual scheduled
    // decisions must not select the existing-Castle upgrade mode.
    let mut low_capacity = before_completion.expect("the real first-stage construction was observed");
    {
        let live = &mut low_capacity.simulation_mut().session.state.live;
        live.economy.economy.owner_base_max_mana.insert(owner, 0);
        let actor = &mut live.economy.entities[usize::from(owner)];
        actor.max_mana = 0;
        actor.record[144..148].copy_from_slice(&0_i32.to_le_bytes());
        actor.set_mana(0);
        actor.set_mana_regen(0);
        let actor = actor.clone();
        assert!(live.nyphur_dispatcher.get_mut(owner_handle).unwrap()
            .sync_support_parent_resources(&actor, false));
    }
    let mut saw_low_capacity_visit = false;
    for _ in 0..256 {
        low_capacity.step(PlayerCommand::default(), 0);
        let sim = low_capacity.simulation();
        assert!(sim.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
        if sim.economy().castle_for_owner(owner).is_some_and(|root| root.action == 4 && root.level == 1) {
            let live = &sim.session.state.live;
            for trace in sim.nyphur_dispatch_traces().iter().filter(|trace| trace.source_slot == owner) {
                saw_low_capacity_visit = true;
                let ai = live.nyphur_dispatcher.get(owner_handle).unwrap().common_upkeep(0);
                let holder = &live.economy.entities[usize::from(holder_slot)];
                assert!(ai.max_mana < holder.max_mana, "the actual upgrade capacity gate must be below the holder price");
                assert_ne!(trace.state_byte, 1, "insufficient capacity cannot enter the upgrade mode");
                assert_ne!(live.nyphur_dispatcher.get(owner_handle).unwrap().state_byte(), 1,
                    "the source decision must not select an unaffordable upgrade");
            }
            if saw_low_capacity_visit { break; }
        }
    }
    assert!(saw_low_capacity_visit, "the completed Castle must reach a real below-capacity AI decision");

    // Controlled travel and aim bring the real rival close to its own
    // completed Castle. They do not set the AI mode, target, holder or flyer.
    let root = *campaign.simulation().economy().castle_for_owner(owner).unwrap();
    let approach = Position { x: root.position.x.wrapping_sub(500), y: root.position.y,
        z: root.position.z.saturating_add(512) };
    campaign.simulation_mut().set_level2_entity_position(owner, approach).unwrap();
    let yaw = i16::try_from(heading_for_positions(approach, root.position)).unwrap();
    {
        let live = &mut campaign.simulation_mut().session.state.live;
        let actor = &mut live.economy.entities[usize::from(owner)];
        actor.set_yaw(yaw);
        actor.set_source_roll(yaw);
    }
    let mut selected_upgrade = false;
    for _ in 0..256 {
        let sim = campaign.simulation();
        assert!(sim.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
        let live = &sim.session.state.live;
        if live.nyphur_dispatcher.get(owner_handle).unwrap().state_byte() == 1 {
            let actor = &live.economy.entities[usize::from(owner)];
            assert_eq!(actor.source_word_96(), root.source_entity_index,
                "the real selector must bind the generation-owned Castle target");
            selected_upgrade = true;
            break;
        }
        campaign.step(PlayerCommand::default(), 0);
    }
    if !selected_upgrade {
        let sim = campaign.simulation();
        let live = &sim.session.state.live;
        let ai = live.nyphur_dispatcher.get(owner_handle).unwrap().common_upkeep(0);
        let current = sim.economy().castle_for_owner(owner).unwrap();
        let actor = &live.economy.entities[usize::from(owner)];
        let holder = sim.shared_spells().pickup(holder_slot).unwrap();
        let mut slots = live.economy.entities.iter()
            .filter(|row| row.active && row.class == 3 && row.life >= 0)
            .map(|row| row.index).collect::<Vec<_>>();
        slots.extend(sim.economy().live_castles().map(|row| row.source_entity_index));
        slots.sort_unstable();
        slots.dedup();
        let site = sim.economy().castle_upgrade_site_checks(
            current.source_entity_index, &live.terrain, &slots, &live.economy.entities);
        let terrain_only = sim.economy().castle_upgrade_site_checks(
            current.source_entity_index, &live.terrain, &[], &live.economy.entities);
        eprintln!("upgrade selector diagnosis: site={site:?} terrain_only={terrain_only:?} root_slot={} position={:?} stage={} action={} timer={} shifts=({},{}) actor_position={:?} yaw={} roll={} W={} holder_price={} cooldown={} ai_mana={}/{} cadence={} mode={}",
            current.source_entity_index, current.position, current.level, current.action, current.source_word_30,
            current.shift_pitch, current.shift_roll, actor.position, actor.yaw, actor.source_roll(), actor.active_value,
            holder.max_mana, ai.spell_enabled[2], ai.mana, ai.max_mana, ai.cadence_eligible,
            live.nyphur_dispatcher.get(owner_handle).unwrap().state_byte());
    }
    assert!(selected_upgrade, "the ordinary AI must actually select its valid existing Castle for upgrade");

    // Selection uses capacity; its following activation uses current mana.
    // Test the actual latched consumer with current mana below the price.
    let mut low_mana = campaign.clone();
    {
        let live = &mut low_mana.simulation_mut().session.state.live;
        let actor = &mut live.economy.entities[usize::from(owner)];
        actor.set_mana(0);
        actor.set_mana_regen(0);
        let actor = actor.clone();
        assert!(live.nyphur_dispatcher.get_mut(owner_handle).unwrap()
            .sync_support_parent_resources(&actor, false));
    }
    let mut saw_denied_visit = false;
    for _ in 0..16 {
        low_mana.step(PlayerCommand::default(), 0);
        let sim = low_mana.simulation();
        if sim.nyphur_dispatch_traces().iter().any(|trace| trace.source_slot == owner && trace.state_byte == 1) {
            saw_denied_visit = true;
            assert!(sim.economy().castle_projectiles().iter().all(|flyer| !flyer.active || flyer.owner_entity_index != owner),
                "the real state1 consumer must refuse a flyer without current mana");
            assert_eq!(sim.shared_spells().pickup(holder_slot).unwrap().active_value, 0,
                "the refused activation must not arm the retained holder");
            break;
        }
    }
    assert!(saw_denied_visit, "the unaffordable real state1 consumer must be visited");

    let (starting_mana, previous_price) = {
        let sim = campaign.simulation();
        let ai = sim.session.state.live.nyphur_dispatcher.get(owner_handle).unwrap().common_upkeep(0);
        let holder = sim.shared_spells().pickup(holder_slot).unwrap();
        (ai.mana, holder.max_mana)
    };
    assert!(previous_price > 1_000 && starting_mana >= previous_price,
        "the controlled owner must actually afford its physical Castle holder price");
    let mut saw_state1 = false;
    let mut flyer = None;
    let mut paid = false;
    let mut upgraded = false;
    let mut upgrade_child = None;
    let mut upgrade_product = None;
    let mut saw_root_mailbox = false;
    let mut saw_queued_tier_zero = false;
    let mut saw_queued_tier_applied = false;
    let mut previous_holder_state = None;
    for _ in 0..2048 {
        let (before_stage, before_holder, before_parent_regen) = {
            let sim = campaign.simulation();
            (sim.economy().castle_for_owner(owner).unwrap().level,
                sim.shared_spells().pickup(holder_slot).unwrap().clone(),
                sim.session.state.live.economy.entities[usize::from(owner)].mana_regen)
        };
        campaign.step(PlayerCommand::default(), 0);
        let sim = campaign.simulation();
        assert!(sim.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
        saw_state1 |= sim.nyphur_dispatch_traces().iter().any(|trace| trace.source_slot == owner && trace.state_byte == 1);
        let live = &sim.session.state.live;
        let ai = live.nyphur_dispatcher.get(owner_handle).unwrap().common_upkeep(0);
        let holder = sim.shared_spells().pickup(holder_slot).unwrap();
        let holder_state = (holder.level_gate, holder.active_value > 0, holder.source_word_2c());
        if previous_holder_state != Some(holder_state) {
            eprintln!("Castle holder transition tier/active/queued={holder_state:?}; owner mode={} cooldown={}",
                live.nyphur_dispatcher.get(owner_handle).unwrap().state_byte(), ai.spell_enabled[2]);
            previous_holder_state = Some(holder_state);
        }
        saw_queued_tier_zero |= holder.active_value > 0 && holder.source_word_2c() == 1;
        if saw_queued_tier_zero && holder.active_value == 0
            && holder.source_word_2c() == 0 && holder.level_gate == 0 {
            let profile = live.source_spell_profile(2, 0, owner)
                .expect("the first released holder has a current-stage source profile");
            let raw = &live.economy.entities[usize::from(holder_slot)];
            assert_eq!((holder.max_mana, holder.mana, holder.mana_regen),
                (profile.max_mana, profile.mana, profile.mana_regen));
            assert_eq!((raw.level_gate, raw.max_mana, raw.mana, raw.mana_regen),
                (holder.level_gate, holder.max_mana, holder.mana, holder.mana_regen),
                "the original cast releases into its source-queued profile");
            saw_queued_tier_applied = true;
        }
        if let Some(actual) = sim.economy().castle_projectiles().iter().find(|projectile| {
            projectile.owner_entity_index == owner && projectile.holder_entity_index == holder_slot
        }) {
            if flyer.is_none() {
                // Source charges the physical price before allocating this
                // fresh holder's flyer; owner 266 precedes holder 269 in the
                // same source pass. Read the actual parent's resulting upkeep
                // on that allocation step, not any later unrelated charge.
                let raw_parent = &live.economy.entities[usize::from(owner)];
                let holder = sim.shared_spells().pickup(holder_slot).unwrap();
                let price = holder.max_mana;
                let current = sim.economy().castle_for_owner(owner).unwrap();
                eprintln!("first owned Castle flyer: before_stage={before_stage} before_holder=(tier={},price={},W={}) before_parent_regen={before_parent_regen} after_stage={} action={} holder=(tier={},price={},W={}) projectile=(subspell={},mana={}) parent_regen={} economy_events={:?}",
                    before_holder.level_gate, before_holder.max_mana, before_holder.active_value,
                    current.level, current.action, holder.level_gate, price, holder.active_value,
                    actual.source_subspell_index, actual.source_mana, raw_parent.mana_regen,
                    sim.dispatch_events().iter().filter(|event| matches!(event, Mc2DispatchEvent::Economy { .. })).collect::<Vec<_>>());
                let selected_profile = live.source_spell_profile(2, holder.level_gate as u8, owner)
                    .expect("actual selected-tier Castle profile for the owner's current stage");
                assert_eq!(price, selected_profile.max_mana,
                    "the source-selected physical tier must carry its current stage price");
                assert!(starting_mana >= price, "the selected physical price must be affordable");
                assert!(before_parent_regen >= 0,
                    "the pre-visit parent must not carry an unrelated negative charge");
                assert_eq!(raw_parent.mana_regen, -price,
                    "the fresh flyer visit must charge exactly the selected physical price");
                assert_eq!(ai.mana_regen, raw_parent.mana_regen,
                    "the source parent charge must reach the rival AI carrier");
                paid = true;
            }
            if flyer.is_none() {
                flyer = sim.source_arena().handle(actual.source_entity_index);
            }
        }
        assert_eq!(sim.source_arena().handle(root.source_entity_index), Some(seed_handle),
            "the upgrade must keep the actual root generation");
        let current = sim.economy().castle_for_owner(owner).expect("owner retains the same Castle");
        if upgrade_product.is_none()
            && let Some(product) = sim.economy().castle_children().iter().find(|child| {
                child.model == 43 && child.parent_entity_index == root.source_entity_index
                    && child.owner_entity_index == owner
            })
        {
            upgrade_product = sim.source_arena().handle(product.source_entity_index);
        }
        saw_root_mailbox |= current.source_word_80 == owner
            || (current.source_word_7c == 10 && current.action == 5);
        if current.level >= 2 && current.construction_slot != 0 && upgrade_child.is_none()
            && let Some(child) = sim.economy().castle_children().iter().find(|child| {
                child.source_entity_index == current.construction_slot && child.active
            })
        {
            assert_eq!((child.model, child.parent_entity_index, child.owner_entity_index),
                (42, root.source_entity_index, owner));
            upgrade_child = sim.source_arena().handle(child.source_entity_index);
        }
        if current.level >= 2 && current.action == 4 {
            upgraded = true;
            break;
        }
    }
    assert!(saw_state1, "the actual scheduled source state1 must run");
    assert!(flyer.is_some(), "the real Castle holder must produce its paid source flyer");
    assert!(paid, "the real Castle holder must charge its owner");
    assert!(upgrade_product.is_some(), "the real flyer must create its owned model43 upgrade product");
    assert!(saw_root_mailbox, "the real model43 product must reach the same Castle root consumer");
    assert!(upgraded, "the original owned Castle must finish a higher-stage construction");
    let child = upgrade_child.expect("the upgrade must create its real linked construction child");
    let product = upgrade_product.unwrap();
    // A completed root can become observable in the same pass that hides a
    // child. UpdateEntities reclaims those rows in its following prepass.
    for _ in 0..16 {
        let sim = campaign.simulation();
        if sim.source_arena().handle(child.source_slot) != Some(child)
            && sim.source_arena().handle(product.source_slot) != Some(product) { break; }
        campaign.step(PlayerCommand::default(), 0);
        assert!(campaign.simulation().dispatch_events().iter()
            .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
        let sim = campaign.simulation();
        let holder = sim.shared_spells().pickup(holder_slot).unwrap();
        let actor = &sim.session.state.live.economy.entities[usize::from(owner)];
        let current_root = sim.economy().castle_for_owner(owner).unwrap();
        eprintln!("upgrade following-reclaim visit: root=(action={},build={},timer={},word30={}) holder=(tier={},W={},duration={},queue={}) actor=(mana={},regen={}) mode={} flyers={:?}",
            current_root.action, current_root.build_state, current_root.timer, current_root.source_word_30,
            holder.level_gate, holder.active_value, holder.activation_value, holder.source_word_2c(),
            actor.mana, actor.mana_regen, sim.session.state.live.nyphur_dispatcher.get(owner_handle).unwrap().state_byte(),
            sim.economy().castle_projectiles().iter().filter(|row| row.owner_entity_index == owner)
                .map(|row| (row.source_entity_index, sim.source_arena().handle(row.source_entity_index), row.active, row.holder_entity_index))
                .collect::<Vec<_>>());
    }
    assert_ne!(campaign.simulation().source_arena().handle(child.source_slot), Some(child),
        "the higher-stage construction child must cross source reclaim");
    assert_ne!(campaign.simulation().source_arena().handle(product.source_slot), Some(product),
        "the actual upgrade product must cross source reclaim");
    let final_sim = campaign.simulation();
    let final_root = final_sim.economy().castle_for_owner(owner).unwrap();
    assert!(final_sim.economy().map_chains().is_linked(final_root.source_entity_index),
        "the completed owned root must remain in the presentation map chain");
    let final_holder = final_sim.shared_spells().pickup(holder_slot).unwrap();
    assert_eq!(final_holder.source_spell_parent_entity_index(), owner);
    if final_holder.active_value != 0 {
        let actor = &final_sim.session.state.live.economy.entities[usize::from(owner)];
        eprintln!("upgrade final active holder: root=(action={},build={},timer={},word30={}) tier={} W={} duration={} queue={} actor=(mana={},regen={}) flyers={:?}",
            final_root.action, final_root.build_state, final_root.timer, final_root.source_word_30,
            final_holder.level_gate, final_holder.active_value, final_holder.activation_value, final_holder.source_word_2c(),
            actor.mana, actor.mana_regen, final_sim.economy().castle_projectiles().iter().filter(|row| row.owner_entity_index == owner)
                .map(|row| (row.source_entity_index, final_sim.source_arena().handle(row.source_entity_index), row.active, row.holder_entity_index))
                .collect::<Vec<_>>());
    }
    let final_live = &final_sim.session.state.live;
    let final_raw = &final_live.economy.entities[usize::from(holder_slot)];
    let final_profile = final_live.source_spell_profile(2, final_holder.level_gate as u8, owner)
        .expect("completed owner Castle has a physical holder profile");
    assert!(saw_queued_tier_zero && saw_queued_tier_applied,
        "the active holder must queue and later apply the actual source tier-zero request");
    assert_eq!(final_holder.level_gate, 0,
        "the holder must retain the source-queued tier after its zero-W cleanup");
    assert_eq!((final_holder.max_mana, final_holder.mana, final_holder.mana_regen),
        (final_profile.max_mana, final_profile.mana, final_profile.mana_regen),
        "the completed Castle reprices the owner's actual holder for its current stage");
    assert_eq!((final_raw.level_gate, final_raw.max_mana, final_raw.mana, final_raw.mana_regen),
        (final_holder.level_gate, final_holder.max_mana, final_holder.mana, final_holder.mana_regen),
        "the source arena and shared holder agree after the upgrade");
    if final_holder.active_value != 0 {
        // The now-correct census can let the still-scheduled rival cast
        // again while the first upgrade's child crosses the reclaim prepass.
        // Accept this only when the actual second flyer and its full current
        // physical price prove a new source activation, not a stale W.
        assert_eq!(final_holder.active_value, final_holder.activation_value as i16 - 1,
            "an active final holder must be the fresh source activation");
        let first_flyer = flyer.expect("the first upgrade flyer retained its generation identity");
        let second = final_sim.economy().castle_projectiles().iter()
            .filter(|row| row.active && row.owner_entity_index == owner && row.holder_entity_index == holder_slot
                && row.target_castle_slot == root.source_entity_index)
            .filter_map(|row| final_sim.source_arena().handle(row.source_entity_index))
            .find(|handle| *handle != first_flyer)
            .expect("the active holder must have a different real owned Castle flyer generation");
        assert_ne!(second, first_flyer);
        assert_eq!(final_live.economy.entities[usize::from(owner)].mana_regen, -final_profile.max_mana,
            "the second source activation must queue its actual current-stage price");
    }
}

#[test]
fn level_two_castles_keep_owner_links_separate_through_destruction_and_reuse() {
    let package = level_two_fixture_path();
    let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level2-day-owner-cache/components/gameplay");
    let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
        .expect("qualified Level 2 owner data");
    let owners = [
        inspection.players()[0].entity_index,
        inspection.players()[1].entity_index,
    ];
    let footprint = inspection
        .level2_castle_stage_footprints
        .get(&1)
        .expect("owner BUILD stage one")
        .clone();
    let mut session = inspection.level2_reset_session().expect("Level 2 session");
    let baseline_hash = session.economy().reset_hash();
    let terrain = session.canonical_terrain().clone();
    let entities = inspection.entities();
    let mut sites = Vec::new();
    for y in (16_u16..240).step_by(16) {
        for x in (16_u16..240).step_by(16) {
            let mut site = Position {
                x: x * 256,
                y: y * 256,
                z: 0,
            };
            site.z = terrain.ground_height(site);
            if !session
                .economy()
                .castle_projectile_site_blocked(site, entities, &terrain)
                && sites.first().is_none_or(|first| {
                    level2_wrapping_xy_distance(*first, site) > 0x3000_u32.pow(2)
                })
            {
                sites.push(site);
            }
            if sites.len() == 2 {
                break;
            }
        }
        if sites.len() == 2 {
            break;
        }
    }
    assert_eq!(
        sites.len(),
        2,
        "owner terrain supplies two valid Castle sites"
    );
    let mut flyers = Vec::new();
    for (owner, site) in owners.into_iter().zip(sites) {
        let mut launch = site;
        launch.z = launch.z.saturating_add(512);
        flyers.push(
            session
                .economy_mut()
                .spawn_castle_projectile_with_payload(
                    Level2CastleProjectileLaunchPayload {
                        position: launch,
                        axis_position: site,
                        yaw: 0,
                        pitch: 0,
                        source_id_entity_index: owner,
                        source_subspell_index: 0,
                        source_mana: 0,
                        source_a4_byte_154: 0,
                    },
                    0,
                    0,
                )
                .expect("ordinary Castle flyer"),
        );
    }
    for _ in 0..32 {
        for &flyer in &flyers {
            session
                .economy_mut()
                .step_castle_projectile_slot(flyer, entities);
        }
        if session.economy().live_castles().count() == 2 {
            break;
        }
    }
    let first = session
        .economy()
        .live_castles()
        .find(|castle| castle.owner_entity_index == owners[0])
        .expect("first flyer produced a Castle seed")
        .source_entity_index;
    let second = session
        .economy()
        .live_castles()
        .find(|castle| castle.owner_entity_index == owners[1])
        .expect("second flyer produced a Castle seed")
        .source_entity_index;
    assert_ne!(first, second);
    assert!(session.economy().castle_for_owner(owners[0]).is_none());
    assert!(session.economy().castle_for_owner(owners[1]).is_none());
    assert!(session.economy_mut().step_castle_action(first));
    assert!(session.economy_mut().step_castle_action(second));
    assert_eq!(
        session
            .economy()
            .castle_for_owner(owners[0])
            .unwrap()
            .source_entity_index,
        first
    );
    assert_eq!(
        session
            .economy()
            .castle_for_owner(owners[1])
            .unwrap()
            .source_entity_index,
        second
    );
    assert_eq!(session.economy().player_castle_entity_index(), first);
    assert_ne!(session.economy().reset_hash(), baseline_hash);
    let destroyed_generation = session.economy().slot_generation(second);
    {
        let mut economy = session.economy_mut();
        let castle = economy.castle_at_mut(second).unwrap();
        castle.action = 6;
        castle.level = 1;
        castle.life = -1;
    }
    assert!(session.economy_mut().step_castle_action(second));
    assert!(session.economy().castle_for_owner(owners[1]).is_none());
    assert_eq!(
        session
            .economy()
            .castle_for_owner(owners[0])
            .unwrap()
            .source_entity_index,
        first
    );
    session
        .economy_mut()
        .reclaim_inactive_slots_at_source_boundary();
    let replacement = session
        .economy_mut()
        .build_castle_with_footprint(
            Position {
                x: 0x4400,
                y: 0x5600,
                z: 0,
            },
            owners[1],
            &footprint,
        )
        .expect("rival rebuild");
    assert_eq!(
        replacement, second,
        "rebuild reuses the reclaimed root slot"
    );
    assert_ne!(
        session.economy().slot_generation(replacement),
        destroyed_generation
    );
    assert!(session.economy().castle_for_owner(owners[1]).is_none());
    assert!(session.economy_mut().step_castle_action(replacement));
    assert_eq!(
        session
            .economy()
            .castle_for_owner(owners[1])
            .unwrap()
            .source_entity_index,
        replacement
    );
    let generation = session.economy().slot_generation(replacement);
    session
        .economy_mut()
        .slot_generations
        .insert(replacement, generation.wrapping_add(1));
    assert!(
        session.economy().castle_for_owner(owners[1]).is_none(),
        "reused root slot cannot satisfy the old relation"
    );
    assert!(session.economy().castle_for_owner(owners[0]).is_some());
    session
        .economy_mut()
        .slot_generations
        .insert(replacement, generation);
    let owner_generation = session.economy().slot_generation(owners[1]);
    session
        .economy_mut()
        .slot_generations
        .insert(owners[1], owner_generation.wrapping_add(1));
    assert!(
        session.economy().castle_for_owner(owners[1]).is_none(),
        "reused owner slot cannot satisfy the old relation"
    );
    session.economy_mut().reset();
    assert_eq!(session.economy().reset_hash(), baseline_hash);
    assert_eq!(session.economy().live_castles().count(), 0);
}

#[test]
fn level_two_possession_contact_retry_preserves_shifted_life_and_prologue_reclaim() {
    let mut session = level_two_possession_lifecycle_session();
    let source_id_entity_index = session.state.entity_index;
    session.activate_level2_authored_switch_group(0, 23, 1);
    let target = session
        .economy()
        .spheres()
        .iter()
        .copied()
        .find(|sphere| {
            sphere.active
                && !sphere.pending_delete
                && u16::try_from(sphere.id).ok() != Some(source_id_entity_index)
                && sphere.player_entity_index != source_id_entity_index
        })
        .expect("fixture has an eligible model-39 possession target");
    session
        .economy_mut()
        .spheres
        .iter_mut()
        .find(|sphere| sphere.source_entity_index == target.source_entity_index)
        .expect("eligible contact target remains mutable")
        .owner_entity_index = source_id_entity_index;
    // `sub_108B0` starts at the rounded map cell while map linking uses
    // the source position's high bytes. Keep this allocation-retry case
    // in class 0; the separate search-order test discriminates the
    // width-derived class-1 backtrack.
    let projectile_position = Position {
        x: target.position.x & 0xff00,
        y: target.position.y & 0xff00,
        z: target.position.z,
    };
    let projectile_slot = session
        .economy_mut()
        .spawn_possession_projectile(
            projectile_position,
            source_id_entity_index,
            116,
            0,
            0,
            -POSSESSION_PROJECTILE_SPEED,
            projectile_position,
        )
        .expect("source arena admits possession flyer");
    {
        let mut economy = session.economy_mut();
        let projectile = economy
            .possession_projectiles
            .iter_mut()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .expect("spawned possession flyer");
        projectile.position = projectile_position;
        projectile.actual_speed = 0;
        economy
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: projectile_slot,
                active: true,
                x: projectile_position.x,
                y: projectile_position.y,
            })
            .expect("flyer source slot links to the live map");
        let projectile = *economy
            .possession_projectiles
            .iter()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .expect("source predicate probe flyer");
        assert_eq!(
            level2_first_possession_target_sphere_slot(
                &economy.map_chains,
                &economy.spheres,
                projectile,
            ),
            Some(target.source_entity_index),
            "an already player-owned target remains eligible when its raw id differs"
        );
        economy
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == target.source_entity_index)
            .expect("source predicate target row")
            .player_entity_index = source_id_entity_index;
        let excluded_target = *economy
            .spheres
            .iter()
            .find(|sphere| sphere.source_entity_index == target.source_entity_index)
            .expect("source predicate target row remains live");
        assert!(
            level2_first_possession_target_sphere_slot(
                &economy.map_chains,
                std::slice::from_ref(&excluded_target),
                projectile,
            )
            .is_none(),
            "the raw playerEntityIndex relation, not owner equality, excludes the target"
        );
        // sub_108B0 continues to other eligible map-chain occupants;
        // excluding this target does not exclude neighboring spheres.
        assert_ne!(
            level2_first_possession_target_sphere_slot(
                &economy.map_chains,
                &economy.spheres,
                projectile,
            ),
            Some(target.source_entity_index)
        );
        economy
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == target.source_entity_index)
            .expect("source predicate target row")
            .player_entity_index = 0;
    }
    let free_after_flyer = session.economy().free_slots.clone();
    let life_before_contact = session
        .economy()
        .possession_projectiles()
        .iter()
        .find(|projectile| projectile.source_entity_index == projectile_slot)
        .expect("live flyer before failed contact")
        .life;
    session.economy_mut().free_slots.clear();

    assert!(
        session
            .economy_mut()
            .step_possession_projectile_slot(projectile_slot),
        "failed model-12 allocation keeps the collision flyer live"
    );
    let retry = session
        .economy()
        .possession_projectiles()
        .iter()
        .find(|projectile| projectile.source_entity_index == projectile_slot)
        .copied()
        .expect("failed contact retains flyer for a later source callback");
    assert!(retry.active);
    assert_eq!(
        retry.life, life_before_contact,
        "contact does not decrement life"
    );
    assert_eq!(
        retry.position.z,
        target.position.z.wrapping_add(target.collision_yaw),
        "source contact copies the target's temporary collision-axis Z without mutating it"
    );
    assert_eq!(
        session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == target.source_entity_index)
            .expect("contact target remains live")
            .position,
        target.position,
        "sub_655A0 restores the model-39 raw position after contact"
    );
    assert!(session.economy().possession_effects().is_empty());

    // The next rounded map-cell scan can still admit this or another
    // authored sphere. Freeing arena capacity therefore legitimately
    // completes the contact retry; a snapped position does not establish
    // the no-target lifetime branch for the complete sphere roster.
    let mut next_probe = retry;
    let ground = session
        .canonical_terrain()
        .ground_height(next_probe.position);
    if ground > next_probe.position.z {
        next_probe.position.z = ground;
    }
    if let Some(ceiling) = session
        .canonical_terrain()
        .ceiling_height(next_probe.position)
    {
        let limit = ceiling.wrapping_sub(POSSESSION_PROJECTILE_COLLISION_HALF_HEIGHT);
        if limit < next_probe.position.z {
            next_probe.position.z = limit;
        }
    }
    let retry_target = {
        let economy = session.economy();
        let slot = level2_first_possession_target_sphere_slot(
            &economy.map_chains,
            &economy.spheres,
            next_probe,
        )
        .expect("the complete source map scan retains an eligible retry contact");
        *economy
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == slot)
            .unwrap()
    };
    let mut contact_retry = session.clone();
    contact_retry.economy_mut().free_slots = free_after_flyer.clone();
    assert!(
        !contact_retry
            .economy_mut()
            .step_possession_projectile_slot(projectile_slot),
        "the actual admitted retry retires only after model12 allocation succeeds"
    );
    let completed = contact_retry
        .economy()
        .possession_projectiles()
        .iter()
        .find(|projectile| projectile.source_entity_index == projectile_slot)
        .copied()
        .unwrap();
    assert!(!completed.active);
    assert_eq!(completed.life, life_before_contact);
    let impact = contact_retry
        .economy()
        .possession_effects()
        .iter()
        .find(|effect| effect.projectile_entity_index == projectile_slot)
        .copied()
        .unwrap();
    assert_eq!(impact.source_id_entity_index, source_id_entity_index);
    assert_eq!(
        impact.position,
        Position {
            z: retry_target
                .position
                .z
                .wrapping_add(retry_target.collision_yaw),
            ..retry_target.position
        }
    );
    assert!(
        contact_retry
            .economy()
            .map_chains()
            .is_linked(projectile_slot)
    );
    assert!(
        !contact_retry
            .economy()
            .free_slots
            .contains(&projectile_slot)
    );

    // Establish a genuine full-roster no-target negative separately:
    // source word94 equality excludes every sphere, while all geometry,
    // health, ownership, arena generations and RNG remain untouched.
    let sphere_fixture = session.economy().spheres().to_vec();
    let generations = sphere_fixture
        .iter()
        .map(|sphere| {
            (
                sphere.source_entity_index,
                session
                    .economy()
                    .slot_generation(sphere.source_entity_index),
            )
        })
        .collect::<Vec<_>>();
    let rng = session.economy().source_rng.clone();
    for sphere in &mut session.economy_mut().spheres {
        sphere.player_entity_index = source_id_entity_index;
    }
    {
        let economy = session.economy();
        for (before, after) in sphere_fixture.iter().zip(economy.spheres()) {
            assert_eq!(
                (
                    after.position,
                    after.life,
                    after.mana,
                    after.owner_entity_index
                ),
                (
                    before.position,
                    before.life,
                    before.mana,
                    before.owner_entity_index
                )
            );
        }
        for (slot, generation) in generations {
            assert_eq!(economy.slot_generation(slot), generation);
        }
        assert_eq!(economy.source_rng, rng);
        assert!(
            level2_first_possession_target_sphere_slot(
                &economy.map_chains,
                &economy.spheres,
                next_probe,
            )
            .is_none(),
            "the complete authored roster now satisfies the raw-relation exclusion"
        );
    }
    session.economy_mut().free_slots = free_after_flyer;
    assert!(
        session
            .economy_mut()
            .step_possession_projectile_slot(projectile_slot),
        "the next ordinary callback takes the established no-target branch"
    );
    let ordinary_next_callback = session
        .economy()
        .possession_projectiles()
        .iter()
        .find(|projectile| projectile.source_entity_index == projectile_slot)
        .copied()
        .expect("ordinary callback keeps the no-longer-contacting flyer live");
    assert!(ordinary_next_callback.active);
    assert_eq!(
        ordinary_next_callback.life,
        life_before_contact.wrapping_sub(1),
        "after leaving target contact, the ordinary no-target branch resumes its signed life decrement"
    );
    assert!(
        session.economy().possession_effects().is_empty(),
        "a fresh callback does not synthesize a second forced target impact"
    );

    let mut retired_from_expiry = false;
    for _ in 0..=usize::try_from(life_before_contact)
        .expect("possession fixture lifetime is non-negative")
    {
        if !session
            .economy_mut()
            .step_possession_projectile_slot(projectile_slot)
        {
            retired_from_expiry = true;
            break;
        }
    }
    assert!(
        retired_from_expiry,
        "the ordinary no-target lifetime eventually allocates its one model-12 effect"
    );
    let retired = session
        .economy()
        .possession_projectiles()
        .iter()
        .find(|projectile| projectile.source_entity_index == projectile_slot)
        .copied()
        .expect("disabled flyer remains represented until the next prologue");
    assert!(!retired.active);
    assert!(
        session.economy().map_chains().is_linked(projectile_slot),
        "DisableEntityDrawing leaves the disabled source row map-linked in this pass"
    );
    assert!(
        !session.economy().free_slots.contains(&projectile_slot),
        "disabled source slot remains unavailable before the next prologue"
    );
    assert_eq!(
        session
            .economy()
            .possession_effects()
            .iter()
            .find(|effect| effect.projectile_entity_index == projectile_slot)
            .expect("eventual no-target expiry creates one model-12 effect")
            .position
            .z,
        target.position.z.wrapping_add(target.collision_yaw),
        "model-12 is created at the shifted contact position"
    );

    session
        .economy_mut()
        .reclaim_possession_rows_at_source_boundary();
    assert!(session.economy().possession_projectiles().is_empty());
    assert!(
        !session.economy().map_chains().is_linked(projectile_slot),
        "the following source-pass prologue alone unlinks the disabled flyer"
    );
    assert!(
        session.economy().free_slots.contains(&projectile_slot),
        "the following source-pass prologue alone releases the slot"
    );
}

#[test]
fn level_two_mana_sphere_claim_synchronizes_the_source_player_relation() {
    let mut session = level_two_possession_lifecycle_session();
    let player = session.state.entity_index;
    session.activate_level2_authored_switch_group(0, 23, 1);
    let target_slot = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.active && !sphere.pending_delete)
        .expect("authored group supplies a mana sphere")
        .source_entity_index;
    let ground = {
        let sphere = session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == target_slot)
            .expect("selected mana sphere remains live");
        session.canonical_terrain().ground_height(sphere.position)
    };
    let baseline_events = session.economy().events().len();
    {
        let mut economy = session.economy_mut();
        let sphere = economy
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == target_slot)
            .expect("selected mana sphere remains mutable");
        sphere.owner_entity_index = 0;
        sphere.player_entity_index = 112;
        sphere.incoming_owner_entity_index = player;
        sphere.incoming_possession_payload = 0;
        sphere.control_flags &= !0x0020_0000;
        sphere.position.z = ground.saturating_add(1_024);
    }

    assert!(
        session.economy_mut().step_mana_sphere_slot(target_slot),
        "the selected source row executes"
    );
    let sphere = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == target_slot)
        .expect("airborne claimed sphere remains live");
    assert_eq!(sphere.owner_entity_index, player);
    assert_eq!(sphere.player_entity_index, player);
    assert_eq!(sphere.incoming_owner_entity_index, 0);
    assert_eq!(session.economy().events().len(), baseline_events + 1);
    assert_eq!(
        session.economy().events().last(),
        Some(&Level2EconomyEvent::ManaSphereClaimed {
            sphere_id: sphere.id,
            owner_entity_index: player,
        })
    );
}

#[test]
fn level_two_mana_sphere_reclaims_contended_raw_relation_before_defender_selection() {
    let mut session = level_two_possession_lifecycle_session();
    let player = session.state.entity_index;
    let rival = player.wrapping_add(1);
    session.activate_level2_authored_switch_group(0, 23, 1);
    let target = *session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.active && !sphere.pending_delete)
        .expect("real authored group supplies a model-39 source");
    let neighbor = *session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.active && sphere.source_entity_index != target.source_entity_index)
        .expect("authored group supplies an untouched neighboring sphere");
    let target_slot = target.source_entity_index;
    let generation = session.economy().slot_generation(target_slot);
    let ground = session.canonical_terrain().ground_height(target.position);
    {
        let mut economy = session.economy_mut();
        let sphere = economy
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == target_slot)
            .unwrap();
        // Actual failed-route state: local semantic owner retained while
        // Nyphur sub_135C0 writes its provisional raw player relation.
        sphere.owner_entity_index = player;
        sphere.player_entity_index = rival;
        sphere.incoming_owner_entity_index = player;
        sphere.incoming_possession_payload = 0;
        sphere.control_flags &= !0x0020_0040;
        sphere.position.z = ground.saturating_add(1_024);
    }
    let before = *session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == target_slot)
        .unwrap();
    let events_before = session.economy().events().len();
    assert_ne!(
        session
            .economy()
            .select_castle_defender_sphere(before.position, player, 0, 0),
        Some(target_slot),
        "semantic ownership alone cannot satisfy the real defender's raw relation gate"
    );
    assert!(session.economy_mut().step_mana_sphere_slot(target_slot));
    let reclaimed = *session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == target_slot)
        .unwrap();
    assert_eq!(
        (reclaimed.owner_entity_index, reclaimed.player_entity_index),
        (player, player)
    );
    assert_eq!(
        (
            reclaimed.incoming_owner_entity_index,
            reclaimed.incoming_possession_payload
        ),
        (0, 0)
    );
    assert_eq!(
        session.economy().events()[events_before..],
        [Level2EconomyEvent::ManaSphereClaimed {
            sphere_id: target.id,
            owner_entity_index: player,
        },]
    );
    assert_eq!(
        session
            .economy()
            .select_castle_defender_sphere(reclaimed.position, player, 0, 0),
        Some(target_slot),
        "the reclaimed source relation feeds the ordinary Castle defender selector"
    );
    assert_eq!(reclaimed.mana, before.mana);
    assert_eq!(reclaimed.entity_rng, before.entity_rng);
    assert_eq!(session.economy().slot_generation(target_slot), generation);
    assert_eq!(
        session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == neighbor.source_entity_index),
        Some(&neighbor)
    );

    // An already-matching raw relation does not publish another claim.
    let events_before = session.economy().events().len();
    session
        .economy_mut()
        .spheres
        .iter_mut()
        .find(|sphere| sphere.source_entity_index == target_slot)
        .unwrap()
        .incoming_owner_entity_index = player;
    assert!(session.economy_mut().step_mana_sphere_slot(target_slot));
    assert_eq!(session.economy().events().len(), events_before);

    // Tier zero still cannot reclaim a protected rival relation. A real
    // nonzero incoming source payload is the existing protected override.
    {
        let mut economy = session.economy_mut();
        let sphere = economy
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == target_slot)
            .unwrap();
        sphere.player_entity_index = rival;
        sphere.control_flags |= 0x0020_0000;
        sphere.incoming_owner_entity_index = player;
    }
    assert!(session.economy_mut().step_mana_sphere_slot(target_slot));
    assert_eq!(session.economy().events().len(), events_before);
    let protected = *session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == target_slot)
        .unwrap();
    assert_eq!(protected.player_entity_index, rival);
    assert_eq!(protected.incoming_owner_entity_index, 0);
    {
        let mut economy = session.economy_mut();
        let sphere = economy
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == target_slot)
            .unwrap();
        sphere.incoming_owner_entity_index = player;
        sphere.incoming_possession_payload = 1;
    }
    assert!(session.economy_mut().step_mana_sphere_slot(target_slot));
    let restored = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == target_slot)
        .unwrap();
    assert_eq!(
        (restored.owner_entity_index, restored.player_entity_index),
        (player, player)
    );
    assert_ne!(restored.control_flags & 0x0020_0000, 0);
    assert_eq!(session.economy().events().len(), events_before + 1);
    assert_eq!(restored.entity_rng, before.entity_rng);
    assert_eq!(session.economy().slot_generation(target_slot), generation);
    assert_eq!(
        session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == neighbor.source_entity_index),
        Some(&neighbor)
    );
}

#[test]
fn model23_mana_sphere_lift_uses_raw_holder_word2c_and_rejects_a_stale_slot() {
    let mut session = crate::native_world::tests::level_two_campaign_session();
    let terrain = session.canonical_terrain().clone();
    let holder_slot = session.state.entity_index;
    let holder_position = Position {
        x: 20_000,
        y: 20_000,
        z: 12_000,
    };
    let sphere = session
        .economy_mut()
        .allocate_authored_model39(
            0,
            0,
            Position {
                z: holder_position.z - 8,
                ..holder_position
            },
        )
        .expect("the ordinary source constructor allocates a sphere");
    let sphere_slot = sphere.source_entity_index;
    let mut entities = session.entities().to_vec();
    let raw_holder = &mut entities[usize::from(holder_slot)];
    raw_holder.active = true;
    raw_holder.index = holder_slot;
    raw_holder.class = 5;
    raw_holder.model = 23;
    raw_holder.position = holder_position;
    raw_holder.record[67] = 5;
    raw_holder.record[68] = 23;
    raw_holder.record[44..46].copy_from_slice(&0x1234_u16.to_le_bytes());
    raw_holder.record[48..50].copy_from_slice(&48_i16.to_le_bytes());
    {
        let mut economy = session.economy_mut();
        let sphere = economy
            .spheres_mut()
            .iter_mut()
            .find(|row| row.source_entity_index == sphere_slot)
            .expect("allocated sphere remains live");
        sphere.position = Position {
            z: holder_position.z - 8,
            ..holder_position
        };
        sphere.vertical_velocity = 0;
        sphere.assigned_defender_slot = holder_slot;
        sphere.control_flags |= 0x40;
    }
    {
        let live = &mut session.session.state.live;
        assert!(live.economy.economy.step_mana_sphere_slot(
            sphere_slot,
            &terrain,
            &entities,
            |_| 105,
        ));
    }
    let lifted = session
        .economy()
        .spheres()
        .iter()
        .find(|row| row.source_entity_index == sphere_slot)
        .expect("lifted sphere remains live");
    assert_eq!(lifted.position.z, holder_position.z + 40);
    assert_ne!(lifted.control_flags & 0x40, 0);
    assert_eq!(lifted.phase, sphere.phase.wrapping_add(1));

    // A reused raw slot must not be rescued by an older typed relation. The
    // failed holder check consumes this visit and preserves its source tail.
    entities[usize::from(holder_slot)].model = 2;
    entities[usize::from(holder_slot)].record[68] = 2;
    let stale_position = Position {
        z: holder_position.z - 8,
        ..holder_position
    };
    {
        let mut economy = session.economy_mut();
        let sphere = economy
            .spheres_mut()
            .iter_mut()
            .find(|row| row.source_entity_index == sphere_slot)
            .expect("sphere remains live for stale-holder case");
        sphere.position = stale_position;
        sphere.vertical_velocity = 0;
        sphere.control_flags |= 0x40;
    }
    let phase_before_stale = session
        .economy()
        .spheres()
        .iter()
        .find(|row| row.source_entity_index == sphere_slot)
        .unwrap()
        .phase;
    {
        let live = &mut session.session.state.live;
        assert!(live.economy.economy.step_mana_sphere_slot(
            sphere_slot,
            &terrain,
            &entities,
            |_| 105,
        ));
    }
    let rejected = session
        .economy()
        .spheres()
        .iter()
        .find(|row| row.source_entity_index == sphere_slot)
        .expect("stale-holder sphere remains live");
    assert_eq!(rejected.position, stale_position);
    assert_eq!(rejected.control_flags & 0x40, 0);
    assert_eq!(rejected.phase, phase_before_stale.wrapping_add(1));
}

#[test]
fn level_two_possession_expiry_retries_model12_allocation() {
    let mut session = level_two_possession_lifecycle_session();
    let source_id_entity_index = session.state.entity_index;
    let no_target_position = Position {
        x: 32_000,
        y: 32_000,
        z: 30_000,
    };
    let projectile_slot = session
        .economy_mut()
        .spawn_possession_projectile(
            no_target_position,
            source_id_entity_index,
            116,
            0,
            0,
            -POSSESSION_PROJECTILE_SPEED,
            no_target_position,
        )
        .expect("source arena admits possession flyer");
    {
        let mut economy = session.economy_mut();
        let projectile = {
            let projectile = economy
                .possession_projectiles
                .iter_mut()
                .find(|projectile| projectile.source_entity_index == projectile_slot)
                .expect("spawned possession flyer");
            projectile.position = no_target_position;
            projectile.actual_speed = 0;
            projectile.life = 0;
            *projectile
        };
        assert!(
            level2_first_possession_target_sphere_slot(
                &economy.map_chains,
                &economy.spheres,
                projectile,
            )
            .is_none(),
            "expiry fixture is outside every eligible model-39 collision volume"
        );
        economy
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: projectile_slot,
                active: true,
                x: no_target_position.x,
                y: no_target_position.y,
            })
            .expect("expiry flyer source slot links to the live map");
    }
    let free_after_flyer = session.economy().free_slots.clone();
    session.economy_mut().free_slots.clear();

    assert!(
        session
            .economy_mut()
            .step_possession_projectile_slot(projectile_slot),
        "expired flyer remains live when model-12 allocation fails"
    );
    let retry = session
        .economy()
        .possession_projectiles()
        .iter()
        .find(|projectile| projectile.source_entity_index == projectile_slot)
        .copied()
        .expect("failed expiry retains flyer");
    assert_eq!(
        retry.life, -1,
        "no-target expiry decrements before allocation"
    );
    assert!(session.economy().possession_effects().is_empty());

    session.economy_mut().free_slots = free_after_flyer;
    assert!(
        !session
            .economy_mut()
            .step_possession_projectile_slot(projectile_slot),
        "the expiry branch retries model-12 allocation on its next callback"
    );
    assert!(
        session
            .economy()
            .possession_effects()
            .iter()
            .any(|effect| effect.projectile_entity_index == projectile_slot),
        "no-target expiry produces the same typed model-12 effect"
    );
}

#[test]
fn level_two_possession_strict_terrain_and_cave_clamps_still_decrement_life() {
    fn assert_no_target_clamp_steps_lifetime(
        session: &mut Mc2Simulation,
        position: Position,
        expected_z: i16,
        boundary_case: &str,
    ) {
        let source_id_entity_index = session.state.entity_index;
        let projectile_slot = session
            .economy_mut()
            .spawn_possession_projectile(
                position,
                source_id_entity_index,
                116,
                0,
                0,
                -POSSESSION_PROJECTILE_SPEED,
                position,
            )
            .expect("source arena admits possession flyer");
        {
            let mut economy = session.economy_mut();
            let projectile = {
                let projectile = economy
                    .possession_projectiles
                    .iter_mut()
                    .find(|projectile| projectile.source_entity_index == projectile_slot)
                    .expect("spawned possession flyer");
                projectile.position = position;
                projectile.actual_speed = 0;
                projectile.life = 4;
                *projectile
            };
            assert!(
                level2_first_possession_target_sphere_slot(
                    &economy.map_chains,
                    &economy.spheres,
                    projectile,
                )
                .is_none(),
                "{boundary_case} fixture is outside every eligible model-39 collision volume"
            );
            economy
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: projectile_slot,
                    active: true,
                    x: position.x,
                    y: position.y,
                })
                .expect("contact flyer source slot links to the live map");
        }

        assert!(
            session
                .economy_mut()
                .step_possession_projectile_slot(projectile_slot),
            "an initial {boundary_case} clamp keeps the no-target flyer live"
        );
        let stepped = session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .copied()
            .expect("clamped flyer remains live after its ordinary callback");
        assert!(stepped.active);
        assert_eq!(
            stepped.life, 3,
            "{boundary_case} clamps still take the signed no-target life decrement"
        );
        assert_eq!(
            stepped.position.z, expected_z,
            "{boundary_case} applies only its strict source Z clamp"
        );
        assert!(
            !session
                .economy()
                .possession_effects()
                .iter()
                .any(|effect| effect.projectile_entity_index == projectile_slot),
            "{boundary_case} does not manufacture model 12 before target, post-scan conflict, or expiry"
        );
    }

    let contact_position = Position {
        x: 32_128,
        y: 32_000,
        z: 1_599,
    };
    let mut ground_session = level_two_possession_lifecycle_session();
    ground_session
        .canonical_terrain_mut()
        .planes
        .ground_heights
        .fill(50);
    assert_no_target_clamp_steps_lifetime(
        &mut ground_session,
        contact_position,
        1_600,
        "below ground",
    );
    assert_no_target_clamp_steps_lifetime(
        &mut ground_session,
        Position {
            z: 1_600,
            ..contact_position
        },
        1_600,
        "equal ground",
    );

    let mut cave_session = level_two_possession_lifecycle_session();
    {
        let terrain = cave_session.canonical_terrain_mut();
        terrain.map_type = MapType::Cave;
        terrain.planes.ground_heights.fill(0);
        terrain.planes.secondary_heights.fill(255);
    }
    let cave_limit = 8_160_i16.wrapping_sub(POSSESSION_PROJECTILE_COLLISION_HALF_HEIGHT);
    assert_no_target_clamp_steps_lifetime(
        &mut cave_session,
        Position {
            z: 8_000,
            ..contact_position
        },
        cave_limit,
        "above cave limit",
    );
    assert_no_target_clamp_steps_lifetime(
        &mut cave_session,
        Position {
            z: cave_limit,
            ..contact_position
        },
        cave_limit,
        "equal cave limit",
    );
}

#[test]
fn level_two_possession_searches_class0_then_class1_cells_before_first_target() {
    let mut session = level_two_possession_lifecycle_session();
    let source_id_entity_index = session.state.entity_index;
    let projectile_position = Position {
        x: 32_000,
        y: 32_000,
        z: 30_000,
    };
    let projectile_slot = session
        .economy_mut()
        .spawn_possession_projectile(
            projectile_position,
            source_id_entity_index,
            116,
            0,
            0,
            -POSSESSION_PROJECTILE_SPEED,
            projectile_position,
        )
        .expect("source arena admits possession flyer");
    {
        let mut economy = session.economy_mut();
        let projectile = economy
            .possession_projectiles
            .iter_mut()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .expect("spawned possession flyer");
        projectile.position = projectile_position;
        projectile.actual_speed = 0;
        economy
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: projectile_slot,
                active: true,
                x: projectile_position.x,
                y: projectile_position.y,
            })
            .expect("projectile source row links to the live map");
    }
    let class1_only_position = Position {
        x: projectile_position.x.wrapping_sub(1),
        ..projectile_position
    };
    let class1_only = session
        .economy_mut()
        .allocate_authored_model39(0, 23, class1_only_position)
        .expect("source arena admits west-neighbour class-1 candidate");
    {
        let mut economy_view = session.economy_mut();
        let candidate = economy_view
            .spheres
            .iter_mut()
            .find(|sphere| {
                sphere.source_entity_index == class1_only.source_entity_index
                    && sphere.position == class1_only_position
            })
            .expect("west-neighbour class-1 candidate remains mutable");
        candidate.id = 0xfeed;
        candidate.player_entity_index = source_id_entity_index.wrapping_add(1);
    }
    let projectile = session
        .economy()
        .possession_projectiles()
        .iter()
        .copied()
        .find(|projectile| projectile.source_entity_index == projectile_slot)
        .expect("live possession flyer");
    let mut economy = session.economy();
    assert!(
        source_map_search_slots(
            economy.map_chains(),
            projectile_position,
            &LEVEL2_MANA_SPHERE_SEARCH_OFFSETS,
        )
        .contains(&class1_only.source_entity_index),
        "the discriminating candidate is visible to the class-1 sphere walk"
    );
    assert!(
        source_map_search_slots(
            economy.map_chains(),
            projectile_position,
            &LEVEL2_POSSESSION_PROJECTILE_SEARCH_OFFSETS,
        )
        .contains(&class1_only.source_entity_index),
        "sub_108B0's width-derived class-1 envelope includes the west neighbour"
    );
    assert_eq!(
        level2_first_possession_target_sphere_slot(
            economy.map_chains(),
            economy.spheres(),
            projectile
        ),
        Some(class1_only.source_entity_index),
        "an eligible class-1 model-39 is a Possession target when class 0 is empty"
    );

    let class0_first = session
        .economy_mut()
        .allocate_authored_model39(0, 23, projectile_position)
        .expect("source arena admits first class-0 candidate");
    {
        let mut economy_view = session.economy_mut();
        let candidate = economy_view
            .spheres
            .iter_mut()
            .find(|sphere| {
                sphere.source_entity_index == class0_first.source_entity_index
                    && sphere.position == projectile_position
            })
            .expect("class-0 candidate remains mutable");
        candidate.id = 0xbeef;
        candidate.player_entity_index = source_id_entity_index.wrapping_add(1);
    }
    let mut economy = session.economy();
    assert!(
        source_map_search_slots(
            economy.map_chains(),
            projectile_position,
            &LEVEL2_POSSESSION_PROJECTILE_SEARCH_OFFSETS,
        )
        .contains(&class0_first.source_entity_index),
        "first class-0 candidate must be in the possession envelope"
    );
    assert_eq!(
        level2_first_possession_target_sphere_slot(
            economy.map_chains(),
            economy.spheres(),
            projectile
        ),
        Some(class0_first.source_entity_index),
        "class-0 map cells retain source search priority over class-1 cells"
    );
}

#[test]
fn level_two_possession_guidance_acquires_once_turns_then_keeps_stale_slot() {
    let mut session = level_two_possession_lifecycle_session();
    let baseline_hash = session.economy().reset_hash();
    let source_id_entity_index = session.state.entity_index;
    let projectile_position = Position {
        x: 32_000,
        y: 32_000,
        z: 24_000,
    };
    let first_position = Position {
        x: projectile_position.x.wrapping_sub(128),
        y: projectile_position.y.wrapping_sub(2_000),
        z: projectile_position.z,
    };
    let second_position = Position {
        x: projectile_position.x.wrapping_add(128),
        ..first_position
    };
    let first = session
        .economy_mut()
        .allocate_authored_model39(0, 65, first_position)
        .expect("first equal-score model-39 guidance target");
    let second = session
        .economy_mut()
        .allocate_authored_model39(0, 65, second_position)
        .expect("second equal-score model-39 guidance target");
    assert!(first.source_entity_index < second.source_entity_index);
    for slot in [first.source_entity_index, second.source_entity_index] {
        let mut economy_view = session.economy_mut();
        let sphere = economy_view
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == slot)
            .expect("guidance candidate remains represented");
        sphere.player_entity_index = source_id_entity_index.wrapping_add(1);
        // Vertical settling does not imply horizontal rest on sloped
        // terrain; Possession must use the sphere's current position.
        sphere.velocity_x = 32;
        sphere.velocity_y = 0;
        sphere.vertical_velocity = 0;
    }
    session
        .economy_mut()
        .rebuild_possession_guidance_source_slots();
    let projectile_slot = session
        .economy_mut()
        .spawn_possession_projectile(
            projectile_position,
            source_id_entity_index,
            116,
            0,
            0,
            0,
            projectile_position,
        )
        .expect("guided Possession flyer allocation");
    {
        let mut economy_view = session.economy_mut();
        let projectile = economy_view
            .possession_projectiles
            .iter_mut()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .expect("guided flyer remains represented");
        assert_eq!(projectile.position, projectile_position);
    }
    let before_first = session
        .economy()
        .possession_projectiles()
        .iter()
        .find(|projectile| projectile.source_entity_index == projectile_slot)
        .copied()
        .expect("guided flyer before first callback");
    let first_target_top = level2_possession_guidance_target_top(
        session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == first.source_entity_index)
            .expect("lower-slot guidance target"),
    );
    let expected_first_angles = projectile_guidance_angles(before_first.position, first_target_top);
    let mut expected_first_position = before_first.position;
    crate::move_entity(
        &mut expected_first_position,
        expected_first_angles.0,
        expected_first_angles.1,
        before_first.actual_speed,
    );

    assert!(
        session
            .economy_mut()
            .step_possession_projectile_slot(projectile_slot),
        "the first guided callback remains in flight"
    );
    let after_first = session
        .economy()
        .possession_projectiles()
        .iter()
        .find(|projectile| projectile.source_entity_index == projectile_slot)
        .copied()
        .expect("guided flyer after first callback");
    assert!(after_first.acquisition_attempted);
    assert_eq!(
        after_first.target_source_entity_index, first.source_entity_index,
        "strict-lower equal-score selection retains the lower source slot"
    );
    assert_eq!((after_first.roll, after_first.fov), expected_first_angles);
    assert_eq!((after_first.yaw, after_first.pitch), expected_first_angles);
    assert_eq!(
        after_first.position, expected_first_position,
        "first acquisition copies desired angles before the first move"
    );

    let moved_target_position = Position {
        x: after_first.position.x.wrapping_add(2_000),
        y: after_first.position.y.wrapping_sub(400),
        z: after_first.position.z,
    };
    session
        .economy_mut()
        .spheres
        .iter_mut()
        .find(|sphere| sphere.source_entity_index == first.source_entity_index)
        .expect("selected guidance target before second callback")
        .position = moved_target_position;
    let moved_target_top = level2_possession_guidance_target_top(
        session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == first.source_entity_index)
            .expect("moved guidance target"),
    );
    let expected_second_angles = projectile_guidance_angles(after_first.position, moved_target_top);
    let expected_second_yaw = after_first.yaw.wrapping_add(turn_toward(
        after_first.yaw,
        expected_second_angles.0,
        PROJECTILE_ACQUISITION_ANGLE,
    )) & 0x07ff;
    let expected_second_pitch = after_first.pitch.wrapping_add(turn_toward(
        after_first.pitch,
        expected_second_angles.1,
        PROJECTILE_ACQUISITION_ANGLE,
    )) & 0x07ff;
    assert_ne!(
        expected_second_yaw, expected_second_angles.0,
        "the moved target requires a capped rather than snapping yaw turn"
    );
    assert!(
        session
            .economy_mut()
            .step_possession_projectile_slot(projectile_slot),
        "the second guided callback remains in flight"
    );
    let after_second = session
        .economy()
        .possession_projectiles()
        .iter()
        .find(|projectile| projectile.source_entity_index == projectile_slot)
        .copied()
        .expect("guided flyer after second callback");
    assert_eq!(
        (after_second.roll, after_second.fov),
        expected_second_angles
    );
    assert_eq!(after_second.yaw, expected_second_yaw);
    assert_eq!(after_second.pitch, expected_second_pitch);

    session
        .economy_mut()
        .spheres
        .iter_mut()
        .find(|sphere| sphere.source_entity_index == first.source_entity_index)
        .expect("selected target before reclaim")
        .active = false;
    session.economy_mut().reclaim_inactive_slots();
    let stale_target_top = session
        .economy()
        .possession_guidance_stale_records
        .get(&first.source_entity_index)
        .copied()
        .expect("source fixed-slot record survives model-39 reclamation");
    let expected_stale_angles = projectile_guidance_angles(after_second.position, stale_target_top);
    let _ = session
        .economy_mut()
        .step_possession_projectile_slot(projectile_slot);
    let after_reclaim = session
        .economy()
        .possession_projectiles()
        .iter()
        .find(|projectile| projectile.source_entity_index == projectile_slot)
        .copied()
        .expect("guided flyer remains represented after target reclaim");
    assert!(after_reclaim.acquisition_attempted);
    assert_eq!(
        after_reclaim.target_source_entity_index, first.source_entity_index,
        "a stale source relation never reacquires the remaining candidate"
    );
    assert_ne!(
        after_reclaim.target_source_entity_index, second.source_entity_index,
        "candidate B cannot replace the one-shot source relation"
    );
    assert_eq!(
        (after_reclaim.roll, after_reclaim.fov),
        expected_stale_angles,
        "a reclaimed source slot continues steering through its stale fixed record"
    );

    let replacement_position = Position {
        x: after_reclaim.position.x.wrapping_add(600),
        y: after_reclaim.position.y.wrapping_sub(1_500),
        z: after_reclaim.position.z.wrapping_add(37),
    };
    let replacement_slot = session
        .economy_mut()
        .spawn_possession_projectile(
            replacement_position,
            source_id_entity_index,
            116,
            0,
            0,
            0,
            Position {
                z: replacement_position.z.wrapping_add(999),
                ..replacement_position
            },
        )
        .expect("released guidance slot can be overwritten by an ordinary model-1 NewEvent");
    assert_eq!(replacement_slot, first.source_entity_index);
    let replacement_top = replacement_position;
    let expected_replacement_angles =
        projectile_guidance_angles(after_reclaim.position, replacement_top);
    let _ = session
        .economy_mut()
        .step_possession_projectile_slot(projectile_slot);
    let after_reuse = session
        .economy()
        .possession_projectiles()
        .iter()
        .find(|projectile| projectile.source_entity_index == projectile_slot)
        .copied()
        .expect("guided flyer follows the reused source record");
    assert_eq!(
        (after_reuse.roll, after_reuse.fov),
        expected_replacement_angles,
        "slot reuse changes the existing relation's desired angles without reacquisition"
    );
    assert!(after_reuse.acquisition_attempted);
    assert_eq!(after_reuse.target_source_entity_index, replacement_slot);
    assert_ne!(session.economy().reset_hash(), baseline_hash);
    session.reset();
    assert_eq!(session.economy().reset_hash(), baseline_hash);
}

#[test]
fn level_two_possession_holder_charges_once_then_clears_positive_cooldown_regen() {
    let mut session = level_two_possession_lifecycle_session();
    let holder_index = 116;
    let (activation_value, mana_cost) = {
        let mut shared_view = session.shared_spells_mut();
        let holder = shared_view
            .dynamic_pickups
            .iter_mut()
            .find(|holder| holder.index == holder_index)
            .expect("ordinary Level 2 Possession holder");
        let activation_value = i16::try_from(holder.activation_value)
            .expect("ordinary Possession activation value fits signed source field");
        holder.active = true;
        holder.active_value = activation_value;
        holder.life = 0;
        (activation_value, holder.mana_cost)
    };
    assert!(mana_cost > 0, "fixture retains a positive source mana cost");
    {
        let mut shared = session.shared_spells_mut();
        shared.player_entity.mana_regen = 37;
        shared.player_entity.record[140..144].copy_from_slice(&37_i32.to_le_bytes());
        let player_index = shared.player_entity.index;
        for entity in &mut shared.player_entities {
            if entity.index == player_index {
                entity.mana_regen = 37;
                entity.record[140..144].copy_from_slice(&37_i32.to_le_bytes());
            }
        }
    }

    session.step_level2_possession_holder_source_slot(holder_index);
    assert_eq!(
        session.shared_spells().player_entity.mana_regen,
        -mana_cost,
        "the activation tick applies the one source mana charge"
    );
    assert_eq!(
        session
            .shared_spells()
            .dynamic_pickups
            .iter()
            .find(|holder| holder.index == holder_index)
            .expect("active possession holder after source callback")
            .active_value,
        activation_value - 1,
        "spawn attempt still precedes the source countdown decrement"
    );
    assert_eq!(
        session
            .economy()
            .possession_projectiles()
            .iter()
            .filter(|projectile| projectile.holder_entity_index == holder_index)
            .count(),
        1,
        "the activation tick creates exactly one possession flyer"
    );

    {
        let mut shared = session.shared_spells_mut();
        shared.player_entity.mana_regen = 37;
        shared.player_entity.record[140..144].copy_from_slice(&37_i32.to_le_bytes());
        let player_index = shared.player_entity.index;
        for entity in &mut shared.player_entities {
            if entity.index == player_index {
                entity.mana_regen = 37;
                entity.record[140..144].copy_from_slice(&37_i32.to_le_bytes());
            }
        }
    }
    session.step_level2_possession_holder_source_slot(holder_index);
    assert_eq!(
        session.shared_spells().player_entity.mana_regen,
        0,
        "a positive regeneration value is cleared on a later active cooldown tick"
    );
    assert_eq!(
        session
            .economy()
            .possession_projectiles()
            .iter()
            .filter(|projectile| projectile.holder_entity_index == holder_index)
            .count(),
        1,
        "cooldown ticks do not create a second possession flyer"
    );
}

#[test]
fn level_two_possession_effect_writes_every_overlapping_live_sphere_mailbox() {
    let mut session = level_two_possession_lifecycle_session();
    let source_id_entity_index = session.state.entity_index;
    let probe_position = Position {
        x: 32_000,
        y: 32_000,
        z: 0,
    };
    let collision_position = Position {
        z: session.canonical_terrain().ground_height(probe_position),
        ..probe_position
    };
    let first = session
        .economy_mut()
        .allocate_authored_model39(0, 23, collision_position)
        .expect("first map-linked model-39 source for Possession effect fan-out");
    let second = session
        .economy_mut()
        .allocate_authored_model39(0, 23, collision_position)
        .expect("second live model-39 source in the same effect volume");
    for (sphere_slot, fixture_id) in [
        (first.source_entity_index, 0xfeed_0001),
        (second.source_entity_index, 0xfeed_0002),
    ] {
        let mut economy_view = session.economy_mut();
        let sphere = economy_view
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == sphere_slot)
            .expect("map-linked fan-out fixture remains mutable");
        sphere.id = fixture_id;
        sphere.player_entity_index = source_id_entity_index.wrapping_add(1);
    }
    let projectile_slot = session
        .economy_mut()
        .spawn_possession_projectile(
            collision_position,
            source_id_entity_index,
            116,
            0,
            0,
            -POSSESSION_PROJECTILE_SPEED,
            collision_position,
        )
        .expect("source arena admits possession flyer");
    {
        let mut economy = session.economy_mut();
        let projectile = economy
            .possession_projectiles
            .iter_mut()
            .find(|projectile| projectile.source_entity_index == projectile_slot)
            .expect("spawned possession flyer");
        projectile.position = collision_position;
        projectile.actual_speed = 0;
        economy
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: projectile_slot,
                active: true,
                x: collision_position.x,
                y: collision_position.y,
            })
            .expect("flyer source slot links to the live map");
    }
    assert!(
        !session
            .economy_mut()
            .step_possession_projectile_slot(projectile_slot),
        "a real model-39 target creates model-12 through the flyer callback"
    );
    let effect_slot = session
        .economy()
        .possession_effects()
        .iter()
        .find(|effect| effect.projectile_entity_index == projectile_slot)
        .expect("target collision effect")
        .source_entity_index;
    assert!(
        session
            .economy_mut()
            .step_possession_effect_slot(effect_slot),
        "first model-12 callback remains live after mailbox writes"
    );
    for sphere_slot in [first.source_entity_index, second.source_entity_index] {
        let sphere = session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == sphere_slot)
            .expect("overlapping map-linked model-39 source");
        assert_eq!(
            sphere.incoming_owner_entity_index, source_id_entity_index,
            "model-12 writes its source id to every overlapping sphere, not only its collision target"
        );
        assert_eq!(sphere.incoming_possession_payload, 0);
    }
}

#[test]
fn level_two_possession_new_rows_follow_the_remaining_ascending_source_cursor() {
    fn run_case(target_after_projectile: bool) -> (u16, u16, u16, u16) {
        let mut session = level_two_possession_lifecycle_session();
        // Let the owned Level 2 bootstrap materialize its pre-existing
        // source rows first. The cursor cases below then isolate a real
        // later `NewEvent` insertion without competing with unrelated
        // first-pass authored constructors.
        session.step_level2_source_pass(0);
        let source_id_entity_index = session.state.entity_index;
        let mut static_route_slots = BTreeSet::from([source_id_entity_index, 38_u16, 75_u16]);
        static_route_slots.extend(
            session
                .shared_spells
                .dynamic_pickups
                .iter()
                .filter(|holder| {
                    holder.active
                        && holder.class == 15
                        && holder.model == POSSESSION_SPELL_FAMILY
                        && holder.action == POSSESSION_SPELL_FAMILY.saturating_mul(3)
                })
                .map(|holder| holder.index),
        );
        static_route_slots.extend(
            session
                .economy
                .entities
                .iter()
                .filter(|entity| {
                    entity.active && entity.class == 3 && entity.model == 1 && entity.action == 1
                })
                .map(|entity| entity.index),
        );
        static_route_slots.extend(session.bad_stones.iter().map(|stone| stone.entity.index));
        static_route_slots.extend(
            session
                .ordinary
                .houses()
                .iter()
                .map(|house| house.entity_index),
        );
        static_route_slots.extend(
            session
                .ordinary
                .villagers()
                .iter()
                .map(|villager| villager.entity_index),
        );
        static_route_slots.extend(
            session
                .ordinary
                .dolmens()
                .iter()
                .map(|dolmen| dolmen.entity.index),
        );
        static_route_slots.extend(
            session
                .economy()
                .castle()
                .filter(|castle| castle.active)
                .map(|castle| castle.source_entity_index),
        );
        static_route_slots.extend(LEVEL_TWO_GOAT_INDICES);
        let mut slots = session
            .economy()
            .free_slots
            .iter()
            .copied()
            .filter(|slot| !static_route_slots.contains(slot))
            .collect::<Vec<_>>();
        slots.sort_unstable();
        assert!(
            slots.len() >= 3,
            "fixture provides three non-static source slots for cursor ordering"
        );
        let (projectile_slot, effect_slot, target_slot) = if target_after_projectile {
            (slots[0], slots[1], slots[2])
        } else {
            (slots[1], slots[2], slots[0])
        };
        let mut position = Position {
            x: 30_000,
            y: 30_000,
            z: 0,
        };
        position.z = session.canonical_terrain().ground_height(position);
        {
            let mut economy = session.economy_mut();
            economy.spheres.push(ManaSphere {
                id: u32::from(target_slot),
                source_entity_index: target_slot,
                action: 41,
                entity_rng: 0,
                position,
                velocity_x: 0,
                velocity_y: 0,
                vertical_velocity: 0,
                yaw: 0,
                max_life: 300,
                life: 300,
                mana: 512,
                particle_index: 53,
                collision_yaw: 28,
                collision_pitch: 28,
                collision_roll: 28,
                collision_fov: 28,
                owner_entity_index: 0,
                player_entity_index: 0,
                incoming_owner_entity_index: 0,
                incoming_possession_payload: 0,
                ordinary_damage_payload: 0,
                ordinary_damage_owner: 0,
                attraction_source_entity_index: 0,
                attraction_step: 0,
                attraction_distance_high: 0,
                assigned_carrier_id: 0,
                assigned_defender_slot: 0,
                control_flags: 0,
                phase: 0,
                active: true,
                pending_delete: false,
            });
            economy
                .possession_projectiles
                .push(Level2PossessionProjectile {
                    source_entity_index: projectile_slot,
                    entity_rng: 0,
                    source_id_entity_index,
                    holder_entity_index: 116,
                    source_class: POSSESSION_PROJECTILE_CLASS,
                    source_model: POSSESSION_PROJECTILE_MODEL,
                    action: POSSESSION_PROJECTILE_ACTION,
                    output_class: POSSESSION_IMPACT_CLASS,
                    output_model: POSSESSION_IMPACT_MODEL,
                    target_source_entity_index: 0,
                    acquisition_attempted: false,
                    threat_accounted: false,
                    position,
                    axis_position: position,
                    yaw: 0,
                    pitch: 0,
                    roll: 0,
                    fov: 0,
                    actual_speed: 0,
                    minimum_speed: POSSESSION_PROJECTILE_SPEED,
                    life: POSSESSION_PROJECTILE_LIFETIME_TICKS,
                    max_life: POSSESSION_PROJECTILE_LIFETIME_TICKS,
                    phase: 0,
                    active: true,
                });
            economy
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: target_slot,
                    active: true,
                    x: position.x,
                    y: position.y,
                })
                .expect("synthetic model-39 is map-linked in source order");
            economy
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: projectile_slot,
                    active: true,
                    x: position.x,
                    y: position.y,
                })
                .expect("synthetic possession flyer is map-linked in source order");
            // `allocate_slot` pops the last element. Force the effect
            // into the source slot between or after the two existing rows
            // without changing the actual flyer/effect constructor.
            economy.free_slots = vec![effect_slot];
            let projectile = economy
                .possession_projectiles
                .iter()
                .find(|projectile| projectile.source_entity_index == projectile_slot)
                .copied()
                .expect("synthetic possession flyer remains live before source pass");
            assert_eq!(
                level2_first_possession_target_sphere_slot(
                    &economy.map_chains,
                    &economy.spheres,
                    projectile,
                ),
                Some(target_slot),
                "the controlled source-map fixture has one eligible collision target"
            );
            assert!(
                economy
                    .live_arena_cursor_rows()
                    .iter()
                    .any(|row| row.source_slot == projectile_slot),
                "the controlled flyer is visible to the initial dynamic cursor"
            );
        }

        session.step_level2_source_pass(1);
        let sphere_after_first = session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == target_slot)
            .expect("target sphere remains in the source arena after first pass");
        let owner_after_first = sphere_after_first.owner_entity_index;
        let mailbox_after_first = sphere_after_first.incoming_owner_entity_index;
        assert!(
            session
                .economy()
                .possession_effects()
                .iter()
                .any(|effect| effect.source_entity_index == effect_slot),
            "real collision allocation consumes the forced effect slot"
        );
        session.step_level2_source_pass(2);
        let owner_after_second = session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == target_slot)
            .expect("target sphere remains in the source arena after second pass")
            .owner_entity_index;
        (
            source_id_entity_index,
            owner_after_first,
            mailbox_after_first,
            owner_after_second,
        )
    }

    let (source_id, higher_owner, higher_mailbox, higher_owner_next) = run_case(true);
    assert_eq!(
        higher_owner, source_id,
        "S>E consumes the model-12 mailbox in the same source pass"
    );
    assert_eq!(
        higher_mailbox, 0,
        "same-pass sphere action clears its mailbox"
    );
    assert_eq!(higher_owner_next, source_id);

    let (source_id, lower_owner, lower_mailbox, lower_owner_next) = run_case(false);
    assert_ne!(
        lower_owner, source_id,
        "S<E has already passed its model-39 action when the effect is born"
    );
    assert_eq!(
        lower_mailbox, source_id,
        "S<E receives the source-id mailbox but cannot consume it early"
    );
    assert_eq!(
        lower_owner_next, source_id,
        "the following ascending source pass consumes the deferred mailbox"
    );
}

#[test]
fn level_two_possession_claim_can_handoff_to_later_castle_root_in_the_same_pass() {
    let mut session = level_two_possession_lifecycle_session();
    let owner_entity_index = session.state.entity_index;
    let position = Position {
        x: 32_000,
        y: 32_000,
        z: 0,
    };
    let position = Position {
        z: session.canonical_terrain().ground_height(position),
        ..position
    };
    let mut slots = session.economy().free_slots.clone();
    slots.retain(|slot| *slot != owner_entity_index);
    slots.sort_unstable();
    let [effect_slot, projectile_slot, sphere_slot, castle_slot] =
        slots[..4].try_into().expect("four ordered dynamic slots");

    session.economy_mut().free_slots = vec![castle_slot];
    assert_eq!(
        session
            .economy_mut()
            .build_castle(position, owner_entity_index),
        Some(castle_slot)
    );
    session
        .economy_mut()
        .castle
        .as_mut()
        .expect("stage-one Castle")
        .phase = 0;

    session.economy_mut().free_slots = vec![sphere_slot];
    let sphere_id = session
        .economy_mut()
        .spawn_mana_sphere_from_death(101, position, 0, 0x1234, 500, 0)
        .expect("neutral Archer death sphere");
    assert_eq!(sphere_id, u32::from(sphere_slot));
    let sphere_generation = session.economy().slot_generation(sphere_slot);
    {
        let mut economy = session.economy_mut();
        let sphere = economy
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == sphere_slot)
            .expect("neutral sphere remains mutable");
        sphere.position = position;
        sphere.velocity_x = 0;
        sphere.velocity_y = 0;
        sphere.vertical_velocity = 0;
        economy
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: sphere_slot,
                active: true,
                x: position.x,
                y: position.y,
            })
            .expect("neutral sphere remains map-linked");
    }

    session.economy_mut().free_slots = vec![projectile_slot];
    assert_eq!(
        session.economy_mut().spawn_possession_projectile(
            position,
            owner_entity_index,
            116,
            0,
            0,
            0,
            position,
        ),
        Some(projectile_slot)
    );
    {
        let mut economy = session.economy_mut();
        let projectile = {
            let projectile = economy
                .possession_projectiles
                .iter_mut()
                .find(|projectile| projectile.source_entity_index == projectile_slot)
                .expect("Possession flyer remains mutable before contact");
            projectile.position = position;
            projectile.actual_speed = 0;
            *projectile
        };
        economy
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: projectile_slot,
                active: true,
                x: position.x,
                y: position.y,
            })
            .expect("Possession flyer remains map-linked");
        economy.free_slots = vec![effect_slot];
        assert_eq!(
            level2_first_possession_target_sphere_slot(
                &economy.map_chains,
                &economy.spheres,
                projectile,
            ),
            Some(sphere_slot),
            "the controlled contact fixture exposes the neutral sphere to the flyer"
        );
    }
    assert!(
        !session
            .economy_mut()
            .step_possession_projectile_slot(projectile_slot),
        "strict contact allocates the real model-12 effect"
    );
    assert!(
        session
            .economy()
            .possession_effects()
            .iter()
            .any(|effect| effect.source_entity_index == effect_slot)
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_ne!(
        session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == sphere_slot)
            .expect("same-pass handoff retains the hidden source row")
            .vertical_velocity,
        0,
        "the ordinary sphere callback advances vertical physics before the handoff"
    );
    let events = session.economy().events();
    let claim_index = events
        .iter()
        .position(|event| {
            matches!(
                event,
                Level2EconomyEvent::ManaSphereClaimed {
                    sphere_id: event_sphere,
                    owner_entity_index: event_owner,
                } if *event_sphere == sphere_id && *event_owner == owner_entity_index
            )
        })
        .expect("Possession mailbox is consumed before later Castle upkeep");
    let collect_index = events
        .iter()
        .position(|event| {
            matches!(
                event,
                Level2EconomyEvent::ManaSphereCollected {
                    sphere_id: event_sphere,
                    carrier_id,
                    mana: 500,
                } if *event_sphere == sphere_id && *carrier_id == u32::from(castle_slot)
            )
        })
        .expect("later Castle root collects the newly claimed sphere in the same pass");
    assert!(claim_index < collect_index);
    assert_eq!(
        session.economy().castle().map(|castle| castle.mana),
        Some(500)
    );
    let hidden = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == sphere_slot)
        .expect("callback-hidden sphere remains allocated through the pass");
    assert_eq!(
        (hidden.owner_entity_index, hidden.player_entity_index),
        (owner_entity_index, owner_entity_index)
    );
    assert!(hidden.pending_delete);
    assert_ne!(hidden.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !session
            .economy()
            .spheres()
            .iter()
            .any(|sphere| sphere.source_entity_index == sphere_slot)
    );
    assert!(!session.economy().map_chains().is_linked(sphere_slot));
    assert!(session.economy().free_slots.contains(&sphere_slot));

    session.economy_mut().free_slots = vec![sphere_slot];
    let replacement_id = session
        .economy_mut()
        .spawn_mana_sphere_from_death(202, position, 0, 0x4321, 300, 0)
        .expect("reclaimed numeric slot admits a later sphere generation");
    assert_eq!(replacement_id, sphere_id);
    assert!(session.economy().slot_generation(sphere_slot) > sphere_generation);
    assert_eq!(
        session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == sphere_slot)
            .map(|sphere| sphere.mana),
        Some(300)
    );
    assert!(session.economy().events().iter().any(|event| {
        matches!(
            event,
            Level2EconomyEvent::ManaSphereCollected {
                sphere_id: event_sphere,
                mana: 500,
                ..
            } if *event_sphere == sphere_id
        )
    }));
    assert!(session.economy().events().iter().any(|event| {
        matches!(
            event,
            Level2EconomyEvent::ManaSphereSpawned {
                sphere_id: event_sphere,
                source_entity_index: 202,
                mana: 300,
            } if *event_sphere == sphere_id
        )
    }));
}

#[test]
fn level_two_economy_projection_tracks_source_order_failure_rng_and_reset() {
    let package = level_two_fixture_path();
    let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level2-day-owner-cache/components/gameplay");
    let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
        .expect("qualified Level 2 gameplay component");
    assert!(
        inspection.level2_castle_stage_footprints.contains_key(&1),
        "Castle lifecycle projection requires the validated stage-1 BUILD footprint"
    );
    let mut first = inspection
        .level2_reset_session()
        .expect("Level 2 reset session");
    let second = inspection
        .level2_reset_session()
        .expect("independent Level 2 reset session");
    let baseline_hash = first.economy().reset_hash();
    let baseline_free_slots = first.economy().free_slot_count();
    assert_eq!(baseline_free_slots, second.economy().free_slot_count());
    assert_eq!(baseline_hash, second.economy().reset_hash());
    let raw_map_entries = raw_source_map_chain_entries(
        &inspection.terrain_planes().entity_indices,
        |source_entity_index| {
            inspection
                .entities()
                .get(usize::from(source_entity_index))
                .map(EntitySnapshot::old_map_entity)
        },
    )
    .expect("validated Level 2 raw map chains");
    let holder_rows = first.shared_spells().dynamic_pickups().to_vec();
    let holder_slots = holder_rows
        .iter()
        .map(|holder| holder.index)
        .collect::<Vec<_>>();
    let mut expected_linked = raw_map_entries
        .iter()
        .map(|entry| entry.entity_index)
        .collect::<Vec<_>>();
    expected_linked.extend(holder_slots.iter().copied());
    expected_linked.sort_unstable();
    assert_eq!(
        first.economy().map_chains().linked_entity_indices(),
        expected_linked
    );
    let mut expected_by_tile = BTreeMap::<u16, Vec<u16>>::new();
    for entry in &raw_map_entries {
        expected_by_tile
            .entry(entry.tile)
            .or_default()
            .push(entry.entity_index);
    }
    for holder in &holder_rows {
        let tile = RenderableMapSeed {
            entity_index: holder.index,
            active: holder.active,
            x: holder.position.x,
            y: holder.position.y,
        }
        .tile_index();
        expected_by_tile
            .entry(tile)
            .or_default()
            .insert(0, holder.index);
    }
    for (tile, expected_chain) in expected_by_tile {
        assert_eq!(
            first
                .economy()
                .map_chains()
                .entity_chain((tile & 0xff) as u8, (tile >> 8) as u8),
            expected_chain
        );
    }

    let sphere_slot = *first.economy().free_slots.last().expect("free sphere slot");
    let seed = sphere_slot.wrapping_add(inspection.runtime().random_state as u16);
    let sphere_rng = level2_next_rng16(seed);
    let expected_yaw =
        i16::try_from((i32::from(sphere_rng % 0x71) + i32::from(0x120_i16) - 56) & 0x07ff)
            .expect("sphere yaw");
    let sphere_id = first
        .economy_mut()
        .spawn_mana_sphere_from_death(
            42,
            Position {
                x: 384,
                y: 62_080,
                z: 1_024,
            },
            0x120,
            0x4321,
            321,
            121,
        )
        .expect("sphere allocation");
    assert_eq!(sphere_id, u32::from(sphere_slot));
    let sphere = &first.economy().spheres()[0];
    assert_eq!(sphere.source_entity_index, sphere_slot);
    assert_eq!(sphere.entity_rng, level2_next_rng16(sphere_rng));
    assert_eq!(sphere.yaw, expected_yaw);
    assert_eq!(sphere.mana, 321);
    assert_eq!(sphere.owner_entity_index, 121);
    assert_eq!(first.economy().source_owner(42), 0);
    assert_eq!(
        first.economy().events()[0],
        Level2EconomyEvent::ManaSphereSpawned {
            sphere_id,
            source_entity_index: 42,
            mana: 321,
        }
    );
    assert_eq!(first.economy().free_slot_count(), baseline_free_slots - 1);

    assert!(first.economy_mut().claim_mana_sphere(sphere_slot, 111));
    assert_eq!(
        first.economy().events()[1],
        Level2EconomyEvent::ManaSphereClaimed {
            sphere_id,
            owner_entity_index: 111,
        }
    );
    assert!(!first.economy_mut().claim_mana_sphere(sphere_slot, 111));

    let carrier_slot = first
        .economy_mut()
        .spawn_mana_carrier(
            Position {
                x: 384,
                y: 62_080,
                z: 1_024,
            },
            111,
            sphere_id,
        )
        .expect("carrier allocation");
    let carrier = first.economy().carrier().expect("active carrier");
    assert_eq!(carrier.source_entity_index, carrier_slot);
    assert_eq!(carrier.speed, MANA_CARRIER_SPEED);
    assert_eq!(carrier.life, MANA_CARRIER_MAX_LIFE);
    assert_eq!(carrier.max_mana, MANA_CARRIER_MAX_MANA);
    assert_eq!(
        first.economy().events()[2],
        Level2EconomyEvent::ManaCarrierSpawned {
            source_entity_index: carrier_slot,
            owner_entity_index: 111,
        }
    );
    assert!(
        first
            .economy_mut()
            .absorb_sphere_into_carrier(sphere_slot, carrier_slot)
    );
    let collected_sphere = first
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == sphere_slot)
        .expect("collected sphere remains allocated through its callback");
    assert!(collected_sphere.active);
    assert!(collected_sphere.pending_delete);
    assert_ne!(collected_sphere.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
    assert_eq!(collected_sphere.assigned_carrier_id, 0);
    assert_eq!(
        first.economy().carrier().expect("carrier").carried_mana,
        321
    );
    assert_eq!(
        first.economy().events()[3],
        Level2EconomyEvent::ManaSphereCollected {
            sphere_id,
            carrier_id: u32::from(carrier_slot),
            mana: 321,
        }
    );
    first
        .economy_mut()
        .reclaim_inactive_slots_at_source_boundary();
    assert!(
        !first
            .economy()
            .spheres()
            .iter()
            .any(|sphere| sphere.source_entity_index == sphere_slot)
    );
    assert!(!first.economy().map_chains().is_linked(sphere_slot));
    assert!(first.economy().free_slots.contains(&sphere_slot));

    let castle_slot = first
        .economy_mut()
        .build_castle(
            Position {
                x: 1_024,
                y: 1_024,
                z: 0,
            },
            111,
        )
        .expect("castle allocation");
    assert_eq!(
        first.economy().castle().expect("castle").life,
        CASTLE_STAGE_ONE_LIFE
    );
    assert_eq!(
        first.economy().events()[4],
        Level2EconomyEvent::CastleBuilt {
            source_entity_index: castle_slot,
            owner_entity_index: 111,
        }
    );
    assert!(first.economy_mut().return_carrier_to_castle(carrier_slot));
    assert_eq!(first.economy().castle().expect("castle").mana, 0);
    assert!(first.economy().carrier().expect("returning carrier").active);
    let castle_position = first.economy().castle().expect("castle").position;
    let below_ground_far = Position {
        x: castle_position.x.wrapping_add(4_096),
        y: castle_position.y,
        z: i16::MIN,
    };
    let carrier_z_before_failed_move = first.economy().carrier().expect("carrier").position.z;
    assert!(
        !first
            .economy_mut()
            .step_carrier_to_castle(carrier_slot, below_ground_far)
    );
    assert_eq!(
        first
            .economy()
            .carrier()
            .expect("unchanged carrier")
            .position
            .z,
        carrier_z_before_failed_move
    );
    assert!(
        first
            .economy_mut()
            .step_carrier_to_castle(carrier_slot, castle_position)
    );
    assert_eq!(first.economy().castle().expect("castle").mana, 321);
    assert_eq!(
        first
            .economy()
            .carrier()
            .expect("arrived carrier")
            .carried_mana,
        0
    );
    assert_eq!(
        first.economy().events()[5],
        Level2EconomyEvent::ManaCarrierArrived {
            carrier_id: u32::from(carrier_slot),
            castle_id: castle_slot,
            mana: 321,
        }
    );

    assert!(first.economy_mut().submit_damage(castle_slot, 100, 77));
    assert!(first.economy_mut().submit_damage(castle_slot, 200, 77));
    assert_eq!(
        first.economy().castle().expect("castle").pending_damage,
        300
    );
    assert_eq!(first.economy_mut().resolve_damage(castle_slot), Some(false));
    assert_eq!(
        first.economy().castle().expect("castle").life,
        CASTLE_STAGE_ONE_LIFE - 300
    );
    assert_eq!(
        first.economy().events()[6],
        Level2EconomyEvent::DamageApplied {
            target_entity_index: castle_slot,
            attacker_entity_index: 77,
            damage: 300,
            lethal: false,
        }
    );
    let remaining_life = first.economy().castle().expect("castle").life;
    assert!(
        first
            .economy_mut()
            .submit_damage(castle_slot, remaining_life + 1, 88)
    );
    assert_eq!(first.economy_mut().resolve_damage(castle_slot), Some(true));
    let dead_castle = first.economy().castle().expect("castle");
    assert!(dead_castle.active);
    assert_eq!(dead_castle.killer_entity_index, 88);
    assert_eq!(
        first.economy().events()[7],
        Level2EconomyEvent::DamageApplied {
            target_entity_index: castle_slot,
            attacker_entity_index: 88,
            damage: remaining_life + 1,
            lethal: true,
        }
    );
    assert!(first.economy_mut().step_castle_action(castle_slot));
    assert_eq!(first.economy().castle().expect("castle").action, 6);
    assert!(first.economy_mut().step_castle_action(castle_slot));
    let reset_castle = first.economy().castle().expect("reset Castle");
    assert!(!reset_castle.active);
    assert_eq!((reset_castle.action, reset_castle.level), (4, 0));

    first.economy_mut().reclaim_inactive_slots();
    // The caller-side `sub_5FD00` pass releases the final Castle mana
    // after `sub_605E0`; that sphere remains active while the carrier is
    // still live, so two arena slots remain occupied.
    assert_eq!(first.economy().free_slot_count(), baseline_free_slots - 2);
    assert_eq!(first.economy().spheres().len(), 1);
    assert!(first.economy().spheres()[0].active);
    assert!(first.economy().carrier().is_some());
    assert!(first.economy().castle().is_none());

    let failed_hash_before = first.economy().reset_hash();
    first.economy_mut().free_slots.clear();
    assert!(
        first
            .economy_mut()
            .spawn_mana_sphere_from_death(43, Position::default(), 0, 0x9876, 12, 0)
            .is_none()
    );
    assert_eq!(first.economy().spheres().len(), 1);
    assert!(first.economy().spheres()[0].active);
    assert_eq!(first.economy().free_slot_count(), 0);
    assert_ne!(first.economy().reset_hash(), failed_hash_before);
    assert_eq!(first.economy().source_rng.get(&43), Some(&0x9876));

    assert_eq!(second.economy().reset_hash(), baseline_hash);
    first.reset();
    assert_eq!(first.economy().reset_hash(), baseline_hash);
    assert_eq!(first.economy().free_slot_count(), baseline_free_slots);
    assert!(first.economy().spheres().is_empty());
    assert!(first.economy().carrier().is_none());
    assert!(first.economy().castle().is_none());

    let fractional_ids = first.economy_mut().spawn_mana_spheres_from_death(
        44,
        Position::default(),
        0,
        0x2468,
        3_500,
        111,
        true,
    );
    assert_eq!(fractional_ids.len(), 1);
    let fractional_mana = fractional_ids
        .iter()
        .map(|id| {
            first
                .economy()
                .spheres()
                .iter()
                .find(|sphere| sphere.id == *id)
                .expect("fractional sphere")
                .mana
        })
        .collect::<Vec<_>>();
    assert_eq!(fractional_mana, vec![3_500]);
    assert_eq!(fractional_mana.iter().sum::<i32>(), 3_500);
    assert_eq!(first.economy().events().len(), 1);
    first.reset();
    assert_eq!(first.economy().reset_hash(), baseline_hash);

    let mut merge_session = inspection
        .level2_reset_session()
        .expect("merge projection session");
    let neutral_id = merge_session
        .economy_mut()
        .spawn_mana_sphere_from_death(45, Position::default(), 0, 0x1111, 100, 0)
        .expect("neutral sphere");
    let owned_id = merge_session
        .economy_mut()
        .spawn_mana_sphere_from_death(46, Position::default(), 0, 0x2222, 50, 111)
        .expect("owned sphere");
    merge_session.economy_mut().begin_turn();
    assert!(merge_session.economy_mut().merge_mana_spheres(
        u16::try_from(neutral_id).expect("sphere slot"),
        u16::try_from(owned_id).expect("sphere slot")
    ));
    let merged = merge_session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.id == neutral_id)
        .expect("merged target");
    assert_eq!(merged.owner_entity_index, 111);
    assert_eq!(merged.mana, 150);
    assert!(
        merge_session
            .economy()
            .spheres()
            .iter()
            .all(|sphere| sphere.id != owned_id)
    );
    assert_eq!(
        merge_session.economy().events(),
        &[Level2EconomyEvent::ManaSpheresMerged {
            target_sphere_id: neutral_id,
            source_sphere_id: owned_id,
            owner_entity_index: 111,
            mana: 50,
        }]
    );

    let mut carrier_gates = inspection
        .level2_reset_session()
        .expect("carrier gate session");
    let carrier_position = Position::default();
    let carrier_slot = carrier_gates
        .economy_mut()
        .spawn_mana_carrier(carrier_position, 111, 0)
        .expect("carrier allocation");
    let wrong_owner = carrier_gates
        .economy_mut()
        .spawn_mana_sphere_from_death(47, carrier_position, 0, 0x3333, 1, 112)
        .expect("wrong-owner sphere");
    assert!(!carrier_gates.economy_mut().absorb_sphere_into_carrier(
        u16::try_from(wrong_owner).expect("sphere slot"),
        carrier_slot
    ));
    let far_sphere = carrier_gates
        .economy_mut()
        .spawn_mana_sphere_from_death(
            48,
            Position {
                x: 2_048,
                ..carrier_position
            },
            0,
            0x4444,
            1,
            111,
        )
        .expect("far sphere");
    assert!(!carrier_gates.economy_mut().absorb_sphere_into_carrier(
        u16::try_from(far_sphere).expect("sphere slot"),
        carrier_slot
    ));
    let near_non_overlap = carrier_gates
        .economy_mut()
        .spawn_mana_sphere_from_death(
            49,
            Position {
                x: 400,
                ..carrier_position
            },
            0,
            0x5555,
            1,
            111,
        )
        .expect("near non-overlap sphere");
    assert!(!carrier_gates.economy_mut().absorb_sphere_into_carrier(
        u16::try_from(near_non_overlap).expect("sphere slot"),
        carrier_slot
    ));
    let near = carrier_gates
        .economy_mut()
        .spawn_mana_sphere_from_death(50, carrier_position, 0, 0x6666, 1, 111)
        .expect("near sphere");
    assert!(
        carrier_gates
            .economy_mut()
            .absorb_sphere_into_carrier(u16::try_from(near).expect("sphere slot"), carrier_slot)
    );
    assert_eq!(
        carrier_gates
            .economy()
            .carrier()
            .expect("carrier")
            .target_sphere_id,
        0
    );

    let mut castle_lifecycle = inspection
        .level2_reset_session()
        .expect("Castle lifecycle session");
    let lifecycle_slot = castle_lifecycle
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("lifecycle Castle");
    let construction_anchor = Position {
        x: 3_072,
        y: 4_096,
        z: 512,
    };
    assert!(
        castle_lifecycle
            .economy_mut()
            .set_castle_anchor(lifecycle_slot, construction_anchor)
    );
    assert!(
        castle_lifecycle
            .economy_mut()
            .start_castle_construction(lifecycle_slot)
    );
    castle_lifecycle.economy_mut().free_slots.clear();
    assert!(
        !castle_lifecycle
            .economy_mut()
            .step_castle_action(lifecycle_slot)
    );
    let blocked = castle_lifecycle.economy().castle().expect("blocked Castle");
    assert_eq!(
        (blocked.action, blocked.build_state, blocked.level),
        (5, 0, 0)
    );
    castle_lifecycle.reset();
    assert_eq!(
        castle_lifecycle
            .economy()
            .map_chains()
            .linked_entity_indices(),
        expected_linked
    );
    let lifecycle_slot = castle_lifecycle
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("lifecycle Castle retry");
    assert!(
        castle_lifecycle
            .economy_mut()
            .set_castle_anchor(lifecycle_slot, construction_anchor)
    );
    assert!(
        castle_lifecycle
            .economy_mut()
            .start_castle_construction(lifecycle_slot)
    );
    assert!(
        castle_lifecycle
            .economy_mut()
            .step_castle_action(lifecycle_slot)
    );
    let construction = castle_lifecycle
        .economy()
        .castle()
        .expect("construction Castle");
    assert_eq!(
        (
            construction.construction_model,
            construction.construction_action,
            construction.construction_control_flags
        ),
        (42, 44, 1)
    );
    assert_eq!(castle_lifecycle.economy().castle_children().len(), 2);
    assert_eq!(
        castle_lifecycle.economy().castle_children()[0],
        Level2CastleChild {
            source_entity_index: construction.construction_slot,
            entity_rng: construction
                .construction_slot
                .wrapping_add(castle_lifecycle.economy().global_rng as u16),
            id_entity_index: 111,
            source_class: 10,
            parent_entity_index: lifecycle_slot,
            owner_entity_index: 111,
            player_entity_index: 0,
            position: construction_anchor,
            axis_position: Position::default(),
            model: 42,
            action: 44,
            mana: 0,
            max_mana: 0,
            life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
            max_life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
            speed: LEVEL2_CASTLE_CHILD_DEFAULT_SPEED,
            target_sphere_slot: 0,
            pending_damage: 0,
            pending_attacker_entity_index: 0,
            damage_attacker_entity_index: 0,
            killer_entity_index: 0,
            source_byte_3e: construction.construction_slot as u8,
            source_byte_39: 0,
            source_byte_3b: 1,
            cave_collision_cooldown: 1,
            source_dword_10: 0,
            source_word_2e: 0,
            construction_target_height: 0,
            source_subspell_index: 0,
            yaw: 0,
            roll: 0,
            visual_index: 0,
            control_flags: 1,
            chain_previous_slot: 0,
            chain_next_slot: 0,
            source_byte_43: 0,
            source_word_4a: 0,
            active: true,
        }
    );
    assert!(
        !castle_lifecycle
            .economy()
            .map_chains()
            .tracks(construction.construction_slot)
    );
    assert!(
        !castle_lifecycle
            .economy()
            .map_chains()
            .is_linked(construction.construction_slot)
    );
    let stage_child = castle_lifecycle
        .economy()
        .castle_children()
        .iter()
        .find(|child| child.active && child.model == 79)
        .expect("source sub_613D0 stage controller");
    // `sub_613D0` owns this row through word32/word34; NewEvent leaves
    // parentId zero for the controller constructor.
    assert_eq!(stage_child.parent_entity_index, 0);
    assert_eq!(stage_child.id_entity_index, 111);
    assert_eq!(
        stage_child.position.z,
        castle_lifecycle
            .canonical_terrain()
            .ground_height(construction.position)
            .saturating_add(384)
    );
    let lifecycle_castle = castle_lifecycle.economy().castle().expect("Castle");
    assert_eq!(lifecycle_castle.controller_chain_count, 1);
    assert_eq!(
        lifecycle_castle.controller_chain_head,
        lifecycle_castle.controller_chain_slots[0]
    );
    assert_eq!(
        lifecycle_castle.controller_chain_tail,
        lifecycle_castle.controller_chain_slots[0]
    );
    assert_eq!(stage_child.chain_previous_slot, lifecycle_slot);
    assert_eq!(stage_child.chain_next_slot, 0);
    assert_eq!(
        (stage_child.source_byte_43, stage_child.source_word_4a),
        (1, 1)
    );
    assert_eq!(lifecycle_castle.source_class, 3);
    assert_eq!(lifecycle_castle.source_model, 2);
    assert_eq!(lifecycle_castle.source_byte_38, 33);
    assert_eq!(lifecycle_castle.source_particle_parameter, 177);
    assert_eq!(
        castle_lifecycle.economy().player_castle_entity_index(),
        lifecycle_slot
    );
    assert_eq!(castle_lifecycle.economy().player_castle_stage(), 1);
    assert!(
        castle_lifecycle
            .economy_mut()
            .complete_castle_construction(lifecycle_slot)
    );
    assert!(
        castle_lifecycle
            .economy_mut()
            .step_castle_action(lifecycle_slot)
    );
    let built = castle_lifecycle.economy().castle().expect("built Castle");
    assert_eq!((built.action, built.build_state, built.level), (4, 0, 1));

    // Source sub_60810/sub_613D0 stage-two vector: the table updates
    // life/capacity and the controller anchor drops from +384 to +224.
    let mut stage_two = inspection
        .level2_reset_session()
        .expect("stage-two Castle session");
    let stage_two_slot = stage_two
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("stage-two Castle");
    assert!(
        stage_two
            .economy_mut()
            .start_castle_construction(stage_two_slot)
    );
    assert!(stage_two.economy_mut().step_castle_action(stage_two_slot));
    {
        let mut economy = stage_two.economy_mut();
        assert!(economy.set_castle_controller_coefficient(2, 0x2a));
        let castle = economy.castle.as_mut().expect("stage-two Castle");
        castle.action = 5;
        castle.build_state = 3;
        castle.level = 1;
        for child in &mut economy.castle_children {
            if child.parent_entity_index == stage_two_slot && child.model == 42 {
                child.active = false;
            }
        }
    }
    assert!(stage_two.economy_mut().step_castle_action(stage_two_slot));
    let stage_two_castle = stage_two.economy().castle().expect("stage-two Castle");
    assert_eq!(
        (
            stage_two_castle.level,
            stage_two_castle.life,
            stage_two_castle.max_life,
            stage_two_castle.mana_capacity
        ),
        (2, 40_000, 40_000, 18_000)
    );
    let stage_two_child = stage_two
        .economy()
        .castle_children()
        .iter()
        .find(|child| child.active && child.model == 79)
        .expect("stage-two controller");
    assert_eq!(stage_two_child.position.z, 224);
    assert_eq!(stage_two_child.source_byte_43, 0x2a);
    assert_eq!(stage_two_child.source_word_4a, 2);
    let stage_two_castle = stage_two.economy().castle().expect("stage-two Castle");
    assert_eq!(stage_two_castle.controller_chain_count, 4);
    assert_eq!(
        stage_two_castle.controller_chain_head,
        stage_two_castle.controller_chain_slots[0]
    );
    assert_eq!(
        stage_two_castle.controller_chain_tail,
        stage_two_castle.controller_chain_slots[3]
    );
    let stage_two_chain = stage_two
        .economy()
        .castle_children()
        .iter()
        .filter(|child| child.active && child.model == 79)
        .collect::<Vec<_>>();
    assert_eq!(stage_two_chain.len(), 4);
    for (index, child) in stage_two_chain.iter().enumerate() {
        let expected_previous = if index == 0 {
            stage_two_slot
        } else {
            stage_two_chain[index - 1].source_entity_index
        };
        let expected_next = stage_two_chain
            .get(index + 1)
            .map_or(0, |next| next.source_entity_index);
        assert_eq!(child.chain_previous_slot, expected_previous);
        assert_eq!(child.chain_next_slot, expected_next);
        assert_eq!((child.source_byte_43, child.source_word_4a), (0x2a, 2));
    }

    let mut defense_lifecycle = inspection
        .level2_reset_session()
        .expect("Castle defense lifecycle session");
    let defense_root = defense_lifecycle
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("defense Castle");
    assert!(
        defense_lifecycle
            .economy_mut()
            .set_castle_stage_coefficient(1, 128)
    );
    assert!(
        defense_lifecycle
            .economy_mut()
            .start_castle_construction(defense_root)
    );
    assert!(
        defense_lifecycle
            .economy_mut()
            .step_castle_action(defense_root)
    );
    assert!(
        defense_lifecycle
            .economy_mut()
            .start_castle_defense_child(defense_root)
    );
    assert!(
        defense_lifecycle
            .economy_mut()
            .step_castle_action(defense_root)
    );
    let defense = defense_lifecycle.economy().castle().expect("defense child");
    assert_eq!(
        (
            defense.defense_model,
            defense.defense_action,
            defense.defense_control_flags
        ),
        (41, 43, 0)
    );
    assert_eq!((defense.life, defense.max_life), (10_000, 10_000));
    assert_eq!(defense_lifecycle.economy().castle_children().len(), 3);
    assert_eq!(
        defense_lifecycle
            .economy()
            .castle_children()
            .iter()
            .filter(|child| child.active && child.model == 41)
            .count(),
        1
    );
    assert_eq!(
        defense_lifecycle
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.active && child.model == 41)
            .expect("defense child")
            .position,
        defense_lifecycle
            .economy()
            .castle()
            .expect("defense Castle")
            .anchor_position
    );

    let mut action4_session = inspection
        .level2_reset_session()
        .expect("Castle action-4 session");
    let action4_slot = action4_session
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("action-4 Castle");
    let action4_position = action4_session.economy().castle().expect("Castle").position;
    let action4_sphere = action4_session
        .economy_mut()
        .spawn_mana_sphere_from_death(51, action4_position, 0, 0x7777, 100, 111)
        .expect("Castle absorption sphere");
    action4_session.economy_mut().begin_turn();
    assert!(
        action4_session
            .economy_mut()
            .step_castle_action(action4_slot)
    );
    assert_eq!(action4_session.economy().castle().expect("Castle").mana, 0);
    assert_eq!(
        action4_session.economy().castle().expect("Castle").phase,
        (action4_slot as u8).wrapping_add(1)
    );
    assert!(
        action4_session
            .economy_mut()
            .step_castle_action(action4_slot)
    );
    assert_eq!(
        action4_session.economy().castle().expect("Castle").mana,
        100
    );
    let absorbed_sphere = action4_session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.id == action4_sphere)
        .expect("absorbed sphere remains allocated through its callback");
    assert!(absorbed_sphere.active);
    assert!(absorbed_sphere.pending_delete);
    assert_ne!(absorbed_sphere.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
    // Source EndOfCastleProjectile calls sub_609E0 before the stage
    // maintenance chain. Non-lethal damage sets byte-195, while the
    // separate word80 upgrade mailbox requests state-0 construction.
    let mut damage_session = inspection
        .level2_reset_session()
        .expect("Castle damage session");
    let damage_slot = damage_session
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("damage Castle");
    {
        let mut economy = damage_session.economy_mut();
        let castle = economy.castle_at_mut(damage_slot).unwrap();
        castle.source_word_80 = 111;
        castle.source_word_7c = 10;
        castle.phase = 0;
    }
    let sphere = damage_session.economy_mut()
        .spawn_mana_sphere_from_death(51, Position::default(), 0, 0x7777, 100, 111)
        .expect("same-pass Castle sphere");
    assert!(
        damage_session
            .economy_mut()
            .submit_damage(damage_slot, 1, 111)
    );
    assert!(damage_session.economy_mut().step_castle_action(damage_slot));
    let damage_transition = damage_session.economy().castle().expect("Castle");
    assert_eq!(
        (damage_transition.action, damage_transition.build_state),
        (5, 0)
    );
    assert_eq!(damage_session.economy().player_castle_aux_flags(), 4);
    assert_ne!(damage_transition.control_flags & 0x40, 0);
    assert_eq!((damage_transition.source_word_80, damage_transition.source_word_7c), (0, 10));
    assert_eq!(damage_transition.mana, 100, "upgrade still runs the source even-phase collection tail");
    assert!(damage_session.economy().spheres().iter().any(|row| row.id == sphere && row.pending_delete));

    // Source sub_5FD00 releases only overflow above stage capacity. The
    // anchor is deliberately separated from the root to prove the later
    // source-order model-39 scan does not immediately reclaim that sphere.
    let mut release_session = inspection
        .level2_reset_session()
        .expect("Castle release session");
    let release_slot = release_session
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("release Castle");
    let release_anchor = Position {
        x: 8_192,
        y: 8_192,
        z: 0,
    };
    assert!(
        release_session
            .economy_mut()
            .set_castle_anchor(release_slot, release_anchor)
    );
    {
        let mut economy = release_session.economy_mut();
        economy.player_mana = 0;
        let castle = economy.castle.as_mut().expect("release Castle");
        castle.mana = castle.mana_capacity + 1_000;
        // Force the source's even-phase maintenance branch for this
        // vector while retaining the slot-seeded default in production.
        castle.phase &= !1;
    }
    assert!(
        release_session
            .economy_mut()
            .step_castle_action(release_slot)
    );
    let release_castle = release_session.economy().castle().expect("release Castle");
    assert_eq!(release_castle.mana, release_castle.mana_capacity);
    assert!(
        release_session
            .economy()
            .spheres()
            .iter()
            .any(|sphere| sphere.active
                && sphere.owner_entity_index == 111
                && sphere.mana == 1_000)
    );

    // When the requested source fraction is larger than the remaining
    // arena, sub_5FD00 recomputes the share from the committed count; it
    // does not silently lose the unreleased remainder.
    let mut partial_release = inspection
        .level2_reset_session()
        .expect("partial Castle release session");
    let partial_slot = partial_release
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("partial release Castle");
    {
        let mut economy = partial_release.economy_mut();
        economy.player_mana = 0;
        economy.free_slots.truncate(2);
        let castle = economy.castle.as_mut().expect("partial release Castle");
        castle.mana = castle.mana_capacity + 3_500;
        castle.phase &= !1;
    }
    assert!(
        partial_release
            .economy_mut()
            .step_castle_action(partial_slot)
    );
    let partial = partial_release
        .economy()
        .spheres()
        .iter()
        .filter(|sphere| sphere.active && sphere.owner_entity_index == 111)
        .map(|sphere| sphere.mana)
        .collect::<Vec<_>>();
    assert_eq!(partial, vec![1_750, 1_750]);
    assert_eq!(partial.iter().sum::<i32>(), 3_500);
    action4_session
        .economy_mut()
        .castle
        .as_mut()
        .expect("Castle")
        .timer = 2;
    assert!(
        action4_session
            .economy_mut()
            .step_castle_action(action4_slot)
    );
    assert_eq!(action4_session.economy().castle().expect("Castle").timer, 1);
    assert!(
        action4_session
            .economy_mut()
            .step_castle_action(action4_slot)
    );
    let timer_transition = action4_session.economy().castle().expect("Castle");
    assert_eq!(
        (
            timer_transition.action,
            timer_transition.build_state,
            timer_transition.timer
        ),
        (5, 3, 0)
    );

    // Source sub_605E0/sub_5FCA0 lowers maxMana by ten percent before
    // releasing overflow, then restores the next stage's exact table
    // values. The stage transition itself is not blocked when the arena
    // has no free slots for the optional overflow spheres.
    let mut teardown = inspection
        .level2_reset_session()
        .expect("Castle teardown session");
    let teardown_slot = teardown
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("teardown Castle");
    assert!(
        teardown
            .economy_mut()
            .start_castle_construction(teardown_slot)
    );
    assert!(teardown.economy_mut().step_castle_action(teardown_slot));
    {
        let mut economy = teardown.economy_mut();
        economy.set_player_spell_cleanup_context(1, 4, 777);
        economy.set_player_spell_holder_counters(17, 9);
        let castle = economy.castle.as_mut().expect("teardown Castle");
        castle.action = 6;
        castle.level = 2;
        castle.life = -10_000;
        castle.max_life = 40_000;
        castle.mana = 20_000;
        castle.mana_capacity = 18_000;
    }
    let terminal_cursor_rows = teardown
        .economy()
        .live_arena_cursor_rows()
        .into_iter()
        .filter(|row| {
            matches!(
                row.kind,
                Level2DynamicArenaKind::CastleChild | Level2DynamicArenaKind::CastleController
            )
        })
        .map(|row| (row.source_slot, row.generation, row.kind))
        .collect::<Vec<_>>();
    let mut expected_terminal_cursor_rows = teardown
        .economy()
        .castle_children()
        .iter()
        .filter(|child| child.active)
        .filter_map(|child| {
            let kind = match child.model {
                42 => Level2DynamicArenaKind::CastleChild,
                79 => Level2DynamicArenaKind::CastleController,
                _ => return None,
            };
            Some((
                child.source_entity_index,
                teardown
                    .economy()
                    .slot_generation(child.source_entity_index),
                kind,
            ))
        })
        .collect::<Vec<_>>();
    expected_terminal_cursor_rows.sort_unstable_by_key(|row| row.0);
    assert_eq!(terminal_cursor_rows, expected_terminal_cursor_rows);
    assert!(teardown.economy_mut().step_castle_action(teardown_slot));
    let downgraded = teardown.economy().castle().expect("downgraded Castle");
    assert!(downgraded.active);
    assert_eq!(
        (
            downgraded.action,
            downgraded.build_state,
            downgraded.level,
            downgraded.life,
            downgraded.max_life,
            downgraded.mana,
            downgraded.mana_capacity,
            downgraded.timer
        ),
        // `sub_5FD00` uses the floor share for each allocated sphere;
        // its two-unit quotient remainder stays in the Castle payload.
        (4, 0, 1, 10_000, 20_000, 8_502, 8_500, 5)
    );
    assert_eq!(
        (
            downgraded.shift_pitch,
            downgraded.shift_roll,
            downgraded.shift_yaw,
            downgraded.shift_fov
        ),
        (1_664, 1_664, -8_192, 0x4000)
    );
    assert_eq!(
        teardown.economy().player_castle_entity_index(),
        teardown_slot
    );
    assert_eq!(teardown.economy().player_castle_stage(), 1);
    assert_eq!(teardown.economy().player_spell_slot_2(), 777);
    assert_eq!(teardown.economy().player_spell_holder_action(), 8);
    assert_eq!(
        (downgraded.source_word_2e, downgraded.source_word_30),
        (0, 5)
    );
    assert!(
        teardown
            .economy()
            .events()
            .contains(&Level2EconomyEvent::CastleDestructionSound {
                source_entity_index: teardown_slot,
                sound_id: 30,
            })
    );
    assert!(
        teardown
            .economy()
            .events()
            .contains(&Level2EconomyEvent::CastleStageDestroyed {
                source_entity_index: teardown_slot,
                previous_level: 2,
                next_level: 1,
            })
    );
    let callback_child_rows = teardown
        .economy()
        .castle_children()
        .iter()
        .filter(|child| child.active)
        .map(|child| {
            (
                child.source_entity_index,
                child.source_class,
                child.model,
                child.action,
                child.parent_entity_index,
                child.chain_previous_slot,
                child.chain_next_slot,
            )
        })
        .collect::<Vec<_>>();
    let model3_slot = callback_child_rows
        .iter()
        .find(|row| (row.1, row.2, row.3) == (3, 3, 9))
        .map(|row| row.0)
        .expect("source model-3 callback row");
    let model42_slot = callback_child_rows
        .iter()
        .find(|row| (row.1, row.2, row.3) == (10, 42, 44))
        .map(|row| row.0)
        .expect("source model-42 callback row");
    assert_eq!(
        callback_child_rows,
        vec![
            (model42_slot, 10, 42, 44, teardown_slot, 0, 0),
            (
                downgraded.controller_chain_slots[0],
                10,
                79,
                0x56,
                0,
                teardown_slot,
                0
            ),
            (model3_slot, 3, 3, 9, 0, 0, 0),
        ]
    );

    teardown.select_level2_campaign_entry_stage();
    teardown.step_outer_turn(PlayerCommand::default(), 0);
    assert!(teardown.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::Castle { source_slot, .. }
                if *source_slot == teardown_slot
        )
    }));
    let terminal_dispatch_rows = teardown
        .dispatch_events()
        .iter()
        .filter_map(|event| match event {
            Mc2DispatchEvent::DynamicArena {
                source_slot, kind, ..
            } if matches!(
                kind,
                Level2DynamicArenaKind::CastleChild | Level2DynamicArenaKind::CastleController
            ) =>
            {
                Some((*source_slot, *kind))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        terminal_dispatch_rows,
        vec![
            (model42_slot, Level2DynamicArenaKind::CastleChild),
            (
                downgraded.controller_chain_slots[0],
                Level2DynamicArenaKind::CastleController
            ),
        ]
    );
    let live_cursor_rows = teardown.economy().live_arena_cursor_rows();
    let terminal_child = terminal_cursor_rows
        .iter()
        .find(|(_, _, kind)| *kind == Level2DynamicArenaKind::CastleChild)
        .copied()
        .expect("terminal construction row");
    let terminal_controller = terminal_cursor_rows
        .iter()
        .find(|(_, _, kind)| *kind == Level2DynamicArenaKind::CastleController)
        .copied()
        .expect("terminal controller row");
    let terminal_child_live = live_cursor_rows
        .iter()
        .find(|row| row.source_slot == terminal_child.0)
        .copied()
        .expect("stage teardown reuses the construction slot");
    assert_eq!(terminal_child_live.kind, terminal_child.2);
    assert_eq!(terminal_child_live.generation, terminal_child.1);
    assert!(!teardown.economy().map_chains().is_linked(terminal_child.0));
    assert!(!teardown.economy().free_slots.contains(&terminal_child.0));
    let terminal_controller_live = live_cursor_rows
        .iter()
        .find(|row| row.source_slot == terminal_controller.0)
        .copied()
        .expect("stage teardown reuses the controller slot");
    // The source stage transition may immediately repopulate this slot
    // with the next live model-79 controller; either that controller or a
    // source-order replacement is valid, but the row must be new, linked,
    // and outside the free stack.
    assert_eq!(
        terminal_controller_live.kind,
        Level2DynamicArenaKind::CastleController
    );
    assert_ne!(terminal_controller_live.generation, terminal_controller.1);
    assert!(
        teardown
            .economy()
            .map_chains()
            .is_linked(terminal_controller.0)
    );
    let terminal_post_turn_rows = terminal_cursor_rows
        .iter()
        .map(|(source_slot, generation, _)| {
            (
                *source_slot,
                *generation,
                live_cursor_rows
                    .iter()
                    .find(|row| row.source_slot == *source_slot)
                    .map(|row| (row.source_slot, row.generation, row.kind)),
                teardown.economy().map_chains().is_linked(*source_slot),
                teardown.economy().free_slots.contains(source_slot),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        terminal_post_turn_rows,
        vec![
            (
                terminal_child.0,
                terminal_child.1,
                Some((
                    terminal_child_live.source_slot,
                    terminal_child_live.generation,
                    terminal_child_live.kind,
                )),
                false,
                false,
            ),
            (
                terminal_controller.0,
                terminal_controller.1,
                Some((
                    terminal_controller_live.source_slot,
                    terminal_controller_live.generation,
                    terminal_controller_live.kind,
                )),
                true,
                false,
            ),
        ]
    );
    assert_eq!(
        teardown
            .economy()
            .castle_children()
            .iter()
            .filter(|child| child.active)
            .map(|child| {
                (
                    child.source_entity_index,
                    child.source_class,
                    child.model,
                    child.action,
                    child.parent_entity_index,
                    child.chain_previous_slot,
                    child.chain_next_slot,
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (model42_slot, 10, 42, 44, teardown_slot, 0, 0),
            (
                downgraded.controller_chain_slots[0],
                10,
                79,
                0x56,
                0,
                teardown_slot,
                0,
            ),
            (model3_slot, 3, 3, 9, 0, 0, 0),
        ]
    );

    // A final-stage teardown also completes with an exhausted arena and
    // clears the player Castle link plus the model-1 spell holder.
    let mut final_teardown = inspection
        .level2_reset_session()
        .expect("final Castle teardown session");
    let final_baseline_hash = final_teardown.economy().reset_hash();
    let final_baseline_holder = final_teardown.economy().player_spell_slot_2();
    let final_slot = final_teardown
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("final Castle");
    {
        let mut economy = final_teardown.economy_mut();
        economy.set_player_spell_cleanup_context(1, 4, 888);
        let castle = economy.castle.as_mut().expect("final Castle");
        castle.action = 6;
        castle.level = 1;
        castle.life = -1;
        castle.max_life = 20_000;
    }
    assert!(final_teardown.economy_mut().step_castle_action(final_slot));
    let destroyed = final_teardown.economy().castle().expect("destroyed Castle");
    assert!(!destroyed.active);
    assert_eq!((destroyed.source_word_2e, destroyed.source_word_30), (0, 5));
    assert_eq!(final_teardown.economy().player_castle_entity_index(), 0);
    assert_eq!(final_teardown.economy().player_castle_stage(), 0);
    assert_eq!(final_teardown.economy().player_spell_slot_2(), 0);
    assert!(
        final_teardown
            .economy()
            .events()
            .contains(&Level2EconomyEvent::CastleDestroyed {
                source_entity_index: final_slot,
                killer_entity_index: 0,
            })
    );
    final_teardown.economy_mut().reset();
    assert_eq!(final_teardown.economy().reset_hash(), final_baseline_hash);
    assert_eq!(
        final_teardown.economy().player_spell_slot_2(),
        final_baseline_holder
    );
}

#[test]
fn level_two_bee_ordinary_hits_accumulate_before_own_callback() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2");
    let mut campaign = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::LEVEL2,
    )
    .expect("qualified semantic Level 2 campaign");
    let session = campaign.simulation_mut();
    session.select_level2_campaign_entry_stage();
    let producer = session
        .economy
        .entities()
        .iter()
        .find(|entity| {
            entity.active
                && entity.class == 11
                && entity.model == 32
                && entity.source_id == 21
                && !entity.drawing_disabled()
        })
        .expect("real model32 group21 producer");
    let stage = usize::try_from(producer.level_gate).unwrap();
    for objective in 0..session.runtime.objectives.len() {
        session
            .runtime
            .set_level2_objective_stage_selector_for(objective, stage, 2);
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    let bee = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.authored_index() == Some(229))
        .copied()
        .expect("real authored Bee constructor");
    let attacker = session.state.entity_index;
    let second_attacker = session
        .economy
        .entities()
        .iter()
        .find(|entity| entity.active && entity.class == 3 && entity.model == 1)
        .expect("real hostile player carrier")
        .source_id;
    {
        let mut economy = session.economy_mut();
        assert!(economy.submit_authored_bee_damage(bee.source_entity_index, 100, attacker));
        assert!(economy.submit_authored_bee_damage(bee.source_entity_index, 200, second_attacker));
        let pending = economy
            .bee_creatures()
            .iter()
            .find(|candidate| candidate.source_entity_index == bee.source_entity_index)
            .unwrap();
        assert_eq!(
            (
                pending.pending_damage,
                pending.pending_attacker_entity_index
            ),
            (300, second_attacker)
        );
    }
    // The arena's real source visits consume both hits exactly once.
    // A constructor-selected linked action may defer consumption one pass.
    for _ in 0..3 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        let callback = session
            .economy()
            .bee_creatures()
            .iter()
            .find(|candidate| candidate.source_entity_index == bee.source_entity_index)
            .unwrap();
        if callback.pending_attacker_entity_index == 0 {
            break;
        }
    }
    let callback = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|candidate| candidate.source_entity_index == bee.source_entity_index)
        .unwrap();
    assert_eq!(callback.pending_damage, 300);
    assert_eq!(callback.pending_attacker_entity_index, 0);
    assert_eq!(callback.life, bee.life.wrapping_sub(300));
    // This focused producer/consumer regression does not establish public
    // aiming, spell casting, or the full death/mana gameplay result.
}

#[test]
fn level_two_economy_conformance_death_split_and_merge_vectors() {
    // Pinned remc2 6806c72, EventsFunctions.cpp:26867-26915
    // (TransformEntityToManaSphere_36BA0): the inner fraction shadows the
    // outer loop bound, so even true creates ONE sphere with all mana.
    // The source also advances the sphere-local RNG
    // twice before deriving yaw and speed and clears the dying owner.
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut split = inspection
        .level2_reset_session()
        .expect("economy split session");
    // InitialiseSpells_54A50 reserves the live family-0..3 holder rows
    // before the economy's first ordinary allocation.  Derive the
    // remaining stack from the inspected inactive census rather than
    // baking the pre-holder baseline into this conformance vector.
    let holder_slots = split
        .shared_spells()
        .dynamic_pickups()
        .iter()
        .map(|holder| holder.index)
        .collect::<Vec<_>>();
    let mut expected_free_stack = inspection
        .entities()
        .iter()
        .filter(|entity| {
            entity.index != 0 && !entity.active && !holder_slots.contains(&entity.index)
        })
        .map(|entity| entity.index)
        .collect::<Vec<_>>();
    expected_free_stack.sort_unstable_by(|left, right| right.cmp(left));
    assert_eq!(split.economy().free_slots, expected_free_stack);
    let expected_slots = split
        .economy()
        .free_slots
        .iter()
        .rev()
        .take(1)
        .copied()
        .collect::<Vec<_>>();
    let first_slot = expected_slots[0];
    let first_seed = first_slot.wrapping_add(split.runtime().random_state as u16);
    let first_rng = level2_next_rng16(first_seed);
    let second_rng = level2_next_rng16(first_rng);
    let baseline_hash = split.economy().reset_hash();
    split.economy_mut().begin_turn();
    let sphere_ids = split.economy_mut().spawn_mana_spheres_from_death(
        42,
        Position {
            x: 384,
            y: 62_080,
            z: 1_024,
        },
        0x120,
        0x4321,
        17_000,
        121,
        true,
    );
    assert_eq!(
        sphere_ids,
        expected_slots
            .iter()
            .map(|slot| u32::from(*slot))
            .collect::<Vec<_>>()
    );
    assert_eq!(split.economy().source_rng.get(&42), Some(&0x4321));
    assert_eq!(split.economy().source_owner(42), 0);
    assert_eq!(split.economy().events().len(), 1);
    let first = split
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.id == sphere_ids[0])
        .expect("first spawned sphere");
    assert_eq!(first.entity_rng, second_rng);
    assert_eq!(
        first.yaw,
        i16::try_from((i32::from(first_rng % 0x71) + i32::from(0x120_i16) - 56) & 0x07ff)
            .expect("source yaw fits")
    );
    assert_eq!(first.mana, 17_000);
    assert_eq!(
        split
            .economy()
            .spheres()
            .iter()
            .map(|sphere| sphere.mana)
            .collect::<Vec<_>>(),
        vec![17_000]
    );
    for (event, &sphere_id) in split.economy().events().iter().zip(&sphere_ids) {
        assert_eq!(
            *event,
            Level2EconomyEvent::ManaSphereSpawned {
                sphere_id,
                source_entity_index: 42,
                mana: split
                    .economy()
                    .spheres()
                    .iter()
                    .find(|sphere| sphere.id == sphere_id)
                    .expect("spawned sphere")
                    .mana,
            }
        );
    }
    split.reset();
    assert_eq!(split.economy().reset_hash(), baseline_hash);
    assert!(split.economy().spheres().is_empty());
    let mut invalid_merge = inspection
        .level2_reset_session()
        .expect("invalid merge session");
    invalid_merge.economy_mut().begin_turn();
    assert!(!invalid_merge.economy_mut().merge_mana_spheres(0, 1));
    assert!(invalid_merge.economy().events().is_empty());

    // Pinned remc2 6806c72, EventsFunctions.cpp:26919-26997
    // (sub_36D50): neutral ownership adopts the other side, class-10
    // owners yield to non-class-10 owners, and non-class-10 ties choose
    // the source side when maxMana is equal.
    struct MergeVector {
        target_owner: u16,
        source_owner: u16,
        target_class: u8,
        source_class: u8,
        target_max_mana: i32,
        source_max_mana: i32,
        expected_owner: u16,
    }
    let vectors = [
        MergeVector {
            target_owner: 0,
            source_owner: 201,
            target_class: 0,
            source_class: 3,
            target_max_mana: 0,
            source_max_mana: 0,
            expected_owner: 201,
        },
        MergeVector {
            target_owner: 200,
            source_owner: 201,
            target_class: 10,
            source_class: 3,
            target_max_mana: 900,
            source_max_mana: 100,
            expected_owner: 201,
        },
        MergeVector {
            target_owner: 200,
            source_owner: 201,
            target_class: 3,
            source_class: 10,
            target_max_mana: 900,
            source_max_mana: 100,
            expected_owner: 200,
        },
        MergeVector {
            target_owner: 200,
            source_owner: 201,
            target_class: 10,
            source_class: 10,
            target_max_mana: 100,
            source_max_mana: 900,
            expected_owner: 200,
        },
        MergeVector {
            target_owner: 200,
            source_owner: 201,
            target_class: 3,
            source_class: 3,
            target_max_mana: 100,
            source_max_mana: 100,
            expected_owner: 201,
        },
    ];
    for vector in vectors {
        let mut session = inspection
            .level2_reset_session()
            .expect("merge vector session");
        session
            .economy_mut()
            .owner_class
            .insert(vector.target_owner, vector.target_class);
        session
            .economy_mut()
            .owner_class
            .insert(vector.source_owner, vector.source_class);
        session
            .economy_mut()
            .set_owner_max_mana(vector.target_owner, vector.target_max_mana);
        session
            .economy_mut()
            .set_owner_max_mana(vector.source_owner, vector.source_max_mana);
        let target_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(
                45,
                Position::default(),
                0,
                0x1111,
                100,
                vector.target_owner,
            )
            .expect("merge target");
        let source_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(
                46,
                Position::default(),
                0,
                0x2222,
                50,
                vector.source_owner,
            )
            .expect("merge source");
        session.economy_mut().begin_turn();
        assert!(session.economy_mut().merge_mana_spheres(
            u16::try_from(target_id).expect("target slot"),
            u16::try_from(source_id).expect("source slot"),
        ));
        let target = session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.id == target_id)
            .expect("merged target");
        assert_eq!(
            (
                target.owner_entity_index,
                target.player_entity_index,
                target.mana,
            ),
            (vector.expected_owner, vector.expected_owner, 150),
            "sub_36D50 keeps the semantic owner and raw player relation on the same winning side"
        );
        assert!(
            session
                .economy()
                .spheres()
                .iter()
                .all(|sphere| sphere.id != source_id)
        );
        assert_eq!(
            session.economy().events(),
            &[Level2EconomyEvent::ManaSpheresMerged {
                target_sphere_id: target_id,
                source_sphere_id: source_id,
                owner_entity_index: vector.expected_owner,
                mana: 50,
            }]
        );
    }
}

#[test]
fn level_two_mana_sphere_merge_reclaims_source_slot_immediately() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 merge reclaim session");
    let baseline_free_slots = session.economy().free_slot_count();
    let target_slot = session
        .economy_mut()
        .spawn_mana_sphere_from_death(903, Position::default(), 0, 0x5678, 100, 0)
        .expect("target sphere allocation");
    let source_slot = session
        .economy_mut()
        .spawn_mana_sphere_from_death(904, Position::default(), 0, 0x6789, 50, 0)
        .expect("source sphere allocation");
    let target_generation = session
        .economy()
        .slot_generation(u16::try_from(target_slot).expect("target slot fits"));
    assert_eq!(session.economy().free_slot_count(), baseline_free_slots - 2);
    session.economy_mut().begin_turn();
    assert!(session.economy_mut().merge_mana_spheres(
        u16::try_from(target_slot).expect("target slot fits"),
        u16::try_from(source_slot).expect("source slot fits"),
    ));
    let neutral = session
        .economy()
        .spheres()
        .iter()
        .filter(|sphere| {
            sphere.active
                && !sphere.pending_delete
                && sphere.owner_entity_index == 0
                && sphere.player_entity_index == 0
        })
        .collect::<Vec<_>>();
    assert_eq!(neutral.len(), 1);
    assert_eq!(
        (
            neutral[0].source_entity_index,
            session
                .economy()
                .slot_generation(neutral[0].source_entity_index),
            neutral[0].mana,
        ),
        (
            u16::try_from(target_slot).expect("target slot fits"),
            target_generation,
            150,
        ),
        "a current-neutral census reselects the surviving target with the exact merged payload"
    );
    assert_eq!(
        session.economy().free_slot_count(),
        baseline_free_slots - 1,
        "sub_57F20 makes the absorbed source slot available immediately"
    );
    assert!(
        !session
            .economy()
            .map_chains()
            .linked_entity_indices()
            .contains(&u16::try_from(source_slot).expect("source slot fits"))
    );
    session.economy_mut().reclaim_inactive_slots();
    assert_eq!(
        session.economy().free_slot_count(),
        baseline_free_slots - 1,
        "reclaim pass must not double-release an already absorbed slot"
    );
}

#[test]
fn level_two_economy_conformance_addballon_carrier_vectors() {
    // Pinned remc2 6806c72, EventsFunctions.cpp:61802-61904
    // (AddBallon_60AB0): model-39 collection requires matching owner,
    // wrapped attraction distance, and the strict source AABB before the
    // carrier receives the payload and clears word96 on the sphere.
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let carrier_position = Position::default();
    let mut session = inspection
        .level2_reset_session()
        .expect("carrier vector session");
    let carrier_slot = session
        .economy_mut()
        .spawn_mana_carrier(carrier_position, 111, 0)
        .expect("carrier allocation");
    let wrong_owner = session
        .economy_mut()
        .spawn_mana_sphere_from_death(47, carrier_position, 0, 0x3333, 1, 112)
        .expect("wrong-owner sphere");
    let far = session
        .economy_mut()
        .spawn_mana_sphere_from_death(
            48,
            Position {
                x: 2_048,
                ..carrier_position
            },
            0,
            0x4444,
            1,
            111,
        )
        .expect("far sphere");
    let near_but_outside_aabb = session
        .economy_mut()
        .spawn_mana_sphere_from_death(
            49,
            Position {
                x: 400,
                ..carrier_position
            },
            0,
            0x5555,
            1,
            111,
        )
        .expect("near non-overlap sphere");
    let valid = session
        .economy_mut()
        .spawn_mana_sphere_from_death(50, carrier_position, 0, 0x6666, 321, 111)
        .expect("valid sphere");
    session.economy_mut().begin_turn();
    for rejected in [wrong_owner, far, near_but_outside_aabb] {
        assert!(!session.economy_mut().absorb_sphere_into_carrier(
            u16::try_from(rejected).expect("sphere slot"),
            carrier_slot,
        ));
    }
    assert!(
        session
            .economy_mut()
            .absorb_sphere_into_carrier(u16::try_from(valid).expect("sphere slot"), carrier_slot,)
    );
    let carrier = session.economy().carrier().expect("active carrier");
    assert_eq!((carrier.carried_mana, carrier.target_sphere_id), (321, 0));
    assert_eq!(
        session.economy().events(),
        &[Level2EconomyEvent::ManaSphereCollected {
            sphere_id: valid,
            carrier_id: u32::from(carrier_slot),
            mana: 321,
        }]
    );
    let collected = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.id == valid)
        .expect("valid sphere remains allocated through its callback");
    assert!(collected.active);
    assert!(collected.pending_delete);
    assert_ne!(collected.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
    assert_eq!(
        session
            .economy()
            .spheres()
            .iter()
            .filter(|sphere| sphere.active)
            .count(),
        4
    );
    let collected_slot = collected.source_entity_index;
    session
        .economy_mut()
        .reclaim_inactive_slots_at_source_boundary();
    assert!(
        !session
            .economy()
            .spheres()
            .iter()
            .any(|sphere| sphere.source_entity_index == collected_slot)
    );
    assert!(!session.economy().map_chains().is_linked(collected_slot));
    assert!(session.economy().free_slots.contains(&collected_slot));
}

#[test]
fn level_two_economy_conformance_castle_action_vectors_and_allocator_fallback() {
    // Pinned remc2 6806c72, Events.cpp:2937-2964 dispatches action 4/6
    // through the Castle helpers; EventsFunctions.cpp:61162-61278 binds
    // the construction state machine and sub_5FCA0's free-arena gate.
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let footprint =
        CastleBuildFootprint::from_owner_cells(1, 8, 8, &[0xff; 128]).expect("source BUILD vector");
    let projectile = Position {
        x: 0x13_45,
        y: 0x56_78,
        z: 1_234,
    };
    let mut production = inspection
        .level2_reset_session()
        .expect("production Castle session");
    let castle_slot = production
        .economy_mut()
        .build_castle_with_footprint(projectile, 111, &footprint)
        .expect("production Castle allocation");
    let castle = production.economy().castle().expect("production Castle");
    assert_eq!((castle.position.x, castle.position.y), (0x14_00, 0x56_00));
    assert_eq!(
        (castle.anchor_position.x, castle.anchor_position.y),
        (0x14_00, 0x56_00)
    );
    production.economy_mut().begin_turn();
    assert!(production.economy_mut().step_castle_action(castle_slot));
    let construction = production
        .economy()
        .castle_children()
        .iter()
        .find(|child| child.active && child.model == 42)
        .expect("production construction child");
    assert_eq!(construction.position, castle.anchor_position);
    assert_eq!(
        production.economy().events()[0],
        Level2EconomyEvent::CastleConstructionSound {
            source_entity_index: castle_slot,
            sound_id: 10,
        }
    );

    // The missing-child fallback scans only on phase multiples of 32, while
    // the source refreshes ground Z and advances phase on every visit.
    let mut waiting = inspection.level2_reset_session().expect("waiting Castle session");
    let waiting_slot = waiting.economy_mut().build_castle(Position::default(), 111)
        .expect("waiting Castle");
    let ground = waiting.canonical_terrain().ground_height(Position::default());
    {
        let mut economy = waiting.economy_mut();
        assert!(economy.castle_children().iter().all(|child| !child.active || child.model != 42));
        let castle = economy.castle.as_mut().expect("waiting Castle");
        castle.action = 5;
        castle.build_state = 4;
        castle.phase = 30;
        castle.position.z = ground.wrapping_add(123);
    }
    for phase in [31, 32] {
        assert!(waiting.economy_mut().step_castle_action(waiting_slot));
        let castle = waiting.economy().castle().unwrap();
        assert_eq!((castle.build_state, castle.phase, castle.position.z), (4, phase, ground));
    }
    assert!(waiting.economy_mut().step_castle_action(waiting_slot));
    let castle = waiting.economy().castle().unwrap();
    assert_eq!((castle.build_state, castle.phase), (3, 33));

    // sub_5FF50/EndOfCastleProjectile's action-4 even-phase branch absorbs
    // an owned sphere before the source phase byte advances.
    let mut action4 = inspection
        .level2_reset_session()
        .expect("action-4 Castle session");
    let action4_slot = action4
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("action-4 Castle");
    let action4_position = action4.economy().castle().expect("Castle").position;
    let action4_sphere = action4
        .economy_mut()
        .spawn_mana_sphere_from_death(51, action4_position, 0, 0x7777, 100, 111)
        .expect("action-4 sphere");
    action4
        .economy_mut()
        .castle
        .as_mut()
        .expect("action-4 Castle")
        .phase = 0;
    action4.economy_mut().begin_turn();
    assert!(action4.economy_mut().step_castle_action(action4_slot));
    assert_eq!(action4.economy().castle().expect("Castle").mana, 100);
    assert_eq!(action4.economy().castle().expect("Castle").phase, 1);
    let collected = action4
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.id == action4_sphere)
        .expect("action-4 sphere remains allocated through its callback");
    assert!(collected.active);
    assert!(collected.pending_delete);
    assert_ne!(collected.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
    assert_eq!(
        action4.economy().events(),
        &[Level2EconomyEvent::ManaSphereCollected {
            sphere_id: action4_sphere,
            carrier_id: u32::from(action4_slot),
            mana: 100,
        }]
    );
    let collected_slot = collected.source_entity_index;
    action4
        .economy_mut()
        .reclaim_inactive_slots_at_source_boundary();
    assert!(
        !action4
            .economy()
            .spheres()
            .iter()
            .any(|sphere| sphere.source_entity_index == collected_slot)
    );
    assert!(!action4.economy().map_chains().is_linked(collected_slot));
    assert!(action4.economy().free_slots.contains(&collected_slot));

    // With a free slot, sub_5FCA0 emits destruction sound before the
    // stage-destroyed and final Castle-destroyed events. This is a source
    // ordering vector, not just a final-state assertion.
    let mut teardown = inspection
        .level2_reset_session()
        .expect("action-6 Castle session");
    let teardown_slot = teardown
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("action-6 Castle");
    {
        let mut economy_view = teardown.economy_mut();
        let castle = economy_view.castle.as_mut().expect("Castle");
        castle.action = 6;
        castle.level = 1;
        castle.life = -1;
        castle.max_life = 20_000;
    }
    teardown.economy_mut().begin_turn();
    assert!(teardown.economy_mut().step_castle_action(teardown_slot));
    assert_eq!(
        teardown.economy().events(),
        &[
            Level2EconomyEvent::CastleDestructionSound {
                source_entity_index: teardown_slot,
                sound_id: 30,
            },
            Level2EconomyEvent::CastleStageDestroyed {
                source_entity_index: teardown_slot,
                previous_level: 1,
                next_level: 0,
            },
            Level2EconomyEvent::CastleDestroyed {
                source_entity_index: teardown_slot,
                killer_entity_index: 0,
            },
        ]
    );
    assert!(
        !teardown
            .economy()
            .castle()
            .expect("destroyed Castle")
            .active
    );

    // The source writes the dying source RNG/owner before allocation and
    // leaves no partial event when the source arena is exhausted.
    let mut exhausted = inspection
        .level2_reset_session()
        .expect("allocator fallback session");
    exhausted.economy_mut().free_slots.clear();
    exhausted.economy_mut().begin_turn();
    assert!(
        exhausted
            .economy_mut()
            .spawn_mana_sphere_from_death(52, Position::default(), 0, 0x9876, 12, 111)
            .is_none()
    );
    assert_eq!(exhausted.economy().source_rng.get(&52), Some(&0x9876));
    assert_eq!(exhausted.economy().source_owner(52), 0);
    assert!(exhausted.economy().events().is_empty());

    // sub_5FCA0's no-free-slot branch resets action to 4 and does not
    // commit a destruction sound or stage transition.
    let mut blocked = inspection
        .level2_reset_session()
        .expect("blocked action-6 session");
    let blocked_slot = blocked
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("blocked Castle");
    {
        let mut economy = blocked.economy_mut();
        economy.free_slots.clear();
        let castle = economy.castle.as_mut().expect("blocked Castle");
        castle.action = 6;
        castle.level = 1;
    }
    blocked.economy_mut().begin_turn();
    assert!(blocked.economy_mut().step_castle_action(blocked_slot));
    let blocked_castle = blocked.economy().castle().expect("blocked Castle");
    assert_eq!((blocked_castle.action, blocked_castle.level), (4, 1));
    assert!(blocked.economy().events().is_empty());
}

#[test]
fn level_two_castle_four_passes_collect_a_disabled_sphere_once() {
    // Pinned remc2 6806c72, EndOfCastleProjectile_5F8F0 adds the first
    // owned model-39 row from the live chain and immediately calls
    // DisableEntityDrawing04_57F10. Later UpdateEntities passes cannot
    // find that disabled row again before its following-boundary reclaim.
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("four-pass Castle session");
    let castle_slot = session
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("action-4 Castle");
    let castle_position = session.economy().castle().expect("Castle").position;
    let sphere_id = session
        .economy_mut()
        .spawn_mana_sphere_from_death(51, castle_position, 0, 0x7777, 500, 111)
        .expect("owned Castle sphere");
    session
        .economy_mut()
        .castle
        .as_mut()
        .expect("action-4 Castle")
        .phase = 0;
    session.economy_mut().begin_turn();

    for _ in 0..4 {
        assert!(session.economy_mut().step_castle_action(castle_slot));
    }

    assert_eq!(session.economy().castle().expect("Castle").mana, 500);
    assert_eq!(
        session
            .economy()
            .events()
            .iter()
            .filter(|event| matches!(
                event,
                Level2EconomyEvent::ManaSphereCollected {
                    sphere_id: collected,
                    carrier_id,
                    mana: 500,
                } if *collected == sphere_id && *carrier_id == u32::from(castle_slot)
            ))
            .count(),
        1
    );
    let sphere = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.id == sphere_id)
        .expect("disabled sphere remains allocated until source-boundary reclaim");
    assert!(sphere.pending_delete);
    assert_ne!(sphere.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);

    // The current source player relation, not the sphere's earlier owner,
    // controls which Castle may collect it after a relation change.
    let foreign = session
        .economy_mut()
        .spawn_mana_sphere_from_death(51, castle_position, 0, 0x7778, 300, 111)
        .expect("sphere with a changed rival relation");
    let owned = session
        .economy_mut()
        .spawn_mana_sphere_from_death(51, castle_position, 0, 0x7779, 700, 112)
        .expect("sphere with a changed local relation");
    {
        let mut economy = session.economy_mut();
        economy.spheres.iter_mut().find(|sphere| sphere.id == foreign)
            .unwrap().player_entity_index = 112;
        economy.spheres.iter_mut().find(|sphere| sphere.id == owned)
            .unwrap().player_entity_index = 111;
        economy.castle.as_mut().expect("action-4 Castle").phase = 0;
        economy.begin_turn();
        assert!(economy.step_castle_action(castle_slot));
    }
    assert_eq!(session.economy().castle().unwrap().mana, 1_200);
    assert!(session.economy().events().iter().any(|event| matches!(event,
        Level2EconomyEvent::ManaSphereCollected { sphere_id, mana: 700, .. } if *sphere_id == owned
    )));
    assert!(!session.economy().spheres().iter()
        .find(|sphere| sphere.id == foreign).unwrap().pending_delete);
    assert!(session.economy().spheres().iter()
        .find(|sphere| sphere.id == owned).unwrap().pending_delete);
}

#[test]
fn level_two_castle_stage_controller_matches_source() {
    let package = level_two_fixture_path();
    let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level2-day-owner-cache/components/gameplay");
    let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
        .expect("qualified Level 2 gameplay component");
    assert_eq!(inspection.metadata().vga_resolution_selector, 1);
    assert_eq!(
        LEVEL2_CASTLE_MODEL41_MSPRD_RECTS,
        [
            (0, 0),
            (16, 11),
            (16, 11),
            (16, 9),
            (16, 9),
            (16, 11),
            (1, 0),
            (1, 0)
        ]
    );
    let expected_dimensions = [
        (1, (8, 8)),
        (2, (21, 21)),
        (3, (21, 21)),
        (4, (35, 35)),
        (5, (35, 35)),
        (6, (48, 48)),
        (7, (48, 48)),
    ];
    assert_eq!(inspection.level2_castle_stage_footprints.len(), 7);
    for (stage, (width, height)) in expected_dimensions {
        let footprint = inspection
            .level2_castle_stage_footprints
            .get(&stage)
            .expect("every Castle stage footprint is installed");
        assert_eq!(
            (footprint.index, footprint.width, footprint.height),
            (stage, width, height)
        );
        assert_eq!(
            footprint.cells().len(),
            usize::from(width) * usize::from(height)
        );
    }

    // The stage controller is reached through the ordinary Level 2
    // source pass, using the footprint installed by the qualified cache.
    let stage_one = inspection
        .level2_castle_stage_footprints
        .get(&1)
        .expect("stage-one footprint")
        .clone();
    let mut controller = inspection
        .level2_reset_session()
        .expect("cache-loaded controller session");
    controller.select_level2_campaign_entry_stage();
    let castle_slot = controller
        .economy_mut()
        .build_castle_with_footprint(
            Position {
                x: 0x20_00,
                y: 0x30_00,
                z: 0,
            },
            111,
            &stage_one,
        )
        .expect("ordinary Castle allocation");
    controller.step_outer_turn(PlayerCommand::default(), 0);
    let castle = controller.economy().castle().expect("live Castle");
    assert_eq!(castle.source_entity_index, castle_slot);
    assert_eq!((castle.level, castle.controller_chain_count), (1, 1));
    let stage_controller_slot = controller
        .dispatch_events()
        .iter()
        .find_map(|event| match event {
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::CastleController,
                ..
            } => Some(*source_slot),
            _ => None,
        })
        .expect("ordinary source-pass stage controller");
    assert_eq!(
        stage_controller_slot,
        controller
            .economy()
            .castle()
            .expect("Castle after controller dispatch")
            .controller_chain_slots[0]
    );
    let first_controller = controller
        .economy()
        .castle_children()
        .iter()
        .find(|child| child.source_entity_index == stage_controller_slot)
        .copied()
        .expect("live model-79 controller after first callback");
    assert_eq!(
        (first_controller.model, first_controller.action),
        (79, 0x56)
    );
    assert_eq!(
        (first_controller.life, first_controller.max_life),
        (100_000, 100_000)
    );
    assert_eq!(first_controller.cave_collision_cooldown, 1);
    assert_eq!(
        first_controller.source_byte_3e,
        (stage_controller_slot as u8).wrapping_add(1)
    );
    assert_eq!(
        first_controller.axis_position.x,
        first_controller.position.x
    );
    assert_eq!(
        first_controller.axis_position.y,
        first_controller.position.y
    );
    let ground = controller
        .canonical_terrain()
        .ground_height(first_controller.axis_position);
    let base_z = ground.saturating_add(384);
    assert_eq!(
        first_controller.position.z,
        if first_controller.axis_position.z < base_z {
            base_z
        } else if stage_controller_slot as u8 & 8 != 0 {
            first_controller.axis_position.z.saturating_add(6)
        } else {
            first_controller.axis_position.z.saturating_sub(6)
        }
    );
    assert!(
        controller
            .economy()
            .map_chains()
            .is_linked(stage_controller_slot)
    );
    assert!(
        !controller
            .economy()
            .free_slots
            .contains(&stage_controller_slot)
    );
    let second_rng = first_controller
        .entity_rng
        .wrapping_mul(9_377)
        .wrapping_add(9_439);
    assert!(
        controller
            .economy_mut()
            .step_castle_child_terminal_slot(stage_controller_slot)
    );
    let second_controller = controller
        .economy()
        .castle_children()
        .iter()
        .find(|child| child.source_entity_index == stage_controller_slot)
        .copied()
        .expect("live model-79 controller after second callback");
    assert_eq!(second_controller.entity_rng, second_rng);
    assert_eq!(second_controller.cave_collision_cooldown, 2);
    assert_eq!(
        second_controller.source_dword_10,
        i32::from(second_rng % 0x30) + 15
    );
    assert_eq!(
        second_controller.source_byte_3e,
        first_controller.source_byte_3e.wrapping_add(1)
    );
    assert!(
        controller
            .economy()
            .map_chains()
            .is_linked(stage_controller_slot)
    );
    assert!(controller.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::Castle { source_slot, .. }
                if *source_slot == castle_slot
        )
    }));

    // The even-phase maintenance call is exercised only by the ordinary
    // source route; no direct maintenance helper call is used here.
    let mut maintenance = inspection
        .level2_reset_session()
        .expect("cache-loaded maintenance session");
    maintenance.select_level2_campaign_entry_stage();
    let maintenance_slot = maintenance
        .economy_mut()
        .build_castle_with_footprint(Position::default(), 111, &stage_one)
        .expect("maintenance Castle allocation");
    maintenance.step_outer_turn(PlayerCommand::default(), 0);
    assert!(maintenance.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                kind: Level2DynamicArenaKind::CastleChild,
                ..
            }
        )
    }));
    assert_eq!(
        (
            maintenance.economy().castle().expect("Castle").action,
            maintenance.economy().castle().expect("Castle").build_state,
        ),
        (5, 4)
    );
    {
        let mut economy_view = maintenance.economy_mut();
        let castle = economy_view.castle.as_mut().expect("Castle");
        castle.action = 4;
        castle.timer = 0;
        castle.phase &= !1;
    }
    maintenance.step_outer_turn(PlayerCommand::default(), 0);
    let maintenance_defender_slot = maintenance
        .economy()
        .castle()
        .expect("Castle")
        .defender_slots
        .iter()
        .take(usize::from(
            maintenance
                .economy()
                .castle()
                .expect("Castle")
                .defender_count,
        ))
        .copied()
        .find(|slot| *slot != 0)
        .expect("source defender relation");
    let defender = maintenance
        .economy()
        .castle_children()
        .iter()
        .find(|child| {
            child.active
                && child.source_entity_index == maintenance_defender_slot
                && child.model == 3
                && child.action == 9
        })
        .expect("even-phase Castle defender maintenance");
    assert!(
        maintenance
            .economy()
            .map_chains()
            .is_linked(defender.source_entity_index)
    );
    assert!(maintenance.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::Castle { source_slot, .. }
                if *source_slot == maintenance_slot
        )
    }));

    // Destruction indexes the loaded old-stage row, and reset restores
    // the cache-loaded arena, terrain authority, and campaign digest.
    let mut teardown = inspection
        .level2_reset_session()
        .expect("cache-loaded teardown session");
    teardown.select_level2_campaign_entry_stage();
    let baseline_campaign_hash = teardown.campaign_state_hash();
    let baseline_terrain_hash = teardown.terrain_hash();
    let baseline_free_slots = teardown.economy().free_slots.len();
    let teardown_slot = teardown
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("teardown Castle allocation");
    {
        let mut economy_view = teardown.economy_mut();
        let castle = economy_view.castle.as_mut().expect("Castle");
        castle.action = 6;
        castle.level = 1;
        castle.life = -1;
    }
    teardown.step_outer_turn(PlayerCommand::default(), 0);
    assert!(teardown.economy().events().iter().any(|event| {
        matches!(
            event,
            Level2EconomyEvent::CastleStageDestroyed {
                source_entity_index,
                previous_level: 1,
                next_level: 0,
            } if *source_entity_index == teardown_slot
        )
    }));
    assert!(!teardown.economy().map_chains().is_linked(teardown_slot));
    assert_ne!(teardown.terrain_hash(), baseline_terrain_hash);
    teardown.reset();
    assert_eq!(teardown.campaign_state_hash(), baseline_campaign_hash);
    assert_eq!(teardown.terrain_hash(), baseline_terrain_hash);
    assert_eq!(teardown.economy().free_slots.len(), baseline_free_slots);
    assert_eq!(teardown.economy().castle_stage_footprints.len(), 7);

    // A binding mutation with its stale payload digest is rejected before
    // any gameplay rows can be installed.
    let tampered_root = std::env::temp_dir().join(format!(
        "mc-bvihb1-gameplay-manifest-tamper-{}",
        std::process::id()
    ));
    if tampered_root.exists() {
        fs::remove_dir_all(&tampered_root).expect("remove stale tamper cache");
    }
    fs::create_dir_all(&tampered_root).expect("create tamper cache");
    fs::copy(
        gameplay_cache.join("manifest.json"),
        tampered_root.join("manifest.json"),
    )
    .expect("copy gameplay manifest");
    let manifest_path = tampered_root.join("manifest.json");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read gameplay manifest"))
            .expect("parse gameplay manifest");
    manifest["bindings"]["level_export_package_sha256"] = json!("0".repeat(64));
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("encode tampered manifest"),
    )
    .expect("write tampered manifest");
    assert!(NativeWorld::inspect_level2_with_owner_cache(&package, &tampered_root).is_err());
    let mut unsigned_manifest = manifest.clone();
    unsigned_manifest
        .as_object_mut()
        .expect("tampered manifest object")
        .remove("manifest_payload_sha256");
    let canonical_manifest = serde_json::to_string_pretty(&unsigned_manifest)
        .expect("encode tampered manifest canonically")
        + "\n";
    manifest["manifest_payload_sha256"] = json!(sha256_hex(canonical_manifest.as_bytes()));
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("encode binding-tampered manifest"),
    )
    .expect("write binding-tampered manifest");
    assert!(NativeWorld::inspect_level2_with_owner_cache(&package, &tampered_root).is_err());
    fs::remove_dir_all(tampered_root).expect("remove test tamper cache");

    // Refresh the DAT declaration, component content identity, and
    // canonical manifest payload after changing a consumed BUILD byte.
    // The immutable qualified BUILD-root pin must still reject it.
    let dat_tampered_root = std::env::temp_dir().join(format!(
        "mc-bvihb1-gameplay-dat-tamper-{}",
        std::process::id()
    ));
    if dat_tampered_root.exists() {
        fs::remove_dir_all(&dat_tampered_root).expect("remove stale DAT tamper cache");
    }
    fs::create_dir_all(dat_tampered_root.join("gameplay")).expect("create DAT tamper cache");
    fs::copy(
        gameplay_cache.join("manifest.json"),
        dat_tampered_root.join("manifest.json"),
    )
    .expect("copy DAT tamper manifest");
    fs::copy(
        gameplay_cache.join("gameplay/DATA_BUILD0-0.TAB.bin"),
        dat_tampered_root.join("gameplay/DATA_BUILD0-0.TAB.bin"),
    )
    .expect("copy DAT tamper TAB");
    let dat_path = dat_tampered_root.join("gameplay/DATA_BUILD0-0.DAT.bin");
    fs::copy(
        gameplay_cache.join("gameplay/DATA_BUILD0-0.DAT.bin"),
        &dat_path,
    )
    .expect("copy DAT tamper DAT");
    let mut dat = fs::read(&dat_path).expect("read DAT tamper DAT");
    assert_eq!(dat.first(), Some(&255));
    dat[0] = 254;
    fs::write(&dat_path, &dat).expect("write DAT tamper DAT");
    let dat_sha256 = sha256_hex(&dat);
    let dat_manifest_path = dat_tampered_root.join("manifest.json");
    let mut dat_manifest: Value =
        serde_json::from_slice(&fs::read(&dat_manifest_path).expect("read DAT tamper manifest"))
            .expect("parse DAT tamper manifest");
    let outputs = dat_manifest["outputs"]
        .as_array_mut()
        .expect("DAT tamper manifest outputs");
    let mut replaced_dat_declaration = false;
    for output in outputs {
        if output["path"] == json!("gameplay/DATA_BUILD0-0.DAT.bin") {
            output["sha256"] = json!(dat_sha256);
            replaced_dat_declaration = true;
        }
    }
    assert!(replaced_dat_declaration);
    let mut content_identity = String::new();
    {
        use std::fmt::Write as _;
        for output in dat_manifest["outputs"]
            .as_array()
            .expect("DAT tamper manifest outputs")
        {
            writeln!(
                &mut content_identity,
                "{}\0{}\0{}",
                output["path"].as_str().expect("DAT tamper output path"),
                output["sha256"].as_str().expect("DAT tamper output hash"),
                output["bytes"].as_u64().expect("DAT tamper output bytes")
            )
            .expect("write DAT tamper content identity");
        }
    }
    dat_manifest["cache_content_sha256"] = json!(sha256_hex(content_identity.as_bytes()));
    let mut unsigned_dat_manifest = dat_manifest.clone();
    unsigned_dat_manifest
        .as_object_mut()
        .expect("DAT tamper manifest object")
        .remove("manifest_payload_sha256");
    let canonical_dat_manifest = serde_json::to_string_pretty(&unsigned_dat_manifest)
        .expect("encode DAT tamper manifest canonically")
        + "\n";
    dat_manifest["manifest_payload_sha256"] = json!(sha256_hex(canonical_dat_manifest.as_bytes()));
    fs::write(
        &dat_manifest_path,
        serde_json::to_vec_pretty(&dat_manifest).expect("encode DAT tamper manifest"),
    )
    .expect("write DAT tamper manifest");
    assert!(NativeWorld::inspect_level2_with_owner_cache(&package, &dat_tampered_root).is_err());
    fs::remove_dir_all(dat_tampered_root).expect("remove DAT tamper cache");
}

#[test]
fn level_two_economy_dispatch_uses_one_terrain_and_reset_authority() {
    let package = level_two_fixture_path();
    let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level2-day-owner-cache/components/gameplay");
    let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
        .expect("qualified Level 2 gameplay component");
    assert_eq!(inspection.level2_house_build_footprints.len(), 9);
    for building_index in [26_u8, 29, 30, 44, 50, 55, 59, 60, 64] {
        let footprint = inspection
            .level2_house_build_footprints
            .get(&building_index)
            .expect("qualified Level 2 House BUILD footprint");
        assert_eq!(footprint.index, building_index);
        assert_eq!(
            footprint.cells().len(),
            usize::from(footprint.width) * usize::from(footprint.height)
        );
    }

    let mut session = inspection
        .level2_reset_session()
        .expect("cache-loaded Level 2 session");
    session.select_level2_campaign_entry_stage();
    let baseline_terrain_hash = session.terrain_hash();
    let baseline_campaign_hash = session.campaign_state_hash();
    let baseline_ordinary_hash = session.ordinary().reset_hash();
    let baseline_free_count = session.economy().free_slots.len();
    let baseline_arena_slots = session.economy().live_arena_slots();
    let baseline_family_counts = (
        session
            .ordinary()
            .houses()
            .iter()
            .filter(|house| house.active)
            .count(),
        session
            .ordinary()
            .villagers()
            .iter()
            .filter(|villager| villager.active)
            .count(),
        session
            .ordinary()
            .dolmens()
            .iter()
            .filter(|dolmen| dolmen.entity().active)
            .count(),
        session.economy().live_arena_slots(),
    );
    let house = session
        .ordinary()
        .houses()
        .iter()
        .copied()
        .find(|house| house.parameter.produces_villagers())
        .expect("qualified Level 2 resident-producing House");
    assert!(
        inspection
            .level2_house_build_footprints
            .contains_key(&house.building_index)
    );

    // Possession is a source mailbox consumed by the same ordinary House
    // visit.  Its owner relation is projected into the shared economy
    // source-owner map, not a parallel test carrier.
    let possession_owner = session.state.entity_index;
    session
        .ordinary_mut()
        .submit_house_possession(house.entity_index, possession_owner, 1)
        .expect("qualified House possession mailbox");
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::House { events, .. }
                if events.iter().any(|event| matches!(
                    event,
                    HouseStepEvent::Possessed {
                        entity_index,
                        owner_entity_index,
                        payload: 1,
                    } if *entity_index == house.entity_index
                        && *owner_entity_index == possession_owner
                ))
        )
    }));
    assert_eq!(
        session.economy().source_owner(house.entity_index),
        possession_owner
    );
    session.reset();
    assert_eq!(session.economy().source_owner(house.entity_index), 0);

    // The qualified owner row starts with two occupants. The existing
    // resident-entry operation supplies the third occupant before the
    // ordinary House source visit; no House row is rewritten here.
    assert_eq!(house.occupants, 2);
    assert_eq!(
        session
            .ordinary_mut()
            .houses
            .try_enter_resident(house.entity_index),
        HouseResidentEntry::Entered
    );
    session
        .ordinary_mut()
        .inject_house_damage(house.entity_index, 1, 111)
        .expect("qualified House resident damage mailbox");
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::House { events, .. }
                if events.iter().any(|event| matches!(
                    event,
                    HouseStepEvent::ResidentRequested {
                        entity_index,
                        model: 4,
                        action: 33,
                        ..
                    } if *entity_index == house.entity_index
                ))
        )
    }));
    let resident = session
        .economy()
        .house_residents()
        .iter()
        .find(|resident| resident.parent_house_entity_index == house.entity_index)
        .copied()
        .expect("resident constructor reaches shared Level 2 arena");
    assert_eq!((resident.model, resident.action), (4, 33));
    assert_eq!(resident.semantic_kind, CreatureKind::Archer);
    let resident_hash = session.economy().reset_hash();
    {
        let resident = session
            .economy
            .economy
            .house_residents
            .iter_mut()
            .find(|candidate| candidate.source_entity_index == resident.source_entity_index)
            .expect("resident semantic identity remains authoritative");
        resident.semantic_kind = CreatureKind::Builder;
    }
    assert_ne!(session.economy().reset_hash(), resident_hash);
    session
        .economy
        .economy
        .house_residents
        .iter_mut()
        .find(|candidate| candidate.source_entity_index == resident.source_entity_index)
        .expect("resident semantic identity remains restorable")
        .semantic_kind = CreatureKind::Archer;
    assert_eq!(session.economy().reset_hash(), resident_hash);
    assert_eq!(
        session.economy().source_owner(resident.source_entity_index),
        resident.owner_entity_index
    );
    assert!(
        session
            .economy()
            .map_chains()
            .is_linked(resident.source_entity_index)
    );
    session.reset();

    // A rejected NewEvent allocation drops only the requested resident.
    // The live House callback has already consumed its damage mailbox,
    // changed life/occupancy, and reached the common byte-0x3E epilogue.
    assert_eq!(
        session
            .ordinary_mut()
            .houses
            .try_enter_resident(house.entity_index),
        HouseResidentEntry::Entered
    );
    session.economy.economy.free_slots.clear();
    session.economy.economy.replacement_cursor_exclusions = (1_u16..1_000).collect();
    let damage_before = session
        .ordinary()
        .houses()
        .iter()
        .copied()
        .find(|candidate| candidate.entity_index == house.entity_index)
        .expect("damage-exhaustion House row");
    let damage_terrain_before = session.terrain_hash();
    session
        .ordinary_mut()
        .inject_house_damage(house.entity_index, 1, 111)
        .expect("damage-exhaustion mailbox");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let damage_after = session
        .ordinary()
        .houses()
        .iter()
        .copied()
        .find(|candidate| candidate.entity_index == house.entity_index)
        .expect("damage-exhaustion House result");
    assert_eq!(damage_after.life, damage_before.life.wrapping_sub(1));
    assert_eq!(damage_after.occupants, damage_before.occupants - 1);
    assert_eq!(damage_after.pending_damage, 0);
    assert_eq!(damage_after.pending_attacker, 0);
    assert_eq!(damage_after.phase, damage_before.phase.wrapping_add(1));
    assert_eq!(session.terrain_hash(), damage_terrain_before);
    assert!(session.economy().house_residents().is_empty());
    assert!(session.economy().map_chains().is_linked(house.entity_index));
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::Error { source_slot, .. }
            if *source_slot == house.entity_index
    )));
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::House { source_slot, events, .. }
            if *source_slot == house.entity_index
                && events.iter().any(|event| matches!(
                    event,
                    HouseStepEvent::ResidentRequested {
                        attacker_entity_index: 111,
                        ..
                    }
                ))
    )));
    session.reset();

    // The periodic source branch consumes an eligibility draw and then a
    // resident-selection draw before NewEvent is attempted. Exhausting the
    // allocator must not rewind either draw.
    let periodic_capacity = house.capacity;
    assert!(periodic_capacity > 5);
    let periodic_seed = (0..=u16::MAX)
        .find(|seed| {
            let after_eligibility = level2_next_rng16(*seed);
            i32::from(after_eligibility % periodic_capacity.cast_unsigned())
                > i32::from(periodic_capacity) - (i32::from(periodic_capacity) >> 4) - 2
        })
        .expect("qualified House has an eligible periodic RNG seed");
    {
        let mut ordinary = session.ordinary_mut();
        let periodic = ordinary
            .house_mut(house.entity_index)
            .expect("periodic House row");
        periodic.phase = 0;
        periodic.occupants = i32::from(periodic_capacity);
        periodic.rng = periodic_seed;
    }
    session.economy.economy.free_slots.clear();
    session.economy.economy.replacement_cursor_exclusions = (1_u16..1_000).collect();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let periodic_after = session
        .ordinary()
        .houses()
        .iter()
        .copied()
        .find(|candidate| candidate.entity_index == house.entity_index)
        .expect("periodic House result");
    assert_eq!(
        periodic_after.rng,
        level2_next_rng16(level2_next_rng16(periodic_seed))
    );
    assert_eq!(periodic_after.occupants, i32::from(periodic_capacity));
    assert_eq!(periodic_after.phase, 1);
    assert!(session.economy().house_residents().is_empty());
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::Error { source_slot, .. }
            if *source_slot == house.entity_index
    )));
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::House { source_slot, events, .. }
            if *source_slot == house.entity_index
                && events.iter().any(|event| matches!(
                    event,
                    HouseStepEvent::ResidentRequested {
                        attacker_entity_index: 0,
                        ..
                    }
                ))
    )));
    session.reset();

    // Source NewEvent exhaustion skips only the requested resident. The
    // House occupant/RNG mutation, terrain removal, draw-disable, and
    // UpdateEntities byte-0x3E epilogue remain committed.
    session
        .ordinary_mut()
        .inject_house_damage(house.entity_index, house.life.wrapping_add(1), 111)
        .expect("exhaustion House damage mailbox");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let failed_house = session
        .ordinary()
        .houses()
        .iter()
        .copied()
        .find(|candidate| candidate.entity_index == house.entity_index)
        .expect("failed House source row");
    let mut expected_removal_terrain = session.canonical_terrain().clone();
    let expected_removal = expected_removal_terrain
        .apply_house_removal(
            failed_house.position,
            failed_house.rng,
            failed_house.occupants,
            failed_house.parameter.produces_villagers(),
            inspection
                .level2_house_build_footprints
                .get(&failed_house.building_index)
                .expect("qualified removal footprint"),
        )
        .expect("independent source-shaped House removal result");

    // Definition/terrain failures are different from ordinary NewEvent
    // exhaustion: they abort the scheduler visit and restore the complete
    // pre-turn simulation snapshot, leaving only one typed error receipt.
    let mut malformed = session.clone();
    assert!(
        malformed
            .ordinary_mut()
            .house_build_footprints
            .remove(&house.building_index)
            .is_some()
    );
    let malformed_live_before = malformed.session.state.live.clone();
    let malformed_terrain_before = malformed.terrain_hash();
    let malformed_ordinary_before = malformed.ordinary().reset_hash();
    let malformed_economy_before = malformed.economy().reset_hash();
    let malformed_map_before = malformed.economy().map_chains().linked_entity_indices();
    let malformed_house_before = malformed
        .ordinary()
        .houses()
        .iter()
        .copied()
        .find(|candidate| candidate.entity_index == house.entity_index)
        .expect("malformed-footprint House before visit");
    malformed.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(malformed.session.state.live, malformed_live_before);
    assert_eq!(malformed.terrain_hash(), malformed_terrain_before);
    assert_eq!(malformed.ordinary().reset_hash(), malformed_ordinary_before);
    assert_eq!(malformed.economy().reset_hash(), malformed_economy_before);
    assert_eq!(
        malformed.economy().map_chains().linked_entity_indices(),
        malformed_map_before
    );
    assert_eq!(
        malformed
            .ordinary()
            .houses()
            .iter()
            .copied()
            .find(|candidate| candidate.entity_index == house.entity_index),
        Some(malformed_house_before)
    );
    assert_eq!(
        malformed
            .dispatch_events()
            .iter()
            .filter(|event| matches!(
                event,
                Mc2DispatchEvent::Error { source_slot, .. }
                    if *source_slot == house.entity_index
            ))
            .count(),
        1
    );
    assert!(!malformed.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::House { source_slot, .. }
            if *source_slot == house.entity_index
    )));

    let mut prefix = session.clone();
    let prefix_slot = *prefix
        .economy()
        .free_slots
        .last()
        .expect("one ordinary NewEvent slot for prefix proof");
    prefix.economy.economy.free_slots = vec![prefix_slot];
    prefix.economy.economy.replacement_cursor_exclusions = (1_u16..1_000).collect();
    prefix.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(prefix.canonical_terrain(), &expected_removal_terrain);
    let prefix_house = prefix
        .ordinary()
        .houses()
        .iter()
        .copied()
        .find(|candidate| candidate.entity_index == house.entity_index)
        .expect("one-slot prefix House tombstone");
    assert_eq!(prefix_house.rng, expected_removal.final_rng);
    assert_eq!(prefix_house.occupants, expected_removal.final_occupants);
    let prefix_residents = prefix
        .economy()
        .house_residents()
        .iter()
        .filter(|resident| resident.parent_house_entity_index == house.entity_index)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(prefix_residents.len(), 1);
    assert_eq!(prefix_residents[0].source_entity_index, prefix_slot);
    assert_eq!(
        (prefix_residents[0].model, prefix_residents[0].action),
        (4, 33)
    );
    assert!(prefix.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::House { source_slot, events, .. }
            if *source_slot == house.entity_index
                && events.iter().any(|event| matches!(
                    event,
                    HouseStepEvent::RemoveFootprint { .. }
                ))
    )));

    session.economy.economy.free_slots.clear();
    session.economy.economy.replacement_cursor_exclusions = (1_u16..1_000).collect();
    let failed_terrain_hash = session.terrain_hash();
    let failed_map = session.economy().map_chains().linked_entity_indices();
    let failed_source_rng = session.economy().source_rng.clone();
    let failed_global_rng = session.economy().global_rng;
    let failed_live_arena = session.economy().live_arena_slots();
    let failed_house_owner = session.economy().source_owner(house.entity_index);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::Error { source_slot, .. }
            if *source_slot == house.entity_index
    )));
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::House { source_slot, events, .. }
                if *source_slot == house.entity_index
                    && events.iter().any(|event| matches!(
                        event,
                        HouseStepEvent::RemoveFootprint { .. }
                    ))
        )
    }));
    let removed_house = session
        .ordinary()
        .houses()
        .iter()
        .find(|candidate| candidate.entity_index == house.entity_index)
        .copied()
        .expect("removed House tombstone");
    assert!(!removed_house.active);
    assert_eq!(removed_house.action, 53);
    assert_eq!(removed_house.phase, failed_house.phase.wrapping_add(1));
    assert_eq!(removed_house.rng, expected_removal.final_rng);
    assert_eq!(removed_house.occupants, expected_removal.final_occupants);
    assert_eq!(session.economy().house_residents().len(), 0);
    assert_eq!(session.canonical_terrain(), &expected_removal_terrain);
    assert_ne!(session.terrain_hash(), failed_terrain_hash);
    assert_eq!(session.economy().source_rng, failed_source_rng);
    assert_eq!(
        session.economy().global_rng,
        level2_next_rng32(failed_global_rng)
    );
    assert_eq!(session.economy().live_arena_slots(), failed_live_arena);
    assert_eq!(
        session.economy().source_owner(house.entity_index),
        failed_house_owner
    );
    assert_eq!(
        session.economy().map_chains().linked_entity_indices(),
        failed_map,
        "DisableEntityDrawing retains the House map link through the dispatch pass"
    );
    let hidden_raw = &session.economy.entities[usize::from(house.entity_index)];
    assert!(hidden_raw.active);
    assert_eq!(hidden_raw.class, 10);
    assert!(hidden_raw.drawing_disabled());
    assert_eq!(
        session.economy.economy.owner_class.get(&house.entity_index),
        Some(&10)
    );
    assert!(!session.economy().free_slots.contains(&house.entity_index));
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(!session.economy().map_chains().is_linked(house.entity_index));
    assert!(session.economy().free_slots.contains(&house.entity_index));
    let reclaimed_raw = &session.economy.entities[usize::from(house.entity_index)];
    assert!(!reclaimed_raw.active);
    assert_eq!(reclaimed_raw.class, 0);
    assert!(
        !session
            .economy
            .economy
            .owner_class
            .contains_key(&house.entity_index)
    );
    assert_eq!(session.economy().source_owner(house.entity_index), 0);
    assert_eq!(
        session.economy().slot_rng.get(&house.entity_index),
        Some(&expected_removal.final_rng),
        "sub_57F20 preserves the reclaimed House RNG in the free-row carrier"
    );
    session.reset();
    assert_eq!(session.terrain_hash(), baseline_terrain_hash);
    assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);

    // Action-53 returns source-ordered resident constructors as part of
    // the same ordinary removal transaction.
    session
        .ordinary_mut()
        .inject_house_damage(house.entity_index, house.life.wrapping_add(1), 111)
        .expect("qualified House damage mailbox");
    session.step_outer_turn(PlayerCommand::default(), 0);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::House { events, .. }
                if events.iter().any(|event| matches!(
                    event,
                    HouseStepEvent::RemoveFootprint { entity_index, .. }
                        if *entity_index == house.entity_index
                ))
        )
    }));
    assert_eq!(session.canonical_terrain(), session.canonical_terrain());
    assert_eq!(session.canonical_terrain(), session.canonical_terrain());
    assert_ne!(session.terrain_hash(), baseline_terrain_hash);
    let removal_residents = session
        .economy()
        .house_residents()
        .iter()
        .filter(|resident| resident.parent_house_entity_index == house.entity_index)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        removal_residents
            .iter()
            .map(|resident| (resident.model, resident.action))
            .collect::<Vec<_>>(),
        vec![(4, 33), (12, 97)]
    );
    let mut expected_resident_slots = removal_residents
        .iter()
        .map(|resident| resident.source_entity_index)
        .collect::<Vec<_>>();
    expected_resident_slots.sort_unstable();
    let dispatched_resident_slots = session
        .dispatch_events()
        .iter()
        .filter_map(|event| match event {
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::HouseResident,
                ..
            } => Some(*source_slot),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(dispatched_resident_slots, expected_resident_slots);
    for resident in &removal_residents {
        assert!(
            session
                .economy()
                .map_chains()
                .is_linked(resident.source_entity_index)
        );
    }
    let replay_dispatch = session.dispatch_events().to_vec();
    let replay_terrain_hash = session.terrain_hash();
    let replay_campaign_hash = session.campaign_state_hash();
    let replay_ordinary_hash = session.ordinary().reset_hash();

    session.reset();
    assert_eq!(session.terrain_hash(), baseline_terrain_hash);
    assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);
    assert_eq!(session.ordinary().reset_hash(), baseline_ordinary_hash);
    assert_eq!(session.economy().free_slots.len(), baseline_free_count);
    assert_eq!(session.economy().live_arena_slots(), baseline_arena_slots);
    assert!(session.economy().house_residents().is_empty());
    assert_eq!(
        (
            session
                .ordinary()
                .houses()
                .iter()
                .filter(|house| house.active)
                .count(),
            session
                .ordinary()
                .villagers()
                .iter()
                .filter(|villager| villager.active)
                .count(),
            session
                .ordinary()
                .dolmens()
                .iter()
                .filter(|dolmen| dolmen.entity().active)
                .count(),
            session.economy().live_arena_slots(),
        ),
        baseline_family_counts
    );

    session
        .ordinary_mut()
        .inject_house_damage(house.entity_index, house.life.wrapping_add(1), 111)
        .expect("replay House damage mailbox");
    session.step_outer_turn(PlayerCommand::default(), 0);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.dispatch_events(), replay_dispatch.as_slice());
    assert_eq!(session.terrain_hash(), replay_terrain_hash);
    assert_eq!(session.campaign_state_hash(), replay_campaign_hash);
    assert_eq!(session.ordinary().reset_hash(), replay_ordinary_hash);
}

#[test]
fn level_two_mana_sphere_retains_birth_particle_until_color_callback() {
    let mut session = level_two_possession_lifecycle_session();
    let id = session
        .economy_mut()
        .spawn_mana_sphere_from_death(
            42,
            Position {
                x: 30_000,
                y: 30_000,
                z: 12_000,
            },
            0,
            0x4321,
            128,
            0,
        )
        .expect("ordinary death sphere");
    let slot = u16::try_from(id).unwrap();
    let read = |session: &Mc2Simulation| {
        *session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == slot)
            .unwrap()
    };
    let born = read(&session);
    assert_eq!(born.collision_yaw, POSSESSION_TARGET_Z_SHIFT);
    assert_eq!(
        (born.mana, born.particle_index, born.collision_pitch),
        (128, 53, 28)
    );
    assert_eq!(
        level2_possession_guidance_target_top(&born).z,
        born.position.z + 28
    );
    let mut peer = born;
    peer.position.x += 40;
    assert!(
        born.overlaps_source_axes(peer.position, 13, 13, 28, 13),
        "birth collision uses28 even though actual mana128 would select13"
    );
    {
        let mut economy = session.economy_mut();
        let sphere = economy
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == slot)
            .unwrap();
        sphere.control_flags |= 0x800;
    }
    assert!(session.economy_mut().step_mana_sphere_slot(slot));
    assert_eq!(
        read(&session).particle_index,
        53,
        "source skip bit defers36920"
    );
    assert!(session.economy_mut().step_mana_sphere_slot(slot));
    let updated = read(&session);
    assert_eq!(
        (
            updated.particle_index,
            updated.collision_yaw,
            updated.collision_pitch,
            updated.collision_roll,
            updated.collision_fov
        ),
        (52, 13, 13, 13, 13)
    );
    // A real sphere's particle callback changed its axis from 28 to 13.
    // Source sub_65580 uses that current value for the flyer's contact Z.
    let owner = session.state.entity_index;
    let projectile_slot = session.economy_mut().spawn_possession_projectile(
        updated.position, owner, 116, 0, 0, -POSSESSION_PROJECTILE_SPEED, updated.position,
    ).expect("source arena admits possession contact probe");
    let projectile = *session.economy().possession_projectiles().iter()
        .find(|row| row.source_entity_index == projectile_slot).unwrap();
    let (contact_slot, contact_position) = level2_first_possession_target(
        session.economy().map_chains(), &[updated], &[], projectile,
    ).expect("updated real sphere remains a contact candidate");
    assert_eq!(contact_slot, slot);
    assert_eq!(contact_position.z, updated.position.z.wrapping_add(updated.collision_yaw));

    let baseline_hash = session.economy().reset_hash();
    {
        let mut economy = session.economy_mut();
        let sphere = economy
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == slot)
            .unwrap();
        sphere.collision_roll = 31;
        sphere.apply_particle_color(52);
        assert_eq!(
            sphere.collision_roll, 31,
            "unchanged particle preserves stored axes"
        );
    }
    assert_ne!(
        session.economy().reset_hash(),
        baseline_hash,
        "retained axes participate in replay state"
    );
    let mut same_particle = read(&session);
    same_particle.mana = 1_025;
    same_particle.apply_particle_color(52);
    assert_eq!(
        (
            same_particle.particle_index,
            same_particle.collision_yaw,
            same_particle.collision_pitch,
            same_particle.collision_roll,
            same_particle.collision_fov
        ),
        (55, 56, 56, 56, 56)
    );
    same_particle.position.z = i16::MAX;
    assert!(
        !same_particle.overlaps_source_axes(
            Position {
                z: i16::MIN,
                ..same_particle.position
            },
            56,
            56,
            56,
            56
        ),
        "source Z arithmetic promotes before addition, unlike wrapped X/Y"
    );
}

#[test]
fn level_two_mana_sphere_new_event_life_fades_and_reclaims_on_source_boundaries() {
    let mut session = level_two_possession_lifecycle_session();
    let sphere = session
        .economy_mut()
        .allocate_authored_model39(348, 344, Position::default())
        .expect("NewEvent-backed model-39 allocation");
    let slot = sphere.source_entity_index;
    assert_eq!((sphere.max_life, sphere.life), (300, 300));
    let peer_slot = session
        .economy_mut()
        .allocate_authored_model39(349, 344, Position::default())
        .expect("overlapping model-39 merge discriminator")
        .source_entity_index;

    {
        let mut economy = session.economy_mut();
        for sphere in economy
            .spheres
            .iter_mut()
            .filter(|sphere| [slot, peer_slot].contains(&sphere.source_entity_index))
        {
            sphere.position = Position::default();
            sphere.velocity_x = 0;
            sphere.velocity_y = 0;
            sphere.vertical_velocity = 0;
            if sphere.source_entity_index == slot {
                sphere.control_flags |= LEVEL2_MANA_SPHERE_TARGET_SUPPRESSION_BIT;
                sphere.life = 13;
            }
        }
    }
    let rng_before = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == slot)
        .expect("fading model-39 carrier")
        .entity_rng;

    assert!(session.economy_mut().step_mana_sphere_slot(slot));
    let at_twelve = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == slot)
        .expect("life-12 model-39 carrier");
    assert_eq!(at_twelve.life, 12);
    assert_ne!(at_twelve.control_flags & 0x0100_0000, 0);
    assert_eq!(at_twelve.entity_rng, rng_before);
    assert!(
        session
            .economy()
            .spheres()
            .iter()
            .any(|sphere| sphere.source_entity_index == peer_slot),
        "source byte[1] bit 0x20 suppresses the current sphere's collision/merge call"
    );

    session
        .economy_mut()
        .spheres
        .iter_mut()
        .find(|sphere| sphere.source_entity_index == slot)
        .expect("life-7 model-39 carrier")
        .life = 7;
    assert!(session.economy_mut().step_mana_sphere_slot(slot));
    let at_six = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == slot)
        .expect("life-6 model-39 carrier");
    assert_eq!(at_six.life, 6);
    assert_ne!(at_six.control_flags & 0x0080_0000, 0);
    assert_eq!(at_six.control_flags & 0x0100_0000, 0);

    session
        .economy_mut()
        .spheres
        .iter_mut()
        .find(|sphere| sphere.source_entity_index == slot)
        .expect("life-1 model-39 carrier")
        .life = 1;
    assert!(session.economy_mut().step_mana_sphere_slot(slot));
    let hidden = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == slot)
        .expect("hidden model-39 remains reserved for this pass");
    assert_eq!(hidden.life, 0);
    assert_ne!(hidden.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
    assert!(!session.economy().free_slots.contains(&slot));

    session
        .economy_mut()
        .reclaim_inactive_slots_at_source_boundary();
    assert!(
        session
            .economy()
            .spheres()
            .iter()
            .all(|sphere| sphere.source_entity_index != slot)
    );
    assert!(session.economy().free_slots.contains(&slot));
}

#[test]
fn level_two_fireball_target_census_includes_only_live_map_linked_dragon_rows() {
    let mut session = level_two_possession_lifecycle_session();
    let terrain = session.terrain.clone();
    let mut position = Position {
        x: 80_u16 << 8,
        y: 80_u16 << 8,
        z: 0,
    };
    position.z = terrain.ground_height(position);
    let root = session
        .economy_mut()
        .allocate_authored_dragon(141, position, 4_000, &terrain)
        .expect("source arena admits one typed Dragon chain");
    let root_slot = root.source_entity_index;
    let part_slots = session
        .economy()
        .authored_dragon_parts()
        .iter()
        .filter(|part| part.entity_id == root_slot)
        .map(|part| part.source_entity_index)
        .collect::<Vec<_>>();
    assert_eq!(part_slots.len(), 16);
    let expected = std::iter::once(root_slot)
        .chain(part_slots.iter().copied())
        .collect::<BTreeSet<_>>();
    let imported = session.economy.entities.to_vec();
    let guidance_targets = session
        .economy()
        .fireball_guidance_targets(&imported)
        .unwrap();
    let collision_targets = session.economy().fireball_collision_targets(&imported);
    let guidance = guidance_targets
        .iter()
        .filter(|target| expected.contains(&target.source_slot))
        .map(|target| target.source_slot)
        .collect::<BTreeSet<_>>();
    let collision = collision_targets
        .iter()
        .filter(|target| expected.contains(&target.source_slot))
        .map(|target| target.source_slot)
        .collect::<BTreeSet<_>>();
    assert_eq!(guidance, expected);
    assert_eq!(collision, expected);
    let root_render = session
        .economy()
        .authored_dragons()
        .iter()
        .find(|render| render.source_entity_index == root_slot)
        .copied()
        .unwrap();
    assert_eq!(
        guidance_targets
            .iter()
            .find(|target| target.source_slot == root_slot)
            .copied(),
        Some(Level2FireballGuidanceTarget {
            source_slot: root_slot,
            source_id: root_slot,
            class: 5,
            model: root.model,
            stage_mode: root_render.stage_var2,
            parent_entity_index: root_render.parent_entity_index,
            top_position: Position {
                z: root.position.z.wrapping_add(root_render.render_yaw),
                ..root.position
            },
        })
    );
    assert_eq!(
        collision_targets
            .iter()
            .find(|target| target.source_slot == root_slot)
            .copied(),
        Some(Level2FireballCollisionTarget {
            source_slot: root_slot,
            source_id: root_slot,
            class: 5,
            model: root.model,
            position: root.position,
            half_width: root_render.render_pitch,
            vertical_offset: root_render.render_yaw,
            half_height: root_render.render_fov,
        })
    );
    let first_part = session
        .economy()
        .authored_dragon_parts()
        .iter()
        .find(|part| part.source_entity_index == part_slots[0])
        .copied()
        .unwrap();
    assert_eq!(
        collision_targets
            .iter()
            .find(|target| target.source_slot == first_part.source_entity_index)
            .copied(),
        Some(Level2FireballCollisionTarget {
            source_slot: first_part.source_entity_index,
            source_id: root_slot,
            class: 5,
            model: first_part.model,
            position: first_part.position,
            half_width: first_part.render_pitch,
            vertical_offset: first_part.render_yaw,
            half_height: first_part.render_fov,
        })
    );

    let inactive_part = part_slots[0];
    let unlinked_part = part_slots[1];
    let hidden_part = part_slots[2];
    {
        let mut economy = session.economy_mut();
        economy
            .authored_dragon_parts
            .iter_mut()
            .find(|part| part.source_entity_index == inactive_part)
            .expect("first Dragon part")
            .active = false;
        economy
            .authored_dragon_parts
            .iter_mut()
            .find(|part| part.source_entity_index == hidden_part)
            .expect("third Dragon part")
            .drawing_enabled = false;
        economy
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: unlinked_part,
                active: false,
                x: position.x,
                y: position.y,
            })
            .expect("second Dragon part unlinks from the source map");
    }
    let guidance = session
        .economy()
        .fireball_guidance_targets(&imported)
        .unwrap()
        .into_iter()
        .map(|target| target.source_slot)
        .collect::<BTreeSet<_>>();
    let collision = session
        .economy()
        .fireball_collision_targets(&imported)
        .into_iter()
        .map(|target| target.source_slot)
        .collect::<BTreeSet<_>>();
    assert!(!guidance.contains(&inactive_part));
    assert!(!guidance.contains(&unlinked_part));
    assert!(!guidance.contains(&hidden_part));
    assert!(!collision.contains(&inactive_part));
    assert!(!collision.contains(&unlinked_part));
    assert!(!collision.contains(&hidden_part));
}

#[test]
fn level_two_mana_sphere_fade_hides_then_reclaims_through_production_scheduler() {
    let mut session = level_two_possession_lifecycle_session();
    let sphere = session
        .economy_mut()
        .allocate_authored_model39(348, 344, Position::default())
        .expect("scheduled model-39 allocation");
    let slot = sphere.source_entity_index;
    let generation = session.economy().slot_generation(slot);
    {
        let mut economy = session.economy_mut();
        let sphere = economy
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == slot)
            .expect("scheduled model-39 carrier");
        sphere.control_flags |= LEVEL2_MANA_SPHERE_TARGET_SUPPRESSION_BIT;
        sphere.life = 1;
        sphere.vertical_velocity = 0;
    }

    session.step_outer_turn(PlayerCommand::default(), 0);
    let hidden = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == slot)
        .expect("callback-hidden sphere remains reserved until the next source boundary");
    assert_eq!(hidden.life, 0);
    assert_ne!(hidden.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
    assert_eq!(session.economy().slot_generation(slot), generation);
    assert!(!session.economy().free_slots.contains(&slot));
    assert!(
        session
            .cf10_production_schedule_receipts()
            .iter()
            .any(|receipt| matches!(
                receipt,
                crate::campaign::schedule::ScheduleReceipt::CallbackCompleted {
                    slot: visited,
                    ..
                } if *visited == slot
            ))
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .spheres()
            .iter()
            .all(|sphere| sphere.source_entity_index != slot)
    );
    assert!(session.economy().free_slots.contains(&slot));
}
#[test]
fn level_two_free_arena_rebuild_keeps_hidden_castle_flyer_reserved_until_reclaim() {
    let mut session = level_two_possession_lifecycle_session();
    let reserved_slot = session
        .economy()
        .free_slots
        .last()
        .copied()
        .expect("qualified reset exposes an ordinary free slot");
    session.economy_mut().free_slots = vec![reserved_slot];
    let position = session.state.position;
    let owner = session.state.entity_index;
    let flyer_slot = session
        .economy_mut()
        .spawn_castle_projectile(position, owner, 0, 0, 0, 0)
        .expect("ordinary Castle flyer allocation");
    assert_eq!(flyer_slot, reserved_slot);
    session
        .economy_mut()
        .disable_castle_projectile(flyer_slot, false);

    session.economy_mut().rebuild_free_arena(&[reserved_slot]);
    assert!(
        !session.economy().free_slots.contains(&reserved_slot),
        "sub_49F90 must not publish the hidden-but-reserved fixed Castle row as class zero"
    );

    session
        .economy_mut()
        .reclaim_hidden_castle_rows_at_source_boundary();
    assert!(
        session.economy().free_slots.contains(&reserved_slot),
        "the following-pass hidden-row sweep owns the Castle flyer release"
    );
}
