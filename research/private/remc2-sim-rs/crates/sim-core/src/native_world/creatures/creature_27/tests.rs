use super::*;
use crate::campaign::source_arena::{CreatureRootOrigin, CreatureRuntimeRole};
use crate::level_definition::CreatureKind;
use crate::native_world::ParticleParameter;
use crate::{FlightWorld, Position};

const AUTHORED_RECORD: u16 = 17;

#[derive(Clone)]
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

type HydraGame = crate::game_session::GameSession<crate::campaign::backend::Mc2CampaignBackend>;

fn hydra_sim(game: &HydraGame) -> &crate::native_world::Mc2Simulation {
    game.backend().compatibility_campaign().simulation()
}

fn hydra_sim_mut(game: &mut HydraGame) -> &mut crate::native_world::Mc2Simulation {
    game.backend_mut()
        .compatibility_campaign_mut_for_test()
        .simulation_mut()
}

fn hydra_public_step(
    game: &mut HydraGame,
    command: crate::game_session::GameCommand,
    label: &str,
    turn: usize,
) {
    game.advance(game.identity().guard(), command).unwrap();
    assert!(
        hydra_sim(game)
            .dispatch_events()
            .iter()
            .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
        "{label} turn{turn}: {:?}",
        hydra_sim(game).dispatch_events()
    );
}

#[test]
fn ordinary_level23_hydra_attacks_and_paid_meteor_reaches_capped_head_and_root() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::campaign::journal::CampaignEffect;
    use crate::game_session::{GameCommand, GameSession, SpellAssignment, SpellAssignmentHand};
    use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent, area_damage, lightning};

    const GROUP16: &[u16] = &[554, 555, 556, 557, 558, 559, 560];
    let package = std::env::var_os("MC_LEVEL23_PACKAGE").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
                "../../../generated/creatures-20261104-integration/current-owner-package/level23",
            )
        },
        std::path::PathBuf::from,
    );
    let definition = load_mc2_campaign(
        &package,
        &crate::game_definition::default_catalog().unwrap(),
    )
    .unwrap();
    let mut game = GameSession::open(
        &Mc2CampaignAdapter,
        std::sync::Arc::new(definition),
        2023,
        Mc2CampaignInputs::new(&package),
    )
    .unwrap();
    let player7 = hydra_sim(&game).players()[7].entity_index;
    assert_eq!(player7, 585);
    let player7_castle = hydra_sim(&game)
        .economy()
        .castle_for_owner(player7)
        .expect("real eighth authored spawn binds player7's initial Castle");
    assert_eq!(player7_castle.source_entity_index, 611);
    assert_eq!(
        (player7_castle.position.x, player7_castle.position.y),
        (17_664, 11_008),
        "source3/subtype11 uses its real BUILD1 anchor, not the zero spawn"
    );
    let player = hydra_sim(&game).state.entity_index;
    let player_handle = hydra_sim(&game).source_arena().handle(player).unwrap();
    let gate = hydra_sim(&game)
        .entities()
        .iter()
        .find(|row| {
            row.active && (row.class, row.model, row.action, row.source_id) == (11, 32, 32, 16)
        })
        .unwrap()
        .clone();
    let gate_handle = hydra_sim(&game).source_arena().handle(gate.index).unwrap();
    assert_eq!(
        gate.index, 422,
        "qualified Level23 reset switch from authored553"
    );
    // Explicit player carry/progression/resources; no earned-upgrade claim.
    // All authored rows and NPC life/pose/state/RNG remain untouched.
    {
        let sim = hydra_sim_mut(&mut game);
        sim.apply_carried_spell_families(&[9], Some(9)).unwrap();
        sim.apply_carried_spell_progression(&[crate::native_world::CarriedSpellProgress {
            family: 9,
            persistent_experience: crate::native_world::LEVEL2_SPELL_TIER_THRESHOLDS[9][2],
            runtime_experience: 0,
            unlocked_tier: 2,
            selected_tier: 2,
        }])
        .unwrap();
        let live = &mut sim.session.state.live;
        let row = &mut live.economy.entities[usize::from(player)];
        row.set_max_life(200_000);
        row.set_life(200_000);
        row.max_mana = 200_000;
        row.record[144..148].copy_from_slice(&200_000_i32.to_le_bytes());
        row.set_mana(200_000);
        row.set_mana_regen(0);
        let row = row.clone();
        live.player_life = row.life;
        live.shared_spells.sync_player_entity_from_authority(&row);
        live.shared_spells.player_mana = row.mana;
        live.shared_spells.caster.mana = row.mana;
        live.shared_spells
            .sync_typed_spell_parent_caches_from_player_entity();
        if let Some(fire) = live.core.context.spell_fire.as_mut() {
            fire.caster.mana = row.mana;
        }
    }
    for turn in 0..256 {
        if hydra_sim(&game).session.state.live.player_protection_ticks == 0 {
            break;
        }
        hydra_public_step(
            &mut game,
            GameCommand::default(),
            "natural startup protection",
            turn,
        );
    }
    assert_eq!(
        hydra_sim(&game).session.state.live.player_protection_ticks,
        0
    );
    assert!(
        !hydra_sim(&game)
            .dispatch_events()
            .iter()
            .any(|event| matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated { group: 16, .. }
            ))
    );
    {
        let sim = hydra_sim_mut(&mut game);
        for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
            sim.economy_mut()
                .set_castle_stage_footprint(stage, footprint);
        }
        let site = (1_u16..32)
            .flat_map(|y| {
                (1_u16..32).map(move |x| Position {
                    x: x * 2048,
                    y: y * 2048,
                    z: 0,
                })
            })
            .find(|position| {
                !sim.economy().castle_projectile_site_blocked(
                    *position,
                    sim.entities(),
                    sim.canonical_terrain(),
                )
            })
            .expect("source-valid Castle prerequisite");
        let castle = sim.economy_mut().build_castle(site, player).unwrap();
        assert!(
            sim.economy_mut()
                .set_castle_mana_prerequisite_for_test(castle, 200_000)
        );
        let color = usize::from(sim.state.player_color_index);
        sim.runtime
            .set_level2_objective_stage_selector_for(color, gate.level_gate as usize, 2);
    }
    let mut produced = false;
    for turn in 0..8 {
        assert_eq!(
            hydra_sim(&game).source_arena().handle(gate.index),
            Some(gate_handle)
        );
        hydra_public_step(
            &mut game,
            if turn == 0 {
                GameCommand {
                    assign_spell: Some(SpellAssignment {
                        spell_id: crate::game_definition::SemanticId::parse("spell.meteor")
                            .unwrap(),
                        tier: 2,
                        hand: SpellAssignmentHand::Left,
                        holder_actor: None,
                    }),
                    ..Default::default()
                }
            } else {
                GameCommand::default()
            },
            "actual553 group16 producer",
            turn,
        );
        if hydra_sim(&game).dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group: 16, authored_indices, .. }
                if *source_slot == gate.index && authored_indices.as_ref() == GROUP16)) {
            produced = true; break;
        }
    }
    assert!(
        produced,
        "actual gate publishes all six Wyverns and Hydra560"
    );
    let identity = hydra_sim(&game)
        .creature_runtime_census()
        .roots_of_kind(CreatureKind::Creature27)
        .find(|row| {
            row.origin
                == CreatureRootOrigin::AuthoredRecord {
                    authored_record: 560,
                }
        })
        .expect("real authored560 current root");
    let root = identity.handle;
    let chain = multipart::read_chain(hydra_sim(&game).source_arena(), root).unwrap();
    let parts = std::iter::once(root)
        .chain(chain.ordered.iter().copied())
        .collect::<Vec<_>>();
    assert_eq!(parts.len(), 51);
    assert_eq!(
        parts
            .iter()
            .map(|handle| handle.source_slot)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        51
    );
    let initial = parts
        .iter()
        .map(|handle| hydra_sim(&game).entities()[usize::from(handle.source_slot)].clone())
        .collect::<Vec<_>>();
    assert_eq!(initial[0].max_life, PROPERTIES.root_max_life);
    for (index, head) in chain.heads.iter().enumerate() {
        let row = &hydra_sim(&game).entities()[usize::from(head.source_slot)];
        assert_eq!(
            row.max_life,
            PROPERTIES.head_base_life + PROPERTIES.head_life_per_index * (index as i32 * 10 + 1)
        );
        assert_eq!(row.action, HEAD_ACTION);
    }
    let mut lure = initial[0].position;
    crate::move_entity(&mut lure, initial[0].yaw, 0, 2304);
    lure.z = hydra_sim(&game)
        .canonical_terrain()
        .ground_height(lure)
        .wrapping_add(384);
    {
        let sim = hydra_sim_mut(&mut game);
        sim.set_level2_entity_position(player, lure).unwrap();
        sim.state.actual_speed = 0;
        sim.state.target_speed = 0;
    }
    // One fixed lure; no actor tracking, variant selection or NPC edits.
    // The window spans32 natural eight-visit head-acquisition cadences.
    let mut moved = vec![false; 51];
    let mut presented = vec![false; 51];
    let mut projectiles = [false; 2];
    let mut audio = [false; 3];
    let mut effects = Vec::new();
    let mut consumed = Vec::new();
    let mut history = Vec::new();
    area_damage::start_actor_damage_trace(&[player_handle]);
    area_damage::start_fireball_launch_trace();
    lightning::start_endpoint_birth_trace();
    combat::start_head_launch_trace(root);
    for turn in 0..256 {
        let sim = hydra_sim(&game);
        let phases = chain
            .heads
            .map(|head| sim.entities()[usize::from(head.source_slot)].phase);
        let body_phases = chain
            .bodies
            .map(|bodies| bodies.map(|body| sim.entities()[usize::from(body.source_slot)].phase));
        let cursor = sim.effect_journal().cursor(0).next_sequence;
        area_damage::start_effect_damage_trace(player);
        hydra_public_step(
            &mut game,
            GameCommand::default(),
            "ordinary Hydra lure",
            turn,
        );
        effects.extend(area_damage::take_effect_damage_trace());
        consumed.extend(area_damage::take_player_damage_consumptions());
        let sim = hydra_sim(&game);
        let views = sim.campaign_entity_presentations(0).unwrap();
        for (index, handle) in parts.iter().enumerate() {
            assert_eq!(sim.source_arena().handle(handle.source_slot), Some(*handle));
            let row = &sim.entities()[usize::from(handle.source_slot)];
            moved[index] |= row.position != initial[index].position;
            let particle = if index == 0 {
                assert!(matches!(
                    row.particle_parameter_index,
                    Some(ROOT_PARTICLE | 337)
                ));
                row.particle_parameter_index.unwrap()
            } else if row.action == HEAD_ACTION {
                HEAD_PARTICLE
            } else {
                BODY_PARTICLE
            };
            presented[index] |= views.iter().any(|view| {
                view.entity_index == handle.source_slot
                    && view.position == row.position
                    && view.render_parameter_index == particle
            });
        }
        for (index, head) in chain.heads.iter().enumerate() {
            assert_eq!(
                sim.entities()[usize::from(head.source_slot)].phase,
                phases[index].wrapping_add(1),
                "head phase is advanced by its root, not an independent233 callback"
            );
            for (segment, body) in chain.bodies[index].iter().enumerate() {
                assert_eq!(
                    sim.entities()[usize::from(body.source_slot)].phase,
                    body_phases[index][segment],
                    "disabled234 has no independent common phase tail"
                );
            }
        }
        for record in sim
            .effect_journal()
            .records()
            .iter()
            .filter(|record| record.sequence >= cursor)
        {
            if let CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                audio_site,
                entity_index,
                player_index: -1,
                sound_id,
            }) = record.effect
            {
                let index = match (audio_site, sound_id) {
                    (crate::MissionAudioSite::HydraProjectile, 15) => Some(0),
                    (crate::MissionAudioSite::HydraProjectile, 23) => Some(1),
                    (crate::MissionAudioSite::HydraAfterAttack, 17) => Some(2),
                    _ => None,
                };
                if let Some(index) = index {
                    assert_eq!(
                        entity_index, root.source_slot,
                        "source addresses the heads' copied root ID"
                    );
                    let prepared = record
                        .audio
                        .expect("actual Hydra call reaches prepared Flight audio");
                    assert_eq!(prepared.play_type, if sound_id == 17 { 3 } else { 1 });
                    assert_eq!(prepared.command.rate_delta_percent, 0);
                    assert_eq!(prepared.command.sound_flags, initial[0].source_id as i16);
                    audio[index] |= prepared.command.audible;
                }
            }
        }
        if turn < 16 || turn % 16 == 0 {
            history.push((
                turn,
                sim.entities()[usize::from(root.source_slot)].position,
                sim.state.position,
                chain.heads.map(|head| {
                    let row = &sim.entities()[usize::from(head.source_slot)];
                    (
                        row.position,
                        row.record[74],
                        row.record[72],
                        row.source_word_96(),
                        row.life,
                    )
                }),
            ));
        }
    }
    let fireballs = area_damage::take_fireball_launch_trace();
    let fireball_impacts = area_damage::take_fireball_impact_birth_trace();
    let endpoints = lightning::take_endpoint_birth_trace();
    let head_launches = combat::take_head_launch_trace();
    for visit in &consumed {
        let Some(writer) = visit.writer else {
            continue;
        };
        let area_damage::PlayerMailboxSource::Effect(impact) = writer.source else {
            continue;
        };
        if visit.target != player_handle
            || visit.mailbox.1 != initial[0].source_id
            || writer.target != player_handle
            || writer.mailbox != visit.mailbox
            || visit.life_after >= visit.life_before
            || visit.life_after != visit.life_before.wrapping_sub(visit.mailbox.0)
        {
            continue;
        }
        if !effects.iter().any(|effect| {
            effect.source_scoped
                && effect.target_slot == player
                && effect.source_slot == Some(impact.source_slot)
                && effect.target_generation == player_handle.generation
                && effect.pulse.owner == initial[0].source_id
                && effect.pulse.payload == 850
                && effect.writer_sequence == Some(writer.sequence)
                && effect.mailbox_after == Some(visit.mailbox)
        }) {
            continue;
        }
        assert!(
            matches!(hydra_sim(&game).economy().events().get(visit.event_cursor),
            Some(Level2EconomyEvent::DamageApplied { target_entity_index, attacker_entity_index, damage, lethal: false })
                if *target_entity_index == player && *attacker_entity_index == initial[0].source_id
                    && *damage == visit.mailbox.0)
        );
        projectiles[0] |=
            fireball_impacts
                .iter()
                .any(|&(slot, generation, shot, shot_generation, owner)| {
                    slot == impact.source_slot
                        && generation == impact.generation
                        && owner == initial[0].source_id
                        && fireballs.contains(&(shot, shot_generation, owner))
                        && head_launches.iter().any(|launch| {
                            launch.root == root
                                && chain.heads.contains(&launch.head)
                                && launch.selector == 0
                                && launch.projectile.source_slot == shot
                                && launch.projectile.generation == shot_generation
                        })
                });
        projectiles[1] |= endpoints.iter().any(|birth| {
            birth.endpoint == impact
                && birth.owner == initial[0].source_id
                && head_launches.iter().any(|launch| {
                    launch.root == root
                        && chain.heads.contains(&launch.head)
                        && launch.selector == 9
                        && launch.projectile == birth.ray
                })
        });
    }
    assert!(
        moved.iter().all(|value| *value) && presented.iter().all(|value| *value),
        "all51 original generations move/present: moved={moved:?} presented={presented:?} history={history:?}"
    );
    assert_eq!(
        projectiles, [true; 2],
        "both ordinary variants must cause actual player HP loss: launches={head_launches:?} audio={audio:?} fireballs={fireballs:?} impacts={fireball_impacts:?} endpoints={endpoints:?} consumed={consumed:?} history={history:?}"
    );
    assert_eq!(audio, [true; 3]);

    // One declared player-only firing placement/aim at original body0/8.
    // This representative paid hit proves capped head damage and root copy;
    // it cannot defeat the intact default five-head chain and makes no such claim.
    let target_handle = chain.bodies[0][8];
    let target = hydra_sim(&game).entities()[usize::from(target_handle.source_slot)].clone();
    assert_eq!(target.action, BODY_ACTION);
    assert_eq!(
        target.record[60], 0,
        "234 retains its source damage immunity"
    );
    let firing = Position {
        z: target.position.z.wrapping_add(1024),
        ..target.position
    };
    let mut hand = firing;
    hand.z = hand
        .z
        .wrapping_add(hydra_sim(&game).entities()[usize::from(player)].array_fov());
    let centre = Position {
        z: target.position.z.wrapping_add(target.array_yaw()),
        ..target.position
    };
    let aim = crate::level1_mission::projectile_guidance_angles(hand, centre);
    let shot_geometry = (
        target_handle,
        target.position,
        centre,
        (
            target.array_pitch(),
            target.array_roll(),
            target.array_yaw(),
            target.array_fov(),
        ),
        chain.heads.map(|head| {
            let row = &hydra_sim(&game).entities()[usize::from(head.source_slot)];
            (head, row.position, row.array_yaw(), row.array_fov())
        }),
    );
    {
        let sim = hydra_sim_mut(&mut game);
        sim.set_level2_entity_position(player, firing).unwrap();
        sim.state.actual_speed = 0;
        sim.state.target_speed = 0;
        sim.set_level2_player_aim_for_test(aim.0, aim.1);
    }
    let mut no_cast = game.backend().compatibility_campaign().clone();
    let holder = hydra_sim(&game)
        .shared_spells()
        .local_player()
        .enabled_spell_holders[9];
    let holder = hydra_sim(&game).source_arena().handle(holder).unwrap();
    assert!(!hydra_sim(&game).shared_spells().free_spell_usage);
    area_damage::start_actor_damage_trace(&chain.heads);
    combat::start_head_damage_trace(root);
    let mut launches = Vec::new();
    let mut impacts = Vec::new();
    let mut effects = Vec::new();
    let mut consumed = Vec::new();
    let mut paid = 0_i32;
    let mut elapsed = 0;
    for turn in 0..160 {
        area_damage::start_effect_damage_trace_for_targets(
            &chain.heads.map(|head| head.source_slot),
        );
        let before = hydra_sim(&game).shared_spells().player_entity().mana;
        hydra_sim(&game).start_creature17_trace();
        hydra_public_step(
            &mut game,
            GameCommand {
                cast_left: turn == 0,
                ..Default::default()
            },
            "one fixed public Meteor",
            turn,
        );
        elapsed += 1;
        let sim = hydra_sim(&game);
        paid += before
            .saturating_sub(sim.shared_spells().player_entity().mana)
            .max(0);
        let trace = sim.take_creature17_trace();
        launches.extend(trace.meteor_launches);
        impacts.extend(trace.meteor_impacts);
        effects.extend(area_damage::take_effect_damage_trace());
        consumed.extend(area_damage::take_player_damage_consumptions());
        if consumed
            .iter()
            .any(|visit: &area_damage::PlayerDamageConsumptionReceipt| {
                visit.mailbox.1 == player && visit.writer.is_some()
            })
        {
            break;
        }
    }
    let copies = combat::take_head_damage_trace();
    for turn in 0..elapsed {
        no_cast.step(crate::PlayerCommand::default(), 0);
        assert!(
            no_cast
                .simulation()
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "no-cast turn{turn}: {:?}",
            no_cast.simulation().dispatch_events()
        );
    }
    let hit = consumed.iter().find(|visit| chain.heads.contains(&visit.target)
        && visit.mailbox.1 == player && visit.life_after == visit.life_before.wrapping_sub(visit.mailbox.0.min(76))
        && visit.writer.is_some_and(|writer| writer.target == visit.target && writer.mailbox == visit.mailbox
            && impacts.iter().any(|impact| writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
                && impact.collision_target.is_some_and(|collider| parts.contains(&collider))
                && (impact.state.source_class, impact.state.model, impact.state.subspell_index) == (10, 17, 16000)
                && launches.iter().any(|launch| launch.projectile == impact.projectile
                    && launch.holder == holder && launch.caster == player_handle
                    && (launch.packet.class, launch.packet.model, launch.packet.subspell_index) == (9, 3, 16000))
                && effects.iter().any(|effect| effect.source_scoped && effect.target_slot == visit.target.source_slot
                    && effect.target_generation == visit.target.generation && effect.pulse.owner == player
                    && effect.pulse.payload == 1600 && effect.writer_sequence == Some(writer.sequence)))))
        .expect(&format!("paid public Meteor must cause actual capped head damage: geometry={shot_geometry:?} launches={launches:?} impacts={impacts:?} consumed={consumed:?} copies={copies:?}"));
    assert_eq!(launches.len(), 1);
    assert!(paid >= crate::level_definition::METEOR_PROPERTIES.tiers[2].mana_cost);
    assert!(copies.iter().any(|copy| copy.root == root
        && copy.head == hit.target
        && copy.mailbox == hit.mailbox
        && copy.root_mailbox_after == hit.mailbox));
    let collider = impacts
        .iter()
        .find(|impact| {
            hit.writer.is_some_and(|writer| {
                writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
            })
        })
        .and_then(|impact| impact.collision_target)
        .unwrap();
    eprintln!(
        "HYDRA_PAID_COLLISION root={root:?} collider={collider:?} body_collider={} head={:?} mailbox={:?} copies={copies:?}",
        chain.bodies.iter().flatten().any(|body| *body == collider),
        hit.target,
        hit.mailbox
    );
    assert!(
        hydra_sim(&game)
            .creature_identity_for_source_slot(root.source_slot)
            .is_some_and(|identity| identity.handle == root && identity.living)
    );
    assert!(
        no_cast
            .simulation()
            .creature_identity_for_source_slot(root.source_slot)
            .is_some_and(|identity| identity.handle == root && identity.living)
    );
    assert!(
        no_cast.simulation().entities()[usize::from(hit.target.source_slot)].life
            > hydra_sim(&game).entities()[usize::from(hit.target.source_slot)].life,
        "equal-elapsed no-cast does not receive the real paid capped head hit"
    );
}

#[test]
fn configured_root_motion_preserves_source_probe_retry_and_hold_boundaries() {
    // Source-vector controls only. The ordinary Level23 encounter above never
    // changes the Hydra or terrain to admit its movement.
    let mut base = fixture();
    let allocation = allocate(&mut base);
    let origin = Position {
        x: 32 * 256 + 128,
        y: 32 * 256 + 10,
        z: 0,
    };
    let mut expected_forward = origin;
    crate::move_entity(&mut expected_forward, 0, 0, PROPERTIES.root_initial_speed);
    assert_ne!(
        origin.y >> 8,
        expected_forward.y >> 8,
        "proposal crosses a tile boundary"
    );
    for (blocked, held) in [(false, false), (true, false), (false, true)] {
        let mut arena = base.arena.clone();
        let mut terrain = base.terrain.clone();
        {
            let (planes, _, _, _) = terrain.disaster_parts();
            planes.ground_heights.fill(0);
            // Source terrain type1 maps to mask2, admitted by descriptor97;
            // type0 maps to water bit1 and would reject even a flat proposal.
            planes.terrain_types.fill(1);
            planes.angles.fill(0);
            if blocked {
                // Literal1B830's fourth corner is +3,+1, not the usual
                // adjacent +1,+1. The initial proposal must reject this
                // source slope while the southward retry path stays flat.
                let x = (expected_forward.x >> 8) as u8;
                let y = (expected_forward.y >> 8) as u8;
                planes.ground_heights
                    [usize::from(x.wrapping_add(3)) | (usize::from(y.wrapping_add(1)) << 8)] = 64;
            }
        }
        let root = &mut arena.entities[usize::from(allocation.root.source_slot)];
        root.set_position(origin);
        root.set_yaw(0);
        root.set_source_roll(0);
        root.set_actual_speed(PROPERTIES.root_initial_speed);
        if held {
            root.set_control_flags(root.control_flags | 0x800);
        }
        crate::world::effects::sync_position(
            &mut arena,
            &mut terrain,
            allocation.root.source_slot,
            origin,
            true,
            false,
        )
        .unwrap();
        let old_scratch = Position {
            x: 123,
            y: 456,
            z: 789,
        };
        terrain.set_source_predicted_position(old_scratch);
        let result =
            combat::step_root_motion(&mut arena, &mut terrain, allocation.root, true).unwrap();
        let row = &arena.entities[usize::from(allocation.root.source_slot)];
        if held {
            assert_eq!(result, 4);
            assert_eq!(row.position, origin);
            assert_eq!(row.control_flags & 0x800, 0);
            assert_eq!(row.action, 216);
            assert_eq!(terrain.source_predicted_position(), old_scratch);
        } else if blocked {
            assert_eq!(result, 3);
            assert_eq!(
                row.position, origin,
                "an accepted retry turns without moving the root"
            );
            assert_ne!(row.source_roll(), 0);
            assert_eq!(
                crate::angle_distance(0, i32::from(row.yaw)),
                PROPERTIES.root_turn_limit
            );
            let mut retry = origin;
            crate::move_entity(
                &mut retry,
                row.source_roll(),
                0,
                PROPERTIES.root_initial_speed,
            );
            retry.z = terrain.ground_height(retry);
            assert_eq!(terrain.source_predicted_position(), retry);
        } else {
            assert_eq!(result, 2);
            assert_eq!(row.position, expected_forward);
            assert_eq!(terrain.source_predicted_position(), expected_forward);
        }
    }
}

#[test]
fn configured_sleep_phase_flags_and_relocation_use_literal_source_order() {
    // Configured callback-entry/terrain controls on a real constructor. No
    // ordinary actor timer, RNG, lifetime or admission is changed by the live gate.
    let mut base = fixture();
    let allocation = allocate(&mut base);
    let parts = std::iter::once(allocation.root)
        .chain(allocation.heads.iter().copied())
        .chain(allocation.bodies.iter().flatten().copied())
        .collect::<Vec<_>>();
    for (old_timer, expected_mask) in [(6, 0x0080_0000), (18, 0)] {
        let mut f = base.clone();
        for handle in &parts {
            let row = &mut f.arena.entities[usize::from(handle.source_slot)];
            row.set_control_flags(row.control_flags | 0x0180_0000);
        }
        let row = &mut f.arena.entities[usize::from(allocation.root.source_slot)];
        row.set_source_action(216);
        row.set_source_dword_10(old_timer);
        row.record[154..156].copy_from_slice(&f.target.to_le_bytes());
        mc2::step_row(
            &mut f.arena,
            &mut f.runtime,
            &mut f.terrain,
            &f.particles,
            crate::native_world::source_target::Context::default(),
            &mut |_, _, _| {},
            allocation.root.source_slot,
            &f.class3,
            &f.families,
            PROPERTIES.root_target_range,
            PROPERTIES.root_target_half_angle,
            0,
            &mut |_, _, _, _| {},
        )
        .unwrap();
        for handle in &parts {
            assert_eq!(f.arena.handle(handle.source_slot), Some(*handle));
            assert_eq!(
                f.arena.entities[usize::from(handle.source_slot)].control_flags & 0x0180_0000,
                expected_mask,
                "old timer{old_timer} applies to all51 current parts"
            );
        }
        let row = &f.arena.entities[usize::from(allocation.root.source_slot)];
        assert_eq!(row.source_dword_10(), old_timer + 1);
        assert_eq!(row.action, if old_timer == 18 { 218 } else { 216 });
        if old_timer == 18 {
            assert_eq!(row.source_word_96(), 0);
            assert_eq!(row.record[74], 1);
            assert_eq!(row.particle_parameter_index, Some(337));
        }
    }

    let origin = Position {
        x: 32 * 256 + 128,
        y: 32 * 256 + 128,
        z: 0,
    };
    for (exhausted, high_plateau) in [(false, false), (false, true), (true, false)] {
        let mut f = base.clone();
        let row = &mut f.arena.entities[usize::from(allocation.root.source_slot)];
        row.set_source_action(216);
        row.set_source_dword_10(9);
        row.set_position(origin);
        row.set_yaw(0);
        let seed = row.entity_rng;
        let first = crate::native_world::level2_next_rng16(seed);
        let second = crate::native_world::level2_next_rng16(first);
        let mut expected_copy = origin;
        expected_copy.x = expected_copy.x.wrapping_add(((first & 7) + 8) << 8);
        expected_copy.y = expected_copy.y.wrapping_add(((second & 7) + 8) << 8);
        {
            let (planes, _, _, _) = f.terrain.disaster_parts();
            planes
                .ground_heights
                .fill(if high_plateau { 127 } else { 0 });
            planes.angles.fill(0);
            // Type1/mask2 admits the first flat candidate; type0/water
            // rejects every candidate through the exact descriptor97 mask.
            planes.terrain_types.fill(if exhausted { 0 } else { 1 });
            if high_plateau {
                // The actor's low origin is source-grounded. Every proposed
                // destination lies on a flat high plateau, so mode1 and slope
                // pass, while mode4 rejects the first steep upward pitches.
                let x = (origin.x >> 8) as u8;
                let y = (origin.y >> 8) as u8;
                for dy in 0..=1_u8 {
                    for dx in 0..=2_u8 {
                        planes.ground_heights[usize::from(x.wrapping_add(dx))
                            | (usize::from(y.wrapping_add(dy)) << 8)] = 0;
                    }
                }
            }
            if exhausted {
                // The untested128th advance has a different final ground.
                // Its copied Z is still the127th probe's zero; sound22 must
                // see that Z before the literal final terrain settle.
                let mut final_xy = expected_copy;
                for _ in 0..128 {
                    crate::move_entity(&mut final_xy, 0, 0, 768);
                }
                let x = (final_xy.x >> 8) as u8;
                let y = (final_xy.y >> 8) as u8;
                for dy in 0..=1_u8 {
                    for dx in 0..=2_u8 {
                        planes.ground_heights[usize::from(x.wrapping_add(dx))
                            | (usize::from(y.wrapping_add(dy)) << 8)] = 8;
                    }
                }
            }
        }
        assert_eq!(f.terrain.ground_height(origin), origin.z);
        let mut rejected = 0;
        for _ in 0..128 {
            expected_copy.z = f.terrain.ground_height(expected_copy);
            let pitch = crate::level1_mission::projectile_guidance_angles(origin, expected_copy).1;
            if !exhausted
                && crate::angle_distance(0, i32::from(pitch)) <= PROPERTIES.root_pitch_limit
            {
                break;
            }
            rejected += 1;
            crate::move_entity(&mut expected_copy, 0, 0, 768);
        }
        if high_plateau {
            assert!(
                (1..128).contains(&rejected),
                "real signed-pitch rejection then admission"
            );
        } else {
            assert_eq!(rejected, if exhausted { 128 } else { 0 });
        }
        crate::world::effects::sync_position(
            &mut f.arena,
            &mut f.terrain,
            allocation.root.source_slot,
            origin,
            true,
            false,
        )
        .unwrap();
        let mut at_call = Vec::new();
        let effects = mc2::step_row(
            &mut f.arena,
            &mut f.runtime,
            &mut f.terrain,
            &f.particles,
            crate::native_world::source_target::Context::default(),
            &mut |_, _, _| {},
            allocation.root.source_slot,
            &f.class3,
            &f.families,
            PROPERTIES.root_target_range,
            PROPERTIES.root_target_half_angle,
            0,
            &mut |arena, slot, sample, site| {
                if site == SoundSite::Relocation {
                    assert_eq!((slot, sample), (allocation.root.source_slot, 22));
                    at_call.push((
                        arena.handle(slot).unwrap(),
                        arena.entities[usize::from(slot)].clone(),
                        arena.economy.map_chains.tile_of(slot),
                    ));
                }
            },
        )
        .unwrap();
        assert_eq!(at_call.len(), 1);
        let (handle, row, tile) = &at_call[0];
        assert_eq!(*handle, allocation.root);
        assert_eq!(row.position, expected_copy);
        assert_eq!(row.entity_rng, second, "only the two literal root draws");
        assert_eq!(row.source_dword_10(), 10);
        let expected_tile = (expected_copy.x >> 8) | ((expected_copy.y >> 8) << 8);
        assert_eq!(
            *tile, expected_tile,
            "actual CopyPosition/map insertion precedes22"
        );
        assert_eq!(
            f.terrain.source_predicted_position(),
            f.arena.entities[usize::from(allocation.heads[4].source_slot)].position,
            "after22/settle the ordinary fifth head's2A9F0 tail is the last scratch writer"
        );
        let mut settled = expected_copy;
        settled.z = f.terrain.ground_height(settled);
        assert_eq!(
            f.arena.entities[usize::from(allocation.root.source_slot)].position,
            settled
        );
        assert_eq!(
            f.arena.entities[usize::from(allocation.root.source_slot)].entity_rng,
            second
        );
        if exhausted {
            assert_ne!(settled.z, row.position.z);
        }
        assert_eq!(
            effects
                .sounds
                .iter()
                .filter(|(_, sample)| *sample == 22)
                .count(),
            1
        );
    }

    // The same reached216/9 path goes through the actual scheduler/audio
    // consumer on the whole owner Level22. Direct callbacks above establish
    // call-time RNG/pose; this checks Flight publication without a stale
    // prepass-listener reconstruction.
    let (mut session, root_slot) = owner_level22_model27();
    let root = session.source_arena().handle(root_slot).unwrap();
    let row = &mut session.session.state.live.economy.entities[usize::from(root_slot)];
    row.set_source_action(216);
    row.set_source_dword_10(9);
    let source_id = row.source_id;
    let expected_rng = crate::native_world::level2_next_rng16(
        crate::native_world::level2_next_rng16(row.entity_rng),
    );
    let cursor = session.effect_journal().cursor(0).next_sequence;
    session.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(
        session
            .dispatch_events()
            .iter()
            .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
        "{:?}",
        session.dispatch_events()
    );
    assert_eq!(session.source_arena().handle(root_slot), Some(root));
    assert_eq!(
        session.entities()[usize::from(root_slot)].entity_rng,
        expected_rng
    );
    let sounds = session
        .effect_journal()
        .records()
        .iter()
        .filter(|record| record.sequence >= cursor)
        .filter(|record| {
            matches!(record.effect,
            crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                audio_site: crate::MissionAudioSite::HydraRelocation,
                entity_index, player_index: -1, sound_id: 22,
            }) if entity_index == root_slot)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        sounds.len(),
        1,
        "scheduled216/9 publishes exactly its reached22"
    );
    let audio = sounds[0]
        .audio
        .expect("actual positional Flight command reaches native consumer");
    assert_eq!(audio.play_type, 1);
    assert_eq!(audio.command.rate_delta_percent, 0);
    assert_eq!(audio.command.sound_flags, source_id as i16);
}

#[test]
fn configured_prekill_skips_root_motion_and_keeps_linked_tail() {
    let mut f = fixture();
    let allocation = allocate(&mut f);
    let origin = f.arena.entities[usize::from(allocation.root.source_slot)].position;
    let root = &mut f.arena.entities[usize::from(allocation.root.source_slot)];
    root.set_source_action(217);
    root.record[63] = 0;
    root.set_control_flags(root.control_flags | 0x800);
    crate::native_world::area_damage::contact(
        &mut f.arena,
        None,
        allocation.root.source_slot,
        76,
        f.target,
    )
    .unwrap();
    let phases = allocation
        .heads
        .map(|head| f.arena.entities[usize::from(head.source_slot)].phase);
    let old_body = f.arena.entities[usize::from(allocation.bodies[0][8].source_slot)].position;
    let scratch = Position {
        x: 123,
        y: 456,
        z: 789,
    };
    f.terrain.set_source_predicted_position(scratch);
    let effects = mc2::step_row(
        &mut f.arena,
        &mut f.runtime,
        &mut f.terrain,
        &f.particles,
        crate::native_world::source_target::Context::default(),
        &mut |_, _, _| {},
        allocation.root.source_slot,
        &f.class3,
        &f.families,
        PROPERTIES.root_target_range,
        PROPERTIES.root_target_half_angle,
        0,
        &mut |_, _, _, _| {},
    )
    .unwrap();
    let root = &f.arena.entities[usize::from(allocation.root.source_slot)];
    assert_eq!(
        root.action, 220,
        "prekill selection cannot be overwritten by hold→216"
    );
    assert_eq!(root.position, origin);
    assert_eq!(
        root.control_flags & 0x800,
        0x800,
        "root motion was not called"
    );
    assert_eq!(
        f.terrain.source_predicted_position(),
        f.arena.entities[usize::from(allocation.heads[4].source_slot)].position,
        "root motion is skipped, but the ordinary linked-head tail still writes scratch"
    );
    assert_ne!(f.terrain.source_predicted_position(), scratch);
    assert_eq!(
        u16::from_le_bytes(root.record[40..42].try_into().unwrap()),
        f.target
    );
    assert_eq!(
        u16::from_le_bytes(root.record[42..44].try_into().unwrap()),
        f.target
    );
    assert_eq!(&root.record[102..104], &[0, 0]);
    assert!(effects.callback_executed && effects.kill_credit.is_none());
    for (head, phase) in allocation.heads.into_iter().zip(phases) {
        assert_eq!(
            f.arena.entities[usize::from(head.source_slot)].phase,
            phase.wrapping_add(1)
        );
    }
    assert_ne!(
        f.arena.entities[usize::from(allocation.bodies[0][8].source_slot)].position,
        old_body,
        "source29A90 still poses the linked chain after early prekill"
    );
}

#[test]
fn configured_alignment_visual_precedes_far_target_exit() {
    let mut base = fixture();
    let allocation = allocate(&mut base);
    let origin = Position {
        x: 32 * 256 + 128,
        y: 64 * 256 + 128,
        z: 0,
    };
    for phase in [31, 32] {
        let mut f = base.clone();
        {
            let (planes, _, _, _) = f.terrain.disaster_parts();
            planes.ground_heights.fill(0);
            planes.terrain_types.fill(1);
            planes.angles.fill(0);
        }
        let root = &mut f.arena.entities[usize::from(allocation.root.source_slot)];
        root.set_source_action(218);
        root.set_phase(phase);
        root.set_position(origin);
        root.set_yaw(0);
        root.set_source_roll(0);
        root.record[74] = 1;
        root.record[154..156].copy_from_slice(&f.target.to_le_bytes());
        assert_eq!(root.particle_parameter_index, Some(ROOT_PARTICLE));
        root.record[96] = 7;
        let mut far = origin;
        crate::move_entity(&mut far, 0, 0, (PROPERTIES.root_target_range + 256) as i16);
        f.arena.entities[usize::from(f.target)].set_position(far);
        combat::step_root_action(
            &mut f.arena,
            &mut f.terrain,
            &f.particles,
            crate::native_world::source_target::Context::default(),
            allocation.root,
            218,
            &f.class3,
            PROPERTIES.root_target_range,
            PROPERTIES.root_target_half_angle,
        )
        .unwrap();
        let root = &f.arena.entities[usize::from(allocation.root.source_slot)];
        assert_eq!(root.particle_parameter_index, Some(ROOT_PARTICLE));
        if phase == 32 {
            assert_eq!(root.action, 217);
            assert_eq!(root.source_word_96(), 0);
            assert_eq!(
                root.record[96], 0,
                "aligned315→337→315 must reset animation before range exit"
            );
        } else {
            assert_eq!(root.action, 218);
            assert_eq!(root.source_word_96(), f.target);
            assert_eq!(
                root.record[96], 7,
                "outside the32 cadence no visual/range transition runs"
            );
        }
    }
}

#[test]
fn configured_head_tail_retains_hidden_flags_and_literal_map_copies() {
    // Separate source-exception controls. The relocation/prekill and ordinary
    // encounter above retain their actual constructor-default head states.
    let mut base = fixture();
    let allocation = allocate(&mut base);
    for initial in [9, 7, 15] {
        let mut f = base.clone();
        for handle in allocation.heads {
            let head = &mut f.arena.entities[usize::from(handle.source_slot)];
            head.record[74] = 9;
            head.set_source_dword_10(10);
        }
        let head = allocation.heads[4];
        let mut first_copy = Position::default();
        let mut second_copy = Position::default();
        if initial == 7 {
            {
                let (planes, _, _, _) = f.terrain.disaster_parts();
                planes.ground_heights.fill(0);
                planes.terrain_types.fill(1);
                planes.angles.fill(0);
            }
            let root = &mut f.arena.entities[usize::from(allocation.root.source_slot)];
            root.set_yaw(0);
            root.set_pitch(0);
            root.set_position(Position {
                x: 16_384,
                y: 16_384,
                z: 0,
            });
            let pose = multipart::pose_head(root, 4);
            let mut position = root.position;
            position.y = position.y.wrapping_add(100_u16.wrapping_sub(pose.y & 255));
            root.set_position(position);
            first_copy = multipart::pose_head(root, 4);
            second_copy = first_copy;
            second_copy.z = second_copy
                .z
                .wrapping_sub(28)
                .max(f.terrain.ground_height(second_copy));
            crate::move_entity(&mut second_copy, 0, 0, 180);
            assert_ne!(first_copy.y >> 8, second_copy.y >> 8);
            crate::world::effects::sync_position(
                &mut f.arena,
                &mut f.terrain,
                allocation.root.source_slot,
                position,
                true,
                false,
            )
            .unwrap();
            // A real retained sibling occupies the final tile ahead of this
            // head. Leaving that tile and returning must reorder them; a
            // collapsed final-only copy cannot satisfy the expected map.
            for handle in [head, allocation.heads[0]] {
                f.arena.entities[usize::from(handle.source_slot)].set_position(second_copy);
                crate::world::effects::sync_position(
                    &mut f.arena,
                    &mut f.terrain,
                    handle.source_slot,
                    second_copy,
                    true,
                    false,
                )
                .unwrap();
            }
        }
        let row = &mut f.arena.entities[usize::from(head.source_slot)];
        row.record[74] = initial;
        if initial == 7 {
            row.record[72] = 8;
            row.set_source_dword_10(0);
            row.record[71] = 0;
            row.record[48..50].copy_from_slice(&6_u16.to_le_bytes());
            row.record[58..60].fill(0);
            row.record[136..138].copy_from_slice(&12_i16.to_le_bytes());
            row.set_actual_speed(192);
            row.set_source_roll(0);
            row.set_fov(0);
        }
        let before_positions = allocation.bodies.map(|group| {
            group.map(|body| f.arena.entities[usize::from(body.source_slot)].position)
        });
        let before_map = f.arena.economy.map_chains.clone();
        let scratch = Position {
            x: 123,
            y: 456,
            z: 789,
        };
        f.terrain.set_source_predicted_position(scratch);
        let chain = multipart::read_chain(&f.arena, allocation.root).unwrap();
        let epoch = f.arena.economy.replacement_list_epoch;
        combat::step_heads(
            &mut f.arena,
            &mut f.terrain,
            &f.particles,
            crate::native_world::source_target::Context::default(),
            &allocation,
            &chain,
            &f.class3,
            epoch,
            PROPERTIES.head_target_range,
            PROPERTIES.head_target_half_angle,
            0,
            &mut |_, _, _, _| {},
        )
        .unwrap();
        let row = &f.arena.entities[usize::from(head.source_slot)];
        if initial == 7 {
            assert_eq!(row.record[74], 7);
            assert_eq!(
                row.control_flags & 9,
                1,
                "timer0 hides the local head before its final commit"
            );
            assert_eq!(row.position, second_copy);
            assert_eq!(
                f.terrain.source_predicted_position(),
                second_copy,
                "nonzero2A940 is the last reached copy for final state7"
            );
            let seed = |slot, position: Position| crate::renderable_map_chain::RenderableMapSeed {
                entity_index: slot,
                active: true,
                x: position.x,
                y: position.y,
            };
            let mut expected = before_map.clone();
            expected
                .sync_entity(seed(head.source_slot, first_copy))
                .unwrap();
            expected
                .sync_entity(seed(head.source_slot, second_copy))
                .unwrap();
            let mut collapsed = before_map.clone();
            collapsed
                .sync_entity(seed(head.source_slot, second_copy))
                .unwrap();
            for body in allocation.bodies[4] {
                let position = f.arena.entities[usize::from(body.source_slot)].position;
                expected
                    .sync_entity(seed(body.source_slot, position))
                    .unwrap();
                collapsed
                    .sync_entity(seed(body.source_slot, position))
                    .unwrap();
            }
            assert_ne!(
                expected, collapsed,
                "the retained sibling discriminates both literal copies"
            );
            assert_eq!(f.arena.economy.map_chains, expected);
            for handle in [head, allocation.heads[0]] {
                let row = &f.arena.entities[usize::from(handle.source_slot)];
                assert_eq!(
                    u16::from_le_bytes(row.record[26..28].try_into().unwrap()),
                    expected.old_map_entity(handle.source_slot)
                );
                assert_eq!(
                    u16::from_le_bytes(row.record[28..30].try_into().unwrap()),
                    expected.next_entity(handle.source_slot)
                );
            }
        } else {
            assert_eq!(row.record[74], if initial == 15 { 8 } else { 9 });
            assert_eq!(f.terrain.source_predicted_position(), scratch);
            assert_eq!(
                f.arena.economy.map_chains, before_map,
                "final8/9 skip head and body position/map copies"
            );
            for (group, positions) in allocation.bodies.iter().zip(before_positions) {
                for (body, position) in group.iter().zip(positions) {
                    assert_eq!(
                        f.arena.entities[usize::from(body.source_slot)].position,
                        position
                    );
                }
            }
            if initial == 15 {
                for handle in std::iter::once(head).chain(allocation.bodies[4]) {
                    assert_eq!(f.arena.handle(handle.source_slot), Some(handle));
                    assert_eq!(
                        f.arena.entities[usize::from(handle.source_slot)].control_flags & 9,
                        1,
                        "state15's actual head/body hide survives callback-local commit"
                    );
                }
            }
        }
    }
}

pub(crate) fn complete_ammyridia_objectives_for_test(
    mut game: &mut crate::game_session::GameSession<crate::campaign::backend::Mc2CampaignBackend>,
) -> (Position, crate::campaign::source_arena::SourceEntityHandle,
      crate::campaign::source_arena::SourceEntityHandle) {
    use crate::campaign::source_arena::CreatureRootOrigin;
    use crate::game_session::{GameCommand, GameSession};
    use crate::level_definition::{CreatureKind, ObjectiveCondition};
    use crate::native_world::{Mc2DispatchEvent, RuntimeRelation, area_damage};
    use std::collections::BTreeSet;

    type Game = GameSession<crate::campaign::backend::Mc2CampaignBackend>;
    fn sim(game: &Game) -> &crate::native_world::Mc2Simulation {
        game.backend().compatibility_campaign().simulation()
    }
    fn sim_mut(game: &mut Game) -> &mut crate::native_world::Mc2Simulation {
        game.backend_mut().compatibility_campaign_mut_for_test().simulation_mut()
    }
    fn step(game: &mut Game) {
        game.advance(game.identity().guard(), GameCommand::default()).unwrap();
        assert!(sim(game).dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "Ammyridia source dispatch: {:?}", sim(game).dispatch_events());
    }
    let goals = game.definition().level_definition().unwrap().objectives();
    assert_eq!(goals.len(), 2);
    assert!(matches!(goals[0].condition(), ObjectiveCondition::NoLivingCreatures(CreatureKind::Wyvern)));
    assert!(matches!(goals[1].condition(), ObjectiveCondition::TargetDeath(_)));
    let player = sim(&game).state.entity_index;
    let owner = usize::from(sim(&game).state.player_color_index);
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[..2], [1, 1]);
    let objective_gate = &sim(&game).entities()[23];
    assert_eq!((objective_gate.class, objective_gate.model, objective_gate.action, objective_gate.source_id),
        (11, 32, 32, 7));
    let objective_gate = sim(&game).source_arena().handle(23).unwrap();
    let wyverns = [299_u16, 300, 301, 302].map(|slot| {
        let row = &sim(&game).entities()[usize::from(slot)];
        assert_eq!((row.class, row.model), (5, 16));
        assert!(row.life >= 0);
        sim(&game).source_arena().handle(slot).unwrap()
    });
    let gate = sim(&game).source_arena().handle(303).unwrap();
    let gate_row = &sim(&game).entities()[303];
    assert_eq!((gate_row.class, gate_row.model, gate_row.action, gate_row.source_id), (11, 29, 29, 1));
    let mut struck = BTreeSet::new();
    // Selected arrived contacts leave one genuinely living Wyvern to prove
    // that both its source absence goal and its producer switch stay closed.
    for leave_one in [true, false] {
        for _ in 0..1024 {
            let live = sim(&game).source_arena().rebuilt_class5_model_slots();
            if leave_one && live[16] == [wyverns[3].source_slot] { break; }
            for handle in wyverns {
                if leave_one && handle == wyverns[3] { continue; }
                if sim(&game).source_arena().resolve(handle).is_some()
                    && sim(&game).entities()[usize::from(handle.source_slot)].life >= 0
                    && struck.insert((handle.source_slot, handle.generation))
                {
                    area_damage::contact(sim_mut(&mut game).source_arena_mut(), None, handle.source_slot, u16::MAX, player).unwrap();
                }
            }
            step(&mut game);
            if leave_one {
                assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[0], 1);
                assert_eq!(sim(&game).source_arena().handle(303), Some(gate));
                assert!(!sim(&game).dispatch_events().iter().any(|event| matches!(event,
                    Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 303, group: 1, .. })));
            } else if sim(&game).source_arena().rebuilt_class5_model_slots()[16].is_empty() { break; }
        }
        if leave_one {
            assert_eq!(sim(&game).source_arena().rebuilt_class5_model_slots()[16], [wyverns[3].source_slot]);
            assert_eq!(sim(&game).source_arena().handle(wyverns[3].source_slot), Some(wyverns[3]));
        }
    }
    assert!(sim(&game).source_arena().rebuilt_class5_model_slots()[16].is_empty());
    let mut spawned = None;
    for _ in 0..256 {
        step(&mut game);
        if sim(&game).dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 303, group: 1, authored_indices, .. }
                if authored_indices.as_ref() == [328, 342]))
        {
            spawned = sim(&game).creature_runtime_census().roots_of_kind(CreatureKind::Creature27)
                .find(|root| root.origin == (CreatureRootOrigin::AuthoredRecord { authored_record: 328 }))
                .map(|root| root.handle);
            break;
        }
    }
    let root = spawned.expect("actual Wyvern absence switch publishes the authored Hydra");
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[0], 2);
    let stage = &sim(&game).runtime().stages[1];
    assert_eq!((stage.kind, stage.flags & 1, stage.relation), (1, 1, RuntimeRelation::LiveEntity(root.source_slot)));
    let chain = multipart::read_chain(sim(&game).source_arena(), root).unwrap();
    assert_eq!(chain.ordered.len(), 50);
    step(&mut game);
    assert!(sim(&game).entities()[usize::from(root.source_slot)].life >= 0);
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[1], 1,
        "the bound living Hydra blocks its actual target-death goal");

    // The real head consumer caps each contact at 76 and regenerates heads.
    // Bring the five current living heads into one admissible finishing
    // window through repeated mailbox contacts, without setting life/state.
    let mut ready = false;
    for _ in 0..2048 {
        let heads = chain.heads.map(|handle| sim(&game).entities()[usize::from(handle.source_slot)].clone());
        if heads.iter().all(|row| row.active && row.record_bytes()[74] <= 5 && (0..=76).contains(&row.life)
            && row.source_upkeep_word_62() == 0) { ready = true; break; }
        for (handle, row) in chain.heads.into_iter().zip(heads) {
            if row.active && row.record_bytes()[74] <= 5 && row.life > 76 && row.source_upkeep_word_62() == 0 {
                area_damage::contact(sim_mut(&mut game).source_arena_mut(), None, handle.source_slot, 76, player).unwrap();
            }
        }
        step(&mut game);
        assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[1], 1);
    }
    assert!(ready, "all real heads must reach the source-damage finishing window: {:?}",
        chain.heads.map(|h| { let row = &sim(&game).entities()[usize::from(h.source_slot)]; (row.life, row.record_bytes()[74], row.source_upkeep_word_62()) }));
    for head in chain.heads {
        area_damage::contact(sim_mut(&mut game).source_arena_mut(), None, head.source_slot, 76, player).unwrap();
    }
    for _ in 0..128 {
        step(&mut game);
        if sim(&game).entities()[usize::from(root.source_slot)].record_bytes()[63] == 0 { break; }
    }
    assert_eq!(sim(&game).entities()[usize::from(root.source_slot)].record_bytes()[63], 0,
        "all five real heads must complete their ordinary death phases");
    area_damage::contact(sim_mut(&mut game).source_arena_mut(), None, root.source_slot, 76, player).unwrap();
    let parts = std::iter::once(root).chain(chain.ordered.iter().copied()).collect::<Vec<_>>();
    let mut exit = None;
    let mut group5 = 0;
    let mut group7 = 0;
    let mut secret_house = None;
    for _ in 0..1024 {
        let prior_objective_gate = sim(&game).source_arena().handle(23);
        step(&mut game);
        for event in sim(&game).dispatch_events() {
            if let Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 317, group: 5, authored_indices, .. } = event {
                assert_eq!(authored_indices.as_ref(), [385, 399, 400, 479, 480]);
                group5 += 1;
                let trigger = sim(&game).economy().authored_switch_producers().iter()
                    .find(|row| row.active && row.authored_index == 399 && (row.model, row.action) == (12, 12))
                    .expect("group5 publishes the source exit");
                assert!(sim(&game).economy().authored_class14().iter().any(|row| row.active && row.authored_index == 385));
                exit = Some((trigger.position, sim(&game).source_arena().handle(trigger.source_entity_index).unwrap()));
            }
            if let Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 23, group: 7, authored_indices, .. } = event {
                assert_eq!(prior_objective_gate, Some(objective_gate));
                assert_eq!(authored_indices.as_ref(), [24, 50, 54, 55, 481, 484, 486, 487, 488, 489,
                    490, 491, 492, 493, 494, 495, 496, 497, 498, 499, 500, 501, 502, 503, 504,
                    505, 506, 507, 508, 509, 510, 511]);
                group7 += 1;
                let house = sim(&game).ordinary().houses().iter().find(|row|
                    row.active && row.building_index == 43 && row.xtype == 8)
                    .expect("group7 produces actual House24 BUILD43");
                secret_house = sim(&game).source_arena().handle(house.entity_index);
            }
        }
        if group5 != 0 && group7 != 0 && parts.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none())
            && wyverns.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()) { break; }
    }
    assert_eq!(group5, 1);
    assert_eq!(group7, 1, "actual Hydra-death objective publishes the entire group7 once");
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[..2], [2, 2]);
    assert_eq!(sim(&game).runtime().objectives[owner].is_level_end, 1);
    assert!(parts.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()),
        "the Hydra root and all 50 original parts must reclaim naturally");
    assert!(wyverns.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()));
    let (position, trigger) = exit.expect("actual level-complete group produces the exit");
    (position, trigger, secret_house.expect("actual group7 House generation"))
}

#[test]
fn ammyridia_wyverns_and_real_hydra_death_open_the_ordinary_exit() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::game_session::{GameCompletionStatus, GameSession};
    use crate::native_world::player_lifecycle_tests::{rival_test_sim as sim, rival_test_sim_mut as sim_mut,
        rival_test_step as step};
    use std::path::PathBuf;
    let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
    let package = std::env::var_os("MC_LEVEL20_PACKAGE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("mc-bhq6-ordinary-20261025-zclu5fjj/level20"));
    let cache = std::env::var_os("MC_LEVEL20_OWNER_CACHE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("campaign-original-environments/level20"));
    let definition = load_mc2_campaign(&package, &crate::game_definition::default_catalog().unwrap()).unwrap();
    let mut game = GameSession::open(&Mc2CampaignAdapter, std::sync::Arc::new(definition), 2220,
        Mc2CampaignInputs::with_owner_cache(&package, &cache)).unwrap();
    let (position, trigger, _secret_house) = complete_ammyridia_objectives_for_test(&mut game);
    let player = sim(&game).state.entity_index;
    let sequence = sim(&game).level_exit_sequence();
    if !sequence.active && !sequence.complete {
        assert!(sim(&game).source_arena().resolve(trigger).is_some());
        sim_mut(&mut game).set_level2_entity_position(player, position).unwrap();
    }
    for _ in 0..700 {
        if matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }) { break; }
        step(&mut game);
    }
    assert!(matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }));
    assert_eq!(sim(&game).source_exit_result_byte(), 0x0a);
}

#[test]
fn jathnar_possession_carry_enters_with_selected_live_holder() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::game_session::{GameCommand, GameSession};
    use std::path::PathBuf;

    let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
    let package = std::env::var_os("MC_LEVEL23_PACKAGE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("creatures-20261104-integration/current-owner-package/level23"));
    let cache = std::env::var_os("MC_LEVEL23_OWNER_CACHE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("campaign-original-environments/level23"));
    let definition = load_mc2_campaign(&package, &crate::game_definition::default_catalog().unwrap()).unwrap();
    let mut game = GameSession::open(&Mc2CampaignAdapter, std::sync::Arc::new(definition), 2323,
        Mc2CampaignInputs::with_owner_cache(&package, &cache)).unwrap();
    let campaign = game.backend_mut().compatibility_campaign_mut_for_test();
    assert_eq!(campaign.rebuild_for_spell_carry(&[1], Some(1)).unwrap(), Some(1));
    let state = game.backend().compatibility_campaign().simulation();
    let owner = state.state.entity_index;
    let player = state.shared_spells().local_player();
    assert_eq!(crate::SOURCE_SPELL_INDEX_MAP[usize::from(player.selected_spell_ui_index)], 1);
    let slot = player.enabled_spell_holders[1];
    let handle = state.source_arena().handle(slot).expect("entry materializes the source holder");
    let holder = state.shared_spells().pickup(slot).expect("entry publishes the physical holder");
    assert!(holder.active && (holder.class, holder.model) == (15, 1)
        && holder.source_spell_parent_entity_index() == owner, "entry holder: {holder:?}");
    let physical = &state.entities()[usize::from(slot)];
    assert!(physical.active && (physical.class, physical.model) == (15, 1)
        && physical.source_spell_parent_entity_index() == owner, "entry physical row: {physical:?}");
    game.advance(game.identity().guard(), GameCommand::default()).unwrap();
    let state = game.backend().compatibility_campaign().simulation();
    assert_eq!(state.source_arena().handle(slot), Some(handle));
    assert_eq!(state.shared_spells().local_player().enabled_spell_holders[1], slot);
    assert!(state.cf10_production_schedule_receipts().iter().any(|row| matches!(row,
        crate::campaign::schedule::ScheduleReceipt::CallbackCompleted {
            pass: crate::campaign::schedule::SourcePass::UpdateEntities, slot: visited, selector, ..
        } if *visited == slot && selector.registry_key().selector == crate::campaign::spec::BehaviorSelector::ClassAction {
            class: 15, action: 3,
        })), "the actual carried Possession holder reaches its source callback");
}

#[test]
fn jathnar_carried_holder_and_first_castle_loss_source_timeline() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::game_session::GameSession;
    use crate::native_world::player_lifecycle_tests::{
        rival_test_contact as contact, rival_test_sim as sim, rival_test_sim_mut as sim_mut, rival_test_step as step,
    };
    use crate::native_world::player_spell_state::{start_castle_final_loss_trace, take_castle_final_loss_trace};
    use std::path::PathBuf;

    let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
    let package = std::env::var_os("MC_LEVEL23_PACKAGE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("creatures-20261104-integration/current-owner-package/level23"));
    let cache = std::env::var_os("MC_LEVEL23_OWNER_CACHE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("campaign-original-environments/level23"));
    let definition = load_mc2_campaign(&package, &crate::game_definition::default_catalog().unwrap()).unwrap();
    let mut game = GameSession::open(&Mc2CampaignAdapter, std::sync::Arc::new(definition), 2323,
        Mc2CampaignInputs::with_owner_cache(&package, &cache)).unwrap();
    assert_eq!(game.backend_mut().compatibility_campaign_mut_for_test()
        .rebuild_for_spell_carry(&[1], Some(1)).unwrap(), Some(1));
    let player = sim(&game).state.entity_index;
    let original_slot = sim(&game).shared_spells().local_player().enabled_spell_holders[1];
    let original = sim(&game).source_arena().handle(original_slot).expect("actual entry holder");
    // One fixed player-only retreat is outside the reset actors and authored
    // creature births; their AI, damage and source scheduling remain live.
    let mut retreat = Position { x: 8192, y: 57344, z: 0 };
    retreat.z = sim(&game).canonical_terrain().ground_height(retreat).checked_add(4096).unwrap();
    sim_mut(&mut game).set_level2_entity_position(player, retreat).unwrap();
    let rivals = [425_u16, 451, 477, 504, 531, 558, 585];
    let initial = rivals.map(|slot| {
        let state = sim(&game);
        let player = state.players().iter().find(|row| row.entity_index == slot).unwrap();
        let holder = player.enabled_spell_holders[2];
        (slot, holder, state.source_arena().handle(holder), state.entities()[usize::from(holder)].clone())
    });
    for (actor, holder, _, row) in &initial {
        assert!(row.active && (row.class, row.model) == (15, 2)
            && row.source_spell_parent_entity_index() == *actor,
            "actual entry rival{actor} Castle holder{holder}: {row:?}");
    }
    eprintln!("Jathnar entry player={player}, possession={original:?}, rival Castle holders={:?}",
        initial.iter().map(|(actor, holder, handle, _)| (*actor, *holder, *handle)).collect::<Vec<_>>());
    let entry_actor = &sim(&game).entities()[usize::from(player)];
    let entry_holder = &sim(&game).entities()[usize::from(original_slot)];
    let entry_one = &sim(&game).entities()[1];
    eprintln!("Jathnar entry local life={} action={}, holder=(class={},model={},action={},flags={:#x},parent={}), physical1=(active={},class={},model={},action={},flags={:#x},generation={:?})",
        entry_actor.life, entry_actor.action, entry_holder.class, entry_holder.model, entry_holder.action,
        entry_holder.control_flags, entry_holder.source_spell_parent_entity_index(), entry_one.active, entry_one.class,
        entry_one.model, entry_one.action, entry_one.control_flags, sim(&game).source_arena().handle(1));

    let mut changes = Vec::new();
    let mut last_local = None;
    let mut last_rivals = None;
    let mut first_loss = None;
    start_castle_final_loss_trace();
    for turn in 0..2048 {
        if let Some(castle) = sim(&game).economy().castle_for_owner(425).copied() {
            if castle.action == 4 && castle.timer == 0 && castle.life >= 0 && castle.pending_attacker_entity_index == 0 {
                contact(&mut game, castle.source_entity_index, player, castle.life);
            }
        }
        step(&mut game);
        let state = sim(&game);
        let local = state.shared_spells().local_player();
        let actor = &state.entities()[usize::from(player)];
        let holder = &state.entities()[usize::from(original_slot)];
        let local_key = (actor.action, actor.life < 0, local.enabled_spell_holders[1],
            state.source_arena().handle(original_slot), holder.active, holder.class, holder.model,
            holder.drawing_disabled(), holder.source_spell_parent_entity_index());
        if last_local.as_ref() != Some(&local_key) {
            changes.push(format!("turn{turn} local life={} action={} A4={} original={:?} physical=(active={},class={},model={},action={},flags={:#x},parent={})",
                actor.life, actor.action, local.enabled_spell_holders[1], state.source_arena().handle(original_slot),
                holder.active, holder.class, holder.model, holder.action, holder.control_flags, holder.source_spell_parent_entity_index()));
            last_local = Some(local_key);
        }
        let current_rivals = rivals.map(|slot| {
            let actor = &state.entities()[usize::from(slot)];
            let player = state.players().iter().find(|row| row.entity_index == slot).unwrap();
            (slot, actor.action, actor.life < 0, player.enabled_spell_holders[2])
        });
        if last_rivals.as_ref() != Some(&current_rivals) {
            changes.push(format!("turn{turn} rivals={current_rivals:?}"));
            last_rivals = Some(current_rivals);
        }
        for loss in take_castle_final_loss_trace() {
            changes.push(format!("turn{turn} Castle loss owner={:?} castle={:?} remove={} holder={} before={:?} after={:?}",
                loss.owner, loss.castle, loss.remove_spell, loss.holder_slot,
                loss.holder_before.as_ref().map(|(handle, row)| (*handle, row.active, row.class, row.model, row.action, row.control_flags, row.source_spell_parent_entity_index())),
                loss.holder_after.as_ref().map(|row| (row.active, row.class, row.model, row.action, row.control_flags))));
            if loss.owner.source_slot == 425 { assert!(loss.remove_spell); first_loss = Some(turn); }
        }
        if first_loss.is_some_and(|loss_turn| turn >= loss_turn + 128) { break; }
    }
    assert!(first_loss.is_some(), "actual owner425 Castle must reach its source final-loss callback; trace={changes:?}");
    let state = sim(&game);
    assert!(state.canonical_player_life_for_test() >= 0
        && state.shared_spells().local_player().enabled_spell_holders[1] == original_slot
        && state.source_arena().handle(original_slot) == Some(original),
        "the fixed retreat must retain the real living player and carried holder through this bounded source path: trace={changes:?}");
    eprintln!("Jathnar entry→first Castle and 128 subsequent source turns: {}", changes.join("\n"));


}

#[test]
fn jathnar_source_objectives_feature4_and_dynamic_groups_reach_exit() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::campaign::source_arena::SourceEntityHandle;
    use crate::game_session::{GameCommand, GameCompletionStatus, GameSession};
    use crate::level_definition::ObjectiveCondition;
    use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent, RuntimeRelation, area_damage};
    use crate::native_world::player_spell_state::{start_castle_final_loss_trace, take_castle_final_loss_trace};
    use crate::native_world::player_lifecycle_tests::{
        rival_test_contact as contact,
        rival_test_held_spell_generations as held_spells, rival_test_participation as participation,
    };
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    type Game = GameSession<crate::campaign::backend::Mc2CampaignBackend>;
    // Reset source slots, group IDs, dispatch selectors and source stage
    // operands. Each original switch is consumed by its own publication.
    const GATES: [(u16, u16, u8, i8); 6] = [
        (14, 13, 32, 0), (420, 14, 32, 1), (421, 15, 32, 2),
        (422, 16, 32, 4), (423, 17, 32, 6), (245, 18, 4, 0),
    ];
    #[derive(Default)]
    struct Receipts {
        groups: [u32; 6],
        gates: [Option<SourceEntityHandle>; 6],
        hydra: Option<SourceEntityHandle>,
        exit: Option<(Position, SourceEntityHandle)>,
        carried_holder: Option<SourceEntityHandle>,
    }
    fn sim(game: &Game) -> &crate::native_world::Mc2Simulation {
        game.backend().compatibility_campaign().simulation()
    }
    fn sim_mut(game: &mut Game) -> &mut crate::native_world::Mc2Simulation {
        game.backend_mut().compatibility_campaign_mut_for_test().simulation_mut()
    }
    fn step(game: &mut Game, receipt: &mut Receipts, command: GameCommand) {
        let before = GATES.map(|(slot, _, _, _)| {
            let state = sim(game);
            let row = &state.entities()[usize::from(slot)];
            (state.source_arena().handle(slot), row.active, row.class, row.model,
                row.action, row.source_id, row.level_gate)
        });
        game.advance(game.identity().guard(), command).unwrap();
        let state = sim(game);
        if let Some(holder) = receipt.carried_holder {
            let player = state.state.entity_index;
            let local = state.shared_spells().local_player();
            let actor = &state.entities()[usize::from(player)];
            let physical = &state.entities()[usize::from(holder.source_slot)];
            let attacker = u16::from_le_bytes([actor.record_bytes()[42], actor.record_bytes()[43]]);
            assert!(state.canonical_player_life_for_test() >= 0
                && local.enabled_spell_holders[1] == holder.source_slot
                && state.source_arena().handle(holder.source_slot) == Some(holder)
                && physical.active && (physical.class, physical.model) == (15, 1)
                && physical.source_spell_parent_entity_index() == player,
                "Jathnar first player/holder loss: turn={} life={} actor_action={} attacker={} attacker_row={:?} A4={} original={holder:?} current={:?} physical=(active={},class={},model={},action={},life={},flags={:#x},parent={})",
                local.turn, state.canonical_player_life_for_test(), actor.action, attacker, state.entities().get(usize::from(attacker)), local.enabled_spell_holders[1],
                state.source_arena().handle(holder.source_slot), physical.active, physical.class, physical.model,
                physical.action, physical.life, physical.control_flags, physical.source_spell_parent_entity_index());
        }
        for event in state.dispatch_events() {
            match event {
                Mc2DispatchEvent::Error { .. } => panic!("Jathnar source dispatch: {event:?}"),
                Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group, authored_indices, .. }
                    if (13..=18).contains(group) => {
                    let expected = state.mission().authored_features().iter()
                        .filter(|row| row.index != 0 && row.kind != 0 && row.activation_group == *group as i16)
                        .map(|row| row.index).collect::<Vec<_>>();
                    assert_eq!(authored_indices.as_ref(), expected, "complete source group{group}");
                    let index = usize::from(*group - 13);
                    let (expected_slot, expected_group, action, stage) = GATES[index];
                    assert_eq!(*source_slot, expected_slot);
                    assert_eq!(before[index], (receipt.gates[index], true, 11, action, action, expected_group, stage),
                        "the actual original gate generation owns this publication before the source pass");
                    assert!(state.cf10_production_schedule_receipts().iter().any(|row| matches!(row,
                        crate::campaign::schedule::ScheduleReceipt::CallbackCompleted {
                            pass: crate::campaign::schedule::SourcePass::UpdateEntities, slot, selector, ..
                        } if *slot == expected_slot
                            && selector.registry_key().selector == crate::campaign::spec::BehaviorSelector::ClassAction {
                                class: 11, action,
                            })), "the source scheduler ran the original class11/action{action} callback");
                    receipt.groups[index] += 1;
                    if *group == 16 {
                        let constructions = mc2::take_construction_trace();
                        let construction = constructions.iter().find(|row| row.authored_record == 560)
                            .unwrap_or_else(|| panic!("group16 did not visit the Hydra560 constructor: {constructions:?}"));
                        assert!(construction.map_allowed && construction.free_slots >= 51,
                            "Hydra560 source admission failed at the actual constructor: {construction:?}");
                        let born = construction.root.unwrap_or_else(|| panic!("Hydra560 constructor returned null: {construction:?}"));
                        let root = state.creature_runtime_census().roots_of_kind(CreatureKind::Creature27)
                            .find(|root| root.handle == born && root.origin == (CreatureRootOrigin::AuthoredRecord { authored_record: 560 }))
                            .unwrap_or_else(|| panic!("Hydra560 born at callback but missing from post-turn census: {construction:?}, physical={:?}", state.entities()[usize::from(born.source_slot)]));
                        let chain = multipart::read_chain(state.source_arena(), root.handle).unwrap();
                        assert_eq!(chain.ordered.len(), 50);
                        receipt.hydra = Some(root.handle);
                    }
                    if *group == 18 {
                        assert_eq!(authored_indices.as_ref(), [285, 286]);
                        let trigger = state.economy().authored_switch_producers().iter()
                            .find(|row| row.active && row.authored_index == 285 && (row.model, row.action) == (12, 12))
                            .expect("real level-complete group publishes exit285");
                        assert!(state.economy().authored_class14().iter().any(|row| row.active && row.authored_index == 286));
                        receipt.exit = Some((trigger.position, state.source_arena().handle(trigger.source_entity_index).unwrap()));
                    }
                }
                _ => {}
            }
        }
    }
    fn ordinary_step(game: &mut Game, receipt: &mut Receipts) { step(game, receipt, GameCommand::default()); }
    fn unprotected(game: &Game, actor: u16, handle: SourceEntityHandle) -> bool {
        let state = sim(game);
        let row = &state.entities()[usize::from(actor)];
        let upkeep = state.session.state.live.nyphur_dispatcher.get(handle).unwrap().common_upkeep(row.phase);
        upkeep.word_0x159 == 0 && !upkeep.high_upkeep
    }
    fn percent(game: &Game, player: u16) -> i32 {
        let economy = sim(game).economy();
        let total = economy.owner_mana_global();
        assert!(total > 0);
        economy.castle_for_owner(player).map_or(0, |castle|
            100 * economy.owner_mana_aggregate(player).saturating_add(castle.mana) / total)
    }

    let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
    let package = std::env::var_os("MC_LEVEL23_PACKAGE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("creatures-20261104-integration/current-owner-package/level23"));
    let cache = std::env::var_os("MC_LEVEL23_OWNER_CACHE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("campaign-original-environments/level23"));
    let definition = load_mc2_campaign(&package, &crate::game_definition::default_catalog().unwrap()).unwrap();
    let goals = definition.level_definition().unwrap().objectives();
    assert_eq!(goals.len(), 7);
    for stage in [0, 2, 3] { assert!(matches!(goals[stage].condition(), ObjectiveCondition::TargetDeathWithoutFont(_))); }
    for stage in [1, 4] { assert!(matches!(goals[stage].condition(), ObjectiveCondition::NoStandingBuildings)); }
    assert!(matches!(goals[5].condition(), ObjectiveCondition::AllOtherPlayersBanished));
    assert!(matches!(goals[6].condition(), ObjectiveCondition::CastleManaPercent(65)));
    let mut game = GameSession::open(&Mc2CampaignAdapter, std::sync::Arc::new(definition), 2323,
        Mc2CampaignInputs::with_owner_cache(&package, &cache)).unwrap();
    assert_eq!(game.backend_mut().compatibility_campaign_mut_for_test()
        .rebuild_for_spell_carry(&[1], Some(1)).unwrap(), Some(1),
        "declared prior Possession carry is materialized at source mission entry");
    mc2::start_construction_trace();
    let player = sim(&game).state.entity_index;
    let owner = usize::from(sim(&game).state.player_color_index);
    assert_eq!(player, 424);
    assert_eq!(sim(&game).runtime().player_count, 8);
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[..7], [1; 7]);
    let holder_slot = sim(&game).shared_spells().local_player().enabled_spell_holders[1];
    let holder_handle = sim(&game).source_arena().handle(holder_slot).expect("source entry Possession holder");
    // This one fixed pose is separated from every reset actor and every
    // authored creature birth; it changes no AI, health or source timer.
    let mut retreat = Position { x: 8192, y: 57344, z: 0 };
    retreat.z = sim(&game).canonical_terrain().ground_height(retreat).checked_add(4096).unwrap();
    sim_mut(&mut game).set_level2_entity_position(player, retreat).unwrap();
    let mut receipt = Receipts { carried_holder: Some(holder_handle), ..Receipts::default() };
    for (index, (slot, group, action, stage)) in GATES.into_iter().enumerate() {
        let row = &sim(&game).entities()[usize::from(slot)];
        assert_eq!((row.active, row.class, row.model, row.action, row.source_id, row.level_gate),
            (true, 11, action, action, group, stage));
        receipt.gates[index] = Some(sim(&game).source_arena().handle(slot).unwrap());
    }
    let mut hit_houses = BTreeSet::new();
    assert_eq!(sim(&game).campaign_building_objective_targets(4).len(), 4,
        "four actual BUILD23 chains start the fifth objective");

    // Exercise feature4 before the unrelated House work gives the seven real
    // AIs time to lose their own Castles independently. The proof still uses
    // ordinary source turns and compares another current owner at the actual
    // Castle-loss boundary, without holding any participant fixed.
    let rivals = [425_u16, 451, 477, 504, 531, 558, 585];
    let rival_handles = rivals.map(|slot| sim(&game).source_arena().handle(slot).unwrap());
    let proof_actor = rivals[0];
    let proof_holder = sim(&game).players().iter().find(|row| row.entity_index == proof_actor).unwrap().enabled_spell_holders[2];
    assert_eq!(proof_holder, 428);
    let proof_holder_handle = sim(&game).source_arena().handle(proof_holder).expect("actual initial Castle holder");
    let proof_castle = sim(&game).economy().castle_for_owner(proof_actor).expect("actual initial rival Castle").source_entity_index;
    assert_eq!(proof_castle, 450);
    let proof_castle_handle = sim(&game).source_arena().handle(proof_castle).unwrap();
    let mut unaffected_peer = None;
    start_castle_final_loss_trace();
    for _ in 0..2048 {
        let Some(castle) = sim(&game).economy().castle_for_owner(proof_actor).copied() else { break };
        assert_eq!(sim(&game).source_arena().handle(castle.source_entity_index), Some(proof_castle_handle));
        let peers = rivals[1..].iter().filter_map(|actor| {
            let state = sim(&game);
            let player = state.players().iter().find(|row| row.entity_index == *actor)?;
            let holder = player.enabled_spell_holders[2];
            let handle = state.source_arena().handle(holder)?;
            state.economy().castle_for_owner(*actor)?;
            state.shared_spells().pickup(holder).filter(|row| row.active && !row.drawing_disabled())?;
            let row = &state.entities()[usize::from(holder)];
            if !row.active || row.drawing_disabled() || (row.class, row.model) != (15, 2)
                || row.source_spell_parent_entity_index() != *actor { return None; }
            Some((*actor, holder, handle))
        }).collect::<Vec<_>>();
        if castle.action == 4 && castle.timer == 0 && castle.life >= 0 && castle.pending_attacker_entity_index == 0 {
            contact(&mut game, castle.source_entity_index, player, castle.life);
        }
        let affected_before = sim(&game).players().iter().find(|row| row.entity_index == proof_actor)
            .map(|row| row.enabled_spell_holders[2]);
        let holder_before = sim(&game).source_arena().handle(proof_holder);
        ordinary_step(&mut game, &mut receipt);
        let losses = take_castle_final_loss_trace();
        if sim(&game).economy().castle_for_owner(proof_actor).is_none() {
            let loss = losses.iter().find(|row| row.owner == rival_handles[0] && row.castle == proof_castle_handle)
                .unwrap_or_else(|| panic!("actual owner425 Castle-loss callback receipt missing: {losses:?}"));
            assert!(loss.remove_spell);
            assert_eq!(loss.holder_slot, proof_holder);
            let (actual_handle, actual_before) = loss.holder_before.as_ref().expect("callback-time physical holder");
            assert_eq!(*actual_handle, proof_holder_handle,
                "the original affected holder is physically current at the source Castle-loss callback; outer-turn prior={affected_before:?}/{holder_before:?}");
            assert_eq!((actual_before.class, actual_before.model, actual_before.source_spell_parent_entity_index()), (15, 2, proof_actor));
            assert!(loss.holder_after.as_ref().is_some_and(|row| row.drawing_disabled()),
                "feature4's actual callback hides the affected holder: {loss:?}");
            unaffected_peer = peers.into_iter().find(|(actor, holder, handle)| {
                let state = sim(&game);
                state.economy().castle_for_owner(*actor).is_some()
                    && state.players().iter().find(|row| row.entity_index == *actor)
                        .is_some_and(|row| row.enabled_spell_holders[2] == *holder)
                    && state.source_arena().handle(*holder) == Some(*handle)
                    && state.shared_spells().pickup(*holder).is_some_and(|row| row.active && !row.drawing_disabled())
                    && state.entities().get(usize::from(*holder)).is_some_and(|row|
                        row.active && !row.drawing_disabled() && (row.class, row.model) == (15, 2)
                            && row.source_spell_parent_entity_index() == *actor)
            });
            break;
        }
    }
    assert!(sim(&game).economy().castle_for_owner(proof_actor).is_none(), "actual owner425 Castle reaches final loss");
    assert_eq!(sim(&game).players().iter().find(|row| row.entity_index == proof_actor).unwrap().enabled_spell_holders[2], 0,
        "feature4 removes the current affected owner's Castle holder");
    if sim(&game).source_arena().resolve(proof_holder_handle).is_some() {
        let current = &sim(&game).entities()[usize::from(proof_holder)];
        assert_eq!((current.class, current.model, current.source_spell_parent_entity_index()), (15, 2, proof_actor));
        assert!(current.drawing_disabled(),
            "feature4 hides the current original holder with source byte1 bit2 before reclaim: {current:?}");
    }
    assert!(unaffected_peer.is_some(), "an actual unaffected live owner/holder must survive the same source loss visit");
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[5], 1,
        "a remaining real rival blocks all-other-players banishment");
    assert_eq!(sim(&game).runtime().objectives[owner].is_level_end, 0);
    for _ in 0..16 {
        if sim(&game).source_arena().resolve(proof_holder_handle).is_none() { break; }
        ordinary_step(&mut game, &mut receipt);
    }
    assert!(sim(&game).source_arena().resolve(proof_holder_handle).is_none());

    // Type2 follows each real target's successive House generations and reads
    // the final dead form's font. Contacts enter the typed House mailbox.
    for stage in [0_usize, 2, 3] {
        assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[stage], 1);
        let mut last_observation = None;
        let mut stagnant = 0;
        let mut packets = 0_u32;
        let mut milestones = Vec::new();
        for turn in 0..512 {
            if sim(&game).runtime().objectives[owner].stage_selectors[stage] == 2 { break; }
            let relation = sim(&game).runtime().stages[stage].relation;
            let mut observation = format!("relation={relation:?}");
            if let RuntimeRelation::LiveEntity(slot) = relation {
                let raw = &sim(&game).entities()[usize::from(slot)];
                let handle = sim(&game).source_arena().handle(slot);
                let house = sim(&game).ordinary().houses().iter()
                    .find(|row| row.source_retained() && row.entity_index == slot).copied();
                observation = format!("slot={slot} handle={handle:?} raw=({},{},{},{},{:#x}) house={:?}",
                    raw.class, raw.model, raw.action, raw.life, raw.control_flags,
                    house.map(|row| (row.building_index, row.action, row.life, row.pending_damage,
                        row.pending_attacker, row.parameter.destruction_successor)));
                if house.is_some_and(|row| row.active && row.action == 52 && row.life >= 0 && row.pending_attacker == 0) {
                    let handle = sim(&game).source_arena().handle(slot).unwrap();
                    hit_houses.insert((handle.source_slot, handle.generation));
                    assert!(sim_mut(&mut game).session.state.live.ordinary.houses.submit_source_damage(slot, u16::MAX, player));
                    packets += 1;
                }
            }
            if last_observation.as_ref() == Some(&observation) { stagnant += 1; } else { stagnant = 0; }
            if turn == 0 || stagnant == 0 && (turn % 32 == 0 || packets > 0 && packets <= 4) {
                if milestones.len() == 12 { milestones.remove(0); }
                milestones.push((turn, packets, observation.clone()));
            }
            assert!(stagnant < 64,
                "actual target stage{stage} made no observed source-state progress for 64 visits: {observation}; packets={packets}; milestones={milestones:?}");
            last_observation = Some(observation);
            ordinary_step(&mut game, &mut receipt);
        }
        assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[stage], 2,
            "actual target stage{stage}; packets={packets}; milestones={milestones:?}; last={last_observation:?}");
    }
    let initial = sim(&game).campaign_building_objective_targets(1);
    assert_eq!(initial.len(), 5);
    let survivor = initial[0].1;
    for leave_one in [true, false] {
        for _ in 0..1536 {
            let targets = sim(&game).campaign_building_objective_targets(1);
            if leave_one && !targets.is_empty() && targets.iter().all(|(_, pos)| (pos.x, pos.y) == (survivor.x, survivor.y)) { break; }
            if !leave_one && sim(&game).runtime().objectives[owner].stage_selectors[1] == 2 { break; }
            for (slot, pos) in targets {
                if leave_one && (pos.x, pos.y) == (survivor.x, survivor.y) { continue; }
                if let Some(house) = sim(&game).ordinary().houses().iter()
                    .find(|row| row.active && row.action == 52 && row.entity_index == slot).copied()
                {
                    assert!([18, 21, 54].contains(&house.building_index));
                    let handle = sim(&game).source_arena().handle(slot).unwrap();
                    if house.life >= 0 && house.pending_attacker == 0 {
                        hit_houses.insert((handle.source_slot, handle.generation));
                        assert!(sim_mut(&mut game).session.state.live.ordinary.houses.submit_source_damage(slot, u16::MAX, player));
                    }
                }
            }
            ordinary_step(&mut game, &mut receipt);
            if leave_one { assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[1], 1); }
        }
        if leave_one {
            let remaining = sim(&game).campaign_building_objective_targets(1);
            assert!(!remaining.is_empty() && remaining.iter().all(|(_, pos)| (pos.x, pos.y) == (survivor.x, survivor.y)));
        }
    }
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[1], 2);
    for _ in 0..128 { if receipt.groups[..3] == [1, 1, 1] { break; } ordinary_step(&mut game, &mut receipt); }
    assert_eq!(receipt.groups[..3], [1, 1, 1]);

    assert!(rivals.iter().any(|actor| participation(&game, *actor) == Some(1)
        && sim(&game).entities()[usize::from(*actor)].active
        && sim(&game).entities()[usize::from(*actor)].life >= 0),
        "an actual living participating rival remains while the fifth objective is reached");
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[5], 1,
        "the real living rival blocks the source banishment objective");

    // Last standing-BUILD objective walks the complete 23→24→28 chain.
    for _ in 0..1536 {
        if sim(&game).runtime().objectives[owner].stage_selectors[4] == 2 && receipt.groups[3] == 1 { break; }
        for (slot, _) in sim(&game).campaign_building_objective_targets(4) {
            if let Some(house) = sim(&game).ordinary().houses().iter()
                .find(|row| row.active && row.action == 52 && row.entity_index == slot).copied()
            {
                assert!([23, 24, 28].contains(&house.building_index));
                let handle = sim(&game).source_arena().handle(slot).unwrap();
                if house.life >= 0 && house.pending_attacker == 0 {
                    hit_houses.insert((handle.source_slot, handle.generation));
                    assert!(sim_mut(&mut game).session.state.live.ordinary.houses.submit_source_damage(slot, u16::MAX, player));
                }
            }
        }
        ordinary_step(&mut game, &mut receipt);
    }
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[4], 2);
    assert_eq!(receipt.groups[3], 1);
    let hydra = receipt.hydra.expect("actual group16 Hydra560");
    let chain = multipart::read_chain(sim(&game).source_arena(), hydra).unwrap();
    assert_eq!(chain.ordered.len(), 50);
    // Hydra is nongating. Apply the source's capped 76-point contacts to its
    // actual head mailboxes and synchronize the five real head deaths.
    let mut ready = false;
    for _ in 0..2048 {
        let heads = chain.heads.map(|handle| sim(&game).entities()[usize::from(handle.source_slot)].clone());
        if heads.iter().all(|row| row.active && row.record_bytes()[74] <= 5 && (0..=76).contains(&row.life)
            && row.source_upkeep_word_62() == 0) { ready = true; break; }
        for (handle, row) in chain.heads.into_iter().zip(heads) {
            if row.active && row.record_bytes()[74] <= 5 && row.life > 76 && row.source_upkeep_word_62() == 0 {
                area_damage::contact(sim_mut(&mut game).source_arena_mut(), None, handle.source_slot, 76, player).unwrap();
            }
        }
        ordinary_step(&mut game, &mut receipt);
    }
    assert!(ready, "actual Hydra head health must reach the ordinary finishing window");
    for head in chain.heads { area_damage::contact(sim_mut(&mut game).source_arena_mut(), None, head.source_slot, 76, player).unwrap(); }
    for _ in 0..128 {
        ordinary_step(&mut game, &mut receipt);
        if sim(&game).entities()[usize::from(hydra.source_slot)].record_bytes()[63] == 0 { break; }
    }
    assert_eq!(sim(&game).entities()[usize::from(hydra.source_slot)].record_bytes()[63], 0);
    area_damage::contact(sim_mut(&mut game).source_arena_mut(), None, hydra.source_slot, 76, player).unwrap();
    let hydra_parts = std::iter::once(hydra).chain(chain.ordered.iter().copied()).collect::<Vec<_>>();
    for _ in 0..256 {
        if hydra_parts.iter().all(|h| sim(&game).source_arena().resolve(*h).is_none()) { break; }
        ordinary_step(&mut game, &mut receipt);
    }
    assert!(hydra_parts.iter().all(|h| sim(&game).source_arena().resolve(*h).is_none()));
    assert!(rivals.iter().any(|actor| participation(&game, *actor) == Some(1)
        && sim(&game).entities()[usize::from(*actor)].active
        && sim(&game).entities()[usize::from(*actor)].life >= 0),
        "an actual living rival remains after Hydra cleanup");
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[5], 1,
        "the source banishment objective remains incomplete while that rival lives");

    // Source feature4 removes the affected rival's Castle holder after its
    // actual last Castle tier falls. Keep an untouched other owner as the
    // positive control, then let all seven ordinary rivals reach banishment.
    let mut castle_handles = vec![proof_castle_handle];
    let mut released_holders = Vec::new();
    for (index, (&actor, &handle)) in rivals.iter().zip(&rival_handles).enumerate() {
        if participation(&game, actor) == Some(0) { continue; }
        assert_eq!(participation(&game, actor), Some(1), "actual current rival must have source participation");
        let holder = sim(&game).players().iter().find(|row| row.entity_index == actor).unwrap().enabled_spell_holders[2];
        let earlier_holder_handle = (holder != 0).then(|| sim(&game).source_arena().handle(holder)).flatten();
        for _ in 0..512 {
            if participation(&game, actor) == Some(0) { break; }
            if unprotected(&game, actor, handle) { break; }
            ordinary_step(&mut game, &mut receipt);
        }
        if participation(&game, actor) == Some(0) { continue; }
        assert!(unprotected(&game, actor, handle));
        if let Some(castle) = sim(&game).economy().castle_for_owner(actor).copied() {
            let castle_handle = sim(&game).source_arena().handle(castle.source_entity_index).unwrap();
            castle_handles.push(castle_handle);
            let mut loss_receipt = None;
            let _ = take_castle_final_loss_trace();
            for _ in 0..2048 {
                let Some(current) = sim(&game).economy().castle_for_owner(actor).copied() else { break };
                assert_eq!(sim(&game).source_arena().handle(current.source_entity_index), Some(castle_handle));
                if current.action == 4 && current.timer == 0 && current.life >= 0
                    && current.pending_attacker_entity_index == 0
                {
                    contact(&mut game, current.source_entity_index, player, current.life);
                }
                // sub_605E0 rereads the current physical owner's A4 only at
                // final loss. An earlier holder can have been stolen, dropped
                // or replaced during protection waits and prior tier visits.
                let before_slot = sim(&game).players().iter().find(|row| row.entity_index == actor)
                    .expect("current rival A4").enabled_spell_holders[2];
                let before = if before_slot == 0 { None } else {
                    let state = sim(&game);
                    Some((before_slot, state.economy().slot_generation(before_slot), state.source_arena().handle(before_slot),
                        state.entities()[usize::from(before_slot)].clone()))
                };
                ordinary_step(&mut game, &mut receipt);
                let losses = take_castle_final_loss_trace();
                if sim(&game).economy().castle_for_owner(actor).is_none() {
                    loss_receipt = Some((losses.into_iter().find(|row| row.owner == handle && row.castle == castle_handle)
                        .unwrap_or_else(|| panic!("actor{actor} missing exact Castle-loss callback receipt; outer-turn A4={before_slot}, physical={before:?}")),
                        before_slot, before));
                    break;
                }
            }
            assert!(sim(&game).economy().castle_for_owner(actor).is_none(), "actual Castle tier chain reaches final loss");
            assert_eq!(sim(&game).players().iter().find(|row| row.entity_index == actor).unwrap().enabled_spell_holders[2], 0,
                "feature4 leaves this owner's current Castle holder absent");
            let (loss, before_slot, outer_before) = loss_receipt.expect("capture the actual final Castle-loss callback");
            assert!(loss.remove_spell, "feature4 must be active for actor{actor}: {loss:?}");
            if loss.holder_slot != 0 {
                let (holder_handle, before) = loss.holder_before.as_ref().expect("callback-time physical holder");
                assert_eq!(holder_handle.source_slot, loss.holder_slot);
                let after = loss.holder_after.as_ref().expect("callback-time physical holder after hide");
                assert!(after.drawing_disabled(),
                    "actor{actor} feature4 must hide callback-time holder: {loss:?}; outer-turn A4={before_slot}/{outer_before:?}; earlier={holder}/{earlier_holder_handle:?}");
                let mut expected = before.clone();
                expected.disable_drawing();
                assert_eq!(*after, expected,
                    "actor{actor} feature4 must apply the source hide to this exact physical record: {loss:?}");
                eprintln!("Jathnar actor{actor} feature4 callback: earlier={holder}/{earlier_holder_handle:?}, outer-turn={before_slot}, source-holder={holder_handle:?}, before=(active={}, class={}, model={}, action={}, parent={}, flags={:#x})",
                    before.active, before.class, before.model, before.action,
                    before.source_spell_parent_entity_index(), before.control_flags);
                if before.active {
                    for _ in 0..16 {
                        if sim(&game).source_arena().resolve(*holder_handle).is_none() { break; }
                        ordinary_step(&mut game, &mut receipt);
                    }
                    assert!(sim(&game).source_arena().resolve(*holder_handle).is_none(),
                        "actor{actor} callback-time live holder {holder_handle:?} did not naturally reclaim: source={loss:?}, outer-turn={before_slot}/{outer_before:?}, current={:?}",
                        sim(&game).entities()[usize::from(holder_handle.source_slot)]);
                } else {
                    // This physical address was already free at the actual
                    // callback; sub_57F20 clears class but leaves model and
                    // action bytes. 605E0 still writes its hide bit, but
                    // there is no live generation to release a second time.
                    assert_eq!(before.class, 0);
                    assert!(!after.active, "the source hide must not allocate the free row");
                }
            } else {
                assert!(loss.holder_before.is_none() && loss.holder_after.is_none());
                eprintln!("Jathnar actor{actor} feature4 callback: source holder was zero; outer-turn={before_slot}, earlier={holder}/{earlier_holder_handle:?}");
            }
        }
        for _ in 0..512 {
            if participation(&game, actor) == Some(0) { break; }
            // A rival may already be falling after an ordinary AI encounter.
            // Its negative life is not a request for another contact.
            if sim(&game).entities()[usize::from(actor)].life < 0 { break; }
            if unprotected(&game, actor, handle) { break; }
            ordinary_step(&mut game, &mut receipt);
        }
        if participation(&game, actor) == Some(0) { continue; }
        let state = sim(&game);
        let row = &state.entities()[usize::from(actor)];
        assert!(row.active && state.economy().castle_for_owner(actor).is_none(),
            "actor{actor} must retain its current Castleless source row: handle={handle:?}, current={:?}, row={row:?}",
            state.source_arena().handle(actor));
        let life = row.life;
        if life >= 0 {
            assert!(unprotected(&game, actor, handle), "living actor{actor} remains protected: row={row:?}");
            released_holders.extend(held_spells(&game, actor));
            contact(&mut game, actor, player, life);
        }
        // sub_5E310 continues the natural fall; once grounded it enters
        // action 3. With the Castle word zero, sub_5E7C0 banishes on the
        // reached visit rather than waiting out the Castle revival timer.
        for _ in 0..768 {
            ordinary_step(&mut game, &mut receipt);
            if participation(&game, actor) == Some(0) { break; }
        }
        assert_eq!(participation(&game, actor), Some(0),
            "actor{actor} did not naturally banish: original={handle:?}, current={:?}, castle={:?}, row={:?}",
            sim(&game).source_arena().handle(actor), sim(&game).economy().castle_for_owner(actor),
            sim(&game).entities()[usize::from(actor)]);
        assert_eq!(sim(&game).source_arena().handle(actor), Some(handle));
        if index < rivals.len() - 1 && rivals[index + 1..].iter().any(|actor| participation(&game, *actor) == Some(1)) {
            assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[5], 1,
                "one live rival blocks actual all-other-players banishment");
        }
    }
    for _ in 0..512 {
        if castle_handles.iter().chain(&released_holders).all(|h| sim(&game).source_arena().resolve(*h).is_none()) { break; }
        ordinary_step(&mut game, &mut receipt);
    }
    assert!(castle_handles.iter().chain(&released_holders).all(|h| sim(&game).source_arena().resolve(*h).is_none()));

    for _ in 0..512 {
        if sim(&game).runtime().objectives[owner].stage_selectors[5] == 2 { break; }
        ordinary_step(&mut game, &mut receipt);
    }
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[5], 2,
        "the ordinary source text order admits the already-earned rival objective");

    // Defend the upcoming real collector route using arrived source contacts.
    // Source Cymmerian roots split into three independently scheduled small
    // bodies; their protected landing and all death/reclaim visits stay live.
    // Wyvern and Cymmerian actors remain authoritative and can still attack.
    let initial_threats = sim(&game).entities().iter().filter(|row|
        row.active && row.class == 5 && matches!(row.model, 16 | 25)
            && row.source_id != player && !row.drawing_disabled())
        .filter_map(|row| sim(&game).source_arena().handle(row.index).map(|handle| (row.model, handle)))
        .collect::<Vec<_>>();
    assert!(initial_threats.iter().any(|(model, _)| *model == 16)
        && initial_threats.iter().any(|(model, _)| *model == 25),
        "the actual reachable mission must supply both kinds of recorded Castle attackers: {initial_threats:?}");
    eprintln!("Jathnar live pre-collector threats: {initial_threats:?}");
    let mut parent_slots = initial_threats.iter().filter_map(|(model, handle)|
        (*model == 25).then_some(handle.source_slot)).collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut contacted = BTreeSet::new();
    let mut split_children = BTreeSet::new();
    let mut cleared = false;
    let mut allocator_wait = 0_u16;
    let mut admission_history = Vec::new();
    for _ in 0..2048 {
        let state = sim(&game);
        let current = state.entities().iter().filter(|row|
            row.active && row.class == 5 && matches!(row.model, 16 | 25) && row.source_id != player)
            .filter_map(|row| state.source_arena().handle(row.index).map(|handle| (handle, row.clone())))
            .collect::<Vec<_>>();
        for (handle, row) in &current {
            seen.insert((handle.source_slot, handle.generation));
            if row.model == 25 && row.record_bytes()[74] == 0 { parent_slots.insert(handle.source_slot); }
        }
        for root in state.creature_runtime_census().roots_of_kind(CreatureKind::Cymmerian) {
            if let CreatureRootOrigin::CreatureConversion { producer_slot } = root.origin
                && parent_slots.contains(&producer_slot)
            {
                split_children.insert((root.handle.source_slot, root.handle.generation));
            }
        }
        if current.is_empty() && seen.iter().all(|&(slot, generation)|
            state.source_arena().resolve(SourceEntityHandle { source_slot: slot, generation }).is_none())
        {
            let economy = &state.source_arena().economy;
            let existing = economy.live_castles().filter(|castle| castle.owner_entity_index == player).copied().collect::<Vec<_>>();
            let admission = (economy.free_slot_count(), economy.typed_replacement_candidate());
            if admission_history.last() != Some(&admission) { admission_history.push(admission); }
            assert!(existing.is_empty(), "the declared prior Castle cannot replace an existing live owner Castle: {existing:?}, admission={admission:?}");
            if admission.0 != 0 || admission.1.is_some() {
                cleared = true;
                break;
            }
            assert!(allocator_wait < 256,
                "real source turns did not release an available Castle slot: wait={allocator_wait}, admission={admission_history:?}, active_rows={}, hidden_rows={}",
                state.entities().iter().filter(|row| row.active).count(),
                state.entities().iter().filter(|row| row.active && row.drawing_disabled()).count());
            allocator_wait += 1;
            ordinary_step(&mut game, &mut receipt);
            continue;
        }
        for (handle, row) in current {
            let admits_contact = match row.model {
                16 => matches!(row.action, 128..=131),
                25 => matches!(row.action, 201..=203)
                    || (row.action == 200 && row.record_bytes()[74] >= 3),
                _ => false,
            };
            if admits_contact && row.life >= 0 && !row.drawing_disabled() && row.source_upkeep_word_62() == 0
                && contacted.insert((handle.source_slot, handle.generation))
            {
                area_damage::contact(sim_mut(&mut game).source_arena_mut(), None, handle.source_slot, u16::MAX, player).unwrap();
            }
        }
        ordinary_step(&mut game, &mut receipt);
    }
    assert!(cleared && !split_children.is_empty() && split_children.iter().all(|&(slot, generation)|
        sim(&game).source_arena().resolve(SourceEntityHandle { source_slot: slot, generation }).is_none()),
        "actual Castle attackers and their born split generations must finish source death/reclaim: initial={initial_threats:?}, contacted={contacted:?}, seen={seen:?}, children={split_children:?}, current={:?}",
        sim(&game).entities().iter().filter(|row| row.active && row.class == 5 && matches!(row.model, 16 | 25))
            .map(|row| (row.index, row.model, row.action, row.record_bytes()[74], row.life, row.source_upkeep_word_62(), row.control_flags)).collect::<Vec<_>>());
    eprintln!("Jathnar actual combat defense cleared {} source generations, including {} born split children; allocator wait={allocator_wait}, admission={admission_history:?}", seen.len(), split_children.len());

    // Declared prior Castle and bank isolate an actual
    // below-65→65 crossing. A real neutral sphere, paid public cast and
    // scheduled collector provide the final source-owned increment.
    for _ in 0..256 {
        if sim(&game).economy().spheres().iter().any(|row|
            row.active && !row.pending_delete && row.player_entity_index == 0 && row.control_flags & 8 != 0
                && row.control_flags & 0x0020_0000 == 0 && row.vertical_velocity == 0 && row.mana > 0) { break; }
        ordinary_step(&mut game, &mut receipt);
    }
    // Choose one existing settled sphere and one admissible Castle site.
    // Maximize current clearance from real living creatures while keeping the
    // carrier route bounded; the source continues to move and attack normally.
    let (sphere, site, threat_clearance) = {
        let state = sim(&game);
        let economy = &state.source_arena().economy;
        let replacement_slot = (economy.free_slot_count() == 0).then(|| economy.typed_replacement_candidate()).flatten();
        let spheres = state.economy().spheres().iter().filter(|row|
            row.active && !row.pending_delete && row.player_entity_index == 0 && row.control_flags & 8 != 0
                && row.control_flags & 0x0020_0000 == 0 && row.vertical_velocity == 0 && row.mana > 0
                && Some(row.source_entity_index) != replacement_slot)
            .copied().collect::<Vec<_>>();
        let threats = state.entities().iter().filter(|row| row.active && row.class == 5
            && row.life >= 0 && !row.drawing_disabled()).map(|row| row.position).collect::<Vec<_>>();
        let mut best = None;
        for y in 1_u16..32 {
            for x in 1_u16..32 {
                let mut position = Position { x: x * 2048, y: y * 2048, z: 0 };
                position.z = state.canonical_terrain().ground_height(position);
                let nearest = spheres.iter().filter_map(|sphere| {
                    let dx = i32::from(position.x.wrapping_sub(sphere.position.x) as i16).unsigned_abs();
                    let dy = i32::from(position.y.wrapping_sub(sphere.position.y) as i16).unsigned_abs();
                    (4096..=8192).contains(&dx.max(dy)).then_some((
                        crate::distance_3d_wrapped(position, sphere.position), *sphere))
                }).min_by_key(|(distance, sphere)| (*distance, sphere.source_entity_index));
                let Some((route, sphere)) = nearest else { continue };
                if state.economy().castle_projectile_site_blocked(position, state.entities(), state.canonical_terrain()) { continue; }
                let clearance = threats.iter().map(|threat| {
                    let dx = i32::from(position.x.wrapping_sub(threat.x) as i16).unsigned_abs();
                    let dy = i32::from(position.y.wrapping_sub(threat.y) as i16).unsigned_abs();
                    dx.max(dy)
                }).min().unwrap_or(u32::MAX);
                let score = (clearance, std::cmp::Reverse(route));
                if best.as_ref().is_none_or(|(previous, _, _, _)| score > *previous) {
                    best = Some((score, sphere, position, clearance));
                }
            }
        }
        let (_, sphere, position, clearance) = best.expect("a real neutral sphere has an admissible nearby Castle site");
        (sphere, position, clearance)
    };
    eprintln!("Jathnar actual collector setup: sphere={sphere:?}, site={site:?}, current creature clearance={threat_clearance}");
    let sphere_handle = sim(&game).source_arena().handle(sphere.source_entity_index).unwrap();
    let admission = {
        let economy = &sim(&game).source_arena().economy;
        (economy.free_slot_count(), economy.typed_replacement_candidate(),
            economy.live_castles().filter(|castle| castle.owner_entity_index == player).copied().collect::<Vec<_>>())
    };
    let castle = {
        let state = sim_mut(&mut game);
        for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
            state.economy_mut().set_castle_stage_footprint(stage, footprint);
        }
        state.economy_mut().build_castle(site, player).unwrap_or_else(||
            panic!("source Castle constructor rejected owner={player}, site={site:?}, sphere={sphere_handle:?}, admission={admission:?}, wait={allocator_wait}, history={admission_history:?}"))
    };
    let built_handle = sim(&game).source_arena().handle(castle);
    let built_raw = sim(&game).entities()[usize::from(castle)].clone();
    let built_typed = sim(&game).economy().castle_at(castle).copied();
    ordinary_step(&mut game, &mut receipt);
    let total = sim(&game).economy().owner_mana_global();
    let aggregate = sim(&game).economy().owner_mana_aggregate(player);
    let numerator = i64::from(total) * 65 - i64::from(aggregate) * 100;
    let threshold = i32::try_from((numerator.max(0) + 34) / 35)
        .expect("source 65-percent threshold fits i32");
    assert!(threshold > 0, "real pre-bank source census must be below65: total={total}, aggregate={aggregate}");
    // As in Pav, stage a prior bank missing the witnessed sphere's payload.
    // A one-unit gap would let unrelated merged mana finish the objective.
    let bank = threshold.checked_sub(sphere.mana).expect("prior bank fits i32");
    assert!(bank >= 0 && sphere.mana > 0);
    let eligible = sim(&game).economy().spheres().iter().filter_map(|row| {
        let handle = sim(&game).source_arena().handle(row.source_entity_index)?;
        (row.active && !row.pending_delete && row.control_flags & 8 != 0 && row.mana > 0)
            .then_some(((handle.source_slot, handle.generation), (row.mana, row.player_entity_index)))
    }).collect::<std::collections::BTreeMap<_, _>>();
    assert!(eligible.contains_key(&(sphere_handle.source_slot, sphere_handle.generation)));
    let eligible_total = eligible.values().try_fold(0_i64, |sum, (mana, _)| sum.checked_add(i64::from(*mana)))
        .expect("actual eligible source payloads fit i64");
    let capacity = i32::try_from(i64::from(aggregate) + i64::from(bank) + eligible_total)
        .expect("source Castle capacity fits i32");
    assert!(capacity <= i32::MAX / 100 && total.checked_add(bank).is_some());
    assert!(sim_mut(&mut game).economy_mut().set_castle_mana_prerequisite_for_test(castle, capacity));
    assert!(sim_mut(&mut game).economy_mut().set_castle_mana_prerequisite_for_test(castle, bank));
    let castle_handle = sim(&game).source_arena().handle(castle).unwrap();
    ordinary_step(&mut game, &mut receipt);
    assert_eq!(sim(&game).source_arena().handle(sphere_handle.source_slot), Some(sphere_handle),
        "selected sphere generation must survive prior-Castle setup: sphere={sphere:?}, current={:?}, castle={:?}",
        sim(&game).economy().spheres().iter().find(|row| row.source_entity_index == sphere_handle.source_slot), sim(&game).economy().castle_at(castle));
    assert_eq!(sim(&game).economy().owner_mana_global(), total + bank,
        "actual source census after prior bank: total={total}, aggregate={aggregate}, bank={bank}, capacity={capacity}, sphere={sphere:?}, castle={:?}", sim(&game).economy().castle_at(castle));
    assert!(percent(&game, player) < 65,
        "declared bank must leave source below65: total={total}, aggregate={aggregate}, bank={bank}, capacity={capacity}, sphere={sphere:?}, current_percent={}", percent(&game, player));
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[6], 1);

    let target = sim(&game).possession_guidance_target_top_for_test(sphere.source_entity_index).unwrap();
    let state = sim_mut(&mut game);
    let fov = state.shared_spells().player_entity().array_fov();
    let mut hand = Position { x: target.x.wrapping_add(2048), y: target.y, z: target.z.checked_add(512).unwrap() };
    let yaw = crate::level1_mission::projectile_guidance_angles(hand, target).0;
    let hand_yaw = yaw.wrapping_sub(state.shared_spells().beyond_sight_auxiliary().next_entity).wrapping_sub(512);
    let mut origin = Position { z: hand.z.checked_sub(fov).unwrap(), ..hand };
    crate::move_entity(&mut origin, hand_yaw.wrapping_add(1024), 0, 256);
    origin.z = origin.z.max(state.canonical_terrain().ground_height(origin).wrapping_add(state.profile().ground_clearance))
        .max(state.canonical_terrain().ground_height(hand).wrapping_add(crate::level1_mission::FIREBALL_COLLISION_HALF_HEIGHT).wrapping_sub(fov));
    hand.z = origin.z.wrapping_add(fov);
    let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(hand, target);
    state.set_level2_entity_position(player, origin).unwrap();
    state.state.actual_speed = 0;
    state.state.target_speed = 0;
    state.set_level2_player_aim_for_test(yaw, pitch);
    let holder = sim(&game).shared_spells().local_player().enabled_spell_holders[1];
    let holder_row = sim(&game).shared_spells().pickup(holder);
    assert!(holder != 0 && holder_row.is_some_and(|row| row.active && (row.class, row.model) == (15, 1)
        && row.source_spell_parent_entity_index() == player),
        "the current carried Possession holder must still be owned at the final cast: holder={holder}, row={holder_row:?}, player={:?}, physical={:?}, free_slots={}",
        sim(&game).shared_spells().local_player(), sim(&game).entities().get(usize::from(holder)),
        sim(&game).source_arena().economy.free_slot_count());
    assert_eq!(crate::SOURCE_SPELL_INDEX_MAP[usize::from(sim(&game).shared_spells().local_player().selected_spell_ui_index)], 1,
        "the current source selector must still choose the carried Possession holder");
    let holder_handle = sim(&game).source_arena().handle(holder).expect("current Possession holder generation");
    let player_handle = sim(&game).source_arena().handle(player).unwrap();
    let cost = sim(&game).shared_spells().pickup(holder).unwrap().max_mana;
    assert!(cost > 0 && sim(&game).shared_spells().player_entity().mana >= cost,
        "the actual carried holder must be payable: cost={cost}, holder={holder_handle:?}, player={:?}, sphere={sphere:?}, castle={:?}",
        sim(&game).shared_spells().player_entity(), sim(&game).economy().castle_at(castle));
    let mut paid_launch = false;
    let mut paid_debit = false;
    let mut claim = false;
    let mut paid_projectile = None;
    let mut paid_effects = std::collections::BTreeMap::new();
    let mut impact_targets = BTreeSet::new();
    // Paid subtotal and last verified total, keyed by physical generation.
    let mut paid_lineage = std::collections::BTreeMap::<(u16, u32), (i32, i32)>::new();
    let mut collected = std::collections::BTreeMap::<(u16, u32), std::collections::BTreeMap<(u16, u32), i32>>::new();
    let mut paid_mailbox = false;
    let mut collector = None;
    let mut credited = false;
    let mut retreated = false;
    let mut cursor = sim(&game).economy().events().len();
    let mut castle_history = std::collections::VecDeque::new();
    let mut castle_damage_history = Vec::new();
    let mut castle_raw_history = Vec::new();
    let mut sphere_history = std::collections::VecDeque::new();
    let mut possession_history = Vec::new();
    let mut mana_events = Vec::new();
    crate::native_world::level2_dynamic_economy::start_possession_trace();
    let castle_position = sim(&game).economy().castle_at(castle).unwrap().position;
    let bound = 32 + 2 * (crate::distance_3d_wrapped(sphere.position, castle_position) as usize).div_ceil(48) + 192;
    for turn in 0..bound {
        if claim && !retreated {
            let mut position = Position { x: 8192, y: 57344, z: 0 };
            position.z = sim(&game).canonical_terrain().ground_height(position).checked_add(4096).unwrap();
            sim_mut(&mut game).set_level2_entity_position(player, position).unwrap();
            retreated = true;
        }
        let before_castle = sim(&game).economy().castle_at(castle).copied();
        let before_percent = percent(&game, player);
        let before_raw = sim(&game).entities()[usize::from(castle)].clone();
        let before_handle = sim(&game).source_arena().handle(castle);
        let before_sphere = sim(&game).economy().spheres().iter()
            .find(|row| row.source_entity_index == sphere_handle.source_slot).copied();
        let before_spheres = sim(&game).economy().spheres().iter().filter(|row| row.active && !row.pending_delete)
            .map(|row| (row.source_entity_index, (sim(&game).economy().slot_generation(row.source_entity_index), row.control_flags, row.mana)))
            .collect::<std::collections::BTreeMap<_, _>>();
        for (&(slot, generation), &(paid, expected)) in &paid_lineage {
            assert!(before_spheres.get(&slot).is_some_and(|(current, _, mana)| *current == generation && *mana == expected),
                "paid sphere must retain its verified current generation and payload: turn={turn}, slot={slot}, generation={generation}, paid={paid}, expected={expected}, before={before_spheres:?}, events={mana_events:?}");
        }
        let before_defenders = sim(&game).economy().castle_children().iter().filter(|row| row.active)
            .map(|row| (row.source_entity_index, (sim(&game).source_arena().handle(row.source_entity_index), row.mana, row.max_mana, row.target_sphere_slot)))
            .collect::<std::collections::BTreeMap<_, _>>();
        step(&mut game, &mut receipt, GameCommand { cast_selected: turn == 0, ..GameCommand::default() });
        let state = sim(&game);
        let after_castle = state.economy().castle_at(castle).copied();
        let after_raw = &state.entities()[usize::from(castle)];
        let before_key = (before_handle, before_raw.active, before_raw.class, before_raw.model,
            before_raw.action, before_raw.control_flags, before_raw.authority);
        let after_key = (state.source_arena().handle(castle), after_raw.active, after_raw.class, after_raw.model,
            after_raw.action, after_raw.control_flags, after_raw.authority);
        if before_key != after_key { castle_raw_history.push((turn, before_key, after_key)); }
        let after_sphere = state.economy().spheres().iter()
            .find(|row| row.source_entity_index == sphere_handle.source_slot).copied();
        if before_sphere != after_sphere {
            if sphere_history.len() == 32 { sphere_history.pop_front(); }
            sphere_history.push_back((turn, before_sphere, after_sphere, state.source_arena().handle(sphere_handle.source_slot)));
        }
        let relevant = state.economy().events()[cursor..].iter().filter(|event| matches!(event,
            Level2EconomyEvent::DamageApplied { target_entity_index, .. } if *target_entity_index == castle)
            || matches!(event, Level2EconomyEvent::CastleStageDestroyed { source_entity_index, .. }
                | Level2EconomyEvent::CastleDestroyed { source_entity_index, .. } if *source_entity_index == castle))
            .cloned().collect::<Vec<_>>();
        for event in &relevant {
            let attacker = match event {
                Level2EconomyEvent::DamageApplied { attacker_entity_index, .. } => Some(*attacker_entity_index),
                Level2EconomyEvent::CastleDestroyed { killer_entity_index, .. } => Some(*killer_entity_index),
                _ => None,
            };
            let current_attacker = attacker.and_then(|slot| state.entities().get(usize::from(slot)).map(|row|
                (slot, state.source_arena().handle(slot), row.active, row.class, row.model, row.action, row.life, row.control_flags)));
            castle_damage_history.push((turn, event.clone(), current_attacker));
        }
        if before_castle != after_castle || !relevant.is_empty() {
            if castle_history.len() == 32 { castle_history.pop_front(); }
            castle_history.push_back((turn, before_castle, after_castle, relevant));
        }
        paid_debit |= state.cf10_production_schedule_receipts().into_iter().any(|row| match row {
            crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted {
                slot, original, current: crate::campaign::schedule::CurrentOccupant::Known { selector, .. }, ..
            } if slot == player => original.observed_player_resources().zip(selector.observed_player_resources())
                .is_some_and(|(before, after)| before.0 == player_handle && after.0 == player_handle
                    && before.2 <= -cost && after.1 == before.1.wrapping_add(before.2).clamp(0, before.3)
                    && before.1.saturating_sub(after.1) >= cost),
            _ => false,
        });
        if turn == 0 {
            paid_projectile = state.economy().possession_projectiles().iter().find_map(|row|
                (row.source_id_entity_index == player && row.holder_entity_index == holder_handle.source_slot)
                    .then(|| state.source_arena().handle(row.source_entity_index)).flatten());
            let charged = state.cf10_production_schedule_receipts().into_iter().any(|row| match row {
                crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted {
                    slot, original, current: crate::campaign::schedule::CurrentOccupant::Known { selector, .. }, ..
                } if slot == holder => original.observed_player_resources().zip(selector.observed_player_resources())
                    .is_some_and(|(before, after)| before.0 == player_handle && after.0 == player_handle
                        && before.1 >= cost && after.2 == if before.2 >= 0 { -cost } else { before.2.wrapping_sub(cost) }),
                _ => false,
            });
            paid_launch = paid_projectile.is_some() && charged;
        }
        let source_receipts = crate::native_world::level2_dynamic_economy::take_possession_trace();
        crate::native_world::level2_dynamic_economy::start_possession_trace();
        for source in source_receipts {
            possession_history.push((turn, source));
            use crate::native_world::level2_dynamic_economy::PossessionSourceReceipt;
            match source {
                PossessionSourceReceipt::ImpactBirth { effect, projectile, holder, owner, packet }
                    if Some(projectile) == paid_projectile && holder == holder_handle && owner == player_handle => {
                        assert!(matches!(packet.source_model, 12 | 70));
                        paid_effects.insert((effect.source_slot, effect.generation), packet.source_model);
                    }
                PossessionSourceReceipt::MailboxWrite { effect, target, source_id, before_possessor, amount, possessor, .. }
                    if source_id == player && before_possessor != player && possessor == player => {
                        let key = (target.source_slot, target.generation);
                        if let Some(model) = paid_effects.get(&(effect.source_slot, effect.generation))
                            && amount == u32::from(*model == 70)
                            && eligible.contains_key(&key)
                            && (amount != 0 || before_spheres.get(&target.source_slot)
                                .is_some_and(|(generation, flags, _)| *generation == target.generation && *flags & 0x0020_0000 == 0))
                        {
                            impact_targets.insert(key);
                            paid_mailbox = true;
                        }
                    }
                _ => {}
            }
        }
        assert_eq!(state.source_arena().handle(castle), Some(castle_handle),
            "declared prior Castle disappeared at collection turn{turn}: built={built_handle:?}/{built_typed:?}/{built_raw:?}, original={castle_handle:?}, before_raw={before_raw:?}, after_raw={:?}, paid_launch={paid_launch}, paid_debit={paid_debit}, claim={claim}, collector={collector:?}, credited={credited}, raw_history={castle_raw_history:?}, damage={castle_damage_history:?}, history={castle_history:?}",
            state.entities()[usize::from(castle)]);
        let mut defender_loads = before_defenders.iter().filter_map(|(&slot, &(handle, mana, _, _))|
            handle.map(|handle| ((slot, handle.generation), mana)))
            .collect::<std::collections::BTreeMap<_, _>>();
        // Reconstruct sphere amounts in event order for this source pass. A
        // merge transfers only already-paid mana, not its unpaid neighbour.
        let mut sphere_loads = before_spheres.iter().map(|(&slot, &(generation, _, mana))|
            ((slot, generation), mana)).collect::<std::collections::BTreeMap<_, _>>();
        let arrival_total = state.economy().events()[cursor..].iter().fold(0_i32, |sum, event| match event {
            Level2EconomyEvent::CastleDefenderArrived { castle_id, mana, .. } if *castle_id == castle => sum.wrapping_add(*mana),
            _ => sum,
        });
        let event_sphere_key = |id: u32| {
            let slot = u16::try_from(id).ok()?;
            let generation = state.economy().slot_generation(slot);
            if before_spheres.get(&slot).is_some_and(|(before, _, _)| *before != generation) { return None; }
            Some((slot, generation))
        };
        for event in &state.economy().events()[cursor..] {
            if matches!(event, Level2EconomyEvent::ManaSphereClaimed { .. } | Level2EconomyEvent::ManaSpheresMerged { .. }
                | Level2EconomyEvent::CastleDefenderCollectedSphere { .. } | Level2EconomyEvent::CastleDefenderArrived { .. }) {
                mana_events.push((turn, *event));
            }
            match *event {
                Level2EconomyEvent::ManaSphereClaimed { sphere_id, owner_entity_index } if owner_entity_index == player => {
                    if let Some(key) = event_sphere_key(sphere_id) && impact_targets.contains(&key) {
                        let amount = *sphere_loads.get(&key).unwrap_or_else(|| panic!(
                            "paid claim requires the actual ordered sphere payload: turn={turn}, key={key:?}, before={before_spheres:?}, events={mana_events:?}"));
                        assert!(amount > 0, "paid claim must have positive source payload: turn={turn}, key={key:?}, amount={amount}, events={mana_events:?}");
                        paid_lineage.insert(key, (amount, amount));
                        claim = true;
                    }
                }
                Level2EconomyEvent::ManaSphereClaimed { sphere_id, .. } => {
                    if let Some(key) = event_sphere_key(sphere_id) { paid_lineage.remove(&key); }
                }
                Level2EconomyEvent::ManaSpheresMerged { target_sphere_id, source_sphere_id, owner_entity_index, mana } => {
                    let target = event_sphere_key(target_sphere_id);
                    let source = event_sphere_key(source_sphere_id);
                    let touches_paid = paid_lineage.keys().any(|(slot, _)| u32::from(*slot) == target_sphere_id || u32::from(*slot) == source_sphere_id);
                    let amounts = target.zip(source).and_then(|(target, source)|
                        sphere_loads.get(&target).copied().zip(sphere_loads.get(&source).copied())
                            .map(|(target_mana, source_mana)| (target, source, target_mana, source_mana)));
                    if let Some((target, source, target_mana, source_mana)) = amounts && source_mana == mana {
                        let merged = target_mana.checked_add(source_mana).expect("actual merged sphere payload fits i32");
                        sphere_loads.insert(target, merged);
                        sphere_loads.remove(&source);
                        let target_paid = paid_lineage.remove(&target);
                        let source_paid = paid_lineage.remove(&source);
                        assert!(target_paid.is_none_or(|(_, expected)| expected == target_mana)
                            && source_paid.is_none_or(|(_, expected)| expected == source_mana),
                            "paid merge inputs must match their last verified totals: turn={turn}, target={target:?}/{target_paid:?}/{target_mana}, source={source:?}/{source_paid:?}/{source_mana}, events={mana_events:?}");
                        let paid = target_paid.map_or(0, |(paid, _)| paid)
                            .checked_add(source_paid.map_or(0, |(paid, _)| paid)).expect("paid merged subtotal fits i32");
                        if paid > 0 && owner_entity_index == player && eligible.contains_key(&target) {
                            assert!(paid <= merged, "paid subtotal cannot exceed actual merged payload: turn={turn}, paid={paid}, merged={merged}, events={mana_events:?}");
                            paid_lineage.insert(target, (paid, merged));
                        }
                    } else {
                        assert!(!touches_paid, "paid merge requires current generation and ordered payloads: turn={turn}, target={target:?}, source={source:?}, source_mana={mana}, loads={sphere_loads:?}, lineage={paid_lineage:?}, events={mana_events:?}");
                        if let Some(target) = target { sphere_loads.remove(&target); }
                        if let Some(source) = source { sphere_loads.remove(&source); }
                    }
                }
                Level2EconomyEvent::CastleDefenderCollectedSphere { defender_slot, sphere_id, mana } => {
                    let defender = state.source_arena().handle(defender_slot);
                    if let Some(handle) = defender && let Some(load) = defender_loads.get_mut(&(defender_slot, handle.generation)) {
                        *load = load.wrapping_add(mana);
                    }
                    if let Some(key) = event_sphere_key(sphere_id) && let Some((paid_amount, expected)) = paid_lineage.remove(&key) {
                        assert!(claim && paid_mailbox && mana > 0,
                            "paid lineage must deliver positive source mana: turn={turn}, claim={claim}, paid_mailbox={paid_mailbox}, mana={mana}, key={key:?}, eligible={eligible:?}, before={before_sphere:?}, after={after_sphere:?}, defenders={before_defenders:?}, total={total}, aggregate={aggregate}, bank={bank}, capacity={capacity}, events={mana_events:?}, possession={possession_history:?}, sphere_history={sphere_history:?}");
                        assert_eq!(sphere_loads.get(&key), Some(&mana),
                            "actual collection must match the ordered sphere payload: turn={turn}, key={key:?}, paid={paid_amount}, loads={sphere_loads:?}, events={mana_events:?}");
                        assert_eq!(expected, mana, "paid sphere must retain its last verified payload until actual collection: turn={turn}, key={key:?}, paid={paid_amount}, events={mana_events:?}");
                        assert!(paid_amount > 0 && paid_amount <= mana,
                            "collector's paid subtotal must be positive and no larger than its actual payload: turn={turn}, key={key:?}, paid={paid_amount}, mana={mana}, events={mana_events:?}");
                        let handle = defender.unwrap_or_else(|| panic!(
                            "paid collection must retain its actual defender generation: turn={turn}, defender={defender_slot}, sphere={key:?}, mana={mana}, before={before_defenders:?}, events={mana_events:?}, possession={possession_history:?}"));
                        collected.entry((handle.source_slot, handle.generation)).or_default().insert(key, paid_amount);
                        collector = Some((handle, mana));
                    }
                }
                Level2EconomyEvent::CastleDefenderArrived { defender_slot, castle_id, mana }
                    if castle_id == castle && mana > 0 => {
                        let Some(handle) = state.source_arena().handle(defender_slot) else { continue };
                        let Some(payloads) = collected.get(&(handle.source_slot, handle.generation)) else { continue };
                        let paid_payload = payloads.values().try_fold(0_i32, |sum, value| sum.checked_add(*value)).expect("actual paid collector payload fits i32");
                        assert_eq!(defender_loads.get(&(handle.source_slot, handle.generation)).copied(), Some(mana),
                            "source collector deposits its full current payload: turn={turn}, collected={collected:?}, before={before_defenders:?}, events={mana_events:?}, sphere_history={sphere_history:?}");
                        assert!(paid_payload > 0 && mana >= paid_payload,
                            "actual arriving defender must include its paid source payload: turn={turn}, mana={mana}, collected={collected:?}, events={mana_events:?}");
                        assert_eq!(after_castle.map(|row| row.mana).zip(before_castle.map(|row| row.mana))
                            .map(|(after, before)| after.wrapping_sub(before)), Some(arrival_total),
                            "the real Castle must receive its source arrivals: turn={turn}, before={before_castle:?}, after={after_castle:?}, events={mana_events:?}");
                        assert!(before_percent < 65 && percent(&game, player) >= 65,
                            "actual collector credit must cross 65%: turn={turn}, total={total}, aggregate={aggregate}, bank={bank}, capacity={capacity}, target_initial={sphere:?}, event_mana={mana}, before_percent={before_percent}, current_total={}, current_aggregate={}, percent={}, events={mana_events:?}, sphere_history={sphere_history:?}", state.economy().owner_mana_global(), state.economy().owner_mana_aggregate(player), percent(&game, player));
                        let sampled_total = state.economy().owner_mana_global();
                        let sampled_owned = state.economy().owner_mana_aggregate(player);
                        let sampled_castle = after_castle.expect("the sampled Castle remains current").mana;
                        let without_paid = sampled_owned.saturating_add(sampled_castle).checked_sub(paid_payload)
                            .expect("paid subtotal fits the sampled Castle numerator");
                        assert!(sampled_total > 0 && without_paid >= 0 && 100_i64 * i64::from(without_paid) / i64::from(sampled_total) < 65,
                            "traced paid subtotal must be necessary at the real source objective sample: turn={turn}, sampled_total={sampled_total}, sampled_owned={sampled_owned}, sampled_castle={sampled_castle}, paid={paid_payload}, collected={collected:?}, events={mana_events:?}, sphere_history={sphere_history:?}");
                        credited = true;
                    }
                _ => {}
            }
        }
        cursor = state.economy().events().len();
        if !credited {
            assert!(percent(&game, player) < 65 && state.runtime().objectives[owner].stage_selectors[6] == 1,
                "source 65% goal must remain incomplete before the traced positive collector credit: turn={turn}, percent={}, total={total}, aggregate={aggregate}, bank={bank}, capacity={capacity}, eligible={eligible:?}, impacts={impact_targets:?}, lineage={paid_lineage:?}, collected={collected:?}, events={mana_events:?}, possession={possession_history:?}, sphere_history={sphere_history:?}", percent(&game, player));
        }
        if credited && receipt.groups[4] == 1 && receipt.groups[5] == 1 { break; }
    }
    let _ = crate::native_world::level2_dynamic_economy::take_possession_trace();
    assert!(paid_launch && paid_debit && paid_mailbox && claim && retreated && collector.is_some() && credited,
        "actual paid sphere claim and collector arrival must earn the 65 percent crossing: paid_launch={paid_launch}, paid_debit={paid_debit}, paid_mailbox={paid_mailbox}, claim={claim}, retreated={retreated}, collector={collector:?}, credited={credited}, total={total}, aggregate={aggregate}, bank={bank}, capacity={capacity}, initial_sphere={sphere:?}, possession={possession_history:?}, events={mana_events:?}, sphere_history={sphere_history:?}, damage={castle_damage_history:?}, history={castle_history:?}");
    assert_eq!(receipt.groups, [1; 6]);
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[..7], [2; 7]);
    assert_eq!(sim(&game).runtime().objectives[owner].is_level_end, 1);
    let (position, trigger) = receipt.exit.expect("actual level-complete switch publishes group18 exit");
    receipt.carried_holder = None;
    let sequence = sim(&game).level_exit_sequence();
    if !sequence.active && !sequence.complete {
        assert!(sim(&game).source_arena().resolve(trigger).is_some());
        sim_mut(&mut game).set_level2_entity_position(player, position).unwrap();
    }
    for _ in 0..700 {
        if matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }) { break; }
        ordinary_step(&mut game, &mut receipt);
    }
    assert!(matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }));
    assert_eq!(sim(&game).source_exit_result_byte(), 0x0a);
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
    // Source17 is outer state5 / inner phase6 (idle curl), not fatal
    // outer state6. The fatal mailbox above selects6, which initializes7
    // and enters the silent death/collapse path instead of the idle call.
    let mut idle_curl_audio_during_death = false;
    for _ in 0..64 {
        let cursor = session.effect_journal().cursor(0).next_sequence;
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "source scheduler error: {:?}",
            session.dispatch_events()
        );
        idle_curl_audio_during_death |= session
            .effect_journal()
            .records()
            .iter()
            .filter(|record| record.sequence >= cursor)
            .any(|record| {
                matches!(record.effect,
                crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                    audio_site: crate::MissionAudioSite::HydraHeadRegrowth,
                    entity_index, player_index: -1, sound_id: 17,
                }) if entity_index == root_slot)
            });
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
    assert!(
        !idle_curl_audio_during_death,
        "fatal6→7 collapse must not invent the idle5/phase6 source17 call"
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
        &mut |_, _, _, _| {},
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
        &mut |_, _, _, _| {},
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
        &mut |_, _, _, _| {},
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
            &mut |_, _, _, _| {},
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
        &mut |_, _, _, _| {},
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
        &mut |_, _, _, _| {},
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
        &mut |_, _, _, _| {},
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
            &mut |_, _, _, _| {},
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
            &mut |_, _, _, _| {},
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
            &mut |_, _, _, _| {},
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
            &mut |_, _, _, _| {},
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
            &mut |_, _, _, _| {},
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
            &mut |_, _, _, _| {},
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
            &mut |_, _, _, _| {},
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
        &mut |_, _, _, _| {},
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
        &mut |_, _, _, _| {},
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
            &mut |_, _, _, _| {},
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
            &mut |_, _, _, _| {},
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
            &mut |_, _, _| {},
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
        &mut |_, _, _, _| {},
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
        &mut |_, _, _, _| {},
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
        &mut |_, _, _, _| {},
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
        &mut |_, _, _, _| {},
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
        &mut |_, _, _, _| {},
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
        &mut |_, _, _, _| {},
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
        &mut |_, _, _, _| {},
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
