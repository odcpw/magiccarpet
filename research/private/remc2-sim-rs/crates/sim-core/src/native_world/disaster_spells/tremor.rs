//! Ordinary physical family15 adapter for the shared guided flyer and the
//! scheduled model71 terrain controller.

use super::{CastInput, ParticleParameter, SessionTerrain};
use crate::{
    FlightWorld, Position, campaign::source_arena::SourceArena, move_entity,
    native_world::EntitySnapshot, world::terrain::disasters::mc2::tremor as terrain_tremor,
};

const RULES: sim_definition::level_definition::DisasterSpellProperties =
    sim_definition::level_definition::TREMOR_PROPERTIES;

fn tier_rule(tier: u8) -> Result<sim_definition::level_definition::DisasterSpellTier, String> {
    RULES
        .tiers
        .get(usize::from(tier))
        .copied()
        .ok_or_else(|| format!("Tremor tier {tier} is outside its source table"))
}

fn set_owner(row: &mut EntitySnapshot, owner: u16) {
    row.source_id = owner;
    row.record[30..32].copy_from_slice(&owner.to_le_bytes());
}

/// sub_6DCA0 family15 allocation, followed by the source hand, aim and speed
/// fields. The shared guided dispatcher owns every movement and collision pass.
pub(crate) fn launch(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    holder_slot: u16,
    parent_slot: u16,
    input: CastInput,
) -> Result<Option<u16>, String> {
    let (initial_parent_position, inherited_speed) = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|row| row.active && parent_slot != 0)
        .map(|row| (row.position, row.actual_speed))
        .ok_or("Tremor cast lost its active parent")?;
    let holder_tier = arena
        .entities
        .get(usize::from(holder_slot))
        .filter(|row| row.active && row.class == 15 && row.model == 15 && row.action == 45)
        .map(|row| row.level_gate)
        .ok_or("Tremor cast lost its family15 active holder")?;
    let tier = crate::spell_activation::level2_applied_tier(holder_tier)
        .ok_or("Tremor holder has an invalid source tier")?;
    let rule = tier_rule(tier)?;
    let particle = *particles
        .get(211)
        .ok_or("Tremor requires retained particle211")?;
    if particle.animation_limit().is_none() {
        return Err(format!(
            "Tremor particle211 has unsupported render mode {}",
            particle.render_mode
        ));
    }
    if RULES.projectile_speed <= 0
        || RULES.maximum_launch_speed < RULES.projectile_speed
        || RULES.projectile_range <= 0
    {
        return Err("Tremor source projectile rules are invalid".to_owned());
    }
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let base_position = if slot == parent_slot {
        Position { x: 0, y: 0, z: 0 }
    } else {
        initial_parent_position
    };
    let mut row = super::super::cave_features::new_event(
        slot,
        arena.economy.new_event_rng(slot),
        base_position,
    );
    row.class = 9;
    row.model = 23;
    row.record[67] = 9;
    row.record[68] = 23;
    row.set_source_action(24);
    row.set_control_flags((row.control_flags & !8) | 4);
    row.set_actual_speed(
        RULES
            .projectile_speed
            .wrapping_add(inherited_speed)
            .clamp(RULES.projectile_speed, RULES.maximum_launch_speed),
    );
    row.record[136..138].copy_from_slice(&(RULES.projectile_speed as u16).to_le_bytes());
    row.set_max_life(RULES.projectile_range / i32::from(RULES.projectile_speed));
    row.set_life(row.max_life);
    row.parameter_relation = Some(60);
    row.record[164..168].copy_from_slice(&61_u32.to_le_bytes());
    super::super::cave_features::bind_particle(&mut row, 211, particle)?;
    row.record[71] = 10;
    row.record[72] = 71;
    row.set_mana(50);
    row.set_subspell_index(rule.damage);
    row.set_source_byte_46(rule.impact_parameter);
    row.set_source_dword_10(i32::from(input.auxiliary_byte));
    row.record[42..44].copy_from_slice(&holder_slot.to_le_bytes());
    row.record[44..46].copy_from_slice(&parent_slot.to_le_bytes());

    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, base_position, true, true)?;

    // These source pointers are read again after NewEvent. Allocation may
    // have replaced either fixed-address row, so all launch-time parent and
    // holder fields below come from their current occupants.
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
        .ok_or("Tremor parent address disappeared after NewEvent")?;
    let holder_mana = arena
        .entities
        .get(usize::from(holder_slot))
        .ok_or("Tremor holder address disappeared after NewEvent")?
        .mana;

    // sub_68E50's hand offset and ground fallback precede the caster height
    // lift. Aim offsets and the source target axis are then retained on the
    // same fixed-address projectile row.
    let mut position = arena.entities[usize::from(slot)].position;
    let base_launch_position = position;
    let hand = if parent_control_flags & 0x100 != 0 {
        Some(parent_yaw.wrapping_sub(512) & 2047)
    } else if parent_control_flags & 0x200 != 0 {
        Some(parent_yaw.wrapping_add(512) & 2047)
    } else {
        None
    };
    if let Some(hand_yaw) = hand {
        move_entity(&mut position, hand_yaw, 0, 256);
        if terrain.ground_height(position) > position.z {
            position = base_launch_position;
        }
    }
    position.z = position.z.wrapping_add(parent_fov);
    let yaw = input.aim_yaw.wrapping_add(parent_yaw);
    let pitch = input.aim_pitch.wrapping_add(parent_pitch);
    let mut axis = parent_position;
    move_entity(&mut axis, yaw, 0, RULES.aim_distance);
    axis.z = terrain.ground_height(axis);
    let projectile = &mut arena.entities[usize::from(slot)];
    projectile.set_position(position);
    set_owner(projectile, parent_source_id);
    projectile.set_mana(holder_mana);
    projectile.record[158..160].copy_from_slice(&axis.x.to_le_bytes());
    projectile.record[160..162].copy_from_slice(&axis.y.to_le_bytes());
    projectile.record[162..164].copy_from_slice(&axis.z.to_le_bytes());
    projectile.set_yaw(yaw);
    projectile.set_pitch(pitch);

    arena.economy.source_rng.insert(slot, projectile.entity_rng);
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    Ok(Some(slot))
}

/// sub_6DCA0's source action24 impact consumer: convert the live carrier into
/// the scheduled model71 controller at the collision position.
pub(crate) fn construct_effect(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    projectile: &EntitySnapshot,
) -> Result<Option<u16>, String> {
    if !projectile.active || (projectile.class, projectile.model) != (9, 23) {
        return Err("Tremor impact is not a live source9/model23 carrier".to_owned());
    }
    terrain_tremor::construct(arena, terrain, projectile.position)
}

/// The source action24 wrapper: shared movement/collision first, then
/// sub_677D0 reads byte46 from the current fixed-address flyer and binds the
/// lifetime onto the already-propagated model71 controller.
pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    targets: Option<&super::super::guided_projectile::Targets<'_>>,
    context: crate::native_world::source_target::Context<'_>,
) -> Result<super::super::guided_projectile::StepOutcome, String> {
    if !arena
        .entities
        .get(usize::from(slot))
        .is_some_and(|row| row.active && (row.class, row.model, row.action) == (9, 23, 24))
    {
        return Err("Tremor action24 has no live source9/model23 projectile".to_owned());
    }
    let outcome = super::super::guided_projectile::step(
        arena,
        terrain,
        particles,
        slot,
        super::super::guided_projectile::Guidance {
            yaw_turn: RULES.projectile_yaw_turn,
            pitch_turn: RULES.projectile_pitch_turn,
        },
        targets,
        context,
    )?;
    if let Some(impact_slot) = outcome.impact_slot {
        let impact_lifetime = arena
            .entities
            .get(usize::from(slot))
            .map(|projectile| projectile.record[74] & 0xf0)
            .ok_or("Tremor flyer address disappeared after guided impact")?;
        let impact = arena
            .entities
            .get_mut(usize::from(impact_slot))
            .filter(|row| row.active && (row.class, row.model, row.action) == (10, 71, 78))
            .ok_or("Tremor guided impact did not publish a live model71 controller")?;
        impact.set_source_byte_46(0);
        impact.set_max_life(i32::from(impact_lifetime));
        impact.set_life(i32::from(impact_lifetime));
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guided_impact_keeps_mutated_payload_and_masks_lifetime() {
        let mut session = super::super::super::tests::level_two_campaign_session();
        let particles = session.session.definition.presentation.particles.clone();
        let live = &mut session.session.state.live;
        let parent = live.core.state.entity_index;
        let holder = live
            .shared_spells
            .pickups()
            .iter()
            .find(|row| row.model == 3)
            .unwrap()
            .index;
        let mut row = live.shared_spells.pickup(holder).unwrap().clone();
        row.model = 15;
        row.record[68] = 15;
        row.set_source_action(45);
        row.apply_nyphur_spell_profile(
            crate::spell_activation::level2_set_spell_profile(15, 0).unwrap(),
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
        .unwrap();
        let mut position = (0..65536_u32)
            .map(|tile| Position {
                x: ((tile & 255) << 8) as u16,
                y: ((tile >> 8) << 8) as u16,
                z: 0,
            })
            .find(|position| live.terrain.material_mask(*position) != 1)
            .unwrap();
        position.z = live.terrain.ground_height(position).wrapping_sub(1);
        let row = &mut live.economy.entities[usize::from(flyer)];
        row.set_position(position);
        row.set_actual_speed(0);
        row.record[136..138].copy_from_slice(&0_i16.to_le_bytes());
        // A reflected or otherwise mutated payload remains source data;
        // the bare controller constructor must not reject it as a bad tier.
        row.set_subspell_index(4321);
        row.set_source_byte_46(0x9b);
        crate::world::effects::sync_position(
            &mut live.economy,
            &mut live.terrain,
            flyer,
            position,
            true,
            false,
        )
        .unwrap();
        let visit = step_projectile(
            &mut live.economy,
            &mut live.terrain,
            &particles,
            flyer,
            None,
            crate::native_world::source_target::Context::from_ordinary(&live.goats, &live.ordinary),
        )
        .unwrap();
        let child = visit
            .impact_slot
            .expect("guided ground collision creates Tremor");
        let row = &live.economy.entities[usize::from(child)];
        assert_eq!((row.class, row.model, row.action), (10, 71, 78));
        assert_eq!((row.record[74], row.max_life, row.life), (0, 0x90, 0x90));
        assert_eq!(
            (row.source_id, row.subspell_index),
            (live.economy.entities[usize::from(parent)].source_id, 4321)
        );

        // Continue through the ordinary source scheduler: the resulting
        // controller must change the terrain sampled by flight, not just
        // produce a model71 row.
        let before = session.clone();
        let phase = before.session.state.live.economy.entities[usize::from(child)].phase;
        session.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "scheduled Tremor must not roll back: {:?}",
            session.dispatch_events()
        );
        let live = &session.session.state.live;
        assert_eq!(
            live.economy.entities[usize::from(child)].phase,
            phase.wrapping_add(1)
        );
        let previous_terrain = &before.session.state.live.terrain;
        let center_x = (position.x.wrapping_add(128) >> 8) as u8;
        let center_y = (position.y.wrapping_add(128) >> 8) as u8;
        let changed = (-2_i8..=2)
            .flat_map(|dy| (-2_i8..=2).map(move |dx| (dx, dy)))
            .map(|(dx, dy)| {
                let x = center_x.wrapping_add_signed(dx);
                let y = center_y.wrapping_add_signed(dy);
                Position {
                    x: u16::from(x) << 8,
                    y: u16::from(y) << 8,
                    z: 0,
                }
            })
            .find(|&point| {
                previous_terrain.ground_height(point) != live.terrain.ground_height(point)
            });
        assert!(
            changed.is_some(),
            "scheduled Tremor changes flight terrain sampling"
        );
    }
}
