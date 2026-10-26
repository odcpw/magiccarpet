//! Source sub_12330/sub_12410/sub_12500 route transitions shared by actors.
//!
//! Every field is borrowed from its canonical actor carrier. The runtime
//! adapter owns the one shared StageVar counter and its source thinning rule.

use super::archer::ArcherStageRuntime;
use crate::Position;
use crate::campaign::source_arena::SourceArena;
use crate::native_world::{EntitySnapshot, RuntimeRelation, RuntimeSnapshot};

pub(crate) struct Route<'a> {
    pub model: u8,
    pub position: Position,
    pub action: &'a mut u8,
    pub index: &'a mut i8,
    pub mode: &'a mut i8,
    pub relation: &'a mut u16,
}

impl Route<'_> {
    pub(crate) fn bind(&mut self, runtime: &mut dyn ArcherStageRuntime, index: u8) {
        if index != 0
            && let Some(descriptor) = runtime.bind(index)
        {
            *self.index = index as i8;
            *self.mode = descriptor.kind as i8;
            *self.relation = if descriptor.kind == 6 {
                descriptor.scalar
            } else {
                0
            };
            *self.action = self.model * 8 + 7;
        } else {
            *self.index = 0;
            *self.mode = 0;
            *self.relation = 0;
            *self.action = self.model * 8 + 1;
        }
    }

    pub(crate) fn prepare(
        &mut self,
        runtime: &mut dyn ArcherStageRuntime,
        target_is_dead: impl Fn(u16) -> bool,
    ) {
        let action = *self.action & 7;
        if matches!(action, 4 | 5) || (*self.index == 0 && *self.mode == 0) {
            return;
        }
        match *self.mode {
            13 | 14 | 16 | 17 => {
                if !matches!(action, 2 | 6) {
                    *self.action = self.model * 8 + 7;
                }
                return;
            }
            10 => {
                if !matches!(action, 2 | 6) {
                    self.bind(runtime, *self.index as u8);
                }
                return;
            }
            15 => {
                if action != 0 {
                    self.bind(runtime, *self.index as u8);
                }
                return;
            }
            _ => {}
        }
        let Some(descriptor) = runtime.descriptor(*self.index as u8) else {
            return;
        };
        let near = |range: u16| {
            self.position
                .x
                .wrapping_sub(descriptor.point.x)
                .cast_signed()
                .unsigned_abs()
                <= range
                && self
                    .position
                    .y
                    .wrapping_sub(descriptor.point.y)
                    .cast_signed()
                    .unsigned_abs()
                    <= range
        };
        let ended = match descriptor.kind {
            1 => near(2048),
            3 | 4 | 5 | 8 | 9 => {
                if descriptor.flags & 4 != 0 {
                    if descriptor.kind == 3 {
                        *self.relation = 0;
                    }
                    descriptor.kind != 3 || !matches!(action, 2 | 6)
                } else {
                    if descriptor.flags & 2 != 0
                        && matches!(descriptor.kind, 3..=5)
                        && *self.relation != 0
                        && target_is_dead(*self.relation)
                    {
                        *self.relation = 0;
                    }
                    descriptor.kind == 9
                        && descriptor.flags & 2 == 0
                        && (descriptor.point.x != 0 || descriptor.point.y != 0)
                        && near(3072)
                }
            }
            6 => {
                *self.relation = self.relation.wrapping_sub(1);
                *self.relation == 0
            }
            7 => descriptor.flags & 0x18 != 0,
            _ => false,
        };
        if ended {
            if descriptor.kind == 7 {
                runtime.rearm_model_relations();
            }
            self.bind(runtime, descriptor.next);
        }
    }
}

fn edit_route(source: &mut EntitySnapshot, edit: impl FnOnce(&mut Route<'_>)) {
    let mut action = source.action;
    let mut index = source.record[76] as i8;
    let mut mode = source.record[77] as i8;
    let mut relation = u16::from_le_bytes([source.record[78], source.record[79]]);
    edit(&mut Route {
        model: source.model,
        position: source.position,
        action: &mut action,
        index: &mut index,
        mode: &mut mode,
        relation: &mut relation,
    });
    source.set_source_action(action);
    source.record[76] = index as u8;
    source.record[77] = mode as u8;
    source.record[78..80].copy_from_slice(&relation.to_le_bytes());
}

/// Source sub_12780, once after membership rebuild and before any actor's
/// sub_12500 visit. Finished target routes latch bit4; group pulses consume
/// their two source bits on successive passes rather than remaining sticky.
pub(crate) fn update_shared_stage_variables(
    runtime: &mut RuntimeSnapshot,
    entities: &[EntitySnapshot],
    models: &[Vec<u16>; 29],
) {
    // Source count is the highest active authored index, so retain and visit
    // holes below it; a cleared count suppresses this scan without erasing rows.
    let active_count = usize::from(runtime.active_stage_variable_count());
    for (index, row) in runtime
        .stage_variables
        .iter_mut()
        .enumerate()
        .skip(1)
        .take(active_count)
    {
        let mut flags = row.flags as u8;
        match row.kind {
            3 | 4 | 5 | 8 | 9 => {
                let finished = if flags & 4 != 0 {
                    true
                } else if flags & 2 != 0 {
                    match row.relation {
                        RuntimeRelation::AxisScalar(model) | RuntimeRelation::Scalar(model) => {
                            models.get(model as usize).is_some_and(Vec::is_empty)
                        }
                        _ => false,
                    }
                } else {
                    match row.relation {
                        RuntimeRelation::LiveEntity(slot)
                        | RuntimeRelation::LiveEntityIndex(slot) => {
                            entities.get(usize::from(slot)).is_some_and(|target| {
                                target.life < 0 || target.control_flags & 0x400 != 0
                            })
                        }
                        _ => false,
                    }
                };
                if finished {
                    flags |= 4;
                }
            }
            7 if flags & 0x18 != 0 => {
                flags &= if flags & 0x10 != 0 { !0x10 } else { !8 };
            }
            _ => {}
        }
        row.flags = flags as i8;
        runtime.bytes[392 + index * 12 + 1] = flags;
    }
}

pub(crate) fn bind_source(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slot: u16,
    index: u8,
) -> Result<(), String> {
    if index != 0 && runtime.descriptor(index).is_none() {
        return Err(format!("source StageVar {index} is absent"));
    }
    let source = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|source| {
            source.active && source.class == 5 && matches!(source.model, 3 | 9 | 17 | 18 | 26 | 28)
        })
        .ok_or_else(|| format!("source route lost creature slot {slot}"))?;
    edit_route(source, |route| route.bind(runtime, index));
    Ok(())
}

pub(crate) fn prepare_source_pass(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    model_slots: &[u16],
) {
    // Route preparation changes no target life/visibility. Capture only this
    // predicate once so each actor can borrow its own source row mutably.
    let dead_targets = arena
        .entities
        .iter()
        .filter(|target| target.life < 0 || target.control_flags & 0x400 != 0)
        .map(|target| target.index)
        .collect::<std::collections::BTreeSet<_>>();
    for &slot in model_slots {
        let Some(source) = arena.entities.get_mut(usize::from(slot)).filter(|source| {
            source.active && source.class == 5 && matches!(source.model, 3 | 9 | 17 | 18 | 26 | 28)
        }) else {
            continue;
        };
        edit_route(source, |route| {
            route.prepare(runtime, |target| dead_targets.contains(&target));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::native_world::Mc2DispatchEvent;
    use crate::{FlightWorld, PlayerCommand};
    use std::path::PathBuf;

    #[test]
    fn opening_skeleton_arrival_releases_route_before_ordinary_visit() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level3-day-block32-campaign-recipe-v2");
        let mut campaign = CampaignSession::open(package, CampaignSelection::LEVEL3)
            .expect("actual opening Level 3");
        let session = campaign.simulation_mut();
        let slot = 93;
        let initial = session.entities()[slot].clone();
        assert_eq!((initial.class, initial.model, initial.action), (5, 9, 72));
        assert_eq!(&initial.record[78..80], &2_u16.to_le_bytes());
        for _ in 0..16 {
            session.step_outer_turn(PlayerCommand::default(), 0);
            assert!(
                session
                    .dispatch_events()
                    .iter()
                    .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "constructor turn must commit: {:?}",
                session.dispatch_events()
            );
        }
        let source = &session.entities()[slot];
        assert_eq!(
            (source.action, source.record[76], source.record[77]),
            (79, 2, 1),
            "the real countdown constructor binds the authored point route"
        );
        let mut point = session
            .session
            .state
            .live
            .core
            .runtime
            .descriptor(2)
            .unwrap()
            .point;
        point.z = session.canonical_terrain().ground_height(point);
        session
            .set_level2_entity_position(slot as u16, point)
            .expect("isolate arrival at the actual authored route destination");
        let phase = session.entities()[slot].phase;
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "arrival turn must commit: {:?}",
            session.dispatch_events()
        );
        let source = &session.entities()[slot];
        assert_eq!(
            source.action, 73,
            "arrival restores ordinary Skeleton movement"
        );
        assert_eq!(&source.record[76..80], &[0, 0, 0, 0]);
        assert_eq!(source.phase, phase.wrapping_add(1));
        campaign.reset();
        assert_eq!(campaign.simulation().entities()[slot], initial);
    }
}
