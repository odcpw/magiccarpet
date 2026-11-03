use super::super::archer::ArcherStageDescriptor as StageDescriptor;
use super::*;

#[test]
fn alliance_follows_retained_parent_and_expires_when_dead() {
    let (mut arena, mut runtime, mut terrain, particles, player, feature) = mission_eighteen_source_parts();
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let slot = mc2::allocate_authored(&mut arena, &mut runtime, &mut terrain, &particles,
        position, RULES.max_life, None, feature.index).unwrap().unwrap();
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


struct EmptyRoutes;

impl StageRuntime for EmptyRoutes {
    fn descriptor(&self, _index: u8) -> Option<StageDescriptor> {
        None
    }

    fn bind(&mut self, _index: u8) -> Option<StageDescriptor> {
        None
    }

    fn rearm_model_relations(&mut self) {}
}

fn terrain() -> SessionTerrain {
    crate::native_world::tests::level_two_campaign_session()
        .canonical_terrain()
        .clone()
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

fn mission_eighteen_feature() -> crate::level1_mission::AuthoredFeature {
    let package = crate::campaign::spec::DecodedCampaignPackage::decode(
        mission_eighteen_root().join("level18"),
        crate::campaign::spec::CampaignSelection::ordinary(18).unwrap(),
    )
    .expect("decode owner-data Mission 18 package");
    let feature = crate::level1_mission::parse_authored_features(package.authored_bytes())
        .expect("parse owner-data Mission 18 feature table")
        .into_iter()
        .find(|feature| feature.index == 172)
        .expect("Mission 18 authored record172 exists");
    assert_eq!(
        (feature.kind, feature.subtype, feature.activation_group),
        (5, 16, 2),
        "owner-data record172 is an ordinary group2 Wyvern placement"
    );
    feature
}

fn mission_eighteen_source_parts() -> (
    crate::campaign::source_arena::SourceArena,
    crate::native_world::RuntimeSnapshot,
    SessionTerrain,
    Vec<crate::native_world::ParticleParameter>,
    u16,
    crate::level1_mission::AuthoredFeature,
) {
    let package = crate::campaign::spec::DecodedCampaignPackage::decode(
        mission_eighteen_root().join("level18"),
        crate::campaign::spec::CampaignSelection::ordinary(18).unwrap(),
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
        .expect("Mission 18 retains a class3/model0 target");
    let player_slot = player.index;
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
    let feature = crate::level1_mission::parse_authored_features(package.authored_bytes())
        .unwrap()
        .into_iter()
        .find(|feature| feature.index == 172)
        .expect("Mission 18 authored record172 exists");
    assert_eq!(
        (feature.kind, feature.subtype, feature.activation_group),
        (5, 16, 2)
    );
    (
        crate::campaign::source_arena::SourceArena::new(entities, economy),
        runtime,
        terrain,
        presentation.particles.to_vec(),
        player_slot,
        feature,
    )
}

fn wyvern(action: u8, position: Position) -> EntitySnapshot {
    let mut row = crate::native_world::cave_features::new_event(7, 19, position);
    row.class = 5;
    row.model = SOURCE_MODEL;
    row.record[67] = 5;
    row.record[68] = SOURCE_MODEL;
    row.set_source_action(action);
    row.set_life(10_000);
    row.set_source_dword_10(1);
    row.record[92..94].copy_from_slice(&128_i16.to_le_bytes());
    row
}

fn target(slot: u16, class: u8, model: u8, position: Position) -> Target {
    Target {
        slot,
        source_id: slot,
        class,
        model,
        life: 10_000,
        position,
        ..Target::default()
    }
}

fn projectile() -> Projectile {
    Projectile {
        parent: 7,
        owner: 7,
        target: 42,
        target_class: 3,
        target_model: 0,
        position: Position {
            x: 500,
            y: 600,
            z: 700,
        },
        target_position: Position {
            x: 900,
            y: 1200,
            z: 300,
        },
        yaw: 117,
        pitch: 219,
        payload: RULES.projectile_payload,
        damage: RULES.projectile_damage,
    }
}

#[test]
fn wyvern_attack_emits_one_descriptor61_targeted_projectile_and_decrements_burst() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 400,
    };
    let target_position = Position {
        x: 21_000,
        y: 20_000,
        z: 400,
    };
    let target = target(42, 3, 0, target_position);
    let targets = [target];
    let class3 = [target.slot];
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let mut routes = EmptyRoutes;
    let mut row = wyvern(130, position);
    row.record[154..156].copy_from_slice(&target.slot.to_le_bytes());
    row.phase = 8;
    row.set_yaw(heading_for_positions(position, target_position) as i16);
    row.set_control_flags(row.control_flags | 0x800);
    row.set_source_dword_10(2);

    let mut actor = Actor { row: &mut row };
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        family: &families[16],
        families: &families,
        runtime: &mut routes,
    };
    let effects = step(&mut actor, &mut context).unwrap();

    let launch = effects
        .projectile
        .expect("positive burst emits its own projectile");
    assert_eq!(
        (launch.target, launch.target_class, launch.target_model),
        (42, 3, 0)
    );
    assert_eq!(
        launch.position.z,
        position.z.wrapping_add(128_i16.wrapping_mul(6))
    );
    assert_eq!(launch.payload, RULES.projectile_payload);
    assert_eq!(launch.damage, RULES.projectile_damage);
    assert_eq!(
        actor.row.source_dword_10(),
        1,
        "source decrements before allocation"
    );
}

#[test]
fn wyvern_attack_with_unresolved_target_does_not_consume_burst_or_launch() {
    let terrain = terrain();
    let mut routes = EmptyRoutes;
    let mut row = wyvern(
        130,
        Position {
            x: 20_000,
            y: 20_000,
            z: 400,
        },
    );
    row.record[154..156].copy_from_slice(&42_u16.to_le_bytes());
    row.set_phase(8);
    row.set_source_dword_10(2);
    let targets = [];
    let class3 = [];
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let mut actor = Actor { row: &mut row };
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        family: &families[16],
        families: &families,
        runtime: &mut routes,
    };

    let effects = step(&mut actor, &mut context).unwrap();

    assert!(effects.projectile.is_none());
    assert_eq!(actor.row.source_dword_10(), 2);
    assert_eq!(actor.row.action, 130);
}

#[test]
fn wyvern_unsupported_action134_is_an_error_instead_of_a_silent_noop() {
    let terrain = terrain();
    let mut routes = EmptyRoutes;
    let mut row = wyvern(134, Position::default());
    let targets = [];
    let class3 = [];
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let mut actor = Actor { row: &mut row };
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        family: &families[16],
        families: &families,
        runtime: &mut routes,
    };

    let error = step(&mut actor, &mut context).unwrap_err();
    assert!(error.contains("action134 has no callback implementation"));
    assert_eq!(actor.row.action, 134);
}

#[test]
fn wyvern_self_replacement_reaims_from_cleared_slot_and_keeps_descriptor61_target() {
    let launch = projectile();
    let replacement_fov = 48;
    let state = mc2::projectile_launch_state(launch, 7, true, replacement_fov);
    let expected = crate::level1_mission::projectile_guidance_angles(
        Position::default(),
        launch.target_position,
    );

    assert_eq!(state.position, Position { x: 0, y: 0, z: 288 });
    assert_eq!((state.yaw, state.pitch), expected);
    assert_eq!((state.owner, state.target), (7, 0));
    assert_eq!((state.target_class, state.target_model), (3, 0));
}

#[test]
fn wyvern_non_replacement_keeps_source_pose_angles_and_target_relation() {
    let launch = projectile();
    let state = mc2::projectile_launch_state(launch, 19, false, 48);

    assert_eq!(state.position, launch.position);
    assert_eq!((state.yaw, state.pitch), (launch.yaw, launch.pitch));
    assert_eq!((state.owner, state.target), (launch.owner, launch.target));
    assert_eq!((state.target_class, state.target_model), (3, 0));
}

#[test]
fn wyvern_mission18_record172_constructs_and_launches_one_descriptor61_fireball() {
    let (mut arena, mut runtime, mut terrain, particles, target_slot, feature) =
        mission_eighteen_source_parts();
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
    .expect("Mission 18 ordinary producer allocates its Wyvern");
    let source_id = arena.entities[usize::from(slot)].source_id;
    {
        let row = &mut arena.entities[usize::from(slot)];
        assert_eq!((row.class, row.model, row.action), (5, SOURCE_MODEL, 129));
        assert_eq!(row.parameter_relation, Some(DESCRIPTOR));
        row.set_source_action(130);
        row.set_phase(8);
        row.set_source_dword_10(1);
        row.set_control_flags(row.control_flags | 0x800);
        row.record[154..156].copy_from_slice(&target_slot.to_le_bytes());
    }
    let before = arena.economy.fireball_projectiles().len();
    let class3 = arena
        .entities
        .iter()
        .filter(|row| row.active && row.class == 3 && row.life >= 0)
        .map(|row| row.index)
        .collect::<Vec<_>>();
    let families: [Vec<u16>; 29] = std::array::from_fn(|model| {
        arena
            .entities
            .iter()
            .filter(|row| row.active && row.class == 5 && usize::from(row.model) == model)
            .map(|row| row.index)
            .collect()
    });
    let control_arena = arena.clone();
    let control_runtime = runtime.clone();
    let control_terrain = terrain.clone();
    let outcome = mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        slot,
        &class3,
        &families,
    )
    .unwrap();

    let emitted = outcome.projectile.expect("owner-data Wyvern attack emits");
    assert_eq!(
        (emitted.target, emitted.target_class, emitted.target_model),
        (target_slot, 3, 0)
    );
    assert_eq!(arena.economy.fireball_projectiles().len(), before + 1);
    let projectile = arena
        .economy
        .fireball_projectiles()
        .last()
        .expect("Wyvern launch is already registered in the shared Fireball arena");
    assert_eq!(
        projectile.origin,
        crate::native_world::FireballProjectileOrigin::Wyvern
    );
    assert_eq!((projectile.source_class, projectile.source_model), (9, 0));
    assert_eq!(projectile.owner_entity_index, source_id);
    assert_eq!((projectile.target_class, projectile.target_model), (3, 0));
    assert_eq!(projectile.target_source_entity_index, target_slot);
    assert_eq!(projectile.source_subspell_index, RULES.projectile_payload);
    assert_eq!(projectile.source_mana, RULES.projectile_damage);
    assert_ne!(projectile.control_flags & 0x0008_0000, 0);
    let light = arena
        .dynamic_lighting
        .active_slots()
        .find(|(_, light)| light.source_entity_index == projectile.source_entity_index)
        .map(|(_, light)| light)
        .expect("real Night Fireball constructor admits its linked light");
    assert_eq!(
        light.position, position,
        "admission precedes the muzzle lift"
    );
    assert_eq!(
        (light.intensity, light.jitter_range, light.life_scaled),
        (128, 1, false)
    );

    // Identical real source attack with a disabled option or occupied finite
    // light pool: only light admission is suppressed, never the projectile.
    for (enabled, full_pool) in [(false, false), (true, true)] {
        let mut arena = control_arena.clone();
        let mut runtime = control_runtime.clone();
        let mut terrain = control_terrain.clone();
        arena.dynamic_lighting_enabled = enabled;
        if full_pool {
            for _ in 0..crate::dynamic_lighting::DYNAMIC_LIGHT_CAPACITY {
                assert!(
                    arena
                        .dynamic_lighting
                        .admit(true, true, slot, position, 128, 1, false,)
                        .is_some()
                );
            }
        }
        let lights_before = arena.dynamic_lighting.active_count();
        let outcome = mc2::step_row(
            &mut arena,
            &mut runtime,
            &mut terrain,
            &particles,
            slot,
            &class3,
            &families,
        )
        .unwrap();
        assert!(outcome.projectile.is_some());
        assert_eq!(arena.economy.fireball_projectiles().len(), before + 1);
        let projectile = arena.economy.fireball_projectiles().last().unwrap();
        assert_eq!(projectile.control_flags & 0x0008_0000, 0);
        assert_eq!(projectile.source_mana, RULES.projectile_damage);
        assert_eq!(projectile.target_source_entity_index, target_slot);
        assert_eq!(arena.dynamic_lighting.active_count(), lights_before);
    }
}

#[test]
fn wyvern_mission18_source_pass_launches_dispatches_and_reclaims_fireball() {
    let feature = mission_eighteen_feature();
    let mut session = crate::campaign::CampaignSession::open(
        mission_eighteen_root().join("level18"),
        crate::campaign::spec::CampaignSelection::ordinary(18).unwrap(),
    )
    .expect("open owner-data Mission 18 campaign")
    .simulation()
    .clone();
    let particles = session.session.definition.presentation.particles.clone();
    let old_projectile_slots = session
        .economy()
        .fireball_projectiles()
        .iter()
        .map(|projectile| projectile.source_entity_index)
        .collect::<std::collections::BTreeSet<_>>();

    let (wyvern_slot, target_slot, owner, generation) = {
        let live = &mut session.session.state.live;
        let position =
            feature.world_position(live.terrain.ground_height(feature.world_position(0)));
        let slot = mc2::allocate_authored(
            &mut live.economy,
            &mut live.core.runtime,
            &mut live.terrain,
            &particles,
            position,
            RULES.max_life,
            None,
            feature.index,
        )
        .unwrap()
        .expect("Mission 18 ordinary producer allocates its Wyvern");
        let target = live
            .economy
            .entities
            .iter()
            .find(|row| row.active && row.class == 3 && row.model == 0 && row.life >= 0)
            .expect("Mission 18 has a live class3/model0 target");
        let target_slot = target.index;
        let owner = {
            let row = &mut live.economy.entities[usize::from(slot)];
            assert_eq!((row.class, row.model, row.action), (5, SOURCE_MODEL, 129));
            assert_eq!(row.parameter_relation, Some(DESCRIPTOR));
            row.set_source_action(130);
            row.set_phase(8);
            row.set_source_dword_10(1);
            row.set_control_flags(row.control_flags | 0x800);
            row.record[154..156].copy_from_slice(&target_slot.to_le_bytes());
            row.source_id
        };
        (slot, target_slot, owner, live.economy.slot_generation(slot))
    };

    session.step_level2_source_pass(0);
    let fresh = session
        .economy()
        .fireball_projectiles()
        .iter()
        .filter(|projectile| {
            projectile.origin == crate::native_world::FireballProjectileOrigin::Wyvern
                && projectile.owner_entity_index == owner
                && !old_projectile_slots.contains(&projectile.source_entity_index)
        })
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(fresh.len(), 1, "one action130 burst creates one Fireball");
    let projectile = fresh[0];
    assert_eq!((projectile.source_class, projectile.source_model), (9, 0));
    assert_eq!((projectile.target_class, projectile.target_model), (3, 0));
    assert_eq!(projectile.owner_entity_index, owner);
    if projectile.source_entity_index == wyvern_slot {
        assert!(
            projectile.target_source_entity_index == 0
                || projectile.target_source_entity_index == target_slot,
            "self-replacement starts unbound; same-pass acquisition may bind the selected target"
        );
    } else {
        assert_eq!(projectile.target_source_entity_index, target_slot);
    }
    if projectile.source_entity_index == wyvern_slot {
        assert_ne!(
            session.source_arena().slot_generation(wyvern_slot),
            generation,
            "NewEvent replaced the current Wyvern slot"
        );
        assert_eq!(projectile.owner_entity_index, wyvern_slot);
    } else {
        let actor = &session.source_arena().entities[usize::from(wyvern_slot)];
        assert_eq!(actor.phase, 9, "same-pass source scheduler advances Wyvern");
    }

    let projectile_slot = projectile.source_entity_index;
    if projectile.active && projectile_slot >= wyvern_slot {
        assert_eq!(
            projectile.phase,
            (projectile_slot as u8).wrapping_add(1),
            "ascending source cursor dispatches a live newborn Fireball in the same pass"
        );
        assert!(session.dispatch_events().iter().any(|event| matches!(
            event,
            crate::native_world::Mc2DispatchEvent::DynamicArena {
                pass: 0,
                source_slot,
                kind: crate::native_world::Level2DynamicArenaKind::FireballProjectile,
            } if *source_slot == projectile_slot
        )));
    }
    let projectile_generation = session.source_arena().slot_generation(projectile_slot);
    if projectile.active {
        let carrier = session
            .session
            .state
            .live
            .economy
            .economy
            .fireball_projectiles
            .iter_mut()
            .find(|carrier| carrier.source_entity_index == projectile.source_entity_index)
            .expect("scheduled Fireball retains its typed carrier");
        carrier.life = 0;
    }

    session.step_level2_source_pass(1);
    let terminal = session
        .economy()
        .fireball_projectiles()
        .iter()
        .find(|carrier| carrier.source_entity_index == projectile.source_entity_index)
        .copied();
    assert!(
        terminal.is_none_or(|carrier| !carrier.active),
        "next source pass dispatches and terminates the short-lived Fireball"
    );

    session.step_level2_source_pass(2);
    let live = &session.session.state.live.economy.economy;
    assert!(
        !live
            .fireball_projectiles()
            .iter()
            .any(|carrier| carrier.source_entity_index == projectile_slot),
        "following source boundary removes the hidden Fireball carrier"
    );
    assert!(
        live.free_source_slots().contains(&projectile_slot)
            || session.source_arena().slot_generation(projectile_slot) != projectile_generation,
        "the reclaimed slot is free or has already been reused with a new generation"
    );
}

#[test]
fn army_ring_allocates_a_real_summoned_wyvern_with_holder_provenance() {
    let (mut arena, mut runtime, mut terrain, particles, owner_slot, feature) =
        mission_eighteen_source_parts();
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let holder_slot = arena
        .allocate_source_slot()
        .expect("reserve the Army holder's source slot");
    let initial_counter = runtime.bytes[crate::native_world::RUNTIME_BYTES + 16];

    let handle = mc2::allocate_summoned(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        owner_slot,
        owner_slot,
        holder_slot,
    )
    .unwrap()
    .expect("ordinary Army ring has source capacity");

    let row = &arena.entities[usize::from(handle.source_slot)];
    assert_eq!((row.class, row.model, row.action), (5, SOURCE_MODEL, 135));
    assert_eq!(row.parameter_relation, Some(DESCRIPTOR));
    assert_eq!(row.source_id, owner_slot);
    assert_eq!(
        u16::from_le_bytes([row.record[44], row.record[45]]),
        owner_slot
    );
    assert_eq!(row.record[77], 13);
    assert_eq!(u16::from_le_bytes([row.record[50], row.record[51]]), 250);
    assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 0);
    assert_eq!((row.mana, row.max_mana), (0, 0));
    assert_eq!(row.position, position);
    assert_eq!(
        runtime.bytes[crate::native_world::RUNTIME_BYTES + 16],
        initial_counter.wrapping_add(1)
    );
    let identity = arena
        .imported_creature_runtime_identity(handle.source_slot, &std::collections::BTreeMap::new())
        .expect("summoned Wyvern has live raw-creature identity");
    assert_eq!(identity.handle, handle);
    assert_eq!(
        identity.origin,
        crate::campaign::source_arena::CreatureRootOrigin::SummonedSpell { holder_slot }
    );
}

#[test]
fn summoned_wyvern_follows_reused_parent_and_expires_when_it_dies() {
    let (mut arena, mut runtime, mut terrain, particles, parent_slot, feature) =
        mission_eighteen_source_parts();
    let parent = arena.handle(parent_slot).unwrap();
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let child = mc2::allocate_summoned(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        parent_slot,
        parent_slot,
        parent_slot,
    )
    .unwrap()
    .unwrap();
    assert_eq!(arena.creature_parent(child), Some(parent));
    let mut families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    families[16].push(child.source_slot);

    let mut living_arena = arena.clone();
    let mut living_runtime = runtime.clone();
    let mut living_terrain = terrain.clone();
    let living = mc2::step_row(
        &mut living_arena,
        &mut living_runtime,
        &mut living_terrain,
        &particles,
        child.source_slot,
        &[],
        &families,
    )
    .unwrap();
    assert!(!living.expire);
    let living_row = &living_arena.entities[usize::from(child.source_slot)];
    assert!(!living_row.drawing_disabled());
    assert!(u16::from_le_bytes(living_row.record[50..52].try_into().unwrap()) < 250);

    let mut freed = arena.clone();
    let mut freed_runtime = runtime.clone();
    let mut freed_terrain = terrain.clone();
    freed.entities[usize::from(parent_slot)].set_life(0);
    freed.entities[usize::from(parent_slot)].reclaim_source_row();
    freed.economy.release_slot(parent_slot, None);
    let result = mc2::step_row(
        &mut freed, &mut freed_runtime, &mut freed_terrain, &particles,
        child.source_slot, &[], &families,
    ).unwrap();
    assert!(!result.expire, "a reclaimed fixed parent record with zero life remains readable");

    let replacement = arena.entities[usize::from(parent_slot)].clone();
    arena.entities[usize::from(parent_slot)].reclaim_source_row();
    arena.economy.release_slot(parent_slot, None);
    assert_eq!(arena.allocate_source_slot(), Some(parent_slot));
    arena.entities[usize::from(parent_slot)] = replacement;
    assert_ne!(arena.handle(parent_slot).unwrap(), parent);
    assert_eq!(arena.creature_parent(child), None);
    assert_eq!(arena.creature_parent_binding(child), Some(parent));
    let followed = mc2::step_row(
        &mut arena, &mut runtime, &mut terrain, &particles, child.source_slot, &[], &families,
    ).unwrap();
    assert!(!followed.expire, "a current living replacement is the source parent");
    assert!(!arena.entities[usize::from(child.source_slot)].drawing_disabled());
    arena.entities[usize::from(parent_slot)].set_life(0);
    let zero = mc2::step_row(
        &mut arena, &mut runtime, &mut terrain, &particles, child.source_slot, &[], &families,
    ).unwrap();
    assert!(!zero.expire, "the source parent predicate accepts zero life");
    arena.entities[usize::from(parent_slot)].set_life(-1);
    let expired = mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        child.source_slot,
        &[],
        &families,
    )
    .unwrap();
    assert!(expired.expire && expired.summon_expire);
    assert!(arena.entities[usize::from(child.source_slot)].drawing_disabled());
    assert!(
        arena.resolve(child).is_some(),
        "expiry retains this generation until the next boundary"
    );
    arena
        .rebuild_source_arena_with_terrain(&mut terrain, &[])
        .unwrap();
    assert!(arena.resolve(child).is_none());
}

#[test]
fn army_ring_reports_none_when_the_shared_source_allocator_is_exhausted() {
    let (mut arena, mut runtime, mut terrain, particles, owner_slot, feature) =
        mission_eighteen_source_parts();
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    while arena.allocate_source_slot().is_some() {}
    let counter = runtime.bytes[crate::native_world::RUNTIME_BYTES + 16];

    let result = mc2::allocate_summoned(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        owner_slot,
        owner_slot,
        owner_slot,
    )
    .unwrap();

    assert_eq!(result, None);
    assert_eq!(
        runtime.bytes[crate::native_world::RUNTIME_BYTES + 16],
        counter
    );
}

#[test]
fn wyvern_stage_wander_uses_one_draw_every_sixteen_visits_without_a_route() {
    struct Routes(Option<StageDescriptor>);
    impl StageRuntime for Routes {
        fn descriptor(&self, _: u8) -> Option<StageDescriptor> {
            self.0
        }
        fn bind(&mut self, _: u8) -> Option<StageDescriptor> {
            self.0
        }
        fn rearm_model_relations(&mut self) {}
    }
    let (_, _, terrain, particles, _, feature) = mission_eighteen_source_parts();
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let base = mc2::construct_row(7, 1, 0x1234, position, &mut 0, &particles).unwrap();
    let route = StageDescriptor {
        kind: 5,
        flags: 2,
        next: 0,
        point: position,
        actor: 0,
        model: 15,
        scalar: 0,
    };
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    for mode in [2, 5, 6, 7, 8, 9] {
        for (phase, blocked, draws) in
            [(8, false, 0), (16, false, 1), (64, false, 1), (16, true, 0)]
        {
            let mut row = base.clone();
            row.set_source_action(135);
            row.record[77] = mode;
            row.set_phase(phase);
            row.set_source_roll(2040);
            // Isolate the post-motion source RNG tail, including the blocked
            // negative; do not force any target, burst, or actor RNG result.
            row.set_control_flags(row.control_flags | 0x800 | if blocked { 0x40000 } else { 0 });
            let mut routes = Routes((mode == 2 || mode == 5).then_some(route));
            let mut context = Context {
                terrain: &terrain,
                targets: &[],
                class3: &[],
                family: &[],
                families: &families,
                runtime: &mut routes,
            };
            step(&mut Actor { row: &mut row }, &mut context).unwrap();
            let expected_rng = if draws == 1 {
                level2_next_rng16(base.entity_rng)
            } else {
                base.entity_rng
            };
            let expected_roll = if draws == 1 {
                (2040 + expected_rng % 113 + 142) & 2047
            } else {
                2040
            };
            assert_eq!(
                (row.entity_rng, row.source_roll()),
                (expected_rng, expected_roll as i16),
                "mode{mode}/phase{phase}/blocked{blocked}: source 1E1C0 or missing-target 1D8C0 tail"
            );
            assert_eq!(row.action, 135);
        }
    }
    // A real point route keeps its distinct two-draw phase64 perturbation.
    let mut row = base.clone();
    row.set_source_action(135);
    row.record[77] = 1;
    row.set_phase(64);
    row.set_control_flags(row.control_flags | 0x800);
    let point = Position {
        x: position.x.wrapping_add(4096),
        ..position
    };
    let mut routes = Routes(Some(StageDescriptor {
        kind: 1,
        flags: 0,
        point,
        ..route
    }));
    let mut context = Context {
        terrain: &terrain,
        targets: &[],
        class3: &[],
        family: &[],
        families: &families,
        runtime: &mut routes,
    };
    step(&mut Actor { row: &mut row }, &mut context).unwrap();
    let first = level2_next_rng16(base.entity_rng);
    let second = level2_next_rng16(first);
    let delta = (i32::from(second & 255) + 85) * (2 * (i32::from(first % 157) / 79) - 1);
    assert_eq!(row.entity_rng, second);
    assert_eq!(
        row.source_roll(),
        (heading_for_positions(position, point) as i16).wrapping_add(delta as i16) & 2047
    );
}

#[test]
fn wyvern_stage_selects_nearest_frozen_target_or_the_same_route_relation() {
    let terrain = terrain();
    let position = Position {
        x: 65_000,
        y: 20_000,
        z: 400,
    };
    let far = target(
        2,
        5,
        15,
        Position {
            x: 60_000,
            ..position
        },
    );
    let mut near = target(30, 5, 15, Position { x: 100, ..position });
    // Membership was frozen before callbacks. A preceding callback's death
    // does not remove the address from sub_1E3E0's model-list search.
    near.life = -1;
    near.control_flags = ENTITY_DRAW_DISABLED_BIT;
    near.model = 24; // The frozen model15 address now holds another model.
    let tied = target(31, 5, 15, near.position);
    let mut unrelated_peer = target(4, 5, 16, position);
    unrelated_peer.stage = StageState {
        index: 8,
        relation: near.slot,
        ..StageState::default()
    };
    let mut peer = target(5, 5, 16, position);
    peer.stage = StageState {
        index: 7,
        relation: far.slot,
        ..StageState::default()
    };
    let targets = [far, near, tied, unrelated_peer, peer];
    let mut families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    families[15] = vec![2, 30, 31];
    families[16] = vec![4, 5];
    struct Routes(StageDescriptor);
    impl StageRuntime for Routes {
        fn descriptor(&self, _: u8) -> Option<StageDescriptor> {
            Some(self.0)
        }
        fn bind(&mut self, _: u8) -> Option<StageDescriptor> {
            Some(self.0)
        }
        fn rearm_model_relations(&mut self) {}
    }
    fn visit(
        row: &mut EntitySnapshot,
        terrain: &SessionTerrain,
        targets: &[Target],
        families: &[Vec<u16>; 29],
        route: StageDescriptor,
    ) -> u16 {
        let mut routes = Routes(route);
        let mut context = Context {
            terrain,
            targets,
            class3: &[],
            family: &families[16],
            families,
            runtime: &mut routes,
        };
        row.record[77] = 5;
        row.set_phase(8);
        row.set_control_flags(row.control_flags | 0x800);
        // Exercise the enabled actor callback, keeping selection private.
        step(&mut Actor { row }, &mut context).unwrap();
        word(row, 78)
    }
    let route = StageDescriptor {
        kind: 5,
        flags: 2,
        next: 0,
        point: position,
        actor: 0,
        model: 15,
        scalar: 0,
    };
    let mut row = wyvern(135, position);
    row.record[76] = 7;
    assert_eq!(
        visit(&mut row, &terrain, &targets, &families, route),
        30,
        "nearest wrapped XY distance wins, with first-in-list tie order"
    );
    assert_eq!(word(&row, 78), 30);
    assert_eq!(
        visit(
            &mut row,
            &terrain,
            &targets,
            &families,
            StageDescriptor { flags: 3, ..route }
        ),
        30,
        "a current relation is retained before looking at another actor"
    );
    put(&mut row, 78, 0);
    assert_eq!(
        visit(
            &mut row,
            &terrain,
            &targets,
            &families,
            StageDescriptor { flags: 3, ..route }
        ),
        2,
        "bit1 shares the first nonzero relation with the same route index"
    );
    visit(
        &mut row,
        &terrain,
        &targets,
        &families,
        StageDescriptor {
            flags: 0,
            actor: 31,
            ..route
        },
    );
    assert_eq!(
        row.source_roll(),
        heading_for_positions(position, near.position) as i16,
        "an explicit actor route bypasses the cached model relation"
    );
}

#[test]
fn owner_mission18_switch_wyvern_acquires_and_damages_its_real_model26_target() {
    use crate::PlayerCommand;
    use crate::campaign::source_arena::{CreatureRootOrigin, SourceEntityView};
    use crate::native_world::Mc2DispatchEvent;

    let mut session = crate::campaign::CampaignSession::open(
        mission_eighteen_root().join("level18"),
        crate::campaign::spec::CampaignSelection::ordinary(18).unwrap(),
    )
    .unwrap()
    .simulation()
    .clone();
    let gate = session.entities()[86].clone();
    assert_eq!(
        (gate.class, gate.model, gate.action, gate.source_id),
        (11, 32, 32, 2)
    );
    let task = usize::try_from(gate.level_gate).unwrap();
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !session.dispatch_events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated { group: 2, .. }
        )),
        "the real objective switch must not fire before its prerequisite"
    );
    // Stage only this package's completed-task input. The real switch, group
    // scan, typed authored constructor and route binding produce record172.
    // This is a focused production/combat check, not an earned mission run.
    for owner in 0..session.runtime.objectives.len() {
        session
            .runtime
            .set_level2_objective_stage_selector_for(owner, task, 2);
    }
    for _ in 0..8 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        if session.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated {source_slot: 86, group: 2, authored_indices, ..}
                if authored_indices.contains(&172)
        )) {break;}
    }
    let handle = session
        .entities()
        .iter()
        .find_map(|row| {
            let identity = session.creature_identity_for_source_slot(row.index)?;
            (identity.kind == crate::level_definition::CreatureKind::Wyvern
                && identity.origin
                    == CreatureRootOrigin::AuthoredRecord {
                        authored_record: 172,
                    })
            .then_some(identity.handle)
        })
        .expect("ordinary owner switch constructs its typed record172 Wyvern");
    let SourceEntityView::Imported(actor) = session.source_arena().resolve(handle).unwrap() else {
        panic!("Wyvern is an authoritative raw source actor");
    };
    let actor = actor.clone();
    assert_eq!((actor.record[76], actor.record[77]), (2, 3));
    assert_eq!(actor.parameter_relation, Some(DESCRIPTOR));
    let targets = session
        .entities()
        .iter()
        .filter(|row| row.active && row.class == 5 && row.model == 26 && row.life >= 0)
        .map(|row| row.index)
        .collect::<Vec<_>>();
    let target_slot = *targets
        .first()
        .expect("owner18 supplies the actual route's model26 targets");
    let target_handle = session.source_arena().handle(target_slot).unwrap();
    let life_before = session.entities()[usize::from(target_slot)].life;
    let nearby = Position {
        x: actor.position.x.wrapping_add(1024),
        ..actor.position
    };
    // Position the existing target using the canonical position operation.
    // No actor action, target, phase, burst, protection or RNG is manufactured.
    session
        .set_level2_entity_position(target_slot, nearby)
        .unwrap();
    let mut distant = session.clone();
    for &slot in &targets {
        let far = Position {
            x: actor.position.x.wrapping_add(0x6000),
            ..actor.position
        };
        distant.set_level2_entity_position(slot, far).unwrap();
    }
    let mut acquired = false;
    let mut moved = false;
    let mut shot_handles = Vec::new();
    let mut consumed = None;
    for turn in 0..120 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "owner18 Wyvern scheduler error: {:?}",
            session.dispatch_events()
        );
        let SourceEntityView::Imported(actor_now) = session
            .source_arena()
            .resolve(handle)
            .expect("ordinary Wyvern remains its original generation")
        else {
            panic!("Wyvern must keep its raw actor carrier");
        };
        acquired |= word(actor_now, 154) == target_slot && actor_now.action == 130;
        moved |= actor_now.position != actor.position;
        for shot in session
            .economy()
            .fireball_projectiles()
            .iter()
            .filter(|shot| {
                shot.origin == crate::native_world::FireballProjectileOrigin::Wyvern
                    && shot.owner_entity_index == actor.source_id
                    && shot.target_source_entity_index == target_slot
            })
        {
            if let Some(handle) = session.source_arena().handle(shot.source_entity_index)
                && !shot_handles.contains(&handle)
            {
                shot_handles.push(handle);
            }
        }
        if let Some(SourceEntityView::Imported(target)) =
            session.source_arena().resolve(target_handle)
            && target.life < life_before
            && word(target, 42) == actor.source_id
        {
            consumed = Some((turn, target.life));
        }
        if turn < 8 {
            distant.step_outer_turn(PlayerCommand::default(), 0);
            let SourceEntityView::Imported(far_actor) =
                distant.source_arena().resolve(handle).unwrap()
            else {
                panic!("negative route must keep its raw Wyvern");
            };
            assert_ne!(
                far_actor.action, 130,
                "same route stays out of attack range with only distant model26 actors"
            );
        }
        if acquired && moved && !shot_handles.is_empty() && consumed.is_some() {
            break;
        }
    }
    assert!(
        acquired && moved && !shot_handles.is_empty() && consumed.is_some(),
        "real route must acquire, fly, fire and damage original model26: acquired={acquired} moved={moved} shots={shot_handles:?} consumed={consumed:?} actor={:?}",
        session.source_arena().resolve(handle)
    );
    eprintln!(
        "ordinary Wyvern{} -> model26{}: consumed={consumed:?}, projectiles={shot_handles:?}",
        handle.source_slot, target_slot
    );
    for _ in 0..32 {
        if shot_handles
            .iter()
            .any(|&shot| session.source_arena().resolve(shot).is_none())
        {
            return;
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    panic!("naturally produced Wyvern projectiles must reach normal next-boundary reclamation");
}
