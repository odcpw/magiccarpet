//! Compatibility façade for the package-neutral render-camera owner.

pub use sim_render_camera::*;

use crate::{FlightState, native_world::Mc2Simulation};

/// `sub_57B20` stores this view before `UpdateEntities`; `DrawWorld_411A0`
/// then adds 128 to eye height. In particular the view uses signed player
/// pitch/2, not the entity's wrapped movement pitch.
pub(crate) fn source_player_view(
    state: &FlightState,
    look_yaw: i16,
    look_pitch: i16,
    color_phase: u8,
) -> sim_session::CameraPose {
    let recoil = i32::from(state.recoil_speed);
    let pitch = i32::from(state.pitch_accumulator) / 2 + i32::from(color_phase) * (recoil / 16)
        - recoil / 8
        + i32::from(look_pitch);
    sim_session::CameraPose {
        x: state.position.x,
        y: state.position.y,
        z: state.position.z.wrapping_add(128),
        yaw: state.yaw.wrapping_add(look_yaw),
        pitch: pitch as i16,
        roll: state.roll_accumulator,
    }
}

impl Mc2Simulation {
    pub(crate) fn source_camera_pose(&self) -> sim_session::CameraPose {
        self.session
            .state
            .scratch
            .source_camera_pose
            .unwrap_or_else(|| {
                let auxiliary = self.shared_spells().beyond_sight_auxiliary();
                source_player_view(
                    &self.state,
                    auxiliary.next_entity,
                    auxiliary.entity_index2,
                    self.session.state.scratch.level2_day_hud_color_phase_1,
                )
            })
    }

    /// Called once after PlayerEvents latches input, before the first entity
    /// pass. Retain the row across multiple entity passes and paused draws.
    pub(crate) fn capture_source_camera_pose(&mut self) {
        let auxiliary = self.shared_spells().beyond_sight_auxiliary();
        self.session.state.scratch.source_camera_pose = Some(source_player_view(
            &self.state,
            auxiliary.next_entity,
            auxiliary.entity_index2,
            self.session.state.scratch.level2_day_hud_color_phase_1,
        ));
    }
}

#[cfg(test)]
mod source_view_tests {
    use super::*;

    #[test]
    fn view_pitch_uses_signed_accumulator_half_and_source_recoil_division() {
        let mut state = FlightState {
            pitch_accumulator: -254,
            display_pitch: 1794,
            recoil_speed: -31,
            ..FlightState::default()
        };
        let view = source_player_view(&state, 17, -9, 1);
        assert_eq!(view.pitch, -134); // -127 - 1 + 3 - 9
        assert_eq!(view.yaw, 17);
        assert_eq!(view.z, state.position.z + 128);
        state.pitch_accumulator = 254;
        state.recoil_speed = 31;
        assert_eq!(source_player_view(&state, 0, 0, 0).pitch, 124);
        assert_eq!(source_player_view(&state, 0, 0, 1).pitch, 125);
    }
}
