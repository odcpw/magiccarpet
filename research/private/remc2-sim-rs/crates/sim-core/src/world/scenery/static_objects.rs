//! MC2 static scenery4/5: real constructors, source-empty scheduled callbacks.

use crate::{
    Position,
    campaign::source_arena::SourceArena,
    native_world::{EntitySnapshot, ParticleParameter, SessionTerrain},
    world::terrain::cave_features::{bind_particle, new_event},
};

pub(crate) use sim_definition::level_definition::StaticSceneryDefinition as Definition;

const PARTICLE: u16 = 48;

/// These identities terminate at the MC2 adapter; the public role is static scenery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    Model4,
    Model5,
}

impl Kind {
    fn fields(self) -> (u8, u8) {
        match self {
            Self::Model4 => (4, 12),
            Self::Model5 => (5, 15),
        }
    }
}

fn valid(row: &EntitySnapshot) -> bool {
    row.active && matches!((row.class, row.model, row.action), (2, 4, 12) | (2, 5, 15))
}

/// Bind immutable properties to an imported carrier without erasing prior damage.
pub(crate) fn bind_initial(row: &mut EntitySnapshot, definition: Definition) -> Result<(), String> {
    definition.validate()?;
    if !valid(row) {
        return Err("static scenery definition requires its active source carrier".into());
    }
    let deficit = row.max_life.wrapping_sub(row.life);
    row.set_max_life(definition.max_life);
    row.set_life(definition.max_life.wrapping_sub(deficit));
    Ok(())
}

/// sub_4AF00/sub_4AF70: no environment gate and no constructor RNG draws.
pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    kind: Kind,
    definition: Definition,
    particles: &[ParticleParameter],
) -> Result<Option<u16>, String> {
    definition.validate()?;
    let particle = *particles
        .get(usize::from(PARTICLE))
        .ok_or("static scenery requires retained particle48")?;
    if particle.animation_limit().is_none() {
        return Err("static scenery particle48 has an unsupported render mode".into());
    }
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut row = new_event(slot, arena.economy.new_event_rng(slot), position);
    let (model, action) = kind.fields();
    row.class = 2;
    row.model = model;
    row.record[67] = 2;
    row.record[68] = model;
    row.set_source_action(action);
    // AddEventToMap sets bit2, retaining NewEvent's low-byte bit3.
    row.set_control_flags(row.control_flags | 4);
    row.set_source_dword_10(i32::from(slot % 11));
    row.set_max_life(definition.max_life);
    row.set_life(row.max_life);
    bind_particle(&mut row, PARTICLE, particle)?;
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    Ok(Some(slot))
}

/// Source dispatcher246150/246180 bodies are empty. Validate the current row,
/// leaving even negative life/mailbox/pose unchanged; the scheduler advances
/// phase afterwards and reclaims a hidden row only at its next owning boundary.
pub(crate) fn step(arena: &SourceArena, slot: u16) -> Result<(), String> {
    if arena.entities.get(usize::from(slot)).is_some_and(valid) {
        Ok(())
    } else {
        Err(format!(
            "static scenery callback lost its source carrier {slot}"
        ))
    }
}
