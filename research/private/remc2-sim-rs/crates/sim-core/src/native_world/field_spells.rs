//! Source holders and guided projectiles for Gravity Well20 and Whirlwind21.
//!
//! This adapter owns the ordinary holder callback and its source-local
//! projectile payload writes. Shared scheduling still owns phase advancement,
//! the same-pass tail, and next-pass reclamation; guided flight and live field
//! state remain with their existing owners.

use super::{EntitySnapshot, Level2SharedSpellResetSession, ParticleParameter, SessionTerrain};
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

fn launch_speed(parent_speed: i16) -> i16 {
    // sub_6DCA0 assigns the sum back to a signed 16-bit source field before
    // applying the 384..8192 clamps.
    PROJECTILE_SPEED
        .wrapping_add(parent_speed)
        .clamp(PROJECTILE_SPEED, PROJECTILE_MAXIMUM_LAUNCH_SPEED)
}

fn source_projectile_flags(flags: u32) -> u32 {
    flags & !8
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
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    holder_slot: u16,
    parent_slot: u16,
    input: CastInput,
    family: Family,
    projectile_power: u16,
    effect_life: u8,
) -> Result<Option<u16>, String> {
    let particle_index = family.particle();
    let particle = *particles
        .get(usize::from(particle_index))
        .ok_or_else(|| format!("field spell requires retained particle{particle_index}"))?;
    let (mut position, inherited_speed) = {
        let parent = arena
            .entities
            .get(usize::from(parent_slot))
            .ok_or("field spell parent address is outside source arena")?;
        (parent.position, parent.actual_speed)
    };
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    if slot == parent_slot {
        position = Position::default();
    }

    let mut row =
        super::cave_features::new_event(slot, arena.economy.new_event_rng(slot), position);
    row.class = 9;
    row.model = family.projectile_model();
    row.record[67] = 9;
    row.record[68] = family.projectile_model();
    row.set_source_action(family.projectile_action());
    row.set_control_flags(source_projectile_flags(row.control_flags));
    row.set_actual_speed(launch_speed(inherited_speed));
    put_word(&mut row, 136, PROJECTILE_SPEED as u16);
    let projectile_life = PROJECTILE_RANGE / i32::from(PROJECTILE_SPEED);
    row.set_max_life(projectile_life);
    row.set_life(projectile_life);
    row.parameter_relation = Some(PARAMETER_RELATION);
    row.record[164..168].copy_from_slice(&61_u32.to_le_bytes());
    super::cave_features::bind_particle(&mut row, particle_index, particle)?;
    row.record[71] = 10;
    row.record[72] = family.terminal_model();
    put_word(&mut row, 42, holder_slot);
    row.set_mana(50);
    row.set_subspell_index(projectile_power);
    row.set_source_byte_46(effect_life);
    row.set_source_dword_10(i32::from(input.auxiliary_byte));

    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, true)?;

    // NewEvent can replace either fixed address. The source rereads both
    // addresses before applying the final owner, mana, hand, and aim writes.
    let parent = arena.entities[usize::from(parent_slot)].clone();
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
    let axis = target_axis(parent.position, yaw, terrain);
    put_word(row, 158, axis.x);
    put_word(row, 160, axis.y);
    put_word(row, 162, axis.z as u16);
    row.set_yaw(yaw);
    row.set_pitch(pitch);
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    Ok(Some(slot))
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
                    mana_cost: profile.mana_cost,
                    castle_mana_required: profile.castle_mana_required,
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
                    mana_cost: profile.mana_cost,
                    castle_mana_required: profile.castle_mana_required,
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
        outcome.projectile = launch(
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
        if outcome.projectile.is_some() {
            outcome.clear_player_aux = Some(parent_slot);
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

    if let Some(mana_regen) = parent_mana_regen
        && let Some(parent) = arena.entities.get_mut(usize::from(parent_slot))
    {
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
            let tier = crate::spell_activation::level2_applied_tier(deferred.wrapping_sub(1) as i8)
                .ok_or("field spell deferred tier is negative")?;
            let profile =
                crate::spell_activation::level2_set_spell_profile(family.profile_family(), tier)
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
        CastInput, Family, GRAVITY_WELL_TIERS, WHIRLWIND_TIERS, launch_speed,
        source_projectile_flags, step_holder, step_projectile, target_axis, word,
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
        assert_eq!(launch_speed(i16::MAX), 384);
        assert_eq!(launch_speed(8_000), 8_192);
    }

    #[test]
    fn projectile_constructor_only_clears_source_flag_08() {
        assert_eq!(source_projectile_flags(0), 0);
        assert_eq!(source_projectile_flags(0x0c), 0x04);
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
                    )
                    .expect("guided source callback constructs the terminal field")
                }
                .impact_slot
                .expect("terminal field allocation succeeds");
                let lifetime = i32::from(profile.effect_parameter) * 8;
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
                assert_eq!((terminal.max_life, terminal.life), (lifetime, lifetime));

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

        let inspection =
            NativeWorld::inspect_level2(crate::native_world::tests::level_two_fixture_path())
                .expect("qualified Level 2 native fixture");
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
            let owner = session.session.state.live.core.state.entity_index;
            let caster_position = session.session.state.live.core.state.position;
            {
                let live = &mut session.session.state.live;
                live.shared_spells
                    .materialize_carried_spell_families(
                        &mut live.economy.economy,
                        &[(family_id, 0)],
                    )
                    .expect("source carry constructor materializes the typed field holder");
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
            let projectile_slot =
                projectile_slot.expect("ordinary PlayerCommand cast reaches its scheduled flyer");
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
            let cooldown = u16::from_le_bytes([holder.record[58], holder.record[59]]);
            assert!(cooldown > 0 && cooldown <= profile.cooldown);

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
            let lifetime = i32::from(profile.effect_parameter) * 8;
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
                        && row.max_life == lifetime
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
                    assert_eq!(
                        session.economy.entities[usize::from(impact_slot)].record[74],
                        0,
                        "sub_678E0 scales field life from projectile byte46 but preserves the Gravity constructor phase"
                    );
                    // The source keeps this payload byte while action72 counts
                    // down, then resets it to phase zero on the scheduled
                    // expiry callback. Its next real action74 visit performs
                    // the Gravity pull through the shared actor authority.
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
                        session.economy.entities[usize::from(impact_slot)].record[62];
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
                        session.economy.entities[usize::from(impact_slot)].record[62],
                        first_shrink_tail_before.wrapping_add(1),
                        "Gravity action74 advances byte 0x3e in the same pass"
                    );

                    let mut hidden_tail = None;
                    for _ in 0..=18 {
                        let field = &session.economy.entities[usize::from(impact_slot)];
                        if field.control_flags & 0x400 != 0 {
                            break;
                        }
                        let byte_3e_before = field.record[62];
                        session.step_outer_turn(PlayerCommand::default(), 0);
                        no_dispatch_errors(&session);
                        let field = &session.economy.entities[usize::from(impact_slot)];
                        if field.control_flags & 0x400 != 0 {
                            assert_eq!(
                                field.record[62],
                                byte_3e_before.wrapping_add(1),
                                "Gravity hide callback advances byte 0x3e in the same pass"
                            );
                            hidden_tail = Some(field.record[62]);
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

                    let children = session
                        .economy
                        .model22_spawned_children()
                        .iter()
                        .filter(|child| child.active && child.parent_entity_index == impact_slot)
                        .map(|child| child.source_entity_index)
                        .collect::<Vec<_>>();
                    let byte_3e_before =
                        session.economy.entities[usize::from(impact_slot)].record[62];
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
                        session.economy.entities[usize::from(impact_slot)].record[62],
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
                    for child in children {
                        assert!(!session.economy.map_chains.is_linked(child));
                    }
                }
            }
        }
    }
}

/// Source guided callback followed by `sub_678E0`'s lifetime write. Guided
/// impact already copies the current projectile subspell value onto its field.
/// Shared movement, impact creation, and pass tails remain with their owners.
pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    targets: Option<&super::guided_projectile::Targets<'_>>,
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
        let field_lifetime = i32::from(effect_life).wrapping_mul(8);
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
        impact.set_max_life(field_lifetime);
        impact.set_life(field_lifetime);
        if family == Family::Whirlwind {
            crate::world::fields::sync_wind_impact(arena, impact_slot);
        }
    }
    Ok(outcome)
}
