//! Source-shaped indexed MAIN post-processing for the fixed 320x200 profile.
//!
//! `DrawWorld_411A0` either draws once to MAIN and optionally applies the
//! in-place 2x2 smoother, or draws once to the scratch plane and blends it
//! against the previously finalized MAIN through TABLESN.  The configured
//! mode and the effective mode are deliberately separate: configured mode 2
//! is not subject to the automatic mode-1 menu and viewport gates.

use crate::terrain_frame::{LEVEL1_VIEW_HEIGHT, LEVEL1_VIEW_PIXELS, LEVEL1_VIEW_WIDTH};

pub const SOURCE_WORLD_PROVENANCE: u8 = 1 << 0;
pub const PRIOR_MAIN_PROVENANCE: u8 = 1 << 1;
pub const SPATIAL_PROVENANCE: u8 = 1 << 2;
pub const OVERLAY_PROVENANCE: u8 = 1 << 3;
pub const TABLESN_BLEND_OFFSET: usize = 0x4000;
pub const TABLESN_REQUIRED_BYTES: usize = TABLESN_BLEND_OFFSET + 0x1_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexedPostprocessInput {
    pub configured_mode: u8,
    pub blur_capability: bool,
    pub high_speed_system: bool,
    pub scratch_present: bool,
    pub vga_resolution: u8,
    pub menu_state: u8,
    pub viewport_width: u16,
    pub viewport_height: u16,
    pub screen_stride: u16,
    pub actual_speed: i16,
    pub spatial_smoothing: bool,
}

impl Default for IndexedPostprocessInput {
    fn default() -> Self {
        Self {
            configured_mode: 0,
            blur_capability: false,
            high_speed_system: true,
            scratch_present: true,
            vga_resolution: 1,
            menu_state: 0,
            viewport_width: LEVEL1_VIEW_WIDTH as u16,
            viewport_height: LEVEL1_VIEW_HEIGHT as u16,
            screen_stride: LEVEL1_VIEW_WIDTH as u16,
            actual_speed: 0,
            spatial_smoothing: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexedPostprocessBranch {
    Direct { spatial: bool },
    Temporal { mode: u8 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexedPostprocessReceipt {
    pub input_hash: u64,
    pub current_world_hash: u64,
    pub prior_main_hash: u64,
    pub output_hash: u64,
    pub provenance_hash: u64,
    pub output_bytes: u32,
    pub branch: IndexedPostprocessBranch,
}

impl IndexedPostprocessReceipt {
    #[must_use]
    pub fn matches(
        self,
        input: IndexedPostprocessInput,
        current_world_hash: u64,
        indices: &[u8],
        provenance: &[u8],
    ) -> bool {
        self.input_hash == postprocess_input_hash(input)
            && self.current_world_hash == current_world_hash
            && self.output_bytes == LEVEL1_VIEW_PIXELS as u32
            && self.output_hash == byte_hash(indices)
            && self.provenance_hash == byte_hash(provenance)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedPostprocessOutput {
    pub indices: Vec<u8>,
    pub provenance: Vec<u8>,
    pub branch: IndexedPostprocessBranch,
    pub receipt: IndexedPostprocessReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedPostprocessError(pub String);

impl std::fmt::Display for IndexedPostprocessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for IndexedPostprocessError {}

#[must_use]
pub fn effective_postprocess_mode(input: IndexedPostprocessInput) -> u8 {
    if input.configured_mode != 0 {
        return input.configured_mode;
    }
    u8::from(
        input.vga_resolution == 1
            && input.menu_state == 0
            && input.high_speed_system
            && input.scratch_present
            && input.viewport_width == LEVEL1_VIEW_WIDTH as u16
            && input.actual_speed.unsigned_abs() > 80,
    )
}

/// Finalize the world plane through the source TABLESN or spatial path.
///
/// `current` is the one and only world render for this presentation request.
/// Temporal calls require a finalized prior MAIN; direct calls ignore it.
pub fn finalize_indexed_world(
    input: IndexedPostprocessInput,
    current: &[u8],
    current_provenance: &[u8],
    prior_main: Option<&[u8]>,
    prior_provenance: Option<&[u8]>,
    tablesn: &[u8],
) -> Result<IndexedPostprocessOutput, IndexedPostprocessError> {
    validate_plane("current world", current)?;
    validate_plane("current provenance", current_provenance)?;
    if input.configured_mode > 2 {
        return Err(IndexedPostprocessError(format!(
            "unsupported configured blur mode {}",
            input.configured_mode
        )));
    }
    if input.viewport_width > LEVEL1_VIEW_WIDTH as u16
        || input.viewport_height > LEVEL1_VIEW_HEIGHT as u16
        || input.screen_stride < input.viewport_width
        || usize::from(input.screen_stride) * usize::from(input.viewport_height)
            > LEVEL1_VIEW_PIXELS
    {
        return Err(IndexedPostprocessError(
            "postprocess viewport/stride exceeds fixed MAIN".to_owned(),
        ));
    }

    let effective_mode = effective_postprocess_mode(input);
    let temporal = input.blur_capability && effective_mode != 0 && input.scratch_present;
    if temporal {
        if tablesn.len() < TABLESN_REQUIRED_BYTES {
            return Err(IndexedPostprocessError(format!(
                "TABLESN has {} bytes, expected at least {TABLESN_REQUIRED_BYTES}",
                tablesn.len()
            )));
        }
        let prior_main = prior_main.ok_or_else(|| {
            IndexedPostprocessError("temporal postprocess requires primed MAIN".to_owned())
        })?;
        let prior_provenance = prior_provenance.ok_or_else(|| {
            IndexedPostprocessError("temporal postprocess requires primed provenance".to_owned())
        })?;
        validate_plane("prior MAIN", prior_main)?;
        validate_plane("prior provenance", prior_provenance)?;
        let mut indices = prior_main.to_vec();
        let mut provenance = prior_provenance.to_vec();
        let width = usize::from(input.viewport_width);
        let height = usize::from(input.viewport_height);
        let stride = usize::from(input.screen_stride);
        let dword_width = width / 4 * 4;
        for row in 0..height {
            let row_start = row * stride;
            for column in 0..dword_width {
                let index = row_start + column;
                // Source mode 1 reverses the operands for the first dword of
                // the complete pass only.  All later dwords, and all of mode
                // 2, use scratch as the high byte.
                let reverse = effective_mode == 1 && row == 0 && column < 4;
                let current_index = usize::from(current[index]);
                let prior_index = usize::from(prior_main[index]);
                let pair = if reverse {
                    (prior_index << 8) | current_index
                } else {
                    (current_index << 8) | prior_index
                };
                indices[index] = tablesn[TABLESN_BLEND_OFFSET + pair];
                provenance[index] = SOURCE_WORLD_PROVENANCE | PRIOR_MAIN_PROVENANCE;
            }
        }
        let branch = IndexedPostprocessBranch::Temporal {
            mode: effective_mode,
        };
        let receipt = IndexedPostprocessReceipt {
            input_hash: postprocess_input_hash(input),
            current_world_hash: byte_hash(current),
            prior_main_hash: byte_hash(prior_main),
            output_hash: byte_hash(&indices),
            provenance_hash: byte_hash(&provenance),
            output_bytes: LEVEL1_VIEW_PIXELS as u32,
            branch,
        };
        return Ok(IndexedPostprocessOutput {
            indices,
            provenance,
            branch,
            receipt,
        });
    }

    let mut indices = current.to_vec();
    let mut provenance = current_provenance
        .iter()
        .map(|value| value | SOURCE_WORLD_PROVENANCE)
        .collect::<Vec<_>>();
    if input.spatial_smoothing {
        spatial_smooth_in_place(
            &mut indices,
            &mut provenance,
            usize::from(input.viewport_width),
            usize::from(input.viewport_height),
            usize::from(input.screen_stride),
            tablesn,
        )?;
    }
    let branch = IndexedPostprocessBranch::Direct {
        spatial: input.spatial_smoothing,
    };
    let receipt = IndexedPostprocessReceipt {
        input_hash: postprocess_input_hash(input),
        current_world_hash: byte_hash(current),
        prior_main_hash: 0,
        output_hash: byte_hash(&indices),
        provenance_hash: byte_hash(&provenance),
        output_bytes: LEVEL1_VIEW_PIXELS as u32,
        branch,
    };
    Ok(IndexedPostprocessOutput {
        indices,
        provenance,
        branch,
        receipt,
    })
}

#[must_use]
pub fn postprocess_input_hash(input: IndexedPostprocessInput) -> u64 {
    let mut bytes = Vec::with_capacity(32);
    bytes.extend_from_slice(b"mc2-indexed-postprocess-input-v1\0");
    bytes.extend_from_slice(&[
        input.configured_mode,
        u8::from(input.blur_capability),
        u8::from(input.high_speed_system),
        u8::from(input.scratch_present),
        input.vga_resolution,
        input.menu_state,
        u8::from(input.spatial_smoothing),
    ]);
    bytes.extend_from_slice(&input.viewport_width.to_le_bytes());
    bytes.extend_from_slice(&input.viewport_height.to_le_bytes());
    bytes.extend_from_slice(&input.screen_stride.to_le_bytes());
    bytes.extend_from_slice(&input.actual_speed.to_le_bytes());
    byte_hash(&bytes)
}

#[must_use]
pub fn byte_hash(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |mut hash, byte| {
            hash ^= u64::from(*byte);
            hash.wrapping_mul(0x0000_0100_0000_01b3)
        })
}

fn spatial_smooth_in_place(
    indices: &mut [u8],
    provenance: &mut [u8],
    width: usize,
    height: usize,
    stride: usize,
    tablesn: &[u8],
) -> Result<(), IndexedPostprocessError> {
    if tablesn.len() < TABLESN_REQUIRED_BYTES {
        return Err(IndexedPostprocessError(format!(
            "TABLESN has {} bytes, expected at least {TABLESN_REQUIRED_BYTES} for spatial smoothing",
            tablesn.len()
        )));
    }
    if width < 2 || height < 2 {
        return Ok(());
    }
    for row in 0..height - 1 {
        for column in 0..width - 1 {
            let top_left = row * stride + column;
            let top_right = top_left + 1;
            let bottom_left = top_left + stride;
            let bottom_right = bottom_left + 1;
            let upper = tablesn[TABLESN_BLEND_OFFSET
                + (usize::from(indices[bottom_left]) << 8)
                + usize::from(indices[top_left])];
            let lower = tablesn[TABLESN_BLEND_OFFSET
                + (usize::from(indices[bottom_right]) << 8)
                + usize::from(indices[top_right])];
            indices[top_left] =
                tablesn[TABLESN_BLEND_OFFSET + (usize::from(lower) << 8) + usize::from(upper)];
            provenance[top_left] |= SPATIAL_PROVENANCE;
        }
    }
    Ok(())
}

fn validate_plane(name: &str, plane: &[u8]) -> Result<(), IndexedPostprocessError> {
    if plane.len() != LEVEL1_VIEW_PIXELS {
        return Err(IndexedPostprocessError(format!(
            "{name} has {} bytes, expected {LEVEL1_VIEW_PIXELS}",
            plane.len()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asymmetric_tablesn() -> Vec<u8> {
        let mut tablesn = vec![0_u8; TABLESN_REQUIRED_BYTES];
        for high in 0..=u8::MAX {
            for low in 0..=u8::MAX {
                let pair = (usize::from(high) << 8) | usize::from(low);
                tablesn[TABLESN_BLEND_OFFSET + pair] =
                    high.wrapping_mul(3).wrapping_add(low.wrapping_mul(7));
                tablesn[pair] = high ^ low.rotate_left(1);
            }
        }
        tablesn
    }

    #[test]
    fn temporal_modes_pin_source_gates_vectors_asymmetry_stride_and_tail() {
        let tablesn = asymmetric_tablesn();
        let mut current = vec![12; LEVEL1_VIEW_PIXELS];
        let prior = vec![77; LEVEL1_VIEW_PIXELS];
        let current_provenance = vec![SOURCE_WORLD_PROVENANCE; LEVEL1_VIEW_PIXELS];
        let prior_provenance = vec![OVERLAY_PROVENANCE; LEVEL1_VIEW_PIXELS];
        // Hard source-derived TABLESN vectors.
        let mut source_tablesn = tablesn.clone();
        source_tablesn[TABLESN_BLEND_OFFSET + (77 << 8) + 12] = 63;
        source_tablesn[TABLESN_BLEND_OFFSET + (12 << 8) + 77] = 195;

        for speed in [80, -80] {
            assert_eq!(
                effective_postprocess_mode(IndexedPostprocessInput {
                    actual_speed: speed,
                    ..IndexedPostprocessInput::default()
                }),
                0
            );
        }
        for speed in [81, -81] {
            assert_eq!(
                effective_postprocess_mode(IndexedPostprocessInput {
                    actual_speed: speed,
                    ..IndexedPostprocessInput::default()
                }),
                1
            );
        }

        let mode1 = finalize_indexed_world(
            IndexedPostprocessInput {
                configured_mode: 1,
                blur_capability: true,
                actual_speed: 81,
                viewport_width: 6,
                viewport_height: 2,
                screen_stride: 8,
                ..IndexedPostprocessInput::default()
            },
            &current,
            &current_provenance,
            Some(&prior),
            Some(&prior_provenance),
            &source_tablesn,
        )
        .expect("mode 1");
        assert_eq!(&mode1.indices[0..4], &[63; 4]);
        assert_eq!(mode1.indices[4], 77, "sub-dword viewport tail is retained");
        assert_eq!(mode1.indices[8], 195, "later rows use normal operands");
        assert_eq!(
            mode1.provenance[0],
            SOURCE_WORLD_PROVENANCE | PRIOR_MAIN_PROVENANCE
        );
        assert_eq!(
            mode1.provenance[4], OVERLAY_PROVENANCE,
            "retained pixels keep their actual prior provenance"
        );

        current[0] = 77;
        let configured_mode2 = finalize_indexed_world(
            IndexedPostprocessInput {
                configured_mode: 2,
                blur_capability: true,
                menu_state: 5,
                viewport_width: 4,
                viewport_height: 1,
                screen_stride: 8,
                actual_speed: 0,
                ..IndexedPostprocessInput::default()
            },
            &current,
            &current_provenance,
            Some(&prior),
            Some(&prior_provenance),
            &source_tablesn,
        )
        .expect("configured mode 2 ignores auto-mode gates");
        assert_eq!(configured_mode2.indices[0], source_tablesn[0x4000 + 0x4d4d]);
        assert_eq!(
            configured_mode2.branch,
            IndexedPostprocessBranch::Temporal { mode: 2 }
        );

        for input in [
            IndexedPostprocessInput {
                blur_capability: false,
                actual_speed: 81,
                ..IndexedPostprocessInput::default()
            },
            IndexedPostprocessInput {
                high_speed_system: false,
                actual_speed: 81,
                ..IndexedPostprocessInput::default()
            },
            IndexedPostprocessInput {
                scratch_present: false,
                actual_speed: 81,
                ..IndexedPostprocessInput::default()
            },
            IndexedPostprocessInput {
                menu_state: 5,
                actual_speed: 81,
                ..IndexedPostprocessInput::default()
            },
            IndexedPostprocessInput {
                viewport_width: 319,
                actual_speed: 81,
                ..IndexedPostprocessInput::default()
            },
        ] {
            assert!(matches!(
                finalize_indexed_world(
                    input,
                    &current,
                    &current_provenance,
                    Some(&prior),
                    Some(&prior_provenance),
                    &source_tablesn
                )
                .expect("negative"),
                IndexedPostprocessOutput {
                    branch: IndexedPostprocessBranch::Direct { .. },
                    ..
                }
            ));
        }
    }

    #[test]
    fn spatial_path_is_exact_and_exclusive_with_temporal_history() {
        let tablesn = asymmetric_tablesn();
        let mut current = vec![0; LEVEL1_VIEW_PIXELS];
        current[0] = 1;
        current[1] = 2;
        current[LEVEL1_VIEW_WIDTH] = 3;
        current[LEVEL1_VIEW_WIDTH + 1] = 4;
        let provenance = vec![0; LEVEL1_VIEW_PIXELS];
        let direct = finalize_indexed_world(
            IndexedPostprocessInput {
                viewport_width: 2,
                viewport_height: 2,
                spatial_smoothing: true,
                ..IndexedPostprocessInput::default()
            },
            &current,
            &provenance,
            None,
            None,
            &tablesn,
        )
        .expect("spatial");
        let upper = tablesn[TABLESN_BLEND_OFFSET + 0x0301];
        let lower = tablesn[TABLESN_BLEND_OFFSET + 0x0402];
        assert_eq!(
            direct.indices[0],
            tablesn[TABLESN_BLEND_OFFSET + (usize::from(lower) << 8) + usize::from(upper)]
        );
        assert_eq!(
            direct.provenance[0],
            SOURCE_WORLD_PROVENANCE | SPATIAL_PROVENANCE
        );

        let temporal = finalize_indexed_world(
            IndexedPostprocessInput {
                configured_mode: 2,
                blur_capability: true,
                spatial_smoothing: true,
                viewport_width: 4,
                viewport_height: 1,
                ..IndexedPostprocessInput::default()
            },
            &current,
            &provenance,
            Some(&vec![0; LEVEL1_VIEW_PIXELS]),
            Some(&vec![0; LEVEL1_VIEW_PIXELS]),
            &tablesn,
        )
        .expect("temporal is exclusive");
        assert_eq!(
            temporal.provenance[0] & SPATIAL_PROVENANCE,
            0,
            "temporal jump skips the spatial path"
        );
    }

    #[test]
    fn temporal_requires_primed_main_and_uses_additive_tablesn_offset() {
        let mut tablesn = vec![0_u8; TABLESN_REQUIRED_BYTES];
        let current = vec![0x40; LEVEL1_VIEW_PIXELS];
        let prior = vec![0; LEVEL1_VIEW_PIXELS];
        let provenance = vec![0; LEVEL1_VIEW_PIXELS];
        let input = IndexedPostprocessInput {
            configured_mode: 2,
            blur_capability: true,
            viewport_width: 4,
            viewport_height: 1,
            ..IndexedPostprocessInput::default()
        };
        assert!(
            finalize_indexed_world(input, &current, &provenance, None, None, &tablesn)
                .expect_err("unprimed")
                .0
                .contains("primed MAIN")
        );
        let pair = 0x4000;
        tablesn[TABLESN_BLEND_OFFSET + pair] = 211;
        tablesn[TABLESN_BLEND_OFFSET | pair] = 73;
        let output = finalize_indexed_world(
            input,
            &current,
            &provenance,
            Some(&prior),
            Some(&provenance),
            &tablesn,
        )
        .expect("primed");
        assert_eq!(output.indices[0], 211, "TABLESN offset is addition, not OR");
    }
}
