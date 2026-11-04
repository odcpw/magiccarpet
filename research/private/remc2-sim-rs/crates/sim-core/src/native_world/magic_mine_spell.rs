//! Runtime adapter for the Magic Mine holder, capture, and model78 field.
//!
//! Holder pulses create ordinary class9/model29 source rows. The field kernel
//! owns charging, target selection, and payload state; this adapter runs the
//! source constructor before applying holder overrides, then commits each
//! successful projectile's actual yaw before the fanout tail.

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
    shared: &Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    parent_slot: u16,
    holder_slot: u16,
    input: CastInput,
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
        let input = if parent.special_relation == super::EntitySpecialRelation::GlobalSpecialState {
            CastInput {
                aim_yaw: shared.global_special_state.input_yaw,
                aim_pitch: shared.global_special_state.input_pitch,
                auxiliary_byte: shared.global_special_state.cast_auxiliary,
                ..input
            }
        } else {
            input
        };
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
        row.set_source_dword_10(i32::from(input.auxiliary_byte));
        put_word(row, 42, holder_slot);
        set_owner(row, parent.source_id);
        let yaw = parent.yaw.wrapping_add(input.aim_yaw);
        let pitch = parent.pitch.wrapping_add(input.aim_pitch);
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
    let mut rule = *rules::SOURCE_RULES
        .get(usize::from(tier))
        .ok_or("Magic Mine holder tier is outside owner SPELLS.DAT")?;
    rule.mana_cost = holder.max_mana;
    rule.castle_mana_required = holder.mana_regen;
    let parent_slot = holder.source_spell_parent_entity_index();
    let parent = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|_| parent_slot != 0)
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
        next_entity: input.aim_yaw as u16,
        entity_index2: input.aim_pitch as u16,
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

    if let (Some(_), Some(caster)) = (parent.as_ref(), caster) {
        if visit.projectile_allocation_attempts != 0 {
            let parameters = rule.projectile_parameters(caster, slot, holder.mana);
            outcome.projectile =
                construct_carrier(shared, arena, terrain, particles, parent_slot, slot, input)?;
            if let Some(projectile) = outcome.projectile {
                if arena.entities[usize::from(parent_slot)].special_relation
                    == super::EntitySpecialRelation::GlobalSpecialState
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
    }

    // Source charging follows NewEvent, including a failed allocation, and
    // reads the current fixed-address parent and physical holder price.
    if visit.mana_pulse {
        let cost = arena.entities[usize::from(slot)].max_mana;
        if let Some(parent) = arena.entities.get_mut(usize::from(parent_slot)) {
            parent.set_mana_regen(if parent.mana_regen >= 0 {
                cost.wrapping_neg()
            } else {
                parent.mana_regen.wrapping_sub(cost)
            });
            shared.sync_player_entity_from_authority(parent);
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
/// the normal family constructor and its per-shot completion.
pub(crate) fn step_field(
    shared: &Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    mine_slot: u16,
    map_order: &[u16],
    captured: Option<CapturedSpell>,
    terrain: &SessionTerrain,
) -> Result<field::Visit, String> {
    let row = arena.entities.get(usize::from(mine_slot))
        .ok_or("Magic Mine slot is outside the source arena")?;
    let position = field::recoil_position(row);
    let owner_slot = row.word_32();
    let context = || super::source_target::Context { spells: Some(shared), ..Default::default() };
    let owner = super::source_target::status_parent(arena, context(), owner_slot)?;
    let owner_color = match &owner {
        Some(owner) => super::source_target::status_player_color(shared, owner)?,
        None => None,
    };
    let holder_slot = shared.local_player.enabled_spell_holders[12];
    let holder = if holder_slot == 0 {
        None
    } else {
        super::source_target::status_parent(arena, context(), holder_slot)?
    };
    let visibility = field::Visibility {
        owner_available: owner.as_ref().is_some_and(|owner| owner.life >= 0 && !owner.drawing_disabled()),
        owner_color,
        local_color: shared.local_player.slot,
        beyond_sight: holder.is_some_and(|holder| holder.source_word_2e() != 0 && holder.record[74] >= 2),
    };
    let sample = TerrainSample {
        ground_height: terrain.ground_height(position),
        cave_ceiling: terrain.ceiling_height(position),
        water: terrain.material_mask(position) == 1,
    };
    let visit = field::visit(&mut arena.entities, mine_slot, map_order, captured, sample, visibility)?;
    if let Some(mine) = arena.entities.get(usize::from(mine_slot)) {
        arena.economy.source_rng.insert(mine_slot, mine.entity_rng);
    }
    Ok(visit)
}

/// Commit one successful constructor before the next source allocation.
pub(crate) fn complete_field_projectile(
    arena: &mut SourceArena,
    mine_slot: u16,
    yaw: i16,
) -> Result<bool, String> {
    field::complete_projectile(&mut arena.entities, mine_slot, yaw)
}

/// Apply the sticky exhaustion tail after the complete fanout loop.
pub(crate) fn finish_field_projectile_burst(
    arena: &mut SourceArena,
    mine_slot: u16,
    exhausted: bool,
) -> Result<bool, String> {
    field::finish_projectile_burst(&mut arena.entities, mine_slot, exhausted)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_magic_mine_gates_acquisition_and_public_tiers_create_fields() {
        use crate::PlayerCommand;
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        use crate::level_definition::late_spells::MAGIC_MINE_PROPERTIES;
        use crate::native_world::{Mc2DispatchEvent, LEVEL2_SPELL_TIER_THRESHOLDS};
        use crate::native_world::simulation::CarriedSpellProgress;

        let package = std::env::var_os("MC_MINE_LEVEL18_PACKAGE").map_or_else(
            || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/all-spells-20261104-integration/current-owner-package/level18"),
            std::path::PathBuf::from,
        );
        let mut acquired = CampaignSession::open(package, CampaignSelection::ordinary(18).unwrap())
            .expect("actual Level 18 owner package opens");
        let player = acquired.simulation().state.entity_index;
        assert_eq!(acquired.simulation().shared_spells().local_player.enabled_spell_holders[23], 0);
        let gate = &acquired.simulation().entities()[89];
        assert!(gate.active);
        assert_eq!((gate.class, gate.model, gate.action, gate.source_id, gate.level_gate), (11, 32, 32, 14, 1));
        // Stage the real gate's objective prerequisite, not earned progression.
        {
            let runtime = &mut acquired.simulation_mut().session.state.live.core.runtime;
            runtime.stage_index = 2;
            for owner in 0..runtime.objectives.len() {
                runtime.set_level2_objective_stage_selector_for(owner, 1, 2);
            }
        }
        let mut gate_fired = false;
        for _ in 0..4 {
            acquired.step(PlayerCommand::default(), 0);
            assert!(!acquired.simulation().dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })), "Level18 objective gate: {:?}", acquired.simulation().dispatch_events());
            gate_fired |= acquired.simulation().dispatch_events().iter().any(|event| matches!(event,
                Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 89, group: 14, authored_indices, .. }
                    if authored_indices.as_ref() == [359]));
            if gate_fired { break; }
        }
        assert!(gate_fired, "actual objective gate produces authored proximity switch359");
        let switch = *acquired.simulation().economy().authored_switch_producers().iter()
            .find(|switch| switch.authored_index == 359 && switch.active)
            .expect("group14 publishes the real group15 proximity switch");
        acquired.simulation_mut().set_level2_entity_position(player, switch.position).unwrap();
        let mut switch_fired = false;
        for _ in 0..10 {
            acquired.step(PlayerCommand::default(), 0);
            assert!(!acquired.simulation().dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })), "Level18 proximity gate: {:?}", acquired.simulation().dispatch_events());
            switch_fired |= acquired.simulation().dispatch_events().iter().any(|event| matches!(event,
                Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group: 15, authored_indices, .. }
                    if *source_slot == switch.source_entity_index && authored_indices.contains(&370)));
            if switch_fired { break; }
        }
        assert!(switch_fired, "ordinary overlap fires the actual group15 switch");
        let pickup = *acquired.simulation().economy().spell_pickups().iter()
            .find(|pickup| pickup.authored_index == 370 && pickup.family == 23)
            .expect("group15 creates the real Magic Mine pickup");
        assert_eq!(pickup.action, 70);
        acquired.simulation_mut().set_level2_entity_position(player, pickup.position).unwrap();
        let mut receipt = None;
        for _ in 0..8 {
            acquired.step(PlayerCommand::default(), 0);
            receipt = acquired.simulation().dispatch_events().iter().find_map(|event| match event {
                Mc2DispatchEvent::SpellAcquisition { source_slot, family: 23, result: Some(outcome), .. }
                    if *source_slot == pickup.source_entity_index => Some(*outcome),
                _ => None,
            });
            if receipt.is_some() { break; }
        }
        let receipt = receipt.expect("ordinary pickup callback acquires Magic Mine");
        assert_eq!((receipt.source_slot, receipt.family, receipt.holder_action, receipt.parent_entity_index),
            (pickup.source_entity_index, 23, 69, player));
        let stone = acquired.simulation().entities().iter().find(|row| row.active
            && (row.class, row.model, row.action) == (2, 3, 9)
            && row.position.x == (91 << 8) + 128 && row.position.y == (68 << 8) + 128)
            .expect("the same real group15 constructed authored BadStone row369").index;
        let stone_phase = acquired.simulation().entities()[usize::from(stone)].phase;
        acquired.step(PlayerCommand::default(), 0);
        let session = acquired.simulation();
        assert!(!session.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })), "group15 BadStone callback: {:?}", session.dispatch_events());
        let stone_row = &session.entities()[usize::from(stone)];
        assert_eq!((stone_row.class, stone_row.model, stone_row.action), (2, 3, 9));
        assert_eq!(stone_row.phase, stone_phase.wrapping_add(1));
        assert_ne!(stone_row.control_flags & 0x0002_0000, 0);
        assert!(session.economy.map_chains.is_linked(stone));
        assert_eq!(stone_row.position.z, session.canonical_terrain().ground_height(stone_row.position));
        let stone_sprite = session.campaign_entity_presentations(0).unwrap().into_iter()
            .find(|sprite| sprite.entity_index == stone).expect("group15 BadStone reaches the actual sprite consumer");
        assert_eq!(stone_sprite.render_parameter_index, 270);
        assert_eq!(stone_sprite.base_tmap, session.session.definition.presentation.particles[270].base_tmap);

        for (tier, profile) in MAGIC_MINE_PROPERTIES.tiers.into_iter().enumerate() {
            let mut campaign = acquired.clone();
            let encounter_rival = if tier == 0 {
                let rival = campaign.simulation().entities().iter()
                    .find(|row| row.active && (row.class, row.model) == (3, 1) && row.life > 0)
                    .expect("Level18 contains a real opposing wizard").index;
                // Declared pre-cast pose: move the real rival away from its
                // actual Castle so its ordinary protection countdown is not
                // continuously refreshed by the Castle overlap.
                let (origin, castle_slot) = {
                    let live = &campaign.simulation().session.state.live;
                    let row = &live.economy.entities[usize::from(rival)];
                    let upkeep = live.nyphur_dispatcher.get_in_arena(&live.economy, rival)
                        .expect("the actual rival has its scheduled upkeep authority")
                        .common_upkeep(row.phase);
                    let origin = live.economy.entities.get(usize::from(upkeep.castle_entity_index))
                        .filter(|_| upkeep.castle_entity_index != 0)
                        .map_or(row.position, |castle| castle.position);
                    (origin, upkeep.castle_entity_index)
                };
                let mut safe_pose = Position { x: origin.x.wrapping_add(16_384), ..origin };
                let terrain = campaign.simulation().canonical_terrain();
                safe_pose.z = terrain.ground_height(safe_pose).saturating_add(512);
                if let Some(ceiling) = terrain.ceiling_height(safe_pose) {
                    safe_pose.z = safe_pose.z.min(ceiling.saturating_sub(256));
                }
                campaign.simulation_mut().set_level2_entity_position(rival, safe_pose).unwrap();
                // Source Nyphur starts with word159 protection. Let its real
                // common-upkeep callbacks expire it before constructing the
                // Mine; no protection or damage mailbox is staged.
                let mut unprotected = false;
                let mut protection_trace = Vec::new();
                for turn in 0..160 {
                    let session = campaign.simulation();
                    let live = &session.session.state.live;
                    let row = &live.economy.entities[usize::from(rival)];
                    let dispatcher = live.nyphur_dispatcher.get_in_arena(&live.economy, rival)
                        .expect("the actual rival has its scheduled upkeep authority");
                    let upkeep = dispatcher.common_upkeep(row.phase);
                    if turn % 16 == 0 {
                        let castle = live.economy.entities.get(usize::from(castle_slot))
                            .filter(|_| castle_slot != 0)
                            .map(|castle| (castle.position, castle.array_pitch(), castle.array_roll(), castle.array_fov()));
                        protection_trace.push((turn, upkeep.word_0x159, row.position, row.control_flags, castle));
                    }
                    if upkeep.word_0x159 == 0 {
                        unprotected = true;
                        break;
                    }
                    campaign.step(PlayerCommand::default(), 0);
                    assert!(!campaign.simulation().dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
                        "ordinary rival protection expiry: {:?}", campaign.simulation().dispatch_events());
                }
                assert!(unprotected, "real rival protection expires through ordinary upkeep: {protection_trace:?}");
                Some(rival)
            } else { None };
            let session = campaign.simulation_mut();
            session.apply_carried_spell_progression(&[CarriedSpellProgress {
                family: 23,
                persistent_experience: LEVEL2_SPELL_TIER_THRESHOLDS[23][tier],
                runtime_experience: 0,
                unlocked_tier: tier as i8,
                selected_tier: tier as i8,
            }]).unwrap();
            let holder = session.shared_spells().local_player.enabled_spell_holders[23];
            assert_eq!(holder, pickup.source_entity_index);
            assert_eq!(session.shared_spells().pickup(holder).unwrap().level_gate, tier as i8);
            let position = session.state.position;
            let castle = session.economy_mut().build_castle(position, player).unwrap();
            assert!(session.economy_mut().set_castle_mana_prerequisite_for_test(castle, profile.runtime_castle_mana_limit));
            let mut cast_position = Position { x: position.x.wrapping_add(4_096), ..position };
            cast_position.z = session.session.state.live.terrain.ground_height(cast_position).saturating_add(512);
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
            let existing = live.economy.entities.iter().filter(|row| row.active && (row.class,row.model)==(10,78)).map(|row| row.index).collect::<Vec<_>>();
            let start = session.effect_journal().records().len();
            let mut flyer_seen = false;
            let mut field_slot = None;
            let mut trace = Vec::new();
            for turn in 0..100 {
                campaign.step(if turn == 0 { PlayerCommand { opcode: 6, argument_1: 23, flags: 0x40, ..PlayerCommand::default() } } else { PlayerCommand::default() }, 0);
                let session = campaign.simulation();
                assert!(!session.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })), "Magic Mine tier {tier} turn {turn}: {:?}", session.dispatch_events());
                flyer_seen |= session.entities().iter().any(|row| row.active && (row.class,row.model,row.action)==(9,29,30) && u16::from_le_bytes([row.record[42], row.record[43]])==holder && row.subspell_index==tier as u16);
                field_slot = session.entities().iter().find(|row| row.active && (row.class,row.model,row.action)==(10,78,85) && row.word_32()==owner_id && row.subspell_index==tier as u16 && !existing.contains(&row.index)).map(|row| row.index);
                if trace.len() < 20 {
                    let rows = session.entities().iter().filter(|row| row.active && ((row.class,row.model)==(9,29) || (row.class,row.model)==(10,78)))
                        .map(|row| (row.index, row.class, row.model, row.action, row.control_flags, row.life, row.source_id, row.word_32(), row.subspell_index, row.position)).collect::<Vec<_>>();
                    if !rows.is_empty() { trace.push((turn, rows)); }
                }
                if field_slot.is_some() { break; }
            }
            assert!(flyer_seen, "acquired tier {tier} emits the actual Mine flyer");
            let field_slot = field_slot.unwrap_or_else(|| panic!("natural Mine tier{tier} owner{owner_id} holder{holder} flyer constructs its actual field; trace={trace:?}"));
            let session = campaign.simulation();
            let field = &session.entities()[usize::from(field_slot)];
            // The constructor starts at 1000; if its allocated slot is still
            // ahead of the source cursor, action85 already applies the live
            // tier's duration in that same pass.
            assert_eq!(field.max_life, if field.record[74] == 0 { 1_000 } else { profile.subspell_index });
            assert_eq!(u16::from_le_bytes([field.record[58],field.record[59]]), u16::MAX);
            assert!(session.effect_journal().records()[start..].iter().any(|record| matches!(record.effect,
                crate::campaign::journal::CampaignEffect::Dispatch(Mc2DispatchEvent::Economy {
                    event: super::super::Level2EconomyEvent::PreparedSound { source_entity_index, entity_index, player_index: -1, sound_id: 15 }, ..
                }) if source_entity_index == entity_index)),
                "actual Magic Mine flyer publishes source sound15");
            let sprite = session.campaign_entity_presentations(0).unwrap().into_iter().find(|sprite| sprite.entity_index == field_slot)
                .expect("natural Mine field reaches the sprite consumer");
            assert_eq!(Some(sprite.render_parameter_index), field.particle_parameter_index);
            assert_eq!(sprite.base_tmap, session.session.definition.presentation.particles[usize::from(sprite.render_parameter_index)].base_tmap);
            if tier == 0 {
                // Carry Fireball through the same campaign holder constructor
                // if the selected Level18 start has not already supplied it.
                // Resource and actor geometry below are controlled; the Mine,
                // Fireball, source targeting, contact and cleanup all tick via
                // the ordinary scheduler.
                let field_position = field.position;
                let generation = session.session.state.live.economy.economy.slot_generation(field_slot);
                let session = campaign.simulation_mut();
                if session.shared_spells().local_player.enabled_spell_holders[0] == 0 {
                    let mut carried = session.shared_spells().local_player.enabled_spell_holders.iter().enumerate()
                        .filter_map(|(family, slot)| (*slot != 0).then_some(family as u8))
                        .collect::<Vec<_>>();
                    carried.push(0);
                    session.apply_carried_spell_families(&carried, Some(0)).unwrap();
                }
                let fireball_holder = session.shared_spells().local_player.enabled_spell_holders[0];
                assert_ne!(fireball_holder, 0);
                let mut shooter = Position { x: field_position.x.wrapping_add(1_024), ..field_position };
                shooter.z = session.session.state.live.terrain.ground_height(shooter).saturating_add(256);
                session.set_level2_entity_position(player, shooter).unwrap();
                let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(shooter, field_position);
                session.set_level2_player_aim_for_test(yaw, pitch);
                {
                    let live = &mut session.session.state.live;
                    let mana = 20_000;
                    for row in &mut live.shared_spells.player_entities {
                        if row.index == player { row.set_mana(mana); row.set_mana_regen(0); }
                    }
                    live.shared_spells.player_entity.set_mana(mana);
                    live.shared_spells.player_entity.set_mana_regen(0);
                    live.shared_spells.caster.mana = mana;
                    live.shared_spells.player_mana = mana;
                    live.economy.entities[usize::from(player)].set_mana(mana);
                    if let Some(fire) = live.core.context.spell_fire.as_mut() { fire.caster.mana = mana; }
                }
                let mut captured = false;
                for turn in 0..80 {
                    campaign.step(if turn == 0 { PlayerCommand { opcode: 6, argument_1: 0, flags: 0x40, ..PlayerCommand::default() } } else { PlayerCommand::default() }, 0);
                    let session = campaign.simulation();
                    assert!(!session.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })), "Mine Fireball capture turn {turn}: {:?}", session.dispatch_events());
                    let mine = &session.entities()[usize::from(field_slot)];
                    assert!(mine.active && (mine.class, mine.model) == (10, 78), "actual Mine survives until Fireball capture");
                    captured = u16::from_le_bytes([mine.record[58], mine.record[59]]) == 0;
                    if captured {
                        assert_eq!(u16::from_le_bytes([mine.record[56], mine.record[57]]), 0);
                        break;
                    }
                }
                assert!(captured, "the actual public Fireball is captured by the naturally produced owned Mine");

                let rival = encounter_rival.expect("natural rival protection expired before casting");
                assert_ne!(campaign.simulation().entities()[usize::from(rival)].source_id, owner_id);
                let initial_life = campaign.simulation().entities()[usize::from(rival)].life;
                let rival_generation = campaign.simulation().session.state.live.economy.economy.slot_generation(rival);
                let mut selected = false;
                let mut emitted = false;
                let mut damaged = false;
                let mut rival_placed = false;
                let mut release_trace = Vec::new();
                for turn in 0..300 {
                    if !rival_placed {
                        let session = campaign.simulation();
                        let mine = &session.entities()[usize::from(field_slot)];
                        if mine.record[74] == 4 && mine.phase & 15 == 0 {
                            let rival_row = &session.entities()[usize::from(rival)];
                            let target_position = Position {
                                x: mine.position.x.wrapping_add(640), y: mine.position.y,
                                z: mine.position.z.wrapping_add(mine.array_yaw()).wrapping_sub(rival_row.array_yaw()),
                            };
                            campaign.simulation_mut().set_level2_entity_position(rival, target_position).unwrap();
                            rival_placed = true;
                        }
                    }
                    campaign.step(PlayerCommand::default(), 0);
                    let session = campaign.simulation();
                    assert!(!session.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })), "Mine release turn {turn}: {:?}", session.dispatch_events());
                    let mine = &session.entities()[usize::from(field_slot)];
                    selected |= mine.source_word_96() == rival;
                    emitted |= session.economy.fireball_projectiles().iter().any(|shot| shot.active && shot.owner_entity_index == owner_id && shot.target_source_entity_index == rival && shot.holder_entity_index == 0);
                    let target = &session.entities()[usize::from(rival)];
                    damaged |= selected && session.session.state.live.economy.economy.slot_generation(rival) == rival_generation
                        && target.life < initial_life && target.word_26() == owner_id;
                    if release_trace.len() < 40 {
                        let projectiles = session.economy.fireball_projectiles().iter()
                            .filter(|shot| shot.active && shot.owner_entity_index == owner_id && shot.holder_entity_index == 0)
                            .map(|shot| (shot.source_entity_index, shot.position, shot.target_source_entity_index,
                                shot.source_subspell_index, shot.yaw, shot.pitch)).collect::<Vec<_>>();
                        if !projectiles.is_empty() || turn % 16 == 0 {
                            release_trace.push((turn, mine.record[74], mine.position, mine.source_word_96(),
                                target.position, target.life, target.control_flags, target.word_26(), target.mana, projectiles));
                        }
                    }
                    if damaged { break; }
                }
                assert!(rival_placed, "the real rival is placed once at the next ordinary Mine target scan");
                assert!(selected, "charged Mine selects the real opposing wizard");
                assert!(emitted, "charged Mine emits its captured Fireball at that wizard");
                assert!(damaged, "the ordinary released Fireball damages the actual wizard with owner attribution: {release_trace:?}");

                let mut hidden = false;
                let mut reclaimed = false;
                for _ in 0..1_100 {
                    campaign.step(PlayerCommand::default(), 0);
                    let session = campaign.simulation();
                    assert!(!session.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })), "natural Mine expiry: {:?}", session.dispatch_events());
                    let arena = &session.session.state.live.economy;
                    let row = &arena.entities[usize::from(field_slot)];
                    if arena.economy.slot_generation(field_slot) != generation || !row.active {
                        reclaimed = true;
                        break;
                    }
                    hidden |= row.drawing_disabled();
                }
                assert!(hidden, "actual Mine naturally reaches its hide branch");
                assert!(reclaimed, "the following source boundary reclaims the hidden Mine");
            }
        }
    }
}
