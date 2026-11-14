use super::super::archer::ArcherStageDescriptor as StageDescriptor;
use super::*;

type GlephPlayerReceipt = (i32, u8, Position, u16, u8);

fn gleph_player_receipt(game: &crate::native_world::player_lifecycle_tests::RivalTestGame) -> GlephPlayerReceipt {
    let state = crate::native_world::player_lifecycle_tests::rival_test_sim(game);
    let player = &state.entities()[usize::from(state.state.entity_index)];
    (state.canonical_player_life_for_test(), player.action, player.position,
        state.economy().player_castle_entity_index(), state.source_exit_result_byte())
}

pub(crate) fn complete_gleph_objectives_for_test(
    mut game: &mut crate::native_world::player_lifecycle_tests::RivalTestGame,
) -> (Vec<(&'static str, GlephPlayerReceipt)>, Option<(usize, GlephPlayerReceipt)>) {
    use crate::campaign::source_arena::CreatureRootOrigin;
    use crate::game_session::GameCommand;
    use crate::level_definition::ObjectiveCondition;
    use crate::native_world::{Mc2DispatchEvent, RuntimeRelation, area_damage};
    use crate::FlightWorld;
    use crate::native_world::player_lifecycle_tests::{
        RivalTestGame, rival_test_contact, rival_test_destroy_castle, rival_test_held_spell_generations,
        rival_test_participation, rival_test_sim as sim, rival_test_sim_mut as sim_mut,
        rival_test_step as step, rival_test_wait_unprotected,
    };

    fn spell113(game: &RivalTestGame) -> String {
        let state = sim(game);
        let raw = &state.entities()[113];
        let fields = |row: &crate::native_world::EntitySnapshot| (
            row.active, row.class, row.model, row.action, row.life, row.phase,
            row.control_flags, row.source_spell_parent_entity_index(), row.source_id,
        );
        format!("handle={:?}, raw={:?}, shared={:?}", state.source_arena().handle(113),
            fields(raw), state.shared_spells().pickup(113).map(fields))
    }
    let goals = game.definition().level_definition().unwrap().objectives();
    assert_eq!(goals.len(), 4);
    assert!(matches!(goals[0].condition(), ObjectiveCondition::CastleManaPercent(25)));
    assert!(matches!(goals[1].condition(), ObjectiveCondition::TargetDeath(_)));
    assert!(matches!(goals[2].condition(), ObjectiveCondition::CastleManaPercent(60)));
    assert!(matches!(goals[3].condition(), ObjectiveCondition::DefeatRivalWizard(_)));
    let player = sim(&game).state.entity_index;
    let source_spawn = sim(&game).state.position;
    let mut player_history = vec![("opened", gleph_player_receipt(&game))];
    let owner = usize::from(sim(&game).state.player_color_index);
    let rival = match &sim(&game).runtime().stages[3].relation {
        RuntimeRelation::LiveEntity(slot) => *slot,
        relation => panic!("selector7 must bind the actual Yragore player: {relation:?}"),
    };
    assert_eq!(sim(&game).players().iter().find(|row| row.entity_index == rival)
        .expect("selector7 has a real roster owner").player_color_index(), 6);
    assert_eq!(rival_test_participation(&game, rival), Some(1),
        "the package must retain the real unbanished source carrier");
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[..4], [1, 1, 1, 1]);
    let gate = &sim(&game).entities()[90];
    assert_eq!((gate.class, gate.model, gate.action, gate.source_id), (11, 4, 4, 20));
    let gate_handle = sim(&game).source_arena().handle(90).unwrap();
    let mut safe = gate.position;
    safe.z = sim(&game).canonical_terrain().ground_height(safe).wrapping_add(1024);
    sim_mut(&mut game).set_level2_entity_position(player, safe).unwrap();
    sim_mut(&mut game).state.actual_speed = 0;
    sim_mut(&mut game).state.target_speed = 0;

    // Declared prior resource boundary: a terrain-admitted player Castle and
    // prior bank support the source 25% and 60% predicates. Neither value is
    // claimed as earned in this scenario; the actual objective callbacks,
    // group construction and all later lifecycles run without staged flags.
    let castle = {
        let state = sim_mut(&mut game);
        for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
            state.economy_mut().set_castle_stage_footprint(stage, footprint);
        }
        let site = (1_u16..32).flat_map(|y| (1_u16..32).map(move |x| Position {
            x: x * 2048, y: y * 2048, z: 0,
        })).filter(|position| !state.economy().castle_projectile_site_blocked(
            *position, state.entities(), state.canonical_terrain()))
            .min_by_key(|position| crate::distance_3d_wrapped(*position, safe))
            .expect("the real map admits the declared prior Castle");
        state.economy_mut().build_castle(site, player).unwrap()
    };
    step(&mut game);
    assert_eq!(sim(&game).economy().player_castle_entity_index(), castle);
    let total = i64::from(sim(&game).economy().owner_mana_global());
    let aggregate = i64::from(sim(&game).economy().owner_mana_aggregate(player));
    assert!(total > 0);
    // 100*(aggregate + B)/(total + B) >= 60, including B in the
    // source-global denominator. Round B upward using checked wide arithmetic.
    let numerator = total.checked_mul(3).and_then(|v| aggregate.checked_mul(5).and_then(|a| v.checked_sub(a)))
        .expect("source census fits prior-bank calculation").max(0);
    let bank = i32::try_from(numerator.checked_add(1).unwrap() / 2).unwrap();
    assert!(bank <= i32::MAX / 100);
    assert!(sim_mut(&mut game).economy_mut().set_castle_mana_prerequisite_for_test(castle, bank));
    let mut group2 = false;
    for _ in 0..128 {
        step(&mut game);
        group2 |= sim(&game).dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 86, group: 2, authored_indices, .. }
                if authored_indices.contains(&172)));
        if group2 { break; }
    }
    assert!(group2, "actual 25% objective must produce group2 and Wyvern172");
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[0], 2);
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[2], 2,
        "actual 60% objective consumes the declared prior resource boundary");
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[1], 1);
    let target = sim(&game).creature_runtime_census().roots_of_kind(crate::level_definition::CreatureKind::Wyvern)
        .find(|row| row.origin == (CreatureRootOrigin::AuthoredRecord { authored_record: 172 }))
        .expect("group2 constructs the actual Wyvern172 generation").handle;
    assert_eq!(sim(&game).runtime().stages[1].relation, RuntimeRelation::LiveEntity(target.source_slot));
    assert!(sim(&game).entities()[usize::from(target.source_slot)].life > 0);
    area_damage::contact(sim_mut(&mut game).source_arena_mut(), None, target.source_slot, u16::MAX, player).unwrap();
    for _ in 0..256 {
        step(&mut game);
        if sim(&game).runtime().objectives[owner].stage_selectors[1] == 2 { break; }
    }
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[1], 2,
        "the original target's real death, not group creation, completes kind1");
    player_history.push(("wyvern goal", gleph_player_receipt(&game)));
    assert_eq!(rival_test_participation(&game, rival), Some(1));
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[3], 1,
        "one participating target keeps the rival objective incomplete");

    // Selected arrived contacts preserve source protection, all current
    // Castle tiers and real banishment/tails; no NPC state is staged.
    for _ in 0..256 {
        if sim(&game).economy().castle_for_owner(rival).is_some() { break; }
        step(&mut game);
    }
    assert!(sim(&game).economy().castle_for_owner(rival).is_some(), "target's source AI supplies its Castle");
    rival_test_wait_unprotected(&mut game, rival);
    player_history.push(("rival unprotected", gleph_player_receipt(&game)));
    let rival_handle = sim(&game).source_arena().handle(rival).unwrap();
    let original_castle = rival_test_destroy_castle(&mut game, rival, player);
    player_history.push(("rival castle lost", gleph_player_receipt(&game)));
    let common = sim(&game).session.state.live.nyphur_dispatcher.get(rival_handle).unwrap()
        .common_upkeep(sim(&game).entities()[usize::from(rival)].phase);
    assert_eq!(common.word_0x159, 0, "source protection must admit the real contact");
    let holders = rival_test_held_spell_generations(&game, rival);
    let at_castle_loss = spell113(&game);
    let life = sim(&game).entities()[usize::from(rival)].life;
    rival_test_contact(&mut game, rival, player, life);
    // Declared single player retreat before the long, natural holder tails.
    // Return to this level's actual starting position, far from the Castle
    // battle, using the current cave floor and ceiling. Enemies remain live.
    assert!(sim(&game).canonical_player_life_for_test() >= 0);
    let retreat = {
        let state = sim(&game);
        let current = state.economy().castle_for_owner(player).expect("current player Castle");
        assert_eq!(current.source_entity_index, castle);
        let dx = i32::from(source_spawn.x.wrapping_sub(current.position.x) as i16).abs();
        let dy = i32::from(source_spawn.y.wrapping_sub(current.position.y) as i16).abs();
        assert!(dx.max(dy) > 8192, "actual starting position is remote from the Castle battle");
        let terrain = state.canonical_terrain();
        let floor = i32::from(terrain.ground_height(source_spawn));
        let ceiling = terrain.ceiling_height(source_spawn).map(i32::from).unwrap_or(i32::from(i16::MAX));
        let min_z = floor + i32::from(state.profile().ground_clearance.max(0));
        let max_z = ceiling - i32::from(state.profile().ceiling_clearance.max(0));
        assert!(min_z <= max_z, "actual starting location has a valid cave flight interval");
        let mut position = source_spawn;
        position.z = i16::try_from(i32::from(source_spawn.z).clamp(min_z, max_z)).unwrap();
        position
    };
    sim_mut(&mut game).set_level2_entity_position(player, retreat).unwrap();
    player_history.push(("retreated to source spawn", gleph_player_receipt(&game)));
    let mut group20 = false;
    let mut slot_history = std::collections::VecDeque::new();
    let mut first_tail_death = None;
    for turn in 0..640 {
        let prior_gate = sim(&game).source_arena().handle(90);
        if slot_history.len() == 16 { slot_history.pop_front(); }
        slot_history.push_back(spell113(&game));
        if let Err(error) = game.advance(game.identity().guard(), GameCommand::default()) {
            panic!("Gleph source step failed: {error:?}; rival={rival_handle:?}, holders={holders:?}, at Castle loss={at_castle_loss}, slot113 history={slot_history:?}, after={}", spell113(&game));
        }
        assert!(sim(&game).dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "Gleph source dispatch: {:?}", sim(&game).dispatch_events());
        if first_tail_death.is_none() && sim(&game).canonical_player_life_for_test() < 0 {
            first_tail_death = Some((turn, gleph_player_receipt(&game)));
        }
        let activated = sim(&game).dispatch_events().iter().find_map(|event| match event {
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 90, group: 20, authored_indices, .. } =>
                Some(authored_indices),
            _ => None,
        });
        if let Some(indices) = activated {
            assert_eq!(prior_gate, Some(gate_handle));
            assert_eq!(indices.as_ref(), [372, 374, 422, 423, 424, 425, 426, 429, 430, 431, 432]);
        }
        group20 |= activated.is_some();
        if group20 && holders.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()) { break; }
    }
    assert_eq!(rival_test_participation(&game, rival), Some(0));
    assert!(sim(&game).source_arena().resolve(original_castle).is_none());
    assert!(holders.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()));
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[..4], [2, 2, 2, 2]);
    assert_eq!(sim(&game).runtime().objectives[owner].is_level_end, 1);
    assert!(group20, "real level-end switch produces authored exit and portal");
    player_history.push(("exit produced and holders reclaimed", gleph_player_receipt(&game)));
    assert!(sim(&game).canonical_player_life_for_test() >= 0,
        "the participant must be alive before the actual exit: history={player_history:?}, first_tail_death={first_tail_death:?}");
    (player_history, first_tail_death)
}

#[test]
fn gleph_source_objectives_produce_the_ordinary_exit() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::game_session::{GameCommand, GameCompletionStatus, GameSession};
    use crate::native_world::Mc2DispatchEvent;
    use crate::native_world::player_lifecycle_tests::{rival_test_sim as sim, rival_test_sim_mut as sim_mut};
    use std::path::PathBuf;
    let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
    let package = std::env::var_os("MC_LEVEL18_PACKAGE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("all-spells-20261104-integration/current-owner-package/level18"));
    let cache = std::env::var_os("MC_LEVEL18_OWNER_CACHE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("campaign-original-environments/level18"));
    let definition = load_mc2_campaign(&package, &crate::game_definition::default_catalog().unwrap()).unwrap();
    let mut game = GameSession::open(&Mc2CampaignAdapter, std::sync::Arc::new(definition), 2218,
        Mc2CampaignInputs::with_owner_cache(&package, &cache)).unwrap();
    let (mut player_history, first_tail_death) = complete_gleph_objectives_for_test(&mut game);
    let player = sim(&game).state.entity_index;
    let exit = *sim(&game).economy().authored_switch_producers().iter()
        .find(|row| row.active && row.authored_index == 432 && row.model == 12 && row.action == 12)
        .expect("real group20 exit432");
    assert!(sim(&game).economy().authored_class14().iter().any(|row| row.active && row.authored_index == 431));
    assert!(sim(&game).canonical_player_life_for_test() >= 0,
        "the participant must be alive before the actual exit: history={player_history:?}, first_tail_death={first_tail_death:?}");
    sim_mut(&mut game).set_level2_entity_position(player, exit.position).unwrap();
    player_history.push(("entered exit", gleph_player_receipt(&game)));
    let mut first_exit_death = None;
    let mut exit_events = Vec::new();
    for turn in 0..700 {
        if matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }) { break; }
        if let Err(error) = game.advance(game.identity().guard(), GameCommand::default()) {
            let state = sim(&game);
            let shared = state.shared_spells();
            let errors = state.dispatch_events().iter().filter_map(|event| match event {
                Mc2DispatchEvent::Error { source_slot, .. } => Some((*source_slot, event)),
                _ => None,
            }).collect::<Vec<_>>();
            let mut slots = errors.iter().map(|(slot, _)| *slot).collect::<std::collections::BTreeSet<_>>();
            for row in shared.pickups().iter().chain(shared.holders.iter()).chain(shared.dynamic_pickups()) {
                if row.active && row.class == 15 && row.model == 3 { slots.insert(row.index); }
            }
            let fields = |row: &crate::native_world::EntitySnapshot| (
                row.active, row.class, row.model, row.action, row.life, row.phase,
                row.source_spell_parent_entity_index(), row.source_id,
            );
            let slot_states = slots.into_iter().map(|slot| {
                let raw = state.entities().get(usize::from(slot)).map(fields);
                let candidates = shared.pickups().iter().map(|row| ("pickup", row))
                    .chain(shared.holders.iter().map(|row| ("holder", row)))
                    .chain(shared.dynamic_pickups().iter().map(|row| ("dynamic", row)))
                    .filter(|(_, row)| row.index == slot).map(|(origin, row)| (origin, fields(row)))
                    .collect::<Vec<_>>();
                let pickup = state.economy().spell_pickups().iter().find(|row| row.source_entity_index == slot);
                let replacement = state.economy().spell_replacements().iter().find(|row| row.source_entity_index == slot);
                (slot, state.source_arena().handle(slot), state.source_arena().economy.live_arena_kind(slot),
                    raw, candidates, pickup.copied(), replacement.copied())
            }).collect::<Vec<_>>();
            panic!("Gleph exit turn{turn}: {error:?}; source errors={errors:?}; slots={slot_states:?}; family3 acquisitions={:?}; player life={} action={}; history={player_history:?}; first_tail_death={first_tail_death:?}; first_exit_death={first_exit_death:?}; exit_events={exit_events:?}",
                shared.acquisition_states().iter().filter(|row| row.family == 3).collect::<Vec<_>>(),
                state.canonical_player_life_for_test(), state.entities()[usize::from(player)].action);
        }
        if first_exit_death.is_none() && sim(&game).canonical_player_life_for_test() < 0 {
            first_exit_death = Some((turn, gleph_player_receipt(&game)));
        }
        exit_events.extend(sim(&game).dispatch_events().iter().filter(|event| matches!(event,
            Mc2DispatchEvent::LevelExitSequenceStarted { .. } | Mc2DispatchEvent::LevelExitSequenceCompleted { .. }
        )).cloned());
    }
    assert!(matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }),
        "real exit did not complete: history={player_history:?}, first_tail_death={first_tail_death:?}, first_exit_death={first_exit_death:?}, exit_events={exit_events:?}, final={:?}", gleph_player_receipt(&game));
    assert_eq!(sim(&game).source_exit_result_byte(), 0x0a);
}

#[test]
fn alliance_follows_retained_parent_and_expires_when_dead() {
    let (mut arena, mut runtime, mut terrain, particles, player, feature) = mission_eighteen_source_parts();
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let slot = mc2::allocate_authored(&mut arena, &mut runtime, &mut terrain, &particles,
        position, RULES.max_life, None, feature.index).unwrap().unwrap();
    let terminal = crate::native_world::alliance_spell::construct_impact(&mut arena, &mut terrain).unwrap();
    let effect = &mut arena.entities[usize::from(terminal)];
    effect.source_id = player;
    effect.record[74] = 1;
    effect.subspell_index = 40;
    effect.record[154..156].copy_from_slice(&slot.to_le_bytes());
    let impact = crate::native_world::alliance_spell::step_impact(&mut arena, terminal, &[slot]).unwrap();
    assert_eq!(impact.affected_entities, [slot]);
    let hostile = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
    let mut enemy = crate::world::terrain::cave_features::new_event(hostile, 17, Position {
        x: position.x.wrapping_add(512), ..position
    });
    enemy.class = 3;
    enemy.model = 1;
    enemy.record[67] = 3;
    enemy.record[68] = 1;
    enemy.set_life(1000);
    arena.entities[usize::from(hostile)] = enemy;
    crate::native_world::area_damage::contact(&mut arena, None, slot, 1, hostile).unwrap();
    let parent = &mut arena.entities[usize::from(player)];
    parent.set_life(100);
    parent.record[154..156].copy_from_slice(&hostile.to_le_bytes());
    parent.record[42..44].fill(0);
    parent.reclaim_source_row();
    assert_eq!(parent.class, 0);
    assert_eq!(parent.life, 100);
    let families = std::array::from_fn(|model| if model == usize::from(SOURCE_MODEL) { vec![slot] } else { Vec::new() });
    for life in [100, 0] {
        let mut trial = arena.clone();
        let mut trial_runtime = runtime.clone();
        let mut trial_terrain = terrain.clone();
        trial.entities[usize::from(player)].set_life(life);
        let effects = mc2::step_row(&mut trial, &mut trial_runtime, &mut trial_terrain, &particles,
            slot, &[], &families).unwrap();
        let actor = &trial.entities[usize::from(slot)];
        assert_eq!(actor.record[77], if life > 0 { 14 } else { 10 });
        assert_eq!(u16::from_le_bytes([actor.record[44], actor.record[45]]),
            if life > 0 { player } else { 0 });
        if life > 0 {
            assert_eq!(effects.earlier_feedback, Some((player, 19)));
            assert_eq!(effects.feedback, Some((player, 24)));
            assert_eq!(i16::from_le_bytes([actor.record[50], actor.record[51]]), 39);
        } else {
            assert_eq!(actor.source_word_96(), 0);
            assert_eq!(actor.control_flags & 0x0600_0000, 0);
        }
    }
}


struct EmptyRoutes;

impl StageRuntime for EmptyRoutes {
    fn descriptor(&self, _index: u8) -> Option<StageDescriptor> {
        None
    }

    fn bind(&mut self, _index: u8) -> Option<StageDescriptor> {
        None
    }

    fn rearm_model_relations(&mut self) {}
}

fn terrain() -> SessionTerrain {
    crate::native_world::tests::level_two_campaign_session()
        .canonical_terrain()
        .clone()
}

fn mission_eighteen_root() -> std::path::PathBuf {
    std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj")
        },
        std::path::PathBuf::from,
    )
}

fn mission_eighteen_feature() -> crate::level1_mission::AuthoredFeature {
    let package = crate::campaign::spec::DecodedCampaignPackage::decode(
        mission_eighteen_root().join("level18"),
        crate::campaign::spec::CampaignSelection::ordinary(18).unwrap(),
    )
    .expect("decode owner-data Mission 18 package");
    let feature = crate::level1_mission::parse_authored_features(package.authored_bytes())
        .expect("parse owner-data Mission 18 feature table")
        .into_iter()
        .find(|feature| feature.index == 172)
        .expect("Mission 18 authored record172 exists");
    assert_eq!(
        (feature.kind, feature.subtype, feature.activation_group),
        (5, 16, 2),
        "owner-data record172 is an ordinary group2 Wyvern placement"
    );
    feature
}

fn mission_eighteen_source_parts() -> (
    crate::campaign::source_arena::SourceArena,
    crate::native_world::RuntimeSnapshot,
    SessionTerrain,
    Vec<crate::native_world::ParticleParameter>,
    u16,
    crate::level1_mission::AuthoredFeature,
) {
    let package = crate::campaign::spec::DecodedCampaignPackage::decode(
        mission_eighteen_root().join("level18"),
        crate::campaign::spec::CampaignSelection::ordinary(18).unwrap(),
    )
    .expect("decode owner-data Mission 18 package");
    let entities = crate::native_world::parse_entities(package.member("entities.bin").unwrap())
        .unwrap()
        .into_boxed_slice();
    let runtime =
        crate::native_world::parse_runtime(package.member("runtime.bin").unwrap()).unwrap();
    let presentation =
        crate::native_world::parse_presentation(package.member("presentation.bin").unwrap())
            .unwrap();
    let terrain = SessionTerrain::from_planes(
        crate::native_world::MapType::Night,
        crate::native_world::parse_terrain(package.member("terrain.bin").unwrap()).unwrap(),
        0,
    );
    let player = entities
        .iter()
        .find(|entity| entity.active && entity.class == 3 && entity.model == 0)
        .expect("Mission 18 retains a class3/model0 target");
    let player_slot = player.index;
    let economy = crate::native_world::Level2EconomyState::from_parts_with_castle_stage_footprints(
        terrain.clone(),
        &entities,
        player.mana,
        player.model,
        0,
        0,
        runtime.random_state,
        Default::default(),
        runtime.constructor_counters(),
    )
    .unwrap();
    let feature = crate::level1_mission::parse_authored_features(package.authored_bytes())
        .unwrap()
        .into_iter()
        .find(|feature| feature.index == 172)
        .expect("Mission 18 authored record172 exists");
    assert_eq!(
        (feature.kind, feature.subtype, feature.activation_group),
        (5, 16, 2)
    );
    (
        crate::campaign::source_arena::SourceArena::new(entities, economy),
        runtime,
        terrain,
        presentation.particles.to_vec(),
        player_slot,
        feature,
    )
}

fn wyvern(action: u8, position: Position) -> EntitySnapshot {
    let mut row = crate::native_world::cave_features::new_event(7, 19, position);
    row.class = 5;
    row.model = SOURCE_MODEL;
    row.record[67] = 5;
    row.record[68] = SOURCE_MODEL;
    row.set_source_action(action);
    row.set_life(10_000);
    row.set_source_dword_10(1);
    row.record[92..94].copy_from_slice(&128_i16.to_le_bytes());
    row
}

fn target(slot: u16, class: u8, model: u8, position: Position) -> Target {
    Target {
        slot,
        source_id: slot,
        class,
        model,
        life: 10_000,
        position,
        ..Target::default()
    }
}

fn projectile() -> Projectile {
    Projectile {
        parent: 7,
        owner: 7,
        target: 42,
        target_class: 3,
        target_model: 0,
        position: Position {
            x: 500,
            y: 600,
            z: 700,
        },
        target_position: Position {
            x: 900,
            y: 1200,
            z: 300,
        },
        yaw: 117,
        pitch: 219,
        payload: RULES.projectile_payload,
        damage: RULES.projectile_damage,
    }
}

#[test]
fn wyvern_attack_emits_one_descriptor61_targeted_projectile_and_decrements_burst() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 400,
    };
    let target_position = Position {
        x: 21_000,
        y: 20_000,
        z: 400,
    };
    let target = target(42, 3, 0, target_position);
    let targets = [target];
    let class3 = [target.slot];
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let mut routes = EmptyRoutes;
    let mut row = wyvern(130, position);
    row.record[154..156].copy_from_slice(&target.slot.to_le_bytes());
    row.phase = 8;
    row.set_yaw(heading_for_positions(position, target_position) as i16);
    row.set_control_flags(row.control_flags | 0x800);
    row.set_source_dword_10(2);

    let mut actor = Actor { row: &mut row };
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        family: &families[16],
        families: &families,
        runtime: &mut routes,
    };
    let effects = step(&mut actor, &mut context).unwrap();

    let launch = effects
        .projectile
        .expect("positive burst emits its own projectile");
    assert_eq!(
        (launch.target, launch.target_class, launch.target_model),
        (42, 3, 0)
    );
    assert_eq!(
        launch.position.z,
        position.z.wrapping_add(128_i16.wrapping_mul(6))
    );
    assert_eq!(launch.payload, RULES.projectile_payload);
    assert_eq!(launch.damage, RULES.projectile_damage);
    assert_eq!(
        actor.row.source_dword_10(),
        1,
        "source decrements before allocation"
    );
}

#[test]
fn wyvern_attack_with_unresolved_target_does_not_consume_burst_or_launch() {
    let terrain = terrain();
    let mut routes = EmptyRoutes;
    let mut row = wyvern(
        130,
        Position {
            x: 20_000,
            y: 20_000,
            z: 400,
        },
    );
    row.record[154..156].copy_from_slice(&42_u16.to_le_bytes());
    row.set_phase(8);
    row.set_source_dword_10(2);
    let targets = [];
    let class3 = [];
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let mut actor = Actor { row: &mut row };
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        family: &families[16],
        families: &families,
        runtime: &mut routes,
    };

    let effects = step(&mut actor, &mut context).unwrap();

    assert!(effects.projectile.is_none());
    assert_eq!(actor.row.source_dword_10(), 2);
    assert_eq!(actor.row.action, 130);
}

#[test]
fn wyvern_unsupported_action134_is_an_error_instead_of_a_silent_noop() {
    let terrain = terrain();
    let mut routes = EmptyRoutes;
    let mut row = wyvern(134, Position::default());
    let targets = [];
    let class3 = [];
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let mut actor = Actor { row: &mut row };
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        family: &families[16],
        families: &families,
        runtime: &mut routes,
    };

    let error = step(&mut actor, &mut context).unwrap_err();
    assert!(error.contains("action134 has no callback implementation"));
    assert_eq!(actor.row.action, 134);
}

#[test]
fn wyvern_self_replacement_reaims_from_cleared_slot_and_keeps_descriptor61_target() {
    let launch = projectile();
    let replacement_fov = 48;
    let state = mc2::projectile_launch_state(launch, 7, true, replacement_fov);
    let expected = crate::level1_mission::projectile_guidance_angles(
        Position::default(),
        launch.target_position,
    );

    assert_eq!(state.position, Position { x: 0, y: 0, z: 288 });
    assert_eq!((state.yaw, state.pitch), expected);
    assert_eq!((state.owner, state.target), (7, 0));
    assert_eq!((state.target_class, state.target_model), (3, 0));
}

#[test]
fn wyvern_non_replacement_keeps_source_pose_angles_and_target_relation() {
    let launch = projectile();
    let state = mc2::projectile_launch_state(launch, 19, false, 48);

    assert_eq!(state.position, launch.position);
    assert_eq!((state.yaw, state.pitch), (launch.yaw, launch.pitch));
    assert_eq!((state.owner, state.target), (launch.owner, launch.target));
    assert_eq!((state.target_class, state.target_model), (3, 0));
}

#[test]
fn wyvern_mission18_record172_constructs_and_launches_one_descriptor61_fireball() {
    let (mut arena, mut runtime, mut terrain, particles, target_slot, feature) =
        mission_eighteen_source_parts();
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let slot = mc2::allocate_authored(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        RULES.max_life,
        None,
        feature.index,
    )
    .unwrap()
    .expect("Mission 18 ordinary producer allocates its Wyvern");
    let source_id = arena.entities[usize::from(slot)].source_id;
    {
        let row = &mut arena.entities[usize::from(slot)];
        assert_eq!((row.class, row.model, row.action), (5, SOURCE_MODEL, 129));
        assert_eq!(row.parameter_relation, Some(DESCRIPTOR));
        row.set_source_action(130);
        row.set_phase(8);
        row.set_source_dword_10(1);
        row.set_control_flags(row.control_flags | 0x800);
        row.record[154..156].copy_from_slice(&target_slot.to_le_bytes());
    }
    let before = arena.economy.fireball_projectiles().len();
    let class3 = arena
        .entities
        .iter()
        .filter(|row| row.active && row.class == 3 && row.life >= 0)
        .map(|row| row.index)
        .collect::<Vec<_>>();
    let families: [Vec<u16>; 29] = std::array::from_fn(|model| {
        arena
            .entities
            .iter()
            .filter(|row| row.active && row.class == 5 && usize::from(row.model) == model)
            .map(|row| row.index)
            .collect()
    });
    let control_arena = arena.clone();
    let control_runtime = runtime.clone();
    let control_terrain = terrain.clone();
    let outcome = mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        slot,
        &class3,
        &families,
    )
    .unwrap();

    let emitted = outcome.projectile.expect("owner-data Wyvern attack emits");
    assert_eq!(
        (emitted.target, emitted.target_class, emitted.target_model),
        (target_slot, 3, 0)
    );
    assert_eq!(arena.economy.fireball_projectiles().len(), before + 1);
    let projectile = arena
        .economy
        .fireball_projectiles()
        .last()
        .expect("Wyvern launch is already registered in the shared Fireball arena");
    assert_eq!(
        projectile.origin,
        crate::native_world::FireballProjectileOrigin::Wyvern
    );
    assert_eq!((projectile.source_class, projectile.source_model), (9, 0));
    assert_eq!(projectile.owner_entity_index, source_id);
    assert_eq!((projectile.target_class, projectile.target_model), (3, 0));
    assert_eq!(projectile.target_source_entity_index, target_slot);
    assert_eq!(projectile.source_subspell_index, RULES.projectile_payload);
    assert_eq!(projectile.source_mana, RULES.projectile_damage);
    assert_ne!(projectile.control_flags & 0x0008_0000, 0);
    let light = arena
        .dynamic_lighting
        .active_slots()
        .find(|(_, light)| light.source_entity_index == projectile.source_entity_index)
        .map(|(_, light)| light)
        .expect("real Night Fireball constructor admits its linked light");
    assert_eq!(
        light.position, position,
        "admission precedes the muzzle lift"
    );
    assert_eq!(
        (light.intensity, light.jitter_range, light.life_scaled),
        (128, 1, false)
    );

    // Identical real source attack with a disabled option or occupied finite
    // light pool: only light admission is suppressed, never the projectile.
    for (enabled, full_pool) in [(false, false), (true, true)] {
        let mut arena = control_arena.clone();
        let mut runtime = control_runtime.clone();
        let mut terrain = control_terrain.clone();
        arena.dynamic_lighting_enabled = enabled;
        if full_pool {
            for _ in 0..crate::dynamic_lighting::DYNAMIC_LIGHT_CAPACITY {
                assert!(
                    arena
                        .dynamic_lighting
                        .admit(true, true, slot, position, 128, 1, false,)
                        .is_some()
                );
            }
        }
        let lights_before = arena.dynamic_lighting.active_count();
        let outcome = mc2::step_row(
            &mut arena,
            &mut runtime,
            &mut terrain,
            &particles,
            slot,
            &class3,
            &families,
        )
        .unwrap();
        assert!(outcome.projectile.is_some());
        assert_eq!(arena.economy.fireball_projectiles().len(), before + 1);
        let projectile = arena.economy.fireball_projectiles().last().unwrap();
        assert_eq!(projectile.control_flags & 0x0008_0000, 0);
        assert_eq!(projectile.source_mana, RULES.projectile_damage);
        assert_eq!(projectile.target_source_entity_index, target_slot);
        assert_eq!(arena.dynamic_lighting.active_count(), lights_before);
    }
}

#[test]
fn wyvern_mission18_source_pass_launches_dispatches_and_reclaims_fireball() {
    let feature = mission_eighteen_feature();
    let mut session = crate::campaign::CampaignSession::open(
        mission_eighteen_root().join("level18"),
        crate::campaign::spec::CampaignSelection::ordinary(18).unwrap(),
    )
    .expect("open owner-data Mission 18 campaign")
    .simulation()
    .clone();
    let particles = session.session.definition.presentation.particles.clone();
    let old_projectile_slots = session
        .economy()
        .fireball_projectiles()
        .iter()
        .map(|projectile| projectile.source_entity_index)
        .collect::<std::collections::BTreeSet<_>>();

    let (wyvern_slot, target_slot, owner, generation) = {
        let live = &mut session.session.state.live;
        let position =
            feature.world_position(live.terrain.ground_height(feature.world_position(0)));
        let slot = mc2::allocate_authored(
            &mut live.economy,
            &mut live.core.runtime,
            &mut live.terrain,
            &particles,
            position,
            RULES.max_life,
            None,
            feature.index,
        )
        .unwrap()
        .expect("Mission 18 ordinary producer allocates its Wyvern");
        let target = live
            .economy
            .entities
            .iter()
            .find(|row| row.active && row.class == 3 && row.model == 0 && row.life >= 0)
            .expect("Mission 18 has a live class3/model0 target");
        let target_slot = target.index;
        let owner = {
            let row = &mut live.economy.entities[usize::from(slot)];
            assert_eq!((row.class, row.model, row.action), (5, SOURCE_MODEL, 129));
            assert_eq!(row.parameter_relation, Some(DESCRIPTOR));
            row.set_source_action(130);
            row.set_phase(8);
            row.set_source_dword_10(1);
            row.set_control_flags(row.control_flags | 0x800);
            row.record[154..156].copy_from_slice(&target_slot.to_le_bytes());
            row.source_id
        };
        (slot, target_slot, owner, live.economy.slot_generation(slot))
    };

    session.step_level2_source_pass(0);
    let fresh = session
        .economy()
        .fireball_projectiles()
        .iter()
        .filter(|projectile| {
            projectile.origin == crate::native_world::FireballProjectileOrigin::Wyvern
                && projectile.owner_entity_index == owner
                && !old_projectile_slots.contains(&projectile.source_entity_index)
        })
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(fresh.len(), 1, "one action130 burst creates one Fireball");
    let projectile = fresh[0];
    assert_eq!((projectile.source_class, projectile.source_model), (9, 0));
    assert_eq!((projectile.target_class, projectile.target_model), (3, 0));
    assert_eq!(projectile.owner_entity_index, owner);
    if projectile.source_entity_index == wyvern_slot {
        assert!(
            projectile.target_source_entity_index == 0
                || projectile.target_source_entity_index == target_slot,
            "self-replacement starts unbound; same-pass acquisition may bind the selected target"
        );
    } else {
        assert_eq!(projectile.target_source_entity_index, target_slot);
    }
    if projectile.source_entity_index == wyvern_slot {
        assert_ne!(
            session.source_arena().slot_generation(wyvern_slot),
            generation,
            "NewEvent replaced the current Wyvern slot"
        );
        assert_eq!(projectile.owner_entity_index, wyvern_slot);
    } else {
        let actor = &session.source_arena().entities[usize::from(wyvern_slot)];
        assert_eq!(actor.phase, 9, "same-pass source scheduler advances Wyvern");
    }

    let projectile_slot = projectile.source_entity_index;
    if projectile.active && projectile_slot >= wyvern_slot {
        assert_eq!(
            projectile.phase,
            (projectile_slot as u8).wrapping_add(1),
            "ascending source cursor dispatches a live newborn Fireball in the same pass"
        );
        assert!(session.dispatch_events().iter().any(|event| matches!(
            event,
            crate::native_world::Mc2DispatchEvent::DynamicArena {
                pass: 0,
                source_slot,
                kind: crate::native_world::Level2DynamicArenaKind::FireballProjectile,
            } if *source_slot == projectile_slot
        )));
    }
    let projectile_generation = session.source_arena().slot_generation(projectile_slot);
    if projectile.active {
        let carrier = session
            .session
            .state
            .live
            .economy
            .economy
            .fireball_projectiles
            .iter_mut()
            .find(|carrier| carrier.source_entity_index == projectile.source_entity_index)
            .expect("scheduled Fireball retains its typed carrier");
        carrier.life = 0;
    }

    session.step_level2_source_pass(1);
    let terminal = session
        .economy()
        .fireball_projectiles()
        .iter()
        .find(|carrier| carrier.source_entity_index == projectile.source_entity_index)
        .copied();
    assert!(
        terminal.is_none_or(|carrier| !carrier.active),
        "next source pass dispatches and terminates the short-lived Fireball"
    );

    session.step_level2_source_pass(2);
    let live = &session.session.state.live.economy.economy;
    assert!(
        !live
            .fireball_projectiles()
            .iter()
            .any(|carrier| carrier.source_entity_index == projectile_slot),
        "following source boundary removes the hidden Fireball carrier"
    );
    assert!(
        live.free_source_slots().contains(&projectile_slot)
            || session.source_arena().slot_generation(projectile_slot) != projectile_generation,
        "the reclaimed slot is free or has already been reused with a new generation"
    );
}

#[test]
fn army_ring_allocates_a_real_summoned_wyvern_with_holder_provenance() {
    let (mut arena, mut runtime, mut terrain, particles, owner_slot, feature) =
        mission_eighteen_source_parts();
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let holder_slot = arena
        .allocate_source_slot()
        .expect("reserve the Army holder's source slot");
    let initial_counter = runtime.bytes[crate::native_world::RUNTIME_BYTES + 16];

    let handle = mc2::allocate_summoned(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        owner_slot,
        owner_slot,
        holder_slot,
    )
    .unwrap()
    .expect("ordinary Army ring has source capacity");

    let row = &arena.entities[usize::from(handle.source_slot)];
    assert_eq!((row.class, row.model, row.action), (5, SOURCE_MODEL, 135));
    assert_eq!(row.parameter_relation, Some(DESCRIPTOR));
    assert_eq!(row.source_id, owner_slot);
    assert_eq!(
        u16::from_le_bytes([row.record[44], row.record[45]]),
        owner_slot
    );
    assert_eq!(row.record[77], 13);
    assert_eq!(u16::from_le_bytes([row.record[50], row.record[51]]), 250);
    assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 0);
    assert_eq!((row.mana, row.max_mana), (0, 0));
    assert_eq!(row.position, position);
    assert_eq!(
        runtime.bytes[crate::native_world::RUNTIME_BYTES + 16],
        initial_counter.wrapping_add(1)
    );
    let identity = arena
        .imported_creature_runtime_identity(handle.source_slot, &std::collections::BTreeMap::new())
        .expect("summoned Wyvern has live raw-creature identity");
    assert_eq!(identity.handle, handle);
    assert_eq!(
        identity.origin,
        crate::campaign::source_arena::CreatureRootOrigin::SummonedSpell { holder_slot }
    );
}

#[test]
fn summoned_wyvern_follows_reused_parent_and_expires_when_it_dies() {
    let (mut arena, mut runtime, mut terrain, particles, parent_slot, feature) =
        mission_eighteen_source_parts();
    let parent = arena.handle(parent_slot).unwrap();
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let child = mc2::allocate_summoned(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        parent_slot,
        parent_slot,
        parent_slot,
    )
    .unwrap()
    .unwrap();
    assert_eq!(arena.creature_parent(child), Some(parent));
    let mut families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    families[16].push(child.source_slot);

    let mut living_arena = arena.clone();
    let mut living_runtime = runtime.clone();
    let mut living_terrain = terrain.clone();
    let living = mc2::step_row(
        &mut living_arena,
        &mut living_runtime,
        &mut living_terrain,
        &particles,
        child.source_slot,
        &[],
        &families,
    )
    .unwrap();
    assert!(!living.expire);
    let living_row = &living_arena.entities[usize::from(child.source_slot)];
    assert!(!living_row.drawing_disabled());
    assert!(u16::from_le_bytes(living_row.record[50..52].try_into().unwrap()) < 250);

    let mut freed = arena.clone();
    let mut freed_runtime = runtime.clone();
    let mut freed_terrain = terrain.clone();
    freed.entities[usize::from(parent_slot)].set_life(0);
    freed.entities[usize::from(parent_slot)].reclaim_source_row();
    freed.economy.release_slot(parent_slot, None);
    let result = mc2::step_row(
        &mut freed, &mut freed_runtime, &mut freed_terrain, &particles,
        child.source_slot, &[], &families,
    ).unwrap();
    assert!(!result.expire, "a reclaimed fixed parent record with zero life remains readable");

    let replacement = arena.entities[usize::from(parent_slot)].clone();
    arena.entities[usize::from(parent_slot)].reclaim_source_row();
    arena.economy.release_slot(parent_slot, None);
    assert_eq!(arena.allocate_source_slot(), Some(parent_slot));
    arena.entities[usize::from(parent_slot)] = replacement;
    assert_ne!(arena.handle(parent_slot).unwrap(), parent);
    assert_eq!(arena.creature_parent(child), None);
    assert_eq!(arena.creature_parent_binding(child), Some(parent));
    let followed = mc2::step_row(
        &mut arena, &mut runtime, &mut terrain, &particles, child.source_slot, &[], &families,
    ).unwrap();
    assert!(!followed.expire, "a current living replacement is the source parent");
    assert!(!arena.entities[usize::from(child.source_slot)].drawing_disabled());
    arena.entities[usize::from(parent_slot)].set_life(0);
    let zero = mc2::step_row(
        &mut arena, &mut runtime, &mut terrain, &particles, child.source_slot, &[], &families,
    ).unwrap();
    assert!(!zero.expire, "the source parent predicate accepts zero life");
    arena.entities[usize::from(parent_slot)].set_life(-1);
    let expired = mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        child.source_slot,
        &[],
        &families,
    )
    .unwrap();
    assert!(expired.expire && expired.summon_expire);
    assert!(arena.entities[usize::from(child.source_slot)].drawing_disabled());
    assert!(
        arena.resolve(child).is_some(),
        "expiry retains this generation until the next boundary"
    );
    arena
        .rebuild_source_arena_with_terrain(&mut terrain, &[])
        .unwrap();
    assert!(arena.resolve(child).is_none());
}

#[test]
fn army_ring_reports_none_when_the_shared_source_allocator_is_exhausted() {
    let (mut arena, mut runtime, mut terrain, particles, owner_slot, feature) =
        mission_eighteen_source_parts();
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    while arena.allocate_source_slot().is_some() {}
    let counter = runtime.bytes[crate::native_world::RUNTIME_BYTES + 16];

    let result = mc2::allocate_summoned(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        owner_slot,
        owner_slot,
        owner_slot,
    )
    .unwrap();

    assert_eq!(result, None);
    assert_eq!(
        runtime.bytes[crate::native_world::RUNTIME_BYTES + 16],
        counter
    );
}

#[test]
fn wyvern_stage_wander_uses_one_draw_every_sixteen_visits_without_a_route() {
    struct Routes(Option<StageDescriptor>);
    impl StageRuntime for Routes {
        fn descriptor(&self, _: u8) -> Option<StageDescriptor> {
            self.0
        }
        fn bind(&mut self, _: u8) -> Option<StageDescriptor> {
            self.0
        }
        fn rearm_model_relations(&mut self) {}
    }
    let (_, _, terrain, particles, _, feature) = mission_eighteen_source_parts();
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let base = mc2::construct_row(7, 1, 0x1234, position, &mut 0, &particles).unwrap();
    let route = StageDescriptor {
        kind: 5,
        flags: 2,
        next: 0,
        point: position,
        actor: 0,
        model: 15,
        scalar: 0,
    };
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    for mode in [2, 5, 6, 7, 8, 9] {
        for (phase, blocked, draws) in
            [(8, false, 0), (16, false, 1), (64, false, 1), (16, true, 0)]
        {
            let mut row = base.clone();
            row.set_source_action(135);
            row.record[77] = mode;
            row.set_phase(phase);
            row.set_source_roll(2040);
            // Isolate the post-motion source RNG tail, including the blocked
            // negative; do not force any target, burst, or actor RNG result.
            row.set_control_flags(row.control_flags | 0x800 | if blocked { 0x40000 } else { 0 });
            let mut routes = Routes((mode == 2 || mode == 5).then_some(route));
            let mut context = Context {
                terrain: &terrain,
                targets: &[],
                class3: &[],
                family: &[],
                families: &families,
                runtime: &mut routes,
            };
            step(&mut Actor { row: &mut row }, &mut context).unwrap();
            let expected_rng = if draws == 1 {
                level2_next_rng16(base.entity_rng)
            } else {
                base.entity_rng
            };
            let expected_roll = if draws == 1 {
                (2040 + expected_rng % 113 + 142) & 2047
            } else {
                2040
            };
            assert_eq!(
                (row.entity_rng, row.source_roll()),
                (expected_rng, expected_roll as i16),
                "mode{mode}/phase{phase}/blocked{blocked}: source 1E1C0 or missing-target 1D8C0 tail"
            );
            assert_eq!(row.action, 135);
        }
    }
    // A real point route keeps its distinct two-draw phase64 perturbation.
    let mut row = base.clone();
    row.set_source_action(135);
    row.record[77] = 1;
    row.set_phase(64);
    row.set_control_flags(row.control_flags | 0x800);
    let point = Position {
        x: position.x.wrapping_add(4096),
        ..position
    };
    let mut routes = Routes(Some(StageDescriptor {
        kind: 1,
        flags: 0,
        point,
        ..route
    }));
    let mut context = Context {
        terrain: &terrain,
        targets: &[],
        class3: &[],
        family: &[],
        families: &families,
        runtime: &mut routes,
    };
    step(&mut Actor { row: &mut row }, &mut context).unwrap();
    let first = level2_next_rng16(base.entity_rng);
    let second = level2_next_rng16(first);
    let delta = (i32::from(second & 255) + 85) * (2 * (i32::from(first % 157) / 79) - 1);
    assert_eq!(row.entity_rng, second);
    assert_eq!(
        row.source_roll(),
        (heading_for_positions(position, point) as i16).wrapping_add(delta as i16) & 2047
    );
}

#[test]
fn wyvern_stage_selects_nearest_frozen_target_or_the_same_route_relation() {
    let terrain = terrain();
    let position = Position {
        x: 65_000,
        y: 20_000,
        z: 400,
    };
    let far = target(
        2,
        5,
        15,
        Position {
            x: 60_000,
            ..position
        },
    );
    let mut near = target(30, 5, 15, Position { x: 100, ..position });
    // Membership was frozen before callbacks. A preceding callback's death
    // does not remove the address from sub_1E3E0's model-list search.
    near.life = -1;
    near.control_flags = ENTITY_DRAW_DISABLED_BIT;
    near.model = 24; // The frozen model15 address now holds another model.
    let tied = target(31, 5, 15, near.position);
    let mut unrelated_peer = target(4, 5, 16, position);
    unrelated_peer.stage = StageState {
        index: 8,
        relation: near.slot,
        ..StageState::default()
    };
    let mut peer = target(5, 5, 16, position);
    peer.stage = StageState {
        index: 7,
        relation: far.slot,
        ..StageState::default()
    };
    let targets = [far, near, tied, unrelated_peer, peer];
    let mut families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    families[15] = vec![2, 30, 31];
    families[16] = vec![4, 5];
    struct Routes(StageDescriptor);
    impl StageRuntime for Routes {
        fn descriptor(&self, _: u8) -> Option<StageDescriptor> {
            Some(self.0)
        }
        fn bind(&mut self, _: u8) -> Option<StageDescriptor> {
            Some(self.0)
        }
        fn rearm_model_relations(&mut self) {}
    }
    fn visit(
        row: &mut EntitySnapshot,
        terrain: &SessionTerrain,
        targets: &[Target],
        families: &[Vec<u16>; 29],
        route: StageDescriptor,
    ) -> u16 {
        let mut routes = Routes(route);
        let mut context = Context {
            terrain,
            targets,
            class3: &[],
            family: &families[16],
            families,
            runtime: &mut routes,
        };
        row.record[77] = 5;
        row.set_phase(8);
        row.set_control_flags(row.control_flags | 0x800);
        // Exercise the enabled actor callback, keeping selection private.
        step(&mut Actor { row }, &mut context).unwrap();
        word(row, 78)
    }
    let route = StageDescriptor {
        kind: 5,
        flags: 2,
        next: 0,
        point: position,
        actor: 0,
        model: 15,
        scalar: 0,
    };
    let mut row = wyvern(135, position);
    row.record[76] = 7;
    assert_eq!(
        visit(&mut row, &terrain, &targets, &families, route),
        30,
        "nearest wrapped XY distance wins, with first-in-list tie order"
    );
    assert_eq!(word(&row, 78), 30);
    assert_eq!(
        visit(
            &mut row,
            &terrain,
            &targets,
            &families,
            StageDescriptor { flags: 3, ..route }
        ),
        30,
        "a current relation is retained before looking at another actor"
    );
    put(&mut row, 78, 0);
    assert_eq!(
        visit(
            &mut row,
            &terrain,
            &targets,
            &families,
            StageDescriptor { flags: 3, ..route }
        ),
        2,
        "bit1 shares the first nonzero relation with the same route index"
    );
    visit(
        &mut row,
        &terrain,
        &targets,
        &families,
        StageDescriptor {
            flags: 0,
            actor: 31,
            ..route
        },
    );
    assert_eq!(
        row.source_roll(),
        heading_for_positions(position, near.position) as i16,
        "an explicit actor route bypasses the cached model relation"
    );
}

#[test]
fn linked_point_route_hit_inherits_life_and_reacts_to_external_attacker() {
    struct Routes(StageDescriptor);
    impl StageRuntime for Routes {
        fn descriptor(&self, _: u8) -> Option<StageDescriptor> { Some(self.0) }
        fn bind(&mut self, _: u8) -> Option<StageDescriptor> { Some(self.0) }
        fn rearm_model_relations(&mut self) {}
    }
    let terrain = terrain();
    let position = Position { x: 20_000, y: 20_000, z: 2_000 };
    let route = StageDescriptor { kind: 1, flags: 0, next: 0,
        point: Position { x: 24_000, ..position }, actor: 0, model: 0, scalar: 0 };
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    for same_family in [false, true] {
        let mut row = wyvern(135, position);
        row.record[76] = 1;
        row.record[77] = 1;
        put(&mut row, 56, 8);
        let mut child = target(8, 5, 16, position);
        child.life = 900;
        child.attacker = 9;
        let attacker = target(9, if same_family { 5 } else { 3 },
            if same_family { 16 } else { 0 }, position);
        let targets = [child, attacker];
        let mut routes = Routes(route);
        let mut context = Context { terrain: &terrain, targets: &targets, class3: &[],
            family: &[], families: &families, runtime: &mut routes };
        step(&mut Actor { row: &mut row }, &mut context).unwrap();
        assert_eq!((row.life, word(&row, 42)), (900, 9));
        if same_family {
            assert_eq!((row.action, row.record[77]), (135, 1));
        } else {
            assert_eq!((row.action, row.record[77], word(&row, 154)), (130, 10, 9));
        }
        assert_ne!(row.position.z, position.z, "the hit branch settles in both cases");
    }
}

#[test]
fn selected_target_overlap_overrides_an_overlapping_peer_heading() {
    struct Routes(StageDescriptor);
    impl StageRuntime for Routes {
        fn descriptor(&self, _: u8) -> Option<StageDescriptor> { Some(self.0) }
        fn bind(&mut self, _: u8) -> Option<StageDescriptor> { Some(self.0) }
        fn rearm_model_relations(&mut self) {}
    }
    let terrain = terrain();
    let position = Position { x: 20_000, y: 20_000, z: 400 };
    let peer = target(8, 5, 16, Position { x: 20_050, ..position });
    let mut selected = target(9, 5, 15, Position { y: 20_050, ..position });
    selected.half_width = 128;
    let mut families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    families[16] = vec![8];
    let route = StageDescriptor { kind: 5, flags: 0, next: 0, point: position,
        actor: 9, model: 15, scalar: 0 };
    for (include_peer, target_near) in [(true, true), (true, false), (false, true)] {
        let mut row = wyvern(135, position);
        put(&mut row, 88, 128);
        row.record[76] = 7;
        row.record[77] = 5;
        row.set_phase(8);
        row.set_control_flags(row.control_flags | 0x800);
        let mut target = selected;
        if !target_near { target.position.y = position.y.wrapping_add(1_024); }
        let targets = [peer, target];
        let mut routes = Routes(route);
        let mut context = Context { terrain: &terrain, targets: &targets, class3: &[],
            family: if include_peer { &families[16] } else { &[] },
            families: &families, runtime: &mut routes };
        step(&mut Actor { row: &mut row }, &mut context).unwrap();
        let expected = if target_near {
            heading_for_positions(target.position, position)
        } else {
            heading_for_positions(peer.position, position)
        };
        assert_eq!(row.source_roll(), expected as i16,
            "the selected-target overlap wins after the independent peer scan: peer={include_peer} target_near={target_near}");
    }
}

#[test]
fn owner_mission18_switch_wyvern_acquires_and_damages_its_real_model26_target() {
    use crate::PlayerCommand;
    use crate::campaign::source_arena::{CreatureRootOrigin, SourceEntityView};
    use crate::native_world::Mc2DispatchEvent;

    let mut session = crate::campaign::CampaignSession::open(
        mission_eighteen_root().join("level18"),
        crate::campaign::spec::CampaignSelection::ordinary(18).unwrap(),
    )
    .unwrap()
    .simulation()
    .clone();
    let gate = session.entities()[86].clone();
    assert_eq!(
        (gate.class, gate.model, gate.action, gate.source_id),
        (11, 32, 32, 2)
    );
    let task = usize::try_from(gate.level_gate).unwrap();
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !session.dispatch_events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated { group: 2, .. }
        )),
        "the real objective switch must not fire before its prerequisite"
    );
    // Stage only this package's completed-task input. The real switch, group
    // scan, typed authored constructor and route binding produce record172.
    // This is a focused production/combat check, not an earned mission run.
    for owner in 0..session.runtime.objectives.len() {
        session
            .runtime
            .set_level2_objective_stage_selector_for(owner, task, 2);
    }
    for _ in 0..8 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        if session.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated {source_slot: 86, group: 2, authored_indices, ..}
                if authored_indices.contains(&172)
        )) {break;}
    }
    let handle = session
        .entities()
        .iter()
        .find_map(|row| {
            let identity = session.creature_identity_for_source_slot(row.index)?;
            (identity.kind == crate::level_definition::CreatureKind::Wyvern
                && identity.origin
                    == CreatureRootOrigin::AuthoredRecord {
                        authored_record: 172,
                    })
            .then_some(identity.handle)
        })
        .expect("ordinary owner switch constructs its typed record172 Wyvern");
    let SourceEntityView::Imported(actor) = session.source_arena().resolve(handle).unwrap() else {
        panic!("Wyvern is an authoritative raw source actor");
    };
    let actor = actor.clone();
    // sub_12100 gives direct authored-row selectors precedence over subtype
    // selectors. StageVar6 directly names row172 (kind4, model17 relation),
    // whereas subtype-matched StageVar2 is the model17 route to model26.
    let feature = mission_eighteen_feature();
    let route = session.session.state.live.mission.level2_authored_stage_variable_for(
        feature,
        session.runtime.active_stage_variable_count(),
    ).expect("record172 has an authored route");
    assert_eq!((route.index, route.kind, route.selector, route.relation_word), (6, 4, 172, 62));
    assert_eq!((actor.record[76], actor.record[77]), (6, 4));
    assert_eq!(actor.parameter_relation, Some(DESCRIPTOR));
    // sub_1D8C0 calls sub_1E3E0 only while its cached model relation is
    // zero. Let the actual source callback choose the guide before arranging
    // the encounter; moving an unrelated model17 cannot change that choice.
    for _ in 0..16 {
        let SourceEntityView::Imported(current) = session.source_arena().resolve(handle).unwrap() else {
            panic!("Wyvern remains the authored source generation");
        };
        if word(current, 78) != 0 { break; }
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
    }
    let SourceEntityView::Imported(actor) = session.source_arena().resolve(handle).unwrap() else {
        panic!("Wyvern remains the authored source generation");
    };
    let actor = actor.clone();
    let guide_slot = word(&actor, 78);
    assert_ne!(guide_slot, 0, "the source callback chooses its model17 guide");
    let guide_handle = session.source_arena().handle(guide_slot).unwrap();
    let guide = &session.entities()[usize::from(guide_slot)];
    assert_eq!((guide.class, guide.model, guide.record[76], guide.record[77]), (5, 17, 1, 6));
    let sprite = session.campaign_entity_presentations(0).unwrap().into_iter()
        .find(|view| view.entity_index == handle.source_slot)
        .expect("the ordinary Wyvern reaches the original sprite projection");
    assert_eq!((sprite.render_parameter_index, sprite.position), (207, actor.position));
    assert_eq!(sprite.base_tmap, session.session.definition.presentation.particles[207].base_tmap);
    let targets = session
        .entities()
        .iter()
        .filter(|row| row.active && row.class == 5 && row.model == 26 && row.life >= 0)
        .map(|row| row.index)
        .collect::<Vec<_>>();
    let target_slot = *targets
        .first()
        .expect("owner18 supplies the actual route's model26 targets");
    let guide_route = session.session.state.live.mission.level2_authored_stage_variable_for(
        session.session.state.live.mission.authored_features().iter()
            .find(|feature| feature.index == 63).copied().unwrap(),
        session.runtime.active_stage_variable_count(),
    ).expect("model17 has an authored subtype route");
    assert_eq!((guide_route.index, guide_route.kind, guide_route.stage, guide_route.relation_word),
        (1, 6, 2, 60));
    let target_handle = session.source_arena().handle(target_slot).unwrap();
    let life_before = session.entities()[usize::from(target_slot)].life;
    let nearby = Position {
        x: actor.position.x.wrapping_add(1024),
        ..actor.position
    };
    // Position the existing target using the canonical position operation.
    // No actor action, target, phase, burst, protection or RNG is manufactured.
    session
        .set_level2_entity_position(target_slot, nearby)
        .unwrap();
    // StageVar6 mode4 follows the selected model17's target. Bring the real
    // model17 guide into range; its own StageVar2 must acquire model26.
    let guide_position = Position { x: actor.position.x.wrapping_add(512), ..actor.position };
    session.set_level2_entity_position(guide_slot, guide_position).unwrap();
    let mut distant = session.clone();
    for &slot in &targets {
        let far = Position {
            x: actor.position.x.wrapping_add(0x6000),
            ..actor.position
        };
        distant.set_level2_entity_position(slot, far).unwrap();
    }
    let mut acquired = false;
    let mut moved = false;
    let mut guide_acquired = false;
    let mut guide_transitioned = false;
    let mut heard = false;
    let mut effect_sequence = session.effect_journal().cursor(0).next_sequence;
    let mut shot_handles = Vec::new();
    let mut consumed = None;
    for turn in 0..240 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "owner18 Wyvern scheduler error: {:?}",
            session.dispatch_events()
        );
        let SourceEntityView::Imported(actor_now) = session
            .source_arena()
            .resolve(handle)
            .expect("ordinary Wyvern remains its original generation")
        else {
            panic!("Wyvern must keep its raw actor carrier");
        };
        for record in session.effect_journal().records().iter().filter(|record| record.sequence >= effect_sequence) {
            if matches!(record.effect, crate::campaign::journal::CampaignEffect::Flight(
                crate::FlightEvent::PrepareSound { audio_site: crate::MissionAudioSite::WyvernAttack,
                    entity_index, sound_id: 39, .. }) if entity_index == handle.source_slot) {
                let audio = record.audio.expect("Wyvern sound39 reaches positional audio preparation");
                assert_eq!((audio.command.relative_x, audio.command.relative_y, audio.command.relative_z),
                    (i32::from(actor_now.position.x) - i32::from(session.state.position.x),
                     i32::from(actor_now.position.y) - i32::from(session.state.position.y),
                     i32::from(actor_now.position.z) - i32::from(session.state.position.z)),
                    "sound39 uses the same actor's current source pose");
                assert_eq!(audio.play_type, 3, "source sound39 uses its original positional play type");
                heard = true;
            }
        }
        effect_sequence = session.effect_journal().cursor(0).next_sequence;
        acquired |= word(actor_now, 154) == target_slot && actor_now.action == 130;
        let guide = &session.entities()[usize::from(guide_slot)];
        assert_eq!(session.source_arena().handle(guide_slot), Some(guide_handle),
            "the source-selected guide retains its generation");
        guide_transitioned |= guide.record[76] == 2;
        guide_acquired |= word(guide, 154) == target_slot;
        moved |= actor_now.position != actor.position;
        for shot in session
            .economy()
            .fireball_projectiles()
            .iter()
            .filter(|shot| {
                shot.origin == crate::native_world::FireballProjectileOrigin::Wyvern
                    && shot.owner_entity_index == actor.source_id
                    && shot.target_source_entity_index == target_slot
            })
        {
            if let Some(handle) = session.source_arena().handle(shot.source_entity_index)
                && !shot_handles.contains(&handle)
            {
                shot_handles.push(handle);
            }
        }
        if let Some(SourceEntityView::Imported(target)) =
            session.source_arena().resolve(target_handle)
            && target.life < life_before
            && word(target, 42) == actor.source_id
        {
            consumed = Some((turn, target.life));
        }
        if turn < 8 {
            distant.step_outer_turn(PlayerCommand::default(), 0);
            let SourceEntityView::Imported(far_actor) =
                distant.source_arena().resolve(handle).unwrap()
            else {
                panic!("negative route must keep its raw Wyvern");
            };
            assert_ne!(
                far_actor.action, 130,
                "same route stays out of attack range with only distant model26 actors"
            );
        }
        if guide_transitioned && guide_acquired && acquired && moved && heard && !shot_handles.is_empty() && consumed.is_some() {
            break;
        }
    }
    assert!(
        guide_transitioned && guide_acquired && acquired && moved && heard && !shot_handles.is_empty() && consumed.is_some(),
        "real route must acquire, fly, sound, fire and damage original model26: guide_transitioned={guide_transitioned} guide={guide_acquired} acquired={acquired} moved={moved} heard={heard} shots={shot_handles:?} consumed={consumed:?} actor={:?}",
        session.source_arena().resolve(handle)
    );
    eprintln!(
        "ordinary Wyvern{} -> model17{} -> model26{}: consumed={consumed:?}, projectiles={shot_handles:?}",
        handle.source_slot, guide_slot, target_slot
    );
    for _ in 0..32 {
        if shot_handles
            .iter()
            .any(|&shot| session.source_arena().resolve(shot).is_none())
        {
            return;
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    panic!("naturally produced Wyvern projectiles must reach normal next-boundary reclamation");
}

#[test]
fn owner_mission18_wyvern_public_meteor_death_reaches_target_objective_and_mana() {
    use crate::PlayerCommand;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::campaign::source_arena::{CreatureRootOrigin, SourceEntityView};
    use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent, RuntimeRelation};

    fn aim_public_possession(
        sim: &mut crate::native_world::Mc2Simulation,
        player: u16,
        reward: u16,
    ) -> (Position, Position, i16, i16) {
        let sphere = *sim.economy().spheres().iter()
            .find(|row| row.source_entity_index == reward && row.active)
            .expect("the real reward remains available for public aim");
        let top = sim.possession_guidance_target_top_for_test(reward).unwrap();
        // Aim two ordinary source visits ahead from the sphere's current
        // velocity. No sphere position, velocity, target or timing is written.
        let aim = Position {
            x: top.x.wrapping_add_signed(sphere.velocity_x.wrapping_mul(2)),
            y: top.y.wrapping_add_signed(sphere.velocity_y.wrapping_mul(2)),
            ..top
        };
        // The selected-hand command uses source bit0x100. Compensate its
        // actual 256-unit lateral offset, then solve the public yaw/pitch from
        // the source hand position, including terrain rollback and eye FOV.
        let mut approach = Position { x: aim.x.wrapping_sub(768), y: aim.y,
            z: aim.z.wrapping_add(512) };
        crate::move_entity(&mut approach, 1024, 0, 256);
        approach.z = approach.z.max(sim.canonical_terrain().ground_height(approach).wrapping_add(256));
        let mut yaw = 512_i16;
        let mut pitch = 0_i16;
        for _ in 0..3 {
            let mut launch = approach;
            crate::move_entity(&mut launch, yaw.wrapping_sub(512), 0, 256);
            if sim.canonical_terrain().ground_height(launch) > launch.z { launch = approach; }
            launch.z = launch.z.wrapping_add(sim.entities()[usize::from(player)].array_fov());
            (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(launch, aim);
        }
        sim.set_level2_entity_position(player, approach).unwrap();
        sim.set_level2_player_aim_for_test(yaw, pitch);
        (approach, aim, yaw, pitch)
    }

    // Acquire the real Level6 pickup, then carry it through the same campaign
    // entry operation used by the existing acquired-Meteor scenario.
    let level6 = std::env::var_os("MC_METEOR_LEVEL6_PACKAGE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level6"));
    let mut source = CampaignSession::open(level6, CampaignSelection::ordinary(6).unwrap()).unwrap();
    // Explicit opening progression, as in the existing earned-mana path;
    // Meteor itself must still be acquired from the actual Level6 pickup.
    source.simulation_mut().apply_carried_spell_families(&[0, 1, 2], Some(0)).unwrap();
    let source_player = source.simulation().state.entity_index;
    let pickup = source.simulation().shared_spells().pickups().iter()
        .find(|row| row.index == 5 && (row.class, row.model, row.action) == (15, 9, 28))
        .expect("Level6 supplies its real Meteor pickup").index;
    for _ in 0..16 {
        let sim = source.simulation_mut();
        let position = sim.entities()[usize::from(pickup)].position;
        sim.set_level2_entity_position(source_player, position).unwrap();
        sim.step_outer_turn(PlayerCommand::default(), 0);
        assert!(sim.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
        if sim.shared_spells().local_player().enabled_spell_holders[9] == pickup { break; }
    }
    assert_eq!(source.simulation().shared_spells().local_player().enabled_spell_holders[9], pickup);
    let learned = source.simulation().shared_spells().local_player().enabled_spell_holders
        .iter().enumerate().filter_map(|(family, holder)| (*holder != 0).then_some(family as u8))
        .collect::<Vec<_>>();
    let mut campaign = CampaignSession::open(mission_eighteen_root().join("level18"),
        CampaignSelection::ordinary(18).unwrap()).unwrap();
    assert_eq!(campaign.rebuild_for_spell_carry(&learned, Some(9)).unwrap(), Some(9));
    let sim = campaign.simulation_mut();
    let player = sim.state.entity_index;
    for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
        sim.economy_mut().set_castle_stage_footprint(stage, footprint);
    }
    let encounter = mission_eighteen_feature().world_position(0);
    let castle_site = (1_u16..32).flat_map(|y| (1_u16..32).map(move |x| Position {
        x: x * 2048, y: y * 2048, z: 0,
    })).filter(|position| !sim.economy().castle_projectile_site_blocked(
        *position, sim.entities(), sim.canonical_terrain()))
        .min_by_key(|position| xy(*position, encounter)).expect("owner terrain has a Castle site");
    let castle = sim.economy_mut().build_castle(castle_site, player).unwrap();
    assert!(sim.economy_mut().set_castle_mana_prerequisite_for_test(castle, 2_000_000));
    sim.shared_spells_mut().award_spell_experience(player, 9, 5_000);
    {
        let live = &mut sim.session.state.live;
        let parent = &mut live.economy.entities[usize::from(player)];
        parent.max_mana = 2_000_000;
        parent.record[144..148].copy_from_slice(&parent.max_mana.to_le_bytes());
        parent.set_mana(parent.max_mana);
        parent.set_mana_regen(0);
        let parent = parent.clone();
        live.shared_spells.sync_player_entity_from_authority(&parent);
        live.shared_spells.player_mana = parent.mana;
        live.shared_spells.caster.mana = parent.mana;
        live.shared_spells.sync_typed_spell_parent_caches_from_player_entity();
        if let Some(fire) = live.core.context.spell_fire.as_mut() { fire.caster.mana = parent.mana; }
    }
    let gate = sim.entities()[86].clone();
    assert_eq!((gate.class, gate.model, gate.action, gate.source_id), (11, 32, 32, 2));
    let task = usize::try_from(gate.level_gate).unwrap();
    for owner in 0..sim.runtime.objectives.len() {
        sim.runtime.set_level2_objective_stage_selector_for(owner, task, 2);
    }
    for _ in 0..8 {
        sim.step_outer_turn(PlayerCommand::default(), 0);
        if sim.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 86, group: 2, authored_indices, .. }
                if authored_indices.contains(&172))) { break; }
    }
    let handle = sim.entities().iter().find_map(|row| {
        let identity = sim.creature_identity_for_source_slot(row.index)?;
        (identity.origin == CreatureRootOrigin::AuthoredRecord { authored_record: 172 })
            .then_some(identity.handle)
    }).expect("the real switch produces authored Wyvern172");
    let slot = handle.source_slot;
    let initial = sim.entities()[usize::from(slot)].clone();
    assert_eq!((initial.class, initial.model, initial.life), (5, 16, RULES.max_life));
    assert!(initial.mana > 0);
    assert!(matches!(sim.runtime.stages[1].relation, RuntimeRelation::LiveEntity(target) if target == slot),
        "the original Level18 target-death task binds row172's live slot");
    let owner = sim.session.state.live.core.players.iter().find(|p| p.entity_index == player)
        .unwrap().player_color_index() as usize;
    for index in 0..sim.runtime.objectives.len() {
        for stage in 0..sim.runtime.objectives[index].stage_selectors.len() {
            sim.runtime.set_level2_objective_stage_selector_for(index, stage, 0);
        }
        sim.runtime.set_level2_objective_level_end_for(index, 0);
    }
    sim.runtime.set_level2_objective_text_selector_for(owner, 1);
    sim.runtime.set_level2_objective_stage_selector_for(owner, 1, 1);
    let ui = crate::SOURCE_SPELL_INDEX_MAP.iter().position(|family| *family == 9).unwrap() as i8;
    sim.step_outer_turn(PlayerCommand { opcode: 31, argument_1: ui, argument_2: 2,
        ..PlayerCommand::default() }, 0);
    let holder = sim.shared_spells().local_player().enabled_spell_holders[9];
    assert_ne!(holder, 0);
    assert_eq!(sim.shared_spells().pickup(holder).unwrap().level_gate, 2);
    assert_eq!(sim.runtime.objectives[owner].stage_selectors[1], 1);

    let event_start = sim.economy().events().len();
    let mut fired = 0;
    let mut damaged = false;
    let mut fatal = false;
    let mut prekill = false;
    let mut kill_counter = None;
    let mut hidden = false;
    let mut reclaimed = false;
    let mut completed = false;
    let mut reward = None;
    // Only the player's aim/position and resource prerequisites are controlled;
    // all casts, impacts, actor life, phase, RNG, death and reclaim remain live.
    for turn in 0..1600 {
        let actor = sim.source_arena().resolve(handle);
        let prekill_before = matches!(actor, Some(SourceEntityView::Imported(row))
            if row.action == 132 && word(row, 40) == player);
        let kills_before = sim.economy().authored_bee_kill_census(player);
        let command = if let Some(SourceEntityView::Imported(row)) = actor
            && row.life >= 0 && turn % 24 == 0
        {
            let aim = Position { z: row.position.z.wrapping_add(word(row, 86) as i16), ..row.position };
            let mut origin = Position { x: aim.x.wrapping_add(256),
                y: aim.y.wrapping_add(768), z: aim.z.wrapping_sub(sim.state.entity_fov) };
            origin.z = origin.z.max(sim.canonical_terrain().ground_height(origin).wrapping_add(256));
            sim.set_level2_entity_position(player, origin).unwrap();
            let launch = Position { x: origin.x.wrapping_sub(256),
                z: origin.z.wrapping_add(sim.state.entity_fov), ..origin };
            let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(launch, aim);
            sim.set_level2_player_aim_for_test(yaw, pitch);
            fired += 1;
            PlayerCommand { flags: 0x10, ..PlayerCommand::default() }
        } else { PlayerCommand::default() };
        let was_hidden = hidden && !reclaimed;
        sim.step_outer_turn(command, 0);
        assert!(sim.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "owner18 Meteor/death dispatch: {:?}", sim.dispatch_events());
        if prekill_before {
            let after = sim.economy().authored_bee_kill_census(player);
            assert_eq!(after, kills_before.wrapping_add(1),
                "source PreKill1C890 credits this hostile model16 to its actual player attacker once");
            kill_counter = Some((kills_before, after));
        }
        if was_hidden {
            assert!(sim.source_arena().resolve(handle).is_none(), "the immediately following source pass reclaims the hidden generation");
        }
        if let Some(SourceEntityView::Imported(row)) = sim.source_arena().resolve(handle) {
            damaged |= row.life < initial.life && word(row, 42) == player;
            fatal |= row.life < 0;
            prekill |= row.action == 132 || row.action == 133;
            hidden |= row.drawing_disabled();
            if row.life >= 0 { assert_ne!(sim.runtime.objectives[owner].stage_selectors[1], 2); }
        } else { reclaimed = true; }
        completed |= sim.runtime.objectives[owner].stage_selectors[1] == 2;
        reward = reward.or_else(|| sim.economy().events()[event_start..].iter().find_map(|event| match event {
            Level2EconomyEvent::ManaSphereSpawned { source_entity_index, sphere_id, mana }
                if *source_entity_index == slot && *mana == initial.mana => u16::try_from(*sphere_id).ok(),
            _ => None,
        }));
        if fatal && prekill && hidden && reclaimed && completed && reward.is_some() { break; }
    }
    assert!(fired > 0 && damaged && fatal && prekill && kill_counter.is_some() && hidden && reclaimed && completed && reward.is_some(),
        "actual Wyvern death closure: casts={fired} damaged={damaged} fatal={fatal} prekill={prekill} kill_counter={kill_counter:?} hidden={hidden} reclaimed={reclaimed} objective={completed} reward={reward:?} row={:?}", sim.source_arena().resolve(handle));
    let reward = reward.unwrap();
    let sphere = *sim.economy().spheres().iter().find(|row| row.source_entity_index == reward && row.active)
        .expect("the actual death reward is a live collectible sphere");
    assert_eq!((sphere.mana, sphere.owner_entity_index, sphere.player_entity_index), (initial.mana, 0, 0));
    let reward_handle = sim.source_arena().handle(reward).unwrap();
    let root_handle = sim.source_arena().handle(castle).unwrap();
    for _ in 0..128 {
        let sphere = sim.economy().spheres().iter().find(|row| row.source_entity_index == reward
            && row.active && !row.pending_delete).expect("the actual death sphere remains available");
        if sphere.velocity_x == 0 && sphere.velocity_y == 0 && sphere.vertical_velocity == 0 { break; }
        sim.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert_eq!(sim.source_arena().handle(reward), Some(reward_handle));
    assert!(sim.economy_mut().set_castle_mana_prerequisite_for_test(castle, 100_000),
        "leave capacity for the real collected reward after Meteor combat");
    let capacity_before = sim.economy().owner_max_mana_capacity(player);
    let castle_mana_before = sim.economy().castle_for_owner(player).unwrap().mana;
    let possession = sim.shared_spells().local_player().enabled_spell_holders[1];
    assert_ne!(possession, 0, "the declared opening carry supplies the real Possession holder");
    for _ in 0..64 {
        let holder = sim.shared_spells().pickup(possession).unwrap();
        if holder.active_value == 0 && sim.shared_spells().player_entity().mana >= holder.max_mana { break; }
        sim.step_outer_turn(PlayerCommand::default(), 0);
    }
    let holder = sim.shared_spells().pickup(possession).unwrap();
    assert!(holder.active_value == 0 && sim.shared_spells().player_entity().mana >= holder.max_mana);
    let (approach, target, yaw, pitch) = aim_public_possession(sim, player, reward);
    let possession_ui = crate::SOURCE_SPELL_INDEX_MAP.iter().position(|family| *family == 1).unwrap() as i8;
    let reward_before_possession = sim.economy().spheres().iter()
        .find(|row| row.source_entity_index == reward && row.active).copied();
    let mut possession_trace = Vec::new();
    sim.step_outer_turn(PlayerCommand { opcode: 6, argument_1: possession_ui, flags: 0x40,
        ..PlayerCommand::default() }, 0);
    let mut flyer = None;
    let mut effect = None;
    let mut claimed = false;
    for turn in 0..64 {
        if let Some(row) = sim.economy().possession_projectiles().iter()
            .find(|row| row.source_id_entity_index == player) {
            flyer = flyer.or_else(|| sim.source_arena().handle(row.source_entity_index));
        }
        if let Some(row) = sim.economy().possession_effects().iter().find(|row|
            row.source_id_entity_index == player
                && flyer.is_some_and(|handle| handle.source_slot == row.projectile_entity_index)) {
            effect = effect.or_else(|| sim.source_arena().handle(row.source_entity_index));
        }
        claimed |= sim.economy().events()[event_start..].iter().any(|event| matches!(event,
            Level2EconomyEvent::ManaSphereClaimed { sphere_id, owner_entity_index }
                if *sphere_id == u32::from(reward) && *owner_entity_index == player));
        let shot = sim.economy().possession_projectiles().iter()
            .find(|row| row.source_id_entity_index == player && flyer.is_some_and(|handle|
                handle.source_slot == row.source_entity_index))
            .map(|row| (row.source_entity_index, row.position, row.target_source_entity_index,
                row.yaw, row.pitch, row.life, row.phase, row.active));
        let pulse = sim.economy().possession_effects().iter()
            .find(|row| effect.is_some_and(|handle| handle.source_slot == row.source_entity_index))
            .map(|row| (row.source_entity_index, row.position, row.life, row.phase, row.active));
        let sphere_now = sim.economy().spheres().iter()
            .find(|row| row.source_entity_index == reward && row.active)
            .map(|row| (row.position, row.mana, row.owner_entity_index, row.player_entity_index,
                row.incoming_owner_entity_index, row.incoming_possession_payload, row.assigned_defender_slot,
                row.control_flags, row.pending_delete));
        if turn < 16 || shot.is_some() || pulse.is_some() {
            possession_trace.push((turn, shot, pulse, sphere_now));
        }
        if claimed { break; }
        sim.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert!(flyer.is_some() && effect.is_some() && claimed,
        "public Possession must claim the actual Wyvern death sphere: flyer={flyer:?} effect={effect:?} claimed={claimed} reward_handle={reward_handle:?} current_handle={:?} pre_cast_reward={reward_before_possession:?} approach={approach:?} target={target:?} yaw={yaw} pitch={pitch} trace={possession_trace:#?} sphere_events={:?}",
        sim.source_arena().handle(reward), sim.economy().events()[event_start..].iter().filter(|event| matches!(event,
            Level2EconomyEvent::ManaSphereClaimed { .. } | Level2EconomyEvent::ManaSpheresMerged { .. }
            | Level2EconomyEvent::CastleDefenderCollectedSphere { .. })).collect::<Vec<_>>());
    assert_eq!(sim.source_arena().handle(reward), Some(reward_handle));
    assert!(sim.economy().spheres().iter().any(|row| row.active && row.source_entity_index == reward
        && row.owner_entity_index == player && row.player_entity_index == player));
    let mut collector = None;
    let mut deposited = false;
    let mut credited = false;
    let mut census_seen = false;
    let mut census_capacity = None;
    let mut successful_census = None;
    let mut retirement_end = None;
    let mut contested = false;
    let mut recasts = 0;
    for turn in 0..2048 {
        if sim.source_arena().handle(reward) != Some(reward_handle) && retirement_end.is_none() {
            retirement_end = Some(sim.economy().events().len());
        }
        if sim.source_arena().handle(reward) == Some(reward_handle) {
            let economy = sim.economy();
            let matching = economy.owner_mana_rows.iter().filter(|row| row.active
                && row.source_entity_index == reward && row.player_entity_index == player && row.mana == sphere.mana).count();
            if matching == 1 {
                census_seen = true;
                let base = economy.owner_base_max_mana.get(&player).copied().unwrap_or_default();
                let expected = economy.owner_mana_rows.iter().filter(|row| row.active && row.player_entity_index == player)
                    .fold(base, |total, row| total.saturating_add(row.mana));
                let projected = economy.owner_max_mana_capacity(player);
                let player_max = sim.entities()[usize::from(player)].max_mana;
                census_capacity = Some((base, expected, projected, player_max));
                if projected == expected && player_max == projected {
                    credited = true;
                    successful_census.get_or_insert((base, expected, projected, player_max));
                }
            }
        }
        let collection_end = retirement_end.unwrap_or_else(|| sim.economy().events().len());
        for event in &sim.economy().events()[event_start..collection_end] {
            if let Level2EconomyEvent::CastleDefenderCollectedSphere { sphere_id, defender_slot, mana } = event
                && *sphere_id == u32::from(reward) && *mana == sphere.mana
                && sim.economy().castle_children().iter().any(|row| row.source_entity_index == *defender_slot
                    && row.source_class == 3 && row.model == 3 && row.id_entity_index == player) {
                collector = Some(*defender_slot);
            }
        }
        deposited |= sim.economy().events()[event_start..].iter().any(|event| matches!(event,
            Level2EconomyEvent::CastleDefenderArrived { defender_slot, castle_id, mana }
                if Some(*defender_slot) == collector && *castle_id == castle && *mana == sphere.mana));
        if deposited && credited && retirement_end.is_some() { break; }
        let rival_owned = sim.source_arena().handle(reward) == Some(reward_handle)
            && sim.economy().spheres().iter().any(|row| row.active && row.source_entity_index == reward
                && row.player_entity_index != 0 && row.player_entity_index != player);
        contested |= rival_owned;
        let holder_ready = sim.shared_spells().pickup(possession).is_some_and(|holder|
            holder.active_value == 0 && sim.shared_spells().player_entity().mana >= holder.max_mana);
        let own_claim_in_flight = sim.economy().possession_projectiles().iter()
            .any(|row| row.active && row.source_id_entity_index == player)
            || sim.economy().possession_effects().iter()
                .any(|row| row.active && row.source_id_entity_index == player);
        let command = if rival_owned && holder_ready && !own_claim_in_flight {
            // A rival may actually claim the earned sphere while its collector
            // travels. Recapture with another public cast; do not write either
            // owner's relation or alter the rival's action or schedule.
            aim_public_possession(sim, player, reward);
            recasts += 1;
            PlayerCommand { opcode: 6, argument_1: possession_ui, flags: 0x40,
                ..PlayerCommand::default() }
        } else { PlayerCommand::default() };
        sim.step_outer_turn(command, 0);
        if sim.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })) {
            let live = &sim.session.state.live;
            let context = crate::native_world::source_target::Context {
                spells: Some(&live.shared_spells), map_type: Some(live.terrain.map_type()),
                player: Some((live.core.state.entity_index, live.core.state.position, live.player_life)),
                player_orientation: Some((live.core.state.yaw, live.core.state.roll_accumulator)),
                particles: Some(&sim.session.definition.presentation.particles),
                ..crate::native_world::source_target::Context::from_ordinary(&live.goats, &live.ordinary)
            };
            panic!("Wyvern reward collection dispatch turn={turn}: events={:?}; target31 cursor={:?} view={:?} raw={:?}; Cymmerian39={:?}; reward={:?}; castle={:?}",
                sim.dispatch_events(), sim.economy().live_arena_cursor_row(31),
                crate::native_world::source_target::read(&live.economy, context, 31),
                sim.entities().get(31), sim.entities().get(39),
                sim.economy().spheres().iter().find(|row| row.source_entity_index == reward),
                sim.economy().castle_for_owner(player));
        }
    }
    assert!(credited && collector.is_some() && deposited && retirement_end.is_some(),
        "the actual Wyvern reward must enter owner census and reach the Castle: credited={credited} census_seen={census_seen} pre_claim_capacity={capacity_before} census_capacity={census_capacity:?} collector={collector:?} deposited={deposited} contested={contested} recasts={recasts} sphere={:?} castle={:?}",
        sim.economy().spheres().iter().find(|row| row.source_entity_index == reward), sim.economy().castle_for_owner(player));
    assert_eq!(sim.source_arena().handle(castle), Some(root_handle));
    assert_eq!(sim.economy().castle_for_owner(player).unwrap().mana, castle_mana_before + sphere.mana);
    let collection_end = retirement_end.unwrap();
    assert_eq!(sim.economy().events()[event_start..collection_end].iter().filter(|event| matches!(event,
        Level2EconomyEvent::CastleDefenderCollectedSphere { sphere_id, .. } if *sphere_id == u32::from(reward))).count(), 1);
    assert_eq!(sim.economy().events()[event_start..].iter().filter(|event| matches!(event,
        Level2EconomyEvent::CastleDefenderArrived { defender_slot, castle_id, mana }
            if Some(*defender_slot) == collector && *castle_id == castle && *mana == sphere.mana)).count(), 1);
    eprintln!("owner18 Wyvern{slot} Meteor casts={fired} kill_counter={kill_counter:?} death sphere={reward} mana={} pre_claim_capacity={capacity_before} successful_census={successful_census:?} contested={contested} recasts={recasts} collector={collector:?} Castle={castle} objective=complete", sphere.mana);
}
