use super::{status, transport, wind};
use crate::{
    FlightContext, FlightEffectsContext, FlightProfile, FlightState, FlightWorld, PlayerCommand,
    Position, Tile,
    level_definition::{FieldDefinition, FieldKind, TilePosition},
    native_world::EntitySnapshot,
    step_turn_with_context,
    world::terrain::cave_features::new_event,
};

struct Ground;
impl FlightWorld for Ground {
    fn ground_height(&self, _: Position) -> i16 {
        0
    }
    fn ceiling_height(&self, _: Position) -> Option<i16> {
        None
    }
    fn material_mask(&self, _: Position) -> u32 {
        0
    }
    fn angle_flags(&self, _: Tile) -> u8 {
        0
    }
}

fn player() -> (EntitySnapshot, FlightState) {
    let position = Position {
        x: 10_000,
        y: 10_000,
        z: 2_000,
    };
    let mut body = new_event(100, 77, position);
    body.class = 3;
    body.model = 0;
    body.record[88..90].copy_from_slice(&256_i16.to_le_bytes());
    body.record[92..94].copy_from_slice(&256_i16.to_le_bytes());
    (
        body,
        FlightState {
            entity_index: 100,
            position,
            actual_speed: 64,
            target_speed: 64,
            ..FlightState::default()
        },
    )
}

fn impact(kind: status::Kind) -> EntitySnapshot {
    let mut row = new_event(120, 13, Position { x: 0, y: 0, z: 0 });
    let (model, action) = kind.source_selector();
    row.class = 10;
    row.model = model;
    row.set_source_action(action);
    row.record[154..156].copy_from_slice(&100_u16.to_le_bytes());
    row
}

fn definition(kind: FieldKind) -> FieldDefinition {
    sim_definition::adapter::mc2_field_definition(kind, TilePosition { x: 0, y: 0 })
}

fn campaign() -> crate::native_world::Mc2Simulation {
    let package = std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
        },
        std::path::PathBuf::from,
    );
    crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::LEVEL2,
    )
    .unwrap()
    .simulation()
    .clone()
}

fn scheduled_impact(session: &mut crate::native_world::Mc2Simulation, kind: FieldKind) -> u16 {
    let live = &mut session.session.state.live;
    let player = live.core.state.entity_index;
    let slot = status::construct(
        &mut live.economy,
        &mut live.terrain,
        live.core.state.position,
        definition(kind),
    )
    .unwrap()
    .unwrap();
    // This is the projectile producer's real target relation, not a callback
    // invocation or a replacement player carrier.
    live.economy.entities[usize::from(slot)].record[154..156]
        .copy_from_slice(&player.to_le_bytes());
    assert!(slot > player, "pin impact after the local player visit");
    slot
}

fn campaign_step(session: &mut crate::native_world::Mc2Simulation) {
    assert!(
        !session
            .step_outer_turn(PlayerCommand::default(), 0)
            .is_empty()
    );
}

#[test]
fn skeleton_controller_schedules_owned_births_and_respects_owner_cap() {
    use super::skeleton_summon::{self, Birth};
    use crate::campaign::source_arena::CreatureRootOrigin;
    use crate::level_definition::CreatureKind;
    use crate::native_world::Mc2DispatchEvent;

    // Controlled source-constructor placement on retained Level2 terrain;
    // this exercises the numeric scheduler, not an earned mission event.
    let mut session = campaign();
    let properties = definition(FieldKind::SkeletonSummon);
    let FieldDefinition::SkeletonSummon { particle_index, .. } = properties else {
        unreachable!("source adapter returns the Skeleton controller definition");
    };
    let particle = session.session.definition.presentation.particles[usize::from(particle_index)];
    let (controller, handle, owner, position, phase, tile) = {
        let live = &mut session.session.state.live;
        let owner = live.core.state.entity_index;
        let position = live.core.state.position;
        let controller = skeleton_summon::construct(
            &mut live.economy,
            &mut live.terrain,
            position,
            properties,
            particle,
        )
        .unwrap()
        .expect("source controller allocation");
        let source = &mut live.economy.entities[usize::from(controller)];
        source.source_id = owner;
        source.record[30..32].copy_from_slice(&owner.to_le_bytes());
        assert_eq!(
            (
                source.class,
                source.model,
                source.action,
                source.life,
                source.subspell_index
            ),
            (10, 36, 38, 8, 64_000)
        );
        let phase = source.phase;
        let handle = live.economy.handle(controller).unwrap();
        let tile = live.economy.map_chains.tile_of(controller);
        (controller, handle, owner, position, phase, tile)
    };
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !session
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
        "scheduled source36 must not roll back: {:?}",
        session.dispatch_events()
    );
    let summoned = {
        let live = &session.session.state.live;
        let root = &live.economy.entities[usize::from(controller)];
        assert!(root.active && root.drawing_disabled());
        assert_eq!(root.phase, phase.wrapping_add(1));
        assert_eq!(root.subspell_index, 10_000);
        assert!(live.economy.map_chains.is_linked(controller));
        let births = session
            .creature_runtime_census()
            .roots_of_kind(CreatureKind::Skeleton)
            .filter(|identity| {
                identity.origin == CreatureRootOrigin::FieldSummon { controller: handle }
            })
            .collect::<Vec<_>>();
        assert!(
            !births.is_empty() && births.len() <= 8,
            "controller produces real Skeletons"
        );
        for birth in &births {
            let actor = &live.economy.entities[usize::from(birth.handle.source_slot)];
            assert_eq!(
                (
                    actor.class,
                    actor.model,
                    actor.source_id,
                    actor.source_player_entity_index()
                ),
                (5, 9, owner, owner)
            );
            assert_eq!(actor.max_life, 1_000);
        }
        births
            .iter()
            .map(|birth| {
                let actor = &live.economy.entities[usize::from(birth.handle.source_slot)];
                (birth.handle, actor.phase)
            })
            .collect::<Vec<_>>()
    };
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !session
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
        "controller reclaim must not roll back: {:?}",
        session.dispatch_events()
    );
    {
        let live = &session.session.state.live;
        assert!(
            !live.economy.entities[usize::from(controller)].active
                || live.economy.economy.slot_generation(controller) != handle.generation
        );
        assert_eq!(
            live.terrain.planes().entity_indices[usize::from(tile)] as u16,
            live.economy.map_chains.tile_head(tile)
        );
        assert!(
            session
                .creature_runtime_census()
                .roots_of_kind(CreatureKind::Skeleton)
                .any(|identity| identity.origin
                    == CreatureRootOrigin::FieldSummon { controller: handle }),
            "Skeletons do not inherit the controller's lifetime"
        );
        assert!(
            summoned.iter().any(|(handle, phase)| {
                let actor = &live.economy.entities[usize::from(handle.source_slot)];
                live.economy.economy.slot_generation(handle.source_slot) == handle.generation
                    && actor.active
                    && actor.phase == phase.wrapping_add(1)
            }),
            "a summoned Skeleton reaches its own scheduled actor callback"
        );
    }

    // The near-identical case populates the source-frozen owner list using
    // the real constructor. At the source cap, the same controller hides
    // without adding another owned Skeleton.
    let mut capped = campaign();
    let particles = capped.session.definition.presentation.particles.clone();
    let descriptor = capped.session.definition.presentation.entity_parameters[80];
    let (controller, handle, owner) = {
        let live = &mut capped.session.state.live;
        let owner = live.core.state.entity_index;
        let controller = skeleton_summon::construct(
            &mut live.economy,
            &mut live.terrain,
            position,
            properties,
            particle,
        )
        .unwrap()
        .expect("source controller allocation");
        let source = &mut live.economy.entities[usize::from(controller)];
        source.source_id = owner;
        source.record[30..32].copy_from_slice(&owner.to_le_bytes());
        let handle = live.economy.handle(controller).unwrap();
        for _ in 0..64 {
            assert!(
                crate::native_world::creatures::skeleton::allocate_from_controller(
                    &mut live.economy,
                    &mut live.terrain,
                    &particles,
                    &descriptor,
                    Birth {
                        position,
                        controller_slot: controller,
                        owner,
                        mana: 0,
                        yaw: 0
                    },
                )
                .unwrap(),
                "real retained terrain admits a Skeleton constructor"
            );
        }
        (controller, handle, owner)
    };
    let before = capped
        .creature_runtime_census()
        .roots_of_kind(CreatureKind::Skeleton)
        .filter(|identity| {
            identity.origin == CreatureRootOrigin::FieldSummon { controller: handle }
        })
        .count();
    assert_eq!(before, 64);
    capped.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !capped
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
        "capped controller must not roll back: {:?}",
        capped.dispatch_events()
    );
    let live = &capped.session.state.live;
    assert!(live.economy.entities[usize::from(controller)].drawing_disabled());
    let after = capped
        .creature_runtime_census()
        .roots_of_kind(CreatureKind::Skeleton)
        .filter(|identity| {
            identity.origin == CreatureRootOrigin::FieldSummon { controller: handle }
        })
        .collect::<Vec<_>>();
    assert_eq!(after.len(), before, "owner cap prevents all new births");
    assert!(after.iter().all(|identity| {
        live.economy.entities[usize::from(identity.handle.source_slot)].source_player_entity_index()
            == owner
    }));

    // A frozen model-9 list stores addresses, not lifetime handles. If an
    // address is reused, the owner-cap scan reads the current occupant's
    // word94, even when the retained raw record still contains an old value.
    use crate::native_world::creatures::skeleton::current_source_player_entity_index;
    let raw_slot = after[0].handle.source_slot;
    assert_eq!(
        current_source_player_entity_index(&live.economy, raw_slot).unwrap(),
        Some(owner)
    );
    let live = &mut capped.session.state.live;
    let stale_owner = owner.wrapping_add(1);
    let sphere = live
        .economy
        .economy
        .spawn_mana_sphere_from_death(999, position, 0, 0x1234, 500, owner, &live.terrain)
        .expect("death constructor allocates a typed sphere") as u16;
    live.economy.entities[usize::from(sphere)].record[152..154]
        .copy_from_slice(&stale_owner.to_le_bytes());
    assert_eq!(
        current_source_player_entity_index(&live.economy, sphere).unwrap(),
        Some(owner),
        "current sphere ownership wins over stale raw bytes"
    );
    let creature = live
        .economy
        .economy
        .allocate_authored_class5(999, 9, position, &live.terrain)
        .expect("ordinary constructor allocates a typed Skeleton");
    let creature_slot = creature.source_entity_index;
    let typed_owner = live.economy.economy.source_owner(creature_slot);
    live.economy.entities[usize::from(creature_slot)].record[152..154]
        .copy_from_slice(&typed_owner.wrapping_add(1).to_le_bytes());
    assert_eq!(
        current_source_player_entity_index(&live.economy, creature_slot).unwrap(),
        Some(typed_owner),
        "current typed creature ownership wins over stale raw bytes"
    );
}

#[test]
fn wind_experience_reaches_the_owner_and_respects_source_gates() {
    let mut session = campaign();
    let live = &mut session.session.state.live;
    let owner_slot = live.core.state.entity_index;
    let owner = live.economy.entities[usize::from(owner_slot)].clone();
    let before = live.shared_spells.spell_experience()[21];
    wind::award_experience(&mut live.shared_spells, Some(&owner), 3);
    assert_eq!(live.shared_spells.spell_experience()[21], before + 3);

    live.shared_spells
        .set_mission_rewards(crate::campaign::rewards::MissionRewards::Suppressed);
    wind::award_experience(&mut live.shared_spells, Some(&owner), 3);
    live.shared_spells
        .set_mission_rewards(crate::campaign::rewards::MissionRewards::Enabled);
    let mut nonhuman = owner.clone();
    nonhuman.model = 1;
    wind::award_experience(&mut live.shared_spells, Some(&nonhuman), 3);
    let mut freed = owner.clone();
    freed.reclaim_source_row();
    wind::award_experience(&mut live.shared_spells, Some(&freed), 3);
    wind::award_experience(&mut live.shared_spells, None, 3);
    assert_eq!(live.shared_spells.spell_experience()[21], before + 3);
}

#[test]
fn wind_projection_cleanup_does_not_retire_a_reused_generation() {
    let mut session = campaign();
    let particles = &session.session.definition.presentation.particles;
    let live = &mut session.session.state.live;
    let slot = super::construct_spell_wind(
        &mut live.economy,
        &mut live.terrain,
        particles,
        live.core.state.position,
    )
    .unwrap()
    .unwrap();
    let old_generation = live.economy.economy.slot_generation(slot);
    let wind = live
        .economy
        .economy
        .fields
        .authored_model22
        .iter_mut()
        .find(|wind| wind.source_entity_index == slot)
        .unwrap();
    wind.control_flags |= 0x400;
    live.economy.economy.claim_raw_replacement_slot(slot);
    assert_ne!(live.economy.economy.slot_generation(slot), old_generation);
    assert!(live.economy.entities[usize::from(slot)].active);
    super::retire_hidden_wind_projections(&mut live.economy, &mut live.terrain).unwrap();
    assert!(
        live.economy.entities[usize::from(slot)].active,
        "the old field must not retire the new occupant at the same address"
    );
}

#[test]
fn hidden_wind_waits_for_source_rebuild_not_bare_allocation() {
    let mut session = campaign();
    let particles = &session.session.definition.presentation.particles;
    let live = &mut session.session.state.live;
    let root = super::construct_spell_wind(
        &mut live.economy,
        &mut live.terrain,
        particles,
        live.core.state.position,
    )
    .unwrap()
    .unwrap();
    let children: Vec<_> = live
        .economy
        .economy
        .fields
        .model22_spawned_children
        .iter()
        .filter(|child| child.parent_entity_index == root)
        .map(|child| child.source_entity_index)
        .collect();
    assert_eq!(children.len(), 11);
    live.economy
        .economy
        .fields
        .authored_model22
        .iter_mut()
        .find(|wind| wind.source_entity_index == root)
        .unwrap()
        .life = 0;
    assert!(
        live.economy
            .economy
            .step_authored_model22_slot(root, &live.terrain)
    );
    let unrelated = live
        .economy
        .allocate_source_slot_with_terrain(&mut live.terrain)
        .unwrap();
    assert_ne!(unrelated, root);
    assert!(!children.contains(&unrelated));
    assert!(live.economy.map_chains.is_linked(root));
    assert!(
        children
            .iter()
            .all(|slot| live.economy.map_chains.is_linked(*slot))
    );
    let tiles: Vec<_> = std::iter::once(root)
        .chain(children.iter().copied())
        .map(|slot| live.economy.map_chains.tile_of(slot))
        .collect();

    // sub_4A1E0 explicitly calls sub_49F90, unlike ordinary NewEvent.
    live.economy
        .rebuild_source_arena_with_terrain(&mut live.terrain, &[])
        .unwrap();
    assert!(!live.economy.entities[usize::from(root)].active);
    assert!(!live.economy.map_chains.is_linked(root));
    for slot in children {
        assert!(!live.economy.entities[usize::from(slot)].active);
        assert!(!live.economy.map_chains.is_linked(slot));
    }
    for tile in tiles {
        assert_eq!(
            live.terrain.planes().entity_indices[usize::from(tile)] as u16,
            live.economy.map_chains.tile_head(tile),
            "source rebuild publishes the reclaimed tile head"
        );
    }
}

#[test]
fn authored_wind_rebuild_publishes_typed_chain_reclaim() {
    let mut session = campaign();
    let root = session
        .allocate_level2_authored_model22(110)
        .unwrap()
        .source_entity_index;
    let live = &mut session.session.state.live;
    let slots: Vec<_> = std::iter::once(root)
        .chain(
            live.economy
                .economy
                .fields
                .model22_spawned_children
                .iter()
                .filter(|child| child.parent_entity_index == root)
                .map(|child| child.source_entity_index),
        )
        .collect();
    assert_eq!(slots.len(), 12);
    let tiles: Vec<_> = slots
        .iter()
        .map(|slot| live.economy.map_chains.tile_of(*slot))
        .collect();
    for &tile in &tiles {
        assert_eq!(
            live.terrain.planes().entity_indices[usize::from(tile)] as u16,
            live.economy.map_chains.tile_head(tile)
        );
    }
    live.economy
        .economy
        .fields
        .authored_model22
        .iter_mut()
        .find(|wind| wind.source_entity_index == root)
        .unwrap()
        .life = 0;
    assert!(
        live.economy
            .economy
            .step_authored_model22_slot(root, &live.terrain)
    );
    live.economy
        .rebuild_source_arena_with_terrain(&mut live.terrain, &[])
        .unwrap();
    for slot in slots {
        assert!(!live.economy.map_chains.is_linked(slot));
    }
    for tile in tiles {
        assert_eq!(
            live.terrain.planes().entity_indices[usize::from(tile)] as u16,
            live.economy.map_chains.tile_head(tile)
        );
    }
}

#[test]
fn scheduled_authored_wind_publishes_old_and_new_map_heads() {
    let mut session = campaign();
    let root = session
        .allocate_level2_authored_model22(110)
        .unwrap()
        .source_entity_index;
    let slots: Vec<_> = std::iter::once(root)
        .chain(
            session
                .economy
                .model22_spawned_children()
                .iter()
                .filter(|child| child.parent_entity_index == root)
                .map(|child| child.source_entity_index),
        )
        .collect();
    let mut crossed = false;
    for _ in 0..4 {
        let before: Vec<_> = slots
            .iter()
            .map(|slot| session.economy.map_chains.tile_of(*slot))
            .collect();
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "Wind scheduling must not roll back on a dispatch error"
        );
        let live = &session.session.state.live;
        for (&slot, &old_tile) in slots.iter().zip(&before) {
            let new_tile = live.economy.map_chains.tile_of(slot);
            crossed |= old_tile != new_tile;
            for tile in [old_tile, new_tile] {
                assert_eq!(
                    live.terrain.planes().entity_indices[usize::from(tile)] as u16,
                    live.economy.map_chains.tile_head(tile)
                );
            }
        }
    }
    assert!(crossed, "scheduled wind must cross a tile during the check");
}

#[test]
fn source_field_constructors_keep_horizontal_bounds_separate_from_height() {
    let mut session = campaign();
    let wind = session.allocate_level2_authored_model22(110).unwrap();
    let rows = session.field_source_rows();
    let root = rows
        .iter()
        .find(|row| row.index == wind.source_entity_index)
        .unwrap();
    assert_eq!((root.array_pitch(), root.array_fov()), (38, 31));
    assert_eq!(
        i16::from_le_bytes(root.record[90..92].try_into().unwrap()),
        38
    );
    let child = rows.iter().find(|row| row.model == 75).unwrap();
    assert_eq!(child.actual_speed, 50, "AddWind clones the parent's speed");
    assert_eq!(
        i16::from_le_bytes(child.record[90..92].try_into().unwrap()),
        child.array_pitch()
    );
    assert!(child.array_fov() > 0);
    let edge = Position {
        y: wind.position.y.wrapping_add(37),
        ..wind.position
    };
    assert!(
        wind::pulse_overlap(wind, edge, 0, 0),
        "vertical height31 cannot narrow the horizontal band38"
    );
    assert!(
        !wind::pulse_overlap(
            wind,
            Position {
                y: edge.y.wrapping_add(1),
                ..edge
            },
            0,
            0
        ),
        "source overlap remains strict at38"
    );

    let particle =
        session.session.definition.presentation.particles[usize::from(transport::PARTICLE)];
    let live = &mut session.session.state.live;
    let slot = transport::construct(
        &mut live.economy,
        &mut live.terrain,
        live.core.state.position,
        definition(FieldKind::Transport),
        particle,
    )
    .unwrap()
    .unwrap();
    let row = &live.economy.entities[usize::from(slot)];
    assert_eq!((row.array_pitch(), row.array_fov()), (256, 256));
    assert_eq!(
        i16::from_le_bytes(row.record[90..92].try_into().unwrap()),
        256
    );
}

#[test]
fn scheduled_slowing_reaches_player_upkeep_palette_and_reclaim() {
    let mut session = campaign();
    let slot = scheduled_impact(&mut session, FieldKind::Slowing);
    campaign_step(&mut session);
    assert_eq!(session.session.state.live.core.state.speed_modifier, 1);
    assert_eq!(
        session.session.state.live.core.state.speed_modifier_counter,
        8
    );
    assert!(session.session.state.live.economy.entities[usize::from(slot)].drawing_disabled());
    campaign_step(&mut session);
    assert!(!session.session.state.live.economy.entities[usize::from(slot)].active);
    assert_eq!(
        session.session.state.live.core.state.speed_modifier_counter,
        7
    );
    let original = [96, 128, 160].repeat(256);
    let mut tinted = original.clone();
    session.field_palette().apply_rgb8(&mut tinted);
    assert_ne!(tinted, original, "actual frame palette consumes slowing");
    for _ in 0..12 {
        campaign_step(&mut session);
    }
    let state = &session.session.state.live.core.state;
    assert_eq!((state.speed_modifier, state.speed_modifier_counter), (0, 0));
    let mut recovered = original.clone();
    session.field_palette().apply_rgb8(&mut recovered);
    assert_eq!(
        recovered, original,
        "frame service finishes the recovery fade"
    );
}

#[test]
fn scheduled_web_damages_actual_player_once_and_refresh_only_recovers() {
    // The source starts the player with 100 protected visits. The same Web
    // contact is discarded by that prelude before normal damage consumption.
    let mut protected = campaign();
    let protected_life = protected.session.state.live.player_life;
    scheduled_impact(&mut protected, FieldKind::Web);
    campaign_step(&mut protected);
    campaign_step(&mut protected);
    assert_eq!(protected.session.state.live.player_life, protected_life);
    assert_eq!(protected.session.state.live.player_pending_attacker, 0);

    let mut session = campaign();
    // Exercise the ordinary post-protection visit, not the spawn grace period.
    session.session.state.live.player_protection_ticks = 0;
    let life = session.session.state.live.player_life;
    let first = scheduled_impact(&mut session, FieldKind::Web);
    campaign_step(&mut session);
    assert_eq!(session.session.state.live.player_life, life);
    assert_eq!(session.session.state.live.player_pending_damage, 200);
    assert_eq!(session.session.state.live.player_pending_attacker, first);
    assert_eq!(
        session.session.state.live.core.state.immobilize_subcounter,
        10
    );
    let second = scheduled_impact(&mut session, FieldKind::Web);
    let rng = session.session.state.live.economy.entities[usize::from(second)].entity_rng;
    campaign_step(&mut session);
    assert_eq!(session.session.state.live.player_life, life - 200);
    assert_eq!(session.session.state.live.player_pending_attacker, 0);
    assert_eq!(
        session.session.state.live.core.state.immobilize_subcounter,
        10
    );
    assert_eq!(
        session.session.state.live.economy.entities[usize::from(second)].entity_rng,
        rng
    );
    for _ in 0..10 {
        campaign_step(&mut session);
    }
    let state = &session.session.state.live.core.state;
    assert_eq!(
        (state.immobilize_counter, state.immobilize_subcounter),
        (0, 0)
    );
    assert_eq!(session.session.state.live.player_life, life - 200);
    assert!(!session.session.state.live.economy.entities[usize::from(second)].active);
}

#[test]
fn slowing_impacts_stack_once_then_real_flight_recovers_all_stages() {
    let (body, mut state) = player();
    let mut source = impact(status::Kind::Slowing);
    for (stage, count) in [(1, 142), (2, 199), (3, 256)] {
        let result = status::impact(
            &mut source,
            definition(FieldKind::Slowing),
            Some((&body, &mut state)),
            0,
        )
        .unwrap();
        assert_eq!(state.speed_modifier, stage);
        assert_eq!(state.speed_modifier_counter, 8);
        assert_eq!(result.palette.unwrap().count, count);
        assert_eq!(result.sounds.len(), usize::from(stage == 1));
        assert!(result.contacts.is_empty());
    }
    let rng = source.entity_rng;
    state.recoil_speed = 12;
    let capped = status::impact(
        &mut source,
        definition(FieldKind::Slowing),
        Some((&body, &mut state)),
        0,
    )
    .unwrap();
    assert!(capped.palette.is_none());
    assert_eq!(source.entity_rng, rng);
    assert_eq!(state.recoil_speed, 12);
    assert!(source.drawing_disabled());
    let mut context = FlightContext {
        effects: Some(FlightEffectsContext::default()),
        ..FlightContext::default()
    };
    for tick in 1..=24 {
        step_turn_with_context(
            &mut state,
            PlayerCommand::default(),
            FlightProfile::OUTDOOR,
            &Ground,
            &mut context,
        );
        assert_eq!(state.speed_modifier, 3 - tick / 8);
    }
    assert_eq!(state.speed_modifier_counter, 0);
    assert_eq!(context.effects.unwrap().palette_submode, 1);
    let before = state.position;
    step_turn_with_context(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::OUTDOOR,
        &Ground,
        &mut FlightContext::default(),
    );
    assert!(
        state.position.y < before.y,
        "ordinary translation resumes after expiry"
    );
}

#[test]
fn web_refreshes_without_duplicate_contact_and_real_movement_expires_it() {
    let (body, mut state) = player();
    let mut source = impact(status::Kind::Web);
    source.set_subspell_index(200);
    let first = status::impact(
        &mut source,
        definition(FieldKind::Web),
        Some((&body, &mut state)),
        0,
    )
    .unwrap();
    assert_eq!((first.contacts.len(), first.contacts[0].payload), (1, 200));
    assert_eq!(
        (
            state.immobilize_counter,
            state.immobilize_subcounter,
            state.recoil_speed
        ),
        (1, 10, -80)
    );
    let rng = source.entity_rng;
    let refresh = status::impact(
        &mut source,
        definition(FieldKind::Web),
        Some((&body, &mut state)),
        0,
    )
    .unwrap();
    assert!(refresh.contacts.is_empty() && refresh.sounds.is_empty());
    assert_eq!(source.entity_rng, rng);
    state.recoil_speed = 0;
    let start = state.position;
    for tick in 1..=10 {
        step_turn_with_context(
            &mut state,
            PlayerCommand::default(),
            FlightProfile::OUTDOOR,
            &Ground,
            &mut FlightContext::default(),
        );
        assert_eq!(state.immobilize_counter, if tick < 10 { 1 } else { 0 });
        assert_eq!(
            state.position.y, start.y,
            "web consumes the actual translation branch"
        );
    }
    assert_eq!(state.position.z, start.z - 9 * 51 - 16);
    let before = state.position;
    step_turn_with_context(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::OUTDOOR,
        &Ground,
        &mut FlightContext::default(),
    );
    assert!(state.position.y < before.y);
}

#[test]
fn transport_uses_facing_and_destination_and_life_one_hides_before_contact() {
    let (mut body, mut state) = player();
    let mut source = new_event(
        120,
        0,
        Position {
            y: state.position.y - 1,
            ..state.position
        },
    );
    source.record[88..90].copy_from_slice(&256_i16.to_le_bytes());
    source.record[90..92].copy_from_slice(&256_i16.to_le_bytes());
    source.record[92..94].copy_from_slice(&256_i16.to_le_bytes());
    source.record[158..160].copy_from_slice(&30_000_u16.to_le_bytes());
    source.record[160..162].copy_from_slice(&40_000_u16.to_le_bytes());
    let mut rejected_source = source.clone();
    let mut rejected_body = body.clone();
    let mut rejected_player = state.clone();
    rejected_player.yaw = 170;
    let rejected = transport::step(
        &mut rejected_source,
        &mut rejected_body,
        &mut rejected_player,
        &Ground,
        384,
        0,
    );
    assert!(
        !rejected.transported,
        "facing170 is outside the strict gate"
    );
    assert_eq!(rejected_player.position, state.position);
    state.yaw = 169;
    let phase = source.phase;
    let outcome = transport::step(&mut source, &mut body, &mut state, &Ground, 384, 0);
    assert!(outcome.transported);
    assert_eq!(
        state.position,
        Position {
            x: 30_000,
            y: 40_000,
            z: 384
        }
    );
    assert_eq!(body.position, state.position);
    assert_eq!(source.life, 0);
    assert_eq!(source.phase, phase, "common tail owns phase");
    assert_eq!(outcome.sounds, [(120, 21), (120, 22)]);
    source.set_life(1);
    let before = state.position;
    let outcome = transport::step(&mut source, &mut body, &mut state, &Ground, 384, 0);
    assert!(!outcome.transported);
    assert_eq!(
        (source.life, source.drawing_disabled(), state.position),
        (0, true, before)
    );
    assert_eq!(outcome.sounds, [(120, 20)]);
}

#[test]
fn transport_uses_distinct_y_extent_and_unwrapped_vertical_distance() {
    let (mut body, mut state) = player();
    state.yaw = 0;
    body.set_source_overlap_half_extents(2, 10);
    let mut source = new_event(
        120,
        0,
        Position {
            y: state.position.y.wrapping_sub(29),
            ..state.position
        },
    );
    source.set_source_overlap_half_extents(2, 20);
    source.record[92..94].copy_from_slice(&256_i16.to_le_bytes());
    source.record[158..160].copy_from_slice(&30_000_u16.to_le_bytes());
    source.record[160..162].copy_from_slice(&40_000_u16.to_le_bytes());

    let mut edge_source = source.clone();
    let mut edge_body = body.clone();
    let mut edge_state = state.clone();
    edge_source.set_position(Position {
        y: edge_source.position.y.wrapping_sub(1),
        ..edge_source.position
    });
    assert!(
        !transport::step(
            &mut edge_source,
            &mut edge_body,
            &mut edge_state,
            &Ground,
            384,
            0
        )
        .transported
    );
    assert!(transport::step(&mut source, &mut body, &mut state, &Ground, 384, 0).transported);

    let (mut body, mut state) = player();
    state.yaw = 0;
    state.position.z = i16::MIN;
    body.set_position(state.position);
    let mut source = new_event(
        121,
        0,
        Position {
            z: i16::MAX,
            y: state.position.y.wrapping_sub(1),
            ..state.position
        },
    );
    source.set_source_overlap_half_extents(256, 256);
    source.record[92..94].copy_from_slice(&256_i16.to_le_bytes());
    assert!(!transport::step(&mut source, &mut body, &mut state, &Ground, 384, 0).transported);
}

#[test]
fn transport_visits_each_player_before_finishing_its_single_lifetime_tick() {
    let (body, state) = player();
    let mut source = new_event(
        120,
        0,
        Position {
            y: state.position.y.wrapping_sub(1),
            ..state.position
        },
    );
    source.set_source_overlap_half_extents(256, 256);
    source.record[92..94].copy_from_slice(&256_i16.to_le_bytes());
    source.record[158..160].copy_from_slice(&30_000_u16.to_le_bytes());
    source.record[160..162].copy_from_slice(&40_000_u16.to_le_bytes());
    source.set_life(2);
    let original_height = source.position.z;
    let (mut outcome, visit_players) = transport::begin(&mut source);
    assert!(visit_players);
    assert_eq!(source.life, 1, "one source callback decrements life once");
    let mut local = body.clone();
    let mut rival = body.clone();
    let mut facing_away = body;
    assert!(
        transport::visit_player(
            &mut source,
            &mut local,
            state.position,
            0,
            &Ground,
            384,
            true,
            &mut outcome
        )
        .is_some()
    );
    assert!(
        transport::visit_player(
            &mut source,
            &mut rival,
            state.position,
            0,
            &Ground,
            384,
            false,
            &mut outcome
        )
        .is_some()
    );
    assert!(
        transport::visit_player(
            &mut source,
            &mut facing_away,
            state.position,
            170,
            &Ground,
            384,
            false,
            &mut outcome
        )
        .is_none()
    );
    assert_eq!(rival.position, local.position);
    assert_eq!(facing_away.position, state.position);
    assert_eq!(
        source.position.z, original_height,
        "the source height changes only after all visits"
    );
    assert_eq!(outcome.sounds, [(120, 21), (120, 22), (120, 22)]);
    assert_eq!(
        outcome.palette,
        Some(super::PaletteRequest { mode: 6, count: 0 })
    );
    transport::finish(&mut source, &Ground);
    assert_eq!(source.position.z, 0);
}

#[test]
fn retained_ordinary_transport_moves_scheduled_player_and_publishes_map_heads() {
    let level = std::env::var("MC_TRANSPORT_LEVEL").map_or(15, |value| {
        value.parse().expect("transport level is a number")
    });
    let package = std::env::var_os("MC_TRANSPORT_PACKAGE").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261024-b/level15")
        },
        std::path::PathBuf::from,
    );
    let base = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::ordinary(level).unwrap(),
    )
    .expect("retained ordinary package loads")
    .simulation()
    .clone();
    let source = base
        .economy
        .entities
        .iter()
        .find(|row| {
            row.active && (row.class, row.model, row.action) == (10, 34, 36) && row.life == 0
        })
        .expect("owner-authored reset contains a persistent transport")
        .clone();
    let mut destination = transport::destination(&source);
    let parameter = source
        .parameter_relation
        .and_then(|index| {
            base.session
                .definition
                .presentation
                .entity_parameters
                .get(usize::from(index))
        })
        .expect("retained transport has its source parameter relation");
    destination.z = base
        .canonical_terrain()
        .ground_height(destination)
        .wrapping_add(i16::from_le_bytes([parameter[12], parameter[13]]));
    let contact = Position {
        y: source.position.y.wrapping_add(1),
        ..source.position
    };
    let heading = crate::heading_for_positions(contact, source.position) as i16;
    let ballistic = if level == 17 {
        let row = base
            .economy
            .entities
            .iter()
            .find(|row| row.active && (row.class, row.model, row.action) == (2, 7, 20))
            .expect("retained Level 17 contains its source ballistic action20");
        Some((
            row.index,
            row.phase,
            base.economy.map_chains.tile_of(row.index),
        ))
    } else {
        None
    };
    let rival = base
        .core
        .players
        .iter()
        .take(usize::from(base.core.runtime.player_count))
        .find(|player| player.is_ai && player.active_player_index >= 0 && player.entity_index != 0)
        .map(|player| player.entity_index);
    if base.core.runtime.player_count > 1 {
        assert!(
            rival.is_some(),
            "multi-wizard source package has a participating rival"
        );
    }
    for (facing, expected_transport) in [(heading, true), ((heading + 1024) & 2047, false)] {
        let mut session = base.clone();
        let player_slot;
        {
            let live = &mut session.session.state.live;
            player_slot = live.core.state.entity_index;
            live.core.state.position = contact;
            live.core.state.yaw = facing;
            live.core.state.actual_speed = 0;
            live.core.state.target_speed = 0;
            let body = &mut live.economy.entities[usize::from(player_slot)];
            body.set_position(contact);
            body.set_yaw(facing);
            crate::world::effects::sync_position(
                &mut live.economy,
                &mut live.terrain,
                player_slot,
                contact,
                true,
                false,
            )
            .unwrap();
            if let Some(rival_slot) = rival {
                let rival_body = &mut live.economy.entities[usize::from(rival_slot)];
                assert!(
                    rival_body.active,
                    "retained rival has an active source body"
                );
                rival_body.set_position(contact);
                rival_body.set_yaw(facing);
                crate::world::effects::sync_position(
                    &mut live.economy,
                    &mut live.terrain,
                    rival_slot,
                    contact,
                    true,
                    false,
                )
                .unwrap();
            }
        }
        session
            .set_level2_entity_position(player_slot, contact)
            .unwrap();
        let old_tile = session.economy.map_chains.tile_of(player_slot);
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "scheduled transport must not roll back: {:?}",
            session.dispatch_events()
        );
        let live = &session.session.state.live;
        if let Some((slot, before_phase, old_tile)) = ballistic {
            let row = &live.economy.entities[usize::from(slot)];
            assert!(row.active, "action20 survives its ordinary source pass");
            assert_eq!((row.class, row.model, row.action), (2, 7, 20));
            assert_eq!(
                row.phase,
                before_phase.wrapping_add(1),
                "enabled action20 advances its source tail"
            );
            for tile in [old_tile, live.economy.map_chains.tile_of(slot)] {
                assert_eq!(
                    live.terrain.planes().entity_indices[usize::from(tile)] as u16,
                    live.economy.map_chains.tile_head(tile)
                );
            }
        }
        let actual = live.core.state.position;
        let near = |a: u16, b: u16| i32::from(a.wrapping_sub(b) as i16).abs() < 256;
        assert_eq!(
            near(actual.x, destination.x) && near(actual.y, destination.y),
            expected_transport,
            "forward-facing contact must be the real transport gate"
        );
        if expected_transport {
            assert_eq!(
                transport::destination(&live.economy.entities[usize::from(source.index)]).z,
                destination.z
            );
        } else {
            assert!(near(actual.x, contact.x) && near(actual.y, contact.y));
        }
        if let Some(rival_slot) = rival {
            let rival_position = live.economy.entities[usize::from(rival_slot)].position;
            assert_eq!(
                near(rival_position.x, destination.x) && near(rival_position.y, destination.y),
                expected_transport,
                "the same scheduled source visits the rival wizard"
            );
            if !expected_transport {
                assert!(near(rival_position.x, contact.x) && near(rival_position.y, contact.y));
            }
        }
        let player_slot = live.core.state.entity_index;
        let new_tile = live.economy.map_chains.tile_of(player_slot);
        assert!(live.economy.map_chains.is_linked(player_slot));
        for tile in [old_tile, new_tile] {
            assert_eq!(
                live.terrain.planes().entity_indices[usize::from(tile)] as u16,
                live.economy.map_chains.tile_head(tile)
            );
        }
    }
}

#[test]
fn follower_parent_sentinel_exact_life_and_hidden_parent_semantics() {
    let mut child = new_event(
        20,
        7,
        Position {
            x: 10,
            y: 20,
            z: 30,
        },
    );
    child.set_yaw(2040);
    child.set_actual_speed(0);
    let mut parent = new_event(
        10,
        9,
        Position {
            x: 500,
            y: 700,
            z: 900,
        },
    );
    let before = child.clone();
    transport::step_follower(&mut child, Some(&parent), &Ground);
    assert_eq!(child, before);
    child.record[44..46].copy_from_slice(&10_u16.to_le_bytes());
    parent.active = false;
    parent.disable_drawing();
    parent.set_life(-1);
    transport::step_follower(&mut child, Some(&parent), &Ground);
    assert_eq!(
        (child.position, child.yaw),
        (
            Position {
                x: 500,
                y: 700,
                z: 256
            },
            37
        )
    );
    assert!(!child.drawing_disabled());
    parent.set_life(1);
    transport::step_follower(&mut child, Some(&parent), &Ground);
    assert!(child.drawing_disabled());
}

#[test]
fn wind_motion_reads_and_writes_the_exported_heading_word() {
    let mut source = new_event(10, 0, Position { x: 0, y: 0, z: 0 });
    source.record[48..50].copy_from_slice(&1111_i16.to_le_bytes());
    source.record[52..54].copy_from_slice(&2222_i16.to_le_bytes());
    let actor = wind::Actor::from_source(&source, None);
    assert_eq!(actor.move_heading, 2222);
    let motion = wind::Motion {
        actor: wind::Actor {
            move_heading: 333,
            ..actor
        },
        actual_speed: None,
        player_roll_delta: 0,
        contact: false,
    };
    motion.write_source(&mut source);
    assert_eq!(wind::Actor::from_source(&source, None).move_heading, 333);
    assert_eq!(source.record[48..50], 1111_i16.to_le_bytes());
}

#[test]
fn palette_uses_rgb6_source_math_and_recovers_through_frame_fade() {
    let original = vec![240_u8; 768];
    let mut palette = super::palette::Palette::default();
    palette.request(super::PaletteRequest {
        mode: 3,
        count: 256,
    });
    let mut colors = original.clone();
    palette.apply_rgb8(&mut colors);
    assert_eq!(
        colors, original,
        "pending source request waits for the frame consumer"
    );
    assert!(palette.advance_frame());
    palette.apply_rgb8(&mut colors);
    assert_eq!(&colors[..6], &[240, 240, 240, 16, 240, 16]);
    for _ in 0..5 {
        palette.advance_frame();
    }
    let mut recovered = original.clone();
    palette.apply_rgb8(&mut recovered);
    assert_eq!(recovered, original);
    palette.request(super::PaletteRequest { mode: 6, count: 0 });
    palette.advance_frame();
    palette.apply_rgb8(&mut recovered);
    assert_eq!(&recovered[3..6], &[252, 252, 252]);
}
