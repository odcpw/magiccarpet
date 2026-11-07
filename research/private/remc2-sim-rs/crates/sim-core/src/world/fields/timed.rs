//! Source model33's finite controller and model35's bare allocator.
//!
//! No ordinary producer for either selector is known in retained campaigns.
//! The bare allocator returns a blank NewEvent; it does not construct a field.

use crate::{
    Position,
    campaign::source_arena::SourceArena,
    level_definition::FieldDefinition,
    native_world::{EntitySnapshot, SessionTerrain},
    world::terrain::cave_features::new_event,
};

/// Source `sub_4FF20`: return the actual blank NewEvent, without map insertion.
pub(crate) fn construct_blank(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
) -> Option<u16> {
    let slot = arena.allocate_source_slot_with_terrain(terrain)?;
    let source = new_event(slot, arena.economy.new_event_rng(slot), Position::default());
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.entities[usize::from(slot)] = source;
    Some(slot)
}

/// Source `sub_50020`: model33 has no particle or map insertion.
/// When NewEvent replaces the address supplying `position`, the source
/// constructor observes that address after NewEvent cleared it.
pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    position_slot: Option<u16>,
    definition: FieldDefinition,
) -> Result<Option<u16>, String> {
    let FieldDefinition::TimedController {
        lifetime,
        half_width,
        half_height,
    } = definition
    else {
        return Err("timed controller needs its field properties".to_owned());
    };
    let Some(slot) = construct_blank(arena, terrain) else {
        return Ok(None);
    };
    let source = &mut arena.entities[usize::from(slot)];
    source.class = 10;
    source.model = 33;
    source.record[67] = 10;
    source.record[68] = 33;
    source.set_source_action(35);
    source.set_control_flags(source.control_flags & !8);
    source.set_position(if position_slot == Some(slot) {
        Position::default()
    } else {
        position
    });
    source.set_max_life(lifetime);
    source.set_life(lifetime);
    source.set_source_overlap_half_extents(half_width, half_width);
    source.record[92..94].copy_from_slice(&half_height.to_le_bytes());
    Ok(Some(slot))
}

/// Source `sub_35600`: increment first and test the old life after decrement.
pub(crate) fn step(source: &mut EntitySnapshot) {
    source.set_source_dword_10(source.source_dword_10().wrapping_add(1));
    let life = source.life;
    source.set_life(life.wrapping_sub(1));
    if life < 0 {
        source.disable_drawing();
    } else if source.control_flags & 2 == 0 {
        source.set_control_flags(source.control_flags | 2);
    }
}
