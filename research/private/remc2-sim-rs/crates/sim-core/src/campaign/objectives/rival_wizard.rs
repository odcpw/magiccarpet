//! MC2 source-selector binding for rival-wizard objectives.
//!
//! The authored selector identifies a wizard color. Resolve that selector to
//! its stable game identity first, then bind it to the constructed session's
//! actual player roster and source arena. This keeps player colors and source
//! entity slots out of the semantic objective itself.

use crate::native_world::{EntitySnapshot, EntitySpecialRelation, PlayerSnapshot};
use sim_definition::level_definition::RivalWizardId;

/// Source player-color order from `GameUI.h::WizardsNames_D93A0`.
///
/// In single-player, `GetTrueWizardNumber_61790` returns the input color index
/// unchanged. Multiplayer's alternate color permutation is outside this
/// objective binding's scope.
const SOURCE_SINGLE_PLAYER_WIZARD_IDS: [&str; 8] = [
    "zanzamar", "nyphur", "rahn", "belix", "jark", "elyssia", "yragore", "prish",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceRivalWizardTarget {
    id: RivalWizardId,
    /// One-based MC2 stage selector retained for the source adapter.
    selector: u16,
    /// The source player-color index selected by `selector - 1`.
    player_color_index: u8,
}

impl SourceRivalWizardTarget {
    #[must_use]
    pub(crate) fn id(&self) -> &RivalWizardId {
        &self.id
    }

    #[must_use]
    pub(crate) const fn selector(&self) -> u16 {
        self.selector
    }

    #[must_use]
    pub(crate) const fn player_color_index(&self) -> u8 {
        self.player_color_index
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BoundRivalWizard {
    pub(crate) id: RivalWizardId,
    pub(crate) entity_index: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RivalWizardBindingError {
    InvalidSelector(u16),
    InvalidSourceIdentity(u8),
    MissingRosterPlayer(u8),
    AmbiguousRosterPlayer(u8),
    MissingArenaRow(u16),
    AmbiguousArenaRow(u16),
    InvalidArenaWizardRow {
        entity_index: u16,
        active: bool,
        class: u8,
        model: u8,
    },
    WrongArenaOwner {
        entity_index: u16,
        player_slot: u8,
        relation: EntitySpecialRelation,
    },
}

impl std::fmt::Display for RivalWizardBindingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSelector(selector) => {
                write!(
                    formatter,
                    "invalid one-based MC2 rival-wizard selector {selector}"
                )
            }
            Self::InvalidSourceIdentity(color) => write!(
                formatter,
                "source player color {color} has no stable single-player wizard identity"
            ),
            Self::MissingRosterPlayer(color) => write!(
                formatter,
                "no active player roster row has source player color {color}"
            ),
            Self::AmbiguousRosterPlayer(color) => write!(
                formatter,
                "multiple active player roster rows have source player color {color}"
            ),
            Self::MissingArenaRow(entity_index) => write!(
                formatter,
                "rival player entity {entity_index} is absent from the source arena"
            ),
            Self::AmbiguousArenaRow(entity_index) => write!(
                formatter,
                "source arena contains multiple rows for rival player entity {entity_index}"
            ),
            Self::InvalidArenaWizardRow {
                entity_index,
                active,
                class,
                model,
            } => write!(
                formatter,
                "rival player entity {entity_index} is not an active class-3 model-0/1 wizard row (active={active}, class={class}, model={model})"
            ),
            Self::WrongArenaOwner {
                entity_index,
                player_slot,
                relation,
            } => write!(
                formatter,
                "rival player entity {entity_index} has owner relation {relation:?}, expected player slot {player_slot}"
            ),
        }
    }
}

impl std::error::Error for RivalWizardBindingError {}

/// Bind one authored one-based player selector to a stable wizard identity.
///
/// # Errors
///
/// Returns an error when the selector is outside the eight source wizard
/// identities or the corresponding stable ID cannot be constructed.
pub(crate) fn target_from_selector(
    selector: u16,
) -> Result<SourceRivalWizardTarget, RivalWizardBindingError> {
    let Some(color_index) = selector
        .checked_sub(1)
        .and_then(|index| u8::try_from(index).ok())
    else {
        return Err(RivalWizardBindingError::InvalidSelector(selector));
    };
    let Some(identity) = SOURCE_SINGLE_PLAYER_WIZARD_IDS
        .get(usize::from(color_index))
        .copied()
    else {
        return Err(RivalWizardBindingError::InvalidSelector(selector));
    };
    let id = RivalWizardId::parse(&format!("rival_wizard.{identity}"))
        .map_err(|_| RivalWizardBindingError::InvalidSourceIdentity(color_index))?;
    Ok(SourceRivalWizardTarget {
        id,
        selector,
        player_color_index: color_index,
    })
}

/// Resolve a semantic rival target against the actual initialized session.
///
/// The target player is selected by the source player-color field, while its
/// live identity comes from the `PlayerSnapshot` entity relation and the
/// matching active class-3/model-0-or-1 row in the authoritative source arena.
///
/// # Errors
///
/// Returns an error for a missing or ambiguous roster entry, or an absent,
/// ambiguous, inactive, non-wizard, or wrongly-owned arena row.
pub(crate) fn bind_target(
    target: &SourceRivalWizardTarget,
    players: &[PlayerSnapshot],
    arena_entities: &[EntitySnapshot],
) -> Result<BoundRivalWizard, RivalWizardBindingError> {
    let mut roster_matches = players.iter().filter(|player| {
        // The eight exported player records include cleared spare rows with
        // color zero and ActPlayerIndex zero. Source sub_58DA0 sees only
        // constructed wizard entities; its null entity relation is not one.
        player.entity_index != 0
            && player.active_player_index >= 0
            && player.player_color_index() == target.player_color_index
    });
    let Some(player) = roster_matches.next() else {
        return Err(RivalWizardBindingError::MissingRosterPlayer(
            target.player_color_index,
        ));
    };
    if roster_matches.next().is_some() {
        return Err(RivalWizardBindingError::AmbiguousRosterPlayer(
            target.player_color_index,
        ));
    }
    let mut arena_matches = arena_entities
        .iter()
        .filter(|entity| entity.index == player.entity_index);
    let Some(entity) = arena_matches.next() else {
        return Err(RivalWizardBindingError::MissingArenaRow(
            player.entity_index,
        ));
    };
    if arena_matches.next().is_some() {
        return Err(RivalWizardBindingError::AmbiguousArenaRow(
            player.entity_index,
        ));
    }
    if !entity.active || entity.class != 3 || !matches!(entity.model, 0 | 1) {
        return Err(RivalWizardBindingError::InvalidArenaWizardRow {
            entity_index: player.entity_index,
            active: entity.active,
            class: entity.class,
            model: entity.model,
        });
    }
    if entity.special_relation != EntitySpecialRelation::PlayerState(player.slot) {
        return Err(RivalWizardBindingError::WrongArenaOwner {
            entity_index: player.entity_index,
            player_slot: player.slot,
            relation: entity.special_relation,
        });
    }

    Ok(BoundRivalWizard {
        id: target.id.clone(),
        entity_index: player.entity_index,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{
        campaign::{CampaignSelection, CampaignSession},
        native_world::EntitySpecialRelation,
    };

    use super::{RivalWizardBindingError, bind_target, target_from_selector};

    fn level_two_session() -> crate::native_world::Mc2Simulation {
        let package = std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
            },
            PathBuf::from,
        );
        CampaignSession::open(package, CampaignSelection::LEVEL2)
            .expect("qualified Level 2 package")
            .simulation()
            .clone()
    }

    #[test]
    fn source_selector_uses_single_player_wizard_identity_without_level_special_case() {
        let expected = [
            "rival_wizard.zanzamar",
            "rival_wizard.nyphur",
            "rival_wizard.rahn",
            "rival_wizard.belix",
            "rival_wizard.jark",
            "rival_wizard.elyssia",
            "rival_wizard.yragore",
            "rival_wizard.prish",
        ];
        for (index, expected_id) in expected.into_iter().enumerate() {
            let selector = u16::try_from(index + 1).unwrap();
            let target = target_from_selector(selector).unwrap();
            assert_eq!(target.id().as_str(), expected_id);
            assert_eq!(target.selector(), selector);
            assert_eq!(target.player_color_index(), u8::try_from(index).unwrap());
        }
    }

    #[test]
    fn invalid_source_selector_is_rejected() {
        assert_eq!(
            target_from_selector(0),
            Err(RivalWizardBindingError::InvalidSelector(0))
        );
        assert_eq!(
            target_from_selector(9),
            Err(RivalWizardBindingError::InvalidSelector(9))
        );
    }

    #[test]
    fn selector_without_an_active_real_roster_player_is_rejected() {
        let target = target_from_selector(2).unwrap();
        assert_eq!(
            bind_target(&target, &[], &[]),
            Err(RivalWizardBindingError::MissingRosterPlayer(1))
        );
    }

    #[test]
    fn real_level_two_roster_binds_nyphur_and_local_player_and_rejects_bad_owners() {
        let session = level_two_session();

        let nyphur_target = target_from_selector(2).unwrap();
        let nyphur = bind_target(&nyphur_target, session.players(), session.entities()).unwrap();
        assert_eq!(nyphur.id.as_str(), "rival_wizard.nyphur");
        assert_eq!(nyphur.entity_index, 112);
        let nyphur_player = session
            .players()
            .iter()
            .find(|player| player.entity_index == nyphur.entity_index)
            .expect("source roster contains the bound Nyphur player");
        assert!(nyphur_player.is_ai);
        assert_eq!(
            nyphur_player.player_color_index(),
            nyphur_target.player_color_index()
        );
        let nyphur_entity = &session.entities()[usize::from(nyphur.entity_index)];
        assert!(nyphur_entity.active);
        assert_eq!(nyphur_entity.class, 3);
        assert!(matches!(nyphur_entity.model, 0 | 1));
        assert_eq!(
            nyphur_entity.special_relation,
            EntitySpecialRelation::PlayerState(nyphur_player.slot)
        );

        let local_target = target_from_selector(1).unwrap();
        let local_player = session
            .players()
            .iter()
            .find(|player| player.player_color_index() == local_target.player_color_index())
            .expect("selector 1 has a source player roster row");
        assert!(!local_player.is_ai);
        let local = bind_target(&local_target, session.players(), session.entities()).unwrap();
        assert_eq!(local.id.as_str(), "rival_wizard.zanzamar");
        assert_eq!(local.entity_index, local_player.entity_index);

        let mut wrong_owner_entities = session.entities().to_vec();
        let wrong_owner_slot = if nyphur_player.slot == 0 { 1 } else { 0 };
        wrong_owner_entities[usize::from(nyphur.entity_index)].special_relation =
            EntitySpecialRelation::PlayerState(wrong_owner_slot);
        assert_eq!(
            bind_target(&nyphur_target, session.players(), &wrong_owner_entities),
            Err(RivalWizardBindingError::WrongArenaOwner {
                entity_index: nyphur.entity_index,
                player_slot: nyphur_player.slot,
                relation: EntitySpecialRelation::PlayerState(wrong_owner_slot),
            })
        );

        let missing_nyphur_roster = session
            .players()
            .iter()
            .filter(|player| player.player_color_index() != nyphur_target.player_color_index())
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            bind_target(&nyphur_target, &missing_nyphur_roster, session.entities()),
            Err(RivalWizardBindingError::MissingRosterPlayer(
                nyphur_target.player_color_index()
            ))
        );
    }
}
