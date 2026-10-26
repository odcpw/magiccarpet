//! Runtime adapter for the Magic Mine holder, capture, and model78 field.
//!
//! Holder pulses create ordinary class9/model29 source rows. The field kernel
//! owns charging, target selection, and payload state; this adapter runs the
//! source constructor before applying holder overrides, then reports actual
//! projectile yaw (or `None`) back through `complete_field_projectiles`.

use super::{EntitySnapshot, Level2SharedSpellResetSession, ParticleParameter, SessionTerrain};
use crate::campaign::source_arena::SourceArena;
use crate::level1_mission::projectile_guidance_angles;
use crate::spell_activation::ProjectileSpellCastInput as CastInput;
use crate::spells::late::magic_mine as rules;
use crate::world::fields::magic_mine::{self as field, CapturedSpell, TerrainSample};
use crate::{FlightWorld, Position, move_entity, turn_toward};

const HOLDER_MODEL: u8 = 23;
const HOLDER_ACTION: u8 = 69;
const PROJECTILE_CLASS: u8 = 9;
const PROJECTILE_MODEL: u8 = 29;
const PROJECTILE_ACTION: u8 = 30;
const PROJECTILE_SPEED: i16 = 384;
const PROJECTILE_MAX_LIFE: i32 = 10;
const PROJECTILE_PARAMETER: u16 = 60;
const PROJECTILE_PARTICLE: u16 = 66;
const TARGET_CLASS: u8 = 10;
const TARGET_MODEL: u8 = 78;

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ProjectileTerminal {
    #[default]
    InFlight,
    FieldCreated {
        field_slot: u16,
    },
    FieldAllocationFailed,
    MineContact {
        target_slot: u16,
        impact_created: bool,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProjectileOutcome {
    /// Reuses the common outcome carrier for the actual terminal row so the
    /// source dispatcher can schedule it in the same ordinary pass.
    pub callback: super::guided_projectile::StepOutcome,
    /// Distinguishes successful field creation, retryable failure, and the
    /// source interception branch that hides the flyer even if its visual
    /// allocation fails.
    pub terminal: ProjectileTerminal,
    pub projectile_hidden: bool,
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
        .ok_or("Magic Mine source constructor needs particle66")?;
    if particle.animation_limit().is_none() {
        return Err("Magic Mine particle66 has an unsupported render mode".to_owned());
    }
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    if slot == parent_slot || slot == holder_slot {
        return Err("Magic Mine source allocator selected a live parent or holder".to_owned());
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

/// Source `sub_68E50` hand placement for the newly constructed row. Its
/// source hand flags select one of the two offsets, and terrain can reject
/// that offset before the holder adds the caster's FOV height.
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

/// `sub_6CAC0` calls the real `(9,29)` constructor before copying holder
/// state. Parent and holder rows are reread after allocation so an arena slot
/// reuse cannot leave the source yaw, FOV, or mana overrides stale.
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
        .ok_or("Magic Mine constructor parent is no longer live")?;
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
            .ok_or("Magic Mine parent was lost during source allocation")?;
        let holder = arena
            .entities
            .get(usize::from(holder_slot))
            .filter(|holder| {
                holder.index == holder_slot
                    && holder.active
                    && (holder.class, holder.model) == (15, HOLDER_MODEL)
            })
            .cloned()
            .ok_or("Magic Mine holder was lost during source allocation")?;
        let row = arena
            .entities
            .get_mut(usize::from(slot))
            .ok_or("Magic Mine projectile address is outside the source arena")?;
        let placed = hand_position(&parent, row.position, terrain);
        row.set_position(placed);
        row.set_actual_speed(PROJECTILE_SPEED.wrapping_add(parent.actual_speed));
        row.record[71] = TARGET_CLASS;
        row.record[72] = TARGET_MODEL;
        row.set_mana(holder.mana);
        row.set_subspell_index(u16::from(holder.record[74]));
        row.set_source_byte_46(holder.record[74]);
        row.set_source_dword_10(i32::from(parameters.pending_cast_auxiliary));
        put_word(row, 42, holder_slot);
        set_owner(row, parent.source_id);
        let yaw = parent.yaw.wrapping_add(parent.next_entity() as i16);
        let pitch = parent
            .pitch
            .wrapping_add(i16::from_le_bytes([parent.record[172], parent.record[173]]));
        let mut destination = parent.position;
        move_entity(&mut destination, yaw, 0, 4_096);
        destination.z = terrain.ground_height(destination);
        put_word(row, 158, destination.x);
        put_word(row, 160, destination.y);
        row.record[162..164].copy_from_slice(&destination.z.to_le_bytes());
        row.set_yaw(yaw);
        row.set_pitch(pitch);
        let final_position = Position {
            z: row.position.z.wrapping_add(parent.array_fov()),
            ..row.position
        };
        row.set_position(final_position);
        Ok(final_position)
    })();
    let final_position = match post_allocation {
        Ok(position) => position,
        Err(error) => {
            crate::world::terrain::cave_features::reclaim(arena, terrain, slot);
            return Err(error);
        }
    };
    if let Err(error) =
        crate::world::effects::sync_position(arena, terrain, slot, final_position, true, true)
    {
        crate::world::terrain::cave_features::reclaim(arena, terrain, slot);
        return Err(error);
    }
    Ok(Some(slot))
}

/// One ordinary class15/model23/action69 holder callback. The leaf owns the
/// source cost gate, model29 constructor, and holder writes; the scheduler
/// applies the returned same-pass tail and following-pass reclaim.
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
        .ok_or_else(|| format!("Magic Mine holder {slot} is not live class15/model23/action69"))?;
    let tier = crate::spell_activation::level2_applied_tier(holder.level_gate)
        .ok_or("Magic Mine holder has a negative source tier")?;
    let rule = *rules::SOURCE_RULES
        .get(usize::from(tier))
        .ok_or("Magic Mine holder tier is outside owner SPELLS.DAT")?;
    let parent_slot = holder.source_spell_parent_entity_index();
    let parent = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|parent| parent_slot != 0 && parent.active)
        .cloned();

    // Preserve the current holder row before NewEvent can recycle an arena
    // address, matching the ordinary source callback's pointer ordering.
    let Some(holder_row) = arena.entities.get_mut(usize::from(slot)) else {
        return Err("Magic Mine holder address is outside the source arena".to_owned());
    };
    *holder_row = holder.clone();

    let mut caster = parent.as_ref().map(|parent| rules::Caster {
        entity_index: parent.index,
        position: parent.position,
        terrain_altitude: terrain.ground_height(parent.position),
        life: parent.life,
        mana: parent.mana,
        mana_regen: parent.mana_regen,
        castle_mana: input.castle_mana,
        yaw: parent.yaw,
        pitch: parent.pitch,
        height_offset: parent.array_fov(),
        next_entity: parent.next_entity(),
        entity_index2: u16::from_le_bytes([parent.record[172], parent.record[173]]),
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
            let parameters = rule.projectile_parameters(caster, slot, holder.mana);
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
                .ok_or("Magic Mine queued profile tier is outside the source table")?;
            let profile = shared
                .physical_spell_profile(23, deferred_tier, parent_slot, None)
                .ok_or("Magic Mine queued profile is missing from owner SPELLS.DAT")?;
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

/// Construct the map-linked class10/model78 field through its existing source
/// constructor (`sub_50840`).
pub(crate) fn construct_field(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    particle: ParticleParameter,
) -> Result<Option<u16>, String> {
    field::construct(arena, terrain, position, particle)
}

/// The accepted source collision tail (`sub_68AC0`). The returned holder row
/// is also copied into shared spell state so the ordinary model23 callback
/// sees its inactive value on the next scheduled visit.
pub(crate) fn capture_flyer(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    mine_slot: u16,
    flyer_slot: u16,
) -> Option<field::Capture> {
    let capture = field::capture(&mut arena.entities, mine_slot, flyer_slot)?;
    if let Some(holder) = arena
        .entities
        .get(usize::from(capture.holder_slot))
        .cloned()
        && let Some(shared_holder) = shared.pickup_mut(capture.holder_slot)
    {
        *shared_holder = holder;
    }
    arena
        .economy
        .prepare_sound(capture.sound_entity, capture.mine_slot, -1, capture.sound);
    Some(capture)
}

/// One scheduled model78 callback. `captured` is resolved from the typed
/// source spell profile by the caller; `visit.projectiles` must be sent through
/// the normal family constructor before `complete_field_projectiles` runs.
pub(crate) fn step_field(
    arena: &mut SourceArena,
    mine_slot: u16,
    map_order: &[u16],
    captured: Option<CapturedSpell>,
    terrain: TerrainSample,
) -> Result<field::Visit, String> {
    let visit = field::visit(&mut arena.entities, mine_slot, map_order, captured, terrain)?;
    if let Some(mine) = arena.entities.get(usize::from(mine_slot)) {
        arena.economy.source_rng.insert(mine_slot, mine.entity_rng);
    }
    Ok(visit)
}

/// Finish one model78 firing visit from actual constructor results. The caller
/// supplies `Some(yaw)` only after an ordinary constructor returns a live
/// projectile and reads that row's yaw; failed or unsupported constructors
/// supply `None` and leave the mine's charge in place.
pub(crate) fn complete_field_projectiles(
    arena: &mut SourceArena,
    mine_slot: u16,
    projectile_yaws: &[Option<i16>],
) -> Result<bool, String> {
    field::complete_projectiles(&mut arena.entities, mine_slot, projectile_yaws)
}

fn source_guidance(
    row: &EntitySnapshot,
    parameters: &[[u8; super::ENTITY_PARAMETER_BYTES]],
) -> Result<super::guided_projectile::Guidance, String> {
    let relation = row
        .parameter_relation
        .filter(|relation| *relation == PROJECTILE_PARAMETER)
        .ok_or("Magic Mine action30 projectile lost source parameter60")?;
    let descriptor = parameters
        .get(usize::from(relation))
        .ok_or("Magic Mine source parameter60 is outside the entity table")?;
    Ok(super::guided_projectile::Guidance {
        // sub_65610 reads subtype_2 as sub_58350's turn cap.
        yaw_turn: i16::from_le_bytes([descriptor[2], descriptor[3]]),
        // Pitch reads word_0x6; word_0x4 is a separate source parameter.
        pitch_turn: i16::from_le_bytes([descriptor[6], descriptor[7]]),
    })
}

/// sub_106C0 tests only the selected target after sub_65610 has steered the
/// flyer. Preserve signed-16 XY subtraction and the source's strict extents.
fn overlaps_target(projectile: &EntitySnapshot, target: &EntitySnapshot) -> bool {
    let dx = i32::from(target.position.x.wrapping_sub(projectile.position.x) as i16).abs();
    let dy = i32::from(target.position.y.wrapping_sub(projectile.position.y) as i16).abs();
    let dz = (i32::from(target.position.z) + i32::from(target.array_yaw())
        - i32::from(projectile.position.z)
        - i32::from(projectile.array_yaw()))
    .abs();
    dx < i32::from(projectile.array_pitch()) + i32::from(target.array_pitch())
        && dy < i32::from(projectile.array_roll()) + i32::from(target.array_roll())
        && dz < i32::from(projectile.array_fov()) + i32::from(target.array_fov())
}

fn closest_enemy_mine(
    arena: &SourceArena,
    projectile: &EntitySnapshot,
    targets: &super::guided_projectile::Targets<'_>,
) -> Option<u16> {
    let owner = arena.entities.get(usize::from(projectile.source_id))?;
    if owner.index != projectile.source_id || owner.class != 3 {
        return None;
    }
    let mut closest_distance = 0x1_0000_u32;
    let mut selected = None;
    for &slot in targets.class10 {
        if slot == 0 {
            continue;
        }
        let Some(candidate) = arena.entities.get(usize::from(slot)) else {
            continue;
        };
        if candidate.index != slot
            || !candidate.active
            || candidate.model != TARGET_MODEL
            || candidate.word_32() == projectile.source_id
            || candidate.subspell_index >= 2
        {
            continue;
        }
        let distance = crate::distance_3d_wrapped(projectile.position, candidate.position);
        if distance < u32::from(targets.owner_target_range) && distance < closest_distance {
            closest_distance = distance;
            selected = Some(slot);
        }
    }
    selected
}

fn aim_at_target(
    projectile: &mut EntitySnapshot,
    target: &EntitySnapshot,
    parameters: &[[u8; super::ENTITY_PARAMETER_BYTES]],
) -> Result<(), String> {
    let guidance = source_guidance(projectile, parameters)?;
    let target_position =
        super::guided_projectile::centre(target.position, target.model, target.array_yaw());
    let (target_yaw, target_pitch) =
        projectile_guidance_angles(projectile.position, target_position);
    projectile.set_source_roll(target_yaw);
    projectile.set_fov(target_pitch);
    projectile.set_yaw(
        projectile
            .yaw
            .wrapping_add(turn_toward(projectile.yaw, target_yaw, guidance.yaw_turn))
            & 2047,
    );
    projectile.set_pitch(
        projectile.pitch.wrapping_add(turn_toward(
            projectile.pitch,
            target_pitch,
            guidance.pitch_turn,
        )) & 2047,
    );
    Ok(())
}

/// The source-specific action30 handler (`sub_67960`), not the shared
/// sub_65820 guided-projectile callback. It seeks one hostile model78 field,
/// steers/collides only against that saved target, and retries a failed
/// 10/78 terminal allocation without hiding the flyer.
pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    targets: &super::guided_projectile::Targets<'_>,
    parameters: &[[u8; super::ENTITY_PARAMETER_BYTES]],
) -> Result<ProjectileOutcome, String> {
    let mut projectile = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| {
            row.index == slot
                && row.active
                && (row.class, row.model, row.action)
                    == (PROJECTILE_CLASS, PROJECTILE_MODEL, PROJECTILE_ACTION)
        })
        .ok_or("Magic Mine callback needs live class9/model29/action30")?
        .clone();
    let mut outcome = ProjectileOutcome::default();

    // sub_67960 only searches at the original full-life boundary and only for
    // captured tiers >= 2. The global source class10 list order breaks ties.
    if projectile.life == projectile.max_life && projectile.subspell_index >= 2 {
        projectile.clear_source_word_96();
        if let Some(target) = closest_enemy_mine(arena, &projectile, targets) {
            projectile.set_max_life(32);
            projectile.set_life(31);
            projectile.set_source_word_96(target);
            let target = arena
                .entities
                .get(usize::from(target))
                .filter(|row| row.index == target)
                .ok_or("Magic Mine seeker target was reclaimed during selection")?;
            // sub_655C0 computes its aim while temporarily adding array.yaw
            // to the target height, then restores the target row.
            let target_position =
                super::guided_projectile::centre(target.position, target.model, target.array_yaw());
            let (yaw, pitch) = projectile_guidance_angles(projectile.position, target_position);
            projectile.set_source_roll(yaw);
            projectile.set_fov(pitch);
        }
    }

    let speed_delta = i32::from(projectile.minimum_speed()) - i32::from(projectile.actual_speed);
    projectile.set_actual_speed(
        projectile
            .actual_speed
            .wrapping_add((2 * speed_delta.signum()) as i16),
    );

    let target_slot = projectile.source_word_96();
    if target_slot != 0 {
        let target = arena
            .entities
            .get(usize::from(target_slot))
            .filter(|target| {
                target.index == target_slot
                    && target.active
                    && target.life >= 0
                    && !target.drawing_disabled()
            })
            .cloned();
        if let Some(target) = target {
            aim_at_target(&mut projectile, &target, parameters)?;
            if overlaps_target(&projectile, &target) {
                let impact_position = Position {
                    z: target.position.z.wrapping_add(if target.model == 2 {
                        0
                    } else {
                        target.array_yaw()
                    }),
                    ..target.position
                };
                projectile.set_position(impact_position);
                arena.entities[usize::from(slot)] = projectile;
                crate::world::effects::sync_position(
                    arena,
                    terrain,
                    slot,
                    impact_position,
                    true,
                    false,
                )?;
                arena.entities[usize::from(target_slot)].set_source_byte_46(8);
                let impact = crate::world::effects::allocate(
                    arena,
                    terrain,
                    sim_definition::adapter::mc2_effect_definition(
                        crate::level_definition::EffectKind::Impact,
                    ),
                    impact_position,
                    particles,
                )?;
                arena.entities[usize::from(slot)].disable_drawing();
                outcome.callback.impact_slot = impact;
                outcome.terminal = ProjectileTerminal::MineContact {
                    target_slot,
                    impact_created: impact.is_some(),
                };
                outcome.projectile_hidden = true;
                return Ok(outcome);
            }
        } else {
            projectile.clear_source_word_96();
        }
    }

    let mut position = projectile.position;
    move_entity(
        &mut position,
        projectile.yaw,
        projectile.pitch,
        projectile.actual_speed,
    );
    projectile.set_position(position);
    let ground = terrain.ground_height(position);
    let terminal_height = if ground > position.z {
        Some(ground)
    } else {
        terrain
            .ceiling_height(position)
            .map(|ceiling| ceiling.wrapping_sub(projectile.array_fov()))
            .filter(|ceiling| position.z > *ceiling)
    };
    let hit_terrain = terminal_height.is_some();
    if let Some(z) = terminal_height {
        projectile.set_position(Position { z, ..position });
        position.z = z;
    }
    projectile.set_life(projectile.life.wrapping_sub(1));
    let terminal = hit_terrain || projectile.life < 0;
    arena.entities[usize::from(slot)] = projectile.clone();
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if !terminal {
        return Ok(outcome);
    }

    let particle = *particles
        .get(usize::from(PROJECTILE_PARTICLE))
        .ok_or("Magic Mine terminal constructor needs particle66")?;
    let Some(field_slot) = construct_field(arena, terrain, position, particle)? else {
        outcome.terminal = ProjectileTerminal::FieldAllocationFailed;
        return Ok(outcome);
    };
    let field = arena
        .entities
        .get_mut(usize::from(field_slot))
        .filter(|field| {
            field.index == field_slot && field.class == TARGET_CLASS && field.model == TARGET_MODEL
        })
        .ok_or("Magic Mine constructor returned without a live 10/78 field")?;
    field.record[54..56].copy_from_slice(&projectile.source_id.to_le_bytes());
    field.set_source_byte_46(0);
    field.set_subspell_index(projectile.subspell_index);
    arena.entities[usize::from(slot)].disable_drawing();
    outcome.callback.impact_slot = Some(field_slot);
    outcome.terminal = ProjectileTerminal::FieldCreated { field_slot };
    outcome.projectile_hidden = true;
    Ok(outcome)
}

/// Typed request seam used by the existing ordinary family constructor.
pub(crate) fn projectile_requests(visit: &field::Visit) -> &[field::ProjectileRequest] {
    &visit.projectiles
}
