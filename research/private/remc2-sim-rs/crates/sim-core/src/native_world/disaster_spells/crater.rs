//! Family-16 Crater projectile and its terrain-ring impact.
//!
//! The holder in `disaster_spells` owns charge/countdown. This leaf owns only
//! the source 9/5/5 flight callback and construction of its 10/11 child; the
//! child then runs through the ordinary terrain-disaster scheduler.

use super::{CastInput, EntitySnapshot, ParticleParameter, SessionTerrain};
use crate::{
    FlightWorld, Position,
    campaign::source_arena::SourceArena,
    level_definition::{CRATER_PROPERTIES as RULES, TerrainDisasterKind},
    move_entity,
    native_world::guided_projectile::{self, Guidance, StepOutcome},
};

fn put(row: &mut EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn set_owner(row: &mut EntitySnapshot, owner: u16) {
    row.source_id = owner;
    put(row, 30, owner);
}

/// sub_6BAB0 and the family-16 branch of sub_6DCA0.
pub(crate) fn launch(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    holder_slot: u16,
    parent_slot: u16,
    input: CastInput,
) -> Result<Option<u16>, String> {
    let (parent_position, inherited_speed) = {
        let parent = arena
            .entities
            .get(usize::from(parent_slot))
            .ok_or("Crater parent address is outside source arena")?;
        (parent.position, parent.actual_speed)
    };
    let holder_tier = arena
        .entities
        .get(usize::from(holder_slot))
        .filter(|row| row.class == 15 && row.model == 16)
        .ok_or("Crater holder is absent or has a different source family")?
        .level_gate;
    let tier = crate::spell_activation::level2_applied_tier(holder_tier)
        .ok_or("Crater holder has a negative source tier")?;
    let rule = RULES
        .tiers
        .get(usize::from(tier))
        .copied()
        .ok_or("Crater tier is outside its source table")?;
    let particle = *particles
        .get(211)
        .ok_or("Crater requires retained particle211")?;
    if particle.animation_limit().is_none() {
        return Err(format!(
            "Crater particle211 has unsupported render mode {}",
            particle.render_mode
        ));
    }
    let base_speed = RULES.projectile_speed;

    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        // The shared holder still charges and advances its countdown after
        // this source NewEvent failure.
        return Ok(None);
    };
    let position = if slot == parent_slot {
        Position::default()
    } else {
        parent_position
    };
    let mut row = crate::native_world::cave_features::new_event(
        slot,
        arena.economy.new_event_rng(slot),
        position,
    );
    row.class = 9;
    row.model = 5;
    row.record[67] = 9;
    row.record[68] = 5;
    row.set_source_action(5);
    row.set_control_flags((row.control_flags & !8) | 4);
    row.set_actual_speed(
        base_speed
            .wrapping_add(inherited_speed)
            .clamp(base_speed, RULES.maximum_launch_speed),
    );
    row.record[136..138].copy_from_slice(&base_speed.to_le_bytes());
    let max_life = RULES.projectile_range / i32::from(base_speed);
    row.set_max_life(max_life);
    row.set_life(max_life);
    row.parameter_relation = Some(60);
    row.record[164..168].copy_from_slice(&61_u32.to_le_bytes());
    crate::native_world::cave_features::bind_particle(&mut row, 211, particle)?;
    row.record[71] = 10;
    row.record[72] = 11;
    put(&mut row, 42, holder_slot);
    // sub_4D620's constructor value is overwritten from the live holder after
    // NewEvent by sub_6BAB0. This matters when allocation aliases that address.
    row.set_mana(50);
    row.set_subspell_index(rule.damage);
    row.set_source_byte_46(rule.impact_parameter);
    row.set_source_dword_10(i32::from(input.auxiliary_byte));

    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, true)?;

    // sub_6BAB0 reads these fixed addresses after NewEvent. Allocation can
    // replace either one, so reborrow their current occupants before applying
    // holder mana, owner, hand offset, orientation and target axis.
    let (
        parent_position,
        parent_source_id,
        parent_control_flags,
        parent_yaw,
        parent_pitch,
        parent_fov,
    ) = arena
        .entities
        .get(usize::from(parent_slot))
        .map(|parent| {
            (
                parent.position,
                parent.source_id,
                parent.control_flags,
                parent.yaw,
                parent.pitch,
                parent.array_fov(),
            )
        })
        .ok_or("Crater parent address disappeared after NewEvent")?;
    let holder_mana = arena
        .entities
        .get(usize::from(holder_slot))
        .ok_or("Crater holder address disappeared after NewEvent")?
        .mana;

    let mut launch_position = arena.entities[usize::from(slot)].position;
    let hand_yaw = if parent_control_flags & 0x100 != 0 {
        Some(parent_yaw.wrapping_sub(512) & 2047)
    } else if parent_control_flags & 0x200 != 0 {
        Some(parent_yaw.wrapping_add(512) & 2047)
    } else {
        None
    };
    if let Some(yaw) = hand_yaw {
        move_entity(&mut launch_position, yaw, 0, 256);
        if terrain.ground_height(launch_position) > launch_position.z {
            launch_position = arena.entities[usize::from(slot)].position;
        }
    }
    launch_position.z = launch_position.z.wrapping_add(parent_fov);

    let yaw = input.aim_yaw.wrapping_add(parent_yaw);
    let pitch = input.aim_pitch.wrapping_add(parent_pitch);
    let mut aim = parent_position;
    move_entity(&mut aim, yaw, 0, RULES.aim_distance);
    aim.z = terrain.ground_height(aim);

    let projectile = &mut arena.entities[usize::from(slot)];
    set_owner(projectile, parent_source_id);
    projectile.set_mana(holder_mana);
    projectile.set_position(launch_position);
    projectile.set_yaw(yaw);
    projectile.set_pitch(pitch);
    projectile.record[158..160].copy_from_slice(&aim.x.to_le_bytes());
    projectile.record[160..162].copy_from_slice(&aim.y.to_le_bytes());
    projectile.record[162..164].copy_from_slice(&aim.z.to_le_bytes());
    crate::world::effects::sync_position(arena, terrain, slot, launch_position, true, false)?;
    Ok(Some(slot))
}

/// sub_66280: run shared sub_65820, then apply Crater's child-life override.
/// Phase advancement and following-pass reclaim remain with the scheduler.
pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    targets: Option<&guided_projectile::Targets<'_>>,
    context: crate::native_world::source_target::Context<'_>,
) -> Result<StepOutcome, String> {
    let outcome = guided_projectile::step(
        arena,
        terrain,
        particles,
        slot,
        Guidance {
            yaw_turn: RULES.projectile_yaw_turn,
            pitch_turn: RULES.projectile_pitch_turn,
        },
        targets,
        context,
    )?;
    if let Some(impact_slot) = outcome.impact_slot {
        // sub_66280 reads byte46 after sub_65820 returns. The fixed source
        // address may now carry payload written during impact construction.
        let impact_life = arena
            .entities
            .get(usize::from(slot))
            .ok_or_else(|| format!("Crater projectile address left the arena at slot {slot}"))?
            .record[74];
        let impact = arena
            .entities
            .get_mut(usize::from(impact_slot))
            .ok_or("Crater impact address is outside source arena")?;
        // sub_66280 writes life only; model11 keeps its constructor maxLife40.
        impact.set_life(i32::from(impact_life));
    }
    Ok(outcome)
}

/// Family-16 child selector (10,11): construct the source-default LoweringRing
/// through the existing callback/kernel; the shared impact path copies the
/// projectile payload only after this bare constructor has returned.
pub(crate) fn construct_effect(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    projectile: &EntitySnapshot,
) -> Result<Option<u16>, String> {
    if (projectile.class, projectile.model, projectile.action) != (9, 5, 5)
        || (projectile.record[71], projectile.record[72]) != (10, 11)
    {
        return Err("Crater impact constructor received a different source selector".into());
    }
    let definition =
        sim_definition::adapter::mc2_terrain_disaster_definition(TerrainDisasterKind::LoweringRing);
    let Some(slot) = crate::world::terrain::disasters::mc2::runtime::construct_defined(
        arena,
        terrain,
        projectile.position,
        definition,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(slot))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guided_crater_impact_uses_current_byte46_and_preserves_mutated_payload() {
        let mut session = super::super::super::tests::level_two_campaign_session();
        let particles = session.session.definition.presentation.particles.clone();
        let live = &mut session.session.state.live;
        let parent = live.core.state.entity_index;
        let holder = live
            .shared_spells
            .pickups()
            .iter()
            .find(|row| row.model == 3)
            .expect("Level2 source holder fixture")
            .index;
        let mut row = live
            .shared_spells
            .pickup(holder)
            .expect("Level2 source holder row")
            .clone();
        row.model = 16;
        row.record[68] = 16;
        row.set_source_action(48);
        row.apply_nyphur_spell_profile(
            crate::spell_activation::level2_set_spell_profile(16, 0)
                .expect("Crater level-zero profile"),
            0,
        );
        live.economy.entities[usize::from(holder)] = row;

        let flyer = launch(
            &mut live.economy,
            &mut live.terrain,
            &particles,
            holder,
            parent,
            CastInput::default(),
        )
        .unwrap()
        .expect("source Crater projectile allocation");
        let source = &live.economy.entities[usize::from(flyer)];
        assert_eq!((source.class, source.model, source.action), (9, 5, 5));
        assert_eq!(
            (source.particle_parameter_index, source.parameter_relation),
            (Some(211), Some(60))
        );
        assert_eq!(
            u32::from_le_bytes(source.record[164..168].try_into().unwrap()),
            61
        );
        let real_owner = source.source_id;
        let mut scheduled = session.clone();
        let live = &mut session.session.state.live;

        let mut position = (0..65536_u32)
            .map(|tile| Position {
                x: ((tile & 255) << 8) as u16,
                y: ((tile >> 8) << 8) as u16,
                z: 0,
            })
            .find(|position| live.terrain.material_mask(*position) != 1)
            .expect("Level2 fixture has a dry terrain cell");
        assert_ne!(live.terrain.material_mask(position), 1);
        position.z = live.terrain.ground_height(position).wrapping_sub(1);

        const MUTATED_OWNER: u16 = 0x4567;
        const MUTATED_DAMAGE: u16 = 4321;
        const MUTATED_BYTE46: u8 = 61;
        let source = &mut live.economy.entities[usize::from(flyer)];
        source.set_position(position);
        source.set_actual_speed(0);
        source.record[136..138].copy_from_slice(&0_i16.to_le_bytes());
        source.source_id = MUTATED_OWNER;
        source.record[30..32].copy_from_slice(&MUTATED_OWNER.to_le_bytes());
        source.set_subspell_index(MUTATED_DAMAGE);
        source.set_source_byte_46(MUTATED_BYTE46);
        crate::world::effects::sync_position(
            &mut live.economy,
            &mut live.terrain,
            flyer,
            position,
            true,
            false,
        )
        .unwrap();

        let outcome = step_projectile(
            &mut live.economy,
            &mut live.terrain,
            &particles,
            flyer,
            None,
            crate::native_world::source_target::Context::from_ordinary(&live.goats, &live.ordinary),
        )
        .unwrap();
        let child_slot = outcome
            .impact_slot
            .expect("dry terrain collision constructs Crater");
        let current_flyer_byte46 = live.economy.entities[usize::from(flyer)].record[74];
        let child = &live.economy.entities[usize::from(child_slot)];
        assert_eq!((child.class, child.model, child.action), (10, 11, 11));
        assert_eq!(child.life, i32::from(current_flyer_byte46));
        assert_eq!(child.max_life, 40, "bare LoweringRing maxLife is retained");
        assert_eq!(child.record[74], current_flyer_byte46);
        assert_eq!(child.subspell_index, MUTATED_DAMAGE);
        assert_eq!(child.source_id, MUTATED_OWNER);

        // The same flyer on retained water follows the source splash path,
        // without producing a terrain-lowering ring.
        let mut water_case = scheduled.clone();
        let water = &mut water_case.session.state.live;
        let mut water_point = (0..65536_u32)
            .map(|tile| Position {
                x: ((tile & 255) << 8) as u16,
                y: ((tile >> 8) << 8) as u16,
                z: 0,
            })
            .find(|point| water.terrain.material_mask(*point) == 1)
            .expect("retained Level2 terrain has water");
        water_point.z = water.terrain.ground_height(water_point).wrapping_sub(1);
        let source = &mut water.economy.entities[usize::from(flyer)];
        source.set_position(water_point);
        source.set_actual_speed(0);
        source.record[136..138].copy_from_slice(&0_i16.to_le_bytes());
        crate::world::effects::sync_position(
            &mut water.economy,
            &mut water.terrain,
            flyer,
            water_point,
            true,
            false,
        )
        .unwrap();
        let water_impact = step_projectile(
            &mut water.economy,
            &mut water.terrain,
            &particles,
            flyer,
            None,
            crate::native_world::source_target::Context::from_ordinary(
                &water.goats,
                &water.ordinary,
            ),
        )
        .unwrap();
        assert!(water_impact.impact_slot.is_none());
        assert!(water.economy.entities[usize::from(flyer)].drawing_disabled());

        // Keep the source-owner branch separate from the payload mutation
        // control above, then follow the real impact into its scheduled
        // model11 callback and the flight world's current ground height.
        let live = &mut scheduled.session.state.live;
        let mut point = (0..65536_u32)
            .map(|tile| Position {
                x: ((tile & 255) << 8) as u16,
                y: ((tile >> 8) << 8) as u16,
                z: 0,
            })
            .find(|point| {
                let x = usize::from(point.x >> 8);
                let y = usize::from(point.y >> 8);
                (2..=253).contains(&x)
                    && (2..=253).contains(&y)
                    && live.terrain.material_mask(*point) != 1
                    && (-2_isize..=2).all(|dx| {
                        (-2_isize..=2).all(|dy| {
                            let index =
                                (x as isize + dx) as usize + 256 * (y as isize + dy) as usize;
                            live.terrain.planes().angles[index] & 15 != 0
                                && live.terrain.planes().ground_heights[index] > 4
                        })
                    })
            })
            .expect("retained Level2 terrain has elevated dry ground");
        point.z = live.terrain.ground_height(point).wrapping_sub(1);
        let source = &mut live.economy.entities[usize::from(flyer)];
        source.set_position(point);
        source.set_actual_speed(0);
        source.record[136..138].copy_from_slice(&0_i16.to_le_bytes());
        crate::world::effects::sync_position(
            &mut live.economy,
            &mut live.terrain,
            flyer,
            point,
            true,
            false,
        )
        .unwrap();
        let outcome = step_projectile(
            &mut live.economy,
            &mut live.terrain,
            &particles,
            flyer,
            None,
            crate::native_world::source_target::Context::from_ordinary(&live.goats, &live.ordinary),
        )
        .unwrap();
        let ring_slot = outcome
            .impact_slot
            .expect("dry Crater impact creates its ring");
        let ring = &live.economy.entities[usize::from(ring_slot)];
        assert_eq!(
            (ring.class, ring.model, ring.action, ring.source_id),
            (10, 11, 11, real_owner)
        );
        let handle = live.economy.handle(ring_slot).unwrap();
        let center = ring.position;
        let phase = ring.phase;
        let baseline = live.terrain.clone();
        let mut lowered = false;
        // The source ring advances its radius once every three phases.
        for visit in 0..3 {
            scheduled.step_outer_turn(crate::PlayerCommand::default(), 0);
            assert!(
                !scheduled.dispatch_events().iter().any(|event| matches!(
                    event,
                    crate::native_world::Mc2DispatchEvent::Error { .. }
                )),
                "scheduled Crater ring must not roll back: {:?}",
                scheduled.dispatch_events()
            );
            let live = &scheduled.session.state.live;
            let ring = &live.economy.entities[usize::from(ring_slot)];
            if visit == 0 {
                assert_eq!(
                    live.economy.economy.slot_generation(ring_slot),
                    handle.generation
                );
                assert_eq!(ring.phase, phase.wrapping_add(1));
            }
            let x = i32::from(center.x >> 8);
            let y = i32::from(center.y >> 8);
            lowered |= (-3_i32..=3)
                .flat_map(|dx| (-3_i32..=3).map(move |dy| (dx, dy)))
                .any(|(dx, dy)| {
                    let point = Position {
                        x: ((x + dx) as u8 as u16) << 8,
                        y: ((y + dy) as u8 as u16) << 8,
                        z: 0,
                    };
                    live.terrain.ground_height(point) < baseline.ground_height(point)
                });
            if lowered {
                break;
            }
        }
        assert!(lowered, "Crater ring lowers the live flight ground");
    }
}
