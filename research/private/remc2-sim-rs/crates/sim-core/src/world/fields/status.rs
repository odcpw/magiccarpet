//! Scheduled model65/66 impacts write the exact carriers consumed by flight.

use super::{Contact, Outcome, PaletteRequest};
use crate::{
    FlightEffectsContext, FlightState, level_definition::FieldDefinition,
    native_world::EntitySnapshot,
};

/// Ordinary player upkeep reasserts the current slowing intensity when the
/// frame service or counter recovery has set the shared moveSpeedFlag.
pub(crate) fn palette_after_flight(
    state: &FlightState,
    effects: &FlightEffectsContext,
) -> Option<PaletteRequest> {
    if state.player_color_index != effects.local_player_index {
        return None;
    }
    if state.speed_modifier != 0 && effects.move_speed_flag != 0 {
        Some(PaletteRequest {
            mode: 3,
            count: 171 * i16::from(state.speed_modifier) / 3 + 85,
        })
    } else if effects.palette_submode == 1 {
        Some(PaletteRequest { mode: 1, count: 0 })
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    Slowing,
    Web,
}

impl Kind {
    pub(crate) const fn source_selector(self) -> (u8, u8) {
        match self {
            Self::Slowing => (65, 70),
            Self::Web => (66, 71),
        }
    }
}

/// Target is source word96, never a proximity search. The caller resolves it
/// against the current arena incarnation before lending the player carrier.
/// This function does not advance phase: that belongs to ApplyEvents' tail.
pub(crate) fn impact(
    source: &mut EntitySnapshot,
    definition: FieldDefinition,
    player: Option<(&EntitySnapshot, &mut FlightState)>,
    local_player_color: u8,
) -> Result<Outcome, String> {
    let (kind, maximum, interval, boost) = match definition {
        FieldDefinition::Slowing {
            max_stages,
            interval,
            first_boost,
        } => (Kind::Slowing, max_stages, interval, first_boost),
        FieldDefinition::Web {
            duration,
            first_boost,
            ..
        } => (Kind::Web, 1, duration, first_boost),
        _ => return Err("scheduled status impact needs slowing or web properties".to_owned()),
    };
    let mut outcome = Outcome::default();
    let target = u16::from_le_bytes([source.record[154], source.record[155]]);
    if target != 0
        && let Some((body, state)) = player
        && body.index == target
        && state.entity_index == target
        && (body.class, body.model) == (3, 0)
    {
        let first = match kind {
            Kind::Slowing => state.speed_modifier == 0,
            Kind::Web => state.immobilize_counter == 0,
        };
        if first {
            state.recoil_speed = boost;
            source.set_entity_rng(source.entity_rng.wrapping_mul(9_377).wrapping_add(9_439));
            outcome.sounds.push((target, (source.entity_rng & 3) + 54));
            if kind == Kind::Web {
                outcome.contacts.push(Contact {
                    target,
                    owner: source.source_id,
                    payload: source.subspell_index,
                });
            }
        }
        match kind {
            Kind::Slowing => {
                if state.speed_modifier < maximum {
                    state.speed_modifier += 1;
                    if state.player_color_index == local_player_color {
                        outcome.palette = Some(PaletteRequest {
                            mode: 3,
                            count: 171 * i16::from(state.speed_modifier) / 3 + 85,
                        });
                    }
                }
                state.speed_modifier_counter = interval;
            }
            Kind::Web => {
                state.immobilize_counter = 1;
                state.immobilize_subcounter = interval;
            }
        }
    }
    source.disable_drawing();
    Ok(outcome)
}

/// sub_50780/sub_507C0. The ordinary projectile owner supplies the real
/// collision target and inherited attacker after this constructor returns.
pub(crate) fn construct(
    arena: &mut crate::campaign::source_arena::SourceArena,
    terrain: &mut crate::native_world::SessionTerrain,
    position: crate::Position,
    definition: FieldDefinition,
) -> Result<Option<u16>, String> {
    let (kind, damage) = match definition {
        FieldDefinition::Slowing { .. } => (Kind::Slowing, 100),
        FieldDefinition::Web { damage, .. } => (Kind::Web, damage),
        _ => return Err("status constructor needs slowing or web properties".to_owned()),
    };
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut source = crate::world::terrain::cave_features::new_event(
        slot,
        arena.economy.new_event_rng(slot),
        position,
    );
    let (model, action) = kind.source_selector();
    source.class = 10;
    source.model = model;
    source.record[67] = 10;
    source.record[68] = model;
    source.set_source_action(action);
    source.set_control_flags((source.control_flags & !9) | 1);
    source.set_subspell_index(damage);
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.entities[usize::from(slot)] = source;
    Ok(Some(slot))
}
