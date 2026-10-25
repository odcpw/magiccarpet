//! Lossless model-22 views over the shared source arena. The family never owns
//! a second live row; every ordered kernel write commits to the same carrier
//! read by collision, targeting, presentation, replay and following-pass reclaim.

use super::{Actor, Allocation, PlayerView, Runtime, StageDescriptor, StageState, Target};
use crate::campaign::source_arena::{SourceArena, SourceEntityHandle, SourceEntityView};
use crate::native_world::creatures::archer::ArcherStageRuntime;
use crate::native_world::level2_class5::archer::relations_in_arena;
use crate::native_world::{
    EntitySnapshot, ParticleParameter, PlayerSnapshot, RuntimeSnapshot, SessionTerrain,
};
use crate::{FlightWorld, Position};

fn word(row: &EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}

fn dword(row: &EntitySnapshot, offset: usize) -> i32 {
    i32::from_le_bytes(
        row.record[offset..offset + 4]
            .try_into()
            .expect("fixed entity record"),
    )
}

/// Resolve each link against the current generation, then retain that handle
/// for the duration of this callback. A later allocation cannot silently
/// redirect an already-read handle to its replacement occupant.
pub(crate) fn read_actor(arena: &SourceArena, handle: SourceEntityHandle) -> Option<Actor> {
    let SourceEntityView::Imported(row) = arena.resolve(handle)? else {
        return None;
    };
    if row.class != 5 || row.model != super::SOURCE_MODEL || row.record.len() < 176 {
        return None;
    }
    let link = |slot| (slot != 0).then(|| arena.handle(slot)).flatten();
    let target = word(row, 154);
    Some(Actor {
        handle,
        source_id: row.source_id,
        entity_rng: row.entity_rng,
        control_flags: row.control_flags,
        action: row.action,
        position: row.position,
        yaw: row.yaw,
        pitch: row.pitch,
        roll: word(row, 36) as i16,
        speed: row.actual_speed,
        minimum_speed: word(row, 136) as i16,
        maximum_speed: word(row, 138) as i16,
        life: row.life,
        max_life: row.max_life,
        mana: row.mana,
        max_mana: row.max_mana,
        phase: row.phase,
        cadence_counter: row.record[61],
        animation_frame: row.record[96],
        length_or_ordinal: row.record[74] as i8,
        rotation: word(row, 48) as i16,
        subspell_index: row.subspell_index,
        oscillation_flags: word(row, 58),
        dword10: row.source_dword_10(),
        root: (row.action == 180).then(|| link(target)).flatten(),
        previous: link(word(row, 54)),
        next: link(word(row, 56)),
        target_or_growth: target,
        owner: word(row, 152),
        damage: dword(row, 98),
        hit_source: word(row, 102),
        possessor: word(row, 108),
        spell_amount: dword(row, 104),
        attacker: word(row, 42),
        visual_index: row.particle_parameter_index.unwrap_or(0),
        half_width: word(row, 88) as i16,
        height: word(row, 92) as i16,
        visual_radius: word(row, 86) as i16,
        climb_hold: word(row, 40) as i16,
        stage: StageState {
            index: row.record[76],
            mode: row.record[77],
            relation: word(row, 78),
            parent: word(row, 44),
            remaining: word(row, 50) as i16,
            duration: word(row, 52) as i16,
        },
    })
}

fn write_word(row: &mut EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_dword(row: &mut EntitySnapshot, offset: usize, value: i32) {
    row.record[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

/// Accepts a fresh NewEvent class-zero row as well as an existing model-22
/// occupant, but never a stale handle or a different live family.
pub(crate) fn write_actor(arena: &mut SourceArena, actor: Actor) -> bool {
    let slot = actor.handle.source_slot;
    if arena.slot_generation(slot) != actor.handle.generation {
        return false;
    }
    // sub_274C0 copies the complete predecessor, including fields not
    // interpreted by this family, before overwriting the child's own links.
    let predecessor = if arena
        .entities
        .get(usize::from(slot))
        .is_some_and(|row| row.active && row.class == 0)
        && actor.action == 180
    {
        actor
            .previous
            .and_then(|handle| match arena.resolve(handle) {
                Some(SourceEntityView::Imported(row))
                    if row.class == 5 && row.model == super::SOURCE_MODEL =>
                {
                    Some(row.clone())
                }
                _ => None,
            })
    } else {
        None
    };
    let Some(row) = arena.entities.get_mut(usize::from(slot)) else {
        return false;
    };
    if !row.active || (row.class != 0 && (row.class != 5 || row.model != super::SOURCE_MODEL)) {
        return false;
    }
    if let Some(predecessor) = predecessor {
        *row = predecessor;
        row.index = slot;
        row.record[0..2].copy_from_slice(&slot.to_le_bytes());
        row.next_relation = None;
    }
    row.class = 5;
    row.model = super::SOURCE_MODEL;
    row.record[67] = 5;
    row.record[68] = super::SOURCE_MODEL;
    row.source_id = actor.source_id;
    write_word(row, 30, actor.source_id);
    row.set_control_flags(actor.control_flags);
    row.set_source_action(actor.action);
    row.set_position(actor.position);
    row.set_yaw(actor.yaw);
    row.set_pitch(actor.pitch);
    row.set_source_roll(actor.roll);
    row.set_entity_rng(actor.entity_rng);
    row.set_actual_speed(actor.speed);
    row.set_max_life(actor.max_life);
    row.set_life(actor.life);
    row.set_mana(actor.mana);
    row.max_mana = actor.max_mana;
    row.mana_cost = actor.max_mana;
    write_dword(row, 144, actor.max_mana);
    row.set_phase(actor.phase);
    row.set_subspell_index(actor.subspell_index);
    row.set_source_dword_10(actor.dword10);
    row.set_source_byte_46(actor.length_or_ordinal as u8);
    row.record[60] = 3;
    row.record[61] = actor.cadence_counter;
    row.record[69] = 3;
    row.record[96] = actor.animation_frame;
    row.record[76] = actor.stage.index;
    row.record[77] = actor.stage.mode;
    write_word(row, 136, actor.minimum_speed as u16);
    write_word(row, 138, actor.maximum_speed as u16);
    write_word(row, 48, actor.rotation as u16);
    write_word(row, 58, actor.oscillation_flags);
    write_word(row, 54, actor.previous.map_or(0, |h| h.source_slot));
    write_word(row, 56, actor.next.map_or(0, |h| h.source_slot));
    write_word(row, 154, actor.target_or_growth);
    write_word(row, 152, actor.owner);
    write_dword(row, 98, actor.damage);
    write_word(row, 102, actor.hit_source);
    write_dword(row, 104, actor.spell_amount);
    write_word(row, 108, actor.possessor);
    write_word(row, 42, actor.attacker);
    write_word(row, 40, actor.climb_hold as u16);
    write_word(row, 78, actor.stage.relation);
    write_word(row, 44, actor.stage.parent);
    write_word(row, 50, actor.stage.remaining as u16);
    write_word(row, 52, actor.stage.duration as u16);
    row.particle_parameter_index = Some(actor.visual_index);
    write_word(row, 94, actor.visual_index);
    write_word(row, 88, actor.half_width as u16);
    write_word(row, 92, actor.height as u16);
    write_word(row, 90, actor.half_width as u16);
    write_word(row, 86, actor.visual_radius as u16);
    row.parameter_relation = Some(super::SOURCE_DESCRIPTOR);
    write_dword(row, 164, i32::from(super::SOURCE_DESCRIPTOR) + 1);
    arena.economy.source_rng.insert(slot, actor.entity_rng);
    true
}

/// Presentation and player notices have no effect on allocation or actor
/// ordering; the scheduler consumes these through its existing journals.
#[derive(Default)]
pub(crate) struct Outcome {
    pub callback_executed: bool,
    pub sounds: Vec<(u16, u8)>,
    pub feedback: Vec<(u16, u8)>,
}

pub(crate) struct Context<'a> {
    pub arena: &'a mut SourceArena,
    pub runtime: &'a mut RuntimeSnapshot,
    pub terrain: &'a mut SessionTerrain,
    pub particles: &'a [ParticleParameter],
    pub players: &'a [PlayerSnapshot],
    pub families: &'a [Vec<u16>; 29],
    pub class3: &'a [u16],
    pub outcome: Outcome,
    pub error: Option<String>,
}

impl Context<'_> {
    fn remember_error(&mut self, error: String) {
        if self.error.is_none() {
            self.error = Some(error);
        }
    }

    pub(crate) fn finish(self) -> Result<Outcome, String> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(self.outcome),
        }
    }
}

impl Runtime for Context<'_> {
    fn actor(&self, handle: SourceEntityHandle) -> Option<Actor> {
        read_actor(self.arena, handle)
    }
    fn actor_at(&self, slot: u16) -> Option<Actor> {
        read_actor(self.arena, self.arena.handle(slot)?)
    }
    fn write_actor(&mut self, actor: Actor) {
        let Some(limit) = self
            .particles
            .get(usize::from(actor.visual_index))
            .and_then(ParticleParameter::animation_limit)
        else {
            self.remember_error(format!(
                "model22 invalid animation for particle {}",
                actor.visual_index
            ));
            return;
        };
        if !write_actor(self.arena, actor) {
            return;
        }
        self.arena.entities[usize::from(actor.handle.source_slot)].record[97] = limit;
        // Hide keeps the map membership until the next pass. Movement updates
        // only the authoritative cell links and the affected terrain heads.
        if let Err(error) = crate::world::effects::sync_position(
            self.arena,
            self.terrain,
            actor.handle.source_slot,
            actor.position,
            true,
            false,
        ) {
            self.remember_error(error);
        }
        self.arena.entities[usize::from(actor.handle.source_slot)]
            .set_control_flags(actor.control_flags | 4);
    }
    fn allocate(&mut self) -> Option<Allocation> {
        let slot = self.arena.allocate_source_slot_with_terrain(self.terrain)?;
        let rng = self.arena.new_event_rng(slot);
        self.arena.entities[usize::from(slot)] =
            crate::world::terrain::cave_features::new_event(slot, rng, Position::default());
        Some(Allocation {
            handle: SourceEntityHandle {
                source_slot: slot,
                generation: self.arena.slot_generation(slot),
            },
            rng,
        })
    }
    fn hide_uninitialized(&mut self, allocation: Allocation) {
        if self.arena.slot_generation(allocation.handle.source_slot) == allocation.handle.generation
        {
            self.arena.entities[usize::from(allocation.handle.source_slot)].disable_drawing();
        }
    }
    fn hide_current_slot(&mut self, slot: u16) {
        if let Some(sphere) = self
            .arena
            .economy
            .spheres
            .iter_mut()
            .find(|r| r.active && r.source_entity_index == slot)
        {
            sphere.control_flags |= crate::native_world::ENTITY_DRAW_DISABLED_BIT;
            sphere.pending_delete = true;
        } else if let Some(row) = self
            .arena
            .entities
            .get_mut(usize::from(slot))
            .filter(|row| row.active)
        {
            row.disable_drawing();
        }
    }
    fn terrain(&self) -> &SessionTerrain {
        self.terrain
    }
    fn frozen_model_slots(&self, model: u8) -> &[u16] {
        self.families
            .get(usize::from(model))
            .map_or(&[], Vec::as_slice)
    }
    fn frozen_class3_slots(&self) -> &[u16] {
        self.class3
    }
    fn target(&self, slot: u16) -> Option<Target> {
        let row = relations_in_arena(self.arena)
            .into_iter()
            .find(|r| r.slot == slot)?;
        Some(Target {
            slot,
            source_id: row.source_id,
            class: row.target.class,
            model: row.target.model,
            life: row.target.life,
            control_flags: row.target.control_flags,
            position: row.target.position,
            yaw: row.yaw,
            roll: row.roll,
            half_width: row.half_width,
            target: row.target.word_0x96_150,
            attacker: row.attacker,
            parent: row.parent,
            stage_mode: row.stage_mode,
        })
    }
    fn player(&self, slot: u16) -> Option<PlayerView> {
        let target = self.target(slot).filter(|r| r.class == 3)?;
        let player = self.players.iter().find(|p| p.entity_index == slot)?;
        let colour = [0_u16, 1, 4, 3, 2, 5, 7, 6]
            .get(usize::from(player.player_color_index()))
            .copied()?;
        let local = self
            .arena
            .castle()
            .filter(|c| c.active && c.owner_entity_index == slot);
        let castle_slot = local.map_or_else(
            || u16::from_le_bytes([player.record[42], player.record[43]]),
            |c| c.source_entity_index,
        );
        let (position, mana, capacity) = if let Some(castle) = local {
            (castle.position, castle.mana, castle.mana_capacity)
        } else if let Some(castle) = self
            .arena
            .entities
            .get(usize::from(castle_slot))
            .filter(|r| castle_slot != 0 && r.active && r.class == 3)
        {
            (castle.position, castle.mana, castle.max_mana)
        } else {
            (Position::default(), 0, 0)
        };
        Some(PlayerView {
            target,
            castle_slot,
            castle_position: position,
            castle_mana: mana,
            castle_max_mana: capacity,
            colour_base: 105 + 8 * colour,
        })
    }
    fn add_castle_mana(&mut self, slot: u16, mana: i32) {
        if let Some(castle) = self
            .arena
            .economy
            .castle
            .as_mut()
            .filter(|c| c.active && c.source_entity_index == slot)
        {
            castle.mana = castle.mana.wrapping_add(mana).min(castle.mana_capacity);
        } else if let Some(castle) = self
            .arena
            .entities
            .get_mut(usize::from(slot))
            .filter(|r| r.active && r.class == 3)
        {
            castle.set_mana(castle.mana.wrapping_add(mana).min(castle.max_mana));
        }
    }
    fn sprite_rotation(&self, visual: u16) -> i16 {
        // Entry points validate the complete retained table before mutation.
        self.particles[usize::from(visual)].rotation_speed as i16
    }
    fn mana_sphere(&mut self, actor: Actor) {
        if actor.mana <= 0 {
            return;
        }
        let slot = actor.handle.source_slot;
        let rng = crate::native_world::level2_next_rng16(actor.entity_rng);
        self.arena.entities[usize::from(slot)].set_entity_rng(rng);
        self.arena.economy.source_rng.insert(slot, rng);
        if let Err(error) = super::super::creature_24::mc2::spawn_mana(
            self.arena,
            self.terrain,
            slot,
            actor.position,
            actor.yaw,
            actor.mana,
            actor.owner,
        ) {
            self.remember_error(error);
        }
        // 36BA0 clears the current address after NewEvent, including a newborn
        // sphere which replaced the callback's original actor.
        if let Some(sphere) = self
            .arena
            .economy
            .spheres
            .iter_mut()
            .find(|r| r.active && r.source_entity_index == slot)
        {
            sphere.owner_entity_index = 0;
            sphere.player_entity_index = 0;
        } else {
            self.arena.entities[usize::from(slot)].set_source_player_entity_index(0);
        }
    }
    fn sound(&mut self, slot: u16, sample: u8) {
        self.outcome.sounds.push((slot, sample));
    }
    fn stage_descriptor(&self, index: u8) -> Option<StageDescriptor> {
        self.runtime.descriptor(index)
    }
    fn bind_stage_descriptor(&mut self, index: u8) -> Option<StageDescriptor> {
        self.runtime.bind(index)
    }
    fn rearm_model_relations(&mut self) {
        self.runtime.rearm_model_relations();
    }
    fn emit_effect(&mut self, position: Position, model: u8, source_slot: u16) {
        // The second expiry allocation reads the same source address again;
        // the first may already have replaced that address with its newborn.
        let position = self
            .arena
            .entities
            .get(usize::from(source_slot))
            .filter(|row| row.active)
            .map_or(position, |row| row.position);
        let generation = self.arena.slot_generation(source_slot);
        if model == 73 {
            if let Err(error) = crate::native_world::creatures::goat::runtime::spawn_expiry_visual(
                self.arena,
                self.terrain,
                position,
                self.particles,
            ) {
                self.remember_error(error);
            }
            if self.arena.slot_generation(source_slot) != generation {
                let mut zero = Position::default();
                zero.z = self.terrain.ground_height(zero);
                self.arena.entities[usize::from(source_slot)].set_position(zero);
                if let Err(error) = crate::world::effects::sync_position(
                    self.arena,
                    self.terrain,
                    source_slot,
                    zero,
                    true,
                    true,
                ) {
                    self.remember_error(error);
                }
            }
        } else if model == 0 {
            match crate::world::effects::allocate(
                self.arena,
                self.terrain,
                sim_definition::adapter::mc2_effect_definition(
                    crate::level_definition::EffectKind::Impact,
                ),
                position,
                self.particles,
            ) {
                Ok(Some(slot)) if slot == source_slot => {
                    self.arena.entities[usize::from(slot)].set_position(Position::default());
                    if let Err(error) = crate::world::effects::sync_position(
                        self.arena,
                        self.terrain,
                        slot,
                        Position::default(),
                        true,
                        true,
                    ) {
                        self.remember_error(error);
                    }
                }
                Err(error) => self.remember_error(error),
                _ => {}
            }
        } else {
            self.remember_error(format!("model22 requested unsupported effect {model}"));
        }
    }
    fn feedback(&mut self, player: u16, message: u8) {
        self.outcome.feedback.push((player, message));
    }
    fn decrement_population(&mut self) {
        self.arena.economy.decrement_source_creature_population();
    }
}

pub(crate) fn validate_particles(particles: &[ParticleParameter]) -> Result<(), String> {
    if particles.len() < 347 {
        return Err("model22 requires the retained particle table".to_owned());
    }
    Ok(())
}

/// The source constructor's free-stack threshold precedes NewEvent. The
/// authored producer subsequently replaces the length and constructs pairs.
pub(crate) fn construct(
    context: &mut Context<'_>,
    phase: &mut u8,
    position: Position,
    length: Option<u8>,
) -> Result<Option<u16>, String> {
    validate_particles(context.particles)?;
    if length.is_some_and(|length| length > 15) {
        return Err("model22 authored length exceeds the source palette table".to_owned());
    }
    if context.arena.free_slot_count() < 15 {
        return Ok(None);
    }
    let Some(allocation) = context.allocate() else {
        return Ok(None);
    };
    let mut actor = Actor::construct(allocation, *phase, position, context.terrain);
    *phase = phase.wrapping_add(1);
    if let Some(length) = length {
        actor.length_or_ordinal = length as i8;
    }
    context.write_actor(actor);
    if length.is_some() {
        super::initialize_body(actor.handle, context);
    }
    Ok(Some(actor.handle.source_slot))
}

pub(crate) fn step(context: &mut Context<'_>, slot: u16) -> Result<bool, String> {
    validate_particles(context.particles)?;
    let Some(handle) = context.arena.handle(slot) else {
        return Ok(false);
    };
    if let Some(actor) = context.actor(handle)
        && usize::from(actor.visual_index) >= context.particles.len()
    {
        return Err(format!(
            "model22 slot {slot} has invalid particle {}",
            actor.visual_index
        ));
    }
    let executed = super::step(handle, context);
    context.outcome.callback_executed = executed;
    Ok(executed)
}

/// Authored 4A310 producer: source counter, health, par1 body and StageVar
/// binding all publish through the same arena before the caller exposes it.
pub(crate) fn allocate_authored(
    context: &mut Context<'_>,
    position: Position,
    max_life: i32,
    length: u8,
    stage_index: Option<u8>,
    authored_record: u16,
) -> Result<Option<u16>, String> {
    if length > 15 {
        return Err("model22 authored length exceeds the source palette table".to_owned());
    }
    let mut counter = context
        .runtime
        .constructor_counters()
        .ok_or_else(|| "model22 source constructor counter is absent".to_owned())?[22];
    let Some(slot) = construct(context, &mut counter, position, None)? else {
        return Ok(None);
    };
    context.runtime.bytes[crate::native_world::RUNTIME_BYTES + 22] = counter;
    let mut actor = context
        .actor_at(slot)
        .ok_or_else(|| "model22 constructor lost its source row".to_owned())?;
    actor.max_life = max_life;
    actor.life = max_life;
    actor.length_or_ordinal = length as i8;
    context.write_actor(actor);
    super::initialize_body(actor.handle, context);
    if let Some(index) = stage_index {
        bind_authored_route(context, slot, index);
    }
    if !context.arena.bind_raw_creature_origin(
        actor.handle,
        crate::level_definition::CreatureKind::Creature22,
        crate::campaign::source_arena::CreatureRootOrigin::AuthoredRecord { authored_record },
    ) {
        return Err("model22 constructor lost its raw provenance carrier".to_owned());
    }
    if let Some(error) = context.error.take() {
        return Err(error);
    }
    Ok(Some(slot))
}

pub(crate) fn prepare_source_pass(context: &mut Context<'_>) {
    let handles = context.families[22]
        .iter()
        .filter_map(|slot| context.arena.handle(*slot))
        .collect::<Vec<_>>();
    for handle in handles {
        super::stage::stage_prepass(handle, context);
    }
}

pub(crate) fn bind_authored_route(context: &mut Context<'_>, slot: u16, index: u8) {
    if let Some(handle) = context.arena.handle(slot) {
        super::stage::bind_stage(handle, index, context);
    }
}
