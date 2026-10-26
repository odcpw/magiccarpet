use super::*;
use crate::native_world::{
    ParticleParameter, PlayerSnapshot, RuntimeRelation, RuntimeSnapshot, RuntimeStageVariable,
};

fn terrain() -> SessionTerrain {
    let session = crate::native_world::tests::level_two_campaign_session();
    session.canonical_terrain().clone()
}

fn position(x: u16) -> Position {
    Position {
        x,
        y: 20_000,
        z: 5_000,
    }
}

fn actor_row(slot: u16, action: u8, position: Position) -> EntitySnapshot {
    let mut row = crate::native_world::cave_features::new_event(slot, 19, position);
    row.class = 5;
    row.model = SOURCE_MODEL;
    row.source_id = slot;
    row.set_source_action(action);
    row.set_control_flags(row.control_flags | 0x800);
    row
}

fn player_target(slot: u16, position: Position) -> Target {
    Target {
        handle: SourceEntityHandle {
            source_slot: slot,
            generation: 1,
        },
        slot,
        source_id: slot,
        class: 3,
        model: 0,
        action: 0,
        life: 500,
        flags: 0,
        position,
        yaw: 0,
        roll: 0,
        player_entity: slot,
        mana: 500,
        mana_regen: 9,
        left_spell: None,
        right_spell: None,
        target: 0,
        next: 0,
        link: 0,
        attacker: 0,
        killer: 0,
        actual_speed: 0,
        maximum_speed: 0,
    }
}

fn theft_roll_seed(value: u32) -> u32 {
    (0_u32..63)
        .find(|seed| seed.wrapping_mul(9377).wrapping_add(9439) % 63 == value)
        .expect("the source LCG reaches each theft selector")
}

fn run_model26_action(
    row: &mut EntitySnapshot,
    terrain: &SessionTerrain,
    runtime: &RuntimeSnapshot,
    targets: &[Target],
    class3: &[u16],
    model26: &[u16],
) -> Effects {
    let mut global_rng = 0;
    let source_slot = row.index;
    let mut actor = Actor {
        row,
        handle: SourceEntityHandle {
            source_slot,
            generation: 1,
        },
    };
    let mut context = Context {
        rules: &RULES,
        terrain,
        targets,
        class3,
        model26,
        runtime: Some(runtime),
        global_rng: &mut global_rng,
    };
    step(&mut actor, &mut context).unwrap()
}

fn equip_right_spell(
    player: &mut PlayerSnapshot,
    player_entity: u16,
    family: u8,
    holder_slot: u16,
) {
    if player.entity_index != player_entity {
        return;
    }
    let family_index = usize::from(family);
    assert!(player.set_enabled_spell_holder_index(family_index, holder_slot));
    player.left_spell_slot_raw = -1;
    player.left_spell_slot = 0;
    player.record[554..556].copy_from_slice(&(-1_i16).to_le_bytes());
    player.right_spell_slot_raw = i16::from(family);
    player.right_spell_slot = family;
    player.record[556..558].copy_from_slice(&i16::from(family).to_le_bytes());
}

#[test]
fn constructor_matches_source_orientation_and_particle_extents() {
    let particle = ParticleParameter {
        base_tmap: 0,
        word_2: 0,
        word_4: 0,
        speed: 90,
        rotation_speed: 52,
        byte_10: 0,
        byte_11: 0,
        render_mode: 0,
        byte_13: 0,
    };
    let particles = vec![particle; usize::from(PARTICLE) + 1];
    let seed = 0x1234_u16;
    let rng = seed.wrapping_mul(9377).wrapping_add(9439);
    let expected_rotation = ((rng & 0x07ff).wrapping_sub(1)) as i16;
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let mut runtime = simulation.session.state.live.core.runtime.clone();
    runtime.bytes[crate::native_world::RUNTIME_BYTES + usize::from(SOURCE_MODEL)] = 17;

    let requested_position = position(20_000);
    let row = mc2::construct_row(7, seed, requested_position, &mut runtime, &particles).unwrap();

    assert_eq!(
        (row.class, row.model, row.action),
        (5, SOURCE_MODEL, ROAM_ACTION)
    );
    assert_eq!((row.max_life, row.life, row.mana), (4400, 4400, 2200));
    assert_eq!(
        (row.yaw, row.pitch, row.source_roll(), row.fov),
        (expected_rotation, expected_rotation, expected_rotation, 0)
    );
    assert_eq!(row.entity_rng, rng);
    assert_eq!(
        row.position, requested_position,
        "sub_4CF00 keeps the placed height"
    );
    assert_eq!(row.phase, 17);
    assert_eq!(
        runtime.bytes[crate::native_world::RUNTIME_BYTES + usize::from(SOURCE_MODEL)],
        18
    );
    assert_eq!(
        (row.particle_parameter_index, row.record[97]),
        (Some(PARTICLE), particle.animation_limit().unwrap())
    );
    assert_eq!(
        (
            row.array_yaw(),
            row.array_pitch(),
            row.array_roll(),
            row.array_fov()
        ),
        (26, 256, 256, 384)
    );
    assert_ne!(row.control_flags & 0x800000, 0);
}

#[test]
fn drain_applies_source_mana_amount_and_steals_the_selected_unheld_spell() {
    let terrain = terrain();
    let mut row = actor_row(1, DRAIN_ACTION, position(20_000));
    put(&mut row, 154, 2);
    let thief = SourceEntityHandle {
        source_slot: 1,
        generation: 2,
    };
    let holder = SourceEntityHandle {
        source_slot: 8,
        generation: 4,
    };
    let mut target = player_target(2, position(21_000));
    target.right_spell = Some(SpellOffer {
        family: 4,
        holder,
        holder_thief: 0,
    });
    target.left_spell = Some(SpellOffer {
        family: 5,
        holder,
        holder_thief: 0,
    });
    let targets = [target];
    let class3 = [2];
    let mut global_rng = theft_roll_seed(4);
    let mut actor = Actor {
        row: &mut row,
        handle: thief,
    };
    let effects = {
        let mut context = Context {
            rules: &RULES,
            terrain: &terrain,
            targets: &targets,
            class3: &class3,
            model26: &[],
            runtime: None,
            global_rng: &mut global_rng,
        };
        step(&mut actor, &mut context).unwrap()
    };

    assert_eq!(
        effects.mana_drain,
        Some(ManaDrain {
            target: target.handle,
            amount: 23
        })
    );
    assert_eq!(
        effects.spell_theft,
        Some(SpellTheft {
            thief,
            victim: target.handle,
            victim_player: 2,
            holder,
            family: 4,
            hand: SpellHand::Right,
            victim_position: target.position,
        })
    );
    assert_eq!(
        actor.row.action, DRAIN_ACTION,
        "the source early return keeps action210"
    );
    assert_eq!(actor.row.position, position(20_000));
    assert_eq!(global_rng % 63, 4);
}

#[test]
fn dynamic_theft_publishes_current_holder_and_uses_detached_return_consumer() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let mut arena = simulation.session.state.live.economy.clone();
    let mut shared = simulation.session.state.live.shared_spells.clone();
    let mut terrain = simulation.session.state.live.terrain.clone();
    let mut runtime = simulation.session.state.live.core.runtime.clone();

    let victim_slot = shared.player_entity.index;
    let victim = arena.entities[usize::from(victim_slot)].clone();
    assert!(victim.active && victim.class == 3 && victim.model == 0);
    let victim_player = victim.source_player_entity_index();
    assert!(
        shared
            .players
            .iter()
            .any(|player| player.entity_index == victim_player)
            || shared.local_player.entity_index == victim_player
    );
    let family = 4;
    let holder_action = family * 3;
    let pickup = arena
        .economy
        .allocate_spell_pickup(0, family, holder_action + 1, victim.position)
        .expect("the source allocator has a free dynamic holder slot");
    let holder_slot = pickup.source_entity_index;
    assert!(
        !arena.entities[usize::from(holder_slot)].active,
        "the setup exercises a current typed holder without a raw row"
    );
    assert!(arena.economy.sync_spell_pickup_projection(
        holder_slot,
        holder_action,
        victim.position
    ));
    let mut holder = crate::native_world::cave_features::new_event(
        holder_slot,
        pickup.entity_rng,
        victim.position,
    );
    holder.class = 15;
    holder.model = family;
    holder.source_id = holder_slot;
    holder.set_entity_rng(pickup.entity_rng);
    holder.set_source_action(holder_action);
    holder.set_control_flags(5);
    holder.set_phase(pickup.phase);
    holder.record[30..32].copy_from_slice(&holder_slot.to_le_bytes());
    holder.record[44..46].copy_from_slice(&victim_slot.to_le_bytes());
    holder.record[58..60].fill(0);
    shared.dynamic_pickups.push(holder);
    for player in &mut shared.players {
        equip_right_spell(player, victim_player, family, holder_slot);
    }
    equip_right_spell(&mut shared.local_player, victim_player, family, holder_slot);

    let thief_slot = arena
        .allocate_source_slot()
        .expect("the source allocator has a thief slot");
    let mut thief = actor_row(thief_slot, DRAIN_ACTION, position(20_000));
    thief.set_entity_rng(arena.new_event_rng(thief_slot));
    thief.set_life(100);
    arena.entities[usize::from(thief_slot)] = thief;
    let theft = SpellTheft {
        thief: arena.handle(thief_slot).expect("live model26 handle"),
        victim: arena.handle(victim_slot).expect("live player handle"),
        victim_player,
        holder: arena
            .handle(holder_slot)
            .expect("live dynamic holder handle"),
        family,
        hand: SpellHand::Right,
        victim_position: victim.position,
    };

    let mut occupied_arena = arena.clone();
    let mut occupied_shared = shared.clone();
    let mut occupant = actor_row(holder_slot, ROAM_ACTION, position(22_000));
    occupant.set_entity_rng(pickup.entity_rng.wrapping_add(1));
    let occupant_before = occupant.clone();
    occupied_arena.entities[usize::from(holder_slot)] = occupant;
    assert!(!mc2::apply_spell_theft(
        &mut occupied_arena,
        &mut occupied_shared,
        theft
    ));
    assert_eq!(
        occupied_arena.entities[usize::from(holder_slot)],
        occupant_before,
        "a current typed holder cannot overwrite a different raw occupant"
    );
    assert_eq!(
        occupied_shared.local_player.enabled_spell_holders[usize::from(family)],
        holder_slot,
        "a rejected identity leaves the player hand unchanged"
    );

    assert!(mc2::apply_spell_theft(&mut arena, &mut shared, theft));
    let stolen = &arena.entities[usize::from(holder_slot)];
    assert!(stolen.active);
    assert_eq!(
        (stolen.class, stolen.model, stolen.action),
        (15, family, 78)
    );
    assert_eq!(word(stolen, 42), thief_slot);
    assert_eq!(word(stolen, 74), 1);
    assert_eq!(stolen.position, victim.position);
    assert_eq!(
        shared.local_player.enabled_spell_holders[usize::from(family)],
        0
    );
    assert_eq!(shared.local_player.right_spell_slot_raw, -1);
    assert_eq!(arena.handle(holder_slot), Some(theft.holder));

    arena.entities[usize::from(thief_slot)].set_life(-1);
    for _ in 0..7 {
        crate::native_world::support_spells::step_detached_source_slot(
            &mut shared,
            &mut arena,
            &mut terrain,
            holder_slot,
        )
        .expect("the actual action78 source consumer advances the stolen holder");
        if arena.entities[usize::from(holder_slot)].action != 78 {
            break;
        }
    }
    let returned = arena.entities[usize::from(holder_slot)].clone();
    assert_eq!(returned.action, holder_action + 1);
    assert_eq!(word(&returned, 42), 0);
    mc2::prepare_source_pass(&mut arena, &mut runtime, &[]);
    let typed = arena
        .economy
        .spell_pickups()
        .iter()
        .find(|source| source.active && source.source_entity_index == holder_slot)
        .expect("the dynamic holder remains in the same source generation");
    assert_eq!(typed.action, returned.action);
    assert_eq!(typed.position, returned.position);
    assert_eq!(typed.phase, returned.phase);
    assert_eq!(arena.handle(holder_slot), Some(theft.holder));
}

#[test]
fn drain_does_not_steal_a_spell_holder_already_linked_to_another_thief() {
    let terrain = terrain();
    let mut row = actor_row(1, DRAIN_ACTION, position(20_000));
    put(&mut row, 154, 2);
    let holder = SourceEntityHandle {
        source_slot: 8,
        generation: 4,
    };
    let mut target = player_target(2, position(21_000));
    target.right_spell = Some(SpellOffer {
        family: 4,
        holder,
        holder_thief: 12,
    });
    let targets = [target];
    let class3 = [2];
    let mut global_rng = theft_roll_seed(4);
    let mut actor = Actor {
        row: &mut row,
        handle: SourceEntityHandle {
            source_slot: 1,
            generation: 2,
        },
    };
    let mut context = Context {
        rules: &RULES,
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model26: &[],
        runtime: None,
        global_rng: &mut global_rng,
    };

    let effects = step(&mut actor, &mut context).unwrap();

    assert_eq!(
        effects.mana_drain,
        Some(ManaDrain {
            target: target.handle,
            amount: 23
        })
    );
    assert_eq!(effects.spell_theft, None);
    assert_eq!(actor.row.action, DRAIN_ACTION);
    assert_eq!(actor.row.position, position(20_000));
}

#[test]
fn prekill_propagates_child_action_and_returns_source_kill_credit_slot() {
    let terrain = terrain();
    let mut row = actor_row(1, PREKILL_ACTION, position(20_000));
    put(&mut row, 56, 3);
    let mut child = player_target(3, position(20_100));
    child.class = 5;
    child.model = SOURCE_MODEL;
    child.killer = 2;
    let player = player_target(2, position(21_000));
    let targets = [child, player];
    let class3 = [2];
    let mut global_rng = 0;
    let mut actor = Actor {
        row: &mut row,
        handle: SourceEntityHandle {
            source_slot: 1,
            generation: 2,
        },
    };
    let mut context = Context {
        rules: &RULES,
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model26: &[1, 3],
        runtime: None,
        global_rng: &mut global_rng,
    };

    let effects = step(&mut actor, &mut context).unwrap();

    assert_eq!(effects.kill_credit, Some(2));
    assert_eq!(effects.peer_updates.len(), 1);
    assert_eq!(effects.peer_updates[0].action, Some(KILL_ACTION));
    assert_eq!(actor.row.action, KILL_ACTION);
}

#[test]
fn action214_is_rejected_as_unsupported_source_dispatch() {
    let terrain = terrain();
    let mut row = actor_row(1, 214, position(20_000));
    let mut global_rng = 0;
    let mut actor = Actor {
        row: &mut row,
        handle: SourceEntityHandle {
            source_slot: 1,
            generation: 2,
        },
    };
    let mut context = Context {
        rules: &RULES,
        terrain: &terrain,
        targets: &[],
        class3: &[],
        model26: &[],
        runtime: None,
        global_rng: &mut global_rng,
    };

    let error = step(&mut actor, &mut context).unwrap_err();

    assert!(error.contains("unsupported model26 action214"));
}

#[test]
fn action215_turns_toward_its_bound_stage_point() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = simulation.session.state.live.terrain.clone();
    let mut runtime = simulation.session.state.live.core.runtime.clone();
    runtime.stage_variables[1] = RuntimeStageVariable {
        kind: 1,
        flags: 0,
        word_2: 0,
        relation: RuntimeRelation::AxisScalar((20_000_u32 << 16) | 30_000),
    };
    let mut row = actor_row(1, STAGE_ACTION, position(20_000));
    row.record[76] = 1;
    row.record[77] = 1;
    row.set_source_roll(777);
    let effects = run_model26_action(&mut row, &terrain, &runtime, &[], &[], &[1]);

    assert_eq!(row.source_roll(), 0);
    assert_eq!(effects.population_decrements, 0);

    let player = player_target(2, position(21_000));
    let targets = [player];
    let class3 = [2];
    let model26 = [1];

    let mut hit = actor_row(1, STAGE_ACTION, position(20_000));
    hit.max_life = 4_400;
    hit.life = 4_400;
    hit.record[76] = 1;
    hit.record[77] = 1;
    hit.record[98..102].copy_from_slice(&100_i32.to_le_bytes());
    put(&mut hit, 102, player.slot);
    let effects = run_model26_action(&mut hit, &terrain, &runtime, &targets, &class3, &model26);

    assert_eq!(hit.life, 4_300);
    assert_eq!(hit.action, DRAIN_ACTION);
    assert_eq!(hit.record[77], 10);
    assert_eq!(word(&hit, 154), player.slot);
    assert_eq!(effects.population_decrements, 0);

    let mut same_model_attacker = player_target(3, position(21_000));
    same_model_attacker.class = 5;
    same_model_attacker.model = SOURCE_MODEL;
    let same_model_targets = [same_model_attacker];
    let same_model_slots = [1, same_model_attacker.slot];
    let mut same_model_hit = actor_row(1, STAGE_ACTION, position(20_000));
    same_model_hit.max_life = 4_400;
    same_model_hit.life = 4_400;
    same_model_hit.record[76] = 1;
    same_model_hit.record[77] = 1;
    same_model_hit.record[98..102].copy_from_slice(&100_i32.to_le_bytes());
    put(&mut same_model_hit, 102, same_model_attacker.slot);
    let effects = run_model26_action(
        &mut same_model_hit,
        &terrain,
        &runtime,
        &same_model_targets,
        &[],
        &same_model_slots,
    );

    assert_eq!(same_model_hit.action, STAGE_ACTION);
    assert_eq!(same_model_hit.record[77], 1);
    assert_eq!(effects.population_decrements, 0);

    let mut fatal = actor_row(1, STAGE_ACTION, position(20_000));
    fatal.life = 50;
    fatal.record[76] = 1;
    fatal.record[77] = 1;
    fatal.record[98..102].copy_from_slice(&100_i32.to_le_bytes());
    put(&mut fatal, 102, player.slot);
    let effects = run_model26_action(&mut fatal, &terrain, &runtime, &targets, &class3, &model26);

    assert_eq!(fatal.life, -50);
    assert_eq!(fatal.action, PREKILL_ACTION);
    assert_eq!(effects.population_decrements, 1);

    let mut early_return_fatal = actor_row(1, STAGE_ACTION, position(20_000));
    early_return_fatal.life = 50;
    early_return_fatal.record[76] = 1;
    early_return_fatal.record[77] = 2;
    early_return_fatal.record[98..102].copy_from_slice(&100_i32.to_le_bytes());
    put(&mut early_return_fatal, 102, player.slot);
    let effects = run_model26_action(
        &mut early_return_fatal,
        &terrain,
        &runtime,
        &targets,
        &class3,
        &model26,
    );

    assert_eq!(early_return_fatal.action, PREKILL_ACTION);
    assert_eq!(effects.population_decrements, 1);
}
