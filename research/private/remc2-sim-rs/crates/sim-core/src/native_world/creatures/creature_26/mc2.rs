//! Source-arena boundary for model26. The numeric row remains authoritative;
//! this adapter resolves generation-bound peers and commits each callback's
//! actual mana, holder, score, and terminal effects before returning.

use super::{
    Actor, Context, Effects, ManaDrain, RULES, SOURCE_MODEL, SpellHand, SpellOffer, SpellTheft,
    Target, step,
};
use crate::Position;
use crate::campaign::source_arena::{SourceArena, SourceEntityHandle, SourceEntityView};
use crate::native_world::{
    ENTITY_DRAW_DISABLED_BIT, EntitySnapshot, EntitySpecialRelation, Level2DynamicArenaKind,
    Level2SharedSpellResetSession, ParticleParameter, PlayerSnapshot, RuntimeSnapshot,
    SessionTerrain,
};
use crate::renderable_map_chain::RenderableMapSeed;

fn word(row: &EntitySnapshot, at: usize) -> u16 {
    u16::from_le_bytes([row.record[at], row.record[at + 1]])
}

fn put(row: &mut EntitySnapshot, at: usize, value: u16) {
    row.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

/// Build the ordinary sub_4CF00 row after its source slot has been allocated.
/// The caller owns allocation and generation registration; this routine owns
/// the complete model26 constructor payload and advances runtime counter26.
pub(crate) fn construct_row(
    slot: u16,
    seed: u16,
    position: Position,
    runtime: &mut RuntimeSnapshot,
    particles: &[ParticleParameter],
) -> Result<EntitySnapshot, String> {
    let particle = particles
        .get(usize::from(super::PARTICLE))
        .ok_or_else(|| "model26 requires retained particle318".to_owned())?;
    let animation_limit = particle
        .animation_limit()
        .ok_or_else(|| "model26 particle318 has invalid animation mode".to_owned())?;

    let counter =
        &mut runtime.bytes[crate::native_world::RUNTIME_BYTES + usize::from(SOURCE_MODEL)];
    let phase = *counter;
    *counter = counter.wrapping_add(1);
    let rng = seed.wrapping_mul(9377).wrapping_add(9439);
    let yaw = ((rng & 0x07ff).wrapping_sub(1)) as i16;
    let mut row = crate::native_world::cave_features::new_event(slot, seed, position);
    row.class = 5;
    row.model = SOURCE_MODEL;
    row.source_id = slot;
    row.parameter_relation = Some(super::DESCRIPTOR);
    put(&mut row, 30, slot);
    row.record[67] = 5;
    row.record[68] = SOURCE_MODEL;
    row.record[69] = 3;
    row.record[60] = 1;
    row.record[61] = 64;
    row.record[164..168].copy_from_slice(&(u32::from(super::DESCRIPTOR) + 1).to_le_bytes());
    row.set_entity_rng(rng);
    row.set_source_action(super::ROAM_ACTION);
    row.set_phase(phase);
    row.set_yaw(yaw);
    row.set_pitch(yaw);
    row.set_source_roll(yaw);
    row.set_fov(0);
    row.set_max_life(RULES.max_life);
    row.set_life(RULES.max_life);
    row.set_mana(RULES.initial_mana);
    row.set_subspell_index(RULES.attack_damage);
    row.record[136..138].copy_from_slice(&RULES.minimum_speed.to_le_bytes());
    row.record[138..140].copy_from_slice(&RULES.maximum_speed.to_le_bytes());
    row.set_actual_speed(RULES.maximum_speed);
    row.particle_parameter_index = Some(super::PARTICLE);
    row.record[94..96].copy_from_slice(&super::PARTICLE.to_le_bytes());
    row.record[96] = 0;
    row.record[97] = animation_limit;
    // SetEntityIndexAndRot_49CD0 initializes array_0x52_82 from particle318;
    // SetEntityShiftRot_49EA0 then replaces its pitch, roll, and fov while
    // retaining the particle-derived array yaw.
    row.record[86..88].copy_from_slice(&((particle.rotation_speed / 2) as i16).to_le_bytes());
    row.record[88..90].copy_from_slice(&RULES.collision_half_width.to_le_bytes());
    row.record[90..92].copy_from_slice(&RULES.collision_half_width.to_le_bytes());
    row.record[92..94].copy_from_slice(&RULES.collision_half_height.to_le_bytes());

    // sub_293D0 establishes the target sentinel, idle-speed flag, and maximum
    // speed. The constructor does not call sub_1EEE0 or alter the input height.
    put(&mut row, 154, 0);
    row.set_control_flags(row.control_flags | 0x800000);
    Ok(row)
}

fn offer(
    arena: &SourceArena,
    shared: &Level2SharedSpellResetSession,
    player: &PlayerSnapshot,
    raw_family: i16,
) -> Option<SpellOffer> {
    let family = u8::try_from(raw_family).ok()?;
    let holder_slot = *player.enabled_spell_holders.get(usize::from(family))?;
    if holder_slot == 0 {
        return None;
    }
    let holder = arena.handle(holder_slot)?;
    let holder_thief = arena
        .entities
        .get(usize::from(holder_slot))
        .filter(|row| row.active)
        .map(|row| word(row, 58))
        .or_else(|| shared.pickup(holder_slot).map(|row| word(row, 58)))?;
    Some(SpellOffer {
        family,
        holder,
        holder_thief,
    })
}

/// sub_28FF0 reads hands through the current victim's A4 pointer. The
/// separate source word94 is a mana/ownership relation, not that pointer.
pub(crate) fn player_state<'a>(
    row: &EntitySnapshot,
    shared: &'a Level2SharedSpellResetSession,
) -> Option<&'a PlayerSnapshot> {
    if !row.active || row.class != 3 || !matches!(row.model, 0 | 1) {
        return None;
    }
    let EntitySpecialRelation::PlayerState(slot) = row.special_relation else {
        return None;
    };
    shared.players.iter()
        .find(|player| player.slot == slot && player.entity_index == row.index)
        .or_else(|| (shared.local_player.slot == slot && shared.local_player.entity_index == row.index)
            .then_some(&shared.local_player))
}

fn target(
    arena: &SourceArena,
    shared: &Level2SharedSpellResetSession,
    slot: u16,
) -> Option<Target> {
    let row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && row.index == slot)?;
    let handle = arena.handle(slot)?;
    let player = player_state(row, shared);
    let player_entity = player.map_or(0, |player| player.entity_index);
    let left_spell =
        player.and_then(|player| offer(arena, shared, player, player.left_spell_slot_raw));
    let right_spell =
        player.and_then(|player| offer(arena, shared, player, player.right_spell_slot_raw));
    Some(Target {
        handle,
        slot,
        source_id: row.source_id,
        class: row.class,
        model: row.model,
        action: row.action,
        life: row.life,
        flags: row.control_flags,
        position: row.position,
        yaw: row.yaw,
        roll: row.source_roll(),
        player_entity,
        mana: row.mana,
        mana_regen: row.mana_regen,
        left_spell,
        right_spell,
        target: word(row, 154),
        next: word(row, 56),
        parent: word(row, 44),
        stage_mode: row.record[77],
        link: word(row, 54),
        attacker: word(row, 42),
        killer: word(row, 40),
        actual_speed: row.actual_speed,
        maximum_speed: i16::from_le_bytes([row.record[138], row.record[139]]),
    })
}

fn targets(arena: &SourceArena, shared: &Level2SharedSpellResetSession) -> Vec<Target> {
    arena
        .entities
        .iter()
        .filter(|row| row.active && row.index != 0 && row.class != 0)
        .filter_map(|row| target(arena, shared, row.index))
        .collect()
}

fn apply_mana_drain(
    arena: &mut SourceArena,
    shared: &mut Level2SharedSpellResetSession,
    drain: ManaDrain,
) -> bool {
    if arena.resolve(drain.target).is_none() {
        return false;
    }
    let Some(row) = arena
        .entities
        .get_mut(usize::from(drain.target.source_slot))
        .filter(|row| {
            row.active
                && row.class == 3
                && matches!(row.model, 0 | 1)
                && row.life >= 0
                && row.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
        })
    else {
        return false;
    };
    let mut mana = row.mana.wrapping_sub(drain.amount);
    if mana < 0 {
        mana = 0;
    }
    row.set_mana(mana);
    let updated = row.clone();
    if shared.player_entity.index == updated.index {
        shared.sync_player_entity_from_authority(&updated);
        shared.player_mana = mana;
        shared.caster.mana = mana;
    }
    true
}

fn apply_theft_to_player(player: &mut PlayerSnapshot, theft: SpellTheft) -> bool {
    let family = usize::from(theft.family);
    if !player.set_enabled_spell_holder_index(family, 0) {
        return false;
    }
    if player.right_spell_slot_raw == i16::from(theft.family) {
        player.right_spell_slot_raw = -1;
        player.right_spell_slot = 0;
        player.record[556..558].copy_from_slice(&(-1_i16).to_le_bytes());
    }
    if player.left_spell_slot_raw == i16::from(theft.family) {
        player.left_spell_slot_raw = -1;
        player.left_spell_slot = 0;
        player.record[554..556].copy_from_slice(&(-1_i16).to_le_bytes());
    }
    true
}

fn holder_row_matches(row: &EntitySnapshot, slot: u16, expected: &EntitySnapshot) -> bool {
    row.active
        && row.index == slot
        && row.class == 15
        && row.model == expected.model
        && row.action == expected.action
        && row.entity_rng == expected.entity_rng
}

fn dynamic_holder_identity_matches(
    arena: &SourceArena,
    kind: Level2DynamicArenaKind,
    slot: u16,
    holder: &EntitySnapshot,
) -> bool {
    match kind {
        Level2DynamicArenaKind::SpellPickup => arena.economy.spell_pickups().iter().any(|source| {
            source.active
                && source.source_entity_index == slot
                && source.family == holder.model
                && source.entity_rng == holder.entity_rng
        }),
        Level2DynamicArenaKind::SpellReplacement => {
            arena.economy.spell_replacements().iter().any(|source| {
                source.active
                    && source.source_entity_index == slot
                    && source.family == holder.model
                    && source.entity_rng == holder.entity_rng
            })
        }
        _ => false,
    }
}

fn dynamic_holder_matches(
    arena: &SourceArena,
    kind: Level2DynamicArenaKind,
    slot: u16,
    holder: &EntitySnapshot,
) -> bool {
    if !dynamic_holder_identity_matches(arena, kind, slot, holder) {
        return false;
    }
    match kind {
        Level2DynamicArenaKind::SpellPickup => arena.economy.spell_pickups().iter().any(|source| {
            source.active && source.source_entity_index == slot && source.action == holder.action
        }),
        Level2DynamicArenaKind::SpellReplacement => {
            arena.economy.spell_replacements().iter().any(|source| {
                source.active
                    && source.source_entity_index == slot
                    && source.action == holder.action
            })
        }
        _ => false,
    }
}

fn sync_dynamic_holder_from_raw(
    arena: &mut SourceArena,
    kind: Level2DynamicArenaKind,
    source: &EntitySnapshot,
) -> bool {
    let slot = source.index;
    match kind {
        Level2DynamicArenaKind::SpellPickup => {
            let Some(pickup) = arena.economy.spell_pickups.iter_mut().find(|pickup| {
                pickup.active
                    && pickup.source_entity_index == slot
                    && pickup.family == source.model
                    && pickup.entity_rng == source.entity_rng
            }) else {
                return false;
            };
            pickup.action = source.action;
            pickup.position = source.position;
            pickup.phase = source.phase;
            true
        }
        Level2DynamicArenaKind::SpellReplacement => {
            let Some(replacement) = arena.economy.spell_replacements.iter_mut().find(|row| {
                row.active
                    && row.source_entity_index == slot
                    && row.family == source.model
                    && row.entity_rng == source.entity_rng
            }) else {
                return false;
            };
            replacement.action = source.action;
            replacement.position = source.position;
            replacement.phase = source.phase;
            replacement.control_flags = source.control_flags;
            true
        }
        _ => false,
    }
}

/// A detached holder is dispatched through its current imported action-78
/// row. Keep its dynamic sidecar current on the next source pass, including
/// the callback's returned pickup action and byte-0x3E position.
fn sync_detached_holder_projections(arena: &mut SourceArena) {
    let dynamic_slots = arena
        .economy
        .live_arena_slots()
        .into_iter()
        .filter(|(_, kind)| {
            matches!(
                kind,
                Level2DynamicArenaKind::SpellPickup | Level2DynamicArenaKind::SpellReplacement
            )
        })
        .collect::<Vec<_>>();
    for (slot, kind) in dynamic_slots {
        let Some(raw) = arena
            .entities
            .get(usize::from(slot))
            .filter(|row| row.active && row.index == slot && row.class == 15)
            .cloned()
        else {
            continue;
        };
        let detached_action = match kind {
            Level2DynamicArenaKind::SpellPickup => arena
                .economy
                .spell_pickups()
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .is_some_and(|row| row.action == 78),
            Level2DynamicArenaKind::SpellReplacement => arena
                .economy
                .spell_replacements()
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .is_some_and(|row| row.action == 78),
            _ => false,
        };
        if (raw.action == 78 || detached_action)
            && dynamic_holder_identity_matches(arena, kind, slot, &raw)
        {
            let _ = sync_dynamic_holder_from_raw(arena, kind, &raw);
        }
    }
}

/// Commit sub_69300 against the generation-checked holder and actual victim
/// PlayerSnapshot, then publish the current row to every live carrier.
pub(super) fn apply_spell_theft(
    arena: &mut SourceArena,
    shared: &mut Level2SharedSpellResetSession,
    theft: SpellTheft,
) -> bool {
    let distinct = theft.thief.source_slot != theft.victim.source_slot
        && theft.thief.source_slot != theft.holder.source_slot
        && theft.victim.source_slot != theft.holder.source_slot;
    if !distinct || arena.resolve(theft.thief).is_none() || arena.resolve(theft.victim).is_none() {
        return false;
    }
    let Some(thief) = arena
        .entities
        .get(usize::from(theft.thief.source_slot))
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
    else {
        return false;
    };
    let thief_slot = thief.index;
    let Some(victim) = arena
        .entities
        .get(usize::from(theft.victim.source_slot))
        .filter(|row| {
            row.active
                && row.class == 3
                && row.model == 0
                && row.life >= 0
                && row.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
                && row.position == theft.victim_position
        })
    else {
        return false;
    };
    let victim_position = victim.position;
    let Some(player) = player_state(victim, shared)
        .filter(|player| player.entity_index == theft.victim_player)
    else {
        return false;
    };
    let player_slot = player.slot;
    let player_index = shared
        .players
        .iter()
        .position(|player| player.slot == player_slot);
    let local_only =
        player_index.is_none() && shared.local_player.slot == player_slot;
    if player_index.is_none() && !local_only {
        return false;
    }
    let player = player_index
        .map(|index| &shared.players[index])
        .unwrap_or(&shared.local_player);
    let expected_hand = i16::from(theft.family);
    let selected_hand = match theft.hand {
        SpellHand::Left => player.left_spell_slot_raw,
        SpellHand::Right => player.right_spell_slot_raw,
    };
    if selected_hand != expected_hand
        || player.enabled_spell_holders.get(usize::from(theft.family))
            != Some(&theft.holder.source_slot)
    {
        return false;
    }
    let right_selected = player.right_spell_slot_raw == expected_hand;
    let left_selected = player.left_spell_slot_raw == expected_hand;
    let holder_slot = usize::from(theft.holder.source_slot);
    if holder_slot >= arena.entities.len() {
        return false;
    }
    let holder_view = match arena.resolve(theft.holder) {
        Some(SourceEntityView::Imported(holder)) => {
            if !holder.active
                || holder.index != theft.holder.source_slot
                || holder.class != 15
                || holder.model != theft.family
            {
                return false;
            }
            None
        }
        Some(SourceEntityView::Dynamic(
            kind @ (Level2DynamicArenaKind::SpellPickup | Level2DynamicArenaKind::SpellReplacement),
        )) => Some(kind),
        _ => return false,
    };
    let shared_holder = shared.pickup(theft.holder.source_slot).cloned();
    let raw_holder = arena
        .entities
        .get(holder_slot)
        .filter(|row| row.active && row.index == theft.holder.source_slot);
    let mut holder = match (holder_view, raw_holder) {
        (None, Some(raw)) if raw.model == theft.family => raw.clone(),
        (Some(kind), Some(raw)) => {
            let Some(shared_holder) = shared_holder
                .as_ref()
                .filter(|holder| holder_row_matches(holder, theft.holder.source_slot, raw))
            else {
                return false;
            };
            if !holder_row_matches(raw, theft.holder.source_slot, shared_holder)
                || !dynamic_holder_matches(arena, kind, theft.holder.source_slot, raw)
            {
                return false;
            }
            raw.clone()
        }
        (Some(kind), None) => {
            let Some(holder) = shared_holder.as_ref().filter(|holder| {
                holder.active
                    && holder.index == theft.holder.source_slot
                    && holder.class == 15
                    && holder.model == theft.family
                    && dynamic_holder_matches(arena, kind, theft.holder.source_slot, holder)
            }) else {
                return false;
            };
            holder.clone()
        }
        _ => {
            // A live typed holder may not overwrite a different active raw
            // occupant, and a generation-valid imported holder needs its row.
            return false;
        }
    };
    if holder.model != theft.family || word(&holder, 58) != 0 {
        return false;
    }
    let mut player_after = player.clone();
    if !apply_theft_to_player(&mut player_after, theft) {
        return false;
    }
    put(&mut holder, 42, thief_slot);
    holder.set_source_action(78);
    holder.set_control_flags(holder.control_flags & !1);
    holder.record[20..24].fill(0);
    holder.set_position(victim_position);
    put(&mut holder, 78, 0);
    if right_selected {
        put(&mut holder, 78, 1);
    }
    if left_selected {
        put(&mut holder, 78, 2);
    }
    if arena
        .economy
        .map_chains
        .sync_entity(RenderableMapSeed {
            entity_index: theft.holder.source_slot,
            active: true,
            x: victim_position.x,
            y: victim_position.y,
        })
        .is_err()
    {
        return false;
    }

    if let Some(kind) = holder_view
        && !sync_dynamic_holder_from_raw(arena, kind, &holder)
    {
        return false;
    }
    arena.entities[holder_slot] = holder.clone();
    shared.sync_pickup_from_authority(&holder);

    if let Some(index) = player_index {
        shared.players[index] = player_after.clone();
    } else {
        shared.local_player = player_after.clone();
    }
    if !local_only && shared.local_player.slot == player_slot {
        shared
            .local_player
            .sync_spell_acquisition_from(&player_after);
    }
    true
}

fn apply_peer_updates(arena: &mut SourceArena, effects: &Effects) {
    for update in &effects.peer_updates {
        if arena.resolve(update.peer).is_none() {
            continue;
        }
        let Some(row) = arena
            .entities
            .get_mut(usize::from(update.peer.source_slot))
            .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        else {
            continue;
        };
        if let Some(action) = update.action {
            row.set_source_action(action);
        }
        if let Some(target) = update.target {
            put(row, 154, target);
        }
        if let Some(link) = update.link {
            put(row, 54, link);
        }
    }
}

/// Advance one model26 source action and immediately apply every world effect
/// whose consumer is already in this feature's boundary. The source kill
/// credit is committed to the authored-creature counter here, in the same
/// callback, before the dispatcher observes the result.
pub(crate) fn step_row(
    arena: &mut SourceArena,
    shared: &mut Level2SharedSpellResetSession,
    runtime: &RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    class3: &[u16],
    families: &[Vec<u16>; 29],
) -> Result<Effects, String> {
    let snapshots = targets(arena, shared);
    let handle = arena
        .handle(slot)
        .ok_or_else(|| format!("model26 callback lost generation-bound slot{slot}"))?;
    let mut effects = {
        let row = arena
            .entities
            .get_mut(usize::from(slot))
            .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
            .ok_or_else(|| format!("model26 callback lost slot{slot}"))?;
        let mut actor = Actor { row, handle };
        let mut context = Context {
            rules: &RULES,
            terrain,
            targets: &snapshots,
            class3,
            model26: &families[usize::from(SOURCE_MODEL)],
            runtime: Some(runtime),
            global_rng: &mut arena.economy.global_rng,
        };
        step(&mut actor, &mut context)?
    };

    apply_peer_updates(arena, &effects);
    for (handle, flags) in &effects.linked_blink_flags {
        if arena.resolve(*handle).is_some()
            && let Some(row) = arena.entities.get_mut(usize::from(handle.source_slot))
            && row.active
        {
            row.set_control_flags((row.control_flags & !0x0600_0000) | *flags);
        }
    }
    if let Some(killer) = effects.kill_credit.take() {
        arena.economy.record_authored_creature_kill(killer);
    }
    if let Some(drain) = effects.mana_drain {
        apply_mana_drain(arena, shared, drain);
    }
    if let Some(theft) = effects.spell_theft
        && apply_spell_theft(arena, shared, theft)
    {
        effects.committed_theft_victim = Some(theft.victim);
    }
    for _ in 0..effects.population_decrements {
        arena.economy.decrement_source_creature_population();
    }
    let position = arena.entities[usize::from(slot)].position;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if let Some(summoned) = effects.stage_expiry {
        super::super::creature_24::mc2::expire(arena, terrain, particles, slot, summoned)?;
        // sub_293B0 runs after the source expiry allocations, including when
        // one allocation reused the callback's fixed source address.
        super::generic_tail(&mut Actor {
            row: &mut arena.entities[usize::from(slot)],
            handle,
        });
    }
    if effects.stage_hide {
        if arena.entities[usize::from(slot)].active {
            arena.entities[usize::from(slot)].disable_drawing();
        }
    }
    if effects.kill {
        super::super::creature_24::mc2::kill(arena, terrain, particles, slot)?;
    }
    // All mutation intents above are committed here. Keep the return value as
    // an observation carrier for the scheduler, without asking it to replay a
    // score, drain, linked write, population change, hide, or terminal call.
    effects.peer_updates.clear();
    effects.linked_blink_flags.clear();
    effects.mana_drain = None;
    effects.spell_theft = None;
    effects.kill = false;
    effects.population_decrements = 0;
    effects.stage_hide = false;
    effects.stage_expiry = None;
    Ok(effects)
}

pub(crate) fn bind_source(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slot: u16,
    index: u8,
) -> Result<(), String> {
    super::super::stage_route::bind_source(arena, runtime, slot, index)
}

pub(crate) fn prepare_source_pass(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    targets: crate::native_world::source_target::Context<'_>,
    model_slots: &[u16],
) -> Result<(), String> {
    super::super::stage_route::prepare_source_pass(arena, runtime, targets, model_slots)?;
    sync_detached_holder_projections(arena);
    Ok(())
}

/// Ordinary authored model26 producer. Exhaustion leaves the constructor
/// counter untouched; once allocated, publication follows the source order:
/// construct, bind authored life, publish RNG and row, map, optional route,
/// then generation-bound authored origin.
pub(crate) fn allocate_authored(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    position: Position,
    max_life: i32,
    stage_index: Option<u8>,
    authored_record: u16,
) -> Result<Option<u16>, String> {
    let particle = particles
        .get(usize::from(super::PARTICLE))
        .ok_or_else(|| "model26 requires retained particle318".to_owned())?;
    if particle.animation_limit().is_none() {
        return Err("model26 particle318 has invalid animation mode".to_owned());
    }
    let _counter = runtime
        .constructor_counters()
        .ok_or_else(|| "model26 source constructor counter table is absent".to_owned())?
        [usize::from(SOURCE_MODEL)];
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let seed = arena.new_event_rng(slot);
    let mut row = construct_row(slot, seed, position, runtime, particles)?;
    row.bind_frame_zero_max_life(max_life);
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if let Some(index) = stage_index {
        bind_source(arena, runtime, slot, index)?;
    }
    let handle = arena
        .handle(slot)
        .ok_or_else(|| format!("model26 constructor lost slot{slot}"))?;
    if !arena.bind_raw_creature_origin(
        handle,
        crate::level_definition::CreatureKind::Creature26,
        crate::campaign::source_arena::CreatureRootOrigin::AuthoredRecord { authored_record },
    ) {
        return Err(format!(
            "model26 record{authored_record} lost generation-bound origin"
        ));
    }
    Ok(Some(slot))
}
