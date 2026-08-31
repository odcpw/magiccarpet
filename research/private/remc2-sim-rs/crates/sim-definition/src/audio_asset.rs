//! Typed PCM WAVE assets retained by owner-qualified game definitions.

use super::{
    ArtifactKind, CompatiblePresentationArtifact, DefinitionError, DefinitionErrorCode,
    GameDefinition, Result, SemanticId, SemanticValue,
};
use serde::Serialize;

const MAX_AUDIO_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PcmWaveAsset {
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub frame_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BoundAudioAsset {
    pub slot_id: String,
    pub content_address: String,
    #[serde(flatten)]
    pub asset: PcmWaveAsset,
}

impl PcmWaveAsset {
    fn parse_and_validate(bytes: &[u8], path: &str) -> Result<Self> {
        if bytes.len() < 44
            || bytes.len() > MAX_AUDIO_BYTES
            || &bytes[..4] != b"RIFF"
            || &bytes[8..12] != b"WAVE"
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::TypeMismatch,
                path,
                "audio artifact is not a bounded RIFF/WAVE payload",
            ));
        }
        let declared = u32::from_le_bytes(bytes[4..8].try_into().expect("RIFF length slice"));
        let declared_end = usize::try_from(declared)
            .ok()
            .and_then(|value| value.checked_add(8));
        if !declared_end.is_some_and(|end| end <= bytes.len() && bytes.len() - end <= 4) {
            return Err(DefinitionError::new(
                DefinitionErrorCode::HashMismatch,
                path,
                "audio RIFF length disagrees with the bounded owner payload",
            ));
        }
        let mut cursor = 12_usize;
        let mut format = None;
        let mut data_bytes = None;
        while cursor.checked_add(8).is_some_and(|end| end <= bytes.len()) {
            let chunk = &bytes[cursor..cursor + 4];
            let length = usize::try_from(u32::from_le_bytes(
                bytes[cursor + 4..cursor + 8]
                    .try_into()
                    .expect("WAVE chunk length slice"),
            ))
            .map_err(|_| {
                DefinitionError::new(
                    DefinitionErrorCode::OutOfRange,
                    path,
                    "audio chunk length exceeds the platform carrier",
                )
            })?;
            let start = cursor + 8;
            let end = start
                .checked_add(length)
                .filter(|end| *end <= bytes.len())
                .ok_or_else(|| {
                    DefinitionError::new(
                        DefinitionErrorCode::HashMismatch,
                        path,
                        "audio chunk escapes the retained payload",
                    )
                })?;
            if chunk == b"fmt " {
                if length < 16 {
                    return Err(DefinitionError::new(
                        DefinitionErrorCode::TypeMismatch,
                        path,
                        "audio format chunk is truncated",
                    ));
                }
                format = Some((
                    u16::from_le_bytes(bytes[start..start + 2].try_into().expect("format tag")),
                    u16::from_le_bytes(bytes[start + 2..start + 4].try_into().expect("channels")),
                    u32::from_le_bytes(
                        bytes[start + 4..start + 8].try_into().expect("sample rate"),
                    ),
                    u16::from_le_bytes(
                        bytes[start + 12..start + 14]
                            .try_into()
                            .expect("block align"),
                    ),
                    u16::from_le_bytes(
                        bytes[start + 14..start + 16]
                            .try_into()
                            .expect("sample width"),
                    ),
                ));
            } else if chunk == b"data" {
                data_bytes = Some(length);
            }
            cursor = end + (length & 1);
        }
        let Some((format_tag, channels, sample_rate, block_align, bits_per_sample)) = format else {
            return Err(DefinitionError::new(
                DefinitionErrorCode::TypeMismatch,
                path,
                "audio payload has no format chunk",
            ));
        };
        let Some(data_bytes) = data_bytes else {
            return Err(DefinitionError::new(
                DefinitionErrorCode::TypeMismatch,
                path,
                "audio payload has no sample-data chunk",
            ));
        };
        if format_tag != 1
            || channels != 1
            || sample_rate != 22_050
            || bits_per_sample != 8
            || block_align != 1
            || data_bytes == 0
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::TypeMismatch,
                path,
                "Fireball launch audio must be non-empty mono 22050 Hz 8-bit PCM",
            ));
        }
        Ok(Self {
            channels,
            sample_rate,
            bits_per_sample,
            frame_count: u32::try_from(data_bytes).map_err(|_| {
                DefinitionError::new(
                    DefinitionErrorCode::OutOfRange,
                    path,
                    "audio frame count exceeds u32",
                )
            })?,
        })
    }
}

pub(super) fn resolve(definition: &GameDefinition, id: &SemanticId) -> Result<BoundAudioAsset> {
    let SemanticValue::Reference(reference) = definition.value(id).ok_or_else(|| {
        DefinitionError::new(
            DefinitionErrorCode::UnknownSemanticId,
            id.as_str(),
            "definition has no audio binding",
        )
    })?
    else {
        return Err(DefinitionError::new(
            DefinitionErrorCode::TypeMismatch,
            id.as_str(),
            "audio binding is not a content reference",
        ));
    };
    resolve_reference(definition, id, reference)
}

pub(super) fn resolve_reference(
    definition: &GameDefinition,
    id: &SemanticId,
    reference: &str,
) -> Result<BoundAudioAsset> {
    let artifact = definition
        .artifacts()
        .iter()
        .find(|artifact| format!("sha256:{}", artifact.sha256) == reference)
        .ok_or_else(|| {
            DefinitionError::new(
                DefinitionErrorCode::HashMismatch,
                id.as_str(),
                "audio binding does not name a retained artifact",
            )
        })?;
    if artifact.kind != ArtifactKind::Asset || !artifact.dependencies.is_empty() {
        return Err(DefinitionError::new(
            DefinitionErrorCode::TypeMismatch,
            id.as_str(),
            "audio binding does not name an independent asset artifact",
        ));
    }
    let payload = definition.artifact_payload(&artifact.id).ok_or_else(|| {
        DefinitionError::new(
            DefinitionErrorCode::DependencyFailed,
            id.as_str(),
            "audio asset payload is external to this definition",
        )
    })?;
    Ok(BoundAudioAsset {
        slot_id: id.as_str().to_owned(),
        content_address: reference.to_owned(),
        asset: PcmWaveAsset::parse_and_validate(payload, &artifact.logical_path)?,
    })
}

pub(super) fn compatible_artifacts(
    definition: &GameDefinition,
    id: &SemanticId,
) -> Result<Vec<CompatiblePresentationArtifact>> {
    let bound = resolve(definition, id)?;
    let mut result = Vec::new();
    for artifact in definition.artifacts() {
        if artifact.kind != ArtifactKind::Asset || !artifact.dependencies.is_empty() {
            continue;
        }
        let Some(payload) = definition.artifact_payload(&artifact.id) else {
            continue;
        };
        let Ok(candidate) = PcmWaveAsset::parse_and_validate(payload, &artifact.logical_path)
        else {
            continue;
        };
        if candidate.channels == bound.asset.channels
            && candidate.sample_rate == bound.asset.sample_rate
            && candidate.bits_per_sample == bound.asset.bits_per_sample
        {
            result.push(CompatiblePresentationArtifact {
                artifact_id: artifact.id.clone(),
                content_address: format!("sha256:{}", artifact.sha256),
                kind: "audio_asset".to_owned(),
                bytes: artifact.bytes,
            });
        }
    }
    Ok(result)
}
