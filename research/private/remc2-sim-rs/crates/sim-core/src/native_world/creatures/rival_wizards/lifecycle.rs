//! Rival action2/3 lifecycle composed from the current arena and player owners.
//!
//! The flight token makes the journal barrier explicit. Grounded completion
//! follows the shared world-reclaim boundary; no campaign/session owner is
//! borrowed here and no predecessor packet is committed after NewEvent.

use super::RivalWizardRoster;
use crate::campaign::source_arena::{SourceArena, SourceManaChain};
use crate::house::HouseFamily;
use crate::native_world::creatures::player_lifecycle;
use crate::native_world::{
    EntitySnapshot, EntitySpecialRelation, Level2SharedSpellResetSession, NativeWorldError,
    PLAYER_LEGACY_RECORD_BYTES, PLAYER_SPELL_COUNT, ParticleParameter, PlayerSnapshot,
    PresentationSelectors, Result as NativeResult, SessionTerrain, level2_next_rng16,
    level2_set_spell_profile, source_target,
};
use crate::renderable_map_chain::RenderableMapSeed;
use crate::{FlightContext, FlightEvent, FlightProfile, FlightWorld, Position};
use player_lifecycle::notifications::{PlayerLifeNoticeKind, PlayerLifecycleNotifications};

pub(crate) struct FlightDomains<'a> {
    pub(crate) arena: &'a mut SourceArena,
    pub(crate) terrain: &'a SessionTerrain,
    pub(crate) shared: &'a mut Level2SharedSpellResetSession,
    pub(crate) players: &'a mut [PlayerSnapshot],
    pub(crate) roster: &'a mut RivalWizardRoster,
    pub(crate) local_life: &'a mut i32,
    pub(crate) local_owner: u16,
    pub(crate) context: &'a mut FlightContext,
}

/// Produced only after the real owner flight body. Journal these events
/// before consuming the token, because the next operation can allocate.
pub(crate) struct DyingFlight {
    pub(crate) events: Vec<FlightEvent>,
    slot: u16,
    position: Position,
    burst_position: Position,
    old_velocity: i16,
    clearance: i16,
}

pub(crate) fn begin_dying(
    domains: FlightDomains<'_>,
    slot: u16,
    mut profile: FlightProfile,
    presentation: &PresentationSelectors,
) -> Result<DyingFlight, String> {
    let actor = &domains.arena.entities[usize::from(slot)];
    let parameter = actor
        .parameter_relation
        .and_then(|index| presentation.entity_parameters.get(usize::from(index)))
        .expect("qualified Nyphur parameter row remains present");
    let word = |at: usize| i16::from_le_bytes([parameter[at], parameter[at + 1]]);
    profile.terrain_reference_height = word(10);
    profile.ground_clearance = word(12);
    profile.passive_vertical_step = word(14);
    profile.raw_word_0x10 = word(16);
    profile.raw_word_0x12 = word(18);
    let old_velocity = actor.source_word_2c();
    let event_start = domains.context.events.len();
    let (position, burst_position) = domains.roster.step_dying_flight(
        domains.arena,
        domains.shared,
        domains.players,
        domains.local_life,
        domains.local_owner,
        slot,
        domains.terrain,
        profile,
        domains.context,
    )?;
    Ok(DyingFlight {
        events: domains.context.events[event_start..].to_vec(),
        slot,
        position,
        burst_position,
        old_velocity,
        clearance: profile.ground_clearance,
    })
}

/// Only this source fall/burst prefix can produce a grounded continuation.
pub(crate) struct GroundedDeath {
    slot: u16,
    release_position: Position,
}

pub(crate) fn fall_and_burst(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    shared: &mut Level2SharedSpellResetSession,
    particles: &[ParticleParameter],
    flight: DyingFlight,
) -> Result<Option<GroundedDeath>, String> {
    let altitude = terrain.ground_height(flight.position);
    let fall = player_lifecycle::fall(
        crate::level_definition::PLAYER_LIFECYCLE_PROPERTIES,
        flight.position,
        flight.old_velocity,
        altitude,
        flight.clearance,
    );
    let actor = &mut arena.entities[usize::from(flight.slot)];
    actor.set_source_word_2c(fall.next_velocity);
    actor.set_position(fall.position);
    let active = actor.active;
    let actor = actor.clone();
    arena
        .map_chains
        .sync_entity(RenderableMapSeed {
            entity_index: flight.slot,
            active,
            x: fall.position.x,
            y: fall.position.y,
        })
        .map_err(|error| error.to_string())?;
    shared.sync_player_entity_from_authority(&actor);
    let burst = player_lifecycle::spawn_dying_burst(
        arena,
        terrain,
        particles,
        flight.slot,
        flight.burst_position,
    )?;
    let grounded = if burst == Some(flight.slot) {
        i32::from(arena.entities[usize::from(flight.slot)].position.z)
            == i32::from(altitude) + i32::from(flight.clearance)
    } else {
        fall.grounded
    };
    Ok(grounded.then_some(GroundedDeath {
        slot: flight.slot,
        release_position: fall.position,
    }))
}

pub(crate) struct GroundedDomains<'a> {
    pub(crate) arena: &'a mut SourceArena,
    pub(crate) terrain: &'a mut SessionTerrain,
    pub(crate) shared: &'a mut Level2SharedSpellResetSession,
    pub(crate) players: &'a mut [PlayerSnapshot],
    pub(crate) roster: &'a mut RivalWizardRoster,
    pub(crate) houses: &'a mut HouseFamily,
    pub(crate) notices: &'a mut PlayerLifecycleNotifications,
}

/// Called after ordinary reclamation and the source arena stack rebuild.
/// Mutations stay in source order, including prefixes before an error.
pub(crate) fn finish_grounded(
    mut domains: GroundedDomains<'_>,
    mana_chain: &SourceManaChain,
    death: GroundedDeath,
) -> Result<(), String> {
    let slot = death.slot;
    if let Some(owner) = domains.arena.handle(slot) {
        domains
            .roster
            .credit_grounded_death(owner, domains.arena, domains.players, domains.shared);
    }
    domains.arena.entities[usize::from(slot)].clear_source_upkeep_mailbox();
    let relation = domains.arena.entities[usize::from(slot)].special_relation;
    let EntitySpecialRelation::PlayerState(player_slot) = relation else {
        return Err(format!("dying rival {slot} has no current player A4"));
    };
    let color = domains
        .players
        .iter()
        .find(|player| player.slot == player_slot)
        .ok_or_else(|| format!("dying rival {slot} has no player A4 slot {player_slot}"))?
        .player_color_index();
    domains
        .notices
        .publish(
            color,
            slot,
            PlayerLifeNoticeKind::Died,
            crate::level_definition::PLAYER_LIFECYCLE_PROPERTIES.death_notice_frames,
        )
        .map_err(|error| error.to_string())?;
    release_holders(&mut domains, slot, death.release_position)?;
    let position = domains.arena.entities[usize::from(slot)].position;
    let marker =
        player_lifecycle::construct_death_marker(domains.arena, domains.terrain, slot, position)?;
    if let Some(marker) = marker {
        let actor = &mut domains.arena.entities[usize::from(slot)];
        actor.set_source_action(3);
        actor.set_source_dword_10(
            crate::level_definition::PLAYER_LIFECYCLE_PROPERTIES.revival_wait_visits,
        );
        domains
            .arena
            .redirect_mana_chain_to_marker(mana_chain, slot, marker.source_entity_index);
    }
    domains.arena.entities[usize::from(slot)].set_player_death_hidden_bit();
    domains
        .arena
        .decrement_replacement_cursor_after_player_contact();
    Ok(())
}

pub(in crate::native_world) fn release_holders(
    domains: &mut GroundedDomains<'_>,
    slot: u16,
    position: Position,
) -> Result<(), String> {
    let Some(player_index) = domains
        .players
        .iter()
        .position(|player| player.entity_index == slot && player.is_ai)
    else {
        return Ok(());
    };
    let enabled = domains.players[player_index].enabled_spell_holders;
    let owner = domains.arena.handle(slot);
    let mut rng = domains.arena.entities[usize::from(slot)].entity_rng;
    for (family, holder_slot) in enabled.into_iter().enumerate() {
        if holder_slot == 1 {
            if player_lifecycle::scatter_current_sentinel_house(
                domains.arena,
                domains.houses,
                domains.terrain,
                slot,
                position,
                &mut rng,
            )? {
                continue;
            }
            if domains.arena.economy.live_arena_kind(1).is_some() {
                return Err(
                    "released-holder sentinel1 has no current typed scatter owner".to_owned(),
                );
            }
        }
        let valid = owner.is_some_and(|handle| {
            domains
                .roster
                .bound_live_holder(
                    handle,
                    &domains.players[player_index],
                    domains.arena,
                    family,
                )
                .is_some_and(|holder| holder.source_slot == holder_slot)
        });
        if holder_slot != 1 && !valid {
            assert!(
                domains.players[player_index]
                    .set_enabled_spell_holder_index(family, u16::from(holder_slot != 0)),
                "qualified Nyphur player owns every spell-family relation"
            );
            continue;
        }
        assert!(
            domains.players[player_index].set_enabled_spell_holder_index(family, 1),
            "qualified Nyphur player owns every spell-family relation"
        );
        let holder = &mut domains.arena.entities[usize::from(holder_slot)];
        holder.set_control_flags(holder.control_flags & !1);
        holder.set_source_action(holder.action.wrapping_add(1));
        rng = level2_next_rng16(rng);
        let x = i16::try_from(i32::from(rng & 0x01ff) - 256).unwrap();
        rng = level2_next_rng16(rng);
        let y = i16::try_from(i32::from(rng & 0x01ff) - 256).unwrap();
        let released_position = Position {
            x: position.x.wrapping_add_signed(x),
            y: position.y.wrapping_add_signed(y),
            z: position.z,
        };
        domains.arena.entities[usize::from(holder_slot)].set_position(released_position);
        domains
            .arena
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: holder_slot,
                active: true,
                x: released_position.x,
                y: released_position.y,
            })
            .expect("released Nyphur holder source slot fits map chain");
        rng = level2_next_rng16(rng);
        domains.arena.entities[usize::from(holder_slot)].set_life(i32::from(rng % 90) + 200);
        let released = domains.arena.entities[usize::from(holder_slot)].clone();
        if let Some(shared) = domains.shared.pickup_mut(holder_slot) {
            shared.clone_from(&released);
        } else {
            domains.shared.dynamic_pickups.push(released);
        }
    }
    domains.arena.entities[usize::from(slot)].set_entity_rng(rng);
    let player = domains.players[player_index].clone();
    if let Some(shared) = domains
        .shared
        .players
        .iter_mut()
        .find(|row| row.entity_index == slot)
    {
        *shared = player;
    }
    Ok(())
}

pub(crate) struct BanishDomains<'a> {
    pub(crate) arena: &'a mut SourceArena,
    pub(crate) shared: &'a mut Level2SharedSpellResetSession,
    pub(crate) players: &'a mut [PlayerSnapshot],
    pub(crate) roster: &'a mut RivalWizardRoster,
    pub(crate) notices: &'a mut PlayerLifecycleNotifications,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Banishment {
    Waiting,
    Revive { castle_slot: u16 },
    Banished { announced: bool },
}

pub(crate) fn step_banished(domains: BanishDomains<'_>, slot: u16) -> Result<Banishment, String> {
    let player_slot = match domains.arena.entities[usize::from(slot)].special_relation {
        EntitySpecialRelation::PlayerState(player_slot) => player_slot,
        _ => return Err(format!("banished rival {slot} has no current player A4")),
    };
    let player_index = domains
        .players
        .iter()
        .position(|player| player.slot == player_slot)
        .ok_or_else(|| format!("banished rival {slot} has no player A4 slot {player_slot}"))?;
    domains
        .roster
        .get_mut_in_arena(domains.arena, slot)
        .ok_or_else(|| format!("banished rival {slot} lost its current owner carrier"))?
        .clear_banished_recoil();
    let outcome = player_lifecycle::step_ai_banished(
        &mut domains.arena.entities[usize::from(slot)],
        &domains.players[player_index],
    )?;
    match outcome {
        player_lifecycle::AiBanishment::Waiting => Ok(Banishment::Waiting),
        player_lifecycle::AiBanishment::Revive { castle_slot } => {
            Ok(Banishment::Revive { castle_slot })
        }
        player_lifecycle::AiBanishment::Banished { announce } => {
            let player = &mut domains.players[player_index];
            if announce {
                domains
                    .notices
                    .publish(
                        player.player_color_index(),
                        slot,
                        PlayerLifeNoticeKind::Banished,
                        crate::level_definition::PLAYER_LIFECYCLE_PROPERTIES
                            .banishment_notice_frames,
                    )
                    .map_err(|error| error.to_string())?;
            }
            player_lifecycle::finish_ai_banishment(player)?;
            if let Some(projected) = domains
                .shared
                .players
                .iter_mut()
                .find(|row| row.slot == player.slot)
            {
                if !projected.set_banishment_state(0) {
                    return Err(format!(
                        "banished rival {slot} lost its shared participation projection"
                    ));
                }
            }
            Ok(Banishment::Banished {
                announced: announce,
            })
        }
    }
}

pub(crate) struct RevivalDomains<'a> {
    pub(crate) arena: &'a mut SourceArena,
    pub(crate) terrain: &'a mut SessionTerrain,
    pub(crate) shared: &'a mut Level2SharedSpellResetSession,
    pub(crate) players: &'a mut [PlayerSnapshot],
    pub(crate) roster: &'a mut RivalWizardRoster,
}

pub(crate) fn revive(
    domains: RevivalDomains<'_>,
    targets: source_target::Context<'_>,
    owner: u16,
    castle_slot: u16,
) -> Result<(), String> {
    let handle = domains
        .arena
        .handle(owner)
        .ok_or("reviving rival source handle is absent")?;
    domains
        .arena
        .rebuild_source_arena_with_terrain(domains.terrain, &[])?;
    let castle_position = source_target::read(
        domains.arena,
        source_target::Context {
            spells: Some(&*domains.shared),
            ..targets
        },
        castle_slot,
    )?
    .ok_or_else(|| format!("reviving rival Castle address {castle_slot} is absent"))?
    .position;
    let actor = domains
        .arena
        .entities
        .get(usize::from(owner))
        .cloned()
        .ok_or("reviving rival actor is absent")?;
    crate::world::effects::sync_position(
        domains.arena,
        domains.terrain,
        owner,
        castle_position,
        true,
        false,
    )?;
    let (mut actor, holders) = reinitialize_rival(
        domains.shared,
        domains.arena,
        domains.terrain,
        castle_slot,
        actor,
        castle_position,
    )
    .map_err(|error| error.to_string())?;
    for holder in holders {
        let slot = holder.index;
        let position = holder.position;
        let active = holder.active;
        domains.arena.entities[usize::from(slot)] = holder;
        crate::world::effects::sync_position(
            domains.arena,
            domains.terrain,
            slot,
            position,
            active,
            false,
        )?;
    }
    actor.set_map_links(
        domains.arena.map_chains.old_map_entity(owner),
        domains.arena.map_chains.next_entity(owner),
    );
    domains.arena.entities[usize::from(owner)] = actor;
    if !domains.roster.publish_revival_player_state(
        handle,
        domains.arena,
        domains.players,
        domains.shared,
    ) {
        return Err("revived rival player state could not be published".to_owned());
    }
    let player = domains
        .players
        .iter()
        .find(|player| player.entity_index == owner)
        .ok_or("revived rival player row is absent")?;
    if !domains
        .roster
        .finish_source_revival(handle, player, domains.arena)
    {
        return Err("revived rival source state could not be rebound".to_owned());
    }
    domains.arena.finish_source_group();
    Ok(())
}

fn reinitialize_rival(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut crate::campaign::source_arena::SourceArena,
    terrain: &mut SessionTerrain,
    castle_slot: u16,
    mut actor: EntitySnapshot,
    castle_position: Position,
) -> NativeResult<(EntitySnapshot, Vec<EntitySnapshot>)> {
    let owner_index = shared
        .players
        .iter()
        .position(|player| player.is_ai && player.entity_index == actor.index)
        .ok_or_else(|| NativeWorldError::new("Castle revival rival player row is absent"))?;
    if !actor.active || actor.life >= 0 || actor.class != 3 || actor.model != 1 {
        return Err(NativeWorldError::new(
            "Castle revival requires the existing dead rival actor",
        ));
    }
    let (pre_revival_life, pre_revival_mana) = (actor.life, actor.mana);
    actor.set_source_action(1);
    actor.set_control_flags(actor.control_flags & !0x20);
    actor.set_position(castle_position);
    let rules = crate::level_definition::PLAYER_LIFECYCLE_PROPERTIES;
    actor.set_max_life(rules.revival_max_life);
    actor.max_mana = rules.revival_max_mana;
    actor.record[144..148].copy_from_slice(&rules.revival_max_mana.to_le_bytes());
    actor.set_life(rules.revival_max_life);
    actor.set_mana(rules.revival_max_mana);
    let mut source_actor = actor.clone();
    source_actor.set_life(pre_revival_life);
    source_actor.set_mana(pre_revival_mana);
    source_actor.set_map_links(
        arena.economy.map_chains.old_map_entity(actor.index),
        arena.economy.map_chains.next_entity(actor.index),
    );
    arena.entities[usize::from(actor.index)] = source_actor;
    shared.sync_player_entity_from_authority(&actor);
    let mut player = shared.players[owner_index].clone();
    player.record[20..24].fill(0);
    player.record[32..38].fill(0);
    player.record[565..569].copy_from_slice(&2_048_i32.to_le_bytes());
    player.record[569..573].copy_from_slice(&2_048_i32.to_le_bytes());
    if let Some(auxiliary) = player.type_str_164_auxiliary.as_mut() {
        auxiliary.dword_0x16D_365 = 2_000;
        let offset = PLAYER_LEGACY_RECORD_BYTES + 17;
        player.record[offset..offset + 4].copy_from_slice(&2_000_i32.to_le_bytes());
    }
    for index in 0..8 {
        let _ = player.set_type_str_164_array_word(4 * index + 4, 24_607);
    }
    shared.players[owner_index] = player.clone();
    let mut holders = Vec::new();
    for family in 0..PLAYER_SPELL_COUNT {
        if player.enabled_spell_holders[family] == 0 {
            continue;
        }
        if level2_set_spell_profile(family as u8, 0).is_none() {
            player.set_enabled_spell_holder_index(family, 0);
            continue;
        }
        let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
            player.set_enabled_spell_holder_index(family, 0);
            continue;
        };
        let Some(source) =
            arena
                .economy
                .initialize_starting_spell_holder_at(slot, family as u8, castle_position)
        else {
            player.set_enabled_spell_holder_index(family, 0);
            continue;
        };
        crate::world::effects::sync_position(arena, terrain, slot, castle_position, true, false)
            .map_err(NativeWorldError::new)?;
        let stage = if family == 2 {
            Level2SharedSpellResetSession::revival_castle_stage(arena, castle_slot)?
        } else {
            None
        };
        let profile = shared
            .physical_spell_profile(family as u8, 0, actor.index, stage)
            .ok_or_else(|| NativeWorldError::new("rival starting holder profile is absent"))?;
        let mut holder =
            Level2SharedSpellResetSession::starting_spell_holder(source, actor.index, 0, profile);
        holder.set_map_links(
            arena.economy.map_chains.old_map_entity(slot),
            arena.economy.map_chains.next_entity(slot),
        );
        arena.entities[usize::from(slot)] = holder.clone();
        player.set_enabled_spell_holder_index(family, holder.index);
        holders.push(holder);
    }
    shared.players[owner_index] = player;
    for family in 0..PLAYER_SPELL_COUNT {
        shared.update_spell_experience(actor.index, family as u8, 0, false);
    }
    let player = shared.players[owner_index].clone();
    for holder in &mut holders {
        let tier = player.family_selected_tiers[usize::from(holder.model)].cast_unsigned();
        let stage = if holder.model == 2 {
            Level2SharedSpellResetSession::revival_castle_stage(arena, castle_slot)?
        } else {
            None
        };
        let profile = shared
            .physical_spell_profile(holder.model, tier, actor.index, stage)
            .ok_or_else(|| NativeWorldError::new("rival selected holder profile is absent"))?;
        holder.apply_nyphur_spell_profile(profile, tier);
        holder.set_map_links(
            arena.economy.map_chains.old_map_entity(holder.index),
            arena.economy.map_chains.next_entity(holder.index),
        );
        shared.register_starting_acquisition(holder, &player);
        if let Some(existing) = shared.pickup_mut(holder.index) {
            *existing = holder.clone();
        } else {
            shared.dynamic_pickups.push(holder.clone());
        }
    }
    let relation = 4 + 4 * usize::from(player.player_color_index());
    let local_entity_index = shared.local_player.entity_index;
    let mut update_local_hostility = false;
    for other in &mut shared.players {
        if shared.player_entities.iter().any(|entity| {
            entity.index == other.entity_index
                && entity.source_id != actor.source_id
                && matches!(entity.model, 0 | 1)
        }) {
            other.set_type_str_164_array_word(relation, (-24_609_i16).cast_unsigned());
            update_local_hostility |= other.entity_index == local_entity_index;
        }
    }
    if update_local_hostility {
        shared
            .local_player
            .set_type_str_164_array_word(relation, (-24_609_i16).cast_unsigned());
    }
    Ok((actor, holders))
}
