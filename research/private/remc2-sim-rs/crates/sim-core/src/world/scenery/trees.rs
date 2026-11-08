//! Source Tree visibility and following-pass map removal.

use crate::{
    FlightWorld, Position,
    campaign::source_arena::SourceArena,
    native_world::{Level2AuthoredTreeSource, ParticleParameter, SessionTerrain, source_target},
};

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct GroundFireBirth {
    pub(super) parent: crate::campaign::source_arena::SourceEntityHandle,
    pub(super) child: crate::campaign::source_arena::SourceEntityHandle,
    pub(super) position: Position,
}

#[cfg(test)]
thread_local! {
    static GROUND_FIRE_BIRTH_TRACE: std::cell::RefCell<Option<(Vec<crate::campaign::source_arena::SourceEntityHandle>, Vec<GroundFireBirth>)>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn start_ground_fire_birth_trace(parent: crate::campaign::source_arena::SourceEntityHandle) {
    start_ground_fire_birth_trace_many(&[parent]);
}

#[cfg(test)]
fn take_ground_fire_birth_trace() -> Option<GroundFireBirth> {
    take_ground_fire_birth_trace_many().into_iter().next()
}

#[cfg(test)]
pub(super) fn start_ground_fire_birth_trace_many(
    parents: &[crate::campaign::source_arena::SourceEntityHandle],
) {
    GROUND_FIRE_BIRTH_TRACE
        .with(|trace| *trace.borrow_mut() = Some((parents.to_vec(), Vec::new())));
}

#[cfg(test)]
pub(super) fn take_ground_fire_birth_trace_many() -> Vec<GroundFireBirth> {
    GROUND_FIRE_BIRTH_TRACE.with(|trace| {
        trace
            .borrow_mut()
            .take()
            .map_or_else(Vec::new, |(_, births)| births)
    })
}

/// Present the current typed Tree occupant through the ordinary sprite path.
pub(crate) fn sprite_sources(
    trees: &[Level2AuthoredTreeSource],
) -> impl Iterator<Item = (u16, u16, u16, i16, Position, u32)> + '_ {
    trees
        .iter()
        .filter(|tree| tree.active && tree.drawing_enabled && tree.control_flags & 0x421 == 0)
        .map(|tree| {
            (
                tree.source_entity_index,
                tree.entity_id,
                tree.particle_index,
                // AddTree and its callbacks leave the source sprite heading
                // at NewEvent's zero; `tree.yaw` is the collision array yaw.
                0,
                tree.position,
                tree.control_flags,
            )
        })
}

/// Run the same Tree actions for reset-imported source rows. Allocation can
/// reuse the visited address, so subsequent writes always reread that address.
pub(crate) fn step_imported(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    context: source_target::Context<'_>,
    particles: &[ParticleParameter],
    slot: u16,
) -> Result<(), String> {
    use crate::level_definition::EffectKind;
    use crate::world::effects::{allocate_from_source_position, bind_particle, sync_position};

    #[cfg(test)]
    let trace_target = arena
        .handle(slot)
        .map(|target| (target, arena.economy.events().len()));
    #[cfg(test)]
    let mut consumed = None;
    let tree = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|tree| tree.active && tree.class == 2 && tree.model == 0)
        .ok_or_else(|| format!("imported Tree callback lost source slot {slot}"))?;
    let had_owner = match tree.action {
        0 => {
            tree.set_control_flags(tree.control_flags | 0x0002_0000);
            let owner = u16::from_le_bytes([tree.record[102], tree.record[103]]);
            if owner != 0 {
                let damage = i32::from_le_bytes([
                    tree.record[98],
                    tree.record[99],
                    tree.record[100],
                    tree.record[101],
                ]);
                #[cfg(test)]
                let before = tree.life;
                tree.set_life(tree.life.wrapping_sub(damage));
                #[cfg(test)]
                {
                    consumed = Some(((damage, owner), before, tree.life));
                }
            }
            owner != 0
        }
        1 => {
            tree.set_life(tree.life.wrapping_sub(1));
            if tree.life < 60 {
                let particle = match tree.particle_parameter_index {
                    Some(83) => Some(226),
                    Some(84) => Some(227),
                    _ => None,
                };
                tree.set_source_action(2);
                if let Some(particle) = particle {
                    bind_particle(tree, particle, particles)?;
                }
            }
            false
        }
        2 => false,
        action => {
            return Err(format!(
                "imported Tree slot {slot} has unsupported action {action}"
            ));
        }
    };
    let lethal = had_owner && tree.life < 0;
    let position = tree.position;
    if lethal
        && let Some(child_slot) = allocate_from_source_position(
            arena,
            terrain,
            sim_definition::adapter::mc2_effect_definition(EffectKind::GroundFire),
            slot,
            position,
            particles,
        )?
    {
        #[cfg(test)]
        if let (Some((parent, _)), Some(child)) = (trace_target, arena.handle(child_slot)) {
            let birth = GroundFireBirth {
                parent,
                child,
                position: arena.entities[usize::from(child_slot)].position,
            };
            GROUND_FIRE_BIRTH_TRACE.with(|trace| {
                if let Some((watched, result)) = trace.borrow_mut().as_mut()
                    && watched.contains(&parent)
                {
                    result.push(birth);
                }
            });
        }
        let current = &arena.entities[usize::from(slot)];
        let owner = u16::from_le_bytes([current.record[102], current.record[103]]);
        let owner_id = if owner == 0 {
            arena.entities[0].source_id
        } else {
            source_target::read(arena, context, owner)?
                .ok_or("imported Tree damage owner is outside the source arena")?
                .source_id
        };
        let height = (i32::from(current.array_fov()) * 3 / 4) as i16;
        let child = &mut arena.entities[usize::from(child_slot)];
        child.source_id = owner_id;
        child.record[30..32].copy_from_slice(&owner_id.to_le_bytes());
        child.set_source_word_2c(height);
        let mut child_position = child.position;
        child_position.z = child_position.z.saturating_sub(128).max(0);
        child.set_position(child_position);
        let current = &mut arena.entities[usize::from(slot)];
        let rng = current.entity_rng.wrapping_mul(9_377).wrapping_add(9_439);
        current.set_entity_rng(rng);
        let life = i32::from(rng % 60) + 130;
        current.set_life(life);
        current.set_control_flags((current.control_flags & !0x0002_0008) | 0x0002_0000);
        current.set_source_action(1);
        arena.economy.set_source_rng(slot, rng);
        arena.entities[usize::from(child_slot)].set_life(life);
        let position = arena.entities[usize::from(slot)].position;
        sync_position(arena, terrain, slot, position, true, true)?;
    }
    let current = &mut arena.entities[usize::from(slot)];
    if had_owner {
        current.record[102..104].fill(0);
    }
    #[cfg(test)]
    if let (Some((target, event_cursor)), Some((mailbox, life_before, life_after))) =
        (trace_target, consumed)
    {
        let writer = crate::native_world::area_damage::take_player_mailbox_writer(target, mailbox);
        crate::native_world::area_damage::trace_player_damage_consumption(
            crate::native_world::area_damage::PlayerDamageConsumptionReceipt {
                target,
                mailbox,
                writer,
                life_before,
                life_after,
                event_cursor,
            },
        );
    }
    let mut position = current.position;
    position.z = terrain.ground_height(position);
    current.set_position(position);
    if terrain.material_mask(position) == crate::WATER_AMBIENCE_MASK {
        if let Some(child_slot) = allocate_from_source_position(
            arena,
            terrain,
            sim_definition::adapter::mc2_effect_definition(EffectKind::Ash),
            slot,
            position,
            particles,
        )? {
            let id = arena.entities[usize::from(slot)].source_id;
            let child = &mut arena.entities[usize::from(child_slot)];
            child.source_id = id;
            child.record[30..32].copy_from_slice(&id.to_le_bytes());
        }
        arena.entities[usize::from(slot)].disable_drawing();
    }
    Ok(())
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::{
        CampaignSession,
        schedule::{CurrentOccupant, ScheduleReceipt},
        spec::CampaignSelection,
    };
    use crate::native_world::{Mc2DispatchEvent, area_damage};

    #[test]
    fn ordinary_level_three_paid_fireballs_burn_original_tree_and_leave_stump() {
        let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3");
        let authored = crate::level1_mission::parse_authored_features(
            &std::fs::read(package.join("authored-level.bin")).unwrap(),
        )
        .unwrap();
        let feature = &authored[14];
        assert_eq!(
            (
                feature.kind,
                feature.subtype,
                feature.activation_group,
                feature.tile_x,
                feature.tile_y
            ),
            (2, 0, 0, 100, 149)
        );
        let mut game = CampaignSession::open(package, CampaignSelection::LEVEL3)
            .expect("the whole ordinary Level3 owner package opens");
        game.simulation_mut()
            .apply_carried_spell_families(&[0], Some(0))
            .unwrap();
        let ui = crate::SOURCE_SPELL_INDEX_MAP
            .iter()
            .position(|family| *family == 0)
            .unwrap() as i8;
        game.step(
            crate::PlayerCommand {
                opcode: 31,
                argument_1: ui,
                argument_2: 0,
                ..Default::default()
            },
            0,
        );
        assert!(
            game.simulation()
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        let sim = game.simulation();
        let target_slot = 95_u16;
        let tree = sim.entities()[usize::from(target_slot)].clone();
        assert!(tree.active && (tree.class, tree.model, tree.action) == (2, 0, 0));
        assert_eq!(
            (tree.position.x >> 8, tree.position.y >> 8),
            (feature.tile_x, feature.tile_y)
        );
        assert_eq!((tree.max_life, tree.life), (300, 300));
        assert_ne!(tree.control_flags & 8, 0);
        assert_ne!(tree.record_bytes()[60] & 1, 0);
        assert_ne!(
            sim.canonical_terrain().material_mask(tree.position),
            crate::WATER_AMBIENCE_MASK
        );
        let original_particle = tree.particle_parameter_index.unwrap();
        assert!([83, 84].contains(&original_particle));
        let target = sim.source_arena().handle(target_slot).unwrap();
        let player = sim.state.entity_index;
        let player_handle = sim.source_arena().handle(player).unwrap();
        let owner_id = sim.entities()[usize::from(player)].source_id;
        let holder = sim.shared_spells().local_player().enabled_spell_holders[0];
        let holder_handle = sim.source_arena().handle(holder).unwrap();
        let cost = crate::level_definition::FIREBALL_PROPERTIES.tiers[0].mana_cost;
        let holder_row = sim
            .shared_spells()
            .pickups()
            .iter()
            .chain(sim.shared_spells().holders())
            .chain(sim.shared_spells().dynamic_pickups())
            .find(|row| row.index == holder)
            .unwrap();
        assert_eq!(
            (
                holder_row.level_gate,
                holder_row.max_mana,
                holder_row.mana_regen
            ),
            (0, cost, 0)
        );
        assert!(sim.shared_spells().player_entity().mana >= 2 * cost);
        assert!(
            sim.campaign_entity_presentations(sim.state.yaw)
                .unwrap()
                .iter()
                .any(|dto| dto.entity_index == target_slot
                    && dto.render_parameter_index == original_particle)
        );
        // One fixed, declared player-only overhead firing line. The target
        // pose, health, actor RNG, callback cadence and all other actors stay live.
        let centre = Position {
            z: tree
                .position
                .z
                .wrapping_add(tree.array_yaw())
                .wrapping_sub(crate::level1_mission::FIREBALL_COLLISION_VERTICAL_OFFSET),
            ..tree.position
        };
        let hand = Position {
            z: centre.z.wrapping_add(1024),
            ..centre
        };
        let aim = crate::level1_mission::projectile_guidance_angles(hand, centre);
        let mut origin = Position {
            z: hand.z.wrapping_sub(sim.state.entity_fov),
            ..hand
        };
        let hand_yaw = aim
            .0
            .wrapping_sub(sim.shared_spells().beyond_sight_auxiliary().next_entity)
            .wrapping_sub(512);
        crate::move_entity(&mut origin, hand_yaw.wrapping_add(1024), 0, 256);
        assert!(
            origin.z
                >= sim
                    .canonical_terrain()
                    .ground_height(origin)
                    .wrapping_add(sim.profile().ground_clearance)
        );
        assert!(!sim.canonical_terrain().cave_collision_blocked(origin));
        assert_eq!(
            crate::resolve_legacy_movement(
                &mut crate::FlightState::default(),
                origin,
                origin,
                sim.profile(),
                sim.canonical_terrain(),
            ),
            crate::MovementResolution::Accept(origin),
            "the real terrain admits the fixed firing pose"
        );
        let target_view = source_target::read(
            sim.source_arena(),
            source_target::Context::default(),
            target_slot,
        )
        .unwrap()
        .unwrap();
        let shape = target_view
            .collision
            .expect("real Tree has source collision axes");
        assert_eq!(
            (
                shape.half_width,
                shape.half_depth,
                shape.vertical_offset,
                shape.half_height
            ),
            (
                tree.array_pitch(),
                tree.array_roll(),
                tree.array_yaw(),
                tree.array_fov()
            )
        );
        assert!(shape.half_width > 0 && shape.half_depth > 0 && shape.half_height > 0);
        assert_eq!(
            (centre.x, centre.y),
            (target_view.position.x, target_view.position.y)
        );
        assert_eq!(
            i32::from(centre.z)
                + i32::from(crate::level1_mission::FIREBALL_COLLISION_VERTICAL_OFFSET),
            i32::from(target_view.position.z) + i32::from(shape.vertical_offset)
        );
        game.simulation_mut()
            .set_level2_entity_position(player, origin)
            .unwrap();
        let mut no_cast = game.clone();
        area_damage::start_actor_damage_trace(&[target]);
        let (mut launches, mut impacts, mut terminals, mut accepted, mut consumed) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut birth = None;
        let mut hidden_on_prior_turn: Option<crate::campaign::source_arena::SourceEntityHandle> =
            None;
        let (
            mut queued,
            mut paid,
            mut child_presented,
            mut child_hidden,
            mut child_reclaimed,
            mut stump_presented,
        ) = (0, 0, false, false, false, false);
        let mut elapsed = 0;
        for turn in 0..256 {
            let firing = turn == 0 || turn == 8;
            if firing {
                game.simulation_mut()
                    .set_level2_player_aim_for_test(aim.0, aim.1);
            }
            area_damage::start_fireball_launch_trace();
            crate::native_world::start_fireball_terminal_trace();
            area_damage::start_effect_damage_trace(target_slot);
            start_ground_fire_birth_trace(target);
            game.step(
                crate::PlayerCommand {
                    flags: if firing { 0x10 } else { 0 },
                    ..Default::default()
                },
                0,
            );
            elapsed += 1;
            let new_launches = area_damage::take_fireball_launch_trace();
            let launched = new_launches.iter().any(|(_, _, owner)| *owner == player);
            launches.extend(new_launches);
            impacts.extend(area_damage::take_fireball_impact_birth_trace());
            terminals.extend(crate::native_world::take_fireball_terminal_trace());
            accepted.extend(area_damage::take_effect_damage_trace());
            consumed.extend(area_damage::take_player_damage_consumptions());
            let newborn = take_ground_fire_birth_trace();
            if let Some(actual) = newborn {
                birth = Some(actual);
            }
            let sim = game.simulation();
            assert!(
                sim.dispatch_events()
                    .iter()
                    .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "actual Tree encounter turn{turn}: {:?}",
                sim.dispatch_events()
            );
            if let Some(hidden) = hidden_on_prior_turn.take() {
                let current = sim.source_arena().handle(hidden.source_slot);
                assert_ne!(
                    current,
                    Some(hidden),
                    "the actual hidden GroundFire is reclaimed on the following source boundary"
                );
                if current.is_none() {
                    assert!(
                        !sim.economy().map_chains().is_linked(hidden.source_slot),
                        "the reclaimed GroundFire no longer occupies the live map chain"
                    );
                }
                eprintln!(
                    "Tree95 GroundFire following reclaim: turn={turn} original={hidden:?} current={current:?} linked={}",
                    sim.economy().map_chains().is_linked(hidden.source_slot)
                );
                child_reclaimed = true;
            }
            if let Some(actual) = newborn {
                let visited = sim.cf10_production_schedule_receipts().iter().any(|receipt| matches!(
                    receipt, ScheduleReceipt::CallbackCompleted { slot, selector, .. }
                    if *slot == actual.child.source_slot && matches!(
                        &selector.registry_key().selector,
                        crate::campaign::spec::BehaviorSelector::ClassAction { class: 10, action: 6 }
                    )
                ));
                assert_eq!(
                    visited,
                    actual.child.source_slot > target_slot,
                    "the real numeric cursor owns the newborn's same-pass visit"
                );
                eprintln!(
                    "Tree95 GroundFire birth: turn={turn} parent={:?} child={:?} same_pass_callback={visited}",
                    actual.parent, actual.child
                );
            }
            for receipt in sim.cf10_production_schedule_receipts() {
                let ScheduleReceipt::EpilogueCompleted {
                    slot,
                    original,
                    current: CurrentOccupant::Known { selector, .. },
                    ..
                } = receipt
                else {
                    continue;
                };
                let (Some(before), Some(after)) = (
                    original.observed_player_resources(),
                    selector.observed_player_resources(),
                ) else {
                    continue;
                };
                if slot == holder
                    && launched
                    && sim.source_arena().handle(holder) == Some(holder_handle)
                    && before.0 == player_handle
                    && after
                        == (
                            player_handle,
                            before.1,
                            if before.2 >= 0 {
                                -cost
                            } else {
                                before.2.wrapping_sub(cost)
                            },
                            before.3,
                        )
                {
                    queued += 1;
                }
                if slot == player
                    && before.0 == player_handle
                    && after.0 == player_handle
                    && before.2 <= -cost
                    && after.1 == before.1.wrapping_add(before.2).clamp(0, before.3)
                    && before.1 - after.1 >= cost
                {
                    paid += 1;
                }
            }
            assert_eq!(sim.source_arena().handle(target_slot), Some(target));
            let current = &sim.entities()[usize::from(target_slot)];
            if current.action == 2 {
                let expected_particle = if original_particle == 83 { 226 } else { 227 };
                assert_eq!(current.particle_parameter_index, Some(expected_particle));
                stump_presented |= sim
                    .campaign_entity_presentations(sim.state.yaw)
                    .unwrap()
                    .iter()
                    .any(|dto| {
                        dto.entity_index == target_slot
                            && dto.render_parameter_index == expected_particle
                    });
            }
            if let Some(actual) = birth {
                if sim.source_arena().handle(actual.child.source_slot) == Some(actual.child) {
                    let child = &sim.entities()[usize::from(actual.child.source_slot)];
                    assert_eq!(
                        (child.class, child.model, child.action, child.source_id),
                        (10, 6, 6, owner_id)
                    );
                    assert_eq!(
                        child.source_word_2c(),
                        (i32::from(tree.array_fov()) * 3 / 4) as i16
                    );
                    if child.drawing_disabled() {
                        child_hidden = true;
                        hidden_on_prior_turn = Some(actual.child);
                    }
                    child_presented |= sim
                        .campaign_entity_presentations(sim.state.yaw)
                        .unwrap()
                        .iter()
                        .any(|dto| dto.entity_index == actual.child.source_slot);
                } else {
                    assert!(
                        child_reclaimed,
                        "GroundFire identity loss must follow an observed hidden packet"
                    );
                }
            }
            if stump_presented && child_presented && child_hidden && child_reclaimed && paid >= 2 {
                break;
            }
        }
        let own = launches
            .iter()
            .filter(|(_, _, owner)| *owner == player)
            .collect::<Vec<_>>();
        assert_eq!(
            own.len(),
            2,
            "exactly two public Fireball launches: {launches:?}"
        );
        let hits = consumed
            .iter()
            .filter(|hit| hit.target == target)
            .collect::<Vec<_>>();
        assert_eq!(
            hits.len(),
            2,
            "two actual original Tree mailbox consumptions: {consumed:?}"
        );
        let mut matched_projectiles = std::collections::BTreeSet::new();
        for (hit, before) in hits.iter().zip([300, 50]) {
            assert_eq!(
                (hit.mailbox, hit.life_before, hit.life_after),
                ((250, player), before, before - 250)
            );
            let writer = hit
                .writer
                .expect("the actual Tree subtraction retains its accepted writer");
            assert!(accepted.iter().any(|receipt| receipt.source_scoped
                && receipt.target_slot == target_slot
                && receipt.target_generation == target.generation
                && receipt.pulse.owner == player
                && receipt.pulse.payload == 250
                && receipt.writer_sequence == Some(writer.sequence)));
            let terminal = terminals
                .iter()
                .find(|terminal| {
                    own.iter().any(|(slot, generation, _)| {
                        terminal.projectile.source_slot == *slot
                            && terminal.projectile.generation == *generation
                    }) && terminal.target_handle == Some(target)
                        && terminal.impact.is_some_and(|impact| {
                            writer.source == area_damage::PlayerMailboxSource::Effect(impact)
                        })
                })
                .expect("the accepted hit comes from one actual Tree collision");
            assert!(
                matched_projectiles.insert((
                    terminal.projectile.source_slot,
                    terminal.projectile.generation
                )),
                "each paid shot must supply its own Tree hit"
            );
            assert!(impacts.iter().any(
                |(impact, generation, parent, parent_generation, owner)| {
                    *owner == player
                        && *parent == terminal.projectile.source_slot
                        && *parent_generation == terminal.projectile.generation
                        && writer.source
                            == area_damage::PlayerMailboxSource::Effect(
                                crate::campaign::source_arena::SourceEntityHandle {
                                    source_slot: *impact,
                                    generation: *generation,
                                },
                            )
                }
            ));
        }
        let birth = birth.expect("the actual lethal Tree callback constructs GroundFire");
        assert_eq!((birth.parent, birth.position), (target, tree.position));
        assert!(
            queued == 2
                && paid >= 2
                && child_presented
                && child_hidden
                && child_reclaimed
                && stump_presented,
            "paid={paid} queued={queued} fire={child_presented}/{child_hidden}/{child_reclaimed} stump={stump_presented}"
        );
        let stump = &game.simulation().entities()[usize::from(target_slot)];
        assert_eq!(
            (
                stump.class,
                stump.model,
                stump.action,
                stump.source_upkeep_word_62()
            ),
            (2, 0, 2, 0)
        );
        assert_eq!(
            game.simulation().source_arena().handle(target_slot),
            Some(target)
        );
        // TLS traces are generation-keyed, not session-keyed; the negative
        // world advances only after all positive receipts have been collected.
        for turn in 0..elapsed {
            if turn == 0 || turn == 8 {
                no_cast
                    .simulation_mut()
                    .set_level2_player_aim_for_test(aim.0, aim.1);
            }
            no_cast.step(crate::PlayerCommand::default(), 0);
            assert!(
                no_cast
                    .simulation()
                    .dispatch_events()
                    .iter()
                    .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
            );
        }
        assert_eq!(
            no_cast.simulation().source_arena().handle(target_slot),
            Some(target)
        );
        let untouched = &no_cast.simulation().entities()[usize::from(target_slot)];
        assert_eq!(
            (untouched.action, untouched.life, untouched.entity_rng),
            (0, tree.life, tree.entity_rng)
        );
    }
}
