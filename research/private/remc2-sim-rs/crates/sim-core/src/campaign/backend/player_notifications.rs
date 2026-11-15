//! Read the same per-participant notice state that gameplay and presentation own.

use sim_session::ParticipantNotification;

use super::Mc2CampaignBackend;
use crate::game_definition::SemanticId;

pub(super) fn observe(backend: &Mc2CampaignBackend) -> Vec<ParticipantNotification> {
    let simulation = backend.campaign.simulation();
    let local_color = simulation
        .shared_spells()
        .local_player()
        .player_color_index();
    let names = super::source_scoreboard::source_names();

    // Source DrawTextPauseEndOfLevel walks participant information, not the
    // live-actor/scoreboard census. Banishment must not erase its own notice.
    simulation
        .player_lifecycle_notifications()
        .entries()
        .iter()
        .enumerate()
        .filter_map(|(color, notice)| {
            if notice.state != 1 && notice.state != 3 {
                return None;
            }
            let kind = notice
                .kind
                .expect("a published player notice has a semantic kind");
            Some(ParticipantNotification {
                participant: backend
                    .participant(u64::from(notice.source_owner))
                    .expect("opaque participant projection is total"),
                is_local: color == usize::from(local_color),
                display_name: names[color]
                    .as_ref()
                    .expect("single-player source roster has all eight names")
                    .clone(),
                kind: SemanticId::parse(kind.semantic_name())
                    .expect("compiled player-notice semantic kind"),
                remaining_frames: notice.remaining_frames,
            })
        })
        .collect()
}
