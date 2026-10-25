use super::super::tests::{level_two_campaign_session, level_two_fixture_path};
use super::super::*;
use super::archer::{
    ARCHER_ACTION_ATTACK, ARCHER_DESCRIPTOR_CADENCE, ARCHER_MODEL, ArcherChildRequest,
};
use crate::{
    goat::{GOAT_PATROL_ACTION, GoatState, GoatStepEvent},
    villager::{VILLAGER_DEATH_ACTION, VILLAGER_HIT_ACTION},
};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn update_level_two_goat<F>(
    session: &mut Level2ResetSession,
    source_slot: u16,
    update: F,
) -> GoatState
where
    F: FnOnce(&mut GoatState),
{
    let mut goat = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == source_slot)
        .cloned()
        .expect("qualified Level 2 Goat source row");
    update(&mut goat);
    assert!(session.goats.replace_goat_for_test(goat.clone()));
    goat
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CreatureAudioReceipt {
    source_slot: u16,
    entity_index: u16,
    sound_id: u16,
    action: u8,
    position: Position,
    audio_site: crate::MissionAudioSite,
}

fn creature_audio_receipts(session: &Level2ResetSession) -> Vec<CreatureAudioReceipt> {
    let mut receipts = Vec::new();
    for dispatch in session.dispatch_events() {
        let Level2DispatchEvent::Goat {
            source_slot,
            events,
            ..
        } = dispatch
        else {
            continue;
        };
        for event in events {
            if let GoatStepEvent::Bleat {
                audio_site,
                entity_index,
                sound_id,
                position,
                action,
            } = event
            {
                receipts.push(CreatureAudioReceipt {
                    source_slot: *source_slot,
                    entity_index: *entity_index,
                    sound_id: *sound_id,
                    action: *action,
                    position: *position,
                    audio_site: *audio_site,
                });
            }
        }
    }
    receipts
}

fn configure_three_level_two_creature_sounds(session: &mut Level2ResetSession) {
    let selected = [39_u16, 40, 42];
    for mut goat in session.goats().to_vec() {
        if selected.contains(&goat.entity_index) {
            continue;
        }
        // Keep muted rows on a compiled Goat callback. Action 0 is a real
        // source callback that this bounded Goat implementation has not
        // closed, so using it as a fake no-op would let the old friendly route
        // vector bypass the registry. This action-9 seed advances without a
        // bleat or perception turn.
        goat.action = 9;
        goat.pending_damage = 0;
        goat.pending_attacker = 0;
        goat.current_attacker = 0;
        goat.partner_entity_index = None;
        goat.target_entity_index = None;
        goat.actual_speed = 0;
        goat.rng = 0;
        goat.phase = 1;
        goat.proximity_countdown = 0;
        goat.proximity_subcounter = 0;
        assert!(session.goats.replace_goat_for_test(goat));
    }

    // AUD-L2-005: sub_1F470/action 11, whose null-partner transition reaches
    // the one-draw sound-46 tail before the source cursor epilogue.
    update_level_two_goat(session, 39, |goat| {
        goat.action = 11;
        goat.partner_entity_index = None;
        goat.target_entity_index = None;
        goat.pending_damage = 0;
        goat.pending_attacker = 0;
        goat.current_attacker = 0;
        goat.rng = 129;
        goat.phase = 0;
    });

    // AUD-L2-004: sub_1F3C0/action 9, kept off its two-draw perception
    // cadence so the receipt isolates its one-draw sound-46 tail.
    update_level_two_goat(session, 40, |goat| {
        goat.action = 9;
        goat.partner_entity_index = None;
        goat.target_entity_index = None;
        goat.pending_damage = 0;
        goat.pending_attacker = 0;
        goat.current_attacker = 0;
        goat.actual_speed = 0;
        goat.rng = 129;
        goat.phase = 1;
        goat.proximity_countdown = 0;
        goat.proximity_subcounter = 0;
    });

    // AUD-L2-006: HitGoat_1F530/action 14, whose direct-hit tail uses the
    // independent modulo-43 sound-46 gate.
    let player_entity_index = session.state.entity_index;
    let hit_goat = update_level_two_goat(session, 42, |goat| {
        goat.action = 14;
        goat.partner_entity_index = None;
        goat.target_entity_index = Some(player_entity_index);
        goat.pending_damage = 0;
        goat.pending_attacker = 0;
        goat.current_attacker = 0;
        goat.rng = 62;
        goat.phase = 1;
    });
    session.state.position = hit_goat.position;
    session.economy.entities[usize::from(player_entity_index)].position = hit_goat.position;
    session
        .submit_goat_damage(42, 1, player_entity_index)
        .expect("qualified direct-hit Goat audio branch");
}

fn assert_no_goat_bleat(events: &[GoatStepEvent]) {
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GoatStepEvent::Bleat { .. }))
    );
}

fn activate_group20_archers() -> (Level2ResetSession, u64) {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let authored_features = session.mission().authored_features().to_vec();
    let feature_position = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored group-20 feature")
            .world_position(0)
    };

    session.state.position = feature_position(11);
    session.runtime.objectives[0].stage_selectors[1] = 2;
    session.runtime.objectives[0].stage_selectors[2] = 2;
    session.step_outer_turn(PlayerCommand::default(), 0);
    session.runtime.objectives[0].stage_selectors[3] = 2;
    session.step_outer_turn(PlayerCommand::default(), 0);
    session.state.position = feature_position(214);

    let expected = [
        216_u16, 217, 218, 219, 220, 221, 222, 223, 232, 233, 234, 235,
    ];
    let activated = (0..8).any(|_| {
        session.step_outer_turn(PlayerCommand::default(), 0);
        session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
                    group: 20,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == expected
            )
        })
    });
    assert!(activated, "row 214 must activate group 20 in production");

    let authored_rows = session
        .economy()
        .authored_class5()
        .iter()
        .filter(|source| source.active && source.model == ARCHER_MODEL)
        .map(|source| source.authored_index)
        .collect::<BTreeSet<_>>();
    assert_eq!(authored_rows, BTreeSet::from([232, 233, 234, 235]));
    (session, baseline_hash)
}

fn produce_archer_child(session: &mut Level2ResetSession) -> (u16, u16, i16) {
    let root = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 232)
        .copied()
        .expect("group-20 row 232 Archer root");
    let player_entity = session.state.entity_index;
    // Keep the ordinary arrow in flight for the following parent/child
    // scheduling test, rather than colliding with an overlapping player in
    // its very first source visit. Day permits this vertical target and the
    // 4,096-unit separation remains within the 5,120-unit Archer range.
    let target_position = Position {
        z: root.position.z.wrapping_add(4_096),
        ..root.position
    };
    session.state.position = target_position;
    session
        .set_level2_entity_position(player_entity, target_position)
        .expect("local player accepts Archer target position");
    let parent_roll = 320;
    let mut economy_view = session.economy_mut();
    let source = economy_view
        .authored_class5
        .iter_mut()
        .find(|source| source.source_entity_index == root.source_entity_index)
        .expect("Archer root remains mutable");
    source.action = ARCHER_ACTION_ATTACK;
    source.actual_speed = 0;
    source.roll = parent_roll;
    source.source_byte_3e = ARCHER_DESCRIPTOR_CADENCE;
    source.word_0x96_150 = player_entity;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let child = session
        .economy()
        .authored_class5_archer_children()
        .iter()
        .find(|child| child.parent_entity_index == root.source_entity_index)
        .copied()
        .expect("ordinary Archer cursor retains a positive child");
    assert!(child.active && child.drawing_enabled && child.life > 0);
    assert_eq!(child.action, super::archer::ARCHER_CHILD_ACTION);
    (
        root.source_entity_index,
        child.source_entity_index,
        parent_roll,
    )
}

#[test]
fn level_two_archers_complete_constructor_action_and_child_chain() {
    let (mut session, baseline_hash) = activate_group20_archers();
    let (root_slot, child_slot, parent_roll) = produce_archer_child(&mut session);

    let roots = session
        .economy()
        .authored_class5()
        .iter()
        .filter(|source| source.active && source.model == ARCHER_MODEL)
        .map(|source| (source.authored_index, source.source_entity_index))
        .collect::<Vec<_>>();
    assert_eq!(
        roots.iter().map(|(row, _)| *row).collect::<BTreeSet<_>>(),
        BTreeSet::from([232, 233, 234, 235])
    );
    assert_eq!(
        roots
            .iter()
            .map(|(_, slot)| *slot)
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );
    let root = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.source_entity_index == root_slot)
        .copied()
        .expect("row-232 root remains live");
    let archer_state = session
        .economy()
        .authored_class5_archer()
        .iter()
        .find(|state| state.source_entity_index == root_slot)
        .copied()
        .expect("row-232 typed Archer state");
    let child = session
        .economy()
        .authored_class5_archer_children()
        .iter()
        .find(|child| child.source_entity_index == child_slot)
        .copied()
        .expect("retained Archer child");
    // sub_1CCE0 produces an independent arrow, never a word-0x34 body
    // link whose 13-tick lifetime could overwrite the Archer's 1,000 life.
    assert_eq!(archer_state.word_0x34, 0);
    assert_eq!(child.word_0x34, 0);
    assert_eq!(child.parent_entity_index, root_slot);
    assert_eq!(child.word_0x96, root.word_0x96_150);
    assert_ne!(parent_roll, 0);
    assert_eq!(root.roll, parent_roll);
    assert_eq!(
        child.roll, 0,
        "the arrow retains its NewEvent-cleared body roll"
    );
    assert!(child.slot_generation > 0);
    assert!(
        child.phase > 0,
        "child was visited by the live source cursor"
    );
    assert!(
        session
            .economy()
            .map_chains()
            .linked_entity_indices()
            .iter()
            .any(|slot| *slot == root_slot)
    );
    assert!(
        session
            .economy()
            .map_chains()
            .linked_entity_indices()
            .iter()
            .any(|slot| *slot == child_slot)
    );
    assert!(session.economy().live_arena_slots().contains(&(
        child_slot,
        Level2DynamicArenaKind::AuthoredClass5ArcherChild
    )));
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Level2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::AuthoredClass5ArcherChild,
                ..
            } if *source_slot == child_slot
        )
    }));

    let first_archers = session
        .economy()
        .authored_class5()
        .iter()
        .filter(|source| source.model == ARCHER_MODEL)
        .copied()
        .collect::<Vec<_>>();
    let first_states = session.economy().authored_class5_archer().to_vec();
    let first_children = session.economy().authored_class5_archer_children().to_vec();
    let first_hash = session.campaign_state_hash();

    session.reset();
    assert!(session.economy().authored_class5_archer().is_empty());
    assert!(
        session
            .economy()
            .authored_class5_archer_children()
            .is_empty()
    );
    assert!(
        session
            .economy()
            .authored_class5()
            .iter()
            .all(|source| !matches!(source.authored_index, 232 | 233 | 234 | 235))
    );
    assert_eq!(session.campaign_state_hash(), baseline_hash);

    let (mut replay, replay_baseline_hash) = activate_group20_archers();
    assert_eq!(replay_baseline_hash, baseline_hash);
    produce_archer_child(&mut replay);
    assert_eq!(
        replay
            .economy()
            .authored_class5()
            .iter()
            .filter(|source| source.model == ARCHER_MODEL)
            .copied()
            .collect::<Vec<_>>(),
        first_archers
    );
    assert_eq!(replay.economy().authored_class5_archer(), first_states);
    assert_eq!(
        replay.economy().authored_class5_archer_children(),
        first_children
    );
    assert_eq!(replay.campaign_state_hash(), first_hash);
}

#[test]
fn archer_root_and_child_allocation_failures_are_atomic() {
    // An empty ordinary free stack follows NewEvent's lowest-replaceable
    // fallback. Clearing the stack is not itself an exhaustion negative.
    let (mut fallback_session, _) = activate_group20_archers();
    fallback_session.economy_mut().free_slots.clear();
    let fallback = fallback_session
        .economy_mut()
        .allocate_authored_archer(235, Position { x: 0, y: 0, z: 0 })
        .expect("empty free stack evicts the lowest replaceable source row");
    assert!(
        fallback_session
            .economy()
            .authored_class5()
            .iter()
            .any(|source| source.source_entity_index == fallback.source_entity_index)
    );
    assert!(fallback_session.economy().free_slots.is_empty());

    let (mut session, _) = activate_group20_archers();
    session.economy_mut().free_slots = vec![u16::MAX];
    let before_no_slot = session.economy().clone();
    let hash_before_no_slot = session.campaign_state_hash();
    assert!(
        session
            .economy_mut()
            .allocate_authored_archer(235, Position { x: 0, y: 0, z: 0 })
            .is_none()
    );
    assert_eq!(session.economy(), &before_no_slot);
    assert_eq!(session.campaign_state_hash(), hash_before_no_slot);

    session.economy_mut().free_slots = vec![u16::MAX];
    let before_map_failure = session.economy().clone();
    let hash_before_map_failure = session.campaign_state_hash();
    assert!(
        session
            .economy_mut()
            .allocate_authored_archer(235, Position { x: 0, y: 0, z: 0 })
            .is_none()
    );
    assert_eq!(session.economy(), &before_map_failure);
    assert_eq!(session.campaign_state_hash(), hash_before_map_failure);

    let root = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 232)
        .copied()
        .expect("row-232 root for child rollback");
    let request = ArcherChildRequest {
        parent_entity_index: root.source_entity_index,
        parent_entity_id: root.source_entity_index,
        parent_word_0x96: root.word_0x96_150,
        parent_roll: root.roll,
        parent_position: root.position,
        parent_fov: root.fov,
        target_entity_index: session.state.entity_index,
        target_class: 3,
        target_model: 0,
        target_position: root.position,
    };
    session.economy_mut().free_slots = vec![u16::MAX];
    let before_child_map_failure = session.economy().clone();
    let hash_before_child_map_failure = session.campaign_state_hash();
    assert!(
        session
            .economy_mut()
            .allocate_archer_child_packet(request)
            .is_none()
    );
    assert_eq!(session.economy(), &before_child_map_failure);
    assert_eq!(session.campaign_state_hash(), hash_before_child_map_failure);
}

#[test]
fn archer_hit_and_kill_propagate_through_the_retained_child_chain() {
    let (mut session, _) = activate_group20_archers();
    let (root_slot, child_slot, _) = produce_archer_child(&mut session);
    assert!(
        child_slot > root_slot,
        "this receipt specifically proves the parent-before-child source order"
    );
    let mut economy_view = session.economy_mut();
    // Ordinary arrows are independent. This focused PreKillEntity test
    // explicitly stages its word-0x34 input chain instead of attributing a
    // body link to the ordinary Archer projectile constructor.
    let state = economy_view
        .authored_class5_archer
        .iter_mut()
        .find(|state| state.source_entity_index == root_slot)
        .expect("Archer state for explicit linked-chain input");
    assert_eq!(state.word_0x34, 0);
    state.word_0x34 = child_slot;
    let root = economy_view
        .authored_class5
        .iter_mut()
        .find(|source| source.source_entity_index == root_slot)
        .expect("Archer root for hit transition");
    root.action = super::archer::ARCHER_ACTION_HIT;
    root.dword_0x10 = 0;
    // HitArcher advances 7 to 8; the following KillArcher visit is the
    // actual source phase-zero producer, not an assumed immediate kill.
    root.source_byte_3e = 7;
    root.phase = 7;

    session.step_outer_turn(PlayerCommand::default(), 0);
    let child_receipts = session.cf10_production_schedule_receipts();
    assert!(child_receipts.iter().any(|receipt| matches!(
        receipt,
        crate::campaign::schedule::ScheduleReceipt::SlotResolved {
            slot,
            kind: crate::campaign::schedule::VisitKind::Terminal,
            ..
        } if *slot == child_slot
    )));
    assert!(child_receipts.iter().any(|receipt| matches!(
        receipt,
        crate::campaign::schedule::ScheduleReceipt::CleanupCompleted { slot, .. }
            if *slot == child_slot
    )));
    assert!(!child_receipts.iter().any(|receipt| matches!(
        receipt,
        crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, .. }
            | crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted { slot, .. }
            if *slot == child_slot
    )));
    assert_eq!(
        session
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.source_entity_index == root_slot)
            .expect("root after pre-kill")
            .action,
        super::archer::ARCHER_ACTION_KILL
    );
    assert_eq!(
        session
            .economy()
            .authored_class5_archer_children()
            .iter()
            .find(|child| child.source_entity_index == child_slot)
            .expect("child after pre-kill")
            .action,
        super::archer::ARCHER_ACTION_KILL
    );
    assert!(
        !session
            .economy()
            .authored_class5_archer_children()
            .iter()
            .find(|child| child.source_entity_index == child_slot)
            .expect("higher-slot child is visited after pre-kill")
            .drawing_enabled,
        "class 9/action 37 is an invalid selector: UpdateEntities hides it without a callback"
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !session
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.source_entity_index == root_slot)
            .expect("root after terminal kill")
            .drawing_enabled
    );
    assert!(
        session
            .economy()
            .authored_class5_archer_children()
            .iter()
            .all(|child| child.source_entity_index != child_slot)
    );
    assert!(
        session
            .economy()
            .live_arena_slots()
            .iter()
            .any(|(slot, _)| *slot == root_slot),
        "the root hidden by KillArcher remains allocated until the next UpdateEntities prologue"
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .authored_class5()
            .iter()
            .all(|source| source.source_entity_index != root_slot)
    );
    assert!(
        session
            .economy()
            .authored_class5_archer_children()
            .iter()
            .all(|child| child.source_entity_index != child_slot)
    );
}

#[test]
fn archer_action37_cadence_advances_rng_copies_owner_and_preserves_life() {
    let (mut session, _) = activate_group20_archers();
    let root_slot = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 232)
        .expect("group-20 row 232 Archer root")
        .source_entity_index;
    let owner_entity_index = session.state.entity_index;
    let initial_rng = 0x3456_u16;
    let retained_life = 777_i32;
    let initial_sphere_slots = session
        .economy()
        .spheres()
        .iter()
        .map(|sphere| sphere.source_entity_index)
        .collect::<BTreeSet<_>>();
    let initial_burst_slots = session
        .economy()
        .death_bursts()
        .iter()
        .map(|burst| burst.source_entity_index)
        .collect::<BTreeSet<_>>();
    {
        let mut economy = session.economy_mut();
        let source = economy
            .authored_class5
            .iter_mut()
            .find(|source| source.source_entity_index == root_slot)
            .expect("Archer root remains mutable");
        source.action = super::archer::ARCHER_ACTION_KILL;
        source.source_byte_3e = 1;
        source.phase = 1;
        source.entity_rng = initial_rng;
        source.mana = 321;
        source.life = retained_life;
        economy.source_owner.insert(root_slot, owner_entity_index);
    }

    for expected_source_byte in 2_u8..=8 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        let source = session
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.source_entity_index == root_slot)
            .expect("off-cadence Archer remains live");
        assert!(source.drawing_enabled);
        assert_eq!(source.life, retained_life);
        assert_eq!(source.entity_rng, initial_rng);
        assert_eq!(source.source_byte_3e, expected_source_byte);
        assert_eq!(source.phase, expected_source_byte);
        assert_eq!(
            session
                .economy()
                .spheres()
                .iter()
                .map(|sphere| sphere.source_entity_index)
                .collect::<BTreeSet<_>>(),
            initial_sphere_slots
        );
        assert_eq!(
            session
                .economy()
                .death_bursts()
                .iter()
                .map(|burst| burst.source_entity_index)
                .collect::<BTreeSet<_>>(),
            initial_burst_slots
        );
    }

    session.step_outer_turn(PlayerCommand::default(), 0);
    let source = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.source_entity_index == root_slot)
        .expect("phase-zero Archer remains addressable until the next prologue");
    assert!(!source.drawing_enabled);
    assert_eq!(source.life, retained_life);
    assert_eq!(source.entity_rng, level2_next_rng16(initial_rng));
    assert_eq!(source.source_byte_3e, 9);
    assert_eq!(source.phase, 9);
    assert_eq!(session.economy().source_owner(root_slot), 0);
    let target = session
        .level2_archer_target(root_slot)
        .expect("hidden Archer remains addressable in the current pass");
    assert_ne!(target.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);

    let sphere_id = session
        .economy()
        .events()
        .iter()
        .find_map(|event| match event {
            Level2EconomyEvent::ManaSphereSpawned {
                sphere_id,
                source_entity_index,
                ..
            } if *source_entity_index == root_slot => Some(*sphere_id),
            _ => None,
        })
        .expect("phase-zero Archer records its own mana-sphere product");
    let sphere = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.id == sphere_id)
        .expect("phase-zero Archer produces one mana sphere");
    assert!(!initial_sphere_slots.contains(&sphere.source_entity_index));
    assert_eq!(sphere.owner_entity_index, owner_entity_index);
    assert_eq!(sphere.player_entity_index, owner_entity_index);
    let burst = session
        .economy()
        .death_bursts()
        .iter()
        .find(|burst| burst.parent_entity_index == root_slot)
        .expect("phase-zero Archer produces the ordered death burst");
    assert!(!initial_burst_slots.contains(&burst.source_entity_index));
    assert_eq!(burst.parent_entity_index, root_slot);
}

#[test]
fn level_two_model45_group22_materializes_authored_class5_rows_in_source_order() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");

    session.step_outer_turn(PlayerCommand::default(), 0);

    let sources = session.economy().authored_class5().to_vec();
    assert_eq!(
        sources
            .iter()
            .map(|source| source.authored_index)
            .collect::<Vec<_>>(),
        vec![238, 239, 240, 241, 242, 243, 245]
    );
    assert_eq!(
        sources
            .iter()
            .map(|source| source.model)
            .collect::<Vec<_>>(),
        vec![3, 9, 9, 9, 9, 9, 9]
    );
    assert_eq!(
        sources
            .iter()
            .map(|source| source.action)
            .collect::<Vec<_>>(),
        vec![25, 72, 72, 72, 72, 72, 72]
    );
    assert!(sources.iter().all(|source| source.active));
    assert!(sources.iter().all(|source| source.drawing_enabled));
    assert_eq!(sources[0].max_life, 9_000);
    assert_eq!(sources[0].mana, 2_250);
    assert_eq!(sources[0].max_mana, 4_500);
    assert_eq!(sources[1].max_life, 1_000);
    assert_eq!(sources[1].mana, 500);
    assert_eq!(sources[1].max_mana, 0);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated {
            group: 22,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [238, 239, 240, 241, 242, 243, 245]
    )));

    let row245_before = sources
        .iter()
        .find(|source| source.authored_index == 245)
        .copied()
        .expect("model-9 row 245 materialized");
    // The same outer turn that allocates group 22 also reaches the dynamic
    // arena once, so the captured production row has consumed callback one.
    assert_eq!(row245_before.dword_0x10, 15);

    // `sub_20370` owns action 72 and only decrements the countdown. The
    // zero test uses the pre-decrement value, so the remaining fifteen
    // callbacks stay stationary and the following callback changes to action
    // 73 without moving.
    for _ in 0..15 {
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    let row245_after_countdown = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 245)
        .copied()
        .expect("model-9 row 245 remains live after countdown callbacks");
    assert_eq!(row245_after_countdown.action, 72);
    assert_eq!(row245_after_countdown.dword_0x10, 0);
    assert_eq!(row245_after_countdown.position, row245_before.position);
    assert_eq!(
        row245_after_countdown.phase,
        row245_before.phase.wrapping_add(15)
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    let row245_after_transition = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 245)
        .copied()
        .expect("model-9 row 245 remains live after action-73 transition");
    assert_eq!(row245_after_transition.action, 73);
    assert_eq!(row245_after_transition.dword_0x10, 400);
    assert_eq!(row245_after_transition.particle_index, 201);
    assert_eq!(row245_after_transition.position, row245_before.position);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::DynamicArena {
            source_slot,
            kind: Level2DynamicArenaKind::AuthoredClass5,
            ..
        } if *source_slot == row245_after_transition.source_entity_index
    )));

    // Action 73 owns the movement/collision body. The qualified witness
    // crosses a tile boundary after seven action-73 callbacks, distinguishing
    // its accepted candidate from both the stationary countdown and a
    // phase-only placeholder.
    for _ in 0..7 {
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    let mut expected_row245_after_seven = row245_before.position;
    for _ in 0..7 {
        crate::move_entity(
            &mut expected_row245_after_seven,
            row245_before.yaw,
            0,
            row245_before.actual_speed,
        );
        expected_row245_after_seven.z = session
            .canonical_terrain()
            .ground_height(expected_row245_after_seven);
    }
    let row245_after_seven_steps = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 245)
        .copied()
        .expect("model-9 row 245 remains live after free callbacks");
    assert_eq!(
        row245_after_seven_steps.position,
        expected_row245_after_seven
    );
    assert_eq!(
        row245_after_seven_steps.phase,
        row245_before.phase.wrapping_add(23)
    );
    assert_ne!(
        row245_after_seven_steps.position.y >> 8,
        row245_before.position.y >> 8,
        "free branch must commit a cross-tile model-9 movement"
    );

    // Row 238 is the model-3/action-25 root.  Exercise the real production
    // target projection at the source cadence rather than the obsolete
    // action-25 target-loss wrapper.
    let row238_before = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 238)
        .copied()
        .expect("model-3 row 238 remains live after model-9 callbacks");
    let row238_ground = session
        .canonical_terrain()
        .ground_height(row238_before.position);
    let player_entity = session.state.entity_index;
    for entity in &mut session.economy.entities {
        if entity.class == 3 {
            entity.control_flags |= MODEL3_RAW_SOURCE_DISABLED_BIT;
        }
    }
    session
        .set_level2_entity_position(player_entity, row238_before.position)
        .expect("production player target position");
    session
        .economy
        .entities
        .get_mut(usize::from(player_entity))
        .expect("production player target")
        .control_flags &= !MODEL3_RAW_SOURCE_DISABLED_BIT;
    let mut economy_view = session.economy_mut();
    let row238_mut = economy_view
        .authored_class5
        .iter_mut()
        .find(|source| source.authored_index == 238)
        .expect("model-3 row 238 remains mutable for terrain fragment");
    row238_mut.action = 25;
    row238_mut.source_byte_3e = 0;
    row238_mut.source_byte_39 = 1;
    row238_mut.actual_speed = 0;
    row238_mut.yaw = 0;
    row238_mut.word_0x96_150 = 0;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row238_after = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 238)
        .copied()
        .expect("model-3 row 238 remains live after terrain-resnap fragment");
    assert_eq!(row238_after.phase, row238_before.phase.wrapping_add(1));
    assert_eq!(row238_after.action, 26);
    assert_eq!(
        (row238_after.position.x, row238_after.position.y),
        (row238_before.position.x, row238_before.position.y)
    );
    assert_eq!(row238_after.position.z, row238_ground);
    assert_eq!(row238_after.word_0x96_150, player_entity);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::DynamicArena {
            source_slot,
            kind: Level2DynamicArenaKind::AuthoredClass5,
            ..
        } if *source_slot == row238_after.source_entity_index
    )));

    // A qualified class-3 target that moves out of range is rejected by the
    // source acquisition predicate, while the cadence still consumes the
    // two LCG draws.  The old wrapper incorrectly waited for action 217 and
    // cleared a stale relation here.
    for entity in &mut session.economy.entities {
        if entity.class == 3 {
            entity.control_flags |= MODEL3_RAW_SOURCE_DISABLED_BIT;
        }
    }
    session
        .set_level2_entity_position(
            player_entity,
            Position {
                x: row238_after.position.x.wrapping_add(0x2000),
                y: row238_after.position.y,
                z: row238_ground,
            },
        )
        .expect("production out-of-range target position");
    let mut economy_view = session.economy_mut();
    let row238_mut = economy_view
        .authored_class5
        .iter_mut()
        .find(|source| source.authored_index == 238)
        .expect("model-3 row 238 remains mutable for source rejection");
    row238_mut.action = 25;
    row238_mut.source_byte_3e = 0;
    row238_mut.source_byte_39 = 1;
    row238_mut.actual_speed = 0;
    row238_mut.yaw = 0;
    row238_mut.entity_rng = 0x1357;
    row238_mut.roll = 0;
    row238_mut.word_0x96_150 = 0;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row238_after_rejection = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 238)
        .copied()
        .expect("model-3 row 238 remains live after source rejection");
    assert_ne!(row238_after_rejection.action, 26);
    assert_eq!(row238_after_rejection.word_0x96_150, 0);
    assert_eq!(
        row238_after_rejection.entity_rng,
        level2_next_rng16(level2_next_rng16(0x1357))
    );

    session.reset();
    assert!(session.economy().authored_class5().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_non_bee_creature_rng_draws_follow_pinned_source_order() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 non-Bee creature session");
    let baseline_campaign_hash = session.campaign_state_hash();
    let baseline_goat_hash = session.goat_hash();
    let baseline_ordinary_hash = session.ordinary().reset_hash();

    // `sub_1F470` is the only reset action-11 path.  Its null partner branch
    // returns before the mailbox/movement body and then reaches the one-draw
    // wrapper tail (RNG-L2-0034/0035).
    let action_11_row_39 = update_level_two_goat(&mut session, 39, |goat| {
        goat.action = 11;
        goat.partner_entity_index = None;
        goat.target_entity_index = None;
        goat.pending_damage = 0;
        goat.pending_attacker = 0;
        goat.current_attacker = 0;
        goat.rng = 0x1357;
        goat.phase = 0;
    });
    let action_11_row_71 = update_level_two_goat(&mut session, 71, |goat| {
        goat.action = 11;
        goat.partner_entity_index = None;
        goat.target_entity_index = None;
        goat.pending_damage = 0;
        goat.pending_attacker = 0;
        goat.current_attacker = 0;
        goat.rng = 0x2468;
        goat.phase = 0;
    });

    // Keep action 9 off its separate perception cadence so this receipt
    // isolates the `sub_1F3C0` wrapper tail (RNG-L2-0032/0033).
    let action_9_row_40 = update_level_two_goat(&mut session, 40, |goat| {
        goat.action = 9;
        goat.partner_entity_index = None;
        goat.target_entity_index = None;
        goat.pending_damage = 0;
        goat.pending_attacker = 0;
        goat.current_attacker = 0;
        goat.actual_speed = 0;
        goat.rng = 0x3579;
        goat.phase = 1;
        goat.proximity_countdown = 0;
        goat.proximity_subcounter = 0;
    });

    // Action 10 enters `sub_1F440`, which forces action 14 and immediately
    // calls `HitGoat_1F530`; with no target it transitions to action 9 only
    // after the distinct modulo-43 one-draw tail (RNG-L2-0036/0037).
    let action_10_row_42 = update_level_two_goat(&mut session, 42, |goat| {
        goat.action = 10;
        goat.partner_entity_index = None;
        goat.target_entity_index = None;
        goat.pending_damage = 0;
        goat.pending_attacker = 0;
        goat.current_attacker = 0;
        goat.actual_speed = 0;
        goat.rng = 0x468a;
        goat.phase = 1;
    });

    // Row 78 is a qualified reset action-105 source with no target.  Advance
    // its ordinary source phase directly to the next cadence boundary without
    // stepping the enclosing cursor; the following real outer pass then
    // observes the two source draws (RNG-L2-0094..0097) alongside the Goat
    // rows in one source-ordered production pass.
    for _ in 0..38 {
        session.ordinary_mut().step_villager_action_105(78);
    }
    let villager_before = session
        .ordinary()
        .villagers()
        .iter()
        .find(|villager| villager.entity_index == 78)
        .cloned()
        .expect("qualified Level 2 action-105 Villager");
    assert_eq!(villager_before.action, 105);
    assert_eq!(villager_before.target_entity_index, None);
    assert_eq!(villager_before.phase % 40, 0);

    session.step_outer_turn(PlayerCommand::default(), 0);

    let villager_after = session
        .ordinary()
        .villagers()
        .iter()
        .find(|villager| villager.entity_index == 78)
        .expect("action-105 Villager after source draw");
    let villager_first = level2_next_rng16(villager_before.rng);
    let villager_second = level2_next_rng16(villager_first);
    let villager_direction = 2 * (i32::from(villager_first % 157) / 79) - 1;
    let villager_magnitude = i32::from(villager_second & 0xff) + 85;
    let villager_roll = (i32::from(villager_before.roll)
        .wrapping_add(villager_direction * villager_magnitude)
        & 0x07ff) as i16;
    assert_eq!(villager_after.rng, villager_second);
    assert_eq!(villager_after.roll, villager_roll);
    assert_eq!(villager_after.phase, villager_before.phase.wrapping_add(1));

    for (before, expected_rng) in [
        (&action_11_row_39, level2_next_rng16(action_11_row_39.rng)),
        (&action_9_row_40, level2_next_rng16(action_9_row_40.rng)),
        (&action_10_row_42, level2_next_rng16(action_10_row_42.rng)),
        (&action_11_row_71, level2_next_rng16(action_11_row_71.rng)),
    ] {
        let after = session
            .goats()
            .iter()
            .find(|goat| goat.entity_index == before.entity_index)
            .expect("Goat remains live after qualified callback");
        assert_eq!(after.rng, expected_rng, "Goat {} RNG", before.entity_index);
        assert_eq!(after.phase, before.phase.wrapping_add(1));
    }
    assert_eq!(
        session
            .goats()
            .iter()
            .find(|goat| goat.entity_index == 39)
            .expect("action-11 row 39")
            .action,
        9
    );
    assert_eq!(
        session
            .goats()
            .iter()
            .find(|goat| goat.entity_index == 71)
            .expect("action-11 row 71")
            .action,
        9
    );
    assert_eq!(
        session
            .goats()
            .iter()
            .find(|goat| goat.entity_index == 42)
            .expect("action-10 row 42")
            .action,
        9
    );

    // The outer source cursor is globally ascending.  These four rows are
    // intentionally separated so the receipt observes both action-11 rows
    // and the action-9/action-10 wrappers in their source-slot order.
    let selected_source_dispatch = session
        .dispatch_events()
        .iter()
        .filter_map(|event| match event {
            Level2DispatchEvent::Goat { source_slot, .. }
                if matches!(*source_slot, 39 | 40 | 42 | 71) =>
            {
                Some(*source_slot)
            }
            Level2DispatchEvent::Villager {
                source_slot: 78,
                pass: 0,
                ..
            } => Some(78),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(selected_source_dispatch, [39, 40, 42, 71, 78]);

    // Reset restores every authoritative carrier, including the one RNG per
    // Goat/Villager row, the dispatch journal, and the hashes consumed by the
    // production route.
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);
    assert_eq!(session.goat_hash(), baseline_goat_hash);
    assert_eq!(session.ordinary().reset_hash(), baseline_ordinary_hash);
    assert!(session.dispatch_events().is_empty());
}

#[test]
fn level_two_non_bee_creature_rng_branches_reject_without_forbidden_draws() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");

    // A stale/absent source slot is a no-op in the live class-5 cursor.  The
    // wrong action is also fail-closed before any Goat callback body.
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 negative-branch session");
    let absent_hash = session.goat_hash();
    let terrain = session.canonical_terrain().clone();
    assert!(
        session
            .goats
            .step_source_slot(999, &terrain, &[])
            .is_empty()
    );
    assert_eq!(session.goat_hash(), absent_hash);

    let wrong_action = update_level_two_goat(&mut session, 40, |goat| {
        goat.action = 8;
        goat.rng = 0x1234;
        goat.phase = 7;
    });
    let wrong_hash = session.goat_hash();
    let wrong_events = session.goats.step_source_slot(40, &terrain, &[]);
    let wrong_after = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 40)
        .expect("wrong-action Goat remains present");
    assert!(wrong_events.is_empty());
    assert_eq!(wrong_after, &wrong_action);
    assert_eq!(session.goat_hash(), wrong_hash);

    // A patrol row with the wrong stage subtype is rejected before the
    // class-5 callback tail; neither its RNG nor its cursor phase may move.
    let wrong_subtype = update_level_two_goat(&mut session, 44, |goat| {
        goat.action = GOAT_PATROL_ACTION;
        goat.stage_var_2 = 1;
        goat.rng = 0x1a2b;
        goat.phase = 9;
    });
    let wrong_subtype_hash = session.goat_hash();
    let wrong_subtype_events = session.goats.step_source_slot(44, &terrain, &[]);
    let wrong_subtype_after = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 44)
        .expect("wrong-subtype Goat remains present");
    assert!(wrong_subtype_events.is_empty());
    assert_eq!(wrong_subtype_after, &wrong_subtype);
    assert_eq!(session.goat_hash(), wrong_subtype_hash);

    // Action 13 is terminal but phase-gated.  At a nonzero phase it advances
    // only the source cursor; it cannot consume the mana-conversion draw.
    let terminal = update_level_two_goat(&mut session, 41, |goat| {
        goat.action = 13;
        goat.phase = 1;
        goat.mana = 0;
        goat.rng = 0x2345;
    });
    let terminal_events = session.goats.step_source_slot(41, &terrain, &[]);
    let terminal_after = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 41)
        .expect("terminal Goat remains present");
    assert!(terminal_events.is_empty());
    assert_eq!(terminal_after.rng, terminal.rng);
    assert_eq!(terminal_after.phase, terminal.phase.wrapping_add(1));

    // UpdateEntities reclaims a drawing-disabled row before the source
    // callback.  The reclaim event is allowed; the forbidden creature draw
    // and callback phase are not.
    let disabled = update_level_two_goat(&mut session, 43, |goat| {
        goat.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        goat.rng = 0x3456;
    });
    let begin_events = session.goats.begin_source_turn(&[]);
    assert!(
        begin_events
            .iter()
            .any(|event| matches!(event, GoatStepEvent::Reclaimed { entity_index: 43 }))
    );
    let disabled_after = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 43)
        .expect("reclaimed Goat remains in the reset projection");
    assert!(!disabled_after.active);
    assert_eq!(disabled_after.rng, disabled.rng);
    assert!(session.goats.step_source_slot(43, &terrain, &[]).is_empty());

    // Wrong class/model allocation is rejected before the class-5 arena or
    // its sidecar/RNG map can change; an absent economy source is the same
    // fail-closed boundary used by the real dynamic cursor.
    let economy_before = session.economy().clone();
    let economy_hash = session.economy().reset_hash();
    for wrong_model in [2, 13] {
        assert!(
            session
                .economy_mut()
                .allocate_authored_class5(900, wrong_model, Position::default())
                .is_none()
        );
    }
    assert!(!session.economy_mut().step_authored_class5_slot(999));
    assert_eq!(session.economy(), &economy_before);
    assert_eq!(session.economy().reset_hash(), economy_hash);

    // A zero attacker/owner sentinel is rejected at the mailbox boundary and
    // cannot create a partial source mutation or draw.
    let wrong_owner_goat_hash = session.goat_hash();
    assert!(session.submit_goat_damage(40, 1, 0).is_err());
    assert_eq!(session.goat_hash(), wrong_owner_goat_hash);

    // A Villager mailbox hit and the following terminal transition mutate
    // only their source action/mailbox/phase; the action-105 RNG tail is not
    // entered on either path.
    let mut nonlethal = inspection
        .level2_reset_session()
        .expect("Level 2 Villager negative session");
    let villager_before = nonlethal
        .ordinary()
        .villagers()
        .iter()
        .find(|villager| villager.entity_index == 77)
        .cloned()
        .expect("qualified Villager row");
    nonlethal
        .ordinary_mut()
        .inject_villager_damage(77, 1, 152)
        .expect("nonlethal Villager mailbox");
    nonlethal.ordinary_mut().step_villager_action_105(77);
    let villager_after_hit = nonlethal
        .ordinary()
        .villagers()
        .iter()
        .find(|villager| villager.entity_index == 77)
        .expect("Villager after nonlethal mailbox");
    assert_eq!(villager_after_hit.rng, villager_before.rng);
    assert_eq!(villager_after_hit.action, VILLAGER_HIT_ACTION);

    let mut lethal = inspection
        .level2_reset_session()
        .expect("Level 2 lethal Villager session");
    let lethal_before = lethal
        .ordinary()
        .villagers()
        .iter()
        .find(|villager| villager.entity_index == 77)
        .cloned()
        .expect("qualified lethal Villager row");
    lethal
        .ordinary_mut()
        .inject_villager_damage(77, lethal_before.life + 1, 152)
        .expect("lethal Villager mailbox");
    lethal.ordinary_mut().step_villager_action_105(77);
    let after_death = lethal
        .ordinary()
        .villagers()
        .iter()
        .find(|villager| villager.entity_index == 77)
        .expect("Villager after lethal mailbox");
    assert_eq!(after_death.rng, lethal_before.rng);
    assert_eq!(after_death.action, VILLAGER_DEATH_ACTION);
    lethal.ordinary_mut().step_villager_action_105(77);
    let after_terminal = lethal
        .ordinary()
        .villagers()
        .iter()
        .find(|villager| villager.entity_index == 77)
        .expect("Villager after death transition");
    assert_eq!(after_terminal.rng, lethal_before.rng);
    assert_eq!(after_terminal.action, VILLAGER_DEATH_ACTION);
    assert!(!after_terminal.active);

    // Once a valid house relation exists, the action-105 target branch is
    // source-defined and draw-free.  Run the preceding phase slots without
    // changing the target, then cross the exact cadence boundary.
    let mut target_branch = inspection
        .level2_reset_session()
        .expect("Level 2 Villager target-branch session");
    target_branch.ordinary_mut().step_villager_action_105(77);
    let after_acquire = target_branch
        .ordinary()
        .villagers()
        .iter()
        .find(|villager| villager.entity_index == 77)
        .cloned()
        .expect("Villager after target acquisition");
    let target = after_acquire
        .target_entity_index
        .expect("qualified action-105 branch acquires a house target");
    let target_rng = after_acquire.rng;
    for _ in 0..38 {
        target_branch.ordinary_mut().step_villager_action_105(77);
    }
    let at_target_cadence = target_branch
        .ordinary()
        .villagers()
        .iter()
        .find(|villager| villager.entity_index == 77)
        .expect("Villager at target cadence");
    assert_eq!(at_target_cadence.phase % 40, 0);
    assert_eq!(at_target_cadence.target_entity_index, Some(target));
    target_branch.ordinary_mut().step_villager_action_105(77);
    let after_target_branch = target_branch
        .ordinary()
        .villagers()
        .iter()
        .find(|villager| villager.entity_index == 77)
        .expect("Villager after target branch");
    assert_eq!(after_target_branch.rng, target_rng);

    // A Level 1/Night package is an unsupported profile for this Day route;
    // profile rejection happens before a mutable creature session exists.
    let level_one_package =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated/level1-export-m");
    assert!(NativeWorld::inspect_level2(level_one_package).is_err());
}

#[test]
fn level_two_bee_damage_mailbox_keeps_payload_and_orders_nonlethal_tail() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2");
    let mut campaign = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::LEVEL2,
    )
    .expect("qualified semantic Level 2 campaign");
    let session = campaign.level2_compatibility_mut();
    session
        .activate_authored_group_prerequisite_for_test(21)
        .expect("qualified group-21 Bee boundary");
    let bee = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.authored_index() == Some(229))
        .copied()
        .expect("qualified authored Day Bee");
    let mut source = bee;
    let initial_rng = source.entity_rng;
    assert_eq!(
        source.action, 23,
        "qualified Bee retains its constructor action"
    );
    source.pending_damage = 250;
    source.pending_attacker_entity_index = session.state.entity_index;

    assert_eq!(
        session
            .economy_mut()
            .consume_authored_bee_damage_mailbox(&mut source),
        Some(false)
    );
    assert_eq!(source.life, bee.life - 250);
    assert_eq!(source.pending_damage, 250);
    assert_eq!(source.pending_attacker_entity_index, 0);
    assert_eq!(
        source.damage_attacker_entity_index,
        session.state.entity_index
    );
    assert_eq!(source.action, 18);
    assert_eq!(source.target_entity_index, Some(session.state.entity_index));
    assert_eq!(source.entity_rng, level2_next_rng16(initial_rng));
    assert!(matches!(
        session.economy().events().last(),
        Some(Level2EconomyEvent::DamageApplied {
            target_entity_index,
            attacker_entity_index,
            damage: 250,
            lethal: false,
        }) if *target_entity_index == source.entity_id
            && *attacker_entity_index == session.state.entity_index
    ));
}

#[test]
fn level_two_bee_lethal_mailbox_skips_rng_and_enters_action20() {
    let mut session = level_two_campaign_session();
    let bee = session
        .allocate_level2_authored_bee(229)
        .expect("qualified authored Day Bee");
    let mut source = bee;
    source.action = 17;
    source.pending_damage = i32::from(source.life) + 1;
    source.pending_attacker_entity_index = session.state.entity_index;

    assert_eq!(
        session
            .economy_mut()
            .consume_authored_bee_damage_mailbox(&mut source),
        Some(true)
    );
    assert_eq!(source.life, -1);
    assert_eq!(source.action, 20);
    assert_eq!(source.entity_rng, bee.entity_rng);
    assert_eq!(source.pending_damage, i32::from(bee.life) + 1);
    assert_eq!(source.pending_attacker_entity_index, 0);
    assert!(matches!(
        session.economy().events().last(),
        Some(Level2EconomyEvent::DamageApplied { lethal: true, .. })
    ));
}

#[test]
fn level_two_bee_death_tail_is_phase_gated_and_orders_outputs() {
    let mut session = level_two_campaign_session();
    let bee = session
        .allocate_level2_authored_bee(229)
        .expect("qualified authored Day Bee");
    let initial_rng = bee.entity_rng;
    let free_slots = session.economy().free_slot_count();
    let mut gated = bee;
    gated.action = 21;
    gated.source_byte_3e = 1;
    session
        .economy_mut()
        .step_authored_bee_death_tail(&mut gated);
    assert!(gated.active);
    assert!(gated.drawing_enabled);
    assert_eq!(gated.life, bee.life);
    assert_eq!(gated.entity_rng, initial_rng);
    assert_eq!(session.economy().free_slot_count(), free_slots);
    assert!(session.economy().spheres().is_empty());
    assert!(session.economy().death_bursts().is_empty());

    gated.source_byte_3e = 8;
    session
        .economy_mut()
        .step_authored_bee_death_tail(&mut gated);
    assert!(
        gated.active,
        "KillEntity only hides; the following pass reclaims"
    );
    assert!(!gated.drawing_enabled);
    assert_eq!(gated.life, bee.life, "KillEntity does not rewrite life");
    assert_eq!(gated.action, 21);
    assert_eq!(gated.entity_rng, level2_next_rng16(initial_rng));
    assert!(session.economy().events().iter().any(|event| matches!(
        event,
        Level2EconomyEvent::ManaSphereSpawned {
            source_entity_index,
            mana: 1500,
            ..
        } if *source_entity_index == gated.source_entity_index
    )));
    assert!(session.economy().events().iter().any(|event| matches!(
        event,
        Level2EconomyEvent::DeathBurstSpawned {
            parent_entity_index,
            id_entity_index,
            ..
        } if *parent_entity_index == gated.source_entity_index
            && *id_entity_index == gated.entity_id
    )));
}

#[test]
fn level_two_bee_death_tail_allocation_failure_still_closes_without_partial_output() {
    let mut session = level_two_campaign_session();
    let bee = session
        .allocate_level2_authored_bee(229)
        .expect("qualified authored Day Bee");
    let mut exhausted = session.economy().clone();
    exhausted.free_slots.clear();
    for row in &mut exhausted.castle_children {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.spheres {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.death_bursts {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.authored_model1 {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.model59_smoke_children {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.authored_model5_water_splashes {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.authored_model13 {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.authored_model6 {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.model13_spawned_children {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.model17_spawned_children {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for row in &mut exhausted.model76_tail_spawns {
        row.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    let events_before = exhausted.events.len();
    let spheres_before = exhausted.spheres.len();
    let death_bursts_before = exhausted.death_bursts.len();
    let initial_rng = bee.entity_rng;
    let source_slot = bee.source_entity_index;
    let mut gated = bee;
    gated.action = 21;
    gated.source_byte_3e = 8;
    let terrain = session.canonical_terrain().clone();
    exhausted.step_authored_bee_death_tail(&mut gated, &terrain);

    assert!(
        gated.active,
        "allocation failure still preserves the retained source slot"
    );
    assert!(!gated.drawing_enabled);
    assert_eq!(gated.life, bee.life);
    let death_rng = level2_next_rng16(initial_rng);
    assert_eq!(gated.entity_rng, death_rng);
    assert_eq!(exhausted.events.len(), events_before);
    assert_eq!(exhausted.spheres.len(), spheres_before);
    assert_eq!(exhausted.death_bursts.len(), death_bursts_before);
    assert_eq!(exhausted.source_rng.get(&source_slot), Some(&death_rng));
    assert_eq!(exhausted.source_owner.get(&source_slot), Some(&0));
}

#[test]
fn level_two_three_reachable_creature_sounds_emit_in_pinned_source_order() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 creature audio session");
    let baseline_campaign_hash = session.campaign_state_hash();
    let baseline_goat_hash = session.goat_hash();

    configure_three_level_two_creature_sounds(&mut session);
    session.step_outer_turn(PlayerCommand::default(), 0);

    let receipts = creature_audio_receipts(&session);
    assert_eq!(receipts.len(), 3, "exactly the three selected audio calls");
    assert_eq!(
        receipts
            .iter()
            .map(|receipt| receipt.source_slot)
            .collect::<Vec<_>>(),
        [39, 40, 42],
        "UpdateEntities source order is the audio journal order"
    );
    assert_eq!(
        receipts
            .iter()
            .map(|receipt| receipt.entity_index)
            .collect::<Vec<_>>(),
        [39, 40, 42],
        "PrepareEventSound owner is the visited Goat source row"
    );
    assert_eq!(
        receipts
            .iter()
            .map(|receipt| receipt.sound_id)
            .collect::<Vec<_>>(),
        [46, 46, 46],
        "AUD-L2-004..006 all prepare owner sound 46"
    );
    assert_eq!(
        receipts
            .iter()
            .map(|receipt| receipt.action)
            .collect::<Vec<_>>(),
        [9, 9, 14],
        "action-11 transitions before its sub_1F470 tail; HitGoat stays action 14"
    );
    assert!(
        receipts
            .iter()
            .all(|receipt| receipt.audio_site == crate::MissionAudioSite::GoatBleat)
    );
    for receipt in &receipts {
        let goat = session
            .goats()
            .iter()
            .find(|goat| goat.entity_index == receipt.source_slot)
            .expect("audio source remains in the typed Goat projection");
        assert_eq!(receipt.position, goat.position);
    }

    // Each source slot contributes one typed audio event to this pass. A
    // second ordinary visit does not replay the just-consumed modulo tail.
    assert_eq!(
        receipts
            .iter()
            .map(|receipt| receipt.source_slot)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([39, 40, 42])
    );
    let first_audio_hash = session.campaign_state_hash();
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        creature_audio_receipts(&session).is_empty(),
        "a duplicate source visit cannot duplicate the prior audio journal"
    );

    // Reset clears the ordered journal and restores the exact state carriers;
    // replaying the same ordinary route reproduces the three receipts/hash.
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);
    assert_eq!(session.goat_hash(), baseline_goat_hash);
    assert!(session.dispatch_events().is_empty());

    configure_three_level_two_creature_sounds(&mut session);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(creature_audio_receipts(&session), receipts);
    assert_eq!(session.campaign_state_hash(), first_audio_hash);
}

#[test]
fn level_two_creature_audio_negative_branches_emit_nothing_and_preserve_state() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 creature audio negative session");

    // Stale, wrong-action, and wrong-patrol-subtype visits fail closed before
    // the three source audio tails and leave their typed journal/hash alone.
    let stale_hash = session.goat_hash();
    let terrain = session.canonical_terrain().clone();
    assert_no_goat_bleat(&session.goats.step_source_slot(999, &terrain, &[]));
    assert_eq!(session.goat_hash(), stale_hash);

    let wrong_action = update_level_two_goat(&mut session, 40, |goat| {
        goat.action = 8;
        goat.rng = 129;
        goat.phase = 1;
    });
    let wrong_action_hash = session.goat_hash();
    let wrong_action_events = session.goats.step_source_slot(40, &terrain, &[]);
    assert_no_goat_bleat(&wrong_action_events);
    assert_eq!(
        session
            .goats()
            .iter()
            .find(|goat| goat.entity_index == 40)
            .expect("wrong-action Goat remains present"),
        &wrong_action
    );
    assert_eq!(session.goat_hash(), wrong_action_hash);

    let wrong_subtype = update_level_two_goat(&mut session, 44, |goat| {
        goat.action = GOAT_PATROL_ACTION;
        goat.stage_var_2 = 1;
        goat.rng = 129;
        goat.phase = 1;
    });
    let wrong_subtype_hash = session.goat_hash();
    let wrong_subtype_events = session.goats.step_source_slot(44, &terrain, &[]);
    assert_no_goat_bleat(&wrong_subtype_events);
    assert_eq!(
        session
            .goats()
            .iter()
            .find(|goat| goat.entity_index == 44)
            .expect("wrong-subtype Goat remains present"),
        &wrong_subtype
    );
    assert_eq!(session.goat_hash(), wrong_subtype_hash);

    // A dead row and a following-pass reclaimed row cannot emit after their
    // lifecycle gate has rejected the ordinary callback.
    let dead = update_level_two_goat(&mut session, 45, |goat| {
        goat.active = false;
        goat.action = 14;
        goat.rng = 62;
    });
    let dead_hash = session.goat_hash();
    assert_no_goat_bleat(&session.goats.step_source_slot(45, &terrain, &[]));
    assert_eq!(session.goat_hash(), dead_hash);
    assert_eq!(
        session
            .goats()
            .iter()
            .find(|goat| goat.entity_index == 45)
            .expect("dead Goat remains in reset projection"),
        &dead
    );

    let reclaimed = update_level_two_goat(&mut session, 43, |goat| {
        goat.action = 11;
        goat.rng = 129;
        goat.control_flags |= 0x0000_0400;
    });
    let reclaim_events = session.goats.begin_source_turn(&[]);
    assert!(
        reclaim_events
            .iter()
            .any(|event| matches!(event, GoatStepEvent::Reclaimed { entity_index: 43 }))
    );
    assert_no_goat_bleat(&reclaim_events);
    let reclaimed_after = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 43)
        .expect("reclaimed Goat remains in reset projection");
    assert!(!reclaimed_after.active);
    assert_eq!(reclaimed_after.rng, reclaimed.rng);

    // Unsupported model allocation is rejected before a class-5 source
    // row exists, so it cannot manufacture a creature audio event. Model 1
    // is the supported Goat constructor, not an allocation negative.
    let economy_before = session.economy().clone();
    let economy_hash = session.economy().reset_hash();
    for wrong_model in [2, 13] {
        assert!(
            session
                .economy_mut()
                .allocate_authored_class5(900, wrong_model, Position::default())
                .is_none()
        );
    }
    assert_eq!(session.economy(), &economy_before);
    assert_eq!(session.economy().reset_hash(), economy_hash);
    assert!(creature_audio_receipts(&session).is_empty());
    let supported_goat = session
        .economy_mut()
        .allocate_authored_class5(900, crate::goat::GOAT_MODEL, Position::default())
        .expect("the package-reachable Goat model has an ordinary constructor");
    assert_eq!(
        (
            supported_goat.model,
            supported_goat.action,
            supported_goat.life,
            supported_goat.mana
        ),
        (
            crate::goat::GOAT_MODEL,
            crate::goat::GOAT_CONSTRUCTOR_ACTION,
            crate::goat::GOAT_CONSTRUCTOR_LIFE,
            crate::goat::GOAT_CONSTRUCTOR_MANA
        )
    );
    assert!(
        creature_audio_receipts(&session).is_empty(),
        "construction alone emits no Goat callback audio"
    );

    // Unsupported Level 1/Night input fails before a mutable Level 2 audio
    // session can exist.
    let level_one_package =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated/level1-export-m");
    assert!(NativeWorld::inspect_level2(level_one_package).is_err());
}
