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
    FireflyFireball,
    DragonFireball,
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
        Product::FinalBossFireball
            | Product::Creature27Fireball
            | Product::FireflyFireball
            | Product::DragonFireball
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
            Product::FireflyFireball => FireballProjectileOrigin::Firefly,
            Product::DragonFireball => FireballProjectileOrigin::Dragon,
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
        Product::FinalBossFireball
        | Product::Creature27Fireball
        | Product::FireflyFireball
        | Product::DragonFireball => unreachable!(),
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

/// `sub_1CC20`: a Firefly or Dragon launches one 9/0 from the current
/// shooter address. The caller owns the successful target-side `sub_5EF70`.
pub(crate) fn construct_creature_fireball(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    targets: crate::native_world::source_target::Context<'_>,
    shooter_slot: u16,
    target_slot: u16,
    origin: FireballProjectileOrigin,
    damage: u16,
) -> Result<Option<u16>, String> {
    let product = match origin {
        FireballProjectileOrigin::Firefly => Product::FireflyFireball,
        FireballProjectileOrigin::Dragon => Product::DragonFireball,
        _ => return Err(format!("unsupported 1CC20 Fireball origin {origin:?}")),
    };
    let targets = crate::native_world::source_target::Context {
        particles: Some(particles),
        map_type: Some(terrain.map_type()),
        ..targets
    };
    let shooter = crate::native_world::source_target::read(arena, targets, shooter_slot)?
        .ok_or("1CC20 shooter address is outside source arena")?;
    let Some(slot) = construct_at_address(
        arena,
        terrain,
        particles,
        Launch {
            product,
            position: shooter.position,
            owner: shooter.source_id,
            yaw: 0,
            pitch: 0,
            target_class: u8::MAX,
            target_model: u8::MAX,
            target: 0,
            damage,
            effect_life: 0,
            descriptor: None,
        },
        Some(shooter_slot),
    )?
    else {
        return Ok(None);
    };
    if slot == shooter_slot {
        // NewEvent replaced the current source pointer; its default id is
        // its own slot, before sub_1CC20 copies that same current id.
        let projectile = arena
            .economy
            .fireball_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .ok_or("new 1CC20 Fireball carrier is absent")?;
        projectile.owner_entity_index = slot;
        arena.economy.source_owner.insert(slot, slot);
    }
    let shooter = crate::native_world::source_target::read(arena, targets, shooter_slot)?
        .ok_or("1CC20 current shooter address is outside source arena")?;
    let target = crate::native_world::source_target::read(arena, targets, target_slot)?
        .ok_or("1CC20 target address is outside source arena")?;
    let fov = shooter
        .collision
        .ok_or("1CC20 current shooter has no source collision axes")?
        .half_height;
    let bound = shooter
        .relations
        .target
        .ok_or("1CC20 current shooter has no source word96")?;
    let (yaw, pitch) =
        crate::level1_mission::projectile_guidance_angles(shooter.position, target.position);
    let projectile = arena
        .economy
        .fireball_projectiles
        .iter_mut()
        .find(|row| row.source_entity_index == slot)
        .ok_or("new 1CC20 Fireball carrier is absent")?;
    projectile.owner_entity_index = shooter.source_id;
    projectile.position.z = projectile.position.z.wrapping_add(fov);
    projectile.yaw = yaw;
    projectile.pitch = pitch;
    projectile.target_source_entity_index = bound;
    projectile.target_class = target.class;
    projectile.target_model = target.model;
    arena.economy.source_owner.insert(slot, shooter.source_id);
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
    sound: &mut dyn FnMut(&SourceArena, u16, u16),
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
    // Source2A7F0 prepares sound at the head's current ID after successful
    // construction/payload850 and before the later owner/aim/target writes.
    // The addressed emitter may itself be the new current occupant.
    sound(arena, head_owner, if selector == 0 { 15 } else { 23 });
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
    #[cfg(test)]
    if product == Product::Creature27Fireball {
        crate::native_world::area_damage::trace_fireball_launch(arena, slot, head_owner);
    }
    Ok(Some(slot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FlightWorld;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::native_world::source_target;
    use std::path::PathBuf;

    #[test]
    fn firefly_1cc20_uses_the_current_target_and_real_allocator() {
        use crate::level_definition::EffectKind;

        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2");
        let game = CampaignSession::open(package, CampaignSelection::LEVEL2)
            .expect("real Level2 owner package");
        let mut session = game.simulation().clone();
        let particles = session.session.definition.presentation.particles.clone();
        let live = &mut session.session.state.live;
        let arena = &mut live.economy;
        let terrain = &mut live.terrain;
        // This qualified package has no initial Firefly. Construct the real
        // unbound carrier for this allocator control; the scheduled authored
        // producer is verified separately by the creature family.
        let shooter_slot = arena
            .economy
            .allocate_authored_firefly_unbound(
                683,
                live.core.state.position,
                crate::level_definition::FIREFLY_CLASS_PROPERTIES.max_life as u32,
                terrain,
            )
            .expect("ordinary Firefly constructor has source capacity")
            .source_entity_index;
        let target_slot = live.core.state.entity_index;
        arena
            .economy
            .authored_class5
            .iter_mut()
            .find(|row| row.source_entity_index == shooter_slot)
            .unwrap()
            .word_0x96_150 = target_slot;
        let context = source_target::Context {
            player: Some((target_slot, live.core.state.position, live.player_life)),
            particles: Some(&particles),
            map_type: Some(terrain.map_type()),
            ..Default::default()
        };
        let shooter = source_target::read(arena, context, shooter_slot)
            .unwrap()
            .unwrap();
        let target = source_target::read(arena, context, target_slot)
            .unwrap()
            .unwrap();
        let (yaw, pitch) =
            crate::level1_mission::projectile_guidance_angles(shooter.position, target.position);
        let mut full = arena.clone();
        let mut full_terrain = terrain.clone();
        while full.economy.free_slot_count() != 0 {
            full.allocate_source_slot_with_terrain(&mut full_terrain)
                .unwrap();
        }
        full.finish_source_group();
        let before = full.economy.fireball_projectiles.len();
        let lights = full.dynamic_lighting.active_count();
        assert_eq!(
            construct_creature_fireball(
                &mut full,
                &mut full_terrain,
                &particles,
                context,
                shooter_slot,
                target_slot,
                FireballProjectileOrigin::Firefly,
                500,
            )
            .unwrap(),
            None
        );
        assert_eq!(full.economy.fireball_projectiles.len(), before);
        assert_eq!(full.dynamic_lighting.active_count(), lights);

        let slot = construct_creature_fireball(
            arena,
            terrain,
            &particles,
            context,
            shooter_slot,
            target_slot,
            FireballProjectileOrigin::Firefly,
            500,
        )
        .unwrap()
        .expect("ordinary Firefly constructor allocates");
        let shot = arena
            .economy
            .fireball_projectiles
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(shot.owner_entity_index, shooter.source_id);
        assert_eq!((shot.yaw, shot.pitch), (yaw, pitch));
        assert_eq!(shot.target_source_entity_index, target_slot);
        assert_eq!(
            (shot.target_class, shot.target_model),
            (target.class, target.model)
        );
        assert_eq!(shot.source_subspell_index, 500);
        assert_eq!(
            shot.position.z,
            shooter
                .position
                .z
                .wrapping_add(shooter.collision.unwrap().half_height)
        );
        assert!(arena.economy.map_chains.is_linked(slot));

        // A real replaceable 10/0 target is captured by the frozen fallback;
        // the NewEvent result occupies that same target address.
        let replaceable = crate::world::effects::allocate(
            arena,
            terrain,
            sim_definition::adapter::mc2_effect_definition(EffectKind::Impact),
            target.position,
            &particles,
        )
        .unwrap()
        .expect("real replaceable impact target");
        arena
            .economy
            .authored_class5
            .iter_mut()
            .find(|row| row.source_entity_index == shooter_slot)
            .unwrap()
            .word_0x96_150 = replaceable;
        arena.rebuild_source_free_arena();
        while arena.economy.free_slot_count() != 0 {
            arena.allocate_source_slot_with_terrain(terrain).unwrap();
        }
        loop {
            let mut probe = arena.clone();
            let mut probe_terrain = terrain.clone();
            let candidate = probe
                .allocate_source_slot_with_terrain(&mut probe_terrain)
                .expect("frozen cursor reaches the effect target");
            if candidate == replaceable {
                break;
            }
            assert_eq!(
                arena.allocate_source_slot_with_terrain(terrain),
                Some(candidate)
            );
        }
        let reused = construct_creature_fireball(
            arena,
            terrain,
            &particles,
            context,
            shooter_slot,
            replaceable,
            FireballProjectileOrigin::Firefly,
            500,
        )
        .unwrap()
        .expect("frozen fallback allocates the target address");
        assert_eq!(reused, replaceable);
        let shot = arena
            .economy
            .fireball_projectiles
            .iter()
            .find(|row| row.source_entity_index == reused)
            .unwrap();
        assert_eq!((shot.target_class, shot.target_model), (9, 0));
        assert_eq!(shot.target_source_entity_index, replaceable);
        let (yaw, pitch) =
            crate::level1_mission::projectile_guidance_angles(shooter.position, shooter.position);
        assert_eq!((shot.yaw, shot.pitch), (yaw, pitch));
    }

    #[test]
    fn water_fireball_hides_the_current_address_after_full_pool_birth() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2");
        let game = CampaignSession::open(package, CampaignSelection::LEVEL2)
            .expect("real Level2 owner package");
        let mut session = game.simulation().clone();
        let particles = session.session.definition.presentation.particles.clone();
        let live = &mut session.session.state.live;
        let arena = &mut live.economy;
        let terrain = &mut live.terrain;
        let mut position = live.core.state.position;
        assert_eq!(
            terrain.material_mask(position),
            1,
            "qualified starting water"
        );
        position.z = terrain.ground_height(position).wrapping_sub(1);
        let slot = construct(
            arena,
            terrain,
            &particles,
            Launch {
                product: Product::Creature27Fireball,
                position,
                owner: live.core.state.entity_index,
                yaw: 73,
                pitch: 31,
                // No active source class zero can intercept the water hit.
                target_class: 0,
                target_model: 0,
                target: 0,
                damage: 850,
                effect_life: 0,
                descriptor: None,
            },
        )
        .unwrap()
        .expect("real Fireball constructor allocates");
        let previous = arena.handle(slot).unwrap();
        let projectile = arena
            .economy
            .fireball_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        projectile.control_flags |= crate::level1_mission::SOURCE_REPLACEABLE_FLAG;
        projectile.actual_speed = 0;
        projectile.minimum_speed = 0;
        projectile.acquisition_attempted = true;
        arena.rebuild_source_free_arena();
        arena
            .economy
            .fireball_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap()
            .control_flags &= !crate::level1_mission::SOURCE_REPLACEABLE_FLAG;
        while arena.economy.free_slot_count() != 0 {
            arena.allocate_source_slot_with_terrain(terrain).unwrap();
        }
        loop {
            let mut probe = arena.clone();
            let mut probe_terrain = terrain.clone();
            let candidate = probe
                .allocate_source_slot_with_terrain(&mut probe_terrain)
                .expect("frozen cursor reaches the projectile address");
            if candidate == slot {
                break;
            }
            assert_eq!(
                arena.allocate_source_slot_with_terrain(terrain),
                Some(candidate)
            );
        }

        let mut exhausted = arena.clone();
        let mut exhausted_terrain = terrain.clone();
        exhausted.finish_source_group();
        let failed = crate::native_world::effects::step_projectile(
            &mut exhausted,
            &mut exhausted_terrain,
            &particles,
            slot,
            source_target::Context::default(),
            &[],
            0,
            None,
        )
        .expect("water callback tolerates a full source arena");
        assert_eq!(failed.impact_slot, None);
        assert!(!failed.live);
        assert!(
            exhausted.economy.fireball_projectiles.iter().any(|row| {
                row.source_entity_index == slot && !row.active && row.drawing_disabled
            })
        );
        assert!(!exhausted.economy.free_source_slots().contains(&slot));

        let step = crate::native_world::effects::step_projectile(
            arena,
            terrain,
            &particles,
            slot,
            source_target::Context::default(),
            &[],
            0,
            None,
        )
        .expect("water callback uses the real frozen source allocator");
        assert_eq!(step.impact_slot, Some(slot));
        assert!(!step.live);
        assert!(arena.resolve(previous).is_none());
        let splash = &arena.entities[usize::from(slot)];
        assert_eq!((splash.class, splash.model, splash.action), (10, 5, 5));
        assert_eq!(splash.position, Position::default());
        assert_eq!(splash.source_id, slot);
        assert!(splash.drawing_disabled());
        assert!(arena.economy.map_chains.is_linked(slot));
        assert_eq!(terrain.planes().entity_indices[0], slot as i16);
    }

    #[test]
    fn fireball_impact_reads_the_reused_source_address_after_allocation() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level4");
        let game = CampaignSession::open(package, CampaignSelection::ordinary(4).unwrap())
            .expect("real Cave4 owner package");
        let mut session = game.simulation().clone();
        let particles = session.session.definition.presentation.particles.clone();
        let live = &mut session.session.state.live;
        let arena = &mut live.economy;
        let terrain = &mut live.terrain;
        let position = Position {
            x: 0x2000,
            y: 0x3000,
            z: 10_000,
        };
        arena.dynamic_lighting_enabled = true;
        let slot = construct(
            arena,
            terrain,
            &particles,
            Launch {
                product: Product::Creature27Fireball,
                position,
                owner: live.core.state.entity_index,
                yaw: 73,
                pitch: 31,
                target_class: u8::MAX,
                target_model: u8::MAX,
                target: 0,
                damage: 850,
                effect_life: 0,
                descriptor: None,
            },
        )
        .unwrap()
        .expect("real Fireball constructor allocates");
        assert_eq!(
            arena
                .dynamic_lighting
                .active_slots()
                .filter(|(_, light)| light.linked && light.source_entity_index == slot)
                .count(),
            1
        );
        let previous = arena.handle(slot).unwrap();
        let projectile = arena
            .economy
            .fireball_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        projectile.control_flags |= crate::level1_mission::SOURCE_REPLACEABLE_FLAG;
        projectile.life = -1;
        projectile.actual_speed = 0;
        projectile.minimum_speed = 0;
        projectile.acquisition_attempted = true;
        arena.rebuild_source_free_arena();
        // The frozen candidate remains after its current occupant changes
        // flags, as in a later allocation within the same source pass.
        arena
            .economy
            .fireball_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap()
            .control_flags &= !crate::level1_mission::SOURCE_REPLACEABLE_FLAG;
        while arena.economy.free_slot_count() != 0 {
            arena
                .allocate_source_slot_with_terrain(terrain)
                .expect("consume a real free-stack slot");
        }
        loop {
            let mut probe = arena.clone();
            let mut probe_terrain = terrain.clone();
            let candidate = probe
                .allocate_source_slot_with_terrain(&mut probe_terrain)
                .expect("frozen cursor reaches the projectile address");
            if candidate == slot {
                break;
            }
            assert_eq!(
                arena.allocate_source_slot_with_terrain(terrain),
                Some(candidate)
            );
        }
        arena.dynamic_lighting_enabled = true;
        let step = crate::native_world::effects::step_projectile(
            arena,
            terrain,
            &particles,
            slot,
            source_target::Context::default(),
            &[],
            0,
            None,
        )
        .expect("the real terminal Fireball callback completes");
        assert_eq!(step.impact_slot, Some(slot));
        assert!(!step.live);
        assert!(arena.resolve(previous).is_none());
        let impact = &arena.entities[usize::from(slot)];
        assert_eq!((impact.class, impact.model, impact.action), (10, 0, 0));
        assert_eq!(impact.position, Position::default());
        assert_eq!(impact.source_id, slot);
        assert_eq!((impact.yaw, impact.pitch), (0, 0));
        assert!(impact.drawing_disabled());
        assert!(arena.economy.map_chains.is_linked(slot));
        assert_eq!(terrain.planes().entity_indices[0], slot as i16);
        assert_eq!(
            arena
                .dynamic_lighting
                .active_slots()
                .filter(|(_, light)| light.linked && light.source_entity_index == slot)
                .count(),
            2,
            "NewEvent preserves the old fixed-address light and admits the impact light"
        );
        assert!(step.direct_hit_owner.is_none());
        let mut lighting = std::mem::take(&mut arena.dynamic_lighting);
        crate::world::effects::lighting::after_updates(
            &mut lighting,
            arena,
            terrain,
            source_target::Context::default(),
        )
        .expect("ordinary lighting service reads the current hidden impact");
        assert!(
            !lighting
                .active_slots()
                .any(|(_, light)| { light.linked && light.source_entity_index == slot })
        );
        arena.dynamic_lighting = lighting;
    }

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
