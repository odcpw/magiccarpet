//! Settlement StageVar binding and arena effects. The source scheduler supplies
//! the frozen memberships; this adapter never schedules an extra actor visit.

use super::stage;
use crate::Position;
use crate::campaign::source_arena::SourceArena;
use crate::native_world::creatures::stage_route::Route;
use crate::native_world::{
    Level2OrdinaryResetSession, ParticleParameter, RuntimeSnapshot, SessionTerrain,
};
use crate::villager::VillagerFamily;

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
                    relation: &mut actor.stage.relation,
                }
                .prepare(runtime, |relation| {
                    targets.iter().any(|row| {
                        row.view.source_entity_index == relation
                            && (row.view.life < 0 || row.view.control_flags & 0x400 != 0)
                    })
                });
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
                    relation: &mut actor.stage.relation,
                }
                .prepare(runtime, |relation| {
                    targets.iter().any(|row| {
                        row.view.source_entity_index == relation
                            && (row.view.life < 0 || row.view.control_flags & 0x400 != 0)
                    })
                });
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
                    relation: &mut state.links.relation,
                }
                .prepare(runtime, |relation| {
                    targets.iter().any(|row| {
                        row.view.source_entity_index == relation
                            && (row.view.life < 0 || row.view.control_flags & 0x400 != 0)
                    })
                });
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

/// sub_68BF0/sub_68C70 runs after route preparation and before actor visits.
/// Use the published raw links for child propagation, then commit the cadence
/// byte back to each typed owner so its callback cannot restore an old value.
pub(crate) fn proximity_prepass(
    arena: &mut SourceArena,
    ordinary: &mut Level2OrdinaryResetSession,
    models: &[Vec<u16>; 29],
    player: Position,
) {
    for model in 12..=14 {
        arena.step_raw_creature_proximity(&models[model], player);
    }
    for actor in ordinary.villagers.villagers_mut() {
        if let Some(raw) = arena.entities.get(usize::from(actor.entity_index)) {
            actor.source_byte_39 = raw.record[61];
        }
    }
    for actor in arena.economy.house_residents.iter_mut() {
        if let Some(raw) = arena.entities.get(usize::from(actor.source_entity_index)) {
            actor.source_byte_39 = raw.record[61];
        }
    }
    for actor in arena
        .economy
        .authored_class5
        .iter_mut()
        .filter(|row| row.model == 14)
    {
        if let Some(raw) = arena.entities.get(usize::from(actor.source_entity_index)) {
            actor.source_byte_39 = raw.record[61];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_one_proximity_cadence_survives_the_typed_actor_publication() {
        let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq8-level1-collision-v5-night");
        let mut campaign = crate::campaign::CampaignSession::open(
            package,
            crate::campaign::spec::CampaignSelection::LEVEL1,
        )
        .unwrap();
        let live = &mut campaign.simulation_mut().session.state.live;
        let position = live.core.state.position;
        let actor = live
            .ordinary
            .villagers
            .villagers_mut()
            .first_mut()
            .expect("Level1 reset contains ordinary Villagers");
        actor.source_byte_39 = 2;
        actor.position = position;
        actor.control_flags &= !1;
        let slot = actor.entity_index;
        let phase = actor.phase;
        actor.publish_source(&mut live.economy.entities[usize::from(slot)]);
        let mut models: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
        models[13].push(slot);
        for expected in [1, 0, 16] {
            proximity_prepass(&mut live.economy, &mut live.ordinary, &models, position);
            let actor = live
                .ordinary
                .villagers
                .villagers()
                .iter()
                .find(|actor| actor.entity_index == slot)
                .unwrap();
            assert_eq!(actor.source_byte_39, expected);
            assert_eq!(
                actor.phase, phase,
                "a prepass is not an additional actor visit"
            );
            actor.publish_source(&mut live.economy.entities[usize::from(slot)]);
            assert_eq!(
                live.economy.entities[usize::from(slot)].record[61],
                expected
            );
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
