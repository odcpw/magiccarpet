//! Source constructors shared by creature attacks that launch guided effects.

use crate::Position;
use crate::campaign::source_arena::SourceArena;
use crate::native_world::{
    FireballProjectileOrigin, ParticleParameter, SessionTerrain, cave_features,
    level2_dynamic_economy::Level2FireballProjectileLaunchPayload, lightning,
};

/// The constructor product, not the terminal effect selector. Fireballs use
/// the existing economy carrier; other projectiles use source arena rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Product {
    FinalBossFireball,
    Creature27Fireball,
    Lightning,
    Whirlwind,
    Meteor,
}

/// Writes made by the real creature callback after the source constructor.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Launch {
    pub product: Product,
    pub position: Position,
    pub owner: u16,
    pub yaw: i16,
    pub pitch: i16,
    pub target_class: u8,
    pub target_model: u8,
    /// Source word96 is a fixed arena address; zero means unbound.
    pub target: u16,
    pub damage: u16,
    pub effect_life: u8,
    /// Overrides constructor descriptor only when the source producer does.
    pub descriptor: Option<u16>,
}

/// Allocate a real source row for the ordinary scheduler. A full arena
/// returns `None`, allowing the producer to suppress its success sound.
pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    launch: Launch,
) -> Result<Option<u16>, String> {
    construct_at_address(arena, terrain, particles, launch, None)
}

fn construct_at_address(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    launch: Launch,
    position_slot: Option<u16>,
) -> Result<Option<u16>, String> {
    if matches!(
        launch.product,
        Product::FinalBossFireball | Product::Creature27Fireball
    ) {
        let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
            return Ok(None);
        };
        let position = if position_slot == Some(slot) {
            Position::default()
        } else {
            launch.position
        };
        let origin = match launch.product {
            Product::FinalBossFireball => FireballProjectileOrigin::FinalBoss,
            Product::Creature27Fireball => FireballProjectileOrigin::Creature27,
            _ => unreachable!(),
        };
        let payload = Level2FireballProjectileLaunchPayload {
            origin,
            position,
            yaw: launch.yaw,
            pitch: launch.pitch,
            axis_position: Position::default(),
            source_id_entity_index: launch.owner,
            holder_entity_index: 0,
            source_subspell_index: launch.damage,
            source_mana: 50,
            source_dword_0x10: 0,
            target_source_entity_index: launch.target,
            target_class: launch.target_class,
            target_model: launch.target_model,
        };
        let slot = arena
            .economy
            .initialize_fireball_projectile(slot, payload, 0)
            .ok_or_else(|| format!("creature Fireball map linkage failed at slot{slot}"))?;
        crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
        crate::world::effects::lighting::admit_source(
            arena,
            terrain,
            slot,
            position,
            crate::world::effects::lighting::SourceLight::Fireball,
        );
        return Ok(Some(slot));
    }
    let (impact, slot) = match launch.product {
        Product::Lightning => (
            23,
            lightning::construct_at_address(
                arena,
                terrain,
                particles,
                lightning::Product::Ray,
                launch.position,
                position_slot,
            )?,
        ),
        Product::Whirlwind | Product::Meteor => {
            let (model, action, particle, impact) = match launch.product {
                Product::Whirlwind => (26, 27, 320, 22),
                Product::Meteor => (3, 3, 76, 17),
                _ => unreachable!(),
            };
            let particle_row = *particles
                .get(usize::from(particle))
                .ok_or_else(|| format!("projectile requires retained particle{particle}"))?;
            let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
                return Ok(None);
            };
            let mut row =
                cave_features::new_event(slot, arena.economy.new_event_rng(slot), launch.position);
            row.class = 9;
            row.model = model;
            row.record[67] = 9;
            row.record[68] = model;
            row.set_source_action(action);
            row.set_control_flags((row.control_flags & !8) | 4);
            row.set_actual_speed(384);
            row.record[136..138].copy_from_slice(&384_i16.to_le_bytes());
            row.set_mana(50);
            row.set_max_life(8192 / 384);
            row.set_life(row.max_life);
            row.parameter_relation = Some(60);
            row.record[164..168].copy_from_slice(&61_u32.to_le_bytes());
            cave_features::bind_particle(&mut row, particle, particle_row)?;
            arena.economy.source_rng.insert(slot, row.entity_rng);
            arena.entities[usize::from(slot)] = row;
            crate::world::effects::sync_position(
                arena,
                terrain,
                slot,
                launch.position,
                true,
                true,
            )?;
            (impact, Some(slot))
        }
        Product::FinalBossFireball | Product::Creature27Fireball => unreachable!(),
    };
    let Some(slot) = slot else { return Ok(None) };
    let row = &mut arena.entities[usize::from(slot)];
    row.source_id = launch.owner;
    row.record[30..32].copy_from_slice(&launch.owner.to_le_bytes());
    row.set_yaw(launch.yaw);
    row.set_pitch(launch.pitch);
    row.record[69] = launch.target_class;
    row.record[70] = launch.target_model;
    row.record[71] = 10;
    row.record[72] = impact;
    row.record[154..156].copy_from_slice(&launch.target.to_le_bytes());
    row.set_subspell_index(launch.damage);
    row.set_source_byte_46(launch.effect_life);
    if let Some(descriptor) = launch.descriptor {
        row.parameter_relation = Some(descriptor);
        row.record[164..168].copy_from_slice(&(u32::from(descriptor) + 1).to_le_bytes());
    }
    Ok(Some(slot))
}

/// `sub_2A7F0`: NewEvent may replace the target's fixed address, so aim and
/// target selectors are read after allocation, from its current occupant.
pub(crate) fn construct_model27(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    targets: crate::native_world::source_target::Context<'_>,
    head_slot: u16,
    target_slot: u16,
    selector: u8,
) -> Result<Option<u16>, String> {
    let product = match selector {
        0 => Product::Creature27Fireball,
        9 => Product::Lightning,
        _ => {
            return Err(format!(
                "unsupported model27 projectile selector {selector}"
            ));
        }
    };
    let head = arena
        .entities
        .get(usize::from(head_slot))
        .ok_or("model27 head address is outside source arena")?;
    let (position, owner) = (head.position, head.source_id);
    let Some(slot) = construct_at_address(
        arena,
        terrain,
        particles,
        Launch {
            product,
            position,
            owner,
            yaw: 0,
            pitch: 0,
            target_class: u8::MAX,
            target_model: u8::MAX,
            target: 0,
            damage: 850,
            effect_life: 0,
            descriptor: None,
        },
        Some(head_slot),
    )?
    else {
        return Ok(None);
    };

    // The source passes &head.position through NewEvent's memset. If the
    // allocator replaces the head itself, that address now contains the new
    // projectile, whose position is initially zero rather than the old head.
    if slot == head_slot {
        if product == Product::Creature27Fireball {
            let projectile = arena
                .economy
                .fireball_projectiles
                .iter_mut()
                .find(|row| row.source_entity_index == slot)
                .ok_or("new model27 Fireball carrier is absent")?;
            projectile.position = Position::default();
            projectile.owner_entity_index = slot;
            arena.economy.source_owner.insert(slot, slot);
        } else {
            let row = &mut arena.entities[usize::from(slot)];
            row.set_position(Position::default());
            row.source_id = slot;
            row.record[30..32].copy_from_slice(&slot.to_le_bytes());
        }
        crate::world::effects::sync_position(
            arena,
            terrain,
            slot,
            Position::default(),
            true,
            false,
        )?;
    }

    let (head_position, head_owner, head_fov, head_target) =
        if slot == head_slot && product == Product::Creature27Fireball {
            let particle = particles
                .get(340)
                .ok_or("model27 Fireball requires particle340")?;
            (
                Position::default(),
                slot,
                (particle.rotation_speed / 2) as i16,
                0,
            )
        } else {
            let head = &arena.entities[usize::from(head_slot)];
            (
                head.position,
                head.source_id,
                head.array_fov(),
                u16::from_le_bytes([head.record[154], head.record[155]]),
            )
        };
    let target = crate::native_world::source_target::read(arena, targets, target_slot)?
        .ok_or("model27 target address is outside source arena")?;
    let (target_position, target_class, target_model) =
        (target.position, target.class, target.model);
    let (yaw, pitch) =
        crate::level1_mission::projectile_guidance_angles(head_position, target_position);
    if product == Product::Creature27Fireball {
        let projectile = arena
            .economy
            .fireball_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .ok_or("new model27 Fireball carrier is absent")?;
        projectile.owner_entity_index = head_owner;
        projectile.yaw = yaw;
        projectile.pitch = pitch;
        projectile.position.z = projectile.position.z.wrapping_add(head_fov / 2);
        projectile.target_source_entity_index = head_target;
        projectile.target_class = target_class;
        projectile.target_model = target_model;
        arena.economy.source_owner.insert(slot, head_owner);
    } else {
        let row = &mut arena.entities[usize::from(slot)];
        row.source_id = head_owner;
        row.record[30..32].copy_from_slice(&head_owner.to_le_bytes());
        row.set_yaw(yaw);
        row.set_pitch(pitch);
        let mut position = row.position;
        position.z = position.z.wrapping_add(head_fov / 2);
        row.set_position(position);
        row.record[154..156].copy_from_slice(&head_target.to_le_bytes());
        let descriptor = crate::world::effects::MODEL27_GUIDANCE;
        row.parameter_relation = Some(descriptor.source_index);
        row.record[164..168]
            .copy_from_slice(&(u32::from(descriptor.source_index) + 1).to_le_bytes());
        row.record[69] = target_class;
        row.record[70] = target_model;
    }
    Ok(Some(slot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::native_world::source_target;
    use std::path::PathBuf;

    #[test]
    fn hidden_fireball_is_retained_by_bare_rebuild_but_reclaimed_by_source_49f90() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level4");
        let game = CampaignSession::open(package, CampaignSelection::ordinary(4).unwrap())
            .expect("real Cave4 owner package");
        let mut original = game.simulation().clone();
        let particles = original.session.definition.presentation.particles.clone();
        let live = &mut original.session.state.live;
        let position = live.core.state.position;
        let owner = live.core.state.entity_index;
        let slot = construct(
            &mut live.economy,
            &mut live.terrain,
            &particles,
            Launch {
                product: Product::FinalBossFireball,
                position,
                owner,
                yaw: 17,
                pitch: 23,
                target_class: 3,
                target_model: 0,
                target: owner,
                damage: 800,
                effect_life: 0,
                descriptor: None,
            },
        )
        .unwrap()
        .expect("ordinary Fireball constructor allocates");
        let projectile = live
            .economy
            .economy
            .fireball_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        projectile.life = 7;
        projectile.active = false;
        projectile.drawing_disabled = true;

        let mut bare = original.clone();
        let arena = &mut bare.session.state.live.economy;
        arena.rebuild_source_free_arena();
        assert!(
            arena
                .economy
                .fireball_projectiles
                .iter()
                .any(|row| row.source_entity_index == slot)
        );
        assert!(!arena.economy.free_source_slots().contains(&slot));
        let retained = source_target::read(arena, source_target::Context::default(), slot)
            .unwrap()
            .expect("hidden typed source remains readable");
        assert_eq!(
            (
                retained.class,
                retained.model,
                retained.life,
                retained.position
            ),
            (9, 0, 7, position)
        );
        assert!(retained.drawing_disabled());

        let live = &mut original.session.state.live;
        live.economy
            .rebuild_source_arena_with_terrain(&mut live.terrain, &[])
            .expect("explicit source 49F90 rebuild");
        assert!(
            !live
                .economy
                .economy
                .fireball_projectiles
                .iter()
                .any(|row| row.source_entity_index == slot)
        );
        assert!(live.economy.economy.free_source_slots().contains(&slot));
        let retained = source_target::read(&live.economy, source_target::Context::default(), slot)
            .unwrap()
            .expect("freed source address retains its bytes");
        assert_eq!(
            (
                retained.class,
                retained.model,
                retained.life,
                retained.position
            ),
            (0, 0, 7, position)
        );
        assert!(retained.drawing_disabled());
        let mut lights = std::mem::take(&mut live.economy.dynamic_lighting);
        crate::world::effects::lighting::after_updates(
            &mut lights,
            &live.economy,
            &mut live.terrain,
            source_target::Context::default(),
        )
        .expect("reclaimed source remains readable by light service");
        assert!(
            lights
                .active_slots()
                .all(|(_, light)| light.source_entity_index != slot)
        );
    }
}
