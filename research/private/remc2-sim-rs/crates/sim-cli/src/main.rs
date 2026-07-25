use atlas_registry::AtlasRegistry;
use sim_core::legacy_terrain::LegacyTerrain;
use sim_core::native_world::NativeWorld;
use sim_core::recording::{InputRecording, RecordedPlayer};
use sim_core::regression_state::RegressionStateSequence;
use sim_core::{
    FlightContext, FlightProfile, FlightState, FlightWorld, PlayerCommand, Position, Tile,
    latch_command, state_hash, step_entity_pass_with_context, step_turn,
};
use std::process::ExitCode;

struct FlatWorld;

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
        Some("inspect-recording") => inspect_recording(),
        Some("inspect-regression") => inspect_regression(),
        Some("inspect-forward") => inspect_forward(),
        Some("inspect-idle-diff") => inspect_idle_diff(),
        Some("trace") => trace(),
        _ => {
            eprintln!(
                "usage: sim-cli <atlas-check|inspect-forward|inspect-idle-diff|inspect-recording|inspect-regression|trace> ..."
            );
            ExitCode::from(2)
        }
    }
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
