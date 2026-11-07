use super::*;
use crate::native_world::area_damage;
use crate::native_world::ENTITY_DRAW_DISABLED_BIT;
use crate::world::effects::DamagePulse;
use std::collections::BTreeSet;

fn ready_lightning(tier: u8) -> (super::super::Mc2Simulation, u16, u16) {
    let mut session = super::super::tests::level_two_campaign_session();
    let live = &mut session.session.state.live;
    let parent = live.core.state.entity_index;
    let slot = live
        .shared_spells
        .pickups()
        .iter()
        .find(|row| row.model == 3)
        .expect("campaign fixture has the existing spell pickup")
        .index;
    {
        let holder = live.shared_spells.pickup_mut(slot).unwrap();
        holder.model = 7;
        holder.record[68] = 7;
        holder.set_source_action(21);
        holder.active_value = 0;
        put(holder, 50, 0);
        holder.apply_nyphur_spell_profile(
            crate::spell_activation::level2_set_spell_profile(7, tier)
                .expect("Lightning tier has a source profile"),
            tier,
        );
        let activation = holder.activation_value;
        holder.active_value = activation as i16;
        put(holder, 50, activation);
        put(holder, 44, parent);
        holder.set_control_flags(holder.control_flags & !ENTITY_DRAW_DISABLED_BIT);
    }
    let max_mana = live.shared_spells.pickup(slot).unwrap().max_mana;
    let player = &mut live.economy.entities[usize::from(parent)];
    player.class = 3;
    player.model = 0;
    player.record[67] = 3;
    player.record[68] = 0;
    player.set_mana(max_mana.max(RULES.tiers[usize::from(tier)].mana_cost));
    player.set_mana_regen(0);
    (session, slot, parent)
}

fn exhaust(arena: &mut SourceArena) {
    for row in &mut arena.entities {
        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    }
    arena.economy.exhaust_new_event_capacity_for_test();
    arena.rebuild_source_free_arena();
}

fn next_rng(rng: &mut u16) -> u16 {
    *rng = rng.wrapping_mul(9377).wrapping_add(9439);
    *rng
}

fn expected_one_step_ray_rng(seed: u16) -> u16 {
    let mut rng = seed;
    let mut bends = [0_i16; 2];
    for remaining in (0..=8).rev() {
        let width = (remaining / 2).clamp(0, 8) as i16;
        for bend in &mut bends {
            *bend = if *bend > width {
                bend.wrapping_sub(1)
            } else if *bend < -width {
                bend.wrapping_add(1)
            } else {
                2 * ((next_rng(&mut rng) % 157) / 79) as i16 - 1 + *bend
            };
        }
    }
    rng
}

#[test]
fn lightning_uses_source_turn_kernel_and_ray_accuracy_identity() {
    assert_eq!(sub58350_delta(0, 2047, 5), -1);
    assert_eq!(sub58350_delta(2047, 0, 5), 1);
    assert_eq!(sub58350_delta(100, 600, 5), 5);
    assert_eq!(sub58350_delta(600, 100, 5), -5);
    assert_eq!(sub58350_delta(17, 17, 5), 0);

    let (mut session, _, owner_slot) = ready_lightning(0);
    let arena = &mut session.session.state.live.economy;
    let targets = arena
        .entities
        .iter()
        .filter(|row| row.active && row.index != 0 && row.index != owner_slot)
        .take(2)
        .map(|row| row.index)
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 2);
    let [hit_slot, target_slot] = [targets[0], targets[1]];
    arena.entities[usize::from(hit_slot)].source_id = 77;
    arena.entities[usize::from(target_slot)].source_id = 77;
    assert_eq!(
        guided_projectile::source_accuracy(arena, 9, owner_slot, Some(hit_slot), target_slot),
        Some((owner_slot, true))
    );
    arena.entities[usize::from(hit_slot)].source_id = 78;
    assert_eq!(
        guided_projectile::source_accuracy(arena, 9, owner_slot, Some(hit_slot), target_slot),
        Some((owner_slot, false))
    );
    assert_eq!(
        guided_projectile::source_accuracy(arena, 13, owner_slot, Some(hit_slot), target_slot),
        None,
        "a non-source-eligible model is not an accuracy attempt"
    );
}

#[test]
fn family22_ray_keeps_nonzero_fixed_targets_and_acquires_only_from_zero() {
    let (mut session, _, owner_slot) = ready_lightning(2);
    let live = &mut session.session.state.live;
    let target = live
        .economy
        .entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.model == 1)
        .cloned()
        .expect("qualified campaign fixture has a live rival target");
    assert_ne!(target.source_id, owner_slot);
    let target_slot = target.index;
    let target_position = Position {
        z: target.position.z.wrapping_add(target.array_yaw()),
        ..target.position
    };

    let terrain = &live.terrain;
    let arena = &mut live.economy;
    let fireball_slot = arena
        .economy
        .spawn_fireball_projectile_with_payload(
            crate::native_world::level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
                origin: crate::native_world::FireballProjectileOrigin::PlayerSpell,
                position: target_position,
                yaw: 0,
                pitch: 0,
                axis_position: target_position,
                source_id_entity_index: owner_slot,
                holder_entity_index: 0,
                source_subspell_index: 100,
                source_mana: 50,
                source_dword_0x10: 0,
                target_source_entity_index: target_slot,
                target_class: target.class,
                target_model: target.model,
            },
            0,
        )
        .expect("the canonical Fireball producer allocates a live typed row");
    assert!(arena.handle(fireball_slot).is_some());
    assert!(
        area_damage::source_actor(arena, None, fireball_slot).is_none(),
        "the area-damage actor projection does not represent a typed Fireball"
    );
    let stale_slot = (1..u16::try_from(arena.entities.len()).expect("arena fits u16"))
        .find(|&slot| arena.handle(slot).is_none())
        .expect("campaign fixture has an unoccupied source slot");
    let reclaimed_slot = arena.economy.allocate_slot().expect("source slot for reclaim");
    {
        let row = &mut arena.entities[usize::from(reclaimed_slot)];
        row.active = true;
        row.class = 3;
    }
    assert!(arena.handle(reclaimed_slot).is_some());
    arena.entities[usize::from(reclaimed_slot)].active = false;
    arena.entities[usize::from(reclaimed_slot)].class = 0;
    arena.economy.release_slot(reclaimed_slot, None);
    assert!(arena.handle(reclaimed_slot).is_none());
    let owner_row = &mut arena.entities[usize::from(owner_slot)];
    owner_row.class = 10;
    owner_row.model = 57;
    owner_row.record[67] = 10;
    owner_row.record[68] = 57;

    let origin = Position {
        x: target_position.x.wrapping_sub(4096),
        ..target_position
    };
    let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(origin, target_position);
    let mut ray = arena.entities[usize::from(owner_slot)].clone();
    ray.class = 9;
    ray.model = 9;
    ray.record[67] = 9;
    ray.record[68] = 9;
    ray.set_source_action(9);
    ray.set_control_flags(0);
    ray.set_position(origin);
    ray.set_yaw(yaw);
    ray.set_pitch(pitch);
    ray.set_actual_speed(1);
    ray.record[136..138].copy_from_slice(&100_u16.to_le_bytes());
    ray.set_max_life(100);
    ray.set_life(100);
    ray.record[69] = 9;
    ray.record[70] = 0;
    owner(&mut ray, owner_slot);
    put(&mut ray, 154, fireball_slot);

    let empty_class5: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let class3 = [target_slot];
    let targets = guided_projectile::Targets {
        class3: &class3,
        class5: &empty_class5,
        class10: &[],
        owner_target_range: 8192,
    };
    let mut outcome = guided_projectile::StepOutcome::default();
    ray_advance(arena, terrain, &mut ray, Some(&targets), &mut outcome, source_target::Context::default()).unwrap();
    assert_eq!(
        word(&ray, 154),
        fireball_slot,
        "a live typed Fireball remains the ray's bound target"
    );
    assert_eq!(
        ray.control_flags & 2,
        0,
        "a live target skips first-pass reacquisition"
    );

    put(&mut ray, 154, stale_slot);
    ray_advance(arena, terrain, &mut ray, Some(&targets), &mut outcome, source_target::Context::default()).unwrap();
    assert_eq!(word(&ray, 154), stale_slot, "an unused nonzero fixed address remains bound");
    assert_eq!(ray.control_flags & 2, 0);

    put(&mut ray, 154, reclaimed_slot);
    ray_advance(arena, terrain, &mut ray, Some(&targets), &mut outcome, source_target::Context::default()).unwrap();
    assert_eq!(word(&ray, 154), reclaimed_slot, "reclaim does not turn a fixed address into slot zero");
    assert_eq!(ray.control_flags & 2, 0);

    assert_eq!(arena.economy.allocate_slot(), Some(reclaimed_slot));
    {
        let replacement = &mut arena.entities[usize::from(reclaimed_slot)];
        replacement.active = true;
        replacement.class = 3;
        replacement.disable_drawing();
    }
    assert!(arena.handle(reclaimed_slot).is_some());
    ray_advance(arena, terrain, &mut ray, Some(&targets), &mut outcome, source_target::Context::default()).unwrap();
    assert_eq!(word(&ray, 154), reclaimed_slot, "a hidden replacement remains the same fixed address");
    assert_eq!(ray.control_flags & 2, 0);

    put(&mut ray, 154, 0);
    ray_advance(arena, terrain, &mut ray, Some(&targets), &mut outcome, source_target::Context::default()).unwrap();
    assert_eq!(word(&ray, 154), target_slot);
    assert_ne!(ray.control_flags & 2, 0);

    put(&mut ray, 154, 0);
    ray_advance(arena, terrain, &mut ray, Some(&targets), &mut outcome, source_target::Context::default()).unwrap();
    assert_eq!(
        word(&ray, 154),
        0,
        "sub_66610 does not retry acquisition after its first attempt"
    );
    put(&mut ray, 154, u16::MAX);
    assert!(ray_advance(arena, terrain, &mut ray, Some(&targets), &mut outcome, source_target::Context::default()).is_err());
}

#[test]
fn lightning_holder_produces_each_tier_and_failed_allocation_still_consumes_the_pulse() {
    for tier in 0..=2 {
        let (mut session, slot, parent) = ready_lightning(tier);
        let particles = session.session.definition.presentation.particles.clone();
        let live = &mut session.session.state.live;
        let rule = RULES.tiers[usize::from(tier)];
        let result = step_holder(
            &mut live.shared_spells,
            &mut live.economy,
            &mut live.terrain,
            &particles,
            slot,
            CastInput {
                castle_mana: Some(rule.castle_mana_required),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.clear_player_aux, Some(parent));
        let first = result
            .projectile
            .expect("successful tier launches a source projectile");
        let row = &live.economy.entities[usize::from(first)];
        let expected_model = if tier == 0 { 9 } else { 12 };
        assert_eq!(
            (row.class, row.model, row.action),
            (9, expected_model, expected_model)
        );
        assert_eq!((row.source_id, row.subspell_index), (parent, rule.damage));
        if tier == 2 {
            let second = word(row, 56);
            assert_ne!(second, 0, "tier two creates its linked second shot");
            assert_eq!(word(&live.economy.entities[usize::from(second)], 56), first);
        } else {
            assert_eq!(word(row, 56), 0);
        }
        let holder = live.shared_spells.pickup(slot).unwrap();
        assert_eq!(holder.active_value, holder.activation_value as i16 - 1);
    }

    let (mut session, slot, parent) = ready_lightning(0);
    let particles = session.session.definition.presentation.particles.clone();
    let live = &mut session.session.state.live;
    let holder = live.shared_spells.pickup(slot).unwrap();
    let activation = holder.activation_value;
    let max_mana = holder.max_mana;
    let initial_regen = live.economy.entities[usize::from(parent)].mana_regen;
    let expected_regen = if initial_regen >= 0 {
        max_mana.wrapping_neg()
    } else {
        initial_regen.wrapping_sub(max_mana)
    };
    exhaust(&mut live.economy);
    let result = step_holder(
        &mut live.shared_spells,
        &mut live.economy,
        &mut live.terrain,
        &particles,
        slot,
        CastInput {
            castle_mana: Some(RULES.tiers[0].castle_mana_required),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result, HolderOutcome::default());
    assert_eq!(
        live.economy.entities[usize::from(parent)].mana_regen,
        expected_regen
    );
    assert_eq!(
        live.shared_spells.pickup(slot).unwrap().active_value,
        activation as i16 - 1
    );
}

#[test]
fn authored_lightning_pickup_and_public_hand_reach_scheduled_tier_consumers() {
    // The ordinary Level 16 group-zero source is a persistent Lightning
    // pickup. Only the player's position and casting resources are staged;
    // acquisition, hand selection, targeting and projectiles are scheduled.
    let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level16");
    for tier in 0_u8..=2 {
        let mut campaign = crate::campaign::CampaignSession::open(
            &package,
            crate::campaign::spec::CampaignSelection::ordinary(16).unwrap(),
        ).expect("actual Level 16 owner package opens");
        let session = campaign.simulation_mut();
        let player = session.state.entity_index;
        let baseline_products = session.session.state.live.economy.entities.iter()
            .filter(|row| row.active && row.source_id == player
                && matches!((row.class, row.model), (9, 9 | 12) | (10, 23 | 38)))
            .map(|row| (row.index, row.class, row.model, row.action)).collect::<Vec<_>>();
        let source = session.session.state.live.economy.entities.iter()
            .find(|row| row.active && (row.class, row.model, row.action) == (15, 7, 23))
            .cloned().expect("actual persistent Lightning pickup");
        assert_eq!(source.index, 160);
        for _ in 0..12 {
            let position = session.session.state.live.economy.entities[usize::from(source.index)].position;
            session.set_level2_entity_position(player, position).unwrap();
            session.step_outer_turn(crate::PlayerCommand::default(), 0);
            assert!(session.dispatch_events().iter().all(|event|
                !matches!(event, super::super::Mc2DispatchEvent::Error { .. })),
                "pickup dispatch errors: {:?}", session.dispatch_events());
            if session.shared_spells().local_player().enabled_spell_holders[7] != 0 { break; }
        }
        let holder = session.shared_spells().local_player().enabled_spell_holders[7];
        assert_eq!(holder, source.index, "ordinary overlap acquires the authored pickup");
        assert_eq!(session.shared_spells().pickup(holder).unwrap().action, 21);

        let ui = crate::SOURCE_SPELL_INDEX_MAP.iter().position(|family| *family == 7).unwrap() as i8;
        session.step_outer_turn(crate::PlayerCommand {
            opcode: 31, argument_1: ui, argument_2: tier as i8,
            ..crate::PlayerCommand::default()
        }, 0);
        assert_eq!(session.shared_spells().pickup(holder).unwrap().level_gate, tier as i8,
            "ordinary hand selection applies the source tier");

        let rule = RULES.tiers[usize::from(tier)];
        let castle_position = session.state.position;
        let castle = session.economy_mut().build_castle(castle_position, player)
            .expect("real owner Castle constructor");
        assert!(session.economy_mut().set_castle_mana_prerequisite_for_test(
            castle, rule.castle_mana_required));
        {
            let live = &mut session.session.state.live;
            let parent = &mut live.economy.entities[usize::from(player)];
            parent.max_mana = rule.mana_cost + 1_000;
            parent.record[144..148].copy_from_slice(&parent.max_mana.to_le_bytes());
            parent.set_mana(rule.mana_cost + 1_000);
            parent.set_mana_regen(0);
            let parent = parent.clone();
            live.shared_spells.sync_player_entity_from_authority(&parent);
            live.shared_spells.player_mana = parent.mana;
            live.shared_spells.caster.mana = parent.mana;
            live.shared_spells.sync_typed_spell_parent_caches_from_player_entity();
            if let Some(fire) = live.core.context.spell_fire.as_mut() { fire.caster.mana = parent.mana; }
            live.economy.refresh_owner_mana_rows();
        }
        let target = session.session.state.live.economy.entities.iter()
            .find(|row| row.active && row.class == 5 && row.model == 9 && row.life > 0)
            .cloned().expect("actual Level 16 living target");
        let player_fov = session.session.state.live.economy.entities[usize::from(player)].array_fov();
        let position = crate::Position {
            x: target.position.x.wrapping_sub(1_024), y: target.position.y,
            z: target.position.z.wrapping_sub(player_fov),
        };
        session.set_level2_entity_position(player, position).unwrap();
        let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(position, target.position);
        session.set_level2_player_aim_for_test(yaw, pitch);
        let target_handle = session.session.state.live.economy.handle(target.index).unwrap();
        let experience_before = session.shared_spells().spell_experience()[7];
        let mut previous_mana = session.session.state.live.economy.entities[usize::from(player)].mana;
        let mut charged = false;
        let mut launch_sound = false;
        let mut endpoint_sound = false;
        let mut particles_seen = [false; 3];
        let mut projected_projectile = false;
        let mut projected_storm = false;
        // Track one observed incarnation of each stage, not all later rays
        // spawned by the storm. Generations distinguish reclaim from reuse.
        let mut tracked = [None; 4];
        let mut hidden = [false; 4];
        let mut reclaimed = [false; 4];
        let mut saw_ray = false;
        let mut saw_shot = false;
        let mut saw_storm = false;
        let mut saw_endpoint = false;
        let mut damaged = false;
        let mut storm_child_audio = false;
        let mut previous_target_life = target.life;
        let mut trace = Vec::new();
        for turn in 0..160 {
            let command = if turn == 0 { crate::PlayerCommand { flags: 0x10, ..Default::default() } }
                else { crate::PlayerCommand::default() };
            let audio_cursor = session.effect_journal().cursor(0).next_sequence;
            session.step_outer_turn(command, 0);
            let live = &session.session.state.live;
            for record in session.effect_journal().records().iter()
                .filter(|record| record.sequence >= audio_cursor)
            {
                if let crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                    audio_site: crate::MissionAudioSite::StormChild,
                    entity_index, player_index: -1, sound_id: 23,
                }) = record.effect {
                    assert_ne!(entity_index, 0);
                    assert_ne!(entity_index, player, "physical child emits23, not the caster");
                    let prepared = record.audio.expect("ordinary Storm child reaches prepared Flight audio");
                    assert_eq!(prepared.play_type, 1);
                    assert_eq!(prepared.command.sound_flags, player as i16);
                    assert_eq!(prepared.command.rate_delta_percent, 0);
                    storm_child_audio |= prepared.command.audible;
                }
            }
            let current_mana = live.economy.entities[usize::from(player)].mana;
            charged |= previous_mana - current_mana == rule.mana_cost;
            previous_mana = current_mana;
            for event in live.economy.economy.events() {
                if let super::super::Level2EconomyEvent::PreparedSound { entity_index, sound_id, .. } = event {
                    launch_sound |= *entity_index == player && *sound_id == if tier == 0 { 23 } else { 9 };
                    endpoint_sound |= *sound_id == 24
                        && live.economy.entities.get(usize::from(*entity_index)).is_some_and(|row|
                            row.active && row.source_id == player
                                && (row.class, row.model, row.action) == (10, 23, 23));
                }
            }
            for row in live.economy.entities.iter().filter(|row| row.active && row.source_id == player) {
                let stage = match (row.class, row.model, row.action) {
                    (9, 9, 9) => Some(0),
                    (9, 12, 12) => Some(1),
                    (10, 38, 40) => Some(2),
                    (10, 23, 23) => Some(3),
                    _ => None,
                };
                if let Some(stage) = stage {
                    tracked[stage].get_or_insert((row.index, live.economy.economy.slot_generation(row.index)));
                    let particle = match stage { 0 | 1 => 216, 2 => 272, _ => 7 };
                    if row.particle_parameter_index == Some(particle) {
                        particles_seen[if stage <= 1 { 0 } else { stage - 1 }] = true;
                    }
                }
            }
            let presentations = session.campaign_entity_presentations(0)
                .expect("Lightning products reach the campaign presentation projection");
            for presentation in &presentations {
                let row = &live.economy.entities[usize::from(presentation.entity_index)];
                if row.source_id != player { continue; }
                let expected = match (row.class, row.model, row.action) {
                    (9, 9, 9 | 14) | (9, 12, 12) => Some((216, false)),
                    (10, 38, 40) => Some((272, true)),
                    _ => None,
                };
                if let Some((particle, storm)) = expected {
                    assert_eq!(presentation.source_id, player);
                    assert_eq!(presentation.position, row.position);
                    assert_eq!(presentation.render_parameter_index, particle);
                    assert_eq!(presentation.base_tmap,
                        session.session.definition.presentation.particles[usize::from(particle)].base_tmap);
                    if storm { projected_storm = true; } else { projected_projectile = true; }
                }
            }
            for (stage, tracked_slot) in tracked.iter().enumerate() {
                if let Some((slot, generation)) = *tracked_slot {
                    let row = &live.economy.entities[usize::from(slot)];
                    hidden[stage] |= live.economy.economy.slot_generation(slot) == generation
                        && row.active && row.drawing_disabled();
                    reclaimed[stage] |= source_slot_cleanup_state(session, slot, generation).1;
                }
            }
            let relevant = live.economy.entities.iter().filter(|row| row.active && row.source_id == player
                && matches!((row.class, row.model), (9, 9 | 12) | (10, 23 | 38)))
                .map(|row| (row.index, row.class, row.model, row.action, row.life, row.drawing_disabled()))
                .collect::<Vec<_>>();
            saw_ray |= relevant.iter().any(|row| (row.1, row.2) == (9, 9));
            saw_shot |= relevant.iter().any(|row| (row.1, row.2) == (9, 12));
            saw_storm |= relevant.iter().any(|row| (row.1, row.2) == (10, 38));
            saw_endpoint |= relevant.iter().any(|row| (row.1, row.2) == (10, 23));
            if live.economy.handle(target.index) == Some(target_handle) {
                let target = &live.economy.entities[usize::from(target.index)];
                damaged |= target.life < previous_target_life && target.word_26() == player;
                previous_target_life = target.life;
            }
            let errors = session.dispatch_events().iter().filter(|event|
                matches!(event, super::super::Mc2DispatchEvent::Error { .. })).collect::<Vec<_>>();
            trace.push(format!("turn={turn} target_life={} relevant={relevant:?} errors={errors:?}",
                live.economy.entities[usize::from(target.index)].life));
            assert!(errors.is_empty(), "tier={tier}: {}", trace.join("\n"));
            let cleaned = reclaimed[0] && reclaimed[3]
                && (tier == 0 || reclaimed[1] && reclaimed[2]);
            if saw_ray && saw_endpoint && damaged && (tier == 0 || saw_shot && saw_storm)
                && charged && launch_sound && endpoint_sound && (tier == 0 || storm_child_audio) && cleaned
                && session.shared_spells().pickup(holder).unwrap().active_value == 0
                && session.shared_spells().spell_experience()[7] > experience_before { break; }
        }
        assert!(saw_ray && saw_endpoint && damaged && (tier == 0 || saw_shot && saw_storm),
            "ordinary tier {tier} must reach its scheduled Lightning damage consumer:\n{}", trace.join("\n"));
        assert!(charged, "tier {tier} must debit its source mana cost");
        assert!(launch_sound && endpoint_sound, "tier {tier} must prepare launch and endpoint sounds");
        assert!(tier == 0 || storm_child_audio,
            "ordinary tier{tier} first Storm child must reach audible prepared positional Flight audio");
        assert!(particles_seen[0] && particles_seen[2] && (tier == 0 || particles_seen[1]),
            "tier {tier} must bind the projectile, emitter, and endpoint presentation particles");
        assert!(projected_projectile && (tier == 0 || projected_storm),
            "tier {tier} visible projectile and emitter reach the campaign sprite consumer");
        assert!(session.shared_spells().spell_experience()[7] > experience_before,
            "tier {tier} damage must award Lightning experience");
        assert_eq!(session.shared_spells().pickup(holder).unwrap().active_value, 0,
            "tier {tier} holder must naturally finish its cast");
        for stage in if tier == 0 { &[0, 3][..] } else { &[0, 1, 2, 3][..] } {
            assert!(hidden[*stage] && reclaimed[*stage],
                "tier {tier} stage {stage} must hide and be reclaimed; tracked={tracked:?}");
        }
        campaign.reset();
        let reset = campaign.simulation();
        assert_eq!(reset.shared_spells().local_player().enabled_spell_holders[7], 0,
            "reset restores the pre-acquisition Lightning holder state");
        let reset_products = reset.session.state.live.economy.entities.iter()
            .filter(|row| row.active && row.source_id == player
                && matches!((row.class, row.model), (9, 9 | 12) | (10, 23 | 38)))
            .map(|row| (row.index, row.class, row.model, row.action)).collect::<Vec<_>>();
        assert_eq!(reset_products, baseline_products,
            "reset removes this cast's products and restores the source baseline");
    }
}

#[test]
fn lightning_missing_or_replaced_parent_only_advances_holder_timers() {
    for replaced in [false, true] {
        let (mut session, slot, parent_slot) = ready_lightning(0);
        let particles = session.session.definition.presentation.particles.clone();
        let live = &mut session.session.state.live;
        let holder = live.shared_spells.pickup_mut(slot).unwrap();
        let active_before = holder.active_value;
        put(holder, 58, 3);
        let parent = &mut live.economy.entities[usize::from(parent_slot)];
        if replaced {
            parent.class = 5;
            parent.record[67] = 5;
        } else {
            parent.active = false;
        }
        let regen_before = parent.mana_regen;
        let outcome = step_holder(
            &mut live.shared_spells,
            &mut live.economy,
            &mut live.terrain,
            &particles,
            slot,
            CastInput {
                castle_mana: Some(i32::MAX),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(outcome.projectile.is_none());
        assert_eq!(
            live.economy.entities[usize::from(parent_slot)].mana_regen,
            regen_before
        );
        let holder = live.shared_spells.pickup(slot).unwrap();
        assert_eq!(holder.active_value, active_before - 1);
        assert_eq!(word(holder, 58), 2);
    }
}

#[test]
fn lightning_storm_shot_attributes_accuracy_and_xp_only_to_player_owners() {
    for is_player in [true, false] {
        let (mut session, _, parent) = ready_lightning(1);
        let particles = session.session.definition.presentation.particles.clone();
        let live = &mut session.session.state.live;
        let arena = &mut live.economy;
        let target = arena.entities.iter().filter(|row| row.active && row.index != parent)
            .find_map(|row| {
                let view = source_target::read(arena, source_target::Context::default(), row.index).ok()??;
                (view.control_flags & 8 != 0 && view.control_flags & 0x8010 == 0
                    && view.source_id != parent && view.collision.is_some())
                    .then_some(view)
            }).expect("campaign fixture has an unshielded collidable target");
        let target_slot = target.slot;
        let shape = target.collision.unwrap();
        let position = guided_projectile::centre(target.position, target.model, shape.vertical_offset);
        let shot = construct(arena, &mut live.terrain, &particles, Product::StormShot, position)
            .unwrap().unwrap();
        {
            let row = &mut arena.entities[usize::from(shot)];
            owner(row, parent);
            row.set_actual_speed(0);
            put(row, 136, 0);
            put(row, 154, target_slot);
            row.record[69] = target.class;
            row.record[70] = target.model;
        }
        if !is_player {
            let row = &mut arena.entities[usize::from(parent)];
            row.class = 10;
            row.model = 57;
            row.record[67] = 10;
            row.record[68] = 57;
        }
        let old_parent_target = word(&arena.entities[usize::from(parent)], 154);
        let outcome = step(arena, &mut live.terrain, &particles, shot, None,
            source_target::Context::default(), &mut |_, _| {}).unwrap();
        assert!(outcome.impact_slot.is_some(), "collision must produce the storm emitter");
        assert_eq!(outcome.experience, is_player.then_some((parent, 7, 1)));
        assert_eq!(outcome.accuracy, is_player.then_some((parent, true)));
        assert_eq!(word(&arena.entities[usize::from(parent)], 154), old_parent_target,
            "66FD0 does not call the guided projectile's owner-target update");
    }
}

#[test]
fn lightning_storm_emitter_uses_one_rng_draw_and_produces_opposite_source_rays() {
    let (mut session, _, parent) = ready_lightning(0);
    let particles = session.session.definition.presentation.particles.clone();
    let live = &mut session.session.state.live;
    let arena = &mut live.economy;
    let base = arena.entities[usize::from(parent)].position;
    let position = Position {
        z: live.terrain.ground_height(base).wrapping_add(1024),
        ..base
    };
    let emitter = construct(
        arena,
        &mut live.terrain,
        &particles,
        Product::Storm,
        position,
    )
    .unwrap()
    .unwrap();
    // This configured emitter represents66FD0's actual9/9 child selector.
    // Bare4FFB0 retains NewEvent's10/0 and is exercised separately below.
    arena.entities[usize::from(emitter)].record[71] = 9;
    arena.entities[usize::from(emitter)].record[72] = 9;
    owner(&mut arena.entities[usize::from(emitter)], parent);
    arena.entities[usize::from(emitter)].set_life(4);
    let seed = 0x1234;
    arena.entities[usize::from(emitter)].set_entity_rng(seed);
    let before = arena
        .entities
        .iter()
        .filter(|row| row.active)
        .map(|row| row.index)
        .collect::<BTreeSet<_>>();
    let tile = arena.economy.map_chains.tile_of(emitter);
    let previous_head = arena.economy.map_chains.tile_head(tile);
    assert_eq!(
        previous_head, emitter,
        "the new controller is the initial tile head"
    );

    let mut calls = Vec::new();
    let outcome = step(
        arena,
        &mut live.terrain,
        &particles,
        emitter,
        None,
        source_target::Context::default(),
        &mut |child, sound| calls.push((child.clone(), sound)),
    )
    .unwrap();
    let expected_rng = seed.wrapping_mul(9377).wrapping_add(9439);
    assert_eq!(
        arena.entities[usize::from(emitter)].entity_rng,
        expected_rng
    );
    assert_eq!(
        arena.economy.source_rng.get(&emitter).copied(),
        Some(expected_rng)
    );
    assert_eq!(arena.entities[usize::from(emitter)].life, 3);
    assert_eq!(outcome.sounds.len(), 1);
    let mut rays = arena
        .entities
        .iter()
        .filter(|row| {
            row.active
                && !before.contains(&row.index)
                && (row.class, row.model, row.action) == (9, 9, 9)
                && row.source_id == parent
        })
        .collect::<Vec<_>>();
    rays.sort_unstable_by_key(|row| row.index);
    assert_eq!(rays.len(), 2);
    assert_eq!(rays[0].yaw.wrapping_add(1024) & 2047, rays[1].yaw);
    assert_eq!((rays[0].life, rays[1].life), (3, 3));
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].1, 23);
    assert_eq!(calls[0].0.index, outcome.sounds[0].0);
    let first = rays
        .iter()
        .find(|ray| ray.index == calls[0].0.index)
        .unwrap();
    let second = rays.iter().find(|ray| ray.index != first.index).unwrap();
    // AddEventToMap57D70 gives each new head next0 and old=previous head,
    // then links the previous head forward to it. The sound precedes the
    // second insertion, so its snapshot must retain the earlier topology.
    assert_eq!(
        (word(&calls[0].0, 26), word(&calls[0].0, 28)),
        (previous_head, 0)
    );
    assert_eq!(
        (word(first, 26), word(first, 28)),
        (previous_head, second.index)
    );
    assert_eq!((word(second, 26), word(second, 28)), (first.index, 0));
    assert_eq!(
        word(&arena.entities[usize::from(previous_head)], 28),
        first.index
    );
    assert_eq!(arena.economy.map_chains.tile_head(tile), second.index);
    assert_eq!(
        live.terrain.planes().entity_indices[usize::from(tile)] as u16,
        second.index
    );
    for (row, previous, next) in [
        (*first, previous_head, second.index),
        (*second, first.index, 0),
    ] {
        assert_eq!(arena.economy.map_chains.old_map_entity(row.index), previous);
        assert_eq!(arena.economy.map_chains.next_entity(row.index), next);
    }
    let mut expected_final = calls[0].0.clone();
    expected_final.record[28..30].copy_from_slice(&second.index.to_le_bytes());
    assert_eq!(
        &expected_final, *first,
        "only the independently verified second-birth map link changes after sound23"
    );
}

#[test]
fn bare_storm_default_children_reach_effect_presentation_and_natural_reclaim() {
    let (mut session, _, parent) = ready_lightning(0);
    let particles = session.session.definition.presentation.particles.clone();
    // Explicit bare-constructor control, not an ordinary Lightning cast:
    // choose its already-settled source height; keep default life32 and RNG.
    let (emitter, emitter_handle, children) = {
        let live = &mut session.session.state.live;
        let base = live.economy.entities[usize::from(parent)].position;
        let position = Position {
            z: live.terrain.ground_height(base).wrapping_add(1024),
            ..base
        };
        let emitter = crate::world::effects::allocate(
            &mut live.economy,
            &mut live.terrain,
            sim_definition::adapter::mc2_effect_definition(
                crate::level_definition::EffectKind::LightningStorm,
            ),
            position,
            &particles,
        )
        .unwrap()
        .unwrap();
        let emitter_handle = live.economy.handle(emitter).unwrap();
        let source = live.economy.entities[usize::from(emitter)].clone();
        assert_eq!(
            (source.record[71], source.record[72], source.life),
            (10, 0, 32)
        );
        assert_eq!(
            source.subspell_index, 100,
            "semantic source default reaches the real constructor"
        );
        let before = live
            .economy
            .entities
            .iter()
            .filter(|row| row.active)
            .map(|row| (row.index, live.economy.slot_generation(row.index)))
            .collect::<BTreeSet<_>>();
        let outcome = step(
            &mut live.economy,
            &mut live.terrain,
            &particles,
            emitter,
            None,
            source_target::Context::default(),
            &mut |_, _| {},
        )
        .unwrap();
        let expected_rng = source.entity_rng.wrapping_mul(9377).wrapping_add(9439);
        let current = &live.economy.entities[usize::from(emitter)];
        assert_eq!((current.life, current.entity_rng), (31, expected_rng));
        assert_eq!(
            live.terrain.source_predicted_position(),
            Position {
                z: position.z.wrapping_add(source.array_yaw()),
                ..position
            }
        );
        let children = live
            .economy
            .entities
            .iter()
            .filter(|row| {
                row.active
                    && !before.contains(&(row.index, live.economy.slot_generation(row.index)))
            })
            .map(|row| live.economy.handle(row.index).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(children.len(), 2);
        assert_eq!(outcome.sounds.len(), 1);
        assert_eq!(outcome.sounds[0].1, 23);
        assert!(
            children
                .iter()
                .any(|handle| handle.source_slot == outcome.sounds[0].0)
        );
        for child in &children {
            let row = &live.economy.entities[usize::from(child.source_slot)];
            assert_eq!(
                (row.class, row.model, row.action, row.life, row.max_life),
                (10, 0, 0, 2, 8)
            );
            assert_eq!(
                (row.source_id, row.subspell_index, row.pitch),
                (source.source_id, source.subspell_index, 56)
            );
            assert_eq!(
                row.position, position,
                "constructor receives the source position address"
            );
            assert_eq!((row.record[71], row.record[72]), (10, 23));
        }
        let first = &live.economy.entities[usize::from(outcome.sounds[0].0)];
        let second = &live.economy.entities[usize::from(
            children
                .iter()
                .find(|handle| handle.source_slot != first.index)
                .unwrap()
                .source_slot,
        )];
        assert_eq!(first.yaw.wrapping_add(1024) & 2047, second.yaw);
        (emitter, emitter_handle, children)
    };
    let presentations = session.campaign_entity_presentations(0).unwrap();
    for child in &children {
        let view = presentations
            .iter()
            .find(|view| view.entity_index == child.source_slot)
            .expect("actual10/0 child reaches original effect presentation");
        assert_eq!(view.render_parameter_index, 7);
        assert_eq!(view.base_tmap, particles[7].base_tmap);
        assert_eq!(view.source_id, emitter);
    }
    let tracked = [emitter_handle, children[0], children[1]];
    let mut hidden = [false; 3];
    let mut reclaimed = [false; 3];
    // Default32 emits through oldlife0, hides on oldlife−1, then reclaims.
    // First children retain their actual shortened constructor life2.
    for turn in 0..40 {
        session.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, super::super::Mc2DispatchEvent::Error { .. })),
            "bare storm lifecycle turn{turn}: {:?}",
            session.dispatch_events()
        );
        let views = session.campaign_entity_presentations(0).unwrap();
        let arena = &session.session.state.live.economy;
        for (index, handle) in tracked.iter().enumerate() {
            if arena.handle(handle.source_slot) == Some(*handle) {
                if arena.entities[usize::from(handle.source_slot)].drawing_disabled() {
                    hidden[index] = true;
                    assert!(
                        !views
                            .iter()
                            .any(|view| view.entity_index == handle.source_slot)
                    );
                }
            } else {
                assert!(
                    hidden[index],
                    "original generation must hide before following reclaim"
                );
                reclaimed[index] = true;
            }
        }
        if reclaimed.iter().all(|value| *value) {
            break;
        }
    }
    assert_eq!((hidden, reclaimed), ([true; 3], [true; 3]));
}

#[test]
fn configured_storm_selected_children_preserve_source_constructors_and_failures() {
    for (selector, capacity) in [
        ((10, 20), None),
        ((10, 21), None),
        ((10, 24), None),
        ((10, 37), None),
        ((10, 0), Some(0)),
        ((10, 0), Some(1)),
        ((10, 33), None),
        ((10, 35), None),
    ] {
        let (mut session, _, parent) = ready_lightning(0);
        let particles = session.session.definition.presentation.particles.clone();
        let live = &mut session.session.state.live;
        let base = live.economy.entities[usize::from(parent)].position;
        let position = Position {
            z: live.terrain.ground_height(base).wrapping_add(1024),
            ..base
        };
        let emitter = construct(
            &mut live.economy,
            &mut live.terrain,
            &particles,
            Product::Storm,
            position,
        )
        .unwrap()
        .unwrap();
        let source = &mut live.economy.entities[usize::from(emitter)];
        source.record[71] = selector.0;
        source.record[72] = selector.1;
        let seed = source.entity_rng;
        if let Some(capacity) = capacity {
            // Explicit capacity control; no source actor is made replaceable.
            let free = *live.economy.economy.free_source_slots().last().unwrap();
            exhaust(&mut live.economy);
            if capacity == 1 {
                live.economy.economy.release_slot(free, None);
            }
        }
        let before = live
            .economy
            .entities
            .iter()
            .map(|row| (row.index, live.economy.slot_generation(row.index)))
            .collect::<Vec<_>>();
        let global_rng = live.economy.economy.global_rng;
        let free = live.economy.economy.free_slot_count();
        let mut calls = Vec::new();
        let outcome = step(
            &mut live.economy,
            &mut live.terrain,
            &particles,
            emitter,
            None,
            source_target::Context::default(),
            &mut |child, sound| calls.push((child.clone(), sound)),
        )
        .unwrap();
        let source = &live.economy.entities[usize::from(emitter)];
        assert_eq!(source.life, 31);
        assert_eq!(
            source.entity_rng,
            seed.wrapping_mul(9377).wrapping_add(9439)
        );
        assert_eq!(source.yaw, (source.entity_rng as i16) & 2047);
        if matches!(selector, (10, 33 | 35)) || capacity == Some(1) {
            // Existing canonical source constructors, configured here only.
            // Selector35 is an allocated blank, not a stamped transport actor.
            let children = live
                .economy
                .entities
                .iter()
                .filter(|row| {
                    before[usize::from(row.index)].1 != live.economy.slot_generation(row.index)
                })
                .collect::<Vec<_>>();
            assert_eq!(children.len(), if capacity == Some(1) { 1 } else { 2 });
            for child in &children {
                assert_eq!(
                    (child.class, child.model, child.action, child.life),
                    match selector.1 {
                        33 => (10, 33, 35, 100),
                        35 => (0, 0, 0, 0),
                        _ => (10, 0, 0, 2),
                    }
                );
                assert_eq!(child.source_id, source.source_id);
                assert_eq!((child.record[71], child.record[72]), (10, 23));
                assert_eq!(
                    live.economy.economy.map_chains.is_linked(child.index),
                    selector.1 == 0
                );
            }
            assert_eq!(outcome.sounds.len(), 1);
            assert_eq!(outcome.sounds[0].1, 23);
            assert_eq!(calls.len(), 1);
            assert_eq!((calls[0].0.index, calls[0].1), outcome.sounds[0]);
            assert!(
                children
                    .iter()
                    .any(|child| child.index == outcome.sounds[0].0)
            );
            if capacity == Some(1) {
                assert_eq!(
                    live.economy.economy.free_slot_count(),
                    0,
                    "the actual first product survives the second failed attempt"
                );
            }
            continue;
        }
        assert!(
            outcome.sounds.is_empty(),
            "failed first construction has no sound23"
        );
        assert!(calls.is_empty());
        assert_eq!(live.economy.economy.free_slot_count(), free);
        assert_eq!(live.economy.economy.global_rng, global_rng);
        assert!(
            before
                .iter()
                .all(|&(slot, generation)| live.economy.slot_generation(slot) == generation)
        );
        assert_eq!(
            live.terrain.source_predicted_position(),
            Position {
                z: position.z.wrapping_add(source.array_yaw()),
                ..position
            }
        );
    }
}

#[test]
fn lightning_ray_keeps_segment_rng_and_trails_reach_the_hidden_state() {
    let (mut session, _, parent) = ready_lightning(0);
    let particles = session.session.definition.presentation.particles.clone();
    let live = &mut session.session.state.live;
    let arena = &mut live.economy;
    for actor in &mut arena.entities {
        if (actor.class == 3 || actor.class == 5) && actor.index != parent {
            actor.set_life(-1);
            actor.set_control_flags(actor.control_flags | ENTITY_DRAW_DISABLED_BIT);
        }
        actor.set_control_flags(
            actor.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
        );
    }
    let base = arena.entities[usize::from(parent)].position;
    let position = Position {
        z: live.terrain.ground_height(base).wrapping_add(4096),
        ..base
    };
    let slot = construct(arena, &mut live.terrain, &particles, Product::Ray, position)
        .unwrap()
        .unwrap();
    owner(&mut arena.entities[usize::from(slot)], parent);
    let seed = 0x3210;
    {
        let row = &mut arena.entities[usize::from(slot)];
        row.set_entity_rng(seed);
        row.set_life(0);
        row.set_yaw(0);
        row.set_pitch(0);
    }
    let expected_rng = expected_one_step_ray_rng(seed);
    let outcome = step(arena, &mut live.terrain, &particles, slot, None, source_target::Context::default(), &mut |_, _| {}).unwrap();
    assert_eq!(outcome.accuracy, Some((parent, false)));
    assert!(outcome.impact_slot.is_some());
    assert_eq!(arena.entities[usize::from(slot)].entity_rng, expected_rng);
    assert_eq!(
        arena.economy.source_rng.get(&slot).copied(),
        Some(expected_rng)
    );

    let trail = arena
        .entities
        .iter()
        .find(|row| {
            row.active
                && row.source_id == parent
                && (row.class, row.model, row.action) == (9, 9, 14)
        })
        .expect("a real source ray allocates visual trail rows")
        .index;
    for _ in 0..=2 {
        if arena.entities[usize::from(trail)].drawing_disabled() {
            break;
        }
        step(arena, &mut live.terrain, &particles, trail, None, source_target::Context::default(), &mut |_, _| {}).unwrap();
    }
    assert!(arena.entities[usize::from(trail)].drawing_disabled());
    assert!(
        arena.entities[usize::from(trail)].active,
        "common following-pass reclaim owns row release"
    );
}

#[test]
fn lightning_endpoint_submits_one_damage_pulse_then_disables() {
    let (mut session, _, parent) = ready_lightning(0);
    let particles = session.session.definition.presentation.particles.clone();
    let live = &mut session.session.state.live;
    let position = live.economy.entities[usize::from(parent)].position;
    let endpoint = construct(
        &mut live.economy,
        &mut live.terrain,
        &particles,
        Product::Endpoint,
        position,
    )
    .unwrap()
    .unwrap();
    let row = &mut live.economy.entities[usize::from(endpoint)];
    owner(row, parent);
    row.set_subspell_index(75);
    let expected = DamagePulse {
        position: row.position,
        owner: parent,
        payload: row.subspell_index,
        half_width: row.array_pitch(),
        vertical_offset: row.array_yaw(),
        half_height: row.array_fov(),
        continuous: false,
        selector: (row.record[69], row.record[70]),
    };
    let mut pulses = Vec::new();
    let first = step_endpoint(&mut live.economy, endpoint, |_, pulse| {
        pulses.push(pulse);
        Ok(1)
    })
    .unwrap();
    assert_eq!(pulses, vec![expected]);
    assert_eq!(first.experience, Some((parent, 7, 1)));
    assert_eq!(first.sounds, vec![(endpoint, 24)]);

    let mut repeated = false;
    step_endpoint(&mut live.economy, endpoint, |_, _| {
        repeated = true;
        Ok(1)
    })
    .unwrap();
    assert!(!repeated, "the endpoint is a one-shot damage producer");
    step_endpoint(&mut live.economy, endpoint, |_, _| Ok(0)).unwrap();
    let third = &live.economy.entities[usize::from(endpoint)];
    assert_eq!(third.life, -1);
    assert!(
        !third.drawing_disabled(),
        "old life zero still runs visibly"
    );
    step_endpoint(&mut live.economy, endpoint, |_, _| Ok(0)).unwrap();
    let fourth = &live.economy.entities[usize::from(endpoint)];
    assert_eq!(fourth.life, -2);
    assert!(
        fourth.drawing_disabled(),
        "old life -1 hides on the fourth call"
    );
    assert!(live.economy.entities[usize::from(endpoint)].active);
}

fn source_slot_cleanup_state(
    session: &super::super::Mc2Simulation,
    slot: u16,
    generation: u32,
) -> (bool, bool) {
    let live = &session.session.state.live;
    let economy = &live.economy.economy;
    let reclaimed =
        economy.free_source_slots().contains(&slot) || economy.slot_generation(slot) > generation;
    let hidden = reclaimed
        || live
            .economy
            .entities
            .get(usize::from(slot))
            .is_some_and(|row| row.active && row.drawing_disabled());
    (hidden, reclaimed)
}

fn source_entity_damage_mailbox(
    session: &super::super::Mc2Simulation,
    target_slot: u16,
    attacker_slot: u16,
) -> Option<i32> {
    let row = session
        .session
        .state
        .live
        .economy
        .entities
        .get(usize::from(target_slot))?;
    let damage = i32::from_le_bytes(row.record[98..102].try_into().ok()?);
    let attacker = u16::from_le_bytes(row.record[102..104].try_into().ok()?);
    (damage > 0 && attacker == attacker_slot).then_some(damage)
}

#[test]
fn family22_lightning_fire_keeps_its_bound_target_and_damages_through_the_endpoint_consumer() {
    let mut session = super::super::tests::level_two_campaign_session();
    // The source initializes rival Castle protection to 100 visits. Let the
    // ordinary scheduler expire it before expecting a Lightning hit to hurt.
    let target_slot = session.session.state.live.economy.entities.iter()
        .find(|row| row.active && row.class == 3 && row.model == 1 && row.action == 1)
        .expect("qualified campaign fixture has its live rival Nyphur").index;
    let target_handle = session.session.state.live.economy.handle(target_slot)
        .expect("rival has a current generation-bound handle");
    for _ in 0..128 {
        let live = &session.session.state.live;
        let row = &live.economy.entities[usize::from(target_slot)];
        assert_eq!(live.economy.handle(target_slot), Some(target_handle));
        let rival = live.nyphur_dispatcher.get(target_handle)
            .expect("rival retains its source carrier during protection expiry");
        if rival.common_upkeep(row.phase).word_0x159 == 0 {
            break;
        }
        session.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().all(|event|
            !matches!(event, super::super::Mc2DispatchEvent::Error { .. })));
    }
    {
        let live = &session.session.state.live;
        let row = &live.economy.entities[usize::from(target_slot)];
        assert_eq!(live.economy.handle(target_slot), Some(target_handle));
        let rival = live.nyphur_dispatcher.get(target_handle)
            .expect("rival retains its source carrier after protection expiry");
        assert_eq!(rival.common_upkeep(row.phase).word_0x159, 0,
            "ordinary visits must expire protection outside the rival Castle");
    }
    let live = &mut session.session.state.live;
    let parent = live.core.state.entity_index;
    let sphere_damage = crate::spells::late::fools_mana::SOURCE_RULES[2].projectile_damage;
    let player = &mut live.economy.entities[usize::from(parent)];
    player.class = 3;
    player.model = 0;
    player.record[67] = 3;
    player.record[68] = 0;

    let target = live
        .economy
        .entities
        .iter()
        .find(|row| row.index == target_slot && row.active && row.class == 3 && row.model == 1 && row.action == 1)
        .cloned()
        .expect("qualified campaign fixture has its live rival Nyphur");
    assert_ne!(
        target.source_id, parent,
        "retaliation targets another owner"
    );
    assert!(target.life >= 0 && target.control_flags & 8 != 0);
    let target_slot = target.index;
    let target_position = Position {
        z: target.position.z.wrapping_add(target.array_yaw()),
        ..target.position
    };
    let sphere_position = Position {
        x: target_position
            .x
            .wrapping_sub(u16::try_from(RULES.projectile_speed).expect("positive ray speed")),
        ..target_position
    };
    let sphere_slot = live
        .economy
        .allocate_source_slot_with_terrain(&mut live.terrain)
        .expect("source sphere slot");
    let mut sphere = super::super::cave_features::new_event(
        sphere_slot,
        live.economy.economy.new_event_rng(sphere_slot),
        sphere_position,
    );
    sphere.set_position(Position {
        z: target_position.z.wrapping_sub(sphere.array_fov()),
        ..sphere_position
    });
    let sphere_position = sphere.position;
    sphere.class = 10;
    sphere.model = 57;
    sphere.record[67] = 10;
    sphere.record[68] = 57;
    sphere.record[71] = 10;
    sphere.record[72] = 1;
    sphere.set_source_action(62);
    sphere.set_max_life(300);
    sphere.set_life(300);
    sphere.record[61] = 0;
    put(&mut sphere, 44, parent);
    put(&mut sphere, 108, target_slot);
    sphere.record[104..108].fill(0);
    sphere.record[20..24].copy_from_slice(&0_i32.to_le_bytes());
    sphere.level_gate = 2;
    sphere.record[74] = 2;
    sphere.set_subspell_index(sphere_damage);
    live.economy.entities[usize::from(sphere_slot)] = sphere;
    crate::world::effects::sync_position(
        &mut live.economy,
        &mut live.terrain,
        sphere_slot,
        sphere_position,
        true,
        false,
    )
    .unwrap();
    let sphere_generation = live.economy.economy.slot_generation(sphere_slot);
    let sphere_phase_before = live.economy.entities[usize::from(sphere_slot)].phase;
    let target_life_before = live.economy.entities[usize::from(target_slot)].life;
    let family7_experience_before = live.shared_spells.spell_experience[7];
    let family22_experience_before = live.shared_spells.spell_experience[22];
    let target_selector = (target.class, target.model);

    // Run the ordinary source scheduler: it dispatches the sphere, same-pass
    // Lightning child and endpoint when their slots are ahead of the cursor.
    session.step_outer_turn(crate::PlayerCommand::default(), 0);
    let live = &session.session.state.live;
    let mut target_damage_queued =
        source_entity_damage_mailbox(&session, target_slot, sphere_slot).is_some();
    let mut target_damage_callback_seen = target_damage_queued;
    let mut target_life_after_damage = (live.economy.entities[usize::from(target_slot)].life
        < target_life_before)
        .then_some(live.economy.entities[usize::from(target_slot)].life);
    assert_eq!(
        live.economy.entities[usize::from(sphere_slot)].phase,
        sphere_phase_before.wrapping_add(1),
        "the sphere handler receives its source same-pass phase tail"
    );
    let ray = live
        .economy
        .entities
        .iter()
        .find(|row| {
            row.active
                && row.source_id == sphere_slot
                && (row.class, row.model, row.action) == (9, 9, 9)
        })
        .expect("scheduled tier-two sphere creates its LightningFire ray")
        .index;
    let ray_row = &live.economy.entities[usize::from(ray)];
    assert_eq!((ray_row.record[69], ray_row.record[70]), target_selector);
    assert_eq!((ray_row.record[71], ray_row.record[72]), (10, 23));
    assert_eq!(ray_row.source_id, sphere_slot);
    assert_eq!(ray_row.source_word_96(), target_slot);
    assert_eq!(ray_row.subspell_index, sphere_damage);
    assert_eq!(ray_row.parameter_relation, Some(64));
    assert_eq!(
        u32::from_le_bytes(ray_row.record[164..168].try_into().unwrap()),
        65
    );
    assert_eq!(
        guided_projectile::source_accuracy(
            &live.economy,
            ray_row.model,
            ray_row.source_id,
            Some(target_slot),
            target_slot,
        ),
        None,
        "a sphere owner is not a player accuracy row"
    );
    let ray_generation = live.economy.economy.slot_generation(ray);
    let ray_initial_phase = ray as u8;

    let mut endpoint_identity = live
        .economy
        .entities
        .iter()
        .find(|row| {
            row.active
                && row.source_id == sphere_slot
                && (row.class, row.model, row.action) == (10, 23, 23)
                && word(row, 154) == target_slot
        })
        .map(|row| {
            (
                row.index,
                live.economy.economy.slot_generation(row.index),
                row.index as u8,
            )
        });
    let mut endpoint_damage_phase_checked = false;
    if target_damage_callback_seen
        && let Some((endpoint, generation, initial_phase)) = endpoint_identity
        && live.economy.economy.slot_generation(endpoint) == generation
    {
        assert_eq!(
            live.economy.entities[usize::from(endpoint)].phase,
            initial_phase.wrapping_add(1),
            "the endpoint receives one scheduler epilogue after its real damage callback"
        );
        endpoint_damage_phase_checked = true;
    }

    let (mut sphere_hidden, mut sphere_reclaimed) =
        source_slot_cleanup_state(&session, sphere_slot, sphere_generation);
    let ray_row = &live.economy.entities[usize::from(ray)];
    let mut ray_hidden = ray_row.active && ray_row.drawing_disabled();
    let mut ray_reclaimed = false;
    if ray_hidden {
        assert_eq!(
            ray_row.phase,
            ray_initial_phase.wrapping_add(1),
            "the hidden ray receives one scheduler epilogue before reclaim"
        );
    }
    let (mut endpoint_hidden, mut endpoint_reclaimed) = endpoint_identity
        .map(|(endpoint, generation, _)| source_slot_cleanup_state(&session, endpoint, generation))
        .unwrap_or((false, false));

    let mut trace = Vec::new();
    for visit in 0..8 {
        let endpoint_before = endpoint_identity.and_then(|(endpoint, generation, _)| {
            let live = &session.session.state.live;
            let row = live.economy.entities.get(usize::from(endpoint))?;
            (live.economy.economy.slot_generation(endpoint) == generation
                && row.active
                && !row.drawing_disabled())
                .then_some((endpoint, generation, row.phase))
        });
        session.step_outer_turn(crate::PlayerCommand::default(), 0);
        let live = &session.session.state.live;
        if let Some((endpoint, generation, previous_phase)) = endpoint_before
            && live.economy.economy.slot_generation(endpoint) == generation
            && let Some(row) = live.economy.entities.get(usize::from(endpoint))
            && row.active
        {
            assert_eq!(row.phase, previous_phase.wrapping_add(1),
                "each subsequent live endpoint visit receives one scheduler epilogue");
        }
        let ray_row = &live.economy.entities[usize::from(ray)];
        let target_row = &live.economy.entities[usize::from(target_slot)];
        let errors = session
            .dispatch_events()
            .iter()
            .filter(|event| matches!(event, super::super::Mc2DispatchEvent::Error { .. }))
            .collect::<Vec<_>>();
        let endpoints = live
            .economy
            .entities
            .iter()
            .filter(|row| {
                row.active
                    && row.source_id == sphere_slot
                    && (row.class, row.model, row.action) == (10, 23, 23)
            })
            .map(|row| {
                (
                    row.index,
                    word(row, 154),
                    row.position,
                    row.life,
                    row.drawing_disabled(),
                )
            })
            .collect::<Vec<_>>();
        trace.push(format!(
            "visit={visit} errors={errors:?} ray=(slot={ray}, generation={}/{ray_generation}, active={}, class/model/action={}/{}/{}, life={}, flags={:#x}, position={:?}, target={}) target=(active={}, life={}, flags={:#x}, position={:?}) endpoints={endpoints:?}",
            live.economy.economy.slot_generation(ray), ray_row.active, ray_row.class,
            ray_row.model, ray_row.action, ray_row.life, ray_row.control_flags, ray_row.position,
            word(ray_row, 154), target_row.active, target_row.life, target_row.control_flags,
            target_row.position,
        ));
        if endpoint_identity.is_none()
            && let Some(row) = live.economy.entities.iter().find(|row| {
                row.active
                    && row.source_id == sphere_slot
                    && (row.class, row.model, row.action) == (10, 23, 23)
                    && word(row, 154) == target_slot
            })
        {
            endpoint_identity = Some((
                row.index,
                live.economy.economy.slot_generation(row.index),
                row.index as u8,
            ));
        }

        target_damage_queued |=
            source_entity_damage_mailbox(&session, target_slot, sphere_slot).is_some();
        target_damage_callback_seen |= target_damage_queued;
        let target_life = live.economy.entities[usize::from(target_slot)].life;
        if target_life < target_life_before {
            target_life_after_damage.get_or_insert(target_life);
        }

        let (hidden, reclaimed) =
            source_slot_cleanup_state(&session, sphere_slot, sphere_generation);
        sphere_hidden |= hidden;
        sphere_reclaimed |= reclaimed;
        let (hidden, reclaimed) = source_slot_cleanup_state(&session, ray, ray_generation);
        ray_hidden |= hidden;
        ray_reclaimed |= reclaimed;
        if live.economy.economy.slot_generation(ray) == ray_generation
            && let Some(row) = live.economy.entities.get(usize::from(ray))
            && row.active
            && row.drawing_disabled()
        {
            assert_eq!(
                row.phase,
                ray_initial_phase.wrapping_add(1),
                "the hidden ray receives one scheduler epilogue before reclaim"
            );
        }
        if let Some((endpoint, generation, _)) = endpoint_identity {
            let (hidden, reclaimed) = source_slot_cleanup_state(&session, endpoint, generation);
            endpoint_hidden |= hidden;
            endpoint_reclaimed |= reclaimed;
        }

        if target_damage_callback_seen && !endpoint_damage_phase_checked
            && let Some((endpoint, generation, initial_phase)) = endpoint_identity
            && live.economy.economy.slot_generation(endpoint) == generation
        {
            assert_eq!(
                live.economy.entities[usize::from(endpoint)].phase,
                initial_phase.wrapping_add(1),
                "the endpoint receives one scheduler epilogue after its real damage callback"
            );
            endpoint_damage_phase_checked = true;
        }

        if endpoint_identity.is_some()
            && target_damage_queued
            && target_life_after_damage.is_some()
            && sphere_reclaimed
            && ray_reclaimed
            && endpoint_reclaimed
        {
            break;
        }
    }

    let _endpoint_identity = endpoint_identity.unwrap_or_else(|| {
        panic!(
            "scheduled Lightning ray produces a bound-target endpoint; trace:\n{}",
            trace.join("\n")
        )
    });
    assert!(endpoint_damage_phase_checked, "the first damage callback's phase was checked");
    let live = &session.session.state.live;
    assert!(
        target_damage_queued,
        "the real LightningFire endpoint queues damage for the rival Nyphur source mailbox"
    );
    assert!(
        sphere_hidden && sphere_reclaimed,
        "consumed sphere hides then reclaims at the following boundary"
    );
    assert!(
        ray_hidden && ray_reclaimed,
        "Lightning ray hides then reclaims at the following boundary"
    );
    assert!(
        endpoint_hidden && endpoint_reclaimed,
        "endpoint hides then reclaims at the following boundary"
    );
    assert_eq!(
        live.shared_spells.spell_experience[7], family7_experience_before,
        "sphere-owned ray and endpoint award no player Lightning XP"
    );
    assert_eq!(
        live.shared_spells.spell_experience[22],
        family22_experience_before.wrapping_add(1),
        "the owning sphere's ordinary terminal callback still awards family22 XP"
    );
    assert!(
        target_life_after_damage.is_some_and(|life| life < target_life_before),
        "the next Nyphur scheduler pass consumes the endpoint mailbox and reduces rival life"
    );
}
