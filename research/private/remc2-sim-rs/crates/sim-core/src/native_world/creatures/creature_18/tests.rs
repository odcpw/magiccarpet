use super::*;
use crate::campaign::source_arena::SourceArena;

fn put(row: &mut crate::native_world::EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_word(row: &crate::native_world::EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}

fn add_creature(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[crate::native_world::ParticleParameter],
    position: Position,
    action: u8,
) -> u16 {
    let slot = arena
        .allocate_source_slot_with_terrain(terrain)
        .expect("fixture has a source slot");
    let seed = arena.new_event_rng(slot);
    let mut counter = 0;
    let mut row = mc2::construct_row(
        slot,
        arena.slot_generation(slot),
        seed,
        position,
        &mut counter,
        particles,
    )
    .expect("model18 constructor assets are retained");
    row.source_id = slot;
    put(&mut row, 30, slot);
    row.set_source_action(action);
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)
        .expect("publish source row position");
    slot
}

fn class3_slots(arena: &SourceArena) -> Vec<u16> {
    arena
        .entities
        .iter()
        .filter(|row| row.active && row.class == 3 && row.life >= 0)
        .map(|row| row.index)
        .collect()
}

fn model_slots(arena: &SourceArena) -> [Vec<u16>; 29] {
    std::array::from_fn(|model| {
        arena
            .entities
            .iter()
            .filter(|row| row.active && row.class == 5 && usize::from(row.model) == model)
            .map(|row| row.index)
            .collect()
    })
}

#[test]
fn constructor_binds_source18_properties_and_first_callback_enters_patrol() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let particles = &simulation.session.definition.presentation.particles;
    let mut counter = 17;
    let row = mc2::construct_row(
        41,
        9,
        0x1234,
        Position {
            x: 20_000,
            y: 20_000,
            z: 256,
        },
        &mut counter,
        particles,
    )
    .expect("model18 binds retained source assets");

    assert_eq!((row.class, row.model, row.action), (5, SOURCE_MODEL, 147));
    assert_eq!(
        (row.max_life, row.life, row.mana),
        (RULES.max_life, RULES.max_life, RULES.initial_mana)
    );
    assert_eq!(row.parameter_relation, Some(DESCRIPTOR));
    assert_eq!(row.particle_parameter_index, Some(286));
    assert_eq!(counter, 18);

    let terrain = &simulation.session.state.live.terrain;
    let runtime = &simulation.session.state.live.core.runtime;
    let targets = [];
    let class3 = [];
    let model18 = [];
    let context = Creature18Context {
        terrain,
        runtime,
        targets: &targets,
        class3: &class3,
        model18: &model18,
    };
    let mut actor = mc2::load(&row, 9);

    step(&mut actor, &context).expect("source constructor action147 callback runs");

    assert_eq!(actor.action, 144);
    assert_eq!(actor.attack_phase, 0);
    assert!((400..=799).contains(&actor.timer));
}

#[test]
fn attack_materializes_five_sourceordered_typed_fireballs_and_lost_target_emits_none() {
    let mut simulation = crate::native_world::tests::level_two_campaign_session();
    let particles = simulation
        .session
        .definition
        .presentation
        .particles
        .to_vec();
    let live = &mut simulation.session.state.live;
    let target_slot = live
        .economy
        .entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.model == 0 && row.life >= 0)
        .expect("fixture retains a live player")
        .index;
    let target_position = live.economy.entities[usize::from(target_slot)].position;
    let actor_position = Position {
        x: target_position.x.wrapping_sub(1_000),
        y: target_position.y,
        z: target_position.z,
    };
    let (arena, runtime, terrain) = (&mut live.economy, &mut live.core.runtime, &mut live.terrain);
    let actor_slot = add_creature(arena, terrain, &particles, actor_position, 146);
    {
        let actor = &mut arena.entities[usize::from(actor_slot)];
        actor.set_phase(RULES.source_scan_period);
        actor.set_source_dword_10(10);
        actor.record[74] = 1;
        put(actor, 154, target_slot);
    }
    let classes = class3_slots(arena);
    let families = model_slots(arena);
    let before = arena.economy.fireball_projectiles().len();
    let effects = mc2::step_row(
        arena, runtime, terrain, &particles, actor_slot, &classes, &families,
    )
    .expect("model18 attack callback succeeds");

    assert_eq!(effects.projectile_slots.len(), 5);
    assert!(
        effects.projectiles.is_empty(),
        "raw spawn intents stay in the leaf"
    );
    let fresh = &arena.economy.fireball_projectiles()[before..];
    assert_eq!(fresh.len(), 5);
    assert_eq!(
        fresh
            .iter()
            .map(|projectile| projectile.source_entity_index)
            .collect::<Vec<_>>(),
        effects.projectile_slots
    );
    let actor = &arena.entities[usize::from(actor_slot)];
    let target = &arena.entities[usize::from(target_slot)];
    let (base_yaw, base_pitch) =
        crate::level1_mission::projectile_guidance_angles(actor.position, target.position);
    for (projectile, offset) in fresh.iter().zip(RULES.fan_yaw_offsets) {
        assert_eq!(
            projectile.origin,
            crate::native_world::FireballProjectileOrigin::Creature18
        );
        assert_eq!((projectile.source_class, projectile.source_model), (9, 0));
        assert_eq!(projectile.owner_entity_index, actor.source_id);
        assert_eq!(projectile.target_source_entity_index, target_slot);
        assert_eq!((projectile.target_class, projectile.target_model), (3, 0));
        assert_eq!(
            projectile.source_subspell_index,
            RULES.projectile_damage as u16
        );
        assert_eq!(projectile.source_mana, 50);
        assert_eq!(projectile.yaw, base_yaw.wrapping_add(offset) & 2047);
        assert_eq!(projectile.pitch, base_pitch);
        assert_eq!(
            projectile.position.z,
            actor.position.z.wrapping_add(RULES.fan_lift)
        );
    }

    let second = add_creature(arena, terrain, &particles, actor.position, 146);
    {
        let actor = &mut arena.entities[usize::from(second)];
        actor.set_phase(RULES.source_scan_period);
        actor.set_source_dword_10(10);
        actor.record[74] = 1;
        put(actor, 154, 0);
    }
    let classes = class3_slots(arena);
    let families = model_slots(arena);
    let before_lost_target = arena.economy.fireball_projectiles().len();
    let outcome = mc2::step_row(
        arena, runtime, terrain, &particles, second, &classes, &families,
    )
    .expect("lost-target callback returns to behavior");
    assert!(outcome.projectile_slots.is_empty());
    assert_eq!(
        arena.economy.fireball_projectiles().len(),
        before_lost_target
    );
}

#[test]
fn prekill_inherits_owned_child_credit_without_running_death_early() {
    let mut simulation = crate::native_world::tests::level_two_campaign_session();
    let particles = simulation
        .session
        .definition
        .presentation
        .particles
        .to_vec();
    let live = &mut simulation.session.state.live;
    let player_slot = live
        .economy
        .entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.model == 0)
        .expect("fixture retains the source player")
        .index;
    let (arena, runtime, terrain) = (&mut live.economy, &mut live.core.runtime, &mut live.terrain);
    let root = add_creature(
        arena,
        terrain,
        &particles,
        Position {
            x: 20_000,
            y: 20_000,
            z: 0,
        },
        148,
    );
    let child = add_creature(
        arena,
        terrain,
        &particles,
        Position {
            x: 20_128,
            y: 20_000,
            z: 0,
        },
        144,
    );
    put(&mut arena.entities[usize::from(root)], 56, child);
    put(&mut arena.entities[usize::from(child)], 40, player_slot);
    let classes = class3_slots(arena);
    let families = model_slots(arena);

    let effects = mc2::step_row(
        arena, runtime, terrain, &particles, root, &classes, &families,
    )
    .expect("PreKill callback succeeds");

    assert_eq!(effects.kill_credit, Some(player_slot));
    assert_eq!(arena.entities[usize::from(root)].action, 149);
    assert_eq!(arena.entities[usize::from(child)].action, 149);
    assert_eq!(
        put_word(&arena.entities[usize::from(root)], 40),
        player_slot
    );
    assert!(arena.entities[usize::from(root)].active);
    assert!(
        !effects.kill,
        "PreKill does not run the phase-gated death callback"
    );
}

#[test]
fn action144_clears_target_when_three_dimensional_distance_exceeds_range() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = simulation.session.state.live.terrain.clone();
    let runtime = simulation.session.state.live.core.runtime.clone();
    let actor_position = Position {
        x: 20_000,
        y: 20_000,
        z: 1_000,
    };
    let target_position = Position {
        x: 21_000,
        y: 20_000,
        z: 20_000,
    };
    let target_slot = 42;
    let targets = [Target {
        slot: target_slot,
        class: 3,
        model: 0,
        life: 1,
        position: target_position,
        ..Target::default()
    }];
    let class3 = [target_slot];
    let model18 = [];
    let context = Creature18Context {
        terrain: &terrain,
        runtime: &runtime,
        targets: &targets,
        class3: &class3,
        model18: &model18,
    };
    let mut phase = 0;
    let mut actor = construct(7, 3, 0x1234, actor_position, &mut phase);
    actor.action = 144;
    actor.attack_phase = 1;
    actor.target = target_slot;

    step(&mut actor, &context).expect("action144 range callback runs");

    assert_eq!(actor.target, 0);
    assert_eq!(actor.action, 144);
    assert_eq!(actor.attack_phase, 0);
    assert!((400..=799).contains(&actor.timer));
}
