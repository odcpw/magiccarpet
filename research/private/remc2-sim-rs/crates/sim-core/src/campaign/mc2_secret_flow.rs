//! MC2 source portal identities mapped onto semantic secret-flow entries.
//!
//! The source table records zero-based map portal indices and source level
//! indices. It contains no authored mission names; the package owner supplies
//! the semantic IDs when binding an association.

use crate::{campaign::spec::CampaignSelection, game_definition::RuntimeFlowDefinition};
use sim_definition::runtime_flow::RuntimeFlowSecretEntry;

/// One source-authored link from an ordinary map portal to a hidden level.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Mc2SecretPortalAssociation {
    origin_portal_index: u16,
    destination_level_index: u16,
}

impl Mc2SecretPortalAssociation {
    #[must_use]
    pub(crate) const fn origin_portal_index(self) -> u16 {
        self.origin_portal_index
    }

    #[must_use]
    pub(crate) const fn destination_level_index(self) -> u16 {
        self.destination_level_index
    }
}

/// Source `secretMapScreenPortals_E2970` associations, excluding its sentinel.
///
/// `origin_portal_index` is the zero-based ordinary map-menu portal index;
/// `destination_level_index` is the source level index loaded by the secret
/// trigger. The five pairs match the pinned remc2 source table exactly.
const MC2_SECRET_PORTAL_ASSOCIATIONS: [Mc2SecretPortalAssociation; 5] = [
    Mc2SecretPortalAssociation {
        origin_portal_index: 4,
        destination_level_index: 30,
    },
    Mc2SecretPortalAssociation {
        origin_portal_index: 7,
        destination_level_index: 31,
    },
    Mc2SecretPortalAssociation {
        origin_portal_index: 11,
        destination_level_index: 32,
    },
    Mc2SecretPortalAssociation {
        origin_portal_index: 17,
        destination_level_index: 33,
    },
    Mc2SecretPortalAssociation {
        origin_portal_index: 19,
        destination_level_index: 34,
    },
];

/// Resolve a source portal association for the selected ordinary package.
///
/// Requiring a canonical ordinary selection rejects secret or malformed
/// source selectors before the semantic package binding is constructed.
#[must_use]
pub(crate) fn association_for(
    origin_selection: CampaignSelection,
    destination_level_index: u16,
) -> Option<Mc2SecretPortalAssociation> {
    let association = association_for_origin(origin_selection)?;
    (association.destination_level_index == destination_level_index).then_some(association)
}

/// Resolve the one source secret destination declared for an ordinary map.
#[must_use]
pub(crate) fn association_for_origin(
    origin_selection: CampaignSelection,
) -> Option<Mc2SecretPortalAssociation> {
    if CampaignSelection::ordinary(origin_selection.external_level) != Some(origin_selection) {
        return None;
    }

    MC2_SECRET_PORTAL_ASSOCIATIONS
        .iter()
        .copied()
        .find(|association| association.origin_portal_index == origin_selection.internal_level)
}

/// Resolve the source route and bind it to the validated parent package and
/// destination package identities.
#[must_use]
pub(crate) fn entry_for(
    origin_selection: CampaignSelection,
    destination_level_index: u16,
    parent_flow: &RuntimeFlowDefinition,
    destination_flow: &RuntimeFlowDefinition,
) -> Option<RuntimeFlowSecretEntry> {
    let association = association_for(origin_selection, destination_level_index)?;
    let origin_level_id = parent_flow.bound_level_id();
    if association.origin_portal_index != origin_selection.internal_level
        || !parent_flow
            .entries()
            .iter()
            .any(|entry| entry.level_id() == origin_level_id)
    {
        return None;
    }

    let destination_level_id = destination_flow.bound_level_id();
    let destination_entry = destination_flow.secret_entry(destination_level_id)?;
    if destination_entry.origin_level_id() != origin_level_id
        || !destination_flow.continues_catalog(parent_flow)
        || parent_flow.secret_entry(destination_level_id) != Some(destination_entry)
    {
        return None;
    }

    Some(destination_entry.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_definition::{
        RuntimeFlowDestination, RuntimeFlowEntry, RuntimeFlowNodeId,
        RuntimeFlowPresentationDefinition, RuntimeFlowPresentationEntry, SemanticGameId,
        SemanticLevelId, SemanticPresentationCueId, TITLE_PRESENTATION_CUE_ID,
    };
    use sim_definition::runtime_flow::RuntimeFlowSecretEntry;

    fn level(value: &str) -> SemanticLevelId {
        SemanticLevelId::parse(value).unwrap()
    }

    fn package_flow(bound_level_id: SemanticLevelId) -> RuntimeFlowDefinition {
        let origin = level("level.magic_carpet_2.origin_fixture");
        let secret = level("level.magic_carpet_2.destination_fixture");
        let title = SemanticPresentationCueId::parse(TITLE_PRESENTATION_CUE_ID).unwrap();
        let presentation = RuntimeFlowPresentationDefinition::new(
            title,
            [origin.clone(), secret.clone()]
                .into_iter()
                .map(|level_id| {
                    let cue = level_id.as_str().strip_prefix("level.").unwrap().to_owned();
                    RuntimeFlowPresentationEntry::new(
                        level_id,
                        SemanticPresentationCueId::parse(&format!("presentation.{cue}.loading"))
                            .unwrap(),
                        SemanticPresentationCueId::parse(&format!("presentation.{cue}.entry"))
                            .unwrap(),
                        SemanticPresentationCueId::parse(&format!("presentation.{cue}.completion"))
                            .unwrap(),
                    )
                })
                .collect(),
        );
        RuntimeFlowDefinition::new_with_secret_entries(
            SemanticGameId::parse("game.magic_carpet_2").unwrap(),
            bound_level_id,
            origin.clone(),
            vec![RuntimeFlowEntry::new(
                origin.clone(),
                RuntimeFlowDestination::Title {
                    node_id: RuntimeFlowNodeId::parse("flow.magic_carpet_2.return_to_title")
                        .unwrap(),
                },
            )],
            vec![RuntimeFlowSecretEntry::new(origin, secret)],
            presentation,
        )
        .unwrap()
    }

    #[test]
    fn source_portals_bind_only_the_five_declared_origin_destination_pairs() {
        let pairs = [
            (5, 4, 30),
            (8, 7, 31),
            (12, 11, 32),
            (18, 17, 33),
            (20, 19, 34),
        ];

        for (external_origin, portal_index, destination_index) in pairs {
            let selection = CampaignSelection::ordinary(external_origin).unwrap();
            let association = association_for_origin(selection).unwrap();
            assert_eq!(association.origin_portal_index(), portal_index);
            assert_eq!(association.destination_level_index(), destination_index);
            assert_eq!(
                association_for(selection, destination_index),
                Some(association)
            );
        }

        assert!(association_for(CampaignSelection::ordinary(6).unwrap(), 30).is_none());
        assert!(association_for(CampaignSelection::ordinary(5).unwrap(), 35).is_none());

        let mut malformed_origin = CampaignSelection::ordinary(5).unwrap();
        malformed_origin.map_type_id = malformed_origin.map_type_id.wrapping_add(1);
        assert!(association_for(malformed_origin, 30).is_none());
    }

    #[test]
    fn semantic_entry_requires_the_source_pair_and_matching_package_catalogs() {
        let origin = level("level.magic_carpet_2.origin_fixture");
        let secret = level("level.magic_carpet_2.destination_fixture");
        let parent = package_flow(origin.clone());
        let destination = package_flow(secret.clone());
        let selection = CampaignSelection::ordinary(5).unwrap();

        let entry = entry_for(selection, 30, &parent, &destination).unwrap();
        assert_eq!(entry.origin_level_id(), &origin);
        assert_eq!(entry.secret_level_id(), &secret);
        assert!(entry_for(selection, 31, &parent, &destination).is_none());
        assert!(entry_for(selection, 30, &parent, &parent).is_none());
    }
}
