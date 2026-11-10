use super::{Definition, valid};
use crate::FlightWorld;
use crate::campaign::{
    CampaignSession,
    backend::{
        Mc2CampaignAdapter, Mc2CampaignBackend, Mc2CampaignInputs,
        load_configured_mc2_campaign_for_test,
    },
    mc2_level_definition::Mc2LevelDefinition,
    schedule::{ScheduleReceipt, SourcePass},
    source_arena::SourceEntityHandle,
    spec::CampaignSelection,
};
use crate::game_session::{GameCommand, GameSession};
use crate::level_definition::{ItemKind, LevelEntityKind};
use crate::native_world::{
    EntitySnapshot, Mc2DispatchEvent, Mc2Simulation, area_damage, source_target,
};

type ItemGame = GameSession<Mc2CampaignBackend>;

fn simulation(game: &ItemGame) -> &Mc2Simulation {
    game.backend().compatibility_campaign().simulation()
}

fn simulation_mut(game: &mut ItemGame) -> &mut Mc2Simulation {
    game.backend_mut()
        .compatibility_campaign_mut_for_test()
        .simulation_mut()
}

fn advance(game: &mut ItemGame) {
    game.advance(game.identity().guard(), GameCommand::default())
        .unwrap();
    assert!(
        simulation(game)
            .dispatch_events()
            .iter()
            .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
        "{:?}",
        simulation(game).dispatch_events()
    );
}

fn callbacks(sim: &Mc2Simulation, slot: u16) -> u8 {
    let receipts = sim.cf10_production_schedule_receipts();
    let callbacks = receipts
        .iter()
        .filter(|receipt| {
            matches!(receipt,
        ScheduleReceipt::CallbackCompleted { slot: current, pass, .. }
            if *current == slot && *pass == SourcePass::UpdateEntities)
        })
        .count();
    assert!(
        !receipts.iter().any(|receipt| matches!(receipt,
        ScheduleReceipt::CallbackCompleted { slot: current, pass: SourcePass::ApplyEvents, .. }
            if *current == slot)),
        "source ApplyEvents skips passive item0"
    );
    assert_eq!(
        callbacks,
        receipts
            .iter()
            .filter(|receipt| matches!(receipt,
        ScheduleReceipt::EpilogueCompleted { slot: current, pass: SourcePass::UpdateEntities, .. }
            if *current == slot))
            .count()
    );
    callbacks.try_into().unwrap()
}

fn item(game: &ItemGame) -> Option<SourceEntityHandle> {
    let sim = simulation(game);
    let rows = sim
        .entities()
        .iter()
        .filter(|row| valid(row))
        .collect::<Vec<_>>();
    assert!(
        rows.len() <= 1,
        "no shipped item0; only one declared source row"
    );
    rows.first()
        .map(|row| sim.source_arena().handle(row.index).unwrap())
}

#[test]
fn configured_passive_item_uses_actual_switch_factory_empty_schedule_and_reset() {
    use sim_definition::adapter::build_level_definition;
    use std::sync::Arc;

    let package = std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES")
        .map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj")
            },
            std::path::PathBuf::from,
        )
        .join("level4");
    let original =
        CampaignSession::open(&package, CampaignSelection::ordinary(4).unwrap()).unwrap();
    let original_features = crate::level1_mission::parse_authored_features(
        &std::fs::read(package.join("authored-level.bin")).unwrap(),
    )
    .unwrap();
    assert!(
        !original_features
            .iter()
            .any(|row| row.kind == 14 && row.subtype == 0)
    );
    let original_level = original.simulation().semantic_level_definition().unwrap();
    let corridor = &original.simulation().entities()[195];
    assert_eq!(
        (
            corridor.class,
            corridor.model,
            corridor.action,
            corridor.life
        ),
        (14, 2, 7, 1)
    );
    let opening = &original_features[582];
    assert_eq!(
        (opening.kind, opening.subtype, opening.activation_group),
        (10, 63, 37)
    );

    // Declared source-language control: one unused row, in the REAL switch124
    // group37. All shipped features, actors, objectives and bindings survive.
    // The closed corridor's center gives a real later floor-change consumer.
    let index = original_features
        .iter()
        .rev()
        .find(|row| row.kind == 0 && row.index != 0)
        .unwrap()
        .index;
    let mut features = original_features.clone();
    let configured = &mut features[usize::from(index)];
    configured.kind = 14;
    configured.subtype = 0;
    configured.tile_x = corridor.position.x >> 8;
    configured.tile_y = corridor.position.y >> 8;
    configured.activation_group = 37;
    let mut source_bytes = std::fs::read(package.join("authored-level.bin")).unwrap();
    let start = crate::level1_mission::AUTHORED_FEATURE_OFFSET + usize::from(index) * 20;
    for (offset, value) in [
        (0, 14),
        (2, 0),
        (4, configured.tile_x),
        (6, configured.tile_y),
        (8, 37),
    ] {
        source_bytes[start + offset..start + offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    assert_eq!(
        crate::level1_mission::parse_authored_features(&source_bytes).unwrap(),
        features
    );
    let translated =
        Mc2LevelDefinition::catalog_binding(&source_bytes, CampaignSelection::ordinary(4).unwrap())
            .unwrap();
    let id = translated
        .item_entity_by_authored_record
        .get(&index)
        .unwrap();
    let entity = translated.semantic.entity(id).unwrap();
    assert_eq!(entity.kind(), LevelEntityKind::Item(ItemKind::PassiveItem));
    assert_eq!(entity.passive_item(), Some(Definition::default()));
    let mut entities = original_level.entities().to_vec();
    entities.push(entity.clone());
    let level = build_level_definition(
        entities,
        original_level.objectives().to_vec(),
        original_level.activation_groups().collect(),
    )
    .unwrap();
    let mut bindings = original
        .simulation()
        .configured_source_bindings_for_test(level);
    assert!(
        bindings
            .authored_items
            .insert(index, ItemKind::PassiveItem)
            .is_none()
    );
    assert!(
        bindings
            .authored_item_entities
            .insert(index, id.clone())
            .is_none()
    );
    let input = Mc2CampaignInputs::new(&package)
        .with_configured_source_for_test(features.clone(), bindings.clone())
        .unwrap();
    let catalog = crate::game_definition::default_catalog().unwrap();
    let definition = load_configured_mc2_campaign_for_test(&input, &catalog).unwrap();

    // The fixture seam rejects unrelated source edits and binding edits. They
    // cannot smuggle a forced switch/objective/NPC into the ordinary factory.
    for invalid in [
        {
            let mut rows = features.clone();
            rows[582].parameter_1 ^= 1;
            Mc2CampaignInputs::new(&package)
                .with_configured_source_for_test(rows, bindings.clone())
                .unwrap()
        },
        {
            let mut changed = bindings.clone();
            changed.authored_items.remove(&index);
            Mc2CampaignInputs::new(&package)
                .with_configured_source_for_test(features.clone(), changed)
                .unwrap()
        },
    ] {
        assert!(
            GameSession::open(
                &Mc2CampaignAdapter,
                Arc::new(definition.clone()),
                2014,
                invalid
            )
            .is_err()
        );
    }
    let mut game =
        GameSession::open(&Mc2CampaignAdapter, Arc::new(definition), 2014, input).unwrap();
    assert_eq!(
        simulation(&game).entities(),
        original.simulation().entities()
    );
    assert!(item(&game).is_none());
    let baseline = simulation(&game).entities().to_vec();
    let baseline_terrain = simulation(&game).canonical_terrain().clone();
    let initial_rng = simulation(&game).source_global_rng();
    let stages = simulation(&game).runtime().stages.clone();
    assert!(!stages.iter().any(
        |stage| stage.relation == crate::native_world::RuntimeRelation::AuthoredFeature(index)
    ));
    // Same elapsed neutral input without the declared approach cannot create
    // the configured item. No source activation bit is changed for either run.
    for _ in 0..16 {
        advance(&mut game);
        assert!(item(&game).is_none());
    }
    game.reset(game.identity().guard()).unwrap();

    // Reset twice through the neutral owner. Each run reaches production only
    // by one legal player approach; no group bits or source phases are staged.
    for run in 0..2 {
        assert_eq!(simulation(&game).entities(), baseline);
        assert_eq!(simulation(&game).canonical_terrain(), &baseline_terrain);
        assert_eq!(simulation(&game).source_global_rng(), initial_rng);
        // Frame zero is source5B100's closing life1, not settled life3.
        // With positive strip speed, byte-height0..254 converges within256
        // eligible visits. Retain normal input and the original controller;
        // the real switch is approached only after ordinary closure settles.
        let corridor_handle = simulation(&game).source_arena().handle(195).unwrap();
        let corridor_position = simulation(&game).entities()[195].position;
        let open_floor = simulation(&game)
            .canonical_terrain()
            .ground_height(corridor_position);
        let open_ceiling = simulation(&game)
            .canonical_terrain()
            .ceiling_height(corridor_position)
            .unwrap();
        let extent = i32::from(simulation(&game).entities()[195].source_word_96() as i16);
        let length = (2 * ((extent << 8) + 512) + 128) >> 8;
        assert!(
            length / 4 > 0,
            "the actual strip has positive source closure speed"
        );
        for _ in 0..256 {
            advance(&mut game);
            assert_eq!(
                simulation(&game).source_arena().handle(195),
                Some(corridor_handle)
            );
            assert!(
                item(&game).is_none(),
                "neutral closure cannot activate group37"
            );
            if simulation(&game).entities()[195].life == 3 {
                break;
            }
        }
        assert_eq!(simulation(&game).entities()[195].life, 3);
        assert!(
            simulation(&game)
                .canonical_terrain()
                .ground_height(corridor_position)
                > open_floor
        );
        assert!(
            simulation(&game)
                .canonical_terrain()
                .ceiling_height(corridor_position)
                .unwrap()
                < open_ceiling
        );
        assert!(
            simulation(&game)
                .canonical_terrain()
                .cave_collision_blocked(corridor_position)
        );
        let sim = simulation(&game);
        let switch = &sim.entities()[124];
        assert_eq!(
            (switch.class, switch.model, switch.action, switch.source_id),
            (11, 0, 0, 37)
        );
        let player = sim.state.entity_index;
        let switch_handle = sim.source_arena().handle(124).unwrap();
        let corridor_handle = sim.source_arena().handle(195).unwrap();
        let mut approach = switch.position;
        approach.y = approach.y.wrapping_sub(768);
        approach.z = sim
            .canonical_terrain()
            .ground_height(approach)
            .checked_add(sim.profile().ground_clearance)
            .unwrap();
        assert!(
            approach.z
                <= sim
                    .canonical_terrain()
                    .ceiling_height(approach)
                    .unwrap()
                    .checked_sub(sim.profile().ceiling_clearance)
                    .unwrap()
        );
        assert!(!sim.canonical_terrain().cave_collision_blocked(approach));
        let extent = |row: &EntitySnapshot, offset| {
            i16::from_le_bytes(row.record_bytes()[offset..offset + 2].try_into().unwrap())
        };
        let parent = &sim.entities()[usize::from(player)];
        assert!(crate::level1_mission::source_entities_overlap_xy(
            switch.position,
            extent(switch, 88),
            extent(switch, 90),
            approach,
            extent(parent, 88),
            extent(parent, 90)
        ));
        let position = features[usize::from(index)].world_position(
            sim.canonical_terrain()
                .ground_height(features[usize::from(index)].world_position(0)),
        );
        let floor_before = sim.canonical_terrain().ground_height(position);
        simulation_mut(&mut game)
            .set_level2_entity_position(player, approach)
            .unwrap();
        let mut activation = false;
        for _ in 0..16 {
            // real switch is visited every eighth phase
            advance(&mut game);
            activation |= simulation(&game).dispatch_events().iter().any(|event| matches!(event,
                Mc2DispatchEvent::AuthoredGroupActivated { source_slot:124, group:37, authored_indices, .. }
                    if authored_indices.contains(&582) && authored_indices.contains(&index)));
            if item(&game).is_some() {
                break;
            }
        }
        assert!(
            activation,
            "real switch/group must publish both original command and configured row"
        );
        let handle = item(&game).expect("ordinary source factory must construct item0");
        let sim = simulation(&game);
        assert_eq!(sim.source_arena().handle(124), Some(switch_handle));
        assert_eq!(sim.source_arena().handle(195), Some(corridor_handle));
        assert_eq!(
            sim.runtime().stages,
            stages,
            "58DA0 is a no-op when no original stage targets the dormant row"
        );
        let row = &sim.entities()[usize::from(handle.source_slot)];
        assert_eq!(
            (
                row.position,
                row.life,
                row.max_life,
                row.control_flags,
                row.actual_speed
            ),
            (position, 300, 300, 12, 16)
        );
        assert_eq!(
            (row.class, row.model, row.action, row.source_id),
            (14, 0, 0, handle.source_slot)
        );
        assert_eq!(
            row.phase,
            (row.index as u8).wrapping_add(callbacks(sim, row.index))
        );
        assert_eq!(row.source_dword_10(), 0);
        assert_eq!(row.subspell_index, 100);
        assert_eq!(row.record_bytes()[74], 0);
        assert_eq!(
            (row.array_pitch(), row.array_roll(), row.array_fov()),
            (384, 384, 384)
        );
        assert_eq!(row.particle_parameter_index, Some(77));
        assert_eq!(row.parameter_relation, Some(59));
        assert!(sim.economy().map_chains().is_linked(row.index));
        let target = source_target::read(
            sim.source_arena(),
            source_target::Context::default(),
            row.index,
        )
        .unwrap()
        .unwrap();
        assert_eq!((target.position, target.life), (position, 300));
        assert_eq!(target.collision.unwrap().half_width, 384);
        assert!(
            sim.campaign_entity_presentations(0)
                .unwrap()
                .iter()
                .any(|dto| dto.entity_index == row.index
                    && dto.render_parameter_index == 77
                    && dto.position == position)
        );

        // Explicit source11900 selected-contact control, not a fabricated
        // pickup/death path: the empty callback must retain the real mailbox.
        area_damage::contact(
            simulation_mut(&mut game).source_arena_mut(),
            None,
            handle.source_slot,
            900,
            player,
        )
        .unwrap();
        let mut floor_changed = false;
        let mut observed_callback = false;
        for _ in 0..96 {
            // existing corridor bound; no adaptive input
            let before = simulation(&game).entities()[usize::from(handle.source_slot)].clone();
            advance(&mut game);
            let sim = simulation(&game);
            assert_eq!(sim.source_arena().handle(handle.source_slot), Some(handle));
            let row = &sim.entities()[usize::from(handle.source_slot)];
            let visits = callbacks(sim, handle.source_slot);
            observed_callback |= visits != 0;
            assert_eq!(row.phase, before.phase.wrapping_add(visits));
            assert_eq!(
                (
                    row.position,
                    row.life,
                    row.max_life,
                    row.entity_rng,
                    row.control_flags
                ),
                (position, 300, 300, before.entity_rng, 12)
            );
            assert_eq!(
                &row.record_bytes()[94..104],
                &before.record_bytes()[94..104],
                "empty callback cannot consume a queued contact payload/owner"
            );
            assert_eq!(
                i32::from_le_bytes(row.record_bytes()[98..102].try_into().unwrap()),
                900
            );
            assert_eq!(
                u16::from_le_bytes(row.record_bytes()[102..104].try_into().unwrap()),
                player
            );
            assert!(sim.campaign_entity_presentations(0).unwrap().iter().any(
                |dto| dto.entity_index == row.index
                    && dto.render_parameter_index == 77
                    && dto.position == position
            ));
            floor_changed |= sim.canonical_terrain().ground_height(position) != floor_before;
            if floor_changed && sim.entities()[195].life == 4 {
                break;
            }
        }
        assert!(
            floor_changed && observed_callback,
            "actual terrain movement must discriminate forbidden item height-following"
        );

        // Declared common hide control. Empty item0 has no natural expiry or
        // pickup. Current hidden identity persists until the next source pass.
        let row = &mut simulation_mut(&mut game).source_arena_mut().entities
            [usize::from(handle.source_slot)];
        row.disable_drawing();
        assert_eq!(
            simulation(&game).source_arena().handle(handle.source_slot),
            Some(handle)
        );
        assert!(
            simulation(&game)
                .economy()
                .map_chains()
                .is_linked(handle.source_slot)
        );
        assert!(
            simulation(&game)
                .campaign_entity_presentations(0)
                .unwrap()
                .iter()
                .all(|dto| dto.entity_index != handle.source_slot)
        );
        advance(&mut game);
        let sim = simulation(&game);
        assert!(sim.source_arena().resolve(handle).is_none());
        if sim.source_arena().handle(handle.source_slot).is_none() {
            assert!(!sim.economy().map_chains().is_linked(handle.source_slot));
            assert!(
                sim.economy()
                    .free_source_slots()
                    .contains(&handle.source_slot)
            );
            assert_eq!(sim.entities()[usize::from(handle.source_slot)].class, 0);
        } else {
            eprintln!(
                "item run{run}: slot reused by {:?}; original {handle:?} is retired",
                sim.source_arena().handle(handle.source_slot)
            );
        }
        eprintln!(
            "configured item run{run}: switch124/group37 -> {handle:?}, position{position:?}, real floor changed, empty callback/mailbox/DTO and following reclaim verified"
        );
        game.reset(game.identity().guard()).unwrap();
        assert!(item(&game).is_none());
    }
    assert_eq!(simulation(&game).entities(), baseline);
    assert_eq!(simulation(&game).canonical_terrain(), &baseline_terrain);
}
