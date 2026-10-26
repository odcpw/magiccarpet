//! Model23 source StageVar callback.

use super::{Actor, Context};
use crate::native_world::creatures::stage_route::Route;

pub(super) fn step(actor: &mut Actor<'_>, ctx: &mut Context<'_>) {
    let mut action = actor.row.action;
    let mut index = actor.row.record[76] as i8;
    let mut mode = actor.row.record[77] as i8;
    let mut relation = actor.word(78);
    let mut route = Route {
        model: super::SOURCE_MODEL,
        position: actor.row.position,
        action: &mut action,
        index: &mut index,
        mode: &mut mode,
        relation: &mut relation,
    };
    route.prepare(ctx.runtime, |slot| {
        ctx.entities.get(usize::from(slot)).is_none_or(|target| {
            !target.active || target.life < 0 || target.control_flags & 0x400 != 0
        })
    });
    actor.row.set_source_action(action);
    actor.row.record[76] = index as u8;
    actor.row.record[77] = mode as u8;
    actor.put(78, relation);
}
