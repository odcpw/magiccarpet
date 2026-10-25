use super::*;
use crate::native_world::TerrainPlanes;

fn terrain() -> SessionTerrain {
    let plane = || vec![0; 65_536].into_boxed_slice();
    SessionTerrain::from_planes(
        MapType::Day,
        TerrainPlanes {
            terrain_types: plane(),
            ground_heights: plane(),
            shading: plane(),
            angles: plane(),
            secondary_heights: plane(),
            entity_indices: vec![0; 65_536].into_boxed_slice(),
        },
        0,
    )
}

fn bee() -> BeeCreatureRuntime {
    construct(
        20,
        7,
        0,
        BeeCreatureOrigin::AuthoredRecord {
            authored_index: 229,
        },
        Position {
            x: 30_000,
            y: 30_000,
            z: 1000,
        },
        3000,
        MapType::Day,
    )
    .unwrap()
}

fn target(source: &BeeCreatureRuntime, slot: u16) -> BeeActor {
    let mut row = source.projection(true);
    row.slot = slot;
    row.source_id = slot;
    row.class = 3;
    row.model = 0;
    row.position.x = row.position.x.wrapping_add(500);
    row.frozen_class3_member = true;
    row.frozen_class5_member = false;
    row.frozen_family_member = false;
    row
}

#[test]
fn nonlethal_attack_mailbox_preserves_recoil_countdown_and_rng() {
    let terrain = terrain();
    let mut source = bee();
    source.action = 18;
    source.target_entity_index = Some(40);
    source.dword_0x10 = 7;
    source.pending_damage = 100;
    source.pending_attacker_entity_index = 41;
    let rng = source.entity_rng;
    let before_position = source.position;
    let actors = [target(&source, 40), target(&source, 41)];
    let effects = step(
        &mut source,
        &BeeActionContext {
            terrain: &terrain,
            actors: &actors,
            stage: None,
        },
    );
    assert_eq!(source.life, 2900);
    assert_eq!(source.action, 18);
    assert_eq!(source.dword_0x10, 6);
    assert_eq!(source.entity_rng, rng);
    assert_eq!(source.target_entity_index, Some(41));
    assert_eq!(source.pending_attacker_entity_index, 0);
    assert_eq!(source.pending_damage, 100);
    assert_eq!(source.source_byte_3e, 1);
    assert_eq!(source.position.x, before_position.x);
    assert!(effects.sound.is_none() && effects.contact.is_none());
}

#[test]
fn signed_dword_damage_is_not_narrowed_to_a_word() {
    let terrain = terrain();
    let mut source = bee();
    source.pending_damage = 65_536;
    source.pending_attacker_entity_index = 40;
    let effects = step(
        &mut source,
        &BeeActionContext {
            terrain: &terrain,
            actors: &[],
            stage: None,
        },
    );
    assert_eq!(source.life, -62_536);
    assert_eq!(source.action, 20);
    assert_eq!(source.kill_credit_entity_index, 40);
    assert!(effects.damage.unwrap().lethal);
    assert!(effects.sound.is_none());
}

#[test]
fn attack_without_relation_does_not_consume_pending_damage() {
    let terrain = terrain();
    let mut source = bee();
    source.action = 18;
    source.pending_damage = 70_000;
    source.pending_attacker_entity_index = 40;
    step(
        &mut source,
        &BeeActionContext {
            terrain: &terrain,
            actors: &[],
            stage: None,
        },
    );
    assert_eq!(source.action, 17);
    assert_eq!(source.life, 3000);
    assert_eq!(source.pending_attacker_entity_index, 40);
    assert_eq!(source.actual_speed, 64);
}

#[test]
fn stationary_callback_selects_family_peer_instead_of_word96() {
    let terrain = terrain();
    let mut source = bee();
    source.action = 16;
    source.target_entity_index = Some(40);
    let player = target(&source, 40);
    step(
        &mut source,
        &BeeActionContext {
            terrain: &terrain,
            actors: &[player],
            stage: None,
        },
    );
    assert_eq!(
        source.action, 16,
        "word96 alone is not sub1BD90's peer relation"
    );
    let mut peer = bee().projection(true);
    peer.slot = 21;
    peer.source_id = 21;
    peer.position.x = source.position.x.wrapping_sub(500);
    source.yaw = heading_for_positions(peer.position, source.position) as i16;
    source.source_byte_3e = 0;
    step(
        &mut source,
        &BeeActionContext {
            terrain: &terrain,
            actors: &[peer, player],
            stage: None,
        },
    );
    assert_eq!(source.action, 19);
    assert_eq!(source.linked_peer_entity_index, 21);
    assert_eq!(source.target_entity_index, Some(40));
}

#[test]
fn linked_peer_attack_is_inherited_only_on_cadence() {
    let terrain = terrain();
    let mut source = bee();
    source.action = 19;
    source.linked_peer_entity_index = 21;
    source.source_byte_3e = 1;
    source.control_flags |= LEVEL2_BEE_MOVEMENT_SKIP_BIT;
    let mut peer = bee().projection(true);
    peer.slot = 21;
    peer.source_id = 21;
    peer.action = 18;
    peer.target = 40;
    step(
        &mut source,
        &BeeActionContext {
            terrain: &terrain,
            actors: &[peer],
            stage: None,
        },
    );
    assert_eq!(source.action, 19);
    source.source_byte_3e = 30;
    step(
        &mut source,
        &BeeActionContext {
            terrain: &terrain,
            actors: &[peer],
            stage: None,
        },
    );
    assert_eq!(source.action, 18);
    assert_eq!(source.target_entity_index, Some(40));
    assert_eq!(source.linked_peer_entity_index, 0);
    assert_eq!(source.dword_0x10, 1);
}

#[test]
fn contact_returns_real_target_write_and_disabled_callback_does_not_advance() {
    let terrain = terrain();
    let mut source = bee();
    source.action = 18;
    source.target_entity_index = Some(40);
    source.control_flags |= LEVEL2_BEE_MOVEMENT_SKIP_BIT;
    let actors = [target(&source, 40)];
    let effects = step(
        &mut source,
        &BeeActionContext {
            terrain: &terrain,
            actors: &actors,
            stage: None,
        },
    );
    assert_eq!(
        effects.contact,
        Some(BeeContact {
            target: 40,
            attacker: 20,
            damage: 200
        })
    );
    assert_eq!(source.actual_speed, -30);
    assert_eq!(source.dword_0x10, 90);
    source.action = 22;
    let before = source;
    let disabled = step(
        &mut source,
        &BeeActionContext {
            terrain: &terrain,
            actors: &actors,
            stage: None,
        },
    );
    assert!(!disabled.callback_executed);
    assert_eq!(source, before);
}

#[test]
fn summon_parent_loss_requests_both_effects_but_defers_slot_release() {
    let terrain = terrain();
    let mut source = bee();
    source.action = 23;
    source.stage_var2 = 13;
    source.parent_entity_index = 40;
    source.stage_remaining = 10;
    let effects = step(
        &mut source,
        &BeeActionContext {
            terrain: &terrain,
            actors: &[],
            stage: None,
        },
    );
    assert!(effects.expire_effect && effects.summon_expire_effect);
    assert!(!source.drawing_enabled);
    assert!(source.active);
    assert_eq!(source.source_class, 5);
    assert_eq!(source.source_byte_3e, 1);
}

#[test]
fn constructor_rejects_non_day_and_preserves_full_life_width() {
    for map in [MapType::Night, MapType::Cave] {
        assert!(construct(20, 7, 0, bee().origin, bee().position, 3000, map).is_none());
    }
    let source = construct(
        20,
        7,
        0,
        bee().origin,
        bee().position,
        100_000,
        MapType::Day,
    )
    .unwrap();
    assert_eq!(source.life, 100_000);
    assert_eq!(source.mana, 50_000);
}

#[test]
fn hidden_before_numeric_visit_still_executes_and_advances_once() {
    let terrain = terrain();
    let mut source = bee();
    source.action = 18;
    source.target_entity_index = Some(40);
    source.control_flags |= LEVEL2_BEE_MOVEMENT_SKIP_BIT;
    source.drawing_enabled = false;
    let mut live_target = target(&source, 40);
    live_target.control_flags &= !DRAW_DISABLED;
    let effects = step(
        &mut source,
        &BeeActionContext {
            terrain: &terrain,
            actors: &[live_target],
            stage: None,
        },
    );
    assert!(effects.callback_executed);
    assert!(effects.contact.is_some());
    assert_eq!(source.source_byte_3e, 1);
    assert!(source.active);
    assert!(!source.drawing_enabled);
}

#[test]
fn death_conversion_advances_positive_mana_rng_before_allocation_only() {
    let world = terrain();
    for mana in [1500, 0, -1] {
        let mut source = bee();
        source.action = 21;
        source.mana = mana;
        source.source_byte_3e = 8;
        let old_rng = source.entity_rng;
        let effects = step(
            &mut source,
            &BeeActionContext {
                terrain: &world,
                actors: &[],
                stage: None,
            },
        );
        assert!(effects.death_conversion);
        assert!(!source.drawing_enabled);
        assert!(source.active);
        assert_eq!(
            source.entity_rng,
            if mana > 0 {
                level2_next_rng16(old_rng)
            } else {
                old_rng
            }
        );
        assert_eq!(source.source_byte_3e, 9);
    }
    let mut source = bee();
    source.source_byte_3e = 7;
    let before = source;
    assert!(!death_conversion_requested(&mut source));
    assert_eq!(source, before);
}

#[test]
fn owner_data_constructor_preserves_authored_health_above_word_width() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let mut arena = session.source_arena().clone();
    let mut world = session.session.state.live.terrain.clone();
    let source =
        mc2::allocate_authored(&mut arena, &mut world, 229, session.state.position, 40_000)
            .expect("real Day arena constructor accepts source dword health");
    let current = arena
        .bee_creatures()
        .iter()
        .find(|row| row.source_entity_index == source.source_entity_index)
        .unwrap();
    assert_eq!(
        (current.max_life, current.life, current.mana),
        (40_000, 40_000, 20_000)
    );
    assert_eq!(current.collision_vertical_offset, 75);
    assert!(arena.map_chains.is_linked(current.source_entity_index));
}

#[test]
fn owner_data_constructor_reuses_frozen_raw_replacement_and_publishes_map_heads() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let mut arena = session.source_arena().clone();
    let mut world = session.session.state.live.terrain.clone();
    crate::world::effects::allocate(
        &mut arena,
        &mut world,
        sim_definition::adapter::mc2_effect_definition(crate::level_definition::EffectKind::Impact),
        session.state.position,
        &session.session.definition.presentation.particles,
    )
    .unwrap()
    .expect("ordinary Impact creates a raw replaceable source row");
    assert!(arena.economy.typed_replacement_candidate().is_none());
    let old = arena
        .entities
        .iter()
        .filter(|row| {
            row.index != 0
                && row.active
                && row.class != 0
                && row.control_flags & crate::level1_mission::SOURCE_REPLACEABLE_FLAG != 0
        })
        .min_by_key(|row| row.index)
        .unwrap()
        .clone();
    let old_tile = arena.map_chains.tile_of(old.index);
    let new_tile = (0..=u16::MAX)
        .find(|tile| *tile != old_tile && arena.map_chains.tile_head(*tile) == 0)
        .unwrap();
    let position = Position {
        x: (u16::from(new_tile as u8) << 8) + 128,
        y: (u16::from((new_tile >> 8) as u8) << 8) + 128,
        z: 128,
    };
    arena.rebuild_source_free_arena();
    let generation = arena.slot_generation(old.index);
    let ordinary_free = arena.free_slot_count();
    for _ in 0..ordinary_free {
        mc2::allocate_authored(&mut arena, &mut world, 229, position, 3000)
            .expect("ordinary source free stack still has capacity");
    }
    let replacement = mc2::allocate_authored(&mut arena, &mut world, 229, position, 3000)
        .expect("canonical NewEvent consumes a frozen raw replacement after free exhaustion");
    assert_eq!(replacement.source_entity_index, old.index);
    assert_eq!(arena.slot_generation(old.index), generation.wrapping_add(1));
    assert!(!arena.entities[usize::from(old.index)].active);
    assert_eq!(
        world.planes.entity_indices[usize::from(old_tile)],
        arena.map_chains.tile_head(old_tile) as i16
    );
    assert_eq!(
        world.planes.entity_indices[usize::from(new_tile)],
        old.index as i16
    );
    assert_eq!(arena.map_chains.tile_head(new_tile), old.index);
}
