//! Source holders and guided projectiles for Gravity Well20 and Whirlwind21.
//!
//! This adapter owns the ordinary holder callback and its source-local
//! projectile payload writes. Shared scheduling still owns phase advancement,
//! the same-pass tail, and next-pass reclamation; guided flight and live field
//! state remain with their existing owners.

use super::{EntitySnapshot, EntitySpecialRelation, Level2SharedSpellResetSession, ParticleParameter, SessionTerrain};
use crate::campaign::source_arena::SourceArena;
use crate::level_definition::{FieldSpellTier, GRAVITY_WELL_TIERS, WHIRLWIND_TIERS};
use crate::spells::{gravity_well, whirlwind};
use crate::{FlightWorld, Position, move_entity};

pub(crate) use crate::spell_activation::{
    ProjectileSpellCastInput as CastInput, ProjectileSpellHolderOutcome as HolderOutcome,
};

const PROJECTILE_SPEED: i16 = 384;
const PROJECTILE_MAXIMUM_LAUNCH_SPEED: i16 = 8_192;
const PROJECTILE_RANGE: i32 = 8_192;
const AIM_DISTANCE: i16 = 4_096;
const GUIDANCE_TURN: i16 = 22;
const PARAMETER_RELATION: u16 = 60;
const GRAVITY_PARTICLE: u16 = 211;
const WHIRLWIND_PARTICLE: u16 = 320;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Family {
    GravityWell,
    Whirlwind,
}

impl Family {
    fn holder_model(self) -> u8 {
        match self {
            Self::GravityWell => 20,
            Self::Whirlwind => 21,
        }
    }

    fn holder_action(self) -> u8 {
        match self {
            Self::GravityWell => 60,
            Self::Whirlwind => 63,
        }
    }

    fn projectile_model(self) -> u8 {
        match self {
            Self::GravityWell => 22,
            Self::Whirlwind => 26,
        }
    }

    fn projectile_action(self) -> u8 {
        match self {
            Self::GravityWell => 23,
            Self::Whirlwind => 27,
        }
    }

    fn terminal_model(self) -> u8 {
        match self {
            Self::GravityWell => 67,
            Self::Whirlwind => 22,
        }
    }

    fn particle(self) -> u16 {
        match self {
            Self::GravityWell => GRAVITY_PARTICLE,
            Self::Whirlwind => WHIRLWIND_PARTICLE,
        }
    }

    fn profile_family(self) -> u8 {
        match self {
            Self::GravityWell => 20,
            Self::Whirlwind => 21,
        }
    }
}

fn word(row: &EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}

fn put_word(row: &mut EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn set_owner(row: &mut EntitySnapshot, owner: u16) {
    row.source_id = owner;
    put_word(row, 30, owner);
}

pub(crate) fn source_constructor(family: u8) -> Result<super::guided_projectile::Constructor, String> {
    let family = match family {
        20 => Family::GravityWell,
        21 => Family::Whirlwind,
        _ => return Err(format!("field spell family {family} has no guided source constructor")),
    };
    Ok(super::guided_projectile::Constructor {
        model: family.projectile_model(),
        action: family.projectile_action(),
        parameter: PARAMETER_RELATION,
        particle: family.particle(),
        speed: PROJECTILE_SPEED,
        maximum_speed: PROJECTILE_MAXIMUM_LAUNCH_SPEED,
        life: PROJECTILE_RANGE / i32::from(PROJECTILE_SPEED),
        mana: 50,
    })
}

fn target_axis(origin: Position, yaw: i16, terrain: &impl FlightWorld) -> Position {
    // sub_6C3E0/sub_6C620 target the ground plane at yaw only, independent of
    // flyer pitch.
    let mut axis = origin;
    move_entity(&mut axis, yaw, 0, AIM_DISTANCE);
    axis.z = terrain.ground_height(axis);
    axis
}

#[derive(Clone, Copy)]
struct CasterState {
    life: i32,
    mana: i32,
    mana_regen: i32,
    castle_mana: Option<i32>,
}

fn holder_family(holder: &EntitySnapshot) -> Result<Family, String> {
    match (holder.class, holder.model, holder.action) {
        (15, 20, 60) => Ok(Family::GravityWell),
        (15, 21, 63) => Ok(Family::Whirlwind),
        selector => Err(format!(
            "field spell active holder has unsupported source selector {selector:?}"
        )),
    }
}

fn typed_tier(family: Family, level_gate: i8) -> Result<FieldSpellTier, String> {
    let tier = crate::spell_activation::level2_applied_tier(level_gate)
        .ok_or("field spell holder has a negative source tier")?;
    let tiers = match family {
        Family::GravityWell => &GRAVITY_WELL_TIERS,
        Family::Whirlwind => &WHIRLWIND_TIERS,
    };
    tiers
        .get(usize::from(tier))
        .copied()
        .ok_or_else(|| format!("field spell source tier {tier} is outside its typed profile"))
}

fn launch(
    shared: &Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    holder_slot: u16,
    parent_slot: u16,
    input: CastInput,
    family: Family,
    projectile_power: u16,
    effect_life: u8,
) -> Result<Option<(u16, bool)>, String> {
    let (position, inherited_speed) = {
        let parent = arena
            .entities
            .get(usize::from(parent_slot))
            .ok_or("field spell parent address is outside source arena")?;
        (parent.position, parent.actual_speed)
    };
    let Some(slot) = super::guided_projectile::construct(
        arena, terrain, particles, position, Some(parent_slot), inherited_speed,
        source_constructor(family.profile_family())?,
    )? else {
        return Ok(None);
    };
    let row = &mut arena.entities[usize::from(slot)];
    let mut position = row.position;
    row.record[71] = 10;
    row.record[72] = family.terminal_model();
    put_word(row, 42, holder_slot);
    row.set_mana(50);
    row.set_subspell_index(projectile_power);
    row.set_source_byte_46(effect_life);
    row.set_source_dword_10(i32::from(input.auxiliary_byte));

    // NewEvent can replace either fixed address. The source rereads both
    // addresses before applying the final owner, mana, hand, and aim writes.
    let parent = arena.entities[usize::from(parent_slot)].clone();
    let uses_global_a4 = parent.special_relation == EntitySpecialRelation::GlobalSpecialState;
    let input = if uses_global_a4 {
        CastInput {
            aim_yaw: shared.global_special_state.input_yaw,
            aim_pitch: shared.global_special_state.input_pitch,
            auxiliary_byte: shared.global_special_state.cast_auxiliary,
            ..input
        }
    } else {
        input
    };
    let holder = arena
        .entities
        .get(usize::from(holder_slot))
        .ok_or("field spell holder address is outside source arena")?
        .clone();
    let row = &mut arena.entities[usize::from(slot)];
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
    put_word(row, 42, holder_slot);
    set_owner(row, parent.source_id);
    row.set_mana(holder.mana);
    row.set_subspell_index(projectile_power);
    row.set_source_byte_46(effect_life);
    row.set_source_dword_10(i32::from(input.auxiliary_byte));
    let yaw = input.aim_yaw.wrapping_add(parent.yaw);
    let pitch = input.aim_pitch.wrapping_add(parent.pitch);
    // If the parent address was recycled for this projectile, the source's
    // preceding z write has already changed the position read for the axis.
    let axis_origin = if slot == parent_slot { position } else { parent.position };
    let axis = target_axis(axis_origin, yaw, terrain);
    put_word(row, 158, axis.x);
    put_word(row, 160, axis.y);
    put_word(row, 162, axis.z as u16);
    row.set_yaw(yaw);
    row.set_pitch(pitch);
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    Ok(Some((slot, uses_global_a4)))
}

/// One source active-holder callback. The scheduler owns phase and reclaim
/// tails; this function only emits the ordinary projectile and holder writes.
pub(crate) fn step_holder(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    input: CastInput,
) -> Result<HolderOutcome, String> {
    let mut holder = shared
        .pickup(slot)
        .cloned()
        .ok_or("field spell active holder is absent")?;
    let family = holder_family(&holder)?;
    let profile = typed_tier(family, holder.level_gate)?;
    // Shared typed spell state is authoritative. Publish it before NewEvent
    // can recycle this fixed address, as the source handler does.
    arena.entities[usize::from(slot)] = holder.clone();
    let parent_slot = holder.source_spell_parent_entity_index();
    let caster = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|_| parent_slot != 0)
        .map(|parent| CasterState {
            life: parent.life,
            mana: parent.mana,
            mana_regen: parent.mana_regen,
            castle_mana: input.castle_mana,
        });
    let (payload, parent_mana_regen, cleanup, active, cooldown) = match family {
        Family::GravityWell => {
            let mut state = gravity_well::Holder {
                active: holder.active_value,
                duration: holder.activation_value as i16,
                cooldown: word(&holder, 58),
            };
            let mut leaf_caster = caster.map(|caster| gravity_well::Caster {
                life: caster.life,
                mana: caster.mana,
                mana_regen: caster.mana_regen,
                castle_mana: caster.castle_mana,
            });
            let visit = state.step(
                gravity_well::Rule {
                    power: profile.power,
                    effect_life: profile.effect_parameter,
                    mana_cost: holder.max_mana,
                    castle_mana_required: holder.mana_regen,
                },
                leaf_caster.as_mut(),
            );
            (
                visit
                    .projectile_payload
                    .map(|payload| (payload.power, payload.effect_life)),
                visit.parent_mana_regen,
                visit.cleanup,
                state.active,
                state.cooldown,
            )
        }
        Family::Whirlwind => {
            let mut state = whirlwind::Holder {
                active: holder.active_value,
                duration: holder.activation_value as i16,
                cooldown: word(&holder, 58),
            };
            let mut leaf_caster = caster.map(|caster| whirlwind::Caster {
                life: caster.life,
                mana: caster.mana,
                mana_regen: caster.mana_regen,
                castle_mana: caster.castle_mana,
            });
            let visit = state.step(
                whirlwind::Rule {
                    power: profile.power,
                    effect_life: profile.effect_parameter,
                    mana_cost: holder.max_mana,
                    castle_mana_required: holder.mana_regen,
                },
                leaf_caster.as_mut(),
            );
            (
                visit
                    .projectile_payload
                    .map(|payload| (payload.projectile_power, payload.effect_life)),
                visit.parent_mana_regen,
                visit.cleanup,
                state.active,
                state.cooldown,
            )
        }
    };

    let mut outcome = HolderOutcome::default();
    if let Some((projectile_power, effect_life)) = payload {
        let emission = launch(
            shared,
            arena,
            terrain,
            particles,
            slot,
            parent_slot,
            input,
            family,
            projectile_power,
            effect_life,
        )?;
        outcome.projectile = emission.map(|(slot, _)| slot);
        if let Some((_, uses_global_a4)) = emission {
            if uses_global_a4 {
                shared.global_special_state.cast_auxiliary = 0;
            } else {
                outcome.clear_player_aux = Some(parent_slot);
            }
            crate::mark_source_audio_call(crate::SourceAudioCallTag::ProjectileDeliveryLaunch);
            shared.emit_flight_effect(crate::FlightEvent::PrepareSound {
                #[cfg(test)]
                audio_site: crate::MissionAudioSite::MeteorLaunch,
                entity_index: parent_slot,
                player_index: -1,
                sound_id: 15,
            });
        }
    }

    if parent_mana_regen.is_some()
        && let Some(parent) = arena.entities.get_mut(usize::from(parent_slot))
    {
        // sub_68DE0 reads the current parent after the projectile allocation,
        // and its price comes from the live holder rather than SPELLS.DAT.
        let mana_regen = if holder.active_value == holder.activation_value as i16 {
            if parent.mana_regen >= 0 {
                holder.max_mana.wrapping_neg()
            } else {
                parent.mana_regen.wrapping_sub(holder.max_mana)
            }
        } else if parent.mana_regen > 0 {
            0
        } else {
            parent.mana_regen
        };
        parent.set_mana_regen(mana_regen);
        shared.sync_player_entity_from_authority(parent);
        if parent_slot == shared.player_entity.index {
            shared.sync_typed_spell_parent_caches_from_player_entity();
        }
    }

    holder.active_value = active;
    if cleanup {
        holder.record[50..52].copy_from_slice(&0_i16.to_le_bytes());
        let deferred = holder.source_word_2c();
        if deferred != 0 {
            let tier = deferred.checked_sub(1)
                .and_then(|value| u8::try_from(value.min(2)).ok())
                .ok_or("field spell deferred tier is negative")?;
            let profile = shared
                .physical_spell_profile(family.profile_family(), tier, parent_slot, None)
                .ok_or("field spell deferred tier has no source profile")?;
            holder.apply_nyphur_spell_profile(profile, tier);
            holder.set_source_word_2c(0);
        }
    }
    put_word(&mut holder, 58, cooldown);
    holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
    if arena.entities[usize::from(slot)].class == 15
        && arena.entities[usize::from(slot)].model == family.holder_model()
    {
        if let Some(current) = shared.pickup_mut(slot) {
            *current = holder.clone();
        }
        arena.entities[usize::from(slot)] = holder;
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::{
        CastInput, Family, GRAVITY_WELL_TIERS, WHIRLWIND_TIERS, source_constructor,
        step_holder, step_projectile, target_axis, word,
    };
    use crate::{FlightWorld, Position, Tile, move_entity};

    struct TestTerrain;

    impl FlightWorld for TestTerrain {
        fn ground_height(&self, position: Position) -> i16 {
            position.x.wrapping_add(position.y) as i16
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            None
        }

        fn material_mask(&self, _position: Position) -> u32 {
            0
        }

        fn angle_flags(&self, _tile: Tile) -> u8 {
            0
        }
    }

    #[test]
    fn launch_speed_wraps_as_i16_before_source_clamps() {
        let definition = source_constructor(20).unwrap();
        assert_eq!(super::super::guided_projectile::source_launch_speed(definition.speed, i16::MAX, definition.maximum_speed), 384);
        assert_eq!(super::super::guided_projectile::source_launch_speed(definition.speed, 8_000, definition.maximum_speed), 8_192);
    }

    #[test]
    fn projectile_constructor_only_clears_source_flag_08() {
        assert_eq!(super::super::guided_projectile::source_projectile_flags(0), 0);
        assert_eq!(super::super::guided_projectile::source_projectile_flags(0x0c), 0x04);
    }

    #[test]
    fn target_axis_uses_yaw_only_then_samples_ground_height() {
        let origin = Position {
            x: 1_000,
            y: 2_000,
            z: 500,
        };
        let yaw = 512;
        let terrain = TestTerrain;
        let axis = target_axis(origin, yaw, &terrain);

        let mut expected = origin;
        move_entity(&mut expected, yaw, 0, 4_096);
        expected.z = terrain.ground_height(expected);
        assert_eq!(axis, expected);

        let mut pitched = origin;
        move_entity(&mut pitched, yaw, 256, 4_096);
        assert_ne!((axis.x, axis.y), (pitched.x, pitched.y));
    }

    #[test]
    fn field_holders_read_live_cost_and_post_allocation_parent() {
        for family in [Family::GravityWell, Family::Whirlwind] {
            let id = family.holder_model();
            let mut simulation = crate::native_world::tests::level_two_campaign_session();
            simulation.apply_carried_spell_families(&[id], Some(id)).unwrap();
            let parent_slot = simulation.state.entity_index;
            let holder_slot = simulation.shared_spells().local_player.enabled_spell_holders[usize::from(id)];
            simulation.session.state.live.economy.economy.exhaust_new_event_capacity_for_test();
            let particles = simulation.session.definition.presentation.particles.clone();
            let live = &mut simulation.session.state.live;
            live.economy.finish_source_group();
            let holder = live.shared_spells.pickup_mut(holder_slot).unwrap();
            holder.active_value = holder.activation_value as i16;
            holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
            holder.max_mana = 1_234;
            holder.record[144..148].copy_from_slice(&1_234_i32.to_le_bytes());
            holder.set_mana_regen(321);
            crate::world::terrain::cave_features::reclaim(&mut live.economy, &mut live.terrain, parent_slot);
            let parent = &mut live.economy.entities[usize::from(parent_slot)];
            parent.set_life(100);
            parent.set_mana(1_234);
            parent.set_mana_regen(-9);
            live.shared_spells.global_special_state.input_yaw = 37;
            live.shared_spells.global_special_state.input_pitch = -21;
            live.shared_spells.global_special_state.cast_auxiliary = 11;
            let outcome = step_holder(&mut live.shared_spells, &mut live.economy, &mut live.terrain,
                &particles, holder_slot, CastInput { aim_yaw: 120, aim_pitch: 55, auxiliary_byte: 7, castle_mana: Some(321) }).unwrap();
            assert_eq!(outcome.projectile, Some(parent_slot));
            assert_eq!(outcome.clear_player_aux, None);
            assert_eq!(live.shared_spells.global_special_state.cast_auxiliary, 0);
            let flyer = &live.economy.entities[usize::from(parent_slot)];
            assert_eq!((flyer.class, flyer.model, flyer.action), (9, family.projectile_model(), family.projectile_action()));
            assert_eq!((flyer.yaw, flyer.pitch, flyer.source_dword_10()), (37, -21, 11));
            assert_eq!(flyer.mana_regen, -1_234, "charge reads the replacement parent, not its old negative regeneration");
        }
    }

    #[test]
    fn field_holders_charge_failed_allocation_and_apply_deferred_free_profile() {
        for family in [Family::GravityWell, Family::Whirlwind] {
            let id = family.holder_model();
            let mut simulation = crate::native_world::tests::level_two_campaign_session();
            simulation.apply_carried_spell_families(&[id], Some(id)).unwrap();
            let parent_slot = simulation.state.entity_index;
            let holder_slot = simulation.shared_spells().local_player.enabled_spell_holders[usize::from(id)];
            simulation.session.state.live.economy.economy.exhaust_new_event_capacity_for_test();
            let particles = simulation.session.definition.presentation.particles.clone();
            let live = &mut simulation.session.state.live;
            let holder = live.shared_spells.pickup_mut(holder_slot).unwrap();
            holder.active_value = holder.activation_value as i16;
            holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
            let cost = holder.max_mana;
            let requirement = holder.mana_regen;
            let parent = &mut live.economy.entities[usize::from(parent_slot)];
            parent.set_life(100);
            parent.set_mana(cost);
            parent.set_mana_regen(13);
            let input = CastInput { aim_yaw: 0, aim_pitch: 0, auxiliary_byte: 7, castle_mana: Some(requirement) };
            let outcome = step_holder(&mut live.shared_spells, &mut live.economy, &mut live.terrain, &particles, holder_slot, input).unwrap();
            assert_eq!(outcome.projectile, None);
            assert_eq!(live.economy.entities[usize::from(parent_slot)].mana_regen, -cost);

            live.shared_spells.free_spell_usage = true;
            let expected = live.shared_spells.physical_spell_profile(id, 2, parent_slot, None).unwrap();
            let holder = live.shared_spells.pickup_mut(holder_slot).unwrap();
            holder.active_value = 1;
            holder.record[50..52].copy_from_slice(&1_i16.to_le_bytes());
            holder.set_source_word_2c(129);
            step_holder(&mut live.shared_spells, &mut live.economy, &mut live.terrain, &particles, holder_slot, input).unwrap();
            let holder = live.shared_spells.pickup(holder_slot).unwrap();
            assert_eq!(holder.active_value, 0);
            assert_eq!(holder.source_word_2c(), 0);
            assert_eq!((holder.level_gate, holder.max_mana, holder.mana_regen, holder.mana), (2, expected.max_mana, expected.mana_regen, expected.mana));
        }
    }

    #[test]
    fn scheduled_field_holders_use_reused_parent_current_castle_and_aim() {
        for family in [Family::GravityWell, Family::Whirlwind] {
            let id = family.holder_model();
            let mut base = crate::native_world::tests::level_two_campaign_session();
            let earlier_slot = {
                let live = &mut base.session.state.live;
                super::super::alliance_spell::construct_impact(&mut live.economy, &mut live.terrain).unwrap()
            };
            base.apply_carried_spell_families(&[id], Some(id)).unwrap();
            let parent_slot = base.state.entity_index;
            let holder_slot = base.shared_spells().local_player.enabled_spell_holders[usize::from(id)];
            assert!(earlier_slot < holder_slot && earlier_slot != parent_slot);
            let castle = base.economy_mut().build_castle_at(Position::default(), Position::default(), parent_slot, false, true).unwrap();
            let requirement = base.shared_spells().pickup(holder_slot).unwrap().mana_regen;
            assert!(requirement > 0);
            assert!(base.economy_mut().set_castle_mana_prerequisite_for_test(castle, requirement));
            let cost;
            {
                let live = &mut base.session.state.live;
                live.economy.finish_source_group();
                crate::world::terrain::cave_features::reclaim(&mut live.economy, &mut live.terrain, parent_slot);
                let slot = super::super::alliance_spell::construct_impact(&mut live.economy, &mut live.terrain).unwrap();
                assert_eq!(slot, parent_slot);
                let holder = live.shared_spells.pickup_mut(holder_slot).unwrap();
                holder.active_value = holder.activation_value as i16;
                holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
                cost = holder.max_mana;
                live.economy.entities[usize::from(holder_slot)] = holder.clone();
                let parent = &mut live.economy.entities[usize::from(parent_slot)];
                parent.set_life(100);
                parent.set_mana(cost);
                crate::world::terrain::cave_features::reclaim(&mut live.economy, &mut live.terrain, earlier_slot);
                live.shared_spells.global_special_state.cast_auxiliary = 11;
            }
            for (castle_slot, should_launch) in [(0, false), (castle, true)] {
                let mut simulation = base.clone();
                simulation.session.state.live.shared_spells.global_special_state.castle_entity_index = castle_slot;
                simulation.step_outer_turn(crate::PlayerCommand { argument_6: 37, argument_8: (-21_i16) as u16, ..Default::default() }, 0);
                assert!(!simulation.dispatch_events().iter().any(|event| matches!(event, super::super::Mc2DispatchEvent::Error { .. })), "{:?}", simulation.dispatch_events());
                let live = &simulation.session.state.live;
                let flyer = live.economy.entities.iter().find(|row| row.active && (row.class, row.model, row.action) == (9, family.projectile_model(), family.projectile_action()) && word(row, 42) == holder_slot);
                assert_eq!(flyer.is_some(), should_launch, "current A4 Castle controls family {id}");
                if let Some(flyer) = flyer {
                    assert_eq!((flyer.yaw, flyer.pitch, flyer.source_dword_10()), (37, -21, 11));
                    assert_eq!(live.shared_spells.global_special_state.cast_auxiliary, 0);
                    assert_eq!(live.economy.entities[usize::from(parent_slot)].mana_regen, -cost);
                } else {
                    assert_eq!(live.shared_spells.global_special_state.cast_auxiliary, 11);
                }
            }
        }
    }

    #[test]
    fn acquired_field_spells_fly_naturally_to_their_fields_at_each_tier() {
        use crate::PlayerCommand;
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        use crate::native_world::{Mc2DispatchEvent, LEVEL2_SPELL_TIER_THRESHOLDS};
        use crate::native_world::simulation::CarriedSpellProgress;

        for (family, id, level, pickup_slot, pickup_action, tiers) in [
            (Family::GravityWell, 20_u8, 21, 214_u16, 61, GRAVITY_WELL_TIERS),
            (Family::Whirlwind, 21_u8, 6, 28_u16, 64, WHIRLWIND_TIERS),
        ] {
            let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj")
                .join(format!("level{level}"));
            let mut acquired = CampaignSession::open(
                package,
                CampaignSelection::ordinary(level).unwrap(),
            ).expect("actual owner-data campaign opens");
            let player = acquired.simulation().state.entity_index;
            assert_eq!(acquired.simulation().shared_spells().local_player.enabled_spell_holders[usize::from(id)], 0);
            let pickup = &acquired.simulation().entities()[usize::from(pickup_slot)];
            assert!(pickup.active);
            assert_eq!((pickup.class, pickup.model, pickup.action), (15, id, pickup_action));
            let pickup_position = pickup.position;
            acquired.simulation_mut().set_level2_entity_position(player, pickup_position).unwrap();
            let mut receipt = None;
            for _ in 0..8 {
                acquired.step(PlayerCommand::default(), 0);
                receipt = acquired.simulation().dispatch_events().iter().find_map(|event| match event {
                    Mc2DispatchEvent::SpellAcquisition { source_slot, family, result: Some(outcome), .. }
                        if *source_slot == pickup_slot && *family == id => Some(*outcome),
                    _ => None,
                });
                if receipt.is_some() { break; }
            }
            let receipt = receipt.expect("ordinary overlap acquires the authored pickup");
            assert_eq!((receipt.source_slot, receipt.family, receipt.parent_entity_index), (pickup_slot, id, player));

            for (tier, profile) in tiers.into_iter().enumerate() {
                let mut campaign = acquired.clone();
                let session = campaign.simulation_mut();
                session.apply_carried_spell_progression(&[CarriedSpellProgress {
                    family: id,
                    persistent_experience: LEVEL2_SPELL_TIER_THRESHOLDS[usize::from(id)][tier],
                    runtime_experience: 0,
                    unlocked_tier: tier as i8,
                    selected_tier: tier as i8,
                }]).unwrap();
                let holder = session.shared_spells().local_player.enabled_spell_holders[usize::from(id)];
                assert_ne!(holder, 0);
                assert_eq!(session.shared_spells().pickup(holder).unwrap().level_gate, tier as i8);
                let position = session.state.position;
                let castle = session.economy_mut().build_castle(position, player).unwrap();
                assert!(session.economy_mut().set_castle_mana_prerequisite_for_test(castle, profile.castle_mana_required));
                // The authored pickup is at terrain level, where its first
                // projectile can naturally impact and hide in the launch
                // pass. Move the actual caster into navigable air for this
                // presentation/consequence scenario; do not alter the flyer.
                let mut cast_position = Position {
                    x: if family == Family::GravityWell {
                        position.x.wrapping_sub(4_096)
                    } else {
                        position.x.wrapping_add(4_096)
                    },
                    ..position
                };
                let ground = session.session.state.live.terrain.ground_height(cast_position);
                let ceiling = session.session.state.live.terrain.ceiling_height(cast_position);
                if let Some(ceiling) = ceiling {
                    assert!(i32::from(ceiling) - i32::from(ground) > 1_024, "controlled cast position has cave clearance");
                }
                cast_position.z = ground.saturating_add(512);
                session.set_level2_entity_position(player, cast_position).unwrap();
                let staged_mana = profile.mana_cost + 1_000;
                let live = &mut session.session.state.live;
                live.economy.refresh_owner_mana_rows();
                for row in &mut live.shared_spells.player_entities {
                    if row.index == player { row.set_mana(staged_mana); row.set_mana_regen(0); }
                }
                live.shared_spells.player_entity.set_mana(staged_mana);
                live.shared_spells.player_entity.set_mana_regen(0);
                live.shared_spells.caster.mana = staged_mana;
                live.shared_spells.player_mana = staged_mana;
                live.economy.entities[usize::from(player)].set_mana(staged_mana);
                if let Some(fire) = live.core.context.spell_fire.as_mut() { fire.caster.mana = staged_mana; }
                let owner_id = live.economy.entities[usize::from(player)].source_id;
                let existing = live.economy.entities.iter().filter(|row| row.active && row.class == 10 && row.model == family.terminal_model()).map(|row| row.index).collect::<Vec<_>>();
                let journal_start = session.effect_journal().records().len();
                let power = if family == Family::Whirlwind && profile.effect_parameter != 0 {
                    profile.power / u16::from(profile.effect_parameter)
                } else { profile.power };
                let mut projectile_seen = false;
                let mut projectile_presented = false;
                let mut field = None;
                for turn in 0..150 {
                    campaign.step(if turn == 0 { PlayerCommand { opcode: 6, argument_1: id as i8, flags: 0x40, ..PlayerCommand::default() } } else { PlayerCommand::default() }, 0);
                    let session = campaign.simulation();
                    assert!(!session.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })), "family {id} tier {tier}, turn {turn}: {:?}", session.dispatch_events());
                    if let Some(projectile) = session.entities().iter().find(|row| row.active && (row.class, row.model, row.action) == (9, family.projectile_model(), family.projectile_action()) && word(row, 42) == holder && row.subspell_index == power) {
                        projectile_seen = true;
                        if tier == 0 && !projectile.drawing_disabled() {
                            let presentations = session.campaign_entity_presentations(0).expect("actual projectile sprite projection");
                            let presentation = presentations.iter().find(|row| row.entity_index == projectile.index)
                                .unwrap_or_else(|| panic!("acquired family {id} tier {tier} projectile slot {} lacks sprite: flags={:#x} particle={:?} position={:?} dynamic={:?} same_slot={:?}", projectile.index, projectile.control_flags, projectile.particle_parameter_index, projectile.position, session.economy.live_arena_cursor_row(projectile.index), presentations.iter().filter(|row| row.entity_index == projectile.index).collect::<Vec<_>>()));
                            assert_eq!(presentation.source_id, owner_id);
                            assert_eq!(presentation.render_parameter_index, family.particle());
                            assert_eq!(presentation.base_tmap, session.session.definition.presentation.particles[usize::from(family.particle())].base_tmap);
                            projectile_presented = true;
                        }
                    }
                    field = session.entities().iter().find(|row| row.active && row.class == 10 && row.model == family.terminal_model() && row.source_id == owner_id && row.subspell_index == power && !existing.contains(&row.index)).map(|row| row.index);
                    if field.is_some() { break; }
                }
                assert!(projectile_seen, "acquired family {id} tier {tier} emits its source flyer");
                let field = field.unwrap_or_else(|| panic!("acquired family {id} tier {tier} naturally reaches its source field"));
                let field_row = &campaign.simulation().entities()[usize::from(field)];
                assert_eq!(field_row.max_life, if family == Family::GravityWell { 300 } else { i32::from(profile.effect_parameter) * 8 });
                if tier == 0 {
                    assert!(projectile_presented);
                    assert!(campaign.simulation().effect_journal().records()[journal_start..].iter().any(|record| matches!(record.effect,
                        crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound { entity_index, sound_id: 15, .. }) if entity_index == player)),
                        "acquired family {id} publishes its source launch sound");
                    let session = campaign.simulation();
                    let field_row = if family == Family::Whirlwind {
                        session.field_source_rows().into_iter().find(|row| row.index == field)
                            .expect("actual Wind publishes its live source descriptor")
                    } else {
                        session.entities()[usize::from(field)].clone()
                    };
                    let field_presentation = session.campaign_entity_presentations(0).expect("actual field sprite projection")
                        .into_iter().find(|row| row.entity_index == field);
                    if family == Family::GravityWell {
                        // sub_51730 sets control bit1; source DrawSprites
                        // intentionally omits this terrain controller.
                        assert_eq!(field_row.control_flags & 5, 5, "Gravity source is hidden from sprites and map-linked");
                        assert!(field_presentation.is_none());
                    } else {
                        let field_presentation = field_presentation.expect("natural Wind reaches the sprite observation");
                        let field_particle = field_row.particle_parameter_index.expect("source Wind has a particle descriptor");
                        assert_eq!(field_presentation.render_parameter_index, field_particle);
                        assert_eq!(field_presentation.base_tmap, session.session.definition.presentation.particles[usize::from(field_particle)].base_tmap);
                    }
                    let original_terrain = (family == Family::GravityWell).then(|| session.canonical_terrain().clone());
                    let field_position = field_row.position;
                    let mut terrain_changed = false;
                    let target_model = if family == Family::GravityWell { 20 } else { 19 };
                    let targets = if family == Family::Whirlwind {
                        // The imported Firefly is retired in the raw row;
                        // authored_class5 is its current source authority.
                        campaign.simulation().economy.authored_class5().iter()
                            .filter(|actor| actor.active && actor.model == target_model && actor.life > 0
                                && actor.drawing_enabled
                                && crate::world::fields::wind::accepts(owner_id, 0, 5, actor.model, actor.action))
                            .map(|actor| actor.source_entity_index).collect::<Vec<_>>()
                    } else {
                        campaign.simulation().entities().iter()
                            .filter(|actor| actor.active && actor.class == 5 && actor.model == target_model
                                && actor.life > 0 && !actor.drawing_disabled() && actor.action != 232
                                && actor.control_flags & 0x0010_0021 == 0 && actor.record[60] & 1 != 0)
                            .map(|actor| actor.index).collect::<Vec<_>>()
                    };
                    let target = *targets.first().unwrap_or_else(|| {
                        let observed = campaign.simulation().entities().iter()
                            .filter(|actor| actor.class == 5 && actor.model == target_model)
                            .map(|actor| (actor.index, actor.active, actor.action, actor.life,
                                actor.control_flags, actor.record[60], actor.position))
                            .collect::<Vec<_>>();
                        panic!("family {id} tier {tier} owner package retains an eligible actual creature: {observed:?}")
                    });
                    let experience_before = campaign.simulation().session.state.live.shared_spells.spell_experience()[usize::from(id)];
                    let mut placed = false;
                    let mut moved = false;
                    let mut contact = false;
                    let mut credited = false;
                    let mut damaged = false;
                    let mut target_lives = Vec::new();
                    let mut gravity_trace = Vec::new();
                    let mut wind_trace = Vec::new();
                    let mut position_before = Position::default();
                    let generation = campaign.simulation().session.state.live.economy.economy.slot_generation(field);
                    let mut hidden = false;
                    let mut reclaimed = false;
                    for turn in 0..(i32::from(profile.effect_parameter) * 8 + 40) {
                        if !placed {
                            let target_position = if family == Family::GravityWell {
                                let source = &campaign.simulation().entities()[usize::from(field)];
                                crate::world::fields::gravity::pull_is_active(source).then_some(Position {
                                    x: source.position.x.wrapping_add(768), y: source.position.y,
                                    z: source.source_word_2c().wrapping_add(256),
                                })
                            } else {
                                campaign.simulation().economy.authored_model22().iter()
                                    .find(|wind| wind.source_entity_index == field).map(|wind| Position {
                                        z: wind.position.z.saturating_add(1_536), ..wind.axis_position
                                    })
                            };
                            if let Some(position) = target_position {
                                let live = &mut campaign.simulation_mut().session.state.live;
                                for (index, &slot) in targets.iter().enumerate() {
                                    if family == Family::Whirlwind && index > 0 { break; }
                                    // The first creature demonstrates pull; the other real
                                    // bounty-enabled owner rows start within the source's
                                    // contact band so their normal RNG visits can award credit.
                                    let actor_position = if family == Family::GravityWell && index > 0 {
                                        Position { x: position.x.wrapping_sub(704).wrapping_add((index as u16) * 8),
                                            z: position.z.wrapping_sub(192), ..position }
                                    } else { position };
                                    if let Some(actor) = live.economy.economy.authored_class5.iter_mut()
                                        .find(|actor| actor.active && actor.source_entity_index == slot)
                                    {
                                        actor.position = actor_position;
                                    }
                                    let generation = live.economy.economy.slot_generation(slot);
                                    let life = super::super::area_damage::read_arena_life(&live.economy, slot)
                                        .expect("selected real actor has a life carrier");
                                    let (bounty, _) = super::super::area_damage::source_bounty(&live.economy, None, slot)
                                        .expect("selected real actor has a bounty carrier");
                                    target_lives.push((slot, generation, life, bounty));
                                    let row = &mut live.economy.entities[usize::from(slot)];
                                    row.set_position(actor_position);
                                    crate::world::effects::sync_position(&mut live.economy, &mut live.terrain, slot, actor_position, true, true)
                                        .expect("controlled target placement keeps the source map chain");
                                }
                                position_before = position;
                                placed = true;
                            }
                        }
                        let prior_phase = campaign.simulation().economy.authored_model22().iter()
                            .find(|wind| wind.source_entity_index == field)
                            .map(|wind| wind.phase);
                        campaign.step(PlayerCommand::default(), 0);
                        let session = campaign.simulation();
                        assert!(!session.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })), "natural family {id} expiry: {:?}", session.dispatch_events());
                        if let Some(before) = &original_terrain && !terrain_changed {
                            let center_x = ((u32::from(field_position.x) + 128) >> 8) as u8;
                            let center_y = ((u32::from(field_position.y) + 128) >> 8) as u8;
                            'samples: for y in 0..30_u8 {
                                for x in 0..30_u8 {
                                    let sample = Position {
                                        x: u16::from(center_x.wrapping_sub(15).wrapping_add(x)) << 8,
                                        y: u16::from(center_y.wrapping_sub(15).wrapping_add(y)) << 8,
                                        z: 0,
                                    };
                                    if before.ground_height(sample) != session.canonical_terrain().ground_height(sample) {
                                        terrain_changed = true;
                                        break 'samples;
                                    }
                                }
                            }
                        }
                        let arena = &session.session.state.live.economy;
                        if placed {
                            if family == Family::GravityWell {
                                for &(slot, generation, initial_life, initial_bounty) in &target_lives {
                                    if arena.economy.slot_generation(slot) != generation { continue; }
                                    let row = &arena.entities[usize::from(slot)];
                                    contact |= row.control_flags & 0x0010_0000 != 0;
                                    if let Ok((bounty, attacker)) = super::super::area_damage::source_bounty(arena, None, slot) {
                                        credited |= bounty > initial_bounty && attacker == owner_id;
                                    }
                                    let life = super::super::area_damage::read_arena_life(arena, slot).unwrap_or(row.life);
                                    let source_damage = life < initial_life && row.word_26() == owner_id;
                                    damaged |= source_damage;
                                    credited |= source_damage;
                                }
                                if gravity_trace.len() < 24 {
                                    let source = &arena.entities[usize::from(field)];
                                    let actors = target_lives.iter().take(4).map(|&(slot, _, _, _)| {
                                        let row = &arena.entities[usize::from(slot)];
                                        (slot, row.active, row.position, row.control_flags, row.life,
                                            super::super::area_damage::source_bounty(arena, None, slot).ok())
                                    }).collect::<Vec<_>>();
                                    gravity_trace.push((turn, source.action, source.record[74], source.position,
                                        source.source_word_2c(), source.life, actors,
                                        session.shared_spells().spell_experience()[usize::from(id)]));
                                }
                            }
                            let target_row = &arena.entities[usize::from(target)];
                            let typed_target = arena.authored_class5().iter()
                                .find(|actor| actor.source_entity_index == target);
                            if family == Family::Whirlwind {
                                let (_, generation, initial_life, initial_bounty) = target_lives[0];
                                if arena.economy.slot_generation(target) == generation {
                                    if let Ok((bounty, attacker)) = super::super::area_damage::source_bounty(arena, None, target) {
                                        credited |= bounty > initial_bounty && attacker == owner_id;
                                    }
                                    if let Some(actor) = typed_target {
                                        let source_damage = actor.life < initial_life && actor.word_0x26 == owner_id;
                                        damaged |= source_damage;
                                        credited |= source_damage;
                                    }
                                }
                                if wind_trace.len() < 24 {
                                    let actor = typed_target.map(|actor| (actor.active, actor.position, actor.life, actor.word_0x26));
                                    let state = arena.economy.authored_fireflies().iter()
                                        .find(|actor| actor.source_entity_index == target)
                                        .map(|actor| actor.control_flags);
                                    let wind = arena.authored_model22().iter()
                                        .find(|wind| wind.source_entity_index == field)
                                        .map(|wind| (wind.phase, wind.position, wind.axis_position, wind.life));
                                    wind_trace.push((turn, wind, actor, state,
                                        super::super::area_damage::source_bounty(arena, None, target).ok(),
                                        session.shared_spells().spell_experience()[usize::from(id)]));
                                }
                            }
                            let position = typed_target.filter(|actor| actor.active)
                                .map(|actor| actor.position)
                                .or_else(|| target_row.active.then_some(target_row.position));
                            if let Some(position) = position {
                                if family == Family::GravityWell {
                                    let source = &arena.entities[usize::from(field)];
                                    let distance_sq = |position: Position| {
                                        let dx = i32::from(position.x.wrapping_sub(source.position.x) as i16);
                                        let dy = i32::from(position.y.wrapping_sub(source.position.y) as i16);
                                        dx * dx + dy * dy
                                    };
                                    moved |= distance_sq(position) < distance_sq(position_before);
                                } else {
                                    moved |= position != position_before;
                                }
                            }
                        }
                        let row = &arena.entities[usize::from(field)];
                        if arena.economy.slot_generation(field) != generation || !row.active {
                            reclaimed = true;
                            break;
                        }
                        if family == Family::Whirlwind {
                            // Wind's live typed carrier is the source of its
                            // compatibility observation; the allocated raw row
                            // is retired at the following pass boundary.
                            let wind = arena.authored_model22().iter().find(|wind| wind.source_entity_index == field)
                                .expect("live Wind generation retains its authoritative carrier");
                            if wind.control_flags & 0x400 != 0 {
                                assert!(row.active && arena.map_chains.is_linked(field));
                                assert_eq!(wind.phase, prior_phase.unwrap().wrapping_add(1), "Wind hide still runs the source same-pass tail");
                                let published = session.field_source_rows().into_iter()
                                    .find(|source| source.index == field)
                                    .expect("Wind observation retains the hidden source row until reclaim");
                                assert!(published.active && published.control_flags & 0x400 != 0);
                                assert_eq!(published.phase, wind.phase);
                                hidden = true;
                            }
                        } else {
                            hidden |= row.control_flags & 0x400 != 0;
                        }
                    }
                    assert!(hidden, "naturally expiring family {id} reaches its hide callback");
                    assert!(reclaimed, "following source boundary reclaims the family {id} field");
                    assert!(placed && moved, "natural family {id} field moves its real owner-data creature");
                    if family == Family::GravityWell {
                        assert!(terrain_changed, "natural Gravity field changes a real flight-world ground query");
                        assert!(contact, "natural Gravity field captures a real eligible creature: {gravity_trace:?}");
                        assert!(credited, "natural Gravity contact credits real creature bounty to the caster: {gravity_trace:?}");
                        assert!(damaged, "real Gravity bounty reaches the creature's damage consumer: {gravity_trace:?}");
                        assert!(campaign.simulation().session.state.live.shared_spells.spell_experience()[usize::from(id)] > experience_before,
                            "natural Gravity contact awards the actual caster's experience");
                    }
                    if family == Family::Whirlwind {
                        assert!(campaign.simulation().session.state.live.shared_spells.spell_experience()[usize::from(id)] > experience_before,
                            "natural Wind contact awards the actual caster's experience: {wind_trace:?}");
                        assert!(credited, "natural Wind contact reaches the real creature's owner-attributed damage mailbox or consumed damage: {wind_trace:?}");
                        assert!(damaged, "the ordinary creature consumer applies the real Wind damage: {wind_trace:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn ordinary_casts_reach_gravity_and_wind_authorities_for_all_tiers() {
        use crate::native_world::{NativeWorld, gravity_field};

        let inspection =
            NativeWorld::inspect_level2(crate::native_world::tests::level_two_fixture_path())
                .expect("qualified Level 2 native fixture");
        let base = inspection
            .level2_reset_session()
            .expect("ordinary Level 2 reset session");
        let particles = inspection.presentation().particles.clone();
        let parameters = inspection.presentation().entity_parameters.as_ref();

        for (family, family_id, tiers) in [
            (Family::GravityWell, 20, GRAVITY_WELL_TIERS),
            (Family::Whirlwind, 21, WHIRLWIND_TIERS),
        ] {
            for (tier, profile) in tiers.into_iter().enumerate() {
                let mut session = base.clone();
                let (actor_slot, actor_position, actor_flags_before, actor_yaw_before) = {
                    let actor = session
                        .ordinary
                        .villagers
                        .villagers()
                        .iter()
                        .find(|actor| {
                            actor.source_retained()
                                && actor.life >= 0
                                && actor.control_flags & 0x0010_0821 == 0
                                && !matches!(actor.action, 180 | 232)
                                && !matches!(actor.model, 10 | 15 | 18 | 22 | 27 | 28)
                        })
                        .expect("fixture has a live ordinary villager accepted by both fields");
                    (
                        actor.entity_index,
                        actor.position,
                        actor.control_flags,
                        actor.yaw,
                    )
                };

                {
                    let live = &mut session.session.state.live;
                    live.shared_spells
                        .materialize_carried_spell_families(
                            &mut live.economy.economy,
                            &[(family_id, tier as u8)],
                        )
                        .expect("source carry constructor materializes the typed holder");
                }
                let holder_slot = session.shared_spells.local_player.enabled_spell_holders
                    [usize::from(family_id)];
                let mut holder = session
                    .shared_spells
                    .pickup(holder_slot)
                    .expect("acquired typed holder remains live")
                    .clone();
                holder.active_value = holder.activation_value as i16;
                holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
                *session
                    .shared_spells
                    .pickup_mut(holder_slot)
                    .expect("holder remains in the shared spell authority") = holder.clone();
                session.economy.entities[usize::from(holder_slot)] = holder;

                let caster_slot = session
                    .shared_spells
                    .pickup(holder_slot)
                    .expect("cast holder remains live")
                    .source_spell_parent_entity_index();
                session.economy.entities[usize::from(caster_slot)].set_mana(profile.mana_cost);
                let input = CastInput {
                    aim_yaw: 0,
                    aim_pitch: 0,
                    auxiliary_byte: 0,
                    castle_mana: Some(profile.castle_mana_required),
                };
                let cast = {
                    let live = &mut session.session.state.live;
                    step_holder(
                        &mut live.shared_spells,
                        &mut live.economy,
                        &mut live.terrain,
                        &particles,
                        holder_slot,
                        input,
                    )
                    .expect("ordinary active holder emits its source projectile")
                };
                let projectile_slot = cast.projectile.expect("successful cast allocates flyer");
                let projectile_power = match family {
                    Family::GravityWell => profile.power,
                    Family::Whirlwind if profile.effect_parameter == 0 => profile.power,
                    Family::Whirlwind => profile.power / u16::from(profile.effect_parameter),
                };
                let projectile = &session.economy.entities[usize::from(projectile_slot)];
                assert_eq!(
                    (projectile.class, projectile.model, projectile.action),
                    (9, family.projectile_model(), family.projectile_action())
                );
                assert_eq!(projectile.subspell_index, projectile_power);
                assert_eq!(projectile.record[74], profile.effect_parameter);
                assert_eq!(word(projectile, 42), holder_slot);

                // Use an actual retained actor as the terminal destination;
                // preserve the source map chain when relocating the flyer.
                {
                    let live = &mut session.session.state.live;
                    let projectile = &mut live.economy.entities[usize::from(projectile_slot)];
                    projectile.set_position(actor_position);
                    projectile.set_actual_speed(0);
                    projectile.set_life(0);
                    crate::world::effects::sync_position(
                        &mut live.economy,
                        &mut live.terrain,
                        projectile_slot,
                        actor_position,
                        true,
                        true,
                    )
                    .expect("moved test flyer is relinked in the actual source arena");
                }
                let impact = {
                    let live = &mut session.session.state.live;
                    step_projectile(
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
                    .expect("guided source callback constructs the terminal field")
                }
                .impact_slot
                .expect("terminal field allocation succeeds");
                let lifetime = if family == Family::GravityWell { i32::from(profile.effect_parameter) } else { i32::from(profile.effect_parameter) * 8 };
                let terminal = &session.economy.entities[usize::from(impact)];
                assert_eq!(
                    (terminal.class, terminal.model, terminal.action),
                    (
                        10,
                        family.terminal_model(),
                        match family {
                            Family::GravityWell => 72,
                            Family::Whirlwind => 22,
                        },
                    )
                );
                assert_eq!(terminal.subspell_index, projectile_power);
                assert_eq!((terminal.max_life, terminal.life),
                    (if family == Family::GravityWell { 300 } else { lifetime }, lifetime));
                if family == Family::GravityWell {
                    assert_eq!(terminal.record[74], 0, "sub_677A0 clears the child phase before its first visit");
                }

                match family {
                    Family::GravityWell => {
                        // A valid late rise phase emits the real Gravity actor
                        // pull through the same adapter used by simulation.
                        {
                            let live = &mut session.session.state.live;
                            let source = &mut live.economy.entities[usize::from(impact)];
                            source.record[74] = 2;
                            source.set_source_dword_10(5);
                        }
                        let live = &mut session.session.state.live;
                        gravity_field::step(
                            &mut gravity_field::Context {
                                arena: &mut live.economy,
                                terrain: &mut live.terrain,
                                goats: &mut live.goats,
                                ordinary: &mut live.ordinary,
                                player: &mut live.core.state,
                                players: &mut live.core.players,
                                progression: &mut live.shared_spells,
                                fire: live.core.context.spell_fire.as_mut(),
                                parameters,
                            },
                            impact,
                        )
                        .expect("Gravity authority consumes the new impact");
                        let actor = live
                            .ordinary
                            .villagers
                            .villagers()
                            .iter()
                            .find(|actor| actor.entity_index == actor_slot)
                            .expect("Gravity retains the impacted actor carrier");
                        assert_ne!(
                            actor.control_flags & 0x0010_0000,
                            actor_flags_before & 0x0010_0000,
                            "Gravity capture reaches the actual villager authority"
                        );
                    }
                    Family::Whirlwind => {
                        let source_before = session
                            .economy
                            .authored_model22()
                            .iter()
                            .find(|source| source.source_entity_index == impact)
                            .copied()
                            .expect("impact is registered with the existing Wind authority");
                        let segments = session
                            .economy
                            .model22_spawned_children()
                            .iter()
                            .filter(|child| child.active && child.parent_entity_index == impact)
                            .collect::<Vec<_>>();
                        assert_eq!(
                            segments.len(),
                            11,
                            "ordinary Whirlwind impact constructs the source linked chain"
                        );
                        assert!(
                            segments
                                .iter()
                                .all(|child| (child.model, child.action) == (75, 82))
                        );
                        assert_eq!(source_before.subspell_index, projectile_power);
                        assert_eq!(
                            (source_before.max_life, source_before.life),
                            (lifetime as i16, lifetime as i16)
                        );
                        {
                            let live = &mut session.session.state.live;
                            assert!(
                                live.economy
                                    .step_authored_model22_slot(impact, &live.terrain)
                            );
                        }
                        session
                            .step_level2_authored_model22_sub33340_eligible_targets(impact)
                            .expect("Wind dispatch consumes the actual affected actor");
                        let source_after = session
                            .economy
                            .authored_model22()
                            .iter()
                            .find(|source| source.source_entity_index == impact)
                            .copied()
                            .expect("live Wind authority retains its source slot");
                        assert_eq!(source_after.life, lifetime as i16 - 1);
                        let actor = session
                            .ordinary
                            .villagers
                            .villagers()
                            .iter()
                            .find(|actor| actor.entity_index == actor_slot)
                            .expect("Wind retains the impacted actor carrier");
                        assert!(
                            actor.control_flags != actor_flags_before
                                || actor.position != actor_position
                                || actor.yaw != actor_yaw_before,
                            "Wind motion reaches the actual villager authority"
                        );
                    }
                }
            }
        }

        let mut exhausted = base;
        {
            let live = &mut exhausted.session.state.live;
            live.shared_spells
                .materialize_carried_spell_families(&mut live.economy.economy, &[(20, 0)])
                .expect("failed-allocation case has a real acquired Gravity holder");
        }
        let holder_slot = exhausted.shared_spells.local_player.enabled_spell_holders[20];
        let mut holder = exhausted
            .shared_spells
            .pickup(holder_slot)
            .expect("allocation-negative holder is live")
            .clone();
        holder.active_value = holder.activation_value as i16;
        holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
        *exhausted
            .shared_spells
            .pickup_mut(holder_slot)
            .expect("allocation-negative holder stays shared") = holder.clone();
        exhausted.economy.entities[usize::from(holder_slot)] = holder;
        let caster_slot = exhausted
            .shared_spells
            .pickup(holder_slot)
            .expect("allocation-negative cast holder remains live")
            .source_spell_parent_entity_index();
        exhausted.economy.entities[usize::from(caster_slot)]
            .set_mana(GRAVITY_WELL_TIERS[0].mana_cost);
        let regen_before = exhausted.economy.entities[usize::from(caster_slot)].mana_regen;
        let expected_regen = if regen_before >= 0 {
            -GRAVITY_WELL_TIERS[0].mana_cost
        } else {
            regen_before.wrapping_sub(GRAVITY_WELL_TIERS[0].mana_cost)
        };
        exhausted.economy.economy.free_slots.clear();
        let rng_before = exhausted.economy.economy.source_rng.clone();
        let projectiles_before = exhausted
            .economy
            .entities
            .iter()
            .filter(|row| {
                row.active
                    && matches!(
                        (row.class, row.model, row.action),
                        (9, 22, 23) | (9, 26, 27)
                    )
            })
            .count();
        let failed = {
            let live = &mut exhausted.session.state.live;
            step_holder(
                &mut live.shared_spells,
                &mut live.economy,
                &mut live.terrain,
                &particles,
                holder_slot,
                CastInput {
                    aim_yaw: 0,
                    aim_pitch: 0,
                    auxiliary_byte: 0,
                    castle_mana: Some(GRAVITY_WELL_TIERS[0].castle_mana_required),
                },
            )
            .expect("source holder handles exhausted-arena allocation")
        };
        assert_eq!(failed.projectile, None);
        assert_eq!(exhausted.economy.economy.source_rng, rng_before);
        assert_eq!(
            exhausted.economy.entities[usize::from(caster_slot)].mana_regen,
            expected_regen,
            "the source cast charge still applies when NewEvent allocation fails"
        );
        assert_eq!(
            exhausted
                .economy
                .entities
                .iter()
                .filter(|row| {
                    row.active
                        && matches!(
                            (row.class, row.model, row.action),
                            (9, 22, 23) | (9, 26, 27)
                        )
                })
                .count(),
            projectiles_before,
            "failed allocation emits no stale or fake flyer"
        );
    }

    #[test]
    fn ordinary_casts_reach_scheduled_gravity_and_wind_actor_consumers() {
        use crate::PlayerCommand;
        use crate::native_world::{Mc2DispatchEvent, NativeWorld};

        let gameplay_cache = std::env::var_os("MC_LEVEL2_GAMEPLAY_CACHE_V5").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/level2-day-owner-cache/components/gameplay")
            },
            std::path::PathBuf::from,
        );
        let inspection = NativeWorld::inspect_level2_with_owner_cache(
            crate::native_world::tests::level_two_fixture_path(),
            gameplay_cache,
        )
        .expect("qualified Level 2 native fixture and owner gameplay cache");
        let base = inspection
            .level2_reset_session()
            .expect("ordinary Level 2 reset session");
        let no_dispatch_errors = |session: &crate::native_world::Mc2Simulation| {
            let errors = session
                .dispatch_events()
                .iter()
                .filter(|event| matches!(event, Mc2DispatchEvent::Error { .. }))
                .collect::<Vec<_>>();
            assert!(
                errors.is_empty(),
                "scheduled field dispatch errors: {errors:?}"
            );
        };
        let place_villager = |session: &mut crate::native_world::Mc2Simulation,
                              actor_slot: u16,
                              position: Position| {
            let live = &mut session.session.state.live;
            let villager = live
                .ordinary
                .villagers
                .villagers_mut()
                .iter_mut()
                .find(|actor| actor.entity_index == actor_slot)
                .expect("selected actor remains in the actual Villager authority");
            villager.position = position;
            live.economy.entities[usize::from(actor_slot)].set_position(position);
            crate::world::effects::sync_position(
                &mut live.economy,
                &mut live.terrain,
                actor_slot,
                position,
                true,
                true,
            )
            .expect("test actor relocation keeps its source map chain current");
        };

        for (family, family_id, tiers) in [
            (Family::GravityWell, 20, GRAVITY_WELL_TIERS),
            (Family::Whirlwind, 21, WHIRLWIND_TIERS),
        ] {
            let profile = tiers[0];
            let mut session = base.clone();
            let resident_slot = if family == Family::Whirlwind {
                let house = session
                    .ordinary()
                    .houses()
                    .iter()
                    .find(|house| {
                        house.active && house.parameter.produces_villagers() && house.capacity > 5
                    })
                    .expect("fixture retains a resident-producing source House")
                    .entity_index;
                // Pin an ordinary full-House production visit with a source
                // RNG value that selects a settlement resident, not an Archer.
                {
                    let live = &mut session.session.state.live;
                    let house = live.ordinary.houses.house_mut(house).unwrap();
                    let capacity = house.capacity as u16;
                    let threshold = i32::from(capacity) - (i32::from(capacity) >> 4) - 2;
                    let seed = (0_u16..=u16::MAX)
                        .find(|seed| {
                            let first = seed.wrapping_mul(9_377).wrapping_add(9_439);
                            let second = first.wrapping_mul(9_377).wrapping_add(9_439);
                            i32::from(first % capacity) > threshold && second % 12 >= 2
                        })
                        .expect("source RNG has a resident-producing draw");
                    house.occupants = i32::from(capacity);
                    house.phase = 0;
                    house.rng = seed;
                    house.publish_source(
                        &mut live.economy.entities[usize::from(house.entity_index)],
                    );
                }
                session.step_outer_turn(PlayerCommand::default(), 0);
                no_dispatch_errors(&session);
                Some(
                    session
                        .economy()
                        .house_residents()
                        .iter()
                        .find(|resident| resident.parent_house_entity_index == house)
                        .expect("ordinary House callback constructs its resident")
                        .source_entity_index,
                )
            } else {
                None
            };
            let actor_slot = session
                .ordinary
                .villagers
                .villagers()
                .iter()
                .find(|actor| {
                    actor.source_retained()
                        && actor.life >= 0
                        && actor.control_flags & 0x0010_0821 == 0
                        && !matches!(actor.action, 180 | 232)
                        && !matches!(actor.model, 10 | 15 | 18 | 22 | 27 | 28)
                })
                .expect("fixture has a live Villager admitted by both source fields")
                .entity_index;
            let actor_generation = session.session.state.live.economy.economy.slot_generation(actor_slot);
            let original_model = session.ordinary.villagers.villagers().iter()
                .find(|actor| actor.entity_index == actor_slot).unwrap().model;
            let owner = session.session.state.live.core.state.entity_index;
            let caster_position = session.session.state.live.core.state.position;
            session
                .apply_carried_spell_families(&[family_id], Some(family_id))
                .expect("campaign carry publishes the scheduled field holder");
            {
                let live = &mut session.session.state.live;
                let mut player = live.shared_spells.player_entity.clone();
                player.set_mana(profile.mana_cost);
                player.set_mana_regen(0);
                player.max_mana = player.max_mana.max(profile.mana_cost);
                player.record[144..148].copy_from_slice(&player.max_mana.to_le_bytes());
                live.shared_spells
                    .sync_player_entity_from_authority(&player);
                live.shared_spells.caster.mana = profile.mana_cost;
                live.shared_spells.player_mana = profile.mana_cost;
                live.economy.entities[usize::from(owner)] = player;
            }
            let castle = session
                .economy_mut()
                .build_castle(caster_position, owner)
                .expect("actual caster Castle supports the source admission gate");
            assert!(
                session
                    .economy_mut()
                    .set_castle_mana_prerequisite_for_test(castle, profile.castle_mana_required)
            );
            let holder_slot = session
                .session
                .state
                .live
                .shared_spells
                .local_player
                .enabled_spell_holders[usize::from(family_id)];
            assert_ne!(holder_slot, 0, "source carry owns the ordinary holder");
            let prior_terminal_slots = session
                .economy
                .entities
                .iter()
                .filter(|row| row.active && row.class == 10 && row.model == family.terminal_model())
                .map(|row| row.index)
                .collect::<Vec<_>>();

            let cast = PlayerCommand {
                opcode: 6,
                argument_1: i8::try_from(family_id).expect("spell family fits source input"),
                flags: 0x40,
                ..PlayerCommand::default()
            };
            let mut projectile_slot = None;
            for turn in 0..4 {
                session.step_outer_turn(
                    if turn == 0 {
                        cast
                    } else {
                        PlayerCommand::default()
                    },
                    0,
                );
                no_dispatch_errors(&session);
                projectile_slot = session
                    .economy
                    .entities
                    .iter()
                    .find(|row| {
                        row.active
                            && (row.class, row.model, row.action)
                                == (9, family.projectile_model(), family.projectile_action())
                            && word(row, 42) == holder_slot
                    })
                    .map(|row| row.index);
                if projectile_slot.is_some() {
                    break;
                }
            }
            let projectile_slot = projectile_slot.unwrap_or_else(|| {
                let live = &session.session.state.live;
                let holder = live.shared_spells.pickup(holder_slot);
                let player = &live.economy.entities[usize::from(owner)];
                panic!(
                    "ordinary PlayerCommand cast reaches its scheduled flyer: family={family_id}, holder={holder:?}, player_mana={}, player_regen={}, castle={:?}, owner_max_mana={:?}, fire={:?}, dispatch={:?}",
                    player.mana,
                    player.mana_regen,
                    live.economy.castle_for_owner(owner),
                    live.economy.owner_max_mana.get(&owner),
                    live.core.context.spell_fire,
                    session.dispatch_events(),
                );
            });
            let projectile_power = if family == Family::Whirlwind {
                profile.power / u16::from(profile.effect_parameter)
            } else {
                profile.power
            };
            let projectile = &session.economy.entities[usize::from(projectile_slot)];
            assert_eq!(projectile.subspell_index, projectile_power);
            assert_eq!(projectile.record[74], profile.effect_parameter);
            assert!(
                session
                    .session
                    .state
                    .live
                    .shared_spells
                    .player_entity
                    .mana_regen
                    < 0
                    || session.session.state.live.shared_spells.player_entity.mana
                        < profile.mana_cost,
                "the ordinary cast applies its source charge"
            );
            let holder = session
                .session
                .state
                .live
                .shared_spells
                .pickup(holder_slot)
                .expect("cast holder remains in the shared spell authority");
            // sub_5F7B0 starts the cast in word 0x2e from word 0x30.
            // The holder's independent word 0x36 only counts down if it was
            // already nonzero; the cast does not initialise it.
            assert_eq!(holder.activation_value, profile.cooldown);
            assert!(
                holder.active_value > 0
                    && holder.active_value < i16::try_from(profile.cooldown).unwrap(),
                "the ordinary holder advances its source active countdown"
            );
            assert_eq!(
                i16::from_le_bytes([holder.record[50], holder.record[51]]),
                holder.active_value
            );

            let actor_position = session
                .ordinary
                .villagers
                .villagers()
                .iter()
                .find(|actor| actor.entity_index == actor_slot)
                .expect("cast target remains in the Villager authority")
                .position;
            // Keep the ordinary flyer and scheduled guided callback; pin the
            // terminal visit so actor checks do not depend on fixture geometry.
            {
                let live = &mut session.session.state.live;
                let projectile = &mut live.economy.entities[usize::from(projectile_slot)];
                projectile.set_position(actor_position);
                projectile.set_actual_speed(0);
                projectile.set_life(0);
                projectile.set_control_flags(projectile.control_flags | 2);
                projectile.record[154..156].fill(0);
                crate::world::effects::sync_position(
                    &mut live.economy,
                    &mut live.terrain,
                    projectile_slot,
                    actor_position,
                    true,
                    true,
                )
                .expect("real flyer keeps its source map chain after deterministic impact setup");
            }
            session.step_outer_turn(PlayerCommand::default(), 0);
            no_dispatch_errors(&session);
            let lifetime = if family == Family::GravityWell { i32::from(profile.effect_parameter) } else { i32::from(profile.effect_parameter) * 8 };
            let impact_slot = session
                .economy
                .entities
                .iter()
                .find(|row| {
                    row.active
                        && row.class == 10
                        && row.model == family.terminal_model()
                        && row.source_id == owner
                        && row.subspell_index == projectile_power
                        && row.max_life == if family == Family::GravityWell { 300 } else { lifetime }
                        && !prior_terminal_slots.contains(&row.index)
                })
                .map(|row| row.index)
                .expect("guided callback creates its source field through the outer turn");

            match family {
                Family::GravityWell => {
                    assert_eq!(
                        session.economy.entities[usize::from(impact_slot)].action,
                        72
                    );
                    assert!(session.economy.entities[usize::from(impact_slot)].record[74] <= 2,
                        "sub_677A0 starts phase zero; a same-pass visit may advance the rise");
                    // Natural action72 visits the crater phases before its
                    // ordinary expiry transitions to action74.
                    for _ in 0..=lifetime {
                        if session.economy.entities[usize::from(impact_slot)].action == 74 {
                            break;
                        }
                        session.step_outer_turn(PlayerCommand::default(), 0);
                        no_dispatch_errors(&session);
                    }
                    assert_eq!(
                        session.economy.entities[usize::from(impact_slot)].action,
                        74
                    );
                    // A source-correct rise can kill and reclaim the original
                    // Villager before the action74 pull. Reuse it only while
                    // it is still eligible; otherwise prove the original
                    // generation is no longer live and select another real
                    // eligible Villager for the same scheduled callback.
                    let eligible = |actor: &&crate::villager::VillagerState| {
                        actor.active && actor.life > 0 && actor.control_flags & 0x0010_0c21 == 0
                            && !matches!(actor.action, 180 | 232)
                            && !matches!(actor.model, 10 | 15 | 18 | 22 | 27 | 28)
                    };
                    let original = session.ordinary.villagers.villagers().iter()
                        .find(|actor| actor.entity_index == actor_slot);
                    let actor_slot = if original.as_ref().is_some_and(eligible) {
                        actor_slot
                    } else {
                        let live = &session.session.state.live;
                        let view = super::super::source_target::read(
                            &live.economy,
                            super::super::source_target::Context::from_ordinary(&live.goats, &live.ordinary),
                            actor_slot,
                        ).expect("original Villager current source address resolves");
                        assert!(live.economy.economy.slot_generation(actor_slot) != actor_generation
                            || view.is_none_or(|actor| !actor.active || actor.life <= 0 || actor.drawing_disabled()
                                || actor.class != 5 || actor.model != original_model),
                            "original Villager left its authority while its original source remains live: {view:?}");
                        session.ordinary.villagers.villagers().iter().filter(eligible)
                            .find(|actor| actor.entity_index != actor_slot)
                            .unwrap_or_else(|| panic!("Gravity action74 needs another real eligible Villager; original={original:?} source={view:?}; remaining={:?}", session.ordinary.villagers.villagers()))
                            .entity_index
                    };
                    let gravity = &session.economy.entities[usize::from(impact_slot)];
                    let field_position = gravity.position;
                    let target_position = Position {
                        x: field_position.x.wrapping_add(768),
                        y: field_position.y,
                        z: gravity.source_word_2c().wrapping_add(256),
                    };
                    place_villager(&mut session, actor_slot, target_position);
                    let position_before = session
                        .ordinary
                        .villagers
                        .villagers()
                        .iter()
                        .find(|actor| actor.entity_index == actor_slot)
                        .expect("Gravity target remains live")
                        .position;
                    let first_shrink_tail_before =
                        session.economy.entities[usize::from(impact_slot)].phase;
                    session.step_outer_turn(PlayerCommand::default(), 0);
                    no_dispatch_errors(&session);
                    let actor = session
                        .ordinary
                        .villagers
                        .villagers()
                        .iter()
                        .find(|actor| actor.entity_index == actor_slot)
                        .expect("scheduled Gravity pull writes the actual Villager carrier");
                    assert_ne!(
                        actor.position, position_before,
                        "ordinary Gravity expiry pulls the actual Villager carrier"
                    );
                    assert_eq!(
                        session.economy.entities[usize::from(impact_slot)].phase,
                        first_shrink_tail_before.wrapping_add(1),
                        "Gravity action74 advances byte 0x3e in the same pass"
                    );

                    let mut hidden_tail = None;
                    for _ in 0..=18 {
                        let field = &session.economy.entities[usize::from(impact_slot)];
                        if field.control_flags & 0x400 != 0 {
                            break;
                        }
                        let byte_3e_before = field.phase;
                        session.step_outer_turn(PlayerCommand::default(), 0);
                        no_dispatch_errors(&session);
                        let field = &session.economy.entities[usize::from(impact_slot)];
                        if field.control_flags & 0x400 != 0 {
                            assert_eq!(
                                field.phase,
                                byte_3e_before.wrapping_add(1),
                                "Gravity hide callback advances byte 0x3e in the same pass"
                            );
                            hidden_tail = Some(field.phase);
                            break;
                        }
                    }
                    assert!(
                        hidden_tail.is_some(),
                        "ordinary Gravity expiry completes its scheduled sixteen-pass shrink"
                    );
                    let hidden = &session.economy.entities[usize::from(impact_slot)];
                    assert!(hidden.active, "hidden Gravity waits for next-pass reclaim");
                    assert!(session.economy.map_chains.is_linked(impact_slot));

                    session.step_outer_turn(PlayerCommand::default(), 0);
                    no_dispatch_errors(&session);
                    assert!(
                        !session.economy.entities[usize::from(impact_slot)].active
                            || (
                                session.economy.entities[usize::from(impact_slot)].class,
                                session.economy.entities[usize::from(impact_slot)].model
                            ) != (10, 67),
                        "following source boundary reclaims the hidden Gravity field"
                    );
                    assert!(!session.economy.map_chains.is_linked(impact_slot));
                }
                Family::Whirlwind => {
                    let wind = session
                        .economy
                        .authored_model22()
                        .iter()
                        .find(|source| source.source_entity_index == impact_slot)
                        .copied()
                        .expect("ordinary impact registers the existing Wind authority");
                    assert_eq!(
                        session
                            .economy
                            .model22_spawned_children()
                            .iter()
                            .filter(|child| child.active && child.parent_entity_index == impact_slot)
                            .count(),
                        11,
                        "ordinary impact constructs the source linked wind chain"
                    );
                    place_villager(&mut session, actor_slot, wind.axis_position);
                    let resident_slot = resident_slot.expect("Wind test has a House resident");
                    {
                        let live = &mut session.session.state.live;
                        let resident = live
                            .economy
                            .economy
                            .house_residents
                            .iter_mut()
                            .find(|resident| {
                                resident.active && resident.source_entity_index == resident_slot
                            })
                            .expect("House-produced resident remains authoritative");
                        resident.position = wind.axis_position;
                        resident.control_flags |= 0x1000_0000;
                        resident.stage.duration = 1_536;
                        let row = &mut live.economy.entities[usize::from(resident_slot)];
                        row.set_position(wind.axis_position);
                        row.set_control_flags(row.control_flags | 0x1000_0000);
                        // A stale shadow must not replace the current typed
                        // source word30 when Wind reads its target.
                        row.record[52..54].fill(0);
                        crate::world::effects::sync_position(
                            &mut live.economy,
                            &mut live.terrain,
                            resident_slot,
                            wind.axis_position,
                            true,
                            false,
                        )
                        .expect("House resident retains the source map head");
                    }
                    // An airborne target takes the source contact route, which
                    // also awards Wind experience to the actual spell owner.
                    {
                        let live = &mut session.session.state.live;
                        let actor = live
                            .ordinary
                            .villagers
                            .villagers_mut()
                            .iter_mut()
                            .find(|actor| actor.entity_index == actor_slot)
                            .expect("Wind target remains in the Villager authority");
                        actor.control_flags |= 0x1000_0000;
                        let source = &mut live.economy.entities[usize::from(actor_slot)];
                        source.set_control_flags(source.control_flags | 0x1000_0000);
                    }
                    let experience_before =
                        session.session.state.live.shared_spells.spell_experience()[21];
                    let mut replay = session.clone();
                    replay
                        .session
                        .state
                        .live
                        .shared_spells
                        .set_mission_rewards(crate::campaign::rewards::MissionRewards::Suppressed);
                    let wind_target_before = session
                        .ordinary
                        .villagers
                        .villagers()
                        .iter()
                        .find(|actor| actor.entity_index == actor_slot)
                        .expect("Wind target remains live before its scheduled force")
                        .position;
                    session.step_outer_turn(PlayerCommand::default(), 0);
                    no_dispatch_errors(&session);
                    replay.step_outer_turn(PlayerCommand::default(), 0);
                    no_dispatch_errors(&replay);
                    assert!(
                        session.session.state.live.shared_spells.spell_experience()[21]
                            > experience_before,
                        "the scheduled airborne contact awards the Wind owner's experience"
                    );
                    assert_eq!(
                        replay.session.state.live.shared_spells.spell_experience()[21],
                        experience_before,
                        "the same scheduled contact on a completed mission suppresses rewards"
                    );
                    let actor = session
                        .ordinary
                        .villagers
                        .villagers()
                        .iter()
                        .find(|actor| actor.entity_index == actor_slot)
                        .expect("scheduled Wind force writes the actual Villager carrier");
                    assert_ne!(actor.control_flags & 0x800, 0);
                    assert_ne!(
                        actor.position, wind_target_before,
                        "ordinary Wind callback moves the actual Villager carrier"
                    );
                    let resident = session
                        .economy()
                        .house_residents()
                        .iter()
                        .find(|resident| {
                            resident.active && resident.source_entity_index == resident_slot
                        })
                        .expect("Wind target remains in the House resident authority");
                    assert_ne!(resident.control_flags & 0x800, 0);
                    assert_ne!(
                        resident.position, wind.axis_position,
                        "ordinary Wind moves the House-produced resident carrier"
                    );
                    assert_eq!(
                        i16::from_le_bytes(
                            session.economy.entities[usize::from(resident_slot)].record[52..54]
                                .try_into()
                                .unwrap()
                        ),
                        resident.stage.duration,
                        "Wind publishes the current typed source word30"
                    );

                    let children = session
                        .economy
                        .model22_spawned_children()
                        .iter()
                        .filter(|child| child.active && child.parent_entity_index == impact_slot)
                        .map(|child| {
                            let slot = child.source_entity_index;
                            (slot, session.economy.economy.slot_generation(slot))
                        })
                        .collect::<Vec<_>>();
                    let byte_3e_before = session
                        .economy
                        .authored_model22()
                        .iter()
                        .find(|source| source.source_entity_index == impact_slot)
                        .expect("Wind phase remains on its typed carrier")
                        .phase;
                    let house_slot = session
                        .ordinary
                        .houses
                        .houses()
                        .iter()
                        .find(|house| house.source_retained())
                        .expect("fixture retains an actual House authority")
                        .entity_index;
                    let cleanup_position = session
                        .economy
                        .authored_model22()
                        .iter()
                        .find(|source| source.source_entity_index == impact_slot)
                        .expect("Wind source remains live before expiry")
                        .position;
                    {
                        let live = &mut session.session.state.live;
                        let house = live.ordinary.houses.house_mut(house_slot).unwrap();
                        house.position = cleanup_position;
                        house.control_flags |= 0x1000_0800;
                        house.publish_source(&mut live.economy.entities[usize::from(house_slot)]);
                        let castle_state = live.economy.economy.castle_at_mut(castle).unwrap();
                        castle_state.position = cleanup_position;
                        castle_state.control_flags |= 0x1000_0800;
                        let castle_row = &mut live.economy.entities[usize::from(castle)];
                        castle_row.set_position(cleanup_position);
                        castle_row.set_control_flags(castle_row.control_flags | 0x1000_0800);
                        for slot in [house_slot, castle] {
                            crate::world::effects::sync_position(
                                &mut live.economy,
                                &mut live.terrain,
                                slot,
                                cleanup_position,
                                true,
                                false,
                            )
                            .expect("expiry targets retain their actual map heads");
                        }
                    }
                    // The next scheduled callback decrements zero to -1 and
                    // exercises the real cleanup branch without waiting a
                    // full Wind lifetime. Natural expiry has a separate check.
                    session
                        .economy_mut()
                        .fields
                        .authored_model22
                        .iter_mut()
                        .find(|source| source.source_entity_index == impact_slot)
                        .expect("Wind source remains in the existing field authority")
                        .life = 0;
                    session.step_outer_turn(PlayerCommand::default(), 0);
                    no_dispatch_errors(&session);
                    let expired = session
                        .economy
                        .authored_model22()
                        .iter()
                        .find(|source| source.source_entity_index == impact_slot)
                        .expect("same-pass Wind expiry retains its carrier until reclaim");
                    assert_ne!(expired.control_flags & 0x400, 0);
                    assert_eq!(
                        expired.phase,
                        byte_3e_before.wrapping_add(1),
                        "same-pass source tail advances byte 0x3e before reclaim"
                    );
                    let actor = session
                        .ordinary
                        .villagers
                        .villagers()
                        .iter()
                        .find(|actor| actor.entity_index == actor_slot)
                        .expect("Wind expiry clears flags on the actual Villager carrier");
                    assert_eq!(actor.control_flags & 0x1000_0800, 0);
                    assert_eq!(
                        session
                            .economy()
                            .house_residents()
                            .iter()
                            .find(|resident| resident.source_entity_index == resident_slot)
                            .expect("House resident survives the Wind expiry callback")
                            .control_flags
                            & 0x1000_0800,
                        0,
                        "expiry clears the House resident's authoritative flags"
                    );
                    assert_eq!(
                        session
                            .ordinary
                            .houses
                            .houses()
                            .iter()
                            .find(|house| house.entity_index == house_slot)
                            .expect("House survives the Wind expiry callback")
                            .control_flags
                            & 0x1000_0800,
                        0,
                        "expiry clears the authoritative House carrier"
                    );
                    assert_eq!(
                        session
                            .economy
                            .economy
                            .castle_at(castle)
                            .expect("Castle survives the Wind expiry callback")
                            .control_flags
                            & 0x1000_0800,
                        0,
                        "expiry clears the authoritative Castle carrier"
                    );
                    session.step_outer_turn(PlayerCommand::default(), 0);
                    no_dispatch_errors(&session);
                    assert!(
                        session
                            .economy
                            .authored_model22()
                            .iter()
                            .all(|source| source.source_entity_index != impact_slot)
                    );
                    assert!(
                        session
                            .economy
                            .model22_spawned_children()
                            .iter()
                            .all(|child| {
                                child.parent_entity_index != impact_slot || !child.active
                            })
                    );
                    let root = &session.economy.entities[usize::from(impact_slot)];
                    assert!(
                        !root.active || (root.class, root.model) != (10, 22),
                        "following source boundary retires the raw Wind root as well"
                    );
                    for (child, generation) in children {
                        let raw = &session.economy.entities[usize::from(child)];
                        let current_generation = session.economy.economy.slot_generation(child);
                        if current_generation == generation {
                            assert!(
                                !session.economy.map_chains.is_linked(child),
                                "retired Wind child generation must be unlinked"
                            );
                        } else if session.economy.map_chains.is_linked(child) {
                            assert!(
                                raw.active
                                    || session
                                        .economy
                                        .economy
                                        .live_arena_cursor_row(child)
                                        .is_some(),
                                "reused map slot {child} must have a live current occupant"
                            );
                            let tile = session.economy.map_chains.tile_of(child);
                            assert_eq!(
                                session.canonical_terrain().planes().entity_indices
                                    [usize::from(tile)] as u16,
                                session.economy.map_chains.tile_head(tile),
                                "the reused occupant publishes its current terrain map head"
                            );
                        }
                        assert!(
                            !raw.active || (raw.class, raw.model) != (10, 75),
                            "following source boundary retires linked raw Wind rows"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn authored_wind_reaches_scheduled_smoke_carriers_and_reclaims_links() {
        use crate::PlayerCommand;
        use crate::native_world::Mc2DispatchEvent;

        let cache = std::env::var_os("MC_LEVEL2_GAMEPLAY_CACHE_V5").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/level2-day-owner-cache/components/gameplay")
            },
            std::path::PathBuf::from,
        );
        let package = std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
            },
            std::path::PathBuf::from,
        );
        let mut session = crate::campaign::CampaignSession::open_with_owner_cache(
            package,
            cache,
            crate::campaign::spec::CampaignSelection::LEVEL2,
        )
        .expect("retained Level 2 campaign and owner cache")
        .simulation()
        .clone();
        let wind = session
            .allocate_level2_authored_model22(110)
            .expect("authored Wind");
        let eruption = session
            .allocate_level2_authored_model13(365)
            .expect("authored eruption");
        let seed = (1_u16..=u16::MAX)
            .find(|seed| seed.wrapping_mul(9_377).wrapping_add(9_439) % 0x9d >= 79)
            .expect("source eruption can emit smoke");
        {
            let live = &mut session.session.state.live;
            let parent = live
                .economy
                .economy
                .authored_model13
                .iter_mut()
                .find(|row| row.source_entity_index == eruption.source_entity_index)
                .expect("authored eruption stays live");
            parent.position = wind.position;
            parent.life = 2;
            parent.entity_rng = seed;
            crate::world::effects::sync_position(
                &mut live.economy,
                &mut live.terrain,
                eruption.source_entity_index,
                wind.position,
                true,
                false,
            )
            .expect("eruption retains its map head");
        }
        let previous_smoke14: std::collections::HashSet<_> = session
            .economy
            .entities
            .iter()
            .filter(|row| row.active && (row.class, row.model) == (10, 14))
            .map(|row| row.index)
            .collect();
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "eruption visit must not roll back: {:?}",
            session.dispatch_events()
        );
        let smoke14 = session
            .economy
            .entities
            .iter()
            .find(|row| {
                row.active
                    && (row.class, row.model) == (10, 14)
                    && row.source_id == eruption.entity_id
                    && !previous_smoke14.contains(&row.index)
            })
            .expect("scheduled eruption produces source smoke14")
            .index;
        let current = *session
            .economy()
            .authored_model22()
            .iter()
            .find(|row| row.source_entity_index == wind.source_entity_index)
            .expect("authored Wind remains live");
        let smoke13;
        {
            let live = &mut session.session.state.live;
            smoke13 = live
                .economy
                .allocate_smoke13_with_terrain(current.axis_position, &mut live.terrain)
                .expect("source smoke13 constructor");
            let a = live
                .economy
                .economy
                .model59_smoke_children
                .iter_mut()
                .find(|row| row.source_entity_index == smoke13)
                .unwrap();
            a.control_flags |= 0x1000_0000;
            a.source_word_30 = 1_536;
            let b = &mut live.economy.entities[usize::from(smoke14)];
            b.set_position(current.axis_position);
            b.set_control_flags(b.control_flags | 0x1000_0000);
            b.set_source_word_30(1_536);
            crate::world::effects::sync_position(
                &mut live.economy,
                &mut live.terrain,
                smoke14,
                current.axis_position,
                true,
                false,
            )
            .expect("smoke14 keeps its source map head");
        }
        let before13 = session
            .economy()
            .model59_smoke_children()
            .iter()
            .find(|row| row.source_entity_index == smoke13)
            .unwrap()
            .position;
        let before14 = session.economy.entities[usize::from(smoke14)].position;
        let excluded_before = session
            .economy()
            .authored_model13()
            .iter()
            .find(|row| row.source_entity_index == eruption.source_entity_index)
            .unwrap()
            .position;
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "Wind smoke contact must not roll back: {:?}",
            session.dispatch_events()
        );
        let a = session
            .economy()
            .model59_smoke_children()
            .iter()
            .find(|row| row.source_entity_index == smoke13)
            .expect("smoke13 remains allocated");
        let b = &session.economy.entities[usize::from(smoke14)];
        assert!(b.active && (b.class, b.model) == (10, 14));
        for (position, before, flags, damage, owner) in [
            (
                a.position,
                before13,
                a.control_flags,
                a.pending_damage,
                a.pending_attacker_entity_index,
            ),
            (
                b.position,
                before14,
                b.control_flags,
                b.source_wind_spell_payload(),
                b.source_wind_spell_owner(),
            ),
        ] {
            assert_ne!(position, before, "Wind moves the actual smoke carrier");
            assert_ne!(flags & 0x800, 0, "Wind marks its forced-motion visit");
            assert_eq!(
                (damage, owner),
                (i32::from(current.subspell_index), current.entity_id)
            );
        }
        let excluded = session
            .economy()
            .authored_model13()
            .iter()
            .find(|row| row.source_entity_index == eruption.source_entity_index)
            .unwrap();
        assert_eq!(
            (excluded.position.x, excluded.position.y),
            (excluded_before.x, excluded_before.y),
            "source model19 remains excluded"
        );
        let phase = session
            .economy()
            .authored_model22()
            .iter()
            .find(|row| row.source_entity_index == wind.source_entity_index)
            .unwrap()
            .phase;
        session
            .session
            .state
            .live
            .economy
            .economy
            .fields
            .authored_model22
            .iter_mut()
            .find(|row| row.source_entity_index == wind.source_entity_index)
            .unwrap()
            .life = 0;
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        let hidden = session
            .economy()
            .authored_model22()
            .iter()
            .find(|row| row.source_entity_index == wind.source_entity_index)
            .unwrap();
        assert_ne!(hidden.control_flags & 0x400, 0);
        assert_eq!(hidden.phase, phase.wrapping_add(1));
        assert_eq!(
            session
                .economy()
                .model59_smoke_children()
                .iter()
                .find(|row| row.source_entity_index == smoke13)
                .unwrap()
                .control_flags
                & 0x1000_0800,
            0
        );
        assert_eq!(
            session.economy.entities[usize::from(smoke14)].control_flags & 0x1000_0800,
            0
        );
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        assert!(
            session
                .economy()
                .authored_model22()
                .iter()
                .all(|row| row.source_entity_index != wind.source_entity_index)
        );
        assert!(
            session
                .economy()
                .model22_spawned_children()
                .iter()
                .all(|row| row.parent_entity_index != wind.source_entity_index || !row.active)
        );
    }
}

/// Source guided callback followed by the family-specific `sub_677A0` or
/// `sub_678E0` terminal write. Guided
/// impact already copies the current projectile subspell value onto its field.
/// Shared movement, impact creation, and pass tails remain with their owners.
pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    targets: Option<&super::guided_projectile::Targets<'_>>,
    context: super::source_target::Context<'_>,
) -> Result<super::guided_projectile::StepOutcome, String> {
    let projectile = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active)
        .ok_or("field spell projectile is absent")?
        .clone();
    let family = match (projectile.class, projectile.model, projectile.action) {
        (9, 22, 23) => Family::GravityWell,
        (9, 26, 27) => Family::Whirlwind,
        selector => {
            return Err(format!(
                "field spell projectile has unsupported source selector {selector:?}"
            ));
        }
    };
    let outcome = super::guided_projectile::step(
        arena,
        terrain,
        particles,
        slot,
        super::guided_projectile::Guidance {
            yaw_turn: GUIDANCE_TURN,
            pitch_turn: GUIDANCE_TURN,
        },
        targets,
        context,
    )?;

    // sub_66180 may reflect the flyer or reuse its source slot while creating
    // the terminal field. The family comes from the dispatched callback, but
    // the wrapper payload is read from the post-guidance source row.
    let current = arena
        .entities
        .get(usize::from(slot))
        .ok_or("field spell projectile address is outside source arena")?;
    let effect_life = current.record[74];

    if let Some(impact_slot) = outcome.impact_slot {
        let impact = arena
            .entities
            .get_mut(usize::from(impact_slot))
            .ok_or("field spell impact address is outside source arena")?;
        let expected = match family {
            Family::GravityWell => (10, 67, 72),
            Family::Whirlwind => (10, 22, 22),
        };
        if (impact.class, impact.model, impact.action) != expected {
            return Err(format!(
                "field spell impact has selector {:?}, expected {expected:?}",
                (impact.class, impact.model, impact.action)
            ));
        }
        if family == Family::GravityWell {
            // sub_677A0 replaces the child phase and life, but leaves the
            // constructor's NewEvent maxLife untouched.
            impact.set_source_byte_46(0);
            impact.set_life(i32::from(effect_life));
        } else {
            // sub_678E0 gives Wind its separate eightfold lifetime.
            let field_lifetime = i32::from(effect_life).wrapping_mul(8);
            impact.set_max_life(field_lifetime);
            impact.set_life(field_lifetime);
            crate::world::fields::sync_wind_impact(arena, impact_slot);
        }
    }
    Ok(outcome)
}
