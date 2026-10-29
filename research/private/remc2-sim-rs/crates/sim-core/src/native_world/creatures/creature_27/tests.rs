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
    families: [Vec<u16>; 29],
    target: u16,
    position: Position,
}

fn fixture() -> Fixture {
    let package = std::env::var_os("MC_SETTLEMENT_FULL_PACKAGE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-1pb-16-level2-full-build-v2")
        });
    let simulation = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::LEVEL2,
    )
    .expect("owner-exported Level 2 retains source constructor counters")
    .simulation()
    .clone();
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
    let families = std::array::from_fn(|model| {
        arena
            .entities
            .iter()
            .filter(|row| row.active && row.class == 5 && usize::from(row.model) == model)
            .map(|row| row.index)
            .collect()
    });
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
        families,
        target: target_slot,
        position,
    }
}

#[test]
fn legacy_runtime_without_constructor_counters_rejects_before_allocation() {
    let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2");
    let simulation = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::LEVEL2,
    )
    .expect("legacy Level 2 campaign remains readable")
    .simulation()
    .clone();
    let live = &simulation.session.state.live;
    let mut arena = live.economy.clone();
    let mut runtime = live.core.runtime.clone();
    let mut terrain = live.terrain.clone();
    let before = arena.economy.free_slot_count();
    assert!(runtime.constructor_counters().is_none());
    let error = mc2::allocate_authored(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &simulation.session.definition.presentation.particles,
        live.core.state.position,
        AUTHORED_RECORD,
    )
    .unwrap_err();
    assert!(error.contains("counter table is absent"), "{error}");
    assert_eq!(arena.economy.free_slot_count(), before);
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
        assert_eq!(head.record[60], 1);
        assert_eq!(head.record[61], 250);
        assert_eq!(head.life, 920 + 460 * (head_index as i32 * 10 + 1));
        assert_eq!(u16::from_le_bytes([head.record[136], head.record[137]]), 16);
        for body_handle in allocation.bodies[head_index] {
            let body = &fixture.arena.entities[usize::from(body_handle.source_slot)];
            assert_eq!(
                (body.action, body.record[63]),
                (BODY_ACTION, head_index as u8)
            );
            assert_eq!(body.record[60], 0);
            assert_eq!(body.record[61], 250);
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
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &fixture.class3,
        &fixture.families,
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
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &fixture.class3,
        &fixture.families,
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
fn root_tail_advances_all_heads_but_only_consumes_active_head_damage() {
    let mut fixture = fixture();
    let allocation = allocate(&mut fixture);
    let active = allocation.heads[0].source_slot;
    let dying = allocation.heads[1].source_slot;
    for (slot, state) in [(active, 1), (dying, 7)] {
        let head = &mut fixture.arena.entities[usize::from(slot)];
        head.record[74] = state;
        head.record[66] = 31;
        head.record[98..102].copy_from_slice(&100_i32.to_le_bytes());
        head.record[102..104].copy_from_slice(&fixture.target.to_le_bytes());
    }
    fixture.arena.entities[usize::from(dying)].record[72] = 7;
    fixture.arena.entities[usize::from(dying)].set_source_dword_10(2);
    fixture.arena.entities[usize::from(dying)].record[48..50].copy_from_slice(&1_u16.to_le_bytes());
    fixture.arena.entities[usize::from(dying)].record[136..138]
        .copy_from_slice(&16_i16.to_le_bytes());
    fixture.arena.entities[usize::from(dying)].set_actual_speed(16);
    fixture.arena.entities[usize::from(dying)].set_source_roll(200);
    fixture.arena.entities[usize::from(dying)].set_fov(300);
    let dying_life = fixture.arena.entities[usize::from(dying)].life;
    let dying_position = fixture.arena.entities[usize::from(dying)].position;
    let dying_pitch = fixture.arena.entities[usize::from(dying)].pitch;

    mc2::step_row(
        &mut fixture.arena,
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &fixture.class3,
        &fixture.families,
        7_680,
        2_048,
        0,
    )
    .unwrap();

    let active = &fixture.arena.entities[usize::from(active)];
    let dying = &fixture.arena.entities[usize::from(dying)];
    assert_eq!(active.record[66], 32);
    assert_eq!(active.phase, 32);
    assert_eq!(&active.record[102..104], &[0, 0]);
    assert_eq!(dying.record[66], 32);
    assert_eq!(dying.phase, 32);
    assert_eq!(dying.life, dying_life);
    // sub_29A90 case 7 calls sub_2A5B0 and then sub_2A340/2A940
    // (EventsFunctions.cpp:20007-20093). Its final state-7 branch omits
    // the second sub_2A940 (20196-20213). This independently applies the
    // second head's source row {440,110,600,407,1685} and one movement.
    let root = &fixture.arena.entities[usize::from(allocation.root.source_slot)];
    let mut expected_position = root.position;
    crate::move_entity(&mut expected_position, root.yaw.wrapping_add(110), 0, 440);
    expected_position.z = expected_position.z.wrapping_add(600);
    crate::move_entity(
        &mut expected_position,
        root.yaw.wrapping_add(407),
        root.pitch.wrapping_add(1685),
        672,
    );
    crate::move_entity(
        &mut expected_position,
        root.yaw.wrapping_add(200),
        root.pitch.wrapping_add(300),
        32,
    );
    assert_ne!(
        expected_position, dying_position,
        "state7 updates its root-relative pose"
    );
    assert_eq!(
        dying.position, expected_position,
        "state7 applies one movement after its source anchor"
    );
    assert_eq!(
        (dying.yaw, dying.pitch, dying.actual_speed),
        (root.yaw.wrapping_add(407 + 204), dying_pitch, 32)
    );
    assert_eq!(
        u16::from_le_bytes([dying.record[102], dying.record[103]]),
        fixture.target
    );
}

#[test]
fn head_state_zero_reuses_the_previous_heads_source_roll() {
    let mut fixture = fixture();
    let allocation = allocate(&mut fixture);
    let first = &mut fixture.arena.entities[usize::from(allocation.heads[0].source_slot)];
    first.record[74] = 1;
    first.record[66] = 7;
    first.set_entity_rng(2); // Second source RNG draw has low three bits zero.
    let next = &mut fixture.arena.entities[usize::from(allocation.heads[1].source_slot)];
    next.record[74] = 0;
    next.record[66] = 7;

    mc2::step_row(
        &mut fixture.arena,
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &[],
        &fixture.families,
        PROPERTIES.root_target_range,
        PROPERTIES.root_target_half_angle,
        0,
    )
    .unwrap();
    assert_eq!(
        fixture.arena.entities[usize::from(allocation.heads[1].source_slot)].record[74],
        1,
        "a zero roll scans the empty target list rather than selecting retreat"
    );
}

#[test]
fn stage_attack_transition_updates_the_root_and_linked_heads_in_one_pass() {
    let mut fixture = fixture();
    let allocation = allocate(&mut fixture);
    let root = &mut fixture.arena.entities[usize::from(allocation.root.source_slot)];
    root.set_source_action(223);
    root.record[77] = 10;
    root.record[154..156].copy_from_slice(&fixture.target.to_le_bytes());
    let head = &mut fixture.arena.entities[usize::from(allocation.heads[0].source_slot)];
    head.record[74] = 1;

    mc2::step_row(
        &mut fixture.arena,
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &fixture.class3,
        &fixture.families,
        PROPERTIES.root_target_range,
        PROPERTIES.root_target_half_angle,
        0,
    )
    .unwrap();

    let root = &fixture.arena.entities[usize::from(allocation.root.source_slot)];
    let head = &fixture.arena.entities[usize::from(allocation.heads[0].source_slot)];
    assert_eq!(root.action, 218);
    assert_eq!(root.record[77], 10);
    assert_eq!(head.record[74], 3);
    assert_eq!(
        u16::from_le_bytes([head.record[154], head.record[155]]),
        fixture.target
    );
}

#[test]
fn authored_point_stage_binds_and_advances_the_real_root() {
    let mut fixture = fixture();
    let allocation = allocate(&mut fixture);
    let position = fixture.arena.entities[usize::from(allocation.root.source_slot)].position;
    let point = Position {
        x: position.x.wrapping_add(4096),
        ..position
    };
    fixture.runtime.stage_variables[1] = crate::native_world::RuntimeStageVariable {
        kind: 1,
        flags: 0,
        word_2: 0,
        relation: crate::native_world::RuntimeRelation::AxisScalar(
            u32::from(point.x) | (u32::from(point.y) << 16),
        ),
    };
    assert!(fixture.runtime.set_active_stage_variable_count(1));
    mc2::bind_authored_stage(&mut fixture.arena, &mut fixture.runtime, allocation.root, 1).unwrap();
    let row = &fixture.arena.entities[usize::from(allocation.root.source_slot)];
    assert_eq!((row.action, row.record[76], row.record[77]), (223, 1, 1));

    mc2::step_row(
        &mut fixture.arena,
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &fixture.class3,
        &fixture.families,
        PROPERTIES.root_target_range,
        PROPERTIES.root_target_half_angle,
        0,
    )
    .unwrap();
    let row = &fixture.arena.entities[usize::from(allocation.root.source_slot)];
    assert_ne!(row.position, position);
    assert_eq!(row.action, 223);
}

#[test]
fn summoned_stage_expires_if_its_parent_is_missing_or_reused() {
    for reuse_parent in [false, true] {
        let mut fixture = fixture();
        let allocation = allocate(&mut fixture);
        if reuse_parent {
            let parent = fixture.arena.handle(fixture.target).unwrap();
            assert!(fixture.arena.bind_creature_parent(allocation.root, parent));
            fixture.arena.entities[usize::from(parent.source_slot)].reclaim_source_row();
            fixture.arena.economy.release_slot(parent.source_slot, None);
            assert_eq!(
                fixture.arena.allocate_source_slot(),
                Some(parent.source_slot)
            );
            assert_ne!(
                fixture.arena.slot_generation(parent.source_slot),
                parent.generation
            );
            let mut replacement = crate::world::terrain::cave_features::new_event(
                parent.source_slot,
                fixture.arena.new_event_rng(parent.source_slot),
                fixture.position,
            );
            replacement.class = 3;
            replacement.set_life(1000);
            fixture.arena.entities[usize::from(parent.source_slot)] = replacement;
        }
        let root = &mut fixture.arena.entities[usize::from(allocation.root.source_slot)];
        root.set_source_action(223);
        root.record[77] = 13;
        root.record[50..52].copy_from_slice(&250_i16.to_le_bytes());
        let free_before = fixture.arena.economy.free_slot_count();
        mc2::step_row(
            &mut fixture.arena,
            &mut fixture.runtime,
            &mut fixture.terrain,
            &fixture.particles,
            allocation.root.source_slot,
            &fixture.class3,
            &fixture.families,
            PROPERTIES.root_target_range,
            PROPERTIES.root_target_half_angle,
            0,
        )
        .unwrap();
        assert!(
            fixture.arena.entities[usize::from(allocation.root.source_slot)].drawing_disabled()
        );
        assert!(
            fixture.arena.economy.free_slot_count() < free_before,
            "expiry allocates its actual source effects"
        );
    }
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
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &fixture.class3,
        &fixture.families,
        7_680,
        2_048,
        0,
    )
    .unwrap();
    assert_eq!(effects.projectile_slots.len(), 1);
    let projectile = fixture
        .arena
        .economy
        .fireball_projectiles()
        .iter()
        .find(|projectile| projectile.source_entity_index == effects.projectile_slots[0])
        .expect("head allocated a live class9/model0 fireball");
    assert_eq!((projectile.source_class, projectile.source_model), (9, 0));
    assert_eq!(projectile.source_subspell_index, 850);
    assert_eq!(projectile.target_source_entity_index, fixture.target);
    assert_eq!(projectile.owner_entity_index, allocation.root.source_slot);

    {
        let head = &mut fixture.arena.entities[usize::from(head_slot)];
        head.record[72] = 3;
        head.mana_regen = 2;
        head.record[140..144].copy_from_slice(&2_i32.to_le_bytes());
        head.set_source_dword_10(1);
    }
    let effects = mc2::step_row(
        &mut fixture.arena,
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &fixture.class3,
        &fixture.families,
        7_680,
        2_048,
        0,
    )
    .unwrap();
    assert_eq!(effects.projectile_slots.len(), 1);
    let ray = &fixture.arena.entities[usize::from(effects.projectile_slots[0])];
    assert_eq!((ray.class, ray.model), (9, 9));
    assert_eq!(ray.parameter_relation, Some(PROJECTILE_DESCRIPTOR));
    assert_eq!(ray.subspell_index, 850);
    assert_eq!(ray.source_id, allocation.root.source_slot);
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
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &fixture.class3,
        &fixture.families,
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
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &fixture.class3,
        &fixture.families,
        7_680,
        2_048,
        0,
    )
    .unwrap();
    let outcome = death.death.expect("action221 runs source death products");
    let effect_slot = outcome
        .secondary_source_effect_slot
        .expect("ordinary root death allocates source type10/subtype1");
    let effect = fixture
        .arena
        .economy
        .death_bursts()
        .iter()
        .find(|effect| effect.source_entity_index == effect_slot)
        .expect("death burst is published to the live economy");
    assert_eq!(effect.id_entity_index, allocation.root.source_slot);
    assert!(!mc2::is_living_root(&fixture.arena, allocation.root));
    assert!(multipart::read_chain(&fixture.arena, allocation.root).is_err());
}

#[test]
fn prekill_parts_run_their_enabled_death_callbacks_in_the_same_pass() {
    let mut fixture = fixture();
    let allocation = allocate(&mut fixture);
    let chain = multipart::read_chain(&fixture.arena, allocation.root).unwrap();
    fixture.arena.entities[usize::from(allocation.root.source_slot)].set_source_action(220);
    mc2::step_row(
        &mut fixture.arena,
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &fixture.class3,
        &fixture.families,
        PROPERTIES.root_target_range,
        PROPERTIES.root_target_half_angle,
        0,
    )
    .unwrap();

    let first = chain.ordered[0];
    let second = chain.ordered[1];
    let first_death = mc2::step_row(
        &mut fixture.arena,
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        first.source_slot,
        &fixture.class3,
        &fixture.families,
        PROPERTIES.root_target_range,
        PROPERTIES.root_target_half_angle,
        0,
    )
    .unwrap()
    .death
    .unwrap();
    assert!(first_death.secondary_source_effect_slot.is_some());
    assert!(!fixture.arena.entities[usize::from(allocation.root.source_slot)].drawing_disabled());
    assert!(
        chain
            .ordered
            .iter()
            .all(|part| fixture.arena.entities[usize::from(part.source_slot)].drawing_disabled())
    );

    let second_death = mc2::step_row(
        &mut fixture.arena,
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        second.source_slot,
        &fixture.class3,
        &fixture.families,
        PROPERTIES.root_target_range,
        PROPERTIES.root_target_half_angle,
        0,
    )
    .unwrap()
    .death
    .unwrap();
    assert!(second_death.secondary_source_effect_slot.is_some());
}

#[test]
fn death_suffix_stops_at_a_reused_part_slot() {
    let mut fixture = fixture();
    let allocation = allocate(&mut fixture);
    fixture.arena.entities[usize::from(allocation.root.source_slot)].set_source_action(220);
    mc2::step_row(
        &mut fixture.arena,
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        allocation.root.source_slot,
        &fixture.class3,
        &fixture.families,
        PROPERTIES.root_target_range,
        PROPERTIES.root_target_half_angle,
        0,
    )
    .unwrap();

    let head = allocation.heads[0];
    let body = allocation.bodies[0][0];
    fixture.arena.entities[usize::from(body.source_slot)].reclaim_source_row();
    fixture.arena.economy.release_slot(body.source_slot, None);
    assert_eq!(fixture.arena.allocate_source_slot(), Some(body.source_slot));
    assert_ne!(
        fixture.arena.slot_generation(body.source_slot),
        body.generation
    );
    let mut replacement = crate::world::terrain::cave_features::new_event(
        body.source_slot,
        fixture.arena.new_event_rng(body.source_slot),
        fixture.position,
    );
    replacement.class = 5;
    replacement.model = 21;
    fixture.arena.entities[usize::from(body.source_slot)] = replacement;

    mc2::step_row(
        &mut fixture.arena,
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        head.source_slot,
        &fixture.class3,
        &fixture.families,
        PROPERTIES.root_target_range,
        PROPERTIES.root_target_half_angle,
        0,
    )
    .unwrap();
    assert!(fixture.arena.entities[usize::from(head.source_slot)].drawing_disabled());
    assert!(!fixture.arena.entities[usize::from(body.source_slot)].drawing_disabled());
    assert_eq!(
        fixture.arena.entities[usize::from(body.source_slot)].model,
        21
    );
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
