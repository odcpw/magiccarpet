//! Admission of the source-qualified, bank-bearing Level 1 cache.
//!
//! Qualification identities describe the retained visual view. The current
//! campaign, owner export and raw runtime remain distinct, exact bindings.

use super::*;

pub(super) const SCHEMA: &str = "mc2-owner-unity-asset-cache/v2";
const MANIFEST_SHA256: &str = "3909045b6cf589c0278780e10096b4b630110027fd500e298fd93f2f8cec3317";
const CONTENT_SHA256: &str = "d63e748f0fa6550e8c0e451d81163ff71cfb3c8cdad1f516d97ea34644985569";
// Ordinal order is shared with the Python importer and JsonUtility consumer.
const BINDINGS: [&str; 15] = [
    "campaign_package_sha256",
    "goat_tmap_golden_file_sha256",
    "goat_tmap_oracle_manifest_sha256",
    "level_asset_closure_sha256",
    "level_export_package_sha256",
    "owner_catalog_file_sha256",
    "owner_catalog_sha256",
    "plan29_contract_file_sha256",
    "plan29_contract_self_sha256",
    "qualification_level_export_package_sha256",
    "qualification_runtime_capture_sha256",
    "qualified_runtime_capture_count",
    "qualified_runtime_capture_sha256",
    "runtime_visual_projection_schema",
    "runtime_visual_projection_sha256",
];

pub(super) fn validate(
    object: &serde_json::Map<String, Value>,
    manifest_sha256: &str,
    content_sha256: &str,
) -> Result<()> {
    if manifest_sha256 != MANIFEST_SHA256 || content_sha256 != CONTENT_SHA256 {
        return Err(NativeWorldError::new("Night cache identity mismatch"));
    }
    let bindings = require_object(object, "bindings", "Night cache manifest")?;
    if bindings.len() != BINDINGS.len() {
        return Err(NativeWorldError::new(
            "Night cache binding field set differs",
        ));
    }
    let mut metadata = format!("{SCHEMA}\n");
    for name in BINDINGS {
        let value = if name == "qualified_runtime_capture_count" {
            require_exact_u64(bindings, name, 8, "Night cache bindings")?;
            "8"
        } else if name == "runtime_visual_projection_schema" {
            let schema = require_string(bindings, name, "Night cache bindings")?;
            if schema != "mc2-runtime-state/v1" {
                return Err(NativeWorldError::new(
                    "Night visual projection schema mismatch",
                ));
            }
            schema
        } else {
            require_hash_string(bindings, name, "Night cache bindings")?
        };
        metadata.push_str(name);
        metadata.push('\0');
        metadata.push_str(value);
        metadata.push('\n');
    }
    metadata.push_str("cache_content_sha256\0");
    metadata.push_str(content_sha256);
    metadata.push('\n');
    if sha256_hex(metadata.as_bytes())
        != require_hash_string(object, "night_metadata_sha256", "Night cache manifest")?
    {
        return Err(NativeWorldError::new(
            "Night cache scalar metadata digest mismatch",
        ));
    }
    Ok(())
}

pub(super) fn require_campaign(
    cache: &QualifiedOwnerCacheReader,
    campaign_sha256: &str,
    owner_sha256: &str,
    runtime_sha256: &str,
) -> Result<()> {
    let manifest = cache.manifest_object()?;
    if require_string(manifest, "schema", "owner cache manifest")? != SCHEMA {
        return Ok(()); // All v1 admission pins are checked by the existing reader.
    }
    let bindings = require_object(manifest, "bindings", "Night cache manifest")?;
    for (key, expected) in [
        ("campaign_package_sha256", campaign_sha256),
        ("level_export_package_sha256", owner_sha256),
        ("qualified_runtime_capture_sha256", runtime_sha256),
    ] {
        if require_hash_string(bindings, key, "Night cache bindings")? != expected {
            return Err(NativeWorldError::new(format!(
                "Night cache does not bind selected {key}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_cache_binds_actual_campaign_and_raw_runtime() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc2-level1-night-cache-v2");
        if !root.is_dir() {
            eprintln!("private Night cache unavailable: {}", root.display());
            return;
        }
        let cache = QualifiedOwnerCacheReader::open(&root).expect("canonical Night cache");
        let campaign = "deacc01638e3d522c2764f8aa8284fe655b61f9cf6060b2d1a0b70b6fd2bd8a3";
        let owner = "61d2d4962aeafa6325d2d0da3de8d0bcfe5f5d2e6859bdaa5b849cea596eac42";
        let runtime = "9af481570b5f8a088ac2882fe1ebff9747097d9c34da8273ce737b4814bf6bcc";
        require_campaign(&cache, campaign, owner, runtime).expect("actual production triple");
        for tuple in [
            (owner, owner, runtime),
            (campaign, campaign, runtime),
            (
                campaign,
                owner,
                "b078dbed443e2cc3bf3bfd08a0f519492546d9f939db74a3d9c185d58f9c559f",
            ),
        ] {
            assert!(require_campaign(&cache, tuple.0, tuple.1, tuple.2).is_err());
        }
        cache
            .read("frame/source-frame-census.json")
            .expect("bound source census");
        assert_eq!(qualified_fireball_sound_artifacts(&root).unwrap().len(), 2);
    }
}
