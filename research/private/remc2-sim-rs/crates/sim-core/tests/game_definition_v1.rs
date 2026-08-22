use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sim_core::game_definition::{
    DefinitionErrorCode, GameDefinition, GamePackage, Mc2OwnedInputs, SemanticId, SemanticValue,
    default_catalog, engine_capabilities, load_mc2_owned,
};
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const SYNTHETIC_FIXTURE: &str = "tests/fixtures/custom-synthetic-v1";
const CENSUS_PATH: &str = "conformance/semantic-required-surfaces-v1.json";

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn synthetic_fixture() -> PathBuf {
    manifest_dir().join(SYNTHETIC_FIXTURE)
}

fn semantic_value(definition: &GameDefinition, id: &str) -> SemanticValue {
    definition
        .value(&SemanticId::parse(id).expect("valid test semantic ID"))
        .expect("definition contains semantic value")
        .clone()
}

#[test]
fn independent_synthetic_package_materializes_a_stable_neutral_definition() {
    let catalog = default_catalog().expect("bounded catalog is valid");
    let capabilities = engine_capabilities().expect("compiled capabilities are valid");
    let first = GameDefinition::from_package(
        GamePackage::load(synthetic_fixture()).expect("synthetic package validates"),
        &capabilities,
        &catalog,
    )
    .expect("synthetic definition materializes");
    let second = GameDefinition::from_package(
        GamePackage::load(synthetic_fixture()).expect("synthetic package revalidates"),
        &capabilities,
        &catalog,
    )
    .expect("synthetic definition rematerializes");

    assert_eq!(first.package_id(), "custom.synthetic");
    assert_eq!(first.revision(), 1);
    assert_eq!(first.definition_hash(), second.definition_hash());
    assert_eq!(first.package_graph_hash(), second.package_graph_hash());
    assert_eq!(first.artifact_graph_hash(), second.artifact_graph_hash());
    assert_eq!(
        first.definition_hash(),
        "sha256:d66b77a1d2180f6a773b1bc4b1666b10e3e967dcb354f81e42862463d6cc7e84"
    );
    assert_eq!(
        first.package_graph_hash(),
        "sha256:409c588163a01b54acb3c915f9e511fd63e6af6403533b6c7bcd6fcfc8c7957d"
    );
    assert_eq!(
        first.artifact_graph_hash(),
        "sha256:6230fa257ad72fd83e6a28c74132781fc7bc2824b8a3c281452b35e2b8d14907"
    );
    assert_eq!(first.artifacts().len(), 6);
    assert_eq!(first.provenance().kind.as_str(), "synthetic");
    assert_eq!(first.provenance().distribution, "redistributable");
    assert_eq!(
        semantic_value(&first, "flight.player.max_forward_speed"),
        SemanticValue::Integer(80)
    );
    assert_eq!(
        semantic_value(&first, "spell.fireball.mana_cost"),
        SemanticValue::Integer(100)
    );
    assert_eq!(
        semantic_value(&first, "ai.enemy.default.policy"),
        SemanticValue::Enum("classic_goat_v1".to_owned())
    );
    assert_eq!(
        semantic_value(&first, "presentation.spell.fireball.projectile_asset"),
        SemanticValue::Reference(
            "sha256:11200749e51e49d4a9f4be95ffa8f6c0636be87e36781481c89e6a0825232508".to_owned()
        )
    );
}

#[test]
fn real_owner_route_adapts_through_existing_validation_without_source_ids() {
    let private = manifest_dir().join("../../..");
    let inputs = Mc2OwnedInputs::new(
        private.join("generated/level1-export-m"),
        private.join("generated/level1-asset-closure.json"),
        private.join("generated/level1-unity-asset-cache-plan29-final"),
    );
    let catalog = default_catalog().expect("bounded catalog is valid");
    let definition = load_mc2_owned(&inputs, &catalog).expect("real owner route validates");

    assert_eq!(definition.package_id(), "mc2.owned");
    assert_eq!(definition.provenance().kind.as_str(), "owner_derived");
    assert_eq!(definition.provenance().distribution, "owner_local");
    assert_eq!(
        definition.provenance().source_sha256,
        "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8"
    );
    assert!(definition.artifacts().iter().any(|artifact| {
        artifact.id == "owner.projectile_asset"
            && artifact.sha256 == "f6755ec0f9931b6748c32893cc11838fb398043dafdc962eb5e288105cf0c88d"
            && artifact.bytes == 1_190
    }));
    assert_eq!(
        semantic_value(&definition, "flight.player.max_forward_speed"),
        SemanticValue::Integer(80)
    );
    assert!(catalog.descriptors().all(|descriptor| {
        !descriptor.id.as_str().contains("level") && !descriptor.id.as_str().contains("source_slot")
    }));
}

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TempPackage(PathBuf);

impl TempPackage {
    fn from_fixture() -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "carpet-flight-f2-package-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create test package directory");
        for entry in fs::read_dir(synthetic_fixture()).expect("read synthetic fixture") {
            let entry = entry.expect("read synthetic package member");
            fs::copy(entry.path(), root.join(entry.file_name()))
                .expect("copy synthetic package member");
        }
        Self(root)
    }

    fn mutate_manifest(&self, mutate: impl FnOnce(&mut Value)) {
        let path = self.0.join("package.json");
        let mut manifest: Value = serde_json::from_slice(&fs::read(&path).expect("read manifest"))
            .expect("parse manifest");
        mutate(&mut manifest);
        fs::write(
            path,
            serde_json::to_vec_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write mutated manifest");
    }

    fn mutate_variant_asset(&self, mutate: impl FnOnce(&mut Value)) {
        let asset_path = self.0.join("projectile-variant.indices.u8");
        let mut asset: Value = serde_json::from_slice(&fs::read(&asset_path).expect("read asset"))
            .expect("parse asset");
        mutate(&mut asset);
        let bytes = serde_json::to_vec(&asset).expect("serialize mutated asset");
        fs::write(&asset_path, &bytes).expect("write mutated asset");
        let hash = format!("{:x}", Sha256::digest(&bytes));
        self.mutate_manifest(|manifest| {
            let artifact = manifest["artifacts"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|row| row["id"] == "presentation.projectile_asset_variant")
                .unwrap();
            artifact["bytes"] = json!(bytes.len());
            artifact["sha256"] = json!(&hash);
            manifest["bindings"][2]["value"]["value"] = json!(format!("sha256:{hash}"));
        });
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn load_error(mutate: impl FnOnce(&mut Value)) -> DefinitionErrorCode {
    let package = TempPackage::from_fixture();
    package.mutate_manifest(mutate);
    GamePackage::load(&package.0)
        .expect_err("mutated package must fail")
        .code
}

#[test]
fn package_schema_fails_closed_for_members_code_graph_paths_provenance_and_hashes() {
    assert_eq!(
        load_error(|manifest| {
            manifest["surprise"] = json!(true);
        }),
        DefinitionErrorCode::UnknownMember
    );
    assert_eq!(
        load_error(|manifest| {
            manifest["provided_capabilities"] = json!([]);
        }),
        DefinitionErrorCode::ExecutableCapabilityForbidden
    );
    assert_eq!(
        load_error(|manifest| {
            manifest["artifacts"][0]["kind"] = json!("native_library");
        }),
        DefinitionErrorCode::ExecutableCapabilityForbidden
    );
    assert_eq!(
        load_error(|manifest| {
            manifest["artifacts"][1]["dependencies"] = json!(["definition.synthetic"]);
        }),
        DefinitionErrorCode::DependencyCycle
    );
    assert_eq!(
        load_error(|manifest| {
            let duplicate = manifest["bindings"][0].clone();
            manifest["bindings"].as_array_mut().unwrap().push(duplicate);
        }),
        DefinitionErrorCode::SemanticIdCollision
    );
    assert_eq!(
        load_error(|manifest| {
            manifest["artifacts"][0]["path"] = json!("../definition.json");
        }),
        DefinitionErrorCode::PathEscape
    );
    assert_eq!(
        load_error(|manifest| {
            manifest["provenance"]["distribution"] = json!("owner_local");
        }),
        DefinitionErrorCode::InvalidProvenance
    );
    assert_eq!(
        load_error(|manifest| {
            manifest["provenance"]["source_sha256"] =
                json!("0000000000000000000000000000000000000000000000000000000000000000");
        }),
        DefinitionErrorCode::InvalidProvenance
    );
    assert_eq!(
        load_error(|manifest| {
            manifest["artifacts"][0]["sha256"] =
                json!("0000000000000000000000000000000000000000000000000000000000000000");
        }),
        DefinitionErrorCode::HashMismatch
    );
    assert_eq!(
        load_error(|manifest| {
            manifest["bindings"][2]["value"]["value"] =
                json!("sha256:0000000000000000000000000000000000000000000000000000000000000000");
        }),
        DefinitionErrorCode::HashMismatch
    );
}

fn typed_asset_error(mutate: impl FnOnce(&mut Value)) -> DefinitionErrorCode {
    let package = TempPackage::from_fixture();
    package.mutate_variant_asset(mutate);
    let catalog = default_catalog().unwrap();
    GameDefinition::from_package(
        GamePackage::load(&package.0).expect("content-addressed mutated package"),
        &engine_capabilities().unwrap(),
        &catalog,
    )
    .expect_err("typed asset mutation must fail")
    .code
}

#[test]
fn typed_presentation_asset_rejects_wrong_contract_shape_animation_and_dependencies() {
    assert_eq!(
        typed_asset_error(|asset| asset["kind"] = json!("mesh")),
        DefinitionErrorCode::TypeMismatch
    );
    assert_eq!(
        typed_asset_error(|asset| asset["runtime_format"] = json!("png/v1")),
        DefinitionErrorCode::TypeMismatch
    );
    assert_eq!(
        typed_asset_error(|asset| asset["width"] = json!(0)),
        DefinitionErrorCode::OutOfRange
    );
    assert_eq!(
        typed_asset_error(|asset| asset["palette_rgba8"] = json!([0, 0, 0])),
        DefinitionErrorCode::OutOfRange
    );
    assert_eq!(
        typed_asset_error(|asset| {
            asset["frame_count"] = json!(2);
            let original = asset["indices"].as_array().unwrap().clone();
            asset["indices"].as_array_mut().unwrap().extend(original);
        }),
        DefinitionErrorCode::OutOfRange
    );
    assert_eq!(
        typed_asset_error(|asset| asset["frame_duration_ms"] = json!(10)),
        DefinitionErrorCode::TypeMismatch
    );
    assert_eq!(
        typed_asset_error(|asset| asset["coordinates"] = json!("y_up")),
        DefinitionErrorCode::TypeMismatch
    );
    assert_eq!(
        typed_asset_error(|asset| asset["dependencies"] = json!(["source.fireball.variant"])),
        DefinitionErrorCode::DependencyFailed
    );
}

#[test]
fn definition_rejects_unknown_compiled_capability_and_unknown_semantic_binding() {
    let catalog = default_catalog().expect("bounded catalog is valid");
    let capabilities = engine_capabilities().expect("compiled capabilities are valid");

    let unknown_capability = TempPackage::from_fixture();
    unknown_capability.mutate_manifest(|manifest| {
        manifest["requires_capabilities"][0]["id"] = json!("engine.unknown.capability");
    });
    let error = GameDefinition::from_package(
        GamePackage::load(&unknown_capability.0).expect("schema-valid package"),
        &capabilities,
        &catalog,
    )
    .expect_err("unknown compiled capability must fail");
    assert_eq!(error.code, DefinitionErrorCode::CapabilityUnavailable);

    let unknown_semantic = TempPackage::from_fixture();
    unknown_semantic.mutate_manifest(|manifest| {
        manifest["bindings"][0]["id"] = json!("ai.enemy.unknown_policy");
    });
    let error = GameDefinition::from_package(
        GamePackage::load(&unknown_semantic.0).expect("schema-valid package"),
        &capabilities,
        &catalog,
    )
    .expect_err("unknown semantic binding must fail");
    assert_eq!(error.code, DefinitionErrorCode::UnknownSemanticId);
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn assert_anchors(source: &str, anchors: &[Value], id: &str) {
    for anchor in anchors {
        let anchor = anchor.as_str().expect("anchor is a string");
        assert!(
            source.contains(anchor),
            "census anchor for {id} is absent: {anchor}"
        );
    }
}

#[test]
fn independent_source_census_matches_catalog_and_rejects_a_planted_consumed_constant() {
    let census: Value = serde_json::from_slice(
        &fs::read(manifest_dir().join(CENSUS_PATH)).expect("read independent surface census"),
    )
    .expect("parse independent surface census");
    assert_eq!(
        census["schema"].as_str(),
        Some("carpet-flight-semantic-required-surfaces/v1")
    );
    let rows = census["surfaces"].as_array().expect("surface rows");
    let catalog = default_catalog().expect("bounded catalog is valid");
    let catalog_ids = catalog
        .descriptors()
        .map(|descriptor| descriptor.id.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    let census_ids = rows
        .iter()
        .map(|row| row["semantic_id"].as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(catalog_ids, census_ids);
    assert_eq!(rows.len(), census_ids.len(), "census IDs must be unique");

    for row in rows {
        let id = row["semantic_id"].as_str().unwrap();
        let descriptor = catalog
            .descriptor(&SemanticId::parse(id).unwrap())
            .expect("census row has catalog descriptor");
        assert_eq!(row["access"].as_str(), Some(descriptor.access.as_str()));
        assert_eq!(
            descriptor.provenance_content_hash,
            format!(
                "sha256:{}",
                row["descriptor_provenance_sha256"].as_str().unwrap()
            )
        );

        let implementation_path = manifest_dir().join(row["implementation_file"].as_str().unwrap());
        let source = fs::read(&implementation_path).expect("read census implementation file");
        assert_eq!(
            sha256(&source),
            row["implementation_sha256"].as_str().unwrap(),
            "implementation fingerprint drift for {id}"
        );
        let source_text = String::from_utf8(source.clone()).expect("implementation is UTF-8");
        assert_anchors(
            &source_text,
            row["required_anchors"].as_array().unwrap(),
            id,
        );
        if let Some(additional_file) = row.get("additional_consumer_file").and_then(Value::as_str) {
            let consumer = fs::read_to_string(manifest_dir().join(additional_file))
                .expect("read additional consumer file");
            assert_anchors(
                &consumer,
                row["additional_consumer_anchors"].as_array().unwrap(),
                id,
            );
        }

        let mut planted = source;
        planted.extend_from_slice(b"\nconst PLANTED_UNREGISTERED_CONSUMED_CONSTANT: i32 = 17;\n");
        assert_ne!(
            sha256(&planted),
            row["implementation_sha256"].as_str().unwrap(),
            "the source fingerprint gate must reject a planted consumed constant"
        );
    }
}

#[test]
fn semantic_ids_refuse_source_and_package_layout_leaks() {
    for invalid in [
        "flight.level1.speed",
        "flight.level2.speed",
        "ai.source_slot.policy",
        "Level.Player.Speed",
        "single",
    ] {
        assert_eq!(
            SemanticId::parse(invalid)
                .expect_err("source-shaped ID must fail")
                .code,
            DefinitionErrorCode::InvalidSemanticId
        );
    }
}
