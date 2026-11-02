use super::*;
use crate::native_world::TerrainPlanes;

fn allocate_typed_morph_shadow(
    arena: &mut crate::campaign::source_arena::SourceArena,
    terrain: &mut SessionTerrain,
    parent_slot: u16,
    position: Position,
    raw_source_id: u16,
) -> u16 {
    let holder_slot = arena
        .allocate_source_slot()
        .expect("reserve a Morph holder source slot");
    let source = mc2::allocate_morph(
        arena,
        terrain,
        holder_slot,
        parent_slot,
        parent_slot,
        position,
    )
    .expect("Day-map Morph constructs a typed Bee");
    let slot = source.source_entity_index;
    let typed = arena
        .economy
        .bee_creatures
        .iter_mut()
        .find(|source| source.active && source.source_entity_index == slot)
        .expect("Morph Bee remains in its typed owner");
    typed.action = 18;
    typed.stage_var1 = 0;
    typed.stage_var2 = 12;
    typed.stage_relation = 9;
    typed.stage_remaining = 5;
    typed.stage_duration = 6;
    typed.target_entity_index = Some(parent_slot);

    // Typed Bees retain their former source record as an inactive compatibility
    // row. The Alliance kernel still resolves its class/model and control data
    // from that row by the source map order.
    let raw = &mut arena.entities[usize::from(slot)];
    raw.source_id = raw_source_id;
    raw.class = 5;
    raw.model = 2;
    raw.record[67] = 5;
    raw.record[68] = 2;
    raw.set_source_action(18);
    raw.record[44..46].copy_from_slice(&parent_slot.to_le_bytes());
    raw.record[76] = 0;
    raw.record[77] = 12;
    raw.record[78..80].copy_from_slice(&9_u16.to_le_bytes());
    raw.record[50..52].copy_from_slice(&5_i16.to_le_bytes());
    raw.record[52..54].copy_from_slice(&6_i16.to_le_bytes());
    raw.record[154..156].copy_from_slice(&parent_slot.to_le_bytes());
    slot
}

fn allocate_alliance_center(
    arena: &mut crate::campaign::source_arena::SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
) -> u16 {
    let slot = arena
        .allocate_source_slot_with_terrain(terrain)
        .expect("allocate Alliance's active Bee center");
    let row = crate::world::terrain::cave_features::new_event(
        slot,
        arena.economy.new_event_rng(slot),
        position,
    );
    let mut row = row;
    row.class = 5;
    row.model = 2;
    row.record[67] = 5;
    row.record[68] = 2;
    arena.entities[usize::from(slot)] = row;
    slot
}

fn run_alliance_impact(
    arena: &mut crate::campaign::source_arena::SourceArena,
    terrain: &mut SessionTerrain,
    bee_slot: u16,
    center_slot: u16,
    owner_slot: u16,
) -> crate::spells::late::alliance::Impact {
    let impact_slot = crate::native_world::alliance_spell::construct_impact(arena, terrain)
        .expect("construct the real source Alliance terminal");
    let effect = &mut arena.entities[usize::from(impact_slot)];
    effect.source_id = owner_slot;
    effect.record[30..32].copy_from_slice(&owner_slot.to_le_bytes());
    effect.record[74] = 32;
    effect.subspell_index = 40;
    effect.record[154..156].copy_from_slice(&center_slot.to_le_bytes());
    crate::native_world::alliance_spell::step_impact(arena, impact_slot, &[bee_slot])
        .expect("consume the real source Alliance terminal")
}

/// Use the real SetSpell holder and scheduled Morph cast to obtain the Bee
/// that the production semantic dispatcher will later visit.
fn public_day_morph_bee() -> (crate::native_world::Mc2Simulation, u16, u16) {
    let mut session = crate::native_world::tests::level_two_campaign_session();
    let parent = session.state.entity_index;
    let mut carried = session.shared_spells().local_player.enabled_spell_holders.iter()
        .enumerate().filter_map(|(family, slot)| (*slot != 0).then_some(family as u8))
        .collect::<std::collections::BTreeSet<_>>();
    carried.insert(4);
    session.apply_carried_spell_families(&carried.into_iter().collect::<Vec<_>>(), Some(4)).unwrap();
    let holder = session.shared_spells().local_player.enabled_spell_holders[4];
    assert_ne!(holder, 0);
    let position = session.state.position;
    let castle = session.economy_mut().build_castle(position, parent).unwrap();
    assert!(session.economy_mut().set_castle_mana_prerequisite_for_test(castle, 25_000));
    {
        let live = &mut session.session.state.live;
        live.economy.refresh_owner_mana_rows();
        for row in &mut live.shared_spells.player_entities {
            if row.index == parent { row.set_mana(20_000); row.set_mana_regen(0); }
        }
        live.shared_spells.player_entity.set_mana(20_000);
        live.shared_spells.player_entity.set_mana_regen(0);
        live.shared_spells.caster.mana = 20_000;
        live.shared_spells.player_mana = 20_000;
        live.economy.entities[usize::from(parent)].set_mana(20_000);
        if let Some(fire) = live.core.context.spell_fire.as_mut() { fire.caster.mana = 20_000; }
    }
    for turn in 0..5 {
        session.step_outer_turn(if turn == 0 { crate::PlayerCommand {
            opcode: 6, argument_1: 4, flags: 0x40, ..crate::PlayerCommand::default()
        }} else { crate::PlayerCommand::default() }, 0);
        assert!(!session.dispatch_events().iter().any(|event|
            matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "ordinary Morph producer must not roll back: {:?}", session.dispatch_events());
        let slot = session.shared_spells().pickup(holder).unwrap().source_word_96();
        if slot != 0 {
            assert!(session.source_arena().bee_creatures().iter().any(|bee|
                bee.active && bee.source_entity_index == slot
                    && matches!(bee.origin, BeeCreatureOrigin::MorphSpell { holder_slot } if holder_slot == holder)));
            return (session, holder, slot);
        }
    }
    panic!("ordinary Morph cast did not produce its real Bee");
}

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
    assert_eq!(source.control_flags, 12, "NewEvent8 plus AddEventToMap4");
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
fn alliance_expiry_returns_linked_blink_clear_without_death_conversion() {
    let world = terrain();
    let mut source = bee();
    source.action = 23;
    source.stage_var2 = 14;
    source.stage_remaining = 1;
    source.stage_relation = 9;
    source.control_flags |= 0x0600_0000;
    source.linked_child_entity_index = 40;
    let effects = step(
        &mut source,
        &BeeActionContext {
            terrain: &world,
            actors: &[],
            stage: None,
        },
    );
    assert_eq!(effects.linked_blink_flags, Some(0));
    assert_eq!(source.control_flags & 0x0600_0000, 0);
    assert_eq!(
        (
            source.stage_var1,
            source.stage_var2,
            source.parent_entity_index
        ),
        (9, 10, 0)
    );
    assert!(source.active && source.drawing_enabled);
    assert!(!effects.death_conversion && !effects.expire_effect);
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

#[test]
fn owner_data_death_outputs_use_frozen_raw_replacements_in_source_order() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let mut arena = session.source_arena().clone();
    let mut world = session.session.state.live.terrain.clone();
    let position = session.state.position;
    let source = mc2::allocate_authored(&mut arena, &mut world, 229, position, 3000).unwrap();
    let particles = &session.session.definition.presentation.particles;
    for _ in 0..2 {
        crate::world::effects::allocate(
            &mut arena,
            &mut world,
            sim_definition::adapter::mc2_effect_definition(
                crate::level_definition::EffectKind::Impact,
            ),
            position,
            particles,
        )
        .unwrap()
        .expect("ordinary Impact creates a replaceable raw source carrier");
    }
    assert!(arena.economy.typed_replacement_candidate().is_none());
    let mut replacements = arena
        .entities
        .iter()
        .filter(|row| row.active && row.index != 0 && row.control_flags & 0x20000 != 0)
        .map(|row| {
            (
                row.index,
                arena.map_chains.tile_of(row.index),
                arena.slot_generation(row.index),
            )
        })
        .collect::<Vec<_>>();
    replacements.sort_unstable_by_key(|row| row.0);
    assert!(replacements.len() >= 2);
    arena.rebuild_source_free_arena();
    for _ in 0..arena.free_slot_count() {
        mc2::allocate_authored(&mut arena, &mut world, 230, position, 3000).unwrap();
    }
    let player = session.state.entity_index;
    arena
        .economy
        .source_owner
        .insert(source.source_entity_index, player);
    let dying = arena
        .economy
        .bee_creatures
        .iter_mut()
        .find(|row| row.source_entity_index == source.source_entity_index)
        .unwrap();
    dying.action = 21;
    dying.life = -1;
    dying.source_byte_3e = 8;
    let models = std::array::from_fn(|_| Vec::new());
    let mut exhausted = arena.clone();
    let mut exhausted_world = world.clone();
    exhausted.finish_source_group();
    assert_eq!(exhausted.free_slot_count(), 0);
    let failed = mc2::step_in_arena(
        &mut exhausted,
        &session.session.state.live.core.runtime,
        &mut exhausted_world,
        particles,
        source.source_entity_index,
        &models,
        &[],
        None,
        None,
    )
    .unwrap();
    assert!(failed.death_conversion);
    assert!(
        !exhausted.economy.events().iter().any(|event| matches!(
            event,
            crate::native_world::Level2EconomyEvent::DeathBurstSpawned { parent_entity_index, .. }
                if *parent_entity_index == source.source_entity_index
        )),
        "failed allocation must not publish a burst event"
    );
    let effects = mc2::step_in_arena(
        &mut arena,
        &session.session.state.live.core.runtime,
        &mut world,
        particles,
        source.source_entity_index,
        &models,
        &[],
        None,
        None,
    )
    .unwrap();
    assert!(effects.death_conversion);
    let sphere = arena
        .economy
        .spheres
        .iter()
        .find(|row| row.source_entity_index == replacements[0].0)
        .unwrap();
    assert_eq!((sphere.mana, sphere.owner_entity_index), (1500, player));
    let burst = &arena.entities[usize::from(replacements[1].0)];
    assert!(burst.active);
    assert_eq!(
        (burst.class, burst.model, burst.action, burst.source_id),
        (10, 1, 1, source.entity_id),
    );
    assert_eq!(
        (burst.max_life, burst.life, burst.subspell_index),
        (1, 1, 400)
    );
    assert_eq!(burst.particle_parameter_index, Some(41));
    assert_eq!(burst.control_flags, 0x20004);
    assert_eq!(
        arena
            .economy
            .events()
            .iter()
            .filter(|event| matches!(
                event,
                crate::native_world::Level2EconomyEvent::DeathBurstSpawned {
                    source_entity_index, parent_entity_index, id_entity_index,
                } if *source_entity_index == burst.index
                    && *parent_entity_index == source.source_entity_index
                    && *id_entity_index == source.entity_id
            ))
            .count(),
        1,
        "the actual raw radial explosion is published once to its death consumer"
    );
    assert!(!arena.entities[usize::from(replacements[0].0)].active);
    for &(slot, old_tile, generation) in &replacements[..2] {
        assert_eq!(arena.slot_generation(slot), generation.wrapping_add(1));
        assert_eq!(
            world.planes.entity_indices[usize::from(old_tile)],
            arena.map_chains.tile_head(old_tile) as i16
        );
        assert!(arena.map_chains.is_linked(slot));
    }
    let dying = arena
        .bee_creatures()
        .iter()
        .find(|row| row.source_entity_index == source.source_entity_index)
        .unwrap();
    assert!(dying.active && !dying.drawing_enabled);
    assert_eq!(dying.life, -1);
    assert_eq!(dying.entity_rng, level2_next_rng16(source.entity_rng));
    assert_eq!(arena.economy.source_owner(source.source_entity_index), 0);
    assert_eq!(
        arena.economy.source_rng[&source.source_entity_index],
        dying.entity_rng
    );

    let burst_slot = replacements[1].0;
    let burst_handle = arena.handle(burst_slot).unwrap();
    // The same live source with a different death parent has a different
    // future expiry consumer and must produce a different arena receipt.
    let mut other_parent = arena.clone();
    other_parent.effects.forget_source_slot(burst_slot);
    crate::world::effects::register_death_burst(
        &mut other_parent,
        burst_slot,
        source.source_entity_index.wrapping_add(1),
    )
    .unwrap();
    assert_eq!(
        arena.economy.reset_hash_with_terrain(&world),
        other_parent.economy.reset_hash_with_terrain(&world)
    );
    assert_ne!(
        arena.reset_hash_with_terrain(&world),
        other_parent.reset_hash_with_terrain(&world)
    );
    let mut replaced = arena.clone();
    let mut replaced_world = world.clone();
    replaced.rebuild_source_free_arena();
    for _ in 0..replaced.entities.len() {
        if replaced.resolve(burst_handle).is_none() {
            break;
        }
        if crate::world::effects::allocate(
            &mut replaced,
            &mut replaced_world,
            sim_definition::adapter::mc2_effect_definition(
                crate::level_definition::EffectKind::Impact,
            ),
            position,
            particles,
        )
        .unwrap()
        .is_none()
        {
            break;
        }
    }
    assert!(
        replaced.resolve(burst_handle).is_none(),
        "the frozen replacement allocator reuses the actual burst slot"
    );
    assert!(!replaced.effects.death_burst_origins().contains_key(&burst_slot));
    assert!(
        !replaced.economy.events().iter().any(|event| matches!(
            event,
            crate::native_world::Level2EconomyEvent::DeathBurstExpired {
                source_entity_index, parent_entity_index,
            } if *source_entity_index == burst_slot && *parent_entity_index == source.source_entity_index
        )),
        "replacement is not a burst expiry"
    );

    // The scheduler temporarily takes effect state while a different effect
    // can allocate children. Its allocator then sees the empty arena state,
    // so reconcile the taken state before putting it back.
    let mut cross_replaced = arena.clone();
    let mut cross_world = world.clone();
    let mut taken_state = std::mem::take(&mut cross_replaced.effects);
    cross_replaced.rebuild_source_free_arena();
    for _ in 0..cross_replaced.entities.len() {
        if cross_replaced.resolve(burst_handle).is_none() {
            break;
        }
        if crate::world::effects::allocate(
            &mut cross_replaced,
            &mut cross_world,
            sim_definition::adapter::mc2_effect_definition(
                crate::level_definition::EffectKind::Impact,
            ),
            position,
            particles,
        )
        .unwrap()
        .is_none()
        {
            break;
        }
    }
    assert!(cross_replaced.resolve(burst_handle).is_none());
    assert!(taken_state.death_burst_origins().contains_key(&burst_slot));
    taken_state.retain_current_generations(&cross_replaced);
    assert!(!taken_state.death_burst_origins().contains_key(&burst_slot));
    cross_replaced.effects = taken_state;
    assert!(
        !cross_replaced.economy.events().iter().any(|event| matches!(
            event,
            crate::native_world::Level2EconomyEvent::DeathBurstExpired {
                source_entity_index, parent_entity_index,
            } if *source_entity_index == burst_slot && *parent_entity_index == source.source_entity_index
        )),
        "reconciling an effect child replacement does not report expiry"
    );

    // sub_57F20 returns a slot without changing its generation. Reconcile
    // a temporarily taken effect state even before a future NewEvent reuse.
    let mut reclaimed = arena.clone();
    let mut reclaim_world = world.clone();
    let mut reclaimed_state = std::mem::take(&mut reclaimed.effects);
    crate::world::terrain::cave_features::reclaim(
        &mut reclaimed,
        &mut reclaim_world,
        burst_slot,
    );
    assert_eq!(reclaimed.slot_generation(burst_slot), burst_handle.generation);
    assert!(reclaimed.economy.free_source_slots().contains(&burst_slot));
    reclaimed_state.retain_current_generations(&reclaimed);
    assert!(!reclaimed_state.death_burst_origins().contains_key(&burst_slot));
    assert!(!reclaimed.economy.events().iter().any(|event| matches!(
        event,
        crate::native_world::Level2EconomyEvent::DeathBurstExpired {
            source_entity_index, parent_entity_index,
        } if *source_entity_index == burst_slot && *parent_entity_index == source.source_entity_index
    )));

    // Point the frozen fallback cursor at this actual Bee burst. During its
    // own action-1 visit its first successful child allocation can then
    // replace the callback's fixed address, which remains the source pointer.
    let mut self_replaced = arena.clone();
    let mut self_world = world.clone();
    self_replaced.rebuild_source_free_arena();
    loop {
        let mut probe = self_replaced.clone();
        let mut probe_world = self_world.clone();
        let candidate = crate::world::effects::allocate(
            &mut probe,
            &mut probe_world,
            sim_definition::adapter::mc2_effect_definition(
                crate::level_definition::EffectKind::Impact,
            ),
            position,
            particles,
        )
        .unwrap()
        .expect("frozen replacement cursor reaches the real burst");
        if candidate == burst_slot {
            break;
        }
        assert_eq!(
            crate::world::effects::allocate(
                &mut self_replaced,
                &mut self_world,
                sim_definition::adapter::mc2_effect_definition(
                    crate::level_definition::EffectKind::Impact,
                ),
                position,
                particles,
            )
            .unwrap(),
            Some(candidate)
        );
    }
    let seed = (0..=u16::MAX)
        .find(|seed| level2_next_rng16(*seed) % 157 >= 79)
        .unwrap();
    self_replaced.entities[usize::from(burst_slot)].set_entity_rng(seed);
    let mut state = std::mem::take(&mut self_replaced.effects);
    crate::world::effects::visit(
        &mut self_replaced,
        &mut self_world,
        burst_slot,
        particles,
        &mut state,
        |_, _| Ok(0),
    )
    .unwrap();
    self_replaced.effects = state;
    let current = &self_replaced.entities[usize::from(burst_slot)];
    assert!(self_replaced.resolve(burst_handle).is_none());
    assert_eq!((current.class, current.model, current.action), (10, 0, 0));
    assert_ne!(current.control_flags & 0x80, 0, "post-birth pointer write");
    assert!(!self_replaced.effects.death_burst_origins().contains_key(&burst_slot));
    assert!(!self_replaced.economy.events().iter().any(|event| matches!(
        event,
        crate::native_world::Level2EconomyEvent::DeathBurstExpired {
            source_entity_index, parent_entity_index,
        } if *source_entity_index == burst_slot && *parent_entity_index == source.source_entity_index
    )));

    for visit in 0..3 {
        let mut state = std::mem::take(&mut arena.effects);
        crate::world::effects::visit(
            &mut arena,
            &mut world,
            burst_slot,
            particles,
            &mut state,
            |_, _| Ok(0),
        )
        .unwrap();
        arena.effects = state;
        let expiries = arena
            .economy
            .events()
            .iter()
            .filter(|event| matches!(
                event,
                crate::native_world::Level2EconomyEvent::DeathBurstExpired {
                    source_entity_index, parent_entity_index,
                } if *source_entity_index == burst_slot && *parent_entity_index == source.source_entity_index
            ))
            .count();
        assert_eq!(expiries, usize::from(visit == 2));
    }
    assert!(arena.entities[usize::from(burst_slot)].drawing_disabled());
    assert!(
        arena.resolve(burst_handle).is_some(),
        "hide retains the actual generation until the next source boundary"
    );
}

#[test]
fn army_ring_allocates_a_generation_bound_summoned_bee_with_source_fields() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let mut arena = session.source_arena().clone();
    let mut runtime = session.session.state.live.core.runtime.clone();
    let mut terrain = session.session.state.live.terrain.clone();
    let particles = &session.session.definition.presentation.particles;
    let position = session.state.position;
    let parent_slot = session.state.entity_index;
    let owner_id = session.state.entity_index;
    let holder_slot = arena
        .allocate_source_slot()
        .expect("reserve the Army holder source slot");

    let handle = mc2::allocate_summoned(
        &mut arena,
        &mut runtime,
        &mut terrain,
        particles,
        position,
        parent_slot,
        owner_id,
        holder_slot,
    )
    .unwrap()
    .expect("Day-map Army Bee has source capacity");
    let source = arena
        .bee_creatures()
        .iter()
        .find(|source| source.source_entity_index == handle.source_slot)
        .expect("Army Bee remains in the canonical typed owner");
    assert_eq!(
        (source.source_class, source.model, source.action),
        (5, 2, 23)
    );
    assert_eq!(source.entity_id, owner_id);
    assert_eq!(source.parent_entity_index, parent_slot);
    assert_eq!(source.stage_var2, 13);
    assert_eq!(source.stage_remaining, 250);
    assert_eq!(source.mana, 0);
    assert_eq!(source.position, position);
    assert_eq!(
        source.origin,
        BeeCreatureOrigin::SummonedSpell { holder_slot }
    );
    assert!(arena.map_chains.is_linked(handle.source_slot));
    assert_eq!(handle.generation, arena.slot_generation(handle.source_slot));

    let empty = std::collections::BTreeMap::new();
    let identity = arena
        .creature_runtime_identity(handle.source_slot, &empty, &empty)
        .expect("summoned Bee resolves as a live creature root");
    assert_eq!(identity.handle, handle);
    assert_eq!(identity.kind, crate::level_definition::CreatureKind::Bee);
    assert_eq!(identity.root_slot, handle.source_slot);
    assert_eq!(
        identity.origin,
        crate::campaign::source_arena::CreatureRootOrigin::SummonedSpell { holder_slot }
    );
    assert_eq!(identity.raw_model, Some(2));
    assert!(identity.living);
}

#[test]
fn summoned_bee_expires_when_parent_slot_is_reused() {
    // Exercise the real acquired Morph producer as well as the Army path:
    // the same numeric player address may later name another incarnation.
    let (morph, _, morph_slot) = public_day_morph_bee();
    let morph_parent = morph.state.entity_index;
    let morph_handle = morph.source_arena().handle(morph_slot).unwrap();
    let original_parent = morph.source_arena().handle(morph_parent).unwrap();
    assert_eq!(morph.source_arena().creature_parent(morph_handle), Some(original_parent));
    for reused in [false, true] {
        let mut arena = morph.source_arena().clone();
        let mut terrain = morph.canonical_terrain().clone();
        if reused {
            arena.economy.release_slot(morph_parent, None);
            assert_eq!(arena.allocate_source_slot_with_terrain(&mut terrain), Some(morph_parent));
            let mut replacement = crate::native_world::cave_features::new_event(
                morph_parent, 5, morph.state.position,
            );
            replacement.class = 3;
            replacement.model = 0;
            replacement.record[67] = 3;
            replacement.set_life(1_000);
            arena.entities[usize::from(morph_parent)] = replacement;
        }
        let models: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
        mc2::step_in_arena(
            &mut arena, &morph.session.state.live.core.runtime, &mut terrain,
            &morph.session.definition.presentation.particles, morph_slot, &models,
            &[morph_parent], None, None,
        ).unwrap();
        let bee = arena.bee_creatures().iter()
            .find(|bee| bee.source_entity_index == morph_slot).unwrap();
        assert_eq!(bee.drawing_enabled, !reused, "Morph follows only its original parent");
    }

    let session = crate::native_world::tests::level_two_campaign_session();
    let mut arena = session.source_arena().clone();
    let mut runtime = session.session.state.live.core.runtime.clone();
    let mut terrain = session.session.state.live.terrain.clone();
    let particles = &session.session.definition.presentation.particles;
    let parent_slot = session.state.entity_index;
    let holder = arena.allocate_source_slot().unwrap();
    let child = mc2::allocate_summoned(
        &mut arena,
        &mut runtime,
        &mut terrain,
        particles,
        session.state.position,
        parent_slot,
        parent_slot,
        holder,
    )
    .unwrap()
    .unwrap();
    let parent = arena.handle(parent_slot).unwrap();
    assert_eq!(arena.creature_parent(child), Some(parent));
    arena.economy.release_slot(parent_slot, None);
    assert_eq!(
        arena.allocate_source_slot_with_terrain(&mut terrain),
        Some(parent_slot)
    );
    let mut replacement =
        crate::native_world::cave_features::new_event(parent_slot, 5, session.state.position);
    replacement.class = 3;
    replacement.model = 0;
    replacement.record[67] = 3;
    replacement.set_life(1000);
    arena.entities[usize::from(parent_slot)] = replacement;
    assert_eq!(arena.creature_parent(child), None);

    let models: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let effects = mc2::step_in_arena(
        &mut arena,
        &runtime,
        &mut terrain,
        particles,
        child.source_slot,
        &models,
        &[parent_slot],
        None,
        None,
    )
    .unwrap();
    assert!(effects.summon_expire_effect);
    assert!(
        !arena
            .economy
            .bee_creatures
            .iter()
            .find(|bee| bee.source_entity_index == child.source_slot)
            .unwrap()
            .drawing_enabled
    );
}

#[test]
fn army_ring_returns_none_when_the_shared_source_arena_is_exhausted() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let mut arena = session.source_arena().clone();
    let mut runtime = session.session.state.live.core.runtime.clone();
    let mut terrain = session.session.state.live.terrain.clone();
    let particles = &session.session.definition.presentation.particles;
    while arena.allocate_source_slot().is_some() {}

    let result = mc2::allocate_summoned(
        &mut arena,
        &mut runtime,
        &mut terrain,
        particles,
        session.state.position,
        session.state.entity_index,
        session.state.entity_index,
        session.state.entity_index,
    )
    .unwrap();

    assert_eq!(result, None);
}

#[test]
fn alliance_impact_syncs_only_the_exact_typed_bee_before_its_source_visit() {
    let mut session = crate::native_world::tests::level_two_campaign_session();
    let mut arena = session.source_arena().clone();
    let mut terrain = session.session.state.live.terrain.clone();
    let parent_slot = session.state.entity_index;
    let position = session.state.position;
    let center_slot = allocate_alliance_center(&mut arena, &mut terrain, position);

    let bee_slot =
        allocate_typed_morph_shadow(&mut arena, &mut terrain, parent_slot, position, parent_slot);
    let bee_handle = arena.handle(bee_slot).unwrap();
    let original_parent = arena.handle(center_slot).unwrap();
    let allied_parent = arena.handle(parent_slot).unwrap();
    assert!(arena.bind_creature_parent(bee_handle, original_parent));
    let impact = run_alliance_impact(&mut arena, &mut terrain, bee_slot, center_slot, parent_slot);
    assert_eq!(impact.affected_entities, vec![bee_slot]);
    assert_eq!(arena.creature_parent_binding(bee_handle), Some(allied_parent),
        "the actual Alliance impact takes over the live Bee's parent generation");
    let raw = &arena.entities[usize::from(bee_slot)];
    assert!(!raw.active, "the retained Bee row is only a source shadow");
    assert_eq!(raw.action, 18);
    assert_eq!(raw.record[76], 0);
    assert_eq!(raw.record[77], 14);
    assert_eq!(
        u16::from_le_bytes([raw.record[44], raw.record[45]]),
        parent_slot
    );
    assert_eq!(u16::from_le_bytes([raw.record[50], raw.record[51]]), 40);
    assert_eq!(u16::from_le_bytes([raw.record[52], raw.record[53]]), 40);
    assert_eq!(u16::from_le_bytes([raw.record[154], raw.record[155]]), 0);

    // Exercise the acquired typed actor's Alliance callback independently of
    // the Morph holder, whose own expiration can hide the Bee on the same pass.
    for parent_case in ["free", "reused", "dead"] {
        let mut controlled = arena.clone();
        let mut controlled_terrain = terrain.clone();
        controlled.economy.bee_creatures.iter_mut().find(|row| row.source_entity_index == bee_slot)
            .unwrap().action = 23;
        controlled.entities[usize::from(bee_slot)].set_source_action(23);
        if parent_case != "dead" {
            controlled.entities[usize::from(parent_slot)].reclaim_source_row();
            controlled.economy.release_slot(parent_slot, None);
            if parent_case == "reused" {
                assert_eq!(controlled.allocate_source_slot_with_terrain(&mut controlled_terrain), Some(parent_slot));
                let mut replacement = crate::native_world::cave_features::new_event(parent_slot, 5, position);
                replacement.class = 3;
                replacement.record[67] = 3;
                replacement.set_life(1_000);
                controlled.entities[usize::from(parent_slot)] = replacement;
                assert_ne!(controlled.handle(parent_slot), Some(allied_parent));
            }
        } else {
            controlled.entities[usize::from(parent_slot)].set_life(0);
        }
        let models: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
        mc2::step_in_arena(
            &mut controlled, &session.session.state.live.core.runtime, &mut controlled_terrain,
            &session.session.definition.presentation.particles, bee_slot, &models, &[], None, None,
        ).expect("Alliance Bee fixed-record parent visit");
        let bee = controlled.economy.bee_creatures.iter().find(|row| row.source_entity_index == bee_slot).unwrap();
        let expected = if parent_case == "dead" { (10, 0) } else { (14, parent_slot) };
        assert_eq!((bee.stage_var2, bee.parent_entity_index), expected, "{parent_case}");
        assert_eq!(controlled.handle(bee_slot), Some(bee_handle));
    }

    // The shadow control above is deliberately local. For the actual source
    // scheduler, obtain a Bee from an acquired/carry Morph holder: an invented
    // holder cannot satisfy the production semantic identity check.
    let (mut actual, holder, actual_bee) = public_day_morph_bee();
    let actual_parent = actual.state.entity_index;
    let actual_parent_handle = actual.source_arena().handle(actual_parent).unwrap();
    let actual_bee_handle = actual.source_arena().handle(actual_bee).unwrap();
    assert_eq!(actual.shared_spells().pickup(holder).unwrap().source_word_96(), actual_bee);
    let impact_slot = {
        let live = &mut actual.session.state.live;
        let impact_slot = crate::native_world::alliance_spell::construct_impact(&mut live.economy, &mut live.terrain).unwrap();
        let effect = &mut live.economy.entities[usize::from(impact_slot)];
        effect.source_id = actual_parent;
        effect.record[30..32].copy_from_slice(&actual_parent.to_le_bytes());
        effect.record[74] = 32;
        effect.subspell_index = 40;
        effect.record[154..156].copy_from_slice(&actual_bee.to_le_bytes());
        impact_slot
    };
    // Observe the real scheduled terminal at individual source boundaries:
    // it stays allocated for its hiding pass and is retired on the next.
    let mut boundary = actual.clone();
    let impact_handle = boundary.source_arena().handle(impact_slot).unwrap();
    assert!(!boundary.source_arena().economy.map_chains.is_linked(impact_slot));
    boundary.step_level2_source_pass(0);
    assert!(boundary.source_arena().resolve(impact_handle).is_some());
    assert!(boundary.source_arena().entities[usize::from(impact_slot)].drawing_disabled());
    assert!(!boundary.campaign_entity_presentations(0).unwrap().iter()
        .any(|row| row.entity_index == impact_slot),
        "the hidden Alliance terminal is absent from the real presentation consumer");
    boundary.step_level2_source_pass(1);
    assert!(boundary.source_arena().resolve(impact_handle).is_none(),
        "the next source boundary reclaims the original Alliance terminal generation");
    if boundary.source_arena().handle(impact_slot).is_none() {
        assert!(!boundary.source_arena().economy.map_chains.is_linked(impact_slot));
    }
    let before = actual.source_arena().bee_creatures().iter()
        .find(|row| row.source_entity_index == actual_bee).unwrap().clone();
    let before_map = actual.source_arena().economy.map_chains.entity_chain(
        (before.position.x >> 8) as u8, (before.position.y >> 8) as u8,
    );
    actual.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(!actual.dispatch_events().iter().any(|event|
        matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
        "actual scheduled Alliance impact must not roll back: {:?}", actual.dispatch_events());
    assert_eq!(actual.source_arena().creature_parent_binding(actual_bee_handle), Some(actual_parent_handle),
        "scheduled Alliance binding: before Bee={before:?}, map={before_map:?}, after Bee={:?}, effect={:?}, dispatch={:?}",
        actual.source_arena().bee_creatures().iter().find(|row| row.source_entity_index == actual_bee),
        actual.source_arena().entities.get(usize::from(impact_slot)), actual.dispatch_events());
    let actual_state = actual.source_arena().bee_creatures().iter()
        .find(|row| row.active && row.source_entity_index == actual_bee).unwrap();
    assert_eq!(actual_state.stage_var2, 14, "actual Morph Bee receives the scheduled Alliance impact");
    assert_eq!(actual_state.stage_duration, 40);
    assert!((1..=40).contains(&actual_state.stage_remaining));

    for parent_reused in [false, true] {
        let mut control = actual.clone();
        let mut controlled_arena = control.source_arena().clone();
        let mut controlled_terrain = control.canonical_terrain().clone();
        if parent_reused {
            controlled_arena.economy.release_slot(actual_parent, None);
            assert_eq!(controlled_arena.allocate_source_slot_with_terrain(&mut controlled_terrain), Some(actual_parent));
            let mut replacement = crate::native_world::cave_features::new_event(actual_parent, 5, position);
            replacement.class = 3;
            replacement.model = 0;
            replacement.record[67] = 3;
            replacement.set_life(1_000);
            controlled_arena.entities[usize::from(actual_parent)] = replacement;
            assert_ne!(controlled_arena.handle(actual_parent), Some(actual_parent_handle));
        }
        *control.source_arena_mut() = controlled_arena;
        *control.canonical_terrain_mut() = controlled_terrain;
        let mut visits = Vec::new();
        for turn in 0..2 {
            control.step_outer_turn(crate::PlayerCommand::default(), 0);
            assert!(!control.dispatch_events().iter().any(|event|
                matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
                "Bee Alliance source pass must execute without rollback: {:?}", control.dispatch_events());
            if turn == 0 {
                let arena = control.source_arena();
                let view = crate::native_world::source_target::read(
                    arena, Default::default(), actual_bee,
                ).unwrap().expect("the original Bee remains allocated for its first-pass tail");
                assert_eq!(view.drawing_disabled(), parent_reused,
                    "shared source targeting observes the actual same-pass hide");
                let archer = crate::native_world::level2_class5::archer::target_in_arena(arena, actual_bee)
                    .expect("the original Bee remains an Archer source target until reclaim");
                assert_eq!(archer.control_flags & crate::native_world::ENTITY_DRAW_DISABLED_BIT != 0,
                    parent_reused, "Archer targeting observes the same hidden state");
            }
            visits.push((turn, control.source_arena().handle(actual_bee),
                control.source_arena().bee_creatures().iter()
                    .find(|row| row.source_entity_index == actual_bee).copied(),
                control.shared_spells().pickup(holder).map(|row| (row.active_value, row.source_word_96()))));
        }
        if parent_reused {
            // The real Morph holder separately hides word96 when it expires.
            let first = visits[0].2.expect("Bee remains allocated for the hiding pass");
            assert_eq!(visits[0].1, Some(actual_bee_handle));
            assert!(!first.drawing_enabled, "Morph expiry hides its Bee on the first pass: {visits:?}");
            assert_eq!(visits[0].3, Some((0, 0)), "the Morph holder expires and clears word96");
            assert!(control.source_arena().resolve(actual_bee_handle).is_none(),
                "the following source boundary reclaims the original Bee generation");
            if control.source_arena().handle(actual_bee).is_none() {
                assert!(!control.source_arena().economy.map_chains.is_linked(actual_bee));
                assert!(!control.campaign_entity_presentations(0).unwrap().iter()
                    .any(|row| row.entity_index == actual_bee));
            }
        } else {
            let current = control.source_arena().bee_creatures().iter()
                .find(|row| row.active && row.source_entity_index == actual_bee)
                .unwrap_or_else(|| panic!("Alliance Bee disappeared with its parent live: {visits:?}"));
            assert_eq!(current.stage_var2, 14, "Alliance follows the current parent record");
        }
    }

    let first_remaining = actual.source_arena().bee_creatures().iter()
        .find(|row| row.active && row.source_entity_index == actual_bee).unwrap().stage_remaining;
    actual.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(!actual.dispatch_events().iter().any(|event|
        matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })));
    assert!(actual.source_arena().bee_creatures().iter()
        .find(|row| row.active && row.source_entity_index == actual_bee).unwrap().stage_remaining < first_remaining);

    let mut arena = actual.source_arena().clone();
    let mut terrain = actual.canonical_terrain().clone();
    let wrong_source_id = parent_slot.wrapping_add(1);
    let rejected_center_slot = allocate_alliance_center(&mut arena, &mut terrain, position);
    let rejected_slot = allocate_typed_morph_shadow(
        &mut arena,
        &mut terrain,
        parent_slot,
        position,
        wrong_source_id,
    );
    let rejected_handle = arena.handle(rejected_slot).unwrap();
    let retained_parent = arena.handle(rejected_center_slot).unwrap();
    assert!(arena.bind_creature_parent(rejected_handle, retained_parent));
    arena
        .economy
        .bee_creatures
        .iter_mut()
        .find(|source| source.active && source.source_entity_index == rejected_slot)
        .expect("mismatched-source Bee exists")
        .target_entity_index = None;
    let mut stale = mc2::alliance_projection(&arena, rejected_slot).unwrap();
    stale.source_id = wrong_source_id;
    stale.record[76] = 0;
    stale.record[77] = 14;
    stale.record[44..46].copy_from_slice(&parent_slot.to_le_bytes());
    assert!(!mc2::apply_alliance_takeover(&mut arena, rejected_handle, &stale, Some(allied_parent)).unwrap(),
        "a stale source identity cannot write the current typed Bee");
    assert_eq!(arena.creature_parent_binding(rejected_handle), Some(retained_parent),
        "a stale raw shadow cannot take over the typed Bee's parent");
    let rejected = arena
        .bee_creatures()
        .iter()
        .find(|source| source.active && source.source_entity_index == rejected_slot)
        .expect("the mismatched slot remains a typed Bee");
    assert_eq!(
        rejected.stage_var2, 12,
        "a different source id cannot import Alliance state"
    );
    assert_eq!(rejected.parent_entity_index, parent_slot);
    assert_eq!(arena.entities[usize::from(rejected_slot)].record[77], 12,
        "the rejected stale write does not change the retained source shadow");
}

#[test]
fn bee_stagevar_access_uses_source_count_without_compacting_sparse_rows() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let mut runtime = session.session.state.live.core.runtime.clone();
    let row = |kind, flags| crate::native_world::RuntimeStageVariable {
        kind,
        flags,
        word_2: 0,
        relation: crate::native_world::RuntimeRelation::Null,
    };

    runtime.stage_variables[1] = row(3, 6);
    runtime.stage_variables[2] = row(0, 6);
    runtime.stage_variables[3] = row(9, 6);
    runtime.stage_variables[4] = row(4, 6);
    assert!(runtime.set_active_stage_variable_count(3));

    assert_eq!(runtime.descriptor(0), None);
    assert_eq!(runtime.descriptor(2).map(|stage| stage.kind), Some(0));
    assert_eq!(runtime.descriptor(3).map(|stage| stage.kind), Some(9));
    assert_eq!(runtime.descriptor(4), None);

    runtime.rearm_model_relations();
    assert_eq!(runtime.stage_variables[1].flags, 2);
    assert_eq!(runtime.stage_variables[2].flags, 6);
    assert_eq!(runtime.stage_variables[3].flags, 2);
    assert_eq!(runtime.stage_variables[4].flags, 6);

    runtime.stage_variables[1].flags = 6;
    assert!(runtime.set_active_stage_variable_count(0));
    assert_eq!(runtime.descriptor(1), None);
    runtime.rearm_model_relations();
    assert_eq!(runtime.stage_variables[1].flags, 6);
}
