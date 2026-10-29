//! Real Skeleton births requested by the source field controller.

use crate::{
    FlightWorld,
    campaign::source_arena::{CreatureRootOrigin, SourceArena},
    level_definition::CreatureKind,
    native_world::{ParticleParameter, SessionTerrain, campaign_shared_families},
    world::fields::skeleton_summon::Birth,
};

/// Read word94 through the current occupant of a frozen model9-list address.
/// Membership is historical; ownership is read anew, including after reuse.
pub(crate) fn current_source_player_entity_index(
    arena: &SourceArena,
    slot: u16,
) -> Result<Option<u16>, String> {
    use crate::native_world::Level2DynamicArenaKind as Kind;

    if slot == 0 {
        return Ok(None);
    }
    let Some(raw) = arena.entities.get(usize::from(slot)) else {
        return Ok(None);
    };
    let kind = arena
        .economy
        .live_arena_cursor_row(slot)
        .map(|cursor| cursor.kind)
        .or_else(|| {
            (!arena.economy.free_source_slots().contains(&slot))
                .then(|| arena.economy.retained_projectile_kind(slot))
                .flatten()
        });
    let Some(kind) = kind else {
        return Ok(Some(raw.source_player_entity_index()));
    };
    let owner = match kind {
        Kind::ManaSphere => arena
            .economy
            .spheres()
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
            .map(|row| row.player_entity_index)
            .ok_or_else(|| format!("Skeleton count lost current sphere at {slot}"))?,
        Kind::CastleRoot => arena
            .economy
            .castle_at(slot)
            .map(|row| row.player_entity_index)
            .ok_or_else(|| format!("Skeleton count lost current Castle at {slot}"))?,
        // Typed creature/common-upkeep ownership is held by the economy's
        // word94 carrier. NewEvent clears that carrier before every reuse;
        // an absent entry is the new occupant's zero, never an old raw row.
        _ => arena.economy.source_owner(slot),
    };
    Ok(Some(owner))
}

/// Source sub_357C0 calls the ordinary sub_4BBB0 constructor, then writes
/// ownership, mana and pose. Rejected placement still consumes phase and RNG.
pub(crate) fn allocate_from_controller(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    descriptor: &[u8; 34],
    birth: Birth,
) -> Result<bool, String> {
    let controller = arena
        .handle(birth.controller_slot)
        .ok_or("Skeleton birth lost its field controller")?;
    let particle = particles
        .get(220)
        .ok_or("Skeleton constructor needs particle220")?;
    let animation_limit = campaign_shared_families::particle_animation_limit(particle.render_mode)
        .ok_or("Skeleton particle220 has no source animation limit")?;
    let pose = campaign_shared_families::ParticlePose {
        animation_limit,
        array_yaw: (particle.rotation_speed / 2) as i16,
        array_pitch: (particle.speed / 2) as i16,
        array_roll: (particle.speed / 2) as i16,
        array_fov: (particle.rotation_speed / 2) as i16,
    };
    let scan_period = u16::from_le_bytes([descriptor[26], descriptor[27]]);
    if scan_period == 0 {
        return Err("Skeleton descriptor80 has a zero scan period".into());
    }
    let material_mask = u32::from_le_bytes(descriptor[20..24].try_into().unwrap());
    let ground_clearance = i16::from_le_bytes([descriptor[12], descriptor[13]]);
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(false);
    };
    let phase = arena.economy.class5_model9_constructor_phase;
    arena.economy.class5_model9_constructor_phase = phase.wrapping_add(1);
    let mut position = birth.position;
    position.z = terrain.ground_height(position);
    let row = campaign_shared_families::construct_skeleton_source(
        slot,
        arena.economy.new_event_rng(slot),
        position,
        phase,
        scan_period,
        pose,
    );
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if campaign_shared_families::skeleton_placement_blocked(
        terrain,
        position,
        material_mask,
        ground_clearance,
    ) {
        let rng = arena.entities[usize::from(slot)].entity_rng;
        crate::world::effects::sync_position(arena, terrain, slot, position, false, false)?;
        arena.entities[usize::from(slot)].reclaim_source_row();
        arena.economy.release_slot(slot, Some(rng));
        return Ok(false);
    }
    // NewEvent can reoccupy the controller's fixed address. Source reads its
    // id only after construction; the historical generation stays provenance.
    let owner = arena.entities[usize::from(birth.controller_slot)].source_id;
    let row = &mut arena.entities[usize::from(slot)];
    row.source_id = owner;
    row.record[30..32].copy_from_slice(&owner.to_le_bytes());
    row.set_source_player_entity_index(owner);
    row.set_mana(i32::from(birth.mana));
    row.set_yaw(birth.yaw);
    row.set_source_roll(birth.yaw);
    let child = arena
        .handle(slot)
        .ok_or("Skeleton birth lost its allocated address")?;
    if !arena.bind_raw_creature_origin(
        child,
        CreatureKind::Skeleton,
        CreatureRootOrigin::FieldSummon { controller },
    ) {
        return Err("Skeleton birth lost its generation-bound field origin".into());
    }
    Ok(true)
}
