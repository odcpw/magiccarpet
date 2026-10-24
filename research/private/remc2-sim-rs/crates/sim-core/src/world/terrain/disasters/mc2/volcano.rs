//! Volcano source rows and the synchronous damage/eruption boundaries.

use super::AreaDamage;
use crate::{
    FlightWorld, Position,
    campaign::source_arena::SourceArena,
    native_world::{SessionTerrain, cave_features::new_event},
    world::terrain::disasters::volcano::{self, Controller},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Eruption {
    pub(crate) position: Position,
    pub(crate) owner: u16,
    pub(crate) alternate: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Visit {
    pub(crate) hide: bool,
    pub(crate) terrain_mutated: bool,
    pub(crate) hit_count: u16,
    pub(crate) rumble: bool,
    pub(crate) alternate_rumble: bool,
    pub(crate) child: Option<u16>,
}

/// NewAdd0A09_4E760. maxLife is the radius parameter, not the duration.
/// Its caller resets the source-global alternate volcano mode on success.
pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
) -> Option<u16> {
    let slot = arena.allocate_source_slot_with_terrain(terrain)?;
    let mut source = new_event(slot, arena.economy.new_event_rng(slot), position);
    source.class = 10;
    source.model = 9;
    source.record[67] = 10;
    source.record[68] = 9;
    source.set_source_action(9);
    source.set_max_life(11);
    source.set_life(17);
    source.set_subspell_index(2000);
    source.set_control_flags(source.control_flags & !8);
    source.record[88..90].copy_from_slice(&7_i16.to_le_bytes());
    source.record[90..92].copy_from_slice(&7_i16.to_le_bytes());
    source.record[92..94].copy_from_slice(&16384_i16.to_le_bytes());
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.entities[usize::from(slot)] = source;
    Some(slot)
}

/// sub_31940 publishes geometry and controller writes before area116A0,
/// then audio, the center cap and the actual eruption child constructor.
pub(crate) fn step<F, G>(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    alternate: bool,
    mut apply_damage: F,
    mut erupt: G,
) -> Result<Visit, String>
where
    F: FnMut(&mut SourceArena, AreaDamage) -> Result<u16, String>,
    G: FnMut(&mut SourceArena, &mut SessionTerrain, Eruption) -> Result<Option<u16>, String>,
{
    let source = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && (row.class, row.model, row.action) == (10, 9, 9))
        .ok_or("volcano source row is not live")?;
    let mut controller = Controller {
        position: source.position,
        radius_parameter: source.max_life,
        pitch: i16::from_le_bytes([source.record[88], source.record[89]]),
        amplitude: i16::from_le_bytes([source.record[48], source.record[49]]),
        phase: source.record[74],
        life: source.life,
    };
    let visit = volcano::step(terrain, &mut controller, alternate);
    source.set_position(controller.position);
    source.set_life(controller.life);
    source.record[74] = controller.phase;
    source.record[48..50].copy_from_slice(&controller.amplitude.to_le_bytes());
    source.record[88..90].copy_from_slice(&controller.pitch.to_le_bytes());
    source.record[90..92].copy_from_slice(&controller.pitch.to_le_bytes());
    if visit.hide {
        source.disable_drawing();
    }
    let damage = AreaDamage {
        source_slot: slot,
        position: source.position,
        owner: source.source_id,
        payload: source.subspell_index,
        half_width: controller.pitch,
        half_depth: controller.pitch,
        vertical_offset: i16::from_le_bytes([source.record[86], source.record[87]]),
        half_height: i16::from_le_bytes([source.record[92], source.record[93]]),
        experience_spell: Some(18),
    };
    let alternate_rumble = alternate && source.phase & 3 == 0;
    let owner = source.source_id;
    let hit_count = if visit.damage {
        apply_damage(arena, damage)?
    } else {
        0
    };
    let child = if visit.eruption {
        volcano::finish_eruption(terrain, &controller);
        let mut position = controller.position;
        position.z = terrain.ground_height(position);
        erupt(
            arena,
            terrain,
            Eruption {
                position,
                owner,
                alternate,
            },
        )?
    } else {
        None
    };
    Ok(Visit {
        hide: visit.hide,
        terrain_mutated: visit.terrain_mutated,
        hit_count,
        rumble: visit.rumble,
        alternate_rumble: visit.rumble && alternate_rumble,
        child,
    })
}
