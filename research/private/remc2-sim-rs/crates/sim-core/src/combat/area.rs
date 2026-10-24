//! `sub_116A0`: synchronous rectangular area damage, distinct from 10C80.
//! Targets and writes are resolved against the caller's authoritative arena.

use crate::Position;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Shape {
    pub half_width: i16,
    pub half_depth: i16,
    pub vertical_offset: i16,
    pub half_height: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AreaDamage {
    pub position: Position,
    pub shape: Shape,
    pub owner: u16,
    pub payload: u16,
    pub selector: (u8, u8),
    pub mode: u8,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Actor {
    pub source_id: u16,
    pub class: u8,
    pub model: u8,
    pub position: Position,
    pub shape: Shape,
    pub damage_mask: u8,
    pub control_flags: u32,
}

/// The adapter resolves the current occupant at every visit. No mutable
/// snapshot or second damage mailbox is retained by the combat kernel.
pub(crate) trait AreaWorld {
    fn class3_slots(&self) -> Vec<u16>;
    fn cell_slots(&self, x: u8, y: u8) -> Vec<u16>;
    fn actor(&self, slot: u16) -> Option<Actor>;
    fn mark_model2(&mut self, slot: u16) -> Result<(), String>;
    fn ordinary_damage(&mut self, slot: u16, payload: u16, owner: u16) -> Result<(), String>;
}

fn overlaps(request: AreaDamage, target: Actor) -> bool {
    // sub_10630 wraps horizontal differences to signed words, but promotes
    // all four signed vertical terms before adding/subtracting them.
    let dx = i32::from(target.position.x.wrapping_sub(request.position.x) as i16).abs();
    let dy = i32::from(target.position.y.wrapping_sub(request.position.y) as i16).abs();
    let dz = (i32::from(request.position.z) + i32::from(request.shape.vertical_offset)
        - i32::from(target.position.z)
        - i32::from(target.shape.vertical_offset))
    .abs();
    dx < i32::from(request.shape.half_width) + i32::from(target.shape.half_width)
        && dy < i32::from(request.shape.half_depth) + i32::from(target.shape.half_depth)
        && dz < i32::from(request.shape.half_height) + i32::from(target.shape.half_height)
}

/// Returns the actual writes, in the source's word-sized caller contract.
/// Terrain and experience consumers must run after this function returns.
pub(crate) fn apply(world: &mut impl AreaWorld, request: AreaDamage) -> Result<u16, String> {
    // The pinned function has no nonzero-mode implementation.
    if request.mode != 0 {
        return Ok(0);
    }
    let mut hits = 0_u16;
    for slot in world.class3_slots() {
        let Some(target) = world.actor(slot) else {
            continue;
        };
        if target.class == 3 && target.model == 2 && overlaps(request, target) {
            // Even an own Castle receives the countdown before owner rejection.
            world.mark_model2(slot)?;
            if target.source_id != request.owner {
                world.ordinary_damage(slot, request.payload, request.owner)?;
                hits = hits.wrapping_add(1);
            }
        }
    }
    // Source signed division truncates toward zero; the x/y coordinates are
    // unsigned words promoted before +128, then wrapped to bytes per cell.
    let center_x = (i32::from(request.position.x) + 128) / 256;
    let center_y = (i32::from(request.position.y) + 128) / 256;
    let radius = (i32::from(request.shape.half_width) + 255) / 256;
    for y in -radius..=radius {
        for x in -radius..=radius {
            for slot in world.cell_slots((center_x + x) as u8, (center_y + y) as u8) {
                let Some(target) = world.actor(slot) else {
                    continue;
                };
                let selected = request.selector.0 == u8::MAX
                    || (request.selector.0 == target.class
                        && (request.selector.1 == u8::MAX || request.selector.1 == target.model));
                if target.source_id != request.owner
                    && overlaps(request, target)
                    && target.damage_mask & 1 != 0
                    && target.control_flags & 8 != 0
                    && (target.class, target.model) != (3, 2)
                    && selected
                {
                    world.ordinary_damage(slot, request.payload, request.owner)?;
                    hits = hits.wrapping_add(1);
                }
            }
        }
    }
    Ok(hits)
}
