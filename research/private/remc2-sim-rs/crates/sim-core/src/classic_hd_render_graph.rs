//! Transactional Classic HD world/postprocess graph.
//!
//! The graph owns profile-sized current and scratch planes. A world closure
//! renders the selected terrain/sprite/reflection path into scratch; lighting
//! and source-style blur then operate on those planes. History is swapped into
//! the committed MAIN only after all enabled stages and receipts validate, so
//! a failed frame cannot leak pixels or advance the history key.

use std::{mem::swap, time::Instant};

use crate::{
    classic_hd::{ClassicHdError, ClassicHdRenderProfile, byte_hash},
    classic_hd_raster::{
        CLASSIC_HD_TABLESN_BYTES, ClassicHdHistoryKey, ClassicHdRasterError,
        apply_classic_hd_dynamic_lighting, blend_classic_hd_history_with_tablesn,
        smooth_classic_hd_indexed_with_tablesn,
    },
};

pub const CLASSIC_HD_MODE1_HISTORY_WEIGHT: u8 = 96;
pub const CLASSIC_HD_MODE2_HISTORY_WEIGHT: u8 = 160;
pub const CLASSIC_HD_STAGE_COUNT: usize = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdBlurInput {
    pub configured_mode: u8,
    pub blur_capability: bool,
    pub high_speed_system: bool,
    pub scratch_present: bool,
    pub menu_state: u8,
    pub actual_speed: i16,
    pub spatial_smoothing: bool,
    pub spatial_radius: u8,
}

impl Default for ClassicHdBlurInput {
    fn default() -> Self {
        Self {
            configured_mode: 0,
            blur_capability: false,
            high_speed_system: true,
            scratch_present: true,
            menu_state: 0,
            actual_speed: 0,
            spatial_smoothing: false,
            spatial_radius: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdGraphInput {
    pub generation: u64,
    pub tick: u64,
    pub profile_revision: u64,
    pub input_hash: u64,
    pub reflections_enabled: bool,
    pub dynamic_lighting_enabled: bool,
    pub water_enabled: bool,
    pub blur: ClassicHdBlurInput,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdWorldReceipt {
    pub world_hash: u64,
    pub provenance_hash: u64,
    pub terrain_rendered: bool,
    pub typed_sprites_rendered: bool,
    pub reflection_pass_rendered: bool,
    pub shadow_pass_rendered: bool,
    pub water_pass_rendered: bool,
}

impl ClassicHdWorldReceipt {
    #[must_use]
    pub const fn complete(
        world_hash: u64,
        provenance_hash: u64,
        reflections_enabled: bool,
        water_enabled: bool,
    ) -> Self {
        Self {
            world_hash,
            provenance_hash,
            terrain_rendered: true,
            typed_sprites_rendered: true,
            reflection_pass_rendered: reflections_enabled,
            shadow_pass_rendered: true,
            water_pass_rendered: water_enabled,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassicHdGraphStage {
    World,
    Reflections,
    DynamicLighting,
    SpatialBlur,
    TemporalBlur,
    Present,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdStageReceipt {
    pub stage: ClassicHdGraphStage,
    pub enabled: bool,
    pub input_hash: u64,
    pub output_hash: u64,
    pub work_units: u64,
    pub elapsed_ns: u64,
}

impl ClassicHdStageReceipt {
    #[must_use]
    pub const fn disabled(stage: ClassicHdGraphStage, hash: u64) -> Self {
        Self {
            stage,
            enabled: false,
            input_hash: hash,
            output_hash: hash,
            work_units: 0,
            elapsed_ns: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassicHdBlurBranch {
    Direct { spatial: bool },
    Temporal { mode: u8 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdGraphReceipt {
    pub generation: u64,
    pub tick: u64,
    pub profile_revision: u64,
    pub input_hash: u64,
    pub prior_main_hash: u64,
    pub output_hash: u64,
    pub provenance_hash: u64,
    pub history_hash: u64,
    pub blur_branch: ClassicHdBlurBranch,
    pub stages: [ClassicHdStageReceipt; CLASSIC_HD_STAGE_COUNT],
}

impl ClassicHdGraphReceipt {
    #[must_use]
    pub fn stage(&self, wanted: ClassicHdGraphStage) -> Option<ClassicHdStageReceipt> {
        self.stages
            .iter()
            .copied()
            .find(|stage| stage.stage == wanted)
    }

    #[must_use]
    pub fn complete(self) -> bool {
        let expected_order = [
            ClassicHdGraphStage::World,
            ClassicHdGraphStage::Reflections,
            ClassicHdGraphStage::DynamicLighting,
            ClassicHdGraphStage::SpatialBlur,
            ClassicHdGraphStage::TemporalBlur,
            ClassicHdGraphStage::Present,
        ];
        let present = self.stage(ClassicHdGraphStage::Present);
        self.stages.map(|stage| stage.stage) == expected_order
            && self.output_hash != 0
            && self.provenance_hash != 0
            && present.is_some_and(|stage| stage.enabled && stage.output_hash == self.output_hash)
            && self
                .stages
                .iter()
                .all(|stage| stage.enabled || stage.work_units == 0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassicHdGraphError(pub String);

impl std::fmt::Display for ClassicHdGraphError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ClassicHdGraphError {}

impl From<ClassicHdError> for ClassicHdGraphError {
    fn from(error: ClassicHdError) -> Self {
        Self(error.0)
    }
}

impl From<ClassicHdRasterError> for ClassicHdGraphError {
    fn from(error: ClassicHdRasterError) -> Self {
        Self(error.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdGraphResult {
    pub receipt: ClassicHdGraphReceipt,
    pub newly_committed: bool,
}

#[derive(Debug)]
pub struct ClassicHdRenderGraph {
    profile: ClassicHdRenderProfile,
    generation: u64,
    current_tick: Option<u64>,
    current_key: Option<ClassicHdHistoryKey>,
    last_receipt: Option<ClassicHdGraphReceipt>,
    last_input: Option<ClassicHdGraphInput>,
    main: Vec<u8>,
    main_provenance: Vec<u8>,
    world_scratch: Vec<u8>,
    world_provenance: Vec<u8>,
    post_scratch: Vec<u8>,
    post_provenance: Vec<u8>,
}

impl ClassicHdRenderGraph {
    pub fn new(
        profile: ClassicHdRenderProfile,
        generation: u64,
    ) -> Result<Self, ClassicHdGraphError> {
        let extent = profile.extent();
        if extent == 0 {
            return Err(ClassicHdGraphError(
                "Classic HD graph profile extent is empty".to_owned(),
            ));
        }
        Ok(Self {
            profile,
            generation,
            current_tick: None,
            current_key: None,
            last_receipt: None,
            last_input: None,
            main: vec![0; extent],
            main_provenance: vec![0; extent],
            world_scratch: vec![0; extent],
            world_provenance: vec![0; extent],
            post_scratch: vec![0; extent],
            post_provenance: vec![0; extent],
        })
    }

    #[must_use]
    pub const fn profile(&self) -> &ClassicHdRenderProfile {
        &self.profile
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn current_tick(&self) -> Option<u64> {
        self.current_tick
    }

    #[must_use]
    pub fn current_indices(&self) -> &[u8] {
        &self.main
    }

    #[must_use]
    pub fn current_provenance(&self) -> &[u8] {
        &self.main_provenance
    }

    #[must_use]
    pub const fn history_key(&self) -> Option<ClassicHdHistoryKey> {
        self.current_key
    }

    #[must_use]
    pub fn storage_pointers(&self) -> (usize, usize, usize, usize, usize, usize) {
        (
            self.main.as_ptr() as usize,
            self.main_provenance.as_ptr() as usize,
            self.world_scratch.as_ptr() as usize,
            self.world_provenance.as_ptr() as usize,
            self.post_scratch.as_ptr() as usize,
            self.post_provenance.as_ptr() as usize,
        )
    }

    pub fn reset(&mut self, generation: u64) {
        self.generation = generation;
        self.current_tick = None;
        self.current_key = None;
        self.last_receipt = None;
        self.last_input = None;
        self.main.fill(0);
        self.main_provenance.fill(0);
        self.world_scratch.fill(0);
        self.world_provenance.fill(0);
        self.post_scratch.fill(0);
        self.post_provenance.fill(0);
    }

    pub fn reconfigure(
        &mut self,
        profile: ClassicHdRenderProfile,
        generation: u64,
    ) -> Result<(), ClassicHdGraphError> {
        let replacement = Self::new(profile, generation)?;
        *self = replacement;
        Ok(())
    }

    pub fn render_frame<F>(
        &mut self,
        input: ClassicHdGraphInput,
        shading: Option<&[u8]>,
        tablesn: &[u8],
        render_world: F,
    ) -> Result<ClassicHdGraphResult, ClassicHdGraphError>
    where
        F: FnOnce(&mut [u8], &mut [u8]) -> Result<ClassicHdWorldReceipt, ClassicHdGraphError>,
    {
        self.validate_input(input)?;
        if let Some(receipt) = self.last_receipt {
            if input.tick == receipt.tick {
                if self.last_input == Some(input) {
                    return Ok(ClassicHdGraphResult {
                        receipt,
                        newly_committed: false,
                    });
                }
                return Err(ClassicHdGraphError(
                    "Classic HD same tick changed graph input".to_owned(),
                ));
            }
            if input.tick < receipt.tick {
                return Err(ClassicHdGraphError(format!(
                    "Classic HD tick {} is stale; committed {}",
                    input.tick, receipt.tick
                )));
            }
        }
        let expected_tick = match self.current_tick {
            None => 0,
            Some(tick) => tick
                .checked_add(1)
                .ok_or_else(|| ClassicHdGraphError("Classic HD tick overflow".to_owned()))?,
        };
        if input.tick != expected_tick {
            return Err(ClassicHdGraphError(format!(
                "Classic HD tick {} is out of order; expected {expected_tick}",
                input.tick
            )));
        }

        self.world_scratch.fill(0);
        self.world_provenance.fill(0);
        self.post_scratch.fill(0);
        self.post_provenance.fill(0);
        let world_started = Instant::now();
        let world = render_world(&mut self.world_scratch, &mut self.world_provenance)?;
        let world_elapsed_ns = elapsed_ns(world_started);
        validate_world_receipt(input, &world, &self.world_scratch, &self.world_provenance)?;

        let world_hash = byte_hash(&self.world_scratch);
        let mut stages = [
            ClassicHdStageReceipt {
                stage: ClassicHdGraphStage::World,
                enabled: true,
                input_hash: input.input_hash,
                output_hash: world_hash,
                work_units: self.world_scratch.len() as u64,
                elapsed_ns: world_elapsed_ns,
            },
            ClassicHdStageReceipt::disabled(ClassicHdGraphStage::Reflections, world_hash),
            ClassicHdStageReceipt::disabled(ClassicHdGraphStage::DynamicLighting, world_hash),
            ClassicHdStageReceipt::disabled(ClassicHdGraphStage::SpatialBlur, world_hash),
            ClassicHdStageReceipt::disabled(ClassicHdGraphStage::TemporalBlur, world_hash),
            ClassicHdStageReceipt::disabled(ClassicHdGraphStage::Present, world_hash),
        ];
        if input.reflections_enabled {
            stages[1] = ClassicHdStageReceipt {
                stage: ClassicHdGraphStage::Reflections,
                enabled: true,
                input_hash: world_hash,
                output_hash: world_hash,
                work_units: self.world_scratch.len() as u64,
                elapsed_ns: 0,
            };
        }

        let mut current_hash = world_hash;
        if input.dynamic_lighting_enabled {
            let shading = shading.ok_or_else(|| {
                ClassicHdGraphError(
                    "Classic HD dynamic lighting is enabled without a shading plane".to_owned(),
                )
            })?;
            let before = current_hash;
            let lighting_started = Instant::now();
            current_hash = apply_classic_hd_dynamic_lighting(
                &self.profile,
                &mut self.world_scratch,
                shading,
                tablesn.get(..CLASSIC_HD_TABLESN_BYTES).ok_or_else(|| {
                    ClassicHdGraphError(format!(
                        "Classic HD dynamic lighting requires at least {CLASSIC_HD_TABLESN_BYTES} TABLESN bytes"
                    ))
                })?,
            )?;
            stages[2] = ClassicHdStageReceipt {
                stage: ClassicHdGraphStage::DynamicLighting,
                enabled: true,
                input_hash: before,
                output_hash: current_hash,
                work_units: self.world_scratch.len() as u64,
                elapsed_ns: elapsed_ns(lighting_started),
            };
        }

        let effective_mode = effective_blur_mode(input.blur);
        let prior_main_hash = self.current_key.map_or(0, |_| byte_hash(&self.main));
        let mut branch = ClassicHdBlurBranch::Direct { spatial: false };
        let mut final_plane = FinalPlane::World;
        let temporal_requested =
            input.blur.blur_capability && effective_mode != 0 && input.blur.scratch_present;
        if temporal_requested {
            if let Some(history_key) = self.current_key {
                let current_key = ClassicHdHistoryKey {
                    generation: input.generation,
                    profile_revision: input.profile_revision,
                    tick: input.tick,
                    input_hash: input.input_hash,
                };
                let before = current_hash;
                let temporal_started = Instant::now();
                current_hash = blend_classic_hd_history_with_tablesn(
                    &self.profile,
                    &mut self.world_scratch,
                    &mut self.world_provenance,
                    &self.main,
                    current_key,
                    history_key,
                    tablesn,
                    effective_mode,
                )?;
                stages[4] = ClassicHdStageReceipt {
                    stage: ClassicHdGraphStage::TemporalBlur,
                    enabled: true,
                    input_hash: before,
                    output_hash: current_hash,
                    work_units: self.world_scratch.len() as u64,
                    elapsed_ns: elapsed_ns(temporal_started),
                };
                branch = ClassicHdBlurBranch::Temporal {
                    mode: effective_mode,
                };
            } else {
                // A reset/profile change has no compatible prior MAIN. The
                // first frame is a genuine profile-sized rerender, not an
                // upscale of a stale or Exact frame.
                branch = ClassicHdBlurBranch::Direct { spatial: false };
            }
        }
        if !matches!(branch, ClassicHdBlurBranch::Temporal { .. }) && input.blur.spatial_smoothing {
            let radius = u32::from(input.blur.spatial_radius);
            let before = current_hash;
            let spatial_started = Instant::now();
            current_hash = smooth_classic_hd_indexed_with_tablesn(
                &self.profile,
                &self.world_scratch,
                &mut self.post_scratch,
                &self.world_provenance,
                &mut self.post_provenance,
                tablesn,
                radius,
            )?;
            stages[3] = ClassicHdStageReceipt {
                stage: ClassicHdGraphStage::SpatialBlur,
                enabled: true,
                input_hash: before,
                output_hash: current_hash,
                work_units: self.post_scratch.len() as u64,
                elapsed_ns: elapsed_ns(spatial_started),
            };
            final_plane = FinalPlane::Post;
            branch = ClassicHdBlurBranch::Direct { spatial: true };
        }

        let (final_indices, final_provenance) = match final_plane {
            FinalPlane::World => (&self.world_scratch, &self.world_provenance),
            FinalPlane::Post => (&self.post_scratch, &self.post_provenance),
        };
        let output_hash = byte_hash(final_indices);
        let provenance_hash = byte_hash(final_provenance);
        stages[5] = ClassicHdStageReceipt {
            stage: ClassicHdGraphStage::Present,
            enabled: true,
            input_hash: current_hash,
            output_hash,
            work_units: final_indices.len() as u64,
            elapsed_ns: 0,
        };
        let receipt = ClassicHdGraphReceipt {
            generation: input.generation,
            tick: input.tick,
            profile_revision: input.profile_revision,
            input_hash: input.input_hash,
            prior_main_hash,
            output_hash,
            provenance_hash,
            history_hash: prior_main_hash,
            blur_branch: branch,
            stages,
        };
        if !receipt.complete() {
            return Err(ClassicHdGraphError(
                "Classic HD graph receipt failed completion".to_owned(),
            ));
        }

        match final_plane {
            FinalPlane::World => {
                swap(&mut self.main, &mut self.world_scratch);
                swap(&mut self.main_provenance, &mut self.world_provenance);
            }
            FinalPlane::Post => {
                swap(&mut self.main, &mut self.post_scratch);
                swap(&mut self.main_provenance, &mut self.post_provenance);
            }
        }
        self.current_tick = Some(input.tick);
        self.current_key = Some(ClassicHdHistoryKey {
            generation: input.generation,
            profile_revision: input.profile_revision,
            tick: input.tick,
            input_hash: input.input_hash,
        });
        self.last_receipt = Some(receipt);
        self.last_input = Some(input);
        Ok(ClassicHdGraphResult {
            receipt,
            newly_committed: true,
        })
    }

    fn validate_input(&self, input: ClassicHdGraphInput) -> Result<(), ClassicHdGraphError> {
        if input.generation != self.generation {
            return Err(ClassicHdGraphError(format!(
                "Classic HD generation {} does not match {}",
                input.generation, self.generation
            )));
        }
        if input.profile_revision != self.profile.profile_revision {
            return Err(ClassicHdGraphError(
                "Classic HD graph profile revision mismatch".to_owned(),
            ));
        }
        if input.blur.configured_mode > 2 {
            return Err(ClassicHdGraphError(format!(
                "unsupported Classic HD configured blur mode {}",
                input.blur.configured_mode
            )));
        }
        if input.blur.spatial_smoothing
            && (input.blur.spatial_radius == 0 || input.blur.spatial_radius > 4)
        {
            return Err(ClassicHdGraphError(
                "Classic HD spatial blur radius must be between 1 and 4".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum FinalPlane {
    World,
    Post,
}

#[must_use]
pub const fn effective_blur_mode(input: ClassicHdBlurInput) -> u8 {
    if input.configured_mode != 0 {
        return input.configured_mode;
    }
    if input.menu_state == 0
        && input.high_speed_system
        && input.scratch_present
        && input.actual_speed.unsigned_abs() > 80
    {
        1
    } else {
        0
    }
}

fn elapsed_ns(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

fn validate_world_receipt(
    input: ClassicHdGraphInput,
    receipt: &ClassicHdWorldReceipt,
    indices: &[u8],
    provenance: &[u8],
) -> Result<(), ClassicHdGraphError> {
    if !receipt.terrain_rendered || !receipt.typed_sprites_rendered || !receipt.shadow_pass_rendered
    {
        return Err(ClassicHdGraphError(
            "Classic HD world receipt is missing terrain, typed sprites, or shadows".to_owned(),
        ));
    }
    if input.reflections_enabled && !receipt.reflection_pass_rendered {
        return Err(ClassicHdGraphError(
            "Classic HD reflections were enabled but the world receipt omitted them".to_owned(),
        ));
    }
    if input.water_enabled && !receipt.water_pass_rendered {
        return Err(ClassicHdGraphError(
            "Classic HD water was enabled but the world receipt omitted mode-26 water".to_owned(),
        ));
    }
    if receipt.world_hash != byte_hash(indices) || receipt.provenance_hash != byte_hash(provenance)
    {
        return Err(ClassicHdGraphError(
            "Classic HD world receipt does not match the rendered planes".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        classic_hd::{ClassicHdRenderProfile, byte_hash},
        indexed_postprocess::{
            IndexedPostprocessInput, TABLESN_BLEND_OFFSET, TABLESN_REQUIRED_BYTES,
            finalize_indexed_world,
        },
    };

    fn tablesn() -> Vec<u8> {
        let mut table = vec![0_u8; TABLESN_REQUIRED_BYTES];
        for pair in 0..=u16::MAX {
            let high = (pair >> 8) as u8;
            let low = pair as u8;
            table[TABLESN_BLEND_OFFSET + usize::from(pair)] =
                high.wrapping_mul(3).wrapping_add(low.wrapping_mul(7));
        }
        table
    }

    fn pattern(seed: u8, extent: usize) -> Vec<u8> {
        (0..extent)
            .map(|index| seed.wrapping_add((index as u8).rotate_left(1)))
            .collect()
    }

    fn input(
        profile: &ClassicHdRenderProfile,
        tick: u64,
        blur: ClassicHdBlurInput,
    ) -> ClassicHdGraphInput {
        ClassicHdGraphInput {
            generation: 7,
            tick,
            profile_revision: profile.profile_revision,
            input_hash: 0x5a,
            reflections_enabled: false,
            dynamic_lighting_enabled: false,
            water_enabled: false,
            blur,
        }
    }

    fn render_pattern(
        graph: &mut ClassicHdRenderGraph,
        input: ClassicHdGraphInput,
        table: &[u8],
        seed: u8,
    ) -> Result<ClassicHdGraphResult, ClassicHdGraphError> {
        graph.render_frame(input, None, table, move |indices, provenance| {
            let rendered = pattern(seed, indices.len());
            indices.copy_from_slice(&rendered);
            provenance.fill(0x20);
            Ok(ClassicHdWorldReceipt::complete(
                byte_hash(indices),
                byte_hash(provenance),
                input.reflections_enabled,
                input.water_enabled,
            ))
        })
    }

    #[test]
    fn automatic_and_configured_blur_gates_cover_direct_and_temporal_branches() {
        let profile = ClassicHdRenderProfile::compatibility_320x200();
        let table = tablesn();
        let mut graph = ClassicHdRenderGraph::new(profile.clone(), 7).unwrap();
        let base = ClassicHdBlurInput {
            blur_capability: true,
            ..ClassicHdBlurInput::default()
        };

        let first = render_pattern(&mut graph, input(&profile, 0, base), &table, 1).unwrap();
        assert_eq!(
            first.receipt.blur_branch,
            ClassicHdBlurBranch::Direct { spatial: false }
        );
        assert!(
            !first
                .receipt
                .stage(ClassicHdGraphStage::TemporalBlur)
                .unwrap()
                .enabled
        );

        let automatic = ClassicHdBlurInput {
            actual_speed: 100,
            ..base
        };
        let second = render_pattern(&mut graph, input(&profile, 1, automatic), &table, 2).unwrap();
        assert_eq!(
            second.receipt.blur_branch,
            ClassicHdBlurBranch::Temporal { mode: 1 }
        );
        assert!(
            second
                .receipt
                .stage(ClassicHdGraphStage::TemporalBlur)
                .unwrap()
                .enabled
        );
        assert!(
            second
                .receipt
                .stage(ClassicHdGraphStage::TemporalBlur)
                .unwrap()
                .elapsed_ns
                > 0
        );

        let menu_suppressed = ClassicHdBlurInput {
            actual_speed: 100,
            menu_state: 1,
            ..base
        };
        let third =
            render_pattern(&mut graph, input(&profile, 2, menu_suppressed), &table, 3).unwrap();
        assert_eq!(
            third.receipt.blur_branch,
            ClassicHdBlurBranch::Direct { spatial: false }
        );

        let scratch_unavailable = ClassicHdBlurInput {
            actual_speed: 100,
            scratch_present: false,
            ..base
        };
        let fourth = render_pattern(
            &mut graph,
            input(&profile, 3, scratch_unavailable),
            &table,
            4,
        )
        .unwrap();
        assert_eq!(
            fourth.receipt.blur_branch,
            ClassicHdBlurBranch::Direct { spatial: false }
        );

        let configured_mode2 = ClassicHdBlurInput {
            configured_mode: 2,
            actual_speed: 0,
            ..base
        };
        let fifth =
            render_pattern(&mut graph, input(&profile, 4, configured_mode2), &table, 5).unwrap();
        assert_eq!(
            fifth.receipt.blur_branch,
            ClassicHdBlurBranch::Temporal { mode: 2 }
        );
        assert!(fifth.receipt.complete());
    }

    #[test]
    fn effective_mode_keeps_configured_mode_above_automatic_gates() {
        let automatic = ClassicHdBlurInput {
            actual_speed: 100,
            ..ClassicHdBlurInput::default()
        };
        assert_eq!(effective_blur_mode(automatic), 1);
        assert_eq!(
            effective_blur_mode(ClassicHdBlurInput {
                menu_state: 1,
                ..automatic
            }),
            0
        );
        assert_eq!(
            effective_blur_mode(ClassicHdBlurInput {
                scratch_present: false,
                ..automatic
            }),
            0
        );
        assert_eq!(
            effective_blur_mode(ClassicHdBlurInput {
                configured_mode: 2,
                menu_state: 1,
                high_speed_system: false,
                scratch_present: false,
                actual_speed: 0,
                ..automatic
            }),
            2
        );
    }

    #[test]
    fn world_receipt_covers_reflections_shadows_lighting_and_mode26_water() {
        let profile = ClassicHdRenderProfile::hd_640x480();
        let table = tablesn();
        let shading = vec![0_u8; 256 * 256];
        let mut graph = ClassicHdRenderGraph::new(profile.clone(), 7).unwrap();
        let mut frame_input = input(&profile, 0, ClassicHdBlurInput::default());
        frame_input.reflections_enabled = true;
        frame_input.dynamic_lighting_enabled = true;
        frame_input.water_enabled = true;
        frame_input.blur = ClassicHdBlurInput {
            configured_mode: 2,
            blur_capability: true,
            ..ClassicHdBlurInput::default()
        };
        let result = graph
            .render_frame(
                frame_input,
                Some(&shading),
                &table,
                move |indices, provenance| {
                    indices.fill(9);
                    provenance.fill(0x31);
                    Ok(ClassicHdWorldReceipt::complete(
                        byte_hash(indices),
                        byte_hash(provenance),
                        true,
                        true,
                    ))
                },
            )
            .unwrap();
        assert!(
            result
                .receipt
                .stage(ClassicHdGraphStage::Reflections)
                .unwrap()
                .enabled
        );
        assert!(
            result
                .receipt
                .stage(ClassicHdGraphStage::DynamicLighting)
                .unwrap()
                .enabled
        );
        assert_eq!(
            result
                .receipt
                .stage(ClassicHdGraphStage::DynamicLighting)
                .unwrap()
                .work_units,
            profile.extent() as u64
        );
        assert!(result.receipt.complete());

        let mut second_input = frame_input;
        second_input.tick = 1;
        let second = graph
            .render_frame(
                second_input,
                Some(&shading),
                &table,
                move |indices, provenance| {
                    indices.fill(10);
                    provenance.fill(0x32);
                    Ok(ClassicHdWorldReceipt::complete(
                        byte_hash(indices),
                        byte_hash(provenance),
                        true,
                        true,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            second.receipt.blur_branch,
            ClassicHdBlurBranch::Temporal { mode: 2 }
        );
        assert!(
            second
                .receipt
                .stage(ClassicHdGraphStage::DynamicLighting)
                .unwrap()
                .enabled
        );
    }

    #[test]
    fn spatial_smoothing_is_profile_sized_and_receipted() {
        let profile = ClassicHdRenderProfile::hd_640x480();
        let table = tablesn();
        let mut graph = ClassicHdRenderGraph::new(profile.clone(), 7).unwrap();
        let blur = ClassicHdBlurInput {
            spatial_smoothing: true,
            spatial_radius: 2,
            ..ClassicHdBlurInput::default()
        };
        let result = render_pattern(&mut graph, input(&profile, 0, blur), &table, 11).unwrap();
        assert_eq!(
            result.receipt.blur_branch,
            ClassicHdBlurBranch::Direct { spatial: true }
        );
        assert!(
            result
                .receipt
                .stage(ClassicHdGraphStage::SpatialBlur)
                .unwrap()
                .enabled
        );
        assert_eq!(graph.current_indices().len(), profile.extent());
        assert!(
            graph
                .current_provenance()
                .iter()
                .any(|value| value & crate::indexed_postprocess::SPATIAL_PROVENANCE != 0)
        );
    }

    #[test]
    fn failed_frame_does_not_advance_main_history_or_storage() {
        let profile = ClassicHdRenderProfile::compatibility_320x200();
        let table = tablesn();
        let mut graph = ClassicHdRenderGraph::new(profile.clone(), 7).unwrap();
        let base = input(&profile, 0, ClassicHdBlurInput::default());
        render_pattern(&mut graph, base, &table, 21).unwrap();
        let before_indices = graph.current_indices().to_vec();
        let before_provenance = graph.current_provenance().to_vec();
        let before_tick = graph.current_tick();
        let before_key = graph.history_key();
        let before_pointers = graph.storage_pointers();

        let mut failed = input(&profile, 1, ClassicHdBlurInput::default());
        failed.reflections_enabled = true;
        let error = graph
            .render_frame(failed, None, &table, |indices, provenance| {
                indices.fill(99);
                provenance.fill(88);
                Err(ClassicHdGraphError("synthetic failed world".to_owned()))
            })
            .unwrap_err();
        assert_eq!(error.0, "synthetic failed world");
        assert_eq!(graph.current_indices(), before_indices.as_slice());
        assert_eq!(graph.current_provenance(), before_provenance.as_slice());
        assert_eq!(graph.current_tick(), before_tick);
        assert_eq!(graph.history_key(), before_key);
        assert_eq!(graph.storage_pointers(), before_pointers);

        let recovered = render_pattern(
            &mut graph,
            input(&profile, 1, ClassicHdBlurInput::default()),
            &table,
            22,
        )
        .unwrap();
        assert!(recovered.newly_committed);
        assert_eq!(graph.current_tick(), Some(1));
    }

    #[test]
    fn reset_and_profile_change_clear_history_without_upscaling() {
        let compatibility = ClassicHdRenderProfile::compatibility_320x200();
        let table = tablesn();
        let mut graph = ClassicHdRenderGraph::new(compatibility.clone(), 7).unwrap();
        render_pattern(
            &mut graph,
            input(&compatibility, 0, ClassicHdBlurInput::default()),
            &table,
            31,
        )
        .unwrap();
        graph.reset(8);
        assert_eq!(graph.generation(), 8);
        assert_eq!(graph.current_tick(), None);
        assert_eq!(graph.history_key(), None);
        assert!(graph.current_indices().iter().all(|value| *value == 0));

        let hd = ClassicHdRenderProfile::hd_640x480();
        graph.reconfigure(hd.clone(), 9).unwrap();
        assert_eq!(graph.current_indices().len(), hd.extent());
        let hd_input = ClassicHdGraphInput {
            generation: 9,
            tick: 0,
            profile_revision: hd.profile_revision,
            input_hash: 0x5a,
            reflections_enabled: false,
            dynamic_lighting_enabled: false,
            water_enabled: false,
            blur: ClassicHdBlurInput {
                configured_mode: 2,
                blur_capability: true,
                ..ClassicHdBlurInput::default()
            },
        };
        let first_hd = render_pattern(&mut graph, hd_input, &table, 32).unwrap();
        assert_eq!(
            first_hd.receipt.blur_branch,
            ClassicHdBlurBranch::Direct { spatial: false }
        );
        assert_eq!(
            first_hd.receipt.output_hash,
            byte_hash(graph.current_indices())
        );
    }

    #[test]
    fn compatibility_temporal_output_matches_exact_postprocess_fixture_path() {
        let profile = ClassicHdRenderProfile::compatibility_320x200();
        let table = tablesn();
        let mut graph = ClassicHdRenderGraph::new(profile.clone(), 7).unwrap();
        let blur = ClassicHdBlurInput {
            configured_mode: 2,
            blur_capability: true,
            ..ClassicHdBlurInput::default()
        };
        let first_input = input(&profile, 0, blur);
        let prior = pattern(41, profile.extent());
        let prior_provenance = vec![0x20_u8; profile.extent()];
        render_pattern(&mut graph, first_input, &table, 41).unwrap();
        let current = pattern(99, profile.extent());
        let current_provenance = vec![0x20_u8; profile.extent()];
        let second_input = input(&profile, 1, blur);
        let result = render_pattern(&mut graph, second_input, &table, 99).unwrap();

        let exact_input = IndexedPostprocessInput {
            configured_mode: 2,
            blur_capability: true,
            scratch_present: true,
            ..IndexedPostprocessInput::default()
        };
        let expected = finalize_indexed_world(
            exact_input,
            &current,
            &current_provenance,
            Some(&prior),
            Some(&prior_provenance),
            &table,
        )
        .unwrap();
        assert_eq!(
            result.receipt.blur_branch,
            ClassicHdBlurBranch::Temporal { mode: 2 }
        );
        assert_eq!(graph.current_indices(), expected.indices.as_slice());
        assert_eq!(graph.current_provenance(), expected.provenance.as_slice());
        assert_eq!(
            result.receipt.output_hash,
            byte_hash(graph.current_indices())
        );
    }

    #[test]
    fn repeated_high_resolution_rerenders_have_stable_hashes() {
        let profile = ClassicHdRenderProfile::hd_960x720();
        let table = vec![0_u8; TABLESN_REQUIRED_BYTES];
        let frame_input = input(&profile, 0, ClassicHdBlurInput::default());
        let mut first_graph = ClassicHdRenderGraph::new(profile.clone(), 7).unwrap();
        let mut second_graph = ClassicHdRenderGraph::new(profile.clone(), 7).unwrap();
        let first = render_pattern(&mut first_graph, frame_input, &table, 73).unwrap();
        let second = render_pattern(&mut second_graph, frame_input, &table, 73).unwrap();
        assert_eq!(first_graph.current_indices().len(), 960 * 720);
        assert_eq!(first.receipt.output_hash, second.receipt.output_hash);
        assert_eq!(
            first.receipt.provenance_hash,
            second.receipt.provenance_hash
        );
        assert_eq!(
            first_graph.current_indices(),
            second_graph.current_indices()
        );
        assert_eq!(
            first_graph.current_provenance(),
            second_graph.current_provenance()
        );
    }
}
