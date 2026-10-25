use super::*;

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
    use crate::native_world::{
        Level2EconomyState, parse_entities, parse_presentation, parse_runtime, parse_terrain,
    };
    let root = std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261024-b")
        },
        std::path::PathBuf::from,
    );
    let package = crate::campaign::spec::DecodedCampaignPackage::decode(
        root.join("level21"),
        crate::campaign::spec::CampaignSelection::ordinary(21).unwrap(),
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
