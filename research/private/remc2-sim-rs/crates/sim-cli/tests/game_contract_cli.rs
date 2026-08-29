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
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../sim-definition/tests/fixtures/custom-synthetic-v1")
}

fn level_one_campaign_fixture() -> PathBuf {
    std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-cf15-level1-package-v3-current")
        },
        PathBuf::from,
    )
}

fn level_two_campaign_fixture() -> PathBuf {
    std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
        },
        PathBuf::from,
    )
}

fn level_three_campaign_fixture() -> PathBuf {
    std::env::var_os("MC_CAMPAIGN_PACKAGE_LEVEL3_V1").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3")
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
    assert_eq!(capabilities["contract_version"], "27.0");
    assert_eq!(
        capabilities["data"]["authoring"]["change_set_schema"],
        "carpet-flight-change-set/v6"
    );
    assert_eq!(
        capabilities["data"]["authoring"]["game_definition_schema"],
        "carpet-flight-game-definition/v7"
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
    let set_max_life = capabilities["data"]["authoring"]["operation_schema"]["variants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|variant| variant["operation"] == "set_entity_max_life")
        .expect("set_entity_max_life is discoverable");
    assert_eq!(set_max_life["target_id"], "<entity-id>.max_life");
    assert_eq!(set_max_life["max_life"]["unit"], "life_points");
    assert_eq!(set_max_life["max_life"]["minimum"], 1);
    assert_eq!(
        set_max_life["max_life"]["maximum"],
        "target descriptor -> maximum"
    );
    assert_eq!(
        set_max_life["required_capability_by_creature_kind"],
        serde_json::json!({
            "archer": {
                "id": "engine.creature.archer.max_life",
                "exact_version": 1
            },
            "bee": {
                "id": "engine.creature.bee.max_life",
                "exact_version": 1
            },
            "dragon": {
                "id": "engine.creature.dragon.max_life",
                "exact_version": 1
            },
            "firefly": {
                "id": "engine.creature.firefly.max_life",
                "exact_version": 1
            },
            "goat": {
                "id": "engine.creature.goat.max_life",
                "exact_version": 1
            },
            "skeleton": {
                "id": "engine.creature.skeleton.max_life",
                "exact_version": 1
            },
            "trader": {
                "id": "engine.creature.trader.max_life",
                "exact_version": 1
            },
            "villager": {
                "id": "engine.creature.villager.max_life",
                "exact_version": 1
            }
        })
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
    assert_eq!(
        put_objective["condition"]["shapes"],
        serde_json::json!([
            {
                "reach_location": {"x": "integer", "y": "integer"},
                "unit": "level_tile",
                "bounds": {
                    "x": {"minimum": 0, "maximum": 255},
                    "y": {"minimum": 0, "maximum": 255}
                }
            },
            {
                "castle_mana_percent": "integer",
                "unit": "percent",
                "minimum": 0,
                "maximum": 100
            },
            {
                "no_living_creatures": "semantic_creature_kind",
                "unit": "creature_kind",
                "values": "target descriptor -> bounds_or_enum.values"
            }
        ])
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
    let set_integer = capabilities["data"]["authoring"]["operation_schema"]["variants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|variant| variant["operation"] == "set_integer")
        .expect("set_integer is discoverable");
    assert_eq!(
        set_integer["parameter_ids"],
        serde_json::json!([
            "flight.player.max_forward_speed",
            "spell.fireball.mana_cost"
        ])
    );
    assert_eq!(
        set_integer["parameter_requirements"]["spell.fireball.mana_cost"]["required_capability"],
        serde_json::json!({
            "id": "engine.spell.fireball.mana_cost",
            "exact_version": 1
        })
    );
    assert_eq!(
        capabilities["data"]["compiled_capabilities"]
            .as_array()
            .unwrap()
            .len(),
        17
    );
    let archer_health_capability = capabilities["data"]["compiled_capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capability| capability["id"] == "engine.creature.archer.max_life")
        .expect("Archer maximum-life capability is discoverable");
    assert_eq!(archer_health_capability["version"], 1);
    assert_eq!(archer_health_capability["provider"], "engine_compiled");
    let bee_health_capability = capabilities["data"]["compiled_capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capability| capability["id"] == "engine.creature.bee.max_life")
        .expect("Bee maximum-life capability is discoverable");
    assert_eq!(bee_health_capability["version"], 1);
    assert_eq!(bee_health_capability["provider"], "engine_compiled");
    let dragon_health_capability = capabilities["data"]["compiled_capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capability| capability["id"] == "engine.creature.dragon.max_life")
        .expect("Dragon maximum-life capability is discoverable");
    assert_eq!(dragon_health_capability["version"], 1);
    assert_eq!(dragon_health_capability["provider"], "engine_compiled");
    let firefly_health_capability = capabilities["data"]["compiled_capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capability| capability["id"] == "engine.creature.firefly.max_life")
        .expect("Firefly maximum-life capability is discoverable");
    assert_eq!(firefly_health_capability["version"], 1);
    assert_eq!(firefly_health_capability["provider"], "engine_compiled");
    let goat_health_capability = capabilities["data"]["compiled_capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capability| capability["id"] == "engine.creature.goat.max_life")
        .expect("Goat maximum-life capability is discoverable");
    assert_eq!(goat_health_capability["version"], 1);
    assert_eq!(goat_health_capability["provider"], "engine_compiled");
    let skeleton_health_capability = capabilities["data"]["compiled_capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capability| capability["id"] == "engine.creature.skeleton.max_life")
        .expect("Skeleton maximum-life capability is discoverable");
    assert_eq!(skeleton_health_capability["version"], 1);
    assert_eq!(skeleton_health_capability["provider"], "engine_compiled");
    let trader_health_capability = capabilities["data"]["compiled_capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capability| capability["id"] == "engine.creature.trader.max_life")
        .expect("Trader maximum-life capability is discoverable");
    assert_eq!(trader_health_capability["version"], 1);
    assert_eq!(trader_health_capability["provider"], "engine_compiled");
    let villager_health_capability = capabilities["data"]["compiled_capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capability| capability["id"] == "engine.creature.villager.max_life")
        .expect("Villager maximum-life capability is discoverable");
    assert_eq!(villager_health_capability["version"], 1);
    assert_eq!(villager_health_capability["provider"], "engine_compiled");
    let castle_capability = capabilities["data"]["compiled_capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capability| capability["id"] == "engine.objective.castle_mana_percent")
        .expect("Castle objective capability is discoverable");
    assert_eq!(castle_capability["version"], 1);
    assert_eq!(castle_capability["provider"], "engine_compiled");
    let rival_wizard_capability = capabilities["data"]["compiled_capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capability| capability["id"] == "engine.objective.defeat_rival_wizard")
        .expect("rival-wizard objective capability is discoverable");
    assert_eq!(rival_wizard_capability["version"], 1);
    assert_eq!(rival_wizard_capability["provider"], "engine_compiled");
    let absence_capability = capabilities["data"]["compiled_capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capability| capability["id"] == "engine.objective.no_living_creatures")
        .expect("creature-absence objective capability is discoverable");
    assert_eq!(absence_capability["version"], 1);
    assert_eq!(absence_capability["provider"], "engine_compiled");
    let building_absence_capability = capabilities["data"]["compiled_capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capability| capability["id"] == "engine.objective.no_standing_buildings")
        .expect("standing-building-absence objective capability is discoverable");
    assert_eq!(building_absence_capability["version"], 1);
    assert_eq!(building_absence_capability["provider"], "engine_compiled");
    let fireball_cost_capability = capabilities["data"]["compiled_capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capability| capability["id"] == "engine.spell.fireball.mana_cost")
        .expect("package-bound Fireball cost capability is discoverable");
    assert_eq!(fireball_cost_capability["version"], 1);
    assert_eq!(fireball_cost_capability["provider"], "engine_compiled");
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
    assert_eq!(entities["contract_version"], "27.0");
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
    assert_eq!(public_entity.len(), 5);
    assert!(public_entity.contains_key("id"));
    assert!(public_entity.contains_key("kind"));
    assert!(public_entity.contains_key("position"));
    assert!(public_entity.contains_key("position_access"));
    assert_eq!(public_entity["creature_health"], serde_json::Value::Null);
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

    let goat = run(&[
        "game",
        "entities",
        "--campaign-package",
        campaign,
        "--id",
        "creature.goat.at_110_38",
    ]);
    assert!(goat.status.success(), "{:?}", goat.stderr);
    assert!(goat.stderr.is_empty());
    let goat: Value = serde_json::from_slice(&goat.stdout).unwrap();
    assert_eq!(
        goat["data"]["entities"][0]["position_access"],
        "session_reset"
    );
    assert_eq!(
        goat["data"]["entities"][0]["creature_health"],
        serde_json::json!({
            "default_max_life": 600,
            "max_life": 600,
            "max_life_access": "session_reset"
        })
    );
    let goat_life = run(&[
        "game",
        "describe",
        "--campaign-package",
        campaign,
        "--id",
        "creature.goat.at_110_38.max_life",
    ]);
    assert!(goat_life.status.success(), "{:?}", goat_life.stderr);
    assert!(goat_life.stderr.is_empty());
    let goat_life: Value = serde_json::from_slice(&goat_life.stdout).unwrap();
    assert_eq!(goat_life["data"]["descriptors"][0]["access"], "writable");
    assert_eq!(
        goat_life["data"]["descriptors"][0]["dependencies"][1],
        "engine.creature.goat.max_life@1"
    );

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

    let level_three = level_three_campaign_fixture();
    let level_three = level_three.to_str().expect("UTF-8 campaign path");
    let shield = run(&[
        "game",
        "entities",
        "--campaign-package",
        level_three,
        "--id",
        "spell_pickup.shield.at_85_85",
    ]);
    assert!(shield.status.success(), "{:?}", shield.stderr);
    assert!(shield.stderr.is_empty());
    let shield: Value = serde_json::from_slice(&shield.stdout).unwrap();
    assert_eq!(shield["data"]["page"]["total"], 1);
    assert_eq!(
        shield["data"]["entities"][0]["kind"],
        serde_json::json!({"spell_pickup": "shield"})
    );
    assert_eq!(
        shield["data"]["entities"][0]["position"],
        serde_json::json!({"x": 85, "y": 85})
    );
    assert_eq!(
        shield["data"]["entities"][0]["position_access"],
        "session_reset"
    );
    let shield_position = run(&[
        "game",
        "describe",
        "--campaign-package",
        level_three,
        "--id",
        "spell_pickup.shield.at_85_85.position",
    ]);
    assert!(
        shield_position.status.success(),
        "{:?}",
        shield_position.stderr
    );
    assert!(shield_position.stderr.is_empty());
    let shield_position: Value = serde_json::from_slice(&shield_position.stdout).unwrap();
    let shield_descriptor = &shield_position["data"]["descriptors"][0];
    assert_eq!(shield_descriptor["kind"], "placement");
    assert_eq!(shield_descriptor["access"], "writable");
    assert_eq!(shield_descriptor["mutation_timing"], "session_reset");
    assert_eq!(
        shield_descriptor["effective_value"]["value"],
        serde_json::json!({"x": 85, "y": 85})
    );
}

#[test]
fn move_entity_targets_are_package_projected() {
    fn assert_position_access(package: &str, ids: &[&str], expected: &str) {
        for id in ids {
            let entity = run(&[
                "game",
                "entities",
                "--campaign-package",
                package,
                "--id",
                id,
            ]);
            assert!(entity.status.success(), "{:?}", entity.stderr);
            assert!(entity.stderr.is_empty());
            let entity: Value = serde_json::from_slice(&entity.stdout).unwrap();
            assert_eq!(entity["data"]["page"]["total"], 1);
            assert_eq!(entity["data"]["entities"][0]["id"], *id);
            assert_eq!(entity["data"]["entities"][0]["position_access"], expected);
        }
    }

    let level_two = level_two_campaign_fixture();
    let level_two = level_two.to_str().expect("UTF-8 campaign path");
    assert_position_access(
        level_two,
        &[
            "spell_pickup.shield.at_138_219",
            "spell_pickup.invisibility.at_138_237",
            "spell_pickup.beyond_sight.at_1_152",
            "spell_pickup.speed.at_167_59",
        ],
        "session_reset",
    );
    assert_position_access(
        level_two,
        &[
            "item.experience_scroll.at_129_149",
            "scenery.exit_portal.at_122_188",
            "scenery.stone.at_157_155",
            "scenery.tree.at_163_53",
            "switch.area_entry_trigger.at_2_152",
            "switch.area_exit_trigger.at_2_153",
            "switch.exit_sequence_trigger.at_121_155",
            "creature.bee.at_116_232",
            "creature.worm.at_43_154",
            "creature.archer.at_150_144",
            "creature.skeleton.at_140_220",
            "creature.trader.at_57_155",
            "creature.villager.at_121_180",
            "building.house.at_2_153",
        ],
        "session_reset",
    );
    let scroll_position = run(&[
        "game",
        "describe",
        "--campaign-package",
        level_two,
        "--id",
        "item.experience_scroll.at_129_149.position",
    ]);
    assert!(
        scroll_position.status.success(),
        "{:?}",
        scroll_position.stderr
    );
    assert!(scroll_position.stderr.is_empty());
    let scroll_position: Value = serde_json::from_slice(&scroll_position.stdout).unwrap();
    assert_eq!(
        scroll_position["data"]["descriptors"][0]["kind"],
        "placement"
    );
    assert_eq!(
        scroll_position["data"]["descriptors"][0]["access"],
        "writable"
    );
    assert_eq!(
        scroll_position["data"]["descriptors"][0]["mutation_timing"],
        "session_reset"
    );
    assert_eq!(
        scroll_position["data"]["descriptors"][0]["effective_value"]["value"],
        serde_json::json!({"x": 129, "y": 149})
    );

    let level_three = level_three_campaign_fixture();
    let level_three = level_three.to_str().expect("UTF-8 campaign path");
    assert_position_access(
        level_three,
        &[
            "spell_pickup.shield.at_85_85",
            "spell_pickup.invisibility.at_136_85",
            "spell_pickup.morph.at_110_109",
            "scenery.stone.at_219_93",
            "scenery.tree.at_100_149",
            "switch.area_entry_trigger.at_218_93",
            "switch.exit_sequence_trigger.at_213_220",
            "item.experience_scroll.at_160_107",
            "scenery.exit_portal.at_14_220",
            "creature.dragon.at_107_150",
            "creature.bee.at_108_136",
            "creature.archer.at_30_97",
            "creature.skeleton.at_75_140",
        ],
        "session_reset",
    );
    assert_position_access(
        level_three,
        &[
            "switch.area_entry_trigger.at_110_110",
            "creature.worm.at_128_43",
        ],
        "session_reset",
    );
    let level_one = level_one_campaign_fixture();
    let level_one = level_one.to_str().expect("UTF-8 campaign path");
    assert_position_access(
        level_one,
        &[
            "spell_pickup.castle.at_194_213",
            "spell_pickup.speed.at_202_212",
            "item.experience_scroll.at_216_213",
            "scenery.exit_portal.at_97_221",
            "scenery.stone.at_75_218",
            "scenery.tree.at_110_96",
            "switch.area_exit_trigger.at_189_212",
            "switch.exit_sequence_trigger.at_75_218",
            "creature.archer.at_202_209",
            "creature.firefly.at_213_125",
            "creature.worm.at_110_97",
        ],
        "session_reset",
    );
    // No-Claim: this is the public package-projection gate. The already-focused
    // reset-live Shield, dynamic Castle, Stone, Tree, overlap-sensitive
    // switches, complete-lifecycle grouped/reset-live creatures, authored
    // Worms, Experience Scroll, and Exit Portal authoring tests own
    // publication/reopen/reset and their ordinary consumers; this test does
    // not duplicate those lifecycles or claim other unsupported
    // creature-family placements.
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
    assert_eq!(described["contract_version"], "27.0");
    assert_eq!(
        described["identity"]["schema"],
        "carpet-flight-game-definition/v7"
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
    assert!(created.stderr.is_empty());

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
        "carpet-flight-change-set/v6"
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
fn campaign_workspace_authors_one_firefly_max_life_through_the_typed_cli_lifecycle() {
    const ENTITY: &str = "creature.firefly.at_213_125";
    const TARGET: &str = "creature.firefly.at_213_125.max_life";

    let temporary = TempWorkspace::new("semantic-firefly-max-life");
    let workspace = temporary.0.to_str().unwrap();
    let campaign = level_one_campaign_fixture();
    let campaign = campaign.to_str().expect("UTF-8 campaign path");

    let entity = run(&[
        "game",
        "entities",
        "--campaign-package",
        campaign,
        "--id",
        ENTITY,
    ]);
    assert!(entity.status.success(), "{:?}", entity.stderr);
    assert!(entity.stderr.is_empty());
    let entity: Value = serde_json::from_slice(&entity.stdout).unwrap();
    assert_eq!(
        entity["data"]["entities"][0]["creature_health"],
        serde_json::json!({
            "default_max_life": 600,
            "max_life": 600,
            "max_life_access": "session_reset"
        })
    );

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
    assert_eq!(described["contract_version"], "27.0");
    assert_eq!(descriptor["id"], TARGET);
    assert_eq!(descriptor["kind"], "entity_parameter");
    assert_eq!(descriptor["value_type"], "integer");
    assert_eq!(descriptor["unit"], "life_points");
    assert_eq!(descriptor["access"], "writable");
    assert_eq!(descriptor["mutation_timing"], "session_reset");
    assert_eq!(descriptor["bounds_or_enum"]["minimum"], 1);
    assert_eq!(descriptor["bounds_or_enum"]["maximum"], i32::MAX);
    assert_eq!(descriptor["default_value"]["value"], 600);
    assert_eq!(descriptor["effective_value"]["value"], 600);

    let created = run(&[
        "game",
        "workspace-create",
        "--workspace",
        workspace,
        "--workspace-id",
        "semantic-firefly-max-life-workspace",
        "--campaign-package",
        campaign,
    ]);
    assert!(created.status.success(), "{:?}", created.stderr);
    assert!(created.stderr.is_empty());

    let changes_path = temporary.0.join("set-firefly-max-life.json");
    fs::write(
        &changes_path,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "set_entity_max_life",
            "entity": ENTITY,
            "max_life": 1200,
            "expected_max_life": 600
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
        "semantic-firefly-max-life-apply",
        "--timing",
        "session_reset",
        "--changes",
        changes_path.to_str().unwrap(),
    ]);
    assert!(staged.status.success(), "{:?}", staged.stderr);
    assert!(staged.stderr.is_empty());
    let staged: Value = serde_json::from_slice(&staged.stdout).unwrap();
    assert_eq!(staged["data"]["published"], false);
    assert_eq!(
        staged["data"]["change_set"]["schema_version"],
        "carpet-flight-change-set/v6"
    );
    let change_set_path = temporary.0.join("firefly-max-life-change-set.json");
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
    assert!(validated.stderr.is_empty());
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
    assert!(diff.stderr.is_empty());
    let diff: Value = serde_json::from_slice(&diff.stdout).unwrap();
    assert_eq!(
        diff["data"]["diff"],
        serde_json::json!([{
            "id": TARGET,
            "before": {"type": "integer", "value": 600},
            "after": {"type": "integer", "value": 1200}
        }])
    );

    let applied = run(&[
        "game",
        "apply",
        "--workspace",
        workspace,
        "--change-set",
        change_set_path.to_str().unwrap(),
        "--validation-receipt",
        validated["data"]["validation"]["receipt_hash"]
            .as_str()
            .unwrap(),
    ]);
    assert!(applied.status.success(), "{:?}", applied.stderr);
    assert!(applied.stderr.is_empty());
    let applied: Value = serde_json::from_slice(&applied.stdout).unwrap();
    assert_eq!(
        applied["data"]["receipt"]["applied_ids"],
        serde_json::json!([TARGET])
    );

    let authored = run(&["game", "get", "--workspace", workspace, "--id", TARGET]);
    assert!(authored.status.success(), "{:?}", authored.stderr);
    assert!(authored.stderr.is_empty());
    let authored: Value = serde_json::from_slice(&authored.stdout).unwrap();
    assert_eq!(authored["identity"]["revision"], 2);
    assert_eq!(
        authored["data"]["descriptor"]["default_value"]["value"],
        600
    );
    assert_eq!(
        authored["data"]["descriptor"]["effective_value"]["value"],
        1200
    );
    assert_eq!(
        authored["data"]["value"],
        serde_json::json!({"type": "integer", "value": 1200})
    );

    let committed_state = fs::read(temporary.0.join("workspace-state.json")).unwrap();
    let zero_path = temporary.0.join("zero-firefly-max-life.json");
    fs::write(
        &zero_path,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "set_entity_max_life",
            "entity": ENTITY,
            "max_life": 0,
            "expected_max_life": 1200
        }]))
        .unwrap(),
    )
    .unwrap();
    let rejected = run(&[
        "game",
        "set",
        "--workspace",
        workspace,
        "--idempotency-key",
        "semantic-firefly-max-life-zero",
        "--timing",
        "session_reset",
        "--changes",
        zero_path.to_str().unwrap(),
    ]);
    assert!(rejected.status.success(), "{:?}", rejected.stderr);
    let rejected: Value = serde_json::from_slice(&rejected.stdout).unwrap();
    let rejected_change_set_path = temporary.0.join("zero-firefly-max-life-change-set.json");
    fs::write(
        &rejected_change_set_path,
        serde_json::to_vec(&rejected["data"]["change_set"]).unwrap(),
    )
    .unwrap();
    let rejected = run(&[
        "game",
        "validate",
        "--workspace",
        workspace,
        "--change-set",
        rejected_change_set_path.to_str().unwrap(),
    ]);
    assert!(rejected.status.success(), "{:?}", rejected.stderr);
    let rejected: Value = serde_json::from_slice(&rejected.stdout).unwrap();
    assert_eq!(rejected["data"]["validation"]["accepted"], false);
    assert_eq!(
        rejected["data"]["validation"]["diagnostics"][0]["code"],
        "OUT_OF_RANGE"
    );
    assert_eq!(
        fs::read(temporary.0.join("workspace-state.json")).unwrap(),
        committed_state
    );

    // No-Claim: this public gate proves one real Level 1 Firefly's closed
    // descriptor, CAS publication, deterministic diff/readback, and atomic
    // zero-value rejection. The backend gate separately owns constructor and
    // damage consumption; this does not claim other creature families,
    // projectile travel, save/replay, or Unity editing UI.
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
fn campaign_workspace_authors_level_one_fireball_cost_through_typed_cli() {
    const TARGET: &str = "spell.fireball.mana_cost";

    let temporary = TempWorkspace::new("semantic-fireball-cost");
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
    assert_eq!(described["contract_version"], "27.0");
    assert_eq!(
        described["identity"]["schema"],
        "carpet-flight-game-definition/v7"
    );
    let descriptor = &described["data"]["descriptors"][0];
    assert_eq!(descriptor["id"], TARGET);
    assert_eq!(descriptor["value_type"], "integer");
    assert_eq!(descriptor["unit"], "mana_units");
    assert_eq!(descriptor["access"], "writable");
    assert_eq!(descriptor["mutation_timing"], "session_reset");
    assert_eq!(descriptor["bounds_or_enum"]["minimum"], 0);
    assert_eq!(descriptor["bounds_or_enum"]["maximum"], i32::MAX);
    assert_eq!(descriptor["default_value"]["value"], 100);
    assert_eq!(descriptor["effective_value"]["value"], 100);

    let created = run(&[
        "game",
        "workspace-create",
        "--workspace",
        workspace,
        "--workspace-id",
        "semantic-fireball-cost-workspace",
        "--campaign-package",
        campaign,
    ]);
    assert!(created.status.success(), "{:?}", created.stderr);

    let changes_path = temporary.0.join("set-fireball-cost.json");
    fs::write(
        &changes_path,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "set_integer",
            "parameter": TARGET,
            "value": 200,
            "expected_value": 100
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
        "semantic-fireball-cost-apply",
        "--timing",
        "session_reset",
        "--changes",
        changes_path.to_str().unwrap(),
    ]);
    assert!(staged.status.success(), "{:?}", staged.stderr);
    assert!(staged.stderr.is_empty());
    let staged: Value = serde_json::from_slice(&staged.stdout).unwrap();
    assert_eq!(staged["data"]["published"], false);
    assert_eq!(
        staged["data"]["change_set"]["schema_version"],
        "carpet-flight-change-set/v6"
    );
    let change_set_path = temporary.0.join("fireball-cost-change-set.json");
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
    assert!(validated.stderr.is_empty());
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
    assert!(diff.stderr.is_empty());
    let diff: Value = serde_json::from_slice(&diff.stdout).unwrap();
    assert_eq!(
        diff["data"]["diff"],
        serde_json::json!([{
            "id": TARGET,
            "before": {"type": "integer", "value": 100},
            "after": {"type": "integer", "value": 200}
        }])
    );

    let receipt = validated["data"]["validation"]["receipt_hash"]
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
        receipt,
    ]);
    assert!(applied.status.success(), "{:?}", applied.stderr);
    assert!(applied.stderr.is_empty());
    let applied: Value = serde_json::from_slice(&applied.stdout).unwrap();
    assert_eq!(
        applied["data"]["receipt"]["applied_ids"],
        serde_json::json!([TARGET])
    );

    let authored = run(&["game", "get", "--workspace", workspace, "--id", TARGET]);
    assert!(authored.status.success(), "{:?}", authored.stderr);
    assert!(authored.stderr.is_empty());
    let authored: Value = serde_json::from_slice(&authored.stdout).unwrap();
    assert_eq!(authored["identity"]["revision"], 2);
    assert_eq!(
        authored["data"]["descriptor"]["default_value"]["value"],
        100
    );
    assert_eq!(
        authored["data"]["descriptor"]["effective_value"]["value"],
        200
    );
    assert_eq!(
        authored["data"]["value"],
        serde_json::json!({"type": "integer", "value": 200})
    );

    // No-Claim: this proves real-Level-1 discovery and guarded typed
    // publication/readback. Ordinary affordability, charge, projectile
    // construction, reopen/reset, and Level 2/3 refusal are separate focused
    // Rust gates; no client runtime or presentation behavior is claimed.
}

#[test]
fn campaign_packages_without_a_local_fireball_holder_fail_closed_in_the_typed_cli() {
    const TARGET: &str = "spell.fireball.mana_cost";

    for campaign in [level_two_campaign_fixture(), level_three_campaign_fixture()] {
        let campaign = campaign.to_str().expect("UTF-8 campaign path");
        for command in ["describe", "get"] {
            let output = run(&[
                "game",
                command,
                "--campaign-package",
                campaign,
                "--id",
                TARGET,
            ]);
            assert!(output.status.success(), "{:?}", output.stderr);
            assert!(output.stderr.is_empty());
            let output: Value = serde_json::from_slice(&output.stdout).unwrap();
            let descriptor = if command == "describe" {
                &output["data"]["descriptors"][0]
            } else {
                &output["data"]["descriptor"]
            };
            assert_eq!(descriptor["access"], "read_only");
            assert_eq!(descriptor["default_value"]["value"], 100);
            assert_eq!(descriptor["effective_value"]["value"], 100);
        }
    }

    let temporary = TempWorkspace::new("unsupported-fireball-cost");
    let workspace = temporary.0.to_str().unwrap();
    let campaign = level_two_campaign_fixture();
    let campaign = campaign.to_str().expect("UTF-8 campaign path");
    let created = run(&[
        "game",
        "workspace-create",
        "--workspace",
        workspace,
        "--workspace-id",
        "unsupported-fireball-cost-workspace",
        "--campaign-package",
        campaign,
    ]);
    assert!(created.status.success(), "{:?}", created.stderr);
    assert!(created.stderr.is_empty());

    let inspected = run(&["game", "inspect", "--workspace", workspace]);
    assert!(inspected.status.success(), "{:?}", inspected.stderr);
    assert!(inspected.stderr.is_empty());
    let inspected: Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert!(
        !inspected["data"]["writable_surface"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == TARGET)
    );

    let state_path = temporary.0.join("workspace-state.json");
    let before = fs::read(&state_path).unwrap();
    let changes_path = temporary.0.join("unsupported-fireball-cost.json");
    fs::write(
        &changes_path,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "set_integer",
            "parameter": TARGET,
            "value": 200,
            "expected_value": 100
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
        "reject-unsupported-fireball-cost",
        "--timing",
        "session_reset",
        "--changes",
        changes_path.to_str().unwrap(),
    ]);
    assert!(staged.status.success(), "{:?}", staged.stderr);
    assert!(staged.stderr.is_empty());
    let staged: Value = serde_json::from_slice(&staged.stdout).unwrap();
    let change_set_path = temporary.0.join("unsupported-fireball-change-set.json");
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
    assert!(validated.stderr.is_empty());
    let validated: Value = serde_json::from_slice(&validated.stdout).unwrap();
    assert_eq!(validated["data"]["validation"]["accepted"], false);
    assert_eq!(
        validated["data"]["validation"]["diagnostics"][0]["code"],
        "DEPENDENCY_FAILED"
    );
    assert_eq!(fs::read(&state_path).unwrap(), before);

    let receipt = validated["data"]["validation"]["receipt_hash"]
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
        receipt,
    ]);
    assert_eq!(rejected.status.code(), Some(3));
    assert!(rejected.stdout.is_empty());
    let rejected: Value = serde_json::from_slice(&rejected.stderr).unwrap();
    assert_eq!(rejected["errors"][0]["code"], "VALIDATION_REJECTED");
    assert_eq!(rejected["errors"][0]["state_changed"], false);
    assert_eq!(fs::read(&state_path).unwrap(), before);

    // No-Claim: this proves the package-conditional public descriptor and
    // publication refusal for packages without a local player Fireball
    // holder. It does not add Fireball acquisition to Level 2/3.
}

#[test]
fn campaign_workspace_authors_level_one_castle_mana_objective_through_typed_cli() {
    const OBJECTIVE: &str = "objective.charge.first_castle";
    const TARGET: &str = "objective.charge.first_castle.condition";

    let temporary = TempWorkspace::new("semantic-castle-objective");
    let workspace = temporary.0.to_str().unwrap();
    let campaign = level_one_campaign_fixture();
    let campaign = campaign.to_str().expect("UTF-8 campaign path");

    let objectives = run(&[
        "game",
        "objectives",
        "--campaign-package",
        campaign,
        "--id",
        OBJECTIVE,
    ]);
    assert!(objectives.status.success(), "{:?}", objectives.stderr);
    let objectives: Value = serde_json::from_slice(&objectives.stdout).unwrap();
    assert_eq!(objectives["data"]["page"]["total"], 1);
    assert_eq!(
        objectives["data"]["objectives"][0],
        serde_json::json!({
            "id": OBJECTIVE,
            "condition": {"castle_mana_percent": 15},
            "condition_access": "session_reset"
        })
    );

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
    assert_eq!(described["contract_version"], "27.0");
    assert_eq!(
        described["identity"]["schema"],
        "carpet-flight-game-definition/v7"
    );
    let descriptor = &described["data"]["descriptors"][0];
    assert_eq!(descriptor["id"], TARGET);
    assert_eq!(descriptor["kind"], "objective");
    assert_eq!(descriptor["value_type"], "objective_condition");
    assert_eq!(descriptor["unit"], "percent");
    assert_eq!(descriptor["access"], "writable");
    assert_eq!(descriptor["mutation_timing"], "session_reset");
    assert_eq!(descriptor["bounds_or_enum"]["minimum"], 0);
    assert_eq!(descriptor["bounds_or_enum"]["maximum"], 100);
    assert_eq!(
        descriptor["default_value"]["value"],
        serde_json::json!({"castle_mana_percent": 15})
    );
    assert_eq!(
        descriptor["effective_value"]["value"],
        serde_json::json!({"castle_mana_percent": 15})
    );

    let created = run(&[
        "game",
        "workspace-create",
        "--workspace",
        workspace,
        "--workspace-id",
        "semantic-castle-objective-workspace",
        "--campaign-package",
        campaign,
    ]);
    assert!(created.status.success(), "{:?}", created.stderr);

    let changes_path = temporary.0.join("put-castle-objective.json");
    fs::write(
        &changes_path,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "put_objective",
            "objective": OBJECTIVE,
            "condition": {"castle_mana_percent": 60},
            "expected_condition": {"castle_mana_percent": 15}
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
        "semantic-castle-objective-apply",
        "--timing",
        "session_reset",
        "--changes",
        changes_path.to_str().unwrap(),
    ]);
    assert!(staged.status.success(), "{:?}", staged.stderr);
    let staged: Value = serde_json::from_slice(&staged.stdout).unwrap();
    assert_eq!(
        staged["data"]["change_set"]["schema_version"],
        "carpet-flight-change-set/v6"
    );
    assert_eq!(
        staged["data"]["change_set"]["operations"][0]["operation"],
        "put_objective"
    );
    let change_set_path = temporary.0.join("castle-objective-change-set.json");
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
            "before": {"type": "objective_condition", "value": {"castle_mana_percent": 15}},
            "after": {"type": "objective_condition", "value": {"castle_mana_percent": 60}}
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
    assert_eq!(authored["data"]["descriptor"]["unit"], "percent");
    assert_eq!(
        authored["data"]["descriptor"]["default_value"]["value"],
        serde_json::json!({"castle_mana_percent": 15})
    );
    assert_eq!(
        authored["data"]["descriptor"]["effective_value"]["value"],
        serde_json::json!({"castle_mana_percent": 60})
    );
    assert_eq!(
        authored["data"]["value"],
        serde_json::json!({
            "type": "objective_condition",
            "value": {"castle_mana_percent": 60}
        })
    );

    let committed_state = fs::read(temporary.0.join("workspace-state.json")).unwrap();
    let out_of_range_path = temporary.0.join("castle-objective-out-of-range.json");
    fs::write(
        &out_of_range_path,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "put_objective",
            "objective": OBJECTIVE,
            "condition": {"castle_mana_percent": 101},
            "expected_condition": {"castle_mana_percent": 60}
        }]))
        .unwrap(),
    )
    .unwrap();
    let out_of_range_staged = run(&[
        "game",
        "set",
        "--workspace",
        workspace,
        "--idempotency-key",
        "semantic-castle-objective-out-of-range",
        "--timing",
        "session_reset",
        "--changes",
        out_of_range_path.to_str().unwrap(),
    ]);
    assert!(
        out_of_range_staged.status.success(),
        "{:?}",
        out_of_range_staged.stderr
    );
    let out_of_range_staged: Value = serde_json::from_slice(&out_of_range_staged.stdout).unwrap();
    let rejected_change_set_path = temporary.0.join("castle-objective-rejected.json");
    fs::write(
        &rejected_change_set_path,
        serde_json::to_vec(&out_of_range_staged["data"]["change_set"]).unwrap(),
    )
    .unwrap();
    let rejected_validation = run(&[
        "game",
        "validate",
        "--workspace",
        workspace,
        "--change-set",
        rejected_change_set_path.to_str().unwrap(),
    ]);
    assert!(
        rejected_validation.status.success(),
        "{:?}",
        rejected_validation.stderr
    );
    let rejected_validation: Value = serde_json::from_slice(&rejected_validation.stdout).unwrap();
    assert_eq!(rejected_validation["data"]["validation"]["accepted"], false);
    assert_eq!(
        rejected_validation["data"]["validation"]["diagnostics"][0]["code"],
        "OUT_OF_RANGE"
    );
    assert_eq!(
        fs::read(temporary.0.join("workspace-state.json")).unwrap(),
        committed_state
    );

    let rejected_receipt = rejected_validation["data"]["validation"]["receipt_hash"]
        .as_str()
        .unwrap();
    let rejected_apply = run(&[
        "game",
        "apply",
        "--workspace",
        workspace,
        "--change-set",
        rejected_change_set_path.to_str().unwrap(),
        "--validation-receipt",
        rejected_receipt,
    ]);
    assert_eq!(rejected_apply.status.code(), Some(3));
    assert!(rejected_apply.stdout.is_empty());
    let rejected_apply: Value = serde_json::from_slice(&rejected_apply.stderr).unwrap();
    assert_eq!(rejected_apply["errors"][0]["code"], "VALIDATION_REJECTED");
    assert_eq!(rejected_apply["errors"][0]["state_changed"], false);
    assert_eq!(
        fs::read(temporary.0.join("workspace-state.json")).unwrap(),
        committed_state
    );

    // No-Claim: this proves the real Level-1 authoring transport and guarded
    // publication/readback only. Runtime evaluation, reset/reopen consumption,
    // organic Castle funding, Level 3, the sealed legacy Level1Mission route,
    // and save/replay are owned by separate focused gates.
}

#[test]
fn campaign_workspace_authors_level_one_creature_absence_objective_through_typed_cli() {
    const OBJECTIVE: &str = "objective.clear.spire_archers";
    const TARGET: &str = "objective.clear.spire_archers.condition";

    let temporary = TempWorkspace::new("semantic-creature-absence-objective");
    let workspace = temporary.0.to_str().unwrap();
    let campaign = level_one_campaign_fixture();
    let campaign = campaign.to_str().expect("UTF-8 campaign path");

    let objectives = run(&[
        "game",
        "objectives",
        "--campaign-package",
        campaign,
        "--id",
        OBJECTIVE,
    ]);
    assert!(objectives.status.success(), "{:?}", objectives.stderr);
    let objectives: Value = serde_json::from_slice(&objectives.stdout).unwrap();
    assert_eq!(objectives["data"]["page"]["total"], 1);
    assert_eq!(
        objectives["data"]["objectives"][0],
        serde_json::json!({
            "id": OBJECTIVE,
            "condition": {"no_living_creatures": "archer"},
            "condition_access": "session_reset"
        })
    );

    let described = run(&[
        "game",
        "describe",
        "--campaign-package",
        campaign,
        "--id",
        TARGET,
    ]);
    assert!(described.status.success(), "{:?}", described.stderr);
    let described: Value = serde_json::from_slice(&described.stdout).unwrap();
    assert_eq!(described["contract_version"], "27.0");
    assert_eq!(
        described["identity"]["schema"],
        "carpet-flight-game-definition/v7"
    );
    let descriptor = &described["data"]["descriptors"][0];
    assert_eq!(descriptor["id"], TARGET);
    assert_eq!(descriptor["kind"], "objective");
    assert_eq!(descriptor["value_type"], "objective_condition");
    assert_eq!(descriptor["unit"], "creature_kind");
    assert_eq!(descriptor["access"], "writable");
    assert_eq!(descriptor["mutation_timing"], "session_reset");
    assert_eq!(
        descriptor["bounds_or_enum"]["values"],
        serde_json::json!(["archer", "firefly"])
    );
    assert_eq!(
        descriptor["default_value"]["value"],
        serde_json::json!({"no_living_creatures": "archer"})
    );
    assert_eq!(
        descriptor["effective_value"]["value"],
        serde_json::json!({"no_living_creatures": "archer"})
    );

    let created = run(&[
        "game",
        "workspace-create",
        "--workspace",
        workspace,
        "--workspace-id",
        "semantic-creature-absence-workspace",
        "--campaign-package",
        campaign,
    ]);
    assert!(created.status.success(), "{:?}", created.stderr);

    let changes_path = temporary.0.join("put-creature-absence-objective.json");
    fs::write(
        &changes_path,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "put_objective",
            "objective": OBJECTIVE,
            "condition": {"no_living_creatures": "firefly"},
            "expected_condition": {"no_living_creatures": "archer"}
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
        "semantic-creature-absence-apply",
        "--timing",
        "session_reset",
        "--changes",
        changes_path.to_str().unwrap(),
    ]);
    assert!(staged.status.success(), "{:?}", staged.stderr);
    let staged: Value = serde_json::from_slice(&staged.stdout).unwrap();
    assert_eq!(
        staged["data"]["change_set"]["schema_version"],
        "carpet-flight-change-set/v6"
    );
    let change_set_path = temporary.0.join("creature-absence-change-set.json");
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
            "before": {"type": "objective_condition", "value": {"no_living_creatures": "archer"}},
            "after": {"type": "objective_condition", "value": {"no_living_creatures": "firefly"}}
        }])
    );
    let receipt = validated["data"]["validation"]["receipt_hash"]
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
        receipt,
    ]);
    assert!(applied.status.success(), "{:?}", applied.stderr);
    assert!(applied.stderr.is_empty());
    let applied: Value = serde_json::from_slice(&applied.stdout).unwrap();
    assert_eq!(
        applied["data"]["receipt"]["applied_ids"],
        serde_json::json!([TARGET])
    );
    let authored = run(&["game", "get", "--workspace", workspace, "--id", TARGET]);
    assert!(authored.status.success(), "{:?}", authored.stderr);
    let authored: Value = serde_json::from_slice(&authored.stdout).unwrap();
    assert_eq!(authored["identity"]["revision"], 2);
    assert_eq!(
        authored["data"]["descriptor"]["default_value"]["value"],
        serde_json::json!({"no_living_creatures": "archer"})
    );
    assert_eq!(
        authored["data"]["descriptor"]["effective_value"]["value"],
        serde_json::json!({"no_living_creatures": "firefly"})
    );
    assert_eq!(
        authored["data"]["value"],
        serde_json::json!({
            "type": "objective_condition",
            "value": {"no_living_creatures": "firefly"}
        })
    );

    let committed_state = fs::read(temporary.0.join("workspace-state.json")).unwrap();
    let rejected_cases = [
        (
            "unsupported",
            serde_json::json!({"no_living_creatures": "goat"}),
            serde_json::json!({"no_living_creatures": "firefly"}),
            "OUT_OF_RANGE",
        ),
        (
            "package-absent",
            serde_json::json!({"no_living_creatures": "bee"}),
            serde_json::json!({"no_living_creatures": "firefly"}),
            "OUT_OF_RANGE",
        ),
        (
            "wrong-family",
            serde_json::json!({"castle_mana_percent": 20}),
            serde_json::json!({"no_living_creatures": "firefly"}),
            "TYPE_MISMATCH",
        ),
        (
            "stale-expected",
            serde_json::json!({"no_living_creatures": "archer"}),
            serde_json::json!({"no_living_creatures": "archer"}),
            "CONFLICT",
        ),
    ];
    let mut rejected_apply = None;
    for (label, condition, expected_condition, expected_code) in rejected_cases {
        let path = temporary.0.join(format!("creature-absence-{label}.json"));
        fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!([{
                "operation": "put_objective",
                "objective": OBJECTIVE,
                "condition": condition,
                "expected_condition": expected_condition
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
            &format!("semantic-creature-absence-{label}"),
            "--timing",
            "session_reset",
            "--changes",
            path.to_str().unwrap(),
        ]);
        assert!(staged.status.success(), "{:?}", staged.stderr);
        let staged: Value = serde_json::from_slice(&staged.stdout).unwrap();
        let rejected_path = temporary
            .0
            .join(format!("creature-absence-{label}-set.json"));
        fs::write(
            &rejected_path,
            serde_json::to_vec(&staged["data"]["change_set"]).unwrap(),
        )
        .unwrap();
        let rejected = run(&[
            "game",
            "validate",
            "--workspace",
            workspace,
            "--change-set",
            rejected_path.to_str().unwrap(),
        ]);
        assert!(rejected.status.success(), "{:?}", rejected.stderr);
        let rejected: Value = serde_json::from_slice(&rejected.stdout).unwrap();
        assert_eq!(rejected["data"]["validation"]["accepted"], false);
        assert!(
            rejected["data"]["validation"]["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|diagnostic| diagnostic["code"] == expected_code)
        );
        assert_eq!(
            fs::read(temporary.0.join("workspace-state.json")).unwrap(),
            committed_state
        );
        if label == "unsupported" {
            rejected_apply = Some((
                rejected_path,
                rejected["data"]["validation"]["receipt_hash"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            ));
        }
    }
    let (rejected_path, rejected_receipt) = rejected_apply.unwrap();
    let rejected = run(&[
        "game",
        "apply",
        "--workspace",
        workspace,
        "--change-set",
        rejected_path.to_str().unwrap(),
        "--validation-receipt",
        &rejected_receipt,
    ]);
    assert_eq!(rejected.status.code(), Some(3));
    assert!(rejected.stdout.is_empty());
    let rejected: Value = serde_json::from_slice(&rejected.stderr).unwrap();
    assert_eq!(rejected["errors"][0]["code"], "VALIDATION_REJECTED");
    assert_eq!(rejected["errors"][0]["state_changed"], false);
    assert_eq!(
        fs::read(temporary.0.join("workspace-state.json")).unwrap(),
        committed_state
    );

    // No-Claim: this proves the real Level-1 closed typed transport,
    // package-derived creature enum, CAS, and atomic definition publication.
    // It does not prove organic creature death, runtime evaluation,
    // reopen/reset consumption, other levels, presentation copy, or save/replay.
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
            "presentation.spell.fireball.projectile_asset"
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
fn real_campaign_goat_policy_is_discoverable_and_publishes_through_the_typed_cli() {
    const TARGET: &str = "ai.enemy.default.policy";
    const CLASSIC: &str = "classic_goat_v1";
    const VIGILANT: &str = "vigilant_goat_v1";

    let capabilities = run(&["game", "capabilities"]);
    assert!(capabilities.status.success());
    assert!(capabilities.stderr.is_empty());
    let capabilities: Value = serde_json::from_slice(&capabilities.stdout).unwrap();
    let select_option = capabilities["data"]["authoring"]["operation_schema"]["variants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|variant| variant["operation"] == "select_option")
        .unwrap();
    assert_eq!(
        select_option["required_capability"],
        serde_json::json!({"id": "engine.ai.creature_policy", "exact_version": 1})
    );
    assert_eq!(select_option["mutation_timing"], "session_reset");

    for package in [
        level_one_campaign_fixture(),
        level_two_campaign_fixture(),
        level_three_campaign_fixture(),
    ] {
        let described = run(&[
            "game",
            "describe",
            "--campaign-package",
            package.to_str().unwrap(),
            "--id",
            TARGET,
        ]);
        assert!(described.status.success(), "{:?}", described.stderr);
        assert!(described.stderr.is_empty());
        let described: Value = serde_json::from_slice(&described.stdout).unwrap();
        assert_eq!(described["contract_version"], "27.0");
        let descriptor = &described["data"]["descriptors"][0];
        assert_eq!(descriptor["id"], TARGET);
        assert_eq!(descriptor["kind"], "behavior");
        assert_eq!(descriptor["value_type"], "enum");
        assert_eq!(descriptor["access"], "writable");
        assert_eq!(descriptor["mutation_timing"], "session_reset");
        assert_eq!(
            descriptor["bounds_or_enum"]["values"],
            serde_json::json!([CLASSIC, VIGILANT])
        );
        assert_eq!(descriptor["default_value"]["value"], CLASSIC);
        assert_eq!(descriptor["effective_value"]["value"], CLASSIC);
    }

    let temporary = TempWorkspace::new("real-goat-policy");
    let workspace = temporary.0.to_str().unwrap();
    let level_one = level_one_campaign_fixture();
    let created = run(&[
        "game",
        "workspace-create",
        "--workspace",
        workspace,
        "--workspace-id",
        "real-goat-policy",
        "--campaign-package",
        level_one.to_str().unwrap(),
    ]);
    assert!(created.status.success(), "{:?}", created.stderr);
    assert!(created.stderr.is_empty());

    let changes = temporary.0.join("select-vigilant.json");
    fs::write(
        &changes,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "select_option",
            "parameter": TARGET,
            "option": VIGILANT,
            "expected_option": CLASSIC
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
        "select-real-vigilant-goats",
        "--timing",
        "session_reset",
        "--changes",
        changes.to_str().unwrap(),
    ]);
    assert!(staged.status.success(), "{:?}", staged.stderr);
    assert!(staged.stderr.is_empty());
    let staged: Value = serde_json::from_slice(&staged.stdout).unwrap();
    assert_eq!(staged["data"]["published"], false);
    let change_set = temporary.0.join("vigilant-change-set.json");
    fs::write(
        &change_set,
        serde_json::to_vec(&staged["data"]["change_set"]).unwrap(),
    )
    .unwrap();

    let validated = run(&[
        "game",
        "validate",
        "--workspace",
        workspace,
        "--change-set",
        change_set.to_str().unwrap(),
    ]);
    assert!(validated.status.success(), "{:?}", validated.stderr);
    assert!(validated.stderr.is_empty());
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
        change_set.to_str().unwrap(),
    ]);
    assert!(diff.status.success(), "{:?}", diff.stderr);
    assert!(diff.stderr.is_empty());
    let diff: Value = serde_json::from_slice(&diff.stdout).unwrap();
    assert_eq!(
        diff["data"]["diff"],
        serde_json::json!([{
            "id": TARGET,
            "before": {"type": "enum", "value": CLASSIC},
            "after": {"type": "enum", "value": VIGILANT}
        }])
    );

    let applied = run(&[
        "game",
        "apply",
        "--workspace",
        workspace,
        "--change-set",
        change_set.to_str().unwrap(),
        "--validation-receipt",
        validated["data"]["validation"]["receipt_hash"]
            .as_str()
            .unwrap(),
    ]);
    assert!(applied.status.success(), "{:?}", applied.stderr);
    assert!(applied.stderr.is_empty());
    let applied: Value = serde_json::from_slice(&applied.stdout).unwrap();
    assert_eq!(
        applied["data"]["receipt"]["applied_ids"],
        serde_json::json!([TARGET])
    );

    let authored = run(&["game", "get", "--workspace", workspace, "--id", TARGET]);
    assert!(authored.status.success(), "{:?}", authored.stderr);
    assert!(authored.stderr.is_empty());
    let authored: Value = serde_json::from_slice(&authored.stdout).unwrap();
    assert_eq!(authored["identity"]["revision"], 2);
    assert_eq!(
        authored["data"]["descriptor"]["default_value"]["value"],
        CLASSIC
    );
    assert_eq!(
        authored["data"]["descriptor"]["effective_value"]["value"],
        VIGILANT
    );
    assert_eq!(
        authored["data"]["value"],
        serde_json::json!({"type": "enum", "value": VIGILANT})
    );

    // No-Claim: this public gate proves closed discovery and guarded
    // publication/readback for the qualified Goat policy. The separate
    // sim-core gate owns ordinary scheduler consumption, reopen, and reset.
}

#[test]
fn real_level_one_fireball_asset_is_discoverable_publishable_and_package_scoped() {
    const SLOT: &str = "presentation.spell.fireball.projectile_asset";
    const BASE: &str = "presentation.fireball.projectile.base";
    const FADE: &str = "presentation.fireball.projectile.fade";
    const BASE_SHA: &str =
        "sha256:331df08c5fa8483d5dd75dc10abd9dd76cc26918d6447403b3f3199bc31f521d";
    const FADE_SHA: &str =
        "sha256:61059124cba22dd3086baab36bb7d12a9ab28ebf37d394827a49bea4f6c96784";

    let capabilities = run(&["game", "capabilities"]);
    assert!(capabilities.status.success());
    assert!(capabilities.stderr.is_empty());
    let capabilities: Value = serde_json::from_slice(&capabilities.stdout).unwrap();
    let bind_asset = capabilities["data"]["authoring"]["operation_schema"]["variants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|variant| variant["operation"] == "bind_asset")
        .unwrap();
    assert_eq!(
        bind_asset["required_capability"],
        serde_json::json!({"id": "engine.presentation.indexed_asset", "exact_version": 1})
    );
    assert_eq!(bind_asset["mutation_timing"], "package_reload");

    let level_one = level_one_campaign_fixture();
    let level_one = level_one.to_str().unwrap();
    let described = run(&[
        "game",
        "describe",
        "--campaign-package",
        level_one,
        "--id",
        SLOT,
    ]);
    assert!(described.status.success(), "{:?}", described.stderr);
    assert!(described.stderr.is_empty());
    let described: Value = serde_json::from_slice(&described.stdout).unwrap();
    assert_eq!(described["contract_version"], "27.0");
    let descriptor = &described["data"]["descriptors"][0];
    assert_eq!(descriptor["access"], "writable");
    assert_eq!(descriptor["mutation_timing"], "package_reload");
    assert_eq!(descriptor["effective_artifact_id"], BASE);
    assert_eq!(
        descriptor["compatible_artifacts"],
        serde_json::json!([
            {"artifact_id": BASE, "content_address": BASE_SHA, "kind": "asset", "bytes": 8015},
            {"artifact_id": FADE, "content_address": FADE_SHA, "kind": "asset", "bytes": 8120}
        ])
    );

    for package in [level_two_campaign_fixture(), level_three_campaign_fixture()] {
        let package = package.to_str().unwrap();
        let read_only = run(&[
            "game",
            "describe",
            "--campaign-package",
            package,
            "--id",
            SLOT,
        ]);
        assert!(read_only.status.success(), "{:?}", read_only.stderr);
        assert!(read_only.stderr.is_empty());
        let read_only: Value = serde_json::from_slice(&read_only.stdout).unwrap();
        assert_eq!(read_only["data"]["descriptors"][0]["access"], "read_only");
        assert_eq!(
            read_only["data"]["descriptors"][0]["compatible_artifacts"],
            serde_json::json!([])
        );
        assert_eq!(
            read_only["data"]["descriptors"][0]["effective_artifact_id"],
            Value::Null
        );
    }

    let temporary = TempWorkspace::new("real-fireball-presentation");
    let workspace = temporary.0.to_str().unwrap();
    let created = run(&[
        "game",
        "workspace-create",
        "--workspace",
        workspace,
        "--workspace-id",
        "real-fireball-presentation",
        "--campaign-package",
        level_one,
    ]);
    assert!(created.status.success(), "{:?}", created.stderr);
    assert!(created.stderr.is_empty());
    let changes = temporary.0.join("bind-fade.json");
    fs::write(
        &changes,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "bind_asset",
            "slot": SLOT,
            "artifact": FADE,
            "expected_artifact": BASE
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
        "bind-real-fireball-fade",
        "--timing",
        "package_reload",
        "--changes",
        changes.to_str().unwrap(),
    ]);
    assert!(staged.status.success(), "{:?}", staged.stderr);
    assert!(staged.stderr.is_empty());
    let staged: Value = serde_json::from_slice(&staged.stdout).unwrap();
    assert_eq!(staged["data"]["published"], false);
    let change_set = temporary.0.join("bind-fade-change-set.json");
    fs::write(
        &change_set,
        serde_json::to_vec(&staged["data"]["change_set"]).unwrap(),
    )
    .unwrap();
    let validated = run(&[
        "game",
        "validate",
        "--workspace",
        workspace,
        "--change-set",
        change_set.to_str().unwrap(),
    ]);
    assert!(validated.status.success(), "{:?}", validated.stderr);
    assert!(validated.stderr.is_empty());
    let validated: Value = serde_json::from_slice(&validated.stdout).unwrap();
    assert_eq!(validated["data"]["validation"]["accepted"], true);
    assert_eq!(
        validated["data"]["validation"]["required_timing"],
        "package_reload"
    );
    assert_eq!(
        validated["data"]["validation"]["affected_ids"],
        serde_json::json!([SLOT])
    );
    let diff = run(&[
        "game",
        "diff",
        "--workspace",
        workspace,
        "--change-set",
        change_set.to_str().unwrap(),
    ]);
    assert!(diff.status.success(), "{:?}", diff.stderr);
    assert!(diff.stderr.is_empty());
    let diff: Value = serde_json::from_slice(&diff.stdout).unwrap();
    assert_eq!(diff["data"]["diff"][0]["id"], SLOT);
    assert_eq!(diff["data"]["diff"][0]["before"]["value"], BASE_SHA);
    assert_eq!(diff["data"]["diff"][0]["after"]["value"], FADE_SHA);
    let applied = run(&[
        "game",
        "apply",
        "--workspace",
        workspace,
        "--change-set",
        change_set.to_str().unwrap(),
        "--validation-receipt",
        validated["data"]["validation"]["receipt_hash"]
            .as_str()
            .unwrap(),
    ]);
    assert!(applied.status.success(), "{:?}", applied.stderr);
    assert!(applied.stderr.is_empty());
    let applied: Value = serde_json::from_slice(&applied.stdout).unwrap();
    assert_eq!(
        applied["data"]["receipt"]["applied_ids"],
        serde_json::json!([SLOT])
    );
    let authored = run(&["game", "get", "--workspace", workspace, "--id", SLOT]);
    assert!(authored.status.success(), "{:?}", authored.stderr);
    assert!(authored.stderr.is_empty());
    let authored: Value = serde_json::from_slice(&authored.stdout).unwrap();
    assert_eq!(authored["identity"]["revision"], 2);
    assert_eq!(
        authored["data"]["descriptor"]["effective_artifact_id"],
        FADE
    );
    assert_eq!(authored["data"]["value"]["value"], FADE_SHA);

    let committed = fs::read(temporary.0.join("workspace-state.json")).unwrap();
    let invalid = temporary.0.join("bind-definition-artifact.json");
    fs::write(
        &invalid,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "bind_asset",
            "slot": SLOT,
            "artifact": "campaign.package",
            "expected_artifact": FADE
        }]))
        .unwrap(),
    )
    .unwrap();
    let staged_invalid = run(&[
        "game",
        "set",
        "--workspace",
        workspace,
        "--idempotency-key",
        "reject-non-asset",
        "--timing",
        "package_reload",
        "--changes",
        invalid.to_str().unwrap(),
    ]);
    assert!(staged_invalid.status.success());
    let staged_invalid: Value = serde_json::from_slice(&staged_invalid.stdout).unwrap();
    let invalid_change_set = temporary.0.join("invalid-change-set.json");
    fs::write(
        &invalid_change_set,
        serde_json::to_vec(&staged_invalid["data"]["change_set"]).unwrap(),
    )
    .unwrap();
    let rejected = run(&[
        "game",
        "validate",
        "--workspace",
        workspace,
        "--change-set",
        invalid_change_set.to_str().unwrap(),
    ]);
    assert!(rejected.status.success());
    assert!(rejected.stderr.is_empty());
    let rejected: Value = serde_json::from_slice(&rejected.stdout).unwrap();
    assert_eq!(rejected["data"]["validation"]["accepted"], false);
    assert!(
        rejected["data"]["validation"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "TYPE_MISMATCH")
    );
    assert_eq!(
        fs::read(temporary.0.join("workspace-state.json")).unwrap(),
        committed
    );

    let unavailable = TempWorkspace::new("level-two-fireball-presentation");
    let unavailable_workspace = unavailable.0.to_str().unwrap();
    let level_two = level_two_campaign_fixture();
    let level_two = level_two.to_str().unwrap();
    let created = run(&[
        "game",
        "workspace-create",
        "--workspace",
        unavailable_workspace,
        "--workspace-id",
        "level-two-fireball-presentation",
        "--campaign-package",
        level_two,
    ]);
    assert!(created.status.success(), "{:?}", created.stderr);
    assert!(created.stderr.is_empty());
    let level_two_state = fs::read(unavailable.0.join("workspace-state.json")).unwrap();
    let unavailable_changes = unavailable.0.join("unavailable-asset.json");
    fs::write(
        &unavailable_changes,
        serde_json::to_vec(&serde_json::json!([{
            "operation": "bind_asset",
            "slot": SLOT,
            "artifact": FADE
        }]))
        .unwrap(),
    )
    .unwrap();
    let staged = run(&[
        "game",
        "set",
        "--workspace",
        unavailable_workspace,
        "--idempotency-key",
        "reject-level-two-presentation",
        "--timing",
        "package_reload",
        "--changes",
        unavailable_changes.to_str().unwrap(),
    ]);
    assert!(staged.status.success(), "{:?}", staged.stderr);
    assert!(staged.stderr.is_empty());
    let staged: Value = serde_json::from_slice(&staged.stdout).unwrap();
    let unavailable_change_set = unavailable.0.join("unavailable-change-set.json");
    fs::write(
        &unavailable_change_set,
        serde_json::to_vec(&staged["data"]["change_set"]).unwrap(),
    )
    .unwrap();
    let rejected = run(&[
        "game",
        "validate",
        "--workspace",
        unavailable_workspace,
        "--change-set",
        unavailable_change_set.to_str().unwrap(),
    ]);
    assert!(rejected.status.success());
    assert!(rejected.stderr.is_empty());
    let rejected: Value = serde_json::from_slice(&rejected.stdout).unwrap();
    assert_eq!(rejected["data"]["validation"]["accepted"], false);
    assert!(
        rejected["data"]["validation"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "DEPENDENCY_FAILED")
    );
    assert_eq!(
        fs::read(unavailable.0.join("workspace-state.json")).unwrap(),
        level_two_state
    );
    let rejected_apply = run(&[
        "game",
        "apply",
        "--workspace",
        unavailable_workspace,
        "--change-set",
        unavailable_change_set.to_str().unwrap(),
        "--validation-receipt",
        rejected["data"]["validation"]["receipt_hash"]
            .as_str()
            .unwrap(),
    ]);
    assert_eq!(rejected_apply.status.code(), Some(3));
    assert!(rejected_apply.stdout.is_empty());
    let rejected_apply: Value = serde_json::from_slice(&rejected_apply.stderr).unwrap();
    assert_eq!(rejected_apply["errors"][0]["code"], "VALIDATION_REJECTED");
    assert_eq!(rejected_apply["errors"][0]["state_changed"], false);
    assert_eq!(
        fs::read(unavailable.0.join("workspace-state.json")).unwrap(),
        level_two_state
    );

    // No-Claim: this proves package-derived Level-1 asset discovery,
    // package-reload CAS publication, typed retained-payload readback, and
    // atomic incompatible-artifact refusal. It does not ingest arbitrary
    // files, render in Unity, alter projectile gameplay, animate the sprite,
    // support Level 2/3 assets, save/replay, or establish distribution rights.
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
            "schema_version": "carpet-flight-change-set/v6",
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
