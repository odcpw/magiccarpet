//! The terrain family's source callback composition. Common phase advancement
//! and following-pass reclaim remain in the shared source scheduler.

use super::{AreaDamage, ribbon, travelling, tremor, volcano};
use crate::{
    Position,
    campaign::source_arena::SourceArena,
    level_definition::{EffectKind, TerrainDisasterDefinition, TerrainDisasterKind},
    native_world::{ParticleParameter, SessionTerrain},
};

/// Direct constructors8/28/29/31/50 and61..64. Authored linked paths use their
/// real chain producers instead of calling these terminal marker factories.
pub(crate) fn construct_marker(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    model: u8,
) -> Result<Option<u16>, String> {
    // sub_4E750 is an enabled source call whose entire body returns null.
    // In particular, it consumes neither a source slot nor a random draw.
    if model == 8 {
        return Ok(None);
    }
    let (action, life, hidden) = match model {
        28 => (30, 0, false),
        29 => (31, 0, false),
        31 => (33, 0, false),
        50 => (54, 0, false),
        61 => (66, 52, true),
        62 => (67, 52, true),
        63 => (68, 1, true),
        64 => (69, 1, true),
        _ => return Err(format!("terrain marker model {model} has no constructor")),
    };
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut row = crate::world::terrain::cave_features::new_event(
        slot,
        arena.economy.new_event_rng(slot),
        position,
    );
    row.class = 10;
    row.model = model;
    row.record[67] = 10;
    row.record[68] = model;
    row.set_source_action(action);
    row.set_max_life(life);
    row.set_life(life);
    row.set_control_flags((row.control_flags & !8) | 4 | u32::from(hidden));
    if let Err(error) =
        arena
            .economy
            .map_chains
            .sync_entity(crate::renderable_map_chain::RenderableMapSeed {
                entity_index: slot,
                active: true,
                x: position.x,
                y: position.y,
            })
    {
        arena.economy.release_slot(slot, Some(row.entity_rng));
        return Err(error);
    }
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::terrain::cave_features::synchronize_links(arena, terrain, slot);
    Ok(Some(slot))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DamageRequest {
    Rectangle(AreaDamage),
    Projectile(AreaDamage),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Outcome {
    pub(crate) sounds: Vec<u16>,
    pub(crate) terrain_mutated: bool,
}

/// Materialize the published semantic properties through the real source
/// constructor. Linked authored chains set route fields after construction.
pub(crate) fn construct_defined(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    definition: TerrainDisasterDefinition,
) -> Result<Option<u16>, String> {
    let kind = definition.kind();
    let slot = match kind {
        TerrainDisasterKind::Volcano => volcano::construct(arena, terrain, position),
        TerrainDisasterKind::Impact => {
            travelling::construct(arena, terrain, position, travelling::Kind::Impact)
        }
        TerrainDisasterKind::LoweringRing => {
            super::construct(arena, terrain, position, definition.into())
        }
        TerrainDisasterKind::Earthquake => {
            travelling::construct(arena, terrain, position, travelling::Kind::Earthquake)
        }
        TerrainDisasterKind::RaisedRibbon => ribbon::construct_at(arena, terrain, position)?,
        TerrainDisasterKind::MarkedPath => super::paths::construct_at(arena, terrain, position)?,
        TerrainDisasterKind::CraterTrail => {
            travelling::construct(arena, terrain, position, travelling::Kind::CraterTrail)
        }
        TerrainDisasterKind::RisingTrail => {
            travelling::construct(arena, terrain, position, travelling::Kind::RisingTrail)
        }
        TerrainDisasterKind::Tremor => tremor::construct(arena, terrain, position)?,
    };
    if let Some(slot) = slot {
        let source = &mut arena.entities[usize::from(slot)];
        source.set_max_life(definition.max_life());
        source.set_life(definition.lifetime());
        if kind != TerrainDisasterKind::RaisedRibbon {
            // Model27's same packed word holds its altitude-derived target,
            // rather than an actor damage payload.
            source.set_subspell_index(definition.damage());
        }
        source.record[88..90].copy_from_slice(&definition.half_width().to_le_bytes());
        source.record[90..92].copy_from_slice(&definition.half_width().to_le_bytes());
        source.record[92..94].copy_from_slice(&definition.half_height().to_le_bytes());
    }
    Ok(slot)
}

/// sub_5B070 follows the real map-chain order, with subtract128 rounding.
/// Source63 writes life2 (restore); source64 writes life1 (seal).
pub(crate) fn set_controller_life_at(
    arena: &mut SourceArena,
    position: Position,
    life: i32,
) -> Option<u16> {
    let x = (position.x.wrapping_sub(128) >> 8) as u8;
    let y = (position.y.wrapping_sub(128) >> 8) as u8;
    let target = arena
        .economy
        .map_chains
        .entity_chain(x, y)
        .into_iter()
        .find(|slot| {
            let row = &arena.entities[usize::from(*slot)];
            row.active && row.class == 14 && matches!(row.model, 1 | 2)
        })?;
    arena.entities[usize::from(target)].set_life(life);
    Some(target)
}

pub(crate) fn step_trigger(arena: &mut SourceArena, slot: u16) -> Result<(), String> {
    let source = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && row.class == 10)
        .ok_or("terrain trigger source row is absent")?;
    let life = match (source.model, source.action) {
        (63, 68) => 2,
        (64, 69) => 1,
        selector => return Err(format!("unsupported terrain trigger {selector:?}")),
    };
    let position = source.position;
    set_controller_life_at(arena, position, life);
    arena.entities[usize::from(slot)].disable_drawing();
    Ok(())
}

pub(crate) fn supports(class: u8, model: u8, action: u8) -> bool {
    class == 10
        && matches!(
            (model, action),
            (9, 9)
                | (10, 10)
                | (11, 11)
                | (15, 15)
                | (27, 27..=29)
                | (28, 30)
                | (29, 31)
                | (30, 32)
                | (31, 33)
                | (32, 34)
                | (50, 54)
                | (51, 55)
                | (61, 66)
                | (62, 67)
                | (63, 68)
                | (64, 69)
                | (71, 78)
        )
}

pub(crate) fn step<F>(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    alternate_volcano: bool,
    mut damage: F,
) -> Result<Outcome, String>
where
    F: FnMut(&mut SourceArena, DamageRequest) -> Result<u16, String>,
{
    let source = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && supports(row.class, row.model, row.action))
        .ok_or("terrain callback has no supported live source row")?;
    let mut outcome = Outcome::default();
    match source.action {
        9 => {
            let visit = volcano::step(
                arena,
                terrain,
                slot,
                alternate_volcano,
                |arena, request| damage(arena, DamageRequest::Rectangle(request)),
                |arena, terrain, request| {
                    if request.alternate {
                        return Err(
                            "final-volcano eruption91 requires the final encounter constructor"
                                .into(),
                        );
                    }
                    let child = crate::world::effects::allocate(
                        arena,
                        terrain,
                        sim_definition::adapter::mc2_effect_definition(EffectKind::Eruption),
                        request.position,
                        particles,
                    )?;
                    if let Some(slot) = child {
                        let row = &mut arena.entities[usize::from(slot)];
                        row.source_id = request.owner;
                        row.record[30..32].copy_from_slice(&request.owner.to_le_bytes());
                    }
                    Ok(child)
                },
            )?;
            outcome.terrain_mutated = visit.terrain_mutated;
            if visit.rumble {
                outcome.sounds.push(10);
            }
            if visit.alternate_rumble {
                outcome.sounds.push(63);
            }
        }
        11 => {
            let visit = super::step(arena, terrain, slot, |arena, request| {
                damage(arena, DamageRequest::Rectangle(request))
            })?;
            outcome.terrain_mutated = visit.terrain_mutated;
            outcome.sounds.extend(visit.sound_id);
        }
        10 | 15 | 34 | 55 => {
            let sequence = terrain.mutation_sequence();
            let visit = travelling::step(arena, terrain, slot, |arena, request| {
                damage(arena, DamageRequest::Projectile(request))
            })?;
            outcome.terrain_mutated = terrain.mutation_sequence() != sequence;
            outcome.sounds.extend(visit.sound_id);
        }
        27..=29 => {
            ribbon::step(arena, terrain, slot)?;
            outcome.terrain_mutated = true;
        }
        32 => {
            let sequence = terrain.mutation_sequence();
            super::paths::step(arena, terrain, slot)?;
            outcome.terrain_mutated = terrain.mutation_sequence() != sequence;
        }
        78 => {
            let visit = tremor::step(arena, terrain, slot, |arena, pulse| {
                outcome.sounds.push(10);
                damage(
                    arena,
                    DamageRequest::Projectile(AreaDamage {
                        source_slot: pulse.source_slot,
                        position: pulse.position,
                        owner: pulse.owner,
                        payload: pulse.payload,
                        half_width: pulse.half_width,
                        half_depth: pulse.half_depth,
                        vertical_offset: pulse.vertical_offset,
                        half_height: pulse.half_height,
                        experience_spell: Some(15),
                    }),
                )
            })?;
            outcome.terrain_mutated = visit.terrain_mutated;
        }
        // These four direct constructors are terminal markers. Authored
        // chains instead construct model27/30/32/51 descendants at birth.
        30 | 31 | 33 | 54 => arena.entities[usize::from(slot)].disable_drawing(),
        // The source pre_sub action66/67 cases are explicit empty callbacks;
        // life52 is retained and common scheduler phase still advances.
        66 | 67 => {}
        68 | 69 => step_trigger(arena, slot)?,
        action => return Err(format!("unsupported terrain source action {action}")),
    }
    Ok(outcome)
}
