//! Source-shaped indexed sprite projection and rasterization for Level 1
//! outdoor world objects.
//!
//! The module covers the qualified selector modes plus the three source sprite
//! calls used by the world painter: primary, reflected-water, and
//! terrain-shadow. Unsupported colour-effect branches remain explicit.

use crate::{
    SIN_16_16,
    goat_draw_admission::GOAT_ORIENTATION_SELECTOR_OFFSETS,
    terrain_presentation::{
        TERRAIN_FOG_END_SQUARED, TERRAIN_FOG_START_SQUARED, TERRAIN_FOG_THICKNESS_SQUARED,
        TERRAIN_RENDER_CUTOFF_SQUARED,
    },
    terrain_reflection::OutdoorProjection,
};

pub const GOAT_MODE20_BASE_SELECTOR: u16 = 152;
pub const GOAT_MODE20_LOGICAL_HEIGHT: i32 = 150;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Mode20SpriteDefinition {
    pub base_selector: u16,
    pub logical_height: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Mode20SpriteCamera {
    pub render_x: u16,
    pub render_y: u16,
    pub eye_z: i32,
    pub yaw: i16,
    pub projection: OutdoorProjection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Mode20SpriteEntity {
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Mode20SpritePlacement {
    pub selector: u16,
    pub orientation_sector: u8,
    pub mirrored: bool,
    pub lateral: i32,
    pub forward_depth: i32,
    pub distance_squared: u32,
    /// Source screen X of the entity point, which is the sprite bottom centre.
    pub bottom_x: i32,
    /// Source screen Y of the entity point, which is the sprite bottom centre.
    pub bottom_y: i32,
    /// Projected width after the source `+1` performed immediately before draw.
    pub projected_width: i32,
    /// Projected height after the source `+1` performed immediately before draw.
    pub projected_height: i32,
    /// Source `dword0x00`: `0x2000` at full brightness, otherwise a fog value.
    pub brightness: i32,
    /// Source special-pivot flag used by modes 21 and 36.
    pub quarter_sum_anchor: bool,
    /// Mode 36 retains the special pivot for its shadow call; mode 21 returns
    /// to the ordinary rotated half-width shadow anchor.
    pub shadow_quarter_sum_anchor: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Mode20SpriteRasterStats {
    pub candidate_pixels: u32,
    pub transparent_pixels: u32,
    pub clipped_pixels: u32,
    pub framebuffer_writes: u32,
}

/// Profile-specific admission limits for the shared source sprite projector.
/// Exact callers use the original constants; Classic HD supplies the
/// profile-scaled cutoffs while retaining the same fixed-point geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Mode20SpriteProjectionLimits {
    pub render_cutoff_squared: u32,
    pub fog_start_squared: u32,
    pub fog_end_squared: u32,
    pub fog_thickness_squared: i64,
}

impl Mode20SpriteProjectionLimits {
    #[must_use]
    pub const fn exact() -> Self {
        Self {
            render_cutoff_squared: TERRAIN_RENDER_CUTOFF_SQUARED,
            fog_start_squared: TERRAIN_FOG_START_SQUARED,
            fog_end_squared: TERRAIN_FOG_END_SQUARED,
            fog_thickness_squared: TERRAIN_FOG_THICKNESS_SQUARED,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode20SpriteReject {
    BehindObjectNearPlane,
    BeyondRenderCutoff,
    EmptySource,
    NonPositiveProjection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceSpriteCompositor {
    /// All source DrawSprite indexed operations, selected by the live world
    /// painter rather than by a creature/spell-specific wrapper.
    Indexed { mode: u8, owner_color: u8 },
    /// `DrawSprite_41BD3` cases 0/1: source index zero is transparent and
    /// nonzero pixels are copied directly or translated through the fog row.
    Standard,
    /// Possession's parameter-209 cases 3/7: blend destination and source
    /// through TABLESN after offset 0x4000 first, then apply the fog row when
    /// required.
    DestinationBlend,
    /// `DrawSprite_41BD3` case 8: the non-zero source index is only a mask.
    /// The selected shadow TABLESN row translates the existing destination.
    ShadowMask,
}

/// The three source call modes used by the outdoor world compositor.
///
/// remc2 passes 1 for the ordinary sprite, 2 for its reflected water image,
/// and 0 for the terrain-projected shadow. These values affect both the
/// screen anchor and vertical source sampling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceSpritePass {
    Primary,
    Reflection,
    Shadow,
}

/// Canonical world consumer for all nine `DrawSprite_41BD3` pixel operations.
/// Clipping bounds the work to the viewport while retaining the original
/// sampling phase, including sprites much larger than the visible screen.
#[allow(clippy::too_many_arguments)]
pub fn rasterize_indexed_source_sprite(
    framebuffer: &mut [u8],
    viewport_width: usize,
    viewport_height: usize,
    clip: crate::classic_hd::ClassicHdViewport,
    placement: Mode20SpritePlacement,
    source_width: u16,
    source_height: u16,
    source_pixels: &[u8],
    tables: &[u8],
    roll: i16,
    pixel_mode: u8,
    owner_color: u8,
    pass: SourceSpritePass,
) -> Result<Mode20SpriteRasterStats, String> {
    if pixel_mode > 8 {
        return Err("source sprite pixel operation exceeds 8".into());
    }
    rasterize_source_sprite_with_clipping(
        framebuffer,
        viewport_width,
        viewport_height,
        placement,
        source_width,
        source_height,
        source_pixels,
        tables,
        roll,
        SourceSpriteCompositor::Indexed {
            mode: pixel_mode,
            owner_color,
        },
        pass,
        true,
        Some(clip),
    )
}

#[must_use]
pub fn mode20_selector(entity_yaw: i16, camera_yaw: i16) -> (u8, u16, bool) {
    mode20_selector_for_base(GOAT_MODE20_BASE_SELECTOR, entity_yaw, camera_yaw)
}

#[must_use]
pub fn mode20_selector_for_base(
    base_selector: u16,
    entity_yaw: i16,
    camera_yaw: i16,
) -> (u8, u16, bool) {
    let orientation_sector = u8::try_from(
        (((i32::from(entity_yaw) - i32::from(camera_yaw.cast_unsigned())) >> 3) & 0x00f0) >> 4,
    )
    .expect("masked mode-20 orientation fits u8");
    let selector = base_selector
        + u16::from(GOAT_ORIENTATION_SELECTOR_OFFSETS[usize::from(orientation_sector)]);
    (orientation_sector, selector, orientation_sector >= 8)
}

/// Reproduce render mode 17's direct eight-facing selector rule.
#[must_use]
pub fn mode17_selector_for_base(
    base_selector: u16,
    entity_yaw: i16,
    camera_yaw: i16,
) -> (u8, u16, bool) {
    let orientation_sector = u8::try_from(
        (((i32::from(entity_yaw) - i32::from(camera_yaw.cast_unsigned())) >> 3) & 0x00f0) >> 4,
    )
    .expect("masked mode-17 orientation fits u8");
    let selector_offset = if orientation_sector < 8 {
        orientation_sector
    } else {
        15 - orientation_sector
    };
    (
        orientation_sector,
        base_selector + u16::from(selector_offset),
        orientation_sector >= 8,
    )
}

#[must_use]
pub fn mode19_selector_for_base(
    base_selector: u16,
    entity_yaw: i16,
    camera_yaw: i16,
) -> (u8, u16, bool) {
    const OFFSETS: [u8; 16] = [0, 1, 1, 2, 2, 3, 3, 4, 4, 3, 3, 2, 2, 1, 1, 0];
    let sector = u8::try_from(
        (((i32::from(entity_yaw) - i32::from(camera_yaw.cast_unsigned())) >> 3) & 0x00f0) >> 4,
    )
    .expect("masked mode-19 orientation fits u8");
    (
        sector,
        base_selector + u16::from(OFFSETS[usize::from(sector)]),
        sector >= 8,
    )
}

#[derive(Clone, Copy)]
enum SourceSpriteSelectorMode {
    Fixed,
    EntityFrame(u8),
    Mode19,
    Mode17,
    Mode20,
}

/// Reproduce the render-mode-20 selection and object projection performed by
/// `DrawSprites_3E360`.
#[must_use]
pub fn project_mode20_sprite(
    camera: Mode20SpriteCamera,
    entity: Mode20SpriteEntity,
    source_width: u16,
    source_height: u16,
) -> Result<Mode20SpritePlacement, Mode20SpriteReject> {
    project_mode20_sprite_with_definition(
        Mode20SpriteDefinition {
            base_selector: GOAT_MODE20_BASE_SELECTOR,
            logical_height: GOAT_MODE20_LOGICAL_HEIGHT,
        },
        camera,
        entity,
        source_width,
        source_height,
    )
}

/// Parameterized render-mode-20 projection shared by Goat and Archer.
#[must_use]
pub fn project_mode20_sprite_with_definition(
    definition: Mode20SpriteDefinition,
    camera: Mode20SpriteCamera,
    entity: Mode20SpriteEntity,
    source_width: u16,
    source_height: u16,
) -> Result<Mode20SpritePlacement, Mode20SpriteReject> {
    project_source_sprite(
        SourceSpriteSelectorMode::Mode20,
        definition,
        camera,
        entity,
        source_width,
        source_height,
        false,
        false,
    )
}

/// Profile-aware mode-20 projection. The compatibility profile should use
/// [`Mode20SpriteProjectionLimits::exact`] and is byte-compatible with the
/// existing wrapper.
#[must_use]
pub fn project_mode20_sprite_with_definition_and_limits(
    definition: Mode20SpriteDefinition,
    camera: Mode20SpriteCamera,
    entity: Mode20SpriteEntity,
    source_width: u16,
    source_height: u16,
    limits: Mode20SpriteProjectionLimits,
) -> Result<Mode20SpritePlacement, Mode20SpriteReject> {
    project_source_sprite_with_limits(
        SourceSpriteSelectorMode::Mode20,
        definition,
        camera,
        entity,
        source_width,
        source_height,
        false,
        false,
        limits,
    )
}

/// Reproduce render mode 17's selector choice and common source sprite
/// projection.
#[must_use]
pub fn project_mode17_sprite_with_definition(
    definition: Mode20SpriteDefinition,
    camera: Mode20SpriteCamera,
    entity: Mode20SpriteEntity,
    source_width: u16,
    source_height: u16,
) -> Result<Mode20SpritePlacement, Mode20SpriteReject> {
    project_source_sprite(
        SourceSpriteSelectorMode::Mode17,
        definition,
        camera,
        entity,
        source_width,
        source_height,
        false,
        false,
    )
}

#[must_use]
pub fn project_mode17_sprite_with_definition_and_limits(
    definition: Mode20SpriteDefinition,
    camera: Mode20SpriteCamera,
    entity: Mode20SpriteEntity,
    source_width: u16,
    source_height: u16,
    limits: Mode20SpriteProjectionLimits,
) -> Result<Mode20SpritePlacement, Mode20SpriteReject> {
    project_source_sprite_with_limits(
        SourceSpriteSelectorMode::Mode17,
        definition,
        camera,
        entity,
        source_width,
        source_height,
        false,
        false,
        limits,
    )
}

/// Project a direct-selector mode 0/1/21 sprite or a base-plus-frame mode
/// 11/36 sprite through the shared source admission and sizing path.
#[must_use]
pub fn project_fixed_sprite_with_definition(
    definition: Mode20SpriteDefinition,
    entity_frame: Option<u8>,
    quarter_sum_anchor: bool,
    camera: Mode20SpriteCamera,
    entity: Mode20SpriteEntity,
    source_width: u16,
    source_height: u16,
) -> Result<Mode20SpritePlacement, Mode20SpriteReject> {
    // The supported direct-selector special branch is mode 21, whose shadow
    // falls back to the ordinary rotated path. The supported base-plus-frame
    // special branch is mode 36, which keeps the square pivot for its shadow.
    let shadow_quarter_sum_anchor = quarter_sum_anchor && entity_frame.is_some();
    project_source_sprite(
        entity_frame.map_or(
            SourceSpriteSelectorMode::Fixed,
            SourceSpriteSelectorMode::EntityFrame,
        ),
        definition,
        camera,
        entity,
        source_width,
        source_height,
        quarter_sum_anchor,
        shadow_quarter_sum_anchor,
    )
}

/// Primary multipart sprites change camera-relative depth after rotation,
/// before near-plane/range admission, sizing and fog. Their earlier shadow
/// and reflection calls continue to use the unmodified source position.
pub(crate) fn project_fixed_sprite_with_depth_adjustment(
    definition: Mode20SpriteDefinition,
    entity_frame: Option<u8>,
    quarter_sum_anchor: bool,
    camera: Mode20SpriteCamera,
    entity: Mode20SpriteEntity,
    source_width: u16,
    source_height: u16,
    depth_adjustment: i16,
) -> Result<Mode20SpritePlacement, Mode20SpriteReject> {
    project_source_sprite_with_depth_adjustment(
        entity_frame.map_or(
            SourceSpriteSelectorMode::Fixed,
            SourceSpriteSelectorMode::EntityFrame,
        ),
        definition,
        camera,
        entity,
        source_width,
        source_height,
        quarter_sum_anchor,
        quarter_sum_anchor && entity_frame.is_some(),
        Mode20SpriteProjectionLimits::exact(),
        depth_adjustment,
    )
}

#[must_use]
pub fn project_fixed_sprite_with_definition_and_limits(
    definition: Mode20SpriteDefinition,
    entity_frame: Option<u8>,
    quarter_sum_anchor: bool,
    camera: Mode20SpriteCamera,
    entity: Mode20SpriteEntity,
    source_width: u16,
    source_height: u16,
    limits: Mode20SpriteProjectionLimits,
) -> Result<Mode20SpritePlacement, Mode20SpriteReject> {
    let shadow_quarter_sum_anchor = quarter_sum_anchor && entity_frame.is_some();
    project_source_sprite_with_limits(
        entity_frame.map_or(
            SourceSpriteSelectorMode::Fixed,
            SourceSpriteSelectorMode::EntityFrame,
        ),
        definition,
        camera,
        entity,
        source_width,
        source_height,
        quarter_sum_anchor,
        shadow_quarter_sum_anchor,
        limits,
    )
}

/// Reproduce mode 19's five-frame directional selection.
#[must_use]
pub fn project_mode19_sprite_with_definition(
    definition: Mode20SpriteDefinition,
    camera: Mode20SpriteCamera,
    entity: Mode20SpriteEntity,
    source_width: u16,
    source_height: u16,
) -> Result<Mode20SpritePlacement, Mode20SpriteReject> {
    project_source_sprite(
        SourceSpriteSelectorMode::Mode19,
        definition,
        camera,
        entity,
        source_width,
        source_height,
        false,
        false,
    )
}

#[must_use]
pub fn project_mode19_sprite_with_definition_and_limits(
    definition: Mode20SpriteDefinition,
    camera: Mode20SpriteCamera,
    entity: Mode20SpriteEntity,
    source_width: u16,
    source_height: u16,
    limits: Mode20SpriteProjectionLimits,
) -> Result<Mode20SpritePlacement, Mode20SpriteReject> {
    project_source_sprite_with_limits(
        SourceSpriteSelectorMode::Mode19,
        definition,
        camera,
        entity,
        source_width,
        source_height,
        false,
        false,
        limits,
    )
}

fn project_source_sprite(
    selector_mode: SourceSpriteSelectorMode,
    definition: Mode20SpriteDefinition,
    camera: Mode20SpriteCamera,
    entity: Mode20SpriteEntity,
    source_width: u16,
    source_height: u16,
    quarter_sum_anchor: bool,
    shadow_quarter_sum_anchor: bool,
) -> Result<Mode20SpritePlacement, Mode20SpriteReject> {
    project_source_sprite_with_limits(
        selector_mode,
        definition,
        camera,
        entity,
        source_width,
        source_height,
        quarter_sum_anchor,
        shadow_quarter_sum_anchor,
        Mode20SpriteProjectionLimits::exact(),
    )
}

/// Shared source near-plane/range admission, before selector rasterization.
/// Returns lateral distance, forward depth and squared planar distance.
pub fn source_sprite_view(
    render_x: u16,
    render_y: u16,
    camera_yaw: i16,
    entity_x: u16,
    entity_y: u16,
    render_cutoff_squared: u32,
) -> Result<(i32, i32, u32), Mode20SpriteReject> {
    source_sprite_view_with_depth_adjustment(
        render_x,
        render_y,
        camera_yaw,
        entity_x,
        entity_y,
        render_cutoff_squared,
        0,
    )
}

pub(crate) fn source_sprite_view_with_depth_adjustment(
    render_x: u16,
    render_y: u16,
    camera_yaw: i16,
    entity_x: u16,
    entity_y: u16,
    render_cutoff_squared: u32,
    depth_adjustment: i16,
) -> Result<(i32, i32, u32), Mode20SpriteReject> {
    let yaw = usize::try_from(i32::from(camera_yaw) & 0x07ff).unwrap();
    let sin_yaw = SIN_16_16[yaw];
    let cos_yaw = SIN_16_16[512 + yaw];
    let dx = i32::from(entity_x.wrapping_sub(render_x).cast_signed());
    let dy = i32::from(render_y.wrapping_sub(entity_y).cast_signed());
    let forward_depth = (sin_yaw
        .wrapping_mul(dx)
        .wrapping_add(cos_yaw.wrapping_mul(dy))
        >> 16)
        .wrapping_add(i32::from(depth_adjustment));
    if forward_depth <= 64 {
        return Err(Mode20SpriteReject::BehindObjectNearPlane);
    }
    let lateral = cos_yaw
        .wrapping_mul(dx)
        .wrapping_sub(sin_yaw.wrapping_mul(dy))
        >> 16;
    let distance_squared = lateral
        .wrapping_mul(lateral)
        .wrapping_add(forward_depth.wrapping_mul(forward_depth))
        .cast_unsigned();
    if distance_squared >= render_cutoff_squared {
        return Err(Mode20SpriteReject::BeyondRenderCutoff);
    }
    Ok((lateral, forward_depth, distance_squared))
}

fn project_source_sprite_with_limits(
    selector_mode: SourceSpriteSelectorMode,
    definition: Mode20SpriteDefinition,
    camera: Mode20SpriteCamera,
    entity: Mode20SpriteEntity,
    source_width: u16,
    source_height: u16,
    quarter_sum_anchor: bool,
    shadow_quarter_sum_anchor: bool,
    limits: Mode20SpriteProjectionLimits,
) -> Result<Mode20SpritePlacement, Mode20SpriteReject> {
    project_source_sprite_with_depth_adjustment(
        selector_mode,
        definition,
        camera,
        entity,
        source_width,
        source_height,
        quarter_sum_anchor,
        shadow_quarter_sum_anchor,
        limits,
        0,
    )
}

fn project_source_sprite_with_depth_adjustment(
    selector_mode: SourceSpriteSelectorMode,
    definition: Mode20SpriteDefinition,
    camera: Mode20SpriteCamera,
    entity: Mode20SpriteEntity,
    source_width: u16,
    source_height: u16,
    quarter_sum_anchor: bool,
    shadow_quarter_sum_anchor: bool,
    limits: Mode20SpriteProjectionLimits,
    depth_adjustment: i16,
) -> Result<Mode20SpritePlacement, Mode20SpriteReject> {
    if source_width == 0 || source_height == 0 {
        return Err(Mode20SpriteReject::EmptySource);
    }
    if definition.logical_height <= 0 {
        return Err(Mode20SpriteReject::NonPositiveProjection);
    }

    let (lateral, forward_depth, distance_squared) = source_sprite_view_with_depth_adjustment(
        camera.render_x,
        camera.render_y,
        camera.yaw,
        entity.x,
        entity.y,
        limits.render_cutoff_squared,
        depth_adjustment,
    )?;

    let brightness = if distance_squared <= limits.fog_start_squared {
        0x2000
    } else if distance_squared < limits.fog_end_squared {
        let remaining = limits.fog_end_squared - distance_squared;
        i32::try_from(32_i64 * i64::from(remaining) / limits.fog_thickness_squared)
            .expect("bounded sprite fog value fits i32")
            .wrapping_shl(8)
    } else {
        // The object cutoff, not the fog end, is the admission boundary.
        // DrawSprite indexes TABLESN row zero for admitted objects beyond fog.
        0
    };

    let projection = camera.projection;
    let projected_lateral = lateral.wrapping_mul(projection.scale) / forward_depth;
    let projected_vertical = projection
        .scale
        .wrapping_mul(i32::from(entity.z).wrapping_sub(camera.eye_z))
        / forward_depth
        + projection.pitch_offset;
    let roll = usize::try_from(i32::from(projection.roll) & 0x07ff)
        .expect("masked source roll fits usize");
    let sin_roll = SIN_16_16[roll];
    let cos_roll = SIN_16_16[512 + roll];
    let bottom_x = projected_lateral
        .wrapping_mul(cos_roll)
        .wrapping_sub(sin_roll.wrapping_mul(projected_vertical))
        .wrapping_shr(16)
        .wrapping_add(projection.center_x);
    let bottom_y = projection.center_y.wrapping_sub(
        sin_roll
            .wrapping_mul(projected_lateral)
            .wrapping_add(projected_vertical.wrapping_mul(cos_roll))
            >> 16,
    );

    let raw_height = projection.scale.wrapping_mul(definition.logical_height) / forward_depth;
    let raw_width = raw_height.wrapping_mul(i32::from(source_width)) / i32::from(source_height);
    // DrawSprites adds one to both dimensions before calling DrawSprite.
    // A distant positive-height effect with a truncated zero extent still
    // draws one pixel; zero logical height was rejected above.
    if raw_width < 0 || raw_height < 0 {
        return Err(Mode20SpriteReject::NonPositiveProjection);
    }

    let (orientation_sector, selector, mirrored) = match selector_mode {
        SourceSpriteSelectorMode::Fixed => (0, definition.base_selector, false),
        SourceSpriteSelectorMode::EntityFrame(frame) => (
            0,
            definition.base_selector.wrapping_add(u16::from(frame)),
            false,
        ),
        SourceSpriteSelectorMode::Mode19 => {
            mode19_selector_for_base(definition.base_selector, entity.yaw, camera.yaw)
        }
        SourceSpriteSelectorMode::Mode17 => {
            mode17_selector_for_base(definition.base_selector, entity.yaw, camera.yaw)
        }
        SourceSpriteSelectorMode::Mode20 => {
            mode20_selector_for_base(definition.base_selector, entity.yaw, camera.yaw)
        }
    };

    Ok(Mode20SpritePlacement {
        selector,
        orientation_sector,
        mirrored,
        lateral,
        forward_depth,
        distance_squared,
        bottom_x,
        bottom_y,
        projected_width: raw_width.wrapping_add(1),
        projected_height: raw_height.wrapping_add(1),
        brightness,
        quarter_sum_anchor,
        shadow_quarter_sum_anchor,
    })
}

/// Reproject an already admitted sprite at a different world altitude.
///
/// Sprite admission, facing, fog, and projected extent depend only on the
/// source XY/depth path. remc2 therefore reuses those values for reflections
/// and shadows and changes only the vertical projection point.
#[must_use]
pub fn reproject_sprite_altitude(
    mut placement: Mode20SpritePlacement,
    camera: Mode20SpriteCamera,
    altitude: i32,
) -> Mode20SpritePlacement {
    let projection = camera.projection;
    let projected_lateral =
        placement.lateral.wrapping_mul(projection.scale) / placement.forward_depth;
    let projected_vertical = projection
        .scale
        .wrapping_mul(altitude.wrapping_sub(camera.eye_z))
        / placement.forward_depth
        + projection.pitch_offset;
    let roll = usize::try_from(i32::from(projection.roll) & 0x07ff)
        .expect("masked source roll fits usize");
    let sin_roll = SIN_16_16[roll];
    let cos_roll = SIN_16_16[512 + roll];
    placement.bottom_x = projected_lateral
        .wrapping_mul(cos_roll)
        .wrapping_sub(sin_roll.wrapping_mul(projected_vertical))
        .wrapping_shr(16)
        .wrapping_add(projection.center_x);
    placement.bottom_y = projection.center_y.wrapping_sub(
        sin_roll
            .wrapping_mul(projected_lateral)
            .wrapping_add(projected_vertical.wrapping_mul(cos_roll))
            >> 16,
    );
    placement
}

/// Build remc2's terrain-shadow projection from the ordinary placement.
///
/// The source calculates the ordinary projected width/height without the
/// primary pass's trailing `+1`, then quarters only the height. The shadow
/// palette row is derived from the pre-shadow fog row and flips around 0x2000
/// between day and night.
pub fn project_sprite_shadow(
    placement: Mode20SpritePlacement,
    camera: Mode20SpriteCamera,
    terrain_altitude: i32,
    night: bool,
) -> Result<Mode20SpritePlacement, Mode20SpriteReject> {
    let mut shadow = reproject_sprite_altitude(placement, camera, terrain_altitude);
    shadow.projected_width = shadow.projected_width.wrapping_sub(1);
    shadow.projected_height = shadow.projected_height.wrapping_sub(1) >> 2;
    if shadow.projected_width <= 0 || shadow.projected_height <= 0 {
        return Err(Mode20SpriteReject::NonPositiveProjection);
    }
    let quarter_brightness = shadow.brightness >> 2;
    shadow.brightness = if night {
        0x2000 - quarter_brightness
    } else {
        0x2000 + quarter_brightness
    };
    Ok(shadow)
}

/// Rasterize one admitted mode-20 sprite into an indexed source framebuffer.
///
/// The inverse fixed-point walk is the coordinate-space equivalent of the
/// source span walker: it starts from the same rounded, roll-aware
/// bottom-centre anchor and retains sampling phase when the viewport clips the
/// sprite. Source index zero is transparent. At reduced fog brightness the
/// source standard branch indexes TABLESN at `brightness | source_index`.
pub fn rasterize_mode20_sprite(
    framebuffer: &mut [u8],
    viewport_width: usize,
    viewport_height: usize,
    placement: Mode20SpritePlacement,
    source_width: u16,
    source_height: u16,
    source_pixels: &[u8],
    tablesn: &[u8],
    roll: i16,
) -> Result<Mode20SpriteRasterStats, String> {
    rasterize_source_sprite(
        framebuffer,
        viewport_width,
        viewport_height,
        placement,
        source_width,
        source_height,
        source_pixels,
        tablesn,
        roll,
        SourceSpriteCompositor::Standard,
        SourceSpritePass::Primary,
    )
}

/// Rasterize the reflected-water call `DrawSprite_41BD3(2)`.
pub fn rasterize_reflected_sprite(
    framebuffer: &mut [u8],
    viewport_width: usize,
    viewport_height: usize,
    placement: Mode20SpritePlacement,
    source_width: u16,
    source_height: u16,
    source_pixels: &[u8],
    tablesn: &[u8],
    roll: i16,
) -> Result<Mode20SpriteRasterStats, String> {
    rasterize_source_sprite(
        framebuffer,
        viewport_width,
        viewport_height,
        placement,
        source_width,
        source_height,
        source_pixels,
        tablesn,
        roll,
        SourceSpriteCompositor::Standard,
        SourceSpritePass::Reflection,
    )
}

/// Rasterize a possession sprite through the reflected-water call while
/// retaining its parameter-209 destination blend.
pub fn rasterize_reflected_possession_sprite(
    framebuffer: &mut [u8],
    viewport_width: usize,
    viewport_height: usize,
    placement: Mode20SpritePlacement,
    source_width: u16,
    source_height: u16,
    source_pixels: &[u8],
    tablesn: &[u8],
    roll: i16,
) -> Result<Mode20SpriteRasterStats, String> {
    rasterize_source_sprite(
        framebuffer,
        viewport_width,
        viewport_height,
        placement,
        source_width,
        source_height,
        source_pixels,
        tablesn,
        roll,
        SourceSpriteCompositor::DestinationBlend,
        SourceSpritePass::Reflection,
    )
}

/// Rasterize the ground-shadow call `DrawSprite_41BD3(0)`.
///
/// `placement.brightness` must already be the source shadow row produced by
/// [`project_sprite_shadow`].
pub fn rasterize_shadow_sprite(
    framebuffer: &mut [u8],
    viewport_width: usize,
    viewport_height: usize,
    placement: Mode20SpritePlacement,
    source_width: u16,
    source_height: u16,
    source_pixels: &[u8],
    tablesn: &[u8],
    roll: i16,
) -> Result<Mode20SpriteRasterStats, String> {
    rasterize_source_sprite(
        framebuffer,
        viewport_width,
        viewport_height,
        placement,
        source_width,
        source_height,
        source_pixels,
        tablesn,
        roll,
        SourceSpriteCompositor::ShadowMask,
        SourceSpritePass::Shadow,
    )
}

/// Rasterize parameter-209 Possession through the exact source cases 3/7.
///
/// At full brightness the address is `0x4000 + (destination << 8 | source)`.
/// In fog, that blended byte is translated through `brightness | blended`.
pub fn rasterize_possession_sprite(
    framebuffer: &mut [u8],
    viewport_width: usize,
    viewport_height: usize,
    placement: Mode20SpritePlacement,
    source_width: u16,
    source_height: u16,
    source_pixels: &[u8],
    tablesn: &[u8],
    roll: i16,
) -> Result<Mode20SpriteRasterStats, String> {
    rasterize_source_sprite(
        framebuffer,
        viewport_width,
        viewport_height,
        placement,
        source_width,
        source_height,
        source_pixels,
        tablesn,
        roll,
        SourceSpriteCompositor::DestinationBlend,
        SourceSpritePass::Primary,
    )
}

#[allow(clippy::too_many_arguments)]
fn rasterize_source_sprite(
    framebuffer: &mut [u8],
    viewport_width: usize,
    viewport_height: usize,
    placement: Mode20SpritePlacement,
    source_width: u16,
    source_height: u16,
    source_pixels: &[u8],
    tablesn: &[u8],
    roll: i16,
    compositor: SourceSpriteCompositor,
    pass: SourceSpritePass,
) -> Result<Mode20SpriteRasterStats, String> {
    rasterize_source_sprite_with_clipping(
        framebuffer,
        viewport_width,
        viewport_height,
        placement,
        source_width,
        source_height,
        source_pixels,
        tablesn,
        roll,
        compositor,
        pass,
        viewport_width > 320 || viewport_height > 200,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn rasterize_source_sprite_with_clipping(
    framebuffer: &mut [u8],
    viewport_width: usize,
    viewport_height: usize,
    placement: Mode20SpritePlacement,
    source_width: u16,
    source_height: u16,
    source_pixels: &[u8],
    tablesn: &[u8],
    roll: i16,
    compositor: SourceSpriteCompositor,
    pass: SourceSpritePass,
    clip_to_profile_viewport: bool,
    source_clip: Option<crate::classic_hd::ClassicHdViewport>,
) -> Result<Mode20SpriteRasterStats, String> {
    let source_extent = usize::from(source_width) * usize::from(source_height);
    if source_width == 0 || source_height == 0 || source_pixels.len() != source_extent {
        return Err(format!(
            "mode-20 source extent mismatch: {}x{} requires {source_extent} bytes, got {}",
            source_width,
            source_height,
            source_pixels.len()
        ));
    }
    let viewport_extent = viewport_width
        .checked_mul(viewport_height)
        .ok_or_else(|| "mode-20 viewport extent overflow".to_owned())?;
    if framebuffer.len() != viewport_extent {
        return Err(format!(
            "mode-20 framebuffer extent mismatch: {viewport_width}x{viewport_height} requires {viewport_extent} bytes, got {}",
            framebuffer.len()
        ));
    }
    if let Some(clip) = source_clip {
        if clip.width == 0 || clip.height == 0
            || u64::from(clip.x) + u64::from(clip.width) > viewport_width as u64
            || u64::from(clip.y) + u64::from(clip.height) > viewport_height as u64
            || u64::from(clip.x) + u64::from(clip.width) > i32::MAX as u64
            || u64::from(clip.y) + u64::from(clip.height) > i32::MAX as u64
        {
            return Err("source sprite clip is outside the indexed target".to_owned());
        }
    }
    let needs_fog = match compositor {
        SourceSpriteCompositor::Indexed { mode, .. } => matches!(mode, 1 | 6 | 7 | 8),
        SourceSpriteCompositor::ShadowMask => true,
        _ => placement.brightness != 0x2000,
    };
    let maximum_fog_address = usize::try_from(placement.brightness)
        .ok()
        .filter(|_| needs_fog)
        .map_or(0, |brightness| brightness | 0xff);
    if needs_fog && tablesn.len() <= maximum_fog_address {
        return Err(format!(
            "source sprite fog translation address {maximum_fog_address:#06x} exceeds {} TABLESN bytes",
            tablesn.len(),
        ));
    }
    let maximum_blend_address = 0x13fff;
    if (compositor == SourceSpriteCompositor::DestinationBlend
        || matches!(
            compositor,
            SourceSpriteCompositor::Indexed { mode: 2..=7, .. }
        ))
        && tablesn.len() <= maximum_blend_address
    {
        return Err(format!(
            "source sprite destination blend address {maximum_blend_address:#07x} exceeds {} TABLESN bytes",
            tablesn.len(),
        ));
    }
    let projected_width = placement.projected_width;
    let projected_height = placement.projected_height;
    if projected_width <= 0 || projected_height <= 0 {
        return Err("mode-20 projected extent must be positive".to_owned());
    }

    let roll_index = usize::try_from(i32::from(roll) & 0x07ff).expect("masked roll fits usize");
    let sin = SIN_16_16[roll_index];
    let cos = SIN_16_16[512 + roll_index];
    let quarter_sum_anchor = placement.quarter_sum_anchor
        && (pass != SourceSpritePass::Shadow || placement.shadow_quarter_sum_anchor);
    let (top_x, top_y) = if quarter_sum_anchor {
        // `x_BYTE_F2CC6 != 0`: modes 21 and 36 use the source's square
        // quarter-sum pivot around the projected entity point. This branch is
        // axis-aligned in remc2; roll changes only the anchor.
        let quarter = projected_width.wrapping_add(projected_height) >> 2;
        match pass {
            SourceSpritePass::Primary => (
                placement
                    .bottom_x
                    .wrapping_sub(sin.wrapping_mul(quarter) >> 16)
                    .wrapping_sub(quarter),
                placement
                    .bottom_y
                    .wrapping_sub(cos.wrapping_mul(quarter) >> 16)
                    .wrapping_sub(quarter),
            ),
            SourceSpritePass::Reflection => (
                placement
                    .bottom_x
                    .wrapping_add(sin.wrapping_mul(quarter) >> 16)
                    .wrapping_sub(quarter),
                placement
                    .bottom_y
                    .wrapping_add(cos.wrapping_mul(quarter) >> 16)
                    .wrapping_sub(quarter),
            ),
            SourceSpritePass::Shadow => (placement.bottom_x, placement.bottom_y),
        }
    } else {
        match pass {
            // Exact a1==1 bottom-centre adjustment in DrawSprite_41BD3.
            SourceSpritePass::Primary => (
                placement.bottom_x.wrapping_sub(
                    ((cos.wrapping_mul(projected_width) >> 1)
                        .wrapping_add(sin.wrapping_mul(projected_height)))
                        >> 16,
                ),
                placement.bottom_y.wrapping_sub(
                    (cos.wrapping_mul(projected_height)
                        .wrapping_sub(sin.wrapping_mul(projected_width) >> 1))
                        >> 16,
                ),
            ),
            // a1==0 and a1==2 share the half-width-only adjustment.
            SourceSpritePass::Reflection | SourceSpritePass::Shadow => (
                placement
                    .bottom_x
                    .wrapping_sub(cos.wrapping_mul(projected_width) >> 17),
                placement
                    .bottom_y
                    .wrapping_add(sin.wrapping_mul(projected_width) >> 17),
            ),
        }
    };

    let transform = |u: i32, v: i32| -> (i32, i32) {
        if quarter_sum_anchor {
            (top_x.wrapping_add(u), top_y.wrapping_add(v))
        } else {
            (
                top_x.wrapping_add(cos.wrapping_mul(u).wrapping_add(sin.wrapping_mul(v)) >> 16),
                top_y.wrapping_add(cos.wrapping_mul(v).wrapping_sub(sin.wrapping_mul(u)) >> 16),
            )
        }
    };
    let corners = [
        transform(0, 0),
        transform(projected_width, 0),
        transform(0, projected_height),
        transform(projected_width, projected_height),
    ];
    let min_x = corners.iter().map(|point| point.0).min().unwrap_or(top_x);
    let max_x = corners.iter().map(|point| point.0).max().unwrap_or(top_x);
    let min_y = corners.iter().map(|point| point.1).min().unwrap_or(top_y);
    let max_y = corners.iter().map(|point| point.1).max().unwrap_or(top_y);
    let mut stats = Mode20SpriteRasterStats::default();

    // The owner Classic HD profiles deliberately admit the larger source
    // lattice and project sprites at the profile scale. A near-plane sprite
    // can therefore have a very large transformed bounding box even when
    // only a small part of it is on screen. Keep the exact 320x200 loop below
    // byte/stat-identical and clip only the profile-sized presentation path
    // before doing the fixed-point inverse walk. This is presentation-space
    // work only: every in-viewport sample retains the same transform, source
    // phase, compositor, and write order.
    let (draw_min_x, draw_max_x, draw_min_y, draw_max_y) = if let Some(clip) = source_clip {
        (
            min_x.max(clip.x as i32),
            max_x.min((clip.x + clip.width - 1) as i32),
            min_y.max(clip.y as i32),
            max_y.min((clip.y + clip.height - 1) as i32),
        )
    } else if clip_to_profile_viewport {
        let viewport_max_x = i32::try_from(viewport_width.saturating_sub(1)).unwrap_or(i32::MAX);
        let viewport_max_y = i32::try_from(viewport_height.saturating_sub(1)).unwrap_or(i32::MAX);
        (
            min_x.max(0),
            max_x.min(viewport_max_x),
            min_y.max(0),
            max_y.min(viewport_max_y),
        )
    } else {
        (min_x, max_x, min_y, max_y)
    };

    if draw_min_x > draw_max_x || draw_min_y > draw_max_y {
        return Ok(stats);
    }

    // Include the transformed maximum edge in the candidate box. The inverse
    // extent checks remove the ordinary positive edge, while retaining the
    // first pixel when an axis runs in the negative screen direction.
    for screen_y in draw_min_y..=draw_max_y {
        for screen_x in draw_min_x..=draw_max_x {
            stats.candidate_pixels = stats.candidate_pixels.wrapping_add(1);
            let delta_x = screen_x.wrapping_sub(top_x);
            let delta_y = screen_y.wrapping_sub(top_y);
            // The special-pivot source branch is axis-aligned. Otherwise
            // invert [cos sin; -sin cos] without converting to floats.
            let (source_plane_x, source_plane_y) = if quarter_sum_anchor {
                (delta_x.wrapping_shl(16), delta_y.wrapping_shl(16))
            } else {
                (
                    cos.wrapping_mul(delta_x)
                        .wrapping_sub(sin.wrapping_mul(delta_y)),
                    sin.wrapping_mul(delta_x)
                        .wrapping_add(cos.wrapping_mul(delta_y)),
                )
            };
            if source_plane_x < 0
                || source_plane_y < 0
                || source_plane_x >= projected_width.wrapping_shl(16)
                || source_plane_y >= projected_height.wrapping_shl(16)
            {
                continue;
            }
            let mut source_x = usize::try_from(
                (i64::from(source_plane_x) * i64::from(source_width) / i64::from(projected_width))
                    >> 16,
            )
            .expect("bounded source X is non-negative");
            let mut source_y = usize::try_from(
                (i64::from(source_plane_y) * i64::from(source_height)
                    / i64::from(projected_height))
                    >> 16,
            )
            .expect("bounded source Y is non-negative");
            if source_x >= usize::from(source_width) || source_y >= usize::from(source_height) {
                continue;
            }
            if placement.mirrored {
                source_x = usize::from(source_width) - 1 - source_x;
            }
            if !quarter_sum_anchor && pass != SourceSpritePass::Primary {
                source_y = usize::from(source_height) - 1 - source_y;
            }
            if screen_x < 0
                || screen_y < 0
                || usize::try_from(screen_x).map_or(true, |x| x >= viewport_width)
                || usize::try_from(screen_y).map_or(true, |y| y >= viewport_height)
            {
                stats.clipped_pixels = stats.clipped_pixels.wrapping_add(1);
                continue;
            }
            let source = source_pixels[source_y * usize::from(source_width) + source_x];
            if source == 0 {
                stats.transparent_pixels = stats.transparent_pixels.wrapping_add(1);
                continue;
            }
            let destination = usize::try_from(screen_y).expect("clipped Y is non-negative")
                * viewport_width
                + usize::try_from(screen_x).expect("clipped X is non-negative");
            let output = match compositor {
                SourceSpriteCompositor::Indexed { mode, owner_color } => {
                    let previous = usize::from(framebuffer[destination]);
                    let source = usize::from(source);
                    let owner = usize::from(owner_color);
                    let brightness = usize::try_from(placement.brightness)
                        .map_err(|_| "source sprite brightness is negative")?;
                    let blend = match mode {
                        0 => source as u8,
                        1 => tablesn[brightness | source],
                        2 | 6 => tablesn[0x4000 + (source << 8 | previous)],
                        3 | 7 => tablesn[0x4000 + (previous << 8 | source)],
                        4 => tablesn[0x4000 + (owner << 8 | source)],
                        5 => tablesn[0x4000 + (source << 8 | owner)],
                        // Source assigns LOBYTE(brightness) = destination;
                        // quarter-fog shadow brightness may retain low bits.
                        8 => tablesn[(brightness & !0xff) | previous],
                        _ => unreachable!("pixel operation validated at the world boundary"),
                    };
                    if matches!(mode, 6 | 7) {
                        tablesn[brightness | usize::from(blend)]
                    } else {
                        blend
                    }
                }
                SourceSpriteCompositor::Standard if placement.brightness == 0x2000 => source,
                SourceSpriteCompositor::Standard => {
                    let address = usize::try_from(placement.brightness)
                        .expect("qualified sprite brightness is non-negative")
                        | usize::from(source);
                    tablesn[address]
                }
                SourceSpriteCompositor::DestinationBlend => {
                    let pair = (usize::from(framebuffer[destination]) << 8) | usize::from(source);
                    let blend_address = 0x4000 + pair;
                    let blended = tablesn[blend_address];
                    if placement.brightness == 0x2000 {
                        blended
                    } else {
                        let fog_address = usize::try_from(placement.brightness)
                            .expect("qualified sprite brightness is non-negative")
                            | usize::from(blended);
                        tablesn[fog_address]
                    }
                }
                SourceSpriteCompositor::ShadowMask => {
                    let address = (usize::try_from(placement.brightness)
                        .expect("qualified shadow brightness is non-negative")
                        & !0xff)
                        | usize::from(framebuffer[destination]);
                    tablesn[address]
                }
            };
            framebuffer[destination] = output;
            stats.framebuffer_writes = stats.framebuffer_writes.wrapping_add(1);
        }
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projection() -> OutdoorProjection {
        OutdoorProjection {
            scale: 256,
            pitch_offset: 0,
            roll: 0,
            center_x: 160,
            center_y: 100,
            viewport_width: 320,
            viewport_height: 200,
        }
    }

    fn camera() -> Mode20SpriteCamera {
        Mode20SpriteCamera {
            render_x: 1_000,
            render_y: 1_000,
            eye_z: 0,
            yaw: 0,
            projection: projection(),
        }
    }

    #[test]
    fn multipart_primary_depth_changes_admission_size_and_fog_without_moving_shadow_basis() {
        let definition = Mode20SpriteDefinition {
            base_selector: 1,
            logical_height: 128,
        };
        let entity = Mode20SpriteEntity {
            x: 1000,
            y: 488,
            z: 100,
            yaw: 0,
        };
        let base =
            project_fixed_sprite_with_definition(definition, None, false, camera(), entity, 2, 2)
                .unwrap();
        let adjusted = project_fixed_sprite_with_depth_adjustment(
            definition,
            None,
            false,
            camera(),
            entity,
            2,
            2,
            390,
        )
        .unwrap();
        assert_eq!(base.forward_depth, 512);
        assert_eq!(adjusted.forward_depth, 902);
        assert_eq!(adjusted.distance_squared, 902 * 902);
        assert!(adjusted.projected_height < base.projected_height);
        assert_eq!(
            base.forward_depth, 512,
            "the earlier shadow/reflection basis remains unchanged"
        );
        let near = Mode20SpriteEntity { y: 937, ..entity };
        assert_eq!(
            project_fixed_sprite_with_definition(definition, None, false, camera(), near, 2, 2),
            Err(Mode20SpriteReject::BehindObjectNearPlane),
        );
        assert_eq!(
            project_fixed_sprite_with_depth_adjustment(
                definition,
                None,
                false,
                camera(),
                near,
                2,
                2,
                390
            )
            .unwrap()
            .forward_depth,
            453,
            "the depth adjustment precedes the primary near-plane test",
        );
        let distant = Mode20SpriteEntity {
            y: 1000_u16.wrapping_sub(4200),
            ..entity
        };
        let far = project_fixed_sprite_with_depth_adjustment(
            definition,
            None,
            false,
            camera(),
            distant,
            2,
            2,
            390,
        )
        .unwrap();
        assert_eq!(far.distance_squared, 4590 * 4590);
        assert_eq!(far.brightness, 0x0900);
        assert_eq!(
            far.brightness as u16,
            crate::campaign::sprite_render::source_sprite_pixel_metadata(0, far.distance_squared)
                .unwrap()
                .brightness,
        );
    }

    #[test]
    fn tiny_positive_source_sprite_keeps_the_trailing_one_pixel() {
        let projected = project_fixed_sprite_with_definition(
            Mode20SpriteDefinition {
                base_selector: 1,
                logical_height: 1,
            },
            None,
            false,
            camera(),
            Mode20SpriteEntity {
                x: 1000,
                y: 0,
                z: 0,
                yaw: 0,
            },
            1,
            1,
        )
        .unwrap();
        assert_eq!(
            (projected.projected_width, projected.projected_height),
            (1, 1)
        );
        assert!(
            project_sprite_shadow(projected, camera(), 0, true).is_err(),
            "the shadow uses unincremented dimensions and must still reject zero"
        );
        let mut frame = vec![0; 320 * 200];
        let result = rasterize_indexed_source_sprite(
            &mut frame,
            320,
            200,
            crate::classic_hd::ClassicHdViewport::full(320, 200),
            projected,
            1,
            1,
            &[7],
            &vec![0; 83456],
            0,
            0,
            0,
            SourceSpritePass::Primary,
        )
        .unwrap();
        assert_eq!(result.framebuffer_writes, 1);
        assert_eq!(frame.iter().filter(|&&pixel| pixel == 7).count(), 1);
    }

    #[test]
    fn source_object_depth_boundary_rejects_64_and_accepts_65() {
        let entity = |depth: u16| Mode20SpriteEntity {
            x: 1_000,
            y: 1_000_u16.wrapping_sub(depth),
            z: 0,
            yaw: 0,
        };
        assert_eq!(
            project_mode20_sprite(camera(), entity(64), 4, 4),
            Err(Mode20SpriteReject::BehindObjectNearPlane)
        );
        let accepted = project_mode20_sprite(camera(), entity(65), 4, 4).expect("depth 65");
        assert_eq!(accepted.forward_depth, 65);
        assert_eq!(accepted.bottom_x, 160);
        assert_eq!(accepted.bottom_y, 100);
    }

    #[test]
    fn all_orientation_sectors_pin_selector_and_mirror_rules() {
        let entity_position = Mode20SpriteEntity {
            x: 1_000,
            y: 500,
            z: 0,
            yaw: 0,
        };
        for sector in 0..16 {
            let mut entity = entity_position;
            entity.yaw = i16::try_from(sector << 7).expect("sector yaw fits i16");
            let placement = project_mode20_sprite(camera(), entity, 4, 4).expect("visible sprite");
            assert_eq!(usize::from(placement.orientation_sector), sector);
            assert_eq!(
                placement.selector,
                152 + u16::from(GOAT_ORIENTATION_SELECTOR_OFFSETS[sector])
            );
            assert_eq!(placement.mirrored, sector >= 8);
        }
    }

    #[test]
    fn parameterized_archer_definition_pins_all_sectors_and_pose_height() {
        let entity_position = Mode20SpriteEntity {
            x: 1_000,
            y: 500,
            z: 0,
            yaw: 0,
        };
        for base_selector in [8_u16, 11, 121] {
            for sector in 0..16 {
                let mut entity = entity_position;
                entity.yaw = i16::try_from(sector << 7).expect("sector yaw fits i16");
                let placement = project_mode20_sprite_with_definition(
                    Mode20SpriteDefinition {
                        base_selector,
                        logical_height: if base_selector == 121 { 240 } else { 250 },
                    },
                    camera(),
                    entity,
                    30,
                    56,
                )
                .expect("visible Archer mode-20 sprite");
                assert_eq!(usize::from(placement.orientation_sector), sector);
                assert_eq!(
                    placement.selector,
                    base_selector + u16::from(GOAT_ORIENTATION_SELECTOR_OFFSETS[sector])
                );
                assert_eq!(placement.mirrored, sector >= 8);
                assert_eq!(
                    placement.projected_height,
                    256 * if base_selector == 121 { 240 } else { 250 } / 500 + 1
                );
            }
        }
    }

    #[test]
    fn mode17_firefly_selector_pins_all_sectors_and_rear_mirroring() {
        let entity_position = Mode20SpriteEntity {
            x: 1_000,
            y: 500,
            z: 0,
            yaw: 0,
        };
        for sector in 0..16 {
            let mut entity = entity_position;
            entity.yaw = i16::try_from(sector << 7).expect("sector yaw fits i16");
            let placement = project_mode17_sprite_with_definition(
                Mode20SpriteDefinition {
                    base_selector: 279,
                    logical_height: 200,
                },
                camera(),
                entity,
                66,
                54,
            )
            .expect("visible Firefly mode-17 sprite");
            let offset = if sector < 8 { sector } else { 15 - sector };
            assert_eq!(usize::from(placement.orientation_sector), sector);
            assert_eq!(placement.selector, 279 + offset as u16);
            assert_eq!(placement.mirrored, sector >= 8);
            assert_eq!(placement.projected_height, 256 * 200 / 500 + 1);
        }
    }

    #[test]
    fn fog_end_selects_table_row_zero_but_only_render_cutoff_rejects() {
        let entity = |depth: u16| Mode20SpriteEntity {
            x: 1_000,
            y: 1_000_u16.wrapping_sub(depth),
            z: 0,
            yaw: 0,
        };
        let fog_end = project_mode20_sprite(camera(), entity(4_864), 4, 4).expect("fog-end sprite");
        assert_eq!(fog_end.distance_squared, TERRAIN_FOG_END_SQUARED);
        assert_eq!(fog_end.brightness, 0);
        let cutoff_minus_one =
            project_mode20_sprite(camera(), entity(5_119), 4, 4).expect("pre-cutoff sprite");
        assert_eq!(cutoff_minus_one.brightness, 0);
        assert_eq!(
            project_mode20_sprite(camera(), entity(5_120), 4, 4),
            Err(Mode20SpriteReject::BeyondRenderCutoff)
        );
    }

    fn placement(mirrored: bool, brightness: i32) -> Mode20SpritePlacement {
        Mode20SpritePlacement {
            selector: 152,
            orientation_sector: if mirrored { 8 } else { 0 },
            mirrored,
            lateral: 0,
            forward_depth: 100,
            distance_squared: 10_000,
            bottom_x: 4,
            bottom_y: 4,
            projected_width: 4,
            projected_height: 2,
            brightness,
            quarter_sum_anchor: false,
            shadow_quarter_sum_anchor: false,
        }
    }

    #[test]
    fn zero_roll_sampling_anchor_transparency_and_mirroring_are_exact() {
        let source = [1, 0, 2, 3, 4, 5, 6, 7];
        let mut normal = [9_u8; 8 * 6];
        let stats = rasterize_mode20_sprite(
            &mut normal,
            8,
            6,
            placement(false, 0x2000),
            4,
            2,
            &source,
            &[],
            0,
        )
        .expect("normal sprite");
        assert_eq!(stats.framebuffer_writes, 7);
        assert_eq!(&normal[2 * 8 + 2..2 * 8 + 6], &[1, 9, 2, 3]);
        assert_eq!(&normal[3 * 8 + 2..3 * 8 + 6], &[4, 5, 6, 7]);

        let mut mirrored = [9_u8; 8 * 6];
        rasterize_mode20_sprite(
            &mut mirrored,
            8,
            6,
            placement(true, 0x2000),
            4,
            2,
            &source,
            &[],
            0,
        )
        .expect("mirrored sprite");
        assert_eq!(&mirrored[2 * 8 + 2..2 * 8 + 6], &[3, 2, 9, 1]);
        assert_eq!(&mirrored[3 * 8 + 2..3 * 8 + 6], &[7, 6, 5, 4]);
    }

    #[test]
    fn reflection_uses_half_width_anchor_and_reverses_ordinary_source_rows() {
        let source = [1, 0, 2, 3, 4, 5, 6, 7];
        let mut frame = [9_u8; 8 * 7];
        let stats = rasterize_reflected_sprite(
            &mut frame,
            8,
            7,
            placement(false, 0x2000),
            4,
            2,
            &source,
            &[],
            0,
        )
        .expect("reflected sprite");
        assert_eq!(stats.framebuffer_writes, 7);
        assert_eq!(&frame[4 * 8 + 2..4 * 8 + 6], &[4, 5, 6, 7]);
        assert_eq!(&frame[5 * 8 + 2..5 * 8 + 6], &[1, 9, 2, 3]);
    }

    #[test]
    fn shadow_projection_removes_primary_plus_one_flattens_and_uses_night_row() {
        let mut projected = placement(false, 0x1000);
        projected.projected_width = 5;
        projected.projected_height = 9;
        let shadow = project_sprite_shadow(projected, camera(), 0, true).expect("positive shadow");
        assert_eq!(shadow.projected_width, 4);
        assert_eq!(shadow.projected_height, 2);
        assert_eq!(shadow.brightness, 0x1c00);

        let day = project_sprite_shadow(projected, camera(), 0, false).expect("positive shadow");
        assert_eq!(day.brightness, 0x2400);
    }

    #[test]
    fn indexed_world_operations_preserve_source_blend_direction_owner_and_fog() {
        let mut tables = vec![0_u8; 83456];
        for (address, value) in [
            (0x1209, 41),
            (0x4000 + (9 << 8 | 77), 31),
            (0x4000 + (77 << 8 | 9), 42),
            (0x4000 + (123 << 8 | 9), 53),
            (0x4000 + (9 << 8 | 123), 64),
            (0x1200 | 31, 71),
            (0x1200 | 42, 82),
            (0x1200 | 77, 93),
        ] {
            tables[address] = value;
        }
        for (mode, expected) in [9, 41, 31, 42, 53, 64, 71, 82, 93].into_iter().enumerate() {
            let mut frame = [77_u8; 16];
            let projected = Mode20SpritePlacement {
                bottom_x: 2,
                bottom_y: 2,
                projected_width: 1,
                projected_height: 1,
                ..placement(false, 0x1200)
            };
            let receipt = rasterize_indexed_source_sprite(
                &mut frame,
                4,
                4,
                crate::classic_hd::ClassicHdViewport::full(4, 4),
                projected,
                1,
                1,
                &[9],
                &tables,
                0,
                mode as u8,
                123,
                SourceSpritePass::Primary,
            )
            .unwrap();
            assert_eq!(receipt.framebuffer_writes, 1, "source operation {mode}");
            assert_eq!(frame[6], expected, "source operation {mode}");
        }
        // The source viewport is an offset subrectangle of a strided target;
        // keep per-pixel composition and sampling identical inside it.
        let clip = crate::classic_hd::ClassicHdViewport { x: 2, y: 1, width: 3, height: 3 };
        let projected = Mode20SpritePlacement {
            bottom_x: 3, bottom_y: 4, projected_width: 6, projected_height: 6,
            ..placement(false, 0x1200)
        };
        for (pass, mode) in [
            (SourceSpritePass::Primary, 0),
            (SourceSpritePass::Reflection, 2),
            (SourceSpritePass::Shadow, 8),
        ] {
            let projected = Mode20SpritePlacement {
                bottom_y: if pass == SourceSpritePass::Primary { 4 } else { 0 },
                ..projected
            };
            let mut full = vec![77_u8; 8 * 6];
            let mut clipped = full.clone();
            for (frame, rect) in [
                (&mut full, crate::classic_hd::ClassicHdViewport::full(8, 6)),
                (&mut clipped, clip),
            ] {
                rasterize_indexed_source_sprite(frame, 8, 6, rect, projected, 1, 1,
                    &[9], &tables, 0, mode, 123, pass).unwrap();
            }
            for y in 0..6_usize {
                for x in 0..8_usize {
                    let inside = (2..5).contains(&x) && (1..4).contains(&y);
                    assert_eq!(clipped[y * 8 + x], if inside { full[y * 8 + x] } else { 77 });
                }
            }
            assert!(clipped.iter().any(|pixel| *pixel != 77), "the pass intersects the source viewport");
        }
    }

    #[test]
    fn shadow_nonzero_source_is_mask_over_existing_destination() {
        let source = [1, 0, 2, 3, 4, 5, 6, 7];
        // Shadow fog is quartered before adding/subtracting 0x2000. Its
        // low byte must be replaced, not ORed into the destination index.
        let mut shadow = placement(false, 0x18c0);
        shadow.bottom_y = 2;
        let mut tablesn = vec![0_u8; 0x1900];
        for destination in 0..256 {
            tablesn[0x1800 | destination] =
                u8::try_from(destination / 2).expect("halved palette index fits u8");
        }
        let mut frame = [100_u8; 8 * 5];
        let stats = rasterize_shadow_sprite(&mut frame, 8, 5, shadow, 4, 2, &source, &tablesn, 0)
            .expect("shadow-mask sprite");
        assert_eq!(stats.framebuffer_writes, 7);
        assert_eq!(&frame[2 * 8 + 2..2 * 8 + 6], &[50, 50, 50, 50]);
        assert_eq!(&frame[3 * 8 + 2..3 * 8 + 6], &[50, 100, 50, 50]);
    }

    #[test]
    fn clipping_retains_sampling_phase_and_fog_uses_source_translation_row() {
        let source = [1, 2, 3, 4, 5, 6, 7, 8];
        let mut clipped_placement = placement(false, 0x1000);
        clipped_placement.bottom_x = 1;
        let mut tablesn = vec![0_u8; 0x1100];
        for index in 0..256 {
            tablesn[0x1000 + index] = u8::try_from(255 - index).expect("table index fits u8");
        }
        let mut frame = [0_u8; 4 * 4];
        let stats = rasterize_mode20_sprite(
            &mut frame,
            4,
            4,
            clipped_placement,
            4,
            2,
            &source,
            &tablesn,
            0,
        )
        .expect("clipped fog sprite");
        assert_eq!(stats.clipped_pixels, 2);
        // The two left source columns are clipped; the visible row begins at
        // source index 3, proving clipping did not restart the sample phase.
        assert_eq!(&frame[2 * 4..2 * 4 + 3], &[253, 252, 251]);
        assert_eq!(&frame[3 * 4..3 * 4 + 3], &[249, 248, 247]);
    }

    #[test]
    fn profile_viewport_clipping_preserves_visible_source_output() {
        let source = [1_u8; 8];
        let mut large_placement = placement(false, 0x2000);
        large_placement.bottom_x = 3;
        large_placement.bottom_y = 4;
        large_placement.projected_width = 200;
        large_placement.projected_height = 100;

        let mut reference = vec![9_u8; 640 * 480];
        let mut clipped = reference.clone();
        let reference_stats = rasterize_source_sprite_with_clipping(
            &mut reference,
            640,
            480,
            large_placement,
            4,
            2,
            &source,
            &[],
            0,
            SourceSpriteCompositor::Standard,
            SourceSpritePass::Primary,
            false,
            None,
        )
        .expect("unclipped profile reference");
        let clipped_stats = rasterize_source_sprite_with_clipping(
            &mut clipped,
            640,
            480,
            large_placement,
            4,
            2,
            &source,
            &[],
            0,
            SourceSpriteCompositor::Standard,
            SourceSpritePass::Primary,
            true,
            None,
        )
        .expect("profile-clipped raster");

        assert_eq!(clipped, reference);
        assert_eq!(
            clipped_stats.framebuffer_writes,
            reference_stats.framebuffer_writes
        );
        assert!(clipped_stats.candidate_pixels < reference_stats.candidate_pixels);
    }

    #[test]
    fn possession_full_brightness_blends_destination_then_source() {
        let source = [1_u8; 8];
        let mut tablesn = vec![0_u8; 0x14000];
        // Destination 0x40 makes addition observably different from OR:
        // 0x4000 + 0x4001 = 0x8001, while OR would incorrectly use 0x4001.
        tablesn[0x4000 + ((0x40 << 8) | 1)] = 77;
        let mut frame = [0x40_u8; 8 * 6];
        let stats = rasterize_possession_sprite(
            &mut frame,
            8,
            6,
            placement(false, 0x2000),
            4,
            2,
            &source,
            &tablesn,
            0,
        )
        .expect("full-brightness Possession sprite");
        assert_eq!(stats.framebuffer_writes, 8);
        assert_eq!(&frame[2 * 8 + 2..2 * 8 + 6], &[77; 4]);
        assert_eq!(&frame[3 * 8 + 2..3 * 8 + 6], &[77; 4]);
    }

    #[test]
    fn possession_fog_blends_before_fog_row_translation() {
        let source = [1_u8; 8];
        let mut tablesn = vec![0_u8; 0x14000];
        tablesn[0x4000 + ((0x40 << 8) | 1)] = 77;
        tablesn[0x1000 | 77] = 88;
        let mut frame = [0x40_u8; 8 * 6];
        rasterize_possession_sprite(
            &mut frame,
            8,
            6,
            placement(false, 0x1000),
            4,
            2,
            &source,
            &tablesn,
            0,
        )
        .expect("fogged Possession sprite");
        assert_eq!(&frame[2 * 8 + 2..2 * 8 + 6], &[88; 4]);
        assert_eq!(&frame[3 * 8 + 2..3 * 8 + 6], &[88; 4]);
    }

    #[test]
    fn roll_aware_bottom_centre_anchor_rotates_without_float_state() {
        let source = [1_u8; 8];
        let mut rolled = [0_u8; 10 * 10];
        let stats = rasterize_mode20_sprite(
            &mut rolled,
            10,
            10,
            Mode20SpritePlacement {
                bottom_x: 5,
                bottom_y: 5,
                ..placement(false, 0x2000)
            },
            4,
            2,
            &source,
            &[],
            512,
        )
        .expect("quarter-turn sprite");
        assert_eq!(stats.framebuffer_writes, 8);
        assert_eq!(
            rolled.iter().filter(|pixel| **pixel == 1).count(),
            usize::try_from(stats.framebuffer_writes).expect("write count fits usize")
        );
    }

    #[test]
    fn mode21_shadow_uses_normal_rotated_anchor_while_mode36_keeps_special_pivot() {
        let definition = Mode20SpriteDefinition {
            base_selector: 61,
            logical_height: 150,
        };
        let entity = Mode20SpriteEntity {
            x: 1_000,
            y: 800,
            z: 0,
            yaw: 0,
        };
        let mode21 =
            project_fixed_sprite_with_definition(definition, None, true, camera(), entity, 4, 4)
                .expect("mode-21 projection");
        assert!(mode21.quarter_sum_anchor);
        assert!(!mode21.shadow_quarter_sum_anchor);

        let mode36 =
            project_fixed_sprite_with_definition(definition, Some(0), true, camera(), entity, 4, 4)
                .expect("mode-36 projection");
        assert!(mode36.quarter_sum_anchor);
        assert!(mode36.shadow_quarter_sum_anchor);

        let source = [1_u8; 8];
        let mut tablesn = [0_u8; 256];
        tablesn[0] = 7;
        let base = Mode20SpritePlacement {
            bottom_x: 5,
            bottom_y: 5,
            projected_width: 4,
            projected_height: 2,
            brightness: 0,
            quarter_sum_anchor: true,
            ..placement(false, 0)
        };
        let mut normal_shadow = [0_u8; 10 * 10];
        rasterize_shadow_sprite(
            &mut normal_shadow,
            10,
            10,
            Mode20SpritePlacement {
                shadow_quarter_sum_anchor: false,
                ..base
            },
            4,
            2,
            &source,
            &tablesn,
            512,
        )
        .expect("mode-21 normal rotated shadow");
        let mut special_shadow = [0_u8; 10 * 10];
        rasterize_shadow_sprite(
            &mut special_shadow,
            10,
            10,
            Mode20SpritePlacement {
                shadow_quarter_sum_anchor: true,
                ..base
            },
            4,
            2,
            &source,
            &tablesn,
            512,
        )
        .expect("mode-36 special-pivot shadow");
        assert_ne!(normal_shadow, special_shadow);
    }
}
