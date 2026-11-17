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

/// The same sub_51660/NewEvent carrier is used by startup and later authored
/// groups. It remains on the ordinary numeric scheduler and terrain map.
pub(crate) fn construct(
    arena: &mut crate::campaign::source_arena::SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    direction: i8,
    span: i32,
) -> Result<Option<u16>, String> {
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut source = crate::world::terrain::cave_features::new_event(
        slot,
        arena.economy.new_event_rng(slot),
        position,
    );
    bind_startup_construction(&mut source, direction, span);
    arena.economy.map_chains.sync_entity(crate::renderable_map_chain::RenderableMapSeed {
        entity_index: slot,
        active: true,
        x: position.x,
        y: position.y,
    })?;
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.entities[usize::from(slot)] = source;
    crate::world::terrain::cave_features::synchronize_links(arena, terrain, slot);
    Ok(Some(slot))
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
pub(crate) mod tests {
    use super::*;

    /// Complete the actual source33 mission in its incoming secret session.
    /// Declared player positions, arrived contacts, carried Possession and a
    /// below-goal Castle bank isolate the final source-owned mana delivery.
    pub(crate) fn complete_beleem_earned_mana_and_exit(
        game: &mut crate::game_session::GameSession<crate::campaign::backend::Mc2CampaignBackend>,
    ) -> u8 {
        use crate::campaign::source_arena::{CreatureRootOrigin, SourceEntityHandle};
        use crate::game_session::{GameCommand, GameCompletionStatus};
        use crate::level_definition::{CreatureKind, ObjectiveCondition};
        use crate::native_world::{Mc2DispatchEvent, area_damage};
        use crate::native_world::player_lifecycle_tests::{RivalTestGame, rival_test_sim as sim,
            rival_test_sim_mut as sim_mut};
        use crate::FlightWorld;
        use std::collections::BTreeMap;

        #[derive(Default)]
        struct Receipts {
            groups: BTreeMap<u16, usize>,
            transport: Option<SourceEntityHandle>,
            wave: Vec<SourceEntityHandle>,
            sphere_wave: Vec<SourceEntityHandle>,
            optional_roots: BTreeMap<u16, Vec<SourceEntityHandle>>,
            model22_parts: BTreeMap<u16, Vec<SourceEntityHandle>>,
            exit: Option<(SourceEntityHandle, Position)>,
            portal: Option<SourceEntityHandle>,
        }
        fn observe(game: &RivalTestGame, receipts: &mut Receipts) {
            let state = sim(game);
            for event in state.dispatch_events() {
                match event {
                    Mc2DispatchEvent::Error { .. } => panic!("Beleem source dispatch: {event:?}"),
                    Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group, authored_indices, .. }
                        if !authored_indices.is_empty() => {
                        let expected = state.mission().authored_features().iter().filter(|row|
                            row.index != 0 && row.kind != 0 && row.activation_group == *group as i16)
                            .map(|row| row.index).collect::<Vec<_>>();
                        assert_eq!(authored_indices.as_ref(), expected.as_slice(),
                            "all original group{group} records must reach the source producer");
                        *receipts.groups.entry(*group).or_default() += 1;
                        if *group == 1 && authored_indices.contains(&74) {
                            let row = state.entities().iter().find(|row| row.active
                                && (row.class, row.model, row.action) == (10, 34, 36)
                                && (row.position.x, row.position.y) == (128 * 256 + 128, 245 * 256 + 128))
                                .expect("actual group1 transport74");
                            receipts.transport = state.source_arena().handle(row.index);
                        }
                        if *group == 5 && authored_indices.contains(&157) {
                            receipts.wave = state.creature_runtime_census().roots_of_kind(CreatureKind::Creature21)
                                .filter(|row| row.living && matches!(row.origin,
                                    CreatureRootOrigin::AuthoredRecord { authored_record }
                                        if expected.contains(&authored_record)))
                                .map(|row| row.handle).collect();
                            assert_eq!(receipts.wave.len(), 27, "all 27 actual group5 creatures are born");
                        }
                        if *group == 14 && authored_indices.contains(&635) {
                            receipts.sphere_wave = state.economy().events().iter().filter_map(|event| match event {
                                crate::native_world::Level2EconomyEvent::ManaSphereSpawned {
                                    sphere_id, source_entity_index, mana: 512,
                                } if source_entity_index == source_slot => u16::try_from(*sphere_id).ok()
                                    .and_then(|slot| state.source_arena().handle(slot)),
                                _ => None,
                            }).collect();
                            assert_eq!(receipts.sphere_wave.len(), 16, "all 16 actual group14 spheres are born");
                        }
                        if matches!(*group, 3 | 7 | 11 | 12 | 13) {
                            let (kind, expected_roots) = match *group {
                                3 => (CreatureKind::Firefly, 14),
                                7 => (CreatureKind::Skeleton, 32),
                                _ => (CreatureKind::Creature22, 1),
                            };
                            let roots = state.creature_runtime_census().roots_of_kind(kind)
                                .filter(|row| row.living && matches!(row.origin,
                                    CreatureRootOrigin::AuthoredRecord { authored_record }
                                        if expected.contains(&authored_record)))
                                .map(|row| row.handle).collect::<Vec<_>>();
                            assert_eq!(roots.len(), expected_roots,
                                "actual group{group} must publish every authored living root");
                            if kind == CreatureKind::Creature22 {
                                let root = roots[0];
                                let mut parts = Vec::new();
                                let mut next = Some(root);
                                let mut previous = None;
                                while let Some(handle) = next {
                                    assert!(!parts.contains(&handle), "model22 source links cannot cycle");
                                    let actor = crate::native_world::creatures::creature_22::mc2::read_actor(
                                        state.source_arena(), handle).expect("current model22 linked actor");
                                    assert_eq!(actor.previous, previous);
                                    if handle != root {
                                        assert_eq!((actor.root, actor.action, actor.mana), (Some(root), 180, 0));
                                        let identity = state.creature_identity_for_source_slot(handle.source_slot)
                                            .expect("current model22 part retains its root identity");
                                        assert_eq!(identity.root_slot, root.source_slot);
                                        assert_eq!(identity.role, crate::campaign::source_arena::CreatureRuntimeRole::MultipartPart);
                                    }
                                    previous = Some(handle);
                                    next = actor.next;
                                    parts.push(handle);
                                    assert!(parts.len() <= 15);
                                }
                                assert_eq!(parts.len(), 15, "authored par1=15 creates seven linked pairs");
                                let presentations = state.campaign_entity_presentations(0)
                                    .expect("actual model22 particle bindings project for presentation");
                                for (ordinal, handle) in parts.iter().enumerate() {
                                    let actor = crate::native_world::creatures::creature_22::mc2::read_actor(
                                        state.source_arena(), *handle).unwrap();
                                    let expected_ordinal = if ordinal == 0 { 15 } else if ordinal % 2 == 1 {
                                        ((ordinal + 1) / 2) as i8
                                    } else { -((ordinal / 2) as i8) };
                                    assert_eq!(actor.length_or_ordinal, expected_ordinal);
                                    let visual = 59 - (ordinal.div_ceil(2) as u16);
                                    assert_eq!(actor.visual_index, visual);
                                    assert!(presentations.iter().any(|row| row.entity_index == handle.source_slot
                                        && row.source_id == actor.source_id && row.render_parameter_index == visual),
                                        "the real model22 actor must reach its selected particle presentation");
                                }
                                receipts.model22_parts.insert(*group, parts);
                            }
                            receipts.optional_roots.insert(*group, roots);
                        }
                        if *group == 9 && authored_indices.contains(&551) {
                            let trigger = state.economy().authored_switch_producers().iter()
                                .find(|row| row.authored_index == 551 && (row.model, row.action) == (12, 12))
                                .expect("actual Beleem exit551");
                            let portal = state.economy().authored_class14().iter()
                                .find(|row| row.authored_index == 552 && row.model == 3)
                                .expect("actual Beleem portal552");
                            receipts.exit = Some((state.source_arena().handle(trigger.source_entity_index).unwrap(), trigger.position));
                            receipts.portal = state.source_arena().handle(portal.source_entity_index);
                        }
                    }
                    _ => {}
                }
            }
        }
        fn step(game: &mut RivalTestGame, receipts: &mut Receipts, command: GameCommand) {
            game.advance(game.identity().guard(), command).unwrap();
            observe(game, receipts);
            assert!(sim(game).canonical_player_life_for_test() >= 0, "the actual Beleem player remains alive");
        }
        fn count(receipts: &Receipts, group: u16) -> usize {
            receipts.groups.get(&group).copied().unwrap_or(0)
        }
        fn contact(game: &mut RivalTestGame, player: u16, handle: SourceEntityHandle) {
            assert_eq!(sim(game).source_arena().handle(handle.source_slot), Some(handle));
            area_damage::contact(sim_mut(game).source_arena_mut(), None, handle.source_slot, u16::MAX, player).unwrap();
        }
        fn place(game: &mut RivalTestGame, player: u16, x: u16, y: u16) {
            let mut position = Position { x, y, z: 0 };
            let state = sim(game);
            let terrain = state.canonical_terrain();
            position.z = terrain.ground_height(position).wrapping_add(state.profile().ground_clearance);
            if let Some(ceiling) = terrain.ceiling_height(position) {
                assert!(position.z <= ceiling.wrapping_sub(state.profile().ceiling_clearance),
                    "the declared source position ({x},{y}) must fit between cave planes: floor={} ceiling={ceiling} floor_clearance={} ceiling_clearance={}",
                    terrain.ground_height(position), state.profile().ground_clearance, state.profile().ceiling_clearance);
            }
            sim_mut(game).set_level2_entity_position(player, position).unwrap();
        }

        let objectives = game.definition().level_definition().unwrap().objectives();
        assert_eq!(objectives.len(), 1);
        assert_eq!(objectives[0].condition(), ObjectiveCondition::CastleManaPercent(45));
        let player = sim(game).state.entity_index;
        let owner = usize::from(sim(game).state.player_color_index);
        let spawn = sim(game).state.position;
        assert_ne!(sim(game).shared_spells().local_player().enabled_spell_holders[1], 0,
            "declared incoming carry includes Possession");
        let gate = sim(game).source_arena().handle(264).expect("actual source exit gate");
        assert_eq!((sim(game).entities()[264].class, sim(game).entities()[264].model,
            sim(game).entities()[264].source_id), (11, 4, 9));
        assert_eq!(sim(game).runtime().objectives[owner].stage_selectors[0], 1);
        let sentinel = sim(game).creature_runtime_census().roots_of_kind(CreatureKind::Creature18)
            .find(|row| row.living).expect("actual reset Sentinel41").handle;
        let mut receipts = Receipts::default();
        step(game, &mut receipts, GameCommand {
            select_spell: Some(crate::game_definition::SemanticId::parse("spell.possession").unwrap()),
            ..GameCommand::default()
        });
        assert_eq!(count(&receipts, 1), 0, "the living source Sentinel blocks its absence gate");
        assert!(sim(game).creature_identity_for_source_slot(sentinel.source_slot)
            .is_some_and(|row| row.handle == sentinel && row.living));
        contact(game, player, sentinel);
        for _ in 0..256 {
            step(game, &mut receipts, GameCommand::default());
            if count(&receipts, 1) == 1 && sim(game).source_arena().resolve(sentinel).is_none() { break; }
        }
        assert_eq!(count(&receipts, 1), 1);
        assert!(sim(game).source_arena().resolve(sentinel).is_none());

        let transport = receipts.transport.expect("group1's actual transport74");
        let row = &sim(game).entities()[usize::from(transport.source_slot)];
        let destination = crate::world::fields::transport::destination(row);
        assert_eq!((destination.x, destination.y), (118 * 256 + 128, 245 * 256 + 128));
        let position = row.position;
        let entry = Position { y: position.y.wrapping_add(1), ..position };
        let heading = crate::heading_for_positions(entry, position) as i16;
        let auxiliary = sim(game).shared_spells().beyond_sight_auxiliary();
        let yaw = heading.wrapping_add(auxiliary.next_entity);
        let pitch = auxiliary.entity_index2;
        sim_mut(game).set_level2_entity_position(player, entry).unwrap();
        sim_mut(game).set_level2_player_aim_for_test(yaw, pitch);
        step(game, &mut receipts, GameCommand::default());
        let actual = sim(game).state.position;
        let near = |a: u16, b: u16| i32::from(a.wrapping_sub(b) as i16).abs() < 256;
        assert!(near(actual.x, destination.x) && near(actual.y, destination.y),
            "the original group1 transport moves the live player to its authored destination");
        for _ in 0..128 {
            if count(&receipts, 2) == 1 && count(&receipts, 4) == 1 { break; }
            step(game, &mut receipts, GameCommand::default());
        }
        assert_eq!((count(&receipts, 2), count(&receipts, 4)), (1, 1));
        // Enter the real group4 area from its outer side, away from the wave.
        // The source cave planes can close a tile even inside the switch.
        let position = {
            let state = sim(game);
            let trigger = state.economy().authored_switch_producers().iter()
                .find(|row| row.active && row.authored_index == 154 && row.model == 0 && row.stage_tag == 5)
                .expect("actual group4 area switch154");
            assert_eq!(trigger.word_10, 16);
            let terrain = state.canonical_terrain();
            (25_u16..40).flat_map(|y| (218_u16..247).map(move |x|
                Position { x: x * 256, y: y * 256, z: 0 }))
                .find(|position| {
                    let dx = i32::from(position.x.wrapping_sub(trigger.position.x) as i16).abs();
                    let dy = i32::from(position.y.wrapping_sub(trigger.position.y) as i16).abs();
                    let floor = i32::from(terrain.ground_height(*position));
                    let ceiling = terrain.ceiling_height(*position).map(i32::from);
                    dx < 15 * 256 && dy < 15 * 256
                        && terrain.material_mask(*position) != crate::MOVEMENT_REDIRECT_MASK
                        && ceiling.is_some_and(|ceiling| floor + i32::from(state.profile().ground_clearance)
                            <= ceiling - i32::from(state.profile().ceiling_clearance))
                }).expect("the actual switch154 admits an open outer cave position")
        };
        eprintln!("Beleem switch154 approach {position:?}: floor={} ceiling={:?}",
            sim(game).canonical_terrain().ground_height(position),
            sim(game).canonical_terrain().ceiling_height(position));
        place(game, player, position.x, position.y);
        for _ in 0..128 {
            if count(&receipts, 5) == 1 { break; }
            step(game, &mut receipts, GameCommand::default());
        }
        assert_eq!(count(&receipts, 5), 1);
        let wave = receipts.wave.clone();
        assert_eq!(wave.len(), 27);
        sim_mut(game).set_level2_entity_position(player, spawn).unwrap();
        let survivor = wave[0];
        for &handle in &wave[1..] { contact(game, player, handle); }
        for _ in 0..512 {
            step(game, &mut receipts, GameCommand::default());
            assert_eq!(count(&receipts, 6), 0, "one living model21 blocks the actual absence switch");
            if wave[1..].iter().all(|handle| sim(game).source_arena().resolve(*handle).is_none()) { break; }
        }
        assert!(wave[1..].iter().all(|handle| sim(game).source_arena().resolve(*handle).is_none()));
        assert!(sim(game).creature_identity_for_source_slot(survivor.source_slot)
            .is_some_and(|row| row.handle == survivor && row.living));
        contact(game, player, survivor);
        for _ in 0..512 {
            step(game, &mut receipts, GameCommand::default());
            if count(&receipts, 6) == 1 && sim(game).source_arena().resolve(survivor).is_none() { break; }
        }
        assert_eq!(count(&receipts, 6), 1);
        assert!(wave.iter().all(|handle| sim(game).source_arena().resolve(*handle).is_none()));
        assert!(sim(game).entities().iter().any(|row| row.active && (row.class, row.model, row.action) == (10, 34, 36)
            && (row.position.x, row.position.y) == (31 * 256 + 128, 62 * 256 + 128)),
            "actual group6 produces authored transport334");
        assert_eq!(sim(game).runtime().objectives[owner].stage_selectors[0], 1);

        // These optional reset areas are ordinary authored producers, not
        // prerequisites for the 45-percent objective or the exit.
        for (authored, group, kind) in [
            (305, 3, CreatureKind::Firefly),
            (359, 7, CreatureKind::Skeleton),
            (553, 11, CreatureKind::Creature22),
            (560, 12, CreatureKind::Creature22),
            (568, 13, CreatureKind::Creature22),
        ] {
            if count(&receipts, group) == 0 {
                let position = {
                    let state = sim(game);
                    let feature = state.mission().authored_features().iter()
                        .find(|row| row.index == authored && row.kind == 11 && row.subtype == 0
                            && row.activation_group == 0 && row.stage_tag == group as i16)
                        .expect("the original optional area record");
                    let center = feature.world_position(0);
                    let trigger = state.entities().iter().find(|row| row.active
                        && (row.class, row.model, row.action, row.source_id) == (11, 0, 0, group)
                        && (row.position.x, row.position.y) == (center.x, center.y))
                        .expect("the actual reset area source trigger is current");
                    let width = i32::from(trigger.array_pitch()).abs();
                    let depth = i32::from(trigger.array_roll()).abs();
                    let terrain = state.canonical_terrain();
                    [-3_i16, 3, -2, 2, -1, 1, 0].into_iter().flat_map(|dy|
                        [-3_i16, 3, -2, 2, -1, 1, 0].into_iter().map(move |dx| Position {
                            x: center.x.wrapping_add_signed(dx * 256),
                            y: center.y.wrapping_add_signed(dy * 256), z: 0,
                        })).find(|position| {
                            let dx = i32::from(position.x.wrapping_sub(center.x) as i16).abs();
                            let dy = i32::from(position.y.wrapping_sub(center.y) as i16).abs();
                            let floor = i32::from(terrain.ground_height(*position));
                            dx < width && dy < depth
                                && terrain.material_mask(*position) != crate::MOVEMENT_REDIRECT_MASK
                                && terrain.ceiling_height(*position).is_some_and(|ceiling|
                                    floor + i32::from(state.profile().ground_clearance)
                                        <= i32::from(ceiling) - i32::from(state.profile().ceiling_clearance))
                        }).unwrap_or_else(|| panic!("optional area{authored}/group{group} has no admitted cave position: trigger={trigger:?}"))
                };
                place(game, player, position.x, position.y);
                for _ in 0..128 {
                    if count(&receipts, group) != 0 { break; }
                    step(game, &mut receipts, GameCommand::default());
                }
            }
            assert_eq!(count(&receipts, group), 1, "the real optional area publishes its group once");
            let roots = receipts.optional_roots.get(&group).expect("actual authored creature roots").clone();
            let root = roots[0];
            assert_eq!(sim(game).source_arena().handle(root.source_slot), Some(root));
            assert_eq!(sim(game).creature_identity_for_source_slot(root.source_slot).unwrap().kind, kind);
            sim_mut(game).set_level2_entity_position(player, spawn).unwrap();
            step(game, &mut receipts, GameCommand::default());
            let scheduled = sim(game).cf10_production_schedule_receipts().into_iter().any(|receipt|
                matches!(receipt, crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, .. }
                    if slot == root.source_slot));
            assert!(scheduled, "the actual group{group} root reaches its source callback");
            assert_eq!(sim(game).source_arena().handle(root.source_slot), Some(root));
            if kind != CreatureKind::Creature22 {
                assert!(sim(game).campaign_entity_presentations(0).unwrap().iter()
                    .any(|row| row.entity_index == root.source_slot),
                    "the actual group{group} root reaches its particle presentation");
            } else {
                let parts = &receipts.model22_parts[&group];
                assert_eq!(parts.len(), 15);
                assert!(crate::native_world::creatures::creature_22::enabled(180)
                    && !crate::native_world::creatures::creature_22::enabled(182),
                    "the source segment callback is enabled and action182 remains null");
                assert!(sim(game).cf10_production_schedule_receipts().into_iter().any(|receipt|
                    matches!(receipt, crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, .. }
                        if parts.iter().skip(1).any(|part| part.source_slot == slot))),
                    "a linked model22 part reaches its real action180 callback");
                if group == 11 {
                    let before = crate::native_world::creatures::creature_22::mc2::read_actor(
                        sim(game).source_arena(), root).unwrap();
                    area_damage::contact(sim_mut(game).source_arena_mut(), None, root.source_slot, 200, player).unwrap();
                    let admitted = crate::native_world::creatures::creature_22::mc2::read_actor(
                        sim(game).source_arena(), root).unwrap();
                    assert_eq!((admitted.hit_source, admitted.damage), (player, 200));
                    step(game, &mut receipts, GameCommand::default());
                    let after = crate::native_world::creatures::creature_22::mc2::read_actor(
                        sim(game).source_arena(), root).expect("the contacted model22 generation remains current");
                    assert_eq!((after.hit_source, after.life), (0, before.life));
                    assert!(after.speed > after.maximum_speed,
                        "source model22 consumes Contact as acceleration rather than generic damage");
                }
            }
            assert_eq!(sim(game).runtime().objectives[owner].stage_selectors[0], 1,
                "optional areas do not fabricate the Castle mana objective");
        }

        // This real reset area produces the source group14 mana wave; it is
        // not a staged objective or a constructed test sphere.
        place(game, player, 130 * 256, 160 * 256);
        for _ in 0..128 {
            if count(&receipts, 14) == 1 { break; }
            step(game, &mut receipts, GameCommand::default());
        }
        assert_eq!(count(&receipts, 14), 1);
        assert_eq!(receipts.sphere_wave.len(), 16);
        sim_mut(game).set_level2_entity_position(player, spawn).unwrap();
        let mut checked_prediction_is_read_only = false;
        let mut predictions = Vec::new();
        let sphere = (0..512).find_map(|_| {
            let candidates = sim(game).economy().spheres().iter().filter(|row|
                row.active && !row.pending_delete && row.player_entity_index == 0 && row.control_flags & 8 != 0
                    && row.control_flags & 0x0020_0000 == 0 && row.vertical_velocity == 0
                    && row.velocity_x == 0 && row.velocity_y == 0 && row.mana > 0
                    && row.position.z == sim(game).canonical_terrain().ground_height(row.position)
                    && sim(game).source_arena().handle(row.source_entity_index)
                        .is_some_and(|handle| receipts.sphere_wave.contains(&handle))
                    && crate::native_world::tests::paid_sphere_firing_pose_for_test(game, **row).is_some())
                .filter_map(|row| sim(game).source_arena().handle(row.source_entity_index).map(|handle| (*row, handle)))
                .collect::<Vec<_>>();
            // An apex also has velocity0. A real following callback must keep
            // the same generation and position on the current cave ground.
            step(game, &mut receipts, GameCommand::default());
            predictions.clear();
            candidates.into_iter().find_map(|(row, handle)| {
                if sim(game).source_arena().handle(row.source_entity_index) != Some(handle)
                    || !sim(game).economy().spheres().iter().any(|current|
                    current.active && !current.pending_delete && current.id == row.id
                        && current.player_entity_index == 0 && current.mana == row.mana
                        && current.position == row.position
                        && current.position.z == sim(game).canonical_terrain().ground_height(current.position)) {
                    return None;
                }
                let current = sim(game).economy().spheres().iter().find(|current|
                    current.active && current.id == row.id).copied().unwrap();
                let (origin, yaw, pitch) = crate::native_world::tests::paid_sphere_firing_pose_for_test(game, current)?;
                let before = (!checked_prediction_is_read_only).then(|| sim(game).campaign_state_hash());
                let prediction = sim(game).possession_target_prediction_for_test(origin, yaw, pitch)
                    .expect("the source Possession prediction can inspect the cloned current carriers");
                if let Some(before) = before {
                    assert_eq!(sim(game).campaign_state_hash(), before,
                        "source shot prediction must not mutate the live campaign");
                    checked_prediction_is_read_only = true;
                }
                predictions.push((handle, origin, yaw, pitch, prediction));
                prediction.filter(|(target, _, _)| *target == handle).map(|_| current)
            })
        }).unwrap_or_else(|| panic!("no settled group14 sphere has an admitted source-ranked firing pose: wave={:?}, predictions={predictions:?}, live={:?}",
            receipts.sphere_wave, sim(game).economy().spheres()));
        eprintln!("Beleem source acquisition preflight: {predictions:?}");
        assert!(checked_prediction_is_read_only);
        let sphere_handle = sim(game).source_arena().handle(sphere.source_entity_index).unwrap();
        let castle = {
            let state = sim_mut(game);
            for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
                state.economy_mut().set_castle_stage_footprint(stage, footprint);
            }
            let site = (1_u16..32).flat_map(|y| (1_u16..32).map(move |x| Position { x: x * 2048, y: y * 2048, z: 0 }))
                .map(|mut position| { position.z = state.canonical_terrain().ground_height(position); position })
                .filter(|position| {
                    let dx = i32::from(position.x.wrapping_sub(sphere.position.x) as i16).abs();
                    let dy = i32::from(position.y.wrapping_sub(sphere.position.y) as i16).abs();
                    dx.max(dy) >= 4096 && !state.economy().castle_projectile_site_blocked(
                        *position, state.entities(), state.canonical_terrain())
                }).min_by_key(|position| crate::distance_3d_wrapped(*position, sphere.position))
                .expect("the actual cave admits a declared prior Castle");
            state.economy_mut().build_castle(site, player).unwrap()
        };
        step(game, &mut receipts, GameCommand::default());
        assert_eq!(sim(game).source_arena().handle(sphere_handle.source_slot), Some(sphere_handle));
        assert!(sim(game).economy().spheres().iter().any(|row| row.active && row.id == sphere.id
            && row.position == sphere.position && row.mana == sphere.mana),
            "the real group14 sphere is stationary with its source payload across Castle setup");
        assert_eq!(sim(game).runtime().objectives[owner].stage_selectors[0], 1,
            "an empty Castle remains below the 45-percent goal");
        let mut paid_projectile = None;
        let mut actual_acquisition = false;
        crate::native_world::tests::deliver_paid_sphere_for_test(game, castle, sphere, spawn, 45,
            |game| {
                observe(game, &mut receipts);
                let state = sim(game);
                if let Some(projectile) = state.economy().possession_projectiles().iter()
                    .find(|row| row.active && row.source_id_entity_index == player)
                {
                    let handle = state.source_arena().handle(projectile.source_entity_index)
                        .expect("the paid Possession flyer has a current generation");
                    if let Some(original) = paid_projectile {
                        assert_eq!(handle, original, "the paid source flyer must keep its generation");
                    } else {
                        paid_projectile = Some(handle);
                    }
                    if projectile.target_source_entity_index != 0 {
                        let actual = state.source_arena().handle(projectile.target_source_entity_index);
                        assert_eq!(actual, Some(sphere_handle),
                            "the actual paid source flyer must acquire the predicted group14 sphere: projectile={projectile:?}, preflight={predictions:?}");
                        actual_acquisition = true;
                    }
                }
            });
        assert!(paid_projectile.is_some() && actual_acquisition,
            "the actual paid source flyer must acquire the same generation, not just predict it");
        for _ in 0..128 {
            if count(&receipts, 9) == 1 { break; }
            step(game, &mut receipts, GameCommand::default());
        }
        assert_eq!(sim(game).runtime().objectives[owner].stage_selectors[0], 2);
        assert_eq!(sim(game).runtime().objectives[owner].is_level_end, 1);
        assert_eq!(count(&receipts, 9), 1);
        for _ in 0..8 {
            if sim(game).source_arena().resolve(gate).is_none() { break; }
            step(game, &mut receipts, GameCommand::default());
        }
        assert!(sim(game).source_arena().resolve(gate).is_none());
        let (exit, position) = receipts.exit.expect("actual group9 exit551");
        let portal = receipts.portal.expect("actual group9 portal552");
        assert!(sim(game).canonical_player_life_for_test() >= 0);
        sim_mut(game).set_level2_entity_position(player, position).unwrap();
        for _ in 0..700 {
            if matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }) { break; }
            step(game, &mut receipts, GameCommand::default());
        }
        assert!(matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }));
        let sequence = sim(game).level_exit_sequence();
        assert_eq!((sequence.trigger_entity_index, sequence.portal_entity_index), (exit.source_slot, portal.source_slot));
        sim(game).source_exit_result_byte()
    }

    #[test]
    fn beleem_actual_source_chain_paid_mana_and_secret_exit() {
        use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
        use crate::game_session::GameSession;
        let generated = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let package = std::env::var_os("MC_SECRET33_PACKAGE").map(std::path::PathBuf::from)
            .unwrap_or_else(|| generated.join("mc-paj2-source-secret-33-campaign-package"));
        let cache = std::env::var_os("MC_SECRET33_OWNER_CACHE").map(std::path::PathBuf::from)
            .unwrap_or_else(|| generated.join("campaign-original-environments/source33"));
        let definition = load_mc2_campaign(&package, &crate::game_definition::default_catalog().unwrap()).unwrap();
        let mut game = GameSession::open(&Mc2CampaignAdapter, std::sync::Arc::new(definition), 2233,
            Mc2CampaignInputs::with_owner_cache(&package, &cache)).unwrap();
        game.backend_mut().compatibility_campaign_mut_for_test().simulation_mut()
            .apply_carried_spell_families(&[1], Some(1)).unwrap();
        assert_eq!(complete_beleem_earned_mana_and_exit(&mut game), 0x0a);
    }

    #[test]
    fn beleem_actual_source_mission_returns_to_retained_parent() {
        crate::campaign::backend::tests::assert_actual_secret_completed_return(
            33, complete_beleem_earned_mana_and_exit,
        );
    }

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
            source_predicted_position: Default::default(),
            voxel: None,
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
        let contract = crate::MissionAudioSite::TerrainStrip.contract();
        assert_eq!((contract.sounds, contract.play_type), ("47", "4"));
        let prepared_type = crate::prepare_event_sound_command(
            Position::default(),
            0,
            Position::default(),
            47,
            0,
            slot,
            7,
        )
        .expect("source sound 47 has a playback policy")
        .play_type;
        assert_eq!(prepared_type.to_string(), contract.play_type);
        let mut prepared_count = 0;
        for record in &session.effect_journal().records()[cursor..] {
            if matches!(record.effect,
                crate::campaign::journal::CampaignEffect::Flight(FlightEvent::PrepareSound {
                    entity_index, sound_id: 47, audio_site: crate::MissionAudioSite::TerrainStrip, ..
                }) if entity_index == slot)
            {
                assert_eq!(
                    record
                        .audio
                        .expect("scheduled corridor sound is prepared")
                        .play_type,
                    prepared_type
                );
                prepared_count += 1;
            }
        }
        assert!(
            prepared_count > 0,
            "actual source callback prepares sound 47"
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
    fn mission_four_spider_and_corridor_share_live_opening() {
        use crate::FlightWorld;
        let generated = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let package = generated.join("mc-bhq6-ordinary-20261025-zclu5fjj/level4");
        let cache = generated.join("mc-b9s2-cursor-level4-20261029");
        let mut campaign = crate::campaign::CampaignSession::open_with_owner_cache(
            &package, &cache, crate::campaign::spec::CampaignSelection::ordinary(4).unwrap(),
        ).expect("installed Cave4 package and selected owner");
        let assets = crate::day_indexed_frame::DayIndexedFrameAssets::load(&cache).unwrap();
        let player = campaign.simulation().state.entity_index;
        let corridor = campaign.simulation().source_arena().handle(206).unwrap();
        let position = campaign.simulation().entities()[206].position;
        let mut receipt = Vec::new();
        let mut step = |campaign: &mut crate::campaign::CampaignSession, phase: &str, command: crate::PlayerCommand| {
            let _ = campaign.step(command, 0);
            let sim = campaign.simulation();
            assert!(sim.dispatch_events().iter().all(|event| !matches!(event,
                crate::native_world::Mc2DispatchEvent::Error { .. })), "{phase}: {:?}", sim.dispatch_events());
            let row = &sim.entities()[206];
            let view = sim.source_camera_pose();
            let spider = sim.creature_runtime_census().roots_of_kind(crate::level_definition::CreatureKind::Spider)
                .find(|row| row.living && matches!(row.origin,
                    crate::campaign::source_arena::CreatureRootOrigin::AuthoredRecord { authored_record: 600 }));
            receipt.push(serde_json::json!({"phase":phase, "tick":sim.state.tick,
                "input":{"flags":command.flags,"roll":command.roll,"pitch":command.pitch,
                    "argument_6":command.argument_6,"argument_8":command.argument_8},
                "player":{"x":sim.state.position.x,"y":sim.state.position.y,"z":sim.state.position.z},
                "camera":{"x":view.x,"y":view.y,"z":view.z,"yaw":view.yaw,"pitch":view.pitch,"roll":view.roll},
                "spider":spider.map(|root| { let row = &sim.entities()[usize::from(root.handle.source_slot)];
                    serde_json::json!({"slot":root.handle.source_slot,"generation":root.handle.generation,
                        "x":row.position.x,"y":row.position.y,"z":row.position.z,"action":row.action}) }),
                "corridor_life":row.life,
                "ground":sim.canonical_terrain().ground_height(position),
                "ceiling":sim.canonical_terrain().ceiling_height(position)}));
        };
        for _ in 0..256 {
            step(&mut campaign, "closing", crate::PlayerCommand::default());
            if campaign.simulation().entities()[206].life == 3 { break; }
        }
        let closed = campaign.simulation().canonical_terrain();
        assert_eq!(campaign.simulation().entities()[206].life, 3);
        assert!(closed.cave_collision_blocked(position));
        let closed_ground = closed.ground_height(position);
        let closed_ceiling = closed.ceiling_height(position).unwrap();

        // Only player position is controlled. The two source switches, their
        // groups, the Spider and controller are ordinary scheduled owners.
        let switch = campaign.simulation().entities()[4].clone();
        assert_eq!((switch.class, switch.model, switch.source_id), (11, 0, 11));
        let approach = (-8_i16..=8).flat_map(|dy| (-8_i16..=8).map(move |dx| (dx, dy)))
            .find_map(|(dx, dy)| {
                let sim = campaign.simulation();
                let mut p = Position { x: switch.position.x.wrapping_add_signed(dx * 256),
                    y: switch.position.y.wrapping_add_signed(dy * 256), z: 0 };
                let player_row = &sim.entities()[usize::from(player)];
                let extent = |row: &EntitySnapshot, offset| i16::from_le_bytes(
                    row.record_bytes()[offset..offset + 2].try_into().unwrap());
                if !crate::level1_mission::source_entities_overlap_xy(switch.position,
                    extent(&switch, 88), extent(&switch, 90), p, extent(player_row, 88), extent(player_row, 90))
                    || sim.canonical_terrain().cave_collision_blocked(p) { return None; }
                p.z = sim.canonical_terrain().ground_height(p).checked_add(sim.profile().ground_clearance)?;
                (p.z <= sim.canonical_terrain().ceiling_height(p)?.checked_sub(sim.profile().ceiling_clearance)?)
                    .then_some(p)
            }).expect("source switch4 has an open player approach");
        campaign.simulation_mut().set_level2_entity_position(player, approach).unwrap();
        let mut group11 = false;
        let mut group50 = false;
        let mut spider = None;
        for _ in 0..128 {
            step(&mut campaign, "spider-switch", crate::PlayerCommand::default());
            let sim = campaign.simulation();
            for event in sim.dispatch_events() {
                if let crate::native_world::Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group, authored_indices, .. } = event {
                    group11 |= *source_slot == 4 && *group == 11 && authored_indices.contains(&577);
                    group50 |= *group == 50 && authored_indices.contains(&600);
                }
            }
            spider = sim.creature_runtime_census().roots_of_kind(crate::level_definition::CreatureKind::Spider)
                .find(|row| row.living && matches!(row.origin,
                    crate::campaign::source_arena::CreatureRootOrigin::AuthoredRecord { authored_record: 600 }))
                .map(|row| row.handle);
            if spider.is_some() && group11 && group50 { break; }
        }
        let spider = spider.expect("real switch4/group11/switch577/group50 constructs Spider600");
        assert!(group11 && group50);
        let switch = campaign.simulation().entities()[125].clone();
        assert_eq!((switch.class, switch.model, switch.action, switch.source_id), (11, 0, 0, 40));
        let mut approach = switch.position;
        approach.x = approach.x.wrapping_sub(768);
        approach.z = campaign.simulation().canonical_terrain().ground_height(approach)
            + campaign.simulation().profile().ground_clearance;
        assert!(!campaign.simulation().canonical_terrain().cave_collision_blocked(approach));
        assert!(approach.z <= campaign.simulation().canonical_terrain().ceiling_height(approach).unwrap()
            - campaign.simulation().profile().ceiling_clearance);
        campaign.simulation_mut().set_level2_entity_position(player, approach).unwrap();
        let mut produced = false;
        let mut opened = false;
        let mut previous_frame: Option<(i16, i16, sim_session::CameraPose, Vec<u8>, Vec<u16>)> = None;
        let mut changed_frame = false;
        let mut admitted_spider = false;
        for _ in 0..256 {
            step(&mut campaign, "corridor-switch-and-opening", crate::PlayerCommand::default());
            let sim = campaign.simulation();
            assert_eq!(sim.source_arena().handle(corridor.source_slot), Some(corridor));
            assert_eq!(sim.source_arena().handle(spider.source_slot), Some(spider));
            produced |= sim.dispatch_events().iter().any(|event| matches!(event,
                crate::native_world::Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group, authored_indices, .. }
                if *source_slot == 125 && *group == 40 && authored_indices.contains(&584)));
            let frame = crate::day_indexed_frame::DayIndexedFrameState::new(campaign.generation())
                .render_current(campaign.inspection(), sim, &assets, campaign.generation()).unwrap();
            admitted_spider |= frame.animation_sources().iter().any(|row| row.entity_index == spider.source_slot);
            let ground = sim.canonical_terrain().ground_height(position);
            let ceiling = sim.canonical_terrain().ceiling_height(position).unwrap();
            let camera = sim.source_camera_pose();
            if let Some((previous_ground, previous_ceiling, previous_camera, previous_pixels, previous_orders)) = previous_frame.take() {
                if (previous_ground != ground || previous_ceiling != ceiling) && previous_camera == camera
                    && previous_pixels.iter().zip(&frame.scene_indices)
                        .zip(previous_orders.iter().zip(frame.cave_terrain_orders.as_ref().unwrap()))
                        .any(|((old_pixel, pixel), (old_order, order))| old_pixel != pixel && old_order != order) {
                    changed_frame = true;
                }
            }
            previous_frame = Some((ground, ceiling, camera, frame.scene_indices, frame.cave_terrain_orders.unwrap()));
            if sim.entities()[206].life == 4 { opened = true; break; }
        }
        assert!(produced && opened, "ordinary switch125/group40/command584 opens controller206");
        let terrain = campaign.simulation().canonical_terrain();
        assert!(!terrain.cave_collision_blocked(position));
        assert!(terrain.ground_height(position) < closed_ground);
        assert!(terrain.ceiling_height(position).unwrap() > closed_ceiling);
        assert!(changed_frame, "live source steps publish changed Cave frames");
        assert!(admitted_spider, "the same live source camera admits Spider600 while the corridor opens");
        let mut flight = position;
        flight.z = (terrain.ground_height(position) + terrain.ceiling_height(position).unwrap()) / 2;
        assert_eq!(crate::resolve_legacy_movement(&mut crate::FlightState::default(), flight, flight,
            crate::FlightProfile::CAVE, terrain), crate::MovementResolution::Accept(flight));
        // Continue this same session with ordinary flight input. No player
        // relocation, camera override or terrain mutation occurs after the
        // source switch approach above.
        // Turn toward the east-west corridor before accelerating, so the
        // player stays within its narrow north-south extent.
        for _ in 0..17 {
            step(&mut campaign, "hover-turn", crate::PlayerCommand { roll: 127, ..Default::default() });
        }
        let flight_start = campaign.simulation().state.position;
        let mut moved = false;
        let mut entered = false;
        let mut crossed = false;
        let mut visible_spider = None;
        for _ in 0..64 {
            // Negative pitch climbs: move_entity subtracts the signed sine
            // contribution from altitude. Ordinary free-look keeps the
            // source camera level and facing east along the corridor.
            let before_step = campaign.simulation().state.position;
            let yaw = campaign.simulation().state.yaw;
            let pitch = campaign.simulation().state.pitch_accumulator;
            step(&mut campaign, "flight", crate::PlayerCommand { flags: 1, pitch: -64,
                argument_6: 512_i16.wrapping_sub(yaw) as u16,
                argument_8: (-(pitch / 2)) as u16, ..Default::default() });
            let sim = campaign.simulation();
            assert_eq!(sim.source_arena().handle(spider.source_slot), Some(spider));
            let player_position = sim.state.position;
            moved |= player_position != flight_start;
            let terrain = sim.canonical_terrain();
            assert!(player_position.z >= terrain.ground_height(player_position));
            assert!(player_position.z <= terrain.ceiling_height(player_position).unwrap());
            assert_eq!(sim.entities()[206].life, 4, "the same corridor remains open during flight");
            let within_span = (99..=106).contains(&(player_position.y >> 8));
            entered |= within_span && player_position.x >= position.x - 256 && player_position.x <= position.x + 256;
            crossed |= entered && within_span && player_position.x > position.x + 256;
            let generation = campaign.generation();
            let full = crate::day_indexed_frame::DayIndexedFrameState::new(generation)
                .render_current(campaign.inspection(), sim, &assets, generation).unwrap();
            if visible_spider.is_none() && full.animation_sources().iter().any(|row| row.entity_index == spider.source_slot) {
                let row = sim.campaign_entity_presentations(sim.source_camera_pose().yaw).unwrap().into_iter()
                    .find(|row| row.entity_index == spider.source_slot).unwrap();
                let mut without = campaign.clone();
                without.simulation_mut().session.state.live.economy.economy.map_chains.sync_entity(
                    crate::renderable_map_chain::RenderableMapSeed { entity_index: spider.source_slot,
                        active: false, x: row.position.x, y: row.position.y }).unwrap();
                let absent = crate::day_indexed_frame::DayIndexedFrameState::new(generation)
                    .render_current(without.inspection(), without.simulation(), &assets, generation).unwrap();
                let changed: Vec<_> = full.scene_indices.iter().zip(&absent.scene_indices).enumerate()
                    .filter_map(|(index, (&a, &b))| {
                        let a = usize::from(a) * 3; let b = usize::from(b) * 3;
                        (full.palette_rgb8[a..a + 3] != absent.palette_rgb8[b..b + 3]).then_some(index)
                    }).collect();
                if player_position != before_step && !changed.is_empty() {
                    visible_spider = Some((sim.source_camera_pose(), full, absent, changed));
                }
            }
            if crossed && visible_spider.is_some() { break; }
        }
        drop(step);
        if let Some(path) = std::env::var_os("MC_CAVE_CONNECTED_RECEIPT") {
            let mut output = std::fs::OpenOptions::new().write(true).create_new(true).open(path).unwrap();
            serde_json::to_writer_pretty(&mut output, &serde_json::json!({"controlled_player_approach":true,
                "spider_slot":spider.source_slot,"spider_generation":spider.generation,
                "corridor_slot":corridor.source_slot,"steps":receipt})).unwrap();
        }
        let sim = campaign.simulation();
        let source = &sim.entities()[usize::from(spider.source_slot)];
        eprintln!("Cave4 flight end moved={moved} crossed={crossed} view={:?} player={:?} Spider={:?} action={} visible={}",
            sim.source_camera_pose(), sim.state.position, source.position, source.action, visible_spider.is_some());
        assert!(moved, "ordinary flight input must move the real player within the live Cave");
        assert!(crossed, "ordinary flight must cross the far edge of the opened corridor");
        let (view, full, absent, changed) = visible_spider.expect("the live flight view shows real Spider600 pixels");
        eprintln!("Cave4 live flight Spider={} view={view:?} visible_pixels={} first=({}, {})",
            spider.source_slot, changed.len(), changed[0] % 320, changed[0] / 320);
        if let Some(root) = std::env::var_os("MC_CAVE_CONNECTED_EVIDENCE") {
            use std::io::Write;
            let root = std::path::PathBuf::from(root);
            std::fs::create_dir_all(&root).unwrap();
            for (name, frame) in [("flight", &full), ("without-spider", &absent)] {
                let mut output = std::fs::OpenOptions::new().write(true).create_new(true)
                    .open(root.join(format!("{name}.ppm"))).unwrap();
                write!(output, "P6\n320 200\n255\n").unwrap();
                for &index in &frame.scene_indices {
                    output.write_all(&frame.palette_rgb8[usize::from(index) * 3..usize::from(index) * 3 + 3]).unwrap();
                }
            }
        }
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
                source_predicted_position: Default::default(),
                voxel: None,
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
        // Keep this separate from the original corridor sequence: the extra
        // source births legitimately change allocator order and callback timing.
        let mut actor_campaign = campaign.clone();
        // Enter the real switch4 region. Its group11 constructs authored
        // switch577; its ordinary callback in turn publishes group50 and the
        // authored Spider600. Only the player's approach is controlled.
        let switch4 = actor_campaign.simulation().entities()[4].clone();
        assert_eq!((switch4.class, switch4.model, switch4.source_id), (11, 0, 11));
        let player = actor_campaign.simulation().state.entity_index;
        let approach = (-8_i16..=8).flat_map(|dy| (-8_i16..=8).map(move |dx| (dx, dy)))
            .find_map(|(dx, dy)| {
                let sim = actor_campaign.simulation();
                let mut position = Position {
                    x: switch4.position.x.wrapping_add_signed(dx * 256),
                    y: switch4.position.y.wrapping_add_signed(dy * 256),
                    z: 0,
                };
                let player_row = &sim.entities()[usize::from(player)];
                let extent = |row: &EntitySnapshot, offset| i16::from_le_bytes(
                    row.record_bytes()[offset..offset + 2].try_into().unwrap());
                if !crate::level1_mission::source_entities_overlap_xy(
                    switch4.position, extent(&switch4, 88), extent(&switch4, 90),
                    position, extent(player_row, 88), extent(player_row, 90),
                ) { return None; }
                let terrain = sim.canonical_terrain();
                if terrain.cave_collision_blocked(position) { return None; }
                position.z = terrain.ground_height(position).checked_add(sim.profile().ground_clearance)?;
                (position.z <= terrain.ceiling_height(position)?.checked_sub(sim.profile().ceiling_clearance)?)
                    .then_some(position)
            }).expect("real switch4 has an open player approach");
        actor_campaign.simulation_mut().set_level2_entity_position(player, approach).unwrap();
        let mut saw_group11 = false;
        let mut saw_group50 = false;
        let mut spider = None;
        for _ in 0..128 {
            let _ = actor_campaign.step(crate::PlayerCommand::default(), 0);
            let sim = actor_campaign.simulation();
            assert!(sim.dispatch_events().iter().all(|event| !matches!(event,
                crate::native_world::Mc2DispatchEvent::Error { .. })), "Spider activation: {:?}", sim.dispatch_events());
            for event in sim.dispatch_events() {
                if let crate::native_world::Mc2DispatchEvent::AuthoredGroupActivated {
                    source_slot, group, authored_indices, ..
                } = event {
                    saw_group11 |= *source_slot == 4 && *group == 11 && authored_indices.contains(&577);
                    saw_group50 |= *group == 50 && authored_indices.contains(&600);
                }
            }
            spider = sim.creature_runtime_census().roots_of_kind(crate::level_definition::CreatureKind::Spider)
                .find(|row| row.living && matches!(row.origin,
                    crate::campaign::source_arena::CreatureRootOrigin::AuthoredRecord { authored_record: 600 }))
                .map(|row| row.handle);
            if spider.is_some() && saw_group11 && saw_group50 { break; }
        }
        let spider = spider.expect("ordinary switch4 -> group11 -> switch577 -> group50 constructs authored Spider600");
        assert!(saw_group11 && saw_group50, "both actual source activation callbacks must run");
        // One declared player-only approach to each real proximity switch;
        // the switch cadence, complete authored group and command lifetime are
        // all driven by the ordinary source scheduler.
        for (switch_slot, group, authored_index, corridor_slot) in
            [(124_u16, 37_u16, 582_u16, 195_u16), (125, 40, 584, 206)]
        {
            let (player, switch_handle, corridor_handle, approach, trigger_position) = {
                let sim = campaign.simulation();
                let live = &sim.session.state.live;
                let switch = &live.economy.entities[usize::from(switch_slot)];
                assert!(switch.active && !switch.drawing_disabled());
                assert_eq!(
                    (switch.class, switch.model, switch.action, switch.source_id),
                    (11, 0, 0, group)
                );
                let corridor = &live.economy.entities[usize::from(corridor_slot)];
                assert!(roots.iter().any(|(slot, _, _, _)| *slot == corridor_slot));
                assert_eq!(corridor.life, 3);
                let feature = authored
                    .iter()
                    .find(|row| row.index == authored_index)
                    .expect("the actual switch group owns its opening command");
                assert_eq!(
                    (feature.kind, feature.subtype, feature.activation_group),
                    (10, 63, group as i16)
                );
                assert_eq!(
                    (feature.tile_x, feature.tile_y),
                    (corridor.position.x >> 8, corridor.position.y >> 8)
                );
                let trigger_position =
                    feature.world_position(live.terrain.ground_height(feature.world_position(0)));
                let mut approach = switch.position;
                // The reset switch lies inside the sealed two-cell strip.
                // Approach perpendicular to it, within the switch's actual
                // source overlap, without changing the corridor or the switch.
                if switch_slot == 124 {
                    approach.y = approach.y.wrapping_sub(768);
                } else {
                    approach.x = approach.x.wrapping_sub(768);
                }
                let player_row = &live.economy.entities[usize::from(sim.state.entity_index)];
                let extent = |row: &EntitySnapshot, offset| {
                    i16::from_le_bytes(row.record_bytes()[offset..offset + 2].try_into().unwrap())
                };
                assert!(
                    crate::level1_mission::source_entities_overlap_xy(
                        switch.position,
                        extent(switch, 88),
                        extent(switch, 90),
                        approach,
                        extent(player_row, 88),
                        extent(player_row, 90),
                    ),
                    "the fixed player approach must enter the actual switch bounds"
                );
                approach.z = live
                    .terrain
                    .ground_height(approach)
                    .checked_add(sim.profile().ground_clearance)
                    .expect("source floor clearance fits the player position");
                assert!(
                    approach.z
                        <= live
                            .terrain
                            .ceiling_height(approach)
                            .unwrap()
                            .checked_sub(sim.profile().ceiling_clearance)
                            .expect("source ceiling clearance fits the player position"),
                    "declared player approach must fit the real Cave"
                );
                assert!(
                    !live.terrain.cave_collision_blocked(approach),
                    "declared player approach must be outside the sealed corridor"
                );
                (
                    sim.state.entity_index,
                    sim.source_arena().handle(switch_slot).unwrap(),
                    sim.source_arena().handle(corridor_slot).unwrap(),
                    approach,
                    trigger_position,
                )
            };
            campaign
                .simulation_mut()
                .set_level2_entity_position(player, approach)
                .unwrap();
            let mut produced = false;
            for _ in 0..16 {
                let _ = campaign.step(crate::PlayerCommand::default(), 0);
                let sim = campaign.simulation();
                assert!(
                    sim.dispatch_events().iter().all(|event| !matches!(
                        event,
                        crate::native_world::Mc2DispatchEvent::Error { .. }
                    )),
                    "source switch callback failed: {:?}",
                    sim.dispatch_events()
                );
                assert_eq!(
                    sim.source_arena().handle(corridor_slot),
                    Some(corridor_handle)
                );
                if sim.dispatch_events().iter().any(|event| {
                    matches!(event,
                    crate::native_world::Mc2DispatchEvent::AuthoredGroupActivated {
                        source_slot, group: actual_group, authored_indices, ..
                    } if *source_slot == switch_slot && *actual_group == group
                        && authored_indices.contains(&authored_index))
                }) {
                    produced = true;
                    break;
                }
                assert_eq!(sim.source_arena().handle(switch_slot), Some(switch_handle));
            }
            assert!(
                produced,
                "actual switch {switch_slot} must publish group {group}"
            );
            let trigger = *campaign
                .simulation()
                .economy()
                .terrain_strip_close_triggers()
                .iter()
                .find(|trigger| trigger.authored_index == authored_index)
                .expect("the authored group constructs its actual model-63 command");
            assert_eq!(
                (
                    trigger.class,
                    trigger.model,
                    trigger.action,
                    trigger.position
                ),
                (10, 63, 68, trigger_position)
            );
            let trigger_handle = campaign
                .simulation()
                .source_arena()
                .handle(trigger.source_entity_index)
                .unwrap();
            if trigger.active {
                let _ = campaign.step(crate::PlayerCommand::default(), 0);
                assert!(
                    campaign.simulation().dispatch_events().iter().all(|event| {
                        !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })
                    }),
                    "source opening command must complete without dispatch errors"
                );
            }
            let sim = campaign.simulation();
            let hidden = sim
                .economy()
                .terrain_strip_close_triggers()
                .iter()
                .find(|row| row.source_entity_index == trigger.source_entity_index)
                .expect("command remains through its actual callback epilogue");
            assert!(!hidden.active && !hidden.drawing_enabled);
            assert_eq!(
                sim.source_arena().handle(corridor_slot),
                Some(corridor_handle)
            );
            assert_eq!(
                sim.session.state.live.economy.entities[usize::from(corridor_slot)].life,
                2
            );
            assert!(sim.cf10_production_schedule_receipts().iter().any(|receipt| matches!(
                receipt, crate::campaign::schedule::ScheduleReceipt::CallbackCompleted {
                    slot, selector, ..
                } if *slot == trigger_handle.source_slot && selector.observed_dynamic_cursor_row()
                    .is_some_and(|row| row.source_slot == trigger_handle.source_slot
                        && row.generation == trigger_handle.generation
                        && row.kind == crate::native_world::Level2DynamicArenaKind::TerrainStripCloseTrigger)
            )), "the actual command generation must reach its scheduled consumer");
            let _ = campaign.step(crate::PlayerCommand::default(), 0);
            assert!(
                campaign.simulation().dispatch_events().iter().all(|event| {
                    !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })
                }),
                "source command reclaim must complete without dispatch errors"
            );
            assert!(
                campaign
                    .simulation()
                    .economy()
                    .terrain_strip_close_triggers()
                    .iter()
                    .all(|row| row.source_entity_index != trigger_handle.source_slot)
            );
            assert_ne!(
                campaign
                    .simulation()
                    .source_arena()
                    .handle(trigger_handle.source_slot),
                Some(trigger_handle),
                "the command reclaims on the following source boundary"
            );
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
        // Compare the commanded controller206's two actually produced planes
        // at one cut with the current authored Spider generation. The view is
        // controlled; neither creature nor controller is constructed by the test.
        assert_eq!(actor_campaign.simulation().source_arena().handle(spider.source_slot), Some(spider),
            "the source Spider generation must survive at the comparison cut");
        {
            let controller = &actor_campaign.simulation().entities()[206];
            let position = controller.position;
            let horizontal = controller.source_word_2c() != 0;
            let length = 2 * (i32::from(controller.source_word_96() as i16) + 2);
            let (width, height) = if horizontal { (length, 2) } else { (2, length) };
            let mut cells = Vec::new();
            for dy in 0..height {
                for dx in 0..width {
                    let x = (i32::from(position.x >> 8) - (width >> 1) + dx) as u8;
                    let y = (i32::from(position.y >> 8) - (height >> 1) + dy) as u8;
                    cells.push(usize::from(y) * 256 + usize::from(x));
                }
            }
            let source = actor_campaign.simulation().campaign_entity_presentations(0).unwrap().into_iter()
                .find(|row| row.entity_index == spider.source_slot)
                .expect("the live authored Spider retains its source sprite");
            eprintln!("Cave4 authored Spider600 slot={} generation={} position={:?}; corridor206 position={position:?} envelope={width}x{height}",
                spider.source_slot, spider.generation, source.position);
            let mut base = actor_campaign.clone();
            {
                let planes = &mut base.simulation_mut().session.state.live.terrain.planes;
                for &cell in &cells {
                    planes.ground_heights[cell] = opened_planes.ground_heights[cell];
                    planes.secondary_heights[cell] = opened_planes.secondary_heights[cell];
                    planes.angles[cell] = opened_planes.angles[cell];
                }
            }
            let views = [
                sim_session::CameraPose { x: 52992, y: 26368, z: 2688, yaw: 562, pitch: 0, roll: 0 },
                sim_session::CameraPose { x: 52992, y: 26368, z: 2688, yaw: 562, pitch: 0, roll: 128 },
                sim_session::CameraPose { x: 50432, y: 26368, z: 4096, yaw: 520, pitch: 0, roll: 0 },
            ];
            for (ordinal, pose) in views.into_iter().enumerate() {
                let mut view = base.clone();
                view.simulation_mut().session.state.scratch.source_camera_pose = Some(pose);
                let generation = view.generation();
                let mut carrier = crate::day_indexed_frame::DayIndexedFrameState::new(generation);
                let opened = carrier.render_current(view.inspection(), view.simulation(), &assets, generation)
                    .expect("authored Spider and commanded Cave corridor render");
                assert_eq!(carrier.render_current(view.inspection(), view.simulation(), &assets, generation).unwrap(), opened);
                assert_eq!(opened.source_sprite_stats.reflection_calls, 0);
                assert!(opened.animation_sources().iter().any(|row| row.entity_index == spider.source_slot),
                    "authored Spider must be admitted at {pose:?}");
                let mut without = view.clone();
                let source = view.simulation().campaign_entity_presentations(pose.yaw).unwrap().into_iter()
                    .find(|row| row.entity_index == spider.source_slot).unwrap();
                without.simulation_mut().session.state.live.economy.economy.map_chains.sync_entity(
                    crate::renderable_map_chain::RenderableMapSeed {
                        entity_index: spider.source_slot, active: false,
                        x: source.position.x, y: source.position.y,
                    }).unwrap();
                let mut no_actor = crate::day_indexed_frame::DayIndexedFrameState::new(generation);
                let absent = no_actor.render_current(without.inspection(), without.simulation(), &assets, generation).unwrap();
                let actor_pixels = opened.scene_indices.iter().zip(&absent.scene_indices)
                    .zip(opened.cave_terrain_orders.as_ref().unwrap())
                    .filter(|((with, without), rank)| with != without && **rank != 0).count();
                let actor_rank = opened.cave_cell_orders.as_ref().unwrap()
                    [usize::from(source.position.y >> 8) * 256 + usize::from(source.position.x >> 8)];
                if ordinal < 2 {
                    assert!(actor_rank != 0 && actor_pixels != 0,
                        "authored Spider must draw over Cave terrain at {pose:?}: rank={actor_rank}, pixels={actor_pixels}");
                }
                let mut closed = view.clone();
                let planes = &mut closed.simulation_mut().session.state.live.terrain.planes;
                for &cell in &cells {
                    planes.ground_heights[cell] = closed_planes.ground_heights[cell];
                    planes.secondary_heights[cell] = closed_planes.secondary_heights[cell];
                    planes.angles[cell] = closed_planes.angles[cell];
                }
                let mut closed_carrier = crate::day_indexed_frame::DayIndexedFrameState::new(generation);
                let closed_frame = closed_carrier.render_current(closed.inspection(), closed.simulation(), &assets, generation).unwrap();
                let closed_planes_copy = closed.simulation().session.state.live.terrain.planes.clone();
                without.simulation_mut().session.state.live.terrain.planes = closed_planes_copy;
                let mut closed_no_actor = crate::day_indexed_frame::DayIndexedFrameState::new(generation);
                let closed_absent = closed_no_actor.render_current(without.inspection(), without.simulation(), &assets, generation).unwrap();
                let changed_visibility = opened.scene_indices.iter().zip(&absent.scene_indices)
                    .zip(closed_frame.scene_indices.iter().zip(&closed_absent.scene_indices))
                    .zip(opened.cave_terrain_orders.as_ref().unwrap())
                    .filter(|item| {
                        let (((open, open_without), (shut, shut_without)), rank) = *item;
                        *rank != 0 && (*open != *open_without) != (*shut != *shut_without)
                    }).count();
                if ordinal == 2 {
                    assert!(changed_visibility != 0,
                        "the real corridor opening must change the authored Spider visibility mask at {pose:?}");
                }
            }
        }
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
            source_predicted_position: Default::default(),
            voxel: None,
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
