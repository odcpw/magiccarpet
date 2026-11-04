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

    // These explicit releases qualify fixed-address semantics; they do not
    // claim that normal player death reclaims the original player packets.
    let target = arena
        .entities
        .iter()
        .find(|row| row.active && (row.class, row.model) == (3, 1))
        .expect("fixture retains a rival target")
        .index;
    let parent_position = arena.entities[usize::from(parent)].position;
    {
        let row = &mut arena.entities[usize::from(bound)];
        put(row, 154, target);
        row.set_phase(1);
        row.set_control_flags(row.control_flags | 0x800);
    }
    for (address, life) in [(target, 100), (parent, 0)] {
        let position = arena.entities[usize::from(address)].position;
        crate::world::effects::sync_position(arena, terrain, address, position, false, false)
            .unwrap();
        arena.entities[usize::from(address)].set_life(life);
        arena.entities[usize::from(address)].reclaim_source_row();
        arena.economy.release_slot(address, None);
    }
    let families = model_slots(arena);
    let retained = mc2::step_row(arena, runtime, terrain, &particles, bound, &[], &families)
        .expect("fixed parent and target remain readable after source release");
    assert!(!retained.expire);
    assert_eq!(put_word(&arena.entities[usize::from(bound)], 154), target);
    assert_eq!(put_word(&arena.entities[usize::from(bound)], 50), 10);
    assert!(!arena.entities[usize::from(bound)].drawing_disabled());

    let replacement = crate::world::effects::allocate(
        arena,
        terrain,
        sim_definition::adapter::mc2_effect_definition(crate::level_definition::EffectKind::Impact),
        parent_position,
        &particles,
    )
    .unwrap()
    .expect("the real Impact constructor allocates a replacement");
    assert_eq!(
        replacement, parent,
        "NewEvent reuses the last released address"
    );
    assert_ne!(arena.handle(parent).unwrap(), parent_handle);
    assert_eq!(
        (
            arena.entities[usize::from(parent)].class,
            arena.entities[usize::from(parent)].model
        ),
        (10, 0)
    );
    assert_eq!(
        arena.creature_parent_binding(child_handle),
        Some(parent_handle)
    );
    assert!(arena.creature_parent(child_handle).is_none());
    let flags = arena.entities[usize::from(bound)].control_flags;
    arena.entities[usize::from(bound)].set_control_flags(flags | 0x800);
    let families = model_slots(arena);
    let reused = mc2::step_row(arena, runtime, terrain, &particles, bound, &[], &families)
        .expect("the current positive-life replacement remains a valid fixed parent");
    assert!(!reused.expire);
    assert_eq!(put_word(&arena.entities[usize::from(bound)], 154), target);
    assert_eq!(put_word(&arena.entities[usize::from(bound)], 50), 10);
    assert!(!arena.entities[usize::from(bound)].drawing_disabled());

    arena.entities[usize::from(parent)].set_life(-1);
    let expired = mc2::step_row(arena, runtime, terrain, &particles, bound, &[], &families)
        .expect("negative current parent life expires the configured summon");
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
fn action144_tracks_by_horizontal_distance_and_clears_at_strict_source_range() {
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
    let tracking = actor;
    let tracking_rng = level2_next_rng16(actor.rng);
    assert_ne!(tracking_rng % 49, 0);

    step(&mut actor, &context).expect("action144 range callback runs");

    assert!(source_distance_3d(actor.position, target_position) > RULES.target_range);
    assert_eq!(actor.target, target_slot, "source58490 ignores altitude");
    assert_eq!(actor.action, 144);
    assert_eq!(actor.attack_phase, 1);
    assert_eq!(actor.rng, tracking_rng);

    for distance in [RULES.target_range, RULES.target_range + 1] {
        let boundary_targets = [Target {
            position: Position {
                x: actor_position.x.wrapping_add(distance as u16),
                ..target_position
            },
            ..targets[0]
        }];
        let boundary = Creature18Context {
            targets: &boundary_targets,
            ..context
        };
        let mut actor = tracking;
        step(&mut actor, &boundary).expect("action144 horizontal boundary callback runs");
        assert_eq!(actor.target, 0, "strict horizontal range={distance}");
        assert_eq!(actor.action, 144);
        assert_eq!(actor.attack_phase, 0);
        assert!((400..=799).contains(&actor.timer));
        assert_eq!(actor.rng, tracking_rng);
    }
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
    // The same authored group has the ordinary Alliance source. Its real
    // fresh-player state also exercises the source resource gate: the holder
    // can be acquired, but a cast cannot launch without an earned Castle and
    // the required mana. Positive casting requires real carried progression.
    let alliance = session
        .economy()
        .spell_pickups()
        .iter()
        .find(|source| source.authored_index == 765)
        .copied()
        .expect("group 23 constructs its owner-authored Alliance pickup");
    assert_eq!(alliance.family, 24);
    assert_eq!(
        session.shared_spells().authored_pickup_slots().get(&765),
        Some(&alliance.source_entity_index)
    );
    let mut alliance_far = session.clone();
    let mut far_alliance = alliance.position;
    far_alliance.x = far_alliance.x.wrapping_add(0x6000);
    alliance_far
        .set_level2_entity_position(player, far_alliance)
        .unwrap();
    for _ in 0..4 {
        alliance_far.step_outer_turn(PlayerCommand::default(), 0);
        assert!(!alliance_far.dispatch_events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::SpellAcquisition {
                source_slot,
                result: Some(_),
                ..
            } if *source_slot == alliance.source_entity_index
        )));
    }
    let mut alliance_near = session.clone();
    alliance_near
        .set_level2_entity_position(player, alliance.position)
        .unwrap();
    let alliance_acquired = (0..4).find_map(|_| {
        alliance_near.step_outer_turn(PlayerCommand::default(), 0);
        alliance_near
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Mc2DispatchEvent::SpellAcquisition {
                    family: 24,
                    source_slot,
                    result: Some(outcome),
                    ..
                } if *source_slot == alliance.source_entity_index => Some(*outcome),
                _ => None,
            })
    });
    let alliance_acquired =
        alliance_acquired.expect("overlap acquires the authored Alliance source");
    assert_eq!(alliance_acquired.source_slot, alliance.source_entity_index);
    assert_eq!(
        alliance_near
            .shared_spells()
            .players()
            .iter()
            .find(|row| row.entity_index == player)
            .expect("source player")
            .enabled_spell_holders[24],
        alliance.source_entity_index
    );
    let alliance_acquired_session = alliance_near.clone();
    assert_eq!(alliance_near.economy().player_castle_entity_index(), 0);
    let alliance_ui = crate::SOURCE_SPELL_INDEX_MAP
        .iter()
        .position(|family| *family == 24)
        .expect("Alliance has a source menu entry") as i8;
    alliance_near.step_outer_turn(
        PlayerCommand {
            opcode: 31,
            argument_1: alliance_ui,
            argument_2: 0,
            ..PlayerCommand::default()
        },
        0,
    );
    for turn in 0..4 {
        alliance_near.step_outer_turn(
            PlayerCommand {
                flags: if turn == 0 { 0x10 } else { 0 },
                ..PlayerCommand::default()
            },
            0,
        );
        assert!(
            !alliance_near.entities().iter().any(|row| row.active
                && row.class == 9
                && row.model == 25
                && row.source_id == player)
        );
    }
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
    // Controlled resource boundary: the holder above is the actual authored
    // pickup, while Castle and mana are deliberately staged for this focused
    // integration check. This is not evidence of earned campaign resources.
    let rule = crate::level_definition::late_spells::ALLIANCE_PROPERTIES.tiers[0];
    let actor_slot = slots[0].0;

    // Exercise the same genuinely acquired holder and authored actor with
    // controlled carried progression and resources for the higher tiers.
    // The source stage-7 Castle can hold both prerequisites; this is not a
    // claim that the player earned the staged mana or progression here.
    for tier in 1_u8..=2 {
        let mut cast = alliance_acquired_session.clone();
        for _ in 0..32 {
            if cast
                .economy()
                .authored_model53()
                .iter()
                .all(|source| !source.active || source.life < 0)
                && cast
                    .economy()
                    .model17_spawned_children()
                    .iter()
                    .all(|child| !child.active)
                && cast.economy().free_slot_count() >= 12
            {
                break;
            }
            cast.step_outer_turn(PlayerCommand::default(), 0);
        }
        let holder_handle = cast
            .source_arena()
            .handle(alliance.source_entity_index)
            .expect("acquired Alliance holder remains live");
        let actor_handle = cast
            .source_arena()
            .handle(actor_slot)
            .expect("owner-authored Alliance target remains live");
        let parent_handle = cast
            .source_arena()
            .handle(player)
            .expect("acquiring player remains live");
        cast.apply_carried_spell_progression(&[crate::native_world::CarriedSpellProgress {
            family: 24,
            persistent_experience: crate::native_world::LEVEL2_SPELL_TIER_THRESHOLDS[24]
                [usize::from(tier)],
            runtime_experience: 0,
            unlocked_tier: tier as i8,
            selected_tier: tier as i8,
        }])
        .expect("controlled carried Alliance progression");
        assert_eq!(
            cast.source_arena().handle(alliance.source_entity_index),
            Some(holder_handle)
        );
        assert_eq!(
            cast.shared_spells()
                .pickup(alliance.source_entity_index)
                .unwrap()
                .level_gate,
            tier as i8
        );

        let tier_rule =
            crate::level_definition::late_spells::ALLIANCE_PROPERTIES.tiers[usize::from(tier)];
        let source_duration = u16::try_from(tier_rule.subspell_index)
            .expect("Alliance source duration fits its physical word");
        let castle = cast
            .economy_mut()
            .build_castle_at(
                Position::default(),
                Position::default(),
                player,
                false,
                true,
            )
            .expect("controlled source Castle constructor");
        {
            let mut economy = cast.economy_mut();
            let root = economy.castle_at_mut(castle).unwrap();
            root.level = 6;
            root.action = 5;
            root.build_state = 3;
            assert!(economy.step_castle_action(castle));
            assert_eq!(economy.castle_at(castle).unwrap().level, 7);
            assert!(
                economy.castle_at(castle).unwrap().mana_capacity
                    >= tier_rule.runtime_castle_mana_limit
            );
            assert!(economy.set_castle_mana_prerequisite_for_test(
                castle,
                tier_rule.runtime_castle_mana_limit
            ));
        }
        let player_mana = tier_rule.mana_cost + 1_000;
        cast.economy.entities[usize::from(player)].set_mana(player_mana);
        {
            let mut shared = cast.shared_spells_mut();
            shared.player_entity.set_mana(player_mana);
            shared.player_entities[0] = shared.player_entity.clone();
            shared.caster.mana = player_mana;
            shared.player_mana = player_mana;
        }
        cast.step_outer_turn(
            PlayerCommand {
                opcode: 31,
                argument_1: alliance_ui,
                argument_2: tier as i8,
                ..PlayerCommand::default()
            },
            0,
        );

        let target = cast.entities()[usize::from(actor_slot)].position;
        let player_position = Position {
            x: target.x,
            y: target.y.wrapping_add(1024),
            z: target.z.wrapping_add(1024),
        };
        cast.set_level2_entity_position(player, player_position)
            .unwrap();
        let (yaw, pitch) =
            crate::level1_mission::projectile_guidance_angles(player_position, target);
        cast.state.yaw = yaw;
        cast.state.display_pitch = pitch;
        cast.state.effective_pitch = pitch;
        cast.state.pitch_accumulator = pitch;
        cast.state.actual_speed = 0;
        cast.state.target_speed = 0;
        let mut saw_flyer = false;
        let mut saw_impact = false;
        let mut saw_allied = false;
        let mut trace = Vec::new();
        for turn in 0..24 {
            cast.step_outer_turn(
                PlayerCommand {
                    flags: if turn == 0 { 0x10 } else { 0 },
                    ..PlayerCommand::default()
                },
                0,
            );
            assert!(
                !cast
                    .dispatch_events()
                    .iter()
                    .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
                "tier {tier} Alliance dispatch: {:?}",
                cast.dispatch_events()
            );
            for row in cast
                .entities()
                .iter()
                .filter(|row| row.active && row.source_id == player)
            {
                if (row.class, row.model) == (9, 25) {
                    assert_eq!(
                        row.record[74], tier_rule.life,
                        "flyer carries source radius"
                    );
                    assert_eq!(
                        row.subspell_index, source_duration,
                        "flyer carries source duration"
                    );
                    saw_flyer = true;
                }
                if (row.class, row.model) == (10, 74) {
                    assert_eq!(
                        row.record[74], tier_rule.life,
                        "impact carries source radius"
                    );
                    assert_eq!(
                        row.subspell_index, source_duration,
                        "impact carries source duration"
                    );
                    saw_impact = true;
                }
            }
            let actor = &cast.entities()[usize::from(actor_slot)];
            let parent = u16::from_le_bytes([actor.record[44], actor.record[45]]);
            let remaining = u16::from_le_bytes([actor.record[50], actor.record[51]]);
            let duration = u16::from_le_bytes([actor.record[52], actor.record[53]]);
            if actor.record[77] == 14
                && parent == player
                && cast.source_arena().creature_parent_binding(actor_handle) == Some(parent_handle)
            {
                assert_eq!(duration, source_duration);
                assert!(remaining > 0 && remaining <= duration);
                saw_allied = true;
            }
            trace.push((
                turn,
                actor.action,
                actor.record[77],
                parent,
                remaining,
                duration,
                cast.economy().free_slot_count(),
            ));
            if saw_flyer && saw_impact && saw_allied {
                break;
            }
        }
        assert!(
            saw_flyer && saw_impact && saw_allied,
            "acquired tier {tier} must produce flyer, impact and ownership: flyer={saw_flyer}, impact={saw_impact}, allied={saw_allied}, trace={trace:?}"
        );
        assert_eq!(cast.source_arena().handle(actor_slot), Some(actor_handle));
        assert_eq!(
            cast.source_arena().handle(alliance.source_entity_index),
            Some(holder_handle)
        );
    }

    for (castle_mana, player_mana, settled, free_profile, admitted) in [
        (
            rule.runtime_castle_mana_limit - 1,
            rule.mana_cost,
            false,
            false,
            false,
        ),
        (
            rule.runtime_castle_mana_limit,
            rule.mana_cost - 1,
            false,
            false,
            false,
        ),
        // The immediate full-arena cast is the observed NewEvent failure
        // control; it still charges mana, as in source sub_6CD20.
        (
            rule.runtime_castle_mana_limit,
            rule.mana_cost,
            false,
            false,
            false,
        ),
        (
            rule.runtime_castle_mana_limit,
            rule.mana_cost,
            true,
            false,
            true,
        ),
        // The public source free-use option clears the physical Castle
        // requirement, but leaves the holder's actual mana cost intact.
        (
            rule.runtime_castle_mana_limit - 1,
            rule.mana_cost,
            true,
            true,
            true,
        ),
    ] {
        let mut cast = alliance_acquired_session.clone();
        if settled {
            let actor_handle = cast.source_arena().handle(actor_slot).unwrap();
            let holder_handle = cast
                .source_arena()
                .handle(alliance.source_entity_index)
                .unwrap();
            let emitters = cast
                .economy()
                .authored_model53()
                .iter()
                .filter(|source| source.active)
                .map(|source| source.source_entity_index)
                .collect::<Vec<_>>();
            assert!(
                !emitters.is_empty(),
                "the authored group produced model53 sources"
            );
            let mut lifetime_trace = Vec::new();
            // Model53 emits for its source life of 6; its model0 children
            // have life 8, followed by hide and next-pass reclaim. These are
            // ordinary no-input turns, not fabricated free slots or actors.
            for _ in 0..32 {
                let emitting = cast
                    .economy()
                    .authored_model53()
                    .iter()
                    .filter(|source| source.active && source.life >= 0)
                    .count();
                let children = cast
                    .economy()
                    .model17_spawned_children()
                    .iter()
                    .filter(|child| child.active && emitters.contains(&child.parent_entity_index))
                    .count();
                lifetime_trace.push((emitting, children, cast.economy().free_slot_count()));
                if emitting == 0 && children == 0 && cast.economy().free_slot_count() >= 2 {
                    break;
                }
                cast.step_outer_turn(PlayerCommand::default(), 0);
            }
            assert!(
                cast.economy()
                    .authored_model53()
                    .iter()
                    .all(|source| !source.active || source.life < 0),
                "authored model53 emission must stop: {lifetime_trace:?}"
            );
            assert!(
                cast.economy()
                    .model17_spawned_children()
                    .iter()
                    .all(|child| !child.active || !emitters.contains(&child.parent_entity_index)),
                "source children must hide and reclaim: {lifetime_trace:?}"
            );
            assert!(
                cast.economy().free_slot_count() >= 2,
                "source reclaim must leave capacity for flyer and impact: {lifetime_trace:?}"
            );
            assert_eq!(
                cast.source_arena().handle(actor_slot),
                Some(actor_handle),
                "the authored target must retain its original generation"
            );
            assert_eq!(
                cast.source_arena().handle(alliance.source_entity_index),
                Some(holder_handle),
                "the acquired holder must retain its original generation"
            );
        }
        let castle = cast
            .economy_mut()
            .build_castle_at(
                Position::default(),
                Position::default(),
                player,
                false,
                true,
            )
            .expect("controlled source Castle constructor");
        assert!(
            cast.economy_mut()
                .set_castle_mana_prerequisite_for_test(castle, castle_mana)
        );
        cast.economy.entities[usize::from(player)].set_mana(player_mana);
        {
            let mut shared = cast.shared_spells_mut();
            shared.player_entity.set_mana(player_mana);
            shared.player_entities[0] = shared.player_entity.clone();
            shared.caster.mana = player_mana;
            shared.player_mana = player_mana;
        }
        if free_profile {
            cast.step_outer_turn(
                PlayerCommand {
                    opcode: 0x1e,
                    argument_1: 9,
                    ..PlayerCommand::default()
                },
                0,
            );
        }
        cast.step_outer_turn(
            PlayerCommand {
                opcode: 31,
                argument_1: alliance_ui,
                argument_2: 0,
                ..PlayerCommand::default()
            },
            0,
        );
        if free_profile {
            let physical = cast
                .shared_spells()
                .pickup(alliance.source_entity_index)
                .unwrap();
            assert_eq!(
                physical.mana_regen, 0,
                "source option clears the physical Castle gate"
            );
            assert_eq!(
                physical.max_mana, rule.mana_cost,
                "source option retains the physical cost"
            );
            assert!(
                cast.economy().castle_for_owner(player).unwrap().mana
                    < rule.runtime_castle_mana_limit
            );
        }
        let target = cast.entities()[usize::from(actor_slot)].position;
        let player_position = Position {
            x: target.x,
            y: target.y.wrapping_add(1024),
            z: target.z.wrapping_add(1024),
        };
        cast.set_level2_entity_position(player, player_position)
            .unwrap();
        let (yaw, pitch) =
            crate::level1_mission::projectile_guidance_angles(player_position, target);
        // Exercise the actual PlayerEvents input words, while keeping the
        // combined launch direction aimed at the same authored creature.
        let (aim_yaw, aim_pitch) = if settled { (71_i16, -43_i16) } else { (0, 0) };
        cast.state.yaw = yaw.wrapping_sub(aim_yaw);
        cast.state.display_pitch = pitch.wrapping_sub(aim_pitch);
        cast.state.effective_pitch = pitch.wrapping_sub(aim_pitch);
        // The next flight step derives display pitch from this accumulator;
        // setting the display alone does not aim the actual source player.
        cast.state.pitch_accumulator = pitch.wrapping_sub(aim_pitch);
        cast.state.actual_speed = 0;
        cast.state.target_speed = 0;
        let parent_handle = cast
            .source_arena()
            .handle(player)
            .expect("live player generation");
        let actor_handle = cast
            .source_arena()
            .handle(actor_slot)
            .expect("live actor generation");
        let mut previous_mana = cast.entities()[usize::from(player)].mana;
        let mut launched = false;
        let mut impacted = false;
        let mut allied = false;
        let mut charged = false;
        let mut aimed_from_input = false;
        let mut trace = Vec::new();
        for turn in 0..16 {
            cast.step_outer_turn(
                PlayerCommand {
                    flags: if turn == 0 { 0x10 } else { 0 },
                    argument_6: aim_yaw.cast_unsigned(),
                    argument_8: aim_pitch.cast_unsigned(),
                    ..PlayerCommand::default()
                },
                0,
            );
            if settled && !aimed_from_input {
                if let Some(projectile) = cast.entities().iter().find(|row| {
                    row.active && (row.class, row.model) == (9, 25) && row.source_id == player
                }) {
                    let parent = &cast.entities()[usize::from(player)];
                    assert_eq!(
                        parent.pitch, cast.state.display_pitch,
                        "Alliance must read the player body's current source pitch"
                    );
                    let mut expected = parent.position;
                    crate::move_entity(
                        &mut expected,
                        parent.yaw.wrapping_add(aim_yaw),
                        parent.pitch.wrapping_add(aim_pitch),
                        16_384,
                    );
                    let target = Position {
                        x: u16::from_le_bytes([projectile.record[158], projectile.record[159]]),
                        y: u16::from_le_bytes([projectile.record[160], projectile.record[161]]),
                        z: i16::from_le_bytes([projectile.record[162], projectile.record[163]]),
                    };
                    assert_eq!(
                        target, expected,
                        "Alliance aim must use latched input words"
                    );
                    aimed_from_input = true;
                }
            }
            let products = cast
                .entities()
                .iter()
                .filter(|row| {
                    row.active
                        && row.source_id == player
                        && matches!((row.class, row.model), (9, 25) | (10, 74))
                })
                .map(|row| {
                    (
                        row.index,
                        row.class,
                        row.model,
                        row.position,
                        row.drawing_disabled(),
                    )
                })
                .collect::<Vec<_>>();
            launched |= products.iter().any(|row| (row.1, row.2) == (9, 25));
            impacted |= products.iter().any(|row| (row.1, row.2) == (10, 74));
            let row = &cast.entities()[usize::from(actor_slot)];
            let parent = u16::from_le_bytes([row.record[44], row.record[45]]);
            allied |= row.record[77] == 14
                && parent == player
                && cast.source_arena().creature_parent_binding(actor_handle) == Some(parent_handle);
            let errors = cast
                .dispatch_events()
                .iter()
                .filter(|event| matches!(event, Mc2DispatchEvent::Error { .. }))
                .cloned()
                .collect::<Vec<_>>();
            let current_castle_mana = cast.economy().castle_for_owner(player).map(|row| row.mana);
            if free_profile {
                assert!(
                    current_castle_mana.is_some_and(|mana| mana < rule.runtime_castle_mana_limit),
                    "free-use cast must remain below the ordinary Castle gate"
                );
            }
            let current_player_mana = cast.entities()[usize::from(player)].mana;
            charged |= previous_mana - current_player_mana == rule.mana_cost;
            previous_mana = current_player_mana;
            let holder = cast
                .shared_spells()
                .pickup(alliance.source_entity_index)
                .map(|row| {
                    (
                        row.index,
                        row.active,
                        row.action,
                        row.active_value,
                        row.activation_value,
                        row.source_spell_parent_entity_index(),
                    )
                });
            let sounds = cast
                .economy()
                .events()
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        crate::native_world::Level2EconomyEvent::PreparedSound {
                            entity_index, sound_id: 9, ..
                        } if *entity_index == player
                    )
                })
                .cloned()
                .collect::<Vec<_>>();
            let raw_products = cast
                .entities()
                .iter()
                .filter(|row| {
                    matches!((row.class, row.model), (9, 25) | (10, 74))
                        && (row.active || row.source_id == player)
                })
                .map(|row| {
                    (
                        row.index,
                        row.source_id,
                        row.active,
                        row.class,
                        row.model,
                        row.position,
                        row.source_word_96(),
                        row.drawing_disabled(),
                    )
                })
                .collect::<Vec<_>>();
            let observed_slots = sounds
                .iter()
                .filter_map(|event| match event {
                    crate::native_world::Level2EconomyEvent::PreparedSound {
                        source_entity_index,
                        ..
                    } => Some(*source_entity_index),
                    _ => None,
                })
                .chain(raw_products.iter().map(|row| row.0))
                .collect::<Vec<_>>();
            let visits = cast
                .cf10_production_schedule_receipts()
                .into_iter()
                .filter(|receipt| {
                    matches!(
                        receipt,
                        crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, .. }
                            if observed_slots.contains(slot)
                    )
                })
                .collect::<Vec<_>>();
            let generations = observed_slots
                .iter()
                .map(|slot| (*slot, cast.source_arena().handle(*slot)))
                .collect::<Vec<_>>();
            let population = (
                cast.entities().iter().filter(|row| row.active).count(),
                cast.economy()
                    .authored_model53()
                    .iter()
                    .filter(|source| source.active)
                    .count(),
                cast.economy()
                    .model17_spawned_children()
                    .iter()
                    .filter(|child| child.active)
                    .count(),
            );
            trace.push((
                turn,
                row.position,
                row.action,
                row.record[77],
                parent,
                current_castle_mana,
                current_player_mana,
                products,
                errors,
                (
                    cast.economy().free_slot_count(),
                    holder,
                    sounds,
                    raw_products,
                    visits,
                    generations,
                    population,
                ),
            ));
            if launched && impacted && allied {
                break;
            }
        }
        if admitted {
            assert!(
                launched && aimed_from_input && impacted && allied,
                "controlled at-gate Alliance must reach the live model18: launched={launched}, aimed={aimed_from_input}, impact={impacted}, allied={allied}, trace={trace:?}"
            );
            if free_profile {
                assert!(
                    charged,
                    "free-use holder still charges its physical cost: {trace:?}"
                );
                continue;
            }
            let actor = &cast.entities()[usize::from(actor_slot)];
            let presentation = cast
                .campaign_entity_presentations(cast.state.yaw)
                .expect("the acquired Alliance actor reaches presentation")
                .into_iter()
                .find(|row| row.entity_index == actor_slot)
                .expect("the allied actor is present in the source sprite census");
            assert_eq!(presentation.position, actor.position);
            assert_eq!(presentation.control_flags, actor.control_flags);

            // A real second pickup and player Fireball collision select a
            // hostile for this same acquired ally. The creature must consume
            // that player-selected source target and produce its own shot.
            let mut combat = cast.clone();
            let hostile_slot = combat
                .entities()
                .iter()
                .find(|row| row.active && row.class == 5 && row.model == 21 && row.life > 0)
                .expect("the authored group retains a living model21 opponent")
                .index;
            combat
                .set_level2_entity_position(player, fireball.position)
                .unwrap();
            let mut acquired_fireball = combat
                .shared_spells()
                .players()
                .iter()
                .find(|row| row.entity_index == player)
                .is_some_and(|row| row.enabled_spell_holders[0] == fireball.source_entity_index);
            for _ in 0..4 {
                if acquired_fireball {
                    break;
                }
                combat.step_outer_turn(PlayerCommand::default(), 0);
                acquired_fireball |= combat.dispatch_events().iter().any(|event| {
                    matches!(
                        event,
                        Mc2DispatchEvent::SpellAcquisition {
                            family: 0,
                            source_slot,
                            result: Some(_),
                            ..
                        } if *source_slot == fireball.source_entity_index
                    )
                });
                if acquired_fireball {
                    break;
                }
            }
            assert!(
                acquired_fireball,
                "the allied caster must acquire the real Fireball pickup"
            );
            let fireball_ui = crate::SOURCE_SPELL_INDEX_MAP
                .iter()
                .position(|family| *family == 0)
                .expect("Fireball has a source menu entry") as i8;
            combat.step_outer_turn(
                PlayerCommand {
                    opcode: 31,
                    argument_1: fireball_ui,
                    ..PlayerCommand::default()
                },
                0,
            );
            let ally_position = combat.entities()[usize::from(actor_slot)].position;
            // Pick a nearby east/west approach with the lowest sampled owner
            // terrain. East/west also keeps the player's northbound Fireball
            // clear of the ally. The previous long offset fired into a ridge.
            let mut hostile_position = [(768_i16, 0_i16), (-768, 0)]
                .into_iter()
                .map(|(dx, dy)| {
                    let peak = (1..=3)
                        .map(|step| {
                            let sample = Position {
                                x: ally_position.x.wrapping_add((dx * step / 3) as u16),
                                y: ally_position.y.wrapping_add((dy * step / 3) as u16),
                                z: ally_position.z,
                            };
                            combat.canonical_terrain().ground_height(sample)
                        })
                        .max()
                        .unwrap();
                    (peak, dx, dy)
                })
                .min_by_key(|(peak, _, _)| *peak)
                .map(|(_, dx, dy)| Position {
                    x: ally_position.x.wrapping_add(dx as u16),
                    y: ally_position.y.wrapping_add(dy as u16),
                    z: ally_position.z,
                })
                .unwrap();
            hostile_position.z = combat.canonical_terrain().ground_height(hostile_position);
            combat
                .set_level2_entity_position(hostile_slot, hostile_position)
                .unwrap();
            let hostile_handle = combat.source_arena().handle(hostile_slot).unwrap();
            let player_position = Position {
                x: hostile_position.x.wrapping_add(256),
                y: hostile_position.y.wrapping_add(1024),
                z: hostile_position.z.wrapping_add(1024),
            };
            combat
                .set_level2_entity_position(player, player_position)
                .unwrap();
            combat.state.yaw = 0;
            combat.state.roll_accumulator = 0;
            combat.state.pitch_accumulator = 0;
            combat.state.display_pitch = 0;
            combat.state.effective_pitch = 0;
            combat.state.actual_speed = 0;
            combat.state.target_speed = 0;
            let mut launch_position = player_position;
            launch_position.x = launch_position.x.wrapping_sub(256);
            launch_position.z = launch_position.z.wrapping_add(combat.state.entity_fov);
            let (_, fireball_pitch) = crate::level1_mission::projectile_guidance_angles(
                launch_position,
                hostile_position,
            );
            let mut selected_hostile = false;
            let mut ally_selected_hostile = false;
            let mut ally_fired = false;
            let mut ally_impact = false;
            let mut ally_mailbox = false;
            let mut ally_damage_consumed = false;
            let mut ally_receipts = Vec::new();
            let mut impact_context = Vec::new();
            for turn in 0..256 {
                let previous_life = combat.entities()[usize::from(hostile_slot)].life;
                crate::native_world::area_damage::start_effect_damage_trace(hostile_slot);
                combat.step_outer_turn(
                    PlayerCommand {
                        flags: if turn == 0 { 0x10 } else { 0 },
                        argument_8: if turn == 0 {
                            fireball_pitch.cast_unsigned()
                        } else {
                            0
                        },
                        ..PlayerCommand::default()
                    },
                    0,
                );
                let receipts = crate::native_world::area_damage::take_effect_damage_trace();
                for receipt in receipts
                    .into_iter()
                    .filter(|receipt| receipt.pulse.owner == actor_slot)
                {
                    if impact_context.len() < 4 {
                        let position = receipt.pulse.position;
                        let nearby = combat
                            .entities()
                            .iter()
                            .filter(|row| {
                                row.active
                                    && i32::from(row.position.x.wrapping_sub(position.x) as i16)
                                        .abs()
                                        < 800
                                    && i32::from(row.position.y.wrapping_sub(position.y) as i16)
                                        .abs()
                                        < 800
                            })
                            .map(|row| {
                                (
                                    row.index,
                                    row.class,
                                    row.model,
                                    row.source_id,
                                    row.position,
                                    row.source_word_96(),
                                    row.life,
                                )
                            })
                            .collect::<Vec<_>>();
                        impact_context.push((
                            position,
                            combat.canonical_terrain().ground_height(position),
                            nearby,
                        ));
                    }
                    ally_receipts.push(receipt);
                }
                let caster = &combat.entities()[usize::from(player)];
                let ally = &combat.entities()[usize::from(actor_slot)];
                selected_hostile |= put_word(caster, 154) == hostile_slot;
                ally_selected_hostile |= selected_hostile
                    && ally.record[77] == 14
                    && put_word(ally, 44) == player
                    && put_word(ally, 154) == hostile_slot;
                ally_fired |= combat.economy().fireball_projectiles().iter().any(|shot| {
                    shot.origin == crate::native_world::FireballProjectileOrigin::Creature18
                        && shot.owner_entity_index == actor_slot
                        && shot.target_source_entity_index == hostile_slot
                });
                ally_impact |= combat.entities().iter().any(|effect| {
                    effect.active
                        && (effect.class, effect.model, effect.source_id) == (10, 0, actor_slot)
                        && effect.source_word_96() == hostile_slot
                });
                ally_mailbox |= ally_receipts.iter().any(|receipt| {
                    receipt.pulse.payload == RULES.projectile_damage
                        && receipt
                            .mailbox_after
                            .is_some_and(|(damage, attacker)| damage > 0 && attacker == actor_slot)
                });
                let hostile = &combat.entities()[usize::from(hostile_slot)];
                ally_damage_consumed |= combat.source_arena().handle(hostile_slot)
                    == Some(hostile_handle)
                    && hostile.life < previous_life
                    && put_word(hostile, 42) == actor_slot;
                if selected_hostile
                    && ally_selected_hostile
                    && ally_fired
                    && ally_impact
                    && ally_mailbox
                    && ally_damage_consumed
                {
                    break;
                }
            }
            assert!(
                selected_hostile
                    && ally_selected_hostile
                    && ally_fired
                    && ally_impact
                    && ally_mailbox
                    && ally_damage_consumed,
                "real Fireball target must drive the acquired ally's damaging attack: player={selected_hostile}, ally={ally_selected_hostile}, fired={ally_fired}, impact={ally_impact}, mailbox={ally_mailbox}, consumed={ally_damage_consumed}, caster_target={}, ally={:?}, hostile={:?}, impact_context={impact_context:?}, receipts={ally_receipts:?}, errors={:?}",
                put_word(&combat.entities()[usize::from(player)], 154),
                &combat.entities()[usize::from(actor_slot)],
                &combat.entities()[usize::from(hostile_slot)],
                combat
                    .dispatch_events()
                    .iter()
                    .filter(|event| matches!(event, Mc2DispatchEvent::Error { .. }))
                    .collect::<Vec<_>>()
            );

            // Keep the real caster out of the nearby combat while the same
            // acquired creature runs its source Alliance lifetime. No owner,
            // target, action, or timer is manufactured by this check.
            let remaining = i16::from_le_bytes([actor.record[50], actor.record[51]]);
            let mut safe_player = cast.entities()[usize::from(player)].position;
            safe_player.x = safe_player.x.wrapping_add(0x6000);
            cast.set_level2_entity_position(player, safe_player)
                .unwrap();
            let mut saw_countdown = false;
            let mut expired = false;
            for _ in 0..160 {
                cast.step_outer_turn(PlayerCommand::default(), 2);
                let actor = &cast.entities()[usize::from(actor_slot)];
                assert_eq!(cast.source_arena().handle(actor_slot), Some(actor_handle));
                assert!(cast.entities()[usize::from(player)].life > 0);
                let left = i16::from_le_bytes([actor.record[50], actor.record[51]]);
                if actor.record[77] == 14 {
                    assert_eq!(put_word(actor, 44), player);
                    assert_ne!(
                        put_word(actor, 154),
                        player,
                        "an ally must not target its caster"
                    );
                    saw_countdown |= left < remaining;
                } else {
                    assert!(left <= 0, "Alliance must expire through its source timer");
                    assert_eq!(put_word(actor, 44), 0);
                    assert_eq!(actor.control_flags & 0x0600_0000, 0);
                    expired = true;
                    break;
                }
            }
            assert!(
                saw_countdown && expired,
                "acquired Alliance must count down and expire"
            );
        } else {
            assert!(
                !launched && !impacted && !allied,
                "immediate controlled cast must not launch or take over: trace={trace:?}"
            );
            if castle_mana == rule.runtime_castle_mana_limit && player_mana == rule.mana_cost {
                assert!(
                    charged,
                    "the saturated source cast must still charge mana: trace={trace:?}"
                );
            }
        }
    }
    // Fire once through the actual selected holder and starting player mana.
    // The paired branch receives the same movement and source turns without
    // the fire input; neither branch edits creature or inventory state.
    let fire_actor_handle = pickup_near.source_arena().handle(actor_slot).unwrap();
    let fire_holder_handle = pickup_near
        .source_arena()
        .handle(fireball.source_entity_index)
        .unwrap();
    let emitters = pickup_near
        .economy()
        .authored_model53()
        .iter()
        .filter(|source| source.active)
        .map(|source| source.source_entity_index)
        .collect::<Vec<_>>();
    assert!(
        !emitters.is_empty(),
        "authored group must produce model53 sources"
    );
    let mut wait_position = pickup_near.entities()[usize::from(actor_slot)].position;
    wait_position.x = wait_position.x.wrapping_add(0x6000);
    pickup_near
        .set_level2_entity_position(player, wait_position)
        .unwrap();
    let mut reclaim_trace = Vec::new();
    for _ in 0..32 {
        let emitting = pickup_near
            .economy()
            .authored_model53()
            .iter()
            .filter(|source| source.active && source.life >= 0)
            .count();
        let children = pickup_near
            .economy()
            .model17_spawned_children()
            .iter()
            .filter(|child| child.active && emitters.contains(&child.parent_entity_index))
            .count();
        let free = pickup_near.economy().free_slot_count();
        reclaim_trace.push((emitting, children, free));
        if emitting == 0 && children == 0 && free >= 2 {
            break;
        }
        pickup_near.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert!(
        pickup_near
            .economy()
            .authored_model53()
            .iter()
            .all(|source| !source.active || source.life < 0)
            && pickup_near
                .economy()
                .model17_spawned_children()
                .iter()
                .all(|child| !child.active || !emitters.contains(&child.parent_entity_index))
            && pickup_near.economy().free_slot_count() >= 2,
        "ordinary emission and reclaim must leave room for Fireball and impact: {reclaim_trace:?}"
    );
    assert_eq!(
        pickup_near.source_arena().handle(actor_slot),
        Some(fire_actor_handle)
    );
    assert_eq!(
        pickup_near
            .source_arena()
            .handle(fireball.source_entity_index),
        Some(fire_holder_handle)
    );
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
        y: actor.position.y.wrapping_add(1024),
        z: actor.position.z.wrapping_add(1024),
    };
    pickup_near
        .set_level2_entity_position(player, player_position)
        .unwrap();
    pickup_near.state.yaw = 0;
    pickup_near.state.display_pitch = 0;
    pickup_near.state.effective_pitch = 0;
    pickup_near.state.pitch_accumulator = 0;
    pickup_near.state.actual_speed = 0;
    pickup_near.state.target_speed = 0;
    let mut launch_position = player_position;
    launch_position.x = launch_position.x.wrapping_sub(256);
    launch_position.z = launch_position.z.wrapping_add(pickup_near.state.entity_fov);
    let (_, fireball_pitch) =
        crate::level1_mission::projectile_guidance_angles(launch_position, actor.position);
    let mut no_fire = pickup_near.clone();
    let life_before = actor.life;
    let mut player_projectile = false;
    let mut actor_hit = false;
    let mut shot_trace = Vec::new();
    for turn in 0..16 {
        pickup_near.step_outer_turn(
            PlayerCommand {
                flags: if turn == 0 { 0x10 } else { 0 },
                argument_8: fireball_pitch.cast_unsigned(),
                ..PlayerCommand::default()
            },
            0,
        );
        let shots = pickup_near
            .economy()
            .fireball_projectiles()
            .iter()
            .filter(|shot| shot.owner_entity_index == player)
            .copied()
            .collect::<Vec<_>>();
        player_projectile |= !shots.is_empty();
        let row = &pickup_near.entities()[usize::from(actor_slot)];
        actor_hit |= row.life < life_before
            && u16::from_le_bytes([row.record[42], row.record[43]]) == player;
        let errors = pickup_near
            .dispatch_events()
            .iter()
            .filter(|event| matches!(event, Mc2DispatchEvent::Error { .. }))
            .cloned()
            .collect::<Vec<_>>();
        let impacts = pickup_near
            .entities()
            .iter()
            .filter(|effect| effect.active && (effect.class, effect.model) == (10, 0))
            .map(|effect| {
                (
                    effect.index,
                    effect.source_id,
                    effect.source_word_96(),
                    effect.position,
                    effect.drawing_disabled(),
                )
            })
            .collect::<Vec<_>>();
        shot_trace.push((
            turn,
            row.position,
            row.life,
            pickup_near.economy().free_slot_count(),
            shots,
            impacts,
            errors,
        ));
        no_fire.step_outer_turn(
            PlayerCommand {
                argument_8: fireball_pitch.cast_unsigned(),
                ..PlayerCommand::default()
            },
            0,
        );
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
        "owner Fireball must reach the authored model18 through real input: projectile={player_projectile}, hit={actor_hit}, trace={shot_trace:?}"
    );
    assert_eq!(
        put_word(&pickup_near.entities()[usize::from(player)], 154),
        actor_slot,
        "the successful source impact must publish the player's selected target"
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
