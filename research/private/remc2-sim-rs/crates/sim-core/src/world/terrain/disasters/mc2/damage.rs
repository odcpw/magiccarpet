//! Synchronous source116A0 -> actor mailboxes -> source6D8B0 experience.
//! Borrow only the authoritative arena, actor and progression domains.

use super::AreaDamage;
use crate::{
    Position,
    campaign::source_arena::SourceArena,
    collision_mask::CollisionMaskBank,
    combat::area::{self, Shape},
    native_world::{Level2SharedSpellResetSession, area_damage},
};

/// Borrow the same pass-start lists and retained collision content as effects.
/// Each call resolves its recipients and writes the canonical live carriers.
pub(crate) struct Context<'a> {
    pub(crate) actors: Option<area_damage::ActorDomains<'a>>,
    pub(crate) player: Option<(u16, Position)>,
    pub(crate) progression: Option<&'a mut Level2SharedSpellResetSession>,
    pub(crate) experience_enabled: bool,
    pub(crate) masks: Option<&'a CollisionMaskBank>,
    pub(crate) lists: &'a area_damage::EffectLists,
}

pub(crate) fn apply_request(
    arena: &mut SourceArena,
    context: Context<'_>,
    request: super::runtime::DamageRequest,
) -> Result<u16, String> {
    let damage = match request {
        super::runtime::DamageRequest::Rectangle(damage) => {
            return apply(
                arena,
                context.actors,
                context.player,
                context.progression,
                context.experience_enabled,
                damage,
            );
        }
        super::runtime::DamageRequest::Projectile(damage) => damage,
    };
    let source = arena
        .entities
        .get(usize::from(damage.source_slot))
        .ok_or("terrain projectile damage source is absent")?;
    let pulse = crate::world::effects::DamagePulse {
        position: damage.position,
        owner: damage.owner,
        payload: damage.payload,
        half_width: damage.half_width,
        vertical_offset: damage.vertical_offset,
        half_height: damage.half_height,
        continuous: false,
        selector: (source.record[69], source.record[70]),
    };
    let count = area_damage::apply_effect(
        arena,
        context.actors,
        context.player,
        context.masks,
        context.lists,
        pulse,
    )?;
    award_experience(
        arena,
        context.progression,
        context.experience_enabled,
        damage,
        count,
    )
}

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
    award_experience(arena, progression, experience_enabled, damage, count)
}

fn award_experience(
    arena: &SourceArena,
    progression: Option<&mut Level2SharedSpellResetSession>,
    experience_enabled: bool,
    damage: AreaDamage,
    count: u16,
) -> Result<u16, String> {
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
