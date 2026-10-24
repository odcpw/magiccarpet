//! MC2 Tremor construction and source-row publication. The caller's pulse
//! callback performs sound10, 10C80 and XP15 before the final life decrement.

use crate::{
    Position,
    campaign::source_arena::SourceArena,
    native_world::{
        SessionTerrain,
        cave_features::{new_event, synchronize_links},
    },
    renderable_map_chain::RenderableMapSeed,
    world::terrain::disasters::tremor::{self, Controller},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProjectilePulse {
    pub(crate) source_slot: u16,
    pub(crate) position: Position,
    pub(crate) owner: u16,
    pub(crate) payload: u16,
    pub(crate) half_width: i16,
    pub(crate) half_depth: i16,
    pub(crate) vertical_offset: i16,
    pub(crate) half_height: i16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Visit {
    pub(crate) hide: bool,
    pub(crate) terrain_mutated: bool,
    pub(crate) hit_count: u16,
}

/// sub_51790. This is an ordinary map-linked controller, not a visual proxy.
pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
) -> Result<Option<u16>, String> {
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut row = new_event(slot, arena.economy.new_event_rng(slot), position);
    row.class = 10;
    row.model = 71;
    row.record[67] = 10;
    row.record[68] = 71;
    row.set_source_action(78);
    row.set_max_life(120);
    row.set_life(120);
    row.set_subspell_index(20_000);
    row.set_control_flags((row.control_flags & !9) | 5);
    row.record[74] = 0;
    row.record[88..90].copy_from_slice(&1280_i16.to_le_bytes());
    row.record[90..92].copy_from_slice(&1280_i16.to_le_bytes());
    row.record[92..94].copy_from_slice(&2048_i16.to_le_bytes());
    if let Err(error) = arena.economy.map_chains.sync_entity(RenderableMapSeed {
        entity_index: slot,
        active: true,
        x: position.x,
        y: position.y,
    }) {
        arena.economy.release_slot(slot, Some(row.entity_rng));
        return Err(error);
    }
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    synchronize_links(arena, terrain, slot);
    Ok(Some(slot))
}

pub(crate) fn step<F>(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    mut apply_pulse: F,
) -> Result<Visit, String>
where
    F: FnMut(&mut SourceArena, ProjectilePulse) -> Result<u16, String>,
{
    let source = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|source| {
            source.active && (source.class, source.model, source.action) == (10, 71, 78)
        })
        .ok_or("Tremor source slot is not live")?;
    let mut controller = Controller {
        position: source.position,
        max_life: source.max_life,
        life: source.life,
        damage: source.subspell_index,
        phase: source.record[74],
        eighth_life: i16::from_le_bytes([source.record[48], source.record[49]]),
        clock: i32::from_le_bytes(source.record[20..24].try_into().expect("source dword10")),
        rng: source.entity_rng,
    };
    let visit = tremor::prepare(terrain, &mut controller)?;
    source.set_subspell_index(controller.damage);
    source.record[74] = controller.phase;
    source.record[48..50].copy_from_slice(&controller.eighth_life.to_le_bytes());
    source.set_source_dword_10(controller.clock);
    source.set_entity_rng(controller.rng);
    if visit.hide {
        source.disable_drawing();
        return Ok(Visit {
            hide: true,
            ..Visit::default()
        });
    }
    let pulse = visit.damage_radius.map(|radius| {
        source.record[88..90].copy_from_slice(&radius.to_le_bytes());
        source.record[90..92].copy_from_slice(&radius.to_le_bytes());
        source.record[92..94].copy_from_slice(&2048_i16.to_le_bytes());
        ProjectilePulse {
            source_slot: slot,
            position: source.position,
            owner: source.source_id,
            payload: source.subspell_index,
            half_width: radius,
            half_depth: radius,
            vertical_offset: i16::from_le_bytes([source.record[86], source.record[87]]),
            half_height: 2048,
        }
    });
    let hit_count = if let Some(pulse) = pulse {
        apply_pulse(arena, pulse)?
    } else {
        0
    };
    tremor::finish(&mut controller);
    arena.entities[usize::from(slot)].set_life(controller.life);
    Ok(Visit {
        hide: false,
        terrain_mutated: visit.terrain_mutated,
        hit_count,
    })
}
