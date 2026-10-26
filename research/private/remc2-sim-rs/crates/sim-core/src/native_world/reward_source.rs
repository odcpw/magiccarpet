//! Hidden source10/91 rewards, including the final volcano's real product.
//! Mana spheres enter the existing economy and its ordinary collection path.

use super::{EntitySnapshot, Level2SharedSpellResetSession, SessionTerrain};
use crate::{FlightWorld, Position, campaign::source_arena::SourceArena};

/// sub_4EF30: no map insertion and no visible particle.
pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    owner: u16,
) -> Option<u16> {
    let slot = arena.allocate_source_slot_with_terrain(terrain)?;
    let mut row =
        super::cave_features::new_event(slot, arena.economy.new_event_rng(slot), position);
    row.class = 10;
    row.model = 91;
    row.record[67] = 10;
    row.record[68] = 91;
    row.set_source_action(98);
    row.set_subspell_index(200);
    row.set_source_dword_10(0);
    row.set_max_life(10000);
    row.set_life(10000);
    row.set_control_flags((row.control_flags & !9) | 1);
    row.source_id = owner;
    row.record[30..32].copy_from_slice(&owner.to_le_bytes());
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    Some(slot)
}

fn draw(row: &mut EntitySnapshot) -> u16 {
    let value = row.entity_rng.wrapping_mul(9377).wrapping_add(9439);
    row.set_entity_rng(value);
    value
}

fn sphere(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    source: &mut EntitySnapshot,
) -> Result<(), String> {
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(());
    };
    // sub_4FA60's NewEvent and its private two draws precede all five parent
    // draws. Its standard sphere is then overwritten by sub_32CF0.
    arena.economy.initialize_death_mana_sphere_at_slot(
        slot,
        source.index,
        source.position,
        0,
        0,
        0,
        terrain,
    );
    let speed = (draw(source) % 0x300).max(64) as i16;
    let vertical_velocity = ((draw(source) & 0x7f) + 128) as i16;
    let variant = (draw(source) % 9).wrapping_sub(1);
    let mana = i32::from(draw(source) % 0xa00) + 1;
    let ordinal = crate::level1_mission::mana_sphere_size_ordinal(mana);
    let base = if variant == u16::MAX {
        52
    } else {
        105 + 8 * [0_u16, 1, 4, 3, 2, 5, 7, 6][usize::from(variant)]
    };
    let yaw = (draw(source) & 0x7ff) as i16;
    let mut velocity = Position::default();
    crate::move_entity(&mut velocity, yaw, 0, speed);
    let mut position = source.position;
    position.z = terrain.ground_height(position).wrapping_add(96);
    let sphere = arena
        .economy
        .spheres
        .iter_mut()
        .find(|sphere| sphere.source_entity_index == slot)
        .ok_or("reward sphere constructor lost its typed carrier")?;
    sphere.position = position;
    sphere.velocity_x = velocity.x as i16;
    sphere.velocity_y = velocity.y as i16;
    sphere.vertical_velocity = vertical_velocity;
    sphere.yaw = yaw;
    sphere.max_life = 140;
    sphere.life = 140;
    sphere.mana = mana;
    sphere.control_flags |= 0x2000;
    sphere.particle_index = base + u16::from(ordinal);
    sphere.collision_yaw = 25 * (i16::from(ordinal) + 1);
    sphere.collision_pitch = 0;
    sphere.collision_roll = 0;
    sphere.collision_fov = sphere.collision_yaw;
    if let Some(super::Level2EconomyEvent::ManaSphereSpawned {
        mana: event_mana, ..
    }) = arena.economy.events.last_mut()
    {
        *event_mana = mana;
    }
    // Publish the ordinary sphere's existing map chain, including live terrain
    // heads. Dynamic dispatch/phase and reclaim belong to the sphere owner.
    crate::world::effects::sync_position(arena, terrain, slot, position, true, true)?;
    Ok(())
}

/// sub_32CF0 has no life decrement or hide. Each failed allocation leaves the
/// parent stream untouched and does not skip later attempts or progression.
pub(crate) fn step(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    shared: &mut Level2SharedSpellResetSession,
    mission_player: u16,
    slot: u16,
) -> Result<(), String> {
    let mut source = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && (row.class, row.model, row.action) == (10, 91, 98))
        .cloned()
        .ok_or("reward source callback has no live source91")?;
    for _ in 0..3 {
        sphere(arena, terrain, &mut source)?;
    }
    if source.phase & 1 == 0 {
        if !shared
            .players()
            .iter()
            .any(|player| player.entity_index == mission_player)
        {
            return Err("reward source mission player is absent".into());
        }
        for (family, thresholds) in super::LEVEL2_SPELL_TIER_THRESHOLDS.iter().enumerate() {
            let threshold = thresholds[2];
            let sign = threshold >> 31;
            let amount = (threshold - (sign << 9) + sign) >> 9;
            shared.award_spell_experience(mission_player, family as u8, amount);
        }
    }
    arena.economy.source_rng.insert(slot, source.entity_rng);
    // NewEvent may have replaced the source's fixed address.
    if let Some(row) = arena.entities.get_mut(usize::from(slot))
        && (row.class, row.model, row.action) == (10, 91, 98)
    {
        row.set_entity_rng(source.entity_rng);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reward_birth_emits_real_spheres_and_even_phase_experience_without_self_expiry() {
        let mut session = super::super::tests::level_two_campaign_session();
        let live = &mut session.session.state.live;
        let player = live.core.state.entity_index;
        let slot = construct(
            &mut live.economy,
            &mut live.terrain,
            live.core.state.position,
            player,
        )
        .expect("available real source slot");
        let source = &mut live.economy.entities[usize::from(slot)];
        assert_eq!((source.class, source.model, source.action), (10, 91, 98));
        assert_eq!(
            source.control_flags & 9,
            1,
            "source hidden, non-damageable flags"
        );
        assert!(
            !source.drawing_disabled(),
            "hidden source91 remains scheduled"
        );
        source.set_phase(0);
        let rng = source.entity_rng;
        let count = live.economy.spheres.len();
        let xp = live.shared_spells.players()[0].runtime_spell_experience;
        step(
            &mut live.economy,
            &mut live.terrain,
            &mut live.shared_spells,
            player,
            slot,
        )
        .unwrap();
        assert_eq!(live.economy.spheres.len(), count + 3);
        for sphere in &live.economy.spheres[count..] {
            assert_eq!((sphere.max_life, sphere.life), (140, 140));
            assert!(sphere.mana > 0 && sphere.mana <= 2560);
            assert_ne!(sphere.control_flags & 0x2000, 0);
            let tile = live.economy.map_chains.tile_of(sphere.source_entity_index);
            assert_eq!(
                live.terrain.planes().entity_indices[usize::from(tile)],
                live.economy.map_chains.tile_head(tile) as i16
            );
        }
        let expected_rng = (0..15).fold(rng, |rng, _| rng.wrapping_mul(9377).wrapping_add(9439));
        assert_eq!(
            live.economy.entities[usize::from(slot)].entity_rng,
            expected_rng
        );
        assert_eq!(live.economy.entities[usize::from(slot)].life, 10000);
        assert_eq!(
            live.economy.entities[usize::from(slot)].phase,
            0,
            "scheduler owns phase"
        );
        assert_ne!(live.shared_spells.players()[0].runtime_spell_experience, xp);
        live.economy.entities[usize::from(slot)].set_phase(1);
        let xp = live.shared_spells.players()[0].runtime_spell_experience;
        step(
            &mut live.economy,
            &mut live.terrain,
            &mut live.shared_spells,
            player,
            slot,
        )
        .unwrap();
        assert_eq!(live.economy.spheres.len(), count + 6);
        assert_eq!(live.shared_spells.players()[0].runtime_spell_experience, xp);
    }
}
