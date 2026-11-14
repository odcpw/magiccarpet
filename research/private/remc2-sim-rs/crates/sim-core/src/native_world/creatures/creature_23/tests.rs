use super::*;

struct EmptyRoutes;

// Configured leaf checks do not publish a Ray; exercise their tail explicitly.
fn configured_step(actor: &mut Actor<'_>, ctx: &mut Context<'_>) -> Result<Effects, String> {
    let mut effects = step(actor, ctx)?;
    if effects.combat_tail_pending {
        let tail = consume_combat_tail(actor, ctx.entities.first().is_some_and(|row| row.class == 3));
        finish_combat_tail(actor, ctx, tail);
        finish_attack(actor, &mut effects);
        effects.combat_tail_pending = false;
    }
    Ok(effects)
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
        action: 41,
        entity_rng: 1,
        position,
        velocity_x: 0,
        velocity_y: 0,
        vertical_velocity: 0,
        yaw: 0,
        roll: 0,
        actual_speed: 32,
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
fn frozen_mana_order_selects_hidden_equal_distance_before_target_validation() {
    let terrain = terrain();
    let position = Position { x: 20_000, y: 20_000, z: 8_192 };
    let mut hidden = mana_sphere(12, position, 350);
    hidden.control_flags |= ENTITY_DRAW_DISABLED_BIT;
    let mut spheres = [hidden, mana_sphere(13, position, 350)];
    let candidates = [
        Ok(Some(ManaCandidate { slot: 12, position })),
        Ok(Some(ManaCandidate { slot: 13, position })),
    ];
    let mut entities = source_rows(16);
    let families = empty_families();
    let mut runtime = EmptyRoutes;
    let mut row = actor(184, position);
    row.set_source_byte_46(1);
    let mut context = Context {
        terrain: &terrain, targets: &[], target_error: None, mana_candidates: &candidates,
        class3: &[], families: &families, peers: &[], entities: &mut entities,
        spheres: &mut spheres,
        population: None,
        runtime: &mut runtime,
    };
    configured_step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 12,
        "sub_28000 chooses the first equal-distance model39 even when hidden");
    assert_eq!(row.record[74], 2);
    configured_step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(row.record[74], 1,
        "the later sub_28420 visit rejects the hidden target");
    assert!(!context.spheres[1].pending_delete);
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
        target_error: None, mana_candidates: &[],
        class3: &class3,
        families: &families, peers: &[],
        entities: &mut entities,
        spheres: &mut spheres,
        population: None,
        runtime: &mut runtime,
    };
    let effects = configured_step(&mut Actor { row: &mut row }, &mut context).unwrap();
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
        target_error: None, mana_candidates: &[],
        class3: &class3,
        families: &families, peers: &[],
        entities: &mut entities,
        spheres: &mut spheres,
        population: None,
        runtime: &mut runtime,
    };
    let effects = configured_step(&mut Actor { row: &mut lost }, &mut context).unwrap();
    let target_slot = u16::from_le_bytes([lost.record[154], lost.record[155]]);
    assert_eq!(
        (lost.action, target_slot, effects.lightning),
        (187, 0, None)
    );
    assert!(!context.spheres[0].pending_delete);
}

#[test]
fn source_adapter_commits_a_real_ray_runs_its_consumer_and_keeps_failed_birth_silent() {
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use std::path::PathBuf;
    let package = std::env::var_os("MC_CREATURE23_LEVEL18_PACKAGE").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
                "../../../generated/all-spells-20261104-integration/current-owner-package/level18",
            )
        },
        PathBuf::from,
    );
    let mut campaign = CampaignSession::open(package, CampaignSelection::ordinary(18).unwrap())
        .expect("qualified Level18 retains the source constructor counters");
    let simulation = campaign.simulation_mut();
    let particles = simulation.session.definition.presentation.particles.clone();
    let live = &mut simulation.session.state.live;
    let (target_slot, target_position, target_source_id) = live
        .economy
        .entities
        .iter()
        .find(|row| row.active && row.life >= 0 && row.class == 3 && row.model == 0)
        .map(|row| (row.index, row.position, row.source_id))
        .expect("Level18 retains a live player target");
    let position = Position {
        z: target_position.z.wrapping_add(384),
        ..target_position
    };
    let source_slot = super::mc2::allocate_authored(
        &mut live.economy,
        &mut live.core.runtime,
        &mut live.terrain,
        &particles,
        position,
        RULES.max_life,
        None,
        29,
    )
    .unwrap()
    .expect("configured real model23 constructor");
    live.economy.entities[usize::from(source_slot)].record[154..156]
        .copy_from_slice(&target_slot.to_le_bytes());
    let source_id = live.economy.entities[usize::from(source_slot)].source_id;
    let request = LightningRequest {
        source_slot,
        target_slot,
        position,
        damage: RULES.lightning_damage,
    };
    let (arena, terrain) = (&mut live.economy, &mut live.terrain);
    let mut feedback = crate::native_world::source_target::Creature23PlayerFeedback {
        shared: &mut live.shared_spells,
        core_players: &mut live.core.players,
        fire: live.core.context.spell_fire.as_mut(),
        local_state: &mut live.core.state,
        domains: None,
    };
    let before_slots = arena
        .entities
        .iter()
        .filter(|row| row.active && row.class == 9 && row.model == 9)
        .map(|row| row.index)
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        super::mc2::launch_lightning(
            arena,
            terrain,
            &particles,
            Default::default(),
            &mut feedback,
            request
        )
        .unwrap()
    );
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
    assert_eq!(feedback.local_state.music_countdown, 100);

    let outcome = crate::native_world::lightning::step(
        arena,
        terrain,
        &particles,
        ray_slot,
        None,
        crate::native_world::source_target::Context::default(),
        &mut |_, _| {},
    )
    .unwrap();
    let endpoint = outcome
        .impact_slot
        .expect("the shared Lightning ray consumer commits its endpoint");
    let endpoint = &arena.entities[usize::from(endpoint)];
    assert_eq!(
        (endpoint.class, endpoint.model, endpoint.action),
        (10, 23, 23)
    );
    assert_eq!(endpoint.subspell_index, RULES.lightning_damage as u16);

    let before_feedback = feedback.local_state.music_countdown;
    exhaust(arena);
    assert!(
        !super::mc2::launch_lightning(
            arena,
            terrain,
            &particles,
            Default::default(),
            &mut feedback,
            request
        )
        .unwrap()
    );
    assert_eq!(feedback.local_state.music_countdown, before_feedback);
    assert_ne!(target_source_id, source_id);
}

#[test]
fn mission18_contact_kill_reclaims_model23_after_the_living_root_census() {
    let package = std::env::var_os("MC_CREATURE23_LEVEL18_PACKAGE").map_or_else(
        || std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
            || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/all-spells-20261104-integration/current-owner-package/level18"),
            |root| std::path::PathBuf::from(root).join("level18"),
        ),
        std::path::PathBuf::from,
    );
    let mut session = crate::campaign::CampaignSession::open(
        package,
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
    assert_eq!(
        killer_slot,
        session
            .session
            .state
            .live
            .shared_spells
            .local_player
            .entity_index,
        "the contact owner is the player read by completion statistics"
    );
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
        let population = live.economy.source_creature_population as i32;
        let expected_percent = if population == 0 {
            100
        } else {
            (kills_after as i32)
                .wrapping_mul(100)
                .wrapping_div(population)
                .clamp(0, 100)
        };
        assert_eq!(
            session.completion_statistics().creatures_killed_percent,
            expected_percent,
            "the campaign statistics consumer observes the credited kill"
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

    let reward_slot = {
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
        let reward_row = crate::native_world::source_target::read(
            &live.economy,
            crate::native_world::source_target::Context::default(),
            reward.source_entity_index,
        ).unwrap().expect("newborn sphere has a current source occupant");
        assert_eq!((reward_row.class, reward_row.model), (10, 39));
        assert_eq!(reward_row.life, reward.life);
        assert_eq!(reward_row.position, reward.position);
        reward.source_entity_index
    };

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

    // Isolate the consumer by constructing a second model23 and staging its
    // already-entered capture state beside the *actual* death sphere. The
    // source scheduler, rather than a copied sphere or direct leaf call,
    // performs the pickup and publishes mana to its live actor row.
    let particles = session.session.definition.presentation.particles.clone();
    let collector_slot = {
        let live = &mut session.session.state.live;
        let reward = live
            .economy
            .economy
            .spheres()
            .iter()
            .find(|sphere| sphere.active && sphere.source_entity_index == reward_slot)
            .copied()
            .expect("the produced death sphere survives the reclaim pass");
        assert_eq!(reward.mana, reward_mana);
        let collector = super::mc2::allocate_authored(
            &mut live.economy,
            &mut live.core.runtime,
            &mut live.terrain,
            &particles,
            reward.position,
            RULES.max_life,
            None,
            29,
        )
        .unwrap()
        .expect("the source constructor allocates a collection actor");
        let row = &mut live.economy.entities[usize::from(collector)];
        row.set_source_action(187);
        row.set_source_byte_46(1);
        row.set_source_dword_10(2);
        row.record[154..156].copy_from_slice(&reward_slot.to_le_bytes());
        row.position = Position {
            z: reward.position.z.wrapping_sub(32),
            ..reward.position
        };
        let position = row.position;
        crate::world::effects::sync_position(
            &mut live.economy,
            &mut live.terrain,
            collector,
            position,
            true,
            false,
        )
        .unwrap();
        collector
    };
    session.step_level2_source_pass(death_pass.wrapping_add(2));
    let live = &session.session.state.live;
    assert_eq!(
        live.economy.entities[usize::from(collector_slot)].mana,
        RULES.initial_mana.wrapping_add(reward_mana),
        "the scheduled capture consumer absorbs the actual death reward"
    );
    assert!(live.economy.economy.spheres().iter().all(|sphere| {
        sphere.source_entity_index != reward_slot || sphere.pending_delete || !sphere.active
    }));
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
    row.set_source_byte_46(1);
    row.set_source_dword_10(2);
    row.record[154..156].copy_from_slice(&12_u16.to_le_bytes());
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        target_error: None, mana_candidates: &[],
        class3: &class3,
        families: &families, peers: &[],
        entities: &mut entities,
        spheres: &mut spheres,
        population: None,
        runtime: &mut runtime,
    };
    let _ = configured_step(&mut Actor { row: &mut row }, &mut context).unwrap();
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
    row.set_source_byte_46(1);
    row.set_source_dword_10(2);
    row.record[154..156].copy_from_slice(&12_u16.to_le_bytes());
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        target_error: None, mana_candidates: &[],
        class3: &class3,
        families: &families, peers: &[],
        entities: &mut entities,
        spheres: &mut spheres,
        population: None,
        runtime: &mut runtime,
    };

    let _ = configured_step(&mut Actor { row: &mut row }, &mut context).unwrap();

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
fn damage_retains_claim_until_source_nearest_victim_scan_detaches_it() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 8_192,
    };
    let mut victim = target(9, 10_000);
    victim.castle = Some(Castle {
        position: Position { x: 30_000, y: 30_000, z: position.z },
        half_x: 384,
        half_y: 384,
    });
    let targets = [victim];
    let class3 = [9];
    let families = empty_families();
    let mut entities = source_rows(16);
    entities[12].index = 12;
    entities[12].active = true;
    entities[12].class = 10;
    entities[12].model = 39;
    entities[12].set_control_flags(MANA_SPHERE_CLAIMED);
    entities[12].record[154..156].copy_from_slice(&7_u16.to_le_bytes());
    let mut claimed = mana_sphere(12, Position { z: 7_000, ..position }, 350);
    claimed.control_flags = MANA_SPHERE_CLAIMED;
    claimed.assigned_carrier_id = 7;
    claimed.assigned_defender_slot = 7;
    let mut spheres = [claimed];
    let mut runtime = EmptyRoutes;
    let mut row = actor(187, position);
    row.set_source_byte_46(1);
    row.set_source_dword_10(4);
    row.record[154..156].copy_from_slice(&12_u16.to_le_bytes());
    row.record[98..102].copy_from_slice(&10_i32.to_le_bytes());
    row.record[102..104].copy_from_slice(&9_u16.to_le_bytes());
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        target_error: None, mana_candidates: &[],
        class3: &class3,
        families: &families, peers: &[],
        entities: &mut entities,
        spheres: &mut spheres,
        population: None,
        runtime: &mut runtime,
    };

    let _ = configured_step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(row.life, RULES.max_life - 10);
    assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 12);
    assert_ne!(context.spheres[0].control_flags & MANA_SPHERE_CLAIMED, 0,
        "pinned sub_28110's slot-zero attacker lookup does not force retaliation");

    // The independent normal 1-in-32 nearest-victim scan can still select
    // a visible player outside Castle protection and release this claim.
    row.set_phase(0);
    let _ = configured_step(&mut Actor { row: &mut row }, &mut context).unwrap();
    let sphere = context.spheres[0];
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
fn level18_actual_hunter_selects_player_and_delivers_lightning_damage() {
    use super::mc2::{start_ray_birth_trace, take_ray_birth_trace};
    use crate::PlayerCommand;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::native_world::lightning::{start_endpoint_birth_trace, take_endpoint_birth_trace};
    use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent, area_damage};
    use std::{collections::BTreeMap, path::PathBuf};

    let package = std::env::var_os("MC_CREATURE23_LEVEL18_PACKAGE").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
                "../../../generated/all-spells-20261104-integration/current-owner-package/level18",
            )
        },
        PathBuf::from,
    );
    let mut campaign = CampaignSession::open(package, CampaignSelection::ordinary(18).unwrap())
        .expect("qualified whole Level18");
    let session = campaign.simulation_mut();
    let player = session.state.entity_index;
    assert_eq!(player, 91);
    let player_handle = session.source_arena().handle(player).unwrap();
    let root = session
        .source_arena()
        .handle(29)
        .expect("real Level18 reset root29");
    let initial = session.entities()[29].clone();
    assert_eq!(
        (
            initial.class,
            initial.model,
            initial.action,
            initial.position.z
        ),
        (5, 23, 184, 8192)
    );
    let source_id = initial.source_id;
    assert!(
        session
            .campaign_entity_presentations(0)
            .unwrap()
            .iter()
            .any(|view| view.entity_index == root.source_slot)
    );
    // Declared player-only survival and placement; the real Castle is an
    // upstream prerequisite for source282D0, not an earned public Castle cast.
    {
        let live = &mut session.session.state.live;
        live.player_life = 200_000;
        live.shared_spells.set_player_entity_life(200_000);
        let raw = &mut live.economy.entities[usize::from(player)];
        raw.set_life(200_000);
        let raw = raw.clone();
        live.shared_spells.sync_player_entity_from_authority(&raw);
        live.shared_spells
            .sync_typed_spell_parent_caches_from_player_entity();
    }
    let player_position = Position {
        x: initial.position.x.wrapping_add(1024),
        ..initial.position
    };
    session
        .set_level2_entity_position(player, player_position)
        .unwrap();
    let castle_position = Position {
        x: initial.position.x.wrapping_add(8192),
        z: 0,
        ..initial.position
    };
    let castle = session
        .economy_mut()
        .build_castle(castle_position, player)
        .expect("declared real Castle prerequisite");
    let castle_handle = session
        .source_arena()
        .handle(castle)
        .expect("real Castle generation");
    let mut rays = Vec::new();
    let mut endpoints = Vec::new();
    let mut writers = BTreeMap::new();
    let mut selected = false;
    let mut sound = false;
    let mut consumed = false;
    let mut impact_diagnostics = Vec::new();
    area_damage::start_player_damage_trace(player_handle);
    let journal_start = session.effect_journal().records().len();
    for turn in 0..192 {
        start_ray_birth_trace();
        // Endpoint tracing is opt-in; it observes the actual 9/9 -> 10/23 birth.
        start_endpoint_birth_trace();
        area_damage::start_effect_damage_trace(player);
        session.step_outer_turn(PlayerCommand::default(), 0);
        let births = take_ray_birth_trace();
        let endpoint_births = take_endpoint_birth_trace();
        let impacts = area_damage::take_effect_damage_trace();
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "turn {turn}: {:?}",
            session.dispatch_events()
        );
        assert_eq!(session.source_arena().handle(castle), Some(castle_handle));
        assert_eq!(session.source_arena().handle(player), Some(player_handle));
        let actor = &session.entities()[usize::from(root.source_slot)];
        assert_eq!(session.source_arena().handle(root.source_slot), Some(root));
        selected |= actor.action == 186 && actor.source_word_96() == player;
        for birth in births {
            if birth.source_before == Some(root)
                && birth.source_after == Some(root)
                && birth.target_before == Some(player_handle)
                && birth.target_after == Some(player_handle)
                && birth.owner == source_id
            {
                rays.push(birth.ray);
            }
        }
        for birth in endpoint_births {
            if rays.contains(&birth.ray) && birth.owner == source_id {
                endpoints.push(birth.endpoint);
            }
        }
        for impact in impacts {
            if impact_diagnostics.len() < 8 {
                let source = impact.source_raw_impact.as_ref().map(|(generation, row)| {
                    (
                        row.index,
                        *generation,
                        row.position,
                        row.source_id,
                        row.subspell_index,
                    )
                });
                let player_row = &session.entities()[usize::from(player)];
                impact_diagnostics.push(format!(
                    "turn={turn} source={source:?} scoped={} pulse={:?} target={:?} in_cells={} before={:?} after={:?} hits={:?} writer={:?} cursor={} player_pose={:?} player_flags={:#x}",
                    impact.source_scoped, impact.pulse, impact.target, impact.target_in_scanned_cells,
                    impact.mailbox_before, impact.mailbox_after, impact.hit_count, impact.writer_sequence,
                    impact.event_cursor, player_row.position, player_row.control_flags,
                ));
            }
            let Some((generation, row)) = impact.source_raw_impact.as_ref() else {
                continue;
            };
            let handle = crate::campaign::source_arena::SourceEntityHandle {
                source_slot: row.index,
                generation: *generation,
            };
            if endpoints.contains(&handle)
                && impact.source_scoped
                && impact.target_generation == player_handle.generation
                && impact.target_slot == player
                && impact.pulse.payload == RULES.lightning_damage as u16
                && let (Some(sequence), Some(mailbox)) =
                    (impact.writer_sequence, impact.mailbox_after)
            {
                writers.insert(sequence, (handle, mailbox, impact.event_cursor));
            }
        }
        for hit in area_damage::take_player_damage_consumptions() {
            let Some(writer) = hit.writer else { continue };
            let Some((impact, mailbox, cursor)) = writers.remove(&writer.sequence) else {
                continue;
            };
            if hit.target == player_handle
                && writer.target == player_handle
                && writer.source == area_damage::PlayerMailboxSource::Effect(impact)
                && writer.mailbox == mailbox
                && hit.mailbox == mailbox
                && hit.event_cursor >= cursor
                && hit.life_after == hit.life_before.wrapping_sub(mailbox.0)
                && hit.life_after < hit.life_before
                && matches!(session.economy().events().get(hit.event_cursor), Some(Level2EconomyEvent::DamageApplied { target_entity_index, attacker_entity_index, damage, lethal: false })
                    if *target_entity_index == player && *attacker_entity_index == mailbox.1 && *damage == mailbox.0)
            {
                consumed = true;
            }
        }
        sound |= session.effect_journal().records()[journal_start..].iter().any(|record|
            matches!(&record.effect, crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                audio_site: crate::MissionAudioSite::Creature23Attack, entity_index, sound_id: 59, player_index: -1,
            }) if *entity_index == root.source_slot)
            && record.audio.is_some_and(|audio| audio.play_type == 3 && audio.command.audible && audio.command.rate_delta_percent == 0));
        if selected && sound && consumed {
            break;
        }
    }
    assert!(
        selected && sound && consumed,
        "actual root29 source-selected player and 9/9 -> 10/23 damage: selected={selected} sound={sound} consumed={consumed} rays={rays:?} endpoints={endpoints:?} writers={writers:?} impacts={impact_diagnostics:?}"
    );
}

#[test]
fn level18_actual_hunter_claims_lifts_and_consumes_public_steal_mana_sphere() {
    level18_steal_mana_encounter(false);
}

#[test]
fn level18_remote_morph_binding_admits_actual_current_body_callback() {
    level18_steal_mana_encounter(true);
}

fn level18_steal_mana_encounter(remote_morph_regression: bool) {
    use super::mc2::{start_sphere_consumption_trace, take_sphere_consumption_trace};
    use crate::PlayerCommand;
    use crate::campaign::schedule::ScheduleReceipt;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::combat::actors::PlayerEffectLane;
    use crate::native_world::Mc2DispatchEvent;
    use crate::native_world::creatures::morph::{
        start_binding_admission_trace, take_binding_admission_trace,
    };
    use crate::native_world::directional_spells::{
        start_steal_mana_sphere_birth_trace, take_steal_mana_sphere_birth_trace,
    };
    use crate::native_world::level2_dynamic_economy::{
        start_claimed_sphere_lift_trace, take_claimed_sphere_lift_trace,
    };
    use crate::native_world::creatures::rival_wizards::{
        start_pending_steal_prelude_trace, take_pending_steal_prelude_trace,
    };
    use std::{collections::BTreeMap, path::PathBuf};

    let package = std::env::var_os("MC_CREATURE23_LEVEL18_PACKAGE").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
                "../../../generated/all-spells-20261104-integration/current-owner-package/level18",
            )
        },
        PathBuf::from,
    );
    let mut campaign = CampaignSession::open(package, CampaignSelection::ordinary(18).unwrap())
        .expect("qualified whole Level18");
    let session = campaign.simulation_mut();
    let player = session.state.entity_index;
    let root = session.source_arena().handle(29).expect("authored hunter");
    let rival = session.source_arena().handle(184).expect("authored rival");
    let rival_castle = session
        .source_arena()
        .handle(209)
        .expect("authored rival Castle");
    let initial = session.entities()[usize::from(root.source_slot)].clone();
    assert_eq!(
        (
            initial.class,
            initial.model,
            initial.action,
            initial.position.z
        ),
        (5, 23, 184, 8192)
    );
    assert_eq!(
        (session.entities()[184].class, session.entities()[184].model),
        (3, 1)
    );
    let player_handle = session.source_arena().handle(player).unwrap();

    // Declared prior-campaign spell carry, XP, player resources and Castle;
    // the rival, its funded Castle, hunter and all sphere callbacks remain
    // the unchanged, ordinarily scheduled Level18 owners.
    session
        .apply_carried_spell_families(&[13], Some(13))
        .unwrap();
    session
        .shared_spells_mut()
        .award_spell_experience(player, 13, 1_200);
    let castle_position = Position {
        x: initial.position.x.wrapping_add(2_048),
        z: 0,
        ..initial.position
    };
    let castle = session
        .economy_mut()
        .build_castle(castle_position, player)
        .expect("real player Castle prerequisite");
    let castle_handle = session.source_arena().handle(castle).unwrap();
    {
        let live = &mut session.session.state.live;
        assert!(
            live.economy
                .economy
                .set_castle_mana_prerequisite_for_test(castle, 60_000)
        );
        live.player_life = 200_000;
        live.shared_spells.set_player_entity_life(200_000);
        let row = &mut live.economy.entities[usize::from(player)];
        row.set_life(200_000);
        row.max_mana = 100_000;
        row.record[144..148].copy_from_slice(&100_000_i32.to_le_bytes());
        row.set_mana(100_000);
        row.set_mana_regen(0);
        let row = row.clone();
        live.shared_spells.sync_player_entity_from_authority(&row);
        live.shared_spells.player_mana = row.mana;
        live.shared_spells.caster.mana = row.mana;
        live.shared_spells
            .sync_typed_spell_parent_caches_from_player_entity();
        if let Some(fire) = live.core.context.spell_fire.as_mut() {
            fire.caster.mana = row.mana;
        }
    }
    let quiet = Position {
        x: 40_000,
        y: 40_000,
        z: 8_192,
    };
    session.set_level2_entity_position(player, quiet).unwrap();
    // The capture case waits for the real rival to leave its current Castle
    // by two projectile steps, then for its own protection counter to clear.
    // The margin gives the single cast and next visit room; the actual
    // protection decision is still verified at the source callback.
    // Keep the independent Morph regression's original wait unchanged.
    let mut rival_ready = false;
    let mut wait_diagnostics = None;
    for _ in 0..256 {
        let live = &session.session.state.live;
        let row = &live.economy.entities[usize::from(rival.source_slot)];
        assert_eq!(live.economy.handle(rival.source_slot), Some(rival));
        let protection_clear = live
            .nyphur_dispatcher
            .get(rival)
            .unwrap()
            .common_upkeep(row.phase)
            .word_0x159
            == 0;
        let outside_castle = if remote_morph_regression {
            true
        } else {
            let castle = live
                .economy
                .economy
                .castle_for_owner(rival.source_slot)
                .expect("current rival Castle owner link");
            assert_eq!(castle.source_entity_index, rival_castle.source_slot);
            let dx =
                i32::from((castle.position.x as i16).wrapping_sub(row.position.x as i16)).abs();
            let dy =
                i32::from((castle.position.y as i16).wrapping_sub(row.position.y as i16)).abs();
            let margin = 2 * i32::from(
                crate::native_world::directional_spells::STEAL_MANA_SOURCE_CONSTRUCTOR.speed,
            );
            wait_diagnostics = Some((
                row.position,
                row.control_flags,
                row.life,
                castle.position,
                castle.shift_pitch,
                castle.shift_roll,
                dx,
                dy,
                margin,
                protection_clear,
            ));
            dx >= i32::from(row.array_pitch()) + i32::from(castle.shift_pitch) + margin
                || dy >= i32::from(row.array_roll()) + i32::from(castle.shift_roll) + margin
        };
        if protection_clear
            && (remote_morph_regression
                || (outside_castle
                    && row.active
                    && row.life >= 0
                    && row.control_flags & 8 != 0
                    && !row.drawing_disabled()))
        {
            rival_ready = true;
            break;
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. }))
        );
    }
    assert!(
        rival_ready,
        "the naturally scheduled rival did not become vulnerable outside its current Castle: {wait_diagnostics:?}"
    );
    {
        let live = &session.session.state.live;
        let row = &live.economy.entities[usize::from(rival.source_slot)];
        assert_eq!(
            live.nyphur_dispatcher
                .get(rival)
                .unwrap()
                .common_upkeep(row.phase)
                .word_0x159,
            0
        );
    }
    let ui = crate::SOURCE_SPELL_INDEX_MAP
        .iter()
        .position(|family| *family == 13)
        .unwrap() as i8;
    session.step_outer_turn(
        PlayerCommand {
            opcode: 31,
            argument_1: ui,
            argument_2: 2,
            ..Default::default()
        },
        0,
    );
    let holder = session.shared_spells().local_player().enabled_spell_holders[13];
    assert_ne!(holder, 0);
    assert_eq!(
        session.shared_spells().pickup(holder).unwrap().level_gate,
        2
    );
    let victim = session.entities()[usize::from(rival.source_slot)].clone();
    let target = {
        let live = &mut session.session.state.live;
        crate::native_world::area_damage::source_actor(&mut live.economy, None, rival.source_slot)
            .unwrap()
    };
    let player_fov = session.entities()[usize::from(player)].array_fov();
    // The capture control compensates the source left-hand offset and starts
    // one projectile step away. The independent Morph regression retains
    // candidate4's original player pose, which exposed the real guard gap.
    let origin = Position {
        x: victim
            .position
            .x
            .wrapping_add(if remote_morph_regression { 0 } else { 256 }),
        y: victim
            .position
            .y
            .wrapping_add(if remote_morph_regression { 1_536 } else { 384 }),
        z: victim
            .position
            .z
            .wrapping_add(target.shape.vertical_offset)
            .wrapping_sub(player_fov),
    };
    session.set_level2_entity_position(player, origin).unwrap();
    session.set_level2_player_aim_for_test(0, 0);
    let castle_before = session
        .economy()
        .castle_at(rival_castle.source_slot)
        .expect("funded rival Castle")
        .mana;
    assert!(castle_before > 0);
    let mut emitted = Vec::new();
    let mut saw_flyer = false;
    let mut saw_field = false;
    let mut castle_debited = false;
    let mut submitted = false;
    let mut consumed_lane = false;
    let mut admitted_prelude = false;
    let mut prelude_diagnostics = Vec::new();
    let mut previous_lane = None;
    let mut previous_castle_mana = castle_before;
    let mut observations = Vec::new();
    let mut cast_diagnostics = Vec::new();
    let mut cast_diagnostic_counts = [0_usize; 2];
    let mut hunter_diagnostics = Vec::new();
    let mut hunter_last = String::new();
    let mut lift_diagnostics = Vec::new();
    let mut last_lift = None;
    let journal_start = session.effect_journal().records().len();
    let mut pursuit_sound = false;
    let mut remote_morph_witness = None;
    let mut morph_diagnostics = Vec::new();
    for turn in 0..800 {
        let hunter_before = session.entities()[usize::from(root.source_slot)].clone();
        start_sphere_consumption_trace();
        start_claimed_sphere_lift_trace();
        start_steal_mana_sphere_birth_trace();
        start_binding_admission_trace();
        start_pending_steal_prelude_trace();
        session.step_outer_turn(
            if turn == 0 {
                PlayerCommand {
                    flags: 0x10,
                    ..Default::default()
                }
            } else {
                PlayerCommand::default()
            },
            0,
        );
        let consumptions = take_sphere_consumption_trace();
        let lifts = take_claimed_sphere_lift_trace();
        let births = take_steal_mana_sphere_birth_trace();
        let binding_admissions = take_binding_admission_trace();
        let preludes = take_pending_steal_prelude_trace();
        let admitted_this_turn = preludes.iter().any(|receipt| {
            receipt.actor == rival
                && receipt.pending.attacker == player
                && receipt.pending.payload == 2
                && receipt.a4_castle_slot == rival_castle.source_slot
                && receipt.effective_castle_slot == rival_castle.source_slot
                && receipt
                    .castle
                    .is_some_and(|castle| castle.slot == rival_castle.source_slot)
                && !receipt.castle_overlaps
                && receipt.word159_before == 0
                && receipt.admitted
                && !receipt.cleared
        });
        admitted_prelude |= admitted_this_turn;
        for receipt in preludes
            .iter()
            .filter(|receipt| receipt.actor == rival && receipt.pending.attacker == player)
        {
            if prelude_diagnostics.len() < 8 {
                prelude_diagnostics.push(*receipt);
            }
        }
        if cast_diagnostic_counts.iter().any(|count| *count < 8) {
            let target = {
                let live = &mut session.session.state.live;
                crate::native_world::area_damage::source_actor(
                    &mut live.economy,
                    None,
                    rival.source_slot,
                )
            };
            let rival_row = &session.entities()[usize::from(rival.source_slot)];
            let protection = session
                .session
                .state
                .live
                .nyphur_dispatcher
                .get(rival)
                .map(|owner| owner.common_upkeep(rival_row.phase).word_0x159);
            for product in session.entities().iter().filter(|row| {
                row.active
                    && row.source_id == player
                    && matches!((row.class, row.model), (9, 8) | (10, 25))
            }) {
                let kind = usize::from(product.class == 10);
                if cast_diagnostic_counts[kind] < 8 {
                    cast_diagnostic_counts[kind] += 1;
                    cast_diagnostics.push(format!(
                        "turn={turn} product={:?} selector={:?} position={:?} yaw={} pitch={} life={} flags={:#x} target={} axes={:?} rival={target:?} rival_raw={:?} protection={protection:?} lane={:?} origin={origin:?}",
                        session.source_arena().handle(product.index), (product.class, product.model, product.action),
                        product.position, product.yaw, product.pitch, product.life, product.control_flags,
                        product.source_word_96(), (product.array_yaw(), product.array_pitch(), product.array_roll(), product.array_fov()),
                        (rival_row.active, rival_row.class, rival_row.model, rival_row.action, rival_row.position, rival_row.life, rival_row.control_flags),
                        rival_row.pending_player_effect(PlayerEffectLane::StealMana),
                    ));
                }
            }
        }
        if let Some(Mc2DispatchEvent::Error {
            source_slot,
            message,
            ..
        }) = session
            .dispatch_events()
            .iter()
            .find(|event| matches!(event, Mc2DispatchEvent::Error { .. }))
        {
            let row = &session.entities()[usize::from(*source_slot)];
            let parent_slot = row.source_spell_parent_entity_index();
            let parent = session.entities().get(usize::from(parent_slot));
            let holders = session
                .shared_spells()
                .holders()
                .iter()
                .chain(session.shared_spells().pickups())
                .chain(session.shared_spells().dynamic_pickups())
                .filter(|holder| holder.model == 4)
                .take(8)
                .map(|holder| {
                    (
                        session.source_arena().handle(holder.index),
                        holder.active,
                        holder.class,
                        holder.model,
                        holder.action,
                        holder.source_spell_parent_entity_index(),
                        holder.source_word_96(),
                        holder.control_flags,
                        holder.drawing_disabled(),
                    )
                })
                .collect::<Vec<_>>();
            let census = session
                .source_arena()
                .creature_runtime_census(&BTreeMap::new(), &BTreeMap::new());
            let origin = census
                .entries()
                .iter()
                .find(|entry| entry.handle.source_slot == *source_slot);
            let morph_bindings = session
                .economy()
                .morph
                .bodies
                .iter()
                .take(8)
                .collect::<Vec<_>>();
            panic!(
                "turn {turn}: slot={source_slot} message={message} handle={:?} packet={:?} parent={:?} holders={holders:?} morph_bindings={morph_bindings:?} origin={origin:?} births={births:?} lifts={lifts:?} consumptions={consumptions:?} emitted={emitted:?} observed={observations:?} flyer={saw_flyer} field={saw_field} lane={submitted}/{consumed_lane} prelude={prelude_diagnostics:?} debit={castle_debited} cast={cast_diagnostics:?}",
                session.source_arena().handle(*source_slot),
                (
                    row.active,
                    row.class,
                    row.model,
                    row.action,
                    row.phase,
                    row.record[77],
                    row.source_id,
                    row.source_word_96(),
                    row.life,
                    row.control_flags,
                    row.position
                ),
                parent.map(|parent| (
                    parent.index,
                    parent.active,
                    parent.class,
                    parent.model,
                    parent.action,
                    parent.source_id,
                    parent.control_flags
                ))
            );
        }
        assert_eq!(session.source_arena().handle(player), Some(player_handle));
        assert_eq!(session.source_arena().handle(root.source_slot), Some(root));
        assert_eq!(
            session.source_arena().handle(rival.source_slot),
            Some(rival)
        );
        assert_eq!(session.source_arena().handle(castle), Some(castle_handle));
        assert_eq!(
            session.source_arena().handle(rival_castle.source_slot),
            Some(rival_castle)
        );
        let schedule = session.cf10_production_schedule_receipts();
        for admission in binding_admissions {
            if remote_morph_regression && morph_diagnostics.len() < 8 {
                let visits = schedule.iter().filter(|receipt| matches!(receipt,
                    ScheduleReceipt::CallbackCompleted { slot, .. } | ScheduleReceipt::EpilogueCompleted { slot, .. }
                        if *slot == admission.body.source_slot)).cloned().collect::<Vec<_>>();
                morph_diagnostics.push(format!("turn={turn} admission={admission:?} current_holder={:?} current_body={:?} final_phase={} visits={visits:?}",
                    session.source_arena().handle(admission.holder.source_slot),
                    session.source_arena().handle(admission.body.source_slot),
                    session.entities()[usize::from(admission.body.source_slot)].phase));
            }
            if admission.parent == player
                || admission.body_drawing_disabled
                || admission.raw_holder_selector != (15, 4, 12)
                || admission.raw_holder_parent != admission.parent
                || admission.raw_word96 != admission.body.source_slot
                || admission.shared_word96 == admission.body.source_slot
                || session.source_arena().handle(admission.holder.source_slot)
                    != Some(admission.holder)
                || session.source_arena().handle(admission.body.source_slot) != Some(admission.body)
            {
                continue;
            }
            for (index, receipt) in schedule.iter().enumerate() {
                let ScheduleReceipt::CallbackCompleted {
                    pass,
                    scan,
                    slot,
                    selector,
                } = receipt
                else {
                    continue;
                };
                if *slot != admission.body.source_slot
                    || selector.observed_phase() != Some(admission.phase)
                {
                    continue;
                }
                let epilogue = schedule[index + 1..].iter().any(|receipt| matches!(receipt,
                    ScheduleReceipt::EpilogueCompleted { pass: ep_pass, scan: ep_scan, slot: ep_slot, original, .. }
                        if ep_pass == pass && ep_scan == scan && ep_slot == slot
                            && original.observed_phase() == Some(admission.phase)));
                let advances = schedule[index + 1..]
                    .iter()
                    .filter(|receipt| {
                        matches!(receipt,
                    ScheduleReceipt::EpilogueCompleted { slot: ep_slot, .. } if ep_slot == slot)
                    })
                    .count();
                let after = session.entities()[usize::from(admission.body.source_slot)].phase;
                if epilogue && advances > 0 && after == admission.phase.wrapping_add(advances as u8)
                {
                    remote_morph_witness = Some((
                        admission.holder,
                        admission.body,
                        admission.parent,
                        admission.phase,
                        after,
                        turn,
                    ));
                    break;
                }
            }
        }
        saw_flyer |= session
            .entities()
            .iter()
            .any(|row| row.active && row.source_id == player && (row.class, row.model) == (9, 8));
        saw_field |= session
            .entities()
            .iter()
            .any(|row| row.active && row.source_id == player && (row.class, row.model) == (10, 25));
        let lane = session.entities()[usize::from(rival.source_slot)]
            .pending_player_effect(PlayerEffectLane::StealMana);
        submitted |= lane.is_some_and(|hit| hit.attacker == player && hit.payload == 2);
        let current_castle_mana = session
            .economy()
            .castle_at(rival_castle.source_slot)
            .expect("current rival Castle")
            .mana;
        let exact_debit = previous_castle_mana.wrapping_sub(current_castle_mana)
            == previous_castle_mana.wrapping_mul(10) / 100;
        if previous_lane.is_some_and(|hit: crate::combat::actors::PlayerEffectHit| {
            hit.attacker == player && hit.payload == 2
        }) && lane.is_none()
            && previous_castle_mana > 0
            && exact_debit
        {
            castle_debited = true;
            consumed_lane = true;
        }
        previous_lane = lane;
        previous_castle_mana = current_castle_mana;
        for birth in births {
            if admitted_this_turn
                && birth.caster == player_handle
                && birth.target == rival
                && birth.tier == 2
                && birth.mana > 0
                && !emitted.iter().any(|(saved, _)| *saved == birth.sphere)
            {
                emitted.push((birth.sphere, birth.mana));
            }
        }
        for &(handle, mana) in &emitted {
            if !observations
                .iter()
                .any(|(saved, _, _, _, _, _, _)| *saved == handle)
            {
                observations.push((handle, mana, false, false, false, false, None));
            }
        }
        let actor = &session.entities()[usize::from(root.source_slot)];
        let target_sphere = session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == actor.source_word_96())
            .map(|sphere| {
                (
                    session.source_arena().handle(sphere.source_entity_index),
                    sphere.position,
                    sphere.life,
                    sphere.mana,
                    sphere.control_flags,
                    sphere.assigned_defender_slot,
                    sphere.pending_delete,
                )
            });
        hunter_last = format!(
            "turn={turn} actor={:?} target_sphere={target_sphere:?}",
            (
                actor.action,
                actor.record[74],
                actor.phase,
                actor.source_dword_10(),
                actor.source_word_2c(),
                actor.source_word_96(),
                actor.position,
                actor.life,
                actor.mana
            )
        );
        if hunter_diagnostics.len() < 40
            && (turn % 128 == 0
                || (
                    hunter_before.action,
                    hunter_before.record[74],
                    hunter_before.source_word_96(),
                ) != (actor.action, actor.record[74], actor.source_word_96()))
        {
            hunter_diagnostics.push(hunter_last.clone());
        }
        for lift in lifts.iter().filter(|receipt| {
            receipt.holder == root && emitted.iter().any(|(handle, _)| *handle == receipt.sphere)
        }) {
            let sample = format!(
                "turn={turn} lift={lift:?} holder_action={} mode={} timer={} target={} phase={}",
                actor.action,
                actor.record[74],
                actor.source_dword_10(),
                actor.source_word_96(),
                actor.phase
            );
            if lift_diagnostics.len() < 8 {
                lift_diagnostics.push(sample.clone());
            }
            last_lift = Some(sample);
        }
        for (handle, mana, claimed, lifted, absorbed, reclaimed, prior_position) in
            &mut observations
        {
            let sphere = session.economy().spheres().iter().find(|sphere| {
                sphere.source_entity_index == handle.source_slot
                    && session.source_arena().handle(handle.source_slot) == Some(*handle)
            });
            if let Some(sphere) = sphere {
                let actual_claim = sphere.assigned_defender_slot == root.source_slot
                    && sphere.control_flags & MANA_SPHERE_CLAIMED != 0;
                if !*claimed
                    && actual_claim
                    && actor.source_word_96() == handle.source_slot
                    && hunter_diagnostics.len() < 40
                {
                    hunter_diagnostics.push(format!("first_claim turn={turn} sphere={handle:?} mana={mana} sphere_position={:?} {hunter_last}", sphere.position));
                }
                *claimed |= actual_claim && actor.source_word_96() == handle.source_slot;
                *prior_position = Some(sphere.position);
            }
            *lifted |= lifts.iter().any(|receipt| {
                receipt.holder == root
                    && receipt.sphere == *handle
                    && receipt.lift_step > 0
                    && receipt.proposal == receipt.after
                    && xy_distance(receipt.before, receipt.holder_position) < 16
                    && receipt.before.z < receipt.holder_position.z
                    && receipt.after.z == receipt.before.z.wrapping_add(receipt.lift_step)
            });
            *absorbed |= *claimed
                && *lifted
                && sphere.is_some_and(|sphere| {
                    sphere.pending_delete
                        && sphere.control_flags & ENTITY_DRAW_DISABLED_BIT != 0
                        && session.entities()[usize::from(handle.source_slot)].drawing_disabled()
                        && consumptions.iter().any(|receipt| {
                            receipt.actor == root
                                && receipt.sphere == *handle
                                && receipt.sphere_mana == sphere.mana
                                && receipt.sphere_mana >= *mana
                                && receipt.mana_after
                                    == receipt.mana_before.wrapping_add(receipt.sphere_mana)
                        })
                });
            if *absorbed && session.source_arena().handle(handle.source_slot) != Some(*handle) {
                *reclaimed = true;
            }
        }
        pursuit_sound |= session.effect_journal().records()[journal_start..].iter().any(|record|
            matches!(&record.effect, crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                audio_site: crate::MissionAudioSite::Creature23ManaPursuit, entity_index, sound_id: 59, player_index: -1,
            }) if *entity_index == root.source_slot)
            && record.audio.is_some_and(|audio| audio.play_type == 3 && audio.command.rate_delta_percent == 0));
        if turn == 0 {
            session.set_level2_entity_position(player, quiet).unwrap();
        }
        let captured =
            observations
                .iter()
                .any(|(_, _, claimed, lifted, absorbed, reclaimed, _)| {
                    *claimed && *lifted && *absorbed && *reclaimed
                });
        if (remote_morph_regression && remote_morph_witness.is_some())
            || (!remote_morph_regression && captured)
        {
            break;
        }
    }
    if remote_morph_regression {
        assert!(
            remote_morph_witness.is_some(),
            "actual remote Morph binding must admit the original holder/body generations and complete its callback/phase epilogue: {morph_diagnostics:?}"
        );
        return;
    }
    assert!(
        saw_flyer && saw_field && submitted && admitted_prelude && consumed_lane && castle_debited,
        "actual public Steal Mana III flyer/field/lane/prelude/Castle debit: {saw_flyer} {saw_field} {submitted} {admitted_prelude} {consumed_lane} {castle_debited}; prelude={prelude_diagnostics:?} emitted={emitted:?} observed={observations:?} sound={pursuit_sound} cast={cast_diagnostics:?} hunter={hunter_diagnostics:?} last={hunter_last} lifts={lift_diagnostics:?} last_lift={last_lift:?}"
    );
    assert!(
        pursuit_sound
            && observations
                .iter()
                .any(|(_, _, claimed, lifted, absorbed, reclaimed, _)| *claimed
                    && *lifted
                    && *absorbed
                    && *reclaimed),
        "actual spawned sphere -> original hunter claim/lift/absorption/reclaim; sound={pursuit_sound}: {observations:?}; hunter={hunter_diagnostics:?} last={hunter_last} lifts={lift_diagnostics:?} last_lift={last_lift:?}"
    );
}

#[test]
fn level18_public_meteor_retires_original_mana_hunter() {
    use super::mc2::{start_callback_damage_trace, take_callback_damage_trace};
    use crate::PlayerCommand;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent, area_damage};
    use std::path::PathBuf;

    let package = std::env::var_os("MC_CREATURE23_LEVEL18_PACKAGE").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
                "../../../generated/all-spells-20261104-integration/current-owner-package/level18",
            )
        },
        PathBuf::from,
    );
    let mut session = CampaignSession::open(package, CampaignSelection::ordinary(18).unwrap())
        .expect("qualified whole Level18")
        .simulation()
        .clone();
    let player = session.state.entity_index;
    let root = session
        .source_arena()
        .handle(29)
        .expect("actual reset hunter");
    let original = session
        .creature_identity_for_source_slot(root.source_slot)
        .expect("actual source root identity");
    assert_eq!(
        original.kind,
        crate::level_definition::CreatureKind::Creature23
    );
    assert_eq!(original.handle, root);
    assert!(original.living);
    let source = &session.entities()[usize::from(root.source_slot)];
    assert_eq!(
        (source.class, source.model, source.life, source.position.z),
        (5, 23, RULES.max_life, 8192)
    );
    assert!(
        session
            .campaign_entity_presentations(0)
            .unwrap()
            .iter()
            .any(|view| view.entity_index == root.source_slot)
    );

    // Declared prior-campaign Meteor carry and XP, player resources, normal
    // Castle prerequisite and a single player-only firing pose. The authored
    // hunter, its scheduler, health, RNG and lifetime are not changed.
    session.apply_carried_spell_families(&[9], Some(9)).unwrap();
    for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
        session
            .economy_mut()
            .set_castle_stage_footprint(stage, footprint);
    }
    let site = (1_u16..32)
        .flat_map(|y| {
            (1_u16..32).map(move |x| Position {
                x: x * 2048,
                y: y * 2048,
                z: 0,
            })
        })
        .find(|position| {
            !session.economy().castle_projectile_site_blocked(
                *position,
                session.entities(),
                session.canonical_terrain(),
            )
        })
        .expect("normal Castle site");
    let castle = session
        .economy_mut()
        .build_castle(site, player)
        .expect("normal Castle constructor");
    assert!(
        session
            .economy_mut()
            .set_castle_mana_prerequisite_for_test(castle, 200_000)
    );
    session
        .shared_spells_mut()
        .award_spell_experience(player, 9, 5_000);
    {
        let live = &mut session.session.state.live;
        let parent = &mut live.economy.entities[usize::from(player)];
        parent.set_max_life(200_000);
        parent.set_life(200_000);
        parent.max_mana = 200_000;
        parent.record[144..148].copy_from_slice(&parent.max_mana.to_le_bytes());
        parent.set_mana(200_000);
        parent.set_mana_regen(0);
        let parent = parent.clone();
        live.player_life = parent.life;
        live.shared_spells
            .sync_player_entity_from_authority(&parent);
        live.shared_spells.player_mana = parent.mana;
        live.shared_spells.caster.mana = parent.mana;
        live.shared_spells
            .sync_typed_spell_parent_caches_from_player_entity();
        if let Some(fire) = live.core.context.spell_fire.as_mut() {
            fire.caster.mana = parent.mana;
        }
    }
    let ui = crate::SOURCE_SPELL_INDEX_MAP
        .iter()
        .position(|family| *family == 9)
        .unwrap() as i8;
    session.step_outer_turn(
        PlayerCommand {
            opcode: 31,
            argument_1: ui,
            argument_2: 2,
            ..Default::default()
        },
        0,
    );
    assert_eq!(session.source_arena().handle(root.source_slot), Some(root));
    let holder = session.shared_spells().local_player().enabled_spell_holders[9];
    assert_ne!(holder, 0);
    assert_eq!(
        session.shared_spells().pickup(holder).unwrap().level_gate,
        2
    );
    let holder_handle = session.source_arena().handle(holder).unwrap();
    let caster_handle = session.source_arena().handle(player).unwrap();
    let current = session.entities()[usize::from(root.source_slot)].clone();
    assert!(current.life >= 0 && current.mana > 0);
    let mut origin = current.position;
    crate::move_entity(&mut origin, 1024, 0, 2048);
    origin.z = session
        .canonical_terrain()
        .ground_height(current.position)
        .wrapping_add(768);
    session.set_level2_entity_position(player, origin).unwrap();
    session.state.actual_speed = 0;
    session.state.target_speed = 0;
    let (_, pitch) = crate::level1_mission::projectile_guidance_angles(origin, current.position);
    session.set_level2_player_aim_for_test(0, pitch);
    let mut no_cast = session.clone();
    let credit_before = session.economy().authored_bee_kill_census(player);
    area_damage::start_actor_damage_trace(&[root]);
    area_damage::start_effect_damage_trace(root.source_slot);
    let mut callbacks = Vec::new();
    let mut launches = Vec::new();
    let mut impacts = Vec::new();
    let mut saw_living_root_loss = false;
    let mut saw_mana = false;
    let mut saw_hidden = false;
    let mut saw_hidden_presentation_loss = false;
    let mut saw_burst_dto = false;
    let mut turns = 0;
    for turn in 0..160 {
        let event_start = session.economy().events().len();
        session.start_creature17_trace();
        start_callback_damage_trace(root);
        session.step_outer_turn(
            PlayerCommand {
                flags: if turn == 0 { 0x10 } else { 0 },
                ..Default::default()
            },
            0,
        );
        let trace = session.take_creature17_trace();
        launches.extend(trace.meteor_launches);
        impacts.extend(trace.meteor_impacts);
        let visits = take_callback_damage_trace();
        let death_visit = visits.iter().any(|visit| {
            visit.actor == root
                && visit.before.action == 189
                && visit
                    .after
                    .as_ref()
                    .is_some_and(|row| row.drawing_disabled())
        });
        saw_hidden |= death_visit;
        if death_visit {
            saw_mana |= session.economy().events()[event_start..]
                .iter()
                .any(|event| {
                    matches!(event,
                Level2EconomyEvent::ManaSphereSpawned { source_entity_index, mana, .. }
                    if *source_entity_index == root.source_slot && *mana > 0)
                });
            saw_hidden_presentation_loss |= !session
                .campaign_entity_presentations(0)
                .unwrap()
                .iter()
                .any(|view| view.entity_index == root.source_slot);
        }
        callbacks.extend(visits);
        if session.source_arena().handle(root.source_slot) == Some(root)
            && session.entities()[usize::from(root.source_slot)].life < 0
        {
            saw_living_root_loss |= !session.source_arena().rebuilt_class5_model_slots()
                [usize::from(SOURCE_MODEL)]
            .contains(&root.source_slot);
        }
        for (&slot, &(generation, parent)) in session.source_arena().effects.death_burst_origins() {
            if parent == root.source_slot
                && session.source_arena().slot_generation(slot) == generation
            {
                let burst = &session.entities()[usize::from(slot)];
                saw_burst_dto |= burst.active
                    && (burst.class, burst.model) == (10, 1)
                    && session
                        .campaign_entity_presentations(0)
                        .unwrap()
                        .iter()
                        .any(|view| view.entity_index == slot && view.source_id == burst.source_id);
            }
        }
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "turn {turn}: {:?}",
            session.dispatch_events()
        );
        turns += 1;
        if session.source_arena().resolve(root).is_none() && saw_hidden && saw_mana && saw_burst_dto
        {
            break;
        }
    }
    let receipts = area_damage::take_effect_damage_trace();
    for turn in 0..turns {
        no_cast.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            no_cast
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "no-cast control turn {turn}: {:?}",
            no_cast.dispatch_events()
        );
    }
    let fatal_match = callbacks.iter().any(|callback| {
        let Some(after) = &callback.after else {
            return false;
        };
        let Some(writer) = callback.entry_writer else {
            return false;
        };
        let damage = i32::from_le_bytes(callback.before.record[98..102].try_into().unwrap());
        let attacker =
            u16::from_le_bytes([callback.before.record[102], callback.before.record[103]]);
        callback.actor == root
            && writer.target == root
            && callback.before.life >= 0
            && damage > 0
            && u16::from_le_bytes([callback.before.record[56], callback.before.record[57]]) == 0
            && attacker == player
            && writer.mailbox == (damage, player)
            && after.life == callback.before.life.wrapping_sub(damage)
            && after.life < 0
            // Source27E00's deferred invalid-target tail can overwrite 188
            // with exactly 187/3/0; the credited 188->189 visit is still
            // required independently below.
            && (after.action == 188
                || (callback.before.action == 186
                    && after.action == 187
                    && after.record[74] == 3
                    && after.source_word_96() == 0
                    && after.source_dword_10() == 0))
            && u16::from_le_bytes([after.record[40], after.record[41]]) == player
            && u16::from_le_bytes([after.record[102], after.record[103]]) == 0
            && receipts.iter().any(|receipt| {
                receipt.target_slot == root.source_slot
                    && receipt.target_generation == root.generation
                    && receipt.source_scoped
                    && receipt.pulse.owner == player
                    && receipt.pulse.payload == 1600
                    && receipt.writer_sequence == Some(writer.sequence)
                    && impacts.iter().any(|impact| {
                        receipt.source_slot == Some(impact.impact.source_slot)
                            && writer.source
                                == area_damage::PlayerMailboxSource::Effect(impact.impact)
                            && (
                                impact.state.source_class,
                                impact.state.model,
                                impact.state.subspell_index,
                            ) == (10, 17, 16_000)
                            && launches.iter().any(|launch| {
                                launch.holder == holder_handle
                                    && launch.caster == caster_handle
                                    && launch.projectile == impact.projectile
                                    && (
                                        launch.packet.class,
                                        launch.packet.model,
                                        launch.packet.subspell_index,
                                    ) == (9, 3, 16_000)
                            })
                    })
            })
    });
    let credited_prekill = callbacks.iter().any(|visit| {
        visit.actor == root
            && visit.before.action == 188
            && visit.kill_credit == Some(player)
            && visit.after.as_ref().is_some_and(|row| {
                row.action == 189 && u16::from_le_bytes([row.record[40], row.record[41]]) == player
            })
    });
    assert!(
        fatal_match && credited_prekill,
        "actual public Meteor must supply this original hunter's fatal mailbox and prekill credit: callbacks={callbacks:?} impacts={impacts:?} launches={launches:?} receipts={receipts:?}"
    );
    assert!(
        saw_living_root_loss
            && saw_mana
            && saw_burst_dto
            && saw_hidden
            && saw_hidden_presentation_loss
            && session.source_arena().resolve(root).is_none()
            && session.economy().authored_bee_kill_census(player) > credit_before,
        "actual original hunter must lose living-root membership, emit mana/burst DTO, hide and reclaim: root_loss={saw_living_root_loss} mana={saw_mana} burst={saw_burst_dto} hidden={saw_hidden} dto_loss={saw_hidden_presentation_loss}"
    );
    assert!(
        no_cast
            .creature_identity_for_source_slot(root.source_slot)
            .is_some_and(|identity| identity.handle == root && identity.living),
        "same-elapsed no-cast control retains the original living root"
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
        target_error: None, mana_candidates: &[],
        class3: &class3,
        families: &families, peers: &[],
        entities: &mut entities,
        spheres: &mut spheres,
        population: None,
        runtime: &mut runtime,
    };
    assert!(configured_step(&mut Actor { row: &mut row }, &mut context).is_err());
}

#[test]
fn configured_already_claimed_sphere_still_runs_capture_before_mode3_tail() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 8_192,
    };
    let mut row = actor(187, position);
    row.set_source_byte_46(0);
    row.set_source_dword_10(0);
    row.record[154..156].copy_from_slice(&12_u16.to_le_bytes());
    let mut sphere = mana_sphere(
        12,
        Position {
            x: 21_000,
            ..position
        },
        350,
    );
    sphere.control_flags = MANA_SPHERE_CLAIMED;
    sphere.assigned_defender_slot = 9;
    let mut spheres = [sphere];
    let mut entities = source_rows(16);
    let families = empty_families();
    let mut runtime = EmptyRoutes;
    let mut ctx = Context {
        terrain: &terrain,
        targets: &[],
        target_error: None, mana_candidates: &[],
        class3: &[],
        families: &families, peers: &[],
        entities: &mut entities,
        spheres: &mut spheres,
        population: None,
        runtime: &mut runtime,
    };
    configured_step(&mut Actor { row: &mut row }, &mut ctx).unwrap();
    assert_eq!(
        (row.action, row.record[74], row.source_dword_10()),
        (187, 3, 0)
    );
    assert_eq!(u16::from_le_bytes([row.record[48], row.record[49]]), 10);
    assert_eq!(ctx.spheres[0].assigned_defender_slot, 7);
    assert_eq!(
        u16::from_le_bytes([ctx.entities[12].record[154], ctx.entities[12].record[155]]),
        7
    );
    assert_ne!(ctx.spheres[0].control_flags & MANA_SPHERE_CLAIMED, 0);
    assert!(!ctx.spheres[0].pending_delete);
}

#[test]
fn configured_absorption_keeps_hidden_sphere_claim_when_same_pass_selects_victim() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 8_192,
    };
    let mut victim = target(9, 10_000);
    victim.castle = Some(Castle {
        position: Position {
            x: 30_000,
            y: 30_000,
            z: position.z,
        },
        half_x: 384,
        half_y: 384,
    });
    let targets = [victim];
    let mut row = actor(187, position);
    row.set_phase(0);
    row.set_source_byte_46(1);
    row.set_source_dword_10(2);
    row.record[154..156].copy_from_slice(&12_u16.to_le_bytes());
    let mut sphere = mana_sphere(
        12,
        Position {
            z: 8_400,
            ..position
        },
        350,
    );
    sphere.control_flags = MANA_SPHERE_CLAIMED;
    sphere.assigned_defender_slot = 7;
    let mut spheres = [sphere];
    let mut entities = source_rows(16);
    entities[12].set_control_flags(MANA_SPHERE_CLAIMED);
    entities[12].record[154..156].copy_from_slice(&7_u16.to_le_bytes());
    let families = empty_families();
    let mut runtime = EmptyRoutes;
    let mut ctx = Context {
        terrain: &terrain,
        targets: &targets,
        target_error: None, mana_candidates: &[],
        class3: &[9],
        families: &families, peers: &[],
        entities: &mut entities,
        spheres: &mut spheres,
        population: None,
        runtime: &mut runtime,
    };
    configured_step(&mut Actor { row: &mut row }, &mut ctx).unwrap();
    assert_eq!(
        (
            row.action,
            u16::from_le_bytes([row.record[154], row.record[155]])
        ),
        (186, 9)
    );
    assert_eq!(row.mana, RULES.initial_mana + 350);
    assert!(ctx.spheres[0].pending_delete);
    assert_ne!(
        ctx.spheres[0].control_flags & (MANA_SPHERE_CLAIMED | ENTITY_DRAW_DISABLED_BIT),
        0
    );
    assert_eq!(
        ctx.spheres[0].control_flags & (MANA_SPHERE_CLAIMED | ENTITY_DRAW_DISABLED_BIT),
        MANA_SPHERE_CLAIMED | ENTITY_DRAW_DISABLED_BIT
    );
    assert_eq!(ctx.spheres[0].assigned_defender_slot, 7);
    assert_eq!(
        u16::from_le_bytes([ctx.entities[12].record[154], ctx.entities[12].record[155]]),
        7
    );
}

#[test]
fn configured_retarget_keeps_far_sphere_before_climb_and_handles_absence() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 8_192,
    };
    let far = Ok(Some(ManaCandidate {
        slot: 12,
        position: Position {
            x: 25_000,
            ..position
        },
    }));
    let near = Ok(Some(ManaCandidate {
        slot: 11,
        position: Position {
            x: 21_000,
            ..position
        },
    }));
    for (candidates, expected) in [
        (&[far][..], (185, 1, 0, 12)),
        (&[][..], (185, 1, 0, 0)),
        (&[near][..], (185, 0, 500, 11)),
    ] {
        let mut row = actor(187, position);
        row.set_source_byte_46(3);
        row.set_source_dword_10(31);
        row.record[154..156].copy_from_slice(&13_u16.to_le_bytes());
        let mut entities = source_rows(16);
        let mut spheres = [];
        let families = empty_families();
        let mut runtime = EmptyRoutes;
        let mut ctx = Context {
            terrain: &terrain,
            targets: &[],
            target_error: None, mana_candidates: candidates,
            class3: &[],
            families: &families, peers: &[],
            entities: &mut entities,
            spheres: &mut spheres,
            population: None,
            runtime: &mut runtime,
        };
        configured_step(&mut Actor { row: &mut row }, &mut ctx).unwrap();
        assert_eq!(
            (
                row.action,
                row.record[74],
                row.source_dword_10(),
                u16::from_le_bytes([row.record[154], row.record[155]])
            ),
            expected
        );
    }
}

#[test]
fn configured_attack_moves_before_steering_and_ray_request() {
    let terrain = terrain();
    let position = Position { x: 20_000, y: 20_000, z: 8_192 };
    let targets = [target(12, 10_000)];
    let families = empty_families();
    let mut entities = source_rows(16);
    let mut spheres = [];
    let mut runtime = EmptyRoutes;
    let mut row = actor(186, position);
    row.set_phase(0);
    row.set_yaw(0);
    row.set_source_roll(0);
    row.record[154..156].copy_from_slice(&12_u16.to_le_bytes());
    let mut context = Context {
        terrain: &terrain, targets: &targets, target_error: None, mana_candidates: &[], class3: &[], families: &families, peers: &[],
        entities: &mut entities, spheres: &mut spheres, population: None, runtime: &mut runtime,
    };
    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();
    let ray = effects.lightning.expect("actual source descriptor cadence requests Ray");
    let mut moved = position;
    crate::move_entity(&mut moved, 0, 0, RULES.minimum_speed);
    assert_eq!(row.position, moved, "unblocked sub1B8C0 commits movement first");
    assert_eq!(row.yaw, 0, "heading change applies to roll until the next source movement");
    assert_eq!(row.source_roll(), heading_for_positions(moved, targets[0].position) as i16);
    assert_eq!(ray.position, Position { z: moved.z.wrapping_add(row.array_fov()), ..moved });
    assert!(effects.combat_tail_pending, "Ray allocation precedes source combat tail");
}

#[test]
fn configured_motion_hold_preserves_shared_prediction_and_population() {
    let terrain = terrain();
    let before_prediction = Position { x: 1_234, y: 4_321, z: 777 };
    terrain.set_source_predicted_position(before_prediction);
    let position = Position { x: 20_000, y: 20_000, z: 8_192 };
    let mut row = actor(184, position);
    row.set_control_flags(row.control_flags | 0x800);
    let mut population = 5;
    let mut entities = source_rows(16);
    let mut spheres = [];
    let families = empty_families();
    let mut runtime = EmptyRoutes;
    let mut ctx = Context {
        terrain: &terrain, targets: &[], target_error: None, mana_candidates: &[], class3: &[], families: &families, peers: &[],
        entities: &mut entities, spheres: &mut spheres, population: Some(&mut population), runtime: &mut runtime,
    };
    move_forward(&mut Actor { row: &mut row }, &mut ctx);
    assert_eq!(row.position, position);
    assert_eq!(row.control_flags & 0x800, 0);
    assert_eq!(terrain.source_predicted_position(), before_prediction);
    assert_eq!(population, 5);
}

#[test]
fn configured_attack_defers_target_loss_until_after_fatal_tail_and_checks_range_on_cadence() {
    let terrain = terrain();
    let position = Position { x: 20_000, y: 20_000, z: 8_192 };
    let families = empty_families();
    let mut entities = source_rows(16);
    let mut spheres = [];
    let mut runtime = EmptyRoutes;
    let mut row = actor(186, position);
    row.set_life(100);
    row.set_source_dword_10(17);
    row.record[154..156].copy_from_slice(&12_u16.to_le_bytes());
    row.record[98..102].copy_from_slice(&101_i32.to_le_bytes());
    row.record[102..104].copy_from_slice(&9_u16.to_le_bytes());
    let mut ctx = Context { terrain: &terrain, targets: &[], target_error: None, mana_candidates: &[], class3: &[], families: &families, peers: &[], entities: &mut entities, spheres: &mut spheres, population: None, runtime: &mut runtime };
    configured_step(&mut Actor { row: &mut row }, &mut ctx).unwrap();
    assert_eq!((row.life, u16::from_le_bytes([row.record[40], row.record[41]])), (-1, 9));
    assert_eq!((row.action, row.record[74], row.source_dword_10(), u16::from_le_bytes([row.record[154], row.record[155]])), (187, 3, 0, 0), "source v8 tail follows and overrides prekill action");

    let mut distant = target(12, 10_000);
    distant.position.x = position.x + 8_000;
    let targets = [distant];
    for (phase, action) in [(0, 187), (1, 186)] {
        let mut row = actor(186, position);
        row.set_phase(phase);
        row.record[154..156].copy_from_slice(&12_u16.to_le_bytes());
        let mut entities = source_rows(16);
        let mut spheres = [];
        let mut runtime = EmptyRoutes;
        let mut ctx = Context { terrain: &terrain, targets: &targets, target_error: None, mana_candidates: &[], class3: &[], families: &families, peers: &[], entities: &mut entities, spheres: &mut spheres, population: None, runtime: &mut runtime };
        let effects = configured_step(&mut Actor { row: &mut row }, &mut ctx).unwrap();
        assert_eq!(row.action, action, "range is evaluated only on source descriptor cadence");
        assert!(effects.lightning.is_none());
    }
}

#[test]
fn configured_class3_sentinel_damage_retaliates_and_releases_live_claim() {
    let terrain = terrain();
    let position = Position { x: 20_000, y: 20_000, z: 8_192 };
    let mut entities = source_rows(16);
    entities[0].class = 3;
    let mut sphere = mana_sphere(12, Position { x: 22_000, z: 7_000, ..position }, 350);
    sphere.control_flags = MANA_SPHERE_CLAIMED;
    sphere.assigned_defender_slot = 7;
    let mut spheres = [sphere];
    entities[12].set_control_flags(MANA_SPHERE_CLAIMED);
    entities[12].record[154..156].copy_from_slice(&7_u16.to_le_bytes());
    let mut row = actor(187, position);
    row.set_phase(1);
    row.set_source_byte_46(1);
    row.set_source_dword_10(4);
    row.record[154..156].copy_from_slice(&12_u16.to_le_bytes());
    row.record[98..102].copy_from_slice(&10_i32.to_le_bytes());
    row.record[102..104].copy_from_slice(&9_u16.to_le_bytes());
    let families = empty_families();
    let mut runtime = EmptyRoutes;
    let mut ctx = Context { terrain: &terrain, targets: &[], target_error: None, mana_candidates: &[], class3: &[], families: &families, peers: &[], entities: &mut entities, spheres: &mut spheres, population: None, runtime: &mut runtime };
    configured_step(&mut Actor { row: &mut row }, &mut ctx).unwrap();
    assert_eq!(row.life, RULES.max_life - 10);
    assert_eq!((row.action, row.record[74], row.source_dword_10(), u16::from_le_bytes([row.record[154], row.record[155]])), (186, 0, 0, 9));
    assert_eq!(ctx.spheres[0].control_flags & MANA_SPHERE_CLAIMED, 0);
    assert_eq!(ctx.spheres[0].assigned_defender_slot, 0);
}

#[test]
fn tunuk_castle_objectives_real_waves_and_ordinary_exit() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::campaign::source_arena::{CreatureRootOrigin, SourceEntityHandle};
    use crate::game_session::{GameCompletionStatus, GameSession};
    use crate::level_definition::{CreatureKind, ObjectiveCondition};
    use crate::native_world::{Mc2DispatchEvent, RuntimeRelation, area_damage};
    use crate::native_world::player_lifecycle_tests::{
        RivalTestGame, rival_test_sim as sim, rival_test_sim_mut as sim_mut,
        rival_test_step as step,
    };
    use std::collections::BTreeSet;

    #[derive(Default)]
    struct Groups {
        counts: [usize; 4],
        exit: Option<(u16, Position)>,
        portal: Option<u16>,
    }
    fn visit(game: &mut RivalTestGame, groups: &mut Groups) {
        step(game);
        let state = sim(game);
        for event in state.dispatch_events() {
            let Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group, authored_indices, .. } = event else { continue };
            let expected = match *group {
                15 if authored_indices.contains(&481) => Some((0, 32, 0)),
                16 if authored_indices.contains(&486) => Some((1, 32, 1)),
                17 if authored_indices.contains(&518) => Some((2, 32, 2)),
                13 if authored_indices.contains(&414) => Some((3, 4, 0)),
                _ => None,
            };
            let Some((index, model, gate)) = expected else { continue };
            groups.counts[index] += 1;
            let producer = &state.entities()[usize::from(*source_slot)];
            assert_eq!((producer.class, producer.model, producer.source_id), (11, model, *group));
            if model == 32 { assert_eq!(producer.level_gate, gate); }
            if *group == 15 {
                assert!((417..=480).all(|row| authored_indices.contains(&row)),
                    "group15 includes all 64 real model58 records");
            }
            if *group == 16 {
                assert!([486, 487, 488, 511, 512, 515].into_iter().all(|row| authored_indices.contains(&row)));
            }
            if *group == 17 {
                assert!([514, 516, 518].into_iter().all(|row| authored_indices.contains(&row)));
            }
            if *group == 13 {
                let trigger = state.economy().authored_switch_producers().iter()
                    .find(|row| row.authored_index == 414 && (row.model, row.action) == (12, 12))
                    .expect("actual Tunuk exit414");
                let portal = state.economy().authored_class14().iter()
                    .find(|row| row.authored_index == 415 && row.model == 3)
                    .expect("actual Tunuk portal415");
                groups.exit = Some((trigger.source_entity_index, trigger.position));
                groups.portal = Some(portal.source_entity_index);
            }
        }
    }
    fn actual_authored(game: &RivalTestGame, kind: CreatureKind, authored: u16) -> SourceEntityHandle {
        sim(game).creature_runtime_census().roots_of_kind(kind)
            .find(|row| row.origin == (CreatureRootOrigin::AuthoredRecord { authored_record: authored }) && row.living)
            .unwrap_or_else(|| panic!("actual living {kind:?} authored row{authored}"))
            .handle
    }
    fn contact(game: &mut RivalTestGame, player: u16, handle: SourceEntityHandle) {
        assert_eq!(sim(game).source_arena().handle(handle.source_slot), Some(handle));
        area_damage::contact(sim_mut(game).source_arena_mut(), None, handle.source_slot, u16::MAX, player).unwrap();
    }

    let generated = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
    let package = std::env::var_os("MC_LEVEL14_PACKAGE").map(std::path::PathBuf::from)
        .unwrap_or_else(|| generated.join("mc-bhq6-ordinary-20261025-zclu5fjj/level14"));
    let cache = std::env::var_os("MC_LEVEL14_OWNER_CACHE").map(std::path::PathBuf::from)
        .unwrap_or_else(|| generated.join("campaign-original-environments/level14"));
    let definition = load_mc2_campaign(&package, &crate::game_definition::default_catalog().unwrap()).unwrap();
    let objectives = definition.level_definition().unwrap().objectives();
    assert_eq!(objectives.len(), 4);
    assert_eq!(objectives[0].condition(), ObjectiveCondition::CastleManaPercent(40));
    assert!(matches!(objectives[1].condition(), ObjectiveCondition::TargetDeath(_)));
    assert_eq!(objectives[2].condition(), ObjectiveCondition::NoLivingCreatures(CreatureKind::Creature23));
    assert_eq!(objectives[3].condition(), ObjectiveCondition::NoLivingCreatures(CreatureKind::Cymmerian));
    let mut game = GameSession::open(&Mc2CampaignAdapter, std::sync::Arc::new(definition), 2214,
        Mc2CampaignInputs::with_owner_cache(&package, &cache)).unwrap();
    let player = sim(&game).state.entity_index;
    let owner = usize::from(sim(&game).state.player_color_index);
    let mut groups = Groups::default();
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[..4], [1; 4]);
    // Declared prior acquisition: an existing Castle and pre-earned bank.
    // The ordinary source census and objective consumer decide the 40% gate.
    let castle = {
        let state = sim_mut(&mut game);
        let site = (1_u16..32).flat_map(|y| (1_u16..32).map(move |x| Position { x: x * 2048, y: y * 2048, z: 0 }))
            .map(|mut position| { position.z = state.canonical_terrain().ground_height(position); position })
            .find(|position| !state.economy().castle_projectile_site_blocked(*position, state.entities(), state.canonical_terrain()))
            .expect("declared prior Castle has a source-admitted site");
        state.economy_mut().build_castle(site, player).unwrap()
    };
    visit(&mut game, &mut groups);
    let total = sim(&game).economy().owner_mana_global();
    assert!(total > 0, "real source census supplies a denominator");
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[0], 1,
        "empty declared bank remains below the first objective");
    let prior_bank = total.checked_add(1_000).expect("source bank fits");
    assert!(sim_mut(&mut game).economy_mut().set_castle_mana_prerequisite_for_test(castle, prior_bank));
    for _ in 0..128 {
        visit(&mut game, &mut groups);
        if groups.counts[0] == 1 { break; }
    }
    let economy = sim(&game).economy();
    let castle_bank = economy.castle_at(castle).expect("declared Castle").mana;
    assert!(100_i64 * i64::from(economy.owner_mana_aggregate(player) + castle_bank)
        >= 40_i64 * i64::from(economy.owner_mana_global()), "real census passes 40%");
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[0], 2);
    assert_eq!(groups.counts[0], 1);
    let target = actual_authored(&game, CreatureKind::Creature23, 481);
    assert_eq!(sim(&game).runtime().stages[1].relation, RuntimeRelation::LiveEntity(target.source_slot));
    contact(&mut game, player, target);
    for _ in 0..256 {
        visit(&mut game, &mut groups);
        if groups.counts[1] == 1 { break; }
    }
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[..3], [2, 2, 1]);
    assert_eq!(groups.counts[1], 1);
    let hunters = [486, 487, 488, 515].map(|row| actual_authored(&game, CreatureKind::Creature23, row));
    for &handle in &hunters[..3] { contact(&mut game, player, handle); }
    for _ in 0..256 {
        visit(&mut game, &mut groups);
        assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[2], 1,
            "one living source hunter blocks the absence objective");
        if hunters[..3].iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()) { break; }
    }
    assert!(hunters[..3].iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()));
    assert!(sim(&game).creature_identity_for_source_slot(hunters[3].source_slot)
        .is_some_and(|row| row.handle == hunters[3] && row.living));
    contact(&mut game, player, hunters[3]);
    for _ in 0..256 {
        visit(&mut game, &mut groups);
        if groups.counts[2] == 1 { break; }
    }
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[..4], [2, 2, 2, 1]);
    assert_eq!(groups.counts[2], 1);
    let authored = [514, 516, 518].map(|row| actual_authored(&game, CreatureKind::Cymmerian, row));
    let mut seen = BTreeSet::new();
    let mut contacted = BTreeSet::new();
    let mut split_seen = false;
    for leave_one in [true, false] {
        for _ in 0..768 {
            for root in sim(&game).creature_runtime_census().roots_of_kind(CreatureKind::Cymmerian) {
                split_seen |= matches!(root.origin, CreatureRootOrigin::CreatureConversion { .. });
                seen.insert((root.handle.source_slot, root.handle.generation));
            }
            let candidates = sim(&game).entities().iter().filter(|row| row.active && row.class == 5 && row.model == 25
                && row.life >= 0 && (row.action != 200 || row.record_bytes()[74] >= 3))
                .map(|row| sim(&game).source_arena().handle(row.index).unwrap()).collect::<Vec<_>>();
            for handle in candidates {
                if leave_one && handle == authored[2] { continue; }
                if contacted.insert((handle.source_slot, handle.generation)) { contact(&mut game, player, handle); }
            }
            visit(&mut game, &mut groups);
            if leave_one {
                assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[3], 1,
                    "one real Cymmerian blocks the final absence objective");
                if seen.iter().all(|&(slot, generation)| {
                    let handle = SourceEntityHandle { source_slot: slot, generation };
                    handle == authored[2] || sim(&game).source_arena().resolve(handle).is_none()
                }) { break; }
            } else if groups.counts[3] == 1 && seen.iter().all(|&(slot, generation)|
                sim(&game).source_arena().resolve(SourceEntityHandle { source_slot: slot, generation }).is_none()) { break; }
        }
        if leave_one {
            assert!(sim(&game).creature_identity_for_source_slot(authored[2].source_slot)
                .is_some_and(|row| row.handle == authored[2] && row.living));
            assert!(seen.iter().all(|&(slot, generation)| {
                let handle = SourceEntityHandle { source_slot: slot, generation };
                handle == authored[2] || sim(&game).source_arena().resolve(handle).is_none()
            }));
            assert_eq!(groups.counts[3], 0, "one survivor blocks actual level-end exit production");
        }
    }
    assert!(authored.iter().all(|handle| seen.contains(&(handle.source_slot, handle.generation))));
    assert!(split_seen, "actual large Cymmerian death produces natural descendants");
    assert!(seen.iter().all(|&(slot, generation)|
        sim(&game).source_arena().resolve(SourceEntityHandle { source_slot: slot, generation }).is_none()));
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[..4], [2; 4]);
    assert_eq!(groups.counts, [1; 4]);
    let (trigger, position) = groups.exit.expect("actual exit414");
    let portal = groups.portal.expect("actual portal415");
    let exit = sim(&game).level_exit_sequence();
    if !exit.active && !exit.complete { sim_mut(&mut game).set_level2_entity_position(player, position).unwrap(); }
    for _ in 0..700 {
        if matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }) { break; }
        visit(&mut game, &mut groups);
    }
    assert!(matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }));
    let exit = sim(&game).level_exit_sequence();
    assert_eq!((exit.trigger_entity_index, exit.portal_entity_index), (trigger, portal));
    assert_eq!(sim(&game).source_exit_result_byte(), 0x0a);
}
