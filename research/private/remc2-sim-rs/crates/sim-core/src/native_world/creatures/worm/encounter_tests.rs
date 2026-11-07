//! Ordinary unfiltered owner-package encounter; configured player entry only.

use crate::campaign::source_arena::{CreatureRootOrigin, CreatureRuntimeRole, SourceEntityHandle};
use crate::campaign::{CampaignSession, spec::CampaignSelection};
use crate::level_definition::{CreatureKind, METEOR_PROPERTIES};
use crate::native_world::{
    EntitySnapshot, FireballProjectileOrigin, Level2EconomyEvent, Mc2DispatchEvent, Mc2Simulation,
    area_damage, source_target,
};
use crate::{FlightWorld, PlayerCommand, Position};

const GROUP4: &[u16] = &[21, 132, 137, 138, 139, 140, 141, 233, 304, 305, 307, 308];

fn no_errors(session: &Mc2Simulation, phase: &str, turn: usize) {
    assert!(
        session
            .dispatch_events()
            .iter()
            .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
        "{phase} turn{turn}: {:?}",
        session.dispatch_events()
    );
}

fn packet(session: &Mc2Simulation, handle: SourceEntityHandle) -> Option<EntitySnapshot> {
    if session.source_arena().handle(handle.source_slot) != Some(handle) {
        return None;
    }
    if let Some(root) =
        super::dynamic::current_root_packet(session.source_arena(), handle.source_slot)
    {
        return Some(root);
    }
    let live = &session.session.state.live;
    source_target::status_parent(
        &live.economy,
        source_target::Context {
            spells: Some(&live.shared_spells),
            particles: Some(&session.session.definition.presentation.particles),
            ..source_target::Context::from_ordinary(&live.goats, &live.ordinary)
        },
        handle.source_slot,
    )
    .unwrap()
}

fn word(row: &EntitySnapshot, at: usize) -> u16 {
    u16::from_le_bytes([row.record[at], row.record[at + 1]])
}

#[test]
fn level1_actual_switch_worm_body_hit_drives_root_credit_cleanup_and_absence() {
    let package = std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq8-level1-collision-v5-night")
        },
        std::path::PathBuf::from,
    );
    let campaign = CampaignSession::open(&package, CampaignSelection::LEVEL1)
        .expect("actual unfiltered Night1 owner package");
    let mut session = campaign.simulation().clone();
    let player = session.state.entity_index;
    let player_handle = session.source_arena().handle(player).unwrap();
    let switch = session.source_arena().handle(112).unwrap();
    assert_eq!(
        (
            session.entities()[112].class,
            session.entities()[112].model,
            session.entities()[112].action,
            session.entities()[112].source_id
        ),
        (11, 4, 4, 4)
    );
    assert!(
        !session
            .creature_runtime_census()
            .roots_of_kind(CreatureKind::Worm)
            .any(|row| row.living)
    );

    // Declared player entry: prior carried MeteorIII/XP and sufficient paid
    // Castle/player resources. All NPC state and the full package are retained.
    session.apply_carried_spell_families(&[9], Some(9)).unwrap();
    session
        .shared_spells_mut()
        .award_spell_experience(player, 9, 5000);
    for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
        session
            .economy_mut()
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
            !session.economy().castle_projectile_site_blocked(
                *position,
                session.entities(),
                session.canonical_terrain(),
            )
        })
        .expect("ordinary source Castle site");
    let castle = session.economy_mut().build_castle(site, player).unwrap();
    assert!(
        session
            .economy_mut()
            .set_castle_mana_prerequisite_for_test(castle, 200_000)
    );
    {
        let live = &mut session.session.state.live;
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
    session.state.actual_speed = 0;
    session.state.target_speed = 0;
    // Age the real source startup protection before admitting the encounter.
    // No protection counter, NPC timer or RNG state is edited.
    for turn in 0..256 {
        if session.session.state.live.player_protection_ticks == 0 {
            break;
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        no_errors(&session, "player protection", turn);
        assert!(session.source_arena().resolve(switch).is_some());
        assert!(
            !session
                .creature_runtime_census()
                .roots_of_kind(CreatureKind::Worm)
                .any(|row| row.living)
        );
    }
    assert_eq!(session.session.state.live.player_protection_ticks, 0);
    let ui = crate::SOURCE_SPELL_INDEX_MAP
        .iter()
        .position(|family| *family == 9)
        .unwrap() as i8;
    session.step_outer_turn(
        PlayerCommand {
            opcode: 31,
            argument_1: ui,
            argument_2: 2,
            ..Default::default()
        },
        0,
    );
    no_errors(&session, "real MeteorIII selection", 0);
    let holder = session.shared_spells().local_player().enabled_spell_holders[9];
    let holder_handle = session.source_arena().handle(holder).unwrap();
    assert_eq!(
        session.shared_spells().pickup(holder).unwrap().level_gate,
        2
    );
    assert!(!session.shared_spells().free_spell_usage);

    // Declared prior objective completion, through the existing runtime owner.
    // Only actual scheduled switch112 may publish group4; no direct group call.
    let color = usize::from(
        session
            .players()
            .iter()
            .find(|row| row.entity_index == player)
            .unwrap()
            .player_color_index(),
    );
    session
        .session
        .state
        .live
        .core
        .runtime
        .set_level2_objective_level_end_for(color, 1);
    let mut published = false;
    for turn in 0..16 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        no_errors(&session, "switch112", turn);
        if session.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 112, group: 4, authored_indices, .. }
                if authored_indices.as_ref() == GROUP4))
        { published = true; break; }
    }
    assert!(
        published,
        "real switch112 must publish the entire authored group4"
    );
    let root = session
        .creature_runtime_census()
        .roots_of_kind(CreatureKind::Worm)
        .find(|row| {
            row.origin
                == (CreatureRootOrigin::AuthoredRecord {
                    authored_record: 307,
                })
        })
        .expect("actual authored307 semantic root")
        .handle;
    let original = packet(&session, root).unwrap();
    assert_eq!(
        (
            original.class,
            original.model,
            original.life,
            original.max_life
        ),
        (5, 3, 9000, 9000)
    );
    let mut parts: Vec<(SourceEntityHandle, Position)> = Vec::new();
    let mut next = word(&original, 56);
    for index in 0..16 {
        let row = session
            .economy()
            .authored_class5_model3_children()
            .iter()
            .find(|row| row.active && row.source_entity_index == next)
            .copied()
            .unwrap();
        assert_eq!(
            (
                row.root_authored_index,
                row.parent_entity_index,
                row.particle_index
            ),
            (307, root.source_slot, 89 + index as u16)
        );
        assert_eq!(
            row.word_0x32,
            if index == 0 {
                root.source_slot
            } else {
                parts[index - 1].0.source_slot
            }
        );
        let handle = session.source_arena().handle(next).unwrap();
        parts.push((handle, row.position));
        next = row.word_0x34;
    }
    assert_eq!(next, 0);
    assert_eq!(
        session
            .creature_runtime_census()
            .roots_of_kind(CreatureKind::Worm)
            .filter(|row| row.living)
            .count(),
        1,
        "the sixteen body carriers are not extra living roots"
    );
    for &(handle, _) in &parts {
        let identity = session
            .creature_identity_for_source_slot(handle.source_slot)
            .unwrap();
        assert_eq!(
            (identity.handle, identity.root_slot, identity.role),
            (handle, root.source_slot, CreatureRuntimeRole::MultipartPart)
        );
    }
    let views = session.campaign_entity_presentations(0).unwrap();
    assert!(
        views
            .iter()
            .any(|view| view.entity_index == root.source_slot && view.render_parameter_index == 88)
    );
    assert!(parts.iter().enumerate().all(|(index, (part, _))| {
        views.iter().any(|view| {
            view.entity_index == part.source_slot
                && view.render_parameter_index == 89 + index as u16
        })
    }));

    // One fixed lure. The C4 initial-yaw line was outside the strict170 cone
    // after source movement at phase30. A fixed -64 heading offset accounts
    // for that motion; source movement, scan and acquisition remain unchanged.
    let mut lure = original.position;
    crate::move_entity(&mut lure, original.yaw.wrapping_sub(64) & 2047, 0, 2304);
    lure.z = session
        .canonical_terrain()
        .ground_height(lure)
        .wrapping_add(256);
    session.set_level2_entity_position(player, lure).unwrap();
    session.state.actual_speed = 0;
    session.state.target_speed = 0;
    let mut shot = None;
    let mut audible = false;
    let mut moved = false;
    let mut parts_moved = Vec::new();
    let mut observations = Vec::new();
    let mut postcallback_scan_geometry = Vec::new();
    for turn in 0..64 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        no_errors(&session, "ordinary Worm attack", turn);
        let row = packet(&session, root)
            .expect("original307 remains its current generation before casting");
        assert_eq!(
            row.life, 9000,
            "ordinary source terrain/combat must preserve the declared unmodified Worm before the public shot"
        );
        moved |= row.position != original.position;
        for &(handle, start) in &parts {
            if !parts_moved.contains(&handle)
                && packet(&session, handle).is_some_and(|part| part.position != start)
            {
                parts_moved.push(handle);
            }
        }
        if observations.len() < 16 && (turn < 4 || turn % 8 == 0) {
            observations.push((
                turn,
                row.action,
                row.phase,
                row.position,
                row.yaw,
                word(&row, 154),
            ));
        }
        if matches!(row.phase, 31 | 61) {
            // This is a postcallback observation, not an actual frozen-list
            // scan receipt. It diagnoses the unchanged fixed lure only.
            let player_row = &session.entities()[usize::from(player)];
            let heading = crate::heading_for_positions(row.position, player_row.position);
            postcallback_scan_geometry.push((
                turn, row.position, row.yaw, row.record[61],
                player_row.position, player_row.control_flags, heading,
                crate::angle_distance(i32::from(row.yaw), heading),
            ));
        }
        shot = session
            .economy()
            .fireball_projectiles()
            .iter()
            .find(|projectile| {
                projectile.active
                    && projectile.origin == FireballProjectileOrigin::Worm
                    && projectile.owner_entity_index == original.source_id
                    && projectile.target_source_entity_index == player
                    && projectile.payload == 500
            })
            .and_then(|projectile| {
                session
                    .source_arena()
                    .handle(projectile.source_entity_index)
            });
        audible |= session.effect_journal().records().iter().any(|record|
            matches!(record.effect, crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                audio_site: crate::MissionAudioSite::MultipartAttack, entity_index, sound_id: 8, player_index: -1,
            }) if entity_index == root.source_slot)
                && record.audio.is_some_and(|audio| audio.command.audible && audio.play_type == 3));
        if shot.is_some() && audible && moved && parts_moved.len() == 16 {
            break;
        }
    }
    assert!(
        shot.is_some() && audible && moved && parts_moved.len() == 16,
        "ordinary original307 and all16 original body generations must move and emit the real9/0 shot: root={root:?} parts_moved={parts_moved:?} shot={shot:?} lure={lure:?} observations={observations:?} postcallback_scan_geometry={postcallback_scan_geometry:?}"
    );

    // ONE declared player-only collision placement over original body15.
    // Its current centre and the actual human FOV put the second384-unit
    // Meteor step at the thin live body, with the left-hand256 XY cancelled.
    let aimed_part = parts[15].0;
    let body = session
        .economy()
        .authored_class5_model3_children()
        .iter()
        .find(|row| row.active && row.source_entity_index == aimed_part.source_slot)
        .copied()
        .unwrap();
    assert_eq!(
        session.source_arena().handle(aimed_part.source_slot),
        Some(aimed_part)
    );
    assert_eq!(body.life, 9000);
    let settled_root = packet(&session, root).unwrap();
    let settled_parts = parts.iter().map(|&(handle, _)| {
        let row = packet(&session, handle).unwrap();
        (handle, row.position, row.array_pitch(), row.array_fov())
    }).collect::<Vec<_>>();
    let dx = i32::from(body.position.x.wrapping_sub(settled_root.position.x) as i16).abs();
    let dy = i32::from(body.position.y.wrapping_sub(settled_root.position.y) as i16).abs();
    // As in the real imported encounter, the planned second384-wide pulse
    // must exclude the current root. The actual body consumption and strict
    // lower-life inheritance remain independently required below.
    assert!(settled_root.array_pitch() > 0 && settled_root.array_roll() > 0);
    assert!(dx > i32::from(settled_root.array_pitch()) + 384
        || dy > i32::from(settled_root.array_roll()) + 384,
        "body15 setup must separate the planned second pulse from the root: root={settled_root:?} parts={settled_parts:?}");
    let firing_aim = (original.yaw, 512_i16);
    let player_fov = session.entities()[usize::from(player)].array_fov();
    let mut firing_pose = body.position;
    crate::move_entity(
        &mut firing_pose,
        firing_aim.0.wrapping_add(512) & 2047,
        0,
        256,
    );
    firing_pose.z = body
        .position
        .z
        .wrapping_add(body.render_yaw)
        .wrapping_add(2 * METEOR_PROPERTIES.projectile_speed)
        .wrapping_sub(player_fov);
    assert!(firing_pose.z > session.canonical_terrain().ground_height(firing_pose));
    session
        .set_level2_entity_position(player, firing_pose)
        .unwrap();
    session.state.actual_speed = 0;
    session.state.target_speed = 0;
    session.set_level2_player_aim_for_test(firing_aim.0, firing_aim.1);
    let mut no_cast = session.clone();
    let credit_before = session.economy().authored_bee_kill_census(player);
    let mut watched = vec![root];
    watched.extend(parts.iter().map(|part| part.0));
    area_damage::start_actor_damage_trace(&watched);
    area_damage::start_effect_damage_trace_for_targets(
        &watched
            .iter()
            .map(|handle| handle.source_slot)
            .collect::<Vec<_>>(),
    );
    super::mc2::start_inheritance_trace(root);
    let mut launches = Vec::new();
    let mut impacts = Vec::new();
    let mut consumptions = Vec::new();
    let mut body_consumptions: Vec<(SourceEntityHandle, i32, area_damage::PlayerMailboxWrite)> =
        Vec::new();
    let mut root_dead_turn = None;
    let mut absence_turn = None;
    let mut root_hidden = false;
    let mut parts_hidden: Vec<SourceEntityHandle> = Vec::new();
    let mut root_dto_lost = false;
    let mut mana_dto = false;
    let mut burst_dto = false;
    let mut paid = 0_i32;
    let mut turns = 0;
    let mut history = Vec::new();
    let mut projectile_history = Vec::new();
    let mut projectile_hidden = false;
    let mut spent_turn = None;
    for turn in 0..160 {
        let root_phase_before = packet(&session, root).map(|row| row.phase);
        let pending_bodies = parts
            .iter()
            .filter_map(|&(handle, _)| {
                if session.source_arena().handle(handle.source_slot) != Some(handle) {
                    return None;
                }
                let body = session
                    .economy()
                    .authored_class5_model3_children()
                    .iter()
                    .find(|body| body.active && body.source_entity_index == handle.source_slot)?;
                let mailbox =
                    area_damage::source_bounty(session.source_arena(), None, handle.source_slot)
                        .unwrap();
                if body.life < 0 || mailbox.0 <= 0 || mailbox.1 != player {
                    return None;
                }
                let writer = area_damage::current_actor_mailbox_writer(handle, mailbox)?;
                Some((handle, body.life, mailbox, writer))
            })
            .collect::<Vec<_>>();
        let mana_before = session.shared_spells().player_entity().mana;
        let event_start = session.economy().events().len();
        session.start_creature17_trace();
        session.step_outer_turn(
            PlayerCommand {
                flags: if turn == 0 { 0x10 } else { 0 },
                ..Default::default()
            },
            0,
        );
        turns += 1;
        no_errors(&session, "one fixed public Meteor", turn);
        for (handle, before, mailbox, writer) in pending_bodies {
            if session.source_arena().handle(handle.source_slot) == Some(handle)
                && let Some(body) = session
                    .economy()
                    .authored_class5_model3_children()
                    .iter()
                    .find(|body| body.active && body.source_entity_index == handle.source_slot)
                && body.life == before.wrapping_sub(mailbox.0)
                && body.word_0x26 == player
            {
                body_consumptions.push((handle, body.life, writer));
            }
        }
        paid = paid.saturating_add(
            mana_before
                .saturating_sub(session.shared_spells().player_entity().mana)
                .max(0),
        );
        let trace = session.take_creature17_trace();
        launches.extend(
            trace
                .meteor_launches
                .into_iter()
                .filter(|launch| launch.holder == holder_handle && launch.caster == player_handle),
        );
        impacts.extend(trace.meteor_impacts);
        for launch in &launches {
            if session.source_arena().handle(launch.projectile.source_slot)
                == Some(launch.projectile)
            {
                let row = &session.entities()[usize::from(launch.projectile.source_slot)];
                projectile_hidden |= row.drawing_disabled();
                if projectile_history.len() < 32 {
                    projectile_history.push((
                        turn,
                        launch.projectile,
                        row.position,
                        row.yaw,
                        row.pitch,
                        row.life,
                        row.drawing_disabled(),
                        word(row, 154),
                    ));
                }
            }
        }
        consumptions.extend(area_damage::take_player_damage_consumptions());
        let current = packet(&session, root);
        if let Some(row) = &current {
            if row.life < 0 && root_dead_turn.is_none() {
                root_dead_turn = Some(turn);
            }
            if row.drawing_disabled() {
                root_hidden = true;
            }
            if turn < 12 || turn % 8 == 0 {
                history.push((
                    turn,
                    row.action,
                    row.phase,
                    row.life,
                    row.position,
                    word(row, 40),
                    word(row, 102),
                ));
            }
        }
        let spent = matches!(launches.as_slice(), [launch]
            if projectile_hidden && session.source_arena().resolve(launch.projectile).is_none())
            && impacts.is_empty()
            && current
                .as_ref()
                .is_some_and(|row| row.life >= 0 && word(row, 102) == 0);
        if spent {
            if let Some(first) = spent_turn {
                if turn > first
                    && current.as_ref().is_some_and(|row| {
                        root_phase_before.is_some_and(|phase| row.phase == phase.wrapping_add(1))
                    })
                {
                    panic!(
                        "only public Meteor naturally spent without impact after following root visit: root={root:?} pose={firing_pose:?} body={body:?} projectile_history={projectile_history:?} history={history:?}"
                    );
                }
            } else {
                spent_turn = Some(turn);
            }
        } else {
            spent_turn = None;
        }
        let views = session.campaign_entity_presentations(0).unwrap();
        if root_hidden && session.source_arena().handle(root.source_slot) == Some(root) {
            root_dto_lost |= !views
                .iter()
                .any(|view| view.entity_index == root.source_slot);
        }
        for &(part, _) in &parts {
            if packet(&session, part).is_some_and(|row| row.drawing_disabled()) {
                if !parts_hidden.contains(&part) {
                    parts_hidden.push(part);
                }
                assert!(
                    !views
                        .iter()
                        .any(|view| view.entity_index == part.source_slot)
                );
            }
        }
        for event in &session.economy().events()[event_start..] {
            if let Level2EconomyEvent::ManaSphereSpawned {
                sphere_id,
                source_entity_index,
                mana,
            } = event
                && *source_entity_index == root.source_slot
                && *mana > 0
            {
                mana_dto |= session.economy().spheres().iter().any(|sphere| {
                    sphere.id == *sphere_id
                        && sphere.active
                        && sphere.mana >= *mana
                        && session
                            .source_arena()
                            .handle(sphere.source_entity_index)
                            .is_some()
                        && views
                            .iter()
                            .any(|view| view.entity_index == sphere.source_entity_index)
                });
            }
        }
        for (&slot, &(generation, parent)) in session.source_arena().effects.death_burst_origins() {
            if parent == root.source_slot
                && session.source_arena().slot_generation(slot) == generation
            {
                burst_dto |= views.iter().any(|view| view.entity_index == slot);
            }
        }
        if session.dispatch_events().iter().any(|event| {
            matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { group: 34, authored_indices, .. }
                if authored_indices.as_ref() == [309, 310, 311])
        }) {
            assert!(
                root_dead_turn.is_some_and(|dead| turn > dead),
                "the real absence producer follows root death"
            );
            absence_turn = Some(turn);
        }
        if absence_turn.is_some()
            && root_hidden
            && root_dto_lost
            && mana_dto
            && burst_dto
            && parts_hidden.len() == 16
            && watched
                .iter()
                .all(|handle| session.source_arena().resolve(*handle).is_none())
        {
            break;
        }
    }
    let effects = area_damage::take_effect_damage_trace();
    let inheritance = super::mc2::take_inheritance_trace();
    for turn in 0..turns {
        no_cast.step_outer_turn(PlayerCommand::default(), 0);
        no_errors(&no_cast, "equal elapsed no-cast", turn);
    }
    let belongs_to_cast = |receipt: &area_damage::EffectDamageReceipt| {
        receipt.source_scoped
            && receipt.pulse.owner == player
            && receipt.pulse.payload == 1600
            && receipt.writer_sequence.is_some()
            && impacts.iter().any(|impact| {
                receipt.source_slot == Some(impact.impact.source_slot)
                    && (
                        impact.state.source_class,
                        impact.state.model,
                        impact.state.subspell_index,
                    ) == (10, 17, 16000)
                    && launches.iter().any(|launch| {
                        launch.projectile == impact.projectile
                            && launch.holder == holder_handle
                            && launch.caster == player_handle
                            && (
                                launch.packet.class,
                                launch.packet.model,
                                launch.packet.subspell_index,
                            ) == (9, 3, 16000)
                    })
            })
    };
    let consumed_body = |handle: SourceEntityHandle, life: Option<i32>| {
        body_consumptions.iter().any(|(part, after, writer)| {
            *part == handle
                && life.is_none_or(|life| life == *after)
                && writer.target == handle
                && writer.mailbox.1 == player
                && impacts.iter().any(|impact| {
                    writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
                })
                && effects.iter().any(|receipt| {
                    belongs_to_cast(receipt)
                        && receipt.target_slot == handle.source_slot
                        && receipt.target_generation == handle.generation
                        && receipt.writer_sequence == Some(writer.sequence)
                })
        })
    };
    let body_hit = effects.iter().any(|receipt| {
        belongs_to_cast(receipt)
            && receipt.target_slot == aimed_part.source_slot
            && receipt.target_generation == aimed_part.generation
            && consumed_body(aimed_part, None)
            && receipt
                .mailbox_after
                .is_some_and(|mailbox| mailbox.0 > 0 && mailbox.1 == player)
    });
    let inherited = inheritance.iter().any(|visit| {
        visit.root == root
            && visit.attacker == player
            && visit.life_before > visit.life_after
            && parts.iter().any(|part| part.0 == visit.child)
            && consumed_body(visit.child, Some(visit.life_after))
            && effects.iter().any(|receipt| {
                belongs_to_cast(receipt)
                    && receipt.target_slot == visit.child.source_slot
                    && receipt.target_generation == visit.child.generation
            })
    });
    let inherited_fatal = inheritance.iter().any(|visit| {
        visit.root == root
            && visit.attacker == player
            && visit.life_before >= 0
            && visit.life_after < 0
            && consumed_body(visit.child, Some(visit.life_after))
            && effects.iter().any(|receipt| {
                belongs_to_cast(receipt)
                    && receipt.target_slot == visit.child.source_slot
                    && receipt.target_generation == visit.child.generation
            })
    });
    let own_fatal = consumptions.iter().any(|visit| {
        visit.target == root
            && visit.life_before >= 0
            && visit.life_after == visit.life_before.wrapping_sub(visit.mailbox.0)
            && visit.life_after < 0
            && visit.mailbox.1 == player
            && visit.writer.is_some_and(|writer| {
                writer.target == root
                    && writer.mailbox == visit.mailbox
                    && impacts.iter().any(|impact| {
                        writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
                    })
                    && effects.iter().any(|receipt| {
                        belongs_to_cast(receipt)
                            && receipt.target_slot == root.source_slot
                            && receipt.target_generation == root.generation
                            && receipt.writer_sequence == Some(writer.sequence)
                    })
            })
    });
    let mut effect_diagnostics = Vec::new();
    let mut body_consumption_diagnostics = Vec::new();
    let mut same_cast_receipt_count = 0;
    if !(body_hit && inherited && (inherited_fatal || own_fatal)) {
        let matching = effects.iter().filter_map(|receipt| {
            let impact = impacts.iter().find(|impact|
                receipt.source_slot == Some(impact.impact.source_slot))?;
            let target = SourceEntityHandle {
                source_slot: receipt.target_slot,
                generation: receipt.target_generation,
            };
            watched.contains(&target).then_some((receipt, impact.impact, target))
        }).collect::<Vec<_>>();
        same_cast_receipt_count = matching.len();
        effect_diagnostics.extend(matching.iter().enumerate()
            .filter(|(index, _)| *index < 32 || *index >= matching.len().saturating_sub(32))
            .map(|(_, (receipt, impact_birth, target))| (
                receipt.source_scoped, *impact_birth, *target,
                receipt.writer_sequence, receipt.mailbox_before,
                receipt.mailbox_after, receipt.pulse.payload,
            )));
        body_consumption_diagnostics.extend(body_consumptions.iter().enumerate()
            .filter(|(index, _)| *index < 32 || *index >= body_consumptions.len().saturating_sub(32))
            .map(|(_, consumption)| *consumption));
    }
    assert!(
        body_hit && inherited && (inherited_fatal || own_fatal),
        "fixed paid Meteor must hit body15 and own root damage/fatal: root={root:?} aimed={aimed_part:?} body_hit={body_hit} inherited={inherited} own_fatal={own_fatal} inherited_fatal={inherited_fatal} pose={firing_pose:?} aim={firing_aim:?} settled_root={settled_root:?} settled_parts={settled_parts:?} launches={launches:?} impacts={impacts:?} inheritance={inheritance:?} consumed={consumptions:?} same_cast_receipt_count={same_cast_receipt_count} first_last_effect_receipts={effect_diagnostics:?} body_consumption_count={} first_last_body_consumptions={body_consumption_diagnostics:?} history={history:?}",
        body_consumptions.len(),
    );
    assert_eq!(launches.len(), 1, "one predetermined public activation");
    assert!(paid >= METEOR_PROPERTIES.tiers[2].mana_cost);
    assert_eq!(
        session.economy().authored_bee_kill_census(player),
        credit_before.wrapping_add(1),
        "one root credit, no sixteen body credits"
    );
    assert!(
        root_hidden
            && root_dto_lost
            && mana_dto
            && burst_dto
            && parts_hidden.len() == 16
            && absence_turn.is_some()
    );
    assert!(
        watched
            .iter()
            .all(|handle| session.source_arena().resolve(*handle).is_none()),
        "all original generations naturally reclaim"
    );
    assert!(
        no_cast
            .creature_identity_for_source_slot(root.source_slot)
            .is_some_and(|identity| identity.handle == root && identity.living),
        "equal elapsed no-cast retains original307"
    );
    assert_eq!(
        no_cast.economy().authored_bee_kill_census(player),
        credit_before
    );
    assert!(
        !no_cast
            .economy()
            .authored_class5()
            .iter()
            .any(|row| matches!(row.authored_index, 309..=311)),
        "the living original root still blocks the real group34 absence producer"
    );
}

#[test]
fn level3_imported_body_hit_drives_original_root_credit_and_cleanup() {
    const ROOTS: [u16; 5] = [133, 150, 167, 184, 201];
    let package = std::env::var_os("MC_CAMPAIGN_PACKAGE_LEVEL3_V1").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3")
        },
        std::path::PathBuf::from,
    );
    let campaign = CampaignSession::open(&package, CampaignSelection::LEVEL3)
        .expect("actual whole Day3 imported package");
    let mut session = campaign.simulation().clone();
    let player = session.state.entity_index;
    let player_handle = session.source_arena().handle(player).unwrap();
    let roots = ROOTS.map(|slot| session.source_arena().handle(slot).unwrap());
    let root = roots[4];
    for &handle in &roots {
        let row = packet(&session, handle).unwrap();
        assert_eq!(
            (row.class, row.model, row.action, row.life, row.max_life),
            (5, 3, 31, 9000, 9000)
        );
        assert_eq!((row.record[76], row.record[77]), (1, 4));
        let identity = session
            .creature_identity_for_source_slot(handle.source_slot)
            .unwrap();
        assert_eq!(
            (identity.handle, identity.origin),
            (handle, CreatureRootOrigin::InitialPackage)
        );
    }
    let original = packet(&session, root).unwrap();
    let mut parts: Vec<(SourceEntityHandle, Position)> = Vec::new();
    let mut next = word(&original, 56);
    for index in 0..16 {
        let handle = session.source_arena().handle(next).unwrap();
        let row = packet(&session, handle).unwrap();
        assert_eq!(
            (row.class, row.model, row.action, row.source_id),
            (5, 3, 232, root.source_slot)
        );
        assert_eq!(row.particle_parameter_index, Some(89 + index as u16));
        assert_eq!(
            word(&row, 54),
            if index == 0 {
                root.source_slot
            } else {
                parts[index - 1].0.source_slot
            }
        );
        let identity = session
            .creature_identity_for_source_slot(handle.source_slot)
            .unwrap();
        assert_eq!(
            (identity.handle, identity.root_slot, identity.role),
            (handle, root.source_slot, CreatureRuntimeRole::MultipartPart)
        );
        parts.push((handle, row.position));
        next = word(&row, 56);
    }
    assert_eq!(next, 0);
    assert_eq!(
        session
            .creature_runtime_census()
            .roots_of_kind(CreatureKind::Worm)
            .filter(|row| row.living)
            .count(),
        5
    );

    // Declared player preparation only: carried MeteorIII/XP and paid Castle
    // resources. The five imported roots, all eighty parts, StageVar and RNG
    // remain exactly those admitted by the whole owner package.
    session.apply_carried_spell_families(&[9], Some(9)).unwrap();
    session
        .shared_spells_mut()
        .award_spell_experience(player, 9, 5000);
    for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
        session
            .economy_mut()
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
            !session.economy().castle_projectile_site_blocked(
                *position,
                session.entities(),
                session.canonical_terrain(),
            )
        })
        .expect("ordinary source Castle site");
    let castle = session.economy_mut().build_castle(site, player).unwrap();
    assert!(
        session
            .economy_mut()
            .set_castle_mana_prerequisite_for_test(castle, 200_000)
    );
    {
        let live = &mut session.session.state.live;
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
    session.state.actual_speed = 0;
    session.state.target_speed = 0;
    let ui = crate::SOURCE_SPELL_INDEX_MAP
        .iter()
        .position(|family| *family == 9);
    let ui = ui.unwrap() as i8;
    session.step_outer_turn(
        PlayerCommand {
            opcode: 31,
            argument_1: ui,
            argument_2: 2,
            ..Default::default()
        },
        0,
    );
    no_errors(&session, "Day3 real MeteorIII selection", 0);
    let holder = session.shared_spells().local_player().enabled_spell_holders[9];
    let holder_handle = session.source_arena().handle(holder).unwrap();
    assert_eq!(
        session.shared_spells().pickup(holder).unwrap().level_gate,
        2
    );
    assert!(!session.shared_spells().free_spell_usage);

    // Ordinary Stage31/mode4 follows the captured relation93 for all64 fixed
    // neutral visits. The original16 spacings total1974; mere initial movement
    // left the constructor cluster inside the same Meteor pulse as the root.
    // No target, cadence or actor field is forced.
    let mut moved = Vec::new();
    let mut root_moved = false;
    for turn in 0..64 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        no_errors(&session, "Day3 natural imported movement", turn);
        let row = packet(&session, root).expect("original imported root survives natural movement");
        assert!(row.life >= 0);
        root_moved |= row.position != original.position;
        for &(handle, position) in &parts {
            let row =
                packet(&session, handle).expect("original imported part survives natural movement");
            if row.position != position && !moved.contains(&handle) {
                moved.push(handle);
            }
        }
    }
    assert!(
        root_moved && moved.len() == 16,
        "all original imported parts must move: {moved:?}"
    );
    let views = session.campaign_entity_presentations(0).unwrap();
    assert!(
        views
            .iter()
            .any(|view| view.entity_index == root.source_slot && view.render_parameter_index == 88)
    );
    assert!(parts.iter().enumerate().all(|(index, (handle, _))| {
        views.iter().any(|view| {
            view.entity_index == handle.source_slot
                && view.render_parameter_index == 89 + index as u16
        })
    }));

    // One fixed collision placement over the predetermined original body15.
    // Cancel the actual hand256 offset and let two ordinary384-unit downward
    // projectile steps reach its source collision centre; no later tracking.
    let aimed_part = parts[15].0;
    let body = packet(&session, aimed_part).unwrap();
    assert_eq!(body.life, 9000);
    let settled_root = packet(&session, root).unwrap();
    let settled_parts = parts.iter().map(|&(handle, _)| {
        let row = packet(&session, handle).unwrap();
        (handle, row.position, row.array_pitch(), row.array_fov())
    }).collect::<Vec<_>>();
    let dx = i32::from(body.position.x.wrapping_sub(settled_root.position.x) as i16).abs();
    let dy = i32::from(body.position.y.wrapping_sub(settled_root.position.y) as i16).abs();
    // Source10/17 widths begin0,384,768. Excluding the current root from a
    // second384-wide pulse at the planned tail centre gives the actual child
    // consumption a chance to precede a strictly-lower root scan. The actual
    // impact/writer/inheritance receipts below remain the causal proof.
    assert!(settled_root.array_pitch() > 0 && settled_root.array_roll() > 0);
    assert!(dx > i32::from(settled_root.array_pitch()) + 384
        || dy > i32::from(settled_root.array_roll()) + 384,
        "fixed64 body15 setup must separate the planned second pulse from the root: root={settled_root:?} parts={settled_parts:?}");
    let aim = (original.yaw, 512_i16);
    let mut firing_pose = body.position;
    crate::move_entity(&mut firing_pose, aim.0.wrapping_add(512) & 2047, 0, 256);
    firing_pose.z = body
        .position
        .z
        .wrapping_add(body.array_yaw())
        .wrapping_add(2 * METEOR_PROPERTIES.projectile_speed)
        .wrapping_sub(session.entities()[usize::from(player)].array_fov());
    assert!(firing_pose.z > session.canonical_terrain().ground_height(firing_pose));
    session
        .set_level2_entity_position(player, firing_pose)
        .unwrap();
    session.state.actual_speed = 0;
    session.state.target_speed = 0;
    session.set_level2_player_aim_for_test(aim.0, aim.1);
    let mut no_cast = session.clone();
    let credit_before = session.economy().authored_bee_kill_census(player);
    let mut watched = vec![root];
    watched.extend(parts.iter().map(|part| part.0));
    area_damage::start_actor_damage_trace(&watched);
    area_damage::start_effect_damage_trace_for_targets(
        &watched
            .iter()
            .map(|handle| handle.source_slot)
            .collect::<Vec<_>>(),
    );
    super::mc2::start_inheritance_trace(root);
    let mut launches = Vec::new();
    let mut impacts = Vec::new();
    let mut consumptions = Vec::new();
    let mut body_consumptions = Vec::new();
    let mut hidden = Vec::new();
    let mut mana_dto = false;
    let mut burst_dto = false;
    let mut paid = 0_i32;
    let mut turns = 0;
    let mut history = Vec::new();
    let mut projectile_history = Vec::new();
    let mut projectile_hidden = false;
    let mut spent_turn = None;
    for turn in 0..160 {
        let phase = packet(&session, root).map(|row| row.phase);
        let pending = parts
            .iter()
            .filter_map(|&(handle, _)| {
                let row = packet(&session, handle)?;
                let mailbox =
                    area_damage::source_bounty(session.source_arena(), None, handle.source_slot)
                        .unwrap();
                (row.life >= 0 && mailbox.0 > 0 && mailbox.1 == player)
                    .then(|| {
                        area_damage::current_actor_mailbox_writer(handle, mailbox)
                            .map(|writer| (handle, row.life, mailbox, writer))
                    })
                    .flatten()
            })
            .collect::<Vec<_>>();
        let mana_before = session.shared_spells().player_entity().mana;
        let event_start = session.economy().events().len();
        session.start_creature17_trace();
        session.step_outer_turn(
            PlayerCommand {
                flags: if turn == 0 { 0x10 } else { 0 },
                ..Default::default()
            },
            0,
        );
        no_errors(&session, "Day3 one fixed public Meteor", turn);
        turns += 1;
        for (handle, before, mailbox, writer) in pending {
            if packet(&session, handle).is_some_and(|row| {
                row.life == before.wrapping_sub(mailbox.0) && word(&row, 42) == player
            }) {
                body_consumptions.push((handle, before.wrapping_sub(mailbox.0), writer));
            }
        }
        paid = paid.saturating_add(
            mana_before
                .saturating_sub(session.shared_spells().player_entity().mana)
                .max(0),
        );
        let trace = session.take_creature17_trace();
        launches.extend(
            trace
                .meteor_launches
                .into_iter()
                .filter(|launch| launch.holder == holder_handle && launch.caster == player_handle),
        );
        impacts.extend(trace.meteor_impacts);
        consumptions.extend(area_damage::take_player_damage_consumptions());
        let current = packet(&session, root);
        if let Some(row) = &current
            && (turn < 12 || turn % 8 == 0)
        {
            history.push((
                turn,
                row.action,
                row.phase,
                row.life,
                row.position,
                word(row, 40),
                word(row, 102),
            ));
        }
        for launch in &launches {
            if session.source_arena().handle(launch.projectile.source_slot)
                == Some(launch.projectile)
            {
                let row = &session.entities()[usize::from(launch.projectile.source_slot)];
                projectile_hidden |= row.drawing_disabled();
                if projectile_history.len() < 32 {
                    projectile_history.push((
                        turn,
                        launch.projectile,
                        row.position,
                        row.life,
                        word(row, 154),
                    ));
                }
            }
        }
        let spent = matches!(launches.as_slice(), [launch] if projectile_hidden && session.source_arena().resolve(launch.projectile).is_none())
            && impacts.is_empty()
            && current
                .as_ref()
                .is_some_and(|row| row.life >= 0 && word(row, 102) == 0);
        if spent {
            if let Some(first) = spent_turn {
                if turn > first
                    && current.as_ref().is_some_and(|row| {
                        phase.is_some_and(|old| row.phase == old.wrapping_add(1))
                    })
                {
                    panic!(
                        "Day3 public shot naturally spent without impact after following callback: pose={firing_pose:?} body={body:?} projectile={projectile_history:?} root={history:?}"
                    );
                }
            } else {
                spent_turn = Some(turn);
            }
        } else {
            spent_turn = None;
        }
        let views = session.campaign_entity_presentations(0).unwrap();
        for &handle in &watched {
            if packet(&session, handle).is_some_and(|row| row.drawing_disabled()) {
                if !hidden.contains(&handle) {
                    hidden.push(handle);
                }
                assert!(
                    !views
                        .iter()
                        .any(|view| view.entity_index == handle.source_slot)
                );
            }
        }
        for event in &session.economy().events()[event_start..] {
            if let Level2EconomyEvent::ManaSphereSpawned {
                sphere_id,
                source_entity_index,
                mana,
            } = event
                && *source_entity_index == root.source_slot
                && *mana > 0
            {
                mana_dto |= session.economy().spheres().iter().any(|sphere| {
                    sphere.id == *sphere_id
                        && sphere.active
                        && sphere.mana >= *mana
                        && views
                            .iter()
                            .any(|view| view.entity_index == sphere.source_entity_index)
                });
            }
        }
        for (&slot, &(generation, parent)) in session.source_arena().effects.death_burst_origins() {
            if parent == root.source_slot
                && session.source_arena().slot_generation(slot) == generation
            {
                burst_dto |= views.iter().any(|view| view.entity_index == slot);
            }
        }
        assert!(
            !session.dispatch_events().iter().any(|event| matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated {
                    source_slot: 247,
                    group: 11,
                    ..
                }
            )),
            "four retained living imported roots must block the all-Worm absence producer"
        );
        if hidden.len() == 17
            && mana_dto
            && burst_dto
            && watched
                .iter()
                .all(|handle| session.source_arena().resolve(*handle).is_none())
        {
            break;
        }
    }
    let effects = area_damage::take_effect_damage_trace();
    let inheritance = super::mc2::take_inheritance_trace();
    for turn in 0..turns {
        no_cast.step_outer_turn(PlayerCommand::default(), 0);
        no_errors(&no_cast, "Day3 equal elapsed no-cast", turn);
    }
    let cast_writer = |receipt: &area_damage::EffectDamageReceipt| {
        receipt.source_scoped
            && receipt.pulse.owner == player
            && receipt.pulse.payload == 1600
            && receipt.writer_sequence.is_some()
            && impacts.iter().any(|impact| {
                receipt.source_slot == Some(impact.impact.source_slot)
                    && (
                        impact.state.source_class,
                        impact.state.model,
                        impact.state.subspell_index,
                    ) == (10, 17, 16000)
                    && launches.iter().any(|launch| {
                        launch.projectile == impact.projectile
                            && launch.holder == holder_handle
                            && launch.caster == player_handle
                            && (
                                launch.packet.class,
                                launch.packet.model,
                                launch.packet.subspell_index,
                            ) == (9, 3, 16000)
                    })
            })
    };
    let consumed_body = |handle: SourceEntityHandle, life: Option<i32>| {
        body_consumptions.iter().any(|(part, after, writer)| {
            *part == handle
                && life.is_none_or(|life| life == *after)
                && writer.target == handle
                && writer.mailbox.1 == player
                && impacts.iter().any(|impact| {
                    writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
                })
                && effects.iter().any(|receipt| {
                    cast_writer(receipt)
                        && receipt.target_slot == handle.source_slot
                        && receipt.target_generation == handle.generation
                        && receipt.writer_sequence == Some(writer.sequence)
                })
        })
    };
    let body_hit = consumed_body(aimed_part, None);
    let inherited = inheritance.iter().any(|visit| {
        visit.root == root
            && visit.attacker == player
            && visit.life_before > visit.life_after
            && parts.iter().any(|part| part.0 == visit.child)
            && consumed_body(visit.child, Some(visit.life_after))
    });
    let inherited_fatal = inheritance.iter().any(|visit| {
        visit.root == root
            && visit.attacker == player
            && visit.life_before >= 0
            && visit.life_after < 0
            && consumed_body(visit.child, Some(visit.life_after))
    });
    let own_fatal = consumptions.iter().any(|visit| {
        visit.target == root
            && visit.life_before >= 0
            && visit.life_after == visit.life_before.wrapping_sub(visit.mailbox.0)
            && visit.life_after < 0
            && visit.mailbox.1 == player
            && visit.writer.is_some_and(|writer| {
                writer.target == root
                    && writer.mailbox == visit.mailbox
                    && impacts.iter().any(|impact| {
                        writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
                    })
                    && effects.iter().any(|receipt| {
                        cast_writer(receipt)
                            && receipt.target_slot == root.source_slot
                            && receipt.target_generation == root.generation
                            && receipt.writer_sequence == Some(writer.sequence)
                    })
            })
    });
    assert!(
        body_hit && inherited && (inherited_fatal || own_fatal),
        "actual imported body writer must cause root damage/fatal: body_hit={body_hit} inherited={inherited} inherited_fatal={inherited_fatal} own_fatal={own_fatal} pose={firing_pose:?} settled_root={settled_root:?} settled_parts={settled_parts:?} launch={launches:?} impacts={impacts:?} inheritance={inheritance:?} consumptions={consumptions:?} history={history:?}"
    );
    assert_eq!(launches.len(), 1, "one fixed paid activation");
    assert!(paid >= METEOR_PROPERTIES.tiers[2].mana_cost);
    assert_eq!(
        session.economy().authored_bee_kill_census(player),
        credit_before.wrapping_add(1)
    );
    assert!(hidden.len() == 17 && mana_dto && burst_dto);
    assert!(
        watched
            .iter()
            .all(|handle| session.source_arena().resolve(*handle).is_none()),
        "all original root/body generations naturally reclaim"
    );
    assert!(
        no_cast
            .creature_identity_for_source_slot(root.source_slot)
            .is_some_and(|row| row.handle == root && row.living)
    );
    assert_eq!(
        no_cast.economy().authored_bee_kill_census(player),
        credit_before
    );
    for &handle in &roots[..4] {
        for current in [&session, &no_cast] {
            assert!(
                current
                    .creature_identity_for_source_slot(handle.source_slot)
                    .is_some_and(|row| row.handle == handle && row.living),
                "the other four unchanged imported generations remain living"
            );
        }
    }
    for current in [&session, &no_cast] {
        let countdown = &current.entities()[247];
        assert!(countdown.active && !countdown.drawing_disabled());
        assert_eq!(
            i32::from_le_bytes(countdown.record[20..24].try_into().unwrap()),
            0,
            "the real absence countdown stays unarmed while original roots live"
        );
    }
}
