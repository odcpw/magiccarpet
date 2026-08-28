//! Versioned content-addressed game-package schema and compiled capabilities.

use super::{
    DefinitionError, DefinitionErrorCode, GAME_PACKAGE_SCHEMA, Result, SemanticId, SemanticValue,
    exact_object, lower_content_hash, lower_sha256, required_string,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledCapability {
    pub id: SemanticId,
    pub version: u16,
    pub summary: String,
}

impl CompiledCapability {
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id.as_str(),
            "version": self.version,
            "provider": "engine_compiled",
            "summary": self.summary,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RequiredCapability {
    pub id: SemanticId,
    pub exact_version: u16,
}

impl RequiredCapability {
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({"id": self.id.as_str(), "exact_version": self.exact_version})
    }
}

#[derive(Clone, Debug)]
pub struct CompiledCapabilityRegistry {
    capabilities: BTreeMap<SemanticId, CompiledCapability>,
}

impl CompiledCapabilityRegistry {
    /// Construct a registry and reject duplicate compiled capability IDs.
    ///
    /// # Errors
    ///
    /// Returns an error when IDs collide.
    pub fn new(capabilities: Vec<CompiledCapability>) -> Result<Self> {
        let count = capabilities.len();
        let capabilities = capabilities
            .into_iter()
            .map(|capability| (capability.id.clone(), capability))
            .collect::<BTreeMap<_, _>>();
        if capabilities.len() != count {
            return Err(DefinitionError::new(
                DefinitionErrorCode::CapabilityUnavailable,
                "engine.capabilities",
                "compiled capability registry contains a duplicate ID",
            ));
        }
        Ok(Self { capabilities })
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &CompiledCapability> {
        self.capabilities.values()
    }

    pub(crate) fn require_all(&self, requirements: &[RequiredCapability]) -> Result<()> {
        for requirement in requirements {
            let Some(capability) = self.capabilities.get(&requirement.id) else {
                return Err(DefinitionError::new(
                    DefinitionErrorCode::CapabilityUnavailable,
                    requirement.id.as_str(),
                    "package requires a capability this engine does not compile",
                ));
            };
            if capability.version != requirement.exact_version {
                return Err(DefinitionError::new(
                    DefinitionErrorCode::CapabilityUnavailable,
                    requirement.id.as_str(),
                    format!(
                        "package requires exact version {}, engine provides {}",
                        requirement.exact_version, capability.version
                    ),
                ));
            }
        }
        Ok(())
    }
}

/// The compiled capability set available to data packages in this build.
///
/// # Errors
///
/// Returns an error only if the static engine registry contains an invalid or
/// duplicate ID.
pub fn engine_capabilities() -> Result<CompiledCapabilityRegistry> {
    CompiledCapabilityRegistry::new(
        [
            (
                "engine.ai.creature_policy",
                "Bounded compiled creature perceive/decide/act lifecycle.",
            ),
            (
                "engine.flight.fixed_point",
                "Deterministic fixed-width player flight integration.",
            ),
            (
                "engine.objective.castle_mana_percent",
                "Owned-Castle mana percentage objective evaluation.",
            ),
            (
                "engine.objective.no_living_creatures",
                "Package-bound complete-lifecycle creature absence objective evaluation.",
            ),
            (
                "engine.presentation.indexed_asset",
                "Typed indexed presentation-asset binding.",
            ),
            (
                "engine.spell.projectile",
                "Compiled spell activation and projectile lifecycle.",
            ),
        ]
        .into_iter()
        .map(|(id, summary)| {
            Ok(CompiledCapability {
                id: SemanticId::parse(id)?,
                version: 1,
                summary: summary.to_owned(),
            })
        })
        .collect::<Result<Vec<_>>>()?,
    )
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Definition,
    Rules,
    BehaviorGraph,
    World,
    Presentation,
    Asset,
}

impl ArtifactKind {
    fn parse(value: &str, path: &str) -> Result<Self> {
        match value {
            "definition" => Ok(Self::Definition),
            "rules" => Ok(Self::Rules),
            "behavior_graph" => Ok(Self::BehaviorGraph),
            "world" => Ok(Self::World),
            "presentation" => Ok(Self::Presentation),
            "asset" => Ok(Self::Asset),
            "executable" | "native_library" | "wasm" | "script" => Err(DefinitionError::new(
                DefinitionErrorCode::ExecutableCapabilityForbidden,
                path,
                "packages cannot provide executable code or compiled engine capabilities",
            )),
            _ => Err(DefinitionError::new(
                DefinitionErrorCode::InvalidSchema,
                path,
                "unknown package artifact kind",
            )),
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Definition => "definition",
            Self::Rules => "rules",
            Self::BehaviorGraph => "behavior_graph",
            Self::World => "world",
            Self::Presentation => "presentation",
            Self::Asset => "asset",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArtifactNode {
    pub id: String,
    pub kind: ArtifactKind,
    pub logical_path: String,
    pub sha256: String,
    pub bytes: u64,
    pub dependencies: Vec<String>,
}

impl ArtifactNode {
    pub(crate) fn adapter(
        id: &str,
        kind: ArtifactKind,
        logical_path: &str,
        sha256: &str,
        bytes: u64,
        dependencies: &[&str],
    ) -> Result<Self> {
        SemanticId::parse(id)?;
        validate_relative_path(logical_path, id)?;
        lower_sha256(sha256, id)?;
        Ok(Self {
            id: id.to_owned(),
            kind,
            logical_path: logical_path.to_owned(),
            sha256: sha256.to_owned(),
            bytes,
            dependencies: dependencies
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        })
    }

    pub(crate) fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "kind": self.kind.as_str(),
            "path": self.logical_path,
            "sha256": self.sha256,
            "bytes": self.bytes,
            "dependencies": self.dependencies,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageProvenanceKind {
    Synthetic,
    OwnerDerived,
}

impl PackageProvenanceKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Synthetic => "synthetic",
            Self::OwnerDerived => "owner_derived",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PackageProvenance {
    pub kind: PackageProvenanceKind,
    pub distribution: String,
    pub source_sha256: String,
}

impl PackageProvenance {
    pub(crate) fn owner_derived(source_sha256: &str) -> Result<Self> {
        lower_sha256(source_sha256, "provenance.source_sha256")?;
        Ok(Self {
            kind: PackageProvenanceKind::OwnerDerived,
            distribution: "owner_local".to_owned(),
            source_sha256: source_sha256.to_owned(),
        })
    }

    fn to_json(&self) -> Value {
        json!({
            "kind": self.kind.as_str(),
            "distribution": self.distribution,
            "source_sha256": self.source_sha256,
        })
    }
}

#[derive(Clone, Debug)]
pub struct GamePackage {
    pub(crate) package_id: String,
    pub(crate) package_version: u64,
    pub(crate) package_graph_hash: String,
    pub(crate) artifact_graph_hash: String,
    pub(crate) required_capabilities: Vec<RequiredCapability>,
    pub(crate) artifacts: Vec<ArtifactNode>,
    pub(crate) artifact_payloads: BTreeMap<String, Vec<u8>>,
    pub(crate) bindings: Vec<(SemanticId, SemanticValue)>,
    pub(crate) provenance: PackageProvenance,
}

impl GamePackage {
    /// Load and validate a portable data-only package.
    ///
    /// # Errors
    ///
    /// Returns a typed error for schema/member/path/hash/provenance/capability,
    /// graph, or semantic-binding failures. No definition is created on error.
    pub fn load(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        let manifest_path = root.join("package.json");
        let manifest_bytes = fs::read(&manifest_path).map_err(|error| {
            DefinitionError::new(
                DefinitionErrorCode::Io,
                "package.json",
                format!("cannot read package manifest: {error}"),
            )
        })?;
        let manifest: Value = serde_json::from_slice(&manifest_bytes).map_err(|error| {
            DefinitionError::new(
                DefinitionErrorCode::InvalidSchema,
                "package.json",
                format!("invalid package JSON: {error}"),
            )
        })?;
        let raw_object = manifest.as_object().ok_or_else(|| {
            DefinitionError::new(
                DefinitionErrorCode::InvalidSchema,
                "package.json",
                "package manifest is not an object",
            )
        })?;
        if raw_object.contains_key("provided_capabilities")
            || raw_object.contains_key("executable_capabilities")
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::ExecutableCapabilityForbidden,
                "package.json",
                "engine capabilities are compiled by the engine; packages may only require them",
            ));
        }
        let object = exact_object(
            &manifest,
            &[
                "artifacts",
                "bindings",
                "package",
                "provenance",
                "requires_capabilities",
                "schema",
            ],
            "package.json",
        )?;
        if required_string(object, "schema", "package.json")? != GAME_PACKAGE_SCHEMA {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidSchema,
                "package.json.schema",
                "unsupported game-package schema",
            ));
        }
        let package_object = exact_object(
            &object["package"],
            &["id", "version"],
            "package.json.package",
        )?;
        let package_id = required_string(package_object, "id", "package.json.package")?;
        SemanticId::parse(package_id)?;
        let package_version = package_object["version"]
            .as_u64()
            .filter(|value| *value > 0)
            .ok_or_else(|| {
                DefinitionError::new(
                    DefinitionErrorCode::InvalidSchema,
                    "package.json.package.version",
                    "package version must be a positive integer",
                )
            })?;
        let provenance = parse_provenance(&object["provenance"])?;
        let required_capabilities = parse_requirements(&object["requires_capabilities"])?;
        let (artifacts, artifact_payloads) = parse_artifacts(root, &object["artifacts"])?;
        validate_artifact_graph(&artifacts)?;
        validate_directory_set(root, &artifacts)?;
        let bindings = parse_bindings(&object["bindings"])?;
        let mut package = Self::materialize(
            package_id,
            package_version,
            required_capabilities,
            artifacts,
            bindings,
            provenance,
        )?;
        package.artifact_payloads = artifact_payloads;
        Ok(package)
    }

    pub(crate) fn materialize(
        package_id: &str,
        package_version: u64,
        mut required_capabilities: Vec<RequiredCapability>,
        mut artifacts: Vec<ArtifactNode>,
        mut bindings: Vec<(SemanticId, SemanticValue)>,
        provenance: PackageProvenance,
    ) -> Result<Self> {
        SemanticId::parse(package_id)?;
        if package_version == 0 {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidSchema,
                "package.version",
                "package version must be positive",
            ));
        }
        required_capabilities.sort_by(|left, right| left.id.cmp(&right.id));
        artifacts.sort_by(|left, right| left.id.cmp(&right.id));
        bindings.sort_by(|left, right| left.0.cmp(&right.0));
        validate_requirement_ids(&required_capabilities)?;
        validate_artifact_graph(&artifacts)?;
        validate_binding_ids(&bindings)?;
        validate_content_references(&artifacts, &bindings)?;
        if !artifacts
            .iter()
            .any(|artifact| artifact.sha256 == provenance.source_sha256)
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidProvenance,
                "package.provenance.source_sha256",
                "provenance source identity is absent from the artifact graph",
            ));
        }
        let artifact_payload = Value::Array(artifacts.iter().map(ArtifactNode::to_json).collect());
        let artifact_graph_hash = format!(
            "sha256:{:x}",
            Sha256::digest(
                serde_json::to_vec(&artifact_payload)
                    .expect("artifact identity payload is serializable")
            )
        );
        let package_payload = json!({
            "schema": GAME_PACKAGE_SCHEMA,
            "package": {"id": package_id, "version": package_version},
            "provenance": provenance.to_json(),
            "requires_capabilities": required_capabilities.iter().map(RequiredCapability::to_json).collect::<Vec<_>>(),
            "artifacts": artifact_payload,
            "bindings": bindings.iter().map(|(id, value)| json!({"id": id.as_str(), "value": value.to_json()})).collect::<Vec<_>>(),
        });
        let package_graph_hash = format!(
            "sha256:{:x}",
            Sha256::digest(
                serde_json::to_vec(&package_payload)
                    .expect("package identity payload is serializable")
            )
        );
        Ok(Self {
            package_id: package_id.to_owned(),
            package_version,
            package_graph_hash,
            artifact_graph_hash,
            required_capabilities,
            artifacts,
            artifact_payloads: BTreeMap::new(),
            bindings,
            provenance,
        })
    }
}

fn validate_requirement_ids(requirements: &[RequiredCapability]) -> Result<()> {
    let ids = requirements
        .iter()
        .map(|requirement| &requirement.id)
        .collect::<BTreeSet<_>>();
    if ids.len() != requirements.len() {
        return Err(DefinitionError::new(
            DefinitionErrorCode::CapabilityUnavailable,
            "package.requires_capabilities",
            "package repeats a compiled capability requirement",
        ));
    }
    Ok(())
}

fn validate_binding_ids(bindings: &[(SemanticId, SemanticValue)]) -> Result<()> {
    let ids = bindings.iter().map(|(id, _)| id).collect::<BTreeSet<_>>();
    if ids.len() != bindings.len() {
        return Err(DefinitionError::new(
            DefinitionErrorCode::SemanticIdCollision,
            "package.bindings",
            "package repeats a semantic binding ID",
        ));
    }
    Ok(())
}

fn validate_content_references(
    artifacts: &[ArtifactNode],
    bindings: &[(SemanticId, SemanticValue)],
) -> Result<()> {
    let hashes = artifacts
        .iter()
        .map(|artifact| format!("sha256:{}", artifact.sha256))
        .collect::<BTreeSet<_>>();
    for (id, value) in bindings {
        if let SemanticValue::Reference(reference) = value
            && !hashes.contains(reference)
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::HashMismatch,
                id.as_str(),
                "semantic content reference is not present in the package artifact graph",
            ));
        }
    }
    Ok(())
}

fn parse_provenance(value: &Value) -> Result<PackageProvenance> {
    let object = exact_object(
        value,
        &["distribution", "kind", "source_sha256"],
        "package.json.provenance",
    )?;
    let kind = match required_string(object, "kind", "package.json.provenance")? {
        "synthetic" => PackageProvenanceKind::Synthetic,
        "owner_derived" => PackageProvenanceKind::OwnerDerived,
        _ => {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidProvenance,
                "package.json.provenance.kind",
                "unknown package provenance kind",
            ));
        }
    };
    let distribution = required_string(object, "distribution", "package.json.provenance")?;
    let valid_pair = matches!(
        (kind, distribution),
        (PackageProvenanceKind::Synthetic, "redistributable")
            | (PackageProvenanceKind::OwnerDerived, "owner_local")
    );
    if !valid_pair {
        return Err(DefinitionError::new(
            DefinitionErrorCode::InvalidProvenance,
            "package.json.provenance.distribution",
            "provenance kind and distribution policy are incompatible",
        ));
    }
    let source_sha256 = required_string(object, "source_sha256", "package.json.provenance")?;
    lower_sha256(source_sha256, "package.json.provenance.source_sha256")?;
    Ok(PackageProvenance {
        kind,
        distribution: distribution.to_owned(),
        source_sha256: source_sha256.to_owned(),
    })
}

fn parse_requirements(value: &Value) -> Result<Vec<RequiredCapability>> {
    let array = value.as_array().ok_or_else(|| {
        DefinitionError::new(
            DefinitionErrorCode::InvalidSchema,
            "package.json.requires_capabilities",
            "capability requirements are not an array",
        )
    })?;
    let mut result = Vec::with_capacity(array.len());
    let mut ids = BTreeSet::new();
    for (index, value) in array.iter().enumerate() {
        let path = format!("package.json.requires_capabilities[{index}]");
        let object = exact_object(value, &["exact_version", "id"], &path)?;
        let id = SemanticId::parse(required_string(object, "id", &path)?)?;
        if !ids.insert(id.clone()) {
            return Err(DefinitionError::new(
                DefinitionErrorCode::CapabilityUnavailable,
                id.as_str(),
                "package repeats a compiled capability requirement",
            ));
        }
        let exact_version = object["exact_version"]
            .as_u64()
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| *value > 0)
            .ok_or_else(|| {
                DefinitionError::new(
                    DefinitionErrorCode::InvalidSchema,
                    format!("{path}.exact_version"),
                    "capability version must be a positive u16",
                )
            })?;
        result.push(RequiredCapability { id, exact_version });
    }
    Ok(result)
}

fn parse_artifacts(
    root: &Path,
    value: &Value,
) -> Result<(Vec<ArtifactNode>, BTreeMap<String, Vec<u8>>)> {
    let array = value.as_array().ok_or_else(|| {
        DefinitionError::new(
            DefinitionErrorCode::InvalidSchema,
            "package.json.artifacts",
            "artifacts are not an array",
        )
    })?;
    let mut result = Vec::with_capacity(array.len());
    let mut payloads = BTreeMap::new();
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for (index, value) in array.iter().enumerate() {
        let path = format!("package.json.artifacts[{index}]");
        let object = exact_object(
            value,
            &["bytes", "dependencies", "id", "kind", "path", "sha256"],
            &path,
        )?;
        let id = required_string(object, "id", &path)?;
        SemanticId::parse(id)?;
        if !ids.insert(id.to_owned()) {
            return Err(DefinitionError::new(
                DefinitionErrorCode::ArtifactCollision,
                id,
                "package repeats an artifact ID",
            ));
        }
        let logical_path = required_string(object, "path", &path)?;
        validate_relative_path(logical_path, &format!("{path}.path"))?;
        if !paths.insert(logical_path.to_owned()) {
            return Err(DefinitionError::new(
                DefinitionErrorCode::ArtifactCollision,
                logical_path,
                "package repeats an artifact path",
            ));
        }
        let expected_hash = required_string(object, "sha256", &path)?;
        lower_sha256(expected_hash, &format!("{path}.sha256"))?;
        let expected_bytes = object["bytes"].as_u64().ok_or_else(|| {
            DefinitionError::new(
                DefinitionErrorCode::InvalidSchema,
                format!("{path}.bytes"),
                "artifact byte count is not u64",
            )
        })?;
        let bytes = read_confined_file(root, logical_path)?;
        if bytes.len() as u64 != expected_bytes
            || format!("{:x}", Sha256::digest(&bytes)) != expected_hash
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::HashMismatch,
                logical_path,
                "artifact bytes do not match declared size and hash",
            ));
        }
        let dependencies = object["dependencies"]
            .as_array()
            .ok_or_else(|| {
                DefinitionError::new(
                    DefinitionErrorCode::InvalidSchema,
                    format!("{path}.dependencies"),
                    "artifact dependencies are not an array",
                )
            })?
            .iter()
            .map(|value| {
                let dependency = value.as_str().ok_or_else(|| {
                    DefinitionError::new(
                        DefinitionErrorCode::InvalidSchema,
                        format!("{path}.dependencies"),
                        "artifact dependency is not a string",
                    )
                })?;
                SemanticId::parse(dependency)?;
                Ok(dependency.to_owned())
            })
            .collect::<Result<Vec<_>>>()?;
        result.push(ArtifactNode {
            id: id.to_owned(),
            kind: ArtifactKind::parse(required_string(object, "kind", &path)?, &path)?,
            logical_path: logical_path.to_owned(),
            sha256: expected_hash.to_owned(),
            bytes: expected_bytes,
            dependencies,
        });
        payloads.insert(id.to_owned(), bytes);
    }
    Ok((result, payloads))
}

fn parse_bindings(value: &Value) -> Result<Vec<(SemanticId, SemanticValue)>> {
    let array = value.as_array().ok_or_else(|| {
        DefinitionError::new(
            DefinitionErrorCode::InvalidSchema,
            "package.json.bindings",
            "semantic bindings are not an array",
        )
    })?;
    let mut result = Vec::with_capacity(array.len());
    let mut ids = BTreeSet::new();
    for (index, value) in array.iter().enumerate() {
        let path = format!("package.json.bindings[{index}]");
        let object = exact_object(value, &["id", "value"], &path)?;
        let id = SemanticId::parse(required_string(object, "id", &path)?)?;
        if !ids.insert(id.clone()) {
            return Err(DefinitionError::new(
                DefinitionErrorCode::SemanticIdCollision,
                id.as_str(),
                "package repeats a semantic binding ID",
            ));
        }
        let value_object = exact_object(
            &object["value"],
            &["type", "value"],
            &format!("{path}.value"),
        )?;
        let tag = required_string(value_object, "type", &format!("{path}.value"))?;
        let semantic_value = match tag {
            "integer" => value_object["value"]
                .as_i64()
                .map(SemanticValue::Integer)
                .ok_or_else(|| {
                    DefinitionError::new(
                        DefinitionErrorCode::TypeMismatch,
                        format!("{path}.value"),
                        "integer binding value is not i64",
                    )
                })?,
            "enum" => SemanticValue::Enum(
                required_string(value_object, "value", &format!("{path}.value"))?.to_owned(),
            ),
            "reference" => {
                let reference = required_string(value_object, "value", &format!("{path}.value"))?;
                lower_content_hash(reference, &format!("{path}.value"))?;
                SemanticValue::Reference(reference.to_owned())
            }
            _ => {
                return Err(DefinitionError::new(
                    DefinitionErrorCode::TypeMismatch,
                    format!("{path}.value.type"),
                    "unknown semantic value tag",
                ));
            }
        };
        result.push((id, semantic_value));
    }
    Ok(result)
}

fn validate_relative_path(value: &str, path: &str) -> Result<()> {
    let candidate = Path::new(value);
    if value.is_empty()
        || value.contains('\\')
        || candidate.is_absolute()
        || candidate.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::CurDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(DefinitionError::new(
            DefinitionErrorCode::PathEscape,
            path,
            "artifact path is absolute, traversing, or noncanonical",
        ));
    }
    Ok(())
}

pub(crate) fn read_confined_file(root: &Path, relative: &str) -> Result<Vec<u8>> {
    let root = root.canonicalize().map_err(|error| {
        DefinitionError::new(
            DefinitionErrorCode::Io,
            root.display().to_string(),
            format!("cannot resolve package root: {error}"),
        )
    })?;
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        DefinitionError::new(
            DefinitionErrorCode::Io,
            relative,
            format!("cannot inspect artifact: {error}"),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(DefinitionError::new(
            DefinitionErrorCode::PathEscape,
            relative,
            "artifact is not a regular non-symlink file",
        ));
    }
    let resolved = path.canonicalize().map_err(|error| {
        DefinitionError::new(
            DefinitionErrorCode::Io,
            relative,
            format!("cannot resolve artifact: {error}"),
        )
    })?;
    if !resolved.starts_with(&root) {
        return Err(DefinitionError::new(
            DefinitionErrorCode::PathEscape,
            relative,
            "artifact resolves outside package root",
        ));
    }
    fs::read(resolved).map_err(|error| {
        DefinitionError::new(
            DefinitionErrorCode::Io,
            relative,
            format!("cannot read artifact: {error}"),
        )
    })
}

fn validate_directory_set(root: &Path, artifacts: &[ArtifactNode]) -> Result<()> {
    let mut actual = BTreeSet::new();
    collect_files(root, root, &mut actual)?;
    let mut expected = artifacts
        .iter()
        .map(|artifact| artifact.logical_path.clone())
        .collect::<BTreeSet<_>>();
    expected.insert("package.json".to_owned());
    if actual != expected {
        return Err(DefinitionError::new(
            DefinitionErrorCode::UnknownMember,
            "package",
            format!("package file set differs; expected={expected:?} actual={actual:?}"),
        ));
    }
    Ok(())
}

fn collect_files(root: &Path, directory: &Path, output: &mut BTreeSet<String>) -> Result<()> {
    for entry in fs::read_dir(directory).map_err(|error| {
        DefinitionError::new(
            DefinitionErrorCode::Io,
            directory.display().to_string(),
            format!("cannot enumerate package directory: {error}"),
        )
    })? {
        let entry = entry.map_err(|error| {
            DefinitionError::new(
                DefinitionErrorCode::Io,
                directory.display().to_string(),
                format!("cannot inspect package entry: {error}"),
            )
        })?;
        let metadata = entry.metadata().map_err(|error| {
            DefinitionError::new(
                DefinitionErrorCode::Io,
                entry.path().display().to_string(),
                format!("cannot inspect package entry: {error}"),
            )
        })?;
        if entry
            .file_type()
            .map_err(|error| {
                DefinitionError::new(
                    DefinitionErrorCode::Io,
                    entry.path().display().to_string(),
                    error.to_string(),
                )
            })?
            .is_symlink()
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::PathEscape,
                entry.path().display().to_string(),
                "package contains a symlink",
            ));
        }
        if metadata.is_dir() {
            collect_files(root, &entry.path(), output)?;
        } else if metadata.is_file() {
            let relative = entry
                .path()
                .strip_prefix(root)
                .expect("enumerated package entry is under root")
                .to_str()
                .ok_or_else(|| {
                    DefinitionError::new(
                        DefinitionErrorCode::PathEscape,
                        entry.path().display().to_string(),
                        "package path is not UTF-8",
                    )
                })?
                .to_owned();
            output.insert(relative);
        } else {
            return Err(DefinitionError::new(
                DefinitionErrorCode::PathEscape,
                entry.path().display().to_string(),
                "package contains a non-file/non-directory entry",
            ));
        }
    }
    Ok(())
}

fn validate_artifact_graph(artifacts: &[ArtifactNode]) -> Result<()> {
    let ids = artifacts
        .iter()
        .map(|artifact| artifact.id.as_str())
        .collect::<BTreeSet<_>>();
    if ids.len() != artifacts.len() {
        return Err(DefinitionError::new(
            DefinitionErrorCode::ArtifactCollision,
            "package.artifacts",
            "artifact graph contains a duplicate ID",
        ));
    }
    for artifact in artifacts {
        for dependency in &artifact.dependencies {
            if !ids.contains(dependency.as_str()) {
                return Err(DefinitionError::new(
                    DefinitionErrorCode::DependencyCycle,
                    &artifact.id,
                    format!("artifact dependency is absent: {dependency}"),
                ));
            }
        }
    }
    let graph = artifacts
        .iter()
        .map(|artifact| (artifact.id.as_str(), artifact.dependencies.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for id in graph.keys() {
        visit(id, &graph, &mut visiting, &mut visited)?;
    }
    Ok(())
}

fn visit<'a>(
    id: &'a str,
    graph: &BTreeMap<&'a str, &'a [String]>,
    visiting: &mut BTreeSet<&'a str>,
    visited: &mut BTreeSet<&'a str>,
) -> Result<()> {
    if visited.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id) {
        return Err(DefinitionError::new(
            DefinitionErrorCode::DependencyCycle,
            id,
            "artifact dependency graph contains a cycle",
        ));
    }
    for dependency in graph[id] {
        visit(dependency, graph, visiting, visited)?;
    }
    visiting.remove(id);
    visited.insert(id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_registry_rejects_unknown_and_wrong_exact_versions() {
        let registry = engine_capabilities().expect("valid engine registry");
        let unknown = RequiredCapability {
            id: SemanticId::parse("engine.unknown.capability").unwrap(),
            exact_version: 1,
        };
        assert_eq!(
            registry.require_all(&[unknown]).unwrap_err().code,
            DefinitionErrorCode::CapabilityUnavailable
        );
        let wrong = RequiredCapability {
            id: SemanticId::parse("engine.flight.fixed_point").unwrap(),
            exact_version: 2,
        };
        assert_eq!(
            registry.require_all(&[wrong]).unwrap_err().code,
            DefinitionErrorCode::CapabilityUnavailable
        );
    }

    #[test]
    fn artifact_graph_rejects_cycle_and_collision() {
        let node = |id: &str, dependency: &str| ArtifactNode {
            id: id.to_owned(),
            kind: ArtifactKind::Definition,
            logical_path: format!("{id}.json"),
            sha256: "0".repeat(64),
            bytes: 0,
            dependencies: vec![dependency.to_owned()],
        };
        let cycle = [
            node("graph.one", "graph.two"),
            node("graph.two", "graph.one"),
        ];
        assert_eq!(
            validate_artifact_graph(&cycle).unwrap_err().code,
            DefinitionErrorCode::DependencyCycle
        );
        let collision = [
            node("graph.one", "graph.one"),
            node("graph.one", "graph.one"),
        ];
        assert_eq!(
            validate_artifact_graph(&collision).unwrap_err().code,
            DefinitionErrorCode::ArtifactCollision
        );
    }
}
