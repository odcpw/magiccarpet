use super::*;
use crate::campaign::source_arena::SourceArena;
use crate::native_world::RuntimeRelation;

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
fn authored_point_route_waits_for_prepass_and_hostile_hit_starts_attack() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let mut runtime = simulation.session.state.live.core.runtime.clone();
    runtime.stage_variables[1] = crate::native_world::RuntimeStageVariable {
        kind: 1,
        flags: 0,
        word_2: 0,
        relation: RuntimeRelation::AxisScalar((20_000_u32 << 16) | 20_000),
    };
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 0,
    };
    let enemy = Target {
        slot: 12,
        source_id: 12,
        class: 3,
        model: 0,
        life: 100,
        position,
        ..Target::default()
    };
    let targets = [enemy];
    let context = Creature18Context {
        terrain: &simulation.session.state.live.terrain,
        runtime: &runtime,
        targets: &targets,
        class3: &[12],
        model18: &[],
        families: &[],
    };
    let mut counter = 0;
    let mut actor = construct(7, 1, 123, position, &mut counter);
    actor.action = 151;
    actor.route_index = 1;
    actor.route_mode = 1;
    actor.phase = 8;
    actor.control_flags |= 0x800;
    let mut hit = actor;
    hit.pending_damage = 50;
    hit.pending_attacker = enemy.slot;
    step(&mut actor, &context).unwrap();
    assert_eq!((actor.action, actor.route_mode, actor.target), (151, 1, 0));
    step(&mut hit, &context).unwrap();
    assert_eq!(hit.life, RULES.max_life - 50);
    assert_eq!(
        (hit.action, hit.route_mode, hit.target),
        (146, 10, enemy.slot)
    );
}

#[test]
fn summoned_target_scan_compares_source_squared_distances() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 0,
    };
    let parent = Target {
        slot: 9,
        source_id: 9,
        class: 3,
        model: 0,
        life: 100,
        position,
        ..Target::default()
    };
    let farther = Target {
        slot: 12,
        source_id: 12,
        class: 3,
        model: 0,
        life: 100,
        position: Position {
            x: 20_100,
            y: 20_009,
            z: 0,
        },
        ..Target::default()
    };
    let closer = Target {
        slot: 13,
        source_id: 13,
        position: Position {
            x: 20_100,
            y: 20_000,
            z: 0,
        },
        ..farther
    };
    assert_eq!(
        crate::legacy_sqrt(super::xy_squared(parent.position, farther.position)),
        crate::legacy_sqrt(super::xy_squared(parent.position, closer.position))
    );
    let targets = [parent, farther, closer];
    let context = Creature18Context {
        terrain: &simulation.session.state.live.terrain,
        runtime: &simulation.session.state.live.core.runtime,
        targets: &targets,
        class3: &[12, 13],
        model18: &[],
        families: &[],
    };
    let only_farther = Creature18Context {
        class3: &[12],
        ..context
    };
    let mut counter = 0;
    let mut actor = construct(7, 1, 123, position, &mut counter);
    actor.action = 151;
    actor.route_mode = 16;
    actor.route_remaining = 40;
    actor.parent = parent.slot;
    actor.phase = 8;
    actor.control_flags |= 0x800;
    let mut without_closer = actor;
    step(&mut actor, &context).unwrap();
    assert_eq!(actor.target, closer.slot);
    step(&mut without_closer, &only_farther).unwrap();
    assert_eq!(without_closer.target, farther.slot);
}

#[test]
fn ordinary_scan_uses_frozen_class3_membership_but_honors_current_exclusion_flag() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 0,
    };
    let enemy = Target {
        slot: 12,
        class: 3,
        model: 0,
        life: -1,
        flags: ENTITY_DRAW_DISABLED_BIT,
        position: Position {
            x: 21_000,
            ..position
        },
        ..Target::default()
    };
    let targets = [enemy];
    let excluded = [Target {
        flags: enemy.flags | 0x20,
        ..enemy
    }];
    let context = Creature18Context {
        terrain: &simulation.session.state.live.terrain,
        runtime: &simulation.session.state.live.core.runtime,
        targets: &targets,
        class3: &[12],
        model18: &[],
        families: &[],
    };
    let rejected = Creature18Context {
        targets: &excluded,
        ..context
    };
    let mut counter = 0;
    let mut actor = construct(7, 1, 1, position, &mut counter);
    // The constructor starts at action147; exercise action144's scan rather
    // than its initial reset callback.
    actor.action = 144;
    actor.yaw = heading_for_positions(position, enemy.position) as i16;
    actor.rng = 1; // The first source RNG step selects the acquisition scan.
    let mut negative = actor;
    step(&mut actor, &context).unwrap();
    assert_eq!(
        (actor.action, actor.attack_phase, actor.target),
        (144, 1, 12)
    );
    step(&mut negative, &rejected).unwrap();
    assert_eq!(
        (negative.action, negative.attack_phase, negative.target),
        (144, 0, 0)
    );
}

#[test]
fn ordinary_scan_checks_the_winning_frozen_slots_current_class_after_selection() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 0,
    };
    let closer = Target {
        slot: 12,
        class: 5,
        position: Position {
            x: 21_000,
            ..position
        },
        ..Target::default()
    };
    let farther = Target {
        slot: 13,
        class: 3,
        position: Position {
            x: 21_200,
            ..position
        },
        ..Target::default()
    };
    let targets = [closer, farther];
    let excluded = [
        Target {
            flags: 0x20,
            ..closer
        },
        farther,
    ];
    let context = Creature18Context {
        terrain: &simulation.session.state.live.terrain,
        runtime: &simulation.session.state.live.core.runtime,
        targets: &targets,
        class3: &[12, 13],
        model18: &[],
        families: &[],
    };
    let mut counter = 0;
    let mut actor = construct(7, 1, 1, position, &mut counter);
    actor.yaw = heading_for_positions(position, closer.position) as i16;
    assert!(nearest_player(&actor, &context).is_none());
    let excluded_context = Creature18Context {
        targets: &excluded,
        ..context
    };
    assert_eq!(nearest_player(&actor, &excluded_context).unwrap().slot, 13);
}

#[test]
fn authored_guard_uses_frozen_class3_membership_after_target_dies() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let mut runtime = simulation.session.state.live.core.runtime.clone();
    runtime.stage_variables[1] = crate::native_world::RuntimeStageVariable {
        kind: 2,
        flags: 0,
        word_2: 0,
        relation: RuntimeRelation::AxisScalar((20_000_u32 << 16) | 20_000),
    };
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 0,
    };
    let enemy = Target {
        slot: 12,
        source_id: 12,
        class: 3,
        model: 0,
        life: -1,
        position: Position {
            x: 21_000,
            ..position
        },
        ..Target::default()
    };
    let targets = [enemy];
    let context = Creature18Context {
        terrain: &simulation.session.state.live.terrain,
        runtime: &runtime,
        targets: &targets,
        class3: &[12],
        model18: &[],
        families: &[],
    };
    let missing = Creature18Context {
        class3: &[],
        ..context
    };
    let mut counter = 0;
    let mut actor = construct(7, 1, 123, position, &mut counter);
    actor.action = 151;
    actor.route_index = 1;
    actor.route_mode = 2;
    actor.phase = RULES.source_scan_period;
    actor.yaw = heading_for_positions(position, enemy.position) as i16;
    actor.control_flags |= 0x800;
    let mut absent = actor;
    step(&mut actor, &context).unwrap();
    assert_eq!(
        (actor.action, actor.route_mode, actor.target),
        (146, 10, enemy.slot)
    );
    step(&mut absent, &missing).unwrap();
    assert_eq!(
        (absent.action, absent.route_mode, absent.target),
        (151, 2, 0)
    );
}

#[test]
fn authored_model_route_selects_only_frozen_source_family() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let mut runtime = simulation.session.state.live.core.runtime.clone();
    runtime.stage_variables[1] = crate::native_world::RuntimeStageVariable {
        kind: 3,
        flags: 2,
        word_2: 0,
        relation: RuntimeRelation::AxisScalar(21),
    };
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 0,
    };
    let enemy = Target {
        slot: 12,
        source_id: 12,
        class: 5,
        model: 21,
        life: 100,
        position: Position {
            x: 21_000,
            ..position
        },
        ..Target::default()
    };
    let targets = [enemy];
    let mut families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    families[21].push(enemy.slot);
    let context = Creature18Context {
        terrain: &simulation.session.state.live.terrain,
        runtime: &runtime,
        targets: &targets,
        class3: &[],
        model18: &[],
        families: &families,
    };
    let missing = Creature18Context {
        families: &[],
        ..context
    };
    let mut counter = 0;
    let mut actor = construct(7, 1, 123, position, &mut counter);
    actor.action = 151;
    actor.route_index = 1;
    actor.route_mode = 3;
    actor.phase = 8;
    actor.control_flags |= 0x800;
    let mut absent = actor;
    step(&mut actor, &context).unwrap();
    assert_eq!(
        (actor.action, actor.route_mode, actor.target),
        (146, 10, enemy.slot)
    );
    step(&mut absent, &missing).unwrap();
    assert_eq!(
        (absent.action, absent.route_mode, absent.target),
        (151, 3, 0)
    );
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
        families: &[],
    };
    let mut actor = mc2::load(&row, 9);

    step(&mut actor, &context).expect("source constructor action147 callback runs");

    assert_eq!(actor.action, 144);
    assert_eq!(actor.attack_phase, 0);
    assert!((400..=799).contains(&actor.timer));
}

#[test]
fn attack_materializes_five_sourceordered_typed_fireballs_and_lost_target_emits_none() {
    use crate::PlayerCommand;
    use crate::campaign::source_arena::CreatureRootOrigin;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent};

    let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-paj2-source-secret-34-campaign-package");
    let mut simulation = CampaignSession::open(package, CampaignSelection::SECRET_DESTINATION_34)
        .expect("owner package for the source group23 switch")
        .simulation()
        .clone();
    let target_slot = simulation.state.entity_index;
    let switch_position = simulation.entities()[96].position;
    simulation
        .set_level2_entity_position(target_slot, switch_position)
        .unwrap();
    let mut published = false;
    // Leave the player at the switch while the source player's normal spawn
    // protection expires; no test code writes the protection countdown.
    for _ in 0..128 {
        if simulation.session.state.live.player_protection_ticks == 0 {
            break;
        }
        simulation.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !simulation
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "owner protection-expiry scheduler error: {:?}",
            simulation.dispatch_events()
        );
        published |= simulation.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated {
                    source_slot: 96,
                    group: 23,
                    ..
                }
            )
        });
    }
    assert_eq!(
        simulation.session.state.live.player_protection_ticks, 0,
        "source visits must expire spawn protection before the impact comparison"
    );
    assert!(simulation.session.state.live.player_life > 0);
    let mut outside = switch_position;
    outside.x = outside.x.wrapping_add(0x6000);
    simulation
        .set_level2_entity_position(target_slot, outside)
        .unwrap();
    for _ in 0..8 {
        if published {
            break;
        }
        simulation.step_outer_turn(PlayerCommand::default(), 0);
        published |= simulation.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated {
                    source_slot: 96,
                    group: 23,
                    ..
                }
            )
        });
    }
    assert!(published, "owner switch must produce the controlled actor");
    let actor_slot = simulation
        .entities()
        .iter()
        .find_map(|row| {
            let identity = simulation.creature_identity_for_source_slot(row.index)?;
            matches!(
                identity.origin,
                CreatureRootOrigin::AuthoredRecord {
                    authored_record: 566
                }
            )
            .then_some(row.index)
        })
        .expect("owner group constructs model18 record566");
    let actor_position = simulation.entities()[usize::from(actor_slot)].position;
    let mut target_position = actor_position;
    target_position.x = target_position.x.wrapping_add(1_000);
    simulation
        .set_level2_entity_position(target_slot, target_position)
        .unwrap();
    let particles = simulation
        .session
        .definition
        .presentation
        .particles
        .to_vec();
    let live = &mut simulation.session.state.live;
    assert_eq!(
        (
            live.economy.entities[usize::from(target_slot)].class,
            live.economy.entities[usize::from(target_slot)].model
        ),
        (3, 0)
    );
    let (arena, runtime, terrain) = (&mut live.economy, &mut live.core.runtime, &mut live.terrain);
    terrain.map_type = crate::native_world::MapType::Night;
    {
        let actor = &mut arena.entities[usize::from(actor_slot)];
        actor.set_source_action(146);
        actor.set_phase(RULES.source_scan_period);
        actor.set_source_dword_10(10);
        actor.record[74] = 1;
        put(actor, 154, target_slot);
    }
    let classes = class3_slots(arena);
    let families = model_slots(arena);
    let mut unlit_arena = arena.clone();
    unlit_arena.dynamic_lighting_enabled = false;
    let mut unlit_runtime = runtime.clone();
    let mut unlit_terrain = terrain.clone();
    let before = arena.economy.fireball_projectiles().len();
    let effects = mc2::step_row(
        arena, runtime, terrain, &particles, actor_slot, &classes, &families,
    )
    .expect("model18 attack callback succeeds");

    assert_eq!(effects.projectile_slots.len(), 5);
    let source_position = arena.entities[usize::from(actor_slot)].position;
    for slot in &effects.projectile_slots {
        assert!(
            arena
                .dynamic_lighting
                .active_slots()
                .any(|(_, light)| light.linked
                    && light.source_entity_index == *slot
                    && light.position == source_position)
        );
    }
    let unlit = mc2::step_row(
        &mut unlit_arena,
        &mut unlit_runtime,
        &mut unlit_terrain,
        &particles,
        actor_slot,
        &classes,
        &families,
    )
    .expect("disabled lighting does not reject the five source projectiles");
    assert_eq!(unlit.projectile_slots.len(), 5);
    assert!(unlit.projectile_slots.iter().all(|slot| {
        !unlit_arena
            .dynamic_lighting
            .active_slots()
            .any(|(_, light)| light.linked && light.source_entity_index == *slot)
    }));
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

    let actor_position = actor.position;
    let shot_handles = effects
        .projectile_slots
        .iter()
        .map(|&slot| arena.handle(slot).expect("emitted source generation"))
        .collect::<Vec<_>>();
    let source_slot = effects.projectile_slots[0];
    let consumer = {
        let (economy, entities) = (&mut arena.economy, &arena.entities);
        economy.step_fireball_projectile_slot(source_slot, terrain, entities)
    };
    assert!(
        consumer.live || consumer.impact_slot.is_some(),
        "the shared source fireball consumer must advance the emitted shot"
    );

    // Keep the no-target actor at the same position in a separate clone, so
    // it cannot collide with the original fan during the later scheduler run.
    let mut lost_arena = arena.clone();
    let mut lost_runtime = runtime.clone();
    let mut lost_terrain = terrain.clone();
    let second = add_creature(
        &mut lost_arena,
        &mut lost_terrain,
        &particles,
        actor_position,
        146,
    );
    {
        let actor = &mut lost_arena.entities[usize::from(second)];
        actor.set_phase(RULES.source_scan_period);
        actor.set_source_dword_10(10);
        actor.record[74] = 1;
        put(actor, 154, 0);
    }
    let classes = class3_slots(&lost_arena);
    let families = model_slots(&lost_arena);
    let before_lost_target = lost_arena.economy.fireball_projectiles().len();
    let outcome = mc2::step_row(
        &mut lost_arena,
        &mut lost_runtime,
        &mut lost_terrain,
        &particles,
        second,
        &classes,
        &families,
    )
    .expect("lost-target callback returns to behavior");
    assert!(outcome.projectile_slots.is_empty());
    assert_eq!(
        lost_arena.economy.fireball_projectiles().len(),
        before_lost_target
    );
    // Let the source attack leave its firing phase on the next callback, so
    // only the original five shots can satisfy this consumer check.
    arena.entities[usize::from(actor_slot)].set_source_dword_10(1);

    // Continue the emitted fan through the ordinary scheduler. Both cases
    // use the same unprotected player; only the negative moves that player
    // out of reach after launch. This is a controlled consumer check, not a
    // substitute for the owner-produced acquisition path below.
    assert_eq!(target_slot, simulation.state.entity_index);
    assert_eq!(simulation.session.state.live.player_protection_ticks, 0);
    let original_life = simulation.session.state.live.player_life;
    let mut distant = simulation.clone();
    let mut distant_position = target_position;
    distant_position.x = distant_position.x.wrapping_add(0x6000);
    distant
        .set_level2_entity_position(target_slot, distant_position)
        .unwrap();
    let mut saw_impact = false;
    let mut saw_damage = false;
    let mut saw_reclaim = false;
    let mut distant_damage = false;
    let mut impact_observation = None;
    let mut near_receipts = Vec::new();
    let mut distant_receipts = Vec::new();
    for _ in 0..24 {
        crate::native_world::area_damage::start_effect_damage_trace(target_slot);
        simulation.step_outer_turn(PlayerCommand::default(), 0);
        near_receipts.extend(
            crate::native_world::area_damage::take_effect_damage_trace()
                .into_iter()
                .filter(|receipt| receipt.pulse.owner == actor_slot),
        );
        crate::native_world::area_damage::start_effect_damage_trace(target_slot);
        distant.step_outer_turn(PlayerCommand::default(), 0);
        distant_receipts.extend(
            crate::native_world::area_damage::take_effect_damage_trace()
                .into_iter()
                .filter(|receipt| receipt.pulse.owner == actor_slot),
        );
        assert!(
            !simulation
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "near-case scheduler error: {:?}",
            simulation.dispatch_events()
        );
        assert!(
            !distant
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "distant-case scheduler error: {:?}",
            distant.dispatch_events()
        );
        let arena = &simulation.session.state.live.economy;
        if let Some(row) = arena
            .entities
            .iter()
            .find(|row| row.active && (row.class, row.model, row.source_id) == (10, 0, actor_slot))
        {
            saw_impact |=
                put_word(row, 154) == target_slot && row.subspell_index == RULES.projectile_damage;
            impact_observation = Some((
                row.index,
                row.position,
                put_word(row, 154),
                row.subspell_index,
                (row.control_flags, row.record[69], row.record[70]),
                (row.life, row.phase, row.source_dword_10()),
                (row.array_pitch(), row.array_yaw(), row.array_fov()),
                arena.entities[usize::from(target_slot)].position,
                crate::native_world::area_damage::read_arena_actor(arena, target_slot),
            ));
        }
        let target = &arena.entities[usize::from(target_slot)];
        let pending = i32::from_le_bytes(target.record[98..102].try_into().unwrap());
        saw_damage |= pending > 0 && put_word(target, 102) == actor_slot;
        saw_damage |= simulation.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::Economy { event: Level2EconomyEvent::DamageApplied {
                target_entity_index, attacker_entity_index, damage, ..
            }, .. } if *target_entity_index == target_slot && *attacker_entity_index == actor_slot && *damage > 0
        ));
        saw_reclaim |= shot_handles.iter().any(|&handle| {
            arena
                .economy
                .free_source_slots()
                .contains(&handle.source_slot)
                && arena.resolve(handle).is_none()
        });
        let distant_target = &distant.entities()[usize::from(target_slot)];
        let distant_pending =
            i32::from_le_bytes(distant_target.record[98..102].try_into().unwrap());
        distant_damage |= distant_pending > 0 && put_word(distant_target, 102) == actor_slot;
        distant_damage |= distant.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::Economy { event: Level2EconomyEvent::DamageApplied {
                target_entity_index, attacker_entity_index, damage, ..
            }, .. } if *target_entity_index == target_slot && *attacker_entity_index == actor_slot && *damage > 0
        ));
        if saw_impact
            && saw_damage
            && saw_reclaim
            && simulation.session.state.live.player_life < original_life
        {
            break;
        }
    }
    assert!(
        saw_impact,
        "an emitted model18 shot must create its target-bound impact: {impact_observation:?}; callbacks={near_receipts:?}"
    );
    assert!(
        saw_damage,
        "the impact must deliver source-attributed damage: {impact_observation:?}; callbacks={near_receipts:?}"
    );
    assert!(
        simulation.session.state.live.player_life < original_life,
        "the player must consume that damage"
    );
    assert!(
        saw_reclaim,
        "an emitted shot must reach the following-pass generation-safe reclaim"
    );
    assert!(
        !distant_damage,
        "the same fan must not damage the player moved out of reach: {distant_receipts:?}"
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

    let bound = add_creature(arena, terrain, &particles, position, 151);
    let row = &mut arena.entities[usize::from(bound)];
    row.record[77] = 16;
    put(row, 44, parent);
    put(row, 50, 10);
    let child_handle = arena.handle(bound).unwrap();
    let parent_handle = arena.handle(parent).unwrap();
    assert!(arena.bind_creature_parent(child_handle, parent_handle));
    let mut living_arena = arena.clone();
    let mut living_runtime = runtime.clone();
    let mut living_terrain = terrain.clone();
    let classes = class3_slots(&living_arena);
    let families = model_slots(&living_arena);
    let living = mc2::step_row(
        &mut living_arena,
        &mut living_runtime,
        &mut living_terrain,
        &particles,
        bound,
        &classes,
        &families,
    )
    .expect("bound live parent callback");
    assert!(!living.expire);
    assert_eq!(
        living_arena.creature_parent(child_handle),
        Some(parent_handle)
    );

    let replacement = arena.entities[usize::from(parent)].clone();
    arena.entities[usize::from(parent)].reclaim_source_row();
    arena.economy.release_slot(parent, None);
    assert_eq!(arena.allocate_source_slot(), Some(parent));
    arena.entities[usize::from(parent)] = replacement;
    assert_ne!(arena.handle(parent).unwrap(), parent_handle);
    assert_eq!(
        arena.creature_parent_binding(child_handle),
        Some(parent_handle)
    );
    assert!(arena.creature_parent(child_handle).is_none());
    let classes = class3_slots(arena);
    let families = model_slots(arena);
    let expired = mc2::step_row(
        arena, runtime, terrain, &particles, bound, &classes, &families,
    )
    .expect("reused parent slot expires the bound summon");
    assert!(expired.expire);
    assert!(!expired.summon_expire);
    assert!(arena.entities[usize::from(bound)].drawing_disabled());
    assert!(arena.resolve(child_handle).is_some());
    arena
        .rebuild_source_arena_with_terrain(terrain, &[])
        .unwrap();
    assert!(arena.resolve(child_handle).is_none());
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
        families: &[],
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
        families: &[],
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
    // The same owner group includes the Fireball source. Exercise the real
    // pickup on a branch so it cannot change the creature attack below.
    let fireball = session
        .economy()
        .spell_pickups()
        .iter()
        .find(|source| source.authored_index == 739)
        .copied()
        .expect("group 23 constructs its owner-authored Fireball pickup");
    assert_eq!(fireball.family, 0);
    assert_eq!(
        session.shared_spells().authored_pickup_slots().get(&739),
        Some(&fireball.source_entity_index)
    );
    let mut pickup_far = session.clone();
    let mut far_position = fireball.position;
    far_position.x = far_position.x.wrapping_add(0x6000);
    pickup_far
        .set_level2_entity_position(player, far_position)
        .unwrap();
    for _ in 0..4 {
        pickup_far.step_outer_turn(PlayerCommand::default(), 0);
        assert!(!pickup_far.dispatch_events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::SpellAcquisition {
                source_slot,
                result: Some(_),
                ..
            } if *source_slot == fireball.source_entity_index
        )));
    }
    let mut pickup_near = session.clone();
    pickup_near
        .set_level2_entity_position(player, fireball.position)
        .unwrap();
    let acquired = (0..4).find_map(|_| {
        pickup_near.step_outer_turn(PlayerCommand::default(), 0);
        pickup_near
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Mc2DispatchEvent::SpellAcquisition {
                    family: 0,
                    source_slot,
                    result: Some(outcome),
                    ..
                } if *source_slot == fireball.source_entity_index => Some(*outcome),
                _ => None,
            })
    });
    let acquired = acquired.expect("overlap acquires the owner-authored Fireball source");
    assert_eq!(
        (acquired.family, acquired.source_slot),
        (0, fireball.source_entity_index)
    );
    assert_eq!(
        pickup_near
            .shared_spells()
            .players()
            .iter()
            .find(|row| row.entity_index == player)
            .expect("source player")
            .enabled_spell_holders[0],
        fireball.source_entity_index
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
            .then_some((row.index, row.phase, row.record[61]))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        slots.len(),
        3,
        "all three owner-authored model18 roots must be live"
    );
    // Fire once through the actual selected holder and starting player mana.
    // The paired branch receives the same movement and source turns without
    // the fire input; neither branch edits creature or inventory state.
    let actor_slot = slots[0].0;
    let fireball_ui = crate::SOURCE_SPELL_INDEX_MAP
        .iter()
        .position(|family| *family == 0)
        .expect("Fireball has a source menu entry") as i8;
    pickup_near.step_outer_turn(
        PlayerCommand {
            opcode: 31,
            argument_1: fireball_ui,
            argument_2: 0,
            ..PlayerCommand::default()
        },
        0,
    );
    let actor = pickup_near.entities()[usize::from(actor_slot)].clone();
    let player_position = Position {
        x: actor.position.x.wrapping_add(256),
        y: actor.position.y.wrapping_add(384),
        z: actor.position.z.wrapping_sub(pickup_near.state.entity_fov),
    };
    pickup_near
        .set_level2_entity_position(player, player_position)
        .unwrap();
    pickup_near.state.yaw = 0;
    pickup_near.state.display_pitch = 0;
    pickup_near.state.effective_pitch = 0;
    pickup_near.state.actual_speed = 0;
    pickup_near.state.target_speed = 0;
    let mut no_fire = pickup_near.clone();
    let life_before = actor.life;
    let mut player_projectile = false;
    let mut actor_hit = false;
    for turn in 0..16 {
        pickup_near.step_outer_turn(
            PlayerCommand {
                flags: if turn == 0 { 0x10 } else { 0 },
                ..PlayerCommand::default()
            },
            0,
        );
        player_projectile |= pickup_near
            .economy()
            .fireball_projectiles()
            .iter()
            .any(|shot| shot.owner_entity_index == player);
        let row = &pickup_near.entities()[usize::from(actor_slot)];
        actor_hit |= row.life < life_before
            && u16::from_le_bytes([row.record[42], row.record[43]]) == player;
        no_fire.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !no_fire
                .economy()
                .fireball_projectiles()
                .iter()
                .any(|shot| shot.owner_entity_index == player)
        );
        if player_projectile && actor_hit {
            break;
        }
    }
    assert!(
        player_projectile && actor_hit,
        "owner Fireball must reach the authored model18 through real input: projectile={player_projectile}, hit={actor_hit}, actor={:?}, events={:?}",
        pickup_near.entities()[usize::from(actor_slot)],
        pickup_near.dispatch_events()
    );
    let model21_slots = session
        .entities()
        .iter()
        .filter_map(|row| {
            let identity = session.creature_identity_for_source_slot(row.index)?;
            (identity.kind == crate::level_definition::CreatureKind::Creature21
                && matches!(
                    identity.origin,
                    CreatureRootOrigin::AuthoredRecord {
                        authored_record: 569..=571
                    }
                ))
            .then_some((row.index, row.phase, row.record[61]))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        model21_slots.len(),
        3,
        "the same real group also constructs its three model21 roots"
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    for (slot, phase, proximity) in slots.iter().copied().chain(model21_slots) {
        let actor = &session.entities()[usize::from(slot)];
        assert!(actor.active && !actor.drawing_disabled());
        assert_eq!(
            actor.phase,
            phase.wrapping_add(1),
            "normal source scheduler visits authored creature slot {slot}"
        );
        assert!(
            proximity > 0,
            "new source creature starts with proximity countdown"
        );
        assert_eq!(
            actor.record[61],
            proximity - 1,
            "source proximity prepass visits authored creature slot {slot}"
        );
    }

    // Keep the player distant from the produced actor until the source
    // player's spawn protection expires, before testing unforced acquisition.
    let mut protection_wait_position = session.entities()[usize::from(slots[0].0)].position;
    protection_wait_position.x = protection_wait_position.x.wrapping_add(0x6000);
    session
        .set_level2_entity_position(player, protection_wait_position)
        .unwrap();
    for _ in 0..128 {
        if session.session.state.live.player_protection_ticks == 0 {
            break;
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "owner model18 protection-expiry scheduler error: {:?}",
            session.dispatch_events()
        );
    }
    assert_eq!(session.session.state.live.player_protection_ticks, 0);
    assert!(session.session.state.live.player_life > 0);

    // Place the real player once in front of an owner-produced model18. The
    // ordinary callback, not the test, selects its target on a later pass.
    let slot = slots[0].0;
    let produced = session.entities()[usize::from(slot)].clone();
    let mut nearby = produced.position;
    let candidates = [
        (1024_i16, 0_i16),
        (724, 724),
        (0, 1024),
        (-724, 724),
        (-1024, 0),
        (-724, -724),
        (0, -1024),
        (724, -724),
    ];
    let (dx, dy) = candidates
        .into_iter()
        .min_by_key(|&(dx, dy)| {
            let mut candidate = produced.position;
            candidate.x = candidate.x.wrapping_add_signed(dx);
            candidate.y = candidate.y.wrapping_add_signed(dy);
            angle_distance(
                i32::from(produced.yaw),
                heading_for_positions(produced.position, candidate),
            )
        })
        .unwrap();
    nearby.x = nearby.x.wrapping_add_signed(dx);
    nearby.y = nearby.y.wrapping_add_signed(dy);
    let mut distant_session = session.clone();
    let mut distant = produced.position;
    distant.x = distant.x.wrapping_add(0x6000);
    distant_session
        .set_level2_entity_position(player, distant)
        .unwrap();
    session.set_level2_entity_position(player, nearby).unwrap();
    let player_life_before_attack = session.session.state.live.player_life;
    let mut acquired = false;
    for _ in 0..20 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        distant_session.step_outer_turn(PlayerCommand::default(), 0);
        acquired |= put_word(&session.entities()[usize::from(slot)], 154) == player;
        assert_ne!(
            put_word(&distant_session.entities()[usize::from(slot)], 154),
            player,
            "distant player must not be acquired by the authored model18"
        );
        if acquired {
            break;
        }
    }
    assert!(
        acquired,
        "authored model18 must acquire the nearby player through its normal source callback: {:?}",
        session.dispatch_events()
    );

    // Continue the same owner-produced actor with no action, target, timer,
    // or RNG edits. Its source callback decides when tracking becomes attack
    // and the shared scheduler advances any higher-slot newborns in that pass.
    let mut entered_attack = false;
    let mut emitted = None;
    let mut player_life_before_last_pass = session.session.state.live.player_life;
    for _ in 0..256 {
        player_life_before_last_pass = session.session.state.live.player_life;
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "owner model18 scheduler error: {:?}",
            session.dispatch_events()
        );
        entered_attack |= session.entities()[usize::from(slot)].action == 146;
        emitted = session
            .session
            .state
            .live
            .economy
            .economy
            .fireball_projectiles()
            .iter()
            .find(|shot| {
                shot.origin == crate::native_world::FireballProjectileOrigin::Creature18
                    && shot.owner_entity_index == slot
                    && shot.target_source_entity_index == player
                    && shot.source_subspell_index == RULES.projectile_damage
            })
            .map(|shot| shot.source_entity_index);
        if emitted.is_some() {
            break;
        }
    }
    let actor = &session.entities()[usize::from(slot)];
    assert!(
        entered_attack && emitted.is_some(),
        "owner model18 must naturally enter attack and emit a target-bound fireball: slot={slot} action={} phase={} attack_phase={} timer={} rng={} target={}",
        actor.action,
        actor.phase,
        actor.record[74],
        actor.source_dword_10(),
        actor.entity_rng,
        put_word(actor, 154)
    );
    eprintln!(
        "owner model18 source actor={slot} projectile={} target={player} damage={} player_protection={} player_life={}",
        emitted.unwrap(),
        RULES.projectile_damage,
        session.session.state.live.player_protection_ticks,
        session.session.state.live.player_life,
    );

    // Follow the shots from that same unforced actor through the common
    // projectile and impact consumers, while tracking their source generations.
    let shot_handles = session
        .session
        .state
        .live
        .economy
        .economy
        .fireball_projectiles()
        .iter()
        .filter(|shot| {
            shot.origin == crate::native_world::FireballProjectileOrigin::Creature18
                && shot.owner_entity_index == slot
                && shot.target_source_entity_index == player
                && shot.source_subspell_index == RULES.projectile_damage
        })
        .filter_map(|shot| session.source_arena().handle(shot.source_entity_index))
        .collect::<Vec<_>>();
    assert!(
        !shot_handles.is_empty(),
        "natural shot retains a source generation"
    );
    let mut natural_impact = false;
    let mut natural_damage = false;
    let mut natural_reclaim = false;
    let mut natural_consumption = None;
    for _ in 0..=24 {
        let arena = session.source_arena();
        natural_impact |= arena.entities.iter().any(|row| {
            row.active
                && (row.class, row.model, row.source_id) == (10, 0, slot)
                && put_word(row, 154) == player
                && row.subspell_index == RULES.projectile_damage
        });
        let target = &arena.entities[usize::from(player)];
        natural_damage |= i32::from_le_bytes(target.record[98..102].try_into().unwrap()) > 0
            && put_word(target, 102) == slot;
        let consumed = session
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Mc2DispatchEvent::Economy {
                    event:
                        crate::native_world::Level2EconomyEvent::DamageApplied {
                            target_entity_index,
                            attacker_entity_index,
                            damage,
                            ..
                        },
                    ..
                } if *target_entity_index == player
                    && *attacker_entity_index == slot
                    && *damage > 0 =>
                {
                    Some(*damage)
                }
                _ => None,
            });
        natural_damage |= consumed.is_some();
        let current_life = session.session.state.live.player_life;
        if let Some(damage) = consumed
            && current_life < player_life_before_last_pass
        {
            natural_consumption = Some((damage, player_life_before_last_pass, current_life));
        }
        natural_reclaim |= shot_handles
            .iter()
            .any(|&handle| arena.resolve(handle).is_none());
        if natural_impact
            && natural_damage
            && natural_consumption.is_some()
            && natural_reclaim
            && session.session.state.live.player_life < player_life_before_attack
        {
            break;
        }
        player_life_before_last_pass = session.session.state.live.player_life;
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "natural model18 projectile scheduler error: {:?}",
            session.dispatch_events()
        );
    }
    assert!(
        natural_impact,
        "the unforced source shot must produce its target-bound impact"
    );
    assert!(
        natural_damage,
        "the unforced source impact must reach the player's damage mailbox"
    );
    let (consumed_damage, previous_life, current_life) = natural_consumption.expect(
        "player source visit must consume this actor's damage and reduce life in that pass",
    );
    eprintln!(
        "owner model18 actor={slot} player={player} consumed_damage={consumed_damage} life={previous_life}->{current_life}"
    );
    assert!(session.session.state.live.player_life < player_life_before_attack);
    assert!(
        natural_reclaim,
        "the unforced source projectile must be generation-safely reclaimed"
    );

    // Exercise the produced actor's downstream death consumer separately:
    // the two worlds differ only in the controlled source mailbox amount.
    // This does not claim that the player's own attack produced the damage.
    let handle = session.source_arena().handle(slot).unwrap();
    let life = session.entities()[usize::from(slot)].life;
    let mana = session.entities()[usize::from(slot)].mana;
    assert!(life > 1 && mana > 0);
    let mut fatal = session.clone();
    let mut nonfatal = session.clone();
    let kills_before = fatal
        .session
        .state
        .live
        .economy
        .economy
        .authored_bee_kill_census(player);
    for (world, damage) in [(&mut fatal, life + 1), (&mut nonfatal, life - 1)] {
        let row = &mut world.session.state.live.economy.entities[usize::from(slot)];
        row.record[98..102].copy_from_slice(&damage.to_le_bytes());
        put(row, 102, player);
    }
    nonfatal.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(nonfatal.entities()[usize::from(slot)].life, 1);
    assert!(
        nonfatal
            .creature_identity_for_source_slot(slot)
            .is_some_and(|identity| identity.handle == handle && identity.living)
    );
    assert_eq!(
        nonfatal
            .session
            .state
            .live
            .economy
            .economy
            .authored_bee_kill_census(player),
        kills_before
    );

    let mut saw_prekill = false;
    let mut saw_mana = false;
    let mut saw_hidden = false;
    let mut saw_reclaim = false;
    for _ in 0..16 {
        let awaiting_next_reclaim = saw_hidden;
        fatal.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !fatal
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "owner model18 death scheduler error: {:?}",
            fatal.dispatch_events()
        );
        if awaiting_next_reclaim {
            assert!(
                fatal.source_arena().resolve(handle).is_none(),
                "the hidden original generation must be reclaimed at the immediately following source boundary"
            );
        }
        let row = &fatal.entities()[usize::from(slot)];
        saw_prekill |= row.action == 148;
        saw_mana |= fatal.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::Economy { event: crate::native_world::Level2EconomyEvent::ManaSphereSpawned {
                source_entity_index, mana: released, ..
            }, .. } if *source_entity_index == slot && *released == mana
        ));
        saw_hidden |= row.drawing_disabled() && fatal.source_arena().resolve(handle).is_some();
        saw_reclaim |= fatal.source_arena().resolve(handle).is_none();
        if saw_hidden && saw_reclaim && saw_mana {
            break;
        }
    }
    assert!(
        saw_prekill,
        "fatal damage must enter the source prekill action"
    );
    assert_eq!(
        fatal
            .session
            .state
            .live
            .economy
            .economy
            .authored_bee_kill_census(player),
        kills_before + 1,
        "the real player receives one source kill credit"
    );
    assert!(
        saw_mana,
        "death must transfer the actor's mana to its source sphere"
    );
    assert!(
        saw_hidden && saw_reclaim,
        "death must hide before next-pass generation reclaim"
    );
    assert!(
        !fatal
            .creature_identity_for_source_slot(slot)
            .is_some_and(|identity| identity.handle == handle && identity.living)
    );
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
        families: &[],
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
    actor.action = 144;
    actor.attack_phase = 1;
    actor.timer = 10;
    actor.target = hidden.slot;
    step(&mut actor, &hidden_context).expect("tracking reads a hidden source target");
    assert_eq!(actor.target, hidden.slot);

    actor.action = 146;
    actor.attack_phase = 1;
    actor.timer = 10;
    let effects = step(&mut actor, &hidden_context).expect("hidden target callback");
    assert!(effects.projectiles.is_empty());
}

#[test]
fn alliance_follows_parent_and_only_fires_at_its_selected_target() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
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
            ..position
        },
        ..Target::default()
    };
    let parent = Target {
        slot: 9,
        class: 3,
        life: 100,
        target: 42,
        position,
        ..Target::default()
    };
    let child = Target {
        slot: 11,
        class: 5,
        model: 18,
        life: RULES.max_life,
        ..Target::default()
    };
    let targets = [parent, target, child];
    let context = Creature18Context {
        terrain: &simulation.session.state.live.terrain,
        runtime: &simulation.session.state.live.core.runtime,
        targets: &targets,
        class3: &[],
        model18: &[],
        families: &[],
    };
    let mut counter = 0;
    let mut actor = construct(7, 1, 0x1234, position, &mut counter);
    actor.action = 151;
    actor.route_mode = 14;
    actor.route_remaining = 40;
    actor.route_duration = 40;
    actor.parent = parent.slot;
    actor.linked_child = child.slot;
    actor.phase = 1;
    actor.pending_attacker = target.slot;
    actor.pending_damage = 50;
    let effects = step(&mut actor, &context).expect("Alliance stage callback");
    assert_eq!(actor.life, RULES.max_life - 50);
    assert_eq!((actor.target, actor.action), (target.slot, 146));
    assert_eq!(effects.earlier_feedback, Some((parent.slot, 19)));
    assert_eq!(effects.feedback, Some((parent.slot, 24)));

    actor.attack_phase = 1;
    actor.phase = 0;
    actor.timer = 10;
    let mut rejected = actor;
    let mut rejected_parent = parent;
    rejected_parent.target = 43;
    let rejected_targets = [rejected_parent, target, child];
    let rejected_context = Creature18Context {
        targets: &rejected_targets,
        ..context
    };
    let rejected_effects =
        step(&mut rejected, &rejected_context).expect("Alliance rejects other target");
    assert!(rejected_effects.projectiles.is_empty());
    assert_eq!(rejected.attack_phase, 2);

    let effects = step(&mut actor, &context).expect("Alliance firing callback");
    assert_eq!(effects.projectiles.len(), 5);
    assert_eq!(actor.route_remaining, 38);
    assert_eq!(actor.control_flags & 0x0600_0000, 0x0400_0000);
    assert_eq!(effects.linked_blink_flags, [(child.slot, 0x0400_0000)]);

    actor.action = 151;
    actor.route_remaining = 1;
    let effects = step(&mut actor, &context).expect("Alliance expiration callback");
    assert_eq!((actor.route_mode, actor.parent, actor.target), (10, 0, 0));
    assert_eq!(actor.control_flags & 0x0600_0000, 0);
    assert_eq!(effects.linked_blink_flags, [(child.slot, 0)]);
}

#[test]
fn alliance_firing_materializes_projectiles_and_updates_linked_source_flags() {
    let mut simulation = crate::native_world::tests::level_two_campaign_session();
    let particles = simulation
        .session
        .definition
        .presentation
        .particles
        .to_vec();
    let live = &mut simulation.session.state.live;
    let parent = live
        .economy
        .entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.model == 0)
        .expect("source player")
        .index;
    let (arena, runtime, terrain) = (&mut live.economy, &mut live.core.runtime, &mut live.terrain);
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 0,
    };
    let actor = add_creature(arena, terrain, &particles, position, 146);
    let child = add_creature(arena, terrain, &particles, position, 144);
    let target = add_creature(
        arena,
        terrain,
        &particles,
        Position {
            x: 21_000,
            ..position
        },
        144,
    );
    put(&mut arena.entities[usize::from(parent)], 154, target);
    {
        let row = &mut arena.entities[usize::from(actor)];
        row.set_phase(0);
        row.set_source_dword_10(10);
        row.record[74] = 1;
        row.record[77] = 14;
        put(row, 44, parent);
        put(row, 50, 40);
        put(row, 52, 40);
        put(row, 56, child);
        put(row, 154, target);
    }
    let classes = class3_slots(arena);
    let families = model_slots(arena);
    let before = arena.economy.fireball_projectiles().len();
    let outcome = mc2::step_row(
        arena, runtime, terrain, &particles, actor, &classes, &families,
    )
    .expect("Alliance source firing");
    assert_eq!(outcome.projectile_slots.len(), 5);
    assert_eq!(arena.economy.fireball_projectiles().len(), before + 5);
    assert_eq!(
        arena.entities[usize::from(child)].control_flags & 0x0600_0000,
        0x0400_0000
    );

    put(&mut arena.entities[usize::from(parent)], 154, child);
    {
        let row = &mut arena.entities[usize::from(actor)];
        row.set_phase(0);
        row.set_source_dword_10(10);
        row.record[74] = 1;
    }
    let outcome = mc2::step_row(
        arena, runtime, terrain, &particles, actor, &classes, &families,
    )
    .expect("Alliance rejected-target callback");
    assert!(outcome.projectile_slots.is_empty());
    assert_eq!(arena.economy.fireball_projectiles().len(), before + 5);
}
