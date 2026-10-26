use super::*;
use crate::Position;
use crate::campaign::source_arena::{CreatureRootOrigin, CreatureRuntimeRole};
use crate::level_definition::CreatureKind;
use crate::native_world::ParticleParameter;

const AUTHORED_RECORD: u16 = 17;

struct Fixture {
    arena: crate::campaign::source_arena::SourceArena,
    runtime: crate::native_world::RuntimeSnapshot,
    terrain: crate::native_world::SessionTerrain,
    particles: Vec<ParticleParameter>,
    class3: Vec<u16>,
    target: u16,
    position: Position,
}

fn fixture() -> Fixture {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let live = &simulation.session.state.live;
    let arena = live.economy.clone();
    let target = arena
        .entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.model == 0 && row.life >= 0)
        .expect("Level 2 retains a living source target");
    let position = target.position;
    let target_slot = target.index;
    let class3 = arena
        .entities
        .iter()
        .filter(|row| row.active && row.class == 3 && row.life >= 0)
        .map(|row| row.index)
        .collect();
    Fixture {
        arena,
        runtime: live.core.runtime.clone(),
        terrain: live.terrain.clone(),
        particles: simulation
            .session
            .definition
            .presentation
            .particles
            .to_vec(),
        class3,
        target: target_slot,
        position,
    }
}

fn allocate(fixture: &mut Fixture) -> Allocation {
    mc2::allocate_authored(
        &mut fixture.arena,
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        fixture.position,
        AUTHORED_RECORD,
    )
    .expect("model27 source producer accepts retained Level 2 content")
    .expect("Level 2 has room for the source root and all 50 linked parts")
}

fn assert_map_linked(arena: &crate::campaign::source_arena::SourceArena, slot: u16) {
    let row = &arena.entities[usize::from(slot)];
    let tile = arena.economy.map_chains.tile_of(slot);
    let expected_tile = crate::renderable_map_chain::RenderableMapSeed {
        entity_index: slot,
        active: true,
        x: row.position.x,
        y: row.position.y,
    }
    .tile_index();
    assert_eq!(tile, expected_tile);
    assert!(
        arena
            .economy
            .map_chains
            .entity_chain(tile as u8, (tile >> 8) as u8)
            .contains(&slot),
        "model27 source slot {slot} must remain in its authoritative map chain"
    );
}

#[test]
fn authored_root_builds_and_advances_five_linked_nine_segment_chains() {
    let mut fixture = fixture();
    let prior_counter = fixture.runtime.constructor_counters().unwrap()[usize::from(SOURCE_MODEL)];
    let allocation = allocate(&mut fixture);
    let chain = multipart::read_chain(&fixture.arena, allocation.root).unwrap();
    let no_external_bindings = std::collections::BTreeMap::new();
    let root_identity = fixture
        .arena
        .creature_runtime_identity(
            allocation.root.source_slot,
            &no_external_bindings,
            &no_external_bindings,
        )
        .expect("authored model27 root retains typed generation identity");
    assert_eq!(root_identity.kind, CreatureKind::Creature27);
    assert_eq!(root_identity.role, CreatureRuntimeRole::Root);
    assert_eq!(
        root_identity.origin,
        CreatureRootOrigin::AuthoredRecord {
            authored_record: AUTHORED_RECORD,
        }
    );
    let head_identity = fixture
        .arena
        .creature_runtime_identity(
            allocation.heads[0].source_slot,
            &no_external_bindings,
            &no_external_bindings,
        )
        .expect("head resolves through its linked generation-bound root");
    assert_eq!(head_identity.handle, allocation.heads[0]);
    assert_eq!(head_identity.root_slot, allocation.root.source_slot);
    assert_eq!(head_identity.role, CreatureRuntimeRole::MultipartPart);
    assert_eq!(head_identity.origin, root_identity.origin);
    assert_eq!(chain.ordered.len(), 50);
    assert_map_linked(&fixture.arena, allocation.root.source_slot);
    for part in &chain.ordered {
        assert_map_linked(&fixture.arena, part.source_slot);
    }
    assert!(mc2::is_living_root(&fixture.arena, allocation.root));
    assert!(!mc2::is_living_root(&fixture.arena, allocation.heads[0]));
    assert_eq!(
        fixture.arena.entities[usize::from(allocation.root.source_slot)].action,
        217
    );
    assert_eq!(
        fixture.runtime.constructor_counters().unwrap()[usize::from(SOURCE_MODEL)],
        prior_counter.wrapping_add(1),
        "sub_2AC50 consumes counter27 once for the completed root"
    );
    for head_index in 0..PROPERTIES.head_count {
        let head = &fixture.arena.entities[usize::from(allocation.heads[head_index].source_slot)];
        assert_eq!(
            (head.action, head.record[63]),
            (HEAD_ACTION, head_index as u8)
        );
        for body_handle in allocation.bodies[head_index] {
            let body = &fixture.arena.entities[usize::from(body_handle.source_slot)];
            assert_eq!(
                (body.action, body.record[63]),
                (BODY_ACTION, head_index as u8)
            );
            assert_eq!(
                u16::from_le_bytes([body.record[54], body.record[55]]),
                allocation.root.source_slot
            );
        }
    }

    let last_segment = allocation.bodies[0][PROPERTIES.body_parts_per_head - 1].source_slot;
    let before = fixture.arena.entities[usize::from(last_segment)].position;
    let root_position_before =
        fixture.arena.entities[usize::from(allocation.root.source_slot)].position;
    let head = allocation.heads[0].source_slot;
    let head_life = fixture.arena.entities[usize::from(head)].life;
    fixture.arena.entities[usize::from(head)].record[98..102]
        .copy_from_slice(&100_i32.to_le_bytes());
    fixture.arena.entities[usize::from(head)].record[102..104]
        .copy_from_slice(&fixture.target.to_le_bytes());

    let effects = mc2::step_row(
        &mut fixture.arena,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &fixture.class3,
        7_680,
        2_048,
        0,
    )
    .unwrap();
    assert!(effects.callback_executed);
    assert_ne!(
        fixture.arena.entities[usize::from(allocation.root.source_slot)].position,
        root_position_before,
        "source action217 advances the root on its ordinary callback"
    );
    assert_eq!(
        fixture.arena.entities[usize::from(head)].life,
        head_life - 76
    );
    assert_eq!(
        &fixture.arena.entities[usize::from(head)].record[102..104],
        &[0, 0]
    );
    assert_eq!(
        i32::from_le_bytes(
            fixture.arena.entities[usize::from(allocation.root.source_slot)].record[98..102]
                .try_into()
                .unwrap()
        ),
        100,
        "the root receives the complete source damage mailbox"
    );
    let after = fixture.arena.entities[usize::from(last_segment)].position;
    assert_ne!(
        before, after,
        "the root same-pass tail poses the body behind its moving head"
    );

    mc2::step_row(
        &mut fixture.arena,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &fixture.class3,
        7_680,
        2_048,
        0,
    )
    .unwrap();
    let root = &fixture.arena.entities[usize::from(allocation.root.source_slot)];
    assert_eq!(root.action, 218);
    assert_eq!(
        u16::from_le_bytes([root.record[42], root.record[43]]),
        fixture.target
    );
}

#[test]
fn live_head_uses_the_linked_target_and_emits_a_real_source_projectile() {
    let mut fixture = fixture();
    let allocation = allocate(&mut fixture);
    {
        let root = &mut fixture.arena.entities[usize::from(allocation.root.source_slot)];
        root.set_source_action(218);
        root.record[154..156].copy_from_slice(&fixture.target.to_le_bytes());
    }

    let head_slot = allocation.heads[0].source_slot;
    let head = &mut fixture.arena.entities[usize::from(head_slot)];
    head.record[74] = 3;
    head.record[72] = 1;
    head.record[154..156].copy_from_slice(&fixture.target.to_le_bytes());
    head.set_actual_speed(192);
    head.set_entity_rng(0);

    let effects = mc2::step_row(
        &mut fixture.arena,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &fixture.class3,
        7_680,
        2_048,
        0,
    )
    .unwrap();
    assert_eq!(effects.projectiles.len(), 1);
    let projectile = effects.projectiles[0];
    assert_eq!(projectile.selector, 0);
    assert_eq!(projectile.descriptor, PROJECTILE_DESCRIPTOR);
    assert_eq!(projectile.source_subspell, 850);
    assert_eq!(projectile.root, allocation.root);
    assert_eq!(projectile.head, allocation.heads[0]);
    assert_eq!(projectile.target.source_slot, fixture.target);
    assert_eq!(projectile.source_owner, allocation.root.source_slot);

    {
        let head = &mut fixture.arena.entities[usize::from(head_slot)];
        head.record[72] = 3;
        head.mana_regen = 2;
        head.record[140..144].copy_from_slice(&2_i32.to_le_bytes());
        head.set_source_dword_10(1);
    }
    let effects = mc2::step_row(
        &mut fixture.arena,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &fixture.class3,
        7_680,
        2_048,
        0,
    )
    .unwrap();
    assert_eq!(effects.projectiles.len(), 1);
    assert_eq!(effects.projectiles[0].selector, 9);
}

#[test]
fn prekill_counts_once_then_death_hides_the_generation_bound_chain() {
    let mut fixture = fixture();
    let allocation = allocate(&mut fixture);
    let chain = multipart::read_chain(&fixture.arena, allocation.root).unwrap();
    let before = fixture
        .arena
        .economy
        .authored_bee_kill_census(fixture.target);
    let root = &mut fixture.arena.entities[usize::from(allocation.root.source_slot)];
    root.set_source_action(220);
    root.record[40..42].copy_from_slice(&fixture.target.to_le_bytes());

    let prekill = mc2::step_row(
        &mut fixture.arena,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &fixture.class3,
        7_680,
        2_048,
        0,
    )
    .unwrap();
    assert_eq!(
        prekill.kill_credit, None,
        "the adapter consumes credit exactly once"
    );
    assert_eq!(
        fixture
            .arena
            .economy
            .authored_bee_kill_census(fixture.target),
        before.wrapping_add(1)
    );
    assert!(!mc2::is_living_root(&fixture.arena, allocation.root));
    assert_eq!(
        fixture.arena.entities[usize::from(allocation.root.source_slot)].action,
        221
    );
    for part in chain.ordered {
        assert_eq!(
            fixture.arena.entities[usize::from(part.source_slot)].action,
            221
        );
    }

    let death = mc2::step_row(
        &mut fixture.arena,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &fixture.class3,
        7_680,
        2_048,
        0,
    )
    .unwrap();
    let outcome = death.death.expect("action221 runs source death products");
    let effect = outcome
        .secondary_source_effect
        .expect("ordinary root death requests source type10/subtype1");
    assert_eq!((effect.source_type, effect.source_subtype), (10, 1));
    assert_eq!(effect.owner_source_id, allocation.root.source_slot);
    assert!(!mc2::is_living_root(&fixture.arena, allocation.root));
    assert!(multipart::read_chain(&fixture.arena, allocation.root).is_err());
}

#[test]
fn linked_chain_rejects_a_part_with_the_wrong_root_owner() {
    let mut fixture = fixture();
    let allocation = allocate(&mut fixture);
    let first_body = allocation.bodies[0][0].source_slot;
    fixture.arena.entities[usize::from(first_body)].record[54..56].fill(0);
    let no_external_bindings = std::collections::BTreeMap::new();
    assert!(
        fixture
            .arena
            .creature_runtime_identity(first_body, &no_external_bindings, &no_external_bindings,)
            .is_none()
    );
    let error = multipart::read_chain(&fixture.arena, allocation.root).unwrap_err();
    assert!(error.contains("lost body segment 0 for head 0"), "{error}");
}
