//! Checked, resolution-independent indexed presentation primitives.
//!
//! This module is deliberately additive.  The fixed `terrain_frame` and
//! `PresentationFrameState` types remain the Exact production path; Classic HD
//! owns these profiles and buffers so a higher-resolution request cannot
//! parameterize or mutate the sealed 320x200 state.

use std::fmt;

use serde_json::{Value, json};

pub const Q16_ONE: u32 = 1 << 16;
pub const MAX_CLASSIC_HD_DIMENSION: u32 = 4_096;
pub const MAX_CLASSIC_HD_PIXELS: u64 = 16 * 1024 * 1024;
pub const MAX_CLASSIC_HD_MEMORY_BYTES: u64 = 128 * 1024 * 1024;
const STORAGE_PLANES: u64 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassicHdProfileId {
    Compatibility320x200,
    Hd640x480,
    Hd960x720,
    Hd1280x960,
}

impl ClassicHdProfileId {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compatibility320x200 => "compatibility-320x200",
            Self::Hd640x480 => "classic-hd-640x480",
            Self::Hd960x720 => "classic-hd-960x720",
            Self::Hd1280x960 => "classic-hd-1280x960",
        }
    }

    #[must_use]
    pub const fn dimensions(self) -> (u32, u32) {
        match self {
            Self::Compatibility320x200 => (320, 200),
            Self::Hd640x480 => (640, 480),
            Self::Hd960x720 => (960, 720),
            Self::Hd1280x960 => (1280, 960),
        }
    }

    fn parse(value: &str) -> Result<Self, ClassicHdError> {
        match value {
            "compatibility-320x200" => Ok(Self::Compatibility320x200),
            "classic-hd-640x480" => Ok(Self::Hd640x480),
            "classic-hd-960x720" => Ok(Self::Hd960x720),
            "classic-hd-1280x960" => Ok(Self::Hd1280x960),
            _ => Err(ClassicHdError(format!(
                "unknown Classic HD profile id {value}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdViewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl ClassicHdViewport {
    #[must_use]
    pub const fn full(width: u32, height: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassicHdFiltering {
    SourceKernel,
    Nearest,
}

impl ClassicHdFiltering {
    const fn as_str(self) -> &'static str {
        match self {
            Self::SourceKernel => "source-kernel",
            Self::Nearest => "nearest",
        }
    }

    fn parse(value: &str) -> Result<Self, ClassicHdError> {
        match value {
            "source-kernel" => Ok(Self::SourceKernel),
            "nearest" => Ok(Self::Nearest),
            _ => Err(ClassicHdError(format!(
                "unknown Classic HD filtering {value}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassicHdPresentPolicy {
    Pillarbox4x3,
    Corrected4x3,
}

impl ClassicHdPresentPolicy {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Pillarbox4x3 => "pillarbox-4x3",
            Self::Corrected4x3 => "corrected-4x3",
        }
    }

    fn parse(value: &str) -> Result<Self, ClassicHdError> {
        match value {
            "pillarbox-4x3" => Ok(Self::Pillarbox4x3),
            "corrected-4x3" => Ok(Self::Corrected4x3),
            _ => Err(ClassicHdError(format!(
                "unknown Classic HD present policy {value}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassicHdAssetTier {
    OriginalIndexed,
    OptionalHighResolution,
}

impl ClassicHdAssetTier {
    const fn as_str(self) -> &'static str {
        match self {
            Self::OriginalIndexed => "original-indexed",
            Self::OptionalHighResolution => "optional-high-resolution",
        }
    }

    fn parse(value: &str) -> Result<Self, ClassicHdError> {
        match value {
            "original-indexed" => Ok(Self::OriginalIndexed),
            "optional-high-resolution" => Ok(Self::OptionalHighResolution),
            _ => Err(ClassicHdError(format!(
                "unknown Classic HD asset tier {value}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassicHdRenderProfile {
    pub id: ClassicHdProfileId,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub viewport: ClassicHdViewport,
    pub square_pixels: bool,
    pub display_aspect_num: u32,
    pub display_aspect_den: u32,
    pub projection_scale_q16: u32,
    pub ui_scale: u16,
    pub view_distance_scale_q16: u32,
    pub filtering: ClassicHdFiltering,
    pub present_policy: ClassicHdPresentPolicy,
    pub asset_tier: ClassicHdAssetTier,
    pub memory_bytes: u64,
    pub profile_revision: u64,
}

impl ClassicHdRenderProfile {
    /// Construct one of the bounded profiles in the Classic HD matrix.
    pub fn try_new(
        id: ClassicHdProfileId,
        width: u32,
        height: u32,
        stride: u32,
        viewport: ClassicHdViewport,
        filtering: ClassicHdFiltering,
        present_policy: ClassicHdPresentPolicy,
        asset_tier: ClassicHdAssetTier,
    ) -> Result<Self, ClassicHdError> {
        let (expected_width, expected_height) = id.dimensions();
        if width != expected_width || height != expected_height {
            return Err(ClassicHdError(format!(
                "profile {} requires {}x{}, got {}x{}",
                id.as_str(),
                expected_width,
                expected_height,
                width,
                height
            )));
        }
        if width == 0 || height == 0 || stride == 0 {
            return Err(ClassicHdError(
                "Classic HD dimensions and stride must be non-zero".to_owned(),
            ));
        }
        if width > MAX_CLASSIC_HD_DIMENSION
            || height > MAX_CLASSIC_HD_DIMENSION
            || stride > MAX_CLASSIC_HD_DIMENSION
        {
            return Err(ClassicHdError(
                "Classic HD dimensions exceed the safety bound".to_owned(),
            ));
        }
        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(|| ClassicHdError("Classic HD pixel multiplication overflow".to_owned()))?;
        if pixels > MAX_CLASSIC_HD_PIXELS {
            return Err(ClassicHdError(
                "Classic HD pixel count exceeds the safety bound".to_owned(),
            ));
        }
        let viewport_right = viewport
            .x
            .checked_add(viewport.width)
            .ok_or_else(|| ClassicHdError("Classic HD viewport x overflow".to_owned()))?;
        let viewport_bottom = viewport
            .y
            .checked_add(viewport.height)
            .ok_or_else(|| ClassicHdError("Classic HD viewport y overflow".to_owned()))?;
        if viewport.width == 0
            || viewport.height == 0
            || viewport.width != width
            || viewport.height != height
            || viewport_right > stride
            || viewport_bottom > height
        {
            return Err(ClassicHdError(
                "Classic HD viewport is inconsistent with stride or dimensions".to_owned(),
            ));
        }
        if id != ClassicHdProfileId::Compatibility320x200 && width * 3 != height * 4 {
            return Err(ClassicHdError(
                "Classic HD production profiles must use a 4:3 internal viewport".to_owned(),
            ));
        }
        let ui_scale = u16::try_from(width / 320)
            .map_err(|_| ClassicHdError("Classic HD UI scale does not fit u16".to_owned()))?;
        if ui_scale == 0 || width % 320 != 0 {
            return Err(ClassicHdError(
                "Classic HD width must be an integer 320-pixel scale".to_owned(),
            ));
        }
        let projection_scale_q16 = u32::try_from(
            u64::from(width)
                .checked_mul(u64::from(Q16_ONE))
                .ok_or_else(|| ClassicHdError("Classic HD projection scale overflow".to_owned()))?
                / 320,
        )
        .map_err(|_| ClassicHdError("Classic HD projection scale does not fit u32".to_owned()))?;
        let memory_bytes = checked_memory_estimate(stride, height)?;
        if memory_bytes > MAX_CLASSIC_HD_MEMORY_BYTES {
            return Err(ClassicHdError(
                "Classic HD memory estimate exceeds the safety bound".to_owned(),
            ));
        }

        let mut profile = Self {
            id,
            width,
            height,
            stride,
            viewport,
            square_pixels: id != ClassicHdProfileId::Compatibility320x200,
            display_aspect_num: 4,
            display_aspect_den: 3,
            projection_scale_q16,
            ui_scale,
            view_distance_scale_q16: projection_scale_q16,
            filtering,
            present_policy,
            asset_tier,
            memory_bytes,
            profile_revision: 0,
        };
        profile.profile_revision = profile.compute_revision();
        Ok(profile)
    }

    #[must_use]
    pub fn compatibility_320x200() -> Self {
        Self::try_new(
            ClassicHdProfileId::Compatibility320x200,
            320,
            200,
            320,
            ClassicHdViewport::full(320, 200),
            ClassicHdFiltering::SourceKernel,
            ClassicHdPresentPolicy::Corrected4x3,
            ClassicHdAssetTier::OriginalIndexed,
        )
        .expect("the fixed Classic HD compatibility profile is valid")
    }

    #[must_use]
    pub fn hd_640x480() -> Self {
        Self::try_new(
            ClassicHdProfileId::Hd640x480,
            640,
            480,
            640,
            ClassicHdViewport::full(640, 480),
            ClassicHdFiltering::SourceKernel,
            ClassicHdPresentPolicy::Pillarbox4x3,
            ClassicHdAssetTier::OriginalIndexed,
        )
        .expect("the mandatory Classic HD 640x480 profile is valid")
    }

    #[must_use]
    pub fn hd_960x720() -> Self {
        Self::try_new(
            ClassicHdProfileId::Hd960x720,
            960,
            720,
            960,
            ClassicHdViewport::full(960, 720),
            ClassicHdFiltering::SourceKernel,
            ClassicHdPresentPolicy::Pillarbox4x3,
            ClassicHdAssetTier::OptionalHighResolution,
        )
        .expect("the candidate Classic HD 960x720 profile is valid")
    }

    #[must_use]
    pub fn hd_1280x960() -> Self {
        Self::try_new(
            ClassicHdProfileId::Hd1280x960,
            1280,
            960,
            1280,
            ClassicHdViewport::full(1280, 960),
            ClassicHdFiltering::SourceKernel,
            ClassicHdPresentPolicy::Pillarbox4x3,
            ClassicHdAssetTier::OptionalHighResolution,
        )
        .expect("the candidate Classic HD 1280x960 profile is valid")
    }

    #[must_use]
    pub const fn extent(&self) -> usize {
        (self.stride as usize) * (self.height as usize)
    }

    #[must_use]
    pub const fn is_compatibility_profile(&self) -> bool {
        matches!(self.id, ClassicHdProfileId::Compatibility320x200)
    }

    fn compute_revision(&self) -> u64 {
        let mut bytes = Vec::with_capacity(96);
        bytes.extend_from_slice(self.id.as_str().as_bytes());
        for value in [
            self.width,
            self.height,
            self.stride,
            self.viewport.x,
            self.viewport.y,
            self.viewport.width,
            self.viewport.height,
            if self.square_pixels { 1 } else { 0 },
            self.display_aspect_num,
            self.display_aspect_den,
            self.projection_scale_q16,
            u32::from(self.ui_scale),
            self.view_distance_scale_q16,
            self.memory_bytes as u32,
            (self.memory_bytes >> 32) as u32,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.push(self.filtering.as_str().as_bytes()[0]);
        bytes.push(self.present_policy.as_str().as_bytes()[0]);
        bytes.push(self.asset_tier.as_str().as_bytes()[0]);
        byte_hash(&bytes)
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        json!({
            "profile_id": self.id.as_str(),
            "width": self.width,
            "height": self.height,
            "stride": self.stride,
            "viewport": {"x": self.viewport.x, "y": self.viewport.y, "width": self.viewport.width, "height": self.viewport.height},
            "square_pixels": self.square_pixels,
            "display_aspect": {"num": self.display_aspect_num, "den": self.display_aspect_den},
            "projection_scale_q16": self.projection_scale_q16,
            "ui_scale": self.ui_scale,
            "view_distance_scale_q16": self.view_distance_scale_q16,
            "filtering": self.filtering.as_str(),
            "present_policy": self.present_policy.as_str(),
            "asset_tier": self.asset_tier.as_str(),
            "memory_bytes": self.memory_bytes,
            "profile_revision": self.profile_revision,
        })
        .to_string()
    }

    pub fn from_json(input: &str) -> Result<Self, ClassicHdError> {
        let value: Value = serde_json::from_str(input)
            .map_err(|error| ClassicHdError(format!("invalid Classic HD profile JSON: {error}")))?;
        let object = value.as_object().ok_or_else(|| {
            ClassicHdError("Classic HD profile JSON must be an object".to_owned())
        })?;
        let text = |key: &str| {
            object.get(key).and_then(Value::as_str).ok_or_else(|| {
                ClassicHdError(format!("Classic HD profile JSON lacks string {key}"))
            })
        };
        let integer = |key: &str| {
            object.get(key).and_then(Value::as_u64).ok_or_else(|| {
                ClassicHdError(format!("Classic HD profile JSON lacks integer {key}"))
            })
        };
        let id = ClassicHdProfileId::parse(text("profile_id")?)?;
        let width = u32::try_from(integer("width")?)
            .map_err(|_| ClassicHdError("width does not fit u32".to_owned()))?;
        let height = u32::try_from(integer("height")?)
            .map_err(|_| ClassicHdError("height does not fit u32".to_owned()))?;
        let stride = u32::try_from(integer("stride")?)
            .map_err(|_| ClassicHdError("stride does not fit u32".to_owned()))?;
        let viewport_object = object
            .get("viewport")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                ClassicHdError("Classic HD profile JSON lacks viewport object".to_owned())
            })?;
        let viewport_integer = |key: &str| {
            viewport_object
                .get(key)
                .and_then(Value::as_u64)
                .ok_or_else(|| ClassicHdError(format!("Classic HD viewport lacks integer {key}")))
        };
        let viewport = ClassicHdViewport {
            x: u32::try_from(viewport_integer("x")?)
                .map_err(|_| ClassicHdError("viewport x does not fit u32".to_owned()))?,
            y: u32::try_from(viewport_integer("y")?)
                .map_err(|_| ClassicHdError("viewport y does not fit u32".to_owned()))?,
            width: u32::try_from(viewport_integer("width")?)
                .map_err(|_| ClassicHdError("viewport width does not fit u32".to_owned()))?,
            height: u32::try_from(viewport_integer("height")?)
                .map_err(|_| ClassicHdError("viewport height does not fit u32".to_owned()))?,
        };
        let profile = Self::try_new(
            id,
            width,
            height,
            stride,
            viewport,
            ClassicHdFiltering::parse(text("filtering")?)?,
            ClassicHdPresentPolicy::parse(text("present_policy")?)?,
            ClassicHdAssetTier::parse(text("asset_tier")?)?,
        )?;
        let supplied_revision = integer("profile_revision")?;
        if supplied_revision != profile.profile_revision {
            return Err(ClassicHdError(
                "Classic HD profile revision does not match its fields".to_owned(),
            ));
        }
        Ok(profile)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassicHdError(pub String);

impl fmt::Display for ClassicHdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ClassicHdError {}

pub fn checked_memory_estimate(stride: u32, height: u32) -> Result<u64, ClassicHdError> {
    u64::from(stride)
        .checked_mul(u64::from(height))
        .and_then(|value| value.checked_mul(STORAGE_PLANES))
        .and_then(|value| value.checked_add(4096))
        .ok_or_else(|| ClassicHdError("Classic HD memory estimate overflow".to_owned()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdClipResult {
    pub attempted_pixels: u64,
    pub written_pixels: u64,
    pub was_clipped: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicHdFrameIdentity {
    pub generation: u64,
    pub tick: u64,
    pub profile_revision: u64,
    pub input_hash: u64,
    pub index_hash: u64,
    pub provenance_hash: u64,
    pub history_hash: u64,
}

pub struct ClassicHdFrameWriter<'a> {
    profile: ClassicHdRenderProfile,
    indices: &'a mut [u8],
    provenance: &'a mut [u8],
    generation: u64,
    tick: u64,
    profile_revision: u64,
    input_hash: u64,
    history_hash: u64,
}

impl ClassicHdFrameWriter<'_> {
    #[must_use]
    pub const fn profile(&self) -> &ClassicHdRenderProfile {
        &self.profile
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.profile.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.profile.height
    }

    #[must_use]
    pub const fn stride(&self) -> u32 {
        self.profile.stride
    }

    pub fn row_mut(&mut self, y: u32) -> Option<&mut [u8]> {
        row_mut(self.indices, self.profile.stride, self.profile.height, y)
    }

    pub fn provenance_row_mut(&mut self, y: u32) -> Option<&mut [u8]> {
        row_mut(self.provenance, self.profile.stride, self.profile.height, y)
    }

    pub fn indices_mut(&mut self) -> &mut [u8] {
        self.indices
    }

    #[must_use]
    pub fn indices(&self) -> &[u8] {
        self.indices
    }

    pub fn provenance_mut(&mut self) -> &mut [u8] {
        self.provenance
    }

    /// Borrow the indexed and provenance planes together for a streaming
    /// raster pass. Callers must preserve the same painter order as the
    /// source compositor; the pair is exposed only to avoid per-pixel bounds
    /// checks and temporary sample vectors in the Classic HD adapters.
    pub fn planes_mut(&mut self) -> (&mut [u8], &mut [u8]) {
        (&mut *self.indices, &mut *self.provenance)
    }

    #[must_use]
    pub fn provenance(&self) -> &[u8] {
        self.provenance
    }

    pub fn set_provenance(&mut self, x: i32, y: i32, provenance: u8) -> bool {
        let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
            return false;
        };
        if x >= self.profile.width || y >= self.profile.height {
            return false;
        }
        let offset = (y as usize) * (self.profile.stride as usize) + (x as usize);
        self.provenance[offset] = provenance;
        true
    }

    pub fn set_pixel(&mut self, x: i32, y: i32, index: u8, provenance: u8) -> bool {
        let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
            return false;
        };
        if x >= self.profile.width || y >= self.profile.height {
            return false;
        }
        let offset = (y as usize) * (self.profile.stride as usize) + (x as usize);
        self.indices[offset] = index;
        self.provenance[offset] = provenance;
        true
    }

    pub fn clip_rect(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        index: u8,
        provenance: u8,
    ) -> ClassicHdClipResult {
        if width <= 0 || height <= 0 {
            return ClassicHdClipResult {
                attempted_pixels: 0,
                written_pixels: 0,
                was_clipped: true,
            };
        }
        let attempted_pixels = (width as u64).saturating_mul(height as u64);
        let x0 = i64::from(x).max(0);
        let y0 = i64::from(y).max(0);
        let x1 = (i64::from(x) + i64::from(width)).min(i64::from(self.profile.width));
        let y1 = (i64::from(y) + i64::from(height)).min(i64::from(self.profile.height));
        if x0 >= x1 || y0 >= y1 {
            return ClassicHdClipResult {
                attempted_pixels,
                written_pixels: 0,
                was_clipped: true,
            };
        }
        let x0 = x0 as usize;
        let x1 = x1 as usize;
        let y0 = y0 as usize;
        let y1 = y1 as usize;
        for row in y0..y1 {
            let offset = row * self.profile.stride as usize;
            self.indices[offset + x0..offset + x1].fill(index);
            self.provenance[offset + x0..offset + x1].fill(provenance);
        }
        let written_pixels = ((x1 - x0) as u64) * ((y1 - y0) as u64);
        ClassicHdClipResult {
            attempted_pixels,
            written_pixels,
            was_clipped: written_pixels != attempted_pixels,
        }
    }

    #[must_use]
    pub fn finish(self) -> ClassicHdFrameIdentity {
        ClassicHdFrameIdentity {
            generation: self.generation,
            tick: self.tick,
            profile_revision: self.profile_revision,
            input_hash: self.input_hash,
            index_hash: byte_hash(self.indices),
            provenance_hash: byte_hash(self.provenance),
            history_hash: self.history_hash,
        }
    }
}

impl<'a> ClassicHdFrameWriter<'a> {
    /// Borrow caller-owned profile-sized planes for one Classic HD world pass.
    ///
    /// `ClassicHdRenderGraph` owns the transactional storage and history, while
    /// the terrain/sprite raster adapters operate through this writer. Keeping
    /// the constructor here prevents those adapters from reaching into graph
    /// internals or falling back to the fixed 320x200 frame.
    pub fn from_planes(
        profile: &ClassicHdRenderProfile,
        indices: &'a mut [u8],
        provenance: &'a mut [u8],
        generation: u64,
        tick: u64,
        input_hash: u64,
        history_hash: u64,
    ) -> Result<Self, ClassicHdError> {
        let extent = profile.extent();
        if indices.len() != extent || provenance.len() != extent {
            return Err(ClassicHdError(format!(
                "Classic HD external planes require {extent} bytes, got {} and {}",
                indices.len(),
                provenance.len()
            )));
        }
        Ok(Self {
            profile: profile.clone(),
            indices,
            provenance,
            generation,
            tick,
            profile_revision: profile.profile_revision,
            input_hash,
            history_hash,
        })
    }
}

#[derive(Debug)]
pub struct ClassicHdFrameBuffers {
    profile: ClassicHdRenderProfile,
    generation: u64,
    current_tick: Option<u64>,
    input_hash: u64,
    indices: Vec<u8>,
    provenance: Vec<u8>,
    history: Vec<u8>,
}

impl ClassicHdFrameBuffers {
    pub fn new(profile: ClassicHdRenderProfile, generation: u64) -> Result<Self, ClassicHdError> {
        let extent = profile.extent();
        if extent == 0 || u64::try_from(extent).unwrap_or(u64::MAX) > MAX_CLASSIC_HD_PIXELS {
            return Err(ClassicHdError(
                "Classic HD buffer extent is invalid".to_owned(),
            ));
        }
        Ok(Self {
            profile,
            generation,
            current_tick: None,
            input_hash: 0,
            indices: vec![0; extent],
            provenance: vec![0; extent],
            history: vec![0; extent],
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
    pub const fn input_hash(&self) -> u64 {
        self.input_hash
    }

    #[must_use]
    pub const fn allocated_bytes(&self) -> u64 {
        (self.indices.len() as u64) * 3
    }

    pub fn reset(&mut self, generation: u64) {
        self.generation = generation;
        self.current_tick = None;
        self.input_hash = 0;
        self.indices.fill(0);
        self.provenance.fill(0);
        self.history.fill(0);
    }

    pub fn begin_frame(
        &mut self,
        generation: u64,
        tick: u64,
        profile_revision: u64,
        input_hash: u64,
    ) -> Result<ClassicHdFrameWriter<'_>, ClassicHdError> {
        if generation != self.generation {
            return Err(ClassicHdError(format!(
                "Classic HD generation {generation} does not match {}",
                self.generation
            )));
        }
        if profile_revision != self.profile.profile_revision {
            return Err(ClassicHdError(
                "Classic HD profile revision mismatch".to_owned(),
            ));
        }
        let expected_tick = self.current_tick.map_or(Ok(0), |current| {
            current
                .checked_add(1)
                .ok_or_else(|| ClassicHdError("Classic HD tick overflow".to_owned()))
        })?;
        if tick != expected_tick {
            return Err(ClassicHdError(format!(
                "Classic HD tick {tick} is out of order; expected {expected_tick}"
            )));
        }

        self.history.copy_from_slice(&self.indices);
        let history_hash = byte_hash(&self.history);
        self.indices.fill(0);
        self.provenance.fill(0);
        self.current_tick = Some(tick);
        self.input_hash = input_hash;
        Ok(ClassicHdFrameWriter {
            profile: self.profile.clone(),
            indices: &mut self.indices,
            provenance: &mut self.provenance,
            generation,
            tick,
            profile_revision,
            input_hash,
            history_hash,
        })
    }

    pub fn current_row(&self, y: u32) -> Option<&[u8]> {
        row(
            self.indices.as_slice(),
            self.profile.stride,
            self.profile.height,
            y,
        )
    }

    pub fn provenance_row(&self, y: u32) -> Option<&[u8]> {
        row(
            self.provenance.as_slice(),
            self.profile.stride,
            self.profile.height,
            y,
        )
    }

    pub fn history_row(&self, y: u32) -> Option<&[u8]> {
        row(
            self.history.as_slice(),
            self.profile.stride,
            self.profile.height,
            y,
        )
    }

    #[must_use]
    pub fn current_index_hash(&self) -> u64 {
        byte_hash(&self.indices)
    }

    #[must_use]
    pub fn current_provenance_hash(&self) -> u64 {
        byte_hash(&self.provenance)
    }

    #[must_use]
    pub fn history_hash(&self) -> u64 {
        byte_hash(&self.history)
    }

    #[must_use]
    pub fn storage_pointers(&self) -> (usize, usize, usize) {
        (
            self.indices.as_ptr() as usize,
            self.provenance.as_ptr() as usize,
            self.history.as_ptr() as usize,
        )
    }
}

fn row<'a>(plane: &'a [u8], stride: u32, height: u32, y: u32) -> Option<&'a [u8]> {
    if y >= height {
        return None;
    }
    let stride = stride as usize;
    let offset = (y as usize) * stride;
    Some(&plane[offset..offset + stride])
}

fn row_mut<'a>(plane: &'a mut [u8], stride: u32, height: u32, y: u32) -> Option<&'a mut [u8]> {
    if y >= height {
        return None;
    }
    let stride = stride as usize;
    let offset = (y as usize) * stride;
    Some(&mut plane[offset..offset + stride])
}

#[must_use]
pub fn byte_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mandatory_and_candidate_profiles_are_checked_and_serializable() {
        for profile in [
            ClassicHdRenderProfile::hd_640x480(),
            ClassicHdRenderProfile::hd_960x720(),
            ClassicHdRenderProfile::hd_1280x960(),
        ] {
            assert!(profile.square_pixels);
            assert_eq!(
                profile.display_aspect_num * profile.height,
                profile.display_aspect_den * profile.width / 1
            );
            let decoded =
                ClassicHdRenderProfile::from_json(&profile.to_json()).expect("profile round-trips");
            assert_eq!(decoded, profile);
        }
        let compatibility = ClassicHdRenderProfile::compatibility_320x200();
        assert!(!compatibility.square_pixels);
        assert_eq!(compatibility.ui_scale, 1);
    }

    #[test]
    fn invalid_dimensions_stride_viewport_and_memory_fail_closed() {
        let valid = ClassicHdRenderProfile::hd_640x480();
        assert!(
            ClassicHdRenderProfile::try_new(
                ClassicHdProfileId::Hd640x480,
                641,
                480,
                641,
                ClassicHdViewport::full(641, 480),
                valid.filtering,
                valid.present_policy,
                valid.asset_tier,
            )
            .is_err()
        );
        assert!(
            ClassicHdRenderProfile::try_new(
                ClassicHdProfileId::Hd640x480,
                640,
                480,
                639,
                ClassicHdViewport::full(640, 480),
                valid.filtering,
                valid.present_policy,
                valid.asset_tier,
            )
            .is_err()
        );
        assert!(
            ClassicHdRenderProfile::try_new(
                ClassicHdProfileId::Hd640x480,
                640,
                480,
                640,
                ClassicHdViewport {
                    x: 1,
                    y: 0,
                    width: 640,
                    height: 480,
                },
                valid.filtering,
                valid.present_policy,
                valid.asset_tier,
            )
            .is_err()
        );
        assert!(checked_memory_estimate(u32::MAX, u32::MAX).is_err());
    }

    #[test]
    fn clipping_covers_every_edge_without_touching_other_rows() {
        let profile = ClassicHdRenderProfile::hd_640x480();
        let mut buffers = ClassicHdFrameBuffers::new(profile.clone(), 7).expect("buffers allocate");
        let mut writer = buffers
            .begin_frame(7, 0, profile.profile_revision, 0x11)
            .expect("frame begins");
        let result = writer.clip_rect(-3, -2, 8, 7, 42, 9);
        assert_eq!(result.attempted_pixels, 56);
        assert_eq!(result.written_pixels, 25);
        assert!(result.was_clipped);
        assert_eq!(writer.row_mut(0).expect("row 0")[0..5], [42; 5]);
        assert_eq!(
            writer.provenance_row_mut(0).expect("provenance row 0")[0..5],
            [9; 5]
        );
        assert!(writer.set_pixel(639, 479, 77, 3));
        assert!(!writer.set_pixel(640, 479, 77, 3));
        assert!(!writer.set_pixel(639, 480, 77, 3));
        let identity = writer.finish();
        assert_eq!(identity.generation, 7);
        assert_eq!(identity.tick, 0);
        assert_ne!(identity.index_hash, byte_hash(&vec![0; profile.extent()]));
    }

    #[test]
    fn stale_generation_tick_profile_and_overflow_are_rejected_before_mutation() {
        let profile = ClassicHdRenderProfile::hd_640x480();
        let mut buffers = ClassicHdFrameBuffers::new(profile.clone(), 3).expect("buffers allocate");
        let pointers_before = buffers.storage_pointers();
        assert!(
            buffers
                .begin_frame(4, 0, profile.profile_revision, 1)
                .is_err()
        );
        assert!(
            buffers
                .begin_frame(3, 1, profile.profile_revision, 1)
                .is_err()
        );
        assert!(
            buffers
                .begin_frame(3, 0, profile.profile_revision + 1, 1)
                .is_err()
        );
        let writer = buffers
            .begin_frame(3, 0, profile.profile_revision, 1)
            .expect("first frame");
        let _ = writer.finish();
        assert!(
            buffers
                .begin_frame(3, 0, profile.profile_revision, 1)
                .is_err()
        );
        assert!(
            buffers
                .begin_frame(3, 2, profile.profile_revision, 1)
                .is_err()
        );
        assert_eq!(buffers.storage_pointers(), pointers_before);
        buffers.reset(9);
        assert_eq!(buffers.current_tick(), None);
        assert_eq!(buffers.generation(), 9);
    }

    #[test]
    fn history_is_revisioned_and_buffers_are_reused_without_reallocation() {
        let profile = ClassicHdRenderProfile::hd_640x480();
        let mut buffers = ClassicHdFrameBuffers::new(profile.clone(), 1).expect("buffers allocate");
        let pointers = buffers.storage_pointers();
        {
            let mut writer = buffers
                .begin_frame(1, 0, profile.profile_revision, 0xabc)
                .expect("frame 0");
            writer.set_pixel(2, 3, 19, 7);
            let first = writer.finish();
            assert_eq!(first.history_hash, byte_hash(&vec![0; profile.extent()]));
        }
        let first_hash = buffers.current_index_hash();
        assert_eq!(
            buffers.history_hash(),
            byte_hash(&vec![0; profile.extent()])
        );
        {
            let mut writer = buffers
                .begin_frame(1, 1, profile.profile_revision, 0xdef)
                .expect("frame 1");
            assert_eq!(writer.set_pixel(2, 3, 21, 8), true);
            let second = writer.finish();
            assert_eq!(second.history_hash, first_hash);
        }
        assert_eq!(buffers.history_row(3).expect("history row")[2], 19);
        assert_eq!(buffers.current_row(3).expect("current row")[2], 21);
        assert_eq!(buffers.storage_pointers(), pointers);
        assert_eq!(buffers.allocated_bytes(), profile.extent() as u64 * 3);
    }

    #[test]
    fn stable_hashes_are_repeatable() {
        let bytes = [0, 1, 2, 3, 255];
        assert_eq!(byte_hash(&bytes), byte_hash(&bytes));
        assert_ne!(byte_hash(&bytes), byte_hash(&[0, 1, 2, 4, 255]));
    }
}
