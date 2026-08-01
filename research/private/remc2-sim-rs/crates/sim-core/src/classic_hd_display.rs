//! Classic HD window composition and source-style HUD geometry.
//!
//! Classic HD renders an indexed 4:3 game surface first and only then fits
//! that surface into the desktop output.  This module owns the second step:
//! it never changes the internal profile, the world projection, or the
//! indexed generation hash.  The HUD is expressed in the source-style 640x480
//! coordinate space used by remc2's HD path and is centered in profiles that
//! have more room than that base canvas.

use super::classic_hd::{ClassicHdError, ClassicHdRenderProfile};

pub const CLASSIC_HD_HUD_WIDTH: u32 = 640;
pub const CLASSIC_HD_HUD_HEIGHT: u32 = 480;
pub const EXACT_HUD_WIDTH: u32 = 320;
pub const EXACT_HUD_HEIGHT: u32 = 200;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl ClassicHdRect {
    #[must_use]
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub const fn right(self) -> u32 {
        self.x.saturating_add(self.width)
    }

    #[must_use]
    pub const fn bottom(self) -> u32 {
        self.y.saturating_add(self.height)
    }

    #[must_use]
    pub const fn contains(self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdOutputExtent {
    pub width: u32,
    pub height: u32,
}

impl ClassicHdOutputExtent {
    fn validate(self) -> Result<Self, ClassicHdDisplayError> {
        if self.width == 0 || self.height == 0 {
            return Err(ClassicHdDisplayError(
                "Classic HD output dimensions must be non-zero".to_owned(),
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassicHdWindowFilter {
    /// Preserve indexed pixel edges when the window scale is integral.
    Point,
    /// Let the presentation surface smooth only the already-generated frame.
    Linear,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdBars {
    pub left: ClassicHdRect,
    pub right: ClassicHdRect,
    pub top: ClassicHdRect,
    pub bottom: ClassicHdRect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdDisplayLayout {
    pub output: ClassicHdOutputExtent,
    /// The only desktop rectangle receiving the profile-sized indexed frame.
    pub content: ClassicHdRect,
    pub bars: ClassicHdBars,
    /// Internal profile rectangle used by the world and HUD renderers.
    pub internal_viewport: ClassicHdRect,
    /// Source-style UI canvas inside the internal profile viewport.
    pub hud_canvas: ClassicHdRect,
    /// remc2's source-style integer UI scale, independent of desktop fit.
    pub game_ui_scale: u32,
    pub filter: ClassicHdWindowFilter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassicHdHudSpace {
    /// The 320x200 coordinate system used by the sealed compatibility path.
    Compatibility320x200,
    /// The source-style 640x480 coordinate system used by Classic HD.
    ClassicHd640x480,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ClassicHdHudLayer {
    DeadPlayerMenu,
    SpiderWeb,
    MinimapTerrain,
    MinimapEntities,
    MinimapMarks,
    LeftSpell,
    RightSpell,
    TopStatus,
    Menu,
    PauseOrEndText,
    BottomSpellMenu,
    ObjectiveTextbox,
    TutorialOrBriefing,
    HelpPopup,
    DebugText,
    MapClear,
    MapTerrain,
    MapEntities,
    MapWorldViewport,
    MapDividers,
    MapMarks,
}

pub const CLASSIC_HD_HUD_LAYER_ORDER: [ClassicHdHudLayer; 21] = [
    ClassicHdHudLayer::DeadPlayerMenu,
    ClassicHdHudLayer::SpiderWeb,
    ClassicHdHudLayer::MinimapTerrain,
    ClassicHdHudLayer::MinimapEntities,
    ClassicHdHudLayer::MinimapMarks,
    ClassicHdHudLayer::LeftSpell,
    ClassicHdHudLayer::RightSpell,
    ClassicHdHudLayer::TopStatus,
    ClassicHdHudLayer::Menu,
    ClassicHdHudLayer::PauseOrEndText,
    ClassicHdHudLayer::BottomSpellMenu,
    ClassicHdHudLayer::ObjectiveTextbox,
    ClassicHdHudLayer::TutorialOrBriefing,
    ClassicHdHudLayer::HelpPopup,
    ClassicHdHudLayer::DebugText,
    ClassicHdHudLayer::MapClear,
    ClassicHdHudLayer::MapTerrain,
    ClassicHdHudLayer::MapEntities,
    ClassicHdHudLayer::MapWorldViewport,
    ClassicHdHudLayer::MapDividers,
    ClassicHdHudLayer::MapMarks,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdDisplayComposition {
    profile_revision: u64,
    source_width: u32,
    source_height: u32,
    layout: ClassicHdDisplayLayout,
}

impl ClassicHdDisplayComposition {
    /// Build a presentation layout without changing the indexed render
    /// profile.  `output_width` and `output_height` are desktop pixels.
    pub fn new(
        profile: &ClassicHdRenderProfile,
        output_width: u32,
        output_height: u32,
        filter: ClassicHdWindowFilter,
    ) -> Result<Self, ClassicHdDisplayError> {
        let output = ClassicHdOutputExtent {
            width: output_width,
            height: output_height,
        }
        .validate()?;
        if profile.display_aspect_num == 0 || profile.display_aspect_den == 0 {
            return Err(ClassicHdDisplayError(
                "Classic HD profile display aspect must be non-zero".to_owned(),
            ));
        }

        let content_width_by_height = u64::from(output.height)
            .checked_mul(u64::from(profile.display_aspect_num))
            .ok_or_else(|| {
                ClassicHdDisplayError("Classic HD output aspect multiplication overflow".to_owned())
            })?
            / u64::from(profile.display_aspect_den);
        let content_height_by_width = u64::from(output.width)
            .checked_mul(u64::from(profile.display_aspect_den))
            .ok_or_else(|| {
                ClassicHdDisplayError("Classic HD output aspect multiplication overflow".to_owned())
            })?
            / u64::from(profile.display_aspect_num);
        let content_width = u32::try_from(u64::from(output.width).min(content_width_by_height))
            .map_err(|_| {
                ClassicHdDisplayError("Classic HD content width does not fit u32".to_owned())
            })?;
        let content_height = u32::try_from(u64::from(output.height).min(content_height_by_width))
            .map_err(|_| {
            ClassicHdDisplayError("Classic HD content height does not fit u32".to_owned())
        })?;
        if content_width == 0 || content_height == 0 {
            return Err(ClassicHdDisplayError(
                "Classic HD output is too small for a 4:3 content rectangle".to_owned(),
            ));
        }
        let content_x = (output.width - content_width) / 2;
        let content_y = (output.height - content_height) / 2;
        let content = ClassicHdRect::new(content_x, content_y, content_width, content_height);
        let bars = ClassicHdBars {
            left: ClassicHdRect::new(0, 0, content.x, output.height),
            right: ClassicHdRect::new(
                content.right(),
                0,
                output.width.saturating_sub(content.right()),
                output.height,
            ),
            top: ClassicHdRect::new(content.x, 0, content.width, content.y),
            bottom: ClassicHdRect::new(
                content.x,
                content.bottom(),
                content.width,
                output.height.saturating_sub(content.bottom()),
            ),
        };

        let internal_viewport = ClassicHdRect::new(
            profile.viewport.x,
            profile.viewport.y,
            profile.viewport.width,
            profile.viewport.height,
        );
        let hud_space = if profile.is_compatibility_profile() {
            ClassicHdHudSpace::Compatibility320x200
        } else {
            ClassicHdHudSpace::ClassicHd640x480
        };
        let (hud_width, hud_height, game_ui_scale) = match hud_space {
            ClassicHdHudSpace::Compatibility320x200 => (EXACT_HUD_WIDTH, EXACT_HUD_HEIGHT, 1),
            ClassicHdHudSpace::ClassicHd640x480 => {
                let horizontal = profile.width / CLASSIC_HD_HUD_WIDTH;
                let vertical = profile.height / CLASSIC_HD_HUD_HEIGHT;
                let scale = horizontal.min(vertical).clamp(1, 8);
                (
                    CLASSIC_HD_HUD_WIDTH * scale,
                    CLASSIC_HD_HUD_HEIGHT * scale,
                    scale,
                )
            }
        };
        if hud_width > internal_viewport.width || hud_height > internal_viewport.height {
            return Err(ClassicHdDisplayError(
                "Classic HD HUD canvas does not fit the internal viewport".to_owned(),
            ));
        }
        let hud_canvas = ClassicHdRect::new(
            internal_viewport.x + (internal_viewport.width - hud_width) / 2,
            internal_viewport.y + (internal_viewport.height - hud_height) / 2,
            hud_width,
            hud_height,
        );

        Ok(Self {
            profile_revision: profile.profile_revision,
            source_width: profile.width,
            source_height: profile.height,
            layout: ClassicHdDisplayLayout {
                output,
                content,
                bars,
                internal_viewport,
                hud_canvas,
                game_ui_scale,
                filter,
            },
        })
    }

    #[must_use]
    pub const fn profile_revision(&self) -> u64 {
        self.profile_revision
    }

    #[must_use]
    pub const fn source_width(&self) -> u32 {
        self.source_width
    }

    #[must_use]
    pub const fn source_height(&self) -> u32 {
        self.source_height
    }

    #[must_use]
    pub const fn layout(&self) -> ClassicHdDisplayLayout {
        self.layout
    }

    #[must_use]
    pub const fn hud_space(&self) -> ClassicHdHudSpace {
        if self.layout.hud_canvas.width == EXACT_HUD_WIDTH
            && self.layout.hud_canvas.height == EXACT_HUD_HEIGHT
            && self.layout.game_ui_scale == 1
            && self.source_width == EXACT_HUD_WIDTH
            && self.source_height == EXACT_HUD_HEIGHT
        {
            ClassicHdHudSpace::Compatibility320x200
        } else {
            ClassicHdHudSpace::ClassicHd640x480
        }
    }

    /// Map an internal source pixel to the desktop pixel containing its
    /// top-left corner.  This is a boundary transform, not a texture sample.
    #[must_use]
    pub fn internal_boundary_to_output(&self, x: u32, y: u32) -> Option<(u32, u32)> {
        if x > self.source_width || y > self.source_height {
            return None;
        }
        Some((
            self.layout.content.x + scale_boundary(x, self.source_width, self.layout.content.width),
            self.layout.content.y
                + scale_boundary(y, self.source_height, self.layout.content.height),
        ))
    }

    /// Map a desktop pixel to an internal source pixel.  Bars intentionally
    /// return `None`, so mouse input cannot accidentally steer the world from
    /// outside the maintained 4:3 surface.
    #[must_use]
    pub fn output_pixel_to_internal(&self, x: u32, y: u32) -> Option<(u32, u32)> {
        if !self.layout.content.contains(x, y) {
            return None;
        }
        let local_x = x - self.layout.content.x;
        let local_y = y - self.layout.content.y;
        Some((
            inverse_boundary(local_x, self.layout.content.width, self.source_width),
            inverse_boundary(local_y, self.layout.content.height, self.source_height),
        ))
    }

    /// Map a source-style HUD boundary into the internal profile.  The
    /// returned rectangle is suitable for an indexed overlay writer and does
    /// not involve desktop bars or a presentation filter.
    pub fn hud_boundary_rect(
        &self,
        space: ClassicHdHudSpace,
        rect: ClassicHdRect,
    ) -> Result<ClassicHdRect, ClassicHdDisplayError> {
        if space != self.hud_space() {
            return Err(ClassicHdDisplayError(
                "Classic HD HUD coordinate space does not match the selected profile".to_owned(),
            ));
        }
        let (base_width, base_height, scale) = self.hud_metrics(space);
        if rect.right() > base_width || rect.bottom() > base_height {
            return Err(ClassicHdDisplayError(
                "Classic HD HUD rectangle is outside its source coordinate space".to_owned(),
            ));
        }
        let x = self
            .layout
            .hud_canvas
            .x
            .checked_add(rect.x.checked_mul(scale).ok_or_else(|| {
                ClassicHdDisplayError("Classic HD HUD x scale overflow".to_owned())
            })?)
            .ok_or_else(|| ClassicHdDisplayError("Classic HD HUD x overflow".to_owned()))?;
        let y = self
            .layout
            .hud_canvas
            .y
            .checked_add(rect.y.checked_mul(scale).ok_or_else(|| {
                ClassicHdDisplayError("Classic HD HUD y scale overflow".to_owned())
            })?)
            .ok_or_else(|| ClassicHdDisplayError("Classic HD HUD y overflow".to_owned()))?;
        let width = rect.width.checked_mul(scale).ok_or_else(|| {
            ClassicHdDisplayError("Classic HD HUD width scale overflow".to_owned())
        })?;
        let height = rect.height.checked_mul(scale).ok_or_else(|| {
            ClassicHdDisplayError("Classic HD HUD height scale overflow".to_owned())
        })?;
        Ok(ClassicHdRect::new(x, y, width, height))
    }

    /// Convert a desktop pointer to source-style HUD coordinates.  A pointer
    /// outside the HUD canvas or inside a desktop bar is not a UI hit.
    #[must_use]
    pub fn output_pixel_to_hud(&self, x: u32, y: u32) -> Option<(ClassicHdHudSpace, u32, u32)> {
        let (internal_x, internal_y) = self.output_pixel_to_internal(x, y)?;
        if !self.layout.hud_canvas.contains(internal_x, internal_y) {
            return None;
        }
        let (space, _, _, scale) = self.hud_metrics_with_space();
        Some((
            space,
            (internal_x - self.layout.hud_canvas.x) / scale,
            (internal_y - self.layout.hud_canvas.y) / scale,
        ))
    }

    #[must_use]
    pub const fn hud_layer_order() -> &'static [ClassicHdHudLayer; 21] {
        &CLASSIC_HD_HUD_LAYER_ORDER
    }

    fn hud_metrics(&self, space: ClassicHdHudSpace) -> (u32, u32, u32) {
        match space {
            ClassicHdHudSpace::Compatibility320x200 => (EXACT_HUD_WIDTH, EXACT_HUD_HEIGHT, 1),
            ClassicHdHudSpace::ClassicHd640x480 => (
                CLASSIC_HD_HUD_WIDTH,
                CLASSIC_HD_HUD_HEIGHT,
                self.layout.game_ui_scale,
            ),
        }
    }

    fn hud_metrics_with_space(&self) -> (ClassicHdHudSpace, u32, u32, u32) {
        let space = self.hud_space();
        let (width, height, scale) = self.hud_metrics(space);
        (space, width, height, scale)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassicHdDisplayError(String);

impl std::fmt::Display for ClassicHdDisplayError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ClassicHdDisplayError {}

impl From<ClassicHdDisplayError> for ClassicHdError {
    fn from(error: ClassicHdDisplayError) -> Self {
        Self(error.0)
    }
}

#[must_use]
fn scale_boundary(value: u32, source_extent: u32, output_extent: u32) -> u32 {
    if value == source_extent {
        return output_extent;
    }
    u32::try_from((u64::from(value) * u64::from(output_extent)) / u64::from(source_extent))
        .expect("bounded display boundary fits u32")
}

#[must_use]
fn inverse_boundary(value: u32, output_extent: u32, source_extent: u32) -> u32 {
    let mapped = (u64::from(value) * u64::from(source_extent)) / u64::from(output_extent);
    u32::try_from(mapped.min(u64::from(source_extent.saturating_sub(1))))
        .expect("bounded source boundary fits u32")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classic_hd::ClassicHdRenderProfile;

    #[test]
    fn exact_4x3_output_has_no_bars_and_preserves_profile_revision() {
        let profile = ClassicHdRenderProfile::hd_640x480();
        let composition =
            ClassicHdDisplayComposition::new(&profile, 640, 480, ClassicHdWindowFilter::Point)
                .expect("640x480 composition");
        let layout = composition.layout();
        assert_eq!(layout.content, ClassicHdRect::new(0, 0, 640, 480));
        assert_eq!(layout.bars.left.width, 0);
        assert_eq!(layout.bars.right.width, 0);
        assert_eq!(layout.bars.top.height, 0);
        assert_eq!(layout.bars.bottom.height, 0);
        assert_eq!(layout.game_ui_scale, 1);
        assert_eq!(layout.hud_canvas, ClassicHdRect::new(0, 0, 640, 480));
        assert_eq!(composition.profile_revision(), profile.profile_revision);
    }

    #[test]
    fn widescreen_outputs_are_centered_without_world_or_hud_reflow() {
        let profile = ClassicHdRenderProfile::hd_640x480();
        let composition =
            ClassicHdDisplayComposition::new(&profile, 1920, 1080, ClassicHdWindowFilter::Linear)
                .expect("16:9 composition");
        let layout = composition.layout();
        assert_eq!(layout.content, ClassicHdRect::new(240, 0, 1440, 1080));
        assert_eq!(layout.bars.left, ClassicHdRect::new(0, 0, 240, 1080));
        assert_eq!(layout.bars.right, ClassicHdRect::new(1680, 0, 240, 1080));
        assert_eq!(layout.bars.top.height, 0);
        assert_eq!(layout.bars.bottom.height, 0);
        assert_eq!(layout.hud_canvas, ClassicHdRect::new(0, 0, 640, 480));
        assert_eq!(layout.filter, ClassicHdWindowFilter::Linear);
    }

    #[test]
    fn ultrawide_and_small_windows_keep_the_4x3_surface() {
        let profile = ClassicHdRenderProfile::hd_640x480();
        let ultrawide =
            ClassicHdDisplayComposition::new(&profile, 3440, 1440, ClassicHdWindowFilter::Point)
                .expect("ultrawide composition");
        assert_eq!(
            ultrawide.layout().content,
            ClassicHdRect::new(760, 0, 1920, 1440)
        );

        let small =
            ClassicHdDisplayComposition::new(&profile, 640, 360, ClassicHdWindowFilter::Point)
                .expect("small composition");
        assert_eq!(small.layout().content, ClassicHdRect::new(80, 0, 480, 360));
        assert_eq!(small.layout().bars.left.width, 80);
        assert_eq!(small.layout().bars.right.width, 80);
    }

    #[test]
    fn pointer_edges_round_trip_and_bars_are_not_game_input() {
        let profile = ClassicHdRenderProfile::hd_640x480();
        let composition =
            ClassicHdDisplayComposition::new(&profile, 1920, 1080, ClassicHdWindowFilter::Point)
                .expect("composition");
        assert_eq!(composition.output_pixel_to_internal(239, 540), None);
        assert_eq!(composition.output_pixel_to_internal(240, 0), Some((0, 0)));
        assert_eq!(
            composition.output_pixel_to_internal(1679, 1079),
            Some((639, 479))
        );
        assert_eq!(
            composition.internal_boundary_to_output(0, 0),
            Some((240, 0))
        );
        assert_eq!(
            composition.internal_boundary_to_output(640, 480),
            Some((1680, 1080))
        );
        let center = composition
            .internal_boundary_to_output(320, 240)
            .expect("center boundary");
        assert_eq!(
            composition.output_pixel_to_internal(center.0, center.1),
            Some((320, 240))
        );
    }

    #[test]
    fn integer_hud_scale_is_profile_local_and_resizes_do_not_change_generation() {
        let profile = ClassicHdRenderProfile::hd_1280x960();
        let wide =
            ClassicHdDisplayComposition::new(&profile, 2560, 1440, ClassicHdWindowFilter::Point)
                .expect("wide composition");
        let narrow =
            ClassicHdDisplayComposition::new(&profile, 1280, 960, ClassicHdWindowFilter::Point)
                .expect("native composition");
        assert_eq!(wide.layout().game_ui_scale, 2);
        assert_eq!(narrow.layout().game_ui_scale, 2);
        assert_eq!(
            narrow.layout().hud_canvas,
            ClassicHdRect::new(0, 0, 1280, 960)
        );
        assert_eq!(wide.profile_revision(), narrow.profile_revision());
        assert_eq!(
            wide.hud_boundary_rect(
                ClassicHdHudSpace::ClassicHd640x480,
                ClassicHdRect::new(10, 20, 30, 40),
            )
            .expect("scaled HUD rectangle"),
            ClassicHdRect::new(20, 40, 60, 80)
        );
    }

    #[test]
    fn intermediate_profile_centers_the_source_hud_canvas() {
        let profile = ClassicHdRenderProfile::hd_960x720();
        let composition =
            ClassicHdDisplayComposition::new(&profile, 960, 720, ClassicHdWindowFilter::Point)
                .expect("960x720 composition");
        let layout = composition.layout();
        assert_eq!(layout.game_ui_scale, 1);
        assert_eq!(layout.hud_canvas, ClassicHdRect::new(160, 120, 640, 480));
        assert_eq!(
            composition
                .hud_boundary_rect(
                    ClassicHdHudSpace::ClassicHd640x480,
                    ClassicHdRect::new(0, 0, 640, 480),
                )
                .expect("full HUD canvas"),
            layout.hud_canvas
        );
    }

    #[test]
    fn compatibility_keeps_the_320x200_ui_space_separate() {
        let profile = ClassicHdRenderProfile::compatibility_320x200();
        let composition =
            ClassicHdDisplayComposition::new(&profile, 1280, 720, ClassicHdWindowFilter::Point)
                .expect("compatibility composition");
        assert_eq!(
            composition.hud_space(),
            ClassicHdHudSpace::Compatibility320x200
        );
        assert_eq!(
            composition.layout().hud_canvas,
            ClassicHdRect::new(0, 0, 320, 200)
        );
        assert_eq!(
            composition
                .hud_boundary_rect(
                    ClassicHdHudSpace::Compatibility320x200,
                    ClassicHdRect::new(1, 2, 3, 4),
                )
                .expect("compatibility HUD rectangle"),
            ClassicHdRect::new(1, 2, 3, 4)
        );
        assert!(
            composition
                .hud_boundary_rect(
                    ClassicHdHudSpace::ClassicHd640x480,
                    ClassicHdRect::new(639, 479, 1, 1),
                )
                .is_err()
        );
    }

    #[test]
    fn hud_pointer_transform_ignores_bars_and_returns_source_coordinates() {
        let profile = ClassicHdRenderProfile::hd_640x480();
        let composition =
            ClassicHdDisplayComposition::new(&profile, 1920, 1080, ClassicHdWindowFilter::Point)
                .expect("composition");
        assert_eq!(
            composition.output_pixel_to_hud(240, 0),
            Some((ClassicHdHudSpace::ClassicHd640x480, 0, 0,))
        );
        assert_eq!(composition.output_pixel_to_hud(239, 0), None);
        assert_eq!(
            composition.output_pixel_to_hud(960, 540),
            Some((ClassicHdHudSpace::ClassicHd640x480, 320, 240,))
        );
    }

    #[test]
    fn all_overlay_layers_have_one_deterministic_order() {
        let layers = ClassicHdDisplayComposition::hud_layer_order();
        assert_eq!(layers.len(), 21);
        for pair in layers.windows(2) {
            assert!(pair[0] < pair[1]);
        }
        assert_eq!(layers[2], ClassicHdHudLayer::MinimapTerrain);
        assert_eq!(layers[10], ClassicHdHudLayer::BottomSpellMenu);
        assert_eq!(layers[20], ClassicHdHudLayer::MapMarks);
    }

    #[test]
    fn invalid_output_is_rejected_before_layout_creation() {
        let profile = ClassicHdRenderProfile::hd_640x480();
        assert!(
            ClassicHdDisplayComposition::new(&profile, 0, 1080, ClassicHdWindowFilter::Point)
                .is_err()
        );
        assert!(
            ClassicHdDisplayComposition::new(&profile, 1, 1, ClassicHdWindowFilter::Point).is_err()
        );
    }
}
