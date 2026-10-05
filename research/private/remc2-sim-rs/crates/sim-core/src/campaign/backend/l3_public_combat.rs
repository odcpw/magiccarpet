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
    let economy = session.backend().campaign.level2_compatibility().economy();
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
    let compatibility = session.backend().campaign.level2_compatibility();
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
        .level2_compatibility()
        .economy()
        .bee_creatures()
        .to_vec();
    let player = session
        .backend()
        .campaign
        .level2_compatibility()
        .shared_spells()
        .player_entity();
    assert!(player.life >= 0, "public combat player died: {player:?}");
    session.step(guard(session), command).unwrap();
    let economy = session.backend().campaign.level2_compatibility().economy();
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
    let state = session.backend().campaign.level2_compatibility().state;
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
    let shared = session
        .backend()
        .campaign
        .level2_compatibility()
        .shared_spells();
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
        let compatibility = session.backend().campaign.level2_compatibility();
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
        let compatibility = session.backend().campaign.level2_compatibility();
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
        let state = session.backend().campaign.level2_compatibility().state;
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
    let compatibility = session.backend().campaign.level2_compatibility();
    let state = forecast.level2_compatibility().state;
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
                .level2_compatibility()
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
            .level2_compatibility()
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
        let economy = session.backend().campaign.level2_compatibility().economy();
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
        session
            .backend()
            .campaign
            .level2_compatibility()
            .state
            .position
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
        let economy = session.backend().campaign.level2_compatibility().economy();
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
    let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
    compatibility
        .apply_carried_spell_families(&[0, 1, 2, 3], Some(0))
        .unwrap();
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
    let mut session = staged_group_boundary(5);
    let first = targets(&session, CombatFamily::Archer)[0];
    let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
    let position = crate::Position {
        x: first.position.x.wrapping_add(1000),
        ..first.position
    };
    compatibility
        .set_level2_entity_position(compatibility.state.entity_index, position)
        .unwrap();
    for _ in 0..180 {
        step(&mut session, GameCommand::default());
        let economy = session.backend().campaign.level2_compatibility().economy();
        if let Some(child) = economy
            .authored_class5_archer_children()
            .iter()
            .find(|child| child.parent_entity_index == first.slot)
        {
            assert_eq!((child.source_class, child.model), (9, 13));
            assert!(
                (0..=13).contains(&child.life),
                "the ordinary source pass can visit a newly allocated arrow"
            );
            let parent = economy
                .authored_class5_archer()
                .iter()
                .find(|parent| parent.source_entity_index == first.slot)
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

#[test]
fn ordinary_level_three_archer_arrow_damages_player_without_lowering_shooter_health() {
    let mut session = staged_group_boundary(5);
    let first = targets(&session, CombatFamily::Archer)[0];
    let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
    let player = compatibility.state.entity_index;
    let position = crate::Position {
        x: first.position.x.wrapping_add(1000),
        // A zero-width sub_10C80 pulse searches the rounded centre tile.
        // Avoid the authored row's exact half-tile boundary, where source
        // can collide but its subsequent damage scan misses the lower tile.
        y: first.position.y.wrapping_sub(64),
        ..first.position
    };
    let initial_life = compatibility.canonical_player_life_for_test();
    compatibility
        .set_level2_entity_position(player, position)
        .unwrap();
    let mut launched = BTreeSet::new();
    for _ in 0..180 {
        step(&mut session, GameCommand::default());
        let compatibility = session.backend().campaign.level2_compatibility();
        let economy = compatibility.economy();
        for child in economy.authored_class5_archer_children() {
            launched.insert(child.entity_id);
        }
        assert!(
            targets(&session, CombatFamily::Archer)
                .iter()
                .all(|root| root.life == 1000),
            "arrows must neither join the health chain nor damage class-5 peers when targeting the player"
        );
        if economy.events().iter().any(|event| matches!(event,
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
    panic!(
        "ordinary Archer launch never reached the player's damage consumer; shooters={launched:?}"
    );
}

#[test]
fn typed_archer_attacker_reaches_imported_skeleton_damage_reaction() {
    let mut session = staged_group_boundary(5);
    for _ in 0..24 {
        if session.backend().campaign.level2_compatibility().entities()[122].action == 79 {
            break;
        }
        step(&mut session, GameCommand::default());
    }
    let archer = targets(&session, CombatFamily::Archer)[0];
    let compatibility = session.backend_mut().campaign.level2_compatibility_mut();
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
    let after = &session.backend().campaign.level2_compatibility().entities()[122];
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

#[test]
fn public_fireballs_retire_real_level_three_final_bees_from_staged_group() {
    let mut session = staged_group_boundary(3);
    let shared = session
        .backend()
        .campaign
        .level2_compatibility()
        .shared_spells();
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
    let receipts = retire_public_combat_for_test(&mut session, CombatFamily::Bee);
    assert!(
        receipts
            .iter()
            .all(|row| row.life_before - row.life_after == 250)
    );
    assert_eq!(
        session
            .backend()
            .campaign
            .level2_compatibility()
            .runtime()
            .objectives[0]
            .stage_selectors[4],
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
