//! Selection-bound music assets for the installed opening campaign caches.

use super::campaign_abi::CampaignDayMusicLifecycle;
use super::validation::read_qualified_cache_file;
use super::{AbiError, AbiResult, MC_STATUS_IO_ERROR, MC_STATUS_VALIDATION_FAILED};
use serde::Deserialize;
use serde_json::Value;
use sim_core::campaign::spec::CampaignSelection;
use std::fs;
use std::path::{Path, PathBuf};

struct MusicBinding {
    descriptor: &'static str,
    bytes: usize,
    sha256: &'static str,
    track: u8,
    xmi_path: &'static str,
    xmi_bytes: usize,
    xmi_sha256: &'static str,
}

fn binding(selection: CampaignSelection) -> AbiResult<MusicBinding> {
    let (descriptor, bytes, sha256, track) = match (
        selection.external_level,
        selection.internal_level,
        selection.map_type_id,
    ) {
        (1, 0, 1) => (
            "audio/music-channel-0-f.json",
            4_957,
            "bbf5670314f89876b3ae2bdd42cfae717b5a36c36fc0cb1e498ce5778744b965",
            1,
        ),
        (2, 1, 0) => (
            "audio/descriptor.json",
            106_324,
            "73e7d78ea28996c8515dbbc791fce86e90ddcc7a01000bbd4bab82c7036e437e",
            2,
        ),
        (3, 2, 0) => (
            "audio/descriptor.json",
            106_324,
            "5854171168c8a474483b9fab430821031a4380c22e68c851b3f1baa124d89ee3",
            2,
        ),
        (4, 3, 2) => (
            "audio/descriptor.json",
            41_480,
            "a3c7e8467c9ba0d5973310745ec938d64436069d0b9b67622256268776599336",
            3,
        ),
        _ => {
            return Err(invalid(
                "no qualified music binding for this campaign selection",
            ));
        }
    };
    // Sound.cpp passes logical track - 1 to SOUND_init_MIDI_sequence.
    // Each map type therefore consumes its own owner MUSIC.DAT record.
    let (xmi_path, xmi_bytes, xmi_sha256) = match track {
        1 => (
            "audio/music/C2GAME1.xmi",
            10_352,
            "0f8081d8ef6ad609f406638862a573b9d1d014f940f0f3194e4d5687f724bd8d",
        ),
        2 => (
            "audio/music/C2GAME2.xmi",
            43_616,
            "8c98b86e74a6187a3a08ac34ef81aa3bd97cf493c34bee88b6d6969e79a6c856",
        ),
        3 => (
            "audio/music/C2GAME3.xmi",
            14_928,
            "566769008b9598bd03ad657575daa089f217385d7e7671dd948e6b2fea1e3251",
        ),
        _ => return Err(invalid("campaign music track is not source-bound")),
    };
    Ok(MusicBinding {
        descriptor,
        bytes,
        sha256,
        track,
        xmi_path,
        xmi_bytes,
        xmi_sha256,
    })
}

fn invalid(message: impl Into<String>) -> AbiError {
    AbiError::new(MC_STATUS_VALIDATION_FAILED, message)
}

/// Day's native material component and client aggregate share one audio sibling.
/// Night and Cave retain their audio alongside the other owner-cache outputs.
fn audio_root(root: &Path) -> Option<PathBuf> {
    if root.join("audio/music-channel-0-f.json").is_file()
        || root.join("audio/descriptor.json").is_file()
    {
        return Some(root.to_path_buf());
    }
    let component = root.join("components/audio");
    if component.join("audio/descriptor.json").is_file() {
        return Some(component);
    }
    if root.file_name().is_some_and(|name| name == "material")
        && let Some(parent) = root.parent()
        && parent.file_name().is_some_and(|name| name == "components")
    {
        let sibling = parent.join("audio");
        if sibling.join("audio/descriptor.json").is_file() {
            return Some(sibling);
        }
    }
    None
}

#[derive(Deserialize)]
struct CacheManifest {
    schema: String,
    outputs: Vec<CacheOutput>,
    profile: Option<Value>,
}

#[derive(Deserialize)]
struct CacheOutput {
    path: String,
    bytes: usize,
    sha256: String,
}

fn verify_manifest(root: &Path, spec: &MusicBinding) -> AbiResult<CacheManifest> {
    let path = root.join("manifest.json");
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        AbiError::new(
            MC_STATUS_IO_ERROR,
            format!("cannot inspect campaign audio manifest: {error}"),
        )
    })?;
    if metadata.file_type().is_symlink() || metadata.len() > 8 * 1024 * 1024 {
        return Err(invalid(
            "campaign audio manifest is a symlink or exceeds its bound",
        ));
    }
    let bytes = fs::read(path).map_err(|error| {
        AbiError::new(
            MC_STATUS_IO_ERROR,
            format!("cannot read campaign audio manifest: {error}"),
        )
    })?;
    let manifest: CacheManifest = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("invalid campaign audio manifest: {error}")))?;
    let expected_schema = match spec.track {
        1 => "mc2-owner-unity-asset-cache/v2",
        3 => "mc2-environment-material-cache/v1",
        _ => "mc2-day-audio-cache/v1",
    };
    if manifest.schema != expected_schema {
        return Err(invalid(
            "campaign audio cache schema does not match its selection",
        ));
    }
    for (path, len, sha) in [
        (spec.descriptor, spec.bytes, spec.sha256),
        (spec.xmi_path, spec.xmi_bytes, spec.xmi_sha256),
    ] {
        let mut rows = manifest.outputs.iter().filter(|row| row.path == path);
        let row = rows
            .next()
            .ok_or_else(|| invalid(format!("campaign audio manifest lacks {path}")))?;
        if rows.next().is_some() || row.bytes != len || row.sha256 != sha {
            return Err(invalid(format!(
                "campaign audio manifest binding mismatch for {path}"
            )));
        }
    }
    Ok(manifest)
}

/// Existing sealed Night assets retain their exact content pins. Day and Cave
/// envelopes also carry the owner and campaign package identities.
pub(super) fn load_campaign_music(
    owner_cache: Option<&Path>,
    selection: CampaignSelection,
    owner_package_sha256: &str,
    campaign_package_sha256: &str,
) -> AbiResult<Option<CampaignDayMusicLifecycle>> {
    let Some(root) = owner_cache.and_then(audio_root) else {
        return Ok(None);
    };
    let spec = binding(selection)?;
    let descriptor = read_qualified_cache_file(&root, spec.descriptor, spec.bytes, spec.sha256)?;
    let manifest = verify_manifest(&root, &spec)?;
    let descriptor: Value = serde_json::from_slice(&descriptor)
        .map_err(|error| invalid(format!("invalid campaign music descriptor: {error}")))?;
    if spec.track == 1 {
        if descriptor["schema"] != "mc2-level1-music-channel/v1" || descriptor["logical_track"] != 1
        {
            return Err(invalid("Night music descriptor has the wrong source track"));
        }
    } else {
        let profile = &descriptor["profile"];
        let (schema, owner_key) = if spec.track == 3 {
            ("mc2-cave-audio/v1", "owner_export_package_sha256")
        } else {
            ("mc2-day-audio/v1", "package_sha256")
        };
        if descriptor["schema"] != schema
            || manifest.profile.as_ref() != Some(profile)
            || profile["external_level"].as_u64() != Some(u64::from(selection.external_level))
            || profile["internal_level"].as_u64() != Some(u64::from(selection.internal_level))
            || profile["map_type"]["id"].as_u64() != Some(u64::from(selection.map_type_id))
            || profile[owner_key].as_str() != Some(owner_package_sha256)
            || profile["campaign_package_sha256"].as_str() != Some(campaign_package_sha256)
            || descriptor["music"]["maptype_music"] != spec.track
        {
            return Err(invalid(
                "music descriptor does not match the active campaign package",
            ));
        }
    }
    let _ = read_qualified_cache_file(&root, spec.xmi_path, spec.xmi_bytes, spec.xmi_sha256)?;
    let mut music = CampaignDayMusicLifecycle::start_day();
    music.track = spec.track;
    Ok(Some(music))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generated() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated")
    }

    #[test]
    fn installed_opening_caches_resolve_their_bound_music() {
        let root = generated();
        let night = load_campaign_music(
            Some(&root.join("mc2-level1-night-cache-v2")),
            CampaignSelection::ordinary(1).unwrap(),
            "",
            "",
        )
        .unwrap()
        .unwrap();
        assert_eq!(night.track, 1);
        for level in [2, 3] {
            let aggregate = root.join(format!("level{level}-day-spell-owner-cache-audio-v4"));
            let descriptor: Value = serde_json::from_slice(
                &fs::read(aggregate.join("components/audio/audio/descriptor.json")).unwrap(),
            )
            .unwrap();
            let profile = &descriptor["profile"];
            for cache in [aggregate.clone(), aggregate.join("components/material")] {
                let music = load_campaign_music(
                    Some(&cache),
                    CampaignSelection::ordinary(level).unwrap(),
                    profile["package_sha256"].as_str().unwrap(),
                    profile["campaign_package_sha256"].as_str().unwrap(),
                )
                .unwrap()
                .unwrap();
                assert_eq!(music.track, 2);
                assert!(music.loaded && music.playing);
            }
        }
    }

    #[test]
    fn wrong_campaign_or_cross_level_audio_is_rejected() {
        let root = generated().join("level2-day-spell-owner-cache-audio-v4/components/material");
        assert!(
            load_campaign_music(
                Some(&root),
                CampaignSelection::ordinary(2).unwrap(),
                "wrong-owner",
                "wrong-campaign"
            )
            .is_err()
        );
        assert!(
            load_campaign_music(
                Some(&root),
                CampaignSelection::ordinary(3).unwrap(),
                "wrong-owner",
                "wrong-campaign"
            )
            .is_err()
        );
        assert!(
            load_campaign_music(None, CampaignSelection::ordinary(2).unwrap(), "", "")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn installed_cave_cache_resolves_source_track_three_and_rejects_wrong_package() {
        let root = generated().join("mc-oi62-environment-level4-audio-v3");
        let descriptor: Value =
            serde_json::from_slice(&fs::read(root.join("audio/descriptor.json")).unwrap()).unwrap();
        let profile = &descriptor["profile"];
        let owner = profile["owner_export_package_sha256"].as_str().unwrap();
        let campaign = profile["campaign_package_sha256"].as_str().unwrap();
        let selection = CampaignSelection::ordinary(4).unwrap();
        let music = load_campaign_music(Some(&root), selection, owner, campaign)
            .unwrap()
            .unwrap();
        assert_eq!(music.track, 3);
        assert!(music.loaded && music.playing);
        assert!(load_campaign_music(Some(&root), selection, "wrong-owner", campaign).is_err());
        assert!(load_campaign_music(Some(&root), selection, owner, "wrong-campaign").is_err());
        assert!(
            load_campaign_music(
                Some(&root),
                CampaignSelection::ordinary(3).unwrap(),
                owner,
                campaign
            )
            .is_err()
        );
    }
}
