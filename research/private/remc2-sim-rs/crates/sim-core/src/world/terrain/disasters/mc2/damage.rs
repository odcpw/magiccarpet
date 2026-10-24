//! Synchronous source116A0 -> actor mailboxes -> source6D8B0 experience.
//! Borrow only the authoritative arena, actor and progression domains.

use super::AreaDamage;
use crate::{
    Position,
    campaign::source_arena::SourceArena,
    combat::area::{self, Shape},
    native_world::{Level2SharedSpellResetSession, area_damage},
};

pub(crate) fn apply(
    arena: &mut SourceArena,
    actors: Option<area_damage::ActorDomains<'_>>,
    player: Option<(u16, Position)>,
    progression: Option<&mut Level2SharedSpellResetSession>,
    experience_enabled: bool,
    damage: AreaDamage,
) -> Result<u16, String> {
    let source = arena
        .entities
        .get(usize::from(damage.source_slot))
        .ok_or("terrain damage source is absent")?;
    let request = area::AreaDamage {
        position: damage.position,
        shape: Shape {
            half_width: damage.half_width,
            half_depth: damage.half_depth,
            vertical_offset: damage.vertical_offset,
            half_height: damage.half_height,
        },
        owner: damage.owner,
        payload: damage.payload,
        selector: (source.record[69], source.record[70]),
        mode: 0,
    };
    let count = area_damage::apply(arena, actors, player, request)?;
    if count == 0 || !experience_enabled || damage.owner == 0 {
        return Ok(count);
    }
    let Some(family) = damage.experience_spell else {
        return Ok(count);
    };
    let Some(owner) = arena.entities.get(usize::from(damage.owner)) else {
        return Err("terrain experience owner is outside the source arena".into());
    };
    if owner.class != 3 || owner.model != 0 {
        return Ok(count);
    }
    let progression = progression.ok_or("terrain experience has no retained progression owner")?;
    if !progression
        .players()
        .iter()
        .any(|player| player.entity_index == damage.owner)
    {
        return Err("terrain experience owner has no retained player record".into());
    }
    progression.award_spell_experience(damage.owner, family, i32::from(count as i16));
    Ok(count)
}
