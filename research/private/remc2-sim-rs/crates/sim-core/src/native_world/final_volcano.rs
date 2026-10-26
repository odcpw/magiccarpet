//! Final-encounter death reward handoff to the ordinary Volcano source.

use crate::Position;
use crate::campaign::source_arena::SourceArena;
use crate::native_world::{RuntimeSnapshot, SessionTerrain};

/// Make the final encounter's one phase15 attempt to create its reward Volcano.
///
/// Failure consumes the pending marker without changing the source-global
/// final-Volcano flag. The caller still clears the boss lifecycle word and hides
/// the boss unconditionally; this source event is not retried.
/// The returned slot is the constructed arena row for downstream consumers,
/// not a mission-victory signal. The caller must continue the ordinary
/// objective/result path for either allocation outcome.
pub(crate) fn materialize_boss_death_reward(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    runtime: &mut RuntimeSnapshot,
    pending: &mut bool,
    position: Position,
    owner: u16,
) -> Result<Option<u16>, String> {
    if !*pending {
        return Ok(None);
    }

    let Some(slot) = crate::world::terrain::disasters::mc2::volcano::construct_owned(
        arena, terrain, position, owner,
    ) else {
        *pending = false;
        return Ok(None);
    };

    {
        let source = arena
            .entities
            .get_mut(usize::from(slot))
            .filter(|source| {
                source.active && (source.class, source.model, source.action) == (10, 9, 9)
            })
            .ok_or_else(|| format!("boss Volcano constructor lost slot{slot}"))?;
        if source.record.get(30..32).is_none() {
            return Err(format!("boss Volcano source slot{slot} has no owner field"));
        }
        source.set_life(32);
        source.set_max_life(11);
        source.source_id = owner;
        source.record[30..32].copy_from_slice(&owner.to_le_bytes());
    }

    runtime.bytes[68] = 1;
    *pending = false;
    Ok(Some(slot))
}
