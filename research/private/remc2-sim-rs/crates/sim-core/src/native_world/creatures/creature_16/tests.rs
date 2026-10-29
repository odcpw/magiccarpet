use super::super::archer::ArcherStageDescriptor as StageDescriptor;
use super::*;

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
    let class5 = [];
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
        class5: &class5,
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
    let class5 = [];
    let mut actor = Actor { row: &mut row };
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        family: &families[16],
        class5: &class5,
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
    let class5 = [];
    let mut actor = Actor { row: &mut row };
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        family: &families[16],
        class5: &class5,
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
fn summoned_wyvern_expires_when_the_original_parent_slot_is_reused() {
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

    let replacement = arena.entities[usize::from(parent_slot)].clone();
    arena.entities[usize::from(parent_slot)].reclaim_source_row();
    arena.economy.release_slot(parent_slot, None);
    assert_eq!(arena.allocate_source_slot(), Some(parent_slot));
    arena.entities[usize::from(parent_slot)] = replacement;
    assert_ne!(arena.handle(parent_slot).unwrap(), parent);
    assert_eq!(arena.creature_parent(child), None);
    assert_eq!(arena.creature_parent_binding(child), Some(parent));
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
