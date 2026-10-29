//! Source-selected campaign sound slots, derived from the retained owner bank.

use super::validation::read_qualified_cache_file;
use super::{AbiError, AbiResult, MC_STATUS_VALIDATION_FAILED};
use sha2::{Digest, Sha256};
use sim_core::campaign::spec::CampaignSelection;
use sim_core::presentation::environment_materials::EnvironmentResources;
use std::path::Path;

const SOUND_CONTAINER: &str = "audio/SOUND.DAT";
const SOUND_DESCRIPTOR: &str = "audio/descriptor.json";
const LEGACY_NIGHT_DESCRIPTOR: &str = "audio/sound-bank-1-822.json";
// This is the existing qualified Night manifest pin in native_world/night_cache.rs.
const LEGACY_NIGHT_MANIFEST_SHA256: &str =
    "3909045b6cf589c0278780e10096b4b630110027fd500e298fd93f2f8cec3317";
const MAX_SOUND_ID: usize = 69;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CampaignSoundBank {
    selection: CampaignSelection,
    null_slots: [bool; MAX_SOUND_ID + 1],
    slot_count: usize,
}

fn invalid(message: impl Into<String>) -> AbiError {
    AbiError::new(MC_STATUS_VALIDATION_FAILED, message)
}

fn read_u16(bytes: &[u8], offset: usize) -> AbiResult<u16> {
    let value: [u8; 2] = slice(bytes, offset, 2)?
        .try_into()
        .map_err(|_| invalid("sound bank u16 is truncated"))?;
    Ok(u16::from_le_bytes(value))
}

fn read_nonnegative(bytes: &[u8], offset: usize) -> AbiResult<usize> {
    let value: [u8; 4] = slice(bytes, offset, 4)?
        .try_into()
        .map_err(|_| invalid("sound bank i32 is truncated"))?;
    usize::try_from(i32::from_le_bytes(value))
        .map_err(|_| invalid("sound bank offset or extent is negative"))
}

fn slice(bytes: &[u8], offset: usize, length: usize) -> AbiResult<&[u8]> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| invalid("sound bank extent overflow"))?;
    bytes
        .get(offset..end)
        .ok_or_else(|| invalid("sound bank extent is truncated"))
}

impl CampaignSoundBank {
    /// A cache that does not advertise gameplay audio is valid for headless
    /// sessions. Once it advertises either descriptor or owner container,
    /// missing or invalid files must fail rather than silently disable audio.
    pub(super) fn load_if_present(
        root: &Path,
        selection: CampaignSelection,
        owner_package_sha256: &str,
        campaign_package_sha256: &str,
    ) -> AbiResult<Option<Self>> {
        let mut candidates = vec![root.to_path_buf(), root.join("components/audio")];
        if root.file_name().is_some_and(|name| name == "material")
            && let Some(parent) = root.parent()
            && parent.file_name().is_some_and(|name| name == "components")
        {
            candidates.push(parent.join("audio"));
        }
        for candidate in candidates {
            if std::fs::symlink_metadata(candidate.join(SOUND_DESCRIPTOR)).is_ok()
                || std::fs::symlink_metadata(candidate.join(SOUND_CONTAINER)).is_ok()
                || std::fs::symlink_metadata(candidate.join(LEGACY_NIGHT_DESCRIPTOR)).is_ok()
            {
                return Self::load(
                    root,
                    selection,
                    owner_package_sha256,
                    campaign_package_sha256,
                )
                .map(Some);
            }
            let manifest_bytes = match std::fs::read(candidate.join("manifest.json")) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(invalid(format!("cannot read sound manifest: {error}"))),
            };
            let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes)
                .map_err(|error| invalid(format!("invalid sound manifest: {error}")))?;
            if manifest["outputs"].as_array().is_some_and(|outputs| {
                outputs.iter().any(|row| {
                    row["path"].as_str().is_some_and(|path| {
                        matches!(
                            path,
                            SOUND_DESCRIPTOR | SOUND_CONTAINER | LEGACY_NIGHT_DESCRIPTOR
                        ) || matches!(
                            path,
                            "components/audio/audio/descriptor.json"
                                | "components/audio/audio/SOUND.DAT"
                        )
                    })
                })
            }) {
                return Self::load(
                    root,
                    selection,
                    owner_package_sha256,
                    campaign_package_sha256,
                )
                .map(Some);
            }
        }
        Ok(None)
    }

    /// Validate package identity and derive null slots from the actual retained
    /// SOUND.DAT metadata. A malformed or unsupported bank is an error, never
    /// a guess at a particular environment's null IDs.
    pub(super) fn load(
        root: &Path,
        selection: CampaignSelection,
        owner_package_sha256: &str,
        campaign_package_sha256: &str,
    ) -> AbiResult<Self> {
        if root.join(LEGACY_NIGHT_DESCRIPTOR).exists() && !root.join(SOUND_DESCRIPTOR).exists() {
            return Self::load_legacy_night(
                root,
                selection,
                owner_package_sha256,
                campaign_package_sha256,
            );
        }
        // The legacy Day aggregate stores its qualified audio component as a
        // sibling of its material component; both paths are existing inputs.
        let component = root.join("components/audio");
        let sibling = root
            .parent()
            .filter(|parent| {
                root.file_name().is_some_and(|name| name == "material")
                    && parent.file_name().is_some_and(|name| name == "components")
            })
            .map(|parent| parent.join("audio"));
        let audio_root = if root.join(SOUND_DESCRIPTOR).is_file() {
            root.to_path_buf()
        } else if component.join(SOUND_DESCRIPTOR).is_file() {
            component
        } else if let Some(sibling) = sibling {
            sibling
        } else {
            return Err(invalid("selected sound descriptor is missing"));
        };
        let root = audio_root.as_path();
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("manifest.json"))
                .map_err(|error| invalid(format!("cannot read sound manifest: {error}")))?,
        )
        .map_err(|error| invalid(format!("invalid sound manifest: {error}")))?;
        let environment = manifest["schema"] == "mc2-environment-material-cache/v1";
        if environment {
            let resources = EnvironmentResources::load(root)
                .map_err(|error| invalid(format!("invalid sound Environment cache: {error}")))?;
            let profile = resources
                .profile
                .as_ref()
                .ok_or_else(|| invalid("sound profile missing"))?;
            if profile.external_level != selection.external_level
                || profile.internal_level != selection.internal_level
                || profile.map_type.id != selection.map_type_id
                || profile.owner_export_package_sha256 != owner_package_sha256
                || profile.campaign_package_sha256 != campaign_package_sha256
            {
                return Err(invalid(
                    "sound bank profile does not match the active package",
                ));
            }
        } else if manifest["schema"] == "mc2-day-audio-cache/v1" {
            // The legacy descriptor and XMI are already pinned and qualified
            // against both package identities by the existing music loader.
            if super::campaign_audio::load_campaign_music(
                Some(root),
                selection,
                owner_package_sha256,
                campaign_package_sha256,
            )?
            .is_none()
            {
                return Err(invalid("legacy Day audio is not qualified"));
            }
        } else {
            return Err(invalid("unsupported sound cache schema"));
        }
        let rows = manifest["outputs"]
            .as_array()
            .ok_or_else(|| invalid("sound outputs missing"))?;
        let output = |path: &str| -> AbiResult<&serde_json::Value> {
            let mut matches = rows.iter().filter(|row| row["path"].as_str() == Some(path));
            let row = matches
                .next()
                .ok_or_else(|| invalid(format!("sound output missing: {path}")))?;
            if matches.next().is_some() {
                return Err(invalid(format!("duplicate sound output: {path}")));
            }
            Ok(row)
        };
        let read_output = |path: &str| -> AbiResult<Vec<u8>> {
            let row = output(path)?;
            let len = row["bytes"]
                .as_u64()
                .and_then(|v| usize::try_from(v).ok())
                .ok_or_else(|| invalid("sound output extent missing"))?;
            let sha = row["sha256"]
                .as_str()
                .ok_or_else(|| invalid("sound output hash missing"))?;
            read_qualified_cache_file(root, path, len, sha)
        };
        let container_row = output(SOUND_CONTAINER)?;
        let container_hash = container_row["sha256"]
            .as_str()
            .ok_or_else(|| invalid("sound container hash missing"))?;
        if container_row["source_asset_ids"] != serde_json::json!(["owner_data:SOUND/SOUND.DAT"]) {
            return Err(invalid(
                "sound container differs from retained owner identity",
            ));
        }
        if environment {
            let source = manifest["source_files"]
                .as_array()
                .and_then(|sources| {
                    sources
                        .iter()
                        .find(|row| row["asset_id"] == "owner_data:SOUND/SOUND.DAT")
                })
                .ok_or_else(|| invalid("sound owner source missing"))?;
            if source["path"] != "SOUND/SOUND.DAT"
                || source["content_address"] != format!("sha256:{container_hash}")
                || source["size"] != container_row["bytes"]
            {
                return Err(invalid(
                    "sound container differs from retained owner identity",
                ));
            }
        }
        let container = read_output(SOUND_CONTAINER)?;
        let descriptor: serde_json::Value = serde_json::from_slice(&read_output(SOUND_DESCRIPTOR)?)
            .map_err(|error| invalid(format!("invalid sound descriptor: {error}")))?;
        let expected_schema = match selection.map_type_id {
            0 => ["mc2-day-audio/v1", "mc2-day-audio/v2"],
            1 => ["mc2-night-audio/v1", "mc2-night-audio/v2"],
            2 => ["mc2-cave-audio/v1", "mc2-cave-audio/v1"],
            _ => return Err(invalid("unsupported sound bank selection")),
        };
        if !descriptor["schema"]
            .as_str()
            .is_some_and(|schema| expected_schema.contains(&schema))
            || descriptor["profile"] != manifest["profile"]
            || descriptor["sound"]["bank"].as_u64() != Some(u64::from(selection.map_type_id))
        {
            return Err(invalid(
                "sound descriptor differs from selected environment",
            ));
        }
        if !environment
            && (descriptor["sound"]["container_path"] != SOUND_CONTAINER
                || descriptor["sound"]["container_sha256"].as_str() != Some(container_hash))
        {
            return Err(invalid(
                "legacy sound container differs from qualified descriptor",
            ));
        }
        let trailer = read_nonnegative(
            &container,
            container
                .len()
                .checked_sub(4)
                .ok_or_else(|| invalid("sound container is truncated"))?,
        )?;
        let quality = 3_usize;
        let count = usize::from(read_u16(&container, trailer + quality * 2)?);
        if usize::from(selection.map_type_id) >= count {
            return Err(invalid("selected source sound bank is absent"));
        }
        let entry = trailer
            .checked_add(12)
            .and_then(|v| v.checked_add(usize::from(selection.map_type_id) * 96))
            .and_then(|v| v.checked_add(quality * 16))
            .ok_or_else(|| invalid("sound bank descriptor offset overflow"))?;
        let metadata_offset = read_nonnegative(&container, entry)?;
        let data_offset = read_nonnegative(&container, entry + 4)?;
        let metadata_size = read_nonnegative(&container, entry + 8)?;
        let data_size = read_nonnegative(&container, entry + 12)?;
        let metadata = slice(&container, metadata_offset, metadata_size)?;
        let data = slice(&container, data_offset, data_size)?;
        if metadata.starts_with(b"RNC") || data.starts_with(b"RNC") {
            return Err(invalid(
                "compressed sound metadata or payload is not admitted by this reader",
            ));
        }
        if metadata.len() < 32 || (metadata.len() - 32) % 32 != 0 {
            return Err(invalid("sound metadata extent is invalid"));
        }
        let slot_count = (metadata.len() - 32) / 32;
        if slot_count == 0 || slot_count > MAX_SOUND_ID {
            return Err(invalid("sound metadata slot count is invalid"));
        }
        let mut null_slots = [false; MAX_SOUND_ID + 1];
        let mut source_rows = Vec::with_capacity(slot_count);
        for index in 0..slot_count {
            let record = slice(metadata, 32 + index * 32, 32)?;
            let end = record[..18]
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(18);
            let name = std::str::from_utf8(&record[..end])
                .map_err(|_| invalid("sound filename is not ASCII"))?;
            if name.is_empty() || !name.is_ascii() || record[22..26] != [0; 4] {
                return Err(invalid("sound source record is malformed"));
            }
            let start = read_nonnegative(record, 18)?;
            let length = read_nonnegative(record, 26)?;
            let bytes = slice(data, start, length)?;
            null_slots[index + 1] = name.eq_ignore_ascii_case("null.wav");
            source_rows.push((
                name.to_owned(),
                format!("{:x}", Sha256::digest(bytes)),
                length,
            ));
        }
        let records = descriptor["sound"]["records"]
            .as_array()
            .ok_or_else(|| invalid("sound records missing"))?;
        let mut seen = [false; MAX_SOUND_ID + 1];
        for record in records {
            let id = record["sound_id"]
                .as_u64()
                .and_then(|v| usize::try_from(v).ok())
                .filter(|id| *id > 0 && *id <= slot_count)
                .ok_or_else(|| invalid("sound record ID is outside the source bank"))?;
            if seen[id] {
                return Err(invalid("duplicate sound record ID"));
            }
            seen[id] = true;
            let (name, sha, len) = &source_rows[id - 1];
            if record["filename"].as_str() != Some(name.as_str())
                || record
                    .get("metadata_slot")
                    .is_some_and(|value| value.as_u64() != Some((id - 1) as u64))
                || record
                    .get("record_bytes")
                    .is_some_and(|value| value.as_u64() != Some(*len as u64))
                || record
                    .get("record_sha256")
                    .is_some_and(|value| value.as_str() != Some(sha.as_str()))
            {
                return Err(invalid(format!(
                    "sound record {id} differs from retained source metadata"
                )));
            }
            if let Some(path) = record["asset_path"].as_str() {
                let expected_path = format!("audio/sfx/{id:03}.wav");
                let row = output(&expected_path)?;
                if path != expected_path
                    || record["asset_sha256"].as_str() != Some(sha.as_str())
                    || row["sha256"].as_str() != Some(sha.as_str())
                    || row["bytes"].as_u64() != Some(*len as u64)
                    || row["source_asset_ids"] != serde_json::json!(["owner_data:SOUND/SOUND.DAT"])
                {
                    return Err(invalid(format!(
                        "sound record {id} differs from retained source sample"
                    )));
                }
            } else if !null_slots[id] {
                return Err(invalid(format!("non-null sound record {id} has no sample")));
            }
        }
        Ok(Self {
            selection,
            null_slots,
            slot_count,
        })
    }

    pub(super) fn selection(&self) -> CampaignSelection {
        self.selection
    }

    fn load_legacy_night(
        root: &Path,
        selection: CampaignSelection,
        owner_package_sha256: &str,
        campaign_package_sha256: &str,
    ) -> AbiResult<Self> {
        if selection != CampaignSelection::LEVEL1 {
            return Err(invalid("legacy Night sound bank has the wrong selection"));
        }
        let manifest_size = std::fs::metadata(root.join("manifest.json"))
            .map_err(|error| invalid(format!("cannot inspect Night manifest: {error}")))?
            .len();
        if manifest_size > 8 * 1024 * 1024 {
            return Err(invalid("Night sound manifest exceeds its bound"));
        }
        let manifest_bytes = read_qualified_cache_file(
            root,
            "manifest.json",
            usize::try_from(manifest_size).map_err(|_| invalid("Night manifest is too large"))?,
            LEGACY_NIGHT_MANIFEST_SHA256,
        )?;
        let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| invalid(format!("invalid Night manifest: {error}")))?;
        if manifest["schema"] != "mc2-owner-unity-asset-cache/v2"
            || manifest["bindings"]["level_export_package_sha256"].as_str()
                != Some(owner_package_sha256)
            || manifest["bindings"]["campaign_package_sha256"].as_str()
                != Some(campaign_package_sha256)
        {
            return Err(invalid(
                "legacy Night audio does not bind the active package",
            ));
        }
        let rows = manifest["outputs"]
            .as_array()
            .ok_or_else(|| invalid("Night outputs missing"))?;
        let mut descriptor_rows = rows
            .iter()
            .filter(|row| row["path"] == LEGACY_NIGHT_DESCRIPTOR);
        let row = descriptor_rows
            .next()
            .ok_or_else(|| invalid("Night sound metadata missing"))?;
        if descriptor_rows.next().is_some()
            || row["source_asset_ids"] != serde_json::json!(["owner_data:SOUND/SOUND.DAT"])
        {
            return Err(invalid("Night sound metadata source differs"));
        }
        let len = row["bytes"]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| invalid("Night sound metadata extent missing"))?;
        let sha = row["sha256"]
            .as_str()
            .ok_or_else(|| invalid("Night sound metadata hash missing"))?;
        let descriptor: serde_json::Value = serde_json::from_slice(&read_qualified_cache_file(
            root,
            LEGACY_NIGHT_DESCRIPTOR,
            len,
            sha,
        )?)
        .map_err(|error| invalid(format!("invalid Night sound metadata: {error}")))?;
        let source = manifest["source_files"]
            .as_array()
            .and_then(|sources| {
                sources
                    .iter()
                    .find(|row| row["asset_id"] == "owner_data:SOUND/SOUND.DAT")
            })
            .ok_or_else(|| invalid("Night owner sound source missing"))?;
        if descriptor["schema"] != "mc2-level1-sound-bank/v1"
            || descriptor["bank"] != 1
            || descriptor["quality_index"] != 3
            || descriptor["source_asset_id"] != "owner_data:SOUND/SOUND.DAT"
            || descriptor["source_sha256"] != source["sha256"]
            || source["path"] != "SOUND/SOUND.DAT"
        {
            return Err(invalid(
                "Night sound metadata does not match retained owner source",
            ));
        }
        let records = descriptor["metadata_records"]
            .as_array()
            .ok_or_else(|| invalid("Night sound metadata records missing"))?;
        if records.is_empty()
            || records.len() > MAX_SOUND_ID
            || descriptor["metadata_record_count"].as_u64() != Some(records.len() as u64)
        {
            return Err(invalid("Night sound metadata extent differs"));
        }
        let mut null_slots = [false; MAX_SOUND_ID + 1];
        for (index, record) in records.iter().enumerate() {
            let name = record["filename"]
                .as_str()
                .filter(|name| !name.is_empty() && name.is_ascii())
                .ok_or_else(|| invalid("Night source sound filename is invalid"))?;
            if record["slot"].as_u64() != Some(index as u64) {
                return Err(invalid("Night sound metadata slot differs"));
            }
            null_slots[index + 1] = name.eq_ignore_ascii_case("null.wav");
        }
        Ok(Self {
            selection,
            null_slots,
            slot_count: records.len(),
        })
    }

    pub(super) fn is_null(&self, sound_id: u16) -> AbiResult<bool> {
        let id = usize::from(sound_id);
        if id == 0 || id > self.slot_count {
            return Err(invalid("sound ID is outside the selected source bank"));
        }
        Ok(self.null_slots[id])
    }
}
