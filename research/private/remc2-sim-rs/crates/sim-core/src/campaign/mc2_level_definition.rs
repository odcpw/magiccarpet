//! Private MC2 authored-record translation into the shared level vocabulary.
//!
//! Raw class/model identities terminate in this module. Runtime code consumes
//! [`CreatureKind`] and the immutable authored-record binding emitted here.

use std::collections::BTreeMap;

use crate::{
    level_definition::{
        ActivationGroupId, CreatureKind, LevelDefinition, LevelEntityDefinition, LevelEntityId,
        LevelEntityKind, TilePosition,
    },
    level1_mission::parse_authored_features,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Mc2LevelDefinition {
    pub(crate) semantic: LevelDefinition,
    pub(crate) creature_by_authored_record: BTreeMap<u16, CreatureKind>,
}

impl Mc2LevelDefinition {
    pub(crate) fn from_authored_bytes(bytes: &[u8]) -> Result<Self, String> {
        let features = parse_authored_features(bytes)?;
        let mut ordinals = BTreeMap::<CreatureKind, u16>::new();
        let mut creature_by_authored_record = BTreeMap::new();
        let mut entities = Vec::new();

        for feature in features {
            let kind = match (feature.kind, feature.subtype) {
                // `sub_4A310` constructor identities. These numbers remain
                // private to the MC2 adapter; shared callers see only kinds.
                (5, 4) => CreatureKind::Archer,
                (5, 19) => CreatureKind::Firefly,
                _ => continue,
            };
            let ordinal = ordinals.entry(kind).or_default();
            let id = LevelEntityId::creature(kind, *ordinal);
            *ordinal = ordinal
                .checked_add(1)
                .ok_or_else(|| format!("too many authored {} creatures", kind.as_str()))?;
            let activation_group = u16::try_from(feature.activation_group)
                .ok()
                .filter(|group| *group != 0)
                .map(ActivationGroupId::new);
            entities.push(LevelEntityDefinition::new(
                id,
                LevelEntityKind::Creature(kind),
                TilePosition {
                    x: feature.tile_x,
                    y: feature.tile_y,
                },
                activation_group,
            ));
            creature_by_authored_record.insert(feature.index, kind);
        }

        Ok(Self {
            semantic: LevelDefinition::new(entities)?,
            creature_by_authored_record,
        })
    }
}
