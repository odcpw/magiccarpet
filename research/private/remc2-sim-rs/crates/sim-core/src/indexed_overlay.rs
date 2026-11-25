//! Typed fixed-profile overlay commands for the owner-cache FINAL MAIN path.
//!
//! This module deliberately does not infer UI state.  The native presentation
//! bridge resolves live Level 1 state into a closed command stream; this module
//! only validates that stream against the CP1 cache and executes its exact
//! indexed primitives.  A missing producer, selector, localization row, or
//! required receipt is an error before the presentation history can commit.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use serde_json::Value;

use crate::{
    indexed_postprocess::OVERLAY_PROVENANCE,
    native_world::QualifiedOwnerCacheReader,
    terrain_frame::{LEVEL1_VIEW_HEIGHT, LEVEL1_VIEW_PIXELS, LEVEL1_VIEW_WIDTH},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedBitmap {
    pub width: u16,
    pub height: u16,
    pub indices: Box<[u8]>,
    pub mask: Box<[u8]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexedGlyph {
    pub offset: u32,
    pub width: u8,
    pub height: u8,
    pub length: u16,
}

trait SourceGlyphView {
    fn dimensions(&self) -> (usize, usize, usize);
}

impl SourceGlyphView for IndexedGlyph {
    fn dimensions(&self) -> (usize, usize, usize) {
        (self.offset as usize, usize::from(self.width), usize::from(self.height))
    }
}

impl SourceGlyphView for crate::game_runtime::preferences::installed_locale::SourceGlyph {
    fn dimensions(&self) -> (usize, usize, usize) {
        (self.offset, usize::from(self.width), usize::from(self.height))
    }
}

/// Source FONT1 byte+1 text traversal, shared by indexed HUD and world text.
/// The sink owns clipping and the destination plane.
fn draw_source_text<G: SourceGlyphView>(
    glyphs: &[G], masks: &[u8], mut x: i32, mut y: i32, text: &[u8],
    coordinate_divisor: i32, height_divisor: usize,
    mut write: impl FnMut(i32, i32) -> bool,
) -> Result<u32, IndexedOverlayError> {
    for byte in text.iter().take_while(|byte| **byte != 0) {
        let index = match *byte {
            b'\r' => continue,
            b'\t' | b' ' => 33,
            b'\n' => 34,
            byte => usize::from(byte) + 1,
        };
        let glyph = glyphs.get(index)
            .ok_or_else(|| IndexedOverlayError(format!("FONT1 glyph {index} missing")))?;
        let (offset, width, height) = glyph.dimensions();
        if offset.checked_add(width.saturating_mul(height)).is_none_or(|end| end > masks.len()) {
            return Err(IndexedOverlayError(format!("FONT1 glyph {index} exceeds mask blob")));
        }
    }
    let origin_x = x;
    let mut writes = 0;
    for byte in text.iter().take_while(|byte| **byte != 0) {
        match *byte {
            b'\r' => continue,
            b'\n' => { x = origin_x; y += glyphs[34].dimensions().2 as i32; continue; }
            b'\t' | b' ' => { x += glyphs[33].dimensions().1 as i32; continue; }
            _ => {}
        }
        let (offset, width, height) = glyphs[usize::from(*byte) + 1].dimensions();
        for row in 0..height / height_divisor {
            for column in 0..width {
                if masks[offset + row * width + column] != 0 {
                    writes += u32::from(write(x / coordinate_divisor + column as i32,
                        y / coordinate_divisor + row as i32));
                }
            }
        }
        x += width as i32;
    }
    Ok(writes)
}

pub(crate) fn draw_selected_source_font(
    font: &crate::game_runtime::preferences::installed_locale::SourceFont,
    x: i32, y: i32, text: &[u8], write: impl FnMut(i32, i32) -> bool,
) -> Result<u32, IndexedOverlayError> {
    // sub_8F935 halves coordinates and the number of source rows, then its
    // colourized RLE writer consumes the original horizontal glyph runs.
    draw_source_text(&font.glyphs, &font.masks, x, y, text, 2, 2, write)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndexedObjective {
    reference_x: u16,
    reference_y: u16,
    width: u16,
    height: u16,
    indices: Box<[u8]>,
    mask: Box<[u8]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SpellTierVector {
    x_for_i: i16,
    y: i16,
    colour: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpellMenuProfile {
    pub family_header_tier_count: u8,
    pub family_flags: u8,
    pub mana_cost: [i32; 3],
    pub max_mana_limit: [i32; 3],
    pub single_player_experience: [i32; 3],
}

#[derive(Clone, Debug)]
pub struct IndexedOverlayAssets {
    bitmaps: BTreeMap<u16, IndexedBitmap>,
    font_masks: Box<[u8]>,
    glyphs: Box<[IndexedGlyph]>,
    localization: Box<[Box<[u8]>]>,
    clrn: Box<[u8]>,
    tablesn: Box<[u8]>,
    minimap_sine: Box<[i32]>,
    minimap_aperture: Box<[u8]>,
    objectives: BTreeMap<u8, IndexedObjective>,
    receipt_census: BTreeMap<String, Box<[IndexedOverlayStage]>>,
    spell_tier_vectors: BTreeMap<IndexedOverlayStage, SpellTierVector>,
    spell_menu_profiles: [SpellMenuProfile; 26],
    source_semantic_vectors: Value,
    locale_binding: Option<crate::game_runtime::preferences::installed_locale::LocaleSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum IndexedOverlayStage {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IndexedOverlayCommand {
    /// A source writer was reached and consumed its complete typed state, but
    /// its selected source branch intentionally wrote no pixels.
    Decision {
        stage: IndexedOverlayStage,
        state_hash: u64,
    },
    Bitmap {
        stage: IndexedOverlayStage,
        selector: u16,
        x: i16,
        y: i16,
    },
    TransparentBitmap {
        stage: IndexedOverlayStage,
        selector: u16,
        x: i16,
        y: i16,
    },
    ColourizedBitmap {
        stage: IndexedOverlayStage,
        selector: u16,
        x: i16,
        y: i16,
        colour: u8,
    },
    LocalizedText {
        stage: IndexedOverlayStage,
        selector: u16,
        x: i16,
        y: i16,
        colour: u8,
    },
    Text {
        stage: IndexedOverlayStage,
        bytes: Box<[u8]>,
        x: i16,
        y: i16,
        colour: u8,
    },
    Rect {
        stage: IndexedOverlayStage,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        colour: u8,
    },
    Line {
        stage: IndexedOverlayStage,
        from_x: i16,
        from_y: i16,
        to_x: i16,
        to_y: i16,
        colour: u8,
    },
    Pixels {
        stage: IndexedOverlayStage,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        indices: Box<[u8]>,
        mask: Box<[u8]>,
    },
    BlendPixels {
        stage: IndexedOverlayStage,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        blend_indices: Box<[u8]>,
        mask: Box<[u8]>,
    },
    BlendPixel {
        stage: IndexedOverlayStage,
        x: i16,
        y: i16,
        blend_index: u8,
    },
    Objective {
        stage: IndexedOverlayStage,
        selector: u8,
    },
    /// Copy a rectangular viewport from the actual source-rendered MAIN that
    /// entered the overlay executor. This preserves the map branch's second
    /// DrawWorld contribution even though MapClear executes first.
    SourceViewport {
        stage: IndexedOverlayStage,
        source_x: u16,
        source_y: u16,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    },
}

impl IndexedOverlayCommand {
    #[must_use]
    pub const fn stage(&self) -> IndexedOverlayStage {
        match self {
            Self::Decision { stage, .. }
            | Self::Bitmap { stage, .. }
            | Self::TransparentBitmap { stage, .. }
            | Self::ColourizedBitmap { stage, .. }
            | Self::LocalizedText { stage, .. }
            | Self::Text { stage, .. }
            | Self::Rect { stage, .. }
            | Self::Line { stage, .. }
            | Self::Pixels { stage, .. }
            | Self::BlendPixels { stage, .. }
            | Self::BlendPixel { stage, .. }
            | Self::Objective { stage, .. }
            | Self::SourceViewport { stage, .. } => *stage,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixedOverlayProfile {
    pub menu_state: u8,
    pub player_alive: bool,
    pub stereo: bool,
    pub alternate_resolution: bool,
    pub dac_output: bool,
    pub hostile_or_remote_player: bool,
}

impl Default for FixedOverlayProfile {
    fn default() -> Self {
        Self {
            menu_state: 0,
            player_alive: true,
            stereo: false,
            alternate_resolution: false,
            dac_output: false,
            hostile_or_remote_player: false,
        }
    }
}

/// Closed, live-state-derived overlay stream for one presentation tick.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct Level1FinalMainState {
    pub profile: FixedOverlayProfile,
    pub commands: Vec<IndexedOverlayCommand>,
    pub required_stages: Vec<IndexedOverlayStage>,
    pub prior_help_sentinel: u8,
    pub next_help_sentinel: u8,
}

impl Level1FinalMainState {
    #[must_use]
    pub fn immutable_hash(&self) -> u64 {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"mc2-level1-final-main-state-v1\0");
        bytes.extend_from_slice(&[
            self.profile.menu_state,
            u8::from(self.profile.player_alive),
            u8::from(self.profile.stereo),
            u8::from(self.profile.alternate_resolution),
            u8::from(self.profile.dac_output),
            u8::from(self.profile.hostile_or_remote_player),
            self.next_help_sentinel,
        ]);
        bytes.extend_from_slice(
            &u32::try_from(self.required_stages.len())
                .expect("stage count fits u32")
                .to_le_bytes(),
        );
        for stage in &self.required_stages {
            bytes.push(*stage as u8);
        }
        bytes.extend_from_slice(
            &u32::try_from(self.commands.len())
                .expect("command count fits u32")
                .to_le_bytes(),
        );
        for command in &self.commands {
            hash_command(command, &mut bytes);
        }
        byte_hash(&bytes)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexedOverlayReceipt {
    pub stage: IndexedOverlayStage,
    pub command_count: u32,
    pub pixel_writes: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedOverlayOutput {
    pub receipts: Vec<IndexedOverlayReceipt>,
    pub pixels_written: u32,
    pub help_sentinel: u8,
    pub command_hash: u64,
    pub index_hash: u64,
    pub provenance_hash: u64,
    pub output_bytes: u32,
}

impl IndexedOverlayOutput {
    #[must_use]
    pub fn matches_plan(
        &self,
        state: &Level1FinalMainState,
        indices: &[u8],
        provenance: &[u8],
    ) -> bool {
        self.command_hash == state.immutable_hash()
            && self.output_bytes == LEVEL1_VIEW_PIXELS as u32
            && self.index_hash == byte_hash(indices)
            && self.provenance_hash == byte_hash(provenance)
            && self.receipts.len() == state.required_stages.len()
            && self
                .receipts
                .iter()
                .zip(&state.required_stages)
                .all(|(receipt, stage)| {
                    receipt.stage == *stage
                        && receipt.command_count > 0
                        && state
                            .commands
                            .iter()
                            .filter(|command| command.stage() == *stage)
                            .count()
                            == usize::try_from(receipt.command_count)
                                .expect("fixed command count fits usize")
                })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedOverlayError(pub String);

impl std::fmt::Display for IndexedOverlayError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for IndexedOverlayError {}

impl IndexedOverlayAssets {
    pub fn load(cache_root: impl AsRef<Path>) -> Result<Self, IndexedOverlayError> {
        let cache_root = cache_root.as_ref();
        let cache = QualifiedOwnerCacheReader::open(cache_root)
            .map_err(|error| IndexedOverlayError(error.to_string()))?;
        let census_path = cache_root.join("frame/source-frame-census.json");
        let census_bytes = cache
            .read("frame/source-frame-census.json")
            .map_err(|error| IndexedOverlayError(error.to_string()))?;
        let census: Value = serde_json::from_slice(&census_bytes).map_err(|error| {
            IndexedOverlayError(format!("parse {}: {error}", census_path.display()))
        })?;
        let fixed_bank = census
            .pointer("/ui_mechanical_census/fixed_source_bitmap_bank/assets")
            .and_then(Value::as_array)
            .ok_or_else(|| IndexedOverlayError("fixed bitmap census missing".to_owned()))?;
        let mut bitmaps = BTreeMap::new();
        for asset in fixed_bank {
            let selector = json_u16(asset, "selector")?;
            let width = json_u16(asset, "stored_width")?;
            let height = json_u16(asset, "stored_height")?;
            let indices = read_qualified(&cache, json_str(asset, "indices_path")?)?;
            let mask = read_qualified(&cache, json_str(asset, "mask_path")?)?;
            let expected = usize::from(width) * usize::from(height);
            if indices.len() != expected || mask.len() != expected {
                return Err(IndexedOverlayError(format!(
                    "selector {selector} extent {width}x{height} has indices={} mask={}",
                    indices.len(),
                    mask.len()
                )));
            }
            bitmaps.insert(
                selector,
                IndexedBitmap {
                    width,
                    height,
                    indices: indices.into_boxed_slice(),
                    mask: mask.into_boxed_slice(),
                },
            );
        }

        let font = census
            .pointer("/ui_mechanical_census/fixed_source_font")
            .ok_or_else(|| IndexedOverlayError("fixed FONT1 census missing".to_owned()))?;
        let font_masks = read_qualified(&cache, json_str(font, "mask_blob_path")?)?;
        let glyphs = font
            .get("glyphs")
            .and_then(Value::as_array)
            .ok_or_else(|| IndexedOverlayError("FONT1 glyph census missing".to_owned()))?
            .iter()
            .map(|glyph| {
                Ok(IndexedGlyph {
                    offset: json_u32(glyph, "offset")?,
                    width: json_u8(glyph, "width")?,
                    height: json_u8(glyph, "height")?,
                    length: json_u16(glyph, "length")?,
                })
            })
            .collect::<Result<Vec<_>, IndexedOverlayError>>()?;
        for (index, glyph) in glyphs.iter().enumerate() {
            if usize::try_from(glyph.offset).unwrap_or(usize::MAX) + usize::from(glyph.length)
                > font_masks.len()
            {
                return Err(IndexedOverlayError(format!(
                    "FONT1 glyph {index} exceeds mask blob"
                )));
            }
        }

        let localization_row = census
            .pointer("/ui_mechanical_census/localization")
            .ok_or_else(|| IndexedOverlayError("localization census missing".to_owned()))?;
        let localization_blob =
            read_qualified(&cache, json_str(localization_row, "records_path")?)?;
        let localization = split_nul_records(&localization_blob);
        let expected_records = usize::from(json_u16(localization_row, "record_count")?);
        if localization.len() != expected_records {
            return Err(IndexedOverlayError(format!(
                "localization has {} records, expected {expected_records}",
                localization.len()
            )));
        }

        let minimap_aperture = read_qualified(&cache, "hud/minimap-aperture-128.mask.u8")?;
        if minimap_aperture.len() != 128 * 128 {
            return Err(IndexedOverlayError(format!(
                "minimap aperture has {} bytes, expected 16384",
                minimap_aperture.len()
            )));
        }
        let hud_path = cache_root.join("hud/level1.json");
        let hud_bytes = read_qualified(&cache, "hud/level1.json")?;
        let hud: Value = serde_json::from_slice(&hud_bytes).map_err(|error| {
            IndexedOverlayError(format!("parse {}: {error}", hud_path.display()))
        })?;
        let objective_rows = hud
            .pointer("/objective/rows")
            .and_then(Value::as_array)
            .ok_or_else(|| IndexedOverlayError("objective row census missing".to_owned()))?;
        let mut objectives = BTreeMap::new();
        for row in objective_rows {
            let selector = json_u8(row, "selector")?;
            let reference_x = json_u16(row, "reference_x")?;
            let reference_y = json_u16(row, "reference_y")?;
            let width = json_u16(row, "render_width")?;
            let height = json_u16(row, "render_height")?;
            if reference_x % 2 != 0 || reference_y % 2 != 0 || width % 2 != 0 || height % 2 != 0 {
                return Err(IndexedOverlayError(format!(
                    "objective selector {selector} cannot use the fixed VGA1 half-size branch"
                )));
            }
            let bytes = read_qualified(&cache, json_str(row, "raster_path")?)?;
            let pixels = usize::from(width) * usize::from(height);
            if bytes.len() != pixels * 2 {
                return Err(IndexedOverlayError(format!(
                    "objective selector {selector} extent {width}x{height} has {} indexed-mask bytes",
                    bytes.len()
                )));
            }
            objectives.insert(
                selector,
                IndexedObjective {
                    reference_x,
                    reference_y,
                    width,
                    height,
                    indices: bytes[..pixels].to_vec().into_boxed_slice(),
                    mask: bytes[pixels..].to_vec().into_boxed_slice(),
                },
            );
        }
        if objectives.keys().copied().collect::<Vec<_>>() != (48..=53).collect::<Vec<_>>() {
            return Err(IndexedOverlayError(format!(
                "objective selectors {:?} do not match Level 1 rows 48..53",
                objectives.keys().collect::<Vec<_>>()
            )));
        }
        let receipt_rows = census
            .pointer("/overlay_receipt_ledger/branch_sequences")
            .and_then(Value::as_array)
            .ok_or_else(|| IndexedOverlayError("overlay receipt ledger missing".to_owned()))?;
        let mut receipt_census = BTreeMap::new();
        for row in receipt_rows {
            let key = json_str(row, "key")?.to_owned();
            let stages = row
                .get("stages")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    IndexedOverlayError(format!("overlay receipt branch {key} has no stages"))
                })?
                .iter()
                .map(|stage| {
                    stage
                        .as_str()
                        .ok_or_else(|| {
                            IndexedOverlayError(format!(
                                "overlay receipt branch {key} has a non-string stage"
                            ))
                        })
                        .and_then(overlay_stage_from_name)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if receipt_census
                .insert(key.clone(), stages.into_boxed_slice())
                .is_some()
            {
                return Err(IndexedOverlayError(format!(
                    "duplicate overlay receipt branch {key}"
                )));
            }
        }
        let tier_rows = census
            .pointer("/overlay_receipt_ledger/vectors/spell_tier_reset")
            .and_then(Value::as_array)
            .ok_or_else(|| IndexedOverlayError("spell-tier vector ledger missing".to_owned()))?;
        let mut spell_tier_vectors = BTreeMap::new();
        for row in tier_rows {
            let stage = match json_str(row, "hand")? {
                "left" => IndexedOverlayStage::LeftSpell,
                "right" => IndexedOverlayStage::RightSpell,
                hand => {
                    return Err(IndexedOverlayError(format!(
                        "unknown spell-tier vector hand {hand}"
                    )));
                }
            };
            let vector = SpellTierVector {
                x_for_i: i16::try_from(json_u16(row, "fixed_x")?)
                    .map_err(|_| IndexedOverlayError("spell-tier x exceeds i16".to_owned()))?,
                y: i16::try_from(json_u16(row, "fixed_y")?)
                    .map_err(|_| IndexedOverlayError("spell-tier y exceeds i16".to_owned()))?,
                colour: json_u8(row, "colour")?,
            };
            spell_tier_vectors.insert(stage, vector);
        }
        let profile_rows = census
            .pointer("/overlay_receipt_ledger/vectors/spell_menu/profiles")
            .and_then(Value::as_array)
            .ok_or_else(|| IndexedOverlayError("spell-menu profile ledger missing".to_owned()))?;
        if profile_rows.len() != 26 {
            return Err(IndexedOverlayError(format!(
                "spell-menu profile ledger has {} families, expected 26",
                profile_rows.len()
            )));
        }
        let mut profiles = Vec::with_capacity(26);
        for (family, row) in profile_rows.iter().enumerate() {
            if usize::from(json_u8(row, "family")?) != family {
                return Err(IndexedOverlayError(format!(
                    "spell-menu profile family {} is out of order",
                    json_u8(row, "family")?
                )));
            }
            profiles.push(SpellMenuProfile {
                family_header_tier_count: json_u8(row, "family_header_tier_count")?,
                family_flags: json_u8(row, "family_flags")?,
                mana_cost: json_i32_triplet(row, "mana_cost")?,
                max_mana_limit: json_i32_triplet(row, "max_mana_limit")?,
                single_player_experience: json_i32_triplet(row, "single_player_experience")?,
            });
        }
        let spell_menu_profiles: [SpellMenuProfile; 26] = profiles
            .try_into()
            .expect("validated spell-menu family count");
        let source_semantic_vectors = census
            .pointer("/overlay_receipt_ledger/vectors/correction_2_source_vectors")
            .cloned()
            .ok_or_else(|| {
                IndexedOverlayError("correction-2 source-semantic vector ledger missing".to_owned())
            })?;
        if spell_tier_vectors.len() != 2 {
            return Err(IndexedOverlayError(
                "spell-tier vector ledger does not cover both hands".to_owned(),
            ));
        }
        let clrn = read_qualified(&cache, "palette/clrn.indices.u8")?;
        if clrn.len() != 4_096 {
            return Err(IndexedOverlayError(format!(
                "CLRN palette has {} entries, expected 4096",
                clrn.len()
            )));
        }
        validate_source_semantic_vectors(&source_semantic_vectors, &clrn, &localization)?;
        Ok(Self {
            bitmaps,
            font_masks: font_masks.into_boxed_slice(),
            glyphs: glyphs.into_boxed_slice(),
            localization: localization.into_boxed_slice(),
            clrn: clrn.into_boxed_slice(),
            tablesn: read_qualified(&cache, "palette/tablesn.u8")?.into_boxed_slice(),
            minimap_sine: read_i32le(&read_qualified(&cache, "hud/remc2-sine-2560.i32le")?)?
                .into_boxed_slice(),
            minimap_aperture: minimap_aperture.into_boxed_slice(),
            objectives,
            receipt_census,
            spell_tier_vectors,
            spell_menu_profiles,
            source_semantic_vectors,
            locale_binding: None,
        })
    }

    /// Called on an instance-owned candidate, after the canonical default cache
    /// has passed its independent source vectors. All selected products replace
    /// the prior text/font/objective set together; no default validation changes.
    pub fn bind_locale(
        &mut self,
        locale: &crate::game_runtime::preferences::installed_locale::InstalledLocaleBinding,
    ) -> Result<(), IndexedOverlayError> {
        let font = locale
            .fonts
            .get("font1")
            .ok_or_else(|| IndexedOverlayError("selected FONT1 is missing".into()))?;
        let glyphs = font
            .glyphs
            .iter()
            .map(|glyph| {
                Ok(IndexedGlyph {
                    offset: u32::try_from(glyph.offset).map_err(|_| {
                        IndexedOverlayError("selected glyph offset exceeds u32".into())
                    })?,
                    width: glyph.width,
                    height: glyph.height,
                    length: u16::try_from(glyph.length).map_err(|_| {
                        IndexedOverlayError("selected glyph mask exceeds u16".into())
                    })?,
                })
            })
            .collect::<Result<Vec<_>, IndexedOverlayError>>()?;
        let mut objectives = BTreeMap::new();
        for row in &locale.objectives {
            let pixels = usize::from(row.width) * usize::from(row.height);
            if row.indices_then_mask.len() != pixels * 2 {
                return Err(IndexedOverlayError(
                    "selected objective resource extent mismatch".into(),
                ));
            }
            objectives.insert(
                row.selector,
                IndexedObjective {
                    reference_x: row.reference_x,
                    reference_y: row.reference_y,
                    width: row.width,
                    height: row.height,
                    indices: row.indices_then_mask[..pixels].into(),
                    mask: row.indices_then_mask[pixels..].into(),
                },
            );
        }
        self.font_masks = font.masks.clone();
        self.glyphs = glyphs.into_boxed_slice();
        self.localization = locale.records.clone();
        self.objectives = objectives;
        self.locale_binding = Some(locale.identity.clone());
        Ok(())
    }

    #[must_use]
    pub fn locale_binding(
        &self,
    ) -> Option<&crate::game_runtime::preferences::installed_locale::LocaleSnapshot> {
        self.locale_binding.as_ref()
    }

    #[must_use]
    pub fn bitmap_count(&self) -> usize {
        self.bitmaps.len()
    }

    #[must_use]
    pub fn bitmap_extent(&self, selector: u16) -> Option<(u16, u16)> {
        self.bitmaps
            .get(&selector)
            .map(|bitmap| (bitmap.width, bitmap.height))
    }

    #[must_use]
    pub fn minimap_aperture(&self) -> &[u8] {
        &self.minimap_aperture
    }

    #[must_use]
    pub fn localization(&self, selector: u16) -> Option<&[u8]> {
        self.localization
            .get(usize::from(selector))
            .map(Box::as_ref)
    }

    #[must_use]
    pub fn minimap_sine(&self) -> &[i32] {
        &self.minimap_sine
    }

    pub fn terrain_shade_index(
        &self,
        terrain_type: u8,
        shade: u8,
    ) -> Result<u8, IndexedOverlayError> {
        let base = *self
            .tablesn
            .get(0x1_4000 + usize::from(terrain_type))
            .ok_or_else(|| IndexedOverlayError("TABLESN terrain base row missing".to_owned()))?;
        self.tablesn
            .get((usize::from(shade) << 8) + usize::from(base))
            .copied()
            .ok_or_else(|| IndexedOverlayError("TABLESN terrain shade row missing".to_owned()))
    }

    #[must_use]
    pub fn palette_index(&self, source_index: usize) -> Option<u8> {
        self.clrn.get(source_index).copied()
    }

    #[must_use]
    pub fn spell_menu_profile(&self, family: usize) -> Option<SpellMenuProfile> {
        self.spell_menu_profiles.get(family).copied()
    }

    #[must_use]
    pub fn source_semantic_vectors(&self) -> &Value {
        &self.source_semantic_vectors
    }

    pub fn apply(
        &self,
        state: &Level1FinalMainState,
        indices: &mut [u8],
        provenance: &mut [u8],
        map_world: Option<&[u8]>,
    ) -> Result<IndexedOverlayOutput, IndexedOverlayError> {
        validate_plane("FINAL MAIN", indices)?;
        validate_plane("FINAL provenance", provenance)?;
        validate_profile(state.profile)?;
        let branch_key = overlay_branch_key(state.profile);
        let expected_stages = self.receipt_census.get(&branch_key).ok_or_else(|| {
            IndexedOverlayError(format!(
                "overlay receipt ledger has no source branch {branch_key}"
            ))
        })?;
        if expected_stages.as_ref() != state.required_stages {
            return Err(IndexedOverlayError(format!(
                "overlay required stages {:?} do not match source ledger {branch_key} {:?}",
                state.required_stages, expected_stages
            )));
        }
        let required = state
            .required_stages
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if required.len() != state.required_stages.len() {
            return Err(IndexedOverlayError(
                "overlay required-stage list contains duplicates".to_owned(),
            ));
        }
        for command in &state.commands {
            if !required.contains(&command.stage()) {
                return Err(IndexedOverlayError(format!(
                    "overlay command stage {:?} has no required receipt",
                    command.stage()
                )));
            }
            self.validate_command(command)?;
        }
        self.validate_source_invariants(state)?;
        let commanded = state
            .commands
            .iter()
            .map(IndexedOverlayCommand::stage)
            .collect::<BTreeSet<_>>();
        if commanded != required {
            return Err(IndexedOverlayError(format!(
                "overlay command stages {commanded:?} do not close required set {required:?}"
            )));
        }

        let before = provenance
            .iter()
            .filter(|value| (**value & OVERLAY_PROVENANCE) != 0)
            .count();
        let mut receipts = BTreeMap::<IndexedOverlayStage, (u32, u32)>::new();
        for command in &state.commands {
            let writes = self.execute(command, indices, provenance, map_world)?;
            let receipt = receipts.entry(command.stage()).or_default();
            receipt.0 = receipt.0.saturating_add(1);
            receipt.1 = receipt.1.saturating_add(writes);
        }
        let receipts = state
            .required_stages
            .iter()
            .map(|stage| {
                let (command_count, pixel_writes) = receipts
                    .get(stage)
                    .copied()
                    .expect("closed command set has every required receipt");
                IndexedOverlayReceipt {
                    stage: *stage,
                    command_count,
                    pixel_writes,
                }
            })
            .collect();
        let after = provenance
            .iter()
            .filter(|value| (**value & OVERLAY_PROVENANCE) != 0)
            .count();
        Ok(IndexedOverlayOutput {
            receipts,
            pixels_written: u32::try_from(after.saturating_sub(before))
                .expect("fixed MAIN pixels fit u32"),
            help_sentinel: state.next_help_sentinel,
            command_hash: state.immutable_hash(),
            index_hash: byte_hash(indices),
            provenance_hash: byte_hash(provenance),
            output_bytes: LEVEL1_VIEW_PIXELS as u32,
        })
    }

    fn validate_command(&self, command: &IndexedOverlayCommand) -> Result<(), IndexedOverlayError> {
        match command {
            IndexedOverlayCommand::Decision { .. } => {}
            IndexedOverlayCommand::Bitmap { selector, .. }
            | IndexedOverlayCommand::TransparentBitmap { selector, .. }
            | IndexedOverlayCommand::ColourizedBitmap { selector, .. } => {
                if !self.bitmaps.contains_key(selector) {
                    return Err(IndexedOverlayError(format!(
                        "fixed bitmap selector {selector} missing"
                    )));
                }
            }
            IndexedOverlayCommand::LocalizedText { selector, .. } => {
                let text = self.localization(*selector).ok_or_else(|| {
                    IndexedOverlayError(format!("localization selector {selector} missing"))
                })?;
                self.validate_text(text)?;
            }
            IndexedOverlayCommand::Text { bytes, .. } => {
                self.validate_text(bytes)?;
            }
            IndexedOverlayCommand::Pixels {
                width,
                height,
                indices,
                mask,
                ..
            } => {
                let expected = usize::from(*width) * usize::from(*height);
                if indices.len() != expected || mask.len() != expected {
                    return Err(IndexedOverlayError(format!(
                        "typed pixel plane {width}x{height} has indices={} mask={}",
                        indices.len(),
                        mask.len()
                    )));
                }
            }
            IndexedOverlayCommand::BlendPixels {
                width,
                height,
                blend_indices,
                mask,
                ..
            } => {
                let expected = usize::from(*width) * usize::from(*height);
                if blend_indices.len() != expected || mask.len() != expected {
                    return Err(IndexedOverlayError(format!(
                        "typed blend plane {width}x{height} has indices={} mask={}",
                        blend_indices.len(),
                        mask.len()
                    )));
                }
                if self.tablesn.len() < 0x1_4000 {
                    return Err(IndexedOverlayError(
                        "TABLESN is too short for indexed blend".to_owned(),
                    ));
                }
            }
            IndexedOverlayCommand::BlendPixel { .. } => {
                if self.tablesn.len() < 0x1_4000 {
                    return Err(IndexedOverlayError(
                        "TABLESN is too short for indexed blend".to_owned(),
                    ));
                }
            }
            IndexedOverlayCommand::Objective { selector, .. } => {
                if !self.objectives.contains_key(selector) {
                    return Err(IndexedOverlayError(format!(
                        "objective selector {selector} missing"
                    )));
                }
            }
            IndexedOverlayCommand::SourceViewport {
                source_x,
                source_y,
                width,
                height,
                ..
            } => {
                if usize::from(*source_x) + usize::from(*width) > LEVEL1_VIEW_WIDTH
                    || usize::from(*source_y) + usize::from(*height) > LEVEL1_VIEW_HEIGHT
                {
                    return Err(IndexedOverlayError(
                        "source viewport exceeds the rendered MAIN".to_owned(),
                    ));
                }
            }
            IndexedOverlayCommand::Rect { .. } | IndexedOverlayCommand::Line { .. } => {}
        }
        Ok(())
    }

    fn validate_source_invariants(
        &self,
        state: &Level1FinalMainState,
    ) -> Result<(), IndexedOverlayError> {
        let mut stage_sequence = Vec::new();
        for command in &state.commands {
            if stage_sequence.last().copied() != Some(command.stage()) {
                stage_sequence.push(command.stage());
            }
        }
        if stage_sequence != state.required_stages {
            return Err(IndexedOverlayError(format!(
                "overlay command stage sequence {stage_sequence:?} does not match source receipt order {:?}",
                state.required_stages
            )));
        }
        for (stage, vector) in &self.spell_tier_vectors {
            for command in state
                .commands
                .iter()
                .filter(|command| command.stage() == *stage)
            {
                let IndexedOverlayCommand::Text {
                    bytes,
                    x,
                    y,
                    colour,
                    ..
                } = command
                else {
                    continue;
                };
                if !matches!(bytes.as_ref(), b"I" | b"II" | b"III" | b"IV" | b"V") {
                    continue;
                }
                let expected_x = vector.x_for_i
                    - 4 * (i16::try_from(bytes.len()).expect("tier text fits i16") - 1);
                if (*x, *y, *colour) != (expected_x, vector.y, vector.colour) {
                    return Err(IndexedOverlayError(format!(
                        "{stage:?} tier glyph {:?} at ({x},{y}) colour {colour} does not match source ({expected_x},{}) colour {}",
                        bytes, vector.y, vector.colour
                    )));
                }
            }
        }
        if state
            .required_stages
            .contains(&IndexedOverlayStage::TopStatus)
        {
            let first_visible = state
                .commands
                .iter()
                .find(|command| command.stage() == IndexedOverlayStage::TopStatus)
                .expect("closed stage has a command");
            if !matches!(
                first_visible,
                IndexedOverlayCommand::Bitmap {
                    selector: 40,
                    x: 1,
                    y: 1,
                    ..
                }
            ) {
                return Err(IndexedOverlayError(
                    "top-status source writer must begin with selector 40 at (1,1)".to_owned(),
                ));
            }
        }
        if state
            .required_stages
            .contains(&IndexedOverlayStage::BottomSpellMenu)
        {
            for family in 0..26 {
                let x = 4 + i16::try_from((family % 13) * 24).expect("menu x fits i16");
                let y = 164 + i16::try_from((family / 13) * 18).expect("menu y fits i16");
                let has_panel = state.commands.iter().any(|command| {
                    matches!(
                        command,
                        IndexedOverlayCommand::Bitmap {
                            stage: IndexedOverlayStage::BottomSpellMenu,
                            selector: 87 | 89 | 91,
                            x: panel_x,
                            y: panel_y,
                        } if (*panel_x, *panel_y) == (x, y)
                    )
                });
                if !has_panel {
                    return Err(IndexedOverlayError(format!(
                        "bottom spell-menu family {family} has no source panel at ({x},{y})"
                    )));
                }
            }
            let tier_panels = state
                .commands
                .iter()
                .filter_map(|command| match command {
                    IndexedOverlayCommand::Bitmap {
                        stage: IndexedOverlayStage::BottomSpellMenu,
                        selector: 161 | 162 | 163,
                        x,
                        y,
                    } => Some((*x, *y)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if !tier_panels.is_empty()
                && (tier_panels.len() != 3
                    || tier_panels.iter().any(|(_, y)| *y != 147)
                    || tier_panels[1].0 - tier_panels[0].0 != 33
                    || tier_panels[2].0 - tier_panels[1].0 != 33)
            {
                return Err(IndexedOverlayError(format!(
                    "bottom tier panels are not the source y147/33px vector: {tier_panels:?}"
                )));
            }
            let xp_colours = [
                self.palette_index(0).expect("qualified CLRN[0]"),
                self.palette_index(3840).expect("qualified CLRN[3840]"),
            ];
            if state.commands.iter().any(|command| {
                matches!(
                    command,
                    IndexedOverlayCommand::Rect {
                        stage: IndexedOverlayStage::BottomSpellMenu,
                        y: 161,
                        height: 1,
                        colour,
                        ..
                    } if !xp_colours.contains(colour)
                )
            }) {
                return Err(IndexedOverlayError(
                    "bottom experience line bypasses its CLRN source indirection".to_owned(),
                ));
            }
        }
        if state
            .required_stages
            .contains(&IndexedOverlayStage::MapWorldViewport)
        {
            if !state.commands.iter().any(|command| {
                matches!(
                    command,
                    IndexedOverlayCommand::SourceViewport {
                        stage: IndexedOverlayStage::MapWorldViewport,
                        source_x: 192,
                        source_y: 0,
                        x: 192,
                        y: 0,
                        width: 128,
                        height: 200,
                    }
                )
            }) {
                return Err(IndexedOverlayError(
                    "map has no executed 128x200 second-world viewport".to_owned(),
                ));
            }
            let divider_colour = self.palette_index(0).expect("qualified CLRN[0]");
            let dividers = state
                .commands
                .iter()
                .filter_map(|command| match command {
                    IndexedOverlayCommand::Line {
                        stage: IndexedOverlayStage::MapDividers,
                        from_x,
                        from_y: 0,
                        to_x,
                        to_y: 199,
                        colour,
                    } if from_x == to_x => Some((*from_x, *colour)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if dividers != [(192, divider_colour), (191, divider_colour)] {
                return Err(IndexedOverlayError(format!(
                    "map divider vector is {dividers:?}, expected x192,x191 CLRN[0]"
                )));
            }
        }
        if state
            .commands
            .iter()
            .any(|command| matches!(command, IndexedOverlayCommand::Bitmap { selector: 9, .. }))
        {
            return Err(IndexedOverlayError(
                "active cursor cannot earn a receipt before selector-9 bank provenance is proven"
                    .to_owned(),
            ));
        }
        if state.commands.iter().any(|command| {
            matches!(
                command,
                IndexedOverlayCommand::Rect {
                    stage: IndexedOverlayStage::PauseOrEndText,
                    x: 0,
                    y: 0,
                    width: 320,
                    height: 200,
                    ..
                }
            )
        }) {
            return Err(IndexedOverlayError(
                "PauseOrEnd must not fabricate a full-screen blank rectangle".to_owned(),
            ));
        }
        self.validate_menu_source_vectors(state)?;
        self.validate_pause_end_source_vectors(state)?;
        self.validate_map_source_vectors(state)?;
        self.validate_bottom_source_vectors(state)?;
        self.validate_debug_source_vectors(state)?;
        Ok(())
    }

    fn validate_menu_source_vectors(
        &self,
        state: &Level1FinalMainState,
    ) -> Result<(), IndexedOverlayError> {
        let commands = state
            .commands
            .iter()
            .filter(|command| command.stage() == IndexedOverlayStage::Menu)
            .collect::<Vec<_>>();
        if commands.is_empty() {
            return Ok(());
        }

        let pause = commands
            .iter()
            .filter_map(|command| match command {
                IndexedOverlayCommand::Bitmap { selector, x, y, .. }
                    if (174..=178).contains(selector) =>
                {
                    Some((*selector, *x, *y))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if !pause.is_empty() {
            let selectors =
                semantic_integers(&self.source_semantic_vectors, "/pause/selector_order")?;
            let mut expected = Vec::with_capacity(selectors.len());
            for (ordinal, selector) in selectors.into_iter().enumerate() {
                let xy = semantic_integers(
                    &self.source_semantic_vectors,
                    &format!("/pause/fixed_xy/{ordinal}"),
                )?;
                expected.push((
                    u16::try_from(selector).expect("validated pause selector"),
                    i16::try_from(xy[0]).expect("validated pause x"),
                    i16::try_from(xy[1]).expect("validated pause y"),
                ));
            }
            if pause != expected {
                return Err(IndexedOverlayError(format!(
                    "pause production vector is {pause:?}, expected {expected:?}"
                )));
            }
            let shade = self.palette_index(0).expect("qualified CLRN[0]");
            for (x, y) in [(134_i16, 54_i16), (134, 70), (160, 70)] {
                let passes = commands
                    .iter()
                    .filter(|command| {
                        matches!(
                            command,
                            IndexedOverlayCommand::BlendPixels {
                                x: actual_x,
                                y: actual_y,
                                blend_indices,
                                ..
                            } if (*actual_x, *actual_y) == (x, y)
                                && blend_indices.iter().all(|value| *value == shade)
                        )
                    })
                    .count();
                if !matches!(passes, 0 | 2) {
                    return Err(IndexedOverlayError(format!(
                        "pause shade at ({x},{y}) executed {passes} passes, expected zero or two"
                    )));
                }
            }
        }

        let option_rows = commands
            .iter()
            .filter_map(|command| match command {
                IndexedOverlayCommand::LocalizedText {
                    selector,
                    x,
                    y,
                    colour,
                    ..
                } if (34..=178).contains(y) => Some((*selector, *x, *y, *colour)),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !option_rows.is_empty() {
            if option_rows.len() != 17 {
                return Err(IndexedOverlayError(format!(
                    "options production vector has {} rows, expected 17",
                    option_rows.len()
                )));
            }
            let on = semantic_integers(&self.source_semantic_vectors, "/options/on_ids")?;
            let off = semantic_integers(&self.source_semantic_vectors, "/options/off_ids")?;
            let allowed_colours = [
                self.palette_index(3840).expect("qualified CLRN[3840]"),
                self.palette_index(4095).expect("qualified CLRN[4095]"),
                self.palette_index(0x888).expect("qualified CLRN[0x888]"),
            ];
            for (ordinal, (selector, x, y, colour)) in option_rows.iter().copied().enumerate() {
                if ![
                    u16::try_from(on[ordinal]).expect("validated options selector"),
                    u16::try_from(off[ordinal]).expect("validated options selector"),
                ]
                .contains(&selector)
                {
                    return Err(IndexedOverlayError(format!(
                        "options row {ordinal} uses selector {selector}, expected source on/off pair"
                    )));
                }
                let text = self
                    .localization(selector)
                    .expect("options localization was preflighted");
                let expected = (
                    160 - 2 * i16::try_from(text.len()).expect("option text fits i16"),
                    34 + 9 * i16::try_from(ordinal).expect("options ordinal fits i16"),
                );
                if (x, y) != expected || !allowed_colours.contains(&colour) {
                    return Err(IndexedOverlayError(format!(
                        "options row {ordinal} is ({x},{y}) colour {colour}, expected {expected:?} and a source CLRN colour"
                    )));
                }
            }
            let panel = commands
                .iter()
                .filter_map(|command| match command {
                    IndexedOverlayCommand::Bitmap {
                        selector: selector @ (168..=170),
                        x,
                        y,
                        ..
                    } => Some((*selector, *x, *y)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let mut expected = Vec::new();
            for ordinal in 0..17_i16 {
                let y = 33 + ordinal * 9;
                expected.push((168, 109, y));
                for repeat in 0..15_i16 {
                    expected.push((169, 117 + repeat * 6, y));
                }
                expected.push((170, 202, y));
            }
            for row in semantic_integer_rows(&self.source_semantic_vectors, "/options/ok")? {
                expected.push((
                    u16::try_from(row[0]).expect("validated OK selector"),
                    i16::try_from(row[1]).expect("validated OK x"),
                    i16::try_from(row[2]).expect("validated OK y"),
                ));
            }
            if panel != expected {
                return Err(IndexedOverlayError(
                    "options panel selector/coordinate/repeat vector differs from source ledger"
                        .to_owned(),
                ));
            }
            if !commands.iter().any(|command| {
                matches!(
                    command,
                    IndexedOverlayCommand::Text {
                        bytes,
                        x: 156,
                        y: 189,
                        colour,
                        ..
                    } if bytes.as_ref() == b"OK" && allowed_colours[..2].contains(colour)
                )
            }) {
                return Err(IndexedOverlayError(
                    "options OK text does not match the source position/colour".to_owned(),
                ));
            }
        }

        if commands
            .iter()
            .any(|command| matches!(command, IndexedOverlayCommand::Bitmap { selector: 259, .. }))
        {
            let buttons = commands
                .iter()
                .filter_map(|command| match command {
                    IndexedOverlayCommand::Bitmap {
                        selector: selector @ (259..=261),
                        x,
                        y,
                        ..
                    } => Some((*selector, *x, *y)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let expected =
                semantic_integer_rows(&self.source_semantic_vectors, "/chat/button_order")?
                    .into_iter()
                    .map(|row| {
                        (
                            u16::try_from(row[0]).expect("validated chat selector"),
                            i16::try_from(row[1]).expect("validated chat x"),
                            i16::try_from(row[2]).expect("validated chat y"),
                        )
                    })
                    .collect::<Vec<_>>();
            if buttons != expected {
                return Err(IndexedOverlayError(format!(
                    "chat button production vector is {buttons:?}, expected {expected:?}"
                )));
            }
            let channels = commands
                .iter()
                .filter_map(|command| match command {
                    IndexedOverlayCommand::Bitmap {
                        selector: selector @ (155 | 158),
                        x,
                        y,
                        ..
                    } => Some((*selector, *x, *y)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if channels.len() != 8
                || channels
                    .iter()
                    .filter(|(selector, _, _)| *selector == 155)
                    .count()
                    != 1
                || channels.iter().enumerate().any(|(ordinal, (_, x, y))| {
                    *x != 2 + 8 * i16::try_from(ordinal % 4).expect("channel x fits i16")
                        || *y != 178 + 10 * i16::try_from(ordinal / 4).expect("channel y fits i16")
                })
            {
                return Err(IndexedOverlayError(
                    "chat channel selector/coordinate vector differs from source".to_owned(),
                ));
            }
            let modes = commands
                .iter()
                .filter_map(|command| match command {
                    IndexedOverlayCommand::Bitmap {
                        selector: selector @ (156 | 159),
                        x,
                        y,
                        ..
                    } => Some((*selector, *x, *y)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let mode_xy = semantic_integer_rows(&self.source_semantic_vectors, "/chat/mode_xy")?;
            if modes.len() != 4
                || modes
                    .iter()
                    .filter(|(selector, _, _)| *selector == 156)
                    .count()
                    != 1
                || modes.iter().zip(mode_xy).any(|((_, x, y), xy)| {
                    *x != i16::try_from(xy[0]).expect("validated chat mode x")
                        || *y != i16::try_from(xy[1]).expect("validated chat mode y")
                })
            {
                return Err(IndexedOverlayError(
                    "chat mode selector/coordinate vector differs from source".to_owned(),
                ));
            }
            let remote = commands
                .iter()
                .filter_map(|command| match command {
                    IndexedOverlayCommand::Bitmap {
                        selector: selector @ (157 | 160),
                        x,
                        y: 177,
                        ..
                    } => Some((*selector, *x)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if remote.len() != 7
                || remote.iter().enumerate().any(|(ordinal, (_, x))| {
                    *x != 192 + 17 * i16::try_from(ordinal).expect("remote ordinal fits i16")
                })
            {
                return Err(IndexedOverlayError(
                    "chat remote-player tile vector differs from source".to_owned(),
                ));
            }
        }

        let volume = commands
            .iter()
            .filter_map(|command| match command {
                IndexedOverlayCommand::Rect {
                    x,
                    y,
                    width,
                    height,
                    colour,
                    ..
                } if (102..=114).contains(y) => Some((*x, *y, *width, *height, *colour)),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !volume.is_empty() {
            let expected_prefix = [
                (134, 102, 51, 12, 0x33),
                (134, 102, 51, 1, 0x11),
                (134, 113, 51, 1, 0x3b),
                (134, 102, 1, 11, 0x11),
                (184, 102, 1, 12, 0x3b),
                (136, 104, 47, 8, 0x33),
                (136, 104, 47, 1, 0x3b),
                (136, 111, 47, 1, 0x11),
                (136, 104, 1, 7, 0x3b),
                (182, 104, 1, 8, 0x11),
                (
                    137,
                    105,
                    45,
                    6,
                    self.palette_index(0).expect("qualified CLRN[0]"),
                ),
            ];
            if !volume.starts_with(&expected_prefix) {
                return Err(IndexedOverlayError(
                    "volume border/bar production order differs from source".to_owned(),
                ));
            }
            if let Some(fill) = volume.get(expected_prefix.len())
                && (fill.0, fill.1, fill.3, fill.4)
                    != (
                        137,
                        105,
                        6,
                        self.palette_index(240).expect("qualified CLRN[240]"),
                    )
            {
                return Err(IndexedOverlayError(
                    "volume foreground bypasses the source CLRN/coordinate vector".to_owned(),
                ));
            }
        }

        if commands
            .iter()
            .any(|command| matches!(command, IndexedOverlayCommand::Bitmap { selector: 257, .. }))
        {
            let buttons = commands
                .iter()
                .filter_map(|command| match command {
                    IndexedOverlayCommand::Bitmap {
                        selector: selector @ (257 | 258),
                        x,
                        y,
                        ..
                    } => Some((*selector, *x, *y)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let expected =
                semantic_integer_rows(&self.source_semantic_vectors, "/ok_cancel/button_order")?
                    .into_iter()
                    .map(|row| {
                        (
                            u16::try_from(row[0]).expect("validated OK/cancel selector"),
                            i16::try_from(row[1]).expect("validated OK/cancel x"),
                            i16::try_from(row[2]).expect("validated OK/cancel y"),
                        )
                    })
                    .collect::<Vec<_>>();
            if buttons != expected {
                return Err(IndexedOverlayError(format!(
                    "OK/cancel button production vector is {buttons:?}, expected {expected:?}"
                )));
            }
            let prompts = ["Abandon level.?", "Load Level?", "Save Level?"];
            if !commands.iter().any(|command| {
                matches!(
                    command,
                    IndexedOverlayCommand::Text { bytes, x, y: 25 | 3, colour, .. }
                        if (*x == 66 || *x == 3)
                            && prompts.iter().any(|prompt| prompt.as_bytes() == bytes.as_ref())
                            && [
                                self.palette_index(0).expect("qualified CLRN[0]"),
                                self.palette_index(3840).expect("qualified CLRN[3840]"),
                            ]
                            .contains(colour)
                )
            }) {
                return Err(IndexedOverlayError(
                    "OK/cancel prompt does not match source text/coordinate/colour".to_owned(),
                ));
            }
        }
        Ok(())
    }

    fn validate_pause_end_source_vectors(
        &self,
        state: &Level1FinalMainState,
    ) -> Result<(), IndexedOverlayError> {
        let commands = state
            .commands
            .iter()
            .filter(|command| command.stage() == IndexedOverlayStage::PauseOrEndText)
            .collect::<Vec<_>>();
        if commands.is_empty() {
            return Ok(());
        }
        let origin = if matches!(state.profile.menu_state, 6 | 7 | 8 | 11 | 12 | 14) {
            (3_i16, 3_i16)
        } else {
            (66_i16, 25_i16)
        };
        let paused = commands.iter().find_map(|command| match command {
            IndexedOverlayCommand::LocalizedText {
                selector: 425,
                x,
                y,
                colour,
                ..
            } => Some((*x, *y, *colour)),
            _ => None,
        });
        if let Some((x, y, colour)) = paused
            && ((x, y) != origin || colour != self.palette_index(240).expect("qualified CLRN[240]"))
        {
            return Err(IndexedOverlayError(format!(
                "paused text is ({x},{y}) colour {colour}, expected {origin:?} CLRN[240]"
            )));
        }
        if let Some((bytes, x, y, colour)) = commands.iter().find_map(|command| match command {
            IndexedOverlayCommand::Text {
                bytes,
                x,
                y,
                colour,
                ..
            } if bytes.as_ref() == b"Tasks completed. Fly to the exit point.." => {
                Some((bytes, *x, *y, *colour))
            }
            _ => None,
        }) {
            let expected_x = origin.0 + if paused.is_some() { 36 } else { 0 };
            if bytes.as_ref() != b"Tasks completed. Fly to the exit point.."
                || (x, y) != (expected_x, origin.1)
                || colour != self.palette_index(3840).expect("qualified CLRN[3840]")
            {
                return Err(IndexedOverlayError(
                    "end-of-level text differs from source punctuation/advance/colour".to_owned(),
                ));
            }
        }
        let allowed_notification_colours = [
            self.palette_index(0).expect("qualified CLRN[0]"),
            self.palette_index(240).expect("qualified CLRN[240]"),
            self.palette_index(3840).expect("qualified CLRN[3840]"),
        ];
        for command in commands {
            let IndexedOverlayCommand::Text {
                bytes,
                x,
                y,
                colour,
                ..
            } = command
            else {
                continue;
            };
            if bytes.as_ref() == b"Tasks completed. Fly to the exit point.." {
                continue;
            }
            let notification_y = origin.1
                + i16::from(state.commands.iter().any(|candidate| {
                    matches!(
                        candidate,
                        IndexedOverlayCommand::Text { bytes, .. }
                            if bytes.as_ref()
                                == b"Tasks completed. Fly to the exit point.."
                    )
                })) * 7;
            let notification_x = if notification_y == origin.1 && paused.is_some() {
                origin.0 + 36
            } else {
                origin.0
            };
            if (*x, *y) != (notification_x, notification_y)
                || !allowed_notification_colours.contains(colour)
            {
                return Err(IndexedOverlayError(format!(
                    "notification row is ({x},{y}) colour {colour}, expected ({notification_x},{notification_y}) and a source CLRN value"
                )));
            }
        }
        Ok(())
    }

    fn validate_map_source_vectors(
        &self,
        state: &Level1FinalMainState,
    ) -> Result<(), IndexedOverlayError> {
        if !state
            .required_stages
            .contains(&IndexedOverlayStage::MapTerrain)
        {
            return Ok(());
        }
        if !state.commands.iter().any(|command| {
            matches!(
                command,
                IndexedOverlayCommand::Pixels {
                    stage: IndexedOverlayStage::MapTerrain,
                    x: 0,
                    y: 0,
                    width: 191,
                    height: 200,
                    ..
                }
            )
        }) {
            return Err(IndexedOverlayError(
                "map terrain does not use the source 191x200 scale-408 raster".to_owned(),
            ));
        }
        let cross = state
            .commands
            .iter()
            .filter_map(|command| match command {
                IndexedOverlayCommand::BlendPixel {
                    stage: IndexedOverlayStage::MapEntities,
                    x,
                    y,
                    blend_index,
                } => Some((*x, *y, *blend_index)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let colour = self.palette_index(4095).expect("qualified CLRN[4095]");
        let mut expected_cross = Vec::with_capacity(64);
        for arm in 0..=15_i16 {
            expected_cross.extend([
                (94 - arm, 100, colour),
                (94 + arm, 100, colour),
                (94, 100 - arm, colour),
                (94, 100 + arm, colour),
            ]);
        }
        if !cross.ends_with(&expected_cross) {
            return Err(IndexedOverlayError(
                "map centre cross does not match source centre/radius/CLRN vector".to_owned(),
            ));
        }
        Ok(())
    }

    fn validate_bottom_source_vectors(
        &self,
        state: &Level1FinalMainState,
    ) -> Result<(), IndexedOverlayError> {
        if !state
            .required_stages
            .contains(&IndexedOverlayStage::BottomSpellMenu)
        {
            return Ok(());
        }
        let commands = state
            .commands
            .iter()
            .filter(|command| command.stage() == IndexedOverlayStage::BottomSpellMenu)
            .collect::<Vec<_>>();
        let edges = commands
            .iter()
            .filter_map(|command| match command {
                IndexedOverlayCommand::Bitmap {
                    selector: 88, x, y, ..
                } => Some((*x, *y)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let edge_width = self.bitmap_extent(88).expect("qualified edge selector").0;
        if edges
            != [
                (0, 164),
                (
                    320 - i16::try_from(edge_width).expect("edge width fits i16"),
                    164,
                ),
            ]
        {
            return Err(IndexedOverlayError(format!(
                "bottom edge vector is {edges:?}, expected both source edges"
            )));
        }
        for command in &commands {
            if let IndexedOverlayCommand::ColourizedBitmap {
                selector,
                x,
                y,
                colour,
                ..
            } = command
            {
                let family = selector.checked_sub(97).ok_or_else(|| {
                    IndexedOverlayError("bottom colourized selector precedes family 0".to_owned())
                })?;
                if family >= 26
                    || *colour != 0xa6
                    || *x != 4 + 24 * i16::try_from(family % 13).expect("family x fits i16")
                    || *y != 164 + 18 * i16::try_from(family / 13).expect("family y fits i16")
                {
                    return Err(IndexedOverlayError(
                        "bottom acquired-family colourized icon differs from source formula"
                            .to_owned(),
                    ));
                }
            }
        }
        let selected = commands
            .iter()
            .filter_map(|command| match command {
                IndexedOverlayCommand::Bitmap {
                    selector: 90, x, y, ..
                } => Some((*x, *y)),
                _ => None,
            })
            .collect::<Vec<_>>();
        if selected.len() != 1 {
            return Err(IndexedOverlayError(format!(
                "bottom selected-frame count is {}, expected one",
                selected.len()
            )));
        }
        let tier_panels = commands
            .iter()
            .filter_map(|command| match command {
                IndexedOverlayCommand::Bitmap {
                    selector: selector @ (161..=163),
                    x,
                    y,
                    ..
                } => Some((*selector, *x, *y)),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !tier_panels.is_empty() {
            if tier_panels.len() != 3
                || tier_panels.iter().any(|(_, _, y)| *y != 147)
                || tier_panels[1].1 - tier_panels[0].1 != 33
                || tier_panels[2].1 - tier_panels[1].1 != 33
            {
                return Err(IndexedOverlayError(
                    "bottom tier panel production vector differs from y147/spacing33".to_owned(),
                ));
            }
            let family = usize::try_from((selected[0].0 - 4) / 24)
                .expect("selected family column is nonnegative")
                + if selected[0].1 >= 182 { 13 } else { 0 };
            let tier_icons = commands
                .iter()
                .filter_map(|command| match command {
                    IndexedOverlayCommand::Bitmap {
                        selector,
                        x,
                        y: 150,
                        ..
                    }
                    | IndexedOverlayCommand::TransparentBitmap {
                        selector,
                        x,
                        y: 150,
                        ..
                    } if (179..=256).contains(selector) => Some((*selector, *x)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            for (tier, (selector, x)) in tier_icons.iter().copied().enumerate() {
                let expected_selector =
                    179 + u16::try_from(3 * family + tier).expect("tier selector fits u16");
                let expected_x = tier_panels[tier].1 + 9;
                if selector != expected_selector || x != expected_x {
                    return Err(IndexedOverlayError(format!(
                        "bottom family {family} tier {tier} icon is selector {selector} x{x}, expected {expected_selector} x{expected_x}"
                    )));
                }
            }
        }
        Ok(())
    }

    fn validate_debug_source_vectors(
        &self,
        state: &Level1FinalMainState,
    ) -> Result<(), IndexedOverlayError> {
        let commands = state
            .commands
            .iter()
            .filter(|command| command.stage() == IndexedOverlayStage::DebugText)
            .collect::<Vec<_>>();
        let text = commands
            .iter()
            .filter_map(|command| match command {
                IndexedOverlayCommand::Text {
                    bytes,
                    x,
                    y,
                    colour,
                    ..
                } => Some((bytes.as_ref(), *x, *y, *colour)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let label = self.palette_index(3840).expect("qualified CLRN[3840]");
        let value = self.palette_index(15).expect("qualified CLRN[15]");
        if text
            .iter()
            .any(|(bytes, _, _, _)| *bytes == b"Product name")
        {
            let static_rows: &[(usize, &[u8], u8)] = &[
                (0, b"Product name", label),
                (1, b"Magic Carpet 2 (Netherworlds)", value),
                (2, b"Version number", label),
                (3, b"Beta", value),
                (4, b"Version date", label),
                (5, b"03:42:13 Sep 06 1995", value),
                (6, b"Programmer", label),
                (7, b"Bullfrog, Alan Wright.", value),
                (8, b"Supplied to", label),
                (9, b"PUBLIC", value),
                (10, b"Level Number", label),
                (12, b"FPS", label),
                (14, b"Sound Number", label),
                (16, b"Game turn", label),
                (18, b"Thing", label),
                (23, b"Memory (Used/Free)", label),
            ];
            if text.len() < 25 {
                return Err(IndexedOverlayError(format!(
                    "debug flag8 has {} text rows, expected at least 25",
                    text.len()
                )));
            }
            for (ordinal, expected_bytes, expected_colour) in static_rows {
                let (bytes, x, y, colour) = text[*ordinal];
                if bytes != *expected_bytes
                    || (x, y) != (160, 7 * i16::try_from(*ordinal).expect("debug y fits i16"))
                    || colour != *expected_colour
                {
                    return Err(IndexedOverlayError(format!(
                        "debug flag8 row {ordinal} differs from source text/position/CLRN"
                    )));
                }
            }
            for (ordinal, prefix) in [
                (20, b"Carpet 224791".as_slice()),
                (21, b"Tape 38547"),
                (22, b"Heap "),
            ] {
                let (bytes, x, y, colour) = text[ordinal];
                if !bytes.starts_with(prefix)
                    || (x, y) != (160, 7 * i16::try_from(ordinal).expect("debug y fits i16"))
                    || colour != value
                {
                    return Err(IndexedOverlayError(format!(
                        "debug flag8 dynamic row {ordinal} differs from source"
                    )));
                }
            }
        }
        if let Some(start) = text.iter().position(|(bytes, _, _, _)| *bytes == b"THING") {
            let selected = self.palette_index(255).expect("qualified CLRN[255]");
            let labels: &[(usize, &[u8])] = &[
                (0, b"THING"),
                (2, b"CLASS / MODEL / STATE"),
                (4, b"LIFE / MAX LIFE"),
                (6, b"SPEED ACTUAL / MINIMUM / MAXIMUM"),
                (8, b"ACTUAL X / Y / Z"),
                (10, b"ID / WHO OWNS ME"),
                (12, b"MANA ACTUAL / MAXIMUM"),
            ];
            if text.len() < start + 14 {
                return Err(IndexedOverlayError(
                    "debug flag0x40 is missing source rows".to_owned(),
                ));
            }
            for offset in 0..14 {
                let (_, x, y, colour) = text[start + offset];
                let expected_colour = if offset % 2 == 0 { label } else { selected };
                if (x, y)
                    != (
                        5,
                        42 + 7 * i16::try_from(offset).expect("debug offset fits i16"),
                    )
                    || colour != expected_colour
                {
                    return Err(IndexedOverlayError(format!(
                        "debug flag0x40 row {offset} differs from source position/CLRN"
                    )));
                }
            }
            for (offset, expected) in labels {
                if text[start + *offset].0 != *expected {
                    return Err(IndexedOverlayError(format!(
                        "debug flag0x40 label {offset} differs from source"
                    )));
                }
            }
        }
        Ok(())
    }

    fn execute(
        &self,
        command: &IndexedOverlayCommand,
        indices: &mut [u8],
        provenance: &mut [u8],
        map_world: Option<&[u8]>,
    ) -> Result<u32, IndexedOverlayError> {
        match command {
            IndexedOverlayCommand::Decision { .. } => Ok(0),
            IndexedOverlayCommand::Bitmap { selector, x, y, .. } => {
                self.draw_bitmap(*selector, indices, provenance, i32::from(*x), i32::from(*y))
            }
            IndexedOverlayCommand::TransparentBitmap { selector, x, y, .. } => {
                let bitmap = self
                    .bitmaps
                    .get(selector)
                    .expect("transparent bitmap command was preflighted");
                Ok(draw_blend_pixels(
                    indices,
                    provenance,
                    i32::from(*x),
                    i32::from(*y),
                    bitmap.width,
                    bitmap.height,
                    &bitmap.indices,
                    &bitmap.mask,
                    &self.tablesn,
                ))
            }
            IndexedOverlayCommand::ColourizedBitmap {
                selector,
                x,
                y,
                colour,
                ..
            } => {
                let bitmap = self
                    .bitmaps
                    .get(selector)
                    .expect("colourized bitmap command was preflighted");
                Ok(draw_colourized_bitmap(
                    indices,
                    provenance,
                    i32::from(*x),
                    i32::from(*y),
                    bitmap,
                    *colour,
                ))
            }
            IndexedOverlayCommand::LocalizedText {
                selector,
                x,
                y,
                colour,
                ..
            } => self.draw_text(
                indices,
                provenance,
                i32::from(*x),
                i32::from(*y),
                self.localization(*selector)
                    .expect("localized command was preflighted"),
                *colour,
            ),
            IndexedOverlayCommand::Text {
                bytes,
                x,
                y,
                colour,
                ..
            } => self.draw_text(
                indices,
                provenance,
                i32::from(*x),
                i32::from(*y),
                bytes,
                *colour,
            ),
            IndexedOverlayCommand::Rect {
                x,
                y,
                width,
                height,
                colour,
                ..
            } => Ok(draw_rect(
                indices,
                provenance,
                i32::from(*x),
                i32::from(*y),
                *width,
                *height,
                *colour,
            )),
            IndexedOverlayCommand::Line {
                from_x,
                from_y,
                to_x,
                to_y,
                colour,
                ..
            } => Ok(draw_line(
                indices,
                provenance,
                i32::from(*from_x),
                i32::from(*from_y),
                i32::from(*to_x),
                i32::from(*to_y),
                *colour,
            )),
            IndexedOverlayCommand::Pixels {
                x,
                y,
                width,
                height,
                indices: source,
                mask,
                ..
            } => Ok(draw_pixels(
                indices,
                provenance,
                i32::from(*x),
                i32::from(*y),
                *width,
                *height,
                source,
                mask,
            )),
            IndexedOverlayCommand::BlendPixels {
                x,
                y,
                width,
                height,
                blend_indices,
                mask,
                ..
            } => Ok(draw_blend_pixels(
                indices,
                provenance,
                i32::from(*x),
                i32::from(*y),
                *width,
                *height,
                blend_indices,
                mask,
                &self.tablesn,
            )),
            IndexedOverlayCommand::BlendPixel {
                x, y, blend_index, ..
            } => Ok(u32::from(blend_pixel(
                indices,
                provenance,
                i32::from(*x),
                i32::from(*y),
                *blend_index,
                &self.tablesn,
            ))),
            IndexedOverlayCommand::Objective { selector, .. } => {
                self.draw_objective(*selector, indices, provenance)
            }
            IndexedOverlayCommand::SourceViewport {
                source_x,
                source_y,
                x,
                y,
                width,
                height,
                ..
            } => {
                let map_world = map_world.ok_or_else(|| {
                    IndexedOverlayError(
                        "map viewport command has no executed second DrawWorld backing".to_owned(),
                    )
                })?;
                Ok(draw_source_viewport(
                    indices,
                    provenance,
                    map_world,
                    *source_x,
                    *source_y,
                    i32::from(*x),
                    i32::from(*y),
                    *width,
                    *height,
                ))
            }
        }
    }

    fn validate_text(&self, text: &[u8]) -> Result<(), IndexedOverlayError> {
        for byte in text.iter().take_while(|byte| **byte != 0) {
            let index = match *byte {
                b'\r' => continue,
                b'\t' | b' ' => 33,
                b'\n' => 34,
                byte => usize::from(byte) + 1,
            };
            if self.glyphs.get(index).is_none() {
                return Err(IndexedOverlayError(format!("FONT1 glyph {index} missing")));
            }
        }
        Ok(())
    }

    fn draw_bitmap(
        &self,
        selector: u16,
        indices: &mut [u8],
        provenance: &mut [u8],
        x: i32,
        y: i32,
    ) -> Result<u32, IndexedOverlayError> {
        let bitmap = self
            .bitmaps
            .get(&selector)
            .expect("bitmap command was preflighted");
        Ok(draw_pixels(
            indices,
            provenance,
            x,
            y,
            bitmap.width,
            bitmap.height,
            &bitmap.indices,
            &bitmap.mask,
        ))
    }

    fn draw_text(
        &self,
        indices: &mut [u8],
        provenance: &mut [u8],
        x: i32,
        y: i32,
        text: &[u8],
        colour: u8,
    ) -> Result<u32, IndexedOverlayError> {
        self.validate_text(text)?;
        draw_source_text(&self.glyphs, &self.font_masks, x, y, text, 1, 1,
            |x, y| write_pixel(indices, provenance, x, y, colour))
    }

    fn draw_objective(
        &self,
        selector: u8,
        indices: &mut [u8],
        provenance: &mut [u8],
    ) -> Result<u32, IndexedOverlayError> {
        let objective = self
            .objectives
            .get(&selector)
            .expect("objective command was preflighted");
        let objective_indices =
            downsample_objective(&objective.indices, usize::from(objective.width));
        let objective_mask = downsample_objective(&objective.mask, usize::from(objective.width));
        Ok(draw_pixels(
            indices,
            provenance,
            i32::from(objective.reference_x / 2),
            i32::from(objective.reference_y / 2),
            objective.width / 2,
            objective.height / 2,
            &objective_indices,
            &objective_mask,
        ))
    }
}

fn validate_profile(profile: FixedOverlayProfile) -> Result<(), IndexedOverlayError> {
    if profile.stereo
        || profile.alternate_resolution
        || profile.dac_output
        || profile.hostile_or_remote_player
    {
        return Err(IndexedOverlayError(
            "fixed compositor is incomplete for stereo/alternate/DAC/remote profile".to_owned(),
        ));
    }
    if !matches!(
        profile.menu_state,
        0 | 3 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14
    ) {
        return Err(IndexedOverlayError(format!(
            "fixed compositor has no reachable menu-state branch {}",
            profile.menu_state
        )));
    }
    Ok(())
}

fn overlay_branch_key(profile: FixedOverlayProfile) -> String {
    if !profile.player_alive {
        format!("dead-{}", profile.menu_state)
    } else if matches!(profile.menu_state, 6 | 7 | 8 | 11 | 12 | 14) {
        format!("map-{}", profile.menu_state)
    } else {
        format!("ordinary-{}", profile.menu_state)
    }
}

fn overlay_stage_from_name(name: &str) -> Result<IndexedOverlayStage, IndexedOverlayError> {
    let stage = match name {
        "DeadPlayerMenu" => IndexedOverlayStage::DeadPlayerMenu,
        "SpiderWeb" => IndexedOverlayStage::SpiderWeb,
        "MinimapTerrain" => IndexedOverlayStage::MinimapTerrain,
        "MinimapEntities" => IndexedOverlayStage::MinimapEntities,
        "MinimapMarks" => IndexedOverlayStage::MinimapMarks,
        "LeftSpell" => IndexedOverlayStage::LeftSpell,
        "RightSpell" => IndexedOverlayStage::RightSpell,
        "TopStatus" => IndexedOverlayStage::TopStatus,
        "Menu" => IndexedOverlayStage::Menu,
        "PauseOrEndText" => IndexedOverlayStage::PauseOrEndText,
        "BottomSpellMenu" => IndexedOverlayStage::BottomSpellMenu,
        "ObjectiveTextbox" => IndexedOverlayStage::ObjectiveTextbox,
        "TutorialOrBriefing" => IndexedOverlayStage::TutorialOrBriefing,
        "HelpPopup" => IndexedOverlayStage::HelpPopup,
        "DebugText" => IndexedOverlayStage::DebugText,
        "MapClear" => IndexedOverlayStage::MapClear,
        "MapTerrain" => IndexedOverlayStage::MapTerrain,
        "MapEntities" => IndexedOverlayStage::MapEntities,
        "MapWorldViewport" => IndexedOverlayStage::MapWorldViewport,
        "MapDividers" => IndexedOverlayStage::MapDividers,
        "MapMarks" => IndexedOverlayStage::MapMarks,
        _ => {
            return Err(IndexedOverlayError(format!(
                "unknown overlay receipt stage {name}"
            )));
        }
    };
    Ok(stage)
}

fn draw_pixels(
    indices: &mut [u8],
    provenance: &mut [u8],
    x: i32,
    y: i32,
    width: u16,
    height: u16,
    source: &[u8],
    mask: &[u8],
) -> u32 {
    let mut writes = 0_u32;
    for row in 0..usize::from(height) {
        for column in 0..usize::from(width) {
            let offset = row * usize::from(width) + column;
            if mask[offset] == 0 {
                continue;
            }
            writes += u32::from(write_pixel(
                indices,
                provenance,
                x + i32::try_from(column).expect("pixel width fits i32"),
                y + i32::try_from(row).expect("pixel height fits i32"),
                source[offset],
            ));
        }
    }
    writes
}

fn draw_colourized_bitmap(
    indices: &mut [u8],
    provenance: &mut [u8],
    x: i32,
    y: i32,
    bitmap: &IndexedBitmap,
    colour: u8,
) -> u32 {
    let mut writes = 0_u32;
    for row in 0..usize::from(bitmap.height) {
        for column in 0..usize::from(bitmap.width) {
            let offset = row * usize::from(bitmap.width) + column;
            if bitmap.mask[offset] == 0 {
                continue;
            }
            writes += u32::from(write_pixel(
                indices,
                provenance,
                x + i32::try_from(column).expect("bitmap width fits i32"),
                y + i32::try_from(row).expect("bitmap height fits i32"),
                colour,
            ));
        }
    }
    writes
}

#[allow(clippy::too_many_arguments)]
fn draw_source_viewport(
    indices: &mut [u8],
    provenance: &mut [u8],
    source_main: &[u8],
    source_x: u16,
    source_y: u16,
    x: i32,
    y: i32,
    width: u16,
    height: u16,
) -> u32 {
    let mut writes = 0_u32;
    for row in 0..usize::from(height) {
        for column in 0..usize::from(width) {
            let source =
                (usize::from(source_y) + row) * LEVEL1_VIEW_WIDTH + usize::from(source_x) + column;
            writes += u32::from(write_pixel(
                indices,
                provenance,
                x + i32::try_from(column).expect("viewport width fits i32"),
                y + i32::try_from(row).expect("viewport height fits i32"),
                source_main[source],
            ));
        }
    }
    writes
}

fn draw_rect(
    indices: &mut [u8],
    provenance: &mut [u8],
    x: i32,
    y: i32,
    width: u16,
    height: u16,
    colour: u8,
) -> u32 {
    let mut writes = 0_u32;
    for row in 0..i32::from(height) {
        for column in 0..i32::from(width) {
            writes += u32::from(write_pixel(
                indices,
                provenance,
                x + column,
                y + row,
                colour,
            ));
        }
    }
    writes
}

#[allow(clippy::too_many_arguments)]
fn draw_blend_pixels(
    indices: &mut [u8],
    provenance: &mut [u8],
    x: i32,
    y: i32,
    width: u16,
    height: u16,
    blend_indices: &[u8],
    mask: &[u8],
    tablesn: &[u8],
) -> u32 {
    let mut writes = 0_u32;
    for row in 0..usize::from(height) {
        for column in 0..usize::from(width) {
            let source = row * usize::from(width) + column;
            if mask[source] == 0 {
                continue;
            }
            writes += u32::from(blend_pixel(
                indices,
                provenance,
                x + i32::try_from(column).expect("blend width fits i32"),
                y + i32::try_from(row).expect("blend height fits i32"),
                blend_indices[source],
                tablesn,
            ));
        }
    }
    writes
}

fn blend_pixel(
    indices: &mut [u8],
    provenance: &mut [u8],
    x: i32,
    y: i32,
    blend_index: u8,
    tablesn: &[u8],
) -> bool {
    if !(0..LEVEL1_VIEW_WIDTH as i32).contains(&x) || !(0..LEVEL1_VIEW_HEIGHT as i32).contains(&y) {
        return false;
    }
    let offset = usize::try_from(y).expect("clipped y") * LEVEL1_VIEW_WIDTH
        + usize::try_from(x).expect("clipped x");
    indices[offset] =
        tablesn[0x4000 + (usize::from(blend_index) << 8) + usize::from(indices[offset])];
    provenance[offset] |= OVERLAY_PROVENANCE;
    true
}

#[allow(clippy::too_many_arguments)]
fn draw_line(
    indices: &mut [u8],
    provenance: &mut [u8],
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    colour: u8,
) -> u32 {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut error = dx + dy;
    let mut writes = 0_u32;
    loop {
        writes += u32::from(write_pixel(indices, provenance, x0, y0, colour));
        if x0 == x1 && y0 == y1 {
            break;
        }
        let doubled = 2 * error;
        if doubled >= dy {
            error += dy;
            x0 += sx;
        }
        if doubled <= dx {
            error += dx;
            y0 += sy;
        }
    }
    writes
}

fn write_pixel(indices: &mut [u8], provenance: &mut [u8], x: i32, y: i32, colour: u8) -> bool {
    if !(0..LEVEL1_VIEW_WIDTH as i32).contains(&x) || !(0..LEVEL1_VIEW_HEIGHT as i32).contains(&y) {
        return false;
    }
    let offset = usize::try_from(y).expect("clipped y") * LEVEL1_VIEW_WIDTH
        + usize::try_from(x).expect("clipped x");
    indices[offset] = colour;
    provenance[offset] |= OVERLAY_PROVENANCE;
    true
}

fn downsample_objective(bytes: &[u8], width: usize) -> Vec<u8> {
    let height = bytes.len() / width;
    let mut output = Vec::with_capacity((width / 2) * (height / 2));
    for y in 0..height / 2 {
        for x in 0..width / 2 {
            output.push(bytes[(y * 2) * width + x * 2]);
        }
    }
    output
}

fn split_nul_records(bytes: &[u8]) -> Vec<Box<[u8]>> {
    let mut records = bytes
        .split(|byte| *byte == 0)
        .map(|row| row.to_vec().into_boxed_slice())
        .collect::<Vec<_>>();
    if bytes.last() == Some(&0) {
        records.pop();
    }
    records
}

fn read_i32le(bytes: &[u8]) -> Result<Vec<i32>, IndexedOverlayError> {
    if bytes.len() % 4 != 0 {
        return Err(IndexedOverlayError(format!(
            "i32le payload has {} bytes",
            bytes.len()
        )));
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

fn hash_command(command: &IndexedOverlayCommand, bytes: &mut Vec<u8>) {
    bytes.push(command.stage() as u8);
    match command {
        IndexedOverlayCommand::Decision { state_hash, .. } => {
            bytes.push(9);
            bytes.extend_from_slice(&state_hash.to_le_bytes());
        }
        IndexedOverlayCommand::Bitmap { selector, x, y, .. } => {
            bytes.push(0);
            bytes.extend_from_slice(&selector.to_le_bytes());
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
        }
        IndexedOverlayCommand::TransparentBitmap { selector, x, y, .. } => {
            bytes.push(10);
            bytes.extend_from_slice(&selector.to_le_bytes());
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
        }
        IndexedOverlayCommand::ColourizedBitmap {
            selector,
            x,
            y,
            colour,
            ..
        } => {
            bytes.push(12);
            bytes.extend_from_slice(&selector.to_le_bytes());
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
            bytes.push(*colour);
        }
        IndexedOverlayCommand::LocalizedText {
            selector,
            x,
            y,
            colour,
            ..
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&selector.to_le_bytes());
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
            bytes.push(*colour);
        }
        IndexedOverlayCommand::Text {
            bytes: text,
            x,
            y,
            colour,
            ..
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
            bytes.push(*colour);
            bytes.extend_from_slice(
                &u32::try_from(text.len())
                    .expect("text length fits u32")
                    .to_le_bytes(),
            );
            bytes.extend_from_slice(text);
        }
        IndexedOverlayCommand::Rect {
            x,
            y,
            width,
            height,
            colour,
            ..
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
            bytes.extend_from_slice(&width.to_le_bytes());
            bytes.extend_from_slice(&height.to_le_bytes());
            bytes.push(*colour);
        }
        IndexedOverlayCommand::Line {
            from_x,
            from_y,
            to_x,
            to_y,
            colour,
            ..
        } => {
            bytes.push(4);
            bytes.extend_from_slice(&from_x.to_le_bytes());
            bytes.extend_from_slice(&from_y.to_le_bytes());
            bytes.extend_from_slice(&to_x.to_le_bytes());
            bytes.extend_from_slice(&to_y.to_le_bytes());
            bytes.push(*colour);
        }
        IndexedOverlayCommand::Pixels {
            x,
            y,
            width,
            height,
            indices,
            mask,
            ..
        } => {
            bytes.push(5);
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
            bytes.extend_from_slice(&width.to_le_bytes());
            bytes.extend_from_slice(&height.to_le_bytes());
            bytes.extend_from_slice(&byte_hash(indices).to_le_bytes());
            bytes.extend_from_slice(&byte_hash(mask).to_le_bytes());
        }
        IndexedOverlayCommand::BlendPixels {
            x,
            y,
            width,
            height,
            blend_indices,
            mask,
            ..
        } => {
            bytes.push(6);
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
            bytes.extend_from_slice(&width.to_le_bytes());
            bytes.extend_from_slice(&height.to_le_bytes());
            bytes.extend_from_slice(&byte_hash(blend_indices).to_le_bytes());
            bytes.extend_from_slice(&byte_hash(mask).to_le_bytes());
        }
        IndexedOverlayCommand::BlendPixel {
            x, y, blend_index, ..
        } => {
            bytes.push(7);
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
            bytes.push(*blend_index);
        }
        IndexedOverlayCommand::Objective { selector, .. } => {
            bytes.push(8);
            bytes.push(*selector);
        }
        IndexedOverlayCommand::SourceViewport {
            source_x,
            source_y,
            x,
            y,
            width,
            height,
            ..
        } => {
            bytes.push(11);
            bytes.extend_from_slice(&source_x.to_le_bytes());
            bytes.extend_from_slice(&source_y.to_le_bytes());
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
            bytes.extend_from_slice(&width.to_le_bytes());
            bytes.extend_from_slice(&height.to_le_bytes());
        }
    }
}

fn read_qualified(
    cache: &QualifiedOwnerCacheReader,
    relative: &str,
) -> Result<Vec<u8>, IndexedOverlayError> {
    cache
        .read(relative)
        .map_err(|error| IndexedOverlayError(error.to_string()))
}

fn json_str<'a>(value: &'a Value, key: &str) -> Result<&'a str, IndexedOverlayError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| IndexedOverlayError(format!("JSON key {key} is not a string")))
}

fn json_u8(value: &Value, key: &str) -> Result<u8, IndexedOverlayError> {
    u8::try_from(json_u64(value, key)?)
        .map_err(|_| IndexedOverlayError(format!("JSON key {key} exceeds u8")))
}

fn json_u16(value: &Value, key: &str) -> Result<u16, IndexedOverlayError> {
    u16::try_from(json_u64(value, key)?)
        .map_err(|_| IndexedOverlayError(format!("JSON key {key} exceeds u16")))
}

fn json_u32(value: &Value, key: &str) -> Result<u32, IndexedOverlayError> {
    u32::try_from(json_u64(value, key)?)
        .map_err(|_| IndexedOverlayError(format!("JSON key {key} exceeds u32")))
}

fn json_u64(value: &Value, key: &str) -> Result<u64, IndexedOverlayError> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| IndexedOverlayError(format!("JSON key {key} is not unsigned")))
}

fn json_i32_triplet(value: &Value, key: &str) -> Result<[i32; 3], IndexedOverlayError> {
    let rows = value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| IndexedOverlayError(format!("{key} is not an array")))?;
    if rows.len() != 3 {
        return Err(IndexedOverlayError(format!(
            "{key} has {} entries, expected 3",
            rows.len()
        )));
    }
    Ok(std::array::from_fn(|index| {
        i32::try_from(
            rows[index]
                .as_i64()
                .expect("validated integer spell profile"),
        )
        .expect("spell profile value fits i32")
    }))
}

fn validate_source_semantic_vectors(
    vectors: &Value,
    clrn: &[u8],
    localization: &[Box<[u8]>],
) -> Result<(), IndexedOverlayError> {
    let integers = |pointer: &str| -> Result<Vec<i64>, IndexedOverlayError> {
        vectors
            .pointer(pointer)
            .and_then(Value::as_array)
            .ok_or_else(|| {
                IndexedOverlayError(format!("source-semantic vector {pointer} is not an array"))
            })?
            .iter()
            .map(|value| {
                value.as_i64().ok_or_else(|| {
                    IndexedOverlayError(format!(
                        "source-semantic vector {pointer} contains a non-integer"
                    ))
                })
            })
            .collect()
    };
    let exact_integer_vectors: &[(&str, &[i64])] = &[
        (
            "/palette_indirection/source_offsets",
            &[0, 15, 240, 255, 0x888, 3840, 4095],
        ),
        (
            "/palette_indirection/indexed_values",
            &[61, 23, 171, 87, 177, 119, 176],
        ),
        ("/pause/selector_order", &[174, 175, 176, 177, 178]),
        ("/pause/fixed_xy/0", &[134, 38]),
        ("/pause/fixed_xy/1", &[134, 54]),
        ("/pause/fixed_xy/2", &[134, 70]),
        ("/pause/fixed_xy/3", &[160, 70]),
        ("/pause/fixed_xy/4", &[134, 86]),
        ("/chat/entry_text_xy", &[5, 167]),
        ("/chat/caret_xy_offset", &[5, 168]),
        ("/chat/mode_xy/0", &[41, 180]),
        ("/chat/mode_xy/1", &[76, 180]),
        ("/chat/mode_xy/2", &[111, 180]),
        ("/chat/mode_xy/3", &[155, 180]),
        ("/chat/mode_inner_order", &[154, 153, 152, 151]),
        ("/chat/button_order/0", &[259, 192, 166]),
        ("/chat/button_order/1", &[261, 237, 166]),
        ("/chat/button_order/2", &[260, 282, 166]),
        ("/chat/hover_text_xy_colour_offset", &[195, 190, 240]),
        (
            "/options/on_ids",
            &[
                400, 390, 392, 469, 398, 7, 382, 13, 359, 363, 357, 395, 365, 441, 442, 468, 402,
            ],
        ),
        (
            "/options/off_ids",
            &[
                401, 391, 393, 470, 399, 7, 381, 14, 360, 364, 358, 396, 366, 440, 443, 468, 403,
            ],
        ),
        ("/options/ok/0", &[168, 139, 188]),
        ("/options/ok/1", &[169, 147, 188]),
        ("/options/ok/2", &[169, 153, 188]),
        ("/options/ok/3", &[169, 159, 188]),
        ("/options/ok/4", &[169, 165, 188]),
        ("/options/ok/5", &[169, 171, 188]),
        ("/options/ok/6", &[170, 172, 188]),
        ("/volume/source_outer", &[268, 204, 102, 24]),
        ("/volume/source_inner", &[272, 208, 94, 16]),
        ("/volume/source_bar", &[274, 210, 90, 12]),
        ("/ok_cancel/localized_ids", &[2, 423, 424]),
        ("/ok_cancel/ordinary_xy", &[66, 25]),
        ("/ok_cancel/map_dead_xy", &[3, 3]),
        ("/ok_cancel/button_order/0", &[257, 135, 52]),
        ("/ok_cancel/button_order/1", &[258, 160, 52]),
        ("/pause_or_end/ordinary_xy", &[66, 25]),
        ("/pause_or_end/map_xy", &[3, 3]),
        ("/pause_or_end/end_ids", &[378, 379]),
        ("/pause_or_end/timer_edge", &[0, 1, 100, 101]),
        ("/map/left_size", &[191, 200]),
        ("/map/right_viewport", &[192, 0, 128, 200]),
        ("/map/divider_order", &[192, 191]),
        ("/map/centre_cross", &[94, 100, 15, 4095]),
        ("/bottom/slot_origin", &[4, 164]),
        ("/bottom/slot_step", &[24, 18]),
        ("/bottom/tier_panels", &[161, 162, 163]),
        ("/debug/flag40_xy", &[5, 42]),
        ("/cursor/msp_dimensions", &[5, 1]),
        ("/cursor/pointer_dimensions", &[12, 13]),
    ];
    for (pointer, expected) in exact_integer_vectors {
        let actual = integers(pointer)?;
        if actual != *expected {
            return Err(IndexedOverlayError(format!(
                "source-semantic vector {pointer} is {actual:?}, expected {expected:?}"
            )));
        }
    }
    let offsets = integers("/palette_indirection/source_offsets")?;
    let values = integers("/palette_indirection/indexed_values")?;
    for (offset, expected) in offsets.into_iter().zip(values) {
        let actual = clrn
            .get(usize::try_from(offset).expect("validated CLRN offset"))
            .copied()
            .ok_or_else(|| IndexedOverlayError("semantic CLRN offset missing".to_owned()))?;
        if i64::from(actual) != expected {
            return Err(IndexedOverlayError(format!(
                "CLRN[{offset}] is {actual}, expected {expected}"
            )));
        }
    }
    for (selector, expected) in [
        (2_usize, b"Abandon level.".as_slice()),
        (423, b"Load Level"),
        (424, b"Save Level"),
    ] {
        if localization.get(selector).map(Box::as_ref) != Some(expected) {
            return Err(IndexedOverlayError(format!(
                "localization {selector} does not match correction-2 source text"
            )));
        }
    }
    let prompts = vectors
        .pointer("/ok_cancel/fixed_prompts")
        .and_then(Value::as_array)
        .ok_or_else(|| IndexedOverlayError("OK/cancel prompt vector missing".to_owned()))?
        .iter()
        .map(Value::as_str)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| IndexedOverlayError("OK/cancel prompt vector is not text".to_owned()))?;
    if prompts != ["Abandon level.?", "Load Level?", "Save Level?"] {
        return Err(IndexedOverlayError(format!(
            "OK/cancel prompt vector is {prompts:?}"
        )));
    }
    for (pointer, expected) in [
        (
            "/pause_or_end/end_text",
            "Tasks completed. Fly to the exit point..",
        ),
        ("/debug/version", "Beta"),
        ("/debug/version_date", "03:42:13 Sep 06 1995"),
        ("/cursor/active_policy", "fail_closed"),
    ] {
        if vectors.pointer(pointer).and_then(Value::as_str) != Some(expected) {
            return Err(IndexedOverlayError(format!(
                "source-semantic string {pointer} does not match {expected:?}"
            )));
        }
    }
    for (pointer, expected) in [
        ("/pause/disabled_shade_passes", 2_i64),
        ("/pause/disabled_colour_offset", 0),
        ("/chat/channel_selected", 155),
        ("/chat/channel_unselected", 158),
        ("/chat/mode_selected", 156),
        ("/chat/mode_unselected", 159),
        ("/chat/remote_selected", 157),
        ("/chat/remote_unselected", 160),
        ("/options/left_x", 109),
        ("/options/row_y_start", 33),
        ("/options/row_y_step", 9),
        ("/options/middle_repeat", 15),
        ("/options/right_x", 202),
        ("/options/text_colour_offsets/hover", 3840),
        ("/options/text_colour_offsets/normal", 4095),
        ("/options/text_colour_offsets/unavailable", 0x888),
        ("/volume/background_colour_offset", 0),
        ("/volume/foreground_colour_offset", 240),
        ("/volume/max_volume", 127),
        ("/pause_or_end/paused_advance", 36),
        ("/pause_or_end/paused_id", 425),
        ("/pause_or_end/paused_colour_offset", 240),
        ("/pause_or_end/end_colour_offset", 3840),
        ("/map/scale", 408),
        ("/map/right_eye_z_add", 128),
        ("/map/divider_colour_offset", 0),
        ("/scores/large_selector", 85),
        ("/scores/mini_selector", 86),
        ("/scores/player_x_step", 18),
        ("/scores/row_y_step", 19),
        ("/bottom/family_count", 26),
        ("/bottom/edge_selector", 88),
        ("/bottom/panel_selected", 87),
        ("/bottom/panel_unavailable", 89),
        ("/bottom/panel_cannot", 91),
        ("/bottom/colourized_tint", 0xa6),
        ("/bottom/selected_frame", 90),
        ("/bottom/tier_y", 147),
        ("/bottom/tier_spacing", 33),
        ("/bottom/tier_number_start", 165),
        ("/bottom/tier_selected", 164),
        ("/bottom/xp_background_offset", 0),
        ("/bottom/xp_foreground_offset", 3840),
        ("/bottom/xp_source_width", 54),
        ("/debug/flag8_x", 160),
        ("/debug/line_step", 7),
        ("/debug/label_colour_offset", 3840),
        ("/debug/value_colour_offset", 15),
        ("/debug/selected_value_colour_offset", 255),
        ("/debug/tape_size", 38_547),
        ("/debug/carpet_size", 224_791),
        ("/cursor/selector", 9),
    ] {
        if vectors.pointer(pointer).and_then(Value::as_i64) != Some(expected) {
            return Err(IndexedOverlayError(format!(
                "source-semantic scalar {pointer} does not match {expected}"
            )));
        }
    }
    if vectors
        .pointer("/cursor/active_bank_proven")
        .and_then(Value::as_bool)
        != Some(false)
    {
        return Err(IndexedOverlayError(
            "cursor bank provenance must remain fail-closed".to_owned(),
        ));
    }
    for pointer in [
        "/scores/requires_all_player_matrix",
        "/scores/requires_help_rng_state",
    ] {
        if vectors.pointer(pointer).and_then(Value::as_bool) != Some(true) {
            return Err(IndexedOverlayError(format!(
                "source-semantic prerequisite {pointer} must stay fail-closed"
            )));
        }
    }
    for (state, format, colour_offset) in [
        ("1", "name_text", 3840_i64),
        ("2", "bracket_name_text", 3840),
        ("3", "text", 3840),
        ("4", "bracket_name_text", 240),
        ("5", "local_text", 3840),
    ] {
        let row = vectors
            .pointer(&format!("/pause_or_end/notification_states/{state}"))
            .and_then(Value::as_array)
            .ok_or_else(|| {
                IndexedOverlayError(format!("notification semantic state {state} is missing"))
            })?;
        if row.first().and_then(Value::as_str) != Some(format)
            || row.get(1).and_then(Value::as_i64) != Some(colour_offset)
        {
            return Err(IndexedOverlayError(format!(
                "notification semantic state {state} changed"
            )));
        }
    }
    Ok(())
}

fn semantic_integers(vectors: &Value, pointer: &str) -> Result<Vec<i64>, IndexedOverlayError> {
    vectors
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| {
            IndexedOverlayError(format!(
                "source-semantic production vector {pointer} is not an array"
            ))
        })?
        .iter()
        .map(|value| {
            value.as_i64().ok_or_else(|| {
                IndexedOverlayError(format!(
                    "source-semantic production vector {pointer} contains a non-integer"
                ))
            })
        })
        .collect()
}

fn semantic_integer_rows(
    vectors: &Value,
    pointer: &str,
) -> Result<Vec<Vec<i64>>, IndexedOverlayError> {
    vectors
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| {
            IndexedOverlayError(format!(
                "source-semantic production rows {pointer} are not an array"
            ))
        })?
        .iter()
        .enumerate()
        .map(|(ordinal, row)| {
            row.as_array()
                .ok_or_else(|| {
                    IndexedOverlayError(format!(
                        "source-semantic production row {pointer}/{ordinal} is not an array"
                    ))
                })?
                .iter()
                .map(|value| {
                    value.as_i64().ok_or_else(|| {
                        IndexedOverlayError(format!(
                            "source-semantic production row {pointer}/{ordinal} contains a non-integer"
                        ))
                    })
                })
                .collect()
        })
        .collect()
}

fn validate_plane(name: &str, plane: &[u8]) -> Result<(), IndexedOverlayError> {
    if plane.len() != LEVEL1_VIEW_PIXELS {
        return Err(IndexedOverlayError(format!(
            "{name} has {} bytes, expected {LEVEL1_VIEW_PIXELS}",
            plane.len()
        )));
    }
    Ok(())
}

fn byte_hash(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    bytes.iter().fold(OFFSET, |mut hash, byte| {
        hash ^= u64::from(*byte);
        hash.wrapping_mul(PRIME)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache_root() -> std::path::PathBuf {
        let private = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let root = crate::native_world::resolve_level1_presentation_cache_root(&private);
        crate::native_world::NativeWorld::load_with_owner_cache(
            private.join("generated/level1-export-m"),
            private.join("generated/level1-asset-closure.json"),
            &root,
        )
        .expect("manifest/hash-qualified interim presentation cache");
        root
    }

    #[test]
    fn installed_locale_rebind_draws_real_source_text_and_all_six_objectives() {
        let cache = std::env::var("MC_LANGUAGE_CACHE")
            .expect("set MC_LANGUAGE_CACHE to an actual built installed-language cache");
        let language =
            crate::game_runtime::preferences::installed_locale::InstalledLanguages::load(cache)
                .unwrap();
        let locale = language.binding(None).unwrap();
        let source = IndexedOverlayAssets::load(cache_root()).unwrap();
        let mut selected = source.clone();
        selected.bind_locale(&locale).unwrap();
        assert_eq!(selected.locale_binding(), Some(&locale.identity));
        // Owner L1/D2 are identical. Compare actual rendered pixels against
        // the independently qualified default; do not invent another language.
        for selector in 48..=53 {
            let mut expected = vec![0; LEVEL1_VIEW_PIXELS];
            let mut actual = expected.clone();
            let mut expected_mask = expected.clone();
            let mut actual_mask = expected.clone();
            assert!(
                source
                    .draw_objective(selector, &mut expected, &mut expected_mask)
                    .unwrap()
                    > 0
            );
            assert!(
                selected
                    .draw_objective(selector, &mut actual, &mut actual_mask)
                    .unwrap()
                    > 0
            );
            assert_eq!(actual, expected, "selected objective {selector} pixels");
            assert_eq!(actual_mask, expected_mask);
        }
        let mut pixels = vec![0; LEVEL1_VIEW_PIXELS];
        let mut provenance = pixels.clone();
        assert!(
            selected
                .draw_text(
                    &mut pixels,
                    &mut provenance,
                    3,
                    4,
                    selected.localization(23).unwrap(),
                    19
                )
                .unwrap()
                > 0
        );
        assert!(provenance.iter().any(|&value| value != 0));
    }

    #[test]
    fn text_uses_source_character_plus_one_and_exact_glyph_advance() {
        let assets = IndexedOverlayAssets::load(cache_root()).unwrap();
        let mut pixels = vec![0; 320 * 200];
        let mut provenance = vec![0; pixels.len()];
        assets
            .draw_text(&mut pixels, &mut provenance, 0, 0, b"AA\0B", 19)
            .unwrap();
        let glyph = assets.glyphs[usize::from(b'A') + 1];
        let width = usize::from(glyph.width);
        let mut expected = vec![0; pixels.len()];
        for origin in [0, width] {
            for row in 0..usize::from(glyph.height) {
                for col in 0..width {
                    if assets.font_masks[glyph.offset as usize + row * width + col] != 0 {
                        expected[row * 320 + origin + col] = 19;
                    }
                }
            }
        }
        assert!(expected.iter().any(|pixel| *pixel == 19));
        assert_eq!(pixels, expected);
        let previous = assets.glyphs[usize::from(b'A')];
        assert_ne!(
            &assets.font_masks
                [glyph.offset as usize..glyph.offset as usize + usize::from(glyph.length)],
            &assets.font_masks
                [previous.offset as usize..previous.offset as usize + usize::from(previous.length)],
            "adjacent glyphs discriminate the incorrect direct-ASCII lookup"
        );
    }

    #[test]
    fn presentation_census_and_hud_are_independently_manifest_hash_bound() {
        let source = cache_root();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("test clock follows epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "mc-plan29-overlay-hash-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("frame")).expect("temporary frame directory");
        std::fs::create_dir_all(root.join("hud")).expect("temporary HUD directory");
        std::fs::copy(source.join("manifest.json"), root.join("manifest.json"))
            .expect("copy qualified manifest");
        for relative in ["frame/source-frame-census.json", "hud/level1.json"] {
            std::fs::copy(source.join(relative), root.join(relative))
                .expect("copy manifest-declared presentation output");
        }

        let cache = QualifiedOwnerCacheReader::open(&root)
            .expect("qualified identity does not depend on undeclared directory contents");
        for relative in ["frame/source-frame-census.json", "hud/level1.json"] {
            let original = cache
                .read(relative)
                .expect("unaltered presentation output matches manifest");
            let mut corrupted = original.clone();
            corrupted[0] ^= 1;
            std::fs::write(root.join(relative), corrupted)
                .expect("write isolated corrupt presentation output");
            let error = cache
                .read(relative)
                .expect_err("content drift must fail before JSON parsing");
            assert!(
                error.to_string().contains("hash/size mismatch"),
                "{relative}: {error}"
            );
            std::fs::write(root.join(relative), original)
                .expect("restore isolated presentation output");
        }
        std::fs::remove_dir_all(&root).expect("remove isolated presentation-cache fixture");
    }

    #[test]
    fn reachable_fixed_overlay_tree_is_ordered_and_fail_closed() {
        let assets =
            IndexedOverlayAssets::load(cache_root()).expect("qualified interim presentation cache");
        for pointer in [
            "/palette_indirection/indexed_values/0",
            "/pause/selector_order/0",
            "/pause/fixed_xy/0/0",
            "/pause/disabled_shade_passes",
            "/chat/entry_text_xy/0",
            "/chat/channel_selected",
            "/chat/mode_xy/0/0",
            "/chat/button_order/0/0",
            "/options/on_ids/0",
            "/options/off_ids/0",
            "/options/left_x",
            "/options/middle_repeat",
            "/options/ok/0/0",
            "/options/text_colour_offsets/hover",
            "/volume/source_bar/0",
            "/volume/foreground_colour_offset",
            "/volume/max_volume",
            "/ok_cancel/localized_ids/0",
            "/ok_cancel/button_order/0/0",
            "/pause_or_end/paused_id",
            "/pause_or_end/end_ids/0",
            "/pause_or_end/timer_edge/0",
            "/pause_or_end/notification_states/1/1",
            "/map/left_size/0",
            "/map/scale",
            "/map/right_viewport/0",
            "/map/right_eye_z_add",
            "/map/divider_order/0",
            "/scores/large_selector",
            "/scores/player_x_step",
            "/bottom/slot_origin/0",
            "/bottom/panel_selected",
            "/bottom/colourized_tint",
            "/bottom/tier_spacing",
            "/bottom/tier_panels/0",
            "/bottom/xp_foreground_offset",
            "/debug/flag8_x",
            "/debug/line_step",
            "/debug/value_colour_offset",
            "/debug/tape_size",
            "/cursor/selector",
            "/cursor/msp_dimensions/0",
        ] {
            let mut changed = assets.source_semantic_vectors.clone();
            let value = changed
                .pointer_mut(pointer)
                .expect("source-semantic mutation pointer");
            *value = Value::from(value.as_i64().expect("integer mutation axis") + 1);
            assert!(
                validate_source_semantic_vectors(&changed, &assets.clrn, &assets.localization,)
                    .is_err(),
                "mutation axis {pointer} must fail the independent source ledger"
            );
        }
        for pointer in [
            "/scores/requires_all_player_matrix",
            "/scores/requires_help_rng_state",
        ] {
            let mut changed = assets.source_semantic_vectors.clone();
            changed
                .pointer_mut(pointer)
                .expect("source-semantic boolean mutation")
                .clone_from(&Value::from(false));
            assert!(
                validate_source_semantic_vectors(&changed, &assets.clrn, &assets.localization,)
                    .is_err(),
                "boolean mutation axis {pointer} must fail the independent source ledger"
            );
        }
        for pointer in [
            "/ok_cancel/fixed_prompts/0",
            "/pause_or_end/end_text",
            "/debug/version",
            "/debug/version_date",
            "/cursor/active_policy",
        ] {
            let mut changed = assets.source_semantic_vectors.clone();
            changed
                .pointer_mut(pointer)
                .expect("source-semantic text mutation")
                .clone_from(&Value::from("changed"));
            assert!(
                validate_source_semantic_vectors(&changed, &assets.clrn, &assets.localization,)
                    .is_err(),
                "text mutation axis {pointer} must fail the independent source ledger"
            );
        }
        let mut cursor_changed = assets.source_semantic_vectors.clone();
        cursor_changed["cursor"]["active_bank_proven"] = Value::from(true);
        assert!(
            validate_source_semantic_vectors(&cursor_changed, &assets.clrn, &assets.localization,)
                .is_err(),
            "unproven cursor-bank promotion must fail closed"
        );
        assert_eq!(
            assets.spell_tier_vectors,
            BTreeMap::from([
                (
                    IndexedOverlayStage::LeftSpell,
                    SpellTierVector {
                        x_for_i: 282,
                        y: 1,
                        colour: 61,
                    },
                ),
                (
                    IndexedOverlayStage::RightSpell,
                    SpellTierVector {
                        x_for_i: 314,
                        y: 1,
                        colour: 61,
                    },
                ),
            ]),
            "pinned GameUI formula and qualified Night CLRN row"
        );
        for family in 0..26 {
            assert_eq!(
                assets
                    .spell_menu_profile(family)
                    .expect("all source families")
                    .family_header_tier_count,
                3,
                "SPELLS byte0/header tier count must be exported for family {family}"
            );
        }
        let terrain = (0..64 * 64)
            .map(|index| u8::try_from(index % 251).expect("bounded"))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let mask = vec![1; 64 * 64].into_boxed_slice();
        let state = Level1FinalMainState {
            commands: vec![
                IndexedOverlayCommand::Decision {
                    stage: IndexedOverlayStage::SpiderWeb,
                    state_hash: 0,
                },
                IndexedOverlayCommand::Pixels {
                    stage: IndexedOverlayStage::MinimapTerrain,
                    x: 0,
                    y: 0,
                    width: 64,
                    height: 64,
                    indices: terrain,
                    mask,
                },
                IndexedOverlayCommand::Decision {
                    stage: IndexedOverlayStage::MinimapEntities,
                    state_hash: 0,
                },
                IndexedOverlayCommand::Decision {
                    stage: IndexedOverlayStage::MinimapMarks,
                    state_hash: 0,
                },
                IndexedOverlayCommand::Bitmap {
                    stage: IndexedOverlayStage::LeftSpell,
                    selector: 1,
                    x: 255,
                    y: 1,
                },
                IndexedOverlayCommand::Text {
                    stage: IndexedOverlayStage::LeftSpell,
                    bytes: b"I".to_vec().into_boxed_slice(),
                    x: 282,
                    y: 1,
                    colour: 61,
                },
                IndexedOverlayCommand::Bitmap {
                    stage: IndexedOverlayStage::RightSpell,
                    selector: 1,
                    x: 287,
                    y: 1,
                },
                IndexedOverlayCommand::Text {
                    stage: IndexedOverlayStage::RightSpell,
                    bytes: b"I".to_vec().into_boxed_slice(),
                    x: 314,
                    y: 1,
                    colour: 61,
                },
                IndexedOverlayCommand::Bitmap {
                    stage: IndexedOverlayStage::TopStatus,
                    selector: 40,
                    x: 1,
                    y: 1,
                },
                IndexedOverlayCommand::Decision {
                    stage: IndexedOverlayStage::Menu,
                    state_hash: 0,
                },
                IndexedOverlayCommand::Decision {
                    stage: IndexedOverlayStage::PauseOrEndText,
                    state_hash: 0,
                },
                IndexedOverlayCommand::Decision {
                    stage: IndexedOverlayStage::ObjectiveTextbox,
                    state_hash: 48,
                },
                IndexedOverlayCommand::Decision {
                    stage: IndexedOverlayStage::TutorialOrBriefing,
                    state_hash: 0,
                },
                IndexedOverlayCommand::Decision {
                    stage: IndexedOverlayStage::HelpPopup,
                    state_hash: 0,
                },
                IndexedOverlayCommand::Decision {
                    stage: IndexedOverlayStage::DebugText,
                    state_hash: 0,
                },
            ],
            required_stages: vec![
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
            ],
            ..Level1FinalMainState::default()
        };
        let mut indices = vec![0; LEVEL1_VIEW_PIXELS];
        let mut provenance = vec![1; LEVEL1_VIEW_PIXELS];
        let output = assets
            .apply(&state, &mut indices, &mut provenance, None)
            .expect("closed real-cache command stream");
        assert_eq!(output.receipts.len(), 13);
        assert_eq!(output.command_hash, state.immutable_hash());
        assert!(output.pixels_written > 4_096);

        let mut missing = state.clone();
        missing.required_stages.swap(7, 8);
        assert!(
            assets
                .apply(
                    &missing,
                    &mut vec![0; LEVEL1_VIEW_PIXELS],
                    &mut vec![0; LEVEL1_VIEW_PIXELS],
                    None,
                )
                .expect_err("missing live producer fails closed")
                .0
                .contains("source ledger")
        );
        for (label, mutate) in [
            ("selector", (IndexedOverlayStage::TopStatus, 0_u8)),
            ("coordinate", (IndexedOverlayStage::LeftSpell, 1_u8)),
            ("colour", (IndexedOverlayStage::RightSpell, 2_u8)),
        ] {
            let mut changed = state.clone();
            match mutate {
                (IndexedOverlayStage::TopStatus, 0) => {
                    let command = changed
                        .commands
                        .iter_mut()
                        .find(|command| command.stage() == IndexedOverlayStage::TopStatus)
                        .expect("top-status command");
                    if let IndexedOverlayCommand::Bitmap { selector, .. } = command {
                        *selector = 41;
                    }
                }
                (IndexedOverlayStage::LeftSpell, 1) => {
                    let command = changed
                        .commands
                        .iter_mut()
                        .find(|command| {
                            matches!(
                                command,
                                IndexedOverlayCommand::Text {
                                    stage: IndexedOverlayStage::LeftSpell,
                                    ..
                                }
                            )
                        })
                        .expect("left tier command");
                    if let IndexedOverlayCommand::Text { x, .. } = command {
                        *x -= 1;
                    }
                }
                (IndexedOverlayStage::RightSpell, 2) => {
                    let command = changed
                        .commands
                        .iter_mut()
                        .find(|command| {
                            matches!(
                                command,
                                IndexedOverlayCommand::Text {
                                    stage: IndexedOverlayStage::RightSpell,
                                    ..
                                }
                            )
                        })
                        .expect("right tier command");
                    if let IndexedOverlayCommand::Text { colour, .. } = command {
                        *colour ^= 1;
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                assets
                    .apply(
                        &changed,
                        &mut vec![0; LEVEL1_VIEW_PIXELS],
                        &mut vec![0; LEVEL1_VIEW_PIXELS],
                        None,
                    )
                    .expect_err(label)
                    .0
                    .contains("source"),
                "{label} mutation must fail the independent source vector"
            );
        }
        let unsupported = Level1FinalMainState {
            profile: FixedOverlayProfile {
                stereo: true,
                ..FixedOverlayProfile::default()
            },
            ..state
        };
        assert!(
            assets
                .apply(
                    &unsupported,
                    &mut vec![0; LEVEL1_VIEW_PIXELS],
                    &mut vec![0; LEVEL1_VIEW_PIXELS],
                    None,
                )
                .expect_err("unsupported profile")
                .0
                .contains("incomplete")
        );
    }

    #[test]
    fn overlay_marks_only_actual_writes_and_help_recurses_from_prior_main() {
        let assets =
            IndexedOverlayAssets::load(cache_root()).expect("qualified interim presentation cache");
        let required_stages = assets
            .receipt_census
            .get("ordinary-0")
            .expect("ordinary source receipt row")
            .to_vec();
        let mut commands = required_stages
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
            .collect::<Vec<_>>();
        let help = commands
            .iter()
            .position(|command| command.stage() == IndexedOverlayStage::HelpPopup)
            .expect("help receipt");
        commands[help] = IndexedOverlayCommand::LocalizedText {
            stage: IndexedOverlayStage::HelpPopup,
            selector: 425,
            x: 68,
            y: 96,
            colour: 255,
        };
        let state = Level1FinalMainState {
            commands,
            required_stages,
            prior_help_sentinel: 7,
            next_help_sentinel: 8,
            ..Level1FinalMainState::default()
        };
        let mut indices = vec![0; LEVEL1_VIEW_PIXELS];
        let mut provenance = vec![1; LEVEL1_VIEW_PIXELS];
        let output = assets
            .apply(&state, &mut indices, &mut provenance, None)
            .expect("source localization row");
        assert_eq!(output.help_sentinel, 8);
        assert!(provenance.iter().any(|value| *value == 1));
        assert!(
            provenance
                .iter()
                .any(|value| (*value & OVERLAY_PROVENANCE) != 0)
        );
        let mut changed = state;
        changed.commands[help] = IndexedOverlayCommand::LocalizedText {
            stage: IndexedOverlayStage::HelpPopup,
            selector: 378,
            x: 68,
            y: 96,
            colour: 255,
        };
        assert_ne!(changed.immutable_hash(), output.command_hash);
    }
}
