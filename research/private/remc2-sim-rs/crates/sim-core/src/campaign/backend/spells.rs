//! MC2 holder relations and source activation gates projected as semantic
//! actions. Clients consume this single binding rather than source selectors.

use crate::{
    SOURCE_SPELL_INDEX_MAP,
    level_definition::SpellKind,
    native_world::{EntitySnapshot, Level2SharedSpellResetSession},
    spell_activation::{SpellActivationHolder, SpellCasterState, activate_spell},
};
use sim_definition::SemanticId;
use sim_session::{SpellAvailability, SpellHand, SpellObservation};

struct SpellBinding {
    kind: SpellKind,
    family: u8,
    id: &'static str,
    name: &'static str,
}

// Extend this binding when an owned spell family joins the typed definition.
// Selection, discovery and campaign carry all use these same identities.
const BINDINGS: &[SpellBinding] = &[
    SpellBinding {
        kind: SpellKind::Fireball,
        family: 0,
        id: "spell.fireball",
        name: "Fireball",
    },
    SpellBinding {
        kind: SpellKind::Possession,
        family: 1,
        id: "spell.possession",
        name: "Possession",
    },
    SpellBinding {
        kind: SpellKind::Castle,
        family: 2,
        id: "spell.castle",
        name: "Castle",
    },
    SpellBinding {
        kind: SpellKind::Speed,
        family: 3,
        id: "spell.speed",
        name: "Speed",
    },
    SpellBinding {
        kind: SpellKind::Morph,
        family: 4,
        id: "spell.morph",
        name: "Morph",
    },
    SpellBinding {
        kind: SpellKind::Shield,
        family: 6,
        id: "spell.shield",
        name: "Shield",
    },
    SpellBinding {
        kind: SpellKind::Invisibility,
        family: 11,
        id: "spell.invisibility",
        name: "Invisibility",
    },
    SpellBinding {
        kind: SpellKind::BeyondSight,
        family: 12,
        id: "spell.beyond_sight",
        name: "Beyond Sight",
    },
    SpellBinding {
        kind: SpellKind::Heal,
        family: 5,
        id: "spell.heal",
        name: "Heal",
    },
    SpellBinding {
        kind: SpellKind::Rebound,
        family: 8,
        id: "spell.rebound",
        name: "Rebound",
    },
    SpellBinding {
        kind: SpellKind::Lightning,
        family: 9,
        id: "spell.lightning",
        name: "Lightning",
    },
    SpellBinding {
        kind: SpellKind::Teleport,
        family: 10,
        id: "spell.teleport",
        name: "Teleport",
    },
    SpellBinding {
        // Source spell_t::cave_in. Cave/castle emission checks belong to the
        // following holder visit, after the common activation boundary.
        kind: SpellKind::CaveIn,
        family: 25,
        id: "spell.cave_in",
        name: "Cave In",
    },
];

pub(super) fn semantic_spell_kind(id: &SemanticId) -> Option<SpellKind> {
    BINDINGS
        .iter()
        .find(|binding| binding.id == id.as_str())
        .map(|binding| binding.kind)
}

pub(super) fn mc2_spell_family(kind: SpellKind) -> Option<u8> {
    BINDINGS
        .iter()
        .find(|binding| binding.kind == kind)
        .map(|binding| binding.family)
}

pub(super) fn mc2_spell_kind_for_family(family: u8) -> Option<SpellKind> {
    BINDINGS
        .iter()
        .find(|binding| binding.family == family)
        .map(|binding| binding.kind)
}

pub(super) fn acquired_holder(
    shared: &Level2SharedSpellResetSession,
    family: u8,
) -> Option<&EntitySnapshot> {
    let player = shared.local_player();
    let index = *player.enabled_spell_holders.get(usize::from(family))?;
    if index == 0 {
        return None;
    }
    shared
        .pickups()
        .iter()
        .chain(shared.holders())
        .chain(shared.dynamic_pickups())
        .find(|holder| holder.index == index)
        .filter(|holder| {
            holder.active
                && holder.class == 15
                && holder.model == family
                && holder.source_spell_parent_entity_index() == player.entity_index
        })
}

pub(super) fn observe(shared: &Level2SharedSpellResetSession) -> Vec<SpellObservation> {
    let player = shared.local_player();
    let entity = shared.player_entity();
    let selected = SOURCE_SPELL_INDEX_MAP
        .get(usize::from(player.selected_spell_ui_index))
        .copied();
    BINDINGS
        .iter()
        .map(|binding| {
            let holder = acquired_holder(shared, binding.family);
            let acquired = holder.is_some();
            let hand = match (
                acquired && player.left_spell_slot_raw == i16::from(binding.family),
                acquired && player.right_spell_slot_raw == i16::from(binding.family),
            ) {
                (true, true) => SpellHand::Both,
                (true, false) => SpellHand::Left,
                (false, true) => SpellHand::Right,
                (false, false) => SpellHand::None,
            };
            let availability = holder.map_or_else(
                || {
                    if player.enabled_spell_holders[usize::from(binding.family)] == 0 {
                        SpellAvailability::NotAcquired
                    } else {
                        SpellAvailability::InvalidHolder
                    }
                },
                |holder| {
                    let mut caster = SpellCasterState {
                        entity_index: player.entity_index,
                        model: entity.model,
                        mana: shared.player_mana(),
                        control_flags: entity.control_flags,
                        ..SpellCasterState::default()
                    };
                    let mut activation = SpellActivationHolder {
                        entity_index: holder.index,
                        model: holder.model,
                        level_gate: holder.level_gate,
                        active_value: holder.active_value,
                        activation_value: holder.activation_value,
                        mana_cost: holder.mana_cost,
                        action_marker: holder.action_marker,
                    };
                    // The exact primitive is side-effectful (including active-spell
                    // cancellation and hints). Only disposable scalar copies and a
                    // private event sink are supplied: reads never tick or consume RNG.
                    if activate_spell(
                        &mut caster,
                        Some(&mut activation),
                        256,
                        false,
                        &mut Vec::new(),
                    ) {
                        SpellAvailability::Ready
                    } else {
                        // A second scalar probe separates a mana refusal from a
                        // source active/timer gate, preserving that gate's precedence.
                        caster.mana = i32::MAX;
                        activation.active_value = holder.active_value;
                        if activate_spell(
                            &mut caster,
                            Some(&mut activation),
                            256,
                            false,
                            &mut Vec::new(),
                        ) {
                            SpellAvailability::InsufficientMana
                        } else {
                            SpellAvailability::Active
                        }
                    }
                },
            );
            SpellObservation {
                id: SemanticId::parse(binding.id).expect("static semantic spell ID"),
                display_name: binding.name.to_owned(),
                acquired,
                selected: acquired && selected == Some(binding.family),
                tier: holder.map_or(0, |holder| holder.level_gate),
                mana_cost: holder.map_or(0, |holder| holder.mana_cost),
                hand,
                availability,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cave_in_command_and_carry_use_the_same_closed_semantic_binding() {
        let id = SemanticId::parse("spell.cave_in").unwrap();
        assert_eq!(semantic_spell_kind(&id), Some(SpellKind::CaveIn));
        assert_eq!(mc2_spell_family(SpellKind::CaveIn), Some(25));
        assert_eq!(mc2_spell_kind_for_family(25), Some(SpellKind::CaveIn));
        assert!(SOURCE_SPELL_INDEX_MAP.contains(&25));
        assert_eq!(format!("spell.{}", SpellKind::CaveIn.as_str()), id.as_str());

        for unsupported in ["spell.family25", "spell.cavein", "spell.cave_in.projectile"] {
            let id = SemanticId::parse(unsupported).unwrap();
            assert_eq!(semantic_spell_kind(&id), None);
        }
    }
}
