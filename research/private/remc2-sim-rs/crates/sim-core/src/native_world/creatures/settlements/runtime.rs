//! Settlement StageVar binding and arena effects. The source scheduler supplies
//! the frozen memberships; this adapter never schedules an extra actor visit.

use super::{SettlementStageLinks, stage};
use crate::Position;
use crate::campaign::source_arena::SourceArena;
use crate::native_world::creatures::archer::ArcherStageRuntime;
use crate::native_world::{
    Level2OrdinaryResetSession, ParticleParameter, RuntimeSnapshot, SessionTerrain,
};
use crate::villager::VillagerFamily;

pub(crate) struct Route<'a> {
    pub model: u8,
    pub position: Position,
    pub action: &'a mut u8,
    pub index: &'a mut i8,
    pub mode: &'a mut i8,
    pub links: &'a mut SettlementStageLinks,
}

impl Route<'_> {
    /// sub_12330/sub_12470: the shared counter is consumed even when this
    /// particular constructor is rejected by the route's distribution flags.
    pub(crate) fn bind(&mut self, runtime: &mut dyn ArcherStageRuntime, index: u8) {
        if index != 0
            && let Some(descriptor) = runtime.bind(index)
        {
            *self.index = index as i8;
            *self.mode = descriptor.kind as i8;
            self.links.relation = if descriptor.kind == 6 {
                descriptor.scalar
            } else {
                0
            };
            *self.action = self.model * 8 + 7;
        } else {
            *self.index = 0;
            *self.mode = 0;
            self.links.relation = 0;
            *self.action = self.model * 8 + 1;
        }
    }

    fn prepare(&mut self, runtime: &mut dyn ArcherStageRuntime, targets: &[stage::StageTarget]) {
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
                        self.links.relation = 0;
                    }
                    descriptor.kind != 3 || !matches!(action, 2 | 6)
                } else {
                    if descriptor.flags & 2 != 0
                        && matches!(descriptor.kind, 3..=5)
                        && targets
                            .iter()
                            .find(|row| row.view.source_entity_index == self.links.relation)
                            .is_some_and(|row| {
                                row.view.life < 0 || row.view.control_flags & 0x400 != 0
                            })
                    {
                        self.links.relation = 0;
                    }
                    descriptor.kind == 9
                        && descriptor.flags & 2 == 0
                        && (descriptor.point.x != 0 || descriptor.point.y != 0)
                        && near(3072)
                }
            }
            6 => {
                self.links.relation = self.links.relation.wrapping_sub(1);
                self.links.relation == 0
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

/// sub_12500 visits model buckets, then ascending source slots within each
/// bucket. It changes routes but never advances byte_0x3E.
pub(crate) fn prepare_source_pass(
    arena: &mut SourceArena,
    ordinary: &mut Level2OrdinaryResetSession,
    runtime: &mut RuntimeSnapshot,
    models: &[Vec<u16>; 29],
) {
    let views = arena.settlement_source_views(
        &ordinary.houses,
        ordinary.villagers.villagers(),
        &ordinary.house_parameters,
    );
    let targets = stage::targets(arena, &views, ordinary.villagers.villagers());
    for model in 12..=14 {
        for &slot in &models[model] {
            if let Some(actor) = ordinary
                .villagers
                .villagers_mut()
                .iter_mut()
                .find(|actor| actor.entity_index == slot)
            {
                Route {
                    model: actor.model,
                    position: actor.position,
                    action: &mut actor.action,
                    index: &mut actor.stage_var_1,
                    mode: &mut actor.stage_var_2,
                    links: &mut actor.stage,
                }
                .prepare(runtime, &targets);
                actor.publish_source(&mut arena.entities[usize::from(slot)]);
            } else if let Some(actor) = arena
                .economy
                .house_residents
                .iter_mut()
                .find(|actor| actor.source_entity_index == slot)
            {
                Route {
                    model: actor.model,
                    position: actor.position,
                    action: &mut actor.action,
                    index: &mut actor.stage_var_1,
                    mode: &mut actor.stage_var_2,
                    links: &mut actor.stage,
                }
                .prepare(runtime, &targets);
                let actor = *actor;
                actor.publish_source(&mut arena.entities[usize::from(slot)]);
            } else if model == 14
                && let Some(actor) = arena
                    .economy
                    .authored_class5
                    .iter_mut()
                    .find(|actor| actor.source_entity_index == slot && actor.model == 14)
            {
                let state = arena
                    .economy
                    .house_residents
                    .traders
                    .entry(slot)
                    .or_insert_with(|| super::AuthoredTraderBehavior::new(slot));
                Route {
                    model: actor.model,
                    position: actor.position,
                    action: &mut actor.action,
                    index: &mut state.index,
                    mode: &mut state.mode,
                    links: &mut state.links,
                }
                .prepare(runtime, &targets);
                let actor = *actor;
                let state = *state;
                crate::native_world::level2_class5::trader::publish_source(
                    &actor,
                    state,
                    &mut arena.entities[usize::from(slot)],
                );
            }
        }
    }
}

pub(crate) fn apply_effects(
    arena: &mut SourceArena,
    villagers: &mut VillagerFamily,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    effects: Vec<stage::StageEffect>,
) -> Result<Vec<(u16, u8)>, String> {
    let mut awards = Vec::new();
    for effect in effects {
        match effect {
            stage::StageEffect::OwnerFeedback { owner, selector } => awards.push((owner, selector)),
            stage::StageEffect::Flash { slot, bits } => {
                if let Some(actor) = villagers
                    .villagers_mut()
                    .iter_mut()
                    .find(|actor| actor.entity_index == slot)
                {
                    actor.control_flags = (actor.control_flags & !0x0600_0000) | bits;
                    actor.publish_source(&mut arena.entities[usize::from(slot)]);
                } else if let Some(actor) = arena
                    .economy
                    .house_residents
                    .iter_mut()
                    .find(|actor| actor.source_entity_index == slot)
                {
                    actor.control_flags = (actor.control_flags & !0x0600_0000) | bits;
                    let actor = *actor;
                    actor.publish_source(&mut arena.entities[usize::from(slot)]);
                } else {
                    if let Some(state) = arena.economy.house_residents.traders.get_mut(&slot) {
                        state.control_flags = (state.control_flags & !0x0600_0000) | bits;
                    }
                    if let Some(raw) = arena
                        .entities
                        .get_mut(usize::from(slot))
                        .filter(|raw| raw.active)
                    {
                        raw.set_control_flags((raw.control_flags & !0x0600_0000) | bits);
                    }
                }
            }
            stage::StageEffect::Expire {
                slot: _,
                position,
                summoned,
            } => {
                if summoned {
                    crate::native_world::creatures::goat::runtime::spawn_expiry_visual(
                        arena, terrain, position, particles,
                    )?;
                }
                let _ = crate::world::effects::allocate(
                    arena,
                    terrain,
                    sim_definition::adapter::mc2_effect_definition(
                        crate::level_definition::EffectKind::Impact,
                    ),
                    position,
                    particles,
                )?;
            }
        }
    }
    Ok(awards)
}
