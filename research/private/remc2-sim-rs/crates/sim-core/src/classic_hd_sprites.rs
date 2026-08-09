//! Resolution-aware Classic HD source-sprite planning.
//!
//! This adapter keeps the source-qualified disposition and painter order from
//! `source_sprite_tmap`, but feeds profile-scaled projection limits into the
//! existing fixed-point sprite projector. The compatibility profile uses the
//! Exact limits and geometry; HD profiles change only the projection extent
//! and admission envelope. Unity and high-resolution asset selection remain
//! outside this module.

use crate::{
    Position,
    classic_hd::ClassicHdRenderProfile,
    classic_hd_terrain::ClassicHdProjection,
    goat_sprite_raster::{
        Mode20SpriteCamera, Mode20SpriteDefinition, Mode20SpriteEntity, Mode20SpritePlacement,
        Mode20SpriteProjectionLimits, Mode20SpriteRasterStats, Mode20SpriteReject,
        project_fixed_sprite_with_definition_and_limits,
        project_mode17_sprite_with_definition_and_limits,
        project_mode19_sprite_with_definition_and_limits,
        project_mode20_sprite_with_definition_and_limits, project_sprite_shadow,
        rasterize_mode20_sprite, rasterize_possession_sprite,
        rasterize_reflected_possession_sprite, rasterize_reflected_sprite, rasterize_shadow_sprite,
        reproject_sprite_altitude,
    },
    legacy_terrain::interpolate_ground,
    level1_mission::POSSESSION_PROJECTILE_RENDER_INDEX,
    source_sprite_tmap::{
        Level1SourceSpawnKind, SourceSpriteDisposition, SourceSpriteExclusionReason,
        SourceSpriteLegacyFamily, SourceSpriteSubmission, SourceSpriteTmapBank,
        legacy_route_for_spawn_kind,
    },
    terrain_frame::Level1TerrainFrameCamera,
    terrain_reflection::OutdoorProjection,
};

pub const CLASSIC_HD_SOURCE_MAP_BYTES: usize = 256 * 256;
pub const CLASSIC_HD_MAX_SPRITE_PLAN_ENTRIES: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdSpriteCamera {
    pub profile_revision: u64,
    pub source: Mode20SpriteCamera,
    pub limits: Mode20SpriteProjectionLimits,
    pub night: bool,
}

impl ClassicHdSpriteCamera {
    #[must_use]
    pub fn from_profile(
        profile: &ClassicHdRenderProfile,
        camera: Level1TerrainFrameCamera,
        night: bool,
    ) -> Self {
        let projection = ClassicHdProjection::from_profile(profile, camera);
        let source_projection = OutdoorProjection {
            scale: projection.scale,
            pitch_offset: projection.pitch_offset,
            roll: projection.roll,
            center_x: projection.center_x,
            center_y: projection.center_y,
            viewport_width: projection.viewport_width,
            viewport_height: projection.viewport_height,
        };
        let fog_thickness_squared = i64::from(
            projection
                .fog_end_squared
                .saturating_sub(projection.fog_start_squared),
        )
        .max(1);
        Self {
            profile_revision: profile.profile_revision,
            source: Mode20SpriteCamera {
                render_x: camera.render_x,
                render_y: camera.render_y,
                eye_z: camera.eye_z,
                yaw: camera.yaw,
                projection: source_projection,
            },
            limits: Mode20SpriteProjectionLimits {
                render_cutoff_squared: projection.render_cutoff_squared,
                fog_start_squared: projection.fog_start_squared,
                fog_end_squared: projection.fog_end_squared,
                fog_thickness_squared,
            },
            night,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassicHdSpritePass {
    Primary,
    Reflection,
    Shadow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdSpritePassSubmission {
    pub source_entity_index: u16,
    pub position: Position,
    pub selector: u16,
    pub render_mode: u8,
    pub render_parameter_index: u16,
    pub source_width: u16,
    pub source_height: u16,
    pub roll: i16,
    pub control_flags: u32,
    pub placement: Mode20SpritePlacement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdLegacySpriteSubmission {
    pub source_entity_index: u16,
    pub family: SourceSpriteLegacyFamily,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdExcludedSpriteSubmission {
    pub source_entity_index: u16,
    pub reason: SourceSpriteExclusionReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdCulledSpriteSubmission {
    pub source_entity_index: u16,
    pub reason: Mode20SpriteReject,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassicHdSpritePlan {
    pub primary: Vec<ClassicHdSpritePassSubmission>,
    pub reflections: Vec<ClassicHdSpritePassSubmission>,
    pub shadows: Vec<ClassicHdSpritePassSubmission>,
    pub legacy: Vec<ClassicHdLegacySpriteSubmission>,
    pub excluded: Vec<ClassicHdExcludedSpriteSubmission>,
    pub culled: Vec<ClassicHdCulledSpriteSubmission>,
    pub order_hash: u64,
}

impl ClassicHdSpritePlan {
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            primary: Vec::with_capacity(capacity),
            reflections: Vec::with_capacity(capacity),
            shadows: Vec::with_capacity(capacity),
            legacy: Vec::with_capacity(capacity),
            excluded: Vec::with_capacity(capacity),
            culled: Vec::with_capacity(capacity),
            order_hash: 0,
        }
    }

    pub fn reset(&mut self) {
        self.primary.clear();
        self.reflections.clear();
        self.shadows.clear();
        self.legacy.clear();
        self.excluded.clear();
        self.culled.clear();
        self.order_hash = 0;
    }

    fn push<T>(target: &mut Vec<T>, value: T, label: &str) -> Result<(), ClassicHdSpriteError> {
        if target.len() == target.capacity() {
            return Err(ClassicHdSpriteError(format!(
                "Classic HD {label} plan capacity {} exhausted",
                target.capacity()
            )));
        }
        target.push(value);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassicHdTypedFamily {
    Source(Level1SourceSpawnKind),
    Goat,
}

pub const CLASSIC_HD_TYPED_FAMILIES: [ClassicHdTypedFamily; 36] = [
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::Reserved),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::ImportedResetTuple),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::AuthoredActivationTuple),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::AuthoredTerrainFeature),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::AuthoredPlayerMarker),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::AuthoredSwitch),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::PlayerFireball),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::ArcherArrow),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::FireflyFireball),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::CastleBuild),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::CastleRoot),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::CastleConstruction),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::CastleDefenseController),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::CastleDefenseShot),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::CastleAssociation),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::Possession),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::PossessionImpact),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::GroundFire),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::PossessionAttraction),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::StrongPossessionImpact),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::FireballImpact),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::WaterSplash),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::ManaSphere),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::ManaCarrier),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::DeathBurst),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::PlayerDeathMarker),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::SpeedTrail),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::CheckpointSmoke),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::SpellPickup),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::Ambient),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::MultipartSegment),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::ResidentArcher),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::ResidentBuilder),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::ResidentVillager),
    ClassicHdTypedFamily::Source(Level1SourceSpawnKind::ResidentTrader),
    ClassicHdTypedFamily::Goat,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassicHdCoverageRoute {
    Renderer(SourceSpriteLegacyFamily),
    ExcludedReserved,
    Excluded(SourceSpriteExclusionReason),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassicHdSpriteError(pub String);

impl std::fmt::Display for ClassicHdSpriteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ClassicHdSpriteError {}

impl From<Mode20SpriteReject> for ClassicHdSpriteError {
    fn from(error: Mode20SpriteReject) -> Self {
        Self(format!("Classic HD sprite projection rejected: {error:?}"))
    }
}

pub fn classic_hd_coverage_route(
    family: ClassicHdTypedFamily,
) -> Result<ClassicHdCoverageRoute, ClassicHdSpriteError> {
    match family {
        ClassicHdTypedFamily::Goat => Ok(ClassicHdCoverageRoute::Renderer(
            SourceSpriteLegacyFamily::Goat,
        )),
        ClassicHdTypedFamily::Source(Level1SourceSpawnKind::Reserved) => {
            Ok(ClassicHdCoverageRoute::ExcludedReserved)
        }
        ClassicHdTypedFamily::Source(kind) => {
            match legacy_route_for_spawn_kind(kind).map_err(ClassicHdSpriteError)? {
                crate::source_sprite_tmap::SourceSpriteContractRoute::Legacy(family) => {
                    Ok(ClassicHdCoverageRoute::Renderer(family))
                }
                crate::source_sprite_tmap::SourceSpriteContractRoute::Excluded(reason) => {
                    Ok(ClassicHdCoverageRoute::Excluded(reason))
                }
                crate::source_sprite_tmap::SourceSpriteContractRoute::Render { .. } => {
                    Err(ClassicHdSpriteError(
                        "Classic HD coverage encountered an unclassified render route".to_owned(),
                    ))
                }
            }
        }
    }
}

/// Reproject the complete typed source disposition list into one reusable
/// Classic HD plan. The caller owns the plan capacity, so a steady-state
/// frame can reset and refill it without heap growth.
pub fn build_classic_hd_sprite_plan(
    profile: &ClassicHdRenderProfile,
    camera: Level1TerrainFrameCamera,
    dispositions: &[SourceSpriteDisposition],
    tmap_bank: &SourceSpriteTmapBank,
    ground_heights: &[u8],
    night: bool,
    plan: &mut ClassicHdSpritePlan,
) -> Result<(), ClassicHdSpriteError> {
    if dispositions.len() > CLASSIC_HD_MAX_SPRITE_PLAN_ENTRIES {
        return Err(ClassicHdSpriteError(format!(
            "Classic HD source disposition count {} exceeds {}",
            dispositions.len(),
            CLASSIC_HD_MAX_SPRITE_PLAN_ENTRIES
        )));
    }
    if ground_heights.len() != CLASSIC_HD_SOURCE_MAP_BYTES {
        return Err(ClassicHdSpriteError(format!(
            "Classic HD sprite ground plane requires {} bytes, got {}",
            CLASSIC_HD_SOURCE_MAP_BYTES,
            ground_heights.len()
        )));
    }
    let sprite_camera = ClassicHdSpriteCamera::from_profile(profile, camera, night);
    plan.reset();
    let mut previous_source = None;
    for disposition in dispositions {
        let source_entity_index = disposition_source_index(*disposition);
        if source_entity_index == 0 {
            return Err(ClassicHdSpriteError(
                "Classic HD source sprite slot zero is reserved".to_owned(),
            ));
        }
        if previous_source.is_some_and(|previous| source_entity_index <= previous) {
            return Err(ClassicHdSpriteError(format!(
                "Classic HD source dispositions are not strictly source ordered at slot {source_entity_index}"
            )));
        }
        previous_source = Some(source_entity_index);
        match *disposition {
            SourceSpriteDisposition::Legacy {
                source_entity_index,
                family,
            } => {
                ClassicHdSpritePlan::push(
                    &mut plan.legacy,
                    ClassicHdLegacySpriteSubmission {
                        source_entity_index,
                        family,
                    },
                    "legacy",
                )?;
                mix_order(&mut plan.order_hash, source_entity_index, 1, family as u16);
            }
            SourceSpriteDisposition::Excluded {
                source_entity_index,
                reason,
            } => {
                ClassicHdSpritePlan::push(
                    &mut plan.excluded,
                    ClassicHdExcludedSpriteSubmission {
                        source_entity_index,
                        reason,
                    },
                    "excluded",
                )?;
                mix_order(&mut plan.order_hash, source_entity_index, 2, reason as u16);
            }
            SourceSpriteDisposition::Render(submission) => {
                let pass = project_submission(submission, sprite_camera, tmap_bank)?;
                match pass {
                    Some(primary) => {
                        let source_entity_index = primary.source_entity_index;
                        mix_order(
                            &mut plan.order_hash,
                            source_entity_index,
                            3,
                            primary.selector,
                        );
                        ClassicHdSpritePlan::push(&mut plan.primary, primary, "primary")?;
                        if submission.reflection_eligible {
                            let mut reflected = primary;
                            reflected.placement = reproject_sprite_altitude(
                                primary.placement,
                                sprite_camera.source,
                                -i32::from(submission.position.z),
                            );
                            ClassicHdSpritePlan::push(
                                &mut plan.reflections,
                                reflected,
                                "reflection",
                            )?;
                        }
                        if submission.shadow_eligible
                            && let Ok(shadow) = project_sprite_shadow(
                                primary.placement,
                                sprite_camera.source,
                                interpolate_ground(
                                    ground_heights,
                                    submission.position.x,
                                    submission.position.y,
                                ),
                                sprite_camera.night,
                            )
                        {
                            let mut shadow_submission = primary;
                            shadow_submission.placement = shadow;
                            ClassicHdSpritePlan::push(
                                &mut plan.shadows,
                                shadow_submission,
                                "shadow",
                            )?;
                        }
                    }
                    None => {
                        let selector = submission.base_tmap;
                        let reason = Mode20SpriteReject::BeyondRenderCutoff;
                        ClassicHdSpritePlan::push(
                            &mut plan.culled,
                            ClassicHdCulledSpriteSubmission {
                                source_entity_index: submission.source_entity_index,
                                reason,
                            },
                            "culled",
                        )?;
                        mix_order(
                            &mut plan.order_hash,
                            submission.source_entity_index,
                            4,
                            selector,
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn disposition_source_index(disposition: SourceSpriteDisposition) -> u16 {
    match disposition {
        SourceSpriteDisposition::Render(submission) => submission.source_entity_index,
        SourceSpriteDisposition::Legacy {
            source_entity_index,
            ..
        }
        | SourceSpriteDisposition::Excluded {
            source_entity_index,
            ..
        } => source_entity_index,
    }
}

fn project_submission(
    submission: SourceSpriteSubmission,
    camera: ClassicHdSpriteCamera,
    tmap_bank: &SourceSpriteTmapBank,
) -> Result<Option<ClassicHdSpritePassSubmission>, ClassicHdSpriteError> {
    let (lookup_selector, definition_base) = match submission.render_mode {
        17 => (
            crate::goat_sprite_raster::mode17_selector_for_base(
                submission.base_tmap,
                submission.yaw,
                camera.source.yaw,
            )
            .1,
            submission.base_tmap,
        ),
        19 => (
            crate::goat_sprite_raster::mode19_selector_for_base(
                submission.base_tmap,
                submission.yaw,
                camera.source.yaw,
            )
            .1,
            submission.base_tmap,
        ),
        20 => (
            crate::goat_sprite_raster::mode20_selector_for_base(
                submission.base_tmap,
                submission.yaw,
                camera.source.yaw,
            )
            .1,
            submission.base_tmap,
        ),
        0 | 1 | 11 | 21 | 36 => {
            let selector = if submission.selected_frame == 0 {
                submission.base_tmap
            } else {
                submission.selected_frame
            };
            (selector, selector)
        }
        mode => {
            return Err(ClassicHdSpriteError(format!(
                "source sprite slot {} uses unsupported render mode {mode}",
                submission.source_entity_index
            )));
        }
    };
    let frame = tmap_bank.frame(lookup_selector).ok_or_else(|| {
        ClassicHdSpriteError(format!(
            "Classic HD source sprite slot {} selector {lookup_selector} has no qualified TMAP frame",
            submission.source_entity_index
        ))
    })?;
    let entity = Mode20SpriteEntity {
        x: submission.position.x,
        y: submission.position.y,
        z: submission.position.z,
        yaw: submission.yaw,
    };
    let definition = Mode20SpriteDefinition {
        base_selector: definition_base,
        logical_height: i32::from(submission.logical_height),
    };
    let placement = match submission.render_mode {
        17 => project_mode17_sprite_with_definition_and_limits(
            definition,
            camera.source,
            entity,
            frame.width,
            frame.height,
            camera.limits,
        ),
        19 => project_mode19_sprite_with_definition_and_limits(
            definition,
            camera.source,
            entity,
            frame.width,
            frame.height,
            camera.limits,
        ),
        20 => project_mode20_sprite_with_definition_and_limits(
            definition,
            camera.source,
            entity,
            frame.width,
            frame.height,
            camera.limits,
        ),
        _ => project_fixed_sprite_with_definition_and_limits(
            definition,
            None,
            matches!(submission.render_mode, 21 | 36),
            camera.source,
            entity,
            frame.width,
            frame.height,
            camera.limits,
        ),
    };
    let Ok(placement) = placement else {
        return Ok(None);
    };
    Ok(Some(ClassicHdSpritePassSubmission {
        source_entity_index: submission.source_entity_index,
        position: submission.position,
        selector: placement.selector,
        render_mode: submission.render_mode,
        render_parameter_index: submission.render_parameter_index,
        source_width: frame.width,
        source_height: frame.height,
        roll: camera.source.projection.roll.wrapping_add(submission.roll),
        control_flags: submission.control_flags,
        placement,
    }))
}

/// Rasterize one already-planned pass into a profile-sized indexed buffer.
/// The same source kernels are used for compatibility and HD profiles.
pub fn rasterize_classic_hd_sprite_pass(
    profile: &ClassicHdRenderProfile,
    pass: ClassicHdSpritePass,
    submission: &ClassicHdSpritePassSubmission,
    tmap_bank: &SourceSpriteTmapBank,
    framebuffer: &mut [u8],
    tablesn: &[u8],
) -> Result<Mode20SpriteRasterStats, ClassicHdSpriteError> {
    let width = usize::try_from(profile.width).unwrap();
    let height = usize::try_from(profile.height).unwrap();
    if profile.stride != profile.width || framebuffer.len() != profile.extent() {
        return Err(ClassicHdSpriteError(
            "Classic HD sprite raster requires a tightly packed profile buffer".to_owned(),
        ));
    }
    let frame = tmap_bank.frame(submission.selector).ok_or_else(|| {
        ClassicHdSpriteError(format!(
            "Classic HD sprite selector {} disappeared before rasterization",
            submission.selector
        ))
    })?;
    if frame.width != submission.source_width || frame.height != submission.source_height {
        return Err(ClassicHdSpriteError(
            "Classic HD sprite source extent changed between planning and rasterization".to_owned(),
        ));
    }
    let possession = submission.render_parameter_index == POSSESSION_PROJECTILE_RENDER_INDEX;
    let result = match pass {
        ClassicHdSpritePass::Primary if possession => rasterize_possession_sprite(
            framebuffer,
            width,
            height,
            submission.placement,
            frame.width,
            frame.height,
            &frame.pixels,
            tablesn,
            submission.roll,
        ),
        ClassicHdSpritePass::Primary => rasterize_mode20_sprite(
            framebuffer,
            width,
            height,
            submission.placement,
            frame.width,
            frame.height,
            &frame.pixels,
            tablesn,
            submission.roll,
        ),
        ClassicHdSpritePass::Reflection if possession => rasterize_reflected_possession_sprite(
            framebuffer,
            width,
            height,
            submission.placement,
            frame.width,
            frame.height,
            &frame.pixels,
            tablesn,
            submission.roll,
        ),
        ClassicHdSpritePass::Reflection => rasterize_reflected_sprite(
            framebuffer,
            width,
            height,
            submission.placement,
            frame.width,
            frame.height,
            &frame.pixels,
            tablesn,
            submission.roll,
        ),
        ClassicHdSpritePass::Shadow => rasterize_shadow_sprite(
            framebuffer,
            width,
            height,
            submission.placement,
            frame.width,
            frame.height,
            &frame.pixels,
            tablesn,
            submission.roll,
        ),
    };
    result.map_err(ClassicHdSpriteError)
}

fn mix_order(hash: &mut u64, source_entity_index: u16, route: u8, selector: u16) {
    let source_bytes = source_entity_index.to_le_bytes();
    let selector_bytes = selector.to_le_bytes();
    let bytes = [
        source_bytes[0],
        source_bytes[1],
        route,
        selector_bytes[0],
        selector_bytes[1],
    ];
    let part = crate::classic_hd::byte_hash(&bytes);
    *hash = hash.rotate_left(7) ^ part.wrapping_add(0x9e37_79b9_7f4a_7c15);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        classic_hd::{
            ClassicHdAssetTier, ClassicHdFiltering, ClassicHdPresentPolicy, ClassicHdProfileId,
            ClassicHdViewport,
        },
        goat_sprite_raster::{Mode20SpriteCamera, project_fixed_sprite_with_definition},
        source_sprite_tmap::{
            SourceSpriteAnimationOwner, SourceSpritePaletteOwner, SourceSpriteTmapFrame,
        },
        terrain_frame::level1_projection,
    };

    fn profile(id: ClassicHdProfileId) -> ClassicHdRenderProfile {
        let (width, height) = id.dimensions();
        ClassicHdRenderProfile::try_new(
            id,
            width,
            height,
            width,
            ClassicHdViewport::full(width, height),
            ClassicHdFiltering::SourceKernel,
            ClassicHdPresentPolicy::Corrected4x3,
            ClassicHdAssetTier::OriginalIndexed,
        )
        .expect("profile")
    }

    fn camera() -> Level1TerrainFrameCamera {
        Level1TerrainFrameCamera {
            render_x: 1_000,
            render_y: 1_000,
            eye_z: 0,
            yaw: 0,
            pitch: 0,
            roll: 0,
            fov: 256,
            source_turn: 0,
            reflections: true,
        }
    }

    fn bank() -> SourceSpriteTmapBank {
        SourceSpriteTmapBank::new(vec![SourceSpriteTmapFrame {
            selector: 80,
            width: 4,
            height: 4,
            pixels: vec![1; 16].into_boxed_slice(),
        }])
        .expect("bank")
    }

    fn render_submission(source_entity_index: u16) -> SourceSpriteDisposition {
        SourceSpriteDisposition::Render(SourceSpriteSubmission {
            source_entity_index,
            position: Position {
                x: 1_000,
                y: 800,
                z: 0,
            },
            yaw: 0,
            pitch: 0,
            roll: 0,
            control_flags: 0,
            class: 3,
            model: 0,
            action: 0,
            xclass: 0,
            xmodel: 0,
            render_parameter_index: 66,
            base_tmap: 80,
            selected_frame: 80,
            render_mode: 0,
            logical_height: 200,
            animation_owner: SourceSpriteAnimationOwner::SourceEntity,
            palette_owner: SourceSpritePaletteOwner::PlayerColour,
            shadow_eligible: true,
            reflection_eligible: true,
        })
    }

    #[test]
    fn compatibility_projection_is_the_existing_projection_and_hd_scales_it() {
        let bank = bank();
        let ground = vec![0; CLASSIC_HD_SOURCE_MAP_BYTES];
        let dispositions = [render_submission(7)];
        let mut exact = ClassicHdSpritePlan::with_capacity(4);
        build_classic_hd_sprite_plan(
            &profile(ClassicHdProfileId::Compatibility320x200),
            camera(),
            &dispositions,
            &bank,
            &ground,
            true,
            &mut exact,
        )
        .expect("exact plan");
        let old_camera = Mode20SpriteCamera {
            render_x: camera().render_x,
            render_y: camera().render_y,
            eye_z: camera().eye_z,
            yaw: camera().yaw,
            projection: level1_projection(camera()),
        };
        let old = project_fixed_sprite_with_definition(
            Mode20SpriteDefinition {
                base_selector: 80,
                logical_height: 200,
            },
            None,
            false,
            old_camera,
            Mode20SpriteEntity {
                x: 1_000,
                y: 800,
                z: 0,
                yaw: 0,
            },
            4,
            4,
        )
        .expect("old projection");
        assert_eq!(exact.primary[0].placement, old);

        let mut hd = ClassicHdSpritePlan::with_capacity(4);
        build_classic_hd_sprite_plan(
            &profile(ClassicHdProfileId::Hd640x480),
            camera(),
            &dispositions,
            &bank,
            &ground,
            true,
            &mut hd,
        )
        .expect("hd plan");
        assert!(hd.primary[0].placement.projected_width > old.projected_width);
        assert!(hd.primary[0].placement.projected_height > old.projected_height);
        assert_eq!(hd.primary[0].placement.selector, 80);

        let tablesn = vec![0; 0x14_000];
        let mut compatibility_frame = vec![0; 320 * 200];
        let mut exact_frame = vec![0; 320 * 200];
        let compatibility_stats = rasterize_classic_hd_sprite_pass(
            &profile(ClassicHdProfileId::Compatibility320x200),
            ClassicHdSpritePass::Primary,
            &exact.primary[0],
            &bank,
            &mut compatibility_frame,
            &tablesn,
        )
        .expect("compatibility raster");
        let exact_stats = crate::goat_sprite_raster::rasterize_mode20_sprite(
            &mut exact_frame,
            320,
            200,
            old,
            4,
            4,
            &bank.frame(80).expect("frame").pixels,
            &tablesn,
            0,
        )
        .expect("exact raster");
        assert_eq!(compatibility_stats, exact_stats);
        assert_eq!(compatibility_frame, exact_frame);
    }

    #[test]
    fn source_order_reflection_shadow_and_capacity_are_explicit() {
        let bank = bank();
        let ground = vec![0; CLASSIC_HD_SOURCE_MAP_BYTES];
        let dispositions = [
            render_submission(7),
            SourceSpriteDisposition::Legacy {
                source_entity_index: 8,
                family: SourceSpriteLegacyFamily::Resident,
            },
            SourceSpriteDisposition::Excluded {
                source_entity_index: 9,
                reason: SourceSpriteExclusionReason::HiddenSourceModel,
            },
        ];
        let mut plan = ClassicHdSpritePlan::with_capacity(8);
        build_classic_hd_sprite_plan(
            &profile(ClassicHdProfileId::Hd640x480),
            camera(),
            &dispositions,
            &bank,
            &ground,
            true,
            &mut plan,
        )
        .expect("ordered plan");
        assert_eq!(plan.primary.len(), 1);
        assert_eq!(plan.reflections.len(), 1);
        assert_eq!(plan.shadows.len(), 1);
        assert_eq!(plan.legacy.len(), 1);
        assert_eq!(plan.excluded.len(), 1);
        assert_ne!(plan.order_hash, 0);
        let pointers = (
            plan.primary.as_ptr(),
            plan.reflections.as_ptr(),
            plan.shadows.as_ptr(),
        );
        plan.reset();
        build_classic_hd_sprite_plan(
            &profile(ClassicHdProfileId::Hd640x480),
            camera(),
            &dispositions,
            &bank,
            &ground,
            false,
            &mut plan,
        )
        .expect("reused plan");
        assert_eq!(
            pointers,
            (
                plan.primary.as_ptr(),
                plan.reflections.as_ptr(),
                plan.shadows.as_ptr()
            )
        );
        let mut too_small = ClassicHdSpritePlan::with_capacity(0);
        assert!(
            build_classic_hd_sprite_plan(
                &profile(ClassicHdProfileId::Hd640x480),
                camera(),
                &dispositions,
                &bank,
                &ground,
                false,
                &mut too_small,
            )
            .is_err()
        );
    }

    #[test]
    fn all_typed_level1_families_are_renderer_or_explicit_exclusion() {
        let routes = CLASSIC_HD_TYPED_FAMILIES
            .iter()
            .copied()
            .map(classic_hd_coverage_route)
            .collect::<Result<Vec<_>, _>>()
            .expect("coverage routes");
        assert_eq!(routes.len(), 36);
        assert_eq!(
            routes
                .iter()
                .filter(|route| matches!(route, ClassicHdCoverageRoute::Renderer(_)))
                .count(),
            30
        );
        assert_eq!(
            routes
                .iter()
                .filter(|route| {
                    matches!(
                        route,
                        ClassicHdCoverageRoute::ExcludedReserved
                            | ClassicHdCoverageRoute::Excluded(_)
                    )
                })
                .count(),
            6
        );
    }

    #[test]
    fn raster_pass_uses_profile_extent_and_reports_writes() {
        let profile = profile(ClassicHdProfileId::Hd640x480);
        let bank = bank();
        let ground = vec![0; CLASSIC_HD_SOURCE_MAP_BYTES];
        let dispositions = [render_submission(7)];
        let mut plan = ClassicHdSpritePlan::with_capacity(4);
        build_classic_hd_sprite_plan(
            &profile,
            camera(),
            &dispositions,
            &bank,
            &ground,
            true,
            &mut plan,
        )
        .expect("plan");
        let mut framebuffer = vec![0; profile.extent()];
        let tablesn = vec![0; 0x14_000];
        let stats = rasterize_classic_hd_sprite_pass(
            &profile,
            ClassicHdSpritePass::Primary,
            &plan.primary[0],
            &bank,
            &mut framebuffer,
            &tablesn,
        )
        .expect("raster");
        assert!(stats.framebuffer_writes > 0);
        assert!(stats.candidate_pixels >= stats.framebuffer_writes);
    }
}
