//! MC2 holder relations and source activation gates projected as semantic
//! actions. Clients consume this single binding rather than source selectors.

use crate::{
    SOURCE_SPELL_INDEX_MAP,
    level_definition::SpellKind,
    native_world::{EntitySnapshot, Level2SharedSpellResetSession},
    spell_activation::{SpellActivationHolder, SpellCasterState, activate_spell},
};
use sim_definition::SemanticId;
use sim_session::{RuntimeActorId, SpellAvailability, SpellHand, SpellMarkHand, SpellObservation};

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
        kind: SpellKind::StealMana,
        family: 13,
        id: "spell.steal_mana",
        name: "Steal Mana",
    },
    SpellBinding {
        kind: SpellKind::Duel,
        family: 14,
        id: "spell.duel",
        name: "Duel",
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
        family: 7,
        id: "spell.lightning",
        name: "Lightning",
    },
    SpellBinding {
        kind: SpellKind::Meteor,
        family: 9,
        id: "spell.meteor",
        name: "Meteor",
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
    SpellBinding {
        kind: SpellKind::Tremor,
        family: 15,
        id: "spell.tremor",
        name: "Tremor",
    },
    SpellBinding {
        kind: SpellKind::Crater,
        family: 16,
        id: "spell.crater",
        name: "Crater",
    },
    SpellBinding {
        kind: SpellKind::Earthquake,
        family: 17,
        id: "spell.earthquake",
        name: "Earthquake",
    },
    SpellBinding {
        kind: SpellKind::Volcano,
        family: 18,
        id: "spell.volcano",
        name: "Volcano",
    },
    SpellBinding {
        kind: SpellKind::SummonArmy,
        family: 19,
        id: "spell.summon_army",
        name: "Summon Army",
    },
    SpellBinding {
        kind: SpellKind::GravityWell,
        family: 20,
        id: "spell.gravity_well",
        name: "Gravity Well",
    },
    SpellBinding {
        kind: SpellKind::Whirlwind,
        family: 21,
        id: "spell.whirlwind",
        name: "Whirlwind",
    },
    SpellBinding {
        kind: SpellKind::FoolsMana,
        family: 22,
        id: "spell.fools_mana",
        name: "Fool's Mana",
    },
    SpellBinding {
        kind: SpellKind::MagicMine,
        family: 23,
        id: "spell.magic_mine",
        name: "Magic Mine",
    },
    SpellBinding {
        kind: SpellKind::Alliance,
        family: 24,
        id: "spell.alliance",
        name: "Alliance",
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

pub(super) fn observe(
    shared: &Level2SharedSpellResetSession,
    holder_actor: impl Fn(&EntitySnapshot) -> Option<RuntimeActorId>,
    emission_availability: impl Fn(&EntitySnapshot) -> SpellAvailability,
) -> Vec<SpellObservation> {
    let player = shared.local_player();
    let entity = shared.player_entity();
    let selected = SOURCE_SPELL_INDEX_MAP
        .get(usize::from(player.selected_spell_ui_index))
        .copied();
    BINDINGS
        .iter()
        .map(|binding| {
            let candidate = acquired_holder(shared, binding.family);
            let actor = candidate.and_then(&holder_actor);
            let holder = candidate.filter(|_| actor.is_some());
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
                        emission_availability(holder)
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
                holder_actor: actor,
                marked_hand: match player.spell_hand_eligibility[usize::from(binding.family)] {
                    1 => SpellMarkHand::Left,
                    2 => SpellMarkHand::Right,
                    _ => SpellMarkHand::None,
                },
                selected: acquired && selected == Some(binding.family),
                tier: holder.map_or(0, |holder| holder.level_gate),
                unlocked_tier: player.spell_unlocked_levels[usize::from(binding.family)].clamp(0, 2)
                    as u8,
                menu_slot: SOURCE_SPELL_INDEX_MAP
                    .iter()
                    .position(|&family| family == binding.family)
                    .expect("bound spell has a menu slot") as u8,
                menu_artwork: Some(sim_session::SpellMenuArtwork {
                    grid_icon: 97 + u16::from(binding.family),
                    tier_icons: std::array::from_fn(|tier| {
                        179 + 3 * u16::from(binding.family) + tier as u16
                    }),
                }),
                mana_cost: holder.map_or(0, |holder| holder.mana_cost),
                hand,
                availability,
            }
        })
        .collect()
}

/// Family-specific prerequisites after the shared scalar activation gate.
/// Read the current source actor and its own Castle without advancing either.
pub(super) fn emission_availability(
    holder: &EntitySnapshot,
    parent: Option<&EntitySnapshot>,
    castle_mana: Option<i32>,
) -> SpellAvailability {
    if !matches!(holder.model, 9 | 12) {
        return SpellAvailability::Ready;
    }
    let Some(parent) = parent.filter(|parent| {
        parent.index == holder.source_spell_parent_entity_index()
            && parent.index != 0
            && parent.active
            && parent.class == 3
    }) else {
        return SpellAvailability::InvalidCaster;
    };
    if holder.model == 12 {
        use crate::spell_activation::{BeyondSightAdmission, beyond_sight_admission};
        return match beyond_sight_admission(
            parent.mana,
            parent.life,
            holder.max_mana,
            holder.mana_regen,
            true,
            castle_mana,
        ) {
            BeyondSightAdmission::Allowed => SpellAvailability::Ready,
            BeyondSightAdmission::ParentUnavailable => SpellAvailability::InvalidCaster,
            BeyondSightAdmission::InsufficientCasterMana
            | BeyondSightAdmission::InsufficientCastleMana => SpellAvailability::InsufficientMana,
        };
    }
    use crate::native_world::{MeteorAdmission, meteor_admission};
    match meteor_admission(
        holder,
        parent,
        castle_mana,
        holder.activation_value.cast_signed(),
    ) {
        MeteorAdmission::Allowed => SpellAvailability::Ready,
        MeteorAdmission::ParentUnavailable => SpellAvailability::InvalidCaster,
        MeteorAdmission::InsufficientCasterMana | MeteorAdmission::InsufficientCastleMana => {
            SpellAvailability::InsufficientMana
        }
    }
}

pub(super) fn assigned_player_command(
    shared: &Level2SharedSpellResetSession,
    assignment: &sim_session::SpellAssignment,
    mut command: crate::PlayerCommand,
) -> Result<crate::PlayerCommand, String> {
    let kind = semantic_spell_kind(&assignment.spell_id)
        .ok_or_else(|| format!("unknown spell {}", assignment.spell_id.as_str()))?;
    let family = mc2_spell_family(kind).ok_or_else(|| "unbound spell family".to_owned())?;
    if acquired_holder(shared, family).is_none() {
        return Err(format!(
            "spell {} has not been acquired",
            assignment.spell_id.as_str()
        ));
    }
    let unlocked =
        shared.local_player().spell_unlocked_levels[usize::from(family)].clamp(0, 2) as u8;
    if assignment.tier > unlocked {
        return Err(format!(
            "spell tier {} is locked (maximum {})",
            assignment.tier, unlocked
        ));
    }
    command.opcode = match assignment.hand {
        sim_session::SpellAssignmentHand::Left => 31,
        sim_session::SpellAssignmentHand::Right => 32,
    };
    command.argument_1 = SOURCE_SPELL_INDEX_MAP
        .iter()
        .position(|&candidate| candidate == family)
        .ok_or_else(|| "spell has no source menu binding".to_owned())?
        as i8;
    command.argument_2 = assignment.tier as i8;
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::super::tests::guard;
    use super::super::{
        GameCommand, Mc2CampaignAdapter, Mc2CampaignBackend, Mc2CampaignInputs, default_catalog,
        load_mc2_campaign,
    };
    use super::*;
    use crate::game_session::GameSession;
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn cave_in_command_and_carry_use_the_same_closed_semantic_binding() {
        let id = SemanticId::parse("spell.cave_in").unwrap();
        assert_eq!(semantic_spell_kind(&id), Some(SpellKind::CaveIn));
        assert_eq!(mc2_spell_family(SpellKind::CaveIn), Some(25));
        assert_eq!(mc2_spell_kind_for_family(25), Some(SpellKind::CaveIn));
        assert!(SOURCE_SPELL_INDEX_MAP.contains(&25));
        assert_eq!(format!("spell.{}", SpellKind::CaveIn.as_str()), id.as_str());

        for (id, kind, family) in [
            ("spell.summon_army", SpellKind::SummonArmy, 19),
            ("spell.magic_mine", SpellKind::MagicMine, 23),
            ("spell.alliance", SpellKind::Alliance, 24),
        ] {
            let id = SemanticId::parse(id).unwrap();
            assert_eq!(semantic_spell_kind(&id), Some(kind));
            assert_eq!(mc2_spell_family(kind), Some(family));
            assert_eq!(mc2_spell_kind_for_family(family), Some(kind));
            assert!(SOURCE_SPELL_INDEX_MAP.contains(&family));
        }

        for unsupported in ["spell.family25", "spell.cavein", "spell.cave_in.projectile"] {
            let id = SemanticId::parse(unsupported).unwrap();
            assert_eq!(semantic_spell_kind(&id), None);
        }
    }

    #[test]
    fn beyond_sight_public_pickup_readiness_cast_and_failed_gate() {
        let root = std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
            },
            PathBuf::from,
        );
        let definition = Arc::new(load_mc2_campaign(&root, &default_catalog().unwrap()).unwrap());
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            definition,
            912,
            Mc2CampaignInputs::new(&root),
        )
        .unwrap();
        let (slot, position, parent) = {
            let simulation = session.backend().campaign.simulation();
            let pickup = simulation
                .shared_spells()
                .pickups()
                .iter()
                .find(|holder| holder.model == 12 && holder.action == 37)
                .expect("ordinary Beyond Sight pickup from the owner package");
            (pickup.index, pickup.position, simulation.state.entity_index)
        };
        session
            .backend_mut()
            .campaign
            .simulation_mut()
            .set_level2_entity_position(parent, position)
            .unwrap();
        for _ in 0..2 {
            session
                .step(guard(&session), GameCommand::default())
                .unwrap();
        }
        let spell = |session: &GameSession<Mc2CampaignBackend>| {
            session
                .snapshot()
                .value
                .player_spells
                .into_iter()
                .find(|spell| spell.id.as_str() == "spell.beyond_sight")
                .unwrap()
        };
        let acquired = spell(&session);
        assert!(acquired.acquired);
        assert_eq!(
            (acquired.tier, acquired.unlocked_tier, acquired.mana_cost),
            (0, 0, 1_800)
        );
        for tier in [1, 2] {
            let rejected = session.step(
                guard(&session),
                GameCommand {
                    assign_spell: Some(sim_session::SpellAssignment {
                        spell_id: acquired.id.clone(),
                        tier,
                        hand: sim_session::SpellAssignmentHand::Left,
                        holder_actor: acquired.holder_actor,
                    }),
                    ..GameCommand::default()
                },
            );
            assert!(rejected.is_err(), "unearned tier {tier} must remain locked");
        }
        session
            .step(
                guard(&session),
                GameCommand {
                    assign_spell: Some(sim_session::SpellAssignment {
                        spell_id: acquired.id.clone(),
                        tier: 0,
                        hand: sim_session::SpellAssignmentHand::Left,
                        holder_actor: acquired.holder_actor,
                    }),
                    ..GameCommand::default()
                },
            )
            .unwrap();
        let simulation = session.backend_mut().campaign.simulation_mut();
        let castle = simulation
            .economy_mut()
            .build_castle(crate::Position::default(), parent)
            .unwrap();
        assert!(
            simulation
                .economy_mut()
                .set_castle_mana_prerequisite_for_test(castle, 10_000)
        );
        // Controlled resource boundary; no acquired/unlocked state is injected.
        let set_mana = |simulation: &mut crate::native_world::Mc2Simulation, mana: i32| {
            let entity = &mut simulation.economy.entities[usize::from(parent)];
            entity.mana = mana;
            entity.record[148..152].copy_from_slice(&mana.to_le_bytes());
            simulation
                .shared_spells_mut()
                .set_local_player_mana_for_test(mana);
            if let Some(fire) = simulation
                .session
                .state
                .live
                .core
                .context
                .spell_fire
                .as_mut()
            {
                fire.caster.mana = mana;
            }
        };
        set_mana(simulation, 1_799);
        assert_eq!(
            spell(&session).availability,
            SpellAvailability::InsufficientMana
        );
        set_mana(session.backend_mut().campaign.simulation_mut(), 5_000);
        assert_eq!(spell(&session).availability, SpellAvailability::Ready);
        session
            .step(
                guard(&session),
                GameCommand {
                    select_spell: Some(acquired.id),
                    cast_selected: true,
                    ..GameCommand::default()
                },
            )
            .unwrap();
        assert_eq!(spell(&session).availability, SpellAvailability::Active);
        assert_eq!(
            session.backend().campaign.simulation().economy.entities[usize::from(slot)]
                .active_value,
            151
        );
        set_mana(session.backend_mut().campaign.simulation_mut(), 0);
        session
            .step(guard(&session), GameCommand::default())
            .unwrap();
        let ended = spell(&session);
        assert!(ended.acquired, "expired holders remain in inventory");
        assert_eq!(ended.availability, SpellAvailability::InsufficientMana);
        assert_eq!(
            session.backend().campaign.simulation().economy.entities[usize::from(slot)]
                .active_value,
            0
        );
    }
}
