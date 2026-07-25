//! Owner-local C ABI for native presentation clients.
//!
//! The legacy scalar ABI uses a process-wide mutex. Every exported call
//! contains Rust panics and returns its documented zero sentinel on failure.
//! The package-backed, handle-based ABI v2 lives in `native_v2`.

#[cfg(not(test))]
use std::{fs, path::PathBuf};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Mutex, OnceLock},
};

use sim_core::{
    FlightContext, FlightEvent, FlightProfile, PlayerCommand,
    legacy_terrain::LegacyTerrain,
    regression_state::RegressionStateSequence,
    spell_activation::{SpellActivationHolder, SpellCasterState},
    state_hash, step_outer_turn_with_context,
};

mod native_v2;

const ABI_VERSION: u32 = 1;
#[cfg(test)]
const OWNER_STATE_SEQUENCE: &[u8] = include_bytes!(
    "../../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions/level1/sequence-002285FF-00356038.bin"
);
#[cfg(test)]
const OWNER_TERRAIN_SEQUENCE: &[u8] = include_bytes!(
    "../../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions/level1/sequence-002285FF-002DC4E0.bin"
);

struct NativeSim {
    state: sim_core::FlightState,
    terrain: LegacyTerrain,
    context: FlightContext,
    fire_sequence: u32,
    last_resolution: u8,
}

impl NativeSim {
    fn owner_level_one() -> Self {
        #[cfg(test)]
        let (state_bytes, terrain_bytes) = (
            OWNER_STATE_SEQUENCE.to_vec(),
            OWNER_TERRAIN_SEQUENCE.to_vec(),
        );
        #[cfg(not(test))]
        let (state_bytes, terrain_bytes) = (
            read_legacy_fixture("MC_LEGACY_STATE_SEQUENCE"),
            read_legacy_fixture("MC_LEGACY_TERRAIN_SEQUENCE"),
        );
        let frame = RegressionStateSequence::new(&state_bytes)
            .expect("embedded owner state sequence is valid")
            .frame(0)
            .expect("embedded owner state sequence has frame zero");
        let player_slot = frame.local_player_slot();
        let player = frame.entity(frame.player_entity_index(player_slot));
        let left_slot = frame.player_left_spell_slot(player_slot);
        let holder_index = frame.player_enabled_spell_holder(player_slot, left_slot);
        let holder = frame.entity(holder_index);
        let projected = frame.project_player_flight(player_slot);
        let context = FlightContext {
            spell_fire: Some(sim_core::SpellFireContext {
                left_holder_index: holder_index,
                caster: SpellCasterState {
                    entity_index: player.entity_index(),
                    model: player.model(),
                    mana: player.mana(),
                    control_flags: player.control_flags(),
                    player_color_index: projected.player_color_index,
                    control_mode: frame.player_control_mode(player_slot),
                },
                holders: vec![SpellActivationHolder {
                    entity_index: holder.entity_index(),
                    model: holder.model(),
                    level_gate: holder.level_gate(),
                    active_value: holder.active_value(),
                    activation_value: holder.activation_value(),
                    mana_cost: holder.mana_cost(),
                    action_marker: holder.action_marker(),
                }],
                ..sim_core::SpellFireContext::default()
            }),
            ..FlightContext::default()
        };
        Self {
            state: projected,
            terrain: LegacyTerrain::from_regression_sequence(&terrain_bytes, 0, false)
                .expect("embedded owner terrain sequence is valid"),
            context,
            fire_sequence: 0,
            last_resolution: 0,
        }
    }

    fn step(&mut self, command: PlayerCommand) -> u64 {
        self.context.events.clear();
        let evidence = step_outer_turn_with_context(
            &mut self.state,
            command,
            0,
            FlightProfile::OUTDOOR,
            &self.terrain,
            &mut self.context,
        );
        let evidence = evidence
            .first()
            .expect("source speed index zero always runs one pass");
        self.last_resolution = match evidence.movement_resolution {
            "accepted" => 0,
            "redirected" => 1,
            "rejected" => 2,
            "rejected-and-stopped" => 3,
            "recovered-after-reject" => 4,
            "skipped" => 5,
            _ => u8::MAX,
        };
        if self
            .context
            .events
            .iter()
            .any(|event| matches!(event, FlightEvent::RequestSpellActivation { .. }))
        {
            self.fire_sequence = self.fire_sequence.wrapping_add(1);
        }
        evidence.state_hash
    }
}

#[cfg(not(test))]
fn read_legacy_fixture(variable: &str) -> Vec<u8> {
    let path = std::env::var_os(variable).map_or_else(
        || panic!("{variable} is required for legacy ABI v1"),
        PathBuf::from,
    );
    fs::read(path).unwrap_or_else(|error| panic!("{variable} fixture unavailable: {error}"))
}

static NATIVE_SIM: OnceLock<Mutex<NativeSim>> = OnceLock::new();

fn native_sim() -> &'static Mutex<NativeSim> {
    NATIVE_SIM.get_or_init(|| Mutex::new(NativeSim::owner_level_one()))
}

fn with_sim<T>(operation: impl FnOnce(&NativeSim) -> T) -> T {
    let guard = native_sim()
        .lock()
        .expect("legacy simulation mutex is poisoned");
    operation(&guard)
}

fn with_sim_mut<T>(operation: impl FnOnce(&mut NativeSim) -> T) -> T {
    let mut guard = native_sim()
        .lock()
        .expect("legacy simulation mutex is poisoned");
    operation(&mut guard)
}

fn legacy_boundary<T: Copy>(sentinel: T, operation: impl FnOnce() -> T) -> T {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(value) => value,
        Err(_) => sentinel,
    }
}

/// Return the scalar ABI contract version.
#[unsafe(no_mangle)]
pub extern "C" fn mc_sim_abi_version() -> u32 {
    legacy_boundary(0, || ABI_VERSION)
}

/// Reset to the exact owner level-one frame-zero projection.
#[unsafe(no_mangle)]
pub extern "C" fn mc_sim_reset() -> u64 {
    legacy_boundary(0, || {
        with_sim_mut(|sim| {
            *sim = NativeSim::owner_level_one();
            state_hash(&sim.state)
        })
    })
}

/// Advance one source-speed-index-zero outer turn.
#[unsafe(no_mangle)]
pub extern "C" fn mc_sim_step(opcode: i8, argument_1: i8, roll: i8, pitch: i8, flags: i8) -> u64 {
    legacy_boundary(0, || {
        with_sim_mut(|sim| {
            sim.step(PlayerCommand {
                opcode,
                argument_1,
                roll,
                pitch,
                flags,
                ..PlayerCommand::default()
            })
        })
    })
}

/// Query the exact terrain height used by the simulation.
#[unsafe(no_mangle)]
pub extern "C" fn mc_sim_ground_height(x: u16, y: u16) -> i16 {
    use sim_core::FlightWorld as _;
    legacy_boundary(0, || {
        with_sim(|sim| sim.terrain.ground_height(sim_core::Position { x, y, z: 0 }))
    })
}

/// Return the current wrapped source-space X coordinate.
#[unsafe(no_mangle)]
pub extern "C" fn mc_sim_x() -> u16 {
    legacy_boundary(0, || with_sim(|sim| sim.state.position.x))
}

/// Return the current wrapped source-space Y coordinate.
#[unsafe(no_mangle)]
pub extern "C" fn mc_sim_y() -> u16 {
    legacy_boundary(0, || with_sim(|sim| sim.state.position.y))
}

/// Return the current signed source-space altitude.
#[unsafe(no_mangle)]
pub extern "C" fn mc_sim_z() -> i16 {
    legacy_boundary(0, || with_sim(|sim| sim.state.position.z))
}

/// Return the current 11-bit source yaw.
#[unsafe(no_mangle)]
pub extern "C" fn mc_sim_yaw() -> i16 {
    legacy_boundary(0, || with_sim(|sim| sim.state.yaw))
}

/// Return the current display pitch.
#[unsafe(no_mangle)]
pub extern "C" fn mc_sim_display_pitch() -> i16 {
    legacy_boundary(0, || with_sim(|sim| sim.state.display_pitch))
}

/// Return the terrain-shaped movement pitch used by the authoritative step.
#[unsafe(no_mangle)]
pub extern "C" fn mc_sim_effective_pitch() -> i16 {
    legacy_boundary(0, || with_sim(|sim| sim.state.effective_pitch))
}

/// Return the current source roll accumulator used to advance yaw.
#[unsafe(no_mangle)]
pub extern "C" fn mc_sim_roll_accumulator() -> i16 {
    legacy_boundary(0, || with_sim(|sim| sim.state.roll_accumulator))
}

/// Return the current target speed.
#[unsafe(no_mangle)]
pub extern "C" fn mc_sim_target_speed() -> i16 {
    legacy_boundary(0, || with_sim(|sim| sim.state.target_speed))
}

/// Return the current actual speed.
#[unsafe(no_mangle)]
pub extern "C" fn mc_sim_actual_speed() -> i16 {
    legacy_boundary(0, || with_sim(|sim| sim.state.actual_speed))
}

/// Return the current source entity phase byte.
#[unsafe(no_mangle)]
pub extern "C" fn mc_sim_phase() -> u8 {
    legacy_boundary(0, || with_sim(|sim| sim.state.entity_phase))
}

/// Return the monotonically wrapping presentation fire-event sequence.
#[unsafe(no_mangle)]
pub extern "C" fn mc_sim_fire_sequence() -> u32 {
    legacy_boundary(0, || with_sim(|sim| sim.fire_sequence))
}

/// Return the last movement-resolution code.
#[unsafe(no_mangle)]
pub extern "C" fn mc_sim_last_resolution() -> u8 {
    legacy_boundary(0, || with_sim(|sim| sim.last_resolution))
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[test]
    fn reset_exposes_the_owner_level_one_projection() {
        let _guard = test_guard();
        let hash = mc_sim_reset();
        assert_eq!(mc_sim_abi_version(), 1);
        assert_eq!(
            (mc_sim_x(), mc_sim_y(), mc_sim_z()),
            (19_840, 56_960, 5_024)
        );
        assert_eq!(mc_sim_yaw(), 0);
        assert_eq!(mc_sim_phase(), 153);
        assert_eq!(hash, with_sim(|sim| state_hash(&sim.state)));
    }

    #[test]
    fn scalar_abi_drives_the_exact_core_and_reports_fire() {
        let _guard = test_guard();
        mc_sim_reset();
        let initial_fire = mc_sim_fire_sequence();
        let first_hash = mc_sim_step(0, 0, 0, 0, 1);
        assert_eq!(mc_sim_target_speed(), 16);
        assert_eq!(mc_sim_actual_speed(), 16);
        assert_eq!(mc_sim_phase(), 154);
        assert_eq!(first_hash, with_sim(|sim| state_hash(&sim.state)));

        mc_sim_step(0, 0, 0, 0, 0x10);
        assert_eq!(mc_sim_fire_sequence(), initial_fire.wrapping_add(1));
    }

    #[test]
    fn eight_forward_ticks_lock_the_native_client_golden() {
        let _guard = test_guard();
        mc_sim_reset();
        let mut hash = 0;
        for _ in 0..8 {
            hash = mc_sim_step(0, 0, 0, 0, 1);
        }
        assert_eq!(hash, 0x9562_46bd_7f27_0164);
        assert_eq!(
            (mc_sim_x(), mc_sim_y(), mc_sim_z()),
            (19_840, 56_480, 5_040)
        );
        assert_eq!(mc_sim_phase(), 161);
    }

    #[test]
    fn legacy_boundary_contains_panics_and_returns_the_typed_sentinel() {
        let _guard = test_guard();
        assert_eq!(
            legacy_boundary(0x55aa_u16, || -> u16 {
                panic!("intentional legacy ABI containment probe");
            }),
            0x55aa
        );
        assert_eq!(mc_sim_abi_version(), ABI_VERSION);
    }
}
