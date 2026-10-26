//! Cymmerian `sub_28F50` StageVar entry and authored-route binding.

use super::{Actor, Context, RULES, SOURCE_MODEL};
use crate::campaign::source_arena::SourceArena;
use crate::native_world::creatures::archer::ArcherStageRuntime;
use crate::native_world::{EntitySnapshot, RuntimeSnapshot};

fn put(row: &mut EntitySnapshot, at: usize, value: u16) {
    row.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn word(row: &EntitySnapshot, at: usize) -> u16 {
    u16::from_le_bytes([row.record[at], row.record[at + 1]])
}

fn distance_squared(a: crate::Position, b: crate::Position) -> u32 {
    let dx = i32::from(b.x.wrapping_sub(a.x) as i16);
    let dy = i32::from(b.y.wrapping_sub(a.y) as i16);
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
}

/// Consume the reachable Army sub_28F50 -> sub_1D5D0(200) -> sub_1E580
/// callback. Some(true) means source type-10/subtype-73 and subtype-0
/// products must be materialized before this source pass returns.
pub(crate) fn step_action_207(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
) -> Result<Option<bool>, String> {
    let mode = actor.row.record[77];
    if mode != crate::native_world::summon_army::SUMMONED_STAGE_MODE {
        return Err(format!(
            "Cymmerian action207 StageVar2={mode} is not implemented by this leaf"
        ));
    }

    let parent_slot = word(actor.row, 44);
    let parent = ctx.target(parent_slot).filter(|parent| {
        parent.life >= 0 && parent.flags & crate::native_world::ENTITY_DRAW_DISABLED_BIT == 0
    });
    if parent.is_none() {
        put(actor.row, 50, 0);
    } else {
        put(actor.row, 50, word(actor.row, 50).wrapping_sub(1));
    }
    let timer = word(actor.row, 50) as i16;
    if timer <= 0 {
        return Ok(Some(true));
    }
    let parent = parent.expect("live Army parent checked above");

    let hit = super::consume(actor, ctx);
    if hit == 2 {
        // sub_1E700 leaves the ordinary death callback for the source tail by
        // setting word_0x2E to one.
        put(actor.row, 50, 1);
        return Ok(None);
    }
    if hit == 1
        && let Some(attacker) = ctx.target(word(actor.row, 42)).filter(|target| {
            target.life >= 0 && target.flags & crate::native_world::ENTITY_DRAW_DISABLED_BIT == 0
        })
        && (attacker.class, attacker.model) != (actor.row.class, actor.row.model)
    {
        actor.set_target(attacker.slot);
        actor.set_action(202);
        return Ok(None);
    }

    let mut target = ctx.target(actor.target()).filter(|target| {
        target.life > 0 && target.flags & crate::native_world::ENTITY_DRAW_DISABLED_BIT == 0
    });
    if target.is_none() {
        actor.set_target(0);
        if actor.row.phase & 7 == 0 {
            target = ctx
                .targets
                .iter()
                .copied()
                .filter(|candidate| {
                    candidate.slot != actor.row.index
                        && candidate.source_id != parent.source_id
                        && matches!(candidate.model, 0 | 1)
                        && candidate.life > 0
                        && candidate.flags & crate::native_world::ENTITY_DRAW_DISABLED_BIT == 0
                })
                .min_by_key(|candidate| distance_squared(parent.position, candidate.position));
            if let Some(target) = target {
                actor.set_target(target.slot);
            }
        }
    }

    if let Some(target) = target {
        super::move_actor(actor, ctx);
        if actor.row.phase & 7 == 0 {
            actor
                .row
                .set_source_roll(
                    crate::heading_for_positions(actor.row.position, target.position) as i16,
                );
            if super::distance(actor.row.position, target.position) < RULES.target_range {
                actor.set_action(202);
            }
        }
    } else {
        // sub_1E580 temporarily points word96 at the parent while invoking
        // sub_1E700, then clears it and applies the source's four-tick penalty.
        actor.set_target(parent_slot);
        super::move_actor(actor, ctx);
        actor.set_target(0);
        put(actor.row, 50, word(actor.row, 50).wrapping_sub(4));
        if (word(actor.row, 50) as i16) <= 0 {
            return Ok(Some(true));
        }
    }
    Ok(None)
}

fn edit_route(row: &mut EntitySnapshot, runtime: &mut dyn ArcherStageRuntime, index: u8) {
    let mut action = row.action;
    let mut route_index = row.record[76] as i8;
    let mut mode = row.record[77] as i8;
    let mut relation = u16::from_le_bytes([row.record[78], row.record[79]]);
    let mut route = super::super::stage_route::Route {
        model: SOURCE_MODEL,
        position: row.position,
        action: &mut action,
        index: &mut route_index,
        mode: &mut mode,
        relation: &mut relation,
    };
    route.bind(runtime, index);
    row.set_source_action(action);
    row.record[76] = route_index as u8;
    row.record[77] = mode as u8;
    put(row, 78, relation);
}

pub(crate) fn bind_authored(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slot: u16,
    index: u8,
) -> Result<(), String> {
    let row = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        .ok_or_else(|| format!("Cymmerian StageVar bind lost slot{slot}"))?;
    edit_route(row, runtime, index);
    Ok(())
}

/// Source `sub_12470(a, 200)` followed by `byte46 = 3` in `sub_28F50`.
pub(crate) fn reset_to_special(actor: &mut Actor<'_>) {
    actor.row.record[76..80].fill(0);
    actor.row.set_source_action(200);
    actor.row.record[74] = 3;
}

pub(crate) fn prepare_source_pass(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slots: &[u16],
) {
    let dead = super::mc2::targets_in_arena(arena)
        .into_iter()
        .filter(|target| target.life < 0 || target.flags & 0x400 != 0)
        .map(|target| target.slot)
        .collect::<std::collections::BTreeSet<_>>();
    for &slot in slots {
        let Some(row) = arena
            .entities
            .get_mut(usize::from(slot))
            .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        else {
            continue;
        };
        let mut action = row.action;
        let mut index = row.record[76] as i8;
        let mut mode = row.record[77] as i8;
        let mut relation = u16::from_le_bytes([row.record[78], row.record[79]]);
        let mut route = super::super::stage_route::Route {
            model: SOURCE_MODEL,
            position: row.position,
            action: &mut action,
            index: &mut index,
            mode: &mut mode,
            relation: &mut relation,
        };
        route.prepare(runtime, |target| dead.contains(&target));
        row.set_source_action(action);
        row.record[76] = index as u8;
        row.record[77] = mode as u8;
        put(row, 78, relation);
    }
}
