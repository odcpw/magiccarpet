use super::*;

pub(crate) fn complete_ul_buthnen_objectives_for_test(
    game: &mut crate::native_world::player_lifecycle_tests::RivalTestGame,
) {
    use crate::game_session::GameCommand;
    use crate::combat::area::{AreaDamage, Shape};
    use crate::house::HouseStepEvent;
    use crate::native_world::{Mc2DispatchEvent, area_damage};

    struct LiveSession<'a> {
        game: &'a mut crate::native_world::player_lifecycle_tests::RivalTestGame,
    }
    impl std::ops::Deref for LiveSession<'_> {
        type Target = crate::native_world::Mc2Simulation;
        fn deref(&self) -> &Self::Target {
            self.game.backend().compatibility_campaign().simulation()
        }
    }
    impl std::ops::DerefMut for LiveSession<'_> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.game.backend_mut().compatibility_campaign_mut_for_test().simulation_mut()
        }
    }
    fn tick(session: &mut LiveSession<'_>,
            groups: &mut Vec<(u16, Vec<u16>)>, deaths: &mut Vec<(u16, u16)>) {
        session.game.advance(session.game.identity().guard(), GameCommand::default()).unwrap();
        for event in session.dispatch_events() {
            match event {
                Mc2DispatchEvent::Error { .. } => panic!("Ul'Buthnen dispatch: {event:?}"),
                Mc2DispatchEvent::AuthoredGroupActivated { group, authored_indices, .. } =>
                    groups.push((*group, authored_indices.to_vec())),
                Mc2DispatchEvent::House { source_slot, events, .. } => {
                    for event in events.iter() {
                        if let HouseStepEvent::DestructionStarted { attacker_entity_index, .. } = event {
                            deaths.push((*source_slot, *attacker_entity_index));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    fn strike_model(session: &mut LiveSession<'_>, player: u16, model: u8,
                    exclude: Option<crate::campaign::source_arena::SourceEntityHandle>)
        -> Vec<crate::campaign::source_arena::SourceEntityHandle> {
        let roots = session.source_arena().rebuilt_class5_model_slots()[usize::from(model)]
            .iter().map(|slot| session.source_arena().handle(*slot).unwrap()).collect::<Vec<_>>();
        for handle in &roots {
            if Some(*handle) == exclude { continue; }
            let row = &session.entities()[usize::from(handle.source_slot)];
            let request = AreaDamage {
                position: row.position,
                shape: Shape { half_width: 1, half_depth: 1,
                    vertical_offset: row.array_yaw(), half_height: 1 },
                owner: player, payload: 60_000, selector: (5, model), mode: 0,
            };
            let live = &mut session.session.state.live;
            let hits = area_damage::apply(&mut live.economy,
                Some(area_damage::ActorDomains::new(&mut live.goats, &mut live.ordinary)),
                Some((player, live.core.state.position)), request).unwrap();
            let (damage, attacker) = area_damage::source_bounty(&live.economy, None,
                handle.source_slot).unwrap();
            assert!(hits > 0 && damage > 0 && attacker == player,
                "source area admission must accept the selected model{model} generation {handle:?}: hits={hits} bounty=({damage},{attacker})");
        }
        roots
    }
    fn wait_absence(session: &mut LiveSession<'_>, player: u16,
                    model: u8, objective: usize, groups: &mut Vec<(u16, Vec<u16>)>,
                    deaths: &mut Vec<(u16, u16)>) {
        let mut generations = Vec::new();
        for _ in 0..512 {
            let current = session.source_arena().rebuilt_class5_model_slots()[usize::from(model)].clone();
            if !current.is_empty() {
                let struck = strike_model(session, player, model, None);
                for handle in struck { if !generations.contains(&handle) { generations.push(handle); } }
            }
            if session.runtime().objectives[0].stage_selectors[objective] == 2
                && generations.iter().all(|handle| session.source_arena().resolve(*handle).is_none())
            { return; }
            tick(session, groups, deaths);
        }
        panic!("model{model} objective{objective} or cleanup did not finish: selector={} remaining={:?}",
            session.runtime().objectives[0].stage_selectors[objective],
            session.source_arena().rebuilt_class5_model_slots()[usize::from(model)]);
    }
    fn has_group(groups: &[(u16, Vec<u16>)], group: u16, rows: &[u16]) -> bool {
        groups.iter().any(|(id, authored)| *id == group
            && rows.iter().all(|row| authored.contains(row)))
    }
    fn complete_group(session: &crate::native_world::Mc2Simulation,
                      groups: &[(u16, Vec<u16>)], group: u16) -> bool {
        let expected = session.mission().authored_features().iter()
            .filter(|row| row.kind != 0 && row.activation_group == group as i16)
            .map(|row| row.index).collect::<Vec<_>>();
        !expected.is_empty() && groups.iter().any(|(id, rows)| *id == group && *rows == expected)
    }
    fn travel(session: &mut LiveSession<'_>, player: u16, x: u16, y: u16) {
        let position = Position { x: x * 256, y: y * 256, z: session.state.position.z };
        session.set_level2_entity_position(player, position).unwrap();
        session.state.actual_speed = 0;
        session.state.target_speed = 0;
    }

    let mut session = LiveSession { game };
    let player = session.state.entity_index;
    let mut groups = Vec::new();
    let mut house_deaths = Vec::new();
    let initial_houses = session.campaign_building_objective_targets(5).into_iter()
        .map(|(slot, _)| session.source_arena().handle(slot).unwrap()).collect::<Vec<_>>();
    assert_eq!(initial_houses.len(), 4, "the package has four selector53 objective houses");
    assert_eq!(session.runtime().stages[0].relation, crate::native_world::RuntimeRelation::Scalar(20));
    assert_eq!(session.source_arena().rebuilt_class5_model_slots()[20].len(), 8);
    let survivor = session.source_arena().handle(session.source_arena().rebuilt_class5_model_slots()[20][0]).unwrap();
    let first_spiders = strike_model(&mut session, player, 20, Some(survivor));
    tick(&mut session, &mut groups, &mut house_deaths);
    assert_eq!(session.runtime().objectives[0].stage_selectors[0], 1,
        "the fatal pass uses its frozen living Spider census");
    for _ in 0..512 {
        assert_eq!(session.runtime().objectives[0].stage_selectors[0], 1,
            "one actual living Spider blocks the source absence objective");
        assert!(session.creature_identity_for_source_slot(survivor.source_slot)
            .is_some_and(|row| row.handle == survivor && row.living));
        if first_spiders.iter().filter(|handle| **handle != survivor)
            .all(|handle| session.source_arena().resolve(*handle).is_none()) { break; }
        tick(&mut session, &mut groups, &mut house_deaths);
    }
    assert!(first_spiders.iter().filter(|handle| **handle != survivor)
        .all(|handle| session.source_arena().resolve(*handle).is_none()));
    wait_absence(&mut session, player, 20, 0, &mut groups, &mut house_deaths);
    for _ in 0..256 {
        if has_group(&groups, 3, &[26, 28])
            && first_spiders.iter().all(|handle| session.source_arena().resolve(*handle).is_none())
        { break; }
        tick(&mut session, &mut groups, &mut house_deaths);
    }
    assert!(has_group(&groups, 3, &[26, 28]), "Spider absence must publish group3: {groups:?}");
    assert!(complete_group(&session, &groups, 3));
    assert!(first_spiders.iter().all(|handle| session.source_arena().resolve(*handle).is_none()));

    travel(&mut session, player, 202, 88);
    for _ in 0..256 {
        tick(&mut session, &mut groups, &mut house_deaths);
        if session.runtime().objectives[0].stage_selectors[1] == 2 { break; }
    }
    assert_eq!(session.runtime().objectives[0].stage_selectors[1], 2);
    assert_eq!(session.runtime().objectives[0].stage_selectors[2], 1);
    travel(&mut session, player, 220, 114);
    for _ in 0..256 {
        tick(&mut session, &mut groups, &mut house_deaths);
        if has_group(&groups, 28, &[116, 117, 118, 167, 168]) { break; }
    }
    assert_eq!(session.runtime().objectives[0].stage_selectors[2], 2);
    assert!(has_group(&groups, 28, &[116, 117, 118, 167, 168]),
        "second reach must publish all five group28 Spell Thieves: {groups:?}");
    assert!(complete_group(&session, &groups, 28));
    assert_eq!(session.runtime().stages[3].relation, crate::native_world::RuntimeRelation::Scalar(26));
    assert_eq!(session.runtime().objectives[0].stage_selectors[3], 1);
    wait_absence(&mut session, player, 26, 3, &mut groups, &mut house_deaths);
    for _ in 0..256 {
        if has_group(&groups, 26, &[113, 140, 154, 164, 165]) { break; }
        tick(&mut session, &mut groups, &mut house_deaths);
    }
    assert!(has_group(&groups, 26, &[113, 140, 154, 164, 165]),
        "Spell Thief absence must publish the model21 group26: {groups:?}");
    assert!(complete_group(&session, &groups, 26));
    assert_eq!(session.runtime().stages[4].relation, crate::native_world::RuntimeRelation::Scalar(21));
    assert_eq!(session.runtime().objectives[0].stage_selectors[4], 1);
    wait_absence(&mut session, player, 21, 4, &mut groups, &mut house_deaths);

    // The objective is a selector53 census, not a pointer to only row231.
    // Route strikes through source area admission and the typed House domain;
    // other source actors may also contribute their own valid damage.
    let mut player_house_hits = 0_u32;
    for _ in 0..512 {
        let targets = session.campaign_building_objective_targets(5);
        for (slot, _) in targets {
            let Some(house) = session.ordinary().houses().iter()
                .find(|house| house.active && house.entity_index == slot && house.life >= 0)
                .copied() else { continue; };
            let request = AreaDamage { position: house.position,
                shape: Shape { half_width: 1, half_depth: 1,
                    vertical_offset: house.collision_vertical_offset, half_height: 1 },
                owner: player, payload: 60_000, selector: (10, 45), mode: 0 };
            let live = &mut session.session.state.live;
            let hits = area_damage::apply(&mut live.economy,
                Some(area_damage::ActorDomains::new(&mut live.goats, &mut live.ordinary)),
                Some((player, live.core.state.position)), request).unwrap();
            player_house_hits += u32::from(hits);
        }
        if session.runtime().objectives[0].stage_selectors[5] == 2
            && initial_houses.iter().all(|handle| session.source_arena().resolve(*handle).is_none())
        { break; }
        tick(&mut session, &mut groups, &mut house_deaths);
    }
    assert_eq!(session.runtime().objectives[0].stage_selectors[5], 2,
        "the source building-chain census must complete after demolition");
    assert!(initial_houses.iter().all(|handle| session.source_arena().resolve(*handle).is_none()));
    assert!(initial_houses.iter().all(|handle| house_deaths.iter()
        .any(|(slot, _)| *slot == handle.source_slot)),
        "every original objective House must report its real destruction writer: {house_deaths:?}");
    assert!(player_house_hits > 0 || house_deaths.iter().any(|(_, attacker)| *attacker != 0),
        "source-admitted player or NPC damage must account for demolition");
    for _ in 0..256 {
        if session.campaign_exit_objective_target().is_some() { break; }
        tick(&mut session, &mut groups, &mut house_deaths);
    }
    assert!(has_group(&groups, 29, &[170, 179]),
        "the level-end gate must construct the authored portal and exit: {groups:?}");
    let (exit_slot, exit_position) = session.campaign_exit_objective_target().expect("real exit");
    assert!(session.economy().authored_switch_producers().iter().any(|source|
        source.source_entity_index == exit_slot && source.authored_index == 170));
    assert!(session.economy().authored_class14().iter().any(|source| source.authored_index == 179));
    assert!(!session.level_exit_sequence().active && !session.level_exit_sequence().complete);
    let _ = exit_position;
}

#[test]
fn level8_ul_buthnen_authored_objectives_through_real_exit() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::game_session::{GameCommand, GameSession};
    use crate::native_world::{Mc2DispatchEvent, player_lifecycle_tests::{rival_test_sim as sim,
        rival_test_sim_mut as sim_mut}};
    let package = std::env::var_os("MC_LEVEL8_PACKAGE").map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/ul-buthnen-source-closure-20261114/level8"));
    let definition = load_mc2_campaign(&package, &crate::game_definition::default_catalog().unwrap()).unwrap();
    let mut game = GameSession::open(&Mc2CampaignAdapter, std::sync::Arc::new(definition), 2208,
        Mc2CampaignInputs::new(&package)).unwrap();
    complete_ul_buthnen_objectives_for_test(&mut game);
    let player = sim(&game).state.entity_index;
    let (_, exit_position) = sim(&game).campaign_exit_objective_target().expect("real exit");
    sim_mut(&mut game).set_level2_entity_position(player, exit_position).unwrap();
    for _ in 0..16 {
        if sim(&game).level_exit_sequence().active { break; }
        game.advance(game.identity().guard(), GameCommand::default()).unwrap();
        assert!(sim(&game).dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
    }
    assert!(sim(&game).level_exit_sequence().active);
    for _ in 0..700 {
        if sim(&game).level_exit_sequence().complete { break; }
        game.advance(game.identity().guard(), GameCommand::default()).unwrap();
        assert!(sim(&game).dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
    }
    assert!(sim(&game).level_exit_sequence().complete);
    assert_eq!(sim(&game).level_exit_sequence().result_code, 10);
    assert_eq!(sim(&game).source_exit_result_byte(), 0x0a);
}

struct EmptyRoutes;

struct PointRoute(crate::native_world::creatures::archer::ArcherStageDescriptor);

impl StageRuntime for PointRoute {
    fn descriptor(&self, index: u8) -> Option<super::super::archer::ArcherStageDescriptor> {
        (index == 1).then_some(self.0)
    }
    fn bind(&mut self, index: u8) -> Option<super::super::archer::ArcherStageDescriptor> {
        self.descriptor(index)
    }
    fn rearm_model_relations(&mut self) {}
}

impl StageRuntime for EmptyRoutes {
    fn descriptor(&self, _index: u8) -> Option<super::super::archer::ArcherStageDescriptor> {
        None
    }

    fn bind(&mut self, _index: u8) -> Option<super::super::archer::ArcherStageDescriptor> {
        None
    }

    fn rearm_model_relations(&mut self) {}
}

fn terrain() -> SessionTerrain {
    crate::native_world::tests::level_two_campaign_session()
        .canonical_terrain()
        .clone()
}

fn actor(action: u8) -> EntitySnapshot {
    let mut row = crate::native_world::cave_features::new_event(
        7,
        19,
        Position {
            x: 20_000,
            y: 20_000,
            z: 0,
        },
    );
    row.class = 5;
    row.model = SOURCE_MODEL;
    row.record[67] = 5;
    row.record[68] = SOURCE_MODEL;
    row.set_source_action(action);
    row.set_life(RULES.max_life);
    row.set_max_life(RULES.max_life);
    row.set_source_roll(0);
    row.set_phase(32);
    row.record[61] = 64;
    row.record[92..94].copy_from_slice(&RULES.collision_half_height.to_le_bytes());
    row
}

fn exhaust(arena: &mut crate::campaign::source_arena::SourceArena) {
    for row in &mut arena.entities {
        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    }
    arena.economy.exhaust_new_event_capacity_for_test();
    arena.rebuild_source_free_arena();
}

#[test]
fn constructor_binds_the_source_visual_descriptor_and_collision_fov() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let particles = &simulation.session.definition.presentation.particles;
    let row = super::mc2::construct_row(
        7,
        19,
        Position {
            x: 20_000,
            y: 20_000,
            z: 0,
        },
        0,
        particles,
    )
    .unwrap();
    assert_eq!(row.parameter_relation, Some(DESCRIPTOR));
    assert_eq!(row.particle_parameter_index, Some(311));
    assert_eq!(row.fov, 0);
    assert_eq!(row.array_fov(), RULES.collision_half_height);
    assert_eq!((row.action, row.record[71]), (169, 64));
    assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 0);
}

fn target(slot: u16) -> Target {
    Target {
        slot,
        source_id: slot,
        class: 3,
        model: 0,
        life: 10_000,
        position: Position {
            x: 20_128,
            y: 20_000,
            z: 0,
        },
        ..Target::default()
    }
}

#[test]
fn summoned_target_scan_compares_source_squared_distances() {
    let terrain = terrain();
    let mut parent = target(9);
    parent.position = Position {
        x: 20_000,
        y: 20_000,
        z: 0,
    };
    let mut farther = target(12);
    farther.position = Position {
        x: 20_100,
        y: 20_009,
        z: 0,
    };
    let mut closer = target(13);
    closer.position = Position {
        x: 20_100,
        y: 20_000,
        z: 0,
    };
    assert_eq!(
        crate::legacy_sqrt(super::xy(parent.position, farther.position)),
        crate::legacy_sqrt(super::xy(parent.position, closer.position))
    );
    let targets = [parent, farther, closer];
    let mut row = actor(175);
    row.set_phase(8);
    row.set_control_flags(row.control_flags | 0x800);
    row.record[77] = 16;
    put(&mut row, 44, parent.slot);
    put(&mut row, 50, 40);
    let mut without_closer = row.clone();
    let mut routes = EmptyRoutes;
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &[12, 13],
        model21: &[],
        families: &[],
        runtime: &mut routes,
    };
    step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(
        u16::from_le_bytes([row.record[154], row.record[155]]),
        closer.slot
    );
    let mut routes = EmptyRoutes;
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &[12],
        model21: &[],
        families: &[],
        runtime: &mut routes,
    };
    step(
        &mut Actor {
            row: &mut without_closer,
        },
        &mut context,
    )
    .unwrap();
    assert_eq!(
        u16::from_le_bytes([without_closer.record[154], without_closer.record[155]]),
        farther.slot
    );
}

#[test]
fn authored_model_route_acquires_only_frozen_family_members() {
    let terrain = terrain();
    let descriptor = super::super::archer::ArcherStageDescriptor {
        kind: 3,
        flags: 2,
        next: 0,
        point: Position::default(),
        actor: 0,
        model: 18,
        scalar: 0,
    };
    let mut enemy = target(12);
    enemy.class = 5;
    enemy.model = 18;
    let targets = [enemy];
    let mut row = actor(175);
    row.set_phase(8);
    row.set_control_flags(row.control_flags | 0x800);
    row.record[76] = 1;
    row.record[77] = 3;
    let mut absent = row.clone();
    let mut families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    families[18].push(enemy.slot);
    let mut routes = PointRoute(descriptor);
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &[],
        model21: &[],
        families: &families,
        runtime: &mut routes,
    };
    step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(
        (
            row.action,
            row.record[77],
            u16::from_le_bytes([row.record[154], row.record[155]])
        ),
        (170, 10, enemy.slot)
    );
    let mut routes = PointRoute(descriptor);
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &[],
        model21: &[],
        families: &[],
        runtime: &mut routes,
    };
    step(&mut Actor { row: &mut absent }, &mut context).unwrap();
    assert_eq!(
        (
            absent.action,
            absent.record[77],
            u16::from_le_bytes([absent.record[154], absent.record[155]])
        ),
        (175, 3, 0)
    );
}

#[test]
fn authored_point_route_steers_and_hostile_damage_interrupts_it() {
    let terrain = terrain();
    let descriptor = super::super::archer::ArcherStageDescriptor {
        kind: 1,
        flags: 0,
        next: 0,
        point: Position {
            x: 30_000,
            y: 20_000,
            z: 0,
        },
        actor: 0,
        model: 0,
        scalar: 0,
    };
    let enemy = target(12);
    let mut row = actor(175);
    row.set_phase(8);
    row.set_control_flags(row.control_flags | 0x800);
    row.set_source_roll(777);
    row.record[76] = 1;
    row.record[77] = 1;
    let mut hit = row.clone();
    hit.record[98..102].copy_from_slice(&50_i32.to_le_bytes());
    put(&mut hit, 102, enemy.slot);
    let targets = [enemy];
    let mut routes = PointRoute(descriptor);
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &[12],
        model21: &[],
        families: &[],
        runtime: &mut routes,
    };
    step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(
        row.source_roll(),
        heading_for_positions(row.position, descriptor.point) as i16
    );
    assert_eq!(row.action, 175);
    let mut routes = PointRoute(descriptor);
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &[12],
        model21: &[],
        families: &[],
        runtime: &mut routes,
    };
    step(&mut Actor { row: &mut hit }, &mut context).unwrap();
    assert_eq!(hit.life, RULES.max_life - 50);
    assert_eq!(
        (
            hit.action,
            hit.record[77],
            u16::from_le_bytes([hit.record[154], hit.record[155]])
        ),
        (170, 10, enemy.slot)
    );
}

#[test]
fn stage_follow_keeps_hidden_parent_pose_and_hides_after_dead_parent_pose() {
    let terrain = terrain();
    let mut parent = target(9);
    parent.control_flags |= ENTITY_DRAW_DISABLED_BIT;
    parent.yaw = 123;
    parent.roll = 456;
    let mut row = actor(175);
    row.record[77] = 12;
    put(&mut row, 44, parent.slot);
    let mut dead_parent = parent;
    dead_parent.life = -1;
    let mut dead = row.clone();
    let targets = [parent];
    let mut routes = EmptyRoutes;
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &[9],
        model21: &[],
        families: &[],
        runtime: &mut routes,
    };
    step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert!(!row.drawing_disabled());
    assert_eq!(
        (row.position, row.yaw, row.source_roll()),
        (parent.position, parent.yaw as i16, parent.roll as i16)
    );
    let targets = [dead_parent];
    let mut routes = EmptyRoutes;
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &[9],
        model21: &[],
        families: &[],
        runtime: &mut routes,
    };
    step(&mut Actor { row: &mut dead }, &mut context).unwrap();
    assert!(dead.drawing_disabled());
    assert_eq!(
        (dead.position, dead.yaw, dead.source_roll()),
        (parent.position, parent.yaw as i16, parent.roll as i16)
    );
}

#[test]
fn alliance_adapter_commits_and_clears_linked_blink_flags() {
    let package = std::env::var_os("MC_SETTLEMENT_FULL_PACKAGE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-1pb-16-level2-full-build-v2")
        });
    let mut simulation = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::LEVEL2,
    )
    .expect("owner Level 2 exports constructor counters")
    .simulation()
    .clone();
    let particles = simulation
        .session
        .definition
        .presentation
        .particles
        .to_vec();
    let live = &mut simulation.session.state.live;
    let (arena, runtime, terrain) = (&mut live.economy, &mut live.core.runtime, &mut live.terrain);
    let parent = arena
        .entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.model == 0 && row.life > 0)
        .expect("live owner player");
    let (parent_slot, position) = (parent.index, parent.position);
    let actor = mc2::allocate_authored(
        arena,
        runtime,
        terrain,
        &particles,
        position,
        RULES.max_life,
        None,
        500,
    )
    .unwrap()
    .unwrap();
    let child = mc2::allocate_authored(
        arena,
        runtime,
        terrain,
        &particles,
        position,
        RULES.max_life,
        None,
        501,
    )
    .unwrap()
    .unwrap();
    {
        let row = &mut arena.entities[usize::from(actor)];
        row.set_source_action(175);
        row.set_phase(8);
        row.record[77] = 14;
        put(row, 44, parent_slot);
        put(row, 50, 40);
        put(row, 52, 40);
        put(row, 56, child);
    }
    let mut expired_arena = arena.clone();
    let mut expired_terrain = terrain.clone();
    let mut expired_runtime = runtime.clone();
    put(&mut expired_arena.entities[usize::from(actor)], 50, 1);
    let row = &mut expired_arena.entities[usize::from(child)];
    row.set_control_flags(row.control_flags | 0x0600_0000);
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    mc2::step_row(
        arena,
        runtime,
        terrain,
        &particles,
        actor,
        &[parent_slot],
        &families,
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        arena.entities[usize::from(child)].control_flags & 0x0600_0000,
        0x0400_0000
    );
    mc2::step_row(
        &mut expired_arena,
        &mut expired_runtime,
        &mut expired_terrain,
        &particles,
        actor,
        &[parent_slot],
        &families,
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        expired_arena.entities[usize::from(child)].control_flags & 0x0600_0000,
        0
    );
}

#[test]
fn alliance_stage_follows_parent_target_and_orders_both_feedback_events() {
    let terrain = terrain();
    let mut parent = target(9);
    parent.target = 12;
    let enemy = target(12);
    let mut row = actor(175);
    row.set_phase(1);
    row.set_control_flags(row.control_flags | 0x800);
    row.record[77] = 14;
    put(&mut row, 44, parent.slot);
    put(&mut row, 50, 40);
    put(&mut row, 52, 40);
    row.record[98..102].copy_from_slice(&50_i32.to_le_bytes());
    put(&mut row, 102, enemy.slot);
    let mut allied = enemy;
    allied.parent = parent.slot;
    allied.stage.mode = 14;
    let mut rejected = row.clone();
    rejected.record[98..104].fill(0);
    let targets = [parent, enemy];
    let mut routes = EmptyRoutes;
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &[9, 12],
        model21: &[],
        families: &[],
        runtime: &mut routes,
    };
    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(row.life, RULES.max_life - 50);
    assert_eq!(
        (
            row.action,
            u16::from_le_bytes([row.record[154], row.record[155]])
        ),
        (170, enemy.slot)
    );
    assert_eq!(effects.earlier_feedback, Some((parent.slot, 19)));
    assert_eq!(effects.feedback, Some((parent.slot, 24)));
    let targets = [parent, allied];
    let mut routes = EmptyRoutes;
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &[9, 12],
        model21: &[],
        families: &[],
        runtime: &mut routes,
    };
    let effects = step(&mut Actor { row: &mut rejected }, &mut context).unwrap();
    assert_eq!(
        (
            rejected.action,
            u16::from_le_bytes([rejected.record[154], rejected.record[155]])
        ),
        (175, 0)
    );
    assert_eq!(effects.feedback, None);
}

#[test]
fn alliance_attack_counts_down_and_only_fires_at_parent_selected_target() {
    let terrain = terrain();
    let mut parent = target(9);
    parent.target = 12;
    let enemy = target(12);
    let mut row = actor(170);
    row.set_control_flags(row.control_flags | 0x800);
    row.record[77] = 14;
    put(&mut row, 44, parent.slot);
    put(&mut row, 50, 40);
    put(&mut row, 52, 40);
    put(&mut row, 154, enemy.slot);
    let mut rejected = row.clone();
    let mut rejected_parent = parent;
    rejected_parent.target = 13;
    let targets = [parent, enemy];
    let mut routes = EmptyRoutes;
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &[9, 12],
        model21: &[],
        families: &[],
        runtime: &mut routes,
    };
    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(effects.strike_target, Some(enemy.slot));
    assert_eq!(u16::from_le_bytes([row.record[50], row.record[51]]), 39);
    let targets = [rejected_parent, enemy];
    let mut routes = EmptyRoutes;
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &[9, 12],
        model21: &[],
        families: &[],
        runtime: &mut routes,
    };
    let effects = step(&mut Actor { row: &mut rejected }, &mut context).unwrap();
    assert_eq!(effects.strike_target, None);
    assert_eq!(rejected.action, 169);
    assert_eq!(
        u16::from_le_bytes([rejected.record[50], rejected.record[51]]),
        39
    );
}

#[test]
fn attack_emits_source_strike_for_live_target_and_reclaims_target_loss_to_roam() {
    let terrain = terrain();
    let mut row = actor(170);
    put(&mut row, 154, 12);
    let target = target(12);
    let mut routes = EmptyRoutes;
    let mut ctx = Context {
        terrain: &terrain,
        targets: &[target],
        class3: &[12],
        model21: &[],
        families: &[],
        runtime: &mut routes,
    };
    let effects = step(&mut Actor { row: &mut row }, &mut ctx).unwrap();
    assert_eq!(effects.strike_target, Some(12));

    let mut lost = actor(170);
    put(&mut lost, 154, 99);
    let mut routes = EmptyRoutes;
    let mut ctx = Context {
        terrain: &terrain,
        targets: &[],
        class3: &[],
        model21: &[],
        families: &[],
        runtime: &mut routes,
    };
    let effects = step(&mut Actor { row: &mut lost }, &mut ctx).unwrap();
    let lost_target = u16::from_le_bytes([lost.record[154], lost.record[155]]);
    assert_eq!((lost.action, lost_target, effects.strike_target), (169, 0, None));
}

#[test]
fn source_reset_168_and_171_clear_target_and_restore_record71_before_action169() {
    let terrain = terrain();
    for action in [168, 171] {
        let mut row = actor(action);
        put(&mut row, 154, 12);
        row.record[71] = 0;
        let mut routes = EmptyRoutes;
        let mut ctx = Context {
            terrain: &terrain,
            targets: &[],
            class3: &[],
            model21: &[],
            families: &[],
            runtime: &mut routes,
        };

        step(&mut Actor { row: &mut row }, &mut ctx).unwrap();

        assert_eq!(row.action, 169);
        assert_eq!(row.record[71], 64);
        assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 0);
    }
}

#[test]
fn summoned_parent_loss_expires_before_consuming_damage_with_mode_specific_visual() {
    let terrain = terrain();
    for mode in [13, 16] {
        let mut row = actor(175);
        row.record[77] = mode;
        put(&mut row, 44, 0);
        put(&mut row, 50, 10);
        row.record[98..102].copy_from_slice(&50_i32.to_le_bytes());
        put(&mut row, 102, 12);
        let before_life = row.life;
        let mut routes = EmptyRoutes;
        let mut ctx = Context {
            terrain: &terrain,
            targets: &[],
            class3: &[],
            model21: &[],
            families: &[],
            runtime: &mut routes,
        };
        let effects = step(&mut Actor { row: &mut row }, &mut ctx).unwrap();
        assert!(effects.expire);
        assert_eq!(effects.summon_expire, mode == 13);
        assert_eq!(row.life, before_life);
        assert_eq!(u16::from_le_bytes([row.record[102], row.record[103]]), 12);
    }
}

#[test]
fn summoned_model21_reads_current_reused_parent_then_expires_on_parent_death() {
    let package = std::env::var_os("MC_SETTLEMENT_FULL_PACKAGE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-1pb-16-level2-full-build-v2")
        });
    let mut simulation = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::LEVEL2,
    )
    .expect("owner Level 2 exports constructor counters")
    .simulation()
    .clone();
    let particles = simulation
        .session
        .definition
        .presentation
        .particles
        .to_vec();
    let live = &mut simulation.session.state.live;
    let (arena, runtime, terrain) = (&mut live.economy, &mut live.core.runtime, &mut live.terrain);
    let parent_row = arena
        .entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.model == 0 && row.life > 0)
        .expect("owner session has a living parent");
    let (parent_slot, position, launch_yaw) = (parent_row.index, parent_row.position, parent_row.yaw);
    let parent = arena.handle(parent_slot).unwrap();
    let slot = mc2::allocate_summoned(
        arena,
        runtime,
        terrain,
        &particles,
        position,
        parent_slot,
        0, // Configured no-target follow keeps the source mode16 path active.
        launch_yaw,
    )
    .unwrap()
    .expect("model21 source allocation");
    let child = arena.handle(slot).unwrap();
    assert_eq!(arena.entities[usize::from(slot)].source_player_entity_index(), 0,
        "boss source sets target, not playerEntityIndex");
    assert!(arena.bind_carried_creature_parent(child, parent));
    arena.entities[usize::from(slot)].record[77] = 16;
    let mut families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    families[21].push(slot);

    let mut living_arena = arena.clone();
    let mut living_runtime = runtime.clone();
    let mut living_terrain = terrain.clone();
    let living = mc2::step_row(
        &mut living_arena,
        &mut living_runtime,
        &mut living_terrain,
        &particles,
        slot,
        &[],
        &families,
        Default::default(),
    )
    .unwrap();
    assert!(!living.expire);
    assert!(!living_arena.entities[usize::from(slot)].drawing_disabled());
    assert_eq!(living_arena.creature_parent(child), Some(parent));

    let replacement = arena.entities[usize::from(parent_slot)].clone();
    arena.entities[usize::from(parent_slot)].reclaim_source_row();
    arena.economy.release_slot(parent_slot, None);
    assert_eq!(arena.allocate_source_slot(), Some(parent_slot));
    arena.entities[usize::from(parent_slot)] = replacement;
    assert_ne!(arena.handle(parent_slot).unwrap(), parent);
    assert_eq!(arena.creature_parent_binding(child), Some(parent));
    assert!(arena.creature_parent(child).is_none());
    let continued = mc2::step_row(
        arena,
        runtime,
        terrain,
        &particles,
        slot,
        &[],
        &families,
        Default::default(),
    )
    .unwrap();
    assert!(!continued.expire, "source1E580 follows the positive-life current fixed address");
    assert!(!arena.entities[usize::from(slot)].drawing_disabled());
    assert!(arena.resolve(child).is_some());
    assert_eq!((arena.entities[usize::from(slot)].action, arena.entities[usize::from(slot)].record[77]),
        (175, 16), "the positive replacement visit remains in the source follow callback");
    arena.entities[usize::from(parent_slot)].set_life(-1);
    let expired = mc2::step_row(
        arena, runtime, terrain, &particles, slot, &[], &families, Default::default(),
    ).unwrap();
    assert!(expired.expire);
    assert!(
        !expired.summon_expire,
        "source mode16 only emits its common expiry effect"
    );
    assert!(arena.entities[usize::from(slot)].drawing_disabled());
    assert!(
        arena.resolve(child).is_some(),
        "hidden child remains until the next source boundary"
    );
    arena
        .rebuild_source_arena_with_terrain(terrain, &[])
        .unwrap();
    assert!(arena.resolve(child).is_none());
}

#[test]
fn boss_summon_acquires_live_target_and_dead_launch_hides() {
    let terrain = terrain();
    let mut parent = target(5);
    parent.source_id = 5;
    let enemy = target(12);
    let targets = [parent, enemy];
    let mut routes = EmptyRoutes;
    let mut ctx = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &[5, 12],
        model21: &[],
        families: &[],
        runtime: &mut routes,
    };
    let mut following = actor(175);
    following.record[77] = 16;
    put(&mut following, 44, 5);
    put(&mut following, 50, 10);
    let effects = step(
        &mut Actor {
            row: &mut following,
        },
        &mut ctx,
    )
    .unwrap();
    assert!(!effects.expire);
    assert_eq!(following.action, 170);
    assert_eq!(
        u16::from_le_bytes([following.record[154], following.record[155]]),
        12
    );

    let mut dead_launch = actor(175);
    dead_launch.record[77] = 17;
    dead_launch.set_life(-1);
    step(
        &mut Actor {
            row: &mut dead_launch,
        },
        &mut ctx,
    )
    .unwrap();
    assert!(dead_launch.drawing_disabled());
}

#[test]
fn dead_boss_launch_keeps_its_map_link_until_source_reclaim() {
    let mut simulation = crate::native_world::tests::level_two_campaign_session();
    let particles = simulation
        .session
        .definition
        .presentation
        .particles
        .to_vec();
    let live = &mut simulation.session.state.live;
    let (arena, runtime, terrain) = (&mut live.economy, &mut live.core.runtime, &mut live.terrain);
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 0,
    };
    let slot = arena
        .allocate_source_slot_with_terrain(terrain)
        .expect("source slot");
    let mut row =
        super::mc2::construct_row(slot, arena.new_event_rng(slot), position, 0, &particles)
            .expect("model21 constructor");
    row.set_source_action(175);
    row.record[77] = 17;
    row.set_life(-1);
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false).unwrap();
    let families = std::array::from_fn(|_| Vec::new());
    super::mc2::step_row(arena, runtime, terrain, &particles, slot, &[], &families, Default::default())
        .expect("dead launch callback");
    assert!(arena.entities[usize::from(slot)].drawing_disabled());
    assert!(arena.entities[usize::from(slot)].active);
    assert!(arena.economy.map_chains.is_linked(slot));
}

#[test]
fn owner_objective_gate_publishes_model21_into_normal_scheduler() {
    use crate::PlayerCommand;
    use crate::campaign::source_arena::CreatureRootOrigin;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::native_world::Mc2DispatchEvent;

    let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-bhq5-cave-reset-v2-level8");
    let campaign = CampaignSession::open(package, CampaignSelection::ordinary(8).unwrap())
        .expect("owner Level8 package for source group26");
    let mut session = campaign.simulation().clone();
    let gate = session.entities()[13].clone();
    assert_eq!(
        (gate.class, gate.model, gate.action, gate.source_id),
        (11, 32, 32, 26)
    );
    let gate_stage = usize::try_from(gate.level_gate).unwrap();
    assert!(gate_stage < usize::from(session.runtime().stage_index));

    // The real owner package starts this objective incomplete. The switch
    // must not publish until the existing objective input is satisfied.
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            source_slot: 13,
            group: 26,
            ..
        }
    )));
    for objective in 0..session.runtime.objectives.len() {
        session
            .runtime
            .set_level2_objective_stage_selector_for(objective, gate_stage, 2);
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().any(|event| matches!(event,
        Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 13, group: 26, authored_indices, .. }
            if authored_indices.contains(&113) && authored_indices.contains(&140)
    )), "owner objective gate must produce its authored model21 group: {:?}", session.dispatch_events());
    let slots = session
        .entities()
        .iter()
        .filter_map(|row| {
            let identity = session.creature_identity_for_source_slot(row.index)?;
            matches!(
                identity.origin,
                CreatureRootOrigin::AuthoredRecord {
                    authored_record: 113 | 140
                }
            )
            .then_some((row.index, row.phase))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        slots.len(),
        2,
        "owner model21 rows 113 and 140 must be live"
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    for (slot, phase) in slots {
        let actor = &session.entities()[usize::from(slot)];
        assert!(actor.active && !actor.drawing_disabled());
        assert_eq!(
            actor.phase,
            phase.wrapping_add(1),
            "normal scheduler visits model21 slot {slot}"
        );
    }
}

#[test]
fn jump_states3_through7_apply_velocity_then_subtract_source_gravity() {
    let terrain = terrain();
    let mut routes = EmptyRoutes;
    let ctx = Context {
        terrain: &terrain,
        targets: &[],
        class3: &[],
        model21: &[],
        families: &[],
        runtime: &mut routes,
    };
    for state in 3..=7 {
        let mut row = actor(170);
        row.position.z = terrain.ground_height(row.position).wrapping_add(1_024);
        let before_z = row.position.z;
        row.record[74] = state;
        row.record[48..50].copy_from_slice(&200_i16.to_le_bytes());
        let velocity = if state == 4 {
            let next_rng = crate::native_world::level2_next_rng16(row.entity_rng);
            RULES
                .jump_impulse_base
                .wrapping_add((next_rng % RULES.jump_impulse_span.max(1)) as i16)
        } else {
            200
        };
        let expected_state = match state {
            3 => 4,
            4 => 5,
            7 => 8,
            _ => state,
        };
        let mut effects = Effects::default();

        jump(&mut Actor { row: &mut row }, &ctx, &mut effects);

        assert_eq!(row.record[74], expected_state, "jump state {state}");
        assert_eq!(
            row.position.z,
            before_z.wrapping_add(velocity),
            "jump state {state}"
        );
        assert_eq!(
            i16::from_le_bytes([row.record[84], row.record[85]]),
            row.position.z,
            "jump state {state} source record"
        );
        assert_eq!(
            i16::from_le_bytes([row.record[48], row.record[49]]),
            velocity - 42,
            "jump state {state}"
        );
    }
}

#[test]
fn jump_states0_and1_mark_the_next_movement_hold_with_bit_800() {
    let terrain = terrain();
    for state in [0, 1] {
        let mut row = actor(170);
        row.record[74] = state;
        row.record[72] = 1;
        let mut routes = EmptyRoutes;
        let ctx = Context {
            terrain: &terrain,
            targets: &[],
            class3: &[],
            model21: &[],
            families: &[],
            runtime: &mut routes,
        };
        let mut effects = Effects::default();

        jump(&mut Actor { row: &mut row }, &ctx, &mut effects);

        assert_ne!(row.control_flags & 0x800, 0, "jump state {state}");
        let held_position = row.position;
        move_actor(&mut Actor { row: &mut row }, &ctx, &mut effects);
        assert_eq!(row.position, held_position, "jump state {state}");
        assert_eq!(row.control_flags & 0x800, 0, "jump state {state}");
    }
}

#[test]
fn source_fireball_adapter_commits_then_shared_consumer_advances_and_exhaustion_has_no_feedback() {
    let mut simulation = crate::native_world::tests::level_two_campaign_session();
    let live = &mut simulation.session.state.live;
    let arena = &mut live.economy;
    let (target_slot, target_position, target_source_id) = arena
        .entities
        .iter()
        .find(|row| row.active && row.life >= 0 && row.class == 3 && row.model == 0)
        .map(|row| (row.index, row.position, row.source_id))
        .expect("Level 2 fixture retains a live player target");
    let owner = arena
        .entities
        .iter()
        .find(|row| {
            row.active
                && row.index != 0
                && row.index != target_slot
                && row.source_id != target_source_id
                && arena.economy.live_arena_kind(row.index).is_none()
        })
        .map(|row| (row.index, row.source_id))
        .expect("Level 2 fixture retains a distinct source owner");
    let mut position = target_position;
    position.x = position.x.wrapping_sub(512);
    let initial_position = Position { z: position.z.wrapping_sub(200), ..position };
    let source = &mut arena.entities[usize::from(owner.0)];
    source.set_position(initial_position);
    put(source, 92, 200);
    put(source, 154, target_slot);
    let before = arena.economy.fireball_projectiles().len();
    let (arena, terrain) = (&mut live.economy, &mut live.terrain);
    terrain.map_type = crate::native_world::MapType::Night;
    let mut unlit_arena = arena.clone();
    unlit_arena.dynamic_lighting_enabled = false;
    let mut unlit_terrain = terrain.clone();
    let (birth, feedback) = super::mc2::launch_strike(
        arena, terrain, owner.0, target_slot, Default::default(),
    ).unwrap();
    assert!(birth.is_some() && feedback);
    let projectile = arena
        .economy
        .fireball_projectiles()
        .iter()
        .skip(before)
        .find(|projectile| {
            projectile.origin == crate::native_world::FireballProjectileOrigin::Creature21
        })
        .copied()
        .expect("successful model21 NewEvent is registered in the shared Fireball arena");
    assert_eq!((projectile.source_class, projectile.source_model), (9, 0));
    assert_eq!(projectile.owner_entity_index, owner.1);
    assert_eq!((projectile.target_class, projectile.target_model), (3, 0));
    assert_eq!(projectile.target_source_entity_index, target_slot);
    assert_eq!(projectile.source_subspell_index, RULES.attack_damage as u16);
    assert_eq!(projectile.source_mana, 50);
    assert!(
        arena
            .dynamic_lighting
            .active_slots()
            .any(|(_, light)| light.linked
                && light.source_entity_index == projectile.source_entity_index
                && light.position == initial_position)
    );
    let (birth, feedback) = super::mc2::launch_strike(
        &mut unlit_arena, &mut unlit_terrain, owner.0, target_slot, Default::default(),
    ).unwrap();
    assert!(birth.is_some() && feedback);
    let unlit_projectile = unlit_arena.economy.fireball_projectiles().last().unwrap();
    assert!(
        !unlit_arena
            .dynamic_lighting
            .active_slots()
            .any(|(_, light)| light.linked
                && light.source_entity_index == unlit_projectile.source_entity_index)
    );
    assert_eq!(
        arena.entities[usize::from(target_slot)].record[58..60],
        100_i16.to_le_bytes()
    );

    let consumer = {
        let (economy, entities) = (&mut arena.economy, &arena.entities);
        economy.step_fireball_projectile_slot(projectile.source_entity_index, terrain, entities)
    };
    assert!(consumer.live || consumer.impact_slot.is_some());

    let feedback = arena.entities[usize::from(target_slot)].record[58..60].to_vec();
    exhaust(arena);
    let before = arena.economy.fireball_projectiles().len();
    assert_eq!(super::mc2::launch_strike(
        arena, terrain, owner.0, target_slot, Default::default(),
    ).unwrap(), (None, false));
    assert_eq!(arena.economy.fireball_projectiles().len(), before);
    assert_eq!(
        arena.entities[usize::from(target_slot)].record[58..60],
        feedback
    );
}

#[test]
fn water_splash_adapter_materializes_source_model5_and_reclaims_after_its_action5_lifecycle() {
    let mut simulation = crate::native_world::tests::level_two_campaign_session();
    let particles = simulation.session.definition.presentation.particles.to_vec();
    let live = &mut simulation.session.state.live;
    let position = live
        .economy
        .entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.model == 0)
        .expect("Level 2 fixture retains the player source row")
        .position;
    let (arena, terrain) = (&mut live.economy, &mut live.terrain);
    let splash_slot = super::mc2::materialize_water_splash(arena, terrain, position)
        .unwrap()
        .expect("available NewEvent slot creates the real class10/model5 splash");
    let splash = arena
        .economy
        .authored_model5_water_splashes()
        .iter()
        .find(|source| source.source_entity_index == splash_slot)
        .copied()
        .expect("splash is held by the shared typed source carrier");
    assert_eq!(
        (splash.source_class, splash.model, splash.action),
        (10, 5, 5)
    );
    assert_eq!(
        (splash.max_life, splash.life, splash.sprite_index),
        (8, 8, 244)
    );
    assert_eq!(splash.actual_speed, 16, "NewAdd0A05 retains NewEvent's speed");
    assert_eq!(splash.position.z, terrain.ground_height(position));

    for _ in 0..9 {
        assert!(
            arena
                .economy
                .step_authored_model5_water_splash_slot(splash_slot, &particles)
        );
    }
    let live_splash = arena
        .economy
        .authored_model5_water_splashes()
        .iter()
        .find(|source| source.source_entity_index == splash_slot)
        .copied()
        .expect("action5 keeps the source through its final visible callback");
    assert_eq!(live_splash.life, -1);
    assert_eq!(live_splash.last_sound_id, 27);
    assert!(live_splash.drawing_enabled);

    assert!(
        arena
            .economy
            .step_authored_model5_water_splash_slot(splash_slot, &particles)
    );
    arena.economy.reclaim_inactive_slots_at_source_boundary();
    assert!(
        !arena
            .economy
            .authored_model5_water_splashes()
            .iter()
            .any(|source| source.source_entity_index == splash_slot)
    );

    exhaust(arena);
    let before = arena.economy.authored_model5_water_splashes().len();
    assert!(
        super::mc2::materialize_water_splash(arena, terrain, position)
            .unwrap()
            .is_none()
    );
    assert_eq!(arena.economy.authored_model5_water_splashes().len(), before);
}

#[test]
fn source_disabled_action174_is_rejected_instead_of_reporting_an_empty_success() {
    assert!(enabled(169));
    assert!(!enabled(174));
    let terrain = terrain();
    let mut row = actor(174);
    let mut routes = EmptyRoutes;
    let mut ctx = Context {
        terrain: &terrain,
        targets: &[],
        class3: &[],
        model21: &[],
        families: &[],
        runtime: &mut routes,
    };
    assert!(step(&mut Actor { row: &mut row }, &mut ctx).is_err());
}

#[test]
fn configured_stage_signed_point_guard_and_descriptorless_wander() {
    let terrain = terrain();
    let point = Position { x: 30_000, y: 22_000, z: 0 };
    let descriptor = super::super::archer::ArcherStageDescriptor {
        kind: 1, flags: 0, next: 0, point, actor: 0, model: 0, scalar: 0,
    };
    let mut row = actor(175);
    row.set_position(Position { x: 32760, y: 20_000, z: 0 });
    row.set_phase(8);
    row.set_control_flags(row.control_flags | 0x800);
    row.set_source_overlap_half_extents(128, 128);
    row.record[76] = 1;
    row.record[77] = 1;
    let mut peer = target(42);
    peer.class = 5;
    peer.model = SOURCE_MODEL;
    peer.position = Position { x: 32775, y: 20_000, z: 0 };
    let rows = [peer];
    let mut routes = PointRoute(descriptor);
    let mut ctx = Context { terrain: &terrain, targets: &rows, class3: &[], model21: &[42],
        families: &[], runtime: &mut routes };
    step(&mut Actor { row: &mut row }, &mut ctx).unwrap();
    assert_eq!(row.source_roll(), heading_for_positions(row.position, point) as i16,
        "1DDA0 treats actor/peer coordinates as separate signed values");

    let mut guard = actor(175);
    let guard_position = Position { x: 100, y: 20_000, z: 0 };
    guard.set_position(guard_position);
    guard.set_phase(RULES.target_scan_period);
    guard.set_control_flags(guard.control_flags | 0x800);
    guard.record[76] = 1;
    guard.record[77] = 2;
    let mut enemy = target(12);
    enemy.position = Position { x: 65_400, ..guard_position };
    guard.set_yaw(heading_for_positions(guard_position, enemy.position) as i16);
    let rows = [enemy];
    let mut routes = PointRoute(super::super::archer::ArcherStageDescriptor {
        kind: 2, point: guard_position, ..descriptor
    });
    let mut ctx = Context { terrain: &terrain, targets: &rows, class3: &[12], model21: &[],
        families: &[], runtime: &mut routes };
    step(&mut Actor { row: &mut guard }, &mut ctx).unwrap();
    assert_eq!((guard.action, guard.record[77], u16::from_le_bytes([guard.record[154], guard.record[155]])),
        (175, 2, 0), "1DBF0 promotes X before subtracting rather than wrapping");

    for mode in 6..=9 {
        let mut source = actor(175);
        source.record[76] = 0;
        source.record[77] = mode;
        source.record[98..102].copy_from_slice(&1_i32.to_le_bytes());
        put(&mut source, 102, enemy.slot);
        let mut routes = EmptyRoutes;
        let mut ctx = Context { terrain: &terrain, targets: &rows, class3: &[], model21: &[],
            families: &[], runtime: &mut routes };
        step(&mut Actor { row: &mut source }, &mut ctx).unwrap();
        assert_eq!((source.life, source.action, source.record[77], u16::from_le_bytes([source.record[154], source.record[155]])),
            (RULES.max_life - 1, 170, 10, enemy.slot), "1E1C0 does not read a descriptor for mode {mode}");
    }
}

#[test]
fn configured_water_landing_uses_retained_source_prediction_and_cave_ceiling() {
    // Explicit terrain/actor exception control, separate from either real
    // authored or boss producer. The shared predicted axis is not reset by a
    // held sub1B8C0 movement attempt.
    let mut water = terrain();
    water.map_type = crate::native_world::MapType::Day;
    let mut row = actor(169);
    let tile = crate::native_world::terrain_index(row.position.x.to_be_bytes()[0], row.position.y.to_be_bytes()[0]);
    water.planes.terrain_types[tile] = 0;
    row.position.z = water.ground_height(row.position);
    row.set_position(row.position);
    row.record[74] = 8;
    row.set_control_flags(row.control_flags | 0x800);
    let prior = Position { x: row.position.x.wrapping_add(768), y: row.position.y.wrapping_add(512), z: 321 };
    water.set_source_predicted_position(prior);
    let mut routes = EmptyRoutes;
    let ctx = Context { terrain: &water, targets: &[], class3: &[], model21: &[],
        families: &[], runtime: &mut routes };
    let mut effects = Effects::default();
    move_actor(&mut Actor { row: &mut row }, &ctx, &mut effects);
    jump(&mut Actor { row: &mut row }, &ctx, &mut effects);
    assert_eq!(water.source_predicted_position(), prior);
    assert_eq!(effects.water_splash, Some(prior), "water splash uses retained global XY, not landing pose");
    let (held, continuation) = effects.water_splash_tail.expect("source resumes after the splash allocation");
    finish_water_splash(&mut Actor { row: &mut row }, &ctx, held, continuation);
    assert_eq!((row.record[74], row.actual_speed), (10, RULES.roam_water_speed));

    let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-bhq5-cave-reset-v2-level8");
    let cave = crate::campaign::CampaignSession::open(package,
        crate::campaign::spec::CampaignSelection::ordinary(8).unwrap())
        .expect("real Level8 Cave terrain").simulation().canonical_terrain().clone();
    let mut row = actor(170);
    let limit = cave.ceiling_height(row.position).expect("Cave ceiling").wrapping_sub(row.array_fov());
    row.position.z = limit;
    row.set_position(row.position);
    row.record[74] = 3;
    row.record[48..50].copy_from_slice(&200_i16.to_le_bytes());
    let mut routes = EmptyRoutes;
    let ctx = Context { terrain: &cave, targets: &[], class3: &[], model21: &[],
        families: &[], runtime: &mut routes };
    let mut effects = Effects::default();
    jump(&mut Actor { row: &mut row }, &ctx, &mut effects);
    assert_eq!(row.position.z, limit, "Cave collision clamps to the actual ceiling minus fov");
    assert_eq!(i16::from_le_bytes([row.record[48], row.record[49]]), 0,
        "Cave collision clears source vertical velocity");
}

#[test]
fn configured_blocked_water_kills_descriptor65_and_keeps_last_proposal() {
    let mut world = terrain();
    world.map_type = crate::native_world::MapType::Cave;
    world.planes.ground_heights.fill(128);
    world.planes.secondary_heights.fill(0);
    world.planes.terrain_types.fill(0);
    let mut routes = EmptyRoutes;
    let ctx = Context { terrain: &world, targets: &[], class3: &[], model21: &[],
        families: &[], runtime: &mut routes };
    let mut row = actor(170);
    row.parameter_relation = Some(65);
    row.set_source_overlap_half_extents(128, 128);
    put(&mut row, 92, 128);
    row.set_actual_speed(512);
    row.set_control_flags(row.control_flags & !0x800);
    let initial = row.position;
    world.set_source_predicted_position(initial);
    let mut effects = Effects::default();
    move_actor(&mut Actor { row: &mut row }, &ctx, &mut effects);
    assert_eq!((row.life, effects.population_decrements), (-1, 1),
        "all blocked water attempts kill even descriptor65 with behavior bit1 clear");
    assert_eq!(row.position, initial, "all four proposals were blocked");
    assert_ne!(world.source_predicted_position(), initial,
        "the shared scratch retains the last rejected proposal");
}

#[test]
fn ordinary_level8_model21_jumps_and_fires_into_real_player_damage() {
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::campaign::source_arena::CreatureRootOrigin;
    use crate::native_world::{area_damage, Mc2DispatchEvent};
    use crate::PlayerCommand;

    let package = std::env::var_os("MC_LEVEL8_PACKAGE").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq5-cave-reset-v2-level8"),
        std::path::PathBuf::from,
    );
    let mut session = CampaignSession::open(package, CampaignSelection::ordinary(8).unwrap())
        .expect("whole Level8 campaign").simulation().clone();
    let gate = session.entities()[13].clone();
    assert_eq!((gate.class, gate.model, gate.action, gate.source_id), (11, 32, 32, 26));
    let gate_stage = usize::try_from(gate.level_gate).unwrap();
    // Declared objective prerequisite: the real switch remains the producer.
    for objective in 0..session.runtime.objectives.len() {
        session.runtime.set_level2_objective_stage_selector_for(objective, gate_stage, 2);
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().any(|event| matches!(event,
        Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 13, group: 26, authored_indices, .. }
            if authored_indices.contains(&113))));
    let actor = session.entities().iter().find_map(|row| {
        let identity = session.creature_identity_for_source_slot(row.index)?;
        ((row.class, row.model) == (5, SOURCE_MODEL)
            && matches!(identity.origin, CreatureRootOrigin::AuthoredRecord { authored_record: 113 }))
            .then(|| session.source_arena().handle(row.index)).flatten()
    }).expect("actual group26 model21 record113");
    let player = session.state.entity_index;
    let player_handle = session.source_arena().handle(player).unwrap();
    {
        let live = &mut session.session.state.live;
        let parent = &mut live.economy.entities[usize::from(player)];
        parent.set_max_life(200_000);
        parent.set_life(200_000);
        let parent = parent.clone();
        live.player_life = parent.life;
        live.shared_spells.sync_player_entity_from_authority(&parent);
    }
    let mut distant = session.entities()[usize::from(actor.source_slot)].position;
    distant.x = distant.x.wrapping_add(0x6000);
    session.set_level2_entity_position(player, distant).unwrap();
    for _ in 0..128 {
        if session.session.state.live.player_protection_ticks == 0 { break; }
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(!session.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })));
    }
    assert_eq!(session.session.state.live.player_protection_ticks, 0);
    assert_eq!(session.source_arena().handle(actor.source_slot), Some(actor));
    let row = session.entities()[usize::from(actor.source_slot)].clone();
    let source_id = row.source_id;
    let mut nearby = row.position;
    crate::move_entity(&mut nearby, row.yaw, 0, 1024);
    nearby.z = session.canonical_terrain().ground_height(nearby).wrapping_add(512);
    session.set_level2_entity_position(player, nearby).unwrap();
    area_damage::start_player_damage_trace(player_handle);
    area_damage::start_effect_damage_trace(player);
    area_damage::start_fireball_launch_trace();
    let mut callbacks = Vec::new();
    let mut terminals = Vec::new();
    let mut consumptions = Vec::new();
    let mut intercepted = false;
    let mut after_interception = 0;
    let mut airborne = false;
    let mut landed = false;
    let mut saw_jump_audio = false;
    let mut saw_actor_dto = false;
    let mut journal_next = session.effect_journal().records().last().map_or(0, |record| record.sequence + 1);
    for _ in 0..500 {
        mc2::start_callback_trace();
        crate::native_world::start_fireball_terminal_trace();
        session.step_outer_turn(PlayerCommand::default(), 0);
        let visits = mc2::take_callback_trace();
        saw_jump_audio |= session.source_arena().handle(actor.source_slot) == Some(actor)
            && visits.iter().any(|visit| visit.actor == actor)
            && session.effect_journal().records().iter().any(|record| record.sequence >= journal_next
                && matches!(record.effect, crate::campaign::journal::CampaignEffect::Flight(
                    crate::FlightEvent::PrepareSound { audio_site: crate::MissionAudioSite::Model21Jump,
                        entity_index, player_index: -1, sound_id: 42 }) if entity_index == actor.source_slot)
                && record.audio.is_some());
        if let Some(record) = session.effect_journal().records().last() {
            journal_next = record.sequence + 1;
        }
        if !saw_actor_dto && session.source_arena().handle(actor.source_slot) == Some(actor) {
            let row = &session.entities()[usize::from(actor.source_slot)];
            if !row.drawing_disabled() {
                saw_actor_dto = session.campaign_entity_presentations(0).unwrap().iter().any(|view|
                    view.entity_index == actor.source_slot && view.source_id == row.source_id
                        && Some(view.render_parameter_index) == row.particle_parameter_index);
            }
        }
        for visit in visits.iter().filter(|visit| visit.actor == actor) {
            if let Some(row) = &visit.after {
                let ground = session.canonical_terrain().ground_height(row.position);
                airborne |= row.position.z > ground && (3..=8).contains(&row.record[74]);
                landed |= airborne && (row.record[74] == 9 || row.record[74] == 10);
            }
        }
        callbacks.extend(visits);
        terminals.extend(crate::native_world::take_fireball_terminal_trace());
        consumptions.extend(area_damage::take_player_damage_consumptions());
        assert!(!session.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "actual model21 callback failed: {:?}", session.dispatch_events());
        // One declared player-only interception after actual projectile birth.
        if !intercepted && let Some(shot) = callbacks.iter().find(|visit| visit.actor == actor
            && visit.projectile_packet.as_ref().is_some_and(|packet| packet.target_source_entity_index == player))
            .and_then(|visit| visit.projectile)
            && let Some(current) = session.economy().fireball_projectiles().iter()
                .find(|row| row.active && row.source_entity_index == shot.source_slot)
            && session.source_arena().handle(shot.source_slot) == Some(shot)
        {
            let mut contact = current.position;
            crate::move_entity(&mut contact, current.yaw, current.pitch, current.actual_speed);
            session.set_level2_entity_position(player, contact).unwrap();
            intercepted = true;
        }
        if intercepted {
            after_interception += 1;
            if after_interception >= 24 && landed && saw_jump_audio && saw_actor_dto { break; }
        }
    }
    let births = area_damage::take_fireball_impact_birth_trace();
    let _ = area_damage::take_fireball_launch_trace();
    let damage = area_damage::take_effect_damage_trace();
    assert!(airborne && landed && intercepted, "actual authored model21 must jump, land and launch toward player");
    assert!(saw_jump_audio && saw_actor_dto,
        "the same authored generation must reach original-particle DTO and prepared source jump audio: audio={saw_jump_audio} dto={saw_actor_dto}");
    assert!(terminals.iter().any(|terminal| callbacks.iter().any(|visit| visit.actor == actor
        && visit.projectile == Some(terminal.projectile)
        && visit.projectile_packet.as_ref().is_some_and(|packet| packet.source_subspell_index == 500
            && packet.target_source_entity_index == player))
        && terminal.owner == source_id && terminal.target_handle == Some(player_handle)
        && terminal.impact.is_some_and(|impact| births.iter().any(|&(slot, generation, parent, parent_generation, owner)|
            (slot, generation, parent, parent_generation, owner) == (impact.source_slot, impact.generation,
                terminal.projectile.source_slot, terminal.projectile.generation, source_id))
            && damage.iter().any(|receipt| receipt.source_slot == Some(impact.source_slot)
                && receipt.target_generation == player_handle.generation && receipt.source_scoped
                && receipt.pulse.owner == source_id && receipt.pulse.payload == 500
                && receipt.writer_sequence.is_some_and(|sequence|
                    consumptions.iter().any(|consumed| consumed.target == player_handle
                        && consumed.writer.is_some_and(|writer| writer.sequence == sequence
                            && writer.source == area_damage::PlayerMailboxSource::Effect(impact))
                        && Some(consumed.mailbox) == receipt.mailbox_after
                        && consumed.life_after < consumed.life_before))))),
        "actual model21 birth must lead to the same-generation impact and consumed player damage");
}

#[test]
fn ordinary_level8_public_meteor_retires_original_model21() {
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::campaign::source_arena::CreatureRootOrigin;
    use crate::native_world::{area_damage, Mc2DispatchEvent};
    use crate::PlayerCommand;

    let package = std::env::var_os("MC_LEVEL8_PACKAGE").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq5-cave-reset-v2-level8"),
        std::path::PathBuf::from,
    );
    let mut session = CampaignSession::open(package, CampaignSelection::ordinary(8).unwrap())
        .expect("whole Level8 campaign").simulation().clone();
    let player = session.state.entity_index;
    // Explicit entry controls: carried Meteor, normal Castle and player resources;
    // group26 still comes from the real objective-gated switch.
    session.apply_carried_spell_families(&[9], Some(9)).unwrap();
    for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
        session.economy_mut().set_castle_stage_footprint(stage, footprint);
    }
    let site = (1_u16..32).flat_map(|y| (1_u16..32).map(move |x| Position {
        x: x * 2048, y: y * 2048, z: 0,
    })).find(|position| !session.economy().castle_projectile_site_blocked(
        *position, session.entities(), session.canonical_terrain())).expect("normal Castle site");
    let castle = session.economy_mut().build_castle(site, player).expect("normal Castle constructor");
    assert!(session.economy_mut().set_castle_mana_prerequisite_for_test(castle, 200_000));
    session.shared_spells_mut().award_spell_experience(player, 9, 5_000);
    {
        let live = &mut session.session.state.live;
        let parent = &mut live.economy.entities[usize::from(player)];
        parent.set_max_life(200_000); parent.set_life(200_000);
        parent.max_mana = 200_000;
        parent.record[144..148].copy_from_slice(&parent.max_mana.to_le_bytes());
        parent.set_mana(200_000); parent.set_mana_regen(0);
        let parent = parent.clone();
        live.player_life = parent.life;
        live.shared_spells.sync_player_entity_from_authority(&parent);
        live.shared_spells.player_mana = parent.mana;
        live.shared_spells.caster.mana = parent.mana;
        live.shared_spells.sync_typed_spell_parent_caches_from_player_entity();
        if let Some(fire) = live.core.context.spell_fire.as_mut() { fire.caster.mana = parent.mana; }
    }
    let gate = session.entities()[13].clone();
    assert_eq!((gate.class, gate.model, gate.action, gate.source_id), (11, 32, 32, 26));
    let gate_stage = usize::try_from(gate.level_gate).unwrap();
    for objective in 0..session.runtime.objectives.len() {
        session.runtime.set_level2_objective_stage_selector_for(objective, gate_stage, 2);
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    let actor = session.entities().iter().find_map(|row| {
        let identity = session.creature_identity_for_source_slot(row.index)?;
        ((row.class, row.model) == (5, SOURCE_MODEL)
            && matches!(identity.origin, CreatureRootOrigin::AuthoredRecord { authored_record: 113 }))
            .then(|| session.source_arena().handle(row.index)).flatten()
    }).expect("actual group26 model21 record113");
    let ui = crate::SOURCE_SPELL_INDEX_MAP.iter().position(|family| *family == 9).unwrap() as i8;
    session.step_outer_turn(PlayerCommand { opcode: 31, argument_1: ui, argument_2: 2,
        ..Default::default() }, 0);
    let holder = session.shared_spells().local_player().enabled_spell_holders[9];
    assert_ne!(holder, 0);
    let holder_handle = session.source_arena().handle(holder).unwrap();
    let caster_handle = session.source_arena().handle(player).unwrap();
    let current = session.entities()[usize::from(actor.source_slot)].clone();
    assert!(current.life > 0 && current.mana > 0);
    let mut origin = current.position;
    crate::move_entity(&mut origin, 1024, 0, 2048);
    origin.z = session.canonical_terrain().ground_height(current.position).wrapping_add(768);
    session.set_level2_entity_position(player, origin).unwrap();
    session.state.actual_speed = 0;
    session.state.target_speed = 0;
    let (_, pitch) = crate::level1_mission::projectile_guidance_angles(origin, current.position);
    session.set_level2_player_aim_for_test(0, pitch);
    let mut no_cast = session.clone();
    area_damage::start_actor_damage_trace(&[actor]);
    area_damage::start_effect_damage_trace(actor.source_slot);
    let mut callbacks = Vec::new();
    let mut launches = Vec::new();
    let mut impacts = Vec::new();
    let mut saw_mana = false;
    let mut saw_hide = false;
    let mut saw_burst_dto = false;
    let mut saw_removal = false;
    let mut turns = 0;
    for turn in 0..160 {
        session.start_creature17_trace();
        mc2::start_callback_trace();
        session.step_outer_turn(PlayerCommand { flags: if turn == 0 { 0x10 } else { 0 },
            ..Default::default() }, 0);
        let trace = session.take_creature17_trace();
        launches.extend(trace.meteor_launches); impacts.extend(trace.meteor_impacts);
        let visits = mc2::take_callback_trace();
        saw_hide |= visits.iter().any(|visit| visit.actor == actor
            && visit.after.as_ref().is_some_and(|row| row.drawing_disabled()));
        callbacks.extend(visits);
        if session.source_arena().handle(actor.source_slot) == Some(actor)
            && session.entities()[usize::from(actor.source_slot)].life < 0 {
            saw_removal |= !session.source_arena().rebuilt_class5_model_slots()
                [usize::from(SOURCE_MODEL)].contains(&actor.source_slot);
        }
        saw_mana |= session.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::Economy { event: crate::native_world::Level2EconomyEvent::ManaSphereSpawned {
                source_entity_index, mana, .. }, .. } if *source_entity_index == actor.source_slot && *mana > 0));
        for (&slot, &(generation, parent)) in session.source_arena().effects.death_burst_origins() {
            if parent == actor.source_slot && session.source_arena().slot_generation(slot) == generation {
                let burst = &session.entities()[usize::from(slot)];
                saw_burst_dto |= burst.active && (burst.class, burst.model) == (10, 1)
                    && session.campaign_entity_presentations(0).unwrap().iter()
                        .any(|view| view.entity_index == slot && view.source_id == burst.source_id);
            }
        }
        assert!(session.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
        turns += 1;
        if session.source_arena().resolve(actor).is_none() && saw_hide && saw_mana && saw_burst_dto { break; }
    }
    let receipts = area_damage::take_effect_damage_trace();
    for _ in 0..turns { no_cast.step_outer_turn(PlayerCommand::default(), 0); }
    let fatal_match = callbacks.iter().any(|callback| {
        let Some(after) = &callback.after else { return false; };
        let Some(writer) = callback.entry_writer else { return false; };
        let damage = i32::from_le_bytes(callback.before.record[98..102].try_into().unwrap());
        let attacker = u16::from_le_bytes([callback.before.record[102], callback.before.record[103]]);
        callback.actor == actor && writer.target == actor && callback.before.life >= 0 && damage > 0
            && u16::from_le_bytes([callback.before.record[56], callback.before.record[57]]) == 0
            && attacker == player && writer.mailbox == (damage, player)
            && after.life == callback.before.life.wrapping_sub(damage) && after.life < 0
            && u16::from_le_bytes([after.record[40], after.record[41]]) == player
            // sub_26830 and the StageVar damage helpers clear only the
            // pending attacker; the consumed payload remains in the packet.
            && i32::from_le_bytes(after.record[98..102].try_into().unwrap()) == damage
            && u16::from_le_bytes([after.record[102], after.record[103]]) == 0
            && receipts.iter().any(|receipt| receipt.target_slot == actor.source_slot
                && receipt.target_generation == actor.generation && receipt.source_scoped
                && receipt.pulse.owner == player && receipt.pulse.payload == 1600
                && receipt.writer_sequence == Some(writer.sequence)
                && impacts.iter().any(|impact| receipt.source_slot == Some(impact.impact.source_slot)
                    && writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
                    && (impact.state.source_class, impact.state.model, impact.state.subspell_index) == (10, 17, 16_000)
                    && launches.iter().any(|launch| launch.holder == holder_handle && launch.caster == caster_handle
                        && launch.projectile == impact.projectile && (launch.packet.class, launch.packet.model,
                            launch.packet.subspell_index) == (9, 3, 16_000))))
    });
    assert!(fatal_match, "the actual public Meteor lineage must be the original model21's fatal writer");
    assert!(saw_removal && saw_mana && saw_burst_dto && saw_hide && session.source_arena().resolve(actor).is_none(),
        "the original generation must leave the living list, produce mana and burst DTO, hide and reclaim");
    assert!(!session.creature_identity_for_source_slot(actor.source_slot)
        .is_some_and(|identity| identity.handle == actor && identity.living));
    assert!(no_cast.creature_identity_for_source_slot(actor.source_slot)
        .is_some_and(|identity| identity.handle == actor && identity.living),
        "paired no-cast control retains the original generation");
}

#[test]
fn configured_mode12_publishes_current_sentinel_position_to_shared_scratch() {
    let terrain = terrain();
    let sentinel = Target { slot: 0, life: -1, position: Position { x: 1234, y: 4321, z: -7 },
        yaw: 71, roll: 93, ..Target::default() };
    let rows = [sentinel];
    let mut row = actor(175);
    row.record[77] = 12;
    put(&mut row, 44, 0);
    let mut routes = EmptyRoutes;
    let mut ctx = Context { terrain: &terrain, targets: &rows, class3: &[], model21: &[],
        families: &[], runtime: &mut routes };
    step(&mut Actor { row: &mut row }, &mut ctx).unwrap();
    let expected = Position { x: 1234, y: 4321, z: 0 };
    assert!(row.drawing_disabled());
    assert_eq!(row.position, expected);
    assert_eq!(terrain.source_predicted_position(), expected,
        "1E4D0 writes the clamped current sentinel pose even when hidden");
}

fn owner_level_two_with_constructor_counters() -> crate::native_world::Mc2Simulation {
    let package = std::env::var_os("MC_SETTLEMENT_FULL_PACKAGE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-1pb-16-level2-full-build-v2"));
    crate::campaign::CampaignSession::open(package, crate::campaign::spec::CampaignSelection::LEVEL2)
        .expect("whole Level2 owner package with source constructor counters")
        .simulation().clone()
}

#[test]
fn configured_scheduled_strike_reads_actual_replacement_actor_and_target() {
    // Explicit allocator-pressure and callback controls, not a claim about
    // ordinary mission pressure. Exercise action170's real adapter and NewEvent.
    let simulation = owner_level_two_with_constructor_counters();
    let particles = &simulation.session.definition.presentation.particles;
    let player = simulation.state.entity_index;
    for replace_actor in [true, false] {
        let mut arena = simulation.source_arena().clone();
        let mut runtime = simulation.runtime().clone();
        let mut terrain = simulation.canonical_terrain().clone();
        let target_before = arena.handle(player).expect("actual player target");
        let mut position = arena.entities[usize::from(player)].position;
        position.x = position.x.wrapping_add(512);
        let actor = mc2::allocate_authored(&mut arena, &mut runtime, &mut terrain,
            particles, position, RULES.max_life, None, 601).unwrap().unwrap();
        let actor_before = arena.handle(actor).unwrap();
        {
            let row = &mut arena.entities[usize::from(actor)];
            row.set_source_action(170);
            row.set_phase(0);
            put(row, 154, player);
        }
        for row in &mut arena.entities {
            row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
        }
        let replaced = if replace_actor { actor } else { player };
        let row = &mut arena.entities[usize::from(replaced)];
        row.set_control_flags(row.control_flags | crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
        arena.economy.exhaust_new_event_capacity_for_test();
        arena.rebuild_source_free_arena();
        let mut families = std::array::from_fn(|_| Vec::new());
        families[usize::from(SOURCE_MODEL)] = vec![actor];
        let targets_context = crate::native_world::source_target::Context {
            particles: Some(particles), map_type: Some(terrain.map_type()), ..Default::default()
        };
        mc2::start_callback_trace();
        mc2::step_row(&mut arena, &mut runtime, &mut terrain, particles, actor,
            &[player], &families, targets_context).expect("actual action170 callback");
        let visits = mc2::take_callback_trace();
        let visit = visits.iter().find(|visit| visit.actor == actor_before)
            .expect("original callback generation observed before allocation");
        let projectile = visit.projectile.expect("actual NewEvent produces 9/0");
        assert_eq!(projectile.source_slot, replaced);
        assert_ne!(projectile.generation, if replace_actor { actor_before.generation } else { target_before.generation });
        assert_eq!(arena.handle(replaced), Some(projectile));
        let birth = visit.projectile_packet.expect("packet recorded at actual birth");
        assert_eq!((birth.source_class, birth.source_model, birth.source_subspell_index), (9, 0, 500));
        let current = arena.economy.fireball_projectiles().iter()
            .find(|row| row.active && row.source_entity_index == replaced)
            .expect("actual current typed projectile");
        if replace_actor {
            assert!(arena.resolve(actor_before).is_none());
            assert_eq!((birth.owner_entity_index, birth.target_source_entity_index), (actor, 0),
                "source ID and target word96 are read from the newborn source address");
            assert_eq!(visit.after, None, "old actor cannot publish over the replacement");
            assert_eq!(current.visual_index, 308, "the original 170 tail runs on the current flyer");
            let particle = &particles[308];
            assert_eq!(current.collision_axes, Some([(particle.rotation_speed / 2) as i16,
                (particle.speed / 2) as i16, (particle.speed / 2) as i16,
                (particle.rotation_speed / 2) as i16]));
            assert_eq!(current.control_flags & 0x800, 0x800);
        } else {
            assert!(arena.resolve(target_before).is_none());
            assert_eq!((birth.target_class, birth.target_model), (9, 0),
                "guidance reads the newborn current target, not the previous player");
            assert_eq!(birth.target_source_entity_index, player);
            assert_eq!(arena.handle(actor), Some(actor_before));
            assert!(visit.after.is_some(), "the original actor executes its normal tail");
        }
    }
}

#[test]
fn configured_scheduled_water_splash_reuses_actor_and_finishes_current_packet() {
    // A source-replaceable actor is deliberately selected under exhausted
    // capacity; this is a configured source exception, not ordinary pressure.
    let simulation = owner_level_two_with_constructor_counters();
    let particles = &simulation.session.definition.presentation.particles;
    let mut arena = simulation.source_arena().clone();
    let mut runtime = simulation.runtime().clone();
    let mut terrain = simulation.canonical_terrain().clone();
    terrain.map_type = crate::native_world::MapType::Day;
    let mut position = simulation.state.position;
    position.z = terrain.ground_height(position);
    let tile = crate::native_world::terrain_index(position.x.to_be_bytes()[0], position.y.to_be_bytes()[0]);
    terrain.planes.terrain_types[tile] = 0;
    let actor = mc2::allocate_authored(&mut arena, &mut runtime, &mut terrain,
        particles, position, RULES.max_life, None, 602).unwrap().unwrap();
    let original = arena.handle(actor).unwrap();
    {
        let row = &mut arena.entities[usize::from(actor)];
        row.set_source_action(169);
        row.set_phase(1);
        row.record[74] = 8;
        row.set_control_flags(row.control_flags | 0x800);
    }
    let splash_origin = Position { x: position.x.wrapping_add(512), y: position.y.wrapping_add(768), z: 123 };
    terrain.set_source_predicted_position(splash_origin);
    for row in &mut arena.entities {
        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    }
    let row = &mut arena.entities[usize::from(actor)];
    row.set_control_flags(row.control_flags | crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    arena.economy.exhaust_new_event_capacity_for_test();
    arena.rebuild_source_free_arena();
    let mut families = std::array::from_fn(|_| Vec::new());
    families[usize::from(SOURCE_MODEL)] = vec![actor];
    mc2::start_callback_trace();
    mc2::step_row(&mut arena, &mut runtime, &mut terrain, particles, actor,
        &[], &families, Default::default()).expect("actual action169 and source water branch");
    let visits = mc2::take_callback_trace();
    assert!(visits.iter().any(|visit| visit.actor == original && visit.after.is_none()));
    let newborn = arena.handle(actor).expect("actual 10/5 occupies the original address");
    assert_ne!(newborn.generation, original.generation);
    assert!(arena.resolve(original).is_none());
    let splash = arena.economy.authored_model5_water_splashes().iter()
        .find(|row| row.active && row.source_entity_index == actor).copied()
        .expect("current typed splash after original callback");
    let source_phase = actor as u8;
    assert_eq!((splash.source_class, splash.model, splash.action, splash.phase), (10, 5, 5, source_phase));
    assert_eq!((splash.position.x, splash.position.y), (splash_origin.x, splash_origin.y));
    assert_eq!(splash.position.z, terrain.ground_height(splash_origin));
    assert_eq!(splash.actual_speed, RULES.roam_water_speed);
    assert_ne!(splash.control_flags & 0x800, 0);
    assert_eq!(splash.sprite_index, 311);
    let particle = &particles[311];
    assert_eq!(splash.collision_axes, Some([(particle.rotation_speed / 2) as i16,
        (particle.speed / 2) as i16, (particle.speed / 2) as i16,
        (particle.rotation_speed / 2) as i16]));
    assert_eq!(splash.animation_frame, 0);
    assert_eq!(splash.animation_limit, Some(particle.animation_limit().unwrap()));
    assert!(splash.animation_limit.unwrap() > 0, "particle311 must retain its source animation limit");
    assert!(arena.economy.map_chains.is_linked(actor));
    for visit_index in 0..9 {
        let mut effect_state = std::mem::take(&mut arena.effects);
        let visit = crate::native_world::effects::visit(&mut arena, &mut terrain, actor,
            crate::native_world::Level2DynamicArenaKind::AuthoredModel5WaterSplash,
            particles, &mut effect_state, |_, _| Ok(0)).expect("real action5 consumer");
        arena.effects = effect_state;
        let current = arena.economy.authored_model5_water_splashes().iter()
            .find(|row| row.source_entity_index == actor).unwrap();
        assert_eq!(current.phase, source_phase, "direct effect visits do not claim the scheduler phase tail");
        assert_eq!(current.life, 7 - visit_index);
        assert_eq!((current.sprite_index, current.animation_limit, current.actual_speed,
            current.collision_axes), (splash.sprite_index, splash.animation_limit,
            splash.actual_speed, splash.collision_axes),
            "the actual scheduled effect consumer retains the current callback's visual and shape");
        assert_eq!(current.animation_frame,
            (u8::try_from(visit_index).unwrap() + 1).min(splash.animation_limit.unwrap()),
            "particle311's actual animation advances through the scheduled header/store");
        assert!(current.active && current.drawing_enabled);
        assert_eq!(visit.sounds.contains(&27), visit_index == 0);
    }
    let mut effect_state = std::mem::take(&mut arena.effects);
    crate::native_world::effects::visit(&mut arena, &mut terrain, actor,
        crate::native_world::Level2DynamicArenaKind::AuthoredModel5WaterSplash,
        particles, &mut effect_state, |_, _| Ok(0)).expect("real action5 hide");
    arena.effects = effect_state;
    let hidden = arena.economy.authored_model5_water_splashes().iter()
        .find(|row| row.source_entity_index == actor).unwrap();
    assert_eq!((hidden.phase, hidden.life), (source_phase, -2));
    assert!(hidden.active && !hidden.drawing_enabled, "action5 retains the hidden carrier until boundary reclaim");
    assert_eq!(arena.handle(actor), Some(newborn));
    arena.rebuild_source_arena_with_terrain(&mut terrain, &[]).unwrap();
    assert!(!arena.economy.authored_model5_water_splashes().iter()
        .any(|row| row.source_entity_index == actor), "following boundary reclaims the newborn");
    assert!(arena.resolve(newborn).is_none());
}

#[test]
fn configured_level8_real_group26_absence_uses_pass_start_membership() {
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::campaign::source_arena::CreatureRootOrigin;
    use crate::level_definition::{CreatureKind, ObjectiveCondition};
    use crate::native_world::{area_damage, Mc2DispatchEvent};
    use crate::PlayerCommand;

    let package = std::env::var_os("MC_LEVEL8_PACKAGE").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq5-cave-reset-v2-level8"),
        std::path::PathBuf::from,
    );
    let mut session = CampaignSession::open(package, CampaignSelection::ordinary(8).unwrap())
        .expect("unfiltered Level8 package").simulation().clone();
    let objective = session.session.definition.semantic_level().unwrap().objectives().iter()
        .position(|objective| objective.id().as_str() == "objective.campaign.task_5")
        .expect("actual Level8 task5");
    assert_eq!(objective, 4);
    assert_eq!(session.session.definition.semantic_level().unwrap().objectives()[objective].condition(),
        ObjectiveCondition::NoLivingCreatures(CreatureKind::Creature21));
    let player = session.state.entity_index;
    let owner = usize::from(session.state.player_color_index);
    // Declared progression and player-isolation controls. Source switch13 and
    // group26 still produce the unfiltered authored actors and normal callbacks.
    let gate = session.entities()[13].clone();
    assert_eq!((gate.class, gate.model, gate.action, gate.source_id), (11, 32, 32, 26));
    let gate_stage = usize::try_from(gate.level_gate).unwrap();
    for index in 0..session.runtime.objectives.len() {
        session.runtime.set_level2_objective_stage_selector_for(index, gate_stage, 2);
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().any(|event| matches!(event,
        Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 13, group: 26, authored_indices, .. }
            if authored_indices.contains(&113) && authored_indices.contains(&140))),
        "the actual objective-gated group26 must construct the authored roots");
    {
        let live = &mut session.session.state.live;
        let parent = &mut live.economy.entities[usize::from(player)];
        parent.set_max_life(200_000);
        parent.set_life(200_000);
        let parent = parent.clone();
        live.player_life = parent.life;
        live.shared_spells.sync_player_entity_from_authority(&parent);
        let runtime = &mut live.core.runtime;
        for index in 0..runtime.objectives.len() {
            for selector in 0..runtime.objectives[index].stage_selectors.len() {
                runtime.set_level2_objective_stage_selector_for(index, selector, 0);
            }
            runtime.set_level2_objective_level_end_for(index, 0);
        }
        runtime.set_level2_objective_text_selector_for(owner, objective);
        runtime.set_level2_objective_stage_selector_for(owner, objective, 1);
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
    assert_eq!(session.runtime().objectives[owner].stage_selectors[objective], 1,
        "living real model21 roots prevent the source objective from completing");
    let roots = session.source_arena().rebuilt_class5_model_slots()[usize::from(SOURCE_MODEL)]
        .iter().map(|&slot| session.source_arena().handle(slot).expect("live original generation"))
        .collect::<Vec<_>>();
    assert!(roots.len() > 1, "real group26 must leave multiple living roots");
    for authored in [113, 140] {
        assert!(roots.iter().any(|handle| session.creature_identity_for_source_slot(handle.source_slot)
            .is_some_and(|identity| identity.handle == *handle
                && matches!(identity.origin, CreatureRootOrigin::AuthoredRecord { authored_record } if authored_record == authored))),
            "the frozen model head must include original group26 record {authored}");
    }
    let survivor = *roots.iter().max_by_key(|handle|
        session.entities()[usize::from(handle.source_slot)].life).unwrap();
    area_damage::start_actor_damage_trace(&roots);
    {
        let live = &mut session.session.state.live;
        for handle in roots.iter().filter(|&&handle| handle != survivor) {
            assert_eq!(live.economy.handle(handle.source_slot), Some(*handle));
            let life = live.economy.entities[usize::from(handle.source_slot)].life;
            let damage = u16::try_from(life + 1).expect("real root life fits the source contact payload");
            area_damage::contact(&mut live.economy, None, handle.source_slot, damage, player).unwrap();
        }
    }
    // One actual source pass consumes the configured contacts. The designated
    // surviving original is never assigned a new life/action or moved.
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
    let surviving = session.source_arena().rebuilt_class5_model_slots()[usize::from(SOURCE_MODEL)].clone();
    assert_eq!(surviving, vec![survivor.source_slot], "exactly one original root remains alive");
    assert_eq!(session.source_arena().handle(survivor.source_slot), Some(survivor));
    assert_eq!(session.runtime().objectives[owner].stage_selectors[objective], 1,
        "one actual surviving root prevents absence completion");
    {
        let live = &mut session.session.state.live;
        let life = live.economy.entities[usize::from(survivor.source_slot)].life;
        assert!(life >= 0);
        area_damage::contact(&mut live.economy, None, survivor.source_slot,
            u16::try_from(life + 1).unwrap(), player).unwrap();
    }
    mc2::start_callback_trace();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let callbacks = mc2::take_callback_trace();
    assert!(session.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
    assert!(callbacks.iter().any(|visit| {
        let Some(after) = &visit.after else { return false; };
        let Some(writer) = visit.entry_writer else { return false; };
        let damage = i32::from_le_bytes(visit.before.record[98..102].try_into().unwrap());
        visit.actor == survivor && writer.target == survivor && writer.mailbox == (damage, player)
            && u16::from_le_bytes([visit.before.record[102], visit.before.record[103]]) == player
            && visit.before.life >= 0 && after.life == visit.before.life.wrapping_sub(damage)
            && after.life < 0 && u16::from_le_bytes([after.record[102], after.record[103]]) == 0
    }), "the last original generation must consume the actual player contact and die");
    assert!(session.source_arena().rebuilt_class5_model_slots()[usize::from(SOURCE_MODEL)].is_empty(),
        "the current living-list rebuild is empty after the lethal callback");
    assert_eq!(session.runtime().objectives[owner].stage_selectors[objective], 1,
        "the same pass still uses the nonempty list frozen before the final hit");
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
    assert_eq!(session.runtime().objectives[owner].stage_selectors[objective], 2,
        "the next source pass freezes an empty model head and completes real task5");
    for _ in 0..24 {
        if roots.iter().all(|handle| session.source_arena().resolve(*handle).is_none()) { break; }
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
    }
    assert!(roots.iter().all(|handle| session.source_arena().resolve(*handle).is_none()),
        "the original generations reach normal death, hide and following reclaim");
}
