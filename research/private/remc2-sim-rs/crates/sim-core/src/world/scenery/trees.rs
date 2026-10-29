//! Source Tree visibility and following-pass map removal.

use crate::{
    FlightWorld, Position,
    campaign::source_arena::SourceArena,
    native_world::{Level2AuthoredTreeSource, SessionTerrain, source_target},
};

/// Run AddTree02_00 against the source arena's frozen allocation cursor.
/// A successful NewEvent may replace the very address being visited; all
/// remaining writes then target its new occupant, as in the source.
pub(crate) fn step(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    context: source_target::Context<'_>,
    slot: u16,
) -> Result<bool, String> {
    let Some(tree) = arena.economy.authored_tree_mut(slot) else {
        return Ok(false);
    };
    tree.control_flags |= 0x0002_0000;
    let had_owner = tree.owner_entity_index != 0;
    if had_owner {
        tree.life = tree.life.wrapping_sub(tree.pending_damage);
    }
    let lethal = had_owner && tree.life < 0;
    let position = tree.position;

    if lethal && let Some(sphere_slot) = arena.allocate_source_slot_with_terrain(terrain) {
        // The source passes &tree.position. NewEvent clears that address
        // before the constructor reads it when the selected address aliases.
        let position = if sphere_slot == slot {
            Position { x: 0, y: 0, z: 0 }
        } else {
            position
        };
        if let Some(sphere) = arena
            .economy
            .initialize_authored_model39(sphere_slot, slot, position)
        {
            crate::world::effects::sync_position(
                arena,
                terrain,
                sphere_slot,
                sphere.position,
                true,
                false,
            )?;
            if sphere_slot == slot {
                let owner_id = arena.entities[0].source_id;
                let sphere = arena
                    .economy
                    .spheres_mut()
                    .iter_mut()
                    .find(|sphere| sphere.active && sphere.source_entity_index == slot)
                    .ok_or("Tree replacement lost its current sphere")?;
                sphere.id = u32::from(owner_id);
                sphere.vertical_velocity = ((i32::from(sphere.collision_fov) * 3) >> 2) as i16;
                sphere.position.z = sphere.position.z.saturating_sub(128).max(0);
                sphere.entity_rng = sphere.entity_rng.wrapping_mul(9_377).wrapping_add(9_439);
                sphere.life = i32::from(sphere.entity_rng % 60) + 130;
                sphere.control_flags = (sphere.control_flags & !0x0002_0008) | 0x0002_0000;
                sphere.action = 1;
                let rng = sphere.entity_rng;
                arena.economy.set_source_rng(slot, rng);
            } else {
                let tree = arena
                    .economy
                    .authored_tree_mut(slot)
                    .ok_or("Tree disappeared while allocating its sphere")?;
                let owner = tree.owner_entity_index;
                let fov = tree.fov;
                let owner_id = source_target::read(arena, context, owner)?
                    .ok_or("Tree damage owner is outside the source arena")?
                    .source_id;
                let sphere = arena
                    .economy
                    .spheres_mut()
                    .iter_mut()
                    .find(|sphere| sphere.active && sphere.source_entity_index == sphere_slot)
                    .ok_or("Tree sphere constructor lost its carrier")?;
                sphere.id = u32::from(owner_id);
                sphere.vertical_velocity = ((i32::from(fov) * 3) >> 2) as i16;
                sphere.position.z = sphere.position.z.saturating_sub(128).max(0);
                let sphere_z = sphere.position.z;
                let tree = arena
                    .economy
                    .authored_tree_mut(slot)
                    .ok_or("Tree disappeared before sphere tail")?;
                tree.last_sphere_source_entity_index = sphere_slot;
                tree.last_sphere_word_0x2c = ((i32::from(fov) * 3) >> 2) as i16;
                tree.last_sphere_position_z = sphere_z;
                tree.entity_rng = tree.entity_rng.wrapping_mul(9_377).wrapping_add(9_439);
                tree.life = i32::from(tree.entity_rng % 60) + 130;
                tree.control_flags = (tree.control_flags & !0x0002_0008) | 0x0002_0000;
                tree.action = 1;
                let life = tree.life;
                arena
                    .economy
                    .spheres_mut()
                    .iter_mut()
                    .find(|sphere| sphere.active && sphere.source_entity_index == sphere_slot)
                    .ok_or("Tree sphere disappeared before life write")?
                    .life = life;
            }
            let position = if sphere_slot == slot {
                arena
                    .economy
                    .spheres_mut()
                    .iter_mut()
                    .find(|sphere| sphere.active && sphere.source_entity_index == slot)
                    .ok_or("Tree replacement disappeared before relink")?
                    .position
            } else {
                arena
                    .economy
                    .authored_tree_mut(slot)
                    .ok_or("Tree disappeared before relink")?
                    .position
            };
            // sub_57D40 unlinks and re-adds even at the same coordinates.
            crate::world::effects::sync_position(arena, terrain, slot, position, true, true)?;
        }
    }

    if let Some(tree) = arena.economy.authored_tree_mut(slot) {
        if had_owner {
            tree.owner_entity_index = 0;
        }
        tree.position.z = terrain.ground_height(tree.position);
    } else if let Some(sphere) = arena
        .economy
        .spheres_mut()
        .iter_mut()
        .find(|sphere| sphere.active && sphere.source_entity_index == slot)
    {
        if had_owner {
            sphere.ordinary_damage_owner = 0;
        }
        sphere.position.z = terrain.ground_height(sphere.position);
    } else {
        return Err("Tree callback lost its current source occupant".to_owned());
    }

    let position = arena
        .economy
        .authored_tree_mut(slot)
        .map(|tree| tree.position)
        .or_else(|| {
            arena
                .economy
                .spheres_mut()
                .iter_mut()
                .find(|sphere| sphere.active && sphere.source_entity_index == slot)
                .map(|sphere| sphere.position)
        })
        .ok_or("Tree callback has no current position")?;
    if terrain.material_mask(position) == crate::WATER_AMBIENCE_MASK {
        if let Some(splash_slot) = arena.allocate_source_slot_with_terrain(terrain) {
            let position = if splash_slot == slot {
                Position { x: 0, y: 0, z: 0 }
            } else {
                position
            };
            if let Some(splash) = arena.economy.initialize_authored_model5_water_splash(
                splash_slot,
                position,
                terrain,
            ) {
                crate::world::effects::sync_position(
                    arena,
                    terrain,
                    splash_slot,
                    splash.position,
                    true,
                    false,
                )?;
                let id = if splash_slot == slot {
                    splash.entity_id
                } else if let Some(tree) = arena.economy.authored_tree_mut(slot) {
                    tree.entity_id
                } else {
                    arena
                        .economy
                        .spheres_mut()
                        .iter_mut()
                        .find(|sphere| sphere.active && sphere.source_entity_index == slot)
                        .map(|sphere| sphere.id as u16)
                        .ok_or("Tree water source disappeared")?
                };
                arena
                    .economy
                    .authored_model5_water_splash_mut(splash_slot)
                    .ok_or("Tree water splash lost its carrier")?
                    .entity_id = id;
            }
        }
        if let Some(tree) = arena.economy.authored_tree_mut(slot) {
            tree.control_flags |= crate::level1_mission::SOURCE_DISABLED_FLAG;
            tree.drawing_enabled = false;
        } else if let Some(splash) = arena.economy.authored_model5_water_splash_mut(slot) {
            splash.control_flags |= crate::level1_mission::SOURCE_DISABLED_FLAG;
            splash.drawing_enabled = false;
        } else if let Some(sphere) = arena
            .economy
            .spheres_mut()
            .iter_mut()
            .find(|sphere| sphere.active && sphere.source_entity_index == slot)
        {
            sphere.control_flags |= crate::level1_mission::SOURCE_DISABLED_FLAG;
            sphere.pending_delete = true;
        } else {
            return Err("Tree water tail lost its current occupant".to_owned());
        }
    }

    if let Some(tree) = arena.economy.authored_tree_mut(slot) {
        tree.phase = tree.phase.wrapping_add(1);
        let rng = tree.entity_rng;
        arena.economy.set_source_rng(slot, rng);
    } else if let Some(splash) = arena.economy.authored_model5_water_splash_mut(slot) {
        splash.phase = splash.phase.wrapping_add(1);
    } else if let Some(sphere) = arena
        .economy
        .spheres_mut()
        .iter_mut()
        .find(|sphere| sphere.active && sphere.source_entity_index == slot)
    {
        sphere.phase = sphere.phase.wrapping_add(1);
    } else {
        return Err("Tree epilogue lost its current occupant".to_owned());
    }
    Ok(true)
}

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
