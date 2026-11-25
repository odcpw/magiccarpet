//! Public-command acceptance for the package's late Archer and Bee combat.
//! Focused tests explicitly stage a starting boundary; combat drivers never
//! write player pose, creature life, damage mailboxes, or objective selectors.

use super::*;
use crate::game_session::GameSession;
use std::collections::BTreeSet;

use super::tests::{guard, level_three_descriptor_fixture_path};

pub(crate) const ARCHER_ROWS: [u16; 18] = [
    177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194,
];
pub(crate) const FINAL_BEE_ROWS: [u16; 6] = [149, 150, 152, 153, 154, 155];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CombatFamily {
    Archer,
    Bee,
}

#[derive(Clone, Copy, Debug)]
struct Target {
    row: u16,
    slot: u16,
    generation: u32,
    life: i32,
    attacker: u16,
    position: crate::Position,
    action: u8,
    drawing: bool,
}

/// The recipient, not the requested aim point, identifies a source hit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PublicCombatReceipt {
    pub row: u16,
    pub slot: u16,
    pub generation: u32,
    pub life_before: i32,
    pub life_after: i32,
    pub projectile_slot: u16,
}

fn targets(session: &GameSession<Mc2CampaignBackend>, family: CombatFamily) -> Vec<Target> {
    let economy = session.backend().campaign.simulation().economy();
    match family {
        CombatFamily::Archer => economy
            .authored_class5()
            .iter()
            .filter(|source| source.model == 4 && ARCHER_ROWS.contains(&source.authored_index))
            .map(|source| Target {
                row: source.authored_index,
                slot: source.source_entity_index,
                generation: economy.slot_generation(source.source_entity_index),
                life: source.life,
                attacker: source.word_0x26,
                position: source.position,
                action: source.action,
                drawing: source.drawing_enabled,
            })
            .collect(),
        CombatFamily::Bee => economy
            .bee_creatures()
            .iter()
            .filter(|source| {
                source
                    .authored_index()
                    .is_some_and(|row| FINAL_BEE_ROWS.contains(&row))
            })
            .map(|source| Target {
                row: source.authored_index().unwrap(),
                slot: source.source_entity_index,
                generation: economy.slot_generation(source.source_entity_index),
                life: i32::from(source.life),
                attacker: source.damage_attacker_entity_index,
                position: source.position,
                action: source.action,
                drawing: source.drawing_enabled,
            })
            .collect(),
    }
}

fn distance(left: crate::Position, right: crate::Position) -> u32 {
    crate::distance_3d_wrapped(left, right)
}

fn clear_fireball_path(
    session: &GameSession<Mc2CampaignBackend>,
    mut launch: crate::Position,
    target: &Target,
    family: CombatFamily,
) -> bool {
    let compatibility = session.backend().campaign.simulation();
    launch.z = launch.z.wrapping_add(compatibility.state.entity_fov);
    let mut aim = target.position;
    aim.z = aim
        .z
        .wrapping_add(if family == CombatFamily::Bee { 14 } else { 78 });
    let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(launch, aim);
    let turns = distance(launch, aim).div_ceil(384);
    for _ in 0..turns {
        crate::move_entity(&mut launch, yaw, pitch, 384);
        if crate::FlightWorld::ground_height(compatibility.canonical_terrain(), launch) >= launch.z
        {
            return false;
        }
    }
    true
}

fn step(session: &mut GameSession<Mc2CampaignBackend>, command: GameCommand) {
    let bees_before = session
        .backend()
        .campaign
        .simulation()
        .economy()
        .bee_creatures()
        .to_vec();
    let player = session
        .backend()
        .campaign
        .simulation()
        .shared_spells()
        .player_entity();
    assert!(player.life >= 0, "public combat player died: {player:?}");
    session.step(guard(session), command).unwrap();
    let economy = session.backend().campaign.simulation().economy();
    for previous in bees_before
        .iter()
        .filter(|bee| bee.life >= 0 && bee.drawing_enabled)
    {
        let next = economy
            .bee_creatures()
            .iter()
            .find(|bee| bee.source_entity_index == previous.source_entity_index);
        if next.is_none_or(|bee| bee.life < 0 || !bee.drawing_enabled) {
            eprintln!(
                "Bee terminal tick={} row={:?} before={:?} after={:?}",
                session.identity().tick,
                previous.authored_index(),
                (previous.life, previous.action, previous.position),
                next.map(|bee| (
                    bee.life,
                    bee.action,
                    bee.damage_attacker_entity_index,
                    bee.position
                ))
            );
        }
    }
}

fn combat_wait_command(
    session: &GameSession<Mc2CampaignBackend>,
    family: CombatFamily,
) -> GameCommand {
    let state = session.backend().campaign.simulation().state;
    if family == CombatFamily::Archer {
        return GameCommand {
            longitudinal: if state.actual_speed > 0 {
                LongitudinalControl::Decelerate
            } else if state.actual_speed < 0 {
                LongitudinalControl::Accelerate
            } else {
                LongitudinalControl::Coast
            },
            ..GameCommand::default()
        };
    }
    let Some(target) = targets(session, family)
        .into_iter()
        .filter(|target| target.life >= 0 && target.drawing)
        .min_by_key(|target| (distance(state.position, target.position), target.slot))
    else {
        return GameCommand::default();
    };
    let away = (crate::heading_for_positions(state.position, target.position) as i16)
        .wrapping_add(1024)
        & 2047;
    let shared = session.backend().campaign.simulation().shared_spells();
    let speed_active = shared
        .pickups()
        .iter()
        .chain(shared.holders())
        .chain(shared.dynamic_pickups())
        .find(|holder| holder.index == shared.local_player().enabled_spell_holders[3])
        .is_some_and(|holder| holder.active_value > 0);
    GameCommand {
        // remc2 deliberately cancels Speed on a manual speed-change command.
        longitudinal: if speed_active {
            LongitudinalControl::Coast
        } else {
            LongitudinalControl::Accelerate
        },
        roll_axis: crate::turn_toward(state.yaw, away, 24) as i8,
        pitch_axis: -127,
        ..GameCommand::default()
    }
}

/// Same ordinary producer as the player: selected carried holder -> class-9
/// projectile -> class-10/model-0 pulse -> source mailbox -> creature callback.
/// Only public flight/look controls alter the incoming session.
pub(crate) fn fire_public_combat_for_test(
    session: &mut GameSession<Mc2CampaignBackend>,
    family: CombatFamily,
) -> Vec<PublicCombatReceipt> {
    if family == CombatFamily::Bee {
        let compatibility = session.backend().campaign.simulation();
        let shared = compatibility.shared_spells();
        let speed_slot = shared.local_player().enabled_spell_holders[3];
        let can_start_speed = shared
            .pickups()
            .iter()
            .chain(shared.holders())
            .chain(shared.dynamic_pickups())
            .find(|holder| holder.index == speed_slot)
            .is_some_and(|holder| {
                holder.active
                    && holder.active_value == 0
                    && shared.player_mana() >= holder.max_mana + 100
            });
        if can_start_speed {
            step(
                session,
                GameCommand {
                    select_spell: Some(SemanticId::parse("spell.speed").unwrap()),
                    cast_selected: true,
                    longitudinal: LongitudinalControl::Coast,
                    ..combat_wait_command(session, family)
                },
            );
        }
    }
    // Finish the preceding impact's eight-life tail and following-pass reclaim.
    for _ in 0..64 {
        let compatibility = session.backend().campaign.simulation();
        let player = compatibility.state.entity_index;
        let economy = compatibility.economy();
        let shared = compatibility.shared_spells();
        let holder_slot = shared.local_player().enabled_spell_holders[0];
        let ready = shared
            .pickups()
            .iter()
            .chain(shared.holders())
            .chain(shared.dynamic_pickups())
            .find(|holder| holder.index == holder_slot)
            .is_some_and(|holder| {
                holder.active && holder.active_value == 0 && shared.player_mana() >= holder.max_mana
            });
        if ready
            && !economy
                .fireball_projectiles()
                .iter()
                .any(|row| row.owner_entity_index == player)
            && !economy.model17_spawned_children().iter().any(|row| {
                row.source_class == 10 && row.model == 0 && row.parent_entity_id == player
            })
        {
            break;
        }
        step(session, combat_wait_command(session, family));
    }

    for _ in 0..640 {
        let state = session.backend().campaign.simulation().state;
        let Some(target) = targets(session, family)
            .into_iter()
            .filter(|target| target.life >= 0 && target.drawing)
            .min_by_key(|target| (distance(state.position, target.position), target.slot))
        else {
            return Vec::new();
        };
        let firing_range = if family == CombatFamily::Bee {
            3000
        } else {
            900
        };
        if distance(state.position, target.position) <= firing_range
            && (family == CombatFamily::Archer
                || clear_fireball_path(session, state.position, &target, family))
        {
            break;
        }
        let heading = crate::heading_for_positions(state.position, target.position) as i16;
        let heading_error = crate::turn_toward(state.yaw, heading, 1024);
        let brake = if state.actual_speed > 0 {
            LongitudinalControl::Decelerate
        } else if state.actual_speed < 0 {
            LongitudinalControl::Accelerate
        } else {
            LongitudinalControl::Coast
        };
        step(
            session,
            GameCommand {
                longitudinal: if heading_error.abs() < 128 {
                    LongitudinalControl::Accelerate
                } else {
                    brake
                },
                roll_axis: heading_error.clamp(-24, 24) as i8,
                pitch_axis: -127,
                ..GameCommand::default()
            },
        );
    }

    let before = targets(session, family);
    let waiting_command = combat_wait_command(session, family);
    let mut forecast = session.backend().campaign.clone();
    forecast.step_with_tuning(
        campaign_player_command(&GameCommand::default()),
        CAMPAIGN_SPEED_INDEX,
        session.backend().flight_binding.tuning(),
    );
    let compatibility = session.backend().campaign.simulation();
    let state = forecast.simulation().state;
    let target = before
        .iter()
        .filter(|target| target.life >= 0 && target.drawing)
        .min_by_key(|target| {
            (
                family == CombatFamily::Bee
                    && !clear_fireball_path(session, state.position, target, family),
                distance(state.position, target.position),
                target.slot,
            )
        })
        .expect("one current package recipient remains");
    let attacker = state.entity_index;
    let direction_bits = compatibility.shared_spells().player_entity().control_flags & 0x0300;
    let previous_projectiles = compatibility
        .economy()
        .fireball_projectiles()
        .iter()
        .map(|row| {
            (
                row.source_entity_index,
                compatibility
                    .economy()
                    .slot_generation(row.source_entity_index),
            )
        })
        .collect::<BTreeSet<_>>();
    let previous_impacts = compatibility
        .economy()
        .model17_spawned_children()
        .iter()
        .map(|row| {
            (
                row.source_entity_index,
                compatibility
                    .economy()
                    .slot_generation(row.source_entity_index),
            )
        })
        .collect::<BTreeSet<_>>();
    let mut launch = state.position;
    let hand = if direction_bits & 0x0100 != 0 {
        Some(state.yaw.wrapping_sub(512))
    } else if direction_bits & 0x0200 != 0 {
        Some(state.yaw.wrapping_add(512))
    } else {
        None
    };
    if let Some(hand) = hand {
        crate::move_entity(&mut launch, hand, 0, 256);
        if crate::FlightWorld::ground_height(compatibility.canonical_terrain(), launch) > launch.z {
            launch = state.position;
        }
    }
    launch.z = launch.z.wrapping_add(state.entity_fov);
    // Choose a discrete intercept, not the time to the target's old position.
    // A charging Bee can halve that distance during the projectile's flight.
    let mut aim = target.position;
    if family == CombatFamily::Bee {
        let mut best = None;
        for turn in 1..=12_u32 {
            if let Some(bee) = forecast
                .simulation()
                .economy()
                .bee_creatures()
                .iter()
                .find(|bee| bee.source_entity_index == target.slot)
            {
                let mut candidate = bee.position;
                candidate.z = candidate.z.wrapping_add(14);
                let (yaw, pitch) =
                    crate::level1_mission::projectile_guidance_angles(launch, candidate);
                let mut projectile = launch;
                let mut blocked = false;
                for _ in 0..turn {
                    crate::move_entity(&mut projectile, yaw, pitch, 384);
                    blocked |= crate::FlightWorld::ground_height(
                        compatibility.canonical_terrain(),
                        projectile,
                    ) >= projectile.z;
                }
                let score = distance(projectile, candidate) + if blocked { 1 << 24 } else { 0 };
                if best.is_none_or(|(best_score, _)| score < best_score) {
                    best = Some((score, candidate));
                }
            }
            forecast.step_with_tuning(
                campaign_player_command(&waiting_command),
                CAMPAIGN_SPEED_INDEX,
                session.backend().flight_binding.tuning(),
            );
        }
        if let Some((_, candidate)) = best {
            aim = candidate;
        }
    } else {
        let flight_turns = crate::distance_3d_wrapped(state.position, target.position)
            .div_ceil(384)
            .clamp(1, 8);
        for _ in 1..flight_turns {
            forecast.step_with_tuning(
                campaign_player_command(&combat_wait_command(session, family)),
                CAMPAIGN_SPEED_INDEX,
                session.backend().flight_binding.tuning(),
            );
        }
        if let Some(archer) = forecast
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|row| row.source_entity_index == target.slot)
        {
            // Grounded Archers have vertical extent; aim above the terrain,
            // rather than below their feet where the shot hits a ridge first.
            aim = archer.position;
            aim.z = aim.z.wrapping_add(128);
        }
        aim.z = aim
            .z
            .wrapping_sub(crate::level1_mission::FIREBALL_COLLISION_VERTICAL_OFFSET);
    }
    let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(launch, aim);
    step(
        session,
        GameCommand {
            select_spell: Some(SemanticId::parse("spell.fireball").unwrap()),
            cast_selected: true,
            look_yaw: yaw.wrapping_sub(state.yaw),
            look_pitch: pitch.wrapping_sub(state.display_pitch),
            ..GameCommand::default()
        },
    );

    let mut projectile_slot = None;
    let mut impact_occurrence = None;
    for _ in 0..32 {
        let economy = session.backend().campaign.simulation().economy();
        if let Some(projectile) = economy.fireball_projectiles().iter().find(|row| {
            row.owner_entity_index == attacker
                && row.source_subspell_index == crate::level1_mission::FIREBALL_PAYLOAD as u16
                && !previous_projectiles.contains(&(
                    row.source_entity_index,
                    economy.slot_generation(row.source_entity_index),
                ))
        }) {
            projectile_slot = Some(projectile.source_entity_index);
        }
        if let Some(impact) = economy.model17_spawned_children().iter().find(|row| {
            row.source_class == 10
                && row.model == 0
                && row.parent_entity_id == attacker
                && row.subspell_index == crate::level1_mission::FIREBALL_PAYLOAD as u16
                && !previous_impacts.contains(&(
                    row.source_entity_index,
                    economy.slot_generation(row.source_entity_index),
                ))
        }) {
            projectile_slot = Some(impact.parent_entity_index);
            impact_occurrence = Some((
                impact.source_entity_index,
                economy.slot_generation(impact.source_entity_index),
                impact.position,
            ));
        }
        let current = targets(session, family);
        let receipts = before
            .iter()
            .filter_map(|previous| {
                let recipient = current.iter().find(|row| {
                    row.row == previous.row
                        && row.slot == previous.slot
                        && row.generation == previous.generation
                })?;
                (recipient.life
                    == previous
                        .life
                        .wrapping_sub(i32::from(crate::level1_mission::FIREBALL_PAYLOAD))
                    && recipient.attacker == attacker)
                    .then(|| PublicCombatReceipt {
                        row: recipient.row,
                        slot: recipient.slot,
                        generation: recipient.generation,
                        life_before: previous.life,
                        life_after: recipient.life,
                        projectile_slot: projectile_slot.expect(
                            "player-owned payload producer accompanies exact mailbox delta",
                        ),
                    })
            })
            .collect::<Vec<_>>();
        if !receipts.is_empty() {
            return receipts;
        }
        step(
            session,
            if family == CombatFamily::Bee {
                waiting_command.clone()
            } else {
                combat_wait_command(session, family)
            },
        );
    }
    eprintln!(
        "public {family:?} missed: player={:?}, target={target:?}, launch={launch:?}, aim={aim:?}, projectile={projectile_slot:?}, impact={impact_occurrence:?}",
        session.backend().campaign.simulation().state.position
    );
    Vec::new()
}

pub(crate) fn retire_public_combat_for_test(
    session: &mut GameSession<Mc2CampaignBackend>,
    family: CombatFamily,
) -> Vec<PublicCombatReceipt> {
    let initial = targets(session, family);
    assert!(!initial.is_empty());
    let mut receipts = Vec::new();
    let mut misses = 0;
    for attempt in 0..512 {
        if targets(session, family)
            .iter()
            .all(|row| row.life < 0 || !row.drawing)
        {
            break;
        }
        let hit = fire_public_combat_for_test(session, family);
        misses = if hit.is_empty() { misses + 1 } else { 0 };
        assert!(
            misses < 4,
            "four consecutive public shots missed {family:?}"
        );
        if misses != 0 && family == CombatFamily::Archer {
            // A terrain impact from rest will repeat forever from the same
            // launch point. Move laterally through public controls before
            // approaching again; do not change the shot or damage gates.
            for _ in 0..16 {
                step(
                    session,
                    GameCommand {
                        lateral: crate::game_session::LateralControl::Positive,
                        pitch_axis: -127,
                        ..GameCommand::default()
                    },
                );
            }
        }
        if attempt % 16 == 0 {
            eprintln!(
                "public {family:?}: attempt={attempt}, tick={}, receipts={}, live={:?}",
                session.identity().tick,
                receipts.len(),
                targets(session, family)
                    .iter()
                    .map(|row| (row.row, row.life, row.action))
                    .collect::<Vec<_>>()
            );
        }
        receipts.extend(hit);
    }
    let terminal = receipts
        .iter()
        .filter(|row| row.life_after < 0)
        .map(|row| (row.row, row.slot, row.generation))
        .collect::<BTreeSet<_>>();
    for row in &initial {
        assert!(
            terminal.contains(&(row.row, row.slot, row.generation)),
            "every initial recipient needs a public lethal pulse: family={family:?}, row={row:?}, remaining={:?}",
            targets(session, family)
        );
    }
    for _ in 0..32 {
        if targets(session, family).is_empty() {
            break;
        }
        step(session, GameCommand::default());
    }
    assert!(
        targets(session, family).is_empty(),
        "ordinary following-pass reclaim"
    );
    if family == CombatFamily::Archer {
        let economy = session.backend().campaign.simulation().economy();
        for row in initial {
            assert!(economy.events().iter().any(|event| matches!(event,
                crate::native_world::Level2EconomyEvent::ManaSphereSpawned { source_entity_index, mana: 500, .. }
                if *source_entity_index == row.slot)), "Archer death outputs its ordinary 500-mana sphere");
        }
    }
    receipts
}

fn staged_group_boundary(group: u16) -> GameSession<Mc2CampaignBackend> {
    let root = level_three_descriptor_fixture_path();
    let definition = Arc::new(load_mc2_campaign(&root, &default_catalog().unwrap()).unwrap());
    let mut session = GameSession::open(
        &Mc2CampaignAdapter,
        definition,
        63,
        Mc2CampaignInputs::new(&root),
    )
    .unwrap();
    session
        .backend_mut()
        .campaign
        .simulation_mut()
        .apply_carried_spell_families(&[0, 1, 2, 3], Some(0))
        .unwrap();
    if group == 3 {
        super::tests::build_real_level_three_carried_castle_for_test(&mut session);
        super::tests::close_real_level_three_strip_for_staged_bees_for_test(&mut session);
    }
    let compatibility = session.backend_mut().campaign.simulation_mut();
    compatibility
        .activate_authored_group_prerequisite_for_test(group)
        .unwrap();
    if group == 3 {
        compatibility
            .stage_level_three_final_bee_objective_prerequisite_for_test()
            .unwrap();
        let first = compatibility.economy().bee_creatures()[0];
        let mut position = crate::Position {
            x: first.position.x.wrapping_add(800),
            ..first.position
        };
        position.z = crate::FlightWorld::ground_height(compatibility.canonical_terrain(), position)
            .wrapping_add(256);
        compatibility
            .set_level2_entity_position(compatibility.state.entity_index, position)
            .unwrap();
    }
    session
}

#[test]
fn archer_projectile_does_not_join_its_shooters_health_chain() {
    let (mut session, shooter) = publicly_provoked_archer_boundary();
    for _ in 0..180 {
        let command = combat_wait_command(&session, CombatFamily::Archer);
        step(&mut session, command);
        let economy = session.backend().campaign.simulation().economy();
        if let Some(child) = economy
            .authored_class5_archer_children()
            .iter()
            .find(|child| child.parent_entity_index == shooter.slot)
        {
            assert_eq!((child.source_class, child.model), (9, 13));
            assert!(
                (0..=13).contains(&child.life),
                "the ordinary source pass can visit a newly allocated arrow"
            );
            let parent = economy
                .authored_class5_archer()
                .iter()
                .find(|parent| parent.source_entity_index == shooter.slot)
                .unwrap();
            assert_eq!(
                parent.word_0x34, 0,
                "sub_1CCE0 launches an independent arrow, not a health-linked body part"
            );
            assert_eq!(child.word_0x34, 0);
            return;
        }
    }
    panic!("the ordinary Archer callback did not launch its projectile");
}

/// The package's StageVar1 is mode4/flags3/model9: Archers inherit a
/// Skeleton's target through `1D700`, rather than attack a nearby player
/// unconditionally. A real public Fireball supplies the `1D8C0` hostile hit
/// that selects the player and mode10. No combat state is staged here.
fn publicly_provoked_archer_boundary() -> (GameSession<Mc2CampaignBackend>, Target) {
    let mut session = staged_group_boundary(5);
    let compatibility = session.backend().campaign.simulation();
    let stage = &compatibility.runtime().stage_variables[1];
    assert_eq!((stage.kind, stage.flags), (4, 3));
    assert_eq!(
        stage.relation,
        crate::native_world::RuntimeRelation::AxisScalar(9)
    );
    // Preserve the package's original player pose, from which the existing
    // retirement driver reaches a real Archer on its first public shot.
    // Repositioning near row177 instead puts that shot into terrain. The
    // actual receipt below, not the requested aim or first row, owns identity.
    let receipt = fire_public_combat_for_test(&mut session, CombatFamily::Archer)
        .into_iter()
        .next()
        .expect("a real public Fireball must provoke an Archer before its retaliation");
    assert_eq!(receipt.life_before - receipt.life_after, 250);
    assert!(
        receipt.life_after >= 0,
        "the provocation must leave a living shooter"
    );
    let shooter = targets(&session, CombatFamily::Archer)
        .into_iter()
        .find(|root| root.slot == receipt.slot && root.generation == receipt.generation)
        .expect("the exact Fireball recipient remains the retaliating shooter");
    let compatibility = session.backend().campaign.simulation();
    let source = compatibility
        .economy()
        .authored_class5()
        .iter()
        .find(|root| root.source_entity_index == shooter.slot)
        .unwrap();
    assert_eq!(source.word_0x96_150, compatibility.state.entity_index);
    assert_eq!(shooter.attacker, compatibility.state.entity_index);
    assert_eq!(shooter.life, receipt.life_after);
    assert_eq!(
        shooter.action, 34,
        "the actual hostile hit must select attack"
    );
    (session, shooter)
}

#[test]
fn ordinary_level_three_archer_arrow_damages_player_without_lowering_shooter_health() {
    let (mut session, shooter) = publicly_provoked_archer_boundary();
    let compatibility = session.backend().campaign.simulation();
    let player = compatibility.state.entity_index;
    let initial_life = compatibility.canonical_player_life_for_test();
    let event_start = compatibility.economy().events().len();
    // The public provocation legitimately costs the recipient 250 life.
    // Every later arrow visit must preserve this exact health/arena baseline.
    let health = |session: &GameSession<Mc2CampaignBackend>| {
        targets(session, CombatFamily::Archer)
            .into_iter()
            .map(|root| (root.row, root.slot, root.generation, root.life))
            .collect::<Vec<_>>()
    };
    let shooter_health_after_provocation = health(&session);
    let mut launched = BTreeSet::new();
    let mut trajectory = Vec::new();
    let mut diagnostic_event_start = event_start;
    for visit in 0..180 {
        let mut command = combat_wait_command(&session, CombatFamily::Archer);
        // sub_10780 rounds the arrow's search origin, while the player
        // is linked by its raw cell. Standing still can repeat a valid source
        // miss forever even when the physical boxes overlap. Strafe briefly
        // between shots through public input, then let ordinary decay stop
        // the player; keep the original bounded retaliation window.
        if visit % 30 < 4 {
            command.lateral = crate::game_session::LateralControl::Positive;
        }
        step(&mut session, command);
        let compatibility = session.backend().campaign.simulation();
        let economy = compatibility.economy();
        for child in economy
            .authored_class5_archer_children()
            .iter()
            .filter(|child| child.parent_entity_index == shooter.slot)
        {
            assert_eq!((child.source_class, child.model), (9, 13));
            launched.insert(child.entity_id);
        }
        let raw_player = &compatibility.entities()[usize::from(player)];
        let collision = raw_player.collision_target();
        let position = compatibility.state.position;
        let children = economy.authored_class5_archer_children().iter()
            .filter(|child| child.parent_entity_index == shooter.slot)
            .map(|child| {
                let mut predicted = child.position;
                crate::move_entity(&mut predicted, child.yaw, child.pitch, child.actual_speed);
                serde_json::json!({
                    "slot": child.source_entity_index,
                    "generation": economy.slot_generation(child.source_entity_index),
                    "owner": child.entity_id,
                    "target": child.word_0x96,
                    "type": [child.xtype, child.xsubtype],
                    "yaw_pitch": [child.yaw, child.pitch],
                    "position": [child.position.x, child.position.y, child.position.z],
                    "predicted": [predicted.x, predicted.y, predicted.z],
                    "predicted_ground": crate::FlightWorld::ground_height(compatibility.canonical_terrain(), predicted),
                    "life": child.life,
                    "speed": child.actual_speed,
                    "drawing": child.drawing_enabled,
                    "flags": child.control_flags,
                })
            }).collect::<Vec<_>>();
        let damage = economy.events()[diagnostic_event_start..]
            .iter()
            .filter_map(|event| {
                if let crate::native_world::Level2EconomyEvent::DamageApplied {
                    target_entity_index,
                    attacker_entity_index,
                    damage: 250,
                    lethal,
                } = event
                {
                    Some((*target_entity_index, *attacker_entity_index, *lethal))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        diagnostic_event_start = economy.events().len();
        // One frame per existing bounded source step, printed only on failure.
        // Hidden child positions are the actual arrow pulse/hide frontier;
        // disappearance in the next frame records following-pass reclaim.
        trajectory.push(serde_json::json!({
            "visit": visit, "tick": session.identity().tick,
            "player": {
                "slot": player,
                "position": [position.x, position.y, position.z],
                "raw_position": [raw_player.position.x, raw_player.position.y, raw_player.position.z],
                "collision": [collision.collision_half_width, collision.collision_vertical_offset, collision.collision_half_height],
                "life": compatibility.canonical_player_life_for_test(),
                "flags": raw_player.control_flags,
                "linked": economy.map_chains().is_linked(player),
                "tile": [position.x >> 8, position.y >> 8],
                "rounded_tile": [position.x.wrapping_add(128) >> 8, position.y.wrapping_add(128) >> 8],
            },
            "shooter": economy.authored_class5().iter()
                .find(|root| root.source_entity_index == shooter.slot).map(|root| format!("{root:?}")),
            "children": children,
            "damage250": damage,
        }));
        assert_eq!(
            health(&session),
            shooter_health_after_provocation,
            "arrows must neither join the health chain nor damage class-5 peers when targeting the player"
        );
        if economy.events()[event_start..].iter().any(|event| matches!(event,
            crate::native_world::Level2EconomyEvent::DamageApplied { target_entity_index, attacker_entity_index, damage: 250, lethal: false }
                if *target_entity_index == player && launched.contains(attacker_entity_index))) {
            // The source damage consumer starts the sixteen-visit healing
            // cooldown before this visit's upkeep; no same-visit regen applies.
            assert_eq!(
                compatibility.canonical_player_life_for_test(),
                initial_life - 250
            );
            return;
        }
    }
    for frame in trajectory {
        eprintln!("Archer trajectory {frame}");
    }
    panic!(
        "ordinary Archer launch never reached the player's damage consumer; shooters={launched:?}"
    );
}

#[test]
fn typed_archer_attacker_reaches_imported_skeleton_damage_reaction() {
    let mut session = staged_group_boundary(5);
    for _ in 0..24 {
        if session.backend().campaign.simulation().entities()[122].action == 79 {
            break;
        }
        step(&mut session, GameCommand::default());
    }
    let archer = targets(&session, CombatFamily::Archer)[0];
    let compatibility = session.backend_mut().campaign.simulation_mut();
    let skeleton = compatibility.entities()[122].clone();
    assert_eq!(
        (skeleton.class, skeleton.model, skeleton.action),
        (5, 9, 79)
    );
    assert!(
        !compatibility.entities()[usize::from(archer.slot)].active,
        "the live Archer is typed, not an imported reset row"
    );
    // Incoming damage boundary only. Ordinary Archer production is tested
    // separately; here the pulse must reach the real Skeleton reaction.
    compatibility.apply_fireball_impact_to_firefly_for_test(skeleton.position, archer.slot, 250);
    step(&mut session, GameCommand::default());
    let after = &session.backend().campaign.simulation().entities()[122];
    assert_eq!(after.life, skeleton.life - 250);
    assert_eq!(after.action, 74);
    assert_eq!((after.record_bytes()[69], after.record_bytes()[70]), (5, 4));
}

#[test]
fn public_fireballs_retire_real_level_three_group_five_archers_from_staged_group() {
    let mut session = staged_group_boundary(5);
    assert_eq!(
        targets(&session, CombatFamily::Archer)
            .iter()
            .map(|row| row.row)
            .collect::<Vec<_>>(),
        ARCHER_ROWS
    );
    let receipts = retire_public_combat_for_test(&mut session, CombatFamily::Archer);
    assert!(
        receipts
            .iter()
            .all(|row| row.life_before - row.life_after == 250)
    );
}

// This is an explicitly staged component command trace, not a production
// campaign save or organic mission proof. Recreate its existing fixture, then
// replay only actual driver commands; candidate forecast clones are excluded.
#[derive(serde::Serialize, serde::Deserialize)]
struct StagedBeeCommandTrace {
    fixture: String,
    initial_hash: u64,
    final_hash: u64,
    final_tick: u64,
    operations: Vec<crate::game_runtime::ReplayOperation<GameCommand>>,
}

fn retire_staged_bees_with_optional_trace(
    session: &mut GameSession<Mc2CampaignBackend>,
) -> Vec<super::tests::BeeFireballDamageReceipt> {
    let trace_path = std::env::var_os("MC_STAGED_BEE_COMMAND_TRACE");
    let initial_hash = session.backend().campaign.state_hash();
    let operation_start = session.backend().replay_operations.len();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        super::tests::retire_real_level_three_final_bees_with_public_commands_for_test(session)
    }));
    if let Some(path) = trace_path {
        use std::io::Write;
        let trace = StagedBeeCommandTrace {
            fixture: "staged_group_boundary(3):real_castle_and_strip_close".to_owned(),
            initial_hash,
            final_hash: session.backend().campaign.state_hash(),
            final_tick: session.identity().tick,
            operations: session.backend().replay_operations[operation_start..].to_vec(),
        };
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        output
            .write_all(&serde_json::to_vec(&trace).unwrap())
            .unwrap();
        eprintln!(
            "actual staged Bee command trace captured: {:?}, {} operations, tick{}",
            path,
            trace.operations.len(),
            trace.final_tick
        );
    }
    match result {
        Ok(receipts) => receipts,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn restore_actual_staged_bee_trace_frontier() -> GameSession<Mc2CampaignBackend> {
    let trace: StagedBeeCommandTrace = serde_json::from_slice(
        &std::fs::read(
            std::env::var_os("MC_STAGED_BEE_COMMAND_TRACE_INPUT")
                .expect("set an actual captured component trace"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        trace.fixture,
        "staged_group_boundary(3):real_castle_and_strip_close"
    );
    let mut session = staged_group_boundary(3);
    assert_eq!(session.backend().campaign.state_hash(), trace.initial_hash);
    for operation in trace.operations {
        match operation {
            crate::game_runtime::ReplayOperation::SpeechPreference {
                enabled,
                show_foreign_text,
            } => {
                session
                    .backend_mut()
                    .set_objective_presentation_preferences(enabled, show_foreign_text);
            }
            crate::game_runtime::ReplayOperation::Step { command } => {
                session.step(guard(&session), command).unwrap();
            }
            crate::game_runtime::ReplayOperation::Pause
            | crate::game_runtime::ReplayOperation::ToggleIconsMap
            | crate::game_runtime::ReplayOperation::TogglePanelTransparency
            | crate::game_runtime::ReplayOperation::ToggleFlatShading
            | crate::game_runtime::ReplayOperation::PlayerNamesNotice { .. }
            | crate::game_runtime::ReplayOperation::SourceAudioNotice { .. }
            | crate::game_runtime::ReplayOperation::FlightAssistanceNotice { .. }
            | crate::game_runtime::ReplayOperation::HelpModeNotice { .. }
            | crate::game_runtime::ReplayOperation::GraphicsNotice {..}
            | crate::game_runtime::ReplayOperation::PausedFrame { .. }
            | crate::game_runtime::ReplayOperation::PresentationFrame { .. } => {
                panic!("the staged Bee combat driver contains only gameplay input");
            }
        }
    }
    assert_eq!(session.identity().tick, trace.final_tick);
    assert_eq!(session.backend().campaign.state_hash(), trace.final_hash);
    session
}

#[test]
#[ignore = "requires an actual MC_STAGED_BEE_COMMAND_TRACE_INPUT; staged component departure proof"]
fn staged_bee_pilot_departs_on_live_contact_cadence_and_heals_at_real_castle() {
    // First qualify the complete captured chronology and final receipt.
    let _qualified_frontier = restore_actual_staged_bee_trace_frontier();
    let trace: StagedBeeCommandTrace = serde_json::from_slice(
        &std::fs::read(std::env::var_os("MC_STAGED_BEE_COMMAND_TRACE_INPUT").unwrap()).unwrap(),
    )
    .unwrap();
    let mut session = staged_group_boundary(3);
    assert_eq!(session.backend().campaign.state_hash(), trace.initial_hash);
    let mut departed = false;
    for operation in trace.operations {
        let compatibility = session.backend().campaign.simulation();
        let shared = compatibility.shared_spells();
        let holder = shared.local_player().enabled_spell_holders[0];
        let cost = shared
            .holders()
            .iter()
            .chain(shared.pickups())
            .chain(shared.dynamic_pickups())
            .find(|row| row.index == holder)
            .unwrap()
            .mana_cost;
        let raw =
            compatibility.entities()[usize::from(compatibility.state.entity_index)].record_bytes();
        let pending = i32::from_le_bytes(raw[98..102].try_into().unwrap());
        if pending > 0
            && shared.player_mana() >= cost
            && super::tests::public_bee_needs_castle_recovery_for_test(compatibility, cost)
        {
            let life_before = shared.player_entity().life;
            assert!(
                life_before > pending,
                "departure must precede the already-lethal packet"
            );
            super::tests::recover_public_bee_at_owned_castle_for_test(
                &mut session,
                cost,
                &mut BTreeSet::new(),
            );
            let after = session.backend().campaign.simulation();
            assert_eq!(
                after.shared_spells().player_entity().life,
                after.shared_spells().player_entity().max_life
            );
            assert!(
                after
                    .economy()
                    .player_castle_shift_aabb_overlap(after.shared_spells().player_entity())
            );
            let raw = after.entities()[usize::from(after.state.entity_index)].record_bytes();
            assert_eq!(&raw[98..104], &[0; 6]);
            departed = true;
            break;
        }
        match operation {
            crate::game_runtime::ReplayOperation::SpeechPreference {
                enabled,
                show_foreign_text,
            } => {
                session
                    .backend_mut()
                    .set_objective_presentation_preferences(enabled, show_foreign_text);
            }
            crate::game_runtime::ReplayOperation::Step { command } => {
                session.step(guard(&session), command).unwrap();
            }
            crate::game_runtime::ReplayOperation::Pause
            | crate::game_runtime::ReplayOperation::ToggleIconsMap
            | crate::game_runtime::ReplayOperation::TogglePanelTransparency
            | crate::game_runtime::ReplayOperation::ToggleFlatShading
            | crate::game_runtime::ReplayOperation::PlayerNamesNotice { .. }
            | crate::game_runtime::ReplayOperation::SourceAudioNotice { .. }
            | crate::game_runtime::ReplayOperation::FlightAssistanceNotice { .. }
            | crate::game_runtime::ReplayOperation::HelpModeNotice { .. }
            | crate::game_runtime::ReplayOperation::GraphicsNotice {..}
            | crate::game_runtime::ReplayOperation::PausedFrame { .. }
            | crate::game_runtime::ReplayOperation::PresentationFrame { .. } => {
                panic!("combat trace contains only gameplay input")
            }
        }
    }
    assert!(
        departed,
        "the actual captured contact prefix reaches early ordinary Castle departure"
    );
}

#[test]
#[ignore = "requires the actual tick869 MC_STAGED_BEE_COMMAND_TRACE_INPUT"]
fn staged_bee_pilot_consumes_exact_current_dragon_impact() {
    let mut session = restore_actual_staged_bee_trace_frontier();
    assert_eq!(session.identity().tick, 869);
    let before = session.backend().campaign.simulation();
    assert_eq!(before.shared_spells().player_entity().life, 6_375);
    let root = *before
        .economy()
        .authored_class5()
        .iter()
        .find(|root| root.source_entity_index == 282)
        .unwrap();
    assert_eq!((root.authored_index, root.model, root.action), (142, 0, 2));
    let generation = before.economy().slot_generation(282);
    assert_eq!(generation, 1);
    assert!(
        super::tests::public_bee_current_dragon_occurrence_is_qualified_for_test(
            &root, &root, generation, generation, 282
        )
    );
    assert!(
        !super::tests::public_bee_current_dragon_occurrence_is_qualified_for_test(
            &root,
            &root,
            generation,
            generation.wrapping_add(1),
            282
        )
    );
    let mut unrelated = root;
    unrelated.authored_index = 149;
    assert!(
        !super::tests::public_bee_current_dragon_occurrence_is_qualified_for_test(
            &unrelated, &unrelated, generation, generation, 282
        )
    );
    let event_start = before.economy().events().len();
    super::tests::step_public_bee_command_for_test(
        &mut session,
        GameCommand::default(),
        &mut BTreeSet::new(),
    );
    let after = session.backend().campaign.simulation();
    assert_eq!(after.shared_spells().player_entity().life, 5_875);
    assert_eq!(after.economy().slot_generation(282), generation);
    assert!(
        after.economy().events()[event_start..]
            .iter()
            .any(|event| matches!(
                event,
                crate::native_world::Level2EconomyEvent::DamageApplied {
                    target_entity_index: 252,
                    attacker_entity_index: 282,
                    damage: 500,
                    lethal: false,
                }
            ))
    );
}

#[test]
#[ignore = "requires MC_STAGED_BEE_COMMAND_TRACE_INPUT; replays only the explicitly staged component"]
fn staged_final_bee_command_trace_restores_exact_failed_frontier() {
    let session = restore_actual_staged_bee_trace_frontier();
    let before = session.backend().campaign.simulation().clone();
    let mut forecast = session.backend().campaign.clone();
    let motion = forecast.step_with_tuning(
        campaign_player_command(&GameCommand::default()),
        CAMPAIGN_SPEED_INDEX,
        session.backend().flight_binding.tuning(),
    );
    let after = forecast.simulation();
    let raw_mailbox = |state: &crate::native_world::Mc2Simulation| {
        let raw = state.entities()[usize::from(state.state.entity_index)].record_bytes();
        (
            i32::from_le_bytes(raw[98..102].try_into().unwrap()),
            u16::from_le_bytes(raw[102..104].try_into().unwrap()),
        )
    };
    for event in after
        .economy()
        .events()
        .get(before.economy().events().len()..)
        .unwrap()
    {
        if let crate::native_world::Level2EconomyEvent::DamageApplied {
            target_entity_index,
            attacker_entity_index,
            ..
        } = event
            && *target_entity_index == before.state.entity_index
        {
            let actor = *attacker_entity_index;
            eprintln!(
                "staged Bee damage actor={} gen={}->{} before_root={:?} after_root={:?} before_flyers={:?} after_flyers={:?} before_impacts={:?} after_impacts={:?} before_imported={:?} after_imported={:?}",
                actor,
                before.economy().slot_generation(actor),
                after.economy().slot_generation(actor),
                before
                    .economy()
                    .authored_class5()
                    .iter()
                    .filter(|row| row.source_entity_index == actor)
                    .collect::<Vec<_>>(),
                after
                    .economy()
                    .authored_class5()
                    .iter()
                    .filter(|row| row.source_entity_index == actor)
                    .collect::<Vec<_>>(),
                before.economy().fireball_projectiles(),
                after.economy().fireball_projectiles(),
                before.economy().model17_spawned_children(),
                after.economy().model17_spawned_children(),
                before.entities().get(usize::from(actor)),
                after.entities().get(usize::from(actor))
            );
        }
    }
    eprintln!(
        "staged Bee exact frontier tick={} life={}->{} raw={:?}->{:?} new_events={:?} motion={:?}",
        session.identity().tick,
        before.shared_spells().player_entity().life,
        after.shared_spells().player_entity().life,
        raw_mailbox(&before),
        raw_mailbox(after),
        after
            .economy()
            .events()
            .get(before.economy().events().len()..),
        motion
    );
}

#[test]
fn public_fireballs_retire_real_level_three_final_bees_from_staged_group() {
    let mut session = staged_group_boundary(3);
    let shared = session.backend().campaign.simulation().shared_spells();
    eprintln!(
        "Bee incoming mana={}, speed holder={:?}",
        shared.player_mana(),
        shared
            .holders()
            .iter()
            .chain(shared.pickups())
            .chain(shared.dynamic_pickups())
            .find(|holder| holder.index == shared.local_player().enabled_spell_holders[3])
            .map(|holder| (
                holder.index,
                holder.max_mana,
                holder.subspell_index,
                holder.activation_value
            ))
    );
    assert_eq!(
        targets(&session, CombatFamily::Bee)
            .iter()
            .map(|row| row.row)
            .collect::<Vec<_>>(),
        FINAL_BEE_ROWS
    );
    let initial = targets(&session, CombatFamily::Bee);
    let receipts = retire_staged_bees_with_optional_trace(&mut session);
    assert!(
        receipts.len() >= 78,
        "six real3000-life Bees require thirteen genuine250 pulses each"
    );
    for row in initial {
        assert!(
            receipts
                .iter()
                .any(|receipt| receipt.authored_index == row.row
                    && receipt.source_slot == row.slot
                    && receipt.generation == row.generation
                    && receipt.life_after < 0),
            "each exact initial Bee occurrence requires a public lethal pulse"
        );
    }
    assert!(
        targets(&session, CombatFamily::Bee).is_empty(),
        "all six source Bee occurrences reach following-pass reclaim"
    );
    assert!(
        session
            .backend()
            .campaign
            .simulation()
            .canonical_player_life_for_test()
            > 0
    );
    assert!(
        receipts
            .iter()
            .all(|row| row.life_before - row.life_after == 250)
    );
    assert_eq!(
        session.backend().campaign.simulation().runtime().objectives[0].stage_selectors[4],
        2
    );
}

#[test]
fn public_fireball_reaches_one_real_level_three_bee_from_staged_cast_boundary() {
    let mut session = staged_group_boundary(3);
    let receipts = fire_public_combat_for_test(&mut session, CombatFamily::Bee);
    assert!(
        !receipts.is_empty(),
        "real public Fireball must reach one generation-qualified Bee"
    );
}
