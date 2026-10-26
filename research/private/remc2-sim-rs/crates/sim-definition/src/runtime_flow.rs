//! Stable semantic identity for one playable level and its runtime flow.
//!
//! The flow is immutable package meaning. It contains no package path or
//! private source selector, and it does not itself execute transitions.

use crate::{DefinitionError, DefinitionErrorCode, Result, SemanticId};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;

macro_rules! semantic_flow_id {
    ($name:ident, $namespace:literal) => {
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(SemanticId);

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(&value).map_err(serde::de::Error::custom)
            }
        }

        impl $name {
            /// Parse a canonical semantic flow identifier in its closed
            /// namespace.
            ///
            /// # Errors
            ///
            /// Returns an error for an invalid semantic ID, the wrong
            /// namespace, or a private source-shaped segment.
            pub fn parse(value: &str) -> Result<Self> {
                let id = SemanticId::parse(value)?;
                if !value.starts_with(concat!($namespace, ".")) {
                    return Err(DefinitionError::new(
                        DefinitionErrorCode::InvalidSemanticId,
                        value,
                        concat!("runtime flow ID must use the ", $namespace, ". namespace"),
                    ));
                }
                if value
                    .split('.')
                    .any(crate::level_definition::is_private_source_identity_segment)
                {
                    return Err(DefinitionError::new(
                        DefinitionErrorCode::InvalidSemanticId,
                        value,
                        "runtime flow ID cannot expose private source identity",
                    ));
                }
                Ok(Self(id))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }
    };
}

semantic_flow_id!(SemanticGameId, "game");
semantic_flow_id!(SemanticLevelId, "level");
semantic_flow_id!(RuntimeFlowNodeId, "flow");
semantic_flow_id!(SemanticPresentationCueId, "presentation");

pub const TITLE_PRESENTATION_CUE_ID: &str = "presentation.campaign.title";

/// Closed semantic presentation bindings for one playable level.
///
/// These IDs name package presentation slots, never source selectors or
/// owner-cache paths. The runtime selects them; a client may only observe
/// the selected cue together with its semantic level context.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeFlowPresentationEntry {
    level_id: SemanticLevelId,
    loading_cue: SemanticPresentationCueId,
    entry_cue: SemanticPresentationCueId,
    completion_cue: SemanticPresentationCueId,
}

impl RuntimeFlowPresentationEntry {
    #[must_use]
    pub const fn new(
        level_id: SemanticLevelId,
        loading_cue: SemanticPresentationCueId,
        entry_cue: SemanticPresentationCueId,
        completion_cue: SemanticPresentationCueId,
    ) -> Self {
        Self {
            level_id,
            loading_cue,
            entry_cue,
            completion_cue,
        }
    }

    #[must_use]
    pub const fn level_id(&self) -> &SemanticLevelId {
        &self.level_id
    }

    #[must_use]
    pub const fn loading_cue(&self) -> &SemanticPresentationCueId {
        &self.loading_cue
    }

    #[must_use]
    pub const fn entry_cue(&self) -> &SemanticPresentationCueId {
        &self.entry_cue
    }

    #[must_use]
    pub const fn completion_cue(&self) -> &SemanticPresentationCueId {
        &self.completion_cue
    }
}

/// Presentation cue catalog bound to the same closed ordered level flow.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeFlowPresentationDefinition {
    title_cue: SemanticPresentationCueId,
    entries: Vec<RuntimeFlowPresentationEntry>,
}

impl RuntimeFlowPresentationDefinition {
    #[must_use]
    pub const fn new(
        title_cue: SemanticPresentationCueId,
        entries: Vec<RuntimeFlowPresentationEntry>,
    ) -> Self {
        Self { title_cue, entries }
    }

    #[must_use]
    pub const fn title_cue(&self) -> &SemanticPresentationCueId {
        &self.title_cue
    }

    #[must_use]
    pub fn entries(&self) -> &[RuntimeFlowPresentationEntry] {
        &self.entries
    }

    #[must_use]
    pub fn entry(&self, level_id: &SemanticLevelId) -> Option<&RuntimeFlowPresentationEntry> {
        self.entries
            .iter()
            .find(|entry| entry.level_id() == level_id)
    }
}

/// Declared destination after one semantic level completes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeFlowDestination {
    Level { level_id: SemanticLevelId },
    Title { node_id: RuntimeFlowNodeId },
}

/// One ordered playable entry in a runtime flow.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeFlowEntry {
    level_id: SemanticLevelId,
    completion_destination: RuntimeFlowDestination,
}

impl RuntimeFlowEntry {
    #[must_use]
    pub const fn new(
        level_id: SemanticLevelId,
        completion_destination: RuntimeFlowDestination,
    ) -> Self {
        Self {
            level_id,
            completion_destination,
        }
    }

    #[must_use]
    pub const fn level_id(&self) -> &SemanticLevelId {
        &self.level_id
    }

    #[must_use]
    pub const fn completion_destination(&self) -> &RuntimeFlowDestination {
        &self.completion_destination
    }
}

/// One optional secret mission reachable from an ordinary campaign mission.
///
/// The association is semantic package meaning. Source portal numbers and
/// trigger actions stay inside the source adapter that produces the request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeFlowSecretEntry {
    origin_level_id: SemanticLevelId,
    secret_level_id: SemanticLevelId,
}

impl RuntimeFlowSecretEntry {
    #[must_use]
    pub const fn new(origin_level_id: SemanticLevelId, secret_level_id: SemanticLevelId) -> Self {
        Self {
            origin_level_id,
            secret_level_id,
        }
    }

    #[must_use]
    pub const fn origin_level_id(&self) -> &SemanticLevelId {
        &self.origin_level_id
    }

    #[must_use]
    pub const fn secret_level_id(&self) -> &SemanticLevelId {
        &self.secret_level_id
    }
}

/// Closed ordered runtime-flow catalog plus the level bound by this one-level
/// game definition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeFlowDefinition {
    game_id: SemanticGameId,
    bound_level_id: SemanticLevelId,
    entry_level_id: SemanticLevelId,
    entries: Vec<RuntimeFlowEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    secret_entries: Vec<RuntimeFlowSecretEntry>,
    presentation: RuntimeFlowPresentationDefinition,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeFlowDefinitionWire {
    game_id: SemanticGameId,
    bound_level_id: SemanticLevelId,
    entry_level_id: SemanticLevelId,
    entries: Vec<RuntimeFlowEntry>,
    #[serde(default)]
    secret_entries: Vec<RuntimeFlowSecretEntry>,
    presentation: RuntimeFlowPresentationDefinition,
}

impl<'de> Deserialize<'de> for RuntimeFlowDefinition {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = RuntimeFlowDefinitionWire::deserialize(deserializer)?;
        Self::new_with_secret_entries(
            wire.game_id,
            wire.bound_level_id,
            wire.entry_level_id,
            wire.entries,
            wire.secret_entries,
            wire.presentation,
        )
        .map_err(serde::de::Error::custom)
    }
}

impl RuntimeFlowDefinition {
    /// Construct and validate one closed, linear, terminating runtime flow.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate, dangling, cyclic, skipped, unbound, or
    /// non-terminating entries.
    pub fn new(
        game_id: SemanticGameId,
        bound_level_id: SemanticLevelId,
        entry_level_id: SemanticLevelId,
        entries: Vec<RuntimeFlowEntry>,
        presentation: RuntimeFlowPresentationDefinition,
    ) -> Result<Self> {
        Self::new_with_secret_entries(
            game_id,
            bound_level_id,
            entry_level_id,
            entries,
            Vec::new(),
            presentation,
        )
    }

    /// Construct a closed ordered ordinary flow plus its optional secret
    /// mission associations.
    ///
    /// A package may bind either an ordinary entry or one of the declared
    /// secret missions. Secret missions remain outside the ordinary frontier
    /// and do not alter its linear completion edges.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate or dangling ordinary entries, invalid
    /// secret associations, missing presentation bindings, cycles, skipped
    /// ordinary entries, or a non-terminating ordinary flow.
    pub fn new_with_secret_entries(
        game_id: SemanticGameId,
        bound_level_id: SemanticLevelId,
        entry_level_id: SemanticLevelId,
        entries: Vec<RuntimeFlowEntry>,
        secret_entries: Vec<RuntimeFlowSecretEntry>,
        presentation: RuntimeFlowPresentationDefinition,
    ) -> Result<Self> {
        let flow = Self {
            game_id,
            bound_level_id,
            entry_level_id,
            entries,
            secret_entries,
            presentation,
        };
        flow.verify()?;
        Ok(flow)
    }

    #[must_use]
    pub const fn game_id(&self) -> &SemanticGameId {
        &self.game_id
    }

    #[must_use]
    pub const fn bound_level_id(&self) -> &SemanticLevelId {
        &self.bound_level_id
    }

    #[must_use]
    pub const fn entry_level_id(&self) -> &SemanticLevelId {
        &self.entry_level_id
    }

    #[must_use]
    pub fn entries(&self) -> &[RuntimeFlowEntry] {
        &self.entries
    }

    #[must_use]
    pub fn secret_entries(&self) -> &[RuntimeFlowSecretEntry] {
        &self.secret_entries
    }

    #[must_use]
    pub fn secret_entry(
        &self,
        secret_level_id: &SemanticLevelId,
    ) -> Option<&RuntimeFlowSecretEntry> {
        self.secret_entries
            .iter()
            .find(|entry| entry.secret_level_id() == secret_level_id)
    }

    #[must_use]
    pub fn contains_level(&self, level_id: &SemanticLevelId) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.level_id() == level_id)
            || self.secret_entry(level_id).is_some()
    }

    #[must_use]
    pub const fn presentation(&self) -> &RuntimeFlowPresentationDefinition {
        &self.presentation
    }

    /// Whether this catalog retains the active campaign while appending ordinary
    /// entries or secret associations. Existing entries, associations, edges,
    /// and presentation cues stay identical; only the old terminal edge may lead
    /// into an appended ordinary suffix. Bound levels are checked by the caller.
    #[must_use]
    pub fn continues_catalog(&self, previous: &Self) -> bool {
        if self.game_id != previous.game_id
            || self.entry_level_id != previous.entry_level_id
            || self.presentation.title_cue != previous.presentation.title_cue
            || self.secret_entries.len() < previous.secret_entries.len()
            || self.secret_entries[..previous.secret_entries.len()] != previous.secret_entries
        {
            return false;
        }
        if self.entries == previous.entries {
            return previous
                .presentation
                .entries
                .iter()
                .all(|entry| self.presentation.entry(entry.level_id()) == Some(entry));
        }
        let count = previous.entries.len();
        if count == 0 || self.entries.len() <= count {
            return false;
        }
        self.entries[..count - 1] == previous.entries[..count - 1]
            && self.entries[count - 1].level_id == previous.entries[count - 1].level_id
            && matches!(
                &previous.entries[count - 1].completion_destination,
                RuntimeFlowDestination::Title { .. }
            )
            && self
                .entries
                .last()
                .map(|entry| &entry.completion_destination)
                == Some(&previous.entries[count - 1].completion_destination)
            && previous
                .presentation
                .entries
                .iter()
                .all(|entry| self.presentation.entry(entry.level_id()) == Some(entry))
    }

    pub(crate) fn verify(&self) -> Result<()> {
        if self.entries.is_empty() {
            return Err(flow_error(
                "runtime_flow.entries",
                "runtime flow has no playable entry",
            ));
        }
        if self.presentation.title_cue.as_str() != TITLE_PRESENTATION_CUE_ID {
            return Err(flow_error(
                self.presentation.title_cue.as_str(),
                "runtime flow title presentation must use the engine-owned campaign title slot",
            ));
        }
        if self.presentation.entries.len() != self.entries.len() + self.secret_entries.len() {
            return Err(flow_error(
                "runtime_flow.presentation.entries",
                "presentation bindings must cover every ordinary and secret runtime-flow entry exactly once",
            ));
        }
        let mut presentation_ids: BTreeSet<&SemanticLevelId> = BTreeSet::new();
        for presentation in &self.presentation.entries {
            if !presentation_ids.insert(presentation.level_id()) {
                return Err(flow_error(
                    presentation.level_id.as_str(),
                    "presentation bindings duplicate a semantic level",
                ));
            }
            if !self.contains_level(presentation.level_id()) {
                return Err(flow_error(
                    presentation.level_id.as_str(),
                    "presentation binding names a level absent from the runtime flow",
                ));
            }
        }
        let mut ids: BTreeSet<&SemanticLevelId> = BTreeSet::new();
        for entry in &self.entries {
            if !ids.insert(entry.level_id()) {
                return Err(flow_error(
                    entry.level_id.as_str(),
                    "runtime flow level ID is duplicated",
                ));
            }
        }
        let ordinary_ids = ids.clone();
        for entry in &self.secret_entries {
            if !ordinary_ids
                .iter()
                .any(|level_id| *level_id == entry.origin_level_id())
            {
                return Err(flow_error(
                    entry.origin_level_id().as_str(),
                    "secret mission origin is absent from the ordinary runtime flow",
                ));
            }
            if !ids.insert(entry.secret_level_id()) {
                return Err(flow_error(
                    entry.secret_level_id().as_str(),
                    "secret mission duplicates an ordinary or secret level ID",
                ));
            }
        }
        if !ordinary_ids
            .iter()
            .any(|level_id| *level_id == &self.entry_level_id)
        {
            return Err(flow_error(
                self.entry_level_id.as_str(),
                "runtime flow entry level is absent from the ordinary campaign",
            ));
        }
        if self.entries[0].level_id != self.entry_level_id {
            return Err(flow_error(
                self.entry_level_id.as_str(),
                "runtime flow entry must be the first ordered playable level",
            ));
        }
        for entry in &self.entries {
            if let RuntimeFlowDestination::Level { level_id } = &entry.completion_destination
                && !ordinary_ids.iter().any(|entry_id| *entry_id == level_id)
            {
                return Err(flow_error(
                    level_id.as_str(),
                    "runtime flow destination is absent",
                ));
            }
        }

        let mut visited = BTreeSet::new();
        let mut current = &self.entry_level_id;
        loop {
            if !visited.insert(current) {
                return Err(flow_error(
                    current.as_str(),
                    "runtime flow contains a cycle",
                ));
            }
            let entry = self
                .entries
                .iter()
                .find(|entry| &entry.level_id == current)
                .expect("validated runtime flow destination exists");
            match &entry.completion_destination {
                RuntimeFlowDestination::Level { level_id } => current = level_id,
                RuntimeFlowDestination::Title { .. } => break,
            }
        }
        if visited.len() != self.entries.len() {
            return Err(flow_error(
                "runtime_flow.entries",
                "runtime flow contains a skipped or unreachable playable level",
            ));
        }
        for pair in self.entries.windows(2) {
            if pair[0].completion_destination
                != (RuntimeFlowDestination::Level {
                    level_id: pair[1].level_id.clone(),
                })
            {
                return Err(flow_error(
                    pair[0].level_id.as_str(),
                    "runtime flow entry does not target the next ordered playable level",
                ));
            }
        }
        if !matches!(
            self.entries
                .last()
                .map(|entry| &entry.completion_destination),
            Some(RuntimeFlowDestination::Title { .. })
        ) {
            return Err(flow_error(
                "runtime_flow.entries",
                "final runtime flow entry must return to Title",
            ));
        }
        if !ids.iter().any(|level_id| *level_id == &self.bound_level_id) {
            return Err(flow_error(
                self.bound_level_id.as_str(),
                "bound semantic level is absent from the ordinary and secret runtime flow",
            ));
        }
        Ok(())
    }
}

fn flow_error(path: &str, message: &str) -> DefinitionError {
    DefinitionError::new(DefinitionErrorCode::InvalidSchema, path, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(value: &str) -> SemanticLevelId {
        SemanticLevelId::parse(value).unwrap()
    }

    fn entry(value: &str, destination: RuntimeFlowDestination) -> RuntimeFlowEntry {
        RuntimeFlowEntry::new(level(value), destination)
    }

    fn presentation(values: &[&str]) -> RuntimeFlowPresentationDefinition {
        RuntimeFlowPresentationDefinition::new(
            SemanticPresentationCueId::parse(TITLE_PRESENTATION_CUE_ID).unwrap(),
            values
                .iter()
                .map(|value| {
                    let cue_stem = value.strip_prefix("level.").unwrap();
                    RuntimeFlowPresentationEntry::new(
                        level(value),
                        SemanticPresentationCueId::parse(&format!(
                            "presentation.{cue_stem}.loading"
                        ))
                        .unwrap(),
                        SemanticPresentationCueId::parse(&format!("presentation.{cue_stem}.entry"))
                            .unwrap(),
                        SemanticPresentationCueId::parse(&format!(
                            "presentation.{cue_stem}.completion"
                        ))
                        .unwrap(),
                    )
                })
                .collect(),
        )
    }

    fn valid_flow() -> RuntimeFlowDefinition {
        RuntimeFlowDefinition::new(
            SemanticGameId::parse("game.magic_carpet_2").unwrap(),
            level("level.magic_carpet_2.night_spire"),
            level("level.magic_carpet_2.night_spire"),
            vec![
                entry(
                    "level.magic_carpet_2.night_spire",
                    RuntimeFlowDestination::Level {
                        level_id: level("level.magic_carpet_2.payahandra_ruins"),
                    },
                ),
                entry(
                    "level.magic_carpet_2.payahandra_ruins",
                    RuntimeFlowDestination::Level {
                        level_id: level("level.magic_carpet_2.revealed_way"),
                    },
                ),
                entry(
                    "level.magic_carpet_2.revealed_way",
                    RuntimeFlowDestination::Title {
                        node_id: RuntimeFlowNodeId::parse("flow.magic_carpet_2.return_to_title")
                            .unwrap(),
                    },
                ),
            ],
            presentation(&[
                "level.magic_carpet_2.night_spire",
                "level.magic_carpet_2.payahandra_ruins",
                "level.magic_carpet_2.revealed_way",
            ]),
        )
        .unwrap()
    }

    fn flow_with_one_secret(bound_level_id: SemanticLevelId) -> RuntimeFlowDefinition {
        let ordinary = valid_flow();
        let secret = level("level.magic_carpet_2.secret_fixture");
        let mut presentations = ordinary.presentation().entries().to_vec();
        presentations.extend(presentation(&[secret.as_str()]).entries().iter().cloned());
        RuntimeFlowDefinition::new_with_secret_entries(
            ordinary.game_id().clone(),
            bound_level_id,
            ordinary.entry_level_id().clone(),
            ordinary.entries().to_vec(),
            vec![RuntimeFlowSecretEntry::new(
                level("level.magic_carpet_2.payahandra_ruins"),
                secret,
            )],
            RuntimeFlowPresentationDefinition::new(
                ordinary.presentation().title_cue().clone(),
                presentations,
            ),
        )
        .unwrap()
    }

    #[test]
    fn catalog_extension_preserves_prior_edges_cues_and_terminal() {
        let previous = valid_flow();
        let mut extended = previous.clone();
        let cave = level("level.magic_carpet_2.spider_cave");
        let terminal = extended
            .entries
            .last()
            .unwrap()
            .completion_destination
            .clone();
        extended.entries.last_mut().unwrap().completion_destination =
            RuntimeFlowDestination::Level {
                level_id: cave.clone(),
            };
        extended
            .entries
            .push(RuntimeFlowEntry::new(cave.clone(), terminal));
        extended.presentation.entries.push(
            presentation(&["level.magic_carpet_2.spider_cave"])
                .entries
                .remove(0),
        );
        extended.bound_level_id = cave;
        extended.verify().unwrap();
        assert!(extended.continues_catalog(&previous));
        assert!(!previous.continues_catalog(&extended));

        let mut changed_cue = extended.clone();
        changed_cue.presentation.entries[0].entry_cue =
            SemanticPresentationCueId::parse("presentation.changed.entry").unwrap();
        changed_cue.verify().unwrap();
        assert!(!changed_cue.continues_catalog(&previous));

        let mut changed_terminal = extended.clone();
        changed_terminal
            .entries
            .last_mut()
            .unwrap()
            .completion_destination = RuntimeFlowDestination::Title {
            node_id: RuntimeFlowNodeId::parse("flow.changed.title").unwrap(),
        };
        changed_terminal.verify().unwrap();
        assert!(!changed_terminal.continues_catalog(&previous));

        let mut changed_game = extended;
        changed_game.game_id = SemanticGameId::parse("game.another_campaign").unwrap();
        changed_game.verify().unwrap();
        assert!(!changed_game.continues_catalog(&previous));
    }

    #[test]
    fn secret_missions_are_closed_associations_outside_the_ordinary_frontier() {
        let secret = level("level.magic_carpet_2.secret_fixture");
        let flow = flow_with_one_secret(secret.clone());
        assert_eq!(flow.entries().len(), 3);
        assert_eq!(flow.secret_entries().len(), 1);
        assert!(flow.contains_level(&secret));
        assert_eq!(
            flow.secret_entry(&secret)
                .unwrap()
                .origin_level_id()
                .as_str(),
            "level.magic_carpet_2.payahandra_ruins"
        );
        assert_eq!(
            flow.bound_level_id(),
            &secret,
            "secret packages bind the associated secret ID without joining the ordinary frontier"
        );
        assert!(flow.continues_catalog(&valid_flow()));
        assert!(!valid_flow().continues_catalog(&flow));
        assert_eq!(
            serde_json::from_str::<RuntimeFlowDefinition>(&serde_json::to_string(&flow).unwrap())
                .unwrap(),
            flow
        );

        let ordinary = valid_flow();
        let mut presentations = ordinary.presentation().entries().to_vec();
        presentations.extend(presentation(&[secret.as_str()]).entries().iter().cloned());
        let secret_presentation = RuntimeFlowPresentationDefinition::new(
            ordinary.presentation().title_cue().clone(),
            presentations,
        );
        let missing_origin = RuntimeFlowDefinition::new_with_secret_entries(
            ordinary.game_id().clone(),
            ordinary.bound_level_id().clone(),
            ordinary.entry_level_id().clone(),
            ordinary.entries().to_vec(),
            vec![RuntimeFlowSecretEntry::new(
                level("level.magic_carpet_2.not_in_campaign"),
                secret.clone(),
            )],
            secret_presentation.clone(),
        );
        assert!(missing_origin.is_err());

        let duplicated_secret = RuntimeFlowDefinition::new_with_secret_entries(
            ordinary.game_id().clone(),
            ordinary.bound_level_id().clone(),
            ordinary.entry_level_id().clone(),
            ordinary.entries().to_vec(),
            vec![RuntimeFlowSecretEntry::new(
                ordinary.entry_level_id().clone(),
                ordinary.entry_level_id().clone(),
            )],
            RuntimeFlowPresentationDefinition::new(
                ordinary.presentation().title_cue().clone(),
                ordinary
                    .presentation()
                    .entries()
                    .iter()
                    .cloned()
                    .chain(
                        presentation(&[ordinary.entry_level_id().as_str()])
                            .entries()
                            .iter()
                            .cloned(),
                    )
                    .collect(),
            ),
        );
        assert!(duplicated_secret.is_err());
    }

    #[test]
    fn runtime_flow_is_closed_ordered_and_source_identity_free() {
        let flow = valid_flow();
        assert_eq!(flow.entries().len(), 3);
        assert_eq!(
            flow.bound_level_id().as_str(),
            "level.magic_carpet_2.night_spire"
        );
        assert!(
            serde_json::to_string(&flow)
                .unwrap()
                .contains("flow.magic_carpet_2.return_to_title")
        );

        for invalid in [
            "game.source48",
            "game.raw_48",
            "game.level_1",
            "level.selector17",
            "flow.slot_4.result",
            "presentation.source48.loading",
        ] {
            assert!(SemanticGameId::parse(invalid).is_err());
            assert!(SemanticLevelId::parse(invalid).is_err());
            assert!(RuntimeFlowNodeId::parse(invalid).is_err());
            assert!(SemanticPresentationCueId::parse(invalid).is_err());
        }

        assert_eq!(
            flow.presentation().title_cue().as_str(),
            TITLE_PRESENTATION_CUE_ID
        );
        assert_eq!(flow.presentation().entries().len(), flow.entries().len());

        let missing_presentation = RuntimeFlowDefinition::new(
            flow.game_id().clone(),
            flow.bound_level_id().clone(),
            flow.entry_level_id().clone(),
            flow.entries().to_vec(),
            presentation(&[
                "level.magic_carpet_2.night_spire",
                "level.magic_carpet_2.payahandra_ruins",
            ]),
        );
        assert!(matches!(
            missing_presentation,
            Err(DefinitionError {
                code: DefinitionErrorCode::InvalidSchema,
                ..
            })
        ));

        let duplicate = RuntimeFlowDefinition::new(
            flow.game_id().clone(),
            flow.bound_level_id().clone(),
            flow.entry_level_id().clone(),
            vec![flow.entries()[0].clone(), flow.entries()[0].clone()],
            flow.presentation().clone(),
        );
        assert!(duplicate.is_err());

        let dangling = RuntimeFlowDefinition::new(
            flow.game_id().clone(),
            flow.bound_level_id().clone(),
            flow.entry_level_id().clone(),
            vec![entry(
                "level.magic_carpet_2.night_spire",
                RuntimeFlowDestination::Level {
                    level_id: level("level.magic_carpet_2.missing_destination"),
                },
            )],
            presentation(&["level.magic_carpet_2.night_spire"]),
        );
        assert!(dangling.is_err());

        let cycle = RuntimeFlowDefinition::new(
            flow.game_id().clone(),
            flow.bound_level_id().clone(),
            flow.entry_level_id().clone(),
            vec![entry(
                "level.magic_carpet_2.night_spire",
                RuntimeFlowDestination::Level {
                    level_id: level("level.magic_carpet_2.night_spire"),
                },
            )],
            presentation(&["level.magic_carpet_2.night_spire"]),
        );
        assert!(cycle.is_err());

        let skipped = RuntimeFlowDefinition::new(
            flow.game_id().clone(),
            flow.bound_level_id().clone(),
            flow.entry_level_id().clone(),
            vec![
                entry(
                    "level.magic_carpet_2.night_spire",
                    RuntimeFlowDestination::Level {
                        level_id: level("level.magic_carpet_2.revealed_way"),
                    },
                ),
                flow.entries()[1].clone(),
                flow.entries()[2].clone(),
            ],
            flow.presentation().clone(),
        );
        assert!(skipped.is_err());

        let mut raw = serde_json::to_value(flow).unwrap();
        raw["private_slot"] = serde_json::json!(48);
        assert!(serde_json::from_value::<RuntimeFlowDefinition>(raw).is_err());

        let mut destination_with_extra = serde_json::to_value(valid_flow()).unwrap();
        destination_with_extra["entries"][0]["completion_destination"]["node_id"] =
            serde_json::json!("flow.magic_carpet_2.not_a_level_field");
        assert!(serde_json::from_value::<RuntimeFlowDefinition>(destination_with_extra).is_err());
    }
}
