use super::*;

struct EmptyRoutes;

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
            runtime: &mut routes,
        };

        step(&mut Actor { row: &mut row }, &mut ctx).unwrap();

        assert_eq!(row.action, 169);
        assert_eq!(row.record[71], 64);
        assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 0);
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
        position,
        target_position,
        yaw,
        pitch,
        damage: RULES.attack_damage,
    };
    let before = arena.economy.fireball_projectiles().len();
    let (arena, terrain) = (&mut live.economy, &mut live.terrain);
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
        runtime: &mut routes,
    };
    assert!(step(&mut Actor { row: &mut row }, &mut ctx).is_err());
}
