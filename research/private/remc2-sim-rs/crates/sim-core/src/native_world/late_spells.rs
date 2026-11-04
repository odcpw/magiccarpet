//! Runtime adapters for late source spell families.
//!
//! Family22's class15/model22/action66 holder allocates six ordinary
//! class10/model57 entities. The entity pass then calls `step_sphere_slot`
//! after the common sphere movement/collision work. Hidden spheres remain
//! map-linked until the next source boundary; absorbed spheres are reclaimed
//! immediately, matching the distinct source tails.

use super::{EntitySnapshot, Level2SharedSpellResetSession, ParticleParameter, SessionTerrain};
use crate::campaign::source_arena::SourceArena;
use crate::level_definition::late_spells::FOOLS_MANA_PROPERTIES;
use crate::level1_mission::{source_map_search_slots, source_search_offsets};
use crate::spell_activation::{
    ProjectileSpellCastInput as CastInput, level2_applied_tier,
};
use crate::spells::late::fools_mana::{self as rules, ProjectileKind, Sphere as SphereState};
use crate::{FlightWorld, Position, move_entity};

const SPHERE_CLASS: u8 = 10;
const SPHERE_MODEL: u8 = 57;
const SPHERE_ACTION: u8 = 62;
const MANA_SPHERE_COLOR_BASE: u16 = 52;
const MANA_SPHERE_THRESHOLDS: [i32; 7] = [256, 512, 1_024, 2_048, 4_096, 9_192, 18_384];
const MANA_SPHERE_ROTATIONS: [i16; 8] = [13, 28, 42, 56, 70, 84, 98, 112];

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct HolderOutcome {
    /// Child addresses are live class10/model57 arena rows and participate in
    /// the current ascending source-slot pass when their addresses are ahead
    /// of its cursor.
    pub spheres: Vec<u16>,
    pub charged: bool,
    pub denied: bool,
    /// The caller runs the holder's existing same-pass expiry tail.
    pub cleanup: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SphereOutcome {
    pub projectile: Option<u16>,
    pub impact_effect: Option<u16>,
    pub clear_target_link: bool,
    pub hidden: bool,
    pub merged_into: Option<u16>,
    pub absorbed_sphere: Option<u16>,
}

fn put_word(row: &mut EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn word(row: &EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}

fn dword(row: &EntitySnapshot, offset: usize) -> i32 {
    i32::from_le_bytes(
        row.record[offset..offset + 4]
            .try_into()
            .expect("source dword"),
    )
}

fn put_dword(row: &mut EntitySnapshot, offset: usize, value: i32) {
    row.record[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn signed_word(row: &EntitySnapshot, offset: usize) -> i16 {
    i16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}

fn set_signed_word(row: &mut EntitySnapshot, offset: usize, value: i16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn set_owner(row: &mut EntitySnapshot, owner: u16) {
    row.source_id = owner;
    put_word(row, 30, owner);
}

fn live_tier(holder: &EntitySnapshot) -> Result<(u8, rules::Rule), String> {
    let tier = level2_applied_tier(holder.level_gate)
        .ok_or("Fool's Mana holder has a negative source tier")?;
    let data = *rules::SOURCE_RULES
        .get(usize::from(tier))
        .ok_or("Fool's Mana holder tier is outside owner SPELLS.DAT")?;
    Ok((tier, data))
}

/// `sub_50130` is the actual class10/model57 constructor selected by source
/// subtype `(10, 57)`. It advances the newborn entity RNG once, rolls mana,
/// links the neutral sphere visual, and initializes its four shape axes before
/// `sub_6C870` overwrites only the launch and tier fields.
fn construct_mana_sphere(
    arena: &SourceArena,
    slot: u16,
    particles: &[ParticleParameter],
    position: Position,
) -> Result<EntitySnapshot, String> {
    let seed = arena.economy.new_event_rng(slot);
    let mut sphere = crate::world::terrain::cave_features::new_event(slot, seed, position);
    sphere.class = SPHERE_CLASS;
    sphere.model = SPHERE_MODEL;
    sphere.record[67] = SPHERE_CLASS;
    sphere.record[68] = SPHERE_MODEL;
    sphere.record[69] = SPHERE_CLASS;
    sphere.record[70] = SPHERE_MODEL;
    sphere.set_source_action(SPHERE_ACTION);
    put_word(&mut sphere, 48, 128);

    let rng = crate::native_world::level2_next_rng16(seed);
    sphere.set_entity_rng(rng);
    sphere.set_actual_speed(0);
    sphere.record[60] = 3;
    sphere.record[61] = 128;
    sphere.record[62] = 0;
    sphere.record[71] = SPHERE_CLASS;
    sphere.record[72] = 1;
    // sub_50130 publishes this sphere with AddEventToMap_57D70, which sets
    // the source map-membership flag as well as linking the cell.
    sphere.set_control_flags(sphere.control_flags | 0x0002_0004);
    sphere.set_max_life(300);
    sphere.set_life(300);

    // `sub_50130` calls SetManaSphereColorAndRot before the holder supplies a
    // parent. A zero playerEntityIndex therefore selects neutral color 52.
    let mana = i32::from(rng % 0x07d0);
    sphere.set_mana(mana);
    let size = MANA_SPHERE_THRESHOLDS
        .iter()
        .take_while(|threshold| mana > **threshold)
        .count();
    let size = u16::try_from(size)
        .map_err(|_| "Fool's Mana sphere size exceeds the source visual table".to_owned())?;
    let rotation = *MANA_SPHERE_ROTATIONS
        .get(usize::from(size))
        .ok_or_else(|| "Fool's Mana sphere size exceeds the source visual table".to_owned())?;
    let particle_index = MANA_SPHERE_COLOR_BASE + size;
    let particle = *particles
        .get(usize::from(particle_index))
        .ok_or_else(|| format!("Fool's Mana requires retained particle{particle_index}"))?;
    crate::world::terrain::cave_features::bind_particle(&mut sphere, particle_index, particle)?;
    for offset in [86, 88, 90, 92] {
        set_signed_word(&mut sphere, offset, rotation);
    }
    Ok(sphere)
}

/// Authored10/57 selects sub_50130 directly, without the spell holder's
/// launch, parent or tier overrides. The caller owns sub_58DA0's reference
/// binding after a successful constructor and consumes the authored row even
/// when NewEvent returns null.
pub(crate) fn allocate_authored_sphere(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    position: Position,
) -> Result<Option<u16>, String> {
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let sphere = match construct_mana_sphere(arena, slot, particles, position) {
        Ok(sphere) => sphere,
        Err(error) => {
            crate::world::terrain::cave_features::reclaim(arena, terrain, slot);
            return Err(error);
        }
    };
    arena.economy.source_rng.insert(slot, sphere.entity_rng);
    arena.entities[usize::from(slot)] = sphere;
    if let Err(error) =
        crate::world::effects::sync_position(arena, terrain, slot, position, true, false)
    {
        crate::world::terrain::cave_features::reclaim(arena, terrain, slot);
        return Err(error);
    }
    Ok(Some(slot))
}

/// sub_6C870's holder callback, including all six ordinary NewEvent attempts.
/// Failed individual allocations do not cancel later attempts or refund the
/// source mana pulse. `cleanup` remains a signal to the shared holder tail.
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
        .filter(|row| row.active && (row.class, row.model, row.action) == (15, 22, 66))
        .cloned()
        .ok_or_else(|| format!("Fool's Mana holder {slot} is not live class15/model22/action66"))?;
    let (_, mut rule) = live_tier(&holder)?;
    rule.mana_cost = holder.max_mana;
    rule.castle_mana_required = holder.mana_regen;
    let parent_slot = holder.source_spell_parent_entity_index();
    let parent = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|_| parent_slot != 0)
        .cloned();

    // NewEvent uses the ordinary free stack; publish the holder row before it
    // can allocate or reuse any other fixed address.
    let holder_row = arena
        .entities
        .get_mut(usize::from(slot))
        .ok_or("Fool's Mana holder address is outside the source arena")?;
    *holder_row = holder.clone();

    let mut caster = parent.as_ref().map(|parent| rules::Caster {
        // sub_6C870 copies the live parent's id, which need not equal its
        // arena address after ownership or fixed-slot changes.
        entity_index: parent.source_id,
        position: parent.position,
        life: parent.life,
        mana: parent.mana,
        mana_regen: parent.mana_regen,
        castle_mana: input.castle_mana,
        speed: parent.actual_speed,
        yaw: parent.yaw,
        next_entity: input.aim_yaw as u16,
        entity_index2: input.aim_pitch as u16,
    });
    // Source word_0x30 is a signed 16-bit countdown, not a saturating or
    // fall-back conversion from the exported unsigned field.
    let duration = holder.activation_value as i16;
    let mut state = rules::Holder {
        active: holder.active_value,
        duration,
        cooldown: i16::from_le_bytes([holder.record[58], holder.record[59]]),
        rng: holder.entity_rng,
    };
    let visit = state.visit(rule, caster.as_mut(), arena.economy.free_slot_count());
    let mut outcome = HolderOutcome {
        charged: visit.mana_pulse,
        denied: visit.denied,
        cleanup: visit.cleanup,
        ..HolderOutcome::default()
    };

    if let (Some(parent), Some(caster)) = (parent.as_ref(), caster) {
        if visit.sphere_allocation_attempts != 0 {
            for _ in 0..visit.sphere_allocation_attempts {
                let Some(child_slot) = arena.allocate_source_slot_with_terrain(terrain) else {
                    continue;
                };
                let mut child =
                    match construct_mana_sphere(arena, child_slot, particles, caster.position) {
                        Ok(child) => child,
                        Err(error) => {
                            crate::world::terrain::cave_features::reclaim(
                                arena, terrain, child_slot,
                            );
                            return Err(error);
                        }
                    };
                // The source constructor runs before the holder consumes its
                // two launch draws for this successfully created child.
                let parameters = state.sphere_parameters(caster, rule, parent.pitch);
                child.set_actual_speed(parameters.speed);
                child.set_yaw(parameters.yaw);
                child.set_pitch(parameters.pitch);
                child.set_subspell_index(parameters.damage);
                child.level_gate = i8::try_from(parameters.tier).unwrap_or(i8::MAX);
                child.record[74] = parameters.tier;
                put_word(&mut child, 44, parameters.parent_entity);
                child.set_source_player_entity_index(parameters.player_entity);
                put_dword(&mut child, 20, 0);

                // sub_68E50 places a linked child at the active left/right
                // hand when present, falling back to the caster if terrain
                // blocks that offset.
                let mut position = parameters.position;
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
                        position = parameters.position;
                    }
                }
                child.set_position(position);
                arena
                    .economy
                    .source_rng
                    .insert(child_slot, child.entity_rng);
                arena.entities[usize::from(child_slot)] = child;
                if let Err(error) = crate::world::effects::sync_position(
                    arena, terrain, child_slot, position, true, true,
                ) {
                    crate::world::terrain::cave_features::reclaim(arena, terrain, child_slot);
                    return Err(error);
                }
                outcome.spheres.push(child_slot);
            }
            if let Some(last) = outcome.spheres.last().copied() {
                arena.economy.prepare_sound(last, last, -1, 11);
            }
        }
        arena.economy.source_rng.insert(slot, state.rng);
    }

    // sub_68DE0 follows all six constructor attempts. A later countdown
    // visit does not touch regeneration; a refused allocation still pays.
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
    holder.entity_rng = state.rng;
    holder.record[50..52].copy_from_slice(&state.active.to_le_bytes());
    holder.record[58..60].copy_from_slice(&state.cooldown.to_le_bytes());
    holder.set_entity_rng(state.rng);
    if visit.cleanup {
        let deferred = holder.source_word_2c();
        if deferred != 0 {
            let deferred_tier = deferred.checked_sub(1)
                .and_then(|value| u8::try_from(value.min(2)).ok())
                .ok_or("Fool's Mana deferred tier is negative")?;
            let profile = shared.physical_spell_profile(22, deferred_tier, parent_slot, None)
                .ok_or("Fool's Mana deferred tier has no source profile")?;
            holder.apply_nyphur_spell_profile(profile, deferred_tier);
            holder.set_source_word_2c(0);
        }
    }
    if let Some(current) = shared.pickup_mut(slot) {
        *current = holder.clone();
    }
    arena.entities[usize::from(slot)] = holder;
    Ok(outcome)
}

fn sphere_state(row: &EntitySnapshot) -> SphereState {
    SphereState {
        entity_index: row.index,
        parent_entity: word(row, 44),
        player_entity: row.source_player_entity_index(),
        target_entity: word(row, 108),
        target_aux: u32::from_le_bytes(row.record[104..108].try_into().expect("target dword")),
        tier: row.record[74],
        damage: row.subspell_index,
        mana: row.mana,
        retaliation_ticks: dword(row, 20),
        rng: row.entity_rng,
    }
}

fn write_sphere_state(row: &mut EntitySnapshot, state: SphereState) {
    put_word(row, 44, state.parent_entity);
    row.set_source_player_entity_index(state.player_entity);
    put_word(row, 108, state.target_entity);
    row.record[104..108].copy_from_slice(&state.target_aux.to_le_bytes());
    row.level_gate = i8::try_from(state.tier).unwrap_or(i8::MAX);
    row.record[74] = state.tier;
    row.set_subspell_index(state.damage);
    row.set_mana(state.mana);
    put_dword(row, 20, state.retaliation_ticks);
    row.set_entity_rng(state.rng);
}

fn construct_retaliation(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    sphere: &EntitySnapshot,
    target: &EntitySnapshot,
    kind: ProjectileKind,
) -> Result<Option<u16>, String> {
    let origin = Position {
        z: sphere.position.z.wrapping_add(sphere.array_fov()),
        ..sphere.position
    };
    let target_position = Position {
        z: if target.model == 2 {
            target.position.z
        } else {
            target.position.z.wrapping_add(target.array_yaw())
        },
        ..target.position
    };
    // Both source constructors call sub_655C0 before lifting the newborn
    // projectile by the sphere's fov, so the aim uses the unlifted birth.
    let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(sphere.position, target_position);
    match kind {
        ProjectileKind::RapidFire => {
            // sub_36770 creates the ordinary class9/model0 Fireball and binds
            // its existing target before the shared Fireball callback runs.
            // Keep that callback and its real class10/model0 impact consumer.
            let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
                return Ok(None);
            };
            let source_subspell_index = u16::try_from(sphere.subspell_index)
                .map_err(|_| "Fool's Mana sphere has an invalid damage payload".to_owned())?;
            let payload =
                crate::native_world::level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
                    origin: crate::native_world::FireballProjectileOrigin::PlayerSpell,
                    position: sphere.position,
                    yaw,
                    pitch,
                    axis_position: target_position,
                    // sub_36770 does not overwrite NewEvent's default id:
                    // the Fireball's own slot is its source identity.
                    source_id_entity_index: slot,
                    holder_entity_index: 0,
                    source_subspell_index,
                    source_mana: 50,
                    source_dword_0x10: 0,
                    target_source_entity_index: target.index,
                    target_class: target.class,
                    target_model: target.model,
                };
            let Some(projectile) = arena
                .economy
                .initialize_fireball_projectile(slot, payload, 0)
            else {
                return Ok(None);
            };
            crate::world::effects::sync_position(
                arena,
                terrain,
                projectile,
                sphere.position,
                true,
                false,
            )?;
            crate::world::effects::lighting::admit_source(
                arena,
                terrain,
                projectile,
                sphere.position,
                crate::world::effects::lighting::SourceLight::Fireball,
            );
            let row = arena
                .economy
                .fireball_projectiles
                .iter_mut()
                .find(|row| row.active && row.source_entity_index == projectile)
                .ok_or("RapidFire constructor lost its Fireball carrier")?;
            row.position = origin;
            // Source sub_36770 plays sound9 only when the ordinary constructor
            // succeeds; this stays attached to the real projectile slot.
            arena.economy.prepare_sound(projectile, projectile, -1, 9);
            if terrain.material_mask(origin) == 1 {
                let splash = crate::world::effects::allocate_from_source_position(
                    arena,
                    terrain,
                    sim_definition::adapter::mc2_effect_definition(crate::level_definition::EffectKind::Ash),
                    sphere.index,
                    sphere.position,
                    particles,
                )?;
                if let Some(splash) = splash {
                    arena.economy.prepare_sound(splash, splash, -1, 27);
                }
            }
            Ok(Some(projectile))
        }
        ProjectileKind::LightningFire => {
            // sub_36850 uses the normal source Ray constructor (particle216,
            // class9/model9 lifetime and speed) then changes only its target,
            // output selector and Type_str relation to family22's variant.
            let Some(slot) = crate::native_world::lightning::construct(
                arena,
                terrain,
                particles,
                crate::native_world::lightning::Product::Ray,
                sphere.position,
            )?
            else {
                return Ok(None);
            };
            let projectile = &mut arena.entities[usize::from(slot)];
            projectile.set_position(origin);
            projectile.record[69] = target.class;
            projectile.record[70] = target.model;
            projectile.record[71] = 10;
            projectile.record[72] = 23;
            projectile.set_source_word_96(target.index);
            projectile.set_subspell_index(sphere.subspell_index);
            projectile.set_source_roll(yaw);
            projectile.set_fov(pitch);
            // sub_655C0 stores target angles in roll/fov, then sub_36850
            // copies them into yaw/pitch before the first callback.
            projectile.set_yaw(yaw);
            projectile.set_pitch(pitch);
            // Unlike RapidFire's projectile constructor, this NewEvent copies
            // the sphere id and uses descriptor65 (relation64).
            set_owner(projectile, sphere.index);
            projectile.parameter_relation = Some(64);
            projectile.record[164..168].copy_from_slice(&65_u32.to_le_bytes());
            Ok(Some(slot))
        }
    }
}

fn construct_consumed_sphere_effect(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    sphere: &EntitySnapshot,
) -> Result<Option<u16>, String> {
    crate::world::effects::allocate(
        arena,
        terrain,
        sim_definition::adapter::mc2_effect_definition(crate::level_definition::EffectKind::Impact),
        sphere.position,
        particles,
    )
}

fn source_spheres_overlap(left: &EntitySnapshot, right: &EntitySnapshot) -> bool {
    let dx = i32::from(right.position.x.wrapping_sub(left.position.x) as i16).abs();
    let dy = i32::from(right.position.y.wrapping_sub(left.position.y) as i16).abs();
    let dz = (i32::from(left.position.z) + i32::from(left.array_yaw())
        - i32::from(right.position.z)
        - i32::from(right.array_yaw()))
    .abs();
    dx < i32::from(left.array_pitch()) + i32::from(right.array_pitch())
        && dy < i32::from(left.array_roll()) + i32::from(right.array_roll())
        && dz < i32::from(left.array_fov()) + i32::from(right.array_fov())
}

/// Source sub_10A50 scans the SEARCH.DAT map neighborhood in chain order,
/// then sub_106C0 applies strict wrapped source extents. A flat arena scan or
/// radial-distance proxy changes which sphere is consumed when cells overlap.
fn merge_candidate(arena: &SourceArena, sphere: &EntitySnapshot) -> Option<u16> {
    let extent = i32::from(sphere.array_pitch()) + 255;
    // Pinned my_sign32 is a sign mask (-1/0), not Rust's -1/0/+1 signum.
    let search_class = ((extent - (extent >> 31) * 255) >> 8).clamp(0, 31) as u8;
    for candidate_slot in source_map_search_slots(
        &arena.economy.map_chains,
        sphere.position,
        &source_search_offsets(search_class),
    ) {
        let Some(candidate) = arena.entities.get(usize::from(candidate_slot)) else {
            continue;
        };
        if candidate.index == sphere.index
            || !candidate.active
            || (candidate.class, candidate.model) != (10, 57)
            || candidate.source_id == sphere.source_id
            || !source_spheres_overlap(sphere, candidate)
        {
            continue;
        }
        return Some(candidate_slot);
    }
    None
}

/// The source class10/model57 callback carries its own flight arc and map
/// membership; it is not a generic guided projectile. Return true only when
/// the grounded drift tail reaches a merge scan.
fn move_sphere(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    context: super::source_target::Context<'_>,
    row: &mut EntitySnapshot,
) -> Result<bool, String> {
    let mut position = row.position;
    let mut gravity = signed_word(row, 48);
    let mut grounded_drift = false;
    let attraction = word(row, 126);
    if attraction != 0 {
        let target = super::source_target::read(arena, context, attraction)?
            .map_or(Position::default(), |view| view.position);
        row.set_actual_speed(0);
        let yaw = crate::heading_for_positions(position, target) as i16;
        row.set_yaw(yaw);
        let mut velocity = Position::default();
        move_entity(&mut velocity, yaw, 0, signed_word(row, 122));
        put_word(row, 158, velocity.x);
        put_word(row, 160, velocity.y);
        put_word(row, 126, 0);
    }
    if row.control_flags & 0x40 != 0 {
        row.set_actual_speed(0);
        let target_slot = row.source_word_96();
        let target = super::source_target::read(arena, context, target_slot)?;
        let lift = match target.map(|target| (target.class, target.model)) {
            Some((3, 3)) => Some(32_i16),
            Some((5, 23)) => Some(signed_word(&arena.entities[usize::from(target_slot)], 48)),
            _ => None,
        };
        if let (Some(target), Some(lift)) = (target, lift) {
            gravity = 128;
            let yaw = crate::heading_for_positions(position, target.position) as i16;
            row.set_yaw(yaw);
            let distance = super::level2_wrapping_target_distance(position, target.position);
            if distance <= 1024 {
                if distance >= 16 {
                    move_entity(&mut position, yaw, 0, 16);
                } else {
                    position.x = target.position.x;
                    position.y = target.position.y;
                    if position.z >= target.position.z {
                        if i32::from(position.z) > i32::from(target.position.z) + 512 {
                            position.z = position.z.wrapping_sub(lift);
                        }
                    } else {
                        position.z = position.z.wrapping_add(lift);
                    }
                }
                position.z = position.z.max(terrain.ground_height(position));
            } else {
                row.set_control_flags(row.control_flags & !0x40);
            }
        } else {
            row.set_control_flags(row.control_flags & !0x40);
        }
        set_signed_word(row, 48, gravity);
        row.set_position(position);
        arena.entities[usize::from(row.index)] = row.clone();
        crate::world::effects::sync_position(arena, terrain, row.index, position, true, false)?;
        *row = arena.entities[usize::from(row.index)].clone();
        return Ok(false);
    }
    if row.actual_speed != 0 {
        let speed = row.actual_speed;
        if speed < -4 {
            row.set_actual_speed(speed.wrapping_add(4));
        } else if speed > 4 {
            row.set_actual_speed(speed.wrapping_sub(4));
        }
        move_entity(&mut position, row.yaw, row.pitch, row.actual_speed);
        gravity = gravity.wrapping_sub(16).max(-128);
        position.z = position.z.wrapping_add(gravity);
        if terrain.ceiling_height(position).is_some_and(|ceiling| {
            i32::from(row.array_fov()) + i32::from(terrain.ground_height(position)) > i32::from(ceiling)
        }) {
            position = row.position;
            row.set_actual_speed(0);
            gravity = -128;
        } else {
            row.set_position(position);
        }
        let ground = terrain.ground_height(position);
        if ground > position.z {
            position.z = ground;
            row.set_actual_speed(0);
            if terrain.material_mask(position) == 1 {
                gravity = 0;
                set_signed_word(row, 48, gravity);
                row.set_position(position);
                arena.entities[usize::from(row.index)] = row.clone();
                crate::world::effects::sync_position(arena, terrain, row.index, position, true, false)?;
                if let Some(splash) = crate::world::effects::allocate_from_source_position(
                    arena, terrain,
                    sim_definition::adapter::mc2_effect_definition(crate::level_definition::EffectKind::Ash),
                    row.index, position, particles,
                )? {
                    arena.economy.prepare_sound(splash, splash, -1, 27);
                }
                *row = arena.entities[usize::from(row.index)].clone();
                return Ok(false);
            } else {
                gravity = 128;
            }
        } else if let Some(ceiling) = terrain.ceiling_height(position) {
            let cap = ceiling.wrapping_sub(row.array_fov());
            if cap < position.z {
                row.set_entity_rng(crate::native_world::level2_next_rng16(row.entity_rng));
                let speed = (row.entity_rng & 3) as i16 - 2;
                row.set_actual_speed(if speed == 0 { 1 } else { speed });
                gravity = gravity.wrapping_abs().wrapping_neg();
                // Source adjusts only predictedAxis here, after CopyEntityPosition;
                // the current sphere position retains the earlier copied Z.
                position = row.position;
            }
        }
    } else if row.record[61] != 0 || attraction != 0 {
        let axis_x = signed_word(row, 158).clamp(-64, 64);
        let axis_y = signed_word(row, 160).clamp(-64, 64);
        set_signed_word(row, 158, axis_x);
        set_signed_word(row, 160, axis_y);
        position.x = position.x.wrapping_add(axis_x as u16);
        position.y = position.y.wrapping_add(axis_y as u16);
        position.z = position.z.wrapping_add(gravity);
        gravity = gravity.wrapping_sub(16).max(-128);

        if terrain.ceiling_height(position).is_some_and(|ceiling| {
            i32::from(row.array_fov()) + i32::from(terrain.ground_height(position)) > i32::from(ceiling)
        }) {
            if attraction != 0 {
                position = row.position;
            } else {
                set_signed_word(row, 158, 64);
                set_signed_word(row, 160, 64);
                position.x = row.position.x.wrapping_add(64);
                position.y = row.position.y.wrapping_add(64);
            }
            gravity = -128;
        }

        let ground = terrain.ground_height(position);
        if ground > position.z {
            position.z = ground;
            let sign = i32::from(gravity) >> 31;
            let bounce = -((i32::from(gravity) - sign * 4 + sign) >> 2);
            gravity = if bounce <= 16 { 0 } else { bounce as i16 };
        }
        let mut contact = ground;
        if let Some(ceiling) = terrain.ceiling_height(position) {
            contact = ceiling.wrapping_sub(row.array_fov());
            if contact < position.z {
                gravity = gravity.wrapping_abs().wrapping_neg();
                position.z = contact;
            }
        }
        grounded_drift = contact == position.z;
    }

    set_signed_word(row, 48, gravity);
    row.set_position(position);
    arena.entities[usize::from(row.index)] = row.clone();
    crate::world::effects::sync_position(arena, terrain, row.index, position, true, false)?;
    *row = arena.entities[usize::from(row.index)].clone();
    Ok(grounded_drift)
}

/// sub_58030's four raw height samples and source 250/256 damping. This
/// follows the merge/recolor boundary, so the current sphere carries both.
fn apply_grounded_slope(row: &mut EntitySnapshot, terrain: &SessionTerrain) {
    let x = (row.position.x >> 8) as u8;
    let y = (row.position.y >> 8) as u8;
    let heights = &terrain.planes().ground_heights;
    let height = |x: u8, y: u8| i32::from(heights[usize::from(x) + 256 * usize::from(y)]);
    let a = height(x, y);
    let b = height(x.wrapping_add(1), y);
    let c = height(x.wrapping_add(1), y.wrapping_add(1));
    let d = height(x, y.wrapping_add(1));
    let dx = (a - b - c + d) as i16;
    let dy = (a + b - c - d) as i16;
    let vx = signed_word(row, 158).wrapping_add(dx);
    let vy = signed_word(row, 160).wrapping_add(dy);
    set_signed_word(row, 158, (i32::from(vx) * 250 / 256) as i16);
    set_signed_word(row, 160, (i32::from(vy) * 250 / 256) as i16);
}

fn update_merged_sphere_color(
    shared: &Level2SharedSpellResetSession,
    arena: &SourceArena,
    row: &mut EntitySnapshot,
    particles: &[ParticleParameter],
    context: super::source_target::Context<'_>,
) -> Result<(), String> {
    const COLORS: [u16; 8] = [0, 1, 4, 3, 2, 5, 7, 6];
    let owner = row.source_player_entity_index();
    let owner_row = super::source_target::status_parent(arena, context, owner)?;
    let base = match owner_row.as_ref().filter(|parent| owner != 0 && parent.class == 3) {
        None => MANA_SPHERE_COLOR_BASE,
        Some(parent) => {
            let color = super::source_target::status_player_color(shared, parent)?
                .ok_or_else(|| format!("Fool's Mana owner {owner} has no current A4 color"))?;
            let color = *COLORS.get(usize::from(color))
                .ok_or("Fool's Mana owner color is outside the source table")?;
            105 + 8 * color
        }
    };
    let size = MANA_SPHERE_THRESHOLDS.iter().take_while(|limit| row.mana > **limit).count();
    let index = base + size as u16;
    if row.particle_parameter_index != Some(index) {
        let particle = *particles.get(usize::from(index))
            .ok_or_else(|| format!("Fool's Mana merge needs retained particle{index}"))?;
        crate::world::terrain::cave_features::bind_particle(row, index, particle)?;
        let rotation = MANA_SPHERE_ROTATIONS[size];
        for offset in [86, 88, 90, 92] {
            set_signed_word(row, offset, rotation);
        }
    }
    Ok(())
}

/// sub_35FB0's class10/model57 scheduled callback, including mailbox-driven
/// retaliation, source sphere flight, and the ground-contact
/// sub_36F30 immediate merge/reclaim.
pub(crate) fn step_sphere_slot(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    _context: super::source_target::Context<'_>,
) -> Result<SphereOutcome, String> {
    let Some(mut row) = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && (row.class, row.model) == (10, 57))
        .cloned()
    else {
        return Err(format!(
            "Fool's Mana sphere {slot} is not live class10/model57"
        ));
    };
    // sub_35FB0 consumes this Wind flag before either the Possession mailbox
    // or movement; the shared scheduler still runs its ordinary phase tail.
    if row.control_flags & 0x800 != 0 {
        row.set_control_flags(row.control_flags & !0x800);
        arena.entities[usize::from(slot)] = row;
        return Ok(SphereOutcome::default());
    }
    // sub_35FB0 consumes an existing word68 mailbox. Possession's sub_112D0
    // writes it; nearby spheres do not themselves create a retaliation link.
    let mut state = sphere_state(&row);
    let visit = state.visit_retaliation();
    let mut outcome = SphereOutcome {
        clear_target_link: visit.clear_target_link,
        ..SphereOutcome::default()
    };

    if let (Some(kind), Some(target_slot)) = (visit.projectile, visit.target_entity)
        && let Some(target) = arena.entities.get(usize::from(target_slot)).cloned()
        && target.active
    {
        outcome.projectile = construct_retaliation(arena, terrain, particles, &row, &target, kind)?;
    }
    if visit.award_spell_experience
        && let Some(parent) = arena.entities.get(usize::from(state.parent_entity))
        && parent.active
        && parent.class == 3
        && parent.model == 0
    {
        shared.award_spell_experience(state.parent_entity, 22, 1);
    }
    write_sphere_state(&mut row, state);
    if visit.consume_sphere {
        outcome.impact_effect = construct_consumed_sphere_effect(arena, terrain, particles, &row)?;
        row.disable_drawing();
        outcome.hidden = true;
    }

    // The sphere callback owns its ordinary source flight before the merge
    // scan. The absorbed row is unlinked and returned to the free stack in
    // this callback; consumed spheres remain hidden for next-boundary reclaim.
    let grounded_drift = if visit.consume_sphere {
        false
    } else {
        move_sphere(arena, terrain, particles, _context, &mut row)?
    };
    if grounded_drift && let Some(absorbed_slot) = merge_candidate(arena, &row) {
        let absorbed = arena.entities[usize::from(absorbed_slot)].clone();
        state = sphere_state(&row);
        let merged = state.merge(sphere_state(&absorbed));
        write_sphere_state(&mut row, state);
        outcome.merged_into = Some(slot);
        outcome.absorbed_sphere = Some(merged.absorbed_entity);
        crate::world::terrain::cave_features::reclaim(arena, terrain, absorbed_slot);
        row.set_map_links(
            arena.economy.map_chains.old_map_entity(slot),
            arena.economy.map_chains.next_entity(slot),
        );
        update_merged_sphere_color(shared, arena, &mut row, particles, _context)?;
    }
    if grounded_drift {
        apply_grounded_slope(&mut row, terrain);
    }

    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    Ok(outcome)
}

/// Minimal scheduler discriminator. Shared roots call the family-specific
/// adapters above at their normal class/model dispatch points.
pub(crate) const HOLDER_SELECTOR: (u8, u8, u8) = (15, 22, 66);
pub(crate) const SPHERE_SELECTOR: (u8, u8, u8) = (SPHERE_CLASS, SPHERE_MODEL, SPHERE_ACTION);
pub(crate) const FOOLS_MANA_FAMILY: u8 = FOOLS_MANA_PROPERTIES.family;

#[cfg(test)]
mod possession_tests {
    use super::*;

    #[test]
    fn grounded_sphere_merge_finds_the_neighbor_before_the_rounded_search_origin() {
        let session = super::super::tests::level_two_campaign_session();
        let live = &session.session.state.live;
        let mut arena = live.economy.clone();
        let mut terrain = live.terrain.clone();
        let mut shared = live.shared_spells.clone();
        let particles = &session.session.definition.presentation.particles;
        let origin = (0..65_536_u32).map(|tile| Position {
            x: ((tile & 255) << 8) as u16,
            y: ((tile >> 8) << 8) as u16,
            z: 0,
        }).find(|point| source_map_search_slots(&arena.economy.map_chains, *point,
            &source_search_offsets(2)).is_empty() && terrain.material_mask(*point) != 1)
            .expect("owner terrain has an empty dry source neighborhood");
        let mut position = Position { x: origin.x.wrapping_add(128), ..origin };
        position.z = terrain.ground_height(position);
        let mut slots = Vec::new();
        for position in [position, Position { x: position.x.wrapping_sub(1), ..position }] {
            let slot = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
            let mut sphere = construct_mana_sphere(&arena, slot, particles, position).unwrap();
            put_word(&mut sphere, 44, session.state.entity_index);
            sphere.set_actual_speed(0);
            set_signed_word(&mut sphere, 48, 0);
            arena.entities[usize::from(slot)] = sphere;
            crate::world::effects::sync_position(&mut arena, &mut terrain, slot, position, true, false).unwrap();
            slots.push(slot);
        }
        let first = arena.entities[usize::from(slots[0])].clone();
        let second = &arena.entities[usize::from(slots[1])];
        assert!(source_spheres_overlap(&first, second));
        // Both occupy the same physical tile, but the rounded search origin
        // is the following tile. SEARCH class1 supplies its real (-1,0) cell.
        assert_eq!(first.position.x >> 8, second.position.x >> 8);
        assert_eq!((first.position.x.wrapping_add(128) >> 8), ((first.position.x >> 8) + 1) & 255);
        assert_eq!(merge_candidate(&arena, &first), Some(slots[1]));
        let mana = first.mana.wrapping_add(second.mana);
        let outcome = step_sphere_slot(&mut shared, &mut arena, &mut terrain, particles,
            slots[0], super::super::source_target::Context::default()).unwrap();
        assert_eq!((outcome.merged_into, outcome.absorbed_sphere), (Some(slots[0]), Some(slots[1])));
        assert_eq!(arena.entities[usize::from(slots[0])].mana, mana);
        assert!(arena.economy.free_source_slots().contains(&slots[1]));
    }

    #[test]
    fn grounded_fools_mana_merges_and_updates_color_before_source_slope() {
        let session = super::super::tests::level_two_campaign_session();
        let live = &session.session.state.live;
        let mut arena = live.economy.clone();
        let mut terrain = live.terrain.clone();
        let mut shared = live.shared_spells.clone();
        let particles = &session.session.definition.presentation.particles;
        let position = (0..65_536_u32).map(|tile| Position {
            x: ((tile & 255) << 8) as u16,
            y: ((tile >> 8) << 8) as u16,
            z: 0,
        }).find(|point| arena.economy.map_chains.entity_chain((point.x >> 8) as u8, (point.y >> 8) as u8).is_empty()
            && terrain.material_mask(*point) != 1).expect("owner terrain has an empty dry source cell");
        let position = Position { z: terrain.ground_height(position), ..position };
        let mut slots = Vec::new();
        for _ in 0..2 {
            let slot = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
            let mut sphere = construct_mana_sphere(&arena, slot, particles, position).unwrap();
            put_word(&mut sphere, 44, session.state.entity_index);
            sphere.set_actual_speed(0);
            set_signed_word(&mut sphere, 48, 0);
            arena.entities[usize::from(slot)] = sphere;
            crate::world::effects::sync_position(&mut arena, &mut terrain, slot, position, true, false).unwrap();
            slots.push(slot);
        }
        let first = arena.entities[usize::from(slots[0])].clone();
        let second = arena.entities[usize::from(slots[1])].clone();
        assert_eq!(first.position, position);
        assert_eq!(second.position, position);
        assert_ne!(first.control_flags & 4, 0, "AddEventToMap marks the source sphere");
        assert_eq!(merge_candidate(&arena, &first), Some(slots[1]),
            "both constructed spheres must overlap in the source map chain: first={first:?}, second={second:?}, tile={:?}, chain={:?}",
            arena.economy.map_chains.tile_of(slots[0]),
            source_map_search_slots(&arena.economy.map_chains, position, &source_search_offsets(0)));
        let x = (position.x >> 8) as u8;
        let y = (position.y >> 8) as u8;
        let heights = &terrain.planes().ground_heights;
        let h = |x: u8, y: u8| i32::from(heights[usize::from(x) + 256 * usize::from(y)]);
        let (a,b,c,d) = (h(x,y), h(x.wrapping_add(1),y), h(x.wrapping_add(1),y.wrapping_add(1)), h(x,y.wrapping_add(1)));
        let expected_velocity = (((a-b-c+d)*250/256) as i16, ((a+b-c-d)*250/256) as i16);
        let expected_mana = first.mana.wrapping_add(second.mana);
        let expected_size = MANA_SPHERE_THRESHOLDS.iter().take_while(|limit| expected_mana > **limit).count();
        let outcome = step_sphere_slot(&mut shared, &mut arena, &mut terrain, particles, slots[0],
            super::super::source_target::Context::default()).unwrap();
        assert_eq!((outcome.merged_into, outcome.absorbed_sphere), (Some(slots[0]), Some(slots[1])));
        assert!(arena.economy.free_source_slots().contains(&slots[1]), "source immediately reclaims the absorbed sphere");
        let row = &arena.entities[usize::from(slots[0])];
        assert_eq!(row.mana, expected_mana);
        assert_eq!(row.particle_parameter_index, Some(MANA_SPHERE_COLOR_BASE + expected_size as u16));
        assert_eq!((signed_word(row, 158), signed_word(row, 160)), expected_velocity);
        assert_eq!(row.phase, first.phase, "the scheduler owns the common phase tail");
    }

    #[test]
    fn flying_fools_mana_water_contact_publishes_real_splash_and_sound() {
        let session = super::super::tests::level_two_campaign_session();
        let live = &session.session.state.live;
        let mut arena = live.economy.clone();
        let mut terrain = live.terrain.clone();
        let mut shared = live.shared_spells.clone();
        let particles = &session.session.definition.presentation.particles;
        let point = (0..65_536_u32).map(|tile| Position {
            x: (((tile & 255) << 8) | 128) as u16,
            y: (((tile >> 8) << 8) | 128) as u16,
            z: 0,
        }).find(|point| terrain.material_mask(*point) == 1)
            .expect("retained owner terrain has a water cell");
        let position = Position { z: terrain.ground_height(point).wrapping_add(32), ..point };
        let slot = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
        let mut sphere = construct_mana_sphere(&arena, slot, particles, position).unwrap();
        sphere.set_actual_speed(1);
        set_signed_word(&mut sphere, 48, -128);
        arena.entities[usize::from(slot)] = sphere;
        crate::world::effects::sync_position(&mut arena, &mut terrain, slot, position, true, false).unwrap();
        step_sphere_slot(&mut shared, &mut arena, &mut terrain, particles, slot,
            super::super::source_target::Context::default()).unwrap();
        let row = &arena.entities[usize::from(slot)];
        assert_eq!(row.position.z, terrain.ground_height(row.position));
        assert_eq!((row.actual_speed, signed_word(row, 48)), (0, 0));
        assert!(arena.economy.events().iter().any(|event| matches!(event,
            super::super::Level2EconomyEvent::PreparedSound { source_entity_index, entity_index, sound_id: 27, .. }
                if source_entity_index == entity_index && arena.entities[usize::from(*entity_index)].class == 10
                    && arena.entities[usize::from(*entity_index)].model == 5)),
            "water contact creates its real 10/5 splash and attaches sound27 to it");
    }

    #[test]
    fn flying_fools_mana_cave_barrier_uses_source_fallback() {
        let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/ribbon-20261104-integration/current-owner-package/level24");
        let session = crate::campaign::CampaignSession::open(
            package, crate::campaign::spec::CampaignSelection::ordinary(24).unwrap(),
        ).expect("unfiltered owner Cave24 opens");
        let simulation = session.simulation();
        let live = &simulation.session.state.live;
        let mut arena = live.economy.clone();
        let mut terrain = live.terrain.clone();
        let particles = &simulation.session.definition.presentation.particles;
        let slot = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
        let mut sphere = construct_mana_sphere(&arena, slot, particles, Position::default()).unwrap();
        let fov = sphere.array_fov();
        let (position, yaw) = (0..65_536_u32).flat_map(|tile| [0_i16, 512, 1024, 1536].into_iter().map(move |yaw| (tile, yaw)))
            .find_map(|(tile, yaw)| {
                let mut origin = Position { x: (((tile & 255) << 8) | 128) as u16,
                    y: (((tile >> 8) << 8) | 128) as u16, z: 0 };
                let ground = terrain.ground_height(origin);
                let ceiling = terrain.ceiling_height(origin)?;
                origin.z = ground.wrapping_add(1);
                if i32::from(ceiling) - i32::from(fov) < i32::from(origin.z) { return None; }
                let mut next = origin;
                move_entity(&mut next, yaw, 0, 252);
                let next_ceiling = terrain.ceiling_height(next)?;
                (i32::from(fov) + i32::from(terrain.ground_height(next)) > i32::from(next_ceiling))
                    .then_some((origin, yaw))
            }).expect("owner Cave24 has an adjacent blocked cell");
        sphere.set_position(position);
        sphere.set_yaw(yaw);
        sphere.set_actual_speed(256);
        set_signed_word(&mut sphere, 48, 0);
        let rng = sphere.entity_rng;
        arena.entities[usize::from(slot)] = sphere.clone();
        crate::world::effects::sync_position(&mut arena, &mut terrain, slot, position, true, false).unwrap();
        assert!(!move_sphere(&mut arena, &mut terrain, particles,
            super::super::source_target::Context::default(), &mut sphere).unwrap());
        assert_eq!(sphere.position, position);
        assert_eq!((sphere.actual_speed, signed_word(&sphere, 48), sphere.entity_rng), (0, -128, rng));
    }

    #[test]
    fn adjacent_spheres_do_not_invent_possession_mailboxes() {
        let session = super::super::tests::level_two_campaign_session();
        let live = &session.session.state.live;
        let mut arena = live.economy.clone();
        let mut terrain = live.terrain.clone();
        let mut shared = live.shared_spells.clone();
        let particles = &session.session.definition.presentation.particles;
        let position = Position { x: 30_000, y: 30_000, z: 8_192 };
        let mut slots = Vec::new();
        for _ in 0..2 {
            let slot = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
            let mut sphere = construct_mana_sphere(&arena, slot, particles, position).unwrap();
            put_word(&mut sphere, 44, session.state.entity_index);
            arena.entities[usize::from(slot)] = sphere;
            crate::world::effects::sync_position(&mut arena, &mut terrain, slot, position, true, false).unwrap();
            slots.push(slot);
        }
        let outcome = step_sphere_slot(&mut shared, &mut arena, &mut terrain, particles, slots[0],
            super::super::source_target::Context::default()).unwrap();
        assert_eq!(outcome.projectile, None);
        assert!(!outcome.hidden);
        assert_eq!(word(&arena.entities[usize::from(slots[0])], 108), 0);
        assert!(arena.entities[usize::from(slots[1])].active);
    }

    #[test]
    fn authored_fools_mana_gate_acquisition_and_public_tiers_produce_six_spheres() {
        use crate::PlayerCommand;
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        use crate::native_world::{Mc2DispatchEvent, LEVEL2_SPELL_TIER_THRESHOLDS};
        use crate::native_world::simulation::CarriedSpellProgress;

        let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level21");
        let mut acquired = CampaignSession::open(package, CampaignSelection::ordinary(21).unwrap())
            .expect("actual Level 21 owner package opens");
        let player = acquired.simulation().state.entity_index;
        assert_eq!(acquired.simulation().shared_spells().local_player.enabled_spell_holders[22], 0);
        // These are the actual group-zero and generated switches. Move the
        // real player to each entry or outside each exit boundary; the source
        // scheduler, not a direct group call, produces every next switch.
        for (authored, expected_group, exit) in [
            (367_u16, 14_u16, false), (387, 15, false), (389, 16, true),
            (395, 17, false), (397, 18, true), (426, 19, false),
        ] {
            let (slot, mut position) = if authored == 367 {
                let root = &acquired.simulation().entities()[97];
                assert!(root.active);
                assert_eq!((root.class, root.model, root.action, root.source_id), (11, 0, 0, expected_group));
                (root.index, root.position)
            } else {
                let switch = acquired.simulation().economy().authored_switch_producers().iter()
                    .find(|switch| switch.active && switch.authored_index == authored)
                    .unwrap_or_else(|| panic!("preceding source group creates switch row{authored}"));
                (switch.source_entity_index, switch.position)
            };
            if exit { position.x = position.x.wrapping_add(4_096); }
            acquired.simulation_mut().set_level2_entity_position(player, position).unwrap();
            let mut triggered = false;
            for _ in 0..10 {
                acquired.step(PlayerCommand::default(), 0);
                assert!(!acquired.simulation().dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })), "Level21 switch row{authored}: {:?}", acquired.simulation().dispatch_events());
                triggered |= acquired.simulation().dispatch_events().iter().any(|event| matches!(event,
                    Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group, .. }
                        if *source_slot == slot && *group == expected_group));
                if triggered { break; }
            }
            assert!(triggered, "ordinary source switch row{authored} fires group{expected_group}");
        }
        let pickup = *acquired.simulation().economy().spell_pickups().iter()
            .find(|pickup| pickup.authored_index == 427 && pickup.family == 22)
            .expect("group19 materializes the real Fool's Mana pickup");
        assert_eq!(pickup.action, 67);
        acquired.simulation_mut().set_level2_entity_position(player, pickup.position).unwrap();
        let mut receipt = None;
        for _ in 0..8 {
            acquired.step(PlayerCommand::default(), 0);
            receipt = acquired.simulation().dispatch_events().iter().find_map(|event| match event {
                Mc2DispatchEvent::SpellAcquisition { source_slot, family: 22, result: Some(outcome), .. }
                    if *source_slot == pickup.source_entity_index => Some(*outcome),
                _ => None,
            });
            if receipt.is_some() { break; }
        }
        let receipt = receipt.expect("the ordinary pickup callback acquires Fool's Mana");
        assert_eq!((receipt.source_slot, receipt.family, receipt.holder_action, receipt.parent_entity_index),
            (pickup.source_entity_index, 22, 66, player));

        for (tier, profile) in FOOLS_MANA_PROPERTIES.tiers.into_iter().enumerate() {
            let mut campaign = acquired.clone();
            let session = campaign.simulation_mut();
            session.apply_carried_spell_progression(&[CarriedSpellProgress {
                family: 22,
                persistent_experience: LEVEL2_SPELL_TIER_THRESHOLDS[22][tier],
                runtime_experience: 0,
                unlocked_tier: tier as i8,
                selected_tier: tier as i8,
            }]).unwrap();
            let holder = session.shared_spells().local_player.enabled_spell_holders[22];
            assert_eq!(holder, pickup.source_entity_index);
            assert_eq!(session.shared_spells().pickup(holder).unwrap().level_gate, tier as i8);
            let position = session.state.position;
            let castle = session.economy_mut().build_castle(position, player).unwrap();
            assert!(session.economy_mut().set_castle_mana_prerequisite_for_test(castle, profile.runtime_castle_mana_limit));
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
            let existing = live.economy.entities.iter().filter(|row| row.active && (row.class,row.model)==(10,57)).map(|row| row.index).collect::<Vec<_>>();
            let start = session.effect_journal().records().len();
            let mut spheres = Vec::new();
            for turn in 0..4 {
                campaign.step(if turn == 0 { PlayerCommand { opcode: 6, argument_1: 22, flags: 0x40, ..PlayerCommand::default() } } else { PlayerCommand::default() }, 0);
                let session = campaign.simulation();
                assert!(!session.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })), "Fool's Mana tier {tier}: {:?}", session.dispatch_events());
                spheres = session.entities().iter().filter(|row| row.active && (row.class,row.model,row.action)==(10,57,62)
                    && word(row, 44) == owner_id && !existing.contains(&row.index)).map(|row| row.index).collect();
                if !spheres.is_empty() { break; }
            }
            assert_eq!(spheres.len(), 6, "real acquired tier {tier} produces six spheres");
            let session = campaign.simulation();
            for slot in &spheres {
                let row = &session.entities()[usize::from(*slot)];
                assert_eq!(row.record[74], profile.life);
                assert_eq!(row.subspell_index, profile.subspell_index as u16);
                assert_eq!(row.source_player_entity_index(), 0);
                let sprite = session.campaign_entity_presentations(0).unwrap().into_iter().find(|sprite| sprite.entity_index == *slot)
                    .expect("actual source sphere reaches the sprite consumer");
                assert_eq!(Some(sprite.render_parameter_index), row.particle_parameter_index);
                assert_eq!(sprite.base_tmap, session.session.definition.presentation.particles[usize::from(sprite.render_parameter_index)].base_tmap);
            }
            assert!(session.effect_journal().records()[start..].iter().any(|record| matches!(record.effect,
                crate::campaign::journal::CampaignEffect::Dispatch(Mc2DispatchEvent::Economy {
                    event: super::super::Level2EconomyEvent::PreparedSound { source_entity_index, entity_index, player_index: -1, sound_id: 11 }, ..
                }) if source_entity_index == entity_index && spheres.contains(&entity_index))),
                "actual six-sphere producer publishes sound11 on the source sphere: {:?}", &session.effect_journal().records()[start..]);

            // Carry the acquired family into actual Level18, which has live
            // rival wizards. The incoming hostile Possession projectile below
            // uses the ordinary constructor with a real rival and holder;
            // it is a controlled boundary, not a claim about AI selection.
            let destination = std::env::var_os("MC_MINE_LEVEL18_PACKAGE").map_or_else(
                || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/all-spells-20261104-integration/current-owner-package/level18"),
                std::path::PathBuf::from,
            );
            let mut encounter = CampaignSession::open(destination, CampaignSelection::ordinary(18).unwrap()).unwrap();
            let session = encounter.simulation_mut();
            session.apply_carried_spell_families(&[22], Some(22)).unwrap();
            session.apply_carried_spell_progression(&[CarriedSpellProgress {
                family: 22, persistent_experience: LEVEL2_SPELL_TIER_THRESHOLDS[22][tier], runtime_experience: 0,
                unlocked_tier: tier as i8, selected_tier: tier as i8,
            }]).unwrap();
            let owner = session.state.entity_index;
            let carried_holder = session.shared_spells().local_player.enabled_spell_holders[22];
            assert_ne!(carried_holder, 0);
            let position = session.state.position;
            let castle = session.economy_mut().build_castle(position, owner).unwrap();
            assert!(session.economy_mut().set_castle_mana_prerequisite_for_test(castle, profile.runtime_castle_mana_limit));
            let staged_mana = profile.mana_cost + 1_000;
            let live = &mut session.session.state.live;
            live.economy.refresh_owner_mana_rows();
            for row in &mut live.shared_spells.player_entities {
                if row.index == owner { row.set_mana(staged_mana); row.set_mana_regen(0); }
            }
            live.shared_spells.player_entity.set_mana(staged_mana);
            live.shared_spells.player_entity.set_mana_regen(0);
            live.shared_spells.caster.mana = staged_mana;
            live.shared_spells.player_mana = staged_mana;
            live.economy.entities[usize::from(owner)].set_mana(staged_mana);
            if let Some(fire) = live.core.context.spell_fire.as_mut() { fire.caster.mana = staged_mana; }
            let owner_id = live.economy.entities[usize::from(owner)].source_id;
            let rival = live.economy.entities.iter().find(|row| row.active && (row.class,row.model)==(3,1) && row.life > 0 && row.source_id != owner_id)
                .expect("actual Level18 has an opposing rival").index;
            let rival_handle = live.economy.handle(rival).expect("rival has a live source generation");
            // Nyphur begins with 100 visits of Castle protection. Place the
            // real rival at this encounter site, then let ordinary visits
            // consume the source counter before producing the retaliation.
            encounter.simulation_mut().set_level2_entity_position(rival, position).unwrap();
            for _ in 0..256 {
                let live = &encounter.simulation().session.state.live;
                assert_eq!(live.economy.handle(rival), Some(rival_handle));
                let row = &live.economy.entities[usize::from(rival)];
                let rival_state = live.nyphur_dispatcher.get(rival_handle)
                    .expect("real rival retains its source carrier during protection expiry");
                let upkeep = rival_state.common_upkeep(row.phase);
                if upkeep.word_0x159 == 0 && !upkeep.high_upkeep { break; }
                encounter.step(PlayerCommand::default(), 0);
                assert!(!encounter.simulation().dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })));
            }
            {
                let live = &encounter.simulation().session.state.live;
                let row = &live.economy.entities[usize::from(rival)];
                assert_eq!(live.economy.handle(rival), Some(rival_handle));
                let upkeep = live.nyphur_dispatcher.get(rival_handle).unwrap().common_upkeep(row.phase);
                assert_eq!(upkeep.word_0x159, 0,
                    "ordinary rival visits expire Castle protection before retaliation");
                assert!(!upkeep.high_upkeep, "rival must leave its protected Castle overlap");
            }
            let previous = encounter.simulation().entities().iter()
                .filter(|row| row.active && (row.class, row.model) == (10,57))
                .map(|row| row.index).collect::<Vec<_>>();
            let mut target_sphere = None;
            for turn in 0..4 {
                encounter.step(if turn == 0 { PlayerCommand { opcode: 6, argument_1: 22, flags: 0x40, ..PlayerCommand::default() } } else { PlayerCommand::default() }, 0);
                let session = encounter.simulation();
                assert!(!session.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })), "carried Fool's Mana cast: {:?}", session.dispatch_events());
                target_sphere = session.entities().iter().find(|row| row.active && !row.drawing_disabled()
                    && (row.class,row.model,row.action)==(10,57,62) && word(row,44)==owner_id
                    && row.record[74] == profile.life && !previous.contains(&row.index)).map(|row| row.index);
                if target_sphere.is_some() { break; }
            }
            let target_sphere = target_sphere.expect("public carried cast creates a live actual decoy");
            let sphere = encounter.simulation().entities()[usize::from(target_sphere)].clone();
            let generation = encounter.simulation().session.state.live.economy.economy.slot_generation(target_sphere);
            let rival_holder = encounter.simulation().entities().iter().find(|row| row.active && (row.class,row.model,row.action)==(15,1,3) && word(row,44)==rival)
                .expect("actual rival has a Possession holder").index;
            let origin = Position {
                y: sphere.position.y.wrapping_add(384),
                z: sphere.position.z.wrapping_add(sphere.array_yaw()).wrapping_sub(super::super::POSSESSION_PROJECTILE_COLLISION_VERTICAL_OFFSET),
                ..sphere.position
            };
            encounter.simulation_mut().set_level2_entity_position(rival, origin).unwrap();
            {
                let live = &encounter.simulation().session.state.live;
                let row = &live.economy.entities[usize::from(rival)];
                let upkeep = live.nyphur_dispatcher.get(rival_handle).unwrap().common_upkeep(row.phase);
                assert_eq!(upkeep.word_0x159, 0, "retaliation starts after protection expires");
                assert!(!upkeep.high_upkeep, "the prior ordinary visit was outside protection");
                if upkeep.castle_entity_index != 0 {
                    let castle = &live.economy.entities[usize::from(upkeep.castle_entity_index)];
                    let dx = i32::from(row.position.x.wrapping_sub(castle.position.x) as i16).abs();
                    let dy = i32::from(row.position.y.wrapping_sub(castle.position.y) as i16).abs();
                    assert!(dx >= i32::from(row.array_pitch()) + i32::from(castle.array_pitch())
                        || dy >= i32::from(row.array_roll()) + i32::from(castle.array_roll()),
                        "retaliation geometry is outside the rival's own Castle");
                }
            }
            let initial_life = encounter.simulation().entities()[usize::from(rival)].life;
            let xp = encounter.simulation().shared_spells().spell_experience()[22];
            let incoming = {
                let live = &mut encounter.simulation_mut().session.state.live;
                live.economy.economy.spawn_possession_projectile(origin, rival, rival_holder, 0, 0, 0, sphere.position)
                    .expect("canonical hostile Possession constructor allocates")
            };
            let mut saw_contact = false;
            let mut retaliations = Vec::new();
            let mut hidden = false;
            let mut reclaimed = false;
            let mut damaged = false;
            let mut trace = Vec::new();
            super::super::area_damage::start_effect_damage_trace(rival);
            for turn in 0..200 {
                encounter.step(PlayerCommand::default(), 0);
                let session = encounter.simulation();
                assert!(!session.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })), "Fool's Mana retaliation tier{tier} turn{turn}: {:?}", session.dispatch_events());
                let arena = &session.session.state.live.economy;
                saw_contact |= arena.economy.possession_effects().iter().any(|effect| effect.projectile_entity_index == incoming);
                if profile.life < 2 {
                    for shot in arena.economy.fireball_projectiles().iter().filter(|shot|
                        shot.target_source_entity_index == rival && shot.holder_entity_index == 0
                            && shot.source_subspell_index == profile.subspell_index as u16)
                    {
                        let produced_by_sphere = arena.economy.events().iter().any(|event| matches!(event,
                            super::super::Level2EconomyEvent::PreparedSound {
                                source_entity_index, entity_index, sound_id: 9, ..
                            } if *source_entity_index == shot.source_entity_index && *entity_index == shot.source_entity_index));
                        let identity = (shot.source_entity_index, arena.economy.slot_generation(shot.source_entity_index));
                        if produced_by_sphere && !retaliations.contains(&identity) {
                            retaliations.push(identity);
                        }
                    }
                } else {
                    for shot in arena.entities.iter().filter(|shot| shot.active && (shot.class,shot.model)==(9,9) && shot.source_id == target_sphere && shot.source_word_96() == rival) {
                        let identity = (shot.index, arena.economy.slot_generation(shot.index));
                        if !retaliations.contains(&identity) { retaliations.push(identity); }
                    }
                }
                let current = &arena.entities[usize::from(target_sphere)];
                if arena.economy.slot_generation(target_sphere) != generation || !current.active {
                    reclaimed = true;
                } else {
                    hidden |= current.drawing_disabled();
                }
                let enemy = &arena.entities[usize::from(rival)];
                if trace.len() < 24 && (!retaliations.is_empty() || saw_contact) {
                    let shots = arena.economy.fireball_projectiles().iter()
                        .filter(|shot| retaliations.iter().any(|(slot, _)| *slot == shot.source_entity_index))
                        .map(|shot| (shot.source_entity_index, shot.position, shot.target_source_entity_index,
                            shot.life, shot.active, shot.drawing_disabled, shot.owner_entity_index))
                        .collect::<Vec<_>>();
                    trace.push((turn, enemy.position, enemy.life, enemy.word_26(), shots));
                }
                let source_attributed = if profile.life < 2 {
                    retaliations.iter().any(|(slot, generation)| *slot == enemy.word_26() && arena.economy.slot_generation(*slot) == *generation)
                } else {
                    !retaliations.is_empty() && enemy.word_26() == target_sphere
                };
                damaged |= source_attributed && enemy.life < initial_life;
                if saw_contact && !retaliations.is_empty() && hidden && reclaimed && damaged && session.shared_spells().spell_experience()[22] > xp { break; }
            }
            let damage_receipts = super::super::area_damage::take_effect_damage_trace();
            assert!(saw_contact, "real hostile Possession produces the actual contact effect");
            assert!(!retaliations.is_empty(), "source tier{tier} decoy retaliates at the real rival");
            assert!(encounter.simulation().shared_spells().spell_experience()[22] > xp, "the actual caster receives source Fool's Mana experience");
            assert!(hidden && reclaimed, "contact consumes the real sphere and the following boundary reclaims it");
            assert!(damaged, "the real retaliation damages the opposing rival with source attribution: tier={tier}, initial_life={initial_life}, shots={retaliations:?}, trace={trace:?}, damage_receipts={damage_receipts:?}");
        }
    }

    #[test]
    fn actual_possession_contact_reaches_fools_mana_retaliation_and_hide() {
        let session = super::super::tests::level_two_campaign_session();
        let live = &session.session.state.live;
        let mut arena = live.economy.clone();
        let mut terrain = live.terrain.clone();
        let mut shared = live.shared_spells.clone();
        let mut houses = live.ordinary.houses.clone();
        let particles = &session.session.definition.presentation.particles;
        let claimant = session.state.entity_index;
        let trap_parent = arena
            .entities
            .iter()
            .find(|row| row.active && row.class == 3 && row.index != claimant)
            .expect("the owner package retains another real class3 caster")
            .index;
        let position = Position {
            x: session.state.position.x.wrapping_add(0x4000),
            y: session.state.position.y,
            z: 8192,
        };
        let trap = arena.allocate_source_slot().unwrap();
        let mut sphere = construct_mana_sphere(&arena, trap, particles, position).unwrap();
        put_word(&mut sphere, 44, trap_parent);
        arena.entities[usize::from(trap)] = sphere;
        crate::world::effects::sync_position(&mut arena, &mut terrain, trap, position, true, false)
            .unwrap();
        let shape_offset = signed_word(&arena.entities[usize::from(trap)], 86);
        let origin = Position {
            y: position.y.wrapping_add(384),
            z: position
                .z
                .wrapping_add(shape_offset)
                .wrapping_sub(super::super::POSSESSION_PROJECTILE_COLLISION_VERTICAL_OFFSET),
            ..position
        };
        let projectile = arena
            .economy
            .spawn_possession_projectile(origin, claimant, 0, 0, 0, 0, position)
            .unwrap();
        let frame = super::super::source_target::possession_collision_frame(
            &arena,
            super::super::source_target::Context {
                houses: Some(&houses),
                particles: Some(particles),
                ..Default::default()
            },
            projectile,
            &terrain,
            None,
        )
        .unwrap();
        arena.step_possession_projectile_source_with_frame(
            &mut terrain,
            houses.houses(),
            &[],
            projectile,
            &frame,
        );
        let effect = arena
            .economy
            .possession_effects()
            .iter()
            .find(|row| row.projectile_entity_index == projectile)
            .expect("actual source collision constructs the Possession pulse")
            .source_entity_index;
        arena.step_possession_effect_source(effect, &mut houses);
        assert_eq!(
            word(&arena.entities[usize::from(trap)], 108),
            claimant,
            "the real contact pulse must reach the existing trap mailbox"
        );
        // Whirlwind can set the sphere's source 0x800 bit. sub_35FB0 clears it
        // before reading the real Possession mailbox and runs neither
        // retaliation nor sphere motion on that visit.
        let before = arena.entities[usize::from(trap)].position;
        let flags = arena.entities[usize::from(trap)].control_flags;
        arena.entities[usize::from(trap)].set_control_flags(flags | 0x800);
        let skipped = step_sphere_slot(
            &mut shared,
            &mut arena,
            &mut terrain,
            particles,
            trap,
            super::super::source_target::Context {
                houses: Some(&houses),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(skipped.projectile, None);
        assert!(!skipped.hidden);
        assert_eq!(arena.entities[usize::from(trap)].position, before);
        assert_eq!(arena.entities[usize::from(trap)].control_flags & 0x800, 0);
        assert_eq!(word(&arena.entities[usize::from(trap)], 108), claimant);
        let outcome = step_sphere_slot(
            &mut shared,
            &mut arena,
            &mut terrain,
            particles,
            trap,
            super::super::source_target::Context {
                houses: Some(&houses),
                ..Default::default()
            },
        )
        .unwrap();
        let retaliation = outcome.projectile.expect("the scheduled trap retaliates");
        assert!(outcome.hidden && arena.entities[usize::from(trap)].drawing_disabled());
        let shot = arena
            .economy
            .fireball_projectiles()
            .iter()
            .find(|row| row.source_entity_index == retaliation)
            .unwrap();
        assert_eq!(shot.target_source_entity_index, claimant);
    }
}
