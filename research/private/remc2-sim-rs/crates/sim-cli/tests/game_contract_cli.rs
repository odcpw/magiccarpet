use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sim-core/tests/fixtures/custom-synthetic-v1")
}

fn level_one_campaign_fixture() -> PathBuf {
    std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE_V1").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-cf15-level1-package-current")
        },
        PathBuf::from,
    )
}

fn run(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_sim-cli"))
        .args(arguments)
        .output()
        .expect("run sim-cli")
}

fn wait_until(label: &str, mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if predicate() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {label}");
}

fn has_staged_workspace_state(workspace: &std::path::Path) -> bool {
    fs::read_dir(workspace).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with(".workspace-state.") && name.ends_with(".next")
        })
    })
}

struct TempWorkspace(PathBuf);

impl TempWorkspace {
    fn new(label: &str) -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "carpet-flight-cli-{label}-{}-{sequence}",
            std::process::id()
        )))
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn capabilities_describe_and_get_are_deterministic_data_only_reads() {
    let capabilities = run(&["game", "capabilities"]);
    assert!(capabilities.status.success());
    assert!(capabilities.stderr.is_empty());
    let capabilities: Value = serde_json::from_slice(&capabilities.stdout).expect("JSON stdout");
    assert_eq!(capabilities["ok"], true);
    assert_eq!(capabilities["data"]["read_only"], false);
    assert_eq!(capabilities["data"]["writable_descriptor_count"], 4);
    assert_eq!(capabilities["contract_version"], "5.0");
    assert_eq!(
        capabilities["data"]["authoring"]["change_set_schema"],
        "carpet-flight-change-set/v5"
    );
    assert_eq!(
        capabilities["data"]["authoring"]["game_definition_schema"],
        "carpet-flight-game-definition/v4"
    );
    assert_eq!(
        capabilities["data"]["authoring"]["operation_schema"]["variants"][0]["operation"],
        "set_integer"
    );
    assert_eq!(
        capabilities["data"]["authoring"]["operation_schema"]["variants"][0]["parameter_ids"][0],
        "flight.player.max_forward_speed"
    );
    let move_entity = capabilities["data"]["authoring"]["operation_schema"]["variants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|variant| variant["operation"] == "move_entity")
        .expect("move_entity is discoverable");
    assert_eq!(
        move_entity["entity_ids"],
        "game entities -> data.entities[].id"
    );
    let put_objective = capabilities["data"]["authoring"]["operation_schema"]["variants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|variant| variant["operation"] == "put_objective")
        .expect("put_objective is discoverable");
    assert_eq!(
        put_objective["objective_ids"],
        "game objectives -> data.objectives[].id"
    );
    let set_countdown = capabilities["data"]["authoring"]["operation_schema"]["variants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|variant| variant["operation"] == "set_mechanic_countdown")
        .expect("set_mechanic_countdown is discoverable");
    assert_eq!(set_countdown["target_id"], "<entity-id>.arming_ticks");
    assert_eq!(set_countdown["arming_ticks"]["unit"], "entity_visits");
    assert_eq!(set_countdown["arming_ticks"]["minimum"], 1);
    assert_eq!(set_countdown["arming_ticks"]["maximum"], i32::MAX);
    assert_eq!(
        capabilities["data"]["compiled_capabilities"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        capabilities["data"]["semantic_ids"]
            .as_array()
            .unwrap()
            .len(),
        4
    );

    let fixture = fixture();
    let fixture = fixture.to_str().expect("UTF-8 fixture path");
    let describe = run(&["game", "describe", "--package", fixture]);
    assert!(describe.status.success());
    assert!(describe.stderr.is_empty());
    let describe_json: Value = serde_json::from_slice(&describe.stdout).expect("JSON stdout");
    assert_eq!(describe_json["identity"]["package_id"], "custom.synthetic");
    assert_eq!(
        describe_json["data"]["descriptors"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(describe_json["data"]["page"]["complete"], true);

    let first_page = run(&["game", "describe", "--package", fixture, "--limit", "2"]);
    assert!(first_page.status.success());
    let first_page: Value = serde_json::from_slice(&first_page.stdout).expect("JSON stdout");
    assert_eq!(
        first_page["data"]["descriptors"].as_array().unwrap().len(),
        2
    );
    assert_eq!(first_page["data"]["page"]["complete"], false);
    let cursor = first_page["data"]["page"]["next_cursor"]
        .as_str()
        .expect("next cursor");
    let second_page = run(&[
        "game",
        "describe",
        "--package",
        fixture,
        "--limit",
        "2",
        "--cursor",
        cursor,
    ]);
    assert!(second_page.status.success());
    let second_page: Value = serde_json::from_slice(&second_page.stdout).expect("JSON stdout");
    assert_eq!(
        second_page["data"]["descriptors"].as_array().unwrap().len(),
        2
    );
    assert_eq!(second_page["data"]["page"]["complete"], true);

    let get_arguments = [
        "game",
        "get",
        "--package",
        fixture,
        "--id",
        "flight.player.max_forward_speed",
    ];
    let first = run(&get_arguments);
    let second = run(&get_arguments);
    assert!(first.status.success());
    assert!(first.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);
    let get_json: Value = serde_json::from_slice(&first.stdout).expect("JSON stdout");
    assert_eq!(get_json["data"]["value"]["type"], "integer");
    assert_eq!(get_json["data"]["value"]["value"], 80);
    assert_eq!(get_json["data"]["descriptor"]["access"], "writable");
}

#[test]
fn semantic_level_entities_are_discoverable_without_source_identity() {
    let campaign = level_one_campaign_fixture();
    let campaign = campaign.to_str().expect("UTF-8 campaign path");
    let entities = run(&[
        "game",
        "entities",
        "--campaign-package",
        campaign,
        "--id",
        "switch.start_area_exit",
    ]);
    assert!(entities.status.success(), "{:?}", entities.stderr);
    assert!(entities.stderr.is_empty());
    let entities: Value = serde_json::from_slice(&entities.stdout).unwrap();
    assert_eq!(entities["contract_version"], "5.0");
    assert_eq!(entities["data"]["page"]["total"], 1);
    assert_eq!(
        entities["data"]["entities"][0]["id"],
        "switch.start_area_exit"
    );
    assert_eq!(
        entities["data"]["entities"][0]["position"],
        serde_json::json!({"x": 74, "y": 212})
    );
    assert_eq!(
        entities["data"]["entities"][0]["position_access"],
        "session_reset"
    );
    let public_entity = entities["data"]["entities"][0].as_object().unwrap();
    assert_eq!(public_entity.len(), 4);
    assert!(public_entity.contains_key("id"));
    assert!(public_entity.contains_key("kind"));
    assert!(public_entity.contains_key("position"));
    assert!(public_entity.contains_key("position_access"));
    assert!(!public_entity.contains_key("activation_group"));
    assert!(!public_entity.contains_key("source_slot"));
    let get = run(&[
        "game",
        "get",
        "--campaign-package",
        campaign,
        "--id",
        "switch.start_area_exit",
    ]);
    assert!(get.status.success(), "{:?}", get.stderr);
    let get: Value = serde_json::from_slice(&get.stdout).unwrap();
    assert_eq!(get["data"]["entity"], entities["data"]["entities"][0]);

    let describe_position = run(&[
        "game",
        "describe",
        "--campaign-package",
        campaign,
        "--id",
        "switch.start_area_exit.position",
    ]);
    assert!(
        describe_position.status.success(),
        "{:?}",
        describe_position.stderr
    );
    let describe_position: Value = serde_json::from_slice(&describe_position.stdout).unwrap();
    let descriptor = &describe_position["data"]["descriptors"][0];
    assert_eq!(descriptor["id"], "switch.start_area_exit.position");
    assert_eq!(descriptor["kind"], "placement");
    assert_eq!(descriptor["value_type"], "position");
    assert_eq!(descriptor["unit"], "level_tile");
    assert_eq!(descriptor["access"], "writable");
    assert_eq!(descriptor["mutation_timing"], "session_reset");
    assert_eq!(descriptor["bounds_or_enum"]["maximum"], 255);
    assert_eq!(
        descriptor["default_value"]["value"],
        serde_json::json!({"x": 74, "y": 212})
    );
    assert_eq!(
        descriptor["effective_value"]["value"],
        serde_json::json!({"x": 74, "y": 212})
    );

    let get_position = run(&[
        "game",
        "get",
        "--campaign-package",
        campaign,
        "--id",
        "switch.start_area_exit.position",
    ]);
    assert!(get_position.status.success(), "{:?}", get_position.stderr);
    let get_position: Value = serde_json::from_slice(&get_position.stdout).unwrap();
    assert_eq!(get_position["data"]["descriptor"], *descriptor);
    assert_eq!(
        get_position["data"]["value"],
        serde_json::json!({"type": "position", "value": {"x": 74, "y": 212}})
    );
}

#[test]
fn campaign_workspace_authors_one_absence_countdown_through_the_typed_cli_lifecycle() {
    const ENTITY: &str = "mechanic.absence_countdown.worm.at_134_98";
    const TARGET: &str = "mechanic.absence_countdown.worm.at_134_98.arming_ticks";

    let temporary = TempWorkspace::new("semantic-countdown");
    let workspace = temporary.0.to_str().unwrap();
    let campaign = level_one_campaign_fixture();
    let campaign = campaign.to_str().expect("UTF-8 campaign path");

    let described = run(&[
        "game",
        "describe",
        "--campaign-package",
        campaign,
        "--id",
        TARGET,
    ]);
    assert!(described.status.success(), "{:?}", described.stderr);
    assert!(described.stderr.is_empty());
    let described: Value = serde_json::from_slice(&described.stdout).unwrap();
    let descriptor = &described["data"]["descriptors"][0];
    assert_eq!(described["contract_version"], "5.0");
    assert_eq!(
        described["identity"]["schema"],
        "carpet-flight-game-definition/v4"
    );
    assert_eq!(descriptor["id"], TARGET);
    assert_eq!(descriptor["kind"], "mechanic");
    assert_eq!(descriptor["value_type"], "integer");
    assert_eq!(descriptor["unit"], "entity_visits");
    assert_eq!(descriptor["access"], "writable");
    assert_eq!(descriptor["mutation_timing"], "session_reset");
    assert_eq!(descriptor["bounds_or_enum"]["minimum"], 1);
    assert_eq!(descriptor["bounds_or_enum"]["maximum"], i32::MAX);
    assert_eq!(descriptor["default_value"]["value"], 16);
    assert_eq!(descriptor["effective_value"]["value"], 16);

    let entities = run(&[
        "game",
        "entities",
        "--campaign-package",
        campaign,
        "--limit",
        "1",
    ]);
    assert!(entities.status.success(), "{:?}", entities.stderr);
    let entities: Value = serde_json::from_slice(&entities.stdout).unwrap();
    let countdown_offset = 4 + entities["data"]["page"]["total"].as_u64().unwrap();
    let cursor = format!(
        "v1.{}.{}.{}",
        described["data"]["catalog_hash"]
            .as_str()
            .unwrap()
            .trim_start_matches("sha256:"),
        described["identity"]["definition_hash"]
            .as_str()
            .unwrap()
            .trim_start_matches("sha256:"),
        countdown_offset
    );
    let listed = run(&[
        "game",
        "describe",
        "--campaign-package",
        campaign,
        "--limit",
        "2",
        "--cursor",
        &cursor,
    ]);
    assert!(listed.status.success(), "{:?}", listed.stderr);
    let listed: Value = serde_json::from_slice(&listed.stdout).unwrap();
    let listed_ids = listed["data"]["descriptors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|descriptor| descriptor["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(listed_ids.contains(&TARGET));

    let created = run(&[
        "game",
        "workspace-create",
        "--workspace",
        workspace,
        "--workspace-id",
        "semantic-countdown-workspace",
        "--campaign-package",
        campaign,
    ]);
    assert!(created.status.success(), "{:?}", created.stderr);

    let initial = run(&["game", "get", "--workspace", workspace, "--id", TARGET]);
    assert!(initial.status.success(), "{:?}", initial.stderr);
    let initial: Value = serde_json::from_slice(&initial.stdout).unwrap();
    assert_eq!(
        initial["data"]["value"],
        serde_json::json!({"type": "integer", "value": 16})
    );

    let changes_path = temporary.0.join("set-countdown.json");
    fs::write(
        &changes_path,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "set_mechanic_countdown",
            "entity": ENTITY,
            "arming_ticks": 2,
            "expected_arming_ticks": 16
        }]))
        .unwrap(),
    )
    .unwrap();
    let staged = run(&[
        "game",
        "set",
        "--workspace",
        workspace,
        "--idempotency-key",
        "semantic-countdown-apply",
        "--timing",
        "session_reset",
        "--changes",
        changes_path.to_str().unwrap(),
    ]);
    assert!(staged.status.success(), "{:?}", staged.stderr);
    let staged: Value = serde_json::from_slice(&staged.stdout).unwrap();
    assert_eq!(
        staged["data"]["change_set"]["schema_version"],
        "carpet-flight-change-set/v5"
    );
    assert_eq!(
        staged["data"]["change_set"]["operations"][0]["operation"],
        "set_mechanic_countdown"
    );
    let change_set_path = temporary.0.join("countdown-change-set.json");
    fs::write(
        &change_set_path,
        serde_json::to_vec(&staged["data"]["change_set"]).unwrap(),
    )
    .unwrap();

    let validated = run(&[
        "game",
        "validate",
        "--workspace",
        workspace,
        "--change-set",
        change_set_path.to_str().unwrap(),
    ]);
    assert!(validated.status.success(), "{:?}", validated.stderr);
    let validated: Value = serde_json::from_slice(&validated.stdout).unwrap();
    assert_eq!(validated["data"]["validation"]["accepted"], true);
    assert_eq!(
        validated["data"]["validation"]["affected_ids"],
        serde_json::json!([TARGET])
    );

    let diff = run(&[
        "game",
        "diff",
        "--workspace",
        workspace,
        "--change-set",
        change_set_path.to_str().unwrap(),
    ]);
    assert!(diff.status.success(), "{:?}", diff.stderr);
    let diff: Value = serde_json::from_slice(&diff.stdout).unwrap();
    assert_eq!(
        diff["data"]["diff"],
        serde_json::json!([{
            "id": TARGET,
            "before": {"type": "integer", "value": 16},
            "after": {"type": "integer", "value": 2}
        }])
    );

    let validation_receipt = validated["data"]["validation"]["receipt_hash"]
        .as_str()
        .unwrap();
    let applied = run(&[
        "game",
        "apply",
        "--workspace",
        workspace,
        "--change-set",
        change_set_path.to_str().unwrap(),
        "--validation-receipt",
        validation_receipt,
    ]);
    assert!(applied.status.success(), "{:?}", applied.stderr);
    let applied: Value = serde_json::from_slice(&applied.stdout).unwrap();
    assert_eq!(
        applied["data"]["receipt"]["applied_ids"],
        serde_json::json!([TARGET])
    );

    let authored = run(&["game", "get", "--workspace", workspace, "--id", TARGET]);
    assert!(authored.status.success(), "{:?}", authored.stderr);
    let authored: Value = serde_json::from_slice(&authored.stdout).unwrap();
    assert_eq!(authored["identity"]["revision"], 2);
    assert_eq!(authored["data"]["descriptor"]["default_value"]["value"], 16);
    assert_eq!(
        authored["data"]["descriptor"]["effective_value"]["value"],
        2
    );
    assert_eq!(
        authored["data"]["value"],
        serde_json::json!({"type": "integer", "value": 2})
    );

    let committed_state = fs::read(temporary.0.join("workspace-state.json")).unwrap();
    let zero_path = temporary.0.join("zero-countdown.json");
    fs::write(
        &zero_path,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "set_mechanic_countdown",
            "entity": ENTITY,
            "arming_ticks": 0,
            "expected_arming_ticks": 2
        }]))
        .unwrap(),
    )
    .unwrap();
    let zero_staged = run(&[
        "game",
        "set",
        "--workspace",
        workspace,
        "--idempotency-key",
        "semantic-countdown-zero",
        "--timing",
        "session_reset",
        "--changes",
        zero_path.to_str().unwrap(),
    ]);
    assert!(zero_staged.status.success(), "{:?}", zero_staged.stderr);
    let zero_staged: Value = serde_json::from_slice(&zero_staged.stdout).unwrap();
    let zero_change_set_path = temporary.0.join("zero-countdown-change-set.json");
    fs::write(
        &zero_change_set_path,
        serde_json::to_vec(&zero_staged["data"]["change_set"]).unwrap(),
    )
    .unwrap();
    let zero_validation = run(&[
        "game",
        "validate",
        "--workspace",
        workspace,
        "--change-set",
        zero_change_set_path.to_str().unwrap(),
    ]);
    assert!(
        zero_validation.status.success(),
        "{:?}",
        zero_validation.stderr
    );
    let zero_validation: Value = serde_json::from_slice(&zero_validation.stdout).unwrap();
    assert_eq!(zero_validation["data"]["validation"]["accepted"], false);
    assert_eq!(
        zero_validation["data"]["validation"]["diagnostics"][0]["code"],
        "OUT_OF_RANGE"
    );
    assert_eq!(
        fs::read(temporary.0.join("workspace-state.json")).unwrap(),
        committed_state
    );
}

#[test]
fn campaign_workspace_moves_a_semantic_entity_through_the_typed_cli_lifecycle() {
    let temporary = TempWorkspace::new("semantic-move-entity");
    let workspace = temporary.0.to_str().unwrap();
    let campaign = level_one_campaign_fixture();
    let campaign = campaign.to_str().expect("UTF-8 campaign path");
    let created = run(&[
        "game",
        "workspace-create",
        "--workspace",
        workspace,
        "--workspace-id",
        "semantic-move-workspace",
        "--campaign-package",
        campaign,
    ]);
    assert!(created.status.success(), "{:?}", created.stderr);

    let changes_path = temporary.0.join("move-entity.json");
    fs::write(
        &changes_path,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "move_entity",
            "entity": "switch.start_area_exit",
            "position": {"x": 150, "y": 100},
            "expected_position": {"x": 74, "y": 212}
        }]))
        .unwrap(),
    )
    .unwrap();
    let staged = run(&[
        "game",
        "set",
        "--workspace",
        workspace,
        "--idempotency-key",
        "semantic-move-apply",
        "--timing",
        "session_reset",
        "--changes",
        changes_path.to_str().unwrap(),
    ]);
    assert!(staged.status.success(), "{:?}", staged.stderr);
    let staged: Value = serde_json::from_slice(&staged.stdout).unwrap();
    let change_set_path = temporary.0.join("move-change-set.json");
    fs::write(
        &change_set_path,
        serde_json::to_vec(&staged["data"]["change_set"]).unwrap(),
    )
    .unwrap();

    let preview = run(&[
        "game",
        "preview",
        "--workspace",
        workspace,
        "--change-set",
        change_set_path.to_str().unwrap(),
    ]);
    assert!(preview.status.success(), "{:?}", preview.stderr);
    let preview: Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(preview["data"]["validation"]["accepted"], true);
    assert_eq!(
        preview["data"]["diff"],
        serde_json::json!([{
            "id": "switch.start_area_exit.position",
            "before": {"type": "position", "value": {"x": 74, "y": 212}},
            "after": {"type": "position", "value": {"x": 150, "y": 100}}
        }])
    );
    let validation_hash = preview["data"]["validation"]["receipt_hash"]
        .as_str()
        .unwrap();
    let applied = run(&[
        "game",
        "apply",
        "--workspace",
        workspace,
        "--change-set",
        change_set_path.to_str().unwrap(),
        "--validation-receipt",
        validation_hash,
    ]);
    assert!(applied.status.success(), "{:?}", applied.stderr);
    let applied: Value = serde_json::from_slice(&applied.stdout).unwrap();
    assert_eq!(
        applied["data"]["receipt"]["applied_ids"],
        serde_json::json!(["switch.start_area_exit.position"])
    );

    let get = run(&[
        "game",
        "get",
        "--workspace",
        workspace,
        "--id",
        "switch.start_area_exit.position",
    ]);
    assert!(get.status.success(), "{:?}", get.stderr);
    let get: Value = serde_json::from_slice(&get.stdout).unwrap();
    assert_eq!(get["identity"]["revision"], 2);
    assert_eq!(
        get["data"]["value"],
        serde_json::json!({"type": "position", "value": {"x": 150, "y": 100}})
    );
    assert_eq!(get["data"]["descriptor"]["access"], "writable");
    assert_eq!(
        get["data"]["descriptor"]["default_value"]["value"],
        serde_json::json!({"x": 74, "y": 212})
    );
    assert_eq!(
        get["data"]["descriptor"]["effective_value"]["value"],
        serde_json::json!({"x": 150, "y": 100})
    );
    assert_eq!(
        get["data"]["descriptor"]["temporal_contract"]["same_pass_epilogue"],
        "receipt/read-back commit; already-open sessions remain unchanged"
    );
    assert!(
        get["data"]["descriptor"]["temporal_contract"]["following_pass_cleanup"]
            .as_str()
            .unwrap()
            .contains("explicit reset/reopen creates a new runtime generation")
    );
    assert_ne!(
        get["data"]["descriptor"]["provenance"]["content_hash"],
        get["identity"]["definition_hash"]
    );
}

#[test]
fn campaign_workspace_discovers_and_authors_reach_objective_through_typed_cli() {
    let temporary = TempWorkspace::new("semantic-put-objective");
    let workspace = temporary.0.to_str().unwrap();
    let campaign = level_one_campaign_fixture();
    let campaign = campaign.to_str().expect("UTF-8 campaign path");
    let objectives = run(&[
        "game",
        "objectives",
        "--campaign-package",
        campaign,
        "--id",
        "objective.reach.my_beacon",
    ]);
    assert!(objectives.status.success(), "{:?}", objectives.stderr);
    let objectives: Value = serde_json::from_slice(&objectives.stdout).unwrap();
    assert_eq!(objectives["data"]["page"]["total"], 1);
    assert_eq!(
        objectives["data"]["objectives"][0],
        serde_json::json!({
            "id": "objective.reach.my_beacon",
            "condition": {"reach_location": {"x": 115, "y": 212}},
            "condition_access": "session_reset"
        })
    );

    let created = run(&[
        "game",
        "workspace-create",
        "--workspace",
        workspace,
        "--workspace-id",
        "semantic-objective-workspace",
        "--campaign-package",
        campaign,
    ]);
    assert!(created.status.success(), "{:?}", created.stderr);
    let changes_path = temporary.0.join("put-objective.json");
    fs::write(
        &changes_path,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "put_objective",
            "objective": "objective.reach.my_beacon",
            "condition": {"reach_location": {"x": 120, "y": 120}},
            "expected_condition": {"reach_location": {"x": 115, "y": 212}}
        }]))
        .unwrap(),
    )
    .unwrap();
    let staged = run(&[
        "game",
        "set",
        "--workspace",
        workspace,
        "--idempotency-key",
        "semantic-objective-apply",
        "--timing",
        "session_reset",
        "--changes",
        changes_path.to_str().unwrap(),
    ]);
    assert!(staged.status.success(), "{:?}", staged.stderr);
    let staged: Value = serde_json::from_slice(&staged.stdout).unwrap();
    let change_set_path = temporary.0.join("objective-change-set.json");
    fs::write(
        &change_set_path,
        serde_json::to_vec(&staged["data"]["change_set"]).unwrap(),
    )
    .unwrap();
    let preview = run(&[
        "game",
        "preview",
        "--workspace",
        workspace,
        "--change-set",
        change_set_path.to_str().unwrap(),
    ]);
    assert!(preview.status.success(), "{:?}", preview.stderr);
    let preview: Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(preview["data"]["validation"]["accepted"], true);
    assert_eq!(
        preview["data"]["diff"],
        serde_json::json!([{
            "id": "objective.reach.my_beacon.condition",
            "before": {"type": "objective_condition", "value": {"reach_location": {"x": 115, "y": 212}}},
            "after": {"type": "objective_condition", "value": {"reach_location": {"x": 120, "y": 120}}}
        }])
    );
    let validation = preview["data"]["validation"]["receipt_hash"]
        .as_str()
        .unwrap();
    let applied = run(&[
        "game",
        "apply",
        "--workspace",
        workspace,
        "--change-set",
        change_set_path.to_str().unwrap(),
        "--validation-receipt",
        validation,
    ]);
    assert!(applied.status.success(), "{:?}", applied.stderr);

    let get = run(&[
        "game",
        "get",
        "--workspace",
        workspace,
        "--id",
        "objective.reach.my_beacon.condition",
    ]);
    assert!(get.status.success(), "{:?}", get.stderr);
    let get: Value = serde_json::from_slice(&get.stdout).unwrap();
    assert_eq!(get["data"]["descriptor"]["kind"], "objective");
    assert_eq!(get["data"]["descriptor"]["access"], "writable");
    assert_eq!(
        get["data"]["descriptor"]["default_value"]["value"],
        serde_json::json!({"reach_location": {"x": 115, "y": 212}})
    );
    assert_eq!(
        get["data"]["value"],
        serde_json::json!({"type": "objective_condition", "value": {"reach_location": {"x": 120, "y": 120}}})
    );
}

#[test]
fn unknown_reads_and_mutation_words_fail_with_typed_non_mutating_errors() {
    let fixture = fixture();
    let fixture = fixture.to_str().expect("UTF-8 fixture path");
    let unknown = run(&[
        "game",
        "get",
        "--package",
        fixture,
        "--id",
        "flight.player.unknown_speed",
    ]);
    assert_eq!(unknown.status.code(), Some(3));
    assert!(unknown.stdout.is_empty());
    let error: Value = serde_json::from_slice(&unknown.stderr).expect("JSON stderr");
    assert_eq!(error["errors"][0]["code"], "UNKNOWN_ID");
    assert_eq!(error["errors"][0]["state_changed"], false);

    for (operation, expected_code) in [
        ("set", "MISSING_WORKSPACE"),
        ("apply", "MISSING_VALIDATION_RECEIPT"),
        ("rollback", "INVALID_REQUEST"),
        ("reset", "SESSION_CONTRACT_NOT_READY"),
    ] {
        let rejected = run(&["game", operation]);
        assert_eq!(rejected.status.code(), Some(2));
        assert!(rejected.stdout.is_empty());
        let error: Value = serde_json::from_slice(&rejected.stderr).expect("JSON stderr");
        assert_eq!(error["errors"][0]["code"], expected_code);
        assert_eq!(error["errors"][0]["state_changed"], false);
    }
}

#[test]
fn describe_rejects_invalid_limits_and_foreign_cursors() {
    let fixture = fixture();
    let fixture = fixture.to_str().expect("UTF-8 fixture path");
    for arguments in [
        vec!["game", "describe", "--package", fixture, "--limit", "0"],
        vec![
            "game",
            "describe",
            "--package",
            fixture,
            "--cursor",
            "v1.not-this-catalog.not-this-definition.2",
        ],
    ] {
        let rejected = run(&arguments);
        assert_eq!(rejected.status.code(), Some(2));
        assert!(rejected.stdout.is_empty());
        let error: Value = serde_json::from_slice(&rejected.stderr).expect("JSON stderr");
        assert_eq!(error["errors"][0]["state_changed"], false);
    }
}

#[test]
fn describe_loads_the_real_owner_definition_through_the_same_cli_contract() {
    let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let export = private.join("generated/level1-export-m");
    let closure = private.join("generated/level1-asset-closure.json");
    let cache = private.join("generated/level1-unity-asset-cache-plan29-final");
    let output = run(&[
        "game",
        "describe",
        "--owner-export",
        export.to_str().unwrap(),
        "--owner-closure",
        closure.to_str().unwrap(),
        "--owner-cache",
        cache.to_str().unwrap(),
        "--id",
        "presentation.spell.fireball.projectile_asset",
    ]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let output: Value = serde_json::from_slice(&output.stdout).expect("JSON stdout");
    assert_eq!(output["identity"]["package_id"], "mc2.owned");
    assert_eq!(
        output["identity"]["provenance"]["distribution"],
        "owner_local"
    );
    assert_eq!(output["data"]["descriptors"].as_array().unwrap().len(), 1);
}

#[test]
fn workspace_authoring_cli_stages_previews_and_refuses_unsafe_asset_timing_without_mutation() {
    let temporary = TempWorkspace::new("f9-unsafe-asset-timing");
    let workspace = temporary.0.to_str().unwrap();
    let fixture = fixture();
    let fixture = fixture.to_str().unwrap();
    let created = run(&[
        "game",
        "workspace-create",
        "--workspace",
        workspace,
        "--workspace-id",
        "cli-workspace",
        "--package",
        fixture,
    ]);
    assert!(created.status.success());
    assert!(created.stderr.is_empty());
    let initial_state = fs::read(temporary.0.join("workspace-state.json")).unwrap();

    let inspect = run(&["game", "inspect", "--workspace", workspace]);
    assert!(inspect.status.success());
    let inspect: Value = serde_json::from_slice(&inspect.stdout).unwrap();
    assert_eq!(
        inspect["data"]["workspace"]["active_definition_revision"],
        1
    );
    assert_eq!(
        inspect["data"]["writable_surface"],
        serde_json::json!([
            "ai.enemy.default.policy",
            "flight.player.max_forward_speed",
            "presentation.spell.fireball.projectile_asset",
            "spell.fireball.mana_cost"
        ])
    );

    let described = run(&[
        "game",
        "describe",
        "--workspace",
        workspace,
        "--id",
        "presentation.spell.fireball.projectile_asset",
    ]);
    assert!(described.status.success());
    let described: Value = serde_json::from_slice(&described.stdout).unwrap();
    assert_eq!(described["data"]["descriptors"][0]["access"], "writable");

    let changes_path = temporary.0.join("changes.json");
    fs::write(
        &changes_path,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "bind_asset",
            "slot": "presentation.spell.fireball.projectile_asset",
            "artifact": "presentation.projectile_asset_variant"
        }]))
        .unwrap(),
    )
    .unwrap();
    let staged = run(&[
        "game",
        "set",
        "--workspace",
        workspace,
        "--idempotency-key",
        "cli-unsafe-asset-timing",
        "--timing",
        "session_reset",
        "--changes",
        changes_path.to_str().unwrap(),
    ]);
    assert!(staged.status.success());
    assert!(staged.stderr.is_empty());
    let staged: Value = serde_json::from_slice(&staged.stdout).unwrap();
    assert_eq!(staged["data"]["published"], false);
    let change_set_path = temporary.0.join("change-set.json");
    fs::write(
        &change_set_path,
        serde_json::to_vec(&staged["data"]["change_set"]).unwrap(),
    )
    .unwrap();

    let preview = run(&[
        "game",
        "preview",
        "--workspace",
        workspace,
        "--change-set",
        change_set_path.to_str().unwrap(),
    ]);
    assert!(preview.status.success());
    assert!(preview.stderr.is_empty());
    let preview: Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(preview["data"]["validation"]["accepted"], false);
    assert_eq!(
        preview["data"]["validation"]["diagnostics"][0]["code"],
        "UNSAFE_TIMING"
    );
    assert_eq!(preview["data"]["apply_request"], Value::Null);
    assert_eq!(
        fs::read(temporary.0.join("workspace-state.json")).unwrap(),
        initial_state
    );

    let validation_hash = preview["data"]["validation"]["receipt_hash"]
        .as_str()
        .unwrap();
    let rejected = run(&[
        "game",
        "apply",
        "--workspace",
        workspace,
        "--change-set",
        change_set_path.to_str().unwrap(),
        "--validation-receipt",
        validation_hash,
    ]);
    assert_eq!(rejected.status.code(), Some(3));
    assert!(rejected.stdout.is_empty());
    let rejected: Value = serde_json::from_slice(&rejected.stderr).unwrap();
    assert_eq!(rejected["errors"][0]["code"], "VALIDATION_REJECTED");
    assert_eq!(rejected["errors"][0]["state_changed"], false);
    assert_eq!(
        fs::read(temporary.0.join("workspace-state.json")).unwrap(),
        initial_state
    );

    let status = run(&[
        "game",
        "apply-status",
        "--workspace",
        workspace,
        "--idempotency-key",
        "cli-unsafe-asset-timing",
    ]);
    assert!(status.status.success());
    let status: Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["data"]["status"], "not_started");
}

#[test]
fn authoring_cli_rejects_session_generation_and_duplicate_workspace_creation() {
    let temporary = TempWorkspace::new("f3-schema");
    let workspace = temporary.0.to_str().unwrap();
    let fixture = fixture();
    let fixture = fixture.to_str().unwrap();
    let create_arguments = [
        "game",
        "workspace-create",
        "--workspace",
        workspace,
        "--workspace-id",
        "schema-workspace",
        "--package",
        fixture,
    ];
    assert!(run(&create_arguments).status.success());
    let duplicate = run(&create_arguments);
    assert_eq!(duplicate.status.code(), Some(3));
    let duplicate: Value = serde_json::from_slice(&duplicate.stderr).unwrap();
    assert_eq!(duplicate["errors"][0]["code"], "WORKSPACE_EXISTS");

    let definition_hash = serde_json::from_slice::<Value>(
        &run(&["game", "inspect", "--workspace", workspace]).stdout,
    )
    .unwrap()["identity"]["definition_hash"]
        .as_str()
        .unwrap()
        .to_owned();
    let invalid_path = temporary.0.join("session-generation.json");
    fs::write(
        &invalid_path,
        serde_json::to_vec(&serde_json::json!({
            "schema_version": "carpet-flight-change-set/v5",
            "target_workspace": "schema-workspace",
            "expected_definition_hash": definition_hash,
            "expected_definition_revision": 1,
            "idempotency_key": "wrong-guard",
            "operations": [{
                "operation": "set_integer",
                "parameter": "flight.player.max_forward_speed",
                "value": 82
            }],
            "requested_timing": "session_reset",
            "intent": null,
            "session_generation": 4
        }))
        .unwrap(),
    )
    .unwrap();
    let rejected = run(&[
        "game",
        "validate",
        "--workspace",
        workspace,
        "--change-set",
        invalid_path.to_str().unwrap(),
    ]);
    assert_eq!(rejected.status.code(), Some(2));
    let rejected: Value = serde_json::from_slice(&rejected.stderr).unwrap();
    assert_eq!(rejected["errors"][0]["code"], "INVALID_REQUEST");
    assert!(
        rejected["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("unknown field `session_generation`")
    );
}

#[test]
fn authoring_cli_applies_reads_back_retries_and_forward_rolls_back_definition_value() {
    let temporary = TempWorkspace::new("f3-positive");
    let workspace = temporary.0.to_str().unwrap();
    let fixture = fixture();
    let fixture = fixture.to_str().unwrap();
    assert!(
        run(&[
            "game",
            "workspace-create",
            "--workspace",
            workspace,
            "--workspace-id",
            "positive-workspace",
            "--package",
            fixture,
        ])
        .status
        .success()
    );
    let changes_path = temporary.0.join("changes.json");
    fs::write(
        &changes_path,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "set_integer",
            "parameter": "flight.player.max_forward_speed",
            "value": 96,
            "expected_value": 80
        }]))
        .unwrap(),
    )
    .unwrap();
    let staged = run(&[
        "game",
        "set",
        "--workspace",
        workspace,
        "--idempotency-key",
        "positive-apply",
        "--timing",
        "session_reset",
        "--changes",
        changes_path.to_str().unwrap(),
        "--intent",
        "prove-definition-publication-only",
    ]);
    assert!(staged.status.success());
    let staged: Value = serde_json::from_slice(&staged.stdout).unwrap();
    let change_set_path = temporary.0.join("change-set.json");
    fs::write(
        &change_set_path,
        serde_json::to_vec(&staged["data"]["change_set"]).unwrap(),
    )
    .unwrap();
    let preview = run(&[
        "game",
        "preview",
        "--workspace",
        workspace,
        "--change-set",
        change_set_path.to_str().unwrap(),
    ]);
    assert!(preview.status.success());
    let preview: Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(preview["data"]["validation"]["accepted"], true);
    assert_eq!(preview["data"]["diff"][0]["before"]["value"], 80);
    assert_eq!(preview["data"]["diff"][0]["after"]["value"], 96);
    let validation_hash = preview["data"]["validation"]["receipt_hash"]
        .as_str()
        .unwrap();
    let apply_arguments = [
        "game",
        "apply",
        "--workspace",
        workspace,
        "--change-set",
        change_set_path.to_str().unwrap(),
        "--validation-receipt",
        validation_hash,
    ];
    let applied = run(&apply_arguments);
    assert!(applied.status.success());
    assert!(applied.stderr.is_empty());
    let applied: Value = serde_json::from_slice(&applied.stdout).unwrap();
    assert_eq!(
        applied["data"]["receipt"]["resulting_definition_revision"],
        2
    );
    let receipt_hash = applied["data"]["receipt"]["receipt_hash"]
        .as_str()
        .unwrap()
        .to_owned();
    let transaction_id = applied["data"]["receipt"]["transaction_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let resulting_hash = applied["data"]["receipt"]["resulting_definition_hash"]
        .as_str()
        .unwrap()
        .to_owned();

    let get = run(&[
        "game",
        "get",
        "--workspace",
        workspace,
        "--id",
        "flight.player.max_forward_speed",
    ]);
    assert!(get.status.success());
    let get: Value = serde_json::from_slice(&get.stdout).unwrap();
    assert_eq!(get["data"]["value"]["value"], 96);
    assert_eq!(get["identity"]["revision"], 2);

    let retry = run(&apply_arguments);
    assert!(retry.status.success());
    let retry: Value = serde_json::from_slice(&retry.stdout).unwrap();
    assert_eq!(retry["data"]["receipt"]["receipt_hash"], receipt_hash);
    let status = run(&[
        "game",
        "apply-status",
        "--workspace",
        workspace,
        "--idempotency-key",
        "positive-apply",
    ]);
    assert!(status.status.success());
    let status: Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["data"]["status"], "committed");
    assert_eq!(status["data"]["receipt"]["receipt_hash"], receipt_hash);

    let rolled = run(&[
        "game",
        "rollback",
        "--workspace",
        workspace,
        "--expected-definition-hash",
        &resulting_hash,
        "--expected-definition-revision",
        "2",
        "--source-transaction",
        &transaction_id,
        "--idempotency-key",
        "positive-rollback",
    ]);
    assert!(rolled.status.success());
    let rolled: Value = serde_json::from_slice(&rolled.stdout).unwrap();
    assert_eq!(rolled["data"]["receipt"]["operation"], "rollback");
    assert_eq!(
        rolled["data"]["receipt"]["resulting_definition_revision"],
        3
    );
    assert_eq!(
        rolled["data"]["receipt"]["mutation_timing"],
        "session_reset"
    );
    assert_eq!(
        rolled["data"]["receipt"]["applied_ids"],
        serde_json::json!(["flight.player.max_forward_speed"])
    );
    let restored = run(&[
        "game",
        "get",
        "--workspace",
        workspace,
        "--id",
        "flight.player.max_forward_speed",
    ]);
    assert!(restored.status.success());
    let restored: Value = serde_json::from_slice(&restored.stdout).unwrap();
    assert_eq!(restored["data"]["value"]["value"], 80);
    assert_eq!(restored["identity"]["revision"], 3);
}

#[test]
fn killed_cli_before_commit_preserves_state_and_reports_not_started() {
    let temporary = TempWorkspace::new("f3-cancel-before");
    let workspace = temporary.0.to_str().unwrap();
    let fixture = fixture();
    let fixture = fixture.to_str().unwrap();
    assert!(
        run(&[
            "game",
            "workspace-create",
            "--workspace",
            workspace,
            "--workspace-id",
            "cancel-before-workspace",
            "--package",
            fixture,
        ])
        .status
        .success()
    );
    let initial_state = fs::read(temporary.0.join("workspace-state.json")).unwrap();
    let changes_path = temporary.0.join("cancel-before-changes.json");
    fs::write(
        &changes_path,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "set_integer",
            "parameter": "flight.player.max_forward_speed",
            "value": 112
        }]))
        .unwrap(),
    )
    .unwrap();
    let staged = run(&[
        "game",
        "set",
        "--workspace",
        workspace,
        "--idempotency-key",
        "cancel-before-apply",
        "--timing",
        "session_reset",
        "--changes",
        changes_path.to_str().unwrap(),
    ]);
    assert!(staged.status.success());
    let staged: Value = serde_json::from_slice(&staged.stdout).unwrap();
    let change_set_path = temporary.0.join("cancel-before-change-set.json");
    fs::write(
        &change_set_path,
        serde_json::to_vec(&staged["data"]["change_set"]).unwrap(),
    )
    .unwrap();
    let preview = run(&[
        "game",
        "preview",
        "--workspace",
        workspace,
        "--change-set",
        change_set_path.to_str().unwrap(),
    ]);
    assert!(preview.status.success());
    let preview: Value = serde_json::from_slice(&preview.stdout).unwrap();
    let validation_hash = preview["data"]["validation"]["receipt_hash"]
        .as_str()
        .unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_sim-cli"))
        .args([
            "game",
            "apply",
            "--workspace",
            workspace,
            "--change-set",
            change_set_path.to_str().unwrap(),
            "--validation-receipt",
            validation_hash,
        ])
        .env(
            "CARPET_FLIGHT_TEST_PUBLICATION_FAULT",
            "pause_before_commit",
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_until("pre-commit staged state", || {
        temporary.0.join(".workspace-lock").is_file() && has_staged_workspace_state(&temporary.0)
    });
    child.kill().unwrap();
    child.wait().unwrap();

    assert_eq!(
        fs::read(temporary.0.join("workspace-state.json")).unwrap(),
        initial_state
    );
    let status = run(&[
        "game",
        "apply-status",
        "--workspace",
        workspace,
        "--idempotency-key",
        "cancel-before-apply",
    ]);
    assert!(status.status.success());
    let status: Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["data"]["status"], "not_started");
    assert_eq!(status["data"]["receipt"], Value::Null);
}

#[test]
fn killed_cli_after_commit_recovers_exact_receipt_and_idempotent_retry() {
    let temporary = TempWorkspace::new("f3-cancel-after");
    let workspace = temporary.0.to_str().unwrap();
    let fixture = fixture();
    let fixture = fixture.to_str().unwrap();
    assert!(
        run(&[
            "game",
            "workspace-create",
            "--workspace",
            workspace,
            "--workspace-id",
            "cancel-after-workspace",
            "--package",
            fixture,
        ])
        .status
        .success()
    );
    let changes_path = temporary.0.join("cancel-after-changes.json");
    fs::write(
        &changes_path,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "set_integer",
            "parameter": "flight.player.max_forward_speed",
            "value": 128
        }]))
        .unwrap(),
    )
    .unwrap();
    let staged = run(&[
        "game",
        "set",
        "--workspace",
        workspace,
        "--idempotency-key",
        "cancel-after-apply",
        "--timing",
        "session_reset",
        "--changes",
        changes_path.to_str().unwrap(),
    ]);
    assert!(staged.status.success());
    let staged: Value = serde_json::from_slice(&staged.stdout).unwrap();
    let change_set_path = temporary.0.join("cancel-after-change-set.json");
    fs::write(
        &change_set_path,
        serde_json::to_vec(&staged["data"]["change_set"]).unwrap(),
    )
    .unwrap();
    let preview = run(&[
        "game",
        "preview",
        "--workspace",
        workspace,
        "--change-set",
        change_set_path.to_str().unwrap(),
    ]);
    assert!(preview.status.success());
    let preview: Value = serde_json::from_slice(&preview.stdout).unwrap();
    let validation_hash = preview["data"]["validation"]["receipt_hash"]
        .as_str()
        .unwrap();
    let apply_arguments = [
        "game",
        "apply",
        "--workspace",
        workspace,
        "--change-set",
        change_set_path.to_str().unwrap(),
        "--validation-receipt",
        validation_hash,
    ];

    let mut child = Command::new(env!("CARGO_BIN_EXE_sim-cli"))
        .args(apply_arguments)
        .env("CARPET_FLIGHT_TEST_PUBLICATION_FAULT", "pause_after_commit")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_until("committed revision", || {
        fs::read(temporary.0.join("workspace-state.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .is_some_and(|state| {
                state["definitions"][state["active_definition_hash"].as_str().unwrap()]["revision"]
                    == 2
            })
    });
    child.kill().unwrap();
    child.wait().unwrap();

    let status = run(&[
        "game",
        "apply-status",
        "--workspace",
        workspace,
        "--idempotency-key",
        "cancel-after-apply",
    ]);
    assert!(status.status.success());
    let status: Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["data"]["status"], "committed");
    assert_eq!(
        status["data"]["receipt"]["resulting_definition_revision"],
        2
    );
    let retry = run(&apply_arguments);
    assert!(retry.status.success());
    let retry: Value = serde_json::from_slice(&retry.stdout).unwrap();
    assert_eq!(retry["data"]["receipt"], status["data"]["receipt"]);
}
