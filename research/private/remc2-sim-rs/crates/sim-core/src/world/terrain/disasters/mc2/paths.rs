//! Source model30 path constructors and its single ordinary scheduled visit.

use crate::{
    Position,
    campaign::source_arena::SourceArena,
    native_world::{
        SessionTerrain,
        cave_features::{new_event, synchronize_links},
    },
    renderable_map_chain::RenderableMapSeed,
    world::terrain::disasters::paths::{self, Segment},
};

pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    segment: Segment,
    retained_z: i16,
) -> Result<Option<u16>, String> {
    // sub_48690 writes X/Y into predictedAxis and leaves its Z untouched.
    // The caller supplies that authoritative scratch position's retained Z.
    let position = Position {
        x: segment.x << 8,
        y: segment.y << 8,
        z: retained_z,
    };
    let Some(slot) = construct_at(arena, terrain, position)? else {
        return Ok(None);
    };
    let source = &mut arena.entities[usize::from(slot)];
    source.set_source_dword_10(segment.length);
    source.yaw = segment.dx;
    source.record[32..34].copy_from_slice(&segment.dx.to_le_bytes());
    source.record[34..36].copy_from_slice(&segment.dy.to_le_bytes());
    Ok(Some(slot))
}

pub(crate) fn construct_at(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
) -> Result<Option<u16>, String> {
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut source = new_event(slot, arena.economy.new_event_rng(slot), position);
    source.class = 10;
    source.model = 30;
    source.record[67] = 10;
    source.record[68] = 30;
    source.set_source_action(32);
    source.set_max_life(0);
    source.set_life(0);
    source.set_control_flags((source.control_flags & !8) | 4);
    if let Err(error) = arena.economy.map_chains.sync_entity(RenderableMapSeed {
        entity_index: slot,
        active: true,
        x: position.x,
        y: position.y,
    }) {
        arena.economy.release_slot(slot, Some(source.entity_rng));
        return Err(error);
    }
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.entities[usize::from(slot)] = source;
    synchronize_links(arena, terrain, slot);
    Ok(Some(slot))
}

pub(crate) fn step(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
) -> Result<(), String> {
    let source = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && (row.class, row.model, row.action) == (10, 30, 32))
        .ok_or("terrain path source row is not live")?;
    let x = (source.position.x.wrapping_add(128) >> 8) as u8;
    let y = (source.position.y.wrapping_add(128) >> 8) as u8;
    let dy = i16::from_le_bytes([source.record[34], source.record[35]]);
    let length =
        i32::from_le_bytes(source.record[20..24].try_into().expect("source dword10")) as u16;
    paths::apply(terrain, x, y, source.yaw, dy, length);
    source.disable_drawing();
    Ok(())
}
