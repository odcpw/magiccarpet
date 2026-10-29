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
fn death_transfers_mana_and_explosion_identity_unless_explosion_is_suppressed() {
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
    let actor = add_creature(arena, terrain, &particles, position, 149);
    arena.entities[usize::from(actor)].set_phase(0);
    arena.entities[usize::from(actor)].source_id = 321;
    put(&mut arena.entities[usize::from(actor)], 30, 321);
    let classes = class3_slots(arena);
    let families = model_slots(arena);
    let old_spheres = arena.economy.spheres.len();
    mc2::step_row(
        arena, runtime, terrain, &particles, actor, &classes, &families,
    )
    .expect("death callback creates source descendants");
    assert!(arena.entities[usize::from(actor)].drawing_disabled());
    assert!(
        arena.economy.map_chains.is_linked(actor),
        "hide retains the map link until following-pass reclaim"
    );
    assert_eq!(arena.economy.spheres[old_spheres].mana, RULES.initial_mana);
    let explosion = arena
        .entities
        .iter()
        .find(|row| row.active && row.class == 10 && row.model == 1 && row.source_id == 321)
        .expect("death explosion inherits source identity");
    assert_eq!(put_word(explosion, 30), 321);

    let quiet = add_creature(arena, terrain, &particles, position, 149);
    let row = &mut arena.entities[usize::from(quiet)];
    row.set_phase(0);
    row.source_id = 322;
    put(row, 30, 322);
    row.set_control_flags(row.control_flags | 0x100000);
    let classes = class3_slots(arena);
    let families = model_slots(arena);
    let old_spheres = arena.economy.spheres.len();
    mc2::step_row(
        arena, runtime, terrain, &particles, quiet, &classes, &families,
    )
    .expect("suppressed explosion still transfers mana");
    assert_eq!(arena.economy.spheres[old_spheres].mana, RULES.initial_mana);
    assert!(
        !arena
            .entities
            .iter()
            .any(|row| { row.active && row.class == 10 && row.model == 1 && row.source_id == 322 })
    );
}

#[test]
fn summon_parent_loss_produces_source_expiry_children_and_hides_next_pass() {
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
    let parent = arena
        .entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.life >= 0)
        .expect("fixture retains a live parent")
        .index;
    for mode in [13, 16] {
        let following = add_creature(arena, terrain, &particles, position, 151);
        let row = &mut arena.entities[usize::from(following)];
        row.record[77] = mode;
        row.set_phase(1);
        put(row, 44, parent);
        put(row, 50, 10);
        let classes = class3_slots(arena);
        let families = model_slots(arena);
        let effects = mc2::step_row(
            arena, runtime, terrain, &particles, following, &classes, &families,
        )
        .expect("live parent follow callback");
        assert!(!effects.expire);
        assert_eq!(
            put_word(&arena.entities[usize::from(following)], 50),
            if mode == 13 { 5 } else { 6 }
        );
        assert!(!arena.entities[usize::from(following)].drawing_disabled());

        let actor = add_creature(arena, terrain, &particles, position, 151);
        let row = &mut arena.entities[usize::from(actor)];
        row.record[77] = mode;
        put(row, 44, 0);
        put(row, 50, 10);
        let before = arena
            .entities
            .iter()
            .filter(|row| row.active)
            .map(|row| row.index)
            .collect::<std::collections::BTreeSet<_>>();
        let classes = class3_slots(arena);
        let families = model_slots(arena);
        let effects = mc2::step_row(
            arena, runtime, terrain, &particles, actor, &classes, &families,
        )
        .expect("missing parent expiry callback");
        assert!(effects.expire);
        assert_eq!(effects.summon_expire, mode == 13);
        assert!(arena.entities[usize::from(actor)].drawing_disabled());
        assert!(
            arena.economy.map_chains.is_linked(actor),
            "expiry keeps its map link for the rest of the source pass"
        );
        assert!(
            arena.entities[usize::from(actor)].active,
            "reclaim belongs to the next pass"
        );
        let born = arena
            .entities
            .iter()
            .filter(|row| row.active && !before.contains(&row.index))
            .map(|row| (row.class, row.model))
            .collect::<Vec<_>>();
        assert!(born.contains(&(10, 0)));
        assert_eq!(born.contains(&(10, 73)), mode == 13);
    }
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

#[test]
fn tracking_without_target_does_not_restart_patrol_or_consume_rng() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let context = Creature18Context {
        terrain: &simulation.session.state.live.terrain,
        runtime: &simulation.session.state.live.core.runtime,
        targets: &[],
        class3: &[],
        model18: &[],
    };
    let mut counter = 0;
    let mut actor = construct(7, 1, 0x1234, Position::default(), &mut counter);
    actor.action = 144;
    actor.attack_phase = 1;
    actor.target = 0;
    let rng = actor.rng;
    let timer = actor.timer;
    step(&mut actor, &context).expect("tracking callback with empty target");
    assert_eq!(
        (actor.action, actor.attack_phase, actor.timer, actor.rng),
        (144, 1, timer, rng)
    );
}

#[test]
fn owner_switch_publishes_model18_and_normal_scheduler_visits_it() {
    use crate::PlayerCommand;
    use crate::campaign::source_arena::CreatureRootOrigin;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::native_world::Mc2DispatchEvent;

    let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-paj2-source-secret-34-campaign-package");
    let campaign = CampaignSession::open(package, CampaignSelection::SECRET_DESTINATION_34)
        .expect("owner package for the source group23 switch");
    let mut session = campaign.simulation().clone();
    let switch = session.entities()[96].clone();
    assert_eq!(
        (switch.class, switch.model, switch.action, switch.source_id),
        (11, 1, 1, 23)
    );
    let player = session.state.entity_index;

    // CheckpointArrived_6F070 fires when no player overlaps the switch.
    // Hold the negative case inside for a whole eight-phase scan cycle.
    let mut inside = session.clone();
    inside
        .set_level2_entity_position(player, switch.position)
        .unwrap();
    for _ in 0..8 {
        inside.step_outer_turn(PlayerCommand::default(), 0);
        assert!(!inside.dispatch_events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated {
                source_slot: 96,
                group: 23,
                ..
            }
        )));
    }

    let mut distant = switch.position;
    distant.x = distant.x.wrapping_add(0x6000);
    session.set_level2_entity_position(player, distant).unwrap();
    let mut published = false;
    for _ in 0..8 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        published |= session.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 96, group: 23, authored_indices, .. }
                if [566, 567, 568].iter().all(|id| authored_indices.contains(id))
        ));
        if published {
            break;
        }
    }
    assert!(
        published,
        "owner switch must produce its authored model18 group: {:?}",
        session.dispatch_events()
    );
    let slots = session
        .entities()
        .iter()
        .filter_map(|row| {
            let identity = session.creature_identity_for_source_slot(row.index)?;
            matches!(
                identity.origin,
                CreatureRootOrigin::AuthoredRecord {
                    authored_record: 566..=568
                }
            )
            .then_some((row.index, row.phase))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        slots.len(),
        3,
        "all three owner-authored model18 roots must be live"
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    for (slot, phase) in slots {
        let actor = &session.entities()[usize::from(slot)];
        assert!(actor.active && !actor.drawing_disabled());
        assert_eq!(
            actor.phase,
            phase.wrapping_add(1),
            "normal source scheduler visits model18 slot {slot}"
        );
    }
}

#[test]
fn attack_turns_toward_non_player_attacker_with_source_phase_caps() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = &simulation.session.state.live.terrain;
    let runtime = &simulation.session.state.live.core.runtime;
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 0,
    };
    let target = Target {
        slot: 42,
        class: 5,
        model: 15,
        life: 100,
        position: Position {
            x: 21_000,
            y: 20_000,
            z: 0,
        },
        ..Target::default()
    };
    let targets = [target];
    let context = Creature18Context {
        terrain,
        runtime,
        targets: &targets,
        class3: &[],
        model18: &[],
    };
    let mut counter = 0;
    let mut actor = construct(7, 1, 0x1234, position, &mut counter);
    let heading = heading_for_positions(position, target.position) as i16;
    actor.action = 146;
    actor.target = target.slot;
    actor.yaw = heading.wrapping_add(100) & 2047;
    actor.timer = 100;
    actor.attack_phase = 0;
    step(&mut actor, &context).expect("attack tracking callback");
    assert_eq!(actor.yaw, heading.wrapping_add(78) & 2047);
    assert_eq!(actor.target, target.slot);

    actor.attack_phase = 1;
    actor.phase = 0;
    actor.timer = 10;
    let effects = step(&mut actor, &context).expect("attack firing callback");
    assert_eq!(actor.yaw, heading);
    assert_eq!(effects.projectiles.len(), 5);
    assert!(
        effects
            .projectiles
            .iter()
            .all(|shot| shot.target_class == 5 && shot.source_target == target.slot)
    );

    let mut hidden = target;
    hidden.flags = ENTITY_DRAW_DISABLED_BIT;
    let hidden_targets = [hidden];
    let hidden_context = Creature18Context {
        targets: &hidden_targets,
        ..context
    };
    actor.attack_phase = 1;
    actor.timer = 10;
    let effects = step(&mut actor, &hidden_context).expect("hidden target callback");
    assert!(effects.projectiles.is_empty());
}
