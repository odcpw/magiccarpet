//! Model36's source Skeleton controller. Its caller supplies the frozen
//! owner count and the actual Skeleton family's arena constructor.

use crate::{
    Position,
    campaign::source_arena::SourceArena,
    level_definition::FieldDefinition,
    move_entity,
    native_world::{ParticleParameter, SessionTerrain},
    renderable_map_chain::RenderableMapSeed,
};

/// Values written after each successful source class5/model9 constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Birth {
    pub(crate) position: Position,
    pub(crate) controller_slot: u16,
    pub(crate) owner: u16,
    pub(crate) mana: i16,
    pub(crate) yaw: i16,
}

/// sub_4FF30. The bare source constructor retains its initial 64000 payload;
/// action38 overwrites that payload only on its scheduled visit.
pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    definition: FieldDefinition,
    particle: ParticleParameter,
) -> Result<Option<u16>, String> {
    let FieldDefinition::SkeletonSummon {
        life,
        initial_budget,
        particle_index,
        ..
    } = definition
    else {
        return Err("Skeleton controller needs Skeleton summon properties".into());
    };
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut source = crate::world::terrain::cave_features::new_event(
        slot,
        arena.economy.new_event_rng(slot),
        position,
    );
    source.class = 10;
    source.model = 36;
    source.record[67] = 10;
    source.record[68] = 36;
    source.set_source_action(38);
    source.set_max_life(life);
    source.set_life(life);
    source.set_subspell_index(initial_budget);
    source.set_control_flags(source.control_flags & !8);
    if let Err(error) =
        crate::world::terrain::cave_features::bind_particle(&mut source, particle_index, particle)
    {
        arena.economy.release_slot(slot, Some(source.entity_rng));
        return Err(error);
    }
    source.set_source_overlap_half_extents(512, 512);
    source.record[92..94].copy_from_slice(&512_i16.to_le_bytes());
    if let Err(error) = arena.economy.map_chains.sync_entity(RenderableMapSeed {
        entity_index: slot,
        active: true,
        x: position.x,
        y: position.y,
    }) {
        arena.economy.release_slot(slot, Some(source.entity_rng));
        return Err(error);
    }
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.entities[usize::from(slot)] = source;
    crate::world::terrain::cave_features::synchronize_links(arena, terrain, slot);
    Ok(Some(slot))
}

/// sub_357C0. `owned_count` is computed from the source-frozen model9 list at
/// this controller's visit, resolving each current occupant's owner. The
/// callback constructs a real Skeleton and publishes its source fields; it
/// returns false on the source constructor's allocation/placement failure.
/// The source scheduler owns the common phase tail and following-pass reclaim.
pub(crate) fn step<F>(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    definition: FieldDefinition,
    owned_count: usize,
    mut spawn: F,
) -> Result<(), String>
where
    F: FnMut(&mut SourceArena, &mut SessionTerrain, Birth) -> Result<bool, String>,
{
    let FieldDefinition::SkeletonSummon {
        scheduled_budget,
        max_births,
        owner_cap,
        spawn_radius,
        ..
    } = definition
    else {
        return Err("Skeleton controller needs Skeleton summon properties".into());
    };
    if scheduled_budget > i16::MAX as u16
        || (scheduled_budget != 0 && scheduled_budget < u16::from(max_births))
    {
        return Err(
            "Skeleton summon budget must fit i16 and give each birth a nonzero share".into(),
        );
    }
    let source = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && (row.class, row.model, row.action) == (10, 36, 38))
        .ok_or("Skeleton controller source row is absent")?;
    source.set_subspell_index(scheduled_budget);
    let count = arena
        .economy
        .free_slot_count()
        .min(usize::from(max_births))
        .min(usize::from(owner_cap).saturating_sub(owned_count));
    if count != 0 && scheduled_budget != 0 {
        let count = i16::try_from(count).expect("typed birth count fits i16");
        let budget = i16::try_from(scheduled_budget).expect("validated signed budget");
        let share = budget / count;
        let angle_step = 2_048_u16 / count as u16;
        let mut remaining = budget;
        let mut angle = 0_u16;
        for _ in 0..count {
            // Source retains the controller's fixed address across NewEvent.
            let controller = &arena.entities[usize::from(slot)];
            let mut position = controller.position;
            move_entity(&mut position, angle as i16, 0, spawn_radius);
            let birth = Birth {
                position,
                controller_slot: slot,
                owner: controller.source_id,
                mana: remaining % share,
                yaw: angle.wrapping_add(1_024) as i16 & 2_047,
            };
            if spawn(arena, terrain, birth)? {
                remaining = remaining.wrapping_sub(share);
            }
            angle = angle.wrapping_add(angle_step);
        }
    }
    arena.entities[usize::from(slot)].disable_drawing();
    Ok(())
}
