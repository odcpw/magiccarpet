//! Gravity field bindings to the same live actor carriers used by combat.
//! The terrain owner controls rise/hold/reclaim state; this adapter commits
//! its same-pass actor, audio and progression results before the common tail.

use super::{
    ENTITY_PARAMETER_BYTES, Level2OrdinaryResetSession, Level2SharedSpellResetSession,
    PlayerSnapshot, SessionTerrain, area_damage,
};
use crate::{
    FlightState, Position,
    campaign::source_arena::SourceArena,
    goat::GoatFamily,
    world::fields::{gravity, wind},
};

pub(crate) struct Context<'a> {
    pub(crate) arena: &'a mut SourceArena,
    pub(crate) terrain: &'a mut SessionTerrain,
    pub(crate) goats: &'a mut GoatFamily,
    pub(crate) ordinary: &'a mut Level2OrdinaryResetSession,
    pub(crate) player: &'a mut FlightState,
    pub(crate) players: &'a mut [PlayerSnapshot],
    pub(crate) progression: &'a mut Level2SharedSpellResetSession,
    pub(crate) parameters: &'a [[u8; ENTITY_PARAMETER_BYTES]],
}

impl super::Level2EconomyState {
    /// sub_3A090 and sub_396D0 change the Castle's capture bit in the same
    /// carrier that its ordinary callback and projection subsequently read.
    pub(crate) fn set_gravity_castle_capture(&mut self, slot: u16, captured: bool) -> bool {
        let Some(castle) = self
            .castle
            .as_mut()
            .filter(|castle| castle.active && castle.source_entity_index == slot)
        else {
            return false;
        };
        if captured {
            castle.control_flags |= 0x0010_0000;
        } else {
            castle.control_flags &= !0x0010_0000;
        }
        true
    }
}

fn targets(arena: &SourceArena, position: Position) -> Vec<u16> {
    let x = ((u32::from(position.x) + 128) >> 8) as u8;
    let y = ((u32::from(position.y) + 128) >> 8) as u8;
    let mut slots = Vec::new();
    // sub_39B60 scans rows/columns before each oldMapEntity chain, and its
    // circular admission is measured at the unrounded cell origin.
    for dy in 0..26_u8 {
        for dx in 0..26_u8 {
            let tx = x.wrapping_sub(13).wrapping_add(dx);
            let ty = y.wrapping_sub(13).wrapping_add(dy);
            let px = i32::from(position.x.wrapping_sub(u16::from(tx) << 8) as i16);
            let py = i32::from(position.y.wrapping_sub(u16::from(ty) << 8) as i16);
            if px.wrapping_mul(px).wrapping_add(py.wrapping_mul(py)) as u32 >= 0xa9_0000 {
                continue;
            }
            slots.extend(arena.economy.map_chains.entity_chain(tx, ty));
        }
    }
    slots
}

fn award(context: &mut Context<'_>, award: gravity::SpellExperienceAward) {
    if context
        .progression
        .players()
        .iter()
        .any(|player| player.entity_index == award.owner_source_id)
    {
        context.progression.award_spell_experience(
            award.owner_source_id,
            award.family,
            award.amount,
        );
    }
}

pub(crate) fn step(context: &mut Context<'_>, slot: u16) -> Result<(), String> {
    let source = &context.arena.entities[usize::from(slot)];
    let pull_before_shrink = source.action == gravity::ACTION_SHRINK && source.record[74] == 0;
    // sub_3A090 is called when the rise timer reaches five, and before the
    // first shrink. Refresh the existing House projections at that boundary
    // so its overlap and direct-life writes use the current family authority.
    let capture_houses = pull_before_shrink
        || (source.action == gravity::ACTION_RISE
            && source.record[74] == 2
            && source.life > 1
            && source.source_dword_10() == 6);
    let houses = if capture_houses {
        publish_houses(context)?
    } else {
        Vec::new()
    };
    if pull_before_shrink {
        pull_actors(context, slot)?;
    }
    let outcome = gravity::step(context.arena, slot, context.terrain);
    commit_house_destruction(context, &houses)?;
    if let Some(sound) = outcome.sound_id {
        context.arena.economy.prepare_sound(slot, slot, -1, sound);
    }
    if let Some(experience) = outcome.spell_experience {
        award(context, experience);
    }
    if outcome.pull_actors && !pull_before_shrink {
        pull_actors(context, slot)?;
    }
    Ok(())
}

fn publish_houses(context: &mut Context<'_>) -> Result<Vec<(u16, i32, u8)>, String> {
    let mut before = Vec::new();
    for house in context
        .ordinary
        .houses
        .houses()
        .iter()
        .filter(|house| house.source_retained())
    {
        let row = context
            .arena
            .entities
            .get_mut(usize::from(house.entity_index))
            .ok_or("Gravity House projection has no source slot")?;
        house.publish_source(row);
        before.push((house.entity_index, row.life, row.font_type_index));
    }
    for house in context
        .arena
        .economy
        .authored_model45
        .iter()
        .filter(|house| house.active)
    {
        if before
            .iter()
            .any(|(slot, _, _)| *slot == house.source_entity_index)
        {
            continue;
        }
        let row = context
            .arena
            .entities
            .get_mut(usize::from(house.source_entity_index))
            .ok_or("Gravity authored House projection has no source slot")?;
        row.active = true;
        row.class = house.source_class;
        row.model = house.model;
        row.set_position(house.position);
        row.set_life(house.life);
        row.font_type_index = house.font_type_index;
        row.record[65] = house.font_type_index;
        row.set_source_overlap_half_extents(house.view_pitch, house.view_roll);
        before.push((house.source_entity_index, row.life, row.font_type_index));
    }
    Ok(before)
}

fn commit_house_destruction(
    context: &mut Context<'_>,
    before: &[(u16, i32, u8)],
) -> Result<(), String> {
    for &(slot, life, successor) in before {
        let row = context
            .arena
            .entities
            .get(usize::from(slot))
            .ok_or("Gravity House result lost its source slot")?;
        if row.life != -1 || row.font_type_index != 0 || (life == -1 && successor == 0) {
            continue;
        }
        if let Some(house) = context.ordinary.houses.house_mut(slot) {
            house.life = -1;
            house.parameter.destruction_successor = 0;
        } else if let Some(house) = context
            .arena
            .economy
            .authored_model45
            .iter_mut()
            .find(|house| house.active && house.source_entity_index == slot)
        {
            house.life = -1;
            house.font_type_index = 0;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gravity_destruction_reaches_the_live_house_carrier() {
        let mut session = super::super::tests::level_two_campaign_session();
        let live = &mut session.session.state.live;
        let house = *live
            .ordinary
            .houses
            .houses()
            .iter()
            .find(|house| house.source_retained() && house.life >= 0)
            .expect("ordinary live House fixture");
        let well = gravity::construct(
            &mut live.economy,
            &mut live.terrain,
            house.position,
            750,
            120,
        )
        .unwrap()
        .unwrap();
        let source = &mut live.economy.entities[usize::from(well)];
        source.source_id = live.core.state.entity_index;
        source.set_source_action(gravity::ACTION_SHRINK);
        source.record[74] = 0;
        step(
            &mut Context {
                arena: &mut live.economy,
                terrain: &mut live.terrain,
                goats: &mut live.goats,
                ordinary: &mut live.ordinary,
                player: &mut live.core.state,
                players: &mut live.core.players,
                progression: &mut live.shared_spells,
                parameters: &session.session.definition.presentation.entity_parameters,
            },
            well,
        )
        .unwrap();
        let house = live
            .ordinary
            .houses
            .houses()
            .iter()
            .find(|current| current.entity_index == house.entity_index)
            .unwrap();
        assert_eq!(house.life, -1);
        assert_eq!(house.parameter.destruction_successor, 0);
        // Publishing the next source visit must keep, rather than undo, the
        // direct life/successor write made by the Gravity callback.
        let source = &mut live.economy.entities[usize::from(house.entity_index)];
        house.publish_source(source);
        assert_eq!((source.life, source.font_type_index), (-1, 0));
    }

    #[test]
    fn gravity_capture_reaches_castle_mailbox_and_releases_inactive_projection() {
        let mut session = super::super::tests::level_two_campaign_session();
        let live = &mut session.session.state.live;
        let owner = live.core.state.entity_index;
        let castle_slot = live
            .economy
            .economy
            .castle()
            .filter(|castle| castle.active)
            .map(|castle| castle.source_entity_index)
            .unwrap_or_else(|| {
                live.economy
                    .economy
                    .build_castle(live.core.state.position, owner, &live.terrain)
                    .expect("real Castle constructor")
            });
        let castle = live.economy.economy.castle().unwrap();
        assert!(castle.life > 750);
        let before_life = castle.life;
        // Dynamic Castle state is authoritative even without an active raw
        // draw projection. Capture must retain its link for phase-two release.
        live.economy.entities[usize::from(castle_slot)].active = false;
        let well = gravity::construct(
            &mut live.economy,
            &mut live.terrain,
            castle.position,
            750,
            120,
        )
        .unwrap()
        .unwrap();
        let source = &mut live.economy.entities[usize::from(well)];
        source.source_id = owner;
        source.record[30..32].copy_from_slice(&owner.to_le_bytes());
        source.set_source_action(gravity::ACTION_SHRINK);
        source.record[74] = 0;
        gravity::step(&mut live.economy, well, &mut live.terrain);
        let castle = live.economy.economy.castle().unwrap();
        assert_eq!(
            (castle.pending_damage, castle.pending_attacker_entity_index),
            (750, owner)
        );
        assert_eq!(castle.source_word_30, 30);
        assert_ne!(castle.control_flags & 0x0010_0000, 0);
        let raw = &live.economy.entities[usize::from(castle_slot)];
        assert!(!raw.active);
        assert_eq!(u16::from_le_bytes([raw.record[42], raw.record[43]]), well);
        assert_eq!(
            live.economy.economy.resolve_damage(castle_slot),
            Some(false)
        );
        let castle = live.economy.economy.castle().unwrap();
        assert_eq!(castle.life, before_life - 750);
        assert_eq!(castle.pending_damage, 0);
        live.economy.entities[usize::from(well)].record[74] = 2;
        gravity::step(&mut live.economy, well, &mut live.terrain);
        let castle = live.economy.economy.castle().unwrap();
        assert_eq!(castle.control_flags & 0x0010_0000, 0);
        let raw = &live.economy.entities[usize::from(castle_slot)];
        assert_eq!(&raw.record[42..44], &[0, 0]);
        assert!(!raw.active);
        assert!(live.economy.entities[usize::from(well)].drawing_disabled());
    }
}

fn pull_actors(context: &mut Context<'_>, slot: u16) -> Result<(), String> {
    let position = context.arena.entities[usize::from(slot)].position;
    let candidates = targets(context.arena, position);
    for target_slot in candidates {
        let Some(raw) = context
            .arena
            .entities
            .get(usize::from(target_slot))
            .cloned()
        else {
            continue;
        };
        let Some(target) = area_damage::source_actor(
            context.arena,
            Some(area_damage::ActorDomains::new(
                context.goats,
                context.ordinary,
            )),
            target_slot,
        ) else {
            continue;
        };
        let parameters = raw
            .parameter_relation
            .and_then(|index| context.parameters.get(usize::from(index)));
        let mut wind_actor = wind::Actor::from_source(&raw, parameters.map(|p| p.as_slice()));
        wind_actor.position = target.position;
        wind_actor.class = target.class;
        wind_actor.model = target.model;
        wind_actor.flags = target.control_flags;
        wind_actor = context
            .arena
            .economy
            .wind_actor(target_slot, wind_actor)
            .unwrap_or(wind_actor);
        wind_actor = context
            .ordinary
            .villagers
            .wind_actor(target_slot)
            .unwrap_or(wind_actor);
        if let Some(goat) = context
            .goats
            .goats()
            .iter()
            .find(|goat| goat.active && goat.entity_index == target_slot)
        {
            wind_actor.position = goat.position;
            wind_actor.yaw = goat.yaw;
            wind_actor.rng = goat.rng;
            wind_actor.flags = goat.control_flags;
            wind_actor.move_heading = goat.initial_lifetime;
        }
        if target_slot == context.player.entity_index {
            wind_actor.position = context.player.position;
            wind_actor.yaw = context.player.yaw;
        }
        let life = area_damage::source_life(
            context.arena,
            Some(area_damage::ActorDomains::new(
                context.goats,
                context.ordinary,
            )),
            target_slot,
        )
        .ok_or("gravity target has no authoritative life carrier")?;
        let (bounty, bounty_attacker) = area_damage::source_bounty(
            context.arena,
            Some(area_damage::ActorDomains::new(
                context.goats,
                context.ordinary,
            )),
            target_slot,
        )?;
        let actor = gravity::Actor {
            source_id: target.source_id,
            class: target.class,
            model: target.model,
            action: raw.action,
            control_flags: wind_actor.flags,
            position: wind_actor.position,
            vertical_state: parameters.map_or(0, |p| i16::from_le_bytes([p[14], p[15]])),
            pitch: raw.pitch,
            life,
            bounty_enabled: raw.record[60] & 1 != 0,
            bounty,
            bounty_attacker,
            capture_link: u16::from_le_bytes([raw.record[42], raw.record[43]]),
            is_current_level_player: target_slot == context.player.entity_index,
        };
        let motion = gravity::pull_actor(
            &mut context.arena.entities[usize::from(slot)],
            actor,
            context.terrain,
        );
        let Some(motion) = motion else {
            continue;
        };
        if let Some(rng) = motion.source_rng {
            context.arena.economy.source_rng.insert(slot, rng);
        }
        let mut actor_motion = wind::Motion {
            actor: wind_actor,
            actual_speed: None,
            player_roll_delta: 0,
            contact: false,
        };
        actor_motion.actor.position = motion.actor.position;
        actor_motion.actor.flags = motion.actor.control_flags;
        if let Some(castle) = context
            .arena
            .economy
            .castle
            .as_mut()
            .filter(|castle| castle.active && castle.source_entity_index == target_slot)
        {
            // Model2 fails pull admission but still reaches action74 LABEL25.
            castle.control_flags = motion.actor.control_flags;
        }
        context
            .arena
            .economy
            .apply_wind_motion(target_slot, actor_motion);
        context
            .ordinary
            .villagers
            .apply_wind_motion(target_slot, actor_motion);
        context
            .goats
            .apply_external_motion(context.arena, target_slot, actor_motion)?;
        actor_motion.write_source(&mut context.arena.entities[usize::from(target_slot)]);
        context.arena.entities[usize::from(target_slot)].set_pitch(motion.actor.pitch);
        if motion.actor.bounty != bounty || motion.actor.bounty_attacker != bounty_attacker {
            area_damage::replace_source_bounty(
                context.arena,
                Some(area_damage::ActorDomains::new(
                    context.goats,
                    context.ordinary,
                )),
                target_slot,
                motion.actor.bounty,
                motion.actor.bounty_attacker,
            )?;
        }
        if target_slot == context.player.entity_index {
            context.player.position = motion.actor.position;
            if motion.contact && motion.actor.model == 0 {
                context.player.display_pitch = 512;
                context.player.effective_pitch = 512;
            }
            actor_motion.write_source(&mut context.progression.player_entity);
            context
                .progression
                .player_entity
                .set_pitch(motion.actor.pitch);
            context
                .progression
                .sync_typed_spell_parent_caches_from_player_entity();
        }
        if motion.contact && (motion.actor.class, motion.actor.model) == (3, 0) {
            for player in context
                .players
                .iter_mut()
                .filter(|player| player.entity_index == target_slot)
            {
                // exporter_main.cpp writes compact roll341 at46 and pitch343
                // at48; these are not offsets into the appended auxiliary row.
                let offset = 48;
                player
                    .record
                    .get_mut(offset..offset + 2)
                    .ok_or("gravity player pitch carrier is truncated")?
                    .copy_from_slice(&512_i16.to_le_bytes());
            }
        }
        crate::world::effects::sync_position(
            context.arena,
            context.terrain,
            target_slot,
            motion.actor.position,
            true,
            false,
        )?;
        if let Some(experience) = motion.spell_experience {
            award(context, experience);
        }
    }
    Ok(())
}
