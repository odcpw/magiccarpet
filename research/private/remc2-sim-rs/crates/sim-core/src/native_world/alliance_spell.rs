//! Runtime adapter for the Alliance holder, ordinary model25 carrier, and
//! scheduled class10/model74 terminal consumer.
//!
//! The terminal consumer writes the source StageVar/parent/timer fields that
//! normal creature visits already read. This leaf owns those writes through
//! the focused Alliance kernel; scheduler tails and following-pass reclaim
//! remain with the shared dispatch owner.

use super::{EntitySnapshot, Level2SharedSpellResetSession, ParticleParameter, SessionTerrain};
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
    if slot == parent_slot || slot == holder_slot {
        return Err("Alliance source allocator selected a live parent or holder".to_owned());
    }
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
) -> Result<Option<u16>, String> {
    let parent_before = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|parent| parent.index == parent_slot && parent.active)
        .cloned()
        .ok_or("Alliance constructor parent is no longer live")?;
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
            .filter(|parent| parent.index == parent_slot && parent.active)
            .cloned()
            .ok_or("Alliance parent was lost during source allocation")?;
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
        row.set_source_dword_10(i32::from(parameters.pending_cast_auxiliary));
        put_word(row, 42, holder_slot);
        put_word(row, 44, holder.source_spell_parent_entity_index());
        set_owner(row, parent.source_id);
        let yaw = parent.yaw.wrapping_add(parent.next_entity() as i16);
        let pitch = parent
            .pitch
            .wrapping_add(i16::from_le_bytes([parent.record[172], parent.record[173]]));
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
    let rule = *rules::SOURCE_RULES
        .get(usize::from(tier))
        .ok_or("Alliance holder tier is outside owner SPELLS.DAT")?;
    let parent_slot = holder.source_spell_parent_entity_index();
    let parent = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|parent| parent_slot != 0 && parent.active)
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
        next_entity: parent.next_entity(),
        entity_index2: u16::from_le_bytes([parent.record[172], parent.record[173]]),
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
        if let Some(authority) = arena.entities.get_mut(usize::from(parent_slot)) {
            authority.set_mana_regen(caster.mana_regen);
            shared.sync_player_entity_from_authority(authority);
            if parent_slot == shared.player_entity.index {
                shared.sync_typed_spell_parent_caches_from_player_entity();
            }
        }
        if visit.projectile_allocation_attempts != 0 {
            let parameters = rule.projectile_parameters(caster, slot);
            outcome.projectile =
                construct_carrier(arena, terrain, particles, parent_slot, slot, parameters)?;
            if let Some(projectile) = outcome.projectile {
                outcome.clear_player_aux = Some(parent_slot);
                arena
                    .economy
                    .prepare_sound(projectile, parent_slot, -1, parameters.sound);
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
                .and_then(|value| u8::try_from(value).ok())
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
    // rather than at the creature's original construction. Observe that live
    // address now so a later reuse cannot prolong the new Alliance binding.
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
