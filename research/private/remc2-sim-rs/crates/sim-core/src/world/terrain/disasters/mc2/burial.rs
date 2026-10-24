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
