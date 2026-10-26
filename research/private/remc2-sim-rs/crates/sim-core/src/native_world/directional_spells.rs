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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DirectionalCastInput {
    pub projectile: CastInput,
    /// The existing local A4 tether relation (`FlightState.tether_target_index`).
    pub duel_target_active: bool,
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
            && let Some(parent) = arena.entities.get(usize::from(parent_slot))
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
            let duel_late_abort = family == DUEL_FAMILY
                && holder.active_value <= holder.activation_value as i16 - 28
                && !input.duel_target_active;
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
    let particle = *particles
        .get(usize::from(particle_index))
        .ok_or_else(|| format!("directional spell requires retained particle {particle_index}"))?;
    let parent = arena
        .entities
        .get(usize::from(parent_slot))
        .ok_or("directional parent address is outside source arena")?
        .clone();
    let position = parent.position;
    let Some(projectile_slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let position = if projectile_slot == parent_slot {
        Position { x: 0, y: 0, z: 0 }
    } else {
        position
    };
    let mut projectile = super::cave_features::new_event(
        projectile_slot,
        arena.economy.new_event_rng(projectile_slot),
        position,
    );
    projectile.class = 9;
    projectile.model = model;
    projectile.record[67] = 9;
    projectile.record[68] = model;
    projectile.set_source_action(model);
    projectile.set_control_flags(projectile.control_flags & !0x08);
    projectile.set_actual_speed(if family == STEAL_MANA_FAMILY {
        PROJECTILE_SPEED
            .wrapping_add(parent.actual_speed)
            .clamp(PROJECTILE_SPEED, PROJECTILE_RANGE as i16)
    } else {
        PROJECTILE_SPEED
    });
    projectile.record[136..138].copy_from_slice(&PROJECTILE_SPEED.to_le_bytes());
    projectile.set_mana(50);
    projectile.set_max_life(PROJECTILE_RANGE / i32::from(PROJECTILE_SPEED));
    projectile.set_life(projectile.max_life);
    projectile.parameter_relation = Some(parameter);
    projectile.record[164..168]
        .copy_from_slice(&u32::from(parameter.wrapping_add(1)).to_le_bytes());
    super::cave_features::bind_particle(&mut projectile, particle_index, particle)?;
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
    let mut axis = parent.position;
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
    super::guided_projectile::step(arena, terrain, particles, slot, guidance, targets)
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
    let radius = (pitch_plus_255 - pitch_plus_255.signum() * 255) >> 8;
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
                    || !source_selector_matches(field, target.class, target.model)
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

fn source_selector_matches(field: &EntitySnapshot, class: u8, model: u8) -> bool {
    let class_selector = field.record[69];
    let model_selector = field.record[70];
    class_selector == u8::MAX
        || (class_selector == class && (model_selector == u8::MAX || model_selector == model))
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
    if !row.active || (row.class, row.model) != (3, 0) {
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
    if !row.active || (row.class, row.model) != (3, 0) {
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

fn castle_slot(castle_slots: &[(u16, u16)], player_slot: u16) -> Option<u16> {
    castle_slots
        .iter()
        .find_map(|(player, castle)| (*player == player_slot && *castle != 0).then_some(*castle))
}

#[derive(Clone, Copy)]
struct CastleProjection {
    owner_entity_index: u16,
    position: Position,
    pitch: i16,
    roll: i16,
    mana: i32,
}

/// Resolve a Castle from its typed authority before considering an active
/// source row. The row fallback is limited to class-3/model-2 Castle roots;
/// an unrelated occupant at the same arena address is never Castle mana or
/// sphere-position state.
fn castle_projection(arena: &SourceArena, slot: u16) -> Option<CastleProjection> {
    if slot == 0 {
        return None;
    }
    if let Some(castle) = arena
        .economy
        .castle
        .filter(|castle| castle.active && castle.source_entity_index == slot)
    {
        return Some(CastleProjection {
            owner_entity_index: castle.owner_entity_index,
            position: castle.position,
            pitch: castle.shift_pitch,
            roll: castle.shift_roll,
            mana: castle.mana,
        });
    }
    let row = arena.entities.get(usize::from(slot))?;
    (row.active && (row.class, row.model) == (3, 2)).then_some(CastleProjection {
        owner_entity_index: row.source_player_entity_index(),
        position: row.position,
        pitch: row.array_pitch(),
        roll: row.array_roll(),
        mana: row.mana,
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

fn castle_owner_entity_index(arena: &SourceArena, slot: u16) -> Option<u16> {
    castle_projection(arena, slot).map(|castle| castle.owner_entity_index)
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
    let typed = arena
        .economy
        .castle
        .is_some_and(|castle| castle.active && castle.source_entity_index == slot);
    if typed {
        if let Some(castle) = arena
            .economy
            .castle
            .as_mut()
            .filter(|castle| castle.active && castle.source_entity_index == slot)
        {
            castle.mana = mana;
        }
    }
    let row_written = if let Some(row) = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && (row.class, row.model) == (3, 2))
    {
        row.set_mana(mana);
        true
    } else {
        false
    };
    typed || row_written
}

fn duel_holder_slot(shared: &Level2SharedSpellResetSession, caster_slot: u16) -> Option<u16> {
    shared
        .pickups()
        .iter()
        .filter(|holder| {
            holder.class == 15
                && holder.model == DUEL_FAMILY
                && holder.action == DUEL_ACTION
                && holder.active_value != 0
                && holder.source_spell_parent_entity_index() == caster_slot
        })
        .map(|holder| holder.index)
        .min()
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
pub(crate) fn consume_actor_mailbox(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    actor_slot: u16,
    castle_slots: &[(u16, u16)],
) -> Result<DirectionalMailboxOutcome, String> {
    let target_row = arena
        .entities
        .get(usize::from(actor_slot))
        .ok_or("directional mailbox actor address is outside source arena")?;
    if !target_row.active || (target_row.class, target_row.model) != (3, 0) {
        return Ok(DirectionalMailboxOutcome::default());
    }

    let mut outcome = DirectionalMailboxOutcome::default();
    let duel_hit = arena.entities[usize::from(actor_slot)]
        .take_player_effect(crate::combat::actors::PlayerEffectLane::Duel);
    if let Some(hit) = duel_hit {
        let caster_slot = hit.attacker;
        if let (Some(caster), Some(target), Some(holder_slot)) = (
            duel_actor(arena, caster_slot),
            duel_actor(arena, actor_slot),
            duel_holder_slot(shared, caster_slot),
        ) && let Some(binding) = bind_tether(hit.payload, caster, target, holder_slot)
        {
            outcome.duel = Some(binding);
        }
    }

    let steal_hit = arena.entities[usize::from(actor_slot)]
        .take_player_effect(crate::combat::actors::PlayerEffectLane::StealMana);
    if let Some(hit) = steal_hit {
        let caster_slot = hit.attacker;
        let Some(mut thief) = actor(arena, caster_slot) else {
            return Ok(outcome);
        };
        let Some(mut victim) = actor(arena, actor_slot) else {
            return Ok(outcome);
        };
        let thief_castle_slot = castle_slot(castle_slots, caster_slot);
        let victim_castle_slot = castle_slot(castle_slots, actor_slot);
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
            if let (Some(slot), Some(mana)) = (victim_castle_slot, result.target_castle_mana_after)
            {
                if !set_castle_mana(arena, slot, mana) {
                    return Err(format!(
                        "Steal Mana Castle debit lost current Castle carrier {slot}"
                    ));
                }
            }
            if let Some(spawn_castle_slot) = thief_castle_slot {
                let Some(castle_owner) = castle_owner_entity_index(arena, spawn_castle_slot) else {
                    return Ok(outcome);
                };
                for grant in &result.mana_spheres {
                    let _ = spawn_steal_mana_sphere(
                        arena,
                        terrain,
                        actor_slot,
                        spawn_castle_slot,
                        castle_owner,
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
    Ok(outcome)
}

fn spawn_steal_mana_sphere(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    hit_actor_slot: u16,
    caster_castle_slot: u16,
    castle_owner_entity_index: u16,
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

    let pitch = i64::from(castle.pitch);
    let roll = i64::from(castle.roll);
    let radius = crate::legacy_sqrt(u32::try_from(pitch * pitch + roll * roll).unwrap_or(u32::MAX));
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
        i16::try_from(radius).unwrap_or(i16::MAX),
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
        castle_owner_entity_index,
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
    // The Rust Castle collector routes by the caster Castle owner, while the
    // raw C++ model-39 targeting relation follows the SPELLS.DAT product byte.
    sphere.owner_entity_index = castle_owner_entity_index;
    sphere.player_entity_index = player_entity_index;
    let entity_rng = sphere.entity_rng;
    arena.economy.source_rng.insert(slot, entity_rng);
    if let Some(row) = arena.entities.get_mut(usize::from(slot)) {
        row.set_source_player_entity_index(player_entity_index);
    }
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    Ok(Some(slot))
}

#[cfg(test)]
mod tests {
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
                duel_target_active,
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
        let radius = ((width - width.signum() * 255) >> 8) as u8;
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
                        .filter(|row| row.active && (row.class, row.model) == (3, 0))
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
            assert_eq!((target.class, target.model), (3, 0));
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
            let radius = ((width - width.signum() * 255) >> 8) as u8;
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

    fn add_castle(
        simulation: &mut super::super::Mc2Simulation,
        template_slot: u16,
        position: Position,
        player_entity_index: u16,
        mana: i32,
    ) -> u16 {
        let live = &mut simulation.session.state.live;
        let slot = live
            .economy
            .allocate_source_slot_with_terrain(&mut live.terrain)
            .expect("test Castle source slot available");
        let mut castle = live.economy.entities[usize::from(template_slot)].clone();
        castle.index = slot;
        castle.source_id = player_entity_index;
        castle.record[30..32].copy_from_slice(&player_entity_index.to_le_bytes());
        castle.class = 3;
        castle.model = 2;
        castle.record[67] = 3;
        castle.record[68] = 2;
        castle.set_source_action(5);
        castle.active = true;
        castle.set_life(40_000);
        castle.set_mana(mana);
        castle.set_source_player_entity_index(player_entity_index);
        castle.set_position(position);
        live.economy.entities[usize::from(slot)] = castle;
        crate::world::effects::sync_position(
            &mut live.economy,
            &mut live.terrain,
            slot,
            position,
            true,
            true,
        )
        .expect("test Castle map link");
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
        let mut expected_axis = parent.position;
        move_entity(&mut expected_axis, parent.yaw, parent.pitch, 10_240);
        assert_eq!(source_axis(&flyer), expected_axis);

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
        assert_eq!(pending_after, pending_before);
    }

    #[test]
    fn tier_two_steal_mana_debits_the_hit_castle_and_spawns_at_the_caster_castle() {
        let (mut simulation, holder_slot, parent_slot) = ready_holder(STEAL_MANA_FAMILY, 2);
        let projectile_slot = cast(&mut simulation, holder_slot, false);
        let (target_slot, field_slot) =
            fly_into_actual_opposing_player(&mut simulation, projectile_slot, parent_slot);
        let constructed_castle_slot = {
            let live = &mut simulation.session.state.live;
            let parent_position = live.economy.entities[usize::from(parent_slot)].position;
            live.economy
                .economy
                .build_castle(parent_position, parent_slot, &live.terrain)
                .expect("tier-two consumer fixture constructs a real local Castle")
        };
        let position =
            simulation.session.state.live.economy.entities[usize::from(field_slot)].position;
        let caster_castle_slot = simulation
            .session
            .state
            .live
            .economy
            .economy
            .castle
            .filter(|castle| castle.active)
            .expect("qualified Level 2 fixture has its local Castle")
            .source_entity_index;
        assert_eq!(caster_castle_slot, constructed_castle_slot);
        {
            let live = &mut simulation.session.state.live;
            deactivate_other_actors(
                &mut live.economy,
                &[parent_slot, target_slot, caster_castle_slot],
            );
            if let Some(castle) = live
                .economy
                .economy
                .castle
                .as_mut()
                .filter(|castle| castle.source_entity_index == caster_castle_slot)
            {
                castle.mana = 9_000;
            }
            if let Some(row) = live
                .economy
                .entities
                .get_mut(usize::from(caster_castle_slot))
            {
                row.set_mana(9_000);
            }
        }
        let target_castle_position = Position {
            x: position.x.wrapping_add(30_000),
            y: position.y.wrapping_add(30_000),
            z: position.z,
        };
        let target_castle_slot = add_castle(
            &mut simulation,
            parent_slot,
            target_castle_position,
            target_slot,
            12_345,
        );
        let live = &mut simulation.session.state.live;
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

        let consumed = consume_actor_mailbox(
            &mut live.shared_spells,
            &mut live.economy,
            &mut live.terrain,
            target_slot,
            &[
                (parent_slot, caster_castle_slot),
                (target_slot, target_castle_slot),
            ],
        )
        .unwrap();
        let result = consumed
            .steal_mana
            .expect("tier-two actor lane consumes through its Castle route");
        assert_eq!(result.result.fixed_transfer, 0);
        assert_eq!(result.result.target_castle_mana_after, Some(11_111));
        assert_eq!(live.economy.entities[usize::from(target_slot)].mana, 8_000);
        assert_eq!(
            live.economy.economy.castle.unwrap().mana,
            9_000,
            "the casting actor's Castle supplies sphere positions but is not debited"
        );
        assert_eq!(
            live.economy.entities[usize::from(target_castle_slot)].mana,
            11_111,
            "the hit actor's Castle pays the tier-two percentage"
        );
        let spheres = &live.economy.economy.spheres[sphere_count..];
        assert_eq!(spheres.len(), 3);
        assert_eq!(spheres.iter().map(|sphere| sphere.mana).sum::<i32>(), 1_234);
        let caster_castle = live.economy.economy.castle.unwrap();
        // This fixture Castle is installed in typed economy state without
        // publishing an active draw row. Sphere placement takes this same
        // Castle.position fallback when its raw row is inactive.
        let caster_castle_position = caster_castle.position;
        assert!(spheres.iter().all(|sphere| {
            sphere.owner_entity_index == caster_castle.owner_entity_index
                && sphere.player_entity_index == 0
                && sphere.max_life == 300
                && sphere.life == 300
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
