use super::*;

#[test]
fn source_visual_transitions_publish_all_current_collision_axes() {
    use crate::native_world::{area_damage, source_target};

    let mut session = crate::native_world::tests::level_two_campaign_session();
    let castle = session.economy_mut().build_castle(
        Position { x: 0x2000, y: 0x3000, z: 0 }, 111,
    ).unwrap();
    session.economy.castle.as_mut().unwrap().level = 3;
    session.economy_mut().maintain_castle_defenders(castle);
    let slot = session.economy.castle().unwrap().auxiliary_slots[0];
    let shape = |session: &crate::native_world::Mc2Simulation| {
        let actor = area_damage::read_arena_actor(session.source_arena(), slot).unwrap();
        let view = source_target::read(session.source_arena(), source_target::Context::default(), slot)
            .unwrap().unwrap();
        assert_eq!(view.collision, Some(actor.shape), "both live consumers read the same axes");
        (actor.shape.half_width, actor.shape.half_depth,
            actor.shape.vertical_offset, actor.shape.half_height)
    };
    assert_eq!(shape(&session), (128, 128, 125, 128),
        "4C1E0 applies particle0 then the 128/128 source shift");
    for (seed, expected_visual, extent) in [(3, 206, 120), (0, 1, 125)] {
        let row = session.economy.castle_defenders.iter_mut()
            .find(|row| row.source_entity_index == slot).unwrap();
        row.entity_rng = seed;
        aim(row);
        assert_eq!(row.visual_index, expected_visual);
        assert_eq!(shape(&session), (0, 0, extent, extent),
            "24100's SetEntityIndexAndRot replaces all four axes");
    }
    let row = session.economy.castle_defenders.iter_mut()
        .find(|row| row.source_entity_index == slot).unwrap();
    idle(row);
    assert_eq!(shape(&session), (0, 0, 125, 125),
        "24150 resets the particle0 axes without reapplying the constructor shift");
    let hash = session.economy.reset_hash();
    let mut changed = session.clone();
    changed.economy.castle_defenders.iter_mut()
        .find(|row| row.source_entity_index == slot).unwrap().collision_vertical_offset += 1;
    assert_ne!(changed.economy.reset_hash(), hash,
        "the independently imported vertical axis participates in deterministic state");
}

#[test]
fn point_routes_react_to_linked_damage_only_for_an_external_attacker() {
    struct Routes(StageDescriptor);
    impl StageRuntime for Routes {
        fn descriptor(&self, _: u8) -> Option<StageDescriptor> { Some(self.0) }
        fn bind(&mut self, _: u8) -> Option<StageDescriptor> { Some(self.0) }
        fn rearm_model_relations(&mut self) {}
    }
    let terrain = crate::native_world::tests::level_two_campaign_session().canonical_terrain().clone();
    let position = Position { x: 20_000, y: 20_000, z: 2_000 };
    let route = StageDescriptor { kind: 1, flags: 0, next: 0,
        point: Position { x: 24_000, ..position }, actor: 0, model: 0, scalar: 0 };
    for mode in [1, 2] {
        for same_family in [false, true] {
            let mut counter = 0;
            let mut actor = CastleDefenderState::construct(50, 0, 1, position,
                &mut counter, DefenderOrigin::AuthoredRecord(1));
            actor.action = 127;
            actor.stage.index = 1;
            actor.stage.mode = mode;
            actor.chain_next_slot = 8;
            let child = DefenderTarget { slot: 8, class: 5, model: 15,
                life: 900, attacker: 9, position, ..DefenderTarget::default() };
            let attacker = DefenderTarget { slot: 9, class: if same_family { 5 } else { 3 },
                model: if same_family { 15 } else { 0 }, life: 1000, position,
                ..DefenderTarget::default() };
            let targets = [child, attacker];
            let mut routes = Routes(route);
            let mut context = DefenderContext { terrain: &terrain, targets: &targets,
                class3: &[], model15: &[], class5: &[], stage_runtime: Some(&mut routes) };
            step(&mut actor, &mut context).unwrap();
            assert_eq!((actor.life, actor.damage_attacker_entity_index), (900, 9));
            if same_family {
                assert_eq!((actor.action, actor.stage.mode), (127, mode));
            } else {
                assert_eq!((actor.action, actor.stage.mode, actor.target_sphere_slot), (122, 10, 9));
            }
            assert_ne!(actor.position.z, position.z,
                "source hit branch settles even when the attacker is the same family");
        }
    }
}

#[test]
fn guard_scan_wraps_y_but_not_x_at_the_map_seam() {
    let terrain = crate::native_world::tests::level_two_campaign_session().canonical_terrain().clone();
    let mut counter = 0;
    let mut actor = CastleDefenderState::construct(50, 0, 1,
        Position { x: 2000, y: 65000, z: 0 }, &mut counter, DefenderOrigin::AuthoredRecord(1));
    let mut target = DefenderTarget { slot: 8, class: 3, model: 0, life: 1000,
        position: Position { x: 2000, y: 1000, z: 0 }, ..DefenderTarget::default() };
    actor.yaw = heading_for_positions(actor.position, target.position) as u16;
    let targets = [target];
    let context = DefenderContext { terrain: &terrain, targets: &targets,
        class3: &[8], model15: &[], class5: &[], stage_runtime: None };
    assert_eq!(guard_target(&actor, &context).map(|row| row.slot), Some(8),
        "1DBF0 stores the Y delta in a signed 16-bit intermediate");
    actor.position = Position { x: 65000, y: 2000, z: 0 };
    target.position = Position { x: 1000, y: 2000, z: 0 };
    actor.yaw = heading_for_positions(actor.position, target.position) as u16;
    let targets = [target];
    let context = DefenderContext { terrain: &terrain, targets: &targets,
        class3: &[8], model15: &[], class5: &[], stage_runtime: None };
    assert_eq!(guard_target(&actor, &context), None,
        "the corresponding X delta remains promoted, outside the source range");
}

#[test]
fn roam_probe_checks_the_full_forward_collision_extent() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let mut terrain = session.canonical_terrain().clone();
    terrain.map_type = MapType::Day;
    terrain.planes.terrain_types.fill(21);
    let start = Position { x: 0x2080, y: 0x2080, z: 0 };
    let mut counter = 0;
    let mut actor = CastleDefenderState::construct(50, 0, 1, start, &mut counter, DefenderOrigin::AuthoredRecord(1));
    actor.yaw = 512;
    actor.shift_rot_x = 256;
    let mut next = start;
    crate::move_entity(&mut next, actor.yaw as i16, 0, 256);
    let index = crate::native_world::world_types::terrain_index((next.x >> 8) as u8, (next.y >> 8) as u8);
    terrain.planes.terrain_types[index] = 0;
    assert!(!probe_blocked(&terrain, start, actor.fov));
    assert!(roam_probe_blocked(&actor, &terrain, start));
    actor.shift_rot_x = 128;
    assert!(!roam_probe_blocked(&actor, &terrain, start));
}

#[test]
fn castle_allocation_failure_keeps_zero_throttle_and_retries_next_callback() {
    let mut session = crate::native_world::tests::level_two_campaign_session();
    let castle = session.economy_mut().build_castle(
        Position { x: 0x2000, y: 0x3000, z: 0 }, 111,
    ).expect("actual Castle constructor");
    // First establish the source's stage-one class3 holder normally. The
    // stage-three entry is the focused allocation-pressure prerequisite.
    session.economy.castle.as_mut().unwrap().level = 1;
    session.economy_mut().maintain_castle_defenders(castle);
    let holder = session.economy.castle().unwrap().defender_slots[0];
    let holder_handle = session.source_arena().handle(holder).unwrap();
    session.economy.castle.as_mut().unwrap().level = 3;
    let phase_before = session.economy.castle_auxiliary_constructor_phase;
    let retry_slot = session.economy.exhaust_new_event_capacity_for_test()
        .expect("the real allocator had an available free slot");
    session.economy.replacement_cursor_exclusions = (1_u16..1000).collect();
    let generation_before = session.economy.slot_generation(retry_slot);
    for _ in 0..2 {
        session.economy_mut().maintain_castle_defenders(castle);
        let root = session.economy.castle().unwrap();
        assert_eq!(root.auxiliary_count, 4);
        assert_eq!(root.auxiliary_slots[..4], [0; 4]);
        assert_eq!(root.auxiliary_cooldown, 0,
            "failed NewEvent attempts do not start the sixteen-visit throttle");
        assert_eq!(session.economy.castle_auxiliary_constructor_phase, phase_before,
            "failed allocation must not consume the model constructor clock");
        assert_eq!(session.source_arena().handle(holder), Some(holder_handle),
            "the earlier class3 holder remains the same source generation");
    }
    assert!(session.economy.restore_new_event_slot_for_test(retry_slot));
    session.economy_mut().maintain_castle_defenders(castle);
    let root = session.economy.castle().unwrap();
    assert_eq!(root.auxiliary_slots[0], retry_slot);
    assert_eq!(root.auxiliary_cooldown, 16);
    assert_eq!(session.economy.slot_generation(retry_slot), generation_before.wrapping_add(1));
    let child = session.economy.castle_defenders().iter()
        .find(|row| row.active && row.source_entity_index == retry_slot).unwrap();
    assert_eq!(child.source_byte_3e, phase_before);
    assert_eq!(session.economy.castle_auxiliary_constructor_phase, phase_before.wrapping_add(1));
    assert_eq!(session.source_arena().handle(holder), Some(holder_handle));
}

#[test]
fn ordinary_hostile_castle_child_receives_public_fireball_and_is_replaced() {
    use crate::campaign::spec::CampaignSelection;
    use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent};

    fn assert_no_dispatch_errors(
        session: &crate::native_world::Mc2Simulation, phase: &str, root_slot: u16,
        child_slot: Option<u16>,
    ) {
        let errors = session.dispatch_events().iter()
            .filter(|event| matches!(event, Mc2DispatchEvent::Error { .. })).collect::<Vec<_>>();
        let live = &session.session.state.live;
        let context = crate::native_world::source_target::Context {
            spells: Some(&live.shared_spells), map_type: Some(live.terrain.map_type()),
            player: Some((live.core.state.entity_index, live.core.state.position, live.player_life)),
            player_orientation: Some((live.core.state.yaw, live.core.state.roll_accumulator)),
            particles: Some(&session.session.definition.presentation.particles),
            ..crate::native_world::source_target::Context::from_ordinary(&live.goats, &live.ordinary)
        };
        assert!(errors.is_empty(),
            "{phase}: dispatch errors={errors:?}, root_handle={:?}, root={:?}, child_handle={:?}, child={:?}, player={:?}, fireballs={:?}, target30_handle={:?}, target30_cursor={:?}, target30_view={:?}, target30_raw={:?}",
            session.source_arena().handle(root_slot), session.economy().castle_at(root_slot),
            child_slot.and_then(|slot| session.source_arena().handle(slot)),
            child_slot.and_then(|slot| session.economy().castle_defenders().iter()
                .find(|row| row.source_entity_index == slot)),
            session.shared_spells().player_entity(), session.economy().fireball_projectiles(),
            live.economy.handle(30), session.economy().live_arena_cursor_row(30),
            crate::native_world::source_target::read(&live.economy, context, 30), session.entities().get(30));
    }
    fn observe_replaced_target30(session: &crate::native_world::Mc2Simulation, observed: &mut bool) {
        if *observed { return; }
        let live = &session.session.state.live;
        let context = crate::native_world::source_target::Context {
            spells: Some(&live.shared_spells), map_type: Some(live.terrain.map_type()),
            player: Some((live.core.state.entity_index, live.core.state.position, live.player_life)),
            player_orientation: Some((live.core.state.yaw, live.core.state.roll_accumulator)),
            particles: Some(&session.session.definition.presentation.particles),
            ..crate::native_world::source_target::Context::from_ordinary(&live.goats, &live.ordinary)
        };
        let cursor = session.economy().live_arena_cursor_row(30);
        let view = crate::native_world::source_target::read(&live.economy, context, 30);
        if cursor.is_some() && view.as_ref().ok().and_then(|row| *row)
            .is_none_or(|row| (row.class, row.model) != (5, 26))
        {
            *observed = true;
            eprintln!("first changed target30 before next callback: handle={:?}, cursor={cursor:?}, view={view:?}, raw={:?}, cymmerian38_view={:?}, cymmerian38_raw={:?}",
                live.economy.handle(30), session.entities().get(30),
                crate::native_world::source_target::read(&live.economy, context, 38), session.entities().get(38));
        }
    }

    let package = std::env::var_os("MC_DEFENDER_HOSTILE_PACKAGE").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/all-spells-20261104-integration/current-owner-package/level18"),
        std::path::PathBuf::from,
    );
    let construction = crate::house::decode_house_construction_inputs(
        &std::fs::read(package.join("feature-construction-inputs.bin"))
            .expect("qualified Level 18 construction input"),
    ).expect("decode the actual Level 18 BUILD records");
    let mut session = crate::campaign::CampaignSession::open(
        package, CampaignSelection::ordinary(18).unwrap(),
    ).expect("ordinary Level 18 owner package").simulation().clone();
    for stage in 1..=5 {
        let expected = &construction.get(&stage)
            .expect("the qualified owner input contains this Castle BUILD stage").footprint;
        assert_eq!(session.economy().castle_stage_footprints.get(&stage), Some(expected),
            "Castle BUILD stage {stage} must retain the exact owner cells and dimensions");
    }
    for stage in [4, 5] {
        let footprint = &construction[&stage].footprint;
        assert_eq!((footprint.width, footprint.height), (35, 35));
    }
    session.apply_carried_spell_families(&[0], Some(0))
        .expect("declared opening Fireball carry materializes its real holder");
    let player = session.state.entity_index;
    // Declared encounter prerequisite: a living player at full original
    // health with base capacity and mana to pay for repeated Fireballs. The
    // optional low-resource diagnostic exercises the same unmodified actor path.
    // Damage, mana expenditure and recovery remain source-driven thereafter.
    let starting_mana = std::env::var("MC_DEFENDER_STARTING_MANA")
        .map(|value| value.parse::<i32>().expect("valid starting mana"))
        .unwrap_or(50_000);
    assert!(starting_mana >= 0);
    if starting_mana != 100 {
        let live = &mut session.session.state.live;
        let health = live.shared_spells.player_entity.max_life;
        live.economy.economy.owner_base_max_mana.insert(player, starting_mana);
        live.economy.economy.rebuild_source_owner_max_mana_projection();
        let capacity = live.economy.economy.owner_max_mana_capacity(player);
        assert!(capacity >= starting_mana);
        live.player_life = health;
        live.shared_spells.set_player_entity_life(health);
        live.shared_spells.set_local_player_mana_for_test(starting_mana);
        live.shared_spells.player_entity.max_mana = capacity;
        live.shared_spells.player_entity.record[144..148].copy_from_slice(&capacity.to_le_bytes());
        for entity in &mut live.shared_spells.player_entities {
            if entity.index == player {
                entity.max_mana = capacity;
                entity.record[144..148].copy_from_slice(&capacity.to_le_bytes());
            }
        }
        let entity = &mut live.economy.entities[usize::from(player)];
        entity.set_life(health);
        entity.set_mana(starting_mana);
        entity.max_mana = capacity;
        entity.record[144..148].copy_from_slice(&capacity.to_le_bytes());
    }
    if starting_mana != 100 {
        if let Some(fire) = session.context.spell_fire.as_mut() { fire.caster.mana = starting_mana; }
    }
    // The actual owner18 reset has rival184's stage-five Castle209 with its
    // in-flight model42 BUILD child. Its construction and 5FF50 callbacks
    // run normally; no ownership or stage is assigned by the test.
    let root = session.economy().castle_for_owner(184)
        .expect("the actual rival184 Castle");
    assert_eq!((root.source_entity_index, root.level), (209, 5));
    let authored_root = session.source_arena().handle(root.source_entity_index).unwrap();
    let mut protection_births = Vec::new();
    let mut target30_observed = false;
    let footprints = (1..=7).map(|stage|
        (stage, session.economy().has_castle_stage_footprint(stage))).collect::<Vec<_>>();
    let mut construction_visits = 0;
    for _ in 0..256 {
        if session.session.state.live.player_protection_ticks == 0 { break; }
        if starting_mana == 100 { observe_replaced_target30(&session, &mut target30_observed); }
        session.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert_no_dispatch_errors(&session, "protection", authored_root.source_slot, None);
        construction_visits += session.dispatch_events().iter().filter(|event| matches!(event,
            Mc2DispatchEvent::DynamicArena { source_slot: 210, .. })).count();
        for row in session.economy().castle_defenders().iter().filter(|row|
            row.active && row.castle_owner == Some(authored_root))
        {
            if let Some(handle) = session.source_arena().handle(row.source_entity_index)
                && !protection_births.contains(&handle) { protection_births.push(handle); }
        }
    }
    assert_eq!(session.session.state.live.player_protection_ticks, 0,
        "ordinary player visits must expire the original protection");
    let mut produced = None;
    let mut production_trace = Vec::new();
    for turn in 0..512 {
        produced = session.economy().castle_defenders().iter().find(|row|
            row.active && row.id_entity_index == 184 && row.castle_owner == Some(authored_root)
                && row.life > 0 && row.drawing_enabled()).copied();
        if produced.is_some() { break; }
        if turn % 64 == 0 {
            let root = session.economy().castle_at(authored_root.source_slot).map(|root|
                (root.action, root.level, root.build_state, root.source_word_2e, root.phase,
                    root.auxiliary_cooldown, root.auxiliary_slots[..4].to_vec()));
            let children = session.economy().castle_children().iter().filter(|row|
                row.active && row.parent_entity_index == authored_root.source_slot)
                .map(|row| (row.source_entity_index, row.model, row.action, row.source_dword_10,
                    row.control_flags, row.cave_collision_cooldown, row.source_byte_3e)).collect::<Vec<_>>();
            let defenders = session.economy().castle_defenders().iter().filter(|row|
                row.id_entity_index == 184).map(|row|
                (row.source_entity_index, row.castle_owner, row.active, row.life, row.action,
                    row.control_flags)).collect::<Vec<_>>();
            production_trace.push((turn, root, children, defenders,
                session.economy().free_slot_count()));
        }
        if starting_mana == 100 { observe_replaced_target30(&session, &mut target30_observed); }
        session.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert_no_dispatch_errors(&session, "construction", authored_root.source_slot, None);
        construction_visits += session.dispatch_events().iter().filter(|event| matches!(event,
            Mc2DispatchEvent::DynamicArena { source_slot: 210, .. })).count();
    }
    let child = produced.unwrap_or_else(|| panic!(
        "an ordinary hostile Castle must actually produce its model15 child: root={authored_root:?}, footprints={footprints:?}, construction_visits={construction_visits}, protection_births={protection_births:?}, trace={production_trace:?}"));
    let root_handle = child.castle_owner.unwrap();
    let child_slot = child.source_entity_index;
    let child_handle = session.source_arena().handle(child_slot).unwrap();
    let root = session.economy().castle_at(root_handle.source_slot).unwrap();
    let child_index = root.auxiliary_slots.iter().position(|slot| *slot == child_slot)
        .expect("the actual hostile Castle owns its produced child relation");
    assert_eq!((child.max_life, child.mana), (1000, 0));
    let initial_child_life = child.life;
    let event_start = session.economy().events().len();
    // One declared approach places the real player in the produced child's
    // forward scan. Neutral input then leaves both real actors to move, aim,
    // shoot and collide under their own source schedules.
    let mut encounter = child.position;
    crate::move_entity(&mut encounter, child.yaw as i16, 0, 768);
    encounter.z = session.canonical_terrain().ground_height(encounter).wrapping_add(256);
    session.set_level2_entity_position(player, encounter).unwrap();
    let player_life = session.canonical_player_life_for_test();
    let mut aimed = false;
    let mut arrows = Vec::new();
    let mut arrow_audio = false;
    let mut player_hit = false;
    let mut trace = Vec::new();
    for _ in 0..256 {
        if let Some(row) = session.economy().castle_defenders().iter()
            .find(|row| row.active && row.source_entity_index == child_slot)
        {
            aimed |= row.action == 122 && row.target_sphere_slot == player;
            if trace.len() < 40 {
                trace.push((row.action, row.source_byte_3e, row.position, row.yaw,
                    session.state.position, session.canonical_player_life_for_test()));
            }
        }
        if starting_mana == 100 { observe_replaced_target30(&session, &mut target30_observed); }
        session.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert_no_dispatch_errors(&session, "defender attack", root_handle.source_slot, Some(child_slot));
        for row in session.economy().authored_class5_archer_children().iter().filter(|row|
            row.source_class == 9 && row.model == 13 && row.entity_id == child.id_entity_index
                && row.word_0x96 == player)
        {
            if let Some(handle) = session.source_arena().handle(row.source_entity_index)
                && !arrows.contains(&handle) { arrows.push(handle); }
        }
        arrow_audio |= session.effect_journal().records().iter().any(|record|
            matches!(record.effect, crate::campaign::journal::CampaignEffect::Flight(
                crate::FlightEvent::PrepareSound { audio_site: crate::MissionAudioSite::ArcherArrow,
                    entity_index, player_index: -1, sound_id: 33 | 34 }
            ) if arrows.iter().any(|handle| handle.source_slot == entity_index))
                && record.audio.is_some_and(|audio| audio.play_type == 3));
        player_hit |= session.economy().events()[event_start..].iter().any(|event| matches!(event,
            Level2EconomyEvent::DamageApplied { target_entity_index, attacker_entity_index, damage, .. }
                if *target_entity_index == player && *attacker_entity_index == child.id_entity_index && *damage > 0));
        if aimed && !arrows.is_empty() && arrow_audio && player_hit { break; }
    }
    assert!(aimed && !arrows.is_empty() && arrow_audio && player_hit,
        "the real Castle child must aim, emit a model13 arrow with actual Flight audio and damage the player: aimed={aimed} arrows={arrows:?} audio={arrow_audio} hit={player_hit} life={player_life}->{}, child={:?}, trace={trace:?}",
        session.canonical_player_life_for_test(), session.economy().castle_defenders().iter()
            .find(|row| row.source_entity_index == child_slot));
    assert!(session.canonical_player_life_for_test() < player_life);
    assert_eq!(session.source_arena().handle(child_slot), Some(child_handle));
    let holder = session.shared_spells().local_player().enabled_spell_holders[0];
    let fire_ui = crate::SOURCE_SPELL_INDEX_MAP.iter().position(|family| *family == 0).unwrap() as i8;
    let mut attempts = 0;
    let mut paid_casts = 0;
    let mut shot_trace = Vec::new();
    let mut player_died = false;
    let mut flyer = false;
    let mut damaged = false;
    let mut lethal = false;
    let mut reclaimed = false;
    let mut relation_cleared = false;
    let mut replacement = None;
    for turn in 0..1024 {
        let current = session.economy().castle_defenders().iter().find(|row|
            row.active && row.source_entity_index == child_slot).copied();
        let still_original = session.source_arena().handle(child_slot) == Some(child_handle);
        let ready = session.shared_spells().pickup(holder).is_some_and(|row|
            row.active_value == 0 && session.shared_spells().player_entity().mana >= row.max_mana);
        let mut command = crate::PlayerCommand::default();
        if still_original && ready && let Some(current) = current.filter(|row| row.life >= 0) {
            // Controlled player travel/aim per shot; child movement, source
            // phase, projectile, damage mailbox and death are all untouched.
            let (position, yaw, pitch) = if starting_mana == 100 {
                let position = Position { x: current.position.x.wrapping_add(256),
                    y: current.position.y.wrapping_add(384),
                    z: current.position.z.wrapping_sub(session.state.entity_fov) };
                let mut launch = position;
                launch.z = launch.z.wrapping_add(session.state.entity_fov);
                let (yaw, _) = crate::level1_mission::projectile_guidance_angles(launch, current.position);
                let flags = session.shared_spells().player_entity().control_flags;
                let hand_yaw = if flags & 0x0100 != 0 { Some(yaw.wrapping_sub(512)) }
                    else if flags & 0x0200 != 0 { Some(yaw.wrapping_add(512)) } else { None };
                if let Some(hand_yaw) = hand_yaw {
                    let mut hand = position;
                    crate::move_entity(&mut hand, hand_yaw, 0, 256);
                    if session.canonical_terrain().ground_height(hand) <= hand.z {
                        launch.x = hand.x; launch.y = hand.y;
                    }
                }
                let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(launch, current.position);
                (position, yaw, pitch)
            } else {
                // Solve the body position from a fixed final shot yaw and the
                // exact source hand offset. The first 384-unit flight visit is
                // aimed at the current defender, from outside its Castle.
                let yaw = 1024_i16;
                let mut launch = current.position;
                crate::move_entity(&mut launch, yaw.wrapping_add(1024), 0, 384);
                let mut position = launch;
                let parent = session.shared_spells().player_entity();
                let parent_yaw = yaw.wrapping_sub(session.shared_spells().beyond_sight_auxiliary().next_entity) & 0x07ff;
                let hand = if parent.control_flags & 0x0100 != 0 { Some(parent_yaw.wrapping_sub(512)) }
                    else if parent.control_flags & 0x0200 != 0 { Some(parent_yaw.wrapping_add(512)) } else { None };
                if let Some(hand) = hand { crate::move_entity(&mut position, hand.wrapping_add(1024), 0, 256); }
                let centre_z = current.position.z.wrapping_add(current.collision_vertical_offset);
                position.z = centre_z.wrapping_sub(parent.array_fov())
                    .max(session.canonical_terrain().ground_height(position).wrapping_add(256))
                    .max(session.canonical_terrain().ground_height(launch).wrapping_add(256));
                launch.z = position.z.wrapping_add(parent.array_fov());
                let target = Position { z: centre_z, ..current.position };
                let (_, pitch) = crate::level1_mission::projectile_guidance_angles(launch, target);
                (position, yaw, pitch)
            };
            session.set_level2_entity_position(player, position).unwrap();
            session.set_level2_player_aim_for_test(yaw, pitch);
            command = crate::PlayerCommand { opcode: 6, argument_1: fire_ui, flags: 0x40,
                ..crate::PlayerCommand::default() };
            attempts += 1;
        }
        let mana_before = session.shared_spells().player_entity().mana;
        let cost = session.shared_spells().pickup(holder).unwrap().max_mana;
        if starting_mana == 100 { observe_replaced_target30(&session, &mut target30_observed); }
        session.step_outer_turn(command, 0);
        assert_no_dispatch_errors(&session, "player retaliation", root_handle.source_slot, Some(child_slot));
        let mana_after = session.shared_spells().player_entity().mana;
        if mana_before.saturating_sub(mana_after) >= cost { paid_casts += 1; }
        if paid_casts > 0 && shot_trace.len() < 24 {
            let projectiles = session.economy().fireball_projectiles().iter()
                .filter(|row| row.owner_entity_index == player)
                .map(|row| (session.source_arena().handle(row.source_entity_index), row.position,
                    row.yaw, row.pitch, row.actual_speed, row.life, row.phase, row.active,
                    crate::native_world::area_damage::read_arena_actor(session.source_arena(), row.source_entity_index)))
                .collect::<Vec<_>>();
            shot_trace.push((turn, attempts, paid_casts, mana_before, mana_after,
                session.state.position, session.state.yaw, session.state.effective_pitch,
                session.canonical_player_life_for_test(),
                crate::native_world::area_damage::read_arena_actor(session.source_arena(), child_slot),
                projectiles));
        }
        flyer |= session.economy().fireball_projectiles().iter().any(|row| row.owner_entity_index == player);
        if session.source_arena().handle(child_slot) == Some(child_handle) {
            if let Some(row) = session.economy().castle_defenders().iter()
                .find(|row| row.active && row.source_entity_index == child_slot)
            {
                damaged |= row.life < initial_child_life && row.damage_attacker_entity_index == player;
                lethal |= row.life < 0 && row.killer_entity_index == player;
            }
        } else {
            reclaimed = true;
        }
        let root = session.economy().castle_at(root_handle.source_slot)
            .expect("the original hostile Castle remains live");
        assert_eq!(session.source_arena().handle(root_handle.source_slot), Some(root_handle));
        let relation = root.auxiliary_slots[child_index];
        relation_cleared |= relation == 0;
        if reclaimed && relation != 0
            && let Some(row) = session.economy().castle_defenders().iter().find(|row|
                row.active && row.source_entity_index == relation && row.castle_owner == Some(root_handle))
            && let Some(handle) = session.source_arena().handle(row.source_entity_index)
            && handle != child_handle
        {
            replacement = Some(handle);
        }
        if paid_casts > 0 && flyer && damaged && lethal && reclaimed && relation_cleared && replacement.is_some() { break; }
        if session.canonical_player_life_for_test() < 0 {
            player_died = true;
            if starting_mana != 100 { break; }
        }
    }
    assert!(paid_casts > 0 && flyer && damaged && lethal && reclaimed && relation_cleared && replacement.is_some(),
        "public Fireball must kill the real hostile Castle child and its owner must replace it: attempts={attempts} paid_casts={paid_casts} player_died={player_died} flyer={flyer} damaged={damaged} lethal={lethal} reclaimed={reclaimed} cleared={relation_cleared} replacement={replacement:?}, shot_trace={shot_trace:?}, child={:?}, root={:?}, player={:?}",
        session.economy().castle_defenders().iter().find(|row| row.source_entity_index == child_slot),
        session.economy().castle_at(root_handle.source_slot), session.shared_spells().player_entity());
    assert!(!session.economy().events()[event_start..].iter().any(|event| matches!(event,
        Level2EconomyEvent::ManaSphereSpawned { source_entity_index, .. } if *source_entity_index == child_slot)),
        "the ordinary zero-mana defender must not manufacture a death sphere");
}

fn expiring_defender(mode: u8) -> (crate::native_world::Mc2Simulation, u16) {
    let mut session = crate::native_world::tests::level_two_campaign_session();
    let castle = session
        .economy_mut()
        .build_castle(
            Position {
                x: 0x2000,
                y: 0x3000,
                z: 0,
            },
            111,
        )
        .expect("actual Castle constructor");
    session.economy.castle.as_mut().expect("Castle").level = 3;
    session.economy_mut().maintain_castle_defenders(castle);
    let slot = session.economy.castle().expect("Castle").auxiliary_slots[0];
    session.economy.exhaust_new_event_capacity_for_test();
    let actor = session
        .economy
        .castle_defenders
        .iter_mut()
        .find(|row| row.source_entity_index == slot)
        .expect("ordinary model15 product");
    actor.action = 127;
    actor.stage.mode = mode;
    actor.stage.parent = 0;
    actor.stage.remaining = 99;
    actor.source_byte_3e = 3;
    actor.control_flags |= crate::native_world::SOURCE_REPLACEABLE_FLAG;
    // UpdateEntities/49F90 freezes replacement addresses before callbacks.
    // The pressure helper above only exhausts the ordinary economy free stack.
    for source in &mut session.economy.entities {
        source.set_control_flags(
            source.control_flags & !crate::native_world::SOURCE_REPLACEABLE_FLAG,
        );
    }
    session.economy.rebuild_source_free_arena();
    assert_eq!(session.economy.free_slot_count(), 0);
    (session, slot)
}

fn expire(session: &mut crate::native_world::Mc2Simulation, slot: u16) {
    let particles = session.session.definition.presentation.particles.clone();
    let membership = session
        .economy
        .castle_auxiliary_membership(&session.economy.entities);
    let live = &mut session.session.state.live;
    let result = mc2::step_source_runtime(
        &mut live.economy,
        &mut live.core.runtime,
        &mut live.terrain,
        &particles,
        slot,
        &membership,
    )
    .expect("enabled scripted callback");
    assert!(result.effects.expire);
    assert!(result.map_synced, "epilogue follows the current occupant");
}

#[test]
fn summoned_expiry_self_replacement_hides_raw73_and_advances_its_own_clock() {
    let (mut session, slot) = expiring_defender(13);
    let sprite = session
        .campaign_entity_presentations(0)
        .expect("ordinary owner particle resolver")
        .into_iter()
        .find(|row| row.entity_index == slot)
        .expect("the current defender carrier produces its sprite descriptor");
    assert_eq!(sprite.source_id, 111);
    assert_eq!(sprite.render_parameter_index, 0);
    assert_eq!(
        sprite.position,
        session.economy.castle_defenders()[0].position
    );
    let generation = session.economy.slot_generation(slot);
    expire(&mut session, slot);
    assert_eq!(
        session.economy.slot_generation(slot),
        generation.wrapping_add(1)
    );
    assert!(
        !session
            .economy
            .castle_defenders()
            .iter()
            .any(|row| row.source_entity_index == slot)
    );
    let effect = &session.economy.entities[usize::from(slot)];
    assert_eq!((effect.class, effect.model, effect.action), (10, 73, 80));
    assert_eq!(effect.position.x, 0);
    assert_eq!(effect.position.y, 0);
    assert_eq!(
        effect.position.z,
        session
            .canonical_terrain()
            .ground_height(Position::default())
    );
    assert!(effect.drawing_disabled());
    assert_eq!(effect.phase, (slot as u8).wrapping_add(1));
    assert!(
        session
            .campaign_entity_presentations(0)
            .expect("hidden replacement presentation")
            .iter()
            .all(|row| row.entity_index != slot)
    );
    assert!(
        session.economy.model17_spawned_children().is_empty(),
        "second allocation failed without a substitute success"
    );
    assert!(!session.economy.free_slots.contains(&slot));
    session.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(
        session.economy.free_slots.contains(&slot),
        "following prologue reclaims the hidden replacement"
    );
}

#[test]
fn temporary_expiry_self_replacement_hides_real_impact_at_zero_position() {
    let (mut session, slot) = expiring_defender(16);
    expire(&mut session, slot);
    let effect = session
        .economy
        .model17_spawned_children()
        .iter()
        .find(|row| row.source_entity_index == slot)
        .expect("real NewAdd0A00 product");
    assert_eq!(
        (effect.source_class, effect.model, effect.action),
        (10, 0, 0)
    );
    assert_eq!(effect.position, Position::default());
    assert_eq!(effect.parent_entity_id, slot);
    assert_ne!(effect.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
    assert_eq!(effect.phase, (slot as u8).wrapping_add(1));
    assert!(!session.economy.free_slots.contains(&slot));
    session.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(session.economy.free_slots.contains(&slot));
}
