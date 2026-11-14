use super::*;
use crate::campaign::{CampaignSession, journal::CampaignEffect, spec::CampaignSelection};
use std::path::PathBuf;

pub(crate) type RivalTestGame =
    crate::game_session::GameSession<crate::campaign::backend::Mc2CampaignBackend>;

pub(crate) fn rival_test_sim(game: &RivalTestGame) -> &Mc2Simulation {
    game.backend().compatibility_campaign().simulation()
}
pub(crate) fn rival_test_sim_mut(game: &mut RivalTestGame) -> &mut Mc2Simulation {
    game.backend_mut()
        .compatibility_campaign_mut_for_test()
        .simulation_mut()
}
pub(crate) fn rival_test_step(game: &mut RivalTestGame) {
    game.advance(
        game.identity().guard(),
        crate::game_session::GameCommand::default(),
    )
    .unwrap();
    assert!(
        rival_test_sim(game)
            .dispatch_events()
            .iter()
            .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
        "rival source dispatch: {:?}",
        rival_test_sim(game).dispatch_events()
    );
}
pub(crate) fn rival_test_participation(game: &RivalTestGame, actor: u16) -> Option<u8> {
    rival_test_sim(game)
        .players()
        .iter()
        .find(|row| row.entity_index == actor)
        .unwrap()
        .objective_player_state()
}
pub(crate) fn rival_test_held_spell_generations(
    game: &RivalTestGame,
    actor: u16,
) -> Vec<crate::campaign::source_arena::SourceEntityHandle> {
    rival_test_sim(game)
        .players()
        .iter()
        .find(|row| row.entity_index == actor)
        .unwrap()
        .enabled_spell_holders
        .into_iter()
        .filter(|slot| *slot > 1)
        .map(|slot| rival_test_sim(game).source_arena().handle(slot).unwrap())
        .collect()
}
pub(crate) fn rival_test_contact(game: &mut RivalTestGame, target: u16, owner: u16, life: i32) {
    assert!(life >= 0, "a selected contact requires a living source target");
    let payload = u16::try_from(life.saturating_add(1)).unwrap_or(u16::MAX);
    crate::native_world::area_damage::contact(
        &mut rival_test_sim_mut(game).session.state.live.economy,
        None,
        target,
        payload,
        owner,
    )
    .unwrap();
}
pub(crate) fn rival_test_destroy_castle(
    game: &mut RivalTestGame,
    owner: u16,
    attacker: u16,
) -> crate::campaign::source_arena::SourceEntityHandle {
    let castle = *rival_test_sim(game)
        .economy()
        .castle_for_owner(owner)
        .expect("current real rival Castle");
    let handle = rival_test_sim(game)
        .source_arena()
        .handle(castle.source_entity_index)
        .unwrap();
    let mut contacts = 0;
    let mut observed_levels = vec![castle.level];
    for _ in 0..2048 {
        let Some(current) = rival_test_sim(game)
            .economy()
            .castle_for_owner(owner)
            .copied()
        else {
            eprintln!(
                "rival owner={owner} Castle final loss: {handle:?}, levels={observed_levels:?}, source_packets={contacts}"
            );
            return handle;
        };
        assert_eq!(
            rival_test_sim(game)
                .source_arena()
                .handle(current.source_entity_index),
            Some(handle),
            "one original Castle generation owns all stage transitions"
        );
        if observed_levels.last().copied() != Some(current.level) {
            observed_levels.push(current.level);
        }
        if current.action == 4
            && current.timer == 0
            && current.life >= 0
            && current.pending_attacker_entity_index == 0
        {
            // High-tier life exceeds one source u16 packet. Wait for the
            // previous owner to be consumed; retain any ownerless residue.
            rival_test_contact(game, current.source_entity_index, attacker, current.life);
            contacts += 1;
        }
        rival_test_step(game);
    }
    panic!(
        "real Castle tiers did not reach final owner-link loss: owner={owner} old={handle:?} levels={observed_levels:?} current={:?}",
        rival_test_sim(game).economy().castle_for_owner(owner)
    );
}
pub(crate) fn rival_test_wait_unprotected(game: &mut RivalTestGame, actor: u16) {
    let handle = rival_test_sim(game).source_arena().handle(actor).unwrap();
    for _ in 0..512 {
        let state = rival_test_sim(game);
        let row = &state.entities()[usize::from(actor)];
        let common = state
            .session
            .state
            .live
            .nyphur_dispatcher
            .get(handle)
            .unwrap()
            .common_upkeep(row.phase);
        if common.word_0x159 == 0 && !common.high_upkeep {
            return;
        }
        rival_test_step(game);
    }
    let state = rival_test_sim(game);
    let row = &state.entities()[usize::from(actor)];
    let common = state
        .session
        .state
        .live
        .nyphur_dispatcher
        .get(handle)
        .unwrap()
        .common_upkeep(row.phase);
    panic!(
        "rival protection did not expire naturally: actor={actor} countdown={} high_upkeep={}",
        common.word_0x159, common.high_upkeep
    );
}

#[test]
fn galiphur_four_rival_banishments_produce_the_real_exit() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::campaign::source_arena::SourceEntityHandle;
    use crate::game_session::{GameCommand, GameCompletionStatus, GameSession};
    use crate::level_definition::ObjectiveCondition;
    use crate::native_world::{Mc2DispatchEvent, RuntimeRelation};

    let package = std::env::var_os("MC_LEVEL13_PACKAGE").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/volcano-quake-20261104-integration/current-owner-package/level13"),
        PathBuf::from,
    );
    let definition = load_mc2_campaign(&package, &crate::game_definition::default_catalog().unwrap()).unwrap();
    let mut game = GameSession::open(&Mc2CampaignAdapter, std::sync::Arc::new(definition), 2205,
        Mc2CampaignInputs::new(&package)).unwrap();
    let sim = rival_test_sim(&game);
    let player = sim.state.entity_index;
    let player_color = usize::from(sim.state.player_color_index);
    let objectives = game.definition().level_definition().unwrap().objectives();
    assert_eq!(objectives.len(), 4);
    assert!(objectives.iter().all(|goal| matches!(goal.condition(), ObjectiveCondition::DefeatRivalWizard(_))));
    let rivals: Vec<u16> = (1..=4).map(|color| {
        let owner = sim.players().iter().find(|p| p.player_color_index() == color && p.is_ai)
            .expect("the authored selector resolves to the active rival roster");
        owner.entity_index
    }).collect();
    for (index, actor) in rivals.iter().copied().enumerate() {
        assert_eq!(sim.runtime().stages[index].kind, 3);
        assert_eq!(sim.runtime().stages[index].relation, RuntimeRelation::LiveEntity(actor));
        assert_eq!(rival_test_participation(&game, actor), Some(1));
    }
    assert_eq!(sim.runtime().objectives[player_color].stage_selectors[..4], [1, 1, 1, 1]);
    let rival_handles: Vec<SourceEntityHandle> = rivals.iter().map(|slot|
        sim.source_arena().handle(*slot).unwrap()).collect();
    let mut castle_hits = std::collections::BTreeMap::<u16, (SourceEntityHandle, u8)>::new();
    let mut actor_hits = std::collections::BTreeSet::<u16>::new();
    let mut released_holders = Vec::<SourceEntityHandle>::new();
    let mut exit = None;
    let mut group_six = 0;

    // Controlled arrived damage contacts and player positioning leave every
    // Castle tier, protection countdown, actor callback and objective census
    // in the ordinary live schedule. Other actors continue their own turns.
    for _ in 0..8192 {
        let state = rival_test_sim(&game);
        if exit.is_none() {
            if let Some(actor) = rivals.iter().copied()
                .find(|actor| rival_test_participation(&game, *actor) == Some(1))
            {
                if let Some(castle) = state.economy().castle_for_owner(actor).copied() {
                    let handle = state.source_arena().handle(castle.source_entity_index).unwrap();
                    let hit = (handle, castle.level);
                    if castle.action == 4 && castle.timer == 0 && castle.life > 0
                        && castle_hits.get(&actor) != Some(&hit)
                    {
                        castle_hits.insert(actor, hit);
                        rival_test_contact(&mut game, castle.source_entity_index, player, castle.life);
                    }
                } else {
                    castle_hits.remove(&actor);
                    let state = rival_test_sim(&game);
                    let row = &state.entities()[usize::from(actor)];
                    let handle = state.source_arena().handle(actor).unwrap();
                    let upkeep = state.session.state.live.nyphur_dispatcher.get(handle).unwrap()
                        .common_upkeep(row.phase);
                    if upkeep.word_0x159 == 0 && !upkeep.high_upkeep && row.life > 0
                        && actor_hits.insert(actor)
                    {
                        let life = row.life;
                        released_holders.extend(rival_test_held_spell_generations(&game, actor));
                        rival_test_contact(&mut game, actor, player, life);
                    }
                }
            }
        }
        rival_test_step(&mut game);
        let state = rival_test_sim(&game);
        for event in state.dispatch_events() {
            if let Mc2DispatchEvent::AuthoredGroupActivated { group: 6, authored_indices, .. } = event {
                assert_eq!(authored_indices.as_ref(), [402, 403, 404, 405, 406, 407, 408]);
                group_six += 1;
                let trigger = state.economy().authored_switch_producers().iter()
                    .find(|row| row.active && row.authored_index == 402 && row.model == 12 && row.action == 12)
                    .expect("group 6 publishes the real authored exit trigger");
                assert!(state.economy().authored_class14().iter()
                    .any(|row| row.active && row.authored_index == 408),
                    "the authored portal is published in the same group");
                exit = Some((trigger.position, state.source_arena().handle(trigger.source_entity_index).unwrap()));
            }
        }
        if exit.is_some() {
            if state.runtime().objectives[player_color].stage_selectors[..4] == [2, 2, 2, 2] {
                break;
            }
        }
    }
    let (exit_position, exit_handle) = exit.expect("all four source banishments must publish group 6");
    let state = rival_test_sim(&game);
    assert_eq!(group_six, 1);
    assert_eq!(state.runtime().objectives[player_color].stage_selectors[..4], [2, 2, 2, 2]);
    assert_eq!(state.runtime().objectives[player_color].is_level_end, 1);
    for (actor, handle) in rivals.iter().copied().zip(rival_handles) {
        assert_eq!(rival_test_participation(&game, actor), Some(0));
        assert_eq!(state.source_arena().handle(actor), Some(handle));
        assert!(state.economy().castle_for_owner(actor).is_none());
    }
    for _ in 0..320 {
        if released_holders.iter().all(|handle| rival_test_sim(&game).source_arena().resolve(*handle).is_none()) { break; }
        rival_test_step(&mut game);
    }
    assert!(released_holders.iter().all(|handle| rival_test_sim(&game).source_arena().resolve(*handle).is_none()),
        "the original scattered holder generations must reclaim naturally");
    let state = rival_test_sim(&game);
    let sequence = state.level_exit_sequence();
    if !sequence.active && !sequence.complete {
        assert!(state.source_arena().resolve(exit_handle).is_some(), "exit was captured at publication: {exit_position:?}");
        game.backend_mut().compatibility_campaign_mut_for_test().simulation_mut()
            .set_level2_entity_position(player, exit_position).unwrap();
    }
    for _ in 0..700 {
        if matches!(game.completion_status(game.identity().guard()).unwrap().value,
            GameCompletionStatus::Completed { .. }) { break; }
        rival_test_step(&mut game);
    }
    assert!(matches!(game.completion_status(game.identity().guard()).unwrap().value,
        GameCompletionStatus::Completed { .. }));
    assert_eq!(rival_test_sim(&game).source_exit_result_byte(), 0x0a);
}

pub(crate) fn complete_tklom_rival_objectives_for_test(mut game: &mut RivalTestGame) {
    use self::{
        RivalTestGame as Game, rival_test_contact as contact,
        rival_test_destroy_castle as destroy_castle,
        rival_test_held_spell_generations as held_spell_generations,
        rival_test_participation as participation, rival_test_sim as sim, rival_test_step as step,
        rival_test_wait_unprotected as wait_unprotected,
    };
    use crate::level_definition::ObjectiveCondition;
    use crate::native_world::{Mc2DispatchEvent, area_damage};

    let objectives = game.definition().level_definition().unwrap().objectives();
    assert_eq!(objectives.len(), 2);
    assert!(objectives.iter().all(|objective| matches!(
        objective.condition(),
        ObjectiveCondition::DefeatRivalWizard(_)
    )));
    let player = sim(&game).state.entity_index;
    let rahn = 272;
    let nyphur = 266;
    let castle = 271;
    let gate = 1;
    let owner = usize::from(sim(&game).state.player_color_index);
    let rahn_handle = sim(&game).source_arena().handle(rahn).unwrap();
    let nyphur_handle = sim(&game).source_arena().handle(nyphur).unwrap();
    let castle_handle = sim(&game).source_arena().handle(castle).unwrap();
    let gate_handle = sim(&game).source_arena().handle(gate).unwrap();
    assert_eq!(player, 265);
    assert_eq!(
        (
            sim(&game).runtime().stages[0].relation,
            sim(&game).runtime().stages[1].relation
        ),
        (
            crate::native_world::RuntimeRelation::LiveEntity(rahn),
            crate::native_world::RuntimeRelation::LiveEntity(nyphur)
        )
    );
    assert_eq!(
        sim(&game).runtime().objectives[owner].stage_selectors[..2],
        [1, 1]
    );
    assert_eq!(
        (participation(&game, rahn), participation(&game, nyphur)),
        (Some(1), Some(1))
    );
    assert!(sim(&game).economy().castle_for_owner(rahn).is_none());
    assert_eq!(
        sim(&game)
            .economy()
            .castle_for_owner(nyphur)
            .unwrap()
            .source_entity_index,
        castle
    );
    let gate_row = &sim(&game).entities()[usize::from(gate)];
    assert_eq!(
        (
            gate_row.class,
            gate_row.model,
            gate_row.action,
            gate_row.source_id
        ),
        (11, 4, 4, 1)
    );

    // Allow Rahn's ordinary AI to construct its real Castle. Selected arrived
    // contacts isolate real damage consumers without staging health, owner
    // relations, objective state, participation or protection countdowns.
    for _ in 0..64 {
        if sim(&game).economy().castle_for_owner(rahn).is_some() {
            break;
        }
        step(&mut game);
    }
    assert!(
        sim(&game).economy().castle_for_owner(rahn).is_some(),
        "Rahn's real AI produces a Castle"
    );
    wait_unprotected(&mut game, rahn);
    let rahn_castle = destroy_castle(&mut game, rahn, player);
    let rahn_common = sim(&game)
        .session
        .state
        .live
        .nyphur_dispatcher
        .get(rahn_handle)
        .unwrap()
        .common_upkeep(sim(&game).entities()[usize::from(rahn)].phase);
    assert_eq!(
        rahn_common.word_0x159, 0,
        "the actual Castle loss leaves Rahn's current contact admissible"
    );
    let mut released_holders = held_spell_generations(&game, rahn);
    let rahn_life = sim(&game).entities()[usize::from(rahn)].life;
    contact(&mut game, rahn, player, rahn_life);
    let observe_rahn = |game: &Game, turn: usize| {
        let state = sim(game);
        let row = &state.entities()[usize::from(rahn)];
        let common = state
            .session
            .state
            .live
            .nyphur_dispatcher
            .get(rahn_handle)
            .unwrap()
            .common_upkeep(row.phase);
        eprintln!(
            "T'Klom Rahn turn={turn} raw_life={} action={} flags={:#x} raw_mailbox=({},{}) canonical_mailbox={:?} typed_life={} regen={} typed_mailbox={:?} linked_castle={:?} participation={:?}",
            row.life,
            row.action,
            row.control_flags,
            row.source_upkeep_payload(),
            row.source_upkeep_word_62(),
            area_damage::source_bounty(state.source_arena(), None, rahn),
            common.life,
            common.life_regen,
            common.source_mailbox,
            state
                .economy()
                .castle_for_owner(rahn)
                .map(|row| row.source_entity_index),
            participation(game, rahn)
        );
    };
    observe_rahn(&game, 0);
    for turn in 0..512 {
        if participation(&game, rahn) == Some(0) {
            break;
        }
        step(&mut game);
        if turn < 8 || turn == 511 {
            observe_rahn(&game, turn + 1);
        }
    }
    assert_eq!(sim(&game).source_arena().handle(rahn), Some(rahn_handle));
    assert_eq!(
        participation(&game, rahn),
        Some(0),
        "Rahn's source death reaches banishment"
    );
    assert_eq!(sim(&game).entities()[usize::from(rahn)].action, 3);
    assert_eq!(participation(&game, nyphur), Some(1));
    assert!(
        sim(&game).source_arena().resolve(rahn_castle).is_none(),
        "Rahn's original Castle is naturally reclaimed after actor contact"
    );
    assert_eq!(
        sim(&game).runtime().objectives[owner].stage_selectors[..2],
        [2, 1]
    );
    assert_eq!(sim(&game).runtime().objectives[owner].is_level_end, 0);
    assert_eq!(sim(&game).source_arena().handle(gate), Some(gate_handle));
    assert!(
        !sim(&game)
            .economy()
            .authored_switch_producers()
            .iter()
            .any(|row| row.authored_index == 132 && row.active)
    );

    assert_eq!(
        sim(&game).source_arena().handle(castle),
        Some(castle_handle)
    );
    wait_unprotected(&mut game, nyphur);
    let nyphur_castle = destroy_castle(&mut game, nyphur, player);
    assert_eq!(nyphur_castle, castle_handle);
    assert_eq!(participation(&game, nyphur), Some(1));
    let nyphur_common = sim(&game)
        .session
        .state
        .live
        .nyphur_dispatcher
        .get(nyphur_handle)
        .unwrap()
        .common_upkeep(sim(&game).entities()[usize::from(nyphur)].phase);
    assert_eq!(
        nyphur_common.word_0x159, 0,
        "the actual Castle loss leaves Nyphur's current contact admissible"
    );
    released_holders.extend(held_spell_generations(&game, nyphur));
    let nyphur_life = sim(&game).entities()[usize::from(nyphur)].life;
    contact(&mut game, nyphur, player, nyphur_life);
    let mut group_one_count = 0;
    for _ in 0..512 {
        step(&mut game);
        group_one_count += sim(&game)
            .dispatch_events()
            .iter()
            .filter(|event| {
                matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group: 1, authored_indices, .. }
                if *source_slot == gate && authored_indices.contains(&132))
            })
            .count();
        if sim(&game)
            .economy()
            .authored_switch_producers()
            .iter()
            .any(|row| row.active && row.authored_index == 132)
        {
            break;
        }
    }
    assert_eq!(
        sim(&game).source_arena().handle(nyphur),
        Some(nyphur_handle)
    );
    assert_eq!(participation(&game, nyphur), Some(0));
    assert!(
        sim(&game).source_arena().resolve(nyphur_castle).is_none(),
        "Nyphur's original Castle is naturally reclaimed after actor contact"
    );
    assert_eq!(sim(&game).entities()[usize::from(nyphur)].action, 3);
    assert_eq!(
        sim(&game).runtime().objectives[owner].stage_selectors[..2],
        [2, 2]
    );
    assert_eq!(sim(&game).runtime().objectives[owner].is_level_end, 1);
    assert_eq!(group_one_count, 1);
    let trigger = *sim(&game)
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|row| row.active && row.authored_index == 132)
        .expect("actual T'Klom exit trigger");
    assert_eq!((trigger.model, trigger.action), (12, 12));

    // Source death scatters each original spell holder into its ordinary
    // timed pickup lifetime. Continue normal turns through following reclaim.
    for _ in 0..320 {
        if released_holders
            .iter()
            .all(|handle| sim(&game).source_arena().resolve(*handle).is_none())
        {
            break;
        }
        step(&mut game);
    }
    assert!(
        released_holders
            .iter()
            .all(|handle| sim(&game).source_arena().resolve(*handle).is_none()),
        "original rival spell-holder generations must reclaim naturally: {released_holders:?}"
    );
    assert!(
        sim(&game)
            .economy()
            .authored_switch_producers()
            .iter()
            .any(|row| row.active
                && row.authored_index == 132
                && row.model == 12
                && row.action == 12)
    );
}

#[test]
fn tklom_selected_rival_contacts_reach_ordinary_exit() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::game_session::{GameCommand, GameCompletionStatus, GameSession};
    use crate::native_world::Mc2DispatchEvent;

    let package = std::env::var_os("MC_LEVEL5_PACKAGE").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
                "../../../generated/dragon-bee-20261105-integration/level5-owner-current/level5",
            )
        },
        PathBuf::from,
    );
    let definition = load_mc2_campaign(
        &package,
        &crate::game_definition::default_catalog().unwrap(),
    )
    .unwrap();
    let mut game = GameSession::open(
        &Mc2CampaignAdapter,
        std::sync::Arc::new(definition),
        2205,
        Mc2CampaignInputs::new(&package),
    )
    .unwrap();
    complete_tklom_rival_objectives_for_test(&mut game);
    let simulation = game.backend().compatibility_campaign().simulation();
    let player = simulation.state.entity_index;
    let trigger = *simulation
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|row| row.active && row.authored_index == 132)
        .expect("actual T'Klom exit trigger after original holder reclaim");

    // Declared player position at the real trigger; the exit's scheduler,
    // countdown and completion result remain unmodified.
    game.backend_mut()
        .compatibility_campaign_mut_for_test()
        .simulation_mut()
        .set_level2_entity_position(player, trigger.position)
        .unwrap();
    for _ in 0..700 {
        if matches!(
            game.completion_status(game.identity().guard())
                .unwrap()
                .value,
            GameCompletionStatus::Completed { .. }
        ) {
            break;
        }
        game.advance(game.identity().guard(), GameCommand::default())
            .unwrap();
        assert!(
            game.backend()
                .compatibility_campaign()
                .simulation()
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
    }
    assert!(matches!(
        game.completion_status(game.identity().guard())
            .unwrap()
            .value,
        GameCompletionStatus::Completed { .. }
    ));
    assert_eq!(
        game.backend()
            .compatibility_campaign()
            .simulation()
            .source_exit_result_byte(),
        0x0a
    );
}

#[test]
fn cave_four_ordinary_lethal_spider_hit_scatters_holders_and_binds_death_mana_without_ending_flow()
{
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::campaign::source_arena::CreatureRootOrigin;
    use crate::game_session::{GameCommand, GameSession, SpellAssignment, SpellAssignmentHand};
    use crate::level_definition::{CreatureKind, SPIDER_CLASS_PROPERTIES};
    use crate::native_world::{area_damage, creatures::spider::web};

    type Game = GameSession<crate::campaign::backend::Mc2CampaignBackend>;
    fn sim(game: &Game) -> &Mc2Simulation {
        game.backend().compatibility_campaign().simulation()
    }
    fn sim_mut(game: &mut Game) -> &mut Mc2Simulation {
        game.backend_mut()
            .compatibility_campaign_mut_for_test()
            .simulation_mut()
    }
    fn step(game: &mut Game, command: GameCommand) {
        game.advance(game.identity().guard(), command).unwrap();
        assert!(
            sim(game)
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "{:?}",
            sim(game).dispatch_events()
        );
    }
    const GROUP: &[u16] = &[
        8, 9, 10, 11, 12, 137, 138, 139, 141, 250, 251, 255, 601, 602, 724, 725, 726,
    ];
    let package = std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES")
        .map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj")
            },
            PathBuf::from,
        )
        .join("level4");
    let definition = load_mc2_campaign(
        &package,
        &crate::game_definition::default_catalog().unwrap(),
    )
    .unwrap();
    let mut game = GameSession::open(
        &Mc2CampaignAdapter,
        std::sync::Arc::new(definition),
        2018,
        Mc2CampaignInputs::new(&package),
    )
    .unwrap();
    let player = sim(&game).state.entity_index;
    let player_handle = sim(&game).source_arena().handle(player).unwrap();
    let safe_position = sim(&game).state.position;
    assert_eq!(
        (
            sim(&game).entities()[usize::from(player)].life,
            sim(&game).entities()[usize::from(player)].max_life
        ),
        (10_000, 10_000)
    );
    for _ in 0..128 {
        if sim(&game).session.state.live.player_protection_ticks == 0 {
            break;
        }
        step(&mut game, GameCommand::default());
    }
    assert_eq!(sim(&game).session.state.live.player_protection_ticks, 0);
    assert_eq!(sim(&game).shared_spells().player_life_regen_for_test(), 5);

    // Declared prior carry, prior objective completion and a real far-away
    // player Castle are entry controls. Human HP, NPCs, source RNG, phases,
    // protection and death/revival countdowns retain their source values.
    let castle = {
        let state = sim_mut(&mut game);
        state.apply_carried_spell_families(&[9], Some(9)).unwrap();
        for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
            state
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
                !state.economy().castle_projectile_site_blocked(
                    *position,
                    state.entities(),
                    state.canonical_terrain(),
                )
            })
            .unwrap();
        let castle = state.economy_mut().build_castle(site, player).unwrap();
        let color = usize::from(state.state.player_color_index);
        state
            .session
            .state
            .live
            .core
            .runtime
            .set_level2_objective_stage_selector_for(color, 0, 2);
        castle
    };
    let switch = sim(&game).source_arena().handle(6).unwrap();
    let mut published = false;
    for _ in 0..8 {
        assert_eq!(sim(&game).source_arena().handle(6), Some(switch));
        step(&mut game, GameCommand::default());
        if sim(&game).dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 6, group: 6, authored_indices, .. }
                if authored_indices.as_ref() == GROUP)) { published = true; break; }
    }
    assert!(
        published,
        "the actual switch must admit the whole unfiltered wave"
    );
    let roots = sim(&game)
        .creature_runtime_census()
        .roots_of_kind(CreatureKind::Spider)
        .filter(|row| row.living)
        .map(|row| (row.origin, row.handle))
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 6);
    let root = roots
        .iter()
        .find_map(|&(origin, handle)| {
            (origin
                == CreatureRootOrigin::AuthoredRecord {
                    authored_record: 137,
                })
            .then_some(handle)
        })
        .unwrap();

    // Real command42 removes the declared Castle. Its ordinary sub5FD00
    // overflow owns this single positive mana unit; no sphere, claimant,
    // collector or NPC state is manufactured for the later death transfer.
    assert!(
        sim_mut(&mut game)
            .economy_mut()
            .set_castle_mana_prerequisite_for_test(castle, 1)
    );
    let event_start = sim(&game).economy().events().len();
    step(
        &mut game,
        GameCommand {
            remove_castle_stage: true,
            ..Default::default()
        },
    );
    let mut owned_sphere = None;
    for _ in 0..64 {
        owned_sphere = sim(&game).economy().events()[event_start..]
            .iter()
            .find_map(|event| match event {
                Level2EconomyEvent::ManaSphereSpawned {
                    sphere_id,
                    source_entity_index,
                    mana,
                } if *source_entity_index == castle && *mana == 1 => {
                    let slot = u16::try_from(*sphere_id).unwrap();
                    sim(&game)
                        .economy()
                        .spheres()
                        .iter()
                        .find(|row| {
                            row.active
                                && !row.pending_delete
                                && row.source_entity_index == slot
                                && row.player_entity_index == player
                        })
                        .and_then(|_| sim(&game).source_arena().handle(slot))
                }
                _ => None,
            });
        if owned_sphere.is_some() {
            break;
        }
        step(&mut game, GameCommand::default());
    }
    let owned_sphere = owned_sphere.expect("actual Castle removal must expose its owned mana");

    // All26 real held rows are constructed through the existing carry seam.
    // This declares prior loadout, not acquisition earned in this encounter.
    let families = (0_u8..26).collect::<Vec<_>>();
    sim_mut(&mut game)
        .apply_carried_spell_families(&families, Some(0))
        .unwrap();
    let held = sim(&game)
        .shared_spells()
        .local_player()
        .enabled_spell_holders
        .map(|slot| {
            sim(&game)
                .source_arena()
                .handle(slot)
                .expect("every declared holder has a current generation")
        });
    assert_eq!(
        held.iter()
            .map(|handle| handle.source_slot)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        26
    );
    let original = sim(&game).entities()[usize::from(root.source_slot)].clone();
    let mut lure = original.position;
    crate::move_entity(&mut lure, original.yaw, 0, 1280);
    lure.z = sim(&game)
        .canonical_terrain()
        .ground_height(lure)
        .wrapping_add(256);
    assert!(
        sim(&game)
            .canonical_terrain()
            .ceiling_height(lure)
            .is_none_or(|ceiling| lure.z <= ceiling.wrapping_sub(256))
    );
    sim_mut(&mut game)
        .set_level2_entity_position(player, lure)
        .unwrap();
    sim_mut(&mut game).state.actual_speed = 0;
    sim_mut(&mut game).state.target_speed = 0;
    let mut hand = lure;
    hand.z = hand
        .z
        .wrapping_add(sim(&game).shared_spells().player_entity().array_fov());
    let aim = crate::level1_mission::projectile_guidance_angles(
        hand,
        Position {
            z: original.position.z.wrapping_add(original.array_fov()),
            ..original.position
        },
    );
    step(
        &mut game,
        GameCommand {
            assign_spell: Some(SpellAssignment {
                spell_id: crate::game_definition::SemanticId::parse("spell.fireball").unwrap(),
                tier: 0,
                hand: SpellAssignmentHand::Left,
                holder_actor: None,
            }),
            ..Default::default()
        },
    );
    sim_mut(&mut game).set_level2_player_aim_for_test(aim.0, aim.1);
    assert_eq!(sim(&game).session.state.live.player_life, 10_000);
    let provocation_holder = sim(&game)
        .shared_spells()
        .local_player()
        .enabled_spell_holders[0];
    let provocation_holder_handle = sim(&game)
        .source_arena()
        .handle(provocation_holder)
        .unwrap();
    let provocation_price = sim(&game)
        .shared_spells()
        .pickup(provocation_holder)
        .unwrap()
        .max_mana;
    assert_eq!(
        provocation_price,
        crate::level_definition::FIREBALL_PROPERTIES.tiers[0].mana_cost
    );
    assert_eq!(
        sim(&game)
            .shared_spells()
            .pickup(provocation_holder)
            .unwrap()
            .mana_regen,
        0
    );
    assert!(!sim(&game).shared_spells().free_spell_usage);
    // Explicit1000 player mana for one real FireballI provocation. C4's
    // LightningI correctly rejected the absent30000-mana Castle; FireballI
    // requires no Castle and queues its actual100 cost after construction.
    // Source HP, life regeneration, NPCs and the fixed1280 lure are unchanged.
    {
        let live = &mut sim_mut(&mut game).session.state.live;
        let actor = &mut live.economy.entities[usize::from(player)];
        assert!(actor.max_mana >= 1000);
        actor.set_mana(1000);
        let actor = actor.clone();
        live.shared_spells.sync_player_entity_from_authority(&actor);
        live.shared_spells.player_mana = actor.mana;
        live.shared_spells.caster.mana = actor.mana;
        live.shared_spells
            .sync_typed_spell_parent_caches_from_player_entity();
        if let Some(fire) = live.core.context.spell_fire.as_mut() {
            fire.caster.mana = actor.mana;
        }
    }
    let mut safe = game.backend().compatibility_campaign().clone();
    safe.simulation_mut()
        .set_level2_entity_position(player, safe_position)
        .unwrap();
    safe.simulation_mut().state.actual_speed = 0;
    safe.simulation_mut().state.target_speed = 0;
    let journal_start = sim(&game).effect_journal().cursor(0).next_sequence;
    web::start_trace_for_roots(&roots.iter().map(|&(_, handle)| handle).collect::<Vec<_>>());
    area_damage::start_fireball_launch_trace();
    area_damage::start_actor_damage_trace(&[player_handle, root]);
    let mut effects = Vec::new();
    let mut consumptions = Vec::new();
    let mut projectiles: Vec<(
        crate::campaign::source_arena::SourceEntityHandle,
        u16,
        u16,
        u16,
        i32,
    )> = Vec::new();
    let mut provocation_visits = Vec::new();
    let mut first_incoming_turn = None;
    let mut provocation_paid = 0_i32;
    let mut payment_visits = Vec::new();
    let mut own_web = false;
    let mut contact_placement = None;
    let mut fatal = None;
    let mut elapsed = 0;
    let mut history = Vec::new();
    let cast_admission_state = |game: &Game| {
        let state = sim(game);
        let live = &state.session.state.live;
        let actor = &live.economy.entities[usize::from(player)];
        let a4 = live.shared_spells.local_player();
        let holder_fields = |row: &crate::native_world::EntitySnapshot| {
            (
                row.index,
                row.class,
                row.model,
                row.action,
                row.level_gate,
                row.active_value,
                row.activation_value,
                row.mana_cost,
                row.max_mana,
                row.mana_regen,
                row.source_spell_parent_entity_index(),
            )
        };
        (
            (
                state.source_arena().handle(player),
                actor.action,
                actor.life,
                actor.mana,
                actor.max_mana,
                actor.mana_regen,
                actor.control_flags,
                actor.special_relation.clone(),
            ),
            (
                a4.slot,
                a4.left_spell_slot_raw,
                a4.left_subspell_index,
                a4.enabled_spell_holders[0],
                u16::from_le_bytes([a4.record[42], a4.record[43]]),
            ),
            live.shared_spells.caster,
            live.shared_spells
                .pickup(provocation_holder)
                .map(holder_fields),
            live.economy
                .entities
                .get(usize::from(provocation_holder))
                .map(holder_fields),
            live.core.context.spell_fire.as_ref().map(|fire| {
                (
                    fire.caster,
                    fire.left_holder_index,
                    fire.holders
                        .iter()
                        .find(|row| row.entity_index == fire.left_holder_index)
                        .copied(),
                    fire.accepted_models.clone(),
                )
            }),
            live.economy.owner_max_mana_capacity(player),
            live.economy
                .castle_for_owner(player)
                .map(|castle| (castle.source_entity_index, castle.mana)),
        )
    };
    let mut cast_admission = None;
    // Reuse the delivered192-turn Spider exposure bound. Inputs consist of
    // one fixed FireballI provocation, neutral visits and at most one
    // disclosed contact-boundary placement after its real own web hit.
    for turn in 0..192 {
        area_damage::start_effect_damage_trace_for_targets(&[player, root.source_slot]);
        let mana_before = sim(&game).shared_spells().player_entity().mana;
        let first_cast_before = (turn == 0).then(|| cast_admission_state(&game));
        let first_cast_journal = sim(&game).effect_journal().cursor(0).next_sequence;
        step(
            &mut game,
            GameCommand {
                cast_left: turn == 0,
                ..Default::default()
            },
        );
        if let Some(before) = first_cast_before {
            let events = sim(&game)
                .effect_journal()
                .records()
                .iter()
                .filter(|record| {
                    record.sequence >= first_cast_journal
                        && matches!(
                            record.effect,
                            CampaignEffect::Flight(
                                crate::FlightEvent::RequestSpellActivation { .. }
                                    | crate::FlightEvent::ShowHelpHint { .. }
                                    | crate::FlightEvent::PrepareSound { .. }
                            )
                        )
                })
                .map(|record| (record.sequence, record.effect.clone()))
                .collect::<Vec<_>>();
            cast_admission = Some((before, cast_admission_state(&game), events));
        }
        elapsed += 1;
        provocation_paid = provocation_paid.saturating_add(
            mana_before
                .saturating_sub(sim(&game).shared_spells().player_entity().mana)
                .max(0),
        );
        effects.extend(area_damage::take_effect_damage_trace());
        let visits = area_damage::take_player_damage_consumptions();
        if first_incoming_turn.is_none() && visits.iter().any(|visit| visit.target == player_handle)
        {
            first_incoming_turn = Some(turn);
        }
        for row in sim(&game)
            .economy()
            .fireball_projectiles()
            .iter()
            .filter(|row| {
                row.active
                    && row.origin == crate::native_world::FireballProjectileOrigin::PlayerSpell
                    && row.owner_entity_index == player
                    && row.holder_entity_index == provocation_holder
                    && row.source_subspell_index == 250
                    && row.payload == 250
            })
        {
            if let Some(handle) = sim(&game).source_arena().handle(row.source_entity_index)
                && !projectiles.iter().any(|(old, ..)| *old == handle)
            {
                projectiles.push((
                    handle,
                    row.owner_entity_index,
                    row.holder_entity_index,
                    row.source_subspell_index,
                    row.payload,
                ));
            }
        }
        payment_visits.extend(
            sim(&game)
                .cf10_production_schedule_receipts()
                .into_iter()
                .filter_map(|receipt| match receipt {
                    crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted {
                        slot,
                        original,
                        current: crate::campaign::schedule::CurrentOccupant::Known { selector, .. },
                        ..
                    } if slot == provocation_holder || slot == player => Some((
                        slot,
                        original.observed_player_resources()?,
                        selector.observed_player_resources()?,
                    )),
                    _ => None,
                }),
        );
        for visit in visits.iter().filter(|visit| {
            visit.target == root
                && visit.mailbox == (250, player)
                && (visit.life_before, visit.life_after) == (5500, 5250)
        }) {
            let actor = &sim(&game).entities()[usize::from(root.source_slot)];
            if actor.life == visit.life_after
                && actor.action == 162
                && actor.source_word_96() == player
            {
                provocation_visits.push((turn, *visit));
            }
        }
        let (launches, impacts) = web::trace_snapshot();
        own_web |= visits.iter().any(|visit| {
            visit.target == player_handle
                && visit.mailbox == (780, original.source_id)
                && visit.life_after == visit.life_before.wrapping_sub(780)
                && visit.writer.is_some_and(|writer| {
                    impacts.iter().any(|impact| {
                        writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
                            && launches.iter().any(|launch| {
                                launch.spider == root
                                    && launch.projectile == impact.projectile
                                    && launch.target == player
                            })
                    })
                })
        });
        for visit in &visits {
            if visit.target != player_handle || visit.life_before < 0 || visit.life_after >= 0 {
                continue;
            }
            let writer = visit
                .writer
                .expect("ordinary lethal hit retains its actual latest writer");
            assert_eq!(
                (writer.target, writer.mailbox),
                (player_handle, visit.mailbox)
            );
            let admitted = match writer.source {
                area_damage::PlayerMailboxSource::Contact(actor) => {
                    roots.iter().any(|&(_, handle)| {
                        actor == handle
                            && sim(&game).source_arena().handle(handle.source_slot) == Some(handle)
                    })
                }
                area_damage::PlayerMailboxSource::Effect(effect) => impacts.iter().any(|impact| {
                    // Fields66 submits its contact through sub11900, not the
                    // area-pulse API. The consumed latest writer supplies the
                    // actual source/target generations, sequence and mailbox.
                    impact.impact == effect
                        && (impact.packet.class, impact.packet.model) == (10, 66)
                        && impact.packet.source_id == visit.mailbox.1
                        && launches.iter().any(|launch| {
                            launch.projectile == impact.projectile
                                && launch.target == player
                                && roots.iter().any(|&(_, handle)| {
                                    launch.spider == handle
                                        && sim(&game).source_arena().handle(handle.source_slot)
                                            == Some(handle)
                                })
                        })
                }),
                area_damage::PlayerMailboxSource::Other => false,
            };
            assert!(
                admitted,
                "fatal damage must be this whole wave's actual Spider contact/web: {visit:?}"
            );
            assert_eq!(
                visit.life_after,
                visit.life_before.wrapping_sub(visit.mailbox.0)
            );
            fatal = Some(*visit);
        }
        consumptions.extend(visits);
        let state = sim(&game);
        assert_eq!(state.source_arena().handle(player), Some(player_handle));
        assert_eq!(state.entities()[usize::from(player)].max_life, 10_000);
        assert_eq!(
            state.source_arena().handle(owned_sphere.source_slot),
            Some(owned_sphere)
        );
        if turn < 8 || turn % 16 == 0 || fatal.is_some() {
            history.push((
                turn,
                state.session.state.live.player_life,
                state.state.position,
                state.entities()[usize::from(root.source_slot)].clone(),
            ));
        }
        if fatal.is_some() {
            break;
        }
        let row = &state.entities()[usize::from(root.source_slot)];
        if contact_placement.is_none()
            && own_web
            && row.action == 162
            && row.record[74] == 2
            && row.phase % SPIDER_CLASS_PROPERTIES.scan_period == 0
        {
            let mut position = row.position;
            crate::move_entity(&mut position, row.yaw, 0, 512);
            let ground = state.canonical_terrain().ground_height(position);
            position.z = i16::try_from(i32::from(ground) + 256)
                .expect("the actual chosen Cave floor+256 must fit source Z");
            let ceiling = state.canonical_terrain().ceiling_height(position);
            assert!(
                position.z >= ground.wrapping_add(256),
                "one fixed contact placement: Spider={:?} player={position:?} ground={ground} ceiling={ceiling:?}",
                row.position
            );
            assert!(
                ceiling.is_none_or(|ceiling| position.z <= ceiling.wrapping_sub(256)),
                "one fixed contact placement must fit the actual Cave: Spider={:?} player={position:?} ground={ground} ceiling={ceiling:?}",
                row.position
            );
            let profile = state.session.definition.profile;
            assert!(
                i32::from(position.z) >= i32::from(ground) + i32::from(profile.ground_clearance)
                    && ceiling.is_none_or(|ceiling| i32::from(position.z)
                        <= i32::from(ceiling) - i32::from(profile.ceiling_clearance)),
                "the source flight envelope must admit this fixed point: Spider={:?} player={position:?} ground={ground} ceiling={ceiling:?} profile={profile:?}",
                row.position
            );
            let contact_distance = crate::legacy_sqrt(
                crate::native_world::level2_wrapping_source_distance_3d(position, row.position),
            );
            assert!(
                contact_distance < SPIDER_CLASS_PROPERTIES.melee_range,
                "the legal fixed point must remain within source strict contact range: Spider={:?} player={position:?} ground={ground} ceiling={ceiling:?} distance={contact_distance}",
                row.position
            );
            contact_placement = Some(position);
            sim_mut(&mut game)
                .set_level2_entity_position(player, position)
                .unwrap();
        }
    }
    let launches = area_damage::take_fireball_launch_trace();
    let impact_births = area_damage::take_fireball_impact_birth_trace();
    let provoked = provocation_visits.iter().any(|(turn, visit)| {
        visit.writer.is_some_and(|writer| {
            first_incoming_turn.is_some_and(|incoming| *turn < incoming)
                && writer.target == root
                && writer.mailbox == visit.mailbox
                && impact_births.iter().any(
                    |&(impact, generation, shot, shot_generation, owner)| {
                        owner == player
                            && writer.source
                                == area_damage::PlayerMailboxSource::Effect(
                                    crate::campaign::source_arena::SourceEntityHandle {
                                        source_slot: impact,
                                        generation,
                                    },
                                )
                            && launches.contains(&(shot, shot_generation, player))
                            && projectiles
                                .iter()
                                .any(|(handle, owner, holder, payload, damage)| {
                                    handle.source_slot == shot
                                        && handle.generation == shot_generation
                                        && *owner == player
                                        && *holder == provocation_holder
                                        && *payload == 250
                                        && *damage == 250
                                })
                            && effects.iter().any(|effect| {
                                effect.source_scoped
                                    && effect.source_slot == Some(impact)
                                    && effect.target_slot == root.source_slot
                                    && effect.target_generation == root.generation
                                    && effect.writer_sequence == Some(writer.sequence)
                                    && effect.pulse.owner == player
                                    && effect.pulse.payload == 250
                            })
                    },
                )
        })
    });
    assert!(
        provoked
            && launches
                .iter()
                .filter(|(_, _, owner)| *owner == player)
                .count()
                == 1
            && projectiles.len() == 1
            && provocation_paid >= provocation_price
            && sim(&game).source_arena().handle(provocation_holder)
                == Some(provocation_holder_handle),
        "one actual paid FireballI must establish the source provocation: cast_admission={cast_admission:?} price={provocation_price} paid={provocation_paid} launches={launches:?} projectiles={projectiles:?} impacts={impact_births:?} visits={provocation_visits:?} effects={effects:?}"
    );
    let (_, before_charge, after_charge) = payment_visits
        .iter()
        .find(|(slot, before, after)| {
            *slot == provocation_holder && after.2 < before.2 && after.2 <= -provocation_price
        })
        .unwrap_or_else(|| {
            panic!("the actual holder must queue the source charge: {payment_visits:?}")
        });
    assert_eq!(before_charge.0, player_handle);
    assert_eq!(
        *after_charge,
        (
            player_handle,
            before_charge.1,
            if before_charge.2 >= 0 {
                -provocation_price
            } else {
                before_charge.2.wrapping_sub(provocation_price)
            },
            before_charge.3
        )
    );
    let (_, before_debit, after_debit) = payment_visits
        .iter()
        .find(|(slot, before, after)| {
            *slot == player
                && before.2 <= -provocation_price
                && before.1 - after.1 >= provocation_price
        })
        .unwrap_or_else(|| {
            panic!("actual later player upkeep must consume the source charge: {payment_visits:?}")
        });
    assert_eq!(
        (before_debit.0, after_debit.0),
        (player_handle, player_handle)
    );
    assert_eq!(
        after_debit.1,
        before_debit
            .1
            .wrapping_add(before_debit.2)
            .clamp(0, before_debit.3)
    );
    let fatal = fatal.unwrap_or_else(|| panic!("default-health ordinary lethal encounter: own_web={own_web} contact_placement={contact_placement:?} history={history:?} damage={consumptions:?} effects={effects:?}"));
    let _ = web::take_trace();
    assert!(own_web && contact_placement.is_some());
    assert_eq!(
        sim(&game).entities()[usize::from(player)].word_24(),
        fatal.mailbox.1
    );
    assert_eq!(sim(&game).entities()[usize::from(player)].action, 2);
    let fall_height = i32::from(sim(&game).state.position.z)
        - i32::from(
            sim(&game)
                .canonical_terrain()
                .ground_height(sim(&game).state.position),
        );
    //128 visits reach the source fall cap256, then enough visits consume
    // the actual height. No falling velocity or countdown is rewritten.
    let fall_bound = 128 + usize::try_from(fall_height.max(0)).unwrap().div_ceil(256) + 4;
    for _ in 0..fall_bound {
        if sim(&game).entities()[usize::from(player)].action == 3 {
            break;
        }
        step(&mut game, GameCommand::default());
        elapsed += 1;
    }
    let state = sim(&game);
    let actor = &state.entities()[usize::from(player)];
    assert_eq!(
        actor.action, 3,
        "the real fall must reach the grounded death producer"
    );
    assert_eq!(actor.source_dword_10(), 1200);
    assert_ne!(actor.control_flags & 0x20, 0);
    let marker = state
        .session
        .state
        .live
        .economy
        .player_death_markers
        .iter()
        .find(|row| row.active && row.trigger_player_entity_index == player)
        .copied()
        .unwrap();
    let marker_handle = state
        .source_arena()
        .handle(marker.source_entity_index)
        .unwrap();
    let views = state.campaign_entity_presentations(0).unwrap();
    assert!(
        views
            .iter()
            .any(|view| view.entity_index == marker.source_entity_index
                && view.render_parameter_index
                    == crate::level1_mission::PLAYER_DEATH_MARKER_PARTICLE_PARAMETER)
    );
    assert!(views.iter().all(|view| view.entity_index != player));
    let dying_burst = state
        .source_arena()
        .effects
        .death_burst_origins()
        .iter()
        .find_map(|(&slot, &(generation, parent))| {
            (parent == player
                && state
                    .source_arena()
                    .handle(slot)
                    .is_some_and(|handle| handle.generation == generation)
                && views.iter().any(|view| view.entity_index == slot))
            .then_some(slot)
        })
        .expect("an actual source10/1 dying burst reaches the original entity presentation");
    let burst_row = &state.entities()[usize::from(dying_burst)];
    assert_eq!(
        (burst_row.class, burst_row.model, burst_row.source_id),
        (10, 1, actor.source_id)
    );
    assert_ne!(burst_row.control_flags & 0x80, 0);
    let sphere = state
        .economy()
        .spheres()
        .iter()
        .find(|row| row.source_entity_index == owned_sphere.source_slot)
        .unwrap();
    assert_eq!(
        state.source_arena().handle(owned_sphere.source_slot),
        Some(owned_sphere)
    );
    assert_eq!(
        (sphere.mana, sphere.player_entity_index),
        (1, marker.source_entity_index)
    );
    for (family, handle) in held.into_iter().enumerate() {
        assert_eq!(
            state.shared_spells().local_player().enabled_spell_holders[family],
            1
        );
        assert_eq!(
            state.source_arena().handle(handle.source_slot),
            Some(handle)
        );
        let row = state
            .shared_spells()
            .pickups()
            .iter()
            .chain(state.shared_spells().holders())
            .chain(state.shared_spells().dynamic_pickups())
            .find(|row| row.index == handle.source_slot)
            .unwrap();
        assert_eq!(
            (row.class, row.model, row.action),
            (15, family as u8, family as u8 * 3 + 1)
        );
        assert!(row.life > 0 && row.life <= 289);
        assert_eq!(row.control_flags & 1, 0);
        assert!(
            row.position
                .x
                .wrapping_sub(actor.position.x)
                .cast_signed()
                .abs()
                <= 256
        );
        assert!(
            row.position
                .y
                .wrapping_sub(actor.position.y)
                .cast_signed()
                .abs()
                <= 256
        );
        assert!(
            state
                .economy()
                .map_chains()
                .entity_chain((row.position.x >> 8) as u8, (row.position.y >> 8) as u8)
                .contains(&handle.source_slot)
        );
    }
    assert!(
        !game.snapshot().value.return_to_title_requested,
        "source death camera is continuing, not the actual PlayerEvents0x1d flow result"
    );
    let before_camera = sim(&game).state.clone();
    let before_actor = sim(&game).entities()[usize::from(player)].clone();
    let killer_slot = before_actor.word_24();
    assert!(
        killer_slot > player
            && roots
                .iter()
                .any(|&(_, handle)| handle.source_slot == killer_slot),
        "these actual raw Spiders receive their own pose callbacks after the human in ascending source order"
    );
    let current_killer = {
        let state = sim(&game);
        let live = &state.session.state.live;
        crate::native_world::source_target::read(
            &live.economy,
            crate::native_world::source_target::Context {
                spells: Some(&live.shared_spells),
                particles: Some(&state.session.definition.presentation.particles),
                ..crate::native_world::source_target::Context::from_ordinary(
                    &live.goats,
                    &live.ordinary,
                )
            },
            killer_slot,
        )
        .unwrap()
        .unwrap()
        .position
    };
    let camera_ground = sim(&game)
        .canonical_terrain()
        .ground_height(before_actor.position);
    let desired_heading =
        crate::heading_for_positions(before_actor.position, current_killer) as i16;
    let expected_yaw =
        before_actor
            .yaw
            .wrapping_add(crate::turn_toward(before_actor.yaw, desired_heading, 22))
            & 0x7ff;
    let dx = i32::from(
        current_killer
            .x
            .wrapping_sub(before_actor.position.x)
            .cast_signed(),
    );
    let dy = i32::from(
        current_killer
            .y
            .wrapping_sub(before_actor.position.y)
            .cast_signed(),
    );
    let horizontal = crate::legacy_sqrt(
        dx.wrapping_mul(dx)
            .cast_unsigned()
            .wrapping_add(dy.wrapping_mul(dy).cast_unsigned()),
    ) as i32;
    let expected_fov = crate::pitch_for_delta(
        horizontal,
        i32::from(current_killer.z.wrapping_sub(before_actor.position.z)),
    );
    crate::native_world::creatures::player_lifecycle::start_camera_trace(player_handle);
    step(&mut game, GameCommand::default());
    elapsed += 1;
    let camera_visits = crate::native_world::creatures::player_lifecycle::take_camera_trace();
    assert_eq!(camera_visits.len(), 1);
    let camera = camera_visits[0];
    assert_eq!(
        (
            camera.player,
            camera.killer_slot,
            camera.player_position,
            camera.player_yaw,
            camera.target_position,
            camera.altitude
        ),
        (
            player_handle,
            killer_slot,
            before_actor.position,
            before_actor.yaw,
            Some(current_killer),
            camera_ground
        ),
        "the reached source camera reads the independently observed current later-slot killer"
    );
    let state = sim(&game);
    assert_eq!(state.source_arena().handle(player), Some(player_handle));
    assert_eq!(
        state.source_arena().handle(marker.source_entity_index),
        Some(marker_handle)
    );
    assert_eq!(state.entities()[usize::from(player)].action, 3);
    assert_eq!(
        state.entities()[usize::from(player)].source_dword_10(),
        1200,
        "human camera does not consume the rival-only1200 revival timer"
    );
    assert!(state.state.tick > before_camera.tick);
    assert_eq!(
        (state.state.yaw, state.entities()[usize::from(player)].yaw),
        (expected_yaw, expected_yaw)
    );
    assert_eq!(
        (
            state.entities()[usize::from(player)].source_roll(),
            state.entities()[usize::from(player)].fov
        ),
        (desired_heading, expected_fov)
    );
    assert_eq!(
        state.state.position,
        Position {
            z: camera_ground,
            ..before_actor.position
        }
    );
    assert_eq!(game.snapshot().value.player_pose.yaw, expected_yaw);
    assert_eq!(
        (
            state.state.display_pitch,
            state.state.pitch_accumulator,
            state.state.roll_accumulator
        ),
        (0, 0, 0)
    );
    assert!(!game.snapshot().value.return_to_title_requested);
    assert_eq!(
        state
            .effect_journal()
            .records()
            .iter()
            .filter(|record| record.sequence >= journal_start
                && matches!(record.effect, CampaignEffect::SceneChange { scene: 4 }))
            .count(),
        1
    );
    let notice = game
        .snapshot()
        .value
        .participant_notifications
        .into_iter()
        .find(|notice| notice.is_local)
        .expect("the actual death reaches the generic participant observation");
    assert_eq!(
        (notice.kind.as_str(), notice.remaining_frames),
        ("participant.died", 100)
    );
    let source_color = usize::from(sim(&game).state.player_color_index);
    assert_eq!(
        sim(&game).player_lifecycle_notifications().entries()[source_color].source_owner,
        player
    );
    let before_frame = sim(&game).campaign_state_hash();
    let gameplay_before_frames = (
        sim(&game).state.clone(),
        sim(&game)
            .source_arena()
            .reset_hash_with_terrain(sim(&game).canonical_terrain()),
        sim(&game).session.state.live.nyphur_dispatcher.clone(),
    );
    let again = game.snapshot().value.participant_notifications;
    assert!(again.iter().any(|current| *current == notice));
    assert_eq!(
        sim(&game).campaign_state_hash(),
        before_frame,
        "snapshot reads never age notices"
    );
    game.presentation_frame(game.identity().guard(), false)
        .unwrap();
    assert_eq!(
        sim(&game).campaign_state_hash(),
        before_frame,
        "suppressed settings/dialog frame does not age"
    );
    for remaining in (0..100).rev() {
        game.presentation_frame(game.identity().guard(), true)
            .unwrap();
        assert!(
            game.snapshot()
                .value
                .participant_notifications
                .iter()
                .any(|current| current.participant == notice.participant
                    && current.remaining_frames == remaining)
        );
    }
    game.presentation_frame(game.identity().guard(), true)
        .unwrap();
    assert!(
        game.snapshot()
            .value
            .participant_notifications
            .iter()
            .all(|current| current.participant != notice.participant)
    );
    assert_eq!(
        sim(&game).entities()[usize::from(player)].source_dword_10(),
        1200,
        "presentation frames never advance the source rival countdown"
    );
    assert_eq!(
        (
            sim(&game).state.clone(),
            sim(&game)
                .source_arena()
                .reset_hash_with_terrain(sim(&game).canonical_terrain()),
            sim(&game).session.state.live.nyphur_dispatcher.clone()
        ),
        gameplay_before_frames,
        "notification UI frames leave gameplay, entity/Watcom RNG carriers and tick unchanged"
    );
    for _ in 0..elapsed {
        safe.step(PlayerCommand::default(), 0);
        assert!(
            safe.simulation()
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "{:?}",
            safe.simulation().dispatch_events()
        );
    }
    let negative = safe.simulation();
    assert!(negative.session.state.live.player_life >= 0);
    assert_eq!(negative.entities()[usize::from(player)].action, 0);
    assert!(
        negative
            .session
            .state
            .live
            .economy
            .player_death_markers
            .iter()
            .all(|row| row.trigger_player_entity_index != player)
    );
    assert!(
        negative
            .economy()
            .spheres()
            .iter()
            .any(|row| row.source_entity_index == owned_sphere.source_slot
                && row.player_entity_index == player)
    );
    assert_eq!(
        negative
            .shared_spells()
            .local_player()
            .enabled_spell_holders,
        held.map(|handle| handle.source_slot)
    );

    // Continue this real no-Castle death through the public recovery input.
    // All death, camera, notice and equal-elapsed living-control assertions
    // above precede this command and retain their original scope.
    let (spawn, headers_before, before_recovery_actor, before_recovery_player) = {
        let state = sim(&game);
        let live = &state.session.state.live;
        let source_row = live.source_player_result.source_player_slot();
        let authored_spawn = live
            .mission
            .authored_features()
            .iter()
            .filter(|feature| {
                feature.index > 0
                    && feature.index < 1200
                    && feature.activation_group == 0
                    && feature.kind == 3
                    && feature.subtype == 4 + source_row as i16
            })
            .max_by_key(|feature| feature.index)
            .expect("the actual current human row has a group-zero spawn constructor")
            .world_position(0);
        let recorded = live.player_spawn_positions.get(source_row).unwrap();
        assert_eq!(
            (recorded.x, recorded.y),
            (authored_spawn.x, authored_spawn.y),
            "the live player-row spawn retains the actual authored constructor XY"
        );
        let player_state = live.shared_spells.local_player();
        assert_eq!(
            u16::from_le_bytes([player_state.record[42], player_state.record[43]]),
            0,
            "the ordinary Castle removal left no current A4 Castle for recovery"
        );
        assert_eq!(live.economy.player_castle_entity_index(), 0);
        assert_eq!(player_state.enabled_spell_holders, [1; 26]);
        (
            Position {
                z: live.terrain.ground_height(recorded).wrapping_add(256),
                ..recorded
            },
            *live.source_player_result.raw_headers(),
            live.economy.entities[usize::from(player)].clone(),
            player_state.clone(),
        )
    };
    let identity_before_recovery = game.identity().guard();
    step(
        &mut game,
        GameCommand {
            recover_player: true,
            ..Default::default()
        },
    );
    let identity_after_recovery = game.identity().guard();
    assert_eq!(
        (
            identity_after_recovery.session_id,
            identity_after_recovery.generation
        ),
        (
            identity_before_recovery.session_id,
            identity_before_recovery.generation
        )
    );
    assert!(identity_after_recovery.expected_tick > identity_before_recovery.expected_tick);
    let state = sim(&game);
    let live = &state.session.state.live;
    let actor = &live.economy.entities[usize::from(player)];
    let restored = live.shared_spells.local_player();
    let rules = crate::level_definition::PLAYER_LIFECYCLE_PROPERTIES;
    assert_eq!(state.source_arena().handle(player), Some(player_handle));
    assert_eq!(
        (
            actor.class,
            actor.model,
            actor.action,
            actor.max_life,
            actor.life
        ),
        (3, 0, 0, rules.revival_max_life, rules.revival_max_life)
    );
    assert!(actor.active && actor.control_flags & 0x20 == 0);
    assert_eq!(live.player_life, actor.life);
    assert_eq!((actor.position.x, actor.position.y), (spawn.x, spawn.y));
    assert_eq!(state.state.position, actor.position);
    assert!(actor.position.z >= live.terrain.ground_height(actor.position));
    // 5C950's ground+256 copy precedes the ordinary same-pass flight body;
    // its final Z is deliberately not compared with that pre-body value.
    assert_eq!(
        actor.mana,
        rules
            .revival_max_mana
            .wrapping_add(before_recovery_actor.mana_regen)
            .clamp(0, actor.max_mana)
    );
    assert_eq!(live.shared_spells.player_mana(), actor.mana);
    let mut expected_headers = headers_before;
    expected_headers[usize::from(live.source_player_result.source_player_slot())] |= 0x000c_0000;
    assert_eq!(live.source_player_result.raw_headers(), &expected_headers);
    assert_eq!(
        restored.persistent_spell_experience,
        before_recovery_player.persistent_spell_experience
    );
    let fire = live.core.context.spell_fire.as_ref().unwrap();
    let mut restored_handles = std::collections::BTreeSet::new();
    for (family, old_handle) in held.iter().enumerate() {
        let slot = restored.enabled_spell_holders[family];
        assert!(slot > 1, "family {family} must bind an actual new holder");
        let handle = state.source_arena().handle(slot).unwrap();
        assert!(restored_handles.insert((handle.source_slot, handle.generation)));
        assert_ne!(handle, *old_handle);
        assert_eq!(
            state.source_arena().handle(old_handle.source_slot),
            Some(*old_handle)
        );
        let holder = &live.economy.entities[usize::from(slot)];
        assert!(holder.active);
        assert_eq!(
            (
                holder.class,
                holder.model,
                holder.action,
                holder.source_spell_parent_entity_index()
            ),
            (15, family as u8, family as u8 * 3, player)
        );
        assert_eq!(fire.enabled_holder_indices[family], slot);
        assert!(fire.holders.iter().any(|row| row.entity_index == slot));
    }
    assert_eq!(restored_handles.len(), 26);
    assert_eq!(
        live.core
            .players
            .iter()
            .find(|row| row.entity_index == player)
            .unwrap()
            .enabled_spell_holders,
        restored.enabled_spell_holders
    );
    assert!(!game.snapshot().value.return_to_title_requested);
}

#[test]
fn whole_mission23_paid_meteor_defeat_runs_the_real_rival_castle_countdown_and_revival() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::game_session::{GameCommand, GameSession, SpellAssignment, SpellAssignmentHand};
    use crate::native_world::{CarriedSpellProgress, area_damage, source_target};
    type Game = GameSession<crate::campaign::backend::Mc2CampaignBackend>;
    fn sim(game: &Game) -> &Mc2Simulation {
        game.backend().compatibility_campaign().simulation()
    }
    fn sim_mut(game: &mut Game) -> &mut Mc2Simulation {
        game.backend_mut()
            .compatibility_campaign_mut_for_test()
            .simulation_mut()
    }
    fn step(game: &mut Game, command: GameCommand) {
        game.advance(game.identity().guard(), command).unwrap();
        assert!(
            sim(game)
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "{:?}",
            sim(game).dispatch_events()
        );
    }
    let package = std::env::var_os("MC_LEVEL23_PACKAGE").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
                "../../../generated/creatures-20261104-integration/current-owner-package/level23",
            )
        },
        PathBuf::from,
    );
    let definition = load_mc2_campaign(
        &package,
        &crate::game_definition::default_catalog().unwrap(),
    )
    .unwrap();
    let mut game = GameSession::open(
        &Mc2CampaignAdapter,
        std::sync::Arc::new(definition),
        2318,
        Mc2CampaignInputs::new(&package),
    )
    .unwrap();
    let player = sim(&game).state.entity_index;
    let player_handle = sim(&game).source_arena().handle(player).unwrap();
    let owner = 425;
    let owner_handle = sim(&game).source_arena().handle(owner).unwrap();
    let original = sim(&game).entities()[usize::from(owner)].clone();
    assert_eq!(
        (
            original.class,
            original.model,
            original.life,
            original.max_life
        ),
        (3, 1, 9921, 9921)
    );
    let castle = sim(&game)
        .economy()
        .castle_for_owner(owner)
        .unwrap()
        .source_entity_index;
    assert_eq!(castle, 450);
    let castle_handle = sim(&game).source_arena().handle(castle).unwrap();
    let peers = sim(&game)
        .players()
        .iter()
        .filter(|row| row.entity_index != 0 && row.entity_index != owner)
        .map(|row| (row.slot, row.entity_index))
        .collect::<Vec<_>>();
    let victim_color = usize::from(
        sim(&game)
            .players()
            .iter()
            .find(|row| row.entity_index == owner)
            .unwrap()
            .player_color_index(),
    );
    let score_before = sim(&game)
        .players()
        .iter()
        .find(|row| row.entity_index == player)
        .unwrap()
        .source_kill_scores()
        .unwrap()[victim_color];

    // Declared player-only prior MeteorIII carry/resources and real Castle.
    // The rival's source HP, Castle, movement, protection, RNG and1200 timer
    // are never rewritten. No unrelated row is removed from the mission.
    {
        let state = sim_mut(&mut game);
        state.apply_carried_spell_families(&[9], Some(9)).unwrap();
        state
            .apply_carried_spell_progression(&[CarriedSpellProgress {
                family: 9,
                persistent_experience: crate::native_world::LEVEL2_SPELL_TIER_THRESHOLDS[9][2],
                runtime_experience: 0,
                unlocked_tier: 2,
                selected_tier: 2,
            }])
            .unwrap();
        for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
            state
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
                !state.economy().castle_projectile_site_blocked(
                    *position,
                    state.entities(),
                    state.canonical_terrain(),
                )
            })
            .unwrap();
        let owned_castle = state.economy_mut().build_castle(site, player).unwrap();
        assert!(
            state
                .economy_mut()
                .set_castle_mana_prerequisite_for_test(owned_castle, 200_000)
        );
        let live = &mut state.session.state.live;
        let row = &mut live.economy.entities[usize::from(player)];
        row.max_mana = 200_000;
        row.record[144..148].copy_from_slice(&200_000_i32.to_le_bytes());
        row.set_mana(200_000);
        row.set_mana_regen(0);
        let row = row.clone();
        live.shared_spells.sync_player_entity_from_authority(&row);
        live.shared_spells.player_mana = row.mana;
        live.shared_spells.caster.mana = row.mana;
        live.shared_spells
            .sync_typed_spell_parent_caches_from_player_entity();
        if let Some(fire) = live.core.context.spell_fire.as_mut() {
            fire.caster.mana = row.mana;
        }
    }
    step(
        &mut game,
        GameCommand {
            assign_spell: Some(SpellAssignment {
                spell_id: crate::game_definition::SemanticId::parse("spell.meteor").unwrap(),
                tier: 2,
                hand: SpellAssignmentHand::Left,
                holder_actor: None,
            }),
            ..Default::default()
        },
    );
    let mut lure = original.position;
    lure.y = lure.y.wrapping_add(8192);
    lure.z = sim(&game)
        .canonical_terrain()
        .ground_height(lure)
        .wrapping_add(1024);
    sim_mut(&mut game)
        .set_level2_entity_position(player, lure)
        .unwrap();
    sim_mut(&mut game).state.actual_speed = 0;
    sim_mut(&mut game).state.target_speed = 0;
    let mut admitted = false;
    let mut approach = Vec::new();
    for turn in 0..256 {
        let state = sim(&game);
        assert_eq!(state.source_arena().handle(owner), Some(owner_handle));
        assert!(state.entities()[usize::from(owner)].life > 0);
        let common = state
            .session
            .state
            .live
            .nyphur_dispatcher
            .get(owner_handle)
            .unwrap()
            .common_upkeep(state.entities()[usize::from(owner)].phase);
        let actor = &state.entities()[usize::from(owner)];
        let castle_row = state.economy().castle_for_owner(owner).unwrap();
        let separation_x = i32::from(
            actor
                .position
                .x
                .wrapping_sub(castle_row.position.x)
                .cast_signed(),
        )
        .abs();
        let separation_y = i32::from(
            actor
                .position
                .y
                .wrapping_sub(castle_row.position.y)
                .cast_signed(),
        )
        .abs();
        let separation = separation_x.max(separation_y);
        let outside_castle = separation_x > 1920 + i32::from(castle_row.shift_pitch)
            || separation_y > 1920 + i32::from(castle_row.shift_roll);
        // Keep the stationary source17 expansion off the genuine Castle;
        // the1200 branch must retain that same current A4 address.
        if common.word_0x159 == 0 && !common.high_upkeep && separation > 4096 && outside_castle {
            admitted = true;
            break;
        }
        if turn < 8 || turn % 32 == 0 {
            approach.push((
                turn,
                actor.position,
                actor.life,
                common.word_0x159,
                common.high_upkeep,
                separation,
            ));
        }
        step(&mut game, GameCommand::default());
    }
    assert!(
        admitted,
        "actual rival must naturally leave Castle protection before the fixed shot: {approach:?}"
    );
    let target = {
        let state = sim(&game);
        let live = &state.session.state.live;
        source_target::read(
            &live.economy,
            source_target::Context {
                spells: Some(&live.shared_spells),
                particles: Some(&state.session.definition.presentation.particles),
                ..source_target::Context::from_ordinary(&live.goats, &live.ordinary)
            },
            owner,
        )
        .unwrap()
        .unwrap()
    };
    let top = Position {
        z: target
            .position
            .z
            .wrapping_add(target.collision.unwrap().vertical_offset),
        ..target.position
    };
    let hand = Position {
        z: top.z.wrapping_add(1024),
        ..top
    };
    let aim = crate::level1_mission::projectile_guidance_angles(hand, top);
    let mut position = Position {
        z: hand.z.wrapping_sub(sim(&game).state.entity_fov),
        ..hand
    };
    let auxiliary = sim(&game)
        .shared_spells()
        .beyond_sight_auxiliary()
        .next_entity;
    crate::move_entity(
        &mut position,
        aim.0.wrapping_sub(auxiliary).wrapping_add(512),
        0,
        256,
    );
    sim_mut(&mut game)
        .set_level2_entity_position(player, position)
        .unwrap();
    sim_mut(&mut game).state.actual_speed = 0;
    sim_mut(&mut game).state.target_speed = 0;
    sim_mut(&mut game).set_level2_player_aim_for_test(aim.0, aim.1);
    let holder = sim(&game)
        .shared_spells()
        .local_player()
        .enabled_spell_holders[9];
    let holder_handle = sim(&game).source_arena().handle(holder).unwrap();
    let price = sim(&game).shared_spells().pickup(holder).unwrap().max_mana;
    assert_eq!(
        price,
        crate::level_definition::METEOR_PROPERTIES.tiers[2].mana_cost
    );
    let held = sim(&game)
        .players()
        .iter()
        .find(|row| row.entity_index == owner)
        .unwrap()
        .enabled_spell_holders
        .into_iter()
        .enumerate()
        .filter(|(_, slot)| *slot > 1)
        .map(|(family, slot)| (family, sim(&game).source_arena().handle(slot).unwrap()))
        .collect::<Vec<_>>();
    let mut no_cast = game.backend().compatibility_campaign().clone();
    area_damage::start_actor_damage_trace(&[owner_handle]);
    let mut launches = Vec::new();
    let mut impacts = Vec::new();
    let mut effects = Vec::new();
    let mut consumed = Vec::new();
    let mut history = Vec::new();
    let mut fatal = None;
    let mut paid = 0_i32;
    let mut banished_at = None;
    let mut elapsed = 0;
    // Three predetermined source cooldown11+one activations. Their48000
    // maximum is12000 even under quarter-damage Shield, above this original
    //9921HP. Fixed geometry may legitimately miss; no command tracks it.
    for turn in 0..160 {
        sim(&game).start_creature17_trace();
        area_damage::start_effect_damage_trace(owner);
        let mana_before = sim(&game).shared_spells().player_entity().mana;
        step(
            &mut game,
            GameCommand {
                cast_left: matches!(turn, 0 | 12 | 24),
                ..Default::default()
            },
        );
        elapsed += 1;
        paid = paid.saturating_add(
            mana_before
                .saturating_sub(sim(&game).shared_spells().player_entity().mana)
                .max(0),
        );
        let trace = sim(&game).take_creature17_trace();
        launches.extend(trace.meteor_launches);
        impacts.extend(trace.meteor_impacts);
        effects.extend(area_damage::take_effect_damage_trace());
        consumed.extend(area_damage::take_player_damage_consumptions());
        fatal = consumed
            .iter()
            .find(|visit| {
                visit.target == owner_handle
                    && visit.life_before >= 0
                    && visit.life_after < 0
                    && visit.writer.is_some_and(|writer| {
                        writer.target == owner_handle
                            && writer.mailbox == visit.mailbox
                            && impacts.iter().any(|impact| {
                                writer.source
                                    == area_damage::PlayerMailboxSource::Effect(impact.impact)
                                    && effects.iter().any(|effect| {
                                        effect.source_scoped
                                            && effect.source_slot == Some(impact.impact.source_slot)
                                            && effect.target_generation == owner_handle.generation
                                            && effect.target_slot == owner
                                            && effect.writer_sequence == Some(writer.sequence)
                                    })
                                    && launches.iter().any(|launch| {
                                        launch.projectile == impact.projectile
                                            && launch.caster == player_handle
                                            && launch.holder == holder_handle
                                    })
                            })
                    })
            })
            .copied();
        let state = sim(&game);
        let row = &state.entities()[usize::from(owner)];
        assert_eq!(state.source_arena().handle(owner), Some(owner_handle));
        if row.action == 3 && banished_at.is_none() {
            assert_eq!(row.source_dword_10(), 1200);
            banished_at = Some(elapsed);
        }
        if turn < 8 || turn % 16 == 0 || fatal.is_some() {
            history.push((turn, row.life, row.action, row.position, row.word_24()));
        }
        if turn >= 25 && fatal.is_some() {
            break;
        }
    }
    assert!(
        fatal.is_some(),
        "real paid projectiles must defeat the original default-health rival: shots={launches:?} impacts={impacts:?} effects={effects:?} consumed={consumed:?} history={history:?}"
    );
    assert_eq!(
        launches
            .iter()
            .filter(|launch| launch.holder == holder_handle && launch.caster == player_handle)
            .count(),
        3
    );
    assert!(
        paid >= 3 * price,
        "all three actual launches must be paid: paid={paid} price={price}"
    );
    for _ in 0..elapsed {
        no_cast.step(PlayerCommand::default(), 0);
        assert!(
            no_cast
                .simulation()
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
    }
    assert!(no_cast.simulation().entities()[usize::from(owner)].life > 0);
    let fall_bound = 128
        + usize::try_from(
            i32::from(sim(&game).entities()[usize::from(owner)].position.z)
                .saturating_sub(i32::from(
                    sim(&game)
                        .canonical_terrain()
                        .ground_height(sim(&game).entities()[usize::from(owner)].position),
                ))
                .max(0),
        )
        .unwrap()
        .div_ceil(256)
        + 4;
    for _ in 0..fall_bound {
        if sim(&game).entities()[usize::from(owner)].action == 3 {
            break;
        }
        step(&mut game, GameCommand::default());
        elapsed += 1;
        if sim(&game).entities()[usize::from(owner)].action == 3 && banished_at.is_none() {
            assert_eq!(
                sim(&game).entities()[usize::from(owner)].source_dword_10(),
                1200
            );
            banished_at = Some(elapsed);
        }
    }
    assert_eq!(sim(&game).entities()[usize::from(owner)].action, 3);
    assert_eq!(sim(&game).entities()[usize::from(owner)].word_24(), player);
    let remaining_timer = 1200 - i32::try_from(elapsed - banished_at.unwrap()).unwrap();
    assert_eq!(
        sim(&game).entities()[usize::from(owner)].source_dword_10(),
        remaining_timer
    );
    assert_eq!(
        sim(&game)
            .players()
            .iter()
            .find(|row| row.entity_index == player)
            .unwrap()
            .source_kill_scores()
            .unwrap()[victim_color],
        score_before.wrapping_add(1)
    );
    let marker = sim(&game)
        .session
        .state
        .live
        .economy
        .player_death_markers
        .iter()
        .find(|row| row.active && row.trigger_player_entity_index == owner)
        .unwrap();
    assert!(
        sim(&game)
            .source_arena()
            .handle(marker.source_entity_index)
            .is_some()
    );
    for &(family, handle) in &held {
        assert_eq!(
            sim(&game).source_arena().handle(handle.source_slot),
            Some(handle)
        );
        assert_eq!(
            sim(&game)
                .players()
                .iter()
                .find(|row| row.entity_index == owner)
                .unwrap()
                .enabled_spell_holders[family],
            1
        );
    }
    for remaining in (0..remaining_timer).rev() {
        step(&mut game, GameCommand::default());
        let state = sim(&game);
        let row = &state.entities()[usize::from(owner)];
        assert_eq!(state.source_arena().handle(owner), Some(owner_handle));
        assert_eq!((row.action, row.source_dword_10()), (3, remaining));
        let player = state
            .players()
            .iter()
            .find(|row| row.entity_index == owner)
            .unwrap();
        assert_eq!(
            player.objective_player_state(),
            Some(1),
            "a dead rival with Castle is still participating"
        );
        assert_eq!(
            u16::from_le_bytes([player.record[42], player.record[43]]),
            castle
        );
        assert_eq!(state.source_arena().handle(castle), Some(castle_handle));
    }
    step(&mut game, GameCommand::default());
    let state = sim(&game);
    let actor = &state.entities()[usize::from(owner)];
    assert_eq!(state.source_arena().handle(owner), Some(owner_handle));
    assert_eq!(
        (actor.action, actor.life, actor.max_life),
        (1, 10_000, 10_000)
    );
    assert_eq!(
        actor.position,
        state.economy().castle_for_owner(owner).unwrap().position
    );
    let player_row = state
        .players()
        .iter()
        .find(|row| row.entity_index == owner)
        .unwrap();
    for (family, prior) in held {
        let current = state
            .session
            .state
            .live
            .nyphur_dispatcher
            .bound_live_holder(owner_handle, player_row, state.source_arena(), family)
            .expect("actual source revival binds its own current holder");
        assert_ne!(
            current, prior,
            "source may reuse a reclaimed physical slot only with a new generation"
        );
    }
    assert!(!crate::campaign::objectives::all_other_players_banished(
        0,
        state.runtime().player_count,
        state
            .players()
            .iter()
            .map(|row| row.objective_player_state())
    ));
    for (slot, entity) in peers {
        assert_eq!(state.players()[usize::from(slot)].entity_index, entity);
    }
    assert!(!game.snapshot().value.return_to_title_requested);
}

#[test]
fn whole_mission_two_paid_meteor_defeats_the_castleless_rival_and_completes_its_real_objective() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::game_session::{GameCommand, GameSession, SpellAssignment, SpellAssignmentHand};
    use crate::level_definition::ObjectiveCondition;
    use crate::native_world::{area_damage, source_target};
    type Game = GameSession<crate::campaign::backend::Mc2CampaignBackend>;
    fn sim(game: &Game) -> &Mc2Simulation {
        game.backend().compatibility_campaign().simulation()
    }
    fn sim_mut(game: &mut Game) -> &mut Mc2Simulation {
        game.backend_mut()
            .compatibility_campaign_mut_for_test()
            .simulation_mut()
    }
    fn step(game: &mut Game, command: GameCommand) {
        game.advance(game.identity().guard(), command).unwrap();
        assert!(
            sim(game)
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "{:?}",
            sim(game).dispatch_events()
        );
    }
    let package = std::env::var_os("MC_LEVEL2_PACKAGE_V5").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/tremor-20261104-integration/current-owner-package/level2")
        },
        PathBuf::from,
    );
    let definition = load_mc2_campaign(
        &package,
        &crate::game_definition::default_catalog().unwrap(),
    )
    .unwrap();
    let mut game = GameSession::open(
        &Mc2CampaignAdapter,
        std::sync::Arc::new(definition),
        2218,
        Mc2CampaignInputs::new(&package),
    )
    .unwrap();
    let player = sim(&game).state.entity_index;
    assert_eq!(player, 111);
    let owner = 112;
    let owner_handle = sim(&game).source_arena().handle(owner).unwrap();
    let player_handle = sim(&game).source_arena().handle(player).unwrap();
    assert_eq!(
        (
            sim(&game).entities()[112].class,
            sim(&game).entities()[112].model,
            sim(&game).entities()[112].life,
            sim(&game).entities()[112].max_life
        ),
        (3, 1, 3085, 3085)
    );
    assert!(sim(&game).economy().castle_for_owner(owner).is_none());
    let rival = sim(&game)
        .players()
        .iter()
        .find(|row| row.entity_index == owner)
        .unwrap();
    assert_eq!(u16::from_le_bytes([rival.record[42], rival.record[43]]), 0);
    assert_eq!(rival.enabled_spell_holders[2], 0);
    assert_eq!(rival.enabled_spell_holders[6], 0);
    assert_eq!(rival.enabled_spell_holders[8], 0);
    assert!(matches!(
        sim(&game)
            .session
            .definition
            .semantic_level()
            .unwrap()
            .objectives()[1]
            .condition(),
        ObjectiveCondition::DefeatRivalWizard(_)
    ));
    assert_eq!(sim(&game).runtime().objectives[0].stage_selectors[1], 1);
    let victim_color = usize::from(rival.player_color_index());
    let victim_name = game
        .snapshot()
        .value
        .source_scoreboard
        .players
        .iter()
        .find(|row| usize::from(row.source_slot) == victim_color)
        .unwrap()
        .display_name
        .clone();
    let score_before = sim(&game)
        .players()
        .iter()
        .find(|row| row.entity_index == player)
        .unwrap()
        .source_kill_scores()
        .unwrap()[victim_color];

    // Prior player MeteorII carry/progression, real constructed Castle and mana are
    // explicit prerequisites. This actual owner package supplies the rival
    // with3085HP/no Castle/no Shield; none of those NPC fields is changed.
    // C4 accepted activation but source68D50 correctly rejected the old50000
    // Castle store: the unchanged tierII producer requires80000.
    let castle_requirement =
        crate::level_definition::METEOR_PROPERTIES.tiers[1].castle_mana_required;
    {
        let state = sim_mut(&mut game);
        state.apply_carried_spell_families(&[9], Some(9)).unwrap();
        state
            .apply_carried_spell_progression(&[crate::native_world::CarriedSpellProgress {
                family: 9,
                persistent_experience: crate::native_world::LEVEL2_SPELL_TIER_THRESHOLDS[9][1],
                runtime_experience: 0,
                unlocked_tier: 1,
                selected_tier: 1,
            }])
            .unwrap();
        for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
            state
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
                !state.economy().castle_projectile_site_blocked(
                    *position,
                    state.entities(),
                    state.canonical_terrain(),
                )
            })
            .unwrap();
        let castle = state.economy_mut().build_castle(site, player).unwrap();
        assert!(
            state
                .economy_mut()
                .set_castle_mana_prerequisite_for_test(castle, castle_requirement)
        );
        let live = &mut state.session.state.live;
        let row = &mut live.economy.entities[usize::from(player)];
        row.max_mana = 50_000;
        row.record[144..148].copy_from_slice(&50_000_i32.to_le_bytes());
        row.set_mana(50_000);
        row.set_mana_regen(0);
        let row = row.clone();
        live.shared_spells.sync_player_entity_from_authority(&row);
        live.shared_spells.player_mana = row.mana;
        live.shared_spells.caster.mana = row.mana;
        live.shared_spells
            .sync_typed_spell_parent_caches_from_player_entity();
        if let Some(fire) = live.core.context.spell_fire.as_mut() {
            fire.caster.mana = row.mana;
        }
    }
    step(
        &mut game,
        GameCommand {
            assign_spell: Some(SpellAssignment {
                spell_id: crate::game_definition::SemanticId::parse("spell.meteor").unwrap(),
                tier: 1,
                hand: SpellAssignmentHand::Left,
                holder_actor: None,
            }),
            ..Default::default()
        },
    );
    for _ in 0..128 {
        if sim(&game)
            .session
            .state
            .live
            .nyphur_dispatcher
            .get(owner_handle)
            .unwrap()
            .common_upkeep(sim(&game).entities()[usize::from(owner)].phase)
            .word_0x159
            == 0
        {
            break;
        }
        step(&mut game, GameCommand::default());
    }
    let state = sim(&game);
    assert_eq!(
        state
            .session
            .state
            .live
            .nyphur_dispatcher
            .get(owner_handle)
            .unwrap()
            .common_upkeep(state.entities()[usize::from(owner)].phase)
            .word_0x159,
        0
    );
    assert_eq!(state.entities()[112].life, 3085);
    assert_eq!(
        state.entities()[112].control_flags & (0x4000 | 0x0040_0000 | 0x10),
        0
    );
    assert!(state.economy().castle_for_owner(owner).is_none());
    let target = source_target::read(
        state.source_arena(),
        source_target::Context {
            spells: Some(state.shared_spells()),
            particles: Some(&state.session.definition.presentation.particles),
            ..source_target::Context::from_ordinary(
                &state.session.state.live.goats,
                &state.session.state.live.ordinary,
            )
        },
        owner,
    )
    .unwrap()
    .unwrap();
    let top = Position {
        z: target
            .position
            .z
            .wrapping_add(target.collision.unwrap().vertical_offset),
        ..target.position
    };
    let hand = Position {
        z: top.z.wrapping_add(1024),
        ..top
    };
    let aim = crate::level1_mission::projectile_guidance_angles(hand, top);
    let mut position = Position {
        z: hand.z.wrapping_sub(state.state.entity_fov),
        ..hand
    };
    let auxiliary = state.shared_spells().beyond_sight_auxiliary().next_entity;
    crate::move_entity(
        &mut position,
        aim.0.wrapping_sub(auxiliary).wrapping_add(512),
        0,
        256,
    );
    sim_mut(&mut game)
        .set_level2_entity_position(player, position)
        .unwrap();
    sim_mut(&mut game).state.actual_speed = 0;
    sim_mut(&mut game).state.target_speed = 0;
    sim_mut(&mut game).set_level2_player_aim_for_test(aim.0, aim.1);
    let holder = sim(&game)
        .shared_spells()
        .local_player()
        .enabled_spell_holders[9];
    let holder_handle = sim(&game).source_arena().handle(holder).unwrap();
    let price = sim(&game).shared_spells().pickup(holder).unwrap().max_mana;
    assert_eq!(
        price,
        crate::level_definition::METEOR_PROPERTIES.tiers[1].mana_cost
    );
    assert_eq!(
        sim(&game)
            .shared_spells()
            .pickup(holder)
            .unwrap()
            .mana_regen,
        castle_requirement
    );
    assert!(sim(&game).economy().castle_for_owner(player).unwrap().mana >= castle_requirement);
    let released = sim(&game)
        .players()
        .iter()
        .find(|row| row.entity_index == owner)
        .unwrap()
        .enabled_spell_holders
        .into_iter()
        .filter(|slot| *slot > 1)
        .map(|slot| sim(&game).source_arena().handle(slot).unwrap())
        .collect::<Vec<_>>();
    let mut no_cast = game.backend().compatibility_campaign().clone();
    area_damage::start_actor_damage_trace(&[owner_handle]);
    let mut launches = Vec::new();
    let mut impacts = Vec::new();
    let mut effects = Vec::new();
    let mut consumed = Vec::new();
    let mut paid = 0_i32;
    let mut fatal = None;
    let mut elapsed = 0;
    let mut history = Vec::new();
    let cast_admission_state = |game: &Game| {
        let state = sim(game);
        let live = &state.session.state.live;
        let actor = &live.economy.entities[usize::from(player)];
        let a4 = live.shared_spells.local_player();
        let holder_fields = |row: &crate::native_world::EntitySnapshot| {
            (
                row.index,
                row.class,
                row.model,
                row.action,
                row.level_gate,
                row.active_value,
                row.activation_value,
                row.mana_cost,
                row.max_mana,
                row.mana_regen,
                row.source_spell_parent_entity_index(),
            )
        };
        (
            (
                state.source_arena().handle(player),
                actor.action,
                actor.life,
                actor.mana,
                actor.max_mana,
                actor.mana_regen,
                actor.control_flags,
                actor.special_relation.clone(),
            ),
            (
                a4.slot,
                a4.left_spell_slot_raw,
                a4.left_subspell_index,
                a4.enabled_spell_holders[9],
                u16::from_le_bytes([a4.record[42], a4.record[43]]),
            ),
            live.shared_spells.caster,
            live.shared_spells.pickup(holder).map(holder_fields),
            live.economy
                .entities
                .get(usize::from(holder))
                .map(holder_fields),
            live.core.context.spell_fire.as_ref().map(|fire| {
                (
                    fire.caster,
                    fire.left_holder_index,
                    fire.holders
                        .iter()
                        .find(|row| row.entity_index == fire.left_holder_index)
                        .copied(),
                    fire.accepted_models.clone(),
                )
            }),
            live.economy.owner_max_mana_capacity(player),
            live.economy
                .castle_for_owner(player)
                .map(|castle| (castle.source_entity_index, castle.mana)),
        )
    };
    let mut cast_admission = None;
    // ONE fixed public activation; the source owns all projectile guidance
    // and five1600 pulses. C2's tierI collision produced only the later
    // width384 pulse; the initial width0 map-cell scan missed the actor.
    // No later command follows the rival's pose.
    for turn in 0..64 {
        let mana = sim(&game).shared_spells().player_entity().mana;
        sim(&game).start_creature17_trace();
        area_damage::start_effect_damage_trace(owner);
        let first_cast_before = (turn == 0).then(|| cast_admission_state(&game));
        let first_cast_journal = sim(&game).effect_journal().cursor(0).next_sequence;
        step(
            &mut game,
            GameCommand {
                cast_left: turn == 0,
                ..Default::default()
            },
        );
        if let Some(before) = first_cast_before {
            let events = sim(&game)
                .effect_journal()
                .records()
                .iter()
                .filter(|record| {
                    record.sequence >= first_cast_journal
                        && matches!(
                            record.effect,
                            CampaignEffect::Flight(
                                crate::FlightEvent::RequestSpellActivation { .. }
                                    | crate::FlightEvent::ShowHelpHint { .. }
                                    | crate::FlightEvent::PrepareSound { .. }
                            )
                        )
                })
                .map(|record| (record.sequence, record.effect.clone()))
                .collect::<Vec<_>>();
            cast_admission = Some((before, cast_admission_state(&game), events));
        }
        elapsed += 1;
        paid = paid.saturating_add(
            mana.saturating_sub(sim(&game).shared_spells().player_entity().mana)
                .max(0),
        );
        let trace = sim(&game).take_creature17_trace();
        launches.extend(trace.meteor_launches);
        impacts.extend(trace.meteor_impacts);
        effects.extend(area_damage::take_effect_damage_trace());
        consumed.extend(area_damage::take_player_damage_consumptions());
        fatal = consumed
            .iter()
            .find(|visit| {
                visit.target == owner_handle
                    && visit.life_before >= 0
                    && visit.life_after < 0
                    && visit.life_after == visit.life_before.wrapping_sub(visit.mailbox.0)
                    && visit.writer.is_some_and(|writer| {
                        writer.mailbox == visit.mailbox
                            && writer.target == owner_handle
                            && impacts.iter().any(|impact| {
                                writer.source
                                    == area_damage::PlayerMailboxSource::Effect(impact.impact)
                                    && launches.iter().any(|launch| {
                                        launch.projectile == impact.projectile
                                            && launch.holder == holder_handle
                                            && launch.caster == player_handle
                                            && launch.packet.subspell_index == 8000
                                    })
                                    && effects.iter().any(|receipt| {
                                        receipt.target_slot == owner
                                            && receipt.target_generation == owner_handle.generation
                                            && receipt.source_scoped
                                            && receipt.source_slot
                                                == Some(impact.impact.source_slot)
                                            && receipt.writer_sequence == Some(writer.sequence)
                                            && receipt.pulse.payload == 1600
                                    })
                            })
                    })
            })
            .copied();
        let actor = &sim(&game).entities()[usize::from(owner)];
        if turn < 8 || turn % 16 == 0 || fatal.is_some() {
            history.push((turn, actor.life, actor.action, actor.position));
        }
        if fatal.is_some() {
            break;
        }
    }
    assert!(
        fatal.is_some(),
        "one real paid MeteorII must reach the actual castleless rival consumer: cast_admission={cast_admission:?} launches={launches:?} impacts={impacts:?} writes={effects:?} consumed={consumed:?} history={history:?}"
    );
    assert_eq!(
        launches
            .iter()
            .filter(|launch| launch.holder == holder_handle && launch.caster == player_handle)
            .count(),
        1
    );
    assert!(paid >= price);
    assert_eq!(sim(&game).entities()[112].action, 2);
    assert_eq!(
        sim(&game).runtime().objectives[0].stage_selectors[1],
        1,
        "life below zero is not yet source participation banishment"
    );
    let mut max_drop_life = 0;
    for _ in 0..256 {
        step(&mut game, GameCommand::default());
        elapsed += 1;
        if sim(&game).entities()[112].action == 3 {
            assert_eq!(sim(&game).entities()[112].source_dword_10(), 1200);
            for handle in &released {
                assert_eq!(
                    sim(&game).source_arena().handle(handle.source_slot),
                    Some(*handle)
                );
                max_drop_life =
                    max_drop_life.max(sim(&game).entities()[usize::from(handle.source_slot)].life);
            }
            break;
        }
    }
    assert_eq!(sim(&game).entities()[112].action, 3);
    assert!((1..=289).contains(&max_drop_life));
    let died = game
        .snapshot()
        .value
        .participant_notifications
        .into_iter()
        .find(|notice| notice.display_name == victim_name && !notice.is_local)
        .unwrap();
    assert_eq!(
        (died.kind.as_str(), died.remaining_frames),
        ("participant.died", 100)
    );
    assert_eq!(
        sim(&game).player_lifecycle_notifications().entries()[victim_color].source_owner,
        owner
    );
    step(&mut game, GameCommand::default());
    elapsed += 1;
    let state = sim(&game);
    assert_eq!(
        state
            .players()
            .iter()
            .find(|row| row.entity_index == owner)
            .unwrap()
            .objective_player_state(),
        Some(0)
    );
    assert_eq!(
        state.runtime().objectives[0].stage_selectors[1],
        2,
        "the package's real bound rival objective observes participation, not merely HP"
    );
    assert!(
        state
            .campaign_entity_presentations(0)
            .unwrap()
            .iter()
            .all(|view| view.entity_index != owner)
    );
    assert_eq!(
        state
            .players()
            .iter()
            .find(|row| row.entity_index == player)
            .unwrap()
            .source_kill_scores()
            .unwrap()[victim_color],
        score_before.wrapping_add(1)
    );
    assert!(!game.snapshot().value.return_to_title_requested);
    let banished = game
        .snapshot()
        .value
        .participant_notifications
        .into_iter()
        .find(|notice| notice.participant == died.participant)
        .unwrap();
    assert_eq!(
        (
            banished.kind.as_str(),
            banished.remaining_frames,
            banished.is_local
        ),
        ("participant.banished", 200, false),
        "notice survives the actual participation clear"
    );
    let gameplay_before_frames = (
        sim(&game).state.clone(),
        sim(&game)
            .source_arena()
            .reset_hash_with_terrain(sim(&game).canonical_terrain()),
        sim(&game).session.state.live.nyphur_dispatcher.clone(),
    );
    game.presentation_frame(game.identity().guard(), false)
        .unwrap();
    assert!(
        game.snapshot()
            .value
            .participant_notifications
            .iter()
            .any(|notice| *notice == banished)
    );
    for remaining in (0..200).rev() {
        game.presentation_frame(game.identity().guard(), true)
            .unwrap();
        assert!(
            game.snapshot()
                .value
                .participant_notifications
                .iter()
                .any(|notice| notice.participant == banished.participant
                    && notice.remaining_frames == remaining)
        );
    }
    game.presentation_frame(game.identity().guard(), true)
        .unwrap();
    assert!(
        game.snapshot()
            .value
            .participant_notifications
            .iter()
            .all(|notice| notice.participant != banished.participant)
    );
    assert_eq!(
        (
            sim(&game).state.clone(),
            sim(&game)
                .source_arena()
                .reset_hash_with_terrain(sim(&game).canonical_terrain()),
            sim(&game).session.state.live.nyphur_dispatcher.clone()
        ),
        gameplay_before_frames
    );
    for _ in 0..=max_drop_life {
        step(&mut game, GameCommand::default());
        elapsed += 1;
    }
    assert!(
        released
            .iter()
            .all(|handle| sim(&game).source_arena().resolve(*handle).is_none()),
        "released spell generations expire and reclaim naturally: original_max_life={max_drop_life} current={:?}",
        released
            .iter()
            .map(|handle| {
                let state = sim(&game);
                let row = &state.entities()[usize::from(handle.source_slot)];
                let owner = state
                    .source_arena()
                    .resolve(*handle)
                    .map(|view| match view {
                        crate::campaign::source_arena::SourceEntityView::Imported(_) => {
                            "imported".to_owned()
                        }
                        crate::campaign::source_arena::SourceEntityView::Dynamic(kind) => {
                            format!("dynamic:{kind:?}")
                        }
                    });
                (
                    *handle,
                    state.source_arena().handle(handle.source_slot),
                    owner,
                    (
                        row.active,
                        row.class,
                        row.model,
                        row.action,
                        row.life,
                        row.phase,
                        row.control_flags,
                        row.source_spell_parent_entity_index(),
                    ),
                    state.shared_spells().pickup(handle.source_slot).map(|row| {
                        (
                            row.active,
                            row.class,
                            row.model,
                            row.action,
                            row.life,
                            row.phase,
                            row.control_flags,
                        )
                    }),
                )
            })
            .collect::<Vec<_>>()
    );
    assert!(
        game.snapshot()
            .value
            .participant_notifications
            .iter()
            .all(|notice| notice.participant != banished.participant),
        "later banished visits must not reannounce"
    );
    assert_eq!(
        sim(&game).source_arena().handle(owner),
        Some(owner_handle),
        "source retains the hidden action3 actor; disappearance is participation/presentation"
    );
    for _ in 0..elapsed {
        no_cast.step(PlayerCommand::default(), 0);
        assert!(
            no_cast
                .simulation()
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
    }
    assert!(no_cast.simulation().entities()[112].life >= 0);
    assert_eq!(
        no_cast.simulation().runtime().objectives[0].stage_selectors[1],
        1
    );
    assert_eq!(
        no_cast
            .simulation()
            .players()
            .iter()
            .find(|row| row.entity_index == owner)
            .unwrap()
            .objective_player_state(),
        Some(1)
    );
}

#[test]
fn castle_menu_cost_and_remove_command_use_authoritative_player_state() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-bhq8-level1-collision-v5-night");
    let mut campaign = CampaignSession::open(package, CampaignSelection::LEVEL1).unwrap();
    let simulation = campaign.simulation_mut();
    // Retained learned-Castle fixture through the real carry constructor.
    // The fresh Level1 package itself starts with Fireball/Possession only.
    simulation
        .apply_carried_spell_families(&[2], Some(2))
        .unwrap();
    let owner = simulation.state.entity_index;
    let holder = simulation
        .shared_spells()
        .local_player()
        .enabled_spell_holders[2];
    assert_ne!(holder, 0, "ordinary carry constructor supplies Castle");
    let compact_spells = simulation.shared_spells().local_player.record[398..450].to_vec();
    // Controlled post-construction Castle fixture, isolating the actual menu
    // and PlayerEvents commands without replaying the whole mission route.
    let position = simulation.state.position;
    let castle = simulation
        .session
        .state
        .live
        .economy
        .economy
        .build_castle_at(position, position, owner, false, false)
        .unwrap();
    let ui_index = SOURCE_SPELL_INDEX_MAP
        .iter()
        .position(|family| *family == 2)
        .unwrap();
    campaign.step(
        PlayerCommand {
            opcode: 31,
            argument_1: ui_index as i8,
            argument_2: 0,
            ..PlayerCommand::default()
        },
        u8::MAX,
    );
    let simulation = campaign.simulation_mut();
    let profile = simulation.shared_spells().pickup(holder).unwrap();
    assert_eq!((profile.max_mana, profile.mana), (10_000, 10_000 / 101));
    campaign.step(
        PlayerCommand {
            opcode: 42,
            ..PlayerCommand::default()
        },
        u8::MAX,
    );
    let simulation = campaign.simulation_mut();
    assert_eq!(
        simulation.economy().castle().unwrap().source_entity_index,
        castle
    );
    assert_eq!(simulation.economy().castle().unwrap().life, -1);
    assert_eq!(simulation.entities()[usize::from(castle)].life, -1);
    assert_eq!(
        simulation.shared_spells().local_player().castle_destroyed,
        1
    );
    assert_eq!(
        simulation
            .players()
            .iter()
            .find(|row| row.entity_index == owner)
            .unwrap()
            .castle_destroyed,
        1
    );
    assert_eq!(
        &simulation.shared_spells().local_player.record[398..450],
        compact_spells
    );
    let changed = ResetReceipt::from_state(&simulation.session.state.live);
    simulation
        .session
        .state
        .live
        .set_player_castle_destroyed(owner, 0);
    assert_ne!(
        changed,
        ResetReceipt::from_state(&simulation.session.state.live)
    );
    campaign.reset();
    assert_eq!(
        campaign
            .simulation()
            .shared_spells()
            .local_player()
            .castle_destroyed,
        0
    );
}

#[test]
fn spawn_guard_and_hit_hud_fields_keep_compact_spell_state_intact() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level3-day-block32-campaign-recipe-v2");
    let mut campaign = CampaignSession::open(package, CampaignSelection::LEVEL3).unwrap();
    let session = campaign.simulation_mut();
    assert_eq!(session.session.state.live.player_protection_ticks, 100);
    let initial = ResetReceipt::from_state(&session.session.state.live);
    let spell_bytes = session.shared_spells().local_player().record_bytes()[398..450].to_vec();
    let player = session.state.entity_index;
    let attacker = session
        .entities()
        .iter()
        .find(|row| row.active && row.class == 5 && row.model == 9)
        .unwrap()
        .index;
    let life = session.session.state.live.player_life;
    session.queue_level2_player_contact_damage(500, attacker);
    campaign.step(PlayerCommand::default(), 0);
    let session = campaign.simulation_mut();
    assert_eq!(session.session.state.live.player_protection_ticks, 99);
    assert_eq!(session.session.state.live.player_life, life);
    assert_eq!(session.session.state.live.player_pending_attacker, 0);
    assert_eq!(session.shared_spells().local_player().hit_frame_time, 0);

    // Isolate the first unprotected visit without advancing a playthrough.
    session.session.state.live.player_protection_ticks = 0;
    session.queue_level2_player_contact_damage(500, attacker);
    campaign.step(PlayerCommand::default(), 0);
    let session = campaign.simulation_mut();
    let shared = session.shared_spells().local_player();
    let expected_timer = if level2_day_color_phases(shared.turn).0 == 0 {
        4
    } else {
        3
    };
    assert_eq!(shared.hit_frame_time, expected_timer);
    assert_eq!(&shared.record_bytes()[398..450], spell_bytes);
    let canonical = session
        .players()
        .iter()
        .find(|row| row.entity_index == player)
        .unwrap();
    assert_eq!(canonical.hit_frame_time, expected_timer);
    assert_eq!(&canonical.record_bytes()[398..450], spell_bytes);
    let with_guard_zero = ResetReceipt::from_state(&session.session.state.live);
    session.session.state.live.player_protection_ticks = 1;
    assert_ne!(
        with_guard_zero,
        ResetReceipt::from_state(&session.session.state.live)
    );
    campaign.reset();
    let session = campaign.simulation();
    assert_eq!(session.session.state.live.player_protection_ticks, 100);
    assert_eq!(session.shared_spells().local_player().hit_frame_time, 0);
    assert_eq!(
        ResetReceipt::from_state(&session.session.state.live),
        initial
    );
}

#[test]
fn acquired_shield_lethal_hit_publishes_death_once() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level3-day-block32-campaign-recipe-v2");
    let mut campaign = CampaignSession::open(package, CampaignSelection::LEVEL3)
        .expect("actual Level 3 opening package");
    let player = campaign.simulation().state.entity_index;
    let pickup = campaign.simulation().entities()[124].clone();
    assert_eq!((pickup.class, pickup.model, pickup.action), (15, 6, 19));
    campaign
        .simulation_mut()
        .set_level2_entity_position(player, pickup.position)
        .expect("approach the original Shield pickup");
    for _ in 0..4 {
        campaign.step(PlayerCommand::default(), 0);
        if campaign
            .simulation()
            .shared_spells()
            .local_player()
            .enabled_spell_holders[6]
            == pickup.index
        {
            break;
        }
    }
    assert_eq!(
        campaign
            .simulation()
            .shared_spells()
            .local_player()
            .enabled_spell_holders[6],
        pickup.index,
        "the ordinary pickup callback acquires Shield"
    );
    let ui_index = crate::SOURCE_SPELL_INDEX_MAP
        .iter()
        .position(|family| *family == 6)
        .expect("original Shield menu binding") as i8;
    campaign.step(
        PlayerCommand {
            opcode: 31,
            argument_1: ui_index,
            argument_2: 0,
            ..PlayerCommand::default()
        },
        0,
    );
    for _ in 0..8 {
        campaign.step(
            PlayerCommand {
                flags: 0x10,
                ..PlayerCommand::default()
            },
            0,
        );
        if campaign
            .simulation()
            .shared_spells()
            .shield_state()
            .is_some_and(|shield| shield.active_value > 0)
        {
            break;
        }
    }
    assert!(
        campaign
            .simulation()
            .shared_spells()
            .shield_state()
            .is_some_and(|shield| shield.active_value > 0),
        "ordinary spell input arms the acquired holder"
    );
    campaign.step(PlayerCommand::default(), 0);
    assert_ne!(
        campaign
            .simulation()
            .shared_spells()
            .shield_state()
            .unwrap()
            .parent_status_flags
            & crate::spell_activation::SHIELD_MANA_STATUS_BIT,
        0,
        "the scheduled Shield pulse establishes quarter-damage protection"
    );

    let session = campaign.simulation_mut();
    let shield_experience_before = session.shared_spells().spell_experience()[6];
    let attacker = session
        .entities()
        .iter()
        .find(|row| row.active && row.class == 5 && row.model == 9)
        .expect("real opening Skeleton supplies the hit owner")
        .index;
    // Isolate a low-life incoming hit, preserving the actual acquired/cast
    // Shield and its numeric schedule. No status or activation is injected.
    {
        let live = &mut session.session.state.live;
        live.player_protection_ticks = 0;
        live.player_life = 10;
        live.economy.entities[usize::from(player)].set_life(10);
        live.shared_spells.set_player_entity_life(10);
        live.shared_spells
            .sync_typed_spell_parent_caches_from_player_entity();
    }
    session.queue_level2_player_contact_damage(200, attacker);
    let journal_start = session.effect_journal().records().len();
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .dispatch_events()
            .iter()
            .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
        "the real source turn must not roll back: {:?}",
        session.dispatch_events()
    );
    assert_eq!(session.player_life, -40);
    assert_eq!(session.entities()[usize::from(player)].action, 2);
    assert_eq!(
        session.shared_spells().spell_experience()[6],
        shield_experience_before + 1,
        "a real Shield hit awards source family6 experience even when lethal"
    );
    assert_eq!(
        session
            .session
            .state
            .live
            .core
            .players
            .iter()
            .find(|linked| linked.entity_index == player)
            .unwrap()
            .runtime_spell_experience[6],
        shield_experience_before + 1,
        "the canonical player and spell view retain the same hit award"
    );
    let death_counts = |session: &Mc2Simulation| {
        let records = &session.effect_journal().records()[journal_start..];
        let sound = records
            .iter()
            .filter(|record| {
                matches!(
                    record.effect,
                    CampaignEffect::Flight(FlightEvent::PrepareSound {
                        entity_index,
                        sound_id: 16,
                        ..
                    }) if entity_index == player
                )
            })
            .count();
        let scene = records
            .iter()
            .filter(|record| matches!(record.effect, CampaignEffect::SceneChange { scene: 4 }))
            .count();
        (sound, scene)
    };
    assert_eq!(death_counts(session), (1, 1));
    assert_eq!(
        session.effect_journal().records()[journal_start..]
            .iter()
            .filter(|record| matches!(
                record.effect,
                CampaignEffect::Flight(FlightEvent::PrepareSound {
                    audio_site: crate::MissionAudioSite::PlayerPain,
                    entity_index,
                    sound_id: 54..=57,
                    ..
                }) if entity_index == player
            ))
            .count(),
        1,
        "Shield reduction retains the source pain feedback before death"
    );
    assert!(
        session.effect_journal().records()[journal_start..]
            .iter()
            .any(|record| matches!(
                record.effect,
                CampaignEffect::SharedSpell(Level2SharedSpellEvent::ShieldDamageApplied {
                    damage,
                    ..
                }) if damage.raw_damage == 200 && damage.applied_damage == 50
            )),
        "the fatal hit still travels through Shield's ordinary damage consumer"
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        session.shared_spells().spell_experience()[6],
        shield_experience_before + 1,
        "an already-dead player's retained damage mailbox cannot award twice"
    );
    assert_eq!(
        death_counts(session),
        (1, 1),
        "the following dying visit must not repeat the death transition"
    );
}

#[test]
fn ordinary_player_hit_reaches_flight_music_rng_and_source_player_views() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level3-day-block32-campaign-recipe-v2");
    let mut campaign = CampaignSession::open(package, CampaignSelection::LEVEL3)
        .expect("actual Level 3 opening package");
    let session = campaign.simulation_mut();
    let player = session.state.entity_index;
    let attacker = session
        .entities()
        .iter()
        .find(|row| row.active && row.class == 5 && row.model == 9)
        .expect("opening Skeleton")
        .index;
    assert!(
        attacker < player,
        "the attacker moves before the hit consumer"
    );
    let mut position = session.state.position;
    position.z = session.canonical_terrain().ground_height(position) + 8_192;
    session
        .set_level2_entity_position(player, position)
        .expect("isolate one incoming mailbox above ordinary creature contacts");
    let rng_before = session.entities()[usize::from(player)].entity_rng;
    let rng_after = rng_before.wrapping_mul(9_377).wrapping_add(9_439);
    {
        let live = &mut session.session.state.live;
        live.player_protection_ticks = 0;
        live.player_life = 10_000;
        live.economy.entities[usize::from(player)].set_life(10_000);
        live.shared_spells.set_player_entity_life(10_000);
        live.core.state.target_speed = 0;
        live.core.state.actual_speed = 0;
        live.core.state.recoil_speed = 0;
        live.core.state.music_countdown = 0;
    }
    session.queue_level2_player_contact_damage(500, attacker);
    let journal_start = session.effect_journal().records().len();
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .dispatch_events()
            .iter()
            .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
        "hit turn must commit: {:?}",
        session.dispatch_events()
    );
    let state = &session.session.state.live.core.state;
    let expected_yaw =
        crate::heading_for_positions(session.entities()[usize::from(attacker)].position, position)
            as i16;
    assert_eq!(state.recoil_yaw, expected_yaw);
    assert_eq!(
        state.recoil_speed, 46,
        "damage/10 recoil is consumed and decays by four in the same flight visit"
    );
    assert_eq!(state.music_countdown, 99);
    assert_ne!(
        (state.position.x, state.position.y),
        (position.x, position.y),
        "the source body applies hit recoil as real movement"
    );
    assert_eq!(
        session.entities()[usize::from(player)].entity_rng,
        rng_after
    );
    assert_eq!(
        session.shared_spells().player_entity().entity_rng,
        rng_after,
        "the shared spell projection cannot retain the pre-hit source RNG"
    );
    for linked in session
        .session
        .state
        .live
        .core
        .players
        .iter()
        .chain(session.shared_spells().players().iter())
        .chain(std::iter::once(session.shared_spells().local_player()))
        .filter(|linked| linked.entity_index == player)
    {
        assert_eq!(&linked.record[32..34], &state.recoil_speed.to_le_bytes());
        assert_eq!(&linked.record[34..36], &state.recoil_yaw.to_le_bytes());
        assert_eq!(&linked.record[38..40], &state.effective_pitch.to_le_bytes());
    }
    let records = &session.effect_journal().records()[journal_start..];
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(
                record.effect,
                CampaignEffect::Flight(FlightEvent::PrepareSound {
                    audio_site: crate::MissionAudioSite::PlayerPain,
                    entity_index,
                    sound_id,
                    ..
                }) if entity_index == player && sound_id == 54 + (rng_after & 3)
            ))
            .count(),
        1
    );
    assert!(records.iter().any(|record| matches!(
        record.effect,
        CampaignEffect::Flight(FlightEvent::UpdateMusic { mode: 2, .. })
    )));
    assert!(
        records
            .iter()
            .all(|record| !matches!(record.effect, CampaignEffect::SceneChange { scene: 4 }))
    );
}
