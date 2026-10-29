use super::*;

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
    assert_eq!(
        effects.strike.map(|strike| strike.target_slot),
        Some(enemy.slot)
    );
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
    assert_eq!(effects.strike, None);
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
    assert_eq!(
        effects
            .strike
            .map(|strike| (strike.target_slot, strike.damage)),
        Some((12, RULES.attack_damage))
    );

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
    assert_eq!((lost.action, lost_target, effects.strike), (169, 0, None));
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
    super::mc2::step_row(arena, runtime, terrain, &particles, slot, &[], &families)
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
        })
        .map(|row| (row.index, row.source_id))
        .expect("Level 2 fixture retains a distinct source owner");
    let mut position = target_position;
    position.x = position.x.wrapping_sub(512);
    let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(position, target_position);
    let strike = Strike {
        source_slot: owner.0,
        source_id: owner.1,
        target_slot,
        target_class: 3,
        target_model: 0,
        initial_position: Position {
            z: position.z.wrapping_sub(200),
            ..position
        },
        position,
        target_position,
        yaw,
        pitch,
        damage: RULES.attack_damage,
    };
    let before = arena.economy.fireball_projectiles().len();
    let (arena, terrain) = (&mut live.economy, &mut live.terrain);
    terrain.map_type = crate::native_world::MapType::Night;
    let mut unlit_arena = arena.clone();
    unlit_arena.dynamic_lighting_enabled = false;
    let mut unlit_terrain = terrain.clone();
    assert!(super::mc2::launch_strike(arena, terrain, strike).unwrap());
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
                && light.position == strike.initial_position)
    );
    assert!(super::mc2::launch_strike(&mut unlit_arena, &mut unlit_terrain, strike).unwrap());
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
    assert!(!super::mc2::launch_strike(arena, terrain, strike).unwrap());
    assert_eq!(arena.economy.fireball_projectiles().len(), before);
    assert_eq!(
        arena.entities[usize::from(target_slot)].record[58..60],
        feedback
    );
}

#[test]
fn water_splash_adapter_materializes_source_model5_and_reclaims_after_its_action5_lifecycle() {
    let mut simulation = crate::native_world::tests::level_two_campaign_session();
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
    assert_eq!(splash.position.z, terrain.ground_height(position));

    for _ in 0..9 {
        assert!(
            arena
                .economy
                .step_authored_model5_water_splash_slot(splash_slot)
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
            .step_authored_model5_water_splash_slot(splash_slot)
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
