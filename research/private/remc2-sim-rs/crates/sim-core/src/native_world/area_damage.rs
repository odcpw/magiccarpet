//! MC2 area writer adapter over the existing arena and family mailboxes.

use std::collections::BTreeSet;

use super::{ENTITY_DRAW_DISABLED_BIT, Level2OrdinaryResetSession};
use crate::{
    Position,
    campaign::source_arena::SourceArena,
    combat::{
        actors::{DamageMailboxMut, DamageWriter},
        area::{self, Actor, AreaDamage, AreaWorld, Shape},
    },
    goat::GoatFamily,
    house::HouseFamily,
    villager::VillagerFamily,
};

/// Borrow the three actual scheduler-owned actor domains. The ordinary
/// session is split at construction; no unrelated session state is retained.
pub(crate) struct ActorDomains<'a> {
    goats: &'a mut GoatFamily,
    houses: &'a mut HouseFamily,
    villagers: &'a mut VillagerFamily,
    house_parameters: &'a std::collections::BTreeMap<u8, crate::house::HouseParameter>,
    particles: Option<&'a [super::ParticleParameter]>,
}

impl<'a> ActorDomains<'a> {
    pub(crate) fn new(
        goats: &'a mut GoatFamily,
        ordinary: &'a mut Level2OrdinaryResetSession,
    ) -> Self {
        Self {
            goats,
            houses: &mut ordinary.houses,
            villagers: &mut ordinary.villagers,
            house_parameters: &ordinary.house_parameters,
            particles: None,
        }
    }

    pub(crate) fn with_particles(mut self, particles: &'a [super::ParticleParameter]) -> Self {
        self.particles = Some(particles);
        self
    }

    pub(crate) fn reborrow(&mut self) -> ActorDomains<'_> {
        ActorDomains {
            goats: &mut *self.goats,
            houses: &mut *self.houses,
            villagers: &mut *self.villagers,
            house_parameters: self.house_parameters,
            particles: self.particles,
        }
    }

    pub(crate) fn source_target_context(&self) -> super::source_target::Context<'_> {
        super::source_target::Context {
            goats: Some(self.goats),
            houses: Some(self.houses),
            villagers: Some(self.villagers),
            house_parameters: Some(self.house_parameters),
            particles: self.particles,
            ..super::source_target::Context::default()
        }
    }

    pub(crate) fn set_status_flags(&mut self, slot: u16, class: u8, model: u8, mask: u32, flags: u32) -> bool {
        if (class, model) == (super::GOAT_CLASS, super::GOAT_MODEL)
            && let Some(row) = self.goats.goats().iter().find(|row| row.active && row.entity_index == slot) {
            return self.goats.set_control_flags(slot, (row.control_flags & !mask) | (flags & mask));
        }
        if (class, model) == (super::HOUSE_CLASS, super::HOUSE_MODEL)
            && let Some(row) = self.houses.house_mut(slot).filter(|row| row.source_retained()) {
            row.control_flags = (row.control_flags & !mask) | (flags & mask);
            return true;
        }
        if let Some(row) = self.villagers.villagers_mut().iter_mut().find(|row| class == 5 && model == row.model && row.source_retained() && row.entity_index == slot) {
            row.control_flags = (row.control_flags & !mask) | (flags & mask);
            return true;
        }
        false
    }

    pub(crate) fn set_status_speed(&mut self, slot: u16, class: u8, model: u8, speed: i16) {
        if (class, model) == (super::GOAT_CLASS, super::GOAT_MODEL) && self.goats.set_status_speed(slot, speed) { return; }
        if let Some(row) = self.villagers.villagers_mut().iter_mut().find(|row| class == 5 && model == row.model && row.source_retained() && row.entity_index == slot) {
            row.actual_speed = speed;
        }
    }

    fn source_view(&self, slot: u16) -> Option<crate::level1_mission::Level1SourceEntityView> {
        self.goats
            .goats()
            .iter()
            .find(|row| row.active && row.entity_index == slot)
            .map(super::goat_source_entity_view)
            .or_else(|| {
                self.houses
                    .houses()
                    .iter()
                    .find(|row| row.source_retained() && row.entity_index == slot)
                    .map(super::house_source_entity_view)
            })
            .or_else(|| {
                self.villagers
                    .villagers()
                    .iter()
                    .find(|row| row.source_retained() && row.entity_index == slot)
                    .map(super::villager_source_entity_view)
            })
    }

    fn submit(&mut self, slot: u16, payload: u16, owner: u16, writer: DamageWriter) -> bool {
        if self
            .houses
            .houses()
            .iter()
            .any(|row| row.source_retained() && row.entity_index == slot)
        {
            match writer {
                DamageWriter::Ordinary => self.houses.submit_source_damage(slot, payload, owner),
                DamageWriter::Contact => self.houses.submit_source_contact(slot, payload, owner),
                DamageWriter::FirstWriter => false,
            }
        } else if self
            .villagers
            .villagers()
            .iter()
            .any(|row| row.source_retained() && row.entity_index == slot)
        {
            match writer {
                DamageWriter::Ordinary => self.villagers.submit_source_damage(slot, payload, owner),
                DamageWriter::Contact => self.villagers.submit_source_contact(slot, payload, owner),
                DamageWriter::FirstWriter => false,
            }
        } else {
            match writer {
                DamageWriter::Ordinary => self.goats.submit_source_damage(slot, payload, owner),
                DamageWriter::Contact => self.goats.submit_source_contact(slot, payload, owner),
                DamageWriter::FirstWriter => false,
            }
        }
    }
}

pub(crate) fn apply(
    arena: &mut SourceArena,
    actors: Option<ActorDomains<'_>>,
    player: Option<(u16, Position)>,
    request: AreaDamage,
) -> Result<u16, String> {
    area::apply(
        &mut ArenaTargets {
            arena,
            actors,
            player,
        },
        request,
    )
}

/// `sub_11900` mode zero after the caller selected its actual target. Contact
/// replaces an owned payload and adds to ownerless residue; it does not repeat
/// the producer's distance, geometry, class or identity admission.
pub(crate) fn contact(
    arena: &mut SourceArena,
    actors: Option<ActorDomains<'_>>,
    slot: u16,
    payload: u16,
    owner: u16,
) -> Result<(), String> {
    if slot == 0 || usize::from(slot) >= arena.entities.len() {
        return Err(format!(
            "contact target {slot} is null or outside source arena"
        ));
    }
    ArenaTargets {
        arena,
        actors,
        player: None,
    }
    .submit_damage(slot, payload, owner, DamageWriter::Contact)
}

/// Gravity reads and replaces the complete signed source bounty. Unlike an
/// ordinary hit, replacing this mailbox must neither narrow nor combine it.
pub(crate) fn source_bounty(
    arena: &SourceArena,
    actors: Option<ActorDomains<'_>>,
    slot: u16,
) -> Result<(i32, u16), String> {
    if let Some(view) = actors
        .as_ref()
        .and_then(|domains| domains.source_view(slot))
    {
        return Ok((view.pending_damage, view.pending_attacker));
    }
    read_arena_bounty(arena, slot)
}

pub(crate) fn replace_source_bounty(
    arena: &mut SourceArena,
    actors: Option<ActorDomains<'_>>,
    slot: u16,
    payload: i32,
    owner: u16,
) -> Result<(), String> {
    edit_source_bounty(arena, actors, slot, |damage, attacker| {
        *damage = payload;
        *attacker = owner;
    })
}

/// Read the same canonical mailbox order as `edit_arena_bounty` without
/// materializing a missing model9 sidecar or changing any carrier state.
fn read_arena_bounty(arena: &SourceArena, slot: u16) -> Result<(i32, u16), String> {
    let economy = &arena.economy;
    macro_rules! smoke_bounty {
        ($rows:expr) => {
            if let Some(row) = $rows
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
            {
                return Ok((row.pending_damage, row.pending_attacker_entity_index));
            }
        };
    }
    smoke_bounty!(economy.model59_smoke_children);
    smoke_bounty!(economy.model13_spawned_children);
    if let Some(row) = economy
        .authored_trees
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        return Ok((row.pending_damage, row.owner_entity_index));
    }
    if let Some(row) = economy
        .spheres
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        return Ok((row.ordinary_damage_payload, row.ordinary_damage_owner));
    }
    if let Some(row) = economy
        .castle_defenders
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        return Ok((row.pending_damage, row.pending_attacker_entity_index));
    }
    if let Some(row) = economy.castle_at(slot) {
        return Ok((row.pending_damage, row.pending_attacker_entity_index));
    }
    if let Some(row) = economy
        .bee_creatures
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        return Ok((row.pending_damage, row.pending_attacker_entity_index));
    }
    if let Some(row) = economy
        .house_residents
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        return Ok((row.pending_damage, row.attacker_entity_index));
    }
    if let Some(row) = economy
        .authored_dragon_parts
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        return Ok((row.pending_damage, row.pending_word_0x62));
    }
    if let Some(row) = economy
        .authored_class5_model3_children
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        return Ok((row.damage_dword_0x5e_94, row.pending_word_0x62));
    }
    if let Some(row) = economy
        .authored_class5
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        let owner = row.pending_word_0x62;
        if let Some(state) = economy
            .authored_class5_archer
            .iter()
            .find(|state| state.source_entity_index == slot)
        {
            return Ok((state.pending_damage, owner));
        }
        if let Some(state) = economy
            .authored_class5_model3
            .iter()
            .find(|state| state.source_entity_index == slot)
        {
            return Ok((state.damage_dword_0x5e_94, owner));
        }
        if let Some(state) = economy
            .authored_class5_runtime
            .iter()
            .find(|state| state.source_entity_index == slot)
        {
            return Ok((state.pending_damage, owner));
        }
        if row.model == 9 {
            return Ok((0, owner));
        }
        return Err(format!("area class5 slot {slot} has no ordinary mailbox"));
    }
    if economy.live_arena_cursor_row(slot).is_some() {
        return Err(format!(
            "damage slot {slot} has no canonical mailbox writer"
        ));
    }
    let row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active)
        .ok_or_else(|| format!("area raw slot {slot} no longer live"))?;
    Ok((
        row.source_wind_spell_payload(),
        row.source_wind_spell_owner(),
    ))
}

fn edit_source_bounty<R>(
    arena: &mut SourceArena,
    mut actors: Option<ActorDomains<'_>>,
    slot: u16,
    edit: impl FnOnce(&mut i32, &mut u16) -> R,
) -> Result<R, String> {
    if let Some(domains) = actors.as_mut()
        && let Some(view) = domains.source_view(slot)
    {
        let mut payload = view.pending_damage;
        let mut owner = view.pending_attacker;
        let result = edit(&mut payload, &mut owner);
        if let Some(house) = domains
            .houses
            .house_mut(slot)
            .filter(|row| row.source_retained())
        {
            house.pending_damage = payload;
            house.pending_attacker = i16::from_le_bytes(owner.to_le_bytes());
        } else if let Some(villager) = domains
            .villagers
            .villagers_mut()
            .iter_mut()
            .find(|row| row.source_retained() && row.entity_index == slot)
        {
            villager.pending_damage = payload;
            villager.pending_attacker = i16::from_le_bytes(owner.to_le_bytes());
        } else {
            if !domains.goats.set_gravity_bounty(slot, payload, owner) {
                return Err(format!("source bounty lost live carrier {slot}"));
            }
            let goat = domains
                .goats
                .goats()
                .iter()
                .find(|row| row.entity_index == slot)
                .ok_or_else(|| format!("source bounty lost Goat projection {slot}"))?;
            arena.economy.sync_authored_goat_projection(goat);
        }
        return Ok(result);
    }
    ArenaTargets {
        arena,
        actors: None,
        player: None,
    }
    .edit_arena_bounty(slot, false, edit)
}

/// Read the same current carrier used by damage writers. Terrain owns cell
/// traversal and admission; it must not consult a previous raw occupant.
pub(crate) fn source_actor(
    arena: &mut SourceArena,
    actors: Option<ActorDomains<'_>>,
    slot: u16,
) -> Option<Actor> {
    ArenaTargets {
        arena,
        actors,
        player: None,
    }
    .actor(slot)
}

/// Read life from the same current carrier as source_actor, including dead
/// retained targets. A typed occupant never falls back to stale raw bytes.
pub(crate) fn source_life(
    arena: &SourceArena,
    actors: Option<ActorDomains<'_>>,
    slot: u16,
) -> Option<i32> {
    if let Some(domains) = actors.as_ref()
        && let Some(view) = domains.source_view(slot)
    {
        return Some(view.life);
    }
    read_arena_life(arena, slot)
}

/// Immutable life projection shared by current-address target readers.
pub(crate) fn read_arena_life(arena: &SourceArena, slot: u16) -> Option<i32> {
    let economy = &arena.economy;
    macro_rules! row_life {
        ($rows:expr) => {
            if let Some(row) = $rows
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
            {
                return Some(i32::from(row.life));
            }
        };
    }
    row_life!(economy.spheres);
    row_life!(economy.castle_defenders);
    row_life!(economy.bee_creatures);
    row_life!(economy.house_residents);
    row_life!(economy.authored_trees);
    row_life!(economy.model59_smoke_children);
    row_life!(economy.model13_spawned_children);
    row_life!(economy.authored_dragon_parts);
    row_life!(economy.authored_class5_model3_children);
    row_life!(economy.authored_class5);
    if let Some(row) = economy.castle_at(slot) {
        return Some(row.life);
    }
    if economy.live_arena_cursor_row(slot).is_some() {
        return None;
    }
    arena
        .entities
        .get(usize::from(slot))
        .filter(|s| s.active)
        .map(|s| s.life)
}

#[cfg(test)]
thread_local! {
    static CONSTRUCTION_KILL_TRACE: std::cell::RefCell<Option<Vec<(u16, u32, u16, Option<i32>)>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn start_construction_kill_trace() {
    CONSTRUCTION_KILL_TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
}

#[cfg(test)]
pub(crate) fn take_construction_kill_trace() -> Vec<(u16, u32, u16, Option<i32>)> {
    CONSTRUCTION_KILL_TRACE.with(|trace| trace.borrow_mut().take().unwrap_or_default())
}

/// The direct life/credit write made by source 57390 after terrain admission.
/// This leaves ordinary pending damage intact for the actual family callback.
pub(crate) fn apply_construction_kill(
    arena: &mut SourceArena,
    mut actors: Option<ActorDomains<'_>>,
    slot: u16,
    source_id: u16,
) -> Result<bool, String> {
    let Some(target) = source_actor(arena, actors.as_mut().map(ActorDomains::reborrow), slot)
    else {
        return Ok(false);
    };
    if target.class != 5 || target.source_id == source_id {
        return Ok(false);
    }
    #[cfg(test)]
    let previous_life = source_life(arena, actors.as_mut().map(ActorDomains::reborrow), slot);
    let wrote = write_direct_creature_kill(arena, actors, slot, source_id, Some(source_id))?;
    #[cfg(test)]
    if wrote {
        CONSTRUCTION_KILL_TRACE.with(|trace| {
            if let Some(trace) = trace.borrow_mut().as_mut() {
                trace.push((slot, arena.slot_generation(slot), source_id, previous_life));
            }
        });
    }
    Ok(wrote)
}

/// `KillAllCreatures_1B5F0` walks the existing source lists, including friendly
/// creatures. It writes the killer word without replacing a damage attacker
/// or consuming the pending mailbox. Model27 enters its real kill callback.
pub(crate) fn apply_encounter_creature_kills(
    arena: &mut SourceArena,
    mut actors: Option<ActorDomains<'_>>,
    lists: &EffectLists,
    families: &[Vec<u16>; 29],
    player: u16,
) -> Result<(), String> {
    if !lists.is_current(arena) {
        return Ok(());
    }
    for (model, slots) in families.iter().enumerate() {
        if model == 10 {
            continue;
        }
        for &slot in slots {
            if model == 27 {
                let row = arena
                    .entities
                    .get_mut(usize::from(slot))
                    .filter(|row| row.active && row.class == 5 && row.model == 27)
                    .ok_or_else(|| format!("final encounter lost model27 source slot {slot}"))?;
                row.set_source_action(221);
                row.record[40..42].copy_from_slice(&player.to_le_bytes());
            } else if !write_direct_creature_kill(
                arena,
                actors.as_mut().map(ActorDomains::reborrow),
                slot,
                player,
                None,
            )? {
                return Err(format!("final encounter lost frozen creature slot {slot}"));
            }
        }
    }
    Ok(())
}

fn write_direct_creature_kill(
    arena: &mut SourceArena,
    mut actors: Option<ActorDomains<'_>>,
    slot: u16,
    source_id: u16,
    attacker: Option<u16>,
) -> Result<bool, String> {
    if let Some(domains) = actors.as_mut() {
        if domains
            .goats
            .goats()
            .iter()
            .any(|row| row.active && row.entity_index == slot)
        {
            domains.goats.apply_source_kill(slot, source_id, attacker);
            let goat = domains
                .goats
                .goats()
                .iter()
                .find(|row| row.entity_index == slot)
                .ok_or_else(|| format!("construction burial lost Goat carrier {slot}"))?;
            arena.economy.sync_authored_goat_projection(goat);
            return Ok(true);
        }
        if domains
            .villagers
            .villagers()
            .iter()
            .any(|row| row.source_retained() && row.entity_index == slot)
        {
            domains
                .villagers
                .apply_source_kill(slot, source_id, attacker);
            return Ok(true);
        }
    }
    let economy = &mut arena.economy;
    if economy.apply_settlement_source_kill(slot, source_id, attacker) {
        return Ok(true);
    }
    if let Some(row) = economy
        .bee_creatures
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        row.life = -1;
        row.kill_credit_entity_index = source_id;
        if let Some(attacker) = attacker {
            row.damage_attacker_entity_index = attacker;
        }
        return Ok(true);
    }
    if let Some(row) = economy
        .castle_defenders
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        row.apply_source_kill(source_id, attacker);
        return Ok(true);
    }
    if let Some(row) = economy
        .authored_class5_model3_children
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        row.life = -1;
        row.word_0x24 = source_id;
        if let Some(attacker) = attacker {
            row.word_0x26 = attacker;
        }
        return Ok(true);
    }
    if let Some(row) = economy
        .authored_dragon_parts
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        row.life = -1;
        row.word_0x24 = source_id;
        if let Some(attacker) = attacker {
            row.word_0x26 = attacker;
        }
        return Ok(true);
    }
    if let Some(row) = economy
        .authored_class5
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        row.life = -1;
        row.word_0x24 = source_id;
        if let Some(attacker) = attacker {
            row.word_0x26 = attacker;
        }
        return Ok(true);
    }
    if economy.live_arena_cursor_row(slot).is_some() {
        return Err(format!(
            "construction burial has no direct-life writer for live carrier {slot}"
        ));
    }
    let row = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && row.class == 5)
        .ok_or_else(|| format!("construction burial lost admitted class5 slot {slot}"))?;
    row.set_life(-1);
    let attacker = attacker.unwrap_or_else(|| row.word_26());
    row.set_damage_result_words(source_id, attacker);
    Ok(true)
}

/// Read a current arena-owned damage actor without borrowing mutation
/// services or retaining a previous occupant's raw projection.
pub(crate) fn read_arena_actor(arena: &SourceArena, slot: u16) -> Option<Actor> {
    let mut actor = read_current_arena_actor(arena, slot)?;
    actor.control_flags = super::source_target::status_flags(arena, slot, actor.control_flags);
    Some(actor)
}

fn read_current_arena_actor(arena: &SourceArena, slot: u16) -> Option<Actor> {
    if slot == 0 {
        return None;
    }
    let economy = &arena.economy;
    if let Some(row) = economy.authored_class5_archer_children.iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        return Some(Actor {
            source_id: row.entity_id,
            class: row.source_class,
            model: row.model,
            position: row.position,
            shape: Shape {
                half_width: row.collision_half_width,
                half_depth: row.collision_half_width,
                vertical_offset: row.collision_vertical_offset,
                half_height: row.collision_half_height,
            },
            damage_mask: 0,
            control_flags: row.control_flags | if row.drawing_enabled { 0 } else { ENTITY_DRAW_DISABLED_BIT },
        });
    }
    if let Some(row) = economy
        .authored_trees
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        return Some(Actor {
            source_id: row.entity_id,
            class: row.source_class,
            model: row.model,
            position: row.position,
            shape: shape(row.pitch, row.roll, row.yaw, row.fov),
            damage_mask: row.source_byte_38,
            control_flags: row.control_flags
                | if row.drawing_enabled {
                    0
                } else {
                    ENTITY_DRAW_DISABLED_BIT
                },
        });
    }
    if let Some(row) = economy
        .spheres
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        return Some(Actor {
            source_id: row.source_entity_index,
            class: 10,
            model: 39,
            position: row.position,
            shape: shape(
                row.collision_pitch,
                row.collision_roll,
                row.collision_yaw,
                row.collision_fov,
            ),
            damage_mask: 3,
            control_flags: row.control_flags,
        });
    }
    if let Some(row) = economy
        .castle_defenders
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        return Some(Actor {
            source_id: row.id_entity_index,
            class: 5,
            model: 15,
            position: row.position,
            shape: shape(row.shift_rot_x, row.shift_rot_y, row.collision_vertical_offset, row.fov),
            damage_mask: row.byte_38,
            control_flags: row.control_flags,
        });
    }
    if let Some(row) = economy.castle_at(slot) {
        return Some(Actor {
            source_id: row.source_id_entity_index,
            class: row.source_class,
            model: row.source_model,
            position: row.position,
            shape: shape(
                row.shift_pitch,
                row.shift_roll,
                row.shift_yaw,
                row.shift_fov,
            ),
            damage_mask: row.source_byte_38,
            control_flags: row.control_flags,
        });
    }
    if let Some(row) = economy
        .bee_creatures
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        // Particle 3 supplies array.yaw; SetEntityShiftRot replaces only
        // pitch/roll/fov. Read the current carrier after either operation.
        return Some(Actor {
            source_id: row.entity_id,
            class: row.source_class,
            model: row.model,
            position: row.position,
            shape: shape(
                row.shift_rot_x as i16,
                row.shift_rot_x as i16,
                row.collision_vertical_offset,
                row.shift_rot_y as i16,
            ),
            damage_mask: 1,
            control_flags: row.control_flags
                | if row.drawing_enabled {
                    0
                } else {
                    ENTITY_DRAW_DISABLED_BIT
                },
        });
    }
    if let Some(row) = economy
        .house_residents
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        // AddVilliger/AddBuilder/AddTrader set byte38=1. Their current
        // domain does not mutate that constructor-owned mask.
        return Some(Actor {
            source_id: row.source_id,
            class: 5,
            model: row.model,
            position: row.position,
            shape: shape(
                row.collision_half_width,
                row.collision_half_width,
                row.collision_vertical_offset,
                row.collision_half_height,
            ),
            damage_mask: 1,
            control_flags: row.control_flags,
        });
    }
    if let Some(row) = economy
        .authored_dragon_parts
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        let mask = economy
            .authored_class5_runtime
            .iter()
            .find(|root| root.source_entity_index == row.parent_entity_index)
            .map_or(1, |root| root.byte_0x38);
        return Some(Actor {
            source_id: row.entity_id,
            class: 5,
            model: row.model,
            position: row.position,
            shape: shape(
                row.render_pitch,
                row.render_roll,
                row.render_yaw,
                row.render_fov,
            ),
            damage_mask: mask,
            control_flags: row.control_flags,
        });
    }
    if let Some(row) = economy
        .authored_class5_model3_children
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        let mask = economy
            .authored_class5_runtime
            .iter()
            .find(|root| root.source_entity_index == row.parent_entity_index)
            .map_or(1, |root| root.byte_0x38);
        return Some(Actor {
            // sub_4B6F0 qmemcpy copies the root's id into every body.
            // The immutable constructor parent retains that id even if
            // the root's numeric slot is later reclaimed and reused.
            source_id: row.parent_entity_index,
            class: 5,
            model: row.model,
            position: row.position,
            shape: shape(
                row.render_pitch,
                row.render_roll,
                row.render_yaw,
                row.render_fov,
            ),
            damage_mask: mask,
            control_flags: row.control_flags,
        });
    }
    if let Some(row) = economy
        .authored_class5
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        let runtime = economy
            .authored_class5_runtime
            .iter()
            .find(|state| state.source_entity_index == slot);
        let archer = economy
            .authored_class5_archer
            .iter()
            .find(|state| state.source_entity_index == slot);
        let firefly = economy
            .authored_fireflies
            .iter()
            .find(|state| state.source_entity_index == slot);
        let dragon = economy
            .authored_dragons
            .iter()
            .find(|state| state.source_entity_index == slot);
        let model3 = economy
            .authored_class5_model3
            .iter()
            .find(|state| state.source_entity_index == slot);
        let flags = archer
            .map(|state| state.control_flags)
            .or_else(|| firefly.map(|state| state.control_flags))
            .or_else(|| dragon.map(|state| state.control_flags))
            .or_else(|| model3.map(|state| state.control_flags))
            .or_else(|| match row.model {
                // These two current adapters keep NewEvent's fixed
                // collision bit: model9 never writes flags, and Trader
                // explicitly starts each canonical callback with 8.
                // Do not admit another family through a generic default.
                9 | 14 => Some(8),
                _ => None,
            })?;
        let dimensions = if let Some(state) = dragon {
            shape(
                state.render_pitch,
                state.render_roll,
                state.render_yaw,
                state.render_fov,
            )
        } else if let Some(state) = model3 {
            shape(
                state.render_pitch,
                state.render_roll,
                state.render_yaw,
                state.render_fov,
            )
        } else {
            let offset = if row.model == 4 {
                super::level2_class5::archer::ARCHER_COLLISION_VERTICAL_OFFSET
            } else {
                0
            };
            shape(row.shift_rot_x, row.shift_rot_x, offset, row.shift_rot_y)
        };
        return Some(Actor {
            source_id: archer
                .map(|state| state.entity_id)
                .or_else(|| firefly.map(|state| state.entity_id))
                .or_else(|| dragon.map(|state| state.entity_id))
                .or_else(|| model3.map(|state| state.entity_id))
                .unwrap_or(slot),
            class: 5,
            model: row.model,
            position: row.position,
            shape: dimensions,
            damage_mask: archer
                .map(|state| state.byte_0x38)
                .or_else(|| runtime.map(|state| state.byte_0x38))
                .unwrap_or(1),
            control_flags: flags
                | if row.drawing_enabled {
                    0
                } else {
                    ENTITY_DRAW_DISABLED_BIT
                },
        });
    }
    // A dynamic occupant cannot expose a previous raw occupant's mailbox.
    if economy.live_arena_cursor_row(slot).is_some() {
        return None;
    }
    let row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active)?;
    Some(Actor {
        source_id: row.source_id,
        class: row.class,
        model: row.model,
        position: row.position,
        shape: shape(
            row.array_pitch(),
            row.array_roll(),
            row.array_yaw(),
            row.array_fov(),
        ),
        damage_mask: row.source_byte_38(),
        control_flags: row.control_flags,
    })
}

struct ArenaTargets<'arena, 'actors> {
    arena: &'arena mut SourceArena,
    actors: Option<ActorDomains<'actors>>,
    player: Option<(u16, Position)>,
}

/// The source class/model heads are frozen before the numeric Update scan.
/// A fallback NewEvent clears those heads; map-chain membership stays live.
#[derive(Clone, Debug, Default)]
pub(crate) struct EffectLists {
    class3: Vec<u16>,
    model45: Vec<u16>,
    class3_chain: crate::campaign::source_arena::SourceSlotChain,
    model45_chain: crate::campaign::source_arena::SourceSlotChain,
    mana_chain: crate::campaign::source_arena::SourceManaChain,
    model22_chain: crate::campaign::source_arena::SourceSlotChain,
    class5_chains: [crate::campaign::source_arena::SourceSlotChain; 29],
    epoch: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EncounterManaAction {
    Approach(u8),
    Refresh,
}

fn mutate_encounter_mana(
    action: EncounterManaAction,
    flags: &mut u32,
    life: &mut i32,
    max_life: &mut i32,
) -> Result<(), String> {
    match action {
        EncounterManaAction::Approach(1) => *flags = (*flags | 0x0100_0000) & !0x0080_0000,
        EncounterManaAction::Approach(2) => *flags = (*flags | 0x8000_0000) & !0x0001_0000,
        EncounterManaAction::Approach(3) => *flags |= ENTITY_DRAW_DISABLED_BIT,
        EncounterManaAction::Approach(mode) => {
            return Err(format!("unknown encounter mana-chain approach mode {mode}"));
        }
        EncounterManaAction::Refresh => {
            *flags |= 0x2000;
            *max_life = 140;
            *life = 140;
        }
    }
    Ok(())
}

/// Consume sub_21490/phase14 writes at their source call site, before any
/// subsequent NewEvent can clear the frozen dword38523 head.
pub(crate) fn apply_encounter_world_action(
    arena: &mut SourceArena,
    lists: &EffectLists,
    action: EncounterManaAction,
    particles: &[super::ParticleParameter],
) -> Result<(), String> {
    if !lists.is_current(arena) {
        return Ok(());
    }
    for &slot in lists.mana_chain.slots() {
        match arena
            .economy
            .live_arena_cursor_row(slot)
            .map(|row| row.kind)
        {
            Some(super::Level2DynamicArenaKind::ManaSphere) => {
                let row = arena
                    .economy
                    .spheres
                    .iter_mut()
                    .find(|row| row.active && row.source_entity_index == slot)
                    .ok_or_else(|| format!("encounter mana sphere {slot} lost its carrier"))?;
                mutate_encounter_mana(
                    action,
                    &mut row.control_flags,
                    &mut row.life,
                    &mut row.max_life,
                )?;
            }
            Some(super::Level2DynamicArenaKind::PlayerDeathMarker) => {
                let row = arena
                    .economy
                    .player_death_markers
                    .iter_mut()
                    .find(|row| row.active && row.source_entity_index == slot)
                    .ok_or_else(|| format!("encounter death marker {slot} lost its carrier"))?;
                mutate_encounter_mana(
                    action,
                    &mut row.control_flags,
                    &mut row.life,
                    &mut row.max_life,
                )?;
                publish_player_death_marker(arena, slot, particles)?;
            }
            Some(kind) => {
                return Err(format!(
                    "encounter mana address {slot} was replaced by unsupported current {kind:?}"
                ));
            }
            None => {
                // Frozen source pointers read the current address, including
                // retained freed rows; there is no active/class/generation gate.
                let row = arena
                    .entities
                    .get_mut(usize::from(slot))
                    .ok_or_else(|| format!("encounter mana address {slot} is outside the arena"))?;
                let (mut flags, mut life, mut max_life) =
                    (row.control_flags, row.life, row.max_life);
                mutate_encounter_mana(action, &mut flags, &mut life, &mut max_life)?;
                row.set_control_flags(flags);
                row.set_life(life);
                row.set_max_life(max_life);
            }
        }
    }
    Ok(())
}

/// Preserve the current typed marker's fixed row for pointer consumers after
/// reclaim. The original particle binding supplies its actual collision axes.
pub(crate) fn publish_player_death_marker(
    arena: &mut SourceArena,
    slot: u16,
    particles: &[super::ParticleParameter],
) -> Result<(), String> {
    let marker = arena
        .economy
        .player_death_markers
        .iter()
        .find(|row| row.source_entity_index == slot)
        .copied()
        .ok_or_else(|| format!("source player death marker {slot} is absent"))?;
    let particle = *particles
        .get(usize::from(marker.particle_parameter_index))
        .ok_or_else(|| format!("player death marker {slot} has no retained particle"))?;
    let mut row =
        crate::world::terrain::cave_features::new_event(slot, marker.entity_rng, marker.position);
    // This is a projection refresh, not another NewEvent. Preserve the
    // current fixed record's intrusive successor through marker callbacks.
    row.next_relation = arena.entities[usize::from(slot)].next_relation;
    let encoded = row.next_relation.map_or(0, |next| u32::from(next) + 1);
    row.record[4..8].copy_from_slice(&encoded.to_le_bytes());
    row.class = 10;
    row.model = 40;
    row.set_source_action(42);
    row.set_phase(marker.dispatch_phase);
    row.set_source_dword_10(marker.constructor_phase as i32);
    row.set_max_life(marker.max_life);
    row.set_life(marker.life);
    row.set_control_flags(marker.control_flags);
    row.record[60] = marker.byte_38;
    row.set_source_player_entity_index(marker.player_entity_index);
    row.record[104..108].copy_from_slice(&marker.pending_possession_payload.to_le_bytes());
    row.record[108..110].copy_from_slice(&marker.pending_possession_owner.to_le_bytes());
    crate::world::terrain::cave_features::bind_particle(
        &mut row,
        marker.particle_parameter_index,
        particle,
    )?;
    row.set_map_links(
        arena.economy.map_chains.old_map_entity(slot),
        arena.economy.map_chains.next_entity(slot),
    );
    arena.entities[usize::from(slot)] = row;
    Ok(())
}

/// Called only at the following source boundary, after the hiding callback's
/// own epilogue. Unlink the actual map row before returning its slot to NewEvent.
pub(crate) fn retire_hidden_player_death_markers(
    arena: &mut SourceArena,
    terrain: &mut super::SessionTerrain,
) -> Result<(), String> {
    let hidden = arena
        .economy
        .player_death_markers
        .iter()
        .filter(|row| row.active && row.control_flags & ENTITY_DRAW_DISABLED_BIT != 0)
        .map(|row| (row.source_entity_index, row.position))
        .collect::<Vec<_>>();
    for (slot, position) in hidden {
        crate::world::effects::sync_position(arena, terrain, slot, position, false, false)?;
        arena.entities[usize::from(slot)].reclaim_source_row();
        if let Some(marker) = arena
            .economy
            .player_death_markers
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
        {
            marker.active = false;
        }
    }
    Ok(())
}

impl EffectLists {
    /// Current fixed records reached from the pass-start class-5 heads.
    /// Preserve NewEvent's next-link cut and source-list reset epoch.
    pub(crate) fn class5_current_slots(&self, arena: &SourceArena) -> [Vec<u16>; 29] {
        self.class5_current_slots_after_allocation(&arena.economy, &arena.entities)
    }

    pub(crate) fn class5_current_slots_after_allocation(
        &self,
        economy: &super::Level2EconomyState,
        entities: &[super::EntitySnapshot],
    ) -> [Vec<u16>; 29] {
        std::array::from_fn(|model| {
            self.class5_chains[model].current_slots(economy, entities)
        })
    }

    pub(crate) fn possession_guidance_slots(
        &self,
        arena: &SourceArena,
    ) -> (Vec<u16>, Vec<u16>, Vec<u16>) {
        (
            self.mana_chain
                .current_slots(&arena.economy, &arena.entities),
            self.model45_chain
                .current_slots(&arena.economy, &arena.entities),
            self.model22_chain
                .current_slots(&arena.economy, &arena.entities),
        )
    }
    pub(crate) fn castle_site_rows(
        &self,
        arena: &SourceArena,
        context: super::source_target::Context<'_>,
    ) -> Result<super::level2_dynamic_economy::CastleSiteRows, String> {
        let resolve = |chain: &crate::campaign::source_arena::SourceSlotChain| {
            chain.current_slots(&arena.economy, &arena.entities).into_iter()
                .map(|slot| {
                    let view = super::source_target::read(arena, context, slot)?
                        .ok_or_else(|| format!("Castle site list lost current slot {slot}"))?;
                    let shape = view.collision
                        .ok_or_else(|| format!("Castle site slot {slot} has no current source axes"))?;
                    Ok(super::level2_dynamic_economy::CastleSiteRow {
                        model: view.model, position: view.position,
                        pitch: shape.half_width, roll: shape.half_depth,
                    })
                }).collect::<Result<Vec<_>, String>>()
        };
        Ok(super::level2_dynamic_economy::CastleSiteRows {
            class3: resolve(&self.class3_chain)?,
            model45: resolve(&self.model45_chain)?,
        })
    }

    pub(crate) fn mana_chain(&self) -> &crate::campaign::source_arena::SourceManaChain {
        &self.mana_chain
    }

    pub(crate) fn is_current(&self, arena: &SourceArena) -> bool {
        self.epoch == arena.economy.replacement_list_epoch
    }

    pub(crate) fn class3_slots(&self, arena: &SourceArena) -> &[u16] {
        if self.is_current(arena) {
            &self.class3
        } else {
            &[]
        }
    }

    /// Follow the captured source head through each current next_0 word.
    /// A late consumer must observe replacement/link changes since capture.
    pub(crate) fn current_class3_slots(&self, arena: &SourceArena) -> Vec<u16> {
        self.class3_chain.current_slots(&arena.economy, &arena.entities)
    }

    pub(crate) fn model45_slots(&self, arena: &SourceArena) -> &[u16] {
        if self.is_current(arena) {
            &self.model45
        } else {
            &[]
        }
    }
}

pub(crate) fn capture_effect_lists(
    arena: &mut SourceArena,
    actors: Option<ActorDomains<'_>>,
) -> EffectLists {
    let targets = ArenaTargets {
        arena,
        actors,
        player: None,
    };
    let mut model45 = targets
        .arena
        .live_model45_buildings()
        .into_iter()
        .map(|(slot, _, _)| slot)
        .collect::<Vec<_>>();
    if let Some(domains) = targets.actors.as_ref() {
        model45.extend(
            domains
                .houses
                .houses()
                .iter()
                .filter(|row| row.source_retained())
                .map(|row| row.entity_index),
        );
    }
    model45.sort_unstable();
    model45.dedup();
    model45.retain(|&slot| match targets.arena.economy.live_arena_kind(slot) {
        Some(super::Level2DynamicArenaKind::AuthoredModel45) => true,
        Some(_) => false,
        None => targets.arena.entities.get(usize::from(slot)).is_some_and(|row| {
            row.active && row.class == 10 && row.model == 45
        }),
    });
    let mut class3 = targets.class3_slots();
    class3.extend(targets.arena.economy.castle_children.iter()
        .filter(|row| row.active && row.source_class == 3 && row.life >= 0)
        .map(|row| row.source_entity_index));
    class3.sort_unstable();
    class3.dedup();
    class3.retain(|&slot| {
        if let Some(row) = targets.arena.economy.castle_at(slot) {
            row.source_class == 3 && row.life >= 0
        } else if let Some(row) = targets.arena.economy.castle_children.iter()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            row.source_class == 3 && row.life >= 0
        } else {
            targets.arena.economy.live_arena_kind(slot).is_none()
                && targets.arena.entities[usize::from(slot)].life >= 0
        }
    });
    let class3_chain = crate::campaign::source_arena::SourceSlotChain::capture(
        targets.arena, class3.clone(),
    );
    let model45_chain = crate::campaign::source_arena::SourceSlotChain::capture(
        targets.arena, model45.clone(),
    );
    let class5_slots = targets.arena.rebuilt_class5_model_slots();
    let class5_chains = std::array::from_fn(|model| {
        crate::campaign::source_arena::SourceSlotChain::capture(
            targets.arena, class5_slots[model].clone(),
        )
    });
    let model22_chain = class5_chains[22].clone();
    EffectLists {
        class3,
        model45,
        class3_chain,
        model45_chain,
        model22_chain,
        class5_chains,
        mana_chain: {
            let mut slots = targets
                .arena
                .entities
                .iter()
                .filter(|row| {
                    row.active
                        && row.class == 10
                        && matches!(row.model, 39 | 40 | 57)
                        && targets
                            .arena
                            .economy
                            .live_arena_cursor_row(row.index)
                            .is_none()
                })
                .map(|row| row.index)
                .collect::<BTreeSet<_>>();
            slots.extend(
                targets
                    .arena
                    .economy
                    .spheres
                    .iter()
                    .filter(|row| row.active)
                    .map(|row| row.source_entity_index),
            );
            slots.extend(
                targets
                    .arena
                    .economy
                    .player_death_markers
                    .iter()
                    .filter(|row| row.active)
                    .map(|row| row.source_entity_index),
            );
            crate::campaign::source_arena::SourceManaChain::capture(
                targets.arena, slots.into_iter().collect(),
            )
        },
        epoch: targets.arena.economy.replacement_list_epoch,
    }
}

/// Adapt the same live actor carriers to the distinct 10C80/11400 policies.
/// No effect-owned victim mirror or second damage mailbox is introduced.
#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct EffectDamageReceipt {
    pub(crate) source_slot: Option<u16>,
    pub(crate) source_scoped: bool,
    /// Current typed impact and its generation at the actual damage callback,
    /// before later passes can reclaim or replace the fixed source address.
    pub(crate) source_impact: Option<(u32, super::Level2Model17SpawnedChild)>,
    pub(crate) source_raw_impact: Option<(u32, super::EntitySnapshot)>,
    pub(crate) pulse: crate::world::effects::DamagePulse,
    pub(crate) target_slot: u16,
    pub(crate) target_generation: u32,
    pub(crate) target: Option<crate::combat::area::Actor>,
    pub(crate) target_in_scanned_cells: bool,
    pub(crate) mailbox_before: Option<(i32, u16)>,
    pub(crate) mailbox_after: Option<(i32, u16)>,
    pub(crate) hit_count: Option<u16>,
    /// Absolute append-only economy event boundary after the mailbox write.
    pub(crate) event_cursor: usize,
    pub(crate) writer_sequence: Option<u64>,
    writer_cursor: u64,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlayerMailboxSource {
    Contact(crate::campaign::source_arena::SourceEntityHandle),
    Effect(crate::campaign::source_arena::SourceEntityHandle),
    Other,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PlayerMailboxWrite {
    pub(crate) sequence: u64,
    pub(crate) target: crate::campaign::source_arena::SourceEntityHandle,
    pub(crate) source: PlayerMailboxSource,
    pub(crate) mailbox: (i32, u16),
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PlayerDamageConsumptionReceipt {
    pub(crate) target: crate::campaign::source_arena::SourceEntityHandle,
    pub(crate) mailbox: (i32, u16),
    pub(crate) writer: Option<PlayerMailboxWrite>,
    pub(crate) life_before: i32,
    pub(crate) life_after: i32,
    pub(crate) event_cursor: usize,
}

#[cfg(test)]
struct PlayerDamageTrace {
    targets: Vec<crate::campaign::source_arena::SourceEntityHandle>,
    sequence: u64,
    latest: Vec<PlayerMailboxWrite>,
    consumed: Vec<PlayerDamageConsumptionReceipt>,
}

#[cfg(test)]
thread_local! {
    static EFFECT_DAMAGE_TRACE: std::cell::RefCell<Option<(Vec<u16>, Vec<EffectDamageReceipt>)>> =
        const { std::cell::RefCell::new(None) };
    static FIREBALL_LAUNCH_TRACE: std::cell::RefCell<Option<Vec<(u16, u32, u16)>>> =
        const { std::cell::RefCell::new(None) };
    static FIREBALL_IMPACT_BIRTH_TRACE: std::cell::RefCell<Option<Vec<(u16, u32, u16, u32, u16)>>> =
        const { std::cell::RefCell::new(None) };
    static CURRENT_EFFECT_SOURCE: std::cell::Cell<Option<u16>> = const { std::cell::Cell::new(None) };
    static PLAYER_DAMAGE_TRACE: std::cell::RefCell<Option<PlayerDamageTrace>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn start_player_damage_trace(target: crate::campaign::source_arena::SourceEntityHandle) {
    start_actor_damage_trace(&[target]);
}

/// Watch the same canonical current mailboxes for exact actor generations.
/// Player checks keep using the single-target entry point above.
#[cfg(test)]
pub(crate) fn start_actor_damage_trace(
    targets: &[crate::campaign::source_arena::SourceEntityHandle],
) {
    assert!(targets.iter().enumerate().all(|(index, target)| {
        targets[..index].iter().all(|previous| previous.source_slot != target.source_slot)
    }), "one watched generation per physical source slot");
    PLAYER_DAMAGE_TRACE.with(|trace| {
        *trace.borrow_mut() = Some(PlayerDamageTrace {
            targets: targets.to_vec(),
            sequence: 0,
            latest: Vec::new(),
            consumed: Vec::new(),
        })
    });
}

/// Observe every accepted write to a watched mailbox, including
/// identical-value writes. Unattributed writers invalidate prior attribution.
#[cfg(test)]
pub(crate) fn trace_player_mailbox_write(
    arena: &SourceArena,
    slot: u16,
    source: Option<PlayerMailboxSource>,
) -> Option<u64> {
    PLAYER_DAMAGE_TRACE.with(|trace| {
        let mut trace = trace.borrow_mut();
        let trace = trace.as_mut()?;
        let target = *trace.targets.iter().find(|target| target.source_slot == slot)?;
        trace.sequence += 1;
        trace.latest.retain(|write| write.target.source_slot != slot);
        if arena.handle(slot) != Some(target) {
            return None;
        }
        let mailbox = if arena.economy.live_arena_cursor_row(slot).is_some() {
            read_arena_bounty(arena, slot)
                .expect("an accepted arena-owned write retains its canonical mailbox")
        } else {
            let row = &arena.entities[usize::from(slot)];
            (row.source_upkeep_payload(), row.source_upkeep_word_62())
        };
        let source = source.unwrap_or_else(|| {
            CURRENT_EFFECT_SOURCE.with(|current| {
                current
                    .get()
                    .and_then(|slot| arena.handle(slot))
                    .map_or(PlayerMailboxSource::Other, PlayerMailboxSource::Effect)
            })
        });
        trace.latest.push(PlayerMailboxWrite {
            sequence: trace.sequence,
            target,
            source,
            mailbox,
        });
        Some(trace.sequence)
    })
}

#[cfg(test)]
fn player_writer_sequence() -> u64 {
    PLAYER_DAMAGE_TRACE.with(|trace| trace.borrow().as_ref().map_or(0, |trace| trace.sequence))
}

#[cfg(test)]
pub(crate) fn latest_player_mailbox_writer() -> Option<PlayerMailboxWrite> {
    PLAYER_DAMAGE_TRACE.with(|trace| {
        let trace = trace.borrow();
        let trace = trace.as_ref()?;
        let target = trace.targets.first()?;
        trace.latest.iter().find(|write| write.target == *target).copied()
    })
}

/// Peek at the latest accepted writer; a callback may legitimately return
/// without consuming its mailbox, so observing entry must not retire it.
#[cfg(test)]
pub(crate) fn current_actor_mailbox_writer(
    target: crate::campaign::source_arena::SourceEntityHandle,
    mailbox: (i32, u16),
) -> Option<PlayerMailboxWrite> {
    PLAYER_DAMAGE_TRACE.with(|trace| {
        trace.borrow().as_ref()?.latest.iter()
            .find(|write| write.target == target && write.mailbox == mailbox)
            .copied()
    })
}

#[cfg(test)]
pub(crate) fn take_player_mailbox_writer(
    target: crate::campaign::source_arena::SourceEntityHandle,
    mailbox: (i32, u16),
) -> Option<PlayerMailboxWrite> {
    PLAYER_DAMAGE_TRACE.with(|trace| {
        let mut trace = trace.borrow_mut();
        let trace = trace.as_mut()?;
        let index = trace.latest.iter().position(|write| write.target.source_slot == target.source_slot)?;
        Some(trace.latest.remove(index))
            .filter(|write| write.target == target && write.mailbox == mailbox)
    })
}

#[cfg(test)]
pub(crate) fn trace_player_damage_consumption(receipt: PlayerDamageConsumptionReceipt) {
    PLAYER_DAMAGE_TRACE.with(|trace| {
        if let Some(trace) = trace.borrow_mut().as_mut()
            && trace.targets.contains(&receipt.target)
            && trace.consumed.len() < 64
        {
            trace.consumed.push(receipt);
        }
    });
}

#[cfg(test)]
pub(crate) fn take_player_damage_consumptions() -> Vec<PlayerDamageConsumptionReceipt> {
    PLAYER_DAMAGE_TRACE.with(|trace| {
        trace
            .borrow_mut()
            .as_mut()
            .map_or_else(Vec::new, |trace| std::mem::take(&mut trace.consumed))
    })
}

#[cfg(test)]
fn effect_receipt_writer(arena: &SourceArena, receipt: &EffectDamageReceipt) -> Option<u64> {
    let source = receipt.source_slot.and_then(|slot| arena.handle(slot))?;
    current_actor_mailbox_writer(
        crate::campaign::source_arena::SourceEntityHandle {
            source_slot: receipt.target_slot,
            generation: receipt.target_generation,
        },
        receipt.mailbox_after?,
    )
        .filter(|writer| {
            receipt.source_scoped
                && writer.sequence > receipt.writer_cursor
                && writer.target.source_slot == receipt.target_slot
                && writer.target.generation == receipt.target_generation
                && Some(writer.mailbox) == receipt.mailbox_after
                && writer.source == PlayerMailboxSource::Effect(source)
        })
        .map(|writer| writer.sequence)
}

#[cfg(test)]
pub(crate) struct EffectSourceTrace(Option<u16>);

#[cfg(test)]
impl Drop for EffectSourceTrace {
    fn drop(&mut self) { CURRENT_EFFECT_SOURCE.with(|source| source.set(self.0)); }
}

#[cfg(test)]
pub(crate) fn trace_effect_source(slot: u16) -> EffectSourceTrace {
    EffectSourceTrace(CURRENT_EFFECT_SOURCE.with(|source| source.replace(Some(slot))))
}

#[cfg(test)]
pub(crate) fn start_fireball_launch_trace() {
    FIREBALL_LAUNCH_TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
    FIREBALL_IMPACT_BIRTH_TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
}

#[cfg(test)]
pub(crate) fn trace_fireball_impact_birth(
    economy: &super::Level2EconomyState,
    impact: u16,
    parent: u16,
    owner: u16,
) {
    FIREBALL_IMPACT_BIRTH_TRACE.with(|trace| {
        if let Some(births) = trace.borrow_mut().as_mut() {
            births.push((impact, economy.slot_generation(impact), parent,
                economy.slot_generation(parent), owner));
        }
    });
}

#[cfg(test)]
pub(crate) fn take_fireball_impact_birth_trace() -> Vec<(u16, u32, u16, u32, u16)> {
    FIREBALL_IMPACT_BIRTH_TRACE.with(|trace| trace.borrow_mut().take().unwrap_or_default())
}

#[cfg(test)]
pub(crate) fn trace_fireball_launch(arena: &SourceArena, slot: u16, owner: u16) {
    FIREBALL_LAUNCH_TRACE.with(|trace| {
        if let Some(launches) = trace.borrow_mut().as_mut() {
            launches.push((slot, arena.economy.slot_generation(slot), owner));
        }
    });
}

#[cfg(test)]
pub(crate) fn take_fireball_launch_trace() -> Vec<(u16, u32, u16)> {
    FIREBALL_LAUNCH_TRACE.with(|trace| trace.borrow_mut().take().unwrap_or_default())
}

/// Record actual effect callbacks on the current test thread until taken.
#[cfg(test)]
pub(crate) fn start_effect_damage_trace(target_slot: u16) {
    start_effect_damage_trace_for_targets(&[target_slot]);
}

#[cfg(test)]
pub(crate) fn start_effect_damage_trace_for_targets(target_slots: &[u16]) {
    EFFECT_DAMAGE_TRACE.with(|trace| *trace.borrow_mut() = Some((target_slots.to_vec(), Vec::new())));
}

#[cfg(test)]
pub(crate) fn take_effect_damage_trace() -> Vec<EffectDamageReceipt> {
    EFFECT_DAMAGE_TRACE.with(|trace| trace.borrow_mut().take().map_or_else(Vec::new, |v| v.1))
}

/// Cursor in the currently enabled trace, before a source callback consumes
/// its mailbox. The caller retains its prior-turn base when draining traces.
#[cfg(test)]
pub(crate) fn effect_damage_trace_len() -> usize {
    EFFECT_DAMAGE_TRACE.with(|trace| {
        trace
            .borrow()
            .as_ref()
            .map_or(0, |(_, receipts)| receipts.len())
    })
}

#[cfg(test)]
fn trace_current_bounty(targets: &ArenaTargets<'_, '_>, slot: u16) -> Option<(i32, u16)> {
    // Match source_bounty and the actual writer: imported typed actors need
    // not publish their authoritative mailbox into a generic raw projection.
    targets
        .actors
        .as_ref()
        .and_then(|actors| actors.source_view(slot))
        .map(|view| (view.pending_damage, view.pending_attacker))
        .or_else(|| read_arena_bounty(targets.arena, slot).ok())
}

/// Observe the legacy class10/model0 callback at its actual writer boundary.
/// The world-effect adapter below is a different producer; it cannot witness
/// this callback merely because it also writes source damage mailboxes.
#[cfg(test)]
pub(crate) fn begin_model0_damage_trace(
    arena: &mut SourceArena,
    source_slot: u16,
    request: crate::world::effects::DamagePulse,
) -> Vec<EffectDamageReceipt> {
    let target_slots = EFFECT_DAMAGE_TRACE.with(|trace| {
        trace.borrow().as_ref().map(|(slots, _)| slots.clone())
    });
    let Some(target_slots) = target_slots else { return Vec::new(); };
    let source_impact = arena.economy.model17_spawned_children().iter()
        .find(|impact| impact.source_entity_index == source_slot)
        .copied()
        .map(|impact| (arena.economy.slot_generation(source_slot), impact));
    let targets = ArenaTargets { arena, actors: None, player: None };
    let center_x = (i32::from(request.position.x) + 128) / 256;
    let center_y = (i32::from(request.position.y) + 128) / 256;
    let radius = (i32::from(request.half_width) + 255) / 256;
    target_slots.into_iter().map(|target_slot| EffectDamageReceipt {
        source_slot: Some(source_slot),
        source_scoped: true,
        source_impact,
        source_raw_impact: None,
        pulse: request,
        target_slot,
        target_generation: targets.arena.economy.slot_generation(target_slot),
        target: targets.actor(target_slot),
        target_in_scanned_cells: (-radius..=radius).any(|y| {
            (-radius..=radius).any(|x| targets.cell_slots((center_x + x) as u8, (center_y + y) as u8)
                .contains(&target_slot))
        }),
        mailbox_before: trace_current_bounty(&targets, target_slot),
        mailbox_after: None,
        hit_count: None,
        event_cursor: 0,
        writer_sequence: None,
        writer_cursor: player_writer_sequence(),
    }).collect()
}

#[cfg(test)]
pub(crate) fn finish_model0_damage_trace(arena: &SourceArena, mut receipts: Vec<EffectDamageReceipt>) {
    for receipt in &mut receipts {
        receipt.mailbox_after = read_arena_bounty(arena, receipt.target_slot).ok();
        receipt.event_cursor = arena.economy.events().len();
        receipt.writer_sequence = effect_receipt_writer(arena, receipt);
    }
    EFFECT_DAMAGE_TRACE.with(|trace| {
        if let Some((_, recorded)) = trace.borrow_mut().as_mut() {
            recorded.extend(receipts);
        }
    });
}

pub(crate) fn apply_effect(
    arena: &mut SourceArena,
    actors: Option<ActorDomains<'_>>,
    player: Option<(u16, Position)>,
    masks: Option<&super::CollisionMaskBank>,
    lists: &EffectLists,
    request: crate::world::effects::DamagePulse,
) -> Result<u16, String> {
    let mut world = EffectTargets {
        targets: ArenaTargets {
            arena,
            actors,
            player,
        },
        masks,
        lists,
    };
    #[cfg(test)]
    let receipts = EFFECT_DAMAGE_TRACE.with(|trace| {
        let trace = trace.borrow();
        let Some((target_slots, _)) = trace.as_ref() else { return Vec::new(); };
        let mut matching_sources = world
            .targets
            .arena
            .entities
            .iter()
            .filter(|row| {
                row.active
                    && (row.class, row.model) == (10, 0)
                    && row.source_id == request.owner
                    && row.position == request.position
            })
            .map(|row| row.index);
        let first_source = matching_sources.next();
        let inferred_source = if matching_sources.next().is_none() {
            first_source
        } else {
            None
        };
        let scoped_source = CURRENT_EFFECT_SOURCE.with(|source| source.get());
        let source_slot = scoped_source.or(inferred_source);
        let center_x = (i32::from(request.position.x) + 128) / 256;
        let center_y = (i32::from(request.position.y) + 128) / 256;
        let radius = (i32::from(request.half_width) + 255) / 256;
        target_slots.iter().copied().map(|target_slot| {
        let target_in_scanned_cells = (-radius..=radius).any(|y| {
            (-radius..=radius).any(|x| {
                world
                    .cell_slots((center_x + x) as u8, (center_y + y) as u8)
                    .contains(&target_slot)
            })
        });
        EffectDamageReceipt {
            source_slot,
            source_scoped: scoped_source.is_some(),
            // The scoped callback also identifies a real Lightning endpoint.
            // Preserve its packet/generation for the ordinary Ray ancestry
            // check; source inference above remains limited to model0.
            source_raw_impact: source_slot.and_then(|slot| world.targets.arena.entities.get(usize::from(slot))
                .filter(|row| row.active && row.class == 10 && matches!(row.model, 0 | 23))
                .map(|row| (world.targets.arena.economy.slot_generation(slot), row.clone()))),
            source_impact: source_slot.and_then(|source_slot| {
                world.targets.arena.economy.model17_spawned_children().iter()
                    .find(|impact| impact.active && impact.source_entity_index == source_slot)
                    .copied()
                    .map(|impact| (
                        world.targets.arena.economy.slot_generation(source_slot),
                        impact,
                    ))
            }),
            pulse: request,
            target_slot,
            target_generation: world.targets.arena.economy.slot_generation(target_slot),
            target: world.actor(target_slot),
            target_in_scanned_cells,
            mailbox_before: trace_current_bounty(&world.targets, target_slot),
            mailbox_after: None,
            hit_count: None,
            event_cursor: 0,
            writer_sequence: None,
            writer_cursor: player_writer_sequence(),
        }
        }).collect::<Vec<_>>()
    });
    let result = crate::world::effects::apply_damage(&mut world, request);
    #[cfg(test)]
    for mut receipt in receipts {
        receipt.mailbox_after = trace_current_bounty(&world.targets, receipt.target_slot);
        receipt.hit_count = result.as_ref().ok().copied();
        receipt.event_cursor = world.targets.arena.economy.events().len();
        receipt.writer_sequence = effect_receipt_writer(world.targets.arena, &receipt);
        EFFECT_DAMAGE_TRACE.with(|trace| {
            if let Some((_, receipts)) = trace.borrow_mut().as_mut() {
                receipts.push(receipt);
            }
        });
    }
    result
}

struct EffectTargets<'arena, 'actors, 'assets> {
    targets: ArenaTargets<'arena, 'actors>,
    masks: Option<&'assets super::CollisionMaskBank>,
    lists: &'assets EffectLists,
}

impl AreaWorld for EffectTargets<'_, '_, '_> {
    fn class3_slots(&self) -> Vec<u16> {
        self.lists.class3_slots(self.targets.arena).to_vec()
    }

    fn cell_slots(&self, x: u8, y: u8) -> Vec<u16> {
        self.targets.cell_slots(x, y)
    }
    fn actor(&self, slot: u16) -> Option<Actor> {
        self.targets.actor(slot)
    }
    fn mark_model2(&mut self, slot: u16) -> Result<(), String> {
        self.targets.mark_model2(slot)
    }
    fn ordinary_damage(&mut self, slot: u16, payload: u16, owner: u16) -> Result<(), String> {
        self.targets.ordinary_damage(slot, payload, owner)
    }
}

impl crate::world::effects::DamageWorld for EffectTargets<'_, '_, '_> {
    fn model45_slots(&self) -> Vec<u16> {
        self.lists.model45_slots(self.targets.arena).to_vec()
    }

    fn model45_contact(
        &self,
        request: crate::world::effects::DamagePulse,
        slot: u16,
    ) -> Result<bool, String> {
        let Some(target) = self.targets.actor(slot) else {
            return Ok(false);
        };
        if !super::level2_compare_axis_with_shift(
            request.position,
            request.half_width,
            request.half_width,
            target.position,
            target.shape.half_width,
            target.shape.half_depth,
        ) {
            return Ok(false);
        }
        let selector = self
            .targets
            .actors
            .as_ref()
            .and_then(|domains| {
                domains
                    .houses
                    .houses()
                    .iter()
                    .find(|row| row.source_retained() && row.entity_index == slot)
                    .map(|row| row.building_index)
            })
            .or_else(|| {
                self.targets
                    .arena
                    .live_model45_buildings()
                    .into_iter()
                    .find(|(id, _, _)| *id == slot)
                    .map(|(_, selector, _)| selector)
            })
            .ok_or_else(|| {
                format!("effect model45 slot {slot} lost its retained building selector")
            })?;
        let bank = self.masks.ok_or_else(|| {
            "effect model45 contact requires the retained collision-mask bank".to_owned()
        })?;
        Ok(super::level2_model76_model45_occupancy(
            bank,
            u16::from(selector),
        ))
    }
}

fn shape(width: i16, depth: i16, offset: i16, height: i16) -> Shape {
    Shape {
        half_width: width,
        half_depth: depth,
        vertical_offset: offset,
        half_height: height,
    }
}

impl AreaWorld for ArenaTargets<'_, '_> {
    fn class3_slots(&self) -> Vec<u16> {
        let mut slots = self
            .arena
            .entities
            .iter()
            .filter(|row| row.active && row.class == 3 && row.index != 0)
            .map(|row| row.index)
            .collect::<BTreeSet<_>>();
        for castle in self.arena.economy.live_castles() {
            slots.insert(castle.source_entity_index);
        }
        // Preserve the imported intrusive class list, including the retained
        // topology of a Castle whose state has moved into its typed carrier.
        let linked = slots
            .iter()
            .filter_map(|slot| self.arena.entities.get(usize::from(*slot))?.next_relation)
            .collect::<BTreeSet<_>>();
        let mut order = Vec::new();
        let mut visited = BTreeSet::new();
        for head in slots
            .iter()
            .filter(|slot| !linked.contains(slot))
            .chain(slots.iter())
        {
            let mut current = *head;
            while slots.contains(&current) && visited.insert(current) {
                order.push(current);
                current = self
                    .arena
                    .entities
                    .get(usize::from(current))
                    .and_then(|row| row.next_relation)
                    .unwrap_or(0);
            }
        }
        order
    }

    fn cell_slots(&self, x: u8, y: u8) -> Vec<u16> {
        self.arena.economy.map_chains.entity_chain(x, y)
    }

    fn actor(&self, slot: u16) -> Option<Actor> {
        if slot == 0 {
            return None;
        }
        let economy = &self.arena.economy;
        // SetSmoke4 leaves byte38 zero. Wind still contacts these rows; its
        // geometry comes from the current particle, not the old raw occupant.
        if let Some(current) = economy.live_arena_cursor_row(slot)
            && matches!(
                current.kind,
                super::Level2DynamicArenaKind::Model59SmokeChild
                    | super::Level2DynamicArenaKind::Model13SpawnedChild
            )
        {
            let context = super::source_target::Context {
                particles: self.actors.as_ref().and_then(|actors| actors.particles),
                ..Default::default()
            };
            let view = super::source_target::read(self.arena, context, slot).ok()??;
            return Some(Actor {
                source_id: view.source_id,
                class: view.class,
                model: view.model,
                position: view.position,
                shape: view.collision?,
                damage_mask: 0,
                control_flags: view.control_flags,
            });
        }
        if let Some(actors) = self.actors.as_ref()
            && let Some(view) = actors.source_view(slot)
        {
            let mask = economy
                .authored_class5_runtime
                .iter()
                .find(|row| row.source_entity_index == slot)
                .map(|row| row.byte_0x38)
                .or_else(|| {
                    economy
                        .authored_model45
                        .iter()
                        .find(|row| row.source_entity_index == slot)
                        .map(|row| row.source_byte_38)
                })
                .unwrap_or_else(|| self.arena.entities[usize::from(slot)].source_byte_38());
            return Some(Actor {
                source_id: view.source_id,
                class: view.class,
                model: view.model,
                position: view.position,
                shape: shape(
                    view.collision_half_width,
                    view.collision_half_depth,
                    view.collision_vertical_offset,
                    view.collision_half_height,
                ),
                damage_mask: mask,
                control_flags: view.control_flags,
            });
        }
        let mut actor = read_arena_actor(self.arena, slot)?;
        if economy.live_arena_cursor_row(slot).is_none()
            && let Some((_, position)) = self.player.filter(|(index, _)| *index == slot)
        {
            actor.position = position;
        }
        Some(actor)
    }

    fn mark_model2(&mut self, slot: u16) -> Result<(), String> {
        if let Some(row) = self.arena.economy.castle_at_mut(slot) {
            row.source_word_30 = 30;
        } else {
            let row = self
                .arena
                .entities
                .get_mut(usize::from(slot))
                .filter(|row| row.active)
                .ok_or_else(|| format!("area model2 slot {slot} no longer live"))?;
            // Export header precedes the source word by four bytes. The
            // older source_word_30 helper addresses a different legacy view.
            row.activation_value = 30;
            row.record[52..54].copy_from_slice(&30_u16.to_le_bytes());
        }
        Ok(())
    }

    fn ordinary_damage(&mut self, slot: u16, payload: u16, owner: u16) -> Result<(), String> {
        self.submit_damage(slot, payload, owner, DamageWriter::Ordinary)
    }
}

impl ArenaTargets<'_, '_> {
    fn submit_damage(
        &mut self,
        slot: u16,
        payload: u16,
        owner: u16,
        writer: DamageWriter,
    ) -> Result<(), String> {
        let current_contact = matches!(writer, DamageWriter::Contact);
        if current_contact && super::source_target::current_castle_entry(self.arena, slot).is_some() {
            let economy = &mut self.arena.economy;
            let castle = economy
                .castle_entry_mut(slot)
                .ok_or_else(|| format!("contact Castle {slot} lost its current carrier"))?;
            DamageMailboxMut::new(
                &mut castle.pending_damage,
                &mut castle.pending_attacker_entity_index,
            )
            .submit(i32::from(payload), owner, writer);
            return Ok(());
        }
        if let Some(actors) = self.actors.as_mut()
            && actors.source_view(slot).is_some()
        {
            if !actors.submit(slot, payload, owner, writer) {
                return Err(format!(
                    "area live-world mailbox {slot} rejected an admitted target"
                ));
            }
            if let Some(goat) = actors
                .goats
                .goats()
                .iter()
                .find(|row| row.entity_index == slot)
            {
                self.arena.economy.sync_authored_goat_projection(goat);
            }
            return Ok(());
        }
        if self
            .arena
            .economy
            .house_residents
            .iter()
            .any(|row| row.active && row.source_entity_index == slot)
        {
            if !self
                .arena
                .economy
                .submit_settlement_damage(slot, i32::from(payload), owner, writer)
            {
                return Err(format!("area settlement slot {slot} rejected its mailbox"));
            }
            return Ok(());
        }
        self.edit_arena_bounty(slot, current_contact, |damage, attacker| {
            DamageMailboxMut::new(damage, attacker).submit(i32::from(payload), owner, writer);
        })
    }

    /// Canonical mailbox selection is shared by contact/ordinary writers and
    /// Gravity's full-width access; typed occupants never fall through to raw.
    fn edit_arena_bounty<R>(
        &mut self,
        slot: u16,
        current_contact: bool,
        edit: impl FnOnce(&mut i32, &mut u16) -> R,
    ) -> Result<R, String> {
        let economy = &mut self.arena.economy;
        macro_rules! smoke_bounty {
            ($rows:expr) => {
                if let Some(row) = $rows
                    .iter_mut()
                    .find(|row| row.active && row.source_entity_index == slot)
                {
                    return Ok(edit(
                        &mut row.pending_damage,
                        &mut row.pending_attacker_entity_index,
                    ));
                }
            };
        }
        smoke_bounty!(economy.model59_smoke_children);
        smoke_bounty!(economy.model13_spawned_children);
        if let Some(row) = economy
            .authored_trees
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            return Ok(edit(&mut row.pending_damage, &mut row.owner_entity_index));
        }
        if let Some(row) = economy
            .spheres
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            return Ok(edit(
                &mut row.ordinary_damage_payload,
                &mut row.ordinary_damage_owner,
            ));
        }
        if let Some(row) = economy
            .castle_defenders
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            return Ok(edit(
                &mut row.pending_damage,
                &mut row.pending_attacker_entity_index,
            ));
        }
        if let Some(row) = economy.castle_at_mut(slot) {
            return Ok(edit(
                &mut row.pending_damage,
                &mut row.pending_attacker_entity_index,
            ));
        }
        if let Some(row) = economy
            .bee_creatures
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            return Ok(edit(
                &mut row.pending_damage,
                &mut row.pending_attacker_entity_index,
            ));
        }
        if let Some(row) = economy
            .house_residents
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            return Ok(edit(
                &mut row.pending_damage,
                &mut row.attacker_entity_index,
            ));
        }
        if let Some(row) = economy
            .authored_dragon_parts
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            return Ok(edit(&mut row.pending_damage, &mut row.pending_word_0x62));
        }
        if let Some(row) = economy
            .authored_class5_model3_children
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            let result = edit(
                &mut row.damage_dword_0x5e_94,
                &mut row.pending_word_0x62,
            );
            #[cfg(test)]
            trace_player_mailbox_write(self.arena, slot, None);
            return Ok(result);
        }
        if let Some(row) = economy
            .authored_class5
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            if row.model == 9
                && !economy
                    .authored_class5_runtime
                    .iter()
                    .any(|state| state.source_entity_index == slot)
            {
                // The existing model9 constructor omits an empty ordinary
                // mailbox sidecar. Materialize that canonical mailbox on its
                // first real write, retaining the pinned constructor header.
                economy
                    .authored_class5_runtime
                    .push(super::Level2AuthoredClass5RuntimeState {
                        source_entity_index: slot,
                        byte_0x38: 1,
                        xtype: 3,
                        descriptor_index: 80,
                        pending_damage: 0,
                        linked_head: 0,
                    });
            }
            let damage = if let Some(state) = economy
                .authored_class5_archer
                .iter_mut()
                .find(|state| state.source_entity_index == slot)
            {
                &mut state.pending_damage
            } else if let Some(state) = economy
                .authored_class5_model3
                .iter_mut()
                .find(|state| state.source_entity_index == slot)
            {
                &mut state.damage_dword_0x5e_94
            } else {
                &mut economy
                    .authored_class5_runtime
                    .iter_mut()
                    .find(|state| state.source_entity_index == slot)
                    .ok_or_else(|| format!("area class5 slot {slot} has no ordinary mailbox"))?
                    .pending_damage
            };
            let result = edit(damage, &mut row.pending_word_0x62);
            #[cfg(test)]
            trace_player_mailbox_write(self.arena, slot, None);
            return Ok(result);
        }
        let retained_typed = current_contact
            && (economy.retained_projectile_kind(slot).is_some()
                || !economy.free_source_slots().contains(&slot)
                    && economy.speed_trails.iter().any(|row|
                        row.source_entity_index == slot && !row.active && row.control_flags & 0x400 != 0));
        if economy.live_arena_cursor_row(slot).is_some() || retained_typed {
            return Err(format!(
                "damage slot {slot} has no canonical mailbox writer"
            ));
        }
        let row = self
            .arena
            .entities
            .get_mut(usize::from(slot))
            .filter(|row| current_contact || row.active)
            .ok_or_else(|| format!("area raw slot {slot} no longer live"))?;
        let mut damage = row.source_wind_spell_payload();
        let mut attacker = row.source_wind_spell_owner();
        let result = edit(&mut damage, &mut attacker);
        row.record[98..102].copy_from_slice(&damage.to_le_bytes());
        row.record[102..104].copy_from_slice(&attacker.to_le_bytes());
        #[cfg(test)]
        trace_player_mailbox_write(self.arena, slot, None);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain::disasters::{RingProfile, mc2};

    #[test]
    fn first_map_v6_configured_builder_house_receives_real_model45_contact() {
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        use std::path::PathBuf;

        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        for selection in [CampaignSelection::LEVEL1, CampaignSelection::LEVEL3] {
            let package = std::env::var_os(format!(
                "MC_LEVEL{}_COLLISION_PACKAGE_V6",
                selection.external_level,
            ))
            .map_or_else(
                || {
                    generated.join(format!(
                        "settlement-20261106-opening-collision-v6/level{}",
                        selection.external_level,
                    ))
                },
                PathBuf::from,
            );
            let mut session = CampaignSession::open(package, selection)
                .expect("genuine closed first-map package reaches the ordinary session")
                .simulation()
                .clone();
            let owner = session.state.entity_index;
            let masks = session.collision_mask_bank().unwrap().clone();
            assert!(
                masks.mask(19).is_some(),
                "previously omitted Builder choice"
            );
            let candidate = Position {
                x: session.state.position.x.wrapping_add(8192),
                y: session.state.position.y.wrapping_add(8192),
                z: 0,
            };
            // Configured constructor control: select a genuine eligible
            // Builder row and use the exact retained family constructor and
            // publication sequence. This is not an ordinary Builder birth.
            let (handle, construction_visits) = {
                let live = &mut session.session.state.live;
                let parameter = live.ordinary.house_parameters[&19];
                let footprint = live.ordinary.house_build_footprints[&19].clone();
                let slot = live
                    .economy
                    .allocate_source_slot_with_terrain(&mut live.terrain)
                    .unwrap();
                let rng = live.economy.new_event_rng(slot);
                let position = super::super::creatures::settlements::builder_house_position(
                    &live.terrain,
                    candidate,
                    super::super::creatures::settlements::BuilderBuildingSpec {
                        sprite_width: 0,
                        sprite_height: 0,
                        footprint_width: footprint.width,
                        footprint_height: footprint.height,
                    },
                );
                let mut house = crate::house::HouseState::builder_from_source(
                    slot,
                    rng,
                    position,
                    19,
                    footprint.width,
                    footprint.height,
                    parameter,
                );
                live.ordinary.houses.insert_builder_house(house).unwrap();
                house.action = crate::house::HOUSE_TERRAIN_TO_LIVE_ACTION;
                live.ordinary
                    .houses
                    .initialize_builder_house(house)
                    .unwrap();
                live.economy.source_rng.insert(slot, rng);
                live.economy.source_owner.insert(slot, 0);
                live.economy.owner_class.insert(slot, 10);
                let mut raw = crate::world::terrain::cave_features::new_event(slot, rng, position);
                crate::house::bind_startup_construction(
                    &mut raw,
                    19,
                    parameter,
                    &footprint,
                    u16::from(footprint.width) * u16::from(footprint.height) * 4,
                );
                house.publish_source(&mut raw);
                live.economy.entities[usize::from(slot)] = raw;
                crate::world::effects::sync_position(
                    &mut live.economy,
                    &mut live.terrain,
                    slot,
                    position,
                    true,
                    false,
                )
                .unwrap();
                (live.economy.handle(slot).unwrap(), house.life as usize)
            };
            for pass in 0..construction_visits {
                session.step_level2_source_pass((pass % 2) as u8);
            }
            let house = *session
                .ordinary()
                .houses()
                .iter()
                .find(|house| house.entity_index == handle.source_slot)
                .unwrap();
            assert_eq!(
                session.source_arena().handle(handle.source_slot),
                Some(handle)
            );
            assert_eq!(house.action, crate::house::HOUSE_LIVE_ACTION);
            assert_eq!(
                house.life,
                i32::from(house.parameter.life_mana_scalar) * 1000
            );
            assert_eq!((house.pending_damage, house.pending_attacker), (0, 0));
            {
                let live = &mut session.session.state.live;
                let lists = capture_effect_lists(
                    &mut live.economy,
                    Some(ActorDomains::new(&mut live.goats, &mut live.ordinary)),
                );
                assert!(
                    apply_effect(
                        &mut live.economy,
                        Some(ActorDomains::new(&mut live.goats, &mut live.ordinary)),
                        None,
                        Some(&masks),
                        &lists,
                        crate::world::effects::DamagePulse {
                            position: house.position,
                            owner,
                            payload: 200,
                            half_width: 128,
                            vertical_offset: 0,
                            half_height: 128,
                            continuous: false,
                            selector: (10, 45),
                        },
                    )
                    .unwrap()
                        > 0
                );
            }
            let pending = session
                .ordinary()
                .houses()
                .iter()
                .find(|row| row.entity_index == handle.source_slot)
                .unwrap();
            assert_eq!(
                (pending.pending_damage, pending.pending_attacker),
                (200, owner.cast_signed())
            );
            assert_eq!(
                pending.life, house.life,
                "the actual writer queues the mailbox"
            );
            session.step_level2_source_pass(0);
            let consumed = session
                .ordinary()
                .houses()
                .iter()
                .find(|row| row.entity_index == handle.source_slot)
                .unwrap();
            assert_eq!(consumed.life, house.life - 200);
            assert_eq!((consumed.pending_damage, consumed.pending_attacker), (0, 0));
            assert_eq!(
                session.source_arena().handle(handle.source_slot),
                Some(handle)
            );
        }
    }

    #[test]
    fn model45_effect_damage_preserves_unsigned_probe_and_strict_axis_admission() {
        let mut session = super::super::tests::level_two_campaign_session();
        let owner = session.state.entity_index;
        let house = *session
            .ordinary()
            .houses()
            .iter()
            .find(|house| house.entity_index == 12)
            .expect("actual Day2 House12");
        assert_eq!((house.pending_damage, house.pending_attacker), (0, 0));
        assert_eq!(
            ((house.position.x >> 8) + (house.position.y >> 8)) & 1,
            1,
            "the failed public Fireball impact has odd source tile parity"
        );
        let masks = session.session.definition.collision_masks.clone().unwrap();
        let mask = masks.mask(u16::from(house.building_index)).unwrap();
        assert_eq!((mask.width(), mask.height()), (2, 1));
        let handle = session.source_arena().handle(house.entity_index).unwrap();
        let live = &mut session.session.state.live;
        let lists = capture_effect_lists(
            &mut live.economy,
            Some(ActorDomains::new(&mut live.goats, &mut live.ordinary)),
        );
        let mut request = crate::world::effects::DamagePulse {
            position: house.position,
            owner,
            payload: 200,
            half_width: 128,
            vertical_offset: 0,
            half_height: 128,
            continuous: false,
            selector: (10, 45),
        };
        assert!(
            apply_effect(
                &mut live.economy,
                Some(ActorDomains::new(&mut live.goats, &mut live.ordinary)),
                None,
                None,
                &lists,
                request,
            )
            .is_err(),
            "the source byte predicate still requires its admitted mask bank"
        );
        for outside_x in [true, false] {
            request.position = house.position;
            if outside_x {
                request.position.x = request
                    .position
                    .x
                    .wrapping_add_signed(house.collision_half_width + request.half_width);
            } else {
                request.position.y = request
                    .position
                    .y
                    .wrapping_add_signed(house.collision_half_depth + request.half_width);
            }
            apply_effect(
                &mut live.economy,
                Some(ActorDomains::new(&mut live.goats, &mut live.ordinary)),
                None,
                Some(&masks),
                &lists,
                request,
            )
            .unwrap();
            let current = live
                .ordinary
                .houses()
                .iter()
                .find(|row| row.entity_index == house.entity_index)
                .unwrap();
            assert_eq!(
                (current.pending_damage, current.pending_attacker),
                (0, 0),
                "strict XY rejects equality at the source collision boundary"
            );
        }
        request.position = house.position;
        assert!(
            apply_effect(
                &mut live.economy,
                Some(ActorDomains::new(&mut live.goats, &mut live.ordinary)),
                None,
                Some(&masks),
                &lists,
                request,
            )
            .unwrap()
                > 0
        );
        let current = live
            .ordinary
            .houses()
            .iter()
            .find(|row| row.entity_index == house.entity_index)
            .unwrap();
        assert_eq!(
            (current.pending_damage, current.pending_attacker),
            (200, owner.cast_signed()),
            "the actual model45 writer accepts the odd-parity House centre"
        );
        assert_eq!(
            current.life, house.life,
            "the later House callback owns consumption"
        );
        assert_eq!(live.economy.handle(house.entity_index), Some(handle));
    }

    #[test]
    fn rival_class5_heads_follow_real_castle_seed_allocation() {
        let session = super::super::tests::level_two_campaign_session();
        for case in ["normal", "typed replacement", "raw replacement", "failure"] {
            let mut arena = session.source_arena().clone();
            let mut terrain = session.session.state.live.terrain.clone();
            let position = session.state.position;
            for row in &mut arena.entities {
                row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
            }
            let old_tile = (0..=u16::MAX)
                .find(|tile| arena.economy.map_chains.tile_head(*tile) == 0).unwrap();
            let old_position = Position {
                x: ((old_tile as u8 as u16) << 8) + 128,
                y: ((old_tile >> 8) << 8) + 128,
                z: 128,
            };
            let raw_slot = if case == "raw replacement" {
                let slot = crate::world::effects::allocate(
                    &mut arena, &mut terrain,
                    sim_definition::adapter::mc2_effect_definition(crate::level_definition::EffectKind::Impact),
                    old_position, &session.session.definition.presentation.particles,
                ).unwrap().unwrap();
                assert!(arena.economy.typed_replacement_candidate().is_none(),
                    "the raw-only control has no typed allocation alternative");
                Some(slot)
            } else if case == "typed replacement" {
                // CreateManaSphere starts with flags12 and is not a
                // replacement candidate. SetSmoke4 explicitly produces bit17.
                let slot = arena.allocate_smoke13_with_terrain(old_position, &mut terrain).unwrap();
                let smoke = arena.economy.model59_smoke_children().iter()
                    .find(|row| row.source_entity_index == slot).unwrap();
                assert_eq!(smoke.control_flags & crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
                    crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
                assert_eq!(arena.economy.typed_replacement_candidate(), Some(slot));
                None
            } else {
                let sphere = arena.allocate_authored_model39(0, 0, position).unwrap();
                assert_eq!(sphere.control_flags, 12,
                    "the ordinary sphere constructor does not produce a replaceable row");
                None
            };
            arena.rebuild_source_free_arena();
            let lists = capture_effect_lists(&mut arena, None);
            let before = lists.class5_current_slots(&arena);
            assert!(before.iter().any(|bucket| !bucket.is_empty()));
            let epoch = arena.economy.source_list_epoch();
            let recycle = matches!(case, "typed replacement" | "raw replacement");
            let fail = case == "failure";
            let replacement = if recycle {
                raw_slot.or_else(|| arena.economy.typed_replacement_candidate())
            } else { None };
            if recycle {
                // Reserve actual free addresses without adding another
                // replaceable row. This isolates the real fallback allocator.
                while arena.economy.free_slot_count() != 0 {
                    arena.allocate_source_slot().unwrap();
                }
            } else if fail {
                arena.economy.exhaust_new_event_capacity_for_test();
                arena.finish_source_group();
            }
            let callback_rows = arena.entities.clone();
            let generations = callback_rows.iter().map(|row| arena.slot_generation(row.index))
                .collect::<Vec<_>>();
            let root = arena.allocate_ai_castle_root_seed(
                position, 112, 112, &mut terrain,
            );
            assert_eq!(root.is_none(), fail, "{case}");
            let after = lists.class5_current_slots_after_allocation(
                &arena.economy, &arena.entities,
            );
            if recycle {
                assert_eq!(Some(root.unwrap().source_entity_index), replacement);
                assert_ne!(arena.economy.source_list_epoch(), epoch);
                assert!(after.iter().all(Vec::is_empty),
                    "NewEvent fallback clears the actual source heads before the rival tail");
            } else {
                assert_eq!(arena.economy.source_list_epoch(), epoch);
                assert_eq!(after, before,
                    "ordinary allocation or clean failure retains pass-start membership");
            }
            // The callback's older ordinary view cannot undo NewEvent or
            // restore its predecessor's map links when it returns.
            arena.commit_rival_callback_rows(&callback_rows, &generations);
            if let Some(root) = root {
                let slot = root.source_entity_index;
                assert_eq!(arena.economy.live_arena_kind(slot), Some(super::super::Level2DynamicArenaKind::CastleRoot));
                assert!(!arena.entities[usize::from(slot)].active);
                assert_eq!(arena.entities[usize::from(slot)].class, 0);
                assert_eq!(arena.entities[usize::from(slot)].source_dword_10(), 0);
                let tile = arena.economy.map_chains.tile_of(slot);
                assert_eq!(terrain.planes().entity_indices[usize::from(tile)],
                    arena.economy.map_chains.tile_head(tile) as i16);
                if let Some(raw_slot) = raw_slot {
                    assert_eq!(slot, raw_slot);
                    assert_eq!(arena.slot_generation(slot), generations[usize::from(slot)].wrapping_add(1));
                    assert_ne!(tile, old_tile);
                    assert_eq!(arena.economy.map_chains.tile_head(old_tile), 0);
                    assert_eq!(terrain.planes().entity_indices[usize::from(old_tile)], 0);
                }
            }
        }
    }

    #[test]
    fn encounter_kill_uses_frozen_creatures_and_preserves_attackers_and_mailboxes() {
        let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-1pb-16-level2-full-build-v2");
        let campaign = crate::campaign::CampaignSession::open(
            package,
            crate::campaign::spec::CampaignSelection::LEVEL2,
        )
        .unwrap();
        let live = &campaign.simulation().session.state.live;
        let mut arena = live.economy.clone();
        let mut runtime = live.core.runtime.clone();
        let mut terrain = live.terrain.clone();
        let mut goats = live.goats.clone();
        let mut ordinary = live.ordinary.clone();
        let particles = &campaign
            .simulation()
            .session
            .definition
            .presentation
            .particles;
        let player = live.core.state.entity_index;
        let player_before = arena.entities[usize::from(player)].clone();
        let position = live.core.state.position;
        let bee = super::super::creatures::bee::mc2::allocate_authored(
            &mut arena,
            &mut terrain,
            2,
            position,
            1_000,
        )
        .unwrap()
        .source_entity_index;
        {
            let row = arena
                .economy
                .bee_creatures
                .iter_mut()
                .find(|row| row.source_entity_index == bee)
                .unwrap();
            row.entity_id = player; // KillAll also kills friendly source ids.
            row.damage_attacker_entity_index = 777;
            row.pending_damage = 321;
            row.pending_attacker_entity_index = 778;
        }
        let raw = super::super::creatures::creature_25::mc2::allocate_authored(
            &mut arena,
            &mut runtime,
            &mut terrain,
            particles,
            position,
            2_000,
            None,
            25,
        )
        .unwrap()
        .unwrap();
        arena.entities[usize::from(raw)].set_source_roll(123);
        arena.entities[usize::from(raw)].set_damage_result_words(1, 777);
        arena.entities[usize::from(raw)].record[98..102].copy_from_slice(&321_i32.to_le_bytes());
        arena.entities[usize::from(raw)].record[102..104].copy_from_slice(&778_u16.to_le_bytes());
        let chain = super::super::creatures::creature_27::mc2::allocate_authored(
            &mut arena,
            &mut runtime,
            &mut terrain,
            particles,
            position,
            27,
        )
        .unwrap()
        .unwrap();
        let root = chain.root.source_slot;
        let root_life = arena.entities[usize::from(root)].life;
        let body = chain.bodies[0][0].source_slot;
        let families = arena.rebuilt_class5_model_slots();
        let lists = capture_effect_lists(
            &mut arena,
            Some(ActorDomains::new(&mut goats, &mut ordinary)),
        );
        let later = super::super::creatures::bee::mc2::allocate_authored(
            &mut arena,
            &mut terrain,
            3,
            position,
            1_000,
        )
        .unwrap()
        .source_entity_index;
        assert!(lists.is_current(&arena));
        assert!(!families[2].contains(&later));

        let mut invalidated = arena.clone();
        invalidated.economy.replacement_list_epoch =
            invalidated.economy.replacement_list_epoch.wrapping_add(1);
        apply_encounter_creature_kills(&mut invalidated, None, &lists, &families, player).unwrap();
        assert_eq!(invalidated.entities[usize::from(raw)].life, 2_000);

        apply_encounter_creature_kills(
            &mut arena,
            Some(ActorDomains::new(&mut goats, &mut ordinary)),
            &lists,
            &families,
            player,
        )
        .unwrap();
        let bee = arena
            .economy
            .bee_creatures
            .iter()
            .find(|row| row.source_entity_index == bee)
            .unwrap();
        assert_eq!(
            (
                bee.life,
                bee.kill_credit_entity_index,
                bee.damage_attacker_entity_index
            ),
            (-1, player, 777)
        );
        assert_eq!(
            (bee.pending_damage, bee.pending_attacker_entity_index),
            (321, 778)
        );
        let raw = &arena.entities[usize::from(raw)];
        assert_eq!(
            (raw.life, raw.word_24(), raw.word_26(), raw.source_roll()),
            (-1, player, 777, 123)
        );
        assert_eq!(&raw.record[98..104], &[65, 1, 0, 0, 10, 3]);
        assert_eq!(
            (
                arena.entities[usize::from(root)].action,
                arena.entities[usize::from(root)].life
            ),
            (221, root_life)
        );
        assert!(
            chain
                .heads
                .iter()
                .all(|head| arena.entities[usize::from(head.source_slot)].action == 221)
        );
        assert_eq!(arena.entities[usize::from(body)].action, 234);
        assert_eq!(arena.entities[usize::from(player)], player_before);
        assert_eq!(
            arena
                .economy
                .bee_creatures
                .iter()
                .find(|row| row.source_entity_index == later)
                .unwrap()
                .life,
            1_000
        );
    }

    #[test]
    fn encounter_mana_chain_mutates_real_carriers_and_reclaims_hidden_markers() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.session.state.live.terrain.clone();
        let particles = &session.session.definition.presentation.particles;
        let player = session.state.entity_index;
        let player_before = arena.entities[usize::from(player)].clone();
        let sphere = arena
            .economy
            .spawn_mana_sphere_from_death(
                999,
                session.state.position,
                0,
                0x1234,
                500,
                player,
                &terrain,
            )
            .unwrap() as u16;
        let marker = crate::native_world::creatures::player_lifecycle::construct_death_marker(
            &mut arena, &mut terrain, player, session.state.position,
        ).unwrap().unwrap().source_entity_index;
        let lists = capture_effect_lists(&mut arena, None);
        let later = crate::native_world::creatures::player_lifecycle::construct_death_marker(
            &mut arena, &mut terrain, player, session.state.position,
        ).unwrap().unwrap().source_entity_index;

        let before_flags = arena.economy.reset_hash();
        apply_encounter_world_action(
            &mut arena,
            &lists,
            EncounterManaAction::Approach(1),
            particles,
        )
        .unwrap();
        assert_ne!(arena.economy.reset_hash(), before_flags);
        for slot in [sphere, marker] {
            let context = super::super::source_target::Context {
                particles: Some(particles),
                ..Default::default()
            };
            let current = super::super::source_target::read(&arena, context, slot)
                .unwrap()
                .unwrap();
            assert_ne!(current.control_flags & 0x0100_0000, 0);
            assert_eq!(current.control_flags & 0x0080_0000, 0);
        }
        apply_encounter_world_action(
            &mut arena,
            &lists,
            EncounterManaAction::Approach(2),
            particles,
        )
        .unwrap();
        apply_encounter_world_action(&mut arena, &lists, EncounterManaAction::Refresh, particles)
            .unwrap();
        assert_eq!(
            arena
                .economy
                .spheres
                .iter()
                .find(|row| row.source_entity_index == sphere)
                .map(|row| (row.life, row.max_life)),
            Some((140, 140))
        );
        let current = arena
            .economy
            .player_death_markers
            .iter()
            .find(|row| row.source_entity_index == marker)
            .unwrap();
        assert_eq!((current.life, current.max_life), (140, 140));
        assert_eq!(current.control_flags & 0x8000_2000, 0x8000_2000);
        assert_eq!(arena.entities[usize::from(player)], player_before);
        assert_eq!(
            arena
                .economy
                .player_death_markers
                .iter()
                .find(|row| row.source_entity_index == later)
                .unwrap()
                .control_flags,
            12
        );

        // A source fallback clears every list head. An otherwise identical
        // callback must leave the same canonical carriers alone afterward.
        let mut invalidated = arena.clone();
        invalidated.economy.replacement_list_epoch =
            invalidated.economy.replacement_list_epoch.wrapping_add(1);
        apply_encounter_world_action(
            &mut invalidated,
            &lists,
            EncounterManaAction::Approach(3),
            particles,
        )
        .unwrap();
        assert_eq!(
            invalidated
                .economy
                .player_death_markers
                .iter()
                .find(|row| row.source_entity_index == marker)
                .unwrap()
                .control_flags
                & ENTITY_DRAW_DISABLED_BIT,
            0
        );

        apply_encounter_world_action(
            &mut arena,
            &lists,
            EncounterManaAction::Approach(3),
            particles,
        )
        .unwrap();
        assert!(
            arena
                .economy
                .player_death_markers
                .iter()
                .any(|row| row.source_entity_index == marker && row.active)
        );
        assert!(arena.economy.map_chains.is_linked(marker));
        retire_hidden_player_death_markers(&mut arena, &mut terrain).unwrap();
        arena.economy.reclaim_inactive_slots_at_source_boundary();
        assert!(!arena.economy.map_chains.is_linked(marker));
        assert!(arena.economy.free_source_slots().contains(&marker));
        assert!(
            !arena
                .economy
                .player_death_markers
                .iter()
                .any(|row| row.source_entity_index == marker)
        );
        let retained = &arena.entities[usize::from(marker)];
        assert!(!retained.active);
        assert_eq!(
            (retained.class, retained.life, retained.max_life),
            (0, 140, 140)
        );
        assert_eq!(retained.position, session.state.position);
    }

    #[test]
    fn gravity_bounty_preserves_full_signed_mailbox_and_typed_authority() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let bee = arena
            .economy
            .allocate_authored_bee(
                1,
                session.state.position,
                None,
                3000,
                &session.session.state.live.terrain,
            )
            .expect("authored Day Bee birth");
        let slot = bee.source_entity_index;
        // Raw bytes are a projection, not this live creature's mailbox.
        arena.entities[usize::from(slot)].record[98..102].copy_from_slice(&123_i32.to_le_bytes());
        replace_source_bounty(&mut arena, None, slot, 100_001, 0).unwrap();
        assert_eq!(source_bounty(&arena, None, slot).unwrap(), (100_001, 0));
        let (payload, _) = source_bounty(&arena, None, slot).unwrap();
        replace_source_bounty(&mut arena, None, slot, payload.wrapping_add(3001), u16::MAX)
            .unwrap();
        let typed = arena
            .economy
            .bee_creatures
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(
            (typed.pending_damage, typed.pending_attacker_entity_index),
            (103_002, u16::MAX)
        );
        assert_eq!(
            arena.entities[usize::from(slot)].source_wind_spell_payload(),
            123
        );
        replace_source_bounty(&mut arena, None, slot, i32::MIN, 0).unwrap();
        assert_eq!(source_bounty(&arena, None, slot).unwrap(), (i32::MIN, 0));

        let raw_slot = session.state.entity_index;
        replace_source_bounty(&mut arena, None, raw_slot, -70_000, u16::MAX).unwrap();
        assert_eq!(
            source_bounty(&arena, None, raw_slot).unwrap(),
            (-70_000, u16::MAX)
        );
        assert!(replace_source_bounty(&mut arena, None, u16::MAX, 1, 2).is_err());

        let tree = arena
            .economy
            .allocate_authored_tree(
                u16::MAX,
                session.state.position,
                &session.session.definition.presentation.particles,
            )
            .expect("source Tree constructor");
        let raw = &mut arena.entities[usize::from(tree.source_entity_index)];
        raw.set_position(Position::default());
        raw.set_life(-400);
        raw.set_control_flags(0);
        raw.record[98..102].copy_from_slice(&123_i32.to_le_bytes());
        let shadow = raw.record.clone();
        let current =
            source_actor(&mut arena, None, tree.source_entity_index).expect("current Tree actor");
        assert_eq!(
            current.shape,
            shape(tree.pitch, tree.roll, tree.yaw, tree.fov)
        );
        assert_eq!(current.damage_mask, tree.source_byte_38);
        assert_eq!(
            source_life(&arena, None, tree.source_entity_index),
            Some(tree.life)
        );
        let request = AreaDamage {
            position: tree.position,
            shape: shape(64, 64, tree.yaw, tree.fov),
            owner: session.state.entity_index,
            payload: 400,
            selector: (tree.source_class, tree.model),
            mode: 0,
        };
        assert!(apply(&mut arena, None, None, request).unwrap() != 0);
        assert_eq!(
            source_bounty(&arena, None, tree.source_entity_index).unwrap(),
            (400, session.state.entity_index)
        );
        assert_eq!(
            arena.entities[usize::from(tree.source_entity_index)].record,
            shadow
        );
        replace_source_bounty(&mut arena, None, tree.source_entity_index, 0, 0).unwrap();
        apply(
            &mut arena,
            None,
            None,
            AreaDamage {
                owner: tree.entity_id,
                ..request
            },
        )
        .unwrap();
        assert_eq!(
            source_bounty(&arena, None, tree.source_entity_index).unwrap(),
            (0, 0)
        );
    }

    #[test]
    fn selected_contact_uses_authored_bee_geometry_and_mailbox() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let bee = arena
            .economy
            .allocate_authored_bee(
                1,
                session.state.position,
                None,
                3000,
                &session.session.state.live.terrain,
            )
            .expect("authored Day Bee birth");
        let slot = bee.source_entity_index;
        let target = source_actor(&mut arena, None, slot).expect("current Bee carrier");
        assert_eq!(target.shape.half_width, 128);
        assert_eq!(target.shape.half_height, 128);
        assert_eq!(target.shape.vertical_offset, 75);
        contact(&mut arena, None, slot, 200, session.state.entity_index).unwrap();
        let current = arena
            .economy
            .bee_creatures
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(current.pending_damage, 200);
        assert_eq!(
            current.pending_attacker_entity_index,
            session.state.entity_index
        );
        assert_eq!(current.life, bee.life);
    }

    #[test]
    fn construction_burial_reaches_the_live_bee_carrier_and_callback() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let terrain = session.canonical_terrain();
        let (x, y) = (0_u8..=u8::MAX)
            .flat_map(|y| (0_u8..=u8::MAX).map(move |x| (x, y)))
            .find(|&(x, y)| arena.economy.map_chains.entity_chain(x, y).is_empty())
            .expect("owner terrain has an unoccupied source cell");
        let position = Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            ..session.state.position
        };
        let bee = arena
            .economy
            .allocate_authored_bee(1, position, None, 3000, terrain)
            .expect("ordinary authored Bee constructor");
        let slot = bee.source_entity_index;
        let generation = arena.slot_generation(slot);
        start_construction_kill_trace();
        crate::world::terrain::disasters::mc2::burial::cell(&mut arena, None, x, y, bee.entity_id)
            .unwrap();
        assert_eq!(
            arena
                .economy
                .bee_creatures
                .iter()
                .find(|row| row.source_entity_index == slot)
                .unwrap()
                .life,
            bee.life
        );
        let owner = session.state.entity_index;
        assert_ne!(owner, bee.entity_id);
        crate::world::terrain::disasters::mc2::burial::cell(&mut arena, None, x, y, owner).unwrap();
        let killed = arena
            .economy
            .bee_creatures
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(
            (
                killed.life,
                killed.kill_credit_entity_index,
                killed.damage_attacker_entity_index
            ),
            (-1, owner, owner)
        );
        // A different source can rewrite an already-dead retained Bee's
        // credit; the first writer remains the only lethal one.
        let later_owner = (1_u16..).find(|candidate| *candidate != owner && *candidate != bee.entity_id)
            .expect("another source identity");
        crate::world::terrain::disasters::mc2::burial::cell(&mut arena, None, x, y, later_owner).unwrap();
        assert_eq!(take_construction_kill_trace(), vec![
            (slot, generation, owner, Some(3000)),
            (slot, generation, later_owner, Some(-1)),
        ]);
        let rewritten = arena.economy.bee_creatures.iter()
            .find(|row| row.source_entity_index == slot).unwrap();
        assert_eq!((rewritten.life, rewritten.kill_credit_entity_index, rewritten.damage_attacker_entity_index),
            (-1, later_owner, later_owner));
        assert!(arena.economy.step_authored_bee_slot(slot, None, terrain));
        let after = arena
            .economy
            .bee_creatures
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(after.action, 20, "the ordinary callback reaches pre-kill");
        assert_eq!(
            after.kill_credit_entity_index, 0,
            "an empty pending owner clears the current attacker before lethal credit"
        );
    }

    #[test]
    fn selected_contact_preserves_raw_mailbox_rules_without_readmission() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let slot = session.state.entity_index;
        let row = &mut arena.entities[usize::from(slot)];
        let owner = row.source_id;
        // An already-selected target may share source identity. An ownerless
        // payload is residue, not an empty mailbox, and addition wraps.
        row.record[98..102].copy_from_slice(&i32::MAX.to_le_bytes());
        row.record[102..104].copy_from_slice(&0_u16.to_le_bytes());
        contact(&mut arena, None, slot, 1, owner).unwrap();
        assert_eq!(
            arena.entities[usize::from(slot)].source_wind_spell_payload(),
            i32::MIN
        );
        assert_eq!(
            arena.entities[usize::from(slot)].source_wind_spell_owner(),
            owner
        );
        contact(&mut arena, None, slot, 200, owner).unwrap();
        assert_eq!(
            arena.entities[usize::from(slot)].source_wind_spell_payload(),
            200
        );
        ArenaTargets {
            arena: &mut arena,
            actors: None,
            player: None,
        }
        .ordinary_damage(slot, 8, owner)
        .unwrap();
        assert_eq!(
            arena.entities[usize::from(slot)].source_wind_spell_payload(),
            208
        );
        contact(&mut arena, None, slot, 0, 0).unwrap();
        assert_eq!(
            arena.entities[usize::from(slot)].source_wind_spell_payload(),
            0
        );
        assert_eq!(
            arena.entities[usize::from(slot)].source_wind_spell_owner(),
            0
        );
        assert!(contact(&mut arena, None, u16::MAX, 200, owner).is_err());
    }

    #[test]
    fn selected_contact_keeps_current_castle_free_record_and_new_event_owners_distinct() {
        use super::super::source_target::{self, Context};

        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let owner = session.state.entity_index;
        let sentinel = arena.entities[0].clone();
        assert!(contact(&mut arena, None, 0, 60, owner).is_err());
        assert!(contact(&mut arena, None, u16::MAX, 60, owner).is_err());
        assert_eq!(arena.entities[0], sentinel);
        let slot = arena
            .economy
            .build_castle(session.state.position, owner, &terrain)
            .unwrap();
        let generation = arena.slot_generation(slot);
        // Configured source-pointer exceptions, not natural alias reachability.
        // The real Castle's current carrier disagrees with its cold raw row.
        let raw = &mut arena.entities[usize::from(slot)];
        raw.set_position(Position { x: 1, y: 2, z: 3 });
        raw.set_life(888);
        raw.record[98..102].copy_from_slice(&777_i32.to_le_bytes());
        raw.record[102..104].copy_from_slice(&999_u16.to_le_bytes());
        let shadow = raw.clone();
        let castle = arena.economy.castle_entry_mut(slot).unwrap();
        castle.pending_damage = 11;
        castle.pending_attacker_entity_index = 0;
        contact(&mut arena, None, slot, 60, 31).unwrap();
        let castle = arena.economy.castle_entry(slot).unwrap();
        assert_eq!(
            (castle.pending_damage, castle.pending_attacker_entity_index),
            (71, 31)
        );
        assert_eq!(arena.entities[usize::from(slot)], shadow);

        let castle = arena.economy.castle_entry_mut(slot).unwrap();
        castle.active = false;
        castle.life = -1;
        castle.source_yaw = 321;
        castle.source_roll = -456;
        castle.pending_damage = i32::MAX;
        castle.pending_attacker_entity_index = 0;
        let expected = *castle;
        assert!(arena.economy.live_arena_kind(slot).is_none());
        assert!(!arena.economy.free_source_slots().contains(&slot));
        let view = source_target::read(&arena, Context::default(), slot)
            .unwrap()
            .unwrap();
        assert_eq!(
            (view.active, view.class, view.model, view.source_id),
            (
                false,
                0,
                expected.source_model,
                expected.source_id_entity_index
            )
        );
        assert_eq!(
            (view.position, view.life, view.yaw, view.roll),
            (expected.position, -1, Some(321), Some(-456))
        );
        let axes = view.collision.unwrap();
        assert_eq!(
            (
                axes.half_width,
                axes.half_depth,
                axes.vertical_offset,
                axes.half_height
            ),
            (
                expected.shift_pitch,
                expected.shift_roll,
                expected.shift_yaw,
                expected.shift_fov
            )
        );
        contact(&mut arena, None, slot, 1, 31).unwrap();
        let castle = arena.economy.castle_entry(slot).unwrap();
        assert_eq!(
            (castle.pending_damage, castle.pending_attacker_entity_index),
            (i32::MIN, 31)
        );
        contact(&mut arena, None, slot, 60, 32).unwrap();
        let castle = arena.economy.castle_entry(slot).unwrap();
        assert_eq!(
            (castle.pending_damage, castle.pending_attacker_entity_index),
            (60, 32)
        );
        assert_eq!(
            arena.entities[usize::from(slot)],
            shadow,
            "retained typed state owns this write"
        );

        // Publish and free this real address through the existing source
        // operations. An explicit free-record residue now belongs to raw,
        // even while the inactive Castle cache still has the old generation.
        arena.retain_castle_target(view);
        arena.economy.release_slot(slot, None);
        assert!(source_target::current_castle_entry(&arena, slot).is_none());
        let raw = &mut arena.entities[usize::from(slot)];
        raw.record[98..102].copy_from_slice(&i32::MAX.to_le_bytes());
        raw.record[102..104].fill(0);
        contact(&mut arena, None, slot, 1, 33).unwrap();
        let raw = &arena.entities[usize::from(slot)];
        assert_eq!(
            (
                raw.source_wind_spell_payload(),
                raw.source_wind_spell_owner()
            ),
            (i32::MIN, 33)
        );
        assert!(!raw.active);
        assert!(
            ArenaTargets {
                arena: &mut arena,
                actors: None,
                player: None
            }
            .ordinary_damage(slot, 1, 33)
            .is_err(),
            "Contact alone accepts the selected inactive raw row"
        );
        let old = arena.economy.castle_entry(slot).unwrap();
        assert_eq!(
            (old.pending_damage, old.pending_attacker_entity_index),
            (60, 32)
        );

        // A genuine subsequent NewEvent and constructor reuse the released
        // LIFO address. The generation barrier forbids mutating that Castle.
        let replacement = crate::world::effects::allocate(
            &mut arena,
            &mut terrain,
            sim_definition::adapter::mc2_effect_definition(crate::level_definition::EffectKind::Impact),
            session.state.position,
            &session.session.definition.presentation.particles,
        )
        .unwrap()
        .unwrap();
        assert_eq!(replacement, slot);
        assert_ne!(arena.slot_generation(slot), generation);
        assert!(arena.economy.castle_entry(slot).is_none());
        assert!(source_target::current_castle_entry(&arena, slot).is_none());
        let view = source_target::read(&arena, Context::default(), slot)
            .unwrap()
            .unwrap();
        assert_eq!((view.class, view.model), (10, 0));
        contact(&mut arena, None, slot, 60, 34).unwrap();
        let raw = &arena.entities[usize::from(slot)];
        assert_eq!(
            (
                raw.source_wind_spell_payload(),
                raw.source_wind_spell_owner()
            ),
            (60, 34)
        );
        let old = arena
            .economy
            .castle
            .0
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(
            (
                old.slot_generation,
                old.pending_damage,
                old.pending_attacker_entity_index
            ),
            (generation, 60, 32)
        );

        // A retained typed flyer still owns its fixed address. Its missing
        // ordinary mailbox is a checked error, never a write to cold raw.
        let flyer = arena
            .economy
            .spawn_fireball_projectile_with_payload(
                super::super::level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
                    origin: super::super::FireballProjectileOrigin::PlayerSpell,
                    position: session.state.position,
                    yaw: 0,
                    pitch: 0,
                    axis_position: Position::default(),
                    source_id_entity_index: owner,
                    holder_entity_index: 0,
                    source_subspell_index: 100,
                    source_mana: 50,
                    source_dword_0x10: 0,
                    target_source_entity_index: 0,
                    target_class: 0,
                    target_model: 0,
                },
                0,
            )
            .unwrap();
        let row = arena
            .economy
            .fireball_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == flyer)
            .unwrap();
        row.active = false;
        row.drawing_disabled = true;
        let shadow = arena.entities[usize::from(flyer)].clone();
        assert!(
            !source_target::read(&arena, Context::default(), flyer)
                .unwrap()
                .unwrap()
                .active
        );
        assert!(contact(&mut arena, None, flyer, 60, owner).is_err());
        assert_eq!(arena.entities[usize::from(flyer)], shadow);
    }

    #[test]
    fn selected_contact_updates_actual_sphere_carrier_not_shadow_record() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let sphere = arena
            .economy
            .allocate_authored_model39(1, session.state.entity_index, session.state.position)
            .expect("ordinary authored sphere birth");
        let slot = sphere.source_entity_index;
        let shadow = arena.entities[usize::from(slot)].clone();
        // Ordinary damage first owns the retained typed mailbox. Contact
        // replaces that hit; another ordinary hit then adds to Contact.
        ArenaTargets {
            arena: &mut arena,
            actors: None,
            player: None,
        }
        .ordinary_damage(slot, 123, session.state.entity_index)
        .unwrap();
        contact(&mut arena, None, slot, 200, session.state.entity_index).unwrap();
        ArenaTargets {
            arena: &mut arena,
            actors: None,
            player: None,
        }
        .ordinary_damage(slot, 8, session.state.entity_index)
        .unwrap();
        let current = arena
            .economy
            .spheres()
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(
            (
                current.ordinary_damage_payload,
                current.ordinary_damage_owner
            ),
            (208, session.state.entity_index)
        );
        assert_eq!(current.mana, sphere.mana);
        assert_eq!(arena.entities[usize::from(slot)].record, shadow.record);
    }

    fn ring_near(
        arena: &mut SourceArena,
        terrain: &mut super::super::SessionTerrain,
        target: Position,
    ) -> u16 {
        // Choose an actual eligible package tile within the default ring's
        // extent. No terrain, victim position, life or mailbox is staged.
        for radius in 0_i16..=7 {
            for y in -radius..=radius {
                for x in -radius..=radius {
                    let position = Position {
                        x: target.x.wrapping_add_signed(x * 256),
                        y: target.y.wrapping_add_signed(y * 256),
                        z: target.z,
                    };
                    let tile = usize::from(position.x >> 8) + 256 * usize::from(position.y >> 8);
                    if terrain.planes().angles[tile] & 15 != 0 {
                        let slot = mc2::construct(arena, terrain, position, RingProfile::default())
                            .expect("ordinary ring allocation");
                        let source = &arena.entities[usize::from(slot)];
                        assert_eq!((source.class, source.model, source.action), (10, 11, 11));
                        assert_eq!(source.record[67], 10, "the dispatch class is lossless");
                        return slot;
                    }
                }
            }
        }
        panic!("owner-data target has no eligible ring tile within range");
    }

    fn request(arena: &SourceArena, damage: mc2::AreaDamage) -> AreaDamage {
        let source = &arena.entities[usize::from(damage.source_slot)];
        AreaDamage {
            position: damage.position,
            owner: damage.owner,
            payload: damage.payload,
            shape: shape(
                damage.half_width,
                damage.half_depth,
                damage.vertical_offset,
                damage.half_height,
            ),
            selector: (source.record[69], source.record[70]),
            mode: 0,
        }
    }

    #[test]
    fn owner_package_ring_hits_player_creature_and_castle_before_their_consumers() {
        // The Level-2 start is sea: its first lowering clears the center's
        // angle and correctly terminates the next visit. Use the existing
        // Level-3 land start to exercise both ordinary payloads, preserving
        // the real package pose and terrain rather than forcing either.
        let root = std::env::var_os("MC_CAMPAIGN_PACKAGE_LEVEL3_V1").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3")
            },
            std::path::PathBuf::from,
        );
        let mut session = crate::campaign::CampaignSession::open(
            root,
            crate::campaign::spec::CampaignSelection::LEVEL3,
        )
        .expect("qualified Level-3 land boundary")
        .simulation()
        .clone();
        let player = session.state.entity_index;
        let position = session.state.position;
        let initial_player_life = session.canonical_player_life_for_test();
        let mut terrain = session.canonical_terrain().clone();
        let mut arena = session.source_arena().clone();
        let castle_slot = arena
            .economy
            .build_castle(position, player, &terrain)
            .expect("ordinary Castle constructor");
        let initial_castle_life = arena.economy.castle().unwrap().life;
        let firefly = arena
            .economy
            .allocate_authored_firefly(
                1,
                position,
                super::super::level2_class5::firefly::FireflyStageRoute {
                    stage_var1: 1,
                    stage_var2: 0,
                    target: position,
                },
                crate::level_definition::FIREFLY_CLASS_PROPERTIES.max_life as u32,
                &terrain,
            )
            .expect("ordinary Firefly constructor");
        let ring = ring_near(&mut arena, &mut terrain, position);
        let owner = arena.entities[usize::from(ring)].source_id;
        for expected_payload in [200, 8] {
            let source = &arena.entities[usize::from(ring)];
            let tile =
                usize::from(source.position.x >> 8) + 256 * usize::from(source.position.y >> 8);
            let before = (
                source.life,
                source.position,
                terrain.planes().angles[tile],
                terrain.planes().ground_heights[tile],
            );
            let outcome = mc2::step(&mut arena, &mut terrain, ring, |arena, damage| {
                assert_eq!(damage.payload, expected_payload);
                let request = request(arena, damage);
                let count = apply(arena, None, Some((player, position)), request)?;
                assert!(count >= 3, "the player, Firefly and Castle are recipients");
                // This callback is synchronous, before the new terrain band.
                assert_eq!(arena.economy.castle().unwrap().source_word_30, 30);
                assert_eq!(
                    arena.entities[usize::from(player)].source_wind_spell_owner(),
                    owner
                );
                Ok(count)
            })
            .unwrap();
            assert!(
                outcome.terrain_mutated,
                "ring payload {expected_payload}: before={before:?}, outcome={outcome:?}, after_angle={}, after_height={}, player_pending={}",
                terrain.planes().angles[tile],
                terrain.planes().ground_heights[tile],
                arena.entities[usize::from(player)].source_wind_spell_payload()
            );
            assert!(outcome.hit_count >= 3);
        }
        assert_eq!(arena.economy.castle().unwrap().pending_damage, 208);
        assert_eq!(
            arena.entities[usize::from(player)].source_wind_spell_payload(),
            208
        );
        let firefly_lifecycle = super::super::level2_class5::firefly::step_lifecycle_in_arena(
            &mut arena,
            &mut terrain,
            &session.session.definition.presentation.particles,
            firefly.source_entity_index,
            &[],
        )
        .unwrap();
        assert!(firefly_lifecycle.handled);
        let after = arena
            .economy
            .authored_class5()
            .iter()
            .find(|row| row.source_entity_index == firefly.source_entity_index)
            .unwrap();
        assert_eq!(after.life, firefly.life - 208);
        assert_eq!(after.word_0x26, owner);
        *session.source_arena_mut() = arena;
        *session.canonical_terrain_mut() = terrain;
        assert_eq!(
            session.consume_level2_player_damage(),
            Some((208, owner, false))
        );
        assert_eq!(
            session.canonical_player_life_for_test(),
            initial_player_life - 208
        );
        assert_eq!(
            session
                .source_arena_mut()
                .economy
                .resolve_damage(castle_slot),
            Some(false)
        );
        assert_eq!(
            session.economy().castle().unwrap().life,
            initial_castle_life - 208
        );
    }

    #[test]
    fn death_sphere_contact_geometry_changes_only_at_its_color_callback() {
        let session = super::super::tests::level_two_campaign_session();
        let terrain = session.canonical_terrain();
        let mut arena = session.source_arena().clone();
        let id = arena
            .economy
            .spawn_mana_sphere_from_death(
                session.state.entity_index,
                session.state.position,
                0,
                0x1234,
                8000,
                0,
                terrain,
            )
            .expect("ordinary death-to-sphere producer");
        let slot = u16::try_from(id).unwrap();
        let newborn = source_actor(&mut arena, None, slot).unwrap();
        assert_eq!(
            newborn.shape,
            shape(28, 28, 28, 28),
            "copied death mana must not resize constructor geometry before its visit"
        );
        assert!(
            arena
                .economy
                .step_mana_sphere_slot(slot, terrain, &mut arena.entities, |_| 52,)
        );
        let current = arena
            .economy
            .spheres()
            .iter()
            .find(|row| row.source_entity_index == slot)
            .copied()
            .unwrap();
        assert!(current.active);
        assert_ne!(current.particle_index, 53);
        assert_ne!(current.collision_pitch, 28);
        let visited = source_actor(&mut arena, None, slot).unwrap();
        assert_eq!(
            visited.shape,
            shape(
                current.collision_pitch,
                current.collision_roll,
                current.collision_yaw,
                current.collision_fov,
            ),
            "contact admission follows the callback's retained geometry"
        );
    }

    #[test]
    fn ordinary_sphere_birth_retains_area_mailbox_and_hit_count_without_mana_loss() {
        let session = super::super::tests::level_two_campaign_session();
        let terrain = session.canonical_terrain();
        let mut arena = session.source_arena().clone();
        let sphere = arena
            .economy
            .allocate_authored_model39(1, session.state.entity_index, session.state.position)
            .expect("ordinary authored mana-sphere constructor");
        assert_eq!(
            sphere.control_flags & 8,
            8,
            "NewEvent's damage admission bit"
        );
        assert_eq!(
            (sphere.ordinary_damage_payload, sphere.ordinary_damage_owner),
            (0, 0)
        );
        let mut request = AreaDamage {
            position: sphere.position,
            shape: shape(1, 1, sphere.collision_yaw, 1),
            owner: sphere.source_entity_index,
            payload: 200,
            selector: (10, 39),
            mode: 0,
        };
        assert_eq!(apply(&mut arena, None, None, request).unwrap(), 0);
        let zero_hash = arena.economy.reset_hash();
        request.owner = session.state.entity_index;
        for (payload, total) in [(200, 200), (8, 208)] {
            request.payload = payload;
            assert_eq!(apply(&mut arena, None, None, request).unwrap(), 1);
            let current = arena
                .economy
                .spheres()
                .iter()
                .find(|row| row.source_entity_index == sphere.source_entity_index)
                .unwrap();
            assert_eq!(
                (
                    current.ordinary_damage_payload,
                    current.ordinary_damage_owner
                ),
                (total, request.owner)
            );
            assert_eq!(current.mana, sphere.mana);
            assert_eq!(current.owner_entity_index, sphere.owner_entity_index);
            assert_eq!(current.incoming_possession_payload, 0);
            assert_eq!(current.incoming_owner_entity_index, 0);
        }
        assert_ne!(arena.economy.reset_hash(), zero_hash);
        assert!(arena.economy.step_mana_sphere_slot(
            sphere.source_entity_index,
            terrain,
            &mut arena.entities,
            |_| 52,
        ));
        let current = arena
            .economy
            .spheres()
            .iter()
            .find(|row| row.source_entity_index == sphere.source_entity_index)
            .unwrap();
        assert!(current.active);
        assert_eq!(
            (
                current.ordinary_damage_payload,
                current.ordinary_damage_owner
            ),
            (208, request.owner),
            "the actual Sphere callback retains ordinary damage words"
        );
        assert_eq!(current.mana, sphere.mana);
        let mut cleared = arena.economy.clone();
        let current = cleared
            .spheres
            .iter_mut()
            .find(|row| row.source_entity_index == sphere.source_entity_index)
            .unwrap();
        current.ordinary_damage_payload = 0;
        current.ordinary_damage_owner = 0;
        assert_ne!(
            cleared.reset_hash(),
            arena.economy.reset_hash(),
            "the authoritative retained mailbox participates in the source hash"
        );
    }

    #[test]
    fn buried_dragon_parts_transfer_last_nonzero_killer_before_root_credit() {
        let session = super::super::tests::level_two_campaign_session();
        let terrain = session.canonical_terrain();
        let mut arena = session.source_arena().clone();
        let player = session.state.entity_index;
        let root = arena
            .economy
            .allocate_authored_class5(1, 0, session.state.position, terrain)
            .expect("ordinary Dragon constructor and copied body chain");
        let earlier_owner = arena
            .entities
            .iter()
            .find(|row| {
                row.active
                    && row.source_id != 0
                    && row.source_id != player
                    && row.source_id != root.source_entity_index
            })
            .expect("another actual source identity from the owner package")
            .source_id;
        let mut slot = arena
            .economy
            .authored_class5_runtime
            .iter()
            .find(|row| row.source_entity_index == root.source_entity_index)
            .unwrap()
            .linked_head;
        let mut chain = Vec::new();
        while slot != 0 {
            let part = arena
                .economy
                .authored_dragon_parts
                .iter()
                .find(|row| row.source_entity_index == slot)
                .unwrap();
            assert_eq!(
                part.word_0x24, root.word_0x24,
                "the body copied its root killer word"
            );
            chain.push(slot);
            slot = part.next_entity_index;
        }
        assert_eq!(chain.len(), 16);
        assert!(apply_construction_kill(&mut arena, None, chain[0], earlier_owner).unwrap());
        assert!(apply_construction_kill(&mut arena, None, chain[15], player).unwrap());
        assert!(
            arena
                .economy
                .step_authored_dragon_roam_slot(root.source_entity_index, &[], &[], terrain, &[])
                .unwrap()
        );
        let source = arena
            .economy
            .authored_class5
            .iter()
            .find(|row| row.source_entity_index == root.source_entity_index)
            .unwrap();
        assert_eq!(
            (source.life, source.word_0x24, source.action),
            (-1, earlier_owner, 4),
            "minimum life selects the first lower child before PreKill"
        );
        let transition = arena
            .economy
            .step_authored_dragon_lifecycle_slot(root.source_entity_index, terrain)
            .unwrap();
        assert!(transition.handled);
        assert_eq!(
            transition.pre_kill_credit,
            Some(player),
            "PreKill resolves the last nonzero child killer before the counter consumer"
        );
        assert!(
            arena
                .economy
                .authored_dragon_parts
                .iter()
                .filter(|row| chain.contains(&row.source_entity_index))
                .all(|row| row.action == 5)
        );
        let direct_root = arena
            .economy
            .allocate_authored_class5(2, 0, session.state.position, terrain)
            .expect("a second ordinary Dragon constructor");
        assert!(
            apply_construction_kill(&mut arena, None, direct_root.source_entity_index, player)
                .unwrap()
        );
        assert!(
            arena
                .economy
                .step_authored_dragon_roam_slot(
                    direct_root.source_entity_index,
                    &[],
                    &[],
                    terrain,
                    &[]
                )
                .unwrap()
        );
        let source = arena
            .economy
            .authored_class5
            .iter()
            .find(|row| row.source_entity_index == direct_root.source_entity_index)
            .unwrap();
        assert_eq!(
            (source.life, source.action),
            (-1, 4),
            "direct negative life reaches death without a fresh mailbox or lower child"
        );
        assert_eq!(
            source.word_0x24, 0,
            "source upkeep clears the ownerless current attacker before assigning root credit"
        );
        let transition = arena
            .economy
            .step_authored_dragon_lifecycle_slot(direct_root.source_entity_index, terrain)
            .unwrap();
        assert_eq!(transition.pre_kill_credit, None);
    }

    #[test]
    fn raw_model2_timer_precedes_owner_gate_and_does_not_touch_legacy_word() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let slot = session.state.entity_index;
        let row = &mut arena.entities[usize::from(slot)];
        // Contract test only: exercise the source's special model-2 branch
        // independently of the package's ordinary player model.
        row.model = 2;
        row.record[68] = 2;
        let before_legacy_word = row.record[48..50].to_vec();
        let request = AreaDamage {
            position: row.position,
            shape: shape(1, 1, 0, 1),
            owner: row.source_id,
            payload: 0,
            selector: (5, 4),
            mode: 0,
        };
        assert_eq!(apply(&mut arena, None, None, request).unwrap(), 0);
        let row = &arena.entities[usize::from(slot)];
        assert_eq!(row.source_spell_word_30(), 30);
        assert_eq!(&row.record[48..50], before_legacy_word.as_slice());
        assert_eq!(row.source_wind_spell_owner(), 0);
    }

    #[test]
    fn worm_area_owner_exclusion_uses_copied_root_identity() {
        let session = super::super::tests::level_two_campaign_session();
        let mut terrain = session.canonical_terrain().clone();
        let mut arena = session.source_arena().clone();
        let root = crate::native_world::creatures::worm::dynamic::construct_authored(
                &mut arena, &mut terrain, 1, session.state.position, 9000,
                &session.session.definition.presentation.particles)
            .expect("ordinary Worm root and body constructor");
        let body_slots = arena
            .economy
            .authored_class5_model3_children
            .iter()
            .filter(|body| body.parent_entity_index == root.source_entity_index)
            .map(|body| body.source_entity_index)
            .collect::<Vec<_>>();
        assert_eq!(body_slots.len(), 16);
        let watched = std::iter::once(root.source_entity_index)
            .chain(body_slots.iter().copied())
            .map(|slot| arena.handle(slot).expect("actual constructed Worm generation"))
            .collect::<Vec<_>>();
        start_actor_damage_trace(&watched);
        let mut request = AreaDamage {
            position: root.position,
            shape: shape(2304, 2304, 0, 8192),
            owner: root.source_entity_index,
            payload: 200,
            selector: (5, 3),
            mode: 0,
        };
        assert_eq!(
            apply(&mut arena, None, None, request).unwrap(),
            0,
            "the root id excludes its whole copied chain"
        );
        assert!(watched.iter().all(|handle| {
            current_actor_mailbox_writer(*handle, (0, 0)).is_none()
        }), "owner exclusion cannot manufacture an accepted writer");
        assert!(
            arena
                .economy
                .authored_class5_model3_children
                .iter()
                .filter(|body| body_slots.contains(&body.source_entity_index))
                .all(|body| body.damage_dword_0x5e_94 == 0 && body.pending_word_0x62 == 0)
        );
        request.owner = session.state.entity_index;
        assert_eq!(
            apply(&mut arena, None, None, request).unwrap(),
            17,
            "a hostile field reaches the root and all sixteen real bodies"
        );
        for &handle in &watched {
            let writer = current_actor_mailbox_writer(handle, (200, request.owner))
                .expect("the actual typed mailbox write must be observed");
            assert_eq!(writer.target, handle);
            assert_eq!(writer.source, PlayerMailboxSource::Other,
                "this configured direct field has no invented effect producer");
            let raw = &arena.entities[usize::from(handle.source_slot)];
            assert_ne!((raw.source_upkeep_payload(), raw.source_upkeep_word_62()), writer.mailbox,
                "the observer reads the typed owner rather than its retired raw packet");
        }
        assert!(
            arena
                .economy
                .authored_class5_model3_children
                .iter()
                .filter(|body| body_slots.contains(&body.source_entity_index))
                .all(|body| body.damage_dword_0x5e_94 == 200
                    && body.pending_word_0x62 == request.owner)
        );
        let source = arena
            .economy
            .authored_class5
            .iter()
            .find(|row| row.source_entity_index == root.source_entity_index)
            .unwrap();
        assert_eq!(source.pending_word_0x62, request.owner);
        assert_eq!(
            arena
                .economy
                .authored_class5_model3
                .iter()
                .find(|row| row.source_entity_index == root.source_entity_index)
                .unwrap()
                .damage_dword_0x5e_94,
            200
        );
        let child_slot = body_slots[0];
        assert!(
            !apply_construction_kill(&mut arena, None, child_slot, root.source_entity_index)
                .unwrap()
        );
        assert!(apply_construction_kill(&mut arena, None, child_slot, request.owner).unwrap());
        let child = arena
            .economy
            .authored_class5_model3_children
            .iter()
            .find(|row| row.source_entity_index == child_slot)
            .unwrap();
        assert_eq!(
            (child.life, child.word_0x24, child.word_0x26),
            (-1, request.owner, request.owner)
        );
        assert_eq!(
            (child.damage_dword_0x5e_94, child.pending_word_0x62),
            (200, request.owner),
            "burial preserves the real ordinary mailbox"
        );
        assert!(arena.economy.step_authored_class5_model3_slot_with_target(
            root.source_entity_index,
            None,
            &terrain
        ));
        let source = arena
            .economy
            .authored_class5
            .iter()
            .find(|row| row.source_entity_index == root.source_entity_index)
            .unwrap();
        assert_eq!(
            (source.life, source.word_0x24, source.action),
            (-1, request.owner, 28),
            "the root consumes its buried child's actual life and credit"
        );
        assert!(arena.economy.step_authored_class5_model3_slot_with_target(
            root.source_entity_index,
            None,
            &terrain
        ));
        assert!(
            arena
                .economy
                .authored_class5_model3_children
                .iter()
                .filter(|row| body_slots.contains(&row.source_entity_index))
                .all(|row| row.action == 29),
            "the next root callback propagates the linked kill tail"
        );
    }
}
