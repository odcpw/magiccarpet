//! Shared remc2 class-2 ballistic-scenery behavior.
//!
//! The behavior is selected by the compiled class/action registry and mutates
//! the lossless imported source row. Allocation, map topology, the common
//! byte-0x3E epilogue, and following-pass reclaim remain owned by the campaign
//! source scheduler and arena.

use crate::{
    FlightWorld, Position,
    campaign::source_arena::SourceArena,
    native_world::{EntitySnapshot, MapType, ParticleParameter, SessionTerrain, terrain_index},
    renderable_map_chain::RenderableMapSeed,
    world::terrain::cave_features::{bind_particle, new_event, synchronize_links},
};

pub(crate) use sim_definition::level_definition::BallisticSceneryDefinition as Definition;

fn next_rng(seed: u16) -> u16 {
    seed.wrapping_mul(9_377).wrapping_add(9_439)
}

#[cfg(test)]
thread_local! {
    static SMOKE_BIRTH_TRACE: std::cell::RefCell<Option<(crate::campaign::source_arena::SourceEntityHandle, Option<crate::campaign::source_arena::SourceEntityHandle>)>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn start_smoke_birth_trace(parent: crate::campaign::source_arena::SourceEntityHandle) {
    SMOKE_BIRTH_TRACE.with(|trace| *trace.borrow_mut() = Some((parent, None)));
}

#[cfg(test)]
fn take_smoke_birth_trace() -> Option<crate::campaign::source_arena::SourceEntityHandle> {
    SMOKE_BIRTH_TRACE.with(|trace| trace.borrow_mut().take().and_then(|(_, newborn)| newborn))
}

/// Observe only a real successful child allocation for the watched generation.
#[cfg(test)]
pub(crate) fn trace_smoke_birth(
    parent: crate::campaign::source_arena::SourceEntityHandle,
    newborn: crate::campaign::source_arena::SourceEntityHandle,
) {
    SMOKE_BIRTH_TRACE.with(|trace| {
        if let Some((watched, birth)) = trace.borrow_mut().as_mut()
            && *watched == parent
        {
            *birth = Some(newborn);
        }
    });
}

/// The two source constructor entries share the same life and motion rules.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BallisticKind {
    Model7,
    Model8,
}

impl BallisticKind {
    const fn source_fields(self) -> (u8, u8, u16) {
        match self {
            Self::Model7 => (7, 20, 322),
            Self::Model8 => (8, 21, 323),
        }
    }
}

/// Bind the source's first constructor draw without changing retained RNG or
/// erasing damage already present on an imported reset carrier.
pub(crate) fn bind_initial(
    source: &mut EntitySnapshot,
    definition: Definition,
) -> Result<(), String> {
    definition.validate()?;
    if !source.active
        || !matches!(
            (source.class, source.model, source.action),
            (2, 7, 20) | (2, 8, 21)
        )
    {
        return Err("ballistic scenery definition requires its active source carrier".into());
    }
    let mut life_draw = source.entity_rng;
    for _ in 0..2 {
        life_draw = life_draw.wrapping_sub(9_439).wrapping_mul(48_993);
    }
    let maximum = definition.minimum_life + i32::from(life_draw % definition.life_variation);
    let deficit = source.max_life.wrapping_sub(source.life);
    source.set_max_life(maximum);
    source.set_life(maximum.wrapping_sub(deficit));
    Ok(())
}

/// sub_4B0F0 / sub_4B120 / sub_4B150. Cave returns before NewEvent.
pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    kind: BallisticKind,
    definition: Definition,
    particles: &[ParticleParameter],
) -> Result<Option<u16>, String> {
    if terrain.map_type() == MapType::Cave {
        return Ok(None);
    }
    definition.validate()?;
    let (model, action, particle_index) = kind.source_fields();
    let particle = *particles
        .get(usize::from(particle_index))
        .ok_or_else(|| format!("ballistic scenery particle {particle_index} is absent"))?;
    if particle.animation_limit().is_none() {
        return Err(format!(
            "ballistic scenery particle {particle_index} has unsupported render mode {}",
            particle.render_mode
        ));
    }
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut source = new_event(slot, arena.economy.new_event_rng(slot), position);
    source.class = 2;
    source.model = model;
    source.record[67] = 2;
    source.record[68] = model;
    source.set_source_action(action);
    source.record[60] = 1;
    source.record[74] = 0;
    source.set_source_word_2c(-128);
    source.set_actual_speed(0);
    source.set_entity_rng(next_rng(source.entity_rng));
    let life = definition.minimum_life + i32::from(source.entity_rng % definition.life_variation);
    source.set_max_life(life);
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
    // AddEventToMap sets the source map-membership bit on the new row.
    source.set_control_flags(source.control_flags | 4);
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.entities[usize::from(slot)] = source;
    arena.economy.map_chains.sync_entity(RenderableMapSeed {
        entity_index: slot,
        active: true,
        x: displaced.x,
        y: displaced.y,
    })?;
    synchronize_links(arena, terrain, slot);
    let source = &mut arena.entities[usize::from(slot)];
    source.set_life(life);
    bind_particle(source, particle_index, particle)?;
    Ok(Some(slot))
}

const SKIP_ONCE_CONTROL_BIT: u32 = 0x0000_0800;
const VERTICAL_ACCELERATION: i16 = 24;
const MIN_VERTICAL_SPEED: i16 = -192;
const MAX_VERTICAL_SPEED: i16 = 192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BallisticScenerySpawn {
    Model13Smoke,
    Model5WaterSplash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BallisticSceneryStep {
    pub(crate) spawn: Option<BallisticScenerySpawn>,
    pub(crate) hide: bool,
    #[cfg(test)]
    pub(crate) damage: Option<((i32, u16), i32, i32)>,
}

fn tile_slope(terrain: &SessionTerrain, position: Position) -> i32 {
    let x = position.x.to_be_bytes()[0];
    let y = position.y.to_be_bytes()[0];
    let heights = terrain.ground_heights();
    let p1 = i32::from(heights[terrain_index(x, y)]);
    let p2 = i32::from(heights[terrain_index(x.wrapping_add(1), y)]);
    let p3 = i32::from(heights[terrain_index(x.wrapping_add(1), y.wrapping_add(1))]);
    let p4 = i32::from(heights[terrain_index(x, y.wrapping_add(1))]);
    (p1 + p4 - p2 - p3).abs().max((p1 + p2 - p4 - p3).abs())
}

fn avoid_steep_tile(source: &EntitySnapshot, terrain: &SessionTerrain, position: &mut Position) {
    if tile_slope(terrain, *position) <= 20 {
        return;
    }

    let mut direction = source.yaw;
    let mut lowest_height = u32::MAX;
    let mut selected = *position;
    for _ in 0..8 {
        let mut candidate = *position;
        crate::move_entity(&mut candidate, direction, 0, 64);
        let height = u32::from(terrain.ground_height(candidate).cast_unsigned());
        if height < lowest_height {
            lowest_height = height;
            selected = candidate;
        }
        direction = direction.wrapping_add(256) & 0x07ff;
    }
    *position = selected;
}

/// Execute `sub_652C0` without taking ownership of scheduler or arena work.
pub(crate) fn step_ballistic_scenery(
    source: &mut EntitySnapshot,
    terrain: &SessionTerrain,
) -> BallisticSceneryStep {
    if source.control_flags & SKIP_ONCE_CONTROL_BIT != 0 {
        source.set_control_flags(source.control_flags & !SKIP_ONCE_CONTROL_BIT);
        return BallisticSceneryStep {
            spawn: None,
            hide: false,
            #[cfg(test)]
            damage: None,
        };
    }

    let mut position = source.position;
    let initial_ground = terrain.ground_height(position);
    if position.z > initial_ground {
        crate::move_entity(&mut position, source.yaw, 0, source.actual_speed);
    } else {
        source.set_actual_speed(0);
        avoid_steep_tile(source, terrain, &mut position);
    }
    source.set_position(position);
    if source.actual_speed != 0 {
        source.set_actual_speed(source.actual_speed.wrapping_sub(1));
    }

    let vertical_speed = source.source_word_2c();
    position.z = position.z.wrapping_add(vertical_speed);
    source.set_source_word_2c(
        vertical_speed
            .wrapping_sub(VERTICAL_ACCELERATION)
            .clamp(MIN_VERTICAL_SPEED, MAX_VERTICAL_SPEED),
    );
    let ground = terrain.ground_height(position);
    if position.z <= ground {
        position.z = ground;
    }
    source.set_position(position);

    let damage_owner = source.source_upkeep_word_62();
    #[cfg(test)]
    let mut observed_damage = None;
    if damage_owner != 0 {
        let damage = source.source_upkeep_payload();
        if position.z <= ground {
            let magnitude = (damage >> 2).clamp(2, 192);
            let first = next_rng(source.entity_rng);
            let bounce = i32::from(first) % magnitude + magnitude;
            source.set_source_word_2c(bounce as i16);

            let second = next_rng(first);
            let half_bounce =
                u16::try_from(bounce >> 1).expect("positive ballistic bounce half-width fits u16");
            source.set_actual_speed(
                i16::try_from(second % half_bounce + 1).expect("ballistic rebound speed fits i16"),
            );

            let third = next_rng(second);
            source.set_yaw((third & 0x07ff) as i16);
            source.set_entity_rng(third);
            position.z = position.z.wrapping_add(bounce as i16);
            source.set_position(position);
        }
        let life_before = source.life;
        source.clear_source_upkeep_owner();
        source.set_life(life_before.wrapping_sub(damage));
        #[cfg(test)]
        {
            observed_damage = Some(((damage, damage_owner), life_before, source.life));
        }
    }

    let spawn = if source.life < 0 {
        Some(BallisticScenerySpawn::Model13Smoke)
    } else if source.position.z <= ground
        && terrain.material_mask(source.position) == crate::WATER_AMBIENCE_MASK
    {
        Some(BallisticScenerySpawn::Model5WaterSplash)
    } else {
        None
    };
    BallisticSceneryStep {
        spawn,
        hide: spawn.is_some(),
        #[cfg(test)]
        damage: observed_damage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_world::{Mc2Simulation, NativeWorld};
    use std::path::PathBuf;

    fn fixture() -> Mc2Simulation {
        let package = std::env::var_os("MC_LEVEL2_PACKAGE_V5").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/l2-debug3.fJ3PaM/level2")
            },
            PathBuf::from,
        );
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component")
            .level2_reset_session()
            .expect("qualified source session")
    }

    fn ballistic_row(session: &Mc2Simulation) -> (EntitySnapshot, SessionTerrain) {
        let mut source = session.session.state.live.economy.entities[100].clone();
        source.active = true;
        source.class = 2;
        source.model = 8;
        source.action = 21;
        source.record[2] = 1;
        source.record[67] = 2;
        source.record[68] = 8;
        source.record[73] = 21;
        source.set_control_flags(0);
        source.set_actual_speed(0);
        source.set_source_word_2c(-128);
        source.set_life(1_000);
        source.set_entity_rng(0x1234);
        let terrain = session.session.state.live.terrain.clone();
        let mut position = source.position;
        position.z = terrain.ground_height(position);
        source.set_position(position);
        (source, terrain)
    }

    #[test]
    fn constructor_uses_three_source_draws_and_cave_returns_before_allocation() {
        let mut session = fixture();
        let particles = session.session.definition.presentation.particles.clone();
        let live = &mut session.session.state.live;
        let initial = Position {
            x: 0x3280,
            y: 0x3480,
            z: 777,
        };
        let definition = Definition::default();
        for (kind, model, action, particle) in [
            (BallisticKind::Model7, 7, 20, 322),
            (BallisticKind::Model8, 8, 21, 323),
        ] {
            let birth_definition = if kind == BallisticKind::Model8 {
                Definition {
                    minimum_life: 700,
                    life_variation: 500,
                }
            } else {
                definition
            };
            let slot = construct(
                &mut live.economy,
                &mut live.terrain,
                initial,
                kind,
                birth_definition,
                &particles,
            )
            .unwrap()
            .expect("outdoor source allocator admits scenery");
            let seed = live.economy.economy.new_event_rng(slot);
            let first = next_rng(seed);
            let second = next_rng(first);
            let third = next_rng(second);
            let expected_position = Position {
                x: initial.x.wrapping_add((second & 63).wrapping_sub(32)),
                y: initial.y.wrapping_add((third & 63).wrapping_sub(32)),
                ..initial
            };
            let row = &live.economy.entities[usize::from(slot)];
            assert_eq!((row.class, row.model, row.action), (2, model, action));
            assert_eq!(row.control_flags, 12);
            let expected_life =
                birth_definition.minimum_life + i32::from(first % birth_definition.life_variation);
            assert_eq!((row.max_life, row.life), (expected_life, expected_life));
            assert_eq!((row.entity_rng, row.position), (third, expected_position));
            assert_eq!(
                (
                    row.actual_speed,
                    row.source_word_2c(),
                    row.record[60],
                    row.record[74]
                ),
                (0, -128, 1, 0)
            );
            assert_eq!(row.particle_parameter_index, Some(particle));
            assert!(live.economy.economy.map_chains().is_linked(slot));
            assert_eq!(
                live.economy.economy.map_chains().tile_of(slot),
                (expected_position.x >> 8) | (expected_position.y & 0xff00)
            );
            let mut imported = row.clone();
            let changed = Definition {
                minimum_life: 700,
                life_variation: 500,
            };
            imported.set_life(imported.life - 25);
            bind_initial(&mut imported, changed).unwrap();
            let changed_max = 700 + i32::from(first % 500);
            assert_eq!(
                (imported.max_life, imported.life, imported.entity_rng),
                (changed_max, changed_max - 25, third)
            );
        }

        let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level12");
        let mut cave = crate::campaign::CampaignSession::open(
            package,
            crate::campaign::spec::CampaignSelection::ordinary(12).unwrap(),
        )
        .expect("qualified Cave owner package")
        .simulation()
        .clone();
        let cave_particles = cave.session.definition.presentation.particles.clone();
        let cave = &mut cave.session.state.live;
        let before = cave.economy.economy.live_arena_slots().to_vec();
        let rng = cave.economy.global_rng;
        for kind in [BallisticKind::Model7, BallisticKind::Model8] {
            assert_eq!(
                construct(
                    &mut cave.economy,
                    &mut cave.terrain,
                    initial,
                    kind,
                    definition,
                    &cave_particles
                )
                .unwrap(),
                None
            );
        }
        assert_eq!(cave.economy.economy.live_arena_slots(), before.as_slice());
        assert_eq!(cave.economy.global_rng, rng);
    }

    #[test]
    fn ordinary_level_three_paid_fireballs_bounce_and_destroy_original_ballistic_scenery() {
        use crate::campaign::{
            CampaignSession,
            schedule::{CurrentOccupant, ScheduleReceipt},
            spec::CampaignSelection,
        };
        use crate::native_world::{Mc2DispatchEvent, area_damage, source_target};
        let package = std::env::var_os("MC_CAMPAIGN_PACKAGE_LEVEL3_V1").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3")
            },
            PathBuf::from,
        );
        let mut game = CampaignSession::open(package, CampaignSelection::LEVEL3)
            .expect("real Day Level3 owner package");
        let original_row = game.simulation().entities()[245].clone();
        assert_eq!(
            (
                original_row.class,
                original_row.model,
                original_row.action,
                original_row.max_life,
                original_row.life
            ),
            (2, 8, 21, 988, 988)
        );
        let mut rebound = original_row.clone();
        bind_initial(&mut rebound, Definition::default()).unwrap();
        assert_eq!(
            rebound, original_row,
            "source-default reset binding preserves the imported packet"
        );
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
        let sim = game.simulation();
        assert!(
            sim.dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        let target_slot = 245_u16;
        let target = sim
            .source_arena()
            .handle(target_slot)
            .expect("original scenery generation");
        let row = sim.entities()[usize::from(target_slot)].clone();
        assert_eq!(
            (row.class, row.model, row.action, row.life),
            (2, 8, 21, 988)
        );
        assert_ne!(
            sim.canonical_terrain().material_mask(row.position),
            crate::WATER_AMBIENCE_MASK
        );
        let original_trees = sim
            .entities()
            .iter()
            .filter(|tree| tree.active && (tree.class, tree.model) == (2, 0))
            .map(|tree| {
                (
                    sim.source_arena()
                        .handle(tree.index)
                        .expect("original Tree generation"),
                    tree.life,
                )
            })
            .collect::<Vec<_>>();
        let trees = original_trees
            .iter()
            .map(|(handle, _)| *handle)
            .collect::<Vec<_>>();
        assert!(
            !trees.is_empty(),
            "the real owner package has original live Trees"
        );
        let source_particle = row.particle_parameter_index.unwrap();
        assert_eq!(source_particle, 323);
        assert!(
            sim.campaign_entity_presentations(sim.state.yaw)
                .unwrap()
                .iter()
                .any(|dto| dto.entity_index == target_slot
                    && dto.render_parameter_index == source_particle)
        );
        let player = sim.state.entity_index;
        let player_handle = sim.source_arena().handle(player).unwrap();
        let holder = sim.shared_spells().local_player().enabled_spell_holders[0];
        let holder_handle = sim.source_arena().handle(holder).unwrap();
        let cost = crate::level_definition::FIREBALL_PROPERTIES.tiers[0].mana_cost;
        let holder_row = sim
            .shared_spells()
            .pickups()
            .iter()
            .chain(sim.shared_spells().holders())
            .chain(sim.shared_spells().dynamic_pickups())
            .find(|row| row.index == holder)
            .expect("current carried Fireball holder");
        assert_eq!(
            (
                holder_row.level_gate,
                holder_row.subspell_index,
                holder_row.max_mana
            ),
            (0, 250, cost)
        );
        assert!(sim.shared_spells().player_entity().mana >= 4 * cost);
        let view = source_target::read(
            sim.source_arena(),
            source_target::Context::default(),
            target_slot,
        )
        .unwrap()
        .unwrap();
        let shape = view.collision.expect("original ballistic collision axes");
        assert!(shape.half_width > 0 && shape.half_depth > 0 && shape.half_height > 0);
        let centre = Position {
            z: row
                .position
                .z
                .wrapping_add(row.array_yaw())
                .wrapping_sub(crate::level1_mission::FIREBALL_COLLISION_VERTICAL_OFFSET),
            ..row.position
        };
        let hand = Position {
            x: centre.x.wrapping_add(2048),
            y: centre.y,
            z: centre.z.wrapping_add(512),
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
        let dx = i32::from(origin.x.wrapping_sub(row.position.x) as i16);
        let dy = i32::from(origin.y.wrapping_sub(row.position.y) as i16);
        let dz = i32::from(origin.z.wrapping_sub(row.position.z));
        let distance = crate::legacy_sqrt(
            dx.wrapping_mul(dx)
                .wrapping_add(dy.wrapping_mul(dy))
                .wrapping_add(dz.wrapping_mul(dz)) as u32,
        );
        assert!(
            distance >= 1536,
            "fixed launch origin must be outside the source 1CE80/1CED0/1CF20 contact radii: origin={origin:?}, target={:?}, distance={distance}",
            row.position
        );
        assert!(
            origin.z
                >= sim
                    .canonical_terrain()
                    .ground_height(origin)
                    .wrapping_add(sim.profile().ground_clearance)
        );
        assert_eq!(
            crate::resolve_legacy_movement(
                &mut crate::FlightState::default(),
                origin,
                origin,
                sim.profile(),
                sim.canonical_terrain()
            ),
            crate::MovementResolution::Accept(origin)
        );
        game.simulation_mut()
            .set_level2_entity_position(player, origin)
            .unwrap();
        game.simulation_mut()
            .set_level2_player_aim_for_test(aim.0, aim.1);
        let mut no_cast = game.clone();
        let mut watched = vec![target];
        watched.extend(trees.iter().copied());
        let watched_slots = watched
            .iter()
            .map(|handle| handle.source_slot)
            .collect::<Vec<_>>();
        area_damage::start_actor_damage_trace(&watched);
        let (mut launches, mut impacts, mut terminals, mut accepted, mut consumed) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let (mut smoke, mut smoke_presented, mut smoke_hidden, mut smoke_reclaimed) =
            (None, false, false, false);
        let (mut target_hidden, mut target_reclaimed, mut bounced) = (false, false, false);
        let mut tree_consumed = Vec::new();
        let mut fire_births = Vec::new();
        let mut charged_launches = std::collections::BTreeSet::new();
        let mut debited = 0_i64;
        let mut counted_solid_impact = false;
        let mut elapsed = 0;
        for turn in 0..256 {
            let attempts_before = game.simulation().shared_spells().local_player().projectile_attempts;
            let target_was_hidden = target_hidden;
            let smoke_was_hidden = smoke_hidden;
            let before = game
                .simulation()
                .source_arena()
                .handle(target_slot)
                .filter(|handle| *handle == target)
                .map(|_| game.simulation().entities()[usize::from(target_slot)].clone());
            area_damage::start_fireball_launch_trace();
            crate::native_world::start_fireball_terminal_trace();
            area_damage::start_effect_damage_trace_for_targets(&watched_slots);
            super::super::trees::start_ground_fire_birth_trace_many(&trees);
            start_smoke_birth_trace(target);
            game.step(
                crate::PlayerCommand {
                    flags: if target_hidden { 0 } else { 0x10 },
                    ..Default::default()
                },
                0,
            );
            elapsed += 1;
            let new_launches = area_damage::take_fireball_launch_trace();
            let own_launch = new_launches.iter().any(|(_, _, owner)| *owner == player);
            let own_this_turn = new_launches
                .iter()
                .filter(|(_, _, owner)| *owner == player)
                .map(|(slot, generation, _)| (*slot, *generation))
                .collect::<Vec<_>>();
            launches.extend(new_launches);
            let new_impacts = area_damage::take_fireball_impact_birth_trace();
            let new_terminals = crate::native_world::take_fireball_terminal_trace();
            impacts.extend(new_impacts.iter().copied());
            terminals.extend(new_terminals.iter().copied());
            accepted.extend(
                area_damage::take_effect_damage_trace()
                    .into_iter()
                    .filter(|event| event.writer_sequence.is_some()),
            );
            let new_consumed = area_damage::take_player_damage_consumptions();
            fire_births.extend(super::super::trees::take_ground_fire_birth_trace_many());
            if let Some(child) = take_smoke_birth_trace() {
                smoke = Some(child);
            }
            let sim = game.simulation();
            assert!(
                sim.dispatch_events()
                    .iter()
                    .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "turn {turn}: {:?}",
                sim.dispatch_events()
            );
            if target_was_hidden {
                assert_ne!(
                    sim.source_arena().handle(target_slot),
                    Some(target),
                    "hidden original must be reclaimed at the next source boundary"
                );
                if sim.source_arena().handle(target_slot).is_none() {
                    assert!(!sim.economy().map_chains().is_linked(target_slot));
                }
            }
            if smoke_was_hidden {
                let child = smoke.expect("hidden child has a recorded generation");
                assert_ne!(
                    sim.source_arena().handle(child.source_slot),
                    Some(child),
                    "hidden Smoke13 must be reclaimed at the next source boundary"
                );
                if sim.source_arena().handle(child.source_slot).is_none() {
                    assert!(!sim.economy().map_chains().is_linked(child.source_slot));
                }
            }
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
                if slot == holder
                    && own_launch
                    && sim.source_arena().handle(holder) == Some(holder_handle)
                    && before.0 == player_handle
                    && after
                        == (
                            player_handle,
                            before.1,
                            if before.2 >= 0 {
                                -cost
                            } else {
                                before.2.wrapping_sub(cost)
                            },
                            before.3,
                        )
                {
                    charged_launches.extend(own_this_turn.iter().copied());
                }
                if slot == player
                    && before.0 == player_handle
                    && after.0 == player_handle
                    && before.2 <= -cost
                    && after.1 == before.1.wrapping_add(before.2).clamp(0, before.3)
                    && before.1 - after.1 >= cost
                {
                    debited += i64::from(before.1 - after.1);
                }
            }
            // The retained Day 3 producer trace places these ordinary solid
            // terminals at this source address. Join the actual generation to
            // both a paid launch and its own class-10 impact constructor.
            for terminal in new_terminals.iter().filter(|terminal|
                terminal.owner == player && terminal.collision_target.is_none()
                    && terminal.position == (Position { x: 38136, y: 26986, z: 3248 }))
            {
                let Some(impact) = terminal.impact else { continue; };
                let projectile = terminal.projectile;
                if charged_launches.contains(&(projectile.source_slot, projectile.generation))
                    && new_impacts.iter().any(|&(slot, generation, parent, parent_generation, owner)|
                        slot == impact.source_slot && generation == impact.generation
                            && parent == projectile.source_slot && parent_generation == projectile.generation
                            && owner == player)
                {
                    assert_ne!(sim.canonical_terrain().material_mask(terminal.position), 1);
                    assert!(sim.shared_spells().local_player().projectile_attempts > attempts_before,
                        "the paid solid impact must increment the source attempts counter");
                    counted_solid_impact = true;
                }
            }
            if new_consumed.iter().any(|hit| hit.target == target) {
                if let (Some(before), Some(current)) =
                    (&before, sim.source_arena().handle(target_slot))
                    && current == target
                {
                    let after = &sim.entities()[usize::from(target_slot)];
                    let third = next_rng(next_rng(next_rng(before.entity_rng)));
                    bounced |= after.entity_rng == third && after.source_word_2c() > 0;
                }
            }
            for hit in new_consumed {
                if hit.target == target {
                    consumed.push(hit);
                } else {
                    tree_consumed.push(hit);
                }
            }
            if sim.source_arena().handle(target_slot) == Some(target) {
                let now = &sim.entities()[usize::from(target_slot)];
                if now.drawing_disabled() {
                    target_hidden = true;
                    assert!(
                        !sim.campaign_entity_presentations(sim.state.yaw)
                            .unwrap()
                            .iter()
                            .any(|dto| dto.entity_index == target_slot)
                    );
                }
            } else if target_hidden {
                target_reclaimed = true;
            }
            if let Some(child) = smoke {
                if sim.source_arena().handle(child.source_slot) == Some(child) {
                    let row = &sim.entities()[usize::from(child.source_slot)];
                    smoke_hidden |= row.drawing_disabled();
                    smoke_presented |= sim
                        .campaign_entity_presentations(sim.state.yaw)
                        .unwrap()
                        .iter()
                        .any(|dto| {
                            dto.entity_index == child.source_slot
                                && dto.render_parameter_index == 67
                        });
                } else if smoke_hidden {
                    smoke_reclaimed = true;
                }
            }
            if target_reclaimed && smoke_reclaimed && debited >= i64::from(4 * cost) {
                break;
            }
        }
        assert!(counted_solid_impact, "the retained real solid impact must reach Results accuracy");
        let player_stats = game.simulation().shared_spells().local_player();
        assert!(player_stats.projectile_attempts > 0);
        assert_eq!(game.simulation().completion_statistics().hit_accuracy_percent,
            ((100_u64 * u64::from(player_stats.projectile_hits)
                / u64::from(player_stats.projectile_attempts)).min(100)) as i32);
        eprintln!(
            "ballistic actual prefix: origin={origin:?} initial_distance={distance} elapsed={elapsed} current={:?} life={} consumed={consumed:?} tree_consumed={tree_consumed:?} fire_births={fire_births:?} launches={launches:?} impacts={impacts:?} terminals={terminals:?} accepted={:?} bounce={bounced} target={target_hidden}/{target_reclaimed} smoke={smoke:?}/{smoke_presented}/{smoke_hidden}/{smoke_reclaimed} charged={charged_launches:?} debited={debited}",
            game.simulation().entities()[usize::from(target_slot)].position,
            game.simulation().entities()[usize::from(target_slot)].life,
            accepted
                .iter()
                .map(|event| (
                    event.target_slot,
                    event.source_slot,
                    event
                        .source_raw_impact
                        .as_ref()
                        .map(|(generation, _)| *generation),
                    event.writer_sequence,
                    event.pulse.owner,
                    event.pulse.payload,
                    event.mailbox_before,
                    event.mailbox_after,
                ))
                .collect::<Vec<_>>()
        );
        let mut contributing = std::collections::BTreeSet::new();
        let mut observed_target_fire = false;
        let mut pending = vec![target];
        let mut validated = Vec::new();
        // Every accepted write between two callbacks belongs to the exact
        // source mailbox interval. Ordinary 10C80 replaces unowned residue,
        // but adds to an already-owned payload; the last writer alone cannot
        // account for the earlier 50-point GroundFire pulses.
        let mut life = row.life;
        while let Some(watched_target) = pending.pop() {
            if validated.contains(&watched_target) {
                continue;
            }
            validated.push(watched_target);
            let hits = if watched_target == target {
                &consumed
            } else {
                &tree_consumed
            };
            let mut previous_sequence = 0;
            let mut previous_life = if watched_target == target {
                row.life
            } else {
                original_trees
                    .iter()
                    .find(|(handle, _)| *handle == watched_target)
                    .expect("required parent is an original Tree")
                    .1
            };
            for hit in hits.iter().filter(|hit| hit.target == watched_target) {
                let writer = hit
                    .writer
                    .expect("actual watched mailbox retains its latest writer");
                let writes = accepted
                    .iter()
                    .filter(|event| {
                        event.target_slot == watched_target.source_slot
                            && event.target_generation == watched_target.generation
                            && event.writer_sequence.is_some_and(|sequence| {
                                sequence > previous_sequence && sequence <= writer.sequence
                            })
                    })
                    .collect::<Vec<_>>();
                assert!(
                    !writes.is_empty(),
                    "every consumed mailbox has real accepted writes: {hit:?}"
                );
                let mut previous_after = None;
                for event in &writes {
                    assert!(event.source_scoped);
                    let before = event.mailbox_before.expect("actual pre-write mailbox");
                    let after = event.mailbox_after.expect("actual post-write mailbox");
                    if let Some(prior) = previous_after {
                        assert_eq!(before, prior);
                    } else {
                        assert_eq!(before.1, 0);
                    }
                    assert_eq!(event.pulse.owner, player);
                    let (generation, source) = event
                        .source_raw_impact
                        .as_ref()
                        .expect("current effect packet and generation at its real write");
                    let effect = crate::campaign::source_arena::SourceEntityHandle {
                        source_slot: event.source_slot.expect("scoped source slot"),
                        generation: *generation,
                    };
                    assert_eq!(
                        (source.index, source.class, source.source_id),
                        (effect.source_slot, 10, player)
                    );
                    match source.model {
                        0 => {
                            assert_eq!(event.pulse.payload, 250);
                            assert!(!event.pulse.continuous, "Fireball impact uses source mode0");
                            let terminal = terminals.iter().find(|terminal| terminal.owner == player && terminal.impact == Some(effect)
                                && launches.iter().any(|(slot, generation, owner)| *owner == player && terminal.projectile.source_slot == *slot && terminal.projectile.generation == *generation))
                                .expect("actual public Fireball launch reached this exact impact generation");
                            let projectile = (
                                terminal.projectile.source_slot,
                                terminal.projectile.generation,
                            );
                            assert!(
                                charged_launches.contains(&projectile),
                                "the contributing Fireball has its own reached holder charge"
                            );
                            contributing.insert(projectile);
                            assert!(impacts.iter().any(
                                |(impact, generation, parent, parent_generation, owner)| *owner
                                    == player
                                    && *impact == effect.source_slot
                                    && *generation == effect.generation
                                    && *parent == projectile.0
                                    && *parent_generation == projectile.1
                            ));
                        }
                        6 => {
                            assert_eq!(event.pulse.payload, 50);
                            assert!(
                                event.pulse.continuous,
                                "GroundFire uses source continuous mode"
                            );
                            observed_target_fire |= watched_target == target;
                            let birth = fire_births.iter().find(|birth| birth.child == effect)
                                .expect("the exact GroundFire generation came from a watched original Tree");
                            assert!(trees.contains(&birth.parent));
                            assert!(
                                tree_consumed.iter().any(|tree_hit| tree_hit.target
                                    == birth.parent
                                    && tree_hit.life_before >= 0
                                    && tree_hit.life_after < 0
                                    && tree_hit
                                        .writer
                                        .is_some_and(|writer| writer.sequence
                                            < event.writer_sequence.unwrap())),
                                "the same original Tree crossed its real lethal boundary before this GroundFire write"
                            );
                            pending.push(birth.parent);
                        }
                        other => {
                            panic!("unexpected contributing effect model {other} at {effect:?}")
                        }
                    }
                    let amount = if event.pulse.continuous && watched_target != target {
                        event.pulse.payload / 10
                    } else {
                        event.pulse.payload
                    };
                    let expected = if before.1 == 0 {
                        i32::from(amount)
                    } else {
                        before.0.wrapping_add(i32::from(amount))
                    };
                    assert_eq!(
                        after,
                        (expected, player),
                        "source Ordinary writer's owned accumulator"
                    );
                    previous_after = Some(after);
                }
                let last = writes.last().unwrap();
                assert_eq!(last.writer_sequence, Some(writer.sequence));
                let (last_generation, _) = last.source_raw_impact.as_ref().unwrap();
                assert_eq!(
                    writer.source,
                    area_damage::PlayerMailboxSource::Effect(
                        crate::campaign::source_arena::SourceEntityHandle {
                            source_slot: last.source_slot.unwrap(),
                            generation: *last_generation,
                        }
                    )
                );
                assert_eq!(hit.mailbox, previous_after.unwrap());
                assert_eq!(
                    (hit.life_before, hit.life_after),
                    (previous_life, previous_life.wrapping_sub(hit.mailbox.0))
                );
                previous_sequence = writer.sequence;
                previous_life = hit.life_after;
            }
            if watched_target == target {
                life = previous_life;
            } else {
                assert!(
                    previous_life < 0,
                    "required Tree's actual mailbox history reaches death"
                );
            }
        }
        assert!(
            observed_target_fire,
            "at least one exact Tree GroundFire generation damages the ballistic actor"
        );
        assert!(
            !contributing.is_empty(),
            "the actual collateral chain starts from paid Fireball impact(s)"
        );
        assert!(
            life < 0
                && bounced
                && target_hidden
                && target_reclaimed
                && smoke.is_some()
                && smoke_presented
                && smoke_hidden
                && smoke_reclaimed
                && debited >= i64::from(cost) * contributing.len() as i64,
            "life={life} hits={} bounce={bounced} target={target_hidden}/{target_reclaimed} smoke={smoke:?}/{smoke_presented}/{smoke_hidden}/{smoke_reclaimed} charged={charged_launches:?} contributing={contributing:?} debited={debited}",
            consumed.len()
        );
        for _ in 0..elapsed {
            no_cast.step(crate::PlayerCommand::default(), 0);
            assert!(
                no_cast
                    .simulation()
                    .dispatch_events()
                    .iter()
                    .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
            );
        }
        assert_eq!(
            no_cast.simulation().source_arena().handle(target_slot),
            Some(target)
        );
        assert_eq!(
            no_cast.simulation().entities()[usize::from(target_slot)].life,
            row.life
        );
        game.reset();
        let reset = &game.simulation().entities()[usize::from(target_slot)];
        assert_eq!(
            (
                reset.class,
                reset.model,
                reset.action,
                reset.max_life,
                reset.life,
                reset.entity_rng
            ),
            (
                original_row.class,
                original_row.model,
                original_row.action,
                original_row.max_life,
                original_row.life,
                original_row.entity_rng
            )
        );
    }

    #[test]
    fn cf14_ballistic_ground_damage_uses_exact_three_local_draws() {
        let session = fixture();
        let (mut source, terrain) = ballistic_row(&session);
        source.record[98..102].copy_from_slice(&400_i32.to_le_bytes());
        source.record[102..104].copy_from_slice(&7_u16.to_le_bytes());

        let first = next_rng(0x1234);
        let bounce = i32::from(first) % 100 + 100;
        let second = next_rng(first);
        let expected_speed = second % u16::try_from(bounce >> 1).unwrap() + 1;
        let third = next_rng(second);

        let step = step_ballistic_scenery(&mut source, &terrain);
        assert_eq!(step.spawn, None);
        assert!(!step.hide);
        assert_eq!(source.life, 600);
        assert_eq!(source.source_upkeep_word_62(), 0);
        assert_eq!(source.entity_rng, third);
        assert_eq!(source.source_word_2c(), bounce as i16);
        assert_eq!(source.actual_speed, expected_speed as i16);
        assert_eq!(source.yaw, (third & 0x07ff) as i16);
        assert_eq!(
            source.position.z,
            terrain
                .ground_height(source.position)
                .wrapping_add(bounce as i16)
        );
    }

    #[test]
    fn cf14_ballistic_airborne_lethal_damage_draws_no_local_rng_and_prefers_smoke() {
        let session = fixture();
        let (mut source, terrain) = ballistic_row(&session);
        source.set_life(50);
        source.set_source_word_2c(0);
        source.set_position(Position {
            z: source.position.z.wrapping_add(1_000),
            ..source.position
        });
        source.record[98..102].copy_from_slice(&100_i32.to_le_bytes());
        source.record[102..104].copy_from_slice(&7_u16.to_le_bytes());

        let step = step_ballistic_scenery(&mut source, &terrain);
        assert_eq!(step.spawn, Some(BallisticScenerySpawn::Model13Smoke));
        assert!(step.hide);
        assert_eq!(source.life, -50);
        assert_eq!(source.entity_rng, 0x1234);
        assert_eq!(source.source_upkeep_word_62(), 0);
    }

    #[test]
    fn cf14_ballistic_skip_once_and_water_contact_preserve_source_priority() {
        let session = fixture();
        let (mut skipped, terrain) = ballistic_row(&session);
        skipped.set_control_flags(SKIP_ONCE_CONTROL_BIT | 0x20);
        skipped.set_actual_speed(17);
        skipped.set_source_word_2c(91);
        let before = skipped.clone();

        let step = step_ballistic_scenery(&mut skipped, &terrain);
        assert_eq!(step.spawn, None);
        assert!(!step.hide);
        assert_eq!(skipped.position, before.position);
        assert_eq!(skipped.actual_speed, before.actual_speed);
        assert_eq!(skipped.source_word_2c(), before.source_word_2c());
        assert_eq!(skipped.life, before.life);
        assert_eq!(skipped.entity_rng, before.entity_rng);
        assert_eq!(skipped.control_flags, 0x20);

        let water_position = (0_u16..=u16::from(u8::MAX))
            .find_map(|tile_x| {
                (0_u16..=u16::from(u8::MAX)).find_map(|tile_y| {
                    let position = Position {
                        x: (tile_x << 8).wrapping_add(128),
                        y: (tile_y << 8).wrapping_add(128),
                        z: 0,
                    };
                    (terrain.material_mask(position) == crate::WATER_AMBIENCE_MASK)
                        .then_some(position)
                })
            })
            .expect("qualified outdoor terrain contains a water tile");
        let (mut water, _) = ballistic_row(&session);
        water.set_position(Position {
            z: terrain.ground_height(water_position),
            ..water_position
        });
        water.set_source_word_2c(0);
        let rng_before = water.entity_rng;

        let step = step_ballistic_scenery(&mut water, &terrain);
        assert_eq!(step.spawn, Some(BallisticScenerySpawn::Model5WaterSplash));
        assert!(step.hide);
        assert_eq!(water.entity_rng, rng_before);
    }
}
