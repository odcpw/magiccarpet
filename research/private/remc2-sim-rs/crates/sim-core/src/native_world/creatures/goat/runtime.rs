//! Goat's arena adapter. The scheduler supplies domains, never a whole session.

use super::stage::ExternalEffect;
use super::{
    GoatFamily, GoatPlayerTarget, GoatState, GoatStepEvent, SourceEntityState, relation_index,
    source_entity_states,
};
use crate::campaign::source_arena::SourceArena;
use crate::native_world::{
    EntityAuthority, EntitySnapshot, EntitySpecialRelation, ParticleParameter, RuntimeRelation,
    RuntimeSnapshot, SessionTerrain,
};
use crate::renderable_map_chain::RenderableMapSeed;
use crate::{FlightWorld, Position};

pub(crate) struct GoatRuntimeStep {
    pub(crate) events: Vec<GoatStepEvent>,
    pub(crate) spell_awards: Vec<(u16, u8)>,
}

impl GoatFamily {
    /// Import views of other authorities afresh for this callback. Own Goat
    /// reads always resolve to the family; these views carry no independent AI.
    fn refresh_views(&mut self, arena: &SourceArena) {
        if self.source_entities.len() != arena.entities.len() {
            self.source_entities = source_entity_states(&arena.entities);
        } else {
            for (view, source) in self.source_entities.iter_mut().zip(arena.entities.iter()) {
                *view = SourceEntityState::from_snapshot(source);
            }
        }
        let views = &mut self.source_entities;
        for source in arena.authored_class5() {
            let view = SourceEntityState {
                entity_index: source.source_entity_index,
                source_id: source.source_entity_index,
                class: 5,
                model: source.model,
                active: source.active,
                control_flags: if source.drawing_enabled { 8 } else { 0x408 },
                life: source.life,
                current_attacker: source.word_0x26 as i16,
                kill_credit_entity: source.word_0x24 as i16,
                action: source.action,
                position: source.position,
                child_relation: arena
                    .authored_class5_runtime()
                    .iter()
                    .find(|row| row.source_entity_index == source.source_entity_index)
                    .and_then(|row| relation_index(row.linked_head)),
                actual_speed: source.actual_speed,
                maximum_speed: source.maximum_speed,
                behavior_flags: 0,
                partner_entity_index: None,
                target_entity_index: relation_index(source.word_0x96_150),
                parent_entity_index: 0,
                yaw: source.yaw,
                roll: source.roll,
                half_width: source.shift_rot_x,
                stage_var_1: 0,
                stage_var_2: 0,
                stage_target: 0,
            };
            if let Some(old) = views.get_mut(usize::from(view.entity_index)) {
                *old = view;
            }
        }
    }

    pub(crate) fn bind_authored_route(
        &mut self,
        slot: u16,
        stage: u8,
        runtime: &mut RuntimeSnapshot,
    ) -> Result<(), String> {
        let index = self
            .goats
            .iter()
            .position(|goat| goat.active && goat.entity_index == slot)
            .ok_or_else(|| format!("Goat constructor lost source slot {slot}"))?;
        self.bind_stage(index, stage, runtime)
    }

    fn bind_stage(
        &mut self,
        index: usize,
        stage: u8,
        runtime: &mut RuntimeSnapshot,
    ) -> Result<(), String> {
        let Some(route) = runtime
            .stage_variables
            .get(usize::from(stage))
            .copied()
            .filter(|_| stage != 0)
        else {
            self.clear_stage(index);
            return Ok(());
        };
        let counter = (route.word_2 >> 8) as u8 & 3;
        if !runtime.advance_level2_stage_variable_counter(stage) {
            return Err(format!(
                "Goat StageVar {stage} has no lossless counter carrier"
            ));
        }
        let rejected = match route.flags as u8 & 0x60 {
            0x20 => counter == 3,
            0x40 => counter & 1 != 0,
            0x60 => counter != 0,
            _ => false,
        };
        if rejected {
            self.clear_stage(index);
            return Ok(());
        }
        let goat = &mut self.goats[index];
        goat.action = 15;
        goat.stage_var_1 = stage as i8;
        goat.stage_var_2 = route.kind;
        goat.stage_target = 0;
        if route.kind == 6 {
            if let RuntimeRelation::AxisScalar(value) = route.relation {
                goat.stage_target = value as u16;
            }
        }
        Ok(())
    }

    fn clear_stage(&mut self, index: usize) {
        self.goats[index].stage_var_1 = 0;
        self.goats[index].stage_var_2 = 0;
        self.goats[index].stage_target = 0;
        self.goats[index].action = 9;
    }

    /// sub_12780/sub_12500 run against frozen lists before proximity and AI.
    pub(crate) fn prepare_runtime_pass(
        &mut self,
        arena: &mut SourceArena,
        runtime: &mut RuntimeSnapshot,
        model_slots: &[Vec<u16>; 29],
    ) -> Result<(), String> {
        self.frozen_model_slots = Some(Box::new(model_slots.clone()));
        self.frozen_class3_slots = Some(
            arena
                .entities
                .iter()
                .filter(|actor| actor.active && actor.class == 3 && actor.life >= 0)
                .map(|actor| actor.index)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        self.refresh_views(arena);
        for index in 1..runtime.stage_variables.len() {
            let stage = runtime.stage_variables[index];
            if stage.kind == 7 && stage.flags as u8 & 0x18 != 0 {
                runtime.stage_variables[index].flags &= if stage.flags as u8 & 0x10 != 0 {
                    !0x10
                } else {
                    !0x08
                };
                runtime.bytes[392 + index * 12 + 1] = runtime.stage_variables[index].flags as u8;
            }
            if !matches!(stage.kind, 3 | 4 | 5 | 8 | 9) {
                continue;
            }
            let invalid = if stage.flags as u8 & 2 != 0 {
                match stage.relation {
                    RuntimeRelation::AxisScalar(model) => {
                        model_slots.get(model as usize).is_none_or(Vec::is_empty)
                    }
                    _ => false,
                }
            } else {
                match stage.relation {
                    RuntimeRelation::LiveEntity(slot) | RuntimeRelation::LiveEntityIndex(slot) => {
                        self.source_entity_view(slot)
                            .is_some_and(|actor| actor.life < 0 || actor.control_flags & 0x400 != 0)
                    }
                    _ => false,
                }
            };
            if invalid {
                runtime.stage_variables[index].flags |= 4;
                runtime.bytes[392 + index * 12 + 1] = runtime.stage_variables[index].flags as u8;
            }
        }
        for index in 0..self.goats.len() {
            if !self.goats[index].active || matches!(self.goats[index].action & 7, 4 | 5) {
                continue;
            }
            let stage_index = self.goats[index].stage_var_1 as u8;
            let attacking = matches!(self.goats[index].action & 7, 2 | 6);
            match self.goats[index].stage_var_2 {
                10 if !attacking => {
                    self.bind_stage(index, stage_index, runtime)?;
                    continue;
                }
                13 | 14 | 16 | 17 if !attacking => {
                    self.goats[index].action = 15;
                    continue;
                }
                15 if self.goats[index].action & 7 != 0 => {
                    self.bind_stage(index, stage_index, runtime)?;
                    continue;
                }
                10 | 13 | 14 | 15 | 16 | 17 => continue,
                _ => {}
            }
            let Some(stage) = runtime
                .stage_variables
                .get(usize::from(stage_index))
                .copied()
            else {
                return Err(format!(
                    "Goat slot {} has invalid StageVar {stage_index}",
                    self.goats[index].entity_index
                ));
            };
            let advance = match stage.kind {
                1 => {
                    if let RuntimeRelation::AxisScalar(point) = stage.relation {
                        self.goats[index]
                            .position
                            .x
                            .wrapping_sub(point as u16)
                            .cast_signed()
                            .unsigned_abs()
                            <= 2048
                            && self.goats[index]
                                .position
                                .y
                                .wrapping_sub((point >> 16) as u16)
                                .cast_signed()
                                .unsigned_abs()
                                <= 2048
                    } else {
                        false
                    }
                }
                3 | 4 | 5 | 8 | 9 => {
                    if stage.flags as u8 & 4 != 0 {
                        stage.kind != 3 || !attacking
                    } else {
                        if stage.flags as u8 & 2 != 0
                            && self.goats[index].stage_target != 0
                            && self
                                .source_entity_view(self.goats[index].stage_target)
                                .is_some_and(|actor| {
                                    !actor.active
                                        || actor.life < 0
                                        || actor.control_flags & 0x400 != 0
                                })
                        {
                            self.goats[index].stage_target = 0;
                        }
                        if stage.kind == 9 && stage.flags as u8 & 2 == 0 {
                            if let RuntimeRelation::AxisScalar(point) = stage.relation {
                                point != 0
                                    && self.goats[index]
                                        .position
                                        .x
                                        .wrapping_sub(point as u16)
                                        .cast_signed()
                                        .unsigned_abs()
                                        <= 3072
                                    && self.goats[index]
                                        .position
                                        .y
                                        .wrapping_sub((point >> 16) as u16)
                                        .cast_signed()
                                        .unsigned_abs()
                                        <= 3072
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                }
                6 => {
                    self.goats[index].stage_target = self.goats[index].stage_target.wrapping_sub(1);
                    self.goats[index].stage_target == 0
                }
                7 => stage.flags as u8 & 0x18 != 0,
                _ => false,
            };
            if advance {
                if stage.kind == 7 {
                    for (stage_index, route) in
                        runtime.stage_variables.iter_mut().enumerate().skip(1)
                    {
                        if matches!(route.kind, 3 | 4 | 5 | 8 | 9) && route.flags as u8 & 6 == 6 {
                            route.flags &= !4;
                            runtime.bytes[392 + stage_index * 12 + 1] = route.flags as u8;
                        }
                    }
                }
                self.bind_stage(index, stage.word_2 as u8, runtime)?;
            }
        }
        self.project_all(arena)
    }

    pub(crate) fn step_runtime_slot(
        &mut self,
        slot: u16,
        world: &SessionTerrain,
        players: &[GoatPlayerTarget],
        arena: &mut SourceArena,
        runtime: &RuntimeSnapshot,
        particles: &[ParticleParameter],
    ) -> Result<GoatRuntimeStep, String> {
        self.refresh_views(arena);
        let index = self
            .goats
            .iter()
            .position(|goat| goat.entity_index == slot && goat.active)
            .ok_or_else(|| format!("Goat callback lost source slot {slot}"))?;
        let players = players
            .iter()
            .map(|player| {
                self.source_entity_view(player.entity_index)
                    .map_or(*player, |actor| GoatPlayerTarget {
                        entity_index: player.entity_index,
                        position: actor.position,
                        control_flags: actor.control_flags,
                    })
            })
            .collect::<Vec<_>>();
        let before = self.goats[index].action;
        let mut effects = Vec::new();
        let events = if before == 15 {
            let mut events = Vec::new();
            self.step_stage(
                index,
                world,
                &players,
                &runtime.stage_variables,
                &mut effects,
                &mut events,
            );
            if self.goats[index].control_flags & 0x400 != 0 {
                events.push(GoatStepEvent::RemovalQueued { entity_index: slot });
            }
            self.advance_action_15_tail(index, &mut events);
            self.goats[index].phase = self.goats[index].phase.wrapping_add(1);
            events
        } else {
            self.step_source_slot(slot, world, &players)
        };
        if before == 12 {
            let mut child = self.child_links[index];
            let mut visited = std::collections::BTreeSet::new();
            while let Some(slot) = child {
                if !visited.insert(slot) || slot == self.goats[index].entity_index {
                    return Err("cyclic Goat PreKill child chain".to_owned());
                }
                let Some(view) = self.source_entity_view(slot) else {
                    return Err(format!("Goat PreKill child {slot} is absent"));
                };
                if view.kill_credit_entity != 0 {
                    self.goats[index].kill_credit_entity = view.kill_credit_entity;
                }
                child = view.child_relation;
                if let Some(goat) = self.goats.iter_mut().find(|goat| goat.entity_index == slot) {
                    goat.action = 13;
                } else if let Some(raw) = arena
                    .entities
                    .get_mut(usize::from(slot))
                    .filter(|raw| raw.active)
                {
                    raw.set_source_action(13);
                }
            }
            let killer = self.goats[index].kill_credit_entity.cast_unsigned();
            if self.source_entity_view(killer).is_some_and(|actor| {
                crate::combat::actors::awards_creature_kill(
                    1,
                    self.goats[index].source_id,
                    actor.source_id,
                    actor.class,
                    actor.model,
                )
            }) {
                arena.record_authored_creature_kill(killer);
            }
        }
        self.project_all(arena)?;
        let mut spell_awards = Vec::new();
        for effect in effects {
            match effect {
                ExternalEffect::Expire { slot, summoned } => {
                    let goat = self
                        .goats
                        .iter()
                        .find(|goat| goat.entity_index == slot)
                        .expect("expiry owner exists");
                    if summoned {
                        spawn_expiry_visual(arena, world, goat.position, particles)?;
                    }
                    let _ = arena.allocate_class10_model0_effect(
                        0,
                        0,
                        0,
                        0,
                        goat.position,
                        400,
                        0,
                        0x20000,
                        128,
                        128,
                    );
                }
                ExternalEffect::OwnerFeedback { owner, selector } => {
                    spell_awards.push((owner, selector))
                }
            }
        }
        Ok(GoatRuntimeStep {
            events,
            spell_awards,
        })
    }

    fn project_all(&self, arena: &mut SourceArena) -> Result<(), String> {
        for goat in &self.goats {
            // Inactive rows may already have been reclaimed/reused. Never
            // resurrect an earlier occupant while projecting a new callback.
            if !goat.active {
                continue;
            }
            if !arena.sync_authored_goat_projection(goat) {
                return Err(format!("Goat {} map projection failed", goat.entity_index));
            }
            if arena
                .authored_class5()
                .iter()
                .any(|row| row.source_entity_index == goat.entity_index)
            {
                continue;
            }
            if let Some(source) = arena
                .entities
                .get_mut(usize::from(goat.entity_index))
                .filter(|source| source.active && source.class == 5 && source.model == 1)
            {
                project_imported(source, goat);
                arena
                    .map_chains
                    .sync_entity(RenderableMapSeed {
                        entity_index: goat.entity_index,
                        active: goat.active,
                        x: goat.position.x,
                        y: goat.position.y,
                    })
                    .map_err(|error| {
                        format!(
                            "Goat {} imported map projection failed: {error}",
                            goat.entity_index
                        )
                    })?;
            }
        }
        Ok(())
    }
}

fn project_imported(source: &mut EntitySnapshot, goat: &GoatState) {
    source.set_position(goat.position);
    source.set_yaw(goat.yaw);
    source.set_pitch(goat.pitch);
    source.set_source_roll(goat.roll);
    source.set_entity_rng(goat.rng);
    source.set_actual_speed(goat.actual_speed);
    source.set_life(goat.life);
    source.set_control_flags(goat.control_flags);
    source.set_source_action(goat.action);
    source.set_phase(goat.phase);
    source.set_mana(goat.mana);
    source.set_damage_result_words(goat.kill_credit_entity as u16, goat.current_attacker as u16);
    source.record[54..56].copy_from_slice(&goat.partner_entity_index.unwrap_or(0).to_le_bytes());
    source.record[61] = goat.proximity_countdown as u8;
    source.record[62] = goat.proximity_subcounter as u8;
    source.record[76] = goat.stage_var_1 as u8;
    source.record[77] = goat.stage_var_2 as u8;
    source.record[78..80].copy_from_slice(&goat.stage_target.to_le_bytes());
    source.record[98..102].copy_from_slice(&goat.pending_damage.to_le_bytes());
    source.record[102..104].copy_from_slice(&goat.pending_attacker.to_le_bytes());
    source.record[154..156].copy_from_slice(&goat.target_entity_index.unwrap_or(0).to_le_bytes());
    source.record[44..46].copy_from_slice(&goat.parent_entity_index.to_le_bytes());
    source.record[50..52].copy_from_slice(&goat.lifetime.to_le_bytes());
    source.record[52..54].copy_from_slice(&goat.initial_lifetime.to_le_bytes());
}

/// sub_51A00 constructs model 73 at action 80 (sub_3A630), not action 73.
pub(crate) fn spawn_expiry_visual(
    arena: &mut SourceArena,
    world: &SessionTerrain,
    position: Position,
    particles: &[ParticleParameter],
) -> Result<(), String> {
    let particle = particles
        .get(220)
        .ok_or_else(|| "Goat expiry requires particle descriptor 220".to_owned())?;
    let Some(slot) = arena.allocate_slot() else {
        return Ok(());
    };
    let seed = arena.new_event_rng(slot);
    let mut source = arena.entities[usize::from(slot)].clone();
    source.record.fill(0);
    source.index = slot;
    source.source_id = slot;
    source.active = true;
    source.authority = EntityAuthority::FrozenObserved;
    source.class = 10;
    source.model = 73;
    source.next_relation = None;
    source.action_marker = 0;
    source.level_gate = 0;
    source.byte_3b = 0;
    source.byte_3c = 0;
    source.font_type_index = 0;
    source.active_value = 0;
    source.activation_value = 0;
    source.mana_regen = 0;
    source.max_mana = 0;
    source.mana_cost = 0;
    source.mana = 0;
    source.subspell_index = 100;
    source.actual_speed = 16;
    source.parameter_relation = Some(60);
    source.special_relation = EntitySpecialRelation::GlobalSpecialState;
    source.particle_parameter_index = Some(220);
    source.record[0..2].copy_from_slice(&slot.to_le_bytes());
    source.record[2] = 1;
    source.record[30..32].copy_from_slice(&slot.to_le_bytes());
    source.record[46..48].copy_from_slice(&100_u16.to_le_bytes());
    source.record[69] = 255;
    source.record[70] = 255;
    source.record[71] = 10;
    source.record[61] = (-6_i8) as u8;
    source.record[164..168].copy_from_slice(&60_u32.to_le_bytes());
    source.record[67] = 10;
    source.record[68] = 73;
    source.record[94..96].copy_from_slice(&220_u16.to_le_bytes());
    source.set_entity_rng(seed);
    source.set_phase(slot as u8);
    source.set_source_action(80);
    source.set_max_life(16);
    source.set_life(16);
    source.set_control_flags(0);
    source.set_position(Position {
        z: world.ground_height(position),
        ..position
    });
    let yaw = (particle.rotation_speed / 2) as i16;
    let pitch = (particle.speed / 2) as i16;
    source.set_yaw(0);
    source.set_pitch(0);
    source.set_source_roll(0);
    source.set_fov(0);
    source.record[86..88].copy_from_slice(&yaw.to_le_bytes());
    source.record[88..90].copy_from_slice(&pitch.to_le_bytes());
    source.record[90..92].copy_from_slice(&pitch.to_le_bytes());
    source.record[92..94].copy_from_slice(&yaw.to_le_bytes());
    if let Err(error) = arena.map_chains.sync_entity(RenderableMapSeed {
        entity_index: slot,
        active: true,
        x: source.position.x,
        y: source.position.y,
    }) {
        arena.release_slot(slot, Some(seed));
        return Err(format!("Goat expiry map link failed: {error}"));
    }
    arena.source_rng.insert(slot, seed);
    arena.entities[usize::from(slot)] = source;
    Ok(())
}

pub(crate) fn step_expiry_visual(source: &mut EntitySnapshot) -> Result<(), String> {
    if source.class != 10 || source.model != 73 || source.action != 80 {
        return Err("invalid Goat expiry visual selector".to_owned());
    }
    source.set_life(source.life.wrapping_sub(1));
    if source.life <= 0 {
        source.disable_drawing();
    }
    Ok(())
}
