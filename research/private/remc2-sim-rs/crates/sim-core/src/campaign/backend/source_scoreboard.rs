//! Source MC2 sorcerer-score projection for the player-information menu.
//!
//! The projection joins active source player slots to their live class-3
//! entities, preserving source slot order, owner names, max mana, kill matrix,
//! and the map-type-dependent CLRN indices used by DrawSorcererScores_2D1D0.

use crate::native_world::{
    EntitySnapshot, MapType, Mc2Simulation, PlayerSnapshot, campaign_hud_palette_indices,
};
use sim_session::{SourceScoreboard, SourceSorcererScore};

const SOURCE_PLAYER_SLOTS: usize = 8;
const SOURCE_WIZARD_NAMES: [&str; SOURCE_PLAYER_SLOTS] = [
    "PLAYER", "Nyphur", "Rahn", "Belix", "Jark", "Elyssia", "Yragore", "Prish",
];

/// The same pinned single-player roster used by world sorcerer labels.
pub(crate) fn source_name_bytes(source_color: u8) -> Option<&'static [u8]> {
    SOURCE_WIZARD_NAMES.get(usize::from(source_color)).map(|name| name.as_bytes())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ProjectionError {
    SourceSlotOutOfRange(u8),
    DuplicateSourceSlot(u8),
    ActivePlayerMissingName(u8),
    ActivePlayerMissingEntity {
        source_slot: u8,
        entity_index: u16,
    },
    ActivePlayerEntityIsNotSorcerer {
        source_slot: u8,
        entity_index: u16,
        class: u8,
    },
    ActivePlayerMissingKillMatrix(u8),
    InvalidPalette {
        source_slot: u8,
        message: String,
    },
}

impl std::fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceSlotOutOfRange(slot) => {
                write!(
                    formatter,
                    "active source player slot {slot} is outside 0..8"
                )
            }
            Self::DuplicateSourceSlot(slot) => {
                write!(
                    formatter,
                    "active source player slot {slot} appears more than once"
                )
            }
            Self::ActivePlayerMissingName(slot) => {
                write!(
                    formatter,
                    "active source player slot {slot} has no source display name"
                )
            }
            Self::ActivePlayerMissingEntity {
                source_slot,
                entity_index,
            } => write!(
                formatter,
                "active source player slot {source_slot} has no linked entity {entity_index}"
            ),
            Self::ActivePlayerEntityIsNotSorcerer {
                source_slot,
                entity_index,
                class,
            } => write!(
                formatter,
                "active source player slot {source_slot} links entity {entity_index} of class {class}, expected class 3"
            ),
            Self::ActivePlayerMissingKillMatrix(slot) => write!(
                formatter,
                "active source player slot {slot} has no authoritative word_0x26_38 kill matrix"
            ),
            Self::InvalidPalette {
                source_slot,
                message,
            } => write!(
                formatter,
                "source player slot {source_slot} has no MC2 palette row: {message}"
            ),
        }
    }
}

impl std::error::Error for ProjectionError {}

/// Pinned single-player `WizardNames` from the remc2 source roster.
/// The C API overlays the local player's live preference on slot zero.
pub(super) fn source_names() -> [Option<String>; SOURCE_PLAYER_SLOTS] {
    SOURCE_WIZARD_NAMES.map(|name| Some(name.to_owned()))
}

/// Project live MC2 source players into the neutral snapshot DTO.
///
/// `names_by_source_slot` starts from the pinned single-player source names;
/// the root C API overlays the local player's live preference on slot zero.
/// Multiplayer color remapping is outside the single-player campaign
/// contract, so `source_slot` is the identity consumed by remc2's
/// GetTrueWizardNumber on this route.
pub(super) fn observe(
    simulation: &Mc2Simulation,
    names_by_source_slot: &[Option<String>; SOURCE_PLAYER_SLOTS],
) -> Result<SourceScoreboard, ProjectionError> {
    let map_type = simulation.canonical_terrain().map_type();
    let players = simulation.shared_spells().players();
    let entities = simulation.entities();
    project(
        players,
        entities,
        names_by_source_slot,
        map_type,
        simulation.runtime().player_count,
    )
}

fn project(
    players: &[PlayerSnapshot],
    entities: &[EntitySnapshot],
    names_by_source_slot: &[Option<String>; SOURCE_PLAYER_SLOTS],
    map_type: MapType,
    player_count: u8,
) -> Result<SourceScoreboard, ProjectionError> {
    let mut active_players = players
        .iter()
        .filter(|player| {
            player.slot < player_count
                && source_player_is_visible(
                    player.active_player_index,
                    player.banishment_state_0x006,
                )
        })
        .collect::<Vec<_>>();
    active_players.sort_by_key(|player| player.slot);

    let mut seen_slots = [false; SOURCE_PLAYER_SLOTS];
    let mut observations = Vec::with_capacity(active_players.len());
    for player in active_players {
        let source_slot = player.slot;
        let Some(seen) = seen_slots.get_mut(usize::from(source_slot)) else {
            return Err(ProjectionError::SourceSlotOutOfRange(source_slot));
        };
        if std::mem::replace(seen, true) {
            return Err(ProjectionError::DuplicateSourceSlot(source_slot));
        }

        let entity = entities
            .iter()
            .find(|entity| entity.index == player.entity_index);
        let entity_values = entity.map(|entity| (entity.class, entity.max_mana));
        let display_name = names_by_source_slot[usize::from(source_slot)].as_ref();
        let kills8 = player.source_kill_scores();
        observations.push(project_score_row(
            source_slot,
            player.entity_index,
            display_name,
            entity_values,
            kills8,
            map_type,
        )?);
    }

    Ok(SourceScoreboard {
        players: observations,
        projection_error: None,
    })
}

fn project_score_row(
    source_slot: u8,
    entity_index: u16,
    display_name: Option<&String>,
    entity: Option<(u8, i32)>,
    kills8: Option<[i16; 8]>,
    map_type: MapType,
) -> Result<SourceSorcererScore, ProjectionError> {
    let display_name = display_name
        .filter(|name| !name.is_empty() && !name.contains('\0'))
        .cloned()
        .ok_or(ProjectionError::ActivePlayerMissingName(source_slot))?;
    let (class, max_mana) = entity.ok_or(ProjectionError::ActivePlayerMissingEntity {
        source_slot,
        entity_index,
    })?;
    if class != 3 {
        return Err(ProjectionError::ActivePlayerEntityIsNotSorcerer {
            source_slot,
            entity_index,
            class,
        });
    }
    let kills8 = kills8.ok_or(ProjectionError::ActivePlayerMissingKillMatrix(source_slot))?;
    let palette = campaign_hud_palette_indices(source_slot, map_type).map_err(|error| {
        ProjectionError::InvalidPalette {
            source_slot,
            message: error.to_string(),
        }
    })?;

    Ok(SourceSorcererScore {
        source_slot,
        display_name,
        outline_palette_index: palette[0],
        text_palette_index: palette[1],
        max_mana,
        kills8,
    })
}

fn source_player_is_visible(active_player_index: i16, source_status: Option<u8>) -> bool {
    source_status.map_or(active_player_index >= 0, |status| status == 1)
}

#[cfg(test)]
mod tests {
    use super::{
        ProjectionError, project, project_score_row, source_names, source_player_is_visible,
    };
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::game_definition::default_catalog;
    use crate::game_session::GameSession;
    use crate::native_world::{MapType, campaign_hud_palette_indices};
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn real_campaign_scoreboard_obeys_source_player_count_and_validates_live_links() {
        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let cases = [
            (
                std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE").map_or_else(
                    || generated.join("mc-cf15-level1-package-v3-current"),
                    PathBuf::from,
                ),
                vec![0],
            ),
            (
                std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
                    || generated.join("mc-cf02-campaign-package-v2-b/qualified-level2"),
                    PathBuf::from,
                ),
                vec![0, 1],
            ),
        ];
        let catalog = default_catalog().unwrap();
        for (root, expected_slots) in cases {
            let definition = Arc::new(load_mc2_campaign(&root, &catalog).unwrap());
            let game = GameSession::open(
                &Mc2CampaignAdapter,
                definition,
                812,
                Mc2CampaignInputs::new(&root),
            )
            .unwrap();
            let simulation = game.backend().campaign.simulation();
            assert_eq!(
                usize::from(simulation.runtime().player_count),
                expected_slots.len()
            );
            let scoreboard = game.snapshot().value.source_scoreboard;
            assert!(scoreboard.projection_error.is_none());
            assert_eq!(
                scoreboard
                    .players
                    .iter()
                    .map(|row| row.source_slot)
                    .collect::<Vec<_>>(),
                expected_slots
            );
            for row in &scoreboard.players {
                let source = &simulation.shared_spells().players()[usize::from(row.source_slot)];
                let entity = &simulation.entities()[usize::from(source.entity_index)];
                assert_eq!(entity.class, 3);
                assert_eq!(row.max_mana, entity.max_mana);
                assert_eq!(Some(row.kills8), source.source_kill_scores());
            }

            // The source count excludes unused slots, but a malformed live
            // in-range player link must still fail closed.
            let mut players = simulation.shared_spells().players().to_vec();
            players[0].entity_index = 0;
            let error = project(
                &players,
                simulation.entities(),
                &source_names(),
                simulation.canonical_terrain().map_type(),
                simulation.runtime().player_count,
            )
            .expect_err("a real active row with a non-sorcerer link is invalid");
            assert!(matches!(
                error,
                ProjectionError::ActivePlayerEntityIsNotSorcerer {
                    source_slot: 0,
                    entity_index: 0,
                    class: 0,
                }
            ));
        }
    }

    #[test]
    fn scoreboard_visibility_matches_active_source_slot_and_state() {
        assert!(source_player_is_visible(0, None));
        assert!(source_player_is_visible(3, Some(1)));
        assert!(source_player_is_visible(-1, Some(1)));
        assert!(!source_player_is_visible(-1, None));
        assert!(!source_player_is_visible(3, Some(0)));
    }

    #[test]
    fn source_names_match_the_pinned_single_player_roster() {
        assert_eq!(
            super::source_names(),
            [
                "PLAYER", "Nyphur", "Rahn", "Belix", "Jark", "Elyssia", "Yragore", "Prish"
            ]
            .map(|name| Some(name.to_owned()))
        );
    }

    #[test]
    fn scoreboard_projection_preserves_source_row_values_and_palette() {
        let name = "Mala".to_owned();
        let kills8 = [4, 12, 0, 7, 2, 19, 1, 0];
        let row = project_score_row(
            2,
            27,
            Some(&name),
            Some((3, 144)),
            Some(kills8),
            MapType::Night,
        )
        .expect("complete active source row projects");
        let palette =
            campaign_hud_palette_indices(2, MapType::Night).expect("slot 2 has a source palette");

        assert_eq!(row.source_slot, 2);
        assert_eq!(row.display_name, "Mala");
        assert_eq!(row.max_mana, 144);
        assert_eq!(row.kills8, kills8);
        assert_eq!(row.outline_palette_index, palette[0]);
        assert_eq!(row.text_palette_index, palette[1]);
    }

    #[test]
    fn scoreboard_projection_rejects_a_missing_authoritative_kill_matrix() {
        let name = "Mala".to_owned();
        let error = project_score_row(2, 27, Some(&name), Some((3, 144)), None, MapType::Night)
            .expect_err("missing source matrix cannot become fabricated zero kills");

        assert_eq!(error, ProjectionError::ActivePlayerMissingKillMatrix(2));
    }
}
