//! Source terrain-strip and dual-plane cave-corridor controllers.
//!
//! This is the reusable Rust projection of pinned remc2 `sub_59F60`. The
//! caller owns source-slot scheduling, phase advancement, and audio journal
//! publication. This module owns the coupled controller/terrain transaction
//! and returns the exact sound-47 lifecycle request produced by one callback.

use super::{EntitySnapshot, MapType, Position, SessionTerrain, TerrainPlanes};

/// Cave corridor state and geometry, corresponding to `sub_5B100`.
///
/// The source arena retains the controller's authoritative words. This owner
/// borrows only that row and the terrain; it never acquires a whole session.
pub(crate) use crate::world::terrain::cave_corridors;

const TERRAIN_STRIP_TYPE: u8 = 8;
const TERRAIN_STRIP_SOUND: u16 = 47;

/// Authored class-10/model-63/action-68 command created by `sub_4F900`.
///
/// The command is a real `NewEvent` row: it owns a numeric source slot, is
/// map-linked at the authored position, closes the controller in its cell,
/// then remains as a drawing-disabled tombstone through the common callback
/// epilogue before the following source boundary reclaims it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerrainStripCloseTrigger {
    pub(crate) source_entity_index: u16,
    pub(crate) entity_rng: u16,
    pub(crate) authored_index: u16,
    pub(crate) class: u8,
    pub(crate) model: u8,
    pub(crate) action: u8,
    pub(crate) max_life: i32,
    pub(crate) life: i32,
    pub(crate) control_flags: u32,
    pub(crate) source_byte_3e: u8,
    pub(crate) position: Position,
    pub(crate) drawing_enabled: bool,
    pub(crate) active: bool,
}

/// Allocate an authored opening command through the shared source arena and
/// publish its map link to the terrain plane read by ordinary consumers.
pub(crate) fn allocate_close_trigger(
    arena: &mut crate::campaign::source_arena::SourceArena,
    terrain: &mut SessionTerrain,
    authored_index: u16,
    position: Position,
) -> Option<TerrainStripCloseTrigger> {
    let slot = arena.allocate_source_slot_with_terrain(terrain)?;
    let trigger =
        arena
            .economy
            .initialize_terrain_strip_close_trigger(slot, authored_index, position)?;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)
        .expect("the allocated terrain trigger has a valid source map slot");
    Some(trigger)
}

/// The command remains linked through its callback epilogue. Publish its
/// unlink before the following source-boundary reclamation releases its slot.
pub(crate) fn prepare_trigger_reclaim(
    arena: &mut crate::campaign::source_arena::SourceArena,
    terrain: &mut SessionTerrain,
) -> Result<(), String> {
    let hidden = arena
        .economy
        .terrain_strip_close_triggers()
        .iter()
        .filter(|trigger| !trigger.active)
        .map(|trigger| (trigger.source_entity_index, trigger.position))
        .collect::<Vec<_>>();
    for (slot, position) in hidden {
        crate::world::effects::sync_position(arena, terrain, slot, position, false, false)?;
    }
    Ok(())
}

/// Mutable source words used by `sub_59F60` after the generic constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerrainStripControllerState {
    pub(crate) life: i32,
    pub(crate) subspell_index: u16,
    pub(crate) direction: i8,
    pub(crate) span: i32,
}

impl TerrainStripControllerState {
    #[must_use]
    pub(crate) const fn new(direction: i8, span: i32) -> Self {
        Self {
            life: 0,
            subspell_index: 0,
            direction,
            span,
        }
    }
}

/// Source `sub_51660` and the ordinary `sub_4A310` authored overrides.
/// The caller owns NewEvent allocation and map insertion; this owner binds
/// the hidden controller's authoritative words, consumed by every visit.
pub(crate) fn bind_startup_construction(source: &mut EntitySnapshot, direction: i8, span: i32) {
    source.class = 14;
    source.model = 1;
    source.record[67] = 14;
    source.record[68] = 1;
    source.set_source_action(6);
    source.set_max_life(0);
    source.set_life(0);
    source.set_control_flags((source.control_flags & !9) | 5);
    source.set_subspell_index(0);
    source.level_gate = direction;
    source.set_source_byte_46(direction as u8);
    source.set_source_dword_10(span);
}

/// Visit the same authoritative row from ordinary play and startup replay.
/// The caller publishes the returned audio before its common epilogue.
pub(crate) fn visit_source(
    source: &mut EntitySnapshot,
    terrain: &mut SessionTerrain,
) -> TerrainStripVisit {
    let mut controller = TerrainStripControllerState {
        life: source.life,
        subspell_index: source.subspell_index,
        direction: source.level_gate,
        span: source.source_dword_10(),
    };
    let visit = terrain.apply_terrain_strip_visit(source.position, &mut controller);
    source.set_life(controller.life);
    source.set_subspell_index(controller.subspell_index);
    source.set_source_dword_10(controller.span);
    visit
}

/// The two audio calls reachable from one terrain-strip callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerrainStripAudioOutcome {
    Prepare47,
    EndLoop47,
}

impl TerrainStripAudioOutcome {
    #[must_use]
    pub(crate) const fn sound_id(self) -> u16 {
        TERRAIN_STRIP_SOUND
    }
}

/// Observable result of one source callback, before the common phase epilogue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerrainStripVisit {
    pub(crate) audio: Option<TerrainStripAudioOutcome>,
    pub(crate) terrain_mutated: bool,
}

impl TerrainStripVisit {
    const INERT: Self = Self {
        audio: None,
        terrain_mutated: false,
    };
}

/// Packed source map cursor: X is the low byte and Y is the high byte.
///
/// remc2 deliberately mixes whole-word arithmetic with byte-field arithmetic.
/// Keeping both forms explicit preserves carry behavior at map seams.
#[derive(Clone, Copy)]
pub(crate) struct TileCursor(u16);

impl TileCursor {
    pub(crate) const fn from_xy(x: u8, y: u8) -> Self {
        Self(u16::from_le_bytes([x, y]))
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }

    pub(crate) const fn x(self) -> u8 {
        self.0.to_le_bytes()[0]
    }

    pub(crate) const fn y(self) -> u8 {
        self.0.to_le_bytes()[1]
    }

    pub(crate) fn set_x(&mut self, x: u8) {
        self.0 = u16::from_le_bytes([x, self.y()]);
    }

    pub(crate) fn set_y(&mut self, y: u8) {
        self.0 = u16::from_le_bytes([self.x(), y]);
    }

    pub(crate) fn add_x(&mut self, delta: i32) {
        self.set_x(self.x().wrapping_add(delta as u8));
    }

    pub(crate) fn add_y(&mut self, delta: i32) {
        self.set_y(self.y().wrapping_add(delta as u8));
    }

    const fn word_add(self, delta: i32) -> Self {
        Self(self.0.wrapping_add(delta as u16))
    }

    fn add_word(&mut self, delta: i32) {
        self.0 = self.0.wrapping_add(delta as u16);
    }
}

impl SessionTerrain {
    /// Execute one `sub_59F60` visit transactionally.
    ///
    /// Controller words always commit. Terrain commits, and the diagnostic
    /// mutation sequence advances once, only when the source branch writes a
    /// terrain plane. The callback never reads or advances either RNG stream.
    pub(crate) fn apply_terrain_strip_visit(
        &mut self,
        position: Position,
        controller: &mut TerrainStripControllerState,
    ) -> TerrainStripVisit {
        let mut staged_terrain = self.clone();
        let mut staged_controller = *controller;
        let visit = apply_visit_inner(
            &mut staged_terrain.planes,
            staged_terrain.map_type,
            position,
            &mut staged_controller,
        );
        *controller = staged_controller;
        if visit.terrain_mutated {
            staged_terrain.mutation_sequence = staged_terrain.mutation_sequence.wrapping_add(1);
            *self = staged_terrain;
        }
        visit
    }
}

fn apply_visit_inner(
    planes: &mut TerrainPlanes,
    map_type: MapType,
    position: Position,
    controller: &mut TerrainStripControllerState,
) -> TerrainStripVisit {
    debug_assert_eq!(planes.terrain_types.len(), 65_536);
    debug_assert_eq!(planes.ground_heights.len(), 65_536);
    debug_assert_eq!(planes.shading.len(), 65_536);
    debug_assert_eq!(planes.angles.len(), 65_536);
    debug_assert_eq!(planes.secondary_heights.len(), 65_536);

    match controller.life {
        0 => initialize_strip(planes, map_type, position, controller),
        1 => open_strip(planes, map_type, position, controller),
        2 => close_strip(planes, map_type, position, controller),
        _ => TerrainStripVisit::INERT,
    }
}

fn rounded_tile(position: Position) -> TileCursor {
    TileCursor::from_xy(
        position.x.wrapping_add(128).to_be_bytes()[0],
        position.y.wrapping_add(128).to_be_bytes()[0],
    )
}

fn working_tile(position: Position, direction: i8) -> TileCursor {
    let mut cursor = rounded_tile(position);
    if direction == 0 {
        cursor.add_word(-1);
    } else {
        cursor.add_y(-1);
    }
    cursor
}

fn cave_adjust(planes: &mut TerrainPlanes, cursor: TileCursor) {
    let index = cursor.index();
    let height = planes.ground_heights[index];
    if planes.secondary_heights[index] > height {
        planes.angles[index] &= 0xf7;
    } else {
        planes.secondary_heights[index] = height.wrapping_sub(1);
        planes.angles[index] |= 0x08;
    }
}

fn clear_cave_angle(planes: &mut TerrainPlanes, cursor: TileCursor) {
    planes.angles[cursor.index()] &= 0xf7;
}

fn mark_dirty(planes: &mut TerrainPlanes, cursor: TileCursor) {
    planes.angles[cursor.index()] |= 0x80;
}

fn source_shading(left: u8, right: u8, map_type: MapType) -> u8 {
    let raw = left.wrapping_sub(right).wrapping_add(32) as i8;
    let adjusted = if raw >= 28 {
        if raw > 40 { (raw & 7) + 40 } else { raw }
    } else {
        (raw & 3) + 28
    };
    if map_type == MapType::Day {
        adjusted as u8
    } else {
        64_i8.wrapping_sub(adjusted) as u8
    }
}

fn write_shading(
    planes: &mut TerrainPlanes,
    destination: TileCursor,
    left: TileCursor,
    right: TileCursor,
    map_type: MapType,
) {
    planes.shading[destination.index()] = source_shading(
        planes.ground_heights[left.index()],
        planes.ground_heights[right.index()],
        map_type,
    );
}

fn initialize_strip(
    planes: &mut TerrainPlanes,
    map_type: MapType,
    position: Position,
    controller: &mut TerrainStripControllerState,
) -> TerrainStripVisit {
    let mut base = rounded_tile(position);
    controller.span = controller.span.wrapping_add(1);
    if controller.direction == 0 {
        base.add_x(-1);
        initialize_direction_zero(planes, map_type, base, controller.span);
    } else {
        base.add_y(-1);
        if controller.direction == 1 {
            initialize_direction_one(planes, map_type, base, controller.span);
        }
    }
    controller.subspell_index = 48;
    controller.life = 3;
    TerrainStripVisit {
        audio: None,
        terrain_mutated: matches!(controller.direction, 0 | 1),
    }
}

fn initialize_direction_zero(
    planes: &mut TerrainPlanes,
    map_type: MapType,
    base: TileCursor,
    span: i32,
) {
    let mut along = base;
    let mut outer = 0_i32;
    while outer < span {
        let mut across = along;
        let mut inner = 0_i32;
        while inner < 2 {
            let index = across.index();
            let neighbor = across.word_add(1).index();
            let difference = (i16::from(planes.ground_heights[index])
                - i16::from(planes.ground_heights[neighbor]))
            .abs();
            if planes.terrain_types[index] != TERRAIN_STRIP_TYPE || difference > 30 {
                planes.ground_heights[index] = planes.ground_heights[index].wrapping_add(48);
            }
            across.add_y(-1);
            inner += 1;
        }
        along.add_word(1);
        outer += 1;
    }

    let mut run = -1_i32;
    let mut column = base.word_add(-1);
    while run < span {
        let mut cursor = column;
        let mut width = 0_i32;
        while width < 3 {
            let index = cursor.index();
            planes.terrain_types[index] = TERRAIN_STRIP_TYPE;
            planes.angles[index] = 1;
            cursor.add_y(-1);
            width += 1;
        }
        column.add_word(1);
        run += 1;
    }

    let mut run = -1_i32;
    let mut column = base.word_add(-1);
    while run < span + 1 {
        let mut cursor = column.word_add(256);
        let mut width = 0_i32;
        while width < 4 {
            if map_type == MapType::Cave {
                cave_adjust(planes, cursor);
            } else {
                clear_cave_angle(planes, cursor);
            }
            cursor.add_y(-1);
            width += 1;
        }
        column.add_word(1);
        run += 1;
    }

    let mut run = -1_i32;
    let mut cursor = base.word_add(-1);
    while run < span {
        cursor.add_x(1);
        cursor.add_y(1);
        let right = cursor;
        cursor.add_x(-2);
        cursor.add_y(-2);
        let left = cursor;
        cursor.add_x(1);
        cursor.add_y(1);
        write_shading(planes, cursor, left, right, map_type);
        cursor.add_word(1);
        run += 1;
    }

    let mut run = -3_i32;
    let mut column = base.word_add(-3);
    while run < span + 3 {
        let mut width = -3_i32;
        let mut cursor = column.word_add(768);
        while width < 6 {
            mark_dirty(planes, cursor);
            cursor.add_y(-1);
            width += 1;
        }
        column.add_word(1);
        run += 1;
    }
}

fn initialize_direction_one(
    planes: &mut TerrainPlanes,
    map_type: MapType,
    base: TileCursor,
    span: i32,
) {
    let mut column = base;
    let mut outer = 0_i32;
    while outer < 2 {
        let start_x = column.x();
        let mut cursor = column;
        let mut run = 0_i32;
        while run < span {
            let mut neighbor = TileCursor::from_xy(start_x, cursor.y());
            neighbor.add_y(1);
            let index = cursor.index();
            let difference = (i16::from(planes.ground_heights[index])
                - i16::from(planes.ground_heights[neighbor.index()]))
            .abs();
            if planes.terrain_types[index] != TERRAIN_STRIP_TYPE || difference > 30 {
                planes.ground_heights[index] = planes.ground_heights[index].wrapping_add(48);
            }
            cursor.add_y(1);
            run += 1;
        }
        column.add_word(-1);
        outer += 1;
    }

    let mut column = base;
    let mut width = 0_i32;
    while width < 3 {
        let mut run = -1_i32;
        let mut cursor = column.word_add(-256);
        while run < span {
            let index = cursor.index();
            planes.terrain_types[index] = TERRAIN_STRIP_TYPE;
            planes.angles[index] = 1;
            cursor.add_y(1);
            run += 1;
        }
        column.add_word(-1);
        width += 1;
    }

    let mut column = base.word_add(1);
    let mut width = 0_i32;
    while width < 4 {
        let mut run = -1_i32;
        let mut cursor = column.word_add(-256);
        while run < span + 1 {
            if map_type == MapType::Cave {
                cave_adjust(planes, cursor);
            } else {
                clear_cave_angle(planes, cursor);
            }
            cursor.add_y(1);
            run += 1;
        }
        if map_type == MapType::Cave {
            column.add_x(-1);
        } else {
            column.add_word(-1);
        }
        width += 1;
    }

    let mut run = -1_i32;
    let mut cursor = base.word_add(-256);
    while run < span {
        cursor.add_x(1);
        cursor.add_y(1);
        let right = cursor;
        cursor.add_x(-2);
        cursor.add_y(-2);
        let left = cursor;
        cursor.add_x(1);
        cursor.add_y(1);
        write_shading(planes, cursor, left, right, map_type);
        cursor.add_y(1);
        run += 1;
    }

    let mut column = base.word_add(-3);
    let mut width = -3_i32;
    while width < 6 {
        let mut run = -3_i32;
        let mut cursor = column.word_add(-768);
        while run < span + 3 {
            mark_dirty(planes, cursor);
            cursor.add_y(1);
            run += 1;
        }
        column.add_word(1);
        width += 1;
    }
}

fn open_strip(
    planes: &mut TerrainPlanes,
    map_type: MapType,
    position: Position,
    controller: &mut TerrainStripControllerState,
) -> TerrainStripVisit {
    if controller.subspell_index >= 48 {
        controller.life = 3;
        return TerrainStripVisit {
            audio: Some(TerrainStripAudioOutcome::EndLoop47),
            terrain_mutated: false,
        };
    }

    let cursor = working_tile(position, controller.direction);
    let mut mutated = false;
    if controller.subspell_index == 0 {
        mutated |= open_first_pass_topology(
            planes,
            map_type,
            cursor,
            controller.direction,
            controller.span,
        );
    }
    controller.subspell_index = controller.subspell_index.wrapping_add(1);
    mutated |= raise_opening_strip(
        planes,
        map_type,
        cursor,
        controller.direction,
        controller.span,
    );
    if controller.subspell_index >= 48 {
        mutated |= finalize_opening_shading(
            planes,
            map_type,
            cursor,
            controller.direction,
            controller.span,
        );
    }
    TerrainStripVisit {
        audio: Some(TerrainStripAudioOutcome::Prepare47),
        terrain_mutated: mutated,
    }
}

fn open_first_pass_topology(
    planes: &mut TerrainPlanes,
    map_type: MapType,
    cursor: TileCursor,
    direction: i8,
    span: i32,
) -> bool {
    if direction == 0 {
        let mut run = 2_i32;
        let mut column = cursor.word_add(2);
        while run < span - 3 {
            let mut across = column;
            let mut width = 0_i32;
            while width < 3 {
                let index = across.index();
                planes.terrain_types[index] = TERRAIN_STRIP_TYPE;
                planes.angles[index] = 1;
                across.add_y(-1);
                width += 1;
            }
            column.add_word(1);
            run += 1;
        }
        if map_type != MapType::Cave {
            let mut run = 2_i32;
            let mut column = cursor.word_add(2);
            while run < span - 2 {
                let mut across = column.word_add(256);
                let mut width = 0_i32;
                while width < 4 {
                    clear_cave_angle(planes, across);
                    across.add_y(-1);
                    width += 1;
                }
                column.add_word(1);
                run += 1;
            }
        }
        let mut run = -3_i32;
        let mut column = cursor.word_add(-3);
        while run < span + 3 {
            let mut width = -3_i32;
            let mut across = column.word_add(768);
            while width < 6 {
                mark_dirty(planes, across);
                across.add_y(-1);
                width += 1;
            }
            column.add_word(1);
            run += 1;
        }
        true
    } else if direction == 1 {
        let mut column = cursor;
        let mut width = 0_i32;
        while width < 3 {
            let mut run = 2_i32;
            let mut along = column.word_add(512);
            while run < span - 3 {
                let index = along.index();
                planes.terrain_types[index] = TERRAIN_STRIP_TYPE;
                planes.angles[index] = 1;
                along.add_y(1);
                run += 1;
            }
            column.add_word(-1);
            width += 1;
        }
        if map_type != MapType::Cave {
            let mut column = cursor.word_add(1);
            let mut width = 0_i32;
            while width < 4 {
                let mut run = 2_i32;
                let mut along = column.word_add(512);
                while run < span - 2 {
                    clear_cave_angle(planes, along);
                    along.add_y(1);
                    run += 1;
                }
                column.add_word(-1);
                width += 1;
            }
        }
        let mut column = cursor.word_add(-3);
        let mut width = -3_i32;
        while width < 6 {
            let mut run = -3_i32;
            let mut along = column.word_add(-768);
            while run < span + 3 {
                mark_dirty(planes, along);
                along.add_y(1);
                run += 1;
            }
            column.add_word(1);
            width += 1;
        }
        true
    } else {
        false
    }
}

fn raise_opening_strip(
    planes: &mut TerrainPlanes,
    map_type: MapType,
    cursor: TileCursor,
    direction: i8,
    span: i32,
) -> bool {
    if direction == 0 {
        let mut mutated = false;
        let mut run = 3_i32;
        let mut along = cursor.word_add(3);
        while run < span - 3 {
            let first = along.index();
            planes.ground_heights[first] = planes.ground_heights[first].wrapping_add(1);
            along.add_y(-1);
            let second = along.index();
            planes.ground_heights[second] = planes.ground_heights[second].wrapping_add(1);
            along.add_y(1);
            along.add_word(1);
            run += 1;
            mutated = true;
        }
        if map_type == MapType::Cave {
            let mut run = 3_i32;
            let mut along = cursor.word_add(3);
            while run < span - 3 {
                cave_adjust(planes, along);
                along.add_y(-1);
                cave_adjust(planes, along);
                along.add_y(1);
                along.add_word(1);
                run += 1;
                mutated = true;
            }
        }
        mutated
    } else if direction == 1 {
        let mut mutated = false;
        let mut run = 3_i32;
        let mut along = cursor;
        along.add_y(3);
        while run < span - 3 {
            let first = along.index();
            planes.ground_heights[first] = planes.ground_heights[first].wrapping_add(1);
            let second_cursor = along.word_add(-1);
            let second = second_cursor.index();
            planes.ground_heights[second] = planes.ground_heights[second].wrapping_add(1);
            along = second_cursor.word_add(1);
            along.add_y(1);
            run += 1;
            mutated = true;
        }
        if map_type == MapType::Cave {
            let mut run = 3_i32;
            let mut along = cursor;
            along.add_y(3);
            while run < span - 3 {
                cave_adjust(planes, along);
                let second = along.word_add(-1);
                cave_adjust(planes, second);
                along = second.word_add(1);
                along.add_y(1);
                run += 1;
                mutated = true;
            }
        }
        mutated
    } else {
        false
    }
}

fn finalize_opening_shading(
    planes: &mut TerrainPlanes,
    map_type: MapType,
    cursor: TileCursor,
    direction: i8,
    span: i32,
) -> bool {
    if direction == 0 {
        let mut mutated = false;
        let mut run = -1_i32;
        let mut current = cursor.word_add(-1);
        while run < span {
            current.add_x(1);
            current.add_y(1);
            let right = current;
            current.add_x(-2);
            current.add_y(-2);
            let left = current;
            current.add_x(1);
            let mut destination = current;
            destination.add_y(1);
            write_shading(planes, destination, left, right, map_type);
            current = destination.word_add(1);
            run += 1;
            mutated = true;
        }
        mutated
    } else if direction == 1 {
        let mut current = cursor;
        let mut run = 0_i32;
        while run < 3 {
            current.add_x(1);
            current.add_y(1);
            let right = current;
            current.add_x(-2);
            current.add_y(-2);
            let left = current;
            current.add_x(1);
            let mut destination = current;
            destination.add_y(1);
            write_shading(planes, destination, left, right, map_type);
            current = destination.word_add(1);
            run += 1;
        }
        true
    } else {
        false
    }
}

fn close_strip(
    planes: &mut TerrainPlanes,
    map_type: MapType,
    position: Position,
    controller: &mut TerrainStripControllerState,
) -> TerrainStripVisit {
    if controller.subspell_index == 0 {
        controller.life = 4;
        return TerrainStripVisit {
            audio: Some(TerrainStripAudioOutcome::EndLoop47),
            terrain_mutated: false,
        };
    }

    let cursor = working_tile(position, controller.direction);
    let mut mutated = lower_closing_strip(planes, cursor, controller.direction, controller.span);
    controller.subspell_index = controller.subspell_index.wrapping_sub(1);
    if controller.subspell_index == 0 {
        mutated |= finalize_closed_strip(
            planes,
            map_type,
            cursor,
            controller.direction,
            controller.span,
        );
    }
    TerrainStripVisit {
        audio: Some(TerrainStripAudioOutcome::Prepare47),
        terrain_mutated: mutated,
    }
}

fn lower_closing_strip(
    planes: &mut TerrainPlanes,
    cursor: TileCursor,
    direction: i8,
    span: i32,
) -> bool {
    if direction == 0 {
        let mut mutated = false;
        let mut run = 3_i32;
        let mut along = cursor.word_add(3);
        while run < span - 3 {
            let mut neighbor = along.word_add(256);
            neighbor.set_y(along.y().wrapping_sub(2));
            let target = (u16::from(planes.ground_heights[neighbor.index()])
                + u16::from(planes.ground_heights[along.word_add(256).index()]))
                >> 1;
            let first = along.index();
            if target < u16::from(planes.ground_heights[first]) {
                planes.ground_heights[first] = planes.ground_heights[first].wrapping_sub(1);
                mutated = true;
            }
            along.add_y(-1);
            let second = along.index();
            if target < u16::from(planes.ground_heights[second]) {
                planes.ground_heights[second] = planes.ground_heights[second].wrapping_sub(1);
                mutated = true;
            }
            along.add_y(1);
            along.add_word(1);
            run += 1;
        }
        mutated
    } else if direction == 1 {
        let mut mutated = false;
        let mut run = 3_i32;
        let mut along = cursor;
        along.add_y(3);
        while run < span - 3 {
            let target = (u16::from(planes.ground_heights[along.word_add(-2).index()])
                + u16::from(planes.ground_heights[along.word_add(1).index()]))
                >> 1;
            let first = along.index();
            if target < u16::from(planes.ground_heights[first]) {
                planes.ground_heights[first] = planes.ground_heights[first].wrapping_sub(1);
                mutated = true;
            }
            let second_cursor = along.word_add(-1);
            let second = second_cursor.index();
            if target < u16::from(planes.ground_heights[second]) {
                planes.ground_heights[second] = planes.ground_heights[second].wrapping_sub(1);
                mutated = true;
            }
            along = second_cursor.word_add(1);
            along.add_y(1);
            run += 1;
        }
        mutated
    } else {
        false
    }
}

fn copy_closed_material(planes: &mut TerrainPlanes, destination: TileCursor, source: TileCursor) {
    let destination = destination.index();
    let source = source.index();
    planes.terrain_types[destination] = planes.terrain_types[source];
    planes.angles[destination] = planes.angles[source];
    planes.shading[destination] = 32;
    planes.angles[destination] |= 0x80;
}

fn finalize_closed_strip(
    planes: &mut TerrainPlanes,
    map_type: MapType,
    cursor: TileCursor,
    direction: i8,
    span: i32,
) -> bool {
    if direction == 0 {
        finalize_closed_direction_zero(planes, map_type, cursor, span);
        true
    } else if direction == 1 {
        finalize_closed_direction_one(planes, map_type, cursor, span);
        true
    } else {
        false
    }
}

fn finalize_closed_direction_zero(
    planes: &mut TerrainPlanes,
    map_type: MapType,
    cursor: TileCursor,
    span: i32,
) {
    let mut run = 3_i32;
    let mut destination = cursor.word_add(3);
    while run < span - 4 {
        let mut source = destination.word_add(512);
        if planes.terrain_types[source.index()] == TERRAIN_STRIP_TYPE {
            source.add_word(span >> 1);
        }
        destination.add_y(1);
        copy_closed_material(planes, destination, source);
        destination.add_y(-1);
        copy_closed_material(planes, destination, source);
        destination.add_y(-1);
        copy_closed_material(planes, destination, source);
        destination.add_y(-1);
        copy_closed_material(planes, destination, source);
        destination.add_y(2);
        destination.add_word(1);
        run += 1;
    }

    if map_type == MapType::Cave {
        let mut run = 3_i32;
        let mut along = cursor.word_add(3);
        while run < span - 4 {
            along.add_y(1);
            cave_adjust(planes, along);
            along.add_y(-1);
            cave_adjust(planes, along);
            along.add_y(-1);
            cave_adjust(planes, along);
            along.add_y(-1);
            cave_adjust(planes, along);
            along.add_y(2);
            along.add_word(1);
            run += 1;
        }
    } else {
        clear_cave_angle(planes, cursor.word_add(259));
        let mut fixed = cursor.word_add(259);
        fixed.set_y(cursor.word_add(3).y());
        clear_cave_angle(planes, fixed);
        fixed.add_y(-1);
        clear_cave_angle(planes, fixed);
        fixed.add_y(-1);
        clear_cave_angle(planes, fixed);
    }

    let mut run = -3_i32;
    let mut column = cursor.word_add(-3);
    while run < span + 3 {
        let mut width = -3_i32;
        let mut across = column.word_add(768);
        while width < 6 {
            mark_dirty(planes, across);
            across.add_y(-1);
            width += 1;
        }
        column.add_word(1);
        run += 1;
    }
}

fn finalize_closed_direction_one(
    planes: &mut TerrainPlanes,
    map_type: MapType,
    cursor: TileCursor,
    span: i32,
) {
    let mut run = 3_i32;
    let mut destination = cursor;
    destination.add_y(3);
    while run < span - 4 {
        let mut source = destination.word_add(2);
        if planes.terrain_types[source.index()] == TERRAIN_STRIP_TYPE {
            source.add_y(span >> 1);
        }
        let first = destination.word_add(1);
        copy_closed_material(planes, first, source);
        let second = destination;
        copy_closed_material(planes, second, source);
        let third = destination.word_add(-1);
        copy_closed_material(planes, third, source);
        let fourth = destination.word_add(-2);
        copy_closed_material(planes, fourth, source);
        destination = fourth.word_add(2);
        destination.add_y(1);
        run += 1;
    }

    if map_type == MapType::Cave {
        let mut run = 3_i32;
        let mut along = cursor;
        along.add_y(3);
        while run < span - 4 {
            let first = along.word_add(1);
            cave_adjust(planes, first);
            let second = first.word_add(-1);
            cave_adjust(planes, second);
            let third = second.word_add(-1);
            cave_adjust(planes, third);
            let fourth = third.word_add(-1);
            cave_adjust(planes, fourth);
            along = fourth.word_add(2);
            along.add_y(1);
            run += 1;
        }
    } else {
        let fixed = cursor.word_add(768);
        clear_cave_angle(planes, fixed.word_add(1));
        clear_cave_angle(planes, fixed);
        clear_cave_angle(planes, cursor.word_add(767));
        clear_cave_angle(planes, cursor.word_add(766));
    }

    let mut width = -3_i32;
    let mut column = cursor.word_add(-3);
    while width < 6 {
        let mut run = -3_i32;
        let mut along = column.word_add(-768);
        while run < span + 3 {
            mark_dirty(planes, along);
            along.add_y(1);
            run += 1;
        }
        column.add_y(1);
        width += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corridor_properties(
        orientation: i16,
        extent: i16,
    ) -> crate::level_definition::CaveCorridorDefinition {
        use crate::level_definition::CorridorOrientation;
        sim_definition::adapter::cave_corridor_definition(
            if orientation != 0 {
                CorridorOrientation::AlongX
            } else {
                CorridorOrientation::AlongY
            },
            extent,
        )
    }

    #[test]
    fn mission_four_corridor_uses_source_scheduler_audio_epilogue_and_reset() {
        use crate::{FlightEvent, native_world::NativeWorld};
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let package = root.join("mc-iol7n-campaign-census-b-final/level4");
        let rows = crate::native_world::parse_entities(
            &std::fs::read(package.join("entities.bin")).unwrap(),
        )
        .unwrap();
        let planes = crate::native_world::parse_terrain(
            &std::fs::read(package.join("terrain.bin")).unwrap(),
        )
        .unwrap();
        let owner_row = &rows[195];
        let imported =
            crate::campaign::mc2_level_definition::Mc2LevelDefinition::from_authored_bytes(
                &std::fs::read(package.join("authored-level.bin")).unwrap(),
                &[],
                crate::campaign::spec::CampaignSelection::ordinary(4).unwrap(),
                None,
            )
            .expect("actual mission4 typed corridor definition");
        let properties = imported
            .semantic
            .entities()
            .iter()
            .find(|entity| {
                entity.cave_corridor().is_some()
                    && entity.position().x == owner_row.position.x >> 8
                    && entity.position().y == owner_row.position.y >> 8
            })
            .unwrap()
            .cave_corridor()
            .unwrap();
        // Reuse the existing owner-backed source scheduler/arena, substituting
        // only this family's real Cave terrain and imported construction data.
        // This is a family execution check, not whole-mission-4 admission/play.
        let mut session = NativeWorld::inspect_level2_with_owner_cache(
            root.join("l2-debug3.fJ3PaM/level2"),
            root.join("level2-day-owner-cache/components/gameplay"),
        )
        .expect("actual owner source arena")
        .level2_reset_session()
        .unwrap();
        let baseline = session.session.state.live.clone();
        let live = &mut session.session.state.live;
        live.terrain = SessionTerrain {
            map_type: MapType::Cave,
            planes,
            terrain_rng: 123,
            mutation_sequence: 0,
        };
        let slot = cave_corridors::construct(
            &mut live.economy,
            &mut live.terrain,
            owner_row.position,
            properties,
        )
        .unwrap();
        assert_eq!(
            session.cf10_live_source_phase(slot),
            Some(slot as u8),
            "ordinary registry resolves the constructor's action7"
        );
        session.step_level2_source_pass(0);
        let source = &mut session.session.state.live.economy.entities[usize::from(slot)];
        assert_eq!(
            (source.life, source.position.z, source.phase),
            (4, owner_row.position.z, (slot as u8).wrapping_add(1))
        );
        // This is the actual source reset command for this authored controller.
        source.set_life(owner_row.life);
        let cursor = session.effect_journal().records().len();
        for pass in 1..=255 {
            session.step_level2_source_pass(pass);
            if session.session.state.live.economy.entities[usize::from(slot)].life == 3 {
                break;
            }
        }
        assert_eq!(
            session.session.state.live.economy.entities[usize::from(slot)].life,
            3
        );
        let sounds = session.effect_journal().records()[cursor..]
            .iter()
            .filter_map(|record| match record.effect {
                crate::campaign::journal::CampaignEffect::Flight(FlightEvent::PrepareSound {
                    entity_index,
                    sound_id: 47,
                    ..
                }) if entity_index == slot => Some("prepare"),
                crate::campaign::journal::CampaignEffect::Flight(FlightEvent::EndLoop {
                    entity_index,
                    sound_id: 47,
                    ..
                }) if entity_index == slot => Some("end"),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(sounds.len() > 1);
        assert_eq!(sounds.last(), Some(&"end"));
        assert_eq!(
            sounds[sounds.len() - 2],
            "prepare",
            "both terminal audio calls survive journal publication"
        );
        // Reach the existing authored close-command producer. In a cave it
        // selects life2, opening the two planes, via the real map-chain lookup.
        let live = &mut session.session.state.live;
        let mut trigger_position = owner_row.position;
        trigger_position.x = trigger_position.x.wrapping_add(128);
        trigger_position.y = trigger_position.y.wrapping_add(128);
        let trigger =
            allocate_close_trigger(&mut live.economy, &mut live.terrain, 0, trigger_position)
                .unwrap();
        assert_eq!(trigger.control_flags & 4, 4);
        let trigger_tile =
            usize::from(trigger_position.y >> 8) * 256 + usize::from(trigger_position.x >> 8);
        assert_eq!(
            live.terrain.planes.entity_indices[trigger_tile],
            live.economy
                .economy
                .map_chains
                .tile_head(trigger_tile as u16) as i16
        );
        for pass in 0..=255 {
            session.step_level2_source_pass(pass);
            if session.session.state.live.economy.entities[usize::from(slot)].life == 4 {
                break;
            }
        }
        assert_eq!(
            session.session.state.live.economy.entities[usize::from(slot)].life,
            4
        );
        assert!(
            !session
                .session
                .state
                .live
                .economy
                .economy
                .terrain_strip_close_triggers()
                .iter()
                .any(|row| row.active && row.source_entity_index == trigger.source_entity_index)
        );
        let live = &session.session.state.live;
        assert!(
            !live
                .economy
                .economy
                .terrain_strip_close_triggers()
                .iter()
                .any(|row| row.source_entity_index == trigger.source_entity_index),
            "the following source pass reclaims the retired command"
        );
        assert_eq!(
            live.terrain.planes.entity_indices[trigger_tile],
            live.economy
                .economy
                .map_chains
                .tile_head(trigger_tile as u16) as i16,
            "the retired command no longer leaves a stale terrain map head"
        );
        // The source reclaims a corridor synchronously when no closed bound
        // exists. Its numeric row still receives this pass's common epilogue.
        let live = &mut session.session.state.live;
        let source = &mut live.economy.entities[usize::from(slot)];
        let old_phase = source.phase;
        let horizontal = source.source_word_2c() != 0;
        let tile_x = usize::from(source.position.x >> 8);
        let tile_y = usize::from(source.position.y >> 8);
        source.set_life(0);
        for coordinate in 0..256 {
            let index = if horizontal {
                tile_y * 256 + coordinate
            } else {
                coordinate * 256 + tile_x
            };
            live.terrain.planes.angles[index] &= !8;
        }
        session.step_level2_source_pass(0);
        let live = &session.session.state.live;
        let source = &live.economy.entities[usize::from(slot)];
        assert!(
            !source.active && source.class == 0,
            "invalid bound reclaims in the callback"
        );
        assert_eq!(
            source.phase,
            old_phase.wrapping_add(1),
            "same-pass epilogue advances retained row"
        );
        assert_eq!(source.record_bytes()[66], source.phase);
        let tile = tile_y * 256 + tile_x;
        assert_eq!(
            live.terrain.planes.entity_indices[tile],
            live.economy.economy.map_chains.tile_head(tile as u16) as i16,
            "reclaim immediately publishes the surviving map head"
        );
        assert!(!session.dispatch_events().iter().any(|event| matches!(event,
            crate::native_world::Mc2DispatchEvent::Error { source_slot, .. } if *source_slot == slot)));
        session.reset();
        assert!(
            session.session.state.live == baseline,
            "reset restores the source-scheduled corridor and both terrain planes"
        );
    }

    #[test]
    fn cave_constructor_initializes_wrapped_bounds_and_reclaims_failure_immediately() {
        use crate::native_world::NativeWorld;
        let package = std::env::var_os("MC_LEVEL2_PACKAGE_V5").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/l2-debug3.fJ3PaM/level2")
            },
            std::path::PathBuf::from,
        );
        let cache = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let mut session = NativeWorld::inspect_level2_with_owner_cache(&package, &cache)
            .expect("actual owner source arena")
            .level2_reset_session()
            .expect("source session");
        let baseline = session.session.state.live.clone();
        for (orientation, extent) in [(0, 0), (1, 0), (0, 2), (1, 2)] {
            let live = &mut session.session.state.live;
            live.terrain = terrain(MapType::Cave);
            live.terrain.planes.angles.fill(0);
            let (a, b) = if orientation == 0 {
                (TileCursor::from_xy(0, 255), TileCursor::from_xy(0, 1))
            } else {
                (TileCursor::from_xy(255, 0), TileCursor::from_xy(1, 0))
            };
            live.terrain.planes.angles[a.index()] = 8;
            live.terrain.planes.angles[b.index()] = 8;
            live.terrain.planes.ground_heights[a.index()] = 40;
            live.terrain.planes.ground_heights[b.index()] = 60;
            let position = Position {
                x: 0,
                y: 0,
                z: 1234,
            };
            let slot = cave_corridors::construct(
                &mut live.economy,
                &mut live.terrain,
                position,
                corridor_properties(orientation, extent),
            )
            .expect("Cave constructor uses the shared NewEvent arena");
            assert_eq!(
                live.terrain.planes.entity_indices[0], slot as i16,
                "AddEventToMap publishes the live controller in the terrain head plane"
            );
            let row = &live.economy.entities[usize::from(slot)];
            assert_eq!(
                (
                    row.class,
                    row.model,
                    row.action,
                    row.max_life,
                    row.life,
                    row.control_flags
                ),
                (14, 2, 7, 300, 0, 5)
            );
            assert_eq!(
                (
                    row.subspell_index,
                    row.source_word_2c(),
                    row.source_word_96(),
                    row.actual_speed
                ),
                (0, orientation, extent as u16, 16)
            );
            assert_eq!(row.particle_parameter_index, Some(0));
            let phase = row.phase;
            let visit = cave_corridors::visit(&mut live.economy, &mut live.terrain, slot);
            assert_eq!(visit, cave_corridors::Visit::default());
            let row = &live.economy.entities[usize::from(slot)];
            assert_eq!(
                row.phase, phase,
                "direct initializer has no Update epilogue"
            );
            assert_eq!((row.life, row.position.z), (4, 50));
            assert_ne!(live.terrain.planes.angles[0] & 0x80, 0);
            live.economy.entities[usize::from(slot)].set_life(1);
            for _ in 0..64 {
                if cave_corridors::visit(&mut live.economy, &mut live.terrain, slot).end_loop {
                    break;
                }
            }
            assert_eq!(live.economy.entities[usize::from(slot)].life, 3);
            assert_eq!(
                (
                    live.terrain.planes.ground_heights[0],
                    live.terrain.planes.secondary_heights[0]
                ),
                (50, 50)
            );
            live.economy.entities[usize::from(slot)].set_life(2);
            for _ in 0..64 {
                if cave_corridors::visit(&mut live.economy, &mut live.terrain, slot).end_loop {
                    break;
                }
            }
            assert_eq!(live.economy.entities[usize::from(slot)].life, 4);
            assert_eq!(
                (
                    live.terrain.planes.ground_heights[0],
                    live.terrain.planes.secondary_heights[0]
                ),
                (20, 80)
            );
            // Removal does not reopen a corridor or write either height plane.
            live.economy.entities[usize::from(slot)].set_life(0);
            live.terrain.planes.angles.fill(0);
            let before_failure = live.terrain.clone();
            let neighbor = cave_corridors::construct(
                &mut live.economy,
                &mut live.terrain,
                position,
                corridor_properties(orientation, extent),
            )
            .unwrap();
            assert_eq!(live.terrain.planes.entity_indices[0], neighbor as i16);
            let failed = cave_corridors::visit(&mut live.economy, &mut live.terrain, slot);
            assert!(failed.reclaim);
            assert!(!live.economy.entities[usize::from(slot)].active);
            assert_eq!(live.economy.entities[usize::from(slot)].class, 0);
            assert_eq!(live.economy.entities[usize::from(slot)].control_flags, 1);
            assert_eq!(
                live.economy.entities[usize::from(neighbor)].old_map_entity(),
                live.economy.economy.map_chains.old_map_entity(neighbor)
            );
            assert_eq!(
                live.economy.entities[usize::from(neighbor)].next_entity(),
                live.economy.economy.map_chains.next_entity(neighbor)
            );
            assert_eq!(live.economy.economy.free_slots.last(), Some(&slot));
            assert!(
                !live
                    .economy
                    .economy
                    .map_chains
                    .entity_chain(0, 0)
                    .contains(&slot)
            );
            let mut expected_after_failure = before_failure;
            expected_after_failure.planes.entity_indices[0] = neighbor as i16;
            assert!(
                live.terrain == expected_after_failure,
                "failed initialization retains geometry and publishes the surviving map head"
            );
            let generation = live.economy.economy.slot_generation(slot);
            let reused = cave_corridors::construct(
                &mut live.economy,
                &mut live.terrain,
                position,
                corridor_properties(orientation, extent),
            )
            .expect("the just-reclaimed row is available to NewEvent");
            assert_eq!(
                reused, slot,
                "immediate reclaim returns the row to the live free stack"
            );
            assert_ne!(live.economy.economy.slot_generation(reused), generation);
            assert!(cave_corridors::visit(&mut live.economy, &mut live.terrain, reused).reclaim);
            assert!(cave_corridors::visit(&mut live.economy, &mut live.terrain, neighbor).reclaim);
            assert_eq!(
                live.terrain.planes.entity_indices[0],
                live.economy.economy.map_chains.tile_head(0) as i16,
                "immediate reclaim publishes the final map head"
            );
        }
        session.reset();
        assert!(
            session.session.state.live == baseline,
            "reset restores terrain and authoritative controller/allocator state together"
        );
        let live = &mut session.session.state.live;
        let arena_before = live.economy.clone();
        assert_eq!(
            cave_corridors::construct(
                &mut live.economy,
                &mut live.terrain,
                position(5, 5),
                corridor_properties(0, 0)
            ),
            None
        );
        assert_eq!(
            live.economy, arena_before,
            "outdoor constructor is a source no-op"
        );
    }

    #[test]
    fn mission_four_corridors_change_both_planes_and_live_flight_collision() {
        use crate::FlightWorld;
        let package = std::env::var_os("MC_CAVE_CORRIDOR_OWNER_PACKAGE").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-iol7n-campaign-census-b-final/level4")
            },
            std::path::PathBuf::from,
        );
        let entities = crate::native_world::parse_entities(
            &std::fs::read(package.join("entities.bin")).expect("actual mission4 owner rows"),
        )
        .expect("decode actual mission4 rows");
        let planes = crate::native_world::parse_terrain(
            &std::fs::read(package.join("terrain.bin")).expect("actual mission4 dual terrain"),
        )
        .expect("decode actual mission4 terrain");
        let imported =
            crate::campaign::mc2_level_definition::Mc2LevelDefinition::from_authored_bytes(
                &std::fs::read(package.join("authored-level.bin"))
                    .expect("actual mission4 authored content"),
                &[],
                crate::campaign::spec::CampaignSelection::ordinary(4).unwrap(),
                None,
            )
            .expect("ordinary MC2 adapter binds actual corridor definitions");
        let rows = entities
            .iter()
            .filter(|row| row.active && (row.class, row.model, row.action) == (14, 2, 7));
        let mut directions = [0; 2];
        for row in rows {
            let mut source = row.clone();
            let position = crate::level_definition::TilePosition {
                x: source.position.x >> 8,
                y: source.position.y >> 8,
            };
            let id = imported
                .initial_mechanic_entity(
                    crate::level_definition::MechanicKind::CaveCorridor,
                    position,
                )
                .expect("each initial controller resolves among six same-kind mechanics");
            let definition = imported
                .semantic
                .entity(id)
                .expect("live controller has a typed authored corridor binding");
            assert_eq!(
                definition.cave_corridor(),
                Some(corridor_properties(
                    source.source_word_2c(),
                    source.source_word_96() as i16
                ))
            );
            cave_corridors::bind_properties(&mut source, definition.cave_corridor().unwrap());
            directions[usize::from(source.source_word_2c() != 0)] += 1;
            let mut terrain = SessionTerrain {
                map_type: MapType::Cave,
                planes: planes.clone(),
                terrain_rng: 123,
                mutation_sequence: 0,
            };
            let position = source.position;
            let rng = source.entity_rng;
            let floor_before = terrain.ground_height(position);
            let ceiling_before = terrain.ceiling_height(position).unwrap();
            assert_eq!(source.life, 1, "real mission constructor starts sealing");
            for _ in 0..256 {
                let visit = cave_corridors::step(&mut terrain, &mut source);
                assert!(!visit.reclaim);
                if visit.end_loop {
                    break;
                }
            }
            assert_eq!(source.life, 3, "corridor {} seals", source.index);
            assert!(terrain.cave_collision_blocked(position));
            assert!(terrain.ground_height(position) > floor_before);
            assert!(terrain.ceiling_height(position).unwrap() < ceiling_before);
            let mut probe = position;
            probe.z = (floor_before + ceiling_before) / 2;
            assert_ne!(
                crate::resolve_legacy_movement(
                    &mut crate::FlightState::default(),
                    probe,
                    probe,
                    crate::FlightProfile::CAVE,
                    &terrain
                ),
                crate::MovementResolution::Accept(probe),
                "ordinary flight cannot continue straight through the sealed cell",
            );
            let closed = terrain.clone();
            source.set_life(2);
            for _ in 0..256 {
                let visit = cave_corridors::step(&mut terrain, &mut source);
                if visit.end_loop {
                    assert!(visit.prepare_sound);
                    break;
                }
            }
            assert_eq!(source.life, 4, "corridor {} opens", source.index);
            assert!(!terrain.cave_collision_blocked(position));
            probe.z =
                (terrain.ground_height(position) + terrain.ceiling_height(position).unwrap()) / 2;
            assert_eq!(
                crate::resolve_legacy_movement(
                    &mut crate::FlightState::default(),
                    probe,
                    probe,
                    crate::FlightProfile::CAVE,
                    &terrain
                ),
                crate::MovementResolution::Accept(probe),
                "ordinary flight accepts the reopened corridor",
            );
            assert!(terrain.ground_height(position) < closed.ground_height(position));
            assert!(
                terrain.ceiling_height(position).unwrap()
                    > closed.ceiling_height(position).unwrap()
            );
            assert_eq!((terrain.terrain_rng, source.entity_rng), (123, rng));
            let settled = terrain.clone();
            assert_eq!(
                cave_corridors::step(&mut terrain, &mut source),
                cave_corridors::Visit::default()
            );
            assert!(terrain == settled, "settled callback retains geometry");
        }
        assert_eq!(
            directions,
            [3, 3],
            "all six actual mission4 corridor roots exercised"
        );
        let corridor = imported
            .semantic
            .entities()
            .iter()
            .find(|entity| entity.cave_corridor().is_some())
            .unwrap();
        let mut ambiguous = imported.clone();
        let mut definitions = ambiguous.semantic.entities().to_vec();
        definitions.push(sim_definition::adapter::cave_corridor_entity_definition(
            sim_definition::adapter::stable_entity_id("mechanic.cave_corridor.duplicate").unwrap(),
            corridor.default_position(),
            None,
            crate::level_definition::LevelEntityPositionAccess::ReadOnly,
            corridor.cave_corridor().unwrap(),
        ));
        ambiguous.semantic = sim_definition::adapter::build_level_definition(
            definitions,
            ambiguous.semantic.objectives().to_vec(),
            ambiguous.semantic.activation_groups().collect(),
        )
        .unwrap();
        assert!(
            ambiguous
                .initial_mechanic_entity(
                    crate::level_definition::MechanicKind::CaveCorridor,
                    corridor.default_position(),
                )
                .unwrap_err()
                .contains("ambiguous"),
            "the join still rejects two same-kind mechanics at one initial position"
        );
        assert!(
            imported
                .initial_mechanic_entity(
                    crate::level_definition::MechanicKind::CaveCorridor,
                    crate::level_definition::TilePosition { x: 0, y: 0 },
                )
                .unwrap_err()
                .contains("no semantic mechanic"),
            "missing initial bindings remain invalid"
        );

        // The real mission session, scheduler and renderer consume the same
        // owner package. The earlier direct checks isolate all six source
        // records; this path exercises normal command turns and trigger births.
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let package = root.join("mc-bhq8-level4-collision-v2-cave");
        let cache = root.join("mc-b9s2-cursor-level4-20261029");
        let mut campaign = crate::campaign::CampaignSession::open_with_owner_cache(
            &package,
            &cache,
            crate::campaign::spec::CampaignSelection::ordinary(4).unwrap(),
        )
        .expect("actual Level4 campaign and matching Cave materials open");
        let assets = crate::day_indexed_frame::DayIndexedFrameAssets::load(&cache)
            .expect("actual package-selected Cave renderer assets");
        let initial_frame = crate::day_indexed_frame::render_level2_day_indexed_frame(
            campaign.inspection(),
            campaign.simulation(),
            &assets,
            campaign.generation(),
        )
        .expect("real Cave presentation consumes the initial source terrain");
        let roots = {
            let live = &campaign.simulation().session.state.live;
            [false, true].map(|horizontal| {
                let row = live
                    .economy
                    .entities
                    .iter()
                    .find(|row| {
                        row.active
                            && (row.class, row.model, row.action) == (14, 2, 7)
                            && row.life == 1
                            && (row.source_word_2c() != 0) == horizontal
                    })
                    .expect("real Level4 supplies a live corridor in each orientation");
                (
                    row.index,
                    row.position,
                    live.terrain.ground_height(row.position),
                    live.terrain.ceiling_height(row.position).unwrap(),
                )
            })
        };
        for _ in 0..256 {
            let _ = campaign.step(crate::PlayerCommand::default(), 0);
            assert!(
                !campaign
                    .simulation()
                    .dispatch_events()
                    .iter()
                    .any(|event| matches!(
                        event,
                        crate::native_world::Mc2DispatchEvent::Error { .. }
                    )),
                "real corridor source pass reported an error: {:?}",
                campaign.simulation().dispatch_events()
            );
            if roots.iter().all(|(slot, _, _, _)| {
                campaign.simulation().session.state.live.economy.entities[usize::from(*slot)].life
                    == 3
            }) {
                break;
            }
        }
        {
            let live = &campaign.simulation().session.state.live;
            for (slot, position, floor, ceiling) in roots {
                assert_eq!(live.economy.entities[usize::from(slot)].life, 3);
                assert!(live.terrain.ground_height(position) > floor);
                assert!(live.terrain.ceiling_height(position).unwrap() < ceiling);
                assert!(live.terrain.cave_collision_blocked(position));
                let mut probe = position;
                probe.z = (floor + ceiling) / 2;
                assert_ne!(
                    crate::resolve_legacy_movement(
                        &mut crate::FlightState::default(),
                        probe,
                        probe,
                        crate::FlightProfile::CAVE,
                        &live.terrain,
                    ),
                    crate::MovementResolution::Accept(probe),
                    "ordinary flight rejects the real closed corridor"
                );
            }
        }
        let closed_frame = crate::day_indexed_frame::render_level2_day_indexed_frame(
            campaign.inspection(),
            campaign.simulation(),
            &assets,
            campaign.generation(),
        )
        .expect("real Cave presentation consumes the closed live terrain");
        assert_eq!(closed_frame.tick, campaign.simulation().state.tick);
        assert_ne!(
            closed_frame.terrain_input_hash,
            initial_frame.terrain_input_hash
        );
        assert_eq!(closed_frame.indices.len(), initial_frame.indices.len());
        let closed_planes = campaign
            .simulation()
            .session
            .state
            .live
            .terrain
            .planes
            .clone();
        let authored = crate::level1_mission::parse_authored_features(
            &std::fs::read(package.join("authored-level.bin")).unwrap(),
        )
        .expect("actual Level4 source trigger records");
        {
            let live = &mut campaign.simulation_mut().session.state.live;
            for (_, position, _, _) in roots {
                let feature = authored
                    .iter()
                    .find(|row| {
                        row.kind == 10
                            && row.subtype == 63
                            && row.tile_x == position.x >> 8
                            && row.tile_y == position.y >> 8
                    })
                    .expect("source-authored open trigger addresses this corridor");
                let position =
                    feature.world_position(live.terrain.ground_height(feature.world_position(0)));
                allocate_close_trigger(
                    &mut live.economy,
                    &mut live.terrain,
                    feature.index,
                    position,
                )
                .expect("ordinary source constructor allocates the authored opening trigger");
            }
        }
        for _ in 0..256 {
            let _ = campaign.step(crate::PlayerCommand::default(), 0);
            assert!(
                !campaign
                    .simulation()
                    .dispatch_events()
                    .iter()
                    .any(|event| matches!(
                        event,
                        crate::native_world::Mc2DispatchEvent::Error { .. }
                    ))
            );
            if roots.iter().all(|(slot, _, _, _)| {
                campaign.simulation().session.state.live.economy.entities[usize::from(*slot)].life
                    == 4
            }) {
                break;
            }
        }
        let live = &campaign.simulation().session.state.live;
        for (slot, position, _, _) in roots {
            assert_eq!(live.economy.entities[usize::from(slot)].life, 4);
            assert!(!live.terrain.cave_collision_blocked(position));
            let mut probe = position;
            probe.z = (live.terrain.ground_height(position)
                + live.terrain.ceiling_height(position).unwrap())
                / 2;
            assert_eq!(
                crate::resolve_legacy_movement(
                    &mut crate::FlightState::default(),
                    probe,
                    probe,
                    crate::FlightProfile::CAVE,
                    &live.terrain,
                ),
                crate::MovementResolution::Accept(probe),
                "ordinary flight accepts the real reopened corridor"
            );
        }
        let opened_frame = crate::day_indexed_frame::render_level2_day_indexed_frame(
            campaign.inspection(),
            campaign.simulation(),
            &assets,
            campaign.generation(),
        )
        .expect("real Cave presentation consumes the reopened live terrain");
        assert_eq!(opened_frame.tick, campaign.simulation().state.tick);
        assert_ne!(
            opened_frame.terrain_input_hash,
            closed_frame.terrain_input_hash
        );
        // Compare the two genuinely produced corridor geometries at the same
        // tick and camera. A cross-tick input hash alone also changes with the
        // camera/turn and cannot prove a visible terrain difference.
        let opened_planes = campaign
            .simulation()
            .session
            .state
            .live
            .terrain
            .planes
            .clone();
        for (slot, position, _, _) in roots {
            let row = &campaign.simulation().session.state.live.economy.entities[usize::from(slot)];
            let horizontal = row.source_word_2c() != 0;
            let length = 2 * (i32::from(row.source_word_96() as i16) + 2);
            let (width, height) = if horizontal { (length, 2) } else { (2, length) };
            let mut cells = Vec::new();
            for dy in 0..height {
                for dx in 0..width {
                    let x = (i32::from(position.x >> 8) - (width >> 1) + dx) as u8;
                    let y = (i32::from(position.y >> 8) - (height >> 1) + dy) as u8;
                    cells.push(usize::from(y) * 256 + usize::from(x));
                }
            }
            let eye = (campaign
                .simulation()
                .session
                .state
                .live
                .terrain
                .ground_height(position)
                + campaign
                    .simulation()
                    .session
                    .state
                    .live
                    .terrain
                    .ceiling_height(position)
                    .unwrap())
                / 2;
            let mut visible = false;
            for yaw in [0, 512, 1024, 1536] {
                campaign
                    .simulation_mut()
                    .session
                    .state
                    .scratch
                    .source_camera_pose = Some(sim_session::CameraPose {
                    x: if horizontal {
                        position.x.wrapping_sub(512)
                    } else {
                        position.x
                    },
                    y: if horizontal {
                        position.y
                    } else {
                        position.y.wrapping_sub(512)
                    },
                    z: eye,
                    yaw,
                    pitch: 0,
                    roll: 0,
                });
                let opened = crate::day_indexed_frame::render_level2_day_indexed_frame(
                    campaign.inspection(),
                    campaign.simulation(),
                    &assets,
                    campaign.generation(),
                )
                .unwrap();
                {
                    let planes = &mut campaign.simulation_mut().session.state.live.terrain.planes;
                    for &cell in &cells {
                        planes.ground_heights[cell] = closed_planes.ground_heights[cell];
                        planes.secondary_heights[cell] = closed_planes.secondary_heights[cell];
                        planes.angles[cell] = closed_planes.angles[cell];
                    }
                }
                let closed = crate::day_indexed_frame::render_level2_day_indexed_frame(
                    campaign.inspection(),
                    campaign.simulation(),
                    &assets,
                    campaign.generation(),
                )
                .unwrap();
                {
                    let planes = &mut campaign.simulation_mut().session.state.live.terrain.planes;
                    for &cell in &cells {
                        planes.ground_heights[cell] = opened_planes.ground_heights[cell];
                        planes.secondary_heights[cell] = opened_planes.secondary_heights[cell];
                        planes.angles[cell] = opened_planes.angles[cell];
                    }
                }
                assert_ne!(opened.terrain_input_hash, closed.terrain_input_hash);
                if opened.indices != closed.indices {
                    visible = true;
                    break;
                }
            }
            assert!(
                visible,
                "real corridor geometry reaches visible Cave pixels in either orientation"
            );
        }
        let (slot, position, _, _) = roots[0];
        let old_phase = {
            let live = &mut campaign.simulation_mut().session.state.live;
            let row = &mut live.economy.entities[usize::from(slot)];
            let phase = row.phase;
            let horizontal = row.source_word_2c() != 0;
            row.set_life(0);
            let x = usize::from(position.x >> 8);
            let y = usize::from(position.y >> 8);
            for coordinate in 0..256 {
                let index = if horizontal {
                    y * 256 + coordinate
                } else {
                    coordinate * 256 + x
                };
                live.terrain.planes.angles[index] &= !8;
            }
            phase
        };
        let _ = campaign.step(crate::PlayerCommand::default(), 0);
        assert!(
            !campaign
                .simulation()
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. }))
        );
        let live = &campaign.simulation().session.state.live;
        let row = &live.economy.entities[usize::from(slot)];
        assert!(
            !row.active && row.class == 0,
            "real scheduler immediately reclaims invalid bounds"
        );
        assert_eq!(
            row.phase,
            old_phase.wrapping_add(1),
            "source epilogue follows immediate reclaim"
        );
        assert_eq!(row.record_bytes()[66], row.phase);
        let tile = usize::from(position.y >> 8) * 256 + usize::from(position.x >> 8);
        assert_eq!(
            live.terrain.planes.entity_indices[tile],
            live.economy.economy.map_chains.tile_head(tile as u16) as i16
        );
    }

    fn terrain(map_type: MapType) -> SessionTerrain {
        let cells = 256 * 256;
        SessionTerrain {
            map_type,
            planes: TerrainPlanes {
                terrain_types: vec![3; cells].into_boxed_slice(),
                ground_heights: vec![20; cells].into_boxed_slice(),
                shading: vec![7; cells].into_boxed_slice(),
                angles: vec![0x08; cells].into_boxed_slice(),
                secondary_heights: vec![80; cells].into_boxed_slice(),
                entity_indices: vec![0; cells].into_boxed_slice(),
            },
            terrain_rng: 0x5a5a,
            mutation_sequence: 0,
        }
    }

    fn position(x: u8, y: u8) -> Position {
        Position {
            x: (u16::from(x) << 8) | 128,
            y: (u16::from(y) << 8) | 128,
            z: 0,
        }
    }

    #[test]
    fn direction_zero_initialization_wraps_tiles_and_preserves_rng() {
        let mut terrain = terrain(MapType::Night);
        let rng = terrain.terrain_rng;
        let mut controller = TerrainStripControllerState::new(0, 8);
        let visit = terrain.apply_terrain_strip_visit(position(0, 0), &mut controller);

        assert_eq!(visit.audio, None);
        assert!(visit.terrain_mutated);
        assert_eq!(controller.life, 3);
        assert_eq!(controller.subspell_index, 48);
        assert_eq!(controller.span, 9);
        assert_eq!(terrain.terrain_rng, rng);
        assert_eq!(terrain.mutation_sequence, 1);
        assert_eq!(
            terrain.planes.terrain_types[TileCursor::from_xy(255, 0).index()],
            8
        );
        assert!(terrain.planes.angles[TileCursor::from_xy(253, 0).index()] & 0x80 != 0);
    }

    #[test]
    fn direction_one_cave_initialization_updates_secondary_plane() {
        let mut terrain = terrain(MapType::Cave);
        terrain.planes.secondary_heights.fill(0);
        let mut controller = TerrainStripControllerState::new(1, 8);
        let visit = terrain.apply_terrain_strip_visit(position(40, 40), &mut controller);

        assert!(visit.terrain_mutated);
        assert_eq!(controller.life, 3);
        let adjusted = (0..65_536)
            .filter(|index| terrain.planes.angles[*index] & 0x08 != 0)
            .count();
        assert!(adjusted > 0);
        assert!(
            terrain
                .planes
                .secondary_heights
                .iter()
                .any(|height| *height != 0)
        );
    }

    #[test]
    fn direction_zero_opening_tick_changes_only_the_two_source_rows() {
        let mut terrain = terrain(MapType::Day);
        let mut controller = TerrainStripControllerState {
            life: 1,
            subspell_index: 1,
            direction: 0,
            span: 12,
        };
        let visit = terrain.apply_terrain_strip_visit(position(80, 80), &mut controller);

        let changed = terrain
            .planes
            .ground_heights
            .iter()
            .enumerate()
            .filter_map(|(index, height)| (*height != 20).then_some(index))
            .collect::<Vec<_>>();
        let mut expected = Vec::new();
        for x in 83..=88 {
            expected.push(TileCursor::from_xy(x, 80).index());
            expected.push(TileCursor::from_xy(x, 81).index());
        }
        expected.sort_unstable();

        assert_eq!(visit.audio, Some(TerrainStripAudioOutcome::Prepare47));
        assert_eq!(
            visit.audio.map(TerrainStripAudioOutcome::sound_id),
            Some(47)
        );
        assert_eq!(changed, expected);
        assert!(
            changed
                .iter()
                .all(|index| terrain.planes.ground_heights[*index] == 21)
        );
        assert_eq!(controller.subspell_index, 2);
    }

    #[test]
    fn day_and_night_initialization_use_complementary_source_shading() {
        let mut day = terrain(MapType::Day);
        let mut night = terrain(MapType::Night);
        let mut day_controller = TerrainStripControllerState::new(0, 8);
        let mut night_controller = day_controller;
        day.apply_terrain_strip_visit(position(40, 40), &mut day_controller);
        night.apply_terrain_strip_visit(position(40, 40), &mut night_controller);

        let changed = day
            .planes
            .shading
            .iter()
            .enumerate()
            .filter_map(|(index, shading)| (*shading != 7).then_some(index))
            .collect::<Vec<_>>();
        assert!(!changed.is_empty());
        assert_eq!(day_controller, night_controller);
        assert!(changed.iter().all(|index| {
            u16::from(day.planes.shading[*index]) + u16::from(night.planes.shading[*index]) == 64
        }));
    }

    #[test]
    fn opening_emits_prepare_on_mutation_and_end_loop_only_next_visit() {
        let mut terrain = terrain(MapType::Day);
        let mut controller = TerrainStripControllerState {
            life: 1,
            subspell_index: 47,
            direction: 0,
            span: 12,
        };
        let first = terrain.apply_terrain_strip_visit(position(80, 80), &mut controller);
        assert_eq!(first.audio, Some(TerrainStripAudioOutcome::Prepare47));
        assert!(first.terrain_mutated);
        assert_eq!(controller.subspell_index, 48);
        assert_eq!(controller.life, 1);

        let second = terrain.apply_terrain_strip_visit(position(80, 80), &mut controller);
        assert_eq!(second.audio, Some(TerrainStripAudioOutcome::EndLoop47));
        assert!(!second.terrain_mutated);
        assert_eq!(controller.life, 3);
        assert_eq!(terrain.mutation_sequence, 1);
    }

    #[test]
    fn closing_emits_prepare_at_zero_transition_and_end_loop_next_visit() {
        let mut terrain = terrain(MapType::Night);
        let mut controller = TerrainStripControllerState {
            life: 2,
            subspell_index: 1,
            direction: 1,
            span: 12,
        };
        let first = terrain.apply_terrain_strip_visit(position(120, 120), &mut controller);
        assert_eq!(first.audio, Some(TerrainStripAudioOutcome::Prepare47));
        assert!(first.terrain_mutated);
        assert_eq!(controller.subspell_index, 0);
        assert_eq!(controller.life, 2);

        let second = terrain.apply_terrain_strip_visit(position(120, 120), &mut controller);
        assert_eq!(second.audio, Some(TerrainStripAudioOutcome::EndLoop47));
        assert!(!second.terrain_mutated);
        assert_eq!(controller.life, 4);
        assert_eq!(terrain.mutation_sequence, 1);
    }

    #[test]
    fn unproved_nonzero_nonone_direction_preserves_source_inert_branches() {
        let mut terrain = terrain(MapType::Day);
        let before = terrain.deterministic_hash();
        let mut controller = TerrainStripControllerState::new(2, 8);
        let initialized = terrain.apply_terrain_strip_visit(position(20, 20), &mut controller);
        assert!(!initialized.terrain_mutated);
        assert_eq!(controller.span, 9);
        assert_eq!(controller.life, 3);
        assert_eq!(controller.subspell_index, 48);
        assert_eq!(terrain.deterministic_hash(), before);
        assert_eq!(terrain.mutation_sequence, 0);

        controller.life = 1;
        controller.subspell_index = 0;
        let opening = terrain.apply_terrain_strip_visit(position(20, 20), &mut controller);
        assert_eq!(opening.audio, Some(TerrainStripAudioOutcome::Prepare47));
        assert!(!opening.terrain_mutated);
        assert_eq!(controller.subspell_index, 1);
        assert_eq!(terrain.deterministic_hash(), before);
    }
}
