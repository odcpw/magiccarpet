//! Owner-local MC2 compatibility adapter into the neutral package schema.
//!
//! Source/package selectors terminate here. The resulting definition exposes
//! only semantic IDs and content identities.

use super::{
    ArtifactKind, DefinitionErrorCode, FIREBALL_MANA_COST_CAPABILITY_ID, GameDefinition,
    RequiredCapability, Result, SemanticCatalog, SemanticId, SemanticValue, engine_capabilities,
};
use crate::{SPEED_LIMIT, SPEED_STEP, goat::GoatBehaviorConfig, native_world::NativeWorld};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sim_definition::adapter::{
    artifact_node, definition_error, game_package, owner_provenance, read_confined_file,
    validate_lower_sha256,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mc2OwnedInputs {
    pub export_root: PathBuf,
    pub closure_path: PathBuf,
    pub cache_root: PathBuf,
}

impl Mc2OwnedInputs {
    #[must_use]
    pub fn new(
        export_root: impl Into<PathBuf>,
        closure_path: impl Into<PathBuf>,
        cache_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            export_root: export_root.into(),
            closure_path: closure_path.into(),
            cache_root: cache_root.into(),
        }
    }
}

/// Validate the real owner route and adapt it into one neutral immutable
/// definition.
///
/// # Errors
///
/// Returns a typed error if the existing owner package/closure/cache loader,
/// selected semantic consumption facts, capability contract, or definition
/// materialization fails.
pub fn load_mc2_owned(
    inputs: &Mc2OwnedInputs,
    catalog: &SemanticCatalog,
) -> Result<GameDefinition> {
    let world = NativeWorld::load_with_owner_cache(
        &inputs.export_root,
        &inputs.closure_path,
        &inputs.cache_root,
    )
    .map_err(|error| {
        definition_error(
            DefinitionErrorCode::InvalidSchema,
            "mc2_owned.inputs",
            format!("owner package/closure/cache validation failed: {error}"),
        )
    })?;
    if world.goat_config() != GoatBehaviorConfig::LEVEL_ONE {
        return Err(definition_error(
            DefinitionErrorCode::HashMismatch,
            "ai.enemy.default.policy",
            "validated owner route does not carry the accepted creature policy",
        ));
    }
    let context = world.session_context();
    let spell_context = context.spell_fire.as_ref().ok_or_else(|| {
        definition_error(
            DefinitionErrorCode::HashMismatch,
            "spell.fireball.mana_cost",
            "validated owner route has no spell-holder context",
        )
    })?;
    let holder_cost = spell_context
        .holders
        .iter()
        .find(|holder| holder.model == 0)
        .map(|holder| holder.mana_cost)
        .ok_or_else(|| {
            definition_error(
                DefinitionErrorCode::HashMismatch,
                "spell.fireball.mana_cost",
                "validated owner route has no family-zero spell holder",
            )
        })?;
    if holder_cost != 100 {
        return Err(definition_error(
            DefinitionErrorCode::HashMismatch,
            "spell.fireball.mana_cost",
            format!("owner spell-holder cost changed: {holder_cost}"),
        ));
    }
    let cache = read_selected_cache_binding(
        &inputs.cache_root,
        world.package_sha256(),
        world.closure_sha256(),
    )?;

    let export_bytes = regular_file_bytes(&inputs.export_root)?;
    let closure_bytes = fs::metadata(&inputs.closure_path)
        .map_err(|error| {
            definition_error(
                DefinitionErrorCode::Io,
                "mc2_owned.closure",
                format!("cannot inspect closure: {error}"),
            )
        })?
        .len();
    let binding_payload = json!({
        "flight.player.max_forward_speed": SPEED_LIMIT,
        "flight.player.forward_acceleration": SPEED_STEP,
        "spell.fireball.mana_cost": holder_cost,
        "ai.enemy.default.policy": "classic_goat_v1",
        "presentation.spell.fireball.projectile_asset": format!("sha256:{}", cache.asset_sha256),
    });
    let binding_bytes = serde_json::to_vec(&binding_payload)
        .expect("owner semantic binding identity is serializable");
    let binding_hash = format!("{:x}", Sha256::digest(&binding_bytes));
    let artifacts = vec![
        artifact_node(
            "owner.export_graph",
            ArtifactKind::World,
            "owner/export-graph",
            world.package_sha256(),
            export_bytes,
            &[],
        )?,
        artifact_node(
            "owner.asset_closure",
            ArtifactKind::Presentation,
            "owner/asset-closure",
            world.closure_sha256(),
            closure_bytes,
            &["owner.export_graph"],
        )?,
        artifact_node(
            "owner.presentation_cache",
            ArtifactKind::Asset,
            "owner/presentation-cache",
            &cache.content_sha256,
            cache.declared_bytes,
            &["owner.asset_closure"],
        )?,
        artifact_node(
            "owner.projectile_asset",
            ArtifactKind::Asset,
            "owner/presentation-cache/projectile-asset",
            &cache.asset_sha256,
            1_190,
            &["owner.presentation_cache"],
        )?,
        artifact_node(
            "definition.semantic_bindings",
            ArtifactKind::Definition,
            "definition/semantic-bindings",
            &binding_hash,
            binding_bytes.len() as u64,
            &["owner.export_graph", "owner.projectile_asset"],
        )?,
    ];
    let requirements = [
        "engine.ai.creature_policy",
        "engine.flight.fixed_point",
        "engine.presentation.indexed_asset",
        FIREBALL_MANA_COST_CAPABILITY_ID,
        "engine.spell.projectile",
    ]
    .into_iter()
    .map(|id| {
        Ok(RequiredCapability {
            id: SemanticId::parse(id)?,
            exact_version: 1,
        })
    })
    .collect::<Result<Vec<_>>>()?;
    let bindings = vec![
        (
            SemanticId::parse("ai.enemy.default.policy")?,
            SemanticValue::Enum("classic_goat_v1".to_owned()),
        ),
        (
            SemanticId::parse("flight.player.max_forward_speed")?,
            SemanticValue::Integer(i64::from(SPEED_LIMIT)),
        ),
        (
            SemanticId::parse("flight.player.forward_acceleration")?,
            SemanticValue::Integer(i64::from(SPEED_STEP)),
        ),
        (
            SemanticId::parse("presentation.spell.fireball.projectile_asset")?,
            SemanticValue::Reference(format!("sha256:{}", cache.asset_sha256)),
        ),
        (
            SemanticId::parse("spell.fireball.mana_cost")?,
            SemanticValue::Integer(i64::from(holder_cost)),
        ),
    ];
    let package = game_package(
        "mc2.owned",
        1,
        requirements,
        artifacts,
        bindings,
        owner_provenance(world.package_sha256())?,
    )?;
    GameDefinition::from_package(package, &engine_capabilities()?, catalog)
}

struct SelectedCacheBinding {
    content_sha256: String,
    declared_bytes: u64,
    asset_sha256: String,
}

fn read_selected_cache_binding(
    root: &Path,
    package_sha256: &str,
    closure_sha256: &str,
) -> Result<SelectedCacheBinding> {
    let manifest_bytes = read_confined_file(root, "manifest.json")?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes).map_err(|error| {
        definition_error(
            DefinitionErrorCode::InvalidSchema,
            "owner_cache.manifest",
            format!("cannot parse qualified owner cache manifest: {error}"),
        )
    })?;
    let object = manifest.as_object().ok_or_else(|| {
        definition_error(
            DefinitionErrorCode::InvalidSchema,
            "owner_cache.manifest",
            "owner cache manifest is not an object",
        )
    })?;
    if !matches!(
        object.get("schema").and_then(Value::as_str),
        Some("mc2-owner-unity-asset-cache/v1" | "mc2-owner-unity-asset-cache/v2")
    ) || object.get("scope").and_then(Value::as_str) != Some("PRIVATE_TRACK_P_OWNER_DATA")
    {
        return Err(definition_error(
            DefinitionErrorCode::InvalidProvenance,
            "owner_cache.manifest",
            "owner cache schema or private scope mismatch",
        ));
    }
    let content_sha256 = object
        .get("cache_content_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            definition_error(
                DefinitionErrorCode::HashMismatch,
                "owner_cache.cache_content_sha256",
                "owner cache content identity is absent",
            )
        })?;
    validate_lower_sha256(content_sha256, "owner_cache.cache_content_sha256")?;
    let bindings = object
        .get("bindings")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            definition_error(
                DefinitionErrorCode::InvalidSchema,
                "owner_cache.bindings",
                "owner cache binding object is absent",
            )
        })?;
    if bindings
        .get("level_export_package_sha256")
        .and_then(Value::as_str)
        != Some(package_sha256)
        || bindings
            .get("level_asset_closure_sha256")
            .and_then(Value::as_str)
            != Some(closure_sha256)
    {
        return Err(definition_error(
            DefinitionErrorCode::HashMismatch,
            "owner_cache.bindings",
            "owner cache does not bind the validated export and closure",
        ));
    }
    let outputs = object
        .get("outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            definition_error(
                DefinitionErrorCode::InvalidSchema,
                "owner_cache.outputs",
                "owner cache outputs are absent",
            )
        })?;
    let selected = outputs
        .iter()
        .filter(|output| {
            output.get("index").and_then(Value::as_u64) == Some(463)
                && output.get("kind").and_then(Value::as_str) == Some("indexed_image_u8")
        })
        .collect::<Vec<_>>();
    if selected.len() != 1 {
        return Err(definition_error(
            DefinitionErrorCode::ArtifactCollision,
            "owner_cache.outputs",
            "selected projectile asset must have exactly one indexed-image output",
        ));
    }
    let selected = selected[0]
        .as_object()
        .expect("filtered output is an object");
    let path = selected
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            definition_error(
                DefinitionErrorCode::InvalidSchema,
                "owner_cache.outputs.path",
                "selected asset path is absent",
            )
        })?;
    let asset_sha256 = selected
        .get("sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            definition_error(
                DefinitionErrorCode::HashMismatch,
                "owner_cache.outputs.sha256",
                "selected asset hash is absent",
            )
        })?;
    validate_lower_sha256(asset_sha256, "owner_cache.outputs.sha256")?;
    let bytes = selected
        .get("bytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            definition_error(
                DefinitionErrorCode::InvalidSchema,
                "owner_cache.outputs.bytes",
                "selected asset byte count is absent",
            )
        })?;
    if selected.get("width").and_then(Value::as_u64) != Some(34)
        || selected.get("height").and_then(Value::as_u64) != Some(35)
        || bytes != 1_190
    {
        return Err(definition_error(
            DefinitionErrorCode::HashMismatch,
            "owner_cache.outputs",
            "selected projectile asset signature changed",
        ));
    }
    let asset = read_confined_file(root, path)?;
    if asset.len() as u64 != bytes || format!("{:x}", Sha256::digest(&asset)) != asset_sha256 {
        return Err(definition_error(
            DefinitionErrorCode::HashMismatch,
            path,
            "selected projectile asset bytes differ from the qualified cache manifest",
        ));
    }
    let declared_bytes = outputs.iter().try_fold(0_u64, |total, output| {
        let bytes = output.get("bytes").and_then(Value::as_u64).ok_or_else(|| {
            definition_error(
                DefinitionErrorCode::InvalidSchema,
                "owner_cache.outputs.bytes",
                "owner cache output byte count is absent",
            )
        })?;
        total.checked_add(bytes).ok_or_else(|| {
            definition_error(
                DefinitionErrorCode::OutOfRange,
                "owner_cache.outputs.bytes",
                "owner cache declared byte total overflows u64",
            )
        })
    })?;
    Ok(SelectedCacheBinding {
        content_sha256: content_sha256.to_owned(),
        declared_bytes,
        asset_sha256: asset_sha256.to_owned(),
    })
}

fn regular_file_bytes(root: &Path) -> Result<u64> {
    let mut total = 0_u64;
    for entry in fs::read_dir(root).map_err(|error| {
        definition_error(
            DefinitionErrorCode::Io,
            "mc2_owned.export",
            format!("cannot enumerate owner export: {error}"),
        )
    })? {
        let entry = entry.map_err(|error| {
            definition_error(
                DefinitionErrorCode::Io,
                "mc2_owned.export",
                format!("cannot inspect owner export: {error}"),
            )
        })?;
        let metadata = entry.metadata().map_err(|error| {
            definition_error(
                DefinitionErrorCode::Io,
                "mc2_owned.export",
                format!("cannot inspect owner export member: {error}"),
            )
        })?;
        if !metadata.is_file()
            || entry
                .file_type()
                .map_err(|error| {
                    definition_error(
                        DefinitionErrorCode::Io,
                        "mc2_owned.export",
                        error.to_string(),
                    )
                })?
                .is_symlink()
        {
            return Err(definition_error(
                DefinitionErrorCode::PathEscape,
                entry.path().display().to_string(),
                "owner export contains a non-regular or symlink member",
            ));
        }
        total = total.checked_add(metadata.len()).ok_or_else(|| {
            definition_error(
                DefinitionErrorCode::OutOfRange,
                "mc2_owned.export",
                "owner export byte total overflows u64",
            )
        })?;
    }
    Ok(total)
}
