use super::*;
use crate::native_world::ENTITY_DRAW_DISABLED_BIT;

/// Reuse the existing spell-owner fixture for boundary cases. The separate
/// ordinary Mission4 group/pickup regression owns the authored reward proof.
fn ready_holder() -> (super::super::Level2ResetSession, u16, u16) {
    let mut session = super::super::tests::level_two_campaign_session();
    let live = &mut session.session.state.live;
    let parent = live.core.state.entity_index;
    let slot = live
        .shared_spells
        .pickups()
        .iter()
        .find(|row| row.model == 3)
        .unwrap()
        .index;
    let holder = live.shared_spells.pickup_mut(slot).unwrap();
    holder.model = 9;
    holder.record[68] = 9;
    holder.set_source_action(27);
    holder.active_value = 0;
    put(holder, 50, 0);
    holder.apply_nyphur_spell_profile(
        crate::spell_activation::level2_set_spell_profile(9, 0).unwrap(),
        0,
    );
    holder.active_value = holder.activation_value as i16;
    put(holder, 50, holder.activation_value);
    put(holder, 44, parent);
    holder.set_control_flags(holder.control_flags & !ENTITY_DRAW_DISABLED_BIT);
    assert!(live.economy.entities[usize::from(parent)].mana >= 1000);
    assert!(live.economy.entities[usize::from(parent)].mana_regen >= 0);
    (session, slot, parent)
}

fn exhaust(arena: &mut SourceArena) {
    for row in &mut arena.entities {
        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    }
    arena.economy.exhaust_new_event_capacity_for_test();
    arena.rebuild_source_free_arena();
}

#[test]
fn failed_lightning_allocation_still_charges_and_decrements_without_clearing_auxiliary() {
    let (mut session, slot, parent) = ready_holder();
    let particles = session.session.definition.presentation.particles.clone();
    let live = &mut session.session.state.live;
    exhaust(&mut live.economy);
    let phase = live.shared_spells.pickup(slot).unwrap().phase;
    let outcome = step_holder(
        &mut live.shared_spells,
        &mut live.economy,
        &mut live.terrain,
        &particles,
        slot,
        CastInput {
            auxiliary_byte: 3,
            castle_mana: Some(5000),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(outcome, HolderOutcome::default());
    assert_eq!(live.economy.entities[usize::from(parent)].mana_regen, -1000);
    assert_eq!(live.shared_spells.pickup(slot).unwrap().active_value, 8);
    assert_eq!(
        live.shared_spells.pickup(slot).unwrap().phase,
        phase,
        "root owns common phase"
    );
}

#[test]
fn successful_lightning_holder_uses_source_mana_and_returns_real_launch_projection() {
    let (mut session, slot, parent) = ready_holder();
    let particles = session.session.definition.presentation.particles.clone();
    let live = &mut session.session.state.live;
    let mana = live.economy.entities[usize::from(parent)].mana;
    let outcome = step_holder(
        &mut live.shared_spells,
        &mut live.economy,
        &mut live.terrain,
        &particles,
        slot,
        CastInput {
            auxiliary_byte: 2,
            castle_mana: Some(5000),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(outcome.clear_player_aux, Some(parent));
    let projectile = &live.economy.entities[usize::from(outcome.projectile.unwrap())];
    assert_eq!(
        (projectile.class, projectile.model, projectile.action),
        (9, 3, 3)
    );
    assert_eq!(
        (
            projectile.source_id,
            word(projectile, 42),
            word(projectile, 154)
        ),
        (parent, slot, 0)
    );
    assert_eq!(projectile.source_dword_10(), 2);
    assert_eq!(live.shared_spells.pickup(slot).unwrap().active_value, 8);
    // The source expenditure mailbox is consumed by the ordinary player,
    // rather than deducting/inventing mana in this holder callback.
    assert_eq!(live.economy.entities[usize::from(parent)].mana, mana);
    assert_eq!(live.economy.entities[usize::from(parent)].mana_regen, -1000);
}

#[test]
fn failed_lightning_predicate_runs_cleanup_and_restores_deferred_tier() {
    let (mut session, slot, parent) = ready_holder();
    let particles = session.session.definition.presentation.particles.clone();
    let live = &mut session.session.state.live;
    let regen = live.economy.entities[usize::from(parent)].mana_regen;
    live.economy.entities[usize::from(parent)].set_life(-1);
    let holder = live.shared_spells.pickup_mut(slot).unwrap();
    holder.set_source_word_2c(2);
    put(holder, 58, 3);
    let outcome = step_holder(
        &mut live.shared_spells,
        &mut live.economy,
        &mut live.terrain,
        &particles,
        slot,
        CastInput {
            castle_mana: Some(5000),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(outcome, HolderOutcome::default());
    let holder = live.shared_spells.pickup(slot).unwrap();
    assert_eq!(
        (holder.active_value, holder.level_gate, holder.max_mana),
        (0, 1, 1500)
    );
    assert_eq!((holder.source_word_2c(), word(holder, 58)), (0, 2));
    assert_eq!(live.economy.entities[usize::from(parent)].mana_regen, regen);
}

#[test]
fn lightning_launch_and_trail_keep_source_payload_two_draws_and_failed_birth_progress() {
    let (mut session, holder, parent) = ready_holder();
    let particles = session.session.definition.presentation.particles.clone();
    let live = &mut session.session.state.live;
    live.economy.entities[usize::from(holder)] = live.shared_spells.pickup(holder).unwrap().clone();
    let input = CastInput {
        aim_yaw: 17,
        aim_pitch: -31,
        auxiliary_byte: 2,
        castle_mana: Some(5000),
    };
    let slot = launch(
        &mut live.economy,
        &mut live.terrain,
        &particles,
        holder,
        parent,
        input,
    )
    .unwrap()
    .unwrap();
    let row = &live.economy.entities[usize::from(slot)];
    assert_eq!((row.class, row.model, row.action, row.life), (9, 3, 3, 21));
    assert_eq!(
        (
            row.record[71],
            row.record[72],
            row.subspell_index,
            row.record[74]
        ),
        (10, 17, 1750, 2)
    );
    assert_eq!(
        (row.particle_parameter_index, row.parameter_relation),
        (Some(76), Some(60))
    );
    assert_eq!(word(row, 42), holder);
    let position = row.position;
    let first = row.entity_rng.wrapping_mul(9377).wrapping_add(9439);
    let second = first.wrapping_mul(9377).wrapping_add(9439);
    after_projectile(&mut live.economy, &mut live.terrain, &particles, slot, None).unwrap();
    assert_eq!(live.economy.entities[usize::from(slot)].entity_rng, second);
    let trail = live
        .economy
        .entities
        .iter()
        .find(|row| {
            row.active
                && row.class == 10
                && row.model == 0
                && row.control_flags & 0x10080 == 0x10080
        })
        .unwrap();
    assert_eq!(
        trail.position,
        Position {
            x: position.x.wrapping_add(first % 129).wrapping_sub(160),
            y: position.y.wrapping_add(second % 129).wrapping_sub(160),
            z: position.z,
        }
    );
    assert_eq!(
        (trail.life, trail.record[96], trail.source_id),
        (4, 3, parent)
    );
    let next = second
        .wrapping_mul(9377)
        .wrapping_add(9439)
        .wrapping_mul(9377)
        .wrapping_add(9439);
    exhaust(&mut live.economy);
    after_projectile(&mut live.economy, &mut live.terrain, &particles, slot, None).unwrap();
    assert_eq!(live.economy.entities[usize::from(slot)].entity_rng, next);
}

#[test]
fn lightning_terminal_projectile_uses_existing_model17_damage_and_lifetime_consumer() {
    let (mut session, holder, parent) = ready_holder();
    let particles = session.session.definition.presentation.particles.clone();
    let live = &mut session.session.state.live;
    live.economy.entities[usize::from(holder)] = live.shared_spells.pickup(holder).unwrap().clone();
    let slot = launch(
        &mut live.economy,
        &mut live.terrain,
        &particles,
        holder,
        parent,
        CastInput {
            castle_mana: Some(5000),
            ..Default::default()
        },
    )
    .unwrap()
    .unwrap();
    // Pin the ordinary source expiry boundary, rather than waiting through
    // a gameplay/input route. The real flyer constructs the real impact.
    let position = live.economy.entities[usize::from(slot)].position;
    let position = Position {
        z: live.terrain.ground_height(position).wrapping_add(512),
        ..position
    };
    let row = &mut live.economy.entities[usize::from(slot)];
    row.set_position(position);
    row.set_actual_speed(0);
    put(row, 136, 0);
    row.set_life(0);
    crate::world::effects::sync_position(
        &mut live.economy,
        &mut live.terrain,
        slot,
        position,
        true,
        false,
    )
    .unwrap();
    let outcome =
        step_projectile(&mut live.economy, &mut live.terrain, &particles, slot, None).unwrap();
    let impact = outcome.impact_slot.unwrap();
    let effect = live
        .economy
        .authored_model17
        .iter()
        .find(|row| row.source_entity_index == impact)
        .unwrap();
    assert_eq!(
        (
            effect.entity_id,
            effect.subspell_index,
            effect.life,
            effect.max_life
        ),
        (parent, 1750, 2, 2)
    );
    assert_eq!(
        (
            live.economy.entities[usize::from(impact)].life,
            live.economy.entities[usize::from(impact)].max_life
        ),
        (2, 2)
    );
    let mut pulses = Vec::new();
    let mut state = crate::world::effects::State::default();
    let visit = super::super::effects::visit(
        &mut live.economy,
        &mut live.terrain,
        impact,
        super::super::Level2DynamicArenaKind::AuthoredModel17,
        &particles,
        &mut state,
        |_, pulse| {
            pulses.push(pulse);
            Ok(0)
        },
    )
    .unwrap();
    assert_eq!(visit.sounds, vec![30]);
    assert_eq!(pulses.len(), 1);
    assert_eq!((pulses[0].owner, pulses[0].payload), (parent, 875));
    let effect = live
        .economy
        .authored_model17
        .iter()
        .find(|row| row.source_entity_index == impact)
        .unwrap();
    assert_eq!(effect.life, 1);
    super::super::effects::visit(
        &mut live.economy,
        &mut live.terrain,
        impact,
        super::super::Level2DynamicArenaKind::AuthoredModel17,
        &particles,
        &mut state,
        |_, pulse| {
            pulses.push(pulse);
            Ok(0)
        },
    )
    .unwrap();
    super::super::effects::visit(
        &mut live.economy,
        &mut live.terrain,
        impact,
        super::super::Level2DynamicArenaKind::AuthoredModel17,
        &particles,
        &mut state,
        |_, pulse| {
            pulses.push(pulse);
            Ok(0)
        },
    )
    .unwrap();
    assert_eq!(pulses.len(), 2, "expiry emits no third damage pulse");
    assert!(
        live.economy
            .authored_model17
            .iter()
            .find(|row| row.source_entity_index == impact)
            .unwrap()
            .control_flags
            & ENTITY_DRAW_DISABLED_BIT
            != 0
    );
}

#[test]
fn lightning_acquires_current_typed_dragon_despite_stale_raw_header() {
    let (mut session, holder, parent) = ready_holder();
    let particles = session.session.definition.presentation.particles.clone();
    let live = &mut session.session.state.live;
    live.economy.entities[usize::from(holder)] = live.shared_spells.pickup(holder).unwrap().clone();
    let projectile = launch(
        &mut live.economy,
        &mut live.terrain,
        &particles,
        holder,
        parent,
        CastInput::default(),
    )
    .unwrap()
    .unwrap();
    let mut position = live.economy.entities[usize::from(projectile)].position;
    move_entity(&mut position, 0, 0, 1536);
    let dragon = live
        .economy
        .economy
        .allocate_authored_dragon(575, position, 1000, &live.terrain)
        .unwrap();
    let dragon_slot = dragon.source_entity_index;
    assert_ne!(dragon.source_byte_39, 0);
    let frozen = live.economy.rebuilt_class5_model_slots();
    assert!(frozen[0].contains(&dragon_slot));
    // Isolate this candidate from the existing spell-owner fixture. Its
    // membership comes from the actual Dragon constructor and frozen head.
    let mut families = std::array::from_fn(|_| Vec::new());
    families[0].push(dragon_slot);
    let targets = super::super::guided_projectile::Targets {
        class3: &[],
        class5: &families,
        class10: &[],
        owner_target_range: 8192,
    };
    let actor =
        super::super::area_damage::source_actor(&mut live.economy, None, dragon_slot).unwrap();
    let centre = Position {
        z: actor.position.z.wrapping_add(actor.shape.vertical_offset),
        ..actor.position
    };
    let mut origin = live.economy.entities[usize::from(projectile)].position;
    origin.z = centre.z;
    let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(origin, centre);
    let row = &mut live.economy.entities[usize::from(projectile)];
    row.set_position(origin);
    row.set_actual_speed(0);
    put(row, 136, 0);
    row.set_yaw(yaw);
    row.set_pitch(pitch);
    let phase = row.phase;
    crate::world::effects::sync_position(
        &mut live.economy,
        &mut live.terrain,
        projectile,
        origin,
        true,
        false,
    )
    .unwrap();
    // A typed birth does not publish its decoded packet. Poison the old
    // packet's ownership/cadence/stage to reject this valid current target.
    let stale = &mut live.economy.entities[usize::from(dragon_slot)];
    stale.reclaim_source_row();
    set_owner(stale, parent);
    stale.record[61] = 0;
    stale.record[77] = 14;
    put(stale, 44, parent);
    let outcome = step_projectile(
        &mut live.economy,
        &mut live.terrain,
        &particles,
        projectile,
        Some(&targets),
    )
    .unwrap();
    assert_eq!(outcome.impact_slot, None);
    let row = &live.economy.entities[usize::from(projectile)];
    assert_eq!(word(row, 154), dragon_slot);
    assert_eq!((row.yaw, row.pitch, row.phase), (yaw, pitch, phase));
    assert_eq!(live.economy.entities[usize::from(dragon_slot)].class, 0);
    assert_eq!(
        live.economy
            .economy
            .authored_dragons
            .iter()
            .find(|state| state.source_entity_index == dragon_slot)
            .unwrap()
            .font_type_index,
        32
    );
    // The reverse mismatch must still reject: a stale positive cadence is
    // not permission to acquire a current zero-cadence typed target.
    live.economy
        .economy
        .authored_class5
        .iter_mut()
        .find(|source| source.source_entity_index == dragon_slot)
        .unwrap()
        .source_byte_39 = 0;
    live.economy.entities[usize::from(dragon_slot)].record[61] = 255;
    let row = &mut live.economy.entities[usize::from(projectile)];
    row.set_control_flags(row.control_flags & !2);
    put(row, 154, 0);
    step_projectile(
        &mut live.economy,
        &mut live.terrain,
        &particles,
        projectile,
        Some(&targets),
    )
    .unwrap();
    assert_eq!(
        word(&live.economy.entities[usize::from(projectile)], 154),
        0
    );
}
