use atlas_registry::AtlasRegistry;
mod game_authoring;
mod game_contract;
mod game_session;

use sim_core::legacy_terrain::LegacyTerrain;
use sim_core::level1_mission::{
    Level1FeatureOracleBytes, Level1ResetSourceSlot,
    compare_level1_reset_from_authored_with_feature_oracle, level1_reset_harness_error_json,
    level1_reset_production_report_json,
};
use sim_core::native_world::NativeWorld;
use sim_core::recording::{InputRecording, RecordedPlayer};
use sim_core::regression_state::RegressionStateSequence;
use sim_core::{
    FlightContext, FlightProfile, FlightState, FlightWorld, PlayerCommand, Position, Tile,
    latch_command, state_hash, step_entity_pass_with_context, step_turn,
};
use std::process::{Command, ExitCode};
use std::time::Instant;

struct FlatWorld;

struct Level1FeatureOracleOwned {
    authored: Vec<u8>,
    pre_terrain: Vec<u8>,
    feature_start: Vec<u8>,
    construction: Vec<u8>,
    post_terrain: Vec<u8>,
    feature_end: Vec<u8>,
    entities: Vec<u8>,
    allocator: Vec<u8>,
}

impl Level1FeatureOracleOwned {
    fn read(root: &std::path::Path) -> Result<Self, String> {
        let read = |name| {
            std::fs::read(root.join(name))
                .map_err(|error| format!("read feature oracle {name}: {error}"))
        };
        Ok(Self {
            authored: read("authored-level.bin")?,
            pre_terrain: read("terrain-pre-features.bin")?,
            feature_start: read("feature-start.bin")?,
            construction: read("feature-construction-inputs.bin")?,
            post_terrain: read("terrain-post-features.bin")?,
            feature_end: read("feature-end.bin")?,
            entities: read("entities-post-features.bin")?,
            allocator: read("feature-allocator.bin")?,
        })
    }

    fn as_bytes(&self) -> Level1FeatureOracleBytes<'_> {
        Level1FeatureOracleBytes {
            authored: &self.authored,
            pre_terrain: &self.pre_terrain,
            feature_start: &self.feature_start,
            construction: &self.construction,
            post_terrain: &self.post_terrain,
            feature_end: &self.feature_end,
            entities: &self.entities,
            allocator: &self.allocator,
        }
    }
}

impl FlightWorld for FlatWorld {
    fn ground_height(&self, _position: Position) -> i16 {
        0
    }
    fn ceiling_height(&self, _position: Position) -> Option<i16> {
        None
    }
    fn material_mask(&self, _position: Position) -> u32 {
        0
    }
    fn angle_flags(&self, _tile: Tile) -> u8 {
        0
    }
}

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("atlas-check") => atlas_check(),
        Some("conformance-validate") => conformance_validate(),
        Some("inspect-recording") => inspect_recording(),
        Some("inspect-regression") => inspect_regression(),
        Some("inspect-forward") => inspect_forward(),
        Some("inspect-idle-diff") => inspect_idle_diff(),
        Some("classic-hd-qualify") => classic_hd_qualify(),
        Some("find-castle-site") => find_castle_site(),
        Some("reset-from-authored") => reset_from_authored(),
        Some("inspect-level2") => inspect_level2(),
        Some("game") => game_contract::run(),
        Some("trace") => trace(),
        _ => {
            eprintln!(
                "usage: sim-cli <atlas-check|classic-hd-qualify|conformance-validate|find-castle-site|game|inspect-forward|inspect-idle-diff|inspect-level2|inspect-recording|inspect-regression|reset-from-authored|trace> ..."
            );
            ExitCode::from(2)
        }
    }
}

fn inspect_level2() -> ExitCode {
    let arguments = std::env::args().collect::<Vec<_>>();
    if arguments.len() != 3 {
        eprintln!("usage: sim-cli inspect-level2 <package>");
        return ExitCode::from(2);
    }
    let inspection = match NativeWorld::inspect_level2(&arguments[2]) {
        Ok(inspection) => inspection,
        Err(error) => {
            eprintln!(
                "{{\"ok\":false,\"error\":\"{}\"}}",
                json_escape(&error.to_string())
            );
            return ExitCode::FAILURE;
        }
    };
    let profile = match inspection.mission_content_profile() {
        Ok(profile) => profile,
        Err(error) => {
            eprintln!(
                "{{\"ok\":false,\"error\":\"{}\"}}",
                json_escape(&error.to_string())
            );
            return ExitCode::FAILURE;
        }
    };
    let active_entities = inspection
        .entities()
        .iter()
        .filter(|entity| entity.active)
        .count();
    let players = inspection
        .players()
        .iter()
        .map(|player| {
            format!(
                "{{\"slot\":{},\"is_ai\":{},\"entity_index\":{},\"active_player_index\":{},\"left_spell_slot_raw\":{},\"right_spell_slot_raw\":{}}}",
                player.slot,
                player.is_ai,
                player.entity_index,
                player.active_player_index,
                player.left_spell_slot_raw,
                player.right_spell_slot_raw
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let objectives = inspection
        .runtime()
        .objectives
        .iter()
        .enumerate()
        .map(|(slot, objective)| {
            format!(
                "{{\"slot\":{},\"is_level_end\":{},\"text_selector\":{},\"done\":{},\"stage_selectors\":{:?}}}",
                slot, objective.is_level_end, objective.text_selector, objective.done, objective.stage_selectors
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "{{\"ok\":true,\"external_level\":{},\"internal_level\":{},\"map_type\":{},\"profile\":\"level2\",\"entity_count\":{},\"active_entity_count\":{},\"player_count\":{},\"objective_count\":{},\"players\":[{}],\"objectives\":[{}]}}",
        profile.external_level,
        profile.internal_level,
        profile.map_type_id,
        inspection.entities().len(),
        active_entities,
        inspection.players().len(),
        inspection.runtime().objectives.len(),
        players,
        objectives
    );
    ExitCode::SUCCESS
}

fn classic_hd_qualify() -> ExitCode {
    use sim_core::classic_hd::{ClassicHdRenderProfile, byte_hash};
    use sim_core::classic_hd_raster::CLASSIC_HD_TABLESN_BYTES;
    use sim_core::classic_hd_render_graph::{
        ClassicHdBlurInput, ClassicHdGraphInput, ClassicHdLightingPath, ClassicHdRenderGraph,
        ClassicHdWorldReceipt,
    };

    let profiles = [
        ("classic-hd-640x480", ClassicHdRenderProfile::hd_640x480()),
        ("classic-hd-960x720", ClassicHdRenderProfile::hd_960x720()),
        ("classic-hd-1280x960", ClassicHdRenderProfile::hd_1280x960()),
    ];
    let mut reports = Vec::with_capacity(profiles.len());
    for (name, profile) in profiles {
        let mut graph = match ClassicHdRenderGraph::new(profile.clone(), 7) {
            Ok(graph) => graph,
            Err(error) => {
                eprintln!("Classic HD qualification could not create {name}: {error}");
                return ExitCode::FAILURE;
            }
        };
        // Lighting consumes the 256x256 source table; temporal/spatial
        // TABLESN kernels additionally address the blend-table region.
        let tablesn = vec![
            0_u8;
            sim_core::indexed_postprocess::TABLESN_REQUIRED_BYTES
                .max(CLASSIC_HD_TABLESN_BYTES)
        ];
        let shading = vec![0_u8; CLASSIC_HD_TABLESN_BYTES];
        let mut samples = Vec::with_capacity(32);
        let mut first_hash = 0_u64;
        let mut first_provenance_hash = 0_u64;
        let mut last_hash = 0_u64;
        let mut render_error = None;

        for tick in 0_u64..32 {
            let input = ClassicHdGraphInput {
                generation: 7,
                tick,
                profile_revision: profile.profile_revision,
                // The graph's history contract treats this as the stable
                // world/render-input identity across ticks.  The synthetic
                // world closure still changes by tick below, which exercises
                // the temporal branch without pretending that a changed
                // world identity can reuse prior presentation history.
                input_hash: 0x1000,
                reflections_enabled: true,
                dynamic_lighting_enabled: true,
                lighting_path: ClassicHdLightingPath::ScreenSpaceMap,
                water_enabled: true,
                blur: ClassicHdBlurInput {
                    configured_mode: 2,
                    blur_capability: true,
                    high_speed_system: true,
                    scratch_present: true,
                    menu_state: 0,
                    actual_speed: 120,
                    spatial_smoothing: false,
                    spatial_radius: 1,
                },
            };
            let started = Instant::now();
            let result =
                graph.render_frame(input, Some(&shading), &tablesn, |indices, provenance| {
                    for (offset, value) in indices.iter_mut().enumerate() {
                        *value = (offset as u64)
                            .wrapping_add(input.tick.wrapping_mul(17))
                            .wrapping_add(input.input_hash) as u8;
                    }
                    for (offset, value) in provenance.iter_mut().enumerate() {
                        *value = ((offset / profile.stride as usize) as u64 ^ input.tick) as u8;
                    }
                    Ok(ClassicHdWorldReceipt::complete(
                        byte_hash(indices),
                        byte_hash(provenance),
                        true,
                        true,
                    ))
                });
            samples.push(started.elapsed().as_nanos());
            match result {
                Ok(result) => {
                    if tick == 0 {
                        first_hash = result.receipt.output_hash;
                        first_provenance_hash = result.receipt.provenance_hash;
                    }
                    last_hash = result.receipt.output_hash;
                }
                Err(error) => {
                    render_error = Some(error.to_string());
                    break;
                }
            }
        }

        if let Some(error) = render_error {
            reports.push(format!(
                "{{\"profile\":\"{name}\",\"status\":\"fail\",\"error\":\"{}\"}}",
                json_escape(&error)
            ));
            continue;
        }

        graph.reset(8);
        let reset_input = ClassicHdGraphInput {
            generation: 8,
            tick: 0,
            profile_revision: profile.profile_revision,
            input_hash: 0x1000,
            reflections_enabled: true,
            dynamic_lighting_enabled: true,
            lighting_path: ClassicHdLightingPath::ScreenSpaceMap,
            water_enabled: true,
            blur: ClassicHdBlurInput {
                configured_mode: 2,
                blur_capability: true,
                high_speed_system: true,
                scratch_present: true,
                menu_state: 0,
                actual_speed: 120,
                spatial_smoothing: false,
                spatial_radius: 1,
            },
        };
        let reset_result = graph.render_frame(
            reset_input,
            Some(&shading),
            &tablesn,
            |indices, provenance| {
                for (offset, value) in indices.iter_mut().enumerate() {
                    *value = (offset as u64).wrapping_add(reset_input.input_hash) as u8;
                }
                for (offset, value) in provenance.iter_mut().enumerate() {
                    *value = (offset / profile.stride as usize) as u8;
                }
                Ok(ClassicHdWorldReceipt::complete(
                    byte_hash(indices),
                    byte_hash(provenance),
                    true,
                    true,
                ))
            },
        );
        let reset_hash = reset_result.map_or(0, |result| result.receipt.output_hash);
        samples.sort_unstable();
        let p95_index = ((samples.len() * 95).saturating_add(99) / 100).saturating_sub(1);
        let p95_ns = samples.get(p95_index).copied().unwrap_or(0);
        let deterministic = reset_hash == first_hash && first_hash != 0 && last_hash != 0;
        let cpu_budget_ns = 33_333_333_u128;
        let cpu_budget_pass = p95_ns <= cpu_budget_ns;
        let mandatory = name == "classic-hd-640x480";
        let status = if mandatory && cpu_budget_pass && deterministic {
            "supported-cpu-and-deterministic"
        } else if !mandatory && cpu_budget_pass && deterministic {
            "candidate-awaiting-unity-gpu-and-owner-route-capture"
        } else {
            "rejected-by-measured-cpu-or-determinism"
        };
        reports.push(format!(
            "{{\"profile\":\"{name}\",\"status\":\"{status}\",\"width\":{},\"height\":{},\"frame_bytes\":{},\"memory_bytes\":{},\"samples\":{},\"p95_cpu_ns\":{},\"cpu_budget_ns\":{},\"deterministic_reset\":{},\"first_hash\":\"{first_hash:016x}\",\"first_provenance_hash\":\"{first_provenance_hash:016x}\",\"reset_hash\":\"{reset_hash:016x}\"}}",
            profile.width,
            profile.height,
            profile.extent(),
            profile.memory_bytes,
            samples.len(),
            p95_ns,
            cpu_budget_ns,
            deterministic,
        ));
    }

    println!(
        "{{\"schema_version\":1,\"measurement\":\"Rust ClassicHdRenderGraph synthetic world closure on owner-free lab profile\",\"presentation_budget\":\"30Hz p95 CPU frame generation <= 33333333ns; Unity GPU/copy route is separately required\",\"owner_payload_embedded\":false,\"profiles\":[{}]}}",
        reports.join(",")
    );
    ExitCode::SUCCESS
}

fn conformance_validate() -> ExitCode {
    let mut arguments = std::env::args().skip(2).collect::<Vec<_>>();
    let default_validator = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/validate_level1_matrix.py");
    let validator = if arguments.first().map(String::as_str) == Some("--validator") {
        if arguments.len() < 2 {
            println!("{{\"ok\":false,\"closed\":false,\"error\":\"--validator requires a path\"}}");
            return ExitCode::from(2);
        }
        let path = std::path::PathBuf::from(arguments.remove(1));
        arguments.remove(0);
        path
    } else {
        default_validator
    };
    if !validator.is_file() {
        println!(
            "{{\"ok\":false,\"closed\":false,\"error\":\"matrix validator is missing: {}\"}}",
            json_escape(&validator.display().to_string())
        );
        return ExitCode::from(2);
    }
    if !arguments
        .iter()
        .any(|argument| argument == "--require-closed" || argument == "--final-closure")
    {
        arguments.insert(0, "--require-closed".to_owned());
    }
    if !arguments.iter().any(|argument| argument == "--json") {
        arguments.insert(0, "--json".to_owned());
    }
    let status = match Command::new("/usr/bin/python3")
        .arg(&validator)
        .args(&arguments)
        .status()
    {
        Ok(status) => status,
        Err(error) => {
            println!(
                "{{\"ok\":false,\"closed\":false,\"error\":\"{}\"}}",
                json_escape(&format!("failed to execute matrix validator: {error}"))
            );
            return ExitCode::from(2);
        }
    };
    match status.code() {
        Some(0) => ExitCode::SUCCESS,
        Some(2) => ExitCode::from(2),
        _ => ExitCode::FAILURE,
    }
}

fn find_castle_site() -> ExitCode {
    let arguments = std::env::args().collect::<Vec<_>>();
    if arguments.len() != 6 {
        eprintln!("usage: sim-cli find-castle-site <package> <closure> <center-x> <center-y>");
        return ExitCode::from(2);
    }
    let world = match NativeWorld::load(&arguments[2], &arguments[3]) {
        Ok(world) => world,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let parse = |value: &str| {
        value.parse::<u16>().map_err(|error| {
            eprintln!("invalid coordinate {value}: {error}");
        })
    };
    let (center_x, center_y) = match (parse(&arguments[4]), parse(&arguments[5])) {
        (Ok(x), Ok(y)) => (x, y),
        _ => return ExitCode::from(2),
    };
    let center_tile_x = center_x.to_be_bytes()[0];
    let center_tile_y = center_y.to_be_bytes()[0];
    let mut best = None;
    for dy in -32_i16..=32 {
        for dx in -32_i16..=32 {
            let tile_x =
                center_tile_x.wrapping_add_signed(i8::try_from(dx).expect("scan delta fits i8"));
            let tile_y =
                center_tile_y.wrapping_add_signed(i8::try_from(dy).expect("scan delta fits i8"));
            let valid = (0..8_u8).all(|row| {
                (0..8_u8).all(|column| {
                    world
                        .angle_flags(Tile {
                            x: tile_x.wrapping_sub(8).wrapping_add(column),
                            y: tile_y.wrapping_sub(8).wrapping_add(row),
                        })
                        .cast_signed()
                        >= 0
                })
            });
            if !valid {
                continue;
            }
            let distance = i32::from(dx) * i32::from(dx) + i32::from(dy) * i32::from(dy);
            if best.is_some_and(|(best_distance, _, _)| distance >= best_distance) {
                continue;
            }
            best = Some((distance, tile_x, tile_y));
        }
    }
    let Some((_, tile_x, tile_y)) = best else {
        eprintln!("no valid castle footprint within 32 tiles");
        return ExitCode::FAILURE;
    };
    let position = Position {
        x: u16::from(tile_x) * 256 + 128,
        y: u16::from(tile_y) * 256 + 128,
        z: 0,
    };
    println!(
        "{{\"x\":{},\"y\":{},\"z\":{},\"tile_x\":{},\"tile_y\":{}}}",
        position.x,
        position.y,
        world.ground_height(position),
        tile_x,
        tile_y
    );
    ExitCode::SUCCESS
}

fn inspect_idle_diff() -> ExitCode {
    let arguments = std::env::args().collect::<Vec<_>>();
    if arguments.len() != 5 {
        eprintln!("usage: sim-cli inspect-idle-diff <package> <closure> <state-sequence>");
        return ExitCode::from(2);
    }
    let state_bytes = match std::fs::read(&arguments[4]) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let sequence = match RegressionStateSequence::new(&state_bytes) {
        Ok(sequence) => sequence,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let world = match NativeWorld::load(&arguments[2], &arguments[3]) {
        Ok(world) => world,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let mut native = world.session();
    for frame_index in 0..sequence.frame_count() {
        if frame_index > 0 {
            native.step(PlayerCommand::default());
        }
        if matches!(frame_index, 0 | 8 | 19) {
            let frame = sequence.frame(frame_index).expect("validated frame");
            let original = frame.project_player_flight(frame.local_player_slot());
            println!(
                "frame={frame_index} original_hash={:016x} native_hash={:016x}\noriginal={original:?}\nnative={:?}",
                state_hash(&original),
                state_hash(&native.state),
                native.state,
            );
        }
    }
    ExitCode::SUCCESS
}

fn inspect_forward() -> ExitCode {
    let arguments = std::env::args().collect::<Vec<_>>();
    if arguments.len() != 4 {
        eprintln!("usage: sim-cli inspect-forward <state-sequence> <terrain-sequence>");
        return ExitCode::from(2);
    }
    let read = |path: &str| {
        std::fs::read(path).map_err(|error| {
            eprintln!(
                "{{\"ok\":false,\"error\":\"{}\"}}",
                json_escape(&error.to_string())
            );
        })
    };
    let state_bytes = match read(&arguments[2]) {
        Ok(bytes) => bytes,
        Err(()) => return ExitCode::FAILURE,
    };
    let terrain_bytes = match read(&arguments[3]) {
        Ok(bytes) => bytes,
        Err(()) => return ExitCode::FAILURE,
    };
    let sequence = match RegressionStateSequence::new(&state_bytes) {
        Ok(sequence) => sequence,
        Err(error) => {
            eprintln!("{{\"ok\":false,\"error\":\"{}\"}}", json_escape(&error));
            return ExitCode::FAILURE;
        }
    };
    let terrain = match LegacyTerrain::from_regression_sequence(&terrain_bytes, 0, false) {
        Ok(terrain) => terrain,
        Err(error) => {
            eprintln!("{{\"ok\":false,\"error\":\"{}\"}}", json_escape(&error));
            return ExitCode::FAILURE;
        }
    };
    let frame = sequence.frame(0).expect("validated frame zero");
    let mut state = frame.project_player_flight(frame.local_player_slot());
    let command = latch_command(
        &mut state,
        PlayerCommand {
            flags: 1,
            ..PlayerCommand::default()
        },
    );
    let mut context = FlightContext::default();
    for pass in 1..=8 {
        step_entity_pass_with_context(
            &mut state,
            command,
            FlightProfile::OUTDOOR,
            &terrain,
            &mut context,
        );
        println!(
            "{{\"native_step_count\":{pass},\"position\":[{},{},{}],\"yaw\":{},\"display_pitch\":{},\"effective_pitch\":{},\"roll_accumulator\":{},\"pitch_accumulator\":{},\"target_speed\":{},\"speed_changed_flag\":{},\"actual_speed\":{},\"strafe_speed\":{},\"entity_phase\":{},\"state_hash\":\"{:016x}\"}}",
            state.position.x,
            state.position.y,
            state.position.z,
            state.yaw,
            state.display_pitch,
            state.effective_pitch,
            state.roll_accumulator,
            state.pitch_accumulator,
            state.target_speed,
            state.speed_changed_flag,
            state.actual_speed,
            state.strafe_speed,
            state.entity_phase,
            state_hash(&state),
        );
    }
    ExitCode::SUCCESS
}

fn atlas_check() -> ExitCode {
    let registry = match AtlasRegistry::load_embedded() {
        Ok(registry) => registry,
        Err(error) => {
            eprintln!("{{\"ok\":false,\"error\":\"{}\"}}", json_escape(&error));
            return ExitCode::FAILURE;
        }
    };
    let summary = registry.summary();
    let statuses = summary
        .by_status
        .iter()
        .map(|(status, count)| format!("\"{status}\":{count}"))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "{{\"ok\":true,\"tables\":{},\"mappings\":{},\"statuses\":{{{statuses}}}}}",
        summary.tables, summary.mappings
    );
    ExitCode::SUCCESS
}

fn reset_from_authored() -> ExitCode {
    let arguments = std::env::args().collect::<Vec<_>>();
    let (package, closure, matrix, feature_oracle) = match arguments.as_slice() {
        [_, _] => (
            std::path::PathBuf::from("../generated/level1-export-m"),
            std::path::PathBuf::from("../generated/level1-asset-closure.json"),
            std::path::PathBuf::from("level1-conformance/level1-conformance-matrix.json"),
            std::path::PathBuf::from("../generated/plan24-o-final3-20260730"),
        ),
        [_, _, package, closure, matrix, feature_oracle] => (
            std::path::PathBuf::from(package),
            std::path::PathBuf::from(closure),
            std::path::PathBuf::from(matrix),
            std::path::PathBuf::from(feature_oracle),
        ),
        _ => {
            reset_comparator_error(
                "invocation",
                "usage: sim-cli reset-from-authored [<package> <closure> <matrix> <feature-oracle>]",
            );
            return ExitCode::from(2);
        }
    };
    let authored = match std::fs::read(package.join("authored-level.bin")) {
        Ok(bytes) => bytes,
        Err(error) => {
            reset_comparator_error("authored-read", &error.to_string());
            return ExitCode::from(2);
        }
    };
    let matrix = match std::fs::read(matrix) {
        Ok(bytes) => bytes,
        Err(error) => {
            reset_comparator_error("matrix-read", &error.to_string());
            return ExitCode::from(2);
        }
    };
    let world = match NativeWorld::load(&package, closure) {
        Ok(world) => world,
        Err(error) => {
            reset_comparator_error("owner-package-load", &error.to_string());
            return ExitCode::from(2);
        }
    };
    let feature_oracle = match Level1FeatureOracleOwned::read(&feature_oracle) {
        Ok(oracle) => oracle,
        Err(error) => {
            reset_comparator_error("feature-oracle-read", &error);
            return ExitCode::from(2);
        }
    };
    let observed = world
        .entities()
        .iter()
        .filter(|entity| entity.active)
        .map(|entity| Level1ResetSourceSlot {
            index: entity.index,
            active: entity.active,
            class: entity.class,
            model: entity.model,
            action: entity.action,
            phase: entity.phase,
            entity_rng: entity.entity_rng,
            control_flags: entity.control_flags,
        })
        .collect::<Vec<_>>();
    let report = match compare_level1_reset_from_authored_with_feature_oracle(
        &authored,
        &observed,
        &matrix,
        feature_oracle.as_bytes(),
    ) {
        Ok(report) => report,
        Err(error) => {
            reset_comparator_error("comparator-harness", &error);
            return ExitCode::from(2);
        }
    };
    println!("{}", level1_reset_production_report_json(&report));
    if report.usable() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn reset_comparator_error(stage: &str, error: &str) {
    eprintln!("{}", level1_reset_harness_error_json(stage, error));
}

fn inspect_regression() -> ExitCode {
    let Some(path) = std::env::args().nth(2) else {
        eprintln!("inspect-regression requires a state sequence path");
        return ExitCode::from(2);
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!(
                "{{\"ok\":false,\"error\":\"{}\"}}",
                json_escape(&error.to_string())
            );
            return ExitCode::FAILURE;
        }
    };
    let sequence = match RegressionStateSequence::new(&bytes) {
        Ok(sequence) => sequence,
        Err(error) => {
            eprintln!("{{\"ok\":false,\"error\":\"{}\"}}", json_escape(&error));
            return ExitCode::FAILURE;
        }
    };
    for frame_index in 0..sequence.frame_count() {
        let frame = sequence.frame(frame_index).expect("validated frame index");
        let player_slot = frame.local_player_slot();
        let entity_index = frame.player_entity_index(player_slot);
        let entity = frame.entity(entity_index);
        let command = frame.player_command(player_slot);
        let decoded = frame.decoded_flight_input(player_slot);
        let state = frame.project_player_flight(player_slot);
        println!(
            "{{\"frame\":{frame_index},\"turn\":{},\"player_slot\":{player_slot},\"entity\":{entity_index},\"class\":{},\"model\":{},\"action\":{},\"raw_input\":{:?},\"decoded_flags\":{},\"decoded_roll_delta\":{},\"decoded_pitch_delta\":{},\"position\":[{},{},{}],\"yaw\":{},\"display_pitch\":{},\"effective_pitch\":{},\"target_speed\":{},\"actual_speed\":{},\"roll_accumulator\":{},\"pitch_accumulator\":{},\"state_hash\":\"{:016x}\"}}",
            frame.player_turn(player_slot),
            entity.class(),
            entity.model(),
            entity.action(),
            command.to_bytes(),
            decoded.command.flags,
            decoded.roll_delta,
            decoded.pitch_delta,
            state.position.x,
            state.position.y,
            state.position.z,
            state.yaw,
            state.display_pitch,
            state.effective_pitch,
            state.target_speed,
            state.actual_speed,
            state.roll_accumulator,
            state.pitch_accumulator,
            state_hash(&state),
        );
    }
    ExitCode::SUCCESS
}

fn inspect_recording() -> ExitCode {
    let arguments = std::env::args().collect::<Vec<_>>();
    if arguments.len() < 6 {
        eprintln!(
            "usage: sim-cli inspect-recording <recording> <legacy|current> <level> <player> [first-turn] [last-turn]"
        );
        return ExitCode::from(2);
    }
    let path = &arguments[2];
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!(
                "{{\"ok\":false,\"error\":\"{}\"}}",
                json_escape(&error.to_string())
            );
            return ExitCode::FAILURE;
        }
    };
    let recording = match arguments[3].as_str() {
        "legacy" => InputRecording::parse_legacy_regression(&bytes),
        "current" => InputRecording::parse_with_spell_metadata(&bytes),
        other => {
            eprintln!("unknown recording format {other:?}; expected legacy or current");
            return ExitCode::from(2);
        }
    };
    let recording = match recording {
        Ok(recording) => recording,
        Err(error) => {
            eprintln!("{{\"ok\":false,\"error\":\"{}\"}}", json_escape(&error));
            return ExitCode::FAILURE;
        }
    };
    let parse_number = |index: usize, name: &str| -> Result<u32, String> {
        arguments[index]
            .parse::<u32>()
            .map_err(|error| format!("invalid {name}: {error}"))
    };
    let level = match parse_number(4, "level")
        .and_then(|value| u16::try_from(value).map_err(|_| "level exceeds u16".to_string()))
    {
        Ok(level) => level,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let player_index = match parse_number(5, "player")
        .and_then(|value| u16::try_from(value).map_err(|_| "player exceeds u16".to_string()))
    {
        Ok(player) => player,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let Some(player) = recording.player(level, player_index) else {
        eprintln!("recording has no level {level} player {player_index}");
        return ExitCode::FAILURE;
    };
    let first_turn = arguments
        .get(6)
        .map_or_else(|| Ok(0), |value| value.parse::<u32>())
        .unwrap_or_else(|error| {
            eprintln!("invalid first turn: {error}");
            u32::MAX
        });
    if first_turn == u32::MAX {
        return ExitCode::from(2);
    }
    let last_turn = arguments
        .get(7)
        .map_or_else(|| Ok(u32::MAX), |value| value.parse::<u32>())
        .unwrap_or_else(|error| {
            eprintln!("invalid last turn: {error}");
            0
        });
    if last_turn == 0 && arguments.get(7).is_some_and(|value| value != "0") {
        return ExitCode::from(2);
    }
    print_recording_turns(player, level, player_index, first_turn, last_turn);
    ExitCode::SUCCESS
}

fn print_recording_turns(
    player: &RecordedPlayer<'_>,
    level: u16,
    player_index: u16,
    first_turn: u32,
    last_turn: u32,
) {
    for turn in player
        .turns
        .iter()
        .filter(|turn| turn.turn >= first_turn && turn.turn <= last_turn)
    {
        let bytes = turn.payload;
        if bytes.len() == 10 {
            let command = PlayerCommand::from_bytes(bytes.try_into().expect("checked length"));
            println!(
                "{{\"level\":{level},\"player\":{player_index},\"turn\":{},\"bytes\":{:?},\"opcode\":{},\"roll\":{},\"pitch\":{},\"flags\":{},\"word6\":{},\"word8\":{}}}",
                turn.turn,
                bytes,
                command.opcode,
                command.roll,
                command.pitch,
                command.flags,
                command.argument_6,
                command.argument_8,
            );
        } else {
            println!(
                "{{\"level\":{level},\"player\":{player_index},\"turn\":{},\"bytes\":{:?}}}",
                turn.turn, bytes
            );
        }
    }
}

fn trace() -> ExitCode {
    let commands = [
        PlayerCommand {
            flags: 1,
            ..Default::default()
        },
        PlayerCommand {
            flags: 1,
            roll: 8,
            ..Default::default()
        },
        PlayerCommand {
            flags: 9,
            roll: 12,
            ..Default::default()
        },
        PlayerCommand {
            flags: 8,
            pitch: -4,
            ..Default::default()
        },
        PlayerCommand::default(),
    ];
    let mut state = FlightState::default();
    for command in commands {
        step_turn(&mut state, command, FlightProfile::OUTDOOR, &FlatWorld);
    }
    println!(
        "{{\"ok\":true,\"ticks\":{},\"position\":{{\"x\":{},\"y\":{},\"z\":{}}},\"yaw\":{},\"target_speed\":{},\"actual_speed\":{},\"strafe_speed\":{},\"state_hash\":\"{:016x}\"}}",
        state.tick,
        state.position.x,
        state.position.y,
        state.position.z,
        state.yaw,
        state.target_speed,
        state.actual_speed,
        state.strafe_speed,
        state_hash(&state)
    );
    ExitCode::SUCCESS
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
