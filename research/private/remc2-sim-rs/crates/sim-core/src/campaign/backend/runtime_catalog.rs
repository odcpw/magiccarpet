//! Source mission identities and composition of a closed installed catalog.
//!
//! The first four immutable semantic IDs remain unchanged. Further IDs name
//! the pinned LevelsNames_D9204 missions. The same table binds the map cache.

use super::*;
use sim_definition::runtime_flow::RuntimeFlowSecretEntry;

fn invalid(message: impl Into<String>) -> DefinitionError {
    definition_error(
        DefinitionErrorCode::InvalidSchema,
        "mc2_campaign.runtime_catalog",
        message,
    )
}

struct SourceMission {
    level_id: SemanticLevelId,
    map_type_id: u8,
}

fn catalog() -> crate::game_definition::Result<BTreeMap<u16, SourceMission>> {
    let mut result = BTreeMap::new();
    for line in include_str!("runtime_catalog.tsv")
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        let (index, tail) = line
            .split_once('\t')
            .ok_or_else(|| invalid("invalid catalog row"))?;
        let (name, map_type) = tail
            .split_once('\t')
            .ok_or_else(|| invalid("catalog row has no source map type"))?;
        let map_type_id = map_type
            .parse::<u8>()
            .map_err(|_| invalid("invalid source map type"))?;
        let index = index
            .parse::<u16>()
            .map_err(|_| invalid("invalid catalog source index"))?;
        if map_type_id > 2
            || !(index < 25 || (30..=34).contains(&index))
            || result
                .insert(
                    index,
                    SourceMission {
                        level_id: SemanticLevelId::parse(name)?,
                        map_type_id,
                    },
                )
                .is_some()
        {
            return Err(invalid("duplicate or non-campaign catalog source index"));
        }
    }
    if result.len() != 30 {
        return Err(invalid(
            "canonical source catalog must contain 25 ordinary and five secret missions",
        ));
    }
    Ok(result)
}

pub(super) fn compose(
    selection: CampaignSelection,
    ordinary_count: u16,
    secret_indices: &[u16],
) -> crate::game_definition::Result<RuntimeFlowDefinition> {
    if !(3..=25).contains(&ordinary_count) {
        return Err(invalid(
            "installed campaign requires a closed ordinary prefix of 3 through 25 missions",
        ));
    }
    let source_catalog = catalog()?;
    if source_catalog
        .get(&selection.internal_level)
        .map(|entry| entry.map_type_id)
        != Some(selection.map_type_id)
    {
        return Err(invalid(
            "package environment does not match the canonical source mission",
        ));
    }
    let catalog = source_catalog
        .into_iter()
        .map(|(index, mission)| (index, mission.level_id))
        .collect::<BTreeMap<_, _>>();
    let ordinary = CampaignSelection::ordinary(selection.external_level) == Some(selection);
    if !ordinary
        && CampaignSelection::secret_destination(selection.internal_level) != Some(selection)
    {
        return Err(invalid(
            "bound package is not a validated source campaign selection",
        ));
    }
    let bound = catalog
        .get(&selection.internal_level)
        .ok_or_else(|| invalid("unknown bound source mission"))?;
    let mut secrets = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut ordered = secret_indices.to_vec();
    ordered.sort_unstable();
    for index in ordered {
        if !seen.insert(index) {
            return Err(invalid("duplicate installed secret source mission"));
        }
        let association = (1..=ordinary_count)
            .filter_map(CampaignSelection::ordinary)
            .find_map(|origin| super::super::mc2_secret_flow::association_for(origin, index).map(|_| origin))
            .ok_or_else(|| invalid("installed secret has no source-associated ordinary origin in the closed prefix"))?;
        let destination = catalog
            .get(&index)
            .ok_or_else(|| invalid("unknown source secret mission"))?;
        secrets.push(RuntimeFlowSecretEntry::new(
            catalog[&association.internal_level].clone(),
            destination.clone(),
        ));
    }
    if (ordinary && selection.external_level > ordinary_count)
        || (!ordinary && !secret_indices.contains(&selection.internal_level))
    {
        return Err(invalid("bound package lies outside the installed catalog"));
    }
    let title = RuntimeFlowDestination::Title {
        node_id: RuntimeFlowNodeId::parse("flow.magic_carpet_2.return_to_title")?,
    };
    let entries = (0..ordinary_count)
        .map(|index| {
            RuntimeFlowEntry::new(
                catalog[&index].clone(),
                if index + 1 < ordinary_count {
                    RuntimeFlowDestination::Level {
                        level_id: catalog[&(index + 1)].clone(),
                    }
                } else {
                    title.clone()
                },
            )
        })
        .collect();
    let presentation_indices = (0..ordinary_count).chain(seen);
    let presentation = RuntimeFlowPresentationDefinition::new(
        SemanticPresentationCueId::parse(TITLE_PRESENTATION_CUE_ID)?,
        presentation_indices
            .map(|index| {
                let level = &catalog[&index];
                let name = level
                    .as_str()
                    .strip_prefix("level.")
                    .ok_or_else(|| invalid("invalid semantic level prefix"))?;
                Ok(RuntimeFlowPresentationEntry::new(
                    level.clone(),
                    SemanticPresentationCueId::parse(&format!("presentation.{name}.loading"))?,
                    SemanticPresentationCueId::parse(&format!("presentation.{name}.entry"))?,
                    SemanticPresentationCueId::parse(&format!("presentation.{name}.completion"))?,
                ))
            })
            .collect::<crate::game_definition::Result<Vec<_>>>()?,
    );
    RuntimeFlowDefinition::new_with_secret_entries(
        SemanticGameId::parse("game.magic_carpet_2")?,
        bound.clone(),
        catalog[&0].clone(),
        entries,
        secrets,
        presentation,
    )
}

/// Derive installed catalog dimensions exclusively from validated package inputs.
pub(super) fn installed(
    selections: &[CampaignSelection],
) -> crate::game_definition::Result<(u16, Vec<u16>)> {
    let mut ordinary = std::collections::BTreeSet::new();
    let mut secrets = std::collections::BTreeSet::new();
    for &selection in selections {
        let inserted = if CampaignSelection::ordinary(selection.external_level) == Some(selection) {
            ordinary.insert(selection.external_level)
        } else if CampaignSelection::secret_destination(selection.internal_level) == Some(selection)
        {
            secrets.insert(selection.internal_level)
        } else {
            return Err(invalid("installed package has an invalid source selection"));
        };
        if !inserted {
            return Err(invalid("duplicate installed source selection"));
        }
    }
    let count = ordinary
        .last()
        .copied()
        .ok_or_else(|| invalid("installed campaign has no ordinary origin"))?;
    if ordinary.len() != usize::from(count) {
        return Err(invalid("installed ordinary campaign prefix has a gap"));
    }
    let secrets = secrets.into_iter().collect::<Vec<_>>();
    compose(CampaignSelection::LEVEL1, count, &secrets)?;
    Ok((count, secrets))
}

/// Recover canonical dimensions from an observed flow without trusting its IDs.
pub(super) fn dimensions(
    flow: &RuntimeFlowDefinition,
) -> crate::game_definition::Result<(u16, Vec<u16>)> {
    let catalog = catalog()?;
    let count =
        u16::try_from(flow.entries().len()).map_err(|_| invalid("oversized ordinary catalog"))?;
    let secrets = flow
        .secret_entries()
        .iter()
        .map(|entry| {
            catalog
                .iter()
                .find_map(|(index, id)| {
                    (&id.level_id == entry.secret_level_id() && *index >= 30).then_some(*index)
                })
                .ok_or_else(|| invalid("noncanonical secret mission identity"))
        })
        .collect::<crate::game_definition::Result<Vec<_>>>()?;
    Ok((count, secrets))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_catalog_preserves_opening_prefixes_and_exact_environments() {
        let first = compose(CampaignSelection::LEVEL1, 3, &[]).unwrap();
        let cave = compose(CampaignSelection::LEVEL1, 4, &[]).unwrap();
        assert!(cave.continues_catalog(&first));
        assert!(!first.continues_catalog(&cave));
        let mut wrong_environment = CampaignSelection::LEVEL1;
        wrong_environment.map_type_id = 0;
        assert!(compose(wrong_environment, 3, &[]).is_err());
        let inputs = (1..=5)
            .map(|index| CampaignSelection::ordinary(index).unwrap())
            .chain([CampaignSelection::secret_destination(30).unwrap()])
            .collect::<Vec<_>>();
        assert_eq!(installed(&inputs).unwrap(), (5, vec![30]));
        let parent = compose(CampaignSelection::LEVEL1, 5, &[30]).unwrap();
        assert!(parent.continues_catalog(&cave));
        assert!(
            installed(
                &inputs[..5]
                    .iter()
                    .copied()
                    .chain([inputs[0]])
                    .collect::<Vec<_>>()
            )
            .is_err()
        );
    }

    #[test]
    fn all_five_secret_catalog_bindings_match_source_origins_and_environments() {
        let parent = compose(CampaignSelection::LEVEL1, 25, &[30, 31, 32, 33, 34]).unwrap();
        assert_eq!(parent.entries().len(), 25);
        assert_eq!(parent.secret_entries().len(), 5);
        for (internal, origin, map_type) in [
            (30, 5, 2),
            (31, 8, 1),
            (32, 12, 2),
            (33, 18, 2),
            (34, 20, 0),
        ] {
            let selection = CampaignSelection::secret_destination(internal).unwrap();
            assert_eq!(selection.map_type_id, map_type);
            let destination = compose(selection, 25, &[30, 31, 32, 33, 34]).unwrap();
            let origin = CampaignSelection::ordinary(origin).unwrap();
            let source = compose(origin, 25, &[30, 31, 32, 33, 34]).unwrap();
            assert!(
                super::super::super::mc2_secret_flow::entry_for(
                    origin,
                    internal,
                    &source,
                    &destination
                )
                .is_some()
            );
        }
    }

    #[test]
    fn source_origin_five_and_secret_thirty_share_one_closed_catalog() {
        let origin = CampaignSelection::ordinary(5).unwrap();
        let secret = CampaignSelection::secret_destination(30).unwrap();
        let parent = compose(origin, 5, &[30]).unwrap();
        let destination = compose(secret, 5, &[30]).unwrap();
        assert_eq!(
            parent.bound_level_id().as_str(),
            "level.magic_carpet_2.t_klom"
        );
        assert_eq!(
            destination.bound_level_id().as_str(),
            "level.magic_carpet_2.karakir"
        );
        assert!(
            super::super::super::mc2_secret_flow::entry_for(origin, 30, &parent, &destination)
                .is_some()
        );
        assert!(compose(origin, 5, &[31]).is_err());
        assert!(compose(secret, 5, &[]).is_err());
        assert!(installed(&[origin, secret]).is_err());
    }
}
