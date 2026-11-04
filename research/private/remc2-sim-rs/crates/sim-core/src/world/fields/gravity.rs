//! Gravity-well construction and model67 actions72/73/74.
//!
//! The field callback owns its rise, hold, shrink, crater mutations, and actor
//! pull rules. The scheduler consumes only the narrow same-pass output.

use crate::{
    FlightWorld, Position, SIN_16_16,
    campaign::source_arena::SourceArena,
    heading_for_positions,
    legacy_terrain::interpolate_ground,
    move_entity,
    native_world::{
        EntitySnapshot, MapType, SessionTerrain, building_topology_rebuild_required,
        rebuild_building_topology, terrain_index,
    },
    renderable_map_chain::RenderableMapSeed,
    world::terrain::cave_features::{new_event, synchronize_links},
};

pub(crate) const SOURCE_CLASS: u8 = 10;
pub(crate) const SOURCE_MODEL: u8 = 67;
pub(crate) const ACTION_RISE: u8 = 72;
pub(crate) const ACTION_HOLD: u8 = 73;
pub(crate) const ACTION_SHRINK: u8 = 74;

const PULL_RADIUS: u32 = 3_328;
const MIN_PULL_SPEED: u32 = 4;
const MAX_PULL_SPEED: u32 = 128;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct StepOutcome {
    /// The crater footprint was denied or action74 finished its shrink.
    pub(crate) hide: bool,
    /// Source sound prepared when the field starts rising.
    pub(crate) sound_id: Option<u16>,
    /// Run `sub_39B60` for this callback. On action74 stage zero the source
    /// calls it before `sub_3A090`, so consume the pre-handler projections.
    pub(crate) pull_actors: bool,
    pub(crate) terrain_mutated: bool,
    /// Spell-family award emitted by `sub_3A090` when captured model-2 rows
    /// contribute to the crater, or by `sub_3A200` on a successful contact.
    pub(crate) spell_experience: Option<SpellExperienceAward>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SpellExperienceAward {
    pub(crate) owner_source_id: u16,
    pub(crate) family: u8,
    pub(crate) amount: i32,
}

/// Minimal actor projection needed by `sub_39FA0` and `sub_39B60`.
/// The caller resolves this from the live player/creature/source carrier and
/// applies the returned motion to that same authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Actor {
    pub(crate) source_id: u16,
    pub(crate) class: u8,
    pub(crate) model: u8,
    pub(crate) action: u8,
    pub(crate) control_flags: u32,
    pub(crate) position: Position,
    /// Source `life_0x8` on the target actor. `sub_3A200` awards this value +1.
    pub(crate) life: i32,
    /// Nested actor `word_0x160_0xe_14`; values below -64 suppress the
    /// vertical pull while still allowing the horizontal pull.
    pub(crate) vertical_state: i16,
    pub(crate) pitch: i16,
    /// Source byte `byte_0x38_56 & 1`, which gates the contact bounty.
    pub(crate) bounty_enabled: bool,
    /// Source `str_0x5E_94.dword_0x5E_94` bounty accumulator.
    pub(crate) bounty: i32,
    /// Source `str_0x5E_94.word_0x62_98` bounty owner.
    pub(crate) bounty_attacker: u16,
    /// Source `word_0x26_38`, linked to the Gravity Well while captured.
    pub(crate) capture_link: u16,
    /// Whether this class-3/model-0 actor's player color matches the current
    /// LevelIndex check used when action74 releases captured actors.
    pub(crate) is_current_level_player: bool,
}

/// Mutation for the same actor the caller projected. The close-contact branch
/// returns its actual actor writes and family-20 award to the same consumer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Motion {
    pub(crate) actor: Actor,
    /// The caller mirrors this draw into `Level2EconomyState::source_rng`.
    pub(crate) source_rng: Option<u16>,
    pub(crate) contact: bool,
    pub(crate) spell_experience: Option<SpellExperienceAward>,
}

/// `sub_51730`. Publish the source into both the authoritative source arena
/// and its map chain before the scheduler can visit it.
pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    power: u16,
    effect_life: i32,
) -> Result<Option<u16>, String> {
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut source = new_event(slot, arena.economy.new_event_rng(slot), position);
    source.class = SOURCE_CLASS;
    source.model = SOURCE_MODEL;
    source.record[67] = SOURCE_CLASS;
    source.record[68] = SOURCE_MODEL;
    source.set_source_action(ACTION_RISE);
    source.set_control_flags((source.control_flags & !9) | 1);
    source.set_life(effect_life);
    source.set_subspell_index(power);
    source.set_source_overlap_half_extents(4_352, 4_352);
    source.record[92..94].copy_from_slice(&4_352_i16.to_le_bytes());

    if let Err(error) = arena.economy.map_chains.sync_entity(RenderableMapSeed {
        entity_index: slot,
        active: true,
        x: position.x,
        y: position.y,
    }) {
        arena.economy.release_slot(slot, Some(source.entity_rng));
        return Err(error);
    }
    // Source AddEventToMap publishes04 after linking this hidden controller.
    source.set_control_flags(source.control_flags | 4);
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.entities[usize::from(slot)] = source;
    synchronize_links(arena, terrain, slot);
    Ok(Some(slot))
}

/// Source actions72/73/74 (`sub_39040`, `sub_396A0`, and `sub_396D0`). State
/// and crater terrain are written through their existing authorities; the
/// returned outcomes are the narrow same-pass audio, pull, hide, and
/// spell-award effects for the scheduler to consume.
pub(crate) fn step(
    arena: &mut SourceArena,
    source_slot: u16,
    terrain: &mut SessionTerrain,
) -> StepOutcome {
    let source_index = usize::from(source_slot);
    let Some(snapshot) = arena.entities.get(source_index) else {
        return StepOutcome::default();
    };
    if !snapshot.active
        || (snapshot.class, snapshot.model) != (SOURCE_CLASS, SOURCE_MODEL)
        || !matches!(snapshot.action, ACTION_RISE | ACTION_HOLD | ACTION_SHRINK)
    {
        return StepOutcome::default();
    }
    if snapshot.action == ACTION_SHRINK {
        return step_shrink(arena, source_slot, terrain);
    }

    // `sub_39E40` reads the arena while the callback later mutates the source
    // row, so resolve this gate before taking the mutable row borrow.
    let can_start = (snapshot.action == ACTION_RISE && snapshot.record[74] == 0)
        .then(|| can_start_rising(&arena.entities, source_slot, terrain));
    let source = &mut arena.entities[source_index];
    source.set_life(source.life.wrapping_sub(1));
    if source.life <= 0 {
        source.set_source_action(74);
        source.record[74] = 0;
        return StepOutcome::default();
    }

    if source.action == ACTION_HOLD {
        return StepOutcome {
            pull_actors: true,
            ..StepOutcome::default()
        };
    }

    match source.record[74] {
        0 => {
            if can_start == Some(true) {
                source.record[74] = 1;
                StepOutcome::default()
            } else {
                source.disable_drawing();
                StepOutcome {
                    hide: true,
                    ..StepOutcome::default()
                }
            }
        }
        1 => {
            let (center_x, center_y) = rounded_center(source.position);
            let start_x = center_x.wrapping_sub(9);
            let start_y = center_y.wrapping_sub(9);
            let (planes, _, _, _) = terrain.disaster_parts();
            let corner_sum = u16::from(planes.ground_heights[terrain_index(start_x, start_y)])
                + u16::from(
                    planes.ground_heights[terrain_index(start_x.wrapping_add(18), start_y)],
                )
                + u16::from(
                    planes.ground_heights
                        [terrain_index(start_x.wrapping_add(18), start_y.wrapping_add(18))],
                )
                + u16::from(
                    planes.ground_heights[terrain_index(start_x, start_y.wrapping_add(18))],
                );
            let average = (corner_sum >> 2) as i16;
            let source = &mut arena.entities[source_index];
            let z = if average > 64 { average - 64 } else { 0 };
            source.set_position(Position {
                z,
                ..source.position
            });
            source.set_source_word_2c(if z > 16 {
                32_i16.wrapping_mul(average - 80)
            } else {
                0
            });
            source.mana = 0;
            source.record[148..152].fill(0);
            source.record[74] = 2;
            source.set_source_dword_10(12);
            let mut outcome = StepOutcome {
                sound_id: Some(64),
                ..StepOutcome::default()
            };
            outcome.merge(step_rise_phase_two(arena, source_slot, terrain));
            outcome
        }
        2 => step_rise_phase_two(arena, source_slot, terrain),
        3 => finish_rising(arena, source_slot, terrain),
        _ => StepOutcome::default(),
    }
}

/// Source `sub_396D0`. The first visit establishes the shrinking field and
/// starts its sixteen terrain interpolation passes. The final visit restores
/// the smoothed ground, unlinks remaining captured rows, and hides the source
/// so the ordinary following source pass can reclaim its slot.
fn step_shrink(
    arena: &mut SourceArena,
    source_slot: u16,
    terrain: &mut SessionTerrain,
) -> StepOutcome {
    let source_index = usize::from(source_slot);
    let phase = arena.entities[source_index].record[74];
    if phase == 2 {
        return finish_shrinking(arena, source_slot, terrain);
    }
    if phase > 2 {
        return StepOutcome::default();
    }

    let mut outcome = if phase == 0 {
        begin_shrinking(arena, source_slot, terrain)
    } else {
        StepOutcome::default()
    };

    // sub_396D0 falls through from stage zero into this cadence check. Once
    // action73 has expired, life is zero, so the timer advances on every pass.
    if arena.entities[source_index].life & 3 != 0 {
        if outcome.terrain_mutated {
            let (_, _, _, sequence) = terrain.disaster_parts();
            *sequence = sequence.wrapping_add(1);
        }
        return outcome;
    }
    let timer = arena.entities[source_index]
        .source_dword_10()
        .wrapping_sub(1);
    arena.entities[source_index].set_source_dword_10(timer);
    if timer <= 0 {
        arena.entities[source_index].record[74] = 2;
        if outcome.terrain_mutated {
            let (_, _, _, sequence) = terrain.disaster_parts();
            *sequence = sequence.wrapping_add(1);
        }
        return outcome;
    }

    shrink_ground(arena, source_slot, terrain, timer);
    outcome.terrain_mutated = true;
    let (_, _, _, sequence) = terrain.disaster_parts();
    *sequence = sequence.wrapping_add(1);
    outcome
}

/// sub_396D0's stage-zero prologue. The actor pull is emitted for the
/// same-pass source callback; the scheduler must use the pre-handler source
/// and actor projections because sub_39B60 runs before sub_3A090 here.
fn begin_shrinking(
    arena: &mut SourceArena,
    source_slot: u16,
    terrain: &mut SessionTerrain,
) -> StepOutcome {
    let source_index = usize::from(source_slot);
    let before = arena.entities[source_index].clone();
    let spell_experience = destroy_and_reward(arena, source_slot, &before);
    let (center_x, center_y) = rounded_center(before.position);
    let start_x = center_x.wrapping_sub(15);
    let start_y = center_y.wrapping_sub(15);
    let end_x = center_x.wrapping_add(15);
    let end_y = center_y.wrapping_add(15);

    {
        let source = &mut arena.entities[source_index];
        source.record[74] = 1;
        source.set_source_dword_10(16);
        source.set_position(Position {
            z: source.position.z.wrapping_add(64),
            ..source.position
        });
    }

    {
        let (planes, map_type, terrain_rng, _) = terrain.disaster_parts();
        for row in 0..30_u8 {
            for column in 0..30_u8 {
                let tile = terrain_index(start_x.wrapping_add(column), start_y.wrapping_add(row));
                if building_topology_rebuild_required(planes.terrain_types[tile]) {
                    planes.terrain_types[tile] = 1;
                    planes.angles[tile] = (planes.angles[tile] & 0xf8) | 1;
                }
            }
        }
        rebuild_building_topology(
            planes,
            start_x,
            start_y,
            end_x,
            end_y,
            map_type,
            terrain_rng,
        );
    }

    StepOutcome {
        sound_id: Some(64),
        pull_actors: true,
        terrain_mutated: true,
        spell_experience,
        ..StepOutcome::default()
    }
}

/// Interpolate the surrounding ground toward the raised field. The source
/// draws once for each tile inside the source's horizontal 3840-unit radius.
fn shrink_ground(
    arena: &mut SourceArena,
    source_slot: u16,
    terrain: &mut SessionTerrain,
    timer: i32,
) {
    let source_index = usize::from(source_slot);
    let source = arena.entities[source_index].clone();
    let (center_x, center_y) = rounded_center(source.position);
    let start_x = center_x.wrapping_sub(15);
    let start_y = center_y.wrapping_sub(15);
    let mut entity_rng = source.entity_rng;

    {
        let (planes, map_type, _, _) = terrain.disaster_parts();
        for row in 0..30_u8 {
            for column in 0..30_u8 {
                let x = start_x.wrapping_add(column);
                let y = start_y.wrapping_add(row);
                let tile_position = Position {
                    x: u16::from(x) << 8,
                    y: u16::from(y) << 8,
                    z: 0,
                };
                // Despite its name, remc2's EuclideanDistXYZ_58490 ignores Z.
                let distance = horizontal_distance(source.position, tile_position);
                let tile = terrain_index(x, y);
                if distance < 3_840 {
                    let heading = heading_for_positions(source.position, tile_position) as i16;
                    let mut sample = source.position;
                    move_entity(&mut sample, heading, 0, 3_840);
                    let altitude =
                        interpolate_ground(&planes.ground_heights, sample.x, sample.y) >> 5;
                    let sine_index = 512 + (((distance << 10) / 3_840) as usize);
                    let blend = (65_536 + SIN_16_16[sine_index]) / 2;
                    let target =
                        altitude - ((blend * (altitude - i32::from(source.position.z))) >> 16);
                    entity_rng = next_rng(entity_rng);
                    let randomized_target = target + i32::from(entity_rng & 3) - 2;
                    let old_height = i32::from(planes.ground_heights[tile]);
                    let next_height =
                        (old_height + (randomized_target - old_height) / timer).clamp(1, 255);
                    planes.ground_heights[tile] = next_height as u8;
                    if timer < 3 {
                        planes.ground_heights[tile] =
                            smooth_ground_height(&planes.ground_heights, &planes.angles, x, y);
                    }
                    let next_height = i32::from(planes.ground_heights[tile]);

                    if map_type == MapType::Cave {
                        let ceiling_target = (next_height + 64).min(254);
                        let old_ceiling = i32::from(planes.secondary_heights[tile]);
                        planes.secondary_heights[tile] =
                            (old_ceiling - (old_ceiling - ceiling_target) / timer) as u8;
                    }
                }

                let upper_left = i32::from(
                    planes.ground_heights[terrain_index(x.wrapping_sub(1), y.wrapping_sub(1))],
                );
                let down_right = i32::from(
                    planes.ground_heights[terrain_index(x.wrapping_add(1), y.wrapping_add(1))],
                );
                let shade = legacy_shade(upper_left - down_right + 32);
                planes.shading[tile] = if map_type == MapType::Day {
                    shade as u8
                } else {
                    (64 - shade) as u8
                };
                if map_type == MapType::Cave {
                    if planes.secondary_heights[tile] > planes.ground_heights[tile] {
                        planes.angles[tile] &= 0xf7;
                    } else {
                        planes.angles[tile] |= 8;
                    }
                }
            }
        }
    }

    if entity_rng != source.entity_rng {
        arena.entities[source_index].set_entity_rng(entity_rng);
        arena.economy.source_rng.insert(source_slot, entity_rng);
    }
}

/// sub_439A0, used to restore source ground without flattening diagonal
/// topology. Its asymmetric height limits are part of the remc2 smoothing.
fn smooth_ground_height(ground: &[u8], angles: &[u8], x: u8, y: u8) -> u8 {
    let center = ground[terrain_index(x, y)];
    if angles[terrain_index(x, y)] & 7 == 0 {
        return center;
    }
    let neighbors = [
        ground[terrain_index(x, y.wrapping_sub(1))],
        ground[terrain_index(x.wrapping_add(1), y.wrapping_sub(1))],
        ground[terrain_index(x.wrapping_add(1), y)],
        ground[terrain_index(x.wrapping_add(1), y.wrapping_add(1))],
        ground[terrain_index(x, y.wrapping_add(1))],
        ground[terrain_index(x.wrapping_sub(1), y.wrapping_add(1))],
        ground[terrain_index(x.wrapping_sub(1), y)],
        ground[terrain_index(x.wrapping_sub(1), y.wrapping_sub(1))],
    ];
    let minimum = neighbors
        .iter()
        .copied()
        .min()
        .unwrap_or(center)
        .min(center);
    let maximum = neighbors
        .iter()
        .copied()
        .max()
        .unwrap_or(center)
        .max(center);
    let average = (neighbors.iter().map(|value| u16::from(*value)).sum::<u16>() >> 3) as u8;
    let center_to_minimum = center - minimum;
    let maximum_to_center = maximum - center;

    if center_to_minimum <= 4 {
        if maximum_to_center <= 4 {
            return center;
        }
        if maximum_to_center <= 10 {
            return ((u16::from(center) + u16::from(average)) >> 1) as u8;
        }
    } else if center_to_minimum <= 10 {
        return ((u16::from(center) + u16::from(average)) >> 1) as u8;
    }
    average
}

/// Restore final source terrain, release any remaining model-2 capture links,
/// and hide this source for the scheduler's next-pass reclaim prologue.
fn finish_shrinking(
    arena: &mut SourceArena,
    source_slot: u16,
    terrain: &mut SessionTerrain,
) -> StepOutcome {
    let source_index = usize::from(source_slot);
    let source_position = arena.entities[source_index].position;
    let (center_x, center_y) = rounded_center(source_position);
    let start_x = center_x.wrapping_sub(15);
    let start_y = center_y.wrapping_sub(15);
    {
        let (planes, _, _, _) = terrain.disaster_parts();
        for row in 0..30_u8 {
            for column in 0..30_u8 {
                let x = start_x.wrapping_add(column);
                let y = start_y.wrapping_add(row);
                let tile = terrain_index(x, y);
                let restored = smooth_ground_height(&planes.ground_heights, &planes.angles, x, y);
                planes.ground_heights[tile] = restored;
            }
        }
    }
    let typed_castle_slots: Vec<_> = arena
        .economy
        .live_castles()
        .filter_map(|castle| {
            (castle.source_class == 3
                && castle.source_model == 2
                && castle.control_flags & 0x0010_0000 != 0
                && arena
                    .entities
                    .get(usize::from(castle.source_entity_index))
                    .is_some_and(|entity| {
                        u16::from_le_bytes([entity.record[42], entity.record[43]]) == source_slot
                    }))
            .then_some(castle.source_entity_index)
        })
        .collect();
    for castle_slot in typed_castle_slots {
        let released = arena.economy.set_gravity_castle_capture(castle_slot, false);
        debug_assert!(
            released,
            "the linked live Castle must accept capture release"
        );
        if let Some(entity) = arena.entities.get_mut(usize::from(castle_slot)) {
            entity.record[42..44].fill(0);
            entity.set_control_flags(entity.control_flags & !0x0010_0000);
        }
    }
    for entity in arena.entities.iter_mut().filter(|entity| {
        entity.active
            && entity.model == 2
            && entity.control_flags & 0x0010_0000 != 0
            && u16::from_le_bytes([entity.record[42], entity.record[43]]) == source_slot
    }) {
        entity.record[42..44].fill(0);
        entity.set_control_flags(entity.control_flags & !0x0010_0000);
    }
    arena.entities[source_index].disable_drawing();
    let (_, _, _, sequence) = terrain.disaster_parts();
    *sequence = sequence.wrapping_add(1);
    StepOutcome {
        hide: true,
        terrain_mutated: true,
        ..StepOutcome::default()
    }
}

impl StepOutcome {
    fn merge(&mut self, other: Self) {
        self.hide |= other.hide;
        self.sound_id = self.sound_id.or(other.sound_id);
        self.pull_actors |= other.pull_actors;
        self.terrain_mutated |= other.terrain_mutated;
        if let Some(award) = other.spell_experience {
            self.spell_experience = Some(award);
        }
    }
}

fn can_start_rising(
    entities: &[EntitySnapshot],
    source_slot: u16,
    terrain: &SessionTerrain,
) -> bool {
    let source = &entities[usize::from(source_slot)];
    let (center_x, center_y) = rounded_center(source.position);
    let mut empty_tiles = 0;
    for row in 0..30_u8 {
        for column in 0..30_u8 {
            let tile = terrain_index(
                center_x.wrapping_sub(15).wrapping_add(column),
                center_y.wrapping_sub(15).wrapping_add(row),
            );
            empty_tiles += usize::from(terrain.planes().terrain_types[tile] == 0);
        }
    }
    if empty_tiles >= 225 {
        return false;
    }

    let left = center_x.wrapping_sub(27);
    let top = center_y.wrapping_sub(27);
    !entities.iter().any(|other| {
        other.active
            && other.index != source_slot
            && other.class == SOURCE_CLASS
            && (other.model == SOURCE_MODEL
                || (other.model == 45 && matches!(other.action, 48 | 51)))
            && entity_in_source_scan(other, left, top)
    })
}

fn entity_in_source_scan(entity: &EntitySnapshot, left: u8, top: u8) -> bool {
    let tile_x = entity.position.x.to_be_bytes()[0];
    let tile_y = entity.position.y.to_be_bytes()[0];
    tile_x.wrapping_sub(left) < 54 && tile_y.wrapping_sub(top) < 54
}

fn step_rise_phase_two(
    arena: &mut SourceArena,
    source_slot: u16,
    terrain: &mut SessionTerrain,
) -> StepOutcome {
    let source_index = usize::from(source_slot);
    let source = &mut arena.entities[source_index];
    let timer = source.source_dword_10().wrapping_sub(1);
    source.set_source_dword_10(timer);
    if timer <= 0 {
        source.record[74] = 3;
        return StepOutcome {
            pull_actors: true,
            ..StepOutcome::default()
        };
    }

    let source_snapshot = source.clone();
    let mut outcome = deform_crater(arena, source_slot, &source_snapshot, timer, terrain);
    if timer < 6 {
        outcome.pull_actors = true;
    }
    outcome
}

fn deform_crater(
    arena: &mut SourceArena,
    source_slot: u16,
    source: &EntitySnapshot,
    timer: i32,
    terrain: &mut SessionTerrain,
) -> StepOutcome {
    let (center_x, center_y) = rounded_center(source.position);
    let start_x = center_x.wrapping_sub(15);
    let start_y = center_y.wrapping_sub(15);
    let mut material_changed = false;
    let mut source_word_2c = i32::from(source.source_word_2c());
    {
        let (planes, map_type, _, _) = terrain.disaster_parts();
        for row in 0..30_u8 {
            for column in 0..30_u8 {
                let x = start_x.wrapping_add(column);
                let y = start_y.wrapping_add(row);
                let tile_position = Position {
                    x: u16::from(x) << 8,
                    y: u16::from(y) << 8,
                    z: 0,
                };
                let distance = horizontal_distance(source.position, tile_position);
                let tile = terrain_index(x, y);
                if distance < 3_840 {
                    let target_height = if distance >= 2_304 {
                        let heading = heading_for_positions(source.position, tile_position) as i16;
                        let mut sample = source.position;
                        move_entity(&mut sample, heading, 0, 3_840);
                        let altitude =
                            interpolate_ground(&planes.ground_heights, sample.x, sample.y) >> 5;
                        if source_word_2c < altitude {
                            source_word_2c = altitude;
                            arena.entities[usize::from(source_slot)]
                                .set_source_word_2c(altitude as i16);
                        }
                        let sine_index = 512 + (((distance - 2_304) << 10) / 1_536) as usize;
                        let blend = (65_536 + SIN_16_16[sine_index]) / 2;
                        altitude
                            - ((blend * (altitude - (i32::from(source.position.z) + 64))) >> 16)
                    } else {
                        let sine_index = 512 + (((2_304 - distance) << 9) / 2_304) as usize;
                        let depth = ((65_536 - SIN_16_16[sine_index]) << 6) >> 16;
                        i32::from(source.position.z) + 64 - depth
                    };
                    let old_height = i32::from(planes.ground_heights[tile]);
                    let next_height =
                        (old_height + (target_height - old_height) / timer).clamp(1, 255) as u8;
                    planes.ground_heights[tile] = next_height;

                    if map_type == MapType::Cave {
                        let ceiling_target = (i32::from(next_height) + 64).min(254);
                        let old_ceiling = i32::from(planes.secondary_heights[tile]);
                        planes.secondary_heights[tile] =
                            (old_ceiling - (old_ceiling - ceiling_target) / timer) as u8;
                    }
                    if i32::from(next_height) <= i32::from(source.position.z) + 64
                        && i32::from(next_height) >= i32::from(source.position.z) + 6 * timer
                        && building_topology_rebuild_required(planes.terrain_types[tile])
                    {
                        planes.terrain_types[tile] = 1;
                        planes.angles[tile] = (planes.angles[tile] & 0xf8) | 1;
                        material_changed = true;
                    }
                }

                let upper_left = i32::from(
                    planes.ground_heights[terrain_index(x.wrapping_sub(1), y.wrapping_sub(1))],
                );
                let down_right = i32::from(
                    planes.ground_heights[terrain_index(x.wrapping_add(1), y.wrapping_add(1))],
                );
                let shade = legacy_shade(upper_left - down_right + 32);
                planes.shading[tile] = if map_type == MapType::Day {
                    shade as u8
                } else {
                    (64 - shade) as u8
                };
                if map_type == MapType::Cave {
                    if planes.secondary_heights[tile] > planes.ground_heights[tile] {
                        planes.angles[tile] &= 0xf7;
                    } else {
                        planes.angles[tile] |= 8;
                    }
                }
            }
        }
    }

    let mut outcome = StepOutcome {
        terrain_mutated: true,
        ..StepOutcome::default()
    };
    if timer == 5 {
        outcome.spell_experience = destroy_and_reward(arena, source_slot, source);
        let (planes, _, _, _) = terrain.disaster_parts();
        for row in 0..30_u8 {
            for column in 0..30_u8 {
                let x = start_x.wrapping_add(column);
                let y = start_y.wrapping_add(row);
                let tile = terrain_index(x, y);
                if matches!(planes.terrain_types[tile], 10..=13 | 26) {
                    planes.terrain_types[tile] = 1;
                    planes.angles[tile] = (planes.angles[tile] & 0xf8) | 1;
                }
            }
        }
        material_changed = true;
    }
    if material_changed {
        let (planes, map_type, rng, _) = terrain.disaster_parts();
        rebuild_building_topology(
            planes,
            start_x,
            start_y,
            center_x.wrapping_add(15),
            center_y.wrapping_add(15),
            map_type,
            rng,
        );
    }

    let (planes, map_type, _, _) = terrain.disaster_parts();
    for row in 0..2_u8 {
        for column in 0..2_u8 {
            let x = center_x.wrapping_sub(1).wrapping_add(column);
            let y = center_y.wrapping_sub(1).wrapping_add(row);
            let tile = terrain_index(x, y);
            let height = i32::from(planes.ground_heights[tile]);
            planes.ground_heights[tile] = (height - height / timer).clamp(0, 255) as u8;
            planes.shading[tile] = if map_type == MapType::Day {
                (31 / timer + 32) as u8
            } else {
                (-31 / timer + 32) as u8
            };
        }
    }
    let (_, _, _, sequence) = terrain.disaster_parts();
    *sequence = sequence.wrapping_add(1);
    outcome
}

fn destroy_and_reward(
    arena: &mut SourceArena,
    source_slot: u16,
    source: &EntitySnapshot,
) -> Option<SpellExperienceAward> {
    for entity in arena.entities.iter_mut().filter(|entity| {
        entity.active
            && entity.index != source_slot
            && entity.class == SOURCE_CLASS
            && entity.model == 45
    }) {
        if overlaps_source(source, entity) {
            entity.set_life(-1);
            entity.font_type_index = 0;
            entity.record[65] = 0;
        }
    }

    let typed_castles: Vec<_> = arena
        .economy
        .live_castles()
        .copied()
        .filter(|castle| {
            castle.source_class == 3
                && castle.source_model == 2
                && castle.source_entity_index != source_slot
        })
        .collect();
    let mut experience = 0;
    for castle in typed_castles.iter().filter(|castle| {
        overlaps_source_bounds(
            source,
            castle.position,
            castle.shift_pitch,
            castle.shift_roll,
        )
    }) {
        if arena.economy.apply_wind_castle_contact(
            castle.source_entity_index,
            source_slot,
            source.source_id,
            source.subspell_index,
        ) && arena
            .economy
            .set_gravity_castle_capture(castle.source_entity_index, true)
        {
            let entity = &mut arena.entities[usize::from(castle.source_entity_index)];
            capture_model2_projection(entity, source_slot, source);
            experience += 2;
        }
    }
    for entity in arena.entities.iter_mut().filter(|entity| {
        entity.active
            && entity.index != source_slot
            && !typed_castles
                .iter()
                .any(|castle| castle.source_entity_index == entity.index)
            && entity.class == 3
            && entity.model == 2
    }) {
        if !overlaps_source(source, entity) {
            continue;
        }
        capture_model2_projection(entity, source_slot, source);
        experience += 2;
    }
    (experience != 0).then_some(SpellExperienceAward {
        owner_source_id: source.source_id,
        family: 20,
        amount: experience,
    })
}

fn capture_model2_projection(
    entity: &mut EntitySnapshot,
    source_slot: u16,
    source: &EntitySnapshot,
) {
    entity.set_control_flags(entity.control_flags | 0x0010_0000);
    entity.record[52..54].copy_from_slice(&30_i16.to_le_bytes());
    entity.record[42..44].copy_from_slice(&source_slot.to_le_bytes());
    let bounty = i32::from_le_bytes([
        entity.record[98],
        entity.record[99],
        entity.record[100],
        entity.record[101],
    ]);
    entity.record[98..102].copy_from_slice(
        &bounty
            .wrapping_add(i32::from(source.subspell_index))
            .to_le_bytes(),
    );
    entity.record[102..104].copy_from_slice(&source.source_id.to_le_bytes());
}

fn finish_rising(
    arena: &mut SourceArena,
    source_slot: u16,
    terrain: &mut SessionTerrain,
) -> StepOutcome {
    let source = &mut arena.entities[usize::from(source_slot)];
    let (center_x, center_y) = rounded_center(source.position);
    let start_x = center_x.wrapping_sub(15);
    let start_y = center_y.wrapping_sub(15);
    {
        let (planes, _, _, _) = terrain.disaster_parts();
        for row in 0..30_u8 {
            for column in 0..30_u8 {
                let x = start_x.wrapping_add(column);
                let y = start_y.wrapping_add(row);
                let tile = terrain_index(x, y);
                if building_topology_rebuild_required(planes.terrain_types[tile])
                    || planes.terrain_types[tile] == 8
                {
                    planes.terrain_types[tile] = 1;
                    planes.angles[tile] = (planes.angles[tile] & 0xf8) | 1;
                }
            }
        }
    }
    add_building_rectangle(
        terrain,
        start_x,
        start_y,
        center_x.wrapping_add(15),
        center_y.wrapping_add(15),
    );
    {
        let (planes, map_type, _, _) = terrain.disaster_parts();
        for row in 0..2_u8 {
            for column in 0..2_u8 {
                let tile = terrain_index(
                    center_x.wrapping_sub(1).wrapping_add(column),
                    center_y.wrapping_sub(1).wrapping_add(row),
                );
                planes.shading[tile] = if map_type == MapType::Day { 63 } else { 1 };
            }
        }
    }
    source.set_source_action(ACTION_HOLD);
    let (_, _, _, sequence) = terrain.disaster_parts();
    *sequence = sequence.wrapping_add(1);
    StepOutcome {
        terrain_mutated: true,
        pull_actors: true,
        ..StepOutcome::default()
    }
}

fn add_building_rectangle(
    terrain: &mut SessionTerrain,
    start_x: u8,
    start_y: u8,
    end_x: u8,
    end_y: u8,
) {
    let width = end_x.wrapping_sub(start_x).wrapping_add(1);
    let height = end_y.wrapping_sub(start_y).wrapping_add(1);
    let (planes, map_type, rng, _) = terrain.disaster_parts();
    for row in 0..height {
        for column in 0..width {
            let x = start_x.wrapping_add(column);
            let y = start_y.wrapping_add(row);
            for (mark_x, mark_y) in [
                (x, y),
                (x.wrapping_sub(1), y),
                (x.wrapping_sub(1), y.wrapping_sub(1)),
                (x, y.wrapping_sub(1)),
            ] {
                planes.terrain_types[terrain_index(mark_x, mark_y)] = 1;
            }
        }
    }
    rebuild_building_topology(planes, start_x, start_y, end_x, end_y, map_type, rng);
}

fn overlaps_source(source: &EntitySnapshot, target: &EntitySnapshot) -> bool {
    let target_x = i16::from_le_bytes([target.record[88], target.record[89]]);
    let target_y = i16::from_le_bytes([target.record[90], target.record[91]]);
    overlaps_source_bounds(source, target.position, target_x, target_y)
}

fn overlaps_source_bounds(
    source: &EntitySnapshot,
    target_position: Position,
    target_half_x: i16,
    target_half_y: i16,
) -> bool {
    // CompareAxisWithShift narrows each subtraction to signed 16-bit before
    // absolute value and uses strict interior comparisons on both axes.
    let source_x = i16::from_le_bytes([source.record[88], source.record[89]]);
    let source_y = i16::from_le_bytes([source.record[90], source.record[91]]);
    let dx = i16::from_ne_bytes(
        source
            .position
            .x
            .wrapping_sub(target_position.x)
            .to_ne_bytes(),
    )
    .unsigned_abs();
    let dy = i16::from_ne_bytes(
        source
            .position
            .y
            .wrapping_sub(target_position.y)
            .to_ne_bytes(),
    )
    .unsigned_abs();
    i32::from(dx) < i32::from(source_x) + i32::from(target_half_x)
        && i32::from(dy) < i32::from(source_y) + i32::from(target_half_y)
}

fn rounded_center(position: Position) -> (u8, u8) {
    (
        position.x.wrapping_add(128).to_be_bytes()[0],
        position.y.wrapping_add(128).to_be_bytes()[0],
    )
}

fn legacy_shade(value: i32) -> i32 {
    if value >= 28 {
        if value > 40 { (value & 7) + 40 } else { value }
    } else {
        (value & 3) + 28
    }
}

/// The field's update calls `sub_39B60` after the rise timer enters its last
/// five ticks, during phase-three settle, on action73 while life remains
/// positive, and once at action74 stage zero before it captures new rows.
pub(crate) fn pull_is_active(source: &EntitySnapshot) -> bool {
    if !source.active || (source.class, source.model) != (SOURCE_CLASS, SOURCE_MODEL) {
        return false;
    }
    match source.action {
        ACTION_RISE => {
            let Some(phase) = source.record.get(74).copied() else {
                return false;
            }; // byte_0x46_70
            let Some(timer_bytes) = source
                .record
                .get(20..24)
                .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
            else {
                return false;
            };
            let timer = i32::from_le_bytes(timer_bytes);
            phase == 3 || (phase == 2 && timer < 6)
        }
        ACTION_HOLD => source.life > 0,
        ACTION_SHRINK => source
            .record
            .get(74)
            .is_some_and(|phase| matches!(*phase, 0 | 1)),
        _ => false,
    }
}

/// Exact target admission from `sub_39FA0`; source ids keep a player from
/// being pulled by a well it owns.
pub(crate) fn accepts_actor(source: &EntitySnapshot, actor: Actor) -> bool {
    match actor.class {
        1 | 4 | 6..=8 | 11..=13 | 15 => false,
        2 => true,
        3 => match actor.model {
            2 => false,
            0 | 1 => actor.control_flags & 0x21 == 0 && actor.source_id != source.source_id,
            _ => true,
        },
        5 => {
            actor.control_flags & 0x21 == 0
                && actor.action != 232
                && actor.model != 22
                && !(actor.model == 27 && matches!(actor.action, 233 | 234))
        }
        9 => actor.model == 0 || matches!(actor.model, 13 | 14),
        10 => matches!(actor.model, 6 | 39 | 40 | 57),
        14 => actor.control_flags & 0x21 == 0 && actor.model != 1,
        _ => true,
    }
}

/// Run the per-actor portion of `sub_39B60`. `None` means either the source
/// is outside its pull window, the target fails source admission, or it lies
/// outside the strict horizontal radius/vertical band.
pub(crate) fn pull_actor(
    source: &mut EntitySnapshot,
    mut actor: Actor,
    terrain: &impl FlightWorld,
) -> Option<Motion> {
    if !pull_is_active(source) {
        return None;
    }

    let distance = horizontal_distance(source.position, actor.position);
    let relative_z = i32::from(actor.position.z) - i32::from(source.source_word_2c());
    // LABEL25 follows sub_3A200, so a newly captured contact is released on
    // this same action-74 visit. Test the flag after each mutation instead of
    // snapshotting it before contact.
    let cleanup_visit = source.action == ACTION_SHRINK && source.record.get(74) == Some(&0);
    if !accepts_actor(source, actor) {
        if cleanup_visit && actor.control_flags & 0x0010_0000 != 0 {
            release_actor_capture(&mut actor);
            return Some(Motion {
                actor,
                source_rng: None,
                contact: false,
                spell_experience: None,
            });
        }
        return None;
    }
    if distance >= PULL_RADIUS || relative_z >= 4_096 {
        if cleanup_visit && actor.control_flags & 0x0010_0000 != 0 {
            release_actor_capture(&mut actor);
            return Some(Motion {
                actor,
                source_rng: None,
                contact: false,
                spell_experience: None,
            });
        }
        return None;
    }

    if distance <= 32 || relative_z <= 96 {
        actor.control_flags |= 0x0010_0001;
        if (actor.class, actor.model) == (3, 0) {
            actor.pitch = 512;
        }
        let always_contact = actor.class == 5 && matches!(actor.model, 12 | 18);
        let bounty_suppressed = actor.class == 5 && actor.model == 27;
        let source_rng = if always_contact {
            None
        } else {
            let entity_rng = next_rng(source.entity_rng);
            source.set_entity_rng(entity_rng);
            Some(entity_rng)
        };
        let awards_contact = always_contact || source_rng.is_some_and(|rng| rng % 7 == 0);
        let spell_experience = if awards_contact && !bounty_suppressed && actor.bounty_enabled {
            actor.bounty = actor.bounty.wrapping_add(actor.life.wrapping_add(1));
            actor.bounty_attacker = source.source_id;
            Some(SpellExperienceAward {
                owner_source_id: source.source_id,
                family: 20,
                amount: 1,
            })
        } else {
            None
        };
        if cleanup_visit && actor.control_flags & 0x0010_0000 != 0 {
            release_actor_capture(&mut actor);
        }
        return Some(Motion {
            actor,
            source_rng,
            contact: true,
            spell_experience,
        });
    }

    let mut position = actor.position;
    let heading = heading_for_positions(position, source.position) as i16;
    let speed = pull_speed(distance).min(distance);
    move_entity(&mut position, heading, 0, speed as i16);

    let player_model_zero = (actor.class, actor.model) == (3, 0);
    let suppress_vertical_pull = actor.vertical_state < -64 && !player_model_zero;
    if !suppress_vertical_pull {
        // Preserve sub_39B60's fixed-point order: scale the remaining height
        // to 12-bit precision before multiplying by 48 and shifting back.
        let fixed_steps = ((4_096 - relative_z) << 8) >> 12;
        let vertical_step = (fixed_steps * 48) >> 8;
        position.z = position.z.wrapping_sub(vertical_step as i16);
    }
    let ground = terrain.ground_height(position);
    position.z = if suppress_vertical_pull {
        ground
    } else {
        position.z.max(ground)
    };
    actor.position = position;
    if cleanup_visit && actor.control_flags & 0x0010_0000 != 0 {
        release_actor_capture(&mut actor);
    }
    Some(Motion {
        actor,
        source_rng: None,
        contact: false,
        spell_experience: None,
    })
}

fn release_actor_capture(actor: &mut Actor) {
    actor.control_flags &= !0x0010_0000;
    actor.control_flags &= !1;
    if (actor.class, actor.model) == (3, 0) && actor.is_current_level_player {
        actor.control_flags |= 1;
    }
}

fn horizontal_distance(from: Position, to: Position) -> u32 {
    let dx = i32::from(to.x.wrapping_sub(from.x) as i16);
    let dy = i32::from(to.y.wrapping_sub(from.y) as i16);
    crate::legacy_sqrt(dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32)
}

fn pull_speed(distance: u32) -> u32 {
    let scaled = (u64::from(PULL_RADIUS - distance) << 8) / u64::from(PULL_RADIUS);
    (((scaled << 7) >> 8) as u32).clamp(MIN_PULL_SPEED, MAX_PULL_SPEED)
}

fn next_rng(rng: u16) -> u16 {
    rng.wrapping_mul(9_377).wrapping_add(9_439)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Tile, world::terrain::cave_features::new_event};

    struct FlatGround;

    impl FlightWorld for FlatGround {
        fn ground_height(&self, _: Position) -> i16 {
            0
        }

        fn ceiling_height(&self, _: Position) -> Option<i16> {
            None
        }

        fn material_mask(&self, _: Position) -> u32 {
            0
        }

        fn angle_flags(&self, _: Tile) -> u8 {
            0
        }
    }

    fn well() -> EntitySnapshot {
        let mut source = new_event(
            500,
            71,
            Position {
                x: 10_000,
                y: 10_000,
                z: 0,
            },
        );
        source.class = SOURCE_CLASS;
        source.model = SOURCE_MODEL;
        source.set_source_action(ACTION_HOLD);
        source.set_life(120);
        source
    }

    fn actor(class: u8, model: u8, position: Position) -> Actor {
        Actor {
            source_id: 100,
            class,
            model,
            action: 0,
            control_flags: 0,
            position,
            vertical_state: 0,
            pitch: 0,
            life: 0,
            bounty_enabled: false,
            bounty: 0,
            bounty_attacker: 0,
            capture_link: 0,
            is_current_level_player: false,
        }
    }

    #[test]
    fn gravity_well_pulls_an_admitted_actor_toward_its_center() {
        let mut source = well();
        let target = actor(
            2,
            0,
            Position {
                x: source.position.x.wrapping_add(1_000),
                y: source.position.y,
                z: 1_000,
            },
        );
        let before = horizontal_distance(source.position, target.position);
        let motion = pull_actor(&mut source, target, &FlatGround).unwrap();
        assert!(horizontal_distance(source.position, motion.actor.position) < before);
    }

    #[test]
    fn gravity_well_leaves_source_excluded_player_models_untouched() {
        let mut source = well();
        let target = actor(
            3,
            2,
            Position {
                x: source.position.x.wrapping_add(1_000),
                y: source.position.y,
                z: 1_000,
            },
        );
        assert!(pull_actor(&mut source, target, &FlatGround).is_none());
    }

    #[test]
    fn gravity_well_does_not_mutate_source_for_rejected_class4_actor() {
        let mut source = well();
        let before = source.clone();
        let target = actor(
            4,
            0,
            Position {
                x: source.position.x.wrapping_add(1_000),
                y: source.position.y,
                z: 1_000,
            },
        );

        assert!(pull_actor(&mut source, target, &FlatGround).is_none());
        assert_eq!(source, before);
    }

    #[test]
    fn gravity_castle_overlap_uses_strict_shifted_axis_extents() {
        let mut source = well();
        source.set_source_overlap_half_extents(100, 200);
        let inside = Position {
            x: source.position.x.wrapping_add(149),
            y: source.position.y.wrapping_add(249),
            z: 0,
        };
        let on_boundary = Position {
            x: source.position.x.wrapping_add(150),
            y: source.position.y.wrapping_add(250),
            z: 0,
        };

        assert!(overlaps_source_bounds(&source, inside, 50, 50));
        assert!(!overlaps_source_bounds(&source, on_boundary, 50, 50));
    }

    #[test]
    fn gravity_well_vertical_pull_keeps_source_fixed_point_fraction() {
        let mut source = well();
        let target = actor(
            2,
            0,
            Position {
                x: source.position.x.wrapping_add(1_000),
                y: source.position.y,
                z: 2_048,
            },
        );

        let motion = pull_actor(&mut source, target, &FlatGround).unwrap();
        assert_eq!(motion.actor.position.z, 2_024);
    }

    #[test]
    fn gravity_well_suppressed_vertical_pull_snaps_to_ground() {
        let mut source = well();
        let mut target = actor(
            2,
            0,
            Position {
                x: source.position.x.wrapping_add(1_000),
                y: source.position.y,
                z: 512,
            },
        );
        target.vertical_state = -65;

        let motion = pull_actor(&mut source, target, &FlatGround).unwrap();
        assert_eq!(motion.actor.position.z, 0);
    }

    #[test]
    fn action74_releases_a_captured_player_in_its_source_callback_window() {
        let mut source = well();
        source.set_source_action(ACTION_SHRINK);
        source.set_life(0);
        source.record[74] = 0;
        let mut target = actor(
            3,
            0,
            Position {
                x: source.position.x.wrapping_add(1_000),
                y: source.position.y,
                z: 0,
            },
        );
        target.source_id = source.source_id.wrapping_add(1);
        target.control_flags = 0x0010_0000;
        target.capture_link = source.index;
        target.is_current_level_player = true;

        let motion = pull_actor(&mut source, target, &FlatGround).unwrap();
        assert_eq!(motion.actor.capture_link, source.index);
        assert_eq!(motion.actor.control_flags & 0x0010_0000, 0);
        assert_eq!(motion.actor.control_flags & 1, 1);
    }

    #[test]
    fn action74_releases_capture_created_by_same_pass_contact() {
        let mut source = well();
        source.set_source_action(ACTION_SHRINK);
        source.set_life(0);
        source.record[74] = 0;
        let target = actor(
            5,
            12,
            Position {
                x: source.position.x.wrapping_add(1),
                y: source.position.y,
                z: 0,
            },
        );

        let motion = pull_actor(&mut source, target, &FlatGround).unwrap();
        assert!(motion.contact);
        assert_eq!(motion.actor.control_flags & 0x0010_0000, 0);
        assert_eq!(motion.actor.control_flags & 1, 0);
    }

    #[test]
    fn action74_releases_scanned_capture_outside_pull_radius_and_admission() {
        let mut source = well();
        source.set_source_action(ACTION_SHRINK);
        source.set_life(0);
        source.record[74] = 0;
        let mut target = actor(
            3,
            2,
            Position {
                x: source.position.x.wrapping_add(PULL_RADIUS as u16 + 1),
                y: source.position.y,
                z: 0,
            },
        );
        target.control_flags = 0x0010_0000;
        target.capture_link = source.index;

        let motion = pull_actor(&mut source, target, &FlatGround).unwrap();
        assert!(!motion.contact);
        assert_eq!(motion.actor.capture_link, source.index);
        assert_eq!(motion.actor.control_flags & 0x0010_0000, 0);
    }

    #[test]
    fn gravity_contact_bounty_uses_target_life() {
        let mut source = well();
        let mut target = actor(
            5,
            12,
            Position {
                x: source.position.x.wrapping_add(1),
                y: source.position.y,
                z: 0,
            },
        );
        target.life = 17;
        target.bounty_enabled = true;
        target.bounty = 5;

        let motion = pull_actor(&mut source, target, &FlatGround).unwrap();
        assert_eq!(motion.actor.bounty, 23);
        assert_eq!(motion.actor.bounty_attacker, source.source_id);
        assert_eq!(motion.spell_experience.unwrap().amount, 1);
    }
}
