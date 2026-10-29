//! MC2 Volcano (spell18) delivery and impact binding.
//! The common flyer owns movement; the terrain source owns the eruption.

use super::{CastInput, ParticleParameter, SessionTerrain};
use crate::{
    FlightWorld, Position,
    campaign::source_arena::SourceArena,
    level_definition::VOLCANO_PROPERTIES as RULES,
    move_entity,
    native_world::{EntitySnapshot, guided_projectile},
};

const PROJECTILE_PARTICLE: u16 = 210;

fn put(row: &mut EntitySnapshot, offset: usize, value: u16) -> Result<(), String> {
    let bytes = row
        .record
        .get_mut(offset..offset + 2)
        .ok_or("Volcano source word is outside the entity record")?;
    bytes.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn put_byte(row: &mut EntitySnapshot, offset: usize, value: u8) -> Result<(), String> {
    let byte = row
        .record
        .get_mut(offset)
        .ok_or("Volcano source byte is outside the entity record")?;
    *byte = value;
    Ok(())
}

fn put_u32(row: &mut EntitySnapshot, offset: usize, value: u32) -> Result<(), String> {
    let bytes = row
        .record
        .get_mut(offset..offset + 4)
        .ok_or("Volcano source dword is outside the entity record")?;
    bytes.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn put_i16(row: &mut EntitySnapshot, offset: usize, value: i16) -> Result<(), String> {
    let bytes = row
        .record
        .get_mut(offset..offset + 2)
        .ok_or("Volcano source word is outside the entity record")?;
    bytes.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn set_owner(row: &mut EntitySnapshot, owner: u16) -> Result<(), String> {
    row.source_id = owner;
    put(row, 30, owner)
}

/// sub_6BF30 -> sub_6DCA0(18), followed by sub_68E50 and the source payload
/// writes. The retained Volcano projectile is class9/model4/action4; its
/// terminal selector constructs the scheduled class10/model9 eruption.
pub(crate) fn launch(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    holder_slot: u16,
    parent_slot: u16,
    input: CastInput,
) -> Result<Option<u16>, String> {
    // sub_6DCA0 receives the position, speed and selected spell row by value;
    // NewEvent may reuse these fixed addresses, so only those constructor
    // inputs are captured before allocation.
    let (initial_rule, initial_position, inherited_speed) = {
        let holder = arena
            .entities
            .get(usize::from(holder_slot))
            .filter(|row| (row.class, row.model, row.action) == (15, 18, 54))
            .ok_or("Volcano launch holder is not live")?;
        let tier = usize::from(
            crate::spell_activation::level2_applied_tier(holder.level_gate)
                .ok_or("Volcano holder has a negative source tier")?,
        );
        let rule = *RULES
            .tiers
            .get(tier)
            .ok_or("Volcano holder tier is outside the rule table")?;
        let parent = arena
            .entities
            .get(usize::from(parent_slot))
            .filter(|_| parent_slot != 0)
            .ok_or("Volcano parent address is outside the source arena")?;
        (rule, parent.position, parent.actual_speed)
    };
    let projectile_life = RULES
        .projectile_range
        .checked_div(i32::from(RULES.projectile_speed))
        .filter(|life| *life > 0)
        .ok_or("Volcano projectile speed or range is invalid")?;
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let position = if slot == parent_slot {
        Position::default()
    } else {
        initial_position
    };
    let mut row =
        super::super::cave_features::new_event(slot, arena.economy.new_event_rng(slot), position);
    row.class = 9;
    row.model = 4;
    put_byte(&mut row, 67, 9)?;
    put_byte(&mut row, 68, 4)?;
    row.set_source_action(4);
    row.set_control_flags((row.control_flags & !8) | 4);
    row.set_actual_speed(
        RULES
            .projectile_speed
            .wrapping_add(inherited_speed)
            .clamp(RULES.projectile_speed, RULES.maximum_launch_speed),
    );
    put_i16(&mut row, 136, RULES.projectile_speed)?;
    row.set_max_life(projectile_life);
    row.set_life(row.max_life);
    row.set_subspell_index(initial_rule.damage);
    row.set_source_byte_46(initial_rule.impact_parameter);
    row.parameter_relation = Some(60);
    put_u32(&mut row, 164, 61)?;
    super::super::cave_features::bind_particle(
        &mut row,
        PROJECTILE_PARTICLE,
        *particles
            .get(usize::from(PROJECTILE_PARTICLE))
            .ok_or("Volcano requires retained particle210")?,
    )?;
    put_byte(&mut row, 71, 10)?;
    put_byte(&mut row, 72, 9)?;
    *arena
        .entities
        .get_mut(usize::from(slot))
        .ok_or("Volcano projectile slot is outside the source arena")? = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, true)?;

    // sub_6BF30 re-reads both fixed addresses after NewEvent publication.
    // If either address was reused, source behavior observes the new occupant.
    let parent = arena
        .entities
        .get(usize::from(parent_slot))
        .ok_or("Volcano parent address is outside the source arena")?
        .clone();
    let holder = arena
        .entities
        .get(usize::from(holder_slot))
        .ok_or("Volcano holder address is outside the source arena")?
        .clone();
    let tier = usize::from(
        crate::spell_activation::level2_applied_tier(holder.level_gate)
            .ok_or("Volcano holder has a negative source tier")?,
    );
    let rule = *RULES
        .tiers
        .get(tier)
        .ok_or("Volcano holder tier is outside the rule table")?;
    let row = arena
        .entities
        .get_mut(usize::from(slot))
        .ok_or("Volcano projectile slot is outside the source arena")?;
    let mut position = row.position;
    let hand = if parent.control_flags & 0x100 != 0 {
        Some(parent.yaw.wrapping_sub(512) & 2047)
    } else if parent.control_flags & 0x200 != 0 {
        Some(parent.yaw.wrapping_add(512) & 2047)
    } else {
        None
    };
    if let Some(yaw) = hand {
        move_entity(&mut position, yaw, 0, 256);
        if terrain.ground_height(position) > position.z {
            position = row.position;
        }
    }
    position.z = position.z.wrapping_add(parent.array_fov());
    row.set_position(position);
    put(row, 42, holder_slot)?;
    set_owner(row, parent.source_id)?;
    row.set_mana(holder.mana);
    row.set_subspell_index(rule.damage);
    row.set_source_byte_46(rule.impact_parameter);
    row.set_source_dword_10(i32::from(input.auxiliary_byte));
    let yaw = input.aim_yaw.wrapping_add(parent.yaw);
    let pitch = input.aim_pitch.wrapping_add(parent.pitch);
    let mut axis = parent.position;
    move_entity(&mut axis, yaw, 0, RULES.aim_distance);
    axis.z = terrain.ground_height(axis);
    put(row, 158, axis.x)?;
    put(row, 160, axis.y)?;
    put_i16(row, 162, axis.z)?;
    row.set_yaw(yaw);
    row.set_pitch(pitch);
    arena.economy.source_rng.insert(slot, row.entity_rng);
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    Ok(Some(slot))
}

/// The projectile flyer has one implementation. Volcano only supplies its
/// authored steering rates and lets the common terminal path call
/// `construct_effect` before the scheduler's same-pass tail.
pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    targets: Option<&guided_projectile::Targets<'_>>,
    context: crate::native_world::source_target::Context<'_>,
) -> Result<guided_projectile::StepOutcome, String> {
    let outcome = guided_projectile::step(
        arena,
        terrain,
        particles,
        slot,
        guided_projectile::Guidance {
            yaw_turn: RULES.projectile_yaw_turn,
            pitch_turn: RULES.projectile_pitch_turn,
        },
        targets,
        context,
    )?;
    if let Some(child) = outcome.impact_slot {
        // sub_66250 runs after sub_65820 has copied its ordinary impact
        // fields. In particular, its byte46=0 must follow that copy.
        let impact_parameter = arena
            .entities
            .get(usize::from(slot))
            .and_then(|row| row.record.get(74))
            .copied()
            .ok_or("Volcano current flyer byte46 is outside the source arena")?;
        let source = arena
            .entities
            .get_mut(usize::from(child))
            .ok_or("Volcano impact slot is outside the source arena")?;
        source.set_source_byte_46(0);
        source.set_max_life(i32::from(impact_parameter));
    }
    Ok(outcome)
}

/// Bind an impact child to the existing source-order Volcano constructor.
/// The action4 wrapper reads byte46 after NewEvent, so retain the flyer address
/// and reborrow it after construction in case that address was reused.
pub(crate) fn construct_effect(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    projectile: &EntitySnapshot,
) -> Result<Option<u16>, String> {
    if (projectile.class, projectile.model, projectile.action) != (9, 4, 4) {
        return Err(format!(
            "Volcano impact received unrelated projectile selector {:?}",
            (projectile.class, projectile.model, projectile.action)
        ));
    }
    // sub_65820 copies the current flyer's owner only after the source
    // constructor returns, including if allocation reused its address.
    Ok(crate::world::terrain::disasters::mc2::volcano::construct(
        arena,
        terrain,
        projectile.position,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impact_wrapper_clears_phase_after_shared_payload_copy() {
        for water in [false, true] {
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
            row.model = 18;
            row.record[68] = 18;
            row.set_source_action(54);
            row.apply_nyphur_spell_profile(
                crate::spell_activation::level2_set_spell_profile(18, 0).unwrap(),
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
            let projectile = live.economy.entities[usize::from(flyer)].clone();
            if !water {
                let mut bare_arena = live.economy.clone();
                let mut bare_terrain = live.terrain.clone();
                let bare_slot = construct_effect(&mut bare_arena, &mut bare_terrain, &projectile)
                    .unwrap()
                    .expect("source Volcano constructor allocates its default row");
                let bare = &bare_arena.entities[usize::from(bare_slot)];
                assert_eq!((bare.source_id, bare.subspell_index), (bare_slot, 2_000));
                let mut unrelated = projectile.clone();
                unrelated.set_source_action(5);
                assert!(construct_effect(&mut bare_arena, &mut bare_terrain, &unrelated).is_err());
            }
            // Pin both dry and water cells: source model4 creates its real
            // Volcano on either, instead of entering the generic water splash.
            // This tests the impact boundary, not acquisition or human playability.
            let mut position = (0..65536_u32)
                .map(|tile| Position {
                    x: ((tile & 255) << 8) as u16,
                    y: ((tile >> 8) << 8) as u16,
                    z: 0,
                })
                .find(|position| (live.terrain.material_mask(*position) == 1) == water)
                .unwrap();
            position.z = live.terrain.ground_height(position).wrapping_sub(1);
            let row = &mut live.economy.entities[usize::from(flyer)];
            row.set_position(position);
            row.set_actual_speed(0);
            row.record[136..138].copy_from_slice(&0_i16.to_le_bytes());
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
                crate::native_world::source_target::Context::from_ordinary(
                    &live.goats,
                    &live.ordinary,
                ),
            )
            .unwrap();
            let child = visit
                .impact_slot
                .expect("ordinary ground collision constructs Volcano");
            let row = &live.economy.entities[usize::from(child)];
            assert_eq!((row.class, row.model, row.action), (10, 9, 9));
            assert_eq!((row.record[74], row.max_life, row.life), (0, 7, 17));
            assert_eq!(row.subspell_index, RULES.tiers[0].damage);
            assert_eq!(row.source_id, projectile.source_id);
        }
    }
}
