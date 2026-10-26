use super::*;
use crate::native_world::ENTITY_DRAW_DISABLED_BIT;
use crate::world::effects::DamagePulse;
use std::collections::BTreeSet;

fn ready_lightning(tier: u8) -> (super::super::Mc2Simulation, u16, u16) {
    let mut session = super::super::tests::level_two_campaign_session();
    let live = &mut session.session.state.live;
    let parent = live.core.state.entity_index;
    let slot = live
        .shared_spells
        .pickups()
        .iter()
        .find(|row| row.model == 3)
        .expect("campaign fixture has the existing spell pickup")
        .index;
    {
        let holder = live.shared_spells.pickup_mut(slot).unwrap();
        holder.model = 7;
        holder.record[68] = 7;
        holder.set_source_action(21);
        holder.active_value = 0;
        put(holder, 50, 0);
        holder.apply_nyphur_spell_profile(
            crate::spell_activation::level2_set_spell_profile(7, tier)
                .expect("Lightning tier has a source profile"),
            tier,
        );
        let activation = holder.activation_value;
        holder.active_value = activation as i16;
        put(holder, 50, activation);
        put(holder, 44, parent);
        holder.set_control_flags(holder.control_flags & !ENTITY_DRAW_DISABLED_BIT);
    }
    let max_mana = live.shared_spells.pickup(slot).unwrap().max_mana;
    let player = &mut live.economy.entities[usize::from(parent)];
    player.class = 3;
    player.model = 0;
    player.record[67] = 3;
    player.record[68] = 0;
    player.set_mana(max_mana.max(RULES.tiers[usize::from(tier)].mana_cost));
    player.set_mana_regen(0);
    (session, slot, parent)
}

fn exhaust(arena: &mut SourceArena) {
    for row in &mut arena.entities {
        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    }
    arena.economy.exhaust_new_event_capacity_for_test();
    arena.rebuild_source_free_arena();
}

fn next_rng(rng: &mut u16) -> u16 {
    *rng = rng.wrapping_mul(9377).wrapping_add(9439);
    *rng
}

fn expected_one_step_ray_rng(seed: u16) -> u16 {
    let mut rng = seed;
    let mut bends = [0_i16; 2];
    for remaining in (0..=8).rev() {
        let width = (remaining / 2).clamp(0, 8) as i16;
        for bend in &mut bends {
            *bend = if *bend > width {
                bend.wrapping_sub(1)
            } else if *bend < -width {
                bend.wrapping_add(1)
            } else {
                2 * ((next_rng(&mut rng) % 157) / 79) as i16 - 1 + *bend
            };
        }
    }
    rng
}

#[test]
fn lightning_uses_source_turn_kernel_and_ray_accuracy_identity() {
    assert_eq!(sub58350_delta(0, 2047, 5), -1);
    assert_eq!(sub58350_delta(2047, 0, 5), 1);
    assert_eq!(sub58350_delta(100, 600, 5), 5);
    assert_eq!(sub58350_delta(600, 100, 5), -5);
    assert_eq!(sub58350_delta(17, 17, 5), 0);

    let (mut session, _, owner_slot) = ready_lightning(0);
    let arena = &mut session.session.state.live.economy;
    let targets = arena
        .entities
        .iter()
        .filter(|row| row.active && row.index != 0 && row.index != owner_slot)
        .take(2)
        .map(|row| row.index)
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 2);
    let [hit_slot, target_slot] = [targets[0], targets[1]];
    arena.entities[usize::from(hit_slot)].source_id = 77;
    arena.entities[usize::from(target_slot)].source_id = 77;
    assert_eq!(
        guided_projectile::source_accuracy(arena, 9, owner_slot, Some(hit_slot), target_slot),
        Some((owner_slot, true))
    );
    arena.entities[usize::from(hit_slot)].source_id = 78;
    assert_eq!(
        guided_projectile::source_accuracy(arena, 9, owner_slot, Some(hit_slot), target_slot),
        Some((owner_slot, false))
    );
    assert_eq!(
        guided_projectile::source_accuracy(arena, 13, owner_slot, Some(hit_slot), target_slot),
        None,
        "a non-source-eligible model is not an accuracy attempt"
    );
}

#[test]
fn family22_ray_reacquires_a_stale_bound_target_only_on_its_first_step() {
    let (mut session, _, owner_slot) = ready_lightning(2);
    let live = &mut session.session.state.live;
    let target = live
        .economy
        .entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.model == 1)
        .cloned()
        .expect("qualified campaign fixture has a live rival target");
    assert_ne!(target.source_id, owner_slot);
    let target_slot = target.index;
    let target_position = Position {
        z: target.position.z.wrapping_add(target.array_yaw()),
        ..target.position
    };

    let terrain = &live.terrain;
    let arena = &mut live.economy;
    let fireball_slot = arena
        .economy
        .spawn_fireball_projectile_with_payload(
            crate::native_world::level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
                origin: crate::native_world::FireballProjectileOrigin::PlayerSpell,
                position: target_position,
                yaw: 0,
                pitch: 0,
                axis_position: target_position,
                source_id_entity_index: owner_slot,
                holder_entity_index: 0,
                source_subspell_index: 100,
                source_mana: 50,
                source_dword_0x10: 0,
                target_source_entity_index: target_slot,
                target_class: target.class,
                target_model: target.model,
            },
            0,
        )
        .expect("the canonical Fireball producer allocates a live typed row");
    assert!(arena.handle(fireball_slot).is_some());
    assert!(
        area_damage::source_actor(arena, None, fireball_slot).is_none(),
        "the area-damage actor projection does not represent a typed Fireball"
    );
    let stale_slot = (1..u16::try_from(arena.entities.len()).expect("arena fits u16"))
        .find(|&slot| arena.handle(slot).is_none())
        .expect("campaign fixture has an unoccupied source slot");
    let owner_row = &mut arena.entities[usize::from(owner_slot)];
    owner_row.class = 10;
    owner_row.model = 57;
    owner_row.record[67] = 10;
    owner_row.record[68] = 57;

    let origin = Position {
        x: target_position.x.wrapping_sub(4096),
        ..target_position
    };
    let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(origin, target_position);
    let mut ray = arena.entities[usize::from(owner_slot)].clone();
    ray.class = 9;
    ray.model = 9;
    ray.record[67] = 9;
    ray.record[68] = 9;
    ray.set_source_action(9);
    ray.set_control_flags(0);
    ray.set_position(origin);
    ray.set_yaw(yaw);
    ray.set_pitch(pitch);
    ray.set_actual_speed(1);
    ray.record[136..138].copy_from_slice(&100_u16.to_le_bytes());
    ray.set_max_life(100);
    ray.set_life(100);
    ray.record[69] = 9;
    ray.record[70] = 0;
    owner(&mut ray, owner_slot);
    put(&mut ray, 154, fireball_slot);

    let empty_class5: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let class3 = [target_slot];
    let targets = guided_projectile::Targets {
        class3: &class3,
        class5: &empty_class5,
        class10: &[],
        owner_target_range: 8192,
    };
    let mut outcome = guided_projectile::StepOutcome::default();
    ray_advance(arena, terrain, &mut ray, Some(&targets), &mut outcome).unwrap();
    assert_eq!(
        word(&ray, 154),
        fireball_slot,
        "a live typed Fireball remains the ray's bound target"
    );
    assert_eq!(
        ray.control_flags & 2,
        0,
        "a live target skips first-pass reacquisition"
    );

    put(&mut ray, 154, stale_slot);
    ray_advance(arena, terrain, &mut ray, Some(&targets), &mut outcome).unwrap();
    assert_eq!(word(&ray, 154), target_slot);
    assert_ne!(ray.control_flags & 2, 0);

    put(&mut ray, 154, stale_slot);
    ray_advance(arena, terrain, &mut ray, Some(&targets), &mut outcome).unwrap();
    assert_eq!(
        word(&ray, 154),
        stale_slot,
        "sub_66610 does not retry acquisition after its first attempt"
    );
}

#[test]
fn lightning_holder_produces_each_tier_and_failed_allocation_still_consumes_the_pulse() {
    for tier in 0..=2 {
        let (mut session, slot, parent) = ready_lightning(tier);
        let particles = session.session.definition.presentation.particles.clone();
        let live = &mut session.session.state.live;
        let rule = RULES.tiers[usize::from(tier)];
        let result = step_holder(
            &mut live.shared_spells,
            &mut live.economy,
            &mut live.terrain,
            &particles,
            slot,
            CastInput {
                castle_mana: Some(rule.castle_mana_required),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.clear_player_aux, Some(parent));
        let first = result
            .projectile
            .expect("successful tier launches a source projectile");
        let row = &live.economy.entities[usize::from(first)];
        let expected_model = if tier == 0 { 9 } else { 12 };
        assert_eq!(
            (row.class, row.model, row.action),
            (9, expected_model, expected_model)
        );
        assert_eq!((row.source_id, row.subspell_index), (parent, rule.damage));
        if tier == 2 {
            let second = word(row, 56);
            assert_ne!(second, 0, "tier two creates its linked second shot");
            assert_eq!(word(&live.economy.entities[usize::from(second)], 56), first);
        } else {
            assert_eq!(word(row, 56), 0);
        }
        let holder = live.shared_spells.pickup(slot).unwrap();
        assert_eq!(holder.active_value, holder.activation_value as i16 - 1);
    }

    let (mut session, slot, parent) = ready_lightning(0);
    let particles = session.session.definition.presentation.particles.clone();
    let live = &mut session.session.state.live;
    let holder = live.shared_spells.pickup(slot).unwrap();
    let activation = holder.activation_value;
    let max_mana = holder.max_mana;
    let initial_regen = live.economy.entities[usize::from(parent)].mana_regen;
    let expected_regen = if initial_regen >= 0 {
        max_mana.wrapping_neg()
    } else {
        initial_regen.wrapping_sub(max_mana)
    };
    exhaust(&mut live.economy);
    let result = step_holder(
        &mut live.shared_spells,
        &mut live.economy,
        &mut live.terrain,
        &particles,
        slot,
        CastInput {
            castle_mana: Some(RULES.tiers[0].castle_mana_required),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result, HolderOutcome::default());
    assert_eq!(
        live.economy.entities[usize::from(parent)].mana_regen,
        expected_regen
    );
    assert_eq!(
        live.shared_spells.pickup(slot).unwrap().active_value,
        activation as i16 - 1
    );
}

#[test]
fn lightning_storm_emitter_uses_one_rng_draw_and_produces_opposite_source_rays() {
    let (mut session, _, parent) = ready_lightning(0);
    let particles = session.session.definition.presentation.particles.clone();
    let live = &mut session.session.state.live;
    let arena = &mut live.economy;
    let base = arena.entities[usize::from(parent)].position;
    let position = Position {
        z: live.terrain.ground_height(base).wrapping_add(1024),
        ..base
    };
    let emitter = construct(
        arena,
        &mut live.terrain,
        &particles,
        Product::Storm,
        position,
    )
    .unwrap()
    .unwrap();
    owner(&mut arena.entities[usize::from(emitter)], parent);
    arena.entities[usize::from(emitter)].set_life(4);
    let seed = 0x1234;
    arena.entities[usize::from(emitter)].set_entity_rng(seed);
    let before = arena
        .entities
        .iter()
        .filter(|row| row.active)
        .map(|row| row.index)
        .collect::<BTreeSet<_>>();

    let outcome = step(arena, &mut live.terrain, &particles, emitter, None).unwrap();
    let expected_rng = seed.wrapping_mul(9377).wrapping_add(9439);
    assert_eq!(
        arena.entities[usize::from(emitter)].entity_rng,
        expected_rng
    );
    assert_eq!(
        arena.economy.source_rng.get(&emitter).copied(),
        Some(expected_rng)
    );
    assert_eq!(arena.entities[usize::from(emitter)].life, 3);
    assert_eq!(outcome.sounds.len(), 1);
    let mut rays = arena
        .entities
        .iter()
        .filter(|row| {
            row.active
                && !before.contains(&row.index)
                && (row.class, row.model, row.action) == (9, 9, 9)
                && row.source_id == parent
        })
        .collect::<Vec<_>>();
    rays.sort_unstable_by_key(|row| row.index);
    assert_eq!(rays.len(), 2);
    assert_eq!(rays[0].yaw.wrapping_add(1024) & 2047, rays[1].yaw);
    assert_eq!((rays[0].life, rays[1].life), (3, 3));
}

#[test]
fn lightning_ray_keeps_segment_rng_and_trails_reach_the_hidden_state() {
    let (mut session, _, parent) = ready_lightning(0);
    let particles = session.session.definition.presentation.particles.clone();
    let live = &mut session.session.state.live;
    let arena = &mut live.economy;
    for actor in &mut arena.entities {
        if (actor.class == 3 || actor.class == 5) && actor.index != parent {
            actor.set_life(-1);
            actor.set_control_flags(actor.control_flags | ENTITY_DRAW_DISABLED_BIT);
        }
        actor.set_control_flags(
            actor.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
        );
    }
    let base = arena.entities[usize::from(parent)].position;
    let position = Position {
        z: live.terrain.ground_height(base).wrapping_add(4096),
        ..base
    };
    let slot = construct(arena, &mut live.terrain, &particles, Product::Ray, position)
        .unwrap()
        .unwrap();
    owner(&mut arena.entities[usize::from(slot)], parent);
    let seed = 0x3210;
    {
        let row = &mut arena.entities[usize::from(slot)];
        row.set_entity_rng(seed);
        row.set_life(0);
        row.set_yaw(0);
        row.set_pitch(0);
    }
    let expected_rng = expected_one_step_ray_rng(seed);
    let outcome = step(arena, &mut live.terrain, &particles, slot, None).unwrap();
    assert_eq!(outcome.accuracy, Some((parent, false)));
    assert!(outcome.impact_slot.is_some());
    assert_eq!(arena.entities[usize::from(slot)].entity_rng, expected_rng);
    assert_eq!(
        arena.economy.source_rng.get(&slot).copied(),
        Some(expected_rng)
    );

    let trail = arena
        .entities
        .iter()
        .find(|row| {
            row.active
                && row.source_id == parent
                && (row.class, row.model, row.action) == (9, 9, 14)
        })
        .expect("a real source ray allocates visual trail rows")
        .index;
    for _ in 0..=2 {
        if arena.entities[usize::from(trail)].drawing_disabled() {
            break;
        }
        step(arena, &mut live.terrain, &particles, trail, None).unwrap();
    }
    assert!(arena.entities[usize::from(trail)].drawing_disabled());
    assert!(
        arena.entities[usize::from(trail)].active,
        "common following-pass reclaim owns row release"
    );
}

#[test]
fn lightning_endpoint_submits_one_damage_pulse_then_disables() {
    let (mut session, _, parent) = ready_lightning(0);
    let particles = session.session.definition.presentation.particles.clone();
    let live = &mut session.session.state.live;
    let position = live.economy.entities[usize::from(parent)].position;
    let endpoint = construct(
        &mut live.economy,
        &mut live.terrain,
        &particles,
        Product::Endpoint,
        position,
    )
    .unwrap()
    .unwrap();
    let row = &mut live.economy.entities[usize::from(endpoint)];
    owner(row, parent);
    row.set_subspell_index(75);
    let expected = DamagePulse {
        position: row.position,
        owner: parent,
        payload: row.subspell_index,
        half_width: row.array_pitch(),
        vertical_offset: row.array_yaw(),
        half_height: row.array_fov(),
        continuous: false,
        selector: (row.record[69], row.record[70]),
    };
    let mut pulses = Vec::new();
    let first = step_endpoint(&mut live.economy, endpoint, |_, pulse| {
        pulses.push(pulse);
        Ok(1)
    })
    .unwrap();
    assert_eq!(pulses, vec![expected]);
    assert_eq!(first.experience, Some((parent, 7, 1)));
    assert_eq!(first.sounds, vec![(endpoint, 24)]);

    let mut repeated = false;
    step_endpoint(&mut live.economy, endpoint, |_, _| {
        repeated = true;
        Ok(1)
    })
    .unwrap();
    assert!(!repeated, "the endpoint is a one-shot damage producer");
    step_endpoint(&mut live.economy, endpoint, |_, _| Ok(0)).unwrap();
    let third = &live.economy.entities[usize::from(endpoint)];
    assert_eq!(third.life, -1);
    assert!(
        !third.drawing_disabled(),
        "old life zero still runs visibly"
    );
    step_endpoint(&mut live.economy, endpoint, |_, _| Ok(0)).unwrap();
    let fourth = &live.economy.entities[usize::from(endpoint)];
    assert_eq!(fourth.life, -2);
    assert!(
        fourth.drawing_disabled(),
        "old life -1 hides on the fourth call"
    );
    assert!(live.economy.entities[usize::from(endpoint)].active);
}

fn source_slot_cleanup_state(
    session: &super::super::Mc2Simulation,
    slot: u16,
    generation: u32,
) -> (bool, bool) {
    let live = &session.session.state.live;
    let economy = &live.economy.economy;
    let reclaimed =
        economy.free_source_slots().contains(&slot) || economy.slot_generation(slot) > generation;
    let hidden = reclaimed
        || live
            .economy
            .entities
            .get(usize::from(slot))
            .is_some_and(|row| row.active && row.drawing_disabled());
    (hidden, reclaimed)
}

fn source_entity_damage_mailbox(
    session: &super::super::Mc2Simulation,
    target_slot: u16,
    attacker_slot: u16,
) -> Option<i32> {
    let row = session
        .session
        .state
        .live
        .economy
        .entities
        .get(usize::from(target_slot))?;
    let damage = i32::from_le_bytes(row.record[98..102].try_into().ok()?);
    let attacker = u16::from_le_bytes(row.record[102..104].try_into().ok()?);
    (damage > 0 && attacker == attacker_slot).then_some(damage)
}

#[test]
fn family22_lightning_fire_keeps_its_bound_target_and_damages_through_the_endpoint_consumer() {
    let mut session = super::super::tests::level_two_campaign_session();
    let live = &mut session.session.state.live;
    let parent = live.core.state.entity_index;
    let sphere_damage = crate::spells::late::fools_mana::SOURCE_RULES[2].projectile_damage;
    let player = &mut live.economy.entities[usize::from(parent)];
    player.class = 3;
    player.model = 0;
    player.record[67] = 3;
    player.record[68] = 0;

    let target = live
        .economy
        .entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.model == 1 && row.action == 1)
        .cloned()
        .expect("qualified campaign fixture has its live rival Nyphur");
    assert_ne!(
        target.source_id, parent,
        "retaliation targets another owner"
    );
    assert!(target.life >= 0 && target.control_flags & 8 != 0);
    let target_slot = target.index;
    let target_position = Position {
        z: target.position.z.wrapping_add(target.array_yaw()),
        ..target.position
    };
    let sphere_position = Position {
        x: target_position
            .x
            .wrapping_sub(u16::try_from(RULES.projectile_speed).expect("positive ray speed")),
        ..target_position
    };
    let sphere_slot = live
        .economy
        .allocate_source_slot_with_terrain(&mut live.terrain)
        .expect("source sphere slot");
    let mut sphere = super::super::cave_features::new_event(
        sphere_slot,
        live.economy.economy.new_event_rng(sphere_slot),
        sphere_position,
    );
    sphere.set_position(Position {
        z: target_position.z.wrapping_sub(sphere.array_fov()),
        ..sphere_position
    });
    let sphere_position = sphere.position;
    sphere.class = 10;
    sphere.model = 57;
    sphere.record[67] = 10;
    sphere.record[68] = 57;
    sphere.record[71] = 10;
    sphere.record[72] = 1;
    sphere.set_source_action(62);
    sphere.set_max_life(300);
    sphere.set_life(300);
    sphere.record[61] = 0;
    put(&mut sphere, 44, parent);
    put(&mut sphere, 108, target_slot);
    sphere.record[104..108].fill(0);
    sphere.record[20..24].copy_from_slice(&0_i32.to_le_bytes());
    sphere.level_gate = 2;
    sphere.record[74] = 2;
    sphere.set_subspell_index(sphere_damage);
    live.economy.entities[usize::from(sphere_slot)] = sphere;
    crate::world::effects::sync_position(
        &mut live.economy,
        &mut live.terrain,
        sphere_slot,
        sphere_position,
        true,
        false,
    )
    .unwrap();
    let sphere_generation = live.economy.economy.slot_generation(sphere_slot);
    let sphere_phase_before = live.economy.entities[usize::from(sphere_slot)].phase;
    let target_life_before = live.economy.entities[usize::from(target_slot)].life;
    let family7_experience_before = live.shared_spells.spell_experience[7];
    let family22_experience_before = live.shared_spells.spell_experience[22];
    let target_selector = (target.class, target.model);

    // Run the ordinary source scheduler: it dispatches the sphere, same-pass
    // Lightning child and endpoint when their slots are ahead of the cursor.
    session.step_outer_turn(crate::PlayerCommand::default(), 0);
    let live = &session.session.state.live;
    let mut target_damage_queued =
        source_entity_damage_mailbox(&session, target_slot, sphere_slot).is_some();
    let mut target_damage_callback_seen = target_damage_queued;
    let mut target_life_after_damage = (live.economy.entities[usize::from(target_slot)].life
        < target_life_before)
        .then_some(live.economy.entities[usize::from(target_slot)].life);
    assert_eq!(
        live.economy.entities[usize::from(sphere_slot)].phase,
        sphere_phase_before.wrapping_add(1),
        "the sphere handler receives its source same-pass phase tail"
    );
    let ray = live
        .economy
        .entities
        .iter()
        .find(|row| {
            row.active
                && row.source_id == sphere_slot
                && (row.class, row.model, row.action) == (9, 9, 9)
        })
        .expect("scheduled tier-two sphere creates its LightningFire ray")
        .index;
    let ray_row = &live.economy.entities[usize::from(ray)];
    assert_eq!((ray_row.record[69], ray_row.record[70]), target_selector);
    assert_eq!((ray_row.record[71], ray_row.record[72]), (10, 23));
    assert_eq!(ray_row.source_id, sphere_slot);
    assert_eq!(ray_row.source_word_96(), target_slot);
    assert_eq!(ray_row.subspell_index, sphere_damage);
    assert_eq!(ray_row.parameter_relation, Some(64));
    assert_eq!(
        u32::from_le_bytes(ray_row.record[164..168].try_into().unwrap()),
        65
    );
    assert_eq!(
        guided_projectile::source_accuracy(
            &live.economy,
            ray_row.model,
            ray_row.source_id,
            Some(target_slot),
            target_slot,
        ),
        None,
        "a sphere owner is not a player accuracy row"
    );
    let ray_generation = live.economy.economy.slot_generation(ray);
    let ray_initial_phase = ray as u8;

    let mut endpoint_identity = live
        .economy
        .entities
        .iter()
        .find(|row| {
            row.active
                && row.source_id == sphere_slot
                && (row.class, row.model, row.action) == (10, 23, 23)
                && word(row, 154) == target_slot
        })
        .map(|row| {
            (
                row.index,
                live.economy.economy.slot_generation(row.index),
                row.index as u8,
            )
        });
    if target_damage_callback_seen
        && let Some((endpoint, generation, initial_phase)) = endpoint_identity
        && live.economy.economy.slot_generation(endpoint) == generation
    {
        assert_eq!(
            live.economy.entities[usize::from(endpoint)].phase,
            initial_phase.wrapping_add(1),
            "the endpoint receives one scheduler epilogue after its real damage callback"
        );
    }

    let (mut sphere_hidden, mut sphere_reclaimed) =
        source_slot_cleanup_state(&session, sphere_slot, sphere_generation);
    let ray_row = &live.economy.entities[usize::from(ray)];
    let mut ray_hidden = ray_row.active && ray_row.drawing_disabled();
    let mut ray_reclaimed = false;
    if ray_hidden {
        assert_eq!(
            ray_row.phase,
            ray_initial_phase.wrapping_add(1),
            "the hidden ray receives one scheduler epilogue before reclaim"
        );
    }
    let (mut endpoint_hidden, mut endpoint_reclaimed) = endpoint_identity
        .map(|(endpoint, generation, _)| source_slot_cleanup_state(&session, endpoint, generation))
        .unwrap_or((false, false));

    for _ in 0..8 {
        session.step_outer_turn(crate::PlayerCommand::default(), 0);
        let live = &session.session.state.live;
        if endpoint_identity.is_none()
            && let Some(row) = live.economy.entities.iter().find(|row| {
                row.active
                    && row.source_id == sphere_slot
                    && (row.class, row.model, row.action) == (10, 23, 23)
                    && word(row, 154) == target_slot
            })
        {
            endpoint_identity = Some((
                row.index,
                live.economy.economy.slot_generation(row.index),
                row.index as u8,
            ));
        }

        target_damage_queued |=
            source_entity_damage_mailbox(&session, target_slot, sphere_slot).is_some();
        target_damage_callback_seen |= target_damage_queued;
        let target_life = live.economy.entities[usize::from(target_slot)].life;
        if target_life < target_life_before {
            target_life_after_damage.get_or_insert(target_life);
        }

        let (hidden, reclaimed) =
            source_slot_cleanup_state(&session, sphere_slot, sphere_generation);
        sphere_hidden |= hidden;
        sphere_reclaimed |= reclaimed;
        let (hidden, reclaimed) = source_slot_cleanup_state(&session, ray, ray_generation);
        ray_hidden |= hidden;
        ray_reclaimed |= reclaimed;
        if live.economy.economy.slot_generation(ray) == ray_generation
            && let Some(row) = live.economy.entities.get(usize::from(ray))
            && row.active
            && row.drawing_disabled()
        {
            assert_eq!(
                row.phase,
                ray_initial_phase.wrapping_add(1),
                "the hidden ray receives one scheduler epilogue before reclaim"
            );
        }
        if let Some((endpoint, generation, _)) = endpoint_identity {
            let (hidden, reclaimed) = source_slot_cleanup_state(&session, endpoint, generation);
            endpoint_hidden |= hidden;
            endpoint_reclaimed |= reclaimed;
        }

        if target_damage_callback_seen
            && let Some((endpoint, generation, initial_phase)) = endpoint_identity
            && live.economy.economy.slot_generation(endpoint) == generation
        {
            assert_eq!(
                live.economy.entities[usize::from(endpoint)].phase,
                initial_phase.wrapping_add(1),
                "the endpoint receives one scheduler epilogue after its real damage callback"
            );
        }

        if endpoint_identity.is_some()
            && target_damage_queued
            && sphere_reclaimed
            && ray_reclaimed
            && endpoint_reclaimed
        {
            break;
        }
    }

    let _endpoint_identity =
        endpoint_identity.expect("scheduled Lightning ray produces a bound-target endpoint");
    let live = &session.session.state.live;
    assert!(
        target_damage_queued,
        "the real LightningFire endpoint queues damage for the rival Nyphur source mailbox"
    );
    assert!(
        sphere_hidden && sphere_reclaimed,
        "consumed sphere hides then reclaims at the following boundary"
    );
    assert!(
        ray_hidden && ray_reclaimed,
        "Lightning ray hides then reclaims at the following boundary"
    );
    assert!(
        endpoint_hidden && endpoint_reclaimed,
        "endpoint hides then reclaims at the following boundary"
    );
    assert_eq!(
        live.shared_spells.spell_experience[7], family7_experience_before,
        "sphere-owned ray and endpoint award no player Lightning XP"
    );
    assert_eq!(
        live.shared_spells.spell_experience[22],
        family22_experience_before.wrapping_add(1),
        "the owning sphere's ordinary terminal callback still awards family22 XP"
    );
    assert!(
        target_life_after_damage.is_some_and(|life| life < target_life_before),
        "the next Nyphur scheduler pass consumes the endpoint mailbox and reduces rival life"
    );
}
