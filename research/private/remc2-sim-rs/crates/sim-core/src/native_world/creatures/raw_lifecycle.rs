//! Common MC2 lifecycle operations for families whose source rows are the
//! authoritative carrier. Typed families retain their own lifecycle owners.

use std::collections::BTreeSet;

use crate::campaign::source_arena::SourceArena;

fn word(row: &crate::native_world::EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}

/// Source `PreKillEntity_1C890`: propagate the kill action along the actual
/// child chain, inherit the last nonzero killer, then publish root credit.
/// No allocation occurs here; the following Kill callback owns death products.
pub(crate) fn prekill(arena: &mut SourceArena, slot: u16, base: u8) -> Result<Option<u16>, String> {
    let action = base.checked_add(5).ok_or("invalid creature action base")?;
    if arena.economy.live_arena_cursor_row(slot).is_some() {
        return Err(format!("prekill root {slot} belongs to a typed family"));
    }
    let root = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && row.class == 5)
        .ok_or_else(|| format!("prekill lost raw creature {slot}"))?;
    let model = root.model;
    let source_id = root.source_id;
    let mut killer = word(root, 40);
    let mut next = word(root, 56);
    let mut visited = BTreeSet::from([slot]);
    let mut children = Vec::new();
    while next != 0 {
        if !visited.insert(next) {
            return Err(format!("prekill child chain cycles at {next}"));
        }
        if arena.economy.live_arena_cursor_row(next).is_some() {
            return Err(format!("prekill child {next} belongs to a typed family"));
        }
        let child = arena
            .entities
            .get(usize::from(next))
            .filter(|row| row.active)
            .ok_or_else(|| format!("prekill child {next} is not retained"))?;
        children.push(next);
        let child_killer = word(child, 40);
        if child_killer != 0 {
            killer = child_killer;
        }
        next = word(child, 56);
    }
    for child in children {
        arena.entities[usize::from(child)].set_source_action(action);
    }
    let root = &mut arena.entities[usize::from(slot)];
    root.record[40..42].copy_from_slice(&killer.to_le_bytes());
    root.set_source_action(action);
    Ok(arena
        .entities
        .get(usize::from(killer))
        .filter(|row| killer != 0 && row.active)
        .filter(|row| {
            crate::combat::actors::awards_creature_kill(
                model,
                source_id,
                row.source_id,
                row.class,
                row.model,
            )
        })
        .map(|_| killer))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_prekill_inherits_last_child_credit_and_preserves_death_boundary() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let player = session.state.entity_index;
        let mut slots = Vec::new();
        for _ in 0..3 {
            let slot = arena.allocate_source_slot().expect("ordinary free slot");
            let mut row = crate::native_world::cave_features::new_event(
                slot,
                arena.new_event_rng(slot),
                session.state.position,
            );
            row.class = 5;
            row.model = 21;
            row.record[67] = 5;
            row.record[68] = 21;
            row.source_id = 600;
            row.set_source_action(172);
            arena.entities[usize::from(slot)] = row;
            slots.push(slot);
        }
        for pair in slots.windows(2) {
            arena.entities[usize::from(pair[0])].record[56..58]
                .copy_from_slice(&pair[1].to_le_bytes());
        }
        arena.entities[usize::from(slots[0])].record[40..42]
            .copy_from_slice(&slots[1].to_le_bytes());
        arena.entities[usize::from(slots[2])].record[40..42].copy_from_slice(&player.to_le_bytes());
        let generations = slots
            .iter()
            .map(|&slot| arena.slot_generation(slot))
            .collect::<Vec<_>>();
        assert_eq!(prekill(&mut arena, slots[0], 168).unwrap(), Some(player));
        for (&slot, generation) in slots.iter().zip(generations) {
            assert_eq!(arena.entities[usize::from(slot)].action, 173);
            assert!(arena.entities[usize::from(slot)].active);
            assert_eq!(arena.slot_generation(slot), generation);
        }
        assert_eq!(word(&arena.entities[usize::from(slots[0])], 40), player);
        // Corrupt topology cannot silently terminate a partial callback.
        arena.entities[usize::from(slots[2])].record[56..58]
            .copy_from_slice(&slots[0].to_le_bytes());
        assert!(prekill(&mut arena, slots[0], 168).is_err());
    }
}
