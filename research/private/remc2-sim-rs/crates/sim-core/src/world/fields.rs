//! Wind, transport and hostile player statuses over the authoritative arena.
//!
//! Field callbacks borrow actor/player carriers. The source scheduler retains
//! phase advancement, dispatch order and following-pass reclaim.

pub mod palette;
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
}

impl State {
    /// Live draw tuples borrow the same root/segment state as the scheduler.
    pub(crate) fn sprite_sources(
        &self,
    ) -> impl Iterator<Item = (u16, u16, u16, i16, crate::Position)> + '_ {
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
            row.set_source_overlap_half_extents(source.shift_rot_x, source.shift_rot_y);
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
            row.set_max_life(i32::from(source.max_life));
            row.set_life(i32::from(source.life));
            row.particle_parameter_index = Some(source.particle_index);
            row.record[94..96].copy_from_slice(&source.particle_index.to_le_bytes());
            row.set_source_overlap_half_extents(source.shift_rot_x, source.shift_rot_y);
            row.record[67] = row.class;
            row.record[68] = row.model;
            row.record[44..46].copy_from_slice(&source.parent_entity_index.to_le_bytes());
            rows.push(row);
        }
        rows
    }
}
