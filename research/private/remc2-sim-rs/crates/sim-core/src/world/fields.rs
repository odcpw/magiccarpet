//! Wind, transport and hostile player statuses over the authoritative arena.
//!
//! Field callbacks borrow actor/player carriers. The source scheduler retains
//! phase advancement, dispatch order and following-pass reclaim.

pub(crate) mod gravity;
pub(crate) mod magic_mine;
pub mod palette;
pub(crate) mod skeleton_summon;
pub(crate) mod status;
pub(crate) mod transport;
pub(crate) mod wind;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PaletteRequest {
    pub(crate) mode: u8,
    pub(crate) count: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Contact {
    pub(crate) target: u16,
    pub(crate) owner: u16,
    pub(crate) payload: u16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Outcome {
    /// Sound source and sound number, in source-call order.
    pub(crate) sounds: Vec<(u16, u16)>,
    pub(crate) palette: Option<PaletteRequest>,
    pub(crate) contacts: Vec<Contact>,
    pub(crate) transported: bool,
}

/// Wind's owned controller state. Transport and impact controllers live in
/// their canonical source records and need no mutable sidecar.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct State {
    pub(crate) authored_model22: Vec<wind::Wind>,
    pub(crate) model22_spawned_children: Vec<wind::Segment>,
    pub(crate) palette: palette::Palette,
    pub(crate) skeleton_summon: skeleton_summon::State,
    /// Generation-bound Wind slots, including authored rows without raw
    /// projections. The parent distinguishes a root from its segments.
    wind_projections: std::collections::BTreeMap<u16, (u32, u16)>,
}

/// Ordinary spell delivery uses the same wind root/segment authority as
/// authored births. The sentinel carries absence of an authored record only;
/// it does not select another allocator or update path.
pub(crate) fn construct_spell_wind(
    arena: &mut crate::campaign::source_arena::SourceArena,
    terrain: &mut crate::native_world::SessionTerrain,
    particles: &[crate::native_world::ParticleParameter],
    position: crate::Position,
) -> Result<Option<u16>, String> {
    let definition = wind::Definition::from_particles(particles)?;
    let Some(root) =
        arena
            .economy
            .allocate_authored_wind(u16::MAX, position, position, terrain, definition)
    else {
        return Ok(None);
    };
    let rows = arena.economy.fields.source_rows();
    for row in rows.into_iter().filter(|row| {
        row.index == root.source_entity_index
            || row.source_spell_parent_entity_index() == root.source_entity_index
    }) {
        let (slot, position) = (row.index, row.position);
        arena.entities[usize::from(slot)] = row;
        let generation = arena.economy.slot_generation(slot);
        arena
            .economy
            .fields
            .wind_projections
            .insert(slot, (generation, root.source_entity_index));
        crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    }
    Ok(Some(root.source_entity_index))
}

/// Record the authored constructor's typed slots and publish their map heads.
pub(crate) fn register_authored_wind(
    arena: &mut crate::campaign::source_arena::SourceArena,
    terrain: &mut crate::native_world::SessionTerrain,
    parent: u16,
) {
    let slots: Vec<u16> = std::iter::once(parent)
        .chain(
            arena
                .economy
                .fields
                .model22_spawned_children
                .iter()
                .filter(|child| child.parent_entity_index == parent && child.active)
                .map(|child| child.source_entity_index),
        )
        .collect();
    for slot in slots {
        let generation = arena.economy.slot_generation(slot);
        arena
            .economy
            .fields
            .wind_projections
            .insert(slot, (generation, parent));
    }
    let snapshot = capture_wind_links(arena, parent);
    publish_wind_links(arena, terrain, snapshot);
}

/// The typed Wind callback moves its slots before returning. Preserve the old
/// tiles and neighbors so its caller can publish both sides of those moves.
pub(crate) struct WindLinks(Vec<(u16, u32, u16, u16, u16)>);

pub(crate) fn capture_wind_links(
    arena: &crate::campaign::source_arena::SourceArena,
    parent: u16,
) -> WindLinks {
    WindLinks(
        arena
            .economy
            .fields
            .wind_projections
            .iter()
            .filter(|(_, (_, owner))| *owner == parent)
            .map(|(&slot, &(generation, _))| {
                let chains = &arena.economy.map_chains;
                (
                    slot,
                    generation,
                    chains.tile_of(slot),
                    chains.old_map_entity(slot),
                    chains.next_entity(slot),
                )
            })
            .collect(),
    )
}

pub(crate) fn publish_wind_links(
    arena: &mut crate::campaign::source_arena::SourceArena,
    terrain: &mut crate::native_world::SessionTerrain,
    snapshot: WindLinks,
) {
    let mut tiles = std::collections::BTreeSet::new();
    let mut neighbors = std::collections::BTreeSet::new();
    for (slot, generation, old_tile, old_previous, old_next) in snapshot.0 {
        if arena.economy.slot_generation(slot) != generation {
            continue;
        }
        let chains = &arena.economy.map_chains;
        tiles.extend([old_tile, chains.tile_of(slot)]);
        neighbors.extend([
            slot,
            old_previous,
            old_next,
            chains.old_map_entity(slot),
            chains.next_entity(slot),
        ]);
    }
    for tile in tiles {
        terrain.disaster_parts().0.entity_indices[usize::from(tile)] =
            arena.economy.map_chains.tile_head(tile) as i16;
    }
    for slot in neighbors.into_iter().filter(|slot| *slot != 0) {
        let links = (
            arena.economy.map_chains.old_map_entity(slot),
            arena.economy.map_chains.next_entity(slot),
        );
        let row = &mut arena.entities[usize::from(slot)];
        if row.active {
            row.record[26..28].copy_from_slice(&links.0.to_le_bytes());
            row.record[28..30].copy_from_slice(&links.1.to_le_bytes());
        }
    }
}

/// Unlink hidden Wind rows before the typed arena reclaims them. A reused
/// generation is never retired on behalf of an older field.
pub(crate) fn retire_hidden_wind_projections(
    arena: &mut crate::campaign::source_arena::SourceArena,
    terrain: &mut crate::native_world::SessionTerrain,
) -> Result<(), String> {
    let entries: Vec<_> = arena
        .economy
        .fields
        .wind_projections
        .iter()
        .map(|(&slot, &(generation, parent))| (slot, generation, parent))
        .collect();
    for (slot, generation, parent) in entries {
        let same_generation = arena.economy.slot_generation(slot) == generation;
        let root = slot == parent;
        let hidden = if root {
            arena
                .economy
                .fields
                .authored_model22
                .iter()
                .find(|wind| wind.source_entity_index == slot)
                .map(|wind| wind.control_flags & 0x400 != 0 || !wind.active)
        } else {
            arena
                .economy
                .fields
                .model22_spawned_children
                .iter()
                .find(|child| {
                    child.source_entity_index == slot && child.parent_entity_index == parent
                })
                .map(|child| !child.active)
        };
        if !same_generation || hidden.is_none() {
            arena.economy.fields.wind_projections.remove(&slot);
            continue;
        }
        if hidden == Some(true) {
            let position = arena.entities[usize::from(slot)].position;
            crate::world::effects::sync_position(arena, terrain, slot, position, false, false)?;
            let row = &mut arena.entities[usize::from(slot)];
            let model = if root { 22 } else { 75 };
            if (row.class, row.model) == (10, model) {
                row.reclaim_source_row();
            }
            arena.economy.fields.wind_projections.remove(&slot);
        }
    }
    Ok(())
}

/// Commit the shared projectile's post-constructor writes, and its caller's
/// duration override, to the wind owner read by simulation and presentation.
pub(crate) fn sync_wind_impact(arena: &mut crate::campaign::source_arena::SourceArena, slot: u16) {
    let Some(row) = arena.entities.get(usize::from(slot)) else {
        return;
    };
    if let Some(wind) = arena
        .economy
        .fields
        .authored_model22
        .iter_mut()
        .find(|wind| wind.active && wind.source_entity_index == slot)
    {
        wind.entity_id = row.source_id;
        wind.yaw = row.yaw;
        wind.pitch = row.pitch;
        wind.max_life = row.max_life as i16;
        wind.life = row.life as i16;
        wind.subspell_index = row.subspell_index;
        wind.control_flags = row.control_flags;
    }
}

impl State {
    pub(crate) fn hash_wind_projections(&self, hasher: &mut sha2::Sha256) {
        use sha2::Digest;
        if !self.wind_projections.is_empty() {
            hasher.update(b"wind-projections-v1");
            for (&slot, &(generation, parent)) in &self.wind_projections {
                hasher.update(slot.to_le_bytes());
                hasher.update(generation.to_le_bytes());
                hasher.update(parent.to_le_bytes());
            }
        }
        self.skeleton_summon.hash(hasher);
    }

    /// Live draw tuples borrow the same root/segment state as the scheduler.
    pub(crate) fn sprite_sources(
        &self,
    ) -> impl Iterator<Item = (u16, u16, u16, i16, crate::Position, u32)> + '_ {
        self.authored_model22
            .iter()
            .filter(|s| s.active && s.control_flags & 0x421 == 0)
            .map(|s| {
                (
                    s.source_entity_index,
                    s.entity_id,
                    s.particle_index,
                    s.yaw,
                    s.position,
                    s.control_flags,
                )
            })
            .chain(
                self.model22_spawned_children
                    .iter()
                    .filter(|s| s.active && s.drawing_enabled)
                    .map(|s| {
                        (
                            s.source_entity_index,
                            s.entity_id,
                            s.particle_index,
                            s.yaw,
                            s.position,
                            if s.drawing_enabled { 0 } else { 0x400 },
                        )
                    }),
            )
    }

    /// Ephemeral compatibility rows for the existing native observation ABI.
    /// Hidden rows remain represented until the shared following-pass reclaim;
    /// no observer stores or mutates a second field authority.
    pub(crate) fn source_rows(&self) -> Vec<crate::native_world::EntitySnapshot> {
        let mut rows =
            Vec::with_capacity(self.authored_model22.len() + self.model22_spawned_children.len());
        for source in &self.authored_model22 {
            let mut row = crate::world::terrain::cave_features::new_event(
                source.source_entity_index,
                source.entity_rng,
                source.position,
            );
            row.source_id = source.entity_id;
            row.record[30..32].copy_from_slice(&source.entity_id.to_le_bytes());
            row.active = source.active;
            row.class = source.source_class;
            row.model = source.model;
            row.set_source_action(source.action);
            row.set_phase(source.phase);
            row.set_control_flags(source.control_flags);
            row.set_yaw(source.yaw);
            row.pitch = source.pitch;
            row.record[34..36].copy_from_slice(&source.pitch.to_le_bytes());
            row.record[36..38].copy_from_slice(&source.roll.to_le_bytes());
            row.set_actual_speed(source.actual_speed);
            row.set_max_life(i32::from(source.max_life));
            row.set_life(i32::from(source.life));
            row.set_subspell_index(source.subspell_index);
            row.particle_parameter_index = Some(source.particle_index);
            row.record[94..96].copy_from_slice(&source.particle_index.to_le_bytes());
            row.set_source_overlap_half_extents(source.shift_rot_x, source.shift_rot_x);
            row.record[92..94].copy_from_slice(&source.shift_rot_y.to_le_bytes());
            row.record[67] = row.class;
            row.record[68] = row.model;
            rows.push(row);
        }
        for source in &self.model22_spawned_children {
            let mut row = crate::world::terrain::cave_features::new_event(
                source.source_entity_index,
                source.entity_rng,
                source.position,
            );
            row.source_id = source.entity_id;
            row.record[30..32].copy_from_slice(&source.entity_id.to_le_bytes());
            row.active = source.active;
            row.class = source.source_class;
            row.model = source.model;
            row.set_source_action(source.action);
            row.set_phase(source.phase);
            row.set_control_flags(if source.drawing_enabled { 0 } else { 0x400 });
            row.set_yaw(source.yaw);
            row.pitch = source.pitch;
            row.record[34..36].copy_from_slice(&source.pitch.to_le_bytes());
            row.record[36..38].copy_from_slice(&source.roll.to_le_bytes());
            // AddWind clones the parent's speed50 before stamping model75.
            // Linked rows have no independent speed-changing callback.
            row.set_actual_speed(50);
            row.set_max_life(i32::from(source.max_life));
            row.set_life(i32::from(source.life));
            row.particle_parameter_index = Some(source.particle_index);
            row.record[94..96].copy_from_slice(&source.particle_index.to_le_bytes());
            row.set_source_overlap_half_extents(source.shift_rot_x, source.shift_rot_x);
            row.record[92..94].copy_from_slice(&source.shift_rot_y.to_le_bytes());
            row.record[67] = row.class;
            row.record[68] = row.model;
            row.record[44..46].copy_from_slice(&source.parent_entity_index.to_le_bytes());
            rows.push(row);
        }
        rows
    }
}
