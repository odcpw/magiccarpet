//! Thin owner-local browser adapter over the deterministic simulation core.
//!
//! This crate deliberately has no atlas, filesystem, renderer, or browser API
//! dependency. It is private preservation research and is not distributable.

use sim_core::{
    FlightProfile, FlightState, FlightWorld, PlayerCommand, Position, Tile, latch_command,
    state_hash, step_entity_pass,
};
use wasm_bindgen::prelude::*;

const WORLD_CENTER: u16 = 32_768;
const START_CLEARANCE: i16 = 1_024;

#[derive(Clone, Copy, Debug, Default)]
struct DemoWorld;

impl DemoWorld {
    fn height(position: Position) -> i16 {
        let x = signed_delta(position.x, WORLD_CENTER);
        let y = signed_delta(position.y, WORLD_CENTER);
        let radius = integer_sqrt(square(x) + square(y));

        let island = 1_250 - radius / 12;
        let broad_hills = triangle_wave(x + y, 8_192) / 10
            + triangle_wave(x - y, 6_144) / 14
            + triangle_wave(x * 2 + y, 3_584) / 20;

        let volcano_x = signed_delta(position.x, 34_000);
        let volcano_y = signed_delta(position.y, 23_000);
        let volcano_radius = integer_sqrt(square(volcano_x) + square(volcano_y));
        let cone = (6_200 - volcano_radius).max(0) / 3;
        let crater = (900 - volcano_radius).max(0) * 2;

        let height = island + broad_hills + cone - crater;
        i16::try_from(height.clamp(-320, 4_200)).expect("clamped terrain height fits i16")
    }
}

impl FlightWorld for DemoWorld {
    fn ground_height(&self, position: Position) -> i16 {
        Self::height(position)
    }

    fn ceiling_height(&self, _position: Position) -> Option<i16> {
        None
    }

    fn material_mask(&self, position: Position) -> u32 {
        if Self::height(position) < 0 {
            sim_core::MOVEMENT_REDIRECT_MASK
        } else {
            0
        }
    }

    fn angle_flags(&self, _tile: Tile) -> u8 {
        0
    }
}

#[wasm_bindgen]
pub struct BrowserSimulation {
    state: FlightState,
    world: DemoWorld,
}

#[wasm_bindgen]
impl BrowserSimulation {
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        let world = DemoWorld;
        let mut state = FlightState::default();
        state.position.z = world
            .ground_height(state.position)
            .saturating_add(START_CLEARANCE);
        Self { state, world }
    }

    /// Latch one browser command, then run the same command through one, four,
    /// or eight entity passes. Other pass counts are clamped to one.
    pub fn advance(&mut self, flags: u8, roll: i8, pitch: i8, passes: u8) {
        let command = latch_command(
            &mut self.state,
            PlayerCommand {
                flags: flags.cast_signed(),
                roll,
                pitch,
                ..PlayerCommand::default()
            },
        );
        let passes = match passes {
            4 => 4,
            8 => 8,
            _ => 1,
        };
        for _ in 0..passes {
            step_entity_pass(
                &mut self.state,
                command,
                FlightProfile::OUTDOOR,
                &self.world,
            );
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    #[must_use]
    pub fn x(&self) -> u16 {
        self.state.position.x
    }

    #[must_use]
    pub fn y(&self) -> u16 {
        self.state.position.y
    }

    #[must_use]
    pub fn z(&self) -> i16 {
        self.state.position.z
    }

    #[must_use]
    pub fn yaw(&self) -> i16 {
        self.state.yaw
    }

    #[must_use]
    pub fn pitch(&self) -> i16 {
        self.state.display_pitch
    }

    #[must_use]
    pub fn roll(&self) -> i16 {
        self.state.roll_accumulator
    }

    #[must_use]
    pub fn target_speed(&self) -> i16 {
        self.state.target_speed
    }

    #[must_use]
    pub fn actual_speed(&self) -> i16 {
        self.state.actual_speed
    }

    #[must_use]
    pub fn strafe_speed(&self) -> i16 {
        self.state.strafe_speed
    }

    #[must_use]
    pub fn tick(&self) -> u64 {
        self.state.tick
    }

    #[must_use]
    pub fn state_hash_hex(&self) -> String {
        format!("{:016x}", state_hash(&self.state))
    }

    #[must_use]
    pub fn terrain_height(&self, x: u16, y: u16) -> i16 {
        DemoWorld::height(Position { x, y, z: 0 })
    }
}

impl Default for BrowserSimulation {
    fn default() -> Self {
        Self::new()
    }
}

fn signed_delta(value: u16, center: u16) -> i32 {
    i32::from(value.wrapping_sub(center).cast_signed())
}

fn square(value: i32) -> u64 {
    i64::from(value).unsigned_abs().pow(2)
}

fn integer_sqrt(value: u64) -> i32 {
    if value < 2 {
        return i32::try_from(value).expect("small root fits i32");
    }
    let mut x = value;
    let mut next = u64::midpoint(x, value / x);
    while next < x {
        x = next;
        next = u64::midpoint(x, value / x);
    }
    i32::try_from(x).unwrap_or(i32::MAX)
}

fn triangle_wave(value: i32, period: i32) -> i32 {
    let phase = value.rem_euclid(period);
    let half = period / 2;
    half - (phase - half).abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_has_ocean_island_and_volcano() {
        let world = DemoWorld;
        let center = Position {
            x: WORLD_CENTER,
            y: WORLD_CENTER,
            z: 0,
        };
        let ocean = Position { x: 0, y: 0, z: 0 };
        let volcano = Position {
            x: 34_000,
            y: 23_900,
            z: 0,
        };
        assert!(world.ground_height(center) > 0);
        assert!(world.ground_height(ocean) < 0);
        assert!(world.ground_height(volcano) > world.ground_height(center));
        assert_eq!(world.material_mask(ocean), sim_core::MOVEMENT_REDIRECT_MASK);
    }

    #[test]
    fn repeated_browser_command_stream_has_a_stable_hash() {
        let run = || {
            let mut simulation = BrowserSimulation::new();
            for tick in 0..300 {
                let flags = if tick < 220 { 1 } else { 9 };
                let roll = if tick % 80 < 40 { 10 } else { -10 };
                let pitch = if tick % 120 < 60 { 3 } else { -3 };
                simulation.advance(flags, roll, pitch, 1);
            }
            simulation.state_hash_hex()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn one_latch_can_drive_multiple_passes() {
        let mut one = BrowserSimulation::new();
        let mut four = BrowserSimulation::new();
        one.advance(1, 8, -4, 1);
        four.advance(1, 8, -4, 4);
        assert_eq!(one.tick(), 1);
        assert_eq!(four.tick(), 4);
        assert_ne!(one.state_hash_hex(), four.state_hash_hex());
    }

    #[test]
    fn browser_spawn_faces_the_negative_y_route_to_the_volcano() {
        let mut simulation = BrowserSimulation::new();
        let start_y = simulation.y();
        assert_eq!(simulation.yaw(), 0);
        for _ in 0..24 {
            simulation.advance(1, 0, 0, 1);
        }
        assert!(simulation.y() < start_y);
    }
}
