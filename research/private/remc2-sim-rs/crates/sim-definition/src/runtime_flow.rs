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

/// Closed ordered runtime-flow catalog plus the level bound by this one-level
/// game definition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeFlowDefinition {
    game_id: SemanticGameId,
    bound_level_id: SemanticLevelId,
    entry_level_id: SemanticLevelId,
    entries: Vec<RuntimeFlowEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeFlowDefinitionWire {
    game_id: SemanticGameId,
    bound_level_id: SemanticLevelId,
    entry_level_id: SemanticLevelId,
    entries: Vec<RuntimeFlowEntry>,
}

impl<'de> Deserialize<'de> for RuntimeFlowDefinition {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = RuntimeFlowDefinitionWire::deserialize(deserializer)?;
        Self::new(
            wire.game_id,
            wire.bound_level_id,
            wire.entry_level_id,
            wire.entries,
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
    ) -> Result<Self> {
        let flow = Self {
            game_id,
            bound_level_id,
            entry_level_id,
            entries,
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

    pub(crate) fn verify(&self) -> Result<()> {
        if self.entries.is_empty() {
            return Err(flow_error(
                "runtime_flow.entries",
                "runtime flow has no playable entry",
            ));
        }
        let mut ids = BTreeSet::new();
        for entry in &self.entries {
            if !ids.insert(entry.level_id.clone()) {
                return Err(flow_error(
                    entry.level_id.as_str(),
                    "runtime flow level ID is duplicated",
                ));
            }
        }
        if !ids.contains(&self.bound_level_id) {
            return Err(flow_error(
                self.bound_level_id.as_str(),
                "bound semantic level is absent from the runtime flow",
            ));
        }
        if !ids.contains(&self.entry_level_id) {
            return Err(flow_error(
                self.entry_level_id.as_str(),
                "runtime flow entry level is absent",
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
                && !ids.contains(level_id)
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
            if !visited.insert(current.clone()) {
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
        )
        .unwrap()
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
        ] {
            assert!(SemanticGameId::parse(invalid).is_err());
            assert!(SemanticLevelId::parse(invalid).is_err());
            assert!(RuntimeFlowNodeId::parse(invalid).is_err());
        }

        let duplicate = RuntimeFlowDefinition::new(
            flow.game_id().clone(),
            flow.bound_level_id().clone(),
            flow.entry_level_id().clone(),
            vec![flow.entries()[0].clone(), flow.entries()[0].clone()],
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
