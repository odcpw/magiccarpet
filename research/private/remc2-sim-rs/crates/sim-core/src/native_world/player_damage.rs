//! Human-player hit feedback at the source sub_5EFA0 mailbox boundary.
//!
//! Damage and Shield own the life/mana transition. This owner supplies the
//! resulting recoil, source RNG/sound and surviving-player music feedback
//! before the same visit enters sub_5D530's flight body.

use super::{EntitySnapshot, PlayerSnapshot};
use crate::{FlightEvent, FlightState, Position};

pub(super) fn receive_hit(
    state: &mut FlightState,
    player: &mut EntitySnapshot,
    attacker_position: Position,
    applied_damage: i32,
    lethal: bool,
) -> FlightEvent {
    let dx = i32::from(
        player
            .position
            .x
            .wrapping_sub(attacker_position.x)
            .cast_signed(),
    );
    let dy = i32::from(
        player
            .position
            .y
            .wrapping_sub(attacker_position.y)
            .cast_signed(),
    );
    let horizontal = crate::legacy_sqrt(
        dx.wrapping_mul(dx)
            .wrapping_add(dy.wrapping_mul(dy))
            .cast_unsigned(),
    );
    state.recoil_yaw = crate::heading_for_positions(attacker_position, player.position) as i16;
    state.effective_pitch = crate::pitch_for_delta(
        horizontal as i32,
        i32::from(player.position.z.wrapping_sub(attacker_position.z)),
    );
    state.recoil_speed = (applied_damage / 10).clamp(0, 80) as i16;
    if !lethal {
        // sub_5EF70 is a linked-player-state write, not entity word36.
        state.music_countdown = 100;
    }
    player.set_entity_rng(player.entity_rng.wrapping_mul(9_377).wrapping_add(9_439));
    crate::mark_source_audio_call(crate::SourceAudioCallTag::PlayerPain);
    FlightEvent::PrepareSound {
        #[cfg(test)]
        audio_site: crate::MissionAudioSite::PlayerPain,
        entity_index: player.index,
        player_index: -1,
        sound_id: (player.entity_rng & 3) + 54,
    }
}

/// The same source linked-player fields are read by reset/save projections.
/// Publish after the flight body as well as at the hit boundary, so its
/// recoil decay and effective pitch remain canonical in every player view.
pub(super) fn publish_flight_feedback(state: &FlightState, player: &mut PlayerSnapshot) {
    if player.entity_index != state.entity_index {
        return;
    }
    player.record[32..34].copy_from_slice(&state.recoil_speed.to_le_bytes());
    player.record[34..36].copy_from_slice(&state.recoil_yaw.to_le_bytes());
    player.record[38..40].copy_from_slice(&state.effective_pitch.to_le_bytes());
}
