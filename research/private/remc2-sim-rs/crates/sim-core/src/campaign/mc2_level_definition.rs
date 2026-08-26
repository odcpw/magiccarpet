//! Private MC2 authored-record translation into the shared level vocabulary.
//!
//! Raw class/model identities terminate in this module. Runtime code consumes
//! semantic kinds and the immutable authored-record binding emitted here.

use std::collections::BTreeMap;

use crate::{
    level_definition::{
        ActivationGroupId, CreatureKind, LevelDefinition, LevelEntityDefinition, LevelEntityId,
        LevelEntityKind, SpellKind, TilePosition,
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
        let mut creature_ordinals = BTreeMap::<CreatureKind, u16>::new();
        let mut spell_ordinals = BTreeMap::<SpellKind, u16>::new();
        let mut creature_by_authored_record = BTreeMap::new();
        let mut entities = Vec::new();

        for feature in features {
            let creature_kind = match (feature.kind, feature.subtype) {
                // `sub_4A310` constructor identities. These numbers remain
                // private to the MC2 adapter; shared callers see only kinds.
                (5, 0) => Some(CreatureKind::Dragon),
                (5, 1) => Some(CreatureKind::Goat),
                (5, 2) => Some(CreatureKind::Bee),
                (5, 3) => Some(CreatureKind::Worm),
                (5, 4) => Some(CreatureKind::Archer),
                (5, 9) => Some(CreatureKind::Skeleton),
                (5, 13) => Some(CreatureKind::Villager),
                (5, 14) => Some(CreatureKind::Trader),
                (5, 19) => Some(CreatureKind::Firefly),
                _ => None,
            };
            let activation_group = u16::try_from(feature.activation_group)
                .ok()
                .filter(|group| *group != 0)
                .map(ActivationGroupId::new);
            let position = TilePosition {
                x: feature.tile_x,
                y: feature.tile_y,
            };

            if let Some(kind) = creature_kind {
                let ordinal = creature_ordinals.entry(kind).or_default();
                let id = LevelEntityId::creature(kind, *ordinal);
                *ordinal = ordinal
                    .checked_add(1)
                    .ok_or_else(|| format!("too many authored {} creatures", kind.as_str()))?;
                entities.push(LevelEntityDefinition::new(
                    id,
                    LevelEntityKind::Creature(kind),
                    position,
                    activation_group,
                ));
                creature_by_authored_record.insert(feature.index, kind);
                continue;
            }

            let spell_kind = match (feature.kind, feature.subtype) {
                // `AddSpellXX_XX_51120` receives these spell identities. Keep
                // the numeric selectors inside the private MC2 adapter.
                (15, 2) => SpellKind::Speed,
                (15, 3) => SpellKind::Possession,
                (15, 4) => SpellKind::Shield,
                (15, 6) => SpellKind::Earthquake,
                (15, 11) => SpellKind::RubberBand,
                (15, 12) => SpellKind::Invisibility,
                _ => continue,
            };
            let ordinal = spell_ordinals.entry(spell_kind).or_default();
            let id = LevelEntityId::spell_pickup(spell_kind, *ordinal);
            *ordinal = ordinal.checked_add(1).ok_or_else(|| {
                format!("too many authored {} spell pickups", spell_kind.as_str())
            })?;
            entities.push(LevelEntityDefinition::new(
                id,
                LevelEntityKind::SpellPickup(spell_kind),
                position,
                activation_group,
            ));
        }

        Ok(Self {
            semantic: LevelDefinition::new(entities)?,
            creature_by_authored_record,
        })
    }
}
