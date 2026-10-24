//! MC2 linked-record preparation and dynamic model27 source callbacks.

use std::collections::BTreeSet;

use crate::{
    Position,
    campaign::source_arena::SourceArena,
    level1_mission::AuthoredFeature,
    native_world::{
        SessionTerrain,
        cave_features::{new_event, synchronize_links},
    },
    renderable_map_chain::RenderableMapSeed,
    world::terrain::disasters::ribbon::{self, Direction, Segment},
};

fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    segment: Segment,
) -> Result<Option<u16>, String> {
    let height = |x: u16, y: u16| {
        terrain.planes().ground_heights[usize::from(x as u8) + 256 * usize::from(y as u8)]
    };
    let position = Position {
        x: segment.x << 8,
        y: segment.y << 8,
        z: 32 * i16::from(height(segment.x, segment.y).max(height(segment.end_x, segment.end_y))),
    };
    let Some(slot) = construct_at(arena, terrain, position)? else {
        return Ok(None);
    };
    let source = &mut arena.entities[usize::from(slot)];
    source.set_source_action(match segment.direction {
        Direction::North => 27,
        Direction::South => 28,
        Direction::East => 29,
    });
    source.set_source_dword_10(i32::from(segment.length));
    Ok(Some(slot))
}

/// sub4F7A0 is also a direct constructor. A linked chain writes its own
/// direction/length only after this ordinary allocation has completed.
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
    source.model = 27;
    source.record[67] = 10;
    source.record[68] = 27;
    source.set_max_life(2);
    source.set_life(2);
    source.set_subspell_index(((position.z >> 5) + 48) as u16);
    source.set_source_action(27);
    source.set_source_dword_10(10);
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

/// sub_49090 -> sub_48400: rewind par1 to the head, consume stage tags,
/// follow par2 and construct the complete subdivision in literal order.
pub(crate) fn prepare_authored(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    features: &mut [AuthoredFeature],
    index: u16,
) -> Result<Vec<u16>, String> {
    let first = *features
        .get(usize::from(index))
        .ok_or("ribbon authored row is absent")?;
    if (first.kind, first.subtype) != (10, 28) {
        return Err("ribbon producer requires a raised-ribbon authored row".into());
    }
    if first.stage_tag == 0 {
        return Ok(Vec::new());
    }
    let mut index = index;
    let mut visited = BTreeSet::new();
    loop {
        if !visited.insert(index) {
            return Err("ribbon predecessor cycle".into());
        }
        let row = features
            .get(usize::from(index))
            .ok_or("ribbon predecessor is absent")?;
        if row.parameter_1 == 0 {
            break;
        }
        index = row.parameter_1;
    }
    visited.clear();
    let mut born = Vec::new();
    loop {
        if !visited.insert(index) {
            return Err("ribbon successor cycle".into());
        }
        let source = features
            .get_mut(usize::from(index))
            .ok_or("ribbon successor is absent")?;
        source.stage_tag = 0;
        let source = *source;
        if source.parameter_2 == 0 {
            break;
        }
        let next = *features
            .get(usize::from(source.parameter_2))
            .ok_or("ribbon endpoint is absent")?;
        for segment in ribbon::segments(source.tile_x, source.tile_y, next.tile_x, next.tile_y) {
            let slot = construct(arena, terrain, segment)?
                .ok_or("raised-ribbon source arena is exhausted")?;
            born.push(slot);
        }
        index = source.parameter_2;
    }
    Ok(born)
}

pub(crate) fn step(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
) -> Result<(), String> {
    let source = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|source| source.active && source.class == 10 && source.model == 27)
        .ok_or("ribbon callback lost its source row")?;
    let direction = match source.action {
        27 => Direction::North,
        28 => Direction::South,
        29 => Direction::East,
        action => return Err(format!("ribbon action {action} is unsupported")),
    };
    let x = (source.position.x.wrapping_add(128) >> 8) as u8;
    let y = (source.position.y.wrapping_add(128) >> 8) as u8;
    let length =
        i32::from_le_bytes(source.record[20..24].try_into().expect("source dword10")) as i16;
    ribbon::apply(terrain, x, y, source.life as i16, length, direction);
    source.disable_drawing();
    Ok(())
}
