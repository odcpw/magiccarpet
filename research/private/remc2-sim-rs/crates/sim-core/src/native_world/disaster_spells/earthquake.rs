//! Earthquake's ordinary cast projectile and its scheduled terrain effect.
//!
//! The shared guided-projectile owner supplies movement, collision and retry.
//! This module owns only family17's source constructor and impact materializer.

use super::super::{
    guided_projectile::{self, Guidance, StepOutcome},
};
#[cfg(test)]
use super::CastInput;
use super::{EntitySnapshot, ParticleParameter, SessionTerrain};
use crate::campaign::source_arena::SourceArena;
use crate::level_definition::{EARTHQUAKE_PROPERTIES, TerrainDisasterKind};
use crate::{FlightWorld, Position, move_entity};

const PROJECTILE_PARTICLE: u16 = 211;

pub(crate) const SOURCE_CONSTRUCTOR: guided_projectile::Constructor = guided_projectile::Constructor {
    model: 2, action: 2, parameter: 60, particle: PROJECTILE_PARTICLE,
    speed: EARTHQUAKE_PROPERTIES.projectile_speed,
    maximum_speed: EARTHQUAKE_PROPERTIES.maximum_launch_speed,
    life: EARTHQUAKE_PROPERTIES.projectile_range / EARTHQUAKE_PROPERTIES.projectile_speed as i32,
    mana: 50,
};

fn put(row: &mut EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn set_owner(row: &mut EntitySnapshot, owner: u16) {
    row.source_id = owner;
    put(row, 30, owner);
}

/// AddSpell17 followed by sub_6BCF0/sub_6DCA0(0x11). Allocation publishes a
/// normal 9/2 guided projectile whose impact is the existing 10/15 disaster.
/// Holder charging and countdown are owned by the common disaster holder.
pub(crate) fn launch(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    holder_slot: u16,
    parent_slot: u16,
    input: super::CastInput,
) -> Result<Option<u16>, String> {
    let (initial_position, inherited_speed) = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|_| parent_slot != 0)
        .map(|parent| (parent.position, parent.actual_speed))
        .ok_or("Earthquake parent address is outside the source arena")?;
    let Some(slot) = guided_projectile::construct(
        arena, terrain, particles, initial_position, Some(parent_slot), inherited_speed,
        SOURCE_CONSTRUCTOR,
    )? else {
        return Ok(None);
    };
    let row = &mut arena.entities[usize::from(slot)];
    row.record[71] = 10;
    row.record[72] = 15;

    // The source code keeps fixed pointers across NewEvent. Re-read both
    // addresses after publication so an allocator alias is observed there.
    let parent = arena
        .entities
        .get(usize::from(parent_slot))
        .ok_or("Earthquake parent address is outside the source arena")?
        .clone();
    let holder = arena
        .entities
        .get(usize::from(holder_slot))
        .ok_or("Earthquake holder address is outside the source arena")?
        .clone();
    let tier = usize::from(
        crate::spell_activation::level2_applied_tier(holder.level_gate)
            .ok_or("Earthquake holder has a negative source tier")?,
    );
    let rule = *EARTHQUAKE_PROPERTIES
        .tiers
        .get(tier)
        .ok_or("Earthquake holder tier is outside its source table")?;

    let mut launch_position = arena.entities[usize::from(slot)].position;
    let base_launch_position = launch_position;
    let hand = if parent.control_flags & 0x100 != 0 {
        Some(parent.yaw.wrapping_sub(512) & 2047)
    } else if parent.control_flags & 0x200 != 0 {
        Some(parent.yaw.wrapping_add(512) & 2047)
    } else {
        None
    };
    if let Some(yaw) = hand {
        move_entity(&mut launch_position, yaw, 0, 256);
        if terrain.ground_height(launch_position) > launch_position.z {
            launch_position = base_launch_position;
        }
    }
    launch_position.z = launch_position.z.wrapping_add(parent.array_fov());

    let yaw = input.aim_yaw.wrapping_add(parent.yaw);
    let pitch = input.aim_pitch.wrapping_add(parent.pitch);
    let mut aim = parent.position;
    // sub_6BCF0 points this terrain-family cast along the ground.
    move_entity(&mut aim, yaw, 0, EARTHQUAKE_PROPERTIES.aim_distance);
    aim.z = terrain.ground_height(aim);

    let projectile = &mut arena.entities[usize::from(slot)];
    projectile.set_position(launch_position);
    put(projectile, 42, holder_slot);
    set_owner(projectile, parent.source_id);
    projectile.set_mana(holder.mana);
    projectile.set_subspell_index(rule.damage);
    projectile.set_source_byte_46(rule.impact_parameter);
    projectile.set_source_dword_10(i32::from(input.auxiliary_byte));
    projectile.set_yaw(yaw);
    projectile.set_pitch(pitch);
    projectile.record[158..160].copy_from_slice(&aim.x.to_le_bytes());
    projectile.record[160..162].copy_from_slice(&aim.y.to_le_bytes());
    projectile.record[162..164].copy_from_slice(&(aim.z as u16).to_le_bytes());
    arena.economy.source_rng.insert(slot, projectile.entity_rng);
    crate::world::effects::sync_position(arena, terrain, slot, launch_position, true, false)?;
    Ok(Some(slot))
}

/// Source action2/sub_66160: one ordinary shared flyer step, then the wrapper
/// re-reads the fixed flyer address and assigns only the new child's life.
pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    targets: Option<&guided_projectile::Targets<'_>>,
    context: crate::native_world::source_target::Context<'_>,
) -> Result<StepOutcome, String> {
    arena
        .entities
        .get(usize::from(slot))
        .filter(|row| (row.class, row.model, row.action) == (9, 2, 2))
        .ok_or_else(|| format!("Earthquake action2 lost flyer slot {slot}"))?;
    let outcome = guided_projectile::step(
        arena,
        terrain,
        particles,
        slot,
        Guidance {
            yaw_turn: EARTHQUAKE_PROPERTIES.projectile_yaw_turn,
            pitch_turn: EARTHQUAKE_PROPERTIES.projectile_pitch_turn,
        },
        targets,
        context,
    )?;
    if let Some(child_slot) = outcome.impact_slot {
        // sub_66160 reads a1x->byte46 after sub_65820. NewEvent may have
        // reused `slot`, so preserve the current fixed-address alias result.
        let flyer_byte_46 = arena
            .entities
            .get(usize::from(slot))
            .ok_or("Earthquake flyer address is outside the source arena")?
            .record[74];
        let child = arena
            .entities
            .get_mut(usize::from(child_slot))
            .ok_or("Earthquake impact address is outside the source arena")?;
        child.set_life(i32::from(flyer_byte_46));
    }
    Ok(outcome)
}

/// sub_6DCA0's class10/model15 impact. Reuse the existing source disaster
/// constructor and carry the selected tier into its real terrain consumer.
pub(crate) fn construct_effect(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    projectile: &EntitySnapshot,
) -> Result<Option<u16>, String> {
    if (projectile.class, projectile.model, projectile.action) != (9, 2, 2)
        || (projectile.record[71], projectile.record[72]) != (10, 15)
    {
        return Err("Earthquake effect received an unrelated projectile".to_owned());
    }
    // sub_65820 constructs model15 independently of the projectile's
    // payload, then copies the current payload. sub_66160 sets its life.
    let definition =
        sim_definition::adapter::mc2_terrain_disaster_definition(TerrainDisasterKind::Earthquake);
    crate::world::terrain::disasters::mc2::runtime::construct_defined(
        arena,
        terrain,
        projectile.position,
        definition,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready_earthquake(tier: u8) -> (super::super::super::Mc2Simulation, u16, u16) {
        let mut simulation = super::super::super::tests::level_two_campaign_session();
        let live = &mut simulation.session.state.live;
        let parent_slot = live.core.state.entity_index;
        let holder_slot = live
            .shared_spells
            .pickups()
            .iter()
            .find(|row| row.model == 3)
            .expect("Level2 source holder fixture")
            .index;
        {
            let holder = live.shared_spells.pickup_mut(holder_slot).unwrap();
            holder.model = 17;
            holder.record[68] = 17;
            holder.set_source_action(51);
            holder.level_gate = tier as i8;
        }
        let holder = live.shared_spells.pickup(holder_slot).unwrap().clone();
        live.economy.entities[usize::from(holder_slot)] = holder;
        (simulation, holder_slot, parent_slot)
    }

    #[test]
    fn family17_cast_reaches_tiered_guided_impact_and_keeps_max_life() {
        for (tier, override_payload) in [
            (0_u8, None),
            (1, None),
            (2, None),
            (1, Some((37_u8, 777_u16))),
        ] {
            let (mut simulation, holder_slot, parent_slot) = ready_earthquake(tier);
            let particles = simulation.session.definition.presentation.particles.clone();
            let rule = EARTHQUAKE_PROPERTIES.tiers[usize::from(tier)];
            let live = &mut simulation.session.state.live;
            let projectile_slot = launch(
                &mut live.economy,
                &mut live.terrain,
                &particles,
                holder_slot,
                parent_slot,
                super::CastInput::default(),
            )
            .unwrap()
            .expect("source projectile allocation");
            let projectile = live.economy.entities[usize::from(projectile_slot)].clone();
            assert_eq!(
                (projectile.class, projectile.model, projectile.action),
                (9, 2, 2)
            );
            assert_eq!(
                (
                    projectile.record[71],
                    projectile.record[72],
                    projectile.subspell_index,
                    projectile.record[74],
                ),
                (10, 15, rule.damage, rule.impact_parameter)
            );
            assert_eq!(
                (
                    projectile.particle_parameter_index,
                    projectile.parameter_relation,
                    u16::from_le_bytes([projectile.record[42], projectile.record[43]]),
                    u32::from_le_bytes(projectile.record[164..168].try_into().unwrap()),
                ),
                (Some(211), Some(60), holder_slot, 61)
            );
            if tier == 0 {
                let mut bare_arena = live.economy.clone();
                let mut bare_terrain = live.terrain.clone();
                let bare_slot = construct_effect(&mut bare_arena, &mut bare_terrain, &projectile)
                    .unwrap()
                    .expect("source constructor allocates the bare Earthquake");
                let bare = &bare_arena.entities[usize::from(bare_slot)];
                assert_eq!(
                    (
                        bare.source_id,
                        bare.subspell_index,
                        bare.life,
                        bare.max_life
                    ),
                    (bare_slot, 100, 128, 128),
                    "guided impact copies the flyer only after this constructor"
                );
                let mut unrelated = projectile.clone();
                unrelated.record[72] = 11;
                assert!(
                    construct_effect(&mut live.economy, &mut live.terrain, &unrelated).is_err()
                );
            }

            // Force the ordinary ground-impact path on a real dry terrain
            // cell. The shared flyer correctly emits water splash and hides
            // without an impact child when material_mask is 1.
            let mut impact_position = (0..65536_u32)
                .map(|tile| Position {
                    x: ((tile & 255) << 8) as u16,
                    y: ((tile >> 8) << 8) as u16,
                    z: 0,
                })
                .find(|position| {
                    let x = usize::from(position.x >> 8);
                    let y = usize::from(position.y >> 8);
                    (2..=253).contains(&x)
                        && (2..=253).contains(&y)
                        && live.terrain.material_mask(*position) != 1
                        && (-2_isize..=2).all(|dx| {
                            (-2_isize..=2).all(|dy| {
                                let index =
                                    (x as isize + dx) as usize + 256 * (y as isize + dy) as usize;
                                live.terrain.planes().angles[index] & 15 != 0
                                    && live.terrain.planes().ground_heights[index] > 4
                            })
                        })
                })
                .expect("qualified Level 2 terrain has elevated dry ground for the ring");
            impact_position.z = live.terrain.ground_height(impact_position).wrapping_sub(1);
            let impact_probe_position = {
                let flyer = &mut live.economy.entities[usize::from(projectile_slot)];
                flyer.set_position(impact_position);
                flyer.set_actual_speed(0);
                flyer.record[136..138].copy_from_slice(&0_i16.to_le_bytes());
                flyer.set_yaw(0);
                flyer.set_pitch(0);
                if let Some((lifetime, damage)) = override_payload {
                    flyer.set_source_byte_46(lifetime);
                    flyer.set_subspell_index(damage);
                }
                flyer.position
            };
            crate::world::effects::sync_position(
                &mut live.economy,
                &mut live.terrain,
                projectile_slot,
                impact_probe_position,
                true,
                false,
            )
            .unwrap();
            let outcome = step_projectile(
                &mut live.economy,
                &mut live.terrain,
                &particles,
                projectile_slot,
                None,
                crate::native_world::source_target::Context::from_ordinary(
                    &live.goats,
                    &live.ordinary,
                ),
            )
            .unwrap();
            let effect_slot = outcome
                .impact_slot
                .expect("source guided impact allocation");
            let effect = &live.economy.entities[usize::from(effect_slot)];
            let (expected_lifetime, expected_damage) =
                override_payload.unwrap_or((rule.impact_parameter, rule.damage));
            assert_eq!(
                (
                    effect.class,
                    effect.model,
                    effect.action,
                    effect.source_id,
                    effect.max_life,
                    effect.life,
                    effect.subspell_index,
                    effect.record[74],
                ),
                (
                    10,
                    15,
                    15,
                    projectile.source_id,
                    128,
                    i32::from(expected_lifetime),
                    expected_damage,
                    expected_lifetime,
                )
            );

            if tier == 0 {
                // Follow the real impact controller through the scheduler:
                // model15 moves and allocates model11, which lowers the same
                // ground queried by flight. This is controlled impact
                // placement on retained owner terrain, not a playthrough.
                let baseline = live.terrain.clone();
                let existing_rings = live
                    .economy
                    .entities
                    .iter()
                    .filter(|row| row.active && (row.class, row.model) == (10, 11))
                    .map(|row| (row.index, live.economy.economy.slot_generation(row.index)))
                    .collect::<std::collections::BTreeSet<_>>();
                simulation.step_outer_turn(crate::PlayerCommand::default(), 0);
                assert!(
                    !simulation.dispatch_events().iter().any(|event| matches!(
                        event,
                        crate::native_world::Mc2DispatchEvent::Error { .. }
                    )),
                    "scheduled Earthquake must not roll back: {:?}",
                    simulation.dispatch_events()
                );
                let live = &simulation.session.state.live;
                let ring = live
                    .economy
                    .entities
                    .iter()
                    .find(|row| {
                        row.active
                            && (row.class, row.model, row.action) == (10, 11, 11)
                            && row.source_id == projectile.source_id
                            && u16::from_le_bytes([row.record[42], row.record[43]]) == 15
                            && !existing_rings.contains(&(
                                row.index,
                                live.economy.economy.slot_generation(row.index),
                            ))
                    })
                    .expect("scheduled Earthquake creates its real lowering ring");
                let center = ring.position;
                let mut changed = None;
                for _ in 0..4 {
                    let live = &simulation.session.state.live;
                    let x = i32::from(center.x >> 8);
                    let y = i32::from(center.y >> 8);
                    changed = (-3_i32..=3)
                        .flat_map(|dx| (-3_i32..=3).map(move |dy| (dx, dy)))
                        .find_map(|(dx, dy)| {
                            let point = Position {
                                x: ((x + dx) as u8 as u16) << 8,
                                y: ((y + dy) as u8 as u16) << 8,
                                z: 0,
                            };
                            let before = baseline.ground_height(point);
                            let after = live.terrain.ground_height(point);
                            (after != before).then_some((point, before, after))
                        });
                    if changed.is_some() {
                        break;
                    }
                    simulation.step_outer_turn(crate::PlayerCommand::default(), 0);
                    assert!(
                        !simulation.dispatch_events().iter().any(|event| matches!(
                            event,
                            crate::native_world::Mc2DispatchEvent::Error { .. }
                        )),
                        "scheduled Earthquake ring must not roll back: {:?}",
                        simulation.dispatch_events()
                    );
                }
                assert!(
                    changed.is_some(),
                    "lowering ring changes live flight ground height"
                );
            }
        }
    }

    #[test]
    fn missing_particle211_rejects_cast_before_source_allocation() {
        let (mut simulation, holder_slot, parent_slot) = ready_earthquake(0);
        let particles = simulation.session.definition.presentation.particles.clone();
        assert!(particles.len() > 211, "source fixture retains particle211");
        let live = &mut simulation.session.state.live;
        let before = live
            .economy
            .entities
            .iter()
            .filter(|row| row.active)
            .count();
        let error = launch(
            &mut live.economy,
            &mut live.terrain,
            &particles[..211],
            holder_slot,
            parent_slot,
            super::CastInput::default(),
        )
        .unwrap_err();
        assert!(error.contains("particle211"));
        assert_eq!(
            live.economy
                .entities
                .iter()
                .filter(|row| row.active)
                .count(),
            before,
            "missing source particle must fail before publishing a flyer"
        );
    }
}
