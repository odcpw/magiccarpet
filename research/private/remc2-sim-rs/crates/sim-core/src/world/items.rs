//! Passive items, experience scrolls and visible portals in the source arena.
//!
//! Construction owns the item packet. The source scheduler owns its common
//! phase tail and following-pass reclamation. Shared spell rewards retain
//! their own state owner; passive items invent no pickup.

use crate::{
    FlightWorld, Position,
    campaign::source_arena::SourceArena,
    native_world::{
        EntitySnapshot, Level2SharedSpellResetSession, MapType, ParticleParameter, SessionTerrain,
    },
    renderable_map_chain::RenderableMapSeed,
    world::terrain::cave_features::{bind_particle, new_event},
};
use sim_definition::adapter::PASSIVE_ITEM_SOURCE_BINDING as SOURCE;

pub(crate) use sim_definition::level_definition::PassiveItemDefinition as Definition;

pub(crate) mod reward_source;

/// One authoritative authored scroll or visible portal incarnation. The arena
/// stores this state; item behavior and construction defaults live here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredClass14Source {
    pub source_entity_index: u16,
    pub entity_rng: u16,
    pub authored_index: u16,
    pub model: u8,
    pub experience_award: Option<i32>,
    pub action: u8,
    pub particle: u16,
    pub control_flags: u8,
    pub shift_rot_x: u16,
    pub shift_rot_y: u16,
    pub activation_group: i16,
    pub stage_tag: i16,
    pub position: Position,
    pub phase: u8,
    /// Drawing is separate from allocation until the next source boundary.
    pub drawing_enabled: bool,
    pub active: bool,
}

pub(crate) struct AuthoredConstructor {
    model: u8,
    action: u8,
    particle: u16,
    flags: u8,
    width: u16,
    height: u16,
    award: Option<i32>,
}

impl AuthoredConstructor {
    /// Source51570/515C0/51610. Validate immutable content before NewEvent.
    pub(crate) fn for_source(model: u8, award: Option<i32>) -> Option<Self> {
        let (action, particle, flags, width, height) = match model {
            3 => (8, 338, 1, 0, 0),
            4 => (9, 339, 1, 0, 0),
            5 => (10, 280, 0, 768, 1280),
            _ => return None,
        };
        if (model == 5) != award.is_some_and(|value| value > 0) {
            return None;
        }
        Some(Self {
            model,
            action,
            particle,
            flags,
            width,
            height,
            award,
        })
    }

    pub(crate) fn construct(
        self,
        slot: u16,
        rng: u16,
        authored_index: u16,
        activation_group: i16,
        stage_tag: i16,
        position: Position,
    ) -> Level2AuthoredClass14Source {
        Level2AuthoredClass14Source {
            source_entity_index: slot,
            entity_rng: rng,
            authored_index,
            model: self.model,
            experience_award: self.award,
            action: self.action,
            particle: self.particle,
            control_flags: self.flags,
            shift_rot_x: self.width,
            shift_rot_y: self.height,
            activation_group,
            stage_tag,
            position,
            phase: slot as u8,
            drawing_enabled: true,
            active: true,
        }
    }
}

impl Level2AuthoredClass14Source {
    pub(crate) fn follow_ground(&mut self, height: i16) {
        self.position.z = height;
    }
    pub(crate) fn hide(&mut self) {
        self.drawing_enabled = false;
    }
    pub(crate) fn suppress_scroll(&mut self) {
        self.control_flags |= 1;
        self.hide();
    }
    pub(crate) fn reveal_portal(&mut self) -> bool {
        if !self.active || !matches!(self.model, 3 | 4) {
            return false;
        }
        self.control_flags &= !1;
        true
    }
}

/// Ordinary DrawSprites consumes the current item carrier's bound particle.
/// The arena's raw packet may be an inactive shadow at a typed item's slot.
/// NewEvent leaves visual yaw zero; SetHalfSpeed changes collision arrays.
pub(crate) fn sprite_sources(
    items: &[Level2AuthoredClass14Source],
) -> impl Iterator<Item = (u16, u16, u16, i16, Position, u32)> + '_ {
    items
        .iter()
        .filter(|item| item.active && item.drawing_enabled && item.control_flags & 0x21 == 0)
        .map(|item| {
            (
                item.source_entity_index,
                item.source_entity_index,
                item.particle,
                0,
                item.position,
                12 | u32::from(item.control_flags),
            )
        })
}

#[derive(Clone, Copy)]
pub(crate) struct ScrollPlayer {
    pub(crate) slot: u8,
    pub(crate) source_slot: u16,
    pub(crate) position: Position,
    pub(crate) life: i32,
}

/// Source59C40/59C60 and UpdateScroll59C80. Shared rewards keep their own
/// spell-state owner; this visit owns the item height, overlap visit and hide.
/// The scheduler retains the common phase tail and following-pass retirement.
pub(crate) fn visit_authored(
    arena: &mut SourceArena,
    terrain: &SessionTerrain,
    shared: &mut Level2SharedSpellResetSession,
    source_slot: u16,
    player: ScrollPlayer,
) -> bool {
    let Some(source) = arena.economy.authored_class14_mut(source_slot) else {
        return false;
    };
    if source.model == 5 && !shared.mission_rewards().allows_experience() {
        source.suppress_scroll();
        return false;
    }
    source.follow_ground(terrain.ground_height(source.position));
    let (model, award, position) = (source.model, source.experience_award, source.position);
    if arena
        .economy
        .map_chains
        .sync_entity(RenderableMapSeed {
            entity_index: source_slot,
            active: true,
            x: position.x,
            y: position.y,
        })
        .is_err()
    {
        return false;
    }
    let collected = model == 5
        && shared.collect_level2_experience_scroll(
            source_slot,
            award.expect("live scroll carries validated reward"),
            position,
            player.slot,
            player.source_slot,
            player.position,
            player.life,
            &arena.entities,
            terrain.map_type() == MapType::Cave,
        );
    if collected {
        arena
            .economy
            .authored_class14_mut(source_slot)
            .expect("same owning item visit")
            .hide();
    }
    !collected
}

fn valid(row: &EntitySnapshot) -> bool {
    row.active && (row.class, row.model, row.action) == (SOURCE.class, SOURCE.model, SOURCE.action)
}

fn bind_shape(row: &mut EntitySnapshot, definition: Definition) {
    row.set_source_overlap_half_extents(
        definition.horizontal_half_extent,
        definition.horizontal_half_extent,
    );
    row.record[92..94].copy_from_slice(&definition.vertical_half_extent.to_le_bytes());
}

/// Adopt immutable properties without reconstructing an imported incarnation.
pub(crate) fn bind_initial(row: &mut EntitySnapshot, definition: Definition) -> Result<(), String> {
    definition.validate()?;
    if !valid(row) {
        return Err("passive item definition requires its active source carrier".into());
    }
    let deficit = row.max_life.wrapping_sub(row.life);
    row.set_max_life(definition.max_life);
    row.set_life(definition.max_life.wrapping_sub(deficit));
    bind_shape(row, definition);
    Ok(())
}

/// MC2 sub51530 -> sub514E0: NewEvent, map link, CopyMaxLifeToLife,
/// SetHalfSpeed(particle77), then SetEntityShiftRot(384,384). No extra RNG
/// draw or terrain-height write follows the caller's construction position.
pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    definition: Definition,
    particles: &[ParticleParameter],
) -> Result<Option<u16>, String> {
    definition.validate()?;
    let particle = *particles
        .get(usize::from(SOURCE.descriptor))
        .ok_or("passive item requires retained particle77")?;
    if particle.animation_limit().is_none() {
        return Err("passive item particle77 has an unsupported render mode".into());
    }
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut row = new_event(slot, arena.economy.new_event_rng(slot), position);
    row.class = SOURCE.class;
    row.model = SOURCE.model;
    row.record[67] = SOURCE.class;
    row.record[68] = SOURCE.model;
    row.set_source_action(SOURCE.action);
    // AddEventToMap sets bit2 and retains NewEvent's bit3.
    row.set_control_flags(row.control_flags | 4);
    row.set_max_life(definition.max_life);
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    let row = &mut arena.entities[usize::from(slot)];
    row.set_life(row.max_life);
    bind_particle(row, SOURCE.descriptor, particle)?;
    bind_shape(row, definition);
    Ok(Some(slot))
}

/// Enabled source action0 is empty. Even queued damage or negative life does
/// not introduce motion, a reward or a death branch. Phase is scheduler-owned.
pub(crate) fn step(arena: &SourceArena, slot: u16) -> Result<(), String> {
    if arena.entities.get(usize::from(slot)).is_some_and(valid) {
        Ok(())
    } else {
        Err(format!(
            "passive item callback lost its source carrier {slot}"
        ))
    }
}

/// Imported source portals share the same family ownership. Their height
/// write is the callback; passive item0 preserves its original position.
pub(crate) fn visit_imported(
    arena: &mut SourceArena,
    terrain: &SessionTerrain,
    slot: u16,
) -> Result<(), String> {
    let source = arena
        .entities
        .get(usize::from(slot))
        .filter(|source| source.active && source.class == 14)
        .ok_or_else(|| format!("imported item callback lost source slot {slot}"))?;
    if (source.model, source.action) == (0, 0) {
        return step(arena, slot);
    }
    if !matches!((source.model, source.action), (3, 8) | (4, 9)) {
        return Err(format!(
            "imported portal {slot} has invalid model/action {}/{}",
            source.model, source.action
        ));
    }
    let mut position = source.position;
    position.z = terrain.ground_height(position);
    arena.entities[usize::from(slot)].set_position(position);
    Ok(())
}

#[cfg(test)]
mod tests;
