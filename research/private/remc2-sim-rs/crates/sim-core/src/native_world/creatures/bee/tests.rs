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
    let impact = run_alliance_impact(&mut arena, &mut terrain, bee_slot, center_slot, parent_slot);
    assert_eq!(impact.affected_entities, vec![bee_slot]);
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

    *session.source_arena_mut() = arena;
    *session.canonical_terrain_mut() = terrain;
    session.step_outer_turn(crate::PlayerCommand::default(), 0);
    let first_remaining;
    {
        let arena = session.source_arena();
        let bee = arena
            .bee_creatures()
            .iter()
            .find(|source| source.active && source.source_entity_index == bee_slot)
            .expect("the Bee remains in its typed owner");
        assert_eq!(
            bee.action, 17,
            "action18 consumes the cleared word96 target"
        );
        assert_eq!(bee.stage_var1, 0);
        assert_eq!(bee.stage_var2, 14);
        assert_eq!(bee.parent_entity_index, parent_slot);
        assert_eq!(bee.stage_relation, 0);
        assert!((1..=40).contains(&bee.stage_remaining));
        assert_eq!(bee.stage_duration, 40);
        first_remaining = bee.stage_remaining;
        let raw = &arena.entities[usize::from(bee_slot)];
        assert_eq!(raw.action, bee.action);
        assert_eq!(raw.record[77], bee.stage_var2);
        assert_eq!(
            u16::from_le_bytes([raw.record[50], raw.record[51]]),
            bee.stage_remaining as u16
        );
    }
    session.step_outer_turn(crate::PlayerCommand::default(), 0);
    let arena = session.source_arena();
    let bee = arena
        .bee_creatures()
        .iter()
        .find(|source| source.active && source.source_entity_index == bee_slot)
        .expect("the allied Bee remains live through its countdown");
    assert!(bee.stage_remaining < first_remaining);
    assert_eq!(
        u16::from_le_bytes([
            arena.entities[usize::from(bee_slot)].record[50],
            arena.entities[usize::from(bee_slot)].record[51],
        ]),
        bee.stage_remaining as u16
    );

    let mut arena = session.source_arena().clone();
    let mut terrain = session.canonical_terrain().clone();
    let wrong_source_id = parent_slot.wrapping_add(1);
    let rejected_center_slot = allocate_alliance_center(&mut arena, &mut terrain, position);
    let rejected_slot = allocate_typed_morph_shadow(
        &mut arena,
        &mut terrain,
        parent_slot,
        position,
        wrong_source_id,
    );
    arena
        .economy
        .bee_creatures
        .iter_mut()
        .find(|source| source.active && source.source_entity_index == rejected_slot)
        .expect("mismatched-source Bee exists")
        .target_entity_index = None;
    let rejected_impact = run_alliance_impact(
        &mut arena,
        &mut terrain,
        rejected_slot,
        rejected_center_slot,
        parent_slot,
    );
    assert_eq!(rejected_impact.affected_entities, vec![rejected_slot]);
    *session.source_arena_mut() = arena;
    *session.canonical_terrain_mut() = terrain;
    session.step_outer_turn(crate::PlayerCommand::default(), 0);
    let arena = session.source_arena();
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
    assert_eq!(
        arena.entities[usize::from(rejected_slot)].record[77],
        14,
        "mismatched source rows are not written back either"
    );
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
