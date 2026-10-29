//! Source Tree visibility and following-pass map removal.

use crate::{
    campaign::source_arena::SourceArena,
    native_world::{Level2AuthoredTreeSource, SessionTerrain},
};

/// `sub_57390` hides a non-owner Tree but leaves it live until the next pass.
pub(crate) fn hide_for_terrain(arena: &mut SourceArena, slot: u16, owner: u16) -> bool {
    arena.economy.hide_tree_for_terrain(slot, owner)
}

pub(crate) fn hide_tree(tree: &mut Level2AuthoredTreeSource, owner: u16) {
    if tree.entity_id != owner {
        tree.control_flags |= crate::level1_mission::SOURCE_DISABLED_FLAG;
        tree.drawing_enabled = false;
    }
}

/// Publish the old tile's head before the economy releases its Tree slot.
pub(crate) fn prepare_reclaim(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
) -> Result<(), String> {
    let hidden = arena
        .economy
        .authored_trees()
        .iter()
        .filter(|tree| {
            !tree.active
                || !tree.drawing_enabled
                || tree.control_flags & crate::level1_mission::SOURCE_DISABLED_FLAG != 0
        })
        .map(|tree| (tree.source_entity_index, tree.position))
        .collect::<Vec<_>>();
    for (slot, position) in hidden {
        crate::world::effects::sync_position(arena, terrain, slot, position, false, false)?;
        arena.entities[usize::from(slot)].reclaim_source_row();
    }
    Ok(())
}
