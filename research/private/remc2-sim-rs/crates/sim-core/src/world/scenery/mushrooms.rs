//! Cave mushrooms: sub_4AFE0, sub_651B0 and sub_65240.
//!
//! The source entity is the mutable carrier; smoke allocation and the
//! following fixed-address terrain tail run at their original boundaries.

use crate::{
    FlightWorld, Position,
    campaign::source_arena::SourceArena,
    native_world::{EntitySnapshot, ParticleParameter, SessionTerrain},
    renderable_map_chain::RenderableMapSeed,
    world::terrain::cave_features::{bind_particle, new_event, synchronize_links},
};

pub(crate) use sim_definition::level_definition::MushroomDefinition as Definition;

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SmokeBirth {
    parent: crate::campaign::source_arena::SourceEntityHandle,
    newborn: crate::campaign::source_arena::SourceEntityHandle,
    pre_ground_position: Position,
}

#[cfg(test)]
thread_local! {
    static SMOKE_BIRTH_TRACE: std::cell::RefCell<Option<(crate::campaign::source_arena::SourceEntityHandle, Option<SmokeBirth>)>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn start_smoke_birth_trace(parent: crate::campaign::source_arena::SourceEntityHandle) {
    SMOKE_BIRTH_TRACE.with(|trace| *trace.borrow_mut() = Some((parent, None)));
}

#[cfg(test)]
fn take_smoke_birth_trace() -> Option<SmokeBirth> {
    SMOKE_BIRTH_TRACE.with(|trace| trace.borrow_mut().take().and_then(|(_, birth)| birth))
}

fn next_rng(rng: u16) -> u16 {
    rng.wrapping_mul(9_377).wrapping_add(9_439)
}

/// Bind imported group-zero construction to the same immutable life rule as
/// later authored births. The retained entity RNG is the fourth constructor
/// draw; mushroom callbacks do not advance it. 48_993 is the inverse of 9_377
/// modulo 2^16, so this does not consume or replace any live RNG draw.
pub(crate) fn bind_initial(
    source: &mut EntitySnapshot,
    definition: Definition,
) -> Result<(), String> {
    definition.validate()?;
    if !source.active || (source.class, source.model) != (2, 6) {
        return Err("mushroom definition requires its active source carrier".into());
    }
    let mut life_draw = source.entity_rng;
    for _ in 0..3 {
        life_draw = life_draw.wrapping_sub(9_439).wrapping_mul(48_993);
    }
    let maximum = definition.minimum_life + i32::from(life_draw % definition.life_variation);
    let deficit = source.max_life.wrapping_sub(source.life);
    source.set_max_life(maximum);
    source.set_life(maximum.wrapping_sub(deficit));
    Ok(())
}

fn bind(
    source: &mut EntitySnapshot,
    index: u16,
    particles: &[ParticleParameter],
) -> Result<(), String> {
    let particle = particles
        .get(usize::from(index))
        .copied()
        .ok_or_else(|| format!("mushroom particle {index} is absent"))?;
    bind_particle(source, index, particle)
}

/// Outside Cave, even allocation and RNG are untouched. Exhaustion likewise
/// consumes no mushroom-local RNG draws.
pub(crate) fn construct(
    cave: bool,
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    definition: Definition,
    particles: &[ParticleParameter],
) -> Result<Option<u16>, String> {
    if !cave {
        return Ok(None);
    }
    definition.validate()?;
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut source = new_event(slot, arena.economy.new_event_rng(slot), position);
    source.class = 2;
    source.model = 6;
    source.record[67] = 2;
    source.record[68] = 6;
    source.set_source_action(18);
    source.record[60] = 1; // byte_0x38
    source.set_entity_rng(next_rng(source.entity_rng));
    let life = definition.minimum_life + i32::from(source.entity_rng % definition.life_variation);
    source.set_max_life(life);
    source.set_life(life);
    let mut displaced = position;
    source.set_entity_rng(next_rng(source.entity_rng));
    displaced.x = displaced
        .x
        .wrapping_add((source.entity_rng & 63).wrapping_sub(32));
    source.set_entity_rng(next_rng(source.entity_rng));
    displaced.y = displaced
        .y
        .wrapping_add((source.entity_rng & 63).wrapping_sub(32));
    source.set_position(displaced);
    source.set_entity_rng(next_rng(source.entity_rng));
    let particle = 324 + (source.entity_rng & 3);
    if let Err(error) = bind(&mut source, particle, particles) {
        arena.economy.release_slot(slot, Some(source.entity_rng));
        return Err(error);
    }
    arena.economy.map_chains.sync_entity(RenderableMapSeed {
        entity_index: slot,
        active: true,
        x: displaced.x,
        y: displaced.y,
    })?;
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.entities[usize::from(slot)] = source;
    synchronize_links(arena, terrain, slot);
    Ok(Some(slot))
}

/// Run the source callback at its current fixed address. A lethally damaged
/// dry mushroom remains action19; on water the current occupant is hidden.
pub(crate) fn step_slot(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    particles: &[ParticleParameter],
) -> Result<(), String> {
    #[cfg(test)]
    let trace_target = arena
        .handle(slot)
        .map(|target| (target, arena.economy.events().len()));
    let mut smoke_position = None;
    let mut consumed = None;
    {
        let source = arena
            .entities
            .get_mut(usize::from(slot))
            .ok_or_else(|| format!("mushroom source slot {slot} is outside the arena"))?;
        match source.action {
            18 => {
                source.set_control_flags(source.control_flags | 0x0002_0000);
                let owner = source.source_upkeep_word_62();
                if owner != 0 {
                    let damage = source.source_upkeep_payload();
                    let before = source.life;
                    source.set_life(before.wrapping_sub(damage));
                    let after = source.life;
                    if after < 0 {
                        // The first clear precedes SetHalfSpeed and NewEvent.
                        source.clear_source_upkeep_owner();
                        let particle = source
                            .particle_parameter_index
                            .ok_or_else(|| "live mushroom has no particle binding".to_owned())?;
                        source.set_source_action(19);
                        source.set_control_flags(source.control_flags & !8);
                        bind(source, particle.wrapping_add(4), particles)?;
                        smoke_position = Some(source.position);
                    } else {
                        source.clear_source_upkeep_owner();
                    }
                    consumed = Some(((damage, owner), before, after));
                }
            }
            19 => {}
            action => return Err(format!("unsupported mushroom action {action}")),
        }
    }
    #[cfg(test)]
    if let (Some((target, event_cursor)), Some((mailbox, life_before, life_after))) =
        (trace_target, consumed)
    {
        let writer = crate::native_world::area_damage::take_player_mailbox_writer(target, mailbox);
        crate::native_world::area_damage::trace_player_damage_consumption(
            crate::native_world::area_damage::PlayerDamageConsumptionReceipt {
                target,
                mailbox,
                writer,
                life_before,
                life_after,
                event_cursor,
            },
        );
    }
    if let Some(position) = smoke_position {
        let newborn = crate::world::effects::allocate_from_source_position(
            arena,
            terrain,
            sim_definition::adapter::mc2_effect_definition(
                crate::level_definition::EffectKind::Smoke,
            ),
            slot,
            position,
            particles,
        )?;
        #[cfg(test)]
        if let (Some((parent, _)), Some(newborn)) = (trace_target, newborn)
            && let Some(newborn) = arena.handle(newborn)
        {
            let pre_ground_position = arena.entities[usize::from(newborn.source_slot)].position;
            SMOKE_BIRTH_TRACE.with(|trace| {
                if let Some((watched, birth)) = trace.borrow_mut().as_mut()
                    && *watched == parent
                {
                    *birth = Some(SmokeBirth {
                        parent,
                        newborn,
                        pre_ground_position,
                    });
                }
            });
        }
        #[cfg(not(test))]
        let _ = newborn;
        // Source clears the owner again after NewEvent, through the same
        // fixed pointer even when that address now belongs to the newborn.
        arena.entities[usize::from(slot)].clear_source_upkeep_owner();
    }
    #[cfg(not(test))]
    let _ = consumed;
    let source = &mut arena.entities[usize::from(slot)];
    let mut position = source.position;
    position.z = terrain.ground_height(position);
    source.set_position(position);
    if terrain.material_mask(position) == crate::WATER_AMBIENCE_MASK {
        source.disable_drawing();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_world::{NativeWorld, NativeWorldInspection};
    use std::path::PathBuf;

    fn owner_world() -> NativeWorldInspection {
        let package = std::env::var_os("MC_LEVEL2_PACKAGE_V5").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/l2-debug3.fJ3PaM/level2")
            },
            PathBuf::from,
        );
        NativeWorld::inspect_level2(package).expect("retained owner particle and arena data")
    }

    #[test]
    fn owner_cave_mushrooms_keep_source_defaults_through_open_step_reset() {
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level12");
        let mut game = CampaignSession::open(package, CampaignSelection::ordinary(12).unwrap())
            .expect("real Cave12 mushroom package admission");
        let mushrooms = |rows: &[EntitySnapshot]| {
            rows.iter()
                .filter(|row| row.active && (row.class, row.model) == (2, 6))
                .cloned()
                .collect::<Vec<_>>()
        };
        let initial = mushrooms(game.simulation().entities());
        assert_eq!(initial.len(), 56);
        for row in &initial {
            let mut rebound = row.clone();
            bind_initial(&mut rebound, Definition::default()).unwrap();
            assert_eq!(&rebound, row, "source default binding changes no row bytes");
        }
        game.step(crate::PlayerCommand::default(), 0);
        let errors = game
            .events()
            .iter()
            .filter_map(|event| match event {
                crate::native_world::Mc2DispatchEvent::Error { message, .. } => {
                    Some(message.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(errors.is_empty(), "ordinary Cave step rejected: {errors:?}");
        assert!(game.simulation().campaign_tick() > 0);
        let live = mushrooms(game.simulation().entities());
        assert!(!live.is_empty(), "ordinary Cave step retains dry mushrooms");
        assert!(live.iter().all(|row| matches!(row.action, 18 | 19)));
        game.reset();
        assert_eq!(mushrooms(game.simulation().entities()), initial);

        // The shipped original slot30 is exercised through one declared
        // carried Fireball and one fixed player-only firing placement. No
        // mushroom state, health, RNG or lifetime is configured.
        use crate::campaign::schedule::{CurrentOccupant, ScheduleReceipt};
        use crate::native_world::{Mc2DispatchEvent, area_damage};
        game.simulation_mut()
            .apply_carried_spell_families(&[0], Some(0))
            .unwrap();
        let ui = crate::SOURCE_SPELL_INDEX_MAP
            .iter()
            .position(|family| *family == 0)
            .unwrap() as i8;
        game.step(
            crate::PlayerCommand {
                opcode: 31,
                argument_1: ui,
                argument_2: 0,
                ..Default::default()
            },
            0,
        );
        assert!(
            game.simulation()
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "actual Cave assignment must dispatch"
        );
        let sim = game.simulation();
        let target_slot = 30_u16;
        let target = sim.entities()[usize::from(target_slot)].clone();
        assert!(target.active && (target.class, target.model, target.action) == (2, 6, 18));
        assert!((100..180).contains(&target.life));
        assert_ne!(target.control_flags & 8, 0);
        assert_ne!(
            sim.canonical_terrain().material_mask(target.position),
            crate::WATER_AMBIENCE_MASK
        );
        let target_handle = sim.source_arena().handle(target_slot).unwrap();
        let original_particle = target.particle_parameter_index.unwrap();
        assert!(
            sim.campaign_entity_presentations(sim.state.yaw)
                .unwrap()
                .iter()
                .any(|row| row.entity_index == target_slot
                    && row.render_parameter_index == original_particle)
        );
        let player = sim.state.entity_index;
        let player_handle = sim.source_arena().handle(player).unwrap();
        let holder = sim.shared_spells().local_player().enabled_spell_holders[0];
        let holder_handle = sim.source_arena().handle(holder).unwrap();
        let cost = crate::level_definition::FIREBALL_PROPERTIES.tiers[0].mana_cost;
        let shared = sim.shared_spells();
        let holder_row = shared
            .pickups()
            .iter()
            .chain(shared.holders())
            .chain(shared.dynamic_pickups())
            .find(|row| row.index == holder)
            .expect("the actual carried Fireball holder");
        assert_eq!(
            (
                holder_row.level_gate,
                holder_row.max_mana,
                holder_row.mana_regen
            ),
            (0, cost, 0)
        );
        assert!(sim.shared_spells().player_entity().mana >= cost);
        let centre = Position {
            z: target
                .position
                .z
                .wrapping_add(target.array_yaw())
                .wrapping_sub(crate::level1_mission::FIREBALL_COLLISION_VERTICAL_OFFSET),
            ..target.position
        };
        let hand = Position {
            z: centre.z.wrapping_add(1024),
            ..centre
        };
        let aim = crate::level1_mission::projectile_guidance_angles(hand, centre);
        let mut origin = Position {
            z: hand.z.wrapping_sub(sim.state.entity_fov),
            ..hand
        };
        let hand_yaw = aim
            .0
            .wrapping_sub(sim.shared_spells().beyond_sight_auxiliary().next_entity)
            .wrapping_sub(512);
        crate::move_entity(&mut origin, hand_yaw.wrapping_add(1024), 0, 256);
        let terrain = sim.canonical_terrain();
        assert!(
            origin.z
                >= terrain
                    .ground_height(origin)
                    .wrapping_add(sim.profile().ground_clearance)
        );
        assert!(
            origin.z
                <= terrain
                    .ceiling_height(origin)
                    .unwrap()
                    .wrapping_sub(sim.profile().ceiling_clearance)
        );
        game.simulation_mut()
            .set_level2_entity_position(player, origin)
            .unwrap();
        let mut no_cast = game.clone();
        area_damage::start_actor_damage_trace(&[target_handle]);
        let mut launches = Vec::new();
        let mut impacts = Vec::new();
        let mut terminals = Vec::new();
        let mut accepted = Vec::new();
        let mut consumed = Vec::new();
        let mut smoke = None;
        let mut stalk_presented = false;
        let mut smoke_presented = false;
        let mut smoke_hidden = false;
        let mut smoke_reclaimed = false;
        let mut queued = false;
        let mut paid = false;
        let mut elapsed = 0;
        for turn in 0..128 {
            if turn == 0 {
                game.simulation_mut()
                    .set_level2_player_aim_for_test(aim.0, aim.1);
            }
            area_damage::start_fireball_launch_trace();
            crate::native_world::start_fireball_terminal_trace();
            area_damage::start_effect_damage_trace(target_slot);
            start_smoke_birth_trace(target_handle);
            game.step(
                crate::PlayerCommand {
                    flags: if turn == 0 { 0x10 } else { 0 },
                    ..Default::default()
                },
                0,
            );
            elapsed += 1;
            let new_launches = area_damage::take_fireball_launch_trace();
            let own_launch_this_turn = new_launches.iter().any(|(_, _, owner)| *owner == player);
            launches.extend(new_launches);
            impacts.extend(area_damage::take_fireball_impact_birth_trace());
            terminals.extend(crate::native_world::take_fireball_terminal_trace());
            accepted.extend(area_damage::take_effect_damage_trace());
            consumed.extend(area_damage::take_player_damage_consumptions());
            if let Some(birth) = take_smoke_birth_trace() {
                smoke = Some(birth);
            }
            let sim = game.simulation();
            assert!(
                sim.dispatch_events()
                    .iter()
                    .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "actual Cave Fireball turn{turn}: {:?}",
                sim.dispatch_events()
            );
            for receipt in sim.cf10_production_schedule_receipts() {
                let ScheduleReceipt::EpilogueCompleted {
                    slot,
                    original,
                    current: CurrentOccupant::Known { selector, .. },
                    ..
                } = receipt
                else {
                    continue;
                };
                let (Some(before), Some(after)) = (
                    original.observed_player_resources(),
                    selector.observed_player_resources(),
                ) else {
                    continue;
                };
                if slot == holder && own_launch_this_turn {
                    let expected = if before.2 >= 0 {
                        -cost
                    } else {
                        before.2.wrapping_sub(cost)
                    };
                    queued |= sim.source_arena().handle(holder) == Some(holder_handle)
                        && before.0 == player_handle
                        && after == (player_handle, before.1, expected, before.3);
                }
                if slot == player
                    && before.0 == player_handle
                    && after.0 == player_handle
                    && before.2 <= -cost
                    && after.1 == before.1.wrapping_add(before.2).clamp(0, before.3)
                    && before.1 - after.1 >= cost
                {
                    paid = true;
                }
            }
            if sim.source_arena().handle(target_slot) == Some(target_handle) {
                let row = &sim.entities()[usize::from(target_slot)];
                if row.action == 19 {
                    let view = crate::native_world::source_target::read(
                        sim.source_arena(),
                        crate::native_world::source_target::Context::default(),
                        target_slot,
                    )
                    .unwrap()
                    .unwrap();
                    assert_eq!(
                        view.control_flags & 8,
                        0,
                        "the actual stalk is no longer a collision target"
                    );
                    assert!(
                        view.collision.is_some(),
                        "the current stalk retains its source collision axes"
                    );
                    stalk_presented |= sim
                        .campaign_entity_presentations(sim.state.yaw)
                        .unwrap()
                        .iter()
                        .any(|dto| {
                            dto.entity_index == target_slot
                                && dto.render_parameter_index == original_particle + 4
                        });
                }
            }
            if let Some(birth) = smoke {
                if sim.source_arena().handle(birth.newborn.source_slot) == Some(birth.newborn) {
                    let row = &sim.entities()[usize::from(birth.newborn.source_slot)];
                    smoke_hidden |= row.drawing_disabled();
                    smoke_presented |= sim
                        .campaign_entity_presentations(sim.state.yaw)
                        .unwrap()
                        .iter()
                        .any(|dto| {
                            dto.entity_index == birth.newborn.source_slot
                                && dto.render_parameter_index == 67
                        });
                } else {
                    smoke_reclaimed = true;
                }
            }
            if smoke_reclaimed && paid && stalk_presented && smoke_presented {
                break;
            }
        }
        let own_launches = launches
            .iter()
            .filter(|(_, _, owner)| *owner == player)
            .collect::<Vec<_>>();
        assert_eq!(
            own_launches.len(),
            1,
            "one real public Fireball: {launches:?}"
        );
        let (projectile_slot, projectile_generation, _) = *own_launches[0];
        let hit = consumed
            .iter()
            .find(|receipt| receipt.target == target_handle)
            .expect("actual mushroom subtraction");
        assert_eq!(
            (hit.mailbox, hit.life_before, hit.life_after),
            ((250, player), target.life, target.life - 250)
        );
        let writer = hit
            .writer
            .expect("the exact accepted effect writer reaches the actual subtraction");
        assert!(
            terminals.iter().any(
                |terminal| terminal.projectile.source_slot == projectile_slot
                    && terminal.projectile.generation == projectile_generation
                    && terminal.target_handle == Some(target_handle)
                    && terminal
                        .impact
                        .is_some_and(|impact| writer.source
                            == area_damage::PlayerMailboxSource::Effect(impact))
            ),
            "the real projectile collides with the same mushroom generation: {terminals:?}"
        );
        assert!(impacts.iter().any(
            |(impact, generation, parent, parent_generation, owner)| *parent == projectile_slot
                && *parent_generation == projectile_generation
                && *owner == player
                && writer.source
                    == area_damage::PlayerMailboxSource::Effect(
                        crate::campaign::source_arena::SourceEntityHandle {
                            source_slot: *impact,
                            generation: *generation
                        }
                    )
        ));
        assert!(accepted.iter().any(|receipt| receipt.source_scoped
            && receipt.target_slot == target_slot
            && receipt.target_generation == target_handle.generation
            && receipt.pulse.owner == player
            && receipt.pulse.payload == 250
            && receipt.writer_sequence == Some(writer.sequence)));
        let birth =
            smoke.expect("the actual lethal mushroom callback creates its own smoke generation");
        assert_eq!(
            (birth.parent, birth.pre_ground_position),
            (target_handle, target.position)
        );
        let sim = game.simulation();
        assert_eq!(sim.source_arena().handle(target_slot), Some(target_handle));
        let stalk = &sim.entities()[usize::from(target_slot)];
        assert_eq!(
            (stalk.action, stalk.life, stalk.source_upkeep_word_62()),
            (19, target.life - 250, 0)
        );
        assert!(
            queued && paid && stalk_presented && smoke_presented && smoke_hidden && smoke_reclaimed,
            "actual paid collision, stalk DTO and natural smoke hide/reclaim: queued={queued} paid={paid} stalk={stalk_presented} smoke={smoke_presented} hidden={smoke_hidden} reclaimed={smoke_reclaimed} elapsed={elapsed}"
        );
        // Trace storage is thread-local and keyed by source generations, not
        // session identity. Run the clone only after the real fight receipts
        // have been collected so its callbacks cannot contaminate attribution.
        for turn in 0..elapsed {
            no_cast.step(crate::PlayerCommand::default(), 0);
            assert!(
                no_cast
                    .simulation()
                    .dispatch_events()
                    .iter()
                    .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "equal-elapsed Cave no-cast turn{turn}: {:?}",
                no_cast.simulation().dispatch_events()
            );
        }
        assert_eq!(
            no_cast.simulation().source_arena().handle(target_slot),
            Some(target_handle)
        );
        assert_eq!(
            no_cast.simulation().entities()[usize::from(target_slot)].life,
            target.life,
            "equal-elapsed no-cast original remains alive"
        );

        // A declared water-material control continues the same real stalk;
        // its actor state, life, RNG and source schedule remain untouched.
        let stalk_position = game.simulation().entities()[usize::from(target_slot)].position;
        let tile = usize::from(stalk_position.x >> 8) + usize::from(stalk_position.y >> 8) * 256;
        game.simulation_mut()
            .session
            .state
            .live
            .terrain
            .disaster_parts()
            .0
            .terrain_types[tile] = 0;
        assert_eq!(
            game.simulation()
                .canonical_terrain()
                .material_mask(stalk_position),
            crate::WATER_AMBIENCE_MASK
        );
        assert!(
            game.simulation()
                .economy()
                .map_chains()
                .is_linked(target_slot)
        );
        game.step(crate::PlayerCommand::default(), 0);
        let sim = game.simulation();
        assert!(
            sim.dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        assert_eq!(sim.source_arena().handle(target_slot), Some(target_handle));
        assert!(
            sim.entities()[usize::from(target_slot)].drawing_disabled(),
            "the real action19 callback hides its original current stalk on water"
        );
        assert!(
            sim.economy().map_chains().is_linked(target_slot),
            "the hidden source occupant remains linked until the following boundary"
        );
        assert!(
            !sim.campaign_entity_presentations(sim.state.yaw)
                .unwrap()
                .iter()
                .any(|dto| dto.entity_index == target_slot),
            "the hidden stalk leaves the actual presentation consumer"
        );
        game.step(crate::PlayerCommand::default(), 0);
        let sim = game.simulation();
        assert!(
            sim.dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        assert!(
            sim.source_arena().resolve(target_handle).is_none(),
            "the following source boundary reclaims the original water-hidden generation"
        );
        assert!(sim.source_arena().handle(target_slot).is_none());
        assert!(
            !sim.economy().map_chains().is_linked(target_slot),
            "the reclaimed stalk leaves its actual map chain"
        );
    }

    #[test]
    fn damage_keeps_dry_stalk_and_water_hides_both_forms() {
        let world = owner_world();
        let particles = &world.presentation().particles;
        let mut session = world.level2_reset_session().unwrap();
        let live = &mut session.session.state.live;
        let slot = construct(
            true,
            &mut live.economy,
            &mut live.terrain,
            Position {
                x: 123,
                y: 456,
                z: 321,
            },
            Definition::default(),
            particles,
        )
        .unwrap()
        .unwrap();
        let position = live.economy.entities[usize::from(slot)].position;
        let tile = usize::from(position.x >> 8) + usize::from(position.y >> 8) * 256;
        live.terrain.disaster_parts().0.terrain_types[tile] = 1; // dry source material
        let ground = live.terrain.ground_height(position);
        let life = live.economy.entities[usize::from(slot)].life;
        let particle = live.economy.entities[usize::from(slot)]
            .particle_parameter_index
            .unwrap();
        step_slot(&mut live.economy, &mut live.terrain, slot, particles).unwrap();
        let source = &mut live.economy.entities[usize::from(slot)];
        assert_eq!((source.life, source.position.z), (life, ground));
        assert_ne!(source.control_flags & 0x20000, 0);
        source.record[98..102].copy_from_slice(&life.to_le_bytes());
        source.record[102..104].copy_from_slice(&1_u16.to_le_bytes());
        step_slot(&mut live.economy, &mut live.terrain, slot, particles).unwrap();
        let source = &mut live.economy.entities[usize::from(slot)];
        assert_eq!(
            (source.life, source.action, source.source_upkeep_word_62()),
            (0, 18, 0)
        );
        let death_position = Position {
            z: ground.wrapping_add(123),
            ..source.position
        };
        source.set_position(death_position);
        source.record[98..102].copy_from_slice(&1_i32.to_le_bytes());
        source.record[102..104].copy_from_slice(&1_u16.to_le_bytes());
        let existing_smoke = live
            .economy
            .entities
            .iter()
            .filter(|row| row.active && (row.class, row.model) == (10, 13))
            .map(|row| row.index)
            .collect::<Vec<_>>();
        step_slot(&mut live.economy, &mut live.terrain, slot, particles).unwrap();
        let source = &mut live.economy.entities[usize::from(slot)];
        assert_eq!(
            (source.life, source.action, source.source_upkeep_word_62()),
            (-1, 19, 0)
        );
        assert_eq!(source.particle_parameter_index, Some(particle + 4));
        assert_eq!(source.control_flags & 8, 0);
        assert!(!source.drawing_disabled());
        assert_eq!(source.position.z, ground);
        assert!(
            live.economy.entities.iter().any(|row| row.active
                && (row.class, row.model) == (10, 13)
                && !existing_smoke.contains(&row.index)
                && row.position == death_position),
            "the real smoke constructor reads the pre-ground source position"
        );
        let source = &mut live.economy.entities[usize::from(slot)];
        source.record[98..102].copy_from_slice(&99_i32.to_le_bytes());
        source.record[102..104].copy_from_slice(&1_u16.to_le_bytes());
        step_slot(&mut live.economy, &mut live.terrain, slot, particles).unwrap();
        assert_eq!(
            live.economy.entities[usize::from(slot)].life,
            -1,
            "action19 does not consume another damage mailbox"
        );
        live.terrain.disaster_parts().0.terrain_types[tile] = 0; // water
        step_slot(&mut live.economy, &mut live.terrain, slot, particles).unwrap();
        assert!(live.economy.entities[usize::from(slot)].drawing_disabled());
        let source = &mut live.economy.entities[usize::from(slot)];
        source.set_control_flags(8);
        source.set_source_action(18);
        source.clear_source_upkeep_owner();
        step_slot(&mut live.economy, &mut live.terrain, slot, particles).unwrap();
        assert!(live.economy.entities[usize::from(slot)].drawing_disabled());
    }

    #[test]
    fn smoke_new_event_uses_current_source_address_and_preserves_failure_order() {
        let world = owner_world();
        let particles = &world.presentation().particles;
        let mut session = world.level2_reset_session().unwrap();
        let live = &mut session.session.state.live;
        let slot = construct(
            true,
            &mut live.economy,
            &mut live.terrain,
            Position {
                x: 5120,
                y: 6144,
                z: 600,
            },
            Definition::default(),
            particles,
        )
        .unwrap()
        .unwrap();
        let position = live.economy.entities[usize::from(slot)].position;
        let tile = usize::from(position.x >> 8) + usize::from(position.y >> 8) * 256;
        live.terrain.disaster_parts().0.terrain_types[tile] = 1; // declared dry pressure control
        // A real action18 visit makes this mushroom source-replaceable. The
        // following isolated allocator-pressure control retains that producer,
        // not an artificially set replacement flag.
        step_slot(&mut live.economy, &mut live.terrain, slot, particles).unwrap();
        assert_ne!(
            live.economy.entities[usize::from(slot)].control_flags & 0x20000,
            0
        );
        assert!(!live.economy.entities[usize::from(slot)].drawing_disabled());
        let mut arena = live.economy.clone();
        let mut terrain = live.terrain.clone();
        arena.economy.exhaust_new_event_capacity_for_test();
        for row in &mut arena.entities {
            if row.index != slot {
                row.set_control_flags(row.control_flags & !0x20000);
            }
        }
        arena.rebuild_free_arena(&[]);
        arena.economy.exhaust_new_event_capacity_for_test();
        let mut typed_candidates = Vec::new();
        arena
            .economy
            .for_each_live_replaceable_arena_row(|candidate, kind| {
                typed_candidates.push((candidate, kind))
            });
        assert!(
            typed_candidates.is_empty(),
            "isolated source fallback: {typed_candidates:?}"
        );
        assert_eq!(arena.economy.free_slot_count(), 0);
        assert_eq!(
            arena
                .entities
                .iter()
                .filter(|row| row.active && row.class != 0 && row.control_flags & 0x20000 != 0)
                .map(|row| row.index)
                .collect::<Vec<_>>(),
            vec![slot]
        );
        let original = arena.handle(slot).unwrap();
        let source = &mut arena.entities[usize::from(slot)];
        let lethal = source.life + 1;
        source.record[98..102].copy_from_slice(&lethal.to_le_bytes());
        source.record[102..104].copy_from_slice(&1_u16.to_le_bytes());
        let mut failed = arena.clone();
        let mut failed_terrain = terrain.clone();
        let rng_before = arena.economy.global_rng;
        let next_rng = rng_before.wrapping_mul(9_377).wrapping_add(9_439);

        start_smoke_birth_trace(original);
        step_slot(&mut arena, &mut terrain, slot, particles).unwrap();
        let birth = take_smoke_birth_trace()
            .expect("real smoke allocation is observed at its source boundary");
        let newborn = &arena.entities[usize::from(slot)];
        assert_eq!((newborn.class, newborn.model, newborn.action), (10, 13, 13));
        assert_ne!(
            arena.handle(slot),
            Some(original),
            "actual NewEvent replaces the captured generation"
        );
        assert_eq!(
            (birth.parent, Some(birth.newborn), birth.pre_ground_position),
            (original, arena.handle(slot), Position::default())
        );
        assert_eq!(
            (newborn.position.x, newborn.position.y),
            (0, 0),
            "NewEvent clears the aliased source-position pointer before the smoke reads it"
        );
        assert_eq!(
            newborn.position.z,
            terrain.ground_height(newborn.position),
            "the mushroom's source tail then grounds the current newborn address"
        );
        assert_eq!(newborn.source_upkeep_word_62(), 0);
        assert_eq!(arena.economy.global_rng, next_rng);

        failed.finish_source_group(); // no ordinary or captured replacement candidate
        start_smoke_birth_trace(original);
        step_slot(&mut failed, &mut failed_terrain, slot, particles).unwrap();
        assert!(
            take_smoke_birth_trace().is_none(),
            "failed NewEvent creates no smoke child"
        );
        let stalk = &failed.entities[usize::from(slot)];
        assert_eq!(failed.handle(slot), Some(original));
        assert_eq!(
            (stalk.class, stalk.model, stalk.action, stalk.life),
            (2, 6, 19, -1)
        );
        assert_eq!(stalk.source_upkeep_word_62(), 0);
        assert_eq!(
            failed.economy.global_rng, next_rng,
            "source smoke RNG advances before an allocation failure"
        );
    }

    #[test]
    fn constructor_uses_four_source_draws_and_preserves_refused_allocation() {
        let world = owner_world();
        let particles = &world.presentation().particles;
        let mut session = world.level2_reset_session().unwrap();
        let live = &mut session.session.state.live;
        let position = Position {
            x: 0,
            y: u16::MAX,
            z: 300,
        };
        let before = live.economy.clone();
        let terrain_before = live.terrain.clone();
        assert_eq!(
            construct(
                false,
                &mut live.economy,
                &mut live.terrain,
                position,
                Definition::default(),
                particles
            )
            .unwrap(),
            None
        );
        assert_eq!(live.economy, before);
        assert_eq!(live.terrain, terrain_before);
        let mut probe = before;
        let mut probe_terrain = terrain_before;
        let expected_slot = probe
            .allocate_source_slot_with_terrain(&mut probe_terrain)
            .unwrap();
        let r1 = next_rng(probe.economy.new_event_rng(expected_slot));
        let r2 = next_rng(r1);
        let r3 = next_rng(r2);
        let r4 = next_rng(r3);
        let definition = Definition {
            minimum_life: 200,
            life_variation: 3,
        };
        let slot = construct(
            true,
            &mut live.economy,
            &mut live.terrain,
            position,
            definition,
            particles,
        )
        .unwrap()
        .unwrap();
        assert_eq!(slot, expected_slot);
        let source = &live.economy.entities[usize::from(slot)];
        assert_eq!((source.class, source.model, source.action), (2, 6, 18));
        assert_eq!(source.life, 200 + i32::from(r1 % 3));
        assert_eq!(source.max_life, source.life);
        assert_eq!(source.entity_rng, r4);
        assert_eq!(
            source.position.x,
            position.x.wrapping_add((r2 & 63).wrapping_sub(32))
        );
        assert_eq!(
            source.position.y,
            position.y.wrapping_add((r3 & 63).wrapping_sub(32))
        );
        assert_eq!(source.particle_parameter_index, Some(324 + (r4 & 3)));
        let mut imported = source.clone();
        bind_initial(&mut imported, definition).unwrap();
        assert_eq!(
            imported, *source,
            "initial binding preserves constructor bytes"
        );
        imported.set_life(imported.life - 7);
        bind_initial(&mut imported, Definition::default()).unwrap();
        assert_eq!(imported.max_life, 100 + i32::from(r1 % 80));
        assert_eq!(imported.life, imported.max_life - 7);
        assert_eq!(imported.entity_rng, r4);
        while live
            .economy
            .allocate_source_slot_with_terrain(&mut live.terrain)
            .is_some()
        {}
        let exhausted = live.economy.clone();
        assert_eq!(
            construct(
                true,
                &mut live.economy,
                &mut live.terrain,
                position,
                Definition::default(),
                particles
            )
            .unwrap(),
            None
        );
        assert_eq!(live.economy, exhausted);
    }
}
