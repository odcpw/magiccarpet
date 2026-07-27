//! Exact bounded CPU draw-admission feedback for Level 1 Goats.
//!
//! The original renderer sets the selected TMAP descriptor's used bit after
//! terrain-cell traversal, entity flags, fixed-point projection, distance,
//! render-mode, and descriptor-load gates succeed.  It does so before sprite
//! clipping or pixel drawing.  Consequently this module deliberately has no
//! dependency on a viewport, Unity renderer, GPU visibility, or sprite pixels.

use crate::{
    SIN_16_16,
    goat::{GOAT_RENDER_PARAMETER_INDEX, GoatState},
    goat_tmap::GOAT_TMAP_DRAW_MASK,
    render_camera::object_projection_eligible,
    terrain_grid::{
        OUTDOOR_GRID_COLUMNS, OUTDOOR_GRID_ROWS, OutdoorGridVertex, TERRAIN_FEATURE_CULLED,
    },
    terrain_presentation::TERRAIN_RENDER_CUTOFF_SQUARED,
};

pub const GOAT_ORIENTATION_SELECTOR_OFFSETS: [u8; 16] =
    [0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 2, 1, 1, 1, 0, 0];
pub const MAP_TILE_COUNT: usize = 256 * 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GoatDrawCamera {
    pub render_x: u16,
    pub render_y: u16,
    pub yaw: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GoatDrawAdmissionRow {
    pub entity_index: u16,
    pub map_tile_x: u8,
    pub map_tile_y: u8,
    pub active: bool,
    pub flags_allowed: bool,
    pub parameter_matches: bool,
    pub billboard_tile_visited: bool,
    pub lateral: i32,
    pub forward_depth: i32,
    pub distance_squared: u32,
    pub projection_eligible: bool,
    pub orientation_sector: u8,
    pub selector: u16,
    pub descriptor_resident: bool,
    pub admitted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoatDrawAdmission {
    pub selector_mask: u8,
    pub rows: Box<[GoatDrawAdmissionRow]>,
}

/// Build the exact outdoor terrain-cell set whose entity-list heads can reach
/// `DrawSprites_3E360`.
///
/// The 20 by 39 cell traversal uses lattice anchors at rows 1..20 and columns
/// 0..38.  Each admitted anchor reads the entity-list head from a
/// quadrant-dependent neighbor of its terrain sample tile.
#[must_use]
pub fn outdoor_billboard_visit_map(
    lattice: &[OutdoorGridVertex; OUTDOOR_GRID_COLUMNS * OUTDOOR_GRID_ROWS],
    camera_yaw: i16,
) -> Box<[bool; MAP_TILE_COUNT]> {
    let mut visited = Box::new([false; MAP_TILE_COUNT]);
    for row in 1..OUTDOOR_GRID_ROWS {
        for column in 0..(OUTDOOR_GRID_COLUMNS - 1) {
            if let Some((tile_x, tile_y)) =
                outdoor_billboard_tile(lattice, row * OUTDOOR_GRID_COLUMNS + column, camera_yaw)
            {
                visited[usize::from(tile_x) + usize::from(tile_y) * 256] = true;
            }
        }
    }
    visited
}

/// Resolve the source map cell whose entity-list head is visited immediately
/// after one outdoor terrain owner.
#[must_use]
pub fn outdoor_billboard_tile(
    lattice: &[OutdoorGridVertex; OUTDOOR_GRID_COLUMNS * OUTDOOR_GRID_ROWS],
    owner_index: usize,
    camera_yaw: i16,
) -> Option<(u8, u8)> {
    if owner_index < OUTDOOR_GRID_COLUMNS
        || owner_index >= OUTDOOR_GRID_COLUMNS * OUTDOOR_GRID_ROWS
        || owner_index % OUTDOOR_GRID_COLUMNS >= OUTDOOR_GRID_COLUMNS - 1
    {
        return None;
    }
    let vertex = lattice[owner_index];
    if vertex.feature_flags & TERRAIN_FEATURE_CULLED != 0 || vertex.pnt5.is_none() {
        return None;
    }
    let yaw = i32::from(camera_yaw) & 0x07ff;
    let quadrant = usize::try_from(((yaw + 256) >> 9) & 3).expect("quadrant fits usize");
    // GameRenderHD's 10-byte quadrant row contributes both the terrain
    // texture offsets and the entity-head offsets.  The combined source cell
    // deltas are therefore not the four cardinal neighbours.
    let (tile_dx, tile_dy) = match quadrant {
        0 => (0_i8, -1_i8),
        1 => (0, 0),
        2 => (-1, 0),
        3 => (-1, -1),
        _ => unreachable!("quadrant is masked to two bits"),
    };
    Some((
        vertex.tile_x.wrapping_add_signed(tile_dx),
        vertex.tile_y.wrapping_add_signed(tile_dy),
    ))
}

/// Evaluate the source CPU gates and produce the selector-global used mask.
#[must_use]
pub fn evaluate_goat_draw_admission(
    goats: &[GoatState],
    camera: GoatDrawCamera,
    visited_billboard_tiles: &[bool; MAP_TILE_COUNT],
    descriptors_resident: bool,
) -> GoatDrawAdmission {
    let yaw = usize::try_from(i32::from(camera.yaw) & 0x07ff).expect("yaw fits usize");
    let sin = SIN_16_16[yaw];
    let cos = SIN_16_16[512 + yaw];
    let mut selector_mask = 0_u8;
    let mut rows = Vec::with_capacity(goats.len());

    for goat in goats {
        let map_tile_x = goat.position.x.to_le_bytes()[1];
        let map_tile_y = goat.position.y.to_le_bytes()[1];
        let billboard_tile_visited =
            visited_billboard_tiles[usize::from(map_tile_x) + usize::from(map_tile_y) * 256];
        let dx = i32::from(goat.position.x.wrapping_sub(camera.render_x).cast_signed());
        let dy = i32::from(camera.render_y.wrapping_sub(goat.position.y).cast_signed());
        let lateral = cos.wrapping_mul(dx).wrapping_sub(sin.wrapping_mul(dy)) >> 16;
        let forward_depth = sin.wrapping_mul(dx).wrapping_add(cos.wrapping_mul(dy)) >> 16;
        let distance_squared = lateral
            .wrapping_mul(lateral)
            .wrapping_add(forward_depth.wrapping_mul(forward_depth))
            .cast_unsigned();
        let projection_eligible = object_projection_eligible(
            forward_depth,
            distance_squared,
            TERRAIN_RENDER_CUTOFF_SQUARED,
        );
        let orientation_sector = u8::try_from(
            (((i32::from(goat.yaw) - i32::from(camera.yaw.cast_unsigned())) >> 3) & 0x00f0) >> 4,
        )
        .expect("masked orientation sector fits u8");
        let selector =
            152 + u16::from(GOAT_ORIENTATION_SELECTOR_OFFSETS[usize::from(orientation_sector)]);
        let active = goat.active;
        let flags_allowed = goat.control_flags & 0x21 == 0;
        let parameter_matches = goat.render_parameter_index == GOAT_RENDER_PARAMETER_INDEX;
        let descriptor_resident = descriptors_resident;
        let admitted = active
            && flags_allowed
            && parameter_matches
            && billboard_tile_visited
            && projection_eligible
            && descriptor_resident;
        if admitted {
            selector_mask |= 1 << (selector - 152);
        }
        rows.push(GoatDrawAdmissionRow {
            entity_index: goat.entity_index,
            map_tile_x,
            map_tile_y,
            active,
            flags_allowed,
            parameter_matches,
            billboard_tile_visited,
            lateral,
            forward_depth,
            distance_squared,
            projection_eligible,
            orientation_sector,
            selector,
            descriptor_resident,
            admitted,
        });
    }

    debug_assert_eq!(selector_mask & !GOAT_TMAP_DRAW_MASK, 0);
    GoatDrawAdmission {
        selector_mask,
        rows: rows.into_boxed_slice(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Position;

    fn goat(entity_index: u16, position: Position, yaw: i16) -> GoatState {
        GoatState {
            entity_index,
            source_id: entity_index,
            active: true,
            control_flags: 12,
            max_life: 100,
            life: 100,
            pending_damage: 0,
            pending_attacker: -1,
            kill_credit_entity: -1,
            current_attacker: -1,
            rng: 0,
            yaw,
            pitch: 0,
            roll: 0,
            position,
            collision_half_width: 0,
            collision_vertical_offset: 0,
            collision_half_height: 0,
            render_parameter_index: GOAT_RENDER_PARAMETER_INDEX,
            animation_frame: 0,
            action: 15,
            phase: 0,
            proximity_countdown: 0,
            proximity_subcounter: 0,
            stage_var_1: 0,
            stage_var_2: 2,
            actual_speed: 0,
            minimum_speed: 0,
            maximum_speed: 0,
            mana: 0,
            target_entity_index: None,
        }
    }

    fn all_tiles() -> Box<[bool; MAP_TILE_COUNT]> {
        Box::new([true; MAP_TILE_COUNT])
    }

    #[test]
    fn all_sixteen_orientation_sectors_map_to_the_exact_three_selectors() {
        let camera = GoatDrawCamera {
            render_x: 0x8000,
            render_y: 0x8000,
            yaw: 0,
        };
        for (sector, expected_offset) in GOAT_ORIENTATION_SELECTOR_OFFSETS.into_iter().enumerate() {
            let result = evaluate_goat_draw_admission(
                &[goat(
                    50,
                    Position {
                        x: 0x8000,
                        y: 0x7f00,
                        z: 0,
                    },
                    i16::try_from(sector * 128).expect("sector yaw fits i16"),
                )],
                camera,
                &all_tiles(),
                true,
            );
            assert_eq!(result.rows[0].orientation_sector, sector as u8);
            assert_eq!(result.rows[0].selector, 152 + u16::from(expected_offset));
            assert_eq!(result.selector_mask, 1 << expected_offset);
        }
    }

    #[test]
    fn repeated_and_mixed_goats_collapse_to_a_three_bit_selector_mask() {
        let camera = GoatDrawCamera {
            render_x: 0x8000,
            render_y: 0x8000,
            yaw: 0,
        };
        let position = Position {
            x: 0x8000,
            y: 0x7f00,
            z: 0,
        };
        let result = evaluate_goat_draw_admission(
            &[
                goat(50, position, 0),
                goat(79, position, 128),
                goat(81, position, 384),
                goat(82, position, 768),
            ],
            camera,
            &all_tiles(),
            true,
        );
        assert_eq!(result.selector_mask, 0b111);
        assert_eq!(
            result
                .rows
                .iter()
                .map(|row| row.selector)
                .collect::<Vec<_>>(),
            [152, 152, 153, 154]
        );
    }

    #[test]
    fn source_gates_reject_flags_depth_tile_parameter_and_missing_descriptors() {
        let camera = GoatDrawCamera {
            render_x: 0x8000,
            render_y: 0x8000,
            yaw: 0,
        };
        let mut rows = vec![
            goat(
                50,
                Position {
                    x: 0x8000,
                    y: 0x7fbf,
                    z: 0,
                },
                0,
            ),
            goat(
                79,
                Position {
                    x: 0x8000,
                    y: 0x7f00,
                    z: 0,
                },
                0,
            ),
            goat(
                81,
                Position {
                    x: 0x8100,
                    y: 0x7f00,
                    z: 0,
                },
                0,
            ),
        ];
        rows[1].control_flags |= 0x01;
        rows[2].render_parameter_index = 0;
        let mut visited = Box::new([false; MAP_TILE_COUNT]);
        visited[0x81 + 0x7f * 256] = true;
        let result = evaluate_goat_draw_admission(&rows, camera, &visited, true);
        assert_eq!(result.selector_mask, 0);
        assert_eq!(result.rows[0].forward_depth, 65);
        assert!(!result.rows[0].billboard_tile_visited);
        assert!(!result.rows[1].flags_allowed);
        assert!(!result.rows[2].parameter_matches);

        let missing = evaluate_goat_draw_admission(
            &[goat(50, rows[1].position, 0)],
            camera,
            &all_tiles(),
            false,
        );
        assert!(!missing.rows[0].descriptor_resident);
        assert_eq!(missing.selector_mask, 0);
    }

    #[test]
    fn billboard_visit_map_applies_quadrant_offsets_boundaries_cull_and_wrap() {
        for (yaw, expected) in [
            (0, (7_u8, 8_u8)),
            (512, (7, 9)),
            (1024, (6, 9)),
            (1536, (6, 8)),
        ] {
            let mut lattice =
                [OutdoorGridVertex::default(); OUTDOOR_GRID_COLUMNS * OUTDOOR_GRID_ROWS];
            let admitted_index = OUTDOOR_GRID_COLUMNS + 1;
            lattice[admitted_index].tile_x = 7;
            lattice[admitted_index].tile_y = 9;
            lattice[admitted_index].pnt5 = Some(1);
            let boundary = 1;
            lattice[boundary].tile_x = 20;
            lattice[boundary].tile_y = 20;
            lattice[boundary].pnt5 = Some(1);
            let culled = OUTDOOR_GRID_COLUMNS + 2;
            lattice[culled].tile_x = 30;
            lattice[culled].tile_y = 30;
            lattice[culled].pnt5 = Some(1);
            lattice[culled].feature_flags = TERRAIN_FEATURE_CULLED;
            let visited = outdoor_billboard_visit_map(&lattice, yaw);
            assert!(visited[usize::from(expected.0) + usize::from(expected.1) * 256]);
            assert!(!visited[20 + 20 * 256]);
            assert!(!visited[30 + 30 * 256]);
        }

        let mut lattice = [OutdoorGridVertex::default(); OUTDOOR_GRID_COLUMNS * OUTDOOR_GRID_ROWS];
        lattice[OUTDOOR_GRID_COLUMNS].tile_x = 0;
        lattice[OUTDOOR_GRID_COLUMNS].tile_y = 0;
        lattice[OUTDOOR_GRID_COLUMNS].pnt5 = Some(1);
        let visited = outdoor_billboard_visit_map(&lattice, 0);
        assert!(visited[0 + 255 * 256]);
    }
}
