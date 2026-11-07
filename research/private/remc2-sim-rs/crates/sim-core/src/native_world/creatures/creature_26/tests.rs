use super::*;
use crate::campaign::source_arena::SourceArena;
use crate::native_world::{
    ParticleParameter, PlayerSnapshot, RuntimeRelation, RuntimeSnapshot, RuntimeStageVariable,
};

fn terrain() -> SessionTerrain {
    let session = crate::native_world::tests::level_two_campaign_session();
    session.canonical_terrain().clone()
}

fn full_level_two_session() -> crate::native_world::Mc2Simulation {
    let package = std::env::var_os("MC_SETTLEMENT_FULL_PACKAGE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-1pb-16-level2-full-build-v2")
        });
    crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::LEVEL2,
    )
    .expect("owner-exported Level 2 retains constructor counters")
    .simulation()
    .clone()
}

fn position(x: u16) -> Position {
    Position {
        x,
        y: 20_000,
        z: 5_000,
    }
}

fn actor_row(slot: u16, action: u8, position: Position) -> EntitySnapshot {
    let mut row = crate::native_world::cave_features::new_event(slot, 19, position);
    row.class = 5;
    row.model = SOURCE_MODEL;
    row.source_id = slot;
    row.set_source_action(action);
    row.set_control_flags(row.control_flags | 0x800);
    row
}

fn player_target(slot: u16, position: Position) -> Target {
    Target {
        handle: SourceEntityHandle {
            source_slot: slot,
            generation: 1,
        },
        slot,
        source_id: slot,
        class: 3,
        model: 0,
        action: 0,
        life: 500,
        flags: 0,
        position,
        yaw: 0,
        roll: 0,
        source_yaw: Some(0),
        source_roll: Some(0),
        relations: crate::native_world::source_target::Relations {
            target: Some(0), attacker: Some(0), parent: Some(0),
            stage_mode: Some(0), stage_index: Some(0), stage_relation: Some(0), next: Some(0),
        },
        player_entity: slot,
        mana: 500,
        mana_regen: 9,
        left_spell: None,
        right_spell: None,
        target: 0,
        next: 0,
        parent: 0,
        stage_index: Some(0),
        stage_relation: Some(0),
        stage_mode: 0,
        pitch_extent: Some(0),
        link: 0,
        attacker: 0,
        killer: 0,
        actual_speed: 0,
        maximum_speed: 0,
    }
}

fn theft_roll_seed(value: u32) -> u32 {
    (0_u32..63)
        .find(|seed| seed.wrapping_mul(9377).wrapping_add(9439) % 63 == value)
        .expect("the source LCG reaches each theft selector")
}

fn run_model26_action(
    row: &mut EntitySnapshot,
    terrain: &SessionTerrain,
    runtime: &RuntimeSnapshot,
    targets: &[Target],
    class3: &[u16],
    model26: &[u16],
) -> Effects {
    let mut global_rng = 0;
    let source_slot = row.index;
    let mut actor = Actor {
        row,
        handle: SourceEntityHandle {
            source_slot,
            generation: 1,
        },
    };
    let mut context = Context {
        rules: &RULES,
        terrain,
        targets,
        current_target: None,
        sentinel: None,
        class3,
        model26,
        families: None,
        runtime: Some(runtime),
        global_rng: &mut global_rng,
    };
    step(&mut actor, &mut context).unwrap()
}

fn equip_right_spell(
    player: &mut PlayerSnapshot,
    player_entity: u16,
    family: u8,
    holder_slot: u16,
) {
    if player.entity_index != player_entity {
        return;
    }
    let family_index = usize::from(family);
    assert!(player.set_enabled_spell_holder_index(family_index, holder_slot));
    player.left_spell_slot_raw = -1;
    player.left_spell_slot = 0;
    player.record[554..556].copy_from_slice(&(-1_i16).to_le_bytes());
    player.right_spell_slot_raw = i16::from(family);
    player.right_spell_slot = family;
    player.record[556..558].copy_from_slice(&i16::from(family).to_le_bytes());
}

#[test]
fn constructor_matches_source_orientation_and_particle_extents() {
    let particle = ParticleParameter {
        base_tmap: 0,
        word_2: 0,
        word_4: 0,
        speed: 90,
        rotation_speed: 52,
        byte_10: 0,
        byte_11: 0,
        render_mode: 0,
        byte_13: 0,
    };
    let particles = vec![particle; usize::from(PARTICLE) + 1];
    let seed = 0x1234_u16;
    let rng = seed.wrapping_mul(9377).wrapping_add(9439);
    let expected_rotation = ((rng & 0x07ff).wrapping_sub(1)) as i16;
    let simulation = full_level_two_session();
    let mut runtime = simulation.session.state.live.core.runtime.clone();
    runtime.bytes[crate::native_world::RUNTIME_BYTES + usize::from(SOURCE_MODEL)] = 17;

    let requested_position = position(20_000);
    let row = mc2::construct_row(7, seed, requested_position, &mut runtime, &particles).unwrap();

    assert_eq!(
        (row.class, row.model, row.action),
        (5, SOURCE_MODEL, ROAM_ACTION)
    );
    assert_eq!((row.max_life, row.life, row.mana), (4400, 4400, 2200));
    assert_eq!(
        (row.yaw, row.pitch, row.source_roll(), row.fov),
        (expected_rotation, expected_rotation, expected_rotation, 0)
    );
    assert_eq!(row.entity_rng, rng);
    assert_eq!(
        row.position, requested_position,
        "sub_4CF00 keeps the placed height"
    );
    assert_eq!(row.phase, 17);
    assert_eq!(
        runtime.bytes[crate::native_world::RUNTIME_BYTES + usize::from(SOURCE_MODEL)],
        18
    );
    assert_eq!(
        (row.particle_parameter_index, row.record[97]),
        (Some(PARTICLE), particle.animation_limit().unwrap())
    );
    assert_eq!(
        (
            row.array_yaw(),
            row.array_pitch(),
            row.array_roll(),
            row.array_fov()
        ),
        (26, 256, 256, 384)
    );
    assert_ne!(row.control_flags & 0x800000, 0);
}

#[test]
fn normal_source_prepass_reactivates_nearby_model26_but_not_distant_model26() {
    use crate::PlayerCommand;

    let mut near = full_level_two_session();
    let particles = near.session.definition.presentation.particles.to_vec();
    let player_slot = near.state.entity_index;
    let player_position = near.entities()[usize::from(player_slot)].position;
    let actor = {
        let live = &mut near.session.state.live;
        let slot = mc2::allocate_authored(
            &mut live.economy,
            &mut live.core.runtime,
            &mut live.terrain,
            &particles,
            player_position,
            RULES.max_life,
            None,
            500,
        )
        .unwrap()
        .expect("ordinary model26 source allocation");
        let row = &mut live.economy.entities[usize::from(slot)];
        row.record[61] = 0;
        row.record[62] = 0;
        row.set_control_flags(row.control_flags & !1);
        slot
    };
    let mut far = near.clone();
    let mut distant = player_position;
    distant.x = distant.x.wrapping_add(0x6000);
    far.set_level2_entity_position(player_slot, distant)
        .unwrap();

    near.step_outer_turn(PlayerCommand::default(), 0);
    far.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(near.entities()[usize::from(actor)].record[61], 16);
    assert_eq!(far.entities()[usize::from(actor)].record[61], 0);
}

#[test]
fn owner_level6_model26_drains_its_real_player_in_the_normal_source_pass() {
    use crate::PlayerCommand;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};

    let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-bhq6-ordinary-20261024-b/level6");
    let campaign = CampaignSession::open(package, CampaignSelection::ordinary(6).unwrap())
        .expect("ordinary owner Level6 package");
    let mut session = campaign.simulation().clone();
    let actor = session.entities()[73].clone();
    let player = session.entities()[77].clone();
    assert_eq!((actor.class, actor.model, actor.action), (5, 26, 210));
    assert_eq!((player.class, player.model), (3, 0));
    assert_eq!(
        u16::from_le_bytes([actor.record[154], actor.record[155]]),
        77
    );
    assert!(player.mana > 0);

    // The unchanged owner-data actor targets the player. In the near-identical
    // negative, only its target word is cleared before the same source pass.
    let mut without_target = session.clone();
    put(
        &mut without_target.session.state.live.economy.entities[73],
        154,
        0,
    );
    without_target.step_outer_turn(PlayerCommand::default(), 0);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.entities()[73].phase, actor.phase.wrapping_add(1));
    assert!(
        session.entities()[77].mana < without_target.entities()[77].mana,
        "scheduled source actor must drain the actual player carrier"
    );

    // Select the package's actual kind-7/model26 objective as a controlled
    // stimulus. No objective relation or creature is invented, and the next
    // location objective remains pending: this is not a mission playthrough.
    assert_eq!(session.runtime().stages[1].kind, 7);
    assert_eq!(
        session.runtime().stages[1].relation,
        RuntimeRelation::Scalar(26)
    );
    let runtime = &mut session.session.state.live.core.runtime;
    runtime.set_level2_objective_stage_selector_for(0, 0, 0);
    runtime.set_level2_objective_text_selector_for(0, 1);
    runtime.set_level2_objective_stage_selector_for(0, 1, 1);
    let roots = session
        .entities()
        .iter()
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL && row.life >= 0)
        .map(|row| (session.source_arena().handle(row.index).unwrap(), row.life))
        .collect::<Vec<_>>();
    assert!(!roots.is_empty());
    let credits_before = session.economy().authored_bee_kill_census(77);
    let mut survivor = session.clone();
    for (handle, life) in &roots {
        for (candidate, damage) in [(&mut session, *life + 1), (&mut survivor, *life - 1)] {
            let row =
                &mut candidate.session.state.live.economy.entities[usize::from(handle.source_slot)];
            // A real damage mailbox is the only death stimulus. Keep the
            // near-identical actors at life1 in the negative case.
            row.record[98..102].copy_from_slice(&damage.to_le_bytes());
            put(row, 102, 77);
        }
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    survivor.step_outer_turn(PlayerCommand::default(), 0);
    for candidate in [&session, &survivor] {
        assert!(
            !candidate
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "{:?}",
            candidate.dispatch_events()
        );
        assert_eq!(
            candidate.runtime().objectives[0].stage_selectors[1],
            1,
            "case7 must retain the pass-start living membership on the fatal callback pass"
        );
    }
    for (handle, _) in &roots {
        let row = &session.entities()[usize::from(handle.source_slot)];
        assert_eq!((row.life, row.action), (-1, PREKILL_ACTION));
        assert_eq!(
            session.source_arena().handle(handle.source_slot),
            Some(*handle)
        );
        assert!(!row.drawing_disabled());
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    survivor.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        session.runtime().objectives[0].stage_selectors[1],
        2,
        "case7 reads the following pass's frozen head, before corpse hiding"
    );
    assert_eq!(
        survivor.runtime().objectives[0].stage_selectors[1],
        1,
        "a surviving model26 must keep the real absence objective pending"
    );
    assert_eq!(
        session.economy().authored_bee_kill_census(77),
        credits_before + roots.len() as u32
    );
    for (handle, _) in &roots {
        let row = &session.entities()[usize::from(handle.source_slot)];
        assert_eq!(row.action, KILL_ACTION);
        assert!(!row.drawing_disabled());
    }
    // Observe each original generation hide at the real death cadence and
    // reclaim at the immediately following pass. Different roots may have
    // different phases, so track the boundary for each one separately.
    let mut hidden = vec![false; roots.len()];
    let mut reclaimed = vec![false; roots.len()];
    let mut mana_release = false;
    for _ in 0..10 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "{:?}",
            session.dispatch_events()
        );
        mana_release |= session.dispatch_events().iter().any(|event| {
            matches!(event, crate::native_world::Mc2DispatchEvent::Economy {
                event: crate::native_world::Level2EconomyEvent::ManaSphereSpawned {
                    source_entity_index, ..
                }, ..
            } if roots.iter().any(|(handle, _)| handle.source_slot == *source_entity_index))
        });
        for (index, (handle, _)) in roots.iter().enumerate() {
            if reclaimed[index] {
                continue;
            }
            let resolved = session.source_arena().resolve(*handle);
            if hidden[index] {
                assert!(
                    resolved.is_none(),
                    "the hidden original generation must be reclaimed on the next pass"
                );
                reclaimed[index] = true;
            } else {
                assert!(
                    resolved.is_some(),
                    "the original generation must remain live until hidden"
                );
                hidden[index] =
                    session.entities()[usize::from(handle.source_slot)].drawing_disabled();
            }
        }
        if reclaimed.iter().all(|done| *done) {
            break;
        }
    }
    assert!(
        mana_release,
        "a real model26 death must emit its mana sphere"
    );
    assert!(hidden.iter().all(|seen| *seen));
    assert!(reclaimed.iter().all(|done| *done));
}

#[test]
fn owner_level6_acquired_spell_runs_theft_detachment_and_pickup() {
    use crate::PlayerCommand;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};

    let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-bhq6-ordinary-20261024-b/level6");
    let campaign = CampaignSession::open(package, CampaignSelection::ordinary(6).unwrap())
        .expect("ordinary owner Level6 package");
    let mut session = campaign.simulation().clone();
    let (actor_slot, player_slot, holder_slot, family) = (73_u16, 77_u16, 1_u16, 3_u8);
    let pickup = session.entities()[usize::from(holder_slot)].clone();
    assert_eq!(
        (pickup.class, pickup.model, pickup.action),
        (15, family, 10)
    );
    assert_eq!(
        session.shared_spells().local_player().enabled_spell_holders[3],
        0
    );
    let holder_handle = session.source_arena().handle(holder_slot).unwrap();
    // Speed's initial overlap box is entirely covered by its real Army and
    // Shield neighbors. Isolate only the existing Army pickup's placement as
    // a check prerequisite; keep real Speed acquisition and Shield recovery.
    let army = session.entities()[7].clone();
    assert_eq!((army.class, army.model, army.action), (15, 19, 58));
    let army_handle = session.source_arena().handle(7).unwrap();
    session.set_level2_entity_position(7, Position {
        x: army.position.x.wrapping_sub(4_096),
        ..army.position
    }).unwrap();
    let isolated = &session.entities()[7];
    assert_eq!(session.source_arena().handle(7), Some(army_handle));
    assert_eq!((isolated.class, isolated.model, isolated.action, isolated.control_flags,
        isolated.life, isolated.max_life, isolated.phase, isolated.entity_rng),
        (army.class, army.model, army.action, army.control_flags,
            army.life, army.max_life, army.phase, army.entity_rng));
    let shield = session.entities()[2].clone();
    assert_eq!((shield.class, shield.model, shield.action), (15, 6, 19));
    let mut pickup_approach = pickup.position;
    crate::move_entity(&mut pickup_approach,
        heading_for_positions(shield.position, pickup.position) as i16, 0, 512);
    let mut player_at_pickup = session.entities()[usize::from(player_slot)].clone();
    player_at_pickup.set_position(pickup_approach);
    for ground in session.entities().iter().filter(|row| {
        row.active && row.class == 15 && !row.drawing_disabled()
            && [crate::spell_activation::PhysicalSpellActionRole::OrdinaryPickup,
                crate::spell_activation::PhysicalSpellActionRole::PersistentPickup]
                .into_iter().any(|role|
                    crate::spell_activation::physical_spell_action(row.model, role) == Some(row.action))
            && session.shared_spells().local_player().enabled_spell_holders
                .get(usize::from(row.model)) == Some(&0)
    }) {
        assert_eq!(crate::native_world::level2_source_intersects(ground, &player_at_pickup),
            ground.index == holder_slot,
            "the declared approach overlaps Speed only among all eligible real pickups: slot {} family {} position {:?}",
            ground.index, ground.model, ground.position);
    }
    session
        .set_level2_entity_position(player_slot, pickup_approach)
        .unwrap();
    for _ in 0..8 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "{:?}",
            session.dispatch_events()
        );
        if session.shared_spells().local_player().enabled_spell_holders[3] == holder_slot {
            break;
        }
    }
    let acquired = session.shared_spells().local_player();
    assert_eq!(acquired.enabled_spell_holders[3], holder_slot);
    assert_eq!(acquired.left_spell_slot_raw, i16::from(family));
    assert_eq!(
        session.entities()[usize::from(holder_slot)].action,
        family * 3
    );

    // sub_69300 refuses a holder while its ordinary 64-visit presentation
    // timer remains nonzero. Let the real holder callback expire it.
    for _ in 0..70 {
        if word(&session.entities()[usize::from(holder_slot)], 58) == 0 {
            break;
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "{:?}",
            session.dispatch_events()
        );
    }
    assert_eq!(word(&session.entities()[usize::from(holder_slot)], 58), 0);
    assert_eq!(session.entities()[usize::from(player_slot)].source_player_entity_index(), 0,
        "the actual player's word94 ownership relation does not identify its A4");
    assert_eq!(session.entities()[usize::from(player_slot)].special_relation,
        crate::native_world::EntitySpecialRelation::PlayerState(session.shared_spells().local_player().slot));

    // An old holder generation cannot mutate the genuinely acquired hand.
    let victim = session.source_arena().handle(player_slot).unwrap();
    let thief = session.source_arena().handle(actor_slot).unwrap();
    let mut wrong_generation = session.clone();
    let stale = SourceEntityHandle {
        source_slot: holder_slot,
        generation: holder_handle.generation.wrapping_add(1),
    };
    let theft = SpellTheft {
        thief,
        victim,
        victim_player: player_slot,
        holder: stale,
        family,
        hand: SpellHand::Left,
        victim_position: session.entities()[usize::from(player_slot)].position,
    };
    let live = &mut wrong_generation.session.state.live;
    assert!(!mc2::apply_spell_theft(
        &mut live.economy,
        &mut live.shared_spells,
        theft
    ));
    assert_eq!(
        live.shared_spells.local_player.enabled_spell_holders[3],
        holder_slot
    );
    assert_eq!(
        live.shared_spells.local_player.left_spell_slot_raw,
        i16::from(family)
    );

    // Approach the original actor with the actual acquired spell. Its own
    // targeting, cadence and unchanged global RNG decide whether theft occurs.
    let actor = session.entities()[usize::from(actor_slot)].clone();
    assert_eq!((actor.class, actor.model), (5, SOURCE_MODEL));
    let mut approach = actor.position;
    crate::move_entity(&mut approach, actor.yaw, 0, 384);
    session
        .set_level2_entity_position(player_slot, approach)
        .unwrap();
    let mut stolen = false;
    let mut drain_visits = 0;
    let mut in_range_cadence_visits = 0;
    let mut minimum_distance = u32::MAX;
    let mut heard_drain = false;
    let heard_since = |session: &crate::native_world::Mc2Simulation, cursor: u64| {
        session.source_arena().handle(actor_slot) == Some(thief)
            && session.effect_journal().records().iter().any(|record| {
                record.sequence >= cursor
                    && matches!(record.effect,
                        crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                            audio_site: crate::MissionAudioSite::Creature26Drain,
                            entity_index, player_index: -1, sound_id: 62,
                        }) if entity_index == actor_slot)
                    && record.audio.is_some_and(|audio| audio.command.audible && audio.play_type == 3)
            })
    };
    for _ in 0..256 {
        let actor_before = &session.entities()[usize::from(actor_slot)];
        let player_before = &session.entities()[usize::from(player_slot)];
        let range = distance(actor_before.position, player_before.position);
        minimum_distance = minimum_distance.min(range);
        if actor_before.action == DRAIN_ACTION {
            drain_visits += 1;
            if actor_before.phase & RULES.theft_cadence_mask == 0 && range < RULES.theft_range {
                in_range_cadence_visits += 1;
            }
        }
        let sound_cursor = session.effect_journal().cursor(0).next_sequence;
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "{:?}",
            session.dispatch_events()
        );
        heard_drain |= heard_since(&session, sound_cursor);
        if session.entities()[usize::from(holder_slot)].action == 78 {
            stolen = true;
            break;
        }
    }
    assert!(
        stolen,
        "model26 theft did not occur; before=(action {}, phase {}, target {}, position {:?}); after=(action {}, phase {}, target {}, position {:?}); player={:?}, holder=(action {}, timer {}), drain_visits={drain_visits}, in_range_cadence_visits={in_range_cadence_visits}, minimum_distance={minimum_distance}, global_rng={}",
        actor.action,
        actor.phase,
        word(&actor, 154),
        actor.position,
        session.entities()[usize::from(actor_slot)].action,
        session.entities()[usize::from(actor_slot)].phase,
        word(&session.entities()[usize::from(actor_slot)], 154),
        session.entities()[usize::from(actor_slot)].position,
        session.entities()[usize::from(player_slot)].position,
        session.entities()[usize::from(holder_slot)].action,
        word(&session.entities()[usize::from(holder_slot)], 58),
        session.economy().global_rng
    );
    let holder = &session.entities()[usize::from(holder_slot)];
    assert_eq!(word(holder, 42), actor_slot);
    assert_eq!(holder.source_word_4a(), 2);
    assert_eq!(
        session.source_arena().handle(holder_slot),
        Some(holder_handle)
    );
    let live = &session.session.state.live;
    let player = live
        .core
        .players
        .iter()
        .find(|player| player.entity_index == player_slot)
        .unwrap();
    assert_eq!(player.enabled_spell_holders[3], 0);
    assert_eq!(player.left_spell_slot_raw, -1);
    assert_eq!(live.shared_spells.local_player.enabled_spell_holders[3], 0);
    assert_eq!(live.shared_spells.local_player.left_spell_slot_raw, -1);

    // The earlier holder slot already ran this pass. Its next normal visits
    // perform action78 and eventually restore the ordinary pickup action.
    assert_eq!(
        session.entities()[usize::from(holder_slot)].source_dword_10(),
        0
    );
    let sound_cursor = session.effect_journal().cursor(0).next_sequence;
    session.step_outer_turn(PlayerCommand::default(), 0);
    heard_drain |= heard_since(&session, sound_cursor);
    assert!(
        session
            .dispatch_events()
            .iter()
            .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
        "{:?}",
        session.dispatch_events()
    );
    assert_eq!(session.entities()[usize::from(holder_slot)].action, 78);
    assert_eq!(
        session.entities()[usize::from(holder_slot)].source_dword_10(),
        1
    );
    let mut returned = false;
    for _ in 0..128 {
        let sound_cursor = session.effect_journal().cursor(0).next_sequence;
        session.step_outer_turn(PlayerCommand::default(), 0);
        heard_drain |= heard_since(&session, sound_cursor);
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "{:?}",
            session.dispatch_events()
        );
        if session.entities()[usize::from(holder_slot)].action == family * 3 + 1 {
            returned = true;
            break;
        }
    }
    assert!(
        returned,
        "the detached holder must return through its scheduled pickup action"
    );
    assert!(heard_drain, "the same original thief must publish audible prepared source62 during the real theft/holder-return lifecycle");
    assert_eq!(
        session.source_arena().handle(holder_slot),
        Some(holder_handle)
    );
    // Acquire the actual Shield pickup into the now-empty left hand. Without
    // the remembered return side the Speed pickup would choose the right hand;
    // this makes the source word_0x4A consumer distinguish the two paths.
    let shield = session.entities()[2].clone();
    assert_eq!((shield.class, shield.model, shield.action), (15, 6, 19));
    // Approach from the west: Shield's center also overlaps the real Beyond
    // Sight pickup. Check all current eligible pickups before placing the player.
    let mut shield_approach = shield.position;
    shield_approach.x = shield_approach.x.wrapping_sub(512);
    let mut player_at_shield = session.entities()[usize::from(player_slot)].clone();
    player_at_shield.set_position(shield_approach);
    for ground in session.entities().iter().filter(|row| {
        row.active && row.class == 15 && !row.drawing_disabled()
            && [crate::spell_activation::PhysicalSpellActionRole::OrdinaryPickup,
                crate::spell_activation::PhysicalSpellActionRole::PersistentPickup]
                .into_iter().any(|role|
                    crate::spell_activation::physical_spell_action(row.model, role) == Some(row.action))
            && session.shared_spells().local_player().enabled_spell_holders
                .get(usize::from(row.model)) == Some(&0)
    }) {
        assert_eq!(crate::native_world::level2_source_intersects(ground, &player_at_shield),
            ground.index == 2,
            "the declared Shield approach overlaps only Shield among eligible pickups: slot {} family {} position {:?}",
            ground.index, ground.model, ground.position);
    }
    session
        .set_level2_entity_position(player_slot, shield_approach)
        .unwrap();
    for _ in 0..8 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "{:?}",
            session.dispatch_events()
        );
        if session.shared_spells().local_player().enabled_spell_holders[6] == 2 {
            break;
        }
    }
    assert_eq!(
        session.shared_spells().local_player().enabled_spell_holders[6],
        2
    );
    assert_eq!(
        session.shared_spells().local_player().left_spell_slot_raw,
        6
    );
    assert_eq!(
        session.shared_spells().local_player().right_spell_slot_raw,
        -1
    );
    let returned_position = session.entities()[usize::from(holder_slot)].position;
    session
        .set_level2_entity_position(player_slot, returned_position)
        .unwrap();
    for _ in 0..8 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "{:?}",
            session.dispatch_events()
        );
        if session.shared_spells().local_player().enabled_spell_holders[3] == holder_slot {
            break;
        }
    }
    assert_eq!(
        session.shared_spells().local_player().enabled_spell_holders[3],
        holder_slot
    );
    assert_eq!(
        session.shared_spells().local_player().left_spell_slot_raw,
        i16::from(family)
    );
    assert_eq!(
        session.shared_spells().local_player().right_spell_slot_raw,
        -1
    );
    assert_eq!(
        session.entities()[usize::from(holder_slot)].source_word_4a(),
        0
    );
    assert_eq!(
        session.source_arena().handle(holder_slot),
        Some(holder_handle)
    );
}

#[test]
fn ordinary_damage_advances_prekill_mana_death_and_generation_reclaim() {
    let mut simulation = full_level_two_session();
    let particles = simulation
        .session
        .definition
        .presentation
        .particles
        .to_vec();
    let live = &mut simulation.session.state.live;
    let (arena, shared, runtime, terrain) = (
        &mut live.economy,
        &mut live.shared_spells,
        &mut live.core.runtime,
        &mut live.terrain,
    );
    let player = arena
        .entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.model == 0 && row.life >= 0)
        .expect("owner session has a living player");
    let (player_slot, position) = (player.index, player.position);
    let actor = mc2::allocate_authored(
        arena,
        runtime,
        terrain,
        &particles,
        position,
        RULES.max_life,
        None,
        500,
    )
    .unwrap()
    .expect("ordinary source slot");
    let handle = arena.handle(actor).unwrap();
    let empty_bindings = std::collections::BTreeMap::new();
    let living = |arena: &SourceArena| {
        arena
            .creature_runtime_census(&empty_bindings, &empty_bindings)
            .roots_of_kind(crate::level_definition::CreatureKind::Creature26)
            .any(|root| root.handle == handle && root.living)
    };
    assert!(living(arena));
    {
        let row = &mut arena.entities[usize::from(actor)];
        row.record[98..102].copy_from_slice(&(RULES.max_life + 1).to_le_bytes());
        put(row, 102, player_slot);
    }
    let mut nonfatal_arena = arena.clone();
    let mut nonfatal_shared = shared.clone();
    let mut nonfatal_terrain = terrain.clone();
    nonfatal_arena.entities[usize::from(actor)].record[98..102]
        .copy_from_slice(&(RULES.max_life - 1).to_le_bytes());
    let mut families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    families[usize::from(SOURCE_MODEL)].push(actor);
    mc2::step_row(
        &mut nonfatal_arena,
        &mut nonfatal_shared,
        runtime,
        &mut nonfatal_terrain,
        &particles,
        Default::default(),
        actor,
        &[player_slot],
        &families,
    )
    .unwrap();
    assert_eq!(
        (
            nonfatal_arena.entities[usize::from(actor)].life,
            nonfatal_arena.entities[usize::from(actor)].action
        ),
        (1, DRAIN_ACTION)
    );
    assert_eq!(
        nonfatal_arena.economy.authored_bee_kill_census(player_slot),
        0
    );
    assert!(living(&nonfatal_arena));
    mc2::step_row(
        arena,
        shared,
        runtime,
        terrain,
        &particles,
        Default::default(),
        actor,
        &[player_slot],
        &families,
    )
    .unwrap();
    assert_eq!(arena.entities[usize::from(actor)].action, PREKILL_ACTION);
    assert_eq!(arena.economy.authored_bee_kill_census(player_slot), 0);
    assert!(!living(arena));
    mc2::step_row(
        arena,
        shared,
        runtime,
        terrain,
        &particles,
        Default::default(),
        actor,
        &[player_slot],
        &families,
    )
    .unwrap();
    assert_eq!(arena.entities[usize::from(actor)].action, KILL_ACTION);
    assert_eq!(arena.economy.authored_bee_kill_census(player_slot), 1);
    arena.entities[usize::from(actor)].set_phase(0);
    let sphere_count = arena.economy.spheres.len();
    mc2::step_row(
        arena,
        shared,
        runtime,
        terrain,
        &particles,
        Default::default(),
        actor,
        &[player_slot],
        &families,
    )
    .unwrap();
    assert!(arena.entities[usize::from(actor)].drawing_disabled());
    assert_eq!(arena.economy.spheres[sphere_count].mana, RULES.initial_mana);
    assert_eq!(arena.handle(actor), Some(handle));
    arena
        .rebuild_source_arena_with_terrain(terrain, &[])
        .unwrap();
    assert!(arena.resolve(handle).is_none());
    assert!(!living(arena));
}

#[test]
fn stage_follow_copies_hidden_and_dead_parent_pose_before_hiding() {
    let simulation = full_level_two_session();
    let terrain = &simulation.session.state.live.terrain;
    let runtime = &simulation.session.state.live.core.runtime;
    let mut parent = player_target(2, position(21_000));
    parent.flags |= ENTITY_DRAW_DISABLED_BIT;
    parent.yaw = 123;
    parent.roll = 456;
    parent.source_yaw = Some(123);
    parent.source_roll = Some(456);
    let mut follower = actor_row(1, STAGE_ACTION, position(20_000));
    follower.set_life(RULES.max_life);
    follower.record[77] = 12;
    put(&mut follower, 44, parent.slot);
    let mut dead_parent = parent;
    dead_parent.life = -1;
    let mut dead_follow = follower.clone();
    let mut missing_parent = follower.clone();
    put(&mut missing_parent, 44, 5);
    let effects = run_model26_action(&mut follower, terrain, runtime, &[parent], &[2], &[1]);
    assert!(!effects.stage_hide);
    assert_eq!(
        (follower.position, follower.yaw, follower.source_roll()),
        (parent.position, parent.yaw, parent.roll)
    );
    let effects = run_model26_action(
        &mut dead_follow,
        terrain,
        runtime,
        &[dead_parent],
        &[2],
        &[1],
    );
    assert!(effects.stage_hide);
    assert_eq!(
        (
            dead_follow.position,
            dead_follow.yaw,
            dead_follow.source_roll()
        ),
        (parent.position, parent.yaw, parent.roll)
    );
    let effects = run_model26_action(&mut missing_parent, terrain, runtime, &[parent], &[2], &[1]);
    assert!(effects.stage_hide);
    assert_eq!(missing_parent.position, Position::default());
    assert_eq!((missing_parent.yaw, missing_parent.source_roll()), (0, 0));
}

#[test]
fn alliance_blink_is_committed_to_live_linked_actor_and_cleared_on_expiry() {
    let mut simulation = full_level_two_session();
    let particles = simulation
        .session
        .definition
        .presentation
        .particles
        .to_vec();
    let live = &mut simulation.session.state.live;
    let (arena, shared, runtime, terrain) = (
        &mut live.economy,
        &mut live.shared_spells,
        &mut live.core.runtime,
        &mut live.terrain,
    );
    let parent = arena
        .entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.model == 0 && row.life > 0)
        .expect("owner session has a live player");
    let (parent_slot, position) = (parent.index, parent.position);
    let actor = mc2::allocate_authored(
        arena,
        runtime,
        terrain,
        &particles,
        position,
        RULES.max_life,
        None,
        500,
    )
    .unwrap()
    .unwrap();
    let child = mc2::allocate_authored(
        arena,
        runtime,
        terrain,
        &particles,
        position,
        RULES.max_life,
        None,
        501,
    )
    .unwrap()
    .unwrap();
    {
        let row = &mut arena.entities[usize::from(actor)];
        row.set_source_action(STAGE_ACTION);
        row.set_phase(8);
        row.record[77] = 14;
        put(row, 44, parent_slot);
        put(row, 50, 40);
        put(row, 52, 40);
        put(row, 56, child);
    }
    let mut expired_arena = arena.clone();
    let mut expired_shared = shared.clone();
    let mut expired_terrain = terrain.clone();
    put(&mut expired_arena.entities[usize::from(actor)], 50, 1);
    let row = &mut expired_arena.entities[usize::from(child)];
    row.set_control_flags(row.control_flags | 0x0600_0000);
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    mc2::step_row(
        arena,
        shared,
        runtime,
        terrain,
        &particles,
        Default::default(),
        actor,
        &[parent_slot],
        &families,
    )
    .unwrap();
    assert_eq!(
        arena.entities[usize::from(child)].control_flags & 0x0600_0000,
        0x0400_0000
    );
    mc2::step_row(
        &mut expired_arena,
        &mut expired_shared,
        runtime,
        &mut expired_terrain,
        &particles,
        Default::default(),
        actor,
        &[parent_slot],
        &families,
    )
    .unwrap();
    assert_eq!(
        expired_arena.entities[usize::from(child)].control_flags & 0x0600_0000,
        0
    );
}

#[test]
fn alliance_follows_parent_target_and_updates_linked_blink_flags() {
    let simulation = full_level_two_session();
    let terrain = &simulation.session.state.live.terrain;
    let runtime = &simulation.session.state.live.core.runtime;
    let mut parent = player_target(2, position(20_000));
    parent.target = 3;
    parent.relations.target = Some(3);
    let enemy = player_target(3, position(21_000));
    let mut child = player_target(4, position(20_000));
    child.class = 5;
    child.model = SOURCE_MODEL;
    child.life = RULES.max_life;
    let mut summon = actor_row(1, STAGE_ACTION, position(20_000));
    summon.set_life(RULES.max_life);
    summon.set_phase(8);
    summon.record[77] = 14;
    put(&mut summon, 44, parent.slot);
    put(&mut summon, 50, 40);
    put(&mut summon, 52, 40);
    put(&mut summon, 56, child.slot);
    summon.record[98..102].copy_from_slice(&50_i32.to_le_bytes());
    put(&mut summon, 102, enemy.slot);
    let mut allied_target = enemy;
    allied_target.parent = parent.slot;
    allied_target.stage_mode = 14;
    allied_target.relations.parent = Some(parent.slot);
    allied_target.relations.stage_mode = Some(14);
    let mut rejected = summon.clone();
    rejected.record[98..104].fill(0);
    let effects = run_model26_action(
        &mut summon,
        terrain,
        runtime,
        &[parent, enemy, child],
        &[2, 3],
        &[1, 4],
    );
    assert_eq!(summon.life, RULES.max_life - 50);
    assert_eq!(
        (summon.action, word(&summon, 154)),
        (DRAIN_ACTION, enemy.slot)
    );
    assert_eq!(effects.earlier_feedback, Some((parent.slot, 19)));
    assert_eq!(effects.feedback, Some((parent.slot, 24)));
    assert_eq!(summon.control_flags & 0x0600_0000, 0x0400_0000);
    assert_eq!(
        effects.linked_blink_flags,
        vec![(child.handle, 0x0400_0000)]
    );
    let effects = run_model26_action(
        &mut rejected,
        terrain,
        runtime,
        &[parent, allied_target, child],
        &[2, 3],
        &[1, 4],
    );
    assert_eq!((rejected.action, word(&rejected, 154)), (STAGE_ACTION, 0));
    assert_eq!(effects.feedback, None);
    assert_eq!(effects.earlier_feedback, None);
}

#[test]
fn summon_acquires_an_enemy_and_returns_to_parent_when_none_exists() {
    let simulation = full_level_two_session();
    let terrain = &simulation.session.state.live.terrain;
    let runtime = &simulation.session.state.live.core.runtime;
    let parent = player_target(2, position(20_000));
    let enemy = player_target(3, position(21_000));
    let mut summon = actor_row(1, STAGE_ACTION, position(20_000));
    summon.set_life(RULES.max_life);
    summon.set_phase(8);
    summon.record[77] = 16;
    put(&mut summon, 44, parent.slot);
    put(&mut summon, 50, 10);
    let mut alone = summon.clone();

    let effects = run_model26_action(
        &mut summon,
        terrain,
        runtime,
        &[parent, enemy],
        &[parent.slot, enemy.slot],
        &[1],
    );
    assert_eq!(
        (summon.action, word(&summon, 154)),
        (DRAIN_ACTION, enemy.slot)
    );
    assert_eq!(word(&summon, 50), 10);
    assert_eq!(effects.feedback, None);

    let effects = run_model26_action(
        &mut alone,
        terrain,
        runtime,
        &[parent],
        &[parent.slot],
        &[1],
    );
    assert_eq!((alone.action, word(&alone, 154)), (STAGE_ACTION, 0));
    assert_eq!(word(&alone, 50), 6);
    assert_eq!(effects.feedback, None);
}

#[test]
fn summon_hostile_hit_awards_parent_while_parent_hit_does_not() {
    let simulation = full_level_two_session();
    let terrain = &simulation.session.state.live.terrain;
    let runtime = &simulation.session.state.live.core.runtime;
    let parent = player_target(2, position(20_000));
    let enemy = player_target(3, position(21_000));
    let mut summon = actor_row(1, STAGE_ACTION, position(20_000));
    summon.set_life(RULES.max_life);
    summon.set_phase(1);
    summon.record[77] = 16;
    put(&mut summon, 44, parent.slot);
    put(&mut summon, 50, 10);
    summon.record[98..102].copy_from_slice(&50_i32.to_le_bytes());
    put(&mut summon, 102, enemy.slot);
    let mut parent_hit = summon.clone();
    put(&mut parent_hit, 102, parent.slot);
    let targets = [parent, enemy];

    let effects = run_model26_action(&mut summon, terrain, runtime, &targets, &[2, 3], &[1]);
    assert_eq!(summon.life, RULES.max_life - 50);
    assert_eq!(summon.action, DRAIN_ACTION);
    assert_eq!(effects.feedback, Some((parent.slot, 19)));
    let effects = run_model26_action(&mut parent_hit, terrain, runtime, &targets, &[2, 3], &[1]);
    assert_eq!(parent_hit.life, RULES.max_life - 50);
    assert_eq!(parent_hit.action, STAGE_ACTION);
    assert_eq!(effects.feedback, None);
}

#[test]
fn summon_expiry_publishes_source_products_then_reclaims_on_next_pass() {
    for mode in [13, 16] {
        let mut simulation = full_level_two_session();
        let particles = simulation
            .session
            .definition
            .presentation
            .particles
            .to_vec();
        let live = &mut simulation.session.state.live;
        let position = live
            .economy
            .entities
            .iter()
            .find(|row| row.active && row.class == 3 && row.model == 0)
            .expect("source player")
            .position;
        let (arena, shared, runtime, terrain) = (
            &mut live.economy,
            &mut live.shared_spells,
            &mut live.core.runtime,
            &mut live.terrain,
        );
        let slot = mc2::allocate_authored(
            arena,
            runtime,
            terrain,
            &particles,
            position,
            RULES.max_life,
            None,
            500,
        )
        .unwrap()
        .expect("available source slot");
        let handle = arena.handle(slot).unwrap();
        {
            let row = &mut arena.entities[usize::from(slot)];
            row.set_source_action(STAGE_ACTION);
            row.record[76] = 0;
            row.record[77] = mode;
            put(row, 44, 0);
            put(row, 50, 10);
            row.record[98..102].copy_from_slice(&50_i32.to_le_bytes());
            put(row, 102, 1);
        }
        let before = arena
            .entities
            .iter()
            .filter(|row| row.active)
            .map(|row| row.index)
            .collect::<std::collections::BTreeSet<_>>();
        let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
        let parent = arena
            .entities
            .iter()
            .find(|row| row.active && row.class == 3 && row.model == 0)
            .unwrap()
            .index;
        let mut following_arena = arena.clone();
        let mut following_shared = shared.clone();
        let mut following_terrain = terrain.clone();
        {
            let row = &mut following_arena.entities[usize::from(slot)];
            put(row, 44, parent);
            row.record[98..104].fill(0);
            row.set_phase(1);
        }
        mc2::step_row(
            &mut following_arena,
            &mut following_shared,
            runtime,
            &mut following_terrain,
            &particles,
            Default::default(),
            slot,
            &[],
            &families,
        )
        .expect("live parent follow callback");
        assert!(!following_arena.entities[usize::from(slot)].drawing_disabled());
        assert!(
            following_arena
                .entities
                .iter()
                .all(|row| !row.active || before.contains(&row.index))
        );

        mc2::step_row(
            arena,
            shared,
            runtime,
            terrain,
            &particles,
            Default::default(),
            slot,
            &[],
            &families,
        )
        .expect("parent-loss expiry consumes real source products");
        let row = &arena.entities[usize::from(slot)];
        assert!(row.active && row.drawing_disabled());
        assert!(arena.economy.map_chains.is_linked(slot));
        assert_eq!(
            i32::from_le_bytes(row.record[98..102].try_into().unwrap()),
            50,
            "expiry precedes damage mailbox consumption"
        );
        assert_eq!(arena.handle(slot), Some(handle));
        let born = arena
            .entities
            .iter()
            .filter(|row| row.active && !before.contains(&row.index))
            .map(|row| (row.class, row.model))
            .collect::<Vec<_>>();
        assert!(born.contains(&(10, 0)));
        assert_eq!(born.contains(&(10, 73)), mode == 13);

        arena
            .rebuild_source_arena_with_terrain(terrain, &[])
            .unwrap();
        assert!(
            arena.resolve(handle).is_none(),
            "following pass reclaims the old generation"
        );
        assert!(!arena.economy.map_chains.is_linked(slot));
    }
}

#[test]
fn summon_expiry_reclaims_a_replacement_at_the_same_source_address() {
    let mut simulation = full_level_two_session();
    let particles = simulation
        .session
        .definition
        .presentation
        .particles
        .to_vec();
    let live = &mut simulation.session.state.live;
    let position = live
        .economy
        .entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.model == 0)
        .unwrap()
        .position;
    let (arena, shared, runtime, terrain) = (
        &mut live.economy,
        &mut live.shared_spells,
        &mut live.core.runtime,
        &mut live.terrain,
    );
    let slot = mc2::allocate_authored(
        arena,
        runtime,
        terrain,
        &particles,
        position,
        RULES.max_life,
        None,
        501,
    )
    .unwrap()
    .unwrap();
    for row in &mut arena.entities {
        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    }
    arena.economy.exhaust_new_event_capacity_for_test();
    {
        let row = &mut arena.entities[usize::from(slot)];
        row.set_source_action(STAGE_ACTION);
        row.set_control_flags(row.control_flags | crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
        row.record[76] = 0;
        row.record[77] = 16;
        put(row, 44, 0);
        put(row, 50, 10);
    }
    arena.rebuild_source_free_arena();
    let old = arena.handle(slot).unwrap();
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    mc2::step_row(
        arena,
        shared,
        runtime,
        terrain,
        &particles,
        Default::default(),
        slot,
        &[],
        &families,
    )
    .unwrap();
    assert!(arena.resolve(old).is_none());
    let replacement = arena
        .handle(slot)
        .expect("impact occupies the source address");
    assert_eq!(replacement.generation, old.generation.wrapping_add(1));
    let row = &arena.entities[usize::from(slot)];
    assert_eq!((row.class, row.model), (10, 0));
    assert!(row.drawing_disabled());
    arena
        .rebuild_source_arena_with_terrain(terrain, &[])
        .unwrap();
    assert!(arena.resolve(replacement).is_none());
}

#[test]
fn drain_applies_source_mana_amount_and_steals_the_selected_unheld_spell() {
    let terrain = terrain();
    let mut row = actor_row(1, DRAIN_ACTION, position(20_000));
    row.set_phase(0);
    put(&mut row, 154, 2);
    let thief = SourceEntityHandle {
        source_slot: 1,
        generation: 2,
    };
    let holder = SourceEntityHandle {
        source_slot: 8,
        generation: 4,
    };
    let mut target = player_target(2, position(21_000));
    target.right_spell = Some(SpellOffer {
        family: 4,
        holder,
        holder_thief: 0,
    });
    target.left_spell = Some(SpellOffer {
        family: 5,
        holder,
        holder_thief: 0,
    });
    let targets = [target];
    let class3 = [2];
    let mut global_rng = theft_roll_seed(4);
    let mut actor = Actor {
        row: &mut row,
        handle: thief,
    };
    let effects = {
        let mut context = Context {
            rules: &RULES,
            terrain: &terrain,
            targets: &targets,
            current_target: None,
            sentinel: None,
            class3: &class3,
            model26: &[],
            families: None,
            runtime: None,
            global_rng: &mut global_rng,
        };
        step(&mut actor, &mut context).unwrap()
    };

    assert_eq!(
        effects.mana_drain,
        Some(ManaDrain {
            target: target.handle,
            amount: 23
        })
    );
    assert_eq!(
        effects.spell_theft,
        Some(SpellTheft {
            thief,
            victim: target.handle,
            victim_player: 2,
            holder,
            family: 4,
            hand: SpellHand::Right,
            victim_position: target.position,
        })
    );
    assert_eq!(
        actor.row.action, DRAIN_ACTION,
        "the source early return keeps action210"
    );
    assert_eq!(actor.row.position, position(20_000));
    assert_eq!(global_rng % 63, 4);
}

#[test]
fn dynamic_theft_publishes_current_holder_and_uses_detached_return_consumer() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let mut arena = simulation.session.state.live.economy.clone();
    let mut shared = simulation.session.state.live.shared_spells.clone();
    let mut terrain = simulation.session.state.live.terrain.clone();
    let mut runtime = simulation.session.state.live.core.runtime.clone();

    let victim_slot = shared.player_entity.index;
    let victim = arena.entities[usize::from(victim_slot)].clone();
    assert!(victim.active && victim.class == 3 && victim.model == 0);
    let crate::native_world::EntitySpecialRelation::PlayerState(victim_player_slot) = victim.special_relation
        else { panic!("the actual victim retains its source A4 player relation") };
    let player_index = shared
        .players
        .iter()
        .position(|player| player.slot == victim_player_slot)
        .expect("victim has a source player record");
    let victim_player = shared.players[player_index].entity_index;
    let family = 4;
    let holder_action = family * 3;
    let pickup = arena
        .economy
        .allocate_spell_pickup(0, family, holder_action + 1, victim.position)
        .expect("the source allocator has a free dynamic holder slot");
    let holder_slot = pickup.source_entity_index;
    assert!(
        !arena.entities[usize::from(holder_slot)].active,
        "the setup exercises a current typed holder without a raw row"
    );
    assert!(arena.economy.sync_spell_pickup_projection(
        holder_slot,
        holder_action,
        victim.position
    ));
    let mut holder = crate::native_world::cave_features::new_event(
        holder_slot,
        pickup.entity_rng,
        victim.position,
    );
    holder.class = 15;
    holder.model = family;
    holder.source_id = holder_slot;
    holder.set_entity_rng(pickup.entity_rng);
    holder.set_source_action(holder_action);
    holder.set_control_flags(5);
    holder.set_phase(pickup.phase);
    holder.record[30..32].copy_from_slice(&holder_slot.to_le_bytes());
    holder.record[44..46].copy_from_slice(&victim_slot.to_le_bytes());
    holder.record[58..60].fill(0);
    shared.dynamic_pickups.push(holder);
    for player in &mut shared.players {
        equip_right_spell(player, victim_player, family, holder_slot);
    }
    equip_right_spell(&mut shared.local_player, victim_player, family, holder_slot);

    let thief_slot = arena
        .allocate_source_slot()
        .expect("the source allocator has a thief slot");
    let mut thief = actor_row(thief_slot, DRAIN_ACTION, position(20_000));
    thief.set_entity_rng(arena.new_event_rng(thief_slot));
    thief.set_life(100);
    arena.entities[usize::from(thief_slot)] = thief;
    let theft = SpellTheft {
        thief: arena.handle(thief_slot).expect("live model26 handle"),
        victim: arena.handle(victim_slot).expect("live player handle"),
        victim_player,
        holder: arena
            .handle(holder_slot)
            .expect("live dynamic holder handle"),
        family,
        hand: SpellHand::Right,
        victim_position: victim.position,
    };

    let mut occupied_arena = arena.clone();
    let mut occupied_shared = shared.clone();
    let mut occupant = actor_row(holder_slot, ROAM_ACTION, position(22_000));
    occupant.set_entity_rng(pickup.entity_rng.wrapping_add(1));
    let occupant_before = occupant.clone();
    occupied_arena.entities[usize::from(holder_slot)] = occupant;
    assert!(!mc2::apply_spell_theft(
        &mut occupied_arena,
        &mut occupied_shared,
        theft
    ));
    assert_eq!(
        occupied_arena.entities[usize::from(holder_slot)],
        occupant_before,
        "a current typed holder cannot overwrite a different raw occupant"
    );
    assert_eq!(
        occupied_shared.players[player_index].enabled_spell_holders[usize::from(family)],
        holder_slot,
        "a rejected identity leaves the player hand unchanged"
    );

    let selector_bytes = shared.pickup(holder_slot).unwrap().record[74..76].to_vec();
    assert!(mc2::apply_spell_theft(&mut arena, &mut shared, theft));
    let stolen = &arena.entities[usize::from(holder_slot)];
    assert!(stolen.active);
    assert_eq!(
        (stolen.class, stolen.model, stolen.action),
        (15, family, 78)
    );
    assert_eq!(word(stolen, 42), thief_slot);
    assert_eq!(stolen.source_word_4a(), 1);
    assert_eq!(&stolen.record[74..76], selector_bytes.as_slice());
    assert_eq!(stolen.position, victim.position);
    assert_eq!(
        shared.players[player_index].enabled_spell_holders[usize::from(family)],
        0
    );
    assert_eq!(shared.players[player_index].right_spell_slot_raw, -1);
    assert_eq!(arena.handle(holder_slot), Some(theft.holder));

    arena.entities[usize::from(thief_slot)].set_life(-1);
    for _ in 0..7 {
        crate::native_world::support_spells::step_detached_source_slot(
            &mut shared,
            &mut arena,
            &mut terrain,
            holder_slot,
        )
        .expect("the actual action78 source consumer advances the stolen holder");
        if arena.entities[usize::from(holder_slot)].action != 78 {
            break;
        }
    }
    let returned = arena.entities[usize::from(holder_slot)].clone();
    assert_eq!(returned.action, holder_action + 1);
    assert_eq!(word(&returned, 42), 0);
    mc2::prepare_source_pass(&mut arena, &mut runtime, Default::default(), &[]).unwrap();
    let typed = arena
        .economy
        .spell_pickups()
        .iter()
        .find(|source| source.active && source.source_entity_index == holder_slot)
        .expect("the dynamic holder remains in the same source generation");
    assert_eq!(typed.action, returned.action);
    assert_eq!(typed.position, returned.position);
    assert_eq!(typed.phase, returned.phase);
    assert_eq!(arena.handle(holder_slot), Some(theft.holder));
}

#[test]
fn drain_does_not_steal_a_spell_holder_already_linked_to_another_thief() {
    let terrain = terrain();
    let mut row = actor_row(1, DRAIN_ACTION, position(20_000));
    row.set_phase(0);
    put(&mut row, 154, 2);
    let holder = SourceEntityHandle {
        source_slot: 8,
        generation: 4,
    };
    let mut target = player_target(2, position(21_000));
    target.right_spell = Some(SpellOffer {
        family: 4,
        holder,
        holder_thief: 12,
    });
    let targets = [target];
    let class3 = [2];
    let mut global_rng = theft_roll_seed(4);
    let mut actor = Actor {
        row: &mut row,
        handle: SourceEntityHandle {
            source_slot: 1,
            generation: 2,
        },
    };
    let mut context = Context {
        rules: &RULES,
        terrain: &terrain,
        targets: &targets,
        current_target: None,
        sentinel: None,
        class3: &class3,
        model26: &[],
        families: None,
        runtime: None,
        global_rng: &mut global_rng,
    };

    let effects = step(&mut actor, &mut context).unwrap();

    assert_eq!(
        effects.mana_drain,
        Some(ManaDrain {
            target: target.handle,
            amount: 23
        })
    );
    assert_eq!(effects.spell_theft, None);
    assert_eq!(actor.row.action, DRAIN_ACTION);
    assert_eq!(actor.row.position, position(20_000));
}

#[test]
fn prekill_propagates_child_action_and_returns_source_kill_credit_slot() {
    let terrain = terrain();
    let mut row = actor_row(1, PREKILL_ACTION, position(20_000));
    put(&mut row, 56, 3);
    let mut child = player_target(3, position(20_100));
    child.class = 5;
    child.model = SOURCE_MODEL;
    child.killer = 2;
    let player = player_target(2, position(21_000));
    let targets = [child, player];
    let class3 = [2];
    let mut global_rng = 0;
    let mut actor = Actor {
        row: &mut row,
        handle: SourceEntityHandle {
            source_slot: 1,
            generation: 2,
        },
    };
    let mut context = Context {
        rules: &RULES,
        terrain: &terrain,
        targets: &targets,
        current_target: None,
        sentinel: None,
        class3: &class3,
        model26: &[1, 3],
        families: None,
        runtime: None,
        global_rng: &mut global_rng,
    };

    let effects = step(&mut actor, &mut context).unwrap();

    assert_eq!(effects.kill_credit, Some(2));
    assert_eq!(effects.peer_updates.len(), 1);
    assert_eq!(effects.peer_updates[0].action, Some(KILL_ACTION));
    assert_eq!(actor.row.action, KILL_ACTION);
}

#[test]
fn action214_is_rejected_as_unsupported_source_dispatch() {
    let terrain = terrain();
    let mut row = actor_row(1, 214, position(20_000));
    let mut global_rng = 0;
    let mut actor = Actor {
        row: &mut row,
        handle: SourceEntityHandle {
            source_slot: 1,
            generation: 2,
        },
    };
    let mut context = Context {
        rules: &RULES,
        terrain: &terrain,
        targets: &[],
        current_target: None,
        sentinel: None,
        class3: &[],
        model26: &[],
        families: None,
        runtime: None,
        global_rng: &mut global_rng,
    };

    let error = step(&mut actor, &mut context).unwrap_err();

    assert!(error.contains("unsupported model26 action214"));
}

#[test]
fn configured_movement_uses_descriptor_subtype_for_source_turn() {
    let terrain = terrain();
    let mut row = actor_row(1, ROAM_ACTION, position(20_000));
    row.set_control_flags(row.control_flags & !0x800);
    row.set_life(RULES.max_life);
    row.set_yaw(0);
    row.set_source_roll(200);
    row.set_actual_speed(0);
    let mut rng = 0;
    let ctx = Context { rules: &RULES, terrain: &terrain, targets: &[], current_target: None, sentinel: None, class3: &[],
        model26: &[], families: None, runtime: None, global_rng: &mut rng };
    let handle = SourceEntityHandle { source_slot: row.index, generation: 1 };
    assert!(!move_source(&mut Actor { row: &mut row, handle }, &ctx));
    assert_eq!(row.yaw, 5, "source58350 clamps by descriptor subtype, not word4=91");
}

#[test]
fn configured_drain_reads_current_target_and_source_peer_width() {
    let terrain = terrain();
    let runtime = crate::native_world::tests::level_two_campaign_session().session.state.live.core.runtime.clone();
    let at = Position { x: 65_520, y: 20_000, z: 5_000 };
    let player = player_target(2, Position { x: 65_420, ..at });
    let mut distant_peer = player_target(3, Position { x: 8, ..at });
    distant_peer.class = 5;
    distant_peer.model = SOURCE_MODEL;
    let mut row = actor_row(1, DRAIN_ACTION, at);
    row.set_life(RULES.max_life);
    row.set_phase(4);
    row.set_source_overlap_half_extents(256, 256);
    row.set_pitch(30_000);
    put(&mut row, 154, player.slot);
    let effects = run_model26_action(&mut row, &terrain, &runtime,
        &[player, distant_peer], &[], &[1, 3]);
    assert_eq!(effects.mana_drain.unwrap().target, player.handle,
        "word96 reads the current player even outside the frozen class3 head");
    assert_eq!(row.source_roll(), heading_for_positions(at, player.position) as i16,
        "unsigned source coordinate distance does not wrap around the map");

    let mut near_peer = distant_peer;
    near_peer.position.x = 65_500;
    let mut row = actor_row(1, DRAIN_ACTION, at);
    row.set_life(RULES.max_life);
    row.set_phase(4);
    row.set_source_overlap_half_extents(256, 256);
    row.set_pitch(0);
    put(&mut row, 154, player.slot);
    run_model26_action(&mut row, &terrain, &runtime, &[player, near_peer], &[], &[1, 3]);
    assert_eq!(row.source_roll(), heading_for_positions(near_peer.position, at) as i16,
        "peer avoidance uses current collision-array pitch, not actor pitch");
}

#[test]
fn configured_scheduled_drain_uses_current_typed_castle_mana_not_raw_shadow() {
    let base = full_level_two_session();
    for configured_model in [None, Some(0), Some(1)] {
    let mut session = base.clone();
    let particles = session.session.definition.presentation.particles.to_vec();
    let live = &mut session.session.state.live;
    let player = live.core.state.entity_index;
    let position = live.core.state.position;
    let castle = live.economy.economy.build_castle(position, player, &live.terrain)
        .expect("real allocated Castle carrier");
    let castle_handle = live.economy.handle(castle).unwrap();
    {
        let current = live.economy.economy.castle_entry_mut(castle).unwrap();
        assert_eq!((current.source_class, current.source_model), (3, 2));
        // Model2 is the real Castle packet and must be rejected. Models0/1
        // are separately configured current-address controls, not players.
        if let Some(model) = configured_model { current.source_model = model; }
        current.life = 1_000;
        current.mana = 300;
        current.mana_capacity = 100;
    }
    let raw = &mut live.economy.entities[usize::from(castle)];
    raw.set_mana(-999);
    raw.set_mana_regen(-19);
    let actor = mc2::allocate_authored(&mut live.economy, &mut live.core.runtime,
        &mut live.terrain, &particles, position, RULES.max_life, None, 500)
        .unwrap().unwrap();
    let row = &mut live.economy.entities[usize::from(actor)];
    row.set_source_action(DRAIN_ACTION);
    row.set_phase(1); // no theft roll; exercise the real action210 callback
    row.set_control_flags(row.control_flags | 0x800);
    put(row, 154, castle);
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    mc2::step_row(&mut live.economy, &mut live.shared_spells, &live.core.runtime,
        &mut live.terrain, &particles, Default::default(), actor, &[], &families).unwrap();
    assert_eq!(live.economy.handle(castle), Some(castle_handle));
    assert_eq!(live.economy.economy.castle_entry(castle).unwrap().mana,
        if configured_model.is_some() { 305 } else { 300 },
        "source accepts only0/1, subtracts signed regen+14, and has no capacity clamp");
    assert_eq!(live.economy.entities[usize::from(castle)].mana, -999,
        "the cold raw shadow is never the mana owner");
    }
}

#[test]
fn configured_stationary_and_roaming_peer_scans_use_source_heading_order() {
    let terrain = terrain();
    let at = position(20_000);
    let mut row = actor_row(1, STATIONARY_ACTION, at);
    row.set_life(RULES.max_life);
    let mut peer = player_target(3, position(21_000));
    peer.class = 5;
    peer.model = SOURCE_MODEL;
    row.set_yaw(heading_for_positions(peer.position, at) as i16);
    let mut rules = RULES;
    rules.target_half_angle = 128;
    let mut rng = 0;
    let ctx = Context { rules: &rules, terrain: &terrain, targets: &[peer], current_target: None, sentinel: None, class3: &[],
        model26: &[3], families: None, runtime: None, global_rng: &mut rng };
    let handle = SourceEntityHandle { source_slot: row.index, generation: 1 };
    let actor = Actor { row: &mut row, handle };
    assert_eq!(scan_peer(&actor, &ctx, true).map(|peer| peer.slot), Some(3));
    assert_eq!(scan_peer(&actor, &ctx, false).map(|peer| peer.slot), None);
}

#[test]
fn configured_linked_step_keeps_source_short_circuit_move_and_peer_tail() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = simulation.canonical_terrain();
    let runtime = &simulation.session.state.live.core.runtime;
    let at = position(20_000);

    let mut unlinked = actor_row(1, LINK_ACTION, at);
    unlinked.set_life(RULES.max_life);
    unlinked.record[98..102].copy_from_slice(&100_i32.to_le_bytes());
    put(&mut unlinked, 102, 2);
    run_model26_action(&mut unlinked, terrain, runtime, &[], &[], &[]);
    assert_eq!((unlinked.action, unlinked.life, word(&unlinked, 102)),
        (ROAM_ACTION, RULES.max_life, 2), "word32 zero returns before damage");

    let mut invalid = actor_row(1, LINK_ACTION, at);
    invalid.set_life(RULES.max_life);
    invalid.set_control_flags(invalid.control_flags & !0x800);
    invalid.set_yaw(0);
    invalid.set_source_roll(200);
    invalid.set_actual_speed(0);
    put(&mut invalid, 54, 9);
    run_model26_action(&mut invalid, terrain, runtime, &[], &[], &[]);
    assert_eq!((invalid.action, word(&invalid, 54), invalid.yaw), (ROAM_ACTION, 0, 5),
        "invalid nonzero peer is discarded only after source movement");

    let mut peer = player_target(3, position(21_000));
    peer.class = 5;
    peer.model = SOURCE_MODEL;
    peer.action = STATIONARY_ACTION;
    peer.maximum_speed = 50;
    peer.actual_speed = 30;
    let mut follower = actor_row(1, LINK_ACTION, at);
    follower.set_life(RULES.max_life);
    follower.set_phase(0);
    follower.set_actual_speed(17);
    put(&mut follower, 54, peer.slot);
    run_model26_action(&mut follower, terrain, runtime, &[peer], &[], &[1, 3]);
    assert_eq!(follower.actual_speed, 80, "source sums peer max and current speed");

    let edge = Position { x: 65_520, y: 20_000, z: 5_000 };
    let mut linked_peer = peer;
    linked_peer.position.x = 65_000;
    linked_peer.action = LINK_ACTION;
    linked_peer.link = 5;
    let next = player_target(5, Position { x: 64_000, ..edge });
    let mut other = player_target(4, Position { x: 8, ..edge });
    other.class = 5;
    other.model = SOURCE_MODEL;
    let mut follower = actor_row(1, LINK_ACTION, edge);
    follower.set_life(RULES.max_life);
    follower.set_phase(0);
    follower.set_source_overlap_half_extents(0, 0);
    put(&mut follower, 54, linked_peer.slot);
    run_model26_action(&mut follower, terrain, runtime,
        &[linked_peer, next, other], &[], &[1, 3, 4]);
    assert_eq!(word(&follower, 54), 5);
    assert_eq!(follower.source_roll(), heading_for_positions(other.position, edge) as i16,
        "peer action211 also executes signed-coordinate fixed256 avoidance");
    assert_eq!(follower.actual_speed, 80);
}

#[test]
fn configured_roam_peer_scan_obeys_source_proximity_byte() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = simulation.canonical_terrain();
    let runtime = &simulation.session.state.live.core.runtime;
    let at = position(20_000);
    let mut peer = player_target(3, position(21_000));
    peer.class = 5;
    peer.model = SOURCE_MODEL;
    let mut row = actor_row(1, ROAM_ACTION, at);
    row.set_life(RULES.max_life);
    row.set_phase(0);
    row.record[61] = 0;
    run_model26_action(&mut row, terrain, runtime, &[peer], &[], &[1, 3]);
    assert_eq!((row.action, word(&row, 54)), (ROAM_ACTION, 0),
        "source68C70 zero proximity suppresses the peer fallback as well as the player scan");
}

#[test]
fn configured_stage_dispatch_preserves_default_attack_and_fatal_population() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = simulation.canonical_terrain();
    let mut runtime = simulation.session.state.live.core.runtime.clone();
    runtime.stage_variables[0] = RuntimeStageVariable {
        kind: 1, flags: 0, word_2: 0,
        relation: RuntimeRelation::AxisScalar((20_000_u32 << 16) | 30_000),
    };
    let mut attack = actor_row(1, STAGE_ACTION, position(20_000));
    attack.set_life(RULES.max_life);
    attack.record[76] = 255;
    attack.record[77] = 10;
    put(&mut attack, 154, 123);
    run_model26_action(&mut attack, terrain, &runtime, &[], &[], &[]);
    assert_eq!((attack.action, word(&attack, 154)), (DRAIN_ACTION, 123),
        "source1E040 selects base+2 without consulting descriptor or target");

    let mut default_case = actor_row(1, STAGE_ACTION, position(20_000));
    default_case.set_life(RULES.max_life);
    default_case.record[76] = 255;
    default_case.record[77] = 11;
    let before = default_case.clone();
    let result = run_model26_action(&mut default_case, terrain, &runtime, &[], &[], &[]);
    assert_eq!(default_case, before, "the source default returns without a StageVar lookup");
    assert_eq!(result.population_decrements, 0);

    let mut fatal = actor_row(1, STAGE_ACTION, position(20_000));
    fatal.set_life(50);
    fatal.record[76] = 0;
    fatal.record[77] = 1;
    fatal.record[98..102].copy_from_slice(&100_i32.to_le_bytes());
    put(&mut fatal, 102, 2);
    let result = run_model26_action(&mut fatal, terrain, &runtime, &[], &[], &[]);
    assert_eq!((fatal.life, fatal.action), (-50, PREKILL_ACTION));
    assert_eq!(result.population_decrements, 0,
        "source1DDA0 fatal damage selects prekill without movement-population decrement");
}

#[test]
fn configured_stage_linked_hit_compares_current_zero_sentinel_family() {
    let terrain = terrain();
    let mut child = player_target(3, position(20_100));
    child.class = 5;
    child.model = SOURCE_MODEL;
    child.life = 900;
    child.relations.attacker = Some(0);
    let targets = [child];
    for same_family in [true, false] {
        let mut sentinel = player_target(0, Position::default());
        sentinel.class = if same_family { 5 } else { 0 };
        sentinel.model = if same_family { SOURCE_MODEL } else { 0 };
        let mut row = actor_row(1, STAGE_ACTION, position(20_000));
        row.set_life(1_000);
        row.record[77] = 1;
        put(&mut row, 56, child.slot);
        put(&mut row, 154, 7);
        let mut rng = 0;
        let mut ctx = Context { rules: &RULES, terrain: &terrain, targets: &targets,
            current_target: None, sentinel: Some(sentinel), class3: &[], model26: &[1, 3],
            families: None, runtime: None, global_rng: &mut rng };
        let handle = SourceEntityHandle { source_slot: 1, generation: 1 };
        step(&mut Actor { row: &mut row, handle }, &mut ctx).unwrap();
        assert_eq!(row.life, 900);
        assert_eq!((row.action, word(&row, 154)),
            if same_family { (STAGE_ACTION, 7) } else { (DRAIN_ACTION, 0) },
            "source compares actual Entities[0] class/model after inherited attacker0");
    }
}

#[test]
fn level10_real_switch_produces_bound_model26_guard_route() {
    use crate::campaign::{CampaignSession, source_arena::CreatureRootOrigin, spec::CampaignSelection};
    use crate::native_world::Mc2DispatchEvent;

    let package = std::env::var_os("MC_LEVEL10_PACKAGE").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/ribbon-20261104-integration/current-owner-package/level10"),
        std::path::PathBuf::from,
    );
    let mut session = CampaignSession::open(package, CampaignSelection::ordinary(10).unwrap())
        .expect("whole qualified Level10 owner package").simulation().clone();
    let switch = session.source_arena().handle(313).expect("real reset switch313");
    let row = &session.entities()[313];
    assert_eq!((row.class, row.model, row.action, row.source_id, row.level_gate),
        (11, 32, 32, 15, 2));
    assert_eq!(switch.generation, 0);
    let feature = session.mission().authored_features().iter().find(|feature| feature.index == 119)
        .expect("actual authored row119");
    assert_eq!((feature.kind, feature.subtype, feature.activation_group), (5, 26, 15));

    // Declared upstream objective prerequisite; the original switch, whole
    // group15 scan, constructor and subsequent callbacks remain scheduled.
    let player_color = usize::from(session.players()[0].player_color_index());
    assert_eq!(session.runtime().stage_index, 4);
    assert_eq!(session.runtime().objectives[player_color].stage_selectors[2], 1);
    session.session.state.live.core.runtime
        .set_level2_objective_stage_selector_for(player_color, 2, 2);
    assert_eq!(session.runtime().objectives[player_color].stage_selectors[2], 2);
    let mut produced = false;
    for turn in 0..32 {
        session.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert!(!session.dispatch_events().iter().any(|event|
            matches!(event, Mc2DispatchEvent::Error { .. })), "group turn{turn}: {:?}", session.dispatch_events());
        if session.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 313, group: 15, authored_indices, .. }
                if authored_indices.contains(&119))) {
            produced = true;
            break;
        }
    }
    assert!(produced, "actual switch313 must produce the full authored group15");
    let root = session.creature_runtime_census().roots_of_kind(crate::level_definition::CreatureKind::Creature26)
        .find(|root| root.origin == (CreatureRootOrigin::AuthoredRecord { authored_record: 119 }))
        .expect("actual row119 original model26 generation").handle;
    let initial = &session.entities()[usize::from(root.source_slot)];
    assert_eq!((initial.class, initial.model, initial.action, initial.record[76], initial.record[77]),
        (5, 26, STAGE_ACTION, 2, 2));
    let point = Position { x: 37 << 8, y: 254 << 8, z: 0 };
    let mut moved = false;
    let mut steered = false;
    for turn in 0..80 {
        let peers = session.source_arena().rebuilt_class5_model_slots()[26].iter()
            .copied().filter(|slot| *slot != root.source_slot)
            .map(|slot| session.entities()[usize::from(slot)].position).collect::<Vec<_>>();
        mc2::start_callback_damage_trace(root);
        session.step_outer_turn(crate::PlayerCommand::default(), 0);
        let callbacks = mc2::take_callback_damage_trace();
        assert!(!session.dispatch_events().iter().any(|event|
            matches!(event, Mc2DispatchEvent::Error { .. })), "guard turn{turn}: {:?}", session.dispatch_events());
        for callback in callbacks {
            let Some(after) = callback.after else { continue };
            let before = callback.before;
            if before.action != STAGE_ACTION || before.record[77] != 2 { continue; }
            moved |= before.position != after.position;
            let away = (i32::from(before.position.x as i16) - i32::from(point.x as i16)).abs() > 3072
                || (i32::from(before.position.y as i16) - i32::from(point.y as i16)).abs() > 3072;
            let isolated = peers.iter().all(|peer|
                (i32::from(before.position.x as i16) - i32::from(peer.x as i16)).abs() > 512
                    || (i32::from(before.position.y as i16) - i32::from(peer.y as i16)).abs() > 512);
            if away && isolated && before.phase & 7 == 0 && before.phase & 63 != 0
                && after.control_flags & 0x40000 == 0 && after.life >= 0
                && after.action == STAGE_ACTION && after.record[77] == 2 {
                assert_eq!(after.source_roll(), heading_for_positions(after.position, point) as i16,
                    "the actual bound guard points toward its authored StageVar after moving");
                steered = true;
            }
        }
        if moved && steered { break; }
    }
    assert!(moved && steered, "original119 must move and steer under real StageVar2: root={root:?} moved={moved} steered={steered}");
}

#[test]
fn stage_peer_avoidance_runs_even_when_heading_update_is_blocked() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = &simulation.session.state.live.terrain;
    let mut runtime = simulation.session.state.live.core.runtime.clone();
    runtime.stage_variables[0] = RuntimeStageVariable {
        kind: 1,
        flags: 0,
        word_2: 0,
        relation: RuntimeRelation::AxisScalar((20_000_u32 << 16) | 30_000),
    };
    let mut actor = actor_row(1, STAGE_ACTION, position(20_000));
    actor.set_life(RULES.max_life);
    actor.set_phase(8);
    actor.record[77] = 1;
    actor.set_source_roll(777);
    actor.set_source_overlap_half_extents(RULES.collision_half_width, RULES.collision_half_width);
    actor.set_control_flags(actor.control_flags | 0x40000);
    let mut without_peer = actor.clone();
    let mut peer = player_target(3, position(20_100));
    peer.class = 5;
    peer.model = SOURCE_MODEL;

    run_model26_action(&mut actor, terrain, &runtime, &[peer], &[], &[1, 3]);
    assert_eq!(
        actor.source_roll(),
        heading_for_positions(peer.position, actor.position) as i16
    );
    run_model26_action(&mut without_peer, terrain, &runtime, &[], &[], &[1]);
    assert_eq!(without_peer.source_roll(), 777);
}

#[test]
fn action215_turns_toward_its_bound_stage_point() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = simulation.session.state.live.terrain.clone();
    let mut runtime = simulation.session.state.live.core.runtime.clone();
    runtime.stage_variables[1] = RuntimeStageVariable {
        kind: 1,
        flags: 0,
        word_2: 0,
        relation: RuntimeRelation::AxisScalar((20_000_u32 << 16) | 30_000),
    };
    let mut row = actor_row(1, STAGE_ACTION, position(20_000));
    row.set_phase(8);
    row.record[76] = 1;
    row.record[77] = 1;
    row.set_source_roll(777);
    let effects = run_model26_action(&mut row, &terrain, &runtime, &[], &[], &[1]);

    assert_eq!(row.source_roll(), 512);
    assert_eq!(effects.population_decrements, 0);

    let player = player_target(2, position(21_000));
    let targets = [player];
    let class3 = [2];
    let model26 = [1];

    let mut hit = actor_row(1, STAGE_ACTION, position(20_000));
    hit.max_life = 4_400;
    hit.life = 4_400;
    hit.record[76] = 1;
    hit.record[77] = 1;
    hit.record[98..102].copy_from_slice(&100_i32.to_le_bytes());
    put(&mut hit, 102, player.slot);
    let effects = run_model26_action(&mut hit, &terrain, &runtime, &targets, &class3, &model26);

    assert_eq!(hit.life, 4_300);
    assert_eq!(hit.action, DRAIN_ACTION);
    assert_eq!(hit.record[77], 10);
    assert_eq!(word(&hit, 154), player.slot);
    assert_eq!(effects.population_decrements, 0);

    let mut same_model_attacker = player_target(3, position(21_000));
    same_model_attacker.class = 5;
    same_model_attacker.model = SOURCE_MODEL;
    let same_model_targets = [same_model_attacker];
    let same_model_slots = [1, same_model_attacker.slot];
    let mut same_model_hit = actor_row(1, STAGE_ACTION, position(20_000));
    same_model_hit.max_life = 4_400;
    same_model_hit.life = 4_400;
    same_model_hit.record[76] = 1;
    same_model_hit.record[77] = 1;
    same_model_hit.record[98..102].copy_from_slice(&100_i32.to_le_bytes());
    put(&mut same_model_hit, 102, same_model_attacker.slot);
    let effects = run_model26_action(
        &mut same_model_hit,
        &terrain,
        &runtime,
        &same_model_targets,
        &[],
        &same_model_slots,
    );

    assert_eq!(same_model_hit.action, STAGE_ACTION);
    assert_eq!(same_model_hit.record[77], 1);
    assert_eq!(effects.population_decrements, 0);

    let mut fatal = actor_row(1, STAGE_ACTION, position(20_000));
    fatal.life = 50;
    fatal.record[76] = 1;
    fatal.record[77] = 1;
    fatal.record[98..102].copy_from_slice(&100_i32.to_le_bytes());
    put(&mut fatal, 102, player.slot);
    let effects = run_model26_action(&mut fatal, &terrain, &runtime, &targets, &class3, &model26);

    assert_eq!(fatal.life, -50);
    assert_eq!(fatal.action, PREKILL_ACTION);
    assert_eq!(effects.population_decrements, 0,
        "source1DDA0 fatal damage returns without the movement population decrement");

    let mut early_return_fatal = actor_row(1, STAGE_ACTION, position(20_000));
    early_return_fatal.life = 50;
    early_return_fatal.record[76] = 1;
    early_return_fatal.record[77] = 2;
    early_return_fatal.record[98..102].copy_from_slice(&100_i32.to_le_bytes());
    put(&mut early_return_fatal, 102, player.slot);
    let effects = run_model26_action(
        &mut early_return_fatal,
        &terrain,
        &runtime,
        &targets,
        &class3,
        &model26,
    );

    assert_eq!(early_return_fatal.action, PREKILL_ACTION);
    assert_eq!(effects.population_decrements, 0,
        "source1DBF0 fatal damage returns without the movement population decrement");
}
