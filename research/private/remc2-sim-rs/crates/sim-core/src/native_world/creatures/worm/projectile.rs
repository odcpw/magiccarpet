//! Source `sub_1CC20`: ordinary Fireball birth followed by fixed-address reads.

use crate::Position;
use crate::campaign::source_arena::SourceArena;
use crate::level_definition::WORM_PROPERTIES;
use crate::native_world::level2_dynamic_economy::Level2FireballProjectileLaunchPayload;
use crate::native_world::{FireballProjectileOrigin, SessionTerrain, source_target};

/// The caller has committed the attack's movement/cadence before this call.
/// On success it must apply source5EF70 to the current target, then prepare
/// sound8 at the actual current shooter. Failure prepares neither operation.
pub(crate) fn launch(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    source_slot: u16,
    target_slot: u16,
    context: source_target::Context<'_>,
) -> Result<Option<u16>, String> {
    if source_slot == 0
        || target_slot == 0
        || usize::from(source_slot) >= arena.entities.len()
        || usize::from(target_slot) >= arena.entities.len()
    {
        return Err(format!(
            "Worm shot has invalid source/target address {source_slot}/{target_slot}"
        ));
    }
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    // SummonFireball reads the position pointer after NewEvent has cleared
    // an aliased address. Do not carry a pre-allocation shooter snapshot.
    let initial_position = if slot == source_slot {
        Position::default()
    } else {
        source_target::read(arena, context, source_slot)?
            .ok_or_else(|| format!("Worm shot lost current constructor source{source_slot}"))?
            .position
    };
    let payload = Level2FireballProjectileLaunchPayload {
        origin: FireballProjectileOrigin::Worm,
        position: initial_position,
        yaw: 0,
        pitch: 0,
        axis_position: Position::default(),
        source_id_entity_index: slot,
        holder_entity_index: 0,
        source_subspell_index: 0,
        source_mana: 50,
        source_dword_0x10: 0,
        target_source_entity_index: 0,
        // Literal NewEvent defaults, replaced below by actual current fields.
        target_class: 0,
        target_model: 0,
    };
    if arena
        .economy
        .initialize_fireball_projectile(slot, payload, 0)
        .is_none()
    {
        return Err(format!(
            "Worm Fireball map linkage failed at allocated slot{slot}"
        ));
    }
    crate::world::effects::sync_position(arena, terrain, slot, initial_position, true, false)?;
    crate::world::effects::lighting::admit_source(
        arena,
        terrain,
        slot,
        initial_position,
        crate::world::effects::lighting::SourceLight::Fireball,
    );
    // Source1CC20 now reads these physical pointers. A retained/replaced
    // current occupant is authoritative; missing fields never use raw shadows.
    let source = source_target::read(arena, context, source_slot)?
        .ok_or_else(|| format!("Worm shot lost current source{source_slot} after birth"))?;
    let target = source_target::read(arena, context, target_slot)?
        .ok_or_else(|| format!("Worm shot lost current target{target_slot} after birth"))?;
    let height = source
        .collision
        .ok_or_else(|| format!("Worm shot current source{source_slot} lacks collision axes"))?
        .half_height;
    let guidance = source
        .relations
        .target
        .ok_or_else(|| format!("Worm shot current source{source_slot} lacks word96"))?;
    let (yaw, pitch) =
        crate::level1_mission::projectile_guidance_angles(source.position, target.position);
    let projectile = arena
        .economy
        .fireball_projectiles
        .iter_mut()
        .find(|projectile| projectile.active && projectile.source_entity_index == slot)
        .ok_or_else(|| format!("Worm shot lost its current Fireball at{slot}"))?;
    projectile.owner_entity_index = source.source_id;
    projectile.yaw = yaw;
    projectile.pitch = pitch;
    projectile.position.z = projectile.position.z.wrapping_add(height);
    projectile.target_source_entity_index = guidance;
    projectile.target_class = target.class;
    projectile.target_model = target.model;
    projectile.source_subspell_index = WORM_PROPERTIES.projectile_damage;
    projectile.payload = i32::from(WORM_PROPERTIES.projectile_damage);
    // OriginWorm selects source descriptor65 (yaw0/pitch22) in the canonical
    // scheduled Fireball consumer. Source leaves constructor roll/fov alone.
    arena.economy.source_owner.insert(slot, source.source_id);
    Ok(Some(slot))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_current_worm_and_player_fields_reach_the_real_fireball_carrier() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let player = session.state.entity_index;
        let root = super::super::dynamic::construct_authored(
            &mut arena,
            &mut terrain,
            65_000,
            session.state.position,
            9000,
            &session.session.definition.presentation.particles,
        )
        .unwrap();
        let handle = arena.handle(root.source_entity_index).unwrap();
        let mut actor =
            super::super::dynamic::current_root_packet(&arena, handle.source_slot).unwrap();
        actor.set_source_action(26);
        actor.record[154..156].copy_from_slice(&player.to_le_bytes());
        assert!(super::super::dynamic::commit_root_packet(&mut arena, handle, &actor).unwrap());
        let target_position = Position {
            x: actor.position.x.wrapping_add(1024),
            y: actor.position.y.wrapping_sub(512),
            z: actor.position.z.wrapping_add(320),
        };
        let context = source_target::Context {
            player: Some((player, target_position, 6000)),
            particles: Some(&session.session.definition.presentation.particles),
            ..source_target::Context::default()
        };
        let source = source_target::read(&arena, context, handle.source_slot)
            .unwrap()
            .unwrap();
        // The original typed owner remains authoritative over this poisoned projection.
        let raw = &mut arena.entities[usize::from(handle.source_slot)];
        raw.set_position(Position {
            x: 1,
            y: 2,
            z: -100,
        });
        raw.record[92..94].copy_from_slice(&(-999_i16).to_le_bytes());
        raw.record[154..156].copy_from_slice(&1_u16.to_le_bytes());
        let free = arena.economy.free_slot_count();
        assert!(free > 0);
        let slot = launch(
            &mut arena,
            &mut terrain,
            handle.source_slot,
            player,
            context,
        )
        .unwrap()
        .unwrap();
        assert_eq!(arena.economy.free_slot_count(), free - 1);
        let projectile = arena
            .economy
            .fireball_projectiles
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
            .unwrap();
        let (yaw, pitch) =
            crate::level1_mission::projectile_guidance_angles(source.position, target_position);
        assert_eq!(
            (
                projectile.source_class,
                projectile.source_model,
                projectile.action
            ),
            (9, 0, 0)
        );
        assert_eq!(projectile.origin, FireballProjectileOrigin::Worm);
        assert_eq!(
            projectile.position,
            Position {
                z: source
                    .position
                    .z
                    .wrapping_add(source.collision.unwrap().half_height),
                ..source.position
            }
        );
        assert_eq!(
            (
                projectile.yaw,
                projectile.pitch,
                projectile.roll,
                projectile.fov
            ),
            (yaw, pitch, 0, 0)
        );
        assert_eq!(
            (
                projectile.target_source_entity_index,
                projectile.target_class,
                projectile.target_model
            ),
            (player, 3, 0)
        );
        assert_eq!(
            (
                projectile.owner_entity_index,
                projectile.holder_entity_index
            ),
            (source.source_id, 0)
        );
        assert_eq!(
            (
                projectile.payload,
                projectile.source_subspell_index,
                projectile.source_mana
            ),
            (500, 500, 50)
        );
        assert_eq!(
            (
                projectile.visual_index,
                projectile.actual_speed,
                projectile.life
            ),
            (340, 384, 21)
        );
        assert_eq!(arena.economy.source_owner(slot), source.source_id);
        assert_eq!(arena.handle(handle.source_slot), Some(handle));
        assert_eq!(
            arena.entities[usize::from(handle.source_slot)].position,
            Position {
                x: 1,
                y: 2,
                z: -100
            }
        );
    }
}
