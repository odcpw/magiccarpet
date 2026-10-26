//! MC2 header views for retained effect carriers. Timers, RNG, descendants,
//! damage and terrain rules live in world::effects; this adapter only maps
//! the arena's existing typed storage to that one implementation.

use super::{EntitySnapshot, Level2DynamicArenaKind as Kind, ParticleParameter, SessionTerrain};
use crate::{Position, campaign::source_arena::SourceArena, world::effects as rules};

/// The flyer owns guidance/collision; its terminal constructor returns to
/// SourceArena so raw and typed replaceable rows arbitrate the same NewEvent.
pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
) -> Result<super::level2_dynamic_economy::Level2FireballStep, String> {
    use super::level2_dynamic_economy::FireballBirth;
    let mut step =
        arena
            .economy
            .advance_fireball_projectile_slot(slot, terrain, &arena.entities, true);
    let Some(birth) = step.pending_birth.take() else {
        return Ok(step);
    };
    let projectile = arena
        .economy
        .fireball_projectiles
        .iter()
        .find(|s| s.source_entity_index == slot)
        .copied()
        .ok_or("terminal flyer lost its current carrier")?;
    let child = if birth == FireballBirth::Impact
        && projectile.origin == super::FireballProjectileOrigin::Eruption
    {
        arena.allocate_source_slot_with_terrain(terrain).map(|id| {
            let mut row =
                arena
                    .economy
                    .initialize_authored_model17(id, u16::MAX, projectile.position);
            row.entity_id = projectile.owner_entity_index;
            *arena
                .economy
                .authored_model17
                .iter_mut()
                .find(|s| s.source_entity_index == id)
                .expect("new Meteor carrier") = row;
            arena.economy.source_owner.insert(id, row.entity_id);
            id
        })
    } else {
        let kind = if birth == FireballBirth::Ash {
            crate::level_definition::EffectKind::Ash
        } else if projectile.source_model == 28 {
            crate::level_definition::EffectKind::FireSpheres
        } else {
            crate::level_definition::EffectKind::Impact
        };
        let child = rules::allocate(
            arena,
            terrain,
            sim_definition::adapter::mc2_effect_definition(kind),
            projectile.position,
            particles,
        )?;
        if let Some(id) = child
            && kind == crate::level_definition::EffectKind::FireSpheres
        {
            // sub_65B50 specializes the successful sub_65C20 result. The
            // constructor's 80-tick children keep their own life; only the
            // root receives the 30-tick lifetime and projectile payload.
            let root = arena
                .economy
                .effect_sphere_roots_mut()
                .iter_mut()
                .find(|row| row.source_entity_index == id)
                .expect("Fire Spheres constructor retains its root");
            root.entity_id = projectile.owner_entity_index;
            root.subspell_index = projectile.source_subspell_index;
            root.yaw = projectile.yaw;
            root.pitch = projectile.pitch;
            root.max_life = 30;
            root.life = 30;
            root.target_entity_index = if step.direct_hit_owner.is_some() {
                projectile.target_source_entity_index
            } else {
                0
            };
            arena
                .economy
                .source_owner
                .insert(id, projectile.owner_entity_index);
            for child in arena
                .economy
                .effect_sphere_children_mut()
                .iter_mut()
                .filter(|row| row.parent_entity_index == id)
            {
                child.entity_id = projectile.owner_entity_index;
            }
        } else if let Some(id) = child {
            let row = &mut arena.entities[usize::from(id)];
            row.source_id = projectile.owner_entity_index;
            row.record[30..32].copy_from_slice(&row.source_id.to_le_bytes());
            if birth == FireballBirth::Impact {
                row.set_subspell_index(projectile.source_subspell_index);
                row.set_yaw(projectile.yaw);
                row.set_pitch(projectile.pitch);
            }
        }
        child
    };
    if child.is_some() || birth == FireballBirth::Ash {
        if let Some(parent) = arena
            .economy
            .fireball_projectiles
            .iter_mut()
            .find(|s| s.source_entity_index == slot)
        {
            parent.active = false;
            parent.drawing_disabled = true;
        }
        step.live = false;
    }
    // sub_6D8B0 belongs to the successful actor-impact allocation branch,
    // not to terrain/lifetime termination or a failed NewEvent attempt.
    if child.is_none() {
        step.direct_hit_owner = None;
    }
    step.impact_slot = child;
    Ok(step)
}

/// MCR1 retains the source globals at the same reset boundary as the arena.
pub(crate) fn reset_state(runtime: &super::RuntimeSnapshot) -> rules::State {
    let (eruption, eruption_smoke) = runtime.effect_globals();
    rules::State {
        eruption,
        eruption_smoke,
    }
}

pub(crate) fn hidden_slots(arena: &SourceArena) -> Vec<(u16, Position)> {
    let mut hidden = Vec::new();
    macro_rules! collect {
        ($rows:expr) => {
            hidden.extend(
                $rows
                    .iter()
                    .filter(|s| s.active && s.control_flags & 0x400 != 0)
                    .map(|s| (s.source_entity_index, s.position)),
            );
        };
    }
    collect!(arena.economy.authored_model1);
    collect!(arena.economy.authored_model5_water_splashes);
    collect!(arena.economy.authored_model6);
    collect!(arena.economy.authored_model13);
    collect!(arena.economy.model13_spawned_children);
    collect!(arena.economy.model59_smoke_children);
    collect!(arena.economy.authored_model17);
    collect!(arena.economy.model17_spawned_children);
    collect!(arena.economy.authored_model76);
    collect!(arena.economy.model76_children);
    hidden.sort_unstable_by_key(|(slot, _)| *slot);
    hidden
}

pub(crate) fn sprite_sources(arena: &SourceArena) -> Vec<(u16, u16, u16, i16, Position)> {
    let mut rows = Vec::new();
    let visible = |active: bool, flags: u32| active && flags & 0x421 == 0;
    for s in arena
        .economy
        .authored_model1
        .iter()
        .filter(|s| visible(s.active, s.control_flags))
    {
        rows.push((
            s.source_entity_index,
            s.source_entity_index,
            s.particle_index,
            0,
            s.position,
        ));
    }
    for s in arena
        .economy
        .authored_model5_water_splashes
        .iter()
        .filter(|s| visible(s.active, s.control_flags))
    {
        rows.push((
            s.source_entity_index,
            s.entity_id,
            s.sprite_index,
            0,
            s.position,
        ));
    }
    for s in arena
        .economy
        .authored_model6
        .iter()
        .filter(|s| visible(s.active, s.control_flags))
    {
        rows.push((
            s.source_entity_index,
            s.entity_id,
            s.particle_index,
            0,
            s.position,
        ));
    }
    for s in arena
        .economy
        .authored_model13
        .iter()
        .filter(|s| visible(s.active, s.control_flags))
    {
        rows.push((
            s.source_entity_index,
            s.entity_id,
            s.particle_index,
            0,
            s.position,
        ));
    }
    for s in arena
        .economy
        .model13_spawned_children
        .iter()
        .filter(|s| visible(s.active, s.control_flags))
    {
        rows.push((
            s.source_entity_index,
            s.parent_entity_id,
            s.particle_index,
            s.yaw,
            s.position,
        ));
    }
    for s in arena
        .economy
        .model59_smoke_children
        .iter()
        .filter(|s| visible(s.active, s.control_flags))
    {
        rows.push((
            s.source_entity_index,
            s.source_entity_index,
            s.particle_index,
            s.yaw,
            s.position,
        ));
    }
    for s in arena
        .economy
        .model76_children
        .iter()
        .filter(|s| visible(s.active, s.control_flags))
    {
        rows.push((s.source_entity_index, s.entity_id, 340, s.yaw, s.position));
    }
    rows
}

pub(crate) fn animation_frame(arena: &SourceArena, slot: u16, particle: u16) -> u8 {
    if let Some(s) = arena
        .economy
        .model17_spawned_children
        .iter()
        .find(|s| s.active && s.source_entity_index == slot)
    {
        return s.animation_frame;
    }
    if let Some(s) = arena
        .economy
        .authored_model5_water_splashes
        .iter()
        .find(|s| s.active && s.source_entity_index == slot)
    {
        return s.animation_frame;
    }
    if arena
        .economy
        .live_arena_slots()
        .iter()
        .any(|(id, _)| *id == slot)
    {
        return 0;
    }
    arena
        .entities
        .get(usize::from(slot))
        .filter(|s| s.active && s.particle_parameter_index == Some(particle))
        .map_or(0, |s| s.record[96])
}

pub(crate) fn renderable_source(row: &EntitySnapshot) -> bool {
    row.class != 10 || rules::source_kind(row.model).is_none() || row.control_flags & 4 != 0
}

fn header(
    arena: &SourceArena,
    slot: u16,
    kind: Kind,
    particles: &[ParticleParameter],
) -> Result<EntitySnapshot, String> {
    let (owner, rng, model, action, max_life, life, payload, flags, phase, position, base_particle) =
        match kind {
            Kind::AuthoredModel17 => {
                let s = arena
                    .economy
                    .authored_model17
                    .iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("Meteor carrier is absent")?;
                (
                    s.entity_id,
                    s.entity_rng,
                    17,
                    17,
                    s.max_life,
                    s.life,
                    s.subspell_index,
                    s.control_flags,
                    s.phase,
                    s.position,
                    0,
                )
            }
            Kind::Model17SpawnedChild => {
                let s = arena
                    .economy
                    .model17_spawned_children
                    .iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("impact carrier is absent")?;
                (
                    s.parent_entity_id,
                    s.entity_rng,
                    s.model,
                    s.action,
                    s.max_life,
                    s.life,
                    s.subspell_index,
                    s.control_flags,
                    s.phase,
                    s.position,
                    7,
                )
            }
            Kind::AuthoredModel1 => {
                let s = arena
                    .economy
                    .authored_model1
                    .iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("radial explosion carrier is absent")?;
                (
                    slot,
                    s.entity_rng,
                    s.model,
                    s.action,
                    s.max_life,
                    s.life,
                    s.subspell_index,
                    s.control_flags,
                    s.phase,
                    s.position,
                    41,
                )
            }
            Kind::AuthoredModel5WaterSplash => {
                let s = arena
                    .economy
                    .authored_model5_water_splashes
                    .iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("ash carrier is absent")?;
                (
                    s.entity_id,
                    s.entity_rng,
                    s.model,
                    s.action,
                    s.max_life,
                    s.life,
                    s.subspell_index,
                    s.control_flags,
                    s.phase,
                    s.position,
                    244,
                )
            }
            Kind::AuthoredModel6 => {
                let s = arena
                    .economy
                    .authored_model6
                    .iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("ground fire carrier is absent")?;
                (
                    s.entity_id,
                    s.entity_rng,
                    s.model,
                    s.action,
                    s.max_life,
                    s.life,
                    s.subspell_index,
                    s.control_flags,
                    s.phase,
                    s.position,
                    228,
                )
            }
            Kind::AuthoredModel13 => {
                let s = arena
                    .economy
                    .authored_model13
                    .iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("eruption smoke carrier is absent")?;
                (
                    s.entity_id,
                    s.entity_rng,
                    s.model,
                    s.action,
                    s.max_life,
                    s.life,
                    s.subspell_index,
                    s.control_flags,
                    s.phase,
                    s.position,
                    228,
                )
            }
            Kind::Model13SpawnedChild => {
                let s = arena
                    .economy
                    .model13_spawned_children
                    .iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("dark smoke carrier is absent")?;
                (
                    s.parent_entity_id,
                    s.entity_rng,
                    s.model,
                    s.action,
                    s.max_life,
                    s.life,
                    100,
                    s.control_flags,
                    s.phase,
                    s.position,
                    9,
                )
            }
            Kind::Model59SmokeChild => {
                let s = arena
                    .economy
                    .model59_smoke_children
                    .iter()
                    .find(|s| s.active && s.source_entity_index == slot)
                    .ok_or("smoke carrier is absent")?;
                (
                    slot,
                    s.entity_rng,
                    s.model,
                    s.action,
                    s.max_life,
                    s.life,
                    100,
                    s.control_flags,
                    s.phase,
                    s.position,
                    67,
                )
            }
            _ => return Err(format!("{kind:?} is not a retained shared effect carrier")),
        };
    let mut row = super::cave_features::new_event(slot, rng, position);
    row.source_id = owner;
    row.record[30..32].copy_from_slice(&owner.to_le_bytes());
    row.class = 10;
    row.record[67] = 10;
    row.model = model;
    row.record[68] = model;
    row.set_source_action(action);
    row.set_max_life(i32::from(max_life));
    row.set_life(i32::from(life));
    row.set_subspell_index(payload);
    row.set_control_flags(
        flags
            | if arena.economy.map_chains.is_linked(slot) {
                4
            } else {
                0
            },
    );
    row.set_phase(phase);
    if kind != Kind::AuthoredModel17 {
        rules::bind_particle(&mut row, base_particle, particles)?;
    }
    match kind {
        Kind::AuthoredModel17 => {
            let s = arena
                .economy
                .authored_model17
                .iter()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            row.set_source_dword_10(s.dword_0x10);
            row.set_yaw(s.yaw);
            row.set_source_overlap_half_extents(s.shift_rot_x, s.shift_rot_x);
            row.record[92..94].copy_from_slice(&s.shift_rot_y.to_le_bytes());
            row.record[69] = u8::MAX;
            row.record[70] = u8::MAX;
        }
        Kind::Model17SpawnedChild => {
            let s = arena
                .economy
                .model17_spawned_children
                .iter()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            row.set_source_dword_10(s.dword_0x10);
            row.set_source_word_2c(s.source_word_2c);
            row.set_yaw(s.yaw);
            row.set_pitch(s.pitch);
            row.set_source_word_96(s.target_source_entity_index);
            row.set_source_overlap_half_extents(s.shift_rot_x, s.shift_rot_x);
            row.record[92..94].copy_from_slice(&s.shift_rot_y.to_le_bytes());
            row.record[96] = s.animation_frame;
        }
        Kind::AuthoredModel5WaterSplash => {
            let s = arena
                .economy
                .authored_model5_water_splashes
                .iter()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            row.set_source_dword_10(s.dword_0x10);
            row.record[96] = s.animation_frame;
        }
        Kind::AuthoredModel6 => {
            let s = arena
                .economy
                .authored_model6
                .iter()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            row.set_source_dword_10(s.dword_0x10);
            row.set_source_word_2c(s.height_offset);
            row.particle_parameter_index = Some(s.particle_index);
            row.record[94..96].copy_from_slice(&s.particle_index.to_le_bytes());
            row.set_source_overlap_half_extents(s.shift_rot_x as i16, s.shift_rot_x as i16);
            row.record[92..94].copy_from_slice(&s.shift_rot_y.to_le_bytes());
        }
        Kind::AuthoredModel13 => {
            let s = arena
                .economy
                .authored_model13
                .iter()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            row.set_source_dword_10(s.dword_0x10);
            row.set_source_overlap_half_extents(s.shift_rot_x, s.shift_rot_x);
            row.record[92..94].copy_from_slice(&s.shift_rot_y.to_le_bytes());
        }
        Kind::Model13SpawnedChild => {
            let s = arena
                .economy
                .model13_spawned_children
                .iter()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            row.set_source_dword_10(s.dword_0x10);
            row.set_actual_speed(s.actual_speed);
            row.set_yaw(s.yaw);
            row.record[138..140].copy_from_slice(&s.max_speed.to_le_bytes());
            row.particle_parameter_index = Some(s.particle_index);
            row.record[94..96].copy_from_slice(&s.particle_index.to_le_bytes());
            row.record[69] = 10;
            row.record[70] = 14;
        }
        Kind::Model59SmokeChild => {
            let s = arena
                .economy
                .model59_smoke_children
                .iter()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            row.set_source_dword_10(s.dword_0x10);
            row.set_actual_speed(s.actual_speed);
            row.set_yaw(s.yaw);
            row.record[138..140].copy_from_slice(&s.max_speed.to_le_bytes());
            row.particle_parameter_index = Some(s.particle_index);
            row.record[94..96].copy_from_slice(&s.particle_index.to_le_bytes());
            row.record[69] = 10;
            row.record[70] = 13;
        }
        _ => {}
    }
    Ok(row)
}

fn store(arena: &mut SourceArena, kind: Kind, row: &EntitySnapshot) {
    let slot = row.index;
    macro_rules! common {
        ($s:ident) => {
            $s.entity_rng = row.entity_rng;
            $s.max_life = row.max_life as i16;
            $s.life = row.life as i16;
            $s.control_flags = row.control_flags;
            $s.position = row.position;
            $s.active = row.active;
        };
    }
    match kind {
        Kind::AuthoredModel17 => {
            let s = arena
                .economy
                .authored_model17
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            common!(s);
            s.dword_0x10 = row.source_dword_10();
            s.shift_rot_x = row.array_pitch();
            s.shift_rot_y = row.array_fov();
        }
        Kind::Model17SpawnedChild => {
            let s = arena
                .economy
                .model17_spawned_children
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            common!(s);
            s.dword_0x10 = row.source_dword_10();
            s.source_word_2c = row.source_word_2c();
            s.animation_frame = row.record[96];
        }
        Kind::AuthoredModel1 => {
            let s = arena
                .economy
                .authored_model1
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            common!(s);
            s.drawing_enabled = !row.drawing_disabled();
            if row.control_flags & 2 != 0 {
                s.last_sound_id = 3;
            }
        }
        Kind::AuthoredModel5WaterSplash => {
            let s = arena
                .economy
                .authored_model5_water_splashes
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            common!(s);
            s.drawing_enabled = !row.drawing_disabled();
            s.animation_frame = row.record[96];
            if row.control_flags & 2 != 0 {
                s.last_sound_id = 27;
            }
        }
        Kind::AuthoredModel6 => {
            let s = arena
                .economy
                .authored_model6
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            common!(s);
            s.dword_0x10 = row.source_dword_10();
            s.height_offset = row.source_word_2c();
            s.particle_index = row.particle_parameter_index.unwrap_or(228);
        }
        Kind::AuthoredModel13 => {
            let s = arena
                .economy
                .authored_model13
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            common!(s);
            s.dword_0x10 = row.source_dword_10();
            s.drawing_enabled = !row.drawing_disabled();
        }
        Kind::Model13SpawnedChild => {
            let s = arena
                .economy
                .model13_spawned_children
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            common!(s);
            s.dword_0x10 = row.source_dword_10();
            s.actual_speed = row.actual_speed;
            s.max_speed = i16::from_le_bytes(row.record[138..140].try_into().unwrap());
            s.particle_index = row.particle_parameter_index.unwrap_or(9);
            s.drawing_enabled = !row.drawing_disabled();
        }
        Kind::Model59SmokeChild => {
            let s = arena
                .economy
                .model59_smoke_children
                .iter_mut()
                .find(|s| s.source_entity_index == slot)
                .unwrap();
            common!(s);
            s.dword_0x10 = row.source_dword_10();
            s.actual_speed = row.actual_speed;
            s.max_speed = i16::from_le_bytes(row.record[138..140].try_into().unwrap());
            s.particle_index = row.particle_parameter_index.unwrap_or(67);
            s.drawing_enabled = !row.drawing_disabled();
        }
        _ => unreachable!("header admitted a retained effect kind"),
    }
}

pub(crate) fn visit<F>(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    kind: Kind,
    particles: &[ParticleParameter],
    state: &mut rules::State,
    apply: F,
) -> Result<rules::Visit, String>
where
    F: FnMut(&mut SourceArena, rules::DamagePulse) -> Result<u16, String>,
{
    let generation = arena.economy.slot_generation(slot);
    let row = header(arena, slot, kind, particles)?;
    let (row, visit) = rules::visit_entity(arena, terrain, row, particles, state, apply)?;
    if arena.economy.slot_generation(slot) != generation {
        // NewEvent replaced this typed parent during its own callback. The
        // source pointer now denotes the child; never restore the old carrier.
        rules::commit_entity(arena, terrain, row)?;
    } else {
        store(arena, kind, &row);
        arena.economy.source_rng.insert(slot, row.entity_rng);
        rules::sync_position(
            arena,
            terrain,
            slot,
            row.position,
            row.active && row.control_flags & 4 != 0,
            false,
        )?;
    }
    Ok(visit)
}
