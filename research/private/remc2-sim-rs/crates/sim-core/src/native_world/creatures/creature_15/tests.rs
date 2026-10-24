use super::*;

fn expiring_defender(mode: u8) -> (crate::native_world::Level2ResetSession, u16) {
    let mut session = crate::native_world::tests::level_two_campaign_session();
    let castle = session
        .economy_mut()
        .build_castle(
            Position {
                x: 0x2000,
                y: 0x3000,
                z: 0,
            },
            111,
        )
        .expect("actual Castle constructor");
    session.economy.castle.as_mut().expect("Castle").level = 3;
    session.economy_mut().maintain_castle_defenders(castle);
    let slot = session.economy.castle().expect("Castle").auxiliary_slots[0];
    session.economy.exhaust_new_event_capacity_for_test();
    let actor = session
        .economy
        .castle_defenders
        .iter_mut()
        .find(|row| row.source_entity_index == slot)
        .expect("ordinary model15 product");
    actor.action = 127;
    actor.stage.mode = mode;
    actor.stage.parent = 0;
    actor.stage.remaining = 99;
    actor.source_byte_3e = 3;
    actor.control_flags |= crate::native_world::SOURCE_REPLACEABLE_FLAG;
    (session, slot)
}

fn expire(session: &mut crate::native_world::Level2ResetSession, slot: u16) {
    let particles = session.session.definition.presentation.particles.clone();
    let membership = session
        .economy
        .castle_auxiliary_membership(&session.economy.entities);
    let live = &mut session.session.state.live;
    let result = mc2::step_source_runtime(
        &mut live.economy,
        &mut live.core.runtime,
        &live.terrain,
        &particles,
        slot,
        &membership,
    )
    .expect("enabled scripted callback");
    assert!(result.effects.expire);
    assert!(result.map_synced, "epilogue follows the current occupant");
}

#[test]
fn summoned_expiry_self_replacement_hides_raw73_and_advances_its_own_clock() {
    let (mut session, slot) = expiring_defender(13);
    let sprite = session
        .campaign_entity_presentations(0)
        .expect("ordinary owner particle resolver")
        .into_iter()
        .find(|row| row.entity_index == slot)
        .expect("the current defender carrier produces its sprite descriptor");
    assert_eq!(sprite.source_id, 111);
    assert_eq!(sprite.render_parameter_index, 0);
    assert_eq!(
        sprite.position,
        session.economy.castle_defenders()[0].position
    );
    let generation = session.economy.slot_generation(slot);
    expire(&mut session, slot);
    assert_eq!(
        session.economy.slot_generation(slot),
        generation.wrapping_add(1)
    );
    assert!(
        !session
            .economy
            .castle_defenders()
            .iter()
            .any(|row| row.source_entity_index == slot)
    );
    let effect = &session.economy.entities[usize::from(slot)];
    assert_eq!((effect.class, effect.model, effect.action), (10, 73, 80));
    assert_eq!(effect.position.x, 0);
    assert_eq!(effect.position.y, 0);
    assert_eq!(
        effect.position.z,
        session
            .canonical_terrain()
            .ground_height(Position::default())
    );
    assert!(effect.drawing_disabled());
    assert_eq!(effect.phase, (slot as u8).wrapping_add(1));
    assert!(
        session
            .campaign_entity_presentations(0)
            .expect("hidden replacement presentation")
            .iter()
            .all(|row| row.entity_index != slot)
    );
    assert!(
        session.economy.model17_spawned_children().is_empty(),
        "second allocation failed without a substitute success"
    );
    assert!(!session.economy.free_slots.contains(&slot));
    session.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(
        session.economy.free_slots.contains(&slot),
        "following prologue reclaims the hidden replacement"
    );
}

#[test]
fn temporary_expiry_self_replacement_hides_real_impact_at_zero_position() {
    let (mut session, slot) = expiring_defender(16);
    expire(&mut session, slot);
    let effect = session
        .economy
        .model17_spawned_children()
        .iter()
        .find(|row| row.source_entity_index == slot)
        .expect("real NewAdd0A00 product");
    assert_eq!(
        (effect.source_class, effect.model, effect.action),
        (10, 0, 0)
    );
    assert_eq!(effect.position, Position::default());
    assert_eq!(effect.parent_entity_id, slot);
    assert_ne!(effect.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
    assert_eq!(effect.phase, (slot as u8).wrapping_add(1));
    assert!(!session.economy.free_slots.contains(&slot));
    session.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(session.economy.free_slots.contains(&slot));
}
