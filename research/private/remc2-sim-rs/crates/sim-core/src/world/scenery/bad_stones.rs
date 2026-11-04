//! MC2 BadStone construction and action9 on the shared source arena.
//!
//! Both authored births and reset-imported stones use the same live row;
//! the source scheduler controls visit order and subsequent reclamation.

use crate::{
    FlightWorld, Position,
    campaign::source_arena::SourceArena,
    native_world::{ParticleParameter, SessionTerrain},
    world::terrain::cave_features::{bind_particle, new_event},
};

/// Typed source-default properties and retained presentation binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Definition {
    pub(crate) max_life: i32,
    pub(crate) particle: u16,
}

pub(crate) const MC2_DEFAULT: Definition = Definition {
    max_life: 300,
    particle: 270,
};

/// sub_4AE80: NewEvent, physical-slot remainder, AddEventToMap, then the
/// source-default lifetime and retained particle270 geometry.
pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    definition: Definition,
    particles: &[ParticleParameter],
) -> Result<Option<u16>, String> {
    let particle = *particles
        .get(usize::from(definition.particle))
        .ok_or("BadStone requires its retained particle")?;
    if particle.animation_limit().is_none() {
        return Err(format!(
            "BadStone particle {} has unsupported render mode {}",
            definition.particle, particle.render_mode
        ));
    }
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut row = new_event(slot, arena.economy.new_event_rng(slot), position);
    row.set_control_flags(4);
    row.set_source_dword_10(i32::from(slot % 11));
    row.class = 2;
    row.model = 3;
    row.record[67] = 2;
    row.record[68] = 3;
    row.set_source_action(9);
    row.set_max_life(definition.max_life);
    row.set_life(row.max_life);
    bind_particle(&mut row, definition.particle, particle)?;
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, true)?;
    Ok(Some(slot))
}

/// sub_65110. The scheduler owns the enabled dispatcher's common phase tail.
/// This callback reads and updates the current fixed-address source carrier;
/// it performs no allocation, damage, RNG draw or lifetime change.
pub(crate) fn step(
    arena: &mut SourceArena,
    terrain: &SessionTerrain,
    slot: u16,
) -> Result<(), String> {
    let row = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && (row.class, row.model, row.action) == (2, 3, 9))
        .ok_or_else(|| format!("BadStone source slot {slot} is not a live action9 row"))?;
    row.set_control_flags(row.control_flags | 0x0002_0000);
    let mut position = row.position;
    position.z = terrain.ground_height(position);
    row.set_position(position);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_and_scheduled_step_keep_source_fields_and_map_links() {
        let package = std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
            },
            std::path::PathBuf::from,
        );
        let mut simulation = crate::campaign::CampaignSession::open(
            package,
            crate::campaign::spec::CampaignSelection::LEVEL2,
        )
        .expect("qualified Level2 campaign")
        .simulation()
        .clone();
        let level = simulation.session.definition.semantic_level().unwrap();
        assert_eq!(
            level.authored_scenery_count(crate::level_definition::SceneryKind::Stone),
            5,
            "the owner level has one Stone and four BadStones"
        );
        for (id, x, y) in [
            ("scenery.stone.at_2_203", 2, 203),
            ("scenery.stone.at_2_103", 2, 103),
            ("scenery.stone.at_51_154", 51, 154),
            ("scenery.stone.at_208_154", 208, 154),
        ] {
            let entity = level
                .entities()
                .iter()
                .find(|row| row.id().as_str() == id)
                .expect("authored BadStone has its sealed semantic identity");
            assert_eq!(
                entity.kind(),
                crate::level_definition::LevelEntityKind::Scenery(
                    crate::level_definition::SceneryKind::Stone
                )
            );
            assert_eq!(
                entity.position(),
                crate::level_definition::TilePosition { x, y }
            );
        }
        let particles = simulation.session.definition.presentation.particles.clone();
        let live = &mut simulation.session.state.live;
        let position = Position {
            x: 0x3280,
            y: 0x3480,
            z: 777,
        };
        let slot = construct(
            &mut live.economy,
            &mut live.terrain,
            position,
            MC2_DEFAULT,
            &particles,
        )
        .unwrap()
        .expect("source allocator admits BadStone");
        let row = &live.economy.entities[usize::from(slot)];
        assert_eq!((row.class, row.model, row.action), (2, 3, 9));
        assert_eq!((row.max_life, row.life, row.actual_speed), (300, 300, 16));
        assert_eq!(
            (row.source_dword_10(), row.phase),
            (i32::from(slot % 11), slot as u8)
        );
        assert_eq!(
            (
                row.control_flags,
                row.particle_parameter_index,
                row.parameter_relation
            ),
            (4, Some(270), Some(59))
        );
        assert_eq!(
            (row.array_pitch(), row.array_fov()),
            (
                (particles[270].speed / 2) as i16,
                (particles[270].rotation_speed / 2) as i16
            )
        );
        let tile = usize::from(position.x >> 8) + 256 * usize::from(position.y >> 8);
        assert_eq!(live.terrain.planes().entity_indices[tile], slot as i16);
        assert_eq!(live.economy.map_chains.tile_head(tile as u16), slot);
        let rng = row.entity_rng;
        let phase = row.phase;
        let expected_z = live.terrain.ground_height(position);
        step(&mut live.economy, &live.terrain, slot).unwrap();
        let row = &live.economy.entities[usize::from(slot)];
        assert_eq!(
            row.position,
            Position {
                z: expected_z,
                ..position
            }
        );
        assert_eq!(row.phase, phase, "scheduler owns the common phase tail");
        assert_eq!(row.control_flags, 0x0002_0004);
        assert_eq!((row.life, row.entity_rng), (300, rng));
        assert_eq!(live.terrain.planes().entity_indices[tile], slot as i16);
    }
}
