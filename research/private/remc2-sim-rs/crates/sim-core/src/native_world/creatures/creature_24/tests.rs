use super::*;

#[test]
fn zyggogg_authored_creature_objectives_produce_the_real_exit() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::campaign::source_arena::{CreatureRootOrigin, SourceEntityHandle};
    use crate::game_session::{GameCommand, GameCompletionStatus, GameSession};
    use crate::level_definition::{CreatureKind, ObjectiveCondition};
    use crate::native_world::{Mc2DispatchEvent, area_damage};
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    type Game = GameSession<crate::campaign::backend::Mc2CampaignBackend>;
    fn sim(game: &Game) -> &crate::native_world::Mc2Simulation {
        game.backend().compatibility_campaign().simulation()
    }
    fn sim_mut(game: &mut Game) -> &mut crate::native_world::Mc2Simulation {
        game.backend_mut().compatibility_campaign_mut_for_test().simulation_mut()
    }
    fn step(game: &mut Game) {
        game.advance(game.identity().guard(), GameCommand::default()).unwrap();
        assert!(sim(game).dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "Zyg Gogg source dispatch: {:?}", sim(game).dispatch_events());
    }
    fn contact(game: &mut Game, player: u16, handle: SourceEntityHandle) {
        area_damage::contact(sim_mut(game).source_arena_mut(), None, handle.source_slot, u16::MAX, player).unwrap();
    }

    let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
    let package = std::env::var_os("MC_LEVEL15_PACKAGE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("mc-bhq6-ordinary-20261025-zclu5fjj/level15"));
    let cache = std::env::var_os("MC_LEVEL15_OWNER_CACHE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("campaign-original-environments/level15"));
    let definition = load_mc2_campaign(&package, &crate::game_definition::default_catalog().unwrap()).unwrap();
    let mut game = GameSession::open(&Mc2CampaignAdapter, std::sync::Arc::new(definition), 2215,
        Mc2CampaignInputs::with_owner_cache(&package, &cache)).unwrap();
    let objectives = game.definition().level_definition().unwrap().objectives();
    assert_eq!(objectives.len(), 2);
    assert!(matches!(objectives[0].condition(), ObjectiveCondition::NoLivingCreatures(CreatureKind::Cymmerian)));
    assert!(matches!(objectives[1].condition(), ObjectiveCondition::NoLivingCreatures(CreatureKind::Creature24)));
    let player = sim(&game).state.entity_index;
    let owner = usize::from(sim(&game).state.player_color_index);
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[..2], [1, 1]);
    let initial = sim(&game).source_arena().rebuilt_class5_model_slots()[25].iter()
        .map(|slot| sim(&game).source_arena().handle(*slot).unwrap()).collect::<Vec<_>>();
    assert_eq!(initial.len(), 9, "the actual reset has nine model25 roots");
    let expected_group10 = sim(&game).mission().authored_features().iter()
        .filter(|row| row.index != 0 && row.kind != 0 && row.activation_group == 10)
        .map(|row| row.index).collect::<Vec<_>>();
    assert_eq!(expected_group10.len(), 125);
    let mut seen25 = initial.clone();
    let mut struck25 = BTreeSet::new();
    let mut split = false;
    let mut wave24 = Vec::<SourceEntityHandle>::new();
    let mut group10 = 0;
    // Selected arrived contacts act on the source roots and split descendants;
    // ordinary landing protection, callbacks, frozen census and reclaim run.
    for _ in 0..2048 {
        split |= sim(&game).creature_runtime_census().roots_of_kind(CreatureKind::Cymmerian)
            .any(|row| matches!(row.origin, CreatureRootOrigin::CreatureConversion { .. }));
        let targets = sim(&game).entities().iter().filter(|row| row.active && row.class == 5 && row.model == 25)
            .map(|row| (sim(&game).source_arena().handle(row.index).unwrap(),
                row.life >= 0 && (row.action != 200 || row.record_bytes()[74] >= 3))).collect::<Vec<_>>();
        for (handle, vulnerable) in targets {
            if !seen25.contains(&handle) { seen25.push(handle); }
            if vulnerable && struck25.insert((handle.source_slot, handle.generation)) { contact(&mut game, player, handle); }
        }
        step(&mut game);
        for event in sim(&game).dispatch_events() {
            if let Mc2DispatchEvent::AuthoredGroupActivated { group: 10, authored_indices, .. } = event {
                assert_eq!(authored_indices.as_ref(), expected_group10);
                group10 += 1;
                let roots = sim(&game).creature_runtime_census().roots_of_kind(CreatureKind::Creature24)
                    .filter_map(|root| match root.origin {
                        CreatureRootOrigin::AuthoredRecord { authored_record }
                            if expected_group10.contains(&authored_record) => Some((authored_record, root.handle)),
                        _ => None,
                    }).collect::<Vec<_>>();
                assert_eq!(roots.len(), 54, "actual group10 must allocate every model24 root");
                assert!(roots.iter().any(|(row, _)| *row == 630));
                wave24.extend(roots.into_iter().map(|(_, handle)| handle));
            }
        }
        if group10 != 0 { break; }
    }
    assert_eq!(group10, 1, "row261 must publish the actual stage0 wave");
    assert!(split && seen25.len() > initial.len(), "the real model25 split generations participate");
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[0], 2);
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[1], 1);
    assert!(sim(&game).source_arena().rebuilt_class5_model_slots()[25].is_empty());
    for _ in 0..128 {
        if seen25.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()) { break; }
        step(&mut game);
    }
    assert!(seen25.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()),
        "source census and the later model25 reclaim must both run");

    // Retain one ordinary living wave root while removing its peers to prove
    // the active objective's actual negative census, then contact that root.
    let survivor = *wave24.last().unwrap();
    let mut struck24 = BTreeSet::new();
    let mut group11 = 0;
    let mut exit = None;
    for leave_one in [true, false] {
        for _ in 0..2048 {
            let targets = sim(&game).source_arena().rebuilt_class5_model_slots()[24].iter()
                .map(|slot| sim(&game).source_arena().handle(*slot).unwrap()).collect::<Vec<_>>();
            if leave_one && targets.len() == 1 && targets[0] == survivor { break; }
            for handle in targets {
                if leave_one && handle == survivor { continue; }
                if struck24.insert((handle.source_slot, handle.generation)) { contact(&mut game, player, handle); }
            }
            step(&mut game);
            for event in sim(&game).dispatch_events() {
                if let Mc2DispatchEvent::AuthoredGroupActivated { group: 11, authored_indices, .. } = event {
                    assert_eq!(authored_indices.as_ref(), [466, 467]);
                    group11 += 1;
                    let trigger = sim(&game).economy().authored_switch_producers().iter()
                        .find(|row| row.active && row.authored_index == 466 && (row.model, row.action) == (12, 12))
                        .expect("the source group publishes the real exit");
                    assert!(sim(&game).economy().authored_class14().iter().any(|row| row.active && row.authored_index == 467));
                    exit = Some((trigger.position, sim(&game).source_arena().handle(trigger.source_entity_index).unwrap()));
                }
            }
            if leave_one {
                assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[1], 1,
                    "a living model24 root blocks source absence");
                assert_eq!(group11, 0);
            } else if group11 != 0 && wave24.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()) {
                break;
            }
        }
        if leave_one {
            let live = sim(&game).source_arena().rebuilt_class5_model_slots();
            assert_eq!(live[24], [survivor.source_slot]);
            assert_eq!(sim(&game).source_arena().handle(survivor.source_slot), Some(survivor));
        }
    }
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[..2], [2, 2]);
    assert_eq!(sim(&game).runtime().objectives[owner].is_level_end, 1);
    assert!(sim(&game).source_arena().rebuilt_class5_model_slots()[24].is_empty());
    assert!(wave24.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()));
    assert_eq!(group11, 1);
    let (position, trigger) = exit.expect("row465 publishes group11 and the exit");
    let sequence = sim(&game).level_exit_sequence();
    if !sequence.active && !sequence.complete {
        assert!(sim(&game).source_arena().resolve(trigger).is_some());
        sim_mut(&mut game).set_level2_entity_position(player, position).unwrap();
    }
    for _ in 0..700 {
        if matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }) { break; }
        step(&mut game);
    }
    assert!(matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }));
    assert_eq!(sim(&game).source_exit_result_byte(), 0x0a);
}

#[test]
fn alliance_follows_retained_parent_and_expires_when_dead() {
    let (package, mut arena, mut runtime, mut terrain, particles) = cave_owner_arena();
    let player = arena.entities.iter().find(|row| row.active && row.class == 3 && row.model == 0).unwrap().index;
    let feature = crate::level1_mission::parse_authored_features(package.authored_bytes()).unwrap().into_iter()
        .find(|row| row.kind == 5 && row.subtype == 24 && row.activation_group > 0).unwrap();
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let slot = mc2::allocate_authored(&mut arena, &mut runtime, &mut terrain, &particles,
        position, 16000, None, feature.index).unwrap().unwrap();
    let terminal = crate::native_world::alliance_spell::construct_impact(&mut arena, &mut terrain).unwrap();
    let effect = &mut arena.entities[usize::from(terminal)];
    effect.source_id = player;
    effect.record[74] = 1;
    effect.subspell_index = 40;
    effect.record[154..156].copy_from_slice(&slot.to_le_bytes());
    let impact = crate::native_world::alliance_spell::step_impact(&mut arena, terminal, &[slot]).unwrap();
    assert_eq!(impact.affected_entities, [slot]);
    let hostile = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
    let mut enemy = crate::world::terrain::cave_features::new_event(hostile, 17, Position {
        x: position.x.wrapping_add(512), ..position
    });
    enemy.class = 3;
    enemy.model = 1;
    enemy.record[67] = 3;
    enemy.record[68] = 1;
    enemy.set_life(1000);
    arena.entities[usize::from(hostile)] = enemy;
    crate::native_world::area_damage::contact(&mut arena, None, slot, 1, hostile).unwrap();
    let parent = &mut arena.entities[usize::from(player)];
    parent.set_life(100);
    parent.record[154..156].copy_from_slice(&hostile.to_le_bytes());
    parent.record[42..44].fill(0);
    parent.reclaim_source_row();
    assert_eq!(parent.class, 0);
    assert_eq!(parent.life, 100);
    let families = std::array::from_fn(|model| if model == usize::from(SOURCE_MODEL) { vec![slot] } else { Vec::new() });
    for life in [100, 0] {
        let mut trial = arena.clone();
        let mut trial_runtime = runtime.clone();
        let mut trial_terrain = terrain.clone();
        trial.entities[usize::from(player)].set_life(life);
        let effects = mc2::step_row(&mut trial, &mut trial_runtime, &mut trial_terrain, &particles,
            slot, &[], &families).unwrap();
        let actor = &trial.entities[usize::from(slot)];
        assert_eq!(actor.record[77], if life > 0 { 14 } else { 10 });
        assert_eq!(u16::from_le_bytes([actor.record[44], actor.record[45]]),
            if life > 0 { player } else { 0 });
        if life > 0 {
            assert_eq!(effects.earlier_feedback, Some((player, 19)));
            assert_eq!(effects.feedback, Some((player, 24)));
            assert_eq!(i16::from_le_bytes([actor.record[50], actor.record[51]]), 39);
        } else {
            assert_eq!(actor.source_word_96(), 0);
            assert_eq!(actor.control_flags & 0x0600_0000, 0);
        }
    }
}


fn actor() -> Creature24State {
    construct(
        701,
        3,
        19,
        Position {
            x: 20000,
            y: 20000,
            z: 0,
        },
        MapType::Cave,
        &mut 0,
    )
    .unwrap()
}
fn terrain() -> SessionTerrain {
    let session = crate::native_world::tests::level_two_campaign_session();
    session.canonical_terrain().clone()
}
fn context<'a>(
    terrain: &'a SessionTerrain,
    targets: &'a [Target],
    class3: &'a [u16],
) -> Creature24Context<'a> {
    Creature24Context {
        terrain,
        targets,
        class3,
        model24: &[],
        class5: &[],
        stage_runtime: None,
    }
}

struct ConfiguredStage(StageDescriptor);

impl StageRuntime for ConfiguredStage {
    fn descriptor(&self, _: u8) -> Option<StageDescriptor> { Some(self.0) }
    fn bind(&mut self, _: u8) -> Option<StageDescriptor> { Some(self.0) }
    fn rearm_model_relations(&mut self) {}
}

#[test]
fn configured_stage_point_inherits_child_hit_before_motion() {
    let terrain = terrain();
    let mut actor = actor();
    actor.action = 199;
    actor.stage.mode = 1;
    actor.phase = 1;
    actor.next = 702;
    actor.control_flags |= 0x800;
    let prior = actor.position;
    let child = Target { slot: 702, life: 15000, attacker: 255, ..Target::default() };
    let attacker = Target { slot: 255, source_id: 255, class: 3, model: 0, life: 10000, ..Target::default() };
    let targets = [child, attacker];
    let mut runtime = ConfiguredStage(StageDescriptor {
        kind: 1, flags: 0, next: 0, point: prior, actor: 0, model: 0, scalar: 0,
    });
    let mut ctx = Creature24Context { terrain: &terrain, targets: &targets, class3: &[], model24: &[], class5: &[], stage_runtime: Some(&mut runtime) };
    let mut effects = Creature24Effects::default();
    stage::step(&mut actor, &mut ctx, &mut effects);
    assert_eq!((actor.life, actor.attacker, actor.target, actor.stage.mode, actor.action), (15000, 255, 255, 10, 194));
    assert_ne!(actor.control_flags & 0x800, 0, "hit handling must not call movement");
    assert_eq!((actor.position.x, actor.position.y), (prior.x, prior.y));
}

#[test]
fn configured_stage_follow_target_separation_overrides_peer_heading() {
    let terrain = terrain();
    let mut actor = actor();
    actor.action = 199;
    actor.stage.mode = 5;
    actor.phase = 8;
    actor.half_width = 256;
    actor.control_flags |= 0x800;
    let point = Position { x: actor.position.x - 64, ..actor.position };
    let peer_position = Position { x: actor.position.x + 64, ..actor.position };
    let target = Target { slot: 255, source_id: 255, class: 5, model: 4, life: 1000, position: point, half_width: 128, ..Target::default() };
    let peer = Target { slot: 702, source_id: 702, class: 5, model: 24, life: 1000, position: peer_position, ..Target::default() };
    let targets = [target, peer];
    let mut runtime = ConfiguredStage(StageDescriptor { kind: 5, flags: 0, next: 0, point, actor: 255, model: 4, scalar: 0 });
    let mut ctx = Creature24Context { terrain: &terrain, targets: &targets, class3: &[], model24: &[702], class5: &[], stage_runtime: Some(&mut runtime) };
    let peer_heading = heading_for_positions(peer_position, actor.position) as i16;
    let expected = heading_for_positions(point, actor.position) as i16;
    assert_ne!(peer_heading, expected);
    stage::step(&mut actor, &mut ctx, &mut Creature24Effects::default());
    assert_eq!(actor.roll, expected);
}

#[test]
fn configured_ground_motion_publishes_accepted_and_blocked_proposals_but_not_hold() {
    use crate::native_world::TerrainPlanes;
    fn world(flags: u8) -> SessionTerrain {
        SessionTerrain::from_planes(MapType::Cave, TerrainPlanes {
            terrain_types: vec![1; 65536].into_boxed_slice(),
            ground_heights: vec![0; 65536].into_boxed_slice(),
            shading: vec![0; 65536].into_boxed_slice(),
            angles: vec![flags; 65536].into_boxed_slice(),
            secondary_heights: vec![255; 65536].into_boxed_slice(),
            entity_indices: vec![0; 65536].into_boxed_slice(),
        }, 0)
    }
    let initial_scratch = Position { x: 1, y: 2, z: 3 };
    let open = world(0);
    open.set_source_predicted_position(initial_scratch);
    let mut direct = actor();
    direct.yaw = 0;
    direct.roll = 200;
    direct.speed = 512;
    let original = direct.position;
    let mut expected = original;
    crate::move_entity(&mut expected, 0, 0, 512);
    assert!(!move_actor(&mut direct, &context(&open, &[], &[])));
    assert_eq!(open.source_predicted_position(), expected);
    assert_eq!(direct.position, expected);
    assert_eq!(direct.yaw, 5, "source 58350 clamps by descriptor subtype, not word4");

    let blocked = world(8);
    blocked.set_source_predicted_position(initial_scratch);
    let mut stopped = actor();
    stopped.yaw = 0;
    stopped.roll = 0;
    stopped.speed = 512;
    let mut last = stopped.position;
    crate::move_entity(&mut last, 1024, 0, 512);
    assert!(move_actor(&mut stopped, &context(&blocked, &[], &[])));
    assert_eq!(stopped.position, original);
    assert_eq!(stopped.life, -1);
    assert_ne!(last, original);
    assert_eq!(blocked.source_predicted_position(), last);

    stopped.control_flags |= 0x800;
    let saved = blocked.source_predicted_position();
    assert!(!move_actor(&mut stopped, &context(&blocked, &[], &[])));
    assert_eq!(blocked.source_predicted_position(), saved);
    assert_eq!(stopped.control_flags & 0x800, 0);
}

#[test]
fn constructor_rejects_non_cave_without_advancing_counter_and_uses_one_rng_draw() {
    let mut counter = 255;
    assert!(construct(701, 3, 19, Position::default(), MapType::Day, &mut counter).is_none());
    assert_eq!(counter, 255);
    let actor = construct(701, 3, 19, Position::default(), MapType::Cave, &mut counter).unwrap();
    assert_eq!((counter, actor.phase), (0, 255));
    assert_eq!(actor.rng, level2_next_rng16(19));
    assert_eq!((actor.life, actor.mana, actor.damage), (16000, 8000, 1500));
    assert_eq!(
        (actor.speed, actor.minimum_speed, actor.maximum_speed),
        (24, 80, 24)
    );
    assert_eq!(
        (actor.yaw, actor.roll, actor.pitch),
        (
            ((actor.rng & 2047) as i16) - 1,
            ((actor.rng & 2047) as i16) - 1,
            ((actor.rng & 2047) as i16) - 1
        )
    );
    assert_eq!(
        actor.control_flags, 8,
        "constructor does not invent a replaceable flag"
    );
}

#[test]
fn melee_contact_uses_strict_wrapped_range_and_flee_visual_speed() {
    let terrain = terrain();
    let mut actor = actor();
    actor.action = 194;
    actor.phase = 8;
    actor.target = 12;
    actor.control_flags |= 0x800;
    let target = Target {
        slot: 12,
        source_id: 12,
        class: 3,
        model: 0,
        life: 10000,
        position: Position {
            x: actor.position.x + 1535,
            ..actor.position
        },
        ..Target::default()
    };
    let effects = step(&mut actor, &mut context(&terrain, &[target], &[12])).unwrap();
    assert_eq!(effects.melee, Some((12, 1500, 701)));
    assert_eq!((actor.action, actor.speed, actor.visual), (198, 48, 335));
    assert_eq!(actor.phase, 8, "shared epilogue owns the only increment");
    actor.action = 194;
    actor.control_flags |= 0x800;
    let target = Target {
        position: Position {
            x: actor.position.x + 1536,
            ..actor.position
        },
        ..target
    };
    let effects = step(&mut actor, &mut context(&terrain, &[target], &[12])).unwrap();
    assert_eq!(effects.melee, None);
    assert_eq!((actor.action, actor.speed), (194, 80));
}

#[test]
fn moving_wrapper_redirects_lethal_mailbox_then_flee_enters_actual_prekill() {
    let terrain = terrain();
    let mut actor = actor();
    actor.action = 193;
    actor.pending_damage = 16001;
    actor.pending_attacker = 12;
    let target = Target {
        slot: 12,
        source_id: 12,
        class: 3,
        model: 0,
        life: 10000,
        ..Target::default()
    };
    step(&mut actor, &mut context(&terrain, &[target], &[12])).unwrap();
    assert_eq!((actor.life, actor.action, actor.killer), (-1, 198, 12));
    assert_eq!(
        (actor.pending_attacker, actor.pending_damage),
        (0, 16001),
        "source payload residue survives consumption"
    );
    actor.phase = 1;
    step(&mut actor, &mut context(&terrain, &[target], &[12])).unwrap();
    assert_eq!(actor.action, 196);
    assert_eq!(
        actor.killer, 0,
        "1C980 clears word26 with no new mailbox owner, then overwrites word24"
    );
    let effects = step(&mut actor, &mut context(&terrain, &[target], &[12])).unwrap();
    assert_eq!(
        effects.kill_credit, None,
        "193's explicit198 redirect loses source kill credit"
    );
    assert_eq!(actor.action, 197);
    actor.phase = 7;
    assert!(
        !step(&mut actor, &mut context(&terrain, &[target], &[12]))
            .unwrap()
            .kill
    );
    actor.phase = 8;
    assert!(
        step(&mut actor, &mut context(&terrain, &[target], &[12]))
            .unwrap()
            .kill
    );
}

#[test]
fn stage_wander_descriptor_bit_two_preserves_pose_rng_and_missing_parent_expires() {
    let terrain = terrain();
    let mut actor = actor();
    actor.action = 199;
    actor.stage.mode = 6;
    actor.phase = 16;
    let before = (actor.position, actor.rng, actor.roll);
    step(&mut actor, &mut context(&terrain, &[], &[])).unwrap();
    assert_eq!(
        (actor.position, actor.rng, actor.roll),
        before,
        "descriptor102 bit2 suppresses wandering"
    );
    actor.stage.mode = 13;
    actor.stage.parent = 123;
    actor.stage.remaining = 25;
    let effects = step(&mut actor, &mut context(&terrain, &[], &[])).unwrap();
    assert!(effects.expire && effects.summon_expire);
    assert_eq!(actor.stage.remaining, 0);
}

#[test]
fn adapter_round_trip_retains_owned_source_fields_and_real_visual_clocks() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let particles = &session.session.definition.presentation.particles;
    let mut row = mc2::construct_row(
        701,
        3,
        19,
        Position {
            x: 20000,
            y: 20000,
            z: 123,
        },
        MapType::Cave,
        &mut 9,
        particles,
    )
    .unwrap()
    .unwrap();
    let mut actor = mc2::load(&row, 3);
    actor.target = 12;
    actor.action = 192;
    actor.pending_damage = 17;
    actor.pending_attacker = 13;
    actor.stage.mode = 14;
    actor.stage.remaining = 200;
    actor.stage.parent = 12;
    visuals(&mut actor);
    mc2::store(&mut row, actor, particles).unwrap();
    let after = mc2::load(&row, 3);
    assert_eq!(
        after,
        Creature24State {
            animation_limit: particles[336].animation_limit().unwrap(),
            ..actor
        }
    );
    assert_eq!(row.parameter_relation, Some(102));
    assert_eq!(
        (row.class, row.model, row.action, row.actual_speed),
        (5, 24, 192, 0)
    );
    assert_eq!(row.record_bytes()[96], 0);
    assert_eq!(
        row.record_bytes()[97],
        particles[336].animation_limit().unwrap()
    );
}

fn cave_owner_arena() -> (
    crate::campaign::spec::DecodedCampaignPackage,
    crate::campaign::source_arena::SourceArena,
    crate::native_world::RuntimeSnapshot,
    SessionTerrain,
    Box<[crate::native_world::ParticleParameter]>,
) {
    let root = std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261024-b")
        },
        std::path::PathBuf::from,
    );
    owner_cave_arena(root, 21)
}

fn owner_cave_arena(
    root: std::path::PathBuf,
    level: u16,
) -> (
    crate::campaign::spec::DecodedCampaignPackage,
    crate::campaign::source_arena::SourceArena,
    crate::native_world::RuntimeSnapshot,
    SessionTerrain,
    Box<[crate::native_world::ParticleParameter]>,
) {
    use crate::native_world::{
        Level2EconomyState, parse_entities, parse_presentation, parse_runtime, parse_terrain,
    };
    let package = crate::campaign::spec::DecodedCampaignPackage::decode(
        root.join(format!("level{level}")),
        crate::campaign::spec::CampaignSelection::ordinary(level).unwrap(),
    )
    .unwrap();
    let entities = parse_entities(package.member("entities.bin").unwrap())
        .unwrap()
        .into_boxed_slice();
    let runtime = parse_runtime(package.member("runtime.bin").unwrap()).unwrap();
    let presentation = parse_presentation(package.member("presentation.bin").unwrap()).unwrap();
    let terrain = SessionTerrain::from_planes(
        MapType::Cave,
        parse_terrain(package.member("terrain.bin").unwrap()).unwrap(),
        0,
    );
    let player = entities
        .iter()
        .find(|r| r.active && r.class == 3 && r.model == 0)
        .unwrap();
    let economy = Level2EconomyState::from_parts_with_castle_stage_footprints(
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
    let arena = crate::campaign::source_arena::SourceArena::new(entities, economy);
    (package, arena, runtime, terrain, presentation.particles)
}

#[test]
fn level16_objective_creature_death_leaves_frozen_model_list_then_reclaims() {
    use crate::level_definition::CreatureKind;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj");
    let (package, mut arena, mut runtime, mut terrain, particles) = owner_cave_arena(root, 16);
    let feature = crate::level1_mission::parse_authored_features(package.authored_bytes()).unwrap()[404];
    assert_eq!(
        (feature.kind, feature.subtype, feature.activation_group),
        (5, 24, 23)
    );
    let player = arena.entities.iter().find(|r| r.active && r.class == 3 && r.model == 0)
        .unwrap().index;
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let slot = mc2::allocate_authored(
        &mut arena, &mut runtime, &mut terrain, &particles, position, 16000, None, feature.index,
    ).unwrap().unwrap();
    let handle = arena.handle(slot).unwrap();
    let authored = std::collections::BTreeMap::from([(feature.index, CreatureKind::Creature24)]);
    assert!(arena.creature_runtime_identity(slot, &Default::default(), &authored).unwrap().living);
    let frozen = arena.rebuilt_class5_model_slots();
    assert!(frozen[24].contains(&slot));
    crate::native_world::area_damage::contact(&mut arena, None, slot, 16001, player).unwrap();
    let mut killed = false;
    let mut excluded_before_hide = false;
    for _ in 0..24 {
        let pass_start = arena.rebuilt_class5_model_slots();
        let outcome = mc2::step_row(
            &mut arena, &mut runtime, &mut terrain, &particles, slot, &[player], &pass_start,
        ).unwrap();
        let row = &mut arena.entities[usize::from(slot)];
        if row.life < 0 && !row.drawing_disabled() {
            excluded_before_hide = true;
            assert!(!arena.rebuilt_class5_model_slots()[24].contains(&slot));
        }
        if outcome.kill {
            killed = true;
            break;
        }
        let row = &mut arena.entities[usize::from(slot)];
        row.set_phase(row.phase.wrapping_add(1));
    }
    assert!(killed && excluded_before_hide);
    assert!(frozen[24].contains(&slot), "the earlier pass list remains frozen");
    assert!(!arena.rebuilt_class5_model_slots()[24].contains(&slot));
    assert!(!arena.creature_runtime_identity(slot, &Default::default(), &authored).unwrap().living);
    assert!(arena.resolve(handle).is_some(), "hide defers reclamation");
    arena.rebuild_source_arena_with_terrain(&mut terrain, &[]).unwrap();
    assert!(arena.resolve(handle).is_none());
    // 49F90 reconstructs the free stack in descending address order. Level16
    // has lower free addresses than this root, so consume that real prefix
    // before requiring reuse rather than assuming the freed address is first.
    let mut previous = 0;
    let reused = loop {
        let next = mc2::allocate_authored(
            &mut arena, &mut runtime, &mut terrain, &particles, position, 16000, None, feature.index,
        ).unwrap().expect("the reclaimed root remains in the free stack");
        assert!(next > previous && next <= slot, "unexpected free-stack order: {previous} -> {next}, root {slot}");
        assert!(arena.resolve(handle).is_none(), "stale generation must stay dead");
        if next == slot {
            break next;
        }
        previous = next;
    };
    assert_ne!(arena.handle(reused).unwrap().generation, handle.generation);
    assert!(arena.rebuilt_class5_model_slots()[24].contains(&reused));
    eprintln!("Level16 authored 404 group 23 slot={slot} reclaimed and reused; frozen model24 membership tracked death");
}

#[test]
fn owner_cave21_roots_acquire_real_player_and_contact_reaches_same_arena_mailbox() {
    let (_, mut arena, mut runtime, mut terrain, particles) = cave_owner_arena();
    let player = arena
        .entities
        .iter()
        .find(|r| r.active && r.class == 3 && r.model == 0)
        .unwrap()
        .index;
    let initial = arena
        .entities
        .iter()
        .filter(|r| r.active && r.class == 5 && r.model == 24)
        .count();
    assert_eq!(initial, 24, "actual owner Cave21 has24 live roots");
    let mut contacts = Vec::new();
    for _ in 0..96 {
        let families = arena.rebuilt_class5_model_slots();
        mc2::prepare_source_pass(&mut arena, &mut runtime, &terrain, &families, &particles)
            .unwrap();
        let player_position = arena.entities[usize::from(player)].position;
        mc2::proximity_prepass(&mut arena, &families[24], player_position);
        let slots = arena
            .entities
            .iter()
            .filter(|r| r.active && r.class == 5 && r.model == 24 && !r.drawing_disabled())
            .map(|r| r.index)
            .collect::<Vec<_>>();
        for slot in slots {
            let outcome = mc2::step_row(
                &mut arena,
                &mut runtime,
                &mut terrain,
                &particles,
                slot,
                &[player],
                &families,
            )
            .unwrap();
            if let Some((target, damage, source_id)) = outcome.melee {
                assert_eq!(target, player);
                crate::native_world::area_damage::contact(
                    &mut arena,
                    None,
                    target,
                    damage as u16,
                    source_id,
                )
                .unwrap();
                contacts.push((slot, target, damage, source_id));
            }
            let row = &mut arena.entities[usize::from(slot)];
            row.set_phase(row.phase.wrapping_add(1));
        }
    }
    assert!(
        !contacts.is_empty(),
        "ordinary owner positions/heading must produce actual melee"
    );
    let last = contacts.last().unwrap();
    let target = &arena.entities[usize::from(player)];
    assert_eq!(target.source_upkeep_word_62(), last.3);
    assert_eq!(target.source_upkeep_payload(), 1500);
    assert_eq!(
        crate::native_world::area_damage::source_actor(&mut arena, None, last.0)
            .unwrap()
            .class,
        5
    );
    eprintln!(
        "Cave21 actual roots={initial} player={player} first_contact={:?} contacts={}",
        contacts.first().unwrap(),
        contacts.len()
    );
}

#[test]
fn level16_authored_creature24_melee_reaches_player_life_and_audio() {
    use crate::PlayerCommand;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::campaign::source_arena::CreatureRootOrigin;
    use crate::level_definition::CreatureKind;
    use crate::native_world::{Mc2DispatchEvent, area_damage, Level2EconomyEvent};

    let package = std::env::var_os("MC_LEVEL16_PACKAGE").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/skeleton-20261106-integration/current-owner-package/level16"),
        std::path::PathBuf::from,
    );
    let mut session = CampaignSession::open(package, CampaignSelection::ordinary(16).unwrap())
        .expect("whole qualified Level16 package")
        .simulation()
        .clone();
    let player = session.state.entity_index;
    let player_handle = session.source_arena().handle(player).unwrap();
    let switch = session.source_arena().handle(92).unwrap();
    let row = &session.entities()[92];
    assert_eq!((row.class, row.model, row.action, row.source_id, row.level_gate), (11, 32, 32, 23, 3));
    assert_eq!(switch.generation, 0);
    assert!(session.source_arena().rebuilt_class5_model_slots()[24].is_empty());

    // Survival resources are a player-only entry control.  Let the actual
    // source player callback age its startup protection before activating the
    // objective; never edit the protection word or any creature state.
    {
        let live = &mut session.session.state.live;
        live.player_life = 200_000;
        live.shared_spells.set_player_entity_life(200_000);
        let parent = &mut live.economy.entities[usize::from(player)];
        parent.set_max_life(200_000);
        parent.set_life(200_000);
        let parent = parent.clone();
        live.shared_spells.sync_player_entity_from_authority(&parent);
    }
    session.state.actual_speed = 0;
    session.state.target_speed = 0;
    let initial_protection = session.session.state.live.player_protection_ticks;
    assert_eq!(initial_protection, 100);
    let mut wait_turns = 0;
    for turn in 0..256 {
        if session.session.state.live.player_protection_ticks == 0 { break; }
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "protection wait turn {turn}: {:?}", session.dispatch_events());
        assert!(!session.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 92, group: 23, .. })));
        wait_turns += 1;
    }
    assert_eq!(session.session.state.live.player_protection_ticks, 0,
        "ordinary player callbacks must exhaust source startup protection");
    assert!(wait_turns > 0);
    assert_eq!(session.source_arena().handle(player), Some(player_handle));
    assert_eq!(session.source_arena().handle(92), Some(switch));
    assert!(session.source_arena().rebuilt_class5_model_slots()[24].is_empty());

    // Explicit upstream objective prerequisite; the actual reset switch and
    // group callback still publish the whole unfiltered authored group.
    session.set_level2_selected_objective_stage_state(2).unwrap();
    let mut produced = None;
    for turn in 0..32 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "group turn {turn}: {:?}", session.dispatch_events());
        if let Some(indices) = session.dispatch_events().iter().find_map(|event| match event {
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 92, group: 23, authored_indices, .. }
                if authored_indices.contains(&404) => Some(authored_indices.clone()),
            _ => None,
        }) {
            produced = Some(indices);
            break;
        }
    }
    let produced = produced.expect("actual switch92 must publish authored model24 row404");
    // Pin every real current group23 Creature24 generation before the
    // interaction. The witness must remain one of these exact source roots.
    let mut roots = Vec::new();
    for identity in session.creature_runtime_census().roots_of_kind(CreatureKind::Creature24) {
        let CreatureRootOrigin::AuthoredRecord { authored_record } = identity.origin else { continue };
        if !produced.contains(&authored_record) { continue; }
        let feature = session.session.state.live.mission.authored_features().iter()
            .find(|feature| feature.index == authored_record).expect("published authored record");
        assert_eq!((feature.kind, feature.subtype, feature.activation_group), (5, 24, 23));
        let row = &session.entities()[usize::from(identity.handle.source_slot)];
        assert_eq!((row.class, row.model), (5, 24));
        assert!(identity.living);
        assert_eq!(session.source_arena().handle(identity.handle.source_slot), Some(identity.handle));
        roots.push((identity.handle, authored_record, row.source_id, row.position, false));
    }
    assert!(!roots.is_empty());
    let handle = session.creature_runtime_census().roots_of_kind(CreatureKind::Creature24)
        .find(|root| root.origin == (CreatureRootOrigin::AuthoredRecord { authored_record: 404 }))
        .map(|root| { assert!(root.living); root.handle })
        .expect("the current generation of real row404");
    assert!(roots.iter().any(|(root, authored, _, _, _)| *root == handle && *authored == 404));
    let source = session.entities()[usize::from(handle.source_slot)].clone();
    assert_eq!((source.class, source.model, source.max_life, source.subspell_index), (5, 24, 16000, 1500));

    // Declared player-only interception. The creature, its phase, route, RNG,
    // target and mailbox all remain source-controlled.
    let anchor = Position { x: source.position.x.wrapping_add(512), ..source.position };
    session.set_level2_entity_position(player, anchor).unwrap();
    // Setting the public player position does not reset flight velocity.
    // Hold the declared initial player approach still; later movement remains
    // under the ordinary player callback, including actual damage recoil.
    session.state.actual_speed = 0;
    session.state.target_speed = 0;
    let journal_start = session.effect_journal().records().len();
    area_damage::start_player_damage_trace(player_handle);
    let mut hit = None;
    let mut observations = Vec::new();
    let mut consumptions = Vec::new();
    for turn in 0..320 {
        let before = session.entities()[usize::from(handle.source_slot)].clone();
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "melee turn {turn}: {:?}", session.dispatch_events());
        assert_eq!(session.source_arena().handle(handle.source_slot), Some(handle));
        assert_eq!(session.source_arena().handle(player), Some(player_handle));
        let after = &session.entities()[usize::from(handle.source_slot)];
        for (root, _, _, initial_position, moved) in &mut roots {
            if session.source_arena().handle(root.source_slot) == Some(*root) {
                *moved |= session.entities()[usize::from(root.source_slot)].position != *initial_position;
            }
        }
        if observations.len() < 32
            && (turn < 8 || turn % 32 == 0 || before.action != after.action)
        {
            observations.push((turn, (before.action, before.phase, before.source_word_96(), before.position),
                (after.action, after.phase, after.source_word_96(), after.position, after.record[61]),
                (session.state.position, session.state.actual_speed, session.state.target_speed),
                area_damage::latest_player_mailbox_writer()));
        }
        for consumed in area_damage::take_player_damage_consumptions() {
            if consumptions.len() < 16 { consumptions.push(consumed); }
            let Some(writer) = consumed.writer else { continue };
            let area_damage::PlayerMailboxSource::Contact(producer) = writer.source else { continue };
            let Some((_, authored_record, source_id, _, moved)) = roots.iter().find(|(root, _, _, _, _)| *root == producer) else { continue };
            if session.source_arena().handle(producer.source_slot) != Some(producer) { continue; }
            let current = &session.entities()[usize::from(producer.source_slot)];
            assert_eq!((current.class, current.model, current.source_id), (5, 24, *source_id));
            assert!(*moved, "the exact witnessed source generation must have moved");
            assert_eq!((consumed.target, writer.target, writer.mailbox, consumed.mailbox),
                (player_handle, player_handle, consumed.mailbox, consumed.mailbox));
            assert_eq!(consumed.mailbox.1, *source_id);
            assert!(consumed.mailbox.0 >= 1500);
            if consumed.life_after == consumed.life_before.wrapping_sub(consumed.mailbox.0)
                && matches!(session.economy().events().get(consumed.event_cursor),
                    Some(Level2EconomyEvent::DamageApplied { target_entity_index, attacker_entity_index, damage, lethal: false })
                    if *target_entity_index == player && *attacker_entity_index == *source_id && *damage == consumed.mailbox.0)
            {
                hit = Some((turn, writer.sequence, consumed, producer, *authored_record));
                break;
            }
        }
        if hit.is_some() { break; }
    }
    let (_, _, _, witness, authored_record) = hit.unwrap_or_else(|| panic!(
        "a pinned group23 model24 writer must reach player life: initial={source:?} roots={roots:?} player_anchor={anchor:?} observations={observations:?} consumptions={consumptions:?}"
    ));
    assert!(produced.contains(&authored_record));
    assert_eq!(session.source_arena().handle(witness.source_slot), Some(witness));
    assert!(session.effect_journal().records()[journal_start..].iter().any(|record| matches!(
        &record.effect,
        crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
            audio_site: crate::MissionAudioSite::Creature24Attack, entity_index, sound_id: 7, player_index: -1,
        }) if *entity_index == witness.source_slot
    ) && record.audio.is_some_and(|audio| audio.command.audible && audio.play_type == 3)),
        "real action194 must prepare the original audible attack sound");
}

#[test]
fn ordinary_owner_constructor_combat_death_reclaim_and_reuse_keep_one_carrier() {
    use crate::campaign::source_arena::CreatureRootOrigin;
    use crate::level_definition::CreatureKind;
    let (package, mut arena, mut runtime, mut terrain, particles) = cave_owner_arena();
    let feature = crate::level1_mission::parse_authored_features(package.authored_bytes())
        .unwrap()
        .into_iter()
        .find(|f| f.kind == 5 && f.subtype == 24 && f.activation_group > 0)
        .expect("actual later group model24 placement");
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let counter = runtime.constructor_counters().unwrap()[24];
    let slot = mc2::allocate_authored(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        16000,
        None,
        feature.index,
    )
    .unwrap()
    .expect("ordinary shared source allocation");
    let old = arena.handle(slot).unwrap();
    assert_eq!(
        runtime.constructor_counters().unwrap()[24],
        counter.wrapping_add(1)
    );
    assert_eq!(arena.entities[usize::from(slot)].phase, counter);
    assert_eq!(arena.entities[usize::from(slot)].position, position);
    let authored = std::collections::BTreeMap::from([(feature.index, CreatureKind::Creature24)]);
    let identity = arena
        .creature_runtime_identity(slot, &Default::default(), &authored)
        .expect("actual semantic consumer");
    assert_eq!(
        identity.origin,
        CreatureRootOrigin::AuthoredRecord {
            authored_record: feature.index
        }
    );
    assert!(identity.living);
    let player = arena
        .entities
        .iter()
        .find(|r| r.active && r.class == 3 && r.model == 0)
        .unwrap()
        .index;
    crate::native_world::area_damage::contact(&mut arena, None, slot, 16001, player).unwrap();
    let mut credit = false;
    let mut dropped = false;
    for _ in 0..24 {
        let families = arena.rebuilt_class5_model_slots();
        let outcome = mc2::step_row(
            &mut arena,
            &mut runtime,
            &mut terrain,
            &particles,
            slot,
            &[player],
            &families,
        )
        .unwrap();
        credit |= outcome.kill_credit == Some(player);
        if outcome.kill {
            dropped = true;
            break;
        }
        let row = &mut arena.entities[usize::from(slot)];
        row.set_phase(row.phase.wrapping_add(1));
    }
    assert!(dropped, "ordinary source death must produce mana and hide");
    assert!(
        !credit,
        "lethal193 redirects through198 and source1C980 clears credit"
    );
    assert!(arena.entities[usize::from(slot)].drawing_disabled());
    assert!(
        !arena
            .creature_runtime_identity(slot, &Default::default(), &authored)
            .unwrap()
            .living
    );
    let sphere = arena
        .spheres()
        .iter()
        .find(|s| s.mana == 8000)
        .expect("real model39 mana descendant");
    assert_eq!(sphere.position, arena.entities[usize::from(slot)].position);
    let sphere_slot = sphere.source_entity_index;
    assert_eq!(
        crate::native_world::area_damage::source_actor(&mut arena, None, sphere_slot)
            .unwrap()
            .model,
        39
    );
    assert!(
        arena.resolve(old).is_some(),
        "hide preserves source lifetime through the same pass"
    );
    arena
        .rebuild_source_arena_with_terrain(&mut terrain, &[])
        .unwrap();
    assert!(
        arena.resolve(old).is_none(),
        "following prologue reclaims the hidden source root"
    );
    let reused = mc2::allocate_authored(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        16000,
        None,
        feature.index,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        reused, slot,
        "source free-stack chooses the newly lowest free address"
    );
    assert_ne!(arena.handle(reused).unwrap().generation, old.generation);
    assert!(
        arena.resolve(old).is_none(),
        "old lifetime cannot resolve the reused source address"
    );
    eprintln!(
        "Cave21 authored_record={} slot={slot} death_mana_slot={sphere_slot} reused_generation={}",
        feature.index,
        arena.handle(reused).unwrap().generation
    );
}

#[test]
fn failed_ordinary_allocation_preserves_counter_and_non_cave_rejection_preserves_arena() {
    let (package, mut arena, mut runtime, mut terrain, particles) = cave_owner_arena();
    let feature = crate::level1_mission::parse_authored_features(package.authored_bytes())
        .unwrap()
        .into_iter()
        .find(|f| f.kind == 5 && f.subtype == 24 && f.activation_group > 0)
        .unwrap();
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let before_runtime = runtime.clone();
    let before_arena = arena.clone();
    terrain.map_type = MapType::Day;
    assert_eq!(
        mc2::allocate_authored(
            &mut arena,
            &mut runtime,
            &mut terrain,
            &particles,
            position,
            16000,
            None,
            feature.index
        )
        .unwrap(),
        None
    );
    assert_eq!(runtime, before_runtime);
    assert_eq!(
        arena.reset_hash_with_terrain(&terrain),
        before_arena.reset_hash_with_terrain(&terrain)
    );
    terrain.map_type = MapType::Cave;
    arena.finish_source_group();
    arena.economy.exhaust_new_event_capacity_for_test();
    let counter = runtime.constructor_counters().unwrap()[24];
    assert_eq!(
        mc2::allocate_authored(
            &mut arena,
            &mut runtime,
            &mut terrain,
            &particles,
            position,
            16000,
            None,
            feature.index
        )
        .unwrap(),
        None
    );
    assert_eq!(runtime.constructor_counters().unwrap()[24], counter);
}

#[test]
fn lethal_pursuit_reaches_prekill_without_flee_and_awards_real_killer() {
    let terrain = terrain();
    let mut actor = actor();
    actor.action = 194;
    actor.pending_damage = 16001;
    actor.pending_attacker = 12;
    let target = Target {
        slot: 12,
        source_id: 12,
        class: 3,
        model: 0,
        life: 10000,
        ..Target::default()
    };
    step(&mut actor, &mut context(&terrain, &[target], &[12])).unwrap();
    assert_eq!((actor.action, actor.killer, actor.life), (196, 12, -1));
    let effects = step(&mut actor, &mut context(&terrain, &[target], &[12])).unwrap();
    assert_eq!(effects.kill_credit, Some(12));
    assert_eq!(actor.action, 197);
}

#[test]
fn full_arena_death_replaces_its_own_address_and_hides_the_newborn_until_reclaim() {
    let (_, mut arena, mut runtime, mut terrain, particles) = cave_owner_arena();
    let slot = arena
        .entities
        .iter()
        .find(|row| row.active && row.class == 5 && row.model == 24)
        .unwrap()
        .index;
    // Bound the allocator failure branch to one replaceable raw address.
    // This is an aliasing regression, not a full-pool gameplay capture.
    for row in &mut arena.entities {
        if row.index != slot {
            row.set_control_flags(
                row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
            );
        }
    }
    arena.economy.exhaust_new_event_capacity_for_test();
    arena.rebuild_source_free_arena();
    assert_eq!(arena.economy.free_slot_count(), 0);
    assert_eq!(
        arena.entities[usize::from(slot)].control_flags
            & crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
        0,
        "ordinary constructor bit8 does not grant source byte2 bit2 replacement"
    );
    assert_eq!(
        arena.allocate_source_slot_with_terrain(&mut terrain),
        None,
        "the source must refuse replacement without the actual eligibility bit"
    );
    // Explicitly admit the selected address into the source49F90 fallback
    // list. Ordinary construction above must continue to omit this flag.
    let row = &mut arena.entities[usize::from(slot)];
    row.set_control_flags(row.control_flags | crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    arena.rebuild_source_free_arena();
    let old = arena.handle(slot).unwrap();
    let seed = arena.new_event_rng(slot);
    let row = &mut arena.entities[usize::from(slot)];
    row.set_source_action(197);
    row.set_phase(0);
    row.set_life(-1);
    row.set_mana(8000);
    row.set_source_player_entity_index(255);
    let families = arena.rebuilt_class5_model_slots();
    let effect = mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        slot,
        &[255],
        &families,
    )
    .unwrap();
    assert!(effect.kill);
    assert!(
        arena.resolve(old).is_none(),
        "NewEvent ends the old lifetime"
    );
    let sphere = arena
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == slot)
        .expect("the fixed-address parent is now a real model39 occupant");
    assert_eq!(sphere.position, Position::default());
    assert_eq!(
        sphere.mana, 512,
        "the source rereads newborn constructor mana"
    );
    assert_eq!(sphere.player_entity_index, 0);
    assert_eq!(
        sphere.entity_rng,
        level2_next_rng16(level2_next_rng16(seed))
    );
    assert!(sphere.active && sphere.pending_delete);
    assert_ne!(sphere.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
    assert_eq!(
        sphere.phase, slot as u8,
        "callback leaves the epilogue to its owner"
    );
    let replacement = arena.handle(slot).unwrap();
    assert_eq!(replacement.generation, old.generation.wrapping_add(1));
    arena
        .rebuild_source_arena_with_terrain(&mut terrain, &[])
        .unwrap();
    assert!(arena.resolve(replacement).is_none());
    assert!(
        !arena
            .spheres()
            .iter()
            .any(|sphere| sphere.source_entity_index == slot)
    );
    assert_eq!(
        terrain.planes().entity_indices[0] as u16,
        arena.economy.map_chains.tile_head(0),
        "following-pass sphere reclaim must publish the same tile head to terrain consumers"
    );
}
