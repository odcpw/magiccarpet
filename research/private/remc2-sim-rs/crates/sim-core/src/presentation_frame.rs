//! Revisioned FINAL-MAIN ownership for the qualified fixed presentation.
//!
//! A successful contiguous request advances MAIN exactly once.  Identical
//! repeats return immutable cached bytes, while stale, out-of-order, or
//! same-tick changed inputs reject before the world renderer is invoked.

use crate::{
    dynamic_lighting::DynamicLightingExecutionReceipt,
    indexed_overlay::{
        IndexedOverlayAssets, IndexedOverlayError, IndexedOverlayReceipt, Level1FinalMainState,
    },
    indexed_postprocess::{
        IndexedPostprocessBranch, IndexedPostprocessError, IndexedPostprocessInput,
        IndexedPostprocessReceipt, SOURCE_WORLD_PROVENANCE, finalize_indexed_world,
        postprocess_input_hash,
    },
    terrain_frame::{LEVEL1_VIEW_PIXELS, SourceRenderExecutionReceipt},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainHistory {
    Unprimed,
    Primed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRenderedFrame {
    pub indices: Vec<u8>,
    /// Backing produced by the map branch's actual fixed 128x200 second
    /// `DrawWorld` call. Ordinary/dead branches leave this absent.
    pub map_world: Option<Vec<u8>>,
    pub receipt: SourceRenderExecutionReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationFrameInput {
    pub generation: u64,
    pub tick: u64,
    pub presentation_revision: u64,
    pub state_hash: u64,
    pub source_arena_hash: u64,
    /// Immutable digest of every source-world renderer input that is not
    /// already covered by the flight/source-arena fields.  This deliberately
    /// excludes post-draw selector latches: those are feedback, not pixels
    /// consumed by the current render.
    pub source_render_input_hash: u64,
    pub lighting_hash: u64,
    pub camera_hash: u64,
    pub source_reflections_expected: bool,
    pub postprocess: IndexedPostprocessInput,
    pub overlay: Level1FinalMainState,
    pub lighting_receipt: Option<DynamicLightingExecutionReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationFrame {
    pub generation: u64,
    pub tick: u64,
    pub presentation_revision: u64,
    pub immutable_input_hash: u64,
    pub indices: Vec<u8>,
    pub provenance: Vec<u8>,
    pub index_hash: u64,
    pub provenance_hash: u64,
    pub postprocess_branch: IndexedPostprocessBranch,
    pub overlay_receipts: Vec<IndexedOverlayReceipt>,
    pub overlay_pixels_written: u32,
    pub help_sentinel: u8,
    pub source_receipt: SourceRenderExecutionReceipt,
    pub lighting_receipt: DynamicLightingExecutionReceipt,
    pub postprocess_receipt: IndexedPostprocessReceipt,
    pub overlay_command_hash: u64,
    pub expected_overlay_hash: u64,
    pub expected_overlay_stages: Vec<crate::indexed_overlay::IndexedOverlayStage>,
    pub postprocess_input_hash: u64,
    pub postprocess_index_hash: u64,
    pub postprocess_provenance_hash: u64,
    pub lighting_hash: u64,
    pub source_reflections_expected: bool,
}

impl PresentationFrame {
    #[must_use]
    pub fn has_provenance(&self) -> bool {
        self.provenance.len() == LEVEL1_VIEW_PIXELS
            && byte_hash(&self.provenance) == self.provenance_hash
    }

    #[must_use]
    pub fn source_complete(&self) -> bool {
        self.source_receipt.is_complete()
            && self.source_receipt.index_hash == self.postprocess_receipt.current_world_hash
            && self.source_receipt.reflected_sprites_composited == self.source_reflections_expected
    }

    #[must_use]
    pub fn lighting_complete(&self) -> bool {
        self.lighting_receipt.is_complete_for(self.lighting_hash)
    }

    #[must_use]
    pub fn postprocess_complete(&self) -> bool {
        self.postprocess_receipt.input_hash == self.postprocess_input_hash
            && self.postprocess_receipt.output_hash == self.postprocess_index_hash
            && self.postprocess_receipt.provenance_hash == self.postprocess_provenance_hash
            && self.postprocess_receipt.output_bytes == LEVEL1_VIEW_PIXELS as u32
    }

    #[must_use]
    pub fn overlays_complete(&self) -> bool {
        self.overlay_command_hash == self.expected_overlay_hash
            && self.overlay_receipts.len() == self.expected_overlay_stages.len()
            && self
                .overlay_receipts
                .iter()
                .zip(&self.expected_overlay_stages)
                .all(|(receipt, expected)| receipt.stage == *expected && receipt.command_count > 0)
    }

    #[must_use]
    pub fn compositor_complete(&self) -> bool {
        self.source_complete()
            && self.lighting_complete()
            && self.postprocess_complete()
            && self.overlays_complete()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationFrameResult {
    pub frame: PresentationFrame,
    pub newly_committed: bool,
}

#[derive(Clone, Debug)]
pub struct PresentationFrameState {
    generation: u64,
    history: MainHistory,
    cached: Option<PresentationFrame>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationFrameError(pub String);

impl std::fmt::Display for PresentationFrameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PresentationFrameError {}

impl From<IndexedPostprocessError> for PresentationFrameError {
    fn from(error: IndexedPostprocessError) -> Self {
        Self(error.0)
    }
}

impl From<IndexedOverlayError> for PresentationFrameError {
    fn from(error: IndexedOverlayError) -> Self {
        Self(error.0)
    }
}

impl PresentationFrameState {
    #[must_use]
    pub const fn new(generation: u64) -> Self {
        Self {
            generation,
            history: MainHistory::Unprimed,
            cached: None,
        }
    }

    pub fn reset(&mut self, generation: u64) {
        self.generation = generation;
        self.history = MainHistory::Unprimed;
        self.cached = None;
    }

    #[must_use]
    pub const fn history(&self) -> MainHistory {
        self.history
    }

    #[must_use]
    pub fn cached(&self) -> Option<&PresentationFrame> {
        self.cached.as_ref()
    }

    pub fn finalize<F>(
        &mut self,
        mut input: PresentationFrameInput,
        tablesn: &[u8],
        overlays: &IndexedOverlayAssets,
        render_world: F,
    ) -> Result<PresentationFrameResult, PresentationFrameError>
    where
        F: FnOnce() -> Result<SourceRenderedFrame, PresentationFrameError>,
    {
        if input.generation != self.generation {
            return Err(PresentationFrameError(format!(
                "presentation generation {} does not match {}",
                input.generation, self.generation
            )));
        }
        let expected_tick = self.cached.as_ref().map_or(0, |frame| frame.tick + 1);
        input.overlay.prior_help_sentinel =
            self.cached.as_ref().map_or(0, |frame| frame.help_sentinel);
        let immutable_input_hash = immutable_input_hash(&input);

        if let Some(cached) = self.cached.as_ref() {
            if input.tick == cached.tick {
                if input.presentation_revision == cached.presentation_revision
                    && immutable_input_hash == cached.immutable_input_hash
                {
                    return Ok(PresentationFrameResult {
                        frame: cached.clone(),
                        newly_committed: false,
                    });
                }
                return Err(PresentationFrameError(
                    "same finalized tick has changed presentation revision or input hash"
                        .to_owned(),
                ));
            }
            if input.tick < cached.tick {
                return Err(PresentationFrameError(format!(
                    "stale presentation tick {}, finalized {}",
                    input.tick, cached.tick
                )));
            }
        }
        if input.tick != expected_tick {
            return Err(PresentationFrameError(format!(
                "out-of-order presentation tick {}, expected {expected_tick}",
                input.tick
            )));
        }
        let lighting_receipt = input.lighting_receipt.ok_or_else(|| {
            PresentationFrameError(
                "qualified FINAL MAIN requires an executed dynamic-lighting receipt".to_owned(),
            )
        })?;
        if !lighting_receipt.is_complete_for(input.lighting_hash) {
            return Err(PresentationFrameError(
                "dynamic-lighting execution receipt does not match live state".to_owned(),
            ));
        }

        let rendered = render_world()?;
        if rendered.indices.len() != LEVEL1_VIEW_PIXELS {
            return Err(PresentationFrameError(format!(
                "world renderer produced {} bytes, expected {LEVEL1_VIEW_PIXELS}",
                rendered.indices.len()
            )));
        }
        if !rendered.receipt.is_complete()
            || rendered.receipt.index_hash != byte_hash(&rendered.indices)
            || rendered.receipt.reflected_sprites_composited != input.source_reflections_expected
        {
            return Err(PresentationFrameError(
                "source-render execution receipt is missing or mismatched".to_owned(),
            ));
        }
        if let Some(map_world) = rendered.map_world.as_ref()
            && map_world.len() != LEVEL1_VIEW_PIXELS
        {
            return Err(PresentationFrameError(format!(
                "map-world renderer produced {} bytes, expected {LEVEL1_VIEW_PIXELS}",
                map_world.len()
            )));
        }
        let current_provenance = vec![SOURCE_WORLD_PROVENANCE; LEVEL1_VIEW_PIXELS];
        let postprocessed = finalize_indexed_world(
            input.postprocess,
            &rendered.indices,
            &current_provenance,
            self.cached.as_ref().map(|frame| frame.indices.as_slice()),
            self.cached
                .as_ref()
                .map(|frame| frame.provenance.as_slice()),
            tablesn,
        )?;
        if !postprocessed.receipt.matches(
            input.postprocess,
            rendered.receipt.index_hash,
            &postprocessed.indices,
            &postprocessed.provenance,
        ) || postprocessed.receipt.branch != postprocessed.branch
        {
            return Err(PresentationFrameError(
                "postprocess execution receipt does not match produced MAIN".to_owned(),
            ));
        }
        let postprocess_index_hash = byte_hash(&postprocessed.indices);
        let postprocess_provenance_hash = byte_hash(&postprocessed.provenance);
        let mut indices = postprocessed.indices;
        let mut provenance = postprocessed.provenance;
        let overlay = overlays.apply(
            &input.overlay,
            &mut indices,
            &mut provenance,
            rendered.map_world.as_deref(),
        )?;
        if !overlay.matches_plan(&input.overlay, &indices, &provenance) {
            return Err(PresentationFrameError(
                "overlay execution receipt does not match the planned command stream".to_owned(),
            ));
        }
        let expected_overlay_hash = input.overlay.immutable_hash();
        let expected_overlay_stages = input.overlay.required_stages.clone();
        let frame = PresentationFrame {
            generation: input.generation,
            tick: input.tick,
            presentation_revision: input.presentation_revision,
            immutable_input_hash,
            index_hash: byte_hash(&indices),
            provenance_hash: byte_hash(&provenance),
            indices,
            provenance,
            postprocess_branch: postprocessed.branch,
            overlay_receipts: overlay.receipts,
            overlay_pixels_written: overlay.pixels_written,
            help_sentinel: overlay.help_sentinel,
            source_receipt: rendered.receipt,
            lighting_receipt,
            postprocess_receipt: postprocessed.receipt,
            overlay_command_hash: overlay.command_hash,
            expected_overlay_hash,
            expected_overlay_stages,
            postprocess_input_hash: postprocess_input_hash(input.postprocess),
            postprocess_index_hash,
            postprocess_provenance_hash,
            lighting_hash: input.lighting_hash,
            source_reflections_expected: input.source_reflections_expected,
        };
        if !frame.compositor_complete() {
            return Err(PresentationFrameError(
                "FINAL MAIN receipts failed completion after local execution".to_owned(),
            ));
        }
        self.history = MainHistory::Primed;
        self.cached = Some(frame.clone());
        Ok(PresentationFrameResult {
            frame,
            newly_committed: true,
        })
    }
}

#[must_use]
pub fn immutable_input_hash(input: &PresentationFrameInput) -> u64 {
    let mut bytes = Vec::with_capacity(160);
    bytes.extend_from_slice(b"mc2-presentation-input-v1\0");
    for value in [
        input.generation,
        input.tick,
        input.presentation_revision,
        input.state_hash,
        input.source_arena_hash,
        input.source_render_input_hash,
        input.lighting_hash,
        input.camera_hash,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&[
        input.postprocess.configured_mode,
        u8::from(input.postprocess.blur_capability),
        u8::from(input.postprocess.high_speed_system),
        u8::from(input.postprocess.scratch_present),
        input.postprocess.vga_resolution,
        input.postprocess.menu_state,
        u8::from(input.postprocess.spatial_smoothing),
        u8::from(input.source_reflections_expected),
    ]);
    if let Some(receipt) = input.lighting_receipt {
        bytes.push(1);
        bytes.extend_from_slice(&receipt.state_hash.to_le_bytes());
        bytes.extend_from_slice(&receipt.shading_hash.to_le_bytes());
        bytes.extend_from_slice(&receipt.active_slot_mask.to_le_bytes());
        bytes.extend_from_slice(&receipt.shading_bytes.to_le_bytes());
        bytes.push(receipt.active_count);
    } else {
        bytes.push(0);
    }
    // The bridge-owned command hash covers every pixel-affecting live field
    // and payload. prior_help_sentinel remains history-owned.
    bytes.extend_from_slice(&input.overlay.immutable_hash().to_le_bytes());
    bytes.extend_from_slice(&input.postprocess.viewport_width.to_le_bytes());
    bytes.extend_from_slice(&input.postprocess.viewport_height.to_le_bytes());
    bytes.extend_from_slice(&input.postprocess.screen_stride.to_le_bytes());
    bytes.extend_from_slice(&input.postprocess.actual_speed.to_le_bytes());
    byte_hash(&bytes)
}

#[must_use]
pub fn byte_hash(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    bytes.iter().fold(OFFSET, |mut hash, byte| {
        hash ^= u64::from(*byte);
        hash.wrapping_mul(PRIME)
    })
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, path::PathBuf};

    use super::*;
    use crate::{
        dynamic_lighting::DynamicLightingExecutionReceipt,
        indexed_overlay::{IndexedOverlayCommand, IndexedOverlayStage},
        indexed_postprocess::TABLESN_REQUIRED_BYTES,
        terrain_frame::{
            SOURCE_RENDER_REQUIRED_FAMILY_MASK, SourceRenderExecutionReceipt,
            source_render_order_hash,
        },
    };

    fn input(tick: u64) -> PresentationFrameInput {
        let required_stages = vec![
            IndexedOverlayStage::SpiderWeb,
            IndexedOverlayStage::MinimapTerrain,
            IndexedOverlayStage::MinimapEntities,
            IndexedOverlayStage::MinimapMarks,
            IndexedOverlayStage::LeftSpell,
            IndexedOverlayStage::RightSpell,
            IndexedOverlayStage::TopStatus,
            IndexedOverlayStage::Menu,
            IndexedOverlayStage::PauseOrEndText,
            IndexedOverlayStage::ObjectiveTextbox,
            IndexedOverlayStage::TutorialOrBriefing,
            IndexedOverlayStage::HelpPopup,
            IndexedOverlayStage::DebugText,
        ];
        let commands = required_stages
            .iter()
            .map(|stage| {
                if *stage == IndexedOverlayStage::TopStatus {
                    IndexedOverlayCommand::Bitmap {
                        stage: *stage,
                        selector: 40,
                        x: 1,
                        y: 1,
                    }
                } else {
                    IndexedOverlayCommand::Decision {
                        stage: *stage,
                        state_hash: u64::from(*stage as u8),
                    }
                }
            })
            .collect();
        PresentationFrameInput {
            generation: 7,
            tick,
            presentation_revision: 3,
            state_hash: 11,
            source_arena_hash: 12,
            source_render_input_hash: 15,
            lighting_hash: 13,
            camera_hash: 14,
            postprocess: IndexedPostprocessInput::default(),
            overlay: Level1FinalMainState {
                commands,
                required_stages,
                next_help_sentinel: u8::try_from(tick + 1).expect("test tick fits u8"),
                ..Level1FinalMainState::default()
            },
            source_reflections_expected: false,
            lighting_receipt: Some(DynamicLightingExecutionReceipt {
                state_hash: 13,
                shading_hash: 17,
                active_slot_mask: 0,
                shading_bytes: 65_536,
                active_count: 0,
            }),
        }
    }

    fn rendered(value: u8) -> SourceRenderedFrame {
        let indices = vec![value; LEVEL1_VIEW_PIXELS];
        SourceRenderedFrame {
            receipt: SourceRenderExecutionReceipt {
                family_mask: SOURCE_RENDER_REQUIRED_FAMILY_MASK,
                order_hash: source_render_order_hash(),
                index_hash: byte_hash(&indices),
                coverage_hash: 19,
                terrain_pixel_writes: 1,
                sky_seeded: true,
                has_coverage: true,
                reflected_sprites_composited: false,
            },
            indices,
            map_world: None,
        }
    }

    fn overlays() -> IndexedOverlayAssets {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let root = crate::native_world::resolve_level1_presentation_cache_root(&private);
        crate::native_world::NativeWorld::load_with_owner_cache(
            private.join("generated/level1-export-m"),
            private.join("generated/level1-asset-closure.json"),
            &root,
        )
        .expect("manifest/hash-qualified interim presentation cache");
        IndexedOverlayAssets::load(root).expect("qualified interim presentation cache")
    }

    #[test]
    fn final_main_is_contiguous_idempotent_recursive_and_reset_unprimed() {
        let overlays = overlays();
        let tablesn = vec![0; TABLESN_REQUIRED_BYTES];
        let calls = Cell::new(0_u32);
        let mut state = PresentationFrameState::new(7);
        let first = state
            .finalize(input(0), &tablesn, &overlays, || {
                calls.set(calls.get() + 1);
                Ok(rendered(17))
            })
            .expect("first");
        assert!(first.newly_committed);
        assert_eq!(state.history(), MainHistory::Primed);
        assert_eq!(first.frame.help_sentinel, 1);
        let repeated = state
            .finalize(input(0), &tablesn, &overlays, || {
                panic!("same-key repeat must not render")
            })
            .expect("repeat");
        assert!(!repeated.newly_committed);
        assert_eq!(repeated.frame, first.frame);
        let second = state
            .finalize(input(1), &tablesn, &overlays, || {
                calls.set(calls.get() + 1);
                Ok(rendered(18))
            })
            .expect("second");
        assert_eq!(second.frame.help_sentinel, 2);
        assert_eq!(calls.get(), 2);

        state.reset(8);
        assert_eq!(state.history(), MainHistory::Unprimed);
        assert!(state.cached().is_none());
    }

    #[test]
    fn changed_stale_out_of_order_and_incomplete_reject_atomically() {
        let overlays = overlays();
        let tablesn = vec![0; TABLESN_REQUIRED_BYTES];
        let mut state = PresentationFrameState::new(7);
        let baseline = state
            .finalize(input(0), &tablesn, &overlays, || Ok(rendered(17)))
            .expect("baseline")
            .frame;
        for (label, changed) in [
            ("same tick changed revision", {
                let mut value = input(0);
                value.presentation_revision = 4;
                value
            }),
            ("same tick changed hash", {
                let mut value = input(0);
                value.state_hash = 99;
                value
            }),
            ("same tick changed source-render input", {
                let mut value = input(0);
                value.source_render_input_hash ^= 1;
                value
            }),
            ("out of order", input(2)),
        ] {
            let called = Cell::new(false);
            let error = state
                .finalize(changed, &tablesn, &overlays, || {
                    called.set(true);
                    Ok(rendered(0))
                })
                .expect_err(label);
            assert!(!called.get(), "{label} rendered before rejecting: {error}");
            assert_eq!(state.cached(), Some(&baseline));
        }
        let mut incomplete = input(1);
        incomplete.lighting_receipt = None;
        assert!(
            state
                .finalize(incomplete, &tablesn, &overlays, || { Ok(rendered(0)) })
                .expect_err("incomplete")
                .0
                .contains("dynamic-lighting")
        );
        assert_eq!(state.cached(), Some(&baseline));

        let mut mismatched_lighting = input(1);
        mismatched_lighting
            .lighting_receipt
            .as_mut()
            .expect("test lighting receipt")
            .state_hash ^= 1;
        assert!(
            state
                .finalize(mismatched_lighting, &tablesn, &overlays, || {
                    Ok(rendered(0))
                })
                .expect_err("mismatched lighting receipt")
                .0
                .contains("dynamic-lighting")
        );
        assert_eq!(state.cached(), Some(&baseline));

        for (label, mutate) in [
            ("missing family", 0_u8),
            ("source order", 1),
            ("reflection", 2),
        ] {
            let result = state.finalize(input(1), &tablesn, &overlays, || {
                let mut value = rendered(0);
                match mutate {
                    0 => value.receipt.family_mask ^= 1,
                    1 => value.receipt.order_hash ^= 1,
                    2 => value.receipt.reflected_sprites_composited = true,
                    _ => unreachable!(),
                }
                Ok(value)
            });
            assert!(
                result.expect_err(label).0.contains("source-render"),
                "{label} must reject its actual source execution receipt"
            );
            assert_eq!(state.cached(), Some(&baseline));
        }

        let mut temporal = input(1);
        temporal.postprocess.configured_mode = 2;
        temporal.postprocess.blur_capability = true;
        assert!(
            state
                .finalize(temporal, &[], &overlays, || Ok(rendered(0)))
                .expect_err("postprocess output receipt")
                .0
                .contains("TABLESN")
        );
        assert_eq!(state.cached(), Some(&baseline));

        let mut bad_overlay = input(1);
        let IndexedOverlayCommand::Bitmap { selector, .. } = bad_overlay
            .overlay
            .commands
            .iter_mut()
            .find(|command| command.stage() == IndexedOverlayStage::TopStatus)
            .expect("top-status source command")
        else {
            unreachable!("test top-status command is a bitmap");
        };
        *selector = 41;
        assert!(
            state
                .finalize(bad_overlay, &tablesn, &overlays, || Ok(rendered(0)))
                .expect_err("overlay selector")
                .0
                .contains("top-status")
        );
        assert_eq!(state.cached(), Some(&baseline));

        let retry = state
            .finalize(input(1), &tablesn, &overlays, || Ok(rendered(23)))
            .expect("same-tick retry after every rejected receipt");
        assert!(retry.newly_committed);
        assert_eq!(state.cached(), Some(&retry.frame));
    }

    #[test]
    fn temporal_reads_prior_final_main_including_overlay_pixels() {
        let overlays = overlays();
        let mut tablesn = vec![0; TABLESN_REQUIRED_BYTES];
        for pair in 0..=u16::MAX {
            tablesn[0x4000 + usize::from(pair)] = pair.to_le_bytes()[0];
        }
        let mut state = PresentationFrameState::new(7);
        let first = state
            .finalize(input(0), &tablesn, &overlays, || Ok(rendered(17)))
            .expect("first")
            .frame;
        let overlay_index = first
            .provenance
            .iter()
            .position(|value| (*value & crate::indexed_postprocess::OVERLAY_PROVENANCE) != 0)
            .expect("overlay pixel");
        let prior_overlay_value = first.indices[overlay_index];
        let mut second_input = input(1);
        second_input.postprocess.configured_mode = 2;
        second_input.postprocess.blur_capability = true;
        let second = state
            .finalize(second_input, &tablesn, &overlays, || Ok(rendered(91)))
            .expect("temporal")
            .frame;
        assert_eq!(
            second.postprocess_branch,
            IndexedPostprocessBranch::Temporal { mode: 2 }
        );
        // TABLESN returns the low byte, which is prior MAIN in normal order.
        // The final current overlay may rewrite a subset, so inspect a prior
        // overlay pixel outside the current help glyph where possible.
        assert!(
            second.indices.contains(&prior_overlay_value),
            "temporal input includes finalized prior overlays"
        );
    }
}
