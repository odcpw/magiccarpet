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

fn source_rows(count: u16) -> Vec<EntitySnapshot> {
    (0..count)
        .map(|slot| crate::native_world::cave_features::new_event(slot, slot, Position::default()))
        .collect()
}

fn actor(action: u8, position: Position) -> EntitySnapshot {
    let mut row = crate::native_world::cave_features::new_event(7, 19, position);
    row.class = 5;
    row.model = SOURCE_MODEL;
    row.record[67] = 5;
    row.record[68] = SOURCE_MODEL;
    row.set_source_action(action);
    row.set_life(RULES.max_life);
    row.set_max_life(RULES.max_life);
    row.set_mana(RULES.initial_mana);
    row.set_source_overlap_half_extents(384, 384);
    row.set_fov(0);
    row.record[92..94].copy_from_slice(&384_i16.to_le_bytes());
    row.set_phase(1);
    row
}

fn target(slot: u16, life: i32) -> Target {
    Target {
        slot,
        source_id: slot,
        class: 3,
        model: 0,
        life,
        position: Position {
            x: 20_128,
            y: 20_000,
            z: 8_192,
        },
        ..Target::default()
    }
}

fn mana_sphere(slot: u16, position: Position, mana: i32) -> crate::level1_mission::ManaSphere {
    crate::level1_mission::ManaSphere {
        id: u32::from(slot),
        source_entity_index: slot,
        entity_rng: 1,
        position,
        velocity_x: 0,
        velocity_y: 0,
        vertical_velocity: 0,
        yaw: 0,
        max_life: 300,
        life: 300,
        mana,
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
    }
}

fn empty_families() -> [Vec<u16>; 29] {
    std::array::from_fn(|_| Vec::new())
}

fn exhaust(arena: &mut crate::campaign::source_arena::SourceArena) {
    for row in &mut arena.entities {
        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    }
    arena.economy.exhaust_new_event_capacity_for_test();
    arena.rebuild_source_free_arena();
}

#[test]
fn constructor_binds_the_source_particle_descriptor_and_collision_fov() {
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
    assert_eq!(row.particle_parameter_index, Some(289));
    assert_eq!(row.fov, 0);
    assert_eq!(row.array_fov(), 384);
    assert_eq!(row.position.z, RULES.maximum_lift);
    assert_eq!(
        i16::from_le_bytes([row.record[48], row.record[49]]),
        RULES.maximum_lift
    );
    assert_eq!(&row.record[44..46], &[0, 0]);
}

#[test]
fn target_height_uses_source_word_0x2c_at_record_offset_48() {
    let mut row = actor(184, Position::default());
    row.record[44..46].copy_from_slice(&0x1234_u16.to_le_bytes());
    row.record[48..50].copy_from_slice(&0x5678_u16.to_le_bytes());
    {
        let mut actor = super::Actor { row: &mut row };
        assert_eq!(actor.target_height(), 0x5678_i16);
        actor.set_target_height(-48);
        assert_eq!(actor.target_height(), -48);
    }
    assert_eq!(&row.record[44..46], &0x1234_u16.to_le_bytes());
    assert_eq!(&row.record[48..50], &(-48_i16).to_le_bytes());
}

#[test]
fn live_victim_emits_lightning_and_dead_victim_does_not_consume_a_live_sphere() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 8_192,
    };
    let targets = [target(12, 10_000)];
    let class3 = [12];
    let families = empty_families();
    let mut entities = source_rows(16);
    let mut spheres = [];
    let mut runtime = EmptyRoutes;
    let mut row = actor(186, position);
    row.set_phase(0);
    row.record[154..156].copy_from_slice(&12_u16.to_le_bytes());
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        families: &families,
        entities: &mut entities,
        spheres: &mut spheres,
        runtime: &mut runtime,
    };
    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(
        effects
            .lightning
            .map(|lightning| (lightning.target_slot, lightning.damage)),
        Some((12, RULES.lightning_damage))
    );

    let targets = [target(12, -1)];
    let mut entities = source_rows(16);
    let mut spheres = [mana_sphere(
        12,
        Position {
            z: 8_400,
            ..position
        },
        350,
    )];
    let mut runtime = EmptyRoutes;
    let mut lost = actor(186, position);
    lost.record[154..156].copy_from_slice(&12_u16.to_le_bytes());
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        families: &families,
        entities: &mut entities,
        spheres: &mut spheres,
        runtime: &mut runtime,
    };
    let effects = step(&mut Actor { row: &mut lost }, &mut context).unwrap();
    let target_slot = u16::from_le_bytes([lost.record[154], lost.record[155]]);
    assert_eq!(
        (lost.action, target_slot, effects.lightning),
        (187, 0, None)
    );
    assert!(!context.spheres[0].pending_delete);
}

#[test]
fn source_adapter_commits_a_real_ray_runs_its_consumer_and_keeps_failed_birth_silent() {
    let mut simulation = crate::native_world::tests::level_two_campaign_session();
    let particles = simulation.session.definition.presentation.particles.clone();
    let live = &mut simulation.session.state.live;
    let (target_slot, target_position, target_source_id) = live
        .economy
        .entities
        .iter()
        .find(|row| row.active && row.life >= 0 && row.class == 3 && row.model == 0)
        .map(|row| (row.index, row.position, row.source_id))
        .expect("Level 2 fixture retains a live player target");
    let source_id = u16::MAX;
    let position = Position {
        z: target_position.z.wrapping_add(384),
        ..target_position
    };
    let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(position, target_position);
    let request = LightningRequest {
        source_slot: 1,
        source_id,
        target_slot,
        target_class: 3,
        target_model: 0,
        position,
        target_position,
        yaw,
        pitch,
        damage: RULES.lightning_damage,
    };
    let (arena, terrain) = (&mut live.economy, &mut live.terrain);
    let before_slots = arena
        .entities
        .iter()
        .filter(|row| row.active && row.class == 9 && row.model == 9)
        .map(|row| row.index)
        .collect::<std::collections::BTreeSet<_>>();
    assert!(super::mc2::launch_lightning(arena, terrain, &particles, request).unwrap());
    let ray = arena
        .entities
        .iter()
        .find(|row| {
            row.active
                && row.class == 9
                && row.model == 9
                && row.source_id == source_id
                && row.record[71] == 10
                && row.record[72] == 23
                && u16::from_le_bytes([row.record[154], row.record[155]]) == target_slot
                && !before_slots.contains(&row.index)
        })
        .expect("successful model23 NewEvent commits a source Ray row");
    let ray_slot = ray.index;
    assert_eq!((ray.record[65], ray.record[66]), (3, 0));
    assert_eq!(ray.subspell_index, RULES.lightning_damage as u16);
    assert_eq!(
        arena.entities[usize::from(target_slot)].record[58..60],
        100_i16.to_le_bytes()
    );

    let outcome =
        crate::native_world::lightning::step(arena, terrain, &particles, ray_slot, None).unwrap();
    let endpoint = outcome
        .impact_slot
        .expect("the shared Lightning ray consumer commits its endpoint");
    let endpoint = &arena.entities[usize::from(endpoint)];
    assert_eq!(
        (endpoint.class, endpoint.model, endpoint.action),
        (10, 23, 23)
    );
    assert_eq!(endpoint.subspell_index, RULES.lightning_damage as u16);

    let feedback = arena.entities[usize::from(target_slot)].record[58..60].to_vec();
    exhaust(arena);
    assert!(!super::mc2::launch_lightning(arena, terrain, &particles, request).unwrap());
    assert_eq!(
        arena.entities[usize::from(target_slot)].record[58..60],
        feedback
    );
    assert_ne!(target_source_id, source_id);
}

#[test]
fn mission18_contact_kill_reclaims_model23_after_the_living_root_census() {
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
    .expect("Mission18 retains an ordinary model23 root")
    .simulation()
    .clone();
    session.step_level2_source_pass(0);
    let source_slot = session
        .session
        .state
        .live
        .economy
        .entities
        .iter()
        .find(|row| row.active && row.class == 5 && row.model == 23 && row.index == 29)
        .expect("ordinary source production retains Mission18 root29")
        .index;
    let killer_slot = session
        .session
        .state
        .live
        .economy
        .entities
        .iter()
        .find(|row| row.active && row.life >= 0 && row.class == 3 && row.model == 0)
        .expect("Mission18 retains a live player")
        .index;
    let identity = session
        .creature_identity_for_source_slot(source_slot)
        .expect("the actual source root census resolves authored model23");
    assert_eq!(
        identity.kind,
        crate::level_definition::CreatureKind::Creature23
    );
    assert!(identity.living);
    let root_handle = identity.handle;
    let root_origin = identity.origin;
    let initial_life = session.session.state.live.economy.entities[usize::from(source_slot)].life;
    let kills_before = session
        .session
        .state
        .live
        .economy
        .economy
        .authored_bee_kill_census(killer_slot);
    assert!(initial_life >= 0);
    let lethal_damage =
        u16::try_from(initial_life + 1).expect("Mission18 model23 life fits source damage");
    {
        let live = &mut session.session.state.live;
        crate::native_world::area_damage::contact(
            &mut live.economy,
            None,
            source_slot,
            lethal_damage,
            killer_slot,
        )
        .expect("the ordinary source contact producer accepts the live root");
        let actor = &live.economy.entities[usize::from(source_slot)];
        assert_eq!(
            actor.life, initial_life,
            "contact only queues source damage"
        );
        assert_ne!(actor.action, 188, "contact does not preset prekill");
    }

    // The first real source pass consumes contact's raw damage mailbox. The
    // next pass handles action188 and publishes kill credit as action189.
    let phase_before_hit = session.entities()[usize::from(source_slot)].phase;
    session.step_level2_source_pass(1);
    {
        let live = &session.session.state.live;
        let actor = &live.economy.entities[usize::from(source_slot)];
        assert_eq!(actor.action, 188);
        assert_eq!(actor.phase, phase_before_hit.wrapping_add(1));
        assert_eq!(actor.life, -1);
        assert!(actor.active);
        assert_eq!(actor.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
        let identity = session
            .creature_identity_for_source_slot(source_slot)
            .expect("lethal but retained root remains in the source census");
        assert_eq!(identity.handle, root_handle);
        assert_eq!(identity.origin, root_origin);
        assert!(!identity.living);
    }

    let phase_before_prekill = session.entities()[usize::from(source_slot)].phase;
    session.step_level2_source_pass(2);
    let reward_mana = {
        let live = &session.session.state.live;
        let actor = &live.economy.entities[usize::from(source_slot)];
        assert_eq!(actor.action, 189);
        assert_eq!(actor.phase, phase_before_prekill.wrapping_add(1));
        assert_eq!(actor.life, -1);
        assert!(actor.active);
        assert_eq!(actor.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
        let kills_after = live.economy.economy.authored_bee_kill_census(killer_slot);
        assert!(
            kills_after > kills_before,
            "prekill credit reaches the player consumer"
        );
        actor.mana
    };
    let sphere_slots_before_death = session
        .session
        .state
        .live
        .economy
        .economy
        .spheres()
        .iter()
        .filter(|sphere| sphere.active)
        .map(|sphere| sphere.source_entity_index)
        .collect::<Vec<_>>();

    let mut death_pass = None;
    for pass in 3_u8..=18 {
        let phase_before_callback = session.entities()[usize::from(source_slot)].phase;
        session.step_level2_source_pass(pass);
        let actor = &session.session.state.live.economy.entities[usize::from(source_slot)];
        assert_eq!(actor.phase, phase_before_callback.wrapping_add(1));
        if actor.drawing_disabled() {
            death_pass = Some(pass);
            break;
        }
        assert_eq!(actor.action, 189, "action189 waits for its source cadence");
    }
    let death_pass = death_pass.expect("action189 cadence produces and hides model23");

    {
        let live = &session.session.state.live;
        let actor = &live.economy.entities[usize::from(source_slot)];
        assert!(
            actor.active,
            "hide retains the source row through this pass"
        );
        let identity = session
            .creature_identity_for_source_slot(source_slot)
            .expect("hidden root remains present until the following pass");
        assert_eq!(identity.handle, root_handle);
        assert_eq!(identity.origin, root_origin);
        assert!(!identity.living, "hidden root is no longer living");
        assert!(
            live.economy.resolve(root_handle).is_some(),
            "same-pass source tail retains the hidden generation"
        );
        let census = live.economy.creature_runtime_census(
            session.session.definition.initial_creature_bindings(),
            session.session.definition.authored_creature_bindings(),
        );
        assert!(census.entries().iter().any(|entry| {
            entry.handle == root_handle && entry.origin == root_origin && !entry.living
        }));
        let reward = live
            .economy
            .economy
            .spheres()
            .iter()
            .find(|sphere| {
                sphere.active
                    && sphere.mana == reward_mana
                    && !sphere_slots_before_death.contains(&sphere.source_entity_index)
            })
            .expect("model23 death produces a new mana sphere");
        let reward_row = &live.economy.entities[usize::from(reward.source_entity_index)];
        assert_eq!((reward_row.class, reward_row.model), (10, 39));
    }

    session.step_level2_source_pass(death_pass.wrapping_add(1));
    let live = &session.session.state.live;
    assert!(
        live.economy.resolve(root_handle).is_none(),
        "following source pass reclaims the old generation"
    );
    let census = live.economy.creature_runtime_census(
        session.session.definition.initial_creature_bindings(),
        session.session.definition.authored_creature_bindings(),
    );
    assert!(
        !census
            .entries()
            .iter()
            .any(|entry| entry.handle == root_handle),
        "actual root census no longer contains the reclaimed Mission18 identity"
    );
    assert!(
        session
            .creature_identity_for_source_slot(source_slot)
            .is_none_or(|identity| identity.handle != root_handle)
    );
}

#[test]
fn claimed_mana_sphere_is_consumed_and_its_source_row_is_hidden_for_reclaim() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 8_192,
    };
    let targets = [];
    let class3 = [];
    let families = empty_families();
    let mut entities = source_rows(16);
    entities[12].index = 12;
    entities[12].active = true;
    entities[12].class = 10;
    entities[12].model = 39;
    let mut spheres = [mana_sphere(
        12,
        Position {
            z: 8_400,
            ..position
        },
        350,
    )];
    let mut runtime = EmptyRoutes;
    let mut row = actor(187, position);
    row.record[70] = 1;
    row.record[16..20].copy_from_slice(&2_i32.to_le_bytes());
    row.record[154..156].copy_from_slice(&12_u16.to_le_bytes());
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        families: &families,
        entities: &mut entities,
        spheres: &mut spheres,
        runtime: &mut runtime,
    };
    let _ = step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(row.mana, RULES.initial_mana + 350);
    assert!(context.spheres[0].pending_delete);
    assert_ne!(
        context.spheres[0].control_flags & ENTITY_DRAW_DISABLED_BIT,
        0
    );
    assert_ne!(
        context.entities[12].control_flags & ENTITY_DRAW_DISABLED_BIT,
        0
    );
}

#[test]
fn claimed_mana_sphere_mirrors_the_source_word96_holder_relation() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 8_192,
    };
    let targets = [];
    let class3 = [];
    let families = empty_families();
    let mut entities = source_rows(16);
    entities[12].index = 12;
    entities[12].active = true;
    entities[12].class = 10;
    entities[12].model = 39;
    let mut spheres = [mana_sphere(
        12,
        Position {
            x: position.x.wrapping_add(2_048),
            z: position.z,
            ..position
        },
        350,
    )];
    let mut runtime = EmptyRoutes;
    let mut row = actor(187, position);
    let predator_slot = row.index;
    row.record[70] = 1;
    row.record[16..20].copy_from_slice(&2_i32.to_le_bytes());
    row.record[154..156].copy_from_slice(&12_u16.to_le_bytes());
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        families: &families,
        entities: &mut entities,
        spheres: &mut spheres,
        runtime: &mut runtime,
    };

    let _ = step(&mut Actor { row: &mut row }, &mut context).unwrap();

    let sphere = context.spheres[0];
    assert_eq!(row.mana, RULES.initial_mana);
    assert_eq!(
        sphere.control_flags & MANA_SPHERE_CLAIMED,
        MANA_SPHERE_CLAIMED
    );
    assert_eq!(sphere.assigned_carrier_id, 0);
    assert_eq!(sphere.assigned_defender_slot, predator_slot);
    assert_eq!(
        u16::from_le_bytes([
            context.entities[12].record[154],
            context.entities[12].record[155]
        ]),
        predator_slot
    );
    assert_eq!(
        context.entities[12].control_flags & MANA_SPHERE_CLAIMED,
        MANA_SPHERE_CLAIMED
    );
    assert!(!sphere.pending_delete);
}

#[test]
fn damage_by_a_class3_attacker_detaches_the_predator_from_its_claimed_sphere() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 8_192,
    };
    let targets = [target(9, 10_000)];
    let class3 = [9];
    let families = empty_families();
    let mut entities = source_rows(16);
    entities[12].index = 12;
    entities[12].active = true;
    entities[12].class = 10;
    entities[12].model = 39;
    entities[12].set_control_flags(MANA_SPHERE_CLAIMED);
    entities[12].record[154..156].copy_from_slice(&7_u16.to_le_bytes());
    let mut claimed = mana_sphere(12, position, 350);
    claimed.control_flags = MANA_SPHERE_CLAIMED;
    claimed.assigned_carrier_id = 7;
    claimed.assigned_defender_slot = 7;
    let mut spheres = [claimed];
    let mut runtime = EmptyRoutes;
    let mut row = actor(186, position);
    row.record[154..156].copy_from_slice(&12_u16.to_le_bytes());
    row.record[98..102].copy_from_slice(&10_i32.to_le_bytes());
    row.record[102..104].copy_from_slice(&9_u16.to_le_bytes());
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        families: &families,
        entities: &mut entities,
        spheres: &mut spheres,
        runtime: &mut runtime,
    };

    let _ = step(&mut Actor { row: &mut row }, &mut context).unwrap();

    let sphere = context.spheres[0];
    assert_eq!(row.life, RULES.max_life - 10);
    assert_eq!(
        u16::from_le_bytes([row.record[154], row.record[155]]),
        targets[0].slot
    );
    assert_eq!(sphere.control_flags & MANA_SPHERE_CLAIMED, 0);
    assert_eq!(sphere.assigned_carrier_id, 0);
    assert_eq!(sphere.assigned_defender_slot, 0);
    assert_eq!(context.entities[12].control_flags & MANA_SPHERE_CLAIMED, 0);
    assert_eq!(
        u16::from_le_bytes([
            context.entities[12].record[154],
            context.entities[12].record[155]
        ]),
        0
    );
}

#[test]
fn source_disabled_action190_is_rejected() {
    assert!(enabled(187));
    assert!(!enabled(190));
    let terrain = terrain();
    let targets = [];
    let class3 = [];
    let families = empty_families();
    let mut entities = source_rows(8);
    let mut spheres = [];
    let mut runtime = EmptyRoutes;
    let mut row = actor(190, Position::default());
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        families: &families,
        entities: &mut entities,
        spheres: &mut spheres,
        runtime: &mut runtime,
    };
    assert!(step(&mut Actor { row: &mut row }, &mut context).is_err());
}
