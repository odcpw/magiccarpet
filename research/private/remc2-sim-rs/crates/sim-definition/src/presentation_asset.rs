//! Typed, bounded presentation assets carried by portable game packages.

use super::{
    ArtifactKind, DefinitionError, DefinitionErrorCode, GameDefinition, Result, SemanticId,
    SemanticValue,
};
use serde::{Deserialize, Serialize};

pub const INDEXED_SPRITE_SCHEMA: &str = "carpet-flight-indexed-sprite/v1";
const INDEXED_SPRITE_KIND: &str = "indexed_sprite";
const INDEXED_SPRITE_FORMAT: &str = "indexed8-rgba8/v1";
const COLOR_CONTRACT: &str = "srgb-straight-alpha-rgba8";
const COORDINATES: &str = "x_right_y_down";
const ORIGIN: &str = "center";
const ENGINE_MAX_ASSET_BYTES: u32 = 1024 * 1024;
const ENGINE_MAX_DIMENSION: u16 = 4096;
const ENGINE_MAX_FRAMES: u16 = 64;
const ENGINE_MAX_PALETTE_COLORS: u16 = 256;
const ENGINE_MAX_PIXELS: u32 = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationAssetBudgets {
    pub max_bytes: u32,
    pub max_frames: u16,
    pub max_palette_colors: u16,
    pub max_pixels: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedPresentationAsset {
    pub schema: String,
    pub kind: String,
    pub runtime_format: String,
    pub width: u16,
    pub height: u16,
    pub frame_count: u16,
    pub palette_rgba8: Vec<u8>,
    pub color_contract: String,
    pub frame_duration_ms: u32,
    pub origin: String,
    pub coordinates: String,
    pub dependencies: Vec<String>,
    pub budgets: PresentationAssetBudgets,
    pub indices: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BoundPresentationAsset {
    pub slot_id: String,
    pub content_address: String,
    #[serde(flatten)]
    pub asset: IndexedPresentationAsset,
}

/// One retained, typed artifact that may replace the current binding for a
/// presentation slot without importing new bytes into the definition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompatiblePresentationArtifact {
    pub artifact_id: String,
    pub content_address: String,
    pub kind: String,
    pub bytes: u64,
}

impl IndexedPresentationAsset {
    fn parse_and_validate(
        bytes: &[u8],
        artifact_dependencies: &[String],
        path: &str,
    ) -> Result<Self> {
        let asset: Self = serde_json::from_slice(bytes).map_err(|error| {
            DefinitionError::new(
                DefinitionErrorCode::InvalidSchema,
                path,
                format!("invalid indexed presentation asset JSON: {error}"),
            )
        })?;
        if asset.schema != INDEXED_SPRITE_SCHEMA
            || asset.kind != INDEXED_SPRITE_KIND
            || asset.runtime_format != INDEXED_SPRITE_FORMAT
            || asset.color_contract != COLOR_CONTRACT
            || asset.coordinates != COORDINATES
            || asset.origin != ORIGIN
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::TypeMismatch,
                path,
                "presentation asset kind, format, color, coordinate, or origin contract is unsupported",
            ));
        }
        if asset.width == 0
            || asset.height == 0
            || asset.width > ENGINE_MAX_DIMENSION
            || asset.height > ENGINE_MAX_DIMENSION
            || asset.frame_count == 0
            || asset.frame_count > asset.budgets.max_frames
            || asset.palette_rgba8.is_empty()
            || asset.palette_rgba8.len() % 4 != 0
            || asset.palette_rgba8.len() / 4 > usize::from(asset.budgets.max_palette_colors)
            || bytes.len() > asset.budgets.max_bytes as usize
            || asset.budgets.max_bytes == 0
            || asset.budgets.max_bytes > ENGINE_MAX_ASSET_BYTES
            || asset.budgets.max_frames == 0
            || asset.budgets.max_frames > ENGINE_MAX_FRAMES
            || asset.budgets.max_palette_colors == 0
            || asset.budgets.max_palette_colors > ENGINE_MAX_PALETTE_COLORS
            || asset.budgets.max_pixels == 0
            || asset.budgets.max_pixels > ENGINE_MAX_PIXELS
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::OutOfRange,
                path,
                "presentation dimensions, frames, palette, or encoded bytes exceed declared budgets",
            ));
        }
        let pixels_per_frame = u32::from(asset.width)
            .checked_mul(u32::from(asset.height))
            .ok_or_else(|| {
                DefinitionError::new(
                    DefinitionErrorCode::OutOfRange,
                    path,
                    "presentation dimensions overflow the pixel carrier",
                )
            })?;
        let total_pixels = pixels_per_frame
            .checked_mul(u32::from(asset.frame_count))
            .ok_or_else(|| {
                DefinitionError::new(
                    DefinitionErrorCode::OutOfRange,
                    path,
                    "presentation frame count overflows the pixel carrier",
                )
            })?;
        if total_pixels > asset.budgets.max_pixels
            || asset.indices.len() != total_pixels as usize
            || asset
                .indices
                .iter()
                .any(|index| usize::from(*index) >= asset.palette_rgba8.len() / 4)
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::OutOfRange,
                path,
                "presentation pixel payload disagrees with dimensions, palette, or pixel budget",
            ));
        }
        if asset.frame_count == 1 && asset.frame_duration_ms != 0
            || asset.frame_count > 1 && asset.frame_duration_ms == 0
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::TypeMismatch,
                path,
                "presentation animation timing disagrees with frame count",
            ));
        }
        if asset.dependencies != artifact_dependencies {
            return Err(DefinitionError::new(
                DefinitionErrorCode::DependencyFailed,
                path,
                "presentation payload dependencies disagree with the artifact graph",
            ));
        }
        Ok(asset)
    }
}

pub(super) fn resolve(
    definition: &GameDefinition,
    id: &SemanticId,
) -> Result<BoundPresentationAsset> {
    let SemanticValue::Reference(reference) = definition.value(id).ok_or_else(|| {
        DefinitionError::new(
            DefinitionErrorCode::UnknownSemanticId,
            id.as_str(),
            "definition has no presentation binding",
        )
    })?
    else {
        return Err(DefinitionError::new(
            DefinitionErrorCode::TypeMismatch,
            id.as_str(),
            "presentation binding is not a content reference",
        ));
    };
    resolve_reference(definition, id, reference)
}

pub(super) fn resolve_reference(
    definition: &GameDefinition,
    id: &SemanticId,
    reference: &str,
) -> Result<BoundPresentationAsset> {
    let artifact = definition
        .artifacts()
        .iter()
        .find(|artifact| format!("sha256:{}", artifact.sha256) == reference)
        .ok_or_else(|| {
            DefinitionError::new(
                DefinitionErrorCode::HashMismatch,
                id.as_str(),
                "presentation binding does not name a retained artifact",
            )
        })?;
    if artifact.kind != ArtifactKind::Asset {
        return Err(DefinitionError::new(
            DefinitionErrorCode::TypeMismatch,
            id.as_str(),
            "presentation binding does not name an asset artifact",
        ));
    }
    let payload = definition.artifact_payload(&artifact.id).ok_or_else(|| {
        DefinitionError::new(
            DefinitionErrorCode::DependencyFailed,
            id.as_str(),
            "presentation asset payload is external to this portable definition",
        )
    })?;
    let asset = IndexedPresentationAsset::parse_and_validate(
        payload,
        &artifact.dependencies,
        &artifact.logical_path,
    )?;
    Ok(BoundPresentationAsset {
        slot_id: id.as_str().to_owned(),
        content_address: reference.to_owned(),
        asset,
    })
}

pub(super) fn compatible_artifacts(
    definition: &GameDefinition,
    id: &SemanticId,
) -> Result<Vec<CompatiblePresentationArtifact>> {
    let bound = resolve(definition, id)?;
    let mut result = Vec::new();
    for artifact in definition.artifacts() {
        if artifact.kind != ArtifactKind::Asset {
            continue;
        }
        let Some(payload) = definition.artifact_payload(&artifact.id) else {
            continue;
        };
        let Ok(asset) = IndexedPresentationAsset::parse_and_validate(
            payload,
            &artifact.dependencies,
            &artifact.logical_path,
        ) else {
            continue;
        };
        if asset.schema == bound.asset.schema
            && asset.kind == bound.asset.kind
            && asset.runtime_format == bound.asset.runtime_format
            && asset.width == bound.asset.width
            && asset.height == bound.asset.height
            && asset.frame_count == bound.asset.frame_count
            && asset.color_contract == bound.asset.color_contract
            && asset.frame_duration_ms == bound.asset.frame_duration_ms
            && asset.origin == bound.asset.origin
            && asset.coordinates == bound.asset.coordinates
        {
            result.push(CompatiblePresentationArtifact {
                artifact_id: artifact.id.clone(),
                content_address: format!("sha256:{}", artifact.sha256),
                kind: "asset".to_owned(),
                bytes: artifact.bytes,
            });
        }
    }
    Ok(result)
}
