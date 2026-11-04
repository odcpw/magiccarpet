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
    ProjectileSpellCastInput as CastInput, level2_applied_tier, level2_set_spell_profile,
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
    sphere.set_control_flags(sphere.control_flags | 0x0002_0000);
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
    let (_, rule) = live_tier(&holder)?;
    let parent_slot = holder.source_spell_parent_entity_index();
    let parent = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|parent| parent_slot != 0 && parent.active)
        .cloned();

    // NewEvent uses the ordinary free stack; publish the holder row before it
    // can allocate or reuse any other fixed address.
    let holder_row = arena
        .entities
        .get_mut(usize::from(slot))
        .ok_or("Fool's Mana holder address is outside the source arena")?;
    *holder_row = holder.clone();

    let mut caster = parent.as_ref().map(|parent| rules::Caster {
        entity_index: parent.index,
        position: parent.position,
        life: parent.life,
        mana: parent.mana,
        mana_regen: parent.mana_regen,
        castle_mana: input.castle_mana,
        speed: parent.actual_speed,
        yaw: parent.yaw,
        next_entity: parent.next_entity(),
        entity_index2: i16::from_le_bytes([parent.record[172], parent.record[173]]) as u16,
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
        if let Some(authority) = arena.entities.get_mut(usize::from(parent_slot)) {
            authority.set_mana_regen(caster.mana_regen);
            shared.sync_player_entity_from_authority(authority);
            if parent_slot == shared.player_entity.index {
                shared.sync_typed_spell_parent_caches_from_player_entity();
            }
        }
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
                arena.economy.prepare_sound(last, parent_slot, -1, 11);
            }
        }
        arena.economy.source_rng.insert(slot, state.rng);
    }

    holder.active_value = state.active;
    holder.entity_rng = state.rng;
    holder.record[50..52].copy_from_slice(&state.active.to_le_bytes());
    holder.record[58..60].copy_from_slice(&state.cooldown.to_le_bytes());
    holder.set_entity_rng(state.rng);
    if visit.cleanup {
        let deferred = holder.source_word_2c();
        if let Some(deferred_tier) = deferred
            .checked_sub(1)
            .and_then(|value| u8::try_from(value).ok())
            && let Some(profile) = level2_set_spell_profile(22, deferred_tier)
        {
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
    let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(origin, target_position);
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
                    source_id_entity_index: word(sphere, 44),
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
            arena.economy.prepare_sound(projectile, sphere.index, -1, 9);
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
    let search_class = ((extent - extent.signum() * 255) >> 8).clamp(0, 31) as u8;
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
            || candidate.source_spell_parent_entity_index() == sphere.source_id
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
    row: &mut EntitySnapshot,
) -> Result<bool, String> {
    if row.control_flags & 0x800 != 0 {
        row.set_control_flags(row.control_flags & !0x800);
        return Ok(false);
    }

    let mut position = row.position;
    let mut gravity = signed_word(row, 48);
    let mut grounded_drift = false;
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
        let ground = terrain.ground_height(position);
        if ground > position.z {
            position.z = ground;
            row.set_actual_speed(0);
            gravity = 128;
        }
    } else if row.record[61] != 0 {
        let axis_x = signed_word(row, 158).clamp(-64, 64);
        let axis_y = signed_word(row, 160).clamp(-64, 64);
        set_signed_word(row, 158, axis_x);
        set_signed_word(row, 160, axis_y);
        position.x = position.x.wrapping_add(axis_x as u16);
        position.y = position.y.wrapping_add(axis_y as u16);
        position.z = position.z.wrapping_add(gravity);
        gravity = gravity.wrapping_sub(16).max(-128);

        let ground = terrain.ground_height(position);
        if ground >= position.z {
            position.z = ground;
            let sign = i32::from(gravity).signum();
            let bounce = -((i32::from(gravity) - sign * 4 + sign) >> 2);
            gravity = if bounce <= 16 { 0 } else { bounce as i16 };
            grounded_drift = true;
        }
    }

    set_signed_word(row, 48, gravity);
    row.set_position(position);
    arena.entities[usize::from(row.index)] = row.clone();
    crate::world::effects::sync_position(arena, terrain, row.index, position, true, false)?;
    *row = arena.entities[usize::from(row.index)].clone();
    Ok(grounded_drift)
}

/// sub_35FB0's class10/model57 scheduled callback, including collision-link
/// acquisition, retaliation, source sphere flight, and the ground-contact
/// sub_36F30 immediate merge/reclaim.
pub(crate) fn step_sphere_slot(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    context: super::source_target::Context<'_>,
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
    // sub_10780's map-chain collision writes the same source word_0x68 read by
    // sub_36680. Existing links are retained so the tier cadence can observe
    // repeated contacts over successive ordinary callbacks.
    if word(&row, 108) == 0
        && let Some((target, _)) = super::guided_projectile::collision(
            arena,
            &row,
            super::source_target::Context {
                spells: Some(&*shared),
                particles: Some(particles),
                map_type: Some(terrain.map_type()),
                ..context
            },
        )?
    {
        put_word(&mut row, 108, target);
        row.record[104..108].fill(0);
    }
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
        move_sphere(arena, terrain, &mut row)?
    };
    if grounded_drift && let Some(absorbed_slot) = merge_candidate(arena, &row) {
        let absorbed = arena.entities[usize::from(absorbed_slot)].clone();
        state = sphere_state(&row);
        let merged = state.merge(sphere_state(&absorbed));
        write_sphere_state(&mut row, state);
        outcome.merged_into = Some(slot);
        outcome.absorbed_sphere = Some(merged.absorbed_entity);
        crate::world::terrain::cave_features::reclaim(arena, terrain, absorbed_slot);
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
