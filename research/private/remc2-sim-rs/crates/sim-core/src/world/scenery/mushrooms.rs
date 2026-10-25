//! Cave mushrooms: sub_4AFE0, sub_651B0 and sub_65240.
//!
//! The source entity is the only mutable carrier. The caller retains source
//! dispatch order, damage-mailbox consumption, smoke allocation and reclaim.

use crate::{
    FlightWorld, Position,
    campaign::source_arena::SourceArena,
    native_world::{EntitySnapshot, ParticleParameter, SessionTerrain},
    renderable_map_chain::RenderableMapSeed,
    world::terrain::cave_features::{bind_particle, new_event, synchronize_links},
};

pub(crate) use sim_definition::level_definition::MushroomDefinition as Definition;

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Outcome {
    /// Captured before the source callback snaps the mushroom to the floor.
    pub(crate) smoke_position: Option<Position>,
    pub(crate) damage_consumed: bool,
}

/// `damage` is Some only when the source damage-owner word is nonzero.
/// A lethally damaged dry mushroom remains as action19; water hides both
/// forms, and the ordinary next source pass reclaims that hidden row.
pub(crate) fn step(
    source: &mut EntitySnapshot,
    terrain: &impl FlightWorld,
    damage: Option<i32>,
    particles: &[ParticleParameter],
) -> Result<Outcome, String> {
    let mut outcome = Outcome::default();
    match source.action {
        18 => {
            source.set_control_flags(source.control_flags | 0x0002_0000);
            if let Some(damage) = damage {
                source.set_life(source.life.wrapping_sub(damage));
                outcome.damage_consumed = true;
                if source.life < 0 {
                    let particle = source
                        .particle_parameter_index
                        .ok_or_else(|| "live mushroom has no particle binding".to_owned())?;
                    source.set_source_action(19);
                    source.set_control_flags(source.control_flags & !8);
                    bind(source, particle.wrapping_add(4), particles)?;
                    outcome.smoke_position = Some(source.position);
                }
            }
        }
        19 => {}
        action => return Err(format!("unsupported mushroom action {action}")),
    }
    let mut position = source.position;
    position.z = terrain.ground_height(position);
    source.set_position(position);
    if terrain.material_mask(position) == crate::WATER_AMBIENCE_MASK {
        source.disable_drawing();
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Tile,
        native_world::{NativeWorld, NativeWorldInspection},
    };
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

    struct Ground {
        water: bool,
    }
    impl FlightWorld for Ground {
        fn ground_height(&self, _: Position) -> i16 {
            120
        }
        fn ceiling_height(&self, _: Position) -> Option<i16> {
            Some(1000)
        }
        fn material_mask(&self, _: Position) -> u32 {
            if self.water {
                crate::WATER_AMBIENCE_MASK
            } else {
                0
            }
        }
        fn angle_flags(&self, _: Tile) -> u8 {
            0
        }
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
        let initial = mushrooms(game.level2_compatibility().entities());
        assert_eq!(initial.len(), 56);
        for row in &initial {
            let mut rebound = row.clone();
            bind_initial(&mut rebound, Definition::default()).unwrap();
            assert_eq!(&rebound, row, "source default binding changes no row bytes");
        }
        game.step(crate::PlayerCommand::default(), 0);
        let live = mushrooms(game.level2_compatibility().entities());
        assert!(!live.is_empty(), "ordinary Cave step retains dry mushrooms");
        assert!(live.iter().all(|row| matches!(row.action, 18 | 19)));
        game.reset();
        assert_eq!(mushrooms(game.level2_compatibility().entities()), initial);
    }

    #[test]
    fn damage_keeps_dry_stalk_and_water_hides_both_forms() {
        let world = owner_world();
        let particles = &world.presentation().particles;
        let mut source = new_event(
            30,
            71,
            Position {
                x: 123,
                y: 456,
                z: 321,
            },
        );
        source.class = 2;
        source.model = 6;
        source.set_source_action(18);
        source.set_life(100);
        bind(&mut source, 324, particles).unwrap();
        let dry = Ground { water: false };
        let untouched = step(&mut source, &dry, None, particles).unwrap();
        assert_eq!(untouched, Outcome::default());
        assert_eq!(source.life, 100);
        assert_eq!(source.position.z, 120);
        assert_ne!(source.control_flags & 0x20000, 0);
        let first_hit = step(&mut source, &dry, Some(100), particles).unwrap();
        assert!(first_hit.damage_consumed);
        assert_eq!(first_hit.smoke_position, None);
        assert_eq!((source.life, source.action), (0, 18));
        source.set_position(Position {
            z: 321,
            ..source.position
        });
        let death_position = source.position;
        let lethal = step(&mut source, &dry, Some(1), particles).unwrap();
        assert_eq!(lethal.smoke_position, Some(death_position));
        assert!(lethal.damage_consumed);
        assert_eq!((source.life, source.action), (-1, 19));
        assert_eq!(source.particle_parameter_index, Some(328));
        assert_eq!(source.control_flags & 8, 0);
        assert!(!source.drawing_disabled());
        assert_eq!(
            step(&mut source, &dry, Some(99), particles).unwrap(),
            Outcome::default()
        );
        assert_eq!(
            source.life, -1,
            "action19 does not consume another damage mailbox"
        );
        step(&mut source, &Ground { water: true }, None, particles).unwrap();
        assert!(source.drawing_disabled());
        source.set_control_flags(8);
        source.set_source_action(18);
        step(&mut source, &Ground { water: true }, None, particles).unwrap();
        assert!(source.drawing_disabled());
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
