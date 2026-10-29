//! Source57390 cell policy. Direct death is distinct from a damage mailbox;
//! the actor adapter writes each actual family carrier before later visits.

use crate::{
    Position,
    campaign::source_arena::SourceArena,
    native_world::area_damage::{self, ActorDomains},
};

pub(crate) fn cell(
    arena: &mut SourceArena,
    mut actors: Option<ActorDomains<'_>>,
    x: u8,
    y: u8,
    owner: u16,
) -> Result<(), String> {
    for slot in arena.economy.map_chains.entity_chain(x, y) {
        if crate::world::scenery::trees::hide_for_terrain(arena, slot, owner) {
            continue;
        }
        let Some(target) =
            area_damage::source_actor(arena, actors.as_mut().map(ActorDomains::reborrow), slot)
        else {
            continue;
        };
        if target.source_id == owner {
            continue;
        }
        match target.class {
            2 => arena
                .entities
                .get_mut(usize::from(slot))
                .ok_or("buried scenery slot is outside the arena")?
                .disable_drawing(),
            // Model25 compares uint8 action against signed -56 in C++ and
            // therefore always takes this kill path, including action200.
            5 if !matches!(target.model, 6 | 8 | 10 | 16 | 22 | 23 | 27) => {
                if !area_damage::apply_construction_kill(
                    arena,
                    actors.as_mut().map(ActorDomains::reborrow),
                    slot,
                    owner,
                )? {
                    return Err(format!(
                        "buried actor slot {slot} has no authoritative direct-life carrier"
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// The BUILD footprint is supplied by its retained construction owner.
/// Source row-major order and world-word rounding remain observable.
pub(crate) fn footprint(
    arena: &mut SourceArena,
    mut actors: Option<ActorDomains<'_>>,
    position: Position,
    width: u8,
    height: u8,
    owner: u16,
) -> Result<(), String> {
    let x = (position.x.wrapping_add(128) >> 8) as u8;
    let y = (position.y.wrapping_add(128) >> 8) as u8;
    let start_x = x.wrapping_sub(width / 2);
    let start_y = y.wrapping_sub(height / 2);
    for row in 0..height {
        for column in 0..width {
            cell(
                arena,
                actors.as_mut().map(ActorDomains::reborrow),
                start_x.wrapping_add(column),
                start_y.wrapping_add(row),
                owner,
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_burial_hides_real_tree_and_reclaims_it_after_the_pass() {
        let package = std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
            },
            std::path::PathBuf::from,
        );
        let cache = std::env::var_os("MC_LEVEL2_GAMEPLAY_CACHE_V5").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/level2-day-owner-cache/components/gameplay")
            },
            std::path::PathBuf::from,
        );
        let mut simulation = crate::campaign::CampaignSession::open_with_owner_cache(
            package,
            cache,
            crate::campaign::spec::CampaignSelection::LEVEL2,
        )
        .expect("retained campaign and owner cache")
        .simulation()
        .clone();
        let tree_authored_index = simulation
            .session
            .state
            .live
            .mission
            .authored_features()
            .iter()
            .find(|feature| {
                feature.kind == 2
                    && feature.subtype == 0
                    && simulation
                        .session
                        .definition
                        .scenery_for_authored_record(feature.index)
                        .is_some()
            })
            .expect("retained package has a semantic authored Tree")
            .index;
        let mut tree = simulation
            .allocate_level2_authored_tree(tree_authored_index)
            .expect("ordinary authored Tree producer");
        let (slot, generation, tile) = {
            let live = &mut simulation.session.state.live;
            let (x, y) = (0_u8..=u8::MAX)
                .flat_map(|y| (0_u8..=u8::MAX).map(move |x| (x, y)))
                .find(|&(x, y)| live.economy.map_chains.entity_chain(x, y).is_empty())
                .expect("retained map has an empty cell");
            let position = Position {
                x: (u16::from(x) << 8) | 128,
                y: (u16::from(y) << 8) | 128,
                z: 0,
            };
            tree.position = position;
            live.economy
                .economy
                .authored_tree_mut(tree.source_entity_index)
                .expect("authored Tree retains its authoritative carrier")
                .position = position;
            let slot = tree.source_entity_index;
            crate::world::effects::sync_position(
                &mut live.economy,
                &mut live.terrain,
                slot,
                tree.position,
                true,
                false,
            )
            .expect("Tree publishes its source map head");
            let tile = live.economy.map_chains.tile_of(slot);
            let (x, y) = (tile as u8, (tile >> 8) as u8);
            cell(&mut live.economy, None, x, y, tree.entity_id).unwrap();
            let same_owner = live
                .economy
                .authored_trees()
                .iter()
                .find(|row| row.source_entity_index == slot)
                .unwrap();
            assert!(same_owner.active && same_owner.drawing_enabled);
            let owner = live.core.state.entity_index;
            assert_ne!(owner, tree.entity_id);
            cell(&mut live.economy, None, x, y, owner).unwrap();
            let hidden = live
                .economy
                .authored_trees()
                .iter()
                .find(|row| row.source_entity_index == slot)
                .unwrap();
            assert!(hidden.active && !hidden.drawing_enabled);
            assert_ne!(hidden.control_flags & 0x400, 0);
            assert!(
                live.economy.map_chains.is_linked(slot),
                "hidden Tree remains linked until the next source boundary"
            );
            (slot, live.economy.economy.slot_generation(slot), tile)
        };
        simulation.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert!(
            !simulation
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "ordinary source pass must not roll back: {:?}",
            simulation.dispatch_events()
        );
        let live = &simulation.session.state.live;
        assert!(
            live.economy
                .authored_trees()
                .iter()
                .all(|tree| tree.source_entity_index != slot
                    || !tree.active
                    || live.economy.economy.slot_generation(slot) != generation)
        );
        assert_eq!(
            live.terrain.planes().entity_indices[usize::from(tile)] as u16,
            live.economy.map_chains.tile_head(tile),
            "Tree reclaim publishes the actual terrain map head"
        );

        // Exercise the same consumer from the real House BUILD callback.
        // This arranges the existing House and a real Tree constructor for a
        // controlled source-order check; it is not an authored playthrough.
        let mut tree = simulation
            .allocate_level2_authored_tree(tree_authored_index)
            .expect("ordinary authored Tree producer");
        let (house_slot, tree_slot, tree_generation, tree_tile, phase_before) = {
            let live = &mut simulation.session.state.live;
            let house = *live
                .ordinary
                .houses()
                .iter()
                .find(|house| house.source_retained())
                .expect("retained live House");
            let position = Position {
                x: house.position.x.wrapping_add(128) & 0xff00 | 128,
                y: house.position.y.wrapping_add(128) & 0xff00 | 128,
                z: house.position.z,
            };
            tree.position = position;
            live.economy
                .economy
                .authored_tree_mut(tree.source_entity_index)
                .expect("authored Tree retains its authoritative carrier")
                .position = position;
            assert!(
                tree.source_entity_index > house.entity_index,
                "House must visit before Tree"
            );
            crate::world::effects::sync_position(
                &mut live.economy,
                &mut live.terrain,
                tree.source_entity_index,
                tree.position,
                true,
                false,
            )
            .expect("publish the Tree map head");
            let owner = live.core.state.entity_index;
            assert_ne!(owner, tree.entity_id);
            let house = live.ordinary.house_mut(house.entity_index).unwrap();
            house.source_id = owner;
            house.action = crate::house::HOUSE_TERRAIN_TO_LIVE_ACTION;
            house.max_life = 2;
            house.life = 2;
            house.pending_damage = 0;
            house.pending_attacker = 0;
            house.publish_source(&mut live.economy.entities[usize::from(house.entity_index)]);
            (
                house.entity_index,
                tree.source_entity_index,
                live.economy
                    .economy
                    .slot_generation(tree.source_entity_index),
                live.economy.map_chains.tile_of(tree.source_entity_index),
                tree.phase,
            )
        };
        simulation.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert!(
            !simulation
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "scheduled House BUILD must not roll back: {:?}",
            simulation.dispatch_events()
        );
        assert!(
            simulation
                .dispatch_events()
                .iter()
                .any(|event| matches!(event,
                    crate::native_world::Mc2DispatchEvent::House { source_slot, events, .. }
                        if *source_slot == house_slot && events.iter().any(|event| matches!(event,
                            crate::house::HouseStepEvent::ConstructionPass { first_pass: true, .. }
                        ))
                )),
            "ordinary House BUILD must produce its first construction pass"
        );
        let live = &simulation.session.state.live;
        let tree = live
            .economy
            .authored_trees()
            .iter()
            .find(|tree| tree.source_entity_index == tree_slot)
            .expect("same-pass hidden Tree remains allocated");
        assert!(tree.active && !tree.drawing_enabled);
        assert_eq!(
            tree.phase,
            phase_before.wrapping_add(1),
            "hidden Tree still runs its later source callback and tail"
        );
        assert!(live.economy.map_chains.is_linked(tree_slot));
        simulation.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert!(
            !simulation
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. }))
        );
        let live = &simulation.session.state.live;
        assert!(
            live.economy
                .authored_trees()
                .iter()
                .all(|tree| tree.source_entity_index != tree_slot
                    || !tree.active
                    || live.economy.economy.slot_generation(tree_slot) != tree_generation)
        );
        assert_eq!(
            live.terrain.planes().entity_indices[usize::from(tree_tile)] as u16,
            live.economy.map_chains.tile_head(tree_tile),
            "scheduled BUILD Tree reclaim publishes the terrain map head"
        );
    }
}
