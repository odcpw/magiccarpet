//! Source Cave corridor construction, visits, terrain and lifetime.

use crate::native_world::{MapType, SessionTerrain, terrain_strip::TileCursor};
use crate::{
    Position,
    campaign::source_arena::SourceArena,
    level_definition::{CaveCorridorDefinition, CorridorOrientation},
    native_world::EntitySnapshot,
};

/// Source `sub_516C0`, followed by authored `PrepareEvents_49540`
/// orientation/extent writes. Failed/non-cave construction has no births.
pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    properties: CaveCorridorDefinition,
) -> Option<u16> {
    if terrain.map_type() != MapType::Cave {
        return None;
    }
    let slot = arena.allocate_source_slot_with_terrain(terrain)?;
    let rng = arena.economy.new_event_rng(slot);
    let mut source = crate::world::terrain::cave_features::new_event(slot, rng, position);
    source.class = 14;
    source.model = 2;
    source.record[67] = 14;
    source.record[68] = 2;
    source.set_source_action(7);
    // sub_516C0 clears NewEvent's bit8 and sets bit1; AddEventToMap sets bit4.
    source.set_control_flags((source.control_flags & !9) | 5);
    source.set_life(0);
    source.set_subspell_index(0);
    bind_properties(&mut source, properties);
    arena.entities[usize::from(slot)] = source;
    // AddEventToMap updates the shared links, neighboring raw rows and the
    // actual terrain map-head plane in one operation.
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)
        .expect("the allocated corridor has a valid source map slot");
    arena.economy.source_rng.insert(slot, rng);
    Some(slot)
}

/// Bind immutable construction content to the authoritative source words.
/// Imported frame-zero controllers and later NewEvent births use the same
/// fields; behavior never reads a second mutable property mirror.
pub(crate) fn bind_properties(source: &mut EntitySnapshot, properties: CaveCorridorDefinition) {
    source.set_source_word_2c(i16::from(
        properties.orientation() == CorridorOrientation::AlongX,
    ));
    source.record[154..156].copy_from_slice(&(properties.extent() as u16).to_le_bytes());
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Visit {
    pub(crate) prepare_sound: bool,
    pub(crate) end_loop: bool,
    pub(crate) reclaim: bool,
}

impl Visit {
    /// Ordered descriptors consumed by the existing campaign audio journal.
    /// The terrain controller never installs an actor sprite or draw clock.
    pub(crate) fn effects(self, source_slot: u16) -> impl Iterator<Item = crate::FlightEvent> {
        [
            self.prepare_sound
                .then_some(crate::FlightEvent::PrepareSound {
                    #[cfg(test)]
                    audio_site: crate::MissionAudioSite::TerrainStrip,
                    entity_index: source_slot,
                    player_index: -1,
                    sound_id: 47,
                }),
            self.end_loop.then_some(crate::FlightEvent::EndLoop {
                #[cfg(test)]
                audio_site: crate::MissionAudioSite::TerrainStrip,
                entity_index: source_slot,
                player_index: -1,
                sound_id: 47,
            }),
        ]
        .into_iter()
        .flatten()
    }
}

/// Execute against the arena authority. Invalid initialization removes the
/// map link and returns the slot to NewEvent immediately, in this callback.
pub(crate) fn visit(arena: &mut SourceArena, terrain: &mut SessionTerrain, slot: u16) -> Visit {
    let source = &mut arena.entities[usize::from(slot)];
    let result = step(terrain, source);
    if result.reclaim {
        let rng = source.entity_rng;
        // SetMapEntity_57E50 clears the linkage bit but retains the
        // retired row's old/next words. The live neighbors must reflect
        // the unlink before the common scheduler epilogue runs.
        source.set_control_flags(source.control_flags & !4);
        let position = source.position;
        let links = (
            arena.economy.map_chains.old_map_entity(slot),
            arena.economy.map_chains.next_entity(slot),
        );
        crate::world::effects::sync_position(arena, terrain, slot, position, false, false)
            .expect("the live corridor has a valid source map slot");
        // The source unlink retains the retired packet's old/next words.
        arena.entities[usize::from(slot)].set_map_links(links.0, links.1);
        arena.entities[usize::from(slot)].reclaim_source_row();
        arena.economy.release_slot(slot, Some(rng));
    }
    result
}

/// Initialize or advance one corridor. Life 1 seals the opening and life 2
/// restores its surroundings. Both branches emit Prepare47 *before* the
/// terminal EndLoop47, including a callback that makes no height writes.
pub(crate) fn step(terrain: &mut SessionTerrain, source: &mut EntitySnapshot) -> Visit {
    if terrain.map_type() != MapType::Cave || source.life as u32 > 2 {
        return Visit::default();
    }
    let center = TileCursor::from_xy(
        source.position.x.to_be_bytes()[0],
        source.position.y.to_be_bytes()[0],
    );
    let horizontal = source.source_word_2c() != 0;
    // The source words are signed, including word96 despite its other
    // uses as an unsigned entity relation. Keep integer promotion here.
    let length = (2 * ((i32::from(source.source_word_96() as i16) << 8) + 512) + 128) >> 8;
    let (width, height) = if horizontal { (length, 2) } else { (2, length) };
    let mut corner = center;
    corner.add_x(-(width >> 1));
    corner.add_y(-(height >> 1));
    let (planes, _, _, mutation_sequence) = terrain.disaster_parts();
    let mut visit = Visit::default();
    let mut mutated = false;
    if source.life == 0 {
        source.set_life(4);
        // The C loop wraps a byte cursor indefinitely on malformed maps.
        // One complete wrapped axis is exhaustive; failure is the source
        // immediate sub_57F20 path, not a draw-disabled tombstone.
        let bound = |delta: i32| {
            let mut cursor = center;
            for _ in 0..256 {
                if planes.angles[cursor.index()] & 8 != 0 {
                    return Some(planes.ground_heights[cursor.index()]);
                }
                if horizontal {
                    cursor.add_x(delta);
                } else {
                    cursor.add_y(delta);
                }
            }
            None
        };
        let (Some(first), Some(second)) = (bound(-1), bound(1)) else {
            visit.reclaim = true;
            return visit;
        };
        let mut position = source.position;
        position.z = (i16::from(first) + i16::from(second)) >> 1;
        source.set_position(position);
        for y in 0..height {
            let mut cursor = corner;
            cursor.add_y(y);
            for _ in 0..width {
                planes.angles[cursor.index()] |= 0x80;
                cursor.add_x(1);
                mutated = true;
            }
        }
    } else {
        visit.prepare_sound = true;
        let product = 8 * length;
        let speed = (product - if product < 0 { -31 } else { 0 }) >> 5;
        let midpoint = i32::from(source.position.z);
        let floor_limit = (i32::from(planes.ground_heights[center.index()]) + speed + 1)
            .max(0)
            .min(midpoint);
        let ceiling_limit = (i32::from(planes.secondary_heights[center.index()]) - speed - 1)
            .max(midpoint)
            .min(254);
        let mut settled = true;
        for y in 0..height {
            let mut cursor = corner;
            cursor.add_y(y);
            for _ in 0..width {
                let index = cursor.index();
                if source.life == 1 {
                    if planes.angles[index] & 8 == 0 {
                        let floor =
                            (i32::from(planes.ground_heights[index]) + speed).min(floor_limit);
                        if floor > i32::from(planes.ground_heights[index]) {
                            planes.ground_heights[index] = floor as u8;
                            settled = false;
                        }
                        let ceiling =
                            (i32::from(planes.secondary_heights[index]) - speed).max(ceiling_limit);
                        if ceiling < i32::from(planes.secondary_heights[index]) {
                            planes.secondary_heights[index] = ceiling as u8;
                            settled = false;
                        }
                    }
                } else {
                    let mut neighbors = [cursor; 3];
                    for (neighbor, offset) in neighbors.iter_mut().zip([-1, 2, 3]) {
                        if horizontal {
                            neighbor.set_y(corner.y().wrapping_add(offset as u8));
                        } else {
                            neighbor.set_x(corner.x().wrapping_add(offset as u8));
                        }
                    }
                    let [a, b, c] = neighbors.map(TileCursor::index);
                    let floor_limit = restored_height(
                        planes.ground_heights[a],
                        planes.ground_heights[b],
                        planes.ground_heights[c],
                    );
                    let floor = (i32::from(planes.ground_heights[index]) - speed).max(floor_limit);
                    if floor < i32::from(planes.ground_heights[index]) {
                        planes.ground_heights[index] = floor as u8;
                        settled = false;
                    }
                    let ceiling_limit = restored_height(
                        planes.secondary_heights[a],
                        planes.secondary_heights[b],
                        planes.secondary_heights[c],
                    );
                    let ceiling =
                        (i32::from(planes.secondary_heights[index]) + speed).min(ceiling_limit);
                    if ceiling > i32::from(planes.secondary_heights[index]) {
                        planes.secondary_heights[index] = ceiling as u8;
                        settled = false;
                    }
                }
                let angle = planes.angles[index];
                if planes.secondary_heights[index] > planes.ground_heights[index] {
                    planes.angles[index] &= 0xf7;
                } else {
                    planes.angles[index] |= 8;
                }
                mutated |= planes.angles[index] != angle;
                cursor.add_x(1);
            }
        }
        mutated |= !settled;
        if settled {
            source.set_life(if source.life == 1 { 3 } else { 4 });
            visit.end_loop = true;
        }
    }
    if mutated {
        *mutation_sequence = mutation_sequence.wrapping_add(1);
    }
    visit
}

fn restored_height(first: u8, second: u8, third: u8) -> i32 {
    let (first, second, third) = (i32::from(first), i32::from(second), i32::from(third));
    if (first - second).abs() <= 4 {
        (first + second) / 2
    } else if (first - third).abs() > 4 {
        second
    } else {
        first
    }
}
