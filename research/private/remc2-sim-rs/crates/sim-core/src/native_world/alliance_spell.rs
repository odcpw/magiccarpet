//! Runtime adapter for the Alliance holder, ordinary model25 carrier, and
//! scheduled class10/model74 terminal consumer.
//!
//! The terminal consumer writes the source StageVar/parent/timer fields that
//! normal creature visits already read. This leaf owns those writes through
//! the focused Alliance kernel; scheduler tails and following-pass reclaim
//! remain with the shared dispatch owner.

use super::{
    EntitySnapshot, EntitySpecialRelation, Level2SharedSpellResetSession, ParticleParameter,
    SessionTerrain,
};
use crate::campaign::source_arena::SourceArena;
use crate::spell_activation::ProjectileSpellCastInput as CastInput;
use crate::spells::late::alliance::{self as rules, Impact};
use crate::{FlightWorld, Position, move_entity};

const HOLDER_MODEL: u8 = 24;
const HOLDER_ACTION: u8 = 72;
const PROJECTILE_CLASS: u8 = 9;
const PROJECTILE_MODEL: u8 = 25;
const PROJECTILE_ACTION: u8 = 26;
const PROJECTILE_SPEED: i16 = 384;
const PROJECTILE_MAX_LIFE: i32 = 4_096 / 384;
const PROJECTILE_PARAMETER: u16 = 61;
const PROJECTILE_PARTICLE: u16 = 321;
const IMPACT_CLASS: u8 = 10;
const IMPACT_MODEL: u8 = 74;
const IMPACT_ACTION: u8 = 81;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct HolderOutcome {
    pub projectile: Option<u16>,
    pub clear_player_aux: Option<u16>,
    pub charged: bool,
    pub denied: bool,
    /// `sub_6D880` applied a queued physical profile. This is not entity
    /// cleanup: the source holder remains available for later casts.
    pub profile_updated: bool,
}

fn guidance_for_projectile(
    row: &EntitySnapshot,
    parameters: &[[u8; super::ENTITY_PARAMETER_BYTES]],
) -> Result<super::guided_projectile::Guidance, String> {
    let relation = row
        .parameter_relation
        .filter(|relation| *relation == u16::from(PROJECTILE_PARAMETER))
        .ok_or("Alliance action26 projectile lost source parameter61")?;
    let descriptor = parameters
        .get(usize::from(relation))
        .ok_or("Alliance source parameter61 is outside the entity table")?;
    Ok(super::guided_projectile::Guidance {
        // sub_65610 passes parameter subtype_2 as sub_58350's turn cap.
        yaw_turn: i16::from_le_bytes([descriptor[2], descriptor[3]]),
        // Pitch uses word_0x6; the intervening word_0x4 is not its cap.
        pitch_turn: i16::from_le_bytes([descriptor[6], descriptor[7]]),
    })
}

fn put_word(row: &mut EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn set_owner(row: &mut EntitySnapshot, owner: u16) {
    row.source_id = owner;
    put_word(row, 30, owner);
}

fn construct_source_carrier(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    position: Position,
    parent_slot: u16,
    holder_slot: u16,
) -> Result<Option<u16>, String> {
    let particle = *particles
        .get(usize::from(PROJECTILE_PARTICLE))
        .ok_or("Alliance source constructor needs particle321")?;
    if particle.animation_limit().is_none() {
        return Err("Alliance particle321 has an unsupported render mode".to_owned());
    }
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    if slot == holder_slot {
        return Err("Alliance source allocator selected the live holder".to_owned());
    }
    // The source passes &parent.position into the constructor. If NewEvent
    // reuses that freed parent, its reset occurs before this pointer is read.
    let position = if slot == parent_slot {
        Position::default()
    } else {
        position
    };
    let mut row = crate::world::terrain::cave_features::new_event(
        slot,
        arena.economy.new_event_rng(slot),
        position,
    );
    row.class = PROJECTILE_CLASS;
    row.model = PROJECTILE_MODEL;
    row.record[67] = PROJECTILE_CLASS;
    row.record[68] = PROJECTILE_MODEL;
    row.set_source_action(PROJECTILE_ACTION);
    row.set_control_flags(row.control_flags & !8);
    row.set_actual_speed(PROJECTILE_SPEED);
    row.record[136..138].copy_from_slice(&PROJECTILE_SPEED.to_le_bytes());
    row.set_max_life(PROJECTILE_MAX_LIFE);
    row.set_mana(50);
    row.parameter_relation = Some(PROJECTILE_PARAMETER);
    row.record[164..168].copy_from_slice(&(u32::from(PROJECTILE_PARAMETER) + 1).to_le_bytes());

    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    if let Err(error) =
        crate::world::effects::sync_position(arena, terrain, slot, position, true, true)
    {
        crate::world::terrain::cave_features::reclaim(arena, terrain, slot);
        return Err(error);
    }
    let row = &mut arena.entities[usize::from(slot)];
    row.set_life(PROJECTILE_MAX_LIFE);
    if let Err(error) =
        crate::world::terrain::cave_features::bind_particle(row, PROJECTILE_PARTICLE, particle)
    {
        crate::world::terrain::cave_features::reclaim(arena, terrain, slot);
        return Err(error);
    }
    Ok(Some(slot))
}

/// `IfSubtypeCallCreatingManaSphere_4A190` dispatches a guided `(10,74)`
/// collision to `sub_50800` (`EventsFunctions.cpp:36944`). That constructor
/// creates a bare NewEvent row and writes only class/model/action/control
/// bytes; the shared guided collision tail applies the source `sub_65820`
/// postwrites. In particular this row is not map-positioned, life/power-set,
/// or particle-bound here.
pub(crate) fn construct_impact(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
) -> Option<u16> {
    // NewEvent replacement must repair the old cell head, but this bare
    // sub_50800 row receives no AddEventToMap for its default position.
    let slot = arena.allocate_source_slot_with_terrain(terrain)?;
    let mut row = crate::world::terrain::cave_features::new_event(
        slot,
        arena.economy.new_event_rng(slot),
        Position::default(),
    );
    row.class = IMPACT_CLASS;
    row.model = IMPACT_MODEL;
    row.record[67] = IMPACT_CLASS;
    row.record[68] = IMPACT_MODEL;
    row.set_source_action(IMPACT_ACTION);
    // sub_50800 clears control byte[0] with &= 0xF6, then sets bit 0.
    let flags = row.control_flags;
    let control_byte = (flags as u8 & 0xF6) | 1;
    row.set_control_flags((flags & !0xFF) | u32::from(control_byte));
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    Some(slot)
}

/// Source `sub_68E50` hand placement for the newly constructed row. Alliance
/// applies no FOV lift after this shared source placement helper.
fn hand_position(
    parent: &EntitySnapshot,
    position: Position,
    terrain: &SessionTerrain,
) -> Position {
    let yaw = if parent.control_flags & 0x100 != 0 {
        parent.yaw.wrapping_sub(512) & 2047
    } else if parent.control_flags & 0x200 != 0 {
        parent.yaw.wrapping_add(512) & 2047
    } else {
        return position;
    };
    let mut placed = position;
    move_entity(&mut placed, yaw, 0, 256);
    if terrain.ground_height(placed) > placed.z {
        position
    } else {
        placed
    }
}

/// `sub_6CD20` calls the real `(9,25)` constructor before applying its
/// class10/model74 guided-target payload. Parent and holder are reread after
/// allocation so source aim, owner, and pending auxiliary remain current.
fn construct_carrier(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    parent_slot: u16,
    holder_slot: u16,
    parameters: rules::ProjectileParameters,
    input: CastInput,
    shared_a4_input: CastInput,
) -> Result<Option<u16>, String> {
    let parent_before = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|parent| parent_slot != 0 && parent.index == parent_slot)
        .cloned()
        .ok_or("Alliance constructor parent address is outside the source arena")?;
    let Some(slot) = construct_source_carrier(
        arena,
        terrain,
        particles,
        parent_before.position,
        parent_slot,
        holder_slot,
    )?
    else {
        return Ok(None);
    };
    let post_allocation = (|| -> Result<Position, String> {
        let parent = arena
            .entities
            .get(usize::from(parent_slot))
            .filter(|parent| parent.index == parent_slot)
            .cloned()
            .ok_or("Alliance parent address was lost during source allocation")?;
        // NewEvent can reset this fixed parent's A4 pointer during allocation;
        // it can also already address the shared A4 on entry to the holder.
        let input = if parent.special_relation == EntitySpecialRelation::GlobalSpecialState {
            shared_a4_input
        } else {
            input
        };
        let holder = arena
            .entities
            .get(usize::from(holder_slot))
            .filter(|holder| {
                holder.index == holder_slot
                    && holder.active
                    && (holder.class, holder.model) == (15, HOLDER_MODEL)
            })
            .cloned()
            .ok_or("Alliance holder was lost during source allocation")?;
        let row = arena
            .entities
            .get_mut(usize::from(slot))
            .ok_or("Alliance projectile address is outside the source arena")?;
        let placed = hand_position(&parent, row.position, terrain);
        row.set_position(placed);
        row.set_actual_speed(PROJECTILE_SPEED.wrapping_add(parent.actual_speed));
        row.record[71] = IMPACT_CLASS;
        row.record[72] = IMPACT_MODEL;
        row.set_subspell_index(parameters.damage);
        row.set_source_byte_46(parameters.radius_tiles);
        row.set_source_dword_10(i32::from(input.auxiliary_byte));
        put_word(row, 42, holder_slot);
        put_word(row, 44, holder.source_spell_parent_entity_index());
        set_owner(row, parent.source_id);
        // sub_6CD20 reads Type_str_164's input auxiliary words. The raw
        // entity's nextEntity word is a map-chain link, not an aim offset.
        let yaw = parent.yaw.wrapping_add(input.aim_yaw);
        let pitch = parent.pitch.wrapping_add(input.aim_pitch);
        let mut aim = parent.position;
        move_entity(&mut aim, yaw, pitch, 16_384);
        put_word(row, 158, aim.x);
        put_word(row, 160, aim.y);
        row.record[162..164].copy_from_slice(&aim.z.to_le_bytes());
        row.set_yaw(yaw);
        row.set_pitch(pitch);
        Ok(placed)
    })();
    let placed = match post_allocation {
        Ok(position) => position,
        Err(error) => {
            crate::world::terrain::cave_features::reclaim(arena, terrain, slot);
            return Err(error);
        }
    };
    if let Err(error) =
        crate::world::effects::sync_position(arena, terrain, slot, placed, true, true)
    {
        crate::world::terrain::cave_features::reclaim(arena, terrain, slot);
        return Err(error);
    }
    Ok(Some(slot))
}

/// One ordinary class15/model24/action72 holder visit. Mana and cooldown use
/// the Alliance kernel's source gate; only a successfully allocated ordinary
/// carrier is reported as a projectile.
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
        .filter(|row| {
            row.active && (row.class, row.model, row.action) == (15, HOLDER_MODEL, HOLDER_ACTION)
        })
        .cloned()
        .ok_or_else(|| format!("Alliance holder {slot} is not live class15/model24/action72"))?;
    let tier = crate::spell_activation::level2_applied_tier(holder.level_gate)
        .ok_or("Alliance holder has a negative source tier")?;
    let source_rule = *rules::SOURCE_RULES
        .get(usize::from(tier))
        .ok_or("Alliance holder tier is outside owner SPELLS.DAT")?;
    // sub_68D50/sub_68DE0 read the current physical holder, including
    // source profile overrides. The projectile payload still comes from
    // the selected SPELLS.DAT row in sub_6CD20.
    let rule = rules::Rule {
        mana_cost: holder.max_mana,
        castle_mana_required: holder.mana_regen,
        ..source_rule
    };
    let parent_slot = holder.source_spell_parent_entity_index();
    let parent = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|parent| parent_slot != 0 && parent.index == parent_slot)
        .cloned();

    let Some(holder_row) = arena.entities.get_mut(usize::from(slot)) else {
        return Err("Alliance holder address is outside the source arena".to_owned());
    };
    *holder_row = holder.clone();

    let mut caster = parent.as_ref().map(|parent| rules::Caster {
        entity_index: parent.index,
        source_id: parent.source_id,
        position: parent.position,
        life: parent.life,
        mana: parent.mana,
        mana_regen: parent.mana_regen,
        castle_mana: input.castle_mana,
        yaw: parent.yaw,
        pitch: parent.pitch,
        next_entity: input.aim_yaw.cast_unsigned(),
        entity_index2: input.aim_pitch.cast_unsigned(),
        height_offset: parent.array_fov(),
        pending_cast_auxiliary: input.auxiliary_byte,
    });
    let mut state = rules::Holder {
        active: holder.active_value,
        duration: holder.activation_value as i16,
        cooldown: i16::from_le_bytes([holder.record[58], holder.record[59]]),
    };
    let visit = state.visit(rule, caster.as_mut());
    let mut outcome = HolderOutcome {
        charged: visit.mana_pulse,
        denied: visit.denied,
        ..HolderOutcome::default()
    };

    if let (Some(parent), Some(caster)) = (parent.as_ref(), caster) {
        if visit.projectile_allocation_attempts != 0 {
            let parameters = rule.projectile_parameters(caster, slot);
            let shared_a4_input = CastInput {
                aim_yaw: shared.global_special_state.input_yaw,
                aim_pitch: shared.global_special_state.input_pitch,
                auxiliary_byte: shared.global_special_state.cast_auxiliary,
                castle_mana: input.castle_mana,
            };
            outcome.projectile = construct_carrier(
                arena,
                terrain,
                particles,
                parent_slot,
                slot,
                parameters,
                input,
                shared_a4_input,
            )?;
            if let Some(projectile) = outcome.projectile {
                if arena
                    .entities
                    .get(usize::from(parent_slot))
                    .is_some_and(|parent| {
                        parent.special_relation == EntitySpecialRelation::GlobalSpecialState
                    })
                {
                    shared.global_special_state.cast_auxiliary = 0;
                } else {
                    outcome.clear_player_aux = Some(parent_slot);
                }
                arena
                    .economy
                    .prepare_sound(projectile, projectile, -1, parameters.sound);
            }
        }
        // sub_68DE0 runs after the allocation attempt and reads the current
        // fixed parent record. Allocation can itself reuse a reclaimed parent.
        if let Some(authority) = arena.entities.get_mut(usize::from(parent_slot)) {
            let regen = if visit.mana_pulse {
                if authority.mana_regen >= 0 {
                    rule.mana_cost.wrapping_neg()
                } else {
                    authority.mana_regen.wrapping_sub(rule.mana_cost)
                }
            } else if caster.mana_regen != parent.mana_regen {
                caster.mana_regen
            } else {
                authority.mana_regen
            };
            authority.set_mana_regen(regen);
            shared.sync_player_entity_from_authority(authority);
            if parent_slot == shared.player_entity.index {
                shared.sync_typed_spell_parent_caches_from_player_entity();
            }
        }
    }

    holder.active_value = state.active;
    holder.record[50..52].copy_from_slice(&state.active.to_le_bytes());
    holder.record[58..60].copy_from_slice(&state.cooldown.to_le_bytes());
    if visit.cleanup {
        let deferred = holder.source_word_2c();
        if deferred != 0 {
            let deferred_tier = deferred
                .checked_sub(1)
                .and_then(|value| u8::try_from(value.min(2)).ok())
                .ok_or("Alliance queued profile tier is outside the source table")?;
            let profile = shared
                .physical_spell_profile(24, deferred_tier, parent_slot, None)
                .ok_or("Alliance queued profile is missing from owner SPELLS.DAT")?;
            holder.apply_nyphur_spell_profile(profile, deferred_tier);
            holder.set_source_word_2c(0);
            outcome.profile_updated = true;
        }
    }
    if arena.entities[usize::from(slot)].class == 15
        && arena.entities[usize::from(slot)].model == HOLDER_MODEL
    {
        if let Some(current) = shared.pickup_mut(slot) {
            *current = holder.clone();
        }
        arena.entities[usize::from(slot)] = holder;
    }
    Ok(outcome)
}

/// Source action26 dispatches Alliance through the shared `sub_65820` flyer.
/// The descriptor remains live input because source `sub_65610` reads its
/// yaw and pitch turn caps on each scheduled visit.
pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    targets: Option<&super::guided_projectile::Targets<'_>>,
    parameters: &[[u8; super::ENTITY_PARAMETER_BYTES]],
    context: super::source_target::Context<'_>,
) -> Result<super::guided_projectile::StepOutcome, String> {
    let row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| {
            row.index == slot
                && row.active
                && (row.class, row.model, row.action)
                    == (PROJECTILE_CLASS, PROJECTILE_MODEL, PROJECTILE_ACTION)
        })
        .ok_or("Alliance projectile callback needs live class9/model25/action26")?;
    let guidance = guidance_for_projectile(row, parameters)?;
    super::guided_projectile::step(arena, terrain, particles, slot, guidance, targets, context)
}

#[derive(Clone, Copy)]
enum TypedAllianceOwner {
    Bee,
    Firefly,
    Archer,
    Dragon,
    Worm,
}

fn typed_projection(
    arena: &SourceArena,
    slot: u16,
) -> Option<(TypedAllianceOwner, EntitySnapshot)> {
    super::creatures::bee::mc2::alliance_projection(arena, slot)
        .map(|row| (TypedAllianceOwner::Bee, row))
        .or_else(|| {
            super::level2_class5::firefly::alliance_projection(arena, slot)
                .map(|row| (TypedAllianceOwner::Firefly, row))
        })
        .or_else(|| {
            super::level2_class5::archer::alliance_projection(arena, slot)
                .map(|row| (TypedAllianceOwner::Archer, row))
        })
        .or_else(|| {
            super::level2_class5::dragon::alliance_projection(arena, slot)
                .map(|row| (TypedAllianceOwner::Dragon, row))
        })
        .or_else(|| {
            super::creatures::worm::dynamic::alliance_projection(arena, slot)
                .map(|row| (TypedAllianceOwner::Worm, row))
        })
}

pub(crate) fn impact_center(arena: &SourceArena, slot: u16) -> Option<EntitySnapshot> {
    typed_projection(arena, slot)
        .map(|(_, row)| row)
        .or_else(|| arena.entities.get(usize::from(slot)).cloned())
}

/// Scheduled class10/model74 terminal consumer. The shared guided collision
/// constructor allocates this row separately from the class9/model25 carrier.
/// It writes the source StageVar, parent, action, and timer state consumed by
/// ordinary class5 visits.
///
/// The postcondition ensures the source-backed model74 terminal actually
/// reaches its consumer before this scheduled callback reports success.
pub(crate) fn step_impact(
    arena: &mut SourceArena,
    effect_slot: u16,
    map_order: &[u16],
) -> Result<Impact, String> {
    let Some(effect) = arena.entities.get(usize::from(effect_slot)) else {
        return Err("Alliance impact address is outside the source arena".to_owned());
    };
    if effect.index != effect_slot
        || !effect.active
        || effect.drawing_disabled()
        || (effect.class, effect.model) != (IMPACT_CLASS, IMPACT_MODEL)
    {
        return Err(
            "Alliance terminal handler needs a visible live class10/model74 effect".to_owned(),
        );
    }
    // Alliance assigns its parent when the terminal takes over the creature,
    // rather than at its original construction. Capture the current typed
    // identity for the write; later source behavior reads the fixed address.
    let parent = arena.handle(effect.source_id);
    let center_slot = effect.source_word_96();
    let mut typed_targets = std::collections::BTreeMap::new();
    for slot in map_order
        .iter()
        .copied()
        .chain(std::iter::once(center_slot))
    {
        if let Some((owner, row)) = typed_projection(arena, slot)
            && let Some(handle) = arena.handle(slot)
        {
            typed_targets.insert(slot, (handle, owner, row));
        }
    }
    // The source kernel needs current action and StageVar fields for both
    // eligibility and its control writes. This view exists for this call
    // only; each owner receives the result without installing a raw row over
    // its typed source or sidecar.
    let mut projected = (!typed_targets.is_empty()).then(|| arena.entities.clone());
    let impact = if let Some(rows) = projected.as_mut() {
        for (&slot, (_, _, row)) in &typed_targets {
            rows[usize::from(slot)] = row.clone();
        }
        rules::impact(rows, effect_slot, map_order)
    } else {
        rules::impact(&mut arena.entities, effect_slot, map_order)
    };
    if let Some(rows) = projected.as_ref() {
        if !rows[usize::from(effect_slot)].drawing_disabled() {
            return Err(
                "Alliance projected terminal was not consumed by the source kernel".to_owned(),
            );
        }
        for &slot in &impact.affected_entities {
            let row = &rows[usize::from(slot)];
            if let Some((child, owner, _)) = typed_targets.get(&slot) {
                let applied = match owner {
                    TypedAllianceOwner::Bee => super::creatures::bee::mc2::apply_alliance_takeover(
                        arena, *child, row, parent,
                    )?,
                    TypedAllianceOwner::Firefly => {
                        super::level2_class5::firefly::apply_alliance_takeover(
                            arena, *child, row, parent,
                        )?
                    }
                    TypedAllianceOwner::Archer => {
                        super::level2_class5::archer::apply_alliance_takeover(
                            arena, *child, row, parent,
                        )?
                    }
                    TypedAllianceOwner::Dragon => {
                        super::level2_class5::dragon::apply_alliance_takeover(
                            arena, *child, row, parent,
                        )?
                    }
                    TypedAllianceOwner::Worm => {
                        super::creatures::worm::dynamic::apply_alliance_takeover(
                            arena, *child, row, parent,
                        )?
                    }
                };
                if !applied {
                    return Err(format!(
                        "Alliance typed creature {slot} rejected its current takeover"
                    ));
                }
            } else {
                arena.entities[usize::from(slot)] = row.clone();
            }
        }
        arena.entities[usize::from(effect_slot)].disable_drawing();
    }
    if !arena.entities[usize::from(effect_slot)].drawing_disabled() {
        return Err(
            "Alliance model74 terminal was not consumed by the current impact kernel".to_owned(),
        );
    }
    if let Some(parent) = parent {
        for &slot in &impact.affected_entities {
            if typed_targets.contains_key(&slot) {
                continue;
            }
            // A typed occupant may have rejected an obsolete compatibility
            // row's control write. Do not change that occupant's parent when
            // its current source identity differs from the affected shadow.
            let current = super::area_damage::read_arena_actor(arena, slot);
            let exact_occupant = arena.entities.get(usize::from(slot)).is_some_and(|row| {
                row.index == slot
                    && current.is_some_and(|actor| {
                        (row.source_id, row.class, row.model)
                            == (actor.source_id, actor.class, actor.model)
                    })
            });
            if exact_occupant
                && let Some(child) = arena.handle(slot)
                && (!matches!(
                    arena.resolve(child),
                    Some(crate::campaign::source_arena::SourceEntityView::Dynamic(
                        super::Level2DynamicArenaKind::BeeCreature
                    ))
                ) || super::creatures::bee::mc2::alliance_shadow_matches_current_bee(
                    arena, slot,
                ))
            {
                arena.bind_creature_parent(child, parent);
            }
        }
    }
    for &(sound_entity, sound) in &impact.sounds {
        arena
            .economy
            .prepare_sound(sound_entity, sound_entity, -1, sound);
    }
    Ok(impact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holder_expiry_clamps_queued_profile_and_reads_reclaimed_parent() {
        let mut base = crate::native_world::tests::level_two_campaign_session();
        base.apply_carried_spell_families(&[24], Some(24)).unwrap();
        let parent_slot = base.state.entity_index;
        let holder_slot = base.shared_spells().local_player.enabled_spell_holders[24];
        for (queued, tier) in [(1, 0), (3, 2), (4, 2), (i16::MAX, 2)] {
            let mut simulation = base.clone();
            let particles = simulation.session.definition.presentation.particles.clone();
            let live = &mut simulation.session.state.live;
            let expected = live
                .shared_spells
                .physical_spell_profile(24, tier, parent_slot, None)
                .expect("Alliance physical tier");
            let holder = live.shared_spells.pickup_mut(holder_slot).unwrap();
            holder.active_value = 1;
            holder.record[50..52].copy_from_slice(&1_i16.to_le_bytes());
            holder.set_source_word_2c(queued);
            let parent = &mut live.economy.entities[usize::from(parent_slot)];
            parent.reclaim_source_row();
            parent.set_life(100);
            parent.set_mana(rules::SOURCE_RULES[0].mana_cost);
            parent.set_mana_regen(13);
            let outcome = step_holder(
                &mut live.shared_spells,
                &mut live.economy,
                &mut live.terrain,
                &particles,
                holder_slot,
                CastInput {
                    castle_mana: Some(rules::SOURCE_RULES[0].castle_mana_required),
                    ..Default::default()
                },
            )
            .expect("fixed parent holder expiry");
            assert!(outcome.profile_updated);
            assert!(!outcome.charged);
            assert_eq!(
                live.economy.entities[usize::from(parent_slot)].mana_regen,
                0,
                "sub_68DE0 reads a nonzero reclaimed parent address"
            );
            let holder = live.shared_spells.pickup(holder_slot).unwrap();
            assert_eq!(holder.level_gate, tier as i8);
            assert_eq!(holder.source_word_2c(), 0);
            assert_eq!(holder.max_mana, expected.max_mana);
            assert_eq!(holder.activation_value, expected.word_0x30);
        }
    }

    #[test]
    fn holder_charges_on_allocation_failure_but_not_on_refusal_for_all_tiers() {
        let mut base = crate::native_world::tests::level_two_campaign_session();
        base.apply_carried_spell_families(&[24], Some(24)).unwrap();
        let parent_slot = base.state.entity_index;
        let holder_slot = base.shared_spells().local_player.enabled_spell_holders[24];
        for tier in 0..3 {
            for refused in [false, true] {
                let mut simulation = base.clone();
                simulation
                    .session
                    .state
                    .live
                    .economy
                    .economy
                    .exhaust_new_event_capacity_for_test();
                let particles = simulation.session.definition.presentation.particles.clone();
                let live = &mut simulation.session.state.live;
                live.economy.finish_source_group();
                let profile = live
                    .shared_spells
                    .physical_spell_profile(24, tier, parent_slot, None)
                    .expect("Alliance physical tier");
                let holder = live.shared_spells.pickup_mut(holder_slot).unwrap();
                holder.apply_nyphur_spell_profile(profile, tier);
                holder.active_value = holder.activation_value.cast_signed();
                holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
                let parent = &mut live.economy.entities[usize::from(parent_slot)];
                parent.set_life(100);
                parent.set_mana(if refused {
                    profile.max_mana - 1
                } else {
                    profile.max_mana
                });
                parent.set_mana_regen(13);
                let outcome = step_holder(
                    &mut live.shared_spells,
                    &mut live.economy,
                    &mut live.terrain,
                    &particles,
                    holder_slot,
                    CastInput {
                        castle_mana: Some(profile.mana_regen),
                        ..Default::default()
                    },
                )
                .expect("Alliance failed allocation or refusal");
                assert!(outcome.projectile.is_none());
                assert!(outcome.clear_player_aux.is_none());
                assert_eq!(outcome.denied, refused);
                assert_eq!(outcome.charged, !refused);
                assert_eq!(
                    live.economy.entities[usize::from(parent_slot)].mana_regen,
                    if refused { 13 } else { -profile.max_mana }
                );
                let holder = live.shared_spells.pickup(holder_slot).unwrap();
                assert_eq!(
                    holder.active_value,
                    if refused {
                        0
                    } else {
                        profile.word_0x30.cast_signed() - 1
                    }
                );
            }
        }
    }

    #[test]
    fn reclaimed_parent_slot_can_be_reused_before_the_pulse_cost() {
        let mut simulation = crate::native_world::tests::level_two_campaign_session();
        simulation
            .apply_carried_spell_families(&[24], Some(24))
            .unwrap();
        let parent_slot = simulation.state.entity_index;
        let holder_slot = simulation
            .shared_spells()
            .local_player
            .enabled_spell_holders[24];
        simulation
            .session
            .state
            .live
            .economy
            .economy
            .exhaust_new_event_capacity_for_test();
        let particles = simulation.session.definition.presentation.particles.clone();
        let live = &mut simulation.session.state.live;
        live.economy.finish_source_group();
        let holder = live.shared_spells.pickup_mut(holder_slot).unwrap();
        holder.active_value = holder.activation_value.cast_signed();
        holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
        crate::world::terrain::cave_features::reclaim(
            &mut live.economy,
            &mut live.terrain,
            parent_slot,
        );
        let parent = &mut live.economy.entities[usize::from(parent_slot)];
        parent.set_life(100);
        parent.set_mana(rules::SOURCE_RULES[0].mana_cost);
        parent.set_mana_regen(13);
        live.shared_spells.global_special_state.input_yaw = 37;
        live.shared_spells.global_special_state.input_pitch = -21;
        live.shared_spells.global_special_state.cast_auxiliary = 11;
        let outcome = step_holder(
            &mut live.shared_spells,
            &mut live.economy,
            &mut live.terrain,
            &particles,
            holder_slot,
            CastInput {
                aim_yaw: 120,
                aim_pitch: 55,
                auxiliary_byte: 7,
                castle_mana: Some(rules::SOURCE_RULES[0].castle_mana_required),
            },
        )
        .expect("Alliance holder can reuse its reclaimed parent");
        assert_eq!(outcome.projectile, Some(parent_slot));
        assert_eq!(outcome.clear_player_aux, None);
        assert_eq!(live.shared_spells.global_special_state.cast_auxiliary, 0);
        let projectile = &live.economy.entities[usize::from(parent_slot)];
        assert_eq!(
            (projectile.class, projectile.model, projectile.action),
            (9, 25, 26)
        );
        assert_eq!(projectile.position, Position::default());
        assert_eq!(
            (
                projectile.yaw,
                projectile.pitch,
                projectile.source_dword_10()
            ),
            (37, -21, 11),
            "the current shared A4 supplies and clears the cast input after NewEvent"
        );
        assert_eq!(
            projectile.mana, 50,
            "the player cache must not overwrite the new carrier"
        );
        assert_eq!(
            projectile.mana_regen,
            -rules::SOURCE_RULES[0].mana_cost,
            "sub_68DE0 charges the freshly constructed fixed row"
        );
        assert!(
            live.economy
                .economy
                .events()
                .iter()
                .any(|event| matches!(event,
            super::super::Level2EconomyEvent::PreparedSound {
                source_entity_index, entity_index, sound_id: 9, ..
            } if *source_entity_index == parent_slot && *entity_index == parent_slot))
        );
    }

    #[test]
    fn already_reused_parent_reads_current_shared_cast_input_and_castle() {
        let mut base = crate::native_world::tests::level_two_campaign_session();
        // Release an earlier slot before the turn so the actual carrier is
        // born behind the source cursor and first visited on the next pass.
        let earlier_slot = {
            let live = &mut base.session.state.live;
            construct_impact(&mut live.economy, &mut live.terrain)
                .expect("reserve an ordinary earlier source slot")
        };
        base.apply_carried_spell_families(&[24], Some(24)).unwrap();
        let parent_slot = base.state.entity_index;
        let holder_slot = base.shared_spells().local_player.enabled_spell_holders[24];
        assert!(earlier_slot < holder_slot && earlier_slot != parent_slot);
        let castle = base
            .economy_mut()
            .build_castle_at(
                Position::default(),
                Position::default(),
                parent_slot,
                false,
                true,
            )
            .expect("controlled owner Castle");
        let requirement = base.shared_spells().pickup(holder_slot).unwrap().mana_regen;
        assert!(requirement > 0);
        assert!(
            base.economy_mut()
                .set_castle_mana_prerequisite_for_test(castle, requirement)
        );
        {
            let live = &mut base.session.state.live;
            live.economy.finish_source_group();
            crate::world::terrain::cave_features::reclaim(
                &mut live.economy,
                &mut live.terrain,
                parent_slot,
            );
            let slot = construct_impact(&mut live.economy, &mut live.terrain)
                .expect("reclaimed parent is available to the actual impact constructor");
            assert_eq!(slot, parent_slot);
            let row = &mut live.economy.entities[usize::from(slot)];
            row.set_life(100);
            row.set_mana(rules::SOURCE_RULES[0].mana_cost);
            crate::world::terrain::cave_features::reclaim(
                &mut live.economy,
                &mut live.terrain,
                earlier_slot,
            );
            let holder = live.shared_spells.pickup_mut(holder_slot).unwrap();
            holder.active_value = holder.activation_value.cast_signed();
            holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
            live.economy.entities[usize::from(holder_slot)] = holder.clone();
            live.shared_spells.global_special_state.cast_auxiliary = 11;
        }
        for (global_castle, should_launch) in [(0, false), (castle, true)] {
            let mut simulation = base.clone();
            simulation
                .session
                .state
                .live
                .shared_spells
                .global_special_state
                .castle_entity_index = global_castle;
            simulation.step_outer_turn(
                crate::PlayerCommand {
                    argument_6: 37,
                    argument_8: (-21_i16).cast_unsigned(),
                    ..Default::default()
                },
                0,
            );
            assert!(
                !simulation.dispatch_events().iter().any(|event| matches!(
                    event,
                    crate::native_world::Mc2DispatchEvent::Error { .. }
                )),
                "outer turn rolled back: {:?}",
                simulation.dispatch_events()
            );
            let live = &simulation.session.state.live;
            assert_eq!(
                (
                    live.shared_spells.global_special_state.input_yaw,
                    live.shared_spells.global_special_state.input_pitch
                ),
                (37, -21)
            );
            let projectile = live.economy.entities.iter().find(|row| {
                row.active
                    && (row.class, row.model, row.action) == (9, 25, 26)
                    && u16::from_le_bytes([row.record[42], row.record[43]]) == holder_slot
            });
            assert_eq!(
                projectile.is_some(),
                should_launch,
                "the current shared Castle address decides the gate, not the stale owner's Castle; parent={:?}, holder={:?}, castle={:?}, carriers={:?}, events={:?}",
                live.economy
                    .entities
                    .get(usize::from(parent_slot))
                    .map(|row| (
                        row.class,
                        row.model,
                        row.action,
                        row.life,
                        row.mana,
                        row.mana_regen,
                        row.special_relation
                    )),
                live.shared_spells.pickup(holder_slot).map(|row| (
                    row.active_value,
                    row.activation_value,
                    row.max_mana,
                    row.mana_regen
                )),
                live.economy
                    .castle_at(castle)
                    .map(|row| (row.active, row.mana)),
                live.economy
                    .entities
                    .iter()
                    .filter(|row| row.class == 9 && row.model == 25)
                    .map(|row| (
                        row.index,
                        row.active,
                        row.action,
                        row.yaw,
                        row.pitch,
                        row.source_dword_10()
                    ))
                    .collect::<Vec<_>>(),
                simulation.dispatch_events()
            );
            if let Some(projectile) = projectile {
                assert_ne!(projectile.index, parent_slot);
                assert_eq!(
                    (
                        projectile.yaw,
                        projectile.pitch,
                        projectile.source_dword_10()
                    ),
                    (37, -21, 11)
                );
                assert_eq!(live.shared_spells.global_special_state.cast_auxiliary, 0);
                assert_eq!(
                    live.economy.entities[usize::from(parent_slot)].mana_regen,
                    -rules::SOURCE_RULES[0].mana_cost
                );
            } else {
                assert_eq!(live.shared_spells.global_special_state.cast_auxiliary, 11);
            }
        }
    }

    #[test]
    fn carried_alliance_holder_launch_uses_aim_not_map_links() {
        let mut base = crate::native_world::tests::level_two_campaign_session();
        // The retained family is a controlled campaign input; its holder and
        // arena slot are created by the real carry constructor.
        base.apply_carried_spell_families(&[24], Some(24)).unwrap();
        let parent_slot = base.state.entity_index;
        let holder_slot = base.shared_spells().local_player.enabled_spell_holders[24];
        assert_ne!(holder_slot, 0);
        let rule = rules::SOURCE_RULES[0];
        let mut launches = Vec::new();
        for (aim_yaw, aim_pitch, map_next, spare_pitch) in [
            (0_i16, 0_i16, 0_u16, 0_i16),
            (0, 0, 123, -456),
            (32, -64, 0, 0),
            (32, -64, 123, -456),
        ] {
            let mut simulation = base.clone();
            let particles = simulation.session.definition.presentation.particles.clone();
            let live = &mut simulation.session.state.live;
            let holder = live.shared_spells.pickup_mut(holder_slot).unwrap();
            assert_eq!((holder.class, holder.model, holder.action), (15, 24, 72));
            assert_eq!(holder.source_spell_parent_entity_index(), parent_slot);
            // Stage the source activation boundary; the actual holder visit
            // below performs the allocation and pulse with controlled mana.
            holder.active_value = holder.activation_value.cast_signed();
            holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
            let parent = &mut live.economy.entities[usize::from(parent_slot)];
            parent.set_mana(rule.mana_cost);
            parent.set_yaw(i16::MAX - 15);
            parent.set_pitch(i16::MIN + 31);
            // These are independent raw map-link/spare words, not input aim.
            parent.record[28..30].copy_from_slice(&map_next.to_le_bytes());
            parent.record[172..174].copy_from_slice(&spare_pitch.to_le_bytes());
            let parent_position = parent.position;
            let expected_yaw = parent.yaw.wrapping_add(aim_yaw);
            let expected_pitch = parent.pitch.wrapping_add(aim_pitch);
            let mut expected_axis = parent_position;
            move_entity(&mut expected_axis, expected_yaw, expected_pitch, 16_384);
            let outcome = step_holder(
                &mut live.shared_spells,
                &mut live.economy,
                &mut live.terrain,
                &particles,
                holder_slot,
                CastInput {
                    aim_yaw,
                    aim_pitch,
                    auxiliary_byte: 7,
                    castle_mana: Some(rule.castle_mana_required),
                },
            )
            .expect("real Alliance holder visit");
            assert!(outcome.charged);
            let slot = outcome
                .projectile
                .expect("Alliance source carrier allocated");
            assert!(
                live.economy
                    .economy
                    .events()
                    .iter()
                    .any(|event| matches!(event,
                super::super::Level2EconomyEvent::PreparedSound {
                    source_entity_index, entity_index, sound_id: 9, ..
                } if *source_entity_index == slot && *entity_index == slot))
            );
            let flyer = &live.economy.entities[usize::from(slot)];
            assert_eq!((flyer.class, flyer.model, flyer.action), (9, 25, 26));
            assert_eq!((flyer.yaw, flyer.pitch), (expected_yaw, expected_pitch));
            assert_eq!(&flyer.record[158..160], &expected_axis.x.to_le_bytes());
            assert_eq!(&flyer.record[160..162], &expected_axis.y.to_le_bytes());
            assert_eq!(&flyer.record[162..164], &expected_axis.z.to_le_bytes());
            launches.push((flyer.yaw, flyer.pitch, expected_axis));
        }
        assert_eq!(
            launches[0], launches[1],
            "map links do not change zero-input aim"
        );
        assert_eq!(
            launches[2], launches[3],
            "map links do not change nonzero aim"
        );
        assert_ne!(launches[0], launches[2], "latched aim changes the carrier");
    }
}
