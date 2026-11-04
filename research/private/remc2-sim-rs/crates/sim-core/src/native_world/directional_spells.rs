//! Steal Mana and Duel through their ordinary Level 2 source rows.
//!
//! The caller owns source-slot scheduling, common phase advancement, player
//! progression publication, and following-pass reclaim. This module creates
//! the ordinary class-9 flyers and terminal class-10 fields, writes the
//! repaired per-target contact lanes, and consumes those lanes against the
//! source actor and Castle carriers.

use super::{
    EntitySnapshot, Level2SharedSpellResetSession, ParticleParameter, SessionTerrain, area_damage,
};
use crate::campaign::source_arena::SourceArena;
use crate::spells::directional::{
    CastleMana, DuelActor, DuelTetherBinding, ManaActor, StealManaOutcome, apply_steal_mana,
    bind_tether, duel, steal_mana,
};
use crate::{FlightWorld, Position, move_entity};

pub(crate) use crate::spell_activation::{
    ProjectileSpellCastInput as CastInput, ProjectileSpellHolderOutcome as HolderOutcome,
};

const STEAL_MANA_FAMILY: u8 = 13;
const DUEL_FAMILY: u8 = 14;
const STEAL_MANA_ACTION: u8 = 39;
const DUEL_ACTION: u8 = 42;
const STEAL_MANA_PROJECTILE_MODEL: u8 = 8;
const DUEL_PROJECTILE_MODEL: u8 = 7;
const STEAL_MANA_FIELD_MODEL: u8 = 25;
const DUEL_FIELD_MODEL: u8 = 26;
const STEAL_MANA_PARAMETER: u16 = 63;
const DUEL_PARAMETER: u16 = 60;
const STEAL_MANA_PARTICLE: u16 = 214;
const DUEL_PARTICLE: u16 = 213;
const PROJECTILE_SPEED: i16 = 384;
const PROJECTILE_RANGE: i32 = 0x2000;
const FIELD_LIFETIME: i32 = 8;

pub(crate) const STEAL_MANA_SOURCE_CONSTRUCTOR: super::guided_projectile::Constructor = super::guided_projectile::Constructor {
    model: STEAL_MANA_PROJECTILE_MODEL, action: STEAL_MANA_PROJECTILE_MODEL,
    parameter: STEAL_MANA_PARAMETER, particle: STEAL_MANA_PARTICLE,
    speed: PROJECTILE_SPEED, maximum_speed: PROJECTILE_RANGE as i16,
    life: PROJECTILE_RANGE / PROJECTILE_SPEED as i32, mana: 50,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DirectionalCastInput {
    pub projectile: CastInput,
    /// The caster's own A4 word146 when its current carrier is available.
    /// Idle, unadmitted and early Duel pulses do not read this source word.
    pub duel_target_active: Option<bool>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct DirectionalFieldOutcome {
    pub accepted_target_slots: Vec<u16>,
    pub life_after: i32,
    pub drawing_disabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StealManaConsumption {
    pub caster_entity_index: u16,
    pub target_entity_index: u16,
    pub result: StealManaOutcome,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct DirectionalMailboxOutcome {
    pub duel: Option<DuelTetherBinding>,
    pub steal_mana: Option<StealManaConsumption>,
    /// Source victim feedback also runs when a consumed lane cannot produce
    /// its effect. Steal's hit timers require a resolved attacker address.
    pub duel_consumed: bool,
    pub steal_consumed: bool,
    pub steal_attacker_resolved: bool,
}

/// Publish the ordered actor-side source effects at their original boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DirectionalFeedback {
    Duel(DuelTetherBinding),
    Steal(u16),
}

fn word(row: &EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}

fn put(row: &mut EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn source_axis(row: &EntitySnapshot) -> Position {
    Position {
        x: word(row, 158),
        y: word(row, 160),
        z: word(row, 162) as i16,
    }
}

fn owner(row: &mut EntitySnapshot, owner: u16) {
    row.source_id = owner;
    put(row, 30, owner);
}

fn source_profile(holder: &EntitySnapshot) -> Result<(u8, u8), String> {
    let family = match (holder.class, holder.model, holder.action) {
        (15, STEAL_MANA_FAMILY, STEAL_MANA_ACTION) => STEAL_MANA_FAMILY,
        (15, DUEL_FAMILY, DUEL_ACTION) => DUEL_FAMILY,
        _ => return Err("directional holder has a different source selector".to_owned()),
    };
    let tier = crate::spell_activation::level2_applied_tier(holder.level_gate)
        .ok_or("directional holder has a negative source tier")?;
    let valid = if family == STEAL_MANA_FAMILY {
        steal_mana::tier_rule(tier).is_some()
    } else {
        duel::tier_rule(tier).is_some()
    };
    if !valid {
        return Err("directional holder tier is outside SPELLS.DAT".to_owned());
    }
    Ok((family, tier))
}

/// `sub_6B3E0` / `sub_6B610`: ordinary family-13/14 holders, including
/// admission, source flyer construction, mana-regeneration bookkeeping,
/// active countdown, cooldown and deferred-tier reclaim.
pub(crate) fn step_holder(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    input: DirectionalCastInput,
) -> Result<HolderOutcome, String> {
    let mut holder = shared
        .pickup(slot)
        .cloned()
        .ok_or("directional holder is absent")?;
    let (family, tier) = source_profile(&holder)?;
    let source_holder = arena
        .entities
        .get_mut(usize::from(slot))
        .ok_or("directional holder address is outside source arena")?;
    *source_holder = holder.clone();
    let parent_slot = holder.source_spell_parent_entity_index();
    let mut outcome = HolderOutcome::default();

    if holder.active_value > 0 {
        if parent_slot != 0
            && let Some(parent) = arena
                .entities
                .get(usize::from(parent_slot))
                .filter(|row| row.active && row.class == 3 && matches!(row.model, 0 | 1))
        {
            let fresh = holder.active_value == holder.activation_value as i16;
            let admitted = parent.mana >= 0
                && parent.life >= 0
                && (holder.mana_regen == 0
                    || input
                        .projectile
                        .castle_mana
                        .is_some_and(|mana| mana >= holder.mana_regen))
                && (!fresh || parent.mana >= holder.max_mana);
            let duel_late_abort = if admitted
                && family == DUEL_FAMILY
                && holder.active_value <= holder.activation_value as i16 - 28
            {
                !input.duel_target_active.ok_or_else(|| {
                    format!("Duel holder {slot} needs caster {parent_slot}'s current A4 target")
                })?
            } else {
                false
            };
            if admitted && !duel_late_abort {
                if fresh {
                    outcome.projectile = construct_projectile(
                        arena,
                        terrain,
                        particles,
                        slot,
                        parent_slot,
                        &holder,
                        family,
                        tier,
                        input.projectile,
                    )?;
                    if outcome.projectile.is_some() && family == STEAL_MANA_FAMILY {
                        outcome.clear_player_aux = Some(parent_slot);
                    }
                    if let Some(projectile_slot) = outcome.projectile {
                        if family == STEAL_MANA_FAMILY {
                            arena
                                .economy
                                .prepare_sound(parent_slot, parent_slot, -1, 15);
                        } else {
                            arena
                                .economy
                                .prepare_sound(projectile_slot, parent_slot, -1, 9);
                        }
                    }
                }
                // `sub_68DE0` runs on each admitted pulse, including a failed
                // NewEvent allocation, but not on Duel's late empty-target
                // abort above.
                let parent = &mut arena.entities[usize::from(parent_slot)];
                parent.set_mana_regen(if fresh {
                    if parent.mana_regen >= 0 {
                        holder.max_mana.wrapping_neg()
                    } else {
                        parent.mana_regen.wrapping_sub(holder.max_mana)
                    }
                } else {
                    parent.mana_regen.min(0)
                });
                shared.sync_player_entity_from_authority(parent);
                if parent_slot == shared.player_entity.index {
                    shared.sync_typed_spell_parent_caches_from_player_entity();
                }
            } else {
                holder.active_value = 1;
            }
        }

        holder.active_value = holder.active_value.wrapping_sub(1);
        if holder.active_value == 0 {
            holder.record[50..52].copy_from_slice(&0_i16.to_le_bytes());
            let deferred = holder.source_word_2c();
            if deferred != 0 {
                let tier =
                    crate::spell_activation::level2_applied_tier(deferred.wrapping_sub(1) as i8)
                        .ok_or("directional deferred tier is negative")?;
                let profile = crate::spell_activation::level2_set_spell_profile(family, tier)
                    .ok_or("directional deferred tier has no source profile")?;
                holder.apply_nyphur_spell_profile(profile, tier);
                holder.set_source_word_2c(0);
            }
        }
    }

    let cooldown = word(&holder, 58) as i16;
    if cooldown != 0 {
        put(&mut holder, 58, cooldown.wrapping_sub(1) as u16);
    }
    holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
    if arena.entities[usize::from(slot)].class == 15
        && arena.entities[usize::from(slot)].model == family
    {
        if let Some(current) = shared.pickup_mut(slot) {
            *current = holder.clone();
        }
        arena.entities[usize::from(slot)] = holder;
    }
    Ok(outcome)
}

fn construct_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    holder_slot: u16,
    parent_slot: u16,
    holder: &EntitySnapshot,
    family: u8,
    tier: u8,
    input: CastInput,
) -> Result<Option<u16>, String> {
    let (model, parameter, particle_index, terminal_model, axis_distance) =
        if family == STEAL_MANA_FAMILY {
            (
                STEAL_MANA_PROJECTILE_MODEL,
                STEAL_MANA_PARAMETER,
                STEAL_MANA_PARTICLE,
                STEAL_MANA_FIELD_MODEL,
                0x4000,
            )
        } else {
            (
                DUEL_PROJECTILE_MODEL,
                DUEL_PARAMETER,
                DUEL_PARTICLE,
                DUEL_FIELD_MODEL,
                10_240,
            )
        };
    let parent = arena
        .entities
        .get(usize::from(parent_slot))
        .ok_or("directional parent address is outside source arena")?
        .clone();
    let position = parent.position;
    let definition = if family == STEAL_MANA_FAMILY {
        STEAL_MANA_SOURCE_CONSTRUCTOR
    } else {
        super::guided_projectile::Constructor {
            model, action: model, parameter, particle: particle_index,
            speed: PROJECTILE_SPEED, maximum_speed: PROJECTILE_RANGE as i16,
            life: PROJECTILE_RANGE / PROJECTILE_SPEED as i32, mana: 50,
        }
    };
    let Some(projectile_slot) = super::guided_projectile::construct(
        arena, terrain, particles, position, Some(parent_slot),
        if family == STEAL_MANA_FAMILY { parent.actual_speed } else { 0 }, definition,
    )? else {
        return Ok(None);
    };
    let mut projectile = arena.entities[usize::from(projectile_slot)].clone();
    projectile.record[71] = 10;
    projectile.record[72] = terminal_model;
    if family == DUEL_FAMILY {
        // sub_6B610 explicitly copies the holder's subSpellIndex after
        // allocating its class-9/model-7 flyer. Family 13 leaves NewEvent's
        // constructor value intact.
        projectile.set_subspell_index(holder.subspell_index);
    }

    // Holder payload is written only after NewEvent has allocated and
    // initialized the actual family flyer, matching sub_6B3E0/sub_6B610.
    put(&mut projectile, 42, holder_slot);
    owner(&mut projectile, parent.source_id);
    projectile.set_mana(holder.mana);
    projectile.set_source_byte_46(tier);
    if family == STEAL_MANA_FAMILY {
        projectile.set_source_dword_10(i32::from(input.auxiliary_byte));
    }

    // `sub_68E50` applies the parent's left/right hand offset before the
    // holder's height lift. Keep the copy in the new flyer's actual source row.
    let mut position = projectile.position;
    let hand_yaw = if parent.control_flags & 0x100 != 0 {
        Some(parent.yaw.wrapping_sub(512) & 2047)
    } else if parent.control_flags & 0x200 != 0 {
        Some(parent.yaw.wrapping_add(512) & 2047)
    } else {
        None
    };
    if let Some(yaw) = hand_yaw {
        move_entity(&mut position, yaw, 0, 256);
        if terrain.ground_height(position) > position.z as i16 {
            position = projectile.position;
        }
    }
    position.z = position.z.wrapping_add(parent.array_fov());
    projectile.set_position(position);
    let yaw = parent.yaw.wrapping_add(input.aim_yaw) & 2047;
    let pitch = parent.pitch.wrapping_add(input.aim_pitch);
    // 6B610 moves NewEvent's zeroed axis directly; unlike 6B3E0 it does
    // not first copy the parent's position into that field.
    let mut axis = if family == DUEL_FAMILY { Position::default() } else { parent.position };
    move_entity(&mut axis, yaw, pitch, axis_distance);
    put(
        &mut projectile,
        158,
        u16::from_le_bytes(axis.x.to_le_bytes()),
    );
    put(
        &mut projectile,
        160,
        u16::from_le_bytes(axis.y.to_le_bytes()),
    );
    put(
        &mut projectile,
        162,
        u16::from_le_bytes(axis.z.to_le_bytes()),
    );
    projectile.set_yaw(yaw);
    projectile.set_pitch(pitch);

    arena
        .economy
        .source_rng
        .insert(projectile_slot, projectile.entity_rng);
    arena.entities[usize::from(projectile_slot)] = projectile;
    crate::world::effects::sync_position(arena, terrain, projectile_slot, position, true, true)?;
    Ok(Some(projectile_slot))
}

/// `sub_4F6A0` / `sub_4F720`, called only after sub_65820 has completed the
/// flyer movement and collision pass. The shared guided step applies its
/// source-ordered owner/target/subspell/tier copies after this allocation.
pub(crate) fn construct_impact(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    projectile_slot: u16,
) -> Result<Option<u16>, String> {
    let projectile = arena
        .entities
        .get(usize::from(projectile_slot))
        .ok_or("directional projectile address is outside source arena")?
        .clone();
    let (model, action) = match (
        projectile.class,
        projectile.model,
        projectile.action,
        projectile.record[71],
        projectile.record[72],
    ) {
        (
            9,
            STEAL_MANA_PROJECTILE_MODEL,
            STEAL_MANA_PROJECTILE_MODEL,
            10,
            STEAL_MANA_FIELD_MODEL,
        ) => (STEAL_MANA_FIELD_MODEL, STEAL_MANA_FIELD_MODEL),
        (9, DUEL_PROJECTILE_MODEL, DUEL_PROJECTILE_MODEL, 10, DUEL_FIELD_MODEL) => {
            (DUEL_FIELD_MODEL, DUEL_FIELD_MODEL)
        }
        _ => return Err("directional guided impact has a different source selector".to_owned()),
    };
    let particle = if model == DUEL_FIELD_MODEL {
        Some(
            *particles
                .get(usize::from(DUEL_PARTICLE))
                .ok_or("Duel field requires retained particle 213")?,
        )
    } else {
        None
    };
    let Some(impact_slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let position = if impact_slot == projectile_slot {
        Position { x: 0, y: 0, z: 0 }
    } else {
        projectile.position
    };
    let mut impact = super::cave_features::new_event(
        impact_slot,
        arena.economy.new_event_rng(impact_slot),
        position,
    );
    impact.class = 10;
    impact.model = model;
    impact.record[67] = 10;
    impact.record[68] = model;
    impact.set_source_action(action);
    impact.set_max_life(FIELD_LIFETIME);
    impact.set_life(FIELD_LIFETIME);
    impact.set_subspell_index(if model == STEAL_MANA_FIELD_MODEL {
        2_000
    } else {
        200
    });
    if model == STEAL_MANA_FIELD_MODEL {
        let low = (impact.control_flags as u8 & 0xf6) | 1;
        impact.set_control_flags((impact.control_flags & !0xff) | u32::from(low));
    } else {
        impact.set_control_flags(impact.control_flags & !0x08);
        if let Some(particle) = particle {
            super::cave_features::bind_particle(&mut impact, DUEL_PARTICLE, particle)?;
        }
    }
    for offset in [86, 88, 92] {
        impact.record[offset..offset + 2].copy_from_slice(&512_i16.to_le_bytes());
    }
    arena
        .economy
        .source_rng
        .insert(impact_slot, impact.entity_rng);
    arena.entities[usize::from(impact_slot)] = impact;
    crate::world::effects::sync_position(arena, terrain, impact_slot, position, true, true)?;
    Ok(Some(impact_slot))
}

/// Selects each ordinary flyer's source turn values and forwards the frozen
/// target snapshot to sub_65820. The shared guided owner still must admit
/// these selectors and route their terminal constructors through
/// [`construct_impact`].
pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    targets: Option<&super::guided_projectile::Targets<'_>>,
    context: super::source_target::Context<'_>,
) -> Result<super::guided_projectile::StepOutcome, String> {
    let guidance = match arena
        .entities
        .get(usize::from(slot))
        .map(|row| (row.class, row.model, row.action, row.parameter_relation))
    {
        Some((9, DUEL_PROJECTILE_MODEL, DUEL_PROJECTILE_MODEL, Some(DUEL_PARAMETER))) => {
            super::guided_projectile::Guidance {
                yaw_turn: 22,
                pitch_turn: 22,
            }
        }
        Some((
            9,
            STEAL_MANA_PROJECTILE_MODEL,
            STEAL_MANA_PROJECTILE_MODEL,
            Some(STEAL_MANA_PARAMETER),
        )) => super::guided_projectile::Guidance {
            yaw_turn: 11,
            pitch_turn: 22,
        },
        _ => {
            return Err("directional guided projectile has a different source selector".to_owned());
        }
    };
    super::guided_projectile::step(arena, terrain, particles, slot, guidance, targets, context)
}

/// Model25 `sub_33E20` and model26 `sub_33E80`. The source fields write the
/// repaired six-byte per-target lane; accepted hits remain in the actor until
/// its scheduled `sub_5EFA0` visit consumes them.
pub(crate) fn step_field(
    arena: &mut SourceArena,
    slot: u16,
) -> Result<DirectionalFieldOutcome, String> {
    let mut field = arena
        .entities
        .get(usize::from(slot))
        .ok_or("directional field address is outside source arena")?
        .clone();
    let (lane, tier) = match (field.class, field.model, field.action) {
        (10, STEAL_MANA_FIELD_MODEL, STEAL_MANA_FIELD_MODEL) => (
            crate::combat::actors::PlayerEffectLane::StealMana,
            field.record[74],
        ),
        (10, DUEL_FIELD_MODEL, DUEL_FIELD_MODEL) => (
            crate::combat::actors::PlayerEffectLane::Duel,
            field.record[74],
        ),
        _ => return Err("directional effect field has a different source selector".to_owned()),
    };
    let mut accepted_target_slots = Vec::new();
    if field.model == STEAL_MANA_FIELD_MODEL {
        field.set_source_dword_10(field.source_dword_10().wrapping_add(1));
        field.set_life(field.life.wrapping_sub(1));
        if field.life >= 0 {
            if field.control_flags & 2 == 0 {
                field.set_control_flags(field.control_flags | 2);
                accepted_target_slots = submit_contacts(arena, &field, lane, tier);
                if !accepted_target_slots.is_empty() {
                    field.set_life(0);
                }
            }
        } else {
            field.disable_drawing();
        }
    } else {
        let previous_life = field.life;
        field.set_source_dword_10(field.source_dword_10().wrapping_add(1));
        field.set_life(previous_life.wrapping_sub(1));
        if previous_life < 0 {
            field.disable_drawing();
        } else {
            if field.record[96] < field.record[97] {
                field.record[96] = field.record[96].wrapping_add(1);
            }
            accepted_target_slots = submit_contacts(arena, &field, lane, tier);
            if !accepted_target_slots.is_empty() {
                field.set_life(0);
            }
        }
    }
    let outcome = DirectionalFieldOutcome {
        accepted_target_slots,
        life_after: field.life,
        drawing_disabled: field.drawing_disabled(),
    };
    arena.entities[usize::from(slot)] = field;
    Ok(outcome)
}

fn submit_contacts(
    arena: &mut SourceArena,
    field: &EntitySnapshot,
    lane: crate::combat::actors::PlayerEffectLane,
    tier: u8,
) -> Vec<u16> {
    let mut accepted = Vec::new();
    // Mode 3/4 sub_10C80 visits a square of map cells in Y/X order and each
    // cell's intrusive chain order. Do not replace that source order with an
    // arena-slot scan. The typed actor view supplies the current Castle or
    // player pose/shape for overlap; only a matching live raw class-3 row owns
    // the per-player six-byte mailbox written here.
    let pitch_plus_255 = i32::from(field.array_pitch()) + 255;
    // my_sign32 is the sign bit (-1 or 0), not Rust's three-way signum.
    let radius = (pitch_plus_255 - (pitch_plus_255 >> 31) * 255) >> 8;
    if radius < 0 {
        return accepted;
    }
    let center_x = ((u32::from(field.position.x) + 128) >> 8) as i32;
    let center_y = ((u32::from(field.position.y) + 128) >> 8) as i32;
    for delta_y in -radius..=radius {
        let tile_y = (center_y + delta_y) as u8;
        for delta_x in -radius..=radius {
            let tile_x = (center_x + delta_x) as u8;
            let candidates = arena.economy.map_chains.entity_chain(tile_x, tile_y);
            for target_slot in candidates {
                let Some(target) = area_damage::source_actor(arena, None, target_slot) else {
                    continue;
                };
                if target.class != 3
                    || target.source_id == field.source_id
                    || !source_contact_overlaps_actor(field, target)
                {
                    continue;
                }
                let Some(target_row) = arena.entities.get_mut(usize::from(target_slot)) else {
                    continue;
                };
                if !target_row.active
                    || (target_row.class, target_row.model) != (target.class, target.model)
                    || target_row.source_id != target.source_id
                {
                    continue;
                }
                if target_row.submit_player_effect_contact(
                    lane,
                    u16::from(tier),
                    field.source_id,
                    true,
                ) {
                    accepted.push(target_slot);
                }
            }
        }
    }
    accepted
}

fn source_contact_overlaps_actor(
    field: &EntitySnapshot,
    target: crate::combat::area::Actor,
) -> bool {
    let dx = i32::from(
        target
            .position
            .x
            .wrapping_sub(field.position.x)
            .cast_signed(),
    );
    let dy = i32::from(
        target
            .position
            .y
            .wrapping_sub(field.position.y)
            .cast_signed(),
    );
    let dz = i32::from(field.position.z) + i32::from(field.array_yaw())
        - i32::from(target.position.z)
        - i32::from(target.shape.vertical_offset);
    dx.abs() < i32::from(field.array_pitch()) + i32::from(target.shape.half_width)
        && dy.abs() < i32::from(field.array_roll()) + i32::from(target.shape.half_depth)
        && dz.abs() < i32::from(field.array_fov()) + i32::from(target.shape.half_height)
}

fn actor(arena: &SourceArena, slot: u16) -> Option<ManaActor> {
    let row = arena.entities.get(usize::from(slot))?;
    if !row.active || row.class != 3 || !matches!(row.model, 0 | 1) {
        return None;
    }
    Some(ManaActor {
        entity_index: row.index,
        active: row.active,
        class: row.class,
        life: row.life,
        mana: row.mana,
        max_mana: row.max_mana,
    })
}

fn duel_actor(arena: &SourceArena, slot: u16) -> Option<DuelActor> {
    let row = arena.entities.get(usize::from(slot))?;
    if !row.active || row.class != 3 || !matches!(row.model, 0 | 1) {
        return None;
    }
    Some(DuelActor {
        entity_index: row.index,
        active: row.active,
        class: row.class,
        life: row.life,
        position: row.position,
    })
}

fn castle_slot(arena: &SourceArena, castle_slots: &[(u16, u16)], player_slot: u16) -> Option<u16> {
    castle_slots.iter().find_map(|(player, castle)| {
        (*player == player_slot
            && arena
                .economy
                .castle_for_owner(player_slot)
                .is_some_and(|root| root.source_entity_index == *castle))
        .then_some(*castle)
    })
}

#[derive(Clone, Copy)]
struct CastleProjection {
    position: Position,
    pitch: i16,
    roll: i16,
    mana: i32,
}

/// Resolve only a current owner/root generation pair from typed authority.
fn castle_projection(arena: &SourceArena, slot: u16) -> Option<CastleProjection> {
    let castle = arena.economy.castle_at(slot)?;
    let current = arena.economy.castle_for_owner(castle.owner_entity_index)?;
    (current.source_entity_index == slot).then_some(CastleProjection {
        position: castle.position,
        pitch: castle.shift_pitch,
        roll: castle.shift_roll,
        mana: castle.mana,
    })
}

fn castle_mana(arena: &SourceArena, slot: Option<u16>) -> Option<CastleMana> {
    let slot = slot?;
    let castle = castle_projection(arena, slot)?;
    Some(CastleMana {
        entity_index: slot,
        active: true,
        mana: castle.mana,
    })
}

fn set_actor_mana(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    slot: u16,
    mana: i32,
) {
    if let Some(row) = arena.entities.get_mut(usize::from(slot)) {
        row.set_mana(mana);
        shared.sync_player_entity_from_authority(row);
    }
}

fn set_castle_mana(arena: &mut SourceArena, slot: u16, mana: i32) -> bool {
    if castle_projection(arena, slot).is_none() {
        return false;
    }
    let Some(castle) = arena.economy.castle_at_mut(slot) else {
        return false;
    };
    castle.mana = mana;
    if let Some(row) = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && (row.class, row.model) == (3, 2))
    {
        row.set_mana(mana);
    }
    true
}

fn duel_holder_slot(shared: &Level2SharedSpellResetSession, caster_slot: u16) -> Option<u16> {
    let slot = shared
        .players
        .iter()
        .find(|player| player.entity_index == caster_slot)?
        .enabled_spell_holders[usize::from(DUEL_FAMILY)];
    shared
        .pickup(slot)
        .filter(|holder| {
            holder.class == 15
                && holder.model == DUEL_FAMILY
                && holder.action == DUEL_ACTION
                && holder.source_spell_parent_entity_index() == caster_slot
        })
        .map(|holder| holder.index)
}

/// Publish a consumed Duel lane to the existing FlightState/`apply_tether`
/// path. The source-boundary integration calls this only for an accepted
/// binding returned by `consume_actor_mailbox`.
pub(crate) fn install_duel_tether_binding(
    state: &mut crate::FlightState,
    binding: DuelTetherBinding,
) {
    state.tether_target_index = binding.target_entity_index;
    state.tether_holder_index = binding.holder_entity_index;
    state.tether_prior_distance = binding.prior_distance;
    state.tether_tier = u16::from(binding.tier);
}

/// `sub_5EFA0` mailbox consumption in source order. Duel produces the binding
/// consumed by the existing FlightState tether path first; Steal Mana then
/// mutates the real actors/Castle and produces model-39 sphere sidecars for
/// the normal economy scheduler.
pub(crate) fn consume_actor_mailbox_with_feedback<F>(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    actor_slot: u16,
    castle_slots: &[(u16, u16)],
    mut feedback: F,
) -> Result<DirectionalMailboxOutcome, String>
where
    F: FnMut(&mut Level2SharedSpellResetSession, &SourceArena, DirectionalFeedback) -> Result<(), String>,
{
    let target_row = arena
        .entities
        .get(usize::from(actor_slot))
        .ok_or("directional mailbox actor address is outside source arena")?;
    // sub_5EFA0 only reads or clears either effect lane while the target's
    // life is nonnegative. Preserve pending hits across the dead callback.
    if !target_row.active
        || target_row.class != 3
        || !matches!(target_row.model, 0 | 1)
        || target_row.life < 0
    {
        return Ok(DirectionalMailboxOutcome::default());
    }
    let actor_handle = arena.handle(actor_slot);

    let mut outcome = DirectionalMailboxOutcome::default();
    let duel_hit = arena.entities[usize::from(actor_slot)]
        .take_player_effect(crate::combat::actors::PlayerEffectLane::Duel);
    if let Some(hit) = duel_hit {
        outcome.duel_consumed = true;
        let caster_slot = hit.attacker;
        // The hit callback writes the relation even when its holder has
        // expired; the later flight callback checks the holder and clears it.
        let holder_slot = duel_holder_slot(shared, caster_slot).unwrap_or(0);
        if let (Some(caster), Some(target)) = (
            duel_actor(arena, caster_slot),
            duel_actor(arena, actor_slot),
        ) && let Some(binding) = bind_tether(hit.payload, caster, target, holder_slot)
        {
            feedback(shared, arena, DirectionalFeedback::Duel(binding))?;
            outcome.duel = Some(binding);
        }
    }

    let steal_hit = arena.entities[usize::from(actor_slot)]
        .take_player_effect(crate::combat::actors::PlayerEffectLane::StealMana);
    if let Some(hit) = steal_hit {
        let caster_slot = hit.attacker;
        outcome.steal_consumed = true;
        outcome.steal_attacker_resolved = arena.handle(caster_slot).is_some();
        if let (Some(mut thief), Some(mut victim)) =
            (actor(arena, caster_slot), actor(arena, actor_slot))
        {
            // sub_61050 awards the caster's XP before any Castle debit or
            // sphere allocation can replace a fixed arena address.
            feedback(shared, arena, DirectionalFeedback::Steal(caster_slot))?;
            let thief_castle_slot = castle_slot(arena, castle_slots, caster_slot);
            let victim_castle_slot = castle_slot(arena, castle_slots, actor_slot);
            let mut thief_castle = castle_mana(arena, thief_castle_slot);
            let mut victim_castle = castle_mana(arena, victim_castle_slot);
            let result = apply_steal_mana(
                u8::try_from(hit.payload).unwrap_or(u8::MAX),
                &mut thief,
                &mut victim,
                thief_castle.as_mut(),
                victim_castle.as_mut(),
            );
            if let Some(result) = result {
                set_actor_mana(shared, arena, caster_slot, thief.mana);
                set_actor_mana(shared, arena, actor_slot, victim.mana);
                if let (Some(slot), Some(mana)) =
                    (victim_castle_slot, result.target_castle_mana_after)
                {
                    if !set_castle_mana(arena, slot, mana) {
                        return Err(format!(
                            "Steal Mana Castle debit lost current Castle carrier {slot}"
                        ));
                    }
                }
                if let Some(spawn_castle_slot) = thief_castle_slot
                    && let Some(castle) = castle_projection(arena, spawn_castle_slot)
                {
                    // 61050 calculates the source Castle's radius once,
                    // before attempting any of the individual NewEvents.
                    let pitch = i64::from(castle.pitch);
                    let roll = i64::from(castle.roll);
                    let radius = crate::legacy_sqrt(
                        u32::try_from(pitch * pitch + roll * roll).unwrap_or(u32::MAX),
                    );
                    for grant in &result.mana_spheres {
                        let _ = spawn_steal_mana_sphere(
                            arena,
                            terrain,
                            actor_slot,
                            spawn_castle_slot,
                            radius,
                            grant.mana,
                            grant.player_entity_index,
                        )?;
                    }
                }
                outcome.steal_mana = Some(StealManaConsumption {
                    caster_entity_index: caster_slot,
                    target_entity_index: actor_slot,
                    result,
                });
            }
        }
    }
    // Allocation in the Castle conversion may reuse source slots. Never
    // publish local-player feedback onto a replacement generation.
    if actor_handle.is_some()
        && arena.handle(actor_slot) == actor_handle
        && (outcome.duel_consumed || outcome.steal_consumed)
    {
        shared.apply_directional_feedback(
            &mut arena.entities[usize::from(actor_slot)],
            outcome.duel_consumed,
            outcome.steal_consumed,
            outcome.steal_attacker_resolved,
        );
    }
    Ok(outcome)
}

#[cfg(test)]
fn consume_actor_mailbox(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    actor_slot: u16,
    castle_slots: &[(u16, u16)],
) -> Result<DirectionalMailboxOutcome, String> {
    consume_actor_mailbox_with_feedback(
        shared, arena, terrain, actor_slot, castle_slots, |_, _, _| Ok(()),
    )
}

fn spawn_steal_mana_sphere(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    hit_actor_slot: u16,
    caster_castle_slot: u16,
    radius: u32,
    mana: i32,
    player_entity_index: u16,
) -> Result<Option<u16>, String> {
    let target_actor = arena
        .entities
        .get(usize::from(hit_actor_slot))
        .ok_or("Steal Mana hit actor is outside source arena")?
        .clone();
    let Some(castle) = castle_projection(arena, caster_castle_slot) else {
        return Ok(None);
    };

    let source_rng = target_actor
        .entity_rng
        .wrapping_mul(9_377)
        .wrapping_add(9_439);
    arena.entities[usize::from(hit_actor_slot)].set_entity_rng(source_rng);
    arena.economy.source_rng.insert(hit_actor_slot, source_rng);
    let angle = (source_rng & 0x07ff) as i16;
    let mut position = castle.position;
    move_entity(
        &mut position,
        angle,
        0,
        (radius as u16).cast_signed(),
    );
    position.z = terrain.ground_height(position).wrapping_add(1_024);

    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    arena.economy.initialize_death_mana_sphere_at_slot(
        slot,
        hit_actor_slot,
        position,
        angle,
        mana,
        player_entity_index,
        terrain,
    );
    let sphere = arena
        .economy
        .spheres
        .iter_mut()
        .find(|sphere| sphere.source_entity_index == slot)
        .ok_or("Steal Mana NewEvent lost its model-39 sphere carrier")?;
    sphere.position = position;
    sphere.mana = mana;
    // sub_61050 sets the new sphere's vertical speed after allocation;
    // death-sphere initialization otherwise derives zero at this altitude.
    sphere.vertical_velocity = 128;
    let entity_rng = sphere.entity_rng;
    arena.economy.source_rng.insert(slot, entity_rng);
    if let Some(row) = arena.entities.get_mut(usize::from(slot)) {
        row.set_source_player_entity_index(player_entity_index);
    }
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    Ok(Some(slot))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    fn ready_holder(family: u8, tier: u8) -> (super::super::Mc2Simulation, u16, u16) {
        let mut simulation = super::super::tests::level_two_campaign_session();
        let live = &mut simulation.session.state.live;
        let parent_slot = live.core.state.entity_index;
        let holder_slot = live
            .shared_spells
            .pickups()
            .iter()
            .find(|row| row.class == 15 && row.model == 3)
            .expect("level-two spell pickup exists")
            .index;
        let profile = crate::spell_activation::level2_set_spell_profile(family, tier)
            .expect("directional source profile exists");
        let holder = live
            .shared_spells
            .pickup_mut(holder_slot)
            .expect("picked spell has a shared row");
        holder.apply_nyphur_spell_profile(profile, tier);
        holder.class = 15;
        holder.model = family;
        holder.record[67] = 15;
        holder.record[68] = family;
        holder.set_source_action(if family == STEAL_MANA_FAMILY {
            STEAL_MANA_ACTION
        } else {
            DUEL_ACTION
        });
        holder.record[44..46].copy_from_slice(&parent_slot.to_le_bytes());
        holder.active_value = holder.activation_value as i16;
        holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
        holder.set_control_flags(
            holder.control_flags & !crate::native_world::ENTITY_DRAW_DISABLED_BIT,
        );
        let required_mana = holder.max_mana;
        let parent = &mut live.economy.entities[usize::from(parent_slot)];
        parent.set_life(100);
        if parent.max_mana < required_mana {
            parent.max_mana = required_mana;
            parent.record[144..148].copy_from_slice(&required_mana.to_le_bytes());
        }
        if parent.mana < required_mana {
            parent.set_mana(required_mana);
        }
        if family == DUEL_FAMILY {
            let player = live
                .shared_spells
                .players
                .iter_mut()
                .find(|player| player.entity_index == parent_slot)
                .expect("holder owner has a player spell state");
            player.enabled_spell_holders[usize::from(DUEL_FAMILY)] = holder_slot;
        }
        (simulation, holder_slot, parent_slot)
    }

    fn cast(
        simulation: &mut super::super::Mc2Simulation,
        holder_slot: u16,
        duel_target_active: bool,
    ) -> u16 {
        let particles = simulation.session.definition.presentation.particles.clone();
        let live = &mut simulation.session.state.live;
        let outcome = step_holder(
            &mut live.shared_spells,
            &mut live.economy,
            &mut live.terrain,
            &particles,
            holder_slot,
            DirectionalCastInput {
                projectile: CastInput {
                    auxiliary_byte: 7,
                    castle_mana: Some(i32::MAX),
                    ..Default::default()
                },
                duel_target_active: Some(duel_target_active),
            },
        )
        .expect("ordinary directional holder step");
        outcome
            .projectile
            .expect("class-9 flyer allocated from cast")
    }

    fn advance_flyer_source_step(flyer: &mut EntitySnapshot) {
        let delta = i32::from(flyer.minimum_speed()) - i32::from(flyer.actual_speed);
        let speed = flyer.actual_speed.wrapping_add((2 * delta.signum()) as i16);
        let mut position = flyer.position;
        move_entity(&mut position, flyer.yaw, flyer.pitch, speed);
        flyer.set_actual_speed(speed);
        flyer.set_position(position);
    }

    fn flyer_position_after_source_steps(flyer: &EntitySnapshot, steps: usize) -> Position {
        let mut projected = flyer.clone();
        for _ in 0..steps {
            advance_flyer_source_step(&mut projected);
        }
        projected.position
    }

    fn source_probe_eligible_target_position(flyer: &EntitySnapshot, steps: usize) -> Position {
        let forecast = flyer_position_after_source_steps(flyer, steps);
        let width = i32::from(flyer.array_pitch()) + 255;
        let radius = ((width - (width >> 31) * 255) >> 8) as u8;
        let offsets = crate::level1_mission::source_search_offsets(radius);
        // GDB observes the real visit-4 probe at (2,237), while the prior
        // lower-half clamp linked the target at (1,236). Use the probe's own
        // cell, which source_search_offsets always includes as (0,0).
        assert!(
            offsets.contains(&(0, 0)),
            "source search admits its probe cell"
        );
        let probe_cell_start = |axis: u16| (axis.wrapping_add(128) >> 8).wrapping_mul(256);
        Position {
            x: probe_cell_start(forecast.x),
            y: probe_cell_start(forecast.y),
            ..forecast
        }
    }

    fn fly_into_target(
        simulation: &mut super::super::Mc2Simulation,
        projectile_slot: u16,
        parent_slot: u16,
        target_class: u8,
    ) -> (u16, u16) {
        let (position, particles) = {
            let live = &simulation.session.state.live;
            (
                source_probe_eligible_target_position(
                    &live.economy.entities[usize::from(projectile_slot)],
                    4,
                ),
                simulation.session.definition.presentation.particles.clone(),
            )
        };
        let target_slot = add_target_actor(simulation, parent_slot, position, target_class);
        fly_projectile_into_target_slot(
            simulation,
            projectile_slot,
            target_slot,
            target_class,
            particles,
        )
    }

    fn fly_into_actual_opposing_player(
        simulation: &mut super::super::Mc2Simulation,
        projectile_slot: u16,
        parent_slot: u16,
    ) -> (u16, u16) {
        let (target_slot, position, particles) = {
            let live = &simulation.session.state.live;
            let target_slot = live
                .shared_spells
                .players
                .iter()
                .filter(|player| {
                    player.entity_index != parent_slot && player.active_player_index >= 0
                })
                .find_map(|player| {
                    live.economy
                        .entities
                        .get(usize::from(player.entity_index))
                        .filter(|row| row.active && row.class == 3 && matches!(row.model, 0 | 1))
                        .map(|row| row.index)
                })
                .expect("qualified Level 2 campaign has a live opposing player actor");
            (
                target_slot,
                source_probe_eligible_target_position(
                    &live.economy.entities[usize::from(projectile_slot)],
                    4,
                ),
                simulation.session.definition.presentation.particles.clone(),
            )
        };
        {
            let live = &mut simulation.session.state.live;
            let target = &mut live.economy.entities[usize::from(target_slot)];
            assert_eq!(target.class, 3);
            assert!(matches!(target.model, 0 | 1));
            assert_ne!(target.source_id, parent_slot);
            target.set_position(position);
            target.set_life(100);
            target.max_mana = 10_000;
            target.record[144..148].copy_from_slice(&10_000_i32.to_le_bytes());
            target.set_mana(8_000);
            let target = target.clone();
            live.shared_spells
                .sync_player_entity_from_authority(&target);
            crate::world::effects::sync_position(
                &mut live.economy,
                &mut live.terrain,
                target_slot,
                position,
                true,
                true,
            )
            .expect("actual opposing player remains linked in its new source cell");
        }
        fly_projectile_into_target_slot(simulation, projectile_slot, target_slot, 3, particles)
    }

    pub(in crate::native_world) fn staged_steal_mana_impact()
    -> (super::super::Mc2Simulation, u16, u16, u16) {
        let (mut simulation, holder, caster) = ready_holder(STEAL_MANA_FAMILY, 0);
        let projectile = cast(&mut simulation, holder, false);
        let (target, field) = fly_into_actual_opposing_player(&mut simulation, projectile, caster);
        (simulation, caster, target, field)
    }

    fn fly_projectile_into_target_slot(
        simulation: &mut super::super::Mc2Simulation,
        projectile_slot: u16,
        target_slot: u16,
        target_class: u8,
        particles: Box<[ParticleParameter]>,
    ) -> (u16, u16) {
        let live = &mut simulation.session.state.live;
        let target = &live.economy.entities[usize::from(target_slot)];
        assert_eq!(
            live.economy.map_chains.tile_of(target_slot),
            u16::from(target.position.x >> 8) | (u16::from(target.position.y >> 8) << 8),
            "target is linked in its AddEventToMap source cell"
        );
        assert_eq!(
            (target.position.x >> 8, target.position.y >> 8),
            (
                target.position.x.wrapping_add(128) >> 8,
                target.position.y.wrapping_add(128) >> 8
            ),
            "target's source map cell matches sub_10780's collision probe"
        );
        let class3 = if target_class == 3 {
            vec![target_slot]
        } else {
            Vec::new()
        };
        let target_model = live.economy.entities[usize::from(target_slot)].model as usize;
        let class5: [Vec<u16>; 29] = std::array::from_fn(|model| {
            if target_class == 5 && target_model == model {
                vec![target_slot]
            } else {
                Vec::new()
            }
        });
        let targets = crate::native_world::guided_projectile::Targets {
            class3: &class3,
            class5: &class5,
            class10: &[],
            owner_target_range: 8_192,
        };
        let mut impact_slot = None;
        let mut visit_trace = Vec::new();
        // A source visit first acquires and steers toward the captured target.
        // Keep stepping that real guidance path until it reaches the actor;
        // targeting the old heading's first endpoint can miss after acquisition.
        for visit in 0..8 {
            let flyer_before = live.economy.entities[usize::from(projectile_slot)].clone();
            let outcome = step_projectile(
                &mut live.economy,
                &mut live.terrain,
                &particles,
                projectile_slot,
                Some(&targets),
                crate::native_world::source_target::Context::from_ordinary(
                    &live.goats,
                    &live.ordinary,
                ),
            )
            .expect("ordinary guided flyer movement");
            let flyer = live.economy.entities[usize::from(projectile_slot)].clone();
            let target = live.economy.entities[usize::from(target_slot)].clone();
            let target_actor = crate::native_world::area_damage::source_actor(
                &mut live.economy,
                None,
                target_slot,
            );
            let width = i32::from(flyer.array_pitch()) + 255;
            let radius = ((width - (width >> 31) * 255) >> 8) as u8;
            let in_probe_chain = crate::level1_mission::source_map_search_slots(
                &live.economy.map_chains,
                flyer.position,
                &crate::level1_mission::source_search_offsets(radius),
            )
            .contains(&target_slot);
            let target_in_frozen_snapshot = match target_class {
                3 => class3.contains(&target_slot),
                5 => class5
                    .get(usize::from(target.model))
                    .is_some_and(|slots| slots.contains(&target_slot)),
                _ => false,
            };
            let terminal_target = outcome
                .impact_slot
                .map(|slot| word(&live.economy.entities[usize::from(slot)], 154));
            visit_trace.push(format!(
                "visit={visit} flyer#{projectile_slot} model/action={}/{} selectors69/70={}/{} flags={:#x} pos={:?}->{:?} yaw/pitch={}/{} shape86/88/90/92={}/{}/{}/{} life={} expired={} hidden={} target#{target_slot} class/model={}/{} source_id={} flags={:#x} pos={:?} frozen={} actor_available={} actor={target_actor:?} linked={} tile={:#06x} in_probe_chain={} hit={:?} word154={terminal_target:?}",
                flyer_before.model,
                flyer_before.action,
                flyer.record[69],
                flyer.record[70],
                flyer.control_flags,
                flyer_before.position,
                flyer.position,
                flyer.yaw,
                flyer.pitch,
                flyer.array_yaw(),
                flyer.array_pitch(),
                flyer.array_roll(),
                flyer.array_fov(),
                flyer.life,
                flyer.life < 0,
                flyer.drawing_disabled(),
                target.class,
                target.model,
                target.source_id,
                target.control_flags,
                target.position,
                target_in_frozen_snapshot,
                target_actor.is_some(),
                live.economy.map_chains.is_linked(target_slot),
                live.economy.map_chains.tile_of(target_slot),
                in_probe_chain,
                outcome.impact_slot,
            ));
            if outcome.impact_slot.is_some() {
                impact_slot = outcome.impact_slot;
                break;
            }
        }
        let impact_slot = impact_slot.unwrap_or_else(|| {
            panic!(
                "ordinary guided flyer missed within eight visits; per-visit state:\n{}",
                visit_trace.join("\n")
            )
        });
        assert_eq!(
            word(&live.economy.entities[usize::from(impact_slot)], 154),
            target_slot,
            "the terminal must come from the real map collision with the selected target"
        );
        assert!(live.economy.entities[usize::from(projectile_slot)].drawing_disabled());
        (target_slot, impact_slot)
    }

    fn add_target_actor(
        simulation: &mut super::super::Mc2Simulation,
        parent_slot: u16,
        position: Position,
        class: u8,
    ) -> u16 {
        let live = &mut simulation.session.state.live;
        let slot = live
            .economy
            .allocate_source_slot_with_terrain(&mut live.terrain)
            .expect("test actor source slot available");
        let mut target = live.economy.entities[usize::from(parent_slot)].clone();
        target.index = slot;
        target.class = class;
        target.record[67] = class;
        target.source_id = slot;
        target.record[30..32].copy_from_slice(&slot.to_le_bytes());
        target.active = true;
        target.set_control_flags(target.control_flags | 8);
        target.set_position(position);
        target.set_life(100);
        target.max_mana = 10_000;
        target.record[144..148].copy_from_slice(&10_000_i32.to_le_bytes());
        target.set_mana(8_000);
        live.economy.entities[usize::from(slot)] = target;
        crate::world::effects::sync_position(
            &mut live.economy,
            &mut live.terrain,
            slot,
            position,
            true,
            true,
        )
        .expect("test actor map link");
        slot
    }

    fn deactivate_other_actors(arena: &mut SourceArena, keep: &[u16]) {
        for row in &mut arena.entities {
            if row.class == 3 && !keep.contains(&row.index) {
                row.active = false;
                row.class = 0;
                row.record[2] = 0;
                row.record[67] = 0;
            }
        }
    }

    fn castle_from_source_flyer(simulation: &mut super::super::Mc2Simulation, owner: u16) -> u16 {
        let live = &mut simulation.session.state.live;
        let position = (1_u16..32)
            .flat_map(|y| {
                (1_u16..32).map(move |x| Position {
                    x: x * 2_048,
                    y: y * 2_048,
                    z: 0,
                })
            })
            .find(|position| {
                !live.economy.economy.castle_projectile_site_blocked(
                    *position,
                    &live.economy.entities,
                    &live.terrain,
                ) && live.economy.economy.live_castles().all(|castle| {
                    crate::distance_3d_wrapped(
                        Position {
                            z: castle.position.z,
                            ..*position
                        },
                        castle.position,
                    ) > 25_000
                })
            })
            .expect("owner terrain has an admissible Castle flyer site");
        let ground = live.terrain.ground_height(position);
        let origin = Position {
            z: ground.wrapping_add(1),
            ..position
        };
        let axis = Position {
            z: ground.wrapping_sub(1),
            ..position
        };
        let payload = super::super::level2_dynamic_economy::Level2CastleProjectileLaunchPayload {
            position: origin,
            axis_position: axis,
            yaw: 0,
            pitch: 0,
            source_id_entity_index: owner,
            source_subspell_index: 0,
            source_mana: 0,
            source_a4_byte_154: 0,
        };
        let flyer = live
            .economy
            .economy
            .spawn_castle_projectile_with_payload(payload, 0, 0, &live.terrain)
            .expect("source Castle flyer allocation");
        let entities = live.economy.entities.clone();
        assert!(
            live.economy
                .economy
                .step_castle_projectile_slot(flyer, &entities, &live.terrain)
        );
        assert!(
            !live
                .economy
                .economy
                .step_castle_projectile_slot(flyer, &entities, &live.terrain)
        );
        let root = live
            .economy
            .economy
            .live_castles()
            .find(|castle| castle.owner_entity_index == owner)
            .expect("flyer landing allocates the source Castle root")
            .source_entity_index;
        let free_slots = live.economy.free_slots.clone();
        assert!(
            live.economy
                .economy
                .step_castle_action(root, &mut live.terrain, &free_slots)
        );
        assert_eq!(
            live.economy
                .economy
                .castle_for_owner(owner)
                .unwrap()
                .source_entity_index,
            root
        );
        root
    }

    fn scheduled_castle_from_source_flyer(
        simulation: &mut super::super::Mc2Simulation,
        owner: u16,
    ) -> u16 {
        let live = &mut simulation.session.state.live;
        // This controlled Castle flyer needs the same owner BUILD bank that
        // the ordinary Castle-spell entry binds. The acquired Level16 carry
        // need not contain Castle, so bind the canonical data explicitly.
        if !live.economy.economy.has_castle_stage_footprint(1) {
            eprintln!("controlled Castle flyer: canonical stage-one BUILD footprint was absent");
            for (stage, footprint) in super::super::campaign_castle_stage_footprints()
                .expect("source Castle BUILD footprints")
            {
                assert!(live.economy.economy.set_castle_stage_footprint(stage, footprint));
            }
        }
        let position = (1_u16..32).flat_map(|y| (1_u16..32).map(move |x| Position {
            x: x * 2_048, y: y * 2_048, z: 0,
        })).find(|position| {
            !live.economy.economy.castle_projectile_site_blocked(*position,
                &live.economy.entities, &live.terrain)
                && live.economy.economy.live_castles().all(|castle|
                    crate::distance_3d_wrapped(Position { z: castle.position.z, ..*position },
                        castle.position) > 25_000)
        }).expect("owner terrain has an admissible Castle flyer site");
        let ground = live.terrain.ground_height(position);
        let payload = super::super::level2_dynamic_economy::Level2CastleProjectileLaunchPayload {
            position: Position { z: ground.wrapping_add(1), ..position },
            axis_position: Position { z: ground.wrapping_sub(1), ..position },
            yaw: 0, pitch: 0, source_id_entity_index: owner,
            source_subspell_index: 0, source_mana: 0, source_a4_byte_154: 0,
        };
        let flyer = live.economy.economy.spawn_castle_projectile_with_payload(payload, 0, 0, &live.terrain)
            .unwrap_or_else(|| panic!("controlled source Castle flyer allocation failed: owner={owner}, footprint={}, free_slots={}",
                live.economy.economy.has_castle_stage_footprint(1), live.economy.economy.free_source_slots().len()));
        for _ in 0..32 {
            simulation.step_outer_turn(crate::PlayerCommand::default(), 0);
            assert!(simulation.dispatch_events().iter().all(|event|
                !matches!(event, super::super::Mc2DispatchEvent::Error { .. })),
                "Castle flyer {flyer} dispatch: {:?}", simulation.dispatch_events());
            if let Some(root) = simulation.session.state.live.economy.economy.castle_for_owner(owner) {
                return root.source_entity_index;
            }
        }
        panic!("scheduled source Castle flyer {flyer} did not produce owner {owner}'s Castle");
    }

    #[test]
    fn authored_steal_mana_pickup_and_public_hand_reach_real_nyphur() {
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        use crate::combat::actors::PlayerEffectLane;
        let packages = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj");
        let mut source = CampaignSession::open(packages.join("level16"),
            CampaignSelection::ordinary(16).unwrap()).expect("unfiltered Level 16 opens");
        let source_player = source.simulation().state.entity_index;
        let pickup = source.simulation().session.state.live.economy.entities.iter()
            .find(|row| row.active && (row.class, row.model, row.action) == (15, 13, 41))
            .cloned().expect("authored persistent Steal Mana pickup");
        assert_eq!(pickup.index, 164);
        for _ in 0..12 {
            let simulation = source.simulation_mut();
            let position = simulation.session.state.live.economy.entities[usize::from(pickup.index)].position;
            simulation.set_level2_entity_position(source_player, position).unwrap();
            simulation.step_outer_turn(crate::PlayerCommand::default(), 0);
            assert!(simulation.dispatch_events().iter().all(|event|
                !matches!(event, super::super::Mc2DispatchEvent::Error { .. })));
            if simulation.shared_spells().local_player().enabled_spell_holders[13] != 0 { break; }
        }
        let learned = source.simulation().shared_spells().local_player().enabled_spell_holders
            .iter().enumerate().filter_map(|(family, holder)|
                (*holder != 0).then_some(family as u8)).collect::<Vec<_>>();
        assert_eq!(source.simulation().shared_spells().local_player().enabled_spell_holders[13], pickup.index,
            "ordinary overlap acquired the real Steal Mana pickup");

        for tier in 0_u8..=2 {
            // The acquired inventory is carried into a real rival mission;
            // destination, resources, and combat positions are test controls.
            let mut campaign = CampaignSession::open(packages.join("level2"), CampaignSelection::LEVEL2)
                .expect("unfiltered Level 2 opens");
            assert_eq!(campaign.rebuild_for_spell_carry(&learned, Some(13)).unwrap(), Some(13));
            let simulation = campaign.simulation_mut();
            let caster = simulation.state.entity_index;
            let target = simulation.session.state.live.economy.entities.iter()
                .find(|row| row.active && (row.class, row.model, row.action) == (3, 1, 1))
                .expect("ordinary Level 2 has a live rival").index;
            let target_handle = simulation.session.state.live.economy.handle(target).unwrap();
            let castle = scheduled_castle_from_source_flyer(simulation, caster);
            let target_castle = (tier == 2).then(|| scheduled_castle_from_source_flyer(simulation, target));
            // Protection is source state: let the real rival upkeep expire it.
            for _ in 0..256 {
                let live = &simulation.session.state.live;
                let row = &live.economy.entities[usize::from(target)];
                assert_eq!(live.economy.handle(target), Some(target_handle));
                let rival = live.nyphur_dispatcher.get(target_handle).unwrap();
                if rival.common_upkeep(row.phase).word_0x159 == 0 { break; }
                simulation.step_outer_turn(crate::PlayerCommand::default(), 0);
                assert!(simulation.dispatch_events().iter().all(|event|
                    !matches!(event, super::super::Mc2DispatchEvent::Error { .. })));
            }
            {
                let live = &simulation.session.state.live;
                let row = &live.economy.entities[usize::from(target)];
                assert_eq!(live.nyphur_dispatcher.get(target_handle).unwrap().common_upkeep(row.phase).word_0x159, 0,
                    "ordinary rival visits expire Castle protection");
            }
            let rule = crate::level_definition::STEAL_MANA_TIERS[usize::from(tier)];
            {
                let live = &mut simulation.session.state.live;
                assert!(live.economy.economy.set_castle_mana_prerequisite_for_test(castle,
                    rule.castle_mana_required.max(rule.mana_cost) + 10_000));
                if let Some(target_castle) = target_castle {
                    assert!(live.economy.economy.set_castle_mana_prerequisite_for_test(target_castle, 12_345));
                }
                let parent = &mut live.economy.entities[usize::from(caster)];
                parent.max_mana = rule.mana_cost + 10_000;
                parent.record[144..148].copy_from_slice(&parent.max_mana.to_le_bytes());
                parent.set_mana(parent.max_mana);
                parent.set_mana_regen(0);
                let parent = parent.clone();
                live.shared_spells.sync_player_entity_from_authority(&parent);
                live.shared_spells.player_mana = parent.mana;
                live.shared_spells.caster.mana = parent.mana;
                live.shared_spells.sync_typed_spell_parent_caches_from_player_entity();
                if let Some(fire) = live.core.context.spell_fire.as_mut() { fire.caster.mana = parent.mana; }
                let target_row = &mut live.economy.entities[usize::from(target)];
                target_row.max_mana = 10_000;
                target_row.record[144..148].copy_from_slice(&10_000_i32.to_le_bytes());
                target_row.set_mana(8_000);
                target_row.set_mana_regen(0);
                let target_row = target_row.clone();
                let rival = live.nyphur_dispatcher.get_mut_in_arena(&live.economy, target).unwrap();
                assert!(rival.sync_support_parent_resources(&target_row, false));
                live.economy.economy.refresh_owner_mana_rows();
            }
            let ui = crate::SOURCE_SPELL_INDEX_MAP.iter().position(|family| *family == 13).unwrap() as i8;
            simulation.step_outer_turn(crate::PlayerCommand { opcode: 31, argument_1: ui,
                argument_2: tier as i8, ..Default::default() }, 0);
            let holder = simulation.shared_spells().local_player().enabled_spell_holders[13];
            assert_eq!(simulation.shared_spells().pickup(holder).unwrap().level_gate, tier as i8);
            {
                let live = &mut simulation.session.state.live;
                let parent = live.economy.entities[usize::from(caster)].clone();
                let actor = area_damage::source_actor(&mut live.economy, None, target).unwrap();
                let mut position = parent.position;
                move_entity(&mut position, parent.yaw, parent.pitch, 1_536);
                position.z = parent.position.z.wrapping_add(parent.array_fov())
                    .wrapping_sub(actor.shape.vertical_offset);
                live.economy.entities[usize::from(target)].set_position(position);
                crate::world::effects::sync_position(&mut live.economy, &mut live.terrain,
                    target, position, true, true).unwrap();
            }
            let xp_before = simulation.shared_spells().spell_experience()[13];
            let mut saw_flyer = false;
            let mut saw_field = false;
            let mut submitted = false;
            let mut consumed = false;
            let mut charged = false;
            let mut launch_sound = false;
            let mut projectile_presented = false;
            let mut stages = [None; 2];
            let mut hidden = [false; 2];
            let mut reclaimed = [false; 2];
            let mut previous_mana = simulation.session.state.live.economy.entities[usize::from(caster)].mana;
            let mut previous_target_mana = simulation.session.state.live.economy.entities[usize::from(target)].mana;
            let mut previous_target_castle_mana = target_castle.map(|slot|
                simulation.session.state.live.economy.economy.castle_at(slot).unwrap().mana);
            let mut prior_lane = None;
            let mut trace = Vec::new();
            for turn in 0..96 {
                simulation.step_outer_turn(if turn == 0 { crate::PlayerCommand { flags: 0x10, ..Default::default() } }
                    else { crate::PlayerCommand::default() }, 0);
                let live = &simulation.session.state.live;
                let caster_mana = live.economy.entities[usize::from(caster)].mana;
                charged |= previous_mana - caster_mana == rule.mana_cost;
                previous_mana = caster_mana;
                let products = live.economy.entities.iter().filter(|row| row.active && row.source_id == caster
                    && matches!((row.class, row.model), (9, 8) | (10, 25)))
                    .map(|row| (row.index, row.class, row.model, row.action, row.life, row.drawing_disabled()))
                    .collect::<Vec<_>>();
                saw_flyer |= products.iter().any(|row| (row.1, row.2) == (9, 8));
                saw_field |= products.iter().any(|row| (row.1, row.2) == (10, 25));
                for product in &products {
                    let stage = if product.1 == 9 { 0 } else { 1 };
                    stages[stage].get_or_insert((product.0, live.economy.economy.slot_generation(product.0)));
                }
                for (stage, saved) in stages.iter().enumerate() {
                    if let Some((slot, generation)) = *saved {
                        let row = &live.economy.entities[usize::from(slot)];
                        let same_generation = live.economy.economy.slot_generation(slot) == generation;
                        hidden[stage] |= same_generation && row.active && row.drawing_disabled();
                        reclaimed[stage] |= !same_generation || live.economy.economy.free_source_slots().contains(&slot);
                    }
                }
                for event in live.economy.economy.events() {
                    if let super::super::Level2EconomyEvent::PreparedSound { entity_index, sound_id: 15, .. } = event {
                        launch_sound |= *entity_index == caster;
                    }
                }
                for view in simulation.campaign_entity_presentations(0).expect("campaign sprite projection") {
                    let row = &live.economy.entities[usize::from(view.entity_index)];
                    if row.source_id == caster && (row.class, row.model, row.action) == (9, 8, 8) {
                        assert_eq!((view.source_id, view.position, view.render_parameter_index),
                            (caster, row.position, STEAL_MANA_PARTICLE));
                        assert_eq!(view.base_tmap, simulation.session.definition.presentation.particles
                            [usize::from(STEAL_MANA_PARTICLE)].base_tmap);
                        projectile_presented = true;
                    }
                }
                let row = &live.economy.entities[usize::from(target)];
                let lane = row.pending_player_effect(PlayerEffectLane::StealMana);
                submitted |= lane.is_some_and(|hit| hit.attacker == caster && hit.payload == i32::from(tier));
                let target_castle_mana = target_castle.and_then(|slot| live.economy.economy.castle_at(slot).map(|root| root.mana));
                let resource_debit = if tier == 2 {
                    target_castle_mana.zip(previous_target_castle_mana).is_some_and(|(now, before)| now < before)
                } else { row.mana < previous_target_mana };
                let accepted = live.economy.handle(target) == Some(target_handle)
                    && prior_lane.is_some_and(|hit: crate::combat::actors::PlayerEffectHit|
                        hit.attacker == caster && hit.payload == i32::from(tier))
                    && lane.is_none() && resource_debit
                    && simulation.shared_spells().spell_experience()[13] > xp_before;
                if accepted && tier == 2 {
                    let before = previous_target_castle_mana.unwrap();
                    let after = target_castle_mana.unwrap();
                    let amount = before.wrapping_mul(i32::from(rule.power)) / 100;
                    assert_eq!(before - after, amount, "the rival Castle pays the source percentage");
                    let grants = live.economy.economy.events().iter().filter_map(|event| match event {
                        super::super::Level2EconomyEvent::ManaSphereSpawned { sphere_id, source_entity_index, mana }
                            if *source_entity_index == target => Some((*sphere_id, *mana)),
                        _ => None,
                    }).collect::<Vec<_>>();
                    assert_eq!(grants.iter().map(|(_, mana)| *mana).sum::<i32>(), amount,
                        "actual scheduled model39 spheres carry the stolen Castle mana");
                    assert!(grants.iter().all(|(id, mana)| *mana > 0 && *mana <= 500
                        && live.economy.economy.spheres().iter().any(|sphere|
                            sphere.id == *id && sphere.player_entity_index == 0)));
                }
                consumed |= accepted;
                prior_lane = lane;
                previous_target_mana = row.mana;
                previous_target_castle_mana = target_castle_mana;
                let errors = simulation.dispatch_events().iter().filter(|event|
                    matches!(event, super::super::Mc2DispatchEvent::Error { .. })).collect::<Vec<_>>();
                trace.push(format!("turn={turn} products={products:?} target_mana={} lane={lane:?} xp={} errors={errors:?}",
                    row.mana, simulation.shared_spells().spell_experience()[13]));
                assert!(errors.is_empty(), "tier {tier}: {}", trace.join("\n"));
                if saw_flyer && saw_field && submitted && consumed && charged && launch_sound
                    && projectile_presented && reclaimed.iter().all(|reclaimed| *reclaimed)
                    && simulation.shared_spells().pickup(holder).unwrap().active_value == 0 { break; }
            }
            assert!(saw_flyer && saw_field && submitted && consumed && charged && launch_sound
                    && projectile_presented && hidden.iter().all(|hidden| *hidden)
                    && reclaimed.iter().all(|reclaimed| *reclaimed)
                    && simulation.shared_spells().pickup(holder).unwrap().active_value == 0,
                "tier {tier} acquired spell must reach the scheduled opposing actor:\n{}", trace.join("\n"));
            campaign.reset();
            assert_ne!(campaign.simulation().shared_spells().local_player().enabled_spell_holders[13], 0,
                "reset restores the carried inventory at entry");
        }
    }

    #[test]
    fn authored_duel_pickup_and_public_hand_reach_real_nyphur_tether() {
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        use crate::combat::actors::PlayerEffectLane;

        let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let mut source = CampaignSession::open(
            base.join("mc-paj2-source-secret-34-campaign-package"),
            CampaignSelection::secret_destination(34).unwrap(),
        ).expect("unfiltered secret internal34 owner package opens");
        let player = source.simulation().state.entity_index;
        let feature = source.simulation().session.state.live.mission.authored_features().iter()
            .find(|feature| feature.index == 755).copied().expect("authored Duel row755");
        assert_eq!((feature.kind, feature.subtype, feature.activation_group, feature.stage_tag),
            (15, 14, 23, 1));
        // Controlled arrival at the real source group, without filtering its
        // other authored constructors or manufacturing the spell pickup.
        source.simulation_mut().activate_authored_group_prerequisite_for_test(23)
            .expect("real secret group23 publishes its authored Duel pickup");
        let pickup = *source.simulation().shared_spells().authored_pickup_slots().get(&755)
            .expect("row755 allocated an actual source pickup");
        let pickup_row = source.simulation().shared_spells().pickup(pickup)
            .expect("authored Duel pickup has its live spell carrier");
        assert_eq!((pickup_row.class, pickup_row.model, pickup_row.action), (15, 14, 43));
        for _ in 0..12 {
            let simulation = source.simulation_mut();
            let position = simulation.shared_spells().pickup(pickup).unwrap().position;
            simulation.set_level2_entity_position(player, position).unwrap();
            simulation.step_outer_turn(crate::PlayerCommand::default(), 0);
            assert!(simulation.dispatch_events().iter().all(|event|
                !matches!(event, super::super::Mc2DispatchEvent::Error { .. })),
                "Duel pickup dispatch errors: {:?}", simulation.dispatch_events());
            if simulation.shared_spells().local_player().enabled_spell_holders[14] != 0 { break; }
        }
        assert_ne!(source.simulation().shared_spells().local_player().enabled_spell_holders[14], 0,
            "ordinary overlap must acquire the real Duel pickup");
        let learned = source.simulation().shared_spells().local_player().enabled_spell_holders
            .iter().enumerate().filter_map(|(family, holder)|
                (*holder != 0).then_some(family as u8)).collect::<Vec<_>>();

        for tier in 0_u8..=2 {
            // Secret34 has no rival; carry the acquired inventory to an
            // unfiltered Level2 entry. Resources and combat poses are controls.
            let mut campaign = CampaignSession::open(
                base.join("mc-bhq6-ordinary-20261025-zclu5fjj/level2"), CampaignSelection::LEVEL2,
            ).expect("unfiltered Level2 opens");
            assert_eq!(campaign.rebuild_for_spell_carry(&learned, Some(14)).unwrap(), Some(14));
            let simulation = campaign.simulation_mut();
            let caster = simulation.state.entity_index;
            let target = simulation.session.state.live.economy.entities.iter()
                .find(|row| row.active && (row.class, row.model, row.action) == (3, 1, 1))
                .expect("real Level2 rival").index;
            let target_handle = simulation.session.state.live.economy.handle(target).unwrap();
            let castle = scheduled_castle_from_source_flyer(simulation, caster);
            for _ in 0..256 {
                let live = &simulation.session.state.live;
                let row = &live.economy.entities[usize::from(target)];
                assert_eq!(live.economy.handle(target), Some(target_handle));
                if live.nyphur_dispatcher.get(target_handle).unwrap().common_upkeep(row.phase).word_0x159 == 0 { break; }
                simulation.step_outer_turn(crate::PlayerCommand::default(), 0);
                assert!(simulation.dispatch_events().iter().all(|event|
                    !matches!(event, super::super::Mc2DispatchEvent::Error { .. })));
            }
            {
                let live = &simulation.session.state.live;
                let row = &live.economy.entities[usize::from(target)];
                assert_eq!(live.nyphur_dispatcher.get(target_handle).unwrap().common_upkeep(row.phase).word_0x159, 0,
                    "ordinary rival visits expire protection");
            }
            let rule = crate::level_definition::DUEL_TIERS[usize::from(tier)];
            {
                let live = &mut simulation.session.state.live;
                assert!(live.economy.economy.set_castle_mana_prerequisite_for_test(castle,
                    rule.castle_mana_required.max(rule.mana_cost) + 10_000));
                let parent = &mut live.economy.entities[usize::from(caster)];
                parent.max_mana = rule.mana_cost + 10_000;
                parent.record[144..148].copy_from_slice(&parent.max_mana.to_le_bytes());
                parent.set_mana(parent.max_mana);
                parent.set_mana_regen(0);
                let parent = parent.clone();
                live.shared_spells.sync_player_entity_from_authority(&parent);
                live.shared_spells.player_mana = parent.mana;
                live.shared_spells.caster.mana = parent.mana;
                live.shared_spells.sync_typed_spell_parent_caches_from_player_entity();
                if let Some(fire) = live.core.context.spell_fire.as_mut() { fire.caster.mana = parent.mana; }
                let target_row = &mut live.economy.entities[usize::from(target)];
                target_row.max_mana = 10_000;
                target_row.record[144..148].copy_from_slice(&10_000_i32.to_le_bytes());
                target_row.set_mana(8_000);
                target_row.set_mana_regen(0);
                let target_row = target_row.clone();
                assert!(live.nyphur_dispatcher.get_mut_in_arena(&live.economy, target).unwrap()
                    .sync_support_parent_resources(&target_row, false));
                live.economy.economy.refresh_owner_mana_rows();
            }
            let ui = crate::SOURCE_SPELL_INDEX_MAP.iter().position(|family| *family == 14).unwrap() as i8;
            simulation.step_outer_turn(crate::PlayerCommand { opcode: 31, argument_1: ui,
                argument_2: tier as i8, ..Default::default() }, 0);
            let holder = simulation.shared_spells().local_player().enabled_spell_holders[14];
            assert_eq!(simulation.shared_spells().pickup(holder).unwrap().level_gate, tier as i8);
            {
                let live = &mut simulation.session.state.live;
                let parent = live.economy.entities[usize::from(caster)].clone();
                let actor = area_damage::source_actor(&mut live.economy, None, target).unwrap();
                let mut position = parent.position;
                move_entity(&mut position, parent.yaw, parent.pitch, 1_536);
                position.z = parent.position.z.wrapping_add(parent.array_fov())
                    .wrapping_sub(actor.shape.vertical_offset);
                live.economy.entities[usize::from(target)].set_position(position);
                crate::world::effects::sync_position(&mut live.economy, &mut live.terrain,
                    target, position, true, true).unwrap();
            }
            let xp_before = simulation.shared_spells().spell_experience()[14];
            let mut prior_mana = simulation.session.state.live.economy.entities[usize::from(caster)].mana;
            let mut prior_lane = None;
            let mut saw_flyer = false;
            let mut saw_field = false;
            let mut saw_lane = false;
            let mut saw_tether = false;
            let mut charged = false;
            let mut launch_sound = false;
            let mut presented = false;
            let mut stages = [None; 2];
            let mut hidden = [false; 2];
            let mut reclaimed = [false; 2];
            let mut trace = Vec::new();
            for turn in 0..96 {
                simulation.step_outer_turn(if turn == 0 { crate::PlayerCommand { flags: 0x10, ..Default::default() } }
                    else { crate::PlayerCommand::default() }, 0);
                let live = &simulation.session.state.live;
                let caster_mana = live.economy.entities[usize::from(caster)].mana;
                charged |= prior_mana - caster_mana == rule.mana_cost;
                prior_mana = caster_mana;
                let products = live.economy.entities.iter().filter(|row| row.active && row.source_id == caster
                    && matches!((row.class, row.model), (9, 7) | (10, 26)))
                    .map(|row| (row.index, row.class, row.model, row.action, row.life, row.drawing_disabled()))
                    .collect::<Vec<_>>();
                saw_flyer |= products.iter().any(|row| (row.1, row.2) == (9, 7));
                saw_field |= products.iter().any(|row| (row.1, row.2) == (10, 26));
                for product in &products {
                    let stage = if product.1 == 9 { 0 } else { 1 };
                    stages[stage].get_or_insert((product.0, live.economy.economy.slot_generation(product.0)));
                }
                for (stage, saved) in stages.iter().enumerate() {
                    if let Some((slot, generation)) = *saved {
                        let row = &live.economy.entities[usize::from(slot)];
                        let same = live.economy.economy.slot_generation(slot) == generation;
                        hidden[stage] |= same && row.active && row.drawing_disabled();
                        reclaimed[stage] |= !same || live.economy.economy.free_source_slots().contains(&slot);
                    }
                }
                for event in live.economy.economy.events() {
                    if let super::super::Level2EconomyEvent::PreparedSound { source_entity_index, entity_index, sound_id: 9, .. } = event {
                        launch_sound |= *entity_index == caster
                            && products.iter().any(|product| product.0 == *source_entity_index && product.1 == 9);
                    }
                }
                for view in simulation.campaign_entity_presentations(0).expect("Duel sprite projection") {
                    let row = &live.economy.entities[usize::from(view.entity_index)];
                    if row.source_id == caster && matches!((row.class, row.model), (9, 7) | (10, 26)) {
                        assert_eq!((view.source_id, view.position, view.render_parameter_index),
                            (caster, row.position, DUEL_PARTICLE));
                        assert_eq!(view.base_tmap, simulation.session.definition.presentation.particles
                            [usize::from(DUEL_PARTICLE)].base_tmap);
                        presented = true;
                    }
                }
                let victim = &live.economy.entities[usize::from(target)];
                let lane = victim.pending_player_effect(PlayerEffectLane::Duel);
                saw_lane |= lane.is_some_and(|hit| hit.attacker == caster && hit.payload == i32::from(tier));
                saw_tether |= live.economy.handle(target) == Some(target_handle)
                    && prior_lane.is_some_and(|hit: crate::combat::actors::PlayerEffectHit|
                        hit.attacker == caster && hit.payload == i32::from(tier))
                    && lane.is_none() && live.core.state.tether_target_index == target
                    && live.core.state.tether_holder_index == holder
                    && live.core.state.tether_tier == u16::from(tier)
                    && simulation.shared_spells().spell_experience()[14] > xp_before;
                prior_lane = lane;
                let errors = simulation.dispatch_events().iter().filter(|event|
                    matches!(event, super::super::Mc2DispatchEvent::Error { .. })).collect::<Vec<_>>();
                trace.push(format!("turn={turn} products={products:?} lane={lane:?} tether={} holder={} victim=({},{}) xp={} errors={errors:?}",
                    live.core.state.tether_target_index, live.shared_spells.pickup(holder).unwrap().active_value,
                    victim.life, victim.mana, live.shared_spells.spell_experience()[14]));
                assert!(errors.is_empty(), "tier {tier}: {}", trace.join("\n"));
                if saw_flyer && saw_field && saw_lane && saw_tether && charged && launch_sound && presented
                    && reclaimed.iter().all(|value| *value) { break; }
            }
            assert!(saw_flyer && saw_field && saw_lane && saw_tether && charged && launch_sound && presented
                    && hidden.iter().all(|value| *value) && reclaimed.iter().all(|value| *value),
                "tier {tier} real Duel cast must produce its tether and clean up: {}", trace.join("\n"));

            // Move the actual rival within source range so the live flight
            // callback must apply a measurable tether displacement/drain.
            let parent = simulation.session.state.live.economy.entities[usize::from(caster)].clone();
            let mut position = parent.position;
            move_entity(&mut position, parent.yaw, parent.pitch, 4_096);
            simulation.set_level2_entity_position(target, position).unwrap();
            let mut untethered = simulation.clone();
            untethered.session.state.live.core.state.tether_target_index = 0;
            let victim_before = simulation.session.state.live.economy.entities[usize::from(target)].clone();
            simulation.step_outer_turn(crate::PlayerCommand::default(), 0);
            untethered.step_outer_turn(crate::PlayerCommand::default(), 0);
            let live = &simulation.session.state.live;
            let victim_after = &live.economy.entities[usize::from(target)];
            assert_eq!(live.core.state.tether_target_index, target, "the live tether remains bound");
            assert_ne!(live.core.state.position, untethered.session.state.live.core.state.position,
                "the real caster flight consumer applies tether movement");
            if tier >= 1 { assert!(victim_after.mana < victim_before.mana, "tier {tier} drains rival mana"); }
            if tier == 2 { assert!(victim_after.life < victim_before.life, "tier2 drains rival life"); }

            if tier == 2 {
                // The caster's current A4 holder selector is authoritative
                // on every visit, even if the previously bound holder lives.
                let mut no_current_holder = simulation.clone();
                {
                    let live = &mut no_current_holder.session.state.live;
                    assert!(live.shared_spells.pickup(holder).unwrap().active_value > 0);
                    assert!(live.shared_spells.local_player.set_enabled_spell_holder_index(14, 0));
                    for player in live.shared_spells.players.iter_mut()
                        .chain(live.core.players.iter_mut()).filter(|player| player.entity_index == caster)
                    {
                        assert!(player.set_enabled_spell_holder_index(14, 0));
                    }
                }
                no_current_holder.step_outer_turn(crate::PlayerCommand::default(), 0);
                assert!(no_current_holder.dispatch_events().iter().all(|event|
                    !matches!(event, super::super::Mc2DispatchEvent::Error { .. })));
                assert_eq!(no_current_holder.session.state.live.core.state.tether_target_index, 0,
                    "a cleared current A4 holder releases the actual live tether");
                assert!(no_current_holder.shared_spells().pickup(holder).unwrap().active_value > 0,
                    "the formerly selected holder remains active during the control");

                // Preserve the actual acquired/cast tether. This control
                // places the caster airborne in source action2, then compares
                // its real dying flight visit with an otherwise identical
                // unbound visit. No new tether or mailbox is fabricated.
                let mut dying_caster = simulation.clone();
                let mut high = dying_caster.session.state.live.core.state.position;
                high.z = dying_caster.session.state.live.terrain.ground_height(high).wrapping_add(4_000);
                dying_caster.set_level2_entity_position(caster, high).unwrap();
                let mut nearby = high;
                move_entity(&mut nearby, dying_caster.session.state.live.core.state.yaw, 0, 2_048);
                dying_caster.set_level2_entity_position(target, nearby).unwrap();
                {
                    let live = &mut dying_caster.session.state.live;
                    let row = &mut live.economy.entities[usize::from(caster)];
                    row.set_source_action(2);
                    row.set_life(-1);
                    row.set_source_word_2c(-10);
                    live.player_life = -1;
                    let row = row.clone();
                    live.shared_spells.sync_player_entity_from_authority(&row);
                }
                let mut unbound_dying = dying_caster.clone();
                unbound_dying.session.state.live.core.state.tether_target_index = 0;
                dying_caster.step_outer_turn(crate::PlayerCommand::default(), 0);
                unbound_dying.step_outer_turn(crate::PlayerCommand::default(), 0);
                assert!(dying_caster.dispatch_events().iter().all(|event|
                    !matches!(event, super::super::Mc2DispatchEvent::Error { .. })));
                let live = &dying_caster.session.state.live;
                let victim = &live.economy.entities[usize::from(target)];
                let unbound_victim = &unbound_dying.session.state.live.economy.entities[usize::from(target)];
                assert_eq!(live.economy.entities[usize::from(caster)].action, 2,
                    "the airborne source caster remains in the dying flight action");
                assert_eq!(live.core.state.tether_target_index, target);
                assert!(victim.mana < unbound_victim.mana && victim.life < unbound_victim.life,
                    "actual dying-flight tether publishes tier2 mana and life drain");
            }

            // A separate controlled damage pulse uses the actual rival death
            // callback; the caster must release that dying target naturally.
            let mut dying = simulation.clone();
            let damage = u16::try_from(dying.session.state.live.economy.entities[usize::from(target)].life + 1)
                .expect("rival life fits a source damage pulse");
            area_damage::contact(&mut dying.session.state.live.economy, None, target, damage, caster).unwrap();
            let mut saw_dead = false;
            for _ in 0..8 {
                dying.step_outer_turn(crate::PlayerCommand::default(), 0);
                assert!(dying.dispatch_events().iter().all(|event|
                    !matches!(event, super::super::Mc2DispatchEvent::Error { .. })));
                let live = &dying.session.state.live;
                saw_dead |= live.economy.handle(target) == Some(target_handle)
                    && live.economy.entities[usize::from(target)].life < 0;
                if saw_dead && live.core.state.tether_target_index == 0 { break; }
            }
            assert!(saw_dead && dying.session.state.live.core.state.tether_target_index == 0,
                "real rival death must clear the caster's Duel tether");

            // Move beyond the source range; the ordinary flight visit clears
            // the tether and the holder's post-28-pulse abort reaches zero.
            let parent = live.economy.entities[usize::from(caster)].clone();
            let mut far = parent.position;
            move_entity(&mut far, parent.yaw, 0, 12_000);
            simulation.set_level2_entity_position(target, far).unwrap();
            for _ in 0..64 {
                simulation.step_outer_turn(crate::PlayerCommand::default(), 0);
                assert!(simulation.dispatch_events().iter().all(|event|
                    !matches!(event, super::super::Mc2DispatchEvent::Error { .. })));
                if simulation.session.state.live.core.state.tether_target_index == 0
                    && simulation.shared_spells().pickup(holder).unwrap().active_value == 0 { break; }
            }
            assert_eq!(simulation.session.state.live.core.state.tether_target_index, 0,
                "range exit clears the caster's source tether");
            assert_eq!(simulation.shared_spells().pickup(holder).unwrap().active_value, 0,
                "the unbound holder naturally completes its source abort");
            campaign.reset();
            assert_ne!(campaign.simulation().shared_spells().local_player().enabled_spell_holders[14], 0,
                "reset restores the actual carried Duel holder");
        }
    }

    #[test]
    fn duel_holder_reads_caster_target_only_for_an_admitted_late_pulse() {
        for (phase, target, admitted, expected, spawns) in [
            ("idle", None, true, Some(0), false),
            ("fresh", None, true, Some(194), true),
            ("early", None, true, Some(167), false),
            ("late", None, true, None, false),
            ("late", Some(false), true, Some(0), false),
            ("late", Some(true), true, Some(166), false),
            ("late", None, false, Some(0), false),
        ] {
            let (mut simulation, holder_slot, parent_slot) = ready_holder(DUEL_FAMILY, 0);
            let particles = simulation.session.definition.presentation.particles.clone();
            let live = &mut simulation.session.state.live;
            let holder = live.shared_spells.pickup_mut(holder_slot).unwrap();
            assert_eq!(holder.activation_value, 195);
            holder.active_value = match phase {
                "idle" => 0,
                "fresh" => 195,
                "early" => 168,
                "late" => 167,
                _ => unreachable!(),
            };
            holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
            if !admitted {
                live.economy.entities[usize::from(parent_slot)].set_mana(-1);
            }
            let outcome = step_holder(
                &mut live.shared_spells,
                &mut live.economy,
                &mut live.terrain,
                &particles,
                holder_slot,
                DirectionalCastInput {
                    projectile: CastInput {
                        castle_mana: Some(i32::MAX),
                        ..Default::default()
                    },
                    duel_target_active: target,
                },
            );
            if let Some(expected) = expected {
                let outcome = outcome.expect("source phase has the authority it needs");
                assert_eq!(outcome.projectile.is_some(), spawns, "{phase}");
                assert_eq!(
                    live.shared_spells.pickup(holder_slot).unwrap().active_value,
                    expected,
                    "{phase} target={target:?} admitted={admitted}"
                );
            } else {
                assert!(outcome.is_err(), "the admitted late pulse needs caster A4");
            }
        }
    }

    #[test]
    fn missing_or_replaced_parent_does_not_cast_but_holder_timers_advance() {
        for family in [STEAL_MANA_FAMILY, DUEL_FAMILY] {
            for replaced in [false, true] {
                let (mut simulation, holder_slot, parent_slot) = ready_holder(family, 0);
                let particles = simulation.session.definition.presentation.particles.clone();
                let live = &mut simulation.session.state.live;
                let holder = live.shared_spells.pickup_mut(holder_slot).unwrap();
                let active_before = holder.active_value;
                put(holder, 58, 3);
                let parent = &mut live.economy.entities[usize::from(parent_slot)];
                if replaced {
                    parent.class = 5;
                    parent.record[67] = 5;
                } else {
                    parent.active = false;
                }
                let regen_before = parent.mana_regen;
                let outcome = step_holder(
                    &mut live.shared_spells,
                    &mut live.economy,
                    &mut live.terrain,
                    &particles,
                    holder_slot,
                    DirectionalCastInput {
                        projectile: CastInput {
                            castle_mana: Some(i32::MAX),
                            ..Default::default()
                        },
                        duel_target_active: None,
                    },
                )
                .unwrap();
                assert!(outcome.projectile.is_none());
                assert_eq!(
                    live.economy.entities[usize::from(parent_slot)].mana_regen,
                    regen_before
                );
                let holder = live.shared_spells.pickup(holder_slot).unwrap();
                assert_eq!(holder.active_value, active_before - 1);
                assert_eq!(word(holder, 58), 2);
            }
        }
    }

    #[test]
    fn directional_fields_ignore_other_mode_selectors_and_use_source_radius() {
        for family in [STEAL_MANA_FAMILY, DUEL_FAMILY] {
            for narrow in [false, true] {
                let (mut simulation, holder, caster) = ready_holder(family, 0);
                let projectile = cast(&mut simulation, holder, false);
                let (target, field) =
                    fly_into_actual_opposing_player(&mut simulation, projectile, caster);
                let live = &mut simulation.session.state.live;
                deactivate_other_actors(&mut live.economy, &[caster, target]);
                let actor = area_damage::source_actor(&mut live.economy, None, target).unwrap();
                let target_position = Position {
                    x: actor.position.x & 0xff00,
                    y: actor.position.y & 0xff00,
                    ..actor.position
                };
                live.economy.entities[usize::from(target)].set_position(target_position);
                crate::world::effects::sync_position(
                    &mut live.economy,
                    &mut live.terrain,
                    target,
                    target_position,
                    true,
                    true,
                )
                .unwrap();
                let row = &mut live.economy.entities[usize::from(field)];
                row.record[69] = 5;
                row.record[70] = 29;
                if narrow {
                    assert!(actor.shape.half_width > 1);
                    row.record[88..90].copy_from_slice(&(-1_i16).to_le_bytes());
                }
                row.set_position(Position {
                    z: target_position
                        .z
                        .wrapping_add(actor.shape.vertical_offset)
                        .wrapping_sub(row.array_yaw()),
                    ..target_position
                });
                let outcome = step_field(&mut live.economy, field).unwrap();
                assert_eq!(
                    outcome.accepted_target_slots,
                    [target],
                    "source mode for family {family}, narrow={narrow}"
                );
            }
        }
    }

    #[test]
    fn steal_mana_cast_hits_and_transfers_from_live_nyphur() {
        let (mut simulation, holder_slot, parent_slot) = ready_holder(STEAL_MANA_FAMILY, 0);
        let projectile_slot = cast(&mut simulation, holder_slot, false);
        let particles = simulation.session.definition.presentation.particles.clone();
        let target_slot = {
            let live = &mut simulation.session.state.live;
            let position = source_probe_eligible_target_position(
                &live.economy.entities[usize::from(projectile_slot)],
                4,
            );
            let target = live
                .economy
                .entities
                .iter_mut()
                .find(|row| {
                    row.active && (row.class, row.model) == (3, 1) && row.source_id != parent_slot
                })
                .expect("campaign has a live rival Nyphur");
            let slot = target.index;
            target.set_position(position);
            target.set_life(100);
            target.max_mana = 10_000;
            target.record[144..148].copy_from_slice(&10_000_i32.to_le_bytes());
            target.set_mana(8_000);
            crate::world::effects::sync_position(
                &mut live.economy,
                &mut live.terrain,
                slot,
                position,
                true,
                true,
            )
            .unwrap();
            slot
        };
        let (_, field_slot) = fly_projectile_into_target_slot(
            &mut simulation,
            projectile_slot,
            target_slot,
            3,
            particles,
        );
        let live = &mut simulation.session.state.live;
        deactivate_other_actors(&mut live.economy, &[parent_slot, target_slot]);
        assert_eq!(
            step_field(&mut live.economy, field_slot)
                .unwrap()
                .accepted_target_slots,
            [target_slot]
        );
        assert!(live.economy.entities[usize::from(target_slot)].submit_player_effect_contact(
            crate::combat::actors::PlayerEffectLane::Duel, 0, parent_slot, true));
        let mut flight = crate::FlightState::default();
        let mut callback_order = Vec::new();
        let result = consume_actor_mailbox_with_feedback(
            &mut live.shared_spells,
            &mut live.economy,
            &mut live.terrain,
            target_slot,
            &[],
            |_, arena, feedback| {
                match feedback {
                    DirectionalFeedback::Duel(binding) => {
                        install_duel_tether_binding(&mut flight, binding);
                        callback_order.push("duel");
                    }
                    DirectionalFeedback::Steal(owner) => {
                        assert_eq!(owner, parent_slot);
                        assert_eq!(flight.tether_target_index, target_slot,
                            "Duel is published before Steal consumption");
                        assert_eq!(arena.entities[usize::from(target_slot)].mana, 8_000,
                            "Steal feedback precedes the actual transfer");
                        callback_order.push("steal");
                    }
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(callback_order, ["duel", "steal"]);
        assert_eq!(result.steal_mana.unwrap().result.fixed_transfer, 2_000);
        assert_eq!(live.economy.entities[usize::from(target_slot)].mana, 6_000);
    }

    #[test]
    fn steal_mana_cast_writes_real_contact_lane_and_transfers_target_mana() {
        let (mut simulation, holder_slot, parent_slot) = ready_holder(STEAL_MANA_FAMILY, 0);
        let projectile_slot = cast(&mut simulation, holder_slot, false);
        let flyer =
            simulation.session.state.live.economy.entities[usize::from(projectile_slot)].clone();
        let parent =
            simulation.session.state.live.economy.entities[usize::from(parent_slot)].clone();
        let holder_mana = simulation
            .session
            .state
            .live
            .shared_spells
            .pickup(holder_slot)
            .unwrap()
            .mana;
        assert_eq!((flyer.class, flyer.model, flyer.action), (9, 8, 8));
        assert_eq!(flyer.parameter_relation, Some(STEAL_MANA_PARAMETER));
        assert_eq!(flyer.particle_parameter_index, Some(STEAL_MANA_PARTICLE));
        assert_eq!(
            u32::from_le_bytes(flyer.record[164..168].try_into().unwrap()),
            u32::from(STEAL_MANA_PARAMETER + 1)
        );
        assert_eq!(
            flyer.max_life,
            PROJECTILE_RANGE / i32::from(PROJECTILE_SPEED)
        );
        assert_eq!(flyer.life, flyer.max_life);
        assert_eq!(flyer.mana, holder_mana);
        assert_eq!(
            (flyer.source_id, word(&flyer, 42)),
            (parent_slot, holder_slot)
        );
        assert_eq!(&flyer.record[71..73], &[10, STEAL_MANA_FIELD_MODEL]);
        assert_eq!(flyer.record[74], 0);
        assert_eq!(flyer.source_dword_10(), 7);
        assert_eq!(flyer.minimum_speed(), PROJECTILE_SPEED);
        assert_eq!(
            flyer.actual_speed,
            PROJECTILE_SPEED
                .wrapping_add(parent.actual_speed)
                .clamp(PROJECTILE_SPEED, PROJECTILE_RANGE as i16)
        );
        let mut expected_axis = parent.position;
        move_entity(&mut expected_axis, parent.yaw, parent.pitch, 0x4000);
        assert_eq!(source_axis(&flyer), expected_axis);

        let (target_slot, field_slot) =
            fly_into_actual_opposing_player(&mut simulation, projectile_slot, parent_slot);
        let terminal =
            simulation.session.state.live.economy.entities[usize::from(field_slot)].clone();
        assert_eq!(
            (terminal.class, terminal.model, terminal.action),
            (10, 25, 25)
        );
        assert_eq!(
            (terminal.max_life, terminal.life),
            (FIELD_LIFETIME, FIELD_LIFETIME)
        );
        assert_eq!(terminal.source_id, parent_slot);
        assert_eq!(terminal.record[74], 0);
        assert_eq!(
            terminal.particle_parameter_index,
            Some(0),
            "NewEvent retains its source-default particle table entry for model25"
        );
        assert_eq!(
            terminal.subspell_index, 100,
            "model25's constructor value is replaced by sub_65820's actual flyer payload"
        );
        let live = &mut simulation.session.state.live;
        deactivate_other_actors(&mut live.economy, &[parent_slot, target_slot]);

        let field = step_field(&mut live.economy, field_slot).unwrap();
        assert_eq!(field.accepted_target_slots, [target_slot]);
        assert_eq!(
            live.economy.entities[usize::from(target_slot)]
                .pending_player_effect(crate::combat::actors::PlayerEffectLane::StealMana)
                .unwrap()
                .payload,
            0
        );

        let caster_mana_before = live.economy.entities[usize::from(parent_slot)].mana;
        let caster_max_mana = live.economy.entities[usize::from(parent_slot)].max_mana;
        let result = consume_actor_mailbox(
            &mut live.shared_spells,
            &mut live.economy,
            &mut live.terrain,
            target_slot,
            &[],
        )
        .unwrap();
        assert!(result.steal_consumed && result.steal_attacker_resolved);
        let consumption = result.steal_mana.expect("real actor mailbox was consumed");
        assert_eq!(consumption.caster_entity_index, parent_slot);
        assert!(consumption.result.experience_awarded);
        assert_eq!(consumption.result.fixed_transfer, 2_000);
        assert_eq!(
            live.economy.entities[usize::from(parent_slot)].mana,
            caster_mana_before
                .wrapping_add(2_000)
                .clamp(0, caster_max_mana.max(0))
        );
        assert_eq!(live.economy.entities[usize::from(target_slot)].mana, 6_000);
        assert!(
            live.economy.entities[usize::from(target_slot)]
                .pending_player_effect(crate::combat::actors::PlayerEffectLane::StealMana)
                .is_none()
        );
        let field = step_field(&mut live.economy, field_slot).unwrap();
        assert!(
            field.drawing_disabled,
            "the contacted field reaches its reclaim tail"
        );
        live.economy
            .rebuild_source_arena_with_terrain(&mut live.terrain, &[])
            .unwrap();
        assert_eq!(live.economy.entities[usize::from(field_slot)].class, 0);


    }

    #[test]
    fn duel_cast_consumes_lane_into_existing_tether_and_reclaims_field() {
        let (mut simulation, holder_slot, parent_slot) = ready_holder(DUEL_FAMILY, 0);
        let projectile_slot = cast(&mut simulation, holder_slot, false);
        let flyer =
            simulation.session.state.live.economy.entities[usize::from(projectile_slot)].clone();
        let parent =
            simulation.session.state.live.economy.entities[usize::from(parent_slot)].clone();
        let holder_mana = simulation
            .session
            .state
            .live
            .shared_spells
            .pickup(holder_slot)
            .unwrap()
            .mana;
        assert_eq!((flyer.class, flyer.model, flyer.action), (9, 7, 7));
        assert_eq!(flyer.parameter_relation, Some(DUEL_PARAMETER));
        assert_eq!(flyer.particle_parameter_index, Some(DUEL_PARTICLE));
        assert_eq!(
            u32::from_le_bytes(flyer.record[164..168].try_into().unwrap()),
            u32::from(DUEL_PARAMETER + 1)
        );
        assert_eq!(flyer.actual_speed, PROJECTILE_SPEED);
        assert_eq!(flyer.minimum_speed(), PROJECTILE_SPEED);
        assert_eq!(
            flyer.max_life,
            PROJECTILE_RANGE / i32::from(PROJECTILE_SPEED)
        );
        assert_eq!(flyer.mana, holder_mana);
        assert_eq!(
            (flyer.source_id, word(&flyer, 42)),
            (parent_slot, holder_slot)
        );
        assert_eq!(flyer.subspell_index, 5_170);
        assert_eq!(&flyer.record[71..73], &[10, DUEL_FIELD_MODEL]);
        assert_eq!(flyer.record[74], 0);
        let mut expected_axis = Position::default();
        move_entity(&mut expected_axis, parent.yaw, parent.pitch, 10_240);
        assert_eq!(source_axis(&flyer), expected_axis, "6B610 moves the NewEvent-zeroed axis");

        let (target_slot, field_slot) =
            fly_into_target(&mut simulation, projectile_slot, parent_slot, 3);
        let live = &mut simulation.session.state.live;
        let field_row = live.economy.entities[usize::from(field_slot)].clone();
        assert_eq!(
            (field_row.class, field_row.model, field_row.action),
            (10, 26, 26)
        );
        assert_eq!(field_row.particle_parameter_index, Some(DUEL_PARTICLE));
        assert_eq!(field_row.max_life, FIELD_LIFETIME);
        assert_eq!(field_row.subspell_index, 5_170);
        deactivate_other_actors(&mut live.economy, &[parent_slot, target_slot]);

        let produced = step_field(&mut live.economy, field_slot).unwrap();
        assert_eq!(produced.accepted_target_slots, [target_slot]);
        let consumed = consume_actor_mailbox(
            &mut live.shared_spells,
            &mut live.economy,
            &mut live.terrain,
            target_slot,
            &[],
        )
        .unwrap();
        assert!(consumed.duel_consumed);
        let tether = consumed
            .duel
            .expect("Duel mailbox produces the existing FlightState tether binding");
        // Fixture arithmetic for pinned sub_5DE30 -> sub_583F0_distance_3d:
        // caster=(384, 62080, 256); four 384-unit forecast steps put the
        // flyer at (384, 60544, 356), and its probe-cell target at
        // (512, 60672, 356). Deltas (128, -1408, 100) give 2,008,848;
        // 1417² <= that value < 1418², so sub_7277A_radix_3d returns 1417.
        let mut flight = crate::FlightState::default();
        install_duel_tether_binding(&mut flight, tether);
        assert_eq!(flight.tether_target_index, target_slot);
        assert_eq!(flight.tether_holder_index, holder_slot);
        assert_eq!(flight.tether_prior_distance, 1_417);
        assert_eq!(flight.tether_tier, 0);
        assert_eq!(
            (
                tether.caster_entity_index,
                tether.target_entity_index,
                tether.holder_entity_index
            ),
            (parent_slot, target_slot, holder_slot)
        );
        assert_eq!(tether.tier, 0);
        assert_eq!(tether.prior_distance, 1_417);

        // A hit arriving after the holder expired still writes the source
        // relation. The ordinary flight consumer clears it on its next pass.
        live.shared_spells
            .pickup_mut(holder_slot)
            .unwrap()
            .active_value = 0;
        assert!(
            live.economy.entities[usize::from(target_slot)].submit_player_effect_contact(
                crate::combat::actors::PlayerEffectLane::Duel,
                0,
                parent_slot,
                true,
            )
        );
        let expired = consume_actor_mailbox(
            &mut live.shared_spells,
            &mut live.economy,
            &mut live.terrain,
            target_slot,
            &[],
        )
        .unwrap()
        .duel
        .expect("the hit still publishes the relation");
        assert_eq!(expired.holder_entity_index, holder_slot);
        install_duel_tether_binding(&mut flight, expired);
        let target = &live.economy.entities[usize::from(target_slot)];
        let mut context = crate::TetherContext {
            holder: Some(crate::SpellHolder {
                entity_index: holder_slot,
                active_value: 0,
            }),
            target: Some(crate::TetherTarget {
                entity_index: target_slot,
                position: target.position,
                life: target.life,
                mana: target.mana,
                mana_regen: target.mana_regen,
                life_regen: 0,
            }),
        };
        let position = live.economy.entities[usize::from(parent_slot)].position;
        let mut proposed = position;
        crate::apply_tether(&mut flight, position, &mut proposed, &mut context, crate::TETHER_SPEED_LIMIT);
        assert_eq!(flight.tether_target_index, 0);

        deactivate_other_actors(&mut live.economy, &[parent_slot]);
        let field = step_field(&mut live.economy, field_slot).unwrap();
        assert_eq!(field.life_after, -1);
        assert!(!field.drawing_disabled);
        let field = step_field(&mut live.economy, field_slot).unwrap();
        assert!(
            field.drawing_disabled,
            "the field becomes reclaimable after its final pass"
        );
        live.economy
            .rebuild_source_arena_with_terrain(&mut live.terrain, &[])
            .unwrap();
        assert_eq!(live.economy.entities[usize::from(field_slot)].class, 0);
        // Repeat the actual holder -> flyer -> field -> mailbox path with
        // the qualified rival as caster and the real local actor as target.
        // Its A4 state belongs to the rival's generation-bound dispatcher.
        let (mut simulation, holder_slot, local_slot) = ready_holder(DUEL_FAMILY, 0);
        let caster_slot = {
            let live = &mut simulation.session.state.live;
            let caster_slot = live.shared_spells.players.iter()
                .find(|player| player.entity_index != local_slot && player.is_ai)
                .expect("qualified rival player")
                .entity_index;
            assert!(live.nyphur_dispatcher.get_in_arena(&live.economy, caster_slot).is_some());
            let holder = live.shared_spells.pickup_mut(holder_slot).unwrap();
            holder.record[44..46].copy_from_slice(&caster_slot.to_le_bytes());
            let caster = &mut live.economy.entities[usize::from(caster_slot)];
            caster.set_life(100);
            caster.max_mana = 10_000;
            caster.record[144..148].copy_from_slice(&10_000_i32.to_le_bytes());
            caster.set_mana(10_000);
            let player = live.shared_spells.players.iter_mut()
                .find(|player| player.entity_index == caster_slot).unwrap();
            player.enabled_spell_holders[usize::from(DUEL_FAMILY)] = holder_slot;
            caster_slot
        };
        let projectile = cast(&mut simulation, holder_slot, false);
        let (target_slot, field_slot) =
            fly_into_actual_opposing_player(&mut simulation, projectile, caster_slot);
        assert_eq!(target_slot, local_slot);
        let live = &mut simulation.session.state.live;
        deactivate_other_actors(&mut live.economy, &[caster_slot, target_slot]);
        let field = step_field(&mut live.economy, field_slot).unwrap();
        assert_eq!(field.accepted_target_slots, [target_slot]);
        let binding = consume_actor_mailbox(
            &mut live.shared_spells,
            &mut live.economy,
            &mut live.terrain,
            target_slot,
            &[],
        ).unwrap().duel.expect("rival contact produces a caster binding");
        assert_eq!(
            (binding.caster_entity_index, binding.target_entity_index, binding.holder_entity_index),
            (caster_slot, target_slot, holder_slot)
        );
        let rival = live.nyphur_dispatcher.get_mut_in_arena(&live.economy, caster_slot)
            .expect("current rival generation owns its A4 carrier");
        assert!(rival.install_duel_tether(binding));
        assert!(rival.has_duel_target());
        assert_eq!(rival.duel_tether().unwrap().target_entity_index, target_slot);
        assert_eq!(live.core.state.tether_target_index, 0, "rival link does not overwrite local A4");
    }

    #[test]
    fn dead_actor_keeps_the_real_pending_mana_lane_for_a_later_callback() {
        let (mut simulation, holder_slot, parent_slot) = ready_holder(STEAL_MANA_FAMILY, 0);
        let projectile_slot = cast(&mut simulation, holder_slot, false);
        let (target_slot, field_slot) =
            fly_into_target(&mut simulation, projectile_slot, parent_slot, 3);
        let live = &mut simulation.session.state.live;
        deactivate_other_actors(&mut live.economy, &[parent_slot, target_slot]);
        step_field(&mut live.economy, field_slot).unwrap();
        live.economy.entities[usize::from(target_slot)].set_life(-1);

        let pending_before = live.economy.entities[usize::from(target_slot)]
            .pending_player_effect(crate::combat::actors::PlayerEffectLane::StealMana);
        assert!(pending_before.is_some(), "field must first deliver a hit");
        let consumed = consume_actor_mailbox(
            &mut live.shared_spells,
            &mut live.economy,
            &mut live.terrain,
            target_slot,
            &[],
        )
        .unwrap();
        let pending_after = live.economy.entities[usize::from(target_slot)]
            .pending_player_effect(crate::combat::actors::PlayerEffectLane::StealMana);
        assert!(consumed.steal_mana.is_none());
        assert!(!consumed.steal_consumed);
        assert_eq!(pending_after, pending_before);

        live.economy.entities[usize::from(target_slot)].set_life(100);
        let resumed = consume_actor_mailbox(
            &mut live.shared_spells,
            &mut live.economy,
            &mut live.terrain,
            target_slot,
            &[],
        )
        .unwrap();
        assert!(resumed.steal_consumed && resumed.steal_attacker_resolved);
        assert!(resumed.steal_mana.is_some());
        assert!(
            live.economy.entities[usize::from(target_slot)]
                .pending_player_effect(crate::combat::actors::PlayerEffectLane::StealMana)
                .is_none()
        );

        let missing = (1..u16::try_from(live.economy.entities.len()).unwrap())
            .find(|&slot| live.economy.handle(slot).is_none())
            .expect("a free source address");
        assert!(
            live.economy.entities[usize::from(target_slot)].submit_player_effect_contact(
                crate::combat::actors::PlayerEffectLane::StealMana,
                0,
                missing,
                true,
            )
        );
        let missing_result = consume_actor_mailbox(
            &mut live.shared_spells,
            &mut live.economy,
            &mut live.terrain,
            target_slot,
            &[],
        )
        .unwrap();
        assert!(missing_result.steal_consumed);
        assert!(!missing_result.steal_attacker_resolved);
        assert!(missing_result.steal_mana.is_none());
    }

    #[test]
    fn local_directional_feedback_distinguishes_resolved_and_missing_attackers() {
        use crate::combat::actors::PlayerEffectLane;

        let (mut resolved, _, local_slot) = ready_holder(STEAL_MANA_FAMILY, 0);
        let attacker = resolved
            .session
            .state
            .live
            .shared_spells
            .players
            .iter()
            .find(|player| player.entity_index != local_slot && player.active_player_index >= 0)
            .expect("owner-data rival player")
            .entity_index;
        let missing = (1..u16::try_from(resolved.session.state.live.economy.entities.len())
            .unwrap())
            .find(|&slot| resolved.session.state.live.economy.handle(slot).is_none())
            .expect("a free source address");
        let mut unresolved = resolved.clone();
        for (session, attacker) in [(&mut resolved, attacker), (&mut unresolved, missing)] {
            let live = &mut session.session.state.live;
            let local = &mut live.economy.entities[usize::from(local_slot)];
            local.record[58..60].fill(0);
            assert!(local.submit_player_effect_contact(
                PlayerEffectLane::StealMana,
                0,
                attacker,
                true
            ));
            let outcome = consume_actor_mailbox(
                &mut live.shared_spells,
                &mut live.economy,
                &mut live.terrain,
                local_slot,
                &[],
            )
            .unwrap();
            assert!(outcome.steal_consumed);
            assert_eq!(outcome.steal_attacker_resolved, attacker != missing);
            assert_eq!(
                i16::from_le_bytes(
                    live.economy.entities[usize::from(local_slot)].record[58..60]
                        .try_into()
                        .unwrap()
                ),
                100
            );
        }
        let accepted = &resolved.session.state.live.shared_spells;
        let rejected = &unresolved.session.state.live.shared_spells;
        assert_eq!(
            (
                accepted.local_player.hit_frame_time,
                accepted.player_life_regen_cooldown,
                accepted.player_hit_effect_cooldown
            ),
            (4, 16, 64)
        );
        assert_eq!(
            (
                rejected.local_player.hit_frame_time,
                rejected.player_life_regen_cooldown,
                rejected.player_hit_effect_cooldown
            ),
            (0, 0, 0)
        );
    }

    #[test]
    fn tier_two_steal_mana_debits_the_hit_castle_and_spawns_at_the_caster_castle() {
        let (mut simulation, holder_slot, parent_slot) = ready_holder(STEAL_MANA_FAMILY, 2);
        let projectile_slot = cast(&mut simulation, holder_slot, false);
        let (target_slot, field_slot) =
            fly_into_actual_opposing_player(&mut simulation, projectile_slot, parent_slot);
        let caster_castle_slot = castle_from_source_flyer(&mut simulation, parent_slot);
        {
            let live = &mut simulation.session.state.live;
            deactivate_other_actors(
                &mut live.economy,
                &[parent_slot, target_slot, caster_castle_slot],
            );
            assert!(set_castle_mana(
                &mut live.economy,
                caster_castle_slot,
                9_000
            ));
        }
        let target_castle_slot = castle_from_source_flyer(&mut simulation, target_slot);
        let live = &mut simulation.session.state.live;
        assert!(set_castle_mana(
            &mut live.economy,
            target_castle_slot,
            12_345
        ));
        let target_castle_position = live
            .economy
            .economy
            .castle_at(target_castle_slot)
            .unwrap()
            .position;
        assert_eq!(
            castle_slot(
                &live.economy,
                &[(target_slot, caster_castle_slot)],
                target_slot
            ),
            None,
            "another owner's live Castle is not the target's Castle"
        );
        let sphere_count = live.economy.economy.spheres.len();
        assert_eq!(
            live.economy.entities[usize::from(field_slot)].record[74],
            2,
            "guided allocation preserves the selected spell tier"
        );
        assert_eq!(
            step_field(&mut live.economy, field_slot)
                .unwrap()
                .accepted_target_slots,
            [target_slot]
        );

        let mut feedback_before_consequence = None;
        let consumed = consume_actor_mailbox_with_feedback(
            &mut live.shared_spells,
            &mut live.economy,
            &mut live.terrain,
            target_slot,
            &[
                (parent_slot, caster_castle_slot),
                (target_slot, target_castle_slot),
            ],
            |shared, arena, feedback| {
                if let DirectionalFeedback::Steal(owner) = feedback {
                    feedback_before_consequence = Some((owner,
                        arena.economy.castle_at(target_castle_slot).unwrap().mana,
                        arena.economy.spheres.len()));
                    shared.award_spell_experience(owner, STEAL_MANA_FAMILY, 1);
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(feedback_before_consequence, Some((parent_slot, 12_345, sphere_count)),
            "61050 awards the caster before debiting the Castle or allocating spheres");
        let result = consumed
            .steal_mana
            .expect("tier-two actor lane consumes through its Castle route");
        assert_eq!(result.result.fixed_transfer, 0);
        assert_eq!(result.result.target_castle_mana_after, Some(11_111));
        assert_eq!(live.economy.entities[usize::from(target_slot)].mana, 8_000);
        assert_eq!(
            live.economy
                .economy
                .castle_at(caster_castle_slot)
                .unwrap()
                .mana,
            9_000,
            "the casting actor's Castle supplies sphere positions but is not debited"
        );
        assert_eq!(
            live.economy
                .economy
                .castle_at(target_castle_slot)
                .unwrap()
                .mana,
            11_111,
            "the hit actor's Castle pays the tier-two percentage"
        );
        let spheres = &live.economy.economy.spheres[sphere_count..];
        assert_eq!(spheres.len(), 3);
        assert_eq!(spheres.iter().map(|sphere| sphere.mana).sum::<i32>(), 1_234);
        let caster_castle = *live.economy.economy.castle_at(caster_castle_slot).unwrap();
        let caster_castle_position = caster_castle.position;
        assert!(spheres.iter().all(|sphere| {
            sphere.owner_entity_index == 0
                && sphere.player_entity_index == 0
                && sphere.max_life == 300
                && sphere.life == 300
                && sphere.vertical_velocity == 128
                && !sphere.pending_delete
                && crate::distance_3d_wrapped(sphere.position, caster_castle_position) < 4_096
                && crate::distance_3d_wrapped(sphere.position, target_castle_position) > 20_000
        }));
        assert!(spheres.iter().all(|sphere| {
            live.economy
                .economy
                .map_chains
                .is_linked(sphere.source_entity_index)
        }));
        // A reused arena address must not revive the old owner/root link.
        live.economy.economy.release_slot(target_castle_slot, None);
        assert_eq!(
            live.economy.economy.allocate_slot(),
            Some(target_castle_slot)
        );
        assert!(castle_projection(&live.economy, target_castle_slot).is_none());
        assert!(!set_castle_mana(&mut live.economy, target_castle_slot, 1));
    }

    #[test]
    fn invalid_class_five_collision_does_not_write_the_steal_mana_lane() {
        let (mut simulation, holder_slot, parent_slot) = ready_holder(STEAL_MANA_FAMILY, 0);
        let projectile_slot = cast(&mut simulation, holder_slot, false);
        let (target_slot, field_slot) =
            fly_into_target(&mut simulation, projectile_slot, parent_slot, 5);
        let live = &mut simulation.session.state.live;
        assert_eq!(
            (
                live.economy.entities[usize::from(field_slot)].class,
                live.economy.entities[usize::from(field_slot)].model
            ),
            (10, STEAL_MANA_FIELD_MODEL)
        );
        let field = step_field(&mut live.economy, field_slot).unwrap();
        assert!(field.accepted_target_slots.is_empty());
        assert!(
            live.economy.entities[usize::from(target_slot)]
                .pending_player_effect(crate::combat::actors::PlayerEffectLane::StealMana)
                .is_none()
        );
        let consumed = consume_actor_mailbox(
            &mut live.shared_spells,
            &mut live.economy,
            &mut live.terrain,
            target_slot,
            &[],
        )
        .unwrap();
        assert!(consumed.steal_mana.is_none());
    }
}
