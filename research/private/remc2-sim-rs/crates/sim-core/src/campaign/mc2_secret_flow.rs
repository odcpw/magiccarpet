//! MC2 source portal identities mapped onto semantic secret-flow entries.
//!
//! The source table records zero-based map portal indices and source level
//! indices. It contains no authored mission names; the package owner supplies
//! the semantic IDs when binding an association.

use crate::{
    campaign::spec::CampaignSelection,
    game_definition::RuntimeFlowDefinition,
    game_runtime::{CampaignSaveState, RuntimeTransitionError},
};
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

/// Apply the MC2 map's associated ordinary-portal activation after a secret
/// return. The caller supplies the exact validated active secret package and
/// stages this change on the retained parent before publishing the return.
pub(crate) fn activate_parent_after_secret_return(
    source: CampaignSelection,
    parent: &mut CampaignSaveState,
) -> Result<(), RuntimeTransitionError> {
    let invalid = || RuntimeTransitionError::InvalidRuntimeFlow {
        message: "secret return does not match its retained MC2 parent portal".to_owned(),
    };
    if CampaignSelection::secret_destination(source.internal_level) != Some(source) {
        return Err(invalid());
    }
    let association = MC2_SECRET_PORTAL_ASSOCIATIONS
        .iter()
        .find(|entry| entry.destination_level_index == source.internal_level)
        .ok_or_else(invalid)?;
    let flow = parent.definition.runtime_flow().ok_or_else(invalid)?;
    let origin = flow
        .entries()
        .get(usize::from(association.origin_portal_index))
        .map(|entry| entry.level_id())
        .ok_or_else(invalid)?;
    if parent.selected_level_id.as_ref() != Some(origin)
        || !flow
            .secret_entries()
            .iter()
            .any(|entry| entry.origin_level_id() == origin)
    {
        return Err(invalid());
    }
    if !parent.completed_levels.contains(origin) {
        parent.completed_levels.push(origin.clone());
    }
    Ok(())
}

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

    #[test]
    fn secret_return_activates_only_its_selected_parent_once() {
        use crate::{
            campaign::backend::{Mc2CampaignInputs, Mc2RuntimePackageResolver, load_mc2_campaign},
            game_definition::default_catalog,
            game_runtime::CampaignMenuPhase,
        };
        use std::path::Path;

        let generated = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let resolver = Mc2RuntimePackageResolver::load(
            [
                "mc-bhq8-level1-collision-v5-night",
                "level2-day-block32-campaign-v2",
                "level3-day-block32-campaign-recipe-v2",
                "mc-bhq6-ordinary-20261025-zclu5fjj/level4",
                "mc-paj2-source4-campaign-package",
                "mc-paj2-source30-campaign-package",
            ]
            .map(|path| Mc2CampaignInputs::new(generated.join(path))),
        )
        .unwrap();
        let definition = load_mc2_campaign(
            generated.join("mc-bhq8-level1-collision-v5-night"),
            &default_catalog().unwrap(),
        )
        .unwrap();
        let flow = resolver.flow().clone();
        let ordinary: Vec<_> = flow
            .entries()
            .iter()
            .map(|entry| entry.level_id().clone())
            .collect();
        let definition = sim_definition::adapter::with_level_and_runtime_flow(
            definition.clone(),
            definition.level_definition().unwrap().clone(),
            flow,
        );
        let mut parent = CampaignSaveState {
            definition,
            phase: CampaignMenuPhase::MissionSelect,
            completed_level_id: Some(ordinary[0].clone()),
            selected_level_id: Some(ordinary[4].clone()),
            completed_levels: ordinary[..4].to_vec(),
            level_statistics: Vec::new(),
        };
        let original = parent.clone();
        assert!(
            activate_parent_after_secret_return(
                CampaignSelection::SECRET_DESTINATION_31,
                &mut parent
            )
            .is_err()
        );
        assert_eq!(parent, original);
        parent.selected_level_id = Some(ordinary[3].clone());
        let wrong_selection = parent.clone();
        assert!(
            activate_parent_after_secret_return(
                CampaignSelection::SECRET_DESTINATION_30,
                &mut parent
            )
            .is_err()
        );
        assert_eq!(parent, wrong_selection);
        parent = original;
        activate_parent_after_secret_return(CampaignSelection::SECRET_DESTINATION_30, &mut parent)
            .unwrap();
        activate_parent_after_secret_return(CampaignSelection::SECRET_DESTINATION_30, &mut parent)
            .unwrap();
        assert_eq!(parent.completed_levels, ordinary);
        assert_eq!(
            parent.completed_level_id,
            Some(parent.completed_levels[0].clone())
        );
        assert!(parent.level_statistics.is_empty());
    }
}
