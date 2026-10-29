//! Source Tree visibility and following-pass map removal.

use crate::{
    FlightWorld, Position,
    campaign::source_arena::SourceArena,
    native_world::{Level2AuthoredTreeSource, ParticleParameter, SessionTerrain, source_target},
};

/// Run AddTree02_00 against the source arena's frozen allocation cursor.
/// A successful NewEvent may replace the very address being visited; all
/// remaining writes then target its new occupant, as in the source.
pub(crate) fn step(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    context: source_target::Context<'_>,
    particles: &[ParticleParameter],
    slot: u16,
) -> Result<bool, String> {
    let Some(tree) = arena.economy.authored_tree_mut(slot) else {
        return Ok(false);
    };
    let had_owner = match tree.action {
        0 => {
            tree.control_flags |= 0x0002_0000;
            let has_owner = tree.owner_entity_index != 0;
            if has_owner {
                tree.life = tree.life.wrapping_sub(tree.pending_damage);
            }
            has_owner
        }
        1 => {
            let life = tree.life.wrapping_sub(1);
            if life < 60 {
                let particle = match tree.particle_index {
                    83 => Some(226),
                    84 => Some(227),
                    _ => None,
                };
                if let Some(index) = particle {
                    let binding = particles
                        .get(usize::from(index))
                        .ok_or_else(|| format!("Tree particle {index} is not bound"))?;
                    tree.particle_index = index;
                    tree.yaw = (binding.rotation_speed / 2) as i16;
                    tree.fov = tree.yaw;
                    tree.pitch = (binding.speed / 2) as i16;
                    tree.roll = tree.pitch;
                }
                tree.action = 2;
            }
            tree.life = life;
            false
        }
        2 => false,
        action => {
            return Err(format!(
                "Tree source slot {slot} has unsupported action {action}"
            ));
        }
    };
    let lethal = had_owner && tree.life < 0;
    let position = tree.position;

    if lethal && let Some(child_slot) = arena.allocate_source_slot_with_terrain(terrain) {
        // The source passes &tree.position. NewEvent clears that address
        // before the constructor reads it when the selected address aliases.
        let position = if child_slot == slot {
            Position { x: 0, y: 0, z: 0 }
        } else {
            position
        };
        if let Some(child) = arena
            .economy
            .initialize_authored_model6(child_slot, slot, position, terrain)
        {
            crate::world::effects::sync_position(
                arena,
                terrain,
                child_slot,
                child.position,
                true,
                false,
            )?;
            crate::world::effects::lighting::admit_source(
                arena,
                terrain,
                child_slot,
                child.position,
                crate::world::effects::lighting::SourceLight::GroundFire,
            );
            if child_slot == slot {
                let owner_id = arena.entities[0].source_id;
                let child = arena
                    .economy
                    .authored_model6_mut(slot)
                    .ok_or("Tree replacement lost its current model-6 child")?;
                child.entity_id = owner_id;
                child.height_offset = ((i32::from(child.shift_rot_y) * 3) >> 2) as i16;
                child.position.z = child.position.z.saturating_sub(128).max(0);
                child.entity_rng = child.entity_rng.wrapping_mul(9_377).wrapping_add(9_439);
                child.life = (child.entity_rng % 60 + 130) as i16;
                child.control_flags = (child.control_flags & !0x0002_0008) | 0x0002_0000;
                child.action = 1;
                let rng = child.entity_rng;
                arena.economy.set_source_rng(slot, rng);
            } else {
                let tree = arena
                    .economy
                    .authored_tree_mut(slot)
                    .ok_or("Tree disappeared while allocating its child")?;
                let owner = tree.owner_entity_index;
                let fov = tree.fov;
                let owner_id = source_target::read(arena, context, owner)?
                    .ok_or("Tree damage owner is outside the source arena")?
                    .source_id;
                let child = arena
                    .economy
                    .authored_model6_mut(child_slot)
                    .ok_or("Tree model-6 constructor lost its carrier")?;
                child.entity_id = owner_id;
                child.height_offset = (i32::from(fov) * 3 / 4) as i16;
                child.position.z = child.position.z.saturating_sub(128).max(0);
                let child_z = child.position.z;
                let tree = arena
                    .economy
                    .authored_tree_mut(slot)
                    .ok_or("Tree disappeared before child tail")?;
                tree.last_sphere_source_entity_index = child_slot;
                tree.last_sphere_word_0x2c = (i32::from(fov) * 3 / 4) as i16;
                tree.last_sphere_position_z = child_z;
                tree.entity_rng = tree.entity_rng.wrapping_mul(9_377).wrapping_add(9_439);
                tree.life = i32::from(tree.entity_rng % 60) + 130;
                tree.control_flags = (tree.control_flags & !0x0002_0008) | 0x0002_0000;
                tree.action = 1;
                let life = tree.life;
                arena
                    .economy
                    .authored_model6_mut(child_slot)
                    .ok_or("Tree child disappeared before life write")?
                    .life = life as i16;
            }
            let position = if child_slot == slot {
                arena
                    .economy
                    .authored_model6_mut(slot)
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
    } else if let Some(child) = arena.economy.authored_model6_mut(slot) {
        // A self-aliased NewEvent cleared the old damage mailbox.
        child.position.z = terrain.ground_height(child.position);
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
                .authored_model6_mut(slot)
                .map(|child| child.position)
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
                        .authored_model6_mut(slot)
                        .map(|child| child.entity_id)
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
        } else if let Some(child) = arena.economy.authored_model6_mut(slot) {
            child.control_flags |= crate::level1_mission::SOURCE_DISABLED_FLAG;
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
    } else if let Some(child) = arena.economy.authored_model6_mut(slot) {
        child.phase = child.phase.wrapping_add(1);
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
