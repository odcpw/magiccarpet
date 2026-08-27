//! Source-exact class-14/model-1/action-6 terrain-strip controller.
//!
//! This is the reusable Rust projection of pinned remc2 `sub_59F60`. The
//! caller owns source-slot scheduling, phase advancement, and audio journal
//! publication. This module owns the coupled controller/terrain transaction
//! and returns the exact sound-47 lifecycle request produced by one callback.

use super::{MapType, Position, SessionTerrain, TerrainPlanes};

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
struct TileCursor(u16);

impl TileCursor {
    const fn from_xy(x: u8, y: u8) -> Self {
        Self(u16::from_le_bytes([x, y]))
    }

    const fn index(self) -> usize {
        self.0 as usize
    }

    const fn x(self) -> u8 {
        self.0.to_le_bytes()[0]
    }

    const fn y(self) -> u8 {
        self.0.to_le_bytes()[1]
    }

    fn set_x(&mut self, x: u8) {
        self.0 = u16::from_le_bytes([x, self.y()]);
    }

    fn set_y(&mut self, y: u8) {
        self.0 = u16::from_le_bytes([self.x(), y]);
    }

    fn add_x(&mut self, delta: i32) {
        self.set_x(self.x().wrapping_add(delta as u8));
    }

    fn add_y(&mut self, delta: i32) {
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
