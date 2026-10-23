use super::*;
use crate::render_camera::TerrainSlopeCamera;
use crate::{regression_state::RegressionStateSequence, source_projection_hash};
use std::fs;
use std::path::PathBuf;

const OWNED_LEVEL_ONE_STATE_SEQUENCE: &[u8] = include_bytes!(
    "../../../../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions/level1/sequence-002285FF-00356038.bin"
);

fn fixture_paths() -> (PathBuf, PathBuf) {
    let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    (
        private.join("generated/level1-export-m"),
        private.join("generated/level1-asset-closure.json"),
    )
}

pub(super) fn level_two_fixture_path() -> PathBuf {
    std::env::var_os("MC_LEVEL2_PACKAGE_V5").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/l2-debug3.fJ3PaM/level2")
        },
        PathBuf::from,
    )
}

fn level_two_campaign_fixture_path() -> PathBuf {
    std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
        },
        PathBuf::from,
    )
}

/// Open the production descriptor-backed fixture for gameplay tests that
/// require semantic authored health, item rewards, or switch configuration.
/// Raw reset/profile/hash inspection tests continue to use the export fixture.
pub(super) fn level_two_campaign_session() -> Level2ResetSession {
    crate::campaign::CampaignSession::open(
        level_two_campaign_fixture_path(),
        crate::campaign::spec::CampaignSelection::LEVEL2,
    )
    .expect("qualified Level 2 semantic campaign fixture")
    .level2_compatibility()
    .clone()
}

/// Keep explicit gameplay-cache fixtures on the same production descriptor
/// path while preserving their authored BUILD footprints.
fn level_two_campaign_session_with_owner_cache(
    owner_cache: &std::path::Path,
) -> Level2ResetSession {
    crate::campaign::CampaignSession::open_with_owner_cache(
        level_two_campaign_fixture_path(),
        owner_cache,
        crate::campaign::spec::CampaignSelection::LEVEL2,
    )
    .expect("qualified Level 2 semantic campaign fixture with owner cache")
    .level2_compatibility()
    .clone()
}

/// Fire one real reset-imported model-32 producer through its source
/// objective relation. Tests that need an authored group must advance the
/// package's producer; a live class-11 row alone is not an activation event.
fn fire_level_two_model32_group(session: &mut Level2ResetSession, group: u16) -> u16 {
    let (source_slot, stage_index) = session
        .economy
        .entities()
        .iter()
        .find(|entity| {
            entity.active
                && entity.class == 11
                && entity.model == 32
                && entity.action == 32
                && entity.source_id == group
                && !entity.drawing_disabled()
        })
        .map(|entity| {
            (
                entity.index,
                usize::try_from(entity.level_gate).expect("model-32 objective stage index"),
            )
        })
        .unwrap_or_else(|| {
            panic!("qualified package has no live model-32 producer for group {group}")
        });
    for objective_index in 0..session.runtime.objectives.len() {
        session
            .runtime
            .set_level2_objective_stage_selector_for(objective_index, stage_index, 2);
    }
    let evidence = session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        evidence.len(),
        1,
        "group {group} producer failed: {:?}",
        session.dispatch_events()
    );
    assert!(
        session.dispatch_events().iter().any(|event| matches!(
            event,
            Level2DispatchEvent::AuthoredGroupActivated {
                source_slot: event_source_slot,
                group: event_group,
                ..
            } if *event_source_slot == source_slot && *event_group == group
        )),
        "group {group} source {source_slot} stage {stage_index} did not fire (stage_count {}, players {:?}, objective states {:?}, source_after {:?}, bees {}, activated {:?}): {:?}",
        session.runtime.stage_index,
        session
            .players()
            .iter()
            .map(|player| (player.active_player_index, player.player_color_index()))
            .collect::<Vec<_>>(),
        session
            .runtime
            .objectives
            .iter()
            .map(|objective| objective.stage_selectors[stage_index])
            .collect::<Vec<_>>(),
        session
            .economy
            .entities()
            .get(usize::from(source_slot))
            .map(|source| (
                source.active,
                source.class,
                source.model,
                source.action,
                source.level_gate,
                source.source_id,
                source.phase,
                source.drawing_disabled()
            )),
        session.economy.bee_creatures().len(),
        session.level2_bees_activated,
        session.dispatch_events()
    );
    source_slot
}

fn level_two_v3_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated/mc2-l2-native2.Ibpz9b/out")
}

fn level_two_v4_fixture_path() -> PathBuf {
    std::env::var_os("MC_LEVEL2_PACKAGE_V4").map_or_else(
        || {
            let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
            let migrated = generated.join("l2-debug3.fJ3PaM-v4/level2");
            // During the recoverable artifact migration the current v4
            // package remains the real inspection fixture until it is
            // moved to the explicit historical path.
            if migrated.exists() {
                migrated
            } else {
                generated.join("l2-debug3.fJ3PaM/level2")
            }
        },
        PathBuf::from,
    )
}

fn level_two_gameplay_cache_path() -> PathBuf {
    std::env::var_os("MC_LEVEL2_GAMEPLAY_CACHE_V5").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/level2-day-owner-cache/components/gameplay")
        },
        PathBuf::from,
    )
}

fn refresh_level_two_v4_package_hash(manifest: &mut Value, package: &Path) {
    let mut identity = Vec::new();
    for (name, expected_size) in LEVEL2_V4_PACKAGE_MEMBERS {
        let bytes = fs::read(package.join(name)).unwrap();
        assert_eq!(bytes.len(), expected_size);
        let hash = sha256_hex(&bytes);
        identity.extend_from_slice(name.as_bytes());
        identity.push(0);
        identity.extend_from_slice(hash.as_bytes());
        identity.push(0);
        identity.extend_from_slice(expected_size.to_string().as_bytes());
        identity.push(b'\n');
    }
    manifest["package_sha256"] = json!(sha256_hex(&identity));
}

fn tamper_level_two_v4_selector_64_and_refresh_manifest(package: &Path) {
    let mask_path = package.join("collision-masks.bin");
    let mut bytes = fs::read(&mask_path).unwrap();
    let count = read_u32(&bytes, 12, "collision-mask count").unwrap();
    let mut cursor = 20;
    let mut row_index = None;
    for index in 0..count as usize {
        let selector = read_u16(&bytes, cursor, "collision-mask selector").unwrap();
        let pixel_bytes =
            usize::try_from(read_u32(&bytes, cursor + 8, "collision-mask pixel bytes").unwrap())
                .unwrap();
        let payload_start = cursor + 12;
        let payload_end = payload_start + pixel_bytes;
        if selector == 64 {
            bytes[payload_end - 1] ^= 1;
            row_index = Some((index, payload_start, payload_end));
            break;
        }
        cursor = payload_end;
    }
    let (row_index, payload_start, payload_end) =
        row_index.expect("qualified v4 bank must contain selector 64");
    fs::write(&mask_path, &bytes).unwrap();

    let manifest_path = package.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["collision_masks"]["selectors"][row_index]["sha256"] =
        json!(sha256_hex(&bytes[payload_start..payload_end]));
    manifest["files"]["collision-masks.bin"]["bytes"] = json!(bytes.len());
    manifest["files"]["collision-masks.bin"]["sha256"] = json!(sha256_hex(&bytes));
    refresh_level_two_v4_package_hash(&mut manifest, package);
    fs::write(manifest_path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
}

const LEVEL2_GROUP31_AUTHORED_INDICES: [u16; 15] = [
    209, 244, 260, 261, 262, 264, 265, 266, 267, 268, 269, 270, 271, 272, 274,
];
const LEVEL2_GROUP31_TREE_INDICES: [u16; 12] =
    [260, 261, 262, 264, 265, 266, 267, 268, 269, 270, 271, 272];

fn level_two_switch_turns_until_cadence(phase: u8) -> usize {
    1 + usize::from((8_u8.wrapping_sub(phase & 7)) & 7)
}

fn level_two_reach_row42(
    session: &mut Level2ResetSession,
    local_player: u16,
    row11_position: Position,
    row42_position: Position,
) {
    session
        .set_level2_entity_position(local_player, row11_position)
        .expect("public player-position input for row 11");
    session.step_outer_turn(PlayerCommand::default(), 0);
    if !session
        .economy()
        .authored_switch_producers()
        .iter()
        .any(|source| source.authored_index == 42)
    {
        let row41_phase = session
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|source| source.authored_index == 41)
            .map(|source| source.phase)
            .expect("row 41 producer remains live before its cadence");
        let mut row41_exit_position = row42_position;
        row41_exit_position.x = row41_exit_position.x.wrapping_add(0x8000);
        for _ in 0..level_two_switch_turns_until_cadence(row41_phase) {
            session
                .set_level2_entity_position(local_player, row41_exit_position)
                .expect("public player-position input leaves the row-41 inverse-overlap switch");
            session.step_outer_turn(PlayerCommand::default(), 0);
        }
    }
    session
        .set_level2_entity_position(local_player, row42_position)
        .expect("public player-position input for row 42");
}

fn level_two_model76_switch25_activation_turns(phase: u8) -> usize {
    1 + usize::from((8_u8.wrapping_sub(phase & 7)) & 7)
}

fn acquire_level_two_beyond_sight_from_ordinary_route(
    session: &mut Level2ResetSession,
) -> SpellHolderAcquisitionOutcome {
    let source_position = session.shared_spells().pickups()[0].position;
    session
        .set_level2_entity_position(session.state.entity_index, source_position)
        .expect("public player-position input for family-12 source");
    for _ in 0..2 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        if let Some(outcome) = session
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Level2DispatchEvent::SpellAcquisition {
                    family: 12,
                    result: Some(outcome),
                    ..
                } => Some(*outcome),
                _ => None,
            })
        {
            return outcome;
        }
    }
    panic!(
        "ordinary family-12 source did not acquire after its source cadence: {:?}",
        session.dispatch_events()
    );
}

fn level_two_acquired_family12_menu_session() -> (Level2ResetSession, u16) {
    let mut session = level_two_campaign_session();
    let source_slot = acquire_level_two_beyond_sight_from_ordinary_route(&mut session).source_slot;
    assert_eq!(
        session.shared_spells().local_player.enabled_spell_holders[12],
        source_slot
    );
    assert!(
        session
            .context
            .spell_fire
            .as_ref()
            .expect("spell context after acquisition")
            .holders
            .iter()
            .any(|holder| holder.entity_index == source_slot)
    );

    (session, source_slot)
}

const LEVEL2_SCHEDULER_RNG_ROWS: [(&str, &str, &str); 11] = [
    (
        "RNG-L2-0464",
        "remc2/engine/EventsFunctions.cpp:62474",
        "AddTree02_00_64E20",
    ),
    (
        "RNG-L2-0465",
        "remc2/engine/EventsFunctions.cpp:62475",
        "AddTree02_00_64E20",
    ),
    (
        "RNG-L2-0477",
        "remc2/engine/Events.cpp:420",
        "ApplyEvents_498A0",
    ),
    (
        "RNG-L2-0478",
        "remc2/engine/Events.cpp:578",
        "NewEvent_4A050",
    ),
    (
        "RNG-L2-0479",
        "remc2/engine/Events.cpp:604",
        "NewEvent_4A050",
    ),
    (
        "RNG-L2-0480",
        "remc2/engine/Terrain.cpp:22",
        "GenerateLevelMap_43830",
    ),
    ("RNG-L2-0489", "remc2/engine/Basic.cpp:2997", "file_scope"),
    ("RNG-L2-0490", "remc2/engine/Basic.cpp:3035", "file_scope"),
    ("RNG-L2-0491", "remc2/engine/Basic.cpp:3190", "file_scope"),
    ("RNG-L2-0492", "remc2/engine/Basic.cpp:3228", "file_scope"),
    (
        "RNG-L2-0493",
        "remc2/engine/Sound.cpp:6333",
        "PrepareEventSound_6E450",
    ),
];

fn configure_level2_scheduler_audio_rows(session: &mut Level2ResetSession) {
    let selected = [39_u16, 40, 42];
    for mut goat in session.goats().to_vec() {
        if selected.contains(&goat.entity_index) {
            continue;
        }
        // Action 0 is a real, still-unclosed class-5 callback. Use a compiled
        // action-9 row whose local RNG seed is discriminatingly non-audio so
        // the registry cannot be bypassed by a synthetic no-op selector.
        goat.action = 9;
        goat.pending_damage = 0;
        goat.pending_attacker = 0;
        goat.current_attacker = 0;
        goat.partner_entity_index = None;
        goat.target_entity_index = None;
        goat.actual_speed = 0;
        goat.rng = 0;
        goat.phase = 1;
        goat.proximity_countdown = 0;
        goat.proximity_subcounter = 0;
        assert!(session.goats.replace_goat_for_test(goat));
    }

    let mut action_11 = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 39)
        .cloned()
        .expect("qualified action-11 Goat");
    action_11.action = 11;
    action_11.partner_entity_index = None;
    action_11.target_entity_index = None;
    action_11.pending_damage = 0;
    action_11.pending_attacker = 0;
    action_11.current_attacker = 0;
    action_11.rng = 129;
    action_11.phase = 0;
    assert!(session.goats.replace_goat_for_test(action_11));

    let mut action_9 = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 40)
        .cloned()
        .expect("qualified action-9 Goat");
    action_9.action = 9;
    action_9.partner_entity_index = None;
    action_9.target_entity_index = None;
    action_9.pending_damage = 0;
    action_9.pending_attacker = 0;
    action_9.current_attacker = 0;
    action_9.actual_speed = 0;
    action_9.rng = 129;
    action_9.phase = 1;
    action_9.proximity_countdown = 0;
    action_9.proximity_subcounter = 0;
    assert!(session.goats.replace_goat_for_test(action_9));

    let player_entity_index = session.state.entity_index;
    let mut action_14 = session
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 42)
        .cloned()
        .expect("qualified action-14 Goat");
    action_14.action = 14;
    action_14.partner_entity_index = None;
    action_14.target_entity_index = Some(player_entity_index);
    action_14.pending_damage = 0;
    action_14.pending_attacker = 0;
    action_14.current_attacker = 0;
    action_14.rng = 62;
    action_14.phase = 1;
    let hit_position = action_14.position;
    assert!(session.goats.replace_goat_for_test(action_14));
    session.state.position = hit_position;
    session.economy.entities[usize::from(player_entity_index)].position = hit_position;
    session
        .submit_goat_damage(42, 1, player_entity_index)
        .expect("qualified action-14 audio branch");
}

fn level2_scheduler_bleat_slots(session: &Level2ResetSession) -> Vec<u16> {
    session
        .dispatch_events()
        .iter()
        .filter_map(|event| match event {
            Level2DispatchEvent::Goat {
                source_slot,
                events,
                ..
            } if events
                .iter()
                .any(|event| matches!(event, GoatStepEvent::Bleat { sound_id: 46, .. })) =>
            {
                Some(*source_slot)
            }
            _ => None,
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
enum Mutation {
    ManifestSchema,
    EntityMagic,
    ResetMapping,
}

#[derive(Clone, Copy, Debug)]
enum ClosureMutation {
    Hash,
    PackageBinding,
    Unresolved,
    AssetOrdinal,
}

struct TempPackage {
    root: PathBuf,
    package: PathBuf,
    closure: PathBuf,
}

struct TempLevel2V4Package {
    root: PathBuf,
    package: PathBuf,
}

impl TempLevel2V4Package {
    fn copy_from(package: &Path) -> Self {
        let root = std::env::temp_dir().join(format!(
            "mc2-native-world-v4-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        let output = root.join("package");
        fs::create_dir_all(&output).unwrap();
        for entry in fs::read_dir(package).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                fs::copy(entry.path(), output.join(entry.file_name())).unwrap();
            }
        }
        Self {
            root,
            package: output,
        }
    }
}

impl TempPackage {
    fn copy_from(package: &Path, closure: &Path) -> Self {
        let root = std::env::temp_dir().join(format!(
            "mc2-native-world-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        let output = root.join("package");
        fs::create_dir_all(&output).unwrap();
        fs::copy(package.join("manifest.json"), output.join("manifest.json")).unwrap();
        for (name, _) in PACKAGE_MEMBERS {
            fs::copy(package.join(name), output.join(name)).unwrap();
        }
        let closure_output = root.join("closure.json");
        fs::copy(closure, &closure_output).unwrap();
        Self {
            root,
            package: output,
            closure: closure_output,
        }
    }

    fn mutate(&self, mutation: Mutation) {
        match mutation {
            Mutation::ManifestSchema | Mutation::ResetMapping => {
                let path = self.package.join("manifest.json");
                let mut manifest: Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                if matches!(mutation, Mutation::ManifestSchema) {
                    manifest["schema"] = json!("mc2-owner-level-export/v999");
                } else {
                    manifest["reset_projection"]["field_mappings"]["ambient_loop_31_distance"]["offset"] =
                        json!(560);
                }
                fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
            }
            Mutation::EntityMagic => self.flip_byte("entities.bin", 0),
        }
    }

    fn mutate_closure(&self, mutation: ClosureMutation) {
        let mut closure: Value = serde_json::from_slice(&fs::read(&self.closure).unwrap()).unwrap();
        match mutation {
            ClosureMutation::Hash => closure["closure_sha256"] = json!("0".repeat(64)),
            ClosureMutation::PackageBinding => {
                closure["source"]["level_export"]["package_sha256"] = json!("0".repeat(64));
            }
            ClosureMutation::Unresolved => {
                closure["sets"]["frame_used"]["unresolved"] = json!(["not-resolved"]);
            }
            ClosureMutation::AssetOrdinal => {
                closure["sets"]["frame_used"]["assets"][0]["trace_event_ordinal"] = json!(2);
            }
        }
        if !matches!(mutation, ClosureMutation::Hash) {
            closure.as_object_mut().unwrap().remove("closure_sha256");
            closure["closure_sha256"] =
                json!(sha256_hex(canonical_json(&closure).unwrap().as_bytes()));
        }
        fs::write(&self.closure, serde_json::to_vec_pretty(&closure).unwrap()).unwrap();
    }

    fn flip_byte(&self, name: &str, offset: usize) {
        let path = self.package.join(name);
        let mut bytes = fs::read(&path).unwrap();
        bytes[offset] ^= 0xff;
        fs::write(path, bytes).unwrap();
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Drop for TempLevel2V4Package {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos()
}

include!("economy_slots.rs");
include!("switches_groups.rs");
include!("houses.rs");
include!("scheduler_rng.rs");
include!("dispatch_events.rs");
include!("reset_hash.rs");
include!("misc.rs");
