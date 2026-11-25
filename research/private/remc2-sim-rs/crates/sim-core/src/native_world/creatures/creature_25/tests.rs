use super::*;

#[test]
fn cymmeria_repeat_exit_rearms_only_after_ten_inside_visits() {
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::native_world::Mc2DispatchEvent;
    use crate::{FlightWorld, PlayerCommand};

    let generated = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
    let package = std::env::var_os("MC_LEVEL10_PACKAGE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            generated.join("ribbon-20261104-integration/current-owner-package/level10")
        });
    let cache = std::env::var_os("MC_LEVEL10_OWNER_CACHE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| generated.join("campaign-original-environments/level10"));
    let mut campaign = CampaignSession::open_with_owner_cache(
        package,
        cache,
        CampaignSelection::ordinary(10).unwrap(),
    )
    .expect("open ordinary Cymmeria package at reset");

    let trigger = 255;
    let expected_rows: Vec<u16> = [249, 250, 251, 252].into_iter().chain(289..=308).collect();
    let sim = campaign.simulation();
    let authored = sim.mission().authored_features()[248];
    assert_eq!(
        (
            authored.kind,
            authored.subtype,
            authored.activation_group,
            authored.stage_tag
        ),
        (11, 3, 0, 1)
    );
    let root = &sim.entities()[trigger];
    assert_eq!(
        (
            root.class,
            root.model,
            root.action,
            root.source_id,
            root.phase
        ),
        (11, 3, 3, 1, 0)
    );
    assert_eq!(root.position, authored.world_position(root.position.z));
    assert_eq!(root.source_dword_10(), 0);
    let controller = sim.source_arena().handle(trigger as u16).unwrap();
    assert_eq!(sim.runtime().player_count, 1);
    let player = sim.state.entity_index;
    let outside = sim.state.position;
    let mut inside = root.position;
    inside.z = sim
        .canonical_terrain()
        .ground_height(inside)
        .wrapping_add(768);

    let visit = |campaign: &mut CampaignSession, outside: bool, publish: bool| {
        let sim = campaign.simulation();
        let root = &sim.entities()[trigger];
        let human = &sim.entities()[usize::from(player)];
        assert_eq!((human.class, human.model), (3, 0));
        let bounds = |row: &crate::native_world::EntitySnapshot| {
            let bytes = row.record_bytes();
            (
                i16::from_le_bytes([bytes[88], bytes[89]]),
                i16::from_le_bytes([bytes[90], bytes[91]]),
            )
        };
        let (root_width, root_depth) = bounds(root);
        let (human_width, human_depth) = bounds(human);
        assert_eq!(
            !crate::level1_mission::source_entities_overlap_xy(
                root.position,
                root_width,
                root_depth,
                sim.state.position,
                human_width,
                human_depth,
            ),
            outside,
            "actual human geometry before the ordinary switch visit",
        );
        let phase = root.phase;
        campaign.step(PlayerCommand::default(), 0);
        let sim = campaign.simulation();
        assert!(
            !sim.dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "repeat exit source dispatch: {:?}",
            sim.dispatch_events()
        );
        assert_eq!(sim.source_arena().handle(trigger as u16), Some(controller));
        let root = &sim.entities()[trigger];
        assert_eq!(root.phase, phase.wrapping_add(1));
        assert!(!root.drawing_disabled());
        let mut groups = sim.dispatch_events().iter().filter(|event| {
            matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated { group: 1, .. }
            )
        });
        if publish {
            assert!(outside && phase & 7 == 0);
            assert!(matches!(groups.next(),
                Some(Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 255, pass: 0, authored_indices, .. })
                    if authored_indices.as_ref() == expected_rows.as_slice()));
        }
        assert!(
            groups.next().is_none(),
            "only the due outside visit publishes group1"
        );
        root.source_dword_10()
    };
    let fields = |campaign: &CampaignSession| {
        let sim = campaign.simulation();
        sim.economy()
            .mana_attraction_fields()
            .iter()
            .filter(|field| field.active && field.authored_index == 250)
            .map(|field| {
                (
                    sim.source_arena()
                        .handle(field.source_entity_index)
                        .unwrap(),
                    field.life,
                )
            })
            .collect::<Vec<_>>()
    };

    assert!(fields(&campaign).is_empty());
    assert_eq!(visit(&mut campaign, true, true), 10);
    let first = fields(&campaign);
    assert_eq!(first.len(), 1);
    for _ in 0..16 {
        assert_eq!(
            visit(&mut campaign, true, false),
            10,
            "staying outside cannot rearm"
        );
    }
    let held = fields(&campaign);
    assert_eq!(held, [(first[0].0, first[0].1 - 16)]);

    // Control only the actual participant's position. Its ordinary visits
    // consume the countdown and stay negative while the participant is inside.
    campaign
        .simulation_mut()
        .set_level2_entity_position(player, inside)
        .unwrap();
    for remaining in (0..10).rev() {
        assert_eq!(visit(&mut campaign, false, false), remaining);
    }
    for _ in 0..8 {
        assert_eq!(
            visit(&mut campaign, false, false),
            0,
            "armed is not outside"
        );
    }

    campaign
        .simulation_mut()
        .set_level2_entity_position(player, outside)
        .unwrap();
    assert!((0..8).any(|_| {
        let due = campaign.simulation().entities()[trigger].phase & 7 == 0;
        visit(&mut campaign, true, due) == 10
    }));
    let repeated = fields(&campaign);
    assert_eq!(repeated.len(), 2);
    assert!(repeated.iter().any(|(handle, _)| *handle == first[0].0));
    let &(new_field, life) = repeated
        .iter()
        .find(|(handle, _)| *handle != first[0].0)
        .unwrap();
    assert_eq!(visit(&mut campaign, true, false), 10);
    assert!(
        fields(&campaign).contains(&(new_field, life - 1)),
        "second publication's real temporary field is scheduled"
    );
}

#[test]
fn cymmeria_source_objectives_reach_earned_mana_crossing_and_ordinary_exit() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::campaign::source_arena::SourceEntityHandle;
    use crate::game_session::{GameCommand, GameCompletionStatus, GameSession};
    use crate::level_definition::ObjectiveCondition;
    use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent, area_damage};
    use crate::{FlightWorld, Position};
    use std::collections::BTreeSet;

    type Game = GameSession<crate::campaign::backend::Mc2CampaignBackend>;
    fn sim(game: &Game) -> &crate::native_world::Mc2Simulation {
        game.backend().compatibility_campaign().simulation()
    }
    fn sim_mut(game: &mut Game) -> &mut crate::native_world::Mc2Simulation {
        game.backend_mut().compatibility_campaign_mut_for_test().simulation_mut()
    }
    fn step(game: &mut Game, command: GameCommand) {
        game.advance(game.identity().guard(), command).unwrap();
        assert!(sim(game).dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "Cymmeria ordinary dispatch: {:?}", sim(game).dispatch_events());
    }
    fn percent(game: &Game, player: u16) -> i32 {
        let economy = sim(game).economy();
        let total = economy.owner_mana_global();
        assert!(total > 0);
        economy.castle_for_owner(player).map_or(0, |castle|
            100 * economy.owner_mana_aggregate(player).saturating_add(castle.mana) / total)
    }

    let generated = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
    let package = std::env::var_os("MC_LEVEL10_PACKAGE").map(std::path::PathBuf::from)
        .unwrap_or_else(|| generated.join("ribbon-20261104-integration/current-owner-package/level10"));
    let cache = std::env::var_os("MC_LEVEL10_OWNER_CACHE").map(std::path::PathBuf::from)
        .unwrap_or_else(|| generated.join("campaign-original-environments/level10"));
    let definition = load_mc2_campaign(&package, &crate::game_definition::default_catalog().unwrap()).unwrap();
    let mut game = GameSession::open(&Mc2CampaignAdapter, std::sync::Arc::new(definition), 2210,
        Mc2CampaignInputs::with_owner_cache(&package, &cache)).unwrap();
    // Declared incoming carry: this is the actual family-1 holder used below.
    sim_mut(&mut game).apply_carried_spell_families(&[1], Some(1)).unwrap();
    let objectives = game.definition().level_definition().unwrap().objectives();
    assert_eq!(objectives.len(), 4);
    assert!(matches!(objectives[0].condition(), ObjectiveCondition::NoStandingBuildings));
    assert!(matches!(objectives[1].condition(), ObjectiveCondition::NoLivingCreatures(crate::level_definition::CreatureKind::Cymmerian)));
    assert!(matches!(objectives[2].condition(), ObjectiveCondition::ReachLocation(_)));
    assert!(matches!(objectives[3].condition(), ObjectiveCondition::CastleManaPercent(90)));
    let player = sim(&game).state.entity_index;
    assert_eq!(player, 314);
    assert_eq!(sim(&game).runtime().player_count, 1);
    let initial = sim(&game).campaign_building_objective_targets(0);
    assert_eq!(initial.len(), 6);
    let held_position = initial[0].1;
    let mut house_hits = BTreeSet::new();
    // Selected arrived damage to actual standing BUILD23->24->28 Houses;
    // leave one complete chain standing as the source absence negative.
    for leave_one in [true, false] {
        for _ in 0..512 {
            let targets = sim(&game).campaign_building_objective_targets(0);
            if leave_one && !targets.is_empty() && targets.iter().all(|(_, position)|
                (position.x, position.y) == (held_position.x, held_position.y)) { break; }
            if !leave_one && sim(&game).runtime().objectives[0].stage_selectors[0] == 2 { break; }
            for (slot, position) in targets {
                if leave_one && (position.x, position.y) == (held_position.x, held_position.y) { continue; }
                let ready = sim(&game).ordinary().houses().iter().any(|house|
                    house.active && house.action == 52 && house.entity_index == slot);
                if !ready { continue; }
                let handle = sim(&game).source_arena().handle(slot).unwrap();
                if house_hits.insert((handle.source_slot, handle.generation)) {
                    assert!(sim_mut(&mut game).session.state.live.ordinary.houses.submit_source_damage(slot, u16::MAX, player));
                }
            }
            step(&mut game, GameCommand::default());
            if leave_one {
                assert_eq!(sim(&game).runtime().objectives[0].stage_selectors[0], 1);
                assert!(!sim(&game).dispatch_events().iter().any(|event| matches!(event,
                    Mc2DispatchEvent::AuthoredGroupActivated { group: 8, .. })));
            }
        }
        if leave_one {
            let remaining = sim(&game).campaign_building_objective_targets(0);
            assert!(!remaining.is_empty() && remaining.iter().all(|(_, position)|
                (position.x, position.y) == (held_position.x, held_position.y)));
            assert_eq!(sim(&game).runtime().objectives[0].stage_selectors[0], 1);
        }
    }
    assert_eq!(sim(&game).runtime().objectives[0].stage_selectors[0], 2);
    let mut group8 = false;
    for _ in 0..64 {
        step(&mut game, GameCommand::default());
        group8 |= sim(&game).dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 1, group: 8, authored_indices, .. }
                if authored_indices.contains(&32)));
        if group8 { break; }
    }
    assert!(group8, "the real House objective must activate authored group8 including record32");
    assert_eq!(sim(&game).runtime().objectives[0].stage_selectors[1], 1);
    let mut seen = Vec::<SourceEntityHandle>::new();
    let mut struck = BTreeSet::new();
    let mut split_seen = false;
    // Damage is a selected arrived-contact control. Constructors, large-body
    // splitting, protected landings, live-list census and reclaim remain real.
    for _ in 0..640 {
        split_seen |= sim(&game).creature_runtime_census().roots_of_kind(crate::level_definition::CreatureKind::Cymmerian)
            .any(|row| matches!(row.origin, crate::campaign::source_arena::CreatureRootOrigin::CreatureConversion { .. }));
        let targets = sim(&game).entities().iter().filter(|row| row.active && row.class == 5 && row.model == 25)
            .map(|row| (sim(&game).source_arena().handle(row.index).unwrap(),
                row.life >= 0 && (row.action != 200 || row.record_bytes()[74] >= 3))).collect::<Vec<_>>();
        for (handle, vulnerable) in targets {
            if !seen.contains(&handle) { seen.push(handle); }
            if vulnerable && struck.insert((handle.source_slot, handle.generation)) {
                area_damage::contact(sim_mut(&mut game).source_arena_mut(), None, handle.source_slot, u16::MAX, player).unwrap();
            }
        }
        step(&mut game, GameCommand::default());
        if sim(&game).runtime().objectives[0].stage_selectors[1] == 2
            && seen.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()) { break; }
    }
    assert!(seen.len() >= 25 && split_seen, "real group8 and its source split generations must be observed");
    assert!(seen.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()));
    assert_eq!(sim(&game).runtime().objectives[0].stage_selectors[1], 2);
    assert!(sim(&game).source_arena().rebuilt_class5_model_slots()[25].is_empty());

    // Declared player position at the real source waypoint, with the
    // objective's own current-text and +/-768 checks left in charge.
    for _ in 0..128 {
        if sim(&game).runtime().objectives[0].text_selector == 2 { break; }
        step(&mut game, GameCommand::default());
    }
    assert_eq!(sim(&game).runtime().objectives[0].text_selector, 2);
    let mut waypoint = Position { x: 176 * 256, y: 32 * 256, z: 0 };
    waypoint.z = sim(&game).canonical_terrain().ground_height(waypoint).wrapping_add(512);
    sim_mut(&mut game).set_level2_entity_position(player, waypoint).unwrap();
    for _ in 0..32 {
        step(&mut game, GameCommand::default());
        if sim(&game).runtime().objectives[0].stage_selectors[2] == 2 { break; }
    }
    assert_eq!(sim(&game).runtime().objectives[0].stage_selectors[2], 2);

    // Isolate the final earned increment with a declared prior Castle, carried
    // Possession and bank. The actual neutral sphere and its source lifetime,
    // possession, Castle collector and objective crossing remain unmodified.
    for _ in 0..128 {
        if sim(&game).economy().spheres().iter().any(|row|
            row.active && !row.pending_delete && row.player_entity_index == 0 && row.control_flags & 8 != 0
                && row.control_flags & 0x0020_0000 == 0
                && row.vertical_velocity == 0 && row.mana > 0) { break; }
        step(&mut game, GameCommand::default());
    }
    let sphere = *sim(&game).economy().spheres().iter().find(|row|
        row.active && !row.pending_delete && row.player_entity_index == 0 && row.control_flags & 8 != 0
            && row.control_flags & 0x0020_0000 == 0
            && row.vertical_velocity == 0 && row.mana > 0)
        .expect("actual Cymmeria mana source supplies an eligible neutral sphere");
    let sphere_handle = sim(&game).source_arena().handle(sphere.source_entity_index).unwrap();
    let castle = {
        let state = sim_mut(&mut game);
        for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
            state.economy_mut().set_castle_stage_footprint(stage, footprint);
        }
        let site = (1_u16..32).flat_map(|y| (1_u16..32).map(move |x| Position {
            x: x * 2048, y: y * 2048, z: 0,
        })).map(|mut position| {
            position.z = state.canonical_terrain().ground_height(position);
            position
        }).filter(|position| {
            // Source Castle/sphere overlap uses wrapped XY, not altitude.
            // Keep the declared prior Castle outside that footprint with a
            // margin; the sphere and collector retain their natural poses.
            let dx = i32::from(position.x.wrapping_sub(sphere.position.x) as i16).abs();
            let dy = i32::from(position.y.wrapping_sub(sphere.position.y) as i16).abs();
            dx.max(dy) >= 4096
        })
            .filter(|position| !state.economy().castle_projectile_site_blocked(
                *position, state.entities(), state.canonical_terrain()))
            .min_by_key(|position| crate::distance_3d_wrapped(*position, sphere.position))
            .expect("declared prior Castle has an admitted real terrain site");
        state.economy_mut().build_castle(site, player).unwrap()
    };
    // The source census is frozen before this numeric pass. Its switch560
    // produces group16 afterwards, so their 35 actual creature generations
    // enter the *following* source census, not the preceding frozen total.
    assert_eq!(sim(&game).economy().castle_at(castle).unwrap().mana, 0);
    step(&mut game, GameCommand::default());
    assert!(sim(&game).dispatch_events().iter().any(|event| matches!(event,
        Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 560, group: 16, authored_indices, .. }
            if (368..=402).all(|record| authored_indices.contains(&record)))),
        "the real switch560 must produce the complete authored group16");
    let wave = sim(&game).creature_runtime_census().entries().iter().filter_map(|row| {
        let crate::campaign::source_arena::CreatureRootOrigin::AuthoredRecord { authored_record } = row.origin else { return None; };
        if !(368..=402).contains(&authored_record) { return None; }
        let source = &sim(&game).entities()[usize::from(row.handle.source_slot)];
        let (model, mana) = if authored_record <= 381 { (21, 1_000) } else { (17, 5_000) };
        assert_eq!((source.class, source.model, source.mana), (5, model, mana));
        Some((authored_record, row.handle, model, mana))
    }).collect::<Vec<_>>();
    assert_eq!(wave.iter().map(|row| row.0).collect::<BTreeSet<_>>(), (368..=402).collect());
    crate::native_world::Level2EconomyState::start_owner_mana_projection_trace();
    step(&mut game, GameCommand::default());
    let publications = crate::native_world::Level2EconomyState::take_owner_mana_projection_trace();
    let source_census = &publications.first().expect("following source census was published").1;
    for (_, handle, model, mana) in &wave {
        assert!(source_census.iter().any(|row| row.source_entity_index == handle.source_slot
            && row.source_class == 5 && row.source_model == *model && row.mana == *mana),
            "the next source census must include actual group16 generation {handle:?}");
    }
    assert_eq!(sim(&game).source_arena().handle(sphere_handle.source_slot), Some(sphere_handle));
    assert!(sim(&game).economy().spheres().iter().any(|row|
        row.source_entity_index == sphere_handle.source_slot && row.active && !row.pending_delete
            && row.player_entity_index == 0 && row.mana == sphere.mana));
    let total = sim(&game).economy().owner_mana_global();
    let aggregate = sim(&game).economy().owner_mana_aggregate(player);
    // Castle bank B is also part of the source global denominator. Solve
    // 100*(aggregate+B)/(total+B) >= 90 for the first crossing bank.
    let threshold = total.checked_mul(9).and_then(|n| aggregate.checked_mul(10).and_then(|a| n.checked_sub(a)))
        .expect("actual source census fits the prior-bank calculation").max(0);
    let bank = threshold.checked_sub(sphere.mana).expect("sphere payload fits prior-bank calculation");
    assert!(bank >= 0 && sphere.mana > 0, "actual denominator and sphere support a below-90 prior bank: total={total}, aggregate={aggregate}, sphere={sphere:?}");
    let capacity = aggregate.checked_add(bank).and_then(|n| n.checked_add(sphere.mana))
        .expect("actual collector capacity fits i32");
    assert!(capacity <= i32::MAX / 100, "actual objective numerator must fit source i32 arithmetic");
    let seeded_total = total.checked_add(bank).expect("actual source global fits i32");
    assert!(sim_mut(&mut game).economy_mut().set_castle_mana_prerequisite_for_test(castle, capacity));
    assert!(sim_mut(&mut game).economy_mut().set_castle_mana_prerequisite_for_test(castle, bank));
    let castle_handle = sim(&game).source_arena().handle(castle).unwrap();
    step(&mut game, GameCommand::default());
    assert_eq!(sim(&game).source_arena().handle(sphere_handle.source_slot), Some(sphere_handle));
    assert_eq!(sim(&game).economy().owner_mana_global(), seeded_total,
        "the real next source census must publish the declared bank against the observed baseline");
    assert!(percent(&game, player) < 90);
    assert_eq!(sim(&game).runtime().objectives[0].stage_selectors[3], 1);
    let target = sim(&game).possession_guidance_target_top_for_test(sphere.source_entity_index).unwrap();
    let state = sim_mut(&mut game);
    let fov = state.shared_spells().player_entity().array_fov();
    let mut hand = Position { x: target.x.wrapping_add(2048), y: target.y, z: target.z.checked_add(512).unwrap() };
    let yaw = crate::level1_mission::projectile_guidance_angles(hand, target).0;
    let hand_yaw = yaw.wrapping_sub(state.shared_spells().beyond_sight_auxiliary().next_entity).wrapping_sub(512);
    let mut origin = Position { z: hand.z.checked_sub(fov).unwrap(), ..hand };
    crate::move_entity(&mut origin, hand_yaw.wrapping_add(1024), 0, 256);
    origin.z = origin.z.max(state.canonical_terrain().ground_height(origin).wrapping_add(state.profile().ground_clearance))
        .max(state.canonical_terrain().ground_height(hand).wrapping_add(crate::level1_mission::FIREBALL_COLLISION_HALF_HEIGHT).wrapping_sub(fov));
    hand.z = origin.z.wrapping_add(fov);
    let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(hand, target);
    state.set_level2_entity_position(player, origin).unwrap();
    state.state.actual_speed = 0;
    state.state.target_speed = 0;
    state.set_level2_player_aim_for_test(yaw, pitch);
    let holder = sim(&game).shared_spells().local_player().enabled_spell_holders[1];
    let holder_handle = sim(&game).source_arena().handle(holder).unwrap();
    let cost = sim(&game).shared_spells().pickup(holder).unwrap().max_mana;
    let player_handle = sim(&game).source_arena().handle(player).unwrap();
    assert!(cost > 0 && sim(&game).shared_spells().player_entity().mana >= cost);
    let mut claim = false;
    let mut collected = None;
    let mut credited = false;
    let mut paid_launch = false;
    let mut paid_debit = false;
    let mut group4 = false;
    let mut retreated = false;
    let mut mana_trace = Vec::new();
    let mut relevant_events = std::collections::VecDeque::new();
    let mut first_meaningful_event = None;
    let source_snapshot = |game: &Game, slot: u16| {
        let row = &sim(game).entities()[usize::from(slot)];
        (sim(game).source_arena().handle(slot), row.active, row.class, row.model, row.action,
            row.life, row.source_id, row.position)
    };
    let gate_before = source_snapshot(&game, 224);
    let player_before = source_snapshot(&game, player);
    let player_link_before = {
        let record = sim(&game).players()[0].record_bytes();
        (u16::from_le_bytes([record[42], record[43]]), sim(&game).economy().player_castle_entity_index())
    };
    let mut post_arrival = Vec::new();
    let mut group4_events = Vec::new();
    let mut first_dead = None;
    let mut event_cursor = sim(&game).economy().events().len();
    let castle_position = sim(&game).economy().castle_at(castle).unwrap().position;
    let bound = 32 + 2 * (crate::distance_3d_wrapped(sphere.position, castle_position) as usize).div_ceil(48) + 192;
    for turn in 0..bound {
        if claim && !retreated {
            // Declared player-position control after the real paid claim:
            // retreat to the source level-end switch, clear of its terrain.
            // The wave, projectiles and collector continue their own turns.
            assert!(sim(&game).canonical_player_life_for_test() >= 0);
            let mut retreat = sim(&game).entities()[224].position;
            retreat.z = sim(&game).canonical_terrain().ground_height(retreat).wrapping_add(1024);
            sim_mut(&mut game).set_level2_entity_position(player, retreat).unwrap();
            retreated = true;
        }
        let before = (sim(&game).economy().owner_mana_global(),
            sim(&game).economy().owner_mana_aggregate(player),
            sim(&game).economy().castle_at(castle).map(|row| (row.mana, row.mana_capacity)),
            percent(&game, player));
        step(&mut game, GameCommand { cast_selected: turn == 0, ..GameCommand::default() });
        let state = sim(&game);
        if first_dead.is_none() && state.canonical_player_life_for_test() < 0 {
            let record = state.players()[0].record_bytes();
            first_dead = Some((turn, state.canonical_player_life_for_test(), source_snapshot(&game, player),
                u16::from_le_bytes([record[42], record[43]]), state.economy().player_castle_entity_index(),
                state.runtime().objectives[0], state.source_exit_result_byte()));
        }
        paid_debit |= state.cf10_production_schedule_receipts().into_iter().any(|receipt| match receipt {
            crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted {
                slot, original, current: crate::campaign::schedule::CurrentOccupant::Known { selector, .. }, ..
            } if slot == player => original.observed_player_resources().zip(selector.observed_player_resources())
                .is_some_and(|(before, after)| before.0 == player_handle && after.0 == player_handle
                    && before.2 <= -cost && after.1 == before.1.wrapping_add(before.2).clamp(0, before.3)
                    && before.1.saturating_sub(after.1) >= cost),
            _ => false,
        });
        if turn == 0 {
            let launched = state.economy().possession_projectiles().iter().any(|row|
                row.source_id_entity_index == player && row.holder_entity_index == holder_handle.source_slot
                    && state.source_arena().handle(row.source_entity_index).is_some());
            let charged = state.cf10_production_schedule_receipts().into_iter().any(|receipt| match receipt {
                crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted {
                    slot, original, current: crate::campaign::schedule::CurrentOccupant::Known { selector, .. }, ..
                } if slot == holder => original.observed_player_resources().zip(selector.observed_player_resources())
                    .is_some_and(|(before, after)| before.0 == player_handle && after.0 == player_handle
                        && before.1 >= cost && after.2 == if before.2 >= 0 { -cost } else { before.2.wrapping_sub(cost) }),
                _ => false,
            });
            paid_launch = launched && charged;
            assert!(paid_launch, "public Possession must launch and queue its real holder cost");
        }
        assert_eq!(state.source_arena().handle(castle), Some(castle_handle));
        for event in &state.economy().events()[event_cursor..] {
            if first_meaningful_event.is_none() && matches!(event,
                Level2EconomyEvent::ManaSphereCollected { sphere_id, .. }
                | Level2EconomyEvent::CastleDefenderCollectedSphere { sphere_id, .. }
                    if *sphere_id == u32::from(sphere_handle.source_slot)) {
                first_meaningful_event = Some((turn, format!("{event:?}")));
            }
            if matches!(event,
                Level2EconomyEvent::ManaSphereSpawned { .. }
                | Level2EconomyEvent::ManaSphereClaimed { .. }
                | Level2EconomyEvent::ManaSpheresMerged { .. }
                | Level2EconomyEvent::ManaSphereCollected { .. }
                | Level2EconomyEvent::CastleDefenderCollectedSphere { .. }
                | Level2EconomyEvent::CastleDestroyed { .. })
                || matches!(event, Level2EconomyEvent::CastleDefenderArrived { mana, .. } if *mana != 0) {
                if relevant_events.len() == 32 { relevant_events.pop_front(); }
                relevant_events.push_back((turn, format!("{event:?}; before={before:?}; after={:?}",
                    (state.economy().owner_mana_global(), state.economy().owner_mana_aggregate(player),
                        state.economy().castle_at(castle).map(|row| (row.mana, row.mana_capacity)), percent(&game, player)))));
            }
            match *event {
                Level2EconomyEvent::ManaSphereClaimed { sphere_id, owner_entity_index }
                    if sphere_id == u32::from(sphere_handle.source_slot) && owner_entity_index == player => claim = true,
                Level2EconomyEvent::CastleDefenderCollectedSphere { defender_slot, sphere_id, mana }
                    if sphere_id == u32::from(sphere_handle.source_slot) => {
                        assert!(claim && mana == sphere.mana);
                        collected = Some((state.source_arena().handle(defender_slot).unwrap(), mana));
                    }
                Level2EconomyEvent::CastleDefenderArrived { defender_slot, castle_id, mana }
                    if castle_id == castle && collected.is_some_and(|(handle, payload)| handle.source_slot == defender_slot && payload == mana) => {
                        assert_eq!(state.source_arena().handle(defender_slot), Some(collected.unwrap().0));
                        assert!(percent(&game, player) >= 90,
                            "actual collector arrival earns the 90-percent crossing: turn={turn}, prior(total,aggregate,castle,percent)={before:?}, after(total,aggregate,castle,percent)={:?}, seeded(total,aggregate,bank,threshold,sphere)={:?}, collector={collected:?}, census_receipts={relevant_events:?}, events={:?}",
                            (state.economy().owner_mana_global(), state.economy().owner_mana_aggregate(player),
                                state.economy().castle_at(castle).map(|row| (row.mana, row.mana_capacity)), percent(&game, player)),
                            (total, aggregate, bank, threshold, sphere.mana),
                            &state.economy().events()[event_cursor..]);
                        credited = true;
                    }
                _ => {}
            }
        }
        event_cursor = state.economy().events().len();
        if turn < 4 || turn % 64 == 0 || (credited && post_arrival.len() < 20) {
            let target = state.economy().spheres().iter().find(|row| row.source_entity_index == sphere_handle.source_slot);
            let castle_row = state.economy().castle_at(castle).map(|row|
                (row.action, row.level, row.life, row.mana, row.mana_capacity, row.position, row.defender_count));
            let collectors = state.economy().castle_children().iter().filter(|row|
                row.active && row.source_class == 3 && row.model == 3).map(|row|
                    (row.source_entity_index, row.action, row.position, row.mana, row.target_sphere_slot)).collect::<Vec<_>>();
            mana_trace.push((turn, state.source_arena().handle(sphere_handle.source_slot), target.copied(), castle_row, collectors));
        }
        if credited && post_arrival.len() < 20 {
            let gate = source_snapshot(&game, 224);
            post_arrival.push((turn, state.economy().owner_mana_global(),
                state.economy().owner_mana_aggregate(player), state.economy().castle_at(castle).map(|row| row.mana),
                percent(&game, player), state.runtime().objectives[0],
                (state.shared_spells().player_entity().life, state.canonical_player_life_for_test(), source_snapshot(&game, player)),
                state.source_exit_result_byte(), game.completion_status(game.identity().guard()).unwrap().value, gate));
        }
        group4_events.extend(state.dispatch_events().iter().filter(|event|
            matches!(event, Mc2DispatchEvent::AuthoredGroupActivated { group: 4, .. })).cloned());
        group4 |= state.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 224, group: 4, authored_indices, .. }
                if authored_indices.contains(&215) && authored_indices.contains(&34)));
        if !credited {
            assert!(percent(&game, player) < 90, "the declared prior bank must remain below 90 until the real deposit");
            assert_eq!(state.runtime().objectives[0].stage_selectors[3], 1);
        }
        if credited && group4 { break; }
    }
    assert!(paid_launch && paid_debit && claim && retreated && collected.is_some() && credited && group4,
        "real paid claim/collector arrival must earn the source 90-percent objective and group4: debit={paid_debit}, claim={claim}, retreated={retreated}, collected={collected:?}, credited={credited}, group4={group4}, bank={bank}, total={total}, sphere={sphere:?}, gate_before={gate_before:?}, player_before={player_before:?}, player_link_before={player_link_before:?}, first_dead={first_dead:?}, group4_events={group4_events:?}, post_arrival={post_arrival:?}, first_collection={first_meaningful_event:?}, recent_events={relevant_events:?}, trace={mana_trace:?}");
    assert_eq!(sim(&game).runtime().objectives[0].stage_selectors[..4], [2, 2, 2, 2]);
    assert_eq!(sim(&game).runtime().objectives[0].is_level_end, 1);
    let trigger = *sim(&game).economy().authored_switch_producers().iter().find(|row|
        row.active && row.authored_index == 215).expect("actual ordinary group4 exit");
    assert_eq!((trigger.model, trigger.action), (12, 12));
    assert!(sim(&game).economy().authored_class14().iter().any(|row| row.active && row.authored_index == 34));
    sim_mut(&mut game).set_level2_entity_position(player, trigger.position).unwrap();
    for _ in 0..700 {
        if matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }) { break; }
        step(&mut game, GameCommand::default());
    }
    assert!(matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }));
    assert_eq!(sim(&game).level_exit_sequence().trigger_entity_index, trigger.source_entity_index);
    assert_eq!(sim(&game).source_exit_result_byte(), 0x0a);
}

fn terrain() -> SessionTerrain {
    crate::native_world::tests::level_two_campaign_session()
        .canonical_terrain()
        .clone()
}

fn configured_player_headers() -> crate::campaign::source_player_result::SourcePlayerResult {
    crate::campaign::source_player_result::SourcePlayerResult::source_level_start(0)
}

fn level18_shared_player_state() -> crate::native_world::Level2SharedSpellResetSession {
    use crate::native_world::{
        Level2SharedSpellResetSession, MapType, OwnerPackageProfile, OwnerPackageSchema,
        parse_entities, parse_players, parse_runtime,
    };
    let package = crate::campaign::spec::DecodedCampaignPackage::decode(
        mission_eighteen_root().join("level18"),
        crate::campaign::spec::CampaignSelection::ordinary(18).unwrap(),
    )
    .expect("actual Mission18 source package");
    let entities = parse_entities(package.member("entities.bin").unwrap()).unwrap();
    let players = parse_players(package.member("players.bin").unwrap(),
        OwnerPackageProfile::Level2, OwnerPackageSchema::V5).unwrap();
    let runtime = parse_runtime(package.member("runtime.bin").unwrap()).unwrap();
    let map_type = match package.campaign_spec.presentation_recipe.map_type.as_str() {
        "Day" => MapType::Day,
        "Night" => MapType::Night,
        "Cave" => MapType::Cave,
        other => panic!("unrecognized actual Mission18 map type {other}"),
    };
    Level2SharedSpellResetSession::from_campaign_parts(&entities, &players, &runtime, map_type)
        .expect("Mission18 actual player and holder records")
}

fn target(slot: u16, class: u8, model: u8, position: Position) -> Target {
    Target {
        slot,
        source_id: slot,
        class,
        model,
        life: 1000,
        position,
        yaw: Some(0),
        roll: Some(0),
        pitch_extent: Some(0),
        roll_extent: Some(0),
        attacker: Some(0),
        target: Some(0),
        stage_index: Some(0),
        stage_mode: Some(0),
        parent: Some(0),
        stage_relation: Some(0),
        next: Some(0),
        ..Target::default()
    }
}

fn cymmerian(action: u8, position: Position) -> EntitySnapshot {
    let mut row = crate::native_world::cave_features::new_event(7, 19, position);
    row.class = SOURCE_CLASS;
    row.model = SOURCE_MODEL;
    row.record[67] = SOURCE_CLASS;
    row.record[68] = SOURCE_MODEL;
    row.record[61] = 64;
    row.parameter_relation = Some(DESCRIPTOR);
    row.record[164..168].copy_from_slice(&(u32::from(DESCRIPTOR) + 1).to_le_bytes());
    row.set_source_overlap_half_extents(RULES.collision_half_width, RULES.collision_half_width);
    row.record[92..94].copy_from_slice(&RULES.collision_half_height.to_le_bytes());
    row.set_source_action(action);
    row.set_max_life(RULES.max_life);
    row.set_life(RULES.max_life);
    row.set_actual_speed(0);
    row.set_phase(RULES.scan_period);
    row
}

fn mission_eighteen_root() -> std::path::PathBuf {
    std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj")
        },
        std::path::PathBuf::from,
    )
}

fn summoned_source_parts() -> (
    crate::campaign::source_arena::SourceArena,
    crate::native_world::RuntimeSnapshot,
    SessionTerrain,
    Vec<crate::native_world::ParticleParameter>,
    u16,
    u16,
    Position,
) {
    source_parts(18)
}

fn source_parts(
    mission: u8,
) -> (
    crate::campaign::source_arena::SourceArena,
    crate::native_world::RuntimeSnapshot,
    SessionTerrain,
    Vec<crate::native_world::ParticleParameter>,
    u16,
    u16,
    Position,
) {
    let package = crate::campaign::spec::DecodedCampaignPackage::decode(
        mission_eighteen_root().join(format!("level{mission}")),
        crate::campaign::spec::CampaignSelection::ordinary(u16::from(mission)).unwrap(),
    )
    .expect("decode owner-data Mission 18 package");
    let entities = crate::native_world::parse_entities(package.member("entities.bin").unwrap())
        .unwrap()
        .into_boxed_slice();
    let runtime =
        crate::native_world::parse_runtime(package.member("runtime.bin").unwrap()).unwrap();
    let presentation =
        crate::native_world::parse_presentation(package.member("presentation.bin").unwrap())
            .unwrap();
    let terrain = SessionTerrain::from_planes(
        crate::native_world::MapType::Night,
        crate::native_world::parse_terrain(package.member("terrain.bin").unwrap()).unwrap(),
        0,
    );
    let player = entities
        .iter()
        .find(|entity| entity.active && entity.class == 3 && entity.model == 0)
        .expect("Mission 18 retains a class3/model0 owner");
    let parent_slot = player.index;
    let owner_id = player.source_id;
    let position = player.position;
    let economy = crate::native_world::Level2EconomyState::from_parts_with_castle_stage_footprints(
        terrain.clone(),
        &entities,
        player.mana,
        player.model,
        0,
        0,
        runtime.random_state,
        Default::default(),
        runtime.constructor_counters(),
    )
    .unwrap();
    (
        crate::campaign::source_arena::SourceArena::new(entities, economy),
        runtime,
        terrain,
        presentation.particles.to_vec(),
        parent_slot,
        owner_id,
        position,
    )
}

fn family_with_cymmerian(slot: u16) -> [Vec<u16>; 29] {
    let mut families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    families[usize::from(SOURCE_MODEL)].push(slot);
    families
}

#[test]
fn source_constructor_uses_canonical_descriptor92_fields() {
    let particle = crate::native_world::ParticleParameter {
        base_tmap: 0,
        word_2: 0,
        word_4: 0,
        speed: 90,
        rotation_speed: 52,
        byte_10: 0,
        byte_11: 0,
        render_mode: 0,
        byte_13: 0,
    };
    let particles = vec![particle; usize::from(PARTICLE) + 1];
    let row = mc2::construct_row(
        7,
        0x1234,
        Position {
            x: 20_000,
            y: 20_000,
            z: 400,
        },
        9,
        &particles,
    )
    .unwrap();

    assert_eq!(
        (row.class, row.model, row.action),
        (SOURCE_CLASS, SOURCE_MODEL, 201)
    );
    assert_eq!(row.parameter_relation, Some(DESCRIPTOR));
    assert_eq!(
        (row.max_life, row.life, row.mana),
        (RULES.max_life, RULES.max_life, RULES.initial_mana)
    );
    assert_eq!(row.subspell_index, RULES.attack_damage);
    assert_eq!(row.phase, 9);
    assert_eq!(row.array_pitch(), RULES.collision_half_width);
    assert_eq!(row.array_roll(), RULES.collision_half_width);
    assert_eq!(row.array_fov(), RULES.collision_half_height);
}

#[test]
fn configured_source_default_and_army_share_birth_but_keep_distinct_mana() {
    let (mut arena, runtime, terrain, particles, parent, owner, position) = summoned_source_parts();
    let holder = arena.allocate_source_slot().expect("Army holder slot");
    let mut default_arena = arena.clone();
    let mut default_runtime = runtime.clone();
    let mut default_terrain = terrain.clone();
    let mut army_runtime = runtime;
    let mut army_terrain = terrain;
    let counter = default_runtime.constructor_counters().unwrap()[usize::from(SOURCE_MODEL)];
    let ordinary = mc2::allocate_source_default(
        &mut default_arena, &mut default_runtime, &mut default_terrain, &particles, position,
    ).unwrap().expect("actual default source allocation");
    let army = mc2::allocate_summoned(
        &mut arena, &mut army_runtime, &mut army_terrain, &particles, position,
        parent, owner, holder,
    ).unwrap().expect("actual Army source allocation");
    assert_eq!(ordinary, army, "both producers use the same constructor slot and generation");
    let ordinary = &default_arena.entities[usize::from(ordinary.source_slot)];
    let army = &arena.entities[usize::from(army.source_slot)];
    assert_eq!((ordinary.class, ordinary.model, ordinary.action, ordinary.mana), (5, 25, 201, 3750));
    assert_eq!((army.class, army.model, army.action, army.mana), (5, 25, 207, 0));
    assert_eq!((ordinary.max_life, ordinary.life, ordinary.phase, ordinary.entity_rng),
        (army.max_life, army.life, army.phase, army.entity_rng));
    assert_eq!(default_runtime.constructor_counters().unwrap()[usize::from(SOURCE_MODEL)], counter.wrapping_add(1));
    assert_eq!(army_runtime.constructor_counters().unwrap()[usize::from(SOURCE_MODEL)], counter.wrapping_add(1));
}

#[test]
fn configured_large_and_small_motion_use_source_descriptor_turn_response() {
    let terrain = terrain();
    for descriptor in [DESCRIPTOR, SPLIT_DESCRIPTOR] {
        let mut row = cymmerian(201, Position { x: 20_000, y: 20_000, z: 400 });
        row.parameter_relation = Some(descriptor);
        row.set_yaw(0);
        row.set_source_roll(200);
        // Same-tile, zero-speed motion isolates the source58350 turn after
        // the real movement helper; descriptor92 and95 both have subtype5.
        row.set_actual_speed(0);
        let ctx = Context {
            terrain: &terrain, targets: &[], class3: &[], model25: &[],
            reader: None, read_error: std::cell::Cell::new(None),
        };
        move_actor(&mut Actor { row: &mut row }, &ctx, &mut Effects::default());
        assert_eq!(row.yaw, 5, "descriptor{descriptor} must use subtype, not word4");
    }
}

#[test]
fn configured_pursuit_peer_overlap_uses_promoted_unsigned_axes() {
    let terrain = terrain();
    let position = Position { x: 65520, y: 20000, z: 400 };
    let target_position = Position { y: position.y + 512, ..position };
    let peer_position = Position { x: 8, ..position };
    let targets = [target(42, 3, 0, target_position), target(43, 5, 25, peer_position)];
    let mut row = cymmerian(202, position);
    row.set_phase(4);
    row.set_control_flags(row.control_flags | 0x800);
    row.record[154..156].copy_from_slice(&42_u16.to_le_bytes());
    let mut ctx = Context {
        terrain: &terrain, targets: &targets, class3: &[42], model25: &[43],
        reader: None, read_error: std::cell::Cell::new(None),
    };
    let expected = heading_for_positions(position, target_position) as i16;
    let incorrectly_wrapped_peer = heading_for_positions(peer_position, position) as i16;
    assert_ne!(expected, incorrectly_wrapped_peer);
    step(&mut Actor { row: &mut row }, &mut ctx).unwrap();
    assert_eq!(row.source_roll(), expected,
        "source peer overlap sees 65512, not a wrapped distance of24");
}

#[test]
fn configured_small_body_reads_current_player_and_retained_castle_without_list_or_life_gate() {
    let terrain = terrain();
    let position = Position { x: 20_000, y: 20_000, z: 400 };
    let mut player = target(42, 3, 0, position);
    player.castle_slot = 43;
    player.life = -1;
    let mut castle = target(43, 1, 0, position);
    castle.life = -1;
    castle.flags = crate::native_world::ENTITY_DRAW_DISABLED_BIT;
    castle.pitch_extent = Some(384);
    castle.roll_extent = Some(384);
    let targets = [player, castle];
    let mut ctx = Context {
        terrain: &terrain, targets: &targets, class3: &[], model25: &[],
        reader: None, read_error: std::cell::Cell::new(None),
    };
    let mut row = cymmerian(200, position);
    row.parameter_relation = Some(SPLIT_DESCRIPTOR);
    row.set_subspell_index(15000);
    row.set_phase(8);
    row.record[74] = 3;
    row.record[40..42].copy_from_slice(&42_u16.to_le_bytes());
    row.set_control_flags(row.control_flags | 0x800);
    step(&mut Actor { row: &mut row }, &mut ctx).unwrap();
    assert_eq!((row.record[74], word(&row, 154)), (5, 42),
        "direct source player address is independent of frozen class3 membership");
    row.set_phase(8);
    row.set_control_flags(row.control_flags | 0x800);
    step(&mut Actor { row: &mut row }, &mut ctx).unwrap();
    assert_eq!(row.record[74], 6, "the current retained Castle remains the source target");
    row.set_control_flags(row.control_flags | 0x800);
    let effects = step(&mut Actor { row: &mut row }, &mut ctx).unwrap();
    assert_eq!(effects.castle_hit, Some(CastleHit { player_slot: 42, castle_slot: 43, damage: 60 }));
}

#[test]
fn ordinary_seek_acquires_a_live_target_and_moves_to_action202() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 400,
    };
    // The source scans the frozen class-3 chain, not every nearby entity.
    let targets = [target(41, 5, 18, position), target(42, 3, 0, position)];
    let class3 = [42];
    let mut row = cymmerian(201, position);
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();

    assert_eq!(row.action, 202);
    assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 42);
    assert!(effects.attack.is_none());
}

#[test]
fn ordinary_seek_keeps_searching_when_no_target_is_in_source_range() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 400,
    };
    let targets = [target(
        42,
        3,
        0,
        Position {
            x: position.x.wrapping_add(RULES.target_range as u16 + 1),
            ..position
        },
    )];
    let class3 = [42];
    let mut row = cymmerian(201, position);
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    step(&mut Actor { row: &mut row }, &mut context).unwrap();

    assert_eq!(row.action, 201);
    assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 0);
}

#[test]
fn authored_follow_leader_attacks_only_its_in_range_target() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let terrain = session.canonical_terrain();
    let position = session.state.position;
    let mut runtime = session.session.state.live.core.runtime.clone();
    runtime.stage_variables[1].flags = 0;
    runtime.stage_variables[1].relation = crate::native_world::RuntimeRelation::LiveEntity(42);
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    for (offset, expected_action) in [(0_u16, 202), (RULES.target_range as u16 + 100, 207)] {
        let mut leader = target(42, 5, 18, position);
        leader.target = Some(43);
        let enemy = target(
            43,
            3,
            0,
            Position {
                x: position.x.wrapping_add(offset),
                ..position
            },
        );
        let targets = [leader, enemy];
        let mut row = cymmerian(207, position);
        row.record[76] = 1;
        row.record[77] = 4;
        row.set_phase(8);
        let context = Context {
            terrain,
            targets: &targets,
            class3: &[43],
            model25: &[],
            reader: None,
            read_error: std::cell::Cell::new(None),
        };
        assert_eq!(
            stage::step_action_207(
                &mut Actor { row: &mut row },
                &context,
                &runtime,
                &families,
                true,
                &mut Effects::default(),
            )
            .unwrap(),
            None
        );
        assert_eq!(row.action, expected_action);
        if expected_action == 202 {
            assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 43);
            assert_eq!(row.record[77], 10);
        }
    }

    // The ordinary Level22 route also binds StageVar8. 8 and 9 share
    // sub_1E1C0: a clear mailbox moves and wanders on phase 0, an enemy hit
    // enters the real attack action, and a same-family hit does not.
    for mode in [8_u8, 9] {
        let mut row = cymmerian(207, position);
        row.record[77] = mode;
        row.set_phase(0);
        row.set_control_flags(row.control_flags | 0x800);
        let old_rng = row.entity_rng;
        let old_roll = row.source_roll();
        let context = Context {
            terrain,
            targets: &[],
            class3: &[],
            model25: &[],
            reader: None,
            read_error: std::cell::Cell::new(None),
        };
        stage::step_action_207(
            &mut Actor { row: &mut row },
            &context,
            &runtime,
            &families,
            true,
            &mut Effects::default(),
        )
        .unwrap();
        let next_rng = level2_next_rng16(old_rng);
        assert_eq!(row.action, 207);
        assert_eq!(row.entity_rng, next_rng);
        assert_eq!(
            row.source_roll(),
            old_roll.wrapping_add((next_rng % 113 + 142) as i16) & 2047
        );

        for (attacker_model, damage, action, next_mode) in [
            (18, 100, 202, 10),
            (SOURCE_MODEL, 100, 207, mode),
            (18, RULES.max_life + 1, 204, mode),
        ] {
            let targets = [target(42, SOURCE_CLASS, attacker_model, position)];
            let context = Context {
                terrain,
                targets: &targets,
                class3: &[],
                model25: &[],
                reader: None,
                read_error: std::cell::Cell::new(None),
            };
            let mut row = cymmerian(207, position);
            row.record[77] = mode;
            row.record[98..102].copy_from_slice(&damage.to_le_bytes());
            row.record[102..104].copy_from_slice(&42_u16.to_le_bytes());
            stage::step_action_207(
                &mut Actor { row: &mut row },
                &context,
                &runtime,
                &families,
                true,
                &mut Effects::default(),
            )
            .unwrap();
            assert_eq!((row.action, row.record[77]), (action, next_mode));
            assert_eq!(row.life, RULES.max_life - damage);
            assert_eq!(u16::from_le_bytes([row.record[102], row.record[103]]), 0);
            if action == 202 {
                assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 42);
            }
        }
    }

    let ground = terrain.ground_height(position);
    let raised = Position {
        z: ground + 512,
        ..position
    };
    for (descriptor, expected_z) in [(DESCRIPTOR, ground + 480), (SPLIT_DESCRIPTOR, ground + 256)] {
        let targets = [target(42, SOURCE_CLASS, 18, raised)];
        let context = Context {
            terrain,
            targets: &targets,
            class3: &[],
            model25: &[],
            reader: None,
            read_error: std::cell::Cell::new(None),
        };
        let mut row = cymmerian(207, raised);
        row.parameter_relation = Some(descriptor);
        row.record[77] = 8;
        row.record[98..102].copy_from_slice(&100_i32.to_le_bytes());
        row.record[102..104].copy_from_slice(&42_u16.to_le_bytes());
        stage::step_action_207(
            &mut Actor { row: &mut row },
            &context,
            &runtime,
            &families,
            true,
            &mut Effects::default(),
        )
        .unwrap();
        assert_eq!(
            row.position.z, expected_z,
            "hit uses its current descriptor's descent"
        );
    }
}

#[test]
fn stage_follow_modes_choose_leader_only_when_requested() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let terrain = session.canonical_terrain();
    let position = session.state.position;
    let mut runtime = session.session.state.live.core.runtime.clone();
    runtime.stage_variables[1].flags = 0;
    runtime.stage_variables[1].relation = crate::native_world::RuntimeRelation::LiveEntity(42);
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let leader = target(42, 5, 18, position);
    let context = Context {
        terrain,
        targets: &[leader],
        class3: &[],
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };
    for (mode, action, expected_target) in [(3, 202, 42), (4, 207, 0), (5, 207, 0)] {
        let mut row = cymmerian(207, position);
        row.record[76] = 1;
        row.record[77] = mode;
        row.set_phase(8);
        stage::step_action_207(
            &mut Actor { row: &mut row },
            &context,
            &runtime,
            &families,
            true,
            &mut Effects::default(),
        )
        .unwrap();
        assert_eq!((row.action, word(&row, 154)), (action, expected_target));
    }
    let mut unreadable_leader = leader;
    unreadable_leader.target = None;
    let context = Context {
        terrain,
        targets: &[unreadable_leader],
        class3: &[],
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };
    let mut row = cymmerian(207, position);
    row.record[76] = 1;
    row.record[77] = 4;
    row.set_phase(8);
    let error = stage::step_action_207(
        &mut Actor { row: &mut row },
        &context,
        &runtime,
        &families,
        true,
        &mut Effects::default(),
    )
    .unwrap_err();
    assert!(error.contains("target 42 lacks current target"));
}

#[test]
fn stage_follow_reads_ordinary_castle_defenders_live_target_over_raw_shadow() {
    use crate::native_world::{Level2DynamicArenaKind, RuntimeRelation, area_damage, source_target};
    use crate::native_world::creatures::creature_15;

    let (mut arena, mut runtime, mut terrain, particles, owner, _, position) = source_parts(18);
    let hostile = arena.entities.iter().find(|row|
        row.active && row.class == 3 && row.source_id != owner && row.life >= 0)
        .expect("actual Level18 hostile class3 target").clone();
    let castle = arena.economy.build_castle(position, owner, &terrain)
        .expect("ordinary Castle constructor");
    // Stage three is the source prerequisite for sub5FF50's model15 producer.
    arena.economy.castle.as_mut().unwrap().level = 3;
    arena.economy.maintain_castle_defenders(castle, &terrain);
    let leader = arena.economy.castle().unwrap().auxiliary_slots[0];
    assert_ne!(leader, 0);
    assert_eq!(arena.economy.live_arena_cursor_row(leader).unwrap().kind,
        Level2DynamicArenaKind::CastleAuxiliary);
    let handle = arena.handle(leader).unwrap();

    // Exercise the actual damage writer and model15 retaliation callback to
    // produce a nonzero target, rather than assuming NewEvent's zero word.
    area_damage::contact(&mut arena, None, leader, 1, hostile.index)
        .expect("contact writes the actual defender mailbox");
    let targets = creature_15::mc2::targets_in_economy(&arena.economy, &arena.entities);
    creature_15::mc2::step_economy(&mut arena.economy, leader,
        creature_15::DefenderContext {
            terrain: &terrain, targets: &targets, class3: &[hostile.index],
            model15: &[leader], class5: &[leader], stage_runtime: None,
        }).expect("ordinary defender consumes contact and retaliates");
    let defender = arena.economy.castle_defenders().iter()
        .find(|row| row.active && row.source_entity_index == leader).unwrap();
    assert_eq!((defender.action, defender.target_sphere_slot), (122, hostile.index));
    assert_eq!(defender.damage_attacker_entity_index, hostile.index);
    assert_eq!(arena.handle(leader), Some(handle));

    // The typed occupant owns word96 even before any raw publication. A
    // plausible stale packet must neither erase nor supply the live target.
    put(&mut arena.entities[usize::from(leader)], 154, 999);
    let current = source_target::read(&arena, source_target::Context::default(), leader)
        .unwrap().unwrap();
    assert_eq!((current.class, current.model), (5, 15));
    assert_eq!(current.relations.target, Some(hostile.index));
    assert_eq!(current.relations.attacker, Some(hostile.index));

    let actor = mc2::allocate_authored(&mut arena, &mut runtime, &mut terrain,
        &particles, hostile.position, RULES.max_life, None, 0)
        .unwrap().unwrap();
    runtime.stage_variables[1].kind = 4;
    runtime.stage_variables[1].flags = 2;
    runtime.stage_variables[1].relation = RuntimeRelation::Scalar(15);
    let row = &mut arena.entities[usize::from(actor)];
    row.set_source_action(207);
    row.record[76] = 1;
    row.record[77] = 4;
    row.set_phase(8);
    put(row, 78, leader);
    let mut families = family_with_cymmerian(actor);
    families[15].push(leader);
    mc2::step_row(&mut arena, &mut runtime, &mut configured_player_headers(), &mut terrain, &particles, actor,
        &[hostile.index], &families, source_target::Context::default())
        .expect("source mode4 consumes its cached current defender target");
    let row = &arena.entities[usize::from(actor)];
    assert_eq!((row.action, row.record[77], word(row, 78), word(row, 154)),
        (202, 10, leader, hostile.index));
    assert_eq!(arena.handle(leader), Some(handle));
    assert_eq!(word(&arena.entities[usize::from(leader)], 154), 999,
        "the shared reader borrows the actual carrier without publishing a raw mirror");
}

#[test]
fn stage_follow_reads_current_mana_sphere_collector_through_hidden_reclaim() {
    use crate::native_world::{Level2DynamicArenaKind, RuntimeRelation, source_target};

    let package = std::env::var_os("MC_DEFENDER_HOSTILE_PACKAGE").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/all-spells-20261104-integration/current-owner-package/level18"),
        std::path::PathBuf::from,
    );
    let mut session = crate::campaign::CampaignSession::open(package,
        crate::campaign::spec::CampaignSelection::ordinary(18).unwrap())
        .expect("actual Level18 owner package").simulation().clone();
    let particles = session.session.definition.presentation.particles.to_vec();
    let live = &mut session.session.state.live;
    let owner = live.core.state.entity_index;
    let position = live.core.state.position;
    let arena = &mut live.economy;
    let runtime = &mut live.core.runtime;
    let terrain = &mut live.terrain;
    let shared = &mut live.shared_spells;
    let castle = arena.economy.build_castle(position, owner, terrain)
        .expect("ordinary Castle constructor");
    arena.economy.maintain_castle_defenders(castle, terrain);
    let collector = arena.economy.castle().unwrap().defender_slots[0];
    assert_eq!(arena.economy.live_arena_kind(collector), Some(Level2DynamicArenaKind::CastleDefender));
    let collector_position = arena.economy.castle_children().iter()
        .find(|row| row.source_entity_index == collector).unwrap().position;
    let sphere_position = Position { x: collector_position.x.wrapping_add(512), ..collector_position };
    let sphere = crate::world::effects::spawn_death_mana_sphere(arena, terrain,
        owner, sphere_position, 0, 512, 0).unwrap().unwrap();
    let sphere_handle = arena.handle(sphere).unwrap();
    assert_eq!(arena.economy.live_arena_kind(sphere), Some(Level2DynamicArenaKind::ManaSphere));
    put(&mut arena.entities[usize::from(sphere)], 154, 999);
    assert_eq!(source_target::read(arena, Default::default(), sphere).unwrap().unwrap()
        .relations.target, Some(0), "the ordinary neutral constructor owns its zero word96");

    let actor = mc2::allocate_authored(arena, runtime, terrain, &particles,
        sphere_position, RULES.max_life, None, 0).unwrap().unwrap();
    runtime.stage_variables[1].kind = 4;
    runtime.stage_variables[1].flags = 2;
    runtime.stage_variables[1].relation = RuntimeRelation::Scalar(26);
    let row = &mut arena.entities[usize::from(actor)];
    row.set_source_action(207);
    row.record[76] = 1;
    row.record[77] = 4;
    row.set_phase(8);
    put(row, 78, sphere);
    let families = family_with_cymmerian(actor);
    mc2::step_row(arena, runtime, &mut configured_player_headers(), terrain, &particles, actor, &[], &families,
        Default::default()).expect("a cached numeric leader reads the neutral sphere's zero target");
    assert_eq!((arena.entities[usize::from(actor)].action,
        word(&arena.entities[usize::from(actor)], 154)), (207, 0));

    // Ordinary ownership and Castle target arbitration precede the real
    // AddBallon callback. No test writes the sphere's collector relation.
    assert!(arena.economy.claim_mana_sphere(sphere, owner));
    // Declared free-capacity prerequisite: sub5FF50 compares the current
    // house-mana census plus the Castle balance with its capacity. Reserve
    // room for this actual sphere, then leave the Castle balance empty.
    arena.economy.rebuild_owner_mana_projection_at_source_boundary(&arena.entities);
    let aggregate = arena.economy.owner_mana_aggregate(owner);
    let incoming_mana = arena.economy.spheres().iter().find(|row|
        row.source_entity_index == sphere).unwrap().mana;
    let capacity = aggregate.checked_add(incoming_mana).unwrap();
    assert!(arena.economy.set_castle_mana_prerequisite_for_test(castle, capacity));
    assert!(arena.economy.set_castle_mana_prerequisite_for_test(castle, 0));
    let root = arena.economy.castle_at(castle).unwrap();
    assert!(aggregate + root.mana < root.mana_capacity,
        "collector selection requires real free capacity: aggregate={aggregate}, root={root:?}");
    // The bounded Castle adapter consumes the actual pass-start model39
    // membership. A same-pass birth cannot enter its scan until this real
    // owning boundary captures all current sphere slots.
    arena.economy.rebuild_possession_guidance_source_slots();
    assert!(arena.economy.possession_guidance_source_slots.contains(&sphere),
        "the newborn sphere must enter the following pass's real frozen membership");
    let holder = arena.economy.castle_children().iter().find(|row|
        row.source_entity_index == collector).unwrap();
    let current_sphere = arena.economy.spheres().iter().find(|row|
        row.source_entity_index == sphere).unwrap();
    assert_eq!(current_sphere.player_entity_index, holder.id_entity_index);
    assert_eq!(current_sphere.control_flags & 0x2000, 0);
    assert_eq!(arena.economy.select_castle_defender_sphere(
        holder.position, holder.id_entity_index, 0, 0), Some(sphere),
        "the admitted real sphere must satisfy sub5F810 before maintenance: sphere={current_sphere:?}");
    arena.economy.maintain_castle_defenders(castle, terrain);
    assert_eq!(arena.economy.castle_children().iter().find(|row|
        row.source_entity_index == collector).unwrap().target_sphere_slot, sphere);
    let step_collector = |arena: &mut crate::campaign::source_arena::SourceArena,
        terrain: &SessionTerrain, shared: &mut crate::native_world::Level2SharedSpellResetSession| {
        let target = source_target::collector_target(arena, Default::default(), sphere).unwrap();
        let mut consumer = source_target::CastleCollectorConsumer {
            target, entities: &mut arena.entities, shared,
        };
        assert!(arena.economy.step_castle_defender_slot_with_consumer(collector, terrain, &mut consumer));
    };
    step_collector(arena, terrain, shared);
    let assigned = *arena.economy.spheres().iter().find(|row| row.source_entity_index == sphere).unwrap();
    assert_eq!(assigned.assigned_defender_slot, collector);
    assert_ne!(assigned.control_flags & 0x40, 0);
    assert!(!assigned.pending_delete);
    put(&mut arena.entities[usize::from(sphere)], 154, 999);
    assert_eq!(source_target::read(arena, Default::default(), sphere).unwrap().unwrap()
        .relations.target, Some(collector));
    mc2::step_row(arena, runtime, &mut configured_player_headers(), terrain, &particles, actor, &[], &families,
        Default::default()).expect("cached StageVar4 reads the current nonzero sphere collector");
    assert_eq!((arena.entities[usize::from(actor)].action,
        arena.entities[usize::from(actor)].record[77], word(&arena.entities[usize::from(actor)], 154)),
        (202, 10, collector));

    // Clone-only boundary prerequisite selects sub5FF50's excess-holder
    // retirement branch. This is not an actual damage/death claim. The
    // source hides the holder and clears its owning array entry, leaving
    // this separate sphere's physical word96 and flags untouched.
    let mut retirement = arena.clone();
    assert_eq!(retirement.economy.castle_children().iter().find(|row|
        row.source_entity_index == collector).unwrap().mana, 0);
    retirement.economy.castle_at_mut(castle).unwrap().level = 0;
    retirement.economy.maintain_castle_defenders(castle, terrain);
    assert_eq!(retirement.economy.castle_at(castle).unwrap().defender_slots[0], 0);
    assert_ne!(retirement.economy.castle_children().iter().find(|row|
        row.source_entity_index == collector).unwrap().control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
    let still_assigned = retirement.economy.spheres().iter().find(|row|
        row.source_entity_index == sphere).unwrap();
    assert_eq!((still_assigned.assigned_defender_slot, still_assigned.control_flags),
        (collector, assigned.control_flags));
    assert_eq!(source_target::read(&retirement, Default::default(), sphere).unwrap().unwrap()
        .relations.target, Some(collector));

    // Outside the attraction radius AddBallon clears only bit40; word96
    // still names the last collector. Control only encounter geometry here.
    let child = arena.economy.castle_children.iter_mut()
        .find(|row| row.source_entity_index == collector).unwrap();
    child.position.x = assigned.position.x.wrapping_add(2048);
    step_collector(arena, terrain, shared);
    let released = arena.economy.spheres().iter().find(|row| row.source_entity_index == sphere).unwrap();
    assert_eq!(released.control_flags & 0x40, 0);
    assert_eq!(released.assigned_defender_slot, collector);
    assert_eq!(source_target::read(arena, Default::default(), sphere).unwrap().unwrap()
        .relations.target, Some(collector));

    // Place the collector's actual collision centre on the sphere, then let
    // its ordinary callback collect/hide it and the owning boundary reclaim.
    let sphere_view = source_target::read(arena, Default::default(), sphere).unwrap().unwrap();
    let collector_shape = source_target::read(arena, source_target::Context {
        particles: Some(&particles), map_type: Some(terrain.map_type()), ..Default::default()
    }, collector)
        .unwrap().unwrap().collision.unwrap();
    let child = arena.economy.castle_children.iter_mut()
        .find(|row| row.source_entity_index == collector).unwrap();
    child.position = Position {
        z: sphere_view.position.z.wrapping_add(sphere_view.collision.unwrap().vertical_offset)
            .wrapping_sub(collector_shape.vertical_offset),
        ..sphere_view.position
    };
    step_collector(arena, terrain, shared);
    let hidden = source_target::read(arena, Default::default(), sphere).unwrap().unwrap();
    assert_eq!(hidden.relations.target, Some(collector));
    assert_ne!(hidden.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
    assert_eq!(arena.handle(sphere), Some(sphere_handle));
    assert!(arena.economy.spheres().iter().find(|row| row.source_entity_index == sphere).unwrap().pending_delete);
    put(&mut arena.entities[usize::from(sphere)], 154, 999);
    arena.rebuild_source_arena_with_terrain(terrain, &[]).unwrap();
    assert!(arena.economy.free_source_slots().contains(&sphere));
    assert!(!arena.economy.spheres().iter().any(|row| row.source_entity_index == sphere));
    let retired = source_target::read(arena, Default::default(), sphere).unwrap().unwrap();
    assert_eq!((retired.class, retired.model, retired.relations.target), (0, 39, Some(collector)));
    assert_eq!(word(&arena.entities[usize::from(sphere)], 154), collector);
}

#[test]
fn stage_follow_reused_model26_address_reads_speed_trail_zero_axes_and_target() {
    use crate::level1_mission::SOURCE_REPLACEABLE_FLAG;
    use crate::native_world::{Level2DynamicArenaKind, RuntimeRelation, source_target};

    let (mut arena, mut runtime, mut terrain, particles, owner, _, position) = source_parts(18);
    let target_slot = arena
        .entities
        .iter()
        .find(|row| row.active && row.class == 5 && row.model == 26)
        .expect("actual Level18 model26 target")
        .index;
    let old_handle = arena.handle(target_slot).unwrap();
    let old_target = mc2::target_at(&arena, source_target::Context::default(), target_slot)
        .unwrap()
        .unwrap();
    assert_eq!((old_target.class, old_target.model), (5, 26));
    assert!(old_target.pitch_extent.unwrap() > 0);

    // Isolate NewEvent's real replacement branch, retaining the creature's
    // cached numeric address across the generation change.
    for row in &mut arena.entities {
        row.set_control_flags(row.control_flags & !SOURCE_REPLACEABLE_FLAG);
    }
    let row = &mut arena.entities[usize::from(target_slot)];
    row.set_control_flags(row.control_flags | SOURCE_REPLACEABLE_FLAG);
    arena.rebuild_source_free_arena();
    arena.economy.free_slots.clear();
    let slot = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
    assert_eq!(slot, target_slot);
    let trail_position = Position {
        x: position.x.wrapping_add(RULES.collision_half_width as u16 + 8),
        y: position.y.wrapping_add(RULES.collision_half_width as u16 + 8),
        ..position
    };
    arena.economy.initialize_speed_trail_at(slot, trail_position, owner);
    assert_eq!(arena.economy.live_arena_kind(slot), Some(Level2DynamicArenaKind::SpeedTrail));
    assert_ne!(arena.handle(slot).unwrap(), old_handle);
    assert!(arena.resolve(old_handle).is_none());

    // A predecessor's plausible axes/target must never supply the typed
    // replacement. SpeedTrail has no particle resize: its source axes and
    // target word remain zero from NewEvent, independent of particle zero.
    let raw = &mut arena.entities[usize::from(slot)];
    raw.set_source_overlap_half_extents(4096, 4096);
    put(raw, 154, 999);
    let source_context = source_target::Context {
        particles: Some(&particles),
        map_type: Some(terrain.map_type()),
        ..Default::default()
    };
    let current = source_target::read(&arena, source_context, slot).unwrap().unwrap();
    assert_eq!((current.class, current.model, current.source_id, current.life), (10, 2, owner, 32));
    assert_eq!(current.position, trail_position);
    let axes = current.collision.expect("SpeedTrail retains NewEvent's physical zero axes");
    assert_eq!((axes.half_width, axes.half_depth, axes.vertical_offset, axes.half_height), (0, 0, 0, 0));
    assert_eq!(current.relations.target, Some(0));

    runtime.stage_variables[1].kind = 4;
    runtime.stage_variables[1].flags = 2;
    runtime.stage_variables[1].relation = RuntimeRelation::Scalar(26);
    let mut families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    families[26].push(target_slot);
    let context = Context {
        terrain: &terrain,
        targets: &[],
        class3: &[],
        model25: &[],
        reader: Some((&arena, source_context)),
        read_error: std::cell::Cell::new(None),
    };
    let mut actor = cymmerian(207, position);
    actor.record[76] = 1;
    actor.record[77] = 4;
    actor.set_phase(8);
    put(&mut actor, 78, target_slot);
    stage::step_action_207(
        &mut Actor { row: &mut actor },
        &context,
        &runtime,
        &families,
        true,
        &mut Effects::default(),
    ).expect("source mode4 follows the current replacement and reads its zero target");
    assert_eq!((actor.action, actor.record[77], word(&actor, 78), word(&actor, 154)), (207, 4, target_slot, 0));
    assert_eq!(actor.source_roll(), crate::heading_for_positions(actor.position, trail_position) as i16,
        "the retired model26 width must not reverse the current heading");
    assert_eq!(context.read_error.get(), None);
    drop(context);

    // Run the constructor's real 32-life countdown through all 34 callbacks;
    // no synthetic inactive carrier or shortened lifetime supplies this tail.
    for visit in 1..=34 {
        assert_eq!(arena.economy.step_speed_trail(slot), visit < 34);
        let trail = arena.economy.speed_trails().iter()
            .find(|row| row.source_entity_index == slot).unwrap();
        assert_eq!(trail.life, 32 - visit);
    }
    let terminal = *arena.economy.speed_trails().iter()
        .find(|row| row.source_entity_index == slot).unwrap();
    assert!(!terminal.active);
    assert_ne!(terminal.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
    assert_eq!(arena.economy.live_arena_kind(slot), None);
    assert!(!arena.economy.free_source_slots().contains(&slot));
    assert!(!arena.economy.map_chains.is_linked(slot));
    let hidden = source_target::read(&arena, source_context, slot).unwrap().unwrap();
    assert_eq!((hidden.class, hidden.model, hidden.life, hidden.position), (10, 2, -2, trail_position));
    assert_eq!((hidden.source_id, hidden.control_flags), (owner, terminal.control_flags));
    let context = Context {
        terrain: &terrain,
        targets: &[],
        class3: &[],
        model25: &[],
        reader: Some((&arena, source_context)),
        read_error: std::cell::Cell::new(None),
    };
    stage::step_action_207(
        &mut Actor { row: &mut actor },
        &context,
        &runtime,
        &families,
        true,
        &mut Effects::default(),
    ).expect("the later cached-address callback still reads the hidden trail's last packet");
    assert_eq!((actor.action, word(&actor, 78), word(&actor, 154)), (207, target_slot, 0));
    assert_eq!(actor.source_roll(), crate::heading_for_positions(actor.position, trail_position) as i16);
    drop(context);

    // The actual following boundary freezes physical fields and frees the
    // typed carrier. Fixed-address readers then see the source class0 packet.
    arena.rebuild_source_arena_with_terrain(&mut terrain, &[]).unwrap();
    assert!(!arena.economy.speed_trails().iter().any(|row| row.source_entity_index == slot));
    assert!(arena.economy.free_source_slots().contains(&slot));
    assert!(!arena.economy.map_chains.is_linked(slot));
    let retired = &arena.entities[usize::from(slot)];
    assert_eq!((retired.class, retired.model, retired.life, retired.max_life), (0, 2, -2, 8));
    assert_eq!((retired.position, retired.phase, retired.entity_rng),
        (terminal.position, terminal.phase, terminal.entity_rng));
    assert_eq!((retired.source_id, retired.control_flags), (owner, terminal.control_flags & !4));
    assert_eq!((retired.array_pitch(), retired.array_roll(), retired.array_yaw(), retired.array_fov()), (0, 0, 0, 0));
    let current = source_target::read(&arena, source_context, slot).unwrap().unwrap();
    assert_eq!((current.class, current.model, current.life, current.position), (0, 2, -2, trail_position));
    assert_eq!(current.relations.target, Some(0));
}

#[test]
fn stage_guard_acquires_only_visible_class3_on_scan_cadence() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let position = session.state.position;
    let terrain = session.canonical_terrain();
    let mut runtime = session.session.state.live.core.runtime.clone();
    runtime.stage_variables[1].relation = crate::native_world::RuntimeRelation::AxisScalar(
        u32::from(position.x) | (u32::from(position.y) << 16),
    );
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    for (hidden, phase, expected) in [(false, 20, 202), (true, 20, 207), (false, 19, 207)] {
        let mut enemy = target(42, 3, 0, position);
        if hidden {
            enemy.flags |= 0x20;
        }
        let targets = [enemy];
        let context = Context {
            terrain,
            targets: &targets,
            class3: &[42],
            model25: &[],
            reader: None,
            read_error: std::cell::Cell::new(None),
        };
        let mut row = cymmerian(207, position);
        row.record[76] = 1;
        row.record[77] = 2;
        row.set_phase(phase);
        row.set_yaw(crate::heading_for_positions(position, position) as i16);
        stage::step_action_207(
            &mut Actor { row: &mut row },
            &context,
            &runtime,
            &families,
            true,
            &mut Effects::default(),
        )
        .unwrap();
        assert_eq!(row.action, expected);
        assert_eq!(word(&row, 154), if expected == 202 { 42 } else { 0 });
    }
}

#[test]
fn stage_default_preserves_actor_but_attack_mode_selects_source_action() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let position = session.state.position;
    let runtime = &session.session.state.live.core.runtime;
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let context = Context {
        terrain: session.canonical_terrain(),
        targets: &[],
        class3: &[],
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };
    for (mode, action) in [(0, 207), (11, 207), (15, 207), (10, 202)] {
        let mut row = cymmerian(207, position);
        row.record[77] = mode;
        put(&mut row, 154, 42);
        let rng = row.entity_rng;
        stage::step_action_207(
            &mut Actor { row: &mut row },
            &context,
            runtime,
            &families,
            true,
            &mut Effects::default(),
        )
        .unwrap();
        assert_eq!(
            (row.action, row.record[77], word(&row, 154), row.entity_rng),
            (action, mode, 42, rng)
        );
    }
}

#[test]
fn stage_morph_tracks_current_parent_and_hides_without_it() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let terrain = session.canonical_terrain();
    let position = session.state.position;
    let runtime = &session.session.state.live.core.runtime;
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let mut parent = target(42, 3, 0, Position { z: 900, ..position });
    parent.yaw = Some(123);
    parent.roll = Some(321);
    let context = Context {
        terrain,
        targets: &[parent],
        class3: &[],
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };
    for current in [true, false] {
        let mut row = cymmerian(207, position);
        row.record[77] = 12;
        put(&mut row, 44, 42);
        stage::step_action_207(
            &mut Actor { row: &mut row },
            &context,
            runtime,
            &families,
            current,
            &mut Effects::default(),
        )
        .unwrap();
        if current {
            assert_eq!(
                (row.position.z, row.yaw, row.source_roll()),
                (388, 123, 321)
            );
            assert_eq!(row.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
        } else {
            assert_ne!(row.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
        }
    }
    parent.position.z = 200;
    let context = Context {
        terrain,
        targets: &[parent],
        class3: &[],
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };
    let mut row = cymmerian(207, position);
    row.record[77] = 12;
    put(&mut row, 44, 42);
    stage::step_action_207(
        &mut Actor { row: &mut row },
        &context,
        runtime,
        &families,
        true,
        &mut Effects::default(),
    )
    .unwrap();
    assert_eq!(row.position.z, 0);
}

#[test]
fn alliance_attacks_parent_target_then_expires_on_parent_loss() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let terrain = session.canonical_terrain();
    let position = session.state.position;
    let runtime = &session.session.state.live.core.runtime;
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let mut parent = target(42, 3, 0, position);
    parent.target = Some(43);
    let enemy = target(43, 5, 18, position);
    let targets = [parent, enemy];
    let context = Context {
        terrain,
        targets: &targets,
        class3: &[],
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };
    for current in [true, false] {
        let mut row = cymmerian(207, position);
        row.record[77] = 14;
        put(&mut row, 44, 42);
        put(&mut row, 50, 40);
        put(&mut row, 52, 60);
        put(&mut row, 78, 3);
        let mut effects = Effects::default();
        stage::step_action_207(
            &mut Actor { row: &mut row },
            &context,
            runtime,
            &families,
            current,
            &mut effects,
        )
        .unwrap();
        if current {
            assert_eq!((row.action, word(&row, 154), word(&row, 50)), (202, 43, 39));
            assert_eq!(effects.feedback, Some((42, 24)));
        } else {
            assert_eq!((row.record[77], row.record[76], word(&row, 44)), (10, 3, 0));
            assert_eq!(effects.feedback, None);
        }
    }
}

#[test]
fn ordinary_seek_waits_for_descriptor92_scan_cadence() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 400,
    };
    let targets = [target(42, 3, 0, position)];
    let class3 = [42];
    let mut row = cymmerian(201, position);
    row.set_phase(RULES.scan_period.wrapping_sub(1));
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    step(&mut Actor { row: &mut row }, &mut context).unwrap();

    assert_eq!(row.action, 201);
    assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 0);
}

#[test]
fn split_body_uses_descriptor95_scan_period_and_target_range() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 400,
    };
    let target_position = Position {
        x: position.x.wrapping_add(3500),
        ..position
    };
    let targets = [target(42, 3, 0, target_position)];
    let class3 = [42];
    let mut row = cymmerian(201, position);
    row.parameter_relation = Some(SPLIT_DESCRIPTOR);
    row.set_source_overlap_half_extents(RULES.split_half_extent, RULES.split_half_extent);
    row.record[92..94].copy_from_slice(&RULES.split_half_extent.to_le_bytes());
    row.set_phase(RULES.split_scan_period);
    row.set_yaw(heading_for_positions(position, target_position) as i16);
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    step(&mut Actor { row: &mut row }, &mut context).unwrap();

    assert_eq!(row.action, 202);
    assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 42);
}

#[test]
fn ordinary_pursuit_requests_one_class9_model0_projectile_for_live_target() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 400,
    };
    let target_position = Position {
        x: position.x.wrapping_add(1000),
        ..position
    };
    let targets = [target(42, 3, 0, target_position)];
    let class3 = [42];
    let mut row = cymmerian(202, position);
    row.parameter_relation = Some(DESCRIPTOR);
    row.record[154..156].copy_from_slice(&42_u16.to_le_bytes());
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();

    let attack = effects
        .attack
        .expect("live in-range pursuit creates a shot");
    assert_eq!(
        (attack.target, attack.target_class, attack.target_model),
        (42, 3, 0)
    );
    assert_eq!(attack.parameter, RULES.projectile_damage);
    assert_eq!(
        attack.position.z,
        row.position.z.wrapping_add(row.array_fov())
    );
}

#[test]
fn ordinary_pursuit_drops_a_target_outside_descriptor92_range() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 400,
    };
    let target_position = Position {
        x: position.x.wrapping_add(RULES.target_range as u16 + 1),
        ..position
    };
    let targets = [target(42, 3, 0, target_position)];
    let class3 = [42];
    let mut row = cymmerian(202, position);
    row.parameter_relation = Some(DESCRIPTOR);
    row.record[154..156].copy_from_slice(&42_u16.to_le_bytes());
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();

    assert!(effects.attack.is_none());
    // sub_1C310 returns to base+1 directly when the target is out of range.
    assert_eq!(row.action, 201);
}

#[test]
fn ordinary_subspell_counter_and_state_timer_follow_source_fields() {
    let terrain = terrain();
    let targets = [];
    let class3 = [];
    let mut row = cymmerian(200, Position::default());
    row.set_subspell_index(RULES.attack_damage);
    row.record[74] = 3;
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    step(&mut Actor { row: &mut row }, &mut context).unwrap();

    assert_eq!(row.subspell_index, RULES.attack_damage - 1);
    assert_eq!(word(&row, 46), RULES.attack_damage - 1);
    assert_eq!(row.source_dword_10(), 100);
    assert_eq!(row.action, 200);
    assert_eq!(row.record[74], 8);
}

#[test]
fn protected_landing_discards_damage_through_the_transition_to_state3() {
    let terrain = terrain();
    let mut row = cymmerian(
        200,
        Position {
            x: 20_000,
            y: 20_000,
            z: 400,
        },
    );
    row.parameter_relation = Some(SPLIT_DESCRIPTOR);
    row.record[74] = 1;
    row.set_max_life(RULES.split_max_life);
    row.set_life(RULES.split_max_life);
    row.set_subspell_index(RULES.split_attack_damage);
    let mut context = Context {
        terrain: &terrain,
        targets: &[],
        class3: &[],
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };
    // Source28860 sets52 then decrements on the first visit. All53 visits,
    // including the final2->3 transition, restore life and clear word62.
    for visit in 0..53 {
        row.record[98..102].copy_from_slice(&1000_i32.to_le_bytes());
        row.record[102..104].copy_from_slice(&42_u16.to_le_bytes());
        step(&mut Actor { row: &mut row }, &mut context).unwrap();
        assert_eq!(row.life, RULES.split_max_life, "protected visit{visit}");
        assert_eq!(word(&row, 102), 0, "protected visit{visit}");
        assert_eq!(row.action, 200);
        assert_eq!(row.record[74], if visit == 52 { 3 } else { 2 });
        assert_eq!(word(&row, 46), RULES.split_attack_damage - visit - 1);
    }
    // No stale attacker survives protection. A newly delivered hit after
    // landing must still take the ordinary small-body prekill route.
    step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(row.life, RULES.split_max_life);
    row.record[98..102].copy_from_slice(&1000_i32.to_le_bytes());
    row.record[102..104].copy_from_slice(&42_u16.to_le_bytes());
    step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(row.action, 204);
    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert!(effects.prekill && !effects.split);
}

#[test]
fn water_visual_change_above_floor_preserves_speed_but_entry_and_land_set_it() {
    let (_, _, terrain, particles, _, _, _) = source_parts(18);
    let sample = |water: bool| {
        (0_u16..=255)
            .find_map(|y| {
                (0_u16..=255).find_map(|x| {
                    let mut point = Position {
                        x: x * 256 + 128,
                        y: y * 256 + 128,
                        z: 0,
                    };
                    ((terrain.material_mask(point) == crate::WATER_AMBIENCE_MASK) == water).then(
                        || {
                            point.z = terrain.ground_height(point) + 512;
                            point
                        },
                    )
                })
            })
            .expect("unchanged owner Mission18 has both water and land samples")
    };
    let water = sample(true);
    let land = sample(false);
    let mut row = cymmerian(200, water);
    row.parameter_relation = Some(SPLIT_DESCRIPTOR);
    row.record[74] = 4;
    row.set_source_dword_10(100);
    row.set_subspell_index(RULES.split_attack_damage);
    row.set_phase(1);
    put(&mut row, 136, RULES.split_minimum_speed as u16);
    row.set_actual_speed(123);
    crate::world::effects::bind_particle(&mut row, SPLIT_PARTICLE, &particles).unwrap();
    let mut context = Context {
        terrain: &terrain,
        targets: &[],
        class3: &[],
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };
    // Branch controls: position this existing small-body shape above real
    // water, keep byte46 unchanged, and use source movement-skip bit0x800 so
    // the callback cannot cross onto another tile. No terrain is rewritten.
    row.set_control_flags(row.control_flags | 0x800);
    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(effects.visual, Some(313));
    assert!(
        !effects.speed_changed,
        "source visual-only branch leaves v26 clear"
    );
    assert_eq!((word(&row, 136), row.actual_speed), (35, 123));

    crate::world::effects::bind_particle(&mut row, 313, &particles).unwrap();
    row.set_control_flags(row.control_flags | 0x800);
    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(effects.visual, Some(SPLIT_PARTICLE));
    assert!(effects.speed_changed);
    assert_eq!((word(&row, 136), row.actual_speed), (35, 35));

    crate::world::effects::bind_particle(&mut row, SPLIT_PARTICLE, &particles).unwrap();
    row.set_position(land);
    row.set_control_flags(row.control_flags | 0x800);
    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(effects.visual, Some(313));
    assert!(effects.speed_changed);
    assert_eq!((word(&row, 136), row.actual_speed), (60, 60));
}

#[test]
fn ordinary_death_callback_requests_three_children_before_parent_kill() {
    let terrain = terrain();
    let mut row = cymmerian(204, Position::default());
    let targets = [];
    let class3 = [];
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();

    assert!(effects.split);
    assert_eq!(effects.population_increments, 3);
    assert_eq!(row.action, 205);
}

#[test]
fn configured_small_prekill_awards_one_credit_across_all_kill_visits() {
    let (mut arena, mut runtime, mut terrain, particles, player, _, position) = source_parts(18);
    let shared = level18_shared_player_state();
    let mut headers = crate::campaign::source_player_result::SourcePlayerResult::source_level_start(
        u16::from(runtime.local_player_slot),
    );
    let header_index = usize::from(runtime.local_player_slot) + 1;
    assert_eq!(shared.players[usize::from(runtime.local_player_slot)].entity_index, player);
    let target_context = crate::native_world::source_target::Context {
        spells: Some(&shared), ..Default::default()
    };
    let child = mc2::allocate_source_default(
        &mut arena, &mut runtime, &mut terrain, &particles, position,
    ).unwrap().expect("real constructor for configured small prekill");
    let row = &mut arena.entities[usize::from(child.source_slot)];
    row.parameter_relation = Some(SPLIT_DESCRIPTOR);
    row.record[74] = 3;
    row.set_source_action(204);
    row.set_life(-1);
    row.set_phase(1);
    put(row, 40, player);
    let families = family_with_cymmerian(child.source_slot);
    let prekill = mc2::step_row(&mut arena, &mut runtime, &mut headers, &mut terrain, &particles,
        child.source_slot, &[], &families, target_context).unwrap();
    assert_eq!(prekill.kill_credit, Some(player));
    assert_eq!(headers.raw_headers()[header_index], 0, "source204 is not the header writer");
    assert_eq!(arena.entities[usize::from(child.source_slot)].action, 205);
    for phase in 1..=8 {
        arena.entities[usize::from(child.source_slot)].set_phase(phase);
        let result = mc2::step_row(&mut arena, &mut runtime, &mut headers, &mut terrain, &particles,
            child.source_slot, &[], &families, target_context).unwrap();
        assert_eq!(result.kill_credit, None, "source205 visit{phase} must not repeat prekill credit");
        assert_eq!(headers.raw_headers()[header_index], u32::from(phase),
            "each source205 visit writes the neighboring header before phase-gated hide");
        if phase < 8 {
            assert!(!arena.entities[usize::from(child.source_slot)].drawing_disabled());
        } else {
            assert!(arena.entities[usize::from(child.source_slot)].drawing_disabled());
        }
    }
}

#[test]
fn action206_is_rejected_because_the_pinned_source_has_no_handler() {
    let terrain = terrain();
    let targets = [];
    let class3 = [];
    let mut row = cymmerian(206, Position::default());
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    let error = step(&mut Actor { row: &mut row }, &mut context).unwrap_err();

    assert!(error.contains("action206 has no implementation"));
    assert_eq!(row.action, 206);
}

#[test]
fn army_ring_allocates_a_real_summoned_cymmerian_with_holder_provenance() {
    let (mut arena, mut runtime, mut terrain, particles, parent_slot, owner_id, position) =
        summoned_source_parts();
    let holder_slot = arena
        .allocate_source_slot()
        .expect("reserve the Army holder's source slot");
    let counter_at = crate::native_world::RUNTIME_BYTES + usize::from(SOURCE_MODEL);
    let initial_counter = runtime.bytes[counter_at];

    let handle = mc2::allocate_summoned(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        parent_slot,
        owner_id,
        holder_slot,
    )
    .unwrap()
    .expect("ordinary Army ring has source capacity");

    let row = &arena.entities[usize::from(handle.source_slot)];
    assert_eq!((row.class, row.model, row.action), (5, SOURCE_MODEL, 207));
    assert_eq!(row.parameter_relation, Some(DESCRIPTOR));
    assert_eq!(row.particle_parameter_index, Some(PARTICLE));
    assert_eq!((row.max_life, row.life), (RULES.max_life, RULES.max_life));
    assert_eq!(row.actual_speed, RULES.minimum_speed);
    assert_eq!(row.subspell_index, RULES.attack_damage);
    assert_eq!(row.phase, initial_counter);
    assert_eq!(row.record[74], 0, "ordinary constructor keeps byte46 clear");
    assert_eq!(
        row.record[77],
        crate::native_world::summon_army::SUMMONED_STAGE_MODE
    );
    assert_eq!(
        u16::from_le_bytes([row.record[44], row.record[45]]),
        parent_slot
    );
    assert_eq!(
        u16::from_le_bytes([row.record[30], row.record[31]]),
        owner_id
    );
    assert_eq!(row.source_id, owner_id);
    assert_eq!(
        u16::from_le_bytes([row.record[50], row.record[51]]),
        crate::native_world::summon_army::SUMMONED_REMAINING_LIFETIME
    );
    assert_eq!(row.active_value, 250);
    assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 0);
    assert_eq!((row.mana, row.max_mana, row.mana_cost), (0, 0, 0));
    assert!(row.record[144..148].iter().all(|byte| *byte == 0));
    assert_eq!(row.position, position);
    assert_eq!(runtime.bytes[counter_at], initial_counter.wrapping_add(1));

    let identity = arena
        .imported_creature_runtime_identity(handle.source_slot, &std::collections::BTreeMap::new())
        .expect("summoned Cymmerian has live raw-creature identity");
    assert_eq!(identity.handle, handle);
    assert_eq!(
        identity.origin,
        crate::campaign::source_arena::CreatureRootOrigin::SummonedSpell { holder_slot }
    );
}

#[test]
fn summoned_cymmerian_follows_reused_parent_until_it_dies() {
    let (mut arena, mut runtime, mut terrain, particles, parent_slot, owner_id, position) =
        summoned_source_parts();
    let holder = arena.allocate_source_slot().unwrap();
    let child = mc2::allocate_summoned(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        parent_slot,
        owner_id,
        holder,
    )
    .unwrap()
    .unwrap();
    let parent = arena.handle(parent_slot).unwrap();
    assert_eq!(arena.creature_parent(child), Some(parent));

    let mut freed = arena.clone();
    let mut freed_runtime = runtime.clone();
    let mut freed_terrain = terrain.clone();
    freed.entities[usize::from(parent_slot)].set_life(0);
    freed.entities[usize::from(parent_slot)].reclaim_source_row();
    freed.economy.release_slot(parent_slot, None);
    let families = family_with_cymmerian(child.source_slot);
    mc2::step_row(
        &mut freed, &mut freed_runtime, &mut configured_player_headers(), &mut freed_terrain, &particles, child.source_slot,
        &[], &families, crate::native_world::source_target::Context::default(),
    ).unwrap();
    assert!(!freed.entities[usize::from(child.source_slot)].drawing_disabled());

    arena.economy.release_slot(parent_slot, None);
    assert_eq!(
        arena.allocate_source_slot_with_terrain(&mut terrain),
        Some(parent_slot)
    );
    let mut replacement = crate::native_world::cave_features::new_event(parent_slot, 5, position);
    replacement.class = 3;
    replacement.model = 0;
    replacement.record[67] = 3;
    replacement.set_life(1000);
    arena.entities[usize::from(parent_slot)] = replacement;
    assert_ne!(arena.handle(parent_slot), Some(parent));
    assert_eq!(arena.creature_parent(child), None);

    let families = family_with_cymmerian(child.source_slot);
    mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut configured_player_headers(),
        &mut terrain,
        &particles,
        child.source_slot,
        &[],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert!(!arena.entities[usize::from(child.source_slot)].drawing_disabled());
    arena.entities[usize::from(parent_slot)].set_life(-1);
    mc2::step_row(
        &mut arena, &mut runtime, &mut configured_player_headers(), &mut terrain, &particles, child.source_slot,
        &[], &families, crate::native_world::source_target::Context::default(),
    ).unwrap();
    assert!(arena.entities[usize::from(child.source_slot)].drawing_disabled());
}

#[test]
fn authored_cymmerian_alliance_impact_follows_current_parent_until_death() {
    let package = crate::campaign::spec::DecodedCampaignPackage::decode(
        mission_eighteen_root().join("level10"),
        crate::campaign::spec::CampaignSelection::ordinary(10).unwrap(),
    )
    .expect("decode real Mission 10 authored records");
    let feature = crate::level1_mission::parse_authored_features(package.authored_bytes())
        .unwrap()
        .into_iter()
        .find(|row| row.kind == 5 && row.subtype == 25)
        .expect("Mission 10 contains an authored Cymmerian");
    let (mut arena, mut runtime, mut terrain, particles, parent_slot, _, _) = source_parts(10);
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let slot = mc2::allocate_authored(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        RULES.max_life,
        None,
        feature.index,
    )
    .unwrap()
    .expect("authored Cymmerian allocation");
    let child = arena.handle(slot).unwrap();
    let parent = arena.handle(parent_slot).unwrap();
    // The same authored body visits the real source route prepass. Its
    // target relation survives a live player and clears when that carrier dies.
    for dead in [false, true] {
        let mut route_arena = arena.clone();
        let mut route_runtime = runtime.clone();
        let row = &mut route_arena.entities[usize::from(slot)];
        row.set_source_action(207);
        row.record[76] = 1;
        row.record[77] = 3;
        put(row, 78, parent_slot);
        route_arena.entities[usize::from(parent_slot)].set_life(if dead { -1 } else { 1000 });
        route_runtime.stage_variables[1].kind = 3;
        route_runtime.stage_variables[1].flags = 2;
        route_runtime.stage_variables[1].relation =
            crate::native_world::RuntimeRelation::AxisScalar(3);
        mc2::prepare_source_pass(
            &mut route_arena,
            &mut route_runtime,
            &[slot],
            crate::native_world::source_target::Context::default(),
        )
        .unwrap();
        assert_eq!(
            word(&route_arena.entities[usize::from(slot)], 78),
            if dead { 0 } else { parent_slot }
        );
    }
    let old_parent = arena
        .entities
        .iter()
        .find(|row| {
            row.active
                && row.index != 0
                && row.index != parent_slot
                && row.index != slot
                && arena.handle(row.index).is_some()
        })
        .and_then(|row| arena.handle(row.index))
        .expect("owner package has a prior live source parent");
    assert!(arena.bind_creature_parent(child, old_parent));
    let impact_slot =
        crate::native_world::alliance_spell::construct_impact(&mut arena, &mut terrain)
            .expect("allocate Alliance's real impact carrier");
    let impact = &mut arena.entities[usize::from(impact_slot)];
    impact.source_id = parent_slot;
    impact.record[30..32].copy_from_slice(&parent_slot.to_le_bytes());
    impact.record[74] = 32;
    impact.subspell_index = 40;
    impact.record[154..156].copy_from_slice(&slot.to_le_bytes());
    let outcome =
        crate::native_world::alliance_spell::step_impact(&mut arena, impact_slot, &[slot]).unwrap();
    assert_eq!(outcome.affected_entities, vec![slot]);
    assert_eq!(arena.creature_parent(child), Some(parent));
    assert_eq!(arena.entities[usize::from(slot)].record[77], 14);
    arena.entities[usize::from(slot)].set_phase(1);
    let families = family_with_cymmerian(slot);
    mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut configured_player_headers(),
        &mut terrain,
        &particles,
        slot,
        &[parent_slot],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert_eq!(word(&arena.entities[usize::from(slot)], 50), 39);
    assert_eq!(arena.entities[usize::from(slot)].record[77], 14);

    arena.economy.release_slot(parent_slot, None);
    assert_eq!(
        arena.allocate_source_slot_with_terrain(&mut terrain),
        Some(parent_slot)
    );
    let mut replacement = crate::native_world::cave_features::new_event(parent_slot, 5, position);
    replacement.class = 3;
    replacement.record[67] = 3;
    replacement.set_life(1000);
    arena.entities[usize::from(parent_slot)] = replacement;
    assert_ne!(arena.handle(parent_slot), Some(parent));
    mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut configured_player_headers(),
        &mut terrain,
        &particles,
        slot,
        &[parent_slot],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert_eq!(arena.entities[usize::from(slot)].record[77], 14);
    assert_eq!(word(&arena.entities[usize::from(slot)], 44), parent_slot);
    arena.entities[usize::from(parent_slot)].set_life(0);
    mc2::step_row(
        &mut arena, &mut runtime, &mut configured_player_headers(), &mut terrain, &particles, slot, &[parent_slot], &families,
        crate::native_world::source_target::Context::default(),
    ).unwrap();
    assert_eq!(arena.entities[usize::from(slot)].record[77], 10);
    assert_eq!(word(&arena.entities[usize::from(slot)], 44), 0);
}

/// Continue the existing public Army cast against a real Night rival. This
/// remains separate from the baseline cast's expiry assertions.
pub(crate) fn assert_night_army_cymmerian_lifetime(
    campaign: &crate::campaign::CampaignSession,
    player: u16,
    holder: u16,
) {
    use crate::PlayerCommand;
    use crate::campaign::source_arena::{CreatureRootOrigin, SourceEntityView};
    use crate::native_world::Mc2DispatchEvent;
    use std::collections::{BTreeMap, BTreeSet};

    let mut attack = campaign.clone();
    let current = attack.simulation();
    let owner_id = current.entities()[usize::from(player)].source_id;
    let rival = current
        .entities()
        .iter()
        .find(|row| {
            row.active
                && row.class == 3
                && row.model == 1
                && row.life > 0
                && row.source_id != owner_id
        })
        .expect("Night 7 owner data contains an opposing living rival");
    let rival_slot = rival.index;
    let position = Position {
        x: rival.position.x.wrapping_add(1024),
        y: rival.position.y,
        z: rival.position.z.saturating_add(512),
    };
    attack
        .simulation_mut()
        .set_level2_entity_position(player, position)
        .unwrap();
    let parent_handle = attack.simulation().source_arena().handle(player).unwrap();
    let mut pursuing = None;
    for turn in 0..480 {
        attack.step(
            if turn == 0 {
                PlayerCommand {
                    opcode: 6,
                    argument_1: 19,
                    flags: 0x40,
                    ..PlayerCommand::default()
                }
            } else {
                PlayerCommand::default()
            },
            0,
        );
        let session = attack.simulation();
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "Night Army pursuit dispatch failed: {:?}",
            session.dispatch_events()
        );
        let arena = session.source_arena();
        pursuing = arena.entities.iter().find_map(|row| {
            if !row.active
                || (row.class, row.model, row.action) != (5, 25, 202)
                || u16::from_le_bytes([row.record[154], row.record[155]]) != rival_slot
            {
                return None;
            }
            let identity = arena.imported_creature_runtime_identity(row.index, &BTreeMap::new())?;
            (identity.origin
                == CreatureRootOrigin::SummonedSpell {
                    holder_slot: holder,
                }
                && arena.creature_parent_binding(identity.handle) == Some(parent_handle))
            .then_some(identity.handle)
        });
        if pursuing.is_some() {
            break;
        }
    }
    let parent = pursuing.expect("actual Army Cymmerian must pursue the real rival");
    eprintln!("Night7 Army Cymmerian pursuit: parent={parent:?}, rival={rival_slot}");
    let phase = attack.simulation().source_arena().entities[usize::from(parent.source_slot)].phase;
    crate::native_world::area_damage::contact(
        attack.simulation_mut().source_arena_mut(),
        None,
        parent.source_slot,
        u16::MAX,
        rival_slot,
    )
    .unwrap();
    attack.step(PlayerCommand::default(), 0);
    let session = attack.simulation();
    assert!(
        !session
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
        "Night Army damage dispatch failed: {:?}",
        session.dispatch_events()
    );
    let Some(SourceEntityView::Imported(row)) = session.source_arena().resolve(parent) else {
        panic!("damaged Cymmerian must retain its original source generation");
    };
    assert_eq!(
        row.life, 0,
        "source28C60 clamps the lethal attack callback tail"
    );
    assert_eq!(row.action, 204, "the real hit selects the death callback");
    assert_eq!(
        row.phase,
        phase.wrapping_add(1),
        "the same-pass source tail runs after the hit"
    );
    assert!(
        session
            .source_arena()
            .imported_creature_runtime_identity(parent.source_slot, &BTreeMap::new())
            .expect("the original dying root retains its semantic identity")
            .living
    );

    let mut children = BTreeMap::new();
    let mut parent_hidden = false;
    let mut parent_reclaimed = false;
    for _ in 0..96 {
        attack.step(PlayerCommand::default(), 0);
        let session = attack.simulation();
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "Night Army split dispatch failed: {:?}",
            session.dispatch_events()
        );
        let arena = session.source_arena();
        if let Some(SourceEntityView::Imported(row)) = arena.resolve(parent) {
            parent_hidden |= row.drawing_disabled();
        } else {
            parent_reclaimed |= parent_hidden;
        }
        for row in arena
            .entities
            .iter()
            .filter(|row| row.active && row.class == 5 && row.model == 25)
        {
            let Some(identity) =
                arena.imported_creature_runtime_identity(row.index, &BTreeMap::new())
            else {
                continue;
            };
            if identity.origin
                == (CreatureRootOrigin::CreatureConversion {
                    producer_slot: parent.source_slot,
                })
            {
                children.insert(identity.handle.source_slot, identity.handle);
            }
        }
        if children.len() == 3 && parent_reclaimed {
            break;
        }
    }
    assert_eq!(
        children.len(),
        3,
        "real large death creates its three generation-bound children"
    );
    assert!(
        parent_hidden && parent_reclaimed,
        "parent hides before following-pass reclaim"
    );

    let mut landed = BTreeSet::new();
    for _ in 0..160 {
        attack.step(PlayerCommand::default(), 0);
        let session = attack.simulation();
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "Night Army child landing dispatch failed: {:?}",
            session.dispatch_events()
        );
        for (&slot, child) in &children {
            let Some(SourceEntityView::Imported(row)) = session.source_arena().resolve(*child)
            else {
                panic!("split child {slot} must survive to landing");
            };
            assert_eq!((row.class, row.model), (5, 25));
            let identity = session
                .source_arena()
                .imported_creature_runtime_identity(slot, &BTreeMap::new())
                .expect("the real split child retains its semantic root");
            assert_eq!(identity.handle, *child);
            assert!(
                identity.living,
                "a live split child remains in the living-root consumer"
            );
            if row.record[74] >= 3 {
                landed.insert(slot);
            }
        }
        if landed.len() == children.len() {
            break;
        }
    }
    assert_eq!(
        landed.len(),
        children.len(),
        "all three small children land and enter their acting state"
    );
    let phases = children
        .iter()
        .map(|(&slot, child)| {
            let Some(SourceEntityView::Imported(row)) =
                attack.simulation().source_arena().resolve(*child)
            else {
                panic!("landed child {slot} lost its source generation");
            };
            (slot, row.phase)
        })
        .collect::<BTreeMap<_, _>>();
    attack.step(PlayerCommand::default(), 0);
    let session = attack.simulation();
    assert!(
        !session
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
        "Night Army post-landing dispatch failed: {:?}",
        session.dispatch_events()
    );
    for (&slot, child) in &children {
        let Some(SourceEntityView::Imported(row)) = session.source_arena().resolve(*child) else {
            panic!("landed child {slot} must receive its next source visit");
        };
        assert_eq!(
            row.phase,
            phases[&slot].wrapping_add(1),
            "child {slot} acts after landing"
        );
    }
    for child in children.values() {
        crate::native_world::area_damage::contact(
            attack.simulation_mut().source_arena_mut(),
            None,
            child.source_slot,
            u16::MAX,
            rival_slot,
        )
        .unwrap();
    }
    let mut hidden = BTreeSet::new();
    let mut reclaimed = BTreeSet::new();
    for _ in 0..160 {
        attack.step(PlayerCommand::default(), 0);
        let session = attack.simulation();
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "Night Army child death dispatch failed: {:?}",
            session.dispatch_events()
        );
        let arena = session.source_arena();
        for (&slot, child) in &children {
            match arena.resolve(*child) {
                Some(SourceEntityView::Imported(row)) if row.drawing_disabled() => {
                    let identity = arena
                        .imported_creature_runtime_identity(slot, &BTreeMap::new())
                        .expect("hidden child remains a source root until reclaim");
                    assert!(
                        !identity.living,
                        "hidden child is excluded from living roots"
                    );
                    hidden.insert(slot);
                }
                None if hidden.contains(&slot) => {
                    reclaimed.insert(slot);
                }
                _ => {}
            }
        }
        for row in arena
            .entities
            .iter()
            .filter(|row| row.active && row.class == 5 && row.model == 25)
        {
            if let Some(identity) =
                arena.imported_creature_runtime_identity(row.index, &BTreeMap::new())
            {
                assert!(
                    !children.keys().any(|slot| identity.origin
                        == (CreatureRootOrigin::CreatureConversion {
                            producer_slot: *slot
                        })),
                    "small children must not split again"
                );
            }
        }
        if reclaimed.len() == children.len() {
            break;
        }
    }
    assert_eq!(
        hidden.len(),
        children.len(),
        "all small children hide in their death pass"
    );
    assert_eq!(
        reclaimed.len(),
        children.len(),
        "all small children reclaim on a later pass"
    );
    eprintln!(
        "Night7 Army Cymmerian split, landed, hid and reclaimed: parent={parent:?}, children={children:?}"
    );
}

#[test]
fn level_ten_buildings_release_switch_group_into_cymmerian_guard() {
    use crate::PlayerCommand;
    use crate::campaign::source_arena::{CreatureRootOrigin, SourceEntityView};
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent};
    use crate::native_world::level2_dynamic_economy::{start_castle_damage_trace, take_castle_damage_trace};
    use std::collections::{BTreeMap, BTreeSet};

    let campaign = CampaignSession::open(
        mission_eighteen_root().join("level10"),
        CampaignSelection::ordinary(10).unwrap(),
    )
    .expect("real Level 10 campaign package");
    let mut session = campaign.simulation().clone();
    let player = session.state.entity_index;
    let owner = usize::from(session.players()[0].player_color_index());
    let switch = &session.entities()[1];
    assert_eq!(
        (switch.class, switch.model, switch.action, switch.source_id),
        (11, 32, 32, 8)
    );
    assert_eq!(session.runtime().stages[0].kind, 9);
    assert_eq!(session.runtime().objectives[owner].stage_selectors[0], 1);
    let initial = session.campaign_building_objective_targets(0);
    assert_eq!(
        initial.len(),
        6,
        "owner data starts with six standing chain-23 buildings"
    );
    let kept = (initial[0].1.x, initial[0].1.y);
    let expected_group = session
        .mission()
        .authored_features()
        .iter()
        .filter(|feature| {
            feature.index != 0
                && feature.index < 1200
                && feature.kind != 0
                && feature.activation_group == 8
        })
        .map(|feature| feature.index)
        .collect::<Vec<_>>();
    assert_eq!(expected_group.len(), 25);
    let mut damaged = BTreeSet::new();
    let mut only_kept = false;

    // Apply real source mailbox damage and allow the ordinary House callbacks
    // to construct each 23 -> 24 -> 28 successor. One House remains intact.
    for _ in 0..256 {
        let targets = session.campaign_building_objective_targets(0);
        if targets.iter().all(|(_, p)| (p.x, p.y) == kept) && !targets.is_empty() {
            only_kept = true;
            break;
        }
        for (slot, position) in targets {
            if (position.x, position.y) == kept {
                continue;
            }
            let ready = session
                .session
                .state
                .live
                .ordinary
                .houses()
                .iter()
                .any(|house| house.entity_index == slot && house.active && house.action == 52);
            if !ready {
                continue;
            }
            let handle = session
                .source_arena()
                .handle(slot)
                .expect("live House source handle");
            if damaged.insert((slot, handle.generation)) {
                assert!(
                    session
                        .session
                        .state
                        .live
                        .ordinary
                        .houses
                        .submit_source_damage(slot, u16::MAX, player)
                );
            }
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "ordinary House cleanup dispatch failed: {:?}",
            session.dispatch_events()
        );
        assert!(
            !session.dispatch_events().iter().any(|event| matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated { group: 8, .. }
            )),
            "a surviving source House must keep the objective switch inert"
        );
    }
    assert!(
        only_kept,
        "other actual House destruction chains must finish"
    );
    assert_eq!(session.runtime().objectives[owner].stage_selectors[0], 1);

    let mut activated = false;
    for _ in 0..256 {
        for (slot, _) in session.campaign_building_objective_targets(0) {
            let ready = session
                .session
                .state
                .live
                .ordinary
                .houses()
                .iter()
                .any(|house| house.entity_index == slot && house.active && house.action == 52);
            if !ready {
                continue;
            }
            let handle = session
                .source_arena()
                .handle(slot)
                .expect("live House source handle");
            if damaged.insert((slot, handle.generation)) {
                assert!(
                    session
                        .session
                        .state
                        .live
                        .ordinary
                        .houses
                        .submit_source_damage(slot, u16::MAX, player)
                );
            }
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "ordinary switch/group dispatch failed: {:?}",
            session.dispatch_events()
        );
        if let Some(indices) = session
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Mc2DispatchEvent::AuthoredGroupActivated {
                    source_slot: 1,
                    group: 8,
                    authored_indices,
                    ..
                } => Some(authored_indices),
                _ => None,
            })
        {
            assert_eq!(indices.as_ref(), expected_group.as_slice());
            assert_eq!(indices[0], 31);
            activated = true;
            break;
        }
    }
    assert!(
        activated,
        "naturally completed building objective must fire the real group-8 switch"
    );
    assert_eq!(session.runtime().objectives[owner].stage_selectors[0], 2);
    let empty_bindings = BTreeMap::new();
    let guard = session
        .entities()
        .iter()
        .find_map(|row| {
            let identity = session
                .source_arena()
                .imported_creature_runtime_identity(row.index, &empty_bindings)?;
            (identity.origin
                == CreatureRootOrigin::AuthoredRecord {
                    authored_record: 31,
                })
            .then_some(identity.handle)
        })
        .expect("source row31 produced its live generation-bound Cymmerian");
    let row = &session.entities()[usize::from(guard.source_slot)];
    assert_eq!((row.class, row.model, row.record[76]), (5, 25, 1));
    assert!(
        matches!(row.record[77], 2 | 10),
        "row31 uses its authored guard route"
    );
    // Pin actual current generations from this same unfiltered group before
    // examining their later trajectories; this is diagnostic only.
    let authored_group25: Vec<_> = session.mission().authored_features().iter()
        .filter(|feature| feature.kind == 5 && feature.subtype == 25 && feature.activation_group == 8)
        .filter_map(|feature| session.entities().iter().find_map(|row| {
            let identity = session.source_arena()
                .imported_creature_runtime_identity(row.index, &empty_bindings)?;
            (identity.origin == CreatureRootOrigin::AuthoredRecord { authored_record: feature.index })
                .then_some((feature.index, identity.handle))
        })).collect();
    let mut group_observations = Vec::new();

    // Place the actual player inside the guard's view cone; callbacks and
    // projectile allocation then run only through the ordinary scheduler.
    let position = row.position;
    let yaw = row.yaw;
    let candidates = [
        (2048_i16, 0_i16),
        (-2048, 0),
        (0, 2048),
        (0, -2048),
        (1448, 1448),
        (1448, -1448),
        (-1448, 1448),
        (-1448, -1448),
    ];
    let point = candidates
        .into_iter()
        .map(|(dx, dy)| Position {
            x: position.x.wrapping_add_signed(dx),
            y: position.y.wrapping_add_signed(dy),
            z: position.z,
        })
        .find(|point| {
            angle_distance(i32::from(yaw), heading_for_positions(position, *point))
                < RULES.target_half_angle
        })
        .expect("a bounded nearby position lies in the guard's source view cone");
    session.set_level2_entity_position(player, point).unwrap();
    // The source position setter does not change flight velocity. Keep the
    // declared one-time player entry still; all later recoil remains live.
    let entry_speed = (session.state.actual_speed, session.state.target_speed);
    session.state.actual_speed = 0;
    session.state.target_speed = 0;
    let mut shooter = None;
    let mut row31_blocked_death = false;
    let mut guard_observations = Vec::new();
    // The actual initial phase is1: scan20 acquires the target, then
    // pursuit at40 emits the first shot. The previous32-visit deadline
    // ended at33, before that second source cadence. Preserve both scans.
    let acquisition_and_attack_visits = 2 * usize::from(RULES.scan_period) + 1;
    for turn in 0..acquisition_and_attack_visits {
        let before = session.entities()[usize::from(guard.source_slot)].clone();
        // Diagnostic only: project the next source movement against a cloned
        // pre-turn terrain, keeping its scratch writes out of the live world.
        let projected_motion = (turn < 4).then(|| {
            let terrain = session.canonical_terrain().clone();
            crate::ground_creature_motion::step_ground_creature_motion(
                before.position, before.yaw, before.source_roll(), before.actual_speed,
                &terrain,
                crate::ground_creature_motion::GroundCreatureMotionConfig {
                    half_width: RULES.collision_half_width,
                    half_height: RULES.collision_half_height,
                    ground_clearance: RULES.ground_clearance,
                    descent: RULES.descent,
                    turn_limit: RULES.turn_response,
                    terrain_mask: RULES.terrain_mask,
                    maximum_tile_slope: RULES.maximum_slope,
                },
            )
        });
        let journal_start = session.effect_journal().cursor(0).next_sequence;
        mc2::start_attack_birth_trace();
        session.step_outer_turn(PlayerCommand::default(), 0);
        let attacks = mc2::take_attack_birth_trace();
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "ordinary guard/projectile dispatch failed: {:?}",
            session.dispatch_events()
        );
        let after = &session.entities()[usize::from(guard.source_slot)];
        row31_blocked_death |= session.source_arena().handle(guard.source_slot) == Some(guard)
            && before.life == 7500 && after.life == -1
            && before.record[98..104] == [0; 6] && after.record[98..104] == [0; 6]
            && after.control_flags & 0x40000 != 0
            && after.yaw == before.yaw.wrapping_add(1024) & 2047
            && before.position == after.position
            && projected_motion.is_some_and(|motion| motion.killed && motion.movement_blocked);
        if turn < 4 || turn % 8 == 0 {
            group_observations.push((turn, authored_group25.iter().map(|&(record, handle)| {
                (record, handle, session.source_arena().resolve(handle).map(|_| {
                    let row = &session.entities()[usize::from(handle.source_slot)];
                    (row.action, row.phase, row.position, row.life, row.control_flags)
                }))
            }).collect::<Vec<_>>()));
        }
        if guard_observations.len() < 16 && (turn < 4 || turn % 8 == 0 || before.action != after.action) {
            let packet = |row: &EntitySnapshot| (
                (row.action, row.phase, row.position, row.yaw, row.source_roll(), row.actual_speed),
                (row.life, row.max_life, row.control_flags,
                    i32::from_le_bytes(row.record[98..102].try_into().unwrap()),
                    u16::from_le_bytes(row.record[102..104].try_into().unwrap()),
                    u16::from_le_bytes(row.record[42..44].try_into().unwrap()),
                    row.source_word_96()),
            );
            guard_observations.push((turn,
                packet(&before),
                packet(after),
                (session.state.position, session.state.actual_speed, session.state.target_speed),
                (session.canonical_terrain().ground_height(before.position),
                    session.canonical_terrain().ground_height(after.position),
                    session.canonical_terrain().source_predicted_position()),
                projected_motion));
        }
        if shooter.is_none() {
            shooter = attacks.into_iter().find_map(|attack| {
                let &(record, handle) = authored_group25.iter().find(|(_, handle)| *handle == attack.actor)?;
                if attack.target != player || session.source_arena().handle(handle.source_slot) != Some(handle) {
                    return None;
                }
                let row = &session.entities()[usize::from(handle.source_slot)];
                if (row.class, row.model) != (5, 25) || row.life < 0 || row.drawing_disabled() {
                    return None;
                }
                let identity = session.source_arena()
                    .imported_creature_runtime_identity(handle.source_slot, &empty_bindings)?;
                if identity.handle != handle || identity.origin != (CreatureRootOrigin::AuthoredRecord { authored_record: record }) {
                    return None;
                }
                let audible = session.effect_journal().records().iter().filter(|event|
                    event.sequence >= journal_start
                ).any(|event| {
                    matches!(event.effect,
                        crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                            entity_index, player_index: -1, sound_id: 37, ..
                        }) if entity_index == handle.source_slot
                    ) && event.audio.is_some_and(|audio| audio.command.audible)
                });
                audible.then_some((record, handle, attack.projectile))
            });
        }
        if shooter.is_some() && row31_blocked_death {
            break;
        }
    }
    assert!(
        row31_blocked_death,
        "the original row31 source-blocked death must remain observed: observations={guard_observations:?}"
    );
    let (shooter_record, guard, projectile) = shooter.expect(
        "an original authored group8 Cymmerian must actually fire at the player and publish its own audible source37"
    );
    assert_eq!(session.source_arena().handle(guard.source_slot), Some(guard));
    assert!(authored_group25.contains(&(shooter_record, guard)));

    // One fixed player-side Castle placement before the parent is hit. The
    // previous encounter placed it at the split and observed child404 travel
    // north for the source's 52 protected visits (~110 units per visit),
    // reaching state5 about 6,000 units away. This control does not track or
    // change a child after birth; the real constructor and terrain still decide.
    let mut castle_position = session.entities()[usize::from(guard.source_slot)].position;
    castle_position.y = castle_position.y.wrapping_sub(6_000);
    let castle = session
        .economy_mut()
        .build_castle(castle_position, player)
        .expect("Castle combat prerequisite for the actual authored split");
    let castle_handle = session.source_arena().handle(castle).expect("current Castle generation");
    let phase = session.entities()[usize::from(guard.source_slot)].phase;
    crate::native_world::area_damage::contact(
        session.source_arena_mut(),
        None,
        guard.source_slot,
        u16::MAX,
        player,
    )
    .unwrap();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row = &session.entities()[usize::from(guard.source_slot)];
    assert_eq!((row.action, row.life), (204, 0));
    assert_eq!(row.phase, phase.wrapping_add(1));
    assert!(
        session
            .source_arena()
            .imported_creature_runtime_identity(guard.source_slot, &empty_bindings,)
            .unwrap()
            .living,
        "source28C60 retains the zero-life root until split"
    );
    assert!(session.source_arena().rebuilt_class5_model_slots()[25].contains(&guard.source_slot));

    let mut children = BTreeMap::new();
    let mut landed = BTreeSet::new();
    let mut parent_hidden = false;
    let mut parent_reclaimed = false;
    let mut small_hit = false;
    let mut castle_contacts = Vec::new();
    let mut castle_consumed = None;
    let mut all_castle_consumptions = Vec::new();
    let mut castle_observations = Vec::new();
    for turn in 0..320 {
        start_castle_damage_trace(castle_handle);
        mc2::start_castle_contact_trace();
        session.step_outer_turn(PlayerCommand::default(), 0);
        castle_contacts.extend(mc2::take_castle_contact_trace());
        let consumed = take_castle_damage_trace();
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "authored split/combat dispatch failed: {:?}",
            session.dispatch_events()
        );
        let arena = session.source_arena();
        match arena.resolve(guard) {
            Some(SourceEntityView::Imported(row)) => parent_hidden |= row.drawing_disabled(),
            None => parent_reclaimed |= parent_hidden,
            _ => panic!("authored Cymmerian was replaced before natural reclaim"),
        }
        for row in arena
            .entities
            .iter()
            .filter(|row| row.active && row.class == 5 && row.model == 25)
        {
            let Some(identity) =
                arena.imported_creature_runtime_identity(row.index, &empty_bindings)
            else {
                continue;
            };
            if identity.origin
                == (CreatureRootOrigin::CreatureConversion {
                    producer_slot: guard.source_slot,
                })
            {
                children.insert(row.index, identity.handle);
                assert_eq!(
                    row.action, 200,
                    "small body stays in its source-specific action200"
                );
                assert!(identity.living);
                assert_eq!(
                    word(row, 46),
                    row.subspell_index,
                    "small countdown raw bytes stay current"
                );
                if row.record[74] >= 3 {
                    landed.insert(row.index);
                }
            }
        }
        let target = arena
            .economy
            .castle_at(castle)
            .expect("Castle survives the bounded contact check");
        // A child before the Castle's numeric slot can write and have its
        // mailbox consumed in this same pass. Observe the actual source write
        // rather than requiring its mailbox to remain pending after the turn.
        small_hit |= castle_contacts.iter().any(|writer|
            writer.castle == castle_handle && writer.damage == 60
                && children.values().any(|child| *child == writer.actor)
                && landed.contains(&writer.actor.source_slot)
                && writer.mailbox.0 >= 60 && writer.mailbox.1 == writer.source_id
                && writer.source_id != 0);
        if turn < 4 || turn % 32 == 0 {
            let current_a4 = crate::native_world::source_target::creature25_castle_slot(
                arena,
                crate::native_world::source_target::Context {
                    spells: Some(session.shared_spells()), ..Default::default()
                },
                player,
            );
            let child_states = children.values().map(|&child| (child,
                arena.resolve(child).map(|_| {
                    let row = &arena.entities[usize::from(child.source_slot)];
                    (row.action, row.phase, row.record[74], row.position, row.life, row.source_word_96())
                }))).collect::<Vec<_>>();
            castle_observations.push((turn, current_a4, arena.handle(castle),
                (target.life, target.pending_damage, target.pending_attacker_entity_index),
                session.session.state.live.player_life, child_states));
        }
        all_castle_consumptions.extend(consumed.iter().copied().map(|receipt| (turn, receipt)));
        for receipt in consumed {
            if receipt.target != castle_handle { continue; }
            let Some(writer) = castle_contacts.iter().rev().find(|writer|
                writer.castle == castle_handle && writer.event_cursor <= receipt.event_cursor
            ) else { continue };
            if !children.values().any(|child| *child == writer.actor)
                || arena.handle(writer.actor.source_slot) != Some(writer.actor)
                || writer.mailbox != receipt.mailbox_before
                || writer.damage != 60
                || writer.source_id != receipt.mailbox_before.1
            {
                continue;
            }
            assert_eq!(arena.handle(castle), Some(castle_handle));
            assert_eq!(writer.source_id, arena.entities[usize::from(writer.actor.source_slot)].source_id);
            assert!(receipt.mailbox_before.0 >= 60);
            assert_eq!(receipt.mailbox_after.1, 0);
            assert_eq!(receipt.life_after, receipt.life_before.wrapping_sub(receipt.mailbox_before.0));
            assert!(matches!(arena.economy.events().get(receipt.event_cursor),
                Some(Level2EconomyEvent::DamageApplied { target_entity_index, attacker_entity_index, damage, lethal: false })
                if *target_entity_index == castle && *attacker_entity_index == writer.source_id && *damage == receipt.mailbox_before.0));
            castle_consumed = Some((turn, *writer, receipt));
        }
        if children.len() == 3 && landed.len() == 3 && parent_reclaimed && small_hit && castle_consumed.is_some() {
            break;
        }
    }
    assert_eq!(
        children.len(),
        3,
        "the ordinary large death creates three real small bodies"
    );
    assert_eq!(
        landed.len(),
        3,
        "all small bodies finish their unshortened landing"
    );
    assert!(parent_hidden && parent_reclaimed);
    assert!(
        small_hit,
        "an original landed small body must write the actual Castle mailbox: record={shooter_record} parent={guard:?} castle_position={castle_position:?} castle={castle_handle:?} contacts={castle_contacts:?} consumptions={all_castle_consumptions:?} observations={castle_observations:?}"
    );
    assert!(castle_consumed.is_some(),
        "one original child must write the actual Castle mailbox consumed by its scheduled callback: record={shooter_record} parent={guard:?} castle_position={castle_position:?} castle={castle_handle:?} contacts={castle_contacts:?} consumptions={all_castle_consumptions:?} observations={castle_observations:?}");

    let credit_before_children = session.economy().authored_bee_kill_census(player);
    for child in children.values() {
        crate::native_world::area_damage::contact(
            session.source_arena_mut(),
            None,
            child.source_slot,
            u16::MAX,
            player,
        )
        .unwrap();
    }
    let mut hidden = BTreeSet::new();
    let mut reclaimed = BTreeSet::new();
    for _ in 0..64 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "authored small-body death dispatch failed: {:?}",
            session.dispatch_events()
        );
        let arena = session.source_arena();
        for (&slot, &child) in &children {
            match arena.resolve(child) {
                Some(SourceEntityView::Imported(row)) if row.drawing_disabled() => {
                    assert!(
                        !arena
                            .imported_creature_runtime_identity(slot, &empty_bindings)
                            .unwrap()
                            .living
                    );
                    hidden.insert(slot);
                }
                None if hidden.contains(&slot) => {
                    reclaimed.insert(slot);
                }
                _ => {}
            }
        }
        assert!(
            !arena
                .entities
                .iter()
                .filter(|row| row.active && row.class == 5 && row.model == 25)
                .filter_map(
                    |row| arena.imported_creature_runtime_identity(row.index, &empty_bindings)
                )
                .any(|identity| children.keys().any(|&slot| identity.origin
                    == (CreatureRootOrigin::CreatureConversion {
                        producer_slot: slot
                    }))),
            "small children must not create another split generation"
        );
        if reclaimed.len() == 3 {
            break;
        }
    }
    assert_eq!(
        (hidden.len(), reclaimed.len()),
        (3, 3),
        "small death hides before following-pass reclaim"
    );
    assert_eq!(
        session.economy().authored_bee_kill_census(player),
        credit_before_children.wrapping_add(3),
        "each actual small-body prekill contributes exactly one A4 creature credit"
    );
    let frozen = session.source_arena().rebuilt_class5_model_slots();
    assert!(!frozen[25].contains(&guard.source_slot));
    assert!(
        children.keys().all(|slot| !frozen[25].contains(slot)),
        "the source absence consumer drops the entire retired lineage"
    );
    eprintln!(
        "Level10 authored Cymmerian combat/split/landing/Castle damage/death/reclaim: record={shooter_record}, parent={guard:?}, projectile={projectile:?}, children={children:?}"
    );
}

#[test]
fn army_action207_consumes_parent_timer_and_stage13_expiry_products() {
    let (mut arena, mut runtime, mut terrain, particles, parent_slot, owner_id, position) =
        summoned_source_parts();
    let holder_slot = arena
        .allocate_source_slot()
        .expect("reserve the Army holder source slot");
    let slot = mc2::allocate_summoned(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        parent_slot,
        owner_id,
        holder_slot,
    )
    .unwrap()
    .unwrap()
    .source_slot;

    // sub_16FC0 selects from the pass-start class-3 chain without a life or
    // visibility test; the selected record is validated on the next visit.
    let mut selected = arena.clone();
    let mut selected_runtime = runtime.clone();
    let mut selected_terrain = terrain.clone();
    let target_slot = selected.allocate_source_slot_with_terrain(&mut selected_terrain).unwrap();
    let mut target = crate::native_world::cave_features::new_event(target_slot, 5, position);
    target.class = 3;
    target.model = 0;
    target.record[67] = 3;
    target.source_id = owner_id.wrapping_add(1);
    target.set_life(-1);
    selected.entities[usize::from(target_slot)] = target;
    selected.entities[usize::from(slot)].set_phase(0);
    mc2::step_row(
        &mut selected, &mut selected_runtime, &mut configured_player_headers(), &mut selected_terrain, &particles, slot,
        &[target_slot], &family_with_cymmerian(slot),
        crate::native_world::source_target::Context::default(),
    ).unwrap();
    assert_eq!(u16::from_le_bytes(selected.entities[usize::from(slot)].record[154..156].try_into().unwrap()), target_slot);
    assert_eq!(u16::from_le_bytes(selected.entities[usize::from(slot)].record[50..52].try_into().unwrap()), 249);

    arena.entities[usize::from(slot)].set_phase(1);
    let families = family_with_cymmerian(slot);
    let effects = mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut configured_player_headers(),
        &mut terrain,
        &particles,
        slot,
        &[parent_slot],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert!(
        !effects.stage_callback,
        "the leaf consumes action207 itself"
    );
    assert_eq!(
        u16::from_le_bytes([
            arena.entities[usize::from(slot)].record[50],
            arena.entities[usize::from(slot)].record[51],
        ]),
        245,
        "StageVar13 decrements the 250 timer, then applies its no-target four-tick penalty"
    );

    arena.entities[usize::from(slot)].record[44..46].copy_from_slice(&u16::MAX.to_le_bytes());
    arena.entities[usize::from(slot)].record[50..52].copy_from_slice(&1_u16.to_le_bytes());
    let effects = mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut configured_player_headers(),
        &mut terrain,
        &particles,
        slot,
        &[parent_slot],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert!(
        !effects.stage_callback,
        "expired callback is not returned for root replay"
    );
    assert!(
        arena.entities[usize::from(slot)].control_flags
            & crate::native_world::ENTITY_DRAW_DISABLED_BIT
            != 0,
        "invalid parent expires and hides the summoned child"
    );
    assert!(
        arena
            .entities
            .iter()
            .any(|row| row.active && row.class == 10 && row.model == 73)
    );
    assert!(
        arena
            .entities
            .iter()
            .any(|row| row.active && row.class == 10 && row.model == 0)
    );
}

#[test]
fn ordinary_step_row_materializes_one_cymmerian_class9_model0_attack() {
    for light_enabled in [true, false] {
        let (mut arena, mut runtime, mut terrain, particles, _parent_slot, _owner_id, _position) =
            summoned_source_parts();
        arena.dynamic_lighting_enabled = light_enabled;
        let target = arena
            .entities
            .iter()
            .find(|row| row.active && row.class == 3 && row.model == 0 && row.life > 0)
            .expect("Mission18 retains a live class3/model0 target")
            .clone();
        let slot = mc2::allocate_authored(
            &mut arena,
            &mut runtime,
            &mut terrain,
            &particles,
            target.position,
            RULES.max_life,
            None,
            0,
        )
        .unwrap()
        .unwrap();
        {
            let row = &mut arena.entities[usize::from(slot)];
            row.set_source_action(202);
            row.set_phase(RULES.scan_period);
            row.record[154..156].copy_from_slice(&target.index.to_le_bytes());
        }
        let families = family_with_cymmerian(slot);
        let effects = mc2::step_row(
            &mut arena,
            &mut runtime,
            &mut configured_player_headers(),
            &mut terrain,
            &particles,
            slot,
            &[target.index],
            &families,
            crate::native_world::source_target::Context::default(),
        )
        .unwrap();
        assert!(
            effects.attack.is_none(),
            "the adapter consumes the attack intent"
        );
        let projectile = arena
            .economy
            .fireball_projectiles()
            .iter()
            .find(|projectile| {
                projectile.origin == crate::native_world::FireballProjectileOrigin::Cymmerian
            })
            .expect("action202 publishes a real Fireball carrier");
        assert_eq!((projectile.source_class, projectile.source_model), (9, 0));
        assert_eq!(
            (
                projectile.target_source_entity_index,
                projectile.target_class,
                projectile.target_model
            ),
            (target.index, target.class, target.model)
        );
        assert_eq!(projectile.source_subspell_index, RULES.projectile_damage);
        assert_eq!(projectile.control_flags & 0x0008_0000 != 0, light_enabled);
        let constructor_position = arena.entities[usize::from(slot)].position;
        let mut launched_position = constructor_position;
        launched_position.z = launched_position
            .z
            .wrapping_add(arena.entities[usize::from(slot)].array_fov());
        assert_eq!(projectile.position, launched_position);
        let projectile_slot = projectile.source_entity_index;
        let linked_lights = arena
            .dynamic_lighting
            .active_slots()
            .filter(|(_, light)| light.linked && light.source_entity_index == projectile_slot)
            .count();
        assert_eq!(
            linked_lights,
            if light_enabled { 1 } else { 0 },
            "Night Fireball light admission"
        );
        if light_enabled {
            let (_, light) = arena
                .dynamic_lighting
                .active_slots()
                .find(|(_, light)| light.linked && light.source_entity_index == projectile_slot)
                .unwrap();
            assert_eq!(light.position, constructor_position);
        }
    }
}

#[test]
fn ordinary_split_publishes_generation_bound_cymmerian_children() {
    let (mut arena, mut runtime, mut terrain, particles, attacker_slot, _owner_id, position) =
        summoned_source_parts();
    let parent_slot = mc2::allocate_authored(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        RULES.max_life,
        None,
        0,
    )
    .unwrap()
    .unwrap();
    // Deliver damage through the real source mailbox; the ordinary seek
    // callback must select the death action before the split is visited.
    assert_eq!(arena.entities[usize::from(parent_slot)].action, 201);
    crate::native_world::area_damage::contact(
        &mut arena,
        None,
        parent_slot,
        u16::MAX,
        attacker_slot,
    )
    .unwrap();
    let families = family_with_cymmerian(parent_slot);
    mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut configured_player_headers(),
        &mut terrain,
        &particles,
        parent_slot,
        &[],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    // The old negative-life oracle omitted sub28C30's wrapper. The source
    // selects204 and then restores0, retaining the next frozen living root.
    assert_eq!(arena.entities[usize::from(parent_slot)].life, 0);
    assert_eq!(word(&arena.entities[usize::from(parent_slot)], 12), 0);
    assert!(arena.rebuilt_class5_model_presence()[usize::from(SOURCE_MODEL)]);
    assert!(
        arena
            .imported_creature_runtime_identity(parent_slot, &std::collections::BTreeMap::new())
            .unwrap()
            .living
    );
    assert_eq!(arena.entities[usize::from(parent_slot)].action, 204);
    let parent_id = arena.entities[usize::from(parent_slot)].source_id;
    let mut exhausted = arena.clone();
    let mut exhausted_runtime = runtime.clone();
    let mut exhausted_terrain = terrain.clone();
    exhausted.finish_source_group();
    while exhausted.economy.free_slot_count() != 0 {
        exhausted
            .allocate_source_slot_with_terrain(&mut exhausted_terrain)
            .expect("consume the actual source free stack");
    }
    let parent_rng = exhausted.entities[usize::from(parent_slot)].entity_rng;
    let exhausted_population = exhausted.economy.source_creature_population;
    assert!(exhausted.entities[usize::from(parent_slot)].mana > 0);
    let families = family_with_cymmerian(parent_slot);
    mc2::step_row(
        &mut exhausted,
        &mut exhausted_runtime,
        &mut configured_player_headers(),
        &mut exhausted_terrain,
        &particles,
        parent_slot,
        &[],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert_eq!(exhausted.entities[usize::from(parent_slot)].action, 205);
    assert_eq!(
        exhausted.economy.source_creature_population,
        exhausted_population + 3,
        "source global population increases by three even when split allocation fails"
    );
    assert_eq!(
        exhausted.entities[usize::from(parent_slot)].entity_rng,
        crate::native_world::level2_next_rng16(parent_rng),
        "the failed mana allocation still consumes the source RNG draw"
    );
    assert!(
        !exhausted.economy.events().iter().any(|event| matches!(
            event,
            crate::native_world::Level2EconomyEvent::DeathBurstSpawned {
                parent_entity_index, ..
            } if *parent_entity_index == parent_slot
        )),
        "failed source allocation does not publish a split burst"
    );

    // Two free source slots exercise the partially allocated branch: the
    // third NewEvent fails, so the source does not award its remainder.
    let mut partial = arena.clone();
    let mut partial_runtime = runtime.clone();
    let mut partial_terrain = terrain.clone();
    partial.finish_source_group();
    for row in &mut partial.entities {
        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    }
    assert!(partial.economy.typed_replacement_candidate().is_none());
    partial.rebuild_source_free_arena();
    while partial.economy.free_slot_count() > 2 {
        partial
            .allocate_source_slot_with_terrain(&mut partial_terrain)
            .expect("reserve the source free stack down to two slots");
    }
    partial.entities[usize::from(parent_slot)].set_mana(100);
    let population_before_partial = partial.economy.source_creature_population;
    mc2::step_row(
        &mut partial,
        &mut partial_runtime,
        &mut configured_player_headers(),
        &mut partial_terrain,
        &particles,
        parent_slot,
        &[],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    let partial_children = partial
        .entities
        .iter()
        .filter_map(|row| {
            let identity = partial.imported_creature_runtime_identity(
                row.index,
                &std::collections::BTreeMap::new(),
            )?;
            (identity.origin
                == (crate::campaign::source_arena::CreatureRootOrigin::CreatureConversion {
                    producer_slot: parent_slot,
                }))
            .then_some(row.mana)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        partial_children,
        [33, 33],
        "a failed final attempt drops the source remainder"
    );
    assert_eq!(
        partial.economy.source_creature_population,
        population_before_partial + 3,
        "source population increments by three despite partial allocation"
    );

    // Exercise NewEvent's frozen fallback with this actual source address.
    // Model 25 does not normally set the replaceable flag; this is a bounded
    // allocator control, not a claim that an ordinary death takes this path.
    let mut replaced = arena.clone();
    let mut replaced_runtime = runtime.clone();
    let mut replaced_terrain = terrain.clone();
    for row in &mut replaced.entities {
        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    }
    assert!(replaced.economy.typed_replacement_candidate().is_none());
    let parent = &mut replaced.entities[usize::from(parent_slot)];
    parent.set_control_flags(parent.control_flags | crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    let original = replaced.handle(parent_slot).unwrap();
    replaced.rebuild_source_free_arena();
    while replaced.economy.free_slot_count() != 0 {
        replaced
            .allocate_source_slot_with_terrain(&mut replaced_terrain)
            .expect("reserve the real free stack before the frozen fallback");
    }
    mc2::step_row(
        &mut replaced,
        &mut replaced_runtime,
        &mut configured_player_headers(),
        &mut replaced_terrain,
        &particles,
        parent_slot,
        &[],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert!(replaced.resolve(original).is_none());
    let sphere = replaced
        .economy
        .spheres()
        .iter()
        .find(|sphere| sphere.active && sphere.source_entity_index == parent_slot)
        .expect("mana conversion replaced the dying source at its actual address");
    assert_eq!(
        sphere.action, 205,
        "the split tail writes the current source address"
    );
    assert_eq!(sphere.owner_entity_index, 0);
    assert!(
        !replaced.economy.events().iter().any(|event| matches!(
            event,
            crate::native_world::Level2EconomyEvent::DeathBurstSpawned {
                parent_entity_index, ..
            } if *parent_entity_index == parent_slot
        )),
        "the exhausted frozen list cannot manufacture a later burst"
    );
    let existing = arena
        .entities
        .iter()
        .filter(|row| row.active)
        .map(|row| row.index)
        .collect::<std::collections::BTreeSet<_>>();
    let families = family_with_cymmerian(parent_slot);

    let population = arena.economy.source_creature_population;
    mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut configured_player_headers(),
        &mut terrain,
        &particles,
        parent_slot,
        &[],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert_eq!(
        arena.economy.source_creature_population,
        population + 3,
        "large source death increases the global population by three"
    );

    let children = arena
        .entities
        .iter()
        .filter(|row| {
            row.active
                && row.class == SOURCE_CLASS
                && row.model == SOURCE_MODEL
                && row.parameter_relation == Some(SPLIT_DESCRIPTOR)
                && !existing.contains(&row.index)
        })
        .map(|row| row.index)
        .collect::<Vec<_>>();
    assert_eq!(children.len(), usize::from(RULES.split_count));
    let burst_slot = arena
        .economy
        .events()
        .iter()
        .find_map(|event| match event {
            crate::native_world::Level2EconomyEvent::DeathBurstSpawned {
                source_entity_index,
                parent_entity_index,
                id_entity_index,
            } if *parent_entity_index == parent_slot && *id_entity_index == parent_id => {
                Some(*source_entity_index)
            }
            _ => None,
        })
        .expect("the actual split publishes its source death burst");
    let burst = &arena.entities[usize::from(burst_slot)];
    assert_eq!((burst.class, burst.model, burst.action), (10, 1, 1));
    assert_eq!(burst.source_id, parent_id);
    assert_eq!(arena.entities[usize::from(parent_slot)].action, 205);
    let denominator = arena.economy.source_creature_population;
    let shared = level18_shared_player_state();
    let local_slot = u16::from(runtime.local_player_slot);
    let local_entity = shared.players[usize::from(local_slot)].entity_index;
    assert_eq!(local_entity, attacker_slot);
    assert_eq!(word(&arena.entities[usize::from(parent_slot)], 40), local_entity);
    let mut headers = crate::campaign::source_player_result::SourcePlayerResult::source_level_start(local_slot);
    let context = crate::native_world::source_target::Context { spells: Some(&shared), ..Default::default() };
    let mut missing_arena = arena.clone();
    let mut missing_runtime = runtime.clone();
    let mut missing_terrain = terrain.clone();
    let mut missing_headers = headers;
    let before_missing = missing_arena.entities[usize::from(parent_slot)].clone();
    assert!(mc2::step_row(&mut missing_arena, &mut missing_runtime, &mut missing_headers,
        &mut missing_terrain, &particles, parent_slot, &[], &families, Default::default()).is_err());
    assert_eq!(missing_arena.entities[usize::from(parent_slot)], before_missing);
    assert_eq!(missing_headers.raw_headers()[usize::from(local_slot) + 1], 0);
    // Separate configured nonmatching-killer control of the same large205
    // source callback. It may hide, but must not write the neighboring header.
    let mut other_arena = arena.clone();
    let mut other_runtime = runtime.clone();
    let mut other_terrain = terrain.clone();
    let mut other_headers = headers;
    put(&mut other_arena.entities[usize::from(parent_slot)], 40, local_entity.wrapping_add(1));
    mc2::step_row(&mut other_arena, &mut other_runtime, &mut other_headers, &mut other_terrain,
        &particles, parent_slot, &[], &families, context).unwrap();
    assert_eq!(other_headers.raw_headers()[usize::from(local_slot) + 1], 0);
    assert!(other_arena.entities[usize::from(parent_slot)].drawing_disabled());
    mc2::step_row(&mut arena, &mut runtime, &mut headers, &mut terrain,
    &particles,
    parent_slot,
    &[],
    &families,
    context,)
    .unwrap();
    assert_eq!(headers.raw_headers()[usize::from(local_slot) + 1], 1,
        "large source205 updates the neighboring header before hiding");
    assert_eq!(
        arena.economy.source_creature_population, denominator,
        "source28EC0 hides the large body without reducing the global denominator"
    );
    assert!(arena.entities[usize::from(parent_slot)].drawing_disabled());
    for child_slot in children {
        let identity = arena
            .imported_creature_runtime_identity(child_slot, &std::collections::BTreeMap::new())
            .expect("split child has generation-bound Cymmerian identity");
        assert_eq!(identity.handle.source_slot, child_slot);
        assert_eq!(
            identity.origin,
            crate::campaign::source_arena::CreatureRootOrigin::CreatureConversion {
                producer_slot: parent_slot
            }
        );
    }
}

#[test]
fn special_castle_contact_commits_damage_inside_cymmerian_adapter() {
    let (mut arena, mut runtime, mut terrain, particles, parent_slot, _owner_id, position) =
        summoned_source_parts();
    let built_castle_slot = arena
        .economy
        .build_castle(position, parent_slot, &terrain)
        .expect("source Castle constructor for Mission18 owner");
    let castle = arena
        .economy
        .castle()
        .expect("source Castle remains live after construction");
    assert_eq!(castle.source_entity_index, built_castle_slot);
    assert_eq!((castle.source_class, castle.source_model), (3, 2));
    let player_slot = castle.owner_entity_index;
    let player = arena.entities[usize::from(player_slot)].clone();
    // This configured adapter check uses the actual owner package's A4
    // carrier, with the Castle constructed above bound in its current word.
    let mut shared = level18_shared_player_state();
    match player.special_relation {
        crate::native_world::EntitySpecialRelation::PlayerState(index) => {
            shared.players[usize::from(index)].record[42..44].copy_from_slice(&built_castle_slot.to_le_bytes());
        }
        crate::native_world::EntitySpecialRelation::GlobalSpecialState => {
            shared.global_special_state.castle_entity_index = built_castle_slot;
        }
        other => panic!("source player lacks configured A4 carrier: {other:?}"),
    }
    let slot = mc2::allocate_authored(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        castle.position,
        RULES.max_life,
        None,
        0,
    )
    .unwrap()
    .unwrap();
    {
        let row = &mut arena.entities[usize::from(slot)];
        row.set_source_action(200);
        row.record[74] = 6;
        row.record[154..156].copy_from_slice(&player_slot.to_le_bytes());
    }
    let families = family_with_cymmerian(slot);
    let previous_damage = castle.pending_damage;
    assert_eq!(
        castle.pending_attacker_entity_index, 0,
        "Mission18 Castle contact mailbox starts clear"
    );
    let _ = mc2::step_row(&mut arena, &mut runtime, &mut configured_player_headers(), &mut terrain,
    &particles,
    slot,
    &[player_slot],
    &families,
    crate::native_world::source_target::Context { spells: Some(&shared), ..Default::default() },)
    .unwrap();
    let after = arena.economy.castle().expect("Castle remains live");
    assert_eq!(after.pending_damage, previous_damage + 60);
    assert_eq!(
        after.pending_attacker_entity_index,
        arena.entities[usize::from(slot)].source_id
    );
    assert!(player.life > 0);

    let latched_damage = after.pending_damage;
    {
        let row = &mut arena.entities[usize::from(slot)];
        row.set_position(castle.position);
        row.record[74] = 7;
    }
    let _ = mc2::step_row(&mut arena, &mut runtime, &mut configured_player_headers(), &mut terrain,
    &particles,
    slot,
    &[player_slot],
    &families,
    crate::native_world::source_target::Context { spells: Some(&shared), ..Default::default() },)
    .unwrap();
    assert_eq!(
        arena.economy.castle().unwrap().pending_damage,
        latched_damage,
        "a latched Castle attacker replaces rather than adds the Cymmerian hit"
    );
    crate::native_world::area_damage::contact(
        &mut arena, None, castle.source_entity_index, 9, 0,
    ).unwrap();
    assert_eq!(arena.economy.castle().unwrap().pending_damage, 9);
    crate::native_world::area_damage::contact(
        &mut arena, None, castle.source_entity_index, 5, 0,
    ).unwrap();
    let zero_owner = arena.economy.castle().unwrap();
    assert_eq!(
        (
            zero_owner.pending_damage,
            zero_owner.pending_attacker_entity_index
        ),
        (14, 0)
    );
}

#[test]
fn army_ring_reports_none_when_the_shared_source_allocator_is_exhausted() {
    let (mut arena, mut runtime, mut terrain, particles, parent_slot, owner_id, position) =
        summoned_source_parts();
    while arena.allocate_source_slot().is_some() {}
    let counter_at = crate::native_world::RUNTIME_BYTES + usize::from(SOURCE_MODEL);
    let counter = runtime.bytes[counter_at];

    let result = mc2::allocate_summoned(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        parent_slot,
        owner_id,
        parent_slot,
    )
    .unwrap();

    assert_eq!(result, None);
    assert_eq!(runtime.bytes[counter_at], counter);
}
