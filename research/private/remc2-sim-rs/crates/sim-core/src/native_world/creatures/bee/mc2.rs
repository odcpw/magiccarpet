//! MC2 arena/StageVar binding for the canonical Bee owner.

use super::{
    BeeActionContext, BeeActor, BeeCreatureOrigin, BeeCreatureRuntime, BeeEffects,
    BeeStageDescriptor, BeeStageRuntime,
};
use crate::campaign::source_arena::SourceArena;
use crate::native_world::{
    Level2EconomyEvent, MapType, ParticleParameter, RuntimeRelation, RuntimeSnapshot,
    SessionTerrain, level2_class5,
};
use crate::{Position, renderable_map_chain::RenderableMapSeed};

pub(crate) fn allocate_authored(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    authored_index: u16,
    mut position: Position,
    max_life: i32,
) -> Option<BeeCreatureRuntime> {
    use crate::FlightWorld;
    if terrain.map_type() != MapType::Day || max_life <= 0 {
        return None;
    }
    position.z = terrain.ground_height(position);
    let slot = arena.allocate_source_slot_with_terrain(terrain)?;
    let source = arena.economy.initialize_bee_creature_at_slot(
        slot,
        BeeCreatureOrigin::AuthoredRecord { authored_index },
        position,
        0,
        0,
        17,
        0,
        0,
        None,
        max_life,
        terrain,
    )?;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)
        .expect("Bee initializer already validated this source map slot");
    Some(source)
}

pub(crate) fn allocate_morph(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    holder_slot: u16,
    parent_slot: u16,
    parent_id: u16,
    position: Position,
) -> Option<BeeCreatureRuntime> {
    if terrain.map_type() != MapType::Day {
        return None;
    }
    let slot = arena.allocate_source_slot_with_terrain(terrain)?;
    let source = arena.economy.initialize_bee_creature_at_slot(
        slot,
        BeeCreatureOrigin::MorphSpell { holder_slot },
        position,
        parent_slot,
        parent_id,
        23,
        0,
        12,
        None,
        crate::level_definition::BEE_CLASS_PROPERTIES.max_life,
        terrain,
    )?;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)
        .expect("Bee initializer already validated this source map slot");
    Some(source)
}

impl BeeStageRuntime for RuntimeSnapshot {
    fn descriptor(&self, index: u8) -> Option<BeeStageDescriptor> {
        let row = self
            .stage_variables
            .get(usize::from(index))
            .filter(|_| index != 0)?;
        let scalar = match row.relation {
            RuntimeRelation::AxisScalar(value) => value,
            _ => 0,
        };
        let actor = match row.relation {
            RuntimeRelation::LiveEntity(slot) | RuntimeRelation::LiveEntityIndex(slot) => slot,
            _ => 0,
        };
        Some(BeeStageDescriptor {
            kind: row.kind as u8,
            flags: row.flags as u8,
            next: row.word_2 as u8,
            point: Position {
                x: scalar as u16,
                y: (scalar >> 16) as u16,
                z: 0,
            },
            actor,
            model: scalar as u8,
            scalar: scalar as u16,
        })
    }

    fn bind(&mut self, index: u8) -> Option<BeeStageDescriptor> {
        let descriptor = self.descriptor(index)?;
        let phase = (self.stage_variables[usize::from(index)].word_2 >> 8) as u8 & 3;
        if !self.advance_level2_stage_variable_counter(index) {
            return None;
        }
        let rejected = match descriptor.flags & 0x60 {
            0x20 => phase == 3,
            0x40 => phase & 1 != 0,
            0x60 => phase != 0,
            _ => false,
        };
        (!rejected).then_some(descriptor)
    }

    fn rearm_model_relations(&mut self) {
        for (index, row) in self.stage_variables.iter_mut().enumerate().skip(1) {
            if matches!(row.kind, 3 | 4 | 5 | 8 | 9) && row.flags as u8 & 6 == 6 {
                row.flags = (row.flags as u8 & !4) as i8;
                self.bytes[392 + index * 12 + 1] = row.flags as u8;
            }
        }
    }
}

/// Preserve captured source state exactly; this is not a second constructor.
/// The slot, RNG, incarnation and existing map membership are unchanged.
pub(crate) fn adopt_imported_bees(arena: &mut SourceArena) {
    let imported = arena
        .entities
        .iter()
        .filter(|row| row.active && row.class == 5 && row.model == 2)
        .filter(|row| {
            !arena
                .bee_creatures()
                .iter()
                .any(|bee| bee.source_entity_index == row.index)
        })
        .cloned()
        .collect::<Vec<_>>();
    for row in imported {
        let record = row.record_bytes();
        let word = |offset| u16::from_le_bytes([record[offset], record[offset + 1]]);
        let dword = |offset| {
            i32::from_le_bytes([
                record[offset],
                record[offset + 1],
                record[offset + 2],
                record[offset + 3],
            ])
        };
        let source = BeeCreatureRuntime {
            source_entity_index: row.index,
            entity_id: row.source_id,
            entity_rng: row.entity_rng,
            origin: BeeCreatureOrigin::InitialPackage,
            parent_entity_index: word(44),
            source_class: row.class,
            model: row.model,
            action: row.action,
            minimum_speed: row.minimum_speed(),
            maximum_speed: word(138) as i16,
            actual_speed: row.actual_speed,
            control_flags: row.control_flags,
            mana: row.mana,
            max_life: row.max_life,
            life: row.life,
            pending_damage: dword(98),
            pending_attacker_entity_index: word(102),
            damage_attacker_entity_index: row.word_26(),
            kill_credit_entity_index: row.word_24(),
            subspell_index: row.subspell_index,
            dword_0x10: row.source_dword_10(),
            target_entity_index: Some(row.source_word_96()).filter(|slot| *slot != 0),
            yaw: row.yaw,
            pitch: row.pitch,
            roll: row.source_roll(),
            source_byte_3e: row.phase,
            source_byte_39: i16::from(record[61]),
            vertical_step: crate::level_definition::BEE_CLASS_PROPERTIES.descent,
            action_window: i16::from(crate::level_definition::BEE_CLASS_PROPERTIES.scan_period),
            stage_var1: record[76],
            stage_var2: record[77],
            stage_target: None,
            last_sound_id: 0,
            linked_peer_entity_index: row.word_32(),
            linked_child_entity_index: row.word_34(),
            stage_relation: word(78),
            stage_remaining: word(50) as i16,
            stage_duration: word(52) as i16,
            proximity_delay: record[62],
            position: row.position,
            particle_index: word(94),
            shift_rot_x: row.array_pitch() as u16,
            shift_rot_y: row.array_fov() as u16,
            collision_vertical_offset: row.array_yaw(),
            phase: row.phase,
            drawing_enabled: !row.drawing_disabled(),
            active: true,
        };
        arena.economy.source_rng.insert(row.index, row.entity_rng);
        arena.economy.bee_creatures.push(source);
        let old = &mut arena.entities[usize::from(row.index)];
        old.active = false;
        old.record[2] = 0;
    }
}

/// Resolve current occupants, then order by each frozen source membership.
/// The retained reset record cannot shadow a newer typed occupant at its slot.
pub(crate) fn actor_rows(
    arena: &SourceArena,
    frozen_models: &[Vec<u16>; 29],
    frozen_players: &[u16],
    local: Option<(u16, Position, i16, i16)>,
) -> Vec<BeeActor> {
    let relations = level2_class5::archer::relations_in_arena(arena);
    let mut current = std::collections::BTreeMap::new();
    for relation in relations {
        let target = relation.target;
        let raw = arena.entities.get(usize::from(relation.slot));
        let mut row = BeeActor {
            slot: relation.slot,
            source_id: relation.source_id,
            class: target.class,
            model: target.model,
            action: target.action,
            position: target.position,
            life: target.life,
            control_flags: target.control_flags,
            target: target.word_0x96_150,
            attacker: relation.attacker,
            killer: raw.map_or(0, |raw| raw.word_24()),
            peer: target.word_0x32,
            child: raw.map_or(0, |raw| raw.word_34()),
            parent: relation.parent,
            stage_index: relation.stage_index,
            stage_mode: relation.stage_mode,
            stage_relation: relation.stage_relation,
            yaw: relation.yaw,
            roll: relation.roll,
            actual_speed: target.actual_speed,
            maximum_speed: target.maximum_speed,
            half_width: relation.half_width,
            height_offset: raw.map_or(0, |raw| raw.array_yaw()),
            frozen_class3_member: frozen_players.contains(&relation.slot),
            frozen_class5_member: target.class == 5
                && frozen_models
                    .get(usize::from(target.model))
                    .is_some_and(|slots| slots.contains(&relation.slot)),
            frozen_family_member: target.class == 5
                && target.model == 2
                && frozen_models[2].contains(&relation.slot),
        };
        if let Some(source) = arena
            .bee_creatures()
            .iter()
            .find(|source| source.active && source.source_entity_index == relation.slot)
        {
            row = source.projection(frozen_models[2].contains(&relation.slot));
        } else if let Some(state) = arena
            .authored_class5_archer()
            .iter()
            .find(|state| state.source_entity_index == relation.slot)
        {
            row.child = state.word_0x34;
        }
        if let Some((slot, position, yaw, roll)) = local
            && row.slot == slot
        {
            row.position = position;
            row.yaw = yaw;
            row.roll = roll;
        }
        current.insert(row.slot, row);
    }
    let mut rows = Vec::with_capacity(current.len());
    for slot in frozen_players.iter().chain(frozen_models.iter().flatten()) {
        if let Some(row) = current.remove(slot) {
            rows.push(row);
        }
    }
    rows.extend(current.into_values());
    rows
}

pub(crate) fn stage_prepass_in_arena(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    frozen_models: &[Vec<u16>; 29],
    frozen_players: &[u16],
    local: Option<(u16, Position, i16, i16)>,
) {
    adopt_imported_bees(arena);
    let rows = actor_rows(arena, frozen_models, frozen_players, local);
    for slot in &frozen_models[2] {
        if let Some(source) = arena
            .economy
            .bee_creatures
            .iter_mut()
            .find(|source| source.active && source.source_entity_index == *slot)
        {
            super::stage_prepass(source, runtime, &rows);
        }
    }
}

pub(crate) fn proximity_prepass_in_arena(
    arena: &mut SourceArena,
    frozen: &[u16],
    player: Position,
) {
    for slot in frozen {
        let Some(index) = arena
            .economy
            .bee_creatures
            .iter()
            .position(|source| source.active && source.source_entity_index == *slot)
        else {
            continue;
        };
        let child_value = super::proximity_prepass(&mut arena.economy.bee_creatures[index], player);
        let mut child = arena.economy.bee_creatures[index].linked_child_entity_index;
        if let Some(value) = child_value {
            for _ in 0..arena.economy.bee_creatures.len() {
                let Some(row) = arena
                    .economy
                    .bee_creatures
                    .iter_mut()
                    .find(|source| source.active && source.source_entity_index == child)
                else {
                    break;
                };
                row.source_byte_39 = i16::from(value);
                child = row.linked_child_entity_index;
            }
        }
    }
}

pub(crate) fn bind_authored_route(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slot: u16,
    index: u8,
) {
    if let Some(source) = arena
        .economy
        .bee_creatures
        .iter_mut()
        .find(|source| source.active && source.source_entity_index == slot)
    {
        super::bind_stage(source, index, runtime);
    }
}

/// Allocations and map writes remain on the one arena. Contact and player XP
/// are returned for composition with their existing authoritative domains.
#[allow(clippy::too_many_arguments)]
pub(crate) fn step_in_arena(
    arena: &mut SourceArena,
    runtime: &RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    frozen_models: &[Vec<u16>; 29],
    frozen_players: &[u16],
    local: Option<(u16, Position, i16, i16)>,
) -> Result<BeeEffects, String> {
    let rows = actor_rows(arena, frozen_models, frozen_players, local);
    let index = arena
        .economy
        .bee_creatures
        .iter()
        .position(|source| source.active && source.source_entity_index == slot)
        .ok_or_else(|| format!("Bee source slot {slot} is absent"))?;
    let before = arena.economy.bee_creatures[index];
    let stage = runtime.descriptor(before.stage_var1);
    let effects = super::step(
        &mut arena.economy.bee_creatures[index],
        &BeeActionContext {
            terrain,
            actors: &rows,
            stage,
        },
    );
    let after = arena.economy.bee_creatures[index];
    if !effects.callback_executed {
        return Ok(effects);
    }
    arena.economy.source_rng.insert(slot, after.entity_rng);
    if let Some(hit) = effects.damage {
        arena
            .economy
            .events
            .push(Level2EconomyEvent::DamageApplied {
                target_entity_index: after.entity_id,
                attacker_entity_index: hit.attacker,
                damage: hit.damage,
                lethal: after.life < 0,
            });
    }
    if let Some(update) = effects.peer_update
        && let Some(peer) = arena
            .economy
            .bee_creatures
            .iter_mut()
            .find(|peer| peer.active && peer.source_entity_index == update.slot)
    {
        peer.action = update.action;
        peer.target_entity_index = Some(update.target).filter(|slot| *slot != 0);
        peer.linked_peer_entity_index = update.peer;
    }
    if effects.kill_linked_children {
        let mut child = before.linked_child_entity_index;
        for _ in 0..arena.economy.bee_creatures.len() {
            let Some(row) = arena
                .economy
                .bee_creatures
                .iter_mut()
                .find(|source| source.active && source.source_entity_index == child)
            else {
                break;
            };
            row.action = 21;
            child = row.linked_child_entity_index;
        }
    }
    if let Some(killer) = effects.kill_credit
        && rows.iter().any(|row| {
            row.slot == killer
                && row.class == 3
                && row.model == 0
                && row.source_id != after.entity_id
        })
    {
        arena.economy.record_authored_creature_kill(killer);
    }
    if effects.population_decrement {
        arena.economy.decrement_source_creature_population();
    }
    if effects.death_conversion {
        let _ = arena.economy.spawn_mana_sphere_from_death(
            slot,
            after.position,
            after.yaw,
            after.entity_rng,
            after.mana,
            arena.economy.source_owner(slot),
            terrain,
        );
        if after.control_flags & 0x0010_0000 == 0 {
            let _ = arena.economy.spawn_death_burst_from_death_with_id(
                slot,
                after.entity_id,
                after.position,
            );
        }
    }
    if effects.expire_effect {
        if effects.summon_expire_effect {
            crate::native_world::creatures::goat::runtime::spawn_expiry_visual(
                arena,
                terrain,
                after.position,
                particles,
            )?;
        }
        if let Some(effect_slot) = arena.allocate_source_slot_with_terrain(terrain)
            && arena
                .economy
                .initialize_class10_model0_effect_at_slot(
                    effect_slot,
                    0,
                    effect_slot,
                    0,
                    0,
                    after.position,
                    400,
                    0,
                    0x20000,
                    128,
                    128,
                )
                .is_some()
        {
            crate::world::effects::sync_position(
                arena,
                terrain,
                effect_slot,
                after.position,
                true,
                false,
            )?;
        }
    }
    // CopyEntityPosition still publishes a moved row hidden earlier in this
    // pass. Drawing disable defers unlink; it does not suppress movement.
    crate::world::effects::sync_position(arena, terrain, slot, after.position, true, false)?;
    if !after.drawing_enabled {
        arena.economy.deferred_bee_reclaims.insert(slot);
    }
    Ok(effects)
}

// Direct callback seams used by existing source-kernel regression checks.
// Shipping callbacks use step_in_arena with current authoritative targets.
#[cfg(test)]
impl crate::native_world::Level2EconomyState {
    pub fn acquire_authored_bee_target(
        &mut self,
        slot: u16,
        candidates: &[(u16, Position, u32)],
        terrain: &SessionTerrain,
    ) -> Option<u16> {
        let source = self
            .bee_creatures
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)?;
        if source.action != 17 {
            return None;
        }
        let actors = candidates
            .iter()
            .map(|&(slot, position, flags)| {
                let mut row = source.projection(false);
                row.slot = slot;
                row.source_id = slot;
                row.class = 3;
                row.model = 0;
                row.position = position;
                row.control_flags = flags;
                row.frozen_class3_member = true;
                row
            })
            .collect::<Vec<_>>();
        let mut effects = BeeEffects::default();
        super::action17(
            source,
            &BeeActionContext {
                terrain,
                actors: &actors,
                stage: None,
            },
            &mut effects,
        );
        self.source_rng.insert(slot, source.entity_rng);
        (source.action == 18)
            .then_some(source.target_entity_index)
            .flatten()
    }

    pub fn step_authored_bee_slot(
        &mut self,
        slot: u16,
        target: Option<Position>,
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(index) = self
            .bee_creatures
            .iter()
            .position(|row| row.active && row.source_entity_index == slot)
        else {
            return false;
        };
        let before = self.bee_creatures[index];
        let mut actors = self
            .bee_creatures
            .iter()
            .filter(|row| row.active)
            .map(|row| row.projection(row.life >= 0))
            .collect::<Vec<_>>();
        if let (Some(target_slot), Some(position)) = (before.target_entity_index, target) {
            actors.retain(|row| row.slot != target_slot);
            let mut row = before.projection(false);
            row.slot = target_slot;
            row.source_id = target_slot;
            row.class = 3;
            row.model = 0;
            row.position = position;
            row.life = 1;
            row.control_flags = 8;
            row.height_offset = 0;
            row.frozen_class3_member = true;
            actors.push(row);
        }
        let stage = before.stage_target.map(|point| BeeStageDescriptor {
            kind: before.stage_var2,
            flags: 0,
            next: 0,
            point,
            actor: 0,
            model: 0,
            scalar: 0,
        });
        let effects = super::step(
            &mut self.bee_creatures[index],
            &BeeActionContext {
                terrain,
                actors: &actors,
                stage,
            },
        );
        let after = self.bee_creatures[index];
        if !effects.callback_executed {
            return false;
        }
        self.source_rng.insert(slot, after.entity_rng);
        if let Some(hit) = effects.damage {
            self.events.push(Level2EconomyEvent::DamageApplied {
                target_entity_index: after.entity_id,
                attacker_entity_index: hit.attacker,
                damage: hit.damage,
                lethal: after.life < 0,
            });
        }
        if let Some(update) = effects.peer_update
            && let Some(peer) = self
                .bee_creatures
                .iter_mut()
                .find(|row| row.active && row.source_entity_index == update.slot)
        {
            peer.action = update.action;
            peer.target_entity_index = Some(update.target).filter(|slot| *slot != 0);
            peer.linked_peer_entity_index = update.peer;
        }
        if effects.death_conversion {
            let mut dying = before;
            self.step_authored_bee_death_tail(&mut dying, terrain);
        }
        if !after.drawing_enabled {
            self.deferred_bee_reclaims.insert(slot);
            return true;
        }
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: true,
                x: after.position.x,
                y: after.position.y,
            })
            .is_ok()
    }
}
