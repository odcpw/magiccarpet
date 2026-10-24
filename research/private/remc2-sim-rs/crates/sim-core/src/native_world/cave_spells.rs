//! MC2 CaveIn identities and lossless state bindings. Decisions and collapse
//! geometry live in spells::cave; the shared arena supplies ordinary lifetimes.

use super::{
    EntitySnapshot, Level2SharedSpellResetSession, MapType, ParticleParameter, RenderableMapSeed,
    SessionTerrain,
};
use crate::level1_mission::{source_map_search_slots, source_search_offsets};
use crate::spells::cave::target_score;
use crate::{
    FlightWorld, Position,
    campaign::{source_arena::SourceArena, state::CanonicalRuntimeState},
    spells::cave::{
        CaveInCaster, CaveInEffect, CaveInHolder, CaveInProjectile, CaveInRule, CaveTerrain,
    },
};

struct Terrain<'a>(&'a mut SessionTerrain);
impl CaveTerrain for Terrain<'_> {
    fn perimeter(&self, x: u8, y: u8, diameter: u16) -> (i32, i32) {
        let floor = crate::legacy_terrain::cave_perimeter_extrema(
            &self.0.planes.ground_heights,
            x,
            y,
            diameter,
            diameter,
        )
        .0;
        let ceiling = crate::legacy_terrain::cave_perimeter_extrema(
            &self.0.planes.secondary_heights,
            x,
            y,
            diameter,
            diameter,
        )
        .1;
        (floor, ceiling)
    }
    fn floor(&self, x: u8, y: u8) -> u8 {
        self.0.planes.ground_heights[super::terrain_index(x, y)]
    }
    fn ceiling(&self, x: u8, y: u8) -> u8 {
        self.0.planes.secondary_heights[super::terrain_index(x, y)]
    }
    fn write_floor(&mut self, x: u8, y: u8, height: i32, force_material: bool) {
        super::cave_features::write_ground(self.0, x, y, height, force_material, true);
    }
    fn write_ceiling(&mut self, x: u8, y: u8, height: u8) {
        self.0.planes.secondary_heights[super::terrain_index(x, y)] = height;
    }
    fn normalize(&mut self, x: u8, y: u8) {
        let i = super::terrain_index(x, y);
        if self.0.planes.secondary_heights[i] > self.0.planes.ground_heights[i] {
            self.0.planes.angles[i] &= 0xf7;
        } else {
            self.0.planes.secondary_heights[i] = self.0.planes.ground_heights[i];
            self.0.planes.angles[i] |= 8;
        }
    }
}

fn set_owner(row: &mut EntitySnapshot, owner: u16) {
    row.source_id = owner;
    row.record[30..32].copy_from_slice(&owner.to_le_bytes());
}

fn publish(arena: &mut SourceArena, mut row: EntitySnapshot) -> Result<u16, String> {
    let slot = row.index;
    arena.economy.map_chains.sync_entity(RenderableMapSeed {
        entity_index: slot,
        active: true,
        x: row.position.x,
        y: row.position.y,
    })?;
    let old = arena.economy.map_chains.old_map_entity(slot);
    row.set_control_flags(row.control_flags | 4);
    row.record[26..28].copy_from_slice(&old.to_le_bytes());
    if old != 0
        && !arena
            .economy
            .live_arena_slots()
            .iter()
            .any(|(id, _)| *id == old)
    {
        arena.entities[usize::from(old)].record[28..30].copy_from_slice(&slot.to_le_bytes());
    }
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    Ok(slot)
}

/// sub_50A20 does not allocate outside Cave. This effect is never an economy
/// sidecar: its stage, wave and lifetime remain in this one raw source row.
pub(crate) fn construct_effect(
    arena: &mut SourceArena,
    terrain: &SessionTerrain,
    position: Position,
    tier: u8,
) -> Result<Option<u16>, String> {
    if terrain.map_type != MapType::Cave {
        return Ok(None);
    }
    let Some(slot) = arena.allocate_source_slot() else {
        return Ok(None);
    };
    let mut row =
        super::cave_features::new_event(slot, arena.economy.new_event_rng(slot), position);
    row.class = 10;
    row.model = 89;
    row.record[67] = 10;
    row.record[68] = 89;
    row.set_source_action(96);
    row.set_control_flags((row.control_flags & !9) | 1);
    row.set_life(40);
    // sub_67910's successful-delivery overrides, after sub_50A20.
    row.set_max_life(i32::from(tier));
    row.set_source_byte_46(0);
    publish(arena, row).map(Some)
}

fn launch(
    arena: &mut SourceArena,
    terrain: &SessionTerrain,
    shared: &mut Level2SharedSpellResetSession,
    core: &mut CanonicalRuntimeState,
    holder: &EntitySnapshot,
    parent: &EntitySnapshot,
    particles: &[ParticleParameter],
) -> Result<Option<u16>, String> {
    let Some(slot) = arena.allocate_source_slot() else {
        return Ok(None);
    };
    let mut position = parent.position;
    let hand = if parent.control_flags & 0x100 != 0 {
        Some(parent.yaw.wrapping_sub(512))
    } else if parent.control_flags & 0x200 != 0 {
        Some(parent.yaw.wrapping_add(512))
    } else {
        None
    };
    if let Some(yaw) = hand {
        crate::move_entity(&mut position, yaw, 0, 256);
        if terrain.ground_height(position) > position.z {
            position = parent.position;
        }
    }
    position.z = position.z.wrapping_add(parent.array_fov());
    let mut row =
        super::cave_features::new_event(slot, arena.economy.new_event_rng(slot), position);
    row.class = 9;
    row.model = 30;
    row.record[67] = 9;
    row.record[68] = 30;
    row.set_source_action(31);
    row.set_control_flags(row.control_flags & !8);
    row.parameter_relation = Some(60);
    row.record[164..168].copy_from_slice(&61_u32.to_le_bytes());
    row.set_max_life(8192 / 384);
    row.set_life(row.max_life);
    row.set_actual_speed(384_i16.wrapping_add(parent.actual_speed).clamp(384, 8192));
    row.record[136..138].copy_from_slice(&384_i16.to_le_bytes());
    super::cave_features::bind_particle(
        &mut row,
        211,
        *particles.get(211).ok_or("CaveIn particle211 absent")?,
    )?;
    set_owner(&mut row, parent.source_id);
    row.record[42..44].copy_from_slice(&holder.index.to_le_bytes());
    row.record[71] = 10;
    row.record[72] = 89;
    let tier = holder.level_gate as u8;
    let rule = CaveInRule {
        power: holder.subspell_index,
        radius_tier: tier,
    };
    row.set_subspell_index(rule.projectile_power());
    row.set_source_byte_46(tier);
    row.set_mana(holder.mana);
    let auxiliary = shared.beyond_sight_auxiliary();
    let mut yaw = parent.yaw;
    let mut pitch = parent.pitch;
    if parent.index == shared.player_entity.index {
        yaw = yaw.wrapping_add(auxiliary.next_entity);
        pitch = pitch.wrapping_add(auxiliary.entity_index2);
    } else if let Some(player) = shared
        .players
        .iter()
        .find(|player| player.entity_index == parent.index)
    {
        if let Some(bytes) = player
            .record
            .get(super::PLAYER_LEGACY_RECORD_BYTES + 24..super::PLAYER_LEGACY_RECORD_BYTES + 28)
        {
            yaw = yaw.wrapping_add(i16::from_le_bytes([bytes[0], bytes[1]]));
            pitch = pitch.wrapping_add(i16::from_le_bytes([bytes[2], bytes[3]]));
        }
    }
    row.set_yaw(yaw);
    row.set_pitch(pitch);
    let mut axis = parent.position;
    crate::move_entity(&mut axis, yaw, 0, 4096);
    axis.z = terrain.ground_height(axis);
    row.record[158..160].copy_from_slice(&axis.x.to_le_bytes());
    row.record[160..162].copy_from_slice(&axis.y.to_le_bytes());
    row.record[162..164].copy_from_slice(&axis.z.to_le_bytes());
    if let Some(player) = shared
        .players
        .iter_mut()
        .find(|player| player.entity_index == parent.index)
        && let Some(aux) = player.type_str_164_auxiliary.as_mut()
    {
        row.set_source_dword_10(i32::from(aux.byte_0x154_340));
        aux.byte_0x154_340 = 0;
        player.record[super::PLAYER_LEGACY_RECORD_BYTES + 16] = 0;
        if shared.local_player.entity_index == parent.index {
            shared.local_player = player.clone();
        }
    }
    if let Some(player) = core
        .players
        .iter_mut()
        .find(|player| player.entity_index == parent.index)
        && let Some(aux) = player.type_str_164_auxiliary.as_mut()
    {
        aux.byte_0x154_340 = 0;
        player.record[super::PLAYER_LEGACY_RECORD_BYTES + 16] = 0;
    }
    let slot = publish(arena, row)?;
    crate::mark_source_audio_call(crate::SourceAudioCallTag::ProjectileDeliveryLaunch);
    shared.emit_flight_effect(crate::FlightEvent::PrepareSound {
        #[cfg(test)]
        audio_site: crate::MissionAudioSite::CaveInLaunch,
        entity_index: parent.index,
        player_index: -1,
        sound_id: 15,
    });
    Ok(Some(slot))
}

pub(crate) fn step_holder(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    core: &mut CanonicalRuntimeState,
    terrain: &SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
) -> Result<(), String> {
    let mut holder = shared
        .pickup(slot)
        .cloned()
        .ok_or("CaveIn holder missing")?;
    let parent_slot = holder.source_spell_parent_entity_index();
    let mut parent = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|row| parent_slot != 0 && row.active)
        .cloned();
    let owner = shared
        .players
        .iter()
        .find(|player| player.entity_index == parent_slot);
    let castle = arena
        .economy
        .castle()
        .filter(|castle| {
            castle.active
                && castle.owner_entity_index == parent_slot
                && arena.economy.player_castle_entity_index() == castle.source_entity_index
        })
        .map(|castle| castle.mana)
        .or_else(|| {
            let player = owner?;
            let slot = u16::from_le_bytes([player.record[42], player.record[43]]);
            arena
                .entities
                .get(usize::from(slot))
                .filter(|row| slot != 0 && row.active)
                .map(|row| row.mana)
        });
    let mut state = CaveInHolder {
        active: holder.active_value,
        duration: holder.activation_value as i16,
        cooldown: u16::from_le_bytes([holder.record[58], holder.record[59]]),
        mana_cost: holder.max_mana,
        castle_mana_required: holder.mana_regen,
    };
    let mut caster = parent.as_ref().map(|row| CaveInCaster {
        life: row.life,
        mana: row.mana,
        mana_regen: row.mana_regen,
        castle_mana: castle,
    });
    let result = state.step(terrain.map_type == MapType::Cave, caster.as_mut());
    if result.launch {
        launch(
            arena,
            terrain,
            shared,
            core,
            &holder,
            parent.as_ref().ok_or("CaveIn launch lost parent")?,
            particles,
        )?;
    }
    if let (Some(parent), Some(caster)) = (parent.as_mut(), caster) {
        parent.set_mana_regen(caster.mana_regen);
        arena.entities[usize::from(parent_slot)] = parent.clone();
        shared.sync_player_entity_from_authority(parent);
        if parent_slot == shared.player_entity.index {
            shared.sync_typed_spell_parent_caches_from_player_entity();
        }
    }
    holder.active_value = state.active;
    holder.record[50..52].copy_from_slice(&state.active.to_le_bytes());
    holder.record[58..60].copy_from_slice(&state.cooldown.to_le_bytes());
    if result.cleanup
        && let Some(tier) = holder
            .source_word_2c()
            .checked_sub(1)
            .and_then(|tier| u8::try_from(tier).ok())
        && let Some(profile) = crate::spell_activation::level2_set_spell_profile(25, tier)
    {
        holder.apply_nyphur_spell_profile(profile, tier);
        holder.set_source_word_2c(0);
    }
    if let Some(target) = shared.pickup_mut(slot) {
        *target = holder.clone();
    }
    arena.entities[usize::from(slot)] = holder.clone();
    if let Some(fire) = core.context.spell_fire.as_mut()
        && let Some(target) = fire
            .holders
            .iter_mut()
            .find(|target| target.entity_index == slot)
    {
        target.active_value = holder.active_value;
        target.activation_value = holder.activation_value;
        target.level_gate = holder.level_gate;
        target.mana_cost = holder.mana_cost;
    }
    let kind = arena
        .economy
        .live_arena_cursor_rows()
        .into_iter()
        .find(|row| row.source_slot == slot)
        .map(|row| row.kind);
    let advanced = match kind {
        Some(super::Level2DynamicArenaKind::SpellPickup) => {
            arena.economy.step_spell_pickup_slot(slot)
        }
        Some(super::Level2DynamicArenaKind::SpellReplacement) => {
            arena.economy.step_spell_replacement_slot(slot)
        }
        _ => false,
    };
    if advanced && let Some(holder) = shared.pickup_mut(slot) {
        holder.set_phase(holder.phase.wrapping_add(1));
        arena.entities[usize::from(slot)] = holder.clone();
    }
    Ok(())
}

pub(crate) fn step_effect(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
) -> Result<Vec<(Position, i16)>, String> {
    let row = &mut arena.entities[usize::from(slot)];
    let mut state = CaveInEffect {
        position: row.position,
        radius_tier: row.max_life as u8,
        stage: row.record[74],
        life: row.life,
        wave_phase: row.source_word_2c(),
        smoke_emitted: u16::from_le_bytes([row.record[58], row.record[59]]) != 0,
        floor: i16::from_le_bytes([row.record[162], row.record[163]]),
    };
    let wizards = arena
        .entities
        .iter()
        .filter(|row| row.active && row.class == 3 && row.model == 0)
        .map(|row| row.position)
        .collect::<Vec<_>>();
    let visit = state.step(&mut Terrain(terrain), &wizards);
    let row = &mut arena.entities[usize::from(slot)];
    row.set_position(state.position);
    row.set_source_byte_46(state.stage);
    row.set_life(state.life);
    row.set_source_word_2c(state.wave_phase);
    row.record[58..60].copy_from_slice(&u16::from(state.smoke_emitted).to_le_bytes());
    row.record[162..164].copy_from_slice(&state.floor.to_le_bytes());
    if visit.hide {
        row.disable_drawing();
    }
    if visit.terrain_mutated {
        terrain.mutation_sequence = terrain.mutation_sequence.wrapping_add(1);
    }
    Ok(visit.smoke)
}

pub(crate) fn step_source(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    shared: &mut Level2SharedSpellResetSession,
    core: &mut CanonicalRuntimeState,
    slot: u16,
    parameters: &[[u8; super::ENTITY_PARAMETER_BYTES]],
) -> Result<(), String> {
    let row = &arena.entities[usize::from(slot)];
    match (row.class, row.model, row.action) {
        (9, 30, 31) => step_projectile(arena, terrain, shared, core, slot, parameters),
        (10, 89, 96) => {
            for (position, yaw) in step_effect(arena, terrain, slot)? {
                if let Some(child) = arena.allocate_smoke13_with_terrain(position, terrain)
                    && let Some(row) = arena
                        .economy
                        .model59_smoke_children
                        .iter_mut()
                        .find(|row| row.source_entity_index == child)
                {
                    row.yaw = yaw;
                    row.max_speed = 512;
                    row.position = position;
                }
            }
            Ok(())
        }
        selector => Err(format!(
            "CaveIn callback received unrelated selector {selector:?}"
        )),
    }
}

/// Source sub_67910 delivers10/89 only after a successful ordinary collision
/// or flight expiry; a failed constructor keeps the flyer alive for retry.
pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &SessionTerrain,
    shared: &mut Level2SharedSpellResetSession,
    core: &mut CanonicalRuntimeState,
    slot: u16,
    parameters: &[[u8; super::ENTITY_PARAMETER_BYTES]],
) -> Result<(), String> {
    let mut row = arena.entities[usize::from(slot)].clone();
    let mut state = CaveInProjectile {
        position: row.position,
        yaw: row.yaw,
        pitch: row.pitch,
        roll: row.source_roll(),
        fov: row.fov,
        speed: row.actual_speed,
        min_speed: row.minimum_speed(),
        life: row.life,
        max_life: row.max_life,
        flags: row.control_flags,
        target: row.source_word_96(),
    };
    let guidance = if state.target != 0 {
        arena
            .entities
            .get(usize::from(state.target))
            .filter(|target| target.active)
            .map(|target| {
                (
                    target.index,
                    Position {
                        z: target.position.z.wrapping_add(if target.model == 2 {
                            0
                        } else {
                            target.array_yaw()
                        }),
                        ..target.position
                    },
                )
            })
    } else if state.flags & 2 == 0 {
        let marker_range = if let Some(owner) = arena
            .entities
            .get(usize::from(row.source_id))
            .filter(|owner| owner.active && owner.class == 3)
        {
            let parameter = owner
                .parameter_relation
                .and_then(|index| parameters.get(usize::from(index)))
                .ok_or("CaveIn source owner parameter missing")?;
            Some(u32::from(u16::from_le_bytes([
                parameter[28],
                parameter[29],
            ])))
        } else {
            None
        };
        let marker = arena
            .entities
            .iter()
            .filter(|target| {
                target.active
                    && target.class == 10
                    && target.model == 78
                    && target.word_32() == row.source_id
                    && i16::from_le_bytes([target.record[58], target.record[59]]) == -1
            })
            .filter(|target| {
                crate::angle_distance(
                    i32::from(row.yaw),
                    i32::from(crate::heading_for_positions(row.position, target.position) as i16),
                ) < 170
            })
            .min_by_key(|target| crate::distance_3d_wrapped(row.position, target.position))
            .filter(|target| {
                marker_range.is_some_and(|range| {
                    crate::distance_3d_wrapped(row.position, target.position) < range
                })
            })
            .map(|target| (target.index, target.position));
        marker
            .or_else(|| {
                arena
                    .economy
                    .fireball_guidance_targets(&arena.entities)
                    .into_iter()
                    .filter(|target| target.source_id != row.source_id)
                    .filter(|target| {
                        arena
                            .entities
                            .get(usize::from(target.source_slot))
                            .is_none_or(|target| target.control_flags & 0x20 == 0)
                    })
                    .filter_map(|target| {
                        target_score(row.position, row.yaw, row.pitch, target.top_position)
                            .map(|score| (score, target))
                    })
                    .min_by_key(|(score, _)| *score)
                    .map(|(_, target)| (target.source_slot, target.top_position))
            })
            .or_else(|| {
                // sub_67CB0 only tries model22's linked bodies when no ordinary
                // wizard/creature candidate qualified. Keep source chain order.
                let mut best = None;
                let mut best_score = u32::MAX;
                for root in arena.entities.iter().filter(|root| {
                    root.active
                        && !root.drawing_disabled()
                        && root.life >= 0
                        && root.class == 5
                        && root.model == 22
                        && root.record[61] != 0
                }) {
                    let mut next = root.word_34();
                    for _ in 0..arena.entities.len() {
                        if next == 0 {
                            break;
                        }
                        let Some(target) = arena
                            .entities
                            .get(usize::from(next))
                            .filter(|target| target.active)
                        else {
                            break;
                        };
                        let position = Position {
                            z: target.position.z.wrapping_add(target.array_yaw()),
                            ..target.position
                        };
                        if let Some(score) =
                            target_score(row.position, row.yaw, row.pitch, position)
                            && score < best_score
                        {
                            best = Some((target.index, position));
                            best_score = score;
                        }
                        next = target.word_34();
                    }
                }
                best
            })
    } else {
        None
    };
    let acquiring = state.target == 0 && state.flags & 2 == 0;
    state.advance(guidance);
    if acquiring
        && let Some((slot, _)) = guidance
        && let Some(target) = arena.entities.get_mut(usize::from(slot))
    {
        if target.class == 5 && target.model == 0 {
            target.record[61] = 32;
        }
        target.apply_source_sub_5ef70();
        if target.class == 3 {
            shared.sync_player_entity_from_authority(target);
        }
    }
    if acquiring
        && let Some((slot, _)) = guidance
        && let Some(dragon) =
            arena.economy.authored_class5.iter_mut().find(|source| {
                source.active && source.source_entity_index == slot && source.model == 0
            })
    {
        dragon.source_byte_39 = 32;
    }
    let targets = arena.economy.fireball_collision_targets(&arena.entities);
    let hit = source_map_search_slots(
        &arena.economy.map_chains,
        state.position,
        &source_search_offsets(1),
    )
    .into_iter()
    .find_map(|slot| {
        targets
            .iter()
            .find(|target| {
                target.source_slot == slot
                    && target.source_id != row.source_id
                    && (row.record[69] == 255
                        || arena.entities.get(usize::from(slot)).is_some_and(|target| {
                            target.active
                                && target.class == row.record[69]
                                && (row.record[70] == 255 || target.model == row.record[70])
                        }))
                    && crate::level1_mission::source_entities_overlap(
                        state.position,
                        row.array_pitch(),
                        row.array_yaw(),
                        row.array_fov(),
                        target.position,
                        target.half_width,
                        target.vertical_offset,
                        target.half_height,
                    )
            })
            .copied()
    });
    let mut reflection = None;
    if let Some(target) = hit
        .and_then(|hit| arena.entities.get(usize::from(hit.source_slot)))
        .filter(|target| target.active && target.control_flags & 0x8010 != 0)
    {
        use crate::spells::support::{
            ReboundProjectile, ReboundSpread, ReboundTarget, reflect_projectile,
        };
        let mut projectile = ReboundProjectile {
            owner: row.source_id,
            target: state.target,
            position: state.position,
            yaw: state.yaw,
            pitch: state.pitch,
            roll: state.roll,
            fov: state.fov,
            damage: row.subspell_index,
            mana: row.mana,
            life: state.life,
            max_life: state.max_life,
            rng: row.entity_rng,
        };
        if let Some(mana_after) = reflect_projectile(
            &mut projectile,
            ReboundTarget {
                source_id: target.source_id,
                flags: target.control_flags,
                mana: target.mana,
                position: target.position,
                fov: target.array_fov(),
            },
            ReboundSpread::Narrow,
        ) {
            state.position = projectile.position;
            state.yaw = projectile.yaw;
            state.pitch = projectile.pitch;
            state.roll = projectile.roll;
            state.fov = projectile.fov;
            state.life = projectile.life;
            state.target = projectile.target;
            set_owner(&mut row, projectile.owner);
            if let Some(target) = arena
                .entities
                .get(usize::from(projectile.target))
                .filter(|target| target.active)
            {
                row.record[69] = target.class;
                row.record[70] = target.model;
            }
            row.set_subspell_index(projectile.damage);
            row.set_entity_rng(projectile.rng);
            reflection = Some(super::support_spells::ReboundReflection {
                target_slot: target.index,
                mana_after,
            });
        }
    }
    if reflection.is_none() {
        let ground = terrain.ground_height(state.position);
        let ceiling = terrain
            .ceiling_height(state.position)
            .map(|height| height.wrapping_sub(row.array_fov()));
        let terrain_contact = hit.is_none()
            && (ground > state.position.z || ceiling.is_some_and(|limit| state.position.z > limit));
        let terminal = state.contact(
            ground,
            ceiling,
            hit.map(|hit| Position {
                z: hit.position.z.wrapping_add(hit.vertical_offset),
                ..hit.position
            }),
        );
        if terminal {
            let marker = hit
                .and_then(|hit| arena.entities.get(usize::from(hit.source_slot)))
                .filter(|target| {
                    target.active
                        && target.class == 10
                        && target.model == 78
                        && target.word_32() == row.source_id
                        && i16::from_le_bytes([target.record[58], target.record[59]]) == -1
                })
                .map(|target| target.index);
            if terrain_contact && terrain.material_mask(state.position) == 1 {
                if let Some(splash) = arena
                    .economy
                    .allocate_authored_model5_water_splash(state.position, terrain)
                {
                    if let Some(child) = arena
                        .economy
                        .authored_model5_water_splashes
                        .iter_mut()
                        .find(|child| child.source_entity_index == splash.source_entity_index)
                    {
                        child.entity_id = row.source_id;
                    }
                    arena
                        .economy
                        .source_owner
                        .insert(splash.source_entity_index, row.source_id);
                }
                row.disable_drawing();
            } else if let Some(marker) = marker {
                let _ = arena.economy.allocate_model0_child(
                    slot,
                    slot,
                    0,
                    state.position,
                    0x20004,
                    128,
                    128,
                );
                crate::mark_source_audio_call(crate::SourceAudioCallTag::SpellProjectileCapture);
                shared.emit_flight_effect(crate::FlightEvent::PrepareSound {
                    #[cfg(test)]
                    audio_site: crate::MissionAudioSite::CaveInCapture,
                    entity_index: slot,
                    player_index: -1,
                    sound_id: 26,
                });
                let holder_slot = row.word_26();
                if let Some(holder) = shared.pickup_mut(holder_slot) {
                    let family = i16::from(holder.model);
                    let tier = i16::from(holder.level_gate);
                    holder.active_value = 1;
                    holder.record[50..52].copy_from_slice(&1_i16.to_le_bytes());
                    arena.entities[usize::from(holder_slot)] = holder.clone();
                    let target = &mut arena.entities[usize::from(marker)];
                    target.record[58..60].copy_from_slice(&family.to_le_bytes());
                    target.record[56..58].copy_from_slice(&tier.to_le_bytes());
                }
                row.disable_drawing();
            } else if let Some(effect) =
                construct_effect(arena, terrain, state.position, row.record[74])?
            {
                let effect = &mut arena.entities[usize::from(effect)];
                set_owner(effect, row.source_id);
                effect.set_yaw(state.yaw);
                effect.set_pitch(state.pitch);
                effect.set_subspell_index(row.subspell_index);
                effect.set_source_word_96(hit.map_or(0, |hit| hit.source_slot));
                if hit.is_some()
                    && arena
                        .entities
                        .get(usize::from(row.source_id))
                        .is_some_and(|parent| {
                            parent.active && parent.class == 3 && parent.model == 0
                        })
                    && shared
                        .players
                        .iter()
                        .any(|player| player.entity_index == row.source_id)
                {
                    shared.award_spell_experience(row.source_id, 25, 1);
                    if let Some(owner) = shared
                        .players
                        .iter()
                        .find(|player| player.entity_index == row.source_id)
                    {
                        for player in core
                            .players
                            .iter_mut()
                            .filter(|player| player.entity_index == row.source_id)
                        {
                            player.sync_spell_progression_family_from(owner, 25);
                        }
                    }
                    if let Some(fire) = core.context.spell_fire.as_mut()
                        && fire.caster.entity_index == row.source_id
                    {
                        fire.spell_experience[25] = shared.spell_experience[25];
                        fire.spell_levels[25] = shared.spell_levels[25];
                    }
                }
                if let Some(hit) = hit
                    && let Some(parent) = arena
                        .entities
                        .get_mut(usize::from(row.source_id))
                        .filter(|parent| parent.active && parent.class == 3 && parent.model <= 1)
                {
                    parent.set_source_word_96(hit.source_slot);
                    shared.sync_player_entity_from_authority(parent);
                }
                row.disable_drawing();
            }
        }
    }
    row.set_position(state.position);
    row.set_yaw(state.yaw);
    row.set_pitch(state.pitch);
    row.set_source_roll(state.roll);
    row.set_fov(state.fov);
    row.set_actual_speed(state.speed);
    row.set_life(state.life);
    row.set_source_word_96(state.target);
    row.set_control_flags(state.flags | (row.control_flags & 0x400));
    arena.economy.map_chains.sync_entity(RenderableMapSeed {
        entity_index: slot,
        active: true,
        x: row.position.x,
        y: row.position.y,
    })?;
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    if let Some(reflection) = reflection {
        super::support_spells::commit_reflection(shared, arena, core, reflection);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PlayerCommand,
        native_world::{Level2DispatchEvent, NativeWorld},
    };

    #[test]
    fn owner_cavein_pickup_cast_collapses_live_planes_and_uses_following_pass_reclaim() {
        let private = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let package = private.join("generated/mc-bhq6-ordinary-20261024-b/level24");
        let authored = crate::level1_mission::parse_authored_features(
            &std::fs::read(package.join("authored-level.bin"))
                .expect("genuine Cave24 authored data"),
        )
        .unwrap();
        let feature = authored[448];
        assert_eq!(
            (feature.kind, feature.subtype, feature.stage_tag),
            (15, 25, 1)
        );
        let source =
            std::fs::read(private.join("remc2-native-runtime/CD_Files/DATA/SPELLS.DAT")).unwrap();
        for tier in 0..3_u8 {
            let offset = 25 * 80 + 2 + usize::from(tier) * 26;
            let row = &source[offset..offset + 26];
            let profile = crate::spell_activation::level2_set_spell_profile(25, tier).unwrap();
            let dword = |i| i32::from_le_bytes(row[i..i + 4].try_into().unwrap());
            assert_eq!(
                (
                    profile.subspell_index,
                    profile.max_mana,
                    profile.mana_regen,
                    profile.word_0x30
                ),
                (
                    dword(0) as u16,
                    dword(4),
                    dword(8),
                    u16::from_le_bytes([row[22], row[23]])
                )
            );
            assert_eq!(row[24], tier, "actual source radius mode");
        }
        // Direct real-family execution, using the qualified numeric arena and
        // exact Cave24 geometry/feature. This is not a full Mission24 claim.
        let inspection =
            NativeWorld::inspect_level2(crate::native_world::tests::level_two_fixture_path())
                .unwrap();
        let mut session = inspection.level2_reset_session().unwrap();
        session.select_level2_campaign_entry_stage();
        {
            let live = &mut session.session.state.live;
            let free = live.economy.free_slot_count();
            assert!(
                construct_effect(&mut live.economy, &live.terrain, Position::default(), 0)
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                live.economy.free_slot_count(),
                free,
                "non-Cave null constructor consumes no slot"
            );
        }
        std::sync::Arc::make_mut(&mut session.session.definition).profile =
            crate::FlightProfile::CAVE;
        let planes = crate::native_world::parse_terrain(
            &std::fs::read(package.join("terrain-post-features.bin")).unwrap(),
        )
        .unwrap();
        let (slot, owner, start) = {
            let live = &mut session.session.state.live;
            live.terrain.map_type = MapType::Cave;
            live.terrain.planes.terrain_types = planes.terrain_types;
            live.terrain.planes.ground_heights = planes.ground_heights;
            live.terrain.planes.shading = planes.shading;
            live.terrain.planes.angles = planes.angles;
            live.terrain.planes.secondary_heights = planes.secondary_heights;
            let slot = live
                .shared_spells
                .materialize_authored_spell_pickup(&mut live.economy, feature, &live.terrain)
                .unwrap()
                .unwrap();
            let mut start = live.shared_spells.pickup(slot).unwrap().position;
            start.z = start.z.wrapping_add(100);
            let owner = live.core.state.entity_index;
            live.core.state.position = start;
            live.core.state.target_speed = 0;
            live.core.state.actual_speed = 0;
            let mut player = live.shared_spells.player_entity.clone();
            player.set_position(start);
            player.set_mana(500_000);
            player.max_mana = 500_000;
            player.record[144..148].copy_from_slice(&500_000_i32.to_le_bytes());
            live.shared_spells
                .sync_player_entity_from_authority(&player);
            live.shared_spells.caster.mana = player.mana;
            live.shared_spells.player_mana = player.mana;
            live.economy.entities[usize::from(owner)] = player;
            live.economy
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: owner,
                    active: true,
                    x: start.x,
                    y: start.y,
                })
                .unwrap();
            (slot, owner, start)
        };
        let castle = session.economy_mut().build_castle(start, owner).unwrap();
        assert!(
            session
                .economy_mut()
                .set_castle_mana_prerequisite_for_test(castle, 300_000)
        );
        let no_errors = |session: &crate::native_world::Level2ResetSession| {
            let errors = session
                .dispatch_events()
                .iter()
                .filter(|e| matches!(e, Level2DispatchEvent::Error { .. }))
                .collect::<Vec<_>>();
            assert!(errors.is_empty(), "{errors:?}");
        };
        for _ in 0..16 {
            session.step_outer_turn(PlayerCommand::default(), 0);
            no_errors(&session);
            if session.shared_spells().players()[0].enabled_spell_holders[25] == slot {
                break;
            }
        }
        assert_eq!(
            session.shared_spells().players()[0].enabled_spell_holders[25],
            slot
        );
        {
            // Actual Cave24 room with seven open tiles of clearance. The
            // pickup alcove can deliver into its ceiling within one outer
            // turn; cast in real open air to observe a visible flyer too.
            let live = &mut session.session.state.live;
            let mut position = Position {
                x: (162 << 8) + 128,
                y: (93 << 8) + 128,
                z: 0,
            };
            let floor = live.terrain.ground_height(position);
            let ceiling = live.terrain.ceiling_height(position).unwrap();
            assert!(ceiling - floor > 4096, "actual room has casting clearance");
            position.z = floor + (ceiling - floor) / 2;
            live.core.state.position = position;
            let mut player = live.shared_spells.player_entity.clone();
            player.set_position(position);
            live.shared_spells
                .sync_player_entity_from_authority(&player);
            live.economy.entities[usize::from(owner)] = player;
            live.economy
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: owner,
                    active: true,
                    x: position.x,
                    y: position.y,
                })
                .unwrap();
        }
        let before = session.canonical_terrain().planes().clone();
        let mana = session.shared_spells().player_entity.mana;
        {
            // Exercise the actual failed NewEvent branch through ordinary
            // casting. This isolated clone keeps all pool occupants live and
            // nonreplaceable, without changing the positive source scenario.
            let mut exhausted = session.clone();
            let live = &mut exhausted.session.state.live;
            live.economy.economy.exhaust_new_event_capacity_for_test();
            for row in &mut live.economy.entities {
                row.set_control_flags(row.control_flags & !0x20000);
            }
            assert!(
                construct_effect(&mut live.economy, &live.terrain, start, 0)
                    .unwrap()
                    .is_none()
            );
            let rng = live.economy.economy.global_rng;
            assert!(live.economy.allocate_smoke13(start).is_none());
            assert_eq!(
                live.economy.economy.global_rng,
                rng.wrapping_mul(9377).wrapping_add(9439),
                "smoke draws the source global stream before failed allocation"
            );
            exhausted.step_outer_turn(
                PlayerCommand {
                    opcode: 6,
                    argument_1: 25,
                    flags: 0x40,
                    ..PlayerCommand::default()
                },
                0,
            );
            for _ in 0..2 {
                exhausted.step_outer_turn(PlayerCommand::default(), 0);
            }
            no_errors(&exhausted);
            assert!(
                !exhausted
                    .entities()
                    .iter()
                    .any(|row| row.active && row.class == 9 && row.model == 30),
                "full nonreplaceable pool refuses the real flyer"
            );
            assert!(
                exhausted.shared_spells().player_entity.mana < mana,
                "failed source allocation still charges ordinary cast"
            );
            let holder = exhausted.shared_spells().pickup(slot).unwrap();
            assert!(
                holder.active_value > 0 && holder.active_value < holder.activation_value as i16,
                "null allocation retains source active countdown"
            );
        }
        session.step_outer_turn(
            PlayerCommand {
                opcode: 6,
                argument_1: 25,
                flags: 0x40,
                ..PlayerCommand::default()
            },
            0,
        );
        no_errors(&session);
        let mut projectile = None;
        let mut effect = None;
        let mut saw_hidden = false;
        let mut saw_smoke = false;
        let mut saw_visible_descriptor = false;
        for _ in 0..96 {
            let arena = &session.session.state.live.economy;
            for smoke in arena
                .model59_smoke_children
                .iter()
                .filter(|row| row.active && row.max_speed == 512)
            {
                let tile = arena.map_chains.tile_of(smoke.source_entity_index);
                assert_eq!(
                    session.canonical_terrain().planes().entity_indices[usize::from(tile)],
                    arena.map_chains.tile_head(tile) as i16,
                    "CaveIn Smoke13 publishes its live terrain cell head"
                );
            }
            saw_smoke |= session
                .session
                .state
                .live
                .economy
                .model59_smoke_children
                .iter()
                .any(|row| row.max_speed == 512);
            for row in session.entities() {
                if row.active && row.class == 9 && row.model == 30 {
                    projectile = Some(row.index);
                    assert_eq!(row.action, 31);
                    assert_eq!(row.particle_parameter_index, Some(211));
                    let dynamic = session
                        .economy()
                        .live_arena_slots()
                        .iter()
                        .any(|(slot, _)| *slot == row.index);
                    let sprite = session
                        .campaign_entity_presentations(0)
                        .unwrap()
                        .into_iter()
                        .find(|sprite| sprite.entity_index == row.index);
                    if row.drawing_disabled() {
                        assert!(
                            sprite.is_none(),
                            "hidden source flyer is excluded from presentation"
                        );
                    } else {
                        assert!(
                            !dynamic && row.control_flags & 0x21 == 0,
                            "visible CaveIn source flags={:#x}, dynamic={dynamic}, life={}",
                            row.control_flags,
                            row.life
                        );
                        assert!(
                            sprite
                                .as_ref()
                                .is_some_and(|sprite| sprite.render_parameter_index == 211),
                            "live flyer particle missing: flags={:#x}, dynamic={dynamic}, particle={:?}, life={}",
                            row.control_flags,
                            row.particle_parameter_index,
                            row.life
                        );
                        saw_visible_descriptor = true;
                    }
                }
                if row.active && row.class == 10 && row.model == 89 {
                    effect = Some(row.index);
                    assert_eq!(row.action, 96);
                    saw_hidden |= row.drawing_disabled();
                }
            }
            if saw_hidden && effect.is_some_and(|id| !session.entities()[usize::from(id)].active) {
                break;
            }
            session.step_outer_turn(PlayerCommand::default(), 0);
            no_errors(&session);
        }
        assert!(projectile.is_some(), "normal casting constructs9/30");
        assert!(
            saw_visible_descriptor,
            "ordinary CaveIn cast exposes a real live flyer descriptor"
        );
        let effect = effect.expect("ordinary flyer delivers10/89");
        assert!(
            saw_hidden && !session.entities()[usize::from(effect)].active,
            "source hides then following pass reclaims"
        );
        assert_ne!(
            session.canonical_terrain().planes().secondary_heights,
            before.secondary_heights,
            "actual CaveIn changes live second plane"
        );
        assert_ne!(
            session.canonical_terrain().planes().ground_heights,
            before.ground_heights,
            "source floor deformation is retained"
        );
        assert!(
            session.shared_spells().player_entity.mana < mana,
            "actual player upkeep consumes source cost"
        );
        assert!(session.session.state.live.core.effect_journal.records().iter().any(|record|matches!(record.effect,crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound { sound_id:15,entity_index,.. }) if entity_index == owner)));
        assert!(saw_smoke, "source smoke ring is constructed");
        let changed = before
            .secondary_heights
            .iter()
            .zip(
                session
                    .canonical_terrain()
                    .planes()
                    .secondary_heights
                    .iter(),
            )
            .position(|(a, b)| a != b)
            .unwrap();
        let probe = Position {
            x: ((changed & 255) as u16) << 8,
            y: ((changed >> 8) as u16) << 8,
            z: 0,
        };
        assert_eq!(
            session.canonical_terrain().ceiling_height(probe),
            Some(crate::legacy_terrain::interpolate_cave(
                &session.canonical_terrain().planes().secondary_heights,
                probe.x,
                probe.y
            ) as i16)
        );
        assert_ne!(
            session.canonical_terrain().ceiling_height(probe),
            Some(crate::legacy_terrain::interpolate_cave(
                &before.secondary_heights,
                probe.x,
                probe.y
            ) as i16),
            "real ceiling consumer observes changed geometry"
        );
    }
}
