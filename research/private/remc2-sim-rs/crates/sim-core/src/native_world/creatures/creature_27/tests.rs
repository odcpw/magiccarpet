use super::*;
use crate::campaign::source_arena::{CreatureRootOrigin, CreatureRuntimeRole};
use crate::level_definition::CreatureKind;
use crate::native_world::ParticleParameter;
use crate::{FlightWorld, Position};

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

fn owner_level23_model27() -> (crate::native_world::Mc2Simulation, u16) {
    use crate::PlayerCommand;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::native_world::Mc2DispatchEvent;

    let package = std::env::var_os("MC_LEVEL23_PACKAGE").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-ordinary-package-kxk0nb7m/level23")
        },
        std::path::PathBuf::from,
    );
    let campaign = CampaignSession::open(package, CampaignSelection::ordinary(23).unwrap())
        .expect("actual Level 23 owner package");
    let mut session = campaign.simulation().clone();
    let gate = session
        .entities()
        .iter()
        .find(|row| {
            row.active && (row.class, row.model, row.action, row.source_id) == (11, 32, 32, 16)
        })
        .cloned()
        .expect("owner record553 is the group16 objective switch");
    let gate_stage = usize::try_from(gate.level_gate).unwrap();
    assert!(gate_stage < usize::from(session.runtime().stage_index));

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !session
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
        "Level 23 turn before objective gate failed: {:?}",
        session.dispatch_events()
    );
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated { group: 16, .. }
    )));
    for objective in 0..session.runtime.objectives.len() {
        session
            .runtime
            .set_level2_objective_stage_selector_for(objective, gate_stage, 2);
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !session
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
        "Level 23 objective gate turn failed: {:?}",
        session.dispatch_events()
    );
    assert!(
        session.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { group: 16, authored_indices, .. }
                if authored_indices.contains(&560)
        )),
        "the owner objective switch must publish its actual model27 record"
    );
    let root_slot = session
        .entities()
        .iter()
        .find_map(|row| {
            let identity = session.creature_identity_for_source_slot(row.index)?;
            (identity.origin
                == CreatureRootOrigin::AuthoredRecord {
                    authored_record: 560,
                })
            .then_some(row.index)
        })
        .expect("ordinary group production must publish the living model27 root");
    let live = &session.session.state.live;
    let root = live.economy.handle(root_slot).unwrap();
    let chain = multipart::read_chain(&live.economy, root).unwrap();
    let root_phase = live.economy.entities[usize::from(root_slot)].phase;
    let head_phases = chain
        .heads
        .map(|head| live.economy.entities[usize::from(head.source_slot)].phase);
    assert_eq!(chain.ordered.len(), 50);
    session.step_outer_turn(PlayerCommand::default(), 0);
    let live = &session.session.state.live;
    assert_eq!(
        live.economy.entities[usize::from(root_slot)].phase,
        root_phase.wrapping_add(1)
    );
    for (head, phase) in chain.heads.into_iter().zip(head_phases) {
        assert_eq!(
            live.economy.entities[usize::from(head.source_slot)].phase,
            phase.wrapping_add(1)
        );
    }
    (session, root_slot)
}

#[test]
fn owner_level23_objective_gate_produces_model27_for_the_source_scheduler() {
    let _ = owner_level23_model27();
}

fn owner_level20_model27() -> (crate::native_world::Mc2Simulation, u16) {
    use crate::PlayerCommand;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::native_world::Mc2DispatchEvent;

    let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-bhq6-ordinary-20261024-b/level20");
    let campaign = CampaignSession::open(package, CampaignSelection::ordinary(20).unwrap())
        .expect("actual Level 20 owner package");
    let mut session = campaign.simulation().clone();
    let gate = session
        .entities()
        .iter()
        .find(|row| {
            row.active && (row.class, row.model, row.action, row.source_id) == (11, 29, 29, 1)
        })
        .expect("owner record327 is its model16-absence switch")
        .index;
    let wyverns = session
        .entities()
        .iter()
        .filter(|row| {
            row.active
                && row.class == 5
                && row.model == 16
                && row.life >= 0
                && !row.drawing_disabled()
        })
        .map(|row| row.index)
        .collect::<Vec<_>>();
    assert!(
        !wyverns.is_empty(),
        "the actual switch begins with its model16 blocker"
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(!session.dispatch_events().iter().any(|event| matches!(event,
        Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group: 1, .. } if *source_slot == gate
    )));
    // Controlled near-identical absence input; the countdown, group producer,
    // allocation and scheduler remain the actual owner-package path.
    for slot in wyverns {
        let row = &mut session.session.state.live.economy.entities[usize::from(slot)];
        row.set_life(-1);
        row.disable_drawing();
    }
    let mut produced = false;
    for _ in 0..20 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "source scheduler error: {:?}",
            session.dispatch_events()
        );
        produced |= session.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group: 1, authored_indices, .. }
                if *source_slot == gate && authored_indices.contains(&328)
        ));
        if produced {
            break;
        }
    }
    assert!(
        produced,
        "the source countdown must publish actual model27 record328"
    );
    let root = session
        .entities()
        .iter()
        .find_map(|row| {
            let identity = session.creature_identity_for_source_slot(row.index)?;
            (identity.origin
                == CreatureRootOrigin::AuthoredRecord {
                    authored_record: 328,
                })
            .then_some(identity.handle)
        })
        .expect("ordinary group1 production retains its actual typed root");
    let chain = multipart::read_chain(&session.session.state.live.economy, root).unwrap();
    assert_eq!(chain.ordered.len(), 50);
    (session, root.source_slot)
}

#[test]
fn owner_level20_absence_switch_produces_model27_after_wyverns_are_gone() {
    let _ = owner_level20_model27();
}

fn owner_level22_model27() -> (crate::native_world::Mc2Simulation, u16) {
    use crate::PlayerCommand;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::native_world::Mc2DispatchEvent;

    let package = std::env::var_os("MC_LEVEL22_PACKAGE").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261024-b/level22")
        },
        std::path::PathBuf::from,
    );
    let campaign = CampaignSession::open(package, CampaignSelection::ordinary(22).unwrap())
        .expect("actual Level 22 owner package");
    let mut session = campaign.simulation().clone();
    let root_slot = 2;
    let arena = &session.session.state.live.economy;
    let root = arena
        .handle(root_slot)
        .expect("owner group0 retains its initial root");
    let chain =
        multipart::read_chain(arena, root).expect("owner group0 retains its complete chain");
    let no_bindings = std::collections::BTreeMap::new();
    let part_identity = arena
        .creature_runtime_identity(chain.heads[0].source_slot, &no_bindings, &no_bindings)
        .expect("reset adopted the owner-package head's generation-bound root");
    assert_eq!(part_identity.root_slot, root_slot);
    assert_eq!(part_identity.origin, CreatureRootOrigin::InitialPackage);
    let mut malformed = arena.clone();
    malformed.clear_raw_creature_origin(root_slot);
    malformed.entities[usize::from(chain.heads[0].source_slot)].record[54..56].fill(0);
    assert!(
        mc2::adopt_imported(&mut malformed).is_err(),
        "reset must reject a wrong-owner head"
    );
    let mut prekill = arena.clone();
    prekill.clear_raw_creature_origin(root_slot);
    for part in std::iter::once(root).chain(chain.ordered.iter().copied()) {
        prekill.entities[usize::from(part.source_slot)].set_source_action(221);
    }
    mc2::adopt_imported(&mut prekill).expect("a valid visible prekill chain retains its bindings");
    assert_eq!(prekill.model27_part_root(chain.heads[0]), Some(root));
    prekill.entities[usize::from(chain.bodies[0][0].source_slot)].set_source_action(BODY_ACTION);
    prekill.clear_raw_creature_origin(root_slot);
    mc2::adopt_imported(&mut prekill)
        .expect("KillAll can leave body actions unchanged until the root's death callback");
    prekill.entities[usize::from(chain.bodies[0][0].source_slot)].set_source_action(HEAD_ACTION);
    prekill.clear_raw_creature_origin(root_slot);
    assert!(
        mc2::adopt_imported(&mut prekill).is_err(),
        "a body slot cannot have a head action even in a KillAll chain"
    );
    let root_phase = arena.entities[usize::from(root_slot)].phase;
    let root_proximity = arena.entities[usize::from(root_slot)].record[61];
    assert_eq!(
        root_proximity, 16,
        "owner Level22 retains its initial proximity countdown"
    );
    let head_phases = chain
        .heads
        .map(|head| arena.entities[usize::from(head.source_slot)].phase);
    assert_eq!(chain.ordered.len(), 50);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !session
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
        "source scheduler error: {:?}",
        session.dispatch_events()
    );
    let arena = &session.session.state.live.economy;
    assert_eq!(
        arena.entities[usize::from(root_slot)].phase,
        root_phase.wrapping_add(1)
    );
    assert_eq!(arena.entities[usize::from(root_slot)].record[61], 15);
    for (head, phase) in chain.heads.into_iter().zip(head_phases) {
        assert_eq!(
            arena.entities[usize::from(head.source_slot)].phase,
            phase.wrapping_add(1)
        );
        assert_eq!(arena.entities[usize::from(head.source_slot)].record[61], 15);
    }
    for body in chain.bodies.into_iter().flatten() {
        assert_eq!(arena.entities[usize::from(body.source_slot)].record[61], 16);
    }
    (session, root_slot)
}

#[test]
fn owner_level22_initial_multipart_chain_runs_in_the_source_scheduler() {
    let (session, root_slot) = owner_level22_model27();
    let arena = &session.session.state.live.economy;
    let chain = multipart::read_chain(arena, arena.handle(root_slot).unwrap()).unwrap();
    let presentations = session.campaign_entity_presentations(0).unwrap();
    let particle = |slot| {
        presentations
            .iter()
            .find(|row| row.entity_index == slot)
            .map(|row| row.render_parameter_index)
    };
    assert_eq!(particle(root_slot), Some(ROOT_PARTICLE));
    for head in chain.heads {
        assert_eq!(particle(head.source_slot), Some(HEAD_PARTICLE));
    }
    for body in chain.bodies.into_iter().flatten() {
        assert_eq!(particle(body.source_slot), Some(BODY_PARTICLE));
    }
}

fn assert_head_projectiles_reach_scheduled_damage_consumers(
    base: crate::native_world::Mc2Simulation,
    root_slot: u16,
) {
    use crate::PlayerCommand;
    use crate::native_world::Mc2DispatchEvent;

    let local_slot = base.state.entity_index;
    let target_slot = base
        .entities()
        .iter()
        .find(|row| {
            row.active
                && row.class == 3
                && row.model == 1
                && row.life > 0
                && !row.drawing_disabled()
        })
        .map_or(local_slot, |row| row.index);
    let mut cases = Vec::new();
    if target_slot == local_slot {
        assert!(base.session.state.live.player_protection_ticks > 24);
        let player = &base.entities()[usize::from(local_slot)];
        assert!(
            !base
                .session
                .state
                .live
                .economy
                .economy
                .player_castle_shift_aabb_overlap(player),
            "the near-identical protection control requires a player outside its Castle"
        );
        let mut expired = base.clone();
        let remaining = expired.session.state.live.player_protection_ticks;
        for _ in 0..remaining {
            expired.step_outer_turn(PlayerCommand::default(), 0);
            assert!(
                !expired
                    .dispatch_events()
                    .iter()
                    .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
                "source scheduler error while startup protection expires: {:?}",
                expired.dispatch_events()
            );
        }
        assert_eq!(expired.session.state.live.player_protection_ticks, 0);
        cases.push((true, base));
        cases.push((false, expired));
    } else {
        let initial = rival_upkeep(&base, target_slot);
        assert!(
            initial.word_0x159 > 0 && initial.high_upkeep,
            "rival protection: {initial:?}"
        );
        let mut expired = base.clone();
        let castle = expired
            .session
            .state
            .live
            .economy
            .economy
            .castle_for_owner(target_slot)
            .expect("the protected rival has its own actual Castle");
        assert_eq!(castle.source_entity_index, initial.castle_entity_index);
        let mut outside = expired.entities()[usize::from(target_slot)].position;
        outside.x = castle.position.x.wrapping_add(4096);
        outside.y = castle.position.y;
        expired
            .set_level2_entity_position(target_slot, outside)
            .unwrap();
        for _ in 0..160 {
            if rival_upkeep(&expired, target_slot).word_0x159 == 0 {
                break;
            }
            expired.step_outer_turn(PlayerCommand::default(), 0);
            assert!(
                !expired
                    .dispatch_events()
                    .iter()
                    .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
                "source scheduler error while rival protection expires: {:?}",
                expired.dispatch_events()
            );
        }
        let final_upkeep = rival_upkeep(&expired, target_slot);
        assert_eq!(
            final_upkeep.word_0x159, 0,
            "rival protection: {final_upkeep:?}"
        );
        cases.push((true, base));
        cases.push((false, expired));
    }
    let mut failures = Vec::new();
    for (protected_case, case_base) in cases {
        check_head_projectiles_in_case(
            &case_base,
            root_slot,
            target_slot,
            protected_case,
            &mut failures,
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn rival_upkeep(
    session: &crate::native_world::Mc2Simulation,
    target_slot: u16,
) -> crate::native_world::Level2NyphurCommonUpkeepSnapshot {
    let live = &session.session.state.live;
    let row = &live.economy.entities[usize::from(target_slot)];
    live.nyphur_dispatcher
        .get_in_arena(&live.economy, target_slot)
        .expect("owner rival has a generation-bound upkeep carrier")
        .common_upkeep(row.phase)
}

fn check_head_projectiles_in_case(
    case_base: &crate::native_world::Mc2Simulation,
    root_slot: u16,
    target_slot: u16,
    protected_case: bool,
    failures: &mut Vec<String>,
) {
    use crate::PlayerCommand;
    use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent};

    for selector in [0, 9] {
        let mut session = case_base.clone();
        let player_slot = session.state.entity_index;
        let target = session
            .entities()
            .iter()
            .find(|row| {
                row.index == target_slot
                    && row.active
                    && row.class == 3
                    && row.life > 0
                    && !row.drawing_disabled()
            })
            .cloned()
            .expect("owner level has a living player or rival target");
        let is_player = target_slot == player_slot;
        let target_life = if is_player {
            session.session.state.live.player_life
        } else {
            target.life
        };
        let mut root_position = if is_player {
            session.session.state.live.core.state.position
        } else {
            target.position
        };
        root_position.x = root_position.x.wrapping_sub(1024);
        root_position.z = session.canonical_terrain().ground_height(root_position);
        session
            .set_level2_entity_position(root_slot, root_position)
            .unwrap();
        if !is_player {
            let mut player_position = root_position;
            player_position.y = player_position.y.wrapping_add(3072);
            session
                .set_level2_entity_position(player_slot, player_position)
                .unwrap();
        }
        let head_slot = {
            let arena = &mut session.session.state.live.economy;
            let root = arena.handle(root_slot).unwrap();
            let chain = multipart::read_chain(arena, root).unwrap();
            let row = &mut arena.entities[usize::from(root_slot)];
            row.set_source_action(217);
            row.record[76..80].fill(0);
            // Keep the other source heads in their ordinary dormant regrowth
            // phase so their shots cannot satisfy this variant's damage test.
            for other in &chain.heads[1..] {
                let row = &mut arena.entities[usize::from(other.source_slot)];
                row.record[74] = 9;
                row.set_source_dword_10(100);
            }
            let head = &mut arena.entities[usize::from(chain.heads[0].source_slot)];
            head.record[74] = 3;
            head.record[72] = 0;
            head.record[154..156].copy_from_slice(&target_slot.to_le_bytes());
            head.set_actual_speed(0);
            chain.heads[0].source_slot
        };
        let mut saw_projectile = false;
        let mut saw_impact = false;
        let mut impact_observations = Vec::new();
        let mut saw_damage = false;
        let mut saw_consumed_life = false;
        let mut saw_any_life_loss = false;
        let mut previous_target_life = target_life;
        let mut fireball_slot = None;
        let mut saw_fireball_reclaim = false;
        let mut player_protection = Vec::new();
        let mut rival_protection = Vec::new();
        let mut callback_observations = Vec::new();
        let mut hit_in_prior_pass = false;
        let mut protected_after_hit = false;
        crate::native_world::area_damage::start_effect_damage_trace(target_slot);
        for _ in 0..24 {
            let head = &mut session.session.state.live.economy.entities[usize::from(head_slot)];
            if head.record[74] == 3 && head.record[72] == 1 && head.actual_speed == 192 {
                // The head first advances RNG for its source roll, then
                // advances it again to choose the attack variant.
                head.set_entity_rng(if selector == 0 { 0 } else { 6 });
            }
            session.step_outer_turn(PlayerCommand::default(), 0);
            assert!(
                !session
                    .dispatch_events()
                    .iter()
                    .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
                "source scheduler error: {:?}",
                session.dispatch_events()
            );
            let arena = &session.session.state.live.economy;
            saw_projectile |= if selector == 0 {
                let projectile = arena
                    .economy
                    .fireball_projectiles()
                    .iter()
                    .find(|shot| shot.owner_entity_index == root_slot);
                if let Some(shot) = projectile {
                    fireball_slot.get_or_insert(shot.source_entity_index);
                }
                projectile.is_some()
            } else {
                arena.entities.iter().any(|shot| {
                    shot.active && (shot.class, shot.model) == (9, 9) && shot.source_id == root_slot
                })
            };
            let target_row = &arena.entities[usize::from(target_slot)];
            if is_player && saw_projectile {
                player_protection.push((
                    arena.economy.player_castle_entity_index(),
                    arena.economy.player_castle_shift_aabb_overlap(target_row),
                    session.session.state.live.player_protection_ticks,
                    session.session.state.live.shared_spells.shield_state(),
                ));
            } else if !is_player && saw_projectile {
                let upkeep = rival_upkeep(&session, target_slot);
                rival_protection.push((
                    upkeep.castle_entity_index,
                    upkeep.high_upkeep,
                    upkeep.word_0x159,
                    upkeep.life,
                ));
            }
            let impact_model = if selector == 0 { 0 } else { 23 };
            if let Some(effect) = arena.entities.iter().find(|effect| {
                effect.active
                    && (effect.class, effect.model, effect.source_id)
                        == (10, impact_model, root_slot)
            }) {
                saw_impact = true;
                impact_observations.push((
                    effect.index,
                    effect.position,
                    u16::from_le_bytes([effect.record[154], effect.record[155]]),
                    target_row.position,
                    (effect.control_flags, effect.record[69], effect.record[70]),
                    (
                        effect.life,
                        effect.phase,
                        effect.source_dword_10(),
                        effect.subspell_index,
                    ),
                    (effect.array_pitch(), effect.array_yaw(), effect.array_fov()),
                    crate::native_world::area_damage::read_arena_actor(arena, target_slot),
                    (
                        arena.economy.map_chains.tile_of(target_slot),
                        arena
                            .economy
                            .map_chains
                            .tile_head(arena.economy.map_chains.tile_of(target_slot)),
                    ),
                ));
            }
            let attacker = u16::from_le_bytes([target_row.record[102], target_row.record[103]]);
            let payload = i32::from_le_bytes(target_row.record[98..102].try_into().unwrap());
            saw_damage |= attacker == root_slot && payload > 0;
            saw_damage |= session.dispatch_events().iter().any(|event| matches!(event,
                Mc2DispatchEvent::Economy { event: Level2EconomyEvent::DamageApplied { target_entity_index, attacker_entity_index, damage, .. }, .. }
                    if *target_entity_index == target_slot && *attacker_entity_index == root_slot && *damage > 0
            ));
            let current_life = if is_player {
                session.session.state.live.player_life
            } else {
                target_row.life
            };
            let last_rival_attacker =
                u16::from_le_bytes([target_row.record[42], target_row.record[43]]);
            saw_any_life_loss |= current_life < target_life;
            saw_consumed_life |= current_life < target_life
                && current_life < previous_target_life
                && (is_player || last_rival_attacker == root_slot);
            previous_target_life = current_life;
            protected_after_hit |= protected_case && hit_in_prior_pass;
            let receipts = crate::native_world::area_damage::take_effect_damage_trace();
            let hit_this_pass = receipts.iter().any(|receipt| {
                receipt.pulse.owner == root_slot
                    && receipt.target_slot == target_slot
                    && receipt.hit_count.is_some_and(|hits| hits > 0)
                    && receipt
                        .mailbox_after
                        .is_some_and(|(payload, owner)| payload > 0 && owner == root_slot)
            });
            saw_damage |= hit_this_pass;
            hit_in_prior_pass |= hit_this_pass;
            callback_observations.extend(receipts);
            if let Some(slot) = fireball_slot
                && arena.economy.free_source_slots().contains(&slot)
            {
                let residual = &arena.entities[usize::from(slot)];
                assert_eq!(residual.parameter_relation, Some(106));
                assert_eq!(
                    u32::from_le_bytes(residual.record[164..168].try_into().unwrap()),
                    107
                );
                saw_fireball_reclaim = true;
            }
            if saw_projectile
                && saw_impact
                && saw_damage
                && if protected_case {
                    protected_after_hit && !saw_any_life_loss
                } else {
                    saw_consumed_life
                }
                && (selector != 0 || saw_fireball_reclaim)
            {
                break;
            }
            crate::native_world::area_damage::start_effect_damage_trace(target_slot);
        }
        callback_observations.extend(crate::native_world::area_damage::take_effect_damage_trace());
        saw_damage |= callback_observations.iter().any(|receipt| {
            receipt.pulse.owner == root_slot
                && receipt.target_slot == target_slot
                && receipt.hit_count.is_some_and(|hits| hits > 0)
                && receipt
                    .mailbox_after
                    .is_some_and(|(payload, owner)| payload > 0 && owner == root_slot)
        });
        let protection_matches = if is_player {
            !player_protection.is_empty()
                && player_protection.iter().all(|(_, overlap, counter, _)| {
                    !*overlap
                        && if protected_case {
                            *counter > 0
                        } else {
                            *counter == 0
                        }
                })
        } else {
            !rival_protection.is_empty()
                && rival_protection.iter().all(|(_, high_upkeep, counter, _)| {
                    if protected_case {
                        *high_upkeep && *counter > 0
                    } else {
                        !*high_upkeep && *counter == 0
                    }
                })
        };
        if !is_player {
            eprintln!(
                "model27 selector {selector}, protected={protected_case}, rival protection first/last (castle, high upkeep, timer, life): {:?} / {:?}",
                rival_protection.first(),
                rival_protection.last()
            );
        }
        if !saw_projectile
            || !saw_impact
            || !saw_damage
            || !protection_matches
            || (protected_case && !protected_after_hit)
            || (protected_case && saw_any_life_loss)
            || (!protected_case && !saw_consumed_life)
            || (selector == 0 && !saw_fireball_reclaim)
        {
            failures.push(format!(
            "selector {selector}, protected={protected_case}: projectile={saw_projectile}, root-attributed impact={saw_impact}, protection matches={protection_matches}, \
             fireball reclaim={saw_fireball_reclaim}; must deliver root-attributed damage to target {target_slot} \
             (player={is_player}, original life={target_life}, \
             damage observed={saw_damage}, protected visit after hit={protected_after_hit}, root-attributed life consumed={saw_consumed_life}, any life loss={saw_any_life_loss}, \
             impacts(slot, position, target, target position, flags/selectors, life/phase/age/payload, axes, target actor, target tile/head)={impact_observations:?}); \
             player protection(castle, overlap, timer, shield)={player_protection:?}; \
             rival protection(castle, high upkeep, timer, life)={rival_protection:?}; \
             callback observations={callback_observations:?}; final target={:?}",
                session.entities().get(usize::from(target_slot))
            ));
        }
    }
}

#[test]
fn owner_model27_head_projectiles_reach_scheduled_damage_consumers() {
    let (session, root) = owner_level23_model27();
    assert_head_projectiles_reach_scheduled_damage_consumers(session, root);
}

#[test]
fn owner_level22_model27_projectiles_reach_scheduled_damage_consumers() {
    let (session, root) = owner_level22_model27();
    assert_head_projectiles_reach_scheduled_damage_consumers(session, root);
}

#[test]
fn owner_level22_area_damage_reaches_linked_head_and_root_mailboxes() {
    use crate::combat::area::{AreaDamage, Shape};
    use crate::native_world::{Mc2DispatchEvent, area_damage};
    let (mut session, root_slot) = owner_level22_model27();
    let attacker = session.state.entity_index;
    let arena = &mut session.session.state.live.economy;
    let root = arena.handle(root_slot).unwrap();
    let chain = multipart::read_chain(arena, root).unwrap();
    let head_slot = chain.heads[0].source_slot;
    let body_slot = chain.bodies[0][0].source_slot;
    let head = area_damage::read_arena_actor(arena, head_slot).unwrap();
    let body = area_damage::read_arena_actor(arena, body_slot).unwrap();
    assert_eq!(head.damage_mask & 1, 1);
    assert_eq!(body.damage_mask & 1, 0);
    let mut position = head.position;
    position.z = position.z.wrapping_add(head.shape.vertical_offset);
    let hits = area_damage::apply(
        arena,
        None,
        None,
        AreaDamage {
            position,
            shape: Shape {
                half_width: 128,
                half_depth: 128,
                vertical_offset: 0,
                half_height: 128,
            },
            owner: attacker,
            payload: 76,
            selector: (5, 27),
            mode: 0,
        },
    )
    .unwrap();
    assert!(hits > 0);
    let head = &arena.entities[usize::from(head_slot)];
    assert_eq!(
        u16::from_le_bytes([head.record[102], head.record[103]]),
        attacker
    );
    let body = &arena.entities[usize::from(body_slot)];
    assert_eq!(u16::from_le_bytes([body.record[102], body.record[103]]), 0);
    session.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(
        !session
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, Mc2DispatchEvent::Error { .. }))
    );
    let root = &session.entities()[usize::from(root_slot)];
    assert_eq!(
        u16::from_le_bytes([root.record[102], root.record[103]]),
        attacker
    );
    assert!(i32::from_le_bytes(root.record[98..102].try_into().unwrap()) > 0);
}

#[test]
fn owner_level20_model27_projectiles_reach_scheduled_damage_consumers() {
    let (session, root) = owner_level20_model27();
    assert_head_projectiles_reach_scheduled_damage_consumers(session, root);
}

fn assert_linked_damage_death_and_next_pass_reclaim_all_slots(
    mut session: crate::native_world::Mc2Simulation,
    root_slot: u16,
    target_objective_stage: Option<usize>,
) {
    use crate::PlayerCommand;
    use crate::campaign::schedule::ScheduleReceipt;
    use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent, area_damage};

    let attacker = session.state.entity_index;
    let objective_owner = usize::from(session.players()[0].player_color_index());
    if let Some(stage_index) = target_objective_stage {
        use crate::native_world::RuntimeRelation;

        let stage = &session.runtime().stages[stage_index];
        assert_eq!(stage.kind, 1, "owner task is the source target-death form");
        assert_eq!(stage.flags & 1, 1, "the real producer binds the target");
        assert_eq!(stage.relation, RuntimeRelation::LiveEntity(root_slot));
        assert_eq!(session.runtime().objectives[objective_owner].done, 0);
        assert_eq!(
            session.runtime().objectives[objective_owner].is_level_end,
            0
        );
        assert_eq!(
            session.runtime().objectives[objective_owner].stage_selectors[stage_index],
            1
        );
        assert!(session.entities()[usize::from(root_slot)].life > -1);

        // The near-identical survivor follows the same real objective pass
        // without the head-damage stimulus used by the death case below.
        let mut survivor = session.clone();
        survivor.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !survivor
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "survivor scheduler error: {:?}",
            survivor.dispatch_events()
        );
        assert!(survivor.entities()[usize::from(root_slot)].life > -1);
        assert_eq!(
            survivor.runtime().objectives[objective_owner].stage_selectors[stage_index],
            1,
            "the actual target-death objective must remain open while its bound root lives"
        );
    }
    let (root, chain) = {
        let arena = &mut session.session.state.live.economy;
        let root = arena.handle(root_slot).unwrap();
        let chain = multipart::read_chain(arena, root).unwrap();
        // Start near the end of the real heads' health so the focused test
        // can exercise the source death lifecycle without playing a mission.
        for head in chain.heads {
            arena.entities[usize::from(head.source_slot)].set_life(1);
            area_damage::contact(arena, None, head.source_slot, 76, attacker).unwrap();
        }
        (root, chain)
    };
    let mut defeated = false;
    for _ in 0..64 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "source scheduler error: {:?}",
            session.dispatch_events()
        );
        let row = &session.session.state.live.economy.entities[usize::from(root_slot)];
        if row.record[63] == 0 {
            defeated = true;
            break;
        }
    }
    assert!(
        defeated,
        "all five damaged heads must complete their source death phases"
    );
    area_damage::contact(
        &mut session.session.state.live.economy,
        None,
        root_slot,
        76,
        attacker,
    )
    .unwrap();
    let parts = std::iter::once(root)
        .chain(chain.ordered.iter().copied())
        .collect::<Vec<_>>();
    let mut hidden_at = vec![None; parts.len()];
    let mut reclaimed_at = vec![None; parts.len()];
    let mut burst_slot = None;
    let mut burst_expired = false;
    let mut death_passes = Vec::new();
    let mut death_events = Vec::new();
    let mut death_schedule = Vec::new();
    let mut group_rebuilds = Vec::new();
    let mut target_objective_completed = false;
    for pass in 0..8 {
        combat::start_death_hide_trace();
        session.step_outer_turn(PlayerCommand::default(), 0);
        let hide_receipts = combat::take_death_hide_trace();
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "source scheduler error: {:?}",
            session.dispatch_events()
        );
        death_schedule.extend(
            session
                .cf10_production_schedule_receipts()
                .into_iter()
                .filter(|receipt| {
                    matches!(receipt,
                    ScheduleReceipt::SlotResolved { slot, .. }
                    | ScheduleReceipt::CallbackCompleted { slot, .. }
                    | ScheduleReceipt::EpilogueCompleted { slot, .. }
                    | ScheduleReceipt::CleanupCompleted { slot, .. }
                    if *slot == root_slot || *slot == chain.heads[0].source_slot)
                }),
        );
        let mut group_rebuilt = false;
        for event in session.dispatch_events() {
            if matches!(event, Mc2DispatchEvent::AuthoredGroupActivated { .. }) {
                group_rebuilt = true;
                death_events.push(format!("{event:?}"));
            }
            match event {
                Mc2DispatchEvent::Economy {
                    event:
                        Level2EconomyEvent::DeathBurstSpawned {
                            source_entity_index,
                            parent_entity_index,
                            ..
                        },
                    ..
                } if *parent_entity_index == root_slot => burst_slot = Some(*source_entity_index),
                Mc2DispatchEvent::Economy {
                    event:
                        Level2EconomyEvent::DeathBurstExpired {
                            source_entity_index,
                            parent_entity_index,
                        },
                    ..
                } if *parent_entity_index == root_slot
                    && Some(*source_entity_index) == burst_slot =>
                {
                    burst_expired = true;
                }
                _ => {}
            }
        }
        group_rebuilds.push(group_rebuilt);
        let arena = &session.session.state.live.economy;
        if target_objective_stage.is_some_and(|stage_index| {
            session.runtime().objectives[objective_owner].stage_selectors[stage_index] == 2
        }) && !target_objective_completed
        {
            assert_eq!(arena.slot_generation(root_slot), root.generation);
            assert_eq!(
                arena.entities[usize::from(root_slot)].life,
                -1,
                "the real objective must observe its bound original root dead"
            );
            target_objective_completed = true;
        }
        let all = parts.iter().copied();
        death_passes.push((
            (
                arena.entities[usize::from(root_slot)].active,
                arena.entities[usize::from(root_slot)].class,
                arena.entities[usize::from(root_slot)].model,
                arena.entities[usize::from(root_slot)].action,
                arena.entities[usize::from(root_slot)].control_flags,
                arena.slot_generation(root_slot),
                arena.economy.free_source_slots().contains(&root_slot),
            ),
            all.clone()
                .filter(|part| {
                    arena.slot_generation(part.source_slot) == part.generation
                        && arena.entities[usize::from(part.source_slot)].drawing_disabled()
                })
                .count(),
            all.clone()
                .filter(|part| {
                    arena.slot_generation(part.source_slot) != part.generation
                        || arena
                            .economy
                            .free_source_slots()
                            .contains(&part.source_slot)
                })
                .count(),
        ));
        let presentations = session.campaign_entity_presentations(0).unwrap();
        let current_root = &arena.entities[usize::from(root_slot)];
        if arena.slot_generation(root_slot) != root.generation
            && current_root.active
            && (current_root.class, current_root.model) == (5, SOURCE_MODEL)
        {
            let identity = session
                .creature_identity_for_source_slot(root_slot)
                .expect("a reused model27 root must retain its new source identity");
            assert_ne!(identity.handle.generation, root.generation);
            assert!(
                !current_root.drawing_disabled(),
                "old death must not hide the reused root"
            );
            multipart::read_chain(arena, identity.handle)
                .expect("reused model27 root must own its complete new chain");
        }
        for (index, part) in parts.iter().copied().enumerate() {
            let same_generation = arena.slot_generation(part.source_slot) == part.generation;
            let hidden_now =
                same_generation && arena.entities[usize::from(part.source_slot)].drawing_disabled();
            let hidden_in_callback = hide_receipts
                .iter()
                .any(|receipt| receipt.hidden.contains(&part) && parts.contains(&receipt.callback));
            if hidden_now || hidden_in_callback {
                hidden_at[index].get_or_insert(pass);
            }
            let reclaimed_now = !same_generation
                || arena
                    .economy
                    .free_source_slots()
                    .contains(&part.source_slot);
            if reclaimed_now {
                assert!(
                    hidden_at[index].is_some(),
                    "original model27 slot {} was reclaimed before its death hide",
                    part.source_slot
                );
                reclaimed_at[index].get_or_insert(pass);
            } else if hidden_now {
                assert!(
                    presentations
                        .iter()
                        .all(|row| row.entity_index != part.source_slot),
                    "hidden model27 slot {} must disappear from presentation",
                    part.source_slot
                );
            }
        }
        if reclaimed_at.iter().all(Option::is_some) && burst_expired {
            break;
        }
    }
    assert!(
        hidden_at.iter().all(Option::is_some),
        "the source death callbacks hide every original linked slot before reclaim; \
         per outer turn (root state, hidden, reclaimed): {death_passes:?}; \
         authored groups during death: {death_events:?}; \
         root/first-head schedule: {death_schedule:?}"
    );
    assert!(
        reclaimed_at.iter().all(Option::is_some),
        "source boundaries reclaim every original linked slot: hidden={hidden_at:?}, reclaimed={reclaimed_at:?}"
    );
    for (index, part) in parts.iter().enumerate() {
        let hidden_pass = hidden_at[index].unwrap();
        let reclaimed_pass = reclaimed_at[index].unwrap();
        assert!(
            reclaimed_pass == hidden_pass + 1
                || (reclaimed_pass == hidden_pass && group_rebuilds[hidden_pass]),
            "slot {} was hidden in pass {hidden_pass} and reclaimed in pass {reclaimed_pass} without the matching next boundary or an authored group rebuild",
            part.source_slot
        );
    }
    assert!(
        burst_slot.is_some(),
        "root death allocates its real class10 burst"
    );
    assert!(
        burst_expired,
        "the source scheduler consumes that same death burst"
    );
    assert!(
        target_objective_stage.is_none() || target_objective_completed,
        "the actual owner task must observe its bound root's death"
    );
}

#[test]
fn owner_model27_linked_damage_death_and_next_pass_reclaim_all_slots() {
    let (session, root) = owner_level23_model27();
    assert_linked_damage_death_and_next_pass_reclaim_all_slots(session, root, None);
}

#[test]
fn owner_level22_model27_linked_damage_death_and_reclaim_all_slots() {
    let (session, root) = owner_level22_model27();
    assert_linked_damage_death_and_next_pass_reclaim_all_slots(session, root, Some(0));
}

#[test]
fn owner_level22_summoned_feedback_reaches_only_the_human_player_progression() {
    use crate::PlayerCommand;
    use crate::native_world::{Mc2DispatchEvent, area_damage};

    let (base, root_slot) = owner_level22_model27();
    for (mode, family, human_parent) in [
        (16_u8, 19_usize, true),
        (16, 19, false),
        (14, 24, true),
        (14, 24, false),
    ] {
        let mut session = base.clone();
        let player = session.state.entity_index;
        let actors = session
            .entities()
            .iter()
            .filter(|row| {
                row.active
                    && row.class == 5
                    && row.model != 27
                    && row.life > 0
                    && !row.drawing_disabled()
            })
            .map(|row| row.index)
            .take(2)
            .collect::<Vec<_>>();
        assert_eq!(
            actors.len(),
            2,
            "owner Level 22 supplies real non-model27 actors"
        );
        let parent_slot = if human_parent { player } else { actors[0] };
        let attacker = actors[1];
        let position = session.entities()[usize::from(root_slot)].position;
        session
            .set_level2_entity_position(attacker, position)
            .unwrap();
        let before = session.session.state.live.shared_spells.spell_experience[family];
        {
            let arena = &mut session.session.state.live.economy;
            let root = arena.handle(root_slot).unwrap();
            let parent = arena.handle(parent_slot).unwrap();
            assert!(arena.bind_creature_parent(root, parent));
            if mode == 14 {
                arena.entities[usize::from(parent_slot)].record[154..156]
                    .copy_from_slice(&attacker.to_le_bytes());
            }
            let row = &mut arena.entities[usize::from(root_slot)];
            row.set_source_action(223);
            row.record[77] = mode;
            row.record[44..46].copy_from_slice(&parent_slot.to_le_bytes());
            row.record[50..52].copy_from_slice(&32_i16.to_le_bytes());
            row.record[52..54].copy_from_slice(&64_i16.to_le_bytes());
            if mode == 16 {
                area_damage::contact(arena, None, root_slot, 1, attacker).unwrap();
            }
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "source scheduler error: {:?}",
            session.dispatch_events()
        );
        let live = &session.session.state.live;
        let after = live.shared_spells.spell_experience[family];
        assert_eq!(
            after,
            before + if human_parent { 1 } else { 0 },
            "family {family}, human parent {human_parent}"
        );
        let player = live
            .core
            .players
            .iter()
            .find(|row| row.entity_index == player)
            .unwrap();
        assert_eq!(player.runtime_spell_experience[family], after);
    }
}

#[test]
fn owner_level20_model27_linked_damage_death_and_reclaim_all_slots() {
    let (session, root) = owner_level20_model27();
    assert_linked_damage_death_and_next_pass_reclaim_all_slots(session, root, Some(1));
}

#[test]
fn constructor_rejects_fewer_than_51_free_slots_without_consuming_them() {
    let mut fixture = fixture();
    while fixture.arena.economy.free_slot_count() > 50 {
        fixture
            .arena
            .allocate_source_slot_with_terrain(&mut fixture.terrain)
            .expect("free stack reports an available source slot");
    }
    let before = fixture.arena.economy.free_slot_count();
    assert_eq!(before, 50);
    assert!(
        mc2::allocate_authored(
            &mut fixture.arena,
            &mut fixture.runtime,
            &mut fixture.terrain,
            &fixture.particles,
            fixture.position,
            AUTHORED_RECORD,
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(fixture.arena.economy.free_slot_count(), before);

    // The source reserves all 51 free slots before constructing the chain,
    // so a consistent free stack cannot exhaust midway. Inject that allocator
    // failure to exercise the source's otherwise unreachable partial cleanup.
    let (mut session, _) = owner_level22_model27();
    let particles = session.session.definition.presentation.particles.to_vec();
    let live = &mut session.session.state.live;
    let free_before = live.economy.economy.free_source_slots().to_vec();
    let position = live.core.state.position;
    mc2::fail_part_allocation_after(Some(17));
    let result = mc2::allocate_authored(
        &mut live.economy,
        &mut live.core.runtime,
        &mut live.terrain,
        &particles,
        position,
        AUTHORED_RECORD,
    );
    mc2::fail_part_allocation_after(None);
    assert!(result.unwrap().is_none());
    let free_after = live.economy.economy.free_source_slots();
    let allocated = free_before
        .iter()
        .copied()
        .filter(|slot| !free_after.contains(slot))
        .map(|slot| {
            live.economy
                .handle(slot)
                .expect("partial row is still live")
        })
        .collect::<Vec<_>>();
    assert_eq!(allocated.len(), 18, "root and 17 parts were allocated");
    assert!(allocated.iter().all(|part| {
        let row = &live.economy.entities[usize::from(part.source_slot)];
        row.class == 5 && row.model == SOURCE_MODEL && row.drawing_disabled()
    }));
    let presentations = session.campaign_entity_presentations(0).unwrap();
    assert!(allocated.iter().all(|part| {
        presentations
            .iter()
            .all(|row| row.entity_index != part.source_slot)
    }));

    session.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(
        !session
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
        "source scheduler error after partial allocation: {:?}",
        session.dispatch_events()
    );
    let arena = &session.session.state.live.economy;
    for part in allocated {
        assert!(
            arena.slot_generation(part.source_slot) != part.generation
                || arena
                    .economy
                    .free_source_slots()
                    .contains(&part.source_slot),
            "partial model27 slot {} must be reclaimed at the next source boundary",
            part.source_slot
        );
        assert_eq!(arena.model27_part_root(part), None);
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
        &fixture.arena.entities[usize::from(allocation.root.source_slot)].record[54..56],
        &[0, 0],
        "NewEvent leaves the root's source word32 clear"
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
        assert_eq!(
            u16::from_le_bytes([head.record[54], head.record[55]]),
            allocation.root.source_slot
        );
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
        crate::native_world::source_target::Context::default(),
        &mut |_, _, _| {},
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
        crate::native_world::source_target::Context::default(),
        &mut |_, _, _| {},
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
        crate::native_world::source_target::Context::default(),
        &mut |_, _, _| {},
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
fn pursuit_reads_the_live_player_position_instead_of_retained_row_bytes() {
    for (far, phase, expected_action) in [(false, 0, 218), (true, 0, 217), (true, 1, 218)] {
        let mut fixture = fixture();
        let allocation = allocate(&mut fixture);
        let root = &mut fixture.arena.entities[usize::from(allocation.root.source_slot)];
        root.set_source_action(218);
        root.set_phase(phase);
        root.record[154..156].copy_from_slice(&fixture.target.to_le_bytes());
        let mut player_position = root.position;
        if far {
            player_position.x = player_position.x.wrapping_add(10_000);
        }
        let targets = crate::native_world::source_target::Context {
            player: Some((fixture.target, player_position, 1000)),
            particles: Some(&fixture.particles),
            ..Default::default()
        };
        mc2::step_row(
            &mut fixture.arena,
            &mut fixture.runtime,
            &mut fixture.terrain,
            &fixture.particles,
            targets,
            &mut |_, _, _| {},
            allocation.root.source_slot,
            &fixture.class3,
            &fixture.families,
            PROPERTIES.root_target_range,
            PROPERTIES.root_target_half_angle,
            0,
        )
        .unwrap();
        let root = &fixture.arena.entities[usize::from(allocation.root.source_slot)];
        assert_eq!(root.action, expected_action);
        assert_eq!(
            u16::from_le_bytes([root.record[154], root.record[155]]),
            if expected_action == 217 {
                0
            } else {
                fixture.target
            }
        );
    }
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
        crate::native_world::source_target::Context::default(),
        &mut |_, _, _| {},
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
fn head_acceleration_stops_at_both_source_speed_boundaries() {
    let mut fixture = fixture();
    let allocation = allocate(&mut fixture);
    let head = fixture.arena.entities[usize::from(allocation.heads[0].source_slot)].clone();
    for (speed, acceleration, expected) in [
        (-192_i16, 16_i16, -192_i16),
        (192, -16, 192),
        (-176, 16, -160),
        (176, 16, 192),
    ] {
        let mut row = head.clone();
        row.record[48..50].copy_from_slice(&1_u16.to_le_bytes());
        row.record[136..138].copy_from_slice(&acceleration.to_le_bytes());
        row.set_actual_speed(speed);
        combat::step_head_steering(&mut row, &fixture.terrain);
        assert_eq!(
            row.actual_speed, expected,
            "speed {speed}, acceleration {acceleration}"
        );
    }
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
        crate::native_world::source_target::Context::default(),
        &mut |_, _, _| {},
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
        crate::native_world::source_target::Context::default(),
        &mut |_, _, _| {},
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
fn actor_follow_mode_attacks_the_followed_actors_current_target() {
    for (typed, linked) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut fixture = fixture();
        let allocation = allocate(&mut fixture);
        let victim = fixture
            .arena
            .entities
            .iter()
            .find(|row| row.active && row.class == 3 && row.model == 1 && row.life >= 0)
            .expect("owner Level 2 has a rival target")
            .index;
        let root_position =
            fixture.arena.entities[usize::from(allocation.root.source_slot)].position;
        fixture.arena.entities[usize::from(victim)].set_position(root_position);
        let leader = if typed {
            let holder = fixture
                .arena
                .allocate_source_slot_with_terrain(&mut fixture.terrain)
                .unwrap();
            let bee = crate::native_world::creatures::bee::mc2::allocate_morph(
                &mut fixture.arena,
                &mut fixture.terrain,
                holder,
                fixture.target,
                fixture.target,
                root_position,
            )
            .expect("real Day-map Bee constructor");
            fixture
                .arena
                .economy
                .bee_creatures
                .iter_mut()
                .find(|row| row.source_entity_index == bee.source_entity_index)
                .unwrap()
                .target_entity_index = linked.then_some(victim);
            // A stale raw slot must not replace the typed Bee's current target.
            fixture.arena.entities[usize::from(bee.source_entity_index)].record[154..156]
                .copy_from_slice(&(if linked { 0_u16 } else { victim }).to_le_bytes());
            bee.source_entity_index
        } else {
            fixture.arena.entities[usize::from(fixture.target)].record[154..156]
                .copy_from_slice(&(if linked { victim } else { 0_u16 }).to_le_bytes());
            fixture.target
        };
        fixture.runtime.stage_variables[1] = crate::native_world::RuntimeStageVariable {
            kind: 4,
            flags: 0,
            word_2: 0,
            relation: crate::native_world::RuntimeRelation::LiveEntity(leader),
        };
        assert!(fixture.runtime.set_active_stage_variable_count(1));
        mc2::bind_authored_stage(&mut fixture.arena, &mut fixture.runtime, allocation.root, 1)
            .unwrap();
        fixture.arena.entities[usize::from(allocation.root.source_slot)].set_phase(0);
        mc2::step_row(
            &mut fixture.arena,
            &mut fixture.runtime,
            &mut fixture.terrain,
            &fixture.particles,
            crate::native_world::source_target::Context::default(),
            &mut |_, _, _| {},
            allocation.root.source_slot,
            &fixture.class3,
            &fixture.families,
            PROPERTIES.root_target_range,
            PROPERTIES.root_target_half_angle,
            0,
        )
        .unwrap();
        let root = &fixture.arena.entities[usize::from(allocation.root.source_slot)];
        assert_eq!(root.action, if linked { 218 } else { 223 });
        assert_eq!(
            u16::from_le_bytes([root.record[154], root.record[155]]),
            if linked { victim } else { 0 }
        );
    }
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
            crate::native_world::source_target::Context::default(),
            &mut |_, _, _| {},
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
fn regrowing_head_uses_same_pass_fallthrough_and_next_linked_position() {
    let expected_tail = |root: &crate::native_world::EntitySnapshot, mut body: crate::Position| {
        // Head zero's pinned Type_D404C words 6/10/12/14.
        body.z = body.z.wrapping_sub(80);
        crate::move_entity(
            &mut body,
            root.yaw.wrapping_add(9),
            root.pitch.wrapping_add(1771),
            30,
        );
        body
    };
    for (initial, timer) in [(8_u8, 0_i32), (10, 0), (12, 0), (12, 8)] {
        let mut fixture = fixture();
        let allocation = allocate(&mut fixture);
        let head_slot = allocation.heads[0].source_slot;
        let head = &mut fixture.arena.entities[usize::from(head_slot)];
        head.record[74] = initial;
        head.set_source_dword_10(timer);
        let old_max = head.max_life;
        mc2::step_row(
            &mut fixture.arena,
            &mut fixture.runtime,
            &mut fixture.terrain,
            &fixture.particles,
            crate::native_world::source_target::Context::default(),
            &mut |_, _, _| {},
            allocation.root.source_slot,
            &fixture.class3,
            &fixture.families,
            PROPERTIES.root_target_range,
            PROPERTIES.root_target_half_angle,
            0,
        )
        .unwrap();
        let head = &fixture.arena.entities[usize::from(head_slot)];
        match (initial, timer) {
            (8, _) => assert_eq!((head.record[74], head.source_dword_10()), (9, 99)),
            (10, _) => {
                assert_eq!((head.record[74], head.source_dword_10()), (11, 6));
                assert_eq!(head.source_roll(), 9);
                let body = fixture.arena.entities[usize::from(allocation.bodies[0][0].source_slot)]
                    .position;
                let root = &fixture.arena.entities[usize::from(allocation.root.source_slot)];
                assert_eq!(head.position, expected_tail(root, body));
            }
            (12, 0) => {
                assert_eq!((head.record[74], head.source_dword_10()), (12, 1));
                assert_eq!(
                    u16::from_le_bytes([head.record[154], head.record[155]]),
                    allocation.bodies[0][1].source_slot
                );
                let body = fixture.arena.entities[usize::from(allocation.bodies[0][1].source_slot)]
                    .position;
                let root = &fixture.arena.entities[usize::from(allocation.root.source_slot)];
                assert_eq!(head.position, expected_tail(root, body));
            }
            (12, 8) => {
                assert_eq!(head.record[74], 0);
                assert_eq!(head.max_life, old_max);
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn head_allocation_replacement_clears_later_frozen_target_scans() {
    for fallback in [false, true] {
        let mut fixture = fixture();
        let allocation = allocate(&mut fixture);
        let chain = multipart::read_chain(&fixture.arena, allocation.root).unwrap();
        let origin = multipart::pose_head(
            &fixture.arena.entities[usize::from(allocation.root.source_slot)],
            1,
        );
        let mut position = origin;
        position.x = position.x.wrapping_add(256);
        fixture.arena.entities[usize::from(fixture.target)].set_position(position);
        let first = &mut fixture.arena.entities[usize::from(allocation.heads[0].source_slot)];
        first.record[74] = 3;
        first.record[72] = 3;
        first.record[154..156].copy_from_slice(&fixture.target.to_le_bytes());
        first.mana_regen = 2;
        first.record[140..144].copy_from_slice(&2_i32.to_le_bytes());
        first.set_source_dword_10(2);
        let second = &mut fixture.arena.entities[usize::from(allocation.heads[1].source_slot)];
        second.record[74] = 1;
        second.set_phase(7);
        second.set_entity_rng(2);
        second.record[42..44].fill(0);
        second.set_yaw(crate::level1_mission::projectile_guidance_angles(origin, position).0);
        if fallback {
            let replaceable = fixture
                .arena
                .entities
                .iter()
                .find(|row| row.active && row.class == 2)
                .expect("owner package retains a scenery row")
                .index;
            let row = &mut fixture.arena.entities[usize::from(replaceable)];
            row.set_control_flags(
                row.control_flags | crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
            );
            fixture.arena.rebuild_source_free_arena();
            while fixture.arena.economy.free_slot_count() != 0 {
                fixture
                    .arena
                    .allocate_source_slot_with_terrain(&mut fixture.terrain)
                    .unwrap();
            }
        }
        let epoch = fixture.arena.economy.replacement_list_epoch;
        let effects = combat::step_heads(
            &mut fixture.arena,
            &mut fixture.terrain,
            &fixture.particles,
            crate::native_world::source_target::Context::default(),
            &allocation,
            &chain,
            &[fixture.target],
            epoch,
            PROPERTIES.head_target_range,
            PROPERTIES.head_target_half_angle,
            0,
        )
        .unwrap();
        assert!(
            !effects.projectile_slots.is_empty(),
            "the first head actually allocates its projectile"
        );
        assert_eq!(
            fixture.arena.economy.replacement_list_epoch != epoch,
            fallback
        );
        let second = &fixture.arena.entities[usize::from(allocation.heads[1].source_slot)];
        assert_eq!(second.record[74], if fallback { 1 } else { 2 });
        assert_eq!(
            u16::from_le_bytes([second.record[154], second.record[155]]),
            if fallback { 0 } else { fixture.target }
        );
    }
}

#[test]
fn newly_acquired_head_target_enters_attack_on_the_next_visit() {
    let mut fixture = fixture();
    let allocation = allocate(&mut fixture);
    let chain = multipart::read_chain(&fixture.arena, allocation.root).unwrap();
    let root = &fixture.arena.entities[usize::from(allocation.root.source_slot)];
    let origin = multipart::pose_head(root, 0);
    let mut position = origin;
    position.x = position.x.wrapping_add(256);
    fixture.arena.entities[usize::from(fixture.target)].set_position(position);
    let head = &mut fixture.arena.entities[usize::from(allocation.heads[0].source_slot)];
    head.record[74] = 1;
    head.set_phase(7);
    head.set_entity_rng(2); // The second source draw selects the cone scan.
    head.record[42..44].fill(0);
    head.set_yaw(crate::level1_mission::projectile_guidance_angles(origin, position).0);
    let class3 = [fixture.target];
    for expected in [2, 3] {
        let list_epoch = fixture.arena.economy.replacement_list_epoch;
        combat::step_heads(
            &mut fixture.arena,
            &mut fixture.terrain,
            &fixture.particles,
            crate::native_world::source_target::Context::default(),
            &allocation,
            &chain,
            &class3,
            list_epoch,
            PROPERTIES.head_target_range,
            PROPERTIES.head_target_half_angle,
            0,
        )
        .unwrap();
        let head = &fixture.arena.entities[usize::from(allocation.heads[0].source_slot)];
        assert_eq!(head.record[74], expected);
        assert_eq!(
            u16::from_le_bytes([head.record[154], head.record[155]]),
            fixture.target
        );
    }
}

#[test]
fn head_target_cone_reads_frozen_membership_after_target_changes() {
    let mut fixture = fixture();
    let origin = fixture.position;
    let row = &mut fixture.arena.entities[usize::from(fixture.target)];
    let mut position = origin;
    position.x = position.x.wrapping_add(256);
    row.set_position(position);
    row.set_life(0);
    row.disable_drawing();
    let heading = crate::level1_mission::projectile_guidance_angles(origin, position).0;
    let targets = crate::native_world::source_target::Context::default();
    let select = |class3: &[u16], heading| {
        combat::choose_target(&fixture.arena, targets, origin, heading, class3, 5120, 100).unwrap()
    };
    assert_eq!(select(&[fixture.target], heading), Some(fixture.target));
    assert_eq!(select(&[], heading), None);
    assert_eq!(select(&[fixture.target], heading.wrapping_add(1024)), None);
    assert_eq!(
        combat::choose_target(
            &fixture.arena,
            targets,
            origin,
            heading,
            &[fixture.target],
            5120,
            u16::MAX
        )
        .unwrap(),
        None,
        "source cone is a signed descriptor word"
    );
}

#[test]
fn summoned_target_acquisition_uses_the_frozen_class3_membership() {
    for listed in [false, true] {
        let mut fixture = fixture();
        let allocation = allocate(&mut fixture);
        let parent = fixture.arena.handle(fixture.target).unwrap();
        assert!(fixture.arena.bind_creature_parent(allocation.root, parent));
        let rival = fixture
            .arena
            .entities
            .iter()
            .find(|row| row.active && row.class == 3 && row.model == 1 && row.life > 0)
            .expect("owner package has a rival")
            .index;
        fixture.arena.entities[usize::from(rival)].set_life(0);
        let root = &mut fixture.arena.entities[usize::from(allocation.root.source_slot)];
        root.set_source_action(223);
        root.set_phase(0);
        root.record[77] = 16;
        root.record[44..46].copy_from_slice(&parent.source_slot.to_le_bytes());
        root.record[50..52].copy_from_slice(&32_i16.to_le_bytes());
        root.record[154..156].fill(0);
        let frozen = if listed { vec![rival] } else { vec![] };
        mc2::step_row(
            &mut fixture.arena,
            &mut fixture.runtime,
            &mut fixture.terrain,
            &fixture.particles,
            crate::native_world::source_target::Context::default(),
            &mut |_, _, _| {},
            allocation.root.source_slot,
            &frozen,
            &fixture.families,
            PROPERTIES.root_target_range,
            PROPERTIES.root_target_half_angle,
            0,
        )
        .unwrap();
        let row = &fixture.arena.entities[usize::from(allocation.root.source_slot)];
        assert_eq!(
            u16::from_le_bytes([row.record[154], row.record[155]]),
            if listed { rival } else { 0 }
        );
    }
}

#[test]
fn summoned_and_allied_feedback_runs_before_the_linked_head_tail() {
    for (mode, trigger, retaliation) in [
        (16_u8, false, false),
        (16, true, true),
        (14, false, false),
        (14, true, false),
        (14, true, true),
    ] {
        let mut fixture = fixture();
        let allocation = allocate(&mut fixture);
        let parent = fixture.arena.handle(fixture.target).unwrap();
        assert!(fixture.arena.bind_creature_parent(allocation.root, parent));
        let rival = fixture
            .arena
            .entities
            .iter()
            .find(|row| row.active && row.class == 3 && row.model == 1 && row.life > 0)
            .expect("owner package has a rival attacker")
            .index;
        let position = fixture.arena.entities[usize::from(allocation.root.source_slot)].position;
        fixture.arena.entities[usize::from(rival)].set_position(position);
        if mode == 14 {
            fixture.arena.entities[usize::from(parent.source_slot)].record[154..156]
                .copy_from_slice(&(if trigger { rival } else { 0_u16 }).to_le_bytes());
            fixture.arena.entities[usize::from(parent.source_slot)].record[42..44].fill(0);
        }
        let row = &mut fixture.arena.entities[usize::from(allocation.root.source_slot)];
        row.set_source_action(223);
        row.record[77] = mode;
        row.record[44..46].copy_from_slice(&parent.source_slot.to_le_bytes());
        row.record[50..52].copy_from_slice(&32_i16.to_le_bytes());
        row.record[52..54].copy_from_slice(&64_i16.to_le_bytes());
        row.set_phase(0);
        if retaliation {
            row.record[98..102].copy_from_slice(&1_i32.to_le_bytes());
            row.record[102..104].copy_from_slice(&rival.to_le_bytes());
        }
        let head = allocation.heads[0].source_slot;
        let before_phase = fixture.arena.entities[usize::from(head)].phase;
        let mut awards = Vec::new();
        let mut feedback = |arena: &crate::campaign::source_arena::SourceArena, owner, family| {
            awards.push((owner, family, arena.entities[usize::from(head)].phase));
        };
        mc2::step_row(
            &mut fixture.arena,
            &mut fixture.runtime,
            &mut fixture.terrain,
            &fixture.particles,
            crate::native_world::source_target::Context::default(),
            &mut feedback,
            allocation.root.source_slot,
            &fixture.class3,
            &fixture.families,
            PROPERTIES.root_target_range,
            PROPERTIES.root_target_half_angle,
            0,
        )
        .unwrap();
        let mut expected = Vec::new();
        if retaliation {
            expected.push((parent.source_slot, 19, before_phase));
        }
        if mode == 14 && trigger {
            expected.push((parent.source_slot, 24, before_phase));
        }
        assert_eq!(
            awards, expected,
            "mode {mode}, trigger {trigger}, retaliation {retaliation}"
        );
        assert_eq!(
            fixture.arena.entities[usize::from(head)].phase,
            before_phase.wrapping_add(1)
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
        crate::native_world::source_target::Context::default(),
        &mut |_, _, _| {},
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
        crate::native_world::source_target::Context::default(),
        &mut |_, _, _| {},
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
fn head_projectiles_do_not_report_creation_when_the_source_arena_is_full() {
    for selector in [0, 9] {
        let mut fixture = fixture();
        let allocation = allocate(&mut fixture);
        let root = &mut fixture.arena.entities[usize::from(allocation.root.source_slot)];
        root.set_source_action(218);
        root.record[154..156].copy_from_slice(&fixture.target.to_le_bytes());
        let head = &mut fixture.arena.entities[usize::from(allocation.heads[0].source_slot)];
        head.record[74] = 3;
        head.record[72] = if selector == 0 { 1 } else { 3 };
        head.record[154..156].copy_from_slice(&fixture.target.to_le_bytes());
        head.set_actual_speed(192);
        head.set_entity_rng(0);
        head.mana_regen = 2;
        head.record[140..144].copy_from_slice(&2_i32.to_le_bytes());
        head.set_source_dword_10(2);
        let mut last_reserved_slot = None;
        while fixture.arena.economy.free_slot_count() != 0 {
            last_reserved_slot = Some(
                fixture
                    .arena
                    .allocate_source_slot_with_terrain(&mut fixture.terrain)
                    .unwrap(),
            );
        }
        fixture.arena.finish_source_group();
        let effects = mc2::step_row(
            &mut fixture.arena,
            &mut fixture.runtime,
            &mut fixture.terrain,
            &fixture.particles,
            crate::native_world::source_target::Context::default(),
            &mut |_, _, _| {},
            allocation.root.source_slot,
            &fixture.class3,
            &fixture.families,
            PROPERTIES.root_target_range,
            PROPERTIES.root_target_half_angle,
            0,
        )
        .unwrap();
        assert!(effects.projectile_slots.is_empty());
        assert!(effects.sounds.iter().any(|(_, sound)| *sound == 17));
        assert!(
            effects
                .sounds
                .iter()
                .all(|(_, sound)| *sound != 15 && *sound != 23)
        );
        // Ray heads fire again on their next source substate-3 visit. A
        // Fireball head fires only on entry, so restoring capacity does not
        // invent a second shot on that visit.
        fixture
            .arena
            .economy
            .release_slot(last_reserved_slot.unwrap(), None);
        let retry = mc2::step_row(
            &mut fixture.arena,
            &mut fixture.runtime,
            &mut fixture.terrain,
            &fixture.particles,
            crate::native_world::source_target::Context::default(),
            &mut |_, _, _| {},
            allocation.root.source_slot,
            &fixture.class3,
            &fixture.families,
            PROPERTIES.root_target_range,
            PROPERTIES.root_target_half_angle,
            0,
        )
        .unwrap();
        if selector == 9 {
            assert_eq!(retry.projectile_slots.len(), 1);
            assert!(retry.sounds.iter().any(|(_, sound)| *sound == 23));
        } else {
            assert!(retry.projectile_slots.is_empty());
            assert!(retry.sounds.iter().all(|(_, sound)| *sound != 15));
        }
    }
}

#[test]
fn projectile_constructor_rereads_a_target_slot_replaced_by_its_own_allocation() {
    for selector in [0, 9] {
        let mut fixture = fixture();
        let allocation = allocate(&mut fixture);
        let head_slot = allocation.heads[0].source_slot;
        fixture.arena.entities[usize::from(head_slot)].record[154..156]
            .copy_from_slice(&fixture.target.to_le_bytes());
        let target = &mut fixture.arena.entities[usize::from(fixture.target)];
        target.set_control_flags(
            target.control_flags | crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
        );
        fixture.arena.rebuild_source_free_arena();
        while fixture.arena.economy.free_slot_count() != 0 {
            fixture
                .arena
                .allocate_source_slot_with_terrain(&mut fixture.terrain)
                .unwrap();
        }
        // Inspect the actual frozen fallback cursor on a clone; skip only
        // earlier replaceable addresses until the target is its next slot.
        loop {
            let mut probe = fixture.arena.clone();
            let next = probe
                .allocate_source_slot()
                .expect("target was captured as replaceable");
            if next == fixture.target {
                break;
            }
            fixture
                .arena
                .decrement_replacement_cursor_after_player_contact();
        }
        let targets = crate::native_world::source_target::Context {
            particles: Some(&fixture.particles),
            ..Default::default()
        };
        let slot = crate::native_world::guided_projectile::boss::construct_model27(
            &mut fixture.arena,
            &mut fixture.terrain,
            &fixture.particles,
            targets,
            head_slot,
            fixture.target,
            selector,
        )
        .unwrap()
        .expect("replacement cursor can allocate its target address");
        assert_eq!(slot, fixture.target);
        if selector == 0 {
            let shot = fixture
                .arena
                .economy
                .fireball_projectiles()
                .iter()
                .find(|row| row.source_entity_index == slot)
                .unwrap();
            assert_eq!((shot.target_class, shot.target_model), (9, 0));
        } else {
            let shot = &fixture.arena.entities[usize::from(slot)];
            assert_eq!((shot.record[69], shot.record[70]), (9, 9));
        }
    }
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
    root.set_source_player_entity_index(fixture.target);

    let prekill = mc2::step_row(
        &mut fixture.arena,
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        crate::native_world::source_target::Context::default(),
        &mut |_, _, _| {},
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

    let mut full_arena = fixture.arena.clone();
    let mut full_terrain = fixture.terrain.clone();
    while full_arena.economy.free_slot_count() != 0 {
        full_arena
            .allocate_source_slot_with_terrain(&mut full_terrain)
            .unwrap();
    }
    full_arena.finish_source_group();
    let burst_count = full_arena.effects.death_burst_origins().len();
    let failed = combat::kill(
        &mut full_arena,
        &mut full_terrain,
        &fixture.particles,
        allocation.root,
    )
    .unwrap();
    assert_eq!(failed.secondary_source_effect_slot, None);
    assert_eq!(full_arena.effects.death_burst_origins().len(), burst_count);
    assert!(
        std::iter::once(allocation.root)
            .chain(allocation.heads)
            .chain(allocation.bodies.into_iter().flatten())
            .all(|part| full_arena.entities[usize::from(part.source_slot)].drawing_disabled())
    );

    let rng_before_death =
        fixture.arena.entities[usize::from(allocation.root.source_slot)].entity_rng;
    let death = mc2::step_row(
        &mut fixture.arena,
        &mut fixture.runtime,
        &mut fixture.terrain,
        &fixture.particles,
        crate::native_world::source_target::Context::default(),
        &mut |_, _, _| {},
        allocation.root.source_slot,
        &fixture.class3,
        &fixture.families,
        7_680,
        2_048,
        0,
    )
    .unwrap();
    let outcome = death.death.expect("action221 runs source death products");
    assert_eq!(
        outcome.mana_sphere_ids.len(),
        1,
        "source's shadowed fraction emits one full-mana sphere"
    );
    let sphere = fixture
        .arena
        .economy
        .spheres()
        .iter()
        .find(|sphere| sphere.id == outcome.mana_sphere_ids[0])
        .unwrap();
    assert_eq!(sphere.mana, 20_000);
    assert_eq!(sphere.player_entity_index, fixture.target);
    assert_eq!(
        fixture.arena.entities[usize::from(allocation.root.source_slot)]
            .source_player_entity_index(),
        0
    );
    assert_eq!(
        fixture.arena.entities[usize::from(allocation.root.source_slot)].entity_rng,
        crate::native_world::level2_next_rng16(rng_before_death)
    );
    let effect_slot = outcome
        .secondary_source_effect_slot
        .expect("ordinary root death allocates source type10/subtype1");
    let effect = &fixture.arena.entities[usize::from(effect_slot)];
    assert!(effect.active);
    assert_eq!((effect.class, effect.model, effect.action), (10, 1, 1));
    assert_eq!(effect.source_id, allocation.root.source_slot);
    assert_eq!(
        fixture
            .arena
            .effects
            .death_burst_origins()
            .get(&effect_slot),
        Some(&(
            fixture.arena.slot_generation(effect_slot),
            allocation.root.source_slot
        ))
    );
    assert!(!mc2::is_living_root(&fixture.arena, allocation.root));
    assert!(multipart::read_chain(&fixture.arena, allocation.root).is_err());
}

#[test]
fn death_allocation_self_replacement_uses_the_current_source_address() {
    for mana in [0, 20_000] {
        let mut fixture = fixture();
        let allocation = allocate(&mut fixture);
        let root_slot = allocation.root.source_slot;
        fixture.arena.entities[usize::from(root_slot)].set_source_action(220);
        combat::prekill(&mut fixture.arena, allocation.root).unwrap();
        let root = &mut fixture.arena.entities[usize::from(root_slot)];
        root.set_mana(mana);
        root.set_source_player_entity_index(fixture.target);
        root.set_control_flags(root.control_flags | crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
        fixture.arena.rebuild_source_free_arena();
        while fixture.arena.economy.free_slot_count() != 0 {
            fixture
                .arena
                .allocate_source_slot_with_terrain(&mut fixture.terrain)
                .unwrap();
        }
        loop {
            let mut probe = fixture.arena.clone();
            let next = probe
                .allocate_source_slot()
                .expect("root is in the frozen replacement list");
            if next == root_slot {
                break;
            }
            fixture
                .arena
                .decrement_replacement_cursor_after_player_contact();
        }
        let outcome = combat::kill(
            &mut fixture.arena,
            &mut fixture.terrain,
            &fixture.particles,
            allocation.root,
        )
        .unwrap();
        assert_ne!(
            fixture.arena.slot_generation(root_slot),
            allocation.root.generation
        );
        assert!(
            allocation
                .heads
                .into_iter()
                .chain(allocation.bodies.into_iter().flatten())
                .all(|part| {
                    !fixture.arena.entities[usize::from(part.source_slot)].drawing_disabled()
                }),
            "a replaced root no longer links its old suffix"
        );
        if mana == 0 {
            assert_eq!(outcome.secondary_source_effect_slot, Some(root_slot));
            let burst = &fixture.arena.entities[usize::from(root_slot)];
            assert_eq!((burst.class, burst.model, burst.action), (10, 1, 1));
            assert_eq!(burst.position, Position::default());
            assert_eq!(burst.source_id, root_slot);
            assert!(
                burst.drawing_disabled(),
                "source kill hides the newly replaced current address"
            );
        } else {
            assert_eq!(outcome.mana_sphere_ids, vec![u32::from(root_slot)]);
            if let Some(burst_slot) = outcome.secondary_source_effect_slot {
                let burst = &fixture.arena.entities[usize::from(burst_slot)];
                assert_eq!(burst.position, Position::default());
                assert_eq!(burst.source_id, root_slot);
            }
            let sphere = fixture
                .arena
                .economy
                .spheres()
                .iter()
                .find(|sphere| sphere.source_entity_index == root_slot)
                .unwrap();
            assert_eq!(sphere.position, Position::default());
            assert_eq!(sphere.mana, 512);
            assert_eq!(sphere.player_entity_index, 0);
            assert!(sphere.pending_delete);
            assert_ne!(
                sphere.control_flags & crate::native_world::ENTITY_DRAW_DISABLED_BIT,
                0
            );
        }
    }
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
        crate::native_world::source_target::Context::default(),
        &mut |_, _, _| {},
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
        crate::native_world::source_target::Context::default(),
        &mut |_, _, _| {},
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
        crate::native_world::source_target::Context::default(),
        &mut |_, _, _| {},
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
        crate::native_world::source_target::Context::default(),
        &mut |_, _, _| {},
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
        crate::native_world::source_target::Context::default(),
        &mut |_, _, _| {},
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

#[test]
fn linked_chain_rejects_root_with_the_old_self_parent_value() {
    let mut fixture = fixture();
    let allocation = allocate(&mut fixture);
    fixture.arena.entities[usize::from(allocation.root.source_slot)].record[54..56]
        .copy_from_slice(&allocation.root.source_slot.to_le_bytes());
    assert!(multipart::read_chain(&fixture.arena, allocation.root).is_err());
}
