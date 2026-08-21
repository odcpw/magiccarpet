//! CF-02 contract-v1 integration gate over real exporter output.

use sha2::{Digest, Sha256};
use sim_core::campaign::spec::{
    BehaviorKey, CampaignCapabilityRegistry, CampaignDefinition, CampaignPackageError,
    CampaignSelection, ValidationStage,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

static TEMP_SEQUENCE: AtomicUsize = AtomicUsize::new(0);
static SESSION_CONSTRUCTIONS: AtomicUsize = AtomicUsize::new(0);

fn fixture() -> PathBuf {
    std::env::var_os("MC_CAMPAIGN_PACKAGE_V1").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-cf02-campaign-package-v1-a/qualified-level2")
        },
        PathBuf::from,
    )
}

struct TempPackage {
    root: PathBuf,
}

impl TempPackage {
    fn copy_from(source: &Path, label: &str) -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("mc-cf02-{label}-{}-{sequence}", std::process::id()));
        fs::create_dir(&root).expect("create unique CF-02 scratch package");
        for entry in fs::read_dir(source).expect("enumerate real CF-02 fixture") {
            let entry = entry.expect("inspect real CF-02 fixture member");
            assert!(
                entry
                    .file_type()
                    .expect("inspect fixture file type")
                    .is_file(),
                "the qualified fixture is a flat regular-file package"
            );
            fs::copy(entry.path(), root.join(entry.file_name()))
                .expect("copy CF-02 fixture member into isolated scratch package");
        }
        Self { root }
    }

    fn rewrite_definition(&self, mutate: impl FnOnce(&mut serde_json::Value)) {
        let definition_path = self.root.join("campaign-definition.json");
        let mut definition: serde_json::Value = serde_json::from_slice(
            &fs::read(&definition_path).expect("read scratch campaign definition"),
        )
        .expect("parse scratch campaign definition");
        mutate(&mut definition);
        let mut definition_bytes =
            serde_json::to_vec(&definition).expect("canonicalize scratch campaign definition");
        definition_bytes.push(b'\n');
        fs::write(&definition_path, &definition_bytes)
            .expect("write isolated scratch campaign definition");

        let manifest_path = self.root.join("manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).expect("read scratch manifest"))
                .expect("parse scratch manifest");
        manifest["files"]["campaign-definition.json"]["bytes"] =
            serde_json::Value::from(definition_bytes.len());
        manifest["files"]["campaign-definition.json"]["sha256"] =
            serde_json::Value::from(sha256_hex(&definition_bytes));

        let files = manifest["files"]
            .as_object()
            .expect("scratch manifest files object");
        let mut package_identity = Vec::new();
        for (name, identity) in files {
            package_identity.extend_from_slice(name.as_bytes());
            package_identity.push(0);
            package_identity.extend_from_slice(
                identity["sha256"]
                    .as_str()
                    .expect("scratch member hash")
                    .as_bytes(),
            );
            package_identity.push(0);
            package_identity.extend_from_slice(
                identity["bytes"]
                    .as_u64()
                    .expect("scratch member byte count")
                    .to_string()
                    .as_bytes(),
            );
            package_identity.push(b'\n');
        }
        manifest["package_sha256"] = serde_json::Value::from(sha256_hex(&package_identity));
        let mut manifest_bytes =
            serde_json::to_vec(&manifest).expect("serialize scratch outer manifest");
        manifest_bytes.push(b'\n');
        fs::write(manifest_path, manifest_bytes).expect("write isolated scratch manifest");
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).expect("remove owned CF-02 scratch package");
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct HarnessSession {
    definition: CampaignDefinition,
}

fn construct_harness_session(
    package: &Path,
    capabilities: &dyn CampaignCapabilityRegistry,
) -> Result<HarnessSession, CampaignPackageError> {
    let definition =
        CampaignDefinition::preflight(package, CampaignSelection::LEVEL2, capabilities)?;
    SESSION_CONSTRUCTIONS.fetch_add(1, Ordering::SeqCst);
    Ok(HarnessSession { definition })
}

fn expect_preconstruction_rejection(
    package: &Path,
    capabilities: &dyn CampaignCapabilityRegistry,
    expected_stage: ValidationStage,
) {
    SESSION_CONSTRUCTIONS.store(0, Ordering::SeqCst);
    let error = match construct_harness_session(package, capabilities) {
        Ok(_) => panic!("planted invalid package unexpectedly constructed a harness session"),
        Err(error) => error,
    };
    assert_eq!(error.stage, expected_stage, "{error}");
    assert_eq!(
        SESSION_CONSTRUCTIONS.load(Ordering::SeqCst),
        0,
        "preflight failure crossed the session-construction boundary"
    );
}

#[test]
fn cf02_real_package_and_full_planted_negative_set_are_preconstruction() {
    let accept_all = |_: &BehaviorKey| true;
    SESSION_CONSTRUCTIONS.store(0, Ordering::SeqCst);
    let positive = construct_harness_session(&fixture(), &accept_all)
        .expect("real exporter package reaches the post-preflight construction boundary");
    assert_eq!(
        positive.definition.completed_stage,
        ValidationStage::ResetState
    );
    assert_eq!(SESSION_CONSTRUCTIONS.load(Ordering::SeqCst), 1);

    let unknown_field = TempPackage::copy_from(&fixture(), "unknown-field");
    unknown_field.rewrite_definition(|definition| {
        definition
            .as_object_mut()
            .expect("definition root object")
            .insert("planted_unknown".to_owned(), serde_json::Value::Null);
    });
    expect_preconstruction_rejection(
        &unknown_field.root,
        &accept_all,
        ValidationStage::Definition,
    );

    let source_fact_mutation = TempPackage::copy_from(&fixture(), "source-fact-mutation");
    source_fact_mutation.rewrite_definition(|definition| {
        let selector =
            &mut definition["source_facts"][0]["selector_provenance"][0]["selector_value"];
        *selector = serde_json::Value::from(
            selector
                .as_u64()
                .expect("source-fact selector is an unsigned integer")
                + 1,
        );
    });
    expect_preconstruction_rejection(
        &source_fact_mutation.root,
        &accept_all,
        ValidationStage::SourceFacts,
    );

    let traversal = TempPackage::copy_from(&fixture(), "source-artifact-traversal");
    traversal.rewrite_definition(|definition| {
        definition["source_facts"][0]["source_artifacts"][0]["logical_path"] =
            serde_json::Value::from("../CD_Files/DATA/MSPRD0-0.DAT");
    });
    expect_preconstruction_rejection(&traversal.root, &accept_all, ValidationStage::SourceFacts);

    let validated =
        CampaignDefinition::preflight(fixture(), CampaignSelection::LEVEL2, &accept_all)
            .expect("read the real declared capability set");
    let missing_key = validated
        .required_behavior_keys()
        .next()
        .expect("Level 2 declares at least one required capability")
        .clone();
    let missing_capability = |key: &BehaviorKey| key != &missing_key;
    expect_preconstruction_rejection(
        &fixture(),
        &missing_capability,
        ValidationStage::BehaviorClosure,
    );

    let missing_transitive = TempPackage::copy_from(&fixture(), "missing-transitive-handler");
    missing_transitive.rewrite_definition(|definition| {
        let requirements = definition["required_behaviors"]
            .as_array_mut()
            .expect("required_behaviors array");
        let index = requirements
            .iter()
            .position(|requirement| {
                requirement["dispatch_site"].as_u64() == Some(2)
                    && requirement["provenance"]
                        .as_array()
                        .is_some_and(|provenance| {
                            provenance
                                .iter()
                                .any(|row| row.get("predecessor").is_some())
                        })
            })
            .expect("real package has a transitive following-pass requirement");
        requirements.remove(index);
    });
    expect_preconstruction_rejection(
        &missing_transitive.root,
        &accept_all,
        ValidationStage::BehaviorClosure,
    );
}
